//! The other half of the citizen: what a hut's block entity records about it.
//!
//! Java reference: `AbstractAssignedCitizenModule.java` (the list `:31`, `isFull`
//! `:106-109`, the hiring mode `:143-155`), `WorkerBuildingModule.java:120-155`
//! (three shapes on the way in) and `:175-185` (one on the way out),
//! `LivingBuildingModule.java:22-66`, `AbstractJob.java:155-179` and
//! `api/colony/buildings/HiringMode.java:9-15`.
//!
//! **One key, two hoses.** An employing hut and a housing hut write the **same**
//! `residents` array — both constants are the string `"residents"`
//! (`NbtTagConstants.java:383`, `:388`) — which is exactly why the citizen's own
//! compound has no home key: the link is hut-side, once. Rebuilding a citizen's
//! world is therefore two reads, this file and [`crate::entity::data`], never one.
//!
//! **Two silent drops, kept by Java and refused here.** A resident the colony does
//! not know is `null` and disappears (`:129`); an excess resident is dropped by
//! `assignCitizen` when full (`:101`). Both lose a worker without a log line, so
//! the port refuses them ([`Refusal::Overfull`], [`Refusal::BadCitizen`]) and says
//! so instead of pretending Java agrees.

use crate::building::Kind;
use crate::citizens::{Posting, Refusal as EmploymentRefusal};
use crate::entity::data::Citizen;
use crate::jobs::Job;
use crate::network::common::int;
use mod_api::{Compound, Tag};

/// The residents, for a work hut and a residence alike; written as an `int[]`
/// (`WorkerBuildingModule.java:181`).
pub const KEY_RESIDENTS: &str = "residents";
/// The legacy list of `{workerId}` compounds (`NbtTagConstants.java:343`), read
/// first (`WorkerBuildingModule.java:124-133`).
pub const KEY_LEGACY_WORKER: &str = "worker";
/// The id inside one legacy record (`NbtTagConstants.java:378`).
pub const KEY_WORKER_ID: &str = "workerId";
/// The hiring mode, as an ordinal (`NbtTagConstants.java:348`).
pub const KEY_HIRING_MODE: &str = "buildingHiringMode";
/// The pre-1.16 wrapper that held it (`NbtTagConstants.java:353`), read first by
/// `AbstractAssignedCitizenModule.java:147-151`.
pub const KEY_ASSIGNED: &str = "assigned";

/// `HiringMode.java:9-15`, in declaration order because the save carries the
/// ordinal (`AbstractAssignedCitizenModule.java:145`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: four constants in Java's enum, and the ordinal is the save format
pub enum HiringMode {
    /// Follows the colony's setting.
    #[default]
    Default,
    /// The hut hires on its own (`WorkerBuildingModule.java:160-172`).
    Auto,
    Manual,
    /// Nothing is added or removed automatically.
    Locked,
}

impl HiringMode {
    pub const ALL: [HiringMode; 4] = [
        HiringMode::Default,
        HiringMode::Auto,
        HiringMode::Manual,
        HiringMode::Locked,
    ];

    pub fn ordinal(self) -> i32 {
        match self {
            HiringMode::Default => 0,
            HiringMode::Auto => 1,
            HiringMode::Manual => 2,
            HiringMode::Locked => 3,
        }
    }

    /// `None` outside `0..=3`: Java writes an ordinal and reads `values()[n]`
    /// (`:149`), which throws on a stray value.
    pub fn from_ordinal(value: i32) -> Option<HiringMode> {
        HiringMode::ALL.get(usize::try_from(value).ok()?).copied()
    }
}

/// Why a hut's worker compound cannot be read, or written as asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// A key present as the wrong NBT kind.
    Field {
        key: &'static str,
    },
    UnknownMode {
        value: i32,
    },
    /// A resident id of zero or less: the colony would answer `null` and Java would
    /// drop the entry without a word (`:129`).
    BadCitizen {
        citizen: i32,
    },
    /// More residents than the module holds. Java drops the extras (`:101`).
    Overfull {
        capacity: i32,
        found: usize,
    },
    /// Full, on a hire (`AbstractAssignedCitizenModule.java:106-109`).
    Full {
        capacity: i32,
    },
    /// Already a resident (`WorkerBuildingModule.java:101`).
    AlreadyThere {
        citizen: i32,
    },
    /// `fire` for somebody who is not in the module (`:42-53`).
    Absent {
        citizen: i32,
    },
    /// No worker to answer with: `getFirstCitizen` is `null` (`:93-97`).
    NoWorker,
    /// A jobless citizen is not a hire: `assignCitizen` creates the job first
    /// (`WorkerBuildingModule.java:146-152`).
    Unemployed,
    /// A job whose hut kind is not resolved yet, or a pair the table does not know;
    /// the employment refusal is kept whole.
    Posting(EmploymentRefusal),
    Unwired {
        job: Job,
    },
}

impl Refusal {
    /// A stable name for logs and tests, namespaced like a registry entry so it can
    /// never be mistaken for a player-facing sentence.
    pub fn name(self) -> &'static str {
        match self {
            Self::Field { .. } => "entity.refusal.field",
            Self::UnknownMode { .. } => "entity.refusal.unknown_mode",
            Self::BadCitizen { .. } => "entity.refusal.bad_citizen",
            Self::Overfull { .. } => "entity.refusal.overfull",
            Self::Full { .. } => "entity.refusal.hut_full",
            Self::AlreadyThere { .. } => "entity.refusal.already_there",
            Self::Absent { .. } => "entity.refusal.absent",
            Self::NoWorker => "entity.refusal.no_worker",
            Self::Unemployed => "entity.refusal.unemployed",
            Self::Posting(refusal) => refusal.name(),
            Self::Unwired { .. } => "entity.refusal.unwired",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

/// One hut's `residents` list and hiring mode: the whole of what a hut block
/// entity holds about its people.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Record {
    residents: Vec<i32>,
    mode: HiringMode,
}

impl Default for Record {
    fn default() -> Record {
        Record::empty()
    }
}

impl Record {
    /// A hut nobody is assigned to.
    pub fn empty() -> Record {
        Record {
            residents: Vec::new(),
            mode: HiringMode::Default,
        }
    }

    pub fn mode(&self) -> HiringMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: HiringMode) {
        self.mode = mode;
    }

    /// The ids, in the order the hut wrote them.
    pub fn residents(&self) -> &[i32] {
        &self.residents
    }

    /// The one worker of a one-head module (`AbstractAssignedCitizenModule.java:93-97`).
    pub fn worker(&self) -> Result<i32, Refusal> {
        self.residents.first().copied().ok_or(Refusal::NoWorker)
    }

    /// Add a resident, refusing what Java refuses and what Java would silently drop.
    pub fn hire(&mut self, citizen: i32, capacity: i32) -> Result<(), Refusal> {
        if citizen <= 0 {
            return Err(Refusal::BadCitizen { citizen });
        }
        if self.residents.contains(&citizen) {
            return Err(Refusal::AlreadyThere { citizen });
        }
        if self.residents.len() as i32 >= capacity {
            return Err(Refusal::Full { capacity });
        }
        self.residents.push(citizen);
        Ok(())
    }

    /// Remove a resident (`AbstractAssignedCitizenModule.removeCitizen`, `:42-53`).
    pub fn fire(&mut self, citizen: i32) -> Result<(), Refusal> {
        let Some(index) = self.residents.iter().position(|id| *id == citizen) else {
            return Err(Refusal::Absent { citizen });
        };
        self.residents.remove(index);
        Ok(())
    }

    /// What the hut writes: `residents` only when non-empty
    /// (`WorkerBuildingModule.java:178-184`), the hiring mode always (`:143-146`).
    pub fn to_module(&self) -> Compound {
        let mut data = Compound::from([(KEY_HIRING_MODE.into(), int(self.mode.ordinal()))]);
        if !self.residents.is_empty() {
            data.insert(KEY_RESIDENTS.into(), Tag::IntArray(self.residents.clone()));
        }
        data
    }

    /// The three shapes Java accepts, in Java's order, checked against the module's
    /// size at its level.
    pub fn from_module(data: &Compound, capacity: i32) -> Result<Record, Refusal> {
        let mode = mode_of(data)?;
        let residents = match data.get(&KEY_LEGACY_WORKER.into()) {
            Some(Tag::List(records)) => {
                let mut ids = Vec::with_capacity(records.len());
                for record in records {
                    match record.get(KEY_WORKER_ID) {
                        Some(Tag::Int(value)) if *value > 0 => ids.push(*value),
                        Some(Tag::Int(value)) => {
                            return Err(Refusal::BadCitizen { citizen: *value });
                        }
                        Some(_) | None => return Err(Refusal::Field { key: KEY_WORKER_ID }),
                    }
                }
                ids
            }
            Some(_) => {
                return Err(Refusal::Field {
                    key: KEY_LEGACY_WORKER,
                });
            }
            None => match data.get(&KEY_RESIDENTS.into()) {
                Some(Tag::IntArray(values)) => {
                    if let Some(bad) = values.iter().find(|id| **id <= 0) {
                        return Err(Refusal::BadCitizen { citizen: *bad });
                    }
                    values.clone()
                }
                Some(_) => return Err(Refusal::Field { key: KEY_RESIDENTS }),
                None => Vec::new(),
            },
        };
        // A duplicate is what `contains` prevents on assignment (`:101`); on load
        // Java's second `assignCitizen` returns `false` and drops the entry. Keeping
        // the first is the same answer without a second write.
        let mut unique: Vec<i32> = Vec::with_capacity(residents.len());
        for id in residents {
            if !unique.contains(&id) {
                unique.push(id);
            }
        }
        if unique.len() as i32 > capacity {
            return Err(Refusal::Overfull {
                capacity,
                found: unique.len(),
            });
        }
        Ok(Record {
            residents: unique,
            mode,
        })
    }
}

/// Where a citizen works, as the hut needs it. This is the conversion in the only
/// direction Java has it: the citizen's job says *which* hut, the hut's record says
/// *who*. The reverse needs the colony's roster (`crate::colony::Colony::citizens`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three facts a hire needs
pub struct Hired {
    pub hut: Kind,
    pub job: Job,
    pub citizen: i32,
}

impl Hired {
    /// Read a citizen's posting as a hire, refusing the two states that are not
    /// one: no job, and a job whose hut kind the colony has not resolved.
    pub fn of(citizen: &Citizen) -> Result<Hired, Refusal> {
        if citizen.id <= 0 {
            return Err(Refusal::BadCitizen {
                citizen: citizen.id,
            });
        }
        let Posting { job, hut } = citizen.posting;
        match (job, hut) {
            (Some(job), Some(hut)) => Ok(Hired {
                hut,
                job,
                citizen: citizen.id,
            }),
            (Some(job), None) => Err(Refusal::Unwired { job }),
            (None, _) => Err(Refusal::Unemployed),
        }
    }

    /// The hut's head count at `level`, from `crate::jobs`' table and not from a
    /// constant of this file.
    pub fn capacity(&self, level: i32) -> Result<i32, Refusal> {
        crate::jobs::employ(self.hut, self.job)
            .map(|module| module.capacity_at(level))
            .map_err(|refusal| Refusal::Posting(EmploymentRefusal::NotEmployed(refusal)))
    }

    /// The record `assignCitizen` + `onAssignment` leave behind
    /// (`WorkerBuildingModule.java:146-152`).
    pub fn record(&self, level: i32) -> Result<Record, Refusal> {
        let mut record = Record::empty();
        record.hire(self.citizen, self.capacity(level)?)?;
        Ok(record)
    }
}

/// The hiring mode, in Java's read order: the legacy `assigned` wrapper, then the
/// module's own key, then `DEFAULT` (`AbstractAssignedCitizenModule.java:147-155`).
fn mode_of(data: &Compound) -> Result<HiringMode, Refusal> {
    let read = |tag: &Tag| match tag {
        Tag::Int(value) => {
            HiringMode::from_ordinal(*value).ok_or(Refusal::UnknownMode { value: *value })
        }
        _ => Err(Refusal::Field {
            key: KEY_HIRING_MODE,
        }),
    };
    if let Some(tag) = data.get(&KEY_ASSIGNED.into()) {
        let Tag::Compound(inner) = tag else {
            return Err(Refusal::Field { key: KEY_ASSIGNED });
        };
        return match inner.get(&KEY_HIRING_MODE.into()) {
            Some(tag) => read(tag),
            None => Err(Refusal::Field {
                key: KEY_HIRING_MODE,
            }),
        };
    }
    match data.get(&KEY_HIRING_MODE.into()) {
        Some(tag) => read(tag),
        None => Ok(HiringMode::Default),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn citizen_with(hut: Kind, job: Job) -> Citizen {
        let mut citizen = Citizen::new(12, "Cato");
        citizen.posting = Posting::employed(hut, job);
        citizen
    }

    #[test]
    fn an_empty_hut_writes_its_mode_and_no_residents_key_at_all() {
        let record = Record::empty();
        let data = record.to_module();
        assert_eq!(data.len(), 1, "Java writes residents only when non-empty");
        assert_eq!(data.get(&KEY_HIRING_MODE.into()), Some(&int(0)));
        assert_eq!(Record::from_module(&data, 1), Ok(Record::empty()));
        assert_eq!(record.worker(), Err(Refusal::NoWorker));
    }

    #[test]
    fn a_hired_citizen_is_one_id_in_the_hut_and_nothing_else() {
        let hired = Hired::of(&citizen_with(Kind::Builder, Job::Builder)).expect("a builder");
        assert_eq!(
            hired,
            Hired {
                hut: Kind::Builder,
                job: Job::Builder,
                citizen: 12
            }
        );
        let record = hired.record(1).expect("one head at level 1");
        assert_eq!(record.worker(), Ok(12));
        let data = record.to_module();
        assert_eq!(
            data.get(&KEY_RESIDENTS.into()),
            Some(&Tag::IntArray(vec![12]))
        );
        assert_eq!(data.len(), 2, "the array and the mode, never the name");
        assert!(
            !format!("{data:?}").contains("Cato"),
            "Java stores ids only"
        );
        assert_eq!(Record::from_module(&data, 1), Ok(record));
    }

    #[test]
    fn the_legacy_worker_list_reads_and_is_written_back_in_the_new_shape() {
        let legacy = Compound::from([(
            KEY_LEGACY_WORKER.into(),
            Tag::List(vec![Tag::compound([(KEY_WORKER_ID, Tag::Int(5))])]),
        )]);
        let record = Record::from_module(&legacy, 2).expect("the old shape");
        assert_eq!(record.residents(), [5]);
        let written = record.to_module();
        assert_eq!(
            written.get(&KEY_RESIDENTS.into()),
            Some(&Tag::IntArray(vec![5]))
        );
        assert!(written.get(&KEY_LEGACY_WORKER.into()).is_none());
    }

    #[test]
    fn the_hiring_mode_round_trips_through_its_ordinal_and_the_legacy_wrapper_wins() {
        for mode in HiringMode::ALL {
            let mut record = Record::empty();
            record.set_mode(mode);
            assert_eq!(
                Record::from_module(&record.to_module(), 1)
                    .expect("a mode")
                    .mode(),
                mode
            );
        }
        let legacy = Compound::from([(
            KEY_ASSIGNED.into(),
            Tag::compound([(KEY_HIRING_MODE, int(2))]),
        )]);
        assert_eq!(
            Record::from_module(&legacy, 1).expect("wrapper").mode(),
            HiringMode::Manual,
            "AbstractAssignedCitizenModule.java:147-151 reads this first"
        );
        let stray = Compound::from([(KEY_HIRING_MODE.into(), int(9))]);
        assert_eq!(
            Record::from_module(&stray, 1),
            Err(Refusal::UnknownMode { value: 9 })
        );
    }

    #[test]
    fn more_residents_than_the_module_holds_is_a_refusal_java_would_drop() {
        let data = Compound::from([(KEY_RESIDENTS.into(), Tag::IntArray(vec![1, 2, 3]))]);
        assert_eq!(
            Record::from_module(&data, 2),
            Err(Refusal::Overfull {
                capacity: 2,
                found: 3
            })
        );
        assert_eq!(
            Refusal::Overfull {
                capacity: 2,
                found: 3
            }
            .name(),
            "entity.refusal.overfull"
        );
        assert_eq!(
            Record::from_module(&data, 3)
                .expect("three heads")
                .residents(),
            [1, 2, 3]
        );
    }

    #[test]
    fn a_zero_resident_is_refused_and_a_duplicate_is_kept_once() {
        let zero = Compound::from([(KEY_RESIDENTS.into(), Tag::IntArray(vec![4, 0]))]);
        assert_eq!(
            Record::from_module(&zero, 4),
            Err(Refusal::BadCitizen { citizen: 0 })
        );
        let twice = Compound::from([(KEY_RESIDENTS.into(), Tag::IntArray(vec![4, 4, 5]))]);
        assert_eq!(
            Record::from_module(&twice, 4).expect("deduped").residents(),
            [4, 5]
        );
    }

    #[test]
    fn hiring_twice_filling_up_and_firing_a_stranger_are_the_three_refusals() {
        let mut record = Record::empty();
        record.hire(4, 1).expect("first head");
        assert_eq!(record.hire(4, 1), Err(Refusal::AlreadyThere { citizen: 4 }));
        assert_eq!(record.hire(5, 1), Err(Refusal::Full { capacity: 1 }));
        assert_eq!(record.hire(0, 1), Err(Refusal::BadCitizen { citizen: 0 }));
        assert_eq!(record.fire(9), Err(Refusal::Absent { citizen: 9 }));
        assert_eq!(record.fire(4), Ok(()));
        assert!(record.residents().is_empty());
        assert_eq!(Refusal::NoWorker.name(), "entity.refusal.no_worker");
    }

    #[test]
    fn the_capacity_is_the_jobs_table_not_a_constant_of_this_file() {
        let builder = Hired::of(&citizen_with(Kind::Builder, Job::Builder)).expect("a builder");
        assert_eq!(builder.capacity(1), Ok(1));
        assert_eq!(builder.capacity(5), Ok(1), "builder_work is Capacity::One");
        let scholar =
            Hired::of(&citizen_with(Kind::University, Job::Researcher)).expect("a researcher");
        assert_eq!(scholar.capacity(3), Ok(3), "PerLevel at level 3");
        let courier =
            Hired::of(&citizen_with(Kind::Warehouse, Job::Deliveryman)).expect("a courier");
        assert_eq!(courier.capacity(3), Ok(6), "TwiceLevel at level 3");
    }

    #[test]
    fn a_citizen_with_no_job_or_no_resolved_hut_cannot_be_a_hire() {
        assert_eq!(
            Hired::of(&Citizen::new(12, "Cato")),
            Err(Refusal::Unemployed)
        );
        let mut unwired = Citizen::new(12, "Cato");
        unwired.posting = Posting {
            job: Some(Job::Farmer),
            hut: None,
        };
        assert_eq!(
            Hired::of(&unwired),
            Err(Refusal::Unwired { job: Job::Farmer })
        );
        assert_eq!(
            Hired::of(&Citizen::new(0, "Cato")),
            Err(Refusal::BadCitizen { citizen: 0 })
        );
        assert_eq!(Refusal::Unemployed.name(), "entity.refusal.unemployed");
    }

    #[test]
    fn a_wrong_kind_on_a_key_is_named_and_an_absent_one_is_not_an_error() {
        let wrong = Compound::from([(KEY_RESIDENTS.into(), int_text("five"))]);
        assert_eq!(
            Record::from_module(&wrong, 2),
            Err(Refusal::Field { key: KEY_RESIDENTS })
        );
        let legacy_wrong = Compound::from([(KEY_LEGACY_WORKER.into(), int(5))]);
        assert_eq!(
            Record::from_module(&legacy_wrong, 2),
            Err(Refusal::Field {
                key: KEY_LEGACY_WORKER
            })
        );
        let entry_without_id =
            Compound::from([(KEY_LEGACY_WORKER.into(), Tag::List(vec![Tag::compound(Vec::<(&str, Tag)>::new())]))]);
        assert_eq!(
            Record::from_module(&entry_without_id, 2),
            Err(Refusal::Field { key: KEY_WORKER_ID })
        );
        assert_eq!(
            Record::from_module(&Compound::new(), 1),
            Ok(Record::empty()),
            "an old hut with no mode is DEFAULT (HiringMode.java:11)"
        );
    }

    fn int_text(value: &str) -> Tag {
        Tag::String(value.into())
    }
}
