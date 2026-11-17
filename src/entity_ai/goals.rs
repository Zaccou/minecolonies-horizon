//! Every goal of `entity/ai/` that is not a trade, with its priority and its
//! trigger.
//!
//! **One table per state machine.** Java's priority is *registration order*: a
//! state's transitions are checked in the order they were added, and
//! `addTransitionGroup` puts a group's rule **in front** of every list it covers
//! (`BasicStateMachine.java:100-118`). The tables below are that order written
//! down, with the goal each rule leads to, what makes it fire, how often it is
//! looked at, and the line it comes from. They are not documentation *about* the
//! machines: each module builds its machine from its table, and `tests.rs` asserts
//! the two agree rule for rule.
//!
//! **What the brief called "targeting" and "raid goals".** There is no
//! `entity/ai/targeting/` package and no `EntityAIChaseCitizen`. The targeting
//! table is `combat/TargetAI.java` + `combat/AttackMoveAI.java` ([`COMBAT_TABLE`]);
//! the raid behaviour is the guard's, because a raid reaches a guard through the
//! threat table and `AbstractEntityAIGuard` (`RAPPORT.md` §1).
//!
//! **`isOkayToEat` is not everywhere.** Only the state types that implement
//! `IAIState` answer it (`api/entity/ai/statemachine/states/IAIState.java:13`).
//! `CitizenAIState`, `EatingState`, `SleepState`, `DiseaseState`, `MourningState`,
//! `WanderState`, `FleeStates` and `VisitorState` implement plain `IState`
//! (`IState.java:6-10`, which has no method at all), so they cannot be asked. The
//! port reproduces that: [`CitizenGoal::okay_to_eat`] and
//! [`GuardGoal::okay_to_eat`] exist, [`EatState::okay_to_eat`] does not.

use crate::ai::state::WorkerState;

/// One rule of a goal table: where it leads, when, how often, and where from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule<S> {
    /// The rank in the state's list, `1` checked first.
    pub priority: u16,
    /// The goal the rule leads to.
    pub goal: S,
    /// This port's name for the rule.
    pub rule: &'static str,
    /// What makes it fire, in one line.
    pub when: &'static str,
    /// The tick rate the transition is looked at.
    pub tick_rate: u32,
    /// `fichier.java:ligne`.
    pub java: &'static str,
}

/// What one tick of a one-rule state produced.
///
/// Every state of the citizen goals (`EntityAIEatTask`, `EntityAISleep`,
/// `EntityAISickTask`, `EntityAIMournCitizen`, `EntityAICitizenWander`,
/// `EntityAICitizenAvoidEntity`) carries **exactly one** transition, built with the
/// constant condition `() -> true`. The machine adds nothing to such a state, so the
/// port exposes the action: it answers the next state, or [`Step::Stayed`] for
/// Java's `return null`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: a state, or Java's `null`
pub enum Step<S> {
    /// The state the rule leads to.
    Next(S),
    /// Java's `return null`: the rule ran and changed nothing.
    Stayed,
}

impl<S: Copy> Step<S> {
    /// The state, if there is one.
    pub fn next(self) -> Option<S> {
        match self {
            Step::Next(state) => Some(state),
            Step::Stayed => None,
        }
    }
}

/// A rule of a vanilla `Goal` — no tick rate and no state, just when and where
/// from (`LookAtEntityGoal`, `EntityAIInteractToggleAble`, `EntityAIFloat`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GoalRule {
    /// This port's name for the rule.
    pub rule: &'static str,
    /// What makes it fire.
    pub when: &'static str,
    /// `fichier.java:ligne`.
    pub java: &'static str,
}

impl<S: Copy + core::fmt::Debug> Rule<S> {
    /// The `but -> priorité -> déclenchement` line, for a report or a failure.
    pub fn columns(&self) -> (u16, S, &'static str, u32, &'static str) {
        (self.priority, self.goal, self.when, self.tick_rate, self.java)
    }
}

/// Whether a table is a well-formed priority list: ranks `1..=len`, no gap, no
/// duplicate.
pub fn contiguous<S: Copy + core::fmt::Debug>(table: &[Rule<S>]) -> bool {
    table
        .iter()
        .enumerate()
        .all(|(index, rule)| rule.priority as usize == index + 1)
}

// ---------------------------------------------------------------------------
// Eating — `minimal/EntityAIEatTask.java:80-91`
// ---------------------------------------------------------------------------

/// The nine states of `EntityAIEatTask.EatingState` (`:80-91`), plus the citizen
/// state it is entered from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: nine substates in Java, plus the entry and the exit
pub enum EatState {
    /// `CitizenAIState.EATING` — the state the hero transition hangs on
    /// (`:148`).
    CitizenEating,
    /// `CHECK_FOR_FOOD` (`:82`).
    CheckForFood,
    /// `GO_TO_HUT` (`:83`).
    GoToHut,
    /// `SEARCH_RESTAURANT` (`:84`).
    SearchRestaurant,
    /// `GO_TO_RESTAURANT` (`:85`).
    GoToRestaurant,
    /// `WAIT_FOR_FOOD` (`:86`).
    WaitForFood,
    /// `GET_FOOD_YOURSELF` (`:87`).
    GetFoodYourself,
    /// `GO_TO_EAT_POS` (`:88`).
    GoToEatPos,
    /// `EAT` (`:89`).
    Eat,
    /// `DONE` (`:90`) — one tick later `CitizenAIState.IDLE` (`:153`).
    Done,
    /// `CitizenAIState.IDLE` — what the citizen machine gets back. Not a Java
    /// substate of this class: it is the value every `return CitizenAIState.IDLE`
    /// of `eat` and `waitForFood` produces (`:242`, `:382`).
    Idle,
}

impl EatState {
    /// Java's identifier.
    pub fn java_name(self) -> &'static str {
        match self {
            EatState::CitizenEating => "EATING",
            EatState::Idle => "IDLE",
            EatState::CheckForFood => "CHECK_FOR_FOOD",
            EatState::GoToHut => "GO_TO_HUT",
            EatState::SearchRestaurant => "SEARCH_RESTAURANT",
            EatState::GoToRestaurant => "GO_TO_RESTAURANT",
            EatState::WaitForFood => "WAIT_FOR_FOOD",
            EatState::GetFoodYourself => "GET_FOOD_YOURSELF",
            EatState::GoToEatPos => "GO_TO_EAT_POS",
            EatState::Eat => "EAT",
            EatState::Done => "DONE",
        }
    }

    /// The nine substates, in `EatingState`'s declaration order (`:82-90`).
    pub const ALL: [EatState; 9] = [
        EatState::CheckForFood,
        EatState::GoToHut,
        EatState::SearchRestaurant,
        EatState::GoToRestaurant,
        EatState::WaitForFood,
        EatState::GetFoodYourself,
        EatState::GoToEatPos,
        EatState::Eat,
        EatState::Done,
    ];
}

/// `EntityAIEatTask`'s transitions in registration order (`:148-161`).
///
/// **The order is not the enum's order.** `EAT` is registered fifth (`:156`) and
/// declared eighth; `DONE` is registered second (`:153`) and declared last. What
/// decides is the list, not the declaration.
pub const EAT_TABLE: &[Rule<EatState>] = &[
    Rule {
        priority: 1,
        goal: EatState::CheckForFood,
        rule: "hero",
        when: "always, when the citizen machine enters EATING",
        tick_rate: 20,
        java: "EntityAIEatTask.java:148",
    },
    Rule {
        priority: 2,
        goal: EatState::Idle,
        rule: "done_is_idle",
        when: "always, one tick after DONE",
        tick_rate: 1,
        java: "EntityAIEatTask.java:153",
    },
    Rule {
        priority: 3,
        goal: EatState::Eat,
        rule: "check_for_food",
        when: "food in the inventory, else go to the hut",
        tick_rate: 20,
        java: "EntityAIEatTask.java:154",
    },
    Rule {
        priority: 4,
        goal: EatState::GoToHut,
        rule: "at_hut",
        when: "arrived at the hut and found food there",
        tick_rate: 20,
        java: "EntityAIEatTask.java:155",
    },
    Rule {
        priority: 5,
        goal: EatState::Eat,
        rule: "eat",
        when: "five ticks of chewing, one stack at a time",
        tick_rate: 20,
        java: "EntityAIEatTask.java:156",
    },
    Rule {
        priority: 6,
        goal: EatState::SearchRestaurant,
        rule: "search_restaurant",
        when: "no cook in the colony, or the hut had nothing",
        tick_rate: 20,
        java: "EntityAIEatTask.java:157",
    },
    Rule {
        priority: 7,
        goal: EatState::GoToRestaurant,
        rule: "go_to_restaurant",
        when: "a cook building was found",
        tick_rate: 20,
        java: "EntityAIEatTask.java:158",
    },
    Rule {
        priority: 8,
        goal: EatState::WaitForFood,
        rule: "wait_for_food",
        when: "inside the restaurant",
        tick_rate: 20,
        java: "EntityAIEatTask.java:159",
    },
    Rule {
        priority: 9,
        goal: EatState::GoToEatPos,
        rule: "go_to_eat_pos",
        when: "the restaurant gave a free seat",
        tick_rate: 20,
        java: "EntityAIEatTask.java:160",
    },
    Rule {
        priority: 10,
        goal: EatState::GetFoodYourself,
        rule: "get_food_yourself",
        when: "two minutes without a cook, or a guard at night",
        tick_rate: 20,
        java: "EntityAIEatTask.java:161",
    },
];

// ---------------------------------------------------------------------------
// Sleeping — `minimal/EntityAISleep.java:75-80`
// ---------------------------------------------------------------------------

/// `EntityAISleep.SleepState` (`:75-80`), plus the entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: three substates in Java, plus the entry
pub enum SleepState {
    /// `CitizenAIState.SLEEP` (`:91`).
    CitizenSleep,
    /// `WALKING_HOME` (`:77`).
    WalkingHome,
    /// `FIND_BED` (`:78`).
    FindBed,
    /// `SLEEPING` (`:79`).
    Sleeping,
}

impl SleepState {
    /// Java's identifier.
    pub fn java_name(self) -> &'static str {
        match self {
            SleepState::CitizenSleep => "SLEEP",
            SleepState::WalkingHome => "WALKING_HOME",
            SleepState::FindBed => "FIND_BED",
            SleepState::Sleeping => "SLEEPING",
        }
    }
}

/// `EntityAISleep`'s transitions in registration order (`:91-95`).
pub const SLEEP_TABLE: &[Rule<SleepState>] = &[
    Rule {
        priority: 1,
        goal: SleepState::WalkingHome,
        rule: "check_sleep",
        when: "always, when the citizen machine enters SLEEP",
        tick_rate: 20,
        java: "EntityAISleep.java:91",
    },
    Rule {
        priority: 2,
        goal: SleepState::FindBed,
        rule: "walk_home",
        when: "home building reached, or within four blocks of the home position",
        tick_rate: 30,
        java: "EntityAISleep.java:93",
    },
    Rule {
        priority: 3,
        goal: SleepState::Sleeping,
        rule: "find_bed",
        when: "already asleep, or ten tries spent looking for a bed",
        tick_rate: 30,
        java: "EntityAISleep.java:94",
    },
    Rule {
        priority: 4,
        goal: SleepState::Sleeping,
        rule: "sleep",
        when: "still in bed; walks home again if the bed is more than three blocks away",
        tick_rate: 30,
        java: "EntityAISleep.java:95",
    },
];

// ---------------------------------------------------------------------------
// Being sick — `minimal/EntityAISickTask.java:91-101`
// ---------------------------------------------------------------------------

/// `EntityAISickTask.DiseaseState` (`:91-101`), plus the entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: eight substates in Java, plus the entry
pub enum SickState {
    /// `CitizenAIState.SICK` (`:124`).
    CitizenSick,
    /// `CHECK_FOR_CURE` (`:93`).
    CheckForCure,
    /// `GO_TO_HUT` (`:94`).
    GoToHut,
    /// `SEARCH_HOSPITAL` (`:95`).
    SearchHospital,
    /// `GO_TO_HOSPITAL` (`:96`).
    GoToHospital,
    /// `WAIT_FOR_CURE` (`:97`).
    WaitForCure,
    /// `FIND_EMPTY_BED` (`:98`).
    FindEmptyBed,
    /// `APPLY_CURE` (`:99`).
    ApplyCure,
    /// `WANDER` (`:100`).
    Wander,
    /// `CitizenAIState.IDLE` — what `checkForCure`, `applyCure`, `waitForCure` and
    /// `searchHospital` hand back (`:247`, `:272`, `:331`, `:423`).
    Idle,
}

impl SickState {
    /// Java's identifier.
    pub fn java_name(self) -> &'static str {
        match self {
            SickState::CitizenSick => "SICK",
            SickState::Idle => "IDLE",
            SickState::CheckForCure => "CHECK_FOR_CURE",
            SickState::GoToHut => "GO_TO_HUT",
            SickState::SearchHospital => "SEARCH_HOSPITAL",
            SickState::GoToHospital => "GO_TO_HOSPITAL",
            SickState::WaitForCure => "WAIT_FOR_CURE",
            SickState::FindEmptyBed => "FIND_EMPTY_BED",
            SickState::ApplyCure => "APPLY_CURE",
            SickState::Wander => "WANDER",
        }
    }
}

/// `EntityAISickTask`'s transitions in registration order (`:124-134`).
///
/// **`CHECK_FOR_CURE` is registered twice**: `:125` and again `:128`, with the same
/// action and the same `() -> true` condition. The second copy can never run — the
/// first one has no condition and always produces a state — so the table keeps the
/// one that exists and `tests.rs` says the duplicate is dead rather than pretending
/// it is a second priority.
pub const SICK_TABLE: &[Rule<SickState>] = &[
    Rule {
        priority: 1,
        goal: SickState::CheckForCure,
        rule: "hero",
        when: "the citizen machine enters SICK and the citizen is sick or hurt",
        tick_rate: 20,
        java: "EntityAISickTask.java:124",
    },
    Rule {
        priority: 2,
        goal: SickState::CheckForCure,
        rule: "check_for_cure",
        when: "always (registered twice, at :125 and :128)",
        tick_rate: 20,
        java: "EntityAISickTask.java:125",
    },
    Rule {
        priority: 3,
        goal: SickState::CheckForCure,
        rule: "wander",
        when: "no hospital: a bit of walking, then look again",
        tick_rate: 200,
        java: "EntityAISickTask.java:126",
    },
    Rule {
        priority: 4,
        goal: SickState::GoToHut,
        rule: "go_to_hut",
        when: "no cure in the inventory, or a doctor is not needed",
        tick_rate: 20,
        java: "EntityAISickTask.java:129",
    },
    Rule {
        priority: 5,
        goal: SickState::SearchHospital,
        rule: "search_hospital",
        when: "the hut is reached, or no work building",
        tick_rate: 20,
        java: "EntityAISickTask.java:130",
    },
    Rule {
        priority: 6,
        goal: SickState::GoToHospital,
        rule: "go_to_hospital",
        when: "a hospital exists",
        tick_rate: 20,
        java: "EntityAISickTask.java:131",
    },
    Rule {
        priority: 7,
        goal: SickState::WaitForCure,
        rule: "wait_for_cure",
        when: "inside the hospital",
        tick_rate: 20,
        java: "EntityAISickTask.java:132",
    },
    Rule {
        priority: 8,
        goal: SickState::ApplyCure,
        rule: "apply_cure",
        when: "every cure item is in the inventory",
        tick_rate: 20,
        java: "EntityAISickTask.java:133",
    },
    Rule {
        priority: 9,
        goal: SickState::FindEmptyBed,
        rule: "find_empty_bed",
        when: "asleep in the hospital and no cure yet",
        tick_rate: 20,
        java: "EntityAISickTask.java:134",
    },
];

// ---------------------------------------------------------------------------
// Mourning — `minimal/EntityAIMournCitizen.java:39-48`
// ---------------------------------------------------------------------------

/// `EntityAIMournCitizen.MourningState` (`:39-48`), plus the entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: seven substates in Java, plus the entry
pub enum MournState {
    /// `CitizenAIState.MOURN` (`:94`).
    CitizenMourn,
    /// `DECIDE` (`:41`).
    Decide,
    /// `WALKING_TO_TOWNHALL` (`:42`).
    WalkingToTownHall,
    /// `WANDERING` (`:43`).
    Wandering,
    /// `STARING` (`:44`).
    Staring,
    /// `WALKING_TO_GRAVEYARD` (`:45`).
    WalkingToGraveyard,
    /// `WANDER_AT_GRAVEYARD` (`:46`).
    WanderAtGraveyard,
    /// `WALK_TO_GRAVE` (`:47`).
    WalkToGrave,
    /// `CitizenAIState.IDLE` — the four places the mourner stops mourning
    /// (`:117`, `:125`, `:239`, `:252`).
    Idle,
}

impl MournState {
    /// Java's identifier.
    pub fn java_name(self) -> &'static str {
        match self {
            MournState::CitizenMourn => "MOURN",
            MournState::Idle => "IDLE",
            MournState::Decide => "DECIDE",
            MournState::WalkingToTownHall => "WALKING_TO_TOWNHALL",
            MournState::Wandering => "WANDERING",
            MournState::Staring => "STARING",
            MournState::WalkingToGraveyard => "WALKING_TO_GRAVEYARD",
            MournState::WanderAtGraveyard => "WANDER_AT_GRAVEYARD",
            MournState::WalkToGrave => "WALK_TO_GRAVE",
        }
    }
}

/// `EntityAIMournCitizen`'s transitions in registration order (`:94-104`).
pub const MOURN_TABLE: &[Rule<MournState>] = &[
    Rule {
        priority: 1,
        goal: MournState::Decide,
        rule: "hero",
        when: "always, when the citizen machine enters MOURN (the entry resets)",
        tick_rate: 20,
        java: "EntityAIMournCitizen.java:94",
    },
    Rule {
        priority: 2,
        goal: MournState::Decide,
        rule: "decide",
        when: "always, once a tick",
        tick_rate: 20,
        java: "EntityAIMournCitizen.java:98",
    },
    Rule {
        priority: 3,
        goal: MournState::Decide,
        rule: "walk_to_townhall",
        when: "arrived at the town hall",
        tick_rate: 20,
        java: "EntityAIMournCitizen.java:99",
    },
    Rule {
        priority: 4,
        goal: MournState::Wandering,
        rule: "wander",
        when: "always",
        tick_rate: 20,
        java: "EntityAIMournCitizen.java:100",
    },
    Rule {
        priority: 5,
        goal: MournState::Staring,
        rule: "stare",
        when: "always",
        tick_rate: 20,
        java: "EntityAIMournCitizen.java:101",
    },
    Rule {
        priority: 6,
        goal: MournState::WanderAtGraveyard,
        rule: "walk_to_graveyard",
        when: "arrived at the graveyard",
        tick_rate: 20,
        java: "EntityAIMournCitizen.java:102",
    },
    Rule {
        priority: 7,
        goal: MournState::WanderAtGraveyard,
        rule: "wander_at_graveyard",
        when: "always",
        tick_rate: 20,
        java: "EntityAIMournCitizen.java:103",
    },
    Rule {
        priority: 8,
        goal: MournState::Decide,
        rule: "walk_to_grave",
        when: "arrived at the named grave",
        tick_rate: 20,
        java: "EntityAIMournCitizen.java:104",
    },
];

// ---------------------------------------------------------------------------
// Idle wandering — `minimal/EntityAICitizenWander.java:43-48`
// ---------------------------------------------------------------------------

/// `EntityAICitizenWander.WanderState` (`:43-48`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: three substates in Java, plus the idle entry
pub enum WanderState {
    /// `CitizenAIState.IDLE` (`:82`).
    CitizenIdle,
    /// `GO_TO_LEISURE_SITE` (`:45`).
    GoToLeisureSite,
    /// `WANDER_AT_LEISURE_SITE` (`:46`).
    WanderAtLeisureSite,
    /// `READ_A_BOOK` (`:47`).
    ReadABook,
}

impl WanderState {
    /// Java's identifier.
    pub fn java_name(self) -> &'static str {
        match self {
            WanderState::CitizenIdle => "IDLE",
            WanderState::GoToLeisureSite => "GO_TO_LEISURE_SITE",
            WanderState::WanderAtLeisureSite => "WANDER_AT_LEISURE_SITE",
            WanderState::ReadABook => "READ_A_BOOK",
        }
    }
}

/// `EntityAICitizenWander`'s transitions in registration order (`:82-85`).
pub const WANDER_TABLE: &[Rule<WanderState>] = &[
    Rule {
        priority: 1,
        goal: WanderState::GoToLeisureSite,
        rule: "decide",
        when: "five in a hundred, and a leisure site exists",
        tick_rate: 100,
        java: "EntityAICitizenWander.java:82",
    },
    Rule {
        priority: 2,
        goal: WanderState::WanderAtLeisureSite,
        rule: "go_to_leisure_site",
        when: "arrived within three blocks",
        tick_rate: 20,
        java: "EntityAICitizenWander.java:83",
    },
    Rule {
        priority: 3,
        goal: WanderState::ReadABook,
        rule: "wander_at_leisure_site",
        when: "a library or university spot with a free seat",
        tick_rate: 20,
        java: "EntityAICitizenWander.java:84",
    },
    Rule {
        priority: 4,
        goal: WanderState::CitizenIdle,
        rule: "read_a_book",
        when: "five in a hundred, or no book spot",
        tick_rate: 20,
        java: "EntityAICitizenWander.java:85",
    },
];

// ---------------------------------------------------------------------------
// Children — `minimal/EntityAICitizenChild.java:42-54`
// ---------------------------------------------------------------------------

/// `EntityAICitizenChild.State` (`:42-54`) — the only `IAIState` of this module,
/// so the only one with an `isOkayToEat` answer, which is `true` (`:49-53`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: four substates in Java
pub enum ChildState {
    /// `BORED` (`:44`).
    Bored,
    /// `PLAYING` (`:45`) — declared and never used.
    Playing,
    /// `VISITING` (`:46`).
    Visiting,
    /// `FOLLOWING` (`:47`).
    Following,
}

impl ChildState {
    /// Java's identifier.
    pub fn java_name(self) -> &'static str {
        match self {
            ChildState::Bored => "BORED",
            ChildState::Playing => "PLAYING",
            ChildState::Visiting => "VISITING",
            ChildState::Following => "FOLLOWING",
        }
    }

    /// `isOkayToEat()` (`:49-53`) — `true` for every child state.
    pub fn okay_to_eat(self) -> bool {
        true
    }
}

/// `EntityAICitizenChild`'s transitions in registration order (`:114-119`).
pub const CHILD_TABLE: &[GoalRule] = &[
    GoalRule {
        rule: "visit",
        when: "idle, a 1-in-10 draw, and no activity running",
        java: "EntityAICitizenChild.java:114",
    },
    GoalRule {
        rule: "grow_up",
        when: "every 500 ticks, in the EVENT pass, while a baby",
        java: "EntityAICitizenChild.java:115",
    },
    GoalRule {
        rule: "follow",
        when: "idle, a 1-in-10 draw, and a civilian or player within ten blocks",
        java: "EntityAICitizenChild.java:116",
    },
    GoalRule {
        rule: "following_entity",
        when: "always, while following",
        java: "EntityAICitizenChild.java:118",
    },
    GoalRule {
        rule: "visit_huts",
        when: "always, while visiting",
        java: "EntityAICitizenChild.java:119",
    },
];

// ---------------------------------------------------------------------------
// Fleeing — `minimal/EntityAICitizenAvoidEntity.java:68-72`
// ---------------------------------------------------------------------------

/// `EntityAICitizenAvoidEntity.FleeStates` (`:68-72`), plus the entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: two substates in Java, plus the entry
pub enum FleeState {
    /// `CitizenAIState.FLEE` (`:105`).
    CitizenFlee,
    /// `CHECK_ENTITIES` (`:70`).
    CheckEntities,
    /// `RUNNING` (`:71`).
    Running,
    /// `CitizenAIState.IDLE` — the flee ends when forty checks found nothing
    /// (`:122-126`).
    Idle,
}

impl FleeState {
    /// Java's identifier.
    pub fn java_name(self) -> &'static str {
        match self {
            FleeState::CitizenFlee => "FLEE",
            FleeState::Idle => "IDLE",
            FleeState::CheckEntities => "CHECK_ENTITIES",
            FleeState::Running => "RUNNING",
        }
    }
}

/// `EntityAICitizenAvoidEntity`'s transitions in registration order (`:105-110`).
pub const FLEE_TABLE: &[Rule<FleeState>] = &[
    Rule {
        priority: 1,
        goal: FleeState::CheckEntities,
        rule: "remember_start",
        when: "always, when the citizen machine enters FLEE",
        tick_rate: 1,
        java: "EntityAICitizenAvoidEntity.java:105",
    },
    Rule {
        priority: 2,
        goal: FleeState::Running,
        rule: "is_entity_close",
        when: "something to avoid is within range; forty clean checks end the flee",
        tick_rate: 5,
        java: "EntityAICitizenAvoidEntity.java:109",
    },
    Rule {
        priority: 3,
        goal: FleeState::CheckEntities,
        rule: "update_moving",
        when: "the path away is no longer in progress",
        tick_rate: 5,
        java: "EntityAICitizenAvoidEntity.java:110",
    },
];

// ---------------------------------------------------------------------------
// Combat — `combat/TargetAI.java` and `combat/AttackMoveAI.java`
// ---------------------------------------------------------------------------

/// `CombatAIStates` (`api/entity/ai/combat/CombatAIStates.java:10-13`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: four combat states in Java
pub enum CombatState {
    /// `NO_TARGET` (`:13`), the only one that is okay to eat.
    NoTarget,
    /// `ATTACKING` (`:10`).
    Attacking,
    /// `FIND_MOUNT` (`:11`) — the cavalry guard only.
    FindMount,
    /// `FIND_STABLE` (`:12`) — the cavalry guard only.
    FindStable,
}

impl CombatState {
    /// Java's identifier.
    pub fn java_name(self) -> &'static str {
        match self {
            CombatState::NoTarget => "NO_TARGET",
            CombatState::Attacking => "ATTACKING",
            CombatState::FindMount => "FIND_MOUNT",
            CombatState::FindStable => "FIND_STABLE",
        }
    }

    /// `isOkayToEat()` (`CombatAIStates.java:10-13`, `:35-38`): only `NO_TARGET`.
    pub fn okay_to_eat(self) -> bool {
        self == CombatState::NoTarget
    }
}

/// `TargetAI`'s two transitions (`:45-46`) then `AttackMoveAI`'s two (`:46-47`).
///
/// The frequency `TargetAI` passes to its search is the constructor argument
/// (`:42`); `AttackMoveAI` passes `80` (`:44`) and the guards keep it.
pub const COMBAT_TABLE: &[Rule<CombatState>] = &[
    Rule {
        priority: 1,
        goal: CombatState::Attacking,
        rule: "check_for_target",
        when: "the threat table has a valid target",
        tick_rate: 5,
        java: "TargetAI.java:45",
    },
    Rule {
        priority: 2,
        goal: CombatState::Attacking,
        rule: "search_nearby_target",
        when: "a visible enemy is inside the shaped search box",
        tick_rate: 80,
        java: "TargetAI.java:46",
    },
    Rule {
        priority: 3,
        goal: CombatState::NoTarget,
        rule: "try_attack",
        when: "no target, or no weapon, or the swing timer is not up",
        tick_rate: 5,
        java: "AttackMoveAI.java:46",
    },
    Rule {
        priority: 4,
        goal: CombatState::NoTarget,
        rule: "move",
        when: "no target, or twenty seconds without seeing it",
        tick_rate: 10,
        java: "AttackMoveAI.java:47",
    },
];

// ---------------------------------------------------------------------------
// Guards — `workers/guard/AbstractEntityAIGuard.java:168-181`
// ---------------------------------------------------------------------------

/// A goal of a guard's machine.
///
/// The guard's machine **is** the worker's: `AbstractEntityAIGuard` extends
/// `AbstractEntityAIFight` extends `AbstractEntityAIInteract`, so `PREPARING`,
/// `START_WORKING`, `NEEDS_ITEM` and `INVENTORY_FULL` are the same states `src/ai`
/// already ported. Rather than re-declare them, [`GuardGoal::Worker`] borrows
/// them — duplication would be two answers to one question.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GuardGoal {
    /// A state [`crate::ai::state::WorkerState`] already names.
    Worker(WorkerState),
    /// `CombatAIStates.NO_TARGET`.
    NoTarget,
    /// `CombatAIStates.ATTACKING`.
    Attacking,
    /// `GUARD_SLEEP` (`AIWorkerState.java:218`).
    GuardSleep,
    /// `GUARD_WAKE` (`:223`).
    GuardWake,
    /// `GUARD_REGEN` (`:238`).
    GuardRegen,
    /// `GUARD_FLEE` (`:243`).
    GuardFlee,
    /// `CombatAIStates.FIND_MOUNT` (`:11`).
    FindMount,
    /// `CombatAIStates.FIND_STABLE` (`:12`).
    FindStable,
}

impl GuardGoal {
    /// Java's identifier, borrowed for the worker states.
    pub fn java_name(self) -> &'static str {
        match self {
            GuardGoal::Worker(state) => state.java_name(),
            GuardGoal::NoTarget => "NO_TARGET",
            GuardGoal::Attacking => "ATTACKING",
            GuardGoal::GuardSleep => "GUARD_SLEEP",
            GuardGoal::GuardWake => "GUARD_WAKE",
            GuardGoal::GuardRegen => "GUARD_REGEN",
            GuardGoal::GuardFlee => "GUARD_FLEE",
            GuardGoal::FindMount => "FIND_MOUNT",
            GuardGoal::FindStable => "FIND_STABLE",
        }
    }

    /// `PREPARING` (`AIWorkerState.java:27`).
    pub const fn preparing() -> GuardGoal {
        GuardGoal::Worker(WorkerState::Preparing)
    }

    /// `START_WORKING` (`:31`).
    pub const fn start_working() -> GuardGoal {
        GuardGoal::Worker(WorkerState::StartWorking)
    }

    /// `DECIDE` (`:59`).
    pub const fn decide() -> GuardGoal {
        GuardGoal::Worker(WorkerState::Decide)
    }

    /// `INVENTORY_FULL` (`:23`).
    pub const fn inventory_full() -> GuardGoal {
        GuardGoal::Worker(WorkerState::InventoryFull)
    }

    /// `NEEDS_ITEM` (`:35`).
    pub const fn needs_item() -> GuardGoal {
        GuardGoal::Worker(WorkerState::NeedsItem)
    }

    /// `isOkayToEat()` — borrowed for the worker states, `true` only for
    /// `NO_TARGET` among the combat ones (`CombatAIStates.java:13`).
    pub fn okay_to_eat(self) -> bool {
        match self {
            GuardGoal::Worker(state) => state.okay_to_eat(),
            GuardGoal::NoTarget => true,
            GuardGoal::Attacking | GuardGoal::FindMount | GuardGoal::FindStable => false,
            // `AIWorkerState.java:218`, `:223`, `:238` are `true`; `:243` is
            // `false`: a guard may be woken to eat, not to run.
            GuardGoal::GuardSleep | GuardGoal::GuardWake | GuardGoal::GuardRegen => true,
            GuardGoal::GuardFlee => false,
        }
    }

    /// Whether this goal belongs to the combat layer.
    pub fn is_combat(self) -> bool {
        matches!(
            self,
            GuardGoal::NoTarget
                | GuardGoal::Attacking
                | GuardGoal::FindMount
                | GuardGoal::FindStable
        )
    }
}

/// `AbstractEntityAIGuard`'s transitions in registration order (`:168-181`),
/// **preceded** by the three its parent registers first: `AbstractEntityAIFight`'s
/// constructor runs before its subclass's (`:73-77`), so `IDLE`, `START_WORKING`
/// and `PREPARING` are the first three rules of the machine.
///
/// The last two are the ones that make a raid fight: `NO_TARGET --decide-->` the
/// task's own movement, and `ATTACKING --inCombat-->` keeps the fight timer up.
pub const GUARD_TABLE: &[Rule<GuardGoal>] = &[
    Rule {
        priority: 1,
        goal: GuardGoal::start_working(),
        rule: "idle",
        when: "always, from IDLE",
        tick_rate: 1,
        java: "AbstractEntityAIFight.java:74",
    },
    Rule {
        priority: 2,
        // `AbstractEntityAIGuard.java:487-494` **ecrase** la version de la classe
        // parente et rend `DECIDE` sans marcher : un garde ne passe pas par
        // `PREPARING` en rentrant au hut.
        goal: GuardGoal::decide(),
        rule: "start_working",
        when: "always: the guard overrides `startWorkingAtOwnBuilding` and goes straight to DECIDE",
        tick_rate: 100,
        java: "AbstractEntityAIGuard.java:493",
    },
    Rule {
        priority: 3,
        goal: GuardGoal::decide(),
        rule: "prepare",
        when: "weapon in hand, armour equipped, and the request list settled",
        tick_rate: 20,
        java: "AbstractEntityAIFight.java:76",
    },
    Rule {
        priority: 4,
        goal: GuardGoal::NoTarget,
        rule: "decide_to_no_target",
        when: "always, when the guard reaches DECIDE",
        tick_rate: 1,
        java: "AbstractEntityAIGuard.java:169",
    },
    Rule {
        priority: 5,
        goal: GuardGoal::GuardSleep,
        rule: "should_sleep",
        when: "not hurt, no target, no fight timer, healthy, and a 1-in-(20+level/2) draw",
        tick_rate: 200,
        java: "AbstractEntityAIGuard.java:170",
    },
    Rule {
        priority: 6,
        goal: GuardGoal::NoTarget,
        rule: "sleep",
        when: "hurt, or the nap timer ran out",
        tick_rate: 1,
        java: "AbstractEntityAIGuard.java:171",
    },
    Rule {
        priority: 7,
        goal: GuardGoal::NoTarget,
        rule: "sleep_particles",
        when: "always while asleep: particles and half a point of health",
        tick_rate: 30,
        java: "AbstractEntityAIGuard.java:172",
    },
    Rule {
        priority: 8,
        goal: GuardGoal::GuardRegen,
        rule: "regen",
        when: "below 75% health and the hut retrieves, until half is back",
        tick_rate: 40,
        java: "AbstractEntityAIGuard.java:173",
    },
    Rule {
        priority: 9,
        goal: GuardGoal::GuardRegen,
        rule: "flee",
        when: "arrived at the hut",
        tick_rate: 20,
        java: "AbstractEntityAIGuard.java:174",
    },
    Rule {
        priority: 10,
        goal: GuardGoal::GuardFlee,
        rule: "should_flee_in_combat",
        when: "below 20% health, away from the hut, hut retrieves, and RETREAT researched",
        tick_rate: 40,
        java: "AbstractEntityAIGuard.java:175",
    },
    Rule {
        priority: 11,
        goal: GuardGoal::GuardFlee,
        rule: "should_flee_idle",
        when: "same, also checked out of combat",
        tick_rate: 40,
        java: "AbstractEntityAIGuard.java:176",
    },
    Rule {
        priority: 12,
        goal: GuardGoal::NoTarget,
        rule: "guard_decide",
        when: "the task's own movement: guard, patrol, follow or patrol the mine",
        tick_rate: 100,
        java: "AbstractEntityAIGuard.java:177",
    },
    Rule {
        priority: 13,
        goal: GuardGoal::NoTarget,
        rule: "wake_up_guard",
        when: "a sleeping comrade was spotted: walk over and hit it",
        tick_rate: 20,
        java: "AbstractEntityAIGuard.java:178",
    },
    Rule {
        priority: 14,
        goal: GuardGoal::Attacking,
        rule: "in_combat",
        when: "in combat: keep the fight timer up, or go and fetch a weapon",
        tick_rate: 8,
        java: "AbstractEntityAIGuard.java:180",
    },
];

/// The two states a guard may abort to fight from
/// (`BehaviourStateGroup.java:16`), with the two rules each of the four combat
/// AIs inserts in front of them (`MeleeCombatAI.java:109-110` and its siblings).
pub const GUARD_ABORT_GROUP: [GuardGoal; 2] = [
    GuardGoal::Worker(WorkerState::NeedsItem),
    GuardGoal::Worker(WorkerState::InventoryFull),
];

/// The group's rules, in the order `MeleeCombatAI` adds them: `checkForTarget` at
/// 5 ticks, then `searchNearbyTarget` at 80.
pub const GUARD_ABORT_TABLE: &[Rule<CombatState>] = &[
    Rule {
        priority: 1,
        goal: CombatState::Attacking,
        rule: "busy_check_target",
        when: "a threat target exists while fetching or dumping",
        tick_rate: 5,
        java: "MeleeCombatAI.java:109",
    },
    Rule {
        priority: 2,
        goal: CombatState::Attacking,
        rule: "busy_search_target",
        when: "an enemy is visible while fetching or dumping",
        tick_rate: 80,
        java: "MeleeCombatAI.java:110",
    },
];

// ---------------------------------------------------------------------------
// Training — `workers/guard/training/`
// ---------------------------------------------------------------------------

/// A goal of a training academy's machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TrainingGoal {
    /// A state [`crate::ai::state::WorkerState`] already names.
    Worker(WorkerState),
    /// `TRAINING_WANDER` (`AIWorkerState.java:446`).
    TrainingWander,
    /// `GO_TO_TARGET` (`:451`).
    GoToTarget,
    /// `COMBAT_TRAINING` (`:456`).
    CombatTraining,
    /// `ARCHER_SELECT_TARGET` (`:470`).
    ArcherSelectTarget,
    /// `ARCHER_CHECK_SHOT` (`:475`).
    ArcherCheckShot,
    /// `ARCHER_SHOOT` (`:480`).
    ArcherShoot,
    /// `FIND_TRAINING_PARTNER` (`:499`).
    FindTrainingPartner,
    /// `KNIGHT_TRAIN_WITH_PARTNER` (`:504`).
    KnightTrainWithPartner,
    /// `FIND_DUMMY_PARTNER` (`:494`).
    FindDummyPartner,
    /// `KNIGHT_ATTACK_DUMMY` (`:489`).
    KnightAttackDummy,
    /// `KNIGHT_ATTACK_PROTECT` (`:509`).
    KnightAttackProtect,
}

impl TrainingGoal {
    /// Java's identifier.
    pub fn java_name(self) -> &'static str {
        match self {
            TrainingGoal::Worker(state) => state.java_name(),
            TrainingGoal::TrainingWander => "TRAINING_WANDER",
            TrainingGoal::GoToTarget => "GO_TO_TARGET",
            TrainingGoal::CombatTraining => "COMBAT_TRAINING",
            TrainingGoal::ArcherSelectTarget => "ARCHER_SELECT_TARGET",
            TrainingGoal::ArcherCheckShot => "ARCHER_CHECK_SHOT",
            TrainingGoal::ArcherShoot => "ARCHER_SHOOT",
            TrainingGoal::FindTrainingPartner => "FIND_TRAINING_PARTNER",
            TrainingGoal::KnightTrainWithPartner => "KNIGHT_TRAIN_WITH_PARTNER",
            TrainingGoal::FindDummyPartner => "FIND_DUMMY_PARTNER",
            TrainingGoal::KnightAttackDummy => "KNIGHT_ATTACK_DUMMY",
            TrainingGoal::KnightAttackProtect => "KNIGHT_ATTACK_PROTECT",
        }
    }
}

/// `AbstractEntityAITraining`'s transitions (`:54-60`).
pub const TRAINING_TABLE: &[Rule<TrainingGoal>] = &[
    Rule {
        priority: 1,
        goal: TrainingGoal::Worker(WorkerState::StartWorking),
        rule: "idle",
        when: "always, from IDLE",
        tick_rate: 1,
        java: "AbstractEntityAITraining.java:55",
    },
    Rule {
        priority: 2,
        goal: TrainingGoal::Worker(WorkerState::Decide),
        rule: "start_working",
        when: "always, from START_WORKING",
        tick_rate: 1,
        java: "AbstractEntityAITraining.java:56",
    },
    Rule {
        priority: 3,
        goal: TrainingGoal::CombatTraining,
        rule: "decide",
        when: "set up, and a 30-in-100 draw",
        tick_rate: 20,
        java: "AbstractEntityAITraining.java:57",
    },
    Rule {
        priority: 4,
        goal: TrainingGoal::TrainingWander,
        rule: "wander",
        when: "the walk inside the hut's corners arrived",
        tick_rate: 200,
        java: "AbstractEntityAITraining.java:58",
    },
    Rule {
        priority: 5,
        goal: TrainingGoal::Worker(WorkerState::Decide),
        rule: "path_to_target",
        when: "arrived at the shooting stand or the dummy",
        tick_rate: 20,
        java: "AbstractEntityAITraining.java:59",
    },
];

/// `EntityAIArcherTraining`'s own transitions (`:100-106`).
pub const ARCHER_TRAINING_TABLE: &[Rule<TrainingGoal>] = &[
    Rule {
        priority: 1,
        goal: TrainingGoal::GoToTarget,
        rule: "find_shooting_stand",
        when: "the archery hut gave a stand position",
        tick_rate: 5,
        java: "EntityAIArcherTraining.java:101",
    },
    Rule {
        priority: 2,
        goal: TrainingGoal::ArcherShoot,
        rule: "select_target",
        when: "a target is loaded, in line of sight, and the counter allows it",
        tick_rate: 5,
        java: "EntityAIArcherTraining.java:102",
    },
    Rule {
        priority: 3,
        goal: TrainingGoal::ArcherSelectTarget,
        rule: "check_shot",
        when: "three seconds after the arrow left",
        tick_rate: 60,
        java: "EntityAIArcherTraining.java:103",
    },
    Rule {
        priority: 4,
        goal: TrainingGoal::ArcherCheckShot,
        rule: "shoot",
        when: "the bow is drawn",
        tick_rate: 5,
        java: "EntityAIArcherTraining.java:104",
    },
];

/// `EntityAICombatTraining`'s own transitions (`:80-87`).
pub const COMBAT_TRAINING_TABLE: &[Rule<TrainingGoal>] = &[
    Rule {
        priority: 1,
        goal: TrainingGoal::FindTrainingPartner,
        rule: "decide_on_training_type",
        when: "a partner is assigned, or a 25-in-100 draw",
        tick_rate: 20,
        java: "EntityAICombatTraining.java:81",
    },
    Rule {
        priority: 2,
        goal: TrainingGoal::KnightTrainWithPartner,
        rule: "find_training_partner",
        when: "a partner was found",
        tick_rate: 20,
        java: "EntityAICombatTraining.java:82",
    },
    Rule {
        priority: 3,
        goal: TrainingGoal::KnightAttackProtect,
        rule: "train_with_partner",
        when: "within five blocks of the partner",
        tick_rate: 20,
        java: "EntityAICombatTraining.java:83",
    },
    Rule {
        priority: 4,
        goal: TrainingGoal::GoToTarget,
        rule: "find_dummy_partner",
        when: "the academy gave a dummy position",
        tick_rate: 20,
        java: "EntityAICombatTraining.java:84",
    },
    Rule {
        priority: 5,
        goal: TrainingGoal::FindDummyPartner,
        rule: "attack_dummy",
        when: "the attack delay ran out",
        tick_rate: 20,
        java: "EntityAICombatTraining.java:85",
    },
    Rule {
        priority: 6,
        goal: TrainingGoal::KnightTrainWithPartner,
        rule: "attack_protect",
        when: "three seconds between swings",
        tick_rate: 60,
        java: "EntityAICombatTraining.java:86",
    },
];

// ---------------------------------------------------------------------------
// A horse going home — `cavalry/ReturnToStableGoal.java:77-109`
// ---------------------------------------------------------------------------

/// `ReturnToStableGoal.State` (`:77-84`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: five states in Java
pub enum StableState {
    /// `INIT` (`:79`).
    Init,
    /// `WALK_TO_STABLE` (`:80`).
    WalkToStable,
    /// `FIND_STALL` (`:81`).
    FindStall,
    /// `WALK_TO_STALL` (`:82`).
    WalkToStall,
    /// `DONE` (`:83`).
    Done,
}

impl StableState {
    /// Java's identifier.
    pub fn java_name(self) -> &'static str {
        match self {
            StableState::Init => "INIT",
            StableState::WalkToStable => "WALK_TO_STABLE",
            StableState::FindStall => "FIND_STALL",
            StableState::WalkToStall => "WALK_TO_STALL",
            StableState::Done => "DONE",
        }
    }
}

/// `ReturnToStableGoal`'s transitions (`:99-108`).
pub const STABLE_TABLE: &[Rule<StableState>] = &[
    Rule {
        priority: 1,
        goal: StableState::WalkToStable,
        rule: "resolve_stable",
        when: "the horse has a stable building in its colony",
        tick_rate: 20,
        java: "ReturnToStableGoal.java:100",
    },
    Rule {
        priority: 2,
        goal: StableState::FindStall,
        rule: "walk_to_stable",
        when: "inside the stable, or the walk ended",
        tick_rate: 10,
        java: "ReturnToStableGoal.java:102",
    },
    Rule {
        priority: 3,
        goal: StableState::WalkToStall,
        rule: "find_stall",
        when: "the stable gave a free stall",
        tick_rate: 20,
        java: "ReturnToStableGoal.java:104",
    },
    Rule {
        priority: 4,
        goal: StableState::Done,
        rule: "walk_to_stall",
        when: "the stall was reached, or there is no stall",
        tick_rate: 10,
        java: "ReturnToStableGoal.java:106",
    },
    Rule {
        priority: 5,
        goal: StableState::Done,
        rule: "stay_done",
        when: "always, once done",
        tick_rate: 20,
        java: "ReturnToStableGoal.java:108",
    },
];

// ---------------------------------------------------------------------------
// A visitor in the tavern — `visitor/EntityAIVisitor.java:36-43`
// ---------------------------------------------------------------------------

/// `EntityAIVisitor.VisitorState` (`:36-43`), plus `INIT`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: five states in Java, plus the entry
pub enum VisitorState {
    /// `EntityState.INIT` (`:76`).
    Init,
    /// `IDLE` (`:38`).
    Idle,
    /// `SLEEPING` (`:39`) — declared and never used.
    Sleeping,
    /// `SITTING` (`:40`).
    Sitting,
    /// `COMBAT` (`:41`).
    Combat,
    /// `WANDERING` (`:42`).
    Wandering,
}

impl VisitorState {
    /// Java's identifier.
    pub fn java_name(self) -> &'static str {
        match self {
            VisitorState::Init => "INIT",
            VisitorState::Idle => "IDLE",
            VisitorState::Sleeping => "SLEEPING",
            VisitorState::Sitting => "SITTING",
            VisitorState::Combat => "COMBAT",
            VisitorState::Wandering => "WANDERING",
        }
    }
}

/// `EntityAIVisitor`'s transitions (`:76-85`).
pub const VISITOR_TABLE: &[Rule<VisitorState>] = &[
    Rule {
        priority: 1,
        goal: VisitorState::Idle,
        rule: "is_entity_loaded",
        when: "colony, citizen data, home building and chunk are all there",
        tick_rate: 50,
        java: "EntityAIVisitor.java:76",
    },
    Rule {
        priority: 2,
        goal: VisitorState::Combat,
        rule: "decide",
        when: "a target first, then 1-in-5 for a seat (2-in-5 at night), then a wander on a 2, else idle",
        tick_rate: 50,
        java: "EntityAIVisitor.java:77",
    },
    Rule {
        priority: 3,
        goal: VisitorState::Idle,
        rule: "wander_done",
        when: "the wander timer ran out",
        tick_rate: 200,
        java: "EntityAIVisitor.java:78",
    },
    Rule {
        priority: 4,
        goal: VisitorState::Combat,
        rule: "wander_fight",
        when: "a target appeared while wandering",
        tick_rate: 200,
        java: "EntityAIVisitor.java:79",
    },
    Rule {
        priority: 5,
        goal: VisitorState::Idle,
        rule: "sit_done",
        when: "the sitting timer ran out",
        tick_rate: 50,
        java: "EntityAIVisitor.java:80",
    },
    Rule {
        priority: 6,
        goal: VisitorState::Combat,
        rule: "sit_fight",
        when: "a target appeared while sitting",
        tick_rate: 50,
        java: "EntityAIVisitor.java:81",
    },
    Rule {
        priority: 7,
        goal: VisitorState::Idle,
        rule: "do_fight",
        when: "the target died, or thirty seconds passed",
        tick_rate: 20,
        java: "EntityAIVisitor.java:82",
    },
    Rule {
        priority: 8,
        goal: VisitorState::Idle,
        rule: "reduce_time_idle",
        when: "**unreachable**: `decide` always answers a state, so it consumes the tick before this rule is looked at",
        tick_rate: 50,
        java: "EntityAIVisitor.java:83",
    },
    Rule {
        priority: 9,
        goal: VisitorState::Wandering,
        rule: "reduce_time_wander",
        when: "same, while wandering",
        tick_rate: 50,
        java: "EntityAIVisitor.java:84",
    },
    Rule {
        priority: 10,
        goal: VisitorState::Sitting,
        rule: "reduce_time_sit",
        when: "same, while sitting",
        tick_rate: 50,
        java: "EntityAIVisitor.java:85",
    },
];

// ---------------------------------------------------------------------------
// Vanilla goals: no machine, just a trigger
// ---------------------------------------------------------------------------

/// `LookAtEntityGoal`'s three steps (`minimal/LookAtEntityGoal.java`).
pub const LOOK_RULES: &[GoalRule] = &[
    GoalRule {
        rule: "can_use",
        when: "a probability roll passes, something is in range, and the guard is awake",
        java: "LookAtEntityGoal.java:48-80",
    },
    GoalRule {
        rule: "start",
        when: "always: look for 40 to 80 ticks",
        java: "LookAtEntityGoal.java:99-103",
    },
    GoalRule {
        rule: "can_continue",
        when: "the watched entity is alive, in range, and the timer is positive",
        java: "LookAtEntityGoal.java:82-97",
    },
    GoalRule {
        rule: "tick",
        when: "every tick while watching",
        java: "LookAtEntityGoal.java:111-120",
    },
];

/// `EntityAIInteractToggleAble`'s steps (`minimal/EntityAIInteractToggleAble.java`).
pub const TOGGLE_RULES: &[GoalRule] = &[
    GoalRule {
        rule: "can_use_mount_collision",
        when: "the rider's mount collided and ten ticks passed",
        java: "EntityAIInteractToggleAble.java:120-129",
    },
    GoalRule {
        rule: "can_use_collision",
        when: "the citizen collided, or is climbing, and ten ticks passed",
        java: "EntityAIInteractToggleAble.java:131-136",
    },
    GoalRule {
        rule: "can_use_slow",
        when: "every fifty ticks, as a safety net",
        java: "EntityAIInteractToggleAble.java:138-143",
    },
    GoalRule {
        rule: "can_continue",
        when: "the path still holds a toggleable block",
        java: "EntityAIInteractToggleAble.java:342-346",
    },
    GoalRule {
        rule: "tick",
        when: "the random timer ran out: open one valid block, close the ones left behind",
        java: "EntityAIInteractToggleAble.java:398-462",
    },
    GoalRule {
        rule: "reset_all",
        when: "the path is done: close what this goal opened",
        java: "EntityAIInteractToggleAble.java:351-369",
    },
];

/// `EntityAIFloat`'s two branches (`minimal/EntityAIFloat.java`).
pub const FLOAT_RULES: &[GoalRule] = &[
    GoalRule {
        rule: "escape_water",
        when: "the eyes are in a swimmable fluid, the block above is not air, and no water path is running",
        java: "EntityAIFloat.java:83-105",
    },
    GoalRule {
        rule: "stop_escape",
        when: "the eyes are out of the fluid: drop the water path at once",
        java: "EntityAIFloat.java:106-116",
    },
];

/// `CavalryStrollGoal`'s trigger (`:41-72`).
pub const STROLL_RULES: &[GoalRule] = &[
    GoalRule {
        rule: "can_use",
        when: "server side, no passenger, no reservation, animal data present",
        java: "CavalryStrollGoal.java:41-46",
    },
    GoalRule {
        rule: "stall_inside_stable",
        when: "the horse is in a stable: drift to the next free stall",
        java: "CavalryStrollGoal.java:83-93",
    },
];

/// `ReturnToStableGoal`'s trigger (`:127-159`, `:214-217`).
pub const RETURN_HOME_RULES: &[GoalRule] = &[
    GoalRule {
        rule: "can_use",
        when: "server side, no passenger, no reservation, a stable further than the start distance, and 6000 ticks since the last dismount",
        java: "ReturnToStableGoal.java:127-159",
    },
    GoalRule {
        rule: "can_continue",
        when: "the state machine is not DONE and the horse is still free",
        java: "ReturnToStableGoal.java:214-217",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_rule_table_is_a_gap_free_priority_list() {
        assert!(contiguous(EAT_TABLE));
        assert!(contiguous(SLEEP_TABLE));
        assert!(contiguous(SICK_TABLE));
        assert!(contiguous(MOURN_TABLE));
        assert!(contiguous(WANDER_TABLE));
        assert!(contiguous(FLEE_TABLE));
        assert!(contiguous(COMBAT_TABLE));
        assert!(contiguous(GUARD_TABLE));
        assert!(contiguous(GUARD_ABORT_TABLE));
        assert!(contiguous(TRAINING_TABLE));
        assert!(contiguous(ARCHER_TRAINING_TABLE));
        assert!(contiguous(COMBAT_TRAINING_TABLE));
        assert!(contiguous(STABLE_TABLE));
        assert!(contiguous(VISITOR_TABLE));
    }

    #[test]
    fn the_tables_have_the_number_of_rules_java_registers() {
        // `EntityAIEatTask.java:148-161` — ten `addTransition` calls.
        assert_eq!(EAT_TABLE.len(), 10);
        // `EntityAISleep.java:91-95`.
        assert_eq!(SLEEP_TABLE.len(), 4);
        // `EntityAISickTask.java:124-134`, one duplicate merged.
        assert_eq!(SICK_TABLE.len(), 9);
        // `EntityAIMournCitizen.java:94-104`.
        assert_eq!(MOURN_TABLE.len(), 8);
        // `EntityAICitizenWander.java:82-85`.
        assert_eq!(WANDER_TABLE.len(), 4);
        // `EntityAICitizenChild.java:114-119`.
        assert_eq!(CHILD_TABLE.len(), 5);
        // `EntityAICitizenAvoidEntity.java:105-110`.
        assert_eq!(FLEE_TABLE.len(), 3);
        // `TargetAI.java:45-46` + `AttackMoveAI.java:46-47`.
        assert_eq!(COMBAT_TABLE.len(), 4);
        // `AbstractEntityAIFight.java:73-77` then `AbstractEntityAIGuard.java:168-181`.
        assert_eq!(GUARD_TABLE.len(), 14);
        // `AbstractEntityAITraining.java:54-60`.
        assert_eq!(TRAINING_TABLE.len(), 5);
        // `EntityAIArcherTraining.java:100-106`.
        assert_eq!(ARCHER_TRAINING_TABLE.len(), 4);
        // `EntityAICombatTraining.java:80-87`.
        assert_eq!(COMBAT_TRAINING_TABLE.len(), 6);
        // `ReturnToStableGoal.java:99-108`.
        assert_eq!(STABLE_TABLE.len(), 5);
        // `EntityAIVisitor.java:76-85`.
        assert_eq!(VISITOR_TABLE.len(), 10);
    }

    #[test]
    fn every_rule_carries_a_java_line_with_a_line_number() {
        /// `fichier.java:148` for a single rule, `fichier.java:48-80` when the rule
        /// *is* a method and its body is the evidence: both are a line reference.
        fn looks_like_a_source(text: &str) -> bool {
            if !text.contains(".java:") {
                return false;
            }
            let tail = match text.rsplit(':').next() {
                Some(tail) => tail,
                None => return false,
            };
            tail.split('-').all(|part| part.parse::<u32>().is_ok())
        }
        for rule in EAT_TABLE {
            assert!(looks_like_a_source(rule.java), "{}", rule.rule);
        }
        for rule in GUARD_TABLE {
            assert!(looks_like_a_source(rule.java), "{}", rule.rule);
        }
        for rule in COMBAT_TABLE {
            assert!(looks_like_a_source(rule.java), "{}", rule.rule);
        }
        for rule in VISITOR_TABLE {
            assert!(looks_like_a_source(rule.java), "{}", rule.rule);
        }
        for rule in LOOK_RULES {
            assert!(looks_like_a_source(rule.java), "{}", rule.rule);
        }
        for rule in TOGGLE_RULES {
            assert!(looks_like_a_source(rule.java), "{}", rule.rule);
        }
        for rule in STROLL_RULES {
            assert!(looks_like_a_source(rule.java), "{}", rule.rule);
        }
        for rule in RETURN_HOME_RULES {
            assert!(looks_like_a_source(rule.java), "{}", rule.rule);
        }
    }

    #[test]
    fn the_guard_borrows_its_worker_states_instead_of_redeclaring_them() {
        assert_eq!(GuardGoal::preparing().java_name(), "PREPARING");
        assert_eq!(GuardGoal::start_working().java_name(), "START_WORKING");
        assert_eq!(GuardGoal::decide().java_name(), "DECIDE");
        assert_eq!(GuardGoal::inventory_full().java_name(), "INVENTORY_FULL");
        assert_eq!(GuardGoal::needs_item().java_name(), "NEEDS_ITEM");
        // `AIWorkerState.java:23` — `INVENTORY_FULL(false)` : un garde qui va vider
        // son inventaire ne s'interrompt pas pour manger. `:35` (`NEEDS_ITEM(true)`)
        // et `:27` (`PREPARING(true)`) sont l'inverse.
        assert!(!GuardGoal::inventory_full().okay_to_eat());
        assert!(GuardGoal::needs_item().okay_to_eat());
        assert!(GuardGoal::preparing().okay_to_eat());
        // `AIWorkerState.java:31` — `START_WORKING(true)` : un garde qui rejoint son
        // poste peut encore s'arreter pour manger.
        assert!(GuardGoal::start_working().okay_to_eat());
    }

    #[test]
    fn only_no_target_is_okay_to_eat_among_the_combat_states() {
        // `CombatAIStates.java:10-13`.
        assert!(CombatState::NoTarget.okay_to_eat());
        assert!(!CombatState::Attacking.okay_to_eat());
        assert!(!CombatState::FindMount.okay_to_eat());
        assert!(!CombatState::FindStable.okay_to_eat());
        // `AIWorkerState.java:243` — a fleeing guard may not stop to eat.
        assert!(!GuardGoal::GuardFlee.okay_to_eat());
        assert!(GuardGoal::GuardSleep.okay_to_eat());
    }

    #[test]
    fn the_two_states_a_guard_may_abort_to_fight_are_needs_item_and_inventory_full() {
        // `BehaviourStateGroup.java:16`.
        assert_eq!(GUARD_ABORT_GROUP[0].java_name(), "NEEDS_ITEM");
        assert_eq!(GUARD_ABORT_GROUP[1].java_name(), "INVENTORY_FULL");
        assert_eq!(GUARD_ABORT_TABLE[0].rule, "busy_check_target");
        assert_eq!(GUARD_ABORT_TABLE[1].tick_rate, 80);
    }

    #[test]
    fn the_eating_table_is_one_state_per_rule_and_the_entry_comes_first() {
        assert_eq!(EAT_TABLE[0].goal, EatState::CheckForFood);
        assert_eq!(EAT_TABLE[0].tick_rate, 20);
        // `EntityAIEatTask.java:153` — `new TickingTransition<>(DONE, () -> true,
        // () -> CitizenAIState.IDLE, 1)` : la cible est IDLE, pas `Done`.
        assert_eq!(EAT_TABLE[1].goal, EatState::Idle);
        assert_eq!(EAT_TABLE[1].tick_rate, 1, "DONE is a one-tick state");
        // Neuf sous-etats, dix regles : `Eat` est la cible de deux d'entre elles
        // (`:154` et `:156`), et `Done` d'aucune — c'est l'entree de la regle 2.
        assert_eq!(EatState::ALL.len(), 9);
        assert!(EAT_TABLE.iter().all(|rule| rule.goal != EatState::Done));
    }

    #[test]
    fn a_child_state_is_the_only_citizen_goal_that_answers_okay_to_eat() {
        // `EntityAICitizenChild.java:49-53` is the only `isOkayToEat` override in
        // `entity/ai/minimal/`; the other sub-states implement plain `IState`.
        for state in [
            ChildState::Bored,
            ChildState::Playing,
            ChildState::Visiting,
            ChildState::Following,
        ] {
            assert!(state.okay_to_eat());
        }
    }
}
