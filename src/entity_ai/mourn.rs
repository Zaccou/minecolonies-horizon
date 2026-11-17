//! Mourning: seven states around the town hall and the graveyard.
//!
//! Java reference: `core/entity/ai/minimal/EntityAIMournCitizen.java`. The point of
//! this goal is where the mourner stops: at the **named** grave of somebody it knew
//! (`:189-203`), which needs the grave's text lines — the only place in the whole
//! of `entity/ai/` where a block entity's text is a decision input.
//!
//! **Two constants of this class are dead.** `AVERAGE_MOURN_TIME` (`:69`) and
//! `MIN_DESTINATION_TO_LOCATION` (`:68`) — the first is never read, the second *is*
//! read but as a **squared** distance (`:316`, `getDistance2D(...) > 225`), so its
//! name and its use disagree by a square root: the mourner walks to the town hall
//! only beyond fifteen blocks, not beyond 225. Both are kept, with that note.
//!
//! **`MourningState.DECIDE` is reached from four places** and always means
//! "re-read the world": `walkToGraveyard` (`:137`), `wanderAtGraveyard` (`:165`,
//! `:208`), `walkToGrave` (`:219`, `:228`) and the entry itself.

use super::facts::{manhattan, CitizenFacts, MournFacts, Tuned};
use super::goals::{MournState, Rule, Step, MOURN_TABLE};
use crate::ai::facts::Pos;

/// `MIN_DESTINATION_TO_LOCATION` (`:68`), compared at `:316` against
/// `BlockPosUtil.getDistance2D` — which is **Manhattan** (`|dx| + |dz|`,
/// `api/util/BlockPosUtil.java:423-429`), not a Euclidean distance and not a
/// square: the mourner walks to the town hall only beyond **225 blocks of
/// Manhattan distance**.
pub const MIN_DESTINATION_TO_LOCATION: Tuned = Tuned::new(225, "EntityAIMournCitizen.java:68");
/// `AVERAGE_MOURN_TIME` (`:69`): declared as `60 * 5` and read nowhere.
pub const AVERAGE_MOURN_TIME: Tuned = Tuned::new(300, "EntityAIMournCitizen.java:69");
/// `AVERAGE_STARE_TIME` (`:70`): `nextInt(200) < 1` ends a stare.
pub const AVERAGE_STARE_TIME: Tuned = Tuned::new(200, "EntityAIMournCitizen.java:70");
/// The graveyard wander keeps walking ninety times in a hundred (`:174`).
pub const GRAVEYARD_WANDER_CHANCE: Tuned = Tuned::new(90, "EntityAIMournCitizen.java:174");
/// `walkToRandomPos(citizen, 10, ...)` for the plain wander (`:238`).
pub const WANDER_RANGE: Tuned = Tuned::new(10, "EntityAIMournCitizen.java:238");
/// The box the stare looks in (`:263`): `inflate(3.0D, 3.0D, 3.0D)`.
pub const STARE_RANGE: Tuned = Tuned::new(3, "EntityAIMournCitizen.java:263");
/// `walkToPosInBuilding(citizen, gravePos, graveyard, 3)` (`:223`).
pub const GRAVE_WALK_RANGE: Tuned = Tuned::new(3, "EntityAIMournCitizen.java:223");

/// The mourning AI's own fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Memory {
    /// `graveyard` (`:75`).
    pub graveyard: Option<Pos>,
    /// `gravePos` (`:80`).
    pub grave_pos: Option<Pos>,
    /// `closestEntity` (`:63`) — the citizen being stared at.
    pub closest: Option<Pos>,
}

impl Memory {
    /// `reset()` (`:324-329`).
    pub fn reset(&mut self) {
        *self = Memory::default();
    }
}

/// The facts this goal reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    /// The citizen.
    pub citizen: CitizenFacts,
    /// The town hall, the graveyard and the graves.
    pub mourn: MournFacts,
    /// The AI's memory.
    pub memory: Memory,
    /// `EntityNavigationUtils.walkToBuilding(citizen, building)` arrived (`:120`,
    /// `:140`).
    pub arrived_at_building: bool,
    /// `walkToPosInBuilding(citizen, gravePos, graveyard, 3)` arrived (`:223`).
    pub arrived_at_grave: bool,
    /// `citizen.getCitizenData().setVisibleStatus(null)` ran (`:326`).
    pub status_cleared: bool,
    /// The position the look control was pointed at, if it moved.
    pub looking_at: Option<Pos>,
}

/// The entry (`:94-97`): reset, then decide.
pub fn enter(scene: &mut Scene) -> Step<MournState> {
    scene.memory.reset();
    scene.status_cleared = true;
    Step::Next(MournState::Decide)
}

/// `decide` (`:285-322`).
///
/// Five branches, in this order: moving → idle; a coin → stare; a graveyard →
/// walk there; far from the town hall → walk there; otherwise wander. The coin
/// comes **before** the graveyard, so a mourner heading to a grave can be turned
/// around by a `nextBoolean()`.
pub fn decide(scene: &mut Scene) -> Step<MournState> {
    if !scene.mourn.nav_done {
        return Step::Next(MournState::Idle);
    }
    if scene.mourn.coin {
        return Step::Next(MournState::Staring);
    }
    if scene.memory.graveyard.is_none() {
        scene.memory.graveyard = scene.mourn.graveyard;
    }
    if scene.memory.graveyard.is_some() {
        return Step::Next(MournState::WalkingToGraveyard);
    }
    if let Some(location) = scene.mourn.mourn_location() {
        if manhattan(scene.citizen.at, location) > MIN_DESTINATION_TO_LOCATION.v() {
            scene.looking_at = Some(Pos::new(
                scene.citizen.at.x,
                scene.citizen.at.y - 10,
                scene.citizen.at.z,
            ));
            return Step::Next(MournState::WalkingToTownHall);
        }
    }
    Step::Next(MournState::Wandering)
}

/// `walkToTownHall` (`:112-126`).
pub fn walk_to_town_hall(scene: &mut Scene) -> Step<MournState> {
    if scene.mourn.mourn_location().is_none() {
        return Step::Next(MournState::Idle);
    }
    if !scene.arrived_at_building {
        return Step::Next(MournState::WalkingToTownHall);
    }
    Step::Next(MournState::Idle)
}

/// `wander` (`:236-240`).
pub fn wander(_scene: &mut Scene) -> Step<MournState> {
    // `EntityNavigationUtils.walkToRandomPos(citizen, 10, this.speed)` (`:238`):
    // a walk, then `CitizenAIState.IDLE`. Nothing else changes.
    let _ = WANDER_RANGE;
    Step::Next(MournState::Idle)
}

/// `stare` (`:247-278`).
///
/// One tick in two hundred ends the stare; with nobody to look at, the mourner
/// also goes idle. The coin is `nextInt(200) < 1`, so the stare almost always
/// continues once started.
pub fn stare(scene: &mut Scene) -> Step<MournState> {
    if scene.mourn.stare_roll < 1 {
        scene.memory.closest = None;
        return Step::Next(MournState::Idle);
    }
    let target = match scene.mourn.nearest_citizen {
        Some(pos) => pos,
        None => {
            scene.memory.closest = None;
            return Step::Next(MournState::Idle);
        }
    };
    scene.memory.closest = Some(target);
    scene.looking_at = Some(target);
    Step::Next(MournState::Staring)
}

/// `walkToGraveyard` (`:133-146`).
pub fn walk_to_graveyard(scene: &mut Scene) -> Step<MournState> {
    if scene.memory.graveyard.is_none() {
        return Step::Next(MournState::Decide);
    }
    if !scene.arrived_at_building {
        return Step::Next(MournState::WalkingToGraveyard);
    }
    Step::Next(MournState::WanderAtGraveyard)
}

/// `wanderAtGraveyard` (`:154-209`).
///
/// **Ninety in a hundred walks at random**, and only the remaining ten looks for a
/// grave. That is why a mourner can stand in a graveyard for minutes without
/// approaching a grave: the search is the rare branch, not the rule.
pub fn wander_at_graveyard(scene: &mut Scene) -> Step<MournState> {
    if scene.memory.graveyard.is_none() {
        return Step::Next(MournState::Decide);
    }
    if !scene.mourn.nav_done {
        return Step::Next(MournState::WanderAtGraveyard);
    }
    if scene.mourn.roll < GRAVEYARD_WANDER_CHANCE.v() as i32 {
        return Step::Next(MournState::WanderAtGraveyard);
    }
    match scene.mourn.grave_of_deceased() {
        Some(grave) => {
            scene.memory.grave_pos = Some(grave);
            Step::Next(MournState::WalkToGrave)
        }
        None => Step::Next(MournState::Decide),
    }
}

/// `walkToGrave` (`:216-229`).
pub fn walk_to_grave(scene: &mut Scene) -> Step<MournState> {
    if scene.memory.grave_pos.is_none() {
        return Step::Next(MournState::Decide);
    }
    if !scene.arrived_at_grave {
        return Step::Next(MournState::WalkToGrave);
    }
    Step::Next(MournState::Decide)
}

/// The rule of one state.
pub fn step(state: MournState, scene: &mut Scene) -> Step<MournState> {
    match state {
        MournState::CitizenMourn => enter(scene),
        MournState::Decide => decide(scene),
        MournState::WalkingToTownHall => walk_to_town_hall(scene),
        MournState::Wandering => wander(scene),
        MournState::Staring => stare(scene),
        MournState::WalkingToGraveyard => walk_to_graveyard(scene),
        MournState::WanderAtGraveyard => wander_at_graveyard(scene),
        MournState::WalkToGrave => walk_to_grave(scene),
        MournState::Idle => Step::Stayed,
    }
}

/// The table this module implements.
pub fn table() -> &'static [Rule<MournState>] {
    MOURN_TABLE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene() -> Scene {
        let mut scene = Scene::default();
        scene.mourn.town_hall = Some(Pos::new(0, 70, 0));
        scene.mourn.nav_done = true;
        scene.mourn.stare_roll = 5;
        scene.mourn.roll = 99;
        scene
    }

    #[test]
    fn entering_mourning_clears_the_graveyard_and_the_grave() {
        // `:94-97`, `:324-329`.
        let mut scene = scene();
        scene.memory.graveyard = Some(Pos::new(1, 64, 1));
        scene.memory.grave_pos = Some(Pos::new(2, 64, 2));
        assert_eq!(
            step(MournState::CitizenMourn, &mut scene),
            Step::Next(MournState::Decide)
        );
        assert_eq!(scene.memory.graveyard, None);
        assert_eq!(scene.memory.grave_pos, None);
        assert!(scene.status_cleared);
    }

    #[test]
    fn a_moving_mourner_does_not_decide() {
        // `:287-290`.
        let mut scene = scene();
        scene.mourn.nav_done = false;
        assert_eq!(
            step(MournState::Decide, &mut scene),
            Step::Next(MournState::Idle)
        );
    }

    #[test]
    fn the_coin_comes_before_the_graveyard() {
        // `:292-295` then `:297-310`.
        let mut scene = scene();
        scene.mourn.coin = true;
        scene.mourn.graveyard = Some(Pos::new(5, 64, 5));
        assert_eq!(
            step(MournState::Decide, &mut scene),
            Step::Next(MournState::Staring)
        );
        assert_eq!(scene.memory.graveyard, None, "the graveyard was not even read");
    }

    #[test]
    fn a_resting_citizen_in_a_graveyard_sends_the_mourner_there() {
        // `:297-310`.
        let mut scene = scene();
        scene.mourn.graveyard = Some(Pos::new(5, 64, 5));
        assert_eq!(
            step(MournState::Decide, &mut scene),
            Step::Next(MournState::WalkingToGraveyard)
        );
        assert_eq!(scene.memory.graveyard, Some(Pos::new(5, 64, 5)));
    }

    #[test]
    fn two_hundred_and_twenty_five_blocks_of_manhattan_distance_is_already_close() {
        // `:316` — `getDistance2D(...) > 225`, and `getDistance2D` is
        // `|dx| + |dz|` (`BlockPosUtil.java:423-429`). Two hundred and twenty-five
        // blocks, not fifteen: the constant is not a square.
        let mut scene = scene();
        scene.citizen.at = Pos::new(225, 64, 0);
        assert_eq!(
            step(MournState::Decide, &mut scene),
            Step::Next(MournState::Wandering)
        );
        scene.citizen.at = Pos::new(226, 64, 0);
        assert_eq!(
            step(MournState::Decide, &mut scene),
            Step::Next(MournState::WalkingToTownHall)
        );
        // Manhattan, not Euclidean: 200 east and 30 north is 230.
        scene.citizen.at = Pos::new(200, 64, 30);
        assert_eq!(
            step(MournState::Decide, &mut scene),
            Step::Next(MournState::WalkingToTownHall)
        );
    }

    #[test]
    fn without_a_colony_the_home_is_the_mourning_place() {
        // `:336-345`.
        let mut scene = scene();
        scene.mourn.town_hall = None;
        scene.mourn.home = Some(Pos::new(300, 64, 0));
        scene.citizen.at = Pos::new(0, 64, 0);
        assert_eq!(
            step(MournState::Decide, &mut scene),
            Step::Next(MournState::WalkingToTownHall)
        );
        assert_eq!(scene.mourn.mourn_location(), Some(Pos::new(300, 64, 0)));
    }

    #[test]
    fn with_neither_town_hall_nor_home_the_mourner_wanders() {
        // `:316-321`: `getMournLocation() != null` is the guard.
        let mut scene = scene();
        scene.mourn.town_hall = None;
        scene.mourn.home = None;
        assert_eq!(
            step(MournState::Decide, &mut scene),
            Step::Next(MournState::Wandering)
        );
    }

    #[test]
    fn walking_to_the_town_hall_ends_at_idle_not_at_wandering() {
        // `:112-126`.
        let mut scene = scene();
        assert_eq!(
            step(MournState::WalkingToTownHall, &mut scene),
            Step::Next(MournState::WalkingToTownHall)
        );
        scene.arrived_at_building = true;
        assert_eq!(
            step(MournState::WalkingToTownHall, &mut scene),
            Step::Next(MournState::Idle)
        );
        let mut homeless = scene;
        homeless.mourn.town_hall = None;
        homeless.mourn.home = None;
        assert_eq!(
            step(MournState::WalkingToTownHall, &mut homeless),
            Step::Next(MournState::Idle)
        );
    }

    #[test]
    fn a_stare_lasts_until_one_tick_in_two_hundred_ends_it() {
        // `:249-253`.
        let mut scene = scene();
        scene.mourn.nearest_citizen = Some(Pos::new(1, 64, 1));
        assert_eq!(
            step(MournState::Staring, &mut scene),
            Step::Next(MournState::Staring)
        );
        assert_eq!(scene.memory.closest, Some(Pos::new(1, 64, 1)));
        let mut no_one = scene.clone();
        no_one.mourn.nearest_citizen = None;
        assert_eq!(
            step(MournState::Staring, &mut no_one),
            Step::Next(MournState::Idle)
        );
        let mut done = scene.clone();
        done.mourn.stare_roll = 0;
        assert_eq!(
            step(MournState::Staring, &mut done),
            Step::Next(MournState::Idle)
        );
        assert_eq!(done.memory.closest, None);
    }

    #[test]
    fn the_graveyard_walk_needs_a_graveyard() {
        // `:135-138`.
        let mut scene = scene();
        assert_eq!(
            step(MournState::WalkingToGraveyard, &mut scene),
            Step::Next(MournState::Decide)
        );
        scene.memory.graveyard = Some(Pos::new(5, 64, 5));
        assert_eq!(
            step(MournState::WalkingToGraveyard, &mut scene),
            Step::Next(MournState::WalkingToGraveyard)
        );
        scene.arrived_at_building = true;
        assert_eq!(
            step(MournState::WalkingToGraveyard, &mut scene),
            Step::Next(MournState::WanderAtGraveyard)
        );
    }

    #[test]
    fn nine_times_in_ten_the_graveyard_visit_is_just_walking() {
        // `:174-178`.
        let mut scene = scene();
        scene.memory.graveyard = Some(Pos::new(5, 64, 5));
        scene.mourn.roll = 89;
        assert_eq!(
            step(MournState::WanderAtGraveyard, &mut scene),
            Step::Next(MournState::WanderAtGraveyard)
        );
        scene.mourn.roll = 90;
        scene.mourn.deceased = vec!["Jeanne Dupont"];
        scene.mourn.named_graves = vec![(Pos::new(6, 64, 6), vec!["Jeanne", "Dupont"])];
        assert_eq!(
            step(MournState::WanderAtGraveyard, &mut scene),
            Step::Next(MournState::WalkToGrave)
        );
        assert_eq!(scene.memory.grave_pos, Some(Pos::new(6, 64, 6)));
    }

    #[test]
    fn a_graveyard_visit_with_no_matching_grave_decides_again() {
        // `:208`.
        let mut scene = scene();
        scene.memory.graveyard = Some(Pos::new(5, 64, 5));
        scene.mourn.roll = 99;
        scene.mourn.deceased = vec!["Jeanne Dupont"];
        scene.mourn.named_graves = vec![(Pos::new(6, 64, 6), vec!["Pierre", "Dupont"])];
        assert_eq!(
            step(MournState::WanderAtGraveyard, &mut scene),
            Step::Next(MournState::Decide)
        );
        assert_eq!(scene.memory.grave_pos, None);
    }

    #[test]
    fn a_moving_mourner_keeps_walking_around_the_graveyard() {
        // `:168-171`.
        let mut scene = scene();
        scene.memory.graveyard = Some(Pos::new(5, 64, 5));
        scene.mourn.nav_done = false;
        assert_eq!(
            step(MournState::WanderAtGraveyard, &mut scene),
            Step::Next(MournState::WanderAtGraveyard)
        );
    }

    #[test]
    fn the_grave_walk_ends_at_decide_so_the_visit_restarts() {
        // `:216-229`.
        let mut scene = scene();
        assert_eq!(
            step(MournState::WalkToGrave, &mut scene),
            Step::Next(MournState::Decide)
        );
        scene.memory.grave_pos = Some(Pos::new(6, 64, 6));
        assert_eq!(
            step(MournState::WalkToGrave, &mut scene),
            Step::Next(MournState::WalkToGrave)
        );
        scene.arrived_at_grave = true;
        assert_eq!(
            step(MournState::WalkToGrave, &mut scene),
            Step::Next(MournState::Decide)
        );
    }

    #[test]
    fn the_dead_constants_are_recorded_rather_than_dropped() {
        assert_eq!(AVERAGE_MOURN_TIME.v(), 300);
        assert_eq!(MIN_DESTINATION_TO_LOCATION.v(), 225);
        assert_eq!(AVERAGE_STARE_TIME.v(), 200);
        assert_eq!(GRAVEYARD_WANDER_CHANCE.v(), 90);
        assert_eq!(STARE_RANGE.v(), 3);
        assert_eq!(GRAVE_WALK_RANGE.v(), 3);
        assert!(AVERAGE_MOURN_TIME.source.contains("EntityAIMournCitizen.java:69"));
    }
}
