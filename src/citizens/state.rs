//! The states a citizen can be in, the icon it shows, and the legal transitions.
//!
//! Java reference: `api/entity/ai/statemachine/states/CitizenAIState.java:6-17`
//! (nine states), `api/entity/citizen/VisibleCitizenStatus.java:25-41` (eight
//! statuses), `core/entity/ai/workers/CitizenAI.java:65-127` (how a decision is
//! applied) and `core/entity/citizen/EntityCitizen.java:1460-1466` (the one
//! transition nothing decides).
//!
//! **Java has three layers of "state" and the brief's nine names sit in all
//! three. The port keeps them apart instead of flattening them.**
//!
//! | Layer | Java | Count | Here |
//! | --- | --- | ---: | --- |
//! | what the citizen is *doing* | `CitizenAIState` | **9** | [`CitizenState`] |
//! | what the player *sees* | `VisibleCitizenStatus` (central class) | **8** | [`Status`] |
//! | steps *inside* a job's AI | `AIWorkerState` | **126** | not ported — `crate::jobs` |
//!
//! So `BadWeather` is **not a state**: it is [`Status::BadWeather`] and the
//! citizen is `IDLE` (`CitizenAI.java:232-241`). `InventoryFull` is not a citizen
//! state either — it is `AIWorkerState.INVENTORY_FULL`
//! (`AIWorkerState.java:23`), a worker step — and `Following` is
//! `EntityAICitizenChild.State.FOLLOWING`
//! (`core/entity/ai/minimal/EntityAICitizenChild.java:42-47`), a sub-state of the
//! idle path. Twenty-one further statuses are declared by job AIs (29
//! `new VisibleCitizenStatus(...)` calls in the tree, 8 here); they belong to
//! `crate::jobs`. The 34 sub-states of the six minimal AIs (eating 9, sick 8,
//! mourning 7, wandering 4, child 4, fleeing 2) are not ported: they fire inside
//! a [`CitizenState`] and need the game.

use crate::jobs::Job;

use super::employment::is_guard;

/// The nine states of Java's `CitizenAIState`, in its declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: the enum is closed in Java and a tenth state is a change to the
// state machine, not a value
pub enum CitizenState {
    /// Doing nothing in particular; also what a citizen is when it rains and it
    /// may not work (`CitizenAI.java:240`, `:247`, `:260`).
    Idle,
    /// Running from a monster. Nothing decides it: damage pushes it
    /// (`EntityCitizen.java:1462`).
    Flee,
    /// Walking to food, waiting for it, or eating it.
    Eating,
    /// Ill or hurt, on the way to a cure (`CitizenAI.java:207`).
    Sick,
    /// Going home, finding a bed, or asleep (`:175`, `:181`).
    Sleep,
    /// Bereaved, walking to the town hall or the graveyard (`:228`).
    Mourn,
    /// The job was chosen and its AI has not started yet (`:256` → `:81-90`).
    Work,
    /// The job's own AI is running, tick by tick (`:92-104`).
    Working,
    /// Declared in Java, produced by nothing in this tree
    /// (`CitizenAIState.java:16`): a citizen whose entity has no data
    /// (`EntityCitizen.java:297-300`) is `EntityState.INACTIVE`, another enum.
    /// Kept, with the gap named, because dropping it would make the count a lie.
    Inactive,
}

impl CitizenState {
    /// Every state, in Java's declaration order.
    pub const ALL: [CitizenState; 9] = [
        CitizenState::Idle,
        CitizenState::Flee,
        CitizenState::Eating,
        CitizenState::Sick,
        CitizenState::Sleep,
        CitizenState::Mourn,
        CitizenState::Work,
        CitizenState::Working,
        CitizenState::Inactive,
    ];

    /// Java's identifier, for a log line or a test that has to name it.
    pub fn java_name(self) -> &'static str {
        match self {
            CitizenState::Idle => "IDLE",
            CitizenState::Flee => "FLEE",
            CitizenState::Eating => "EATING",
            CitizenState::Sick => "SICK",
            CitizenState::Sleep => "SLEEP",
            CitizenState::Mourn => "MOURN",
            CitizenState::Work => "WORK",
            CitizenState::Working => "WORKING",
            CitizenState::Inactive => "INACTIVE",
        }
    }

    /// Whether a job's AI ticks in this state: only `WORKING`
    /// (`CitizenAI.java:92-104`); `WORK` resets it (`:81-90`).
    pub fn job_runs(self) -> bool {
        self == CitizenState::Working
    }
}

impl std::fmt::Display for CitizenState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.java_name())
    }
}

/// The eight icons of `VisibleCitizenStatus`, in declaration order.
///
/// Java declares them as static fields of a class rather than as an enum
/// (`VisibleCitizenStatus.java:25-41`), each with an icon, a translation key and
/// a "draw it over the head" flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: the central class declares exactly these eight
pub enum Status {
    /// `EAT` — hungry (`:27`).
    Eat,
    /// `HOUSE` — idle at home (`:29`).
    House,
    /// `RAIDED` — the colony is under attack (`:31`).
    Raided,
    /// `MOURNING` — bereaved (`:33`).
    Mourning,
    /// `BAD_WEATHER` — it rains and this worker may not work in it (`:35`).
    BadWeather,
    /// `SLEEP` — asleep (`:37`).
    Sleep,
    /// `SICK` — ill or hurt (`:39`).
    Sick,
    /// `WORKING` — on the job (`:41`).
    Working,
}

impl Status {
    /// Every status of the central class, in its declaration order.
    pub const ALL: [Status; 8] = [
        Status::Eat,
        Status::House,
        Status::Raided,
        Status::Mourning,
        Status::BadWeather,
        Status::Sleep,
        Status::Sick,
        Status::Working,
    ];

    /// The icon path, as Java writes it minus the `minecolonies:` namespace
    /// (`:27-41`). It is a texture path in the tree, not a registry name, and the
    /// port keeps it as text: nothing here can load a resource, and building a
    /// resource location by hand is the class of mistake this crate refuses.
    pub fn icon(self) -> &'static str {
        match self {
            Status::Eat => "textures/icons/hungry.png",
            Status::House => "textures/icons/house_big.png",
            Status::Raided => "textures/icons/raid_icon.png",
            Status::Mourning => "textures/icons/mourning.png",
            Status::BadWeather => "textures/icons/bad_weather.png",
            Status::Sleep => "textures/icons/bed_icon.png",
            Status::Sick => "textures/icons/small_sick_icon.png",
            Status::Working => "textures/icons/working.png",
        }
    }

    /// The translation key, resolved from `TranslationConstants.java:798-812`.
    pub fn lang_key(self) -> &'static str {
        match self {
            Status::Eat => "com.minecolonies.gui.visiblestatus.eat",
            Status::House => "com.minecolonies.gui.visiblestatus.idle",
            Status::Raided => "com.minecolonies.gui.visiblestatus.raid",
            Status::Mourning => "com.minecolonies.gui.visiblestatus.mourn",
            Status::BadWeather => "com.minecolonies.gui.visiblestatus.rain",
            Status::Sleep => "com.minecolonies.gui.visiblestatus.sleep",
            Status::Sick => "com.minecolonies.gui.visiblestatus.sick",
            Status::Working => "com.minecolonies.gui.visiblestatus.working",
        }
    }

    /// Java's `render` flag, set by the **three-argument** constructor: seven
    /// statuses pass `true`, `WORKING` uses the two-argument one and stays
    /// `false` (`VisibleCitizenStatus.java:41`, `:52-66`).
    pub fn renders_over_head(self) -> bool {
        self != Status::Working
    }
}

/// Which branch of `calculateNextState` a citizen takes.
///
/// One `instanceof AbstractJobGuard` decides it (`CitizenAI.java:136`), and the
/// branch changes the whole reachable set: a guard eats, is sick and works, and
/// **never** sleeps, mourns or idles — its branch returns `WORK` at `:150`
/// whatever the hour, the raid or the mourning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: two branches, and the `instanceof` has two answers
pub enum Branch {
    /// Every job but the six guard ones — and every jobless citizen.
    Ordinary,
    /// A knight, a ranger, a druid, a cavalryman, a huscarl or a marksman.
    Guard,
}

impl Branch {
    /// The branch this job takes (`CitizenAI.java:136`).
    pub fn of(job: Option<Job>) -> Branch {
        match job {
            Some(job) if is_guard(job) => Branch::Guard,
            _ => Branch::Ordinary,
        }
    }

    /// Every state this branch's decision can return — the whole of what
    /// `calculateNextState` produces for it. **The order is not a priority**:
    /// [`super::priority`] carries that, check by check. This is a set as a slice.
    pub fn decision_targets(self) -> &'static [CitizenState] {
        // Ordinary: SICK at :157 and :207, SLEEP at :165, :175, :181, EATING at
        // :213, MOURN at :228, IDLE at :240, :247, :260, WORK at :256.
        const ORDINARY: &[CitizenState] = &[
            CitizenState::Sick,
            CitizenState::Sleep,
            CitizenState::Eating,
            CitizenState::Mourn,
            CitizenState::Work,
            CitizenState::Idle,
        ];
        // Guard: EATING at :140, SICK at :147, WORK at :150. No SLEEP, no MOURN,
        // no IDLE — that branch never returns them.
        const GUARD: &[CitizenState] = &[
            CitizenState::Eating,
            CitizenState::Sick,
            CitizenState::Work,
        ];
        match self {
            Branch::Ordinary => ORDINARY,
            Branch::Guard => GUARD,
        }
    }
}

/// Why a state change was refused.
///
/// Java has no such error: its `TickRateStateMachine` simply has no transition to
/// offer, and a state no `AITarget` names is unreachable. The port names the
/// refusal because a caller here can ask for anything, and "the machine ignored
/// you" is not a diagnosis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct IllegalTransition {
    pub from: CitizenState,
    pub to: CitizenState,
}

impl IllegalTransition {
    /// A stable name, namespaced.
    pub const NAME: &'static str = "citizens.refusal.illegal_transition";

    pub fn name(&self) -> &'static str {
        Self::NAME
    }
}

impl std::fmt::Display for IllegalTransition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {} -> {}", Self::NAME, self.from, self.to)
    }
}

impl std::error::Error for IllegalTransition {}

impl CitizenState {
    /// Whether `self` may become `to`, and why not when it may not.
    ///
    /// Four ways in, each a line of Java:
    ///
    /// 1. **staying** — a machine that re-enters its own state changes nothing,
    ///    and `decideAiTask` answers `null` when the decision repeats
    ///    (`CitizenAI.java:121`), leaving `WORKING` to loop on itself every 5
    ///    ticks (`:92-104`);
    /// 2. **`FLEE`** — pushed from *any* state by damage, through a one-time
    ///    event target (`EntityCitizen.java:1462`), never decided;
    /// 3. **`WORK` → `WORKING`** — an `AITarget` of its own, ticked every 10,
    ///    which resets the job AI and returns `WORKING` (`CitizenAI.java:81-90`).
    ///    That is why the decision's `WORK` rung answers `WORK` and not `WORKING`
    ///    while showing the `WORKING` icon (`:255`): the icon is set there, the
    ///    state by this target;
    /// 4. **the branch's decision targets** — the periodic event fires whatever
    ///    the current state is (`CitizenAI.java:65`; `AIBlockingEventType.EVENT`
    ///    says of itself "Checked regardless of current state").
    ///
    /// Two families fall out of it: **`WORKING` is only ever entered from
    /// `WORK`** (`:81-90` is its only writer), and **`INACTIVE` is entered by
    /// nothing at all**.
    pub fn may_become(self, to: CitizenState, branch: Branch) -> Result<(), IllegalTransition> {
        // :81-90 — `WORK` carries its own target, outside `calculateNextState`.
        if self == CitizenState::Work && to == CitizenState::Working {
            return Ok(());
        }
        if self == to || to == CitizenState::Flee || branch.decision_targets().contains(&to) {
            return Ok(());
        }
        Err(IllegalTransition { from: self, to })
    }
}

/// What one pass of the decision returns: the state to switch to, and the icon
/// Java sets on the way.
///
/// Java returns a bare `IState` and writes the icon through
/// `setVisibleStatus`/`setVisibleStatusIfNone` as a side effect (`:146`, `:156`,
/// `:164`, `:173`, `:206`, `:226`, `:234`, `:246`, `:255`, `:259`). The port
/// returns the pair: "`IDLE`, and it rains" is testable, a side effect on an
/// entity that does not exist yet is not. The exact ordering of
/// `setVisibleStatus(null)` (`:124`) is not modelled — the decision says which
/// icon it asked for, the caller decides when to replace the old one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Decision {
    pub state: CitizenState,
    /// `None` is "this change shows no icon of its own": `EATING` arrives bare,
    /// its status is written by the eat AI, not by `calculateNextState`.
    pub status: Option<Status>,
}

impl Decision {
    pub fn bare(state: CitizenState) -> Decision {
        Decision {
            state,
            status: None,
        }
    }

    pub fn with(state: CitizenState, status: Status) -> Decision {
        Decision {
            state,
            status: Some(status),
        }
    }
}

/// The citizen's state machine, as `CitizenAI` runs it.
///
/// Two fields, both Java's: the current state, and `lastState`
/// (`CitizenAI.java:57`, initialised to `IDLE`), which the decision reads to know
/// what it decided last time — `shouldEat` answers `true` outright while
/// `lastState == EATING` (`:280`), and the sleep branch keeps a sleeper asleep
/// (`:171`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Machine {
    state: CitizenState,
    last: CitizenState,
}

impl Default for Machine {
    fn default() -> Machine {
        Machine::new()
    }
}

impl Machine {
    /// A fresh citizen: `IDLE`, and `lastState` `IDLE` too (`CitizenAI.java:57`).
    pub fn new() -> Machine {
        Machine {
            state: CitizenState::Idle,
            last: CitizenState::Idle,
        }
    }

    pub fn state(&self) -> CitizenState {
        self.state
    }

    pub fn last(&self) -> CitizenState {
        self.last
    }

    /// One pass of `decideAiTask` (`CitizenAI.java:112-127`).
    ///
    /// Java's rule in full: if the decision repeats `lastState` it is only
    /// re-issued when the citizen is `IDLE` and the decision is not `IDLE`
    /// itself; otherwise the answer is `null`, i.e. no change. On a real change
    /// `lastState` is recorded — before the caller applies the state, so the two
    /// fields genuinely differ during a pass.
    ///
    /// The legality check is the port's addition: `pick` is any closure, and one
    /// naming `WORKING` from `IDLE` would describe a transition Java cannot make.
    pub fn decide<F>(&mut self, branch: Branch, pick: F) -> Result<Option<Decision>, IllegalTransition>
    where
        F: FnOnce(&Machine) -> Decision,
    {
        let decision = pick(&*self);
        let next = decision.state;
        if next == self.last {
            if self.state == CitizenState::Idle && next != CitizenState::Idle {
                self.state.may_become(next, branch)?;
                self.state = next;
                return Ok(Some(decision));
            }
            return Ok(None);
        }
        self.state.may_become(next, branch)?;
        self.last = next;
        self.state = next;
        Ok(Some(decision))
    }

    /// The one transition nothing decides: damage pushes `FLEE`
    /// (`EntityCitizen.java:1460-1466`), from whatever state the citizen is in.
    /// `lastState` is untouched, exactly as in Java — the push goes through a
    /// one-time event target, not through `decideAiTask`.
    pub fn flee(&mut self) -> Result<CitizenState, IllegalTransition> {
        self.state.may_become(CitizenState::Flee, Branch::Ordinary)?;
        self.state = CitizenState::Flee;
        Ok(self.state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nine_states_and_eight_statuses_and_the_briefs_names_land_where_java_puts_them() {
        assert_eq!(CitizenState::ALL.len(), 9, "CitizenAIState.java:6-17");
        assert_eq!(Status::ALL.len(), 8, "VisibleCitizenStatus.java:25-41");
        let named: Vec<&str> = CitizenState::ALL.iter().map(|s| s.java_name()).collect();
        assert_eq!(
            named,
            [
                "IDLE", "FLEE", "EATING", "SICK", "SLEEP", "MOURN", "WORK", "WORKING", "INACTIVE"
            ]
        );
        assert!(
            !named.contains(&"BAD_WEATHER"),
            "BadWeather is a status; CitizenAI.java:232-241 returns IDLE"
        );
        // Seven icons render over the head; WORKING uses the 2-arg constructor.
        let rendered = Status::ALL
            .iter()
            .filter(|status| status.renders_over_head())
            .count();
        assert_eq!(rendered, 7);
        assert!(!Status::Working.renders_over_head());
        for status in Status::ALL {
            assert!(status.icon().starts_with("textures/icons/"));
            assert!(status.lang_key().starts_with("com.minecolonies.gui.visiblestatus."));
        }
    }

    #[test]
    fn a_guard_never_sleeps_mourns_or_idles_but_an_ordinary_citizen_does() {
        assert_eq!(Branch::Guard.decision_targets().len(), 3);
        for refused in [CitizenState::Sleep, CitizenState::Mourn, CitizenState::Idle] {
            assert_eq!(
                CitizenState::Work.may_become(refused, Branch::Guard),
                Err(IllegalTransition {
                    from: CitizenState::Work,
                    to: refused
                }),
                "the guard branch (CitizenAI.java:136-151) never returns {refused}"
            );
            assert_eq!(
                CitizenState::Work.may_become(refused, Branch::Ordinary),
                Ok(())
            );
        }
        assert_eq!(Branch::of(Some(Job::Marksman)), Branch::Guard);
        assert_eq!(Branch::of(Some(Job::Farmer)), Branch::Ordinary);
        assert_eq!(Branch::of(None), Branch::Ordinary);
    }

    #[test]
    fn working_is_only_entered_from_work_and_inactive_by_nothing() {
        for from in CitizenState::ALL {
            let allowed = from == CitizenState::Work || from == CitizenState::Working;
            assert_eq!(
                from.may_become(CitizenState::Working, Branch::Ordinary).is_ok(),
                allowed,
                "CitizenAI.java:81-90 is the only writer of WORKING"
            );
            if from != CitizenState::Inactive {
                assert!(
                    from.may_become(CitizenState::Inactive, Branch::Ordinary)
                        .is_err(),
                    "{from} reaches INACTIVE, which nothing in the tree produces"
                );
            }
        }
    }

    #[test]
    fn staying_and_fleeing_are_legal_from_every_state_and_branch() {
        for from in CitizenState::ALL {
            for branch in [Branch::Ordinary, Branch::Guard] {
                assert_eq!(from.may_become(from, branch), Ok(()));
                assert_eq!(
                    from.may_become(CitizenState::Flee, branch),
                    Ok(()),
                    "the damage push at EntityCitizen.java:1462 asks nothing"
                );
            }
        }
        assert_eq!(
            IllegalTransition {
                from: CitizenState::Eating,
                to: CitizenState::Working
            }
            .to_string(),
            "citizens.refusal.illegal_transition: EATING -> WORKING"
        );
    }

    #[test]
    fn decide_records_the_last_state_and_replays_it_only_from_idle() {
        let mut machine = Machine::new();
        assert_eq!((machine.state(), machine.last()), (CitizenState::Idle, CitizenState::Idle));

        let decided = machine
            .decide(Branch::Ordinary, |_| Decision::bare(CitizenState::Work))
            .expect("WORK is reachable from IDLE");
        assert_eq!(decided.map(|d| d.state), Some(CitizenState::Work));
        assert_eq!((machine.state(), machine.last()), (CitizenState::Work, CitizenState::Work));

        // `next == lastState`, state is not IDLE: Java answers null (:115-122).
        assert_eq!(
            machine.decide(Branch::Ordinary, |_| Decision::bare(CitizenState::Work)),
            Ok(None)
        );
        assert_eq!(machine.state(), CitizenState::Work);

        // Back to IDLE, then SLEEP again: a real change, because SLEEP != IDLE.
        machine
            .decide(Branch::Ordinary, |_| Decision::bare(CitizenState::Idle))
            .unwrap();
        machine
            .decide(Branch::Ordinary, |_| Decision::bare(CitizenState::Sleep))
            .unwrap();
        assert_eq!(machine.state(), CitizenState::Sleep);
    }

    #[test]
    fn decide_refuses_what_java_cannot_do_and_a_refusal_changes_nothing() {
        let mut machine = Machine::new();
        assert_eq!(
            machine.decide(Branch::Ordinary, |_| Decision::bare(CitizenState::Working)),
            Err(IllegalTransition {
                from: CitizenState::Idle,
                to: CitizenState::Working
            })
        );
        assert_eq!(machine.state(), CitizenState::Idle);

        let mut guard = Machine::new();
        assert_eq!(
            guard.decide(Branch::Guard, |_| Decision::bare(CitizenState::Sleep)),
            Err(IllegalTransition {
                from: CitizenState::Idle,
                to: CitizenState::Sleep
            })
        );
    }

    #[test]
    fn the_pick_sees_the_machine_and_fleeing_does_not_move_the_last_state() {
        let mut machine = Machine::new();
        machine
            .decide(Branch::Ordinary, |_| Decision::bare(CitizenState::Work))
            .unwrap();
        let seen = machine
            .decide(Branch::Ordinary, |current| {
                if current.last() == CitizenState::Work {
                    Decision::with(CitizenState::Idle, Status::House)
                } else {
                    Decision::bare(CitizenState::Mourn)
                }
            })
            .unwrap();
        assert_eq!(seen, Some(Decision::with(CitizenState::Idle, Status::House)));

        assert_eq!(machine.flee(), Ok(CitizenState::Flee));
        assert_eq!(machine.state(), CitizenState::Flee);
        assert_eq!(
            machine.last(),
            CitizenState::Idle,
            "the push goes through a one-time target, not decideAiTask"
        );
    }

    #[test]
    fn only_working_ticks_a_job_ai() {
        let running: Vec<CitizenState> = CitizenState::ALL
            .iter()
            .copied()
            .filter(|state| state.job_runs())
            .collect();
        assert_eq!(running, vec![CitizenState::Working]);
    }
}
