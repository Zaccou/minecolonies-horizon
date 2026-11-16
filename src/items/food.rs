//! What the **79 edible items** feed you, and what they do besides feeding you.
//!
//! Java reference: `api/items/ModItems.java` (the three `getAllFoods` lists,
//! `:196-246`), `apiimp/initializer/ModItemsInitializer.java:143-230` (the sixty
//! `ItemFood`/`ItemBowlFood` rows and their numbers), the four bread classes
//! (`ItemMilkyBread`, `ItemSugaryBread`, `ItemGoldenBread`, `ItemChorusBread`),
//! `core/items/ItemFood.java` (83 lines), `ItemBowlFood.java` (84) and
//! `ItemCrop.java:49` for the fifteen crops.
//!
//! **79 = 60 + 4 + 15**, and the three groups are not the same kind of item:
//!
//! | Group | Items | Class | Has a tier |
//! | --- | --- | --- | --- |
//! | `ItemFood` / `ItemBowlFood` | 60 | `Item` + `IMinecoloniesFoodItem` | yes, 1 to 3 |
//! | the four breads | 4 | `ItemFood` subclasses | 1, 1, 1, 2 |
//! | the crops | 15 | `ItemCrop` (`BlockItem` + food) | **no** |
//!
//! **Three numbers a reader will get wrong.**
//!
//! 1. `saturationMod` is a **multiplier, not points**. Vanilla's
//!    `FoodData.add` adds `nutrition * saturationMod * 2` saturation points, so a
//!    tier-1 dish at `0.1` restores 6 hunger and **1.2** saturation, and a
//!    `chorus_bread` at `2.0` restores 5 hunger and **20** saturation. This is
//!    vanilla arithmetic (`net.minecraft.world.food.FoodData#add`), not a
//!    MineColonies line: this tree carries no vanilla source, so
//!    [`saturation_points`] states the formula and `MANQUANT.md` §3 says where it
//!    comes from.
//! 2. Java's `getAllFoods()` lists **59** of the 60, and the missing one is
//!    `fried_rice` (`ModItemsInitializer.java:366`): it is a food, it is served,
//!    and it is the **one food that is not compostable**.
//! 3. The four breads are registered for compost **one by one**
//!    (`ModItemsInitializer.java:468-471`), not through `getAllFoods`, which is
//!    the only reason a bread is compostable at all.
//!
//! **What is not here.** No eating: the `finishUsingItem` overrides are
//! consequences ([`AfterEating`]), and the hook that runs one is
//! `item_use_finish`, which `src/lib.rs` does not subscribe to (`MANQUANT.md` §2).
//! No restaurant, no menu, no tier gating: which tier a hut may serve is
//! `core/colony/buildings/modules/RestaurantMenuModule`, a building rule.

use super::{Refusal, STACKSIZE};

/// The vanilla eating time of an edible item, in ticks:
/// `Item.getUseDuration` answers `32` for a food and `0` for anything else.
///
/// A vanilla constant this tree does not carry — every MineColonies call site is
/// `super.getUseDuration(stack) * Math.max(1, tier)`
/// (`ItemFood.java:73-77`), which takes the base as read. Stated here so
/// [`use_ticks`] is a formula and not a magic number, and flagged in
/// `MANQUANT.md` §3.
pub const ITEM_USE_TICKS: i32 = 32;

/// `Constants.STACKSIZE`, the limit of a food stack: Java states none, and a food
/// takes its template's.
pub const FOOD_STACK: u8 = STACKSIZE;

/// What an edible item is made of, which is what it leaves behind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: three superclasses, and a fourth shape of food would be a fourth
pub enum FoodForm {
    /// `new ItemFood(...)`: eaten whole, leaves nothing.
    Solid,
    /// `new ItemBowlFood(...)`: `finishUsingItem` hands back a `minecraft:bowl`
    /// to a player, and to nobody else (`ItemBowlFood.java:43-64`).
    Bowl,
    /// `new ItemCrop(block, properties.food(1, 0.3F))`, a `BlockItem` that is
    /// also food (`ItemCrop.java:49`).
    Crop,
}

impl FoodForm {
    /// What the last bite leaves in the eater's hand.
    pub fn remainder(self) -> Option<&'static str> {
        match self {
            FoodForm::Bowl => Some("minecraft:bowl"),
            FoodForm::Solid | FoodForm::Crop => None,
        }
    }
}

/// What eating one **does** once the hunger is restored — the body of each
/// `finishUsingItem` override.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: the five overrides Java writes, and no item does two of them
pub enum AfterEating {
    /// Nothing: 75 of the 79, `ItemFood` as it stands.
    Nothing,
    /// `entityLiving.heal(4)` — two hearts, in half-hearts
    /// (`ItemGoldenBread.java:53`).
    Heal(u8),
    /// `entityLiving.curePotionEffects(new ItemStack(Items.MILK_BUCKET))` — every
    /// effect, as drinking milk (`ItemMilkyBread.java:52`).
    CureAll,
    /// `entityLiving.removeEffect(MobEffects.POISON)` — poison only, not the other
    /// effects (`ItemSugaryBread.java:54`).
    CurePoison,
    /// `TeleportHelper.surfaceTeleport(player)`, overworld only
    /// (`ItemChorusBread.java:55`).
    SurfaceTeleport,
}

/// An effect a food grants on being eaten, with the chance it is granted at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effect {
    /// The registry path of the mob effect: `minecraft:speed`.
    pub effect: &'static str,
    /// How long it lasts, in ticks.
    pub ticks: i32,
    /// The amplifier, 0 being level I.
    pub amplifier: i32,
    /// `FoodProperties.effect`'s chance, `1.0` when Java does not say.
    pub chance: f64,
    /// `fichier.java:ligne` of the builder call.
    pub source: &'static str,
}

/// One edible item: its hunger points, its saturation multiplier, its tier, and
/// what it leaves or does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Food {
    /// The registry path, without namespace.
    pub item: &'static str,
    /// `FoodProperties.nutrition`: half-drumsticks, 1 to 13 in this mod.
    pub nutrition: u8,
    /// `FoodProperties.saturationMod`: a **multiplier**, one of `0.1`, `0.25`,
    /// `0.3`, `0.6`, `0.7`, `2.0`.
    pub saturation: f64,
    /// `IMinecoloniesFoodItem.getTier`, 1 to 3 — `None` for the fifteen crops,
    /// which are not `IMinecoloniesFoodItem` at all.
    pub tier: Option<u8>,
    /// Solid, bowl or crop.
    pub form: FoodForm,
    /// What happens after the bite.
    pub after: AfterEating,
    /// The effect granted on eating, if any.
    pub effect: Option<Effect>,
    /// `FoodProperties.alwaysEat()`: edible at full hunger.
    pub always_eat: bool,
    /// `fichier.java:ligne` of the numbers — the initialiser line for the sixty
    /// and the crops, the class's builder for the four breads.
    pub source: &'static str,
}

/// Build a plain solid food row.
const fn f(
    item: &'static str,
    nutrition: u8,
    saturation: f64,
    tier: u8,
    source: &'static str,
) -> Food {
    Food {
        item,
        nutrition,
        saturation,
        tier: Some(tier),
        form: FoodForm::Solid,
        after: AfterEating::Nothing,
        effect: None,
        always_eat: false,
        source,
    }
}

/// Build a bowl food row: same numbers, a bowl back.
const fn bowl(
    item: &'static str,
    nutrition: u8,
    saturation: f64,
    tier: u8,
    source: &'static str,
) -> Food {
    Food {
        item,
        nutrition,
        saturation,
        tier: Some(tier),
        form: FoodForm::Bowl,
        after: AfterEating::Nothing,
        effect: None,
        always_eat: false,
        source,
    }
}

/// Build a crop row: `ItemCrop`, no tier, always a solid bite.
const fn crop(item: &'static str) -> Food {
    Food {
        item,
        nutrition: 1,
        saturation: 0.3,
        tier: None,
        form: FoodForm::Crop,
        after: AfterEating::Nothing,
        effect: None,
        always_eat: false,
        source: "ItemCrop.java:49",
    }
}

/// The fifteen crops, in `ModBlocksInitializer.java:132-147` order. Their item
/// half is here; their block half — the soil, the `age` 0..6, the biome tag — is
/// `crate::blocks::ressource`.
///
/// They are rows of [`ALL`] like the rest and are counted by
/// [`tests::the_table_holds_the_seventy_nine_edibles`]; this constant is the
/// count, so a sixteenth crop fails one assertion instead of shifting five.
pub const CROP_COUNT: usize = 15;

/// The **79** edibles: the sixty `ItemFood` rows in registration order, the four
/// breads, then the fifteen crops.
pub const ALL: &[Food] = &[
    // -- tier 1: nutrition 6 or 7, saturation 0.1 --
    f("cheddar_cheese", 6, 0.1, 1, "ModItemsInitializer.java:143"),
    f("feta_cheese", 6, 0.1, 1, "ModItemsInitializer.java:144"),
    bowl("cooked_rice", 6, 0.1, 1, "ModItemsInitializer.java:145"),
    f("tofu", 6, 0.1, 1, "ModItemsInitializer.java:146"),
    f("flatbread", 6, 0.1, 1, "ModItemsInitializer.java:147"),
    f("cheese_ravioli", 7, 0.1, 1, "ModItemsInitializer.java:148"),
    f("chicken_broth", 6, 0.1, 1, "ModItemsInitializer.java:149"),
    f("meat_ravioli", 7, 0.1, 1, "ModItemsInitializer.java:150"),
    f("mint_jelly", 7, 0.1, 1, "ModItemsInitializer.java:151"),
    f("mint_tea", 7, 0.1, 1, "ModItemsInitializer.java:152"),
    f("polenta", 6, 0.1, 1, "ModItemsInitializer.java:153"),
    f("potato_soup", 6, 0.1, 1, "ModItemsInitializer.java:154"),
    f("veggie_ravioli", 6, 0.1, 1, "ModItemsInitializer.java:155"),
    f("yogurt", 6, 0.1, 1, "ModItemsInitializer.java:156"),
    f("manchet_bread", 6, 0.1, 1, "ModItemsInitializer.java:157"),

    // -- tier 2: the first `0.25` block --
    f("lembas_scone", 8, 0.25, 2, "ModItemsInitializer.java:160"),
    f("muffin", 8, 0.25, 2, "ModItemsInitializer.java:161"),
    bowl("pottage", 9, 0.25, 2, "ModItemsInitializer.java:162"),
    bowl("pasta_plain", 11, 0.25, 2, "ModItemsInitializer.java:163"),
    f("apple_pie", 7, 0.25, 2, "ModItemsInitializer.java:164"),
    f("plain_cheesecake", 11, 0.25, 2, "ModItemsInitializer.java:165"),
    f("baked_salmon", 9, 0.25, 2, "ModItemsInitializer.java:166"),
    f("eggdrop_soup", 13, 0.25, 2, "ModItemsInitializer.java:167"),
    f("fish_n_chips", 13, 0.25, 2, "ModItemsInitializer.java:168"),
    f("pierogi", 11, 0.25, 2, "ModItemsInitializer.java:169"),
    f("veggie_soup", 11, 0.25, 2, "ModItemsInitializer.java:170"),
    bowl("yogurt_with_berries", 9, 0.25, 2, "ModItemsInitializer.java:171"),
    f("borscht", 9, 0.25, 2, "ModItemsInitializer.java:172"),

    // -- tier 3 --
    f("hand_pie", 13, 0.25, 3, "ModItemsInitializer.java:175"),
    f("mintchoco_cheesecake", 13, 0.25, 3, "ModItemsInitializer.java:176"),
    f("schnitzel", 13, 0.25, 3, "ModItemsInitializer.java:177"),
    f("steak_dinner", 12, 0.25, 3, "ModItemsInitializer.java:178"),

    // -- the cold-biome block, tier 1 then 2 then 3 --
    f("squash_soup", 6, 0.1, 1, "ModItemsInitializer.java:182"),
    bowl("cabochis", 11, 0.25, 2, "ModItemsInitializer.java:184"),
    f("veggie_quiche", 7, 0.25, 2, "ModItemsInitializer.java:185"),
    bowl("lamb_stew", 13, 0.25, 3, "ModItemsInitializer.java:187"),
    f("fish_dinner", 12, 0.25, 3, "ModItemsInitializer.java:188"),

    // -- the hot-humid block --
    f("pea_soup", 7, 0.1, 1, "ModItemsInitializer.java:192"),
    f("rice_ball", 9, 0.25, 2, "ModItemsInitializer.java:194"),
    f("mutton_dinner", 8, 0.25, 2, "ModItemsInitializer.java:195"),
    f("sushi_roll", 13, 0.25, 3, "ModItemsInitializer.java:197"),
    f("ramen", 13, 0.25, 3, "ModItemsInitializer.java:198"),
    f("fried_rice", 13, 0.25, 3, "ModItemsInitializer.java:199"),

    // -- the temperate block --
    f("corn_chowder", 7, 0.1, 1, "ModItemsInitializer.java:203"),
    f("tortillas", 6, 0.1, 1, "ModItemsInitializer.java:204"),
    bowl("pasta_tomato", 11, 0.25, 2, "ModItemsInitializer.java:206"),
    f("cheese_pizza", 13, 0.25, 2, "ModItemsInitializer.java:207"),
    f("eggplant_dolma", 12, 0.25, 3, "ModItemsInitializer.java:209"),
    f("stuffed_pita", 13, 0.25, 3, "ModItemsInitializer.java:210"),
    f("mushroom_pizza", 12, 0.25, 3, "ModItemsInitializer.java:211"),

    // -- the hot-dry block --
    f("spicy_grilled_chicken", 7, 0.1, 1, "ModItemsInitializer.java:215"),
    f("pepper_hummus", 9, 0.25, 2, "ModItemsInitializer.java:217"),
    f("kebab", 8, 0.25, 2, "ModItemsInitializer.java:218"),
    f("pita_hummus", 12, 0.25, 3, "ModItemsInitializer.java:220"),
    f("spicy_eggplant", 12, 0.25, 3, "ModItemsInitializer.java:221"),

    // -- the trade-only block --
    bowl("congee", 9, 0.25, 2, "ModItemsInitializer.java:225"),
    f("kimchi", 11, 0.25, 2, "ModItemsInitializer.java:226"),
    f("stew_trencher", 13, 0.25, 3, "ModItemsInitializer.java:228"),
    f("stuffed_pepper", 13, 0.25, 3, "ModItemsInitializer.java:229"),
    f("tacos", 13, 0.25, 3, "ModItemsInitializer.java:230"),

    // -- the four breads, each with its own class and effect --
    // `milkBread.nutrition(5).saturationMod(0.6F)` and tier 1 (`ItemMilkyBread.java:30-38`).
    Food {
        item: "milky_bread",
        nutrition: 5,
        saturation: 0.6,
        tier: Some(1),
        form: FoodForm::Solid,
        after: AfterEating::CureAll,
        effect: None,
        always_eat: false,
        source: "ItemMilkyBread.java:30",
    },
    // `.effect(MOVEMENT_SPEED 600 ticks, 1.0F)` plus a poison cure and tier 1
    // (`ItemSugaryBread.java:31-39`).
    Food {
        item: "sugary_bread",
        nutrition: 6,
        saturation: 0.7,
        tier: Some(1),
        form: FoodForm::Solid,
        after: AfterEating::CurePoison,
        effect: Some(Effect {
            effect: "minecraft:speed",
            ticks: 600,
            amplifier: 0,
            chance: 1.0,
            source: "ItemSugaryBread.java:33",
        }),
        always_eat: false,
        source: "ItemSugaryBread.java:31",
    },
    // `heal(4)` and tier 1 (`ItemGoldenBread.java:31-53`).
    Food {
        item: "golden_bread",
        nutrition: 5,
        saturation: 0.6,
        tier: Some(1),
        form: FoodForm::Solid,
        after: AfterEating::Heal(4),
        effect: None,
        always_eat: false,
        source: "ItemGoldenBread.java:31",
    },
    // `saturationMod(2.0F).alwaysEat()`, tier 2, surface teleport
    // (`ItemChorusBread.java:32-55`).
    Food {
        item: "chorus_bread",
        nutrition: 5,
        saturation: 2.0,
        tier: Some(2),
        form: FoodForm::Solid,
        after: AfterEating::SurfaceTeleport,
        effect: None,
        always_eat: true,
        source: "ItemChorusBread.java:32",
    },

    // -- the fifteen crops, one hunger point each, `ItemCrop.java:49` --
    crop("bell_pepper"),
    crop("cabbage"),
    crop("chickpea"),
    crop("durum"),
    crop("eggplant"),
    crop("garlic"),
    crop("onion"),
    crop("soybean"),
    crop("tomato"),
    crop("rice"),
    crop("butternut_squash"),
    crop("corn"),
    crop("mint"),
    crop("nether_pepper"),
    crop("peas"),
];

/// Every edible item, the crops included.
pub fn all() -> impl Iterator<Item = &'static Food> {
    ALL.iter()
}

/// The food rule an id names, or `None`.
pub fn of(item: &str) -> Option<&'static Food> {
    ALL.iter().find(|food| food.item == item)
}

/// The food rule an id names, or a named refusal.
pub fn require(item: &'static str) -> Result<&'static Food, Refusal> {
    of(item).ok_or(Refusal::NotFood { id: item })
}

/// Every food of a tier, in table order.
pub fn of_tier(tier: u8) -> impl Iterator<Item = &'static Food> {
    ALL.iter().filter(move |food| food.tier == Some(tier))
}

/// The crops, whose item half this module owns and whose block half
/// `crate::blocks::ressource` does.
pub fn crops() -> impl Iterator<Item = &'static Food> {
    ALL.iter().filter(|food| food.form == FoodForm::Crop)
}

/// The saturation **points** one portion restores: `nutrition * saturationMod * 2`.
///
/// Vanilla's own arithmetic, `FoodData.add`; the multiplier is not a point count
/// and `0.1` on six hunger is 1.2 points, not 0.6.
pub fn saturation_points(food: &Food) -> f64 {
    f64::from(food.nutrition) * food.saturation * 2.0
}

impl Food {
    /// The hunger points one bite restores, or a named refusal for a zero.
    ///
    /// Every food of the mod restores at least one — `nutrition(1)` is the crops'
    /// (`ItemCrop.java:49`) and a food that restores nothing is not a food — so a
    /// zero is only reachable from a table typo, and it is refused rather than fed.
    pub fn hunger(&self) -> Result<u8, Refusal> {
        if self.nutrition == 0 {
            return Err(Refusal::NoNutrition { id: self.item });
        }
        Ok(self.nutrition)
    }

    /// Whether the effect, if any, is granted **always** (`chance == 1.0`) or on a
    /// roll.
    pub fn effect_is_certain(&self) -> bool {
        self.effect.is_some_and(|effect| (effect.chance - 1.0).abs() < 1e-9)
    }
}

/// How long eating one takes, in ticks: `32 * max(1, tier)`, which is
/// `ItemFood.java:75` and `ItemBowlFood.java:70` exactly — a tier-3 dish takes
/// three times as long as a tier-1 one.
///
/// A crop has no tier and therefore eats at the vanilla speed;
/// `NoSuchTier` for a tier outside `1..=3`, which the three lists of
/// `ModItems.java:186-254` cannot produce.
pub fn use_ticks(food: &Food) -> Result<i32, Refusal> {
    match food.tier {
        None => Ok(ITEM_USE_TICKS),
        Some(tier) if (1..=3).contains(&tier) => Ok(ITEM_USE_TICKS * i32::from(tier)),
        Some(tier) => Err(Refusal::NoSuchTier { tier: i32::from(tier) }),
    }
}

/// Whether `getAllFoods()` names this item — the list the composter and the
/// creative food tab read (`ModItems.java:186-254`).
///
/// **59 of the 79**, and the five that are not are named: `fried_rice`, which the
/// list forgets, and the four breads, which are registered for compost by hand
/// (`ModItemsInitializer.java:468-471`). A crop is never in it either: the list
/// is `ItemFood` items, and a crop is an `ItemCrop`.
pub fn in_all_foods(item: &str) -> bool {
    of(item).is_some()
        && of(item).map(|food| food.form) != Some(FoodForm::Crop)
        && !matches!(
            item,
            "fried_rice" | "milky_bread" | "sugary_bread" | "golden_bread" | "chorus_bread"
        )
}

/// The composter's chance for one item, or `None` where Java registers none.
///
/// `ModItemsInitializer.java:466-495` in three branches:
///
/// - the four breads, `factor 6` each (`:468-471`);
/// - every `getAllFoods()` item, `factor 6` (`:477-480`);
/// - `mistletoe`, a flat `0.25` (`:482`);
///
/// with `strength = min(1, nutrition / factor)`, and **nothing** where
/// `getFoodProperties` answers `null` — which is exactly what happens to the
/// seven ingredients, all plain `Item`s: the loop at `:473-476` runs seven times
/// and registers nothing. `fried_rice` is absent because `getAllFoods()` forgets
/// it. `mistletoe` is the only non-food entry.
pub fn compost_chance(item: &str) -> Option<f64> {
    const FACTOR: f64 = 6.0;
    if item == "mistletoe" {
        return Some(0.25);
    }
    if matches!(item, "fried_rice") {
        return None;
    }
    if matches!(item, "milky_bread" | "sugary_bread" | "golden_bread" | "chorus_bread") {
        let food = of(item)?;
        return Some(compost_strength(food.nutrition, FACTOR));
    }
    let food = of(item)?;
    if !in_all_foods(item) {
        return None;
    }
    Some(compost_strength(food.nutrition, FACTOR))
}

/// `Math.min(1.0f, nutrition / factor)` (`ModItemsInitializer.java:490`), as the
/// port computes it — a `f64`, because Java's `float` would round `5 / 6` twice.
pub fn compost_strength(nutrition: u8, factor: f64) -> f64 {
    (f64::from(nutrition) / factor).min(1.0)
}

// ---------------------------------------------------------------------------
// What a *citizen* may eat — the illness side.
// ---------------------------------------------------------------------------

/// The four items that make a citizen **ill**, as the `minecolonies:poisonousfood`
/// tag lists them (`api/items/ModTags.java:70`, values from
/// `DefaultItemTagsProvider.java:178` and the generated
/// `data/minecolonies/tags/items/poisonousfood.json`).
///
/// **All four are vanilla**, and no MineColonies food is in the tag: a mod dish
/// cannot poison a citizen, and the only way to make one sick by mouth is to feed
/// it rotten flesh, raw chicken, a spider eye or a poisonous potato. A citizen fed
/// one is refused it by [`citizen_can_eat`] and, when a player right-clicks a
/// citizen with it, catches a **random disease**
/// (`core/entity/citizen/EntityCitizen.java:492-499`,
/// `DiseasesListener.getRandomDisease`).
pub const POISONOUS_FOOD: [&str; 4] = [
    "minecraft:poisonous_potato",
    "minecraft:chicken",
    "minecraft:spider_eye",
    "minecraft:rotten_flesh",
];

/// Why a citizen refuses a plate — the three answers of
/// `api/util/FoodUtils.java:43-77`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: Java refuses for exactly three reasons
pub enum EatRefusal {
    /// The `poisonousfood` tag: `if (... || stack.is(ModTags.poisonous_food)) return false`
    /// (`FoodUtils.java:45`).
    Poisonous { id: &'static str },
    /// A crop is never a citizen's meal: `if (stack.getItem() instanceof ItemCrop) return false`
    /// (`FoodUtils.java:62-65`).
    Crop { id: &'static str },
    /// A home or a hut at level 3 or more serves nothing plainer than
    /// `level + 1` hunger points (`FoodUtils.java:67-73`).
    TooPlain { id: &'static str, nutrition: u8, level: i32 },
}

impl EatRefusal {
    /// A stable name, for a log line or a test.
    pub fn name(self) -> &'static str {
        match self {
            EatRefusal::Poisonous { .. } => "poisonous",
            EatRefusal::Crop { .. } => "crop",
            EatRefusal::TooPlain { .. } => "too_plain",
        }
    }
}

/// Whether this item is one of the four that make a citizen ill.
pub fn makes_citizen_ill(item: &str) -> bool {
    POISONOUS_FOOD.contains(&item)
}

/// Whether a citizen in a building of `building_level` may eat `food` —
/// `FoodUtils.canEat` and `FoodUtils.canEatLevel`, in Java's order.
///
/// The threshold is the one rule here a player feels: below level 3, any food
/// goes; from level 3 up, the food must restore **at least `level + 1`** hunger
/// points. At level 5 that is six, so the four breads (five points) stop being
/// served and every dish of the mod still is — the crops are one point and were
/// never served at all.
pub fn citizen_can_eat(food: &Food, building_level: i32) -> Result<(), EatRefusal> {
    if makes_citizen_ill(food.item) {
        return Err(EatRefusal::Poisonous { id: food.item });
    }
    if food.form == FoodForm::Crop {
        return Err(EatRefusal::Crop { id: food.item });
    }
    if building_level < 3 {
        return Ok(());
    }
    if i32::from(food.nutrition) >= building_level + 1 {
        return Ok(());
    }
    Err(EatRefusal::TooPlain { id: food.item, nutrition: food.nutrition, level: building_level })
}

/// The building level a food is worth, for the menu that offers it:
/// `Math.max(2, Math.min(nutrition - 1, MAX_BUILDING_LEVEL))`
/// (`FoodUtils.java:80-83`).
pub fn building_level_for_food(nutrition: u8) -> i32 {
    (i32::from(nutrition) - 1).clamp(2, crate::building::MAX_BUILDING_LEVEL)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::{get, Category, Refusal};

    #[test]
    fn the_table_holds_the_seventy_nine_edibles() {
        assert_eq!(ALL.len(), 79);
        assert_eq!(ALL.iter().filter(|food| food.form == FoodForm::Crop).count(), 15);
        assert_eq!(ALL.iter().filter(|food| food.form == FoodForm::Bowl).count(), 8);
        assert_eq!(ALL.iter().filter(|food| food.form == FoodForm::Solid).count(), 56);
        assert_eq!(CROP_COUNT, 15);
        assert_eq!(all().count(), ALL.len(), "`all` is the same table, not a copy");
    }

    #[test]
    fn no_food_has_two_rows_and_every_row_names_an_item_of_the_table() {
        for (index, food) in ALL.iter().enumerate() {
            for other in &ALL[index + 1..] {
                assert_ne!(food.item, other.item);
            }
            assert_eq!(get(food.item).map(|entry| entry.category), Some(Category::Food), "{}", food.item);
            assert_eq!(of(food.item), Some(food));
        }
    }

    #[test]
    fn every_row_cites_a_java_line() {
        for food in ALL {
            assert!(food.source.contains(".java:"), "{} cites {}", food.item, food.source);
        }
    }

    #[test]
    fn the_hunger_values_are_the_six_to_thirteen_java_writes() {
        for food in ALL {
            assert!((1..=13).contains(&food.nutrition), "{} feeds {}", food.item, food.nutrition);
            assert!(food.nutrition > 0);
        }
        // the two ends
        assert_eq!(of("cheddar_cheese").unwrap().nutrition, 6);
        assert_eq!(of("eggdrop_soup").unwrap().nutrition, 13);
        assert_eq!(of("bell_pepper").unwrap().nutrition, 1, "a crop is one hunger");
    }

    #[test]
    fn the_saturation_multipliers_are_the_six_java_writes() {
        for food in ALL {
            assert!(
                [0.1, 0.25, 0.3, 0.6, 0.7, 2.0].contains(&food.saturation),
                "{} has {}",
                food.item,
                food.saturation
            );
        }
        assert_eq!(of("chorus_bread").unwrap().saturation, 2.0);
        assert_eq!(of("sugary_bread").unwrap().saturation, 0.7);
        assert_eq!(of("bell_pepper").unwrap().saturation, 0.3);
    }

    #[test]
    fn a_tier_is_one_two_or_three_and_only_the_crops_have_none() {
        for food in ALL {
            match food.tier {
                Some(tier) => assert!((1..=3).contains(&tier), "{}", food.item),
                None => assert_eq!(food.form, FoodForm::Crop, "{}", food.item),
            }
        }
        assert_eq!(of_tier(1).count(), 23);
        assert_eq!(of_tier(2).count(), 24);
        assert_eq!(of_tier(3).count(), 17);
        assert_eq!(ALL.iter().filter(|food| food.tier.is_none()).count(), 15);
        assert_eq!(23 + 24 + 17 + 15, ALL.len());
    }

    #[test]
    fn a_food_outside_the_three_tiers_is_refused_by_name() {
        let broken = Food { tier: Some(4), ..*of("muffin").unwrap() };
        assert_eq!(use_ticks(&broken), Err(Refusal::NoSuchTier { tier: 4 }));
        assert_eq!(Refusal::NoSuchTier { tier: 4 }.name(), "no_such_tier");
        assert_eq!(use_ticks(&Food { tier: Some(0), ..broken }), Err(Refusal::NoSuchTier { tier: 0 }));
    }

    #[test]
    fn the_saturation_points_are_the_multiplier_and_the_doubling() {
        // 6 * 0.1 * 2 = 1.2, not 0.6 and not 6.
        let dish = of("cheddar_cheese").unwrap();
        assert!((saturation_points(dish) - 1.2).abs() < 1e-9);
        assert!((saturation_points(of("chorus_bread").unwrap()) - 20.0).abs() < 1e-9);
        assert!((saturation_points(of("bell_pepper").unwrap()) - 0.6).abs() < 1e-9);
        // The best dish of the mod, by saturation: chorus bread, by far.
        let best = ALL
            .iter()
            .max_by(|a, b| saturation_points(a).total_cmp(&saturation_points(b)))
            .unwrap();
        assert_eq!(best.item, "chorus_bread");
    }

    #[test]
    fn eating_a_tier_three_dish_takes_three_times_as_long_as_a_tier_one() {
        assert_eq!(use_ticks(of("cheddar_cheese").unwrap()), Ok(32));
        assert_eq!(use_ticks(of("pottage").unwrap()), Ok(64));
        assert_eq!(use_ticks(of("tacos").unwrap()), Ok(96));
        assert_eq!(use_ticks(of("bell_pepper").unwrap()), Ok(32), "a crop has no tier");
        assert_eq!(ITEM_USE_TICKS, 32);
    }

    #[test]
    fn a_bowl_gives_a_bowl_back_and_nothing_else_gives_anything() {
        assert_eq!(FoodForm::Bowl.remainder(), Some("minecraft:bowl"));
        assert_eq!(FoodForm::Solid.remainder(), None);
        assert_eq!(FoodForm::Crop.remainder(), None);
        let bowls: Vec<&str> = ALL
            .iter()
            .filter(|food| food.form == FoodForm::Bowl)
            .map(|food| food.item)
            .collect();
        assert_eq!(
            bowls,
            vec![
                "cooked_rice",
                "pottage",
                "pasta_plain",
                "yogurt_with_berries",
                "cabochis",
                "lamb_stew",
                "pasta_tomato",
                "congee",
            ]
        );
        // `ItemBowlFood` is a `Item` with eight of the sixty rows, not a subclass
        // of `ItemFood`: the two classes share no code (`ItemBowlFood.java:15`).
        assert_eq!(bowls.len(), 8);
    }

    #[test]
    fn the_four_overrides_are_the_four_breads() {
        let after: Vec<(&str, AfterEating)> = ALL
            .iter()
            .filter(|food| food.after != AfterEating::Nothing)
            .map(|food| (food.item, food.after))
            .collect();
        assert_eq!(
            after,
            vec![
                ("milky_bread", AfterEating::CureAll),
                ("sugary_bread", AfterEating::CurePoison),
                ("golden_bread", AfterEating::Heal(4)),
                ("chorus_bread", AfterEating::SurfaceTeleport),
            ]
        );
        // 4 of 79: everything else is a plain `ItemFood`.
        assert_eq!(ALL.iter().filter(|food| food.after == AfterEating::Nothing).count(), 75);
    }

    #[test]
    fn only_the_sugary_bread_grants_an_effect_and_it_is_speed_for_thirty_seconds() {
        let effect = of("sugary_bread").unwrap().effect.unwrap();
        assert_eq!(effect.effect, "minecraft:speed");
        assert_eq!(effect.ticks, 600);
        assert_eq!(effect.amplifier, 0);
        assert!((effect.chance - 1.0).abs() < 1e-9, "certain, not 1-in-N");
        assert_eq!(ALL.iter().filter(|food| food.effect.is_some()).count(), 1);
        assert_eq!(effect.source, "ItemSugaryBread.java:33");
    }

    #[test]
    fn only_the_chorus_bread_is_eaten_at_full_hunger() {
        let always: Vec<&str> = ALL
            .iter()
            .filter(|food| food.always_eat)
            .map(|food| food.item)
            .collect();
        assert_eq!(always, vec!["chorus_bread"]);
    }

    #[test]
    fn the_crops_are_the_fifteen_the_blocks_module_sows() {
        let names: Vec<&str> = crops().map(|food| food.item).collect();
        assert_eq!(names.len(), 15);
        assert_eq!(names[0], "bell_pepper");
        assert_eq!(names[9], "rice", "the tenth, and the one sown on flooded farmland");
        for name in names {
            assert!(crate::blocks::block(name).is_some(), "{name} has no block");
        }
    }

    #[test]
    fn the_food_list_names_fifty_nine_and_the_five_it_does_not() {
        let listed: Vec<&str> = ALL
            .iter()
            .filter(|food| in_all_foods(food.item))
            .map(|food| food.item)
            .collect();
        assert_eq!(listed.len(), 59);
        assert!(!in_all_foods("fried_rice"), "the one food the list forgets");
        assert!(!in_all_foods("milky_bread"));
        assert!(!in_all_foods("sugary_bread"));
        assert!(!in_all_foods("golden_bread"));
        assert!(!in_all_foods("chorus_bread"));
        assert!(!in_all_foods("bell_pepper"), "a crop is an ItemCrop, not an ItemFood");
        assert!(!in_all_foods("butter"), "an ingredient is not food");
    }

    #[test]
    fn the_composter_takes_every_food_except_fried_rice_and_the_ingredients() {
        // all `getAllFoods` entries: nutrition >= 6 at factor 6, so exactly 1.0
        assert_eq!(compost_chance("cheddar_cheese"), Some(1.0));
        assert_eq!(compost_chance("squash_soup"), Some(1.0));
        // the four breads, by hand: 5 / 6 for three of them
        assert_eq!(compost_chance("sugary_bread"), Some(1.0));
        let milky = compost_chance("milky_bread").unwrap();
        assert!((milky - 5.0 / 6.0).abs() < 1e-9);
        // the one food the list forgets, and the seven ingredients
        assert_eq!(compost_chance("fried_rice"), None);
        for id in crate::items::materials::INGREDIENTS {
            assert_eq!(compost_chance(id), None, "{id} has no food properties to read");
        }
        // the only non-food entry, at a flat quarter
        assert_eq!(compost_chance("mistletoe"), Some(0.25));
        assert_eq!(compost_chance("cabbage"), None, "a crop is not registered either");
    }

    #[test]
    fn the_compost_strength_is_capped_at_one() {
        assert_eq!(compost_strength(6, 6.0), 1.0);
        assert_eq!(compost_strength(13, 6.0), 1.0, "13 / 6 clamps, it does not overflow");
        assert!((compost_strength(5, 6.0) - 0.833_333_333_333_333_4).abs() < 1e-12);
        assert!((compost_strength(5, 10.0) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn a_non_food_is_refused_by_name() {
        assert_eq!(require("spear"), Err(Refusal::NotFood { id: "spear" }));
        assert_eq!(Refusal::NotFood { id: "spear" }.name(), "not_food");
        assert_eq!(of("spear"), None);
    }

    #[test]
    fn every_food_restores_at_least_one_hunger_and_a_zero_is_refused() {
        for food in ALL {
            assert_eq!(food.hunger(), Ok(food.nutrition), "{}", food.item);
            assert!(food.nutrition >= 1, "{}", food.item);
        }
        // a zero only exists in a broken table, and it does not feed anyone
        let broken = Food { nutrition: 0, ..*of("muffin").unwrap() };
        assert_eq!(broken.hunger(), Err(Refusal::NoNutrition { id: "muffin" }));
        assert_eq!(Refusal::NoNutrition { id: "muffin" }.name(), "no_nutrition");
        assert!((saturation_points(&broken) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn the_one_effect_of_the_mod_is_certain_not_a_roll() {
        assert!(of("sugary_bread").unwrap().effect_is_certain());
        // 78 of the 79 have no effect at all, so "certain" is not true of them
        assert!(!of("muffin").unwrap().effect_is_certain());
        assert_eq!(ALL.iter().filter(|food| food.effect_is_certain()).count(), 1);
    }

    #[test]
    fn every_food_row_of_the_item_table_is_here_and_nothing_else_is() {
        let table: Vec<&str> = crate::items::of_category(Category::Food)
            .map(|entry| entry.id)
            .collect();
        for id in table {
            assert!(of(id).is_some(), "{id} is a food with no rule");
        }
        // the item table's `Food` rows and this table are the same 79: 64 items of
        // the items initialiser, plus the 15 crops their block registers.
        assert_eq!(crate::items::count_of(Category::Food), 79);
        assert_eq!(ALL.len() - 15, 64, "79 edibles = 64 registry foods + 15 crops");
        assert_eq!(crate::items::count_of(Category::Food), ALL.len());
    }

    #[test]
    fn the_food_stack_is_the_shared_sixty_four() {
        assert_eq!(FOOD_STACK, 64);
        assert_eq!(FOOD_STACK, STACKSIZE);
    }

    #[test]
    fn no_mod_food_makes_a_citizen_ill_and_the_four_that_do_are_vanilla() {
        assert_eq!(POISONOUS_FOOD.len(), 4);
        for id in POISONOUS_FOOD {
            assert!(id.starts_with("minecraft:"), "{id} is not vanilla");
            assert_eq!(of(id), None, "{id} is not a mod food");
        }
        for food in ALL {
            assert!(!makes_citizen_ill(food.item), "{} is poisonous", food.item);
        }
        assert!(makes_citizen_ill("minecraft:rotten_flesh"));
        assert!(!makes_citizen_ill("cheddar_cheese"));
        assert_eq!(
            crate::items::get("mistletoe").map(|entry| entry.category),
            Some(Category::Material),
            "the one plant item is not poisonous either"
        );
    }

    #[test]
    fn a_crop_is_never_a_citizens_meal() {
        for food in crops() {
            assert_eq!(
                citizen_can_eat(food, 1),
                Err(EatRefusal::Crop { id: food.item }),
                "{}",
                food.item
            );
            assert_eq!(
                citizen_can_eat(food, 9),
                Err(EatRefusal::Crop { id: food.item }),
                "the crop check comes before the level check"
            );
        }
        assert_eq!(EatRefusal::Crop { id: "rice" }.name(), "crop");
    }

    #[test]
    fn below_level_three_any_food_goes_and_from_there_the_plate_must_match_the_level() {
        let bread = of("milky_bread").unwrap(); // 5 hunger points
        let cheese = of("cheddar_cheese").unwrap(); // 6
        assert_eq!(citizen_can_eat(bread, 2), Ok(()));
        assert_eq!(citizen_can_eat(cheese, 2), Ok(()));
        // level 3 needs >= 4, level 4 needs >= 5, level 5 needs >= 6
        assert_eq!(citizen_can_eat(bread, 3), Ok(()));
        assert_eq!(citizen_can_eat(bread, 4), Ok(()));
        assert_eq!(
            citizen_can_eat(bread, 5),
            Err(EatRefusal::TooPlain { id: "milky_bread", nutrition: 5, level: 5 }),
            "five points is not a level-5 meal"
        );
        assert_eq!(citizen_can_eat(cheese, 5), Ok(()), "six points is");
        assert_eq!(EatRefusal::TooPlain { id: "x", nutrition: 5, level: 5 }.name(), "too_plain");
    }

    #[test]
    fn three_of_the_four_breads_are_the_only_dishes_a_top_home_refuses() {
        let refused: Vec<&str> = ALL
            .iter()
            .filter(|food| citizen_can_eat(food, 5).is_err())
            .map(|food| food.item)
            .collect();
        // 15 crops (never) + the three breads at five points; the sugary one has
        // six and is served even at level 5
        assert_eq!(refused.len(), 18);
        for id in ["milky_bread", "golden_bread", "chorus_bread"] {
            assert!(refused.contains(&id), "{id}");
        }
        assert_eq!(of("sugary_bread").unwrap().nutrition, 6);
        assert_eq!(citizen_can_eat(of("sugary_bread").unwrap(), 5), Ok(()));
        // every dish of six points or more is served at the top level
        for food in ALL {
            if food.form == FoodForm::Crop || food.nutrition < 6 {
                continue;
            }
            assert_eq!(citizen_can_eat(food, 5), Ok(()), "{}", food.item);
        }
    }

    #[test]
    fn a_building_level_for_food_is_the_nutrition_minus_one_clamped_to_two_and_five() {
        assert_eq!(building_level_for_food(1), 2, "a crop is worth the floor");
        assert_eq!(building_level_for_food(5), 4);
        assert_eq!(building_level_for_food(6), 5);
        assert_eq!(building_level_for_food(8), 5);
        assert_eq!(building_level_for_food(13), 5, "the cap is the building cap");
        assert_eq!(crate::building::MAX_BUILDING_LEVEL, 5);
    }
}
