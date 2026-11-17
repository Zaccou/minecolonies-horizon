//! The concrete harvest goals: what a forester, a miner, a farmer and a fisherman do
//! once the decision layer has said "work".
//!
//! Java reference: `core/entity/ai/workers/production/EntityAIWorkLumberjack.java`,
//! `core/entity/ai/workers/production/EntityAIStructureMiner.java`,
//! `core/entity/ai/workers/production/agriculture/EntityAIWorkFarmer.java` and
//! `.../agriculture/EntityAIWorkFisherman.java`, with the three helpers they lean on
//! (`util/Tree.java`, `util/MineNode.java`, `api/util/Pond.java`).
//!
//! **What this module is, next to [`crate::ai`].** `crate::ai` answers *which* trade
//! works and drives the fourteen targets every worker inherits (`INIT`, `IDLE`,
//! `INVENTORY_FULL`, `NEEDS_ITEM`, `PAUSED`, …). It gives a table to two trades, the
//! composter and the builder, and answers `Err(Unported)` for the rest. This module is
//! the missing half for the four harvest trades: their own targets, their own states,
//! their own refusals, and the numbers each one carries.
//!
//! | Trade | Class | States | Targets | Actions |
//! | --- | --- | ---: | ---: | --- |
//! | forester | `EntityAIWorkLumberjack` | 6 | 8 | [`lumberjack`] |
//! | miner | `EntityAIStructureMiner` | 6 | 8 | [`miner`] |
//! | farmer | `EntityAIWorkFarmer` | 3 | 5 | [`farmer`] |
//! | fisherman | `EntityAIWorkFisherman` | 4 | 8 | [`fisherman`] |
//!
//! **One machine, one state enum, as in Java.** `AIWorkerState` is a single enum of
//! 127 values for every trade, and each trade's constructor *adds targets* to the
//! base table (`AbstractEntityAIBasic.java:200-260`, then e.g.
//! `EntityAIWorkLumberjack.java:193-201`). [`GatherState`] keeps that shape: the
//! nineteen trade states and the ten base states a trade enters or leaves. The engine
//! is [`crate::ai::state::Machine`], unchanged — this module adds a table, not a
//! second tick loop.
//!
//! **The boundary with the base table is explicit, not re-implemented.** A gather
//! table does not carry `INVENTORY_FULL`, `NEEDS_ITEM` or `PAUSED`: those are
//! `crate::ai::worker::register_base`'s, and [`register_handover`] only says where
//! control comes back *from* — `IDLE` to the trade's entry, and the wait states back
//! to `IDLE`, each with the base line that decides it. Two consequences, both written
//! down in `MANQUANT.md`:
//!
//! 1. `Machine<S, F>` is monomorphic in both `S` and `F`, so a gather machine cannot
//!    *contain* the base machine: a caller ticks one, then the other, and the
//!    hand-over states ([`GatherState::Base`]) are the seam.
//! 2. The miner is a structure AI (`AbstractEntityAIStructureWithWorkOrder`): its
//!    `LOAD_STRUCTURE` / `BUILDING_STEP` / `MINE_BLOCK` states belong to the structure
//!    table, which `crate::ai::builder` carries over `WorkerState`. The port hands
//!    those three back and says so, rather than porting that table twice.
//!
//! **What is deliberately absent.** No walking, no block breaking, no item spawn: a
//! goal *decides*, the caller acts. Every trade number Java keeps as a field of the AI
//! object is a [`facts::GatherMemory`] field; every draw Java makes with
//! `worker.getRandom()` is a fact the caller supplies, because a decision that cannot
//! be replayed cannot be tested.
//!
//! ```ignore
//! let mut ai = GatherAi::new(GatherTrade::Farmer);
//! let mut scene = ai.scene(WorldFacts::new());     // the trade's facts, defaulted
//! scene.job = GatherJob::Farmer(farmer_facts);
//! let outcome = ai.tick(&mut scene);
//! match (outcome.goal(), outcome.why_not()) {
//!     (GatherState::FarmerHarvest, _) => /* harvest one cell */,
//!     (_, Some(GatherRefusal::NoField)) => /* the hut has no field at all */,
//!     _ => {}
//! }
//! ```

pub mod facts;
pub mod farmer;
pub mod fisherman;
pub mod lumberjack;
pub mod miner;
pub mod state;

#[cfg(test)]
mod tests;

use core::fmt;

use crate::ai::facts::WorldFacts;
use crate::ai::state::{
    Action, Fired, Machine, Transition, WorkerState, ENTITY_AI_TICKRATE,
};
use crate::ai::worker::WOKR_IN_BUILDING_DIST;

pub use facts::{
    FarmerFacts, FarmerMemory, FieldFacts, FieldStage, FishermanFacts, FishermanMemory, GatherBlock,
    GatherBlocks, GatherJob, GatherMemory, GatherScene, Hook, LumberjackFacts, LumberjackMemory,
    MinerFacts, MinerMemory, NodeFacts, NodeStatus, NodeType, PathState, PondState, TreeFacts,
    Water,
};
pub use state::{GatherRefusal, GatherState};

/// `Constants.TICKS_SECOND` (`api/util/constant/Constants.java:21`): the rate the four
/// trade constructors pass to `registerTargets`.
pub const TICKS_SECOND: u32 = 20;

/// `AbstractEntityAIBasic.STANDARD_DELAY` as the AI module already fixed it: the rate
/// `startWorkingAtOwnBuilding` and the miner's structure steps run at.
pub const STANDARD_DELAY: u32 = crate::ai::worker::STANDARD_DELAY;

/// The four trades whose harvest goals this module carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: the four harvest trades of this slice
pub enum GatherTrade {
    /// `EntityAIWorkLumberjack` (`production/EntityAIWorkLumberjack.java:62`).
    Lumberjack,
    /// `EntityAIStructureMiner` (`production/EntityAIStructureMiner.java:64`). Java
    /// has **no** `EntityAIWorkMiner`: the miner's AI is a structure AI.
    Miner,
    /// `EntityAIWorkFarmer` (`production/agriculture/EntityAIWorkFarmer.java:75`).
    Farmer,
    /// `EntityAIWorkFisherman` (`production/agriculture/EntityAIWorkFisherman.java:65`).
    Fisherman,
}

impl GatherTrade {
    /// The four trades, in the order this module documents them.
    pub const ALL: [GatherTrade; 4] = [
        GatherTrade::Lumberjack,
        GatherTrade::Miner,
        GatherTrade::Farmer,
        GatherTrade::Fisherman,
    ];

    /// Java's class name, for a log line.
    pub fn class(self) -> &'static str {
        match self {
            GatherTrade::Lumberjack => "EntityAIWorkLumberjack",
            GatherTrade::Miner => "EntityAIStructureMiner",
            GatherTrade::Farmer => "EntityAIWorkFarmer",
            GatherTrade::Fisherman => "EntityAIWorkFisherman",
        }
    }

    /// The trade's name, for a message key.
    pub fn name(self) -> &'static str {
        match self {
            GatherTrade::Lumberjack => "lumberjack",
            GatherTrade::Miner => "miner",
            GatherTrade::Farmer => "farmer",
            GatherTrade::Fisherman => "fisherman",
        }
    }

    /// The job this trade is, in `crate::jobs` — the bridge for a caller that already
    /// has a `Job` (`jobs/job.rs:102-113`).
    pub fn job(self) -> crate::jobs::job::Job {
        match self {
            GatherTrade::Lumberjack => crate::jobs::job::Job::Lumberjack,
            GatherTrade::Miner => crate::jobs::job::Job::Miner,
            GatherTrade::Farmer => crate::jobs::job::Job::Farmer,
            GatherTrade::Fisherman => crate::jobs::job::Job::Fisherman,
        }
    }

    /// The first state the trade's own table claims — where `IDLE` hands over.
    ///
    /// All four go through `START_WORKING`, because all four parent classes register
    /// `IDLE → START_WORKING` and leave the trade to override what `START_WORKING`
    /// does: `AbstractEntityAICrafting.java:164` for the forester and the farmer,
    /// `AbstractEntityAIStructure.java:186` for the miner, and the fisherman's own
    /// `EntityAIWorkFisherman.java:169`.
    pub fn entry(self) -> GatherState {
        GatherState::Base(WorkerState::StartWorking)
    }

    /// The line that registers `IDLE → START_WORKING` for this trade.
    pub fn entry_java(self) -> &'static str {
        match self {
            GatherTrade::Lumberjack | GatherTrade::Farmer => "AbstractEntityAICrafting.java:164",
            GatherTrade::Miner => "AbstractEntityAIStructure.java:186",
            GatherTrade::Fisherman => "EntityAIWorkFisherman.java:169",
        }
    }

    /// The trade's facts, empty.
    pub fn job_facts(self) -> GatherJob {
        match self {
            GatherTrade::Lumberjack => GatherJob::Lumberjack(LumberjackFacts::default()),
            GatherTrade::Miner => GatherJob::Miner(MinerFacts::default()),
            GatherTrade::Farmer => GatherJob::Farmer(FarmerFacts::default()),
            GatherTrade::Fisherman => GatherJob::Fisherman(FishermanFacts::default()),
        }
    }

    /// `getActionsDoneUntilDumping()` for this trade, which
    /// `AbstractEntityAIBasic.inventoryNeedsDump` reads (`:413`).
    ///
    /// The base class answers `ACTIONS_UNTIL_DUMP` = 32 (`:425-428`); each trade
    /// overrides it, and the miner's answer is **not** a constant (`:222-225`).
    pub fn actions_until_dumping(self, scene: &GatherScene) -> u32 {
        match self {
            // `MAX_BLOCKS_MINED` (`EntityAIWorkLumberjack.java:128`, `:999-1002`).
            GatherTrade::Lumberjack => lumberjack::MAX_BLOCKS_MINED,
            // `building.getBuildingLevel() * MAX_BLOCKS_MINED`
            // (`EntityAIStructureMiner.java:222-225`).
            GatherTrade::Miner => miner::actions_until_dumping(scene),
            // `MAX_BLOCKS_MINED` (`EntityAIWorkFarmer.java:80`, `:197-201`).
            GatherTrade::Farmer => farmer::MAX_BLOCKS_MINED,
            // `MAX_FISHES_IN_INV` (`EntityAIWorkFisherman.java:105`, `:225-229`).
            GatherTrade::Fisherman => fisherman::MAX_FISHES_IN_INV,
        }
    }

    /// Register this trade's targets — the body of its constructor's
    /// `super.registerTargets(...)` call.
    pub fn register(self, machine: &mut Machine<GatherState, GatherScene>) {
        register_handover(machine, self);
        match self {
            GatherTrade::Lumberjack => lumberjack::register(machine),
            GatherTrade::Miner => miner::register(machine),
            GatherTrade::Farmer => farmer::register(machine),
            GatherTrade::Fisherman => fisherman::register(machine),
        }
    }
}

impl fmt::Display for GatherTrade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The states `trade`'s machine can be in: the base states it enters, plus its own.
pub fn states_of(trade: GatherTrade) -> Vec<GatherState> {
    let mut states: Vec<GatherState> = GatherState::ALL
        .iter()
        .copied()
        .filter(|state| state.belongs_to(trade))
        .collect();
    for base in handover_states(trade) {
        states.push(GatherState::Base(base));
    }
    states
}

/// The `WorkerState`s `trade`'s table enters or leaves, in `WorkerState`'s order.
///
/// The six base states every trade shares, plus the miner's three structure states.
fn handover_states(trade: GatherTrade) -> Vec<WorkerState> {
    use WorkerState::*;
    let mut states = vec![
        Idle,
        Preparing,
        StartWorking,
        InventoryFull,
        NeedsItem,
        Paused,
        GatheringRequiredMaterials,
    ];
    if trade == GatherTrade::Miner {
        states.extend([StartBuilding, MineBlock, LoadStructure, BuildingStep]);
    }
    states
}

/// The seam between a gather table and the base table: one transition per base state
/// the trade hands over, each citing the base line that owns it.
///
/// **This is not a second `register_base`.** `crate::ai::worker` already carries the
/// fourteen base targets over `WorkerState`, and a gather machine cannot hold them
/// (different state type — see the module note). What is registered here is only the
/// way *back*: a base state no gather action claims sends the machine to `IDLE` — or,
/// for the miner's three structure states, to the shaft check.
pub fn register_handover(machine: &mut Machine<GatherState, GatherScene>, trade: GatherTrade) {
    use WorkerState::*;
    // `dumpInventory` ends in `IDLE` (or `PAUSED`) — `afterDump`
    // (`AbstractEntityAIBasic.java:1213-1222`).
    machine.add(Transition::goto(
        GatherState::Base(InventoryFull),
        GatherState::Base(Idle),
        "afterDump",
        "AbstractEntityAIBasic.java:1213-1222",
        STANDARD_DELAY,
    ));
    // `waitForRequests` answers `IDLE` once the request is served
    // (`AbstractEntityAIBasic.java:238`, `:605-640`).
    machine.add(Transition::goto(
        GatherState::Base(NeedsItem),
        GatherState::Base(Idle),
        "waitForRequests",
        "AbstractEntityAIBasic.java:238",
        STANDARD_DELAY,
    ));
    // `PAUSED` → `IDLE` when the player unpauses (`AbstractEntityAIBasic.java:251`).
    machine.add(Transition::goto(
        GatherState::Base(Paused),
        GatherState::Base(Idle),
        "bePaused",
        "AbstractEntityAIBasic.java:251",
        TICKS_SECOND,
    ));
    // `GATHERING_REQUIRED_MATERIALS` → `IDLE` once the stack is in the inventory
    // (`AbstractEntityAIBasic.java:242`, `:323-326`).
    machine.add(Transition::goto(
        GatherState::Base(GatheringRequiredMaterials),
        GatherState::Base(Idle),
        "getNeededItem",
        "AbstractEntityAIBasic.java:242",
        STANDARD_DELAY,
    ));
    // `IDLE` leaves for the trade's entry: every concrete AI registers this, which is
    // why the base class is abstract (`AbstractEntityAIBasic.java:204-260`).
    machine.add(Transition::goto(
        GatherState::Base(Idle),
        trade.entry(),
        "idleToEntry",
        trade.entry_java(),
        1,
    ));
    // The structure half of the miner. `crate::ai::builder` carries those steps over
    // `WorkerState`; here they only say where the shaft check resumes.
    if trade == GatherTrade::Miner {
        for (state, rule, java) in [
            (
                LoadStructure,
                "afterStructureLoading",
                "EntityAIStructureMiner.java:828-832",
            ),
            (
                BuildingStep,
                "afterStructureLoading",
                "EntityAIStructureMiner.java:828-832",
            ),
        ] {
            machine.add(Transition::goto(
                GatherState::Base(state),
                GatherState::MinerCheckMineshaft,
                rule,
                java,
                STANDARD_DELAY,
            ));
        }
    }
}

/// What one trade action decided, in the two halves Java keeps apart: the state it
/// returns (`return getState()` when it can go no further) and the branch that
/// refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decision {
    /// The state the action returns, `None` for "stay in the state we are in" — the
    /// same shape as `Action::Run`'s `Option`.
    pub next: Option<GatherState>,
    /// Why it stopped, when it did.
    pub refusal: Option<GatherRefusal>,
}

impl Decision {
    /// Move to `state`.
    pub fn go(state: GatherState) -> Decision {
        Decision {
            next: Some(state),
            refusal: None,
        }
    }

    /// `return getState()`: no state change, no complaint.
    pub fn stay() -> Decision {
        Decision {
            next: None,
            refusal: None,
        }
    }

    /// `return getState()` with the reason recorded.
    pub fn refuse(refusal: GatherRefusal) -> Decision {
        Decision {
            next: None,
            refusal: Some(refusal),
        }
    }

    /// Move to `state` and record why.
    pub fn go_after(state: GatherState, refusal: GatherRefusal) -> Decision {
        Decision {
            next: Some(state),
            refusal: Some(refusal),
        }
    }

    /// Add a reason to a decision that has none.
    pub fn because(mut self, refusal: GatherRefusal) -> Decision {
        if self.refusal.is_none() {
            self.refusal = Some(refusal);
        }
        self
    }
}

/// What one tick of a harvest AI decided: the goal now in force, the rule that chose
/// it, and the refusal if it went no further — the same three answers as
/// [`crate::ai::worker::Outcome`], for a [`GatherState`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GatherOutcome {
    /// The goal in force after this tick.
    pub state: GatherState,
    /// The transition that ran, if one did.
    pub fired: Option<Fired<GatherState>>,
    /// The refusal recorded during this tick, if any.
    pub refusal: Option<GatherRefusal>,
}

impl GatherOutcome {
    /// The goal in force.
    pub fn goal(&self) -> GatherState {
        self.state
    }

    /// Why nothing moved, if the tick recorded a reason.
    pub fn why_not(&self) -> Option<GatherRefusal> {
        self.refusal
    }

    /// Whether this tick changed the goal.
    pub fn moved(&self) -> bool {
        matches!(self.fired, Some(fired) if fired.from != fired.to)
    }

    /// Whether the machine fell into a trap (`BasicStateMachine.java:220-228`).
    pub fn faulted(&self) -> bool {
        matches!(self.fired, Some(fired) if fired.reset)
    }

    /// The Java line of the rule that ran, for a log line.
    pub fn why(&self) -> Option<&'static str> {
        self.fired.map(|fired| fired.java)
    }
}

/// One harvest AI: a trade and its machine, nothing else.
pub struct GatherAi {
    trade: GatherTrade,
    machine: Machine<GatherState, GatherScene>,
}

impl fmt::Debug for GatherAi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GatherAi")
            .field("trade", &self.trade)
            .field("machine", &self.machine)
            .finish()
    }
}

impl GatherAi {
    /// The trade's AI, starting in `IDLE` at `ENTITY_AI_TICKRATE`
    /// (`AbstractEntityCitizen.java:64`).
    ///
    /// Java starts in `INIT`, and `initSafetyChecks` (`AbstractEntityAIBasic.java:
    /// 446-465`) sends it to `IDLE`; `INIT` and those checks belong to `crate::ai`, so
    /// this machine starts where that one arrives.
    pub fn new(trade: GatherTrade) -> GatherAi {
        let mut machine = Machine::new(GatherState::Base(WorkerState::Idle), ENTITY_AI_TICKRATE);
        trade.register(&mut machine);
        GatherAi { trade, machine }
    }

    /// A scene on `world` carrying this trade's facts, empty.
    pub fn scene(&self, world: WorldFacts) -> GatherScene {
        GatherScene::new(world, self.trade.job_facts())
    }

    /// A scene on `world` and `job`. A forester handed a miner's facts answers
    /// [`GatherRefusal::NoFactsForTrade`] on its first tick instead of pretending to
    /// have no tree.
    pub fn scene_with(&self, world: WorldFacts, job: GatherJob) -> GatherScene {
        GatherScene::new(world, job)
    }

    /// The trade this AI belongs to.
    pub fn trade(&self) -> GatherTrade {
        self.trade
    }

    /// The state the machine is in (`AbstractAISkeleton.getState`, `:120-126`).
    pub fn state(&self) -> GatherState {
        self.machine.state()
    }

    /// The last trap, if a tick fell into one (`BasicStateMachine.java:220-228`).
    pub fn missing(&self) -> Option<crate::ai::state::MissingTransition<GatherState>> {
        self.machine.missing()
    }

    /// `AbstractAISkeleton.resetAI` (`:145-152`).
    pub fn reset(&mut self) {
        self.machine.reset();
    }

    /// The machine itself, for a caller that wants to read the table.
    pub fn machine(&self) -> &Machine<GatherState, GatherScene> {
        &self.machine
    }

    /// One AI tick (`AbstractAISkeleton.tick`, `:63-66`).
    pub fn tick(&mut self, scene: &mut GatherScene) -> GatherOutcome {
        scene.refusal = None;
        let fired = self.machine.tick(scene);
        GatherOutcome {
            state: self.machine.state(),
            fired,
            refusal: scene.refusal,
        }
    }
}

/// `walkToBuilding()` (`AbstractEntityAIBasic.java:757-766`): the citizen is within
/// `WOKR_IN_BUILDING_DIST` of its hut, the same test `crate::ai::worker` makes.
pub fn at_hut(facts: &WorldFacts) -> bool {
    facts
        .worker
        .at
        .is_living_at_site(facts.building.position, WOKR_IN_BUILDING_DIST)
}

/// `inventoryNeedsDump` (`AbstractEntityAIBasic.java:409-416`) for a harvest scene.
///
/// **Restated, because it is not reachable.** The AI module has this rule as a private
/// function over its own `Scene` (`crate::ai::worker::inventory_needs_dump`). The
/// three conditions are the same three, plus the farmer's own `wantInventoryDumped`
/// (`EntityAIWorkFarmer.java:156-165`), which is a mutable flag and not the
/// `JobProfile::want_dump` constant.
pub fn dump_due(trade: GatherTrade, scene: &GatherScene) -> bool {
    let farmer_wants =
        matches!(&scene.job, GatherJob::Farmer(_)) && scene.memory.farmer.should_dump_inventory;
    scene.facts.inventory.full // :412
        || scene.facts.inventory.actions_done >= trade.actions_until_dumping(scene) // :413
        || farmer_wants // `EntityAIWorkFarmer.java:159`
}

/// A one-shot delay, so a trade does not have to reach into `crate::ai::worker` for
/// it: `hasNotDelayed` (`AbstractEntityAIBasic.java:1441-1451`).
pub fn has_not_delayed(scene: &mut GatherScene, ticks: u32) -> bool {
    crate::ai::worker::has_not_delayed(&mut scene.memory.base, ticks)
}

/// `setDelay` (`AbstractEntityAIBasic.java:397-400`).
pub fn set_delay(scene: &mut GatherScene, ticks: u32) {
    scene.memory.base.delay = ticks as i32;
}

/// `new AITarget(state, action, tickRate)` — the shape every trade constructor uses
/// (`AITarget.java:24-53`), written once because all four tables add the same kind of
/// transition and a closure would hold `machine` borrowed while a `goto` transition is
/// added next to it.
pub(crate) fn add_target(
    machine: &mut Machine<GatherState, GatherScene>,
    state: GatherState,
    rule: &'static str,
    java: &'static str,
    tick_rate: u32,
    action: fn(&mut GatherScene, GatherState) -> Option<GatherState>,
) {
    machine.add(Transition::target(
        state,
        rule,
        java,
        tick_rate,
        Action::Run(action),
    ));
}

/// The `Action::Run` shape every trade action has: split the scene into its three
/// fields, run the decision on the trade's own facts, record the refusal.
///
/// Written as a macro because the borrow shape is the fiddly part — the trade facts
/// and the memory are two fields of one struct, and only a destructuring borrow lets
/// the decision read the first while writing the second.
///
/// **Every name is `$crate`-qualified on purpose.** `macro_rules!` resolves paths at
/// the *expansion* site, so a bare `GatherScene` here would only compile in a module
/// that happens to import it: `lumberjack` imports `GatherState` and `GatherMemory`
/// but not `GatherScene`, and the eight calls at `lumberjack.rs:436-443` failed on
/// exactly that. A qualified body needs nothing from its caller.
macro_rules! gather_action {
    ($name:ident, $job:ident, $decide:path) => {
        fn $name(
            scene: &mut $crate::ai_gather::GatherScene,
            state: $crate::ai_gather::GatherState,
        ) -> Option<$crate::ai_gather::GatherState> {
            let $crate::ai_gather::GatherScene {
                facts,
                job,
                memory,
                refusal,
            } = scene;
            let $crate::ai_gather::GatherJob::$job(trade_facts) = job else {
                *refusal = Some($crate::ai_gather::GatherRefusal::NoFactsForTrade);
                return Some(state);
            };
            let decision: $crate::ai_gather::Decision = $decide(facts, trade_facts, memory);
            *refusal = decision.refusal;
            decision.next.or(Some(state))
        }
    };
}

pub(crate) use gather_action;
