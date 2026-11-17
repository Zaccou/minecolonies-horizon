//! Being sick: eight states, a bed in the hospital, and a cure that has to be
//! carried before it can be applied.
//!
//! Java reference: `core/entity/ai/minimal/EntityAISickTask.java`. Three things in
//! this class are easy to get wrong and are spelled out here:
//!
//! 1. **`checkForCure` is the gate for everything.** `applyCure` starts by calling
//!    it and bailing out if it does not answer `APPLY_CURE` (`:237-241`), and
//!    `waitForCure` calls it too (`:323-332`). The cure is not applied because the
//!    state says so; it is applied because every cure item is in the inventory.
//! 2. **`checkForCure` is registered twice** (`:125` and `:128`). The second copy is
//!    dead: the first has `() -> true` and always produces a state. The table keeps
//!    the one that runs; `tests.rs` in [`super::goals`] counts them.
//! 3. **A citizen with no disease is still this AI's business**: `isSick` accepts
//!    `isHurt()` too (`:139-140`), and `checkForCure` sends a hurt citizen to the hut
//!    when the health is at or below [`SEEK_DOCTOR_HEALTH`] (`:450-455`).
//!
//! **The 1-in-7200 self-cure** (`:334-338`) is a draw: `nextInt(60 * 60 * 2) < 1`,
//! which is 1 in 7200 checks. It is a fact here ([`Scene::self_cure_roll`]) so that
//! a test can pin both sides of it.

use super::facts::{
    AVERAGE_SATURATION, CitizenFacts, Disease, HomeFacts, HospitalFacts, SEEK_DOCTOR_HEALTH, Tuned,
};
use super::goals::{Rule, SickState, Step, SICK_TABLE};
use crate::ai::facts::Pos;

/// `MIN_DIST_TO_HUT` (`EntityAISickTask.java:51`). Declared, and read nowhere in
/// the file.
pub const MIN_DIST_TO_HUT: Tuned = Tuned::new(5, "EntityAISickTask.java:51");
/// `MIN_DIST_TO_HOSPITAL` (`:56`): the walk-to range inside the hospital.
pub const MIN_DIST_TO_HOSPITAL: Tuned = Tuned::new(3, "EntityAISickTask.java:56");
/// `MINIMUM_DISTANCE_TO_HOSPITAL` (`:61`): beyond ten blocks, walk closer.
pub const MINIMUM_DISTANCE_TO_HOSPITAL: Tuned = Tuned::new(10, "EntityAISickTask.java:61");
/// `REQUIRED_TIME_TO_CURE` (`:66`): sixty ticks of applying the cure.
pub const REQUIRED_TIME_TO_CURE: Tuned = Tuned::new(60, "EntityAISickTask.java:66");
/// `GOING_TO_BED_ATTEMPTS` (`:71`): twenty tries to lie down.
pub const GOING_TO_BED_ATTEMPTS: Tuned = Tuned::new(20, "EntityAISickTask.java:71");
/// The self-cure denominator: `nextInt(60 * 60 * 2)` (`:334`).
pub const SELF_CURE_DENOMINATOR: Tuned = Tuned::new(7200, "EntityAISickTask.java:334");

/// The sick AI's own fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Memory {
    /// `waitingTicks` (`:81`).
    pub waiting_ticks: i32,
    /// `usedBed` (`:86`).
    pub used_bed: Option<Pos>,
}

/// The facts this goal reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    /// The citizen.
    pub citizen: CitizenFacts,
    /// `getCitizenDiseaseHandler().getDisease()`, `None` when there is none.
    pub disease: Option<Disease>,
    /// The hospital and its beds.
    pub hospital: HospitalFacts,
    /// The home and work buildings (the walk to the hut is the work one).
    pub home: HomeFacts,
    /// The AI's memory.
    pub memory: Memory,
    /// `getCitizenDiseaseHandler().setSleepsAtHospital(...)` (`:199`, `:374`,
    /// `:395`, `:484`).
    pub sleeps_at_hospital: bool,
    /// `BlockPosUtil.getDistance2D(bestHospital, citizen.blockPosition())`
    /// (`:353`).
    pub distance_to_hospital_2d: f64,
    /// `citizen.getRandom().nextInt(60 * 60 * 2)` (`:334`).
    pub self_cure_roll: i32,
    /// `citizen.getRandom().nextInt(list.size())` — which cure stack is shown in
    /// hand (`:253`).
    pub cure_index_roll: i32,
    /// The interaction the last step raised, if any (`:425`, `:432`).
    pub interaction: Option<&'static str>,
    /// `EntityNavigationUtils.walkToPosInBuilding(citizen, usedBed, hospital, 3)`
    /// reports the citizen at the bed (`:212`).
    pub at_bed: bool,
    /// `((BuildingHospital) hospital).registerPatient(usedBed, citizenID)`
    /// (`:201`).
    pub patient_registered: Option<Pos>,
    /// `registerPatient(usedBed, 0)` — the slot given back (`:217`, `:298`).
    pub patient_released: Option<Pos>,
    /// `diseaseHandler.cure()` ran and the health went back to full (`:303-304`).
    pub cured: bool,
}

/// `isSick` (`:137-147`): sick **or** hurt, and the memory is reset when it is.
pub fn is_sick(scene: &mut Scene) -> bool {
    if scene.citizen.sick || scene.citizen.hurt {
        scene.memory = Memory::default();
        return true;
    }
    false
}

/// `checkForCure` (`:445-472`).
///
/// No disease: idle when the health is above the doctor threshold, the hut
/// otherwise. With a disease: `APPLY_CURE` only when **every** cure item has a slot.
pub fn check_for_cure(scene: &Scene) -> Step<SickState> {
    let disease = match &scene.disease {
        Some(disease) => disease,
        None => {
            if scene.citizen.health > SEEK_DOCTOR_HEALTH.v() as f64 {
                return Step::Next(SickState::Idle);
            }
            return Step::Next(SickState::GoToHut);
        }
    };

    if disease.cure_items.len() != scene.hospital.cure_present.len()
        || scene.hospital.cure_present.iter().any(|present| !present)
    {
        if scene.citizen.sick {
            return Step::Next(SickState::GoToHut);
        }
        return Step::Next(SickState::Idle);
    }
    Step::Next(SickState::ApplyCure)
}

/// `applyCure` (`:237-273`).
pub fn apply_cure(scene: &mut Scene) -> Step<SickState> {
    if check_for_cure(scene) != Step::Next(SickState::ApplyCure) {
        return Step::Next(SickState::CheckForCure);
    }
    if scene.disease.is_none() {
        return Step::Next(SickState::Idle);
    }
    scene.memory.waiting_ticks += 1;
    if scene.memory.waiting_ticks < REQUIRED_TIME_TO_CURE.v() as i32 {
        return Step::Next(SickState::ApplyCure);
    }
    // `cure()` (`:278-306`): the caller extracts the items, unregisters the
    // patient, clears the bed and the hand, and heals to full.
    scene.memory = Memory::default();
    scene.sleeps_at_hospital = false;
    scene.cured = true;
    Step::Next(SickState::Idle)
}

/// `waitForCure` (`:313-364`).
pub fn wait_for_cure(scene: &mut Scene) -> Step<SickState> {
    if scene.hospital.pos.is_none() {
        return Step::Next(SickState::SearchHospital);
    }
    match check_for_cure(scene) {
        Step::Next(SickState::ApplyCure) => return Step::Next(SickState::ApplyCure),
        Step::Next(SickState::Idle) => {
            scene.memory = Memory::default();
            return Step::Next(SickState::Idle);
        }
        _ => {}
    }

    // `nextInt(60 * 60 * 2) < 1`: a rare spontaneous recovery.
    if scene.self_cure_roll < 1 {
        scene.cured = true;
        scene.memory = Memory::default();
        return Step::Next(SickState::Idle);
    }

    if scene.citizen.asleep && !scene.hospital.asleep_bed_registered {
        // `getCitizenSleepHandler().onWakeUp()` (`:348`): not in *this* hospital.
        scene.citizen.asleep = false;
    }
    if !scene.citizen.asleep && scene.distance_to_hospital_2d > MINIMUM_DISTANCE_TO_HOSPITAL.v() as f64
    {
        return Step::Next(SickState::GoToHospital);
    }
    if !scene.citizen.asleep {
        return Step::Next(SickState::FindEmptyBed);
    }
    Step::Next(SickState::WaitForCure)
}

/// `findEmptyBed` (`:165-230`).
///
/// A bed is chosen only if it is a bed, **not occupied**, a head, and has an empty
/// block above it (`:194-197`) — the same four tests the sleeper makes, plus
/// occupancy, because a hospital bed is shared.
pub fn find_empty_bed(scene: &mut Scene) -> Step<SickState> {
    if scene.memory.used_bed.is_none() {
        if let Some(bed) = scene.hospital.our_bed {
            if scene.hospital.beds.contains(&bed) {
                scene.memory.used_bed = Some(bed);
            }
        }
    }

    if let Some(bed) = scene.memory.used_bed {
        if !scene.hospital.beds.contains(&bed) {
            scene.memory.used_bed = None;
        }
    }

    if scene.memory.used_bed.is_none() {
        let free = scene
            .hospital
            .beds
            .iter()
            .zip(scene.hospital.beds_free.iter())
            .find(|(_, free)| **free)
            .map(|(pos, _)| *pos);
        match free {
            Some(bed) => {
                scene.sleeps_at_hospital = true;
                scene.memory.used_bed = Some(bed);
                scene.patient_registered = Some(bed);
                return Step::Next(SickState::FindEmptyBed);
            }
            None => return Step::Next(SickState::WaitForCure),
        }
    }

    let bed = match scene.memory.used_bed {
        Some(bed) => bed,
        None => return Step::Next(SickState::WaitForCure),
    };

    if scene.at_bed {
        scene.memory.waiting_ticks += 1;
        if !scene.hospital.try_sleep_ok {
            scene.patient_released = Some(bed);
            scene.memory.used_bed = None;
        }
    }

    if scene.memory.waiting_ticks > GOING_TO_BED_ATTEMPTS.v() as i32 {
        scene.memory.waiting_ticks = 0;
        return Step::Next(SickState::WaitForCure);
    }
    Step::Next(SickState::FindEmptyBed)
}

/// `goToHut` (`:371-386`): walk to the work building, then look for a hospital.
pub fn go_to_hut(scene: &mut Scene) -> Step<SickState> {
    scene.sleeps_at_hospital = false;
    if scene.home.building.is_none() {
        return Step::Next(SickState::SearchHospital);
    }
    if scene.citizen.asleep || scene.home.arrived_at_work {
        return Step::Next(SickState::SearchHospital);
    }
    Step::Next(SickState::GoToHut)
}

/// `goToHospital` (`:393-406`).
pub fn go_to_hospital(scene: &mut Scene) -> Step<SickState> {
    scene.sleeps_at_hospital = false;
    if scene.hospital.pos.is_none() {
        return Step::Next(SickState::SearchHospital);
    }
    if scene.citizen.asleep || scene.at_hospital() {
        return Step::Next(SickState::WaitForCure);
    }
    Step::Next(SickState::SearchHospital)
}

/// `searchHospital` (`:413-438`).
///
/// **No disease and no hospital ends the AI** (`:421-424`); with a disease it is a
/// **blocking** interaction naming the disease and its cure (`:425-427`), then the
/// citizen wanders until a hospital is built.
pub fn search_hospital(scene: &mut Scene) -> Step<SickState> {
    if scene.hospital.pos.is_none() {
        if scene.disease.is_none() {
            return Step::Next(SickState::Idle);
        }
        scene.interaction = Some("com.minecolonies.coremod.gui.chat.nohospital");
        return Step::Next(SickState::Wander);
    }
    if scene.disease.is_some() {
        scene.interaction = Some("com.minecolonies.coremod.gui.chat.waitingforcure");
    }
    Step::Next(SickState::GoToHospital)
}

/// `wander` (`:154-158`).
pub fn wander(_scene: &mut Scene) -> Step<SickState> {
    Step::Next(SickState::CheckForCure)
}

/// The rule of one state.
pub fn step(state: SickState, scene: &mut Scene) -> Step<SickState> {
    match state {
        SickState::CitizenSick => {
            if is_sick(scene) {
                Step::Next(SickState::CheckForCure)
            } else {
                // `TickingTransition<>(SICK, this::isSick, ...)` (`:124`): a
                // condition that fails leaves the machine where it is.
                Step::Stayed
            }
        }
        SickState::CheckForCure => check_for_cure(scene),
        SickState::GoToHut => go_to_hut(scene),
        SickState::SearchHospital => search_hospital(scene),
        SickState::GoToHospital => go_to_hospital(scene),
        SickState::WaitForCure => wait_for_cure(scene),
        SickState::FindEmptyBed => find_empty_bed(scene),
        SickState::ApplyCure => apply_cure(scene),
        SickState::Wander => wander(scene),
        SickState::Idle => Step::Stayed,
    }
}

impl Scene {
    /// Whether the citizen is within walking range of the hospital
    /// (`walkToPos(citizen, bestHospital, MIN_DIST_TO_HOSPITAL, true)`, `:401`).
    pub fn at_hospital(&self) -> bool {
        self.distance_to_hospital_2d <= MIN_DIST_TO_HOSPITAL.v() as f64
    }
}

/// The table this module implements.
pub fn table() -> &'static [Rule<SickState>] {
    SICK_TABLE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sick_scene() -> Scene {
        let mut scene = Scene::default();
        scene.citizen.sick = true;
        scene.citizen.health = 4.0;
        scene.disease = Some(Disease::new("flu", &["minecraft:honey_bottle"]));
        scene.hospital.cure_present = vec![true];
        scene.hospital.pos = Some(Pos::new(10, 64, 10));
        scene
    }

    #[test]
    fn entering_the_sick_state_is_refused_when_the_citizen_is_neither_sick_nor_hurt() {
        // `:124`, `:137-147`.
        let mut scene = Scene::default();
        assert_eq!(step(SickState::CitizenSick, &mut scene), Step::Stayed);
        scene.citizen.hurt = true;
        assert_eq!(
            step(SickState::CitizenSick, &mut scene),
            Step::Next(SickState::CheckForCure)
        );
    }

    #[test]
    fn a_hurt_citizen_above_the_threshold_goes_back_to_idle() {
        // `:450-455` — `health > SEEK_DOCTOR_HEALTH` (6).
        let mut scene = Scene::default();
        scene.citizen.health = 6.5;
        assert_eq!(
            check_for_cure(&scene),
            Step::Next(SickState::Idle)
        );
        scene.citizen.health = 6.0;
        assert_eq!(
            check_for_cure(&scene),
            Step::Next(SickState::GoToHut),
            "exactly six is not above six"
        );
    }

    #[test]
    fn the_cure_is_applied_only_when_every_item_has_a_slot() {
        // `:457-471`.
        let mut scene = sick_scene();
        assert_eq!(
            check_for_cure(&scene),
            Step::Next(SickState::ApplyCure)
        );
        scene.hospital.cure_present = vec![false];
        assert_eq!(
            check_for_cure(&scene),
            Step::Next(SickState::GoToHut),
            "sick and missing a cure: try the hut"
        );
        scene.citizen.sick = false;
        assert_eq!(
            check_for_cure(&scene),
            Step::Next(SickState::Idle),
            "hurt-only and missing a cure: give up"
        );
    }

    #[test]
    fn a_disease_with_two_cures_needs_both() {
        // `:457-470` walks the whole list.
        let mut scene = sick_scene();
        scene.disease = Some(Disease::new(
            "plague",
            &["minecraft:honey_bottle", "minecraft:milk_bucket"],
        ));
        scene.hospital.cure_present = vec![true];
        assert_eq!(
            check_for_cure(&scene),
            Step::Next(SickState::GoToHut),
            "one present out of two is not enough"
        );
        scene.hospital.cure_present = vec![true, true];
        assert_eq!(
            check_for_cure(&scene),
            Step::Next(SickState::ApplyCure)
        );
    }

    #[test]
    fn applying_the_cure_takes_sixty_ticks() {
        // `:265-269`.
        let mut scene = sick_scene();
        for _ in 0..59 {
            assert_eq!(
                step(SickState::ApplyCure, &mut scene),
                Step::Next(SickState::ApplyCure)
            );
        }
        assert_eq!(scene.memory.waiting_ticks, 59);
        assert_eq!(step(SickState::ApplyCure, &mut scene), Step::Next(SickState::Idle));
        assert!(scene.cured);
        assert!(!scene.sleeps_at_hospital);
    }

    #[test]
    fn applying_a_cure_we_no_longer_carry_falls_back_to_the_check() {
        // `:239-242`.
        let mut scene = sick_scene();
        scene.hospital.cure_present = vec![false];
        assert_eq!(
            step(SickState::ApplyCure, &mut scene),
            Step::Next(SickState::CheckForCure)
        );
    }

    #[test]
    fn waiting_without_a_hospital_searches_again() {
        // `:315-321`.
        let mut scene = sick_scene();
        scene.hospital.pos = None;
        assert_eq!(
            step(SickState::WaitForCure, &mut scene),
            Step::Next(SickState::SearchHospital)
        );
    }

    #[test]
    fn the_one_in_seventy_two_hundred_recovery_ends_the_illness() {
        // `:334-338`.
        let mut scene = sick_scene();
        scene.hospital.cure_present = vec![false];
        scene.citizen.asleep = true;
        scene.self_cure_roll = 0;
        assert_eq!(step(SickState::WaitForCure, &mut scene), Step::Next(SickState::Idle));
        assert!(scene.cured);
        let mut unlucky = sick_scene();
        unlucky.hospital.cure_present = vec![false];
        unlucky.citizen.asleep = true;
        // Dors dans le lit *enregistre* de cet hopital (`:340-351`) : sinon Java
        // reveille le citoyen et le renvoie chercher un lit (`:356-361`).
        unlucky.hospital.asleep_bed_registered = true;
        unlucky.self_cure_roll = 1;
        assert_eq!(
            step(SickState::WaitForCure, &mut unlucky),
            Step::Next(SickState::WaitForCure)
        );
        assert!(!unlucky.cured);
    }

    #[test]
    fn a_citizen_asleep_in_the_wrong_hospital_is_woken_and_moved() {
        // `:340-356`.
        let mut scene = sick_scene();
        scene.hospital.cure_present = vec![false];
        scene.citizen.asleep = true;
        scene.hospital.asleep_bed_registered = false;
        scene.self_cure_roll = 99;
        scene.distance_to_hospital_2d = 40.0;
        assert_eq!(
            step(SickState::WaitForCure, &mut scene),
            Step::Next(SickState::GoToHospital)
        );
        assert!(!scene.citizen.asleep);
    }

    #[test]
    fn awake_and_close_the_citizen_looks_for_a_bed() {
        // `:356-361`.
        let mut scene = sick_scene();
        scene.hospital.cure_present = vec![false];
        scene.self_cure_roll = 99;
        scene.distance_to_hospital_2d = 3.0;
        assert_eq!(
            step(SickState::WaitForCure, &mut scene),
            Step::Next(SickState::FindEmptyBed)
        );
    }

    #[test]
    fn a_hospital_bed_must_be_free_and_is_then_registered() {
        // `:188-204`.
        let mut scene = sick_scene();
        scene.hospital.beds = vec![Pos::new(1, 64, 1), Pos::new(3, 64, 1)];
        scene.hospital.beds_free = vec![false, true];
        assert_eq!(
            step(SickState::FindEmptyBed, &mut scene),
            Step::Next(SickState::FindEmptyBed)
        );
        assert_eq!(scene.memory.used_bed, Some(Pos::new(3, 64, 1)));
        assert_eq!(scene.patient_registered, Some(Pos::new(3, 64, 1)));
        assert!(scene.sleeps_at_hospital);
    }

    #[test]
    fn no_free_bed_means_waiting_not_a_second_hospital() {
        // `:206-209`.
        let mut scene = sick_scene();
        scene.hospital.beds = vec![Pos::new(1, 64, 1)];
        scene.hospital.beds_free = vec![false];
        assert_eq!(
            step(SickState::FindEmptyBed, &mut scene),
            Step::Next(SickState::WaitForCure)
        );
    }

    #[test]
    fn twenty_failed_attempts_to_lie_down_send_the_citizen_back_to_waiting() {
        // `:224-228`.
        let mut scene = sick_scene();
        scene.hospital.beds = vec![Pos::new(1, 64, 1)];
        scene.hospital.beds_free = vec![true];
        scene.memory.used_bed = Some(Pos::new(1, 64, 1));
        scene.at_bed = true;
        scene.hospital.try_sleep_ok = true;
        scene.memory.waiting_ticks = 19;
        assert_eq!(
            step(SickState::FindEmptyBed, &mut scene),
            Step::Next(SickState::FindEmptyBed)
        );
        scene.memory.waiting_ticks = 20;
        assert_eq!(
            step(SickState::FindEmptyBed, &mut scene),
            Step::Next(SickState::WaitForCure)
        );
        assert_eq!(scene.memory.waiting_ticks, 0);
    }

    #[test]
    fn a_refused_sleep_releases_the_patient_slot() {
        // `:215-221`.
        let mut scene = sick_scene();
        scene.hospital.beds = vec![Pos::new(1, 64, 1)];
        scene.hospital.beds_free = vec![true];
        scene.memory.used_bed = Some(Pos::new(1, 64, 1));
        scene.at_bed = true;
        scene.hospital.try_sleep_ok = false;
        assert_eq!(
            step(SickState::FindEmptyBed, &mut scene),
            Step::Next(SickState::FindEmptyBed)
        );
        assert_eq!(scene.patient_released, Some(Pos::new(1, 64, 1)));
        assert_eq!(scene.memory.used_bed, None);
    }

    #[test]
    fn the_hut_walk_ends_when_the_citizen_arrives_or_is_asleep() {
        // `:376-385`.
        let mut scene = sick_scene();
        scene.home.building = Some(Pos::new(0, 64, 0));
        assert_eq!(
            step(SickState::GoToHut, &mut scene),
            Step::Next(SickState::GoToHut)
        );
        scene.home.arrived_at_work = true;
        assert_eq!(
            step(SickState::GoToHut, &mut scene),
            Step::Next(SickState::SearchHospital)
        );
    }

    #[test]
    fn a_sick_citizen_without_a_work_building_skips_the_hut() {
        // `:376-379`.
        let mut scene = sick_scene();
        assert_eq!(
            step(SickState::GoToHut, &mut scene),
            Step::Next(SickState::SearchHospital)
        );
    }

    #[test]
    fn no_hospital_and_a_disease_is_a_blocking_complaint_then_wandering() {
        // `:419-429`.
        let mut scene = sick_scene();
        scene.hospital.pos = None;
        assert_eq!(
            step(SickState::SearchHospital, &mut scene),
            Step::Next(SickState::Wander)
        );
        assert_eq!(
            scene.interaction,
            Some("com.minecolonies.coremod.gui.chat.nohospital")
        );
        let mut hurt_only = sick_scene();
        hurt_only.hospital.pos = None;
        hurt_only.disease = None;
        assert_eq!(
            step(SickState::SearchHospital, &mut hurt_only),
            Step::Next(SickState::Idle)
        );
        assert_eq!(hurt_only.interaction, None);
    }

    #[test]
    fn a_hospital_with_a_disease_asks_for_the_cure_by_name() {
        // `:430-435`.
        let mut scene = sick_scene();
        assert_eq!(
            step(SickState::SearchHospital, &mut scene),
            Step::Next(SickState::GoToHospital)
        );
        assert_eq!(
            scene.interaction,
            Some("com.minecolonies.coremod.gui.chat.waitingforcure")
        );
    }

    #[test]
    fn wandering_only_looks_again() {
        // `:154-158`.
        let mut scene = sick_scene();
        assert_eq!(
            step(SickState::Wander, &mut scene),
            Step::Next(SickState::CheckForCure)
        );
    }

    #[test]
    fn the_constants_of_the_sick_ai_keep_their_lines() {
        assert_eq!(REQUIRED_TIME_TO_CURE.v(), 60);
        assert_eq!(GOING_TO_BED_ATTEMPTS.v(), 20);
        assert_eq!(MINIMUM_DISTANCE_TO_HOSPITAL.v(), 10);
        assert_eq!(SELF_CURE_DENOMINATOR.v(), 7200);
        assert_eq!(AVERAGE_SATURATION.v(), 10);
        assert!(MIN_DIST_TO_HUT.source.contains("EntityAISickTask.java:51"));
    }
}
