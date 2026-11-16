//! Conversion from Structurize's blueprint to **vanilla's structure-template
//! NBT**, the format `/place template` and a data pack's `structure/` folder
//! use — and the only structure format the engine knows how to place.
//!
//! [`convert`] is pure: a parsed [`Blueprint`] in, a [`Conversion`] out.
//! [`Conversion::to_compound`] renders it as the engine's own tree,
//! [`Conversion::to_nbt`] as the bytes of a document, and
//! [`Conversion::to_gzipped_nbt`] as a real `.nbt` file's member. [`Structure::parse`]
//! reads such a document back, which is what lets the tests prove a conversion
//! is faithful instead of asserting it.
//!
//! # Why convert at all, and why in memory
//!
//! The pack holds **9 515 blueprints** (24 MiB compressed, 338 MiB and 47.6
//! million cells decompressed). Converting them ahead of time yields about
//! **2 GB** on disk, for a mod that ships as one `.hmod`: so the mod converts
//! one building **when it needs it** and keeps nothing. A blueprint is a few
//! kilobytes to a few hundred, a converted structure is a `Vec<StructureBlock>`
//! that dies with the placement, and no byte ever reaches a path: the document
//! is returned, the caller places it.
//!
//! # The output, field by field
//!
//! The engine's writer is the specification this follows:
//! `crates/horizon-world/src/worldgen/structure/template.rs` (the tree
//! `StructureTemplate::save` builds), and the API-side shape a caller can hold
//! is `crates/horizon-mod-vocab/src/world/structure_template.rs:36-56`.
//!
//! | Tag | Type | Meaning | Blueprint source |
//! |---|---|---|---|
//! | `DataVersion` | int | data version of the palette | `mcversion` (`BlueprintUtil.java:272`), or [`CURRENT_DATA_VERSION`] |
//! | `size` | list\<int\> (3) | `[x, y, z]` | `size_x`/`size_y`/`size_z` (`:212-214`) |
//! | `palette` | list\<compound\> | one block state per entry | `palette` (`:217-223`) |
//! | `blocks` | list\<compound\> | `{pos: [3 int], state: int, nbt?: compound}` | the cell grid (`:226-228`) |
//! | `entities` | list\<compound\> | `{pos: [3 double], blockPos: [3 int], nbt: compound}` | `entities` (`:240-243`) |
//!
//! Written in that order by the engine (`template.rs:276-322`); a `Compound`
//! here is a `BTreeMap`, so the keys of each entry come out sorted instead.
//! Nothing reads them positionally.
//!
//! **Cross-checked against the port's own generated tree**, not only against the
//! engine's writer: `data/minecolonies/structure/minecolonies/**.nbt` (9 515
//! files, 1.8 GB, git-ignored) holds the documents this port produced for the
//! engine's `/place template` path, and they are the shape below — root keys
//! `DataVersion`/`blocks`/`entities`/`palette`/`size`, `DataVersion` 5023,
//! `id`/`properties` palettes, one `blocks` entry per cell air included, and a
//! block-entity payload with `id` and `blueprintDataProvider` but no `x`/`y`/`z`.
//! Measured on `structure/minecolonies/acacia/agriculture/fields/bamboo.nbt`:
//! 770 entries for a 10×7×11 blueprint, 18 palette entries.
//!
//! `state` is the **index into `palette`** of that cell, and a block entity is
//! not a sibling list as in a blueprint: it is the `nbt` compound of the block
//! entry at its position, with the position itself only in `pos`
//! (`template.rs:276-284`).
//!
//! ## Two spellings for one palette entry
//!
//! A palette entry is `{Name, Properties}` up to 1.20.1 and `{id, properties}`
//! from 26.3 on — not a reading error: the engine writes `id`/`properties`
//! (`template.rs:287-303`) and reads `Name` as a fallback (`:196-198`), so both
//! load. [`PaletteDialect`] picks which one [`Structure::to_compound`] emits.
//!
//! ## Air is kept
//!
//! Vanilla's own writer drops air cells when "show air" is off, so a real
//! template may hold fewer entries than `size` implies. This converter keeps
//! **one entry per cell**, air included, exactly like the blueprint: the output
//! is then a cell-for-cell image of the input, and a round trip can compare
//! every cell. Both forms are valid to the engine.
//!
//! ## What is deliberately not carried over
//!
//! * `version`, `name`, `architects`, `required_mods`, `optional_data` — no
//!   vanilla equivalent, and the engine's writer emits only the five root tags
//!   above (`template.rs:318-322`).
//! * the `x`/`y`/`z` of a blueprint block entity: they hold the structure-local
//!   cell (`BlueprintUtil.java:106-108`) and vanilla carries that position in
//!   `pos` instead. Everything else in the compound — `id`, MineColonies'
//!   `blueprintDataProvider`, inventories, `structurize:` substitution payloads
//!   — is kept verbatim.
//! * the `TileX`/`TileY`/`TileZ` of an entity, which becomes `blockPos`
//!   (`BlueprintUtil.java:150-152`).
//!
//! # The anchor, and why it cannot be written
//!
//! A blueprint names its anchor **cell** in
//! `optional_data.structurize.primary_offset` (`BlueprintUtil.java:274-280`,
//! read back at `:600-609`). Vanilla's structure format has no anchor field —
//! the five root tags above are exhaustive — and vanilla's origin is always the
//! `(0, 0, 0)` corner, so nothing is translated to make the anchor the origin
//! and no tag is invented: the anchor comes back as data ([`Anchor`]) for the
//! placer to do `world_origin = target - anchor`, which is what Structurize
//! itself does (`BlueprintUtil.java:161` records `anchorPos - scanOrigin`).
//! When a file has no `primary_offset` the anchor is recomputed the way
//! Structurize does (`Blueprint.java:594-604`): the single cell whose block
//! entity carries `blueprintDataProvider`, else `(size_x / 2, 0, size_z / 2)`.
//! Structurize's rule also accepts an `IAnchorBlock` cell, which cannot be seen
//! without a block registry, so that half is not reproduced — and it never
//! matters on the pack: all 9 515 files declare the anchor.

use crate::blueprint::format::{BlockState, Blueprint, Pos};
use crate::blueprint::gzip;
use crate::blueprint::nbt::{self, as_f64, as_i64, as_text, type_name, without, NbtError};
use mod_api::{Compound, Tag};
use std::collections::HashMap;

/// The data version a template written in the 26.3 dialect declares:
/// `crates/horizon-core/src/version.rs:11` (`DATA_VERSION = 5023`), which is
/// also the `DataVersion` every 26.3 structure template carries.
pub const CURRENT_DATA_VERSION: i32 = 5023;

/// Which spelling a palette entry uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaletteDialect {
    /// `{id: "...", properties: {...}}` — 26.3, and what the engine's own
    /// writer emits (`template.rs:287-303`).
    #[default]
    Modern,
    /// `{Name: "...", Properties: {...}}` — up to 1.20.1, and what every
    /// blueprint's palette already looks like.
    Legacy,
}

impl PaletteDialect {
    /// The data version a template of this dialect may honestly declare.
    fn data_version(self, blueprint_data_version: i32) -> i32 {
        match self {
            PaletteDialect::Modern => CURRENT_DATA_VERSION,
            PaletteDialect::Legacy => blueprint_data_version,
        }
    }

    /// The `Name`/`id` key of a palette entry.
    fn name_key(self) -> &'static str {
        match self {
            PaletteDialect::Modern => "id",
            PaletteDialect::Legacy => "Name",
        }
    }

    /// The `Properties`/`properties` key of a palette entry.
    fn properties_key(self) -> &'static str {
        match self {
            PaletteDialect::Modern => "properties",
            PaletteDialect::Legacy => "Properties",
        }
    }
}

impl std::fmt::Display for PaletteDialect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PaletteDialect::Modern => "modern (id/properties)",
            PaletteDialect::Legacy => "legacy (Name/Properties)",
        })
    }
}

/// Where a [`Conversion`]'s anchor comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorSource {
    /// `optional_data.structurize.primary_offset`, present in all 9 515
    /// shipped blueprints.
    Declared,
    /// Recomputed as the single cell whose block entity carries
    /// `blueprintDataProvider` (`Blueprint.java:596-604`).
    RecoveredBlockEntity,
    /// Recomputed as `(size_x / 2, 0, size_z / 2)` (`Blueprint.java:602`).
    RecoveredDefault,
}

impl std::fmt::Display for AnchorSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            AnchorSource::Declared => "declared",
            AnchorSource::RecoveredBlockEntity => "recovered (blueprintDataProvider cell)",
            AnchorSource::RecoveredDefault => "recovered (size_x/2, 0, size_z/2)",
        })
    }
}

/// The anchor of a converted structure: a structure-local cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anchor {
    pub pos: Pos,
    pub source: AnchorSource,
}

/// One block of a structure template.
#[derive(Debug, Clone, PartialEq)]
pub struct StructureBlock {
    /// Structure-local position, `[x, y, z]`.
    pub pos: [i32; 3],
    /// Index into [`Structure::palette`].
    pub state: i32,
    /// Block-entity payload, if the cell holds one. Never carries `x`/`y`/`z`.
    pub nbt: Option<Compound>,
}

/// One entity of a structure template.
#[derive(Debug, Clone, PartialEq)]
pub struct StructureEntity {
    /// Position relative to the structure origin, `[x, y, z]`.
    pub pos: [f64; 3],
    /// The cell the entity hangs in, `[x, y, z]`.
    pub block_pos: [i32; 3],
    /// The entity's own NBT, including its `id`.
    pub nbt: Compound,
}

/// A vanilla structure template, as data.
#[derive(Debug, Clone, PartialEq)]
pub struct Structure {
    /// Root `DataVersion`.
    pub data_version: i32,
    /// Which spelling [`Structure::palette`] is written with.
    pub dialect: PaletteDialect,
    /// Root `size`, `[x, y, z]`.
    pub size: [i32; 3],
    /// Root `palette`, or the first entry of `palettes` when reading one of the
    /// 20 real templates that ship several.
    pub palette: Vec<BlockState>,
    /// Root `blocks`, one entry per cell.
    pub blocks: Vec<StructureBlock>,
    /// Root `entities`.
    pub entities: Vec<StructureEntity>,
}

/// A converted blueprint: the template plus the anchor vanilla cannot express.
#[derive(Debug, Clone, PartialEq)]
pub struct Conversion {
    pub structure: Structure,
    pub anchor: Anchor,
}

impl Conversion {
    /// Convert a parsed blueprint, in [`PaletteDialect::Modern`].
    pub fn from_blueprint(blueprint: &Blueprint) -> Result<Conversion, ConvertError> {
        Conversion::from_blueprint_with(blueprint, PaletteDialect::Modern)
    }

    /// Convert a parsed blueprint, choosing the palette spelling.
    pub fn from_blueprint_with(
        blueprint: &Blueprint,
        dialect: PaletteDialect,
    ) -> Result<Conversion, ConvertError> {
        Ok(Conversion {
            structure: Structure::from_blueprint_with(blueprint, dialect)?,
            anchor: find_anchor(blueprint),
        })
    }

    /// Flip this conversion to the other palette spelling, moving `DataVersion`
    /// with it.
    pub fn with_dialect(&self, dialect: PaletteDialect, blueprint_data_version: i32) -> Conversion {
        let mut structure = self.structure.clone();
        structure.dialect = dialect;
        structure.data_version = dialect.data_version(blueprint_data_version);
        Conversion {
            structure,
            anchor: self.anchor,
        }
    }

    /// The document as the engine's tree — what a placement path wants in hand.
    pub fn to_compound(&self) -> Compound {
        self.structure.to_compound()
    }

    /// The document's bytes (`NbtIo.write` output, uncompressed).
    pub fn to_nbt(&self) -> Result<Vec<u8>, ConvertError> {
        self.structure.to_nbt()
    }

    /// A real `.nbt` file's member: [`Conversion::to_nbt`] inside gzip.
    pub fn to_gzipped_nbt(&self) -> Result<Vec<u8>, ConvertError> {
        Ok(gzip::gzip(&self.to_nbt()?))
    }
}

impl Structure {
    /// Build the template from a blueprint, in [`PaletteDialect::Modern`].
    ///
    /// Refuses only what vanilla cannot represent: a block entity outside the
    /// grid, two block entities in one cell, and an entity without `Pos`.
    pub fn from_blueprint(blueprint: &Blueprint) -> Result<Structure, ConvertError> {
        Structure::from_blueprint_with(blueprint, PaletteDialect::Modern)
    }

    /// Same, with an explicit palette spelling (which fixes `DataVersion`).
    pub fn from_blueprint_with(
        blueprint: &Blueprint,
        dialect: PaletteDialect,
    ) -> Result<Structure, ConvertError> {
        let size = [
            i32::from(blueprint.size.x),
            i32::from(blueprint.size.y),
            i32::from(blueprint.size.z),
        ];

        // Block entities belong to the cell they sit in; index them by linear
        // cell index first so building the cell list stays linear.
        let mut pending: HashMap<i64, Compound> =
            HashMap::with_capacity(blueprint.block_entities.len());
        for entity in &blueprint.block_entities {
            if !in_grid(entity.pos, size) {
                return Err(ConvertError::BlockEntityOutOfGrid(entity.pos));
            }
            let index = linear_index(entity.pos, size);
            // Vanilla carries the position in `pos`; a payload with x/y/z in it
            // is not what its own writer produces (`template.rs:276-284`).
            if pending
                .insert(index, without(&entity.data, &["x", "y", "z"]))
                .is_some()
            {
                return Err(ConvertError::DuplicateBlockEntity(entity.pos));
            }
        }

        let mut blocks = Vec::with_capacity(blueprint.blocks.len());
        for cell in &blueprint.blocks {
            blocks.push(StructureBlock {
                pos: [cell.pos.x, cell.pos.y, cell.pos.z],
                state: i32::from(cell.palette_index),
                nbt: pending.remove(&linear_index(cell.pos, size)),
            });
        }
        if !pending.is_empty() {
            // Unreachable for a blueprint whose grid is complete, but say so
            // rather than dropping the payload in silence.
            return Err(ConvertError::OutOfRange(format!(
                "{} block entities matched no cell",
                pending.len()
            )));
        }

        let mut entities = Vec::with_capacity(blueprint.entities.len());
        for entity in &blueprint.entities {
            let pos = entity
                .pos
                .ok_or_else(|| ConvertError::EntityWithoutPosition(entity.id.clone()))?;
            let block_pos = match entity.tile_pos {
                Some(tile) => [tile.x, tile.y, tile.z],
                // `BlueprintUtil.java:144-152` stores `entity.blockPosition()`,
                // the floor of the position; a blueprint written before those
                // helper tags existed has none.
                None => [
                    pos[0].floor() as i32,
                    pos[1].floor() as i32,
                    pos[2].floor() as i32,
                ],
            };
            entities.push(StructureEntity {
                pos,
                block_pos,
                nbt: without(&entity.data, &["TileX", "TileY", "TileZ"]),
            });
        }

        Ok(Structure {
            data_version: dialect.data_version(blueprint.data_version),
            dialect,
            size,
            palette: blueprint
                .palette
                .iter()
                .map(|state| (**state).clone())
                .collect(),
            blocks,
            entities,
        })
    }

    /// The document as the engine's tree (`template.rs:318-322`).
    pub fn to_compound(&self) -> Compound {
        let mut root = Compound::new();
        root.insert("DataVersion".into(), Tag::Int(self.data_version));
        root.insert("size".into(), int_list(&self.size));
        root.insert(
            "palette".into(),
            Tag::List(
                self.palette
                    .iter()
                    .map(|state| palette_entry(state, self.dialect))
                    .collect(),
            ),
        );
        root.insert(
            "blocks".into(),
            Tag::List(self.blocks.iter().map(block_entry).collect()),
        );
        root.insert(
            "entities".into(),
            Tag::List(self.entities.iter().map(entity_entry).collect()),
        );
        root
    }

    /// The document's bytes (`NbtIo.write` output).
    pub fn to_nbt(&self) -> Result<Vec<u8>, ConvertError> {
        Ok(nbt::write("", &self.to_compound())?)
    }

    /// Read a structure template back — raw NBT or gzip, auto-detected.
    ///
    /// The palette is taken from the first list of `palettes` when the document
    /// has one, else from `palette` — vanilla's own order (`StructureTemplate.load`),
    /// and the engine's (`template.rs:352-370`). This converter only ever writes
    /// `palette`.
    pub fn parse(bytes: &[u8]) -> Result<Structure, ConvertError> {
        let raw = if gzip::is_gzip(bytes) {
            gzip::gunzip(bytes)?
        } else {
            bytes.to_vec()
        };
        let (_name, root) = nbt::parse(&raw)?;

        let data_version = int_tag(&root, "DataVersion")?;
        let size_values = int_list_tag(&root, "size")?;
        if size_values.len() != 3 {
            return Err(ConvertError::WrongType {
                tag: format!("size has {} entries", size_values.len()),
                expected: "a list of 3 ints",
                found: "list",
            });
        }
        let size = [size_values[0], size_values[1], size_values[2]];

        let palette_tag = match nbt::tag(&root, "palettes") {
            Some(palettes) => {
                let Tag::List(lists) = palettes else {
                    return Err(ConvertError::WrongType {
                        tag: "palettes".into(),
                        expected: "list",
                        found: type_name(palettes),
                    });
                };
                lists.first().ok_or_else(|| missing("palettes[0]"))?
            }
            None => nbt::tag(&root, "palette").ok_or_else(|| missing("palette"))?,
        };
        let Tag::List(entries) = palette_tag else {
            return Err(ConvertError::WrongType {
                tag: "palette".into(),
                expected: "list",
                found: type_name(palette_tag),
            });
        };
        let mut palette = Vec::with_capacity(entries.len());
        for entry in entries {
            let Tag::Compound(compound) = entry else {
                return Err(ConvertError::WrongType {
                    tag: "palette[]".into(),
                    expected: "compound",
                    found: type_name(entry),
                });
            };
            palette.push(BlockState::from_palette(compound)?);
        }
        // Keep the spelling that was found, so a read/write round trip does not
        // change it. A file mixing both is not something either writer emits;
        // the first entry decides.
        let dialect = match entries.first() {
            Some(Tag::Compound(entry)) if nbt::tag(entry, "Name").is_some() => PaletteDialect::Legacy,
            _ => PaletteDialect::Modern,
        };

        let mut blocks = Vec::new();
        for item in compound_list_tag(&root, "blocks")? {
            let pos = int_list_tag(item, "pos")?;
            if pos.len() != 3 {
                return Err(ConvertError::WrongType {
                    tag: "blocks[].pos".into(),
                    expected: "a list of 3 ints",
                    found: "list",
                });
            }
            let state = int_tag(item, "state")?;
            if state < 0 || state as usize >= palette.len() {
                return Err(ConvertError::OutOfRange(format!(
                    "blocks[].state = {state}, the palette holds {} entries",
                    palette.len()
                )));
            }
            let nbt = match nbt::tag(item, "nbt") {
                None => None,
                Some(Tag::Compound(payload)) => Some(payload.clone()),
                Some(other) => {
                    return Err(ConvertError::WrongType {
                        tag: "blocks[].nbt".into(),
                        expected: "compound",
                        found: type_name(other),
                    })
                }
            };
            blocks.push(StructureBlock {
                pos: [pos[0], pos[1], pos[2]],
                state,
                nbt,
            });
        }

        let mut entities = Vec::new();
        for item in compound_list_tag(&root, "entities")? {
            let pos = double_list_tag(item, "pos")?;
            if pos.len() != 3 {
                return Err(ConvertError::WrongType {
                    tag: "entities[].pos".into(),
                    expected: "a list of 3 doubles",
                    found: "list",
                });
            }
            let block_pos = int_list_tag(item, "blockPos")?;
            if block_pos.len() != 3 {
                return Err(ConvertError::WrongType {
                    tag: "entities[].blockPos".into(),
                    expected: "a list of 3 ints",
                    found: "list",
                });
            }
            let nbt = match nbt::tag(item, "nbt") {
                Some(Tag::Compound(payload)) => payload.clone(),
                Some(other) => {
                    return Err(ConvertError::WrongType {
                        tag: "entities[].nbt".into(),
                        expected: "compound",
                        found: type_name(other),
                    })
                }
                None => return Err(missing("entities[].nbt")),
            };
            entities.push(StructureEntity {
                pos: [pos[0], pos[1], pos[2]],
                block_pos: [block_pos[0], block_pos[1], block_pos[2]],
                nbt,
            });
        }

        Ok(Structure {
            data_version,
            dialect,
            size,
            palette,
            blocks,
            entities,
        })
    }

    /// How many cells the template covers (`size_x * size_y * size_z`).
    pub fn volume(&self) -> i64 {
        i64::from(self.size[0]) * i64::from(self.size[1]) * i64::from(self.size[2])
    }
}

/// Convert a blueprint into a structure template plus its anchor.
pub fn convert(blueprint: &Blueprint) -> Result<Conversion, ConvertError> {
    Conversion::from_blueprint(blueprint)
}

/// [`convert`], then serialise to a gzipped document (a real `.nbt` file).
pub fn convert_to_gzipped_nbt(blueprint: &Blueprint) -> Result<Vec<u8>, ConvertError> {
    Conversion::from_blueprint(blueprint)?.to_gzipped_nbt()
}

// ---------------------------------------------------------------------------
// The anchor
// ---------------------------------------------------------------------------

/// `true` when `pos` is a cell of a structure of that size.
fn in_grid(pos: Pos, size: [i32; 3]) -> bool {
    pos.x >= 0
        && pos.y >= 0
        && pos.z >= 0
        && pos.x < size[0]
        && pos.y < size[1]
        && pos.z < size[2]
}

/// The blueprint's own cell order (y outermost, then z, then x) as one number.
fn linear_index(pos: Pos, size: [i32; 3]) -> i64 {
    (i64::from(pos.y) * i64::from(size[2]) + i64::from(pos.z)) * i64::from(size[0])
        + i64::from(pos.x)
}

/// Structurize's own resolution order (`Blueprint.java:594-604`).
fn find_anchor(blueprint: &Blueprint) -> Anchor {
    if let Some(pos) = blueprint.primary_offset {
        return Anchor {
            pos,
            source: AnchorSource::Declared,
        };
    }

    let mut candidates = blueprint
        .block_entities
        .iter()
        .filter(|entity| nbt::tag(&entity.data, "blueprintDataProvider").is_some())
        .map(|entity| entity.pos);
    if let (Some(pos), None) = (candidates.next(), candidates.next()) {
        return Anchor {
            pos,
            source: AnchorSource::RecoveredBlockEntity,
        };
    }

    Anchor {
        pos: Pos::new(
            i32::from(blueprint.size.x) / 2,
            0,
            i32::from(blueprint.size.z) / 2,
        ),
        source: AnchorSource::RecoveredDefault,
    }
}

// ---------------------------------------------------------------------------
// NBT building blocks
// ---------------------------------------------------------------------------

fn missing(name: &str) -> ConvertError {
    ConvertError::MissingTag(name.to_string())
}

fn int_list(values: &[i32]) -> Tag {
    Tag::List(values.iter().map(|value| Tag::Int(*value)).collect())
}

fn palette_entry(state: &BlockState, dialect: PaletteDialect) -> Tag {
    let mut compound = Compound::new();
    compound.insert(dialect.name_key().into(), Tag::String(state.name.as_str().into()));
    if !state.properties.is_empty() {
        // Sorted by name: deterministic, and a compound is read by key anyway.
        let mut properties = Compound::new();
        for (key, value) in &state.properties {
            properties.insert(key.as_str().into(), Tag::String(value.as_str().into()));
        }
        compound.insert(dialect.properties_key().into(), Tag::Compound(properties));
    }
    Tag::Compound(compound)
}

fn block_entry(block: &StructureBlock) -> Tag {
    let mut compound = Compound::new();
    compound.insert("pos".into(), int_list(&block.pos));
    compound.insert("state".into(), Tag::Int(block.state));
    if let Some(payload) = &block.nbt {
        compound.insert("nbt".into(), Tag::Compound(payload.clone()));
    }
    Tag::Compound(compound)
}

fn entity_entry(entity: &StructureEntity) -> Tag {
    let mut compound = Compound::new();
    compound.insert(
        "pos".into(),
        Tag::List(
            entity
                .pos
                .iter()
                .map(|value| Tag::Double(value.to_bits()))
                .collect(),
        ),
    );
    compound.insert("blockPos".into(), int_list(&entity.block_pos));
    compound.insert("nbt".into(), Tag::Compound(entity.nbt.clone()));
    Tag::Compound(compound)
}

// ---------------------------------------------------------------------------
// Reading values back
// ---------------------------------------------------------------------------

fn int_tag(compound: &Compound, name: &str) -> Result<i32, ConvertError> {
    let tag = nbt::tag(compound, name).ok_or_else(|| missing(name))?;
    as_i64(tag)
        .map(|value| value as i32)
        .ok_or_else(|| ConvertError::WrongType {
            tag: name.to_string(),
            expected: "int",
            found: type_name(tag),
        })
}

fn int_list_tag(compound: &Compound, name: &str) -> Result<Vec<i32>, ConvertError> {
    let tag = nbt::tag(compound, name).ok_or_else(|| missing(name))?;
    let Tag::List(items) = tag else {
        return Err(ConvertError::WrongType {
            tag: name.to_string(),
            expected: "list",
            found: type_name(tag),
        });
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        match as_i64(item) {
            Some(value) => out.push(value as i32),
            None => {
                return Err(ConvertError::WrongType {
                    tag: format!("{name}[]"),
                    expected: "int",
                    found: type_name(item),
                })
            }
        }
    }
    Ok(out)
}

fn double_list_tag(compound: &Compound, name: &str) -> Result<Vec<f64>, ConvertError> {
    let tag = nbt::tag(compound, name).ok_or_else(|| missing(name))?;
    let Tag::List(items) = tag else {
        return Err(ConvertError::WrongType {
            tag: name.to_string(),
            expected: "list",
            found: type_name(tag),
        });
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        match as_f64(item) {
            Some(value) => out.push(value),
            None => {
                return Err(ConvertError::WrongType {
                    tag: format!("{name}[]"),
                    expected: "double",
                    found: type_name(item),
                })
            }
        }
    }
    Ok(out)
}

fn compound_list_tag<'a>(
    compound: &'a Compound,
    name: &str,
) -> Result<Vec<&'a Compound>, ConvertError> {
    let Some(tag) = nbt::tag(compound, name) else {
        return Ok(Vec::new());
    };
    let Tag::List(items) = tag else {
        return Err(ConvertError::WrongType {
            tag: name.to_string(),
            expected: "list",
            found: type_name(tag),
        });
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        match item {
            Tag::Compound(entry) => out.push(entry),
            other => {
                return Err(ConvertError::WrongType {
                    tag: format!("{name}[]"),
                    expected: "compound",
                    found: type_name(other),
                })
            }
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Everything that can stop a conversion or a re-read.
#[derive(Debug, Clone, PartialEq)]
pub enum ConvertError {
    /// The blueprint itself failed to parse.
    Blueprint(crate::blueprint::format::Error),
    /// A gzip layer is broken.
    Gzip(gzip::GzipError),
    /// An NBT document is malformed.
    Nbt(NbtError),
    /// A tag vanilla requires is absent.
    MissingTag(String),
    /// A tag is present with an unexpected type.
    WrongType {
        tag: String,
        expected: &'static str,
        found: &'static str,
    },
    /// A value vanilla cannot represent.
    OutOfRange(String),
    /// Two block entities claim the same cell.
    DuplicateBlockEntity(Pos),
    /// A block entity sits outside the structure grid.
    BlockEntityOutOfGrid(Pos),
    /// An entity has no `Pos`, so vanilla has nowhere to put it.
    EntityWithoutPosition(String),
}

impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConvertError::Blueprint(error) => write!(f, "blueprint: {error}"),
            ConvertError::Gzip(error) => write!(f, "gzip: {error}"),
            ConvertError::Nbt(error) => write!(f, "{error}"),
            ConvertError::MissingTag(name) => write!(f, "missing tag `{name}`"),
            ConvertError::WrongType {
                tag,
                expected,
                found,
            } => write!(f, "tag `{tag}` is a {found}, expected {expected}"),
            ConvertError::OutOfRange(detail) => write!(f, "value out of range: {detail}"),
            ConvertError::DuplicateBlockEntity(pos) => {
                write!(f, "two block entities at the same cell ({pos})")
            }
            ConvertError::BlockEntityOutOfGrid(pos) => {
                write!(f, "block entity outside the structure grid at ({pos})")
            }
            ConvertError::EntityWithoutPosition(id) => write!(f, "entity `{id}` has no Pos"),
        }
    }
}

impl std::error::Error for ConvertError {}

impl From<crate::blueprint::format::Error> for ConvertError {
    fn from(error: crate::blueprint::format::Error) -> Self {
        ConvertError::Blueprint(error)
    }
}

impl From<gzip::GzipError> for ConvertError {
    fn from(error: gzip::GzipError) -> Self {
        ConvertError::Gzip(error)
    }
}

impl From<NbtError> for ConvertError {
    fn from(error: NbtError) -> Self {
        ConvertError::Nbt(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blueprint::corpus;
    use crate::blueprint::format::{Block, BlockEntity, Entity, Size};
    use std::sync::Arc;

    /// A hand-built blueprint: 2x1x2, four cells, one block entity, one entity.
    fn tiny_blueprint() -> Blueprint {
        let air = Arc::new(BlockState {
            name: "minecraft:air".into(),
            properties: Default::default(),
        });
        let mut log_properties = std::collections::BTreeMap::new();
        log_properties.insert("axis".to_string(), "y".to_string());
        let log = Arc::new(BlockState {
            name: "minecraft:oak_log".into(),
            properties: log_properties,
        });

        let chest = Compound::from([
            ("id".into(), Tag::String("minecraft:chest".into())),
            ("x".into(), Tag::Short(1)),
            ("y".into(), Tag::Short(0)),
            ("z".into(), Tag::Short(1)),
            ("Items".into(), Tag::List(Vec::new())),
        ]);

        let entity = Compound::from([
            ("id".into(), Tag::String("minecraft:armor_stand".into())),
            (
                "Pos".into(),
                Tag::List(vec![
                    Tag::Double(1.5f64.to_bits()),
                    Tag::Double(0.0f64.to_bits()),
                    Tag::Double(0.5f64.to_bits()),
                ]),
            ),
            ("TileX".into(), Tag::Int(1)),
            ("TileY".into(), Tag::Int(0)),
            ("TileZ".into(), Tag::Int(0)),
        ]);

        let mut blocks = Vec::new();
        for y in 0..1u16 {
            for z in 0..2u16 {
                for x in 0..2u16 {
                    let is_log = x == 1 && z == 1;
                    blocks.push(Block {
                        pos: Pos::new(i32::from(x), i32::from(y), i32::from(z)),
                        palette_index: if is_log { 1 } else { 0 },
                        state: if is_log { log.clone() } else { air.clone() },
                    });
                }
            }
        }

        Blueprint {
            format_version: 1,
            data_version: 3465,
            size: Size { x: 2, y: 1, z: 2 },
            name: Some("test/tiny.blueprint".into()),
            architects: Vec::new(),
            required_mods: Vec::new(),
            primary_offset: Some(Pos::new(1, 0, 1)),
            palette: vec![air, log],
            blocks,
            block_entities: vec![BlockEntity {
                pos: Pos::new(1, 0, 1),
                id: "minecraft:chest".into(),
                data: chest,
            }],
            entities: vec![Entity {
                id: "minecraft:armor_stand".into(),
                pos: Some([1.5, 0.0, 0.5]),
                tile_pos: Some(Pos::new(1, 0, 0)),
                data: entity,
            }],
        }
    }

    #[test]
    fn the_conversion_maps_every_field() {
        let conversion = convert(&tiny_blueprint()).expect("the tiny blueprint converts");
        let structure = &conversion.structure;

        assert_eq!(structure.data_version, CURRENT_DATA_VERSION);
        assert_eq!(structure.dialect, PaletteDialect::Modern);
        assert_eq!(structure.size, [2, 1, 2]);
        assert_eq!(structure.volume(), 4);
        assert_eq!(structure.palette.len(), 2);
        assert_eq!(structure.palette[0].to_string(), "minecraft:air");
        assert_eq!(structure.palette[1].to_string(), "minecraft:oak_log[axis=y]");

        assert_eq!(structure.blocks.len(), 4);
        assert_eq!(structure.blocks[3].pos, [1, 0, 1]);
        assert_eq!(structure.blocks[3].state, 1);

        let chest = structure.blocks[3]
            .nbt
            .as_ref()
            .expect("the chest payload");
        assert_eq!(
            nbt::tag(chest, "id").and_then(as_text),
            Some("minecraft:chest".to_string())
        );
        assert!(
            nbt::tag(chest, "x").is_none(),
            "the position lives in `pos`, not in the payload"
        );
        assert!(nbt::tag(chest, "Items").is_some());

        assert_eq!(structure.entities.len(), 1);
        let stand = &structure.entities[0];
        assert_eq!(stand.pos, [1.5, 0.0, 0.5]);
        assert_eq!(stand.block_pos, [1, 0, 0]);
        assert!(nbt::tag(&stand.nbt, "TileX").is_none());
        assert_eq!(
            nbt::tag(&stand.nbt, "id").and_then(as_text),
            Some("minecraft:armor_stand".to_string())
        );

        assert_eq!(conversion.anchor.pos, Pos::new(1, 0, 1));
        assert_eq!(conversion.anchor.source, AnchorSource::Declared);
    }

    #[test]
    fn the_document_round_trips_through_parse() {
        let conversion = convert(&tiny_blueprint()).expect("converts");
        let raw = conversion.to_nbt().expect("the tree is writable");
        let read = Structure::parse(&raw).expect("and readable");
        assert_eq!(read, conversion.structure);

        let gzipped = conversion.to_gzipped_nbt().expect("writable");
        assert!(gzip::is_gzip(&gzipped));
        assert_eq!(Structure::parse(&gzipped).expect("readable"), conversion.structure);
    }

    fn palette_entries(conversion: &Conversion) -> Vec<Compound> {
        let root = conversion.to_compound();
        match nbt::tag(&root, "palette") {
            Some(Tag::List(items)) => items
                .iter()
                .map(|item| match item {
                    Tag::Compound(entry) => entry.clone(),
                    other => panic!("palette entry is {}", type_name(other)),
                })
                .collect(),
            other => panic!("palette is {:?}", other.map(type_name)),
        }
    }

    #[test]
    fn both_palette_spellings_write_and_read_back_as_themselves() {
        let blueprint = tiny_blueprint();

        let modern = Conversion::from_blueprint_with(&blueprint, PaletteDialect::Modern)
            .expect("converts");
        let entries = palette_entries(&modern);
        assert!(nbt::tag(&entries[0], "id").is_some(), "modern uses `id`");
        assert!(nbt::tag(&entries[0], "Name").is_none());
        assert_eq!(modern.structure.data_version, CURRENT_DATA_VERSION);

        let legacy = Conversion::from_blueprint_with(&blueprint, PaletteDialect::Legacy)
            .expect("converts");
        let entries = palette_entries(&legacy);
        assert!(nbt::tag(&entries[0], "Name").is_some(), "legacy uses `Name`");
        assert!(nbt::tag(&entries[0], "id").is_none());
        assert!(nbt::tag(&entries[1], "Properties").is_some());
        assert_eq!(
            legacy.structure.data_version, 3465,
            "the blueprint's own mcversion"
        );

        // Same blocks either way, and each spelling round trips as itself.
        assert_eq!(modern.structure.palette, legacy.structure.palette);
        assert_eq!(modern.structure.blocks, legacy.structure.blocks);
        assert_eq!(
            Structure::parse(&legacy.to_nbt().expect("writable"))
                .expect("readable")
                .dialect,
            PaletteDialect::Legacy
        );
        assert_eq!(
            Structure::parse(&modern.to_nbt().expect("writable"))
                .expect("readable")
                .dialect,
            PaletteDialect::Modern
        );
    }

    #[test]
    fn with_dialect_moves_the_data_version_with_the_spelling() {
        let blueprint = tiny_blueprint();
        let modern = convert(&blueprint).expect("converts");
        let legacy = modern.with_dialect(PaletteDialect::Legacy, blueprint.data_version);
        assert_eq!(legacy.structure.data_version, 3465);
        assert_eq!(legacy.structure.dialect, PaletteDialect::Legacy);
        assert_eq!(legacy.anchor, modern.anchor);
        assert_eq!(legacy.structure.blocks, modern.structure.blocks);
    }

    #[test]
    fn the_anchor_falls_back_the_way_structurize_does() {
        let mut blueprint = tiny_blueprint();
        blueprint.primary_offset = None;
        let anchor = convert(&blueprint).expect("converts").anchor;
        // The one block entity here carries no provider, so the default rule
        // applies: (size_x / 2, 0, size_z / 2) = (1, 0, 1).
        assert_eq!(anchor.pos, Pos::new(1, 0, 1));
        assert_eq!(anchor.source, AnchorSource::RecoveredDefault);

        blueprint.block_entities[0].data = Compound::from([(
            "blueprintDataProvider".into(),
            Tag::Compound(Compound::new()),
        )]);
        let anchor = convert(&blueprint).expect("converts").anchor;
        assert_eq!(anchor.pos, Pos::new(1, 0, 1));
        assert_eq!(anchor.source, AnchorSource::RecoveredBlockEntity);
    }

    #[test]
    fn duplicate_and_off_grid_block_entities_are_refused_by_name() {
        let mut blueprint = tiny_blueprint();
        let duplicate = blueprint.block_entities[0].clone();
        blueprint.block_entities.push(duplicate);
        assert_eq!(
            convert(&blueprint).expect_err("two payloads for one cell"),
            ConvertError::DuplicateBlockEntity(Pos::new(1, 0, 1))
        );

        let mut blueprint = tiny_blueprint();
        blueprint.block_entities[0].pos = Pos::new(9, 0, 0);
        assert_eq!(
            convert(&blueprint).expect_err("outside a 2x1x2 grid"),
            ConvertError::BlockEntityOutOfGrid(Pos::new(9, 0, 0))
        );
        assert!(convert(&blueprint)
            .expect_err("still refused")
            .to_string()
            .contains("outside the structure grid"));
    }

    #[test]
    fn an_entity_without_a_position_is_refused_and_without_tile_tags_is_floored() {
        let mut blueprint = tiny_blueprint();
        blueprint.entities[0].pos = None;
        blueprint.entities[0].tile_pos = None;
        assert!(matches!(
            convert(&blueprint).expect_err("no Pos"),
            ConvertError::EntityWithoutPosition(_)
        ));

        let mut blueprint = tiny_blueprint();
        blueprint.entities[0].tile_pos = None;
        let entity = &convert(&blueprint).expect("converts").structure.entities[0];
        assert_eq!(
            entity.block_pos,
            [1, 0, 0],
            "the floor of Pos when the Tile tags are absent"
        );
    }

    #[test]
    fn a_real_building_converts_cell_for_cell() {
        // Measured on fisherman2: 12x15x21 = 3 780 cells, 40 palette entries,
        // anchor (8,7,17) on the hut block, one block entity with a provider.
        let blueprint = Blueprint::parse(&corpus::read(
            "darkoak/agriculture/husbandry/fisherman2.blueprint",
        ))
        .expect("a shipped blueprint parses");
        let conversion = convert(&blueprint).expect("and converts");

        assert_eq!(conversion.structure.size, [12, 15, 21]);
        assert_eq!(conversion.structure.volume(), 3_780);
        assert_eq!(conversion.structure.blocks.len(), 3_780);
        assert_eq!(conversion.structure.palette.len(), 40);
        let expected_palette: Vec<BlockState> = blueprint
            .palette
            .iter()
            .map(|state| (**state).clone())
            .collect();
        assert_eq!(
            conversion.structure.palette, expected_palette,
            "the palette crosses over unchanged, order included"
        );
        assert_eq!(conversion.anchor.pos, Pos::new(8, 7, 17));
        assert_eq!(conversion.anchor.source, AnchorSource::Declared);

        // The anchor cell, by the template's own y-major order.
        let index: usize = ((7 * 21) + 17) * 12 + 8;
        let hut = &conversion.structure.blocks[index];
        assert_eq!(hut.pos, [8, 7, 17]);
        assert_eq!(hut.state, 36);
        assert_eq!(
            conversion.structure.palette[36].to_string(),
            "minecolonies:blockhutfisherman[facing=south]"
        );
        let payload = hut.nbt.as_ref().expect("the colony building sits there");
        assert!(
            nbt::tag(payload, "blueprintDataProvider").is_some(),
            "the provider payload survives the move into `nbt`"
        );
        assert!(nbt::tag(payload, "x").is_none(), "x/y/z stay out of the payload");
        assert_eq!(
            nbt::tag(payload, "id").and_then(as_text),
            Some("minecolonies:colonybuilding".to_string())
        );

        // One entry per cell: air is kept, so the image is exact.
        assert_eq!(conversion.structure.blocks.len(), blueprint.blocks.len());
        assert_eq!(
            conversion.structure.blocks[0].pos,
            [blueprint.blocks[0].pos.x, blueprint.blocks[0].pos.y, blueprint.blocks[0].pos.z]
        );
    }

    #[test]
    fn a_real_building_survives_the_round_trip_through_bytes() {
        let blueprint = Blueprint::parse(&corpus::read(
            "darkoak/agriculture/husbandry/fisherman2.blueprint",
        ))
        .expect("parses");
        let conversion = convert(&blueprint).expect("converts");

        let raw = conversion.to_nbt().expect("writable");
        assert_eq!(Structure::parse(&raw).expect("readable"), conversion.structure);

        let gzipped = conversion.to_gzipped_nbt().expect("writable");
        assert!(gzip::is_gzip(&gzipped));
        assert_eq!(
            Structure::parse(&gzipped).expect("readable"),
            conversion.structure,
            "the gzip member is what a real .nbt file is"
        );
    }

    #[test]
    fn the_five_levels_of_a_building_all_convert() {
        // Levels 1 to 5 of the same building, on the real pack: one conversion
        // each, every cell accounted for, palette and block entities growing.
        let expected_palette = [44usize, 51, 79, 106, 143];
        let expected_entities = [2usize, 2, 4, 7, 14];
        for level in 1usize..=5 {
            let file = format!("darkoak/fundamentals/builder{level}.blueprint");
            let blueprint =
                Blueprint::parse(&corpus::read(&file)).expect("a shipped level parses");
            let conversion = convert(&blueprint).expect("and converts");
            assert_eq!(conversion.structure.size, [14, 14, 15], "{file}");
            assert_eq!(conversion.structure.blocks.len(), 2_940, "{file}");
            assert_eq!(
                conversion.structure.palette.len(),
                expected_palette[level - 1],
                "{file}"
            );
            let payloads = conversion
                .structure
                .blocks
                .iter()
                .filter(|block| block.nbt.is_some())
                .count();
            assert_eq!(payloads, expected_entities[level - 1], "{file}");
            assert_eq!(conversion.anchor.pos, Pos::new(7, 5, 3), "{file}");
        }
    }

    #[test]
    fn converting_the_pack_keeps_every_cell_of_every_sampled_file() {
        // The counting test: 24 files spread over the whole pack, each parsed,
        // converted, and held against the blueprint's own cell count.
        let sample = corpus::sample(24);
        assert_eq!(sample.len(), 24);
        let mut cells = 0usize;
        for (file, bytes) in &sample {
            let blueprint = Blueprint::parse(bytes)
                .unwrap_or_else(|error| panic!("{file} did not parse: {error}"));
            let conversion = convert(&blueprint)
                .unwrap_or_else(|error| panic!("{file} did not convert: {error}"));
            assert_eq!(
                conversion.structure.blocks.len(),
                blueprint.blocks.len(),
                "{file}: one template entry per blueprint cell"
            );
            assert_eq!(
                conversion.structure.size,
                [
                    i32::from(blueprint.size.x),
                    i32::from(blueprint.size.y),
                    i32::from(blueprint.size.z)
                ],
                "{file}"
            );
            assert_eq!(
                conversion.structure.palette.len(),
                blueprint.palette.len(),
                "{file}"
            );
            assert_eq!(
                conversion.anchor.source,
                AnchorSource::Declared,
                "{file}: every shipped file declares its anchor"
            );
            cells += conversion.structure.blocks.len();
        }
        assert!(
            cells > 100_000,
            "24 real structures hold well over 100 000 cells, saw {cells}"
        );
    }

    #[test]
    fn four_sampled_files_also_survive_the_nbt_round_trip() {
        // The counting test above converts everything; this one takes the first
        // four of the same sample and proves the document they become reads
        // back identical — palette, payloads, entities included.
        for (file, bytes) in corpus::sample(24).iter().take(4) {
            let blueprint =
                Blueprint::parse(bytes).unwrap_or_else(|error| panic!("{file}: {error}"));
            let conversion =
                convert(&blueprint).unwrap_or_else(|error| panic!("{file}: {error}"));
            let raw = conversion
                .to_nbt()
                .unwrap_or_else(|error| panic!("{file}: {error}"));
            let read = Structure::parse(&raw).unwrap_or_else(|error| panic!("{file}: {error}"));
            assert_eq!(read, conversion.structure, "{file}");
        }
    }

    #[test]
    fn a_hand_written_template_reads_and_a_broken_one_is_refused() {
        // A template with `palettes` (20 of the engine's 1 511 ship several):
        // the first list is the one read.
        let mut entry = Compound::new();
        entry.insert("id".into(), Tag::String("minecraft:stone".into()));
        let mut root = Compound::new();
        root.insert("DataVersion".into(), Tag::Int(CURRENT_DATA_VERSION));
        root.insert(
            "size".into(),
            Tag::List(vec![Tag::Int(1), Tag::Int(1), Tag::Int(1)]),
        );
        root.insert(
            "palettes".into(),
            Tag::List(vec![Tag::List(vec![Tag::Compound(entry)])]),
        );
        let mut block = Compound::new();
        block.insert(
            "pos".into(),
            Tag::List(vec![Tag::Int(0), Tag::Int(0), Tag::Int(0)]),
        );
        block.insert("state".into(), Tag::Int(0));
        root.insert("blocks".into(), Tag::List(vec![Tag::Compound(block)]));
        let bytes = nbt::write("", &root).expect("writable");
        let structure = Structure::parse(&bytes).expect("palettes fallback is read");
        assert_eq!(structure.palette.len(), 1);
        assert_eq!(structure.palette[0].name, "minecraft:stone");
        assert_eq!(structure.blocks.len(), 1);
        assert!(structure.entities.is_empty(), "a missing list is empty");

        // A state index outside the palette is named, not clamped.
        let mut bad = root.clone();
        let mut block = Compound::new();
        block.insert(
            "pos".into(),
            Tag::List(vec![Tag::Int(0), Tag::Int(0), Tag::Int(0)]),
        );
        block.insert("state".into(), Tag::Int(7));
        bad.insert("blocks".into(), Tag::List(vec![Tag::Compound(block)]));
        let bytes = nbt::write("", &bad).expect("writable");
        let error = Structure::parse(&bytes).expect_err("index 7 does not exist");
        assert!(
            matches!(&error, ConvertError::OutOfRange(detail) if detail.contains("state = 7")),
            "got {error}"
        );
    }

    #[test]
    fn a_template_without_a_size_is_refused() {
        let mut root = Compound::new();
        root.insert("DataVersion".into(), Tag::Int(CURRENT_DATA_VERSION));
        root.insert("palette".into(), Tag::List(Vec::new()));
        let bytes = nbt::write("", &root).expect("writable");
        assert_eq!(
            Structure::parse(&bytes).expect_err("no size"),
            ConvertError::MissingTag("size".into())
        );
    }
}
