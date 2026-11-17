//! The goals of the two education trades: what a researcher does at a university and
//! what a teacher does at a school — the states they walk, what they ask the world,
//! the refusals they name, and how long each step takes.
//!
//! Java reference: `core/entity/ai/workers/education/EntityAIWorkResearcher.java` (126
//! lines) and `.../EntityAIWorkTeacher.java` (237), plus the four classes they read —
//! `colony/buildings/workerbuildings/BuildingUniversity.java` (`getRandomBookShelf`),
//! `.../BuildingSchool.java` (`getRandomPlaceToSit`), `colony/jobs/JobResearch.java`
//! (the mana) and `core/entity/other/SittingEntity.java` (the seat).
//!
//! ## The brief named three files; one of them was already done
//!
//! The brief asks for the researcher, the teacher **and**
//! `production/EntityAIWorkLumberjack.java`. The forester is already ported, in full:
//! `src/ai_gather/lumberjack.rs` (837 lines) carries its eight targets
//! (`EntityAIWorkLumberjack.java:193-201` plus the `START_WORKING` override at `:224`),
//! its eleven constants with their lines, its drop calculation, its tree search and its
//! ten tests. Re-porting it here would be a second copy of a table that already has
//! tests — rule 6 — so this module deliberately does **not** contain it, and this
//! paragraph is the report of that instead of a silent omission. `RAPPORT.md` §1 says
//! the same thing with the measurements.
//!
//! ## What this module adds
//!
//! | File | Question it answers | Java |
//! | --- | --- | --- |
//! | [`state`] | the two states these trades own, and the sixteen refusals they name | `AIWorkerState.java:439`, `:622` |
//! | [`facts`] | what a university pass and a lesson may read, as observations | the two AI files, `BuildingUniversity`, `BuildingSchool` |
//! | [`researcher`] | walk to a bookshelf, study one pass, decide whether it advanced anything | `EntityAIWorkResearcher.java:68-125` |
//! | [`teacher`] | check the paper, pick a seated pupil, sit, teach, hand a sheet over | `EntityAIWorkTeacher.java:89-236` |
//!
//! **The decision, in one sentence.** Given a [`facts::EduScene`] (the same
//! [`WorldFacts`] `crate::ai` takes, the trade's own facts, and the AI's memory), an
//! [`EduAi`] ticks and answers an [`EduOutcome`]: the state now in force, the rule that
//! chose it with its `fichier.java:ligne`, the refusal when it went no further, and the
//! [`Effect`]s the caller must carry out. Nothing here moves a citizen, spawns a seat,
//! moves a sheet of paper or advances a research.
//!
//! ```ignore
//! let mut ai = EduAi::new(EducationTrade::Teacher);
//! let mut scene = ai.scene(WorldFacts::new());
//! scene.job = EduJob::Teacher(TeacherFacts::default());
//! match ai.tick(&mut scene) {
//!     outcome => match (outcome.goal(), outcome.why_not()) {
//!         (EduState::Teach, _) => /* walk to the pupil and sit */,
//!         (_, Some(EduRefusal::NoPupil)) => /* the school has no child sitting down */,
//!         _ => {}
//!     },
//! }
//! ```
//!
//! ## The engine is `crate::ai`'s, unchanged
//!
//! [`EduState`] is one enum for both trades, the way `AIWorkerState` is one enum for all
//! 127 states; the tick order, the rate countdown and the three global passes are
//! [`crate::ai::state::Machine`], which already has its tests. This module adds a table,
//! not a second tick loop. [`register_handover`] is the seam back to the base table: it
//! holds one transition per base state an education AI enters or leaves, each citing the
//! base line that owns it, and **it is not** a second `register_base`
//! (`crate::ai::worker` carries those fourteen targets over `WorkerState`).
//!
//! Two consequences, both written down in `MANQUANT.md` §1:
//!
//! 1. `Machine<S, F>` is monomorphic in `S` and `F`, so an education machine cannot
//!    *contain* the base machine. A caller ticks one, then the other; the base states
//!    ([`EduState::Base`]) are the seam.
//! 2. The base class's `waitingForSomething` (`AbstractEntityAIBasic.java:496-517`) is
//!    what makes `setDelay(1200)` mean "one pass per minute". It belongs to the base
//!    table, so an education machine *records* `memory.base.delay` and does not spend
//!    it — a caller that wants the minute to pass ticks `JobAi` too.
//!
//! ## No world, no clock, no random
//!
//! Every read is a field of [`facts::EduScene`]; every draw Java makes with
//! `worker.getRandom()` — which research a pass advances
//! (`EntityAIWorkResearcher.java:86`), which pupil is picked
//! (`EntityAIWorkTeacher.java:110`), how long a lesson lasts (`:134`) — is a fact the
//! caller supplies, because a decision that depends on a random draw cannot be replayed
//! and therefore cannot be tested. **No SDK call:** nothing here imports `mod_api`.

pub mod facts;
pub mod researcher;
pub mod state;
pub mod teacher;

#[cfg(test)]
mod tests;

use core::fmt;

use crate::ai::facts::{Pos, WorldFacts};
use crate::ai::state::{
    Action, Fired, Machine, Transition, WorkerState, ENTITY_AI_TICKRATE,
};
use crate::ai::worker::{STANDARD_DELAY as BASE_STANDARD_DELAY, WOKR_IN_BUILDING_DIST};
use crate::jobs::job::Job;

pub use facts::{
    EduJob, EduMemory, EduScene, InProgress, Pupil, ResearcherFacts, ResearcherMemory,
    TeacherFacts, TeacherMemory,
};
pub use state::{EduRefusal, EduState};

/// `Constants.TICKS_SECOND` (`api/util/constant/Constants.java:21`): the rate both
/// trade constructors pass to `registerTargets`, and the unit `walkDelay` and
/// `sittingTicks` are counted in.
pub const TICKS_SECOND: u32 = 20;

/// `AbstractEntityAIBasic.STANDARD_DELAY` (`:100`), as `crate::ai::worker` fixed it: the
/// rate the base table's dump and pickup rules run at, used here for the hand-over.
pub const STANDARD_DELAY: u32 = BASE_STANDARD_DELAY;

/// The two trades whose education goals this module carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: the two education trades of this slice
pub enum EducationTrade {
    /// `EntityAIWorkResearcher` (`EntityAIWorkResearcher.java:19`), the university.
    Researcher,
    /// `EntityAIWorkTeacher` (`EntityAIWorkTeacher.java:34`), the school.
    Teacher,
}

impl EducationTrade {
    /// The two trades, in the order this module documents them.
    pub const ALL: [EducationTrade; 2] = [EducationTrade::Researcher, EducationTrade::Teacher];

    /// Java's class name, for a log line.
    pub fn class(self) -> &'static str {
        match self {
            EducationTrade::Researcher => "EntityAIWorkResearcher",
            EducationTrade::Teacher => "EntityAIWorkTeacher",
        }
    }

    /// The trade's name, for a message key.
    pub fn name(self) -> &'static str {
        match self {
            EducationTrade::Researcher => "researcher",
            EducationTrade::Teacher => "teacher",
        }
    }

    /// The job this trade is, in `crate::jobs` (`jobs/job.rs:120`, `:130`).
    pub fn job(self) -> Job {
        match self {
            EducationTrade::Researcher => Job::Researcher,
            EducationTrade::Teacher => Job::Teacher,
        }
    }

    /// The first state the trade's own table claims — where `IDLE` hands over.
    ///
    /// Both go through `START_WORKING`, because both constructors register
    /// `IDLE → START_WORKING` and leave the trade to override what `START_WORKING`
    /// does (`EntityAIWorkResearcher.java:50-51`, `EntityAIWorkTeacher.java:76-77`).
    pub fn entry(self) -> EduState {
        EduState::Base(WorkerState::StartWorking)
    }

    /// The line that registers `IDLE → START_WORKING` for this trade.
    pub fn entry_java(self) -> &'static str {
        match self {
            EducationTrade::Researcher => "EntityAIWorkResearcher.java:50",
            EducationTrade::Teacher => "EntityAIWorkTeacher.java:76",
        }
    }

    /// The trade's facts, empty.
    pub fn job_facts(self) -> EduJob {
        match self {
            EducationTrade::Researcher => EduJob::Researcher(ResearcherFacts::default()),
            EducationTrade::Teacher => EduJob::Teacher(TeacherFacts::default()),
        }
    }

    /// `getActionsDoneUntilDumping()` for this trade, which
    /// `AbstractEntityAIBasic.inventoryNeedsDump` reads (`:413`).
    pub fn actions_until_dumping(self) -> u32 {
        match self {
            // The base class's answer: the researcher does not override it
            // (`AbstractEntityAIBasic.java:425-428`).
            EducationTrade::Researcher => crate::ai::worker::ACTIONS_UNTIL_DUMP,
            // `return 50;` (`EntityAIWorkTeacher.java:200-204`).
            EducationTrade::Teacher => teacher::ACTIONS_UNTIL_DUMPING,
        }
    }

    /// Register this trade's targets — the body of its constructor's
    /// `super.registerTargets(...)` call.
    pub fn register(self, machine: &mut Machine<EduState, EduScene>) {
        register_handover(machine, self);
        match self {
            EducationTrade::Researcher => researcher::register(machine),
            EducationTrade::Teacher => teacher::register(machine),
        }
    }
}

impl fmt::Display for EducationTrade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The states `trade`'s machine can be in: its own trade states plus the seven base
/// states the hand-over enters.
pub fn states_of(trade: EducationTrade) -> Vec<EduState> {
    let mut states: Vec<EduState> = EduState::ALL
        .iter()
        .copied()
        .filter(|state| state.belongs_to(trade))
        .collect();
    for base in [
        WorkerState::Idle,
        WorkerState::StartWorking,
        WorkerState::Decide,
        WorkerState::InventoryFull,
        WorkerState::NeedsItem,
        WorkerState::Paused,
        WorkerState::GatheringRequiredMaterials,
    ] {
        states.push(EduState::Base(base));
    }
    states
}

/// The seam between an education table and the base table: one transition per base
/// state these trades enter or leave, each citing the base line that owns it.
///
/// **This is not a second `register_base`.** `crate::ai::worker` already carries the
/// fourteen base targets over `WorkerState`, and an education machine cannot hold them
/// (different state type). What is registered here is only the way *back*.
pub fn register_handover(machine: &mut Machine<EduState, EduScene>, trade: EducationTrade) {
    use WorkerState::*;
    // `dumpInventory` ends in `IDLE` (or `PAUSED`) — `afterDump`
    // (`AbstractEntityAIBasic.java:1213-1222`).
    machine.add(Transition::goto(
        EduState::Base(InventoryFull),
        EduState::Base(Idle),
        "afterDump",
        "AbstractEntityAIBasic.java:1213-1222",
        STANDARD_DELAY,
    ));
    // `waitForRequests` answers `afterRequestPickUp()`, `IDLE` in the base class
    // (`AbstractEntityAIBasic.java:238`, `:736-739`).
    machine.add(Transition::goto(
        EduState::Base(NeedsItem),
        EduState::Base(Idle),
        "waitForRequests",
        "AbstractEntityAIBasic.java:736-739",
        STANDARD_DELAY,
    ));
    // `PAUSED` → `IDLE` when the player unpauses (`AbstractEntityAIBasic.java:251`).
    machine.add(Transition::goto(
        EduState::Base(Paused),
        EduState::Base(Idle),
        "bePaused",
        "AbstractEntityAIBasic.java:251",
        TICKS_SECOND,
    ));
    // `getNeededItem` ends in `getStateAfterPickUp()`, `START_WORKING` in the base class
    // (`AbstractEntityAIBasic.java:242`, `:323-326`) — this is the teacher's paper fetch.
    machine.add(Transition::goto(
        EduState::Base(GatheringRequiredMaterials),
        EduState::Base(StartWorking),
        "getStateAfterPickUp",
        "AbstractEntityAIBasic.java:323-326",
        STANDARD_DELAY,
    ));
    // `IDLE` leaves for the trade's entry: every concrete AI registers this, which is why
    // the base class is abstract (`AbstractEntityAIBasic.java:204-260`).
    machine.add(Transition::goto(
        EduState::Base(Idle),
        trade.entry(),
        "idleToEntry",
        trade.entry_java(),
        1,
    ));
}

/// What one pass asks the caller to do — an order, not a fact and not a memory.
///
/// Java performs these in place, with the citizen, the level and the colony in hand:
/// it draws a research and calls `research()` (`EntityAIWorkResearcher.java:88-94`), it
/// spawns a `SittingEntity` (`:136`), it moves one sheet of paper between two
/// inventories (`EntityAIWorkTeacher.java:155-159`) and it adds experience to a second
/// citizen (`:184-198`). None of that is a decision, so each one leaves the machine as an
/// [`Effect`] and the caller either carries it out or drops it.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: the six orders these two trades can give
pub enum Effect {
    /// One study pass: `addExperience(XP_PER_STUDYPOS)` (`EntityAIWorkResearcher.java:100`)
    /// plus, when the pass advanced one, the research to hand to
    /// `crate::research::Progress::advance` (`:88-96`).
    Studied {
        /// `XP_PER_STUDYPOS` (`:29`), paid on every pass.
        xp: f64,
        /// The research this pass advances, `None` when nothing was running or the mana
        /// was dry (`:84`).
        research: Option<ResearchAdvance>,
    },
    /// `requestPaper` (`EntityAIWorkTeacher.java:209-216`): one `Stack` of
    /// [`teacher::PAPER_TO_REQUEST`], created only when no such request is open.
    RequestPaper {
        /// The stack size of the request (`:214`).
        count: u32,
    },
    /// `needsCurrently = new Tuple<>(PAPER, PAPER_TO_REQUEST)` (`:100`): the base
    /// class's fetch, which walks to a chest and takes the stack.
    FetchPaper {
        /// `PAPER_TO_REQUEST` (`:39`).
        count: u32,
    },
    /// `SittingEntity.sitDown(pos, worker, maxSittingTicks * 20)` (`:136`).
    Sit {
        /// `worker.blockPosition()` — where the seat is spawned (`:136`).
        at: Pos,
        /// `maxSittingTicks * 20`, in **game** ticks (`:136`); the counter itself is in
        /// seconds.
        ticks: u32,
    },
    /// One sheet from the teacher to the pupil
    /// (`transferXOfFirstSlotInItemHandlerWithIntoNextFreeSlotInItemHandler`, `:155-159`).
    GivePaper {
        /// The pupil's citizen id.
        pupil: u32,
        /// [`teacher::PAPER_PER_LESSON`] = 1 (`:158`).
        count: u32,
    },
    /// `teachPupil` (`:184-198`) and the teacher's own `addExperience(0.1)` (`:169`).
    Taught {
        /// The pupil's citizen id.
        pupil: u32,
        /// `teach_xp(...)` (`:162-166`), added to the **pupil's** Intelligence.
        pupil_xp: f64,
        /// The teacher's own share (`:169`).
        teacher_xp: f64,
    },
}

/// The research a study pass advances, and whether that pass finishes it.
///
/// Java (`EntityAIWorkResearcher.java:86-94`) draws one record out of
/// `getResearchInProgress()`, calls `research(...)` on it and, when that answered true,
/// `building.onSuccess(research)`. The port names both halves so the caller can do them
/// in that order: `crate::research::Progress::advance(tree, &id, &mut effects)` answers
/// the same bool `concludes` predicts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResearchAdvance {
    /// `research.getId()`, the key `Progress::advance` takes.
    pub id: String,
    /// `LocalResearch.research(...)` would answer `true` (`LocalResearch.java:56-75`).
    pub concludes: bool,
}

/// What one pass decided, in the three halves Java keeps apart: the state it returns
/// (`return getState()` when it can go no further), the branch that refused, and the
/// orders it gives.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Decision {
    /// The state the pass returns, `None` for "stay in the state we are in" — the same
    /// shape as `Action::Run`'s `Option`.
    pub next: Option<EduState>,
    /// Why it stopped, or which side of the pass was degraded.
    pub refusal: Option<EduRefusal>,
    /// What the caller must do, in the order Java did it.
    pub effects: Vec<Effect>,
}

impl Decision {
    /// Move to `state`.
    pub fn go(state: EduState) -> Decision {
        Decision {
            next: Some(state),
            refusal: None,
            effects: Vec::new(),
        }
    }

    /// `return getState()`: no state change, no complaint.
    pub fn stay() -> Decision {
        Decision::default()
    }

    /// `return getState()` with the reason recorded.
    pub fn refuse(refusal: EduRefusal) -> Decision {
        Decision {
            next: None,
            refusal: Some(refusal),
            effects: Vec::new(),
        }
    }

    /// Move to `state`, keeping the refusal and the effects already recorded.
    pub fn going(mut self, state: EduState) -> Decision {
        self.next = Some(state);
        self
    }

    /// Record a reason, if none is recorded yet: Java's first failing branch is the one
    /// a player should see, and the paper request of `decide` runs before the pupil
    /// search of the same pass.
    pub fn because(mut self, refusal: EduRefusal) -> Decision {
        if self.refusal.is_none() {
            self.refusal = Some(refusal);
        }
        self
    }

    /// Add an order for the caller.
    pub fn effect(mut self, effect: Effect) -> Decision {
        self.effects.push(effect);
        self
    }
}

/// What one tick of an education AI decided: the goal now in force, the rule that chose
/// it, the refusal if it went no further, and the orders it left.
#[derive(Clone, Debug, PartialEq)]
pub struct EduOutcome {
    /// The goal in force after this tick.
    pub state: EduState,
    /// The transition that ran, if one did.
    pub fired: Option<Fired<EduState>>,
    /// The refusal recorded during this tick, if any.
    pub refusal: Option<EduRefusal>,
    /// The orders this tick left for the caller.
    pub effects: Vec<Effect>,
}

impl EduOutcome {
    /// The goal in force.
    pub fn goal(&self) -> EduState {
        self.state
    }

    /// Why nothing moved, if the tick recorded a reason.
    pub fn why_not(&self) -> Option<EduRefusal> {
        self.refusal
    }

    /// Whether this tick changed the goal.
    pub fn moved(&self) -> bool {
        matches!(self.fired, Some(fired) if fired.from != fired.to)
    }

    /// Whether the machine fell into a trap state (`BasicStateMachine.java:220-228`).
    pub fn faulted(&self) -> bool {
        matches!(self.fired, Some(fired) if fired.reset)
    }

    /// The Java line of the rule that ran, for a log line.
    pub fn why(&self) -> Option<&'static str> {
        self.fired.map(|fired| fired.java)
    }
}

/// One education AI: a trade and its machine, nothing else.
pub struct EduAi {
    trade: EducationTrade,
    machine: Machine<EduState, EduScene>,
}

impl fmt::Debug for EduAi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EduAi")
            .field("trade", &self.trade)
            .field("machine", &self.machine)
            .finish()
    }
}

impl EduAi {
    /// The trade's AI, starting in `IDLE` at `ENTITY_AI_TICKRATE`
    /// (`AbstractEntityCitizen.java:64`).
    ///
    /// Java starts in `INIT`, and `initSafetyChecks` (`AbstractEntityAIBasic.java:446-465`)
    /// sends it to `IDLE`; `INIT` and those checks belong to `crate::ai`, so this machine
    /// starts where that one arrives.
    pub fn new(trade: EducationTrade) -> EduAi {
        let mut machine = Machine::new(EduState::Base(WorkerState::Idle), ENTITY_AI_TICKRATE);
        trade.register(&mut machine);
        EduAi { trade, machine }
    }

    /// A scene on `world` carrying this trade's facts, empty.
    pub fn scene(&self, world: WorldFacts) -> EduScene {
        EduScene::new(world, self.trade.job_facts())
    }

    /// A scene on `world` and `job`. A researcher handed a teacher's facts answers
    /// [`EduRefusal::NoFactsForTrade`] on its first pass instead of pretending to have
    /// no research running.
    pub fn scene_with(&self, world: WorldFacts, job: EduJob) -> EduScene {
        EduScene::new(world, job)
    }

    /// The trade this AI belongs to.
    pub fn trade(&self) -> EducationTrade {
        self.trade
    }

    /// The state the machine is in (`AbstractAISkeleton.getState`, `:120-126`).
    pub fn state(&self) -> EduState {
        self.machine.state()
    }

    /// The last trap, if a tick fell into one (`BasicStateMachine.java:220-228`).
    pub fn missing(&self) -> Option<crate::ai::state::MissingTransition<EduState>> {
        self.machine.missing()
    }

    /// `AbstractAISkeleton.resetAI` (`:145-152`).
    pub fn reset(&mut self) {
        self.machine.reset();
    }

    /// The machine itself, for a caller that wants to read the table.
    pub fn machine(&self) -> &Machine<EduState, EduScene> {
        &self.machine
    }

    /// One AI tick (`AbstractAISkeleton.tick`, `:63-66`).
    pub fn tick(&mut self, scene: &mut EduScene) -> EduOutcome {
        scene.refusal = None;
        scene.effects.clear();
        let fired = self.machine.tick(scene);
        EduOutcome {
            state: self.machine.state(),
            fired,
            refusal: scene.refusal,
            effects: core::mem::take(&mut scene.effects),
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

/// `inventoryNeedsDump` (`AbstractEntityAIBasic.java:409-416`) for an education scene.
///
/// **Restated, because it is not reachable.** The AI module has this rule as a private
/// function over its own `Scene` (`crate::ai::worker::inventory_needs_dump`). The
/// conditions are the same ones, minus the two that never apply here: neither trade is a
/// `JobDeliveryman` (`:415`), and both education states answer `isOkayToEat = true`
/// (`AIWorkerState.java:439`, `:622`), so `canBeInterrupted()` never refuses the dump.
pub fn dump_due(trade: EducationTrade, scene: &EduScene) -> bool {
    scene.facts.inventory.full // :412
        || scene.facts.inventory.actions_done >= trade.actions_until_dumping() // :413
}

/// `new AITarget(state, action, tickRate)` — the shape both trade constructors use
/// (`AITarget.java:24-53`), written once because both tables add the same kind of
/// transition.
pub(crate) fn add_target(
    machine: &mut Machine<EduState, EduScene>,
    state: EduState,
    rule: &'static str,
    java: &'static str,
    tick_rate: u32,
    action: fn(&mut EduScene, EduState) -> Option<EduState>,
) {
    machine.add(Transition::target(
        state,
        rule,
        java,
        tick_rate,
        Action::Run(action),
    ));
}

/// The `Action::Run` shape every trade pass has: split the scene into its fields, run the
/// decision on the trade's own facts, record the refusal and the orders.
///
/// Written as a macro because the borrow shape is the fiddly part — the trade facts and
/// the memory are two fields of one struct, and only a destructuring borrow lets the
/// decision read the first while writing the second.
///
/// **Every name is `$crate`-qualified on purpose.** `macro_rules!` resolves paths at the
/// *expansion* site, so a bare `EduScene` here would only compile in a module that
/// happens to import it. A qualified body needs nothing from its caller.
macro_rules! edu_action {
    ($name:ident, $job:ident, $decide:path) => {
        fn $name(
            scene: &mut $crate::ai_work_06::EduScene,
            state: $crate::ai_work_06::EduState,
        ) -> Option<$crate::ai_work_06::EduState> {
            let $crate::ai_work_06::EduScene {
                facts,
                job,
                memory,
                refusal,
                effects,
            } = scene;
            let $crate::ai_work_06::EduJob::$job(trade_facts) = job else {
                *refusal = Some($crate::ai_work_06::EduRefusal::NoFactsForTrade);
                return Some(state);
            };
            let decision: $crate::ai_work_06::Decision = $decide(facts, trade_facts, memory);
            *refusal = decision.refusal;
            *effects = decision.effects;
            decision.next.or(Some(state))
        }
    };
}

pub(crate) use edu_action;

/// The table tests: the trade mapping, the hand-over, and the decision builder. The
/// *scenarios* — facts ticked into a state sequence — are in `tests.rs`, next to the
/// module's own file.
#[cfg(test)]
mod trade_tests {
    use super::*;
    use crate::ai::facts::Pos;

    #[test]
    fn each_trade_names_its_java_class_and_its_job() {
        assert_eq!(EducationTrade::Researcher.class(), "EntityAIWorkResearcher");
        assert_eq!(EducationTrade::Teacher.class(), "EntityAIWorkTeacher");
        assert_eq!(EducationTrade::Researcher.job(), Job::Researcher);
        assert_eq!(EducationTrade::Teacher.job(), Job::Teacher);
        assert_eq!(EducationTrade::ALL.len(), 2);
        assert_eq!(EducationTrade::Teacher.to_string(), "teacher");
        // Both constructors register the hand-over at their own line.
        assert_eq!(
            EducationTrade::Researcher.entry_java(),
            "EntityAIWorkResearcher.java:50"
        );
        assert_eq!(
            EducationTrade::Teacher.entry_java(),
            "EntityAIWorkTeacher.java:76"
        );
        for trade in EducationTrade::ALL {
            assert_eq!(trade.entry(), EduState::Base(WorkerState::StartWorking));
        }
    }

    #[test]
    fn the_dump_threshold_is_the_base_thirty_two_for_the_researcher_and_fifty_for_the_teacher() {
        // `AbstractEntityAIBasic.java:425-428` versus `EntityAIWorkTeacher.java:200-204`.
        assert_eq!(EducationTrade::Researcher.actions_until_dumping(), 32);
        assert_eq!(EducationTrade::Teacher.actions_until_dumping(), 50);
        assert_eq!(teacher::ACTIONS_UNTIL_DUMPING, 50);

        let mut scene = EduScene::new(WorldFacts::new(), EduJob::Teacher(TeacherFacts::default()));
        assert!(!dump_due(EducationTrade::Teacher, &scene), "nothing done yet");
        scene.facts.inventory.actions_done = 49;
        assert!(!dump_due(EducationTrade::Teacher, &scene));
        scene.facts.inventory.actions_done = 50;
        assert!(dump_due(EducationTrade::Teacher, &scene));
        // Forty actions is **over** the base threshold and **under** the teacher's: the
        // same counter answers differently for the two trades (`:413` is a `>=`).
        let scene = EduScene::new(
            WorldFacts {
                inventory: crate::ai::facts::InventoryFacts {
                    actions_done: 40,
                    ..Default::default()
                },
                ..WorldFacts::new()
            },
            EduJob::Researcher(ResearcherFacts::default()),
        );
        assert!(dump_due(EducationTrade::Researcher, &scene), "40 >= 32");
        assert!(!dump_due(EducationTrade::Teacher, &scene), "40 < 50");
        // A full inventory dumps whatever the counter says (`:412`).
        let mut full = scene.clone();
        full.facts.inventory.full = true;
        assert!(dump_due(EducationTrade::Researcher, &full));
    }

    #[test]
    fn the_hand_over_gives_idle_one_transition_and_the_trade_entry_another() {
        // Five hand-over rules plus the trade's own targets, and `IDLE` must leave only
        // for the entry.
        for trade in EducationTrade::ALL {
            let ai = EduAi::new(trade);
            let machine = ai.machine();
            assert!(
                machine.can_leave(EduState::Base(WorkerState::Idle)),
                "{trade} has no way out of IDLE"
            );
            assert!(machine.can_leave(EduState::Base(WorkerState::InventoryFull)));
            assert!(machine.can_leave(EduState::Base(WorkerState::NeedsItem)));
            assert!(machine.can_leave(EduState::Base(WorkerState::Paused)));
            assert!(
                machine.can_leave(EduState::Base(WorkerState::GatheringRequiredMaterials)),
                "the teacher's paper fetch must come back"
            );
            // Five hand-over transitions, and the trade's own on top.
            let handover = machine
                .transitions()
                .iter()
                .filter(|transition| transition.state == Some(EduState::Base(WorkerState::Idle)))
                .count();
            assert_eq!(handover, 1, "{trade}");
        }
        let researcher = EduAi::new(EducationTrade::Researcher);
        assert_eq!(researcher.machine().transitions().len(), 5 + 2);
        let teacher = EduAi::new(EducationTrade::Teacher);
        assert_eq!(teacher.machine().transitions().len(), 5 + 3);
    }

    #[test]
    fn a_trade_action_on_the_other_trade_s_facts_refuses_by_name() {
        // The macro's `let … else`: a teacher AI handed a researcher's job facts. The
        // first tick walks out of `IDLE`, the second reaches the trade target.
        let mut ai = EduAi::new(EducationTrade::Teacher);
        let mut scene = ai.scene_with(
            WorldFacts::new(),
            EduJob::Researcher(ResearcherFacts::default()),
        );
        let walking = ai.tick(&mut scene);
        assert_eq!(walking.state, EduState::Base(WorkerState::StartWorking));
        let wrong = ai.tick(&mut scene);
        assert_eq!(wrong.refusal, Some(EduRefusal::NoFactsForTrade));
        assert!(!wrong.moved(), "the state holds at START_WORKING");
        assert!(wrong.effects.is_empty());
        assert!(EduRefusal::NoFactsForTrade.java().starts_with("port:"));
        // The scene really does carry the other trade's facts.
        assert!(scene.researcher().is_some());
        assert!(scene.teacher().is_none());
    }

    #[test]
    fn states_of_lists_the_trade_state_then_the_seven_base_ones() {
        let researcher = states_of(EducationTrade::Researcher);
        assert_eq!(researcher.len(), 8);
        assert!(researcher.contains(&EduState::Study));
        assert!(!researcher.contains(&EduState::Teach));
        let teacher = states_of(EducationTrade::Teacher);
        assert_eq!(teacher.len(), 8);
        assert!(teacher.contains(&EduState::Teach));
        assert!(!teacher.contains(&EduState::Study));
    }

    #[test]
    fn the_decision_builder_records_effects_in_order_and_keeps_the_first_reason() {
        let decision = Decision::stay()
            .effect(Effect::FetchPaper { count: 16 })
            .effect(Effect::Sit {
                at: Pos::new(1, 2, 3),
                ticks: 40,
            })
            .because(EduRefusal::PaperInHutOnly)
            .because(EduRefusal::NoPupil)
            .going(EduState::Teach);
        assert_eq!(decision.next, Some(EduState::Teach));
        assert_eq!(decision.refusal, Some(EduRefusal::PaperInHutOnly));
        assert_eq!(decision.effects.len(), 2);
        assert_eq!(
            Decision::refuse(EduRefusal::Sitting).next,
            None,
            "`return getState()`"
        );
    }

    #[test]
    fn the_two_huts_are_measured_the_same_way_walk_to_building_is() {
        // `AbstractEntityAIBasic.java:757-766` — `WOKR_IN_BUILDING_DIST` = 7, strict.
        let facts = WorldFacts::new();
        assert!(at_hut(&facts));
        let mut near = facts.clone();
        near.worker.at = Pos::new(6, 64, 0);
        assert!(near.worker.at.is_living_at_site(near.building.position, 7));
        let mut far = facts;
        far.worker.at = Pos::new(7, 64, 0);
        assert!(!at_hut(&far), "7 blocks away is not inside a 7-block radius");
    }
}
