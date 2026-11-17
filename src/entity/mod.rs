//! The citizen as data: everything `CitizenData` keeps, what the job makes of it,
//! the two ways out (death, flight), and the despawn rules that say it never
//! leaves.
//!
//! Java reference: `core/colony/CitizenData.java` (2 162 lines),
//! `api/colony/ICitizenData.java` (473), `core/entity/citizen/EntityCitizen.java`
//! (2 001), and the handlers `CitizenColonyHandler` / `CitizenSkillHandler`. Not
//! one line of `core/entity/ai/workers/` is ported on purpose.
//!
//! | File | Question |
//! | --- | --- |
//! | `data` | what a citizen is, and the compound it saves |
//! | `derived` | what the job makes of it: speed, carrying, the damage cap |
//! | `death` | what a death, a flight and a removal write — and what they do not |
//! | `worker` | what a hut records about it, and why the citizen cannot say |
//!
//! **Two modules, one citizen, and the wall between them is the finding.**
//! `crate::citizens` holds the *rules*: nine states, fourteen rungs, needs that
//! move with the tick, the happiness sum. This module holds the *record* those
//! rules act on, plus what Java spreads over three other classes. Nothing is
//! repeated: [`data::Citizen`] *has* a [`crate::citizens::Needs`] and a
//! [`crate::citizens::happiness::Happiness`] instead of copying their fields.
//! **Not the mob**: `src/citizen.rs` is the entity; this module never touches a
//! `MobRef` — no host call, no clock, no thread, no file, which is what lets every
//! rule be tested without a game.
//!
//! **Three premises of the brief are false; the code says so instead of bending.**
//!
//! 1. **A citizen's home is not in the citizen.** `serializeNBT` writes no home
//!    key (`CitizenData.java:1316-1421`); the residence hut writes the id into its
//!    own `residents` array (`LivingBuildingModule.java:91`) and the job compound
//!    keeps only a **position** (`AbstractJob.java:202`). A citizen read from its
//!    own compound is *unwired*, and [`data::Citizen::check`] allows that state.
//! 2. **What it can carry is not derived from the job**: 27 slots for everybody
//!    (`InventoryCitizen.java:40`), plus the colony's research (`:549-553`). Only
//!    the tool is job-derived — `crate::jobs` already owns that table.
//! 3. **Speed is job-derived for 3 jobs out of 50, and only on a level-up**
//!    (`JobLumberjack.java:90`, `JobDeliveryman.java:95`, `JobHealer.java:68`).
//!
//! Gaps, with what each costs, are in `MANQUANT.md`: the two missing codecs
//! (happiness, disease), the family and quest keys, the job's `actionsDone`, and
//! the per-entity tick that would call any of this on a clock.

pub mod data;
pub mod death;
pub mod derived;
pub mod worker;

pub use data::{Citizen, JobStatus, Refusal as DataRefusal, Skills};
pub use death::{
    ColonyState, Fate, Ids, Loss, Pace, Refusal as DeathRefusal, Removal, Standing, Why,
};
pub use derived::{
    BASE_MOVEMENT_SPEED, BONUS_SPEED_PER_LEVEL, DAMAGE_CAP_FRACTION, INVENTORY_SLOTS,
    MAX_SPEED_FACTOR, Refusal as DerivedRefusal, carry_slots, damage_taken, held_tools, walk_speed,
};
pub use worker::{Hired, HiringMode, Record, Refusal as WorkerRefusal};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::Kind;
    use crate::jobs::{Job, employ};

    /// One value per refusal the four sets can raise.
    fn every_refusal() -> Vec<&'static str> {
        let posting = crate::citizens::Refusal::JobWithoutHut { job: Job::Farmer };
        vec![
            DataRefusal::Field { key: "id" }.name(),
            DataRefusal::NoId.name(),
            DataRefusal::NoName.name(),
            DataRefusal::Posting(posting).name(),
            DataRefusal::WorkWithoutPosition { hut: Kind::Builder }.name(),
            DataRefusal::UnknownStatus { value: 7 }.name(),
            DerivedRefusal::Unwired { job: Job::Healer }.name(),
            DerivedRefusal::NoSkill {
                job: Job::Deliveryman,
            }
            .name(),
            DerivedRefusal::NotEmployed(employ(Kind::Farmer, Job::Healer).unwrap_err()).name(),
            DeathRefusal::NoColony { citizen: 4 }.name(),
            DeathRefusal::NotWounded { health: 20.0 }.name(),
            DeathRefusal::AlreadyHome { distance_sq: 1.0 }.name(),
            DeathRefusal::RetreatLocked { distance_sq: 100.0 }.name(),
            DeathRefusal::Persistent.name(),
            DeathRefusal::NotFleeing.name(),
            WorkerRefusal::Field {
                key: worker::KEY_RESIDENTS,
            }
            .name(),
            WorkerRefusal::UnknownMode { value: 5 }.name(),
            WorkerRefusal::BadCitizen { citizen: 0 }.name(),
            WorkerRefusal::Overfull {
                capacity: 1,
                found: 2,
            }
            .name(),
            WorkerRefusal::Full { capacity: 1 }.name(),
            WorkerRefusal::AlreadyThere { citizen: 4 }.name(),
            WorkerRefusal::Absent { citizen: 4 }.name(),
            WorkerRefusal::NoWorker.name(),
            WorkerRefusal::Unemployed.name(),
            WorkerRefusal::Unwired { job: Job::Farmer }.name(),
            WorkerRefusal::Posting(posting).name(),
        ]
    }

    #[test]
    fn every_refusal_is_namespaced_and_only_two_names_are_shared() {
        // A wrong-kind field is a wrong-kind field wherever it is read, and an
        // unwired citizen is unwired whether a speed or a hire asked: those two
        // names are shared **on purpose**. Every other name is unique, and the
        // three wrapped refusals keep their own namespace so a log line still says
        // whether the hut hires nobody or hires somebody else.
        use std::collections::BTreeMap;
        let names = every_refusal();
        assert_eq!(names.len(), 26, "one line per variant");
        // Three refusals are wrapped, not invented: two come from
        // `crate::citizens` (`citizens.refusal.*`) and one from `crate::jobs`
        // (`jobs.refusal.*`). Their names must survive, so the check accepts the
        // three namespaces and nothing else.
        assert_eq!(
            names.iter().filter(|n| !n.starts_with("entity.")).count(),
            3
        );
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for name in names.into_iter() {
            assert!(
                name.starts_with("entity.refusal.")
                    || name.starts_with("citizens.refusal.")
                    || name.starts_with("jobs.refusal."),
                "{name} is not namespaced"
            );
            *counts.entry(name).or_insert(0) += 1;
        }
        let shared: Vec<&str> = counts
            .iter()
            .filter(|(name, count)| **count > 1 && name.starts_with("entity."))
            .map(|(name, _)| *name)
            .collect();
        assert_eq!(
            shared,
            ["entity.refusal.field", "entity.refusal.unwired"],
            "a new shared name is a log line nobody can attribute"
        );
    }

    #[test]
    fn a_citizen_is_wired_then_asked_what_its_hut_records() {
        let mut citizen = Citizen::new(12, "Cato");
        assert_eq!(citizen.check(), Ok(()), "a fresh citizen is coherent");
        citizen.posting = crate::citizens::Posting::employed(Kind::Builder, Job::Builder);
        citizen.work_at = Some([8, 70, -2]);
        assert_eq!(citizen.check(), Ok(()));
        assert_eq!(
            walk_speed(&citizen),
            Ok(BASE_MOVEMENT_SPEED),
            "a builder buys no speed"
        );
        let record = Hired::of(&citizen)
            .expect("a hired builder")
            .record(1)
            .expect("one head at level 1");
        assert_eq!(record.residents(), [12]);
        assert_eq!(carry_slots(0), INVENTORY_SLOTS);
        assert_eq!(damage_taken(100.0, 20.0), 4.0);
    }

    #[test]
    fn born_with_nothing_buried_into_the_books_and_never_despawned() {
        let citizen = Citizen::new(1, "Alma");
        assert_eq!((citizen.home, citizen.posting.job), (None, None));
        let standing = Standing::InColony {
            inside: true,
            invisible: false,
        };
        assert!(
            death::on_death(1, Some(Job::Farmer), standing)
                .expect("colony")
                .grave
        );
        assert_eq!(
            death::removal_allowed(Removal::WorldDiscard),
            Err(DeathRefusal::Persistent)
        );
    }
}
