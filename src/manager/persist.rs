//! What the manager saves, and what it reads back.
//!
//! Java reference: `core/colony/IColonyManagerCapability.Storage` (`:177-260`,
//! the `colonies` list and the manager part), `core/colony/ColonyManager.java:586-610`
//! (`write`/`read` of the manager's own part), `api/util/constant/NbtTagConstants.java:41-42`
//! (`colonies`, `colonymanager`), `:30` (`abandoned`), `:225` (`autoDelete`),
//! `ColonyManagerConstants.java:10-12` (`dist`), `core/colony/Colony.java:993`
//! (where the away timer is written into the colony's own compound),
//! `api/colony/IColonyTagCapability.java:331-360` (the claims layout) and
//! `core/colony/Colony.java:725-745` (`loadColony`, which gives up on one colony
//! rather than on the save).
//!
//! **The port writes a compound, not a file.** In Java the manager owns
//! `minecolonies/colonies.dat` and one `colony<id>.dat` per colony
//! (`ColonyManagerConstants.java:25-37`). A wasm mod has no `std::fs` and the host
//! exposes no world-save handle, so the port produces the same *shape* and leaves
//! the placement to whoever holds a compound: today a block entity
//! (`crate::wiring`, *Where a colony lives*), tomorrow a real store if the SDK grows
//! one. What is not negotiable is the layout below, because that is what a save
//! written by Java would read back as.
//!
//! ```text
//! { colonies: [ { …colony compound…, abandoned: int, autoDelete: byte }, … ],
//!   colonymanager: { dist: byte },          // ColonyManager.write, :593
//!   chunkClaims: [ { dimension: string, chunk: {x,z}, id: int,
//!                    colonies: [{id}], buildingsClaim: [{id, buildings: [{building:{x,y,z}}]}] } ] }
//! ```
//!
//! **Two of those keys are the port's own, and they are named as such.**
//! `dimension` and the `chunkClaims` list have no Java counterpart: Java hangs the
//! claims on the chunk and needs neither. Everything inside an entry is Java's own
//! layout (`IColonyTagCapability.Storage.writeNBT`, `:331-360`), down to the `id`
//! of the owner and the `building` position nested in a list.
//!
//! **`dist` is a dead tag, written for the format and never read.** `ColonyManager.write`
//! puts a boolean under `dist` (`:593`) and no code in the tree reads it back; the
//! port writes it too, says so here, and does not pretend to understand it.
//!
//! **Tolerant reads, named refusals.** A save that has no `colonies` key is refused
//! (Java falls back to a manager backup, `IColonyManagerCapability.java:279-284`,
//! which the port has no file for). A single unreadable colony is **skipped and
//! reported**, never fatal, which is `loadColony` returning `null` (`:740-744`) and
//! `Storage.readNBT` carrying on with the rest (`:293-299`).

use std::collections::BTreeMap;

use mod_api::{Compound, Tag};

use crate::colony::Colony;

use super::claims::{Chunk, ChunkClaims};
use super::index::Manager;
use super::NO_COLONY_ID;

/// The colony list: `TAG_COLONIES` (`NbtTagConstants.java:41`), used by
/// `IColonyManagerCapability.Storage.writeNBT` (`:186`).
pub const KEY_COLONIES: &str = "colonies";
/// The manager's own part: `TAG_COLONY_MANAGER` (`:42`), written only for the
/// overworld (`IColonyManagerCapability.java:188-193`).
pub const KEY_MANAGER: &str = "colonymanager";
/// The dead distance marker (`ColonyManagerConstants.java:12`, written at
/// `ColonyManager.java:593`).
pub const KEY_DIST: &str = "dist";
/// The away timer, inside the colony's own compound (`NbtTagConstants.java:30`,
/// `Colony.java:993` and `:828`).
pub const KEY_ABANDONED: &str = "abandoned";
/// The auto-delete flag (`NbtTagConstants.java:225`, `Colony.java:996`).
pub const KEY_AUTO_DELETE: &str = "autoDelete";

/// **Port key.** The claims, which Java keeps on the chunk.
pub const KEY_CLAIMS: &str = "chunkClaims";
/// **Port key.** Which level a claim entry belongs to; Java's chunk knows its own.
pub const KEY_DIMENSION: &str = "dimension";
/// **Port key.** The chunk of a claim entry, under `x` and `z`.
pub const KEY_CHUNK: &str = "chunk";
/// The owning id of an entry, and the id of one static claimant
/// (`IColonyTagCapability.java:335-350`).
pub const KEY_OWNER: &str = "id";
/// The static claimants of a chunk (`IColonyTagCapability.java:336`).
pub const KEY_STATIC: &str = "colonies";
/// The building claims of a chunk (`:337`).
pub const KEY_BUILDING_CLAIMS: &str = "buildingsClaim";
/// The positions of one colony's buildings (`:343-350`).
pub const KEY_BUILDINGS: &str = "buildings";
/// One position inside that list (`:345`, `BlockPosUtil.write(compound, TAG_BUILDING, pos)`).
pub const KEY_BUILDING: &str = "building";

/// Why a manager compound could not be read at all. Three refusals, which are the
/// three moments Java gives up on the save instead of on one colony
/// (`IColonyManagerCapability.java:271-292`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum LoadRefusal {
    /// No `colonies` key: Java loads a manager backup here (`:279-284`).
    NoColoniesKey,
    /// `colonies` is there, of another kind. Java's `getList` answers an empty
    /// list, which would silently delete every colony; the port refuses.
    ColoniesNotList,
    /// The overworld's part is missing the manager compound (`:286-290`).
    NoManagerPart,
}

impl LoadRefusal {
    pub fn name(&self) -> &'static str {
        match self {
            Self::NoColoniesKey => "manager.load.no_colonies_key",
            Self::ColoniesNotList => "manager.load.colonies_not_list",
            Self::NoManagerPart => "manager.load.no_manager_part",
        }
    }
}

impl std::fmt::Display for LoadRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for LoadRefusal {}

/// One colony that was passed over, with the reason. Java logs and carries on
/// (`IColonyManagerCapability.java:293-299`, `Colony.java:740-744`); the port
/// answers the list so a caller can log it or refuse the save, its choice.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Skipped {
    pub index: usize,
    pub reason: Skip,
}

/// Why one entry did not become a colony or a claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two entries a save holds
pub enum Skip {
    /// A colony entry that is not a compound, or carries no id: `loadColony` would
    /// build a colony of id `0`, which answers for every unclaimed position.
    Colony,
    /// A second colony with an id already taken (`ColonyList.java:85-95`).
    Clash { id: i32 },
    /// A claim entry with no dimension or no chunk.
    Claim,
}

/// Two colonies saved at the same centre. Java reports it and keeps both
/// (`IColonyManagerCapability.java:302-330`); the port does the same, and hands the
/// report back so the operator can see what the log would have said.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Duplicate {
    pub at: [i32; 3],
    pub ids: Vec<i32>,
}

/// A read manager, plus what the read had to pass over.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Loaded {
    pub manager: Manager,
    pub duplicates: Vec<Duplicate>,
    pub skipped: Vec<Skipped>,
}

/// The manager as a compound. Colony order is id ascending and claim order is
/// `(dimension, x, z)`, so the same world always writes the same bytes.
pub fn write(manager: &Manager) -> Compound {
    let mut colonies = Vec::new();
    for colony in manager.colonies() {
        if colony.id <= NO_COLONY_ID {
            continue;
        }
        let mut record = colony.to_compound();
        // Written into the colony's own compound, as Java does (`Colony.java:993`),
        // so a Java save and a port save hold the same key in the same place.
        record.insert(
            KEY_ABANDONED.into(),
            Tag::Int(manager.hours(colony.id).unwrap_or(0)),
        );
        record.insert(
            KEY_AUTO_DELETE.into(),
            Tag::Byte(i8::from(
                manager.can_be_auto_deleted(colony.id).unwrap_or(true),
            )),
        );
        colonies.push(record);
    }
    let claims: Vec<Compound> = manager
        .claims()
        .chunks()
        .map(|(dimension, chunk, claims)| claim_entry(dimension, chunk, claims))
        .collect();
    Compound::from([
        (KEY_COLONIES.into(), compound_list(colonies)),
        (
            KEY_MANAGER.into(),
            Tag::Compound(Compound::from([(KEY_DIST.into(), Tag::Byte(1))])),
        ),
        (KEY_CLAIMS.into(), compound_list(claims)),
    ])
}

/// Reads a manager back. `overworld` says whether this compound is the one that
/// carries the manager's own part — Java's `Storage.writeNBT(…, overworld)`
/// parameter (`:184`) — because every other level holds only its colonies.
pub fn read(data: &Compound, overworld: bool) -> Result<Loaded, LoadRefusal> {
    if overworld && !data.contains_key(&KEY_MANAGER.into()) {
        return Err(LoadRefusal::NoManagerPart);
    }
    let Some(tag) = data.get(&KEY_COLONIES.into()) else {
        return Err(LoadRefusal::NoColoniesKey);
    };
    let Tag::List(items) = tag else {
        return Err(LoadRefusal::ColoniesNotList);
    };

    let mut manager = Manager::new();
    let mut skipped = Vec::new();
    let mut by_centre: BTreeMap<[i32; 3], Vec<i32>> = BTreeMap::new();
    for (index, item) in items.iter().enumerate() {
        let Tag::Compound(record) = item else {
            skipped.push(Skipped {
                index,
                reason: Skip::Colony,
            });
            continue;
        };
        let colony = Colony::from_compound(record);
        if colony.id <= NO_COLONY_ID {
            skipped.push(Skipped {
                index,
                reason: Skip::Colony,
            });
            continue;
        }
        let (id, centre) = (colony.id, colony.center);
        let hours = int(record, KEY_ABANDONED).unwrap_or(0);
        let allowed = flag(record, KEY_AUTO_DELETE).unwrap_or(true);
        if manager.insert(colony).is_err() {
            skipped.push(Skipped {
                index,
                reason: Skip::Clash { id },
            });
            continue;
        }
        manager.set_hours(id, hours);
        manager.set_auto_deletable(id, allowed);
        by_centre.entry(centre).or_default().push(id);
    }

    if let Some(Tag::List(entries)) = data.get(&KEY_CLAIMS.into()) {
        for (index, item) in entries.iter().enumerate() {
            let Tag::Compound(entry) = item else {
                skipped.push(Skipped {
                    index,
                    reason: Skip::Claim,
                });
                continue;
            };
            let (Some(dimension), Some(chunk)) = (
                text(entry, KEY_DIMENSION),
                entry.get(&KEY_CHUNK.into()).and_then(chunk_of),
            ) else {
                skipped.push(Skipped {
                    index,
                    reason: Skip::Claim,
                });
                continue;
            };
            manager.insert_claims(&dimension, chunk, claims_from(entry));
        }
    }

    let duplicates: Vec<Duplicate> = by_centre
        .into_iter()
        .filter(|(_, ids)| ids.len() > 1)
        .map(|(at, ids)| Duplicate { at, ids })
        .collect();
    Ok(Loaded {
        manager,
        duplicates,
        skipped,
    })
}

/// One chunk's claims, in Java's layout (`IColonyTagCapability.Storage.writeNBT`,
/// `:331-360`) plus the two port keys that say which chunk of which level it is.
fn claim_entry(dimension: &str, chunk: Chunk, claims: &ChunkClaims) -> Compound {
    let mut entry = Compound::from([
        (KEY_DIMENSION.into(), Tag::String(dimension.into())),
        (
            KEY_CHUNK.into(),
            Tag::compound([("x", Tag::Int(chunk[0])), ("z", Tag::Int(chunk[1]))]),
        ),
        (
            KEY_OWNER.into(),
            Tag::Int(claims.owner().unwrap_or(NO_COLONY_ID)),
        ),
    ]);
    let statics: Vec<Compound> = claims
        .static_claims()
        .map(|id| Compound::from([(KEY_OWNER.into(), Tag::Int(id))]))
        .collect();
    entry.insert(KEY_STATIC.into(), compound_list(statics));
    let buildings: Vec<Compound> = claims
        .claiming_buildings()
        .iter()
        .map(|(id, positions)| {
            let mut held = Compound::from([(KEY_OWNER.into(), Tag::Int(*id))]);
            let positions: Vec<Compound> = positions
                .iter()
                .map(|at| Compound::from([(KEY_BUILDING.into(), position(*at))]))
                .collect();
            held.insert(KEY_BUILDINGS.into(), compound_list(positions));
            held
        })
        .collect();
    entry.insert(KEY_BUILDING_CLAIMS.into(), compound_list(buildings));
    entry
}

/// One claim entry back into a [`ChunkClaims`], following `readFromNBT`
/// (`:312-327`), including the repair of `:320-325`: a chunk with no owner but a
/// static claim takes the first of them.
fn claims_from(entry: &Compound) -> ChunkClaims {
    let mut claims = ChunkClaims::new();
    if let Some(owner) = int(entry, KEY_OWNER) {
        claims.set_owning((owner != NO_COLONY_ID).then_some(owner));
    }
    let statics: Vec<i32> = compounds(entry, KEY_STATIC)
        .into_iter()
        .filter_map(|held| int(held, KEY_OWNER))
        .collect();
    claims.set_static_claims(statics);
    for colony in compounds(entry, KEY_BUILDING_CLAIMS) {
        let Some(id) = int(colony, KEY_OWNER) else {
            continue;
        };
        for building in compounds(colony, KEY_BUILDINGS) {
            if let Some(at) = position_in(building, KEY_BUILDING) {
                claims.restore_building_claim(id, at);
            }
        }
    }
    // The repair of :320-325, read out before the write: an ownerless chunk with a
    // static claim takes the first of them.
    let owner = claims.owner().or_else(|| claims.static_claims().next());
    claims.set_owning(owner);
    claims
}

/// A position, as `BlockPosUtil` lays one out (`api/util/BlockPosUtil.java:190-226`).
fn position(at: [i32; 3]) -> Tag {
    Tag::compound([
        ("x", Tag::Int(at[0])),
        ("y", Tag::Int(at[1])),
        ("z", Tag::Int(at[2])),
    ])
}

fn position_in(data: &Compound, key: &str) -> Option<[i32; 3]> {
    let tag = data.get(&key.into())?;
    Some([
        number(tag.get("x")?)?,
        number(tag.get("y")?)?,
        number(tag.get("z")?)?,
    ])
}

fn chunk_of(tag: &Tag) -> Option<Chunk> {
    Some([number(tag.get("x")?)?, number(tag.get("z")?)?])
}

/// An integer under `key`, in whichever width it was saved. `crate::colony::nbt`
/// holds the same four readers and is private to that module (`colony/mod.rs:28`),
/// so the manager keeps its own copy rather than reaching into it.
fn int(data: &Compound, key: &str) -> Option<i32> {
    number(data.get(&key.into())?)
}

fn text(data: &Compound, key: &str) -> Option<String> {
    match data.get(&key.into())? {
        Tag::String(value) => Some(value.to_string_lossy()),
        _ => None,
    }
}

/// A boolean NBT stores as a byte (`TAG_AUTO_DELETE`, `Colony.java:996`).
fn flag(data: &Compound, key: &str) -> Option<bool> {
    match data.get(&key.into())? {
        Tag::Byte(value) => Some(*value != 0),
        _ => None,
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

fn compounds<'a>(data: &'a Compound, key: &str) -> Vec<&'a Compound> {
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

fn compound_list(items: Vec<Compound>) -> Tag {
    Tag::List(items.into_iter().map(Tag::Compound).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colony::colonie::KEY_ID;
    use crate::colony::Citizen;
    use crate::colony::Building;

    const OVERWORLD: &str = "minecraft:overworld";
    /// A hut outside the founding box: the dynamic claim of `claimBuildingChunks`
    /// (`ChunkDataHelper.java:152-162`), which is what puts a tenth chunk in the book.
    const HUT: [i32; 3] = [200, 64, 200];

    fn manager() -> Manager {
        let mut manager = Manager::new();
        let mut colony = Colony::new(7, "Bridgewater", OVERWORLD, [0, 70, 0], 1);
        colony.citizens.push(Citizen::new(1, "Ada"));
        colony.buildings.push(Building::new("minecolonies:blockhutbuilder", HUT, 2));
        manager.insert(colony).expect("a free id");
        manager
            .claim_ground(OVERWORLD, [0, 70, 0], 1, 7)
            .expect("a known colony");
        manager.claim_building(OVERWORLD, HUT, 7).expect("a known colony");
        manager
    }

    #[test]
    fn a_colony_and_its_claims_round_trip() {
        let written = write(&manager());
        let loaded = read(&written, true).expect("the port's own compound");
        assert_eq!(loaded.manager, manager());
        assert!(loaded.duplicates.is_empty());
        assert!(loaded.skipped.is_empty());
        assert_eq!(
            loaded.manager.record(7).map(|colony| colony.name.as_str()),
            Ok("Bridgewater")
        );
        assert_eq!(loaded.manager.owner_at(OVERWORLD, [0, 70, 0]), Some(7));
        assert_eq!(loaded.manager.claims().len(), 10, "9 chunks of the box plus the hut's");
    }

    #[test]
    fn the_saved_compound_carries_java_keys_and_the_colony_keeps_its_own() {
        let written = write(&manager());
        assert!(matches!(written.get(&KEY_COLONIES.into()), Some(Tag::List(_))));
        let Some(Tag::Compound(part)) = written.get(&KEY_MANAGER.into()) else {
            panic!("the manager part is a compound, ColonyManager.java:589-597");
        };
        assert_eq!(part.get(&KEY_DIST.into()), Some(&Tag::Byte(1)));
        let colonies = compounds(&written, KEY_COLONIES);
        assert_eq!(int(colonies[0], KEY_ID), Some(7), "the record's own id");
        assert_eq!(int(colonies[0], KEY_ABANDONED), Some(0), "the away timer, Colony.java:993");
        assert_eq!(flag(colonies[0], KEY_AUTO_DELETE), Some(true));
        let claims = compounds(&written, KEY_CLAIMS);
        assert_eq!(text(claims[0], KEY_DIMENSION).as_deref(), Some(OVERWORLD));
        assert_eq!(int(claims[0], KEY_OWNER), Some(7));
    }

    #[test]
    fn a_compound_without_a_colony_list_is_refused_by_name_and_one_of_another_kind_too() {
        assert_eq!(read(&Compound::new(), false), Err(LoadRefusal::NoColoniesKey));
        let wrong = Compound::from([(KEY_COLONIES.into(), Tag::Int(3))]);
        assert_eq!(read(&wrong, false), Err(LoadRefusal::ColoniesNotList));
        assert_eq!(LoadRefusal::NoColoniesKey.to_string(), "manager.load.no_colonies_key");
    }

    #[test]
    fn the_overworld_needs_the_manager_part_and_another_level_does_not() {
        let level = Compound::from([(KEY_COLONIES.into(), compound_list(Vec::new()))]);
        assert_eq!(read(&level, true), Err(LoadRefusal::NoManagerPart));
        let loaded = read(&level, false).expect("a nether part holds colonies only");
        assert!(loaded.manager.is_empty());
    }

    #[test]
    fn a_colony_without_an_id_is_skipped_and_the_others_are_kept() {
        let mut anonymous = Compound::new();
        anonymous.insert(KEY_ID.into(), Tag::Int(0));
        let saved = Compound::from([
            (KEY_COLONIES.into(), compound_list(vec![anonymous])),
            (KEY_MANAGER.into(), Tag::Compound(Compound::new())),
        ]);
        let loaded = read(&saved, true).expect("the shape is right");
        assert!(loaded.manager.is_empty(), "an id of 0 is not a colony");
        assert_eq!(
            loaded.skipped,
            vec![Skipped {
                index: 0,
                reason: Skip::Colony
            }]
        );
    }

    #[test]
    fn two_colonies_at_one_centre_are_reported_and_both_kept() {
        let first = Colony::new(1, "One", OVERWORLD, [0, 70, 0], 1);
        let second = Colony::new(2, "Two", OVERWORLD, [0, 70, 0], 2);
        let saved = Compound::from([
            (
                KEY_COLONIES.into(),
                compound_list(vec![first.to_compound(), second.to_compound()]),
            ),
            (KEY_MANAGER.into(), Tag::Compound(Compound::new())),
        ]);
        let loaded = read(&saved, true).expect("the shape is right");
        assert_eq!(loaded.manager.len(), 2, "Java keeps both, IColonyManagerCapability.java:302-330");
        assert_eq!(
            loaded.duplicates,
            vec![Duplicate {
                at: [0, 70, 0],
                ids: vec![1, 2]
            }]
        );
    }

    #[test]
    fn a_second_colony_of_one_id_is_skipped_with_the_id_it_clashed_on() {
        let first = Colony::new(1, "One", OVERWORLD, [0, 70, 0], 1);
        let second = Colony::new(1, "Two", OVERWORLD, [400, 70, 0], 2);
        let saved = Compound::from([
            (
                KEY_COLONIES.into(),
                compound_list(vec![first.to_compound(), second.to_compound()]),
            ),
            (KEY_MANAGER.into(), Tag::Compound(Compound::new())),
        ]);
        let loaded = read(&saved, true).expect("the shape is right");
        assert_eq!(loaded.manager.len(), 1);
        assert_eq!(
            loaded.skipped,
            vec![Skipped {
                index: 1,
                reason: Skip::Clash { id: 1 }
            }],
            "ColonyList.add refuses the clash, ColonyList.java:85-95"
        );
        assert_eq!(loaded.manager.record(1).map(|colony| colony.name.as_str()), Ok("One"));
    }

    #[test]
    fn the_away_timer_and_the_auto_delete_flag_are_saved_with_their_colony() {
        let mut manager = manager();
        manager.set_hours(7, 12);
        manager.set_auto_deletable(7, false);
        let loaded = read(&write(&manager), true).expect("the port's own compound");
        assert_eq!(loaded.manager.hours(7), Some(12));
        assert_eq!(loaded.manager.can_be_auto_deleted(7), Some(false));
        assert_eq!(loaded.manager.abandoned_since(12), vec![7]);
    }

    #[test]
    fn a_claim_entry_with_no_dimension_or_no_chunk_is_skipped() {
        let saved = Compound::from([
            (KEY_COLONIES.into(), compound_list(Vec::new())),
            (KEY_MANAGER.into(), Tag::Compound(Compound::new())),
            (
                KEY_CLAIMS.into(),
                compound_list(vec![
                    Compound::from([(KEY_OWNER.into(), Tag::Int(7))]),
                    Compound::from([
                        (KEY_DIMENSION.into(), Tag::String(OVERWORLD.into())),
                        (KEY_OWNER.into(), Tag::Int(7)),
                    ]),
                ]),
            ),
        ]);
        let loaded = read(&saved, true).expect("the colonies part is fine");
        assert!(loaded.manager.claims().is_empty());
        assert_eq!(
            loaded.skipped.iter().map(|skipped| skipped.index).collect::<Vec<usize>>(),
            vec![0, 1]
        );
    }

    #[test]
    fn a_chunk_whose_colony_is_not_in_the_save_keeps_its_owner_for_a_stale_refusal() {
        let saved = Compound::from([
            (KEY_COLONIES.into(), compound_list(Vec::new())),
            (KEY_MANAGER.into(), Tag::Compound(Compound::new())),
            (
                KEY_CLAIMS.into(),
                compound_list(vec![Compound::from([
                    (KEY_DIMENSION.into(), Tag::String(OVERWORLD.into())),
                    (
                        KEY_CHUNK.into(),
                        Tag::compound([("x", Tag::Int(0)), ("z", Tag::Int(0))]),
                    ),
                    (KEY_OWNER.into(), Tag::Int(42)),
                ])]),
            ),
        ]);
        let loaded = read(&saved, true).expect("a save with no colony is still a save");
        assert_eq!(loaded.manager.owner_at(OVERWORLD, [0, 70, 0]), Some(42));
        assert_eq!(
            loaded.manager.record_at(OVERWORLD, [0, 70, 0]).map(|colony| colony.id),
            Err(crate::manager::LookupRefusal::StaleOwner {
                dimension: OVERWORLD.to_owned(),
                chunk: [0, 0],
                id: 42
            }),
            "the read keeps the contradiction instead of erasing it"
        );
    }
}
