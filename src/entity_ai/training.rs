//! The two training academies: an archer shooting at a stand, a knight hitting a
//! dummy or a colleague.
//!
//! Java references: `workers/guard/training/AbstractEntityAITraining.java`,
//! `EntityAIArcherTraining.java`, `EntityAICombatTraining.java`.
//!
//! **Training does no damage.**
//! `trainingPartner.hurt(..., DamageSourceKeys.TRAINING, 0.0F)`
//! (`EntityAICombatTraining.java:213`) is a zero-damage hit whose only purpose is
//! the experience and the animation — the guard's skill grows, the colleague loses
//! nothing. The port keeps the rule and says so, because a reader who sees `hurt`
//! will assume damage.
//!
//! **The counters are per building level.** An archer fires at most
//! `level * 5` arrows per visit to a stand (`:117`), and a knight throws at most
//! `level * 5` blows at a dummy before going back to deciding (`:244`) — but the
//! partner training uses `> level * 5` (strictly) while the dummy uses
//! `>= level * 5`, so the two counters disagree by exactly one blow
//! (`:219` versus `:244`).
//!
//! **A missing shield does not stop a knight.** `isSetup()` requires a sword **and**
//! a shield (`:335-351`), but the shield is only *used* on a coin flip (`:198`), so
//! the requirement is about having it, not about raising it.

use crate::ai::facts::Pos;
use crate::ai::state::{Action, Machine, Transition, WorkerState, ENTITY_AI_TICKRATE};
use super::combat::{range_hit_chance};
use super::facts::{TrainingFacts, Tuned};
use super::goals::{
    Rule, TrainingGoal, ARCHER_TRAINING_TABLE, COMBAT_TRAINING_TABLE, TRAINING_TABLE,
};

/// `TARGET_SEARCH_CHANCE` (`AbstractEntityAITraining.java:23`): thirty in a hundred.
pub const TARGET_SEARCH_CHANCE: Tuned =
    Tuned::new(30, "workers/guard/training/AbstractEntityAITraining.java:23");
/// `ONE_HUNDRED_PERCENT` (`:28`).
pub const ONE_HUNDRED_PERCENT: Tuned =
    Tuned::new(100, "workers/guard/training/AbstractEntityAITraining.java:28");
/// The wander range inside the hut's corners (`:97`).
pub const TRAINING_WANDER_RANGE: Tuned =
    Tuned::new(20, "workers/guard/training/AbstractEntityAITraining.java:97");
/// `XP_PER_SUCCESSFUL_SHOT` (`EntityAIArcherTraining.java:43`).
pub const XP_PER_SUCCESSFUL_SHOT: Tuned =
    Tuned::new(1, "workers/guard/training/EntityAIArcherTraining.java:43");
/// `BUILDING_LEVEL_TARGET_MULTIPLIER` (`:48`).
pub const BUILDING_LEVEL_TARGET_MULTIPLIER: Tuned =
    Tuned::new(5, "workers/guard/training/EntityAIArcherTraining.java:48");
/// `MIN_DISTANCE_FOR_SUCCESS` (`:53`).
pub const MIN_DISTANCE_FOR_SUCCESS: Tuned =
    Tuned::new(2, "workers/guard/training/EntityAIArcherTraining.java:53");
/// The archer's own reload (`:58`).
pub const ARCHER_RELOAD: Tuned =
    Tuned::new(10, "workers/guard/training/EntityAIArcherTraining.java:58");
/// `XP_BASE_RATE` of the archer (`:63`).
pub const ARCHER_XP_BASE: f64 = 0.2;
/// `CHECK_SHOT_DELAY` (`:68`): three seconds before the arrow is judged.
pub const CHECK_SHOT_DELAY: Tuned =
    Tuned::new(60, "workers/guard/training/EntityAIArcherTraining.java:68");
/// `ACTIONS_PER_BUILDING_LEVEL` (`EntityAICombatTraining.java:34`).
pub const ACTIONS_PER_BUILDING_LEVEL: Tuned =
    Tuned::new(5, "workers/guard/training/EntityAICombatTraining.java:34");
/// `XP_BASE_RATE` of the knight academy (`:39`) — five times the archer's.
pub const COMBAT_XP_BASE: f64 = 2.0;
/// `PARTNER_TRAINING_CHANCE` (`:44`).
pub const PARTNER_TRAINING_CHANCE: Tuned =
    Tuned::new(25, "workers/guard/training/EntityAICombatTraining.java:44");
/// `TRAININGS_DELAY` (`:49`): `TICKS_20 * 3`.
pub const TRAININGS_DELAY: Tuned =
    Tuned::new(60, "workers/guard/training/EntityAICombatTraining.java:49");
/// `MIN_DISTANCE_TO_TRAIN` (`:54`).
pub const MIN_DISTANCE_TO_TRAIN: Tuned =
    Tuned::new(5, "workers/guard/training/EntityAICombatTraining.java:54");
/// The knight academy's own reload (`:225`, `:321`).
pub const COMBAT_RELOAD: Tuned =
    Tuned::new(10, "workers/guard/training/EntityAICombatTraining.java:225");

/// The trainee's own fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Memory {
    /// `currentPathingTarget` (`AbstractEntityAITraining.java:33`).
    pub current_pathing_target: Option<Pos>,
    /// `stateAfterPathing` (`:38`).
    pub state_after_pathing: Option<TrainingGoal>,
    /// `currentAttackDelay` (`:43`).
    pub attack_delay: i32,
    /// `targetCounter` (`EntityAIArcherTraining.java:84`).
    pub target_counter: i32,
    /// `currentShootingTarget` (`:79`).
    pub shooting_target: Option<Pos>,
    /// `currentCombatTarget` (`EntityAICombatTraining.java:59`).
    pub combat_target: Option<Pos>,
    /// `trainingPartner` (`:64`).
    pub partner: Option<Pos>,
}

/// The facts a trainee reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    /// The academy and the trainee.
    pub facts: TrainingFacts,
    /// What it remembers.
    pub memory: Memory,
    /// The experience earned this tick (`addExperience`).
    pub experience: f64,
    /// `worker.startUsingItem(MAIN_HAND)` — the bow drawn or the shield raised.
    pub using_item: bool,
    /// `building.resetPartner(worker)` (`EntityAICombatTraining.java:221`).
    pub partner_reset: bool,
    /// `walkToRandomPosWithin(...)` was asked for (`AbstractEntityAITraining.java:97`).
    pub wandered: bool,
}

/// `decide` (`AbstractEntityAITraining.java:69-81`): thirty in a hundred trains at
/// combat, the rest walks.
pub fn decide(scene: &mut Scene) -> Option<TrainingGoal> {
    if !scene.facts.setup {
        // `:71-74` — `return DECIDE`: a self-transition, so the tick stops here.
        return Some(TrainingGoal::Worker(WorkerState::Decide));
    }
    if scene.facts.roll.rem_euclid(ONE_HUNDRED_PERCENT.v() as i32)
        < TARGET_SEARCH_CHANCE.v() as i32
    {
        return Some(TrainingGoal::CombatTraining);
    }
    Some(TrainingGoal::TrainingWander)
}

/// `wander` (`:95-103`).
pub fn wander(scene: &mut Scene) -> Option<TrainingGoal> {
    scene.wandered = true;
    Some(TrainingGoal::Worker(WorkerState::Decide))
}

/// `pathToTarget` (`:110-117`): the walk ends in whatever the caller planned.
pub fn path_to_target(scene: &mut Scene) -> Option<TrainingGoal> {
    let target = scene.memory.current_pathing_target?;
    let _ = target;
    scene
        .memory
        .state_after_pathing
        .or(Some(TrainingGoal::Worker(WorkerState::Decide)))
}

/// `reduceAttackDelay` (`:122-128`).
pub fn reduce_attack_delay(scene: &mut Scene) {
    if scene.memory.attack_delay > 0 {
        scene.memory.attack_delay -= 1;
    }
}

/// `EntityAIArcherTraining.findShootingStandPosition` (`:140-153`).
pub fn find_shooting_stand(scene: &mut Scene) -> Option<TrainingGoal> {
    // `EntityAIArcherTraining.java:145-148` — `if (shootingPos == null) return
    // DECIDE;`, donc un etat, pas « rien ».
    let stand = match scene.facts.shooting_stand {
        Some(stand) => stand,
        None => return Some(TrainingGoal::Worker(WorkerState::Decide)),
    };
    scene.memory.current_pathing_target = Some(stand);
    scene.memory.state_after_pathing = Some(TrainingGoal::ArcherSelectTarget);
    Some(TrainingGoal::GoToTarget)
}

/// `EntityAIArcherTraining.selectTarget` (`:114-133`).
///
/// Three ways to give up: the counter reached `level * 5`, the hut gave no target,
/// or a ray from the eye does not reach the target's own block.
pub fn select_target(scene: &mut Scene) -> Option<TrainingGoal> {
    let cap = scene.facts.building_level * BUILDING_LEVEL_TARGET_MULTIPLIER.v() as i32;
    if scene.memory.target_counter >= cap {
        scene.memory.target_counter = 0;
        return Some(TrainingGoal::Worker(WorkerState::Decide));
    }
    let target = match scene.facts.shooting_target {
        Some(target) => target,
        None => return Some(TrainingGoal::Worker(WorkerState::Decide)),
    };
    if !scene.facts.line_clear {
        return Some(TrainingGoal::Worker(WorkerState::Decide));
    }
    scene.memory.shooting_target = Some(target);
    scene.memory.target_counter += 1;
    Some(TrainingGoal::ArcherShoot)
}

/// `EntityAIArcherTraining.shoot` (`:160-214`).
pub fn shoot(scene: &mut Scene) -> Option<TrainingGoal> {
    if scene.memory.shooting_target.is_none() || !scene.facts.setup {
        return Some(TrainingGoal::Worker(WorkerState::StartWorking));
    }
    if !scene.using_item {
        reduce_attack_delay(scene);
        if scene.memory.attack_delay <= 0 {
            scene.using_item = true;
        }
        return Some(TrainingGoal::ArcherShoot);
    }
    scene.experience += ARCHER_XP_BASE;
    scene.memory.attack_delay = ARCHER_RELOAD.v() as i32;
    scene.using_item = false;
    Some(TrainingGoal::ArcherCheckShot)
}

/// `EntityAIArcherTraining.checkShot` (`:216-243`).
pub fn check_shot(scene: &mut Scene) -> Option<TrainingGoal> {
    if scene.facts.arrow_distance_sqr < MIN_DISTANCE_FOR_SUCCESS.v() as f64 {
        scene.experience += XP_PER_SUCCESSFUL_SHOT.v() as f64;
    } else {
        scene.experience += ARCHER_XP_BASE;
    }
    Some(TrainingGoal::ArcherSelectTarget)
}

/// `EntityAIArcherTraining.isSetup`'s shooting chance (`:182`):
/// `HIT_CHANCE_DIVIDER / (primarySkill / 2 + 1)`.
pub fn archer_hit_chance(primary_skill: i32) -> f32 {
    range_hit_chance(primary_skill / 2)
}

/// `EntityAICombatTraining.decideOnTrainingType` (`:95-102`).
pub fn decide_on_training_type(scene: &mut Scene) -> Option<TrainingGoal> {
    if scene.facts.has_partner
        || scene.facts.partner_roll.rem_euclid(ONE_HUNDRED_PERCENT.v() as i32)
            < PARTNER_TRAINING_CHANCE.v() as i32
    {
        return Some(TrainingGoal::FindTrainingPartner);
    }
    Some(TrainingGoal::FindDummyPartner)
}

/// `EntityAICombatTraining.findTrainingPartner` (`:119-136`).
pub fn find_training_partner(scene: &mut Scene) -> Option<TrainingGoal> {
    if !scene.facts.has_partner {
        return Some(TrainingGoal::CombatTraining);
    }
    match scene.facts.partner {
        Some(partner) => {
            scene.memory.partner = Some(partner);
            Some(TrainingGoal::KnightTrainWithPartner)
        }
        None => Some(TrainingGoal::CombatTraining),
    }
}

/// `EntityAICombatTraining.trainWithPartner` (`:143-157`).
pub fn train_with_partner(scene: &mut Scene) -> Option<TrainingGoal> {
    if !scene.facts.partner_assigned {
        scene.memory.partner = None;
        return Some(TrainingGoal::CombatTraining);
    }
    if scene.facts.partner_distance > MIN_DISTANCE_TO_TRAIN.v() as f64 {
        return Some(TrainingGoal::KnightTrainWithPartner);
    }
    Some(TrainingGoal::KnightAttackProtect)
}

/// `EntityAICombatTraining.attack` (`:164-234`): the blow, the shield, the counter.
pub fn attack_partner(scene: &mut Scene) -> Option<TrainingGoal> {
    if scene.memory.partner.is_none() {
        return Some(TrainingGoal::Worker(WorkerState::StartWorking));
    }
    if scene.facts.partner_distance > MIN_DISTANCE_TO_TRAIN.v() as f64 {
        scene.memory.current_pathing_target = scene.facts.partner;
        scene.memory.state_after_pathing = Some(TrainingGoal::KnightTrainWithPartner);
        return Some(TrainingGoal::GoToTarget);
    }
    if scene.memory.attack_delay > 0 {
        reduce_attack_delay(scene);
        return Some(TrainingGoal::KnightAttackProtect);
    }
    scene.experience += COMBAT_XP_BASE;
    // `nextBoolean()`: shield up, or a zero-damage swing (`:198-215`).
    scene.using_item = scene.facts.coin;
    if !scene.facts.coin {
        scene.memory.partner = scene.facts.partner;
    }
    scene.memory.target_counter += 1;
    let cap = scene.facts.building_level * ACTIONS_PER_BUILDING_LEVEL.v() as i32;
    if scene.memory.target_counter > cap {
        scene.partner_reset = true;
        scene.memory.target_counter = 0;
        return Some(TrainingGoal::Worker(WorkerState::StartWorking));
    }
    scene.memory.attack_delay = COMBAT_RELOAD.v() as i32;
    Some(TrainingGoal::KnightAttackProtect)
}

/// `EntityAICombatTraining.findDummyPartner` (`:241-269`).
pub fn find_dummy_partner(scene: &mut Scene) -> Option<TrainingGoal> {
    let cap = scene.facts.building_level * ACTIONS_PER_BUILDING_LEVEL.v() as i32;
    if scene.memory.target_counter >= cap {
        scene.memory.target_counter = 0;
        return Some(TrainingGoal::Worker(WorkerState::Decide));
    }
    if scene.facts.has_partner {
        return Some(TrainingGoal::KnightTrainWithPartner);
    }
    let dummy = match scene.facts.dummy {
        Some(dummy) => dummy,
        None => return Some(TrainingGoal::Worker(WorkerState::Decide)),
    };
    scene.memory.combat_target = Some(dummy);
    scene.memory.target_counter += 1;
    scene.memory.current_pathing_target = Some(dummy);
    scene.memory.state_after_pathing = Some(TrainingGoal::KnightAttackDummy);
    Some(TrainingGoal::GoToTarget)
}

/// `EntityAICombatTraining.attackDummy` (`:276-330`).
pub fn attack_dummy(scene: &mut Scene) -> Option<TrainingGoal> {
    if scene.memory.combat_target.is_none() {
        return Some(TrainingGoal::Worker(WorkerState::StartWorking));
    }
    if scene.memory.attack_delay > 0 {
        reduce_attack_delay(scene);
        return Some(TrainingGoal::KnightAttackDummy);
    }
    scene.experience += COMBAT_XP_BASE;
    scene.using_item = scene.facts.coin;
    scene.memory.attack_delay = COMBAT_RELOAD.v() as i32;
    Some(TrainingGoal::FindDummyPartner)
}

fn base_rules(machine: &mut Machine<TrainingGoal, Scene>) {
    machine.add(Transition::goto(
        TrainingGoal::Worker(WorkerState::Idle),
        TrainingGoal::Worker(WorkerState::StartWorking),
        "idle",
        "AbstractEntityAITraining.java:55",
        1,
    ));
    machine.add(Transition::goto(
        TrainingGoal::Worker(WorkerState::StartWorking),
        TrainingGoal::Worker(WorkerState::Decide),
        "start_working",
        "AbstractEntityAITraining.java:56",
        1,
    ));
    machine.add(Transition::target(
        TrainingGoal::Worker(WorkerState::Decide),
        "decide",
        "AbstractEntityAITraining.java:57",
        20,
        Action::Run(|scene: &mut Scene, _: TrainingGoal| decide(scene)),
    ));
    machine.add(Transition::target(
        TrainingGoal::TrainingWander,
        "wander",
        "AbstractEntityAITraining.java:58",
        200,
        Action::Run(|scene: &mut Scene, _: TrainingGoal| wander(scene)),
    ));
    machine.add(Transition::target(
        TrainingGoal::GoToTarget,
        "path_to_target",
        "AbstractEntityAITraining.java:59",
        20,
        Action::Run(|scene: &mut Scene, _: TrainingGoal| path_to_target(scene)),
    ));
}

/// The shooting academy's machine: the base, then the four archer states.
pub fn archer_machine() -> Machine<TrainingGoal, Scene> {
    let mut machine = Machine::new(
        TrainingGoal::Worker(WorkerState::Idle),
        ENTITY_AI_TICKRATE,
    );
    base_rules(&mut machine);
    machine.add(Transition::target(
        TrainingGoal::CombatTraining,
        "find_shooting_stand",
        "EntityAIArcherTraining.java:101",
        5,
        Action::Run(|scene: &mut Scene, _: TrainingGoal| find_shooting_stand(scene)),
    ));
    machine.add(Transition::target(
        TrainingGoal::ArcherSelectTarget,
        "select_target",
        "EntityAIArcherTraining.java:102",
        5,
        Action::Run(|scene: &mut Scene, _: TrainingGoal| select_target(scene)),
    ));
    machine.add(Transition::target(
        TrainingGoal::ArcherCheckShot,
        "check_shot",
        "EntityAIArcherTraining.java:103",
        CHECK_SHOT_DELAY.v() as u32,
        Action::Run(|scene: &mut Scene, _: TrainingGoal| check_shot(scene)),
    ));
    machine.add(Transition::target(
        TrainingGoal::ArcherShoot,
        "shoot",
        "EntityAIArcherTraining.java:104",
        5,
        Action::Run(|scene: &mut Scene, _: TrainingGoal| shoot(scene)),
    ));
    machine
}

/// The combat academy's machine: the base, then the six knight states.
pub fn combat_machine() -> Machine<TrainingGoal, Scene> {
    let mut machine = Machine::new(
        TrainingGoal::Worker(WorkerState::Idle),
        ENTITY_AI_TICKRATE,
    );
    base_rules(&mut machine);
    machine.add(Transition::target(
        TrainingGoal::CombatTraining,
        "decide_on_training_type",
        "EntityAICombatTraining.java:81",
        20,
        Action::Run(|scene: &mut Scene, _: TrainingGoal| decide_on_training_type(scene)),
    ));
    machine.add(Transition::target(
        TrainingGoal::FindTrainingPartner,
        "find_training_partner",
        "EntityAICombatTraining.java:82",
        20,
        Action::Run(|scene: &mut Scene, _: TrainingGoal| find_training_partner(scene)),
    ));
    machine.add(Transition::target(
        TrainingGoal::KnightTrainWithPartner,
        "train_with_partner",
        "EntityAICombatTraining.java:83",
        20,
        Action::Run(|scene: &mut Scene, _: TrainingGoal| train_with_partner(scene)),
    ));
    machine.add(Transition::target(
        TrainingGoal::FindDummyPartner,
        "find_dummy_partner",
        "EntityAICombatTraining.java:84",
        20,
        Action::Run(|scene: &mut Scene, _: TrainingGoal| find_dummy_partner(scene)),
    ));
    machine.add(Transition::target(
        TrainingGoal::KnightAttackDummy,
        "attack_dummy",
        "EntityAICombatTraining.java:85",
        20,
        Action::Run(|scene: &mut Scene, _: TrainingGoal| attack_dummy(scene)),
    ));
    machine.add(Transition::target(
        TrainingGoal::KnightAttackProtect,
        "attack_protect",
        "EntityAICombatTraining.java:86",
        TRAININGS_DELAY.v() as u32,
        Action::Run(|scene: &mut Scene, _: TrainingGoal| attack_partner(scene)),
    ));
    machine
}

/// The three tables this module implements.
pub fn tables() -> (
    &'static [Rule<TrainingGoal>],
    &'static [Rule<TrainingGoal>],
    &'static [Rule<TrainingGoal>],
) {
    (TRAINING_TABLE, ARCHER_TRAINING_TABLE, COMBAT_TRAINING_TABLE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trainee() -> Scene {
        Scene {
            facts: TrainingFacts {
                building_level: 1,
                setup: true,
                roll: 0,
                partner_roll: 99,
                line_clear: true,
                ..TrainingFacts::default()
            },
            ..Scene::default()
        }
    }

    #[test]
    fn a_trainee_that_is_not_set_up_stays_in_decide() {
        // `AbstractEntityAITraining.java:71-74` — `return DECIDE`, a self-loop.
        let mut scene = trainee();
        scene.facts.setup = false;
        assert_eq!(
            decide(&mut scene),
            Some(TrainingGoal::Worker(WorkerState::Decide))
        );
    }

    #[test]
    fn thirty_in_a_hundred_goes_to_combat_training_the_rest_walks() {
        // `:76-80`.
        let mut scene = trainee();
        scene.facts.roll = 29;
        assert_eq!(decide(&mut scene), Some(TrainingGoal::CombatTraining));
        scene.facts.roll = 30;
        assert_eq!(
            decide(&mut scene),
            Some(TrainingGoal::TrainingWander)
        );
    }

    #[test]
    fn the_archer_fires_at_most_five_arrows_per_building_level() {
        // `EntityAIArcherTraining.java:117-121`.
        let mut scene = trainee();
        scene.facts.shooting_target = Some(Pos::new(1, 64, 1));
        for _ in 0..5 {
            assert_eq!(
                select_target(&mut scene),
                Some(TrainingGoal::ArcherShoot)
            );
        }
        assert_eq!(scene.memory.target_counter, 5);
        assert_eq!(
            select_target(&mut scene),
            Some(TrainingGoal::Worker(WorkerState::Decide))
        );
        assert_eq!(scene.memory.target_counter, 0);
    }

    #[test]
    fn a_target_out_of_line_of_sight_is_skipped() {
        // `:123-127` — the ray must land on the target's own block.
        let mut scene = trainee();
        scene.facts.shooting_target = Some(Pos::new(1, 64, 1));
        scene.facts.line_clear = false;
        assert_eq!(
            select_target(&mut scene),
            Some(TrainingGoal::Worker(WorkerState::Decide))
        );
        assert_eq!(scene.memory.target_counter, 0);
    }

    #[test]
    fn the_archer_walks_to_a_stand_then_selects_a_target() {
        // `:140-153`.
        let mut scene = trainee();
        scene.facts.shooting_stand = None;
        assert_eq!(
            find_shooting_stand(&mut scene),
            Some(TrainingGoal::Worker(WorkerState::Decide))
        );
        scene.facts.shooting_stand = Some(Pos::new(3, 64, 3));
        assert_eq!(
            find_shooting_stand(&mut scene),
            Some(TrainingGoal::GoToTarget)
        );
        assert_eq!(scene.memory.state_after_pathing, Some(TrainingGoal::ArcherSelectTarget));
        assert_eq!(
            path_to_target(&mut scene),
            Some(TrainingGoal::ArcherSelectTarget)
        );
    }

    #[test]
    fn drawing_the_bow_then_releasing_it_and_reloading() {
        // `:168-213`.
        let mut scene = trainee();
        scene.memory.shooting_target = Some(Pos::new(1, 64, 1));
        // The delay is already spent, so the first look draws the bow.
        assert_eq!(shoot(&mut scene), Some(TrainingGoal::ArcherShoot));
        assert!(scene.using_item);
        // The second look releases: one arrow, two tenths of experience.
        assert_eq!(shoot(&mut scene), Some(TrainingGoal::ArcherCheckShot));
        assert!(!scene.using_item);
        assert_eq!(scene.experience, ARCHER_XP_BASE);
        assert_eq!(scene.memory.attack_delay, 10);
        // Then the reload counts down one per look.
        assert_eq!(shoot(&mut scene), Some(TrainingGoal::ArcherShoot));
        assert_eq!(scene.memory.attack_delay, 9);
    }

    #[test]
    fn a_hit_is_worth_one_experience_and_a_miss_two_tenths() {
        // `:221-230`.
        let mut hit = trainee();
        hit.facts.arrow_distance_sqr = 1.0;
        let _ = check_shot(&mut hit);
        assert_eq!(hit.experience, 1.0);
        let mut miss = trainee();
        miss.facts.arrow_distance_sqr = 4.0;
        let _ = check_shot(&mut miss);
        assert_eq!(miss.experience, ARCHER_XP_BASE);
        assert_eq!(miss.experience, 0.2);
    }

    #[test]
    fn the_archer_shoots_worse_as_its_skill_grows_but_not_that_much() {
        // `:182` — `15 / (primary / 2 + 1)`.
        assert_eq!(archer_hit_chance(0), 15.0);
        assert_eq!(archer_hit_chance(2), 7.5);
    }

    #[test]
    fn a_knight_with_a_partner_always_trains_with_it() {
        // `EntityAICombatTraining.java:97-101`.
        let mut scene = trainee();
        scene.facts.has_partner = true;
        assert_eq!(
            decide_on_training_type(&mut scene),
            Some(TrainingGoal::FindTrainingPartner)
        );
        let mut no_partner = trainee();
        no_partner.facts.partner_roll = 24;
        assert_eq!(
            decide_on_training_type(&mut no_partner),
            Some(TrainingGoal::FindTrainingPartner)
        );
        no_partner.facts.partner_roll = 25;
        assert_eq!(
            decide_on_training_type(&mut no_partner),
            Some(TrainingGoal::FindDummyPartner)
        );
    }

    #[test]
    fn a_partner_that_left_the_academy_ends_the_training() {
        // `:143-149`.
        let mut scene = trainee();
        scene.facts.partner_assigned = false;
        assert_eq!(
            train_with_partner(&mut scene),
            Some(TrainingGoal::CombatTraining)
        );
        assert_eq!(scene.memory.partner, None);
    }

    #[test]
    fn five_blocks_apart_the_knight_swings_without_damage() {
        // `:164-234`.
        let mut scene = trainee();
        scene.memory.partner = Some(Pos::new(2, 64, 0));
        scene.facts.partner = Some(Pos::new(2, 64, 0));
        scene.facts.partner_distance = 4.0;
        scene.facts.coin = false;
        assert_eq!(
            attack_partner(&mut scene),
            Some(TrainingGoal::KnightAttackProtect)
        );
        assert_eq!(scene.experience, 2.0);
        assert!(!scene.using_item, "a swing, not a shield");
        assert_eq!(scene.memory.attack_delay, 10);
        assert_eq!(scene.memory.target_counter, 1);
    }

    #[test]
    fn the_shield_branch_raises_the_shield_instead_of_swinging() {
        // `:198-208`.
        let mut scene = trainee();
        scene.memory.partner = Some(Pos::new(2, 64, 0));
        scene.facts.partner = Some(Pos::new(2, 64, 0));
        scene.facts.partner_distance = 2.0;
        scene.facts.coin = true;
        assert_eq!(
            attack_partner(&mut scene),
            Some(TrainingGoal::KnightAttackProtect)
        );
        assert!(scene.using_item);
    }

    #[test]
    fn the_partner_counter_is_strictly_greater_than_five_per_level() {
        // `:219` — `targetCounter > level * 5`, one more blow than the dummy.
        let mut scene = trainee();
        scene.memory.partner = Some(Pos::new(2, 64, 0));
        scene.facts.partner = Some(Pos::new(2, 64, 0));
        scene.facts.partner_distance = 2.0;
        scene.memory.target_counter = 4;
        assert_eq!(
            attack_partner(&mut scene),
            Some(TrainingGoal::KnightAttackProtect),
            "the fifth blow is still one short of six"
        );
        assert_eq!(scene.memory.target_counter, 5);
        assert!(!scene.partner_reset);
        // `:178-181` — `currentAttackDelay > 0` renvoie sans frapper : il faut
        // laisser le delai s'ecouler avant le sixieme coup.
        scene.memory.attack_delay = 0;
        let _ = attack_partner(&mut scene);
        assert_eq!(scene.memory.target_counter, 0);
        assert!(scene.partner_reset, "the sixth blow ends the session");
    }

    #[test]
    fn the_dummy_counter_gives_up_at_five_per_level() {
        // `:244` — `targetCounter >= level * 5`.
        let mut scene = trainee();
        scene.memory.target_counter = 5;
        assert_eq!(
            find_dummy_partner(&mut scene),
            Some(TrainingGoal::Worker(WorkerState::Decide))
        );
        assert_eq!(scene.memory.target_counter, 0);
    }

    #[test]
    fn a_dummy_sends_the_knight_to_go_to_target_again() {
        // `:256-268`.
        let mut scene = trainee();
        scene.facts.dummy = Some(Pos::new(6, 64, 6));
        assert_eq!(
            find_dummy_partner(&mut scene),
            Some(TrainingGoal::GoToTarget)
        );
        assert_eq!(scene.memory.state_after_pathing, Some(TrainingGoal::KnightAttackDummy));
        assert_eq!(scene.memory.combat_target, Some(Pos::new(6, 64, 6)));
    }

    #[test]
    fn attacking_a_dummy_also_hits_for_nothing() {
        // `:283-329`.
        let mut scene = trainee();
        scene.memory.combat_target = Some(Pos::new(6, 64, 6));
        scene.facts.coin = false;
        assert_eq!(
            attack_dummy(&mut scene),
            Some(TrainingGoal::FindDummyPartner)
        );
        assert_eq!(scene.experience, 2.0);
        assert_eq!(scene.memory.attack_delay, 10);
    }

    #[test]
    fn an_archer_machine_matches_its_two_tables() {
        let machine = archer_machine();
        let rules: Vec<&'static str> = machine
            .transitions()
            .iter()
            .map(|transition| transition.rule)
            .collect();
        let expected: Vec<&'static str> = TRAINING_TABLE
            .iter()
            .chain(ARCHER_TRAINING_TABLE.iter())
            .map(|rule| rule.rule)
            .collect();
        assert_eq!(rules, expected);
    }

    #[test]
    fn a_knight_machine_matches_its_two_tables() {
        let machine = combat_machine();
        let rules: Vec<&'static str> = machine
            .transitions()
            .iter()
            .map(|transition| transition.rule)
            .collect();
        let expected: Vec<&'static str> = TRAINING_TABLE
            .iter()
            .chain(COMBAT_TRAINING_TABLE.iter())
            .map(|rule| rule.rule)
            .collect();
        assert_eq!(rules, expected);
    }

    #[test]
    fn the_training_constants_keep_their_lines() {
        assert_eq!(TARGET_SEARCH_CHANCE.v(), 30);
        assert_eq!(BUILDING_LEVEL_TARGET_MULTIPLIER.v(), 5);
        assert_eq!(ACTIONS_PER_BUILDING_LEVEL.v(), 5);
        assert_eq!(TRAININGS_DELAY.v(), 60);
        assert_eq!(CHECK_SHOT_DELAY.v(), 60);
        assert_eq!(PARTNER_TRAINING_CHANCE.v(), 25);
        assert!(COMBAT_RELOAD.source.contains("EntityAICombatTraining.java:225"));
    }
}
