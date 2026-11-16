//! Blueprints: the pack's 9 515 `.blueprint` files, read and turned into a
//! structure the engine can place — **in memory, one building at a time**.
//!
//! # Why this module exists, in one paragraph
//!
//! MineColonies describes every building it builds as a Structurize blueprint:
//! `data/minecolonies/blueprints/minecolonies/**/*.blueprint`, 25 styles × the
//! building kinds × up to 5 levels, **9 515 files, 24 MiB gzipped — 338 MiB and
//! 47.6 million cells decompressed**. Turned into vanilla structure templates
//! ahead of time they weigh about **2 GB** (41 bytes per cell), which a mod
//! shipping as one `.hmod` cannot carry — so the port carries the pack and
//! converts a building when it needs it. Nothing is cached and nothing is written:
//! [`Blueprint::parse`] takes the bytes the pack stack handed over,
//! [`structure_nbt::convert`] returns a document, and both die with the
//! placement. The port does keep one *local* tree of converted templates —
//! `data/minecolonies/structure/` (9 515 files, 1.8 GB, git-ignored) — and it is
//! this converter's output, Modern dialect: that is where the format was
//! cross-checked, not something the mod ships.
//!
//! # The pipeline
//!
//! ```text
//! data/minecolonies/blueprints/…/*.blueprint        (the pack, 24 MiB)
//!   │  Host::resource(PackType::ServerData, …)       host/resources.rs:20
//!   ▼  bytes
//! Blueprint::parse(&bytes)                           format::Blueprint::parse
//!   │  gzip → NBT → the model
//!   ▼
//! structure_nbt::convert(&blueprint)                 → Conversion { structure, anchor }
//!   │
//!   ├── Conversion::to_compound()   the engine's tree, ready to hand back
//!   └── Conversion::to_nbt() / to_gzipped_nbt()   the bytes of a `.nbt` document
//! ```
//!
//! **The bytes are handed in, never read.** `Host::resource` is the one way a
//! mod sees a file (`crates/horizon-mod-vocab/src/host/resources.rs:20`), it
//! answers the winning bytes of a path in the server's data stack, and it caps
//! a resource at `MAX_RESOURCE_BYTES` = 64 MiB
//! (`crates/horizon-mod-vocab/src/client/pack_finders.rs:8`). The largest real
//! blueprint is 943 kB decompressed
//! (`original/infrastructure/mineshafts/mediumquarryshaft1`, 79 458 cells and
//! 3 928 block entities), so the cap is 68 times away.
//!
//! # Files
//!
//! | File | Question |
//! | --- | --- |
//! | `format` | what a `.blueprint` *is*, field by field, and the reader |
//! | `structure_nbt` | the conversion to vanilla's template NBT, and the anchor |
//! | `gzip` | the container the file is in (RFC 1952) |
//! | `inflate` | raw DEFLATE (RFC 1951), written out because no dependency may be added |
//! | `nbt` | the byte layer over the engine's own `Compound`/`Tag` |
//! | `corpus` | the real pack as the tests read it — **test-only** |
//!
//! # Two things this module does not decide
//!
//! * **Where the blueprint's bytes come from.** The caller asks the host once it
//!   knows the style, the building and the level; the path is content
//!   (`src/founding/mod.rs` and `src/network/founding.rs` already carry the pack
//!   and path shapes MineColonies' own messages use).
//! * **Turning the structure.** A blueprint is scanned with a facing; a hut
//!   placed the other way round needs a rotation, which the engine applies to a
//!   template, not to a blueprint (`Blueprint.java:628-770` is Java's). The
//!   converter emits the structure **as scanned** and says so.
//!
//! What is missing from the SDK for the step *after* the conversion — handing a
//! `Structure` to a placement path without a registry round trip — is written
//! down, with what it would take, in `MANQUANT.md`.

pub mod format;
pub mod gzip;
pub mod inflate;
pub mod nbt;
pub mod structure_nbt;

#[cfg(test)]
mod corpus;

pub use format::{
    fnv1a64, Block, BlockEntity, BlockState, Blueprint, Error, Pos, Size, StateRef, MAX_CELLS,
};
pub use structure_nbt::{
    convert, convert_to_gzipped_nbt, Anchor, AnchorSource, Conversion, ConvertError,
    PaletteDialect, Structure, StructureBlock, StructureEntity, CURRENT_DATA_VERSION,
};
