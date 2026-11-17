//! Which kind a saved `type` names, and the one alias the registry does not hold.
//!
//! Java reference: `core/colony/buildings/registry/BuildingDataManager.java`
//! (102 lines) and the registry it reads, `api/colony/buildings/registry/IBuildingRegistry.java`
//! filled by `apiimp/initializer/ModBuildingsInitializer.java`.
//!
//! **The manager is four answers and one of them is not a lookup.**
//!
//! | Entry point | Java | Answer here |
//! | --- | --- | --- |
//! | `createFrom(colony, compound)` | `:28-52` | [`from_record`] |
//! | `createFrom(colony, tileEntity)` | `:55-58` | [`from_tile`] |
//! | `createFrom(colony, pos, name)` | `:61-74` | [`kind_for`], the lookup both of the above share |
//! | `createViewFrom(view, pos, buffer)` | `:77-95` | [`view_kind`] — **no alias** |
//!
//! **The record itself is not re-read here.** `crate::building::Building::from_compound`
//! already owns the compound and its tolerance (`src/building/instance.rs:373-421`),
//! and `crate::persistence::workshop::read` owns the list that keeps the entries
//! that survive (`src/persistence/workshop.rs:56-124`). This file answers the one
//! question neither of them asks — which *kind* — and does it **before** handing
//! the compound over, so a save with an unknown type costs its own entry and not
//! the colony (`:35-39`, `RegisteredStructureManager.java:172-176`).
//!
//! **`home` is an alias, and it is matched on the path alone.** Nothing is
//! registered as `home`: `ModBuildings.HOME_ID` is **`"residence"`**
//! (`api/colony/buildings/ModBuildings.java:25`) and the entry is registered under
//! that id (`ModBuildingsInitializer.java:245-253`). So a save that says
//! `minecolonies:home` — or `minecraft:home`, or `anything:home` — misses the
//! registry and lands in the fallback, which produces **the residence**
//! (`BuildingDataManager.java:64-72`). The test below holds all three spellings,
//! because the fallback tests `buildingName.getPath().equals("home")` and never
//! the namespace.
//!
//! **The fallback is on the read path only.** `createViewFrom` has no such branch
//! (`:77-95`): a view whose name is `home` is logged and dropped. That asymmetry is
//! Java's, and a port that "tidied" it would make a client show a residence for a
//! building the server does not have.
//!
//! **One divergence, inherited and named.** `Kind::from_path` accepts a bare
//! `residence` (`src/building/kind.rs:130-139`), where Java's `new ResourceLocation("residence")`
//! becomes `minecraft:residence` and misses the registry. The tolerance is
//! deliberate in `kind.rs` — a hand-written `/data merge` may drop the namespace —
//! and this file keeps it rather than holding two answers about one key.
//!
//! **What is not here**: `openBuildingBrowser` (`:98-101`) opens a client window,
//! and the view half of `createViewFrom` reads the rest of a `FriendlyByteBuf`
//! after the name. Both are in `MANQUANT.md` §2 and §3.

use crate::building::{Building, Kind};
use mod_api::{Compound, Identifier, Tag};

/// The key a record's building kind is stored under:
/// `NbtTagConstants.TAG_BUILDING_TYPE` (`api/util/constant/NbtTagConstants.java:140`),
/// written by `AbstractSchematicProvider.java:171` and read at `:30`.
pub const KEY_TYPE: &str = "type";

/// Why a building could not be produced, named.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the four ways `createFrom` answers null, plus the unreadable record
pub enum Refusal {
    /// The record has no `type` string. Java reads it with `getString`, which
    /// answers `""` — and `new ResourceLocation("")` **throws**, out of the
    /// manager and into the caller (`:30`).
    TypeMissing,
    /// The `type` string is not a resource location (Java throws
    /// `InvalidIdentifierException` at `:30;32`).
    TypeInvalid { found: String },
    /// A well-formed id no building entry holds (`:64-72`), and not the `home`
    /// alias.
    UnknownType { id: String },
    /// The kind resolved and the record still would not read: the case where
    /// Java's `deserializeNBT` threw and it dropped the whole building (`:40-51`).
    Unreadable { id: String },
}

/// The kind a saved `type` names, or `None` — the registry first, the `home`
/// alias second (`BuildingDataManager.java:61-74`).
///
/// The order matters and is Java's: an entry registered under the path a file
/// names always wins, and the alias is only reached by a miss.
pub fn kind_for(id: &Identifier) -> Option<Kind> {
    Kind::from_path(&id.to_string()).or_else(|| {
        // `:66`: `buildingName.getPath().equals("home")` — the namespace is not
        // read, so `minecraft:home` and `mystery:home` land here too.
        if id.path() == "home" {
            Some(Kind::Residence)
        } else {
            None
        }
    })
}

/// One building out of its compound: Java's `createFrom(colony, compound)`
/// (`:28-52`), with the record read by `crate::building::Building`.
///
/// Java's order, kept: the type and the position are **both** read before the
/// lookup (`:30-31`), so a corrupt position does not hide a bad type and a bad
/// type does not cost the position's read. The refusal then stops everything —
/// no building is produced, which is Java's `return null` at `:35-39`.
///
/// The `home` rewrite is the only write this function makes, and it is made on a
/// **copy** of the compound: the caller's save is not touched.
pub fn from_record(data: &Compound) -> Result<Building, Refusal> {
    let id = read_type(data)?;
    let kind = kind_for(&id).ok_or_else(|| Refusal::UnknownType {
        id: id.to_string(),
    })?;

    // `:33` produces the building from `(pos, type)` and `:42` then fills it from
    // the same compound. The `home` alias replaces the *type* and leaves every
    // other key where it was, which is why a residence read through the alias
    // keeps its level, its modules and its worker.
    if Kind::from_path(&id.to_string()).is_none() {
        let mut patched = data.clone();
        patched.insert(
            KEY_TYPE.into(),
            Tag::String(kind.path().as_str().into()),
        );
        return Building::from_compound(&patched).ok_or(Refusal::Unreadable {
            id: id.to_string(),
        });
    }
    Building::from_compound(data).ok_or(Refusal::Unreadable {
        id: id.to_string(),
    })
}

/// A building as the block entity knows it: Java's `createFrom(colony, tile)`
/// (`:55-58`), which is the three-argument overload with the tile's own position
/// and name.
///
/// **Nothing is deserialised on this path**, and that is not an omission: the tile
/// entity calls this when it is placed, and the building it produces is a fresh
/// one at level 0 (`AbstractSchematicProvider.java:64`). The saved compound comes
/// later, through [`from_record`].
pub fn from_tile(id: &Identifier, at: [i32; 3]) -> Result<Building, Refusal> {
    let kind = kind_for(id).ok_or_else(|| Refusal::UnknownType {
        id: id.to_string(),
    })?;
    Ok(Building::new(kind, at))
}

/// The kind a **view** names: Java's `createViewFrom` (`:77-95`), reduced to the
/// lookup that decides whether a view exists at all.
///
/// **No `home` alias here** — `:80-87` has no second chance, and a view named
/// `home` is a miss, logged and dropped.
pub fn view_kind(id: &Identifier) -> Result<Kind, Refusal> {
    Kind::from_path(&id.to_string()).ok_or_else(|| Refusal::UnknownType {
        id: id.to_string(),
    })
}

/// The `type` string of a record, parsed. Java's two failure modes, split:
/// an absent or non-string tag is [`Refusal::TypeMissing`], a string the engine
/// cannot parse is [`Refusal::TypeInvalid`].
fn read_type(data: &Compound) -> Result<Identifier, Refusal> {
    let text = match data.get(&KEY_TYPE.into()) {
        Some(Tag::String(text)) => text.to_string_lossy(),
        _ => return Err(Refusal::TypeMissing),
    };
    if text.is_empty() {
        // `new ResourceLocation("")` throws where the engine's parser accepts an
        // empty path (`horizon-core/src/identifier.rs:34-46` only checks the
        // characters), so the empty case is refused here, not passed on.
        return Err(Refusal::TypeInvalid { found: text });
    }
    Identifier::parse(&text)
        .ok()
        .filter(|id| !id.path().is_empty())
        .ok_or(Refusal::TypeInvalid { found: text })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::Stage;
    use crate::manager_02::nbt::position;

    fn id(text: &str) -> Identifier {
        Identifier::parse_namespaced(text).expect("a literal id parses")
    }

    fn record(kind: &str, at: [i32; 3], level: i32) -> Compound {
        Compound::from([
            ("type".into(), Tag::String(kind.into())),
            ("location".into(), position(at)),
            ("level".into(), Tag::Int(level)),
            ("isBuilt".into(), Tag::Byte(1)),
        ])
    }

    #[test]
    fn a_registered_path_is_its_kind() {
        assert_eq!(kind_for(&id("minecolonies:builder")), Some(Kind::Builder));
        assert_eq!(kind_for(&id("minecolonies:townhall")), Some(Kind::TownHall));
        assert_eq!(kind_for(&id("minecolonies:residence")), Some(Kind::Residence));
    }

    #[test]
    fn home_is_the_residence_whatever_its_namespace() {
        // `ModBuildings.java:25`: `HOME_ID = "residence"`, so nothing is
        // registered as `home` and every spelling of it falls to `:66`.
        assert_eq!(kind_for(&id("minecolonies:home")), Some(Kind::Residence));
        assert_eq!(kind_for(&id("minecraft:home")), Some(Kind::Residence));
        assert_eq!(kind_for(&id("mystery:home")), Some(Kind::Residence));
    }

    #[test]
    fn an_unknown_path_is_no_kind_and_is_not_guessed() {
        assert_eq!(kind_for(&id("minecolonies:unicorn")), None);
        assert_eq!(kind_for(&id("other:builder")), None);
        // `largeQuarry` is declared and never registered (`ModBuildingsInitializer.java:643-648`).
        assert_eq!(kind_for(&id("minecolonies:largequarry")), None);
    }

    #[test]
    fn a_record_reads_into_a_building_at_its_own_position_and_level() {
        let building = from_record(&record("minecolonies:baker", [10, 64, -3], 3)).expect("a baker");
        assert_eq!(building.kind(), Kind::Baker);
        assert_eq!(building.at(), [10, 64, -3]);
        assert_eq!(building.level(), 3);
    }

    #[test]
    fn the_home_alias_rewrites_the_type_and_keeps_every_other_key() {
        // Level, module list and worker are read from the same compound: the alias
        // replaces `type` and nothing else (`:33`, `:42`).
        let building = from_record(&record("minecolonies:home", [1, 2, 3], 4)).expect("a residence");
        assert_eq!(building.kind(), Kind::Residence);
        assert_eq!(building.at(), [1, 2, 3]);
        assert_eq!(building.level(), 4);
    }

    #[test]
    fn an_unknown_type_is_refused_before_anything_is_produced() {
        let data = record("minecolonies:unicorn", [10, 64, -3], 2);
        assert_eq!(
            from_record(&data),
            Err(Refusal::UnknownType {
                id: "minecolonies:unicorn".to_owned()
            })
        );
        // Java's `return null` at `:35-39` writes nothing, and neither does this:
        // the record the caller handed over is untouched.
        assert_eq!(data, record("minecolonies:unicorn", [10, 64, -3], 2));
    }

    #[test]
    fn a_missing_or_unparsable_type_is_named_apart_from_an_unknown_one() {
        let no_type = Compound::from([("location".into(), position([0, 64, 0]))]);
        assert_eq!(from_record(&no_type), Err(Refusal::TypeMissing));
        let empty = Compound::from([("type".into(), Tag::String("".into()))]);
        assert_eq!(
            from_record(&empty),
            Err(Refusal::TypeInvalid {
                found: String::new()
            })
        );
        let shouty = Compound::from([("type".into(), Tag::String("Minecolonies:Builder".into()))]);
        assert_eq!(
            from_record(&shouty),
            Err(Refusal::TypeInvalid {
                found: "Minecolonies:Builder".to_owned()
            })
        );
    }

    #[test]
    fn a_tile_produces_a_fresh_building_and_reads_no_compound() {
        // `:55-58` is the three-argument overload: level 0, not built
        // (`AbstractSchematicProvider.java:64`).
        let building = from_tile(&id("minecolonies:builder"), [4, 5, 6]).expect("a builder");
        assert_eq!(building.kind(), Kind::Builder);
        assert_eq!(building.at(), [4, 5, 6]);
        assert_eq!(building.level(), 0);
        assert_eq!(building.stage(), Stage::Unbuilt);
        assert_eq!(
            from_tile(&id("minecolonies:unicorn"), [0, 0, 0]),
            Err(Refusal::UnknownType {
                id: "minecolonies:unicorn".to_owned()
            })
        );
    }

    #[test]
    fn a_view_named_home_is_a_miss_because_there_is_no_alias_on_that_path() {
        assert_eq!(view_kind(&id("minecolonies:baker")), Ok(Kind::Baker));
        assert_eq!(
            view_kind(&id("minecolonies:home")),
            Err(Refusal::UnknownType {
                id: "minecolonies:home".to_owned()
            })
        );
    }
}
