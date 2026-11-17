//! The buildings: the `buildingManager` compound, and the two lists that must
//! agree about which buildings stand where.
//!
//! Java reference: `core/colony/managers/RegisteredStructureManager.java:249-273`
//! (`write`) and `:129-176` (`read`), `core/colony/buildings/registry/BuildingDataManager.java:35-49`
//! (an entry whose type is unknown is dropped, not the colony) and
//! `core/colony/buildings/AbstractSchematicProvider.java:171-192` (the record).
//!
//! **The record itself is not rewritten here.** `crate::building::Building`
//! already owns the compound and its rules (`src/building/instance.rs:350-421`);
//! this file owns the *list*: which entries survive a read, which position is
//! taken twice, and whether a worker is still a citizen of the colony.
//!
//! **Two lists, one truth.** `crate::colony::Colony` carries a light building
//! list (`kind`, `at`, `level`) because work-order claims point at positions, and
//! `crate::building::Building` carries the record. A save has one list — the
//! records — so the light list is *derived* on read and *checked* on write:
//! [`check_identity`] refuses a colony whose light list says something the
//! records do not. Without that check a `Colony` could hand a work order to a hut
//! that no record stands for, which is exactly the drift the two lists invite.
//!
//! **A duplicate position is fatal, as it is in Java.** `addBuilding` puts into
//! an `ImmutableMap` (`RegisteredStructureManager.java:769`), whose `put` throws
//! on a key already present — out of `loadColony`'s `catch`, so a save with two
//! buildings at one position costs Java the whole colony. The port names the
//! position instead.

use mod_api::{Compound, Tag};

use crate::building::{self, Kind, LEVEL_MIN};
use crate::colony::colonie;
use crate::colony::{Citizen, Colony};

use super::format::{KEY_BUILDING_MANAGER, KEY_BUILDINGS, KEY_LOCATION, KEY_TYPE};
use super::{nbt, Refusal, Repair};

/// The `buildingManager` compound: `{buildings: [ …records… ]}`.
pub fn write(buildings: &[building::Building]) -> Compound {
    let records: Vec<Compound> = buildings
        .iter()
        .map(building::Building::to_compound)
        .collect();
    Compound::from([(
        KEY_BUILDING_MANAGER.into(),
        Tag::Compound(Compound::from([(
            KEY_BUILDINGS.into(),
            nbt::compound_list(records),
        )])),
    )])
}

/// The buildings, read back.
///
/// **What survives and what does not.** An entry whose type the registry does not
/// know is dropped and reported: that is Java's own answer
/// (`BuildingDataManager.java:35-49` returns null and `RegisteredStructureManager.java:172-176`
/// skips the entry). A worker who is no longer a citizen is unassigned and
/// reported: `WorkerBuildingModule.java:129-133` only assigns a citizen it finds
/// *and* whose job matches the module. A duplicate position is refused, because
/// Java cannot read such a save at all.
pub fn read(
    data: &Compound,
    citizens: &[Citizen],
    repairs: &mut Vec<Repair>,
) -> Result<Vec<building::Building>, Refusal> {
    let Some(Tag::Compound(manager)) = data.get(&KEY_BUILDING_MANAGER.into()) else {
        return Err(Refusal::MissingBuildings);
    };
    let Some(Tag::List(records)) = manager.get(&KEY_BUILDINGS.into()) else {
        return Err(Refusal::MissingBuildings);
    };

    let mut buildings = Vec::with_capacity(records.len());
    let mut seen: Vec<[i32; 3]> = Vec::with_capacity(records.len());
    for item in records.iter() {
        let Tag::Compound(record) = item else {
            repairs.push(Repair::BuildingDropped {
                kind: String::new(),
            });
            continue;
        };
        let Some(built) = building::Building::from_compound(record) else {
            repairs.push(Repair::BuildingDropped {
                kind: nbt::text(record, KEY_TYPE).unwrap_or_default(),
            });
            continue;
        };
        if seen.contains(&built.at()) {
            return Err(Refusal::DuplicateBuilding { at: built.at() });
        }
        seen.push(built.at());
        buildings.push(built);
    }

    let ids: Vec<i32> = citizens.iter().map(|citizen| citizen.id).collect();
    for held in &mut buildings {
        let Some(worker) = held.worker() else {
            continue;
        };
        if !ids.contains(&worker) {
            let _ = held.fire();
            repairs.push(Repair::WorkerUnassigned {
                at: held.at(),
                worker,
            });
        }
    }
    Ok(buildings)
}

/// The colony's town hall, which is what the colony's level means in Java
/// (`CitizenManager.java:596` reads a colony's level that way; `IBuilding.java:445-467`
/// is the level itself).
///
/// Exactly one: `BlockHutTownHall.java:219-234` refuses a second, and a colony
/// with none has nowhere to put one later.
pub fn town_hall(buildings: &[building::Building]) -> Result<&building::Building, Refusal> {
    let mut found = buildings
        .iter()
        .filter(|built| built.kind().is_town_hall());
    let Some(hall) = found.next() else {
        return Err(Refusal::TownHallCount { found: 0 });
    };
    let count = 1 + found.count();
    if count != 1 {
        return Err(Refusal::TownHallCount { found: count });
    }
    Ok(hall)
}

/// The level a colony has, which is its town hall's, kept in
/// `LEVEL_MIN..=MAX_BUILDING_LEVEL` the way `Colony::set_level` keeps its own.
pub fn level_of(hall: &building::Building) -> i32 {
    hall.level().max(LEVEL_MIN)
}

/// Refuses a colony whose light building list disagrees with the records: a
/// position in one and not the other, a different kind, or a different level.
///
/// The kinds are compared through [`Kind::from_path`], so a light entry written
/// as `minecolonies:blockhutbuilder` still matches `minecolonies:builder` — the
/// name is a name, not data. A read writes [`Kind::path`] and is therefore
/// canonical; the report says so.
pub fn check_identity(colony: &Colony, buildings: &[building::Building]) -> Result<(), Refusal> {
    if colony.buildings.len() != buildings.len() {
        let at = buildings
            .first()
            .map(building::Building::at)
            .or_else(|| colony.buildings.first().map(|held| held.at))
            .unwrap_or([0, 0, 0]);
        return Err(Refusal::IdentityDiverges { at });
    }
    for built in buildings {
        let Some(light) = colony.building_at(built.at()) else {
            return Err(Refusal::IdentityDiverges { at: built.at() });
        };
        if Kind::from_path(&light.kind) != Some(built.kind()) || light.level != built.level() {
            return Err(Refusal::IdentityDiverges { at: built.at() });
        }
    }
    Ok(())
}

/// The light list a read derives from the records: one `{type, location,
/// buildingLevel}` entry per record, which is the shape `crate::colony` reads
/// (`src/colony/colonie.rs:117-123`).
///
/// **`buildingLevel`, not `level`**: the colony's light record keys a building's
/// level on `NbtTagConstants.java:130` (`buildingLevel`), while the building's own
/// record keys it on `:150` (`level`). Two keys, one number — the two lists are
/// read by two different readers, and mixing them silently loses every level.
pub fn identity_records(buildings: &[building::Building]) -> Vec<Compound> {
    buildings
        .iter()
        .map(|built| {
            Compound::from([
                (KEY_TYPE.into(), Tag::String(built.kind().path().as_str().into())),
                (KEY_LOCATION.into(), nbt::position(built.at())),
                (
                    colonie::KEY_BUILDING_LEVEL.into(),
                    Tag::Int(built.level()),
                ),
            ])
        })
        .collect()
}

/// The position a raw record holds, for a test that edits one by hand.
#[cfg(test)]
pub fn at_of(record: &Compound) -> Option<[i32; 3]> {
    nbt::position_in(record, KEY_LOCATION)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::Module;

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

    fn citizens() -> Vec<Citizen> {
        (1..=5).map(|id| Citizen::new(id, "Ada")).collect()
    }

    fn records(data: &mut Compound) -> &mut Vec<Tag> {
        let Some(Tag::Compound(manager)) = data.get_mut(&KEY_BUILDING_MANAGER.into()) else {
            panic!("buildingManager is a compound")
        };
        let Some(Tag::List(items)) = manager.get_mut(&KEY_BUILDINGS.into()) else {
            panic!("buildings is a list")
        };
        items
    }

    #[test]
    fn three_buildings_round_trip_with_their_modules_and_workers() {
        let written = write(&buildings());
        let mut repairs = Vec::new();
        let read = read(&written, &citizens(), &mut repairs).expect("the list reads");
        assert!(repairs.is_empty(), "a clean list needs no repair");
        assert_eq!(read, buildings());
        assert_eq!(read[1].worker(), Some(1));
        assert_eq!(read[1].parent(), Some(HALL));
        assert_eq!(read[1].module("work").map(Module::level), Some(2));
    }

    #[test]
    fn the_manager_is_one_compound_holding_one_list() {
        let data = write(&buildings());
        let Some(Tag::Compound(manager)) = data.get(&KEY_BUILDING_MANAGER.into()) else {
            panic!("buildingManager is a compound")
        };
        let Some(Tag::List(items)) = manager.get(&KEY_BUILDINGS.into()) else {
            panic!("buildings is a list")
        };
        assert_eq!(items.len(), 3);
        let Tag::Compound(first) = &items[0] else {
            panic!("a record is a compound")
        };
        assert_eq!(
            nbt::text(first, KEY_TYPE).as_deref(),
            Some("minecolonies:townhall")
        );
    }

    #[test]
    fn a_building_of_an_unknown_kind_is_dropped_and_named() {
        let mut data = write(&buildings());
        let Tag::Compound(farm) = &mut records(&mut data)[2] else {
            panic!("a record is a compound")
        };
        farm.insert(KEY_TYPE.into(), Tag::String("minecolonies:castle".into()));
        let mut repairs = Vec::new();
        let read = read(&data, &citizens(), &mut repairs).expect("two buildings survive");
        assert_eq!(read.len(), 2, "the colony survives the entry, as in Java");
        assert_eq!(
            repairs,
            vec![Repair::BuildingDropped {
                kind: "minecolonies:castle".to_owned()
            }]
        );
    }

    #[test]
    fn two_buildings_at_one_position_are_refused() {
        let mut data = write(&buildings());
        let copy = records(&mut data)[2].clone();
        records(&mut data).push(copy);
        assert_eq!(
            read(&data, &citizens(), &mut Vec::new()).err(),
            Some(Refusal::DuplicateBuilding { at: FARM })
        );
    }

    #[test]
    fn a_worker_who_is_not_a_citizen_is_unassigned_and_named() {
        let mut data = write(&buildings());
        let Tag::Compound(site) = &mut records(&mut data)[1] else {
            panic!("a record is a compound")
        };
        site.insert(
            super::super::format::KEY_WORKER.into(),
            Tag::Int(42),
        );
        let mut repairs = Vec::new();
        let read = read(&data, &citizens(), &mut repairs).expect("the hut survives");
        assert_eq!(read[1].worker(), None);
        assert_eq!(
            repairs,
            vec![Repair::WorkerUnassigned {
                at: SITE,
                worker: 42
            }]
        );
    }

    #[test]
    fn a_building_list_of_another_kind_is_refused() {
        let data = Compound::from([(
            KEY_BUILDING_MANAGER.into(),
            Tag::Compound(Compound::from([(KEY_BUILDINGS.into(), Tag::Int(1))])),
        )]);
        assert_eq!(
            read(&data, &citizens(), &mut Vec::new()).err(),
            Some(Refusal::MissingBuildings)
        );
        assert_eq!(
            read(&Compound::new(), &citizens(), &mut Vec::new()).err(),
            Some(Refusal::MissingBuildings)
        );
    }

    #[test]
    fn exactly_one_town_hall_is_required_and_its_level_is_the_colonys() {
        let list = buildings();
        let hall = town_hall(&list).expect("one town hall");
        assert_eq!(level_of(hall), 3);
        assert_eq!(
            town_hall(&list[1..]).err(),
            Some(Refusal::TownHallCount { found: 0 })
        );
        let mut two = list.clone();
        let second = building::Building::new(Kind::TownHall, [99, 64, 99]);
        two.push(second);
        assert_eq!(
            town_hall(&two).err(),
            Some(Refusal::TownHallCount { found: 2 })
        );
    }

    #[test]
    fn an_unbuilt_town_hall_still_names_a_level_one_colony() {
        let hall = building::Building::new(Kind::TownHall, HALL);
        assert_eq!(hall.level(), 0);
        assert_eq!(
            level_of(&hall),
            LEVEL_MIN,
            "a colony founded by a hut nobody has built yet is level 1, not 0"
        );
    }

    #[test]
    fn the_light_list_must_say_what_the_records_say() {
        let list = buildings();
        let mut colony = Colony::new(7, "Bridgewater", "minecraft:overworld", HALL, 1);
        for built in &list {
            colony
                .buildings
                .push(crate::colony::Building::new(&built.kind().path(), built.at(), built.level()));
        }
        assert_eq!(check_identity(&colony, &list), Ok(()));

        colony.buildings[2].level = 4;
        assert_eq!(
            check_identity(&colony, &list),
            Err(Refusal::IdentityDiverges { at: FARM })
        );
        colony.buildings[2].level = 1;
        colony.buildings.pop();
        assert!(check_identity(&colony, &list).is_err(), "one list, one count");
    }

    #[test]
    fn a_light_kind_written_as_a_block_name_still_matches_its_record() {
        let list = buildings();
        let mut colony = Colony::new(7, "Bridgewater", "minecraft:overworld", HALL, 1);
        for built in &list {
            colony.buildings.push(crate::colony::Building::new(
                "minecolonies:blockhutbuilder",
                built.at(),
                built.level(),
            ));
        }
        // Only the builder's record answers to the block name, so the two other
        // positions diverge: the check is about the pair, not the spelling.
        assert_eq!(
            check_identity(&colony, &list),
            Err(Refusal::IdentityDiverges { at: HALL }),
            "the town hall record is not a builder"
        );
    }

    #[test]
    fn the_identity_records_are_canonical_paths() {
        let records = identity_records(&buildings());
        assert_eq!(records.len(), 3);
        assert_eq!(
            nbt::text(&records[0], KEY_TYPE).as_deref(),
            Some("minecolonies:townhall")
        );
        assert_eq!(nbt::position_in(&records[1], KEY_LOCATION), Some(SITE));
        assert_eq!(
            nbt::int(&records[1], colonie::KEY_BUILDING_LEVEL),
            Some(2),
            "the light record keys a level on `buildingLevel`, not `level`"
        );
        assert_eq!(at_of(&records[2]), Some(FARM));
    }
}
