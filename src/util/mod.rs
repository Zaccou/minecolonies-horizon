//! The shared utilities: what `core/util` *decides*, as functions.
//!
//! Java reference: `com.minecolonies.core.util` — 23 files, 4 104 lines, of which
//! `BackUpHelper` (640) and `ChunkDataHelper` (476) are the two halves no
//! smaller module can absorb.
//!
//! # What is portable here, and what is not
//!
//! Two thirds of these files take a `Level`, an `AbstractEntityCitizen` or a
//! `ServerPlayer` and *do* something to it: `TeleportHelper.colonyTeleport`
//! moves a player, `CitizenItemUtils.hitBlockWithToolInHand` sends a packet and
//! plays a sound, `WorkerUtil.updateLevelSign` writes a sign. A wasm mod has no
//! such handle — the `Host` hands out parts, not live entities
//! (`crates/horizon-mod-vocab/src/host/`) — so copying those bodies would mean
//! writing fiction.
//!
//! What *is* portable is the rule each body computes on the way: which block a
//! tool can harvest, how far a rotation may turn in one tick, which slot an
//! inventory sort fills next, what counts as "too many unrelated items". Every
//! file below is that layer, and every function is checked on the bare host
//! target with no game, no wasm and no prepared resources.
//!
//! # Nothing is ported twice
//!
//! Three of the twenty-three files were already ported elsewhere, and this
//! module **calls** them instead of growing a second copy — the row says which:
//!
//! | Java | Already in | Used here by |
//! | --- | --- | --- |
//! | `ExperienceUtils.getXPNeededForNextLevel` (`:68-77`) | [`crate::jobs::experience`] | [`experience`] |
//! | `ChunkDataHelper` (`:1-476`) | [`crate::base::coords`], [`crate::base::territory`], [`crate::base::claims`] | [`chunkpos`] |
//! | `MutableChunkPos.getMinBlockX` and friends | [`crate::base::coords::ChunkPos`] | [`chunkpos`] |
//!
//! And the files with nothing left to decide are in `MANQUANT.md` rather than in
//! fake code: `RecipeHandler` is an empty method with two `todo`s
//! (`RecipeHandler.java:14-16`), `MultimapCollector` and `CollectorUtils` are one
//! `Vec` and one `shuffle` each, and `DomumOrnamentumUtils` needs a block type this
//! SDK does not export.
//!
//! # The files
//!
//! | File | Question it answers | Java |
//! | --- | --- | --- |
//! | [`experience`] | what fraction of a level is done, which level some XP is | `ExperienceUtils` (94) |
//! | [`worker`] | which tool, which harvest level, which way to look, too many items | `WorkerUtil` (408) |
//! | [`building`] | which jobs a hut allows, how far it is turned | `BuildingUtils` (211) |
//! | [`chunkpos`] | region, local index and chessboard distance of a chunk | `MutableChunkPos` (162) |
//! | [`sorting`] | in which order a warehouse sorts, and how many slots that takes | `SortingUtils` (193) |
//! | [`schem`] | what a blueprint costs, and what a rename has to rewrite | `SchemAnalyzerUtil` (248), `SchemFixerUtil` (145) |
//! | [`attributes`] | which modifier to remove, and what that does to health | `AttributeModifierUtils` (121) |
//! | [`server`] | which online player is this id, which of them manage the colony | `ServerUtils` (150), `AdvancementUtils` (32) |
//! | [`teleport`] | where a teleport lands, and the binary search for open sky | `TeleportHelper` (179) |
//! | [`furnace`] | what a smelting recipe gives, and which recipe gives this | `FurnaceRecipes` (99) |
//! | [`backup`] | when to back up, what to name it, which ones to keep | `BackUpHelper` (640) |
//! | [`citizen_item`] | how much armour a hit costs, how much XP mending eats | `citizenutils/CitizenItemUtils` (357) |
//! | [`raider`] | which raider a gold block in a ship spawns | `CreativeRaiderStructureHandler` (224) |
//! | [`json`] | a JSON field with a default, and the id that has to be valid | `GsonHelper` (45) |
//!
//! # Three rules that shape the rest
//!
//! 1. **A position is `[i32; 3]` and a chunk is [`crate::base::coords::ChunkPos`].**
//!    Same choice as `base`, for the same reason: every module already speaks
//!    arrays, and converting at each boundary costs more than the three
//!    floor-divisions `base::coords` re-implements.
//! 2. **A refusal is named, never a panic.** No `unwrap()` on anything that came
//!    from a file or a caller: a bad blueprint, an unknown facing or an empty
//!    recipe table answers `None` or an `Err` with a name. The one deliberate
//!    exception is a *non-finite* number where Java would hang — [`experience::calculate_level`]
//!    refuses it, and the test says so.
//! 3. **The Java answer wins, even when it is wrong.** [`experience::calculate_level`]
//!    is dead code in Java *and* disagrees with the live loop; it is ported as
//!    written, with the disagreement pinned by a test, because a port that
//!    silently "fixes" Java is a port that cannot be diffed against it.

pub mod attributes;
pub mod backup;
pub mod building;
pub mod chunkpos;
pub mod citizen_item;
pub mod experience;
pub mod furnace;
pub mod json;
pub mod raider;
pub mod schem;
pub mod server;
pub mod sorting;
pub mod teleport;
pub mod worker;

pub use worker::MIDDLE_BLOCK_OFFSET;
