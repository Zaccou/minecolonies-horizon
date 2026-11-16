//! The transverse floor: identities, geometry, constants, territory, claims.
//!
//! Java reference: `api/util/BlockPosUtil.java` (1 243 lines, every distance the
//! mod computes), `api/util/MathUtils.java` (66), `api/util/constant/`
//! (`Constants`, `CitizenConstants`, `PathingConstants`, `ColonyManagerConstants`,
//! and the claim settings in `api/configuration/ServerConfiguration.java`),
//! `api/colony/IColonyRelated.java` (26),
//! `api/colony/requestsystem/location/ILocation.java` (37), and the two files
//! that decide what land is: `api/colony/IColonyTagCapability.java` (407, the
//! per-chunk claim record) and `core/util/ChunkDataHelper.java` (476, which
//! chunks a colony sweeps).
//!
//! | File | Question |
//! | --- | --- |
//! | `ident` | what it is to be *in* a colony, *at* a position, *identified* |
//! | `constants` | the numbers Java hard-codes, each with its line |
//! | `coords` | block ↔ chunk ↔ section, and a square of chunks |
//! | `geom` | the seven distances Java has, and the two boxes |
//! | `territory` | where a colony's land is, and where a new one may be founded |
//! | `claims` | who owns a chunk, as the chunk capability decides it |
//!
//! **No host call anywhere here**, like `colony`, `inventory` and `pathfinding`:
//! this module carries data and rules, and every one of them is checked on the
//! host target with no game, no wasm and no prepared resources.
//!
//! **Three decisions that shape the rest.**
//!
//! 1. **A position is `[x, y, z]`, not a type of our own.** [`Pos`] is
//!    `[i32; 3]`, the alias `pathfinding::node::Pos` already uses and the shape
//!    `CommandArgumentValue::BlockPos` hands a command
//!    (`src/hire.rs:113`). The SDK has `mod_api::BlockPos` with `.chunk()` and
//!    `.section()`
//!    (`.horizon/sdk/api/crates/horizon-core/src/position.rs:41-56`), but using
//!    it would put a conversion at every boundary with the modules that already
//!    speak arrays; `coords` re-implements only the three floor-divisions, and
//!    cites the SDK lines it mirrors.
//! 2. **Being "in" a colony is a chunk claim, not a radius.** Java answers
//!    `isCoordInColony` by asking the chunk who owns it
//!    (`core/colony/Colony.java:1298-1308`) — the square of chunks the colony
//!    swept at founding (`core/util/ChunkDataHelper.java:135-139`) then grew by
//!    building. [`Territory`] is the square; [`ChunkClaims`] is the answer.
//!    The *circle* also exists in Java, for a different question — "is this
//!    place far enough from that colony to found another"
//!    (`core/colony/ColonyManager.java:311-320`) — and the two disagree at the
//!    corners of the square. Both are here, named for the question they answer.
//! 3. **A colony is named by its id, never by a borrowed object.** Java's
//!    `IColonyRelated.getColony()` hands out a live `IColony` (`:18`); the port
//!    keeps the `i32` and lets the caller resolve it, because there is no colony
//!    registry in a wasm mod and no `Rc` to build a cycle with.
//!
//! **What is not here**, and what each gap costs:
//!
//! - **No `registerWithColony()`** (`api/colony/IColonyRelated.java:11`): it
//!   inserts the entity into the colony's own citizen or raider list
//!   (`core/entity/citizen/citizenhandlers/CitizenColonyHandler.java:81`), which
//!   needs a colony registry the mod does not have.
//! - **No dirty flag.** Every setter in the claim capability ends on
//!   `chunk.setUnsaved(true)` (`api/colony/IColonyTagCapability.java:142`, `:166`);
//!   a mod has no chunk handle to mark, so [`ChunkClaims`] reports the change to
//!   its caller instead.
//! - **No NBT.** The claim record has a codec (`:312-326`, tags `id`, `colonies`,
//!   `buildingsClaim`, then `buildings`/`building` per position —
//!   `api/util/constant/NbtTagConstants.java:8`, `:41`, `:17`, `:13`, `:16`);
//!   `colony`, `building` and `requests` each grew their own position codec
//!   already, and a fourth copy here would not be the one those three call.
//! - **No `Level`.** `isCoordInColony` takes the level to compare dimensions
//!   (`Colony.java:1300`); the port compares the dimension *name*, which is what
//!   `colony::Colony::dimension` already stores.

pub mod claims;
pub mod constants;
pub mod coords;
pub mod geom;
pub mod ident;
pub mod territory;

pub use claims::ChunkClaims;
pub use constants::{
    BLOCKS_PER_CHUNK, INITIAL_COLONY_SIZE, MAX_COLONY_SIZE, MIN_COLONY_DISTANCE, NO_COLONY_ID,
    RANGE_TO_BE_HOME,
};
pub use coords::{ChunkPos, ChunkSquare, SectionPos, chunk_of, section_of};
pub use geom::{
    BlockBox, clamp, dist, dist_2d, dist_2d_max, dist_2d_sum, dist_manhattan, dist_sqr,
    dist_sqr_2d,
};
pub use ident::{Dimension, Identified, InColony, Positioned, Site, is_reachable};
pub use territory::{FoundingLimits, FoundingRefusal, Territory, founding_allowed};

/// A block position: `[x, y, z]`, integer block coordinates.
///
/// The same alias `pathfinding::node::Pos` declares (`src/pathfinding/node.rs:29`):
/// two names for one array type, which `Copy`, orders and keys a `BTreeMap` with
/// no wrapper. Whichever a caller writes, the other accepts it — the port has one
/// position type, spelled two ways.
pub type Pos = [i32; 3];
