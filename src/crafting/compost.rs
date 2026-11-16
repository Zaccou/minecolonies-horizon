//! `minecolonies:composting` — a recipe that is a **scale**, not a craft.
//!
//! Java reference: `api/crafting/CompostRecipe.java` (the recipe and its
//! serializer), `api/tileentities/AbstractTileEntityBarrel.java` (the barrel's
//! capacity) and `core/generation/CompostRecipeBuilder.java` (the datagen that
//! writes the three files).
//!
//! **Why it has no `result`.** A `CompostRecipe` builds its output in the
//! constructor — six `minecolonies:compost`, `COMPOST_RESULT`
//! (`CompostRecipe.java:31`, `:44`) — so the file says nothing about it. What the
//! file says is *how strong* the input is: `strength` 8 means eight compost per
//! barrel-load, 2 means two (`:63-69`).
//!
//! **The three shipped files, and what separates them.**
//!
//! | File | `strength` | Input |
//! | --- | ---: | --- |
//! | `compostables_rich.json` | 8 | food with saturation ≥ 1.0, or the `compostables_rich` tag |
//! | `compostables.json` | 4 | food with 0.5 ≤ saturation < 1.0, or a crop, or the `compostables` tag |
//! | `compostables_poor.json` | 2 | food with saturation < 0.5, or the `compostables_poor` tag |
//!
//! The gaps are deliberate and the half-open bounds are what draw them: the rich
//! file tests `min-saturation: 1.0` (inclusive), the ordinary one tests
//! `min 0.5` **and** `max 1.0` (exclusive), the poor one `max 0.5` (exclusive).
//! An item with exactly 0.5 saturation falls in the ordinary file and not in the
//! poor one; an item with exactly 1.0 is rich and not ordinary.
//!
//! **The one number Java would throw on.** `strength` is a divisor —
//! `MAX_ITEMS / strength` (`:127-130`, `MAX_ITEMS = 64` at
//! `AbstractTileEntityBarrel.java:14`) — and the reader takes any integer,
//! including zero. Java does not validate it: it throws `ArithmeticException` on
//! the first barrel tick. [`Refusal::StrengthNotPositive`] is this port saying so
//! at load instead.

use super::ingredient::Ingredient;
use super::{Facts, Refusal};
use mod_api::serde_json::Value;

/// `CompostRecipe.FERMENT_TIME` (`:30`): ticks a barrel takes. Informational —
/// Java's comment says it is "not configurable (nor actually used)" (`:71-76`).
pub const FERMENT_TIME: i32 = 24_000;

/// `CompostRecipe.COMPOST_RESULT` (`:31`): what one operation makes.
pub const COMPOST_RESULT: u32 = 6;

/// The item one operation makes (`CompostRecipe.java:44`).
pub const COMPOST_ITEM: &str = "minecolonies:compost";

/// `AbstractTileEntityBarrel.MAX_ITEMS` (`:14`): the barrel holds 64 items, and
/// that is the numerator of the per-item count.
pub const BARREL_CAPACITY: u32 = 64;

/// One `minecolonies:composting` file.
#[derive(Clone, Debug, PartialEq)]
pub struct Composting {
    /// How many compost one barrel-load is worth: 2, 4 or 8 in the three shipped
    /// files, any positive number in principle.
    pub strength: u32,
    /// The ingredients that count, as alternatives.
    pub input: Ingredient,
}

impl Composting {
    /// A composting recipe from its file.
    ///
    /// `input` is required — `Ingredient.fromJson(json.get("input"))` throws on
    /// `null` (`:146`) — and `strength` defaults to one (`:147`).
    pub fn parse(value: &Value) -> Result<Self, Refusal> {
        let input = Ingredient::parse(
            value
                .get("input")
                .ok_or(Refusal::MissingField { field: "input" })?,
            "input",
        )?;
        let strength = super::int_or(value, "strength", 1)?;
        let strength = u32::try_from(strength)
            .ok()
            .filter(|strength| *strength > 0)
            .ok_or(Refusal::StrengthNotPositive { strength })?;
        Ok(Self { strength, input })
    }

    /// How many items of this recipe's input one operation consumes:
    /// `MAX_ITEMS / strength`, truncated like Java's integer division
    /// (`:127-130`).
    ///
    /// 8 → 8 items, 4 → 16, 2 → 32, 1 → 64. A strength that does not divide 64
    /// truncates: 3 → 21, which leaves three items of the barrel unused.
    pub fn per_item(&self) -> u32 {
        match BARREL_CAPACITY.checked_div(self.strength) {
            Some(items) => items,
            // Unreachable through `parse`, which refuses a zero strength by name;
            // a hand-built value answers 0 rather than panicking here.
            None => 0,
        }
    }

    /// What one operation makes, as the constructor fixes it
    /// (`:39-45`): `minecolonies:compost` × 6, whatever the strength.
    pub fn output(&self) -> (&'static str, u32) {
        (COMPOST_ITEM, COMPOST_RESULT)
    }

    /// Ticks a barrel takes (`:71-77`).
    pub fn ferment_time(&self) -> i32 {
        FERMENT_TIME
    }

    /// Whether `item` is one of this recipe's inputs.
    pub fn admits(&self, item: &super::ItemId, facts: &dyn Facts) -> bool {
        self.input.admits(item, facts)
    }

    /// The same recipe narrowed to one item, which is what
    /// `CompostRecipe.individualize` does for JEI (`:132-138`, "JEI looks better
    /// if we render these as many individual recipes").
    pub fn individualize(&self, item: super::ItemId) -> Composting {
        Self {
            strength: self.strength,
            input: Ingredient::Item(item),
        }
    }

    /// Every item this recipe can name without a tag: the members of an array
    /// that are plain item ids. A tag or a food bound needs [`Facts`], so it is
    /// not here — a caller that wants the whole set asks
    /// [`Ingredient::admits`] per candidate.
    pub fn named_items(&self) -> Vec<&super::ItemId> {
        fn collect<'a>(ingredient: &'a Ingredient, out: &mut Vec<&'a super::ItemId>) {
            match ingredient {
                Ingredient::Any(alternatives) => {
                    for alternative in alternatives {
                        collect(alternative, out);
                    }
                }
                Ingredient::Item(item) => out.push(item),
                Ingredient::Tag(_) | Ingredient::Food(_) | Ingredient::Plant => {}
            }
        }
        let mut out = Vec::new();
        collect(&self.input, &mut out);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crafting::{ItemId, NoFacts};

    fn json(text: &str) -> Value {
        mod_api::serde_json::from_str(text).expect("a literal in a test is valid JSON")
    }

    // The three shipped files, verbatim.
    const COMPOSTABLES: &str = r#"{
        "type": "minecolonies:composting",
        "input": [
            {"type": "minecolonies:food", "max-saturation": 1.0, "min-saturation": 0.5},
            {"type": "minecolonies:plant"},
            {"tag": "minecolonies:compostables"}
        ],
        "strength": 4
    }"#;

    const POOR: &str = r#"{
        "type": "minecolonies:composting",
        "input": [
            {"type": "minecolonies:food", "max-saturation": 0.5},
            {"tag": "minecolonies:compostables_poor"}
        ],
        "strength": 2
    }"#;

    const RICH: &str = r#"{
        "type": "minecolonies:composting",
        "input": [
            {"type": "minecolonies:food", "min-saturation": 1.0},
            {"tag": "minecolonies:compostables_rich"}
        ],
        "strength": 8
    }"#;

    #[test]
    fn the_three_shipped_files_all_parse() {
        for (file, text, strength) in [
            ("compostables", COMPOSTABLES, 4),
            ("compostables_poor", POOR, 2),
            ("compostables_rich", RICH, 8),
        ] {
            let recipe = Composting::parse(&json(text))
                .unwrap_or_else(|refusal| panic!("{file} must parse: {refusal}"));
            assert_eq!(recipe.strength, strength, "{file}");
        }
    }

    #[test]
    fn the_per_item_count_is_the_barrel_capacity_over_the_strength() {
        for (strength, items) in [(8, 8), (4, 16), (2, 32), (1, 64)] {
            let recipe = Composting {
                strength,
                input: Ingredient::Plant,
            };
            assert_eq!(recipe.per_item(), items, "strength {strength}");
        }
        // Java's integer division truncates; 3 is not one of the shipped values
        // but nothing stops a pack writing it.
        let odd = Composting {
            strength: 3,
            input: Ingredient::Plant,
        };
        assert_eq!(odd.per_item(), 21);
    }

    #[test]
    fn the_output_is_fixed_and_ignores_the_strength() {
        for strength in [1, 2, 4, 8] {
            let recipe = Composting {
                strength,
                input: Ingredient::Plant,
            };
            assert_eq!(recipe.output(), ("minecolonies:compost", 6));
            assert_eq!(recipe.ferment_time(), 24_000);
        }
    }

    #[test]
    fn a_missing_input_is_a_named_refusal() {
        assert_eq!(
            Composting::parse(&json(r#"{"strength": 4}"#)),
            Err(Refusal::MissingField { field: "input" })
        );
    }

    #[test]
    fn a_missing_strength_is_one() {
        let recipe = Composting::parse(&json(r#"{"input": {"tag": "minecolonies:compostables"}}"#))
            .expect("parses");
        assert_eq!(recipe.strength, 1, "GsonHelper.getAsInt default: :147");
        assert_eq!(recipe.per_item(), 64);
    }

    #[test]
    fn a_strength_java_would_divide_by_is_refused_by_name() {
        assert_eq!(
            Composting::parse(&json(
                r#"{"input": {"tag": "minecolonies:compostables"}, "strength": 0}"#
            )),
            Err(Refusal::StrengthNotPositive { strength: 0 })
        );
        assert_eq!(
            Composting::parse(&json(
                r#"{"input": {"tag": "minecolonies:compostables"}, "strength": -4}"#
            )),
            Err(Refusal::StrengthNotPositive { strength: -4 })
        );
    }

    #[test]
    fn the_input_is_an_alternative_of_its_three_members() {
        let recipe = Composting::parse(&json(COMPOSTABLES)).expect("parses");
        let Ingredient::Any(alternatives) = &recipe.input else {
            panic!("compostables.json lists three alternatives");
        };
        assert_eq!(alternatives.len(), 3);
        assert!(matches!(alternatives[0], Ingredient::Food(_)));
        assert!(matches!(alternatives[1], Ingredient::Plant));
        assert!(matches!(alternatives[2], Ingredient::Tag(_)));
    }

    #[test]
    fn nothing_is_named_by_an_input_made_of_bounds_and_tags() {
        // The shipped files name no item at all: every member needs the host.
        for text in [COMPOSTABLES, POOR, RICH] {
            let recipe = Composting::parse(&json(text)).expect("parses");
            assert!(recipe.named_items().is_empty(), "{text}");
        }
    }

    #[test]
    fn individualize_narrows_the_input_to_one_item() {
        let recipe = Composting::parse(&json(COMPOSTABLES)).expect("parses");
        let leaves = ItemId::parse("item", "minecraft:oak_leaves").expect("parses");
        let one = recipe.individualize(leaves.clone());
        assert_eq!(one.strength, recipe.strength);
        assert_eq!(one.input, Ingredient::Item(leaves));
        assert_eq!(one.named_items().len(), 1);
    }

    #[test]
    fn a_member_of_no_tag_admits_nothing_without_a_world() {
        let recipe = Composting::parse(&json(COMPOSTABLES)).expect("parses");
        let facts = NoFacts;
        for id in ["minecraft:oak_leaves", "minecraft:wheat_seeds", "minecraft:stone"] {
            let item = ItemId::parse("item", id).expect("parses");
            assert!(
                !recipe.admits(&item, &facts),
                "{id} is in no tag and is no food this host knows"
            );
        }
    }
}
