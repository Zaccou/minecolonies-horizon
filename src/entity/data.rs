//! The citizen's own data, and the compound it writes.
//!
//! Java reference: `core/colony/CitizenData.java` (fields `:108-347`, codec
//! `:1316-1421` and `:1425-1600`), `api/colony/ICitizenData.java`, and
//! `CitizenDataManager.loadFromNBT` (`:1838-1845`).
//!
//! **Three facts a player sees are written elsewhere and only read back into
//! `CitizenData`**: the **home** (the residence hut's `residents` array,
//! `LivingBuildingModule.java:91`), the **work hut's kind** (the hut module; the
//! job compound keeps a position, `AbstractJob.java:202`) and **health** (the
//! entity's attribute, `AbstractEntityCitizen.java:159`). So [`Citizen::home`] and
//! [`Citizen::posting`]'s `hut` are **runtime** fields, [`Citizen::to_compound`]
//! does not write them — Java does not, and a new key would be a save Java cannot
//! read — and a citizen loaded alone is *unwired*, not broken.
//!
//! **The keys are Java's, typos included**: `isWorking` is saved under
//! `activeNodeNode` (`BuildingConstants.java:43`, which is `TAG_ACTIVE`). Renaming
//! it would orphan every existing save.
//!
//! **The writers are shared, the readers are not.** `crate::network::common`
//! already builds every NBT scalar this file writes, so it is used as-is; its
//! *readers* refuse with a `network.refusal.*` name, and a citizen's field is not a
//! message — so the six readers below return this module's own names.

use crate::citizens::happiness::Happiness;
use crate::citizens::{MAX_SATURATION, Needs, Posting, Refusal as EmploymentRefusal};
use crate::jobs::Job;
use crate::jobs::employment::Skill;
use crate::jobs::experience::MAX_CITIZEN_LEVEL;
use crate::network::common::{boolean, double, int, nested, position, text};
use mod_api::{Compound, Tag};

/// Keys, with Java's names: `id` (`NbtTagConstants.java:8`), `name` (`:9`),
/// `suffix` (`:10`), `soundProfile` (`:33`), `female` (`:488`), `paused` (`:81`),
/// `child` (`:82`), `texture` (`:489`), `newSkills` (`:494`), `pos` (`:51`),
/// `nextrespawnpos` (`:714`), `saturation` (`:495`), `beds` (`:408`), `asleep`
/// (`:496`), `justAte` (`:84`), `jobStatus` (`:93`), `leisureSites` (`:46`).
pub const KEY_ID: &str = "id";
pub const KEY_NAME: &str = "name";
pub const KEY_SUFFIX: &str = "suffix";
pub const KEY_SOUND_PROFILE: &str = "soundProfile";
pub const KEY_FEMALE: &str = "female";
pub const KEY_PAUSED: &str = "paused";
pub const KEY_CHILD: &str = "child";
pub const KEY_TEXTURE: &str = "texture";
pub const KEY_SKILLS: &str = "newSkills";
pub const KEY_POS: &str = "pos";
pub const KEY_RESPAWN: &str = "nextrespawnpos";
pub const KEY_SATURATION: &str = "saturation";
pub const KEY_BEDS: &str = "beds";
pub const KEY_ASLEEP: &str = "asleep";
pub const KEY_JUST_ATE: &str = "justAte";
pub const KEY_JOB_STATUS: &str = "jobStatus";
pub const KEY_LEISURE: &str = "leisureSites";
/// `isWorking`, under Java's misspelt key (`BuildingConstants.java:43`,
/// written at `CitizenData.java:1386`).
pub const KEY_WORKING: &str = "activeNodeNode";
/// The literal `"job"` (`CitizenData.java:1339`): Java gave this key no constant.
pub const KEY_JOB: &str = "job";
/// The job compound's registry path (`NbtTagConstants.java:574`, `AbstractJob.java:192`).
pub const KEY_JOB_TYPE: &str = "type";
/// The hut the job belongs to, as a position (`AbstractJob.java:55`, `:202`).
pub const KEY_JOB_WORK: &str = "workPos";
/// The skill list's key (`NbtTagConstants.java:309`) and one record's three keys
/// (`:491`, `:496`-adjacent `level`, `:493`).
pub const KEY_LEVEL_MAP: &str = "levelMap";
pub const KEY_SKILL: &str = "skill";
pub const KEY_LEVEL: &str = "level";
pub const KEY_EXPERIENCE: &str = "experience";

/// `BlockPos.ZERO`: what Java writes for "no bed" and compares against
/// (`CitizenData.java:574-577`).
pub const NO_POSITION: [i32; 3] = [0, 0, 0];

/// The three constants of `JobStatus` (`api/entity/ai/JobStatus.java:3-16`),
/// saved as an **ordinal** (`CitizenData.java:1367`, read at `:1541`), so the
/// order is part of the format.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: three constants in Java's enum
pub enum JobStatus {
    /// Nothing to do this instant.
    #[default]
    Idle,
    Working,
    /// Has work and cannot do it.
    Stuck,
}

impl JobStatus {
    pub const ALL: [JobStatus; 3] = [JobStatus::Idle, JobStatus::Working, JobStatus::Stuck];

    pub fn ordinal(self) -> i32 {
        match self {
            JobStatus::Idle => 0,
            JobStatus::Working => 1,
            JobStatus::Stuck => 2,
        }
    }

    /// `None` outside `0..=2`: Java indexes `values()[n]` (`:1541`) and throws, so
    /// a stray ordinal is refused here rather than read as `IDLE`.
    pub fn from_ordinal(value: i32) -> Option<JobStatus> {
        JobStatus::ALL.get(usize::try_from(value).ok()?).copied()
    }
}

/// Why a citizen's compound cannot be read as a citizen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// Absent, or present as another NBT kind, where Java's `getInt` would answer
    /// `0` and hide it.
    Field { key: &'static str },
    /// `id` is zero or less: ids start at `1` and `registerWithColony` discards a
    /// zero (`CitizenColonyHandler.java:94-97`).
    NoId,
    /// `name` is empty: `generateName` never answers one (`CitizenData.java:458-508`).
    NoName,
    /// The job/hut pair is not one Java produces (`AbstractJob.assignTo`, `:155-179`).
    Posting(EmploymentRefusal),
    /// A hut kind without the position it stands at: it cannot be walked to.
    WorkWithoutPosition { hut: crate::building::Kind },
    /// `jobStatus` outside `0..=2`.
    UnknownStatus { value: i32 },
}

impl Refusal {
    /// A stable name for logs and tests, namespaced like a registry entry so it
    /// can never be mistaken for a player-facing sentence.
    pub fn name(self) -> &'static str {
        match self {
            Self::Field { .. } => "entity.refusal.field",
            Self::NoId => "entity.refusal.no_id",
            Self::NoName => "entity.refusal.no_name",
            Self::Posting(refusal) => refusal.name(),
            Self::WorkWithoutPosition { .. } => "entity.refusal.work_without_position",
            Self::UnknownStatus { .. } => "entity.refusal.unknown_status",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

/// The eleven skills of one citizen, in Java's declaration order (`Skill::ALL`),
/// so the enum ordinal **is** the index — which is how Java saves them
/// (`CitizenSkillHandler.java:127`, keyed by ordinal, not by name).
///
/// The birth draw is not here: a new citizen's levels are
/// `random.nextInt(levelCap - 1) + 1` (`:63`), so [`Skills::new`] is the floor and
/// a caller that wants the draw passes the levels in ([`Skills::with_levels`]).
#[derive(Clone, Copy, Debug, PartialEq)]
// exhaustive: eleven skills in Java, and a twelfth would change the save ordinal
pub struct Skills {
    levels: [i32; 11],
    experience: [f64; 11],
}

impl Default for Skills {
    fn default() -> Skills {
        Skills::new()
    }
}

impl Skills {
    /// Every skill at level 1 with no experience: the floor of `:63`, and what
    /// `read` clamps to (`:146`).
    pub fn new() -> Skills {
        Skills {
            levels: [1; 11],
            experience: [0.0; 11],
        }
    }

    /// The same with one level per skill, each clamped to `1..=99` (`:146`).
    pub fn with_levels(levels: [i32; 11]) -> Skills {
        let mut skills = Skills::new();
        for (index, level) in levels.into_iter().enumerate() {
            skills.levels[index] = level.clamp(1, MAX_CITIZEN_LEVEL);
        }
        skills
    }

    pub fn index(skill: Skill) -> usize {
        Skill::ALL
            .iter()
            .position(|candidate| *candidate == skill)
            .unwrap_or(0)
    }

    pub fn level(&self, skill: Skill) -> i32 {
        self.levels[Skills::index(skill)]
    }

    pub fn experience(&self, skill: Skill) -> f64 {
        self.experience[Skills::index(skill)]
    }

    /// Set one skill, clamping the level as the loader does and refusing a
    /// negative total: a banked experience below zero is a corrupt file.
    pub fn set(&mut self, skill: Skill, level: i32, experience: f64) -> Result<(), Refusal> {
        if !(experience >= 0.0) {
            return Err(Refusal::Field { key: KEY_SKILLS });
        }
        let index = Skills::index(skill);
        self.levels[index] = level.clamp(1, MAX_CITIZEN_LEVEL);
        self.experience[index] = experience;
        Ok(())
    }

    /// One record per skill under `levelMap` (`CitizenSkillHandler.java:117-136`).
    pub fn to_compound(&self) -> Compound {
        let mut entries = Vec::with_capacity(Skill::ALL.len());
        for skill in Skill::ALL {
            let index = Skills::index(skill);
            entries.push(Tag::compound([
                (KEY_SKILL, int(i32::try_from(index).unwrap_or(0))),
                (KEY_LEVEL, int(self.levels[index])),
                (KEY_EXPERIENCE, double(self.experience[index])),
            ]));
        }
        Compound::from([(KEY_LEVEL_MAP.into(), Tag::List(entries))])
    }

    /// `CitizenSkillHandler.read` (`:139-148`): the level is clamped, a skill the
    /// list omits keeps its floor, an ordinal `Skill` does not have is refused.
    pub fn from_compound(data: &Compound) -> Result<Skills, Refusal> {
        let mut skills = Skills::new();
        let Some(tag) = data.get(&KEY_LEVEL_MAP.into()) else {
            return Ok(skills);
        };
        let Tag::List(entries) = tag else {
            return Err(Refusal::Field { key: KEY_SKILLS });
        };
        for entry in entries {
            let Some(skill) = usize::try_from(int_in(entry, KEY_SKILL)?)
                .ok()
                .and_then(|index| Skill::ALL.get(index))
            else {
                return Err(Refusal::Field { key: KEY_SKILLS });
            };
            let level = int_in(entry, KEY_LEVEL)?;
            let experience = double_in(entry, KEY_EXPERIENCE)?;
            skills.set(*skill, level, experience)?;
        }
        Ok(skills)
    }
}

/// One citizen, as `CitizenData` holds it. [`crate::colony::Citizen`] is the
/// roster line (id and name); [`Citizen::roster`] bridges the two.
#[derive(Clone, Debug, PartialEq)]
// non-exhaustive: this port's own value, and the family fields are still missing
pub struct Citizen {
    /// `CitizenData.id` (`:108`): unique in the colony, never reused.
    pub id: i32,
    pub name: String,
    /// A texture variant, one of `CitizenData.SUFFIXES` (`:98`, `:246`).
    pub suffix: String,
    /// The voice, `0..4` (`:103`, `:327`).
    pub voice: i32,
    pub female: bool,
    pub child: bool,
    /// Stopped by the manager by hand (`:143`).
    pub paused: bool,
    /// The texture index, `0..255` (`:521`).
    pub texture: i32,
    /// Saturation, health, sleep, hunger, disease, mourning — `crate::citizens::Needs`
    /// already models them, so they are not repeated here.
    pub needs: Needs,
    /// The ten modifiers plus the event ones. **Not serialised**:
    /// `citizens::happiness::MANQUANT.md` §4.
    pub happiness: Happiness,
    pub skills: Skills,
    pub status: JobStatus,
    /// The job and the hut it works in. The kind is `None` for a citizen read from
    /// its compound: only the position is saved.
    pub posting: Posting,
    /// The position in the job compound (`AbstractJob.java:202`).
    pub work_at: Option<[i32; 3]>,
    /// From the residence hut (`LivingBuildingModule.java:91`), never from here.
    pub home: Option<[i32; 3]>,
    /// Saved when the entity is absent (`CitizenData.java:1332`).
    pub last_position: [i32; 3],
    /// Where a recall must bring it back (`:1335`).
    pub respawn: Option<[i32; 3]>,
    pub bed: Option<[i32; 3]>,
    /// `isWorking` (`:297`, `:1386`).
    pub working: bool,
    /// `leisureTime` (`:347`).
    pub leisure: i32,
}

impl Citizen {
    /// A fresh citizen: what `initForNewCivilian` sets (`:513-538`) minus the
    /// three draws — the name, the gender and the skill levels — which are the
    /// caller's, because Java draws them at random.
    pub fn new(id: i32, name: &str) -> Citizen {
        Citizen {
            id,
            name: name.to_owned(),
            suffix: "_a".to_owned(),
            voice: 0,
            female: false,
            child: false,
            paused: false,
            texture: 0,
            needs: Needs::new(),
            happiness: Happiness::new(),
            skills: Skills::new(),
            status: JobStatus::Idle,
            posting: Posting::jobless(),
            work_at: None,
            home: None,
            last_position: NO_POSITION,
            respawn: None,
            bed: None,
            working: false,
            leisure: 0,
        }
    }

    pub fn roster(&self) -> crate::colony::Citizen {
        crate::colony::Citizen::new(self.id, &self.name)
    }

    pub fn sleep_in(&mut self, bed: [i32; 3]) {
        self.bed = Some(bed);
        self.needs.asleep = true;
    }

    /// Home, from the residence hut's assignment.
    pub fn housed_at(&mut self, at: [i32; 3]) {
        self.home = Some(at);
    }

    /// Resolve the hut kind of a loaded job. The refusal is `crate::jobs`' own: the
    /// pair must be one the hut hires (`WorkerBuildingModule.java:157-160`).
    pub fn wire_work(&mut self, hut: crate::building::Kind) -> Result<(), Refusal> {
        let Some(job) = self.posting.job else {
            return Err(Refusal::Posting(EmploymentRefusal::HutWithoutJob { hut }));
        };
        crate::jobs::employ(hut, job)
            .map_err(|refusal| Refusal::Posting(EmploymentRefusal::NotEmployed(refusal)))?;
        self.posting.hut = Some(hut);
        Ok(())
    }

    /// Whether the record is self-consistent — with the one state Java re-reads
    /// allowed: **a job and its position with no hut kind yet**, which is what
    /// [`Citizen::from_compound`] always produces.
    pub fn check(&self) -> Result<(), Refusal> {
        if self.id <= 0 {
            return Err(Refusal::NoId);
        }
        if self.name.is_empty() {
            return Err(Refusal::NoName);
        }
        match (self.posting.hut, self.work_at) {
            (Some(hut), None) => Err(Refusal::WorkWithoutPosition { hut }),
            (Some(_), Some(_)) => self.posting.check().map_err(Refusal::Posting),
            (None, _) => Ok(()),
        }
    }

    /// Everything `serializeNBT` writes that this slice models (`:1316-1421`).
    pub fn to_compound(&self) -> Compound {
        let mut data = Compound::from([
            (KEY_ID.into(), int(self.id)),
            (KEY_NAME.into(), text(&self.name)),
            (KEY_SUFFIX.into(), text(&self.suffix)),
            (KEY_SOUND_PROFILE.into(), int(self.voice)),
            (KEY_FEMALE.into(), boolean(self.female)),
            (KEY_PAUSED.into(), boolean(self.paused)),
            (KEY_CHILD.into(), boolean(self.child)),
            (KEY_TEXTURE.into(), int(self.texture)),
            (KEY_SKILLS.into(), nested(&self.skills.to_compound())),
            (KEY_POS.into(), position(self.last_position)),
            (KEY_SATURATION.into(), double(self.needs.saturation)),
            (KEY_BEDS.into(), position(self.bed.unwrap_or(NO_POSITION))),
            (KEY_ASLEEP.into(), boolean(self.needs.asleep)),
            (KEY_JUST_ATE.into(), boolean(self.needs.just_ate)),
            (KEY_JOB_STATUS.into(), int(self.status.ordinal())),
            (KEY_WORKING.into(), boolean(self.working)),
            (KEY_LEISURE.into(), int(self.leisure)),
        ]);
        if let Some(at) = self.respawn {
            data.insert(KEY_RESPAWN.into(), position(at));
        }
        if let Some(job) = self.posting.job {
            let mut compound = Compound::from([(KEY_JOB_TYPE.into(), text(job.path()))]);
            if let Some(at) = self.work_at {
                compound.insert(KEY_JOB_WORK.into(), position(at));
            }
            data.insert(KEY_JOB.into(), nested(&compound));
        }
        data
    }

    /// Read back what [`Citizen::to_compound`] wrote, plus Java's defaults.
    ///
    /// **`saturation` absent reads as `0.0` because Java's `getDouble` does**
    /// (`:1460`, with no `contains` guard, unlike `asleep` and `justAte`): a save
    /// predating the key starves its colony. The port keeps that instead of
    /// inventing a floor.
    pub fn from_compound(data: &Compound) -> Result<Citizen, Refusal> {
        let id = i32_of(data, KEY_ID)?;
        let status = match data.get(&KEY_JOB_STATUS.into()) {
            None => JobStatus::Idle,
            Some(_) => {
                let value = i32_of(data, KEY_JOB_STATUS)?;
                JobStatus::from_ordinal(value).ok_or(Refusal::UnknownStatus { value })?
            }
        };
        let mut citizen = Citizen {
            id,
            name: text_or_empty(data, KEY_NAME)?,
            suffix: text_or_empty(data, KEY_SUFFIX)?,
            voice: i32_of(data, KEY_SOUND_PROFILE).unwrap_or(0),
            female: bool_of(data, KEY_FEMALE).unwrap_or(false),
            child: bool_of(data, KEY_CHILD).unwrap_or(false),
            paused: bool_of(data, KEY_PAUSED).unwrap_or(false),
            texture: i32_of(data, KEY_TEXTURE).unwrap_or(0),
            skills: match data.get(&KEY_SKILLS.into()) {
                Some(Tag::Compound(skills)) => Skills::from_compound(skills)?,
                Some(_) => return Err(Refusal::Field { key: KEY_SKILLS }),
                None => Skills::new(),
            },
            status,
            posting: Posting::jobless(),
            work_at: None,
            home: None,
            last_position: pos_or_zero(data, KEY_POS),
            respawn: pos_of(data, KEY_RESPAWN)?,
            bed: Some(pos_or_zero(data, KEY_BEDS)).filter(|at| *at != NO_POSITION),
            working: bool_of(data, KEY_WORKING).unwrap_or(false),
            leisure: i32_of(data, KEY_LEISURE).unwrap_or(0),
            happiness: Happiness::new(),
            needs: Needs::new(),
        };
        citizen.needs.just_ate = bool_of(data, KEY_JUST_ATE).unwrap_or(false);
        citizen.needs.asleep = bool_of(data, KEY_ASLEEP).unwrap_or(false);
        citizen.needs.saturation = f64_of(data, KEY_SATURATION)
            .unwrap_or(0.0)
            .clamp(0.0, MAX_SATURATION);
        match data.get(&KEY_JOB.into()) {
            Some(Tag::Compound(job)) => {
                let kind = Job::from_path(&text_or_empty(job, KEY_JOB_TYPE)?)
                    .ok_or(Refusal::Field { key: KEY_JOB_TYPE })?;
                citizen.posting.job = Some(kind);
                citizen.work_at = pos_of(job, KEY_JOB_WORK)?;
            }
            Some(_) => return Err(Refusal::Field { key: KEY_JOB }),
            None => {}
        }
        citizen.check()?;
        Ok(citizen)
    }
}

/// An integer under `key`. Java's `getInt` answers `0` for an absence and for a
/// wrong kind, which is why a citizen with no `id` reads as citizen zero there and
/// is discarded without a word.
fn i32_of(data: &Compound, key: &'static str) -> Result<i32, Refusal> {
    match data.get(&key.into()) {
        Some(Tag::Int(value)) => Ok(*value),
        Some(Tag::Byte(value)) => Ok(i32::from(*value)),
        Some(Tag::Short(value)) => Ok(i32::from(*value)),
        Some(_) | None => Err(Refusal::Field { key }),
    }
}

fn int_in(tag: &Tag, key: &'static str) -> Result<i32, Refusal> {
    match tag.get(key) {
        Some(Tag::Int(value)) => Ok(*value),
        Some(_) | None => Err(Refusal::Field { key }),
    }
}

fn double_in(tag: &Tag, key: &'static str) -> Result<f64, Refusal> {
    match tag.get(key) {
        Some(Tag::Double(bits)) => Ok(f64::from_bits(*bits)),
        Some(_) | None => Err(Refusal::Field { key }),
    }
}

/// A string under `key`, `""` when the key is not there (Java's `getString` too),
/// and a refusal when it is there as something else.
fn text_or_empty(data: &Compound, key: &'static str) -> Result<String, Refusal> {
    match data.get(&key.into()) {
        None => Ok(String::new()),
        Some(Tag::String(value)) => Ok(value.to_string_lossy()),
        Some(_) => Err(Refusal::Field { key }),
    }
}

fn bool_of(data: &Compound, key: &'static str) -> Option<bool> {
    match data.get(&key.into()) {
        Some(Tag::Byte(value)) => Some(*value != 0),
        Some(Tag::Int(value)) => Some(*value != 0),
        _ => None,
    }
}

fn f64_of(data: &Compound, key: &'static str) -> Option<f64> {
    match data.get(&key.into()) {
        Some(Tag::Double(bits)) => Some(f64::from_bits(*bits)),
        Some(Tag::Float(bits)) => Some(f64::from(f32::from_bits(*bits))),
        _ => None,
    }
}

/// A position that may be absent; `Ok(None)` only when the key is not there.
///
/// The three coordinates are read through `Tag::get`, not through
/// `BTreeMap::get`: a compound's key is a `JavaString` (UTF-16) and `&str` is not
/// one, so indexing it with a bare name does not type-check.
fn pos_of(data: &Compound, key: &'static str) -> Result<Option<[i32; 3]>, Refusal> {
    match data.get(&key.into()) {
        None => Ok(None),
        Some(at @ Tag::Compound(_)) => {
            let axis = |name: &str| match at.get(name) {
                Some(Tag::Int(value)) => Ok(*value),
                _ => Err(Refusal::Field { key }),
            };
            Ok(Some([axis("x")?, axis("y")?, axis("z")?]))
        }
        Some(_) => Err(Refusal::Field { key }),
    }
}

fn pos_or_zero(data: &Compound, key: &'static str) -> [i32; 3] {
    pos_of(data, key).ok().flatten().unwrap_or(NO_POSITION)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::Kind;
    use crate::citizens::Refusal as EmploymentRefusal;

    fn citizen_job(job: Job) -> Compound {
        let mut data = Citizen::new(7, "Alma").to_compound();
        data.insert(
            KEY_JOB.into(),
            Tag::compound([
                (KEY_JOB_TYPE, Tag::String(job.path().into())),
                (KEY_JOB_WORK, position([4, 70, 4])),
            ]),
        );
        data
    }

    #[test]
    fn a_fresh_citizen_is_coherent_and_at_rest() {
        let data = Citizen::new(7, "Alma");
        assert_eq!(data.check(), Ok(()));
        assert_eq!(
            data.needs.saturation, MAX_SATURATION,
            "CitizenData.java:526"
        );
        assert_eq!(data.posting, Posting::jobless());
        assert_eq!(data.home, None, "the home is the residence hut's");
    }

    #[test]
    fn a_zero_id_and_an_empty_name_are_refused_by_name() {
        assert_eq!(Citizen::new(0, "Alma").check(), Err(Refusal::NoId));
        assert_eq!(Citizen::new(-3, "Alma").check(), Err(Refusal::NoId));
        assert_eq!(Citizen::new(7, "").check(), Err(Refusal::NoName));
        assert_eq!(Refusal::NoId.name(), "entity.refusal.no_id");
    }

    #[test]
    fn a_round_trip_keeps_every_key_this_slice_models() {
        let mut original = Citizen::new(7, "Alma");
        original.female = true;
        original.texture = 42;
        original.suffix = "_w".to_owned();
        original.voice = 3;
        original.needs.saturation = 12.5;
        original.needs.asleep = true;
        original.status = JobStatus::Stuck;
        original.working = true;
        original.leisure = 17;
        original.last_position = [10, 64, -3];
        original.bed = Some([10, 64, -4]);
        original.respawn = Some([1, 2, 3]);
        original
            .skills
            .set(Skill::Focus, 12, 44.5)
            .expect("a level");
        let read = Citizen::from_compound(&original.to_compound()).expect("its own compound");
        assert_eq!((read.id, read.texture, read.suffix.as_str()), (7, 42, "_w"));
        assert_eq!(read.needs.saturation, 12.5);
        assert!(read.needs.asleep);
        assert_eq!(read.status, JobStatus::Stuck);
        assert_eq!(
            (read.bed, read.respawn),
            (Some([10, 64, -4]), Some([1, 2, 3]))
        );
        assert_eq!(read.skills.experience(Skill::Focus), 44.5);
        assert_eq!(read.last_position, [10, 64, -3]);
    }

    #[test]
    fn the_job_travels_as_a_path_and_a_position_never_as_a_hut_kind() {
        let mut original = Citizen::new(7, "Alma");
        original.posting = Posting::employed(Kind::Lumberjack, Job::Lumberjack);
        original.work_at = Some([4, 70, 4]);
        let data = original.to_compound();
        let Some(Tag::Compound(job)) = data.get(&KEY_JOB.into()) else {
            panic!("the job is a compound");
        };
        assert_eq!(
            text_or_empty(job, KEY_JOB_TYPE),
            Ok("lumberjack".to_owned())
        );
        assert_eq!(pos_of(job, KEY_JOB_WORK), Ok(Some([4, 70, 4])));
        let read = Citizen::from_compound(&data).expect("its own compound");
        assert_eq!(read.posting.job, Some(Job::Lumberjack));
        assert_eq!(read.posting.hut, None, "the kind is the colony's answer");
        assert_eq!(read.work_at, Some([4, 70, 4]));
    }

    #[test]
    fn a_loaded_job_is_wired_from_the_hut_side_and_only_into_a_hut_that_hires_it() {
        let mut read = Citizen::from_compound(&citizen_job(Job::Lumberjack)).expect("a forester");
        assert_eq!(
            read.check(),
            Ok(()),
            "a job with no kind is Java's loaded state"
        );
        assert_eq!(
            read.wire_work(Kind::Farmer),
            Err(Refusal::Posting(EmploymentRefusal::NotEmployed(
                crate::jobs::employ(Kind::Farmer, Job::Lumberjack).unwrap_err()
            )))
        );
        read.wire_work(Kind::Lumberjack).expect("its own hut");
        assert_eq!(
            read.posting,
            Posting::employed(Kind::Lumberjack, Job::Lumberjack)
        );
        assert_eq!(read.check(), Ok(()));
    }

    #[test]
    fn a_hut_kind_without_a_position_is_refused() {
        let mut data = Citizen::new(7, "Alma");
        data.posting = Posting::employed(Kind::Builder, Job::Builder);
        assert_eq!(
            data.check(),
            Err(Refusal::WorkWithoutPosition { hut: Kind::Builder })
        );
        assert_eq!(
            Refusal::WorkWithoutPosition { hut: Kind::Builder }.name(),
            "entity.refusal.work_without_position"
        );
    }

    #[test]
    fn a_stray_job_status_is_refused_rather_than_read_as_idle() {
        let mut data = Citizen::new(7, "Alma").to_compound();
        data.insert(KEY_JOB_STATUS.into(), Tag::Int(9));
        assert_eq!(
            Citizen::from_compound(&data),
            Err(Refusal::UnknownStatus { value: 9 })
        );
        assert_eq!(JobStatus::from_ordinal(-1), None);
        assert_eq!(JobStatus::from_ordinal(2), Some(JobStatus::Stuck));
    }

    #[test]
    fn a_compound_without_saturation_starves_its_citizen_because_java_does() {
        let mut data = Citizen::new(7, "Alma").to_compound();
        data.remove(&KEY_SATURATION.into());
        let read = Citizen::from_compound(&data).expect("legal NBT");
        assert_eq!(read.needs.saturation, 0.0, "CitizenData.java:1460");
    }

    #[test]
    fn a_bed_written_as_zero_and_an_absent_bed_both_read_as_no_bed() {
        let mut data = Citizen::new(7, "Alma").to_compound();
        assert_eq!(data.get(&KEY_BEDS.into()), Some(&position(NO_POSITION)));
        assert_eq!(Citizen::from_compound(&data).expect("zero bed").bed, None);
        data.remove(&KEY_BEDS.into());
        assert_eq!(Citizen::from_compound(&data).expect("no bed").bed, None);
    }

    #[test]
    fn a_wrong_kind_on_a_key_is_a_named_refusal_not_a_zero() {
        let mut data = Citizen::new(7, "Alma").to_compound();
        data.insert(KEY_ID.into(), Tag::String("seven".into()));
        assert_eq!(
            Citizen::from_compound(&data),
            Err(Refusal::Field { key: KEY_ID })
        );
        data.insert(KEY_ID.into(), Tag::Int(7));
        data.insert(KEY_NAME.into(), Tag::Int(1));
        assert_eq!(
            Citizen::from_compound(&data),
            Err(Refusal::Field { key: KEY_NAME })
        );
        assert_eq!(
            Refusal::Field { key: KEY_ID }.name(),
            "entity.refusal.field"
        );
    }

    #[test]
    fn an_unknown_job_path_is_refused_and_the_placeholder_is_a_job() {
        let mut data = Citizen::new(7, "Alma").to_compound();
        data.insert(
            KEY_JOB.into(),
            Tag::compound([(KEY_JOB_TYPE, Tag::String("astronaut".into()))]),
        );
        assert_eq!(
            Citizen::from_compound(&data),
            Err(Refusal::Field { key: KEY_JOB_TYPE })
        );
        let placeholder = Citizen::from_compound(&citizen_job(Job::Placeholder)).expect("a job");
        assert_eq!(placeholder.posting.job, Some(Job::Placeholder));
        assert_eq!(placeholder.check(), Ok(()), "JobPlaceholder names no hut");
    }

    #[test]
    fn the_eleven_skills_keep_their_ordinals_and_the_index_is_the_ordinal() {
        assert_eq!(Skill::ALL.len(), 11, "Skill.java:10-150");
        for (index, skill) in Skill::ALL.iter().enumerate() {
            assert_eq!(Skills::index(*skill), index, "{skill:?} is out of order");
        }
    }

    #[test]
    fn a_skill_level_is_clamped_and_a_negative_experience_is_refused() {
        let mut skills = Skills::with_levels([120, -4, 5, 1, 1, 1, 1, 1, 1, 1, 1]);
        assert_eq!(skills.level(Skill::Athletics), MAX_CITIZEN_LEVEL, ":146");
        assert_eq!(skills.level(Skill::Dexterity), 1);
        assert_eq!(
            skills.set(Skill::Mana, 5, -1.0),
            Err(Refusal::Field { key: KEY_SKILLS })
        );
        assert_eq!(
            skills.set(Skill::Mana, 500, 0.0),
            Ok(()),
            "clamped, not refused"
        );
        assert_eq!(skills.level(Skill::Mana), MAX_CITIZEN_LEVEL);
    }

    #[test]
    fn a_compound_with_no_skills_keeps_the_floor_and_a_partial_list_keeps_the_rest() {
        assert_eq!(Skills::from_compound(&Compound::new()), Ok(Skills::new()));
        let partial = Compound::from([(
            KEY_LEVEL_MAP.into(),
            Tag::List(vec![Tag::compound([
                (KEY_SKILL, Tag::Int(10)),
                (KEY_LEVEL, Tag::Int(30)),
                (KEY_EXPERIENCE, Tag::Double(1.5f64.to_bits())),
            ])]),
        )]);
        let skills = Skills::from_compound(&partial).expect("one skill");
        assert_eq!(skills.level(Skill::Intelligence), 30);
        assert_eq!(
            skills.level(Skill::Athletics),
            1,
            "the ten others keep the floor"
        );
    }

    #[test]
    fn the_roster_line_is_the_id_and_the_name_and_a_bed_is_a_sleep() {
        let mut data = Citizen::new(7, "Alma");
        assert_eq!(data.roster(), crate::colony::Citizen::new(7, "Alma"));
        data.sleep_in([1, 65, 1]);
        assert_eq!(data.bed, Some([1, 65, 1]));
        assert!(data.needs.asleep);
        let read = Citizen::from_compound(&data.to_compound()).expect("its own compound");
        assert_eq!((read.bed, read.needs.asleep), (Some([1, 65, 1]), true));
    }
}
