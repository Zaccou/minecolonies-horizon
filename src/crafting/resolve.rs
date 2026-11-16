//! `type` field + file → the typed recipe, or the refusal that names why not.
//!
//! This is the one entry point the two loaders share. Java has two — the game's
//! `RecipeManager` for `recipe/` and `CrafterRecipeListener` for
//! `crafterrecipes/` — and they dispatch on the same `type` field in two
//! different vocabularies, which is why [`resolve`] takes the folder the file
//! came from ([`Origin`]) and refuses a file whose kind belongs to the other one.
//!
//! **Order of the checks, because the refusals name different mistakes.**
//!
//! 1. the file is a JSON object → [`Refusal::NotAnObject`]
//! 2. it has a `type` string → [`Refusal::MissingType`], [`Refusal::WrongType`]
//! 3. that string names a kind → [`Refusal::UnknownKind`]
//! 4. that kind belongs to this folder → [`Refusal::WrongOrigin`]
//! 5. the kind's own fields → whatever its parser refuses
//!
//! Steps 2 to 4 are what Java answers with silence: an unknown `type` falls
//! through its switch with no `default` (`CrafterRecipeListener.java:50-63`), and
//! a `type` whose serializer nobody registered makes the game refuse the **whole**
//! recipe table rather than the one file. Both are `Refusal`s here, which is the
//! point of the module.

use super::crafter::{CrafterRecipe, Removal, Template};
use super::{Composting, Kind, Origin, Refusal, ZeroWaste};
use mod_api::Identifier;
use mod_api::serde_json::Value;

/// What a recipe file turned out to be.
#[derive(Clone, Debug, PartialEq)]
pub enum Recipe {
    /// `minecolonies:composting` — a scale, not a craft.
    Composting(Composting),
    /// `minecolonies:zero_waste` — shapeless, and the leftovers vanish.
    ZeroWaste(ZeroWaste),
    /// `recipe`, `recipe-multi-out` or `recipe-multi-in` — one crafter's step.
    Crafter(CrafterRecipe),
    /// `recipe-template` — a recipe per tag member, expanded later.
    Template(Template),
    /// `remove` — an id to delete.
    Remove(Removal),
}

impl Recipe {
    /// The kind this value came from.
    pub fn kind(&self) -> Kind {
        match self {
            Self::Composting(_) => Kind::Composting,
            Self::ZeroWaste(_) => Kind::ZeroWaste,
            Self::Crafter(_) => Kind::CrafterRecipe,
            Self::Template(_) => Kind::Template,
            Self::Remove(_) => Kind::Remove,
        }
    }
}

/// The kind a file names, without parsing it — the cheap half of [`resolve`],
/// for a caller that only wants to sort files.
pub fn kind_of(origin: Origin, value: &Value) -> Result<Kind, Refusal> {
    super::fields(value)?;
    if !super::has(value, "type") {
        // Java reads the default "" and every case misses: silent drop.
        return Err(Refusal::MissingType);
    }
    let found = super::text(value, "type")?;
    let kind = Kind::from_type(found).ok_or_else(|| Refusal::UnknownKind {
        found: found.to_owned(),
    })?;
    if kind.origin() != origin {
        return Err(Refusal::WrongOrigin {
            found: found.to_owned(),
        });
    }
    Ok(kind)
}

/// The typed recipe a file holds.
///
/// `origin` is the folder it was read from: [`Origin::Recipes`] for
/// `data/<ns>/recipe/`, [`Origin::CrafterRecipes`] for
/// `data/<ns>/crafterrecipes/`.
pub fn resolve(origin: Origin, value: &Value) -> Result<Recipe, Refusal> {
    match kind_of(origin, value)? {
        Kind::Composting => Ok(Recipe::Composting(Composting::parse(value)?)),
        Kind::ZeroWaste => Ok(Recipe::ZeroWaste(ZeroWaste::parse(value)?)),
        // `recipe-multi-in` is declared and never handled in Java; the file still
        // parses as the same model, and `Kind::handled` is what tells a caller
        // that nothing will read it.
        Kind::CrafterRecipe | Kind::CrafterMultiOut | Kind::CrafterMultiIn => {
            Ok(Recipe::Crafter(CrafterRecipe::parse(value)?))
        }
        Kind::Template => Ok(Recipe::Template(Template::parse(value)?)),
        Kind::Remove => Ok(Recipe::Remove(Removal::parse(value)?)),
    }
}

/// A `ResourceLocation`, parsed rather than trusted. An empty path is refused
/// because `Identifier::from_parts` only checks the charset, so `""` would
/// otherwise parse as `minecraft:`.
pub(crate) fn ident(field: &'static str, text: &str) -> Result<Identifier, Refusal> {
    let id = Identifier::parse(text).map_err(|_| Refusal::UnparsableItem { field })?;
    if id.path().is_empty() {
        return Err(Refusal::UnparsableItem { field });
    }
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json(text: &str) -> Value {
        mod_api::serde_json::from_str(text).expect("a literal in a test is valid JSON")
    }

    // One shipped file per kind.
    const COMPOSTING: &str = r#"{
        "type": "minecolonies:composting",
        "input": [{"type": "minecolonies:plant"}],
        "strength": 4
    }"#;

    const ZERO_WASTE: &str = r#"{
        "type": "minecolonies:zero_waste",
        "ingredients": [{"item": "minecolonies:large_water_bottle"}, {"item": "minecolonies:soybean"}],
        "result": {"item": "minecolonies:large_soy_milk_bottle"}
    }"#;

    const CRAFTER: &str = r#"{
        "type": "recipe",
        "crafter": "stonemason_crafting",
        "inputs": [{"item": "minecraft:stone"}],
        "intermediate": "minecraft:air",
        "result": "minecraft:stone_bricks"
    }"#;

    const TEMPLATE: &str = r#"{
        "type": "recipe-template",
        "tag": "minecraft:logs",
        "recipe": {"crafter": "lumberjack_custom", "inputs": [{"item": "[NS]:[PATH]"}]}
    }"#;

    const REMOVE: &str = r#"{"type": "remove", "recipe-id-to-remove": "minecolonies:baker/bread"}"#;

    #[test]
    fn each_kind_resolves_from_its_own_folder() {
        let resolved = resolve(Origin::Recipes, &json(COMPOSTING)).expect("parses");
        assert_eq!(resolved.kind(), Kind::Composting);
        assert!(matches!(resolved, Recipe::Composting(_)));

        let resolved = resolve(Origin::Recipes, &json(ZERO_WASTE)).expect("parses");
        assert_eq!(resolved.kind(), Kind::ZeroWaste);

        let resolved = resolve(Origin::CrafterRecipes, &json(CRAFTER)).expect("parses");
        assert_eq!(resolved.kind(), Kind::CrafterRecipe);
        assert!(matches!(resolved, Recipe::Crafter(_)));

        let resolved = resolve(Origin::CrafterRecipes, &json(TEMPLATE)).expect("parses");
        assert_eq!(resolved.kind(), Kind::Template);

        let resolved = resolve(Origin::CrafterRecipes, &json(REMOVE)).expect("parses");
        assert_eq!(resolved.kind(), Kind::Remove);
    }

    #[test]
    fn the_two_multi_kinds_resolve_to_the_same_model() {
        for kind in ["recipe-multi-out", "recipe-multi-in"] {
            let text = CRAFTER.replace("\"recipe\"", &format!("\"{kind}\""));
            let resolved = resolve(Origin::CrafterRecipes, &json(&text))
                .unwrap_or_else(|refusal| panic!("{kind} must resolve: {refusal}"));
            assert!(matches!(resolved, Recipe::Crafter(_)), "{kind}");
        }
    }

    #[test]
    fn a_file_of_the_other_folder_is_refused_by_name() {
        assert_eq!(
            resolve(Origin::Recipes, &json(CRAFTER)),
            Err(Refusal::WrongOrigin {
                found: "recipe".into()
            })
        );
        assert_eq!(
            resolve(Origin::CrafterRecipes, &json(ZERO_WASTE)),
            Err(Refusal::WrongOrigin {
                found: "minecolonies:zero_waste".into()
            })
        );
    }

    #[test]
    fn a_type_nobody_registers_is_named_not_silent() {
        assert_eq!(
            resolve(Origin::Recipes, &json(r#"{"type": "minecolonies:compost"}"#)),
            Err(Refusal::UnknownKind {
                found: "minecolonies:compost".into()
            })
        );
        assert_eq!(
            resolve(
                Origin::Recipes,
                &json(r###"{"type": "minecraft:crafting_shaped", "pattern": ["##"]}"###)
            ),
            Err(Refusal::UnknownKind {
                found: "minecraft:crafting_shaped".into()
            }),
            "the port's resolver answers for this mod's kinds, not the vanilla ones"
        );
    }

    #[test]
    fn a_file_with_no_type_is_refused_where_java_drops_it_in_silence() {
        assert_eq!(
            resolve(Origin::CrafterRecipes, &json(r#"{"crafter": "baker_crafting"}"#)),
            Err(Refusal::MissingType)
        );
        assert_eq!(
            resolve(Origin::Recipes, &json(r#"{"type": 4}"#)),
            Err(Refusal::WrongType { field: "type" })
        );
    }

    #[test]
    fn a_file_that_is_not_an_object_is_refused() {
        for text in ["[]", "\"recipe\"", "3", "null"] {
            assert_eq!(
                resolve(Origin::Recipes, &json(text)),
                Err(Refusal::NotAnObject),
                "{text}"
            );
        }
    }

    #[test]
    fn kind_of_answers_without_reading_the_fields() {
        // The file has no `input`, so `resolve` would refuse it; `kind_of` does
        // not care.
        let partial = json(r#"{"type": "minecolonies:composting"}"#);
        assert_eq!(kind_of(Origin::Recipes, &partial), Ok(Kind::Composting));
        assert_eq!(
            resolve(Origin::Recipes, &partial),
            Err(Refusal::MissingField { field: "input" })
        );
    }

    #[test]
    fn a_kind_shows_through_before_its_fields_are_read() {
        // Wrong origin wins over a missing field: the folder is checked first, so
        // a mis-filed recipe is reported as mis-filed, not as incomplete.
        assert_eq!(
            kind_of(Origin::CrafterRecipes, &json(COMPOSTING)),
            Err(Refusal::WrongOrigin {
                found: "minecolonies:composting".into()
            })
        );
    }

    #[test]
    fn an_id_is_parsed_and_an_empty_path_is_refused() {
        assert_eq!(ident("loot-table", "minecolonies:recipes/baker").map(|id| id.to_string()), Ok("minecolonies:recipes/baker".to_owned()));
        assert_eq!(ident("loot-table", "baker"), Ok(Identifier::parse("minecraft:baker").expect("parses")));
        assert_eq!(
            ident("loot-table", ""),
            Err(Refusal::UnparsableItem {
                field: "loot-table"
            })
        );
        assert_eq!(
            ident("loot-table", "MineColonies:Recipes"),
            Err(Refusal::UnparsableItem {
                field: "loot-table"
            })
        );
    }

    #[test]
    fn every_refusal_from_a_shipped_file_is_reachable_from_resolve() {
        // A short tour: one refusal per check, all through the public entry point.
        assert_eq!(
            resolve(Origin::Recipes, &json("[]")),
            Err(Refusal::NotAnObject)
        );
        assert_eq!(
            resolve(Origin::Recipes, &json("{}")),
            Err(Refusal::MissingType)
        );
        assert_eq!(
            resolve(Origin::Recipes, &json(r#"{"type": "x:y"}"#)),
            Err(Refusal::UnknownKind {
                found: "x:y".into()
            })
        );
        assert_eq!(
            resolve(Origin::Recipes, &json(r#"{"type": "remove", "recipe-id-to-remove": "a:b"}"#)),
            Err(Refusal::WrongOrigin {
                found: "remove".into()
            })
        );
        assert_eq!(
            resolve(Origin::Recipes, &json(r#"{"type": "minecolonies:zero_waste"}"#)),
            Err(Refusal::MissingField { field: "result" })
        );
    }
}
