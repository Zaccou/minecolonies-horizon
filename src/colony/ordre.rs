//! A work order: one build, upgrade, repair or removal asked for at a place.
//!
//! Java reference: `core/colony/workorders/AbstractWorkOrder.java` (920 lines),
//! `WorkOrderBuilding`, `WorkManager.java` (542 lines) over
//! `api/colony/workorders`.
//!
//! **The state is not a field in Java, and the model says so.** An order's life
//! lives in two places there: `claimedBy`, a position, zero when no builder took
//! it (`AbstractWorkOrder.java:398-417`), and the order's mere presence in the
//! manager's map — a finished order is **removed** (`WorkManager.java:80-95`).
//! [`OrderState`] names that hidden state; [`WorkOrder::to_compound`] refuses to
//! save a finished order for the same reason.
//!
//! **One refusal Java does not have.** `setClaimedBy` on an order another
//! builder holds only logs a warning and overwrites the claim
//! (`AbstractWorkOrder.java:400-403`) — the warning is there because the
//! overwrite is a bug. The port returns [`TransitionRefusal::AlreadyClaimed`].

use std::cmp::Ordering;

use mod_api::{Compound, Tag};

use super::nbt;

/// The saved keys, with `AbstractWorkOrder`'s names (`:49-67`, written at
/// `:709-730`). `building` and the int form of `claimedByBuilding` are this
/// port's: Java identifies a building by its position.
pub const KEY_ID: &str = "id";
pub const KEY_PRIORITY: &str = "priority";
pub const KEY_TYPE: &str = "workOrderType";
pub const KEY_LOCATION: &str = "location";
pub const KEY_TARGET_LEVEL: &str = "targetLevel";
pub const KEY_CLAIMED_BY: &str = "claimedByBuilding";
pub const KEY_BUILDING: &str = "building";

/// `WorkOrderType` (`api/colony/workorders/WorkOrderType.java:8-13`), saved as
/// its ordinal (`AbstractWorkOrder.java:719`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum WorkType {
    Build,
    Upgrade,
    Repair,
    Remove,
}

impl WorkType {
    /// Every type, in the order that fixes the saved ordinals.
    pub const ALL: [WorkType; 4] = [
        WorkType::Build,
        WorkType::Upgrade,
        WorkType::Repair,
        WorkType::Remove,
    ];

    /// The ordinal saved in the compound (`AbstractWorkOrder.java:719`).
    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// The type an ordinal names. Unknown is `None`: guessing `Build` would
    /// start a build nobody asked for.
    pub fn from_ordinal(ordinal: i32) -> Option<WorkType> {
        WorkType::ALL
            .into_iter()
            .find(|work| work.ordinal() == ordinal)
    }
}

/// Where an order stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum OrderState {
    /// No builder holds it: `claimedBy` is at the origin
    /// (`AbstractWorkOrder.java:414-417`).
    Open,
    /// A builder's hut holds it, named by the hut's position
    /// (`AbstractWorkOrder.java:398-411`).
    Claimed { by: [i32; 3] },
    /// Built, upgraded, repaired or removed; Java has removed the order by then
    /// (`WorkManager.java:80-95`).
    Done,
    /// Called off before completion, removed the same way.
    Cancelled,
}

impl OrderState {
    /// Whether the order is still in the manager's map.
    pub fn is_live(self) -> bool {
        matches!(self, OrderState::Open | OrderState::Claimed { .. })
    }
}

/// Why a state change was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum TransitionRefusal {
    /// Another hut already holds the order (`AbstractWorkOrder.java:400-403`).
    AlreadyClaimed { by: [i32; 3] },
    /// The change needs a builder to hold the order first.
    NotClaimed,
    /// The order is over, and nothing in Java brings one back.
    Finished,
}

/// One asked-for piece of work.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct WorkOrder {
    /// The id the colony's counter gave it (`WorkManager.java:305-311`).
    pub id: i32,
    /// The kind of building, `minecolonies:builder` style. Java finds the
    /// building by position instead (`WorkOrderBuilding.java:47-63`).
    pub building: String,
    /// Where the work happens: `location` (`AbstractWorkOrder.java:466-469`).
    pub at: [i32; 3],
    /// What is asked for.
    pub work: WorkType,
    /// The level to reach; for a removal, the current one
    /// (`WorkOrderBuilding.java:63`).
    pub target_level: i32,
    /// Higher goes first. Starts at zero, which is Java's field default
    /// (`AbstractWorkOrder.java:95`).
    pub priority: i32,
    pub state: OrderState,
}

impl WorkOrder {
    /// A fresh, open order at priority zero.
    pub fn new(
        id: i32,
        building: &str,
        at: [i32; 3],
        work: WorkType,
        target_level: i32,
    ) -> WorkOrder {
        WorkOrder {
            id,
            building: building.to_owned(),
            at,
            work,
            target_level,
            priority: 0,
            state: OrderState::Open,
        }
    }

    /// A hut takes the order. Taking it again with the same hut is a no-op: a
    /// builder asking twice is not stealing from itself.
    pub fn claim(&mut self, builder: [i32; 3]) -> Result<(), TransitionRefusal> {
        match self.state {
            OrderState::Open => {
                self.state = OrderState::Claimed { by: builder };
                Ok(())
            }
            OrderState::Claimed { by } if by == builder => Ok(()),
            OrderState::Claimed { by } => Err(TransitionRefusal::AlreadyClaimed { by }),
            OrderState::Done | OrderState::Cancelled => Err(TransitionRefusal::Finished),
        }
    }

    /// The hut lets go and the order returns to the pool
    /// (`WorkManager.java:214`, `:271`, `:413`, `setClaimedBy(null)`).
    pub fn release(&mut self) -> Result<(), TransitionRefusal> {
        match self.state {
            OrderState::Claimed { .. } => {
                self.state = OrderState::Open;
                Ok(())
            }
            OrderState::Open => Err(TransitionRefusal::NotClaimed),
            OrderState::Done | OrderState::Cancelled => Err(TransitionRefusal::Finished),
        }
    }

    /// The work is over: the order is done and leaves the map
    /// (`WorkManager.java:80-95`).
    pub fn complete(&mut self) -> Result<(), TransitionRefusal> {
        match self.state {
            OrderState::Claimed { .. } => {
                self.state = OrderState::Done;
                Ok(())
            }
            OrderState::Open => Err(TransitionRefusal::NotClaimed),
            OrderState::Done | OrderState::Cancelled => Err(TransitionRefusal::Finished),
        }
    }

    /// Called off. Allowed from both live states — a manager may drop an order
    /// nobody took. The right is `ManageHuts`, checked by the message in Java
    /// (`AbstractColonyServerMessage.java:61-64`) and by
    /// [`Colony::cancel_order`](super::colony::Colony::cancel_order) here.
    pub fn cancel(&mut self) -> Result<(), TransitionRefusal> {
        match self.state {
            OrderState::Open | OrderState::Claimed { .. } => {
                self.state = OrderState::Cancelled;
                Ok(())
            }
            OrderState::Done | OrderState::Cancelled => Err(TransitionRefusal::Finished),
        }
    }

    /// The claim as Java keeps it: a position, or `None` for "nobody".
    pub fn claimed_by(&self) -> Option<[i32; 3]> {
        match self.state {
            OrderState::Claimed { by } => Some(by),
            _ => None,
        }
    }

    /// The saved form, or `None` for a finished order: Java has removed it
    /// before it could be saved (`WorkManager.java:80-95`), so saving one would
    /// resurrect work that is over.
    pub fn to_compound(&self) -> Option<Compound> {
        if !self.state.is_live() {
            return None;
        }
        let mut data = Compound::from([
            (KEY_ID.into(), Tag::Int(self.id)),
            (KEY_PRIORITY.into(), Tag::Int(self.priority)),
            (KEY_TYPE.into(), Tag::Int(self.work.ordinal())),
            (KEY_TARGET_LEVEL.into(), Tag::Int(self.target_level)),
            (
                KEY_BUILDING.into(),
                Tag::String(self.building.as_str().into()),
            ),
            (KEY_LOCATION.into(), nbt::position(self.at)),
        ]);
        if let Some(by) = self.claimed_by() {
            data.insert(KEY_CLAIMED_BY.into(), nbt::position(by));
        }
        Some(data)
    }

    /// Reads an order. `None` when there is no id, no position, or no work type
    /// the port knows: a half-read order is not put in the colony.
    pub fn from_compound(data: &Compound) -> Option<WorkOrder> {
        let work = WorkType::from_ordinal(nbt::int(data, KEY_TYPE)?)?;
        let state = match nbt::position_in(data, KEY_CLAIMED_BY) {
            Some(by) => OrderState::Claimed { by },
            None => OrderState::Open,
        };
        Some(WorkOrder {
            id: nbt::int(data, KEY_ID)?,
            building: nbt::text(data, KEY_BUILDING).unwrap_or_default(),
            at: nbt::position_in(data, KEY_LOCATION)?,
            work,
            target_level: nbt::int(data, KEY_TARGET_LEVEL)?,
            priority: nbt::int(data, KEY_PRIORITY).unwrap_or(0),
            state,
        })
    }
}

/// The order a builder takes first: higher priority wins, then the lower id —
/// `IWorkOrder.WORK_ORDER_COMPARATOR`, which reverses the priority and keeps the
/// id ascending (`api/colony/workorders/IWorkOrder.java:19-24`).
pub fn compare(left: &WorkOrder, right: &WorkOrder) -> Ordering {
    right
        .priority
        .cmp(&left.priority)
        .then_with(|| left.id.cmp(&right.id))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HUT: [i32; 3] = [10, 64, 10];
    const OTHER: [i32; 3] = [20, 64, 10];

    fn order() -> WorkOrder {
        WorkOrder::new(1, "minecolonies:builder", HUT, WorkType::Build, 1)
    }

    #[test]
    fn an_order_starts_open_and_only_the_hut_that_holds_it_keeps_it() {
        let mut order = order();
        assert_eq!(order.state, OrderState::Open);
        assert_eq!(order.claim(HUT), Ok(()));
        assert_eq!(order.state, OrderState::Claimed { by: HUT });
        assert_eq!(order.claim(HUT), Ok(()), "asking twice is not stealing");
        assert_eq!(
            order.claim(OTHER),
            Err(TransitionRefusal::AlreadyClaimed { by: HUT })
        );
        assert_eq!(order.state, OrderState::Claimed { by: HUT }, "the claim stands");
    }

    #[test]
    fn releasing_needs_a_claim_and_returns_the_order_to_the_pool() {
        let mut order = order();
        assert_eq!(order.release(), Err(TransitionRefusal::NotClaimed));
        order.claim(HUT).expect("claim");
        assert_eq!(order.release(), Ok(()));
        assert_eq!(order.state, OrderState::Open);
        assert_eq!(order.claimed_by(), None);
    }

    #[test]
    fn completing_needs_a_claim_and_nothing_follows_it() {
        let mut order = order();
        assert_eq!(order.complete(), Err(TransitionRefusal::NotClaimed));
        order.claim(HUT).expect("claim");
        assert_eq!(order.complete(), Ok(()));
        assert_eq!(order.state, OrderState::Done);
        assert_eq!(order.claim(OTHER), Err(TransitionRefusal::Finished));
        assert_eq!(order.cancel(), Err(TransitionRefusal::Finished));
        assert_eq!(order.release(), Err(TransitionRefusal::Finished));
    }

    #[test]
    fn cancelling_works_from_both_live_states_and_ends_the_order() {
        let mut open = order();
        assert_eq!(open.cancel(), Ok(()));
        assert_eq!(open.state, OrderState::Cancelled);
        assert_eq!(open.cancel(), Err(TransitionRefusal::Finished));

        let mut claimed = order();
        claimed.claim(HUT).expect("claim");
        assert_eq!(claimed.cancel(), Ok(()));
        assert!(!claimed.state.is_live());
    }

    #[test]
    fn orders_sort_by_priority_down_then_id_up() {
        let mut high = order();
        high.id = 7;
        high.priority = 5;
        let mut low = order();
        low.id = 9;
        low.priority = 1;
        let mut earlier = order();
        earlier.id = 3;
        earlier.priority = 1;
        assert_eq!(compare(&high, &low), Ordering::Less);
        assert_eq!(compare(&low, &high), Ordering::Greater);
        assert_eq!(compare(&low, &earlier), Ordering::Greater, "same priority, higher id later");
        assert_eq!(compare(&low, &low.clone()), Ordering::Equal);
    }

    #[test]
    fn an_order_round_trips_with_and_without_a_claim_and_a_finished_one_is_not_saved() {
        let mut order = order();
        let open = WorkOrder::from_compound(&order.to_compound().expect("open")).expect("read");
        assert_eq!(open, order);

        order.claim(HUT).expect("claim");
        order.priority = -2;
        let claimed =
            WorkOrder::from_compound(&order.to_compound().expect("claimed")).expect("read");
        assert_eq!(claimed, order);

        order.complete().expect("complete");
        assert_eq!(order.to_compound(), None, "a removed order is not resurrected");
    }

    #[test]
    fn an_order_of_an_unknown_type_is_not_read() {
        let mut data = order().to_compound().expect("open");
        data.insert(KEY_TYPE.into(), Tag::Int(9));
        assert_eq!(WorkOrder::from_compound(&data), None);
        data.insert(KEY_TYPE.into(), Tag::Int(WorkType::Repair.ordinal()));
        assert_eq!(
            WorkOrder::from_compound(&data).map(|order| order.work),
            Some(WorkType::Repair)
        );
    }
}
