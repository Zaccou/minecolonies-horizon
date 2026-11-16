//! What an ingredient may be: vanilla's two forms, and the two kinds this mod
//! adds to vanilla's ingredient registry.
//!
//! Java reference: `net.minecraft.world.item.crafting.Ingredient` (via Forge's
//! `CraftingHelper.getIngredient`), `core/recipes/FoodIngredient.java` (an
//! ingredient matching food by healing and saturation) and
//! `core/recipes/PlantIngredient.java` (an ingredient matching plantable crops).
//!
//! **Only two ingredient types are MineColonies'.** They are registered as
//! Forge ingredient serializers under this mod's namespace, and a data file names
//! them with a `type` field, exactly like any other ingredient type:
//!
//! ```json
//! { "type": "minecolonies:food", "max-saturation": 1.0 }
//! ```
//!
//! | `type` | Java | Fields |
//! | --- | --- | --- |
//! | `minecolonies:food` | `FoodIngredient.java:48-122` | `min-healing`, `max-healing`, `min-saturation`, `max-saturation` |
//! | `minecolonies:plant` | `PlantIngredient.java:30-50` | none |
//!
//! **Bounds: min is inclusive, max is exclusive**
//! (`FoodIngredient.java:28-46`, and the comparisons at `:114-121`). So
//! `"max-saturation": 1.0` means "strictly less than one", which is how
//! `compostables.json` separates the ordinary food from the rich food, and
//! `"min-saturation": 1.0` in `compostables_rich.json` means "one or more".
//!
//! **The item is a name here, and the count is the caller's.** An ingredient
//! answers "does this item fit", which is `Ingredient.test`
//! (`Ingredient.ItemValue.test` → `stack.is(item)`, the item alone, NBT and
//! count ignored). Whether the *stack* fits is a slot question and lives in
//! `crate::inventory`.

use super::{Facts, Refusal};
use crate::inventory::ItemName;
use mod_api::serde_json::Value;

/// An item id, and the NBT suffix a data file may glue to it.
///
/// Java's `ItemStackUtils.idToItemStack` splits at the first `{` and keeps
/// everything from there as SNBT (`ItemStackUtils.java:791-796`), which is how
/// `CustomRecipeProvider.stackAsJson` writes an item that carries a tag
/// (`:241-250`, "this could be incorrect for items with both damage and other
/// NBT"). The suffix is kept as text here and **not parsed**: the host parses it
/// with `Host::item_from_snbt` (`crates/horizon-mod-vocab/src/host/items.rs:64`)
/// when it needs a real stack, and two ids with the same text are the same NBT.
///
/// A namespace-less id takes `minecraft:`, which is what Java's
/// `split.length == 1` branch does (`:798-802`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ItemId {
    name: ItemName,
    nbt: Option<String>,
}

impl ItemId {
    /// The item `text` names, `field` naming the JSON field when it does not.
    pub fn parse(field: &'static str, text: &str) -> Result<Self, Refusal> {
        let (id, nbt) = match text.find('{') {
            Some(at) => (&text[..at], Some(text[at..].to_owned())),
            None => (text, None),
        };
        let name = ItemName::new(id).ok_or(Refusal::UnparsableItem { field })?;
        Ok(Self { name, nbt })
    }

    /// An item with no NBT suffix.
    pub fn of(name: ItemName) -> Self {
        Self { name, nbt: None }
    }

    /// The item, without its NBT.
    pub fn name(&self) -> &ItemName {
        &self.name
    }

    /// The SNBT suffix, `{...}` included, when the id carried one.
    pub fn nbt(&self) -> Option<&str> {
        self.nbt.as_deref()
    }

    /// The same item: `ignore_nbt` is `ItemStorage`'s `"matchType": "ignore"`
    /// (`ItemStorage.java:158-159`).
    pub fn same_item(&self, other: &ItemId, ignore_nbt: bool) -> bool {
        self.name == other.name && (ignore_nbt || self.nbt == other.nbt)
    }
}

impl std::fmt::Display for ItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name.as_str())?;
        if let Some(nbt) = &self.nbt {
            f.write_str(nbt)?;
        }
        Ok(())
    }
}

/// The two numbers Java reads off `FoodProperties`
/// (`FoodIngredient.java:116-120`).
///
/// In 26.3 they are the `minecraft:food` data component's `nutrition` and
/// `saturation` fields, and `saturation` is the *modifier* —
/// `FoodProperties.getSaturationModifier()`, the number the JSON already holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Food {
    /// `FoodProperties.getNutrition()`, in half-drumsticks.
    pub nutrition: i32,
    /// `FoodProperties.getSaturationModifier()`.
    pub saturation: f32,
}

impl Food {
    /// Java's `ISFOOD`: "at least some healing and some saturation, not purely
    /// for effects" (`ItemStackUtils.java:146-152`) — nutrition above zero,
    /// saturation above zero, and not in the excluded-food tag.
    pub fn is_standard(&self) -> bool {
        self.nutrition > 0 && self.saturation > 0.0
    }
}

/// The four bounds of `minecolonies:food`, each optional.
///
/// `None` is Java's `Optional.empty()` and matches everything on that axis
/// (`FoodIngredient.java:57-60`, `:117-120`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FoodRange {
    pub min_healing: Option<i32>,
    pub max_healing: Option<i32>,
    pub min_saturation: Option<f32>,
    pub max_saturation: Option<f32>,
}

impl FoodRange {
    /// `FoodIngredient.Serializer.parse` (`:132-144`): four optional numbers.
    pub fn parse(value: &Value) -> Result<Self, Refusal> {
        Ok(Self {
            min_healing: optional_int(value, "min-healing")?,
            max_healing: optional_int(value, "max-healing")?,
            min_saturation: super::float_or(value, "min-saturation")?,
            max_saturation: super::float_or(value, "max-saturation")?,
        })
    }

    /// `FoodIngredient.Builder.matchesFood` (`:114-121`), bound for bound.
    ///
    /// The two `max` comparisons are `<`, not `<=`, on purpose: the source says
    /// so in two places (`:42-43` in the class comment, `:118` and `:120` in the
    /// code).
    pub fn accepts(&self, food: &Food) -> bool {
        self.min_healing
            .map(|healing| food.nutrition >= healing)
            .unwrap_or(true)
            && self
                .max_healing
                .map(|healing| food.nutrition < healing)
                .unwrap_or(true)
            && self
                .min_saturation
                .map(|saturation| food.saturation >= saturation)
                .unwrap_or(true)
            && self
                .max_saturation
                .map(|saturation| food.saturation < saturation)
                .unwrap_or(true)
    }

    /// Whether any bound is set at all. Java still asks `ISFOOD` for an
    /// unbounded `minecolonies:food`, so this is not "matches everything".
    pub fn bounded(&self) -> bool {
        self.min_healing.is_some()
            || self.max_healing.is_some()
            || self.min_saturation.is_some()
            || self.max_saturation.is_some()
    }
}

fn optional_int(value: &Value, field: &'static str) -> Result<Option<i32>, Refusal> {
    match super::int_or(value, field, 0)? {
        0 if !super::has(value, field) => Ok(None),
        whole => Ok(Some(whole as i32)),
    }
}

/// One ingredient value of a recipe file.
///
/// Vanilla's two forms are [`Self::Item`] and [`Self::Tag`]; [`Self::Any`] is a
/// JSON array, which vanilla reads as alternatives; the last two are this mod's
/// own serializer types.
#[derive(Clone, Debug, PartialEq)]
pub enum Ingredient {
    /// An array: any member admitting the item is enough.
    Any(Vec<Ingredient>),
    /// `{"item": "minecolonies:soybean"}` or a bare id.
    Item(ItemId),
    /// `{"tag": "minecolonies:compostables"}` — the name without its `#`, which
    /// is how the JSON writes it.
    Tag(String),
    /// `{"type": "minecolonies:food", …}`.
    Food(FoodRange),
    /// `{"type": "minecolonies:plant"}`.
    Plant,
}

impl Ingredient {
    /// An ingredient from its JSON value, `field` naming it in a refusal.
    ///
    /// Java's shapes, in the order `CraftingHelper.getIngredient` tests them: an
    /// array is a list of alternatives, an object with `type` goes to that
    /// ingredient serializer, and an object with `item` or `tag` is vanilla's.
    /// A **bare string** is not an ingredient value in 1.20.1 and is refused;
    /// neither is an object with none of the three keys.
    pub fn parse(value: &Value, field: &'static str) -> Result<Self, Refusal> {
        if let Some(items) = value.as_array() {
            let mut parsed = Vec::with_capacity(items.len());
            for item in items {
                parsed.push(Ingredient::parse(item, field)?);
            }
            return Ok(Ingredient::Any(parsed));
        }
        let object = value.as_object().ok_or(Refusal::WrongType { field })?;
        if let Some(kind) = object.get("type").and_then(Value::as_str) {
            return match kind {
                "minecolonies:food" => Ok(Ingredient::Food(FoodRange::parse(value)?)),
                "minecolonies:plant" => Ok(Ingredient::Plant),
                // Forge's own types (`forge:nbt`, `forge:block_tag`, …) are not
                // read here: they are a registry this port does not carry.
                other => Err(Refusal::UnknownIngredient {
                    found: other.to_owned(),
                }),
            };
        }
        if let Some(item) = object.get("item").and_then(Value::as_str) {
            return Ok(Ingredient::Item(ItemId::parse(field, item)?));
        }
        if let Some(tag) = object.get("tag").and_then(Value::as_str) {
            return Ok(Ingredient::Tag(tag.trim_start_matches('#').to_owned()));
        }
        Err(Refusal::UnknownIngredient {
            found: String::new(),
        })
    }

    /// Whether this ingredient admits `item`, asking `facts` only what only the
    /// game knows.
    ///
    /// `Ingredient.test` compares **items**, never counts or NBT for the two
    /// vanilla forms. `minecolonies:food` adds `ISFOOD` on top of the bounds:
    /// an item with no food, or food with no nutrition or no saturation, fits no
    /// food ingredient at all (`FoodIngredient.java:72-79`, `:114-121`).
    pub fn admits(&self, item: &ItemId, facts: &dyn Facts) -> bool {
        match self {
            Self::Any(alternatives) => alternatives
                .iter()
                .any(|alternative| alternative.admits(item, facts)),
            Self::Item(named) => named.name() == item.name(),
            Self::Tag(tag) => facts.in_tag(tag, item),
            Self::Food(range) => facts
                .food(item)
                .is_some_and(|food| food.is_standard() && range.accepts(&food)),
            Self::Plant => facts.crop(item),
        }
    }

    /// Which of [`Facts`]'s three questions this ingredient asks, in order, with
    /// duplicates kept: an array asks one per member. Empty for an array with no
    /// member, which is what a caller needs to know before asking the host
    /// anything.
    pub fn questions(&self) -> Vec<Question> {
        match self {
            Self::Any(alternatives) => alternatives
                .iter()
                .flat_map(Ingredient::questions)
                .collect(),
            Self::Item(_) => Vec::new(),
            Self::Tag(_) => vec![Question::InTag],
            Self::Food(_) => vec![Question::Food],
            Self::Plant => vec![Question::Crop],
        }
    }
}

/// One question [`Facts`] answers, named so a manifest can be checked against
/// what a data pack will actually ask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Question {
    /// Is this item in that tag? (`Ingredient.of(tag)`)
    InTag,
    /// What is this item's food? (`FoodIngredient.java:116`)
    Food,
    /// Does this item plant a crop? (`PlantIngredient.java:36-39`)
    Crop,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crafting::NoFacts;

    fn json(text: &str) -> Value {
        mod_api::serde_json::from_str(text).expect("a literal in a test is valid JSON")
    }

    fn item(id: &str) -> ItemId {
        ItemId::parse("item", id).expect("a literal item id parses")
    }

    /// A table of facts, the way `ItemLimits` stands in for the engine elsewhere.
    #[derive(Default)]
    struct Table {
        tags: Vec<(String, ItemId)>,
        foods: Vec<(ItemId, Food)>,
        crops: Vec<ItemId>,
    }

    impl Facts for Table {
        fn in_tag(&self, tag: &str, item: &ItemId) -> bool {
            self.tags
                .iter()
                .any(|(name, member)| name == tag && member == item)
        }
        fn food(&self, item: &ItemId) -> Option<Food> {
            self.foods
                .iter()
                .find(|(named, _)| named == item)
                .map(|(_, food)| *food)
        }
        fn crop(&self, item: &ItemId) -> bool {
            self.crops.contains(item)
        }
    }

    #[test]
    fn an_item_id_takes_the_vanilla_namespace_when_it_has_none() {
        assert_eq!(item("stone").name().as_str(), "minecraft:stone");
        assert_eq!(item("minecolonies:compost").name().as_str(), "minecolonies:compost");
    }

    #[test]
    fn an_item_id_keeps_its_nbt_suffix_as_text() {
        let tagged = item("minecolonies:large_water_bottle{}");
        assert_eq!(tagged.name().as_str(), "minecolonies:large_water_bottle");
        assert_eq!(tagged.nbt(), Some("{}"));
        let plain = item("minecolonies:large_water_bottle");
        assert_eq!(plain.nbt(), None);
        // Java splits at the first `{` and keeps the rest, so a second brace stays.
        let nested = item("minecraft:potion{CustomPotionEffects:[{Id:1b}]}");
        assert_eq!(nested.nbt(), Some("{CustomPotionEffects:[{Id:1b}]}"));
    }

    #[test]
    fn two_ids_differ_on_their_nbt_unless_the_match_type_says_otherwise() {
        let plain = item("minecraft:potion");
        let tagged = item("minecraft:potion{}");
        assert!(!plain.same_item(&tagged, false));
        assert!(plain.same_item(&tagged, true));
        assert!(plain.same_item(&item("minecraft:potion"), false));
    }

    #[test]
    fn an_unparsable_item_id_is_named_with_its_field() {
        assert_eq!(
            ItemId::parse("result", ""),
            Err(Refusal::UnparsableItem { field: "result" })
        );
        assert_eq!(
            ItemId::parse("result", "MineColonies:Compost"),
            Err(Refusal::UnparsableItem { field: "result" }),
            "the charset is the engine's, and it has no capitals"
        );
        assert_eq!(
            ItemId::parse("result", "a:b:c"),
            Err(Refusal::UnparsableItem { field: "result" }),
            "Java logs an error and keeps a:b; this refuses instead"
        );
    }

    #[test]
    fn a_food_range_with_no_bound_matches_any_standard_food() {
        let range = FoodRange::default();
        assert!(!range.bounded());
        assert!(range.accepts(&Food {
            nutrition: 1,
            saturation: 0.1
        }));
    }

    #[test]
    fn the_min_bound_is_inclusive_and_the_max_bound_is_not() {
        let range = FoodRange::parse(&json(
            r#"{"type": "minecolonies:food", "min-saturation": 1.0}"#,
        ))
        .expect("rich food parses");
        assert!(range.accepts(&Food {
            nutrition: 4,
            saturation: 1.0
        }));
        assert!(!range.accepts(&Food {
            nutrition: 4,
            saturation: 0.9
        }));

        let poor = FoodRange::parse(&json(
            r#"{"type": "minecolonies:food", "max-saturation": 0.5}"#,
        ))
        .expect("poor food parses");
        assert!(poor.accepts(&Food {
            nutrition: 2,
            saturation: 0.49
        }));
        assert!(
            !poor.accepts(&Food {
                nutrition: 2,
                saturation: 0.5
            }),
            "max is exclusive: FoodIngredient.java:118"
        );
    }

    #[test]
    fn the_healing_bounds_read_the_nutrition() {
        let range = FoodRange::parse(&json(
            r#"{"type": "minecolonies:food", "min-healing": 4, "max-healing": 8}"#,
        ))
        .expect("parses");
        assert!(range.accepts(&Food {
            nutrition: 4,
            saturation: 1.0
        }));
        assert!(!range.accepts(&Food {
            nutrition: 3,
            saturation: 1.0
        }));
        assert!(range.accepts(&Food {
            nutrition: 7,
            saturation: 1.0
        }));
        assert!(!range.accepts(&Food {
            nutrition: 8,
            saturation: 1.0
        }));
    }

    #[test]
    fn a_food_ingredient_needs_a_standard_food_not_just_a_bound_match() {
        let facts = Table {
            foods: vec![
                (
                    item("minecraft:cake"),
                    Food {
                        nutrition: 0,
                        saturation: 5.0,
                    },
                ),
                (
                    item("minecraft:golden_apple"),
                    Food {
                        nutrition: 4,
                        saturation: 1.2,
                    },
                ),
            ],
            ..Table::default()
        };
        let any_food = Ingredient::parse(&json(r#"{"type": "minecolonies:food"}"#), "input")
            .expect("parses");
        assert!(
            !any_food.admits(&item("minecraft:cake"), &facts),
            "ISFOOD refuses zero nutrition: ItemStackUtils.java:150"
        );
        assert!(any_food.admits(&item("minecraft:golden_apple"), &facts));
        assert!(!any_food.admits(&item("minecraft:stone"), &facts));
    }

    #[test]
    fn the_three_vanilla_forms_parse() {
        assert_eq!(
            Ingredient::parse(&json(r#"{"item": "minecolonies:soybean"}"#), "input"),
            Ok(Ingredient::Item(item("minecolonies:soybean")))
        );
        assert_eq!(
            Ingredient::parse(&json(r#"{"tag": "minecolonies:compostables"}"#), "input"),
            Ok(Ingredient::Tag("minecolonies:compostables".into()))
        );
        assert_eq!(
            Ingredient::parse(&json(r#"["minecraft:apple"]"#), "input"),
            Ok(Ingredient::Any(vec![Ingredient::Item(item(
                "minecraft:apple"
            ))]))
        );
    }

    #[test]
    fn a_hash_in_a_tag_name_is_the_ones_java_strips() {
        assert_eq!(
            Ingredient::parse(&json(r##"{"tag": "#minecolonies:compostables"}"##), "input"),
            Ok(Ingredient::Tag("minecolonies:compostables".into()))
        );
    }

    #[test]
    fn the_two_ingredient_types_this_mod_adds_parse() {
        assert_eq!(
            Ingredient::parse(&json(r#"{"type": "minecolonies:plant"}"#), "input"),
            Ok(Ingredient::Plant)
        );
        let food = Ingredient::parse(
            &json(r#"{"type": "minecolonies:food", "min-healing": 4}"#),
            "input",
        )
        .expect("parses");
        assert!(matches!(food, Ingredient::Food(_)));
    }

    #[test]
    fn another_ingredient_type_is_named_not_guessed() {
        assert_eq!(
            Ingredient::parse(&json(r#"{"type": "forge:nbt", "item": "minecraft:stone"}"#), "input"),
            Err(Refusal::UnknownIngredient {
                found: "forge:nbt".into()
            })
        );
        assert_eq!(
            Ingredient::parse(&json(r#"{"count": 2}"#), "input"),
            Err(Refusal::UnknownIngredient {
                found: String::new()
            })
        );
        assert_eq!(
            Ingredient::parse(&json(r#""minecraft:stone""#), "input"),
            Err(Refusal::WrongType { field: "input" })
        );
    }

    #[test]
    fn an_item_ingredient_matches_the_item_and_never_its_nbt() {
        let facts = NoFacts;
        let ingredient = Ingredient::Item(item("minecraft:potion"));
        assert!(ingredient.admits(&item("minecraft:potion"), &facts));
        assert!(
            ingredient.admits(&item("minecraft:potion{Magic:1}"), &facts),
            "Ingredient.ItemValue.test is stack.is(item)"
        );
        assert!(!ingredient.admits(&item("minecraft:glass_bottle"), &facts));
    }

    #[test]
    fn a_tag_and_a_plant_ingredient_ask_the_host_and_nothing_else() {
        let facts = Table {
            tags: vec![(
                "minecolonies:compostables".into(),
                item("minecraft:oak_leaves"),
            )],
            crops: vec![item("minecraft:wheat_seeds")],
            ..Table::default()
        };
        let tag = Ingredient::Tag("minecolonies:compostables".into());
        assert!(tag.admits(&item("minecraft:oak_leaves"), &facts));
        assert!(!tag.admits(&item("minecraft:wheat_seeds"), &facts));
        let plant = Ingredient::Plant;
        assert!(plant.admits(&item("minecraft:wheat_seeds"), &facts));
        assert!(!plant.admits(&item("minecraft:oak_leaves"), &facts));
        // A caller with no world answers exactly what Java answers with no tags
        // loaded: nothing.
        assert!(!tag.admits(&item("minecraft:oak_leaves"), &NoFacts));
    }

    #[test]
    fn an_array_ingredient_is_an_alternative() {
        let facts = NoFacts;
        let any = Ingredient::Any(vec![
            Ingredient::Item(item("minecraft:apple")),
            Ingredient::Item(item("minecraft:carrot")),
        ]);
        assert!(any.admits(&item("minecraft:apple"), &facts));
        assert!(any.admits(&item("minecraft:carrot"), &facts));
        assert!(!any.admits(&item("minecraft:potato"), &facts));
    }
}
