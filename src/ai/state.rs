//! The tick-rate state machine a job's AI runs on: states, transitions, the order
//! they are checked in, and how long each one waits between two checks.
//!
//! Java reference: `api/entity/ai/statemachine/basestatemachine/BasicStateMachine.java:
//! 189-231` (the tick order of a plain machine), `:300-370` (`transitionToNext`, the
//! trap-state reset), `:55-135` (`addTransition`, `addTransitionGroup` — a group goes
//! **first** in every state it covers), `api/entity/ai/statemachine/tickratestatemachine/
//! TickRateStateMachine.java:95-150` (the three global lists, then the current state's,
//! first hit wins), `TickingTransition.java:39-77` (`tickRate` clamped to `1..
//! MAX_AI_TICKRATE`, countdown), `:150-160` (`countdownTicksToUpdate`,
//! `setTicksToUpdate`), `TickRateConstants.java:9-19` (the three constants),
//! `api/entity/ai/statemachine/states/AIWorkerState.java` (the states and their
//! `isOkayToEat` flag) and `AbstractEntityCitizen.java:64` (`ENTITY_AI_TICKRATE`).
//!
//! **One tick, four passes, first hit wins.** `TickRateStateMachine.tick`
//! (`:95-150`) walks the `AI_BLOCKING` list, then the `EVENT` list, then the
//! `STATE_BLOCKING` list, then the list of the state we are in, and *returns* at the
//! first transition that moves the machine. [`Machine::tick`] is that loop, and
//! [`Blocking::ORDER`] is the pass order written down once.
//!
//! **A transition that returns no state does not win.** Java's
//! `checkTransition` answers `transitionToNext(...)`, which is `true` only when the
//! supplier returned a non-null state (`BasicStateMachine.java:340-368`): an action
//! that returns `null` ("I did my work, no state change") lets the *next* transition
//! of the same tick run. That is why [`Action::Fixed`] has a `None` case and why
//! [`Fired`] is only produced for a real `Some(state)`.
//!
//! **The countdown is not a clock.** A transition built with `tick_rate = 20` is
//! *looked at* every tick but only *run* when its counter reaches zero; the counter
//! is decremented by the machine's tick rate on every look
//! (`TickingTransition.java:150-158`). The port keeps that counter inside
//! [`Transition`], so `tick` twice with no game time and the second call is a no-op.
//!
//! **What is dropped, and why.** `TickingTransition`'s static `tickOffsetVariant`
//! (`:26-37`, `:63-70`) spreads a new transition's first check over the next fifty
//! ticks, so that fifty AIs built in the same tick do not all wake on the same one.
//! It is load spreading, not a rule any decision depends on, and a hidden static
//! would make a test's result depend on how many transitions other tests built —
//! so every ported transition starts at `0`. `slownessFactor` (`:22`) is the
//! lag-compensation knob (a server running slow divides the next delay); the port
//! runs it at `1.0`, its default, and says so here rather than silently ignoring it.
//! Java also catches `RuntimeException` around a condition and a supplier
//! (`BasicStateMachine.java:299-320`); Rust has no exceptions to catch, and a
//! panicking condition is a bug in this crate, not a game event.

use core::fmt;

/// `AbstractEntityCitizen.ENTITY_AI_TICKRATE` (`AbstractEntityCitizen.java:64`):
/// how many game ticks one AI tick covers.
pub const ENTITY_AI_TICKRATE: u32 = 5;

/// `TickRateConstants.MAX_AI_TICKRATE` (`:14`): `20 * 60 * 10`, ten minutes of ticks.
/// A `tickRate` above this is clamped down to it (`TickingTransition.java:53-54`).
pub const MAX_AI_TICKRATE: u32 = 20 * 60 * 10;

/// `TickRateConstants.MAX_TICKRATE_VARIANT` (`:19`), the spread Java uses for the
/// first check of each transition. Kept as a number because it is cited; see the
/// module note for why it is not applied.
pub const MAX_TICKRATE_VARIANT: u32 = 50;

/// Which of the three global passes a transition belongs to.
///
/// Java reference: `api/entity/ai/statemachine/states/AIBlockingEventType.java:7-24`.
/// `AI_BLOCKING` is "checked regardless of the current state, and if it runs it stops
/// the tick"; `STATE_BLOCKING` is the same but checked *after* the state-less events;
/// `EVENT` is the one that may consume itself. The port keeps the three names and the
/// order, and drops the self-consumption (`AIOneTimeEventTarget`), which no worker AI
/// in `core/entity/ai/workers/` registers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: three event types in Java, and a fourth would need a second pass
pub enum Blocking {
    /// `AI_BLOCKING`, checked first (`TickRateStateMachine.java:99-108`).
    AiBlocking,
    /// `EVENT`, checked second (`:110-117`).
    Event,
    /// `STATE_BLOCKING`, checked third, just before the state's own list (`:119-126`).
    StateBlocking,
}

impl Blocking {
    /// The three passes, in the order one tick makes them.
    pub const ORDER: [Blocking; 3] = [
        Blocking::AiBlocking,
        Blocking::Event,
        Blocking::StateBlocking,
    ];

    /// The rank of this pass in one tick, `0` first.
    pub fn rank(self) -> usize {
        match self {
            Blocking::AiBlocking => 0,
            Blocking::Event => 1,
            Blocking::StateBlocking => 2,
        }
    }

    /// Java's name for it, for a log line or a test failure.
    pub fn java_name(self) -> &'static str {
        match self {
            Blocking::AiBlocking => "AI_BLOCKING",
            Blocking::Event => "EVENT",
            Blocking::StateBlocking => "STATE_BLOCKING",
        }
    }
}

/// The states this port gives a name to, out of `AIWorkerState`'s 127.
///
/// Java reference: `api/entity/ai/statemachine/states/AIWorkerState.java`. Each
/// variant carries the `isOkayToEat` flag Java passes to its constructor, because
/// that flag is not decoration: `AbstractAISkeleton.canBeInterrupted()` reads it
/// (`AbstractAISkeleton.java:137-140`) and `inventoryNeedsDump` refuses to dump
/// while it is false (`AbstractEntityAIBasic.java:411`).
///
/// **Eighteen of 127.** The eighteen are the general states a worker's base class
/// drives, the four the builder's structure AI adds and the three the composter
/// adds; the other 109 belong to jobs whose AI is not ported (see
/// `RAPPORT.md` §3). A job that is not ported gets no table, not a table of
/// guesses — [`super::JobAi::for_job`] says so by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
// exhaustive: the eighteen states this port gives a table to
pub enum WorkerState {
    /// `IDLE` (`AIWorkerState.java:16`) — "start working from here".
    Idle,
    /// `INIT` (`:20`) — the null check, then `IDLE`.
    Init,
    /// `INVENTORY_FULL` (`:24`) — walk to the hut and dump.
    InventoryFull,
    /// `PREPARING` (`:28`) — not used by this port's tables, kept for `ALL`.
    Preparing,
    /// `START_WORKING` (`:32`) — walk to the building, then decide.
    StartWorking,
    /// `NEEDS_ITEM` (`:36`) — wait for the request system.
    NeedsItem,
    /// `START_BUILDING` (`:40`) — the builder's iterator, stage zero.
    StartBuilding,
    /// `MINE_BLOCK` (`:44`) — the builder breaks a block the blueprint wants gone.
    MineBlock,
    /// `LOAD_STRUCTURE` (`:48`) — read the blueprint, then request its materials.
    LoadStructure,
    /// `BUILDING_STEP` (`:52`) — place (or remove) one structure step.
    BuildingStep,
    /// `COMPLETE_BUILD` (`:56`) — finish the work order.
    CompleteBuild,
    /// `DECIDE` (`:60`) — "what to do next", the general decide state.
    Decide,
    /// `PAUSED` (`:64`) — the player paused this worker.
    Paused,
    /// `GATHERING_REQUIRED_MATERIALS` (`:196`) — walk to a chest and take the stack.
    GatheringRequiredMaterials,
    /// `PICK_UP` (`:130`) — the builder tops its inventory up from the hut.
    PickUp,
    /// `COMPOSTER_FILL` (`:292`) — put compostables into a barrel.
    ComposterFill,
    /// `COMPOSTER_HARVEST` (`:297`) — take the compost out of a finished barrel.
    ComposterHarvest,
    /// `GET_MATERIALS` (`:302`) — the composter asks for its compostables.
    GetMaterials,
}

impl WorkerState {
    /// Every state this port names, in `AIWorkerState`'s declaration order.
    pub const ALL: [WorkerState; 18] = [
        WorkerState::Idle,
        WorkerState::Init,
        WorkerState::InventoryFull,
        WorkerState::Preparing,
        WorkerState::StartWorking,
        WorkerState::NeedsItem,
        WorkerState::StartBuilding,
        WorkerState::MineBlock,
        WorkerState::LoadStructure,
        WorkerState::BuildingStep,
        WorkerState::CompleteBuild,
        WorkerState::Decide,
        WorkerState::Paused,
        WorkerState::GatheringRequiredMaterials,
        WorkerState::PickUp,
        WorkerState::ComposterFill,
        WorkerState::ComposterHarvest,
        WorkerState::GetMaterials,
    ];

    /// Java's identifier, which is what a state history or a nametag shows
    /// (`AbstractEntityAIBasic.java:475` puts it on the citizen's nametag).
    pub fn java_name(self) -> &'static str {
        match self {
            WorkerState::Idle => "IDLE",
            WorkerState::Init => "INIT",
            WorkerState::InventoryFull => "INVENTORY_FULL",
            WorkerState::Preparing => "PREPARING",
            WorkerState::StartWorking => "START_WORKING",
            WorkerState::NeedsItem => "NEEDS_ITEM",
            WorkerState::StartBuilding => "START_BUILDING",
            WorkerState::MineBlock => "MINE_BLOCK",
            WorkerState::LoadStructure => "LOAD_STRUCTURE",
            WorkerState::BuildingStep => "BUILDING_STEP",
            WorkerState::CompleteBuild => "COMPLETE_BUILD",
            WorkerState::Decide => "DECIDE",
            WorkerState::Paused => "PAUSED",
            WorkerState::GatheringRequiredMaterials => "GATHERING_REQUIRED_MATERIALS",
            WorkerState::PickUp => "PICK_UP",
            WorkerState::ComposterFill => "COMPOSTER_FILL",
            WorkerState::ComposterHarvest => "COMPOSTER_HARVEST",
            WorkerState::GetMaterials => "GET_MATERIALS",
        }
    }

    /// `AIWorkerState.isOkayToEat` for this state — the flag
    /// `AbstractAISkeleton.canBeInterrupted()` returns (`AbstractAISkeleton.java:
    /// 137-140`).
    ///
    /// It is `true` for the states a citizen may walk away from (idle, walking,
    /// waiting) and `false` for the ones that must finish (dumping, building,
    /// mining, placing). Read it as "may the citizen go eat, sleep or see a doctor
    /// right now".
    pub fn okay_to_eat(self) -> bool {
        match self {
            // AIWorkerState.java:16, :20, :28, :32, :36, :60, :64, :196, :302
            WorkerState::Idle
            | WorkerState::Init
            | WorkerState::Preparing
            | WorkerState::StartWorking
            | WorkerState::NeedsItem
            | WorkerState::Decide
            | WorkerState::Paused
            | WorkerState::GatheringRequiredMaterials
            | WorkerState::GetMaterials
            | WorkerState::ComposterFill
            | WorkerState::ComposterHarvest => true,
            // :24, :40, :44, :48, :52, :56, :130
            WorkerState::InventoryFull
            | WorkerState::StartBuilding
            | WorkerState::MineBlock
            | WorkerState::LoadStructure
            | WorkerState::BuildingStep
            | WorkerState::CompleteBuild
            | WorkerState::PickUp => false,
        }
    }
}

impl fmt::Display for WorkerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.java_name())
    }
}

/// When a transition is allowed to run, i.e. Java's `IBooleanConditionSupplier`.
///
/// Java reference: `tickratestatemachine/IBooleanConditionSupplier.java:5-12`.
/// `AITarget`'s short constructors pass `() -> true` (`AITarget.java:75-86`), which
/// is [`Condition::Always`].
pub enum Condition<S, F> {
    /// `() -> true` — the state's own list decides.
    Always,
    /// A function of the facts **and of the state the machine is in**, e.g.
    /// `AbstractEntityAIBasic::inventoryNeedsDump`, whose first test is
    /// `getState() != INVENTORY_FULL` (`AbstractEntityAIBasic.java:411`). Java's
    /// predicate calls `this.getState()`; here the state is handed in, so a condition
    /// cannot read a stale copy.
    When(fn(&F, S) -> bool),
}

// Hand-written rather than derived: `#[derive(Copy)]` would also demand `F: Copy`,
// and the facts a job's AI reads (a `Vec` of barrels, a string) are not.
impl<S, F> Clone for Condition<S, F> {
    fn clone(&self) -> Condition<S, F> {
        *self
    }
}

impl<S, F> Copy for Condition<S, F> {}

impl<S: Copy, F> Condition<S, F> {
    /// Evaluate it. `Always` never looks at the facts, so it cannot fail.
    pub fn holds(&self, facts: &F, state: S) -> bool {
        match self {
            Condition::Always => true,
            Condition::When(predicate) => predicate(facts, state),
        }
    }
}

/// What a transition does when it runs, i.e. Java's `IStateSupplier` plus the two
/// ways a worker AI uses it.
///
/// Java reference: `tickratestatemachine/IStateSupplier.java:5-12`. The distinction
/// between the two variants is Java's own: `new AITarget(IDLE, START_WORKING, 10)`
/// (`AbstractEntityAIStructure.java:186`) is a constant supplier, while
/// `this::decideWhatToDo` (`EntityAIWorkComposter.java:99`) is a function that reads
/// the world.
///
/// `None` is Java's `return null`: the action ran, it changed nothing, and the tick
/// goes on with the next transition (`BasicStateMachine.java:340-368`).
pub enum Action<S, F> {
    /// A constant next state — Java's `() -> STATE`.
    Fixed(Option<S>),
    /// A function of the facts; the second argument is the state we are in, which is
    /// what Java's `this::getState` suppliers read
    /// (`AbstractEntityAIBasic.java:205`, `:218`).
    Run(fn(&mut F, S) -> Option<S>),
}

impl<S: Copy, F> Clone for Action<S, F> {
    fn clone(&self) -> Action<S, F> {
        *self
    }
}

impl<S: Copy, F> Copy for Action<S, F> {}

impl<S: Copy, F> Action<S, F> {
    /// Apply it to the facts and the current state.
    pub fn next(&self, facts: &mut F, from: S) -> Option<S> {
        match self {
            Action::Fixed(state) => *state,
            Action::Run(run) => run(facts, from),
        }
    }
}

/// One target of the AI: the state it belongs to (or none, for a global pass), when
/// it may run, what it does, and how often it is looked at.
///
/// Java reference: `api/entity/ai/statemachine/AITarget.java:20-90` on top of
/// `tickratestatemachine/TickingTransition.java:39-90`.
pub struct Transition<S, F> {
    /// `BasicTransition.state` (`:38-44`): `Some` for an `AITarget`, `None` for an
    /// `AIEventTarget`.
    pub state: Option<S>,
    /// Which global pass, for an `AIEventTarget` (`AIEventTarget.java:20-60`).
    pub blocking: Option<Blocking>,
    /// This port's name for the rule — Java names it from the method reference
    /// (`BasicTransition.java:75-78`); a name is cheaper to assert on.
    pub rule: &'static str,
    /// `fichier.java:ligne`, the line that implements the rule.
    pub java: &'static str,
    /// `TickingTransition.tickRate`: how many ticks of the machine between two runs.
    pub tick_rate: u32,
    /// `TickingTransition.ticksToUpdate`, the countdown.
    ticks_to_update: i32,
    /// `condition` (`BasicTransition.java:40-42`).
    pub condition: Condition<S, F>,
    /// `nextState` (`:46-52`).
    pub action: Action<S, F>,
}

/// Clamp a tick rate the way `TickingTransition`'s two constructors do
/// (`TickingTransition.java:52-57`): at least `1`, at most [`MAX_AI_TICKRATE`].
pub fn clamp_tick_rate(tick_rate: u32) -> u32 {
    tick_rate.clamp(1, MAX_AI_TICKRATE)
}

impl<S: Copy, F> Clone for Transition<S, F> {
    fn clone(&self) -> Transition<S, F> {
        *self
    }
}

impl<S: Copy, F> Copy for Transition<S, F> {}

impl<S: Copy, F> Transition<S, F> {
    /// An `AITarget(state, action, tickRate)` (`AITarget.java:28-40`, condition
    /// `() -> true`).
    pub fn target(
        state: S,
        rule: &'static str,
        java: &'static str,
        tick_rate: u32,
        action: Action<S, F>,
    ) -> Transition<S, F> {
        Transition {
            state: Some(state),
            blocking: None,
            rule,
            java,
            tick_rate: clamp_tick_rate(tick_rate),
            ticks_to_update: 0,
            condition: Condition::Always,
            action,
        }
    }

    /// Drop the state, which turns an `AITarget` into the shape
    /// `addTransitionGroup` requires (`BasicStateMachine.java:76-81`). Only
    /// [`Machine::add_group`] has a use for it.
    pub fn without_state(mut self) -> Transition<S, F> {
        self.state = None;
        self
    }

    /// An `AITarget(state, predicate, action, tickRate)` (`AITarget.java:28-40`).
    pub fn target_when(
        state: S,
        rule: &'static str,
        java: &'static str,
        tick_rate: u32,
        condition: fn(&F, S) -> bool,
        action: Action<S, F>,
    ) -> Transition<S, F> {
        Transition {
            condition: Condition::When(condition),
            ..Transition::target(state, rule, java, tick_rate, action)
        }
    }

    /// `AITarget(predicateState, state, tickRate)` (`AITarget.java:62-70`): "in this
    /// state, go there", the constant supplier.
    pub fn goto(
        from: S,
        to: S,
        rule: &'static str,
        java: &'static str,
        tick_rate: u32,
    ) -> Transition<S, F> {
        Transition::target(from, rule, java, tick_rate, Action::Fixed(Some(to)))
    }

    /// An `AIEventTarget(eventType, action, tickRate)` (`AIEventTarget.java:48-56`):
    /// checked on every state, in its own pass.
    pub fn event(
        blocking: Blocking,
        rule: &'static str,
        java: &'static str,
        tick_rate: u32,
        action: Action<S, F>,
    ) -> Transition<S, F> {
        Transition {
            state: None,
            blocking: Some(blocking),
            rule,
            java,
            tick_rate: clamp_tick_rate(tick_rate),
            ticks_to_update: 0,
            condition: Condition::Always,
            action,
        }
    }

    /// An `AIEventTarget(eventType, predicate, action, tickRate)`
    /// (`AIEventTarget.java:20-46`).
    pub fn event_when(
        blocking: Blocking,
        rule: &'static str,
        java: &'static str,
        tick_rate: u32,
        condition: fn(&F, S) -> bool,
        action: Action<S, F>,
    ) -> Transition<S, F> {
        Transition {
            condition: Condition::When(condition),
            ..Transition::event(blocking, rule, java, tick_rate, action)
        }
    }

    /// Whether this transition is due on a machine ticking at `rate`, and if so
    /// restart its counter.
    ///
    /// Java reference: `TickingTransition.countdownTicksToUpdate`
    /// (`TickingTransition.java:150-158`) called from
    /// `TickRateStateMachine.checkTransition` (`:114-125`), which decrements by the
    /// machine's tick rate and treats "still positive" as "not yet".
    fn count_down(&mut self, rate: u32) -> bool {
        if self.ticks_to_update > 0 {
            self.ticks_to_update -= rate as i32;
            if self.ticks_to_update > 0 {
                return false;
            }
        }
        // `transition.setTicksToUpdate((int) (getTickRate() / slownessFactor))`
        // (`:122`), with `slownessFactor = 1.0` (`:22`).
        self.ticks_to_update = self.tick_rate as i32;
        true
    }
}

/// One transition that ran, and where it sent the machine.
///
/// Java logs the same thing into a twenty-entry ring buffer
/// (`BasicStateMachine.java:57-60`, `:340-370`) that `onException` prints
/// (`AbstractEntityAIBasic.java:381`); the port returns it instead, so a test can
/// name the rule that decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fired<S> {
    /// The rule that ran, for a message or an assertion.
    pub rule: &'static str,
    /// The Java line it implements.
    pub java: &'static str,
    /// The state the machine was in.
    pub from: S,
    /// The state it is in now. May equal `from`: `this::getState` re-asserts the
    /// current state to hold the machine (`AbstractEntityAIBasic.java:205`).
    pub to: S,
    /// `true` when `to` had no transition list, which Java treats as a trap: it
    /// reports the exception and resets to `INIT` (`BasicStateMachine.java:344-356`).
    pub reset: bool,
}

/// A transition that moved the machine into a state nothing knows how to leave.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissingTransition<S> {
    /// The state that had no transition list.
    pub state: S,
    /// The rule that led there.
    pub rule: &'static str,
}

/// The generic engine: `BasicStateMachine` plus the tick-rate and pass ordering of
/// `TickRateStateMachine`.
///
/// `S` is the state type (here always [`WorkerState`]); `F` is whatever the actions
/// read and write — for this port [`super::Scene`], the facts plus the worker's own
/// working memory.
pub struct Machine<S: Copy + Eq + fmt::Debug, F> {
    state: S,
    init: S,
    tick_rate: u32,
    transitions: Vec<Transition<S, F>>,
    /// One list per state, in registration order (`transitionMap`,
    /// `BasicStateMachine.java:45-52`).
    state_lists: Vec<(S, Vec<usize>)>,
    /// The three global passes (`eventTransitionMap`, `:48`).
    ai_blocking: Vec<usize>,
    events: Vec<usize>,
    state_blocking: Vec<usize>,
    /// `executedTransition` (`TickRateStateMachine.java:30`), needed by
    /// `setCurrentDelay` (`:132-136`).
    executed: Option<usize>,
    missing: Option<MissingTransition<S>>,
}

impl<S: Copy + Eq + fmt::Debug, F> Machine<S, F> {
    /// A machine that starts in `initial`, ticking at `tick_rate`
    /// (`TickRateStateMachine.java:44-63`; a worker passes `ENTITY_AI_TICKRATE`).
    pub fn new(initial: S, tick_rate: u32) -> Machine<S, F> {
        Machine {
            state: initial,
            init: initial,
            tick_rate,
            transitions: Vec::new(),
            state_lists: vec![(initial, Vec::new())],
            ai_blocking: Vec::new(),
            events: Vec::new(),
            state_blocking: Vec::new(),
            executed: None,
            missing: None,
        }
    }

    /// `BasicStateMachine.addTransition` (`:55-70`): a state transition joins its
    /// state's list, a global one joins its pass's list.
    pub fn add(&mut self, transition: Transition<S, F>) {
        let index = self.transitions.len();
        if let Some(state) = transition.state {
            self.list_mut(state).push(index);
        }
        if let Some(blocking) = transition.blocking {
            self.pass_mut(blocking).push(index);
        }
        self.transitions.push(transition);
    }

    /// `BasicStateMachine.addTransitionGroup` (`:76-95`): the transition covers
    /// every state of the group and goes **in front** of each list, so a group rule
    /// is checked before the state's own rules.
    pub fn add_group(&mut self, states: &[S], transition: Transition<S, F>) {
        debug_assert!(
            transition.state.is_none(),
            "a group transition has no state of its own (BasicStateMachine.java:78-81)"
        );
        let index = self.transitions.len();
        if let Some(blocking) = transition.blocking {
            self.pass_mut(blocking).insert(0, index);
        }
        for state in states {
            self.list_mut(*state).insert(0, index);
        }
        self.transitions.push(transition);
    }

    fn list_mut(&mut self, state: S) -> &mut Vec<usize> {
        if let Some(position) = self.state_lists.iter().position(|(known, _)| *known == state) {
            return &mut self.state_lists[position].1;
        }
        self.state_lists.push((state, Vec::new()));
        let last = self.state_lists.len() - 1;
        &mut self.state_lists[last].1
    }

    fn list(&self, state: S) -> Option<&Vec<usize>> {
        self.state_lists
            .iter()
            .find(|(known, _)| *known == state)
            .map(|(_, list)| list)
    }

    fn pass_mut(&mut self, blocking: Blocking) -> &mut Vec<usize> {
        match blocking {
            Blocking::AiBlocking => &mut self.ai_blocking,
            Blocking::Event => &mut self.events,
            Blocking::StateBlocking => &mut self.state_blocking,
        }
    }

    fn pass(&self, blocking: Blocking) -> &Vec<usize> {
        match blocking {
            Blocking::AiBlocking => &self.ai_blocking,
            Blocking::Event => &self.events,
            Blocking::StateBlocking => &self.state_blocking,
        }
    }

    /// The state the machine is in (`AbstractAISkeleton.getState`, `:120-126`).
    pub fn state(&self) -> S {
        self.state
    }

    /// How many ticks one machine tick covers (`getTickRate`, `:129-133`).
    pub fn tick_rate(&self) -> u32 {
        self.tick_rate
    }

    /// `TickRateStateMachine.setTickRate` (`:128-132`).
    pub fn set_tick_rate(&mut self, tick_rate: u32) {
        self.tick_rate = tick_rate.max(1);
    }

    /// `AbstractAISkeleton.resetAI` (`:145-152`) → `BasicStateMachine.reset`
    /// (`:373-380`). Note what it does **not** clear: the transitions' countdowns
    /// and `executed`, exactly like Java.
    pub fn reset(&mut self) {
        self.state = self.init;
        self.missing = None;
    }

    /// The state a reset would go back to.
    pub fn init_state(&self) -> S {
        self.init
    }

    /// `TickRateStateMachine.setCurrentDelay` (`:132-136`): push the transition that
    /// ran last, so that it does not run again for `ticks`.
    ///
    /// Returns the rule it was applied to, or `None` when nothing has run yet — Java
    /// would throw a `NullPointerException` here, which a mod must not do.
    pub fn set_current_delay(&mut self, ticks: u32) -> Option<&'static str> {
        let index = self.executed?;
        self.transitions[index].ticks_to_update = ticks as i32;
        Some(self.transitions[index].rule)
    }

    /// The trap `BasicStateMachine.transitionToNext` reports (`:344-356`), if the
    /// last tick fell into one.
    pub fn missing(&self) -> Option<MissingTransition<S>> {
        self.missing
    }

    /// Every transition this machine holds, in registration order.
    pub fn transitions(&self) -> &[Transition<S, F>] {
        &self.transitions
    }

    /// Whether any transition names `state` — i.e. whether the machine can leave it.
    pub fn can_leave(&self, state: S) -> bool {
        self.list(state).map(|list| !list.is_empty()).unwrap_or(false)
    }

    /// One tick: `TickRateStateMachine.tick` (`:95-150`).
    ///
    /// The three global passes in [`Blocking::ORDER`], then the current state's own
    /// list; the first transition that produces a state wins and stops the tick.
    pub fn tick(&mut self, facts: &mut F) -> Option<Fired<S>> {
        for blocking in Blocking::ORDER {
            let count = self.pass(blocking).len();
            for position in 0..count {
                let index = self.pass(blocking)[position];
                if let Some(fired) = self.step(index, facts) {
                    return Some(fired);
                }
            }
        }
        let state = self.state;
        let count = self.list(state).map(|list| list.len()).unwrap_or(0);
        for position in 0..count {
            let index = self.list(state).expect("just measured")[position];
            if let Some(fired) = self.step(index, facts) {
                return Some(fired);
            }
        }
        None
    }

    /// `TickRateStateMachine.checkTransition` (`:114-125`) on top of
    /// `BasicStateMachine.checkTransition` (`:283-322`).
    fn step(&mut self, index: usize, facts: &mut F) -> Option<Fired<S>> {
        let rate = self.tick_rate;
        {
            let transition = &mut self.transitions[index];
            if !transition.count_down(rate) {
                return None;
            }
        }
        self.executed = Some(index);
        let transition = self.transitions[index];

        if !transition.condition.holds(facts, self.state) {
            return None;
        }
        let from = self.state;
        let to = transition.action.next(facts, from)?;
        let fired = Fired {
            rule: transition.rule,
            java: transition.java,
            from,
            to,
            reset: false,
        };
        if to == from {
            // `transitionToNext` still answers `true` for a self-transition: the
            // tick stops, the state does not move.
            return Some(fired);
        }
        if !self.can_leave(to) {
            // `BasicStateMachine.java:344-356`: a state no transition names is a trap.
            self.missing = Some(MissingTransition {
                state: to,
                rule: transition.rule,
            });
            self.state = self.init;
            return Some(Fired { reset: true, ..fired });
        }
        self.state = to;
        Some(fired)
    }
}

impl<S: Copy + Eq + fmt::Debug, F> fmt::Debug for Machine<S, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Machine")
            .field("state", &self.state)
            .field("init", &self.init)
            .field("tick_rate", &self.tick_rate)
            .field("transitions", &self.transitions.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Facts for the machine tests: a switch per condition, and a log of what ran.
    #[derive(Default)]
    struct Facts {
        first: bool,
        second: bool,
        ran: Vec<&'static str>,
    }

    fn always(_: &mut Facts, from: WorkerState) -> Option<WorkerState> {
        Some(from)
    }

    fn go_start(_: &mut Facts, _: WorkerState) -> Option<WorkerState> {
        Some(WorkerState::StartWorking)
    }

    fn go_idle(facts: &mut Facts, _: WorkerState) -> Option<WorkerState> {
        facts.ran.push("idle");
        Some(WorkerState::Idle)
    }

    fn go_needs_item(facts: &mut Facts, _: WorkerState) -> Option<WorkerState> {
        facts.ran.push("needs");
        Some(WorkerState::NeedsItem)
    }

    fn first(facts: &Facts, _: WorkerState) -> bool {
        facts.first
    }

    fn second(facts: &Facts, _: WorkerState) -> bool {
        facts.second
    }

    #[test]
    fn eighteen_states_and_their_eat_flag_follow_java() {
        assert_eq!(WorkerState::ALL.len(), 18);
        // The flags Java passes to the enum constructor, spot-checked both ways.
        assert!(WorkerState::Idle.okay_to_eat()); // AIWorkerState.java:16
        assert!(WorkerState::StartWorking.okay_to_eat()); // :32
        assert!(!WorkerState::InventoryFull.okay_to_eat()); // :24
        assert!(!WorkerState::BuildingStep.okay_to_eat()); // :52
        assert!(!WorkerState::PickUp.okay_to_eat()); // :130
        assert!(WorkerState::GetMaterials.okay_to_eat()); // :302
        // One name per state, and Java's spelling is upper snake case.
        let mut names: Vec<&str> = WorkerState::ALL.iter().map(|s| s.java_name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 18, "two states share a name");
        assert_eq!(WorkerState::GatheringRequiredMaterials.java_name(),
            "GATHERING_REQUIRED_MATERIALS");
    }

    #[test]
    fn the_three_passes_run_ai_blocking_then_event_then_state_blocking() {
        assert_eq!(Blocking::ORDER.len(), 3);
        for (rank, blocking) in Blocking::ORDER.iter().enumerate() {
            assert_eq!(blocking.rank(), rank);
        }
        assert_eq!(Blocking::AiBlocking.java_name(), "AI_BLOCKING");

        let mut machine: Machine<WorkerState, Facts> =
            Machine::new(WorkerState::Idle, ENTITY_AI_TICKRATE);
        // Registered in the reverse order on purpose: the pass decides, not the
        // registration.
        machine.add(Transition::event(
            Blocking::StateBlocking,
            "state_blocking",
            "test",
            1,
            Action::Run(go_needs_item),
        ));
        machine.add(Transition::event(
            Blocking::Event,
            "event",
            "test",
            1,
            Action::Run(go_idle),
        ));
        machine.add(Transition::event(
            Blocking::AiBlocking,
            "ai_blocking",
            "test",
            1,
            Action::Run(go_start),
        ));
        // An exit for the state we land in, so the machine is not caught in a trap
        // (`BasicStateMachine.java:344-356`) when the assertion reads its state.
        machine.add(Transition::goto(
            WorkerState::StartWorking,
            WorkerState::Idle,
            "back",
            "test",
            1,
        ));
        let mut facts = Facts::default();
        let fired = machine.tick(&mut facts).expect("one pass wins");
        assert_eq!(fired.rule, "ai_blocking", "TickRateStateMachine.java:99-108");
        assert_eq!(machine.state(), WorkerState::StartWorking);
        assert_eq!(facts.ran, Vec::<&str>::new(), "the later passes never ran");
    }

    #[test]
    fn an_action_that_returns_no_state_does_not_stop_the_tick() {
        // `BasicStateMachine.checkTransition` answers `transitionToNext`, which is
        // false for a null supplier result (`:340-368`).
        let mut machine: Machine<WorkerState, Facts> =
            Machine::new(WorkerState::Idle, ENTITY_AI_TICKRATE);
        machine.add(Transition::target(
            WorkerState::Idle,
            "does-nothing",
            "test",
            1,
            Action::Run(|_: &mut Facts, _: WorkerState| None),
        ));
        machine.add(Transition::goto(
            WorkerState::Idle,
            WorkerState::StartWorking,
            "then-goes",
            "test",
            1,
        ));
        machine.add(Transition::goto(
            WorkerState::StartWorking,
            WorkerState::Idle,
            "back",
            "test",
            1,
        ));
        let mut facts = Facts::default();
        let fired = machine.tick(&mut facts).expect("the second transition runs");
        assert_eq!(fired.rule, "then-goes");
        assert_eq!(machine.state(), WorkerState::StartWorking);
    }

    #[test]
    fn a_countdown_only_lets_a_transition_run_every_n_ticks() {
        // `TickingTransition.java:150-158`: decrement by the machine's rate, run at
        // zero, restart at `tickRate`.
        let mut machine: Machine<WorkerState, Facts> =
            Machine::new(WorkerState::Idle, ENTITY_AI_TICKRATE);
        // A self-transition: it runs, changes nothing, and so never trips the trap
        // rule. `AbstractEntityAIBasic.java:205` is exactly this shape.
        machine.add(Transition::goto(
            WorkerState::Idle,
            WorkerState::Idle,
            "every-20",
            "test",
            20,
        ));
        let mut facts = Facts::default();
        // First look: `ticks_to_update` is 0, so it runs and restarts at 20.
        assert!(machine.tick(&mut facts).is_some(), "first look always runs");
        assert_eq!(machine.transitions()[0].ticks_to_update, 20);
        // Then 20 - 5 = 15, then 10, then 5: three machine ticks of no work.
        for _ in 0..3 {
            assert!(machine.tick(&mut facts).is_none(), "not due yet");
        }
        assert_eq!(machine.transitions()[0].ticks_to_update, 5);
        // The fourth reaches 0, which is due (`TickingTransition.java:150-158`).
        assert!(machine.tick(&mut facts).is_some(), "due again");
        assert_eq!(machine.transitions()[0].ticks_to_update, 20);
    }

    #[test]
    fn a_tick_rate_of_zero_is_clamped_up_and_a_huge_one_down() {
        // `TickingTransition.java:52-57`: `min(tickRate, MAX_AI_TICKRATE)` then
        // `max(that, 1)`.
        let zero = Transition::<WorkerState, Facts>::goto(
            WorkerState::Idle,
            WorkerState::Init,
            "r",
            "test",
            0,
        );
        assert_eq!(zero.tick_rate, 1, "a rate of 0 becomes 1");
        let huge = Transition::<WorkerState, Facts>::goto(
            WorkerState::Idle,
            WorkerState::Init,
            "r",
            "test",
            u32::MAX,
        );
        assert_eq!(huge.tick_rate, MAX_AI_TICKRATE, ":14, ten minutes of ticks");
        assert_eq!(MAX_AI_TICKRATE, 20 * 60 * 10);
        assert_eq!(MAX_TICKRATE_VARIANT, 50);
        // And the machine's own rate: `setTickRate` takes what it is given, but the
        // port refuses a 0 so that a countdown can always make progress.
        let mut machine: Machine<WorkerState, Facts> =
            Machine::new(WorkerState::Idle, ENTITY_AI_TICKRATE);
        machine.set_tick_rate(0);
        assert_eq!(machine.tick_rate(), 1, "a machine tick rate of 0 is refused");
        assert_eq!(ENTITY_AI_TICKRATE, 5);
    }

    #[test]
    fn set_current_delay_pushes_the_transition_that_just_ran() {
        // `TickRateStateMachine.setCurrentDelay` (`:132-136`) on the executed
        // transition. `AbstractEntityAIBasic.whenException` uses it (`:361`).
        let mut machine: Machine<WorkerState, Facts> =
            Machine::new(WorkerState::Idle, ENTITY_AI_TICKRATE);
        machine.add(Transition::target(
            WorkerState::Idle,
            "holds",
            "test",
            1,
            Action::Run(always),
        ));
        assert_eq!(machine.set_current_delay(40), None, "nothing has run yet");
        let mut facts = Facts::default();
        machine.tick(&mut facts).expect("the self transition ran");
        assert_eq!(machine.set_current_delay(40), Some("holds"));
        for _ in 0..7 {
            assert!(machine.tick(&mut facts).is_none(), "still delayed");
        }
    }

    #[test]
    fn a_state_no_transition_names_is_a_trap_and_resets_to_init() {
        // `BasicStateMachine.java:344-356`: warn, `reset()`, return true.
        let mut machine: Machine<WorkerState, Facts> =
            Machine::new(WorkerState::Init, ENTITY_AI_TICKRATE);
        machine.add(Transition::goto(
            WorkerState::Init,
            WorkerState::BuildingStep,
            "into-the-void",
            "test",
            1,
        ));
        let mut facts = Facts::default();
        let fired = machine.tick(&mut facts).expect("the transition runs");
        assert!(fired.reset);
        assert_eq!(machine.state(), WorkerState::Init, "reset to the init state");
        assert_eq!(
            machine.missing(),
            Some(MissingTransition {
                state: WorkerState::BuildingStep,
                rule: "into-the-void"
            })
        );
    }

    #[test]
    fn a_group_transition_is_checked_before_the_states_own_rules() {
        // `BasicStateMachine.addTransitionGroup` inserts at index 0 (`:76-95`).
        let mut machine: Machine<WorkerState, Facts> =
            Machine::new(WorkerState::Idle, ENTITY_AI_TICKRATE);
        machine.add(Transition::target(
            WorkerState::Idle,
            "own-rule",
            "test",
            1,
            Action::Run(go_start),
        ));
        machine.add_group(
            &[WorkerState::Idle, WorkerState::StartWorking],
            Transition::target_when(
                WorkerState::Idle,
                "group-rule",
                "test",
                1,
                second,
                Action::Run(go_idle),
            )
            .without_state(),
        );
        let mut facts = Facts {
            second: true,
            ..Facts::default()
        };
        let fired = machine.tick(&mut facts).expect("the group rule runs");
        assert_eq!(fired.rule, "group-rule");
        assert_eq!(machine.state(), WorkerState::Idle);

        // With the group's condition false, the state's own rule wins again.
        let mut machine: Machine<WorkerState, Facts> =
            Machine::new(WorkerState::Idle, ENTITY_AI_TICKRATE);
        machine.add(Transition::target(
            WorkerState::Idle,
            "own-rule",
            "test",
            1,
            Action::Run(go_start),
        ));
        machine.add_group(
            &[WorkerState::Idle],
            Transition::target_when(
                WorkerState::Idle,
                "group-rule",
                "test",
                1,
                first,
                Action::Run(go_idle),
            )
            .without_state(),
        );
        let fired = machine.tick(&mut facts).expect("the own rule runs");
        assert_eq!(fired.rule, "own-rule");
    }

    #[test]
    fn a_condition_that_says_no_leaves_the_countdown_restarted_but_the_state_alone() {
        // Java restarts `ticksToUpdate` *before* the condition (`:114-125`), so a
        // transition whose condition is false still waits its rate again.
        let mut machine: Machine<WorkerState, Facts> =
            Machine::new(WorkerState::Idle, ENTITY_AI_TICKRATE);
        machine.add(Transition::target_when(
            WorkerState::Idle,
            "never",
            "test",
            10,
            first,
            Action::Run(go_start),
        ));
        let mut facts = Facts::default();
        assert!(machine.tick(&mut facts).is_none());
        assert_eq!(machine.transitions()[0].ticks_to_update, 10);
        assert_eq!(machine.state(), WorkerState::Idle);
    }

    #[test]
    fn reset_goes_back_to_init_without_forgetting_the_countdowns() {
        // `BasicStateMachine.reset` (`:373-380`) sets the state and the list only.
        let mut machine: Machine<WorkerState, Facts> =
            Machine::new(WorkerState::Init, ENTITY_AI_TICKRATE);
        machine.add(Transition::goto(
            WorkerState::Init,
            WorkerState::Idle,
            "init",
            "test",
            1,
        ));
        machine.add(Transition::goto(
            WorkerState::Idle,
            WorkerState::Init,
            "idle",
            "test",
            1,
        ));
        let mut facts = Facts::default();
        machine.tick(&mut facts);
        assert_eq!(machine.state(), WorkerState::Idle);
        machine.reset();
        assert_eq!(machine.state(), WorkerState::Init);
        assert_eq!(machine.init_state(), WorkerState::Init);
    }
}
