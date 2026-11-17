//! Why a hut said no, named.
//!
//! Java reference, one line per variant, all in `core/colony/buildings/` and
//! `api/colony/buildings/`. **No variant is invented**: each one is a branch
//! Java really takes, and most of them take it *silently* — a `return` with no
//! message, or a `WARNING_*` the player may never read. The port names them so a
//! log line, a test and a command can tell two refusals apart.
//!
//! **Why a second refusal enum.** `crate::building::Refusal` already names the
//! rules of a *level* change, a parent and a placement. Those rules are not
//! restated here: [`Refusal::Building`] wraps that enum and reuses its name, so
//! `building.refusal.module_duplicate` keeps one spelling and one meaning
//! wherever it surfaces. What this enum adds is the refusals *about a module and
//! its content*, which the level rules never had to answer.
//!
//! A variant is **not** a `lang` key: those live in `assets/minecolonies/lang/`,
//! which this module does not own. The name is namespaced so it can never be
//! mistaken for a sentence.

use crate::building::Kind;

use super::key::ModuleKey;

/// A refusal a building or one of its modules opposes.
///
/// **`PartialEq` without `Eq`**, on purpose: [`Refusal::HutResearchTooLow`]
/// carries the effect's strength, and Java's `getEffectStrength` returns a
/// `double` (`api/research/IResearchEffectManager.java:17`), so the value is a
/// float in the source and `f64` is not `Eq`. Every other variant could be `Eq`;
/// the enum as a whole cannot, and a comparison against an exact literal
/// (`strength: 2.0`) is still exact because the number is copied, not computed.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// The hut's registry entry does not register this module, so there is
    /// nothing to ask. Java's `getModule(producer)` returns `null` and the caller
    /// either checks first or throws
    /// (`api/colony/buildings/registry/BuildingEntry.java:148-160` is the list;
    /// `core/colony/buildings/AbstractBuilding.java:196-200` is the unchecked
    /// getter).
    ModuleNotOnThisHut { kind: Kind, key: ModuleKey },
    /// The hut is placed and not built, so nothing in it is live: the level is 0
    /// (`AbstractSchematicProvider.java:64`), `canAssignCitizens` is false
    /// (`api/colony/buildings/IBuilding.java:464-467`) and the tavern's own tick
    /// returns on `level <= 0` (`modules/TavernBuildingModule.java:98-101`).
    ///
    /// The port also raises it for a **deconstructed** hut, whose `isBuilt` is
    /// still true in Java (`AbstractSchematicProvider.java:142-145`) but whose
    /// structure is gone; [`super::rules::Hut::stands`] says why.
    HutUnderConstruction { kind: Kind, level: i32 },
    /// The module exists and the hut is built, but the research its hiring needs
    /// is not known: `WorkerBuildingModule.onColonyTick` requires
    /// `getEffectStrength(researchRequirement) > 0`
    /// (`modules/WorkerBuildingModule.java:167`), which is how a marksman and a
    /// huscarl stay unhireable until their effect is researched
    /// (`BuildingModules.java:558-563`, `:575-578`).
    ResearchMissing {
        key: ModuleKey,
        research: &'static str,
    },
    /// The module's own feature is not switched on yet at this level: the
    /// warehouse sorts from level 3 (`workerbuildings/BuildingWareHouse.java:194-197`),
    /// the baker accepts crafting types from level 3
    /// (`workerbuildings/BuildingBaker.java:161-165`), the florist filters a
    /// second flower from level 4 (`moduleviews/FloristFlowerListModuleView.java:56-58`).
    LevelTooLow {
        kind: Kind,
        key: ModuleKey,
        level: i32,
        needed: i32,
    },
    /// The module holds its maximum. Java's own answer is a silent `return` in
    /// three places — `addMinimumStock` (`modules/MinimumStockModule.java:63-67`),
    /// `addMenuItem` (`modules/RestaurantMenuModule.java:96-101`),
    /// `canLearnRecipe` (`modules/AbstractCraftingBuildingModule.java:147-151`) —
    /// and a boolean in the fourth, `isFull`
    /// (`modules/AbstractAssignedCitizenModule.java:105-109`).
    Full {
        key: ModuleKey,
        capacity: i32,
        held: i32,
    },
    /// The module assigns no citizen, so a citizen cannot be given to it: Java
    /// would look for an `IAssignsCitizen` and find a settings page
    /// (`api/colony/buildings/modules/IAssignsCitizen.java:14-46`).
    NotAnAssignment { key: ModuleKey },
    /// The module keeps no recipe list, so a recipe cannot be given to it: only
    /// `AbstractCraftingBuildingModule` and its subclasses own one
    /// (`modules/AbstractCraftingBuildingModule.java:171-179`).
    NotACraftingModule { key: ModuleKey },
    /// The module counts nothing, so there is no head count, no stock and no
    /// recipe list to ask for. **A refusal of this port's own**: Java never asks
    /// a statistics panel how many citizens it holds, so it has no branch to
    /// name — the port must, because [`super::rules::Hut::seats`] takes any key.
    NotCounted { key: ModuleKey },
    /// The hut does not sort its inventory at all: `IBuilding.canSort()` is
    /// `false` by default and only the warehouse overrides it
    /// (`api/colony/buildings/IBuilding.java:518-525`).
    NotSortable { kind: Kind },
    /// Above the **kind's own** ceiling. Java's `setBuildingLevel` compares the
    /// level to `getMaxBuildingLevel()` and returns **silently**
    /// (`core/colony/buildings/AbstractSchematicProvider.java:480-485`), which is
    /// 3 for a tavern (`apiimp/initializer/ModBuildingsInitializer.java:521`) and
    /// 0 for a stash (`workerbuildings/Stash.java:54-57`).
    /// `crate::building::Building::set_level` only knows the global 5, so the
    /// per-kind ceiling is named here rather than there.
    LevelAboveHutMax { kind: Kind, level: i32, max: i32 },
    /// The hut's **own** research effect is not high enough. Java writes two
    /// messages for one effect: the hut cannot be unlocked at all while the effect
    /// is below 1 (`core/colony/buildings/AbstractBuilding.java:786-792`,
    /// `WARNING_BUILDING_REQUIRES_RESEARCH_UNLOCK`), and it cannot be upgraded
    /// while the effect is not above its current level (`:793-800`,
    /// `WARNING_BUILDING_REQUIRES_RESEARCH_UPGRADE`). `needed` is 1 in the first
    /// case and `level + 1` in the second.
    ///
    /// `strength` is a **float because Java's is**: `getEffectStrength` returns a
    /// `double` (`api/research/IResearchEffectManager.java:17`), and the two tests
    /// above compare that double to `1` and to an `int` level.
    HutResearchTooLow {
        kind: Kind,
        effect: &'static str,
        strength: f64,
        needed: i32,
    },
    /// The florist's flower filter holds one flower up to level 3, and Java
    /// refuses the second with `TOO_MANY_FILTERED_BELOW_LVL4_FLORIST`
    /// (`moduleviews/FloristFlowerListModuleView.java:56-58`).
    FlowersNotSortedYet { level: i32 },
    /// The florist's flower filter holds five flowers up to level 4, and Java
    /// refuses the sixth with `TOO_MANY_FILTERED_FLORIST` (`:62-66`).
    FlowersNotFilteredAtThisLevel { level: i32, limit: i32 },
    /// A rule `crate::building` already names: an unknown module, a duplicate, a
    /// module above its building, a level out of range. Passed through so the two
    /// enums never spell the same refusal twice.
    Building(crate::building::Refusal),
}

impl Refusal {
    /// A stable name for logs and tests, namespaced like a registry entry.
    ///
    /// The wrapped [`crate::building::Refusal`] answers its own name, which is
    /// the point of the wrapper: one refusal, one spelling.
    pub fn name(&self) -> &'static str {
        match self {
            Self::ModuleNotOnThisHut { .. } => "building.module.refusal.not_on_this_hut",
            Self::HutUnderConstruction { .. } => "building.module.refusal.under_construction",
            Self::ResearchMissing { .. } => "building.module.refusal.research_missing",
            Self::LevelTooLow { .. } => "building.module.refusal.level_too_low",
            Self::Full { .. } => "building.module.refusal.full",
            Self::NotAnAssignment { .. } => "building.module.refusal.not_an_assignment",
            Self::NotACraftingModule { .. } => "building.module.refusal.not_a_crafting_module",
            Self::NotCounted { .. } => "building.module.refusal.not_counted",
            Self::NotSortable { .. } => "building.module.refusal.not_sortable",
            Self::LevelAboveHutMax { .. } => "building.hut.refusal.level_above_max",
            Self::HutResearchTooLow { .. } => "building.hut.refusal.research_too_low",
            Self::FlowersNotSortedYet { .. } => "building.module.refusal.flowers_not_sorted_yet",
            Self::FlowersNotFilteredAtThisLevel { .. } => {
                "building.module.refusal.flowers_not_filtered_at_this_level"
            }
            Self::Building(inner) => inner.name(),
        }
    }

    /// The module the refusal is about, when it is about one.
    pub fn module(&self) -> Option<ModuleKey> {
        match self {
            Self::ModuleNotOnThisHut { key, .. }
            | Self::LevelTooLow { key, .. }
            | Self::Full { key, .. }
            | Self::NotAnAssignment { key }
            | Self::NotACraftingModule { key }
            | Self::NotCounted { key }
            | Self::ResearchMissing { key, .. } => Some(*key),
            Self::HutUnderConstruction { .. }
            | Self::NotSortable { .. }
            | Self::LevelAboveHutMax { .. }
            | Self::HutResearchTooLow { .. }
            | Self::FlowersNotSortedYet { .. }
            | Self::FlowersNotFilteredAtThisLevel { .. }
            | Self::Building(_) => None,
        }
    }

    /// The same refusal as the level-rule enum, when it is one.
    pub fn building_refusal(&self) -> Option<&crate::building::Refusal> {
        match self {
            Self::Building(inner) => Some(inner),
            _ => None,
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

impl From<crate::building::Refusal> for Refusal {
    fn from(inner: crate::building::Refusal) -> Refusal {
        Refusal::Building(inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::Refusal as BuildingRefusal;

    fn every_refusal() -> Vec<Refusal> {
        vec![
            Refusal::ModuleNotOnThisHut {
                kind: Kind::Stash,
                key: ModuleKey::MinStock,
            },
            Refusal::HutUnderConstruction {
                kind: Kind::Builder,
                level: 0,
            },
            Refusal::ResearchMissing {
                key: ModuleKey::MarksmanTowerWork,
                research: "marksman",
            },
            Refusal::LevelTooLow {
                kind: Kind::Baker,
                key: ModuleKey::BakerCraft,
                level: 2,
                needed: 3,
            },
            Refusal::Full {
                key: ModuleKey::MinStock,
                capacity: 5,
                held: 5,
            },
            Refusal::NotAnAssignment {
                key: ModuleKey::StatsModule,
            },
            Refusal::NotACraftingModule {
                key: ModuleKey::ItemListFuel,
            },
            Refusal::NotCounted {
                key: ModuleKey::StatsModule,
            },
            Refusal::NotSortable {
                kind: Kind::Baker,
            },
            Refusal::LevelAboveHutMax {
                kind: Kind::Tavern,
                level: 4,
                max: 3,
            },
            Refusal::HutResearchTooLow {
                kind: Kind::Library,
                effect: "blockhutlibrary",
                strength: 0.0,
                needed: 1,
            },
            Refusal::FlowersNotSortedYet { level: 3 },
            Refusal::FlowersNotFilteredAtThisLevel { level: 4, limit: 5 },
            Refusal::Building(BuildingRefusal::ModuleDuplicate {
                key: "work".to_owned(),
            }),
        ]
    }

    #[test]
    fn every_refusal_has_its_own_name_and_a_display_that_matches() {
        let all = every_refusal();
        for (index, refusal) in all.iter().enumerate() {
            assert!(
                refusal.name().starts_with("building."),
                "{} is not namespaced",
                refusal.name()
            );
            assert_eq!(refusal.to_string(), refusal.name());
            for other in &all[index + 1..] {
                assert_ne!(refusal.name(), other.name(), "two refusals share a name");
            }
        }
    }

    #[test]
    fn the_wrapped_refusal_keeps_the_name_the_eight_level_rules_have() {
        let wrapped = Refusal::from(BuildingRefusal::Occupied { worker: 3 });
        assert_eq!(wrapped.name(), "building.refusal.occupied");
        assert_eq!(
            wrapped.building_refusal(),
            Some(&BuildingRefusal::Occupied { worker: 3 })
        );
        assert_eq!(Refusal::NotSortable { kind: Kind::Stash }.building_refusal(), None);
    }

    #[test]
    fn a_refusal_about_a_module_says_which_one_and_a_refusal_about_the_hut_does_not() {
        for refusal in every_refusal() {
            if let Some(key) = refusal.module() {
                assert!(!key.key().is_empty());
            }
        }
        assert_eq!(
            Refusal::Full {
                key: ModuleKey::TavernLiving,
                capacity: 4,
                held: 4
            }
            .module(),
            Some(ModuleKey::TavernLiving)
        );
        assert_eq!(Refusal::FlowersNotSortedYet { level: 2 }.module(), None);
    }

    #[test]
    fn the_names_of_the_two_enums_never_collide() {
        // A pass-through must not be able to fake a module refusal either.
        let building_name = BuildingRefusal::ModuleDuplicate {
            key: "x".to_owned(),
        }
        .name();
        for refusal in every_refusal() {
            if refusal.building_refusal().is_none() {
                assert_ne!(refusal.name(), building_name);
            }
        }
    }

    #[test]
    fn the_two_florist_refusals_are_the_two_messages_java_sends() {
        let below = Refusal::FlowersNotSortedYet { level: 3 };
        let above = Refusal::FlowersNotFilteredAtThisLevel { level: 4, limit: 5 };
        assert_ne!(below.name(), above.name());
        assert_ne!(below, above);
        // `TOO_MANY_FILTERED_BELOW_LVL4_FLORIST` names no limit, because at that
        // level the limit is one and the message says "level 4" instead
        // (`FloristFlowerListModuleView.java:56-58`).
        assert_eq!(below.module(), None);
        assert_eq!(above.module(), None);
        // The refusal says which level it happened at, which is what a log needs.
        match above {
            Refusal::FlowersNotFilteredAtThisLevel { level, limit } => {
                assert_eq!((level, limit), (4, 5));
            }
            _ => unreachable!("the variant just built"),
        }
    }
}
