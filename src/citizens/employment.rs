//! The link between a citizen, the job it holds, and the building it works in.
//!
//! Java reference: `core/colony/jobs/AbstractJob.java:155-179` (`assignTo`, the
//! only writer of `workBuilding`), `core/colony/buildings/modules/
//! WorkerBuildingModule.java:99-117` (`assignCitizen`), `AbstractAssignedCitizenModule
//! .java:106-109` (`isFull`) and `:155-172` (the auto-hire pass), and the guard
//! family (`core/colony/jobs/guard/`).
//!
//! **Two sides, two questions.**
//!
//! - **The citizen's** is a [`Posting`]: a job and where it works. Java keeps them
//!   in two fields `assignTo` writes together (`AbstractJob.java:173-177`), so a
//!   job without a building is not a state Java produces for a hired citizen — it
//!   is a tag somebody edited. [`Posting::check`] names it instead of guessing.
//! - **The hut's** is a [`Staffing`]: how many heads a module holds at a level
//!   against how many are assigned. A module with room and nobody in it is not an
//!   error in Java — it is what the auto-hire pass exists for
//!   (`WorkerBuildingModule.java:155-172`) — so it is a refusal only for the
//!   caller that needs the titulaire now.
//!
//! **The table stays `crate::jobs`'.** Which hut hires which job at how many heads
//! per level is `crate::jobs::employ` and `Capacity::at` — 58 modules over 47 huts.
//! This file never repeats it; it keeps the module's refusal whole
//! ([`Refusal::NotEmployed`]), so a log line still says whether the hut employs
//! nobody or just not this job.
//!
//! **What is not here.** The *firing* side — `removeFromWorkerBuilding`
//! (`AbstractJob.java:435-446`) and `removeCitizen` — is the same table read
//! backwards and belongs with the assignment list, which is `crate::colony`'s. No
//! gold and no research gate either (`WorkerBuildingModule.java:63`, `:170`): both
//! are data the hooks will read.

use crate::building::Kind;
use crate::jobs::{Job, Refusal as EmploymentRefusal, employ};

/// The six jobs whose class descends from `AbstractJobGuard`.
///
/// Java has four direct subclasses — `JobKnight` (`guard/JobKnight.java:33`),
/// `JobRanger` (`:16`), `JobDruid` (`:20`), `JobCavalry` (`:34`) — and two that
/// inherit the behaviour through one of them: `JobHuscarl extends JobKnight`
/// (`guard/JobHuscarl.java:9`) and `JobMarksman extends JobRanger`
/// (`guard/JobMarksman.java:12`). The count is **6**, and `CitizenAI` branches on
/// `instanceof AbstractJobGuard` (`CitizenAI.java:136`), so both inherited ones
/// take the guard branch too.
pub const GUARD_JOBS: [Job; 6] = [
    Job::Cavalry,
    Job::Druid,
    Job::Huscarl,
    Job::Knight,
    Job::Marksman,
    Job::Ranger,
];

/// Whether this job takes the guard branch of the decision — the question
/// `CitizenAI.java:136` asks, and the only job fact this module needs.
pub fn is_guard(job: Job) -> bool {
    GUARD_JOBS.contains(&job)
}

/// Why a job and a building do not fit together, named.
///
/// [`Refusal::NotEmployed`] keeps `crate::jobs`' refusal whole rather than
/// re-wording it: that module already says whether the hut hires nobody or hires
/// somebody else, and melting the two into one string loses the half that matters
/// when a player asks why a knight will not take the farm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// The hut cannot hire this job — `crate::jobs::employ`'s answer, unchanged.
    NotEmployed(EmploymentRefusal),
    /// Every head the module holds is taken at this level
    /// (`AbstractAssignedCitizenModule.java:106-109`).
    HutFull {
        hut: Kind,
        level: i32,
        capacity: i32,
    },
    /// The citizen already works in this module: `assignCitizen` answers `false`
    /// on `assignedCitizen.contains(citizen)` (`WorkerBuildingModule.java:101`).
    AlreadyAssigned { hut: Kind, job: Job },
    /// A job with no building. `assignTo` writes `workBuilding` and
    /// `workBuildingPos` in the same call that creates the job
    /// (`AbstractJob.java:173-174`), so the pair cannot come apart in Java.
    JobWithoutHut { job: Job },
    /// A building with no job: the only way in is a module that matched the job's
    /// registry entry (`:157-160`), so this pair is impossible there too.
    HutWithoutJob { hut: Kind },
    /// A worker module of this hut holds nobody. Java's answer is the auto-hire
    /// pass (`WorkerBuildingModule.java:155-172`), not a refusal; this variant
    /// exists for the caller that needs the titulaire now.
    HutWithoutHolder { hut: Kind },
}

impl Refusal {
    /// A stable name for logs and tests, namespaced like a registry entry so it
    /// can never be mistaken for a player-facing sentence.
    pub fn name(self) -> &'static str {
        match self {
            Self::NotEmployed(refusal) => refusal.name(),
            Self::HutFull { .. } => "citizens.refusal.hut_full",
            Self::AlreadyAssigned { .. } => "citizens.refusal.already_assigned",
            Self::JobWithoutHut { .. } => "citizens.refusal.job_without_hut",
            Self::HutWithoutJob { .. } => "citizens.refusal.hut_without_job",
            Self::HutWithoutHolder { .. } => "citizens.refusal.hut_without_holder",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

/// A citizen's working life: what it does, and where.
///
/// Both fields are `Option` because both sides of the transition exist in Java: a
/// fresh citizen has no job, and a hut is only attached by `assignTo`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Posting {
    /// The job, or `None` for a citizen the colony never hired.
    pub job: Option<Job>,
    /// The hut it works in (`AbstractJob.workBuilding`, `AbstractJob.java:95`).
    pub hut: Option<Kind>,
}

impl Posting {
    /// A citizen with neither: every citizen starts here.
    pub fn jobless() -> Posting {
        Posting {
            job: None,
            hut: None,
        }
    }

    /// A citizen already hired: the pair `assignTo` leaves behind.
    pub fn employed(hut: Kind, job: Job) -> Posting {
        Posting {
            job: Some(job),
            hut: Some(hut),
        }
    }

    /// Whether the pair is coherent now.
    ///
    /// `Job::Placeholder` is the one job allowed to stand alone: it is what Java
    /// gives a citizen that has none, and its worker AI is `null`
    /// (`core/colony/jobs/JobPlaceholder.java:9`, `:31-34`), so it names no
    /// building by design.
    pub fn check(&self) -> Result<(), Refusal> {
        match (self.job, self.hut) {
            (None, None) => Ok(()),
            (None, Some(hut)) => Err(Refusal::HutWithoutJob { hut }),
            (Some(job), None) => {
                if job == Job::Placeholder {
                    Ok(())
                } else {
                    Err(Refusal::JobWithoutHut { job })
                }
            }
            (Some(job), Some(hut)) => employ(hut, job)
                .map(|_| ())
                .map_err(Refusal::NotEmployed),
        }
    }

    /// Hire this citizen into `hut`, following Java's conditions in its own order.
    ///
    /// | Condition | Where |
    /// | --- | --- |
    /// | already in this module | `WorkerBuildingModule.java:101` |
    /// | module full at this level | `:101` → `AbstractAssignedCitizenModule.java:106-109` |
    /// | the module's job is not this job | `AbstractJob.java:157-160` |
    pub fn assign(
        &mut self,
        hut: Kind,
        level: i32,
        job: Job,
        assigned_now: usize,
    ) -> Result<(), Refusal> {
        if self.job == Some(job) && self.hut == Some(hut) {
            return Err(Refusal::AlreadyAssigned { hut, job });
        }
        let module = employ(hut, job).map_err(Refusal::NotEmployed)?;
        let capacity = module.capacity_at(level);
        if capacity <= assigned_now as i32 {
            return Err(Refusal::HutFull {
                hut,
                level,
                capacity,
            });
        }
        self.job = Some(job);
        self.hut = Some(hut);
        Ok(())
    }
}

/// How full one worker module is: `hut` hires `job`, `assigned` citizens are in
/// it, the hut is `level`.
///
/// A value rather than a query on a building because the two numbers come from two
/// places in Java — the module holds the list, the building holds the level
/// (`WorkerBuildingModule.java:60`, `:67-79`) — and a wasm mod can hold neither yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Staffing {
    pub hut: Kind,
    pub job: Job,
    pub level: i32,
    pub assigned: usize,
}

impl Staffing {
    pub fn new(hut: Kind, job: Job, level: i32, assigned: usize) -> Staffing {
        Staffing {
            hut,
            job,
            level,
            assigned,
        }
    }

    /// The head count at this level — the module's own lambda through
    /// [`crate::jobs::Capacity::at`] (`WorkerBuildingModule.java:60`; a level-0 hut
    /// answers `0`, `AbstractSchematicProvider.java:64`).
    pub fn capacity(&self) -> Result<i32, Refusal> {
        Ok(employ(self.hut, self.job)
            .map_err(Refusal::NotEmployed)?
            .capacity_at(self.level))
    }

    /// Heads still missing, never negative: Java asks `isFull()` (`assigned >=
    /// max`), not "how many are missing", and an over-staffed module must not
    /// answer `-2`.
    pub fn vacancies(&self) -> Result<i32, Refusal> {
        Ok((self.capacity()? - self.assigned as i32).max(0))
    }

    /// `AbstractAssignedCitizenModule.isFull` (`:106-109`).
    pub fn is_full(&self) -> Result<bool, Refusal> {
        Ok(self.vacancies()? == 0)
    }

    /// Nobody is in this module. A hut at level `0` holds nobody by design, so it
    /// is not "unstaffed" — it is not built yet.
    pub fn require_staffed(&self) -> Result<(), Refusal> {
        if self.assigned == 0 && self.capacity()? > 0 {
            Err(Refusal::HutWithoutHolder { hut: self.hut })
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_guard_jobs_and_no_others() {
        assert_eq!(GUARD_JOBS.len(), 6, "four direct subclasses, two inherited");
        let guards: Vec<Job> = Job::ALL.iter().copied().filter(|job| is_guard(*job)).collect();
        assert_eq!(guards.len(), 6, "no other job takes the guard branch");
        for job in GUARD_JOBS {
            assert!(Job::ALL.contains(&job), "{job:?} is not a job at all");
        }
        assert!(!is_guard(Job::Farmer));
        assert!(!is_guard(Job::Placeholder));
    }

    #[test]
    fn a_job_without_a_building_is_refused_and_the_placeholder_is_the_one_exception() {
        assert_eq!(
            Posting {
                job: Some(Job::Farmer),
                hut: None
            }
            .check(),
            Err(Refusal::JobWithoutHut { job: Job::Farmer })
        );
        assert_eq!(
            Posting {
                job: Some(Job::Placeholder),
                hut: None
            }
            .check(),
            Ok(()),
            "the jobless job names no hut (JobPlaceholder.java:31-34)"
        );
        assert_eq!(Posting::jobless().check(), Ok(()));
        assert_eq!(
            Posting {
                job: None,
                hut: Some(Kind::Farmer)
            }
            .check(),
            Err(Refusal::HutWithoutJob {
                hut: Kind::Farmer
            })
        );
    }

    #[test]
    fn the_hut_and_the_job_must_be_a_row_of_the_table_and_the_two_refusals_stay_apart() {
        assert_eq!(Posting::employed(Kind::Farmer, Job::Farmer).check(), Ok(()));
        assert_eq!(
            Posting::employed(Kind::Farmer, Job::Knight).check(),
            Err(Refusal::NotEmployed(EmploymentRefusal::JobNotEmployedHere {
                hut: Kind::Farmer,
                job: Job::Knight,
            }))
        );
        // A residence hires nobody: not the same refusal as "somebody else".
        assert_eq!(
            Posting::employed(Kind::Residence, Job::Farmer).check(),
            Err(Refusal::NotEmployed(
                EmploymentRefusal::BuildingEmploysNobody {
                    hut: Kind::Residence
                }
            ))
        );
        assert_eq!(
            Posting::jobless().assign(Kind::Residence, 3, Job::Farmer, 0),
            Err(Refusal::NotEmployed(
                EmploymentRefusal::BuildingEmploysNobody {
                    hut: Kind::Residence
                }
            ))
        );
    }

    #[test]
    fn assigning_twice_is_refused_before_anything_else_and_a_success_writes_both_fields() {
        let mut posted = Posting::employed(Kind::Farmer, Job::Farmer);
        assert_eq!(
            posted.assign(Kind::Farmer, 5, Job::Farmer, 0),
            Err(Refusal::AlreadyAssigned {
                hut: Kind::Farmer,
                job: Job::Farmer
            })
        );
        let mut fresh = Posting::jobless();
        assert_eq!(fresh.assign(Kind::Farmer, 5, Job::Farmer, 0), Ok(()));
        assert_eq!(fresh, Posting::employed(Kind::Farmer, Job::Farmer));
    }

    #[test]
    fn a_full_module_refuses_the_extra_head() {
        // A farm holds one worker at any level (`Capacity::One`).
        assert_eq!(
            Posting::jobless().assign(Kind::Farmer, 5, Job::Farmer, 1),
            Err(Refusal::HutFull {
                hut: Kind::Farmer,
                level: 5,
                capacity: 1
            })
        );
    }

    #[test]
    fn staffing_counts_heads_against_the_module_and_never_goes_negative() {
        // The warehouse's couriers are two heads per building level.
        let empty = Staffing::new(Kind::Warehouse, Job::Deliveryman, 3, 0);
        assert_eq!(empty.capacity(), Ok(6));
        assert_eq!(empty.vacancies(), Ok(6));
        assert_eq!(empty.is_full(), Ok(false));
        assert_eq!(
            empty.require_staffed(),
            Err(Refusal::HutWithoutHolder {
                hut: Kind::Warehouse
            }),
            "a standing hut with an empty module is what the auto-hire pass fills"
        );
        let over = Staffing::new(Kind::Warehouse, Job::Deliveryman, 1, 5);
        assert_eq!(over.vacancies(), Ok(0), "over-staffed is full, not -3");
        assert_eq!(over.is_full(), Ok(true));
        assert_eq!(over.require_staffed(), Ok(()));
        let unbuilt = Staffing::new(Kind::Warehouse, Job::Deliveryman, 0, 0);
        assert_eq!(unbuilt.capacity(), Ok(0), "level 0 holds nobody");
        assert_eq!(unbuilt.require_staffed(), Ok(()));
        // A hut that does not hire the job cannot be counted at all.
        assert_eq!(
            Staffing::new(Kind::Farmer, Job::Knight, 3, 0).capacity(),
            Err(Refusal::NotEmployed(EmploymentRefusal::JobNotEmployedHere {
                hut: Kind::Farmer,
                job: Job::Knight
            }))
        );
    }

    #[test]
    fn every_refusal_has_its_own_namespaced_name() {
        let all = [
            Refusal::HutFull {
                hut: Kind::Farmer,
                level: 1,
                capacity: 1,
            },
            Refusal::AlreadyAssigned {
                hut: Kind::Farmer,
                job: Job::Farmer,
            },
            Refusal::JobWithoutHut { job: Job::Farmer },
            Refusal::HutWithoutJob {
                hut: Kind::Farmer,
            },
            Refusal::HutWithoutHolder {
                hut: Kind::Farmer,
            },
            Refusal::NotEmployed(EmploymentRefusal::BuildingEmploysNobody {
                hut: Kind::Residence,
            }),
        ];
        for (index, refusal) in all.iter().enumerate() {
            for other in &all[index + 1..] {
                assert_ne!(
                    refusal.name(),
                    other.name(),
                    "two refusals share a name, so a log line cannot tell them apart"
                );
            }
            assert!(
                refusal.name().starts_with("citizens.refusal.")
                    || refusal.name().starts_with("jobs.refusal."),
                "{} is not namespaced",
                refusal.name()
            );
        }
    }
}
