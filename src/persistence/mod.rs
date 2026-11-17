//! What a colony writes to disk, and what it reads back.
//!
//! Java reference: `core/colony/Colony.java:911-1005` (`write`) and `:760-795`
//! (`read`), `core/colony/Colony.java:725-745` (`loadColony`, which gives up on
//! one colony rather than on the save), and the managers that write into the
//! colony's own compound — citizens, buildings, work orders, permissions, plus the
//! research tree. Every key, and the sentence that fixes it, is in [`format`].
//!
//! ```text
//! colony compound (Colony.write)
//!   data_version: int          id: int        name: string
//!   dimension: string          center: {x,y,z}    level: int   ← port key
//!   permissionVersion: int     ownerid: string
//!   ranks: [ {id, name, is_initial, is_colony_manager, is_hostile} ]
//!   owners: [ {id, rank} ]     permissionMap: [ {rank, flags: [name…]} ]
//!   citizenManager: { citizens: [ {id, name, saturation, asleep, justAte} ] }
//!   buildingManager: { buildings: [ …crate::building's record… ] }
//!   work: { workOrders: [ …crate::colony::ordre's record… ], newsystem: byte }
//!   research: { researchTree: [ {state, id, branch, progress, depth} ] }
//! ```
//!
//! **Three questions, one answer each.** [`write`] produces the compound above
//! from a `Colony`, its building records, one [`Needs`] per citizen and a
//! [`Progress`]. [`read`] turns it back into the same four things, or names why
//! it will not. [`Loaded`] carries the *repairs*: what the read dropped or
//! defaulted, which is the half of compatibility a `Result` cannot say.
//!
//! **A refusal is for a save that is not a colony; a repair is for a save that is
//! one, slightly.** Java's own line is *per colony*: `loadColony` catches every
//! exception and answers `null`, so a broken citizen id costs the whole colony
//! (`CitizenManager.java:178-180`), while a building whose type the registry does
//! not know only costs that building (`RegisteredStructureManager.java:172-176`).
//! [`Refusal`] is the first half, [`Repair`] the second, and every variant of each
//! says which file and line it comes from.
//!
//! **What this module is not.** It is not `manager::persist`: that writes the
//! *world* file (`{colonies: […]}`, `chunkClaims`), and it nests the flat colony
//! record `crate::colony` already produces. This is the *per-colony* file, in
//! Java's nesting, with the two things the flat record has no room for —
//! permissions as Java writes them, and the research tree. `wiring` still anchors
//! the flat record in the main building's block entity; connecting the two is one
//! call in `wiring`, which this agent does not own (`MANQUANT.md`, last section).
//!
//! **No host call, no clock, no file.** A wasm mod has no `std::fs`, so what is
//! written is a `Compound` — the same shape Java's `colony<id>.dat` holds — and
//! the placement is the caller's business.

mod access;
pub mod format;
mod roster;
mod workshop;

use std::collections::BTreeMap;

use mod_api::{Compound, Tag};

pub use access::ACTION_NAMES;
pub use format::{KEYS, Key, Presence, TagKind};

use crate::building;
use crate::citizens::Needs;
use crate::colony::colonie;
use crate::colony::ordre::WorkOrder;
use crate::colony::Colony;
use crate::research::Progress;

/// `ColonyConstants.DATA_VERSION` (`api/util/constant/ColonyConstants.java:19`),
/// the only version this crate writes and the highest it reads.
pub const DATA_VERSION: i32 = 1;

/// Why a compound is not a colony this port will load.
///
/// Each variant is a rule that exists in Java, except the two marked as the
/// port's own. A variant is **not** a `lang` key: those live in
/// `assets/minecolonies/lang/`, which this module does not own. The name is
/// namespaced so it can never be mistaken for a player-facing sentence.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this module's own refusal set, closed on purpose
pub enum Refusal {
    /// No `id` at all (`Colony.java:916` / `:729`).
    MissingId,
    /// An id of 0 or less: `manager::NO_COLONY_ID` answers for every unclaimed
    /// position, and `ColonyManager` hands out ids from 1.
    IdNotPositive { id: i32 },
    /// No `dimension`, or a blank one: `new ResourceLocation("")` throws
    /// (`Colony.java:733`) and `loadColony` drops the colony (`:740-744`).
    MissingDimension,
    /// No `citizenManager.citizens` list (`CitizenManager.java:178`).
    MissingCitizens,
    /// No `buildingManager.buildings` list (`RegisteredStructureManager.java:160`).
    MissingBuildings,
    /// Not exactly one town hall: `BlockHutTownHall.java:219-234` refuses a
    /// second, and a colony without one has nowhere to put one later.
    TownHallCount { found: usize },
    /// A `data_version` above [`DATA_VERSION`]. **A rule of this port's own**:
    /// Java writes the tag (`Colony.java:913`) and reads it nowhere.
    UnknownVersion { found: i32, known: i32 },
    /// A citizen record with no `id` (`CitizenDataManager.java:16` reads 0, an id no
    /// factory hands out).
    CitizenWithoutId { index: usize },
    /// Two citizens with one id: `Collectors.toMap` throws
    /// (`CitizenManager.java:178-180`), out of `loadColony`'s catch.
    DuplicateCitizen { id: i32 },
    /// Two building records at one position: `ImmutableMap.Builder.put` throws
    /// (`RegisteredStructureManager.java:769`), same catch.
    DuplicateBuilding { at: [i32; 3] },
    /// The colony's light building list and its records disagree — a write-side
    /// refusal, so a work order cannot be handed to a hut no record stands for.
    IdentityDiverges { at: [i32; 3] },
    /// The needs map and the citizen list disagree about one citizen.
    RosterDiverges { id: i32 },
    /// A member record with no uuid, or one that does not parse:
    /// `UUID.fromString` throws (`Permissions.java:391`).
    MemberWithoutId { index: usize },
    /// A member record with no `rank`, or a name no `OldRank` holds. Java's
    /// `getInt` answers 0, which is the Owner rank (`Rank.java:36-40`).
    MemberWithoutRank { id: String },
    /// A rank id outside the five (`Permissions.java:1054-1080` lets a colony add
    /// its own; this port's ladder is an enum).
    UnknownRank { found: i32 },
    /// A `permissionVersion` above `access::PERMISSIONS_VERSION` (5,
    /// `Permissions.java:119`). **A rule of this port's own**: Java reads the
    /// number and hopes.
    UnknownPermissionsVersion { found: i32, known: i32 },
    /// The stored `level` (a port key) disagrees with the town hall's, which is
    /// what stands in for a colony's level (`CitizenManager.java:596`).
    LevelDisagrees { saved: i32, town_hall: i32 },
}

impl Refusal {
    /// A stable name for logs and tests, namespaced like a registry entry.
    pub fn name(&self) -> &'static str {
        match self {
            Self::MissingId => "persistence.refusal.missing_id",
            Self::IdNotPositive { .. } => "persistence.refusal.id_not_positive",
            Self::MissingDimension => "persistence.refusal.missing_dimension",
            Self::MissingCitizens => "persistence.refusal.missing_citizens",
            Self::MissingBuildings => "persistence.refusal.missing_buildings",
            Self::TownHallCount { .. } => "persistence.refusal.town_hall_count",
            Self::UnknownVersion { .. } => "persistence.refusal.unknown_version",
            Self::CitizenWithoutId { .. } => "persistence.refusal.citizen_without_id",
            Self::DuplicateCitizen { .. } => "persistence.refusal.duplicate_citizen",
            Self::DuplicateBuilding { .. } => "persistence.refusal.duplicate_building",
            Self::IdentityDiverges { .. } => "persistence.refusal.identity_diverges",
            Self::RosterDiverges { .. } => "persistence.refusal.roster_diverges",
            Self::MemberWithoutId { .. } => "persistence.refusal.member_without_id",
            Self::MemberWithoutRank { .. } => "persistence.refusal.member_without_rank",
            Self::UnknownRank { .. } => "persistence.refusal.unknown_rank",
            Self::UnknownPermissionsVersion { .. } => {
                "persistence.refusal.unknown_permissions_version"
            }
            Self::LevelDisagrees { .. } => "persistence.refusal.level_disagrees",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

/// What a read dropped or defaulted. A repair is never fatal: Java logs these and
/// carries on, and so does the port — but it hands the list back, because a log
/// line nobody reads is not a record.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this module's own repair set, closed on purpose
pub enum Repair {
    /// `center` was absent, so the colony sits at the origin (`Colony.java:732`).
    CenterDefaulted,
    /// A citizen record with no `name` (`CitizenData.java:1427` reads "").
    CitizenUnnamed { id: i32 },
    /// A record in the citizen list that is not a compound
    /// (`CitizenManager.java:178` streams compounds only).
    CitizenDropped { index: usize },
    /// No `saturation`: Java's `getDouble` answers 0.0, a starving citizen
    /// (`CitizenData.java:1460`), and the port keeps it.
    SaturationMissing { id: i32 },
    /// A saturation outside `0..=60` (`src/citizens/needs.rs:32`), clamped.
    SaturationOutOfRange { id: i32 },
    /// A building record whose `type` is absent or unknown
    /// (`BuildingDataManager.java:35-49`); the colony keeps the rest.
    BuildingDropped { kind: String },
    /// A building's worker names no citizen of this colony
    /// (`WorkerBuildingModule.java:129-133`); the hut is left vacant.
    WorkerUnassigned { at: [i32; 3], worker: i32 },
    /// A save without `newsystem`, so every order is dropped on purpose
    /// (`WorkManager.java:250-254`).
    OrdersFromOldSave { count: usize },
    /// An order record that does not read (`AbstractWorkOrder.java:232-277`
    /// answers null and the manager skips it).
    OrderDropped { index: usize },
    /// An order record with no `building`: a Java record, whose structure path
    /// lives under two other keys (`AbstractWorkOrder.java:717-719`).
    OrderWithoutKind { id: i32 },
    /// Two order records with one id; Java's map keeps the last
    /// (`WorkManager.java:57` is a map, `:333` is its `put`).
    OrderDuplicated { id: i32 },
    /// A claim on a hut that no longer stands, released on read
    /// (`WorkManager.java:269-272`).
    ClaimReleased { id: i32 },
    /// Research records the tree refused (`LocalResearchTree.java:382-390` warns
    /// on an unknown research and keeps the rest).
    ResearchDropped { count: u32 },
    /// Two member records for one uuid: Java's map keeps the last
    /// (`Permissions.java:412`).
    MemberDuplicated { id: String },
    /// A member whose rank the save's own table does not hold
    /// (`Permissions.java:410-417` only adds a resolved rank).
    MemberDropped { id: String },
    /// The owner came back from `ownerid`, as `restoreOwnerIfNull` does
    /// (`Permissions.java:453-469`).
    OwnerRestored,
    /// No owner left: Java calls `setOwnerAbandoned` in the same breath.
    NoOwner,
    /// The save's `permissionMap` is not this port's table. The port keeps its
    /// compiled masks; an edited Java colony therefore loses its edits here.
    RanksDiffer { rank: crate::colony::permissions::Rank },
}

impl Repair {
    /// A stable name for logs and tests, namespaced like [`Refusal::name`].
    pub fn name(&self) -> &'static str {
        match self {
            Self::CenterDefaulted => "persistence.repair.center_defaulted",
            Self::CitizenUnnamed { .. } => "persistence.repair.citizen_unnamed",
            Self::CitizenDropped { .. } => "persistence.repair.citizen_dropped",
            Self::SaturationMissing { .. } => "persistence.repair.saturation_missing",
            Self::SaturationOutOfRange { .. } => "persistence.repair.saturation_out_of_range",
            Self::BuildingDropped { .. } => "persistence.repair.building_dropped",
            Self::WorkerUnassigned { .. } => "persistence.repair.worker_unassigned",
            Self::OrdersFromOldSave { .. } => "persistence.repair.orders_from_old_save",
            Self::OrderDropped { .. } => "persistence.repair.order_dropped",
            Self::OrderWithoutKind { .. } => "persistence.repair.order_without_kind",
            Self::OrderDuplicated { .. } => "persistence.repair.order_duplicated",
            Self::ClaimReleased { .. } => "persistence.repair.claim_released",
            Self::ResearchDropped { .. } => "persistence.repair.research_dropped",
            Self::MemberDuplicated { .. } => "persistence.repair.member_duplicated",
            Self::MemberDropped { .. } => "persistence.repair.member_dropped",
            Self::OwnerRestored => "persistence.repair.owner_restored",
            Self::NoOwner => "persistence.repair.no_owner",
            Self::RanksDiffer { .. } => "persistence.repair.ranks_differ",
        }
    }
}

impl std::fmt::Display for Repair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// A colony that was read, plus what reading it cost.
///
/// `colony.buildings` is the light list `crate::colony` keys work orders on, and
/// it is built from `buildings` — never read from the save — so the two cannot
/// say different things.
#[derive(Clone, Debug)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Loaded {
    pub colony: Colony,
    pub buildings: Vec<building::Building>,
    pub needs: BTreeMap<i32, Needs>,
    pub research: Progress,
    pub repairs: Vec<Repair>,
}

/// A colony as the compound Java writes to `colony<id>.dat`.
///
/// **Refused, not repaired, on the three things a caller can get wrong**: the
/// light building list must match the records ([`Refusal::IdentityDiverges`]),
/// the needs map must match the citizen list ([`Refusal::RosterDiverges`]), and
/// the colony's level must be the town hall's ([`Refusal::LevelDisagrees`]).
/// Writing past those would produce a save that reads back as a different colony.
pub fn write(
    colony: &Colony,
    buildings: &[building::Building],
    needs: &BTreeMap<i32, Needs>,
    research: &Progress,
) -> Result<Compound, Refusal> {
    workshop::check_identity(colony, buildings)?;
    let hall = workshop::town_hall(buildings)?;
    let level = workshop::level_of(hall);
    if colony.level() != level {
        return Err(Refusal::LevelDisagrees {
            saved: colony.level(),
            town_hall: hall.level(),
        });
    }

    let mut data = Compound::from([
        (format::KEY_DATA_VERSION.into(), Tag::Int(DATA_VERSION)),
        (format::KEY_ID.into(), Tag::Int(colony.id)),
        (
            format::KEY_NAME.into(),
            Tag::String(colony.name.as_str().into()),
        ),
        (
            format::KEY_DIMENSION.into(),
            Tag::String(colony.dimension.as_str().into()),
        ),
        (format::KEY_CENTER.into(), nbt::position(colony.center)),
        (format::KEY_LEVEL.into(), Tag::Int(level)),
    ]);
    access::write(&mut data, &colony.members);
    for (key, tag) in roster::write(colony, needs)? {
        data.insert(key, tag);
    }
    for (key, tag) in workshop::write(buildings) {
        data.insert(key, tag);
    }
    for (key, tag) in written_work(colony) {
        data.insert(key, tag);
    }
    for (key, tag) in written_research(research) {
        data.insert(key, tag);
    }
    Ok(data)
}

/// A colony from its compound, or the name of the reason it is not one.
///
/// The order of the checks is the order Java's own read takes them in, because a
/// refusal is a statement about the file: an unknown `data_version` is decided
/// before anything is believed, an id and a dimension before a colony exists, the
/// roster before the buildings (which hire from it), the town hall before the
/// level (which is its level), and the permissions last, since they gate nothing
/// a read needs.
pub fn read(data: &Compound) -> Result<Loaded, Refusal> {
    let mut repairs = Vec::new();

    let version = nbt::int(data, format::KEY_DATA_VERSION).unwrap_or(0);
    if version > DATA_VERSION {
        return Err(Refusal::UnknownVersion {
            found: version,
            known: DATA_VERSION,
        });
    }
    let id = nbt::int(data, format::KEY_ID).ok_or(Refusal::MissingId)?;
    if id <= 0 {
        return Err(Refusal::IdNotPositive { id });
    }
    let dimension = match nbt::text(data, format::KEY_DIMENSION) {
        Some(text) if !text.trim().is_empty() => text,
        _ => return Err(Refusal::MissingDimension),
    };
    let name = nbt::text(data, format::KEY_NAME).unwrap_or_default();
    let center = match nbt::position_in(data, format::KEY_CENTER) {
        Some(at) => at,
        None => {
            repairs.push(Repair::CenterDefaulted);
            [0, 0, 0]
        }
    };

    let (citizens, needs) = roster::read(data, &mut repairs)?;
    let buildings = workshop::read(data, &citizens, &mut repairs)?;
    let hall = workshop::town_hall(&buildings)?;
    let level = workshop::level_of(hall);
    if let Some(saved) = nbt::int(data, format::KEY_LEVEL) {
        if saved != level {
            return Err(Refusal::LevelDisagrees {
                saved,
                town_hall: hall.level(),
            });
        }
    }
    let members = access::read(data, &mut repairs)?;
    let orders = read_orders(data, &buildings, &mut repairs);
    let research = read_research(data, &mut repairs);

    // The flat record `crate::colony` reads, built from what this module already
    // decided — so the light building list, the order counter and the members are
    // the colony record's own answers and not a second reading of the file.
    let flat_citizens: Vec<Compound> = citizens
        .iter()
        .map(|citizen| {
            Compound::from([
                (
                    colonie::KEY_CITIZEN_ID.into(),
                    Tag::Int(citizen.id),
                ),
                (
                    colonie::KEY_CITIZEN_NAME.into(),
                    Tag::String(citizen.name.as_str().into()),
                ),
            ])
        })
        .collect();
    let flat_orders: Vec<Compound> = orders.iter().filter_map(WorkOrder::to_compound).collect();
    let mut flat = Compound::from([
        (colonie::KEY_ID.into(), Tag::Int(id)),
        (
            colonie::KEY_NAME.into(),
            Tag::String(name.as_str().into()),
        ),
        (colonie::KEY_LEVEL.into(), Tag::Int(level)),
        (
            colonie::KEY_DIMENSION.into(),
            Tag::String(dimension.as_str().into()),
        ),
        (colonie::KEY_CENTER.into(), nbt::position(center)),
        (
            colonie::KEY_CITIZENS.into(),
            nbt::compound_list(flat_citizens),
        ),
        (
            colonie::KEY_BUILDINGS.into(),
            nbt::compound_list(workshop::identity_records(&buildings)),
        ),
        (colonie::KEY_ORDERS.into(), nbt::compound_list(flat_orders)),
    ]);
    for (key, tag) in members.to_compound() {
        flat.insert(key, tag);
    }
    let colony = Colony::from_compound(&flat);

    Ok(Loaded {
        colony,
        buildings,
        needs,
        research,
        repairs,
    })
}

/// `{work: {workOrders: […], newsystem: byte}}`.
fn written_work(colony: &Colony) -> Compound {
    let records: Vec<Compound> = colony
        .orders
        .iter()
        .filter_map(WorkOrder::to_compound)
        .collect();
    Compound::from([(
        format::KEY_WORK.into(),
        Tag::Compound(Compound::from([
            (
                format::KEY_WORK_ORDERS.into(),
                nbt::compound_list(records),
            ),
            (format::KEY_NEW_SYSTEM.into(), nbt::boolean_tag(true)),
        ])),
    )])
}

/// The order list, read with Java's three answers: no `newsystem` drops them all,
/// a record that does not read is skipped, and a claim on a hut that is gone is
/// released (by `Colony::release_orphan_claims`, which runs on the flat record).
fn read_orders(
    data: &Compound,
    buildings: &[building::Building],
    repairs: &mut Vec<Repair>,
) -> Vec<WorkOrder> {
    let Some(Tag::Compound(manager)) = data.get(&format::KEY_WORK.into()) else {
        return Vec::new();
    };
    let Some(Tag::List(records)) = manager.get(&format::KEY_WORK_ORDERS.into()) else {
        return Vec::new();
    };
    if !manager.contains_key(&format::KEY_NEW_SYSTEM.into()) {
        repairs.push(Repair::OrdersFromOldSave {
            count: records.len(),
        });
        return Vec::new();
    }

    let standing: Vec<[i32; 3]> = buildings.iter().map(building::Building::at).collect();
    let mut orders: Vec<WorkOrder> = Vec::with_capacity(records.len());
    for (index, item) in records.iter().enumerate() {
        let Tag::Compound(record) = item else {
            repairs.push(Repair::OrderDropped { index });
            continue;
        };
        let Some(order) = WorkOrder::from_compound(record) else {
            repairs.push(Repair::OrderDropped { index });
            continue;
        };
        if order.building.is_empty() {
            repairs.push(Repair::OrderWithoutKind { id: order.id });
        }
        if let Some(by) = order.claimed_by() {
            if !standing.contains(&by) {
                repairs.push(Repair::ClaimReleased { id: order.id });
            }
        }
        match orders.iter().position(|held| held.id == order.id) {
            Some(slot) => {
                repairs.push(Repair::OrderDuplicated { id: order.id });
                orders[slot] = order;
            }
            None => orders.push(order),
        }
    }
    orders
}

/// `{research: {researchTree: […]}}`, the compound
/// `LocalResearchTree.writeToNBT` produces (`:344-357`).
fn written_research(research: &Progress) -> Compound {
    Compound::from([(
        format::KEY_RESEARCH.into(),
        Tag::Compound(research.to_compound()),
    )])
}

/// The research tree, read with Java's guard: no `research` key is a colony that
/// researched nothing (`Colony.java:786`), and a record the tree refuses is
/// counted by [`Progress::dropped`] rather than losing the colony.
fn read_research(data: &Compound, repairs: &mut Vec<Repair>) -> Progress {
    let Some(Tag::Compound(inner)) = data.get(&format::KEY_RESEARCH.into()) else {
        return Progress::new();
    };
    let progress = Progress::from_compound(inner);
    if progress.dropped() > 0 {
        repairs.push(Repair::ResearchDropped {
            count: progress.dropped(),
        });
    }
    progress
}

/// The tag reads and writes this module needs.
///
/// **This is the fourth copy of the same handful of functions**, and the module
/// headers of `src/colony/nbt.rs:9-11`, `src/building/nbt.rs:8-13` and
/// `src/research/nbt.rs:8-13` already say the same thing: each is `mod nbt;`,
/// private to its folder, and making one of them `pub(crate)` is a one-line change
/// to a file this agent does not own. What is added here and nowhere else is
/// [`double`] / [`double_of`]: a saturation is a `Double`, and `Tag::Double` holds
/// its bits (`crates/horizon-core/src/nbt.rs:44-46`).
mod nbt {
    use mod_api::{Compound, Tag};

    /// A position, as `BlockPosUtil` lays one out (`api/util/BlockPosUtil.java:190-197`).
    pub fn position(at: [i32; 3]) -> Tag {
        Tag::compound([
            ("x", Tag::Int(at[0])),
            ("y", Tag::Int(at[1])),
            ("z", Tag::Int(at[2])),
        ])
    }

    /// The position stored under `key`. A missing coordinate is `None`, not 0.
    pub fn position_in(data: &Compound, key: &str) -> Option<[i32; 3]> {
        let tag = data.get(&key.into())?;
        Some([
            number(tag.get("x")?)?,
            number(tag.get("y")?)?,
            number(tag.get("z")?)?,
        ])
    }

    /// An integer under `key`, in whichever width it was saved.
    pub fn int(data: &Compound, key: &str) -> Option<i32> {
        number(data.get(&key.into())?)
    }

    /// The integer a tag holds: the nested case.
    pub fn int_of(tag: &Tag) -> Option<i32> {
        number(tag)
    }

    /// A string under `key`. A tag of another kind is `None`, not its debug form.
    pub fn text(data: &Compound, key: &str) -> Option<String> {
        match data.get(&key.into())? {
            Tag::String(value) => Some(value.to_string_lossy()),
            _ => None,
        }
    }

    /// A boolean under `key`. NBT has none: `putBoolean` writes a `Byte`, and a
    /// hand-written `/data merge` may leave an `Int`.
    pub fn boolean(data: &Compound, key: &str) -> Option<bool> {
        match data.get(&key.into())? {
            Tag::Byte(value) => Some(*value != 0),
            Tag::Int(value) => Some(*value != 0),
            _ => None,
        }
    }

    /// The tag a boolean is written as: a `Byte`, like `putBoolean`.
    pub fn boolean_tag(value: bool) -> Tag {
        Tag::Byte(if value { 1 } else { 0 })
    }

    /// A `double`, as the bits the tag holds.
    pub fn double(value: f64) -> Tag {
        Tag::Double(value.to_bits())
    }

    /// The `double` a tag holds. A `Float` is **not** read as one: Java's
    /// `getDouble` answers 0 for it, and a save is read the way Java reads it.
    pub fn double_of(tag: &Tag) -> Option<f64> {
        match tag {
            Tag::Double(bits) => Some(f64::from_bits(*bits)),
            _ => None,
        }
    }

    /// A list of compounds: NBT lists are homogeneous.
    pub fn compound_list(items: Vec<Compound>) -> Tag {
        Tag::List(items.into_iter().map(Tag::Compound).collect())
    }

    /// Every compound of the list under `key`. An absent or corrupt list is
    /// empty, which is Java's `getList`.
    pub fn compounds<'a>(data: &'a Compound, key: &str) -> Vec<&'a Compound> {
        match data.get(&key.into()) {
            Some(Tag::List(items)) => items
                .iter()
                .filter_map(|item| match item {
                    Tag::Compound(inner) => Some(inner),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    fn number(tag: &Tag) -> Option<i32> {
        match tag {
            Tag::Int(value) => Some(*value),
            Tag::Byte(value) => Some(i32::from(*value)),
            Tag::Short(value) => Some(i32::from(*value)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::building::{Kind, Module};
    use crate::colony::permissions::Rank;
    use crate::colony::ordre::WorkType;
    use crate::colony::Citizen;

    const OWNER: u128 = 0x0123_4567_89ab_cdef_fedc_ba98_7654_3210;
    const FRIEND: u128 = 2;
    const HALL: [i32; 3] = [10, 64, -20];
    const SITE: [i32; 3] = [16, 64, -20];
    const FARM: [i32; 3] = [4, 64, -20];

    fn buildings() -> Vec<building::Building> {
        let mut hall = building::Building::new(Kind::TownHall, HALL);
        hall.complete(3).expect("a town hall is built");
        let mut site = building::Building::new(Kind::Builder, SITE);
        site.complete(2).expect("the builder is built");
        site.set_parent(Some(HALL)).expect("the builder belongs to the hall");
        site.attach_module(Module::new("work", 2).expect("a named module"))
            .expect("two of two");
        site.hire(1).expect("a built hut hires");
        let mut farm = building::Building::new(Kind::Farmer, FARM);
        farm.complete(1).expect("the farm is built");
        farm.hire(2).expect("a built hut hires");
        vec![hall, site, farm]
    }

    fn colony() -> Colony {
        let list = buildings();
        let mut colony = Colony::new(7, "Bridgewater", "minecraft:overworld", HALL, OWNER);
        colony.set_level(3);
        for (id, name) in [(1, "Ada"), (2, "Bram"), (3, "Cleo"), (4, "Dirk"), (5, "Els")] {
            colony.citizens.push(Citizen::new(id, name));
        }
        for built in &list {
            colony.buildings.push(crate::colony::Building::new(
                &built.kind().path(),
                built.at(),
                built.level(),
            ));
        }
        colony.members.set(FRIEND, Rank::Friend);
        let first = colony.place_order(&Kind::Builder.path(), SITE, WorkType::Build, 2);
        let index = colony
            .orders
            .iter()
            .position(|order| order.id == first)
            .expect("just placed");
        colony.orders[index].claim(HALL).expect("a hut takes it");
        colony.place_order(&Kind::Farmer.path(), FARM, WorkType::Upgrade, 2);
        colony
    }

    fn needs() -> BTreeMap<i32, Needs> {
        let mut needs = BTreeMap::new();
        for id in 1..=5 {
            let mut held = Needs::new();
            held.saturation = 60.0 - f64::from(id);
            held.asleep = id == 3;
            held.just_ate = id == 2;
            needs.insert(id, held);
        }
        needs
    }

    fn research() -> Progress {
        fn record(state: i32, id: &str, branch: &str, progress: i32, depth: i32) -> Tag {
            Tag::compound([
                ("state", Tag::Int(state)),
                ("id", Tag::String(id.into())),
                ("branch", Tag::String(branch.into())),
                ("progress", Tag::Int(progress)),
                ("depth", Tag::Int(depth)),
            ])
        }
        Progress::from_compound(&Compound::from([(
            "researchTree".into(),
            Tag::List(vec![
                record(2, "civilian/woodwork", "civilian", 5, 1),
                record(1, "technology/glassblower", "technology", 12, 2),
            ]),
        )]))
    }

    fn save() -> (Colony, Vec<building::Building>, BTreeMap<i32, Needs>, Progress) {
        (colony(), buildings(), needs(), research())
    }

    fn written() -> Compound {
        let (colony, buildings, needs, research) = save();
        write(&colony, &buildings, &needs, &research).expect("the save writes")
    }

    fn refused(data: &Compound) -> Refusal {
        read(data).err().expect("the save is refused")
    }

    #[test]
    fn a_colony_of_three_huts_and_five_citizens_round_trips() {
        let (colony, buildings, needs, research) = save();
        let data = write(&colony, &buildings, &needs, &research).expect("writes");
        let read = read(&data).expect("reads");
        assert!(read.repairs.is_empty(), "a clean save needs no repair: {:?}", read.repairs);
        assert_eq!(read.colony, colony);
        assert_eq!(read.buildings, buildings);
        assert_eq!(read.needs, needs);
        assert_eq!(read.research.to_compound(), research.to_compound());
    }

    #[test]
    fn the_same_colony_writes_the_same_bytes_twice() {
        let (colony, buildings, needs, research) = save();
        let data = write(&colony, &buildings, &needs, &research).expect("writes");
        let read = read(&data).expect("reads");
        let again = write(&read.colony, &read.buildings, &read.needs, &read.research)
            .expect("what was read writes back");
        assert_eq!(again, data, "write, read, write is a fixed point");
    }

    #[test]
    fn the_colony_compound_carries_javas_keys_at_the_top() {
        let data = written();
        assert_eq!(nbt::int(&data, format::KEY_DATA_VERSION), Some(DATA_VERSION));
        assert_eq!(nbt::int(&data, format::KEY_ID), Some(7));
        assert_eq!(
            nbt::text(&data, format::KEY_NAME).as_deref(),
            Some("Bridgewater")
        );
        assert_eq!(
            nbt::text(&data, format::KEY_DIMENSION).as_deref(),
            Some("minecraft:overworld")
        );
        assert_eq!(nbt::position_in(&data, format::KEY_CENTER), Some(HALL));
        assert_eq!(nbt::int(&data, format::KEY_LEVEL), Some(3));
        for key in [
            format::KEY_RANKS,
            format::KEY_OWNERS,
            format::KEY_PERMISSION_MAP,
            format::KEY_CITIZEN_MANAGER,
            format::KEY_BUILDING_MANAGER,
            format::KEY_WORK,
            format::KEY_RESEARCH,
            format::KEY_PERMISSION_VERSION,
            format::KEY_OWNER_ID,
        ] {
            assert!(data.contains_key(&key.into()), "{key} is not written");
        }
    }

    /// Every path a written colony contains, with a list marked `[]` and the
    /// module key left as it is (normalised by `format::normalise`).
    fn walk(tag: &Tag, path: &str, out: &mut BTreeSet<String>) {
        let path = format::normalise(path);
        out.insert(path.clone());
        match tag {
            Tag::Compound(map) => {
                for (key, value) in map {
                    walk(value, &format!("{path}.{}", key.to_string_lossy()), out);
                }
            }
            Tag::List(items) => {
                for item in items {
                    walk(item, &format!("{path}[]"), out);
                }
            }
            _ => {}
        }
    }

    fn paths(data: &Compound) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for (key, tag) in data {
            walk(tag, &key.to_string_lossy(), &mut out);
        }
        out
    }

    #[test]
    fn every_path_a_save_contains_is_a_documented_row() {
        let data = written();
        for path in paths(&data) {
            assert!(
                format::documented(&path),
                "`{path}` is written and no row of format::KEYS describes it"
            );
        }
    }

    #[test]
    fn every_documented_row_is_a_path_a_save_contains() {
        let seen = paths(&written());
        for key in KEYS {
            let emitted = match key.path.strip_suffix(".*") {
                Some(prefix) => seen.iter().any(|path| path.starts_with(prefix)),
                None => seen.contains(key.path),
            };
            assert!(emitted, "`{}` is documented and never written", key.path);
        }
    }

    #[test]
    fn a_missing_id_is_refused() {
        let mut data = written();
        data.remove(&format::KEY_ID.into());
        assert_eq!(refused(&data), Refusal::MissingId);
        assert_eq!(refused(&Compound::new()), Refusal::MissingId);
    }

    #[test]
    fn an_id_of_zero_is_refused() {
        let mut data = written();
        data.insert(format::KEY_ID.into(), Tag::Int(0));
        assert_eq!(refused(&data), Refusal::IdNotPositive { id: 0 });
    }

    #[test]
    fn a_missing_or_blank_dimension_is_refused() {
        let mut data = written();
        data.insert(format::KEY_DIMENSION.into(), Tag::String("  ".into()));
        assert_eq!(refused(&data), Refusal::MissingDimension);
        let mut data = written();
        data.remove(&format::KEY_DIMENSION.into());
        assert_eq!(refused(&data), Refusal::MissingDimension);
    }

    #[test]
    fn a_version_this_port_does_not_know_is_refused() {
        let mut data = written();
        data.insert(format::KEY_DATA_VERSION.into(), Tag::Int(2));
        assert_eq!(
            refused(&data),
            Refusal::UnknownVersion {
                found: 2,
                known: DATA_VERSION
            }
        );
        // And a save from before the tag existed is a version 0 save.
        data.insert(format::KEY_DATA_VERSION.into(), Tag::Int(0));
        assert!(read(&data).is_ok());
    }

    #[test]
    fn a_missing_citizen_or_building_list_is_refused() {
        let mut data = written();
        data.remove(&format::KEY_CITIZEN_MANAGER.into());
        assert_eq!(refused(&data), Refusal::MissingCitizens);
        let mut data = written();
        data.remove(&format::KEY_BUILDING_MANAGER.into());
        assert_eq!(refused(&data), Refusal::MissingBuildings);
    }

    #[test]
    fn a_colony_needs_exactly_one_town_hall() {
        let mut data = written();
        let Some(Tag::Compound(manager)) = data.get_mut(&format::KEY_BUILDING_MANAGER.into()) else {
            panic!("buildingManager is a compound")
        };
        let Some(Tag::List(items)) = manager.get_mut(&format::KEY_BUILDINGS.into()) else {
            panic!("buildings is a list")
        };
        items.retain(|item| match item {
            Tag::Compound(record) => nbt::text(record, format::KEY_TYPE).as_deref()
                != Some("minecolonies:townhall"),
            _ => false,
        });
        assert_eq!(refused(&data), Refusal::TownHallCount { found: 0 });
    }

    #[test]
    fn a_level_that_disagrees_with_the_town_hall_is_refused() {
        let mut data = written();
        data.insert(format::KEY_LEVEL.into(), Tag::Int(4));
        assert_eq!(
            refused(&data),
            Refusal::LevelDisagrees {
                saved: 4,
                town_hall: 3
            }
        );
    }

    #[test]
    fn a_save_without_a_centre_sits_at_the_origin_and_says_so() {
        let mut data = written();
        data.remove(&format::KEY_CENTER.into());
        let read = read(&data).expect("a colony at the origin is a colony");
        assert_eq!(read.colony.center, [0, 0, 0]);
        assert_eq!(read.repairs, vec![Repair::CenterDefaulted]);
    }

    #[test]
    fn an_order_list_from_before_newsystem_is_dropped_as_java_drops_it() {
        let mut data = written();
        let Some(Tag::Compound(work)) = data.get_mut(&format::KEY_WORK.into()) else {
            panic!("work is a compound")
        };
        work.remove(&format::KEY_NEW_SYSTEM.into());
        let read = read(&data).expect("the colony still reads");
        assert!(read.colony.orders.is_empty(), "WorkManager.java:250-254");
        assert_eq!(read.repairs, vec![Repair::OrdersFromOldSave { count: 2 }]);
    }

    #[test]
    fn an_order_record_that_does_not_read_is_dropped_and_counted() {
        let mut data = written();
        let Some(Tag::Compound(work)) = data.get_mut(&format::KEY_WORK.into()) else {
            panic!("work is a compound")
        };
        let Some(Tag::List(items)) = work.get_mut(&format::KEY_WORK_ORDERS.into()) else {
            panic!("workOrders is a list")
        };
        let Tag::Compound(first) = &mut items[0] else {
            panic!("an order is a compound")
        };
        first.remove(&crate::colony::ordre::KEY_TYPE.into());
        let read = read(&data).expect("one order less is still a colony");
        assert_eq!(read.colony.orders.len(), 1);
        assert_eq!(read.repairs, vec![Repair::OrderDropped { index: 0 }]);
    }

    #[test]
    fn an_order_without_a_building_kind_is_kept_and_reported() {
        let mut data = written();
        let Some(Tag::Compound(work)) = data.get_mut(&format::KEY_WORK.into()) else {
            panic!("work is a compound")
        };
        let Some(Tag::List(items)) = work.get_mut(&format::KEY_WORK_ORDERS.into()) else {
            panic!("workOrders is a list")
        };
        let Tag::Compound(first) = &mut items[0] else {
            panic!("an order is a compound")
        };
        first.insert(
            crate::colony::ordre::KEY_BUILDING.into(),
            Tag::String("".into()),
        );
        let read = read(&data).expect("reads");
        assert_eq!(read.colony.orders.len(), 2);
        assert!(read
            .repairs
            .contains(&Repair::OrderWithoutKind { id: read.colony.orders[0].id }));
    }

    #[test]
    fn a_claim_on_a_hut_that_is_gone_is_released_and_reported() {
        let mut data = written();
        let Some(Tag::Compound(manager)) = data.get_mut(&format::KEY_BUILDING_MANAGER.into()) else {
            panic!("buildingManager is a compound")
        };
        let Some(Tag::List(items)) = manager.get_mut(&format::KEY_BUILDINGS.into()) else {
            panic!("buildings is a list")
        };
        // The hall is the hut the first order is claimed by: drop it, and keep
        // one town hall so the colony still has one.
        let Tag::Compound(hall) = &mut items[0] else {
            panic!("a building is a compound")
        };
        hall.insert(
            format::KEY_LOCATION.into(),
            nbt::position([10, 64, 99]),
        );
        let read = read(&data).expect("reads");
        assert_eq!(
            read.colony.orders[0].claimed_by(),
            None,
            "the claim is released on read (WorkManager.java:269-272)"
        );
        assert_eq!(
            read.repairs,
            vec![Repair::ClaimReleased {
                id: read.colony.orders[0].id
            }]
        );
    }

    #[test]
    fn two_order_records_with_one_id_keep_the_last_and_say_so() {
        let mut data = written();
        let Some(Tag::Compound(work)) = data.get_mut(&format::KEY_WORK.into()) else {
            panic!("work is a compound")
        };
        let Some(Tag::List(items)) = work.get_mut(&format::KEY_WORK_ORDERS.into()) else {
            panic!("workOrders is a list")
        };
        let copy = items[1].clone();
        items[1] = items[0].clone();
        items.push(copy);
        let read = read(&data).expect("reads");
        assert!(read
            .repairs
            .iter()
            .any(|repair| matches!(repair, Repair::OrderDuplicated { .. })));
    }

    #[test]
    fn research_records_that_do_not_read_are_counted_not_fatal() {
        let mut data = written();
        let Some(Tag::Compound(research)) = data.get_mut(&format::KEY_RESEARCH.into()) else {
            panic!("research is a compound")
        };
        let Some(Tag::List(items)) = research.get_mut(&"researchTree".into()) else {
            panic!("researchTree is a list")
        };
        // A record whose `state` is no `ResearchState`: dropped and counted, which
        // is `LocalResearchFactory.java:66` throwing the record away.
        items.push(Tag::compound([
            ("state", Tag::Int(9)),
            ("id", Tag::String("civilian/ghost".into())),
            ("branch", Tag::String("civilian".into())),
        ]));
        let read = read(&data).expect("reads");
        assert_eq!(read.research.len(), 2, "the two good records survive");
        assert_eq!(read.repairs, vec![Repair::ResearchDropped { count: 1 }]);
    }

    #[test]
    fn a_light_building_list_that_disagrees_with_the_records_is_refused_before_it_is_written() {
        let (mut colony, buildings, needs, research) = save();
        colony.buildings[2].level = 4;
        assert_eq!(
            write(&colony, &buildings, &needs, &research).err(),
            Some(Refusal::IdentityDiverges { at: FARM })
        );
    }

    #[test]
    fn a_level_the_town_hall_does_not_have_is_refused_before_it_is_written() {
        let (mut colony, buildings, needs, research) = save();
        colony.set_level(5);
        assert_eq!(
            write(&colony, &buildings, &needs, &research).err(),
            Some(Refusal::LevelDisagrees {
                saved: 5,
                town_hall: 3
            })
        );
    }

    #[test]
    fn a_minimal_java_save_reads_with_every_default_java_would_take() {
        // An id, a dimension, an empty roster and a town hall: everything else is
        // absent, which is what a save from an old mod version looks like.
        let hall = building::Building::new(Kind::TownHall, HALL).to_compound();
        let data = Compound::from([
            (format::KEY_ID.into(), Tag::Int(3)),
            (
                format::KEY_DIMENSION.into(),
                Tag::String("minecraft:overworld".into()),
            ),
            (
                format::KEY_CITIZEN_MANAGER.into(),
                Tag::Compound(Compound::from([(
                    format::KEY_CITIZENS.into(),
                    Tag::List(Vec::new()),
                )])),
            ),
            (
                format::KEY_BUILDING_MANAGER.into(),
                Tag::Compound(Compound::from([(
                    format::KEY_BUILDINGS.into(),
                    Tag::List(vec![Tag::Compound(hall)]),
                )])),
            ),
        ]);
        let read = read(&data).expect("a minimal save is a colony");
        assert_eq!(read.colony.id, 3);
        assert_eq!(read.colony.name, "");
        assert_eq!(read.colony.level(), 1, "an unbuilt town hall is a level-1 colony");
        assert!(read.colony.citizens.is_empty());
        assert!(read.colony.orders.is_empty());
        assert!(read.research.is_empty());
        assert_eq!(read.colony.members.rank_of(OWNER), Rank::Neutral);
        assert_eq!(read.repairs, vec![Repair::CenterDefaulted, Repair::NoOwner]);
    }

    #[test]
    fn every_refusal_has_its_own_name() {
        let all = [
            Refusal::MissingId,
            Refusal::IdNotPositive { id: 0 },
            Refusal::MissingDimension,
            Refusal::MissingCitizens,
            Refusal::MissingBuildings,
            Refusal::TownHallCount { found: 2 },
            Refusal::UnknownVersion { found: 2, known: 1 },
            Refusal::CitizenWithoutId { index: 0 },
            Refusal::DuplicateCitizen { id: 1 },
            Refusal::DuplicateBuilding { at: HALL },
            Refusal::IdentityDiverges { at: HALL },
            Refusal::RosterDiverges { id: 1 },
            Refusal::MemberWithoutId { index: 0 },
            Refusal::MemberWithoutRank {
                id: String::from("0-0-0-0-0"),
            },
            Refusal::UnknownRank { found: 9 },
            Refusal::UnknownPermissionsVersion { found: 6, known: 5 },
            Refusal::LevelDisagrees {
                saved: 4,
                town_hall: 3,
            },
        ];
        for (index, refusal) in all.iter().enumerate() {
            assert!(refusal.name().starts_with("persistence.refusal."));
            assert_eq!(refusal.to_string(), refusal.name());
            for other in &all[index + 1..] {
                assert_ne!(refusal.name(), other.name(), "two refusals share a name");
            }
        }
        assert_eq!(all.len(), 17, "one refusal per rule this module states");
    }

    #[test]
    fn every_repair_has_its_own_name() {
        let all = [
            Repair::CenterDefaulted,
            Repair::CitizenUnnamed { id: 1 },
            Repair::CitizenDropped { index: 0 },
            Repair::SaturationMissing { id: 1 },
            Repair::SaturationOutOfRange { id: 1 },
            Repair::BuildingDropped {
                kind: String::from("minecolonies:castle"),
            },
            Repair::WorkerUnassigned {
                at: HALL,
                worker: 42,
            },
            Repair::OrdersFromOldSave { count: 2 },
            Repair::OrderDropped { index: 0 },
            Repair::OrderWithoutKind { id: 1 },
            Repair::OrderDuplicated { id: 1 },
            Repair::ClaimReleased { id: 1 },
            Repair::ResearchDropped { count: 1 },
            Repair::MemberDuplicated {
                id: String::from("0-0-0-0-0"),
            },
            Repair::MemberDropped {
                id: String::from("0-0-0-0-0"),
            },
            Repair::OwnerRestored,
            Repair::NoOwner,
            Repair::RanksDiffer { rank: Rank::Owner },
        ];
        for (index, repair) in all.iter().enumerate() {
            assert!(repair.name().starts_with("persistence.repair."));
            assert_eq!(repair.to_string(), repair.name());
            for other in &all[index + 1..] {
                assert_ne!(repair.name(), other.name(), "two repairs share a name");
            }
        }
        assert_eq!(all.len(), 18, "one repair per answer this module gives");
    }

    #[test]
    fn the_format_table_documents_fewer_paths_than_a_save_contains() {
        // A guard against a table that quietly documents nothing: the walk of a
        // full save must find more paths than the table has rows, because a row
        // may cover a whole record with `*`.
        let seen = paths(&written());
        assert!(
            seen.len() > KEYS.len(),
            "{} paths for {} rows",
            seen.len(),
            KEYS.len()
        );
        for path in [
            "citizenManager.citizens[].saturation",
            "permissionMap[].flags[]",
            "buildingManager.buildings[].building_modules.*.level",
            "work.workOrders[].claimedByBuilding.x",
            "research.researchTree[].depth",
        ] {
            assert!(seen.contains(path), "the walk never reaches `{path}`");
        }
    }
}
