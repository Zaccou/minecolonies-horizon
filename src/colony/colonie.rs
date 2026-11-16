//! The colony record: identity, citizens, buildings, work orders, members.
//!
//! Java reference: `core/colony/Colony.java` (1 400 lines) and the managers a
//! colony record is made of — `managers/CitizenManager`,
//! `managers/RegisteredStructureManager`, `workorders/WorkManager`.
//!
//! It is the part of a colony that survives a restart: an id, a name, a level, a
//! dimension, a centre, who lives there, what is built, what is asked for, and
//! who may touch it — not the live half (no citizen AI, no request system, no
//! research, no raid), which is a manager of its own in Java and a slice of its
//! own here.
//!
//! **Two rules the record enforces, both from `WorkManager`.**
//!
//! - The id counter is not saved: it is the highest id among the saved orders,
//!   and the next order takes that plus one (`WorkManager.java:274`,
//!   `:305-311`). A stale counter would hand out an id twice.
//! - A claim on a building that no longer exists is released on read
//!   (`WorkManager.java:269-273`): the hut was broken, the order must not stay
//!   locked to a hole in the ground.

use mod_api::{Compound, Tag};

use super::nbt;
use super::ordre::{TransitionRefusal, WorkOrder, WorkType};
use super::permissions::{Action, Members, Rank, Refusal};

/// The colony's saved keys, with Java's names: `id`, `name`, `dimension` and
/// `center` from `NbtTagConstants.java:8-12`, `citizens` from `:19`,
/// `buildings` from `:13`, `workOrders` from `WorkManager.java:47`. The port
/// flattens Java's manager nesting and keeps the leaf names.
pub const KEY_ID: &str = "id";
pub const KEY_NAME: &str = "name";
pub const KEY_LEVEL: &str = "level";
pub const KEY_DIMENSION: &str = "dimension";
pub const KEY_CENTER: &str = "center";
pub const KEY_CITIZENS: &str = "citizens";
pub const KEY_BUILDINGS: &str = "buildings";
pub const KEY_ORDERS: &str = "workOrders";

/// One citizen's keys (`NbtTagConstants.java:8`, `:9`).
pub const KEY_CITIZEN_ID: &str = "id";
pub const KEY_CITIZEN_NAME: &str = "name";

/// One building's keys: `type`, `buildingLevel`, `location`
/// (`NbtTagConstants.java:140`, `:130`, `:145`).
pub const KEY_BUILDING_TYPE: &str = "type";
pub const KEY_BUILDING_LEVEL: &str = "buildingLevel";
pub const KEY_BUILDING_AT: &str = "location";

/// The bounds of a building level (`api/util/constant/Constants.java:20`), and
/// of a colony's level with it: MineColonies has no colony level of its own, it
/// is the town hall's building level. Same two bounds as `crate::hut`.
pub const LEVEL_MIN: i32 = 1;
pub const LEVEL_MAX: i32 = 5;

/// A citizen of the colony. Java's `ICitizenData` carries a job, a family, a
/// happiness; none of that belongs to the colony record.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Citizen {
    pub id: i32,
    pub name: String,
}

impl Citizen {
    pub fn new(id: i32, name: &str) -> Citizen {
        Citizen {
            id,
            name: name.to_owned(),
        }
    }

    fn to_compound(&self) -> Compound {
        Compound::from([
            (KEY_CITIZEN_ID.into(), Tag::Int(self.id)),
            (KEY_CITIZEN_NAME.into(), Tag::String(self.name.as_str().into())),
        ])
    }

    fn from_compound(data: &Compound) -> Option<Citizen> {
        Some(Citizen {
            id: nbt::int(data, KEY_CITIZEN_ID)?,
            name: nbt::text(data, KEY_CITIZEN_NAME).unwrap_or_default(),
        })
    }
}

/// A building. It has no id of its own, as in Java: **a building is its
/// position**, which is what the manager keys on and what a claim points at
/// (`WorkManager.java:450`).
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Building {
    pub kind: String,
    pub at: [i32; 3],
    pub level: i32,
}

impl Building {
    pub fn new(kind: &str, at: [i32; 3], level: i32) -> Building {
        Building {
            kind: kind.to_owned(),
            at,
            level,
        }
    }

    fn to_compound(&self) -> Compound {
        Compound::from([
            (KEY_BUILDING_TYPE.into(), Tag::String(self.kind.as_str().into())),
            (KEY_BUILDING_LEVEL.into(), Tag::Int(self.level)),
            (KEY_BUILDING_AT.into(), nbt::position(self.at)),
        ])
    }

    fn from_compound(data: &Compound) -> Option<Building> {
        Some(Building {
            kind: nbt::text(data, KEY_BUILDING_TYPE).unwrap_or_default(),
            at: nbt::position_in(data, KEY_BUILDING_AT)?,
            level: nbt::int(data, KEY_BUILDING_LEVEL).unwrap_or(0),
        })
    }
}

/// Why an order operation on a colony was refused: the right to touch the
/// order, or the state of the order itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum OrderRefusal {
    /// No live order has that id. Java silently does nothing on a missing id
    /// (`WorkManager.java:80-95`); the port says which one it did not find.
    Unknown { id: i32 },
    /// The player may not manage the colony's huts
    /// (`AbstractColonyServerMessage.java:61-64`).
    Denied(Refusal),
    /// The order does not accept the change.
    Transition(TransitionRefusal),
}

/// A colony.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Colony {
    pub id: i32,
    pub name: String,
    /// The town hall's level, kept in 1..=5 by [`Colony::set_level`].
    level: i32,
    /// `dimensionId.location().toString()` (`Colony.java:917`).
    pub dimension: String,
    /// Where the town hall stands (`Colony.java:921`).
    pub center: [i32; 3],
    pub citizens: Vec<Citizen>,
    pub buildings: Vec<Building>,
    /// The live orders. A finished one leaves this list (`WorkManager.java:80-95`).
    pub orders: Vec<WorkOrder>,
    /// The last id handed out: Java's `topWorkOrderId` (`WorkManager.java:58`,
    /// `:305-311`). Never saved; a read recomputes it (`:274`).
    last_order_id: i32,
    pub members: Members,
}

impl Colony {
    /// A colony founded by `founder`, who becomes its owner
    /// (`Permissions.java:506`), at level 1, empty.
    pub fn new(id: i32, name: &str, dimension: &str, center: [i32; 3], founder: u128) -> Colony {
        Colony {
            id,
            name: name.to_owned(),
            level: LEVEL_MIN,
            dimension: dimension.to_owned(),
            center,
            citizens: Vec::new(),
            buildings: Vec::new(),
            orders: Vec::new(),
            last_order_id: 0,
            members: Members::with_owner(founder),
        }
    }

    /// The colony's level, a town hall level.
    pub fn level(&self) -> i32 {
        self.level
    }

    /// Sets the level, clamped: a `/data merge` or a bad save cannot put a
    /// colony outside the range every other rule assumes.
    pub fn set_level(&mut self, level: i32) {
        self.level = level.clamp(LEVEL_MIN, LEVEL_MAX);
    }

    /// The last order id handed out. Read-only: the counter is the colony's to
    /// hand out, never a caller's to set.
    pub fn last_order_id(&self) -> i32 {
        self.last_order_id
    }

    /// **The resolution.** May `player` do `action` in **this** colony? `Ok(())`
    /// is yes; the error is the named refusal, and there is no bare `false`
    /// anywhere in the port.
    pub fn may(&self, player: u128, action: Action) -> Result<(), Refusal> {
        self.members.may(player, action)
    }

    /// May `actor` give `target` a rank? See [`Members::may_invite`].
    pub fn may_invite(&self, actor: u128, target: Rank) -> Result<(), Refusal> {
        self.members.may_invite(actor, target)
    }

    /// May `actor` take `target` out? See [`Members::may_expel`].
    pub fn may_expel(&self, actor: u128, target: u128) -> Result<(), Refusal> {
        self.members.may_expel(actor, target)
    }

    /// A building by position, which is how Java identifies one.
    pub fn building_at(&self, at: [i32; 3]) -> Option<&Building> {
        self.buildings.iter().find(|building| building.at == at)
    }

    /// A live order by id.
    pub fn order(&self, id: i32) -> Option<&WorkOrder> {
        self.orders.iter().find(|order| order.id == id)
    }

    /// Queues an order and returns its id. Two rules from `addWorkOrder`
    /// (`WorkManager.java:286-329`): an order at a position that already has
    /// one **replaces** it (`:290-300` — Java also compares the structure path,
    /// which the port does not have), and an id-less order takes the next id of
    /// the counter (`:305-311`).
    pub fn place_order(
        &mut self,
        building: &str,
        at: [i32; 3],
        work: WorkType,
        target_level: i32,
    ) -> i32 {
        self.orders.retain(|order| order.at != at);
        self.last_order_id += 1;
        self.orders
            .push(WorkOrder::new(self.last_order_id, building, at, work, target_level));
        self.last_order_id
    }

    /// Marks a claimed order done and takes it out of the list
    /// (`WorkManager.java:80-95`).
    pub fn finish_order(&mut self, id: i32) -> Result<(), OrderRefusal> {
        let Some(index) = self.orders.iter().position(|order| order.id == id) else {
            return Err(OrderRefusal::Unknown { id });
        };
        self.orders[index]
            .complete()
            .map_err(OrderRefusal::Transition)?;
        self.orders.remove(index);
        Ok(())
    }

    /// Calls an order off on a player's behalf: the `ManageHuts` bit first, then
    /// the state machine. Java puts the bit on the message
    /// (`AbstractColonyServerMessage.java:61-64`) and the rest on the order; the
    /// port does both here so a caller cannot forget one.
    pub fn cancel_order(&mut self, actor: u128, id: i32) -> Result<(), OrderRefusal> {
        let Some(index) = self.orders.iter().position(|order| order.id == id) else {
            return Err(OrderRefusal::Unknown { id });
        };
        self.members
            .may(actor, Action::ManageHuts)
            .map_err(OrderRefusal::Denied)?;
        self.orders[index]
            .cancel()
            .map_err(OrderRefusal::Transition)?;
        self.orders.remove(index);
        Ok(())
    }

    /// Releases claims whose building is gone, as Java does on read
    /// (`WorkManager.java:269-273`). The positions are copied out first:
    /// `building_at` takes the whole colony, which cannot be borrowed while
    /// `orders` is borrowed mutably.
    pub fn release_orphan_claims(&mut self) {
        let standing: Vec<[i32; 3]> = self.buildings.iter().map(|building| building.at).collect();
        for order in &mut self.orders {
            let Some(by) = order.claimed_by() else {
                continue;
            };
            if !standing.contains(&by) {
                // A claimed order is the only state this meets, and `release`
                // cannot fail on one.
                let _ = order.release();
            }
        }
    }

    /// The colony as a compound.
    pub fn to_compound(&self) -> Compound {
        let mut data = Compound::from([
            (KEY_ID.into(), Tag::Int(self.id)),
            (KEY_NAME.into(), Tag::String(self.name.as_str().into())),
            (KEY_LEVEL.into(), Tag::Int(self.level)),
            (
                KEY_DIMENSION.into(),
                Tag::String(self.dimension.as_str().into()),
            ),
            (KEY_CENTER.into(), nbt::position(self.center)),
            (
                KEY_CITIZENS.into(),
                nbt::compound_list(self.citizens.iter().map(Citizen::to_compound).collect()),
            ),
            (
                KEY_BUILDINGS.into(),
                nbt::compound_list(self.buildings.iter().map(Building::to_compound).collect()),
            ),
            (
                KEY_ORDERS.into(),
                nbt::compound_list(
                    self.orders
                        .iter()
                        .filter_map(WorkOrder::to_compound)
                        .collect(),
                ),
            ),
        ]);
        for (key, tag) in self.members.to_compound() {
            data.insert(key, tag);
        }
        data
    }

    /// Reads a colony. Every field has a defensible default, so a truncated
    /// compound gives a smaller colony rather than nothing.
    pub fn from_compound(data: &Compound) -> Colony {
        let orders: Vec<WorkOrder> = nbt::compounds(data, KEY_ORDERS)
            .into_iter()
            .filter_map(WorkOrder::from_compound)
            .collect();
        let mut colony = Colony {
            id: nbt::int(data, KEY_ID).unwrap_or(0),
            name: nbt::text(data, KEY_NAME).unwrap_or_default(),
            level: LEVEL_MIN,
            dimension: nbt::text(data, KEY_DIMENSION).unwrap_or_default(),
            center: nbt::position_in(data, KEY_CENTER).unwrap_or([0, 0, 0]),
            citizens: nbt::compounds(data, KEY_CITIZENS)
                .into_iter()
                .filter_map(Citizen::from_compound)
                .collect(),
            buildings: nbt::compounds(data, KEY_BUILDINGS)
                .into_iter()
                .filter_map(Building::from_compound)
                .collect(),
            // Recomputed from the orders, never read back (`WorkManager.java:274`).
            last_order_id: orders.iter().map(|order| order.id).max().unwrap_or(0),
            orders,
            members: Members::from_compound(data),
        };
        colony.set_level(nbt::int(data, KEY_LEVEL).unwrap_or(LEVEL_MIN));
        colony.release_orphan_claims();
        colony
    }
}

#[cfg(test)]
mod tests {
    use super::super::ordre::OrderState;
    use super::*;

    const OWNER: u128 = 1;
    const FRIEND: u128 = 2;
    const HUT: [i32; 3] = [10, 64, 10];
    const HUT_KIND: &str = "minecolonies:blockhutbuilder";

    fn colony() -> Colony {
        let mut colony = Colony::new(7, "Bridgewater", "minecraft:overworld", [0, 70, 0], OWNER);
        colony.buildings.push(Building::new(HUT_KIND, HUT, 1));
        colony.members.set(FRIEND, Rank::Friend);
        colony
    }

    fn claimed(colony: &mut Colony, id: i32) {
        let index = colony.orders.iter().position(|order| order.id == id).unwrap();
        colony.orders[index].claim(HUT).expect("the builder takes it");
    }

    #[test]
    fn a_new_colony_is_level_one_with_its_founder_as_owner() {
        let mut colony = colony();
        assert_eq!(colony.level(), 1);
        assert_eq!(colony.members.rank_of(OWNER), Rank::Owner);
        assert_eq!(colony.may(OWNER, Action::PlaceHuts), Ok(()));
        assert_eq!(
            colony.may(FRIEND, Action::PlaceHuts),
            Err(Refusal::LacksAction {
                rank: Rank::Friend,
                action: Action::PlaceHuts
            })
        );
    }

    #[test]
    fn the_level_is_kept_between_one_and_five() {
        let mut colony = colony();
        colony.set_level(9);
        assert_eq!(colony.level(), LEVEL_MAX);
        colony.set_level(-4);
        assert_eq!(colony.level(), LEVEL_MIN);
    }

    #[test]
    fn placing_an_order_hands_out_an_id_and_a_second_one_here_replaces_it() {
        let mut colony = colony();
        let first = colony.place_order(HUT_KIND, HUT, WorkType::Build, 1);
        let second = colony.place_order(HUT_KIND, HUT, WorkType::Upgrade, 2);
        assert_eq!((first, second), (1, 2), "the counter moved anyway, as in Java");
        assert_eq!(colony.orders.len(), 1, "one order where one hut stands");
        assert_eq!(colony.order(first).map(|order| order.work), None);
        assert_eq!(colony.place_order(HUT_KIND, [40, 64, 40], WorkType::Build, 1), 3);
    }

    #[test]
    fn the_counter_is_recomputed_on_read_and_never_reused() {
        let mut colony = colony();
        colony.place_order(HUT_KIND, HUT, WorkType::Build, 1);
        colony.place_order(HUT_KIND, [40, 64, 40], WorkType::Build, 1);
        let mut read = Colony::from_compound(&colony.to_compound());
        assert_eq!(read.last_order_id(), 2, "the highest saved id");
        assert_eq!(
            read.place_order(HUT_KIND, [70, 64, 70], WorkType::Build, 1),
            3,
            "no id is handed out twice"
        );
    }

    #[test]
    fn a_claim_on_a_building_that_is_gone_is_released_on_read() {
        let mut colony = colony();
        let id = colony.place_order(HUT_KIND, HUT, WorkType::Build, 1);
        claimed(&mut colony, id);
        assert_eq!(
            Colony::from_compound(&colony.to_compound())
                .order(id)
                .map(|order| order.state),
            Some(OrderState::Claimed { by: HUT }),
            "the hut exists, so the claim stands"
        );
        colony.buildings.clear();
        assert_eq!(
            Colony::from_compound(&colony.to_compound())
                .order(id)
                .map(|order| order.state),
            Some(OrderState::Open),
            "the hut is gone, so nobody holds the order"
        );
    }

    #[test]
    fn cancelling_needs_the_manage_bit_and_finishing_needs_a_claim() {
        let mut colony = colony();
        let id = colony.place_order(HUT_KIND, HUT, WorkType::Build, 1);
        assert_eq!(
            colony.cancel_order(FRIEND, id),
            Err(OrderRefusal::Denied(Refusal::LacksAction {
                rank: Rank::Friend,
                action: Action::ManageHuts
            }))
        );
        assert!(colony.order(id).is_some(), "a refused cancel changes nothing");
        assert_eq!(
            colony.finish_order(id),
            Err(OrderRefusal::Transition(TransitionRefusal::NotClaimed))
        );
        claimed(&mut colony, id);
        assert_eq!(colony.finish_order(id), Ok(()));
        assert_eq!(colony.order(id), None);

        let other = colony.place_order(HUT_KIND, HUT, WorkType::Build, 1);
        assert_eq!(colony.cancel_order(OWNER, other), Ok(()));
        assert_eq!(colony.order(other), None);
        assert_eq!(
            colony.cancel_order(OWNER, other),
            Err(OrderRefusal::Unknown { id: other })
        );
    }

    #[test]
    fn a_colony_round_trips_and_nothing_finished_is_saved() {
        let mut colony = colony();
        colony.set_level(4);
        colony.citizens.push(Citizen::new(1, "Ada"));
        colony.citizens.push(Citizen::new(2, "Bram"));
        let id = colony.place_order(HUT_KIND, HUT, WorkType::Build, 1);
        claimed(&mut colony, id);
        colony.orders[0].priority = 3;

        let read = Colony::from_compound(&colony.to_compound());
        assert_eq!(read, colony);
        assert_eq!(read.name, "Bridgewater");
        assert_eq!(read.dimension, "minecraft:overworld");
        assert_eq!(read.center, [0, 70, 0]);

        assert_eq!(colony.finish_order(id), Ok(()));
        assert_eq!(nbt::compounds(&colony.to_compound(), KEY_ORDERS).len(), 0);
    }

    #[test]
    fn an_empty_compound_reads_as_an_empty_level_one_colony() {
        let colony = Colony::from_compound(&Compound::new());
        assert_eq!((colony.id, colony.level()), (0, LEVEL_MIN));
        assert!(colony.orders.is_empty());
        assert!(colony.members.iter().next().is_none(), "no member, no owner");
    }
}
