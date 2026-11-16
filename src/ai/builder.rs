//! The builder: nine targets on top of the base table, the stage list of a build,
//! and the six outcomes one structure step can have.
//!
//! Java reference: `core/entity/ai/workers/AbstractEntityAIStructure.java` (the
//! structure AI: the nine targets at `:166-200`, `startWorkingAtOwnBuilding` at
//! `:207-214`, `pickUpMaterial` at `:221-268`, `completeBuild` at `:275-282`,
//! `startBuilding` at `:292-299`, `structureStep` at `:334-535`, `loadStructure` at
//! `:749-797`, `doMining` at `:704-734`, `isThereAStructureToBuild` at `:1055-1062`),
//! `AbstractEntityAIStructureWithWorkOrder.java` (`loadRequirements` at `:113-136`,
//! `requestMaterialsState` at `:203-224`, the `checkIfNeedsItem` override at
//! `:211-217`, `checkIfCanceled` at `:480-497`) and
//! `builder/EntityAIStructureBuilder.java` (`checkForWorkOrder` at `:96-123`,
//! `startWorkingAtOwnBuilding` at `:144-161`, `isAfterDumpPickupAllowed` at
//! `:137-141`, `afterRequestPickUp`/`afterDump` at `:195-205`,
//! `getActionsDoneUntilDumping` at `:289-293`, `canGoIdle` at `:335-339`).
//!
//! ## The table this adds
//!
//! | # | Pass / state | Rule | Rate | Java |
//! | ---: | --- | --- | ---: | --- |
//! | 1 | `STATE_BLOCKING` | `checkIfCanceled` → `IDLE` | 1 | `:170` |
//! | 2 | `LOAD_STRUCTURE` | `loadRequirements` | 5 | `:174` |
//! | 3 | `START_BUILDING` | `startBuilding` | 1 | `:178` |
//! | 4 | `MINE_BLOCK` | `doMining` | 10 | `:182` |
//! | 5 | `IDLE` | `IDLE → START_WORKING` | 10 | `:186` |
//! | 6 | `START_WORKING` | `startWorkingAtOwnBuilding` | 20 | `:190` |
//! | 7 | `BUILDING_STEP` | `structureStep` | 5 | `:194` |
//! | 8 | `COMPLETE_BUILD` | `completeBuild` | 5 | `:198` |
//! | 9 | `PICK_UP` | `pickUpMaterial` | 5 | `:199` |
//!
//! **Why the builder is the interesting trade.** Its loop is not "work until the work
//! is done", it is "one step, read what the step answered, follow it":
//! `structureStep` has six exits (`:461-535`) and four of them leave the state
//! altogether — `COMPLETE_BUILD` when the last stage ends (`:475`), `LOAD_STRUCTURE`
//! when the material list must be recomputed (`:512`), `NEEDS_ITEM` when the
//! blueprint wants something (`:514`), `MINE_BLOCK` when a block is in the way
//! (`:524`), plus `INVENTORY_FULL` when the worker has no room left (`:348`). That is
//! what [`step_outcome`] ports, and it is the part a caller cannot get from the stage
//! list alone.
//!
//! **Two numbers that look like bugs, copied as they are.** `setTotalStages` reports
//! 6 for a new build and 5 for an upgrade while the arrays hold 8 and 6 stages
//! (`AbstractEntityAIStructure.java:775-786`) — the hut's progress bar reads the
//! reported number ([`Stages`]). And `isAfterDumpPickupAllowed` is
//! `!checkForWorkOrder()` (`EntityAIStructureBuilder.java:137-141`): a builder *with*
//! an order does not top its inventory up after a dump; its `afterDump` sends it to
//! `PICK_UP` instead (`:202-205`), which is the same errand, run deliberately.
//!
//! **What needs the blueprint and is not here.** The switch over the eight stages
//! (`:379-459`) walks a structure iterator: it asks, block by block, *what shall be
//! placed at this position*. Nothing in this port can answer that — a blueprint is
//! `crate::building`'s data, and the iterator is Structurize's. So the caller runs
//! one step and reports its [`Placement`], and this module decides what the answer
//! means. That is the whole division: the placer owns "where and what", this module
//! owns "and now what".

use super::facts::{Placement, Pos, Stages, WorkOrder};
use super::state::{Action, Blocking, Machine, Transition, WorkerState};
use super::worker::{
    JobProfile, Refusal, Scene, MIN_OPEN_SLOTS, STANDARD_DELAY, WOKR_IN_BUILDING_DIST,
};

/// `EntityAIStructureBuilder.ACTIONS_UNTIL_DUMP` (`:50`): a builder carries 4096
/// blocks worth of work before it empties its pockets — its own `PICK_UP` handles
/// materials far more often than the base rule would.
pub const ACTIONS_UNTIL_DUMP: u32 = 4096;
/// `EntityAIStructureBuilder.LEVEL_TO_PURGE_MOBS` (`:55`): from this hut level on, a
/// build kills the monsters standing on the site (`:166-187`). Not a decision this
/// module makes — it needs entities — kept because `checkForExtraBuildingActions`
/// calls it on every step.
pub const LEVEL_TO_PURGE_MOBS: i32 = 4;
/// `EntityAIStructureBuilder.SPEED_BUFF_0` (`:45`): the mining-time multiplier at
/// depth zero (`:278-282`).
pub const SPEED_BUFF_0: f64 = 0.5;
/// `AbstractEntityAIStructure.STANDARD_DELAY` is inherited: `structureStep`,
/// `completeBuild` and `pickUpMaterial` all run every five ticks (`:194`, `:198`,
/// `:199`).
pub const STEP_RATE: u32 = STANDARD_DELAY;

/// What `checkForWorkOrder` answered (`EntityAIStructureBuilder.java:96-123`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three exits of `checkForWorkOrder`
pub enum WorkOrderCheck {
    /// An order, with its blueprint, on a building that exists (`:122`).
    Ready,
    /// `!building.hasWorkOrder()` or a null order (`:98-113`).
    NoOrder,
    /// The order points at a building that is gone, and the order is not a removal
    /// (`:115-120`) — the builder completes it and forgets it.
    BuildingMissing,
}

/// Why `checkIfCanceled` says the current structure must be dropped
/// (`AbstractEntityAIStructureWithWorkOrder.java:480-497`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three ways a structure becomes unbuildable
pub enum CancelReason {
    /// The hut lost its work order while a placer was loaded (`:482`) — somebody
    /// cancelled the build.
    OrderLost,
    /// The placer holds no blueprint (`:482`).
    BlueprintLost,
    /// The work order's site is in an unloaded chunk (`:496`).
    ChunkUnloaded,
}

/// The exits of `structureStep` (`AbstractEntityAIStructure.java:334-535`), as the
/// AI follows them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one variant per `return` of `structureStep`
pub enum BuilderStep {
    /// Nothing to build at all (`:336-339`) → `IDLE`.
    NoStructure,
    /// The placer has no stage (`:340-344`) → forget the structure, `IDLE`.
    NoStage,
    /// No room in the inventory (`:346-349`) → `INVENTORY_FULL`.
    NoSpace,
    /// Still walking to the block (`:365-370`) → stay.
    Walking,
    /// Run one step: no step was reported yet, so nothing has been placed.
    Run,
    /// A step placed or removed a block (`:517-534`) → stay.
    Placed,
    /// `FAIL`: logged, and treated as "not finished" (`:461-464`) → stay.
    Failed,
    /// The last stage is over (`:467-475`) → `COMPLETE_BUILD`.
    Complete,
    /// The stage is over and another follows (`:469-477`): progress stored → stay.
    StageAdvanced,
    /// `LIMIT_REACHED` (`:483-487`): store where we stopped → stay.
    Limit,
    /// `MISSING_ITEMS` and the list must be recomputed (`:509-512`) →
    /// `LOAD_STRUCTURE`.
    Reload,
    /// `MISSING_ITEMS` (`:514`) → `NEEDS_ITEM`.
    NeedsItem,
    /// `BREAK_BLOCK` (`:520-525`) → `MINE_BLOCK` at that position.
    Break(Pos),
}

/// The exits of `doMining` (`AbstractEntityAIStructure.java:704-734`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one variant per `return` of `doMining`
pub enum MiningOutcome {
    /// No block to mine (`:706-709`) → `BUILDING_STEP`.
    NothingToMine,
    /// No placer, so no site (`:711-714`) → `IDLE`.
    NoStructure,
    /// The block is air or water now (`:716-720`) → `BUILDING_STEP`.
    AlreadyGone,
    /// Still walking to the block (`:722-725`) → stay.
    Walking,
    /// No standing position yet (`:727-730`) → stay.
    NotInPosition,
    /// The block is being hit, not broken yet (`:732-736`) → stay.
    StillMining,
    /// The block broke (`:738-739`) → `BUILDING_STEP`.
    Mined,
}

impl MiningOutcome {
    /// What the state machine does with it.
    pub fn next(self) -> Option<WorkerState> {
        match self {
            MiningOutcome::NothingToMine | MiningOutcome::AlreadyGone | MiningOutcome::Mined => {
                Some(WorkerState::BuildingStep)
            }
            MiningOutcome::NoStructure => Some(WorkerState::Idle),
            MiningOutcome::Walking | MiningOutcome::NotInPosition | MiningOutcome::StillMining => {
                Some(WorkerState::MineBlock)
            }
        }
    }
}

/// `isThereAStructureToBuild` (`:1055-1062`): a placer with a blueprint and a work
/// order behind it.
pub fn is_there_a_structure(scene: &Scene) -> bool {
    match scene.facts.builder() {
        None => false,
        Some(builder) => {
            builder.placer
                && builder
                    .work_order
                    .map(|order| order.has_blueprint)
                    .unwrap_or(false)
        }
    }
}

/// `checkForWorkOrder` (`EntityAIStructureBuilder.java:96-123`).
///
/// The order's own building is a fact (`order_building_exists`), because Java reaches
/// it through the colony's building manager (`:115`).
pub fn check_for_work_order(scene: &Scene) -> WorkOrderCheck {
    let Some(builder) = scene.facts.builder() else {
        return WorkOrderCheck::NoOrder;
    };
    let Some(order) = builder.work_order else {
        return WorkOrderCheck::NoOrder; // :98-113
    };
    if !order.kind.is_removal() && !builder.order_building_exists {
        return WorkOrderCheck::BuildingMissing; // :115-120
    }
    WorkOrderCheck::Ready // :122
}

/// `checkIfCanceled` (`AbstractEntityAIStructureWithWorkOrder.java:480-497`).
pub fn check_if_canceled(scene: &Scene) -> Option<CancelReason> {
    let Some(builder) = scene.facts.builder() else {
        return None;
    };
    let order = builder.work_order;
    if order.is_none() && builder.placer {
        return Some(CancelReason::OrderLost); // :482
    }
    if builder.placer && !order.map(|order| order.has_blueprint).unwrap_or(false) {
        return Some(CancelReason::BlueprintLost); // :482
    }
    match order {
        Some(order) if !scene.facts.blocks.is_loaded(order.at) => {
            Some(CancelReason::ChunkUnloaded) // :496
        }
        _ => None,
    }
}

/// `structureStep`'s decisions (`:334-535`), without running the step.
pub fn step_outcome(scene: &Scene) -> BuilderStep {
    if !is_there_a_structure(scene) {
        return BuilderStep::NoStructure; // :336-339
    }
    let Some(builder) = scene.facts.builder() else {
        return BuilderStep::NoStructure;
    };
    let Some(stage) = builder.stage else {
        return BuilderStep::NoStage; // :340-344
    };
    if !scene.facts.inventory.has_space {
        return BuilderStep::NoSpace; // :346-349
    }
    if !builder.at_construction_site && !builder.limit_reached {
        return BuilderStep::Walking; // :365-370
    }
    match builder.placement {
        None => BuilderStep::Run,
        Some(Placement::Placed) => BuilderStep::Placed, // :517-534
        Some(Placement::Fail) => BuilderStep::Failed,   // :461-464
        Some(Placement::Finished) => {
            let stages = builder
                .work_order
                .map(|order| order.kind.stages())
                .unwrap_or(Stages::NEW_BUILD);
            match stages.next(stage) {
                // :471-476 — `goToNextStage` said no: the build is over.
                None => BuilderStep::Complete,
                // :477-482 — the next stage is stored and the step carries on.
                Some(_) => BuilderStep::StageAdvanced,
            }
        }
        Some(Placement::LimitReached) => BuilderStep::Limit, // :483-487
        Some(Placement::MissingItems) => {
            if builder.recalculate {
                BuilderStep::Reload // :509-512
            } else {
                BuilderStep::NeedsItem // :514
            }
        }
        Some(Placement::BreakBlock(at)) => BuilderStep::Break(at), // :520-525
    }
}

/// `doMining` (`:704-734`), without swinging.
pub fn mining_decision(scene: &Scene) -> MiningOutcome {
    let Some(block) = scene.memory.block_to_mine else {
        return MiningOutcome::NothingToMine; // :706-709
    };
    let Some(builder) = scene.facts.builder() else {
        return MiningOutcome::NoStructure;
    };
    if !builder.placer {
        return MiningOutcome::NoStructure; // :711-714
    }
    if scene.facts.blocks.at(block).needs_no_mining() || scene.facts.blocks.at(block).is_fluid() {
        return MiningOutcome::AlreadyGone; // :716-720
    }
    if !builder.at_construction_site {
        return MiningOutcome::Walking; // :722-725
    }
    if scene.memory.work_from.is_none() {
        return MiningOutcome::NotInPosition; // :727-730
    }
    if !builder.mining_done {
        return MiningOutcome::StillMining; // :732-736
    }
    MiningOutcome::Mined // :738-739
}

/// The stage list a work order type walks (`AbstractEntityAIStructure.java:762-786`).
pub fn stages_for(order: Option<WorkOrder>) -> Stages {
    match order {
        Some(order) => order.kind.stages(),
        None => Stages::NEW_BUILD,
    }
}

/// The builder's `checkIfNeedsItem` override
/// (`AbstractEntityAIStructureWithWorkOrder.java:211-217`).
///
/// It narrows the base rule to false while a structure is loaded but its materials
/// have not been asked for yet: the builder is about to request them itself through
/// `PICK_UP`, and asking the request system for the same stacks twice would be
/// wasted.
pub fn structure_check_if_needs_item(scene: &Scene, state: WorkerState) -> bool {
    let Some(builder) = scene.facts.builder() else {
        return false;
    };
    let quiet = match builder.work_order {
        None => false,
        Some(order) => {
            builder.needed_resources_empty
                && scene.facts.building.completed_sync == 0
                && !builder.recalculate
                && (!builder.placer || !order.has_blueprint || !order.requested)
        }
    };
    if quiet {
        return false; // :213-216
    }
    // `super.checkIfNeedsItem()` (`:217`) — the base predicate.
    super::worker::base_check_if_needs_item(scene, state)
}

/// Register the nine targets of `AbstractEntityAIStructure.java:166-200`.
pub fn register(machine: &mut Machine<WorkerState, Scene>) {
    // :170 — `new AIEventTarget(STATE_BLOCKING, this::checkIfCanceled, IDLE, 1)`.
    machine.add(Transition::event_when(
        Blocking::StateBlocking,
        "checkIfCanceled",
        "AbstractEntityAIStructure.java:170",
        1,
        |scene: &Scene, _: WorkerState| check_if_canceled(scene).is_some(),
        Action::Run(|scene: &mut Scene, _: WorkerState| {
            let reason = check_if_canceled(scene);
            scene.forget_structure();
            scene.refuse(match reason {
                Some(CancelReason::OrderLost) => Refusal::NoWorkOrder,
                Some(CancelReason::BlueprintLost) => Refusal::NoBlueprint,
                Some(CancelReason::ChunkUnloaded) | None => Refusal::NotArrived,
            });
            Some(WorkerState::Idle)
        }),
    ));
    // :174 — `new AITarget(LOAD_STRUCTURE, this::loadRequirements, 5)`.
    machine.add(Transition::target(
        WorkerState::LoadStructure,
        "loadRequirements",
        "AbstractEntityAIStructure.java:174",
        5,
        Action::Run(load_requirements),
    ));
    // :178 — `new AITarget(START_BUILDING, this::startBuilding, 1)`.
    machine.add(Transition::target(
        WorkerState::StartBuilding,
        "startBuilding",
        "AbstractEntityAIStructure.java:178",
        1,
        Action::Run(start_building),
    ));
    // :182 — `new AITarget(MINE_BLOCK, this::doMining, 10)`.
    machine.add(Transition::target(
        WorkerState::MineBlock,
        "doMining",
        "AbstractEntityAIStructure.java:182",
        10,
        Action::Run(do_mining),
    ));
    // :186 — `new AITarget(IDLE, START_WORKING, 10)`.
    machine.add(Transition::goto(
        WorkerState::Idle,
        WorkerState::StartWorking,
        "idleToStartWorking",
        "AbstractEntityAIStructure.java:186",
        10,
    ));
    // :190 — `new AITarget(START_WORKING, this::startWorkingAtOwnBuilding, TICKS_SECOND)`.
    machine.add(Transition::target(
        WorkerState::StartWorking,
        "startWorkingAtOwnBuilding",
        "AbstractEntityAIStructure.java:190",
        20,
        Action::Run(start_working),
    ));
    // :194 — `new AITarget(BUILDING_STEP, this::structureStep, STANDARD_DELAY)`.
    machine.add(Transition::target(
        WorkerState::BuildingStep,
        "structureStep",
        "AbstractEntityAIStructure.java:194",
        STEP_RATE,
        Action::Run(structure_step),
    ));
    // :198 — `new AITarget(COMPLETE_BUILD, this::completeBuild, STANDARD_DELAY)`.
    machine.add(Transition::target(
        WorkerState::CompleteBuild,
        "completeBuild",
        "AbstractEntityAIStructure.java:198",
        STEP_RATE,
        Action::Run(complete_build),
    ));
    // :199 — `new AITarget(PICK_UP, this::pickUpMaterial, 5)`.
    machine.add(Transition::target(
        WorkerState::PickUp,
        "pickUpMaterial",
        "AbstractEntityAIStructure.java:199",
        5,
        Action::Run(pick_up_material),
    ));
}

/// The builder's profile: the base answers plus the five overrides it makes.
pub const PROFILE: JobProfile = JobProfile::BUILDER;

/// The full builder AI: the base table, then the nine targets above.
pub fn ai() -> super::worker::JobAi {
    let mut ai = super::worker::JobAi::base(crate::jobs::job::Job::Builder);
    register(ai.machine_mut());
    ai
}

// ------------------------------------------------------------------- actions

/// `loadRequirements` (`AbstractEntityAIStructureWithWorkOrder.java:113-136`).
fn load_requirements(scene: &mut Scene, state: WorkerState) -> Option<WorkerState> {
    let Some(builder) = scene.facts.builder().cloned() else {
        scene.refuse(Refusal::NoWorkOrder);
        return Some(WorkerState::Idle);
    };
    if builder.loading_blueprint {
        return Some(state); // :113-116
    }
    match builder.work_order {
        None => {
            // `loadStructure()` found nothing and the work order is gone (`:120-136`).
            scene.refuse(Refusal::NoWorkOrder);
            Some(WorkerState::Idle)
        }
        Some(order) if !order.has_blueprint => {
            scene.refuse(Refusal::NoBlueprint);
            Some(WorkerState::Idle)
        }
        Some(_) if !builder.placer => {
            // A load is starting (`:118-122`): hold the state until it finishes.
            Some(state)
        }
        Some(order) if order.requested => Some(WorkerState::StartBuilding), // :129-134
        Some(_) => Some(state), // `requestMaterialsState()` (`:136-138`)
    }
}

/// `startBuilding` (`AbstractEntityAIStructure.java:292-299`).
fn start_building(scene: &mut Scene, _: WorkerState) -> Option<WorkerState> {
    if is_there_a_structure(scene) {
        return Some(WorkerState::BuildingStep); // :298
    }
    scene.refuse(Refusal::NoStructure);
    Some(WorkerState::LoadStructure) // :294-297
}

/// `startWorkingAtOwnBuilding`, the builder's override
/// (`EntityAIStructureBuilder.java:144-161`).
fn start_working(scene: &mut Scene, state: WorkerState) -> Option<WorkerState> {
    if !scene
        .facts
        .worker
        .at
        .is_living_at_site(scene.facts.building.position, WOKR_IN_BUILDING_DIST)
    {
        return Some(state); // `!walkToBuilding()`, :146-149
    }
    match check_for_work_order(scene) {
        WorkOrderCheck::Ready => {
            if is_there_a_structure(scene) {
                Some(WorkerState::StartBuilding) // :151-157
            } else {
                // `super.startWorkingAtOwnBuilding()` answered `IDLE`, which the
                // builder turns into a load (`:154-157`).
                Some(WorkerState::LoadStructure)
            }
        }
        WorkOrderCheck::NoOrder => {
            scene.forget_structure();
            scene.refuse(Refusal::NoWorkOrder);
            Some(WorkerState::Idle) // :160
        }
        WorkOrderCheck::BuildingMissing => {
            scene.forget_structure();
            scene.refuse(Refusal::NoWorkOrder);
            Some(WorkerState::Idle) // :115-120
        }
    }
}

/// `doMining` (`:704-734`) through the machine.
fn do_mining(scene: &mut Scene, state: WorkerState) -> Option<WorkerState> {
    let outcome = mining_decision(scene);
    match outcome {
        MiningOutcome::Walking | MiningOutcome::NotInPosition | MiningOutcome::StillMining => {
            Some(state)
        }
        MiningOutcome::Mined => {
            // `mineBlock` ends with `incrementActionsDone()` (`:238-240` in
            // `AbstractEntityAIInteract.java:151-243`).
            scene.facts.inventory.actions_done += 1;
            scene.memory.block_to_mine = None;
            outcome.next()
        }
        MiningOutcome::NothingToMine => {
            scene.memory.block_to_mine = None;
            Some(WorkerState::BuildingStep)
        }
        MiningOutcome::AlreadyGone => {
            scene.memory.block_to_mine = None;
            Some(WorkerState::BuildingStep)
        }
        MiningOutcome::NoStructure => {
            scene.refuse(Refusal::NoStructure);
            Some(WorkerState::Idle)
        }
    }
}

/// `structureStep` (`:334-535`) through the machine.
fn structure_step(scene: &mut Scene, state: WorkerState) -> Option<WorkerState> {
    let step = step_outcome(scene);
    match step {
        BuilderStep::NoStructure => {
            scene.refuse(Refusal::NoStructure);
            Some(WorkerState::Idle) // :336-339
        }
        BuilderStep::NoStage => {
            scene.forget_structure();
            scene.refuse(Refusal::StageLost);
            Some(WorkerState::Idle) // :340-344
        }
        BuilderStep::NoSpace => Some(WorkerState::InventoryFull), // :346-349
        BuilderStep::Walking => Some(state),                      // :365-370
        BuilderStep::Run | BuilderStep::Placed | BuilderStep::Failed => {
            scene.memory.limit_reached = false; // :372
            Some(WorkerState::BuildingStep)     // :534
        }
        BuilderStep::Complete => Some(WorkerState::CompleteBuild), // :475
        BuilderStep::StageAdvanced => {
            scene.memory.limit_reached = false; // :372
            Some(WorkerState::BuildingStep)
        }
        BuilderStep::Limit => {
            scene.memory.limit_reached = true; // :485
            Some(WorkerState::BuildingStep)
        }
        BuilderStep::Reload => Some(WorkerState::LoadStructure), // :512
        BuilderStep::NeedsItem => {
            scene.refuse(Refusal::MissingMaterials);
            Some(WorkerState::NeedsItem) // :514
        }
        BuilderStep::Break(at) => {
            scene.memory.block_to_mine = Some(at); // :522-523
            Some(WorkerState::MineBlock)           // :524
        }
    }
}

/// `completeBuild` (`:275-282`): one more action done, one building's worth of xp,
/// the structure forgotten, back to `IDLE`.
fn complete_build(scene: &mut Scene, _: WorkerState) -> Option<WorkerState> {
    scene.facts.inventory.actions_done += 1; // :277
    scene.forget_structure(); // `resetCurrentStructure()`, :280
    Some(WorkerState::Idle) // :281
}

/// `pickUpMaterial` (`:221-268`): top the inventory up from the hut, one resource at
/// a time, then go back to work.
fn pick_up_material(scene: &mut Scene, _: WorkerState) -> Option<WorkerState> {
    let Some(builder) = scene.facts.builder().cloned() else {
        return Some(WorkerState::Idle);
    };
    if !builder.placer
        || !builder
            .work_order
            .map(|order| order.has_blueprint)
            .unwrap_or(false)
    {
        return Some(WorkerState::Idle); // :223-226
    }
    match builder.stage {
        None => {
            scene.memory.picked = 0;
            Some(WorkerState::StartWorking) // :228-232
        }
        Some(stage) if !stages_for(builder.work_order).is_work(stage) => {
            scene.memory.picked = 0;
            Some(WorkerState::StartWorking) // :228-232
        }
        Some(_) => {
            if builder.pickup_pending <= scene.memory.picked
                || scene.facts.inventory.open_slots as u32 <= MIN_OPEN_SLOTS
            {
                // :251-257 — nothing left to fetch, or no room to carry it.
                scene.memory.picked = 0;
                return Some(WorkerState::StartWorking);
            }
            scene.memory.picked += 1; // :259-260
            if builder.needed_in_hut {
                Some(WorkerState::GatheringRequiredMaterials) // :262-265
            } else {
                Some(WorkerState::PickUp) // :267
            }
        }
    }
}

impl Scene {
    /// `resetCurrentStructure()` (`:1111-1118`) and `checkForWorkOrder`'s progress
    /// reset (`EntityAIStructureBuilder.java:100-112`): drop everything the AI knew
    /// about the structure being built.
    pub fn forget_structure(&mut self) {
        self.memory.work_from = None;
        self.memory.block_to_mine = None;
        self.memory.picked = 0;
        self.memory.limit_reached = false;
        if let Some(builder) = builder_mut(&mut self.facts) {
            builder.placer = false;
            builder.stage = None;
            builder.progress = None;
        }
    }
}

/// The builder half of a scene's facts, mutably, if this is a builder.
fn builder_mut(facts: &mut super::facts::WorldFacts) -> Option<&mut super::facts::BuilderFacts> {
    match &mut facts.job {
        super::facts::JobFacts::Builder(builder) => Some(builder),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::facts::{
        Block, BuilderFacts, InventoryFacts, JobFacts, Stage, WorkerFacts, WorldFacts,
    };

    /// A builder standing at its hut, with `builder` facts and the builder profile.
    fn builder_scene(builder: BuilderFacts) -> Scene {
        Scene::with_profile(
            WorldFacts {
                job: JobFacts::Builder(builder),
                ..WorldFacts::new()
            },
            PROFILE,
        )
    }

    fn ready(work_order: WorkOrder) -> BuilderFacts {
        BuilderFacts {
            work_order: Some(work_order),
            stage: Some(Stage::BuildSolid),
            placer: true,
            order_building_exists: true,
            at_construction_site: true,
            ..BuilderFacts::default()
        }
    }

    #[test]
    fn a_structure_needs_a_placer_a_blueprint_and_an_order() {
        // `:1055-1062`.
        let mut scene = builder_scene(BuilderFacts::default());
        assert!(!is_there_a_structure(&scene));
        scene.facts.job = JobFacts::Builder(ready(WorkOrder::new_build(Pos::new(1, 64, 1))));
        assert!(is_there_a_structure(&scene));
        if let JobFacts::Builder(builder) = &mut scene.facts.job {
            builder.placer = false;
        }
        assert!(!is_there_a_structure(&scene), "no placer, no structure");
    }

    #[test]
    fn a_missing_order_or_a_missing_building_is_not_a_ready_order() {
        // `EntityAIStructureBuilder.java:96-123`.
        let mut scene = builder_scene(BuilderFacts::default());
        assert_eq!(check_for_work_order(&scene), WorkOrderCheck::NoOrder);
        scene.facts.job = JobFacts::Builder(ready(WorkOrder::new_build(Pos::new(1, 64, 1))));
        assert_eq!(check_for_work_order(&scene), WorkOrderCheck::Ready);
        if let JobFacts::Builder(builder) = &mut scene.facts.job {
            builder.order_building_exists = false;
        }
        assert_eq!(check_for_work_order(&scene), WorkOrderCheck::BuildingMissing);
        // …unless the order is a removal, which has no building to point at.
        if let JobFacts::Builder(builder) = &mut scene.facts.job {
            builder.work_order = Some(WorkOrder {
                at: Pos::new(1, 64, 1),
                kind: crate::ai::facts::OrderKind::Remove,
                requested: false,
                has_blueprint: true,
            });
        }
        assert_eq!(check_for_work_order(&scene), WorkOrderCheck::Ready);
    }

    #[test]
    fn an_order_lost_or_a_blueprint_lost_cancels_and_an_unloaded_site_too() {
        // `AbstractEntityAIStructureWithWorkOrder.java:480-497`.
        let mut scene = builder_scene(ready(WorkOrder::new_build(Pos::new(1, 64, 1))));
        assert_eq!(check_if_canceled(&scene), None);
        if let JobFacts::Builder(builder) = &mut scene.facts.job {
            builder.work_order = None;
        }
        assert_eq!(check_if_canceled(&scene), Some(CancelReason::OrderLost));

        let mut scene = builder_scene(ready(WorkOrder::new_build(Pos::new(1, 64, 1))));
        if let JobFacts::Builder(builder) = &mut scene.facts.job {
            builder.work_order = Some(WorkOrder {
                at: Pos::new(1, 64, 1),
                kind: crate::ai::facts::OrderKind::Build,
                requested: false,
                has_blueprint: false,
            });
        }
        assert_eq!(check_if_canceled(&scene), Some(CancelReason::BlueprintLost));

        let mut scene = builder_scene(ready(WorkOrder::new_build(Pos::new(1, 64, 1))));
        scene.facts.blocks.mark_unloaded(Pos::new(1, 64, 1));
        assert_eq!(check_if_canceled(&scene), Some(CancelReason::ChunkUnloaded));
    }

    #[test]
    fn a_step_with_no_structure_and_no_stage_goes_to_idle() {
        // `:336-344`.
        assert_eq!(
            step_outcome(&builder_scene(BuilderFacts::default())),
            BuilderStep::NoStructure
        );
        let mut builder = ready(WorkOrder::new_build(Pos::new(1, 64, 1)));
        builder.stage = None;
        assert_eq!(step_outcome(&builder_scene(builder.clone())), BuilderStep::NoStage);
    }

    #[test]
    fn a_full_inventory_stops_the_step_before_it_runs() {
        // `:346-349`, and the machine follows the answer.
        let mut scene = builder_scene(ready(WorkOrder::new_build(Pos::new(1, 64, 1))));
        scene.facts.inventory.has_space = false;
        assert_eq!(step_outcome(&scene), BuilderStep::NoSpace);
        let mut machine: Machine<WorkerState, Scene> = Machine::new(WorkerState::BuildingStep, 5);
        crate::ai::worker::register_base(&mut machine);
        register(&mut machine);
        let fired = machine.tick(&mut scene).expect("the step runs");
        assert_eq!(fired.rule, "structureStep");
        assert_eq!(fired.to, WorkerState::InventoryFull);
        assert_eq!(machine.state(), WorkerState::InventoryFull);
    }

    #[test]
    fn walking_to_the_site_holds_the_state_unless_the_limit_was_reached() {
        // `:365-370` — `!limitReached` gates the walk.
        let mut builder = ready(WorkOrder::new_build(Pos::new(1, 64, 1)));
        builder.at_construction_site = false;
        assert_eq!(step_outcome(&builder_scene(builder.clone())), BuilderStep::Walking);
        builder.limit_reached = true;
        assert_ne!(step_outcome(&builder_scene(builder.clone())), BuilderStep::Walking);
    }

    #[test]
    fn the_last_stage_completes_the_build_and_an_intermediate_one_advances() {
        // `:467-482` against the stage array of a new build.
        let mut builder = ready(WorkOrder::new_build(Pos::new(1, 64, 1)));
        builder.stage = Some(Stage::BuildSolid);
        builder.placement = Some(Placement::Finished);
        assert_eq!(step_outcome(&builder_scene(builder.clone())), BuilderStep::StageAdvanced);
        builder.stage = Some(Stage::Spawn); // the last of the eight
        assert_eq!(step_outcome(&builder_scene(builder.clone())), BuilderStep::Complete);
        // A removal has two stages, so REMOVE_WATER finishes into REMOVE.
        let mut removal = ready(WorkOrder {
            at: Pos::new(1, 64, 1),
            kind: crate::ai::facts::OrderKind::Remove,
            requested: true,
            has_blueprint: true,
        });
        removal.stage = Some(Stage::RemoveWater);
        removal.placement = Some(Placement::Finished);
        assert_eq!(step_outcome(&builder_scene(removal.clone())), BuilderStep::StageAdvanced);
        removal.stage = Some(Stage::Remove);
        assert_eq!(step_outcome(&builder_scene(removal)), BuilderStep::Complete);
    }

    #[test]
    fn missing_items_reload_the_list_or_ask_for_them() {
        // `:507-515`.
        let mut builder = ready(WorkOrder::new_build(Pos::new(1, 64, 1)));
        builder.placement = Some(Placement::MissingItems);
        assert_eq!(step_outcome(&builder_scene(builder.clone())), BuilderStep::NeedsItem);
        builder.recalculate = true;
        assert_eq!(step_outcome(&builder_scene(builder.clone())), BuilderStep::Reload);
    }

    #[test]
    fn a_block_in_the_way_sends_the_builder_to_mine_it() {
        // `:520-525`, then `doMining`'s `:716-720` for a block already gone.
        let at = Pos::new(2, 65, 2);
        let mut builder = ready(WorkOrder::new_build(Pos::new(1, 64, 1)));
        builder.placement = Some(Placement::BreakBlock(at));
        assert_eq!(step_outcome(&builder_scene(builder.clone())), BuilderStep::Break(at));

        let mut scene = builder_scene(ready(WorkOrder::new_build(Pos::new(1, 64, 1))));
        scene.memory.block_to_mine = Some(at);
        assert_eq!(mining_decision(&scene), MiningOutcome::AlreadyGone);
        scene.facts.blocks.set(at, Block::Solid);
        assert_eq!(mining_decision(&scene), MiningOutcome::NotInPosition);
        scene.memory.work_from = Some(Pos::new(1, 64, 2));
        assert_eq!(mining_decision(&scene), MiningOutcome::StillMining);
        if let JobFacts::Builder(builder) = &mut scene.facts.job {
            builder.mining_done = true;
        }
        assert_eq!(mining_decision(&scene), MiningOutcome::Mined);
        assert_eq!(MiningOutcome::Mined.next(), Some(WorkerState::BuildingStep));
        assert_eq!(MiningOutcome::NoStructure.next(), Some(WorkerState::Idle));
    }

    #[test]
    fn a_limit_reached_step_is_remembered_and_the_next_one_skips_the_walk() {
        // `:483-487` then `:372`.
        let mut builder = ready(WorkOrder::new_build(Pos::new(1, 64, 1)));
        builder.placement = Some(Placement::LimitReached);
        let mut scene = builder_scene(builder.clone());
        assert_eq!(step_outcome(&scene), BuilderStep::Limit);
        assert_eq!(
            structure_step(&mut scene, WorkerState::BuildingStep),
            Some(WorkerState::BuildingStep)
        );
        assert!(scene.memory.limit_reached);
    }

    #[test]
    fn load_requirements_waits_holds_or_starts_building() {
        // `AbstractEntityAIStructureWithWorkOrder.java:113-136`.
        let mut builder = ready(WorkOrder::new_build(Pos::new(1, 64, 1)));
        builder.loading_blueprint = true;
        let mut scene = builder_scene(builder.clone());
        assert_eq!(
            load_requirements(&mut scene, WorkerState::LoadStructure),
            Some(WorkerState::LoadStructure)
        );

        let mut scene = builder_scene(BuilderFacts::default());
        assert_eq!(
            load_requirements(&mut scene, WorkerState::LoadStructure),
            Some(WorkerState::Idle)
        );
        assert_eq!(scene.memory.refusal, Some(Refusal::NoWorkOrder));

        let mut builder = ready(WorkOrder::new_build(Pos::new(1, 64, 1)));
        builder.work_order = Some(WorkOrder {
            at: Pos::new(1, 64, 1),
            kind: crate::ai::facts::OrderKind::Build,
            requested: false,
            has_blueprint: false,
        });
        let mut scene = builder_scene(builder.clone());
        assert_eq!(
            load_requirements(&mut scene, WorkerState::LoadStructure),
            Some(WorkerState::Idle)
        );
        assert_eq!(scene.memory.refusal, Some(Refusal::NoBlueprint));

        // Loaded, not requested yet: the material request starts and the state holds.
        let mut scene = builder_scene(ready(WorkOrder::new_build(Pos::new(1, 64, 1))));
        assert_eq!(
            load_requirements(&mut scene, WorkerState::LoadStructure),
            Some(WorkerState::LoadStructure)
        );

        // Requested: afterStructureLoading answers START_BUILDING.
        let mut builder = ready(WorkOrder {
            at: Pos::new(1, 64, 1),
            kind: crate::ai::facts::OrderKind::Build,
            requested: true,
            has_blueprint: true,
        });
        builder.stage = None;
        let mut scene = builder_scene(builder.clone());
        assert_eq!(
            load_requirements(&mut scene, WorkerState::LoadStructure),
            Some(WorkerState::StartBuilding)
        );
    }

    #[test]
    fn starting_to_build_needs_the_blueprint_or_goes_back_to_load_it() {
        // `:292-299`.
        let mut scene = builder_scene(ready(WorkOrder::new_build(Pos::new(1, 64, 1))));
        assert_eq!(
            start_building(&mut scene, WorkerState::StartBuilding),
            Some(WorkerState::BuildingStep)
        );
        let mut scene = builder_scene(BuilderFacts::default());
        assert_eq!(
            start_building(&mut scene, WorkerState::StartBuilding),
            Some(WorkerState::LoadStructure)
        );
    }

    #[test]
    fn a_builder_away_from_its_hut_keeps_walking() {
        // `EntityAIStructureBuilder.java:146-149`.
        let mut scene = builder_scene(ready(WorkOrder::new_build(Pos::new(1, 64, 1))));
        scene.facts.worker.at = Pos::new(40, 64, 40);
        assert_eq!(
            start_working(&mut scene, WorkerState::StartWorking),
            Some(WorkerState::StartWorking)
        );
        scene.facts.worker.at = scene.facts.building.position;
        assert_eq!(
            start_working(&mut scene, WorkerState::StartWorking),
            Some(WorkerState::StartBuilding)
        );
        // With an order but nothing loaded, the builder loads first.
        if let JobFacts::Builder(builder) = &mut scene.facts.job {
            builder.placer = false;
        }
        assert_eq!(
            start_working(&mut scene, WorkerState::StartWorking),
            Some(WorkerState::LoadStructure)
        );
    }

    #[test]
    fn pick_up_fetches_one_resource_and_stops_when_the_inventory_is_short() {
        // `:221-268`.
        let mut builder = ready(WorkOrder::new_build(Pos::new(1, 64, 1)));
        builder.pickup_pending = 3;
        builder.needed_in_hut = true;
        let mut scene = builder_scene(builder.clone());
        assert_eq!(
            pick_up_material(&mut scene, WorkerState::PickUp),
            Some(WorkerState::GatheringRequiredMaterials)
        );
        assert_eq!(scene.memory.picked, 1);
        // Not in the hut: it goes to PICK_UP, which is the state it is already in.
        if let JobFacts::Builder(builder) = &mut scene.facts.job {
            builder.needed_in_hut = false;
        }
        assert_eq!(
            pick_up_material(&mut scene, WorkerState::PickUp),
            Some(WorkerState::PickUp)
        );
        assert_eq!(scene.memory.picked, 2);
        // Five free slots is the floor: no room, back to work.
        scene.facts.inventory.open_slots = MIN_OPEN_SLOTS as usize;
        assert_eq!(
            pick_up_material(&mut scene, WorkerState::PickUp),
            Some(WorkerState::StartWorking)
        );
        assert_eq!(scene.memory.picked, 0);
    }

    #[test]
    fn pick_up_with_nothing_loaded_goes_idle_and_a_clear_stage_goes_back_to_work() {
        // `:223-232`.
        let mut scene = builder_scene(BuilderFacts::default());
        assert_eq!(
            pick_up_material(&mut scene, WorkerState::PickUp),
            Some(WorkerState::Idle)
        );
        let mut builder = ready(WorkOrder::new_build(Pos::new(1, 64, 1)));
        builder.stage = Some(Stage::Clear);
        let mut scene = builder_scene(builder.clone());
        assert_eq!(
            pick_up_material(&mut scene, WorkerState::PickUp),
            Some(WorkerState::StartWorking)
        );
    }

    #[test]
    fn completing_a_build_counts_one_action_and_forgets_the_structure() {
        // `:275-282`.
        let mut scene = builder_scene(ready(WorkOrder::new_build(Pos::new(1, 64, 1))));
        assert_eq!(
            complete_build(&mut scene, WorkerState::CompleteBuild),
            Some(WorkerState::Idle)
        );
        assert_eq!(scene.facts.inventory.actions_done, 1);
        assert!(!is_there_a_structure(&scene), "resetCurrentStructure cleared it");
    }

    #[test]
    fn the_builder_never_dumps_before_4096_actions_and_never_tops_up_with_an_order() {
        // `EntityAIStructureBuilder.java:50`, `:137-141`, `:202-205`.
        assert_eq!(PROFILE.actions_until_dumping, 4096);
        let with_order = builder_scene(ready(WorkOrder::new_build(Pos::new(1, 64, 1))));
        assert!(!PROFILE.after_dump_pickup.allows(&with_order.facts));
        let without = builder_scene(BuilderFacts::default());
        assert!(PROFILE.after_dump_pickup.allows(&without.facts));
        // afterDump is PICK_UP, not IDLE.
        let mut scene = builder_scene(BuilderFacts::default());
        assert_eq!(crate::ai::worker::after_dump(&mut scene), WorkerState::PickUp);
    }

    #[test]
    fn a_builder_without_an_order_may_go_idle() {
        // `:335-339` via `IdleRule::WithoutWorkOrder`.
        let without = builder_scene(BuilderFacts::default());
        assert!(crate::ai::worker::can_go_idle(&PROFILE, &without.facts));
        let with = builder_scene(ready(WorkOrder::new_build(Pos::new(1, 64, 1))));
        assert!(!crate::ai::worker::can_go_idle(&PROFILE, &with.facts));
    }

    #[test]
    fn the_builder_narrows_the_base_needs_item_rule() {
        // `AbstractEntityAIStructureWithWorkOrder.java:211-217`.
        let mut builder = ready(WorkOrder::new_build(Pos::new(1, 64, 1)));
        builder.needed_resources_empty = true;
        let mut scene = builder_scene(builder.clone());
        scene.facts.building.has_open_sync_request = true;
        assert!(
            !structure_check_if_needs_item(&scene, WorkerState::BuildingStep),
            "an order with nothing needed stays quiet"
        );
        scene.facts.building.has_open_sync_request = false;
        scene.facts.building.completed_sync = 1;
        assert!(
            structure_check_if_needs_item(&scene, WorkerState::BuildingStep),
            "a completed request defeats the narrowing (:213 `!hasCitizenCompletedRequests`)"
        );
        // A completed request with materials to fetch does need the state.
        if let JobFacts::Builder(builder) = &mut scene.facts.job {
            builder.needed_resources_empty = false;
        }
        assert!(structure_check_if_needs_item(&scene, WorkerState::BuildingStep));
    }

    #[test]
    fn a_machine_that_starts_on_building_step_runs_the_step_when_it_is_due() {
        // Target 7 at `STANDARD_DELAY` (`:194`).
        let mut scene = builder_scene(ready(WorkOrder::new_build(Pos::new(1, 64, 1))));
        let mut machine: Machine<WorkerState, Scene> = Machine::new(WorkerState::BuildingStep, 5);
        crate::ai::worker::register_base(&mut machine);
        register(&mut machine);
        let fired = machine.tick(&mut scene).expect("first look always runs");
        assert_eq!(fired.rule, "structureStep");
        // No placement was reported yet, so the step is a "run one" and stays.
        assert_eq!(fired.to, WorkerState::BuildingStep);
        // Rate 5 on a machine that ticks every 5: the countdown 5 - 5 = 0 is "due",
        // so `STANDARD_DELAY` here means *every* AI tick — the five ticks of real delay
        // come from `setDelay` instead (`AbstractEntityAIBasic.java:100`, `:218`).
        assert_eq!(machine.transitions().iter().find(|t| t.rule == "structureStep").map(|t| t.tick_rate), Some(5));
        assert!(machine.tick(&mut scene).is_some(), "due again next tick");
    }

    #[test]
    fn the_builders_inventory_of_states_has_no_trap() {
        // Every state the nine targets can enter has at least one transition out —
        // otherwise `BasicStateMachine.java:220-228` would reset the AI. The base
        // table supplies IDLE, START_WORKING, INVENTORY_FULL, NEEDS_ITEM, PAUSED,
        // GATHERING_REQUIRED_MATERIALS and INIT; the builder adds the rest.
        let ai = ai();
        for state in [
            WorkerState::LoadStructure,
            WorkerState::StartBuilding,
            WorkerState::MineBlock,
            WorkerState::BuildingStep,
            WorkerState::CompleteBuild,
            WorkerState::PickUp,
            WorkerState::Idle,
        ] {
            assert!(
                ai.machine().can_leave(state),
                "{state} has no transition out"
            );
        }
        assert_eq!(ai.machine().transitions().len(), 14 + 9);
    }

    #[test]
    fn the_inventory_facts_default_is_an_empty_bag() {
        let facts = InventoryFacts::default();
        assert!(!facts.full);
        assert_eq!(facts.open_slots, 27);
        assert!(WorkerFacts::default().hired);
    }
}
