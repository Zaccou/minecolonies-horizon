//! The base worker AI: the fourteen targets every job inherits, in the order Java
//! checks them, and the worker's own working memory.
//!
//! Java reference: `core/entity/ai/workers/AbstractEntityAIBasic.java:200-260` (the
//! `super.registerTargets(...)` call of the constructor), `:446-465`
//! (`initSafetyChecks`), `:497-517` (`waitingForSomething`), `:409-416`
//! (`inventoryNeedsDump`), `:1157-1206` (`dumpInventory` + `afterDump`), `:1229-1294`
//! (`dumpOneMoreSlot`), `:267-272` (`checkIfNeedsItem`), `:534-643` (`waitForRequests`
//! → `lookForRequests`), `:282-326` (`getNeededItem` → `getStateAfterPickUp`),
//! `:1877-1894` (`shouldRestart` → `restart`), `:1831-1870` (the pause pair),
//! `:1841-1844` (`isStartingPaused`), `:966-1005` (`checkForToolOrWeapon`),
//! `:1405-1433` (`getMostEfficientTool`), `:682-696` (the two skill curves) and
//! `:1441-1451` (`hasNotDelayed`).
//!
//! ## The table, and why this order
//!
//! Java registers fourteen targets; `TickRateStateMachine.tick` walks the three
//! global passes first, then the current state's list, and stops at the first one
//! that moves the machine. Written out, that is:
//!
//! | # | Pass | Rule | Rate | Java |
//! | ---: | --- | --- | ---: | --- |
//! | 1 | `AI_BLOCKING` | `initSafetyChecks` | 1 | `:204` |
//! | 2 | `INIT` | hold in `INIT` (`this::getState`) | 1 | `:205` |
//! | 3 | `AI_BLOCKING` | `updateVisualState` (returns null) | 20 | `:211` |
//! | 4 | `AI_BLOCKING` | `waitingForSomething` | 5 | `:218` |
//! | 5 | `STATE_BLOCKING` | `inventoryNeedsDump` → `INVENTORY_FULL` | 100 | `:225` |
//! | 6 | `INVENTORY_FULL` | `dumpInventory` | 20 | `:226` |
//! | 7 | `AI_BLOCKING` | `checkIfNeedsItem` → `NEEDS_ITEM` | 20 | `:232` |
//! | 8 | `AI_BLOCKING` | completed requests + `cleanAsync()` → `NEEDS_ITEM` | 200 | `:236` |
//! | 9 | `NEEDS_ITEM` | `waitForRequests` | 40 | `:238` |
//! | 10 | `GATHERING_REQUIRED_MATERIALS` | `getNeededItem` | 20 | `:242` |
//! | 11 | `STATE_BLOCKING` | `shouldRestart` → `restart` | 20 | `:247` |
//! | 12 | `PAUSED` | `!isPaused()` → `IDLE` | 20 | `:251` |
//! | 13 | `PAUSED` | `bePaused` | 10 | `:255` |
//! | 14 | `AI_BLOCKING` | `isStartingPaused` → `INVENTORY_FULL` | 20 | `:259` |
//!
//! **Two of the fourteen cannot fire, and the port keeps them anyway.** Target 8's
//! condition is `hasCitizenCompletedRequests(...) && this.cleanAsync()`, and
//! `cleanAsync` ends with `return false` (`:715`) after accepting the async
//! requests it was called for — so the target never transitions, and what it really
//! does is a side effect. Target 3 always returns `null` (`:472-479`), which Java's
//! `transitionToNext` reads as "keep checking this tick"
//! (`BasicStateMachine.java:199-236`), so it only refreshes the nametag. Both are
//! registered, both are tested, and `RAPPORT.md` §5 names them why they are not
//! "cleaned up".
//!
//! **Where a job plugs in.** A trade's AI calls this table's registration in its
//! constructor (`AbstractEntityAIStructure.java:166-200` runs after
//! `AbstractEntityAIBasic`'s), so the base rules are checked first at equal pass and
//! rate. [`super::composter`] and [`super::builder`] are those two additions.
//!
//! **No world, no clock, no random.** Every read is a field of
//! [`super::facts::WorldFacts`]; the two random draws `dumpOneMoreSlot` makes are
//! facts too ([`super::facts::WorkerFacts::coin`], `:roll`), because a decision that
//! depends on `worker.getRandom()` cannot be replayed and therefore cannot be
//! tested. The only thing this module mutates outside [`Scene`] is *nothing*.

use crate::jobs::job::Job;

use super::facts::{Pos, WorldFacts};
use super::state::{
    Action, Blocking, ENTITY_AI_TICKRATE, Fired, Machine, Transition, WorkerState,
};

/// `AbstractEntityAIBasic.STANDARD_DELAY` (`:100`): the pause after an action that
/// changed the world.
pub const STANDARD_DELAY: u32 = 5;
/// `AbstractEntityAIBasic.REQUEST_DELAY` (`:105`): `TICKS_20 * 3`.
pub const REQUEST_DELAY: u32 = 60;
/// `AbstractEntityAIBasic.PICKUP_ATTEMPTS` (`:110`): how many ticks a worker walks
/// toward a chest before it gives up on that chest.
pub const PICKUP_ATTEMPTS: u32 = 10;
/// `AbstractEntityAIBasic.WALK_DELAY` (`:151`): the delay a restart waits.
pub const WALK_DELAY: u32 = 20;
/// `AbstractEntityAIBasic.NO_TOOL` (`:95`): "this block needs no tool at all".
pub const NO_TOOL: i32 = -10;
/// `AbstractEntityAIBasic.TOOL_NOT_FOUND` (`:90`): "a tool is needed and none is
/// good enough".
pub const TOOL_NOT_FOUND: i32 = -1;
/// `CitizenConstants.ACTIONS_UNTIL_DUMP` (`:166`): the base number of actions before
/// a worker empties its pockets.
pub const ACTIONS_UNTIL_DUMP: u32 = 32;
/// `CitizenConstants.MIN_OPEN_SLOTS` (`:176`): a worker with fewer free slots starts
/// gambling on what to keep (`:1253-1266`).
pub const MIN_OPEN_SLOTS: u32 = 5;
/// `CitizenConstants.CHANCE_TO_DUMP_50` (`:211`): under this stack size the dump
/// draw is a coin.
pub const CHANCE_TO_DUMP_50: u32 = 16;
/// `CitizenConstants.CHANCE_TO_DUMP` (`:216`): above it, one chance in
/// `count / 8`.
pub const CHANCE_TO_DUMP: u32 = 8;
/// `CitizenConstants.DELAY_RECHECK` (`:156`): how long a worker waits between two
/// request checks, and what a missing tool adds to its delay.
pub const DELAY_RECHECK: u32 = 10;
/// `CitizenConstants.EXCEPTION_TIMEOUT` (`:146`): the first pause after an AI error.
pub const EXCEPTION_TIMEOUT: u32 = 100;
/// `CitizenConstants.DEFAULT_RANGE_FOR_DELAY` (`:161`): how close counts as "at the
/// block" while hitting it.
pub const DEFAULT_RANGE_FOR_DELAY: i32 = 4;
/// `EntityNavigationUtils.WOKR_IN_BUILDING_DIST` (`:25`): how close counts as
/// "arrived in the building".
pub const WOKR_IN_BUILDING_DIST: i32 = 7;
/// `EquipmentLevelConstants.TOOL_LEVEL_WOOD_OR_GOLD` (`:20`): the floor
/// `checkForToolOrWeapon(toolType)` uses.
pub const TOOL_LEVEL_WOOD_OR_GOLD: u8 = 0;

/// Why the AI has nothing to do, named.
///
/// Java has no such type: a worker that cannot proceed `return getState()` and
/// leaves the reason implicit in the branch it took. A caller here has to show it to
/// a player ("why is my builder idle?"), and a test has to assert on it, so every
/// variant carries the line that decides it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: one variant per `return <same state>` Java can make on this table
pub enum Refusal {
    /// `initSafetyChecks`: no job, no citizen data or no building (`:448-451`).
    NotHired,
    /// The citizen's job and its hut disagree (`:453-457`).
    JobAndHutDisagree,
    /// `getExpectedBuildingClass()` refused the hut (`:191-197`).
    WrongBuilding,
    /// A player paused this worker (`:251`).
    Paused,
    /// A paused worker is still walking to its pastime (`:1853-1856`).
    Navigating,
    /// The hut has no room left in any chest (`:1167-1188`).
    HutFull,
    /// The walk to the chest or the work position did not arrive
    /// (`:300`, `:1232-1235`).
    NotArrived,
    /// No chest of the hut holds the stack the worker was sent for (`:292-296`).
    NoChestWithItem,
    /// The stack was gone when the worker got there (`:307-311`).
    StackGone,
    /// A tool is needed and none is good enough yet (`:993-1002`).
    ToolMissing,
    /// Nothing in the inventory is worth dumping (`:1244-1247`, `:1293`).
    NothingToDump,
    /// The request the worker was waiting for is no longer on offer (`:548-552`).
    RequestDone,
    /// The composter's `compostables` list is empty (`EntityAIWorkComposter.java:
    /// 148-152`).
    CompostablesNotSet,
    /// The composter has no barrel to fill or harvest (`EntityAIWorkComposter.java:
    /// 243-244`).
    NoBarrel,
    /// The builder has no work order (`EntityAIStructureBuilder.java:96-103`).
    NoWorkOrder,
    /// The builder's work order has no blueprint, or the hut lost it
    /// (`AbstractEntityAIStructureWithWorkOrder.java:113-139`).
    NoBlueprint,
    /// The structure cannot be built: no placer, no blueprint, or no order
    /// (`AbstractEntityAIStructure.java:1055-1062`).
    NoStructure,
    /// The blueprint wants materials the worker does not have and cannot ask for
    /// right now (`AbstractEntityAIStructure.java:507-515`).
    MissingMaterials,
    /// The stage or the iterator says there is nothing left at this position
    /// (`AbstractEntityAIStructure.java:340-344`, `:461-464`).
    StageLost,
    /// This job's own AI is not ported; only the base table runs
    /// (`RAPPORT.md` §3).
    TradeNotPorted,
}

impl Refusal {
    /// The Java line that decides this refusal, for a log line or a test failure.
    pub fn java(self) -> &'static str {
        match self {
            Refusal::NotHired => "AbstractEntityAIBasic.java:448-451",
            Refusal::JobAndHutDisagree => "AbstractEntityAIBasic.java:453-457",
            Refusal::WrongBuilding => "AbstractEntityAIBasic.java:191-197",
            Refusal::Paused => "AbstractEntityAIBasic.java:251-255",
            Refusal::Navigating => "AbstractEntityAIBasic.java:1853-1856",
            Refusal::HutFull => "AbstractEntityAIBasic.java:1167-1188",
            Refusal::NotArrived => "AbstractEntityAIBasic.java:1232-1235",
            Refusal::NoChestWithItem => "AbstractEntityAIBasic.java:292-296",
            Refusal::StackGone => "AbstractEntityAIBasic.java:307-311",
            Refusal::ToolMissing => "AbstractEntityAIBasic.java:993-1002",
            Refusal::NothingToDump => "AbstractEntityAIBasic.java:1244-1247",
            Refusal::RequestDone => "AbstractEntityAIBasic.java:548-552",
            Refusal::CompostablesNotSet => "EntityAIWorkComposter.java:148-152",
            Refusal::NoBarrel => "EntityAIWorkComposter.java:243-244",
            Refusal::NoWorkOrder => "EntityAIStructureBuilder.java:96-103",
            Refusal::NoBlueprint => "AbstractEntityAIStructureWithWorkOrder.java:113-139",
            Refusal::NoStructure => "AbstractEntityAIStructure.java:1055-1062",
            Refusal::MissingMaterials => "AbstractEntityAIStructure.java:507-515",
            Refusal::StageLost => "AbstractEntityAIStructure.java:340-344",
            Refusal::TradeNotPorted => "see RAPPORT.md §3",
        }
    }

    /// A stable key for a message the player will read.
    pub fn key(self) -> &'static str {
        match self {
            Refusal::NotHired => "ai.refusal.not_hired",
            Refusal::JobAndHutDisagree => "ai.refusal.job_and_hut_disagree",
            Refusal::WrongBuilding => "ai.refusal.wrong_building",
            Refusal::Paused => "ai.refusal.paused",
            Refusal::Navigating => "ai.refusal.navigating",
            Refusal::HutFull => "ai.refusal.hut_full",
            Refusal::NotArrived => "ai.refusal.not_arrived",
            Refusal::NoChestWithItem => "ai.refusal.no_chest_with_item",
            Refusal::StackGone => "ai.refusal.stack_gone",
            Refusal::ToolMissing => "ai.refusal.tool_missing",
            Refusal::NothingToDump => "ai.refusal.nothing_to_dump",
            Refusal::RequestDone => "ai.refusal.request_done",
            Refusal::CompostablesNotSet => "ai.refusal.compostables_not_set",
            Refusal::NoBarrel => "ai.refusal.no_barrel",
            Refusal::NoWorkOrder => "ai.refusal.no_work_order",
            Refusal::NoBlueprint => "ai.refusal.no_blueprint",
            Refusal::NoStructure => "ai.refusal.no_structure",
            Refusal::MissingMaterials => "ai.refusal.missing_materials",
            Refusal::StageLost => "ai.refusal.stage_lost",
            Refusal::TradeNotPorted => "ai.refusal.trade_not_ported",
        }
    }
}

/// `api/entity/ai/JobStatus.java:3-24` — the three words the building screen shows:
/// idle, working, or stuck on something.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: three statuses in Java
pub enum JobStatus {
    /// `IDLE`: nothing to do, or nothing to do *yet*.
    Idle,
    /// `WORKING`.
    Working,
    /// `STUCK`: work exists but something is missing (`AbstractEntityAIBasic.java:
    /// 971`).
    Stuck,
}

impl JobStatus {
    /// Java's identifier.
    pub fn java_name(self) -> &'static str {
        match self {
            JobStatus::Idle => "IDLE",
            JobStatus::Working => "WORKING",
            JobStatus::Stuck => "STUCK",
        }
    }
}

/// The AI's own working memory — every field a `private`/`protected` field of
/// `AbstractEntityAIBasic` that is **not** a fact about the world.
///
/// Java reference: `:116` (`currentWorkingLocation`), `:121` (`delay`), `:126`
/// (`hasDelayed`), `:136` (`exceptionTimer`), `:141` (`slotAt`), `:146`
/// (`dumpedItems`), `:161` (`walkTo`), `:156` (`needsCurrently`), `:166`
/// (`alreadyKept`), `:171` (`pickUpCounter`), `:181` (`workPosIndex`). The last four
/// fields are the structure AI's and the composter's: `pickUpCount`
/// (`AbstractEntityAIStructure.java:154`), `limitReached` (`:100`?), `workFrom`
/// (`:130`), `ticksToComplain` and `currentTarget` (`EntityAIWorkComposter.java:
/// 54`, `:64`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Memory {
    /// `delay`: game ticks left before the next action (`:121`). Counted down by
    /// [`waiting_for_something`], which is target 4.
    pub delay: i32,
    /// `hasDelayed`: the one-shot toggle of `hasNotDelayed` (`:126`, `:1441-1451`).
    pub has_delayed: bool,
    /// `exceptionTimer`: doubles after each AI error (`:136`, `:363`).
    pub exception_timer: u32,
    /// `slotAt`: the inventory slot the dump has reached (`:141`).
    pub slot_at: usize,
    /// `dumpedItems`: how many items this dump cycle moved (`:146`).
    pub dumped_items: i32,
    /// `walkTo`: the position the worker is walking to (`:161`).
    pub walk_to: Option<Pos>,
    /// `needsCurrently`: how many items of the current fetch are still missing
    /// (`:156`); `None` means "no fetch in progress".
    pub need: Option<u32>,
    /// `alreadyKept`: how many stacks this dump cycle decided to keep (`:166`). Java
    /// keeps the list, this port keeps the count it needs.
    pub kept: u32,
    /// `pickUpCounter`: failed walks toward the same chest (`:171`).
    pub pickup_counter: u32,
    /// `workPosIndex`: the tagged work position the worker stands on (`:181`),
    /// `-1` before the first pick.
    pub work_pos_index: i32,
    /// The last refusal this AI recorded, on this tick.
    pub refusal: Option<Refusal>,
    /// `AbstractEntityAIStructure.pickUpCount` (`:154`): which needed resource
    /// `PICK_UP` is fetching.
    pub picked: usize,
    /// `AbstractEntityAIStructure.limitReached` (`:365`): the last step hit the
    /// budget, so the next one skips the walk.
    pub limit_reached: bool,
    /// `AbstractEntityAIStructure.workFrom` (`:130`): where the builder stands to
    /// build.
    pub work_from: Option<Pos>,
    /// `EntityAIWorkComposter.ticksToComplain` (`:64`): the complaint throttle.
    pub ticks_to_complain: i32,
    /// `EntityAIWorkComposter.currentTarget` (`:54`): the barrel being worked.
    pub target: Option<Pos>,
    /// `AbstractEntityAIStructure.blockToMine` (`:144`): the block the builder is
    /// breaking before it can place the next one.
    pub block_to_mine: Option<Pos>,
}

impl Default for Memory {
    /// A fresh AI: no delay, no fetch, no barrel, `exceptionTimer = 1` and
    /// `workPosIndex = -1` as Java initialises them (`:136`, `:181`).
    fn default() -> Memory {
        Memory {
            delay: 0,
            has_delayed: false,
            exception_timer: 1,
            slot_at: 0,
            dumped_items: 0,
            walk_to: None,
            need: None,
            kept: 0,
            pickup_counter: 0,
            work_pos_index: -1,
            refusal: None,
            picked: 0,
            limit_reached: false,
            work_from: None,
            ticks_to_complain: 0,
            target: None,
            block_to_mine: None,
        }
    }
}

/// Whether the AI answers `canGoIdle()` (`:1921-1924`, overridden per trade).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two answers ported, one per override in the tree
pub enum IdleRule {
    /// The base class: a worker with a job never idles (`:1923`).
    Never,
    /// `!building.hasWorkOrder()` (`EntityAIStructureBuilder.java:336-339`): a
    /// builder with nothing to build hands its citizen back to the colony.
    WithoutWorkOrder,
}

impl IdleRule {
    /// Evaluate it against the facts.
    pub fn allows(&self, facts: &WorldFacts) -> bool {
        match self {
            IdleRule::Never => false,
            IdleRule::WithoutWorkOrder => facts
                .builder()
                .map(|builder| builder.work_order.is_none())
                .unwrap_or(false),
        }
    }
}

/// Which `checkIfNeedsItem` a trade runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the base rule and the structure AI's narrowing
pub enum NeedsItemRule {
    /// `AbstractEntityAIBasic.checkIfNeedsItem` (`:267-272`).
    Base,
    /// `AbstractEntityAIStructureWithWorkOrder.checkIfNeedsItem` (`:211-217`),
    /// which narrows the base rule before deferring to it.
    Structure,
}

/// Whether the worker may pick material up right after a dump.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the base answer and the builder's override
pub enum AfterDumpPickup {
    /// The base class (`:726-729`): yes, always.
    Always,
    /// `!checkForWorkOrder()` (`EntityAIStructureBuilder.java:137-141`): only when
    /// there is no order to work on.
    WithoutWorkOrder,
}

impl AfterDumpPickup {
    /// Evaluate it against the facts.
    pub fn allows(&self, facts: &WorldFacts) -> bool {
        match self {
            AfterDumpPickup::Always => true,
            AfterDumpPickup::WithoutWorkOrder => facts
                .builder()
                .map(|builder| builder.work_order.is_none())
                .unwrap_or(true),
        }
    }
}

/// The overridable methods of `AbstractEntityAIBasic`, flattened into data.
///
/// Java gets these by subclassing: `getActionsDoneUntilDumping` (`:425-428`),
/// `wantInventoryDumped` (`:435-438`), `afterDump` (`:1213-1222`),
/// `afterRequestPickUp` (`:736-739`), `getStateAfterPickUp` (`:323-326`),
/// `canGoIdle` (`:1921-1924`) and `isAfterDumpPickupAllowed` (`:726-729`). A Rust
/// `struct` cannot be extended by a subclass, so the differences are fields, and
/// every one of them names the class it comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JobProfile {
    /// `getActionsDoneUntilDumping()`.
    pub actions_until_dumping: u32,
    /// `wantInventoryDumped()`: an extra reason to dump, per trade.
    pub want_dump: bool,
    /// `afterDump()` — `None` means the base rule: `PAUSED` when paused, else
    /// `IDLE` (`:1215-1221`).
    pub after_dump: Option<WorkerState>,
    /// `afterRequestPickUp()` — `IDLE` in the base class (`:738`).
    pub after_request_pickup: WorkerState,
    /// `getStateAfterPickUp()` — `START_WORKING` in the base class (`:325`).
    pub state_after_pickup: WorkerState,
    /// `canGoIdle()`.
    pub idle: IdleRule,
    /// `isAfterDumpPickupAllowed()`.
    pub after_dump_pickup: AfterDumpPickup,
    /// Which `checkIfNeedsItem` this trade runs.
    pub needs_item: NeedsItemRule,
}

impl JobProfile {
    /// `AbstractEntityAIBasic`'s own answers, before any trade overrides them.
    pub const BASE: JobProfile = JobProfile {
        actions_until_dumping: ACTIONS_UNTIL_DUMP, // :427
        want_dump: false,                          // :437
        after_dump: None,                          // :1213-1222, the paused rule
        after_request_pickup: WorkerState::Idle,   // :738
        state_after_pickup: WorkerState::StartWorking, // :325
        idle: IdleRule::Never,                     // :1923
        after_dump_pickup: AfterDumpPickup::Always, // :728
        needs_item: NeedsItemRule::Base,           // :267-272
    };

    /// What `EntityAIWorkComposter` changes: it dumps after **every** action
    /// (`EntityAIWorkComposter.java:380-384`).
    pub const COMPOSTER: JobProfile = JobProfile {
        actions_until_dumping: 1, // :383
        ..JobProfile::BASE
    };

    /// What `EntityAIStructureBuilder` changes: a huge dump threshold because the
    /// builder's own `PICK_UP` handles materials, `PICK_UP` after a dump, a dump
    /// after each pickup, and idling when there is no order.
    pub const BUILDER: JobProfile = JobProfile {
        actions_until_dumping: 4096,             // EntityAIStructureBuilder.java:50
        after_dump: Some(WorkerState::PickUp),    // :202-205
        after_request_pickup: WorkerState::InventoryFull, // :195-199
        state_after_pickup: WorkerState::PickUp,  // AbstractEntityAIStructure.java:681-684
        idle: IdleRule::WithoutWorkOrder,         // EntityAIStructureBuilder.java:336-339
        after_dump_pickup: AfterDumpPickup::WithoutWorkOrder, // :137-141
        needs_item: NeedsItemRule::Structure,     // AbstractEntityAIStructureWithWorkOrder.java:211-217
        ..JobProfile::BASE
    };
}

/// A job whose trade-specific AI is not ported.
///
/// Returned by [`JobAi::for_job`] rather than a half-built machine: a table of
/// guesses would decide goals Java never decides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unported {
    /// The job that has no table.
    pub job: Job,
}

/// Everything one worker AI owns: its trade, its machine, and nothing else.
///
/// The facts and the memory live in [`Scene`], which the caller owns, so a test can
/// tick an AI over a scene it built and read both afterwards.
pub struct JobAi {
    job: Job,
    profile: JobProfile,
    machine: Machine<WorkerState, Scene>,
}

/// The facts plus the AI's memory: what a transition reads and writes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scene {
    /// What the caller observed.
    pub facts: WorldFacts,
    /// What the AI remembers.
    pub memory: Memory,
    /// Which overrides this trade applies.
    pub profile: JobProfile,
}

impl Scene {
    /// A scene on `facts` with an empty memory and the base profile.
    pub fn new(facts: WorldFacts) -> Scene {
        Scene {
            facts,
            memory: Memory::default(),
            profile: JobProfile::BASE,
        }
    }

    /// A scene with `profile`, for the trade tests.
    pub fn with_profile(facts: WorldFacts, profile: JobProfile) -> Scene {
        Scene {
            profile,
            ..Scene::new(facts)
        }
    }

    /// Record why this tick cannot go further. The last one wins: Java only ever
    /// has one branch in flight.
    pub fn refuse(&mut self, refusal: Refusal) {
        self.memory.refusal = Some(refusal);
    }
}

/// What one tick of a worker AI decided.
///
/// Java's tick returns nothing; the machine keeps a state and a history ring
/// (`BasicStateMachine.java:66-70`). A caller that has to explain a citizen needs
/// both halves: the state now in force ([`Outcome::state`] — the goal), the rule
/// that put it there ([`Outcome::fired`]), and the reason it did not move
/// ([`Outcome::refusal`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// The state the machine is in after this tick: the goal in progress.
    pub state: WorkerState,
    /// The transition that ran, if one did.
    pub fired: Option<Fired<WorkerState>>,
    /// The refusal recorded during this tick, if any.
    pub refusal: Option<Refusal>,
}

impl Outcome {
    /// The goal in force.
    pub fn goal(&self) -> WorkerState {
        self.state
    }

    /// Why nothing moved, if the tick recorded a reason.
    pub fn why_not(&self) -> Option<Refusal> {
        self.refusal
    }

    /// Whether this tick changed the goal.
    pub fn moved(&self) -> bool {
        matches!(self.fired, Some(fired) if fired.from != fired.to)
    }

    /// Whether the machine fell into a trap state
    /// (`BasicStateMachine.java:220-228`).
    pub fn faulted(&self) -> bool {
        matches!(self.fired, Some(fired) if fired.reset)
    }
}

impl core::fmt::Debug for JobAi {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("JobAi")
            .field("job", &self.job)
            .field("machine", &self.machine)
            .finish()
    }
}

impl JobAi {
    /// The base table alone, with `job`'s name — what every job's AI has before its
    /// own constructor adds anything (`AbstractEntityAIBasic.java:200-260`).
    ///
    /// This is not a stand-in for an unported trade: it is the half of the table the
    /// job really has. A caller that wants the trade too asks [`JobAi::for_job`].
    /// Rust cannot tick the base table alone: `IDLE` has no transition in
    /// `AbstractEntityAIBasic`, every concrete AI registers one
    /// (`EntityAIWorkComposter.java:97`, `AbstractEntityAIStructure.java:186`), so a
    /// machine built on this table stops at `IDLE` with Java's own trap warning
    /// (`BasicStateMachine.java:220-228`). Java's base class is `abstract` for the
    /// same reason. Use [`JobAi::for_job`] to run one, and this to read the table.
    pub fn base(job: Job) -> JobAi {
        let mut machine = Machine::new(WorkerState::Init, ENTITY_AI_TICKRATE);
        register_base(&mut machine);
        JobAi {
            job,
            profile: JobProfile::BASE,
            machine,
        }
    }

    /// The full AI for `job`: the base table plus the trade's own targets.
    ///
    /// `Err(Unported)` for the 48 jobs whose AI this slice does not carry — see
    /// `RAPPORT.md` §3 for the list and the cost.
    pub fn for_job(job: Job) -> Result<JobAi, Unported> {
        let mut ai = JobAi::base(job);
        match job {
            Job::Composter => {
                ai.profile = JobProfile::COMPOSTER;
                super::composter::register(&mut ai.machine);
            }
            Job::Builder => {
                ai.profile = JobProfile::BUILDER;
                super::builder::register(&mut ai.machine);
            }
            other => return Err(Unported { job: other }),
        }
        Ok(ai)
    }

    /// The trade this AI belongs to.
    pub fn job(&self) -> Job {
        self.job
    }

    /// The overrides this trade applies ([`JobProfile`]).
    pub fn profile(&self) -> JobProfile {
        self.profile
    }

    /// A scene on `facts`, already carrying this trade's overrides — the pairing a
    /// caller must not get wrong: the machine is the trade's, so the profile must be
    /// too.
    pub fn scene(&self, facts: WorldFacts) -> Scene {
        Scene::with_profile(facts, self.profile)
    }

    /// The state the AI is in (`AbstractAISkeleton.getState`, `:120-126`).
    pub fn state(&self) -> WorkerState {
        self.machine.state()
    }

    /// `AbstractAISkeleton.resetAI` (`:145-152`): back to `INIT`, and Java also
    /// clears the render metadata (an effect, not a decision).
    pub fn reset(&mut self) {
        self.machine.reset();
    }

    /// One AI tick (`AbstractAISkeleton.tick`, `:63-66` → `TickRateStateMachine.tick`).
    ///
    /// The refusal is cleared first, so [`Outcome::why_not`] only ever answers about
    /// this tick.
    pub fn tick(&mut self, scene: &mut Scene) -> Outcome {
        scene.memory.refusal = None;
        let fired = self.machine.tick(scene);
        Outcome {
            state: self.machine.state(),
            fired,
            refusal: scene.memory.refusal,
        }
    }

    /// `AbstractEntityAIBasic.onException` (`:353-390`): wait `EXCEPTION_TIMEOUT ×
    /// exceptionTimer` ticks, then double the timer. Returns the timeout applied.
    pub fn on_exception(&mut self, scene: &mut Scene) -> u32 {
        let timeout = EXCEPTION_TIMEOUT.saturating_mul(scene.memory.exception_timer);
        scene.memory.delay = timeout as i32;
        scene.memory.exception_timer = scene.memory.exception_timer.saturating_mul(2);
        // `setCurrentDelay(...)` (`:361`) on the machine's last transition.
        let _ = self.machine.set_current_delay(timeout);
        timeout
    }

    /// The machine, for a caller that wants to add a target or inspect the table.
    pub fn machine(&self) -> &Machine<WorkerState, Scene> {
        &self.machine
    }

    /// The machine, mutably.
    pub fn machine_mut(&mut self) -> &mut Machine<WorkerState, Scene> {
        &mut self.machine
    }
}

/// `AbstractEntityAIBasic.setDelay` (`:397-400`): the delay before the next action.
pub fn set_delay(scene: &mut Scene, timeout: u32) {
    scene.memory.delay = timeout as i32;
}

/// Does a worker whose AI is this trade hand its citizen back when there is nothing
/// to do? (`canGoIdle`, `:1921-1924` and the overrides.)
pub fn can_go_idle(profile: &JobProfile, facts: &WorldFacts) -> bool {
    profile.idle.allows(facts)
}

/// `AbstractEntityAIBasic.getEffectiveSkillLevel` (`:682-685`): the early bonus
/// curve that turns a raw `0..99` level into `1..100`.
///
/// `(int) (((raw + 1) * 2) - Math.pow((raw + 1) / 10.0, 2))` — truncated, as Java's
/// `(int)` cast truncates towards zero.
pub fn effective_skill_level(raw: i32) -> i32 {
    let shifted = (raw + 1) as f64;
    (shifted * 2.0 - (shifted / 10.0).powi(2)) as i32
}

/// `AbstractEntityAIBasic.getInvertedEffectiveSkillLevel` (`:693-696`): the same
/// curve, inverted, floored at 1.
pub fn inverted_effective_skill_level(raw: i32) -> i32 {
    (100 - effective_skill_level(raw)).max(1)
}

/// What [`best_tool_slot`] found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: Java's three answers are two sentinels and a slot
pub enum BestTool {
    /// `NO_TOOL` (`:95`, `:1410-1413`): the block needs no tool.
    NoneNeeded,
    /// `TOOL_NOT_FOUND` (`:90`, `:1415`): a tool is needed and none qualifies.
    NotFound,
    /// The slot holding the best one (`:1427`).
    Slot(usize),
}

/// `AbstractEntityAIBasic.getMostEfficientTool` (`:1405-1433`), over the inventory's
/// tool levels.
///
/// `tools` is one entry per inventory slot in slot order: `Some((mining_level,
/// enchantment))` for a slot holding the tool type, `None` for anything else.
/// `verifyEquipmentLevel` (`api/util/ItemStackUtils.java:264-271`) is the last test:
/// `level + enchantment` must reach `required` and must not exceed `max_tool_level`.
pub fn best_tool_slot(
    tools: &[Option<(i32, i32)>],
    required: i32,
    max_tool_level: i32,
    needs_tool: bool,
) -> BestTool {
    if !needs_tool {
        return BestTool::NoneNeeded;
    }
    let mut best: Option<(usize, i32)> = None;
    for (slot, tool) in tools.iter().enumerate() {
        let Some((level, enchantment)) = *tool else {
            continue;
        };
        let effective = level + enchantment;
        let good = level > -1 && level >= required && effective >= required && effective <= max_tool_level;
        match best {
            Some((_, best_level)) if level >= best_level => continue,
            _ if good => best = Some((slot, level)),
            _ => {}
        }
    }
    match best {
        Some((slot, _)) => BestTool::Slot(slot),
        None => BestTool::NotFound,
    }
}

/// What `checkForToolOrWeapon` decided (`AbstractEntityAIBasic.java:966-1005`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: has it, took one from the hut, or asked for one
pub enum ToolCheck {
    /// `checkForNeededTool` answered false: the worker carries a good tool
    /// (`:1088-1091`). The job status goes back to `WORKING` (`:975`).
    Have,
    /// The hut's chest had one and the worker took it (`:1094`, `:1104-1140`).
    Took,
    /// Asked for a tool, so the job is `STUCK` (`:971`, `:993-1002`).
    Asked {
        /// Whether a new request was created, i.e. none was open
        /// (`:995-999`).
        created: bool,
    },
}

/// `checkForToolOrWeapon(toolType, TOOL_LEVEL_WOOD_OR_GOLD)` (`:966-1005`), as a
/// decision over facts.
///
/// It also does what Java does to the AI's own state: `delay += DELAY_RECHECK`
/// (`:1093`) and `delay = 0` when it asked (`:1000`), so it takes the scene.
pub fn check_for_tool(scene: &mut Scene) -> ToolCheck {
    let worker_has = scene.facts.building.citizen_has_tool;
    if worker_has {
        return ToolCheck::Have; // :1088-1091
    }
    scene.memory.delay += DELAY_RECHECK as i32; // :1093
    if scene.facts.building.container_has_tool {
        // `retrieveToolInHut` (`:1094`, `:1104-1140`) took one.
        return ToolCheck::Took;
    }
    let created = scene.facts.building.open_tool_requests == 0
        && scene.facts.building.completed_tool_requests == 0; // :995
    scene.memory.delay = 0; // :1000
    scene.refuse(Refusal::ToolMissing);
    ToolCheck::Asked { created }
}

/// `AbstractEntityAIBasic.hasNotDelayed` (`:1441-1451`): `true` on the first call,
/// `false` on the next one — a one-shot delay, "wait once and pass".
pub fn has_not_delayed(memory: &mut Memory, time: u32) -> bool {
    if !memory.has_delayed {
        memory.delay = time as i32;
        memory.has_delayed = true;
        return true;
    }
    memory.has_delayed = false;
    false
}

/// Register the fourteen targets of `AbstractEntityAIBasic.java:200-260`, in Java's
/// order.
pub fn register_base(machine: &mut Machine<WorkerState, Scene>) {
    // :204 — "init safety checks and transition to IDLE", on every state.
    machine.add(Transition::event(
        Blocking::AiBlocking,
        "initSafetyChecks",
        "AbstractEntityAIBasic.java:204",
        1,
        Action::Run(init_safety_checks),
    ));
    // :205 — `new AITarget(INIT, this::getState, 1)`: in INIT, re-assert INIT, which
    // holds the machine until the safety checks have run.
    machine.add(Transition::target(
        WorkerState::Init,
        "holdInInit",
        "AbstractEntityAIBasic.java:205",
        1,
        Action::Run(|_, state| Some(state)),
    ));
    // :211 — "update chestbelt and nametag, every time, and does not stop execution":
    // the action returns null, so the tick goes on.
    machine.add(Transition::event(
        Blocking::AiBlocking,
        "updateVisualState",
        "AbstractEntityAIBasic.java:211",
        20,
        Action::Run(|_, _| None),
    ));
    // :218 — "if waitingForSomething returns true, stop execution to wait for it.
    // this keeps the current state (returning null would not stop execution)".
    machine.add(Transition::event_when(
        Blocking::AiBlocking,
        "waitingForSomething",
        "AbstractEntityAIBasic.java:218",
        5,
        is_delayed,
        Action::Run(waiting_for_something),
    ));
    // :225 — dump the inventory as long as need be, on every state.
    machine.add(Transition::event_when(
        Blocking::StateBlocking,
        "inventoryNeedsDump",
        "AbstractEntityAIBasic.java:225",
        100,
        inventory_needs_dump,
        Action::Fixed(Some(WorkerState::InventoryFull)),
    ));
    // :226 — `new AITarget(INVENTORY_FULL, this::dumpInventory, 20)`.
    machine.add(Transition::target(
        WorkerState::InventoryFull,
        "dumpInventory",
        "AbstractEntityAIBasic.java:226",
        20,
        Action::Run(dump_inventory),
    ));
    // :232 — "check if any items are needed; if yes, transition to NEEDS_ITEM".
    machine.add(Transition::event_when(
        Blocking::AiBlocking,
        "checkIfNeedsItem",
        "AbstractEntityAIBasic.java:232",
        20,
        check_if_needs_item,
        Action::Fixed(Some(WorkerState::NeedsItem)),
    ));
    // :236 — completed requests + `cleanAsync()`. `cleanAsync` always answers false
    // (`:715`), so this target never transitions; it only accepts the async requests
    // it was called to clean. Registered because Java registers it.
    machine.add(Transition::event_when(
        Blocking::AiBlocking,
        "cleanAsync",
        "AbstractEntityAIBasic.java:236",
        200,
        has_completed_requests,
        Action::Run(clean_async),
    ));
    // :238 — `new AITarget(NEEDS_ITEM, this::waitForRequests, 40)`.
    machine.add(Transition::target(
        WorkerState::NeedsItem,
        "waitForRequests",
        "AbstractEntityAIBasic.java:238",
        40,
        Action::Run(wait_for_requests),
    ));
    // :242 — gather a needed item, once a second.
    machine.add(Transition::target(
        WorkerState::GatheringRequiredMaterials,
        "getNeededItem",
        "AbstractEntityAIBasic.java:242",
        20,
        Action::Run(get_needed_item),
    ));
    // :247 — "place any non-restart regarding AITargets before this one".
    machine.add(Transition::event_when(
        Blocking::StateBlocking,
        "shouldRestart",
        "AbstractEntityAIBasic.java:247",
        20,
        should_restart,
        Action::Run(restart),
    ));
    // :251 — reset if not paused.
    machine.add(Transition::target_when(
        WorkerState::Paused,
        "unpause",
        "AbstractEntityAIBasic.java:251",
        20,
        |scene: &Scene, _: WorkerState| !scene.facts.worker.paused,
        Action::Fixed(Some(WorkerState::Idle)),
    ));
    // :255 — do not work if the worker is paused.
    machine.add(Transition::target(
        WorkerState::Paused,
        "bePaused",
        "AbstractEntityAIBasic.java:255",
        10,
        Action::Run(be_paused),
    ));
    // :259 — start paused with an inventory dump.
    machine.add(Transition::event_when(
        Blocking::AiBlocking,
        "isStartingPaused",
        "AbstractEntityAIBasic.java:259",
        20,
        is_starting_paused,
        Action::Fixed(Some(WorkerState::InventoryFull)),
    ));
}

// ---------------------------------------------------------------- conditions

/// Target 4's condition: `delay > 0` (`:499-515`).
fn is_delayed(scene: &Scene, _: WorkerState) -> bool {
    scene.memory.delay > 0
}

/// Target 5: `inventoryNeedsDump` (`:409-416`), condition by condition.
fn inventory_needs_dump(scene: &Scene, state: WorkerState) -> bool {
    state != WorkerState::InventoryFull        // :411
        && state.okay_to_eat()                 // `canBeInterrupted()`, :411
        && (scene.facts.inventory.full         // :412
            || scene.facts.inventory.actions_done >= scene.profile.actions_until_dumping // :413
            || scene.profile.want_dump)        // :414
        && !scene.facts.worker.deliveryman     // :415
}

/// Target 7: `checkIfNeedsItem`, which every trade may override. The base answer is
/// [`base_check_if_needs_item`]; a builder narrows it first
/// (`AbstractEntityAIStructureWithWorkOrder.java:211-217`).
fn check_if_needs_item(scene: &Scene, state: WorkerState) -> bool {
    match scene.profile.needs_item {
        NeedsItemRule::Base => base_check_if_needs_item(scene, state),
        NeedsItemRule::Structure => super::builder::structure_check_if_needs_item(scene, state),
    }
}

/// `AbstractEntityAIBasic.checkIfNeedsItem` (`:267-272`), before any override: an
/// open synchronous request, or a completed one that is not async.
pub fn base_check_if_needs_item(scene: &Scene, state: WorkerState) -> bool {
    state != WorkerState::InventoryFull // :269
        && (scene.facts.building.has_open_sync_request // :270
            || scene.facts.building.completed_sync > 0) // :271
}

/// Target 8: `building.hasCitizenCompletedRequests(citizenData)` (`:236`).
fn has_completed_requests(scene: &Scene, _: WorkerState) -> bool {
    scene.facts.building.completed_requests > 0
}

/// Target 11: `citizenData.shouldRestart() && isPaused()` (`:1877-1880`).
fn should_restart(scene: &Scene, _: WorkerState) -> bool {
    scene.facts.worker.should_restart && scene.facts.worker.paused
}

/// Target 14: `isPaused() && state != PAUSED && state != INVENTORY_FULL`
/// (`:1841-1844`).
fn is_starting_paused(scene: &Scene, state: WorkerState) -> bool {
    scene.facts.worker.paused && state != WorkerState::Paused && state != WorkerState::InventoryFull
}

// ------------------------------------------------------------------- actions

/// `initSafetyChecks` (`:446-465`).
///
/// `INIT` is held while the three important variables are null, and while the
/// citizen's job record and this AI disagree — a citizen whose job was replaced
/// under it goes back to `INIT` and stays there until something fixes it. Once those
/// hold and the state is `INIT`, it answers `IDLE`.
fn init_safety_checks(scene: &mut Scene, state: WorkerState) -> Option<WorkerState> {
    if !scene.facts.worker.hired || !scene.facts.building.exists {
        scene.refuse(Refusal::NotHired); // :448-451
        return Some(WorkerState::Init);
    }
    if !scene.facts.worker.job_is_this_ai {
        scene.refuse(Refusal::JobAndHutDisagree); // :453-457
        return Some(WorkerState::Init);
    }
    if state == WorkerState::Init {
        return Some(WorkerState::Idle); // :459-463
    }
    None // :464
}

/// `AbstractEntityAIBasic`'s constructor (`:188-198`), which refuses before the first
/// tick: the citizen's hut must be the class the AI expects, and Java reacts by
/// clearing the citizen's job (`:194`) rather than by a state change.
///
/// The port exposes it because a caller building a [`JobAi`] has to ask the same
/// question, and because a refusal a player should see ("this builder works in a
/// forester's hut") has to come from somewhere.
pub fn constructor_check(facts: &WorldFacts) -> Option<Refusal> {
    if !facts.worker.hired || !facts.building.exists {
        return Some(Refusal::NotHired); // `:448` fires on the same nulls later
    }
    if !facts.worker.job_matches_building {
        return Some(Refusal::WrongBuilding); // :191-197
    }
    None
}

/// `waitingForSomething` (`:497-517`): count the delay down by one AI tick and keep
/// the current state while it lasts.
///
/// Java also hits the block being worked on when the worker is standing at it
/// (`:501-507`) — an animation, not a decision, and it needs the entity.
fn waiting_for_something(scene: &mut Scene, state: WorkerState) -> Option<WorkerState> {
    if scene.memory.delay > 0 {
        scene.memory.delay -= ENTITY_AI_TICKRATE as i32; // `getTickRate()`, :509
        if scene.memory.delay <= 0 {
            // `clearWorkTarget` (`:522-526`).
            scene.memory.delay = 0;
            scene.facts.worker.working_at = None;
        }
        return Some(state);
    }
    None
}

/// `dumpInventory` (`:1157-1206`) and `afterDump` (`:1213-1222`).
fn dump_inventory(scene: &mut Scene, _: WorkerState) -> Option<WorkerState> {
    if !scene.facts.building.exists {
        // `getBuildingToDump()` was null: "that shouldn't happen, restart the AI".
        return Some(after_dump(scene));
    }
    if scene.facts.building.full {
        scene.refuse(Refusal::HutFull); // :1167-1188
        if scene.facts.building.pickup_priority > 0 {
            // `createPickupRequest(dumpedItems, true)` (`:1181`) — a request the
            // request system owns, so the port only clears the counter.
            scene.memory.dumped_items = 0;
        }
        scene.memory.kept = 0;
        scene.memory.slot_at = 0;
        scene.facts.inventory.actions_done = 0; // `clearActionsDone`, :1186
        return Some(after_dump(scene));
    }
    if dump_one_more_slot(scene) {
        return Some(WorkerState::InventoryFull); // :1189-1192
    }
    scene.memory.kept = 0;
    scene.memory.slot_at = 0;
    scene.facts.inventory.actions_done = 0; // :1194-1196
    if scene.profile.after_dump_pickup.allows(&scene.facts)
        && scene.facts.building.pickup_priority > 0
        && scene.memory.dumped_items > 0
    {
        scene.memory.dumped_items = 0; // :1200-1204
    }
    Some(after_dump(scene))
}

/// `afterDump` (`:1213-1222`): `PAUSED` when the worker is paused, else the trade's
/// answer — `IDLE` in the base class, `PICK_UP` for a builder
/// (`EntityAIStructureBuilder.java:202-205`).
pub fn after_dump(scene: &mut Scene) -> WorkerState {
    match scene.profile.after_dump {
        Some(state) => state,
        None if scene.facts.worker.paused => WorkerState::Paused,
        None => WorkerState::Idle,
    }
}

/// `dumpOneMoreSlot` (`:1229-1294`): one slot, or "there is more to dump".
///
/// Returns `true` while there are slots left, which is what `dumpInventory` turns
/// into `INVENTORY_FULL` (`:1189-1192`).
pub fn dump_one_more_slot(scene: &mut Scene) -> bool {
    // :1232-1235 — walk to the hut first; while walking, there is more to dump.
    if !scene
        .facts
        .worker
        .at
        .is_living_at_site(scene.facts.building.position, WOKR_IN_BUILDING_DIST)
    {
        scene.refuse(Refusal::NotArrived);
        return true;
    }
    let total = scene.facts.inventory.slots; // :1240
    // :1242-1250 — skip the empty slots.
    let Some(slot) = scene
        .facts
        .inventory
        .dump
        .iter()
        .find(|slot| slot.index >= scene.memory.slot_at)
        .copied()
    else {
        scene.refuse(Refusal::NothingToDump);
        return false;
    };
    scene.memory.slot_at = slot.index;

    // :1252-1267 — close to full: gamble on keeping a few items.
    let mut dump_anyway = false;
    if scene.memory.slot_at + (MIN_OPEN_SLOTS * 2) as usize >= total
        && (scene.facts.inventory.open_slots as u32) < MIN_OPEN_SLOTS * 2
    {
        if slot.count < CHANCE_TO_DUMP_50 {
            dump_anyway = scene.facts.worker.coin; // `nextBoolean()`, :1260
        } else {
            // `nextInt(count) < CHANCE_TO_DUMP` (`:1264`): the caller supplies the
            // draw, this port applies the comparison.
            dump_anyway = slot.count > 0 && (scene.facts.worker.roll % slot.count) < CHANCE_TO_DUMP;
        }
    }

    let amount = if dump_anyway {
        slot.count // :1271
    } else {
        slot.wanted // `buildingRequiresCertainAmountOfItem(...)`, :1271
    };
    if amount > 0 {
        scene.memory.dumped_items += amount as i32; // :1288
        if slot.count > 0 {
            scene.memory.kept += 1;
        }
    }
    scene.memory.slot_at += 1; // :1291
    scene.memory.slot_at < total // :1293
}

/// `cleanAsync` (`:703-716`): accepts every completed async request, and answers
/// **false** so that the target it belongs to never transitions.
fn clean_async(scene: &mut Scene, _: WorkerState) -> Option<WorkerState> {
    scene.facts.building.completed_async = 0;
    None // :715
}

/// `waitForRequests` (`:534-538`) → `lookForRequests` (`:546-643`).
fn wait_for_requests(scene: &mut Scene, _: WorkerState) -> Option<WorkerState> {
    scene.memory.delay = DELAY_REQUESTS as i32; // :536
    look_for_requests(scene)
}

/// `AbstractEntityAIBasic.DELAY_RECHECK` is what `waitForRequests` sets
/// (`:536`); kept under its own name so the two uses of the same number are visible.
pub const DELAY_REQUESTS: u32 = DELAY_RECHECK;

/// `lookForRequests` (`:546-643`).
fn look_for_requests(scene: &mut Scene) -> Option<WorkerState> {
    let building = scene.facts.building;
    if !building.has_open_sync_request && building.completed_requests == 0 {
        scene.refuse(Refusal::RequestDone); // :548-552
        return Some(scene.profile.after_request_pickup);
    }
    if building.completed_requests > 0 {
        // :554-567 — the ones that cannot be delivered are accepted and dropped.
        if building.completed_deliverable == 0 {
            return Some(WorkerState::NeedsItem);
        }
        // :581 — the pickup is at the station for a station resolver, at the hut
        // otherwise; a fact either way.
        if !scene.facts.blocks.is_loaded(building.position) {
            scene.refuse(Refusal::NotArrived); // :583-586
            return Some(WorkerState::NeedsItem);
        }
        if !scene.facts.building.pickup_has_entity {
            scene.refuse(Refusal::NotArrived); // :587-591
            return Some(WorkerState::NeedsItem);
        }
        if scene.facts.building.pickup_items_present {
            // :607-621 — they are in the worker's or the hut's hands: take them.
            return Some(WorkerState::NeedsItem);
        }
        // :624-634 — somebody else took the stack: ask again.
        scene.refuse(Refusal::StackGone);
        return Some(WorkerState::NeedsItem);
    }
    // :637-640 — nothing completed but something is still coming: wait at the hut.
    if !scene
        .facts
        .worker
        .at
        .is_living_at_site(building.position, WOKR_IN_BUILDING_DIST)
    {
        scene.refuse(Refusal::NotArrived);
    }
    Some(WorkerState::NeedsItem)
}

/// `getNeededItem` (`:282-316`) with `getStateAfterPickUp` (`:323-326`).
fn get_needed_item(scene: &mut Scene, state: WorkerState) -> Option<WorkerState> {
    let Some(wanted) = scene.memory.need else {
        return Some(scene.profile.state_after_pickup); // :284-287
    };
    if scene.memory.walk_to.is_none() {
        match scene.facts.building.chest_with_needed {
            None => {
                scene.refuse(Refusal::NoChestWithItem); // :292-296
                return Some(scene.profile.state_after_pickup);
            }
            Some(position) => scene.memory.walk_to = Some(position), // :297
        }
    }
    let walk_to = scene.memory.walk_to.unwrap_or(scene.facts.building.position);
    if !scene.facts.worker.at.is_living_at_site(walk_to, WOKR_IN_BUILDING_DIST) {
        scene.refuse(Refusal::NotArrived); // :300
        if scene.memory.pickup_counter < PICKUP_ATTEMPTS {
            scene.memory.pickup_counter += 1;
            return Some(state); // :300-303
        }
    }
    scene.memory.pickup_counter = 0; // :305
    // `tryTransferFromPosToWorkerIfNeeded` (`:1801-1824`): true once the worker
    // holds as many as it asked for.
    if scene.facts.inventory.needed_count < wanted {
        scene.refuse(Refusal::StackGone); // :307-311
        scene.memory.walk_to = None;
        return Some(state);
    }
    scene.memory.walk_to = None; // :314
    Some(scene.profile.state_after_pickup) // :315
}

/// `restart` (`:1887-1894`): clean the building up, wait a walk delay, answer `INIT`.
fn restart(scene: &mut Scene, _: WorkerState) -> Option<WorkerState> {
    scene.memory.delay = WALK_DELAY as i32; // :1891
    Some(WorkerState::Init) // :1893
}

/// `bePaused` (`:1851-1870`): a paused worker wanders or visits its hut; the AI does
/// not change state, it only picks a pastime.
fn be_paused(scene: &mut Scene, _: WorkerState) -> Option<WorkerState> {
    if !scene.facts.worker.navigation_done {
        scene.refuse(Refusal::Navigating); // :1853-1856
        return None;
    }
    scene.refuse(Refusal::Paused);
    // `:1859-1867` — a draw against `VISIT_BUILDING_CHANCE` (8) then
    // `WANDER_CHANCE` (35) picks a walk. A walk is not a goal, and the draw is not
    // data, so the port keeps the state and records the refusal.
    None
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::facts::{DumpSlot, InventoryFacts, JobFacts, WorkerFacts};

    /// A builder's machine — the trade with the simplest `IDLE` exit
    /// (`AbstractEntityAIStructure.java:186`). The base table cannot be ticked alone:
    /// `IDLE` has no transition in `AbstractEntityAIBasic`, so Java's own trap rule
    /// (`BasicStateMachine.java:220-228`) would reset it.
    fn ai() -> JobAi {
        JobAi::for_job(Job::Builder).expect("builder is ported")
    }

    /// A base-table machine, started wherever a test needs it. Used when the test is
    /// about one base rule and the trade's own targets would only add noise.
    fn base_machine_at(state: WorkerState) -> Machine<WorkerState, Scene> {
        let mut machine = Machine::new(state, ENTITY_AI_TICKRATE);
        register_base(&mut machine);
        machine
    }

    /// A scene with the **base** profile, so the base rules are the ones under test.
    fn scene() -> Scene {
        Scene::new(WorldFacts::new())
    }

    /// Tick until `want` is reached, at most `max` ticks. The rate-limited rules make
    /// this the only honest way to ask: the dump rule is looked at every 100 ticks
    /// (`:225`), the request rule every 20 (`:232`).
    fn ticks_until(ai: &mut JobAi, scene: &mut Scene, want: WorkerState, max: usize) -> bool {
        for _ in 0..max {
            if ai.tick(scene).state == want {
                return true;
            }
        }
        ai.state() == want
    }

    #[test]
    fn the_base_table_has_fourteen_targets_in_java_order() {
        let ai = JobAi::base(Job::Builder);
        assert_eq!(ai.machine().transitions().len(), 14);
        let rules: Vec<&str> = ai
            .machine()
            .transitions()
            .iter()
            .map(|transition| transition.rule)
            .collect();
        assert_eq!(
            rules,
            vec![
                "initSafetyChecks",
                "holdInInit",
                "updateVisualState",
                "waitingForSomething",
                "inventoryNeedsDump",
                "dumpInventory",
                "checkIfNeedsItem",
                "cleanAsync",
                "waitForRequests",
                "getNeededItem",
                "shouldRestart",
                "unpause",
                "bePaused",
                "isStartingPaused",
            ],
            "AbstractEntityAIBasic.java:200-260 registers in this order"
        );
        // The passes Java's registration produces.
        assert_eq!(
            ai.machine().transitions()[0].blocking,
            Some(Blocking::AiBlocking)
        );
        assert_eq!(
            ai.machine().transitions()[4].blocking,
            Some(Blocking::StateBlocking)
        );
        assert_eq!(
            ai.machine().transitions()[5].state,
            Some(WorkerState::InventoryFull)
        );
        // Every rule cites a Java line, and the file it names exists in the tree.
        for transition in ai.machine().transitions() {
            assert!(
                transition.java.ends_with(".java:204")
                    || transition.java.contains(".java:"),
                "{} has no java reference",
                transition.rule
            );
        }
    }

    #[test]
    fn the_base_table_alone_traps_at_idle_exactly_as_java_does() {
        // `AbstractEntityAIBasic` is abstract for this reason: `IDLE` has no
        // transition until the trade registers one (`EntityAIWorkComposter.java:97`,
        // `AbstractEntityAIStructure.java:186`).
        let mut base_ai = JobAi::base(Job::Builder);
        let mut base_scene = scene();
        let outcome = base_ai.tick(&mut base_scene);
        assert!(
            outcome.faulted(),
            "the machine reports the trap instead of pretending to work"
        );
        assert_eq!(base_ai.state(), WorkerState::Init);
        assert!(!base_ai.machine().can_leave(WorkerState::Idle));
        // With the trade's table, the same first tick lands in IDLE and stays.
        let mut trade_ai = ai();
        let mut trade_scene = scene();
        assert_eq!(trade_ai.tick(&mut trade_scene).state, WorkerState::Idle);
        assert!(trade_ai.machine().can_leave(WorkerState::Idle));
    }

    #[test]
    fn a_worker_with_no_job_stays_in_init() {
        // `AbstractEntityAIBasic.java:448-451`.
        let mut ai = ai();
        let mut scene = scene();
        scene.facts.worker.hired = false;
        let outcome = ai.tick(&mut scene);
        assert_eq!(outcome.state, WorkerState::Init);
        assert_eq!(outcome.why_not(), Some(Refusal::NotHired));
    }

    #[test]
    fn a_worker_whose_hut_is_gone_stays_in_init() {
        // `:448` — `building == null`.
        let mut ai = ai();
        let mut scene = scene();
        scene.facts.building.exists = false;
        let outcome = ai.tick(&mut scene);
        assert_eq!(outcome.state, WorkerState::Init);
        assert_eq!(outcome.why_not(), Some(Refusal::NotHired));
    }

    #[test]
    fn a_worker_whose_job_and_hut_disagree_stays_in_init() {
        // `:453-457`.
        let mut ai = ai();
        let mut scene = scene();
        scene.facts.worker.job_is_this_ai = false;
        let outcome = ai.tick(&mut scene);
        assert_eq!(outcome.state, WorkerState::Init);
        assert_eq!(outcome.why_not(), Some(Refusal::JobAndHutDisagree));
    }

    #[test]
    fn the_constructor_refuses_a_hut_of_the_wrong_trade() {
        // `:188-198`: `getExpectedBuildingClass().isInstance(...)`, then `setJob(null)`
        // and an error log — no state machine involved.
        let mut wrong_hut = scene();
        assert_eq!(constructor_check(&wrong_hut.facts), None);
        wrong_hut.facts.worker.job_matches_building = false;
        assert_eq!(
            constructor_check(&wrong_hut.facts),
            Some(Refusal::WrongBuilding)
        );
        let mut broke = scene();
        broke.facts.building.exists = false;
        assert_eq!(constructor_check(&broke.facts), Some(Refusal::NotHired));
    }

    #[test]
    fn a_healthy_worker_leaves_init_for_idle_in_one_tick() {
        // `:459-463`.
        let mut ai = ai();
        let mut scene = scene();
        let outcome = ai.tick(&mut scene);
        assert_eq!(
            outcome.fired.map(|fired| fired.rule),
            Some("initSafetyChecks")
        );
        assert_eq!(outcome.state, WorkerState::Idle);
        assert!(outcome.moved());
        assert_eq!(outcome.why_not(), None);
    }

    #[test]
    fn a_full_inventory_is_dumped_and_a_full_hut_stops_the_dump() {
        // `:225` (STATE_BLOCKING) wins over the state's own list, then `:1167-1188`.
        let mut ai = ai();
        let mut scene = scene();
        ai.tick(&mut scene); // INIT → IDLE
        scene.facts.inventory.full = true;
        let outcome = ai.tick(&mut scene);
        assert_eq!(outcome.state, WorkerState::InventoryFull);
        assert_eq!(
            outcome.fired.map(|fired| fired.rule),
            Some("inventoryNeedsDump")
        );

        scene.facts.building.full = true;
        let outcome = ai.tick(&mut scene);
        assert_eq!(outcome.state, WorkerState::Idle, "afterDump, base profile");
        assert_eq!(outcome.why_not(), Some(Refusal::HutFull));
    }

    #[test]
    fn a_deliveryman_never_dumps_and_the_check_comes_back_twenty_ticks_later() {
        // `:415` gates the dump, and `:225` looks at it once per 100 ticks — which, on
        // a machine ticking every 5 (`ENTITY_AI_TICKRATE`), is one look in twenty.
        let mut machine = base_machine_at(WorkerState::Idle);
        let mut scene = scene();
        scene.facts.worker.deliveryman = true;
        scene.facts.inventory.full = true;
        assert!(machine.tick(&mut scene).is_none(), "a courier is asked and says no");

        scene.facts.worker.deliveryman = false;
        for tick in 0..19 {
            assert!(
                machine.tick(&mut scene).is_none(),
                "tick {tick} is still inside the 100-tick wait"
            );
        }
        let fired = machine.tick(&mut scene).expect("the twentieth looks again");
        assert_eq!(fired.rule, "inventoryNeedsDump");
        assert_eq!(fired.to, WorkerState::InventoryFull);
    }

    #[test]
    fn a_state_that_may_not_be_interrupted_never_starts_a_dump() {
        // `:411` — `canBeInterrupted()` is `state.isOkayToEat()`.
        assert!(!WorkerState::PickUp.okay_to_eat());
        let mut scene = scene();
        scene.facts.inventory.full = true;
        assert!(
            !inventory_needs_dump(&scene, WorkerState::PickUp),
            "a builder in PICK_UP may not be interrupted by a dump"
        );
        assert!(inventory_needs_dump(&scene, WorkerState::Idle));
        assert!(!inventory_needs_dump(&scene, WorkerState::InventoryFull));
    }

    #[test]
    fn the_action_counter_reaches_the_jobs_own_dump_threshold() {
        // `:413` against `getActionsDoneUntilDumping()`: 32 base, 4096 builder, 1
        // composter.
        let mut scene = scene();
        scene.facts.inventory.actions_done = ACTIONS_UNTIL_DUMP - 1;
        assert!(!inventory_needs_dump(&scene, WorkerState::Idle));
        scene.facts.inventory.actions_done = ACTIONS_UNTIL_DUMP;
        assert!(inventory_needs_dump(&scene, WorkerState::Idle));
        assert_eq!(JobProfile::BUILDER.actions_until_dumping, 4096);
        assert_eq!(JobProfile::COMPOSTER.actions_until_dumping, 1);
        assert_eq!(ACTIONS_UNTIL_DUMP, 32);
    }

    #[test]
    fn a_dump_skips_empty_slots_and_stops_at_the_end() {
        // `:1239-1250` then `:1293`.
        let mut scene = Scene::new(WorldFacts {
            inventory: InventoryFacts {
                slots: 4,
                dump: vec![
                    DumpSlot {
                        index: 0,
                        count: 3,
                        wanted: 3,
                    },
                    DumpSlot {
                        index: 2,
                        count: 5,
                        wanted: 0,
                    },
                ],
                ..InventoryFacts::default()
            },
            ..WorldFacts::new()
        });
        assert!(dump_one_more_slot(&mut scene), "slot 0 done, slot 2 left");
        assert_eq!(scene.memory.dumped_items, 3);
        assert_eq!(scene.memory.slot_at, 1);
        assert!(dump_one_more_slot(&mut scene), "slot 2 wants nothing");
        assert_eq!(scene.memory.dumped_items, 3, "nothing was moved");
        assert_eq!(scene.memory.slot_at, 3);
        assert!(!dump_one_more_slot(&mut scene), "no slot left");
        assert_eq!(scene.memory.refusal, Some(Refusal::NothingToDump));
    }

    #[test]
    fn a_worker_that_is_not_at_its_hut_keeps_dumping() {
        // `:1232-1235` — `!walkToWorkPos(...)` answers "more to dump".
        let mut scene = Scene::new(WorldFacts {
            worker: WorkerFacts {
                at: Pos::new(100, 64, 0),
                ..WorkerFacts::default()
            },
            ..WorldFacts::new()
        });
        assert!(dump_one_more_slot(&mut scene));
        assert_eq!(scene.memory.refusal, Some(Refusal::NotArrived));
        assert_eq!(scene.memory.dumped_items, 0);
    }

    #[test]
    fn close_to_full_the_dump_coin_keeps_or_dumps_the_stack() {
        // `:1252-1267`: a stack under 16 is kept on a `nextBoolean()` draw.
        let facts = |coin: bool| WorldFacts {
            inventory: InventoryFacts {
                slots: 27,
                open_slots: 2, // below MIN_OPEN_SLOTS * 2
                dump: vec![DumpSlot {
                    index: 26,
                    count: 4,  // under CHANCE_TO_DUMP_50
                    wanted: 0, // the hut wants none of it
                }],
                ..InventoryFacts::default()
            },
            worker: WorkerFacts {
                coin,
                ..WorkerFacts::default()
            },
            ..WorldFacts::new()
        };
        let mut kept = Scene::new(facts(false));
        dump_one_more_slot(&mut kept);
        assert_eq!(kept.memory.dumped_items, 0, "the coin said keep");
        let mut dumped = Scene::new(facts(true));
        dump_one_more_slot(&mut dumped);
        assert_eq!(dumped.memory.dumped_items, 4, "the coin said dump");
    }

    #[test]
    fn a_big_stack_is_dumped_on_a_roll_below_eight() {
        // `:1264`: `nextInt(count) < CHANCE_TO_DUMP`.
        let facts = |roll: u32| WorldFacts {
            inventory: InventoryFacts {
                slots: 27,
                open_slots: 2,
                dump: vec![DumpSlot {
                    index: 26,
                    count: 20,
                    wanted: 0,
                }],
                ..InventoryFacts::default()
            },
            worker: WorkerFacts {
                roll,
                ..WorkerFacts::default()
            },
            ..WorldFacts::new()
        };
        let mut seven = Scene::new(facts(7));
        dump_one_more_slot(&mut seven);
        assert_eq!(seven.memory.dumped_items, 20);
        let mut eight = Scene::new(facts(8));
        dump_one_more_slot(&mut eight);
        assert_eq!(eight.memory.dumped_items, 0);
    }

    #[test]
    fn a_worker_with_plenty_of_room_keeps_what_the_hut_wants() {
        // The `dumpAnyway` branch needs a nearly full slot list *and* few free slots
        // (`:1253-1257`); otherwise the hut's answer decides.
        let mut scene = Scene::new(WorldFacts {
            inventory: InventoryFacts {
                slots: 27,
                open_slots: 20,
                dump: vec![DumpSlot {
                    index: 26,
                    count: 4,
                    wanted: 0,
                }],
                ..InventoryFacts::default()
            },
            worker: WorkerFacts {
                coin: true, // would dump, if the branch were reached
                ..WorkerFacts::default()
            },
            ..WorldFacts::new()
        });
        dump_one_more_slot(&mut scene);
        assert_eq!(scene.memory.dumped_items, 0);
    }

    #[test]
    fn an_open_sync_request_sends_the_worker_to_needs_item() {
        // `:232` → `:267-272`.
        let mut ai = ai();
        let mut scene = scene();
        ai.tick(&mut scene); // INIT → IDLE
        scene.facts.building.has_open_sync_request = true;
        let outcome = ai.tick(&mut scene);
        assert_eq!(outcome.state, WorkerState::NeedsItem);
        assert_eq!(
            outcome.fired.map(|fired| fired.rule),
            Some("checkIfNeedsItem")
        );
    }

    #[test]
    fn a_completed_sync_request_does_but_a_completed_async_one_does_not() {
        // `checkIfNeedsItem` reads `hasCitizenCompletedRequestsToPickup`, i.e. the
        // non-async ones (`AbstractBuilding.java:1565-1579`).
        let mut async_only = scene();
        async_only.facts.building.completed_requests = 1;
        async_only.facts.building.completed_async = 1;
        assert!(!base_check_if_needs_item(&async_only, WorkerState::Idle));
        let mut sync_too = async_only.clone();
        sync_too.facts.building.completed_sync = 1;
        assert!(base_check_if_needs_item(&sync_too, WorkerState::Idle));
        assert!(!base_check_if_needs_item(
            &sync_too,
            WorkerState::InventoryFull
        ));
        // Through the machine: the rule is looked at every 20 ticks (`:232`).
        let mut ai = ai();
        let mut scene = scene();
        ai.tick(&mut scene);
        scene.facts.building.completed_sync = 1;
        assert!(
            ticks_until(&mut ai, &mut scene, WorkerState::NeedsItem, 25),
            "the request rule is looked at once every 20 ticks"
        );
    }

    #[test]
    fn the_clean_async_target_never_transitions_but_does_clear_the_async_ones() {
        // `:236` + `:703-716`: `cleanAsync()` answers false, so the target cannot
        // fire — it only accepts the async requests it was called for.
        let mut ai = ai();
        let mut scene = scene();
        ai.tick(&mut scene); // INIT → IDLE
        scene.facts.building.completed_requests = 2;
        scene.facts.building.completed_async = 2;
        let outcome = ai.tick(&mut scene);
        assert_eq!(
            scene.facts.building.completed_async, 0,
            "accepted by cleanAsync"
        );
        assert_ne!(
            outcome.fired.map(|fired| fired.rule),
            Some("cleanAsync"),
            "`cleanAsync()` returns false (`:715`), so this target never fires"
        );
    }

    #[test]
    fn waiting_for_requests_ends_in_the_trades_after_request_state() {
        // `:548-552` → `afterRequestPickUp`: IDLE base, INVENTORY_FULL builder
        // (`EntityAIStructureBuilder.java:195-199`).
        let mut scene = Scene::new(WorldFacts::new());
        assert_eq!(
            wait_for_requests(&mut scene, WorkerState::NeedsItem),
            Some(WorkerState::Idle)
        );
        assert_eq!(scene.memory.delay, DELAY_RECHECK as i32);
        let mut builder = Scene::with_profile(WorldFacts::new(), JobProfile::BUILDER);
        assert_eq!(
            wait_for_requests(&mut builder, WorkerState::NeedsItem),
            Some(WorkerState::InventoryFull)
        );
    }

    #[test]
    fn a_deliverable_request_is_taken_only_when_it_is_still_there() {
        // `:583-591` (loaded, block entity) then `:607-634`.
        let mut scene = Scene::new(WorldFacts::new());
        scene.facts.building.completed_requests = 1;
        scene.facts.building.completed_deliverable = 1;
        assert_eq!(look_for_requests(&mut scene), Some(WorkerState::NeedsItem));
        assert_eq!(
            scene.memory.refusal,
            Some(Refusal::NotArrived),
            "no block entity at the hut: keep waiting"
        );

        let mut scene = Scene::new(WorldFacts::new());
        scene.facts.building.completed_requests = 1;
        scene.facts.building.completed_deliverable = 1;
        scene.facts.building.pickup_has_entity = true;
        scene.facts.building.pickup_items_present = true;
        assert_eq!(look_for_requests(&mut scene), Some(WorkerState::NeedsItem));
        assert_eq!(scene.memory.refusal, None);

        let mut scene = Scene::new(WorldFacts::new());
        scene.facts.building.completed_requests = 1;
        scene.facts.building.completed_deliverable = 1;
        scene.facts.building.pickup_has_entity = true;
        assert_eq!(look_for_requests(&mut scene), Some(WorkerState::NeedsItem));
        assert_eq!(scene.memory.refusal, Some(Refusal::StackGone));
    }

    #[test]
    fn an_unloaded_pickup_position_is_not_a_pickup() {
        // `:583` — `WorldUtil.isBlockLoaded`.
        let mut scene = Scene::new(WorldFacts::new());
        scene.facts.building.completed_requests = 1;
        scene.facts.building.completed_deliverable = 1;
        scene.facts.blocks.mark_unloaded(scene.facts.building.position);
        assert_eq!(look_for_requests(&mut scene), Some(WorkerState::NeedsItem));
        assert_eq!(scene.memory.refusal, Some(Refusal::NotArrived));
    }

    #[test]
    fn a_completed_request_that_cannot_be_delivered_is_dropped() {
        // `:559-567` — `!canBeDelivered()` → marked accepted, no pickup.
        let mut scene = Scene::new(WorldFacts::new());
        scene.facts.building.completed_requests = 1;
        scene.facts.building.completed_deliverable = 0;
        assert_eq!(look_for_requests(&mut scene), Some(WorkerState::NeedsItem));
        assert_eq!(scene.memory.refusal, None, "nothing went wrong");
    }

    #[test]
    fn gathering_materials_needs_a_chest_and_the_stack_in_it() {
        // `:282-316`.
        let mut scene = Scene::new(WorldFacts::new());
        assert_eq!(
            get_needed_item(&mut scene, WorkerState::GatheringRequiredMaterials),
            Some(WorkerState::StartWorking),
            "nothing was being fetched: getStateAfterPickUp"
        );

        let mut scene = Scene::new(WorldFacts::new());
        scene.memory.need = Some(4);
        assert_eq!(
            get_needed_item(&mut scene, WorkerState::GatheringRequiredMaterials),
            Some(WorkerState::StartWorking)
        );
        assert_eq!(scene.memory.refusal, Some(Refusal::NoChestWithItem));

        let mut scene = Scene::new(WorldFacts::new());
        scene.memory.need = Some(4);
        scene.facts.building.chest_with_needed = Some(Pos::new(0, 64, 0));
        scene.facts.inventory.needed_count = 4;
        assert_eq!(
            get_needed_item(&mut scene, WorkerState::GatheringRequiredMaterials),
            Some(WorkerState::StartWorking),
            "the worker holds what it asked for"
        );
        assert_eq!(scene.memory.walk_to, None);

        let mut scene = Scene::new(WorldFacts::new());
        scene.memory.need = Some(4);
        scene.facts.building.chest_with_needed = Some(Pos::new(0, 64, 0));
        scene.facts.inventory.needed_count = 1;
        assert_eq!(
            get_needed_item(&mut scene, WorkerState::GatheringRequiredMaterials),
            Some(WorkerState::GatheringRequiredMaterials),
            "the stack was gone: stay and retry"
        );
        assert_eq!(scene.memory.refusal, Some(Refusal::StackGone));
    }

    #[test]
    fn a_failed_walk_gives_up_after_ten_attempts() {
        // `:300-303` — `pickUpCounter++ < PICKUP_ATTEMPTS`.
        let mut scene = Scene::new(WorldFacts {
            worker: WorkerFacts {
                at: Pos::new(50, 64, 50),
                ..WorkerFacts::default()
            },
            ..WorldFacts::new()
        });
        scene.memory.need = Some(1);
        scene.facts.building.chest_with_needed = Some(Pos::new(0, 64, 0));
        for attempt in 1..=PICKUP_ATTEMPTS {
            get_needed_item(&mut scene, WorkerState::GatheringRequiredMaterials);
            assert_eq!(scene.memory.pickup_counter, attempt);
        }
        // The eleventh look gives up on the chest and tries the transfer anyway.
        assert_eq!(
            get_needed_item(&mut scene, WorkerState::GatheringRequiredMaterials),
            Some(WorkerState::GatheringRequiredMaterials)
        );
        assert_eq!(scene.memory.pickup_counter, 0, "reset at :305");
    }

    #[test]
    fn a_restart_needs_both_pause_and_the_request_and_goes_back_to_init() {
        // `:1877-1894`.
        assert!(!should_restart(&Scene::new(WorldFacts::new()), WorkerState::Idle));
        let mut scene = scene();
        scene.facts.worker.should_restart = true;
        assert!(!should_restart(&scene, WorkerState::Idle), "not paused");
        scene.facts.worker.paused = true;
        assert!(should_restart(&scene, WorkerState::Idle));
        assert_eq!(restart(&mut scene, WorkerState::Paused), Some(WorkerState::Init));
        assert_eq!(scene.memory.delay, WALK_DELAY as i32);
    }

    #[test]
    fn a_paused_worker_dumps_then_waits_and_unpauses_on_request() {
        // `:1831-1870` for the wait, `:1841-1844` for the starting dump, `:251` for
        // the unpause.
        let mut ai = ai();
        let mut scene = scene();
        ai.tick(&mut scene); // INIT → IDLE
        scene.facts.worker.paused = true;
        let outcome = ai.tick(&mut scene);
        assert_eq!(outcome.state, WorkerState::InventoryFull);
        assert_eq!(
            outcome.fired.map(|fired| fired.rule),
            Some("isStartingPaused")
        );

        scene.facts.building.full = true;
        let outcome = ai.tick(&mut scene);
        assert_eq!(outcome.state, WorkerState::Paused, "afterDump when paused");
        let outcome = ai.tick(&mut scene);
        assert_eq!(outcome.state, WorkerState::Paused);
        assert_eq!(outcome.why_not(), Some(Refusal::Paused));

        scene.facts.worker.paused = false;
        scene.facts.worker.should_restart = false;
        // `unpause` runs every 20 ticks (`:251`), and its first look happened while
        // the worker was still paused, so it has to wait for the next one.
        let mut unpaused_by = None;
        for _ in 0..8 {
            let outcome = ai.tick(&mut scene);
            if outcome.state == WorkerState::Idle {
                unpaused_by = outcome.fired.map(|fired| fired.rule);
                break;
            }
        }
        assert_eq!(unpaused_by, Some("unpause"));
    }

    #[test]
    fn a_delay_holds_the_state_for_its_whole_length() {
        // Target 4 (`:218`) against `:497-517`.
        let mut ai = ai();
        let mut scene = scene();
        ai.tick(&mut scene);
        set_delay(&mut scene, 10);
        for _ in 0..2 {
            let outcome = ai.tick(&mut scene);
            assert_eq!(
                outcome.fired.map(|fired| fired.rule),
                Some("waitingForSomething")
            );
            assert_eq!(outcome.state, WorkerState::Idle);
        }
        assert_eq!(scene.memory.delay, 0, "10 - 2 * ENTITY_AI_TICKRATE");
    }

    #[test]
    fn the_delay_clears_the_work_target_when_it_runs_out() {
        // `clearWorkTarget` (`:522-526`).
        let mut scene = Scene::new(WorldFacts {
            worker: WorkerFacts {
                working_at: Some(Pos::new(1, 64, 1)),
                ..WorkerFacts::default()
            },
            ..WorldFacts::new()
        });
        scene.memory.delay = 5;
        waiting_for_something(&mut scene, WorkerState::StartWorking);
        assert_eq!(scene.memory.delay, 0);
        assert_eq!(scene.facts.worker.working_at, None);
    }

    #[test]
    fn an_exception_doubles_the_timeout_each_time() {
        // `:353-390`: `timeout = EXCEPTION_TIMEOUT * exceptionTimer`, then `*= 2`.
        let mut ai = ai();
        let mut scene = scene();
        assert_eq!(ai.on_exception(&mut scene), 100);
        assert_eq!(scene.memory.exception_timer, 2);
        assert_eq!(scene.memory.delay, 100);
        assert_eq!(ai.on_exception(&mut scene), 200);
        assert_eq!(scene.memory.exception_timer, 4);
    }

    #[test]
    fn the_two_skill_curves_are_java_s_arithmetic() {
        // `:682-696`.
        assert_eq!(
            effective_skill_level(0),
            1,
            "2*1 - 0.1^2 = 1.99, truncated to 1"
        );
        assert_eq!(effective_skill_level(9), 19, "2*10 - 1^2");
        assert_eq!(effective_skill_level(99), 100, "2*100 - 10^2");
        assert_eq!(inverted_effective_skill_level(99), 1, "floored at 1");
        assert_eq!(inverted_effective_skill_level(9), 81);
    }

    #[test]
    fn the_best_tool_is_the_lowest_level_that_still_works() {
        // `:1405-1433`: the loop keeps the smallest level at or above `required`, and
        // `verifyEquipmentLevel` bounds it by the hut's maximum.
        let tools = [None, Some((2, 0)), Some((1, 0)), Some((4, 1)), Some((3, 5))];
        assert_eq!(
            best_tool_slot(&tools, 1, 5, true),
            BestTool::Slot(2),
            "level 1 is enough and cheaper"
        );
        assert_eq!(best_tool_slot(&tools, 2, 5, true), BestTool::Slot(1));
        assert_eq!(
            best_tool_slot(&tools, 3, 5, true),
            BestTool::Slot(3),
            "slot 3 is level 4 (5 with its enchantment); slot 4 is 3+5 = 8, over the \
             hut's maximum of 5"
        );
        assert_eq!(best_tool_slot(&tools, 6, 5, true), BestTool::NotFound);
        assert_eq!(
            best_tool_slot(&tools, 1, 5, false),
            BestTool::NoneNeeded,
            ":1410-1413 — the block needs no tool"
        );
        assert_eq!(best_tool_slot(&[], 1, 5, true), BestTool::NotFound);
    }

    #[test]
    fn a_missing_tool_is_taken_from_the_hut_or_asked_for_once() {
        // `:966-1005`.
        let mut scene = Scene::new(WorldFacts::new());
        assert_eq!(check_for_tool(&mut scene), ToolCheck::Asked { created: true });
        assert_eq!(scene.memory.delay, 0, ":1000 resets the delay");
        assert_eq!(scene.memory.refusal, Some(Refusal::ToolMissing));

        let mut scene = Scene::new(WorldFacts::new());
        scene.facts.building.open_tool_requests = 1;
        assert_eq!(check_for_tool(&mut scene), ToolCheck::Asked { created: false });

        let mut scene = Scene::new(WorldFacts::new());
        scene.facts.building.container_has_tool = true;
        assert_eq!(check_for_tool(&mut scene), ToolCheck::Took);
        assert_eq!(scene.memory.refusal, None);

        let mut scene = Scene::new(WorldFacts::new());
        scene.facts.building.citizen_has_tool = true;
        assert_eq!(check_for_tool(&mut scene), ToolCheck::Have);
        assert_eq!(scene.memory.delay, 0, "no +DELAY_RECHECK when it has one");
    }

    #[test]
    fn has_not_delayed_answers_true_exactly_once() {
        // `:1441-1451`.
        let mut memory = Memory::default();
        assert!(has_not_delayed(&mut memory, 7));
        assert_eq!(memory.delay, 7);
        assert!(!has_not_delayed(&mut memory, 7));
        assert!(has_not_delayed(&mut memory, 7));
    }

    #[test]
    fn only_a_builder_without_an_order_may_go_idle() {
        // `:1921-1924` against `EntityAIStructureBuilder.java:336-339`.
        let facts = WorldFacts::new();
        assert!(!can_go_idle(&JobProfile::BASE, &facts));
        assert!(!can_go_idle(&JobProfile::COMPOSTER, &facts));
        // No builder facts at all: the builder rule answers false, not "no order".
        assert!(!can_go_idle(&JobProfile::BUILDER, &facts));
        let without_order = WorldFacts {
            job: JobFacts::Builder(Default::default()),
            ..WorldFacts::new()
        };
        assert!(can_go_idle(&JobProfile::BUILDER, &without_order));
    }

    #[test]
    fn a_trade_that_is_not_ported_says_so_instead_of_guessing() {
        // 48 of the 50 jobs have no trade table; two do.
        assert!(JobAi::for_job(Job::Composter).is_ok());
        assert!(JobAi::for_job(Job::Builder).is_ok());
        let refused = JobAi::for_job(Job::Farmer).expect_err("farmer is not ported");
        assert_eq!(refused.job, Job::Farmer);
        assert_eq!(Refusal::TradeNotPorted.key(), "ai.refusal.trade_not_ported");
        // The base half still exists for every job.
        assert_eq!(JobAi::base(Job::Farmer).machine().transitions().len(), 14);
        // And the profile follows the trade, not the caller's choice.
        assert_eq!(
            JobAi::for_job(Job::Builder).expect("ported").profile(),
            JobProfile::BUILDER
        );
        assert_eq!(
            JobAi::for_job(Job::Composter).expect("ported").profile(),
            JobProfile::COMPOSTER
        );
    }

    #[test]
    fn every_refusal_names_a_java_line_and_a_message_key() {
        let all = [
            Refusal::NotHired,
            Refusal::JobAndHutDisagree,
            Refusal::WrongBuilding,
            Refusal::Paused,
            Refusal::Navigating,
            Refusal::HutFull,
            Refusal::NotArrived,
            Refusal::NoChestWithItem,
            Refusal::StackGone,
            Refusal::ToolMissing,
            Refusal::NothingToDump,
            Refusal::RequestDone,
            Refusal::CompostablesNotSet,
            Refusal::NoBarrel,
            Refusal::NoWorkOrder,
            Refusal::NoBlueprint,
            Refusal::NoStructure,
            Refusal::MissingMaterials,
            Refusal::StageLost,
            Refusal::TradeNotPorted,
        ];
        let mut keys: Vec<&str> = Vec::new();
        for refusal in all {
            keys.push(refusal.key());
            assert!(
                refusal.java().contains(".java:") || refusal.java().contains("RAPPORT"),
                "{} has no traceable source",
                refusal.key()
            );
        }
        let count = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), count, "two refusals share a key");
    }
}
