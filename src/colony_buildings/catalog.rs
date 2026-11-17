//! One row per building kind: its top level, its research effect, and **every**
//! module its registry entry registers.
//!
//! Java reference: `apiimp/initializer/ModBuildingsInitializer.java` (691 lines),
//! the one place where a hut is bound to its modules. Each row below quotes the
//! line range of its `register(...)` call, and the module order is Java's order —
//! the order `ModBuildingsInitializer` calls `addBuildingModuleProducer` in, which
//! is also the order `AbstractBuilding.registerModule` numbers them
//! (`core/colony/buildings/AbstractBuilding.java:206-219`).
//!
//! **The counts.** 54 kinds, **279** (kind, module) rows, **156** of the 157 keys
//! in use, and one key nobody uses (`druid_tower_work`, see
//! [`ModuleKey`](super::key::ModuleKey)). A test holds all four numbers.
//!
//! **The top level is per kind, and five are not 5.** `getMaxBuildingLevel` answers
//! `CONST_DEFAULT_MAX_BUILDING_LEVEL = 5`
//! (`api/util/constant/BuildingConstants.java:11`, read at
//! `AbstractBuilding.java:1154-1157`) unless the kind overrides it:
//!
//! | Kind | Top level | Where |
//! | --- | --- | --- |
//! | stash | **0** | `workerbuildings/Stash.java:54-56` |
//! | postbox | **1** | `workerbuildings/PostBox.java:63-66` |
//! | simple quarry, medium quarry | **1** | `ModBuildingsInitializer.java:631`, `:638` |
//! | tavern | **3** | `:521` — `new DefaultBuildingInstance(..., "tavern", 3)` |
//! | the other 48 | 5 | their own `MAX_BUILDING_LEVEL` / `MAX_LEVEL` constant, all 5 |
//!
//! A stash at level 0 is not a mistake: it is a hole in the ground that stores
//! what a builder digs up, and Java refuses it any upgrade because its maximum is
//! 0 (`AbstractBuilding.java:807` compares the level to the maximum).
//!
//! **The research effect is the upgrade gate, and 28 kinds have none.** Java reads
//! `getResearchEffectIdFrom(block)` = `minecolonies:effects/<block path>`
//! (`core/colony/managers/ResearchManager.java:135-140`) and refuses the upgrade
//! when that effect exists but is below the level asked
//! (`AbstractBuilding.java:786-800`). Only **26** kinds have such an effect in the
//! research data; the builder's hut, the farmer's and 26 others do not, so their
//! upgrade is not research-gated at all. A test checks every id against
//! `crate::research::effect`.

use crate::building::Kind;

use super::key::ModuleKey;

/// What one kind of hut is: how far it grows, what unlocks its upgrade, and the
/// modules it carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HutSpec {
    /// The registry kind, which is also the row's identity.
    pub kind: Kind,
    /// `getMaxBuildingLevel()`: 5 unless the kind overrides it.
    pub max_level: i32,
    /// `minecolonies:effects/<block path>`, the effect that gates the upgrade —
    /// `blockhutlibrary` style. `None` when the research data declares no effect
    /// for this hut, and then `requestUpgrade` has nothing to test
    /// (`AbstractBuilding.java:788-800`).
    pub effect: Option<&'static str>,
    /// Every module the kind's entry registers, in Java's registration order.
    pub modules: &'static [ModuleKey],
}

/// The 54 rows, in `Kind::ALL` order — the order every function here returns.
macro_rules! huts {
    ($($kind:ident: $max:literal, $effect:expr => [$($module:ident),* $(,)?];)*) => {
        /// One row per registered building kind.
        pub const HUTS: &[HutSpec] = &[$(HutSpec {
            kind: Kind::$kind,
            max_level: $max,
            effect: $effect,
            modules: &[$(ModuleKey::$module),*],
        }),*];
    };
}

huts! {
    // ModBuildingsInitializer.java:650-659
    Alchemist: 5, Some("blockhutalchemist") => [AlchemistWork, AlchemistCraft, AlchemistBrew, CraftTaskView, StatsModule];
    // :29-38
    Archery: 5, Some("blockhutarchery") => [ArcheryWorkHome, Bed, MinStock, StatsModule];
    // :40-54
    Baker: 5, None => [BakerWork, BakerCraft, CraftTaskView, BakerSmelt, CraftSettings, Furnace, MinStock, ItemListFuel, StatsModule];
    // :56-63
    Barracks: 5, Some("blockhutbarracks") => [MinStock, BarracksStatsModule];
    // :65-81
    BarracksTower: 5, None => [KnightBarracksWork, RangerBarracksWork, DruidBarracksWork, HuscarlBarracksWork, MarksmanBarracksWork, GuardTool, GuardEntityList, GuardSettings, MinStock, Bed, StatsModule];
    // :581-593 — the beekeeper's hut declares no `blockhutbeekeeper` effect in the
    // research data, so its upgrade is not research-gated
    Beekeeper: 5, None => [BeekeeperWork, BeekeeperTool, BeekeeperHerding, BeekeeperSettings, ItemListFlower, MinStock, StatsModule];
    // :83-93
    Blacksmith: 5, Some("blockhutblacksmith") => [BlacksmithWork, BlacksmithCraft, CraftSettings, CraftTaskView, StatsModule];
    // :95-107
    Builder: 5, None => [BuilderWork, BuilderCraft, BuildingResources, BuilderSettings, WorkOrderView, MinStock, StatsModule];
    // :109-119
    ChickenHerder: 5, None => [ChickenHerderWork, ChickenHerderHerding, ChickenHerderSettings, MinStock, StatsModule];
    // :121-130
    CombatAcademy: 5, Some("blockhutcombatacademy") => [KnightTraining, MinStock, Bed, StatsModule];
    // :132-141
    Composter: 5, Some("blockhutcomposter") => [ComposterWork, ComposterSettings, ItemListCompostable, StatsModule];
    // :569-579
    ConcreteMixer: 5, Some("blockhutconcretemixer") => [ConcreteMixerWork, ConcreteMixerCraft, CraftSettings, CraftTaskView, StatsModule];
    // :143-153
    Cook: 5, None => [CookWork, Furnace, ItemListFuel, RestaurantMenu, StatsModule];
    // :155-165
    Cowboy: 5, None => [CowHerderWork, CowHerderHerding, CowHerderSettings, MinStock, StatsModule];
    // :182-192
    Crusher: 5, Some("blockhutcrusher") => [CrusherWork, CrusherCraft, CrusherSettings, CraftTaskView, StatsModule];
    // :194-202
    Deliveryman: 5, None => [CourierWork, CourierTaskView, StatsModule];
    // :491-504
    Dyer: 5, Some("blockhutdyer") => [DyerWork, DyerCraft, DyerSmelt, CraftSettings, Furnace, ItemListFuel, CraftTaskView, StatsModule];
    // :425-435 — no `blockhutenchanter` effect either: the enchanter's hut, like
    // the beekeeper's, is not research-gated
    Enchanter: 5, None => [EnchanterWork, EnchanterCraft, EnchanterStations, MinStock, StatsModule];
    // :204-216
    Farmer: 5, None => [FarmerWork, FarmerCraft, FarmerFields, FarmerSettings, CraftTaskView, MinStock, StatsModule];
    // :218-226
    Fisherman: 5, None => [FisherWork, MinStock, StatsModule];
    // :506-517
    Fletcher: 5, Some("blockhutfletcher") => [FletcherWork, FletcherCraft, FletcherDoCraft, CraftSettings, CraftTaskView, StatsModule];
    // :414-423
    Florist: 5, Some("blockhutflorist") => [FloristWork, FloristItems, MinStock, StatsModule];
    // :676-689
    GateHouse: 5, None => [KnightGateWork, RangerGateWork, GuardEntityList, GateGuardSettings, MinStock, Bed, StatsModule, ColonyConnections];
    // :475-489
    Glassblower: 5, Some("blockhutglassblower") => [GlassblowerWork, GlassblowerCraft, GlassblowerDoCraft, GlassblowerSmelting, CraftSettings, Furnace, ItemListFuel, CraftTaskView, StatsModule];
    // :603-612
    Graveyard: 5, Some("blockhutgraveyard") => [GraveyardWork, Graveyard, MinStock, StatsModule];
    // :228-243
    GuardTower: 5, None => [KnightTowerWork, RangerTowerWork, MarksmanTowerWork, HuscarlTowerWork, GuardTool, GuardEntityList, GuardSettings, MinStock, Bed, StatsModule];
    // :447-455
    Hospital: 5, Some("blockhuthospital") => [HealerWork, MinStock, StatsModule];
    // :661-674
    Kitchen: 5, None => [MinStock, CraftTaskView, ChefWork, ChefCraft, ChefSmelt, Furnace, ItemListFuel, StatsModule];
    // :255-263
    Library: 5, Some("blockhutlibrary") => [StudentWork, MinStock, StatsModule];
    // :265-278
    Lumberjack: 5, None => [ForesterWork, ForesterCraft, ForesterSettings, ForesterTool, ItemListSapling, CraftTaskView, MinStock, StatsModule];
    // :531-542
    Mechanic: 5, Some("blockhutmechanic") => [MechanicWork, MechanicCraft, MechanicDoCraft, CraftSettings, CraftTaskView, StatsModule];
    // :636-641
    MediumQuarry: 1, None => [MediumQuarry];
    // :280-294
    Miner: 5, None => [MinerWork, QuarrierWork, MinerCraft, MinerLevels, MinerSettings, MinerGuardAssign, BuildingResources, MinStock, StatsModule];
    // :595-600
    MysticalSite: 5, Some("blockhutmysticalsite") => [];
    // :614-627
    NetherWorker: 5, Some("blockhutnetherworker") => [NetherWorkerWork, NetherWorkerCraft, NetherWorkerExpedition, NetherWorkerSettings, NetherMinerMenu, CraftTaskView, MinStock, StatsModule];
    // :544-555
    Plantation: 5, Some("blockhutplantation") => [PlantationWork, PlantationCraft, PlantationFields, PlantationSettings, CraftTaskView, StatsModule];
    // :406-412
    Postbox: 1, None => [MinStockPostbox];
    // :557-567
    RabbitHutch: 5, None => [RabbitHerderWork, RabbitHerderHerding, RabbitHerderSettings, MinStock, StatsModule];
    // :245-253 — `new DefaultBuildingInstance(colony, blockPos, "residence", 5)`
    Residence: 5, None => [Home, Living, Bed];
    // :296-307
    Sawmill: 5, Some("blockhutsawmill") => [SawmillWork, SawmillCraft, SawmillDoCraft, CraftSettings, CraftTaskView, StatsModule];
    // :464-473
    School: 5, Some("blockhutschool") => [TeacherWork, PupilWork, MinStock, StatsModule];
    // :309-319
    Shepherd: 5, None => [ShepherdWork, ShepherdHerding, ShepherdSettings, MinStock, StatsModule];
    // :321-330
    Sifter: 5, Some("blockhutsifter") => [SifterWork, SifterCraft, MinStock, StatsModule];
    // :629-634 — `new DefaultBuildingInstance(colony, blockPos, SIMPLE_QUARRY_ID, 1)`
    SimpleQuarry: 1, None => [SimpleQuarry];
    // :332-346
    Smeltery: 5, Some("blockhutsmeltery") => [SmelterWork, SmelterSmelting, SmelterOreBreak, Furnace, ItemListFuel, ItemListOre, MinStock, SmelterSettings, StatsModule];
    // :167-180
    Stable: 5, Some("blockhutstable") => [CavalryStableWork, Bed, StablemasterWork, StablemasterHerding, GuardEntityList, StableSettings, MinStock, StatsModule];
    // :457-462 — no module at all
    Stash: 0, None => [];
    // :348-359
    StoneMason: 5, Some("blockhutstonemason") => [StonemasonWork, StonemasonCraft, StonemasonDoCraft, CraftSettings, CraftTaskView, StatsModule];
    // :361-373
    StoneSmeltery: 5, Some("blockhutstonesmeltery") => [StoneSmelterWork, StoneSmelterSmelting, CraftSettings, Furnace, ItemListFuel, CraftTaskView, StatsModule];
    // :375-385
    SwineHerder: 5, None => [SwineHerderWork, SwineHerderHerding, SwineHerderSettings, MinStock, StatsModule];
    // :519-529 — `new DefaultBuildingInstance(colony, blockPos, "tavern", 3)`
    Tavern: 3, None => [TavernLiving, TavernVisitor, Bed, StatsModule, TavernSettings];
    // :387-393
    TownHall: 5, None => [TownhallSettings];
    // :437-445
    University: 5, None => [UniversityWork, UniversityResearch, StatsModule];
    // :395-404
    Warehouse: 5, None => [WarehouseCouriers, WarehouseOptions, MinStock, WarehouseRequestQueue];
}

/// The row of a kind. Total by construction: [`HUTS`] holds one row per variant
/// of [`Kind`], and a test checks the bijection.
pub fn spec_of(kind: Kind) -> &'static HutSpec {
    HUTS
        .iter()
        .find(|hut| hut.kind == kind)
        .expect("HUTS holds one row per Kind, checked by a test")
}

/// The modules of a kind, in Java's registration order.
pub fn modules_of(kind: Kind) -> &'static [ModuleKey] {
    spec_of(kind).modules
}

/// Whether the kind's entry registers this module at all — the question Java
/// answers by `hasModule(producer)` (`AbstractBuilding.java:190-194`).
pub fn accepts(kind: Kind, key: ModuleKey) -> bool {
    modules_of(kind).contains(&key)
}

/// Every kind that carries a given module, in `Kind::ALL` order.
pub fn kinds_with(key: ModuleKey) -> impl Iterator<Item = Kind> {
    HUTS.iter()
        .filter(move |hut| hut.modules.contains(&key))
        .map(|hut| hut.kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn there_is_one_row_per_kind_and_the_order_is_the_algorithmic_one() {
        assert_eq!(HUTS.len(), 54);
        let kinds: Vec<Kind> = HUTS.iter().map(|hut| hut.kind).collect();
        assert_eq!(kinds, Kind::ALL.to_vec(), "HUTS keeps Kind::ALL's order");
        for kind in Kind::ALL {
            assert_eq!(spec_of(*kind).kind, *kind);
        }
    }

    #[test]
    fn the_two_hundred_and_seventy_nine_rows_are_all_distinct_within_a_hut() {
        let total: usize = HUTS.iter().map(|hut| hut.modules.len()).sum();
        assert_eq!(total, 279, "the number of addBuildingModuleProducer calls");
        for hut in HUTS {
            let unique: BTreeSet<ModuleKey> = hut.modules.iter().copied().collect();
            assert_eq!(
                unique.len(),
                hut.modules.len(),
                "{} registers a module twice, which Java refuses (AbstractBuilding.java:210-216)",
                hut.kind
            );
        }
    }

    #[test]
    fn a_hundred_and_fifty_six_keys_are_in_use_and_the_druid_tower_key_is_not() {
        let used: BTreeSet<&str> = HUTS
            .iter()
            .flat_map(|hut| hut.modules.iter().map(|key| key.key()))
            .collect();
        assert_eq!(used.len(), 156, "157 keys, one of which no hut registers");
        assert!(!used.contains("druid_tower_work"));
        assert!(used.contains("druid_barracks_work"));
        assert!(used.contains("min_stock"));
        assert!(used.contains("colony_connections"));
    }

    #[test]
    fn six_kinds_do_not_go_to_five_and_the_other_forty_eight_do() {
        let mut odd: Vec<(&str, i32)> = HUTS
            .iter()
            .filter(|hut| hut.max_level != 5)
            .map(|hut| (hut.kind.name(), hut.max_level))
            .collect();
        odd.sort_unstable();
        assert_eq!(
            odd,
            vec![
                ("mediumquarry", 1),
                ("postbox", 1),
                ("simplequarry", 1),
                ("stash", 0),
                ("tavern", 3),
            ],
            "five kinds, listed with the line that states each"
        );
        for hut in HUTS {
            assert!(hut.max_level <= 5, "{} goes above the top", hut.kind);
            assert!(hut.max_level >= 0);
        }
    }

    #[test]
    fn an_unbuilt_stash_is_a_hole_and_can_never_grow() {
        // `AbstractBuilding.java:807` compares the level to the maximum, so a
        // maximum of 0 refuses every upgrade and the hut never leaves level 0.
        assert_eq!(spec_of(Kind::Stash).max_level, 0);
        assert_eq!(spec_of(Kind::Stash).modules.len(), 0);
    }

    #[test]
    fn twenty_six_kinds_state_a_research_effect_and_every_one_is_declared() {
        let gated: Vec<&str> = HUTS.iter().filter_map(|hut| hut.effect).collect();
        assert_eq!(gated.len(), 26);
        for effect in &gated {
            assert!(
                crate::research::effect::category(effect).is_some(),
                "{effect} is stated by a hut but the research data does not declare it"
            );
            assert!(effect.starts_with("blockhut"), "{effect} is not a hut effect");
        }
        // The three the brief names, and the four exceptions that matter: 26 of
        // the 54 kinds are gated, so the majority is not.
        assert_eq!(spec_of(Kind::Library).effect, Some("blockhutlibrary"));
        assert_eq!(spec_of(Kind::Builder).effect, None);
        assert_eq!(spec_of(Kind::TownHall).effect, None);
        assert_eq!(spec_of(Kind::Farmer).effect, None);
        // No `blockhutbeekeeper` and no `blockhutenchanter` exist in the data.
        assert_eq!(spec_of(Kind::Beekeeper).effect, None);
        assert_eq!(spec_of(Kind::Enchanter).effect, None);
    }

    #[test]
    fn every_worker_row_of_the_jobs_table_is_a_module_its_hut_registers() {
        // The cross-check that keeps `catalog` and `jobs::employment` from
        // drifting: employment says which job a module hires, the catalog says
        // the hut has the module. Neither is allowed to know a row the other does
        // not.
        for row in crate::jobs::EMPLOYMENTS {
            let key = ModuleKey::from_key(row.module)
                .unwrap_or_else(|| panic!("{} is not a module key", row.module));
            assert!(
                accepts(row.hut, key),
                "{} hires through {}, which the catalog does not give it",
                row.hut,
                row.module
            );
        }
        // And the other way: every `*_work` key a hut carries is in employment,
        // except the three that are not worker modules.
        for hut in HUTS {
            for key in hut.modules {
                if key.hires() && key.key().ends_with("_work") {
                    assert!(
                        crate::jobs::EMPLOYMENTS
                            .iter()
                            .any(|row| row.hut == hut.kind && row.module == key.key()),
                        "{} carries {}, which employment does not list",
                        hut.kind,
                        key.key()
                    );
                }
            }
        }
    }

    #[test]
    fn the_seven_huts_that_employ_nobody_carry_no_worker_module() {
        for kind in [
            Kind::Barracks,
            Kind::Residence,
            Kind::TownHall,
            Kind::Postbox,
            Kind::Stash,
            Kind::Tavern,
            Kind::MysticalSite,
        ] {
            assert!(
                crate::jobs::employees(kind).is_err(),
                "{kind} employs somebody after all"
            );
            for key in modules_of(kind) {
                assert!(
                    !crate::jobs::EMPLOYMENTS
                        .iter()
                        .any(|row| row.hut == kind && row.module == key.key()),
                    "{kind} carries a worker module"
                );
            }
        }
    }

    #[test]
    fn a_hut_carries_what_it_carries_and_says_so() {
        assert!(accepts(Kind::Builder, ModuleKey::WorkOrderView));
        assert!(!accepts(Kind::Baker, ModuleKey::WorkOrderView));
        assert!(accepts(Kind::Baker, ModuleKey::Furnace));
        assert!(!accepts(Kind::Blacksmith, ModuleKey::Furnace), "no furnace in the smithy");
        assert!(accepts(Kind::Tavern, ModuleKey::TavernLiving));
        assert!(!accepts(Kind::Residence, ModuleKey::TavernLiving));
        // The two huts that carry nothing at all.
        assert_eq!(modules_of(Kind::Stash).len(), 0);
        assert_eq!(modules_of(Kind::MysticalSite).len(), 0);
        assert_eq!(modules_of(Kind::Postbox), &[ModuleKey::MinStockPostbox]);
        assert_eq!(modules_of(Kind::TownHall), &[ModuleKey::TownhallSettings]);
    }

    #[test]
    fn the_three_global_modules_sit_on_the_most_huts() {
        let mut counts: Vec<(usize, &str)> = [ModuleKey::MinStock, ModuleKey::StatsModule, ModuleKey::Bed]
            .iter()
            .map(|key| (kinds_with(*key).count(), key.key()))
            .collect();
        counts.sort_unstable();
        assert_eq!(
            counts,
            vec![
                (8, "bed"),
                (30, "min_stock"),
                (45, "stats_module"),
            ],
            "min_stock is on 30 huts (the postbox has its own key), stats on 45, a bed on 8"
        );
        // Every hut without a statistics panel, named: the nine Java gives none.
        let without: Vec<&str> = Kind::ALL
            .iter()
            .filter(|kind| !accepts(**kind, ModuleKey::StatsModule))
            .map(|kind| kind.name())
            .collect();
        assert_eq!(
            without,
            vec![
                "barracks",
                "mediumquarry",
                "mysticalsite",
                "postbox",
                "residence",
                "simplequarry",
                "stash",
                "townhall",
                "warehouse",
            ],
            "the barracks has a `barracks_stats_module` of its own, and the other eight have none"
        );
    }

    #[test]
    fn the_farmers_two_modules_are_not_the_same_module() {
        // `farmer_work` holds recipes, `farmer_craft` hires the farmer: the pair
        // that makes a name-based reading of the table wrong.
        assert!(accepts(Kind::Farmer, ModuleKey::FarmerWork));
        assert!(accepts(Kind::Farmer, ModuleKey::FarmerCraft));
        assert_eq!(
            super::super::capacity::capacity(ModuleKey::FarmerWork),
            super::super::Capacity::ManyRecipes
        );
        assert_eq!(
            super::super::capacity::capacity(ModuleKey::FarmerCraft),
            super::super::Capacity::Uncounted
        );
    }

    #[test]
    fn the_two_towers_do_not_carry_the_same_guard_types() {
        // The barracks tower lists all five (`ModBuildingsInitializer.java:70-74`),
        // the guard tower four (`:233-236`), and the missing one is the druid.
        let barracks: Vec<&str> = modules_of(Kind::BarracksTower)
            .iter()
            .filter(|key| key.key().ends_with("_barracks_work"))
            .map(|key| key.key())
            .collect();
        assert_eq!(
            barracks,
            vec![
                "knight_barracks_work",
                "ranger_barracks_work",
                "druid_barracks_work",
                "huscarl_barracks_work",
                "marksman_barracks_work",
            ]
        );
        let tower: Vec<&str> = modules_of(Kind::GuardTower)
            .iter()
            .filter(|key| key.key().ends_with("_tower_work"))
            .map(|key| key.key())
            .collect();
        assert_eq!(
            tower,
            vec![
                "knight_tower_work",
                "ranger_tower_work",
                "marksman_tower_work",
                "huscarl_tower_work",
            ],
            "no druid: the key exists and no entry adds it"
        );
    }

    #[test]
    fn kinds_with_walks_the_table_and_answers_the_huts_that_carry_the_key() {
        let carriers: Vec<Kind> = kinds_with(ModuleKey::DruidBarracksWork).collect();
        assert_eq!(carriers, vec![Kind::BarracksTower]);
        assert_eq!(
            kinds_with(ModuleKey::DruidTowerWork).count(),
            0,
            "the one key nobody carries"
        );
        // The two global modules, as a count, from the other direction.
        assert_eq!(kinds_with(ModuleKey::MinStockPostbox).count(), 1);
        assert_eq!(kinds_with(ModuleKey::MinStock).count(), 30);
        for kind in Kind::ALL {
            for key in modules_of(*kind) {
                assert!(
                    kinds_with(*key).any(|carrier| carrier == *kind),
                    "{} carries {key} but kinds_with does not say so",
                    kind
                );
            }
        }
    }

    #[test]
    fn every_hut_states_a_top_level_inside_the_range_of_the_record() {
        // `crate::building::Building::set_level` refuses above 5 and below 0, so a
        // table row outside that range would be a hut the record can never reach.
        for hut in HUTS {
            assert!(
                (0..=crate::building::MAX_BUILDING_LEVEL).contains(&hut.max_level),
                "{} states a top level of {}",
                hut.kind,
                hut.max_level
            );
        }
        assert_eq!(
            crate::building::MAX_BUILDING_LEVEL,
            HUTS.iter().map(|hut| hut.max_level).max().unwrap_or(0),
            "the highest top level any kind states is the record's maximum"
        );
    }
}
