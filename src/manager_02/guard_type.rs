//! The six guard types, and what a "job name" resolves to.
//!
//! Java reference: `core/colony/buildings/registry/GuardTypeDataManager.java`
//! (20 lines), `api/colony/guardtype/GuardType.java`, the registry it reads,
//! `api/colony/guardtype/registry/IGuardTypeRegistry.java`, and the two files
//! that fill it: `api/colony/guardtype/registry/ModGuardTypes.java` (the six ids)
//! and `apiimp/initializer/ModGuardTypesInitializer.java:25-89` (their contents).
//!
//! **The manager is one lookup and one null check.** `getFrom` returns `null` for
//! a `null` name (`:12-16`) and otherwise hands the name to the registry, which
//! answers `null` for an id nobody registered (`:18`). Nothing is defaulted, and
//! nothing is logged: a caller that gets `null` has no guard type, and every
//! caller in this tree does its own `null` test.
//!
//! **The parameter is called `jobName` and it really is one — through a name that
//! reads like a mistake.** The registry `getValue` reads is
//! `minecolonies:guardtypes` (`ModGuardTypesInitializer.java:16`), keyed by the six
//! *guard type* ids. Those six are the six guard **job** ids one for one, because
//! `ModJobs` names the archer's constant `ARCHER_ID` and gives it the path
//! **`"ranger"`** (`api/colony/jobs/ModJobs.java:28`), exactly as
//! `ModBuildings.HOME_ID` is `"residence"`. So `getFrom(minecolonies:ranger)` is
//! the archer's guard type, `getFrom(minecolonies:archer)` is nothing at all —
//! there is no id `minecolonies:archer` in either registry — and the method's name
//! is honest. The test below holds both halves, because the field name and the
//! registry id disagreeing is the kind of thing a port "fixes" and breaks.
//!
//! **`getFrom` has no caller in this tree.** `grep -rn getGuardTypeDataManager`
//! finds the API proxy, the implementation field and the interface, and nothing
//! that calls `getFrom` — the guard type a building picks its guard from comes
//! through the *job*, not through this manager. The port keeps it and adds
//! [`for_job`], which is the same lookup for a caller that holds a [`Job`] rather
//! than an id; it is marked as an addition.
//!
//! **The six entries are the interesting content, so they are here in full**, one
//! field per `GuardType.Builder` call, with the initialiser line above each row.
//! Two things are worth reading twice: **three** entries sound like `"archer"`
//! (`ranger` `:41`, `marksman` `:52`, `cavalry` `:85`), and `cavalry` carries the
//! knight's two skills even though its job is ranged (`:80-89`) — neither is a port
//! error.

use crate::jobs::employment::Skill;
use crate::jobs::Job;
use mod_api::Identifier;

/// One entry of the `minecolonies:guardtypes` registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: six entries in Java, and a seventh needs its own initialiser line
pub struct GuardType {
    /// The registry path without its namespace: `ModGuardTypes.KNIGHT_ID`'s
    /// `getPath()` (`ModGuardTypes.java:11-16`), which is also the key the
    /// deferred register uses (`ModGuardTypesInitializer.java:25`).
    pub path: &'static str,
    /// The job this guard type produces, `ModJobs.<x>` (`:31`, `:42`, `:53`,
    /// `:64`, `:75`, `:86`) — and what `GuardType.isInstance` tests against
    /// (`GuardType.java:152-155`, `clazz.isInstance(job)`).
    pub job: Job,
    /// `JOB_KNIGHT`, `JOB_RANGER`, … (`JobTranslationConstants.java:12-22`).
    pub job_key: &'static str,
    /// `JOB_KNIGHT_BUTTON`, … (`JobTranslationConstants.java:25-35`).
    pub button_key: &'static str,
    /// The skill the guard's combat maths reads first (`Skill.java`, and the
    /// `.setPrimarySkill` call of its row).
    pub primary: Skill,
    /// The skill it reads second.
    pub secondary: Skill,
    /// The name the client's sound manager looks up: `"knight"`, `"archer"`,
    /// `"druid"` — three values for six types (`:30`, `:41`, `:52`, `:63`,
    /// `:74`, `:85`).
    pub sound: &'static str,
}

impl GuardType {
    /// The full registry id, `minecolonies:knight`, which is what a file holds
    /// and what `IGuardTypeRegistry.getInstance().getValue` is keyed by.
    pub fn identifier(&self) -> Identifier {
        Identifier::from_parts(super::NAMESPACE, self.path)
            .expect("the six paths below are lower-case ASCII, which the engine accepts")
    }

    /// Java's `clazz.isInstance(job)` (`GuardType.java:152-155`), answered by the
    /// job the entry produces.
    ///
    /// `JobKnight.class.isInstance(job)` and `job == Job::Knight` are the same
    /// question for the six jobs Java registers, because each guard type names
    /// its own job class and no other entry uses it. What the class test does
    /// *not* cover, and this does not either: a job that *extends* `JobKnight`.
    /// No such job exists in the tree.
    pub fn is_instance(&self, job: Job) -> bool {
        self.job == job
    }
}

/// The `minecolonies:guardtypes` registry, in the order
/// `ModGuardTypesInitializer`'s static block registers it (`:25-89`).
///
/// The order is the registry's, so a port that ever needs an index into it — the
/// network never sends one, but a test can — reads the same index Java would if
/// the game preserved registration order. Nothing in this module depends on it.
pub const GUARDS: &[GuardType] = &[
    // `ModGuardTypesInitializer.java:25-34`
    GuardType {
        path: "knight",
        job: Job::Knight,
        job_key: "com.minecolonies.job.knight",
        button_key: "com.minecolonies.coremod.gui.workerhuts.knight",
        primary: Skill::Adaptability,
        secondary: Skill::Stamina,
        sound: "knight",
    },
    // `ModGuardTypesInitializer.java:36-45`
    GuardType {
        path: "ranger",
        job: Job::Ranger,
        job_key: "com.minecolonies.job.ranger",
        button_key: "com.minecolonies.coremod.gui.workerhuts.ranger",
        primary: Skill::Agility,
        secondary: Skill::Adaptability,
        sound: "archer",
    },
    // `ModGuardTypesInitializer.java:47-56`
    GuardType {
        path: "marksman",
        job: Job::Marksman,
        job_key: "com.minecolonies.job.marksman",
        button_key: "com.minecolonies.coremod.gui.workerhuts.marksman",
        primary: Skill::Agility,
        secondary: Skill::Adaptability,
        sound: "archer",
    },
    // `ModGuardTypesInitializer.java:58-67`
    GuardType {
        path: "huscarl",
        job: Job::Huscarl,
        job_key: "com.minecolonies.job.huscarl",
        button_key: "com.minecolonies.coremod.gui.workerhuts.huscarl",
        primary: Skill::Adaptability,
        secondary: Skill::Stamina,
        sound: "knight",
    },
    // `ModGuardTypesInitializer.java:69-78`
    GuardType {
        path: "druid",
        job: Job::Druid,
        job_key: "com.minecolonies.job.druid",
        button_key: "com.minecolonies.coremod.gui.workerhuts.druid",
        primary: Skill::Mana,
        secondary: Skill::Focus,
        sound: "druid",
    },
    // `ModGuardTypesInitializer.java:80-89`
    GuardType {
        path: "cavalry",
        job: Job::Cavalry,
        job_key: "com.minecolonies.job.cavalry",
        button_key: "com.minecolonies.coremod.gui.workerhuts.cavalry",
        primary: Skill::Adaptability,
        secondary: Skill::Stamina,
        sound: "archer",
    },
];

impl GuardType {
    /// The entry a registry path names, or `None` — the same answer
    /// `IGuardTypeRegistry.getValue` gives (`GuardTypeDataManager.java:18`).
    pub fn from_path(path: &str) -> Option<&'static GuardType> {
        GUARDS.iter().find(|guard| guard.path == path)
    }

    /// The entry a full id names, or `None`. **The namespace is part of the
    /// key**, as it is for a forge registry: `mystery:knight` is not
    /// `minecolonies:knight`, and a bare `knight` parses as `minecraft:knight`
    /// and is not either.
    pub fn from_identifier(id: &Identifier) -> Option<&'static GuardType> {
        if id.namespace() != super::NAMESPACE {
            return None;
        }
        Self::from_path(id.path())
    }
}

/// Why a lookup produced nothing, named.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: two ways to fail, and Java has one — a null return
pub enum Refusal {
    /// The caller had no name at all: Java's `jobName == null`
    /// (`GuardTypeDataManager.java:12-16` returns `null` before the lookup).
    NoName,
    /// The name is an id the registry does not hold (`:18`).
    NotRegistered { id: String },
}

/// Java's `getFrom`, with its answer kept as an `Option` and its two failures
/// named (`GuardTypeDataManager.java:10-20`).
///
/// `None` in gives [`Refusal::NoName`]; an id nobody registered gives
/// [`Refusal::NotRegistered`]. Nothing is defaulted: a caller that has no guard
/// type has to say what it does instead.
pub fn get_from(id: Option<&Identifier>) -> Result<&'static GuardType, Refusal> {
    let Some(id) = id else {
        return Err(Refusal::NoName);
    };
    GuardType::from_identifier(id).ok_or_else(|| Refusal::NotRegistered {
        id: id.to_string(),
    })
}

/// The guard type a **job** has — the same six answers as [`get_from`], reached
/// without spelling an id.
///
/// **A port addition.** `getFrom` takes a `ResourceLocation` and the tree's callers
/// have a [`Job`]; this is the one-line bridge, and it is also the honest way to
/// write a test for the six ids, since it never goes through a string.
///
/// It answers `None` for the forty-four jobs that are not guards, and it is not a
/// fallback for [`get_from`]: a file holds an id and must go through that one.
pub fn for_job(job: Job) -> Option<&'static GuardType> {
    GUARDS.iter().find(|guard| guard.is_instance(job))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(text: &str) -> Identifier {
        Identifier::parse_namespaced(text).expect("a literal id parses")
    }

    #[test]
    fn the_registry_holds_the_six_ids_mod_guard_types_declares() {
        // `ModGuardTypes.java:11-16`: six `ResourceLocation` constants, and
        // `ModGuardTypesInitializer.java:25-89` registers every one of them —
        // unlike `ModBuildings.largeQuarry`, nothing here is commented out.
        assert_eq!(GUARDS.len(), 6);
        let paths: Vec<&str> = GUARDS.iter().map(|guard| guard.path).collect();
        assert_eq!(
            paths,
            ["knight", "ranger", "marksman", "huscarl", "druid", "cavalry"]
        );
    }

    #[test]
    fn every_entry_names_its_own_job_and_no_two_share_one() {
        for (position, guard) in GUARDS.iter().enumerate() {
            assert!(guard.is_instance(guard.job), "{}", guard.path);
            assert_eq!(
                guard.identifier(),
                id(&format!("minecolonies:{}", guard.path)),
                "{}",
                guard.path
            );
            for other in &GUARDS[position + 1..] {
                assert_ne!(guard.job, other.job, "{} and {}", guard.path, other.path);
                assert_ne!(guard.path, other.path);
            }
        }
    }

    #[test]
    fn a_null_name_is_a_refusal_and_not_a_default() {
        // `GuardTypeDataManager.java:12-16` returns null before touching the registry.
        assert_eq!(get_from(None), Err(Refusal::NoName));
    }

    #[test]
    fn an_unknown_id_is_refused_by_name() {
        // `:18`: `getValue` answers null, which the manager passes straight out.
        assert_eq!(
            get_from(Some(&id("minecolonies:unicorn"))),
            Err(Refusal::NotRegistered {
                id: "minecolonies:unicorn".to_owned()
            })
        );
    }

    #[test]
    fn the_archer_job_is_registered_as_ranger_and_the_registry_follows_the_id() {
        // `ModJobs.java:28`: `ARCHER_ID = new ResourceLocation(MOD_ID, "ranger")` —
        // the constant is named archer and the id is ranger, so there is no
        // `minecolonies:archer` in either registry.
        assert_eq!(
            get_from(Some(&id("minecolonies:archer"))),
            Err(Refusal::NotRegistered {
                id: "minecolonies:archer".to_owned()
            })
        );
        let ranger = get_from(Some(&id("minecolonies:ranger"))).expect("ranger is registered");
        assert_eq!(ranger.job, Job::Ranger);
        assert_eq!(ranger.job.path(), "ranger");
        assert_eq!(for_job(Job::Ranger), Some(ranger));
    }

    #[test]
    fn each_guard_types_id_is_its_jobs_registry_id() {
        // The six guard ids and the six guard job ids are the same six strings
        // (`ModGuardTypes.java:11-16` against `ModJobs.java:28-32`, `:59`).
        for guard in GUARDS {
            let job_id = Identifier::parse(&format!("minecolonies:{}", guard.job.path()))
                .expect("a registry path is an id");
            assert_eq!(guard.identifier(), job_id, "{}", guard.path);
            assert_eq!(
                get_from(Some(&guard.identifier())),
                Ok(guard),
                "the lookup a file makes finds it"
            );
        }
    }

    #[test]
    fn the_skills_and_sounds_are_the_initialisers_own() {
        let knight = GuardType::from_path("knight").expect("knight");
        assert_eq!((knight.primary, knight.secondary), (Skill::Adaptability, Skill::Stamina));
        assert_eq!(knight.sound, "knight");
        let druid = GuardType::from_path("druid").expect("druid");
        assert_eq!((druid.primary, druid.secondary), (Skill::Mana, Skill::Focus));
        // `:80-89`: the cavalry is ranged in combat but carries the knight's two
        // skills, and the archer's sound name.
        let cavalry = GuardType::from_path("cavalry").expect("cavalry");
        assert_eq!((cavalry.primary, cavalry.secondary), (Skill::Adaptability, Skill::Stamina));
        assert_eq!(cavalry.sound, "archer");
        let marksman = GuardType::from_path("marksman").expect("marksman");
        assert_eq!((marksman.primary, marksman.secondary), (Skill::Agility, Skill::Adaptability));
    }

    #[test]
    fn a_job_that_is_not_a_guard_has_no_type() {
        // Forty-four of the fifty jobs are not guards; the list is the port's
        // own and the answer Java gives is the same null.
        for job in [Job::Builder, Job::Baker, Job::Placeholder, Job::ArcherTraining] {
            assert_eq!(for_job(job), None, "{}", job.path());
        }
    }
}
