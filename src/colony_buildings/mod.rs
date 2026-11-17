//! The buildings as a *model*: which hut carries which module, what each module
//! holds at a given level, and the refusals a hut opposes.
//!
//! Java reference: `core/colony/buildings/` — `AbstractBuilding` (2 150 lines),
//! `AbstractBuildingContainer` (229), `AbstractBuildingGuards`, the 43 files of
//! `modules/`, the 50 of `workerbuildings/`, and the initialiser that wires the
//! two together, `apiimp/initializer/ModBuildingsInitializer.java` (691 lines).
//!
//! **What was already here, and what this module adds.** `crate::building` holds
//! one building: its kind, level, stage, parent, the modules *attached* to it
//! and the rules of a level change. `crate::jobs::employment` holds the 58
//! worker modules: which hut hires which job, with which skills, how many heads.
//! What neither carries is the **table**: for each of the 54 kinds, the complete
//! list of modules its registry entry registers, and the rule saying what each
//! of those modules holds. That is this module, and it adds no second truth —
//! [`capacity_on`] reads the worker rows out of `employment`.
//!
//! | File | Question |
//! | --- | --- |
//! | `key` | the 157 module keys Java declares, and which of them carry a rule |
//! | `catalog` | one row per kind: max level, research effect, every module |
//! | `capacity` | what a module holds at a level, and what research does to it |
//! | `refusal` | why the answer was no, named |
//! | `rules` | the same, asked of a live building |
//!
//! **The premise this port had to correct.** The brief asks for "module → level
//! of unlocking". Java has no such thing: a hut's entry registers **every** one
//! of its modules when the building is produced, with no level test at all
//! (`api/colony/buildings/registry/BuildingEntry.java:148-160`, the loop over
//! `buildingModuleProducers`), so a level-0 hut already has its crafting module,
//! its settings page and its statistics panel. What the level changes is *how
//! much* each module holds — 0 seats, 0 recipes, 0 stock — and, for a handful of
//! capabilities, whether the feature is switched on at all:
//!
//! | What is really gated | At | Where Java says it |
//! | --- | --- | --- |
//! | the warehouse sorts its racks | level 3 | `workerbuildings/BuildingWareHouse.java:194-197` |
//! | the baker's crafting types | level 3 | `workerbuildings/BuildingBaker.java:161-165` |
//! | the florist's flower filter, 2nd flower | level 4 | `moduleviews/FloristFlowerListModuleView.java:56-58` |
//! | the florist's flower filter, 6th flower | level 5 | `:62-66` |
//! | a marksman or a huscarl may be hired | research | `BuildingModules.java:558-563`, `:575-578` |
//! | the nether worker's expedition log is readable | research | `modules/ExpeditionLogModule.java:46-52` |
//! | **any module exists at all** | nothing | `BuildingEntry.java:148-160` |
//!
//! So the gate column has four values, and only one of them is a level:
//! [`Gate::Always`], [`Gate::OnceBuilt`] (capacity 0 until the first build),
//! [`Gate::Research`] and [`Gate::Level`]. A test holds the claim by checking
//! that no key holds a level gate Java does not state.
//!
//! **Pure data, pure rules.** Nothing here calls the host — no world, no clock,
//! no file — for the same reason `crate::building` and `crate::colony` do not: a
//! wasm mod has none of them, and every rule below is testable with nothing but
//! numbers.
//!
//! **What is *not* here**, and what each gap costs:
//!
//! - **no GUI and no packets.** `moduleviews/` (the 40 client-side module views)
//!   and `AbstractBuildingView` answer "what does the window show"; the port has
//!   one side, so a [`ModuleKey`] is only its server half. `MANQUANT.md` lists the
//!   views that state a rule of their own (the florist's filter, above).
//! - **no schematic, no chest, no bed.** `AbstractBuildingContainer` (229 lines)
//!   is the container list a schematic fills; [`Hut`] counts *modules*, not
//!   blocks, and says so where a caller might expect a container count.
//! - **no request system.** A module here states a capacity; Java's
//!   `MinimumStockModule` also raises requests to keep that stock
//!   (`modules/MinimumStockModule.java:70-140`), which needs
//!   `crate::requests`. The capacity is the half that is a model.
//! - **one worker per building.** `crate::building::Building` holds one worker;
//!   the real count is per module (`IAssignsCitizen.getModuleMax`). Every rule
//!   that needs the real count takes it as an argument (`held`), so the record's
//!   single slot never has to be trusted as a total.

use crate::building::Kind;

pub mod capacity;
pub mod catalog;
pub mod key;
pub mod refusal;
pub mod rules;

pub use capacity::{CAPACITIES, Capacity, capacity_on};
pub use catalog::{HUTS, HutSpec, accepts, modules_of, spec_of};
pub use key::{MODULE_KEY_COUNT, ModuleKey};
pub use refusal::Refusal;
pub use rules::{Hut, SORT_FROM_LEVEL, claim_radius, default_claim_radius};

/// The five building levels, re-exported from `crate::building` so a caller
/// working on modules does not have to name the other module to read the rule.
/// `Constants.MAX_BUILDING_LEVEL` (`api/util/constant/Constants.java:20`).
pub use crate::building::MAX_BUILDING_LEVEL;

/// The level a placed and not yet built hut stands at
/// (`core/colony/buildings/AbstractSchematicProvider.java:64`).
pub use crate::building::LEVEL_MIN;

/// The lowest **built** level, which is 1: a hut at 0 is a construction site and
/// no module holds anything (`IBuilding.canAssignCitizens`,
/// `api/colony/buildings/IBuilding.java:464-467`).
pub const FIRST_BUILT_LEVEL: i32 = LEVEL_MIN;

/// The level from which a warehouse sorts its racks:
/// `DEFAULT_REQUIRED_SORT_LEVEL` (`api/util/constant/BuildingConstants.java:108`),
/// read at `BuildingWareHouse.java:196`.
pub use rules::SORT_FROM_LEVEL as WAREHOUSE_SORT_LEVEL;

/// How a module is reached: what has to be true before it does anything.
///
/// Java never writes this enum; it writes the four conditions in four places,
/// each listed on the variant. The point of naming it is that a caller can ask
/// one question — "may this module be used now" — and get one answer, instead of
/// re-deriving it from a level, a research table and a build flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: four conditions in Java, and a fifth would need its line
pub enum Gate {
    /// Present and usable whatever the level, including 0: a statistics panel,
    /// a settings page, an item list head. Java registers it and nothing tests
    /// the level (`BuildingEntry.java:148-160`).
    Always,
    /// Present from level 0, but it holds nothing until the hut is built: the
    /// count is a formula of the level, and every one of them answers 0 there
    /// (`LivingBuildingModule.java:103-106`, `CourierAssignmentModule.java:100`).
    OnceBuilt,
    /// Present, but unusable until a research effect is known: a marksman or a
    /// huscarl is only auto-hired when its effect is above 0
    /// (`WorkerBuildingModule.java:167`), which the two guard modules state as
    /// `MARKSMAN` / `HUSCARL` (`BuildingModules.java:558-563`, `:575-578`).
    Research(&'static str),
    /// Present, but the feature it carries switches on at this level: the
    /// warehouse's sorting (`BuildingWareHouse.java:194-197`), the baker's
    /// crafting types (`BuildingBaker.java:161-165`), the florist's filter
    /// (`FloristFlowerListModuleView.java:56-66`).
    Level(i32),
}

impl Gate {
    /// The level the gate needs, or 0 when it needs none.
    pub fn level(self) -> i32 {
        match self {
            Gate::Level(level) => level,
            Gate::Always | Gate::OnceBuilt | Gate::Research(_) => 0,
        }
    }

    /// The research effect the gate names, if any.
    pub fn research(self) -> Option<&'static str> {
        match self {
            Gate::Research(effect) => Some(effect),
            _ => None,
        }
    }

    /// Whether the gate decides whether the module is **registered**. Always
    /// false, and deliberately a question: it is the one the brief asked, and the
    /// answer is the finding (`BuildingEntry.java:148-160`).
    pub fn decides_presence(self) -> bool {
        false
    }
}

/// The gate of a module on one kind of hut. Most modules carry none.
///
/// The gate takes the kind because half of it is the hut's: a module whose count
/// is a formula of the level holds nothing before the hut is built
/// ([`Gate::OnceBuilt`]), and that count comes from [`capacity_on`] — the same key
/// is `2 × level` on a warehouse and uncounted elsewhere.
pub fn gate(kind: Kind, key: ModuleKey) -> Gate {
    match key {
        // BuildingModules.java:558-563 — `new GuardBuildingModule(..., MARKSMAN)`
        ModuleKey::MarksmanBarracksWork | ModuleKey::MarksmanTowerWork => Gate::Research("marksman"),
        // :561-563, :577-578
        ModuleKey::HuscarlBarracksWork | ModuleKey::HuscarlTowerWork => Gate::Research("huscarl"),
        // :630-631 — `ExpeditionLogModule(ResearchConstants.NETHER_LOG)`, gated
        // in `ExpeditionLogModule.java:46-52`
        ModuleKey::NetherWorkerExpedition => Gate::Research("netherexpeditionlog"),
        // BuildingBaker.java:161-165 — no crafting type below level 3
        ModuleKey::BakerCraft | ModuleKey::BakerSmelt => Gate::Level(3),
        // BuildingWareHouse.java:194-197, BuildingConstants.java:108
        ModuleKey::WarehouseOptions => Gate::Level(SORT_FROM_LEVEL),
        // FloristFlowerListModuleView.java:56-66 — level 4 for the second flower,
        // level 5 for the sixth
        ModuleKey::FloristItems => Gate::Level(4),
        // Everything whose count is a formula of the level: nothing before the build.
        _ if capacity_on(kind, key).is_level_based() => Gate::OnceBuilt,
        _ => Gate::Always,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every (hut, module) row of the catalog, which is the unit a gate is about.
    fn rows() -> impl Iterator<Item = (Kind, ModuleKey)> {
        HUTS.iter()
            .flat_map(|hut| hut.modules.iter().map(move |key| (hut.kind, *key)))
    }

    #[test]
    fn the_counts_this_module_states_are_the_ones_the_java_has() {
        // 157 `ModuleProducer`s in one table (BuildingModules.java, 635 lines).
        assert_eq!(ModuleKey::ALL.len(), 157);
        assert_eq!(MODULE_KEY_COUNT, 157);
        // 54 registered buildings, 180 files in the Java package.
        assert_eq!(HUTS.len(), 54);
        assert_eq!(Kind::ALL.len(), 54);
        // 279 (kind, module) rows, from the 279 `addBuildingModuleProducer` calls.
        assert_eq!(rows().count(), 279);
    }

    #[test]
    fn no_gate_decides_whether_a_module_exists() {
        // The finding, as a test: a module is registered by the entry, whatever
        // the level (`BuildingEntry.java:148-160`), so no gate answers "absent".
        for (kind, key) in rows() {
            assert!(
                !gate(kind, key).decides_presence(),
                "{kind}/{key} claims a gate that decides its presence, which Java does not have"
            );
        }
    }

    #[test]
    fn the_four_level_gates_are_the_four_java_states_and_no_others() {
        let levelled: Vec<(&str, i32)> = rows()
            .filter_map(|(kind, key)| match gate(kind, key) {
                Gate::Level(level) => Some((key.key(), level)),
                _ => None,
            })
            .collect();
        assert_eq!(
            levelled,
            vec![
                ("baker_craft", 3),
                ("baker_smelt", 3),
                ("florist_items", 4),
                ("warehouse_options", 3),
            ],
            "four keys, two levels, each with the line that states it"
        );
    }

    #[test]
    fn the_five_research_gates_name_effects_that_exist() {
        let mut seen = Vec::new();
        for (kind, key) in rows() {
            if let Gate::Research(effect) = gate(kind, key) {
                assert!(
                    crate::research::effect::category(effect).is_some(),
                    "{kind}/{key} names the effect {effect}, which the research data does not declare"
                );
                seen.push((key.key(), effect));
            }
        }
        assert_eq!(
            seen,
            vec![
                ("huscarl_barracks_work", "huscarl"),
                ("marksman_barracks_work", "marksman"),
                ("marksman_tower_work", "marksman"),
                ("huscarl_tower_work", "huscarl"),
                ("netherworker_expedition", "netherexpeditionlog"),
            ]
        );
    }

    #[test]
    fn a_row_that_holds_per_level_is_gated_on_the_build_or_on_something_more() {
        // A count of `level` is worth 0 at level 0, so the gate is either the
        // build itself or a stricter condition (a research, a feature level).
        // The converse is *not* claimed: a module may be gated and hold nothing
        // countable, which is exactly the warehouse's sorting.
        //
        // The 88 rows are, by where the count comes from:
        //   31 recipe lists   — the 33 crafting keys minus the baker's two, which
        //                       are level-gated at 3 (`BuildingBaker.java:161-165`)
        //   31 stock modules  — 30 `min_stock` + the postbox's own key
        //    7 furnaces       — `FurnaceUserModule.java:95`
        //    3 per-level module rows — living, home, the farmer's fields
        //    1 menu, 1 half-level, 1 tavern's beds, 1 tavern's visitors
        //   12 worker rows    — the 13 per-level and 3 twice-level rows of
        //                       `crate::jobs::employment`, minus the four guard
        //                       modules a research gates
        let mut gated = 0;
        for (kind, key) in rows() {
            let holds = capacity_on(kind, key).is_level_based();
            let gate = gate(kind, key);
            if holds && !matches!(gate, Gate::Research(_) | Gate::Level(_)) {
                assert_eq!(
                    gate,
                    Gate::OnceBuilt,
                    "{kind}/{key} holds per level but is ungated"
                );
                gated += 1;
            }
        }
        assert_eq!(gated, 88, "88 rows hold a level formula and nothing more");
    }

    #[test]
    fn a_research_gate_states_no_level_and_a_level_gate_states_no_research() {
        for (kind, key) in rows() {
            let gate = gate(kind, key);
            assert!(
                gate.research().is_none() || gate.level() == 0,
                "{kind}/{key} states both a research and a level"
            );
            match gate {
                Gate::Research(_) | Gate::Level(_) => assert!(gate.level() > 0 || gate.research().is_some()),
                Gate::Always | Gate::OnceBuilt => {
                    assert_eq!(gate.level(), 0);
                    assert_eq!(gate.research(), None);
                }
            }
        }
    }

    #[test]
    fn the_first_built_level_is_one_and_the_top_is_five() {
        assert_eq!(FIRST_BUILT_LEVEL, 1);
        assert_eq!(MAX_BUILDING_LEVEL, 5);
        assert_eq!(WAREHOUSE_SORT_LEVEL, 3);
    }

    #[test]
    fn a_view_only_module_has_no_gate_and_holds_nothing() {
        // The producer's server half is `null` (`BuildingModules.java:52-53`), so
        // there is nothing to gate and nothing to count.
        for key in ModuleKey::ALL.iter().filter(|key| key.is_view_only()) {
            let Some(kind) = catalog::kinds_with(*key).next() else {
                panic!("{key} is view-only and no hut carries it")
            };
            assert_eq!(gate(kind, *key), Gate::Always, "{key}");
            assert_eq!(
                capacity_on(kind, *key),
                Capacity::Uncounted,
                "{key} claims to hold something with no server half"
            );
        }
        // ... and no level gate sits on a view-only producer.
        for (kind, key) in rows() {
            if let Gate::Level(_) = gate(kind, key) {
                assert!(!key.is_view_only(), "{kind}/{key}");
            }
        }
    }

    #[test]
    fn the_same_key_can_be_gated_on_one_hut_and_not_on_another() {
        // `warehouse_couriers` is the module that makes the point: it counts per
        // level, so it is `OnceBuilt`, and the key alone would have said `Always`.
        assert_eq!(
            gate(Kind::Warehouse, ModuleKey::WarehouseCouriers),
            Gate::OnceBuilt
        );
        assert_eq!(
            capacity_on(Kind::Warehouse, ModuleKey::WarehouseCouriers),
            Capacity::TwiceLevel
        );
        // A flat module is `Always`, and Java's lambda answers its number at
        // level 0 too (`BuildingModules.java:113-116`).
        assert_eq!(gate(Kind::Farmer, ModuleKey::FarmerCraft), Gate::Always);
        assert_eq!(capacity_on(Kind::Farmer, ModuleKey::FarmerCraft), Capacity::Flat(1));
    }
}
