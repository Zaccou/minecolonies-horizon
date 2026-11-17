//! The crafting recipes and the cooking recipes: what a hut costs, and how a
//! dough bakes.
//!
//! Java reference: `defaults/DefaultRecipeProvider.java` — 1 108 lines, of which
//! this module ports three parts:
//!
//! | Part | Java | Files |
//! | --- | --- | --- |
//! | the three hut patterns | `:284-365` | 47 |
//! | the eleven colony blocks with a shape of their own | `:117-231` | 11 |
//! | the cooking matrix | `:453-544` | 18 |
//!
//! The id of a variant is `append`'s business, not a pattern's (`:264-276`).
//!
//! **The 78 recipes this module does not write** — counted on the pack, which
//! carries 154 — are `buildOtherBlocks` (`:366-451`: 7 shaped, 2 shapeless),
//! `buildOtherItems` (`:546-1108`: 22 shaped, 44 shapeless, and the scimitar's
//! two nugget recipes at `:688-697`), plus **one grid left inside
//! `buildHutRecipes` itself**: the assistant hammer (`:243-249`), the only
//! recipe of that method that is not a colony block. One shaped or shapeless
//! recipe per item, each with its own grid, and no rule between them that a
//! generator could apply. They are content — the pack carries them
//! (`data/minecolonies/recipe/`, 154 files) and the port reads them through
//! [`crate::recipes_content`]; they are not regenerated here, and a generator
//! that did would be a table copy with a build step.
//!
//! # The three hut patterns, and why they are three
//!
//! `registerHutRecipe1` puts the hut's one distinctive item in the middle of nine
//! planks and the build tool on top (`:284-296`); `registerHutRecipe3` puts that
//! item on the **whole middle row** — three of it instead of one (`:351-365`);
//! `registerHutRecipe1x2` is the first pattern producing two huts, for the hut a
//! player is expected to build twice (`:329-343`). The four variants that exist
//! are the cheaper material of the four huts whose tool wears out: the delivery
//! man's iron boots, and the farmer's, lumberjack's and miner's stone tool.
//!
//! **The guard tower is the odd one.** It is registered with `1x2` and an *empty*
//! suffix (`:92`), so it produces two for the price of one bow and keeps the plain
//! id — which is why [`constants::append`]'s second argument decides whether a
//! suffix exists at all.
//!
//! # The cooking matrix is a matrix
//!
//! Six doughs, three methods, one time each (`:274-364`). The cookie is the only
//! row that is not `300 / 100 / 600` seconds at `0.35` experience: it is divided
//! by eight, `300 / 8 = 37`, `100 / 8 = 12`, `600 / 8 = 75`, at `0.0475`
//! experience — because eight cookies come out of one dough elsewhere.
//!
//! **One delivered file disagrees with the code that wrote it**, and it is the
//! only difference between this module's 76 recipes and the pack:
//! `baked_cake.json` says `"category": "blocks"` while its two siblings
//! (`_smoking`, `_campfire_cooking`) say `"food"`, and Java passes
//! `RecipeCategory.FOOD` to all three (`:281-284`, `:311-314`, `:341-344`). The
//! port writes `food` and the test names the difference instead of reproducing
//! it.

use super::constants::{cooking_variant, recipe_path};
use super::json::{arr, f, i, obj, own, s, Json};
use super::File;

/// What the middle of a hut's pattern is: an item, a tag, or a choice of two
/// items (the scarecrow's hay block *or* pumpkin, `:175`).
#[derive(Clone, Copy)]
pub enum Input {
    Item(&'static str),
    Tag(&'static str),
    Either(&'static [&'static str]),
}

/// Which of the three hut patterns a hut uses.
#[derive(Clone, Copy)]
pub enum Shape {
    /// One item, middle of the grid: nine planks around it (`:284`).
    One,
    /// The same item three times, whole middle row (`:351`).
    Three,
    /// The `One` grid, two huts, and a suffix on the id (`:329`).
    Double(&'static str),
}

/// The 43 huts of `registerHutRecipe1/3/1x2`, in source order
/// (`DefaultRecipeProvider.java:69-115`): 42 registered with the `1` or `3`
/// pattern, plus the guard tower, a `1x2` whose suffix is empty (`:92`) and
/// which therefore lives here rather than in [`VARIANTS`].
///
/// The item is the one Java names: `Items.X` for vanilla, `ModBlocks.X` for the
/// composter's barrel and `Tags.Items.CHESTS` for the warehouse.
pub const HUTS: [(&str, Input, Shape); 43] = [
    ("blockhutalchemist", Input::Item("minecraft:brewing_stand"), Shape::One),
    ("blockhutarchery", Input::Item("minecraft:bow"), Shape::Three),
    ("blockhutbaker", Input::Item("minecraft:wheat"), Shape::One),
    ("blockhutbarracks", Input::Item("minecraft:iron_block"), Shape::One),
    ("blockhutbeekeeper", Input::Item("minecraft:beehive"), Shape::One),
    ("blockhutblacksmith", Input::Item("minecraft:iron_ingot"), Shape::Three),
    ("blockhutbuilder", Input::Tag("minecraft:wooden_doors"), Shape::One),
    ("blockhutchickenherder", Input::Item("minecraft:egg"), Shape::One),
    ("blockhutcitizen", Input::Item("minecraft:torch"), Shape::One),
    ("blockhutcombatacademy", Input::Item("minecraft:iron_sword"), Shape::Three),
    ("blockhutcomposter", Input::Item("minecolonies:barrel_block"), Shape::One),
    ("blockhutconcretemixer", Input::Item("minecraft:white_concrete_powder"), Shape::One),
    ("blockhutcook", Input::Item("minecraft:apple"), Shape::One),
    ("blockhutcowboy", Input::Item("minecraft:beef"), Shape::One),
    ("blockhutdeliveryman", Input::Item("minecraft:leather_boots"), Shape::One),
    ("blockhutdyer", Input::Item("minecraft:red_dye"), Shape::One),
    ("blockhutenchanter", Input::Item("minecraft:enchanting_table"), Shape::One),
    ("blockhutfarmer", Input::Item("minecraft:wooden_hoe"), Shape::One),
    ("blockhutfisherman", Input::Item("minecraft:fishing_rod"), Shape::One),
    ("blockhutfletcher", Input::Item("minecraft:string"), Shape::One),
    ("blockhutflorist", Input::Item("minecraft:poppy"), Shape::One),
    ("blockhutglassblower", Input::Item("minecraft:glass"), Shape::One),
    ("blockhutguardtower", Input::Item("minecraft:bow"), Shape::Double("")),
    ("blockhuthospital", Input::Item("minecraft:carrot"), Shape::One),
    ("blockhutlibrary", Input::Item("minecraft:book"), Shape::One),
    ("blockhutlumberjack", Input::Item("minecraft:wooden_axe"), Shape::One),
    ("blockhutmechanic", Input::Item("minecraft:redstone_block"), Shape::One),
    ("blockhutminer", Input::Item("minecraft:wooden_pickaxe"), Shape::One),
    ("blockhutmysticalsite", Input::Item("minecraft:diamond"), Shape::One),
    ("blockhutplantation", Input::Item("minecraft:cactus"), Shape::One),
    ("blockhutrabbithutch", Input::Item("minecraft:rabbit"), Shape::One),
    ("blockhutsawmill", Input::Item("minecraft:wooden_axe"), Shape::Three),
    ("blockhutshepherd", Input::Item("minecraft:shears"), Shape::One),
    ("blockhutsifter", Input::Item("minecraft:string"), Shape::Three),
    ("blockhutsmeltery", Input::Item("minecraft:iron_ingot"), Shape::One),
    ("blockhutstonemason", Input::Item("minecraft:stone_bricks"), Shape::Three),
    ("blockhutswineherder", Input::Item("minecraft:porkchop"), Shape::One),
    ("blockhuttavern", Input::Item("minecraft:barrel"), Shape::One),
    ("blockhuttownhall", Input::Item("structurize:sceptergold"), Shape::One),
    ("blockhutwarehouse", Input::Tag("forge:chests"), Shape::One),
    ("blockhutnetherworker", Input::Item("minecraft:obsidian"), Shape::One),
    ("blockhutkitchen", Input::Item("minecraft:smoker"), Shape::One),
    ("blockhutstable", Input::Item("minecraft:golden_apple"), Shape::One),
];

/// The four `registerHutRecipe1x2` variants (`:83`, `:87`, `:96`, `:99`).
pub const VARIANTS: [(&str, Input, Shape); 4] = [
    ("blockhutdeliveryman", Input::Item("minecraft:iron_boots"), Shape::Double("iron")),
    ("blockhutfarmer", Input::Item("minecraft:stone_hoe"), Shape::Double("stone")),
    ("blockhutlumberjack", Input::Item("minecraft:stone_axe"), Shape::Double("stone")),
    ("blockhutminer", Input::Item("minecraft:stone_pickaxe"), Shape::Double("stone")),
];

/// The build tool, which every hut but the scarecrow costs.
///
/// `structurize:sceptergold` — `ModItems.buildTool`, from the Structurize
/// dependency (`:28`). The scarecrow is built with a build tool too, in its
/// unlock condition only (`:176`): its three keys are leather, stick and hay.
pub const BUILD_TOOL: &str = "structurize:sceptergold";

/// A shaped recipe of the `misc` category — the category of every recipe here,
/// because `RecipeCategory.MISC` is what all eleven specials and all 47 huts pass.
///
/// The **result is `id`'s block**; a recipe whose file id carries a suffix passes
/// the base id here and renames the path, because Java's `1x2` variants suffix the
/// *file* and not the result (`DefaultRecipeProvider.java:334`, `shaped(MISC, output, 2)`,
/// and `:342`, `append(output, name)`).
pub fn shaped(id: &str, count: u32, pattern: [&str; 3], key: &[(&str, Input)]) -> File {
    let mut keys = Vec::new();
    for (symbol, input) in key {
        keys.push((*symbol, input_json(*input)));
    }
    let result = if count == 1 {
        obj(vec![("item", s(&own(id)))])
    } else {
        obj(vec![("count", i(count as i64)), ("item", s(&own(id)))])
    };
    File {
        path: recipe_path(id),
        json: obj(vec![
            ("type", s("minecraft:crafting_shaped")),
            ("category", s("misc")),
            ("key", obj(keys)),
            (
                "pattern",
                arr(pattern.iter().map(|row| s(row)).collect()),
            ),
            ("result", result),
            ("show_notification", Json::Bool(true)),
        ]),
    }
}

/// One key of a shaped grid.
///
/// A key is **an object** when the ingredient is one item or one tag, and an
/// **array** when it is a choice (`Ingredient.of(hay_block, pumpkin)`,
/// `DefaultRecipeProvider.java:175`): that is `Ingredient.toJson`, which writes
/// the single case bare and only wraps in an array for the alternatives form.
fn input_json(input: Input) -> Json {
    match input {
        Input::Item(id) => obj(vec![("item", s(id))]),
        Input::Tag(id) => obj(vec![("tag", s(id))]),
        Input::Either(ids) => arr(ids
            .iter()
            .map(|id| obj(vec![("item", s(id))]))
            .collect()),
    }
}

/// The 47 hut recipes of the three patterns.
///
/// **The four `1x2` variants suffix the file, not the result.** Java calls
/// `ShapedRecipeBuilder.shaped(MISC, output, 2)` — `output` is the hut — and then
/// saves under `append(output, name)` (`DefaultRecipeProvider.java:334-342`), so
/// `blockhutminerstone.json` crafts a `blockhutminer`; the pack confirms it
/// (`data/minecolonies/recipe/blockhutminerstone.json` names
/// `minecolonies:blockhutminer`, count 2).
pub fn hut_recipes() -> Vec<File> {
    let mut files = Vec::new();
    for (hut, input, shape) in HUTS {
        files.push(hut_recipe(hut, input, shape));
    }
    for (hut, input, shape) in VARIANTS {
        if let Shape::Double(_) = shape {
            files.push(hut_recipe(hut, input, shape));
        }
    }
    files.extend(special_huts());
    files
}

/// One row of [`HUTS`] or [`VARIANTS`]: the grid of its shape, the hut as the
/// result, and the suffix on the file name when the shape has one.
fn hut_recipe(hut: &str, input: Input, shape: Shape) -> File {
    let (count, pattern) = match shape {
        Shape::One => (1, ["XTX", "XBX", "XXX"]),
        // `registerHutRecipe3` (`:351-365`): the item on the whole middle row.
        Shape::Three => (1, ["XTX", "BBB", "XXX"]),
        // `registerHutRecipe1x2` (`:329-343`): the `One` grid, two huts.
        Shape::Double(_) => (2, ["XTX", "XBX", "XXX"]),
    };
    let key = [
        ("X", Input::Tag("minecraft:planks")),
        ("T", Input::Item(BUILD_TOOL)),
        ("B", input),
    ];
    let mut file = shaped(hut, count, pattern, &key);
    if let Shape::Double(suffix) = shape {
        file.path = recipe_path(&format!("{hut}{suffix}"));
    }
    file
}

/// The eleven colony blocks whose grid is theirs alone (`:117-231`).
///
/// Two of them are not huts in the registry sense and are named like one anyway:
/// the scarecrow is `blockhutfield` and the plantation field is
/// `blockhutplantationfield` (`ModBlocks.blockScarecrow`,
/// `ModBlocks.blockPlantationField`). The colony sign is the only one producing
/// six (`:223`), and its id is the sign's own block name —
/// `BlockColonySign.java:56` says `BLOCK_NAME = "colonysign"`, and
/// `BlockColonySign.java:80` returns `new ResourceLocation(MOD_ID, BLOCK_NAME)`
/// — so it is a block id like the two quarries': the three of the eleven that
/// are not spelled `blockhut…` are `simplequarry`, `mediumquarry` and
/// `colonysign`.
///
/// The one grid of `buildHutRecipes` that is **not here** is the assistant
/// hammer (`:243-249`): it crafts an item (`ModItems.assistantHammer_Gold`), not
/// a colony block, and it is one of the 78 recipes this module leaves to the
/// pack (see the module doc).
fn special_huts() -> Vec<File> {
    let planks = Input::Tag("minecraft:planks");
    let tool = Input::Item(BUILD_TOOL);
    vec![
        // `:117-126` — planks, cobblestone, iron, the tool.
        shaped("blockhutcrusher", 1, ["XTX", "CBC", "XXX"], &[
            ("X", planks),
            ("C", Input::Item("minecraft:cobblestone")),
            ("B", Input::Item("minecraft:iron_ingot")),
            ("T", tool),
        ]),
        // `:128-136` — stone instead of planks, and a bone.
        shaped("blockhutgraveyard", 1, ["XTX", "XBX", "XXX"], &[
            ("X", Input::Item("minecraft:stone")),
            ("B", Input::Item("minecraft:bone")),
            ("T", tool),
        ]),
        // `:138-146` — a feather.
        shaped("blockhutschool", 1, ["XTX", "XBX", "XBX"], &[
            ("X", planks),
            ("B", Input::Item("minecraft:feather")),
            ("T", tool),
        ]),
        // `:148-156` — a book.
        shaped("blockhutuniversity", 1, ["XTX", "XBX", "XBX"], &[
            ("X", planks),
            ("B", Input::Item("minecraft:book")),
            ("T", tool),
        ]),
        // `:158-167` — stone bricks and a furnace.
        shaped("blockhutstonesmeltery", 1, ["XTX", "AFA", "XXX"], &[
            ("X", planks),
            ("A", Input::Item("minecraft:stone_bricks")),
            ("F", Input::Item("minecraft:furnace")),
            ("T", tool),
        ]),
        // `:169-177` — the scarecrow: hay *or* pumpkin, leather and sticks, and
        // no build tool in the grid at all (only in the unlock condition).
        shaped("blockhutfield", 1, [" H ", "SLS", " S "], &[
            ("H", Input::Either(&["minecraft:hay_block", "minecraft:pumpkin"])),
            ("L", Input::Item("minecraft:leather")),
            ("S", Input::Item("minecraft:stick")),
        ]),
        // `:179-187` — the plantation field.
        shaped("blockhutplantationfield", 1, [" T ", "SIS", "PPP"], &[
            ("S", Input::Item("minecraft:stick")),
            ("P", planks),
            ("I", Input::Item("minecraft:iron_ingot")),
            ("T", tool),
        ]),
        // `:189-209` — the two quarries, and they differ in one key.
        shaped("simplequarry", 1, ["XTX", "XDX", "XBX"], &[
            ("X", planks),
            ("D", Input::Item("minecraft:iron_pickaxe")),
            ("B", Input::Item("minecraft:barrel")),
            ("T", tool),
        ]),
        shaped("mediumquarry", 1, ["XTX", "XDX", "XBX"], &[
            ("X", planks),
            ("D", Input::Item("minecraft:diamond_pickaxe")),
            ("B", Input::Item("minecraft:barrel")),
            ("T", tool),
        ]),
        // `:212-221` — the gate house buys a bow and an iron sword.
        shaped("blockhutgatehouse", 1, ["XTX", "CXB", "XXX"], &[
            ("X", planks),
            ("B", Input::Item("minecraft:bow")),
            ("C", Input::Item("minecraft:iron_sword")),
            ("T", tool),
        ]),
        // `:223-231` — six signs for six planks.
        shaped("colonysign", 6, ["XXX", "XXX", " T "], &[
            ("X", planks),
            ("T", tool),
        ]),
    ]
}

/// One dough and what it bakes into.
type Dough = (&'static str, &'static str, f64, u32);

/// The six doughs of `:276-304`, with the experience and the divisor of the
/// cooking time.
///
/// The divisor is 8 for the cookie and 1 for everything else: Java writes
/// `300 / 8` (`:287`) rather than 37, so the cook time of a cookie is a division
/// and it is written here as one.
const DOUGHS: [Dough; 6] = [
    ("minecolonies:bread_dough", "minecraft:bread", 0.35, 1),
    ("minecolonies:cake_batter", "minecraft:cake", 0.35, 1),
    ("minecolonies:cookie_dough", "minecraft:cookie", 0.0475, 8),
    ("minecolonies:raw_pumpkin_pie", "minecraft:pumpkin_pie", 0.35, 1),
    ("minecolonies:manchet_dough", "minecolonies:manchet_bread", 0.35, 1),
    ("minecolonies:muffin_dough", "minecolonies:muffin", 0.35, 1),
];

/// The three methods, the recipe type each one writes and its base time in
/// ticks: a furnace takes 300, a smoker 100, a campfire 600 (`:306-364`).
const METHODS: [(&str, &str, u32); 3] = [
    ("", "minecraft:smelting", 300),
    ("smoking", "minecraft:smoking", 100),
    ("campfire_cooking", "minecraft:campfire_cooking", 600),
];

/// The 18 cooking recipes: six doughs × three methods.
pub fn cooking_recipes() -> Vec<File> {
    let mut files = Vec::new();
    for (dough, result, experience, divisor) in DOUGHS {
        for (variant, kind, base_time) in METHODS {
            let id = cooking_variant(dough_base(dough), variant);
            files.push(File {
                path: recipe_path(&id),
                json: obj(vec![
                    ("type", s(kind)),
                    ("category", s("food")),
                    ("cookingtime", i((base_time / divisor) as i64)),
                    ("experience", f(experience)),
                    ("ingredient", obj(vec![("item", s(dough))])),
                    ("result", s(result)),
                ]),
            });
        }
    }
    files
}

/// The recipe id a dough's base carries. Kept separate from the match above
/// because three of the six results are vanilla's and their ids are the mod's.
fn dough_base(dough: &str) -> &'static str {
    match dough {
        "minecolonies:bread_dough" => "baked_bread",
        "minecolonies:cake_batter" => "baked_cake",
        "minecolonies:cookie_dough" => "baked_cookies",
        "minecolonies:raw_pumpkin_pie" => "baked_pumpkin_pie",
        "minecolonies:manchet_dough" => "baked_manchet_bread",
        _ => "baked_muffin",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::json::{f, i, s};
    use crate::generation::{matches_delivered, Pack};

    fn pack() -> Pack {
        Pack::build()
    }

    #[test]
    fn the_43_pattern_huts_and_their_four_variants_are_the_47_the_pack_delivers() {
        let pack = pack();
        let mut checked = 0;
        for (hut, _, _) in HUTS {
            matches_delivered(&pack, &format!("minecolonies/recipe/{hut}.json"));
            checked += 1;
        }
        assert_eq!(checked, 43, "43 rows: the variants are a second table");
        for (hut, _, shape) in VARIANTS {
            if let Shape::Double(suffix) = shape {
                matches_delivered(&pack, &format!("minecolonies/recipe/{hut}{suffix}.json"));
                checked += 1;
            }
        }
        assert_eq!(checked, 47);
    }

    #[test]
    fn the_cheap_pattern_gives_two_and_the_others_give_one() {
        // `registerHutRecipe1x2` is the only one with a count (`:329-343`), and
        // the guard tower uses it with an empty suffix, so `blockhutguardtower`
        // is a two-for-one in a file with no suffix.
        let pack = pack();
        let guard = pack.get("minecolonies/recipe/blockhutguardtower.json").unwrap();
        assert_eq!(guard["result"]["count"], i(2));
        let miner = pack.get("minecolonies/recipe/blockhutminer.json").unwrap();
        assert!(miner["result"].get("count").is_none(), "one hut, no count key");
        let stone = pack.get("minecolonies/recipe/blockhutminerstone.json").unwrap();
        assert_eq!(stone["result"]["count"], i(2));
        assert_eq!(stone["result"]["item"], s("minecolonies:blockhutminer"));
    }

    #[test]
    fn the_eleven_specials_are_the_eleven_grids_of_their_own() {
        let pack = pack();
        for id in [
            "blockhutcrusher",
            "blockhutgraveyard",
            "blockhutschool",
            "blockhutuniversity",
            "blockhutstonesmeltery",
            "blockhutfield",
            "blockhutplantationfield",
            "simplequarry",
            "mediumquarry",
            "blockhutgatehouse",
            "colonysign",
        ] {
            matches_delivered(&pack, &format!("minecolonies/recipe/{id}.json"));
        }
    }

    #[test]
    fn the_scarecrow_is_the_one_grid_without_a_build_tool() {
        // `:169-177`: the tool is in the unlock condition, not in the grid, and
        // the top row is a choice of hay block or pumpkin.
        let pack = pack();
        let scarecrow = pack.get("minecolonies/recipe/blockhutfield.json").unwrap();
        assert!(scarecrow["key"].get("T").is_none());
        let choice = &scarecrow["key"]["H"];
        assert_eq!(choice.as_array().map(|rows| rows.len()), Some(2));
    }

    #[test]
    fn the_two_quarries_differ_in_exactly_one_key() {
        let pack = pack();
        let simple = pack.get("minecolonies/recipe/simplequarry.json").unwrap();
        let medium = pack.get("minecolonies/recipe/mediumquarry.json").unwrap();
        assert_eq!(simple["key"]["D"]["item"], s("minecraft:iron_pickaxe"));
        assert_eq!(medium["key"]["D"]["item"], s("minecraft:diamond_pickaxe"));
        assert_eq!(simple["pattern"], medium["pattern"]);
    }

    #[test]
    fn the_cooking_matrix_is_six_doughs_three_ways() {
        let pack = pack();
        assert!(pack.under("minecolonies/recipe/").len() >= 76);
        for (dough, result, _, _) in DOUGHS {
            for (variant, _, _) in METHODS {
                let id = cooking_variant(dough_base(dough), variant);
                let path = format!("minecolonies/recipe/{id}.json");
                if id != "baked_cake" {
                    // `baked_cake.json` is the one delivered file whose
                    // `category` disagrees with the code that wrote it; see the
                    // next test.
                    matches_delivered(&pack, &path);
                }
                let file = pack.get(&path).unwrap();
                assert_eq!(file["result"], s(result), "{id}");
            }
        }
    }

    #[test]
    fn the_delivered_baked_cake_is_the_one_file_that_says_blocks() {
        // Java passes `RecipeCategory.FOOD` to all three cake recipes
        // (`:281-284`, `:311-314`, `:341-344`), and the delivered
        // `baked_cake_smoking.json` / `baked_cake_campfire_cooking.json` do say
        // `food`. The smelting one says `blocks`. Measured, not copied: this
        // port writes `food` for the three and the difference is stated here
        // instead of being reproduced.
        let pack = pack();
        let ours = pack.get("minecolonies/recipe/baked_cake.json").unwrap();
        let theirs = crate::generation::delivered("minecolonies/recipe/baked_cake.json");
        assert_eq!(ours["category"], s("food"));
        assert_eq!(theirs["category"], s("blocks"));
        for field in ["type", "cookingtime", "experience", "ingredient", "result"] {
            assert_eq!(ours[field], theirs[field], "{field}");
        }
        let mut differing: Vec<String> = Vec::new();
        for file in pack.under("minecolonies/recipe/") {
            let path = &file.path;
            let bytes = std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("data")
                    .join(path),
            );
            if let Ok(bytes) = bytes {
                if let Ok(parsed) = mod_api::serde_json::from_slice::<Json>(&bytes) {
                    if parsed != file.json {
                        differing.push(path.clone());
                    }
                }
            }
        }
        assert_eq!(
            differing,
            vec!["minecolonies/recipe/baked_cake.json".to_owned()],
            "one delivered recipe differs, and it is this one"
        );
    }

    #[test]
    fn the_cookie_is_the_one_row_divided_by_eight() {
        // `300 / 8`, `100 / 8`, `600 / 8` and `0.0475` experience (`:286-289`).
        let pack = pack();
        for (id, time) in [
            ("baked_cookies", 37),
            ("baked_cookies_smoking", 12),
            ("baked_cookies_campfire_cooking", 75),
        ] {
            let file = pack.get(&format!("minecolonies/recipe/{id}.json")).unwrap();
            assert_eq!(file["cookingtime"], i(time), "{id}");
            assert_eq!(file["experience"], f(0.0475), "{id}");
        }
    }

    #[test]
    fn a_furnace_smokes_in_a_third_of_the_time_and_a_campfire_twice_as_long() {
        let pack = pack();
        let smelt = pack.get("minecolonies/recipe/baked_bread.json").unwrap();
        let smoke = pack.get("minecolonies/recipe/baked_bread_smoking.json").unwrap();
        let fire = pack
            .get("minecolonies/recipe/baked_bread_campfire_cooking.json")
            .unwrap();
        assert_eq!(smelt["cookingtime"], i(300));
        assert_eq!(smoke["cookingtime"], i(100));
        assert_eq!(fire["cookingtime"], i(600));
        for file in [smelt, smoke, fire] {
            assert_eq!(file["experience"], f(0.35));
            assert_eq!(file["category"], s("food"));
        }
    }

    #[test]
    fn every_hut_a_recipe_names_is_a_block_of_this_port_and_every_hut_but_one_has_a_recipe() {
        // The cross-check with `crate::blocks`: a hut the port knows and the
        // generator forgets is a gap, and a recipe for a hut the port does not
        // know is a typo.
        let pack = pack();
        for (hut, _, _) in HUTS {
            assert!(
                crate::blocks::block(hut).is_some(),
                "{hut} is not a block of this port"
            );
        }
        let mut without = Vec::new();
        for block in crate::blocks::all() {
            if block.family != crate::blocks::Family::Huts {
                continue;
            }
            // `Family::Huts` holds 54 colony blocks, and two of them are not huts:
            // `BlockPostBox` and `BlockStash` descend from `AbstractColonyBlock`
            // without being `AbstractBlockHut`. Java's `registerHutRecipe*` never
            // names them — their two recipes are among the 74 this module does not
            // write (`DefaultRecipeProvider.buildOtherBlocks`, `:186-263`) — so
            // asking this generator for them would be asking for a file it refuses
            // by design.
            if block.name == "blockpostbox" || block.name == "blockstash" {
                continue;
            }
            let path = format!("minecolonies/recipe/{}.json", block.name);
            if pack.get(&path).is_none() {
                without.push(block.name);
            }
        }
        assert_eq!(without, vec!["blockhutbarrackstower"], "the tower has no recipe");
    }

    #[test]
    fn a_suffix_only_exists_when_java_gives_one() {
        // The four variants carry a suffix; the guard tower was registered with
        // an empty one, so its file has none — the two halves of
        // `constants::append`.
        let pack = pack();
        assert!(pack.get("minecolonies/recipe/blockhutdeliverymaniron.json").is_some());
        assert!(pack.get("minecolonies/recipe/blockhutfarmerstone.json").is_some());
        assert!(pack.get("minecolonies/recipe/blockhutlumberjackstone.json").is_some());
        assert!(pack.get("minecolonies/recipe/blockhutminerstone.json").is_some());
        assert!(pack.get("minecolonies/recipe/blockhutguardtower.json").is_some());
        assert!(pack.get("minecolonies/recipe/blockhutguardtowerbow.json").is_none());
    }

    #[test]
    fn the_warehouse_is_the_one_hut_that_costs_a_tag() {
        let pack = pack();
        let warehouse = pack.get("minecolonies/recipe/blockhutwarehouse.json").unwrap();
        assert_eq!(warehouse["key"]["B"]["tag"], s("forge:chests"));
    }
}
