//! The four recipe files MineColonies ships that this port cannot load yet, kept
//! as content with the Java type each one needs — ready to be validated.
//!
//! # What these four are
//!
//! The pack has 158 recipe files. 154 are in `data/minecolonies/recipe/` and load
//! as they are; these four sit in `recettes-custom/`, byte for byte as the datagen
//! wrote them, because their `type` is not `minecraft:` and their ingredients are
//! not vanilla's:
//!
//! | File | `type` | Java class that decodes it |
//! | --- | --- | --- |
//! | `compostables.json` | `minecolonies:composting` | `CompostRecipe.Serializer` (`CompostRecipe.java:140-150`) |
//! | `compostables_poor.json` | `minecolonies:composting` | the same |
//! | `compostables_rich.json` | `minecolonies:composting` | the same |
//! | `soy_milk.json` | `minecolonies:zero_waste` | `ZeroWasteRecipe.Serializer` (`ZeroWasteRecipe.java:63-78`) |
//!
//! Java registers exactly two things for them
//! (`apiimp/initializer/ModRecipeSerializerInitializer.java:21`, `:24`): a
//! serializer `composting` **with a recipe type of its own** (`:22`, because
//! `CompostRecipe` implements `Recipe` and has to name its type), and a serializer
//! `zero_waste` with **none** — `ZeroWasteRecipe` extends `ShapelessRecipe`, so
//! `getType()` already answers `minecraft:crafting`.
//!
//! # The premise that is false, and the one that is not
//!
//! The reason these files are outside `data/` is stated as *"an unknown `type`
//! makes the engine refuse the whole recipe table"*. In the pinned tree that is
//! **not what happens**: a file whose type is outside `minecraft:` is kept as a
//! snapshot of that type (`crates/horizon-client/src/crafting.rs:1625-1643`) and
//! a mod reads it back through `Host::recipes_of_type`
//! (`.horizon/sdk/api/crates/horizon-mod-vocab/src/host/recipes.rs:29`). One
//! `composting` file in `data/` would cost the other 154 nothing.
//!
//! What the engine *does* refuse is a file naming an **ingredient kind** nobody
//! answers: `crafting.rs:1521-1527` drops that one file, naming it and the kind,
//! and `:1529-1532` keeps the file but warns it cannot be placed when the answer
//! is empty. The three `compostables` files write `{"type": "minecolonies:food"}`
//! and `{"type": "minecolonies:plant"}` — the two ingredient serializers Java
//! registers at `MineColonies.java:269-270` — so they are files the engine drops,
//! for a reason the brief does not mention. The hook that answers them exists in
//! the pinned SDK
//! (`.horizon/sdk/api/crates/horizon-mod-vocab/src/hooks/ingredient.rs:1-36`) and
//! nothing in this port implements it: `MANQUANT.md` §1 carries that gap, and
//! [`Excluded::readiness`] is what states it mechanically instead of by memory.
//!
//! # Ready to be validated, not ready to be shipped
//!
//! [`Excluded::parse`] runs each payload through the typed recipes of
//! `src/crafting/` **today** — a compost strength, a zero-waste ingredient list —
//! so a mistake in the copy is caught now rather than on the day the types land.
//! [`Excluded::readiness`] takes the two lists a caller has to supply (the
//! serializers it registered, the ingredient kinds its hook answers) and says what
//! the engine will do with each file. No state is guessed: an empty list means
//! "registered nothing", never "everything works".

use crate::crafting::{resolve, Kind, Origin, Recipe, Refusal};
use mod_api::serde_json::Value;

/// One Java class that reads a payload of this port, with the lines that prove it.
#[derive(Clone, Copy, Debug)]
pub struct JavaType {
    /// The class, as Java names it.
    pub class: &'static str,
    /// Where the serializer is registered, `file.java:line`.
    pub serializer: &'static str,
    /// Where its recipe type is registered, `file.java:line`, when it has one.
    /// `None` means the recipe answers a vanilla type — [`JavaType::recipe_type`].
    pub type_decl: Option<&'static str>,
    /// The `Serializer.fromJson` that decodes the file, `file.java:lines`.
    pub reader: &'static str,
}

/// The type a recipe answers when Java registers no type of its own.
pub const VANILLA_CRAFTING: &str = "minecraft:crafting";

/// One excluded file: its bytes, its type, and where Java reads it.
#[derive(Clone, Copy, Debug)]
pub struct Excluded {
    /// The file name inside `recettes-custom/`, and the name the datagen gives it.
    pub file: &'static str,
    /// The file's bytes, exactly as shipped — the verbatim test compares them.
    pub json: &'static str,
    /// The kind its `type` field names, as `src/crafting/` reads it.
    pub kind: Kind,
    /// What the datagen wrote it from, `file.java:lines`.
    pub generated: &'static str,
    pub java: JavaType,
}

impl Excluded {
    /// The payload as a JSON object, or [`Refusal::NotAnObject`] — the refusal
    /// Java's `GsonHelper.getAsJsonObject` path produces, and the state every
    /// other method here would otherwise have to re-check.
    pub fn value(&self) -> Result<Value, Refusal> {
        let value: Value =
            mod_api::serde_json::from_str(self.json).map_err(|_| Refusal::NotAnObject)?;
        if value.is_object() {
            Ok(value)
        } else {
            Err(Refusal::NotAnObject)
        }
    }

    /// The typed recipe this file becomes, through [`crate::crafting::resolve`] —
    /// the same path every file of `data/minecolonies/recipe/` takes.
    pub fn parse(&self) -> Result<Recipe, Refusal> {
        resolve(Origin::Recipes, &self.value()?)
    }

    /// The ingredient kinds the file names that are not vanilla's, in file order,
    /// deduplicated. A `minecolonies:food` alternative and a `minecolonies:plant`
    /// alternative are two kinds.
    pub fn ingredient_kinds(&self) -> Result<Vec<String>, Refusal> {
        Ok(custom_ingredient_kinds(&self.value()?))
    }

    /// What the engine will do with this file, from the two lists the caller
    /// knows: the recipe serializers it registered, and the ingredient kinds its
    /// `ingredient` hook answers (`None` meaning "no hook at all").
    pub fn readiness(
        &self,
        registered_types: &[&str],
        answered: Option<&[&str]>,
    ) -> Result<Readiness, Refusal> {
        let kinds = self.ingredient_kinds()?;
        let unanswered: Vec<String> = match answered {
            None => kinds.clone(),
            Some(answered) => kinds
                .iter()
                .filter(|kind| !answered.iter().any(|known| known == &kind.as_str()))
                .cloned()
                .collect(),
        };
        let loads = match answered {
            // No listener: the engine refuses the file (`crafting.rs:1521-1527`).
            None if !kinds.is_empty() => Loads::Dropped { kinds: unanswered },
            // A listener that answers nothing for this kind: kept, unplaceable
            // (`crafting.rs:1529-1532`).
            Some(_) if !unanswered.is_empty() => Loads::Unplaceable { kinds: unanswered },
            _ => Loads::Snapshot,
        };
        Ok(Readiness {
            loads,
            decodable: registered_types.contains(&self.kind.type_name()),
            kinds,
        })
    }
}

/// What the load does with one file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Loads {
    /// The file survives as a snapshot of its own type, which a mod reads back
    /// through `Host::recipes_of_type`.
    Snapshot,
    /// The file survives, but an ingredient kind it names is answered with no
    /// item: it cannot be placed.
    Unplaceable { kinds: Vec<String> },
    /// The engine drops the file at load, naming it and the kind.
    Dropped { kinds: Vec<String> },
}

impl Loads {
    /// Whether the file reaches the load at all.
    pub fn is_loaded(&self) -> bool {
        !matches!(self, Self::Dropped { .. })
    }
}

/// The answer of [`Excluded::readiness`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Readiness {
    pub loads: Loads,
    /// Whether a serializer is registered for this file's `type`, which is what
    /// lets a mod decode it into something typed instead of raw JSON.
    pub decodable: bool,
    /// The custom ingredient kinds the payload names, file order.
    pub kinds: Vec<String>,
}

/// The two keys of a recipe file that hold ingredients in these four files.
///
/// The engine reads 23 of them (`crafting.rs:1319-1342`, `INGREDIENT_KEYS`);
/// these four files use two, which is measured, not assumed: `input` for the
/// three `composting` files and `ingredients` for `soy_milk`.
pub const INGREDIENT_KEYS: [&str; 2] = ["input", "ingredients"];

/// Every ingredient kind of `value` that is not vanilla's, in file order.
///
/// The rule is the engine's (`crafting/ingredient.rs:31-32`): an object whose
/// `type` — or `neoforge:ingredient_type` — contains `:` and does not start with
/// `minecraft:`. The recipe's own `type` is **not** an ingredient and is never
/// read here: the walk starts below the root, at [`INGREDIENT_KEYS`].
pub fn custom_ingredient_kinds(value: &Value) -> Vec<String> {
    let mut found = Vec::new();
    for key in INGREDIENT_KEYS {
        if let Some(part) = value.get(key) {
            collect_kinds(part, &mut found);
        }
    }
    found
}

fn collect_kinds(value: &Value, found: &mut Vec<String>) {
    match value {
        Value::Array(items) => items.iter().for_each(|item| collect_kinds(item, found)),
        Value::Object(object) => {
            let kind = object
                .get("type")
                .or_else(|| object.get("neoforge:ingredient_type"))
                .and_then(Value::as_str);
            if let Some(kind) = kind {
                if kind.contains(':')
                    && !kind.starts_with("minecraft:")
                    && !found.iter().any(|seen: &String| seen.as_str() == kind)
                {
                    found.push(kind.to_owned());
                }
            }
            // Nested alternatives: an ingredient object can hold more of them.
            for key in INGREDIENT_KEYS {
                if let Some(part) = object.get(key) {
                    collect_kinds(part, found);
                }
            }
        }
        _ => {}
    }
}

/// The four files, as the datagen wrote them.
///
/// Order is the pack's: the three `composting` files by increasing strength, then
/// the one `zero_waste` file.
pub const EXCLUDED: &[Excluded] = &[
    Excluded {
        file: "compostables_poor.json",
        json: r#"{
  "type": "minecolonies:composting",
  "input": [
    {
      "type": "minecolonies:food",
      "max-saturation": 0.5
    },
    {
      "tag": "minecolonies:compostables_poor"
    }
  ],
  "strength": 2
}"#,
        kind: Kind::Composting,
        generated: "DefaultRecipeProvider.java:52-55 (strength 2, `FoodIngredient` bounded at \
                    0.5 saturation, plus the `compostables_poor` tag)",
        java: JavaType {
            class: "CompostRecipe.Serializer",
            serializer: "ModRecipeSerializerInitializer.java:21",
            type_decl: Some("ModRecipeSerializerInitializer.java:22"),
            reader: "CompostRecipe.java:144-150",
        },
    },
    Excluded {
        file: "compostables.json",
        json: r#"{
  "type": "minecolonies:composting",
  "input": [
    {
      "type": "minecolonies:food",
      "max-saturation": 1.0,
      "min-saturation": 0.5
    },
    {
      "type": "minecolonies:plant"
    },
    {
      "tag": "minecolonies:compostables"
    }
  ],
  "strength": 4
}"#,
        kind: Kind::Composting,
        generated: "DefaultRecipeProvider.java:56-59 (strength 4, the half-open food band, a \
                    `PlantIngredient`, plus the `compostables` tag)",
        java: JavaType {
            class: "CompostRecipe.Serializer",
            serializer: "ModRecipeSerializerInitializer.java:21",
            type_decl: Some("ModRecipeSerializerInitializer.java:22"),
            reader: "CompostRecipe.java:144-150",
        },
    },
    Excluded {
        file: "compostables_rich.json",
        json: r#"{
  "type": "minecolonies:composting",
  "input": [
    {
      "type": "minecolonies:food",
      "min-saturation": 1.0
    },
    {
      "tag": "minecolonies:compostables_rich"
    }
  ],
  "strength": 8
}"#,
        kind: Kind::Composting,
        generated: "DefaultRecipeProvider.java:60-63 (strength 8, food from 1.0 saturation, plus \
                    the `compostables_rich` tag)",
        java: JavaType {
            class: "CompostRecipe.Serializer",
            serializer: "ModRecipeSerializerInitializer.java:21",
            type_decl: Some("ModRecipeSerializerInitializer.java:22"),
            reader: "CompostRecipe.java:144-150",
        },
    },
    Excluded {
        file: "soy_milk.json",
        json: r#"{
  "type": "minecolonies:zero_waste",
  "ingredients": [
    {
      "item": "minecolonies:large_water_bottle"
    },
    {
      "item": "minecolonies:soybean"
    }
  ],
  "result": {
    "item": "minecolonies:large_soy_milk_bottle"
  }
}"#,
        kind: Kind::ZeroWaste,
        generated: "DefaultRecipeProvider.java:966-970 (`large_water_bottle` + the soybean \
                    block's item, one soy milk)",
        java: JavaType {
            class: "ZeroWasteRecipe.Serializer",
            serializer: "ModRecipeSerializerInitializer.java:24",
            // `ShapelessRecipe.getType()` answers `RecipeType.CRAFTING`, and Java
            // registers no type for this serializer.
            type_decl: None,
            reader: "ZeroWasteRecipe.java:63-78",
        },
    },
];

/// The file of `EXCLUDED` whose name is `file`.
pub fn find(file: &str) -> Option<&'static Excluded> {
    EXCLUDED.iter().find(|entry| entry.file == file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crafting::register::{COMPOSTING, ZERO_WASTE};
    use crate::crafting::Recipe;

    const DIR: &str = "recettes-custom";

    fn shipped(file: &str) -> Option<String> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(DIR)
            .join(file);
        std::fs::read_to_string(path).ok()
    }

    #[test]
    fn the_four_files_are_the_four_excluded_ones() {
        let names: Vec<&str> = EXCLUDED.iter().map(|entry| entry.file).collect();
        assert_eq!(
            names,
            [
                "compostables_poor.json",
                "compostables.json",
                "compostables_rich.json",
                "soy_milk.json",
            ]
        );
        assert_eq!(EXCLUDED.len(), 4);
        assert!(find("soy_milk.json").is_some());
        assert!(find("blockhutbuilder.json").is_none());
    }

    #[test]
    fn every_file_cites_the_java_class_and_lines_that_decode_it() {
        for entry in EXCLUDED {
            let java = entry.java;
            assert!(java.class.contains("Serializer"), "{}", java.class);
            for citation in [java.serializer, java.reader] {
                assert!(citation.contains(".java:"), "{} cites no line", citation);
            }
            assert!(java.reader.contains("CompostRecipe") || java.reader.contains("ZeroWasteRecipe"));
            assert!(
                entry.generated.contains(".java:"),
                "{} says who generated it",
                entry.file
            );
        }
        assert_eq!(
            EXCLUDED[0].java.type_decl,
            Some("ModRecipeSerializerInitializer.java:22"),
            "composting declares a type of its own"
        );
        assert_eq!(
            EXCLUDED[3].java.type_decl,
            None,
            "zero_waste answers minecraft:crafting, so it declares none"
        );
        assert_eq!(
            crate::crafting::register::CRAFTING,
            VANILLA_CRAFTING,
            "the type zero_waste answers is the one the crate registers for it"
        );
    }

    #[test]
    fn every_payload_parses_through_the_typed_recipe_of_the_port() {
        for entry in EXCLUDED {
            let recipe = entry
                .parse()
                .unwrap_or_else(|refusal| panic!("{}: {refusal:?}", entry.file));
            assert_eq!(recipe.kind(), entry.kind, "{}", entry.file);
        }
    }

    #[test]
    fn the_three_composting_files_are_the_three_strengths_datagen_writes() {
        let strengths: Vec<u32> = EXCLUDED
            .iter()
            .filter_map(|entry| match entry.parse() {
                Ok(Recipe::Composting(composting)) => Some(composting.strength),
                _ => None,
            })
            .collect();
        assert_eq!(strengths, [2, 4, 8], "poor, ordinary, rich");
        for entry in EXCLUDED.iter().filter(|entry| entry.kind == Kind::Composting) {
            let Ok(Recipe::Composting(composting)) = entry.parse() else {
                panic!("{} is not a composting recipe", entry.file);
            };
            assert_eq!(
                composting.output(),
                ("minecolonies:compost", 6),
                "the result is built by the constructor, so the file never names it"
            );
            assert_eq!(
                composting.ferment_time(),
                24_000,
                "CompostRecipe.java:30 FERMENT_TIME"
            );
            assert!(composting.per_item() > 0);
        }
    }

    #[test]
    fn a_stronger_input_needs_fewer_items_per_operation() {
        // 64 / strength, Java's own integer division (`CompostRecipe.java:127-130`).
        let per_item: Vec<u32> = EXCLUDED
            .iter()
            .filter_map(|entry| match entry.parse() {
                Ok(Recipe::Composting(composting)) => Some(composting.per_item()),
                _ => None,
            })
            .collect();
        assert_eq!(per_item, [32, 16, 8], "a stronger input needs fewer items");
    }

    #[test]
    fn soy_milk_is_a_zero_waste_recipe_of_two_items() {
        let entry = find("soy_milk.json").expect("the file is in the table");
        let Ok(Recipe::ZeroWaste(recipe)) = entry.parse() else {
            panic!("soy_milk.json is not a zero_waste recipe");
        };
        assert_eq!(recipe.ingredients.len(), 2);
        assert_eq!(recipe.count, 1, "no `count` in the file, one by default");
        assert_eq!(recipe.result.to_string(), "minecolonies:large_soy_milk_bottle");
        assert_eq!(
            recipe.remainders().len(),
            2,
            "the empty bottle does not come back, and a slot per ingredient is what says so"
        );
        assert!(recipe.remainders().iter().all(Option::is_none));
    }

    #[test]
    fn the_types_are_the_two_the_crate_registers() {
        assert_eq!(Kind::Composting.type_name(), COMPOSTING);
        assert_eq!(Kind::ZeroWaste.type_name(), ZERO_WASTE);
        for entry in EXCLUDED {
            assert_eq!(entry.kind.origin(), Origin::Recipes, "{}", entry.file);
            let named = entry.kind.type_name();
            let expected = if entry.kind == Kind::Composting {
                COMPOSTING
            } else {
                ZERO_WASTE
            };
            assert_eq!(named, expected, "{}", entry.file);
        }
    }

    #[test]
    fn the_composting_files_name_the_two_custom_ingredient_kinds() {
        for entry in EXCLUDED.iter().filter(|entry| entry.kind == Kind::Composting) {
            let kinds = entry
                .ingredient_kinds()
                .unwrap_or_else(|refusal| panic!("{}: {refusal:?}", entry.file));
            assert!(
                kinds.iter().any(|kind| kind == "minecolonies:food"),
                "{} names no food ingredient: {kinds:?}",
                entry.file
            );
            assert!(
                kinds.iter().all(|kind| kind == "minecolonies:food"
                    || kind == "minecolonies:plant"),
                "{} names an unexpected kind: {kinds:?}",
                entry.file
            );
        }
        let both = find("compostables.json")
            .expect("the file is in the table")
            .ingredient_kinds()
            .expect("the payload parses");
        assert_eq!(
            both,
            ["minecolonies:food", "minecolonies:plant"],
            "file order, deduplicated"
        );
        let poor = find("compostables_poor.json")
            .expect("the file is in the table")
            .ingredient_kinds()
            .expect("the payload parses");
        assert_eq!(poor, ["minecolonies:food"], "no plant alternative");
    }

    #[test]
    fn soy_milk_names_no_custom_ingredient_at_all() {
        let kinds = find("soy_milk.json")
            .expect("the file is in the table")
            .ingredient_kinds()
            .expect("the payload parses");
        assert!(kinds.is_empty(), "{kinds:?}");
    }

    #[test]
    fn a_custom_kind_nested_in_an_alternative_is_found_and_the_recipe_type_is_not() {
        let value: Value = mod_api::serde_json::from_str(
            r#"{
              "type": "minecolonies:composting",
              "input": [
                {"type": "minecolonies:food", "min-saturation": 1.0},
                {"item": "minecraft:water_bucket"},
                {"tag": "minecraft:saplings"}
              ],
              "strength": 8
            }"#,
        )
        .expect("a literal in a test is valid JSON");
        assert_eq!(
            custom_ingredient_kinds(&value),
            ["minecolonies:food"],
            "the recipe's own type is not an ingredient"
        );
    }

    #[test]
    fn without_an_ingredient_hook_the_three_composting_files_are_dropped() {
        for entry in EXCLUDED.iter().filter(|entry| entry.kind == Kind::Composting) {
            let readiness = entry
                .readiness(&[COMPOSTING, ZERO_WASTE], None)
                .unwrap_or_else(|refusal| panic!("{}: {refusal:?}", entry.file));
            assert!(
                matches!(readiness.loads, Loads::Dropped { .. }),
                "{} should be refused, got {:?}",
                entry.file,
                readiness.loads
            );
            assert!(!readiness.loads.is_loaded());
            assert!(
                readiness.decodable,
                "the serializer is registered, so the payload is decodable"
            );
            assert!(!readiness.kinds.is_empty());
        }
    }

    #[test]
    fn soy_milk_reaches_the_load_with_nothing_registered() {
        let entry = find("soy_milk.json").expect("the file is in the table");
        let readiness = entry.readiness(&[], None).expect("the payload parses");
        assert_eq!(readiness.loads, Loads::Snapshot);
        assert!(readiness.loads.is_loaded());
        assert!(
            !readiness.decodable,
            "no serializer registered: the file is raw JSON, not a typed value"
        );
        assert!(readiness.kinds.is_empty());
    }

    #[test]
    fn answering_the_ingredient_kinds_makes_the_composting_files_load() {
        let answered = ["minecolonies:food", "minecolonies:plant"];
        for entry in EXCLUDED {
            let readiness = entry
                .readiness(&[], Some(&answered))
                .unwrap_or_else(|refusal| panic!("{}: {refusal:?}", entry.file));
            assert_eq!(readiness.loads, Loads::Snapshot, "{}", entry.file);
            assert!(!readiness.decodable, "nothing was registered in this run");
        }
    }

    #[test]
    fn a_kind_answered_by_nobody_is_kept_but_cannot_be_placed() {
        let answered = ["minecolonies:food"];
        let readiness = find("compostables.json")
            .expect("the file is in the table")
            .readiness(&[], Some(&answered))
            .expect("the payload parses");
        match readiness.loads {
            Loads::Unplaceable { kinds } => assert_eq!(kinds, ["minecolonies:plant"]),
            other => panic!("expected an unplaceable file, got {other:?}"),
        }
    }

    #[test]
    fn the_payloads_are_the_shipped_files_byte_for_byte() {
        for entry in EXCLUDED {
            let Some(text) = shipped(entry.file) else {
                continue;
            };
            assert_eq!(
                text.trim_end(),
                entry.json.trim_end(),
                "{} differs from the copy in {DIR}/",
                entry.file
            );
        }
    }

    #[test]
    fn a_payload_that_is_not_an_object_is_refused_by_name() {
        let broken = Excluded {
            file: "broken.json",
            json: "[]",
            kind: Kind::Composting,
            generated: "DefaultRecipeProvider.java:52-55",
            java: EXCLUDED[0].java,
        };
        assert_eq!(broken.parse(), Err(Refusal::NotAnObject));
        assert_eq!(broken.ingredient_kinds(), Err(Refusal::NotAnObject));
        assert!(broken.readiness(&[], None).is_err());
    }
}
