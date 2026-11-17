//! The 157 module keys Java declares, and the handful that carry a rule.
//!
//! Java reference: `core/colony/buildings/modules/BuildingModules.java` (635
//! lines), a flat list of `BuildingEntry.ModuleProducer`s. The **key** is what a
//! building saves its module under (`api/colony/buildings/registry/BuildingEntry.java:204-220`,
//! the `public final String key`), never the runtime id, which is a counter that
//! changes between builds (`:216`, `id = ++runtimeIdGenerator`).
//!
//! **157 keys, and one of them is dead.** `druid_tower_work` is declared at
//! `BuildingModules.java:573-574` and **no building entry adds it**: the guard
//! tower lists knight, ranger, marksman and huscarl (`ModBuildingsInitializer.java:233-236`)
//! and the barracks tower lists the druid (`:70-73`), so a guard tower never
//! hires a druid. The port keeps the key, because a saved building may carry it,
//! and [`ModuleKey::druid_tower_work_is_reachable`] is the test that says so.
//!
//! **The table is one row per key and three functions read it** — [`ModuleKey::key`],
//! [`ModuleKey::from_key`] and [`ModuleKey::ALL`] — expanded from one macro for
//! the reason `crate::building::kind` does the same: three hand-written tables
//! can disagree, one cannot.
//!
//! **Keys are not class names, and three disagree on purpose.**
//!
//! | Producer constant | Key | Class |
//! | --- | --- | --- |
//! | `COOK_WORK` | `cook_craft` | `NoPrivateCrafterWorkerModule` (`:447-449`) |
//! | `FORESTER_TOOL` | `forester_zone` | a view-only scepter page (`:478-479`) |
//! | `GUARD_TOOL` | `tool_scepterguard_view` | the same, for the guard (`:587-589`) |
//!
//! A fourth is spelled by Java with a typo: `sheperd_work`, `sheperd_settings`,
//! `sheperd_herding` (`:226-235`). The port keeps the spelling, because the key
//! is what a saved compound holds.

use std::fmt;

/// How many keys `BuildingModules.java` declares: **157** producers, in 26
/// commented groups, from `min_stock` to `colony_connections`.
pub const MODULE_KEY_COUNT: usize = 157;

macro_rules! module_keys {
    ($($variant:ident => $key:literal,)*) => {
        /// One `ModuleProducer` of `BuildingModules.java`, named by its key.
        ///
        /// The variant names follow the Java constant where there is one
        /// (`MIN_STOCK` → [`ModuleKey::MinStock`]) and the key where the constant
        /// would be misleading (`COOK_WORK` → [`ModuleKey::CookWork`], whose key
        /// is `cook_craft`).
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        // exhaustive: the producer table is a closed list in Java
        pub enum ModuleKey {
            $($variant,)*
        }

        impl ModuleKey {
            /// Every key, in `BuildingModules.java` declaration order — which is
            /// the order the file groups them in: global, item lists, workers,
            /// horticulture, husbandry, craftmanship, storage, education,
            /// fundamentals, military, mystic, connection.
            pub const ALL: &'static [ModuleKey] = &[$(ModuleKey::$variant,)*];

            /// The serialisation key, exactly as Java spells it.
            pub fn key(self) -> &'static str {
                match self {
                    $(ModuleKey::$variant => $key,)*
                }
            }

            /// The key a saved compound names, or `None`. Case matters: the
            /// engine's charset is lower case only
            /// (`horizon-core/src/identifier.rs:65-76`), and every key here is.
            pub fn from_key(text: &str) -> Option<ModuleKey> {
                match text {
                    $($key => Some(ModuleKey::$variant),)*
                    _ => None,
                }
            }
        }
    };
}

module_keys! {
    // The global modules, BuildingModules.java:43-68
    MinStock => "min_stock",
    MinStockPostbox => "min_stock_postbox",
    Bed => "bed",
    Furnace => "furnace",
    CraftTaskView => "craft_task_view",
    CourierTaskView => "courier_task_view",
    CraftSettings => "craft_settings",
    GuardEntityList => "guard_entity_list",
    StatsModule => "stats_module",
    BarracksStatsModule => "barracks_stats_module",
    // Item lists, :73-104
    ItemListFuel => "itemlist_fuel",
    ItemListCompostable => "itemlist_compostable",
    RestaurantMenu => "restaurant_menu",
    NetherMinerMenu => "netherminer_menu",
    ItemListSapling => "itemlist_sapling",
    ItemListOre => "itemlist_ore",
    ItemListFlower => "itemlist_flower",
    // Horticulture, :113-150
    ComposterWork => "composter_work",
    ComposterSettings => "composter_settings",
    FarmerCraft => "farmer_craft",
    FarmerWork => "farmer_work",
    FarmerFields => "farmer_fields",
    FarmerSettings => "farmer_settings",
    PlantationWork => "plantation_work",
    PlantationCraft => "plantation_craft",
    PlantationFields => "plantation_fields",
    PlantationSettings => "plantation_settings",
    FloristWork => "florist_work",
    FloristItems => "florist_items",
    // Husbandry, :156-245
    BeekeeperWork => "beekeeper_work",
    BeekeeperSettings => "beekeeper_settings",
    BeekeeperTool => "beekeeper_tool",
    BeekeeperHerding => "beekeeper_herding",
    ChickenHerderWork => "chickenherder_work",
    ChickenHerderHerding => "chickenherder_herding",
    ChickenHerderSettings => "chickenherder_breeding_settings",
    CowHerderWork => "cowherder_work",
    CowHerderSettings => "cowherder_settings",
    CowHerderHerding => "cowherder_herding",
    StablemasterWork => "stablemaster_work",
    StablemasterHerding => "stablemaster_herding",
    StableSettings => "stable_settings",
    FisherWork => "fisher_work",
    RabbitHerderWork => "rabbitherder_work",
    RabbitHerderSettings => "rabbitherder_settings",
    RabbitHerderHerding => "rabbitherder_herding",
    ShepherdWork => "sheperd_work",
    ShepherdSettings => "sheperd_settings",
    ShepherdHerding => "sheperd_herding",
    SwineHerderWork => "swineherder_work",
    SwineHerderSettings => "swineherder_settings",
    SwineHerderHerding => "swineherder_herding",
    // Craftmanship, :251-384
    AlchemistWork => "alchemist_work",
    AlchemistCraft => "alchemist_craft",
    AlchemistBrew => "alchemist_brew",
    BakerCraft => "baker_craft",
    BakerSmelt => "baker_smelt",
    BakerWork => "baker_work",
    BlacksmithWork => "blacksmith_work",
    BlacksmithCraft => "blacksmith_craft",
    ConcreteMixerWork => "concretemixer_work",
    ConcreteMixerCraft => "concretemixer_craft",
    CrusherWork => "crusher_work",
    CrusherCraft => "crusher_craft",
    CrusherSettings => "crusher_settings",
    DyerWork => "dyer_work",
    DyerCraft => "dyer_craft",
    DyerSmelt => "dyer_smelt",
    FletcherWork => "fletcher_work",
    FletcherCraft => "fletcher_craft",
    FletcherDoCraft => "fletcher_do_craft",
    GlassblowerWork => "glassblower_work",
    GlassblowerCraft => "glassblower_craft",
    GlassblowerSmelting => "glassblower_smelting",
    GlassblowerDoCraft => "glassblower_do_craft",
    MechanicWork => "mechanic_work",
    MechanicCraft => "mechanic_craft",
    MechanicDoCraft => "mechanic_do_craft",
    SawmillWork => "sawmill_work",
    SawmillCraft => "sawmill_craft",
    SawmillDoCraft => "sawmill_do_craft",
    SifterWork => "sifter_work",
    SifterCraft => "sifter_craft",
    SmelterWork => "smelter_work",
    SmelterSmelting => "smelter_smelting",
    SmelterOreBreak => "smelter_orebreak",
    SmelterSettings => "smelter_settings",
    StonemasonWork => "stonemason_work",
    StonemasonCraft => "stonemason_craft",
    StonemasonDoCraft => "stonemason_do_craft",
    StoneSmelterWork => "stonesmelter_work",
    StoneSmelterSmelting => "stonesmelter_smelting",
    // Storage, :390-399
    CourierWork => "courier_work",
    WarehouseCouriers => "warehouse_couriers",
    WarehouseOptions => "warehouse_options",
    WarehouseRequestQueue => "warehouse_request_queue",
    // Education, :405-424
    PupilWork => "pupil_work",
    StudentWork => "student_work",
    TeacherWork => "teacher_work",
    UniversityWork => "university_work",
    UniversityResearch => "university_research",
    // Fundamentals, :430-506
    BuilderWork => "builder_work",
    BuilderSettings => "builder_settings",
    BuilderCraft => "builder_craft",
    WorkOrderView => "workorderlist",
    BuildingResources => "buildingresources",
    CookWork => "cook_craft",
    ChefWork => "chef_work",
    ChefCraft => "chef_craft",
    ChefSmelt => "chef_smelt",
    ForesterCraft => "forester_craft",
    ForesterWork => "forester_work",
    ForesterSettings => "forester_settings",
    ForesterTool => "forester_zone",
    HealerWork => "healer_work",
    Home => "home",
    Living => "living",
    MinerWork => "miner_work",
    MinerCraft => "miner_craft",
    MinerLevels => "miner_levels",
    MinerSettings => "miner_settings",
    MinerGuardAssign => "miner_guard_assign",
    QuarrierWork => "quarrier_work",
    SimpleQuarry => "simple_quarry",
    MediumQuarry => "medium_quarry",
    TavernLiving => "tavern_living",
    TavernVisitor => "tavern_visitor",
    TavernSettings => "tavern_settings",
    TownhallSettings => "townhall_settings",
    // Military, :538-600
    ArcheryWorkHome => "archery_work",
    KnightTraining => "knight_training",
    KnightBarracksWork => "knight_barracks_work",
    RangerBarracksWork => "ranger_barracks_work",
    DruidBarracksWork => "druid_barracks_work",
    MarksmanBarracksWork => "marksman_barracks_work",
    HuscarlBarracksWork => "huscarl_barracks_work",
    CavalryStableWork => "cavalry_stable_work",
    KnightTowerWork => "knight_tower_work",
    RangerTowerWork => "ranger_tower_work",
    DruidTowerWork => "druid_tower_work",
    MarksmanTowerWork => "marksman_tower_work",
    HuscarlTowerWork => "huscarl_tower_work",
    KnightGateWork => "knight_gate_work",
    RangerGateWork => "ranger_gate_work",
    GuardTool => "tool_scepterguard_view",
    GuardSettings => "guard_settings",
    GateGuardSettings => "gate_guard_settings",
    // Mystic, :606-631
    EnchanterWork => "enchanter_work",
    EnchanterCraft => "enchanter_craft",
    EnchanterStations => "enchanter_stations",
    Graveyard => "graveyard",
    GraveyardWork => "graveyard_work",
    NetherWorkerWork => "netherworker_work",
    NetherWorkerCraft => "netherworker_craft",
    NetherWorkerSettings => "netherworker_settings",
    NetherWorkerExpedition => "netherworker_expedition",
    // The one key of another kind, :633-634
    ColonyConnections => "colony_connections",
}

/// What a module holds, as far as the *key* can say it. Anything finer needs the
/// hut (a `farmer_work` holds 2^level recipes wherever it sits, but a
/// `warehouse_couriers` only exists on a warehouse).
impl ModuleKey {
    /// Whether this key hires citizens, so that the count lives in the worker
    /// table (`crate::jobs::employment`) rather than here.
    ///
    /// True for the 58 worker modules plus the modules that assign without
    /// declaring skills: the residence's `living`, the tavern's
    /// `tavern_living`, the warehouse's `warehouse_couriers` and the two quarry
    /// modules (`QuarryModule extends AbstractAssignedCitizenModule`,
    /// `modules/QuarryModule.java:28`).
    pub fn hires(self) -> bool {
        crate::jobs::EMPLOYMENTS
            .iter()
            .any(|row| row.module == self.key())
            || matches!(
                self,
                ModuleKey::Living
                    | ModuleKey::Home
                    | ModuleKey::TavernLiving
                    | ModuleKey::SimpleQuarry
                    | ModuleKey::MediumQuarry
            )
    }

    /// Whether this key is one of the 33 modules that hold a recipe list, and so
    /// answer `getMaxRecipes() = 2^level` (`AbstractCraftingBuildingModule.java:171-179`).
    ///
    /// The class, not the name, is the test: every one of these extends
    /// `AbstractCraftingBuildingModule` (as `Crafting`, `Custom`, `Smelting`,
    /// `Brewing`, or through `SimpleCraftingModule` /
    /// `AbstractDOCraftingBuildingModule`), and `farmer_work` is a crafting
    /// module while `farmer_craft` is the worker module — the pair that makes a
    /// name-based rule wrong.
    pub fn crafts(self) -> bool {
        matches!(
            self,
            ModuleKey::AlchemistCraft
                | ModuleKey::AlchemistBrew
                | ModuleKey::BakerCraft
                | ModuleKey::BakerSmelt
                | ModuleKey::BlacksmithCraft
                | ModuleKey::BuilderCraft
                | ModuleKey::ChefCraft
                | ModuleKey::ChefSmelt
                | ModuleKey::ConcreteMixerCraft
                | ModuleKey::CrusherCraft
                | ModuleKey::DyerCraft
                | ModuleKey::DyerSmelt
                | ModuleKey::EnchanterCraft
                | ModuleKey::FarmerWork
                | ModuleKey::FletcherCraft
                | ModuleKey::FletcherDoCraft
                | ModuleKey::ForesterCraft
                | ModuleKey::GlassblowerCraft
                | ModuleKey::GlassblowerSmelting
                | ModuleKey::GlassblowerDoCraft
                | ModuleKey::MechanicCraft
                | ModuleKey::MechanicDoCraft
                | ModuleKey::MinerCraft
                | ModuleKey::NetherWorkerCraft
                | ModuleKey::PlantationCraft
                | ModuleKey::SawmillCraft
                | ModuleKey::SawmillDoCraft
                | ModuleKey::SifterCraft
                | ModuleKey::SmelterSmelting
                | ModuleKey::SmelterOreBreak
                | ModuleKey::StonemasonCraft
                | ModuleKey::StonemasonDoCraft
                | ModuleKey::StoneSmelterSmelting
        )
    }

    /// Whether the producer has **no server half**: Java passes `null` as the
    /// module supplier and only a view (`BuildingModules.java:52-53`, `:164-165`,
    /// `:423-424`, `:442-443`, `:478-479`, `:505-506`, `:587-589`).
    ///
    /// `BuildingEntry.produceBuilding` skips those (`BuildingEntry.java:150-155`),
    /// so they exist on the client only and can hold nothing.
    pub fn is_view_only(self) -> bool {
        matches!(
            self,
            ModuleKey::CraftTaskView
                | ModuleKey::CourierTaskView
                | ModuleKey::WorkOrderView
                | ModuleKey::UniversityResearch
                | ModuleKey::MinerGuardAssign
                | ModuleKey::BeekeeperTool
                | ModuleKey::ForesterTool
                | ModuleKey::GuardTool
        )
    }

    /// Whether the module is a list of settings, whose entries carry the ids a
    /// compound saves under `settings` (see the table in `MANQUANT.md`: the 25
    /// setting keys are declared next to the buildings, not in this module).
    pub fn is_settings(self) -> bool {
        matches!(
            self,
            ModuleKey::CraftSettings
                | ModuleKey::ComposterSettings
                | ModuleKey::FarmerSettings
                | ModuleKey::PlantationSettings
                | ModuleKey::BeekeeperSettings
                | ModuleKey::ChickenHerderSettings
                | ModuleKey::CowHerderSettings
                | ModuleKey::StableSettings
                | ModuleKey::RabbitHerderSettings
                | ModuleKey::ShepherdSettings
                | ModuleKey::SwineHerderSettings
                | ModuleKey::CrusherSettings
                | ModuleKey::SmelterSettings
                | ModuleKey::BuilderSettings
                | ModuleKey::TavernSettings
                | ModuleKey::TownhallSettings
                | ModuleKey::GuardSettings
                | ModuleKey::GateGuardSettings
                | ModuleKey::MinerSettings
                | ModuleKey::ForesterSettings
                | ModuleKey::NetherWorkerSettings
        )
    }

    /// Whether the module keeps a list of items a worker may or may not use.
    pub fn is_item_list(self) -> bool {
        matches!(
            self,
            ModuleKey::ItemListFuel
                | ModuleKey::ItemListCompostable
                | ModuleKey::ItemListSapling
                | ModuleKey::ItemListOre
                | ModuleKey::ItemListFlower
                | ModuleKey::FloristItems
        )
    }

    /// Whether the module only counts: a statistics panel.
    pub fn is_statistics(self) -> bool {
        matches!(
            self,
            ModuleKey::StatsModule | ModuleKey::BarracksStatsModule
        )
    }
}

impl fmt::Display for ModuleKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn the_table_holds_the_hundred_and_fifty_seven_keys_java_declares() {
        assert_eq!(ModuleKey::ALL.len(), MODULE_KEY_COUNT);
        let unique: BTreeSet<&str> = ModuleKey::ALL.iter().map(|key| key.key()).collect();
        assert_eq!(unique.len(), MODULE_KEY_COUNT, "two keys share a name");
    }

    #[test]
    fn every_key_names_itself_and_comes_back_from_its_name() {
        for key in ModuleKey::ALL {
            assert_eq!(ModuleKey::from_key(key.key()), Some(*key));
            assert!(!key.key().is_empty());
            assert!(
                key.key()
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                "{} is not a lower-case identifier, so the engine would refuse it",
                key.key()
            );
            assert_eq!(key.to_string(), key.key());
        }
        assert_eq!(ModuleKey::from_key("Min_Stock"), None, "the charset is lower case");
        assert_eq!(ModuleKey::from_key(""), None);
        assert_eq!(ModuleKey::from_key("stash"), None, "no such key");
    }

    #[test]
    fn the_keys_that_disagree_with_their_producer_constant_keep_their_key() {
        assert_eq!(ModuleKey::CookWork.key(), "cook_craft");
        assert_eq!(ModuleKey::ForesterTool.key(), "forester_zone");
        assert_eq!(ModuleKey::GuardTool.key(), "tool_scepterguard_view");
        // Java's own typo, kept because a saved compound holds it.
        assert_eq!(ModuleKey::ShepherdWork.key(), "sheperd_work");
        assert_eq!(ModuleKey::ShepherdSettings.key(), "sheperd_settings");
        assert_eq!(ModuleKey::ShepherdHerding.key(), "sheperd_herding");
    }

    #[test]
    fn thirty_three_keys_hold_recipes_and_the_farmers_pair_is_not_confused() {
        let crafting: Vec<&str> = ModuleKey::ALL
            .iter()
            .filter(|key| key.crafts())
            .map(|key| key.key())
            .collect();
        assert_eq!(crafting.len(), 33);
        assert!(ModuleKey::FarmerWork.crafts(), "farmer_work is the crafting module");
        assert!(!ModuleKey::FarmerCraft.crafts(), "farmer_craft hires the farmer");
        // The four DO modules are crafting modules too, and every *craft key
        // except the two work modules is in the list.
        for key in [
            ModuleKey::FletcherDoCraft,
            ModuleKey::GlassblowerDoCraft,
            ModuleKey::MechanicDoCraft,
            ModuleKey::SawmillDoCraft,
            ModuleKey::StonemasonDoCraft,
        ] {
            assert!(key.crafts(), "{key}");
        }
        assert!(!ModuleKey::FisherWork.crafts());
        assert!(!ModuleKey::StatsModule.crafts());
    }

    #[test]
    fn eight_keys_have_no_server_half() {
        let mut views: Vec<&str> = ModuleKey::ALL
            .iter()
            .filter(|key| key.is_view_only())
            .map(|key| key.key())
            .collect();
        views.sort_unstable();
        assert_eq!(
            views,
            vec![
                "beekeeper_tool",
                // `courier_task_view` sorts before `craft_task_view`: `cou` < `cra`.
                "courier_task_view",
                "craft_task_view",
                "forester_zone",
                "miner_guard_assign",
                "tool_scepterguard_view",
                "university_research",
                "workorderlist",
            ]
        );
        for key in ModuleKey::ALL.iter().filter(|key| key.is_view_only()) {
            assert_eq!(crate::colony_buildings::capacity::capacity(*key), crate::colony_buildings::Capacity::Uncounted);
        }
    }

    #[test]
    fn the_settings_and_item_lists_do_not_overlap() {
        for key in ModuleKey::ALL {
            assert!(
                !(key.is_settings() && key.is_item_list()),
                "{key} is both a settings page and an item list"
            );
            assert!(!(key.is_settings() && key.crafts()));
            assert!(!(key.is_statistics() && key.hires()));
        }
        let settings = ModuleKey::ALL.iter().filter(|key| key.is_settings()).count();
        assert_eq!(settings, 21, "the 21 settings pages BuildingModules declares");
        let lists = ModuleKey::ALL.iter().filter(|key| key.is_item_list()).count();
        assert_eq!(lists, 6);
    }

    #[test]
    fn the_druid_tower_work_key_is_declared_and_no_hut_registers_it() {
        // BuildingModules.java:573-574 declares it; the guard tower's entry
        // (ModBuildingsInitializer.java:233-236) lists four other guard types.
        assert_eq!(ModuleKey::DruidTowerWork.key(), "druid_tower_work");
        assert!(
            !crate::colony_buildings::catalog::HUTS
                .iter()
                .any(|hut| hut.modules.contains(&ModuleKey::DruidTowerWork)),
            "a hut registers the druid's tower module after all"
        );
        // Its barracks sibling *is* registered, which is the asymmetry itself.
        assert!(
            crate::colony_buildings::catalog::HUTS
                .iter()
                .any(|hut| hut.modules.contains(&ModuleKey::DruidBarracksWork))
        );
    }

    #[test]
    fn the_fifty_eight_worker_keys_all_hire_somebody() {
        for row in crate::jobs::EMPLOYMENTS {
            let key = ModuleKey::from_key(row.module)
                .unwrap_or_else(|| panic!("{} is a worker module the key table does not hold", row.module));
            assert!(key.hires(), "{key} is a worker module but does not say it hires");
        }
        // ... and the five keys that assign citizens without declaring a skill
        // (two of them are worker rows already).
        for key in [
            ModuleKey::Living,
            ModuleKey::Home,
            ModuleKey::TavernLiving,
            ModuleKey::SimpleQuarry,
            ModuleKey::MediumQuarry,
        ] {
            assert!(key.hires(), "{key}");
        }
    }

    #[test]
    fn a_key_that_hires_is_a_worker_row_or_one_of_the_three_homes() {
        // The converse of the test above, and the reason the two tables can be
        // checked against each other: nothing else in the 157 claims to hire.
        // The two quarry keys are worker rows already, so the union is 58 + 3.
        let hirers: Vec<&str> = ModuleKey::ALL
            .iter()
            .filter(|key| key.hires())
            .map(|key| key.key())
            .collect();
        assert_eq!(hirers.len(), 61);
        for name in &hirers {
            let in_employment = crate::jobs::EMPLOYMENTS.iter().any(|row| row.module == *name);
            let one_of_three = matches!(*name, "living" | "home" | "tavern_living");
            assert!(
                in_employment || one_of_three,
                "{name} claims to hire with no row behind it"
            );
        }
    }

    #[test]
    fn a_settings_page_never_hires_never_crafts_and_never_lists_items() {
        for key in ModuleKey::ALL {
            if key.is_settings() {
                assert!(!key.hires(), "{key}");
                assert!(!key.is_view_only(), "{key}");
                assert!(!key.is_item_list(), "{key}");
            }
            if key.is_statistics() {
                assert!(!key.hires() && !key.crafts());
            }
        }
        // And the one settings key that is spelled like a crafting module:
        // `craft_settings` configures a crafter, it is not one.
        assert!(ModuleKey::CraftSettings.is_settings());
        assert!(!ModuleKey::CraftSettings.crafts());
    }
}
