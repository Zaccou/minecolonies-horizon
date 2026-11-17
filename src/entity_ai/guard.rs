//! The guard's own goals: sleeping on duty, fleeing at low health, and the four
//! ways of walking a post.
//!
//! Java reference: `core/entity/ai/workers/guard/AbstractEntityAIGuard.java`, with
//! `AbstractEntityAIFight.java` for the three states it inherits and
//! `EntityAICavalry.java` for the mounted variant.
//!
//! **A guard's machine is the worker's machine**, so [`GuardGoal`] borrows
//! [`WorkerState`] rather than redeclaring `PREPARING`, `START_WORKING`,
//! `INVENTORY_FULL` and `NEEDS_ITEM`. The two states a guard may be pulled out of to
//! fight — `NEEDS_ITEM` and `INVENTORY_FULL` — get the two group rules of
//! `BehaviourStateGroup.GUARD_ABORT_AND_FIGHT` (`:16`), inserted **in front** of
//! both lists by `addTransitionGroup` (`BasicStateMachine.java:76-95`). That is why
//! [`machine`] registers them with `add_group` and not with `add`.
//!
//! **Sleeping on duty is the strangest rule here.** `shouldSleep` (`:268-290`) needs
//! four conditions *and* two draws: `nextInt(20 + Adaptability / 2) == 1` and
//! `nextDouble() < 1 / (1 + SLEEP_LESS)`. A level-1 guard naps once in twenty-one
//! checks, twice a minute at worst, for 2500 to 3000 ticks — over two minutes of a
//! guard asleep at its post, which is exactly what enemies exploit.
//!
//! **The execution distance is not one number.** `getPersecutionDistance`
//! (`:757-773`) answers 80 on patrol, 30 when following or patrolling a mine, 30 at
//! a rally point, and 10 at a post — **plus 20 for a knight**, so a knight chases
//! twice as far from its post as a ranger. `isWithinPersecutionDistance` then
//! squares the sum (`:728-731`).
//!
//! **`regen` overrides its own condition.** The transition fires when health drops
//! under 75% (`:375`), but the *action* re-applies Regeneration every forty ticks
//! while that holds, so the guard can stand at its hut for a long time with the
//! effect on and never leave `GUARD_REGEN`.

use crate::ai::facts::Pos;
use crate::ai::state::{Action, Machine, Transition, WorkerState, ENTITY_AI_TICKRATE};
use super::facts::{manhattan, GuardFacts, GuardTask, Tuned};
use super::goals::{
    CombatState, GuardGoal, Rule, GUARD_ABORT_GROUP, GUARD_ABORT_TABLE, GUARD_TABLE,
};

/// `ACTIONS_UNTIL_DUMPING` (`:63`): five kills per building level before a dump.
pub const ACTIONS_UNTIL_DUMPING: Tuned = Tuned::new(5, "workers/guard/AbstractEntityAIGuard.java:63");
/// `MAX_PATROL_DERIVATION` (`:68`).
pub const MAX_PATROL_DERIVATION: Tuned =
    Tuned::new(80, "workers/guard/AbstractEntityAIGuard.java:68");
/// `PATROL_DEVIATION_RAID_POINT` (`:73`): `40 * 40`, a squared distance.
pub const PATROL_DEVIATION_RAID_POINT: Tuned =
    Tuned::new(1600, "workers/guard/AbstractEntityAIGuard.java:73");
/// `MAX_FOLLOW_DERIVATION` (`:78`).
pub const MAX_FOLLOW_DERIVATION: Tuned =
    Tuned::new(30, "workers/guard/AbstractEntityAIGuard.java:78");
/// `MAX_GUARD_DERIVATION` (`:83`).
pub const MAX_GUARD_DERIVATION: Tuned =
    Tuned::new(10, "workers/guard/AbstractEntityAIGuard.java:83");
/// The extra reach a knight gets at its post (`:771`).
pub const KNIGHT_DERIVATION_BONUS: Tuned =
    Tuned::new(20, "workers/guard/AbstractEntityAIGuard.java:771");
/// `COMBAT_TIME` (`:88`): `30 * 20` ticks of combat memory.
pub const COMBAT_TIME: Tuned = Tuned::new(600, "workers/guard/AbstractEntityAIGuard.java:88");
/// `PARTICLE_INTERVAL` (`:108`).
pub const PARTICLE_INTERVAL: Tuned =
    Tuned::new(30, "workers/guard/AbstractEntityAIGuard.java:108");
/// `SHOULD_SLEEP_INTERVAL` (`:113`).
pub const SHOULD_SLEEP_INTERVAL: Tuned =
    Tuned::new(200, "workers/guard/AbstractEntityAIGuard.java:113");
/// `GUARD_TASK_INTERVAL` (`:118`).
pub const GUARD_TASK_INTERVAL: Tuned =
    Tuned::new(100, "workers/guard/AbstractEntityAIGuard.java:118");
/// `GUARD_REGEN_INTERVAL` (`:123`).
pub const GUARD_REGEN_INTERVAL: Tuned =
    Tuned::new(40, "workers/guard/AbstractEntityAIGuard.java:123");
/// `ACTION_INCREASE_INTERVAL` (`:128`).
pub const ACTION_INCREASE_INTERVAL: Tuned =
    Tuned::new(10, "workers/guard/AbstractEntityAIGuard.java:128");
/// The nap length base (`:282`): `nextInt(500) + 2500`.
pub const NAP_TICKS: Tuned = Tuned::new(2500, "workers/guard/AbstractEntityAIGuard.java:282");
/// The nap spread (`:282`).
pub const NAP_SPREAD: Tuned = Tuned::new(500, "workers/guard/AbstractEntityAIGuard.java:282");
/// The wake-up timer ceiling (`:240`): thirty ticks and the guard gives up.
pub const WAKE_TIMER_LIMIT: Tuned =
    Tuned::new(30, "workers/guard/AbstractEntityAIGuard.java:240");
/// The reach of the wake-up blow (`:248`): `getDistanceSquared(...) > 2.25` — a
/// **squared** distance of one and a half blocks.
pub const WAKE_REACH_SQUARED: super::facts::TunedF =
    super::facts::TunedF::new(2.25, "workers/guard/AbstractEntityAIGuard.java:248");
/// The low-health flee threshold (`:355`): `0.2`.
pub const FLEE_HEALTH_FRACTION: f64 = 0.2;
/// The regen threshold (`:375`): `0.75`.
pub const REGEN_HEALTH_FRACTION: f64 = 0.75;
/// How far from the hut a guard must be to bother fleeing home (`:355`).
pub const FLEE_DISTANCE_SQUARED: Tuned =
    Tuned::new(20, "workers/guard/AbstractEntityAIGuard.java:355");
/// `GLOW_EFFECT_DURATION` (`Constants.java:168`, `20 * 30`) — the glow a rallying
/// or following guard wears (`AbstractEntityAIGuard.java:700`).
pub const GLOW_DURATION: Tuned = Tuned::new(600, "api/util/constant/Constants.java:168");
/// The shuffle of a guarding soldier (`:427`): one in ten.
pub const GUARD_SHUFFLE_CHANCE: Tuned =
    Tuned::new(10, "workers/guard/AbstractEntityAIGuard.java:427");
/// The patrol walk that gives up after twenty blocks (`:517`).
pub const MANUAL_PATROL_WANDER: Tuned =
    Tuned::new(20, "workers/guard/AbstractEntityAIGuard.java:517");
/// The patrol point change chance (`:522`): `nextInt(5) <= 1`.
pub const PATROL_POINT_CHANCE: Tuned =
    Tuned::new(5, "workers/guard/AbstractEntityAIGuard.java:522");
/// `GUARD_FOLLOW_TIGHT_RANGE` (`GuardConstants.java:23`).
pub const GUARD_FOLLOW_TIGHT_RANGE: Tuned =
    Tuned::new(6, "api/util/constant/GuardConstants.java:23");
/// `GUARD_FOLLOW_LOSE_RANGE` (`:28`).
pub const GUARD_FOLLOW_LOSE_RANGE: Tuned =
    Tuned::new(15, "api/util/constant/GuardConstants.java:28");

/// The guard's own fields, beyond the combat AI's.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Memory {
    /// `sleepTimer` (`:133`).
    pub sleep_timer: i32,
    /// `wakeTimer` (`:138`).
    pub wake_timer: i32,
    /// `fighttimer` (`:143`).
    pub fight_timer: i32,
    /// `regularActionTimer` (`:158`).
    pub regular_action_timer: i32,
    /// `currentPatrolPoint` (`:98`).
    pub current_patrol_point: Option<Pos>,
    /// `lastGuardActionPos` (`:163`).
    pub last_action_pos: Pos,
    /// `sleepingGuard` (`:148`) — the comrade to wake.
    pub sleeping_guard: Option<Pos>,
    /// `target` (`:93`).
    pub target: Option<Pos>,
}

/// The facts the guard machine reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    /// The guard, its hut and its task.
    pub facts: GuardFacts,
    /// What it remembers.
    pub memory: Memory,
    /// The look the guard is doing (`setLookAt`).
    pub looking_at: Option<Pos>,
    /// `SittingEntity.sitDown(worker.blockPosition(), worker, sleepTimer)` (`:284`).
    pub sat_down: bool,
    /// `stopRiding()` + `setPos(y + 1)` on waking (`:341-343`).
    pub stood_up: bool,
    /// The experience the guard earned this tick (`:343`, `:436`).
    pub experience: f64,
    /// The reach the guard asked for (`walkToUnSafePos(pos, range)`).
    pub walked_to: Option<(Pos, i32)>,
    /// `worker.addEffect(MobEffects.REGENERATION, 200)` (`:379`).
    pub regenerating: bool,
    /// The speed effect applied while fleeing (`:396-400`).
    pub speed_effect: Option<i32>,
    /// `worker.addEffect(GLOW_EFFECT, ...)` (`:700`).
    pub glowing: bool,
    /// A player asked for help and the guard accepted (`canHelp`, `:643-657`).
    pub helping: bool,
    /// The patrol point the guard registered as reached (`:538`).
    pub patrol_reached: bool,
    /// `worker.setCanBeStuck(...)` / the rails flag of `onCombatEnter` (`:211-221`).
    pub can_use_rails: bool,
    /// `buildingGuards.setTempNextPatrolPoint(buildingGuards.getPosition())`
    /// (`:491`): the guard sends its next patrol to its own hut on the way in.
    pub temp_patrol_point: Option<Pos>,
}

impl Scene {
    /// A guard at its post, with its memory seeded from the hut's own position
    /// (`:184`).
    pub fn with_hut(at: Pos) -> Scene {
        Scene {
            memory: Memory {
                last_action_pos: at,
                ..Memory::default()
            },
            facts: GuardFacts {
                at,
                building_at: at,
                guard_pos: at,
                ..GuardFacts::default()
            },
            ..Scene::default()
        }
    }
}

/// `inCombat` (`:190-204`).
pub fn in_combat(scene: &mut Scene) -> Option<GuardGoal> {
    if scene.memory.fight_timer <= 0 {
        // `onCombatEnter` (`:209-213`): no stuck state, no rails while fighting.
        scene.can_use_rails = false;
    }
    if !scene.facts.has_tool {
        return Some(GuardGoal::preparing());
    }
    scene.memory.fight_timer = COMBAT_TIME.v() as i32;
    None
}

/// `shouldSleep` (`:268-290`).
pub fn should_sleep(scene: &mut Scene) -> bool {
    if scene.facts.last_hurt_by_mob
        || scene.facts.target.is_some()
        || scene.memory.fight_timer > 0
        || scene.facts.sick
    {
        return false;
    }
    let chance = 1.0 / (1.0 + scene.facts.research.sleep_less);
    // `nextInt((int) (Adaptability * 0.5) + 20) == 1`
    let denominator = (scene.facts.adaptability as f64 * 0.5) as i32 + 20;
    let draw = scene.facts.sleep_roll.rem_euclid(denominator.max(1));
    if draw == 1 && scene.facts.sleep_chance < chance {
        scene.memory.sleep_timer =
            NAP_TICKS.v() as i32 + scene.facts.nap_roll.rem_euclid(NAP_SPREAD.v() as i32);
        scene.sat_down = true;
        return true;
    }
    false
}

/// `sleep` (`:314-332`).
pub fn sleep_on_duty(scene: &mut Scene) -> Option<GuardGoal> {
    scene.memory.sleep_timer -= ENTITY_AI_TICKRATE as i32;
    if scene.facts.last_hurt_by_mob || scene.memory.sleep_timer < 0 {
        stop_sleeping(scene);
        scene.memory.target = None;
        return Some(GuardGoal::NoTarget);
    }
    scene.looking_at = Some(scene.facts.at);
    None
}

/// `sleepParticles` (`:297-307`): the particle, and half a point of health.
pub fn sleep_particles(scene: &mut Scene) -> Option<GuardGoal> {
    if scene.facts.health < scene.facts.max_health {
        scene.facts.health += 0.5;
    }
    None
}

/// `stopSleeping` (`:337-346`).
pub fn stop_sleeping(scene: &mut Scene) {
    scene.sat_down = false;
    scene.stood_up = true;
    scene.experience += 1.0;
}

/// `shouldFlee` (`:353-361`).
///
/// Four conditions in one `if`: the hut retrieves, the health is under 20%, the
/// guard is more than `sqrt(20)` blocks from its hut, **and** `RETREAT` is
/// researched. A guard without the research dies at its post.
pub fn should_flee(scene: &Scene) -> bool {
    if !scene.facts.retrieves_on_low_health {
        return false;
    }
    let threshold = (scene.facts.max_health as i32) as f64 * FLEE_HEALTH_FRACTION;
    if scene.facts.health >= threshold {
        return false;
    }
    if scene.facts.at.dist_sqr(scene.facts.building_at) <= FLEE_DISTANCE_SQUARED.v() {
        return false;
    }
    scene.facts.research.retreat > 0.0
}

/// `regen` (`:368-385`).
pub fn regen(scene: &mut Scene) -> Option<GuardGoal> {
    if scene.facts.threat_within_10 {
        return Some(GuardGoal::Attacking);
    }
    let threshold = (scene.facts.max_health as i32) as f64 * REGEN_HEALTH_FRACTION;
    if scene.facts.health < threshold && scene.facts.retrieves_on_low_health {
        if !scene.facts.has_regeneration {
            scene.regenerating = true;
        }
        return Some(GuardGoal::GuardRegen);
    }
    Some(GuardGoal::start_working())
}

/// `flee` (`:392-409`).
pub fn flee_home(scene: &mut Scene) -> Option<GuardGoal> {
    if !scene.facts.has_speed {
        let effect = scene.facts.research.fleeing_speed;
        if effect > 0.0 {
            scene.speed_effect = Some(effect as i32);
        }
    }
    if !scene.facts.arrived_at_building {
        return Some(GuardGoal::GuardFlee);
    }
    Some(GuardGoal::GuardRegen)
}

/// `wakeUpGuard` (`:229-261`).
///
/// Three ways out: no comrade any more, thirty ticks spent, or the blow landed —
/// and the blow is `hurt(..., 1)`, one point of damage, with the waker set as the
/// attacker so the sleeper fights back (`:255-257`).
pub fn wake_up_guard(scene: &mut Scene) -> Option<GuardGoal> {
    if scene.memory.sleeping_guard.is_none() {
        return Some(GuardGoal::NoTarget);
    }
    scene.memory.wake_timer += 1;
    if scene.memory.wake_timer > WAKE_TIMER_LIMIT.v() as i32 {
        return Some(GuardGoal::NoTarget);
    }
    let sleeper = match scene.memory.sleeping_guard {
        Some(pos) => pos,
        None => return Some(GuardGoal::NoTarget),
    };
    if (sleeper.dist_sqr(scene.facts.at) as f64) > WAKE_REACH_SQUARED.v() {
        scene.walked_to = Some((sleeper, 1));
        return None;
    }
    scene.memory.sleeping_guard = None;
    Some(GuardGoal::NoTarget)
}

/// `guardMovement` (`:425-431`): one shuffle in ten towards the post.
pub fn guard_movement(scene: &mut Scene) -> Option<GuardGoal> {
    if scene.facts.shuffle_roll.rem_euclid(GUARD_SHUFFLE_CHANCE.v() as i32) == 0 {
        scene.walked_to = Some((scene.facts.guard_pos, 5));
    }
    None
}

/// `follow` (`:438-448`).
pub fn follow(scene: &mut Scene) -> Option<GuardGoal> {
    let position = match scene.facts.follow_pos {
        Some(pos) => pos,
        None => return Some(GuardGoal::preparing()),
    };
    // `BlockPosUtil.getDistance2D` is Manhattan (`BlockPosUtil.java:423-429`).
    if manhattan(scene.facts.at, position) > MAX_FOLLOW_DERIVATION.v() {
        // `TeleportHelper.teleportCitizen(worker, ..., positionToFollow)`.
        scene.memory.last_action_pos = position;
        return None;
    }
    let range = if scene.facts.tight_grouping {
        GUARD_FOLLOW_TIGHT_RANGE.v() as i32
    } else {
        GUARD_FOLLOW_LOSE_RANGE.v() as i32
    };
    scene.walked_to = Some((position, range));
    None
}

/// `rally` (`:461-484`): a random offset of up to [`GUARD_FOLLOW_TIGHT_RANGE`] and a
/// speed effect that grows with adaptability, `Adaptability / 30` clamped to 3.
pub fn rally(scene: &mut Scene) -> Option<GuardGoal> {
    let position = match scene.facts.rally {
        Some(pos) => pos,
        None => return None,
    };
    if !scene.facts.arrived_at_rally && !scene.facts.has_speed {
        let amplifier = (scene.facts.adaptability / 30).clamp(0, 3);
        scene.speed_effect = Some(amplifier);
    }
    scene.walked_to = Some((position, GUARD_FOLLOW_TIGHT_RANGE.v() as i32));
    None
}

/// `patrol` (`:510-542`).
pub fn patrol(scene: &mut Scene) -> Option<GuardGoal> {
    if scene.facts.manual_target {
        if scene.memory.current_patrol_point.is_none()
            || scene.facts.arrived_at_patrol
            || !scene.facts.patrol_loaded
        {
            scene.memory.current_patrol_point = None;
            if scene.facts.shuffle_roll.rem_euclid(PATROL_POINT_CHANCE.v() as i32) <= 1 {
                scene.memory.current_patrol_point = scene.facts.patrol_point;
            }
        }
        return None;
    }
    // `:535-539` — `currentPatrolPoint != null && (!isEntityBlockLoaded || walkToSafePos(...))` :
    // l'arrivee compte autant que le chunk decharge, et les deux appellent
    // `setCurrentDelay(10)` puis `arrivedAtPatrolPoint(worker)`.
    if scene.facts.patrol_point.is_some()
        && (!scene.facts.patrol_loaded || scene.facts.arrived_at_patrol)
    {
        scene.patrol_reached = true;
    }
    scene.memory.current_patrol_point = scene.facts.patrol_point;
    None
}

/// `patrolMine` (`:549-584`).
///
/// **A guard patrolling a mine whose level is gone is switched back to `PATROL`**
/// (`:575`, `:580`) — the hut's setting is rewritten from the AI, one of the few
/// places a decision changes the player's configuration.
pub fn patrol_mine(scene: &mut Scene) -> Option<GuardGoal> {
    if scene.facts.mine_pos.is_none() {
        return Some(GuardGoal::preparing());
    }
    if scene.memory.current_patrol_point.is_none() || scene.facts.arrived_at_patrol {
        scene.memory.current_patrol_point = scene.facts.patrol_point.or(scene.facts.mine_pos);
    }
    None
}

/// `decide` (`:664-720`).
pub fn decide(scene: &mut Scene) -> Option<GuardGoal> {
    // `:668` — `if (regularActionTimer++ > ACTION_INCREASE_INTERVAL)`: c'est la
    // valeur **avant** increment qui est comparee, donc on remet a zero au
    // douzieme passage, pas au onzieme.
    let previous_action_timer = scene.memory.regular_action_timer;
    scene.memory.regular_action_timer += 1;
    if previous_action_timer > ACTION_INCREASE_INTERVAL.v() as i32 {
        scene.memory.regular_action_timer = 0;
    }
    if scene.facts.armour_roll < 0.05 {
        // `equipInventoryArmor()` (`:676`).
        scene.looking_at = None;
    }
    if !scene.facts.has_tool {
        return Some(GuardGoal::preparing());
    }
    if scene.memory.fight_timer > 0 {
        scene.memory.fight_timer -= GUARD_TASK_INTERVAL.v() as i32;
        if scene.memory.fight_timer <= 0 {
            // `onCombatLeave` (`:218-222`).
            scene.can_use_rails = true;
        }
    } else {
        scene.memory.last_action_pos = scene.facts.at;
    }

    scene.glowing = scene.facts.rally.is_some() || scene.facts.task == GuardTask::Follow;

    if scene.facts.rally.is_some() && scene.facts.rally_reachable {
        return rally(scene);
    }
    match scene.facts.task {
        GuardTask::Guard => guard_movement(scene),
        GuardTask::Patrol => patrol(scene),
        GuardTask::Follow => follow(scene),
        GuardTask::PatrolMine => patrol_mine(scene),
    }
}

/// `getTaskReferencePoint` (`:738-750`): what the execution distance is measured
/// from.
pub fn task_reference_point(scene: &Scene) -> Pos {
    match scene.facts.task {
        GuardTask::Patrol | GuardTask::PatrolMine => scene.memory.last_action_pos,
        GuardTask::Follow => scene.facts.follow_pos.unwrap_or(scene.facts.building_at),
        GuardTask::Guard => scene.facts.guard_pos,
    }
}

/// `getPersecutionDistance` (`:757-773`).
pub fn persecution_distance(scene: &Scene, knight: bool) -> i32 {
    if scene.facts.rally.is_some() {
        return MAX_FOLLOW_DERIVATION.v() as i32;
    }
    match scene.facts.task {
        GuardTask::Patrol => MAX_PATROL_DERIVATION.v() as i32,
        GuardTask::PatrolMine | GuardTask::Follow => MAX_FOLLOW_DERIVATION.v() as i32,
        GuardTask::Guard => {
            MAX_GUARD_DERIVATION.v() as i32
                + if knight {
                    KNIGHT_DERIVATION_BONUS.v() as i32
                } else {
                    0
                }
        }
    }
}

/// `isWithinPersecutionDistance` (`:728-731`): the sum of the two ranges, squared.
pub fn within_persecution_distance(scene: &Scene, entity: Pos, attack_range: f64, knight: bool) -> bool {
    let reach = persecution_distance(scene, knight) as f64 + attack_range;
    (task_reference_point(scene).dist_sqr(entity) as f64) <= reach * reach
}

/// `canBeInterrupted` (`:776-784`).
pub fn can_be_interrupted(scene: &Scene) -> bool {
    if scene.memory.fight_timer > 0
        || scene.facts.last_attacker
        || scene.facts.rally.is_some()
        || scene.facts.task == GuardTask::Follow
    {
        return false;
    }
    true
}

/// `canHelp` (`:643-657`).
pub fn can_help(scene: &Scene, pos: Pos, attack_range: f64, knight: bool) -> bool {
    if !can_be_interrupted(scene) {
        return false;
    }
    if scene.facts.task == GuardTask::Guard
        && !within_persecution_distance(scene, pos, attack_range, knight)
    {
        return false;
    }
    true
}

/// `startHelpCitizen` (`:628-635`): the threat a comrade's attacker gets.
pub fn help_scene(scene: &mut Scene, pos: Pos, attack_range: f64, knight: bool) -> bool {
    if !can_help(scene, pos, attack_range, knight) {
        return false;
    }
    scene.helping = true;
    stop_sleeping(scene);
    true
}

/// `getActionsDoneUntilDumping` (`:450-454`): five per building level.
pub fn actions_until_dumping(building_level: i32) -> i32 {
    ACTIONS_UNTIL_DUMPING.v() as i32 * building_level
}

/// The guard's machine: [`GUARD_TABLE`], then the two abort rules inserted in front
/// of `NEEDS_ITEM` and `INVENTORY_FULL`.
pub fn machine() -> Machine<GuardGoal, Scene> {
    let mut machine = Machine::new(GuardGoal::Worker(WorkerState::Idle), ENTITY_AI_TICKRATE);
    machine.add(Transition::target(
        GuardGoal::Worker(WorkerState::Idle),
        "idle",
        "AbstractEntityAIFight.java:74",
        1,
        Action::Fixed(Some(GuardGoal::start_working())),
    ));
    machine.add(Transition::target(
        GuardGoal::start_working(),
        "start_working",
        "AbstractEntityAIGuard.java:491",
        100,
        Action::Run(|scene: &mut Scene, _: GuardGoal| {
            // L'override du garde : `setTempNextPatrolPoint(position)` puis `DECIDE`,
            // sans `walkToBuilding` et sans passer par `PREPARING`.
            scene.temp_patrol_point = Some(scene.facts.building_at);
            Some(GuardGoal::decide())
        }),
    ));
    machine.add(Transition::target(
        GuardGoal::preparing(),
        "prepare",
        "AbstractEntityAIFight.java:76",
        20,
        Action::Run(|scene: &mut Scene, _: GuardGoal| {
            if scene.facts.has_tool {
                Some(GuardGoal::decide())
            } else {
                None
            }
        }),
    ));
    machine.add(Transition::goto(
        GuardGoal::decide(),
        GuardGoal::NoTarget,
        "decide_to_no_target",
        "AbstractEntityAIGuard.java:169",
        1,
    ));
    machine.add(Transition::target(
        GuardGoal::NoTarget,
        "should_sleep",
        "AbstractEntityAIGuard.java:170",
        SHOULD_SLEEP_INTERVAL.v() as u32,
        Action::Run(|scene: &mut Scene, _: GuardGoal| {
            if should_sleep(scene) {
                Some(GuardGoal::GuardSleep)
            } else {
                None
            }
        }),
    ));
    machine.add(Transition::target(
        GuardGoal::GuardSleep,
        "sleep",
        "AbstractEntityAIGuard.java:171",
        1,
        Action::Run(|scene: &mut Scene, _: GuardGoal| sleep_on_duty(scene)),
    ));
    machine.add(Transition::target(
        GuardGoal::GuardSleep,
        "sleep_particles",
        "AbstractEntityAIGuard.java:172",
        PARTICLE_INTERVAL.v() as u32,
        Action::Run(|scene: &mut Scene, _: GuardGoal| sleep_particles(scene)),
    ));
    machine.add(Transition::target(
        GuardGoal::GuardRegen,
        "regen",
        "AbstractEntityAIGuard.java:173",
        GUARD_REGEN_INTERVAL.v() as u32,
        Action::Run(|scene: &mut Scene, _: GuardGoal| regen(scene)),
    ));
    machine.add(Transition::target(
        GuardGoal::GuardFlee,
        "flee",
        "AbstractEntityAIGuard.java:174",
        20,
        Action::Run(|scene: &mut Scene, _: GuardGoal| flee_home(scene)),
    ));
    machine.add(Transition::target_when(
        GuardGoal::Attacking,
        "should_flee_in_combat",
        "AbstractEntityAIGuard.java:175",
        GUARD_REGEN_INTERVAL.v() as u32,
        |scene: &Scene, _: GuardGoal| should_flee(scene),
        Action::Fixed(Some(GuardGoal::GuardFlee)),
    ));
    machine.add(Transition::target_when(
        GuardGoal::NoTarget,
        "should_flee_idle",
        "AbstractEntityAIGuard.java:176",
        GUARD_REGEN_INTERVAL.v() as u32,
        |scene: &Scene, _: GuardGoal| should_flee(scene),
        Action::Fixed(Some(GuardGoal::GuardFlee)),
    ));
    machine.add(Transition::target(
        GuardGoal::NoTarget,
        "guard_decide",
        "AbstractEntityAIGuard.java:177",
        GUARD_TASK_INTERVAL.v() as u32,
        Action::Run(|scene: &mut Scene, _: GuardGoal| decide(scene)),
    ));
    machine.add(Transition::target(
        GuardGoal::GuardWake,
        "wake_up_guard",
        "AbstractEntityAIGuard.java:178",
        20,
        Action::Run(|scene: &mut Scene, _: GuardGoal| wake_up_guard(scene)),
    ));
    machine.add(Transition::target(
        GuardGoal::Attacking,
        "in_combat",
        "AbstractEntityAIGuard.java:180",
        8,
        Action::Run(|scene: &mut Scene, _: GuardGoal| in_combat(scene)),
    ));

    // `addTransitionGroup(GUARD_ABORT_AND_FIGHT, ...)` — in front of both lists
    // (`MeleeCombatAI.java:109-110`).
    machine.add_group(
        &GUARD_ABORT_GROUP,
        Transition::target(
            GuardGoal::NoTarget,
            "busy_check_target",
            "MeleeCombatAI.java:109",
            5,
            Action::Fixed(Some(GuardGoal::Attacking)),
        )
        .without_state(),
    );
    machine.add_group(
        &GUARD_ABORT_GROUP,
        Transition::target(
            GuardGoal::NoTarget,
            "busy_search_target",
            "MeleeCombatAI.java:110",
            80,
            Action::Fixed(Some(GuardGoal::Attacking)),
        )
        .without_state(),
    );
    machine
}

/// The combat helper the abort group and the combat machine share: `checkForTarget`
/// on a guard's scene, expressed on the combat facts (`:109`).
pub fn busy_check_target(targeted: bool) -> Option<CombatState> {
    if targeted {
        Some(CombatState::Attacking)
    } else {
        None
    }
}

/// `CombatState` is re-exported so a caller of [`busy_check_target`] does not have
/// to import two modules.
pub use super::goals::CombatState as GuardCombatState;

/// The table this module implements, first part.
pub fn table() -> &'static [Rule<GuardGoal>] {
    GUARD_TABLE
}

/// The second part: the two abort rules.
pub fn abort_table() -> &'static [Rule<CombatState>] {
    GUARD_ABORT_TABLE
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::state::WorkerState;

    fn guard() -> Scene {
        let mut scene = Scene::with_hut(Pos::new(0, 64, 0));
        scene.facts.has_tool = true;
        scene.facts.health = 20.0;
        scene.facts.max_health = 20.0;
        scene
    }

    #[test]
    fn a_guard_without_a_weapon_goes_and_fetches_one_instead_of_fighting() {
        // `:190-204`.
        let mut scene = guard();
        scene.facts.has_tool = false;
        assert_eq!(in_combat(&mut scene), Some(GuardGoal::preparing()));
        assert!(!scene.can_use_rails, "combat entered even without a weapon");
        scene.facts.has_tool = true;
        assert_eq!(in_combat(&mut scene), None);
        assert_eq!(scene.memory.fight_timer, 600);
    }

    #[test]
    fn a_healthy_guard_never_naps() {
        // `:270-273`: a hurt guard, a target, a fight timer or a sickness forbid it.
        let mut scene = guard();
        scene.facts.last_hurt_by_mob = true;
        assert!(!should_sleep(&mut scene));
        let mut targeted = guard();
        targeted.facts.target = Some(Pos::new(1, 64, 0));
        assert!(!should_sleep(&mut targeted));
        let mut fighting = guard();
        fighting.memory.fight_timer = 100;
        assert!(!should_sleep(&mut fighting));
        let mut sick = guard();
        sick.facts.sick = true;
        assert!(!should_sleep(&mut sick));
    }

    #[test]
    fn a_level_one_guard_naps_once_in_twenty_one_checks() {
        // `:278-287`: `nextInt(20 + Adaptability / 2) == 1`.
        let mut hit = guard();
        hit.facts.adaptability = 2;
        hit.facts.sleep_roll = 1;
        hit.facts.sleep_chance = 0.5;
        assert!(should_sleep(&mut hit));
        assert!(hit.memory.sleep_timer >= 2500 && hit.memory.sleep_timer < 3000);
        assert!(hit.sat_down);

        let mut miss = guard();
        miss.facts.adaptability = 2;
        miss.facts.sleep_roll = 0;
        miss.facts.sleep_chance = 0.5;
        assert!(!should_sleep(&mut miss));
    }

    #[test]
    fn the_sleep_research_makes_a_nap_less_likely() {
        // `:275` — `chance = 1 / (1 + SLEEP_LESS)`.
        let mut scene = guard();
        scene.facts.adaptability = 2;
        scene.facts.sleep_roll = 1;
        scene.facts.research.sleep_less = 3.0;
        scene.facts.sleep_chance = 0.4;
        assert!(!should_sleep(&mut scene), "1/(1+3) = 0.25, and 0.4 is above it");
        scene.facts.sleep_chance = 0.2;
        assert!(should_sleep(&mut scene));
    }

    #[test]
    fn a_nap_ends_when_something_hurts_the_guard_or_the_timer_runs_out() {
        // `:316-322`.
        // `:316` — `(sleepTimer -= getTickRate()) < 0` : strictement negatif, donc
        // 2500 n'est pas encore la fin et 5 -> 0 non plus.
        let mut scene = guard();
        scene.memory.sleep_timer = 9;
        assert_eq!(sleep_on_duty(&mut scene), None);
        assert_eq!(scene.memory.sleep_timer, 4, "the machine ticks five times a tick");
        assert_eq!(sleep_on_duty(&mut scene), Some(GuardGoal::NoTarget));
        assert!(scene.stood_up);
        assert_eq!(scene.experience, 1.0);

        let mut hurt = guard();
        hurt.memory.sleep_timer = 2000;
        hurt.facts.last_hurt_by_mob = true;
        assert_eq!(sleep_on_duty(&mut hurt), Some(GuardGoal::NoTarget));
    }

    #[test]
    fn sleeping_regenerates_half_a_point_a_tick() {
        // `:301-304`.
        let mut scene = guard();
        scene.facts.health = 10.0;
        assert_eq!(sleep_particles(&mut scene), None);
        assert_eq!(scene.facts.health, 10.5);
        let mut full = guard();
        assert_eq!(sleep_particles(&mut full), None);
        assert_eq!(full.facts.health, 20.0, "full health does not overflow");
    }

    #[test]
    fn fleeing_needs_low_health_distance_retrieval_and_the_research() {
        // `:353-361`.
        let mut scene = guard();
        scene.facts.retrieves_on_low_health = true;
        scene.facts.health = 3.0;
        scene.facts.at = Pos::new(10, 64, 0);
        scene.facts.research.retreat = 0.0;
        assert!(!should_flee(&scene), "no RETREAT, no flight");
        scene.facts.research.retreat = 1.0;
        assert!(should_flee(&scene));
        scene.facts.at = Pos::new(4, 64, 0);
        assert!(!should_flee(&scene), "too close to the hut to bother");
        scene.facts.at = Pos::new(10, 64, 0);
        scene.facts.health = 5.0;
        assert!(!should_flee(&scene), "5 of 20 is above a fifth");
    }

    #[test]
    fn a_guard_without_the_retrieve_setting_dies_at_its_post() {
        // `:355` — `shallRetrieveOnLowHealth()` guards everything.
        let mut scene = guard();
        scene.facts.health = 1.0;
        scene.facts.at = Pos::new(30, 64, 0);
        scene.facts.research.retreat = 5.0;
        assert!(!should_flee(&scene));
    }

    #[test]
    fn regenerating_stops_at_three_quarters_health_and_a_nearby_enemy_preempts_it() {
        // `:368-385`.
        let mut scene = guard();
        scene.facts.retrieves_on_low_health = true;
        scene.facts.health = 10.0;
        assert_eq!(regen(&mut scene), Some(GuardGoal::GuardRegen));
        assert!(scene.regenerating);
        scene.facts.has_regeneration = true;
        assert_eq!(regen(&mut scene), Some(GuardGoal::GuardRegen));
        assert!(scene.regenerating, "the flag is not cleared");

        let mut healed = guard();
        healed.facts.retrieves_on_low_health = true;
        healed.facts.health = 19.0;
        assert_eq!(healed.facts.health, 19.0);
        assert_eq!(
            regen(&mut healed),
            Some(GuardGoal::start_working()),
            "above 75% the guard goes back to work"
        );

        let mut attacked = guard();
        attacked.facts.threat_within_10 = true;
        attacked.facts.health = 2.0;
        attacked.facts.retrieves_on_low_health = true;
        assert_eq!(regen(&mut attacked), Some(GuardGoal::Attacking));
    }

    #[test]
    fn fleeing_home_grants_the_speed_it_was_researched_with() {
        // `:392-409`.
        let mut scene = guard();
        scene.facts.research.fleeing_speed = 2.0;
        assert_eq!(flee_home(&mut scene), Some(GuardGoal::GuardFlee));
        assert_eq!(scene.speed_effect, Some(2));
        scene.facts.arrived_at_building = true;
        assert_eq!(flee_home(&mut scene), Some(GuardGoal::GuardRegen));
        let mut slow = guard();
        slow.facts.has_speed = true;
        assert_eq!(flee_home(&mut slow), Some(GuardGoal::GuardFlee));
        assert_eq!(slow.speed_effect, None);
    }

    #[test]
    fn waking_a_comrade_gives_up_after_thirty_ticks_or_when_none_is_left() {
        // `:229-261`.
        let mut scene = guard();
        assert_eq!(wake_up_guard(&mut scene), Some(GuardGoal::NoTarget));
        scene.memory.sleeping_guard = Some(Pos::new(10, 64, 0));
        assert_eq!(wake_up_guard(&mut scene), None);
        assert_eq!(scene.walked_to, Some((Pos::new(10, 64, 0), 1)));
        scene.memory.wake_timer = 31;
        assert_eq!(wake_up_guard(&mut scene), Some(GuardGoal::NoTarget));
    }

    #[test]
    fn a_comrade_within_reach_is_hit_and_left_to_wake() {
        // `:252-258`.
        let mut scene = guard();
        scene.memory.sleeping_guard = Some(Pos::new(1, 64, 0));
        assert_eq!(wake_up_guard(&mut scene), Some(GuardGoal::NoTarget));
        assert_eq!(scene.memory.sleeping_guard, None);
        assert_eq!(scene.walked_to, None);
    }

    #[test]
    fn a_guarding_soldier_shuffles_once_in_ten() {
        // `:425-431`.
        let mut scene = guard();
        scene.facts.shuffle_roll = 0;
        assert_eq!(guard_movement(&mut scene), None);
        assert_eq!(scene.walked_to, Some((scene.facts.guard_pos, 5)));
        let mut still = guard();
        still.facts.shuffle_roll = 3;
        assert_eq!(guard_movement(&mut still), None);
        assert_eq!(still.walked_to, None);
    }

    #[test]
    fn a_follower_teleports_when_it_falls_thirty_blocks_behind() {
        // `:438-448`.
        let mut scene = guard();
        scene.facts.follow_pos = Some(Pos::new(40, 64, 0));
        assert_eq!(follow(&mut scene), None);
        assert_eq!(scene.walked_to, None);
        assert_eq!(scene.memory.last_action_pos, Pos::new(40, 64, 0));

        let mut close = guard();
        close.facts.follow_pos = Some(Pos::new(10, 64, 0));
        assert_eq!(follow(&mut close), None);
        assert_eq!(close.walked_to, Some((Pos::new(10, 64, 0), 6)));
        close.facts.tight_grouping = false;
        let _ = follow(&mut close);
        assert_eq!(close.walked_to, Some((Pos::new(10, 64, 0), 15)));
    }

    #[test]
    fn a_rally_point_wins_over_the_task() {
        // `:707-710` comes before the switch (`:712-719`).
        let mut scene = guard();
        scene.facts.task = GuardTask::Guard;
        scene.facts.rally = Some(Pos::new(3, 64, 3));
        scene.facts.rally_reachable = true;
        assert_eq!(decide(&mut scene), None);
        assert_eq!(scene.walked_to, Some((Pos::new(3, 64, 3), 6)));
        assert!(scene.glowing);
    }

    #[test]
    fn an_unreachable_rally_point_falls_back_to_the_task() {
        // `:707` — `isReachableFromLocation` gates the branch.
        let mut scene = guard();
        scene.facts.task = GuardTask::Guard;
        scene.facts.rally = Some(Pos::new(3, 64, 3));
        scene.facts.rally_reachable = false;
        scene.facts.shuffle_roll = 0;
        assert_eq!(decide(&mut scene), None);
        assert_eq!(scene.walked_to, Some((scene.facts.guard_pos, 5)));
    }

    #[test]
    fn deciding_without_a_tool_prepares_and_the_fight_timer_expires() {
        // `:679-696`.
        let mut scene = guard();
        scene.facts.has_tool = false;
        assert_eq!(decide(&mut scene), Some(GuardGoal::preparing()));

        let mut fighting = guard();
        fighting.memory.fight_timer = 50;
        let _ = decide(&mut fighting);
        assert!(
            fighting.memory.fight_timer <= 0,
            "one decide tick takes a whole 100-tick interval off the 600-tick timer"
        );
        assert!(fighting.can_use_rails, "combat left, rails allowed again");
    }

    #[test]
    fn the_regular_action_counter_rerolls_on_its_twelfth_tick() {
        // `:668-672` — `regularActionTimer++ > 10` compare la valeur **avant**
        // incrementation : il faut donc 11 deja la pour remettre a zero.
        let mut scene = guard();
        scene.memory.regular_action_timer = 10;
        let _ = decide(&mut scene);
        assert_eq!(scene.memory.regular_action_timer, 11, "10 n'est pas > 10");
        let _ = decide(&mut scene);
        assert_eq!(scene.memory.regular_action_timer, 0, "11 > 10 : remise a zero");
        scene.memory.regular_action_timer = 3;
        let _ = decide(&mut scene);
        assert_eq!(scene.memory.regular_action_timer, 4);
    }

    #[test]
    fn patrolling_a_mine_without_a_mine_prepares() {
        // `:551-554`.
        let mut scene = guard();
        assert_eq!(patrol_mine(&mut scene), Some(GuardGoal::preparing()));
        scene.facts.mine_pos = Some(Pos::new(2, 64, 2));
        assert_eq!(patrol_mine(&mut scene), None);
        assert_eq!(scene.memory.current_patrol_point, Some(Pos::new(2, 64, 2)));
    }

    #[test]
    fn a_manual_patrol_only_rerolls_its_point_one_time_in_five() {
        // `:512-531`.
        let mut scene = guard();
        scene.facts.manual_target = true;
        scene.facts.shuffle_roll = 1;
        scene.facts.patrol_point = Some(Pos::new(7, 64, 7));
        assert_eq!(patrol(&mut scene), None);
        assert_eq!(scene.memory.current_patrol_point, Some(Pos::new(7, 64, 7)));
        let mut seldom = guard();
        seldom.facts.manual_target = true;
        seldom.facts.shuffle_roll = 3;
        seldom.memory.current_patrol_point = Some(Pos::new(1, 64, 1));
        seldom.facts.arrived_at_patrol = true;
        assert_eq!(patrol(&mut seldom), None);
        assert_eq!(seldom.memory.current_patrol_point, None);
    }

    #[test]
    fn an_automatic_patrol_reaches_its_point_and_delays_ten_ticks() {
        // `:534-540`.
        let mut scene = guard();
        scene.facts.patrol_point = Some(Pos::new(7, 64, 7));
        scene.facts.patrol_loaded = false;
        assert_eq!(patrol(&mut scene), None);
        assert!(scene.patrol_reached);
        assert_eq!(scene.memory.current_patrol_point, Some(Pos::new(7, 64, 7)));
    }

    #[test]
    fn the_execution_distance_follows_the_task_and_doubles_for_a_knight_at_its_post() {
        // `:757-773`.
        let mut scene = guard();
        assert_eq!(persecution_distance(&scene, false), 10);
        assert_eq!(persecution_distance(&scene, true), 30);
        scene.facts.task = GuardTask::Patrol;
        assert_eq!(persecution_distance(&scene, false), 80);
        scene.facts.task = GuardTask::Follow;
        assert_eq!(persecution_distance(&scene, true), 30);
        scene.facts.task = GuardTask::PatrolMine;
        assert_eq!(persecution_distance(&scene, false), 30);
        scene.facts.rally = Some(Pos::new(1, 64, 1));
        assert_eq!(persecution_distance(&scene, true), 30, "a rally point caps it");
    }

    #[test]
    fn the_reference_point_is_the_last_action_on_patrol_and_the_post_otherwise() {
        // `:738-750`.
        let mut scene = guard();
        scene.memory.last_action_pos = Pos::new(9, 64, 9);
        assert_eq!(task_reference_point(&scene), scene.facts.guard_pos);
        scene.facts.task = GuardTask::Patrol;
        assert_eq!(task_reference_point(&scene), Pos::new(9, 64, 9));
        scene.facts.task = GuardTask::Follow;
        scene.facts.follow_pos = Some(Pos::new(4, 64, 4));
        assert_eq!(task_reference_point(&scene), Pos::new(4, 64, 4));
    }

    #[test]
    fn the_persecution_test_squares_the_sum_of_the_two_ranges() {
        // `:728-731`.
        let mut scene = guard();
        scene.memory.last_action_pos = Pos::new(0, 64, 0);
        scene.facts.task = GuardTask::Guard;
        // Post at the origin, reach 10 + 2 = 12.
        assert!(within_persecution_distance(&scene, Pos::new(12, 64, 0), 2.0, false));
        assert!(!within_persecution_distance(&scene, Pos::new(13, 64, 0), 2.0, false));
        assert!(within_persecution_distance(&scene, Pos::new(32, 64, 0), 2.0, true));
    }

    #[test]
    fn a_guard_fighting_or_following_cannot_be_interrupted() {
        // `:776-784`.
        let mut scene = guard();
        assert!(can_be_interrupted(&scene));
        scene.memory.fight_timer = 1;
        assert!(!can_be_interrupted(&scene));
        let mut attacked = guard();
        attacked.facts.last_attacker = true;
        assert!(!can_be_interrupted(&attacked));
        let mut following = guard();
        following.facts.task = GuardTask::Follow;
        assert!(!can_be_interrupted(&following));
        let mut rallying = guard();
        rallying.facts.rally = Some(Pos::new(1, 64, 1));
        assert!(!can_be_interrupted(&rallying));
    }

    #[test]
    fn a_guarding_soldier_only_helps_within_its_execution_distance() {
        // `:643-657`.
        let mut scene = guard();
        assert!(can_help(&scene, Pos::new(5, 64, 0), 2.0, false));
        assert!(!can_help(&scene, Pos::new(40, 64, 0), 2.0, false));
        scene.facts.task = GuardTask::Patrol;
        assert!(can_help(&scene, Pos::new(40, 64, 0), 2.0, false), "80 blocks on patrol");
    }

    #[test]
    fn helping_a_comrade_wakes_the_guard_and_registers_the_fight() {
        // `:628-635` and `:652-653`.
        let mut scene = guard();
        scene.memory.sleep_timer = 900;
        scene.sat_down = true;
        assert!(help_scene(&mut scene, Pos::new(2, 64, 0), 2.0, false));
        assert!(scene.helping);
        assert!(scene.stood_up, "a sleeping guard is woken by the call");
        let mut far = guard();
        assert!(!help_scene(&mut far, Pos::new(99, 64, 0), 2.0, false));
        assert!(!far.helping);
    }

    #[test]
    fn the_dump_threshold_is_five_kills_per_building_level() {
        // `:450-454`.
        assert_eq!(actions_until_dumping(1), 5);
        assert_eq!(actions_until_dumping(5), 25);
    }

    #[test]
    fn the_guard_machine_matches_its_table_and_puts_the_abort_rules_first() {
        // `AbstractEntityAIFight.java:73-77`, `AbstractEntityAIGuard.java:168-181`,
        // then `MeleeCombatAI.java:109-110`.
        let machine = machine();
        let rules: Vec<&'static str> = machine
            .transitions()
            .iter()
            .map(|transition| transition.rule)
            .collect();
        assert_eq!(rules.len(), GUARD_TABLE.len() + 2);
        for (index, rule) in GUARD_TABLE.iter().enumerate() {
            assert_eq!(rules[index], rule.rule, "priority {}", rule.priority);
            assert_eq!(machine.transitions()[index].tick_rate, rule.tick_rate);
        }
        assert_eq!(rules[rules.len() - 2], "busy_check_target");
        assert_eq!(rules[rules.len() - 1], "busy_search_target");
        assert_eq!(machine.state(), GuardGoal::Worker(WorkerState::Idle));
    }

    #[test]
    fn a_guard_without_a_weapon_lands_on_preparing_not_in_a_trap() {
        let mut machine = machine();
        let mut scene = guard();
        scene.facts.has_tool = false;
        scene.facts.arrived_at_building = true;
        assert_eq!(
            machine.state(),
            GuardGoal::Worker(WorkerState::Idle),
            "no tick has run yet: the machine waits in IDLE"
        );
        // Un tick : IDLE -> START_WORKING (`AbstractEntityAIFight.java:74`).
        let _ = machine.tick(&mut scene);
        assert_eq!(
            machine.state(),
            GuardGoal::Worker(WorkerState::StartWorking)
        );
        // Deux : le garde **ecrase** `startWorkingAtOwnBuilding` et rend DECIDE au
        // lieu de PREPARING (`AbstractEntityAIGuard.java:487-494`).
        let _ = machine.tick(&mut scene);
        assert_eq!(machine.state(), GuardGoal::decide());
        assert_eq!(scene.temp_patrol_point, Some(scene.facts.building_at));
        // Ensuite DECIDE -> NO_TARGET -> `guard_decide`, qui renvoie PREPARING
        // parce que le garde n'a pas d'outil (`:679-682`) : pas de cul-de-sac.
        for _ in 0..4 {
            let _ = machine.tick(&mut scene);
        }
        assert_eq!(machine.state(), GuardGoal::preparing());
        assert!(machine.missing().is_none(), "PREPARING must have a rule");
    }

    #[test]
    fn the_constants_of_the_guard_keep_their_lines() {
        assert_eq!(COMBAT_TIME.v(), 600);
        assert_eq!(MAX_PATROL_DERIVATION.v(), 80);
        assert_eq!(MAX_FOLLOW_DERIVATION.v(), 30);
        assert_eq!(MAX_GUARD_DERIVATION.v(), 10);
        assert_eq!(KNIGHT_DERIVATION_BONUS.v(), 20);
        assert_eq!(PATROL_DEVIATION_RAID_POINT.v(), 1600);
        assert_eq!(WAKE_TIMER_LIMIT.v(), 30);
        assert!(NAP_TICKS.source.contains("AbstractEntityAIGuard.java:282"));
    }
}
