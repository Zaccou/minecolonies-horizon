//! The colony manager: who owns which position, and what a colony leaves behind.
//!
//! Java reference: `core/colony/ColonyManager.java` (875 lines),
//! `api/colony/IColonyManager.java` (456 lines), `api/colony/IColonyTagCapability.java`
//! (the per-chunk claims) and `core/util/ChunkDataHelper.java` (the ground).
//!
//! | File | Question |
//! | --- | --- |
//! | `claims` | what a chunk says: who owns it, and which buildings put a colony there |
//! | `index` | **which colony owns this block, chunk or entity** — and the named refusal when none does |
//! | `proximity` | how far two colonies must stay apart, and how far one may reach |
//! | `lifecycle` | the away timer, and what a deletion takes down |
//! | `persist` | what is written, what is read back, and what a bad save is refused for |
//!
//! **Pure data, pure rules**, like `crate::colony` and `crate::founding`: no host
//! call, no clock, no thread, no file. That is what lets the four questions be
//! answered in a test with no world, and what a wasm mod gets anyway — the four
//! hooks in `crate::lib` own *when*, this module owns *what*.
//!
//! **Three premises, all of them measurable, and one of them false in Java.**
//!
//! 1. **The claims live here, not on the chunk.** Java attaches them to the
//!    `LevelChunk` (`IColonyTagCapability`), where the chunk's own save carries
//!    them. The SDK's chunk handle has nowhere to hang a mod's data
//!    (`crates/horizon-mod-vocab/src/host/levels.rs:13-17`) and exposes no chunk
//!    NBT, so [`ClaimBook`] holds the same three fields and [`persist`] saves them
//!    beside the colonies. Consequence: a chunk claimed by a mod version that did
//!    not save them comes back unclaimed, and [`Manager`]'s own box fallback is what
//!    keeps its colony answering.
//! 2. **There is no colony file to write.** Java owns `minecolonies/colonies.dat`
//!    and one `colony<id>.dat` per colony; a wasm mod has no `std::fs`. The port
//!    produces the same compound and leaves the placement to the caller — today the
//!    main building's block entity (`crate::wiring`), which is where MineColonies
//!    anchors a colony in the world anyway (`TileEntityColonyBuilding`).
//! 3. **Nothing deletes a colony on its own.** `getColoniesAbandonedSince` has no
//!    caller in the Java tree of this version and `canBeAutoDeleted` is only printed
//!    by `/colony printstats`: the port answers the question and never sweeps
//!    (`lifecycle`).
//!
//! **Two things this module deliberately does not port**, each with its cost in
//! `MANQUANT.md`: the whole client half of `ColonyManager` (the view cache, the
//! packet handlers, the sound manager — a port with no view packets has nothing to
//! answer), and the recipe/compatibility managers the Java `write` also saves.
//!
//! **The one rule of the port's own** is [`Manager`]'s box fallback: a chunk nobody
//! claimed still belongs to the colony whose founding box covers it. Java has no
//! such rule; it is what keeps a record read out of a block entity — with no claims
//! beside it — from answering "nobody" for its own town hall.

pub mod claims;
pub mod index;
pub mod lifecycle;
pub mod persist;
pub mod proximity;

pub use claims::{Chunk, ChunkClaims, ClaimBook, RemoveOutcome};
pub use index::{LookupRefusal, Manager};
pub use lifecycle::{Attendance, Dissolution, TICKS_HOUR, UPDATE_STATE_INTERVAL};
pub use persist::{Duplicate, LoadRefusal, Loaded, Skip, Skipped};
pub use proximity::{Proximity, Reach};

/// The id that means "no colony", Java's `NO_COLONY_ID`
/// (`ColonyManagerConstants.java:64-67`). Written as the owner of a chunk nobody
/// owns (`IColonyTagCapability.java:292-297`) and as the id of a record that has
/// been deleted (`crate::wiring`, *Where a colony lives*).
pub const NO_COLONY_ID: i32 = 0;
