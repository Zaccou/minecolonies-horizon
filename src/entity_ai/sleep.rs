//! Sleeping: three states, one bed, and the trap in the middle of it.
//!
//! Java reference: `core/entity/ai/minimal/EntityAISleep.java`. The queue is a
//! chain — `checkSleep` → `WALKING_HOME` → `FIND_BED` → `SLEEPING` — and the only
//! interesting part is `findBed`, whose condition is *the opposite* of its action:
//! `new TickingTransition<>(FIND_BED, this::findBed, () -> SLEEPING, 30)` (`:94`)
//! means "**while** `findBed` says the citizen is not in bed yet, stay in
//! `FIND_BED`; this transition only produces `SLEEPING` when `findBed` answers
//! true, which is when the citizen is asleep or out of tries". A reader who assumes
//! the usual `condition = when to fire` reads it backwards, so [`find_bed`] returns
//! the condition and [`find_bed_and_try_to_sleep`] the try.
//!
//! **`byte`-level faithfulness.** `walkHome` compares `homePosition.distSqr(...)
//! <= RANGE_TO_BE_HOME` (`:120`) — a **squared** distance against `16`, i.e. four
//! blocks, not sixteen. The constant's name invites the other reading.
//!
//! **One bed per citizen, by index.** `bedList.get(index)` where `index` is the
//! citizen's position in the hut's assigned list (`:167-172`): two citizens of the
//! same house never pick the same bed by accident, and a bed that is no longer a
//! bed is **removed from the hut** (`removeBed(pos)`, `:180`).

use super::facts::{CitizenFacts, HomeFacts, RANGE_TO_BE_HOME, Tuned};
use super::goals::{SleepState, Step, SLEEP_TABLE, Rule};
use crate::ai::facts::Pos;

/// `TICK_INTERVAL` (`EntityAISleep.java:48`): particles and the position check
/// every 30 ticks while asleep.
pub const TICK_INTERVAL: Tuned = Tuned::new(30, "EntityAISleep.java:48");
/// `CHANCE` (`:53`): one in thirty-three walks home with a sound.
pub const CHANCE: Tuned = Tuned::new(33, "EntityAISleep.java:53");
/// `MAX_BED_TICKS` (`:58`): ten tries, then the citizen sleeps where it stands.
pub const MAX_BED_TICKS: Tuned = Tuned::new(10, "EntityAISleep.java:58");
/// The radius `walkToPosInBuilding(..., 12)` uses to reach the bed (`:196`).
pub const BED_WALK_RANGE: Tuned = Tuned::new(12, "EntityAISleep.java:196");
/// The distance² above which a sleeper walks back to its bed (`:229`):
/// `distSqr > 3 * 3`.
pub const BED_RADIUS_SQUARED: Tuned = Tuned::new(9, "EntityAISleep.java:229");
/// `RANGE_TO_BE_HOME` (`:120`) is compared **squared**: four blocks.
pub const HOME_RADIUS_SQUARED: Tuned = RANGE_TO_BE_HOME;

/// The sleeping AI's own fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Memory {
    /// `usedBed` (`:68`).
    pub used_bed: Option<Pos>,
    /// `bedTicks` (`:73`).
    pub bed_ticks: i32,
    /// `citizen.getCitizenSleepHandler().isAsleep()` — Java asks the handler, not a
    /// field of this class.
    pub asleep: bool,
    /// `bedTicks` was reset because the walk failed (`:217`).
    pub walk_failed: bool,
    /// `citizenData.setBedPos(BlockPos.ZERO)` after a refused sleep (`:210`).
    pub bed_cleared: bool,
}

impl Memory {
    /// `initAI()` (`:153-157`).
    pub fn reset(&mut self) {
        self.used_bed = None;
        self.bed_ticks = 0;
        self.walk_failed = false;
    }
}

/// The facts this goal reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    /// The citizen.
    pub citizen: CitizenFacts,
    /// Its home and beds.
    pub home: HomeFacts,
    /// Its memory.
    pub memory: Memory,
    /// `citizen.getCitizenData().getCitizenHappinessHandler().resetModifier(
    /// SLEPTTONIGHT)` was called (`:213`).
    pub slept_tonight_reset: bool,
    /// `homeBuilding.isInBuilding(citizen.blockPosition())` — the home building,
    /// not the work one.
    pub inside_home: bool,
    /// `getRandom().nextInt(CHANCE)` for the go-home sound (`:260`).
    pub sound_roll: i32,
    /// `EntityNavigationUtils.walkToPosInBuilding(citizen, usedBed, home, 12)`
    /// reports the citizen has arrived (`:196`).
    pub at_bed: bool,
    /// A living entity other than this citizen is sleeping in the bed
    /// (`:202-206`).
    pub someone_else_in_bed: bool,
    /// The bed the hut must unregister, because the block there is no longer a bed
    /// (`hut.getModule(BuildingModules.BED).removeBed(pos)`, `:180`).
    pub bed_to_remove: Option<Pos>,
}

/// `checkSleep` (`:103-107`): reset, then walk home.
pub fn check_sleep(scene: &mut Scene) -> Step<SleepState> {
    scene.memory.reset();
    Step::Next(SleepState::WalkingHome)
}

/// `walkHome` (`:114-133`).
///
/// Two ways of being home: inside the home **building**, or — when the citizen has
/// no home building — within `sqrt(RANGE_TO_BE_HOME)` of the position it calls
/// home. The second test is `distSqr <= 16`, so four blocks.
pub fn walk_home(scene: &mut Scene) -> Step<SleepState> {
    if scene.home.homeless() {
        let at_home = scene.citizen.at.dist_sqr(scene.home.home_pos)
            <= HOME_RADIUS_SQUARED.v();
        if at_home {
            return Step::Next(SleepState::FindBed);
        }
    } else if scene.inside_home {
        return Step::Next(SleepState::FindBed);
    }
    // `setVisibleStatus(SLEEP)` then `goHome()` (`:130-131`): the walk is the
    // caller's to perform, `MANQUANT.md` §1.
    Step::Next(SleepState::WalkingHome)
}

/// `findBed`'s **condition** (`:140-148`): `true` means "stop looking, go to
/// `SLEEPING`".
pub fn find_bed(scene: &Scene) -> bool {
    scene.memory.asleep || scene.memory.bed_ticks >= MAX_BED_TICKS.v() as i32
}

/// `findBedAndTryToSleep` (`:159-220`).
///
/// The order matters: pick the bed by index, reject a block that is no longer a
/// bed (**and unregister it**), fall back to the home position, walk to it, and only
/// then try to sleep. A failed `trySleep` clears the bed position (`:210`).
pub fn find_bed_and_try_to_sleep(scene: &mut Scene) {
    if scene.home.building.is_none() {
        return;
    }

    if scene.memory.used_bed.is_none() {
        let mut chosen = None;
        let index = scene.home.bed_index;
        if index >= 0 {
            let slot = index as usize;
            if slot < scene.home.beds.len() {
                if scene.home.bed_usable(slot) {
                    chosen = scene.home.beds.get(slot).copied();
                } else if !scene.home.bed_states_valid.get(slot).copied().unwrap_or(false) {
                    // `!state.is(BlockTags.BEDS)` → `removeBed(pos)` (`:178-182`).
                    // The removal is the hut's business, the decision is the AI's.
                    scene.bed_to_remove = scene.home.beds.get(slot).copied();
                }
            }
        }
        scene.memory.used_bed = chosen.or(Some(scene.home.home_pos));
    }

    if !scene.at_bed {
        scene.memory.bed_ticks = 0;
        scene.memory.walk_failed = true;
        return;
    }

    scene.memory.bed_ticks += 1;
    if scene.someone_else_in_bed {
        // `usedBed = homePos` (`:204`): somebody is in *my* bed, share the floor.
        scene.memory.used_bed = Some(scene.home.home_pos);
    }
    if !scene.home.try_sleep_ok {
        scene.memory.used_bed = None;
        scene.memory.bed_cleared = true;
    }
    scene.slept_tonight_reset = true;
}

/// `sleep` (`:225-243`).
pub fn sleep(scene: &mut Scene) -> Step<SleepState> {
    match scene.memory.used_bed {
        Some(bed) => {
            if bed.dist_sqr(scene.citizen.at) > BED_RADIUS_SQUARED.v() {
                return Step::Next(SleepState::WalkingHome);
            }
            // `citizen.setPose(Pose.SLEEPING)` (`:233`) is the caller's.
            Step::Stayed
        }
        None => {
            find_bed_and_try_to_sleep(scene);
            Step::Stayed
        }
    }
}

/// `goHome`'s sound roll (`:260-265`): one chance in thirty-three, and only when
/// the citizen has both a work building and a job.
pub fn plays_go_home_sound(scene: &Scene) -> bool {
    // `chance <= 1` (`:261`) *and* a work building *and* a job (`:261`).
    scene.sound_roll <= 1 && scene.citizen.job.has_job()
}

/// The rule of one state.
pub fn step(state: SleepState, scene: &mut Scene) -> Step<SleepState> {
    match state {
        SleepState::CitizenSleep => check_sleep(scene),
        SleepState::WalkingHome => walk_home(scene),
        SleepState::FindBed => {
            if find_bed(scene) {
                Step::Next(SleepState::Sleeping)
            } else {
                find_bed_and_try_to_sleep(scene);
                Step::Next(SleepState::FindBed)
            }
        }
        SleepState::Sleeping => sleep(scene),
    }
}

/// The table this module implements.
pub fn table() -> &'static [Rule<SleepState>] {
    SLEEP_TABLE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home_scene() -> Scene {
        let mut scene = Scene::default();
        scene.home.building = Some(Pos::new(0, 64, 0));
        scene.home.home_pos = Pos::new(0, 64, 0);
        scene.home.beds = vec![Pos::new(2, 64, 2), Pos::new(4, 64, 2)];
        scene.home.beds_loaded = vec![true, true];
        scene.home.bed_states_valid = vec![true, true];
        scene.home.bed_is_head = vec![true, true];
        scene.home.bed_has_headroom = vec![true, true];
        scene.home.bed_index = 1;
        scene.home.try_sleep_ok = true;
        scene
    }

    #[test]
    fn entering_sleep_clears_the_last_bed() {
        // `:103-107`, `:153-157`.
        let mut scene = home_scene();
        scene.memory.used_bed = Some(Pos::new(9, 64, 9));
        scene.memory.bed_ticks = 7;
        assert_eq!(
            step(SleepState::CitizenSleep, &mut scene),
            Step::Next(SleepState::WalkingHome)
        );
        assert_eq!(scene.memory.used_bed, None);
        assert_eq!(scene.memory.bed_ticks, 0);
    }

    #[test]
    fn being_inside_the_home_building_is_being_home() {
        // `:125-128`.
        let mut scene = home_scene();
        scene.inside_home = true;
        assert_eq!(
            step(SleepState::WalkingHome, &mut scene),
            Step::Next(SleepState::FindBed)
        );
    }

    #[test]
    fn without_a_home_building_four_blocks_are_home_and_five_are_not() {
        // `:117-124` — `distSqr <= 16`.
        let mut scene = Scene::default();
        scene.home.home_pos = Pos::new(0, 64, 0);
        scene.citizen.at = Pos::new(4, 64, 0);
        assert_eq!(
            step(SleepState::WalkingHome, &mut scene),
            Step::Next(SleepState::FindBed)
        );
        scene.citizen.at = Pos::new(5, 64, 0);
        assert_eq!(
            step(SleepState::WalkingHome, &mut scene),
            Step::Next(SleepState::WalkingHome)
        );
    }

    #[test]
    fn find_bed_is_the_opposite_of_its_name_it_answers_stop_looking() {
        // `:140-148`.
        let mut scene = home_scene();
        assert!(!find_bed(&scene), "not asleep, no tries spent");
        scene.memory.bed_ticks = 9;
        assert!(!find_bed(&scene));
        scene.memory.bed_ticks = 10;
        assert!(find_bed(&scene), "ten tries and the citizen gives up");
        let mut asleep = home_scene();
        asleep.memory.asleep = true;
        assert!(find_bed(&asleep));
    }

    #[test]
    fn the_bed_is_chosen_by_the_citizens_index_in_the_hut() {
        // `:167-172` — index 1 of two beds.
        let mut scene = home_scene();
        find_bed_and_try_to_sleep(&mut scene);
        assert_eq!(scene.memory.used_bed, Some(Pos::new(4, 64, 2)));
    }

    #[test]
    fn a_block_that_is_no_longer_a_bed_is_removed_from_the_hut() {
        // `:178-182`.
        let mut scene = home_scene();
        scene.home.bed_states_valid[1] = false;
        find_bed_and_try_to_sleep(&mut scene);
        assert_eq!(scene.bed_to_remove, Some(Pos::new(4, 64, 2)));
        assert_eq!(
            scene.memory.used_bed,
            Some(Pos::new(0, 64, 0)),
            "falls back to the home position"
        );
    }

    #[test]
    fn a_headless_bed_is_not_used_and_the_home_position_takes_over() {
        // `:184-193`: the bed must be a HEAD with room above.
        let mut scene = home_scene();
        scene.home.bed_is_head[1] = false;
        find_bed_and_try_to_sleep(&mut scene);
        assert_eq!(scene.memory.used_bed, Some(Pos::new(0, 64, 0)));
    }

    #[test]
    fn a_failed_walk_resets_the_bed_ticks_instead_of_counting_them() {
        // `:216-218`.
        let mut scene = home_scene();
        scene.at_bed = false;
        find_bed_and_try_to_sleep(&mut scene);
        assert_eq!(scene.memory.bed_ticks, 0);
        assert!(scene.memory.walk_failed);
    }

    #[test]
    fn a_bed_somebody_else_sleeps_in_sends_the_citizen_to_the_floor() {
        // `:200-206`.
        let mut scene = home_scene();
        scene.at_bed = true;
        scene.someone_else_in_bed = true;
        find_bed_and_try_to_sleep(&mut scene);
        assert_eq!(scene.memory.used_bed, Some(Pos::new(0, 64, 0)));
    }

    #[test]
    fn a_refused_sleep_clears_the_bed_and_its_position() {
        // `:208-213`.
        let mut scene = home_scene();
        scene.at_bed = true;
        scene.home.try_sleep_ok = false;
        find_bed_and_try_to_sleep(&mut scene);
        assert_eq!(scene.memory.used_bed, None);
        assert!(scene.memory.bed_cleared);
        assert!(scene.slept_tonight_reset, "the happiness modifier is reset");
    }

    #[test]
    fn sleeping_far_from_the_bed_walks_back_to_it() {
        // `:227-233`.
        let mut scene = home_scene();
        scene.memory.used_bed = Some(Pos::new(10, 64, 10));
        scene.citizen.at = Pos::new(0, 64, 0);
        assert_eq!(
            step(SleepState::Sleeping, &mut scene),
            Step::Next(SleepState::WalkingHome)
        );
        let mut close = home_scene();
        close.memory.used_bed = Some(Pos::new(2, 64, 0));
        close.citizen.at = Pos::new(0, 64, 0);
        assert_eq!(step(SleepState::Sleeping, &mut close), Step::Stayed);
    }

    #[test]
    fn without_a_bed_the_sleeper_keeps_looking() {
        // `:235-238`.
        let mut scene = home_scene();
        scene.memory.used_bed = None;
        assert_eq!(step(SleepState::Sleeping, &mut scene), Step::Stayed);
        assert!(scene.memory.used_bed.is_some(), "a bed was found on the way");
    }

    #[test]
    fn the_go_home_sound_is_one_in_thirty_three() {
        // `:260-265`.
        let mut scene = home_scene();
        scene.sound_roll = 1;
        assert!(plays_go_home_sound(&scene));
        scene.sound_roll = 2;
        assert!(!plays_go_home_sound(&scene));
        scene.sound_roll = 1;
        scene.citizen.job = crate::entity_ai::facts::JobKind::None;
        assert!(!plays_go_home_sound(&scene), "a jobless citizen has nowhere to go");
    }

    #[test]
    fn the_two_radii_are_squared_against_a_sixteen_and_a_nine() {
        // `:120` (`RANGE_TO_BE_HOME = 16`) and `:229` (`3 * 3`).
        assert_eq!(HOME_RADIUS_SQUARED.v(), 16);
        assert_eq!(BED_RADIUS_SQUARED.v(), 9);
        assert_eq!(MAX_BED_TICKS.v(), 10);
        assert_eq!(BED_WALK_RANGE.v(), 12);
        assert!(TICK_INTERVAL.source.contains("EntityAISleep.java:48"));
    }
}
