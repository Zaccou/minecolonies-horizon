//! The colony's save layout, key by key, with Java's sentence for each.
//!
//! Java reference: `core/colony/Colony.java:911-1005` (`write`) and `:760-795`
//! (`read`), the five managers that write *into* the colony's own compound —
//! `core/colony/managers/CitizenManager.java:200-205` and `:173-189`,
//! `core/colony/managers/RegisteredStructureManager.java:249-273` and `:129-176`,
//! `core/colony/workorders/WorkManager.java:226-238` and `:246-276`,
//! `core/colony/permissions/Permissions.java:560-622` and `:330-376`,
//! `core/research/LocalResearchTree.java:344-357` and `:359-368` — and the tag
//! names of `api/util/constant/NbtTagConstants.java`.
//!
//! **Why a table and not a comment.** [`KEYS`] *is* the format: one row per path
//! a written colony contains, and for each row what Java does when the key is
//! missing. A save is not judged against prose but against this table: `mod.rs`
//! walks a full save and asserts both directions — every path it contains is a
//! row here, and every row is a path a save contains. A key added to the writer
//! without a row, or a row left behind by a key that moved, fails a test instead
//! of drifting.
//!
//! **Three `presence` values, and the third is the honest one.**
//!
//! - [`Presence::Required`] — the port refuses the save when the key is absent.
//!   Reserved for the keys whose Java default silently produces a colony that
//!   cannot be told apart from a real one: an id of `0`, an empty dimension, a
//!   missing building list.
//! - [`Presence::Defaulted`] — Java's own default is taken, and the sentence is
//!   in the row. This is the *compatibility* column: a save written by Java with
//!   an old version of the mod must read here exactly as it read there.
//! - [`Presence::Unread`] — written so a Java reader finds the field, read by no
//!   code here. `ranks[].name` is one: the port's five ranks are a closed enum
//!   (`src/colony/permissions.rs:36-43`) and a save cannot rename them.
//!
//! **`*` in a path** matches the rest of the path, so a record whose keys are
//! mechanical (`center.x`, `center.y`, `center.z`) or whose key is data
//! (`building_modules.<module key>.level`) is one row and not three or fifty.

/// The version this crate writes: `ColonyConstants.DATA_VERSION`
/// (`api/util/constant/ColonyConstants.java:19`), under `DATA_VERSION_TAG`
/// (`:20`).
pub const KEY_DATA_VERSION: &str = "data_version";
/// `TAG_ID` (`NbtTagConstants.java:8`).
pub const KEY_ID: &str = "id";
/// `TAG_NAME` (`:9`).
pub const KEY_NAME: &str = "name";
/// `TAG_DIMENSION` (`:11`).
pub const KEY_DIMENSION: &str = "dimension";
/// `TAG_CENTER` (`:12`).
pub const KEY_CENTER: &str = "center";
/// **A port key.** Java has no colony level on the root: a colony's level *is*
/// its town hall's `buildingLevel` (`CitizenManager.java:596` reads it that way).
/// The port writes it so a colony whose town hall record was dropped still knows
/// its level, and derives it from the town hall when it is absent.
pub const KEY_LEVEL: &str = "level";
/// `TAG_CITIZEN_MANAGER` (`:57`).
pub const KEY_CITIZEN_MANAGER: &str = "citizenManager";
/// `TAG_BUILDING_MANAGER` (`:56`).
pub const KEY_BUILDING_MANAGER: &str = "buildingManager";
/// `TAG_CITIZENS` (`:19`), the list inside `citizenManager`.
pub const KEY_CITIZENS: &str = "citizens";
/// `TAG_BUILDINGS` (`:13`), the list inside `buildingManager`.
pub const KEY_BUILDINGS: &str = "buildings";
/// `TAG_WORK` (`:21`).
pub const KEY_WORK: &str = "work";
/// `TAG_WORK_ORDERS` (`core/colony/workorders/WorkManager.java:47`).
pub const KEY_WORK_ORDERS: &str = "workOrders";
/// `TAG_NEW_SYSTEM` (`WorkManager.java:48`): `true` once a save is in the
/// request-based order system.
pub const KEY_NEW_SYSTEM: &str = "newsystem";
/// `TAG_RESEARCH` (`NbtTagConstants.java:536`).
pub const KEY_RESEARCH: &str = "research";
/// `TAG_RANKS` (`Permissions.java:47`).
pub const KEY_RANKS: &str = "ranks";
/// `TAG_OWNERS` (`:39`).
pub const KEY_OWNERS: &str = "owners";
/// `TAG_PERMISSIONS` (`:42`).
pub const KEY_PERMISSION_MAP: &str = "permissionMap";
/// `TAG_VERSION` (`:55`), the permissions' own version, not the colony's.
pub const KEY_PERMISSION_VERSION: &str = "permissionVersion";
/// `TAG_OWNER_ID` (`:45`).
pub const KEY_OWNER_ID: &str = "ownerid";
/// `TAG_FLAGS` (`:43`), the action names a rank holds.
pub const KEY_FLAGS: &str = "flags";
/// `TAG_RANK` (`:41`).
pub const KEY_RANK: &str = "rank";
/// `TAG_INITIAL` (`:48`).
pub const KEY_IS_INITIAL: &str = "is_initial";
/// `TAG_COLONY_MANAGER` (`:49`).
pub const KEY_IS_COLONY_MANAGER: &str = "is_colony_manager";
/// `TAG_HOSTILE` (`:50`).
pub const KEY_IS_HOSTILE: &str = "is_hostile";
/// `TAG_BUILDING_TYPE` (`NbtTagConstants.java:140`), inside a building record.
pub const KEY_TYPE: &str = "type";
/// `TAG_LOCATION` (`:145`).
pub const KEY_LOCATION: &str = "location";
/// `TAG_SCHEMATIC_LEVEL` (`:150`), the building's level — *not* the colony's.
pub const KEY_BUILDING_LEVEL: &str = "level";
/// `TAG_IS_BUILT` (`:75`).
pub const KEY_IS_BUILT: &str = "isBuilt";
/// `TAG_DECONSTRUCTED` (`:713`).
pub const KEY_DECONSTRUCTED: &str = "deconstructed";
/// `TAG_PARENT_SCHEM` (`:155`).
pub const KEY_PARENT: &str = "parent";
/// `TAG_BUILDING_MODULES` (`:15`).
pub const KEY_BUILDING_MODULES: &str = "building_modules";
/// `TAG_WORKER` (`:343`).
pub const KEY_WORKER: &str = "worker";
/// `TAG_SATURATION` (`:495`).
pub const KEY_SATURATION: &str = "saturation";
/// `TAG_ASLEEP` (`:496`).
pub const KEY_ASLEEP: &str = "asleep";
/// `TAG_JUST_ATE` (`:84`).
pub const KEY_JUST_ATE: &str = "justAte";

/// How a value is stored. `Position` is the three-int compound
/// `BlockPosUtil.write` builds (`api/util/BlockPosUtil.java:190-197`), which is
/// why a coordinate is a row of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the NBT kinds this format uses, closed on purpose
pub enum TagKind {
    Byte,
    Int,
    String,
    Double,
    Position,
    List,
    Compound,
}

/// What the port does when the key is not there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three answers this format allows
pub enum Presence {
    /// The save is refused, with the refusal named in `mod.rs`.
    Required,
    /// Java's default is taken, exactly as `java` describes it.
    Defaulted,
    /// Written for a Java reader; nothing here reads it back.
    Unread,
}

/// One row of the format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: a static row of the table
pub struct Key {
    /// The path, relative to the colony compound. `[]` marks a list item, `*`
    /// matches the rest of the path.
    pub path: &'static str,
    pub tag: TagKind,
    pub presence: Presence,
    /// What Java does when this key is absent, with the line that says so.
    pub java: &'static str,
    /// What this port does, when it is not simply Java's answer.
    pub port: &'static str,
}

/// The format, in the order the colony's own compound nests it: identity first,
/// then permissions, then the three managers, then research.
pub const KEYS: &'static [Key] = &[
    Key {
        path: "data_version",
        tag: TagKind::Int,
        presence: Presence::Defaulted,
        java: "Colony.java:913 writes ColonyConstants.DATA_VERSION; no code in the tree reads it back, so an absent version is never noticed",
        port: "read as 0 and accepted; anything above 1 is refused, because a later format read by this one is a guess",
    },
    Key {
        path: "id",
        tag: TagKind::Int,
        presence: Presence::Required,
        java: "Colony.java:916 putInt / :729 getInt — absent reads 0, and a colony of id 0 answers for every unclaimed position",
        port: "refused when absent or not positive",
    },
    Key {
        path: "name",
        tag: TagKind::String,
        presence: Presence::Defaulted,
        java: "Colony.java:920 / :731 getString — absent is the empty name, which a colony may have",
        port: "same",
    },
    Key {
        path: "dimension",
        tag: TagKind::String,
        presence: Presence::Required,
        java: "Colony.java:917 writes dimensionId.location(); :733 reads it with new ResourceLocation(\"\") which throws, and loadColony catches and drops the colony (:740-744)",
        port: "refused when absent or blank; Java's outcome is the same, its reason is an exception",
    },
    Key {
        path: "center",
        tag: TagKind::Position,
        presence: Presence::Defaulted,
        java: "Colony.java:921 BlockPosUtil.write / :732 read — getCompound on a missing key is an empty compound, so the centre reads as the origin",
        port: "the origin, and a repair says so: a colony whose centre moved is a colony the map points at the wrong place",
    },
    Key {
        path: "center.*",
        tag: TagKind::Int,
        presence: Presence::Defaulted,
        java: "BlockPosUtil.java:190-197, three getInt on the coordinates — each absent one is 0",
        port: "same",
    },
    Key {
        path: "level",
        tag: TagKind::Int,
        presence: Presence::Defaulted,
        java: "no such key: Java has no colony level; the town hall's buildingLevel stands in for it (CitizenManager.java:596)",
        port: "port key, written from Colony::level(); on read the town hall decides, and a stored level that disagrees is refused",
    },
    Key {
        path: "ranks",
        tag: TagKind::List,
        presence: Presence::Defaulted,
        java: "Permissions.java:334 — without it, loadRanks() (:164-219) builds the five stock ranks from the fall-through switch",
        port: "same table (Rank::mask in src/colony/permissions.rs:83-98); a rank id outside the five is ignored",
    },
    Key {
        path: "ranks[].id",
        tag: TagKind::Int,
        presence: Presence::Defaulted,
        java: "Permissions.java:342 getInt — absent is 0, which is the Owner rank",
        port: "same",
    },
    Key {
        path: "ranks[].name",
        tag: TagKind::String,
        presence: Presence::Unread,
        java: "Permissions.java:343 / :568, the rank's display name",
        port: "written, not read: the five names are an enum (src/colony/permissions.rs:57-65)",
    },
    Key {
        path: "ranks[].is_initial",
        tag: TagKind::Byte,
        presence: Presence::Unread,
        java: "Permissions.java:344 / :569, true for the five stock ranks (:168)",
        port: "written as true, not read: a save cannot make a stock rank deletable",
    },
    Key {
        path: "ranks[].is_colony_manager",
        tag: TagKind::Byte,
        presence: Presence::Unread,
        java: "Permissions.java:345 / :570, set on OWNER and OFFICER by the fall-through (:191)",
        port: "written from Rank::is_manager, not read, for the same reason",
    },
    Key {
        path: "ranks[].is_hostile",
        tag: TagKind::Byte,
        presence: Presence::Unread,
        java: "Permissions.java:346 / :571, set on HOSTILE (:214)",
        port: "written from Rank::is_hostile, not read",
    },
    Key {
        path: "owners",
        tag: TagKind::List,
        presence: Presence::Defaulted,
        java: "Permissions.java:387 getList — empty, and restoreOwnerIfNull (:453-469) then rebuilds the owner from ownerid",
        port: "same, and a colony left with no owner reports it",
    },
    Key {
        path: "owners[].id",
        tag: TagKind::String,
        presence: Presence::Required,
        java: "Permissions.java:391 UUID.fromString(getString) — an empty string throws, and loadColony drops the colony (:740-744)",
        port: "refused when absent or not a UUID; the index of the record is named",
    },
    Key {
        path: "owners[].rank",
        tag: TagKind::Int,
        presence: Presence::Required,
        java: "Permissions.java:400 getInt when permissionVersion >= 3, else :404 OldRank.valueOf(getString); absent reads 0, which is the Owner rank",
        port: "refused when absent: a member handed ownership by omission is not a save this port reads",
    },
    Key {
        path: "permissionMap",
        tag: TagKind::List,
        presence: Presence::Defaulted,
        java: "Permissions.java:353 — without it a rank keeps only the bits upgradePermissions (:228-262) adds, which are the stock table's",
        port: "same, and a map that disagrees with Rank::mask is reported",
    },
    Key {
        path: "permissionMap[].rank",
        tag: TagKind::Int,
        presence: Presence::Defaulted,
        java: "Permissions.java:358 getInt, then ranks.get — a rank the table does not hold is skipped (:359-362)",
        port: "same",
    },
    Key {
        path: "permissionMap[].flags",
        tag: TagKind::List,
        presence: Presence::Defaulted,
        java: "Permissions.java:364 getList — no flags at all",
        port: "same",
    },
    Key {
        path: "permissionMap[].flags[]",
        tag: TagKind::String,
        presence: Presence::Defaulted,
        java: "Permissions.java:366-375 Action.valueOf(name), an unknown name is caught and ignored",
        port: "same, so a flag this port has no enum for cannot grant anything",
    },
    Key {
        path: "permissionVersion",
        tag: TagKind::Int,
        presence: Presence::Defaulted,
        java: "Permissions.java:332 getInt — absent is 0, which is the pre-rank-table format where a rank is an OldRank name (:404)",
        port: "same threshold (< 3 reads a name, >= 3 reads an int); above 5 is refused",
    },
    Key {
        path: "ownerid",
        tag: TagKind::String,
        presence: Presence::Defaulted,
        java: "Permissions.java:424 getString, used by restoreOwnerIfNull (:453-469) when owners no longer holds an owner",
        port: "same, and the restore is reported",
    },
    Key {
        path: "citizenManager",
        tag: TagKind::Compound,
        presence: Presence::Required,
        java: "Colony.java:770 getCompound(TAG_CITIZEN_MANAGER) — an absent key is an empty compound, so the colony reads with no citizens and every job unassigned",
        port: "refused: the port has no backup file to fall back on, and a colony that lost its whole population is not one to load quietly",
    },
    Key {
        path: "citizenManager.citizens",
        tag: TagKind::List,
        presence: Presence::Required,
        java: "CitizenManager.java:178 getList(TAG_CITIZENS) — absent is an empty list, citizens.clear() at :176 having already run",
        port: "refused; an empty list is read as a colony whose population died, which is a legitimate state",
    },
    Key {
        path: "citizenManager.citizens[].id",
        tag: TagKind::Int,
        presence: Presence::Required,
        java: "CitizenData.java:1320 putInt / CitizenDataManager.java:16 getInt — absent is 0, a citizen no factory ever hands out",
        port: "refused, with the index of the record",
    },
    Key {
        path: "citizenManager.citizens[].name",
        tag: TagKind::String,
        presence: Presence::Defaulted,
        java: "CitizenData.java:1321 putString / :1427 getString — absent is the empty name",
        port: "same, and the citizen is reported unnamed",
    },
    Key {
        path: "citizenManager.citizens[].saturation",
        tag: TagKind::Double,
        presence: Presence::Defaulted,
        java: "CitizenData.java:1337 putDouble / :1460 getDouble — absent reads 0.0, which is a starving citizen, not the maximum :526 creates",
        port: "same 0.0, because reading it as full would refill a colony Java starved; the repair says so",
    },
    Key {
        path: "citizenManager.citizens[].asleep",
        tag: TagKind::Byte,
        presence: Presence::Defaulted,
        java: "CitizenData.java:1355 putBoolean / :1479-1482 only when present — absent is false",
        port: "same",
    },
    Key {
        path: "citizenManager.citizens[].justAte",
        tag: TagKind::Byte,
        presence: Presence::Defaulted,
        java: "CitizenData.java:1356 putBoolean / :1485-1487 only when present — absent is false",
        port: "same",
    },
    Key {
        path: "buildingManager",
        tag: TagKind::Compound,
        presence: Presence::Required,
        java: "Colony.java:773 getCompound(TAG_BUILDING_MANAGER) — absent is a colony with no buildings, so no town hall, and none can be founded again",
        port: "refused",
    },
    Key {
        path: "buildingManager.buildings",
        tag: TagKind::List,
        presence: Presence::Required,
        java: "RegisteredStructureManager.java:160-176 getList(TAG_BUILDINGS), an entry whose factory answers null is skipped (:172-176)",
        port: "refused when the list itself is missing; an unreadable entry is dropped and reported, as Java does",
    },
    Key {
        path: "buildingManager.buildings[].*",
        tag: TagKind::Compound,
        presence: Presence::Defaulted,
        java: "one building record, written by AbstractSchematicProvider.java:171-192 and AbstractBuilding.java:328-367",
        port: "src/building/instance.rs:350-421 owns the record and documents it; an entry with no known type is dropped, per BuildingDataManager.java:35-49",
    },
    Key {
        path: "buildingManager.buildings[].type",
        tag: TagKind::String,
        presence: Presence::Defaulted,
        java: "NbtTagConstants.java:140, looked up by BuildingDataManager.java:38-44 — unknown or absent means no building, and the entry is skipped",
        port: "same, reported",
    },
    Key {
        path: "buildingManager.buildings[].location",
        tag: TagKind::Position,
        presence: Presence::Defaulted,
        java: "NbtTagConstants.java:145, read by BlockPosUtil.read — absent is the origin, and two buildings at the origin collide",
        port: "same; two records at one position are refused before they can collide",
    },
    Key {
        path: "buildingManager.buildings[].location.*",
        tag: TagKind::Int,
        presence: Presence::Defaulted,
        java: "BlockPosUtil.java:190-197, each coordinate 0 when absent",
        port: "same",
    },
    Key {
        path: "buildingManager.buildings[].level",
        tag: TagKind::Int,
        presence: Presence::Defaulted,
        java: "NbtTagConstants.java:150, copied without a guard by AbstractSchematicProvider.java:191",
        port: "clamped to 0..=5 by crate::building::clamp_level",
    },
    Key {
        path: "buildingManager.buildings[].isBuilt",
        tag: TagKind::Byte,
        presence: Presence::Defaulted,
        java: "AbstractBuilding.java:332-339 — absent reads as built when the level is above 0, which is the legacy save path",
        port: "same",
    },
    Key {
        path: "buildingManager.buildings[].deconstructed",
        tag: TagKind::Byte,
        presence: Presence::Defaulted,
        java: "AbstractSchematicProvider.java:182 putBoolean, and :212-215 read only when present",
        port: "absent is false",
    },
    Key {
        path: "buildingManager.buildings[].parent",
        tag: TagKind::Position,
        presence: Presence::Defaulted,
        java: "TAG_PARENT_SCHEM, BlockPos.ZERO meaning none (AbstractSchematicProvider.java:297-306); the port writes no key instead",
        port: "absent is no parent",
    },
    Key {
        path: "buildingManager.buildings[].worker",
        tag: TagKind::Int,
        presence: Presence::Defaulted,
        java: "TAG_WORKER holds a list of {workerId} compounds (WorkerBuildingModule.java:121-135) and a worker whose citizen is gone is skipped (:129-133)",
        port: "a port key as an int: a worker who is not in the citizen list is unassigned and reported",
    },
    Key {
        path: "buildingManager.buildings[].building_modules",
        tag: TagKind::Compound,
        presence: Presence::Defaulted,
        java: "AbstractBuilding.java:349-366 one compound keyed by module key; without it each module reads the building's own compound (:365)",
        port: "absent is no module",
    },
    Key {
        path: "buildingManager.buildings[].building_modules.*",
        tag: TagKind::Compound,
        presence: Presence::Defaulted,
        java: "the module's own compound, keyed by its serialisation key (BuildingEntry.java:206-230, the key is never the runtime id)",
        port: "the key is kept, the level is the only thing a port module holds",
    },
    Key {
        path: "buildingManager.buildings[].building_modules.*.level",
        tag: TagKind::Int,
        presence: Presence::Defaulted,
        java: "no such key: a module reads its building's level instead (AbstractSchematicProvider.java:485-490)",
        port: "port key; a module above its building is dropped on read (src/building/instance.rs:626-637)",
    },
    Key {
        path: "work",
        tag: TagKind::Compound,
        presence: Presence::Defaulted,
        java: "Colony.java:795 getCompound(TAG_WORK) — absent is an empty compound, and WorkManager.read (:250-254) then drops every order because newsystem is gone",
        port: "same, and a save that carried orders loses them with a repair naming the count",
    },
    Key {
        path: "work.workOrders",
        tag: TagKind::List,
        presence: Presence::Defaulted,
        java: "WorkManager.java:257 getList(TAG_WORK_ORDERS) — absent is no order",
        port: "same",
    },
    Key {
        path: "work.newsystem",
        tag: TagKind::Byte,
        presence: Presence::Defaulted,
        java: "WorkManager.java:250 — without it read() returns at :254, so every order of a pre-request save is dropped on purpose",
        port: "same, reported",
    },
    Key {
        path: "work.workOrders[].*",
        tag: TagKind::Compound,
        presence: Presence::Defaulted,
        java: "the order record of AbstractWorkOrder.java:709-730; the port's own layout is in src/colony/ordre.rs:213-251",
        port: "read by crate::colony::ordre::WorkOrder::from_compound; a record that does not read is dropped and reported",
    },
    Key {
        path: "research",
        tag: TagKind::Compound,
        presence: Presence::Defaulted,
        java: "Colony.java:786 guards the read with contains(TAG_RESEARCH) — absent is a colony that has researched nothing",
        port: "same",
    },
    Key {
        path: "research.researchTree",
        tag: TagKind::List,
        presence: Presence::Defaulted,
        java: "LocalResearchTree.java:365 getList(TAG_RESEARCH_TREE) — absent is no record",
        port: "same",
    },
    Key {
        path: "research.researchTree[].*",
        tag: TagKind::Compound,
        presence: Presence::Defaulted,
        java: "LocalResearchFactory.java:45-56 writes state, id, branch, progress and depth; :58-70 reads them back",
        port: "read by crate::research::Progress::from_compound, which drops and counts a record it cannot read",
    },
];

/// Whether a path walks a documented row.
///
/// Three ways to match: the row names the path, the row ends in `.*` and names a
/// prefix of it, or the path is a *container* some documented path lives in
/// (`ranks[]` is documented by `ranks[].id`, because a compound with no leaf of
/// its own is only ever as interesting as its leaves). Test-only: it is the check
/// the table is held to, not a reader.
#[cfg(test)]
pub fn documented(path: &str) -> bool {
    KEYS.iter().any(|key| {
        key.path == path
            || key
                .path
                .strip_suffix(".*")
                .is_some_and(|prefix| path.starts_with(prefix))
            || key.path.starts_with(&format!("{path}."))
            || key.path.starts_with(&format!("{path}["))
    })
}

/// The path a walker reports, with a module's dynamic key replaced by `*`
/// (`building_modules.work.level` → `building_modules.*.level`). Test-only, like
/// [`documented`].
#[cfg(test)]
pub fn normalise(path: &str) -> String {
    const MODULES: &str = "buildingManager.buildings[].building_modules.";
    let Some(rest) = path.strip_prefix(MODULES) else {
        return path.to_owned();
    };
    match rest.split_once('.') {
        Some((_key, tail)) => format!("{MODULES}*.{tail}"),
        None => format!("{MODULES}*"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_two_rows_describe_the_same_path() {
        for (index, key) in KEYS.iter().enumerate() {
            for other in &KEYS[index + 1..] {
                assert_ne!(key.path, other.path, "two rows, one path");
            }
        }
    }

    #[test]
    fn every_row_carries_a_java_sentence() {
        for key in KEYS {
            assert!(
                key.java.contains(".java:"),
                "{} has no Java line to cite: \"{}\"",
                key.path,
                key.java
            );
            assert!(!key.port.is_empty(), "{} says nothing about the port", key.path);
        }
    }

    #[test]
    fn the_required_rows_are_the_ones_the_reader_refuses_on() {
        let required: Vec<&str> = KEYS
            .iter()
            .filter(|key| key.presence == Presence::Required)
            .map(|key| key.path)
            .collect();
        assert_eq!(
            required,
            vec![
                "id",
                "dimension",
                "owners[].id",
                "owners[].rank",
                "citizenManager",
                "citizenManager.citizens",
                "citizenManager.citizens[].id",
                "buildingManager",
                "buildingManager.buildings",
            ],
            "every required row is a refusal named in mod.rs, and nothing else is"
        );
    }
}
