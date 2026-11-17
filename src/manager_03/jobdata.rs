//! The job of a saved citizen: which registry id it names, what a bad id costs,
//! and the order in which Java builds then restores it.
//!
//! Java reference: `core/colony/jobs/registry/JobDataManager.java` (93 lines),
//! read by `core/colony/CitizenData.java:1462-1465` (the file) and written by
//! `core/colony/jobs/AbstractJob.java:192` (`serializeNBT`) and `:233`
//! (`writeToBuffer`).
//!
//! **The registry is not duplicated here.** `IJobRegistry` is a Forge registry of
//! `JobEntry` (`api/colony/jobs/registry/JobEntry.java:20-140`); its content is
//! already ported as [`crate::jobs::Job`], which carries the 50 classes, their
//! paths and their translation keys. What this file ports is the **lookup and
//! its failures**, which `Job` has no room for: a name that resolves to nothing,
//! a producer that answers `null`, and a restore that throws.
//!
//! **The saved name is a full id, and a bare path is a trap.** `:192` writes
//! `getJobRegistryEntry().getKey().toString()`, i.e. `minecolonies:builder`. The
//! reader does `new ResourceLocation(jobTypeName)` (`:31-32`), and a
//! `ResourceLocation` built from a string with **no colon** gets the `minecraft`
//! namespace: a save holding `builder` asks the registry for `minecraft:builder`,
//! which no job is, and `:38-42` logs and returns `null` — **the citizen loses
//! its job, silently, for a missing namespace**. [`job_of`] names that case
//! ([`NameRefusal::Foreign`]) instead of folding it into "unknown".
//!
//! **Three dead checks in Java, kept as decisions here.**
//!
//! 1. `:34-37` tests `jobType == null`, which a `ResourceLocation` never is.
//! 2. `:44` re-wraps `jobEntry` in `Optional.ofNullable` **after** `:38` already
//!    returned when it was `null`.
//! 3. The `else` at `:64-67` ("Unknown Job type or missing constructor") can only
//!    be reached when `produceJob` answered `null`, because the registry miss was
//!    answered at `:38`. That is [`Refusal::NoProducer`] here.
//!
//! **What a refusal costs differs between the two halves**, and the difference is
//! Java's:
//!
//! | Half | A refused job costs |
//! | --- | --- |
//! | the file (`CitizenData.java:1462-1465`, no `try`) | **nothing** — `createFrom` answers `null` and `setJob(null, true)` takes it |
//! | the wire (`createViewFrom`, `:72-93`) | that job's view on one client |
//!
//! The one thing that does cost the colony is a `type` key that is **not a
//! string**: `getString` answers `""`, `new ResourceLocation("")` throws
//! (`:32-33`), and nothing between `:33` and `Colony.read` catches it. [`job_of`]
//! names it [`NameRefusal::Empty`].

use mod_api::{Compound, Tag};

use crate::jobs::Job;

/// The key the job type is saved under: `"type"` (`NbtTagConstants.java:574`).
pub const KEY_JOB_TYPE: &str = "type";

/// The key the whole job compound sits under in a citizen record
/// (`CitizenData.java:1462`).
pub const KEY_JOB: &str = "job";

/// The job a colony falls back to when the file names none
/// (`ModJobs.java:13`), as a bare path.
pub const PLACEHOLDER: &str = "placeholder";

/// The one namespace the port's registry has (`Constants.java:13`).
pub const NAMESPACE: &str = "minecolonies";

/// Why a saved or received job name did not resolve, named.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set for a name
pub enum NameRefusal {
    /// The key is present and not a usable string: `new ResourceLocation("")`
    /// throws at `:33`, outside any `catch`.
    Empty,
    /// More than one colon, or an empty half: the same throw, for a value that
    /// is not empty. (Vanilla also refuses characters outside
    /// `[a-z0-9_.-/]`; that check is in `ResourceLocation`, not in this tree.)
    Malformed { value: String },
    /// A namespace that is not the mod's — in practice `minecraft`, which is
    /// what a bare path becomes (`:31-32`).
    Foreign { namespace: String, path: String },
    /// `<mod>:<path>` where no job answers to `path` (`:38-42`).
    Unknown { path: String },
}

impl NameRefusal {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Empty => "job.refusal.empty",
            Self::Malformed { .. } => "job.refusal.malformed",
            Self::Foreign { .. } => "job.refusal.foreign_namespace",
            Self::Unknown { .. } => "job.refusal.unknown",
        }
    }
}

/// Why no job was built, or why the one built was thrown away, named.
///
/// `E` is the caller's own restore error: this module has no `Compound` decoder
/// for a job's internals (`AbstractJob.deserializeNBT` reads thirteen keys of its
/// own), so it owns the order and the caller owns the fields.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: Java's two null exits, plus the name
pub enum Refusal<E> {
    /// The name did not resolve (`:31-42`).
    Name(NameRefusal),
    /// `produceJob` answered `null` (`:44`, `:64-67`).
    NoProducer { job: Job },
    /// `deserializeNBT` threw (`:54-61`): "its state cannot be restored", and Java
    /// answers `null` rather than a half-restored job.
    StateLost { job: Job, cause: E },
}

impl<E> Refusal<E> {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Name(inner) => inner.name(),
            Self::NoProducer { .. } => "job.refusal.no_producer",
            Self::StateLost { .. } => "job.refusal.state_lost",
        }
    }

    /// The job the refusal is about, when the name did resolve.
    pub fn job(&self) -> Option<Job> {
        match self {
            Self::Name(_) => None,
            Self::NoProducer { job } | Self::StateLost { job, .. } => Some(*job),
        }
    }
}

impl<E: std::fmt::Debug> std::fmt::Display for Refusal<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// `new ResourceLocation(value)` followed by the registry lookup
/// (`:31-41`), without the registry: [`Job::from_path`] is it.
///
/// A value with no colon is **not** "one of ours": Java gives it the `minecraft`
/// namespace, so it is [`NameRefusal::Foreign`].
pub fn job_named(value: &str) -> Result<Job, NameRefusal> {
    let mut parts = value.split(':');
    let (namespace, path) = match (parts.next(), parts.next(), parts.next()) {
        (Some(path), None, _) => {
            if path.is_empty() {
                return Err(NameRefusal::Empty);
            }
            ("minecraft", path)
        }
        (Some(namespace), Some(path), None) => {
            if namespace.is_empty() || path.is_empty() {
                return Err(NameRefusal::Malformed {
                    value: value.to_owned(),
                });
            }
            (namespace, path)
        }
        _ => {
            return Err(NameRefusal::Malformed {
                value: value.to_owned(),
            });
        }
    };

    if namespace != NAMESPACE {
        return Err(NameRefusal::Foreign {
            namespace: namespace.to_owned(),
            path: path.to_owned(),
        });
    }
    Job::from_path(path).ok_or_else(|| NameRefusal::Unknown {
        path: path.to_owned(),
    })
}

/// The job a saved compound names (`:31-35`), with Java's default: **an absent
/// `type` key is the placeholder**, not a refusal.
pub fn job_of(compound: &Compound) -> Result<Job, NameRefusal> {
    match compound.get(&KEY_JOB_TYPE.into()) {
        None => Ok(Job::Placeholder),
        Some(Tag::String(value)) => job_named(&value.to_string_lossy()),
        // A `type` key of another kind: `getString` answers "" and the
        // constructor throws (`:32-33`), which nothing here catches.
        Some(_) => Err(NameRefusal::Empty),
    }
}

/// `createFrom` (`:27-70`), whole, with the job's own decoding handed in.
///
/// The order is the port's whole value, and a test below pins it: **resolve the
/// name, then build, then restore**. Nothing is built for a name that does not
/// resolve, and nothing is restored into a job that was not built.
pub fn load<T, E>(
    compound: &Compound,
    produce: impl FnOnce(Job) -> Option<T>,
    restore: impl FnOnce(&mut T, &Compound) -> Result<(), E>,
) -> Result<T, Refusal<E>> {
    let job = job_of(compound).map_err(Refusal::Name)?;
    let mut built = produce(job).ok_or(Refusal::NoProducer { job })?;
    match restore(&mut built, compound) {
        Ok(()) => Ok(built),
        Err(cause) => Err(Refusal::StateLost { job, cause }),
    }
}

/// `createViewFrom` (`:72-93`): the same three steps on a name that arrived over
/// the wire, with `AbstractJob.java:233` as the writer.
///
/// Two differences from the file half, both Java's: there is **no placeholder**
/// for a missing name (the buffer always carries one), and a producer that is
/// absent is a different failure — Java calls `getJobViewProducer().get()` on an
/// `Optional` (`:85`), which throws when it is empty. The port answers
/// [`Refusal::NoProducer`], since a view is what the producer makes.
pub fn load_view<T, E>(
    name: &str,
    produce: impl FnOnce(Job) -> Option<T>,
    decode: impl FnOnce(&mut T) -> Result<(), E>,
) -> Result<T, Refusal<E>> {
    let job = job_named(name).map_err(Refusal::Name)?;
    let mut built = produce(job).ok_or(Refusal::NoProducer { job })?;
    match decode(&mut built) {
        Ok(()) => Ok(built),
        Err(cause) => Err(Refusal::StateLost { job, cause }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved(name: &str) -> Compound {
        Compound::from([(KEY_JOB_TYPE.into(), Tag::String(name.into()))])
    }

    #[test]
    fn a_missing_type_is_the_placeholder_and_a_full_id_is_the_job() {
        // `:34-35`: no key, no refusal — the placeholder.
        assert_eq!(job_of(&Compound::new()), Ok(Job::Placeholder));
        // `AbstractJob.java:192` writes `getKey().toString()`.
        assert_eq!(job_of(&saved("minecolonies:builder")), Ok(Job::Builder));
        assert_eq!(job_of(&saved("minecolonies:chickenherder")), Ok(Job::ChickenHerder));
    }

    #[test]
    fn a_bare_path_is_another_namespace_and_is_named_as_such() {
        // `new ResourceLocation("builder")` is `minecraft:builder`, which the
        // registry does not hold — the citizen loses its job for a colon.
        assert_eq!(
            job_of(&saved("builder")),
            Err(NameRefusal::Foreign {
                namespace: "minecraft".to_owned(),
                path: "builder".to_owned(),
            })
        );
        assert_eq!(
            job_of(&saved("othermod:builder")),
            Err(NameRefusal::Foreign {
                namespace: "othermod".to_owned(),
                path: "builder".to_owned(),
            })
        );
        assert_eq!(
            NameRefusal::Foreign {
                namespace: "minecraft".to_owned(),
                path: "builder".to_owned(),
            }
            .name(),
            "job.refusal.foreign_namespace"
        );
    }

    #[test]
    fn an_empty_or_malformed_name_is_the_one_refusal_that_costs_the_colony() {
        // `getString` on a non-string answers "", and `new ResourceLocation("")`
        // throws at `:33` with nothing to catch it.
        assert_eq!(
            job_of(&Compound::from([(KEY_JOB_TYPE.into(), Tag::Int(3))])),
            Err(NameRefusal::Empty)
        );
        assert_eq!(job_of(&saved("")), Err(NameRefusal::Empty));
        assert_eq!(
            job_of(&saved("minecolonies:")),
            Err(NameRefusal::Malformed {
                value: "minecolonies:".to_owned()
            })
        );
        assert_eq!(
            job_of(&saved("a:b:c")),
            Err(NameRefusal::Malformed {
                value: "a:b:c".to_owned()
            })
        );
    }

    #[test]
    fn a_path_no_job_answers_to_is_unknown_including_the_deprecated_one() {
        assert_eq!(
            job_of(&saved("minecolonies:nosuchjob")),
            Err(NameRefusal::Unknown {
                path: "nosuchjob".to_owned()
            })
        );
        // `cookassistant` is in `ModJobs` but reuses `JobChef` and has no entry
        // of its own (`crate::jobs::job`, and `ModJobs.java:64-65`), so a save
        // naming it resolves to nothing rather than to a chef.
        assert_eq!(
            job_of(&saved("minecolonies:cookassistant")),
            Err(NameRefusal::Unknown {
                path: "cookassistant".to_owned()
            })
        );
    }

    #[test]
    fn nothing_is_built_before_the_name_resolves_and_nothing_restored_before_it_is_built() {
        let mut produced = 0;
        let mut restored = 0;

        let refused = load::<i32, ()>(
            &saved("minecolonies:nosuchjob"),
            |_| {
                produced += 1;
                Some(1)
            },
            |_, _| {
                restored += 1;
                Ok(())
            },
        );
        assert!(matches!(refused, Err(Refusal::Name(_))));
        assert_eq!((produced, restored), (0, 0));

        // A producer that answers nothing stops before the restore (`:64-67`).
        let refused = load::<i32, ()>(
            &saved("minecolonies:builder"),
            |_| None,
            |_, _| {
                restored += 1;
                Ok(())
            },
        );
        assert_eq!(refused, Err(Refusal::NoProducer { job: Job::Builder }));
        assert_eq!(restored, 0);

        // And the whole path, in order.
        let mut seen = Vec::new();
        let built = load(
            &saved("minecolonies:builder"),
            |job| {
                seen.push(job.path().to_owned());
                Some(String::from("job"))
            },
            |job: &mut String, compound: &Compound| {
                assert_eq!(compound, &saved("minecolonies:builder"));
                job.push_str(" restored");
                Ok::<(), ()>(())
            },
        );
        assert_eq!(built, Ok(String::from("job restored")));
        assert_eq!(seen, vec!["builder".to_owned()]);
    }

    #[test]
    fn a_state_that_cannot_be_restored_costs_the_job_and_the_refusal_says_which() {
        let refused = load(
            &saved("minecolonies:miner"),
            |_| Some(0u8),
            |_, _| Err("Unknown job: minecolonies:miner"),
        );
        assert_eq!(
            refused,
            Err(Refusal::StateLost {
                job: Job::Miner,
                cause: "Unknown job: minecolonies:miner",
            })
        );
        assert_eq!(
            Refusal::<&str>::StateLost {
                job: Job::Miner,
                cause: "x",
            }
            .job(),
            Some(Job::Miner)
        );
        assert_eq!(
            Refusal::<&str>::StateLost {
                job: Job::Miner,
                cause: "x",
            }
            .name(),
            "job.refusal.state_lost"
        );
    }

    #[test]
    fn the_wire_has_no_placeholder_and_the_same_three_steps() {
        // `:76-83`: an unknown name on the wire is an error log and a `null`
        // view, never a placeholder.
        assert_eq!(
            load_view::<i32, ()>("minecolonies:nosuchjob", |_| Some(1), |_| Ok(())),
            Err(Refusal::Name(NameRefusal::Unknown {
                path: "nosuchjob".to_owned()
            }))
        );
        assert_eq!(
            load_view::<i32, ()>("builder", |_| Some(1), |_| Ok(())),
            Err(Refusal::Name(NameRefusal::Foreign {
                namespace: "minecraft".to_owned(),
                path: "builder".to_owned(),
            }))
        );
        // `:85`: the view producer's `Optional.get()` throws when it is empty,
        // which the port names.
        assert_eq!(
            load_view::<i32, ()>("minecolonies:builder", |_| None, |_| Ok(())),
            Err(Refusal::NoProducer { job: Job::Builder })
        );
        // The decode runs after the build, on the value it produced.
        assert_eq!(
            load_view("minecolonies:builder", |_| Some(7i32), |view| {
                *view += 1;
                Ok::<(), ()>(())
            }),
            Ok(8)
        );
        let lost = load_view("minecolonies:builder", |_| Some(7i32), |_| Err("short buffer"));
        assert_eq!(
            lost,
            Err(Refusal::StateLost {
                job: Job::Builder,
                cause: "short buffer"
            })
        );
    }
}
