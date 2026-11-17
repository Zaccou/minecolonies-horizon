//! The board of work orders a colony holds: what is accepted, who takes it, and in
//! which order a removal is written.
//!
//! Java reference: `core/colony/workorders/WorkManager.java` (542 lines), over
//! `api/colony/workorders/IWorkManager.java`, `WorkOrderBuilding.java` (the builder's
//! three conditions) and `AbstractBuildingStructureBuilder` (the two facts a builder
//! hut carries).
//!
//! | Rule | Java | Here |
//! | --- | --- | --- |
//! | a refused order costs the colony nothing | `:288-306` | [`add`] |
//! | the six callbacks of a removal, in order | `:95-118` | [`remove`] |
//! | a citizen's hut lets go of its orders | `:205-218` | [`clear_work_for`] |
//! | the tick's three passes | `:386-430` | [`tick`] |
//! | who may take an order, and why the others did not | `:437-483` | [`decide`] |
//! | a builder's three conditions | `WorkOrderBuilding.java:154-182` | [`can_build`] |
//! | the order a builder is offered | `:506-514` | [`ordered_for`] |
//! | the colony's box, chunk by chunk | `:343-379` | [`within_colony`] |
//!
//! **What is already ported, and therefore not here**: `read` (`:246-283`) and
//! `write` (`:226-238`) are `crate::persistence::{read_orders, written_work}`;
//! `getWorkOrder` (`:148-151`), `getWorkOrdersOfType` (`:183-188`) and the id counter
//! are `Colony::order` / `Colony::place_order`; the four state transitions are
//! `crate::colony::ordre`; the priority comparator is `ordre::compare`.
//!
//! **The shape is Java's, and it is the shape that makes the order a rule.** Every
//! method here answers in two steps: a **refusal**, named, before any write; then the
//! writes themselves, announced through a [`Sink`] in Java's order. A test reads the
//! journal, so "nothing was written" is an empty vector and not a comment.
//!
//! # Three differences from the Java, all of them deliberate
//!
//! 1. **A refused order does not cost the incumbent.** `addWorkOrder` sets `dirty`
//!    (`:288`), finds the order already standing at that position and **removes it**
//!    (`:292-299`), and only then answers the out-of-colony refusal (`:301-305`). A
//!    player who places a hut outside their colony therefore loses the order that was
//!    there. [`add`] answers the refusal first, and the incumbent survives.
//! 2. **Which builder wins is specified here and is not in Java.**
//!    `tryAssignWorkOrder` walks `getBuildings().values()`, a `HashMap` (`:439`): two
//!    qualified builders and the winner is whatever the hash order says. [`tick`]
//!    walks the caller's slice in its order, so two runs of the same colony agree —
//!    the same choice `manager/claims.rs:195-197` makes against
//!    `HashSet.iterator().next()`.
//! 3. **`dirty` is a journal entry, not a field.** Java keeps one flag on the manager
//!    (`:59`) and one on the colony; the colony record of this port has neither, so
//!    [`Write::ManagerDirty`] and [`Write::ColonyDirty`] say *that* a save is owed and
//!    the caller decides where the flag lives.
//!
//! # What is not here
//!
//! The per-order `changed` flag (`AbstractWorkOrder.java:547-557`, read at
//! `WorkManager.java:404-407`) has no field in `colony::WorkOrder`, so the tick cannot
//! re-dirty the colony for it; `getWorkOrder(id, Class)`, `getWorkOrdersOfType` and
//! `getUnassignedWorkOrder` are keyed by a Java **class** in Java and by a
//! [`WorkType`] here, which is a different partition of the same orders; and the
//! advancements and the out-of-colony chat message (`:315-331`, `:304`) need a player.
//! `MANQUANT.md` §3 and §4 carry each one with its cost.

use std::collections::BTreeSet;

use crate::building::{Footprint, Kind, MAX_BUILDING_LEVEL};
use crate::colony::ordre::{self, WorkOrder, WorkType};
use crate::colony::{Colony, OrderRefusal};
use crate::manager::ClaimBook;
use crate::requests::distance;

/// The builder's mode setting, `BuildingBuilder.MODE`
/// (`core/colony/buildings/workerbuildings/BuildingBuilder.java:42`): a
/// `ResourceLocation(MOD_ID, "mode")`, which is what its `SettingsFactory` keys on.
pub const MODE_SETTING: &str = "minecolonies:mode";

/// The value that means "only what I am told": `BuildingBuilder.MANUAL_SETTING`
/// (`:48`), compared at `WorkManager.java:466`.
pub const MANUAL_SETTING: &str = "com.minecolonies.core.builder.setting.manual";

/// How far a builder hut may reach. Java compares squared distances against
/// `MAX_DISTANCE_SQ = 100 * 100` (`WorkOrderBuilding.java:30`, used at `:164`); the
/// port compares the distance itself, which is the same test and says "100 blocks".
pub const MAX_DISTANCE: f64 = 100.0;

/// One chunk is 16 blocks on each axis, which is `x >> 4` (`WorkManager.java:365-366`).
const CHUNK_SIZE: i32 = 16;

/// A hut as the assignment sees it.
///
/// Three of its six facts come from the port's building record —
/// `building::Building`'s `kind`, `level` and `worker` — and three do not exist
/// there: whether the hut is a *structure* builder, whether it already holds an
/// order, and whether its mode is manual. Java answers the last two from fields of
/// `AbstractBuildingStructureBuilder` and `BuildingBuilder`
/// (`WorkManager.java:449-452`, `:465-469`); [`Hut::of`] takes them as parameters
/// and `MANQUANT.md` §4 keeps the count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hut {
    pub at: [i32; 3],
    pub level: i32,
    /// `building instanceof AbstractBuildingStructureBuilder` (`:441`).
    pub builder: bool,
    /// `getModule(WorkerBuildingModule.class).getFirstCitizen() != null` (`:443-447`).
    pub staffed: bool,
    /// `abstractBuildingStructureBuilder.hasWorkOrder()` (`:449-452`).
    pub busy: bool,
    /// `getSetting(MODE).getValue().equals(MANUAL_SETTING)` (`:465-469`).
    pub manual: bool,
}

impl Hut {
    /// A builder hut that is staffed, idle and in automatic mode, at the origin and
    /// level 1: the ordinary case, so a test can write only the field it is about.
    pub fn builder() -> Hut {
        Hut { at: [0, 0, 0], level: 1, builder: true, staffed: true, busy: false, manual: false }
    }

    /// The three facts the port's building record already holds, plus the two it does
    /// not. A hut whose worker is absent is not staffed, which is Java's
    /// `getFirstCitizen() == null` (`:443-447`).
    pub fn of(hut: &crate::building::Building, busy: bool, manual: bool) -> Hut {
        Hut {
            at: hut.at(),
            level: hut.level(),
            // The structure builders are the builder and the miner, the two kinds
            // whose Java class extends `AbstractBuildingStructureBuilder`
            // (`BuildingBuilder.java`, `BuildingMiner.java`).
            builder: matches!(hut.kind(), Kind::Builder | Kind::Miner),
            staffed: hut.worker().is_some(),
            busy,
            manual,
        }
    }
}

/// Why one hut passed on one order. Java says nothing about it — it `continue`s
/// (`:441-483`) — but a colony whose orders nobody takes has to be diagnosable, and
/// the order of these variants is the order Java asks the questions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the six ways the loop of WorkManager.java:441-483 moves on
pub enum Skip {
    /// Not a structure builder (`:441`).
    NotABuilder { at: [i32; 3] },
    /// No citizen is assigned to the hut (`:443-447`).
    NoCitizen { at: [i32; 3] },
    /// The hut already holds another order (`:449-452`).
    Busy { at: [i32; 3] },
    /// The order is claimed by another hut (`:460`).
    HeldElsewhere { at: [i32; 3], by: [i32; 3] },
    /// Automatic mode is off for this hut (`:465-469`).
    Manual { at: [i32; 3] },
    /// The predicate failed — for a builder, one of the three conditions of
    /// `WorkOrderBuilding.java:154-165`.
    CannotBuild { at: [i32; 3] },
}

impl Skip {
    pub fn name(self) -> &'static str {
        match self {
            Self::NotABuilder { .. } => "work.skip.not_a_builder",
            Self::NoCitizen { .. } => "work.skip.no_citizen",
            Self::Busy { .. } => "work.skip.busy",
            Self::HeldElsewhere { .. } => "work.skip.held",
            Self::Manual { .. } => "work.skip.manual",
            Self::CannotBuild { .. } => "work.skip.cannot_build",
        }
    }

    pub fn at(self) -> [i32; 3] {
        match self {
            Self::NotABuilder { at }
            | Self::NoCitizen { at }
            | Self::Busy { at }
            | Self::HeldElsewhere { at, .. }
            | Self::Manual { at }
            | Self::CannotBuild { at } => at,
        }
    }
}

/// The answer to "who takes this order".
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: Java either returns from the loop or falls off its end
pub enum Decision {
    /// A hut took it. `held` is true when the hut is the one that already had it —
    /// Java re-sets the same claim (`:456-459`), which the port records as such so a
    /// reader can tell a confirmation from a new claim.
    Take { by: [i32; 3], held: bool },
    /// Nobody did, and here is why each hut passed.
    Nobody { skips: Vec<Skip> },
}

/// One write, in the order Java makes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the twelve writes of WorkManager.java a caller can be owed
pub enum Write {
    /// `dirty = true` on the manager (`:109`, `:288`; `:400` for an invalid order).
    ManagerDirty,
    /// `colony.markDirty()` (`:116`).
    ColonyDirty,
    /// `abstractBuildingStructureBuilder.onWorkOrderCancellation(order)` (`:105`).
    BuilderTold { id: i32, by: [i32; 3] },
    /// `workOrders.remove(orderId)` (`:110`), and `iter.remove()` for an invalid one
    /// (`:399`).
    Removed { id: i32 },
    /// `colony.removeWorkOrderInView(orderId)` (`:111`).
    ViewTold { id: i32 },
    /// `builderWorkOrder.onRemoved(colony)` (`:114`).
    OrderNotified { id: i32 },
    /// `workOrders.put(order.getID(), order)` (`:329`).
    Placed { id: i32 },
    /// `order.setClaimedBy(building.getID())`, a new claim (`:474`).
    Claimed { id: i32, by: [i32; 3] },
    /// The same write for the hut that already held it (`:459`).
    Reaffirmed { id: i32, by: [i32; 3] },
    /// `order.setClaimedBy(null)` (`:214`) or `BlockPos.ZERO` (`:413`), through the
    /// order's own state machine (`colony::ordre`).
    Released { id: i32 },
    /// How many claims the orphan pass above released at once (`:410-413`).
    ClaimsReleased { count: usize },
    /// An order the colony no longer has a building for, dropped by the tick
    /// (`:397-400`).
    Dropped { id: i32 },
}

impl Write {
    pub fn name(self) -> &'static str {
        match self {
            Self::ManagerDirty => "work.write.manager_dirty",
            Self::ColonyDirty => "work.write.colony_dirty",
            Self::BuilderTold { .. } => "work.write.builder_told",
            Self::Removed { .. } => "work.write.removed",
            Self::ViewTold { .. } => "work.write.view",
            Self::OrderNotified { .. } => "work.write.order_notified",
            Self::Placed { .. } => "work.write.placed",
            Self::Claimed { .. } => "work.write.claimed",
            Self::Reaffirmed { .. } => "work.write.reaffirmed",
            Self::Released { .. } => "work.write.released",
            Self::ClaimsReleased { .. } => "work.write.claims_released",
            Self::Dropped { .. } => "work.write.dropped",
        }
    }
}

/// Somewhere to announce a write. A `Vec<Write>` is one, so a test's journal is
/// `&mut journal` and its assertion is a slice comparison.
pub trait Sink {
    fn write(&mut self, write: Write);
}

impl Sink for Vec<Write> {
    fn write(&mut self, write: Write) {
        self.push(write);
    }
}

/// What the tick did, for a caller that would otherwise have to diff the colony: the
/// counts, and the reason each order nobody took stayed where it was.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tick {
    pub dropped: usize,
    pub released: usize,
    pub taken: usize,
    pub reaffirmed: usize,
    pub unclaimed: Vec<(i32, Vec<Skip>)>,
}

/// Why [`add`] refused, named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two ways a fresh order is refused
pub enum AddRefusal {
    /// The structure reaches a chunk this colony does not own (`:301-305`, the
    /// `OUT_OF_COLONY` message). `at` is where the order would have stood.
    OutOfColony { at: [i32; 3] },
    /// The order standing at that position, which the new one replaces, could not be
    /// taken out (`:292-299`). Unreachable from a colony whose list agrees with
    /// itself; carried as a variant rather than folded into the refusal above,
    /// because a reader looking at an out-of-colony message would never find it.
    Incumbent(OrderRefusal),
}

impl AddRefusal {
    pub fn name(self) -> &'static str {
        match self {
            Self::OutOfColony { .. } => "work.refusal.out_of_colony",
            Self::Incumbent(_) => "work.refusal.incumbent",
        }
    }
}

/// Where a fresh order stands, as the out-of-colony check sees it.
pub struct Site<'a> {
    /// The structure's box, which is what `ColonyUtils.calculateCorners` gives Java
    /// (`:346-352`). `crate::structures::place::Placing::boxed` is the same box here.
    ///
    /// `None` is **a gap of this port, not a rule**: the caller that places a hut has
    /// the blueprint's size, so it can always answer, and a caller that does not
    /// skips the check. `MANQUANT.md` §3 says so.
    pub bounds: Option<&'a Footprint>,
    /// The colony's claims, which is what `ColonyUtils.getOwningColony` reads in Java
    /// (`:371-374`) and `crate::manager::ClaimBook` answers here.
    pub book: &'a ClaimBook,
}

/// The three conditions of `WorkOrderBuilding.canBuild` (`:154-165`).
pub fn can_build(order: &WorkOrder, hut: &Hut) -> bool {
    // `building instanceof BuildingBuilder` (`:156`).
    hut.builder
        // `canBuildIgnoringDistance(building, building.getPosition(), building.getBuildingLevel())`
        // (`:158`).
        && can_build_ignoring_distance(order, hut)
        // `building.getPosition().distSqr(getLocation()) <= MAX_DISTANCE_SQ` (`:164`).
        && distance(hut.at, order.at) <= MAX_DISTANCE
}

/// `WorkOrderBuilding.canBuildIgnoringDistance` (`:175-182`): a builder reaches a
/// level above its own only if it is at the top level, or if the order is for its own
/// hut.
pub fn can_build_ignoring_distance(order: &WorkOrder, hut: &Hut) -> bool {
    hut.level >= order.target_level || hut.level == MAX_BUILDING_LEVEL || hut.at == order.at
}

/// The order is still worth keeping: its building stands
/// (`WorkOrderBuilding.java:201-205` over `AbstractWorkOrder.java:642-645`).
///
/// The base class answers `true` for every order and the building order narrows it,
/// which is what the port keeps: a colony order in this port is always *for a place*.
pub fn valid(colony: &Colony, order: &WorkOrder) -> bool {
    colony.building_at(order.at).is_some()
}

/// Who takes `order`, in Java's order of questions (`:441-483`).
///
/// The loop stops at the **first** hut that answers yes, so the order of `huts` is the
/// answer when two qualify — see the module's second difference.
pub fn decide(order: &WorkOrder, huts: &[Hut]) -> Decision {
    let mut skips = Vec::new();
    for hut in huts {
        // `if (building instanceof AbstractBuildingStructureBuilder …)` (`:441`).
        if !hut.builder {
            skips.push(Skip::NotABuilder { at: hut.at });
            continue;
        }
        // `getFirstCitizen() == null` → `continue` (`:443-447`).
        if !hut.staffed {
            skips.push(Skip::NoCitizen { at: hut.at });
            continue;
        }
        // `hasWorkOrder()` → `continue` (`:449-452`).
        if hut.busy {
            skips.push(Skip::Busy { at: hut.at });
            continue;
        }
        // `if (order.isClaimed())` (`:454-461`).
        if let Some(by) = order.claimed_by() {
            if by == hut.at {
                return Decision::Take { by: hut.at, held: true };
            }
            skips.push(Skip::HeldElsewhere { at: hut.at, by });
            continue;
        }
        // `getSetting(MODE).getValue().equals(MANUAL_SETTING)` → `continue` (`:465-469`).
        if hut.manual {
            skips.push(Skip::Manual { at: hut.at });
            continue;
        }
        // `predicate.test(building)` (`:471-476`), `order::canBuild` at the call site
        // (`:428`).
        if can_build(order, hut) {
            return Decision::Take { by: hut.at, held: false };
        }
        skips.push(Skip::CannotBuild { at: hut.at });
    }
    Decision::Nobody { skips }
}

/// The colony's box, chunk by chunk, as `isWorkOrderWithinColony` measures it
/// (`:343-379`).
///
/// **Two details of the Java are kept as they are, because they change the answer.**
///
/// - The minimum corner is taken **plus one** (`:355`, `:358`), so a box that ends
///   exactly on a chunk boundary never names the chunk beyond it. A box from `x = 0`
///   to `x = 15` is judged on chunk `0` only.
/// - A box that covers nothing — `min == max`, which one block does — walks no chunk
///   at all and is answered `true` (`:366-382` never enters its body).
pub fn within_colony(
    bounds: &Footprint,
    colony: i32,
    dimension: &str,
    book: &ClaimBook,
) -> bool {
    let (low, high) = (bounds.low(), bounds.high());
    // `Math.min(a.getX(), b.getX()) + 1` (`:355`).
    let min_x = low[0].min(high[0]) + 1;
    let max_x = low[0].max(high[0]);
    let min_z = low[2].min(high[2]) + 1;
    let max_z = low[2].max(high[2]);
    let mut seen: BTreeSet<[i32; 2]> = BTreeSet::new();
    let mut x = min_x;
    while x < max_x {
        let mut z = min_z;
        while z < max_z {
            // `final int chunkX = x >> 4` (`:365`).
            let chunk = [x >> 4, z >> 4];
            if seen.insert(chunk) && book.owner(dimension, chunk) != Some(colony) {
                // `if (ColonyUtils.getOwningColony(...) != colony.getID()) return false;`
                // (`:371-374`).
                return false;
            }
            z += CHUNK_SIZE;
        }
        x += CHUNK_SIZE;
    }
    true
}

/// Adds a fresh order — Java's `addWorkOrder(order, false)` (`:286-331`), with the
/// refusal moved to the front.
///
/// Two rules: an order at a position that already has one **replaces** it through
/// [`remove`] (`:292-299`; Java also compares the structure path and pack, which this
/// port's record does not carry — `colony/ordre.rs:113-115`), and an id-less order
/// takes the counter's next id (`:308-312`, `Colony::place_order`).
///
/// The refusal comes first here, and it is a divergence: Java has already deleted the
/// incumbent when it refuses (module doc, difference 1).
pub fn add(
    colony: &mut Colony,
    building: &str,
    at: [i32; 3],
    work: WorkType,
    target_level: i32,
    site: &Site<'_>,
    journal: &mut impl Sink,
) -> Result<i32, AddRefusal> {
    // `if (!readingFromNbt && !isWorkOrderWithinColony(order))` (`:301-305`).
    if let Some(bounds) = site.bounds {
        if !within_colony(bounds, colony.id, &colony.dimension, site.book) {
            return Err(AddRefusal::OutOfColony { at });
        }
    }
    // `for (IServerWorkOrder or : workOrders.values()) … removeWorkOrder(or)` (`:292-299`).
    let incumbent = colony
        .orders
        .iter()
        .find(|order| order.at == at)
        .map(|order| order.id);
    if let Some(incumbent) = incumbent {
        remove(colony, incumbent, journal).map_err(AddRefusal::Incumbent)?;
    }
    // `if (order.getID() == 0) { topWorkOrderId++; order.setID(topWorkOrderId); }`
    // (`:308-312`).
    let id = colony.place_order(building, at, work, target_level);
    journal.write(Write::Placed { id });
    journal.write(Write::ManagerDirty);
    Ok(id)
}

/// Takes an order out of the colony, with the six callbacks Java makes on the way
/// (`:95-118`).
///
/// The state change — `complete` or `cancel` — is **not** here: it is
/// `Colony::finish_order` / `Colony::cancel_order` (`colony/colonie.rs:246-273`), which
/// remove the order themselves. This function is the half Java writes around the
/// removal, and a caller that has already removed the order gets
/// [`OrderRefusal::Unknown`] — which is why `add` uses it for the incumbent.
pub fn remove(
    colony: &mut Colony,
    id: i32,
    journal: &mut impl Sink,
) -> Result<(), OrderRefusal> {
    let Some(index) = colony.orders.iter().position(|order| order.id == id) else {
        // Java returns quietly when the id is unknown (`:98`); the port names it.
        return Err(OrderRefusal::Unknown { id });
    };
    // `if (workOrder.isClaimed()) { … onWorkOrderCancellation(workOrder) }` (`:100-107`).
    if let Some(by) = colony.orders[index].claimed_by() {
        let holds_it = colony
            .building_at(by)
            .and_then(|record| Kind::from_path(&record.kind))
            .is_some_and(|kind| matches!(kind, Kind::Builder | Kind::Miner));
        if holds_it {
            journal.write(Write::BuilderTold { id, by });
        }
    }
    // `dirty = true; workOrders.remove(orderId);` (`:109-110`).
    journal.write(Write::ManagerDirty);
    colony.orders.remove(index);
    journal.write(Write::Removed { id });
    // `colony.removeWorkOrderInView(orderId);` (`:111`).
    journal.write(Write::ViewTold { id });
    // `builderWorkOrder.onRemoved(colony);` (`:112-115`).
    journal.write(Write::OrderNotified { id });
    // `colony.markDirty();` (`:116`).
    journal.write(Write::ColonyDirty);
    Ok(())
}

/// A citizen leaves its hut: every order that hut holds goes back to the pool
/// (`:205-218`).
///
/// Java sets `dirty` **before** it looks at `citizen.getWorkBuilding()`
/// (`:207`), so a citizen with no hut still costs the colony a save. The port writes
/// [`Write::ManagerDirty`] only when a claim came off.
pub fn clear_work_for(colony: &mut Colony, hut: [i32; 3], journal: &mut impl Sink) -> usize {
    let mut released = 0;
    for order in &mut colony.orders {
        if order.claimed_by() != Some(hut) {
            continue;
        }
        if order.release().is_ok() {
            journal.write(Write::Released { id: order.id });
            released += 1;
        }
    }
    if released > 0 {
        journal.write(Write::ManagerDirty);
    }
    released
}

/// One colony tick (`:386-430`): drop what is invalid, let go of what is orphaned,
/// then offer what is left, highest priority first.
///
/// The sort is Java's (`:420-423`): **claimed orders first**, then
/// `WORK_ORDER_COMPARATOR` — higher priority, then the lower id. Claimed first is not
/// a mistake: a claimed order is re-offered to the hut that holds it, which is how
/// Java re-establishes a builder's own order after a reload.
pub fn tick(colony: &mut Colony, huts: &[Hut], journal: &mut impl Sink) -> Tick {
    let mut summary = Tick::default();

    // Pass 1: `if (!order.isValid(colony)) { iter.remove(); dirty = true; }` (`:397-400`).
    let mut invalid: Vec<i32> = Vec::new();
    for order in &colony.orders {
        if !valid(colony, order) {
            invalid.push(order.id);
        }
    }
    for id in invalid {
        colony.orders.retain(|order| order.id != id);
        journal.write(Write::Dropped { id });
        journal.write(Write::ManagerDirty);
        summary.dropped += 1;
    }

    // `if (order.isClaimed() && getBuilding(order.getClaimedBy()) == null)
    //  order.setClaimedBy(BlockPos.ZERO);` (`:410-413`). The rule is
    // `Colony::release_orphan_claims`; the count is this pass's own reading of its
    // result, so the journal can say how many claims went.
    let before = colony.orders.iter().filter(|order| order.claimed_by().is_some()).count();
    colony.release_orphan_claims();
    let after = colony.orders.iter().filter(|order| order.claimed_by().is_some()).count();
    if before > after {
        journal.write(Write::ClaimsReleased { count: before - after });
        summary.released = before - after;
    }

    // `orderedWorkOrders.sort(Comparator.comparing(isClaimed).reversed()
    //  .thenComparing(WORK_ORDER_COMPARATOR))` (`:420-423`).
    let ids: Vec<i32> = {
        let mut ordered: Vec<&WorkOrder> = colony.orders.iter().collect();
        ordered.sort_by(|left, right| {
            right
                .claimed_by()
                .is_some()
                .cmp(&left.claimed_by().is_some())
                .then_with(|| ordre::compare(*left, *right))
        });
        ordered.iter().map(|order| order.id).collect()
    };

    // `for (IServerWorkOrder order : orderedWorkOrders) tryAssignWorkOrder(order, order::canBuild);`
    // (`:426-429`).
    for id in ids {
        let Some(index) = colony.orders.iter().position(|order| order.id == id) else {
            continue;
        };
        match decide(&colony.orders[index], huts) {
            Decision::Take { by, held } => {
                // The claim is the order's own state machine (`colony/ordre.rs:150-160`);
                // `decide` has already checked that it cannot refuse here.
                if colony.orders[index].claim(by).is_ok() {
                    journal.write(if held {
                        Write::Reaffirmed { id, by }
                    } else {
                        Write::Claimed { id, by }
                    });
                    if held {
                        summary.reaffirmed += 1;
                    } else {
                        summary.taken += 1;
                    }
                }
            }
            Decision::Nobody { skips } => summary.unclaimed.push((id, skips)),
        }
    }
    summary
}

/// What one builder is offered, in the order it takes them (`:506-514`): everything
/// unclaimed or claimed by this hut, kept by `keep`, sorted by priority.
pub fn ordered_for<'a>(
    colony: &'a Colony,
    builder: [i32; 3],
    keep: impl Fn(&WorkOrder) -> bool,
) -> Vec<&'a WorkOrder> {
    let mut list: Vec<&WorkOrder> = colony
        .orders
        .iter()
        .filter(|order| order.claimed_by().map_or(true, |by| by == builder))
        .filter(|order| keep(*order))
        .collect();
    list.sort_by(|left, right| ordre::compare(*left, *right));
    list
}

/// The first unclaimed order of one kind (`:162-177`).
///
/// Java keys this on the order's **class** (`WorkOrderBuilding`, `WorkOrderDecoration`,
/// `WorkOrderMiner`); the port keys it on [`WorkType`], which is Java's second axis
/// (`AbstractWorkOrder.java:719`). The two partitions are not the same, and
/// `MANQUANT.md` §3 says which caller would notice.
pub fn first_unclaimed(colony: &Colony, work: WorkType) -> Option<&WorkOrder> {
    colony
        .orders
        .iter()
        .find(|order| order.claimed_by().is_none() && order.work == work)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::Building as HutRecord;
    use crate::colony::ordre::OrderState;
    use crate::colony::Building;
    use crate::manager::ClaimBook;

    const HUT: [i32; 3] = [10, 64, 10];
    const OTHER: [i32; 3] = [20, 64, 10];
    const FAR: [i32; 3] = [400, 64, 400];
    const KIND: &str = "minecolonies:builder";

    fn colony() -> Colony {
        let mut colony = Colony::new(7, "Bridgewater", "minecraft:overworld", [0, 70, 0], 1);
        colony.buildings.push(Building::new(KIND, HUT, 1));
        colony.buildings.push(Building::new(KIND, OTHER, 1));
        colony
    }

    fn order(id: i32, at: [i32; 3]) -> WorkOrder {
        WorkOrder::new(id, KIND, at, WorkType::Build, 1)
    }

    /// The colony's first order, claimed by `by`.
    fn place_and_claim(colony: &mut Colony, at: [i32; 3], by: [i32; 3]) -> i32 {
        let id = colony.place_order(KIND, at, WorkType::Build, 1);
        let index = colony.orders.iter().position(|order| order.id == id).expect("just placed");
        colony.orders[index].claim(by).expect("an open order");
        id
    }

    #[test]
    fn a_builder_reaches_a_level_above_its_own_only_at_the_top_or_at_its_own_hut() {
        let mut target = order(1, HUT);
        target.target_level = 3;
        assert!(!can_build_ignoring_distance(&target, &Hut { level: 1, ..Hut::builder() }));
        assert!(can_build_ignoring_distance(&target, &Hut { level: 3, ..Hut::builder() }));
        assert!(can_build_ignoring_distance(&target, &Hut { level: 5, ..Hut::builder() }));
        assert!(
            can_build_ignoring_distance(&target, &Hut { level: 1, at: HUT, ..Hut::builder() }),
            "the order is for its own hut: WorkOrderBuilding.java:181"
        );
    }

    #[test]
    fn a_hundred_blocks_is_in_reach_and_one_hundred_and_one_is_not() {
        let target = order(1, [0, 64, 0]);
        assert!(can_build(&target, &Hut { at: [100, 64, 0], level: 1, ..Hut::builder() }));
        assert!(!can_build(&target, &Hut { at: [101, 64, 0], level: 1, ..Hut::builder() }));
        assert!(
            !can_build(&target, &Hut { at: [60, 154, 0], level: 1, ..Hut::builder() }),
            "60 blocks away on x but 90 up: sqrt(3600 + 8100) is 108, and BlockPos.distSqr counts y"
        );
        assert!(
            !can_build(&target, &Hut { at: [0, 64, 0], level: 1, builder: false, ..Hut::builder() }),
            "a hut that is not a structure builder builds nothing"
        );
    }

    #[test]
    fn a_hut_takes_an_order_only_if_staffed_idle_automatic_and_able() {
        let target = order(1, [0, 64, 0]);
        let cases = [
            (Hut { staffed: false, ..Hut::builder() }, Skip::NoCitizen { at: [0, 0, 0] }),
            (Hut { busy: true, ..Hut::builder() }, Skip::Busy { at: [0, 0, 0] }),
            (Hut { manual: true, ..Hut::builder() }, Skip::Manual { at: [0, 0, 0] }),
            (Hut { builder: false, ..Hut::builder() }, Skip::NotABuilder { at: [0, 0, 0] }),
            (
                Hut { at: [900, 64, 900], ..Hut::builder() },
                Skip::CannotBuild { at: [900, 64, 900] },
            ),
        ];
        for (hut, expected) in cases {
            assert_eq!(
                decide(&target, &[hut]),
                Decision::Nobody { skips: vec![expected] },
                "{expected:?}"
            );
        }
        assert_eq!(
            decide(&target, &[Hut::builder()]),
            Decision::Take { by: [0, 0, 0], held: false }
        );
    }

    #[test]
    fn a_manual_hut_may_still_finish_the_order_it_holds() {
        let mut target = order(1, [0, 0, 0]);
        target.claim(HUT).expect("an open order");
        assert_eq!(
            decide(&target, &[Hut { at: HUT, manual: true, ..Hut::builder() }]),
            Decision::Take { by: HUT, held: true },
            "the claim is checked before the mode: WorkManager.java:454-469"
        );
        assert_eq!(
            decide(&target, &[Hut { at: OTHER, ..Hut::builder() }]),
            Decision::Nobody { skips: vec![Skip::HeldElsewhere { at: OTHER, by: HUT }] }
        );
    }

    #[test]
    fn the_first_hut_that_answers_yes_takes_it_and_the_rest_are_not_asked() {
        let target = order(1, [0, 0, 0]);
        let huts = [
            Hut { at: HUT, busy: true, ..Hut::builder() },
            Hut { at: OTHER, ..Hut::builder() },
        ];
        assert_eq!(decide(&target, &huts), Decision::Take { by: OTHER, held: false });
        assert_eq!(decide(&target, &[]), Decision::Nobody { skips: Vec::new() });
    }

    #[test]
    fn an_order_whose_building_is_gone_is_invalid_and_the_tick_drops_it() {
        let mut colony = colony();
        let standing = colony.place_order(KIND, HUT, WorkType::Build, 1);
        let orphan = colony.place_order(KIND, [300, 64, 300], WorkType::Build, 1);
        let mut journal: Vec<Write> = Vec::new();
        let summary = tick(&mut colony, &[], &mut journal);
        assert_eq!(summary.dropped, 1, "only the one without a building");
        assert_eq!(colony.order(orphan), None);
        assert!(colony.order(standing).is_some());
        assert_eq!(
            journal,
            vec![Write::Dropped { id: orphan }, Write::ManagerDirty],
            "iter.remove() then dirty = true, WorkManager.java:399-400"
        );
    }

    #[test]
    fn the_tick_offers_claimed_orders_first_then_by_priority_then_by_id() {
        let mut colony = colony();
        // Two orders at the same (other) hut would collide, so they stand apart.
        let low = colony.place_order(KIND, HUT, WorkType::Build, 1);
        let high = colony.place_order(KIND, OTHER, WorkType::Build, 1);
        let index = colony.orders.iter().position(|order| order.id == high).expect("placed");
        colony.orders[index].priority = 5;
        let held = place_and_claim(&mut colony, [60, 64, 60], [80, 64, 80]);
        colony.buildings.push(Building::new(KIND, [60, 64, 60], 1));
        colony.buildings.push(Building::new(KIND, [80, 64, 80], 1));

        let mut journal: Vec<Write> = Vec::new();
        let summary = tick(&mut colony, &[], &mut journal);
        // The claimed one is re-offered first and nobody answers, then the others in
        // priority order; every order nobody took is reported in that order.
        assert_eq!(
            summary.unclaimed.iter().map(|(id, _)| *id).collect::<Vec<i32>>(),
            vec![held, high, low],
            "claimed first, then priority 5, then the lower id"
        );
        assert!(journal.is_empty(), "an order nobody takes writes nothing");
    }

    #[test]
    fn the_tick_claims_with_the_hut_that_answers_and_counts_it() {
        let mut colony = colony();
        let id = colony.place_order(KIND, [200, 64, 200], WorkType::Build, 1);
        colony.buildings.push(Building::new(KIND, [200, 64, 200], 1));
        let hut = Hut { at: [210, 64, 200], ..Hut::builder() };
        let mut journal: Vec<Write> = Vec::new();
        let summary = tick(&mut colony, &[hut], &mut journal);
        assert_eq!(summary.taken, 1);
        assert_eq!(journal, vec![Write::Claimed { id, by: [210, 64, 200] }]);
        assert_eq!(colony.order(id).map(|order| order.state), Some(OrderState::Claimed { by: [210, 64, 200] }));
    }

    #[test]
    fn the_tick_reaffirms_a_claim_instead_of_claiming_it_again() {
        let mut colony = colony();
        let id = place_and_claim(&mut colony, [200, 64, 200], OTHER);
        colony.buildings.push(Building::new(KIND, [200, 64, 200], 1));
        let mut journal: Vec<Write> = Vec::new();
        let summary = tick(&mut colony, &[Hut { at: OTHER, ..Hut::builder() }], &mut journal);
        assert_eq!(summary.reaffirmed, 1);
        assert_eq!(summary.taken, 0);
        assert_eq!(journal, vec![Write::Reaffirmed { id, by: OTHER }], ":459");
    }

    #[test]
    fn a_claim_on_a_hut_that_no_longer_stands_is_released_by_the_tick() {
        let mut colony = colony();
        let id = place_and_claim(&mut colony, [200, 64, 200], FAR);
        colony.buildings.push(Building::new(KIND, [200, 64, 200], 1));
        let mut journal: Vec<Write> = Vec::new();
        let summary = tick(&mut colony, &[], &mut journal);
        assert_eq!(summary.released, 1);
        assert_eq!(journal, vec![Write::ClaimsReleased { count: 1 }], ":410-413");
        assert_eq!(colony.order(id).map(|order| order.state), Some(OrderState::Open));
    }

    #[test]
    fn a_removal_writes_its_six_callbacks_in_javas_order() {
        let mut colony = colony();
        let id = place_and_claim(&mut colony, [200, 64, 200], HUT);
        colony.buildings.push(Building::new(KIND, [200, 64, 200], 1));
        let mut journal: Vec<Write> = Vec::new();
        assert_eq!(remove(&mut colony, id, &mut journal), Ok(()));
        assert_eq!(
            journal,
            vec![
                Write::BuilderTold { id, by: HUT },
                Write::ManagerDirty,
                Write::Removed { id },
                Write::ViewTold { id },
                Write::OrderNotified { id },
                Write::ColonyDirty,
            ],
            "WorkManager.java:105, :109, :110, :111, :114, :116"
        );
        assert_eq!(colony.order(id), None);
    }

    #[test]
    fn a_removal_of_an_unknown_order_refuses_and_writes_nothing() {
        let mut colony = colony();
        let mut journal: Vec<Write> = Vec::new();
        assert_eq!(remove(&mut colony, 9, &mut journal), Err(OrderRefusal::Unknown { id: 9 }));
        assert!(journal.is_empty());
    }

    #[test]
    fn a_claim_held_by_a_hut_that_is_not_a_builder_is_not_told_about_the_removal() {
        let mut colony = colony();
        colony.buildings.push(Building::new("minecolonies:townhall", [200, 64, 200], 1));
        let id = place_and_claim(&mut colony, [300, 64, 300], [200, 64, 200]);
        let mut journal: Vec<Write> = Vec::new();
        assert_eq!(remove(&mut colony, id, &mut journal), Ok(()));
        assert!(
            !journal.iter().any(|write| matches!(write, Write::BuilderTold { .. })),
            "WorkManager.java:103 asks for AbstractBuildingStructureBuilder first"
        );
        assert_eq!(journal.first(), Some(&Write::ManagerDirty));
    }

    #[test]
    fn a_citizen_leaving_releases_only_the_orders_its_own_hut_held() {
        let mut colony = colony();
        let ours = place_and_claim(&mut colony, [200, 64, 200], HUT);
        let theirs = place_and_claim(&mut colony, [300, 64, 300], OTHER);
        colony.buildings.push(Building::new(KIND, [200, 64, 200], 1));
        colony.buildings.push(Building::new(KIND, [300, 64, 300], 1));
        let mut journal: Vec<Write> = Vec::new();
        assert_eq!(clear_work_for(&mut colony, HUT, &mut journal), 1);
        assert_eq!(journal, vec![Write::Released { id: ours }, Write::ManagerDirty]);
        assert_eq!(colony.order(ours).map(|order| order.state), Some(OrderState::Open));
        assert_eq!(colony.order(theirs).map(|order| order.state), Some(OrderState::Claimed { by: OTHER }));

        journal.clear();
        assert_eq!(clear_work_for(&mut colony, HUT, &mut journal), 0, "nothing left to let go");
        assert!(journal.is_empty(), "Java would have dirtied the colony anyway, :207");
    }

    #[test]
    fn adding_an_order_at_a_position_that_has_one_replaces_it_through_the_removal_path() {
        // Reuse the colony under test with a builder at the order's position so the
        // removal path has someone to tell.
        let mut colony = colony();
        let book = ClaimBook::new();
        let mut journal: Vec<Write> = Vec::new();
        let first = add(&mut colony, KIND, HUT, WorkType::Build, 1, &Site { bounds: None, book: &book }, &mut journal)
            .expect("no bounds, so no refusal");
        assert_eq!(first, 1, "the counter's first id");
        let index = colony.orders.iter().position(|order| order.id == first).expect("placed");
        colony.orders[index].claim(HUT).expect("its own hut");
        journal.clear();
        let second = add(&mut colony, KIND, HUT, WorkType::Upgrade, 2, &Site { bounds: None, book: &book }, &mut journal)
            .expect("replacing is not refused");
        assert_eq!(second, 2, "a new id, as WorkManager.java:308-312 hands out");
        assert_eq!(colony.orders.len(), 1, "one order where one hut stands");
        assert_eq!(colony.order(first), None);
        assert_eq!(
            journal,
            vec![
                Write::BuilderTold { id: first, by: HUT },
                Write::ManagerDirty,
                Write::Removed { id: first },
                Write::ViewTold { id: first },
                Write::OrderNotified { id: first },
                Write::ColonyDirty,
                Write::Placed { id: second },
                Write::ManagerDirty,
            ],
            "the incumbent is removed with its callbacks before the new one is placed"
        );
    }

    #[test]
    fn an_order_outside_the_colony_is_refused_before_anything_is_written() {
        let mut colony = colony();
        let mut book = ClaimBook::new();
        book.entry("minecraft:overworld", [0, 0]).add_static_claim(7, true);
        let inside = Footprint::between([1, 0, 1], [15, 0, 15]);
        let outside = Footprint::between([17, 0, 1], [31, 0, 15]);
        let mut journal: Vec<Write> = Vec::new();
        assert_eq!(
            add(&mut colony, KIND, [4, 64, 4], WorkType::Build, 1, &Site { bounds: Some(&inside), book: &book }, &mut journal),
            Ok(1)
        );
        journal.clear();
        assert_eq!(
            add(&mut colony, KIND, [20, 64, 4], WorkType::Build, 1, &Site { bounds: Some(&outside), book: &book }, &mut journal),
            Err(AddRefusal::OutOfColony { at: [20, 64, 4] })
        );
        assert!(journal.is_empty(), "and the order before it is untouched");
        assert!(colony.order(1).is_some(), "the incumbent survives a refusal here, not in Java");
        assert_eq!(AddRefusal::OutOfColony { at: HUT }.name(), "work.refusal.out_of_colony");
    }

    #[test]
    fn the_colony_box_is_walked_chunk_by_chunk_with_javas_plus_one() {
        let mut book = ClaimBook::new();
        book.entry("minecraft:overworld", [0, 0]).add_static_claim(7, true);
        // 0..15 lies in chunk 0, and chunk 0 is the colony's.
        assert!(within_colony(
            &Footprint::between([0, 0, 0], [15, 0, 15]),
            7,
            "minecraft:overworld",
            &book
        ));
        // 1..16 still samples only x = 2, so chunk 1 is never looked at: the walk
        // stops before 16 (:361, :363).
        assert!(within_colony(
            &Footprint::between([1, 0, 1], [16, 0, 16]),
            7,
            "minecraft:overworld",
            &book
        ));
        // 1..32 samples x = 2 then x = 18, which is chunk 1, and chunk 1 is nobody's.
        assert!(!within_colony(
            &Footprint::between([1, 0, 1], [32, 0, 32]),
            7,
            "minecraft:overworld",
            &book
        ));
        // A box that straddles the boundary and is two blocks wide is emptied by the
        // `+ 1` of :355: min == max, no chunk is sampled, and Java answers true.
        assert!(within_colony(
            &Footprint::between([15, 0, 15], [16, 0, 16]),
            7,
            "minecraft:overworld",
            &book
        ));
        // One block: no chunk at all, true by emptiness.
        assert!(within_colony(
            &Footprint::between([40, 0, 40], [40, 0, 40]),
            7,
            "minecraft:overworld",
            &book
        ));
        // The book is per dimension: the same chunk in the nether says nothing here.
        assert!(!within_colony(
            &Footprint::between([1, 0, 1], [15, 0, 15]),
            7,
            "minecraft:the_nether",
            &book
        ));
    }

    #[test]
    fn a_builder_is_offered_what_is_free_or_already_its_own_in_priority_order() {
        let mut colony = colony();
        let mine = place_and_claim(&mut colony, [200, 64, 200], HUT);
        colony.buildings.push(Building::new(KIND, [200, 64, 200], 1));
        let theirs = place_and_claim(&mut colony, [300, 64, 300], OTHER);
        colony.buildings.push(Building::new(KIND, [300, 64, 300], 1));
        let free = colony.place_order(KIND, HUT, WorkType::Repair, 1);
        let index = colony.orders.iter().position(|order| order.id == free).expect("placed");
        colony.orders[index].priority = 9;

        let offered: Vec<i32> = ordered_for(&colony, HUT, |_| true).iter().map(|order| order.id).collect();
        assert_eq!(offered, vec![free, mine], "the highest priority, and never another hut's");
        assert!(!offered.contains(&theirs));
        let only_repairs: Vec<i32> = ordered_for(&colony, HUT, |order| order.work == WorkType::Repair)
            .iter()
            .map(|order| order.id)
            .collect();
        assert_eq!(only_repairs, vec![free]);
    }

    #[test]
    fn the_first_unclaimed_order_of_a_kind_skips_what_is_taken() {
        let mut colony = colony();
        let first = colony.place_order(KIND, HUT, WorkType::Build, 1);
        let second = colony.place_order(KIND, OTHER, WorkType::Build, 1);
        assert_eq!(first_unclaimed(&colony, WorkType::Build).map(|order| order.id), Some(first));
        let index = colony.orders.iter().position(|order| order.id == first).expect("placed");
        colony.orders[index].claim(HUT).expect("its own hut");
        assert_eq!(first_unclaimed(&colony, WorkType::Build).map(|order| order.id), Some(second));
        assert_eq!(first_unclaimed(&colony, WorkType::Repair).map(|order| order.id), None);
    }

    #[test]
    fn a_hut_is_read_from_the_building_record_and_the_two_facts_it_lacks() {
        let mut record = HutRecord::new(Kind::Builder, HUT);
        record.complete(1).expect("a built hut");
        assert_eq!(
            Hut::of(&record, true, false),
            Hut { at: HUT, level: 1, builder: true, staffed: false, busy: true, manual: false },
            "no worker yet, and the caller says it is busy"
        );
        record.hire(3).expect("a citizen");
        assert!(Hut::of(&record, false, true).staffed);
        let town_hall = HutRecord::new(Kind::TownHall, OTHER);
        assert!(!Hut::of(&town_hall, false, false).builder, "a town hall is not a structure builder");
    }
}
