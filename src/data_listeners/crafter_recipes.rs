//! `crafterrecipes/*.json`: five declared types, four handled, and a table that is
//! still incomplete when the listener returns.
//!
//! Java reference: `core/datalistener/CrafterRecipeListener.java` (73 lines),
//! `core/colony/crafting/CustomRecipe.java:47-77` for the type names,
//! `CustomRecipeManager.java:87-302` for the store, and the 163 shipped files
//! (161 `recipe`, 2 `recipe-template`, counted with `grep -c type`).
//!
//! **The type table.** `CustomRecipe` declares five type strings (`:52-62`); the
//! listener's `switch` has four cases and **no `default`** (`:53-66`). The missing
//! one is `recipe-multi-in`, declared and never handled:
//!
//! | Type | Listener | Effect |
//! | --- | --- | --- |
//! | `recipe` | `:54-56` | added |
//! | `recipe-multi-out` | `:54-56` | added |
//! | `recipe-template` | `:57-59` | stored raw, parsed after the tags load |
//! | `remove` | `:60-63` | deferred to the next read |
//! | `recipe-multi-in` | **absent** | silently ignored, not even a log line |
//! | `""` (no `type` field, the default at `:52`) | **absent** | silently ignored |
//!
//! No shipped file uses `recipe-multi-in`, so nothing is broken today; a data pack
//! that did would get a crafter that cannot craft, with no diagnostic anywhere.
//! [`Fate::Ignored`] is that outcome, named.
//!
//! **Why a removal does not depend on the order of the pack.** `removeRecipe`
//! only appends the id to a list (`CustomRecipeManager.java:115-122`); the entries
//! are actually dropped in `removeRecipes`, called from `getRecipes` (`:154`,
//! `:249-275`), i.e. **after** the whole pack is read. So a `remove` file and the
//! `recipe` file it kills may arrive in either order and the answer is the same —
//! [`Table::read`] is where the removal lands, and the two orders are a test.
//!
//! **Two phases, and the window between them.** A template cannot be parsed before
//! the item tags exist (`resolveTemplates`, `:280-302`, called from
//! `DataPackSyncEventHandler.java:56`), so a `recipe-template` row is a *promise*
//! and not a recipe until then. [`super::timeline`] measures that window.
//!
//! **The catch that is narrower than its neighbours.** Every other listener catches
//! `Exception` (`CitizenNameListener.java:84`, `DiseasesListener.java:145` …);
//! this one catches `JsonParseException` (`:63`). An `IllegalArgumentException` or
//! an NPE thrown by `CustomRecipe.parse` therefore leaves `apply`, aborts the rest
//! of the pack, and propagates out of the reload listener — with the store already
//! reset at `:48`. [`caught`] keeps that asymmetry as a value.

use super::Refused;

/// The five `RECIPE_TYPE_*` strings, in declaration order (`CustomRecipe.java:52-62`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the five constants, and no pack can add a sixth
pub enum Kind {
    /// `recipe` — `CustomRecipe.java:52`.
    Recipe,
    /// `recipe-multi-out` — `:57`.
    MultiOut,
    /// `recipe-multi-in` — `:62`, **no case in the listener**.
    MultiIn,
    /// `recipe-template` — `:67`.
    Template,
    /// `remove` — `:72`.
    Remove,
}

impl Kind {
    /// The five, with the spelling the JSON holds.
    pub const ALL: &'static [(Kind, &'static str)] = &[
        (Kind::Recipe, "recipe"),
        (Kind::MultiOut, "recipe-multi-out"),
        (Kind::MultiIn, "recipe-multi-in"),
        (Kind::Template, "recipe-template"),
        (Kind::Remove, "remove"),
    ];

    /// The spelling, as `GsonHelper.getAsString(recipeJson, "type", "")` returns it.
    pub fn name(self) -> &'static str {
        Kind::ALL
            .iter()
            .find(|(kind, _)| *kind == self)
            .map(|(_, name)| *name)
            .unwrap_or("")
    }
}

/// What the listener did with one file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four statements in the switch, and the fall-through
pub enum Fate {
    /// `addRecipe` (`:55`).
    Added,
    /// `addRecipeTemplate` (`:58`): a promise, resolved after the tags.
    Deferred,
    /// `removeRecipe` (`:61`): dropped at the next read, not now.
    Removed,
    /// The `switch` fell through with no case: no log, no entry (`:53-66`).
    Ignored,
}

/// Why a recipe file is refused, named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// A `type` that is neither of the five nor the empty default. **Stricter than
    /// Java**, whose `switch` says nothing (`:53-66`).
    UnknownType { id: &'static str, kind: &'static str },
    /// `recipe-multi-in`: declared at `CustomRecipe.java:62`, unhandled at `:53-66`.
    /// The port refuses it, because the file meant something.
    DeclaredButUnhandled { id: &'static str },
}

impl Refused for Refusal {
    fn name(&self) -> String {
        match *self {
            Refusal::UnknownType { id, kind } => {
                format!("data_listeners.refusal.unknown_recipe_type({id}.{kind})")
            }
            Refusal::DeclaredButUnhandled { id } => {
                format!("data_listeners.refusal.declared_but_unhandled_recipe_type({id})")
            }
        }
    }
}

/// The only failure the listener's `catch` sees (`:63`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the catch is `JsonParseException`, everything else escapes
pub enum Failure {
    /// A malformed JSON document — what `JsonParseException` covers.
    Parse,
    /// Anything else a producer throws: a bad ingredient, a missing field, a null.
    Other,
}

/// Whether `apply` survives this failure.
///
/// `false` means the exception leaves the listener, aborts the remaining files and
/// escapes the reload — with the store already reset (`:48`).
pub fn caught(failure: Failure) -> bool {
    failure == Failure::Parse
}

/// Reads one file's `type` and says what happens to it.
pub fn classify(id: &'static str, type_prop: &'static str) -> Result<(Kind, Fate), Refusal> {
    match type_prop {
        "recipe" => Ok((Kind::Recipe, Fate::Added)),
        "recipe-multi-out" => Ok((Kind::MultiOut, Fate::Added)),
        "recipe-template" => Ok((Kind::Template, Fate::Deferred)),
        "remove" => Ok((Kind::Remove, Fate::Removed)),
        "recipe-multi-in" => Err(Refusal::DeclaredButUnhandled { id }),
        other => Err(Refusal::UnknownType { id, kind: other }),
    }
}

/// The crafter-keyed store, reduced to what decides an order: the recipes, the
/// removals still pending, and the templates still unresolved.
///
/// Not the 161 parsed recipes — `crafting::crafter` owns those. What is here is the
/// **sequencing** `CustomRecipeManager` applies to them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Table {
    pub recipes: Vec<&'static str>,
    pub pending_removals: Vec<&'static str>,
    pub templates: Vec<&'static str>,
}

impl Table {
    pub fn new() -> Table {
        Table::default()
    }

    /// `CustomRecipeManager.reset` (`:138-146`): every map, list and template
    /// cleared, so a reload never inherits another world's recipes.
    pub fn reset(&mut self) {
        self.recipes.clear();
        self.pending_removals.clear();
        self.templates.clear();
    }

    /// One accepted file.
    pub fn accept(&mut self, id: &'static str, kind: Kind) -> Fate {
        match kind {
            Kind::Recipe | Kind::MultiOut => {
                if !self.recipes.contains(&id) {
                    self.recipes.push(id);
                }
                Fate::Added
            }
            Kind::Template => {
                self.templates.push(id);
                Fate::Deferred
            }
            Kind::Remove => {
                // `removeRecipe` (`:115-122`): recorded, not applied.
                if !self.pending_removals.contains(&id) {
                    self.pending_removals.push(id);
                }
                Fate::Removed
            }
            Kind::MultiIn => Fate::Ignored,
        }
    }

    /// `getRecipes`, which is `removeRecipes` then the list (`:154`, `:249-275`).
    ///
    /// This is the only place a pending removal takes effect, which is why the
    /// order of the pack cannot change the answer.
    pub fn read(&mut self) -> Vec<&'static str> {
        for doomed in std::mem::take(&mut self.pending_removals) {
            self.recipes.retain(|recipe| *recipe != doomed);
        }
        self.recipes.clone()
    }

    /// `resolveTemplates` (`:282-302`): every template becomes the recipes it
    /// describes, then the raw list is cleared.
    ///
    /// `produced` is the parser's answer for each template id, which is the part
    /// this module does not do.
    pub fn resolve_templates(&mut self, produced: &[(&'static str, &'static [&'static str])]) {
        for template in std::mem::take(&mut self.templates) {
            if let Some((_, ids)) = produced.iter().find(|(id, _)| *id == template) {
                for id in *ids {
                    if !self.recipes.contains(id) {
                        self.recipes.push(*id);
                    }
                }
            }
        }
    }

    /// Whether the table is complete: no template left unresolved.
    pub fn complete(&self) -> bool {
        self.templates.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_types_are_declared_and_four_are_handled() {
        assert_eq!(Kind::ALL.len(), 5);
        let handled: Vec<&str> = Kind::ALL
            .iter()
            .filter(|(kind, _)| classify("x", kind.name()).is_ok())
            .map(|(_, name)| *name)
            .collect();
        assert_eq!(handled, vec!["recipe", "recipe-multi-out", "recipe-template", "remove"]);
        assert_eq!(Kind::Recipe.name(), "recipe");
    }

    #[test]
    fn recipe_multi_in_is_refused_as_declared_and_unhandled() {
        // `CustomRecipe.java:62` declares it; `CrafterRecipeListener.java:53-66` has
        // no case, and no `default` either.
        let refusal = classify("crafterrecipes/x", "recipe-multi-in").expect_err("declared, unhandled");
        assert_eq!(
            refusal.name(),
            "data_listeners.refusal.declared_but_unhandled_recipe_type(crafterrecipes/x)"
        );
    }

    #[test]
    fn an_empty_or_unknown_type_is_refused_instead_of_falling_through() {
        // `GsonHelper.getAsString(recipeJson, "type", "")` (`:52`): absent means
        // empty, and the empty string matches no case.
        for spelling in ["", "recipes", "Remove"] {
            let refusal = classify("crafterrecipes/x", spelling).expect_err("no case");
            assert_eq!(
                refusal.name(),
                format!("data_listeners.refusal.unknown_recipe_type(crafterrecipes/x.{spelling})")
            );
        }
        assert_eq!(classify("x", "recipe").map(|(_, fate)| fate), Ok(Fate::Added));
        assert_eq!(classify("x", "recipe-template").map(|(_, fate)| fate), Ok(Fate::Deferred));
        assert_eq!(classify("x", "remove").map(|(_, fate)| fate), Ok(Fate::Removed));
    }

    #[test]
    fn a_removal_gives_the_same_answer_whichever_order_the_pack_arrives_in() {
        let mut first = Table::new();
        first.accept("a", Kind::Remove);
        first.accept("a", Kind::Recipe);
        let mut second = Table::new();
        second.accept("a", Kind::Recipe);
        second.accept("a", Kind::Remove);
        // Nothing is decided until the read, so both orders lose the recipe.
        assert!(!first.read().contains(&"a"));
        assert!(!second.read().contains(&"a"));
        assert!(first.pending_removals.is_empty(), "a read consumes the list");
    }

    #[test]
    fn a_template_is_a_promise_and_not_a_recipe_until_it_is_resolved() {
        let mut table = Table::new();
        assert_eq!(table.accept("tpl/planks", Kind::Template), Fate::Deferred);
        assert!(table.read().is_empty());
        assert!(!table.complete());
        table.resolve_templates(&[("tpl/planks", &["planks/oak", "planks/spruce"])]);
        assert_eq!(table.read(), vec!["planks/oak", "planks/spruce"]);
        assert!(table.complete());
        // `resolveTemplates` clears the raw list (`:301`).
        assert!(table.templates.is_empty());
    }

    #[test]
    fn only_a_parse_error_is_caught_and_the_rest_escapes_the_reload() {
        assert!(caught(Failure::Parse));
        assert!(!caught(Failure::Other));
        // And the reset happened before the file was read (`:48`), so the store is
        // empty rather than old when the exception escapes.
        let mut table = Table::new();
        table.accept("kept", Kind::Recipe);
        table.reset();
        assert!(table.recipes.is_empty());
    }
}
