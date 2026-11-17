//! Everything in `entity/ai/` that is **not** a trade: the goals a citizen runs as
//! a citizen, the combat layer, the guards, the training academies, the horses and
//! the visitors.
//!
//! Java reference: `com.minecolonies.core.entity.ai`, 93 files. The 36 trades are
//! ported in [`crate::ai_work_01`]..[`crate::ai_work_12`], the base classes in
//! [`crate::ai`] and the builder in [`crate::ai_build`]. This module is the rest.
//!
//! | Module | Java | What it answers |
//! | --- | --- | --- |
//! | [`facts`] | — | what these goals may read, with a [`facts::Tuned`] constant per tunable |
//! | [`goals`] | `AIWorkerState`, `CombatAIStates`, each AI's own enum | every state, its priority table, its trigger, its line |
//! | [`eat`] | `minimal/EntityAIEatTask.java` | nine states: the hut, the restaurant, the seat, the bite |
//! | [`sleep`] | `minimal/EntityAISleep.java` | three states, one bed per citizen, by index |
//! | [`sick`] | `minimal/EntityAISickTask.java` | eight states, a hospital bed, a cure that must be carried |
//! | [`mourn`] | `minimal/EntityAIMournCitizen.java` | seven states, the town hall and the *named* grave |
//! | [`idle`] | `minimal/EntityAICitizenWander.java`, `EntityAICitizenChild.java` | the leisure site and the child's clock |
//! | [`avoid`] | `minimal/EntityAICitizenAvoidEntity.java`, `LookAtEntityGoal.java`, `EntityAIInteractToggleAble.java`, `EntityAIFloat.java` | running away, looking, opening a gate, not drowning |
//! | [`combat`] | `combat/TargetAI.java`, `combat/AttackMoveAI.java`, `combat/CombatUtils.java` | who to hit, how to reach it, when to swing |
//! | [`guard`] | `workers/guard/AbstractEntityAIFight.java`, `AbstractEntityAIGuard.java` | sleeping on duty, fleeing, and the four ways of walking a post |
//! | [`training`] | `workers/guard/training/*` | the archery stand and the combat academy |
//! | [`cavalry`] | `workers/guard/EntityAICavalry.java`, `cavalry/*` | the mount search and the horse going home |
//! | [`visitor`] | `visitor/EntityAIVisitor.java` | the tavern guest who never works and eventually leaves |
//!
//! **Three premises of the brief are wrong** and are corrected rather than
//! worked around (`RAPPORT.md` §1): there is no `EntityAIChaseCitizen.java`, no
//! `EntityAIPlayerAction.java`, and no `entity/ai/targeting/` package. The brief's
//! "construction goals" are the builder's, already in [`crate::ai_build`]; the
//! "raid goals" are the guard's, because a raid reaches a guard through the threat
//! table and [`guard`]. `MANQUANT.md` lists what these goals cannot do.
//!
//! ## The whole priority table, in one place
//!
//! Every table below is a `&[Rule<S>]`: rank, goal, rule name, trigger, tick rate,
//! `fichier.java:ligne`. Where a state has one rule — which is the case of every
//! citizen goal — priority is trivially 1 and the rank records the *registration*
//! order, which is all Java has.
//!
//! | Table | Rules | Machine? |
//! | --- | --- | --- |
//! | [`goals::EAT_TABLE`] | 10 | no — one rule per state, [`eat::step`] |
//! | [`goals::SLEEP_TABLE`] | 4 | no — [`sleep::step`] |
//! | [`goals::SICK_TABLE`] | 9 | no — [`sick::step`] |
//! | [`goals::MOURN_TABLE`] | 8 | no — [`mourn::step`] |
//! | [`goals::WANDER_TABLE`] | 4 | no — [`idle::wander_step`] |
//! | [`goals::CHILD_TABLE`] | 5 | no — [`idle::child_step`], plus the `EVENT` that grows the child up |
//! | [`goals::FLEE_TABLE`] | 3 | no — [`avoid::flee_step`] |
//! | [`goals::COMBAT_TABLE`] | 4 | **yes** — [`combat::machine`] |
//! | [`goals::GUARD_TABLE`] | 14 | **yes** — [`guard::machine`], plus the two abort rules |
//! | [`goals::TRAINING_TABLE`] | 5 | **yes** — [`training::archer_machine`], [`training::combat_machine`] |
//! | [`goals::STABLE_TABLE`] | 5 | **yes** — [`cavalry::stable_machine`] |
//! | [`goals::VISITOR_TABLE`] | 10 | **yes** — [`visitor::machine`] |
//!
//! ## Why some are machines and some are not
//!
//! A state with **one** transition, built with the constant condition `() -> true`,
//! is not a machine: the action *is* the transition, and wrapping it in a `Machine`
//! would only add a tick counter. That is every citizen goal
//! (`EntityAIEatTask.java:148-161` and its siblings). A state with **two** rules —
//! `NO_TARGET` looking for a target at five ticks and at eighty, `GUARD_SLEEP`
//! sleeping and emitting particles, `IDLE` deciding and counting saturation — is a
//! real machine, and priority decides. Those are the six in the right-hand column.
//!
//! **No `mod_api`.** Nothing here imports the SDK: these are decisions and their
//! reasons. What a decision needs to *act* (walk, sit, swing, hurt, place) is named
//! in `MANQUANT.md` with the exact primitive that is missing.
//!
//! **Randomness is a fact.** `getRandom()` appears in `shouldSleep`, `stare`,
//! `searchRestaurant`, `decide`, the gate opener and half a dozen more. Each draw is
//! a field of the facts ([`facts::CitizenFacts::roll`],
//! [`facts::GuardFacts::sleep_roll`], [`facts::ToggleFacts::choice_roll`]…), so
//! every rule below can be replayed and asserted.

pub mod avoid;
pub mod cavalry;
pub mod combat;
pub mod eat;
pub mod facts;
pub mod goals;
pub mod guard;
pub mod idle;
pub mod mourn;
pub mod sick;
pub mod sleep;
pub mod training;
pub mod visitor;

pub use facts::{
    CavalryFacts, ChildFacts, CitizenFacts, CombatFacts, Disease, FleeFacts, FoodFacts, GuardFacts,
    GuardTask, HomeFacts, HospitalFacts, JobKind, LookFacts, MournFacts, PathState, Research,
    RestaurantFacts, Seat, SiteKind, Threat, ToggleFacts, ToggleKind, TogglePos, TrainingFacts,
    Tuned, TunedF, VisitorFacts, WanderFacts,
};
pub use goals::{
    ChildState, CombatState, EatState, FleeState, GoalRule, GuardGoal, MournState, Rule, SickState,
    SleepState, StableState, Step, TrainingGoal, VisitorState, WanderState, COMBAT_TABLE,
    GUARD_ABORT_GROUP, GUARD_ABORT_TABLE, GUARD_TABLE, SICK_TABLE, SLEEP_TABLE, STABLE_TABLE,
    TRAINING_TABLE, VISITOR_TABLE,
};

#[cfg(test)]
mod tests;
