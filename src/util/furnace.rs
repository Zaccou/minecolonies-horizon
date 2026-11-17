//! `FurnaceRecipes`: what a smelting recipe gives, and which recipe gives an item.
//!
//! Java reference: `core/util/FurnaceRecipes.java` (99 lines), the whole of the
//! `IFurnaceRecipes` implementation: a `loadRecipes` that fills two maps and two
//! lookups, one in each direction.
//!
//! # The two maps, and the two ways the names lie
//!
//! `loadRecipes` (`:37-66`) fills `recipes` keyed by the **input** and
//! `reverseRecipes` keyed by the **output**, both from one pass over
//! `RecipeType.SMELTING`. Two details are worth writing down because both are
//! behaviour, not accident:
//!
//! 1. **`recipeManager.byType(SMELTING).values()` is a `HashMap`'s values.** When
//!    two smelting recipes share an input — which recipe packs do — the winner is
//!    `HashMap` iteration order, i.e. arbitrary and stable only within one run.
//!    The port keeps a `Vec` and lets the **later** recipe win, which is what
//!    `Map.put` does to `HashMap` too; only the choice among duplicates becomes
//!    deterministic. Recorded in the report.
//! 2. **`getFirstSmeltingRecipeByResult` returns the *last* recipe for that
//!    output.** The method is named "first", and `reverseRecipes.put(...)` keeps
//!    the last write. Nothing can be done about it here without inventing an order
//!    the recipe manager does not have, so the port keeps `put`'s semantics and
//!    the name is documented as wrong.
//!
//! # The key is the caller's
//!
//! Java keys on `ItemStorage`, whose `equals` is
//! `ItemStackUtils.compareItemStacksIgnoreStackSize` — **item, damage and NBT, in
//! that order, and not the count** (`api/crafting/ItemStorage.java:255-267`).
//! That last part is what makes `getSmeltingResult(stack of 64)` find the recipe
//! registered from a stack of 1. The port takes the key as a generic `K`: what
//! "the same item" means (does a renamed sword smelt differently from a plain
//! one?) is a question for the item layer, and answering it here with a made-up
//! type would be a second, weaker answer than the one `mod_api::ItemStack`
//! already gives.

use std::collections::BTreeMap;

/// One smelting recipe, after the shape `loadRecipes` requires:
/// **exactly one ingredient**, whose accepted stacks are its variants.
// exhaustive: the input side and the result
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Smelt<K> {
    /// `storage.getCleanedInput().get(0)` (`:57`): the one stack this recipe
    /// consumes. A variant of the same ingredient gives a separate [`Smelt`].
    pub input: K,
    /// `recipe.getResultItem(...)` (`:51`): what comes out, with the recipe's own
    /// count.
    pub output: K,
}

/// The two smelting maps: `FurnaceRecipes.recipes` and
/// `FurnaceRecipes.reverseRecipes` (`:29-30`).
///
/// Both indexes point **into the same `Vec`**, which is what Java's two maps of a
/// shared `RecipeStorage` reference do (`:57`, `:61`): one recipe object answers
/// both directions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FurnaceRecipes<K> {
    smelts: Vec<Smelt<K>>,
    by_input: BTreeMap<K, usize>,
    by_output: BTreeMap<K, usize>,
}

impl<K: Clone + Ord> FurnaceRecipes<K> {
    /// An empty table, which is what a world with no recipe pack gives.
    pub fn new() -> FurnaceRecipes<K> {
        FurnaceRecipes {
            smelts: Vec::new(),
            by_input: BTreeMap::new(),
            by_output: BTreeMap::new(),
        }
    }

    /// Fill the table: `loadRecipes` (`:37-66`).
    ///
    /// ```java
    /// recipes.clear(); reverseRecipes.clear();
    /// recipeManager.byType(RecipeType.SMELTING).values().forEach(recipe -> {
    ///     final NonNullList<Ingredient> list = recipe.getIngredients();
    ///     if (list.size() == 1) {
    ///         for (final ItemStack smeltable : list.get(0).getItems()) {
    ///             if (!smeltable.isEmpty()) {
    ///                 ... recipes.put(storage.getCleanedInput().get(0), storage);
    ///                 ... reverseRecipes.put(new ItemStorage(output), storage);
    ///             }
    ///         }
    ///     }
    /// });
    /// ```
    ///
    /// Takes, per recipe, **all** its ingredients and its result — the shape the
    /// recipe manager hands over — and applies Java's two filters itself:
    ///
    /// - **more than one ingredient is skipped entirely** (`list.size() == 1`,
    ///   `:43`). A smelting recipe in vanilla has one; a mod that adds a
    ///   two-input one is silently not a furnace recipe here, exactly as in Java.
    /// - **each variant of that one ingredient becomes its own entry**
    ///   (`list.get(0).getItems()`, `:45`): the `#minecraft:logs` ingredient is
    ///   six recipes for the price of one. An empty variant has no `K` in this
    ///   port, so the caller must not pass one — Java's `isEmpty()` guard (`:47`)
    ///   has nothing to guard.
    ///
    /// Loading **replaces** everything: Java clears both maps first (`:39-40`),
    /// so a `/reload` does not accumulate stale recipes.
    pub fn load(recipes: impl IntoIterator<Item = (Vec<Vec<K>>, K)>) -> FurnaceRecipes<K> {
        let mut table = FurnaceRecipes::new();
        for (ingredients, result) in recipes {
            let variants = match ingredients.as_slice() {
                [only] => only,
                _ => continue,
            };
            for input in variants {
                table.insert(Smelt {
                    input: input.clone(),
                    output: result.clone(),
                });
            }
        }
        table
    }

    /// Add one prepared recipe, last write winning — what `Map.put` does to the
    /// two maps (`:57`, `:61`).
    ///
    /// `pub` because a caller that already knows it has a one-ingredient recipe
    /// does not need to dress it up as a manager's shape; it is the same path
    /// [`FurnaceRecipes::load`] takes.
    pub fn insert(&mut self, smelt: Smelt<K>) {
        let index = self.smelts.len();
        self.by_input.insert(smelt.input.clone(), index);
        self.by_output.insert(smelt.output.clone(), index);
        self.smelts.push(smelt);
    }

    /// What this stack smelts into: `getSmeltingResult` (`:69-77`).
    ///
    /// ```java
    /// final RecipeStorage storage = recipes.getOrDefault(new ItemStorage(itemStack), null);
    /// return storage != null ? storage.getPrimaryOutput() : ItemStack.EMPTY;
    /// ```
    ///
    /// `None` is Java's `ItemStack.EMPTY`: **not smeltable** is a normal answer, not
    /// an error, and a caller that treats it as one will refuse to cook half the
    /// pack. The count of the returned output is the recipe's, not the input's.
    pub fn smelting_result(&self, input: &K) -> Option<&K> {
        self.by_input
            .get(input)
            .map(|index| &self.smelts[*index].output)
    }

    /// The recipe that produces this item: `getFirstSmeltingRecipeByResult`
    /// (`:81-84`).
    ///
    /// The name says "first" and the body says `reverseRecipes.get(storage)`,
    /// which is a `Map.put` and therefore holds the **last** recipe registered for
    /// that output. Keeping `put`'s semantics is the port's only honest option:
    /// "first" would need an order the source has no way to supply.
    pub fn first_recipe_by_result(&self, output: &K) -> Option<&Smelt<K>> {
        self.by_output
            .get(output)
            .map(|index| &self.smelts[*index])
    }

    /// How many `(input, output)` pairs the table holds — one per ingredient
    /// variant, which is what `recipes.size()` reports in Java.
    pub fn len(&self) -> usize {
        self.smelts.len()
    }

    /// A table with no recipe at all: a world whose pack failed to load.
    pub fn is_empty(&self) -> bool {
        self.smelts.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The vanilla shape: one ingredient with several accepted variants, one
    /// result. Each variant is its own entry, and both lookup directions answer.
    #[test]
    fn one_ingredient_with_several_variants_becomes_one_entry_each() {
        let table = FurnaceRecipes::load([(
            vec![vec!["oak_log", "birch_log", "spruce_log"]],
            "charcoal",
        )]);
        assert_eq!(table.len(), 3);
        assert_eq!(table.smelting_result(&"oak_log"), Some(&"charcoal"));
        assert_eq!(table.smelting_result(&"birch_log"), Some(&"charcoal"));
        assert_eq!(table.smelting_result(&"spruce_log"), Some(&"charcoal"));
        assert_eq!(table.smelting_result(&"stone"), None);
    }

    /// The reverse direction finds the recipe, and therefore also the input it
    /// wants.
    #[test]
    fn the_reverse_lookup_answers_the_recipe_and_its_input() {
        let table = FurnaceRecipes::load([(vec![vec!["raw_iron"]], "iron_ingot")]);
        let recipe = table.first_recipe_by_result(&"iron_ingot");
        assert_eq!(recipe.map(|s| s.input), Some("raw_iron"));
        assert_eq!(recipe.map(|s| s.output), Some("iron_ingot"));
        assert!(table.first_recipe_by_result(&"gold_ingot").is_none());
    }

    /// A recipe with two ingredients is not a furnace recipe: Java's
    /// `list.size() == 1` skips it before reading anything.
    #[test]
    fn a_two_ingredient_recipe_is_skipped_entirely() {
        let table = FurnaceRecipes::load([
            (vec![vec!["sand"], vec!["gravel"]], "glass"),
            (vec![vec!["cobblestone"]], "stone"),
        ]);
        assert_eq!(table.len(), 1);
        assert!(table.smelting_result(&"sand").is_none());
        assert!(table.smelting_result(&"gravel").is_none());
        assert_eq!(table.smelting_result(&"cobblestone"), Some(&"stone"));
    }

    /// A recipe with no ingredient at all is skipped for the same reason, and a
    /// one-ingredient recipe with no variant registers nothing.
    #[test]
    fn a_recipe_with_no_ingredient_registers_nothing() {
        let table = FurnaceRecipes::load([(vec![], "nothing"), (vec![vec![]], "also_nothing")]);
        assert!(table.is_empty());
        assert_eq!(table.len(), 0);
    }

    /// Two recipes for the same input: the **later** write wins, like `put`.
    #[test]
    fn a_duplicate_input_keeps_the_last_recipe() {
        let table = FurnaceRecipes::load([
            (vec![vec!["clay_ball"]], "brick"),
            (vec![vec!["clay_ball"]], "terracotta"),
        ]);
        assert_eq!(table.smelting_result(&"clay_ball"), Some(&"terracotta"));
    }

    /// And two recipes for the same output: `first_recipe_by_result` answers the
    /// **last** one, because that is what `put` kept — the name is wrong, the
    /// behaviour is ported.
    #[test]
    fn a_duplicate_output_answers_the_last_recipe_despite_the_name() {
        let table = FurnaceRecipes::load([
            (vec![vec!["cobblestone"]], "stone"),
            (vec![vec!["stone"]], "stone"),
        ]);
        let recipe = table.first_recipe_by_result(&"stone");
        assert_eq!(recipe.map(|s| s.input), Some("stone"));
    }

    /// Loading twice replaces the table; it never accumulates.
    #[test]
    fn a_reload_replaces_the_whole_table() {
        let first = FurnaceRecipes::load([(vec![vec!["a"]], "b")]);
        assert_eq!(first.smelting_result(&"a"), Some(&"b"));
        let second = FurnaceRecipes::load([(vec![vec!["c"]], "d")]);
        assert!(second.smelting_result(&"a").is_none());
        assert_eq!(second.smelting_result(&"c"), Some(&"d"));
        assert_eq!(second.len(), 1);
    }

    /// An empty table answers nothing in either direction, which is what a world
    /// whose pack did not load gets.
    #[test]
    fn an_empty_table_answers_nothing() {
        let table: FurnaceRecipes<String> = FurnaceRecipes::new();
        assert!(table.is_empty());
        assert!(table.smelting_result(&"anything".to_owned()).is_none());
        assert!(table.first_recipe_by_result(&"anything".to_owned()).is_none());
    }

    /// `insert` is the same path `load` takes, one recipe at a time.
    #[test]
    fn insert_and_load_build_the_same_table() {
        let mut built = FurnaceRecipes::new();
        built.insert(Smelt { input: "sand", output: "glass" });
        let loaded = FurnaceRecipes::load([(vec![vec!["sand"]], "glass")]);
        assert_eq!(built, loaded);
    }
}
