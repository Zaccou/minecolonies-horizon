//! The combat layer: how a fighter picks a target, walks into reach, and swings.
//!
//! Java references: `core/entity/ai/combat/TargetAI.java`,
//! `core/entity/ai/combat/AttackMoveAI.java`, `core/entity/ai/combat/CombatUtils.java`,
//! then the four concrete AIs of `workers/guard/` (`MeleeCombatAI`,
//! `RangeCombatAI`, `DruidCombatAI`, `CavalryCombatAI`).
//!
//! **This is a real state machine**, unlike the citizen goals: `NO_TARGET` carries
//! two rules and `ATTACKING` two more, so priority decides. [`machine`] is built
//! from [`super::goals::COMBAT_TABLE`] and `tests.rs` checks the two agree.
//!
//! **The search box is a lozenge, not a cube.** `getSearchArea` (`:188-201`) picks
//! **one of four horizontal directions** (`Direction.from3DDataValue(rand(4) + 2)`,
//! `:191`) and shapes the box as `max(range * step + 16, 16)` on one side and
//! `min(range * step - 16, -16)` on the other (`:193-198`). Because `stepX` and
//! `stepZ` are zero for the two axis-aligned directions, the box is a 24-block
//! square on one axis for half the draws and collapses to ±16 on the other — a
//! guard literally looks *sideways* for a tick. That is not a bug to fix here: it
//! is what the guard does, and [`search_box`] reproduces it exactly.
//!
//! **`searchNearbyTarget` returns `false` as soon as `skipSearch` says so**
//! (`:157-160`) — it does not `continue`. One sleeping comrade in the search box
//! therefore **aborts the whole scan**, which is how a guard ends up waking a
//! colleague instead of finding the raider behind it.
//!
//! **Twenty seconds without sight ends the chase** (`STOP_PERSECUTION_AFTER`,
//! `:28`), and so does a threat level below 5 after a failed path (`:89-94`): a
//! guard forgets what it cannot reach.

use crate::ai::facts::Pos;
use crate::ai::state::{Machine, Transition, ENTITY_AI_TICKRATE};
use super::facts::{CombatFacts, PathState, Research, Threat, Tuned};
use super::goals::{CombatState, Rule, COMBAT_TABLE};

/// `DEFAULT_VISION` (`GuardConstants.java:13`): the minimum half-width of the
/// search box.
pub const DEFAULT_VISION: Tuned = Tuned::new(16, "api/util/constant/GuardConstants.java:13");
/// `Y_VISION` (`:33`): the vertical half-height.
pub const Y_VISION: Tuned = Tuned::new(3, "api/util/constant/GuardConstants.java:33");
/// The search range `TargetAI.getSearchRange` answers (`TargetAI.java:218-221`).
pub const SEARCH_RANGE: Tuned = Tuned::new(16, "combat/TargetAI.java:220");
/// `STOP_PERSECUTION_AFTER` (`AttackMoveAI.java:28`): `20 * 60` ticks.
pub const STOP_PERSECUTION_AFTER: Tuned = Tuned::new(1200, "combat/AttackMoveAI.java:28");
/// The path attempts before giving the chase up (`:84`).
pub const MAX_PATH_ATTEMPTS: Tuned = Tuned::new(5, "combat/AttackMoveAI.java:84");
/// The threat floor under which a target is forgotten (`:90`).
pub const MIN_THREAT_TO_CHASE: Tuned = Tuned::new(5, "combat/AttackMoveAI.java:90");
/// The default reach `getAttackDistance` answers (`:195`).
pub const DEFAULT_ATTACK_DISTANCE: Tuned = Tuned::new(5, "combat/AttackMoveAI.java:195");
/// The default swing delay `getAttackDelay` answers (`:205`).
pub const DEFAULT_ATTACK_DELAY: Tuned = Tuned::new(40, "combat/AttackMoveAI.java:205");
/// `HALF_ROTATION` (`Constants.java:57`): the full turn `lookAt` asks for.
pub const HALF_ROTATION: Tuned = Tuned::new(180, "api/util/constant/Constants.java:57");
/// `MAX_DISTANCE_FOR_ATTACK` (`GuardConstants.java:164`): a knight's reach.
pub const MAX_DISTANCE_FOR_ATTACK: Tuned =
    Tuned::new(2, "api/util/constant/GuardConstants.java:164");
/// `BASE_DISTANCE_FOR_RANGED_ATTACK` (`:64`).
pub const BASE_DISTANCE_FOR_RANGED_ATTACK: Tuned =
    Tuned::new(10, "api/util/constant/GuardConstants.java:64");
/// `MAX_DISTANCE_FOR_RANGED_ATTACK` (`:74`).
pub const MAX_DISTANCE_FOR_RANGED_ATTACK: Tuned =
    Tuned::new(24, "api/util/constant/GuardConstants.java:74");
/// `BASE_DISTANCE_FOR_POTION_ATTACK` (`:69`).
pub const BASE_DISTANCE_FOR_POTION_ATTACK: Tuned =
    Tuned::new(3, "api/util/constant/GuardConstants.java:69");
/// `RANGER_BASE_DMG` (`:79`).
pub const RANGER_BASE_DMG: Tuned = Tuned::new(2, "api/util/constant/GuardConstants.java:79");
/// `RANGED_FLEE_SQDIST` (`:84`): a ranger backs off below `sqrt(7)` blocks.
pub const RANGED_FLEE_SQDIST: Tuned = Tuned::new(7, "api/util/constant/GuardConstants.java:84");
/// `HIT_CHANCE_DIVIDER` (`:99`).
pub const HIT_CHANCE_DIVIDER: Tuned = Tuned::new(15, "api/util/constant/GuardConstants.java:99");
/// `RANGED_ATTACK_DELAY_BASE` (`:94`).
pub const RANGED_ATTACK_DELAY_BASE: Tuned =
    Tuned::new(60, "api/util/constant/GuardConstants.java:94");
/// `KNIGHT_ATTACK_DELAY_BASE` (`:113`).
pub const KNIGHT_ATTACK_DELAY_BASE: Tuned =
    Tuned::new(32, "api/util/constant/GuardConstants.java:113");
/// `KNIGHT_ATTACK_DELAY_MIN` (`:118`).
pub const KNIGHT_ATTACK_DELAY_MIN: Tuned =
    Tuned::new(16, "api/util/constant/GuardConstants.java:118");
/// `BASE_PHYSICAL_DAMAGE` (`:173`).
pub const BASE_PHYSICAL_DAMAGE: Tuned = Tuned::new(3, "api/util/constant/GuardConstants.java:173");
/// `CAVALRY_DAMAGE_MULTIPLIER` (`:133`) — 1.00, i.e. the cavalry hits no harder.
pub const CAVALRY_DAMAGE_MULTIPLIER: f64 = 1.00;
/// `CAVALRY_RANGE_MULTIPLIER` (`:138`) — 1.20, i.e. it reaches further.
pub const CAVALRY_RANGE_MULTIPLIER: f64 = 1.20;
/// `EXP_PER_MOB_DEATH` (`:38`).
pub const EXP_PER_MOB_DEATH: Tuned = Tuned::new(15, "api/util/constant/GuardConstants.java:38");
/// `KNOCKBACK_CHANCE` (`MeleeCombatAI.java:77`): one in five whirlwinds.
pub const KNOCKBACK_CHANCE: Tuned = Tuned::new(5, "workers/guard/MeleeCombatAI.java:77");
/// `KNOCKBACK_COOLDOWN` (`:88`): `30 * 8` ticks.
pub const KNOCKBACK_COOLDOWN: Tuned = Tuned::new(240, "workers/guard/MeleeCombatAI.java:88");
/// `MIN_TIME_TO_ATTACK` (`:93`): a shield goes up eight ticks before the swing.
pub const MIN_TIME_TO_ATTACK: Tuned = Tuned::new(8, "workers/guard/MeleeCombatAI.java:93");
/// `COMBAT_SPEED` (`:98`, `RangeCombatAI.java:69`, `DruidCombatAI.java:71`).
pub const COMBAT_SPEED: f64 = 1.0;
/// `SPEED_LEVEL_BONUS` (`AbstractEntityAIFight.java:63`): one hundredth per level.
pub const SPEED_LEVEL_BONUS: f64 = 0.01;
/// The speed cap both combat AIs apply (`MeleeCombatAI.java:385`).
pub const MAX_SPEED_BONUS: f64 = 0.3;
/// `BOW_HOLDING_DELAY` (`RangeCombatAI.java:79`).
pub const BOW_HOLDING_DELAY: Tuned = Tuned::new(40, "workers/guard/RangeCombatAI.java:79");
/// `GUARD_BONUS_RANGE` (`:84`): a guarding ranger shoots ten blocks further.
pub const GUARD_BONUS_RANGE: Tuned = Tuned::new(10, "workers/guard/RangeCombatAI.java:84");
/// `FLEE_CHANCE` (`:89`).
pub const FLEE_CHANCE: Tuned = Tuned::new(3, "workers/guard/RangeCombatAI.java:89");
/// `ARROW_EXTRA_DAMAGE` (`:74`).
pub const ARROW_EXTRA_DAMAGE: f64 = 2.0;
/// The marksman's faster reload (`:263`): `0.67`.
pub const MARKSMAN_RELOAD_FACTOR: f64 = 0.67;
/// The marksman's true-damage share base (`:337`): `(50 + Adaptability / 2) / 100`.
pub const MARKSMAN_SHARE_BASE: f64 = 50.0;

/// The combat AI's own fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Memory {
    /// `target` (`TargetAI.java:35`).
    pub target: Option<Pos>,
    /// `targetPath` (`AttackMoveAI.java:33`).
    pub path: PathState,
    /// `nextAttackTime` (`:38`).
    pub next_attack_time: i64,
    /// `pathAttempts` (`:40`).
    pub path_attempts: u32,
    /// `nextTarget.setLastSeen(gameTime)` (`:71`).
    pub last_seen: i64,
}

/// The facts the combat machine reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    /// The target, the path, the timer.
    pub facts: CombatFacts,
    /// What the AI remembers between ticks.
    pub memory: Memory,
    /// The position the AI last walked to (`moveInAttackPosition`'s target).
    pub walked_to: Option<Pos>,
    /// `skipSearch(entity)` answered true — a sleeping comrade in the box.
    pub skip_search: bool,
    /// The scan was aborted by [`Scene::skip_search`].
    pub skipped: bool,
    /// `onTargetChange` ran, which for a guard also warns the other guards
    /// (`MeleeCombatAI.java:419-424`).
    pub woke_comrades: bool,
    /// `onTargetDied` ran (`TargetAI.java:56-60`).
    pub died: bool,
    /// `user.swing(MAIN_HAND)` — the caller's to perform, recorded so a test can
    /// see that the attack fired.
    pub swung: bool,
}

/// `isEntityValidTarget` (`TargetAI.java:91-104`).
///
/// Order matters: self/dead/fake/out-of-range first, then "who hurt me last", then
/// the guard's own `isAttackableTarget`.
pub fn is_entity_valid_target(facts: &CombatFacts, target: &Threat) -> bool {
    if !target.alive || target.pos == facts.at || facts.fake_player {
        return false;
    }
    if !facts.within_persecution {
        return false;
    }
    if facts.last_hurt_by_mob {
        return true;
    }
    facts.attackable
}

/// `checkForTarget` (`:54-83`).
///
/// A dead target is reported to `onTargetDied` *before* the table is read, and a
/// threat entry that is no longer valid resets the target rather than being skipped.
pub fn check_for_target(scene: &mut Scene) -> bool {
    if let Some(held) = scene.memory.target {
        let still_alive = scene
            .facts
            .threat_target
            .map(|threat| threat.alive && threat.pos == held)
            .unwrap_or(false);
        if !still_alive {
            // `onTargetDied(target)` (`:56-60`): the killer gets the experience and
            // the hut's statistics, and the AI forgets the corpse.
            scene.died = true;
            scene.memory.target = None;
        }
    }

    let next = match scene.facts.threat_target {
        Some(threat) => threat,
        None => return false,
    };
    if !is_entity_valid_target(&scene.facts, &next) {
        reset_target(scene);
        return false;
    }
    if scene.memory.target != Some(next.pos) {
        scene.memory.target = Some(next.pos);
        // `onTargetChange` (`:258-262`) sets the vanilla target and, for a guard,
        // calls the other guards of the hut (`CombatUtils.notifyGuardsOfTarget`).
        scene.woke_comrades = true;
    }
    true
}

/// `resetTarget` (`:109-128`).
pub fn reset_target(scene: &mut Scene) {
    scene.memory.target = None;
    scene.memory.path = PathState::None;
    scene.memory.path_attempts = 0;
}

/// `getSearchArea` (`:188-201`), as the two corners of the box.
///
/// `Direction.from3DDataValue(rand(4) + 2)` yields, in order, `NORTH`, `SOUTH`,
/// `WEST`, `EAST` — the four horizontal directions only, which is why `y` never
/// varies with the draw.
pub fn search_box(facts: &CombatFacts) -> (Pos, Pos) {
    let (step_x, step_z) = match facts.direction_roll.rem_euclid(4) {
        0 => (0, -1),
        1 => (0, 1),
        2 => (-1, 0),
        _ => (1, 0),
    };
    let range = SEARCH_RANGE.v() as i32;
    let vision = DEFAULT_VISION.v() as i32;
    let y_range = Y_VISION.v() as i32;

    let x1 = facts.at.x + (range * step_x + vision).max(vision);
    let x2 = facts.at.x + (range * step_x - vision).min(-vision);
    let z1 = facts.at.z + (range * step_z + vision).max(vision);
    let z2 = facts.at.z + (range * step_z - vision).min(-vision);

    let low = Pos::new(x1.min(x2), facts.at.y - y_range, z1.min(z2));
    let high = Pos::new(x1.max(x2), facts.at.y + y_range, z1.max(z2));
    (low, high)
}

/// Whether a position falls inside [`search_box`].
pub fn in_search_box(facts: &CombatFacts, pos: Pos) -> bool {
    let (low, high) = search_box(facts);
    pos.x >= low.x
        && pos.x <= high.x
        && pos.y >= low.y
        && pos.y <= high.y
        && pos.z >= low.z
        && pos.z <= high.z
}

/// `searchNearbyTarget` (`:135-170`).
///
/// The `skipSearch` early return is faithful and surprising: one sleeping comrade
/// stops the whole scan, so [`Scene::skipped`] is exposed.
pub fn search_nearby_target(scene: &mut Scene) -> bool {
    if check_for_target(scene) {
        return true;
    }
    if scene.facts.entities == 0 {
        return false;
    }
    if scene.skip_search {
        scene.skipped = true;
        return false;
    }
    match scene.facts.nearest_valid {
        Some(pos) => {
            scene.memory.target = Some(pos);
            true
        }
        None => false,
    }
}

/// `move` (`:55-107`).
pub fn move_step(scene: &mut Scene) -> Option<CombatState> {
    if !check_for_target(scene) {
        return Some(CombatState::NoTarget);
    }
    let threat = match scene.facts.threat_target {
        Some(threat) => threat,
        None => return Some(CombatState::NoTarget),
    };
    if scene.facts.line_of_sight {
        scene.memory.last_seen = scene.facts.game_time;
    } else if scene.facts.game_time - scene.memory.last_seen > STOP_PERSECUTION_AFTER.v() {
        reset_target(scene);
        return None;
    }

    if scene.facts.distance > scene.facts.attack_distance || !scene.facts.line_of_sight {
        // `targetPath` is a field of `AttackMoveAI` (`:33`), i.e. the AI's own
        // memory, not an observation: the same `PathState` is not read twice.
        let failed_path = scene.memory.path == PathState::Failed;
        if scene.memory.path_attempts > MAX_PATH_ATTEMPTS.v() as u32 || failed_path {
            scene.memory.path_attempts = 0;
            scene.memory.path = PathState::None;
            let reduced = threat.threat - 1;
            if reduced < MIN_THREAT_TO_CHASE.v() as i32 {
                reset_target(scene);
                return None;
            }
        }
        let needs_path = scene.memory.path == PathState::None
            || scene.facts.nav_done
            || re_path_needed(scene);
        if needs_path {
            scene.memory.path = PathState::InProgress;
            scene.memory.path_attempts += 1;
            scene.walked_to = scene.facts.threat_target.map(|threat| threat.pos);
        }
    }
    None
}

/// The third clause of `:97-103`: the old path ends too far from the target, so a
/// new one is asked for.
fn re_path_needed(scene: &Scene) -> bool {
    if scene.memory.path != PathState::Done {
        return false;
    }
    match scene.facts.path_target {
        Some(target) => {
            let reach = scene.facts.attack_distance * scene.facts.attack_distance - 1.0;
            (target.dist_sqr(scene.facts.at) as f64) > reach
        }
        None => false,
    }
}

/// `tryAttack` (`:133-154`).
pub fn try_attack(scene: &mut Scene) -> Option<CombatState> {
    if !check_for_target(scene) || !scene.facts.can_attack {
        return Some(CombatState::NoTarget);
    }
    if scene.memory.next_attack_time > scene.facts.game_time {
        return None;
    }
    if scene.facts.distance > scene.facts.attack_distance {
        return None;
    }
    if scene.facts.line_of_sight {
        scene.memory.path_attempts = 0;
        scene.swung = true;
        scene.memory.next_attack_time = scene.facts.game_time + scene.facts.attack_delay as i64;
    }
    None
}

/// The four rules of the combat machine.
pub fn machine() -> Machine<CombatState, Scene> {
    let mut machine = Machine::new(CombatState::NoTarget, ENTITY_AI_TICKRATE);
    machine.add(Transition::target(
        CombatState::NoTarget,
        "check_for_target",
        "combat/TargetAI.java:45",
        5,
        crate::ai::state::Action::Run(|scene: &mut Scene, _: CombatState| {
            if check_for_target(scene) {
                Some(CombatState::Attacking)
            } else {
                None
            }
        }),
    ));
    machine.add(Transition::target(
        CombatState::NoTarget,
        "search_nearby_target",
        "combat/TargetAI.java:46",
        80,
        crate::ai::state::Action::Run(|scene: &mut Scene, _: CombatState| {
            if search_nearby_target(scene) {
                Some(CombatState::Attacking)
            } else {
                None
            }
        }),
    ));
    machine.add(Transition::target(
        CombatState::Attacking,
        "try_attack",
        "combat/AttackMoveAI.java:46",
        5,
        crate::ai::state::Action::Run(|scene: &mut Scene, _: CombatState| try_attack(scene)),
    ));
    machine.add(Transition::target(
        CombatState::Attacking,
        "move",
        "combat/AttackMoveAI.java:47",
        10,
        crate::ai::state::Action::Run(|scene: &mut Scene, _: CombatState| move_step(scene)),
    ));
    machine
}

/// The table this module implements.
pub fn table() -> &'static [Rule<CombatState>] {
    COMBAT_TABLE
}

// ---------------------------------------------------------------------------
// The four concrete fighters
// ---------------------------------------------------------------------------

/// `MeleeCombatAI.getAttackDelay` (`:360-366`):
/// `max(32 - Adaptability / (huscarl ? 2 : 3), 16)`.
pub fn melee_attack_delay(adaptability: i32, huscarl: bool) -> i32 {
    let divisor = if huscarl { 2 } else { 3 };
    let reload = KNIGHT_ATTACK_DELAY_BASE.v() as i32 - adaptability / divisor;
    reload.max(KNIGHT_ATTACK_DELAY_MIN.v() as i32)
}

/// `MeleeCombatAI.getAttackDamage` (`:310-352`).
///
/// Six modifiers, applied in Java's order: weapon bonus plus
/// [`BASE_PHYSICAL_DAMAGE`], the `MELEE_DAMAGE` research, **twice the damage under
/// 20% health**, the crit that multiplies by 1.5, and the server's guard damage
/// multiplier. The crit is the caller's draw ([`crit`]).
pub fn melee_attack_damage(
    weapon_bonus: f64,
    research: &Research,
    health: f64,
    max_health: f64,
    crit: bool,
    server_multiplier: f64,
) -> f64 {
    let mut damage = weapon_bonus + BASE_PHYSICAL_DAMAGE.v() as f64;
    damage += research.melee_damage;
    if health <= max_health * 0.2 {
        damage *= 2.0;
    }
    if crit {
        damage *= 1.5;
    }
    damage * server_multiplier
}

/// `ColonyConstants.rand.nextDouble() > 1 / (1 + GUARD_CRIT)` (`:345-349`) — the
/// crit happens when the draw **exceeds** that threshold, i.e. **more often** as the
/// research grows.
pub fn crit(roll: f64, research: &Research) -> bool {
    roll > 1.0 / (1.0 + research.guard_crit)
}

/// `RangeCombatAI.getAttackDistance` (`:230-258`).
///
/// Base ten, plus one per building level, plus `Adaptability / 50 * 15`, capped at
/// 24 — and **only then** the height difference and the ten-block guard bonus are
/// added, which means a ranger can shoot further than 24 blocks from a hill.
pub fn range_attack_distance(
    building_level: i32,
    adaptability: i32,
    height_difference: f64,
    guarding: bool,
) -> f64 {
    let mut distance = BASE_DISTANCE_FOR_RANGED_ATTACK.v() as f64;
    distance += building_level as f64;
    distance += (adaptability as f64 / 50.0) * 15.0;
    distance = distance.min(MAX_DISTANCE_FOR_RANGED_ATTACK.v() as f64);
    distance += height_difference;
    if guarding {
        distance += GUARD_BONUS_RANGE.v() as f64;
    }
    distance
}

/// `RangeCombatAI.getAttackDelay` (`:260-264`): `60 * (marksman ? 0.67 : 1)`.
pub fn range_attack_delay(marksman: bool) -> i32 {
    let base = RANGED_ATTACK_DELAY_BASE.v() as f64;
    (if marksman {
        base * MARKSMAN_RELOAD_FACTOR
    } else {
        base
    }) as i32
}

/// `HIT_CHANCE_DIVIDER / (Adaptability + 1)` (`:220`).
pub fn range_hit_chance(adaptability: i32) -> f32 {
    HIT_CHANCE_DIVIDER.v() as f32 / (adaptability as f32 + 1.0)
}

/// `RangeCombatAI.getCombatMovementSpeed` (`:373-380`): agility, not adaptability.
pub fn range_movement_speed(agility: i32, building_level: i32) -> f64 {
    let mut adjustment = agility as f64 * SPEED_LEVEL_BONUS;
    adjustment += ((building_level * 2 - 1) as f64) * SPEED_LEVEL_BONUS;
    COMBAT_SPEED + adjustment.min(MAX_SPEED_BONUS)
}

/// `MeleeCombatAI.getCombatMovementSpeed` (`:380-387`): adaptability here.
pub fn melee_movement_speed(adaptability: i32, building_level: i32) -> f64 {
    let mut adjustment = adaptability as f64 * SPEED_LEVEL_BONUS;
    adjustment += ((building_level - 1) as f64) * SPEED_LEVEL_BONUS;
    COMBAT_SPEED + adjustment.min(MAX_SPEED_BONUS)
}

/// `DruidCombatAI.getAttackDistance` (`:184-200`): base three, one per building
/// level, plus the height difference. **No cap and no guard bonus**, unlike the
/// ranger.
pub fn druid_attack_distance(building_level: i32, height_difference: f64) -> f64 {
    BASE_DISTANCE_FOR_POTION_ATTACK.v() as f64 + building_level as f64 + height_difference
}

/// `DruidCombatAI.getAttackDelay` (`:178-182`): an instant potion doubles the wait.
pub fn druid_attack_delay(instant_effect: bool) -> i32 {
    let base = DEFAULT_ATTACK_DELAY.v() as i32;
    if instant_effect {
        base * 2
    } else {
        base
    }
}

/// `DruidCombatAI`'s damage, which is not a damage at all but an effect duration:
/// `time = primarySkill * 20` (`:138`), and the inaccuracy `99 / secondarySkill`
/// (`:140`).
pub fn druid_effect_duration(primary_skill: i32) -> i32 {
    primary_skill * 20
}

/// The druid's throw inaccuracy (`:140`). A zero skill divides by zero in Java;
/// here it is capped at the worst meaningful spread.
pub fn druid_inaccuracy(secondary_skill: i32) -> f32 {
    if secondary_skill <= 0 {
        return 99.0;
    }
    99.0 / secondary_skill as f32
}

/// `CavalryCombatAI.getAttackDistance` (`:52-55`): the melee reach times 1.20.
pub fn cavalry_attack_distance() -> f64 {
    MAX_DISTANCE_FOR_ATTACK.v() as f64 * CAVALRY_RANGE_MULTIPLIER
}

/// `CavalryCombatAI.getAttackDamage` (`:29-33`): the melee damage times 1.00 —
/// which is to say, unchanged. Kept as a function so the multiplier has one home.
pub fn cavalry_attack_damage(
    weapon_bonus: f64,
    research: &Research,
    health: f64,
    max_health: f64,
    crit_hit: bool,
    server_multiplier: f64,
) -> f64 {
    melee_attack_damage(
        weapon_bonus,
        research,
        health,
        max_health,
        crit_hit,
        server_multiplier,
    ) * CAVALRY_DAMAGE_MULTIPLIER
}

/// `AbstractEntityAIGuard.isAttackableTarget` (`:811-847`), minus the registry
/// lookups: a monster on the colony's hostile list is attackable, **unless** the
/// guard's hut excludes it; a player is attackable when hostile or already
/// attacking; a foreign citizen is attackable when it attacks the colony.
pub fn guard_attackable_target(monster_on_list: bool, listed_out: bool, hostile_player: bool, foreign_attacker: bool) -> bool {
    if monster_on_list && !listed_out {
        return true;
    }
    hostile_player || foreign_attacker
}

/// `CombatUtils.notifyGuardsOfTarget` (`:104-126`): the range is **squared**
/// (`PATROL_DEVIATION_RAID_POINT = 40 * 40`).
pub fn notifies_guards(distance_sqr: i64, is_raider: bool) -> bool {
    is_raider && distance_sqr < 40 * 40
}

/// The threat a guard adds when a comrade is attacked (`startHelpCitizen`, `:632`).
pub fn help_threat() -> i32 {
    20
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene() -> Scene {
        Scene {
            facts: CombatFacts {
                at: Pos::new(0, 64, 0),
                game_time: 100,
                threat_target: Some(Threat {
                    pos: Pos::new(5, 64, 0),
                    threat: 10,
                    last_seen: 100,
                    alive: true,
                }),
                target: Some(Pos::new(5, 64, 0)),
                distance: 5.0,
                line_of_sight: true,
                attack_distance: 5.0,
                attack_delay: 40,
                ..CombatFacts::default()
            },
            ..Scene::default()
        }
    }

    #[test]
    fn a_target_must_be_inside_the_persecution_distance_and_not_a_fake_player() {
        // `TargetAI.java:91-104`.
        let mut facts = CombatFacts::default();
        let alive = Threat {
            pos: Pos::new(1, 64, 0),
            threat: 1,
            last_seen: 0,
            alive: true,
        };
        let dead = Threat {
            alive: false,
            ..alive
        };
        assert!(is_entity_valid_target(&facts, &alive));
        assert!(!is_entity_valid_target(&facts, &dead), "a corpse is not a target");
        facts.fake_player = true;
        assert!(!is_entity_valid_target(&facts, &alive));
        facts.fake_player = false;
        facts.within_persecution = false;
        assert!(!is_entity_valid_target(&facts, &alive));
        facts.within_persecution = true;
        facts.attackable = false;
        assert!(!is_entity_valid_target(&facts, &alive));
        facts.last_hurt_by_mob = true;
        assert!(
            is_entity_valid_target(&facts, &alive),
            "whoever hurt me is always fair game"
        );
    }

    #[test]
    fn the_combat_machine_starts_without_a_target_and_finds_one() {
        // `TargetAI.java:45-46`.
        let mut scene = scene();
        assert!(check_for_target(&mut scene));
        assert_eq!(scene.memory.target, Some(Pos::new(5, 64, 0)));
        let mut machine = machine();
        let fired = machine.tick(&mut scene);
        assert!(fired.is_some());
        assert_eq!(fired.unwrap().rule, "check_for_target");
        assert_eq!(machine.state(), CombatState::Attacking);
    }

    #[test]
    fn a_dead_target_is_dropped_and_a_new_one_is_looked_for() {
        // `:56-61`.
        let mut scene = scene();
        scene.memory.target = Some(Pos::new(5, 64, 0));
        if let Some(threat) = scene.facts.threat_target.as_mut() {
            threat.alive = false;
        }
        assert!(!check_for_target(&mut scene));
        assert_eq!(scene.memory.target, None);
    }

    #[test]
    fn an_empty_threat_table_means_no_target_at_all() {
        // `:62-66`.
        let mut scene = scene();
        scene.facts.threat_target = None;
        assert!(!check_for_target(&mut scene));
        assert!(!search_nearby_target(&mut scene));
    }

    #[test]
    fn one_sleeping_comrade_aborts_the_whole_search() {
        // `:157-160` — `return false`, not `continue`.
        let mut scene = scene();
        scene.facts.threat_target = None;
        scene.facts.entities = 3;
        scene.facts.nearest_valid = Some(Pos::new(4, 64, 0));
        scene.skip_search = true;
        assert!(!search_nearby_target(&mut scene));
        assert!(scene.skipped);
        let mut clear = scene.clone();
        clear.skip_search = false;
        assert!(search_nearby_target(&mut clear));
    }

    #[test]
    fn the_search_box_is_shaped_by_one_of_four_directions() {
        // `:188-201`. `from3DDataValue(2..=5)` is NORTH, SOUTH, WEST, EAST.
        let mut facts = CombatFacts {
            at: Pos::new(0, 64, 0),
            direction_roll: 0,
            ..CombatFacts::default()
        };
        // NORTH: stepX = 0, stepZ = -1. The X side collapses to ±16.
        let (low, high) = search_box(&facts);
        assert_eq!((low.x, high.x), (-16, 16));
        assert_eq!((low.z, high.z), (-32, 16));
        assert_eq!((low.y, high.y), (61, 67));

        facts.direction_roll = 1; // SOUTH
        let (low, high) = search_box(&facts);
        assert_eq!((low.z, high.z), (-16, 32));

        facts.direction_roll = 2; // WEST
        let (low, high) = search_box(&facts);
        assert_eq!((low.x, high.x), (-32, 16));

        facts.direction_roll = 3; // EAST
        let (low, high) = search_box(&facts);
        assert_eq!((low.x, high.x), (-16, 32));
    }

    #[test]
    fn a_position_is_in_the_box_only_when_the_shape_covers_it() {
        let facts = CombatFacts {
            at: Pos::new(0, 64, 0),
            direction_roll: 3, // EAST: x from -16 to 32
            ..CombatFacts::default()
        };
        assert!(in_search_box(&facts, Pos::new(20, 64, 0)));
        assert!(!in_search_box(&facts, Pos::new(-20, 64, 0)));
        assert!(in_search_box(&facts, Pos::new(0, 67, 10)));
        assert!(!in_search_box(&facts, Pos::new(0, 68, 0)));
    }

    #[test]
    fn moving_at_the_target_keeps_the_last_seen_up_to_date() {
        // `:68-72`.
        let mut scene = scene();
        scene.memory.last_seen = 0;
        assert_eq!(move_step(&mut scene), None);
        assert_eq!(scene.memory.last_seen, 100);
    }

    #[test]
    fn twenty_seconds_without_sight_ends_the_chase() {
        // `:73-77`.
        let mut scene = scene();
        scene.facts.line_of_sight = false;
        // `AttackMoveAI.java:73` — `(gameTime - lastSeen) > STOP_PERSECUTION_AFTER`,
        // strictement superieur : a 1200 pile la poursuite continue.
        scene.memory.last_seen = scene.facts.game_time - STOP_PERSECUTION_AFTER.v();
        assert_eq!(move_step(&mut scene), None);
        assert_eq!(scene.memory.target, Some(Pos::new(5, 64, 0)));
        scene.memory.last_seen = scene.facts.game_time - STOP_PERSECUTION_AFTER.v() - 1;
        assert_eq!(move_step(&mut scene), None);
        assert_eq!(scene.memory.target, None);
    }

    #[test]
    fn out_of_reach_the_ai_asks_for_a_path_once() {
        // `:97-103`.
        let mut scene = scene();
        scene.facts.distance = 40.0;
        assert_eq!(move_step(&mut scene), None);
        assert_eq!(scene.memory.path, PathState::InProgress);
        assert_eq!(scene.memory.path_attempts, 1);
        assert_eq!(scene.walked_to, Some(Pos::new(5, 64, 0)));
    }

    #[test]
    fn a_path_that_failed_drops_the_threat_and_may_end_the_chase() {
        // `:84-95`.
        let mut scene = scene();
        scene.facts.distance = 40.0;
        scene.memory.path = PathState::Failed;
        if let Some(threat) = scene.facts.threat_target.as_mut() {
            threat.threat = 5;
        }
        assert_eq!(move_step(&mut scene), None);
        assert_eq!(scene.memory.target, None, "threat 4 is below the floor of 5");
    }

    #[test]
    fn a_high_threat_survives_one_failed_path() {
        // `:89-94`.
        let mut scene = scene();
        scene.facts.distance = 40.0;
        scene.memory.path = PathState::Failed;
        if let Some(threat) = scene.facts.threat_target.as_mut() {
            threat.threat = 9;
        }
        assert_eq!(move_step(&mut scene), None);
        assert_eq!(scene.memory.target, Some(Pos::new(5, 64, 0)));
        assert_eq!(scene.memory.path, PathState::InProgress);
    }

    #[test]
    fn six_path_attempts_reset_the_counter() {
        // `:84` — `pathAttempts > 5`.
        let mut scene = scene();
        scene.facts.distance = 40.0;
        scene.memory.path_attempts = 6;
        let _ = move_step(&mut scene);
        assert_eq!(scene.memory.path_attempts, 1, "reset then incremented once");
    }

    #[test]
    fn a_swing_happens_only_when_the_timer_is_up_and_the_target_is_in_reach() {
        // `:135-152`.
        let mut scene = scene();
        scene.memory.next_attack_time = 140;
        assert_eq!(try_attack(&mut scene), None);
        assert!(!scene.swung);

        let mut ready = scene.clone();
        ready.memory.next_attack_time = 100;
        assert_eq!(try_attack(&mut ready), None);
        assert!(ready.swung);
        assert_eq!(ready.memory.next_attack_time, 140);

        let mut far = scene.clone();
        far.memory.next_attack_time = 100;
        far.facts.distance = 9.0;
        assert_eq!(try_attack(&mut far), None);
        assert!(!far.swung);
    }

    #[test]
    fn a_fighter_without_a_weapon_goes_back_to_no_target() {
        // `:135-138`.
        let mut scene = scene();
        scene.facts.can_attack = false;
        assert_eq!(try_attack(&mut scene), Some(CombatState::NoTarget));
        let mut blind = scene.clone();
        blind.facts.can_attack = true;
        blind.facts.threat_target = None;
        assert_eq!(try_attack(&mut blind), Some(CombatState::NoTarget));
    }

    #[test]
    fn the_machine_alternates_attack_and_movement_in_the_order_java_registers() {
        // `AttackMoveAI.java:46-47`.
        let mut machine = machine();
        let rules: Vec<&'static str> = machine
            .transitions()
            .iter()
            .map(|transition| transition.rule)
            .collect();
        assert_eq!(
            rules,
            vec![
                "check_for_target",
                "search_nearby_target",
                "try_attack",
                "move"
            ]
        );
        assert_eq!(machine.transitions()[3].tick_rate, 10);
    }

    #[test]
    fn the_knight_swings_faster_as_adaptability_grows_and_never_below_sixteen() {
        // `MeleeCombatAI.java:360-366`.
        // `MeleeCombatAI.java:364` — `32 - Adaptability / 3`, division entiere :
        // 1 / 3 vaut 0, donc un chevalier de niveau 1 frappe toutes les 32 ticks.
        assert_eq!(melee_attack_delay(1, false), 32);
        assert_eq!(melee_attack_delay(30, false), 22);
        assert_eq!(melee_attack_delay(99, false), 16);
        assert_eq!(melee_attack_delay(30, true), 17, "a huscarl divides by two");
        assert_eq!(melee_attack_delay(99, true), 16);
    }

    #[test]
    fn a_badly_hurt_knight_hits_twice_as_hard() {
        // `MeleeCombatAI.java:340-343`.
        let research = Research::default();
        let healthy = melee_attack_damage(4.0, &research, 20.0, 20.0, false, 1.0);
        assert_eq!(healthy, 7.0, "4 + BASE_PHYSICAL_DAMAGE(3)");
        let wounded = melee_attack_damage(4.0, &research, 4.0, 20.0, false, 1.0);
        assert_eq!(wounded, 14.0);
    }

    #[test]
    fn the_crit_multiplies_by_one_and_a_half_and_grows_with_the_research() {
        // `MeleeCombatAI.java:345-349`.
        let plain = Research::default();
        assert!(!crit(0.5, &plain), "no research: 1/(1+0) = 1, never beaten");
        let mut researched = Research::default();
        researched.guard_crit = 1.0;
        assert!(crit(0.6, &researched), "1/(1+1) = 0.5, beaten by 0.6");
        assert!(!crit(0.4, &researched));
        let critted = melee_attack_damage(4.0, &plain, 20.0, 20.0, true, 1.0);
        assert_eq!(critted, 10.5);
    }

    #[test]
    fn the_server_damage_multiplier_applies_last() {
        // `MeleeCombatAI.java:351`.
        let research = Research::default();
        assert_eq!(
            melee_attack_damage(4.0, &research, 20.0, 20.0, false, 2.0),
            14.0
        );
    }

    #[test]
    fn a_ranger_reaches_ten_plus_its_hut_plus_its_skill_capped_at_twenty_four() {
        // `RangeCombatAI.java:230-258`.
        assert_eq!(range_attack_distance(0, 0, 0.0, false), 10.0);
        assert_eq!(range_attack_distance(5, 0, 0.0, false), 15.0);
        assert_eq!(range_attack_distance(5, 50, 0.0, false), 24.0);
        assert_eq!(
            range_attack_distance(5, 50, 3.0, true),
            37.0,
            "the cap is applied before the height and the guard bonus"
        );
        assert_eq!(range_attack_distance(5, 0, -2.0, false), 13.0);
    }

    #[test]
    fn a_marksman_reloads_in_two_thirds_of_the_time() {
        // `:260-264`.
        assert_eq!(range_attack_delay(false), 60);
        assert_eq!(range_attack_delay(true), 40);
    }

    #[test]
    fn the_hit_chance_falls_as_adaptability_grows() {
        // `:220`.
        assert_eq!(range_hit_chance(0), 15.0);
        assert_eq!(range_hit_chance(4), 3.0);
        assert!((range_hit_chance(99) - 0.15).abs() < 0.01);
    }

    #[test]
    fn the_two_movement_speeds_read_two_different_skills() {
        // `RangeCombatAI.java:373-380` (Agility) and `MeleeCombatAI.java:380-387`
        // (Adaptability).
        assert!((range_movement_speed(0, 1) - 1.01).abs() < 1e-9);
        assert!((melee_movement_speed(0, 1) - 1.0).abs() < 1e-9);
        assert_eq!(range_movement_speed(99, 5), 1.3, "capped");
        assert_eq!(melee_movement_speed(99, 5), 1.3, "capped");
    }

    #[test]
    fn a_druid_throws_three_blocks_plus_its_hut_and_the_height() {
        // `DruidCombatAI.java:184-200`.
        assert_eq!(druid_attack_distance(1, 0.0), 4.0);
        assert_eq!(druid_attack_distance(5, 2.0), 10.0);
        assert_eq!(druid_attack_delay(false), 40);
        assert_eq!(druid_attack_delay(true), 80);
    }

    #[test]
    fn a_druids_potion_lasts_twenty_ticks_per_primary_level() {
        // `DruidCombatAI.java:137-140`.
        assert_eq!(druid_effect_duration(1), 20);
        assert_eq!(druid_effect_duration(10), 200);
        assert!((druid_inaccuracy(3) - 33.0).abs() < 0.5);
        assert_eq!(druid_inaccuracy(0), 99.0, "a zero skill is not a division by zero");
    }

    #[test]
    fn a_cavalryman_reaches_further_but_hits_no_harder() {
        // `CavalryCombatAI.java:29-55`.
        assert!((cavalry_attack_distance() - 2.4).abs() < 1e-9);
        let research = Research::default();
        assert_eq!(
            cavalry_attack_damage(4.0, &research, 20.0, 20.0, false, 1.0),
            melee_attack_damage(4.0, &research, 20.0, 20.0, false, 1.0)
        );
    }

    #[test]
    fn a_monster_is_attackable_unless_the_hut_struck_it_off_the_list() {
        // `AbstractEntityAIGuard.java:811-847`.
        assert!(guard_attackable_target(true, false, false, false));
        assert!(!guard_attackable_target(true, true, false, false));
        assert!(guard_attackable_target(false, false, true, false));
        assert!(guard_attackable_target(false, false, false, true));
        assert!(!guard_attackable_target(false, false, false, false));
    }

    #[test]
    fn only_a_raider_within_forty_blocks_wakes_the_other_guards() {
        // `CombatUtils.java:114-125`, `AbstractEntityAIGuard.java:73`.
        assert!(notifies_guards(1599, true));
        assert!(!notifies_guards(1600, true));
        assert!(!notifies_guards(10, false), "a player is not a raider");
        assert_eq!(help_threat(), 20);
    }
}
