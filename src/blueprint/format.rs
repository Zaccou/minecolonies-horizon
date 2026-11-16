//! What a `.blueprint` *is*: the model, and the reader that fills it.
//!
//! This is the port of the standalone crate's `blueprint.rs`, adapted to this
//! package: the NBT tree is the engine's (`mod_api::{Compound, Tag}`), there is
//! no path (`Blueprint::parse` takes the bytes the pack stack gave the caller),
//! and every rejection is a named variant of [`Error`] instead of a `null`
//! return.
//!
//! # The format, field by field
//!
//! Citations are `file:line` into `strukturize-src`, the mod that *defines* the
//! format (this port's content comes from MineColonies, but the container is
//! Structurize's). Paths are relative to
//! `src/main/java/com/ldtteam/structurize/`.
//!
//! The format calls itself "Blueprint V1 Specification"
//! (`blueprints/v1/BlueprintUtil.java:38`). Version 1 is the only one that
//! exists: `readBlueprintFromNBT` returns `null` for anything else
//! (`:540-542`, `:613`), and this reader turns that into
//! [`Error::UnsupportedVersion`].
//!
//! ## Container: gzip around one NBT document
//!
//! A blueprint is one gzip member whose payload is a single NBT document whose
//! root is a compound with an empty name. Structurize writes it with
//! `NbtIo.writeCompressed` (`:626`) and reads it with `NbtIo.readCompressed`
//! (`storage/StructurePacks.java:428`). All 9 515 files of the pack begin
//! `1f 8b 08`; [`crate::blueprint::gzip`] is that layer.
//!
//! ## Root compound
//!
//! | Tag | Type | Meaning | Written | Read |
//! |---|---|---|---|---|
//! | `version` | byte | format version, always `1` | `:210` | `:540` |
//! | `size_x` | short | extent on X | `:212` | `:543` |
//! | `size_y` | short | extent on Y | `:213` | `:543` |
//! | `size_z` | short | extent on Z | `:214` | `:543` |
//! | `palette` | list\<compound\> | block states | `:217-223` | `:563-564` |
//! | `blocks` | int array | the cells, see below | `:226-228` | `:567` |
//! | `tile_entities` | list\<compound\> | block entities | `:230-237` | `:570` |
//! | `entities` | list\<compound\> | entities | `:240-243` | `:573` |
//! | `required_mods` | list\<string\> | mod ids the palette needs | `:246-253` | `:548` |
//! | `name` | string | pack-relative path | `:260` | `:587` |
//! | `architects` | list\<string\> | authors | `:264-269` | `:589-598` |
//! | `mcversion` | int | Minecraft **data version** | `:272` | `:560` |
//! | `optional_data` | compound | extension point | `:274-280` | `:600-609` |
//!
//! `mcversion` is a data version, not a version string: 3465 is 1.20.1
//! (`blueprints/v1/DataVersion.java:17`). Every shipped blueprint carries
//! exactly 3465 — the palette needs no data fixing, and this reader does none.
//! When the tag is absent the reader falls back to 1343 (1.12.2, the last
//! format before the 1.13 flattening) *and* Structurize then runs extra
//! cross-version fix-ups (`:43`, `:560`, `:575-578`); the data version is kept
//! here, the fix-ups are not written.
//!
//! ## Palette
//!
//! One compound per distinct block state, written by vanilla
//! `NbtUtils.writeBlockState` (`:221`) and read by `NbtUtils.readBlockState`
//! (`:306`):
//!
//! ```text
//! { Name: "minecraft:oak_stairs", Properties: { facing: "north", half: "bottom" } }
//! ```
//!
//! `Name` is always present; `Properties` only when the state has properties.
//! Index 0 is *not* guaranteed to be air — Structurize's own writer puts air
//! first only because the scanner starts from it (`:69-70`), and one shipped
//! file (`original/infrastructure/misc/fill.blueprint`) has a single-entry
//! palette of `structurize:blocksolidsubstitution` and no air at all. Nothing
//! here assumes index 0.
//!
//! ## Blocks: two 16-bit indices per int
//!
//! The cells are not positions: they are a dense array of
//! `size_x * size_y * size_z` palette indices, laid out **y outermost, then z,
//! then x** (`blueprints/v1/Blueprint.java:266` says so; writer `:649-658`,
//! reader `:711-720`), packed **two per int**, first cell in the high 16 bits
//! (`:664-669`). When the cell count is odd the last int carries one padding
//! zero in its low half (`:670-674`), which the reader produces and never
//! reads. Indices are unsigned: the writer stores them in a `short` and the
//! reader masks with `& 0xFFFF` (`Blueprint.java:550`).
//!
//! Checked over the whole pack: `blocks.len() == ceil(cells / 2)` in all 9 515
//! files and every index is inside the palette. Both are enforced here, as
//! [`Error::Format`] and [`Error::OutOfRange`].
//!
//! ## Tile entities (block entities)
//!
//! A flat list; each compound is the vanilla block entity's NBT plus its
//! structure-local position as `x`/`y`/`z`, written as **shorts**
//! (`:106-108`). Java reads them with `getShort` (`:562`), which accepts any
//! numeric tag, so this reader takes any width ([`crate::blueprint::nbt::as_i64`]).
//!
//! Every entry has an `id`. MineColonies' own block entities also carry a
//! `blueprintDataProvider` compound — the anchor payload: `corner1`, `corner2`,
//! `posTagMap` (`blockentities/interfaces/IBlueprintDataProviderBE.java:19-28`,
//! `:86-123`). The whole compound is kept verbatim in
//! [`BlockEntity::data`], so nothing is lost on the way to a structure.
//!
//! ## Entities
//!
//! Vanilla entity compounds plus three helpers (`:137-153`): `Pos`, three
//! doubles relative to the structure origin, and `TileX`/`TileY`/`TileZ`, ints
//! naming the cell the entity hangs in (`Blueprint.java:908-915`). Measured on
//! the pack: 7 023 entities, `Pos` always three doubles, mostly item frames.
//!
//! ## The anchor
//!
//! `optional_data.structurize.primary_offset` is `{x:int, y:int, z:int}`
//! (`:274-280` writing, `:600-609` reading through
//! `api/util/BlockPosUtil.java:42-49` and `:58-64`). It is the **cell** of the
//! anchor, so the component corners are stored as negative offsets from it
//! (`corner1: {x:-8, y:-7, z:-17}` in `fisherman2`). All 9 515 files have it.
//!
//! # What this module does not do
//!
//! * **No data fixers.** `mcversion` is read and exposed, never used to upgrade
//!   a palette (`:285-359`). Every shipped file is already 1.20.1.
//! * **No registry validation.** An unknown block id is kept, not rejected: a
//!   mod has no block registry at this layer, and `Host` would need a resolved
//!   `BlockKey` for it (`MANQUANT.md` §3).
//! * **No rotation, mirror or placement** (`Blueprint.java:628-770`). The
//!   converter emits the structure as scanned; turning it is the placer's job.
//! * **No property order from the block's declaration.** Properties are sorted
//!   by name ([`BlockState`]): vanilla orders them by each block's own
//!   declaration, which needs the registry.

use crate::blueprint::gzip::{self, GzipError};
use crate::blueprint::nbt::{self, as_f64, as_i64, as_text, type_name, NbtError};
use mod_api::{Compound, Tag};
use std::collections::BTreeMap;
use std::sync::Arc;

/// The largest structure this reader will build, in cells (8 MiB of palette
/// indices). A refusal is better than an allocation driven by a `short` three
/// times over: `65535³` cells would be 2.8·10¹⁴, and the `blocks` array check
/// below is what a hostile file cannot satisfy anyway.
pub const MAX_CELLS: u64 = 8_388_608;

/// A palette entry, shared by every cell that uses it.
pub type StateRef = Arc<BlockState>;

/// Structure dimensions, in blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Size {
    pub x: u16,
    pub y: u16,
    pub z: u16,
}

impl Size {
    /// Number of cells (`x*y*z`), in 64 bits: three `u16` do not fit a `u32`.
    pub fn cells(&self) -> u64 {
        u64::from(self.x) * u64::from(self.y) * u64::from(self.z)
    }
}

impl std::fmt::Display for Size {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}x{}", self.x, self.y, self.z)
    }
}

/// An integer position: structure-local for blocks and block entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Pos {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Pos { x, y, z }
    }
}

impl std::fmt::Display for Pos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{},{},{}", self.x, self.y, self.z)
    }
}

/// A block state: an id plus its properties.
///
/// Displays as the vanilla form, `minecraft:oak_stairs[facing=north,half=bottom]`,
/// with properties sorted by name. Vanilla orders them by the block's own
/// declaration instead; without a registry that order cannot be reproduced, so
/// sorting keeps the form deterministic instead of wrong in an unstable way.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockState {
    /// Namespaced block id, e.g. `minecraft:oak_stairs`.
    pub name: String,
    /// Properties, `facing` -> `north`. Empty for blocks without any.
    pub properties: BTreeMap<String, String>,
}

impl BlockState {
    /// Build from a palette entry — a blueprint's, or a structure template's.
    ///
    /// A blueprint writes `{Name, Properties}` (`BlueprintUtil.java:217-223`); a
    /// 26.3 structure template writes `{id, properties}`
    /// (`crates/horizon-world/src/worldgen/structure/template.rs:280-298`).
    /// Both are accepted, `Name` first, because no file carries both.
    pub fn from_palette(tag: &Compound) -> Result<BlockState, Error> {
        let name = nbt::tag(tag, "Name")
            .or_else(|| nbt::tag(tag, "id"))
            .and_then(as_text)
            .ok_or(Error::MissingField("palette[].Name"))?;

        let mut properties = BTreeMap::new();
        if let Some(entry) = nbt::tag(tag, "Properties").or_else(|| nbt::tag(tag, "properties")) {
            let Tag::Compound(values) = entry else {
                return Err(Error::WrongType {
                    field: "palette[].Properties".into(),
                    expected: "compound",
                    found: type_name(entry),
                });
            };
            for (key, value) in values {
                // Real files always store strings here; a numeric value is
                // tolerated so a hand-edited file does not sink the whole
                // blueprint, exactly as `getAsString` would flatten it.
                let text = match as_text(value) {
                    Some(text) => text,
                    None => match as_i64(value) {
                        Some(number) => number.to_string(),
                        None => {
                            return Err(Error::WrongType {
                                field: format!("palette[].Properties.{}", key.to_string_lossy()),
                                expected: "string",
                                found: type_name(value),
                            })
                        }
                    },
                };
                properties.insert(key.to_string_lossy(), text);
            }
        }
        Ok(BlockState { name, properties })
    }

    /// The block id without its namespace, or the whole id when it has none.
    pub fn path(&self) -> &str {
        self.name
            .split_once(':')
            .map(|(_, path)| path)
            .unwrap_or(&self.name)
    }

    /// The namespace, `minecraft` when the id has none.
    pub fn namespace(&self) -> &str {
        self.name
            .split_once(':')
            .map(|(namespace, _)| namespace)
            .unwrap_or("minecraft")
    }

    /// `true` for `minecraft:air`, the only id the pack uses for air.
    pub fn is_air(&self) -> bool {
        self.name == "minecraft:air"
    }

    /// A property by name.
    pub fn property(&self, name: &str) -> Option<&str> {
        self.properties.get(name).map(String::as_str)
    }
}

impl std::fmt::Display for BlockState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)?;
        if self.properties.is_empty() {
            return Ok(());
        }
        f.write_str("[")?;
        for (index, (key, value)) in self.properties.iter().enumerate() {
            if index > 0 {
                f.write_str(",")?;
            }
            write!(f, "{key}={value}")?;
        }
        f.write_str("]")
    }
}

/// One cell of the structure.
#[derive(Debug, Clone)]
pub struct Block {
    /// Structure-local position; `x` in `0..size.x`, and so on.
    pub pos: Pos,
    /// Index into [`Blueprint::palette`].
    pub palette_index: u16,
    /// The resolved state — the same [`Arc`] as the palette entry.
    pub state: StateRef,
}

impl Block {
    /// The vanilla string form of this cell's block state.
    pub fn state_string(&self) -> String {
        self.state.to_string()
    }
}

/// A block entity stored with the structure.
#[derive(Debug, Clone)]
pub struct BlockEntity {
    /// Structure-local position, from `x`/`y`/`z`.
    pub pos: Pos,
    /// The `id` tag, e.g. `minecolonies:colonybuilding`. Empty when absent.
    pub id: String,
    /// The whole original compound, kept verbatim: blueprint providers,
    /// inventories, `structurize:` substitutions, all of it.
    pub data: Compound,
}

/// A non-block entity stored with the structure (item frames, armour stands…).
#[derive(Debug, Clone)]
pub struct Entity {
    /// The `id` tag, e.g. `minecraft:item_frame`.
    pub id: String,
    /// `Pos`: three doubles, relative to the structure origin.
    pub pos: Option<[f64; 3]>,
    /// `TileX`/`TileY`/`TileZ`: the cell the entity hangs in.
    pub tile_pos: Option<Pos>,
    /// The whole original compound.
    pub data: Compound,
}

/// A parsed blueprint.
#[derive(Debug, Clone)]
pub struct Blueprint {
    /// `version`. Always 1 in practice; anything else is refused.
    pub format_version: i8,
    /// `mcversion`: the data version the palette was recorded with (3465 in
    /// every file of the pack).
    pub data_version: i32,
    /// `size_x`/`size_y`/`size_z`.
    pub size: Size,
    /// `name`: usually the pack-relative path, e.g.
    /// `darkoak/agriculture/husbandry/fisherman2.blueprint`.
    pub name: Option<String>,
    /// `architects`. Absent from every shipped MineColonies blueprint.
    pub architects: Vec<String>,
    /// `required_mods`: mod ids whose blocks appear in the palette.
    pub required_mods: Vec<String>,
    /// `optional_data.structurize.primary_offset`: the anchor **cell**,
    /// relative to the structure origin.
    pub primary_offset: Option<Pos>,
    /// The block states, in the order `blocks` indexes them.
    pub palette: Vec<StateRef>,
    /// One entry per cell, y-major then z then x.
    pub blocks: Vec<Block>,
    /// `tile_entities`.
    pub block_entities: Vec<BlockEntity>,
    /// `entities`.
    pub entities: Vec<Entity>,
}

impl Blueprint {
    /// Parse a blueprint file's bytes.
    ///
    /// Accepts the gzip form every real file is, and a bare NBT document
    /// (detected from the gzip magic), which is what a test or a tool that
    /// decompressed one hands over.
    pub fn parse(bytes: &[u8]) -> Result<Blueprint, Error> {
        if gzip::is_gzip(bytes) {
            Blueprint::from_nbt_bytes(&gzip::gunzip(bytes)?)
        } else {
            Blueprint::from_nbt_bytes(bytes)
        }
    }

    /// Parse an already-decompressed NBT document.
    pub fn from_nbt_bytes(bytes: &[u8]) -> Result<Blueprint, Error> {
        let (_name, root) = nbt::parse(bytes)?;
        Blueprint::from_compound(&root)
    }

    /// Build from an already-parsed NBT root.
    pub fn from_compound(root: &Compound) -> Result<Blueprint, Error> {
        let format_version = version_field(root)?;
        if format_version != 1 {
            return Err(Error::UnsupportedVersion(format_version));
        }

        let size = Size {
            x: dimension(root, "size_x")?,
            y: dimension(root, "size_y")?,
            z: dimension(root, "size_z")?,
        };

        let data_version = match nbt::tag(root, "mcversion") {
            Some(tag) => as_i64(tag)
                .map(|value| value as i32)
                .ok_or_else(|| Error::WrongType {
                    field: "mcversion".into(),
                    expected: "int",
                    found: type_name(tag),
                })?,
            // `BlueprintUtil.DEFAULT_FIXER_IF_NOT_FOUND` (`:43`, used at `:560`).
            None => 1343,
        };

        let palette = read_palette(root)?;
        let blocks = read_blocks(root, size, &palette)?;
        let block_entities = read_block_entities(root)?;
        let entities = read_entities(root)?;

        Ok(Blueprint {
            format_version,
            data_version,
            size,
            name: nbt::tag(root, "name").and_then(as_text),
            architects: string_list(root, "architects")?,
            required_mods: string_list(root, "required_mods")?,
            primary_offset: read_primary_offset(root)?,
            palette,
            blocks,
            block_entities,
            entities,
        })
    }

    /// How many cells the structure holds — `blocks.len()`, by construction.
    pub fn cells(&self) -> usize {
        self.blocks.len()
    }

    /// The cell at a structure-local position, if it exists.
    pub fn block_at(&self, x: u16, y: u16, z: u16) -> Option<&Block> {
        if x >= self.size.x || y >= self.size.y || z >= self.size.z {
            return None;
        }
        let index =
            (usize::from(y) * usize::from(self.size.z) + usize::from(z)) * usize::from(self.size.x)
                + usize::from(x);
        self.blocks.get(index)
    }

    /// The block state at a structure-local position.
    pub fn state_at(&self, x: u16, y: u16, z: u16) -> Option<&BlockState> {
        self.block_at(x, y, z).map(|block| block.state.as_ref())
    }

    /// Cells whose state is not `minecraft:air`.
    pub fn non_air_blocks(&self) -> impl Iterator<Item = &Block> {
        self.blocks.iter().filter(|block| !block.state.is_air())
    }

    /// How many cells hold `minecraft:air`.
    pub fn air_block_count(&self) -> usize {
        self.blocks.iter().filter(|block| block.state.is_air()).count()
    }

    /// The block entity sitting in a cell, if one does.
    pub fn block_entity_at(&self, x: u16, y: u16, z: u16) -> Option<&BlockEntity> {
        self.block_entities.iter().find(|entity| {
            entity.pos.x == i32::from(x)
                && entity.pos.y == i32::from(y)
                && entity.pos.z == i32::from(z)
        })
    }

    /// A deterministic, human-readable dump of everything this reader
    /// understood, for pinning a file in a test.
    ///
    /// ```text
    /// size={x},{y},{z}
    /// palette={n}
    /// {x},{y},{z}={state}      (y-major)
    /// te={id}@{x},{y},{z}
    /// entity={id}
    /// ```
    pub fn canonical_dump(&self) -> String {
        let mut out = String::with_capacity(self.blocks.len() * 40);
        out.push_str(&format!(
            "size={},{},{}\n",
            self.size.x, self.size.y, self.size.z
        ));
        out.push_str(&format!("palette={}\n", self.palette.len()));
        for block in &self.blocks {
            out.push_str(&format!(
                "{},{},{}={}\n",
                block.pos.x, block.pos.y, block.pos.z, block.state
            ));
        }
        for entity in &self.block_entities {
            out.push_str(&format!("te={}@{}\n", entity.id, entity.pos));
        }
        for entity in &self.entities {
            out.push_str(&format!("entity={}\n", entity.id));
        }
        out
    }

    /// [`Blueprint::canonical_dump`] hashed, so one `u64` pins a whole file.
    pub fn fingerprint(&self) -> u64 {
        fnv1a64(self.canonical_dump().as_bytes())
    }
}

/// FNV-1a, 64 bit: the hash [`Blueprint::fingerprint`] uses.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

// ---------------------------------------------------------------------------
// Field readers
// ---------------------------------------------------------------------------

/// `version`, as the byte Structurize writes (`BlueprintUtil.java:210`) but
/// read at any width.
fn version_field(root: &Compound) -> Result<i8, Error> {
    let tag = nbt::tag(root, "version").ok_or(Error::MissingField("version"))?;
    let value = as_i64(tag).ok_or_else(|| Error::WrongType {
        field: "version".into(),
        expected: "a numeric tag",
        found: type_name(tag),
    })?;
    Ok(value as i8)
}

/// A `u16` dimension. Zero is not a structure, and a value above `u16::MAX`
/// cannot have been written by `putShort`.
fn dimension(root: &Compound, name: &'static str) -> Result<u16, Error> {
    let tag = nbt::tag(root, name).ok_or(Error::MissingField(name))?;
    let value = as_i64(tag).ok_or_else(|| Error::WrongType {
        field: name.to_string(),
        expected: "a numeric tag",
        found: type_name(tag),
    })?;
    if !(1..=i64::from(u16::MAX)).contains(&value) {
        return Err(Error::OutOfRange(format!("{name} = {value}")));
    }
    Ok(value as u16)
}

fn string_list(root: &Compound, name: &str) -> Result<Vec<String>, Error> {
    let Some(tag) = nbt::tag(root, name) else {
        return Ok(Vec::new());
    };
    let Tag::List(items) = tag else {
        return Err(Error::WrongType {
            field: name.to_string(),
            expected: "list",
            found: type_name(tag),
        });
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        match as_text(item) {
            Some(text) => out.push(text),
            None => {
                return Err(Error::WrongType {
                    field: format!("{name}[]"),
                    expected: "string",
                    found: type_name(item),
                })
            }
        }
    }
    Ok(out)
}

fn read_palette(root: &Compound) -> Result<Vec<StateRef>, Error> {
    let tag = nbt::tag(root, "palette").ok_or(Error::MissingField("palette"))?;
    let Tag::List(items) = tag else {
        return Err(Error::WrongType {
            field: "palette".into(),
            expected: "list",
            found: type_name(tag),
        });
    };
    if items.is_empty() {
        return Err(Error::Format("palette is empty".into()));
    }
    let mut palette = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let Tag::Compound(entry) = item else {
            return Err(Error::WrongType {
                field: format!("palette[{index}]"),
                expected: "compound",
                found: type_name(item),
            });
        };
        palette.push(Arc::new(BlockState::from_palette(entry)?));
    }
    Ok(palette)
}

/// Decode the flat `blocks` int array back into cells.
fn read_blocks(root: &Compound, size: Size, palette: &[StateRef]) -> Result<Vec<Block>, Error> {
    let tag = nbt::tag(root, "blocks").ok_or(Error::MissingField("blocks"))?;
    let Tag::IntArray(words) = tag else {
        return Err(Error::WrongType {
            field: "blocks".into(),
            expected: "int array",
            found: type_name(tag),
        });
    };

    let cells = size.cells();
    if cells > MAX_CELLS {
        return Err(Error::OutOfRange(format!(
            "{} cells ({size}) is above the {MAX_CELLS}-cell ceiling",
            cells
        )));
    }
    let expected = (cells + 1) / 2;
    if words.len() as u64 != expected {
        return Err(Error::Format(format!(
            "blocks holds {} ints, expected {expected} for {cells} cells ({size})",
            words.len()
        )));
    }

    let mut blocks = Vec::with_capacity(cells as usize);
    let mut cursor = 0usize;
    for y in 0..size.y {
        for z in 0..size.z {
            for x in 0..size.x {
                let word = words[cursor / 2];
                let index = if cursor % 2 == 0 {
                    ((word as u32) >> 16) as u16
                } else {
                    (word as u32 & 0xffff) as u16
                };
                cursor += 1;

                let state = palette.get(usize::from(index)).ok_or_else(|| {
                    Error::OutOfRange(format!(
                        "palette index {index} at x={x} y={y} z={z}, the palette holds {} entries",
                        palette.len()
                    ))
                })?;
                blocks.push(Block {
                    pos: Pos::new(i32::from(x), i32::from(y), i32::from(z)),
                    palette_index: index,
                    state: Arc::clone(state),
                });
            }
        }
    }
    Ok(blocks)
}

fn read_block_entities(root: &Compound) -> Result<Vec<BlockEntity>, Error> {
    let Some(tag) = nbt::tag(root, "tile_entities") else {
        return Ok(Vec::new());
    };
    let Tag::List(items) = tag else {
        return Err(Error::WrongType {
            field: "tile_entities".into(),
            expected: "list",
            found: type_name(tag),
        });
    };

    let mut out = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let Tag::Compound(entry) = item else {
            return Err(Error::WrongType {
                field: format!("tile_entities[{index}]"),
                expected: "compound",
                found: type_name(item),
            });
        };
        let pos = Pos::new(
            number_or_zero(entry, "x"),
            number_or_zero(entry, "y"),
            number_or_zero(entry, "z"),
        );
        let id = nbt::tag(entry, "id")
            .and_then(as_text)
            .unwrap_or_default();
        out.push(BlockEntity {
            pos,
            id,
            data: entry.clone(),
        });
    }
    Ok(out)
}

fn read_entities(root: &Compound) -> Result<Vec<Entity>, Error> {
    let Some(tag) = nbt::tag(root, "entities") else {
        return Ok(Vec::new());
    };
    let Tag::List(items) = tag else {
        return Err(Error::WrongType {
            field: "entities".into(),
            expected: "list",
            found: type_name(tag),
        });
    };

    let mut out = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let Tag::Compound(entry) = item else {
            return Err(Error::WrongType {
                field: format!("entities[{index}]"),
                expected: "compound",
                found: type_name(item),
            });
        };

        let pos = nbt::tag(entry, "Pos").and_then(|value| {
            let Tag::List(coordinates) = value else {
                return None;
            };
            if coordinates.len() < 3 {
                return None;
            }
            let mut triple = [0f64; 3];
            for (slot, coordinate) in triple.iter_mut().zip(coordinates.iter()) {
                *slot = as_f64(coordinate)?;
            }
            Some(triple)
        });

        let tile_pos = match (
            nbt::tag(entry, "TileX"),
            nbt::tag(entry, "TileY"),
            nbt::tag(entry, "TileZ"),
        ) {
            (Some(x), Some(y), Some(z)) => Some(Pos::new(
                as_i64(x).unwrap_or(0) as i32,
                as_i64(y).unwrap_or(0) as i32,
                as_i64(z).unwrap_or(0) as i32,
            )),
            _ => None,
        };

        let id = nbt::tag(entry, "id")
            .and_then(as_text)
            .unwrap_or_default();
        out.push(Entity {
            id,
            pos,
            tile_pos,
            data: entry.clone(),
        });
    }
    Ok(out)
}

fn read_primary_offset(root: &Compound) -> Result<Option<Pos>, Error> {
    let Some(optional) = nbt::tag(root, "optional_data").and_then(as_compound) else {
        return Ok(None);
    };
    let Some(structurize) = nbt::tag(optional, "structurize").and_then(as_compound) else {
        return Ok(None);
    };
    let Some(offset) = nbt::tag(structurize, "primary_offset").and_then(as_compound) else {
        return Ok(None);
    };
    Ok(Some(Pos::new(
        number_or_zero(offset, "x"),
        number_or_zero(offset, "y"),
        number_or_zero(offset, "z"),
    )))
}

fn as_compound(tag: &Tag) -> Option<&Compound> {
    match tag {
        Tag::Compound(compound) => Some(compound),
        _ => None,
    }
}

/// A number under `key`, `0` when it is absent — which is what Java's
/// `getShort`/`getInt` answer (they never fail, they return zero).
fn number_or_zero(compound: &Compound, name: &str) -> i32 {
    nbt::tag(compound, name)
        .and_then(as_i64)
        .unwrap_or(0)
        .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Everything that can go wrong while reading a blueprint.
///
/// One refusal per reason, named: a blueprint that will not load says *why* in
/// a log or a test, which is what `readBlueprintFromNBT` returning `null`
/// (`BlueprintUtil.java:613`) cannot.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// The gzip container is broken.
    Gzip(GzipError),
    /// The NBT document is malformed.
    Nbt(NbtError),
    /// A tag the format requires is absent.
    MissingField(&'static str),
    /// A tag is present with an unexpected type.
    WrongType {
        field: String,
        expected: &'static str,
        found: &'static str,
    },
    /// A value is well formed but cannot be part of a blueprint.
    OutOfRange(String),
    /// Structural inconsistency (size against the blocks array, empty palette…).
    Format(String),
    /// `version` is not 1, which Structurize itself answers with `null`.
    UnsupportedVersion(i8),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Gzip(error) => write!(f, "gzip error: {error}"),
            Error::Nbt(error) => write!(f, "{error}"),
            Error::MissingField(name) => write!(f, "missing required tag `{name}`"),
            Error::WrongType {
                field,
                expected,
                found,
            } => write!(f, "tag `{field}` is a {found}, expected {expected}"),
            Error::OutOfRange(detail) => write!(f, "value out of range: {detail}"),
            Error::Format(detail) => write!(f, "malformed blueprint: {detail}"),
            Error::UnsupportedVersion(version) => write!(
                f,
                "blueprint format version {version}, only version 1 exists"
            ),
        }
    }
}

impl std::error::Error for Error {}

impl From<GzipError> for Error {
    fn from(error: GzipError) -> Self {
        Error::Gzip(error)
    }
}

impl From<NbtError> for Error {
    fn from(error: NbtError) -> Self {
        Error::Nbt(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blueprint::corpus;

    /// A hand-built document, for the refusals the pack cannot show.
    fn synthetic(version: i8, size: [i16; 3], palette: &[&str], words: &[i32]) -> Compound {
        let mut root = Compound::new();
        root.insert("version".into(), Tag::Byte(version));
        root.insert("size_x".into(), Tag::Short(size[0]));
        root.insert("size_y".into(), Tag::Short(size[1]));
        root.insert("size_z".into(), Tag::Short(size[2]));
        root.insert(
            "palette".into(),
            Tag::List(
                palette
                    .iter()
                    .map(|name| {
                        Tag::Compound(Compound::from([(
                            "Name".into(),
                            Tag::String((*name).into()),
                        )]))
                    })
                    .collect(),
            ),
        );
        root.insert("blocks".into(), Tag::IntArray(words.to_vec()));
        root
    }

    #[test]
    fn a_real_blueprint_reads_with_every_field_measured() {
        // Measured on the file itself, then asserted: 12x15x21 = 3 780 cells,
        // 40 palette entries, 1 543 non-air cells, one block entity, one entity.
        let bytes = corpus::read("darkoak/agriculture/husbandry/fisherman2.blueprint");
        let blueprint = Blueprint::parse(&bytes).expect("a shipped blueprint parses");

        assert_eq!(blueprint.format_version, 1);
        assert_eq!(blueprint.data_version, 3465, "1.20.1, as every file says");
        assert_eq!(blueprint.size, Size { x: 12, y: 15, z: 21 });
        assert_eq!(blueprint.size.cells(), 3_780);
        assert_eq!(blueprint.cells(), 3_780);
        assert_eq!(blueprint.palette.len(), 40);
        assert_eq!(blueprint.blocks.len(), 3_780);
        assert_eq!(blueprint.air_block_count(), 3_780 - 1_543);
        assert_eq!(blueprint.non_air_blocks().count(), 1_543);
        assert_eq!(
            blueprint.name.as_deref(),
            Some("darkoak/agriculture/husbandry/fisherman2.blueprint"),
            "the name tag is the pack-relative path"
        );
        assert_eq!(blueprint.required_mods, vec!["minecolonies".to_string()]);
        assert!(blueprint.architects.is_empty(), "no shipped file has any");
        assert_eq!(blueprint.primary_offset, Some(Pos::new(8, 7, 17)));
        assert_eq!(blueprint.block_entities.len(), 1);
        assert_eq!(blueprint.entities.len(), 1);
    }

    #[test]
    fn the_cells_come_back_in_y_major_z_then_x_order() {
        let bytes = corpus::read("darkoak/agriculture/husbandry/fisherman2.blueprint");
        let blueprint = Blueprint::parse(&bytes).expect("parses");
        // Within one line x advances fastest, then z, then y is slowest
        // (`Blueprint.java:266`; the reader nests the same way,
        // `port-blueprint/src/blueprint.rs:487-489`, and so does the writer,
        // `BlueprintUtil.java:649-658`).
        for (index, cell) in blueprint.blocks.iter().take(4).enumerate() {
            assert_eq!(cell.pos, Pos::new(index as i32, 0, 0));
        }
        // z advances only once x has wrapped: 12 cells of x, then (0,0,1).
        assert_eq!(blueprint.blocks[12].pos, Pos::new(0, 0, 1));
        // Reading by position must give the same objects the array holds.
        assert_eq!(
            blueprint.block_at(0, 0, 1).map(|block| block.pos),
            Some(Pos::new(0, 0, 1))
        );
    }

    #[test]
    fn a_known_cell_holds_the_hut_block_and_the_anchor_cell_agrees() {
        let bytes = corpus::read("darkoak/agriculture/husbandry/fisherman2.blueprint");
        let blueprint = Blueprint::parse(&bytes).expect("parses");
        // Measured: palette index 36 at (8,7,17), the declared anchor cell.
        assert_eq!(
            blueprint.state_at(8, 7, 17).map(ToString::to_string),
            Some("minecolonies:blockhutfisherman[facing=south]".to_string())
        );
        assert_eq!(
            blueprint.block_at(8, 7, 17).map(|block| block.palette_index),
            Some(36)
        );
        // The bottom corners are the substitution marker, not air
        // (Structurize fills what it cannot know at scan time).
        assert_eq!(
            blueprint.state_at(0, 0, 0).map(|state| state.name.as_str()),
            Some("structurize:blocksolidsubstitution")
        );
        // Out of the grid is `None`, never a panic and never a wrong cell.
        assert!(blueprint.block_at(12, 0, 0).is_none(), "x = size_x is outside");
        assert!(blueprint.block_at(0, 15, 0).is_none());
        assert!(blueprint.block_at(0, 0, 21).is_none());
    }

    #[test]
    fn the_block_entity_keeps_its_whole_payload() {
        let bytes = corpus::read("darkoak/agriculture/husbandry/fisherman2.blueprint");
        let blueprint = Blueprint::parse(&bytes).expect("parses");
        let entity = blueprint
            .block_entity_at(8, 7, 17)
            .expect("the colony building sits on the anchor cell");
        assert_eq!(entity.id, "minecolonies:colonybuilding");
        assert_eq!(entity.pos, Pos::new(8, 7, 17));
        // The provider payload is what a hut reads its colony from; it must
        // survive the parse untouched (`IBlueprintDataProviderBE.java:19-28`).
        assert!(
            nbt::tag(&entity.data, "blueprintDataProvider").is_some(),
            "the anchor payload is kept verbatim"
        );
        assert!(nbt::tag(&entity.data, "colony").is_some());
    }

    #[test]
    fn the_entity_keeps_its_position_and_its_cell() {
        let bytes = corpus::read("darkoak/agriculture/husbandry/fisherman2.blueprint");
        let blueprint = Blueprint::parse(&bytes).expect("parses");
        let entity = &blueprint.entities[0];
        assert_eq!(entity.id, "minecraft:item_frame");
        assert_eq!(entity.pos, Some([4.5, 9.5, 17.96875]));
        assert_eq!(entity.tile_pos, Some(Pos::new(4, 9, 17)));
    }

    #[test]
    fn a_file_with_no_air_at_all_reads_as_itself() {
        // The one shipped file whose palette has a single entry and no air
        // (`original/infrastructure/misc/fill.blueprint`): 8x8x8 of the same
        // substitution block, so no index may be assumed to mean air.
        let bytes = corpus::read("original/infrastructure/misc/fill.blueprint");
        let blueprint = Blueprint::parse(&bytes).expect("parses");
        assert_eq!(blueprint.size, Size { x: 8, y: 8, z: 8 });
        assert_eq!(blueprint.palette.len(), 1);
        assert_eq!(blueprint.palette[0].name, "structurize:blocksolidsubstitution");
        assert_eq!(blueprint.cells(), 512);
        assert_eq!(blueprint.air_block_count(), 0);
        assert_eq!(blueprint.non_air_blocks().count(), 512);
        assert_eq!(blueprint.name.as_deref(), Some("dig"));
        assert_eq!(blueprint.primary_offset, Some(Pos::new(4, 0, 4)));
    }

    #[test]
    fn the_five_levels_of_one_building_all_read() {
        // One building, levels 1 to 5: same footprint, and each level adds
        // states and block entities. This is the level coverage the converter
        // is measured on, on a real building rather than a synthetic grid.
        let expected = [
            ("darkoak/fundamentals/builder1.blueprint", 44usize, 2usize, 809usize),
            ("darkoak/fundamentals/builder2.blueprint", 51, 2, 817),
            ("darkoak/fundamentals/builder3.blueprint", 79, 4, 961),
            ("darkoak/fundamentals/builder4.blueprint", 106, 7, 1004),
            ("darkoak/fundamentals/builder5.blueprint", 143, 14, 1075),
        ];
        let mut previous = 0usize;
        for (level, (file, palette, block_entities, non_air)) in expected.iter().enumerate() {
            let bytes = corpus::read(file);
            let blueprint = Blueprint::parse(&bytes)
                .unwrap_or_else(|error| panic!("level {} did not parse: {error}", level + 1));
            assert_eq!(blueprint.size, Size { x: 14, y: 14, z: 15 }, "{file}");
            assert_eq!(blueprint.cells(), 2_940, "{file}");
            assert_eq!(blueprint.palette.len(), *palette, "{file}");
            assert_eq!(blueprint.block_entities.len(), *block_entities, "{file}");
            assert_eq!(blueprint.non_air_blocks().count(), *non_air, "{file}");
            assert_eq!(blueprint.primary_offset, Some(Pos::new(7, 5, 3)), "{file}");
            assert!(
                *palette >= previous,
                "a level never shrinks the palette ({file})"
            );
            previous = *palette;
        }
    }

    #[test]
    fn the_five_levels_of_a_husbandry_hut_all_read() {
        // The same check on a second building, whose levels keep the same
        // footprint and add one rack each from level 3 (measured: 1, 1, 2, 3, 4
        // block entities — level 1 and 2 hold only the colony building).
        let block_entities = [1usize, 1, 2, 3, 4];
        for level in 1usize..=5 {
            let file = format!("darkoak/agriculture/husbandry/fisherman{level}.blueprint");
            let bytes = corpus::read(&file);
            let blueprint = Blueprint::parse(&bytes).expect("a level parses");
            assert_eq!(blueprint.size, Size { x: 12, y: 15, z: 21 }, "{file}");
            assert_eq!(blueprint.cells(), 3_780, "{file}");
            assert_eq!(
                blueprint.block_entities.len(),
                block_entities[level - 1],
                "{file}"
            );
            assert_eq!(blueprint.primary_offset, Some(Pos::new(8, 7, 17)), "{file}");
        }
    }

    #[test]
    fn a_bare_nbt_document_parses_the_same_as_the_gzip_file() {
        let compressed = corpus::read("darkoak/agriculture/husbandry/fisherman2.blueprint");
        let raw = gzip::gunzip(&compressed).expect("the file is gzip");
        let from_file = Blueprint::parse(&compressed).expect("gzip is detected");
        let from_raw = Blueprint::parse(&raw).expect("a bare document is accepted");
        assert_eq!(from_file.fingerprint(), from_raw.fingerprint());
        assert_eq!(from_file.canonical_dump(), from_raw.canonical_dump());
    }

    #[test]
    fn the_dump_is_stable_and_pins_the_shape_of_a_real_file() {
        let bytes = corpus::read("darkoak/agriculture/husbandry/fisherman2.blueprint");
        let first = Blueprint::parse(&bytes).expect("parses");
        let second = Blueprint::parse(&bytes).expect("parses again");
        assert_eq!(first.fingerprint(), second.fingerprint());
        let dump = first.canonical_dump();
        let lines: Vec<&str> = dump.lines().collect();
        // size + palette + one line per cell + one block entity + one entity.
        assert_eq!(lines.len(), 2 + 3_780 + 1 + 1);
        assert_eq!(lines[0], "size=12,15,21");
        assert_eq!(lines[1], "palette=40");
        assert_eq!(lines[2], "0,0,0=structurize:blocksolidsubstitution");
        assert_eq!(lines[3_782], "te=minecolonies:colonybuilding@8,7,17");
        assert_eq!(lines[3_783], "entity=minecraft:item_frame");
    }

    #[test]
    fn a_version_other_than_one_is_refused_by_name() {
        let document = synthetic(2, [1, 1, 1], &["minecraft:air"], &[0]);
        assert_eq!(
            Blueprint::from_compound(&document).expect_err("only version 1 exists"),
            Error::UnsupportedVersion(2)
        );
        // A missing version is its own refusal, not a silent 1.
        let mut none = synthetic(1, [1, 1, 1], &["minecraft:air"], &[0]);
        none.remove(&"version".into());
        assert_eq!(
            Blueprint::from_compound(&none).expect_err("no version tag"),
            Error::MissingField("version")
        );
    }

    #[test]
    fn a_blocks_array_that_does_not_match_the_size_is_refused() {
        // 2x2x2 needs ceil(8/2) = 4 ints.
        let document = synthetic(1, [2, 2, 2], &["minecraft:air"], &[0, 0, 0]);
        let error = Blueprint::from_compound(&document).expect_err("3 ints cannot hold 8 cells");
        assert!(
            matches!(&error, Error::Format(detail) if detail.contains("expected 4 for 8 cells")),
            "got {error}"
        );
        assert!(error.to_string().contains("malformed blueprint"));
    }

    #[test]
    fn a_palette_index_outside_the_palette_names_the_cell() {
        // One palette entry, one cell of index 3 in the high half of the int.
        let document = synthetic(1, [2, 1, 1], &["minecraft:air"], &[3 << 16]);
        let error = Blueprint::from_compound(&document).expect_err("index 3 does not exist");
        assert!(
            matches!(&error, Error::OutOfRange(detail) if detail.contains("palette index 3")),
            "got {error}"
        );
    }

    #[test]
    fn an_empty_palette_and_a_zero_dimension_are_refused() {
        let document = synthetic(1, [1, 1, 1], &[], &[0]);
        assert_eq!(
            Blueprint::from_compound(&document).expect_err("no palette"),
            Error::Format("palette is empty".into())
        );
        let document = synthetic(1, [0, 1, 1], &["minecraft:air"], &[]);
        let error = Blueprint::from_compound(&document).expect_err("a zero extent");
        assert!(matches!(&error, Error::OutOfRange(_)), "got {error}");
    }

    #[test]
    fn a_missing_mcversion_falls_back_to_the_pre_flattening_data_version() {
        // `DEFAULT_FIXER_IF_NOT_FOUND` = 1343, 1.12.2 (`BlueprintUtil.java:43`).
        let document = synthetic(1, [1, 1, 1], &["minecraft:air"], &[0]);
        let blueprint = Blueprint::from_compound(&document).expect("parses");
        assert_eq!(blueprint.data_version, 1343);
        assert_eq!(blueprint.cells(), 1);
        assert!(blueprint.blocks[0].state.is_air());
    }

    #[test]
    fn a_wrong_type_says_what_it_found() {
        let mut document = synthetic(1, [1, 1, 1], &["minecraft:air"], &[0]);
        document.insert("blocks".into(), Tag::List(Vec::new()));
        let error = Blueprint::from_compound(&document).expect_err("a list is not an int array");
        assert_eq!(
            error,
            Error::WrongType {
                field: "blocks".into(),
                expected: "int array",
                found: "list",
            }
        );
        assert_eq!(error.to_string(), "tag `blocks` is a list, expected int array");
    }

    #[test]
    fn block_entity_coordinates_are_read_at_any_width() {
        // Structurize writes shorts (`BlueprintUtil.java:106-108`) and Java's
        // `getShort` reads ints too, so both must land on the same cell.
        for coordinate in [Tag::Short(3), Tag::Int(3)] {
            let mut document = synthetic(1, [1, 1, 1], &["minecraft:air"], &[0]);
            let mut entry = Compound::new();
            entry.insert("id".into(), Tag::String("minecraft:chest".into()));
            entry.insert("x".into(), coordinate.clone());
            entry.insert("y".into(), Tag::Short(0));
            entry.insert("z".into(), Tag::Int(0));
            document.insert("tile_entities".into(), Tag::List(vec![Tag::Compound(entry)]));
            let blueprint = Blueprint::from_compound(&document).expect("parses");
            assert_eq!(blueprint.block_entities[0].pos, Pos::new(3, 0, 0));
            assert_eq!(blueprint.block_entity_at(3, 0, 0).map(|e| e.id.as_str()), Some("minecraft:chest"));
        }
    }

    #[test]
    fn an_entity_without_tile_tags_keeps_no_cell_and_its_position_survives() {
        let mut document = synthetic(1, [1, 1, 1], &["minecraft:air"], &[0]);
        let entry = Compound::from([
            ("id".into(), Tag::String("minecraft:armor_stand".into())),
            (
                "Pos".into(),
                Tag::List(vec![
                    Tag::Double(0.5f64.to_bits()),
                    Tag::Double(1.0f64.to_bits()),
                    Tag::Double((-0.5f64).to_bits()),
                ]),
            ),
        ]);
        document.insert("entities".into(), Tag::List(vec![Tag::Compound(entry)]));
        let blueprint = Blueprint::from_compound(&document).expect("parses");
        assert_eq!(blueprint.entities[0].pos, Some([0.5, 1.0, -0.5]));
        assert_eq!(blueprint.entities[0].tile_pos, None);
    }

    #[test]
    fn an_odd_cell_count_leaves_a_padding_zero_unread() {
        // 3x1x1 = 3 cells, so 2 ints and a padding zero in the last low half.
        // The third cell must read the high half of the second int.
        let document = synthetic(
            1,
            [3, 1, 1],
            &["minecraft:air", "minecraft:stone"],
            &[1 << 16, 1 << 16],
        );
        let blueprint = Blueprint::from_compound(&document).expect("parses");
        assert_eq!(blueprint.cells(), 3);
        assert_eq!(blueprint.blocks[0].state.name, "minecraft:stone");
        assert_eq!(blueprint.blocks[1].state.name, "minecraft:air");
        assert_eq!(
            blueprint.blocks[2].state.name, "minecraft:stone",
            "the third cell is the high half of the second int, not the padding"
        );
    }

    #[test]
    fn a_palette_entry_reads_both_spellings_and_sorts_its_properties() {
        let mut properties = Compound::new();
        properties.insert("waterlogged".into(), Tag::String("false".into()));
        properties.insert("facing".into(), Tag::String("north".into()));
        let entry = Compound::from([
            ("Name".into(), Tag::String("minecraft:oak_stairs".into())),
            ("Properties".into(), Tag::Compound(properties)),
        ]);
        let state = BlockState::from_palette(&entry).expect("a blueprint's spelling");
        assert_eq!(
            state.to_string(),
            "minecraft:oak_stairs[facing=north,waterlogged=false]"
        );
        assert_eq!(state.namespace(), "minecraft");
        assert_eq!(state.path(), "oak_stairs");
        assert_eq!(state.property("facing"), Some("north"));
        assert_eq!(state.property("half"), None);

        // A 26.3 template's spelling, and a numeric property value.
        let mut properties = Compound::new();
        properties.insert("level".into(), Tag::Int(3));
        let entry = Compound::from([
            ("id".into(), Tag::String("minecraft:composter".into())),
            ("properties".into(), Tag::Compound(properties)),
        ]);
        let state = BlockState::from_palette(&entry).expect("a template's spelling");
        assert_eq!(state.to_string(), "minecraft:composter[level=3]");
        assert!(!state.is_air());

        let nameless = Compound::from([("Properties".into(), Tag::Compound(Compound::new()))]);
        assert_eq!(
            BlockState::from_palette(&nameless).expect_err("no id at all"),
            Error::MissingField("palette[].Name")
        );
    }

    #[test]
    fn fnv_matches_its_known_vectors() {
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
