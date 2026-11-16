//! The buildings: what a hut *is*, what it costs, what it demands, how far it
//! has got.
//!
//! Java reference: `api/colony/buildings/` (the interfaces and the registry) and
//! `core/colony/buildings/` (the implementations). **54 building types** are
//! registered — Java declares 55 ids and one entry is commented out; `kind.rs`
//! holds the count and says where it comes from.
//!
//! | File | Question |
//! | --- | --- |
//! | `kind` | which kinds exist, and the registry path each is keyed by |
//! | `instance` | one building: level, stage, modules, worker, its compound |
//! | `cost` | what a kind costs, level by level, and what is still missing |
//! | `requirements` | what a kind demands to be placed or to grow |
//! | `nbt` | the tag reads and writes the others share |
//!
//! **Pure data, pure rules.** Nothing here calls the host: a wasm mod gets no
//! clock, no thread and no file, and the placement hooks will call into this
//! module later, as `colony` and `inventory` already are. Every rule returns a
//! [`Refusal`] with a name, never a `false` — a hut that will not appear has to
//! say why, which is what MineColonies' own `WARNING_*` messages are for.
//!
//! **What is *not* here**, and what each gap costs:
//!
//! - **no per-kind module list**: Java fills it when it registers the entry
//!   (`apiimp/initializer/ModBuildingsInitializer.java:95-107` gives the builder
//!   seven modules), so a pack that knows "a builder has a work module" needs a
//!   table this module does not carry yet. [`Building::attach_module`] takes
//!   whatever is handed to it.
//! - **one worker per building**: Java's count is per module
//!   (`api/colony/buildings/modules/IAssignsCitizen.java:37-46`), so a warehouse
//!   holds its couriers; this record holds one citizen.
//! - **no research gate**: `requestUpgrade` refuses an upgrade the colony's
//!   research has not unlocked (`core/colony/buildings/AbstractBuilding.java:786-799`),
//!   and the port has no research system.
//! - **no work order**: an upgrade here is a fact the caller states, not a
//!   `BUILD`/`UPGRADE` order handed to a builder
//!   (`AbstractBuilding.java:803-810`);
//!   [`crate::colony::WorkOrder`] is where that lives.

pub mod behaviour;
mod cost;
mod instance;
mod kind;
mod nbt;
mod requirements;

pub use cost::CostTable;
pub use instance::{Building, Module, Stage, clamp_level};
pub use kind::Kind;
pub use requirements::{Footprint, ParentRule, Requirements, Rules, Site, Standing};

/// The lowest **built** level. A hut placed and not yet built is level 0, as in
/// Java (`core/colony/buildings/AbstractSchematicProvider.java:64`), and this is
/// what a finished build starts at.
pub const LEVEL_MIN: i32 = 1;

/// The top building level, `Constants.MAX_BUILDING_LEVEL`
/// (`api/util/constant/Constants.java:20`) and
/// `BuildingConstants.CONST_DEFAULT_MAX_BUILDING_LEVEL` (`:11`), two constants
/// with the same value that Java never reconciled.
pub const MAX_BUILDING_LEVEL: i32 = 5;

/// Why a building operation was refused, named.
///
/// Every variant is a rule that exists in Java, with one exception marked as
/// such: [`Self::ModuleAboveBuilding`] and [`Self::LevelBelowModule`] are the two
/// halves of a rule this port adds, because a module here has a level of its own
/// and Java's modules do not.
///
/// A variant is **not** a `lang` key: those live in
/// `assets/minecolonies/lang/`, which this module does not own. The name is for
/// logs and tests, namespaced so it can never be mistaken for a sentence.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// No town hall stands in the colony, and this is not one
    /// (`api/blocks/AbstractBlockHut.java:309-327`).
    TownHallMissing,
    /// A town hall already stands: a colony has one
    /// (`core/blocks/huts/BlockHutTownHall.java:219-234`).
    TownHallAlreadyThere { at: [i32; 3] },
    /// Above [`MAX_BUILDING_LEVEL`]. Java refuses it **silently**
    /// (`AbstractSchematicProvider.java:482-485`); the port names it.
    LevelAboveMax { level: i32 },
    /// Below 0 for a stored level, below [`LEVEL_MIN`] for a finished build.
    LevelOutOfRange { level: i32 },
    /// The cost table has no entry for this kind at this level: the level does
    /// not exist in the data, which is not the same as costing nothing.
    NoSuchLevel { kind: Kind, level: i32 },
    /// The kind must be nested inside a building of this kind, and none was
    /// named.
    ParentRequired { kind: Kind },
    /// A parent position was named and nothing stands there. Java's
    /// `isParentValid` returns false and `setParent` silently does nothing
    /// (`AbstractSchematicProvider.java:309-321`).
    ParentMissing { at: [i32; 3] },
    /// The parent is the building itself (`:320`).
    ParentSelf { at: [i32; 3] },
    /// The parent has a parent: nesting stops at one level (`:319-320`).
    ParentNested { at: [i32; 3] },
    /// The building stands in something, but not in what the data asks for.
    /// Java does not check the kind, only the position (`:317-321`); this port
    /// adds the check because a barracks tower outside barracks has nowhere to
    /// belong.
    ParentWrongKind {
        at: [i32; 3],
        found: Kind,
        required: Kind,
    },
    /// The parent has not reached the level the data asks for.
    ParentLevelTooLow {
        at: [i32; 3],
        level: i32,
        required: i32,
    },
    /// The hut block is not in the box its own schematic covers, so the
    /// schematic and the placement disagree (`AbstractSchematicProvider.java:505-521`
    /// gives one block of slack, and this is beyond it).
    HutOutsideFootprint { at: [i32; 3] },
    /// The box is smaller than the kind needs.
    FootprintTooSmall {
        required: [u32; 3],
        measured: [u32; 3],
    },
    /// A module with this key is already attached
    /// (`core/colony/buildings/AbstractBuilding.java:210-216`).
    ModuleDuplicate { key: String },
    /// No module has this key.
    ModuleUnknown { key: String },
    /// A module may not outrank its building. **A rule of this port's own**:
    /// Java's modules have no level, they read the building's.
    ModuleAboveBuilding {
        key: String,
        level: i32,
        building: i32,
    },
    /// Lowering the building would leave a module above it — the same rule seen
    /// from the other end.
    LevelBelowModule {
        key: String,
        level: i32,
        level_asked: i32,
    },
    /// Nothing is built yet, so the hut takes no worker
    /// (`api/colony/buildings/IBuilding.java:464-467`) — and a level-0 hut is a
    /// build, not an upgrade (`AbstractBuilding.java:803-805`).
    NotBuilt,
    /// The hut already has its worker. Java puts the capacity on the module
    /// (`IAssignsCitizen.getModuleMax`); this record holds one.
    Occupied { worker: i32 },
    /// Nobody works here.
    Vacant,
    /// The building is at [`MAX_BUILDING_LEVEL`] and cannot grow: Java's
    /// `WARNING_NO_UPGRADE` branch (`AbstractBuilding.java:811-814`).
    AlreadyMaxLevel { level: i32 },
    /// The building has caught its parent, which is not at the top of its own
    /// range yet, so the parent has to grow first
    /// (`AbstractBuilding.java:807`, the refusal side of the condition).
    ParentLevelAhead { level: i32, parent: i32 },
}

impl Refusal {
    /// A stable name for logs and tests, namespaced like a registry entry so it
    /// can never be mistaken for a player-facing sentence.
    pub fn name(&self) -> &'static str {
        match self {
            Self::TownHallMissing => "building.refusal.town_hall_missing",
            Self::TownHallAlreadyThere { .. } => "building.refusal.town_hall_already_there",
            Self::LevelAboveMax { .. } => "building.refusal.level_above_max",
            Self::LevelOutOfRange { .. } => "building.refusal.level_out_of_range",
            Self::NoSuchLevel { .. } => "building.refusal.no_such_level",
            Self::ParentRequired { .. } => "building.refusal.parent_required",
            Self::ParentMissing { .. } => "building.refusal.parent_missing",
            Self::ParentSelf { .. } => "building.refusal.parent_self",
            Self::ParentNested { .. } => "building.refusal.parent_nested",
            Self::ParentWrongKind { .. } => "building.refusal.parent_wrong_kind",
            Self::ParentLevelTooLow { .. } => "building.refusal.parent_level_too_low",
            Self::HutOutsideFootprint { .. } => "building.refusal.hut_outside_footprint",
            Self::FootprintTooSmall { .. } => "building.refusal.footprint_too_small",
            Self::ModuleDuplicate { .. } => "building.refusal.module_duplicate",
            Self::ModuleUnknown { .. } => "building.refusal.module_unknown",
            Self::ModuleAboveBuilding { .. } => "building.refusal.module_above_building",
            Self::LevelBelowModule { .. } => "building.refusal.level_below_module",
            Self::NotBuilt => "building.refusal.not_built",
            Self::Occupied { .. } => "building.refusal.occupied",
            Self::Vacant => "building.refusal.vacant",
            Self::AlreadyMaxLevel { .. } => "building.refusal.already_max_level",
            Self::ParentLevelAhead { .. } => "building.refusal.parent_level_ahead",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_refusal_has_its_own_name() {
        let at = [1, 2, 3];
        let key = String::from("work");
        let all = [
            Refusal::TownHallMissing,
            Refusal::TownHallAlreadyThere { at },
            Refusal::LevelAboveMax { level: 6 },
            Refusal::LevelOutOfRange { level: -1 },
            Refusal::NoSuchLevel {
                kind: Kind::Builder,
                level: 3,
            },
            Refusal::ParentRequired {
                kind: Kind::Barracks,
            },
            Refusal::ParentMissing { at },
            Refusal::ParentSelf { at },
            Refusal::ParentNested { at },
            Refusal::ParentWrongKind {
                at,
                found: Kind::TownHall,
                required: Kind::Barracks,
            },
            Refusal::ParentLevelTooLow {
                at,
                level: 1,
                required: 2,
            },
            Refusal::HutOutsideFootprint { at },
            Refusal::FootprintTooSmall {
                required: [5, 5, 5],
                measured: [4, 4, 4],
            },
            Refusal::ModuleDuplicate { key: key.clone() },
            Refusal::ModuleUnknown { key: key.clone() },
            Refusal::ModuleAboveBuilding {
                key: key.clone(),
                level: 4,
                building: 3,
            },
            Refusal::LevelBelowModule {
                key,
                level: 2,
                level_asked: 1,
            },
            Refusal::NotBuilt,
            Refusal::Occupied { worker: 7 },
            Refusal::Vacant,
            Refusal::AlreadyMaxLevel { level: 5 },
            Refusal::ParentLevelAhead {
                level: 3,
                parent: 3,
            },
        ];
        for (index, refusal) in all.iter().enumerate() {
            assert!(refusal.name().starts_with("building.refusal."));
            assert_eq!(refusal.to_string(), refusal.name(), "Display is the name");
            for other in &all[index + 1..] {
                assert_ne!(
                    refusal.name(),
                    other.name(),
                    "two refusals share a name, so a log line cannot tell them apart"
                );
            }
        }
    }

    #[test]
    fn the_two_levels_java_never_reconciled_are_the_same_number() {
        // Constants.java:20 and BuildingConstants.java:11 both say 5, in two
        // different classes; the port keeps one, and this is the check that it
        // is the one every rule quotes.
        assert_eq!(MAX_BUILDING_LEVEL, 5);
        assert_eq!(LEVEL_MIN, 1);
        assert!(LEVEL_MIN <= MAX_BUILDING_LEVEL);
    }
}
