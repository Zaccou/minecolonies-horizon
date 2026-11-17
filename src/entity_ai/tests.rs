//! The checks that span several goals: the tables against the machines, and the two
//! or three chains a single goal cannot show on its own.
//!
//! These are the tests that would catch a drift the per-module tests miss — a table
//! reordered without its machine, a constant that lost its `fichier.java:ligne`, a
//! chain that goes somewhere no state knows how to leave.

use crate::ai::facts::Pos;
use crate::ai::state::WorkerState;
use crate::entity_ai::avoid::{flee_step, flee_table};
use crate::entity_ai::cavalry::{stable_machine, table as stable_table};
use crate::entity_ai::combat::{machine as combat_machine, table as combat_table};
use crate::entity_ai::eat::{step as eat_step, table as eat_table};
use crate::entity_ai::facts::{CitizenFacts, JobKind, Tuned, TunedF};
use crate::entity_ai::goals::{
    contiguous, ChildState, CombatState, EatState, FleeState, GuardGoal, MournState, Rule,
    SickState, SleepState, StableState, Step, TrainingGoal, VisitorState, WanderState,
    ARCHER_TRAINING_TABLE, CHILD_TABLE, COMBAT_TABLE, COMBAT_TRAINING_TABLE, EAT_TABLE, FLEE_TABLE,
    GUARD_ABORT_TABLE, GUARD_TABLE, MOURN_TABLE, SICK_TABLE, SLEEP_TABLE, STABLE_TABLE,
    TRAINING_TABLE, VISITOR_TABLE, WANDER_TABLE,
};
use crate::entity_ai::guard::machine as guard_machine;
use crate::entity_ai::guard::table as guard_table;
use crate::entity_ai::idle::{child_table, wander_table};
use crate::entity_ai::mourn::{step as mourn_step, table as mourn_table};
use crate::entity_ai::sick::{
    go_to_hospital, search_hospital, step as sick_step, table as sick_table, wait_for_cure,
};
use crate::entity_ai::sleep::{step as sleep_step, table as sleep_table};
// Deux « combat machine » coexistent : celle du combat (`combat::machine`, ligne 12)
// et celle de l'academie de combat (`training::combat_machine`). La seconde porte un
// nom distinct pour qu'aucun des deux imports ne se masque.
use crate::entity_ai::training::{archer_machine, combat_machine as combat_training_machine};
use crate::entity_ai::visitor::{machine as visitor_machine, table as visitor_table};

/// The three modules whose machine is the worker's own, and therefore has states
/// Java declares in `AIWorkerState`.
const WORKER_STATES_NOTE: &str = "PREPARING, START_WORKING, NEEDS_ITEM and INVENTORY_FULL \
     belong to crate::ai::state::WorkerState and are borrowed, not redeclared";

#[test]
fn every_table_is_a_gap_free_priority_list() {
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
fn the_module_accessors_return_the_table_they_document() {
    // Une `const` de type `&[T]` n'a **pas** d'adresse stable en Rust : chaque
    // usage peut promouvoir sa propre copie, donc comparer les pointeurs teste un
    // detail d'implementation. On compare le contenu, qui est ce que l'accesseur
    // promet.
    fn names<S: Copy>(table: &[Rule<S>]) -> Vec<&'static str> {
        table.iter().map(|rule| rule.rule).collect()
    }
    fn goal_names(table: &[crate::entity_ai::goals::GoalRule]) -> Vec<&'static str> {
        table.iter().map(|rule| rule.rule).collect()
    }
    assert_eq!(names(eat_table()), names(EAT_TABLE));
    assert_eq!(names(sleep_table()), names(SLEEP_TABLE));
    assert_eq!(names(sick_table()), names(SICK_TABLE));
    assert_eq!(names(mourn_table()), names(MOURN_TABLE));
    assert_eq!(names(wander_table()), names(WANDER_TABLE));
    assert_eq!(goal_names(child_table()), goal_names(CHILD_TABLE));
    assert_eq!(names(flee_table()), names(FLEE_TABLE));
    assert_eq!(names(combat_table()), names(COMBAT_TABLE));
    assert_eq!(names(guard_table()), names(GUARD_TABLE));
    assert_eq!(names(stable_table()), names(STABLE_TABLE));
    assert_eq!(names(visitor_table()), names(VISITOR_TABLE));
    let _ = WORKER_STATES_NOTE;
}

#[test]
fn each_machine_registers_its_rules_in_its_table_order() {
    fn names<S: Copy>(table: &[Rule<S>], of: impl Fn(&Rule<S>) -> &'static str) -> Vec<&'static str> {
        table.iter().map(of).collect()
    }

    let combat = combat_machine();
    assert_eq!(
        names(COMBAT_TABLE, |rule| rule.rule),
        combat
            .transitions()
            .iter()
            .map(|transition| transition.rule)
            .collect::<Vec<_>>()
    );

    let visitor = visitor_machine();
    assert_eq!(
        names(VISITOR_TABLE, |rule| rule.rule),
        visitor
            .transitions()
            .iter()
            .map(|transition| transition.rule)
            .collect::<Vec<_>>()
    );

    let stable = stable_machine();
    assert_eq!(
        names(STABLE_TABLE, |rule| rule.rule),
        stable
            .transitions()
            .iter()
            .map(|transition| transition.rule)
            .collect::<Vec<_>>()
    );

    let archer = archer_machine();
    let expected: Vec<&'static str> = TRAINING_TABLE
        .iter()
        .chain(ARCHER_TRAINING_TABLE.iter())
        .map(|rule| rule.rule)
        .collect();
    assert_eq!(
        expected,
        archer
            .transitions()
            .iter()
            .map(|transition| transition.rule)
            .collect::<Vec<_>>()
    );

    let knight = combat_training_machine();
    let expected: Vec<&'static str> = TRAINING_TABLE
        .iter()
        .chain(COMBAT_TRAINING_TABLE.iter())
        .map(|rule| rule.rule)
        .collect();
    assert_eq!(
        expected,
        knight
            .transitions()
            .iter()
            .map(|transition| transition.rule)
            .collect::<Vec<_>>()
    );

    let guard = guard_machine();
    let mut expected: Vec<&'static str> = GUARD_TABLE.iter().map(|rule| rule.rule).collect();
    expected.extend(GUARD_ABORT_TABLE.iter().map(|rule| rule.rule));
    assert_eq!(
        expected,
        guard
            .transitions()
            .iter()
            .map(|transition| transition.rule)
            .collect::<Vec<_>>()
    );
}

#[test]
fn every_rule_of_every_table_names_a_java_line() {
    fn check<S: Copy>(table: &[Rule<S>], label: &str) {
        for rule in table {
            assert!(
                rule.java.contains(".java:"),
                "{label}: {} has no source",
                rule.rule
            );
            let line = rule.java.rsplit(':').next().unwrap_or("");
            assert!(
                line.parse::<u32>().is_ok(),
                "{label}: {} ends on {line}, not a line number",
                rule.rule
            );
            assert!(
                !rule.when.is_empty(),
                "{label}: {} has no trigger",
                rule.rule
            );
            assert!(
                rule.tick_rate > 0,
                "{label}: {} has a zero tick rate",
                rule.rule
            );
        }
    }
    check(EAT_TABLE, "eat");
    check(SLEEP_TABLE, "sleep");
    check(SICK_TABLE, "sick");
    check(MOURN_TABLE, "mourn");
    check(WANDER_TABLE, "wander");
    check(FLEE_TABLE, "flee");
    check(COMBAT_TABLE, "combat");
    check(GUARD_TABLE, "guard");
    check(GUARD_ABORT_TABLE, "guard abort");
    check(TRAINING_TABLE, "training");
    check(ARCHER_TRAINING_TABLE, "archer");
    check(COMBAT_TRAINING_TABLE, "combat training");
    check(STABLE_TABLE, "stable");
    check(VISITOR_TABLE, "visitor");
}

#[test]
fn every_constant_carries_the_line_it_was_read_from() {
    let constants: &[Tuned] = &[
        crate::entity_ai::facts::MAX_SATURATION,
        crate::entity_ai::facts::AVERAGE_SATURATION,
        crate::entity_ai::facts::SEEK_DOCTOR_HEALTH,
        crate::entity_ai::facts::RANGE_TO_BE_HOME,
        crate::entity_ai::facts::MAX_GUARD_CALL_RANGE,
        crate::entity_ai::eat::REQUIRED_TIME_TO_EAT,
        crate::entity_ai::eat::MINUTES_WAITING_TIME,
        crate::entity_ai::sleep::MAX_BED_TICKS,
        crate::entity_ai::sleep::TICK_INTERVAL,
        crate::entity_ai::sick::REQUIRED_TIME_TO_CURE,
        crate::entity_ai::sick::GOING_TO_BED_ATTEMPTS,
        crate::entity_ai::mourn::AVERAGE_STARE_TIME,
        crate::entity_ai::idle::MIN_ACTIVE_TIME,
        crate::entity_ai::idle::BONUS_TIME_COLONY,
        crate::entity_ai::avoid::CHECKS_BEFORE_SAFE,
        crate::entity_ai::avoid::TOGGLE_MAX_DISTANCE,
        crate::entity_ai::combat::STOP_PERSECUTION_AFTER,
        crate::entity_ai::combat::MIN_THREAT_TO_CHASE,
        crate::entity_ai::guard::COMBAT_TIME,
        crate::entity_ai::guard::KNIGHT_DERIVATION_BONUS,
        crate::entity_ai::training::TARGET_SEARCH_CHANCE,
        crate::entity_ai::cavalry::LINGER_AFTER_DISMOUNT,
        crate::entity_ai::visitor::COMBAT_TIMEOUT,
    ];
    for constant in constants {
        assert!(
            constant.source.contains(".java:"),
            "a Tuned constant lost its source: {constant:?}"
        );
        assert!(constant.value > 0, "a Tuned constant is not positive");
    }
    let floats: &[TunedF] = &[
        crate::entity_ai::eat::RESTAURANT_LIMIT,
        crate::entity_ai::eat::FOOD_HANDOUT_EXTRA,
    ];
    for constant in floats {
        assert!(constant.source.contains(".java:"));
    }
}

#[test]
fn the_citizen_chains_start_at_the_state_the_citizen_machine_enters() {
    // Every citizen goal's table begins with its entry state (`EATING`,
    // `SLEEP`, `SICK`, `MOURN`, `IDLE`, `FLEE`), and that state leads into the
    // goal's own first substate.
    assert_eq!(eat_step(EatState::CitizenEating, &mut Default::default()), Step::Next(EatState::CheckForFood));
    assert_eq!(
        sleep_step(SleepState::CitizenSleep, &mut Default::default()),
        Step::Next(SleepState::WalkingHome)
    );
    assert_eq!(
        mourn_step(MournState::CitizenMourn, &mut Default::default()),
        Step::Next(MournState::Decide)
    );
    let mut fleeing = Default::default();
    assert_eq!(
        flee_step(FleeState::CitizenFlee, &mut fleeing),
        Step::Next(FleeState::CheckEntities)
    );
    // A citizen who is neither sick nor hurt does not enter the sick chain at all.
    assert_eq!(
        sick_step(SickState::CitizenSick, &mut Default::default()),
        Step::Stayed
    );
}

#[test]
fn a_hungry_citizen_walks_the_whole_eating_chain_to_a_bite() {
    // `EntityAIEatTask.java:148-161`: hut, restaurant, seat, bite, idle.
    let mut scene = crate::entity_ai::eat::Scene::default();
    scene.citizen.saturation = 2.0;

    // No food in the pocket and no hut: search a restaurant.
    assert_eq!(eat_step(EatState::CheckForFood, &mut scene), Step::Next(EatState::GoToHut));
    assert_eq!(
        eat_step(EatState::GoToHut, &mut scene),
        Step::Next(EatState::SearchRestaurant)
    );
    // No restaurant and a starving citizen: a blocking complaint, then look again.
    assert_eq!(
        eat_step(EatState::SearchRestaurant, &mut scene),
        Step::Next(EatState::CheckForFood)
    );
    assert!(scene.interaction.is_some());

    // A restaurant appears: go there, sit, eat.
    scene.restaurant.pos = Some(Pos::new(9, 64, 0));
    assert_eq!(
        eat_step(EatState::SearchRestaurant, &mut scene),
        Step::Next(EatState::GoToRestaurant)
    );
    scene.restaurant.we_are_inside = true;
    scene.restaurant.sitting_pos = Some(Pos::new(10, 64, 0));
    assert_eq!(
        eat_step(EatState::WaitForFood, &mut scene),
        Step::Next(EatState::GoToEatPos)
    );
    scene.food.best_slot = Some(0);
    scene.citizen.saturation = 60.0;
    for _ in 0..4 {
        let _ = eat_step(EatState::Eat, &mut scene);
    }
    assert_eq!(eat_step(EatState::Eat, &mut scene), Step::Next(EatState::Idle));
    assert!(scene.memory.just_ate);
}

#[test]
fn a_sick_citizen_without_a_hospital_wanders_and_may_cure_itself() {
    // `EntityAISickTask.java:419-429` then `:334-338`.
    let mut scene = crate::entity_ai::sick::Scene::default();
    scene.citizen.sick = true;
    scene.citizen.health = 3.0;
    scene.disease = Some(crate::entity_ai::facts::Disease::new(
        "flu",
        &["minecraft:honey_bottle"],
    ));
    assert_eq!(
        search_hospital(&mut scene),
        Step::Next(SickState::Wander)
    );
    assert_eq!(
        scene.interaction,
        Some("com.minecolonies.coremod.gui.chat.nohospital")
    );
    // No hospital: goToHospital sends the citizen back to the search.
    assert_eq!(
        go_to_hospital(&mut scene),
        Step::Next(SickState::SearchHospital)
    );
    // The house call: one check in seven thousand two hundred.
    scene.hospital.cure_present = vec![false];
    scene.citizen.asleep = true;
    scene.self_cure_roll = 0;
    scene.hospital.pos = Some(Pos::new(1, 64, 1));
    assert_eq!(
        wait_for_cure(&mut scene),
        Step::Next(SickState::Idle)
    );
    assert!(scene.cured);
}

#[test]
fn a_guard_asleep_at_its_post_wakes_on_the_first_blow() {
    // `:268-290` then `:314-332`.
    let mut scene = crate::entity_ai::guard::Scene::with_hut(Pos::new(0, 64, 0));
    scene.facts.adaptability = 2;
    scene.facts.sleep_roll = 1;
    scene.facts.sleep_chance = 0.0;
    assert!(crate::entity_ai::guard::should_sleep(&mut scene));
    assert_eq!(scene.memory.sleep_timer, 2500);
    assert!(scene.sat_down);

    assert_eq!(
        crate::entity_ai::guard::sleep_on_duty(&mut scene),
        None,
        "one tick asleep"
    );
    scene.facts.last_hurt_by_mob = true;
    assert_eq!(
        crate::entity_ai::guard::sleep_on_duty(&mut scene),
        Some(GuardGoal::NoTarget)
    );
    assert!(scene.stood_up);
}

#[test]
fn a_knight_and_a_ranger_disagree_about_reach_at_the_same_hut() {
    // `MeleeCombatAI.java:356-358` versus `RangeCombatAI.java:230-258`.
    let melee = crate::entity_ai::combat::MAX_DISTANCE_FOR_ATTACK.v() as f64;
    let ranged = crate::entity_ai::combat::range_attack_distance(5, 20, 0.0, true);
    assert_eq!(melee, 2.0);
    assert_eq!(
        ranged, 31.0,
        "10 + 5 + 6 = 21 under the cap of 24, then the guard bonus of 10"
    );
    assert!(ranged > 10.0 * melee);
}

#[test]
fn the_two_guard_skills_are_not_interchangeable() {
    // `MeleeCombatAI.java:382` ajoute `Adaptability * 0.01 + (niveau - 1) * 0.01`,
    // `RangeCombatAI.java:375` ajoute `Agility * 0.01 + (niveau * 2 - 1) * 0.01`.
    // A skill egal et sous le plafond, c'est le terme de batiment qui differe.
    let melee = crate::entity_ai::combat::melee_movement_speed(4, 3);
    let ranged = crate::entity_ai::combat::range_movement_speed(4, 3);
    assert!((melee - 1.06).abs() < 1e-9, "0.04 d'agilite + 0.02 de batiment");
    assert!((ranged - 1.09).abs() < 1e-9, "0.04 d'agilite + 0.05 de batiment");
    // Le plafond de `+0.3` s'applique des deux cotes (`:378`, `:385`) : a haut
    // niveau les deux formules se rejoignent, ce n'est donc pas la peine d'y
    // chercher une difference.
    assert_eq!(crate::entity_ai::combat::melee_movement_speed(99, 9), 1.3);
    assert_eq!(crate::entity_ai::combat::range_movement_speed(99, 9), 1.3);
}

#[test]
fn a_visitor_lasts_hours_and_then_leaves() {
    // `EntityAIVisitor.java:93-107`.
    let mut scene = crate::entity_ai::visitor::Scene::default();
    scene.facts.saturation = 1.0;
    scene.facts.loaded = true;
    let mut checks = 0;
    while !crate::entity_ai::visitor::reduce_time(&mut scene) {
        checks += 1;
        assert!(checks < 100, "fifty checks should empty one point");
    }
    assert!(
        (49..=51).contains(&checks),
        "0.02 per check: one point needs fifty checks, not {checks}"
    );
    assert!(scene.left);
}

#[test]
fn a_gate_is_opened_once_and_remembered_by_kind() {
    // `EntityAIInteractToggleAble.java:446-461`.
    use crate::entity_ai::facts::{ToggleKind, TogglePos};
    let mut memory = crate::entity_ai::avoid::ToggleMemory::default();
    let facts = crate::entity_ai::facts::ToggleFacts {
        positions: vec![TogglePos {
            pos: Pos::new(1, 64, 0),
            kind: ToggleKind::Fence,
            has_open: true,
            is_air: false,
            open: false,
            distance_sqr: 2.0,
        }],
        choice_roll: 0,
        ..Default::default()
    };
    let toggled = crate::entity_ai::avoid::toggle_tick(&facts, &mut memory);
    assert_eq!(toggled.kept.len(), 1);
    assert_eq!(toggled.open, Some(Pos::new(1, 64, 0)));
    assert!(toggled.close.is_empty());
    assert_eq!(memory.my_toggled, vec![ToggleKind::Fence]);
}

#[test]
fn the_three_states_a_guard_borrows_are_the_workers_own() {
    // `AbstractEntityAIGuard` extends `AbstractEntityAIFight` extends
    // `AbstractEntityAIInteract`: same machine, same states.
    assert_eq!(GuardGoal::preparing().java_name(), WorkerState::Preparing.java_name());
    assert_eq!(
        GuardGoal::start_working().java_name(),
        WorkerState::StartWorking.java_name()
    );
    assert_eq!(GuardGoal::decide().java_name(), WorkerState::Decide.java_name());
    assert_eq!(
        GuardGoal::inventory_full().java_name(),
        WorkerState::InventoryFull.java_name()
    );
    assert_eq!(
        GuardGoal::needs_item().java_name(),
        WorkerState::NeedsItem.java_name()
    );
}

#[test]
fn a_step_that_stays_put_is_none_and_a_step_that_moves_is_some() {
    assert_eq!(Step::<EatState>::Stayed.next(), None);
    assert_eq!(
        Step::Next(EatState::Eat).next(),
        Some(EatState::Eat)
    );
    // The same shape is reused by every citizen goal, which is why it is generic.
    assert_eq!(Step::<SickState>::Stayed.next(), None);
    assert_eq!(Step::Next(FleeState::Running).next(), Some(FleeState::Running));
}

#[test]
fn the_state_names_are_the_java_ones_so_a_log_line_can_be_compared() {
    assert_eq!(EatState::CheckForFood.java_name(), "CHECK_FOR_FOOD");
    assert_eq!(EatState::GoToEatPos.java_name(), "GO_TO_EAT_POS");
    assert_eq!(SleepState::WalkingHome.java_name(), "WALKING_HOME");
    assert_eq!(SickState::FindEmptyBed.java_name(), "FIND_EMPTY_BED");
    assert_eq!(MournState::WanderAtGraveyard.java_name(), "WANDER_AT_GRAVEYARD");
    assert_eq!(WanderState::ReadABook.java_name(), "READ_A_BOOK");
    assert_eq!(ChildState::Following.java_name(), "FOLLOWING");
    assert_eq!(FleeState::CheckEntities.java_name(), "CHECK_ENTITIES");
    assert_eq!(CombatState::NoTarget.java_name(), "NO_TARGET");
    assert_eq!(GuardGoal::GuardSleep.java_name(), "GUARD_SLEEP");
    assert_eq!(TrainingGoal::KnightAttackProtect.java_name(), "KNIGHT_ATTACK_PROTECT");
    assert_eq!(StableState::WalkToStall.java_name(), "WALK_TO_STALL");
    assert_eq!(VisitorState::Wandering.java_name(), "WANDERING");
}

#[test]
fn a_guard_crafted_eating_branch_only_concerns_a_real_guard_job() {
    // `EntityAIEatTask.java:296-303`: the night branch is `instanceof AbstractJobGuard`.
    let guard = CitizenFacts {
        job: JobKind::Guard,
        ..CitizenFacts::default()
    };
    let worker = CitizenFacts::default();
    assert!(guard.job.is_guard());
    assert!(!worker.job.is_guard());
    assert!(JobKind::Cavalry.is_guard());
    assert!(JobKind::Druid.is_guard());
}

#[test]
fn no_table_mentions_a_state_that_does_not_belong_to_its_machine() {
    // The two enums that share a machine (`TrainingGoal` with `WorkerState`) are the
    // only place a borrowed state may appear.
    for rule in GUARD_TABLE {
        match rule.goal {
            GuardGoal::Worker(_) => {}
            GuardGoal::NoTarget
            | GuardGoal::Attacking
            | GuardGoal::GuardSleep
            | GuardGoal::GuardWake
            | GuardGoal::GuardRegen
            | GuardGoal::GuardFlee
            | GuardGoal::FindMount
            | GuardGoal::FindStable => {}
        }
    }
    for rule in TRAINING_TABLE {
        match rule.goal {
            TrainingGoal::Worker(_) => {}
            TrainingGoal::TrainingWander
            | TrainingGoal::GoToTarget
            | TrainingGoal::CombatTraining
            | TrainingGoal::ArcherSelectTarget
            | TrainingGoal::ArcherCheckShot
            | TrainingGoal::ArcherShoot
            | TrainingGoal::FindTrainingPartner
            | TrainingGoal::KnightTrainWithPartner
            | TrainingGoal::FindDummyPartner
            | TrainingGoal::KnightAttackDummy
            | TrainingGoal::KnightAttackProtect => {}
        }
    }
}
