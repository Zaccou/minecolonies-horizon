//! What a module holds at a given building level, and what research does to it.
//!
//! Java reference: the module classes of `core/colony/buildings/modules/`. Every
//! count is a lambda or a one-line formula attached to a module, never to a
//! building: a warehouse's couriers are `level * 2`, a residence houses one
//! citizen per level, a crafter's recipes are `2^level`. Three of them read a
//! research effect, which is why [`Capacity::with_research`] exists beside
//! [`Capacity::at`].
//!
//! **Level 0 is a real level and it holds nothing.** A hut placed and not built
//! is level 0 (`core/colony/buildings/AbstractSchematicProvider.java:64`), and
//! every formula below answers 0 there except [`Capacity::Flat`] — Java's
//! `(b) -> 1` lambda really does answer 1 for an unbuilt hut, and the rule that
//! stops a worker arriving then is elsewhere (`canAssignCitizens`,
//! `api/colony/buildings/IBuilding.java:464-467`). The port keeps both facts
//! separate instead of folding one into the other.
//!
//! | Formula | Module | Where Java says it |
//! | --- | --- | --- |
//! | `level` | living, home, farmer's fields | `LivingBuildingModule.java:103-106`, `HomeBuildingModule.java:19-22`, `workerbuildings/BuildingFarmer.java:280-284` |
//! | `2 * level` | the warehouse's couriers | `modules/CourierAssignmentModule.java:98-101` |
//! | `3 * level` | the tavern's visitors | `modules/TavernBuildingModule.java:162` |
//! | `4` once built | the tavern's beds | `TavernLivingBuildingModule.java:15-18` |
//! | `5 * level * (1 + minimumstockmultiplier)` | minimum stock | `modules/MinimumStockModule.java:56-61` |
//! | `5 * level` | the restaurant menu | `modules/RestaurantMenuModule.java:45`, `:98` |
//! | `2^level * (1 + recipesmultiplier)` | every crafter's recipe list | `modules/AbstractCraftingBuildingModule.java:171-179` |
//! | `5 ×` the recipe count, for every crafter but the two simple ones | builder, miner | `:87`, `:1001-1004`, `SimpleCraftingModule.java:46-49` |
//! | `ceil(level / 2)`, `+1` with `plantationlarge` | the plantation's fields | `workerbuildings/BuildingPlantation.java:246-252` |
//! | `64 * level` stacks of fuel kept | the furnace module | `modules/FurnaceUserModule.java:93-96` |
//!
//! **`held` is an argument, never a field.** The building record
//! (`crate::building::Building`) has one worker slot, while Java's count is per
//! module (`api/colony/buildings/modules/IAssignsCitizen.java:36-46`). Every
//! rule here takes "how many are already there" from the caller, so nobody has
//! to trust the record as a total.
//!
//! **What is *not* here.** The worker rows of the table are not restated: 58 of
//! them live in `crate::jobs::employment`, and [`capacity_on`] reads that table
//! first. That is deliberate — one table, one place to fix when a number is
//! wrong, and a test in `rules` that a hut's worker module answers the same count
//! through both paths.

use crate::jobs::employment::{Capacity as HeadCount, employees};

use super::key::ModuleKey;
use crate::building::{Kind, MAX_BUILDING_LEVEL};

/// The stock a `min_stock` module holds per level: `STOCK_PER_LEVEL = 5`
/// (`modules/MinimumStockModule.java:44`, read at `:60`).
pub const STOCK_PER_LEVEL: i32 = 5;

/// The menu entries a restaurant holds per level — the same `5`, declared in its
/// own class (`modules/RestaurantMenuModule.java:45`, read at `:98`). Two
/// constants with the same value that Java never joined; the port keeps the
/// distinction because only one of them scales with research.
pub const MENU_PER_LEVEL: i32 = 5;

/// The stacks of fuel a furnace module keeps per level: `Constants.STACKSIZE`
/// is 64 (`api/util/constant/Constants.java:23`), multiplied by the building
/// level at `modules/FurnaceUserModule.java:95`.
pub const FUEL_STACKS: i32 = 64;

/// How many beds the tavern offers once built: a flat 4, whatever the level above
/// 1 (`TavernLivingBuildingModule.java:15-18`).
pub const TAVERN_BEDS: i32 = 4;

/// A module's head count, item count or recipe count, as a function of the
/// building's level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: every formula Java attaches to a module, and a new one needs its line
pub enum Capacity {
    /// Nothing countable: a settings page, a statistics panel, a view-only
    /// producer whose server half is `null` (`BuildingModules.java:52-53`).
    Uncounted,
    /// A constant, whatever the level — Java's `(b) -> 1` and `(b) -> 2`
    /// (`BuildingModules.java:113-116`, `:580-585`). **Answers its number even at
    /// level 0**, which is what the lambda does; `canAssignCitizens` is what
    /// stops a worker arriving there.
    Flat(i32),
    /// A constant once the hut is built, nothing before: the tavern's four beds
    /// (`TavernLivingBuildingModule.java:17`, `level > 0 ? 4 : 0`).
    FlatWhenBuilt(i32),
    /// One per level (`LivingBuildingModule.java:105`).
    PerLevel,
    /// Two per level (`CourierAssignmentModule.java:100`).
    TwiceLevel,
    /// Three per level (`TavernBuildingModule.java:162`).
    ThreePerLevel,
    /// Five per level, scaled by the `minimumstockmultiplier` research
    /// (`MinimumStockModule.java:56-61`).
    StockPerLevel,
    /// Five per level, **not** scaled by any research (`RestaurantMenuModule.java:98`).
    MenuPerLevel,
    /// `2^level`, scaled by the `recipesmultiplier` research
    /// (`AbstractCraftingBuildingModule.java:178`).
    Recipes,
    /// Five times [`Self::Recipes`]: `EXTRA_RECIPE_MULTIPLIER = 5` (`:87`)
    /// multiplies the count for every crafting module whose `canLearnManyRecipes`
    /// is true, which is the class default (`:1001-1004`). Only the builder's and
    /// the miner's crafting modules answer false, being `SimpleCraftingModule`s
    /// (`SimpleCraftingModule.java:46-49`).
    ManyRecipes,
    /// `ceil(level / 2)`, plus one when `plantationlarge` is researched
    /// (`BuildingPlantation.java:246-252`).
    HalfLevel,
    /// N stacks per level, the fuel a furnace user keeps
    /// (`FurnaceUserModule.java:95`, `STACKSIZE = 64`).
    KeptStacksPerLevel(i32),
}

impl Capacity {
    /// How many the module holds at `level`, with no research known.
    ///
    /// A negative level is read as 0: the only way to produce one is a hand-written
    /// compound, which `crate::building::clamp_level` has already clamped, so this
    /// is a belt over a brace rather than a rule.
    pub fn at(self, level: i32) -> i32 {
        self.with_research(level, 0.0)
    }

    /// The same, with one research effect's strength.
    ///
    /// `strength` is `IResearchEffectManager.getEffectStrength` — 0 when the
    /// research is unknown, and the effect's level value otherwise
    /// (`core/colony/managers/ResearchManager.java:135-140` gives the effect id a
    /// hut uses; the module constants name their own, listed per variant). Only
    /// three variants read it; the others ignore it, which is a fact and not an
    /// oversight — a warehouse's couriers do not care about `minimumstockmultiplier`.
    pub fn with_research(self, level: i32, strength: f64) -> i32 {
        let level = level.max(0);
        let scaled = |base: i32, strength: f64| (base as f64 * (1.0 + strength)) as i32;
        match self {
            Capacity::Uncounted => 0,
            Capacity::Flat(count) => count,
            Capacity::FlatWhenBuilt(count) => {
                if level > 0 {
                    count
                } else {
                    0
                }
            }
            Capacity::PerLevel => level,
            Capacity::TwiceLevel => 2 * level,
            Capacity::ThreePerLevel => 3 * level,
            // `(int)(level * 5 * increase)` — MinimumStockModule.java:60
            Capacity::StockPerLevel => scaled(level * STOCK_PER_LEVEL, strength),
            // `level * 5`, no increase — RestaurantMenuModule.java:98
            Capacity::MenuPerLevel => level * MENU_PER_LEVEL,
            // `(int)(Math.pow(2, level) * increase)` — AbstractCraftingBuildingModule.java:178
            Capacity::Recipes => scaled(2i32.saturating_pow(level as u32), strength),
            // ... and the same with `increase *= EXTRA_RECIPE_MULTIPLIER` (5)
            // before the product — `:174-178`, `:87`
            Capacity::ManyRecipes => scaled(5 * 2i32.saturating_pow(level as u32), strength),
            // `(int) Math.ceil(level / 2D)`, `+1` when the effect is above 0 —
            // BuildingPlantation.java:248-251. `level` is already `max(0)`
            // (`AbstractSchematicProvider.java:64` has no negative level), so the
            // integer form of a ceiling is `(level + 1) / 2`; `div_ceil` is still
            // unstable for signed integers and the CI refuses it.
            Capacity::HalfLevel => {
                let base = (level + 1) / 2;
                if strength > 0.0 {
                    base + 1
                } else {
                    base
                }
            }
            // `STACKSIZE * building.getBuildingLevel()` — FurnaceUserModule.java:95
            Capacity::KeptStacksPerLevel(stacks) => stacks * level,
        }
    }

    /// Whether the number grows with the level, and so is 0 for an unbuilt hut.
    ///
    /// [`Self::Flat`] answers false because Java's lambda does: the flat modules
    /// are the ones whose count never moves, and the unbuilt hut is refused a
    /// worker by another rule.
    pub fn is_level_based(self) -> bool {
        !matches!(self, Capacity::Uncounted | Capacity::Flat(_))
    }

    /// The research effect the count reads, when it reads one.
    pub fn effect(self) -> Option<&'static str> {
        match self {
            Capacity::StockPerLevel => Some("minimumstockmultiplier"),
            Capacity::Recipes | Capacity::ManyRecipes => Some("recipesmultiplier"),
            Capacity::HalfLevel => Some("plantationlarge"),
            _ => None,
        }
    }

    /// Whether the module counts anything at all.
    pub fn is_counted(self) -> bool {
        !matches!(self, Capacity::Uncounted)
    }

    /// The most the count can reach, at [`MAX_BUILDING_LEVEL`] and with research
    /// at 0. `None` when the count does not grow.
    pub fn ceiling(self) -> Option<i32> {
        if self.is_level_based() {
            Some(self.at(MAX_BUILDING_LEVEL))
        } else {
            None
        }
    }
}

/// The keys whose count is a constant of the module itself, one row each.
///
/// Short on purpose: the 58 worker rows are in `crate::jobs::employment`, and the
/// 33 recipe lists are answered by [`ModuleKey::crafts`] — a row for each here
/// would be a second copy of both.
pub const CAPACITIES: &[(ModuleKey, Capacity)] = &[
    // MinimumStockModule.java:44, :60 — 5 per level, research-scaled
    (ModuleKey::MinStock, Capacity::StockPerLevel),
    // BuildingModules.java:46-48 — the postbox's own producer, same module class
    (ModuleKey::MinStockPostbox, Capacity::StockPerLevel),
    // RestaurantMenuModule.java:45, :98 — 5 per level, not research-scaled
    (ModuleKey::RestaurantMenu, Capacity::MenuPerLevel),
    // BuildingModules.java:88-89 — `new RestaurantMenuModule(false, building -> 1)`
    (ModuleKey::NetherMinerMenu, Capacity::Flat(1)),
    // LivingBuildingModule.java:103-106
    (ModuleKey::Living, Capacity::PerLevel),
    // HomeBuildingModule.java:19-22 — `Math.max(prev, building.getBuildingLevel())`
    (ModuleKey::Home, Capacity::PerLevel),
    // TavernLivingBuildingModule.java:15-18
    (ModuleKey::TavernLiving, Capacity::FlatWhenBuilt(TAVERN_BEDS)),
    // TavernBuildingModule.java:162 — `externalCitizens.size() < 3 * level`
    (ModuleKey::TavernVisitor, Capacity::ThreePerLevel),
    // BuildingFarmer.java:280-284 — `getMaxExtensionCount() = level`
    (ModuleKey::FarmerFields, Capacity::PerLevel),
    // BuildingPlantation.java:246-252 — `ceil(level / 2)`, `+1` with the research
    (ModuleKey::PlantationFields, Capacity::HalfLevel),
    // FurnaceUserModule.java:93-96 — `STACKSIZE * level` stacks of fuel
    (ModuleKey::Furnace, Capacity::KeptStacksPerLevel(FUEL_STACKS)),
];

/// The count of a key, without knowing which hut carries it.
///
/// The recipe lists are answered by the class they share, not by a table row:
/// `2^level` for everything, five times that for every crafting module that is
/// not a `SimpleCraftingModule` (`AbstractCraftingBuildingModule.java:87`,
/// `:171-179`, `SimpleCraftingModule.java:46-49`). A key this module does not
/// know answers [`Capacity::Uncounted`], which is the truth for a settings page
/// and a view.
pub fn capacity(key: ModuleKey) -> Capacity {
    if key.crafts() {
        return if key.learns_many_recipes() {
            Capacity::ManyRecipes
        } else {
            Capacity::Recipes
        };
    }
    CAPACITIES
        .iter()
        .find(|(held, _)| *held == key)
        .map(|(_, capacity)| *capacity)
        .unwrap_or(Capacity::Uncounted)
}

/// The count of a module **on a hut**: the worker table first, then the table
/// above.
///
/// The order matters for `warehouse_couriers`, which is both a worker row of the
/// warehouse (`crate::jobs::employment`, `TwiceLevel`) and a module of it; reading
/// employment first means the number comes from one place and a test holds the two
/// in agreement.
pub fn capacity_on(kind: Kind, key: ModuleKey) -> Capacity {
    if let Ok(rows) = employees(kind) {
        if let Some(row) = rows.iter().find(|row| row.module == key.key()) {
            return match row.capacity {
                HeadCount::One => Capacity::Flat(1),
                HeadCount::Two => Capacity::Flat(2),
                HeadCount::PerLevel => Capacity::PerLevel,
                HeadCount::TwiceLevel => Capacity::TwiceLevel,
            };
        }
    }
    capacity(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_holds_no_key_twice_and_no_key_that_holds_nothing() {
        let mut keys: Vec<ModuleKey> = CAPACITIES.iter().map(|(key, _)| *key).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), CAPACITIES.len(), "a key is listed twice");
        for (key, capacity) in CAPACITIES {
            assert!(capacity.is_counted(), "{key} is listed but holds nothing");
            assert!(
                !key.crafts(),
                "{key} holds recipes, which the crafts() rule already answers"
            );
            assert!(
                !key.is_view_only(),
                "{key} has no server half, so it cannot hold anything"
            );
        }
        assert_eq!(CAPACITIES.len(), 11);
    }

    #[test]
    fn a_count_per_level_is_zero_before_the_build_and_five_rungs_after() {
        for (capacity, wanted) in [
            (Capacity::PerLevel, [0, 1, 2, 3, 4, 5]),
            (Capacity::TwiceLevel, [0, 2, 4, 6, 8, 10]),
            (Capacity::ThreePerLevel, [0, 3, 6, 9, 12, 15]),
            (Capacity::StockPerLevel, [0, 5, 10, 15, 20, 25]),
            (Capacity::MenuPerLevel, [0, 5, 10, 15, 20, 25]),
        ] {
            for (level, expected) in wanted.iter().enumerate() {
                assert_eq!(
                    capacity.at(level as i32),
                    *expected,
                    "{capacity:?} at level {level}"
                );
            }
        }
    }

    #[test]
    fn the_recipe_list_doubles_and_the_research_doubles_it_again() {
        let wanted = [1, 2, 4, 8, 16, 32];
        for (level, expected) in wanted.iter().enumerate() {
            assert_eq!(Capacity::Recipes.at(level as i32), *expected);
        }
        // `recipesmultiplier` tops out at 2.0 (`research/effect.rs:202`):
        // `(int)(2^5 * (1 + 2.0))` = 96.
        assert_eq!(Capacity::Recipes.with_research(5, 2.0), 96);
        assert_eq!(Capacity::Recipes.with_research(1, 0.25), 2, "1.25 truncated to 2.5 → 2");
        // `EXTRA_RECIPE_MULTIPLIER = 5` multiplies the whole product
        // (AbstractCraftingBuildingModule.java:174-178, :87).
        assert_eq!(Capacity::ManyRecipes.at(0), 5);
        assert_eq!(Capacity::ManyRecipes.at(1), 10);
        assert_eq!(Capacity::ManyRecipes.at(3), 40);
        assert_eq!(Capacity::ManyRecipes.at(5), 160);
        assert_eq!(Capacity::ManyRecipes.with_research(5, 2.0), 480);
        assert_eq!(Capacity::ManyRecipes.at(4) / Capacity::Recipes.at(4), 5);
        // The two five-per-level formulas disagree exactly on this.
        assert_eq!(Capacity::StockPerLevel.with_research(2, 1.0), 20);
        assert_eq!(Capacity::MenuPerLevel.with_research(2, 1.0), 10, "no research reads the menu");
    }

    #[test]
    fn the_plantations_fields_are_half_a_level_each_and_the_research_adds_one() {
        let wanted = [0, 1, 1, 2, 2, 3];
        for (level, expected) in wanted.iter().enumerate() {
            assert_eq!(
                Capacity::HalfLevel.at(level as i32),
                *expected,
                "ceil({level} / 2)"
            );
        }
        assert_eq!(Capacity::HalfLevel.with_research(2, 1.0), 2);
        assert_eq!(Capacity::HalfLevel.with_research(5, 1.0), 4);
        // BuildingPlantation.java:249 tests `> 0`, so a zero-strength effect is
        // "not researched" and not "researched at zero".
        assert_eq!(Capacity::HalfLevel.with_research(5, 0.0), 3);
    }

    #[test]
    fn the_tavern_offers_four_beds_and_none_before_it_stands() {
        assert_eq!(Capacity::FlatWhenBuilt(TAVERN_BEDS).at(0), 0);
        for level in 1..=5 {
            assert_eq!(Capacity::FlatWhenBuilt(TAVERN_BEDS).at(level), 4);
        }
        // A flat count is flat even at level 0, as Java's `(b) -> 1` is.
        assert_eq!(Capacity::Flat(1).at(0), 1);
        assert_eq!(Capacity::Flat(2).at(5), 2);
    }

    #[test]
    fn a_negative_level_is_read_as_zero_and_the_top_of_the_range_is_five() {
        assert_eq!(Capacity::PerLevel.at(-3), 0);
        assert_eq!(Capacity::Recipes.at(-1), 1);
        assert_eq!(Capacity::PerLevel.ceiling(), Some(5));
        assert_eq!(Capacity::TwiceLevel.ceiling(), Some(10));
        assert_eq!(Capacity::Recipes.ceiling(), Some(32));
        assert_eq!(Capacity::Flat(1).ceiling(), None, "a constant has no ceiling");
        assert_eq!(Capacity::Uncounted.ceiling(), None);
    }

    #[test]
    fn only_three_formulas_read_a_research_and_each_names_its_effect() {
        let reading: Vec<(ModuleKey, &str)> = CAPACITIES
            .iter()
            .filter_map(|(key, capacity)| capacity.effect().map(|effect| (*key, effect)))
            .collect();
        assert_eq!(
            reading,
            vec![
                (ModuleKey::MinStock, "minimumstockmultiplier"),
                (ModuleKey::MinStockPostbox, "minimumstockmultiplier"),
                (ModuleKey::PlantationFields, "plantationlarge"),
            ]
        );
        for effect in [
            Capacity::Recipes.effect(),
            Capacity::ManyRecipes.effect(),
            Capacity::HalfLevel.effect(),
            Capacity::StockPerLevel.effect(),
        ]
        .into_iter()
        .flatten()
        {
            assert!(
                crate::research::effect::category(effect).is_some(),
                "the effect {effect} is not declared by the research data"
            );
        }
        assert_eq!(Capacity::PerLevel.effect(), None);
        // `furnace` is a `Capacity`, not a key: the count is the module's
        // (`FurnaceUserModule.java:95`), and it reads no research.
        assert_eq!(
            capacity(ModuleKey::Furnace),
            Capacity::KeptStacksPerLevel(FUEL_STACKS)
        );
        assert_eq!(capacity(ModuleKey::Furnace).effect(), None);
        assert_eq!(capacity(ModuleKey::Furnace).at(3), 192);
    }

    #[test]
    fn a_crafter_holds_recipes_and_the_two_simple_ones_hold_five_times_fewer() {
        // Every crafting module holds `2^level`, and every one that is not a
        // `SimpleCraftingModule` holds five times that
        // (AbstractCraftingBuildingModule.java:174-178, SimpleCraftingModule.java:46-49).
        assert_eq!(capacity(ModuleKey::FarmerWork), Capacity::ManyRecipes);
        assert_eq!(capacity(ModuleKey::StonemasonDoCraft), Capacity::ManyRecipes);
        assert_eq!(capacity(ModuleKey::BuilderCraft), Capacity::Recipes);
        assert_eq!(capacity(ModuleKey::MinerCraft), Capacity::Recipes);
        assert_eq!(capacity(ModuleKey::StatsModule), Capacity::Uncounted);
        assert_eq!(capacity(ModuleKey::Bed), Capacity::Uncounted);
        // `bed` holds no count of its own: the beds are block entities the
        // schematic places (`modules/BedHandlingModule.java:24-30`).
        assert!(!Capacity::Uncounted.is_counted());
    }

    #[test]
    fn the_warehouse_couriers_read_four_heads_at_level_two_from_the_worker_table() {
        assert_eq!(
            capacity_on(Kind::Warehouse, ModuleKey::WarehouseCouriers),
            Capacity::TwiceLevel
        );
        // ... and the same number through the other path, which is the point.
        assert_eq!(
            capacity_on(Kind::Warehouse, ModuleKey::WarehouseCouriers).at(2),
            crate::jobs::employment::EMPLOYMENTS
                .iter()
                .find(|row| row.hut == Kind::Warehouse && row.module == "warehouse_couriers")
                .map(|row| row.capacity_at(2))
                .expect("the warehouse's courier row")
        );
    }

    #[test]
    fn a_hut_answers_the_capacity_of_the_key_it_carries_and_nothing_else() {
        assert_eq!(capacity_on(Kind::Baker, ModuleKey::MinStock), Capacity::StockPerLevel);
        assert_eq!(capacity_on(Kind::Fisherman, ModuleKey::MinStock), Capacity::StockPerLevel);
        assert_eq!(capacity_on(Kind::Tavern, ModuleKey::TavernLiving), Capacity::FlatWhenBuilt(4));
        assert_eq!(capacity_on(Kind::Residence, ModuleKey::Living), Capacity::PerLevel);
        // The worker rows of the warehouse and of a farm both come from employment.
        assert_eq!(capacity_on(Kind::Farmer, ModuleKey::FarmerCraft), Capacity::Flat(1));
        assert_eq!(capacity_on(Kind::Library, ModuleKey::StudentWork), Capacity::TwiceLevel);
        assert_eq!(capacity_on(Kind::GateHouse, ModuleKey::KnightGateWork), Capacity::Flat(2));
        assert_eq!(capacity_on(Kind::University, ModuleKey::UniversityWork), Capacity::PerLevel);
        // A key the hut does not carry still answers its own count: `capacity_on`
        // is about the key, `rules::Hut::capacity` is about the hut.
        assert_eq!(capacity_on(Kind::Stash, ModuleKey::MinStock), Capacity::StockPerLevel);
    }

    #[test]
    fn the_menu_and_the_minimum_stock_are_the_same_five_and_only_one_scales() {
        // Two constants, `STOCK_PER_LEVEL` and `MENU_PER_LEVEL`, both 5, declared
        // in two classes Java never joined (MinimumStockModule.java:44,
        // RestaurantMenuModule.java:45). The port keeps both variants because only
        // one of them reads a research.
        assert_eq!(STOCK_PER_LEVEL, 5);
        assert_eq!(MENU_PER_LEVEL, 5);
        assert_ne!(Capacity::StockPerLevel, Capacity::MenuPerLevel);
        assert!(Capacity::StockPerLevel.effect().is_some());
        assert_eq!(Capacity::MenuPerLevel.effect(), None);
        assert_eq!(Capacity::StockPerLevel.at(3), Capacity::MenuPerLevel.at(3));
    }

    #[test]
    fn every_variant_answers_a_ceiling_that_matches_its_kind_and_no_variant_is_forgotten() {
        // A list of all twelve variants, so a new one cannot be added without a
        // line here and a decision about whether it has a ceiling.
        let all = [
            Capacity::Uncounted,
            Capacity::Flat(1),
            Capacity::FlatWhenBuilt(TAVERN_BEDS),
            Capacity::PerLevel,
            Capacity::TwiceLevel,
            Capacity::ThreePerLevel,
            Capacity::StockPerLevel,
            Capacity::MenuPerLevel,
            Capacity::Recipes,
            Capacity::ManyRecipes,
            Capacity::HalfLevel,
            Capacity::KeptStacksPerLevel(FUEL_STACKS),
        ];
        assert_eq!(all.len(), 12, "twelve formulas in the enum");
        for capacity in all {
            match capacity.ceiling() {
                Some(top) => {
                    assert!(capacity.is_level_based());
                    assert_eq!(top, capacity.at(MAX_BUILDING_LEVEL));
                    assert!(top > capacity.at(0), "{capacity:?} grows by nothing");
                }
                None => assert!(
                    !capacity.is_level_based() || capacity == Capacity::Uncounted,
                    "{capacity:?} grows but states no ceiling"
                ),
            }
            assert_eq!(
                capacity.is_counted(),
                capacity != Capacity::Uncounted,
                "{capacity:?}"
            );
        }
    }
}
