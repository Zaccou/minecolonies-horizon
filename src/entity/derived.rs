//! What the job makes of the citizen: how fast it walks, what it holds, and the
//! damage cap.
//!
//! Java reference: `CitizenConstants.java:38` (`BASE_MOVEMENT_SPEED`, `0.3`) and
//! `AbstractEntityCitizen.java:160` (the attribute), `EntityCitizen.java:149` and
//! `:1351-1354` (the `0.5` clamp), the three `onLevelUp` that buy speed
//! (`JobLumberjack.java:90`, `JobDeliveryman.java:95`, `JobHealer.java:68`),
//! `InventoryCitizen.java:40` and `:549-553` (carrying) and `EntityCitizen.java:1358`
//! (the damage cap).
//!
//! **A premise of the brief is false.** "What it can carry" is **not** derived from
//! the job: 27 slots for every citizen, plus the colony's `citizen_inv_slots`
//! research (9, 18 or 27 more, `DefaultResearchProvider.java:86`). A courier
//! carries no more than a builder. What *is* job-derived is the **tool**, and
//! `crate::jobs::Job::tools()` owns that table — [`held_tools`] only points at it.
//!
//! **Speed is job-derived for exactly three jobs, and only on a level-up**: forester
//! `(secondary / 2) * 0.003`, courier and doctor `primary * 0.003`. Every other job
//! adds nothing, so a level-99 farmer walks at the base speed.
//!
//! **The one job Java would crash on.** A courier is hired by the courier's hut and
//! by the warehouse, but `JobDeliveryman.onLevelUp` reads its skill through the
//! **courier hut's** module key, which the warehouse does not carry — the Java asks
//! a null module for a skill. [`walk_speed`] refuses with [`Refusal::NoSkill`].

use crate::citizens::Posting;
use crate::entity::data::Citizen;
use crate::jobs::employment::Skill;
use crate::jobs::{Job, Refusal as JobsRefusal, ToolKind, employ};

/// `CitizenConstants.BASE_MOVEMENT_SPEED` (`:38`), the attribute's base value.
pub const BASE_MOVEMENT_SPEED: f64 = 0.3;
/// `EntityCitizen.MAX_SPEED_FACTOR` (`:149`): what `getSpeed()` clamps to.
pub const MAX_SPEED_FACTOR: f64 = 0.5;
/// `BONUS_SPEED_PER_LEVEL`, the same `0.003` in both job classes
/// (`JobLumberjack.java:29`, `JobDeliveryman.java:55`).
pub const BONUS_SPEED_PER_LEVEL: f64 = 0.003;
/// The forester divides its secondary skill by two (`JobLumberjack.java:90`).
pub const FORESTER_SKILL_DIVISOR: f64 = 2.0;
/// `InventoryCitizen.DEFAULT_INV_SIZE` (`:40`): three rows of nine.
pub const INVENTORY_SLOTS: usize = 27;
/// The armour slots beside them (`:49`).
pub const ARMOUR_SLOTS: usize = 4;
/// The largest research grant: `CITIZEN_INV_SLOTS` levels are 9, 18, 27
/// (`DefaultResearchProvider.java:86`).
pub const MAX_RESEARCH_SLOTS: usize = 27;
/// `Math.min(damage, getMaxHealth() * 0.2f)` (`EntityCitizen.java:1358`): a citizen
/// cannot be one-shot from full health.
pub const DAMAGE_CAP_FRACTION: f32 = 0.2;

/// Why a derived value cannot be computed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// A job with no hut kind: the two skills a job levels are the **module's**
    /// declaration (`WorkerBuildingModule.java:63-79`), so the hut is needed.
    /// [`Citizen::wire_work`] answers it.
    Unwired { job: Job },
    /// The job/hut pair is not employed there — `crate::jobs`' refusal, unchanged.
    NotEmployed(JobsRefusal),
    /// The module declares no skills: the rows `crate::jobs` marks with `-` are not
    /// `WorkerBuildingModule`s, so they level nothing.
    NoSkill { job: Job },
}

impl Refusal {
    /// A stable name for logs and tests, namespaced like a registry entry so it can
    /// never be mistaken for a player-facing sentence.
    pub fn name(self) -> &'static str {
        match self {
            Self::Unwired { .. } => "entity.refusal.unwired",
            Self::NotEmployed(refusal) => refusal.name(),
            Self::NoSkill { .. } => "entity.refusal.no_skill",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

/// The walking speed in blocks per tick, in Java's order: the base attribute, the
/// `ADDITION` modifier the job adds, then the clamp in `getSpeed`.
pub fn walk_speed(citizen: &Citizen) -> Result<f64, Refusal> {
    Ok((BASE_MOVEMENT_SPEED + speed_bonus(citizen)?).min(MAX_SPEED_FACTOR))
}

/// The job's additive bonus alone, `0.0` for the 47 jobs that pay none.
pub fn speed_bonus(citizen: &Citizen) -> Result<f64, Refusal> {
    let Posting { job, hut } = citizen.posting;
    let Some(job) = job else {
        return Ok(0.0);
    };
    let level = |skill: Skill| f64::from(citizen.skills.level(skill));
    match job {
        // `getModule(FORESTER_WORK).getSecondarySkill() / 2.0` (`JobLumberjack.java:90`).
        Job::Lumberjack => {
            let (_, secondary) = skills_of(hut, job)?;
            Ok((level(secondary) / FORESTER_SKILL_DIVISOR) * BONUS_SPEED_PER_LEVEL)
        }
        // `JobDeliveryman.java:95` and `JobHealer.java:68`: the primary, no division.
        Job::Deliveryman | Job::Healer => {
            let (primary, _) = skills_of(hut, job)?;
            Ok(level(primary) * BONUS_SPEED_PER_LEVEL)
        }
        _ => Ok(0.0),
    }
}

/// The two skills the citizen's hut module levels for its job.
fn skills_of(hut: Option<crate::building::Kind>, job: Job) -> Result<(Skill, Skill), Refusal> {
    let Some(hut) = hut else {
        return Err(Refusal::Unwired { job });
    };
    employ(hut, job)
        .map_err(Refusal::NotEmployed)?
        .skills()
        .ok_or(Refusal::NoSkill { job })
}

/// How many stacks a citizen can carry: [`INVENTORY_SLOTS`] plus the research, and
/// **never the job**. A negative effect (a datapack can write one) adds nothing; the
/// three research levels are the cap.
pub fn carry_slots(research_slots: i32) -> usize {
    let extra = usize::try_from(research_slots)
        .unwrap_or(0)
        .min(MAX_RESEARCH_SLOTS);
    INVENTORY_SLOTS + extra
}

/// What the job states in hand — `crate::jobs`' table, the only job-derived part of
/// "carrying".
pub fn held_tools(job: Job) -> &'static [ToolKind] {
    job.tools()
}

/// The damage a citizen really takes: at most a fifth of its maximum, so twenty
/// hits from full health whatever the weapon. A heal fed through this path stays a
/// heal rather than a negative hit.
pub fn damage_taken(damage: f32, max_health: f32) -> f32 {
    if damage <= 0.0 {
        return 0.0;
    }
    damage.min(max_health * DAMAGE_CAP_FRACTION)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::Kind;
    use crate::citizens::MAX_HEALTH;

    fn citizen_with(hut: Kind, job: Job) -> Citizen {
        let mut citizen = Citizen::new(3, "Bram");
        citizen.posting = Posting::employed(hut, job);
        citizen
    }

    #[test]
    fn a_jobless_citizen_walks_at_the_base_speed() {
        let citizen = Citizen::new(3, "Bram");
        assert_eq!(walk_speed(&citizen), Ok(BASE_MOVEMENT_SPEED));
        assert_eq!(speed_bonus(&citizen), Ok(0.0));
    }

    #[test]
    fn forty_seven_jobs_pay_no_speed_and_three_do() {
        let paying = [Job::Lumberjack, Job::Deliveryman, Job::Healer];
        for job in Job::ALL.iter().copied() {
            let Some(hut) = job.huts().next() else {
                continue;
            };
            let bonus = speed_bonus(&citizen_with(hut, job)).unwrap_or(0.0);
            if paying.contains(&job) {
                assert!(bonus > 0.0, "{job:?} pays a bonus at level 1");
            } else {
                assert_eq!(bonus, 0.0, "{job:?} pays no speed");
            }
        }
    }

    #[test]
    fn the_forester_pays_half_its_secondary_skill() {
        let mut citizen = citizen_with(Kind::Lumberjack, Job::Lumberjack);
        // `forester_work` is (Strength, Focus, false): the secondary is Focus.
        citizen.skills.set(Skill::Focus, 10, 0.0).expect("a level");
        assert_eq!(speed_bonus(&citizen), Ok(5.0 * BONUS_SPEED_PER_LEVEL));
        assert_eq!(
            walk_speed(&citizen),
            Ok(BASE_MOVEMENT_SPEED + 0.015),
            "0.315"
        );
    }

    #[test]
    fn the_courier_and_the_doctor_pay_their_whole_primary_skill_and_the_clamp_bites() {
        let mut courier = citizen_with(Kind::Deliveryman, Job::Deliveryman);
        courier
            .skills
            .set(Skill::Agility, 10, 0.0)
            .expect("a level");
        assert_eq!(speed_bonus(&courier), Ok(10.0 * BONUS_SPEED_PER_LEVEL));
        let mut doctor = citizen_with(Kind::Hospital, Job::Healer);
        doctor.skills.set(Skill::Mana, 10, 0.0).expect("a level");
        assert_eq!(speed_bonus(&doctor), Ok(10.0 * BONUS_SPEED_PER_LEVEL));
        // 0.3 + 99 * 0.003 = 0.597 → clamped to 0.5.
        let mut fast = citizen_with(Kind::Deliveryman, Job::Deliveryman);
        fast.skills
            .set(Skill::Agility, crate::jobs::MAX_CITIZEN_LEVEL, 0.0)
            .expect("a level");
        assert_eq!(walk_speed(&fast), Ok(MAX_SPEED_FACTOR));
    }

    #[test]
    fn a_job_with_no_hut_kind_yet_cannot_pick_its_two_skills() {
        let mut citizen = Citizen::new(3, "Bram");
        citizen.posting = Posting {
            job: Some(Job::Lumberjack),
            hut: None,
        };
        assert_eq!(
            speed_bonus(&citizen),
            Err(Refusal::Unwired {
                job: Job::Lumberjack
            })
        );
        assert_eq!(
            Refusal::Unwired {
                job: Job::Lumberjack
            }
            .name(),
            "entity.refusal.unwired"
        );
    }

    #[test]
    fn the_warehouse_courier_is_a_named_refusal_and_not_the_java_null_module() {
        let citizen = citizen_with(Kind::Warehouse, Job::Deliveryman);
        assert_eq!(
            speed_bonus(&citizen),
            Err(Refusal::NoSkill {
                job: Job::Deliveryman
            })
        );
        assert_eq!(
            Refusal::NoSkill {
                job: Job::Deliveryman
            }
            .name(),
            "entity.refusal.no_skill"
        );
        let wrong = citizen_with(Kind::Farmer, Job::Healer);
        assert_eq!(
            speed_bonus(&wrong),
            Err(Refusal::NotEmployed(
                employ(Kind::Farmer, Job::Healer).unwrap_err()
            ))
        );
    }

    #[test]
    fn carrying_is_twenty_seven_slots_plus_the_research_and_never_the_job() {
        assert_eq!(carry_slots(0), 27, "InventoryCitizen.java:40");
        assert_eq!(carry_slots(9), 36);
        assert_eq!(carry_slots(18), 45);
        assert_eq!(carry_slots(27), 54);
        assert_eq!(carry_slots(-5), 27, "a negative effect adds nothing");
        assert_eq!(carry_slots(999), 54, "the three research levels cap it");
        assert_eq!(ARMOUR_SLOTS, 4);
    }

    #[test]
    fn the_tool_in_hand_is_the_jobs_table_and_nothing_new() {
        assert_eq!(held_tools(Job::Lumberjack), Job::Lumberjack.tools());
        assert_eq!(held_tools(Job::Farmer), &[ToolKind::Hoe]);
        assert!(held_tools(Job::Builder).is_empty());
    }

    #[test]
    fn damage_is_capped_at_a_fifth_of_max_health_and_a_heal_stays_a_heal() {
        let max = MAX_HEALTH;
        assert_eq!(
            damage_taken(100.0, max),
            max * 0.2,
            "EntityCitizen.java:1358"
        );
        assert_eq!(damage_taken(1.5, max), 1.5);
        assert_eq!(damage_taken(-4.0, max), 0.0);
        assert_eq!(damage_taken(0.0, max), 0.0);
    }
}
