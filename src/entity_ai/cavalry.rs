//! The mounted guard and the horse going home.
//!
//! Java references: `workers/guard/EntityAICavalry.java` (the guard looking for a
//! mount) and `cavalry/ReturnToStableGoal.java` + `cavalry/CavalryStrollGoal.java`
//! (the horse on its own).
//!
//! **A cavalryman without a horse is a worse knight, and it knows it.** `decide`
//! sends it to `FIND_MOUNT` only when the hut's task is `GUARD` (`EntityAICavalry.java:
//! 108-111`); on patrol it rides whatever it has, and `patrol` returns `null` while
//! unmounted, so a dismounted cavalryman on patrol simply stands still until a horse
//! frees up.
//!
//! **The horse obeys a delay longer than most sessions.** `LINGER_AFTER_DISMOUNT` is
//! six thousand ticks (`ReturnToStableGoal.java:47`) — five minutes — during which a
//! riderless horse will **not** walk home, so a player who hops off keeps it nearby.
//!
//! **`ReturnToStableGoal` is a real state machine with a `DONE` self-loop**
//! (`:107-108`): once home, the goal keeps answering `DONE` rather than removing
//! itself, and `canContinueToUse` is what makes it stop (`:214-217`).

use crate::ai::facts::Pos;
use crate::ai::state::{Action, Machine, Transition, ENTITY_AI_TICKRATE};
use super::facts::{CavalryFacts, Tuned};
use super::goals::{CombatState, Rule, StableState, STABLE_TABLE};

/// `GUARD_MOUNT_INTERVAL` (`EntityAICavalry.java:52`).
pub const GUARD_MOUNT_INTERVAL: Tuned = Tuned::new(50, "workers/guard/EntityAICavalry.java:52");
/// `HORSE_SEARCH_RADIUS` (`:57`): fifty blocks.
pub const HORSE_SEARCH_RADIUS: Tuned = Tuned::new(50, "workers/guard/EntityAICavalry.java:57");
/// `STABLE_REST_DISPERSION_RADIUS` (`:62`).
pub const STABLE_REST_DISPERSION_RADIUS: Tuned =
    Tuned::new(10, "workers/guard/EntityAICavalry.java:62");
/// `STABLE_REST_WANDER_RANGE` (`:67`).
pub const STABLE_REST_WANDER_RANGE: Tuned =
    Tuned::new(5, "workers/guard/EntityAICavalry.java:67");
/// `CAVALRY_PATROL_RANGE` (`GuardConstants.java:154`).
pub const CAVALRY_PATROL_RANGE: Tuned =
    Tuned::new(150, "api/util/constant/GuardConstants.java:154");
/// `DININGHALL_HORSE_PARKING_RANGE` (`JobCavalry.java:37`), read by
/// `EntityAIEatTask` (`:454`) as a **Manhattan** distance (`distManhattan`).
pub const DININGHALL_HORSE_PARKING_RANGE: Tuned =
    Tuned::new(40, "colony/jobs/guard/JobCavalry.java:37");
/// `PATH_REFRESH_TICK_RATE` (`ReturnToStableGoal.java:27`).
pub const PATH_REFRESH_TICK_RATE: Tuned =
    Tuned::new(10, "cavalry/ReturnToStableGoal.java:27");
/// `LOOKUP_TICK_RATE` (`:32`).
pub const LOOKUP_TICK_RATE: Tuned = Tuned::new(20, "cavalry/ReturnToStableGoal.java:32");
/// `LINGER_AFTER_DISMOUNT` (`:47`): six thousand ticks.
pub const LINGER_AFTER_DISMOUNT: Tuned = Tuned::new(6000, "cavalry/ReturnToStableGoal.java:47");
/// The radius `walkToPos(horse, target, 2, false, speed)` uses (`:300`, `:350`).
pub const STALL_WALK_RANGE: Tuned = Tuned::new(2, "cavalry/ReturnToStableGoal.java:300");
/// The `CavalryStrollGoal` speed (`:19`).
pub const STROLL_SPEED: f64 = 0.6;

/// The cavalry guard's own fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Memory {
    /// The horse the guard reserved, if any.
    pub mount: Option<Pos>,
    /// Whether the walk to the horse arrived (`EntityAICavalry.java:214`).
    pub arrived_at_mount: bool,
}

/// The facts the mount search reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    /// The guard, the horse and the stable.
    pub facts: CavalryFacts,
    /// What it remembers.
    pub memory: Memory,
    /// `startRiding(horse)` ran (`:233`).
    pub mounted_now: bool,
    /// The guard gave up this tick (`:226`, `:249`, `:257`).
    pub gave_up: bool,
}

/// `EntityAICavalry.decide`'s extra branch (`:102-112`).
///
/// Returns `None` for "the parent's `decide` runs", which is what Java does by
/// calling `super.decide()`.
pub fn decide(scene: &mut Scene) -> Option<CombatState> {
    if scene.facts.mounted {
        return None;
    }
    if scene.facts.may_ride {
        return Some(CombatState::FindMount);
    }
    None
}

/// `findMount` (`:163-266`).
///
/// The order of the failures matters: no horse at all is `NO_TARGET` (the guard
/// stops looking), a horse it cannot reach is `FIND_MOUNT` again (it keeps trying),
/// and a horse that is not available is skipped by the search rather than chosen.
pub fn find_mount(scene: &mut Scene) -> Option<CombatState> {
    let horse = match scene.facts.horse {
        Some(horse) => horse,
        None => {
            scene.gave_up = true;
            return Some(CombatState::NoTarget);
        }
    };
    if !scene.facts.horse_available {
        return Some(CombatState::FindMount);
    }
    scene.memory.mount = Some(horse);
    if !scene.memory.arrived_at_mount {
        return Some(CombatState::FindMount);
    }
    if scene.facts.in_stable {
        // A horse inside its stable is led out first (`:217`).
        return Some(CombatState::FindStable);
    }
    scene.mounted_now = true;
    Some(CombatState::NoTarget)
}

/// `findStable` (`:134-157`).
pub fn find_stable(scene: &mut Scene) -> Option<CombatState> {
    if scene.facts.stable.is_none() {
        return Some(CombatState::NoTarget);
    }
    if !scene.facts.arrived {
        return Some(CombatState::FindStable);
    }
    Some(CombatState::NoTarget)
}

/// `patrol` (`:271-281`): a dismounted cavalryman does not patrol.
pub fn patrol(scene: &Scene) -> bool {
    scene.facts.mounted
}

/// `getPatrolCenter` (`:287-297`): a hut-centred point, offset by
/// [`STABLE_REST_DISPERSION_RADIUS`], so a resting cavalryman does not stand in the
/// door.
pub fn patrol_center(hut: Pos, x: i32, z: i32) -> Pos {
    let radius = STABLE_REST_DISPERSION_RADIUS.v() as i32;
    Pos::new(
        hut.x + x.rem_euclid(radius.max(1)),
        hut.y,
        hut.z + z.rem_euclid(radius.max(1)),
    )
}

/// `isAvailableFor` (`:306-337`): alive, no reservation, nobody riding it.
pub fn horse_available(alive: bool, reserved: bool, has_rider: bool) -> bool {
    alive && !reserved && !has_rider
}

// ---------------------------------------------------------------------------
// The horse going home
// ---------------------------------------------------------------------------

/// The horse's own fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StableMemory {
    /// `targetStable` (`ReturnToStableGoal.java:57`).
    pub target_stable: Option<Pos>,
    /// `targetStall` (`:62`).
    pub target_stall: Option<Pos>,
    /// `reachedTarget` (`:67`).
    pub reached_target: bool,
    /// `horse.getAnimalData().setHomeBuilding(null)` (`:190`, `:198`).
    pub forgot_stable: bool,
}

/// The facts the horse reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StableScene {
    /// The horse, its stable and the clock.
    pub facts: CavalryFacts,
    /// What it remembers.
    pub memory: StableMemory,
}

/// `canStillRun` (`:255-261`): server side, no passenger, no reservation, animal
/// data present.
pub fn can_still_run(scene: &StableScene) -> bool {
    !scene.facts.has_passenger && !scene.facts.reservation
}

/// `canUse` (`:127-159`).
///
/// Four refusals: it cannot run, the horse is already in, six thousand ticks have not
/// passed since the last dismount, or the horse is already closer than the start
/// distance.
pub fn can_use(scene: &StableScene) -> bool {
    if !can_still_run(scene) || scene.facts.in_stable {
        return false;
    }
    if scene.facts.last_dismount > 0
        && scene.facts.game_time - scene.facts.last_dismount < LINGER_AFTER_DISMOUNT.v()
    {
        return false;
    }
    if !scene.facts.stable_is_stable {
        return false;
    }
    scene.facts.distance_to_stable_sqr > scene.facts.start_distance_sqr
}

/// `validateHomeStable` (`:169-206`): with no colony or no stable, the horse forgets
/// where it lives.
pub fn validate_home_stable(scene: &mut StableScene, colony_found: bool, stable_found: bool) {
    if !colony_found || !stable_found {
        scene.memory.forgot_stable = true;
        scene.facts.stable = None;
    }
}

/// `resolveStable` (`:268-285`).
pub fn resolve_stable(scene: &mut StableScene) -> bool {
    if !can_still_run(scene) {
        return true;
    }
    if !scene.facts.stable_is_stable {
        return true;
    }
    if let Some(stable) = scene.facts.stable {
        scene.memory.target_stable = Some(stable);
    }
    true
}

/// `walkToStable` (`:292-302`).
pub fn walk_to_stable(scene: &mut StableScene) -> bool {
    if !can_still_run(scene) || scene.memory.target_stable.is_none() {
        scene.memory.reached_target = true;
        return true;
    }
    scene.memory.reached_target = scene.facts.arrived;
    true
}

/// `nextAfterStableWalk` (`:309-317`).
pub fn next_after_stable_walk(scene: &StableScene) -> StableState {
    if !can_still_run(scene) {
        return if scene.facts.in_stable {
            StableState::FindStall
        } else {
            StableState::Done
        };
    }
    if scene.facts.in_stable || scene.memory.reached_target {
        StableState::FindStall
    } else {
        StableState::WalkToStable
    }
}

/// `findStall` (`:324-335`).
pub fn find_stall(scene: &mut StableScene) -> bool {
    if !scene.facts.stable_is_stable {
        return true;
    }
    scene.memory.target_stall = scene.facts.stall;
    true
}

/// `walkToStall` (`:342-352`).
pub fn walk_to_stall(scene: &mut StableScene) -> bool {
    if scene.memory.target_stall.is_none() {
        scene.memory.reached_target = true;
        return true;
    }
    scene.memory.reached_target = scene.facts.arrived;
    true
}

/// `nextAfterStallWalk` (`:359-367`).
pub fn next_after_stall_walk(scene: &StableScene) -> StableState {
    if scene.memory.target_stall.is_none() || scene.memory.reached_target {
        StableState::Done
    } else {
        StableState::WalkToStall
    }
}

/// `CavalryStrollGoal.isFreeToRoam` (`:59-72`): the same two conditions as
/// [`can_still_run`], plus a real horse.
pub fn free_to_roam(is_a_cavalry_horse: bool, has_passenger: bool, reserved: bool) -> bool {
    is_a_cavalry_horse && !has_passenger && !reserved
}

/// The horse's machine: [`STABLE_TABLE`], rule for rule.
pub fn stable_machine() -> Machine<StableState, StableScene> {
    let mut machine = Machine::new(StableState::Init, ENTITY_AI_TICKRATE);
    machine.add(Transition::target(
        StableState::Init,
        "resolve_stable",
        "cavalry/ReturnToStableGoal.java:100",
        LOOKUP_TICK_RATE.v() as u32,
        Action::Run(|scene: &mut StableScene, _: StableState| {
            resolve_stable(scene);
            Some(StableState::WalkToStable)
        }),
    ));
    machine.add(Transition::target(
        StableState::WalkToStable,
        "walk_to_stable",
        "cavalry/ReturnToStableGoal.java:102",
        PATH_REFRESH_TICK_RATE.v() as u32,
        Action::Run(|scene: &mut StableScene, _: StableState| {
            walk_to_stable(scene);
            Some(next_after_stable_walk(scene))
        }),
    ));
    machine.add(Transition::target(
        StableState::FindStall,
        "find_stall",
        "cavalry/ReturnToStableGoal.java:104",
        LOOKUP_TICK_RATE.v() as u32,
        Action::Run(|scene: &mut StableScene, _: StableState| {
            find_stall(scene);
            Some(StableState::WalkToStall)
        }),
    ));
    machine.add(Transition::target(
        StableState::WalkToStall,
        "walk_to_stall",
        "cavalry/ReturnToStableGoal.java:106",
        PATH_REFRESH_TICK_RATE.v() as u32,
        Action::Run(|scene: &mut StableScene, _: StableState| {
            walk_to_stall(scene);
            Some(next_after_stall_walk(scene))
        }),
    ));
    machine.add(Transition::target(
        StableState::Done,
        "stay_done",
        "cavalry/ReturnToStableGoal.java:108",
        LOOKUP_TICK_RATE.v() as u32,
        Action::Fixed(Some(StableState::Done)),
    ));
    machine
}

/// The table this module implements.
pub fn table() -> &'static [Rule<StableState>] {
    STABLE_TABLE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cavalry() -> Scene {
        Scene {
            facts: CavalryFacts {
                horse: Some(Pos::new(10, 64, 0)),
                horse_available: true,
                may_ride: true,
                ..CavalryFacts::default()
            },
            memory: Memory {
                arrived_at_mount: true,
                ..Memory::default()
            },
            ..Scene::default()
        }
    }

    fn horse() -> StableScene {
        StableScene {
            facts: CavalryFacts {
                in_stable: false,
                stable: Some(Pos::new(0, 64, 0)),
                stable_is_stable: true,
                distance_to_stable_sqr: 400.0,
                start_distance_sqr: 100.0,
                game_time: 10000,
                last_dismount: 0,
                stall: Some(Pos::new(1, 64, 1)),
                ..CavalryFacts::default()
            },
            ..StableScene::default()
        }
    }

    #[test]
    fn a_mounted_guard_does_nothing_this_ai_owns() {
        // `:102-111` — `super.decide()` when already riding.
        let mut scene = cavalry();
        scene.facts.mounted = true;
        assert_eq!(decide(&mut scene), None);
    }

    #[test]
    fn a_guard_that_may_not_ride_does_not_look_for_a_horse() {
        // `:108-111`.
        let mut scene = cavalry();
        scene.facts.may_ride = false;
        assert_eq!(decide(&mut scene), None);
        scene.facts.may_ride = true;
        assert_eq!(decide(&mut scene), Some(CombatState::FindMount));
    }

    #[test]
    fn no_horse_in_range_stops_the_search_entirely() {
        // `:226` — `NO_TARGET`, not `FIND_MOUNT`.
        let mut scene = cavalry();
        scene.facts.horse = None;
        assert_eq!(find_mount(&mut scene), Some(CombatState::NoTarget));
        assert!(scene.gave_up);
    }

    #[test]
    fn a_reserved_horse_sends_the_guard_back_to_the_search() {
        // `:214-217`.
        let mut scene = cavalry();
        scene.facts.horse_available = false;
        assert_eq!(find_mount(&mut scene), Some(CombatState::FindMount));
        assert!(!scene.mounted_now);
    }

    #[test]
    fn reaching_the_horse_mounts_it() {
        // `:233`.
        let mut scene = cavalry();
        assert_eq!(find_mount(&mut scene), Some(CombatState::NoTarget));
        assert!(scene.mounted_now);
        assert_eq!(scene.memory.mount, Some(Pos::new(10, 64, 0)));
    }

    #[test]
    fn a_horse_still_in_its_stable_is_fetched_from_there() {
        // `:217`.
        let mut scene = cavalry();
        scene.facts.in_stable = true;
        assert_eq!(find_mount(&mut scene), Some(CombatState::FindStable));
        assert!(!scene.mounted_now);
    }

    #[test]
    fn a_dismounted_cavalryman_does_not_patrol() {
        // `:271-281`.
        let scene = cavalry();
        assert!(!patrol(&scene));
        let mut riding = cavalry();
        riding.facts.mounted = true;
        assert!(patrol(&riding));
    }

    #[test]
    fn the_patrol_centre_is_offset_from_the_hut_but_stays_at_its_height() {
        // `:287-297`.
        let centre = patrol_center(Pos::new(100, 70, 100), 3, 7);
        assert_eq!(centre.y, 70);
        assert!(centre.x >= 100 && centre.x < 110);
        assert!(centre.z >= 100 && centre.z < 110);
    }

    #[test]
    fn a_horse_with_a_rider_or_a_reservation_is_not_available() {
        // `:306-337`.
        assert!(horse_available(true, false, false));
        assert!(!horse_available(false, false, false));
        assert!(!horse_available(true, true, false));
        assert!(!horse_available(true, false, true));
    }

    #[test]
    fn a_horse_five_minutes_after_a_dismount_does_not_walk_home() {
        // `:136-139`.
        let mut scene = horse();
        scene.facts.last_dismount = 9000;
        assert!(!can_use(&scene), "only a thousand ticks have passed");
        scene.facts.game_time = 15000;
        assert!(can_use(&scene));
    }

    #[test]
    fn a_horse_already_home_or_too_close_stays_where_it_is() {
        // `:129-154`.
        let mut home = horse();
        home.facts.in_stable = true;
        assert!(!can_use(&home));
        let mut close = horse();
        close.facts.distance_to_stable_sqr = 100.0;
        assert!(!can_use(&close));
        let mut passenger = horse();
        passenger.facts.has_passenger = true;
        assert!(!can_use(&passenger));
        let mut reserved = horse();
        reserved.facts.reservation = true;
        assert!(!can_use(&reserved));
    }

    #[test]
    fn a_horse_with_no_stable_in_its_colony_forgets_its_home() {
        // `:184-205`.
        let mut scene = horse();
        validate_home_stable(&mut scene, true, false);
        assert!(scene.memory.forgot_stable);
        assert_eq!(scene.facts.stable, None);
        let mut fine = horse();
        validate_home_stable(&mut fine, true, true);
        assert!(!fine.memory.forgot_stable);
        assert_eq!(fine.facts.stable, Some(Pos::new(0, 64, 0)));
    }

    #[test]
    fn the_stable_walk_repeats_until_the_horse_is_inside() {
        // `:309-317`.
        let mut scene = horse();
        resolve_stable(&mut scene);
        assert_eq!(scene.memory.target_stable, Some(Pos::new(0, 64, 0)));
        walk_to_stable(&mut scene);
        assert_eq!(next_after_stable_walk(&scene), StableState::WalkToStable);
        scene.facts.arrived = true;
        walk_to_stable(&mut scene);
        assert_eq!(next_after_stable_walk(&scene), StableState::FindStall);
        scene.facts.in_stable = true;
        assert_eq!(next_after_stable_walk(&scene), StableState::FindStall);
    }

    #[test]
    fn a_reached_stall_ends_the_goal_and_an_unreachable_one_repeats() {
        // `:342-367`.
        let mut scene = horse();
        find_stall(&mut scene);
        assert_eq!(scene.memory.target_stall, Some(Pos::new(1, 64, 1)));
        walk_to_stall(&mut scene);
        assert_eq!(next_after_stall_walk(&scene), StableState::WalkToStall);
        scene.facts.arrived = true;
        walk_to_stall(&mut scene);
        assert_eq!(next_after_stall_walk(&scene), StableState::Done);
        let mut stallless = horse();
        stallless.facts.stall = None;
        find_stall(&mut stallless);
        walk_to_stall(&mut stallless);
        assert_eq!(next_after_stall_walk(&stallless), StableState::Done);
    }

    #[test]
    fn a_horse_that_lost_its_passenger_mid_walk_goes_straight_home() {
        // `:311-314`.
        let mut scene = horse();
        scene.facts.in_stable = true;
        scene.facts.has_passenger = true;
        assert_eq!(next_after_stable_walk(&scene), StableState::FindStall);
        scene.facts.in_stable = false;
        assert_eq!(next_after_stable_walk(&scene), StableState::Done);
    }

    #[test]
    fn a_stroll_needs_a_real_free_cavalry_horse() {
        // `CavalryStrollGoal.java:59-72`.
        assert!(free_to_roam(true, false, false));
        assert!(!free_to_roam(false, false, false));
        assert!(!free_to_roam(true, true, false));
        assert!(!free_to_roam(true, false, true));
    }

    #[test]
    fn the_horse_machine_matches_its_table_and_ends_on_a_self_loop() {
        let machine = stable_machine();
        let rules: Vec<&'static str> = machine
            .transitions()
            .iter()
            .map(|transition| transition.rule)
            .collect();
        let expected: Vec<&'static str> = STABLE_TABLE.iter().map(|rule| rule.rule).collect();
        assert_eq!(rules, expected);
        assert_eq!(machine.state(), StableState::Init);
        assert!(machine.can_leave(StableState::Done), "DONE loops on purpose");
    }

    #[test]
    fn the_cavalry_constants_keep_their_lines() {
        assert_eq!(HORSE_SEARCH_RADIUS.v(), 50);
        assert_eq!(GUARD_MOUNT_INTERVAL.v(), 50);
        assert_eq!(LINGER_AFTER_DISMOUNT.v(), 6000);
        assert_eq!(CAVALRY_PATROL_RANGE.v(), 150);
        assert!(LINGER_AFTER_DISMOUNT.source.contains("ReturnToStableGoal.java:47"));
    }
}
