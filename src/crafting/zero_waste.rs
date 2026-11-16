//! `minecolonies:zero_waste` — a shapeless recipe that **discards the empties**.
//!
//! Java reference: `api/crafting/ZeroWasteRecipe.java`.
//!
//! **What makes it different from a shapeless recipe is one method.** It extends
//! `ShapelessRecipe` (`:38`) and overrides `getRemainingItems` to fill every
//! returned slot with `ItemStack.EMPTY` (`:47-54`). Vanilla's shapeless recipe
//! would hand back the empty bottle, the empty bucket, the empty bowl; this one
//! hands back nothing. That is its entire reason to exist, and the one shipped
//! file uses it for exactly that:
//!
//! ```json
//! {
//!   "type": "minecolonies:zero_waste",
//!   "ingredients": [{"item": "minecolonies:large_water_bottle"}, {"item": "minecolonies:soybean"}],
//!   "result": {"item": "minecolonies:large_soy_milk_bottle"}
//! }
//! ```
//!
//! The bottle goes in and does not come back; the soy milk comes out. A vanilla
//! shapeless recipe would leave an empty `large_water_bottle` in the grid.
//!
//! **Its recipe type is `minecraft:crafting`, not its own.** Java registers a
//! *serializer* for it (`ModRecipeSerializerInitializer.java:24`) and no
//! `RecipeType` — `ShapelessRecipe.getType()` already answers
//! `RecipeType.CRAFTING`. So in Horizon the serializer name is
//! `minecolonies:zero_waste` and the recipe type is `minecraft:crafting`, which
//! is what [`crate::register`] declares.
//!
//! **Shape matching.** `ShapelessRecipe.matches` requires the grid to hold
//! exactly as many items as there are ingredients, then matches them one to one.
//! [`ZeroWaste::matches`] is that rule, on item ids.

use super::ingredient::Ingredient;
use super::{Facts, ItemId, Refusal};
use mod_api::serde_json::Value;

/// One `minecolonies:zero_waste` file.
#[derive(Clone, Debug, PartialEq)]
pub struct ZeroWaste {
    /// What goes in, in file order. Java keeps them as a `NonNullList`
    /// (`:72-76`) and order does not matter to a shapeless match.
    pub ingredients: Vec<Ingredient>,
    /// What comes out (`:70`).
    pub result: ItemId,
    /// How many come out; `count` in the `result` object, one by default.
    pub count: u32,
}

impl ZeroWaste {
    /// A zero-waste recipe from its file.
    ///
    /// `result` goes through `ShapedRecipe.itemStackFromJson` (`:70`), which
    /// reads a `{"item": …, "count": …}` object — **`item`**, not the `id` a
    /// 26.3 `result` uses. `ingredients` is a plain array of ingredient values
    /// (`:71-76`), read with `strict = false`, so an element the game cannot
    /// parse becomes an empty ingredient instead of an error; this port names
    /// such an element instead of silently emptying a slot.
    pub fn parse(value: &Value) -> Result<Self, Refusal> {
        let object = value
            .get("result")
            .ok_or(Refusal::MissingField { field: "result" })?;
        let result = ItemId::parse("result", super::text(object, "item")?)?;
        let count = super::int_or(object, "count", 1)?;
        let count = u32::try_from(count)
            .ok()
            .filter(|count| *count > 0)
            .ok_or(Refusal::CountNotPositive { field: "count" })?;

        let mut ingredients = Vec::new();
        for element in super::array(value, "ingredients")? {
            ingredients.push(Ingredient::parse(element, "ingredients")?);
        }
        Ok(Self {
            ingredients,
            result,
            count,
        })
    }

    /// What one craft makes.
    pub fn output(&self) -> (&ItemId, u32) {
        (&self.result, self.count)
    }

    /// Whether this grid makes this recipe: the grid holds exactly as many items
    /// as there are ingredients, and each cell fits one they do not already use.
    ///
    /// `ShapelessRecipe.matches` by items, so the same item twice in the grid
    /// needs it twice in `ingredients`.
    pub fn matches(&self, grid: &[Option<ItemId>], facts: &dyn Facts) -> bool {
        if grid.len() != self.ingredients.len() {
            return false;
        }
        let mut used = vec![false; self.ingredients.len()];
        for cell in grid {
            let Some(item) = cell else {
                return false;
            };
            let mut placed = false;
            for (index, ingredient) in self.ingredients.iter().enumerate() {
                if !used[index] && ingredient.admits(item, facts) {
                    used[index] = true;
                    placed = true;
                    break;
                }
            }
            if !placed {
                return false;
            }
        }
        true
    }

    /// What the grid keeps: **nothing**, one empty slot per ingredient.
    ///
    /// This is the whole point of the type. `getRemainingItems` starts from
    /// vanilla's answer and then overwrites every entry with `ItemStack.EMPTY`
    /// (`:49-53`), so the vector is always as long as the ingredient list and
    /// never holds an item.
    pub fn remainders(&self) -> Vec<Option<ItemId>> {
        vec![None; self.ingredients.len()]
    }
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

    // The one shipped file, verbatim.
    const SOY_MILK: &str = r#"{
        "type": "minecolonies:zero_waste",
        "ingredients": [
            {"item": "minecolonies:large_water_bottle"},
            {"item": "minecolonies:soybean"}
        ],
        "result": {"item": "minecolonies:large_soy_milk_bottle"}
    }"#;

    #[test]
    fn the_shipped_soy_milk_file_parses() {
        let recipe = ZeroWaste::parse(&json(SOY_MILK)).expect("soy milk must parse");
        assert_eq!(recipe.ingredients.len(), 2);
        assert_eq!(recipe.result.name().as_str(), "minecolonies:large_soy_milk_bottle");
        assert_eq!(recipe.count, 1, "no count field means one");
    }

    #[test]
    fn a_result_count_is_read_from_the_result_object() {
        let recipe = ZeroWaste::parse(&json(
            r#"{
                "ingredients": [{"item": "minecolonies:large_water_bottle"}],
                "result": {"item": "minecolonies:large_soy_milk_bottle", "count": 3}
            }"#,
        ))
        .expect("parses");
        assert_eq!(recipe.count, 3);
        assert_eq!(recipe.output(), (&item("minecolonies:large_soy_milk_bottle"), 3));
    }

    #[test]
    fn the_result_object_names_its_item_with_item_not_id() {
        // 26.3 renamed the `result` key to `id`; this file format is 1.21's and
        // Java reads `item` (ZeroWasteRecipe.java:70).
        let refused = ZeroWaste::parse(&json(
            r#"{"ingredients": [], "result": {"id": "minecolonies:large_soy_milk_bottle"}}"#,
        ));
        assert_eq!(refused, Err(Refusal::MissingField { field: "item" }));
    }

    #[test]
    fn a_missing_result_or_ingredients_is_a_named_refusal() {
        assert_eq!(
            ZeroWaste::parse(&json(r#"{"ingredients": []}"#)),
            Err(Refusal::MissingField { field: "result" })
        );
        assert_eq!(
            ZeroWaste::parse(&json(r#"{"result": {"item": "minecraft:bread"}}"#)),
            Err(Refusal::MissingField {
                field: "ingredients"
            })
        );
    }

    #[test]
    fn a_count_of_zero_is_refused_instead_of_becoming_an_empty_stack() {
        assert_eq!(
            ZeroWaste::parse(&json(
                r#"{"ingredients": [], "result": {"item": "minecraft:bread", "count": 0}}"#
            )),
            Err(Refusal::CountNotPositive { field: "count" })
        );
    }

    #[test]
    fn the_remainders_are_one_empty_slot_per_ingredient() {
        let recipe = ZeroWaste::parse(&json(SOY_MILK)).expect("parses");
        assert_eq!(
            recipe.remainders(),
            vec![None, None],
            "the empty water bottle does not come back: ZeroWasteRecipe.java:51-53"
        );
    }

    #[test]
    fn the_soy_milk_grid_matches_in_either_order() {
        let recipe = ZeroWaste::parse(&json(SOY_MILK)).expect("parses");
        let facts = NoFacts;
        let one = item("minecolonies:large_water_bottle");
        let two = item("minecolonies:soybean");
        assert!(recipe.matches(&[Some(one.clone()), Some(two.clone())], &facts));
        assert!(recipe.matches(&[Some(two.clone()), Some(one.clone())], &facts));
    }

    #[test]
    fn a_grid_of_the_wrong_size_does_not_match() {
        let recipe = ZeroWaste::parse(&json(SOY_MILK)).expect("parses");
        let facts = NoFacts;
        let bottle = item("minecolonies:large_water_bottle");
        assert!(
            !recipe.matches(&[Some(bottle.clone())], &facts),
            "one cell for two ingredients"
        );
        assert!(!recipe.matches(
            &[
                Some(bottle.clone()),
                Some(item("minecolonies:soybean")),
                Some(item("minecraft:sugar"))
            ],
            &facts
        ));
        assert!(!recipe.matches(&[], &facts));
    }

    #[test]
    fn the_same_item_twice_needs_the_ingredient_twice() {
        let doubled = ZeroWaste::parse(&json(
            r#"{
                "ingredients": [{"item": "minecraft:sugar"}],
                "result": {"item": "minecraft:bread"}
            }"#,
        ))
        .expect("parses");
        let facts = NoFacts;
        let sugar = item("minecraft:sugar");
        assert!(doubled.matches(&[Some(sugar.clone())], &facts));
        assert!(
            !doubled.matches(&[Some(sugar.clone()), Some(sugar)], &facts),
            "one ingredient, two cells: ShapelessRecipe.matches refuses"
        );
    }

    #[test]
    fn a_wrong_item_does_not_match() {
        let recipe = ZeroWaste::parse(&json(SOY_MILK)).expect("parses");
        let facts = NoFacts;
        assert!(!recipe.matches(
            &[
                Some(item("minecolonies:large_water_bottle")),
                Some(item("minecraft:sugar"))
            ],
            &facts
        ));
        // A hole in the grid is not an ingredient either.
        assert!(!recipe.matches(
            &[Some(item("minecolonies:large_water_bottle")), None],
            &facts
        ));
    }

    #[test]
    fn an_empty_ingredient_list_only_matches_an_empty_grid() {
        let empty = ZeroWaste::parse(&json(
            r#"{"ingredients": [], "result": {"item": "minecraft:bread"}}"#,
        ))
        .expect("Java accepts it: a shapeless recipe with no ingredients");
        let facts = NoFacts;
        assert!(empty.matches(&[], &facts));
        assert!(!empty.matches(&[Some(item("minecraft:sugar"))], &facts));
        assert!(empty.remainders().is_empty());
    }
}
