//! Four managers of Java's colony tree: the raid, the animals, and the two
//! registries that turn a saved name back into an object.
//!
//! Java reference: `core/colony/events/raid/RaidManager.java` (1 253 lines),
//! `core/colony/managers/AnimalManager.java` (447),
//! `core/colony/jobs/registry/JobDataManager.java` (93) and
//! `core/colony/interactionhandling/registry/InteractionResponseHandlerManager.java`
//! (49).
//!
//! | File | Question | Java |
//! | --- | --- | --- |
//! | [`raid`] | may a raid happen, how many and how hard, how many nights, what a death costs, what the colony remembers | `RaidManager.java:74-460`, `:700-1103` |
//! | [`animal`] | which id an animal may attach to, which id a new one takes, what a re-registration discards, what the save holds | `AnimalManager.java:74-446` |
//! | [`jobdata`] | which job a saved name is, and what a name that resolves to nothing costs | `JobDataManager.java:27-93` |
//! | [`interaction`] | the seven chat kinds, which class each is rebuilt as, and what a bad `handlertype` costs | `InteractionResponseHandlerManager.java:22-49` |
//!
//! **What was already covered, and is therefore not duplicated here.**
//!
//! - **The raid's event machine is `crate::events::raid`** (`Lifecycle`,
//!   `Raid`, `Status`): the id counter, `addEvent`, `onColonyTick`'s
//!   start/update/finish/drop, and the "active" statuses. [`raid`] does not
//!   repeat it — it *calls* it: `Manager::is_raided` walks `Lifecycle::ids()` and
//!   asks `Lifecycle::is_active`, which is `IColonyRaidEvent.isRaidActive()`.
//! - **The job registry is `crate::jobs`** (`Job`, 50 classes, their paths,
//!   their hut rows). [`jobdata`] ports the lookup `JobDataManager` performs on
//!   top of it, with `Job::from_path` as the registry.
//! - **The colony's own save is `crate::persistence`**, and it says in
//!   `src/persistence/MANQUANT.md:165` that `animalManager` (`Colony.java:942`)
//!   has no module to hand a compound to. [`animal`] is that module; its
//!   `write`/`read` speak the flat colony-level shape, not Java's double-wrapped
//!   one.
//!
//! **Three premises of the brief, checked against the sources.**
//!
//! 1. **"Port the rules, not the world" is possible for all four.** Right for
//!    [`raid`] (the spawn search, the biome ladder and the path job are the world
//!    and are listed in `MANQUANT.md`), [`jobdata`] and [`interaction`]. Right
//!    for [`animal`] except one thing: registration compares *object identity*
//!    (`AnimalManager.java:111`), which a value cannot, so [`animal::Managed`]
//!    carries `same_as_bound` and the caller says so.
//! 2. **"The order of the writes" is a rule, and one place breaks it.** The four
//!    pre-write refusals of `raiderEvent` are pinned by a test; the fifth
//!    (`:442-445`) happens **after** the history entry and the night reset,
//!    because Java's does, and that is tested too rather than tidied away.
//! 3. **"The AnimalManager save works" is false.** `Colony.write:940-942` wraps
//!    the manager's own `animalManager` key in a second one, and
//!    `Colony.read:772` hands `read` the inner compound, whose `:280` then looks
//!    for `animalManager` inside it and returns: **no animal survives a restart
//!    in Java**, silently. [`animal::Registry`] writes and reads the level
//!    `AnimalManager.read` was written for, and a test round-trips it. Details in
//!    the module note.
//!
//! **Pure data, pure rules, the same bargain as `crate::manager`, `crate::colony`
//! and `crate::events`**: no host call, no clock, no thread, no file. A raid's
//! clock is a `now: i64`, its coin is a `roll: f64` in `[0, 1)`, its world is a
//! slice of [`raid::Point`]; a job's or an interaction's decoding is a closure.
//! That is what lets thirty-odd rules be answered in a test with no game — and
//! what a wasm mod gets anyway, since it has no `Level` to ask.

pub mod animal;
pub mod interaction;
pub mod jobdata;
pub mod raid;

// The names that cannot be confused with another module's, re-exported; every
// `Refusal` and `Repair` of this slice stays qualified (`raid::Refusal`,
// `jobdata::NameRefusal`, …), because four managers having four refusal sets is
// the point of the split.
pub use animal::{Discard, Entry, Managed, Registry, Uuid, Verdict};
pub use interaction::{Kind, KindRefusal};
pub use jobdata::NameRefusal;
pub use raid::{
    Building, BuildingKind, Env, Finished, History, Manager, NightStep, Point, Settings, Spawn,
    SpawnResult,
};
