//! Three trades' goals: the nether miner, the beekeeper, and — already carried
//! elsewhere — the composter.
//!
//! Java reference: `core/entity/ai/workers/production/EntityAIWorkNether.java`,
//! `core/entity/ai/workers/production/agriculture/EntityAIWorkBeekeeper.java` and
//! `core/entity/ai/workers/production/agriculture/EntityAIWorkComposter.java`.
//!
//! | Trade | Class | Own states | Targets | File |
//! | --- | --- | ---: | ---: | --- |
//! | nether miner | `EntityAIWorkNether` | 5 | 7 | [`nether`] |
//! | beekeeper | `EntityAIWorkBeekeeper` | 2 | 6 | [`beekeeper`] |
//! | composter | `EntityAIWorkComposter` | — | — | [`composter`], a seam |
//!
//! **What this module adds to [`crate::ai`].** `crate::ai` answers *which* trade works
//! and drives the fourteen targets every worker inherits (`INIT`, `IDLE`,
//! `INVENTORY_FULL`, `NEEDS_ITEM`, `PAUSED`, …). It gives a table to two trades, the
//! composter and the builder, and answers `Err(Unported)` for the rest —
//! `RAPPORT.md` §3 lists the 47, nether miner and beekeeper among them. This module is
//! the missing half for two of those 47: their own states, their own targets, their own
//! refusals, and the numbers each one carries.
//!
//! **The composter is not duplicated.** `crate::ai::composter` already ports all six of
//! its targets, its two barrel scans, its loot multiplier and its complaint throttle,
//! with fourteen tests. Re-writing them here would be a second copy of a table that
//! exists, so this module only reaches that table through [`WorkerAi`] and closes the
//! one rule `crate::ai::composter` left as a comment: which product a harvested barrel
//! gives — dirt or podzol (`EntityAIWorkComposter.java:316-344`), in
//! [`composter::harvest_product`]. `RAPPORT.md` §1 has the inventory of what was
//! already there.
//!
//! **One machine, one state enum, as in Java.** `AIWorkerState` is a single enum of 127
//! values for every trade, and each constructor *adds targets* to the base table
//! (`AbstractEntityAIBasic.java:200-260`, then e.g. `EntityAIWorkNether.java:101-107`).
//! [`WorkState`] keeps that shape; the engine is [`crate::ai::state::Machine`],
//! unchanged — this module adds a table, not a second tick loop.
//!
//! **The boundary with the base table is explicit.** A work table does not carry
//! `INVENTORY_FULL`, `NEEDS_ITEM` or `PAUSED`: those are `crate::ai::worker::register_base`'s
//! and [`register_handover`] only says where control comes back *from*, each line citing
//! the base line that owns it. `Machine<S, F>` is monomorphic in both `S` and `F`, so a
//! work machine cannot *contain* the base machine: a caller ticks one, then the other.
//!
//! **What is deliberately absent.** No walking, no block breaking, no loot table, no
//! entity spawn: a goal *decides*, the caller acts. Every draw Java makes with
//! `worker.getRandom()` is a fact the caller supplies, because a decision that cannot be
//! replayed cannot be tested. The blow-by-blow fight of `stayInNether`
//! (`EntityAIWorkNether.java:409-474`) stays with the caller for that reason —
//! `nether.rs` gives it the four numbers it computes and says why.
//!
//! ```ignore
//! let mut ai = WorkAi::new(WorkTrade::Nether).expect("the composter is a WorkerAi");
//! let mut scene = ai.scene(WorldFacts::new());
//! let outcome = ai.tick(&mut scene);
//! match (outcome.goal(), outcome.why_not()) {
//!     (WorkState::NetherLeave, _) => /* walk to the portal */,
//!     (_, Some(WorkRefusal::MissingTool(tool))) => /* ask the player for `tool` */,
//!     _ => {}
//! }
//! ```

pub mod beekeeper;
pub mod composter;
pub mod facts;
pub mod nether;
pub mod state;

#[cfg(test)]
mod tests;

use core::fmt;

use crate::ai::facts::WorldFacts;
use crate::ai::state::{Action, Fired, Machine, Transition, WorkerState};
use crate::ai::worker::JobProfile;

pub use facts::{
    AdventureToken, Bee, BeekeeperFacts, BeekeeperMemory, CraftCheck, CraftedResult,
    HarvestType, Hive, NetherFacts, NetherMemory, NetherRecipe, ProcessedItem, SwordDamage,
    WorkBlock, WorkBlocks, WorkJob, WorkMemory, WorkScene,
};
pub use state::{WorkRefusal, WorkState};

/// `Constants.TICKS_SECOND` (`api/util/constant/Constants.java:21`): the rate the
/// beekeeper's constructor passes for `START_WORKING` and `BEEKEEPER_HARVEST`
/// (`EntityAIWorkBeekeeper.java:110`, `:114`).
pub const TICKS_SECOND: u32 = 20;

/// `AbstractEntityAIBasic.STANDARD_DELAY` (`:100`), the rate the crafting parent
/// registers `START_WORKING → decide` at (`AbstractEntityAICrafting.java:166`).
pub const STANDARD_DELAY: u32 = crate::ai::worker::STANDARD_DELAY;

/// The three trades this slice names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: the three classes the mission names
pub enum WorkTrade {
    /// `EntityAIWorkNether` (`production/EntityAIWorkNether.java:65`).
    Nether,
    /// `EntityAIWorkBeekeeper` (`production/agriculture/EntityAIWorkBeekeeper.java:53`).
    Beekeeper,
    /// `EntityAIWorkComposter` (`production/agriculture/EntityAIWorkComposter.java:44`) —
    /// already ported by `crate::ai::composter`.
    Composter,
}

impl WorkTrade {
    /// The three trades, in the order this module documents them.
    pub const ALL: [WorkTrade; 3] = [
        WorkTrade::Nether,
        WorkTrade::Beekeeper,
        WorkTrade::Composter,
    ];

    /// Java's class name, for a log line.
    pub fn class(self) -> &'static str {
        match self {
            WorkTrade::Nether => "EntityAIWorkNether",
            WorkTrade::Beekeeper => "EntityAIWorkBeekeeper",
            WorkTrade::Composter => "EntityAIWorkComposter",
        }
    }

    /// The trade's name, for a message key.
    pub fn name(self) -> &'static str {
        match self {
            WorkTrade::Nether => "netherworker",
            WorkTrade::Beekeeper => "beekeeper",
            WorkTrade::Composter => "composter",
        }
    }

    /// The job this trade is, in `crate::jobs` — the bridge for a caller that already
    /// has a `Job` (`jobs/job.rs`).
    pub fn job(self) -> crate::jobs::job::Job {
        match self {
            WorkTrade::Nether => crate::jobs::job::Job::NetherWorker,
            WorkTrade::Beekeeper => crate::jobs::job::Job::Beekeeper,
            WorkTrade::Composter => crate::jobs::job::Job::Composter,
        }
    }

    /// A machine over [`WorkState`] for this trade, `None` for the composter — whose
    /// table is `crate::ai::composter`'s over `WorkerState`, i.e. a
    /// [`crate::ai::worker::JobAi`], not a [`WorkAi`].
    ///
    /// Returning `None` rather than a second composter table is the whole point of this
    /// method: a caller that wants all three asks [`WorkTrade::ai`].
    pub fn work_ai(self) -> Option<WorkAi> {
        match self {
            WorkTrade::Nether | WorkTrade::Beekeeper => Some(WorkAi::half(self)),
            WorkTrade::Composter => None,
        }
    }

    /// The AI of the trade, whichever of the two engines it runs on.
    pub fn ai(self) -> WorkerAi {
        match self {
            WorkTrade::Composter => WorkerAi::Composter(crate::ai::composter::ai()),
            trade => WorkerAi::Work(WorkAi::half(trade)),
        }
    }

    /// The first state the trade's own table claims — where `IDLE` hands over.
    pub fn entry(self) -> WorkState {
        WorkState::Base(WorkerState::StartWorking)
    }

    /// The line that registers `IDLE → START_WORKING` for this trade.
    pub fn entry_java(self) -> &'static str {
        match self {
            // The parent of the nether miner is the crafting AI.
            WorkTrade::Nether => "AbstractEntityAICrafting.java:164",
            WorkTrade::Beekeeper => "EntityAIWorkBeekeeper.java:109",
            // `crate::ai::composter::register` carries it (`EntityAIWorkComposter.java:97`).
            WorkTrade::Composter => "EntityAIWorkComposter.java:97",
        }
    }

    /// The trade's facts, empty — `None` for the composter, whose facts are
    /// `crate::ai::facts::ComposterFacts` reached through `WorldFacts::composter()`
    /// and whose scene is `crate::ai::worker::Scene`, not [`WorkScene`].
    pub fn job_facts(self) -> Option<WorkJob> {
        match self {
            WorkTrade::Nether => Some(WorkJob::Nether(NetherFacts::default())),
            WorkTrade::Beekeeper => Some(WorkJob::Beekeeper(BeekeeperFacts::default())),
            WorkTrade::Composter => None,
        }
    }

    /// `getActionsDoneUntilDumping()` for this trade, which
    /// `AbstractEntityAIBasic.inventoryNeedsDump` reads (`:413`).
    ///
    /// The base class answers `ACTIONS_UNTIL_DUMP` = 32 (`:425-428`); two of these three
    /// trades override it to `1` or `5`.
    pub fn actions_until_dumping(self) -> u32 {
        match self {
            // `AbstractEntityAICrafting.getActionsDoneUntilDumping` (`:724-727`), which
            // `EntityAIWorkNether` does not override.
            WorkTrade::Nether => 1,
            // `EntityAIWorkBeekeeper.getActionsDoneUntilDumping` (`:118-122`).
            WorkTrade::Beekeeper => beekeeper::ACTIONS_UNTIL_DUMPING,
            // `EntityAIWorkComposter.getActionsDoneUntilDumping` (`:380-384`).
            WorkTrade::Composter => 1,
        }
    }

    /// The base-table profile of this trade — the fields
    /// `crate::ai::worker::JobProfile` needs so that the *base* machine dumps at the
    /// right moment and resumes in the right state.
    pub fn profile(self) -> JobProfile {
        match self {
            // `getStateAfterPickUp()` answers `START_WORKING`
            // (`EntityAIWorkNether.java:158-162`).
            WorkTrade::Nether => JobProfile {
                actions_until_dumping: 1, // AbstractEntityAICrafting.java:727
                state_after_pickup: WorkerState::StartWorking, // :161
                ..JobProfile::BASE
            },
            WorkTrade::Beekeeper => JobProfile {
                actions_until_dumping: beekeeper::ACTIONS_UNTIL_DUMPING, // :121
                ..JobProfile::BASE
            },
            WorkTrade::Composter => JobProfile::COMPOSTER,
        }
    }

    /// Register this trade's targets — the body of its constructor's
    /// `super.registerTargets(...)` call.
    ///
    /// The composter registers nothing here: `crate::ai::composter::register` does it
    /// over `WorkerState`, and copying those six lines would be the duplication this
    /// module exists to avoid.
    pub fn register(self, machine: &mut Machine<WorkState, WorkScene>) {
        register_handover(machine, self);
        match self {
            WorkTrade::Nether => nether::register(machine),
            WorkTrade::Beekeeper => beekeeper::register(machine),
            WorkTrade::Composter => {}
        }
    }
}

impl fmt::Display for WorkTrade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The states `trade`'s machine can be in, in [`WorkState::ALL`]'s order.
///
/// Every state this returns has at least one transition — the hand-over seam's for a base
/// state, the trade's own target for the rest — so a machine over this list can always
/// leave the state it lands in (`BasicStateMachine.java:220-228` calls the alternative a
/// trap).
///
/// The composter's answer is empty and that is not a bug: its states are `WorkerState`s
/// held by `crate::ai::composter`, and this enum has no variant for them.
pub fn states_of(trade: WorkTrade) -> Vec<WorkState> {
    if trade == WorkTrade::Composter {
        return Vec::new();
    }
    WorkState::ALL
        .iter()
        .copied()
        .filter(|state| state.belongs_to(trade))
        .collect()
}

/// The seam between a work table and the base table: what happens to a base state this
/// module does not own.
///
/// **This is not a second `register_base`.** `crate::ai::worker` already carries the
/// fourteen base targets over `WorkerState`. What is registered here is only the way
/// *back*: a base state no work action claims sends the machine to `IDLE`, which is what
/// Java's own base table does with it.
pub fn register_handover(machine: &mut Machine<WorkState, WorkScene>, trade: WorkTrade) {
    use WorkerState::*;
    // `IDLE` leaves for the trade's entry: every concrete AI registers this line
    // itself, which is why the base class is abstract
    // (`AbstractEntityAIBasic.java:204-260`).
    machine.add(Transition::goto(
        WorkState::Base(Idle),
        trade.entry(),
        "idleToEntry",
        trade.entry_java(),
        1,
    ));
    // `dumpInventory` ends in `IDLE` (or `PAUSED`) — `afterDump`
    // (`AbstractEntityAIBasic.java:1213-1222`).
    machine.add(Transition::goto(
        WorkState::Base(InventoryFull),
        WorkState::Base(Idle),
        "afterDump",
        "AbstractEntityAIBasic.java:1213-1222",
        STANDARD_DELAY,
    ));
    // `getNeededItem` answers `IDLE` once the stack is in the inventory
    // (`AbstractEntityAIBasic.java:242`).
    machine.add(Transition::goto(
        WorkState::Base(GatheringRequiredMaterials),
        WorkState::Base(Idle),
        "getNeededItem",
        "AbstractEntityAIBasic.java:242",
        STANDARD_DELAY,
    ));
    // The nether miner reaches `INVENTORY_FULL` from two of its own actions
    // (`EntityAIWorkNether.java:294`, `:321`, `:651`, `:712`); `IDLE` is what the base
    // table then does with it. Nothing else is added: `NEEDS_ITEM` and `PAUSED` are
    // base targets and stay the base machine's.
}

/// What one trade action decided, in the two halves Java keeps apart: the state it
/// returns (`return getState()` when it can go no further) and the branch that refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decision {
    /// The state the action returns, `None` for "stay in the state we are in" — the
    /// same shape as `Action::Run`'s `Option`.
    pub next: Option<WorkState>,
    /// Why it stopped, when it did.
    pub refusal: Option<WorkRefusal>,
}

impl Decision {
    /// Move to `state`.
    pub fn go(state: WorkState) -> Decision {
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
    pub fn refuse(refusal: WorkRefusal) -> Decision {
        Decision {
            next: None,
            refusal: Some(refusal),
        }
    }

    /// Move to `state` and record why.
    pub fn go_after(state: WorkState, refusal: WorkRefusal) -> Decision {
        Decision {
            next: Some(state),
            refusal: Some(refusal),
        }
    }

    /// Add a reason to a decision that has none.
    pub fn because(mut self, refusal: WorkRefusal) -> Decision {
        if self.refusal.is_none() {
            self.refusal = Some(refusal);
        }
        self
    }
}

/// What one tick of a work AI decided: the goal now in force, the rule that chose it,
/// and the refusal if it went no further — the same three answers as
/// [`crate::ai::worker::Outcome`], for a [`WorkState`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkOutcome {
    /// The goal in force after this tick.
    pub state: WorkState,
    /// The transition that ran, if one did.
    pub fired: Option<Fired<WorkState>>,
    /// The refusal recorded during this tick, if any.
    pub refusal: Option<WorkRefusal>,
}

impl WorkOutcome {
    /// The goal in force.
    pub fn goal(&self) -> WorkState {
        self.state
    }

    /// Why nothing moved, if the tick recorded a reason.
    pub fn why_not(&self) -> Option<WorkRefusal> {
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

/// One work AI: a trade, its machine, and the empty facts its scenes start from.
pub struct WorkAi {
    trade: WorkTrade,
    machine: Machine<WorkState, WorkScene>,
    blank: WorkJob,
}

impl fmt::Debug for WorkAi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorkAi")
            .field("trade", &self.trade)
            .field("machine", &self.machine)
            .field("blank", &self.blank)
            .finish()
    }
}

impl WorkAi {
    /// The trade's AI, starting in `IDLE` at `ENTITY_AI_TICKRATE`
    /// (`AbstractEntityCitizen.java:64`).
    ///
    /// `None` for the composter: its table is `crate::ai::composter`'s and lives over
    /// another scene. [`WorkTrade::ai`] is the door that answers for all three.
    pub fn new(trade: WorkTrade) -> Option<WorkAi> {
        trade.work_ai()
    }

    /// The same, from inside the module, where the caller has already ruled the
    /// composter out.
    fn half(trade: WorkTrade) -> WorkAi {
        let mut machine = Machine::new(
            WorkState::Base(WorkerState::Idle),
            crate::ai::state::ENTITY_AI_TICKRATE,
        );
        trade.register(&mut machine);
        // `work_ai` only answers `Some` for the two trades with a table, so the facts
        // template always exists; the fallback is the nether one, which no composter
        // scene can reach.
        let blank = trade
            .job_facts()
            .unwrap_or(WorkJob::Nether(NetherFacts::default()));
        WorkAi {
            trade,
            machine,
            blank,
        }
    }

    /// A scene on `world` carrying this trade's facts, empty.
    pub fn scene(&self, world: WorldFacts) -> WorkScene {
        WorkScene::new(world, self.blank.clone())
    }

    /// A scene on `world` and `job`. A nether miner handed a beekeeper's facts answers
    /// [`WorkRefusal::NotThisTrade`] on its first tick instead of pretending the hut
    /// has no portal.
    pub fn scene_with(&self, world: WorldFacts, job: WorkJob) -> WorkScene {
        WorkScene::new(world, job)
    }

    /// The trade this AI belongs to.
    pub fn trade(&self) -> WorkTrade {
        self.trade
    }

    /// The state the machine is in (`AbstractAISkeleton.getState`, `:120-126`).
    pub fn state(&self) -> WorkState {
        self.machine.state()
    }

    /// The last trap, if a tick fell into one (`BasicStateMachine.java:220-228`).
    pub fn missing(&self) -> Option<crate::ai::state::MissingTransition<WorkState>> {
        self.machine.missing()
    }

    /// `AbstractAISkeleton.resetAI` (`:145-152`).
    pub fn reset(&mut self) {
        self.machine.reset();
    }

    /// The machine itself, for a caller that wants to read the table.
    pub fn machine(&self) -> &Machine<WorkState, WorkScene> {
        &self.machine
    }

    /// One AI tick (`AbstractAISkeleton.tick`, `:63-66`).
    pub fn tick(&mut self, scene: &mut WorkScene) -> WorkOutcome {
        scene.refusal = None;
        let fired = self.machine.tick(scene);
        WorkOutcome {
            state: self.machine.state(),
            fired,
            refusal: scene.refusal,
        }
    }
}

/// The AI of a trade, on whichever of the two engines its table lives.
///
/// The composter's six targets were ported before this module existed
/// (`crate::ai::composter`), over `WorkerState` and `crate::ai::worker::Scene`. A
/// `Machine<S, F>` cannot hold two state types, so the two cannot be one machine: this
/// enum is the seam, and [`composter`] documents what it does and does not cover.
#[derive(Debug)]
pub enum WorkerAi {
    /// The nether miner's or the beekeeper's table, over [`WorkState`].
    Work(WorkAi),
    /// The composter's table, over `crate::ai::state::WorkerState`.
    Composter(crate::ai::worker::JobAi),
}

impl WorkerAi {
    /// The trade this AI belongs to.
    pub fn trade(&self) -> WorkTrade {
        match self {
            WorkerAi::Work(ai) => ai.trade(),
            WorkerAi::Composter(_) => WorkTrade::Composter,
        }
    }

    /// The work machine, `None` for the composter.
    pub fn work(&self) -> Option<&WorkAi> {
        match self {
            WorkerAi::Work(ai) => Some(ai),
            WorkerAi::Composter(_) => None,
        }
    }
}

/// `walkToBuilding()` (`AbstractEntityAIBasic.java:757-766`): the citizen is within
/// `WOKR_IN_BUILDING_DIST` of its hut.
///
/// Written here rather than reused from `crate::ai_gather`, which restates the same
/// rule: a module that must keep compiling while another agent edits its folder reads
/// only `crate::ai`, whose constant is the shared part.
pub fn at_hut(facts: &WorldFacts) -> bool {
    facts
        .worker
        .at
        .is_living_at_site(facts.building.position, crate::ai::worker::WOKR_IN_BUILDING_DIST)
}

/// A one-shot delay, so a trade does not have to reach into `crate::ai::worker` for it:
/// `hasNotDelayed` (`AbstractEntityAIBasic.java:1441-1451`).
pub fn has_not_delayed(scene: &mut WorkScene, ticks: u32) -> bool {
    crate::ai::worker::has_not_delayed(&mut scene.memory.base, ticks)
}

/// `setDelay` (`AbstractEntityAIBasic.java:397-400`).
pub fn set_delay(scene: &mut WorkScene, ticks: u32) {
    scene.memory.base.delay = ticks as i32;
}

/// `new AITarget(state, action, tickRate)` — the shape every trade constructor uses
/// (`AITarget.java:24-53`), written once because all of them add the same kind of
/// transition and a closure would hold `machine` borrowed while a `goto` transition is
/// added next to it.
pub(crate) fn add_target(
    machine: &mut Machine<WorkState, WorkScene>,
    state: WorkState,
    rule: &'static str,
    java: &'static str,
    tick_rate: u32,
    action: fn(&mut WorkScene, WorkState) -> Option<WorkState>,
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
/// Written as a macro because the borrow shape is the fiddly part — the trade facts and
/// the memory are two fields of one struct, and only a destructuring borrow lets the
/// decision read the first while writing the second.
///
/// **Every name is `$crate`-qualified on purpose.** `macro_rules!` resolves paths at the
/// *expansion* site, so a bare `WorkScene` here would only compile in a module that
/// happens to import it; `nether` imports `WorkState` but not `WorkScene`.
macro_rules! work_action {
    ($name:ident, $job:ident, $decide:path) => {
        fn $name(
            scene: &mut $crate::ai_work_07::WorkScene,
            state: $crate::ai_work_07::WorkState,
        ) -> Option<$crate::ai_work_07::WorkState> {
            let $crate::ai_work_07::WorkScene {
                facts,
                job,
                memory,
                refusal,
            } = scene;
            let $crate::ai_work_07::WorkJob::$job(trade) = job else {
                *refusal = Some($crate::ai_work_07::WorkRefusal::NotThisTrade);
                return Some(state);
            };
            let decision: $crate::ai_work_07::Decision = $decide(facts, trade, memory);
            *refusal = decision.refusal;
            decision.next.or(Some(state))
        }
    };
}

/// The same as [`work_action`] for a decision that reads only the trade's facts and the
/// memory — `stayInNether` (`EntityAIWorkNether.java:375-596`), `openPortal` (`:657-685`)
/// and `closePortal` (`:690-713`) never look at the worker, its inventory or its hut.
macro_rules! work_action_trade {
    ($name:ident, $job:ident, $decide:path) => {
        fn $name(
            scene: &mut $crate::ai_work_07::WorkScene,
            state: $crate::ai_work_07::WorkState,
        ) -> Option<$crate::ai_work_07::WorkState> {
            let $crate::ai_work_07::WorkScene {
                job, memory, refusal, ..
            } = scene;
            let $crate::ai_work_07::WorkJob::$job(trade) = job else {
                *refusal = Some($crate::ai_work_07::WorkRefusal::NotThisTrade);
                return Some(state);
            };
            let decision: $crate::ai_work_07::Decision = $decide(trade, memory);
            *refusal = decision.refusal;
            decision.next.or(Some(state))
        }
    };
}

pub(crate) use work_action;
pub(crate) use work_action_trade;
