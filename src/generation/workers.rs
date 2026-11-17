//! The worker recipes: the `crafterrecipes/` files, which are neither vanilla
//! recipes nor loot tables but the third thing this mod writes.
//!
//! Java reference: `CustomRecipeProvider.java` (345 lines — the `DataProvider`
//! and its builder, `:71-344`), `CustomRecipeAndLootTableProvider.java` (77 — the
//! two children one provider runs), and the six `workers/` providers that emit
//! both a recipe and the table it rolls.
//!
//! **The format is not vanilla's.** A `crafterrecipes/` file names one of five
//! *kinds* in its `type` field, read by `CrafterRecipeListener` and not by the
//! game's recipe manager: `recipe`, `recipe-multi-out`, `recipe-multi-in`,
//! `recipe-template`, `remove` (`CustomRecipe.java:47-77`). This module writes the
//! first kind only, because the other four are hand-written per building.
//!
//! | Field | Java property | When it is written |
//! | --- | --- | --- |
//! | `type` | `RECIPE_TYPE_PROP` | always, `recipe` |
//! | `crafter` | `RECIPE_CRAFTER_PROP` | always, `<crafter>_<module>` |
//! | `inputs` | `RECIPE_INPUTS_PROP` | always, `count` omitted when 1 |
//! | `intermediate` | `RECIPE_INTERMEDIATE_PROP` | **always**, `minecraft:air` unless the crafter needs a furnace |
//! | `result` / `count` | `RECIPE_RESULT_PROP` / `count` | when the recipe makes an item; `count` only when not 1 |
//! | `loot-table` | `RECIPE_LOOTTABLE_PROP` | when the result is what a table rolls |
//! | `additional-output` | `RECIPE_SECONDARY_PROP` | with a table: what the player is shown |
//! | `min-building-level` / `max-building-level` | `:132-142` | when the recipe is gated |
//! | `research-id` | `RECIPE_RESEARCHID_PROP` | when a research unlocks it (23 of the 161) |
//! | `show-tooltip` | `RECIPE_SHOW_TOOLTIP` | when the result is a choice of several |
//!
//! # One divergence from the 161 delivered files, and it is not the pack that is wrong
//!
//! The `crafterrecipes/` of this mod were **not** converted into `data/`: they
//! exist only in the 1.20.1 datagen output, 161 files. One field of this module's
//! 26 files still disagrees with that output, and it is this module that is wrong:
//!
//! - **the nether trip's `additional-output`.** Java builds it from
//!   `LootTableAnalyzer.toDrops` over the trip table
//!   (`DefaultNetherWorkerLootProvider.java:241-247`), and the analyzer *expands an
//!   adventure token into the loot table of the mob it names*
//!   (`LootTableAnalyzer.java:171-180`, `expandAdventureToken` `:261-278`), then
//!   sorts by probability (`:133`). So the delivered `netherworker/trip1.json`,
//!   which lists flesh, gunpowder, ghast tears and no token, is **what the current
//!   Java writes** — a regenerated datagen would produce it again. This module
//!   lists the token instead, because the expansion needs the six mobs' vanilla
//!   loot tables, which are the game's and not this crate's; the gap is priced in
//!   `MANQUANT.md` §2 rather than filled with invented drops.
//!
//! The other difference the pack and the code used to show — `matchtype` on the
//! sifter's mesh input — was **not** real: `new ItemStorage(stack, true, false)`
//! is `(stack, ignoreDamage, ignoreNbt)` (`ItemStorage.java:90-96`), so the NBT is
//! matched and the field is never written. Both sides agree, and this module now
//! writes neither.
//!
//! # What is generated
//!
//! 26 of the 161: the 16 sifter recipes and their 4 mesh recipes, and the 6 nether
//! worker recipes — because those are the ones whose inputs and outputs are
//! **derived** from a table this module already builds. The other 135 are one
//! hand-written entry per craft (a baker's twenty-five, a chef's forty…), which is
//! content; see `MANQUANT.md` §3.

use super::json::{arr, i, obj, own, s, Json};
use super::json as json_helpers;
use super::File;

/// The two modules a crafter recipe can belong to (`BuildingConstants`):
/// `custom` is the building's own tab, `crafting` the standard one.
pub const MODULE_CUSTOM: &str = "custom";
pub const MODULE_CRAFTING: &str = "crafting";

/// One `crafterrecipes/` file, built field by field the way
/// `CustomRecipeProvider.CustomRecipeBuilder` builds it.
pub struct CrafterRecipe {
    crafter: String,
    id: String,
    inputs: Vec<Json>,
    intermediate: &'static str,
    result: Option<(String, u32)>,
    loot_table: Option<String>,
    secondary: Vec<Json>,
    min_level: Option<u32>,
    max_level: Option<u32>,
    research: Vec<String>,
    tooltip: bool,
}

impl CrafterRecipe {
    /// `CustomRecipeBuilder.create(crafter, module, id)`: the crafter is the two
    /// names joined, and the file id is `crafter/id` — **one folder per crafter**.
    pub fn create(crafter: &str, module: &str, id: &str) -> CrafterRecipe {
        CrafterRecipe {
            crafter: format!("{crafter}_{module}"),
            id: format!("{crafter}/{id}"),
            inputs: Vec::new(),
            intermediate: "minecraft:air",
            result: None,
            loot_table: None,
            secondary: Vec::new(),
            min_level: None,
            max_level: None,
            research: Vec::new(),
            tooltip: false,
        }
    }

    /// `inputs(ItemStorage…)`: the count is written only when it is not 1
    /// (`CustomRecipeProvider.java:277-284`), and `matchtype` only when the NBT is
    /// ignored.
    pub fn input(mut self, item: &str, count: u32, ignore_nbt: bool) -> CrafterRecipe {
        let mut entry = obj(vec![("item", s(item))]);
        if count != 1 {
            json_helpers::set(&mut entry, "count", i(count as i64));
        }
        if ignore_nbt {
            json_helpers::set(&mut entry, "matchtype", s("ignore"));
        }
        self.inputs.push(entry);
        self
    }

    /// `result(ItemStack)`: the id, and the count when it is not 1.
    pub fn result(mut self, item: &str, count: u32) -> CrafterRecipe {
        self.result = Some((item.to_owned(), count));
        self
    }

    /// `lootTable(…)`: the result is what this table rolls.
    pub fn loot_table(mut self, table: &str) -> CrafterRecipe {
        self.loot_table = Some(table.to_owned());
        self
    }

    /// `secondaryOutputs(…)`: `additional-output`, one entry per item.
    pub fn secondary(mut self, items: &[&str]) -> CrafterRecipe {
        self.secondary = items.iter().map(|item| obj(vec![("item", s(item))])).collect();
        self
    }

    /// `minBuildingLevel` and `maxBuildingLevel`.
    pub fn levels(mut self, min: u32, max: Option<u32>) -> CrafterRecipe {
        self.min_level = Some(min);
        self.max_level = max;
        self
    }

    /// `minResearchId(…)`, which can be called more than once.
    pub fn research(mut self, id: &str) -> CrafterRecipe {
        self.research.push(id.to_owned());
        self
    }

    /// `showTooltip(true)`.
    pub fn tooltip(mut self) -> CrafterRecipe {
        self.tooltip = true;
        self
    }

    /// `intermediate(Blocks.FURNACE)`, for the six smelting crafters.
    pub fn furnace(mut self) -> CrafterRecipe {
        self.intermediate = "minecraft:furnace";
        self
    }

    /// The file, with the fields in the order a reader expects and no empty one.
    pub fn build(self) -> File {
        let mut pairs = vec![
            ("type", s("recipe")),
            ("crafter", s(&self.crafter)),
        ];
        if !self.secondary.is_empty() {
            pairs.push(("additional-output", arr(self.secondary)));
        }
        pairs.push(("inputs", arr(self.inputs)));
        pairs.push(("intermediate", s(self.intermediate)));
        if let Some(table) = &self.loot_table {
            pairs.push(("loot-table", s(table)));
        }
        if let Some(max) = self.max_level {
            pairs.push(("max-building-level", i(max as i64)));
        }
        if let Some(min) = self.min_level {
            pairs.push(("min-building-level", i(min as i64)));
        }
        if !self.research.is_empty() {
            let gate = if self.research.len() == 1 {
                s(&self.research[0])
            } else {
                arr(self.research.iter().map(|id| s(id)).collect())
            };
            pairs.push(("research-id", gate));
        }
        if let Some((item, count)) = &self.result {
            if *count != 1 {
                pairs.push(("count", i(*count as i64)));
            }
            pairs.push(("result", s(item)));
        }
        if self.tooltip {
            pairs.push(("show-tooltip", Json::Bool(true)));
        }
        File {
            path: format!("minecolonies/crafterrecipes/{}.json", self.id),
            json: obj(pairs),
        }
    }
}

/// The 20 sifter recipes: 16 sifts and 4 meshes.
///
/// A sift is `input + mesh`, and its result is the table this module already
/// builds for the same pair — one fact, one source
/// (`DefaultSifterCraftingProvider.java:276-301`).
pub fn sifter_recipes() -> Vec<File> {
    let mut files = Vec::new();
    for (mesh, input, level, _, drops) in super::loot::SIFTER {
        let mut shown: Vec<&str> = vec![mesh_item(mesh)];
        for (dropped, _) in drops {
            if *dropped != "minecraft:empty" && !shown.contains(dropped) {
                shown.push(*dropped);
            }
        }
        files.push(
            CrafterRecipe::create("sifter", MODULE_CUSTOM, &format!("{mesh}/{input}"))
                .input(&format!("minecraft:{input}"), 1, false)
                // `new ItemStorage(new ItemStack(mesh.getMesh()), true, false)`
                // (`DefaultSifterCraftingProvider.java:291`) is the **four-argument**
                // constructor's three-argument sibling: `(stack, ignoreDamage,
                // ignoreNbt)` (`ItemStorage.java:90-96`), so damage is ignored and
                // NBT is **not** — and `storageAsJson` only writes `matchtype`
                // when `ignoreNBT()` is true (`CustomRecipeProvider.java:288-291`).
                // The 16 delivered `sifter/*` files carry no `matchtype`, which is
                // the same answer.
                .input(mesh_item(mesh), 1, false)
                .secondary(&shown)
                .loot_table(&own(&format!("recipes/{mesh}/{input}")))
                .levels(level, None)
                .build(),
        );
    }
    files
}

/// The item a mesh name stands for: `iron` → `minecolonies:sifter_mesh_iron`
/// (`ForgeRegistries.ITEMS.getKey(mesh).getPath().replace("sifter_mesh_", "")`,
/// `DefaultSifterCraftingProvider.java:257`).
fn mesh_item(mesh: &str) -> &'static str {
    match mesh {
        "flint" => "minecolonies:sifter_mesh_flint",
        "iron" => "minecolonies:sifter_mesh_iron",
        "diamond" => "minecolonies:sifter_mesh_diamond",
        _ => "minecolonies:sifter_mesh_string",
    }
}

/// The four mesh recipes, one per crafter that can make its own
/// (`DefaultSifterCraftingProvider.java:303-323`).
///
/// Each is gated by its research and shows a tooltip, because the mesh is a
/// component rather than a product.
pub fn mesh_recipes() -> Vec<File> {
    [
        ("fletcher", "minecraft:string", "sifter_mesh_string", "sifterstringunlock"),
        ("stonemason", "minecraft:flint", "sifter_mesh_flint", "sifterflintunlock"),
        ("blacksmith", "minecraft:iron_ingot", "sifter_mesh_iron", "sifterironunlock"),
        ("mechanic", "minecraft:diamond", "sifter_mesh_diamond", "sifterdiamondunlock"),
    ]
    .into_iter()
    .map(|(job, input, output, research)| {
        CrafterRecipe::create(job, MODULE_CRAFTING, output)
            .input(input, 1, false)
            .result(&own(output), 1)
            .research(&own(&format!("effects/{research}")))
            .tooltip()
            .build()
    })
    .collect()
}

/// The six nether worker recipes: five trips and the lava bucket
/// (`DefaultNetherWorkerLootProvider.java:226-257`).
///
/// A trip costs a stack of cobblestone, half a stack of torches and sixteen
/// ladders, and its `min-building-level` equals its `max-building-level`: trip 3
/// exists at level 3 and nowhere else.
pub fn nether_worker_recipes() -> Vec<File> {
    let mut files = Vec::new();
    for level in 1..=5 {
        let mut shown: Vec<&str> = Vec::new();
        for (block, _, min_level, _) in super::loot::NETHER_BLOCKS {
            if min_level <= level && !shown.contains(&block) {
                shown.push(block);
            }
        }
        shown.push("minecolonies:adventure_token");
        files.push(
            CrafterRecipe::create("netherworker", MODULE_CUSTOM, &format!("trip{level}"))
                .input("minecraft:cobblestone", 64, false)
                .input("minecraft:torch", 32, false)
                .input("minecraft:ladder", 16, false)
                .secondary(&shown)
                .loot_table(&own(&format!("recipes/netherworker/trip{level}")))
                .levels(level, Some(level))
                .build(),
        );
    }
    files.push(
        CrafterRecipe::create("netherworker", MODULE_CUSTOM, "lava")
            .input("minecraft:bucket", 1, false)
            .result("minecraft:lava_bucket", 1)
            .build(),
    );
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::json::{i, s, Json};
    use crate::generation::Pack;

    fn pack() -> Pack {
        Pack::build()
    }

    #[test]
    fn a_crafter_recipe_lives_in_a_folder_named_after_its_crafter() {
        let pack = pack();
        let file = pack
            .get("minecolonies/crafterrecipes/netherworker/trip1.json")
            .unwrap();
        assert_eq!(file["crafter"], s("netherworker_custom"));
        assert_eq!(file["type"], s("recipe"));
        // The three inputs Java names for a trip: cobblestone 64, torch 32, ladder 16
        // (`DefaultNetherWorkerLootProvider.java:260-270`, and the file it writes,
        // `src/datagen/generated/minecolonies/data/minecolonies/crafterrecipes/
        // netherworker/trip1.json`, says `"count": 32` for the torch).
        assert_eq!(file["inputs"][0]["count"], i(64));
        assert_eq!(file["inputs"][1]["count"], i(32));
        assert_eq!(file["inputs"][2]["count"], i(16));
        // A count of 1 is not written: the lava recipe's single bucket is the proof
        // (`…/crafterrecipes/netherworker/lava.json` says `{"item":
        // "minecraft:bucket"}` with no `count`).
        let lava = pack
            .get("minecolonies/crafterrecipes/netherworker/lava.json")
            .unwrap();
        assert_eq!(lava["inputs"][0]["item"], s("minecraft:bucket"));
        assert!(
            lava["inputs"][0].get("count").is_none(),
            "a count of 1 is not written"
        );
    }

    #[test]
    fn the_intermediate_is_written_even_when_it_is_air() {
        // The 161 delivered files all carry it, 155 of them `minecraft:air`
        // (`CustomRecipeProvider.java:236`), and a reader dispatches on it.
        let pack = pack();
        let mut air = 0;
        let mut furnace = 0;
        for file in pack.under("minecolonies/crafterrecipes/") {
            match file.json["intermediate"].as_str() {
                Some("minecraft:air") => air += 1,
                Some("minecraft:furnace") => furnace += 1,
                other => panic!("{}: {other:?}", file.path),
            }
        }
        assert_eq!(air, 26, "every recipe here is an air crafter");
        assert_eq!(furnace, 0, "none of the six providers smelts");
    }

    #[test]
    fn a_trip_exists_at_one_level_only() {
        let pack = pack();
        for level in 1..=5 {
            let file = pack
                .get(&format!("minecolonies/crafterrecipes/netherworker/trip{level}.json"))
                .unwrap();
            assert_eq!(file["min-building-level"], i(level as i64));
            assert_eq!(file["max-building-level"], i(level as i64));
        }
    }

    #[test]
    fn the_sixteen_sifts_point_at_the_table_this_module_builds() {
        let pack = pack();
        let mut checked = 0;
        for (mesh, input, level, _, _) in crate::generation::loot::SIFTER {
            let path = format!("minecolonies/crafterrecipes/sifter/{mesh}/{input}.json");
            let file = pack.get(&path).unwrap_or_else(|| panic!("{path}"));
            assert_eq!(
                file["loot-table"],
                s(&format!("minecolonies:recipes/{mesh}/{input}"))
            );
            assert!(
                pack.get(&format!(
                    "minecolonies/loot_table/recipes/{mesh}/{input}.json"
                ))
                .is_some(),
                "{path} points at a table that is not written"
            );
            assert_eq!(file["min-building-level"], i(level as i64));
            checked += 1;
        }
        assert_eq!(checked, 16);
    }

    #[test]
    fn the_sifter_mesh_input_ignores_damage_and_not_its_nbt() {
        // `new ItemStorage(new ItemStack(mesh.getMesh()), true, false)` is
        // `(stack, ignoreDamage, ignoreNbt)` (`ItemStorage.java:90-96`): damage is
        // ignored, **NBT is not**. `storageAsJson` writes `matchtype` only when
        // `ignoreNBT()` (`CustomRecipeProvider.java:288-291`), so neither input
        // carries the field — and the 16 delivered `sifter/*` files agree.
        let pack = pack();
        let file = pack
            .get("minecolonies/crafterrecipes/sifter/iron/dirt.json")
            .unwrap();
        assert_eq!(file["inputs"][1]["item"], s("minecolonies:sifter_mesh_iron"));
        assert!(
            file["inputs"][1].get("matchtype").is_none(),
            "the mesh keeps its NBT: the flag is the third argument, and it is false"
        );
        assert!(file["inputs"][0].get("matchtype").is_none());
    }

    #[test]
    fn the_four_meshes_are_gated_by_their_own_research() {
        let pack = pack();
        for (job, _, output, research) in [
            ("fletcher", "", "sifter_mesh_string", "sifterstringunlock"),
            ("stonemason", "", "sifter_mesh_flint", "sifterflintunlock"),
            ("blacksmith", "", "sifter_mesh_iron", "sifterironunlock"),
            ("mechanic", "", "sifter_mesh_diamond", "sifterdiamondunlock"),
        ] {
            let path = format!("minecolonies/crafterrecipes/{job}/{output}.json");
            let file = pack.get(&path).unwrap_or_else(|| panic!("{path}"));
            assert_eq!(
                file["research-id"],
                s(&format!("minecolonies:effects/{research}"))
            );
            assert_eq!(file["show-tooltip"], Json::Bool(true));
            assert_eq!(file["crafter"], s(&format!("{job}_crafting")));
        }
    }

    #[test]
    fn a_trip_shows_one_token_and_the_blocks_of_its_level() {
        // The stale pack lists thirteen raw mob drops; the loot table it points at
        // rolls one adventure token. This port follows the table.
        let pack = pack();
        let one = pack
            .get("minecolonies/crafterrecipes/netherworker/trip1.json")
            .unwrap();
        let shown = one["additional-output"].as_array().unwrap();
        assert_eq!(shown.len(), 6, "five blocks and one token: {shown:?}");
        assert_eq!(
            shown.last().map(|entry| entry["item"].clone()),
            Some(s("minecolonies:adventure_token"))
        );
        let five = pack
            .get("minecolonies/crafterrecipes/netherworker/trip5.json")
            .unwrap();
        assert_eq!(
            five["additional-output"].as_array().unwrap().len(),
            23,
            "22 blocks and one token"
        );
    }

    #[test]
    fn the_crafter_name_is_the_two_names_joined_and_never_one() {
        // `crafter + "_" + module` (`:80`): a reader splits on the underscore, so
        // a name without one would be unreadable.
        let pack = pack();
        for file in pack.under("minecolonies/crafterrecipes/") {
            let crafter = file.json["crafter"].as_str().unwrap();
            assert!(crafter.contains('_'), "{}: {crafter}", file.path);
            let (name, module) = crafter.rsplit_once('_').unwrap();
            assert!(!name.is_empty());
            assert!(module == "custom" || module == "crafting", "{module}");
            assert!(
                file.path.contains(&format!("/{name}/")),
                "{}: the folder is the crafter",
                file.path
            );
        }
    }
}
