//! The citizens: the states they are in, the order those states win in, their
//! needs, and the job that gives them a building.
//!
//! Java reference: `core/entity/ai/workers/CitizenAI.java` (the decision),
//! `api/entity/ai/statemachine/states/CitizenAIState.java` (the states),
//! `api/entity/citizen/VisibleCitizenStatus.java` (the icons),
//! `core/colony/CitizenData.java` + `core/entity/citizen/EntityCitizen.java` (the
//! needs), and `core/colony/jobs/AbstractJob.java` +
//! `core/colony/buildings/modules/WorkerBuildingModule.java` (the job and its hut).
//!
//! | File | Question |
//! | --- | --- |
//! | `state` | what a citizen is doing, what the player sees, which changes are legal |
//! | `needs` | how food, health and sleep move with the tick, and when a state opens |
//! | `priority` | when two states are possible, which one wins and why |
//! | `employment` | what a citizen does and where, and when the two disagree |
//!
//! **Three counts, because Java has three layers of "state" and the brief's list
//! mixes them.**
//!
//! | Layer | Java | Count | Here |
//! | --- | --- | ---: | --- |
//! | what the citizen is doing | `CitizenAIState` | **9** | [`CitizenState`] |
//! | what is drawn over its head | `VisibleCitizenStatus` (central class) | **8** | [`Status`] |
//! | the steps inside a worker's AI | `AIWorkerState` | **126** | not ported — `crate::jobs` |
//!
//! So `BadWeather` is a status and the state is `IDLE` (`CitizenAI.java:232-241`),
//! `InventoryFull` is `AIWorkerState.INVENTORY_FULL`, and `Following` is
//! `EntityAICitizenChild.State.FOLLOWING`. `state`'s note says where each lives.
//!
//! **This is the structure, not the AI.** Java's worker AI is 29 780 lines under
//! `core/entity/ai/workers/` and runs from a per-entity tick; that hook is a global
//! `entity_tick_pre` today, so nothing here ticks itself. What is here is what the
//! AI will drive: a state machine with a legal-transition table, a decision order
//! taken line by line from `calculateNextState`, a needs model parameterised by the
//! tick, and the job↔hut link with its refusals.
//!
//! **Pure data, pure rules**: no host call, no clock, no thread, no file — the same
//! choice `crate::colony`, `crate::inventory` and `crate::jobs` made, and what lets
//! every rule be tested on the host target with no game, no wasm and no prepared
//! resources. The only thing asked of a caller is the tick and the weather, which
//! arrive as values ([`World`], [`Advance`]).
//!
//! **What is not here, and what each gap costs.**
//!
//! - **the tick**: nothing calls [`step`] yet, because a citizen's own tick does
//!   not exist (`crates/horizon-mod-vocab/src/hooks/entity_tick_pre.rs:19` is
//!   global). The machine can be advanced by hand in a test and by nothing else.
//! - **the sub-AIs**: eating is nine steps, being sick eight, mourning seven,
//!   wandering four, the child four, fleeing two — 34 states that fire *inside* a
//!   [`CitizenState`] and need the game (paths, beds, restaurants, hospitals).
//! - **the side effects**: every chat interaction, every `onWakeUp`, every
//!   `setCurrentDelay` in `CitizenAI` is a message or a call on an entity.
//! - **hiring and firing**: `employment` says whether a pair is coherent, not how a
//!   colony's assignment list changes. That list belongs to `crate::colony`.

mod employment;
mod needs;
mod priority;
mod state;

pub use employment::{GUARD_JOBS, Posting, Refusal, Staffing, is_guard};
pub use needs::{
    AVERAGE_SATURATION, Advance, Appetite, FULL_SATURATION, HEAL_CITIZENS_AFTER, LOW_SATURATION,
    MAX_HEALTH, MAX_SATURATION, NIGHT, NOON, Needs, RESTAURANT_LIMIT, SATURATION_DECREASE_AFTER,
    SEEK_DOCTOR_HEALTH, TICKS_PER_DAY, World, consumption_factor, should_go_sleep,
};
pub use priority::{Rung, Situation};
pub use state::{Branch, CitizenState, Decision, IllegalTransition, Machine, Status};

/// One pass of `CitizenAI`: decide from the situation, apply it to the machine.
///
/// This is the seam the AI will sit on. Java calls `decideAiTask` from an event
/// target every ten ticks (`CitizenAI.java:65`) whatever the citizen is doing, and
/// the answer is either "no change" or one of the branch's states; here that is
/// [`Machine::decide`] with the situation's own decision as the pick, and a named
/// refusal when the decision names a transition the machine cannot make.
///
/// ```ignore
/// // What the hook will do, once a per-entity tick exists.
/// situation.world = World::at_day_time(level.day_time);
/// situation.needs = citizen.needs;
/// let _ = citizens::step(&mut citizen.machine, &situation);
/// ```
pub fn step(
    machine: &mut Machine,
    situation: &Situation,
) -> Result<Option<Decision>, IllegalTransition> {
    machine.decide(situation.branch, |current| situation.decide(current.last()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_layers_keep_their_javas_counts() {
        assert_eq!(CitizenState::ALL.len(), 9, "CitizenAIState.java:6-17");
        assert_eq!(Status::ALL.len(), 8, "VisibleCitizenStatus.java:25-41");
        assert_eq!(Rung::ALL.len(), 14, "CitizenAI.java:134-261, one if each");
    }

    #[test]
    fn every_refusal_this_module_raises_is_namespaced_and_distinct() {
        let names = [
            IllegalTransition::NAME,
            Refusal::HutFull {
                hut: crate::building::Kind::Farmer,
                level: 1,
                capacity: 1,
            }
            .name(),
            Refusal::JobWithoutHut {
                job: crate::jobs::Job::Farmer,
            }
            .name(),
        ];
        for (index, name) in names.iter().enumerate() {
            for other in &names[index + 1..] {
                assert_ne!(name, other, "two refusals share a name");
            }
            assert!(name.starts_with("citizens.refusal."), "{name}");
        }
    }

    #[test]
    fn a_pass_of_step_leaves_the_machine_where_the_decision_points() {
        let mut machine = Machine::new();
        let situation = Situation {
            has_job: true,
            ..Situation::default()
        };
        let decided = step(&mut machine, &situation).expect("WORK is reachable from IDLE");
        assert_eq!(
            decided,
            Some(Decision::with(CitizenState::Work, Status::Working))
        );
        assert_eq!(machine.state(), CitizenState::Work);

        // WORK again: `next == lastState` and the state is not IDLE, so
        // `decideAiTask` answers null (CitizenAI.java:115-122).
        assert_eq!(step(&mut machine, &situation), Ok(None));
        assert_eq!(machine.state(), CitizenState::Work);
    }

    #[test]
    fn a_night_falls_and_the_machine_walks_into_bed_on_its_own() {
        let mut machine = Machine::new();
        let at = [0, 64, 0];
        let working = Situation {
            world: World::at_day_time(NOON),
            ..Situation::worker(at)
        };
        assert_eq!(
            step(&mut machine, &working).unwrap().map(|d| d.state),
            Some(CitizenState::Work)
        );
        assert_eq!(
            step(
                &mut machine,
                &Situation {
                    world: World::at_day_time(NIGHT),
                    ..working
                }
            )
            .unwrap()
            .map(|d| d.state),
            Some(CitizenState::Sleep),
            "at NIGHT the walk home is due (CitizenAI.java:178-182)"
        );
        assert_eq!(machine.state(), CitizenState::Sleep);
        // And it stays there: KeepSleeping (:171) is a state the machine holds,
        // which `decideAiTask` reports as no change.
        assert_eq!(
            step(
                &mut machine,
                &Situation {
                    world: World::at_day_time(NIGHT + 200),
                    ..working
                }
            ),
            Ok(None)
        );
    }
}
