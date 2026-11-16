//! What the **14 materials** and the **4 bottles** are for, read out of the tables
//! that already exist.
//!
//! Java reference: `ModItems.getAllIngredients()` (`api/items/ModItems.java:172-182`,
//! the seven), the four dough classes (`ItemBreadDough`, `ItemCookieDough`,
//! `ItemCakeBatter`, `ItemRawPumpkinPie`, 19 lines each), `ItemCompost.java` (83),
//! `ItemMistletoe.java` (19), `ItemLargeBottle.java` (122) and the two recipe
//! providers that say what each one becomes: `DefaultRecipeProvider.java` for the
//! vanilla-crafting recipes and `defaults/workers/*CraftingProvider.java` for the
//! hut recipes.
//!
//! **Two lookups, and neither is a copy of a table.**
//!
//! 1. [`job_consumers`] and [`job_producers`] walk `crate::jobs::kinds` — the 49
//!    rows that job module already carries — so "the farmer takes compost" is read
//!    from `EntityAIWorkFarmer.java:297` through that table, not restated here.
//!    Change the farmer's filter there and this module follows.
//! 2. [`Material::uses`] names the **recipe** each item feeds, with the provider
//!    line that declares it. A recipe is data (`data/minecolonies/recipe/`, 154
//!    files in this tree) and the recipe *format* is another slice's; what a
//!    material is *for* is a fact about the item, which is why it lives here.
//!
//! **The finding, and it is the same one twice.** `getAllIngredients()` and the
//! four batters are **not the same list**: the seven ingredients include
//! `muffin_dough` and `manchet_dough`, while `bread_dough`, `cookie_dough`,
//! `cake_batter` and `raw_pumpkin_pie` are in no list at all — they bake into
//! **vanilla** bread, cookies, cake and pumpkin pie
//! (`DefaultRecipeProvider.java:455-472`), so the mod's four most common
//! intermediates never appear in its own ingredient tab.
//!
//! **What is not here.** No recipe: what a recipe *costs* and *yields* is
//! `data/minecolonies/recipe/`, and `MANQUANTS.md` at the crate root lists the
//! 143 items those files still name that this port has no declaration for. No
//! crafting: the chef, the baker and the alchemist do that through
//! `crate::jobs::kinds`, and the tick that runs them does not exist yet.

use crate::jobs::Job;
use crate::jobs::kinds::{Output, all as all_rules};

/// `ModItems.getAllIngredients()`, in Java's order (`ModItems.java:172-182`).
///
/// Seven items, and Java reads the list three times: the composter, where it
/// registers **nothing** because none of them has food properties
/// (`ModItemsInitializer.java:473-475`, see [`super::food::compost_chance`]), the
/// creative ingredient tab (`api/creativetab/ModCreativeTabs.java:179`) and the
/// model provider (`DefaultItemModelProvider.java:38`). None of the three is a
/// rule about the item, which is why the list is here and the three readers are
/// not.
pub const INGREDIENTS: &[&str] = &[
    "muffin_dough",
    "manchet_dough",
    "raw_noodle",
    "butter",
    "cornmeal",
    "creamcheese",
    "soysauce",
];

/// The four doughs that bake into a **vanilla** item, and that
/// `getAllIngredients()` does not name.
pub const BATTERS: &[&str] = &["bread_dough", "cookie_dough", "cake_batter", "raw_pumpkin_pie"];

/// The two things a colony spends on a block rather than in a recipe.
pub const CATALYSTS: &[&str] = &["compost", "mistletoe"];

/// What a material is, which is also how Java files it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: the four ways Java groups the fourteen
pub enum MaterialKind {
    /// The seven of `getAllIngredients()`.
    Ingredient,
    /// The four that bake into vanilla bread, cookies, cake and pie.
    Batter,
    /// A block-growth item or a brewing herb: `compost`, `mistletoe`.
    Catalyst,
    /// A data-carrying token: `adventure_token`, matched by name and read for its
    /// NBT, never crafted.
    Token,
}

impl MaterialKind {
    /// A stable name, for a refusal or a test failure.
    pub fn name(self) -> &'static str {
        match self {
            MaterialKind::Ingredient => "ingredient",
            MaterialKind::Batter => "batter",
            MaterialKind::Catalyst => "catalyst",
            MaterialKind::Token => "token",
        }
    }
}

/// One recipe a material feeds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Use {
    /// The recipe id the datagen saves: `manchet_dough`, `pasta_plain`.
    pub recipe: &'static str,
    /// The hut that runs it, or `None` for a vanilla crafting recipe the player
    /// runs at a bench.
    pub by: Option<Job>,
    /// `fichier.java:ligne` of the `.inputs(...)`/`.requires(...)` that names this
    /// item — not the `.save(...)`, which names the recipe.
    pub source: &'static str,
}

/// Build a [`Use`] for a vanilla-crafting recipe.
const fn craft(recipe: &'static str, source: &'static str) -> Use {
    Use { recipe, by: None, source }
}

/// Build a [`Use`] for a hut recipe.
const fn at(recipe: &'static str, by: Job, source: &'static str) -> Use {
    Use { recipe, by: Some(by), source }
}

/// One material: what makes it, and what it makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Material {
    /// The registry path, without namespace.
    pub item: &'static str,
    /// Which of the four groups it belongs to.
    pub kind: MaterialKind,
    /// The hut that crafts it, when a hut does.
    pub made_by: Option<Job>,
    /// What it feeds, in provider order.
    pub uses: &'static [Use],
    /// `fichier.java:ligne` of the declaration, or of the recipe that makes it
    /// when there is one.
    pub source: &'static str,
}

/// Build a material row.
const fn mat(
    item: &'static str,
    kind: MaterialKind,
    made_by: Option<Job>,
    uses: &'static [Use],
    source: &'static str,
) -> Material {
    Material { item, kind, made_by, uses, source }
}

/// The **14** materials, in `getAllIngredients` order, then the four batters, then
/// the two catalysts and the token.
pub const ALL: &[Material] = &[
    // `muffin_dough`, made by the baker (`DefaultBakerCraftingProvider.java:204`)
    // from two durum, butter, sugar, an egg and sweet berries; baked into a muffin
    // by the same hut (`:219`).
    mat("muffin_dough", MaterialKind::Ingredient, Some(Job::Baker),
        &[at("muffin", Job::Baker, "DefaultBakerCraftingProvider.java:219")],
        "DefaultBakerCraftingProvider.java:204"),
    // `manchet_dough`, two per craft from two durum and butter (`:193`), then
    // `manchet_bread` (`:226`).
    mat("manchet_dough", MaterialKind::Ingredient, Some(Job::Baker),
        &[at("manchet", Job::Baker, "DefaultBakerCraftingProvider.java:226")],
        "DefaultBakerCraftingProvider.java:193"),
    // `raw_noodle`, a shapeless durum recipe the **player** runs
    // (`DefaultRecipeProvider.java:795`), then ramen at the chef's (`:176`).
    mat("raw_noodle", MaterialKind::Ingredient, None,
        &[at("ramen", Job::Chef, "DefaultChefCraftingProvider.java:176")],
        "DefaultRecipeProvider.java:795"),
    // `butter`, three per craft from six large milk bottles (`:719`) or six soy
    // milk bottles (`soy_butter`, `:726`) — and the most used ingredient of the
    // mod: two recipes at a bench, three in the bakery.
    mat("butter", MaterialKind::Ingredient, None,
        &[
            at("lembas_scone", Job::Baker, "DefaultBakerCraftingProvider.java:181"),
            at("manchet_dough", Job::Baker, "DefaultBakerCraftingProvider.java:192"),
            at("muffin_dough", Job::Baker, "DefaultBakerCraftingProvider.java:200"),
            craft("pasta_plain", "DefaultRecipeProvider.java:799"),
            craft("plain_cheesecake", "DefaultRecipeProvider.java:1090"),
        ],
        "DefaultRecipeProvider.java:719"),
    // `cornmeal`, three per craft (`:768`), then polenta and tortillas.
    mat("cornmeal", MaterialKind::Ingredient, None,
        &[
            craft("polenta", "DefaultRecipeProvider.java:931"),
            craft("tortillas", "DefaultRecipeProvider.java:954"),
        ],
        "DefaultRecipeProvider.java:768"),
    // `creamcheese`, two per craft (`:746`, soy variant `:753`), then one recipe.
    mat("creamcheese", MaterialKind::Ingredient, None,
        &[craft("plain_cheesecake", "DefaultRecipeProvider.java:1087")],
        "DefaultRecipeProvider.java:746"),
    // `soysauce`, one shapeless recipe (`:763`), spent only in ramen.
    mat("soysauce", MaterialKind::Ingredient, None,
        &[at("ramen", Job::Chef, "DefaultChefCraftingProvider.java:177")],
        "DefaultRecipeProvider.java:763"),

    // -- the four batters, which bake into a **vanilla** item --
    // `bread_dough`, made by the baker twice over (`:54`, `:61`), smelted into
    // `minecraft:bread` in 300 ticks (`DefaultRecipeProvider.java:455`).
    mat("bread_dough", MaterialKind::Batter, Some(Job::Baker),
        &[craft("baked_bread", "DefaultRecipeProvider.java:455")],
        "DefaultBakerCraftingProvider.java:54"),
    mat("cookie_dough", MaterialKind::Batter, Some(Job::Baker),
        &[craft("baked_cookies", "DefaultRecipeProvider.java:465")],
        "DefaultBakerCraftingProvider.java:118"),
    // `cake_batter`, and the soy variant `scake_batter` (`:160`).
    mat("cake_batter", MaterialKind::Batter, Some(Job::Baker),
        &[craft("baked_cake", "DefaultRecipeProvider.java:460")],
        "DefaultBakerCraftingProvider.java:149"),
    mat("raw_pumpkin_pie", MaterialKind::Batter, Some(Job::Baker),
        &[craft("baked_pumpkin_pie", "DefaultRecipeProvider.java:470")],
        "DefaultBakerCraftingProvider.java:133"),

    // -- the two catalysts --
    // `compost`: no recipe makes it (the composter building does, six at a time —
    // `crate::jobs::kinds` says so), and one vanilla recipe takes it, for a double
    // fern (`DefaultRecipeProvider.java:384`).
    mat("compost", MaterialKind::Catalyst, None,
        &[craft("doublefern", "DefaultRecipeProvider.java:384")],
        "ItemCompost.java:31"),
    // `mistletoe`: grown, not crafted; spent at the alchemist's with a large water
    // bottle to make the one magic potion
    // (`DefaultAlchemistCraftingProvider.java:41`).
    mat("mistletoe", MaterialKind::Catalyst, None,
        &[at("magicpotion", Job::Alchemist, "DefaultAlchemistCraftingProvider.java:41")],
        "ItemMistletoe.java:17"),

    // -- the token --
    // Matched by name and read for `entity_type`; no recipe in any provider.
    mat("adventure_token", MaterialKind::Token, None, &[], "ItemAdventureToken.java:28"),
];

/// The material rule an id names, or `None`.
pub fn of(item: &str) -> Option<&'static Material> {
    ALL.iter().find(|material| material.item == item)
}

/// The material rule an id names, or a named refusal.
pub fn require(item: &'static str) -> Result<&'static Material, super::Refusal> {
    of(item).ok_or(super::Refusal::NotAMaterial { id: item })
}

/// Every material of a kind, in table order.
pub fn of_kind(kind: MaterialKind) -> impl Iterator<Item = &'static Material> {
    ALL.iter().filter(move |material| material.kind == kind)
}

/// The jobs whose `accepts` table names this item — `crate::jobs::kinds` walked,
/// never copied.
///
/// The row's item is compared **with** its namespace (`material("minecolonies:compost", …)`),
/// **without** it (how a caller usually writes it), and **as a tag**
/// (`material("#minecolonies:meshes", …)`) — the three spellings
/// `crate::jobs::kinds` uses. In table order, no duplicates: a job that lists
/// compost twice is one job.
///
/// Four items of this module are named by a job, and the four are the link this
/// function exists for: `compost` (the florist, the planter and the farmer **take**
/// it), `mistletoe` (the alchemist), `magicpotion` (the druid) and `meshes`, which
/// the sifter takes as a tag.
pub fn job_consumers(item: &str) -> Vec<Job> {
    let prefixed = format!("{}:{}", super::NAMESPACE, item);
    let tag = format!("#{}", prefixed);
    let mut jobs = Vec::new();
    for rules in all_rules() {
        let takes = rules.accepts.iter().any(|accepted| {
            accepted.item == prefixed || accepted.item == item || accepted.item == tag
        });
        if takes && !jobs.contains(&rules.job) {
            jobs.push(rules.job);
        }
    }
    jobs
}

/// The jobs whose `produces` table names this item as a **fixed** stack — the
/// composter's six compost (`kinds::ferme.rs:208`, the only fixed output of the
/// port that is a MineColonies item) answers one entry.
///
/// A recipe output is `Output::Recipe`: the item is not named there, so a job that
/// makes this item through a recipe does not appear, and [`Material::made_by`] is
/// where that answer lives.
pub fn job_producers(item: &str) -> Vec<Job> {
    let prefixed = format!("{}:{}", super::NAMESPACE, item);
    let mut jobs = Vec::new();
    for rules in all_rules() {
        let makes = rules.produces.iter().any(|product| match product.output {
            Output::Fixed { item: made, .. } => made == prefixed || made == item,
            Output::Drops | Output::Recipe | Output::Entity(_) => false,
        });
        if makes && !jobs.contains(&rules.job) {
            jobs.push(rules.job);
        }
    }
    jobs
}

/// The item a craft of `item` leaves in the grid, when Java says it does.
///
/// Three of the four bottles say `craftRemainder(large_empty_bottle)`
/// (`ModItemsInitializer.java:242-244`); the empty one itself says nothing,
/// because it is what the other three leave.
pub fn craft_remainder(item: &str) -> Option<&'static str> {
    match item {
        "large_water_bottle" | "large_milk_bottle" | "large_soy_milk_bottle" => {
            Some("large_empty_bottle")
        }
        _ => None,
    }
}

/// The block an item places, for a declaration's `block` field.
///
/// Three items are block items, and the block is one of this mod's own, so the
/// name is bare (`content::Item.block`: "one of this mod's (a bare name is this
/// mod's)"); `crate::blocks::block` knows all three.
pub fn block_of(item: &str) -> Option<&'static str> {
    match item {
        "gate_iron" => Some("gate_iron"),
        "gate_wood" => Some("gate_wood"),
        "colony_banner" => Some("colony_banner"),
        _ => None,
    }
}

/// How long a large bottle is on cooldown after being **filled with water**, in
/// ticks: `TICKS_SECOND` (`ItemLargeBottle.java:115`), one second.
pub const WATER_FILL_COOLDOWN: i32 = 20;

/// How long a large bottle is on cooldown after **milking** a cow or a goat:
/// `TICKS_SECOND * 10` (`ItemLargeBottle.java:68`, `:79`), ten seconds.
pub const MILKING_COOLDOWN: i32 = 200;

/// What a large bottle becomes when it is used on a filled cauldron or a cow.
///
/// Only the **empty** bottle is a tool: `if (this != ModItems.large_empty_bottle)
/// return super...` (`ItemLargeBottle.java:53-55`), so a full bottle is inert.
/// Milking a cow and a goat both give a `large_milk_bottle`, baby animals refuse,
/// and a water source gives a `large_water_bottle`
/// (`ItemLargeBottle.java:60-121`).
pub fn bottle_becomes(item: &str, from: BottleSource) -> Option<(&'static str, i32)> {
    if item != "large_empty_bottle" {
        return None;
    }
    match from {
        BottleSource::Cow | BottleSource::Goat => Some(("large_milk_bottle", MILKING_COOLDOWN)),
        BottleSource::Water => Some(("large_water_bottle", WATER_FILL_COOLDOWN)),
        BottleSource::Air | BottleSource::BabyAnimal => None,
    }
}

/// What a large bottle was used on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four answers a right click can give, plus the miss
pub enum BottleSource {
    /// A water source block, `FluidTags.WATER` (`ItemLargeBottle.java:107`).
    Water,
    /// A grown cow (`:60`).
    Cow,
    /// A grown goat (`:71`).
    Goat,
    /// A calf or a kid: `!entity.isBaby()` guards both branches.
    BabyAnimal,
    /// Anything else, including air.
    Air,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::{get, Category, Refusal};

    #[test]
    fn the_table_holds_the_fourteen_materials() {
        assert_eq!(ALL.len(), 14);
        assert_eq!(INGREDIENTS.len(), 7);
        assert_eq!(BATTERS.len(), 4);
        assert_eq!(CATALYSTS.len(), 2);
        assert_eq!(of_kind(MaterialKind::Ingredient).count(), 7);
        assert_eq!(of_kind(MaterialKind::Batter).count(), 4);
        assert_eq!(of_kind(MaterialKind::Catalyst).count(), 2);
        assert_eq!(of_kind(MaterialKind::Token).count(), 1);
    }

    #[test]
    fn every_material_is_a_material_row_of_the_item_table() {
        for material in ALL {
            let entry = get(material.item).unwrap_or_else(|| panic!("{} has no row", material.item));
            assert_eq!(entry.category, Category::Material, "{}", material.item);
            assert_eq!(of(material.item), Some(material));
        }
        // and the reverse: the table's fourteen are exactly these
        let table: Vec<&str> = crate::items::of_category(Category::Material)
            .map(|entry| entry.id)
            .collect();
        for id in table {
            assert!(of(id).is_some(), "{id} has no material rule");
        }
    }

    #[test]
    fn the_two_lists_are_not_the_same_list() {
        // `getAllIngredients()` names muffin_dough and manchet_dough; the four
        // batters are named by no list at all.
        for id in INGREDIENTS {
            assert!(!BATTERS.contains(id), "{id} is in both lists");
        }
        assert!(INGREDIENTS.contains(&"muffin_dough"));
        assert!(!INGREDIENTS.contains(&"bread_dough"));
        assert!(BATTERS.contains(&"bread_dough"));
    }

    #[test]
    fn the_four_batters_bake_into_a_vanilla_item() {
        for id in BATTERS {
            let material = of(id).unwrap();
            assert!(!material.uses.is_empty(), "{id} bakes into nothing");
            for use_ in material.uses {
                assert_eq!(use_.by, None, "{id} bakes at a bench, not in a hut");
                assert!(use_.recipe.starts_with("baked_"), "{id} -> {}", use_.recipe);
            }
            assert_eq!(material.made_by, Some(Job::Baker), "{id}");
        }
    }

    #[test]
    fn no_material_has_two_rows_and_every_row_cites_a_java_line() {
        for (index, material) in ALL.iter().enumerate() {
            for other in &ALL[index + 1..] {
                assert_ne!(material.item, other.item);
            }
            assert!(material.source.contains(".java:"), "{}", material.item);
            for use_ in material.uses {
                assert!(use_.source.contains(".java:"), "{} -> {}", material.item, use_.recipe);
                assert!(!use_.recipe.is_empty());
            }
        }
    }

    #[test]
    fn the_kind_names_are_unique_and_non_empty() {
        let kinds = [
            MaterialKind::Ingredient,
            MaterialKind::Batter,
            MaterialKind::Catalyst,
            MaterialKind::Token,
        ];
        for (index, kind) in kinds.iter().enumerate() {
            for other in &kinds[index + 1..] {
                assert_ne!(kind.name(), other.name());
            }
        }
        assert_eq!(MaterialKind::Ingredient.name(), "ingredient");
        assert_eq!(MaterialKind::Token.name(), "token");
    }

    #[test]
    fn the_farmer_the_florist_and_the_planter_take_compost() {
        let mut consumers = job_consumers("compost");
        consumers.sort_by_key(|job| job.path());
        assert_eq!(consumers, vec![Job::Farmer, Job::Florist, Job::Planter]);
        // read out of `crate::jobs::kinds`, not restated:
        let mut from_kinds: Vec<Job> = crate::jobs::kinds::all()
            .filter(|rules| rules.accepts.iter().any(|a| a.item == "minecolonies:compost"))
            .map(|rules| rules.job)
            .collect();
        from_kinds.sort_by_key(|job| job.path());
        assert_eq!(consumers, from_kinds);
        // and the sifter, whose material is a mesh tag rather than an item name
        assert_eq!(job_consumers("meshes"), vec![Job::Sifter]);
    }

    #[test]
    fn the_composter_is_the_only_producer_of_six_compost() {
        // **One** producer, not two. The composter's recipe yields six
        // (`api/crafting/CompostRecipe.java:31` `COMPOST_RESULT = 6`, read into the
        // output at `:44`), and `kinds::ferme.rs:208` carries that row.
        //
        // The swineherd was expected here and does not belong: its AI is a plain
        // herder (`EntityAIWorkSwineHerder.java:13`) and its building eats
        // **carrots only** (`BuildingSwineHerder.java:55-57`). No Java line makes
        // it produce compost, so the port does not either — adding one would have
        // been an invented rule in someone else's table.
        let producers = job_producers("compost");
        assert_eq!(producers, vec![Job::Composter]);
        // the count is the table's, and it is six
        for rules in crate::jobs::kinds::all() {
            for product in rules.produces {
                if let Output::Fixed { item, count } = product.output {
                    if item == "minecolonies:compost" {
                        assert_eq!(count, 6, "{}", rules.job.path());
                    }
                }
            }
        }
        // and no job but one names it in `produces`
        let naming: Vec<&str> = crate::jobs::kinds::all()
            .filter(|rules| {
                rules.produces.iter().any(|product| {
                    matches!(product.output, Output::Fixed { item, .. } if item == "minecolonies:compost")
                })
            })
            .map(|rules| rules.job.path())
            .collect();
        assert_eq!(naming, vec!["composter"]);
    }

    #[test]
    fn the_alchemist_takes_the_mistletoe_and_the_druid_the_magic_potion() {
        assert_eq!(job_consumers("mistletoe"), vec![Job::Alchemist]);
        assert_eq!(job_consumers("magicpotion"), vec![Job::Druid]);
        // `magicpotion` is a Potion of the item table, not a Material: the job
        // link exists for an item this table files elsewhere.
        assert_eq!(get("magicpotion").unwrap().category, Category::Potion);
    }

    #[test]
    fn nothing_else_is_named_by_a_job() {
        // The ingredients and the batters reach a hut through a *recipe*, not
        // through its `accepts` filter: the empty answer is the true one.
        for material in ALL {
            if matches!(
                material.item,
                "compost" | "mistletoe" | "magicpotion"
            ) {
                continue;
            }
            assert!(
                job_consumers(material.item).is_empty(),
                "{} has a consumer",
                material.item
            );
        }
    }

    #[test]
    fn the_baker_makes_six_of_the_fourteen() {
        let made: Vec<&str> = ALL
            .iter()
            .filter(|material| material.made_by == Some(Job::Baker))
            .map(|material| material.item)
            .collect();
        assert_eq!(
            made,
            vec![
                "muffin_dough",
                "manchet_dough",
                "bread_dough",
                "cookie_dough",
                "cake_batter",
                "raw_pumpkin_pie",
            ]
        );
        assert_eq!(made.len(), 6);
        // and the other eight are made at a bench, by the player
        assert_eq!(ALL.iter().filter(|material| material.made_by.is_none()).count(), 8);
        for material in ALL {
            if let Some(job) = material.made_by {
                assert_eq!(job, Job::Baker, "{} is made by {job:?}", material.item);
            }
        }
    }

    #[test]
    fn a_non_material_is_refused_by_name() {
        assert_eq!(require("spear"), Err(Refusal::NotAMaterial { id: "spear" }));
        assert_eq!(Refusal::NotAMaterial { id: "spear" }.name(), "not_a_material");
        assert_eq!(of("cheddar_cheese"), None, "a food is not a material");
    }

    #[test]
    fn three_bottles_leave_an_empty_one_and_the_empty_one_leaves_nothing() {
        assert_eq!(craft_remainder("large_milk_bottle"), Some("large_empty_bottle"));
        assert_eq!(craft_remainder("large_water_bottle"), Some("large_empty_bottle"));
        assert_eq!(craft_remainder("large_soy_milk_bottle"), Some("large_empty_bottle"));
        assert_eq!(craft_remainder("large_empty_bottle"), None);
        assert_eq!(craft_remainder("spear"), None);
    }

    #[test]
    fn a_declaration_of_a_bottle_carries_the_remainder_it_leaves() {
        let entry = get("large_milk_bottle").unwrap();
        // no template: the bottle's model is an asset decision, so the whole
        // declaration is refused before the remainder is reached.
        assert_eq!(entry.declaration(), Err(Refusal::NoTemplate { id: "large_milk_bottle" }));
        assert_eq!(entry.stated_stack(), Ok(None));
    }

    #[test]
    fn a_full_bottle_is_inert_and_an_empty_one_fills() {
        assert_eq!(
            bottle_becomes("large_empty_bottle", BottleSource::Cow),
            Some(("large_milk_bottle", 200))
        );
        assert_eq!(
            bottle_becomes("large_empty_bottle", BottleSource::Goat),
            Some(("large_milk_bottle", 200))
        );
        assert_eq!(
            bottle_becomes("large_empty_bottle", BottleSource::Water),
            Some(("large_water_bottle", 20))
        );
        assert_eq!(bottle_becomes("large_empty_bottle", BottleSource::BabyAnimal), None);
        assert_eq!(bottle_becomes("large_empty_bottle", BottleSource::Air), None);
        // a full bottle is not the tool: `if (this != ModItems.large_empty_bottle)`
        assert_eq!(bottle_becomes("large_milk_bottle", BottleSource::Cow), None);
        assert_eq!(bottle_becomes("large_water_bottle", BottleSource::Water), None);
    }

    #[test]
    fn the_milking_cooldown_is_ten_seconds_and_the_water_one() {
        assert_eq!(WATER_FILL_COOLDOWN, 20);
        assert_eq!(MILKING_COOLDOWN, 200);
        assert_eq!(MILKING_COOLDOWN, WATER_FILL_COOLDOWN * 10);
    }

    #[test]
    fn the_three_block_items_name_a_block_the_blocks_module_has() {
        let mut named = 0;
        for entry in crate::items::of_category(Category::Block) {
            let block = block_of(entry.id);
            assert_eq!(block, Some(entry.id), "{} places nothing", entry.id);
            assert!(crate::blocks::block(entry.id).is_some());
            named += 1;
        }
        assert_eq!(named, 3);
        assert_eq!(block_of("spear"), None);
        assert_eq!(block_of("large_empty_bottle"), None);
    }

    #[test]
    fn a_declaration_of_a_gate_names_the_block_it_places() {
        let entry = get("gate_iron").unwrap();
        // a gate has no template either, so the refusal comes first — the second
        // half of this test is the field the declaration would carry.
        assert_eq!(entry.declaration(), Err(Refusal::NoTemplate { id: "gate_iron" }));
        assert_eq!(block_of("gate_iron"), Some("gate_iron"));
    }
}
