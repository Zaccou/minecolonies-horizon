//! The recipe types that are MineColonies' own: their fields, what makes one
//! invalid, and the one function that turns a saved recipe into a typed value or
//! a named refusal.
//!
//! Java reference: `api/crafting/` (the two recipe classes the mod registers and
//! the two *storage* types), `core/recipes/` (the two custom ingredient kinds),
//! `core/colony/crafting/CustomRecipe.java` (the crafter-recipe format) and
//! `core/datalistener/CrafterRecipeListener.java` (the loader that dispatches on
//! the `type` field).
//!
//! | File | Question |
//! | --- | --- |
//! | `kinds` | which types exist, which fields each one reads, and which loader reads it |
//! | `ingredient` | what an ingredient may be, and the four kinds this mod adds to vanilla's |
//! | `compost` | `minecolonies:composting`: strengths, the per-item count, the food ranges |
//! | `zero_waste` | `minecolonies:zero_waste`: a shapeless recipe that discards the empties |
//! | `crafter` | the 161 `crafterrecipes/` files: `recipe`, `recipe-multi-out`, `recipe-template`, `remove` |
//! | `resolve` | `type` field + file → the typed recipe, or [`Refusal`] |
//! | `register` | the two registrations that stop the recipe table failing to load |
//!
//! **Two loaders, not one, and that is the first thing to get right.** A file
//! under `data/<ns>/recipe/` names a *recipe serializer* in its `type` field and
//! is read by the game's `RecipeManager`; a file under
//! `data/<ns>/crafterrecipes/` names one of five *kinds* read by
//! `CrafterRecipeListener` (`:29-32` — the listener's own directory). A file in
//! the wrong folder is not a near miss: Java's switch has no case for it and the
//! file is dropped in silence (`CrafterRecipeListener.java:50-63`), and the game
//! refuses a `type` no serializer claims for the *whole* recipe table. [`Origin`]
//! carries that folder, and [`resolve`] refuses a mismatch by name.
//!
//! **Why the four blocked files are blocked.** `compostables.json`,
//! `compostables_poor.json`, `compostables_rich.json` (`minecolonies:composting`)
//! and `soy_milk.json` (`minecolonies:zero_waste`) sit in
//! `recettes-custom/` because a recipe type nobody registered makes
//! `Recipes::from_resolved` refuse the **entire** table — 154 recipes lost to one
//! file. [`register::register`] is the three calls that fix it, and
//! `src/crafting/MANQUANT.md` says what is still missing afterwards.
//!
//! **What is not here.** No matching against a live inventory (that needs a
//! `LevelRef` and a slot list), no craft execution, no request tree — the recipe
//! *is* data; who crafts it is `crate::requests`' question. [`Facts`] is the one
//! host question this module asks, and it is a trait so every rule below can be
//! tested on the host target with a table instead of a game.

pub mod compost;
pub mod crafter;
pub mod ingredient;
pub mod kinds;
pub mod register;
pub mod resolve;
pub mod zero_waste;

pub use compost::Composting;
pub use crafter::{CountedItem, CrafterRecipe, Filter, Gate, Input, Removal, Template};
pub use ingredient::{Food, FoodRange, Ingredient, ItemId, Question};
pub use kinds::{Field, FieldKind, Kind, Origin, StorageType, Teaching};
pub use resolve::{Recipe, resolve};

pub(crate) use resolve::ident;
pub use zero_waste::ZeroWaste;

use mod_api::serde_json::{Map, Value};

/// The answer to one host question an ingredient needs, and the only thing this
/// module cannot decide alone.
///
/// Java asks the item registry three questions, each with its own class:
///
/// | Question | Java | Answer |
/// | --- | --- | --- |
/// | is this item in that tag? | `Ingredient.of(tag)` → `HolderSet` | [`Question::InTag`] |
/// | what is this item's food? | `stack.getItem().getFoodProperties(stack, null)` (`FoodIngredient.java:116`) | [`Question::Food`] |
/// | is this item a planted crop? | `item instanceof BlockItem` and a `CropBlock`/`StemBlock` (`PlantIngredient.java:36-39`) | [`Question::Crop`] |
///
/// All three are answers a table can give in a test, which is why this is a
/// trait and not a host call here. The real one lives outside this folder: the
/// food component is `Host::item_default_component(item, "minecraft:food")`
/// (`crates/horizon-mod-vocab/src/host/items.rs:30`), whose compound holds
/// `nutrition` and `saturation` — the two numbers
/// `FoodProperties.getNutrition()` and `getSaturationModifier()` read.
pub trait Facts {
    /// Whether `item` is in `tag`, written `namespace:path` without the `#`.
    fn in_tag(&self, tag: &str, item: &ItemId) -> bool;
    /// The item's food, `None` for an item that is not edible.
    fn food(&self, item: &ItemId) -> Option<Food>;
    /// Whether the item plants a crop: Java's `CropBlock` or `StemBlock` block
    /// item (`PlantIngredient.java:36-39`).
    fn crop(&self, item: &ItemId) -> bool;
}

/// A [`Facts`] that knows nothing: every tag empty, no item edible, no crop.
///
/// It is the honest default for a test or a caller with no world: it answers
/// exactly what Java answers outside a world, where a tag is not loaded.
pub struct NoFacts;

impl Facts for NoFacts {
    fn in_tag(&self, _tag: &str, _item: &ItemId) -> bool {
        false
    }
    fn food(&self, _item: &ItemId) -> Option<Food> {
        None
    }
    fn crop(&self, _item: &ItemId) -> bool {
        false
    }
}

/// Why this module refused a recipe, named.
///
/// A variant is **not** a `lang` key: those live in
/// `assets/minecolonies/lang/`, which this module does not own.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// The file is not a JSON object. Java throws `IllegalStateException` from
    /// `getAsJsonObject` and the listener skips that one file
    /// (`CrafterRecipeListener.java:48`, `:65-68`).
    NotAnObject,
    /// No `type` field. Java reads it with the default `""`
    /// (`CrafterRecipeListener.java:50`) and every case misses, so the file is
    /// dropped without a line in the log.
    MissingType,
    /// A `type` this port does not know. Same silence in Java: the switch has no
    /// `default`, so nothing is reported.
    UnknownKind { found: String },
    /// A kind that belongs to the other loader. Java never sees the combination
    /// (two loaders, two folders), so no line of Java says what happens; the
    /// port refuses it instead of guessing which loader was meant.
    WrongOrigin { found: String },
    /// A field Java's `GsonHelper` requires is absent. The helper throws
    /// `JsonSyntaxException` and the listener drops the file
    /// (`CrafterRecipeListener.java:65-68`).
    MissingField { field: &'static str },
    /// A field holds a JSON kind this port does not read. Java coerces a number
    /// to text where it reads text; this port refuses instead, because no
    /// shipped file relies on it.
    WrongType { field: &'static str },
    /// An item id the engine cannot parse, or the empty string.
    /// `Identifier::parse` is the same charset check the game makes
    /// (`crates/horizon-core/src/identifier.rs:55-65`).
    UnparsableItem { field: &'static str },
    /// A count a stack cannot hold: `ItemStack.setCount(0)` in Java is an empty
    /// stack, which is never what a data file means.
    CountNotPositive { field: &'static str },
    /// `composting` with a strength of zero or less. Java does not refuse it — it
    /// divides by it (`CompostRecipe.java:129`) and throws
    /// `ArithmeticException` on the first barrel tick.
    StrengthNotPositive { strength: i64 },
    /// An ingredient object whose `type` is not one this port reads. Java throws
    /// `JsonSyntaxException` from `CraftingHelper.getIngredient`
    /// (`CompostRecipe.java:146`).
    UnknownIngredient { found: String },
    /// An equipment type id the registry does not hold. Java's
    /// `ModEquipmentTypes.getRegistry().getValue(...)` answers `null` and the
    /// recipe keeps a `null` tool (`CustomRecipe.java:292-296`), which throws on
    /// the first use.
    UnknownTool { found: String },
    /// A template that expands to no output at all
    /// (`CustomRecipe.java:543-551`, "rejecting, no outputs").
    TemplateNoOutput,
}

impl Refusal {
    /// A stable name for logs and tests, namespaced like a registry entry so it
    /// can never be mistaken for a player-facing sentence.
    pub fn name(&self) -> &'static str {
        match self {
            Self::NotAnObject => "crafting.refusal.not_an_object",
            Self::MissingType => "crafting.refusal.missing_type",
            Self::UnknownKind { .. } => "crafting.refusal.unknown_kind",
            Self::WrongOrigin { .. } => "crafting.refusal.wrong_origin",
            Self::MissingField { .. } => "crafting.refusal.missing_field",
            Self::WrongType { .. } => "crafting.refusal.wrong_type",
            Self::UnparsableItem { .. } => "crafting.refusal.unparsable_item",
            Self::CountNotPositive { .. } => "crafting.refusal.count_not_positive",
            Self::StrengthNotPositive { .. } => "crafting.refusal.strength_not_positive",
            Self::UnknownIngredient { .. } => "crafting.refusal.unknown_ingredient",
            Self::UnknownTool { .. } => "crafting.refusal.unknown_tool",
            Self::TemplateNoOutput => "crafting.refusal.template_no_output",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())?;
        match self {
            Self::UnknownKind { found } | Self::WrongOrigin { found } => {
                write!(f, " ({found})")
            }
            Self::MissingField { field } | Self::WrongType { field } => write!(f, " ({field})"),
            Self::UnparsableItem { field } => write!(f, " ({field})"),
            Self::CountNotPositive { field } => write!(f, " ({field})"),
            Self::StrengthNotPositive { strength } => write!(f, " ({strength})"),
            Self::UnknownIngredient { found } => write!(f, " ({found})"),
            Self::UnknownTool { found } => write!(f, " ({found})"),
            Self::NotAnObject
            | Self::MissingType
            | Self::TemplateNoOutput => Ok(()),
        }
    }
}

impl std::error::Error for Refusal {}

// --- the GsonHelper-shaped readers, one place ------------------------------
//
// Java reads every one of these fields through
// `net.minecraft.util.GsonHelper` or `com.minecolonies.core.util.GsonHelper`,
// whose names this file repeats so a field table can cite the reader. Two
// differences, both stated rather than hidden: `getAsString` in Gson coerces a
// number to its text and this refuses it, and `getAsInt` truncates a float and
// this does too (`as_f64` branch below).

/// The fields of a JSON object, or [`Refusal::NotAnObject`].
pub(crate) fn fields(value: &Value) -> Result<&Map<String, Value>, Refusal> {
    value.as_object().ok_or(Refusal::NotAnObject)
}

/// Whether `field` is present at all, which is how Java decides between a default
/// and a value (`recipeJson.has(...)`, `CustomRecipe.java:258`).
pub(crate) fn has(value: &Value, field: &str) -> bool {
    value.get(field).is_some_and(|found| !found.is_null())
}

/// `GsonHelper.getAsString`. Absent is [`Refusal::MissingField`], not `""`.
pub(crate) fn text<'a>(value: &'a Value, field: &'static str) -> Result<&'a str, Refusal> {
    match value.get(field) {
        None => Err(Refusal::MissingField { field }),
        Some(found) => found.as_str().ok_or(Refusal::WrongType { field }),
    }
}

/// `GsonHelper.getAsString(json, key)` guarded by `has`: `None` when absent.
pub(crate) fn text_or<'a>(
    value: &'a Value,
    field: &'static str,
) -> Result<Option<&'a str>, Refusal> {
    if has(value, field) {
        Ok(Some(text(value, field)?))
    } else {
        Ok(None)
    }
}

/// `GsonHelper.getAsInt(json, key, default)`.
pub(crate) fn int_or(value: &Value, field: &'static str, default: i64) -> Result<i64, Refusal> {
    if has(value, field) {
        number(value, field)
    } else {
        Ok(default)
    }
}

/// `GsonHelper.getAsInt(json, key)`: absent is a refusal, not zero.
pub(crate) fn number(value: &Value, field: &'static str) -> Result<i64, Refusal> {
    let found = value.get(field).ok_or(Refusal::MissingField { field })?;
    if let Some(whole) = found.as_i64() {
        return Ok(whole);
    }
    // Gson's `getAsInt` reads `getAsNumber().intValue()`: a float truncates
    // toward zero rather than being refused.
    found
        .as_f64()
        .filter(|real| real.is_finite())
        .map(|real| real as i64)
        .ok_or(Refusal::WrongType { field })
}

/// `GsonHelper.getAsFloat(json, key)`, absent answered as `None`.
pub(crate) fn float_or(value: &Value, field: &'static str) -> Result<Option<f32>, Refusal> {
    if !has(value, field) {
        return Ok(None);
    }
    let found = value.get(field).ok_or(Refusal::MissingField { field })?;
    if let Some(whole) = found.as_i64() {
        return Ok(Some(whole as f32));
    }
    found
        .as_f64()
        .map(|real| Some(real as f32))
        .ok_or(Refusal::WrongType { field })
}

/// `GsonHelper.getAsBoolean(json, key, false)`: absent is `false`, which is what
/// `recipeJson.has(...) && getAsBoolean(...)` means at
/// `CustomRecipe.java:381-388`.
pub(crate) fn flag_or(value: &Value, field: &'static str, default: bool) -> Result<bool, Refusal> {
    if !has(value, field) {
        return Ok(default);
    }
    value
        .get(field)
        .and_then(Value::as_bool)
        .ok_or(Refusal::WrongType { field })
}

/// `GsonHelper.getAsJsonArray`: absent is an empty list, a non-array is a
/// refusal. Java uses `has(...)` before the call everywhere this is used
/// (`CustomRecipe.java:263`, `:298`, `:320`).
pub(crate) fn array_or<'a>(
    value: &'a Value,
    field: &'static str,
) -> Result<Vec<&'a Value>, Refusal> {
    if !has(value, field) {
        return Ok(Vec::new());
    }
    let found = value.get(field).ok_or(Refusal::MissingField { field })?;
    found
        .as_array()
        .map(|items| items.iter().collect())
        .ok_or(Refusal::WrongType { field })
}

/// `GsonHelper.getAsJsonArray(json, key)`: absent is a refusal.
pub(crate) fn array<'a>(value: &'a Value, field: &'static str) -> Result<Vec<&'a Value>, Refusal> {
    if !has(value, field) {
        return Err(Refusal::MissingField { field });
    }
    array_or(value, field)
}

/// The text of a field that may be one string or an array of them, which is how
/// `research-id` and `not-research-id` are written (`CustomRecipe.java:353-372`).
pub(crate) fn texts(value: &Value, field: &'static str) -> Result<Vec<String>, Refusal> {
    if !has(value, field) {
        return Ok(Vec::new());
    }
    let Some(found) = value.get(field) else {
        return Ok(Vec::new());
    };
    if let Some(one) = found.as_str() {
        return Ok(vec![one.to_owned()]);
    }
    let items = found.as_array().ok_or(Refusal::WrongType { field })?;
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        out.push(item.as_str().ok_or(Refusal::WrongType { field })?.to_owned());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::serde_json::Value;

    fn json(text: &str) -> Value {
        mod_api::serde_json::from_str(text).expect("a literal in a test is valid JSON")
    }

    #[test]
    fn every_refusal_has_its_own_name() {
        let all = [
            Refusal::NotAnObject,
            Refusal::MissingType,
            Refusal::UnknownKind {
                found: "x".into(),
            },
            Refusal::WrongOrigin {
                found: "x".into(),
            },
            Refusal::MissingField { field: "x" },
            Refusal::WrongType { field: "x" },
            Refusal::UnparsableItem { field: "x" },
            Refusal::CountNotPositive { field: "x" },
            Refusal::StrengthNotPositive { strength: 0 },
            Refusal::UnknownIngredient {
                found: "x".into(),
            },
            Refusal::UnknownTool {
                found: "x".into(),
            },
            Refusal::TemplateNoOutput,
        ];
        for (index, refusal) in all.iter().enumerate() {
            for other in &all[index + 1..] {
                assert_ne!(
                    refusal.name(),
                    other.name(),
                    "two refusals share a name, so a log line cannot tell them apart"
                );
            }
            assert!(refusal.name().starts_with("crafting.refusal."));
        }
    }

    #[test]
    fn a_refusal_carries_the_name_of_what_it_refused() {
        let refusal = Refusal::MissingField { field: "ingredients" };
        assert_eq!(
            refusal.to_string(),
            "crafting.refusal.missing_field (ingredients)"
        );
        assert_eq!(
            Refusal::UnknownKind {
                found: "minecolonies:smelting".into()
            }
            .to_string(),
            "crafting.refusal.unknown_kind (minecolonies:smelting)"
        );
    }

    #[test]
    fn the_readers_use_the_default_java_gives() {
        let recipe = json(r#"{"strength": 8, "name": "baker", "on": true}"#);
        assert_eq!(int_or(&recipe, "strength", 1), Ok(8));
        assert_eq!(int_or(&recipe, "absent", 1), Ok(1));
        assert_eq!(text_or(&recipe, "name"), Ok(Some("baker")));
        assert_eq!(text_or(&recipe, "absent"), Ok(None));
        assert_eq!(flag_or(&recipe, "on", false), Ok(true));
        assert_eq!(flag_or(&recipe, "absent", false), Ok(false));
        assert_eq!(array_or(&recipe, "absent"), Ok(Vec::new()));
        assert_eq!(texts(&recipe, "absent"), Ok(Vec::new()));
    }

    #[test]
    fn a_missing_required_field_is_named_not_defaulted() {
        let recipe = json(r#"{"count": 1}"#);
        assert_eq!(
            text(&recipe, "result"),
            Err(Refusal::MissingField { field: "result" })
        );
        assert_eq!(
            array(&recipe, "ingredients"),
            Err(Refusal::MissingField {
                field: "ingredients"
            })
        );
    }

    #[test]
    fn a_field_of_the_wrong_json_kind_is_named() {
        let recipe = json(r#"{"result": 4}"#);
        assert_eq!(
            text(&recipe, "result"),
            Err(Refusal::WrongType { field: "result" })
        );
        let list = json(r#"{"inputs": {}}"#);
        assert_eq!(
            array_or(&list, "inputs"),
            Err(Refusal::WrongType { field: "inputs" })
        );
    }

    #[test]
    fn get_as_int_truncates_a_float_the_way_gson_does() {
        let recipe = json(r#"{"count": 2.9}"#);
        assert_eq!(number(&recipe, "count"), Ok(2));
    }

    #[test]
    fn one_string_or_an_array_of_them_both_read() {
        let one = json(r#"{"research-id": "minecolonies:effects/consumepotions"}"#);
        assert_eq!(
            texts(&one, "research-id"),
            Ok(vec!["minecolonies:effects/consumepotions".to_owned()])
        );
        let many = json(r#"{"research-id": ["a:b", "c:d"]}"#);
        assert_eq!(texts(&many, "research-id"), Ok(vec!["a:b".into(), "c:d".into()]));
    }
}
