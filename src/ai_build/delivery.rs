//! The courier: six targets, one warehouse door, and four errands that each end
//! somewhere named.
//!
//! Java reference: `core/entity/ai/workers/service/EntityAIWorkDeliveryman.java` —
//! `registerTargets` (`:113-124`), `pickup` (`:145-207`), `pickupFromBuilding`
//! (`:215-262`), `cannotHoldMoreItems` (`:264-276`), `dump` (`:291-314`), `deliver`
//! (`:327-466`), `prepareDelivery` (`:468-552`) and `decide` (`:586-631`), plus
//! `JobDeliveryman.getMaxParallelDeliveries` (`:507-518`).
//!
//! ## Why this trade is worth a machine of its own
//!
//! Every other worker's AI is a loop that ends when there is nothing to do. The
//! courier's is a *four-way dispatch*: one decision state
//! (`START_WORKING`, `:118`) chooses between dumping, preparing a delivery, picking
//! up, and standing still, and the three errand states each come back to it with a
//! different answer. Two of the four are not exits at all but **detours**: a
//! delivery with a full backpack goes dumping first (`:615-621`), and a pickup that
//! cannot hold more goes dumping too (`:156-161`). A state table is the only honest
//! way to write that, and a test can then say which of the four was chosen and why.
//!
//! ## The four errands, in the order Java tests them
//!
//! | State | First question | Where it ends |
//! | --- | --- | --- |
//! | `PREPARE_DELIVERY` | is there still something to collect at the source? | `DELIVERY`, `DUMPING`, or itself |
//! | `DELIVERY` | does the target still exist and is it reachable? | `START_WORKING` or `DUMPING` |
//! | `PICKUP` | can the backpack hold more? | `START_WORKING` or `DUMPING` |
//! | `DUMPING` | is there a warehouse to empty into? | `START_WORKING` |
//!
//! **The one refusal that stops the whole trade.** `checkIfExecute` (`:638-661`) is
//! the `START_WORKING` target's condition: with no warehouse — or a warehouse whose
//! block entity is gone — the transition never runs and the citizen stays in
//! `START_WORKING` with a blocking message. That is [`Refusal::NoWarehouse`] and
//! [`Refusal::WarehouseHasNoChest`], and it is the only place this trade refuses
//! rather than choosing.
//!
//! **Two numbers.** The backpack limit is `2^(level − 1) + 1` stacks, and it is
//! **removed** at the hut's maximum level (`:269-276`) — a level-5 courier carries
//! anything. The parallel delivery limit is `1 + secondarySkillLevel / 5`
//! (`JobDeliveryman.java:511-518`), so a skilled courier collects several requests
//! that share a source and a destination in one trip.
//!
//! **What is not here.** Every walk, every item-handler call, the chest-full
//! interaction and the statistics: host work, kept as facts (`at_target`,
//! `at_warehouse`, `extracted`, `chest_refused_same_stack`, `still_gathering`). The
//! module decides where the citizen goes; it never moves it.

use crate::ai::{Action, Machine, Transition};

/// Where this file's rules come from, as one string a log line can print.
pub const SOURCE: &str = "EntityAIWorkDeliveryman.java:113-124,:145-207,:215-276,:291-314,:327-466,\
:468-552,:586-661 · JobDeliveryman.java:507-518";

/// `EntityAIWorkDeliveryman.PICKUP_DELAY` (`:72`): the pickup target's rate.
pub const PICKUP_DELAY: u32 = 5;

/// `DECISION_DELAY` (`:64-67`): `TICKS_SECOND * 5`, the decision target's rate.
pub const DECISION_DELAY: u32 = 20 * 5;

/// `PRIORITY_FORCING_DUMP` (`:80-82`): a completed request at or above this priority
/// sends the courier dumping instead of back to the decision state.
pub const PRIORITY_FORCING_DUMP: i32 = 10;

/// The six states of the courier's table.
///
/// Java reference: `AIWorkerState`'s `PREPARE_DELIVERY`, `DELIVERY`, `PICKUP`,
/// `DUMPING` and the two general states every worker has. The line numbers are the
/// Java enum's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: the six targets `:113-124` registers
pub enum DeliveryState {
    /// `IDLE` — one target, straight to work (`:117`).
    Idle,
    /// `START_WORKING` — the decision state (`:118`).
    StartWorking,
    /// `PREPARE_DELIVERY` — collect what the next delivery needs (`:119`).
    PrepareDelivery,
    /// `DELIVERY` — carry it to the hut that asked (`:120`).
    Delivery,
    /// `PICKUP` — empty a hut that asked to be emptied (`:121`).
    Pickup,
    /// `DUMPING` — empty the backpack into the warehouse (`:122`).
    Dumping,
}

impl DeliveryState {
    /// The six, in registration order.
    pub const ALL: [DeliveryState; 6] = [
        DeliveryState::Idle,
        DeliveryState::StartWorking,
        DeliveryState::PrepareDelivery,
        DeliveryState::Delivery,
        DeliveryState::Pickup,
        DeliveryState::Dumping,
    ];

    /// Java's identifier.
    pub fn java_name(self) -> &'static str {
        match self {
            DeliveryState::Idle => "IDLE",
            DeliveryState::StartWorking => "START_WORKING",
            DeliveryState::PrepareDelivery => "PREPARE_DELIVERY",
            DeliveryState::Delivery => "DELIVERY",
            DeliveryState::Pickup => "PICKUP",
            DeliveryState::Dumping => "DUMPING",
        }
    }
}

/// What the courier's job currently holds (`job.getCurrentTask()`, `:148`, `:334`,
/// `:594`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: `PickupRequest`, `DeliveryRequest`, and no task at all
pub enum Task {
    /// Nothing pending (`:595-596`).
    #[default]
    None,
    /// `PickupRequest` (`:150`, `:628-630`).
    Pickup,
    /// `DeliveryRequest` (`:336`, `:476`, `:615-626`).
    Delivery,
}

/// One request the courier could collect on the way (`:483-511`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Carry {
    /// `task.getRequest().getStack().getCount()`: how many the request wants.
    pub wanted: u32,
    /// `InventoryUtils.getItemCountInItemHandler(inv, matches(stack))`: how many the
    /// courier already holds of it.
    pub carried: u32,
}

/// Everything the courier's AI reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliveryFacts {
    /// `building.getBuildingLevel()` (`:275`).
    pub building_level: i32,
    /// `building.getMaxBuildingLevel()` (`:271`).
    pub max_building_level: i32,
    /// The skill `getMaxParallelDeliveries` reads (`JobDeliveryman.java:517`).
    pub secondary_level: i32,
    /// `job.findWareHouse() != null` (`:324`, `:640`).
    pub warehouse: bool,
    /// `wareHouse.getTileEntity() != null` (`:644`).
    pub warehouse_has_chest: bool,
    /// `walkToBuilding(warehouse)` (`:304`, `:598`).
    pub at_warehouse: bool,
    /// `job.getCurrentTask()` (`:594`).
    pub task: Task,
    /// `worker.getInventoryCitizen().isEmpty()` (`:605`, `:618`).
    pub inventory_empty: bool,
    /// `InventoryUtils.openSlotCount(...)` (`:197`).
    pub open_slots: usize,
    /// `InventoryUtils.getAmountOfStacksInItemHandler(...)` (`:275`).
    pub stacks: u32,
    /// `currentTask.getRequest().getPriority()` (`:188`).
    pub request_priority: i32,
    /// The target hut still exists (`:166-171`, `:352-357`).
    pub target_exists: bool,
    /// `walkToBuilding(targetTransport)` (`:173`, `:359`), or `walkToSafePos(source)`
    /// (`:526`) while preparing.
    pub at_target: bool,
    /// `!location.isReachableFromLocation(worker.getLocation())` (`:346`, `:520`).
    pub unreachable: bool,
    /// `getInventory().isFull()` (`:531`).
    pub inventory_full: bool,
    /// `pickupFromBuilding` answered true (`:178-196`): the hut is emptied.
    pub gathered: bool,
    /// `gatherIfInTileEntity(...)` answered true (`:538-541`): one stack came out and
    /// there may be more.
    pub still_gathering: bool,
    /// At least one matching stack left the backpack (`extracted`, `:366`, `:390`).
    pub extracted: bool,
    /// The target refused the stack and gave it back unchanged
    /// (`ItemStack.matches(insertionResultStack, stack)`, `:413-417`).
    pub chest_refused_same_stack: bool,
    /// `job.getTaskListWithSameDestination(...)` (`:483`, `:369`).
    pub tasks: Vec<Carry>,
}

impl Default for DeliveryFacts {
    /// A level-1 courier with a warehouse, no task, an empty backpack and no request.
    fn default() -> DeliveryFacts {
        DeliveryFacts {
            building_level: 1,
            max_building_level: 5,
            secondary_level: 0,
            warehouse: true,
            warehouse_has_chest: true,
            at_warehouse: true,
            task: Task::None,
            inventory_empty: true,
            open_slots: 27,
            stacks: 0,
            request_priority: 0,
            target_exists: true,
            at_target: true,
            unreachable: false,
            inventory_full: false,
            gathered: false,
            still_gathering: false,
            extracted: false,
            chest_refused_same_stack: false,
            tasks: Vec::new(),
        }
    }
}

/// Why the courier's trade cannot start at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two ways `checkIfExecute` answers false
pub enum Refusal {
    /// `getAndCheckWareHouse() == null` (`:640-661`): no warehouse in the colony.
    NoWarehouse,
    /// `wareHouse.getTileEntity() == null` (`:644-647`): the building is there, its
    /// chest is not.
    WarehouseHasNoChest,
}

impl core::fmt::Display for Refusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Refusal::NoWarehouse => f.write_str("no warehouse in the colony"),
            Refusal::WarehouseHasNoChest => f.write_str("the warehouse has no block entity"),
        }
    }
}

/// `checkIfExecute` (`:638-661`): the condition of the `START_WORKING` target.
///
/// Java also toggles `citizenData.setWorking(...)` and raises a blocking message
/// (`:643`, `:653-658`); both are effects, and the decision is this pair.
pub fn check_if_execute(facts: &DeliveryFacts) -> Result<(), Refusal> {
    if !facts.warehouse {
        return Err(Refusal::NoWarehouse); // `:653-660`
    }
    if !facts.warehouse_has_chest {
        return Err(Refusal::WarehouseHasNoChest); // `:644-647`
    }
    Ok(())
}

/// `cannotHoldMoreItems` (`:264-276`): `2^(level − 1) + 1` stacks, and no limit at
/// the hut's own maximum level.
pub fn cannot_hold_more_items(facts: &DeliveryFacts) -> bool {
    if facts.building_level >= facts.max_building_level {
        return false; // `:271-274`
    }
    // `Math.pow(2, getBuildingLevel() - 1.0D) + 1` (`:275`).
    let limit = 2f64.powi(facts.building_level.saturating_sub(1)) + 1.0;
    f64::from(facts.stacks) >= limit
}

/// `JobDeliveryman.getMaxParallelDeliveries` (`:511-518`): `1 + skill / 5`, and one
/// when nobody is assigned.
pub fn max_parallel_deliveries(secondary_level: i32) -> u32 {
    1 + (secondary_level.max(0) as u32) / 5
}

/// What `decide` chose (`:586-631`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one variant per `return` of `decide`
pub enum Decide {
    /// `START_WORKING`: either the walk to the warehouse is in flight (`:598-602`) or
    /// there is nothing to do and nothing to empty (`:605-613`).
    Stay,
    /// `DUMPING`, either before a delivery (`:618-621`) or after loitering with a full
    /// backpack (`:605-608`).
    Dump,
    /// `PREPARE_DELIVERY` (`:624`).
    PrepareDelivery,
    /// `PICKUP` (`:629`).
    Pickup,
}

/// `decide` (`:591-631`).
///
/// The order is Java's: no task first (walk to the warehouse, then maybe dump), then
/// a delivery (dump first, always), then a pickup.
pub fn decide(facts: &DeliveryFacts) -> Decide {
    if facts.task == Task::None {
        // `:597-613` — loiter at the warehouse.
        if !facts.at_warehouse {
            return Decide::Stay; // `:598-602`
        }
        if facts.inventory_empty {
            return Decide::Stay; // `:611`
        }
        return Decide::Dump; // `:607`
    }
    if facts.task == Task::Delivery {
        if !facts.inventory_empty {
            return Decide::Dump; // `:618-621`
        }
        return Decide::PrepareDelivery; // `:624`
    }
    Decide::Pickup // `:629`
}

/// Why `prepareDelivery` answered `DELIVERY` without collecting anything
/// (`:513-516`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two halves of `nextPickUp == null || parallel > max`
pub enum NoCollect {
    /// Every request on the way is already covered (`nextPickUp == null`).
    NothingToCollect,
    /// The courier is already at its parallel limit (`parallelDeliveryCount >
    /// job.getMaxParallelDeliveries()`), with the count it reached.
    OverParallel { parallel: u32 },
}

/// What one `PREPARE_DELIVERY` tick decided (`:468-552`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one variant per `return` of `prepareDelivery`
pub enum Prepare {
    /// The task changed under it (`:476-481`) → `START_WORKING`.
    WrongTask,
    /// Nothing left to collect (`:513-516`) → `DELIVERY`.
    Deliver(NoCollect),
    /// The source is in another dimension or unloaded (`:518-524`) → `START_WORKING`,
    /// with the request finished as failed.
    Unreachable,
    /// Still walking to the source (`:526-529`) → `PREPARE_DELIVERY`.
    Walking,
    /// `getInventory().isFull()` (`:531-534`) → `DUMPING`.
    Dump,
    /// One stack came out of the source (`:537-541`) → `PREPARE_DELIVERY`, because
    /// there may be more.
    StillGathering,
    /// Several requests share this source and one is covered (`:543-547`) →
    /// `DELIVERY`.
    EnoughParallel,
    /// Nothing came out and there is only one request (`:549-551`) →
    /// `START_WORKING`, request finished as failed.
    NothingThere,
}

/// `prepareDelivery` (`:468-552`).
pub fn prepare(facts: &DeliveryFacts) -> Prepare {
    if facts.task != Task::Delivery {
        return Prepare::WrongTask; // `:476-481`
    }
    let max = max_parallel_deliveries(facts.secondary_level);
    match next_pickup(&facts.tasks, max) {
        Ok(_) => {}
        Err(no_collect) => return Prepare::Deliver(no_collect), // `:513-516`
    }
    if facts.unreachable {
        return Prepare::Unreachable; // `:518-524`
    }
    if !facts.at_target {
        return Prepare::Walking; // `:526-529`
    }
    if facts.inventory_full {
        return Prepare::Dump; // `:531-534`
    }
    if facts.still_gathering {
        return Prepare::StillGathering; // `:537-541`
    }
    if facts.tasks.len() > 1 {
        return Prepare::EnoughParallel; // `:543-547`
    }
    Prepare::NothingThere // `:549-551`
}

/// The loop of `:483-511`, as an answer: which request can still be collected, and
/// how many the courier has counted so far.
///
/// Java walks the same-destination list, keeps a running total of what the earlier
/// requests already account for, and stops at the first request the backpack cannot
/// cover (`totalCount < hasCount + count`). `Err` means "go to `DELIVERY`".
pub fn next_pickup(tasks: &[Carry], max_parallel: u32) -> Result<usize, NoCollect> {
    let mut already = 0u32; // `hasCount`, summed over `alreadyInInv`
    for (index, task) in tasks.iter().enumerate() {
        let parallel = index as u32 + 1; // `parallelDeliveryCount++` (`:489`)
        if task.carried < already.saturating_add(task.wanted) {
            if parallel > max_parallel {
                return Err(NoCollect::OverParallel { parallel }); // `:513`
            }
            return Ok(index); // `nextPickUp = task; break`
        }
        already = already.saturating_add(task.wanted); // `alreadyInInv.add(...)`
    }
    Err(NoCollect::NothingToCollect) // `:513`
}

/// What one `DELIVERY` tick decided (`:327-466`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one variant per `return` of `deliver`
pub enum Delivery {
    /// The task changed under it (`:334-341`) → `DUMPING`: the backpack is already
    /// loaded for a delivery that no longer exists.
    WrongTask,
    /// The target is in another dimension (`:345-350`) → `START_WORKING`.
    Unreachable,
    /// The target hut is gone (`:352-357`) → `START_WORKING`, request finished.
    TargetGone,
    /// Still walking (`:359-363`) → `DELIVERY`.
    Walking,
    /// The backpack held nothing the request wanted (`:449-459`) → `START_WORKING`.
    NothingToDeliver,
    /// The target's chest was full of the same stack, so the item came back
    /// (`:409-441`) → `DUMPING` (`:465`, `success == false`).
    ChestFull,
    /// Everything went in (`:461-465`) → `START_WORKING`.
    Delivered,
}

/// `deliver` (`:327-466`).
pub fn deliver(facts: &DeliveryFacts) -> Delivery {
    if facts.task != Task::Delivery {
        return Delivery::WrongTask; // `:334-341`
    }
    if facts.unreachable {
        return Delivery::Unreachable; // `:345-350`
    }
    if !facts.target_exists {
        return Delivery::TargetGone; // `:352-357`
    }
    if !facts.at_target {
        return Delivery::Walking; // `:359-363`
    }
    if !facts.extracted {
        return Delivery::NothingToDeliver; // `:449-459`
    }
    if facts.chest_refused_same_stack {
        return Delivery::ChestFull; // `:413-441`, then `:465`
    }
    Delivery::Delivered // `:464-465`
}

/// What one `PICKUP` tick decided (`:145-207`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one variant per `return` of `pickup`
pub enum Pickup {
    /// The task changed under it (`:150-154`) → `START_WORKING`.
    WrongTask,
    /// The backpack is full for this hut's level (`:156-161`) → `DUMPING`.
    ForcedDump,
    /// The hut to empty is gone (`:166-171`) → `START_WORKING`, request finished.
    BuildingGone,
    /// Still walking (`:173-176`) → `PICKUP`.
    Walking,
    /// The hut is emptied and the request was important (`:188-191`) → `DUMPING`.
    PickedUpForcedDump,
    /// The hut is emptied (`:194`) → `START_WORKING`.
    PickedUp,
    /// One slot at a time: another turn (`:204-206`).
    More,
}

/// `pickup` (`:145-207`).
pub fn pickup(facts: &DeliveryFacts) -> Pickup {
    if facts.task != Task::Pickup {
        return Pickup::WrongTask; // `:150-154`
    }
    if cannot_hold_more_items(facts) {
        return Pickup::ForcedDump; // `:156-161`
    }
    if !facts.target_exists {
        return Pickup::BuildingGone; // `:166-171`
    }
    if !facts.at_target {
        return Pickup::Walking; // `:173-176`
    }
    if facts.gathered {
        // `:178-196` — the hut is empty; the priority decides where to go.
        if facts.request_priority >= PRIORITY_FORCING_DUMP {
            return Pickup::PickedUpForcedDump; // `:188-191`
        }
        return Pickup::PickedUp; // `:194`
    }
    if facts.open_slots == 0 {
        return Pickup::ForcedDump; // `:197-202`
    }
    Pickup::More // `:204-206`
}

/// What one `DUMPING` tick decided (`:291-314`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one variant per `return` of `dump`
pub enum Dump {
    /// No warehouse to empty into (`:298-302`) → `START_WORKING`; the next decision
    /// will fail its own door and refuse with [`Refusal::NoWarehouse`].
    NoWarehouse,
    /// Still walking (`:304-308`) → `DUMPING`.
    Walking,
    /// Emptied (`:310-313`) → `START_WORKING`.
    Dumped,
}

/// `dump` (`:291-314`).
pub fn dump(facts: &DeliveryFacts) -> Dump {
    if !facts.warehouse {
        return Dump::NoWarehouse; // `:298-302`
    }
    if !facts.at_warehouse {
        return Dump::Walking; // `:304-308`
    }
    Dump::Dumped // `:310-313`
}

// ------------------------------------------------------------------- actions

/// `:117` — `new AITarget(IDLE, () -> START_WORKING, 1)`.
fn to_work(_: &mut DeliveryFacts, _: DeliveryState) -> Option<DeliveryState> {
    Some(DeliveryState::StartWorking)
}

/// `:638-661` — the door of the trade.
fn door(facts: &DeliveryFacts, _: DeliveryState) -> bool {
    check_if_execute(facts).is_ok()
}

/// `:118` — `new AITarget(START_WORKING, this::checkIfExecute, this::decide, DECISION_DELAY)`.
fn decide_action(facts: &mut DeliveryFacts, state: DeliveryState) -> Option<DeliveryState> {
    match decide(facts) {
        // Java returns `START_WORKING`: the same state, the tick ends here.
        Decide::Stay => Some(state),
        Decide::Dump => Some(DeliveryState::Dumping),
        Decide::PrepareDelivery => Some(DeliveryState::PrepareDelivery),
        Decide::Pickup => Some(DeliveryState::Pickup),
    }
}

/// `:119` — `new AITarget(PREPARE_DELIVERY, this::prepareDelivery, STANDARD_DELAY)`.
fn prepare_action(facts: &mut DeliveryFacts, _: DeliveryState) -> Option<DeliveryState> {
    match prepare(facts) {
        Prepare::WrongTask | Prepare::Unreachable | Prepare::NothingThere => {
            Some(DeliveryState::StartWorking)
        }
        Prepare::Deliver(_) | Prepare::EnoughParallel => Some(DeliveryState::Delivery),
        Prepare::Walking | Prepare::StillGathering => Some(DeliveryState::PrepareDelivery),
        Prepare::Dump => Some(DeliveryState::Dumping),
    }
}

/// `:120` — `new AITarget(DELIVERY, this::deliver, STANDARD_DELAY)`.
fn deliver_action(facts: &mut DeliveryFacts, _: DeliveryState) -> Option<DeliveryState> {
    match deliver(facts) {
        Delivery::WrongTask | Delivery::ChestFull => Some(DeliveryState::Dumping),
        Delivery::Unreachable
        | Delivery::TargetGone
        | Delivery::NothingToDeliver
        | Delivery::Delivered => Some(DeliveryState::StartWorking),
        Delivery::Walking => Some(DeliveryState::Delivery),
    }
}

/// `:121` — `new AITarget(PICKUP, this::pickup, PICKUP_DELAY)`.
fn pickup_action(facts: &mut DeliveryFacts, _: DeliveryState) -> Option<DeliveryState> {
    match pickup(facts) {
        Pickup::WrongTask => Some(DeliveryState::StartWorking),
        Pickup::ForcedDump | Pickup::PickedUpForcedDump => Some(DeliveryState::Dumping),
        Pickup::BuildingGone | Pickup::PickedUp => Some(DeliveryState::StartWorking),
        Pickup::Walking | Pickup::More => Some(DeliveryState::Pickup),
    }
}

/// `:122` — `new AITarget(DUMPING, this::dump, TICKS_SECOND)`.
fn dump_action(facts: &mut DeliveryFacts, _: DeliveryState) -> Option<DeliveryState> {
    match dump(facts) {
        Dump::NoWarehouse | Dump::Dumped => Some(DeliveryState::StartWorking),
        Dump::Walking => Some(DeliveryState::Dumping),
    }
}

/// Register the six targets of `EntityAIWorkDeliveryman.java:113-124`.
pub fn register(machine: &mut Machine<DeliveryState, DeliveryFacts>) {
    // :117 — `new AITarget(IDLE, () -> START_WORKING, 1)`.
    machine.add(Transition::target(
        DeliveryState::Idle,
        "idleToWork",
        "EntityAIWorkDeliveryman.java:117",
        1,
        Action::Run(to_work),
    ));
    // :118 — the decision, behind the warehouse door.
    machine.add(Transition::target_when(
        DeliveryState::StartWorking,
        "decide",
        "EntityAIWorkDeliveryman.java:118",
        DECISION_DELAY,
        door,
        Action::Run(decide_action),
    ));
    // :119 — `new AITarget(PREPARE_DELIVERY, this::prepareDelivery, STANDARD_DELAY)`.
    machine.add(Transition::target(
        DeliveryState::PrepareDelivery,
        "prepareDelivery",
        "EntityAIWorkDeliveryman.java:119",
        5,
        Action::Run(prepare_action),
    ));
    // :120 — `new AITarget(DELIVERY, this::deliver, STANDARD_DELAY)`.
    machine.add(Transition::target(
        DeliveryState::Delivery,
        "deliver",
        "EntityAIWorkDeliveryman.java:120",
        5,
        Action::Run(deliver_action),
    ));
    // :121 — `new AITarget(PICKUP, this::pickup, PICKUP_DELAY)`.
    machine.add(Transition::target(
        DeliveryState::Pickup,
        "pickup",
        "EntityAIWorkDeliveryman.java:121",
        PICKUP_DELAY,
        Action::Run(pickup_action),
    ));
    // :122 — `new AITarget(DUMPING, this::dump, TICKS_SECOND)`.
    machine.add(Transition::target(
        DeliveryState::Dumping,
        "dump",
        "EntityAIWorkDeliveryman.java:122",
        20,
        Action::Run(dump_action),
    ));
}

/// A machine with this table on it, starting in `IDLE`.
pub fn ai() -> Machine<DeliveryState, DeliveryFacts> {
    let mut machine = Machine::new(DeliveryState::Idle, crate::ai::ENTITY_AI_TICKRATE);
    register(&mut machine);
    machine
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `:638-661`: the two halves of the door, named apart.
    #[test]
    fn the_trade_does_not_start_without_a_warehouse() {
        assert_eq!(check_if_execute(&DeliveryFacts::default()), Ok(()));
        assert_eq!(
            check_if_execute(&DeliveryFacts {
                warehouse: false,
                ..DeliveryFacts::default()
            }),
            Err(Refusal::NoWarehouse)
        );
        assert_eq!(
            check_if_execute(&DeliveryFacts {
                warehouse_has_chest: false,
                ..DeliveryFacts::default()
            }),
            Err(Refusal::WarehouseHasNoChest)
        );
        assert!(Refusal::NoWarehouse.to_string().contains("warehouse"));
    }

    /// `:264-276`: `2^(level − 1) + 1` stacks, and no limit at the hut's maximum.
    #[test]
    fn the_backpack_limit_doubles_with_the_hut_and_disappears_at_maximum() {
        let level = |level: i32, stacks: u32| DeliveryFacts {
            building_level: level,
            stacks,
            ..DeliveryFacts::default()
        };
        // Level 1: 2^0 + 1 = 2 stacks.
        assert!(!cannot_hold_more_items(&level(1, 1)));
        assert!(cannot_hold_more_items(&level(1, 2)));
        // Level 2: 2^1 + 1 = 3.
        assert!(!cannot_hold_more_items(&level(2, 2)));
        assert!(cannot_hold_more_items(&level(2, 3)));
        // Level 3: 2^2 + 1 = 5.
        assert!(cannot_hold_more_items(&level(3, 5)));
        // At the maximum the limit is removed entirely (`:271-274`).
        assert!(!cannot_hold_more_items(&level(5, 400)));
    }

    /// `JobDeliveryman.java:511-518`: `1 + skill / 5`, floored at one.
    #[test]
    fn the_parallel_limit_grows_by_five_skill_levels() {
        assert_eq!(max_parallel_deliveries(0), 1);
        assert_eq!(max_parallel_deliveries(4), 1);
        assert_eq!(max_parallel_deliveries(5), 2);
        assert_eq!(max_parallel_deliveries(14), 3);
        assert_eq!(max_parallel_deliveries(-3), 1);
    }

    /// `:597-629`: no task loiters and may dump; a delivery dumps first; a pickup
    /// goes straight to the hut.
    #[test]
    fn the_decision_dispatches_the_four_errands() {
        assert_eq!(decide(&DeliveryFacts::default()), Decide::Stay);
        assert_eq!(
            decide(&DeliveryFacts {
                at_warehouse: false,
                inventory_empty: false,
                ..DeliveryFacts::default()
            }),
            Decide::Stay,
            "walking to the warehouse first (`:598-602`)"
        );
        assert_eq!(
            decide(&DeliveryFacts {
                inventory_empty: false,
                ..DeliveryFacts::default()
            }),
            Decide::Dump,
            "loitering with a full backpack (`:607`)"
        );
        assert_eq!(
            decide(&DeliveryFacts {
                task: Task::Delivery,
                inventory_empty: false,
                ..DeliveryFacts::default()
            }),
            Decide::Dump,
            "a delivery dumps before it collects (`:618-621`)"
        );
        assert_eq!(
            decide(&DeliveryFacts {
                task: Task::Delivery,
                ..DeliveryFacts::default()
            }),
            Decide::PrepareDelivery
        );
        assert_eq!(
            decide(&DeliveryFacts {
                task: Task::Pickup,
                ..DeliveryFacts::default()
            }),
            Decide::Pickup
        );
    }

    /// `:483-511`: the first request the backpack cannot cover is the one to collect;
    /// the ones before it are already accounted for.
    #[test]
    fn the_next_pickup_is_the_first_uncovered_request() {
        let tasks = vec![
            Carry {
                wanted: 4,
                carried: 0,
            },
            Carry {
                wanted: 4,
                carried: 0,
            },
        ];
        // Four carried cover the first request, not the second.
        let covered = vec![
            Carry {
                wanted: 4,
                carried: 4,
            },
            Carry {
                wanted: 4,
                carried: 0,
            },
        ];
        assert_eq!(next_pickup(&tasks, 4), Ok(0));
        assert_eq!(next_pickup(&covered, 4), Ok(1));
        // Everything covered: nothing to collect.
        let done = vec![Carry {
            wanted: 4,
            carried: 4,
        }];
        assert_eq!(next_pickup(&done, 4), Err(NoCollect::NothingToCollect));
        assert_eq!(next_pickup(&[], 4), Err(NoCollect::NothingToCollect));
    }

    /// `:513`: a courier at its parallel limit stops collecting, and says how far it
    /// got.
    #[test]
    fn the_parallel_limit_stops_the_collection() {
        let tasks = vec![
            Carry {
                wanted: 1,
                carried: 1,
            },
            Carry {
                wanted: 1,
                carried: 0,
            },
        ];
        // The second request needs a pickup at index 1 → parallel count 2.
        assert_eq!(next_pickup(&tasks, 2), Ok(1));
        assert_eq!(
            next_pickup(&tasks, 1),
            Err(NoCollect::OverParallel { parallel: 2 })
        );
    }

    /// `:468-552`: the six exits, in the order Java tests them.
    #[test]
    fn preparing_a_delivery_tests_the_task_then_the_source_then_the_space() {
        let delivery = DeliveryFacts {
            task: Task::Delivery,
            tasks: vec![Carry {
                wanted: 4,
                carried: 0,
            }],
            ..DeliveryFacts::default()
        };
        assert_eq!(prepare(&delivery), Prepare::NothingThere);
        assert_eq!(
            prepare(&DeliveryFacts {
                task: Task::Pickup,
                ..delivery.clone()
            }),
            Prepare::WrongTask
        );
        assert_eq!(
            prepare(&DeliveryFacts {
                unreachable: true,
                ..delivery.clone()
            }),
            Prepare::Unreachable
        );
        assert_eq!(
            prepare(&DeliveryFacts {
                at_target: false,
                ..delivery.clone()
            }),
            Prepare::Walking
        );
        assert_eq!(
            prepare(&DeliveryFacts {
                inventory_full: true,
                ..delivery.clone()
            }),
            Prepare::Dump
        );
        assert_eq!(
            prepare(&DeliveryFacts {
                still_gathering: true,
                ..delivery.clone()
            }),
            Prepare::StillGathering
        );
        // Two requests on the way, one covered, and a courier with room for more:
        // `:543-547` goes and delivers the one it has.
        assert_eq!(
            prepare(&DeliveryFacts {
                secondary_level: 20, // `max_parallel_deliveries(20) == 5`
                tasks: vec![
                    Carry {
                        wanted: 1,
                        carried: 1
                    },
                    Carry {
                        wanted: 1,
                        carried: 0
                    }
                ],
                ..delivery.clone()
            }),
            Prepare::EnoughParallel
        );
        // The same facts with a courier at its parallel limit stop at `:513-516`.
        assert_eq!(
            prepare(&DeliveryFacts {
                secondary_level: 0, // `max_parallel_deliveries(0) == 1`
                tasks: vec![
                    Carry {
                        wanted: 1,
                        carried: 1
                    },
                    Carry {
                        wanted: 1,
                        carried: 0
                    }
                ],
                ..delivery.clone()
            }),
            Prepare::Deliver(NoCollect::OverParallel { parallel: 2 })
        );
        // Nothing left to collect at all.
        assert_eq!(
            prepare(&DeliveryFacts {
                tasks: vec![Carry {
                    wanted: 1,
                    carried: 1
                }],
                ..delivery
            }),
            Prepare::Deliver(NoCollect::NothingToCollect)
        );
    }

    /// `:327-466`: the seven exits.
    #[test]
    fn delivering_names_what_went_wrong() {
        let carrying = DeliveryFacts {
            task: Task::Delivery,
            extracted: true,
            ..DeliveryFacts::default()
        };
        assert_eq!(deliver(&carrying), Delivery::Delivered);
        assert_eq!(
            deliver(&DeliveryFacts {
                task: Task::Pickup,
                ..carrying.clone()
            }),
            Delivery::WrongTask
        );
        assert_eq!(
            deliver(&DeliveryFacts {
                unreachable: true,
                ..carrying.clone()
            }),
            Delivery::Unreachable
        );
        assert_eq!(
            deliver(&DeliveryFacts {
                target_exists: false,
                ..carrying.clone()
            }),
            Delivery::TargetGone
        );
        assert_eq!(
            deliver(&DeliveryFacts {
                at_target: false,
                ..carrying.clone()
            }),
            Delivery::Walking
        );
        assert_eq!(
            deliver(&DeliveryFacts {
                extracted: false,
                ..carrying.clone()
            }),
            Delivery::NothingToDeliver
        );
        assert_eq!(
            deliver(&DeliveryFacts {
                chest_refused_same_stack: true,
                ..carrying
            }),
            Delivery::ChestFull
        );
    }

    /// `:145-207`: a priority of 10 or more forces a dump, and a full backpack does
    /// too.
    #[test]
    fn picking_up_fills_the_backpack_or_dumps() {
        let picking = DeliveryFacts {
            task: Task::Pickup,
            ..DeliveryFacts::default()
        };
        assert_eq!(pickup(&picking), Pickup::More);
        assert_eq!(
            pickup(&DeliveryFacts {
                task: Task::Delivery,
                ..picking.clone()
            }),
            Pickup::WrongTask
        );
        assert_eq!(
            pickup(&DeliveryFacts {
                stacks: 9,
                ..picking.clone()
            }),
            Pickup::ForcedDump,
            "level 1 holds two stacks (`:156-161`)"
        );
        assert_eq!(
            pickup(&DeliveryFacts {
                open_slots: 0,
                ..picking.clone()
            }),
            Pickup::ForcedDump
        );
        assert_eq!(
            pickup(&DeliveryFacts {
                gathered: true,
                request_priority: PRIORITY_FORCING_DUMP,
                ..picking.clone()
            }),
            Pickup::PickedUpForcedDump
        );
        assert_eq!(
            pickup(&DeliveryFacts {
                gathered: true,
                request_priority: PRIORITY_FORCING_DUMP - 1,
                ..picking.clone()
            }),
            Pickup::PickedUp
        );
        assert_eq!(
            pickup(&DeliveryFacts {
                target_exists: false,
                ..picking
            }),
            Pickup::BuildingGone
        );
    }

    /// `:291-314`: without a warehouse there is nothing to empty into, and walking
    /// there takes another turn.
    #[test]
    fn dumping_needs_a_warehouse_and_a_walk() {
        assert_eq!(dump(&DeliveryFacts::default()), Dump::Dumped);
        assert_eq!(
            dump(&DeliveryFacts {
                warehouse: false,
                ..DeliveryFacts::default()
            }),
            Dump::NoWarehouse
        );
        assert_eq!(
            dump(&DeliveryFacts {
                at_warehouse: false,
                ..DeliveryFacts::default()
            }),
            Dump::Walking
        );
    }

    /// The machine: `IDLE` goes to work on the first tick, and the decision state
    /// does nothing at all when the warehouse door refuses.
    #[test]
    fn the_machine_walks_idle_to_work_and_stops_at_the_door() {
        let mut machine = ai();
        let mut facts = DeliveryFacts::default();
        assert_eq!(machine.state(), DeliveryState::Idle);
        machine.tick(&mut facts).expect("IDLE has one target");
        assert_eq!(machine.state(), DeliveryState::StartWorking);

        // The door is open: the decision runs and chooses `IDLE`'s successor.
        let fired = machine.tick(&mut facts).expect("decide runs");
        assert_eq!(fired.rule, "decide");
        assert_eq!(machine.state(), DeliveryState::StartWorking);

        // The door is shut: the same state, no transition at all.
        let mut doorless = DeliveryFacts {
            warehouse: false,
            ..DeliveryFacts::default()
        };
        let mut stopped = ai();
        stopped.tick(&mut doorless).expect("IDLE still leaves");
        assert_eq!(stopped.state(), DeliveryState::StartWorking);
        assert!(stopped.tick(&mut doorless).is_none());
        assert_eq!(stopped.state(), DeliveryState::StartWorking);
        // And the refusal that names it is one call away.
        assert_eq!(check_if_execute(&doorless), Err(Refusal::NoWarehouse));
    }

    /// A full path through the machine: `IDLE` → `START_WORKING` → `PICKUP` →
    /// `DUMPING` → `START_WORKING`.
    #[test]
    fn a_pickup_that_fills_the_backpack_dumps_then_decides_again() {
        let mut machine = ai();
        let mut facts = DeliveryFacts {
            task: Task::Pickup,
            stacks: 2, // a level-1 courier holds two stacks
            at_target: false,
            ..DeliveryFacts::default()
        };
        machine.tick(&mut facts).expect("IDLE");
        machine.tick(&mut facts).expect("decide → PICKUP");
        assert_eq!(machine.state(), DeliveryState::Pickup);
        // `pickup` sees a full backpack (`:156-161`) and dumps.
        machine.tick(&mut facts).expect("pickup → DUMPING");
        assert_eq!(machine.state(), DeliveryState::Dumping);
        // The warehouse is right there, so the dump is immediate.
        facts.inventory_empty = true;
        machine.tick(&mut facts).expect("dump → START_WORKING");
        assert_eq!(machine.state(), DeliveryState::StartWorking);
        // And the decision runs again after its own delay: its countdown is
        // `DECISION_DELAY`, so the tick is a no-op until it is due.
        assert!(machine.tick(&mut facts).is_none());
    }

    /// `checkIfExecute` is a condition, so a courier without a warehouse keeps its
    /// state and is reset by nobody: Java's target simply never fires (`:638-661`).
    #[test]
    fn every_state_of_the_table_can_be_left() {
        let machine = ai();
        for state in DeliveryState::ALL {
            assert!(
                machine.can_leave(state),
                "{} has no transition",
                state.java_name()
            );
        }
        let mut names: Vec<&str> = DeliveryState::ALL.iter().map(|s| s.java_name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 6);
        assert!(SOURCE.contains("JobDeliveryman.java:507-518"));
    }
}
