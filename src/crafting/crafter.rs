//! The `crafterrecipes/` format: a recipe only a colony's worker can make.
//!
//! Java reference: `core/colony/crafting/CustomRecipe.java` (the model, its
//! parser, its template expander and its validity rules),
//! `core/colony/crafting/CustomRecipeManager.java` (the store, keyed by crafter),
//! `core/datalistener/CrafterRecipeListener.java` (the loader) and
//! `core/generation/CustomRecipeProvider.java` (the datagen that writes 161 of
//! the 163 files).
//!
//! **A third recipe folder, and it is not the game's.** A file under
//! `data/<ns>/crafterrecipes/<crafter>/<name>.json` is not a vanilla recipe: the
//! game never matches it against a grid, and `type: "recipe"` is not a recipe
//! serializer — it is one of five kinds the listener's switch reads
//! (`CrafterRecipeListener.java:50-63`). What it describes is *one step of a hut's
//! work*: what a baker takes, what the furnace in the middle of the room is, what
//! research unlocks it.
//!
//! ```json
//! {
//!   "type": "recipe",
//!   "crafter": "baker_smelting",
//!   "inputs": [{"item": "minecolonies:cake_batter"}],
//!   "intermediate": "minecraft:furnace",
//!   "min-building-level": 4,
//!   "result": "minecraft:cake"
//! }
//! ```
//!
//! **The crafter key is `<hut>_<module>`, and the datagen builds it that way**
//! (`CustomRecipeProvider.java:80`). The 161 files name 16 of them:
//!
//! | Crafter | Files | Crafter | Files |
//! | --- | ---: | --- | ---: |
//! | `baker_crafting` | 19 | `mechanic_crafting` | 12 |
//! | `baker_smelting` | 6 | `netherworker_custom` | 6 |
//! | `blacksmith_crafting` | 16 | `sifter_custom` | 16 |
//! | `chef_crafting` | 15 | `stonemason_crafting` | 6 |
//! | `concretemixer_custom` | 32 | `alchemist_crafting` | 1 |
//! | `crusher_custom` | 14 | `dyer_crafting` | 3 |
//! | `enchanter_custom` | 9 | `farmer_crafting` | 2 |
//! | `fletcher_crafting` | 3 | `lumberjack_custom` | 1 |
//!
//! **`result` is a bare string, and that is not a mistake.** Every other recipe
//! format in this mod writes `{"item": …}`; this one writes `"result":
//! "minecraft:bread"` and lets `count` carry the quantity (`CustomRecipe.java:280`,
//! `:341-344`), because `CustomRecipeProvider.CustomRecipeBuilder.result` flattens
//! the stack that way (`:98-108`).
//!
//! **What is not here.** The manager that keys these by crafter and by output
//! (`CustomRecipeManager.java:87-119`), the network sync (a 35-field packet
//! serializer, `CustomRecipe.java:880-980`) and the request tree a crafter
//! builds from one. This file is the model and the rules; storing them is a layer
//! above.

use super::{Refusal, array_or, flag_or, int_or, texts};
use crate::jobs::ToolKind;
use mod_api::Identifier;
use mod_api::serde_json::Value;

// `ItemId` is `super::ItemId`, the item id plus its NBT suffix
// (`src/crafting/ingredient.rs`); `src/items/` names an item with a
// `&'static str` registry path in its tables and has no owned id type, so the
// owned one is `crate::inventory::ItemName`, which `ItemId` wraps.
use super::ItemId;

/// An item and a count, as `ItemStorage`'s JSON constructor reads one
/// (`ItemStorage.java:152-168`).
#[derive(Clone, Debug, PartialEq)]
pub struct Input {
    pub item: ItemId,
    pub count: u32,
    /// `"matchType": "ignore"` — the one value Java compares with NBT ignored
    /// (`ItemStorage.java:158-159`, `NbtTagConstants.java:727-728`). The default,
    /// `"exact"`, compares the NBT too.
    pub ignore_nbt: bool,
}

impl Input {
    /// One `inputs` element.
    ///
    /// `count` defaults to one, `matchType` to `"exact"`. An element **without**
    /// an `item` key becomes an empty storage in Java (`ItemStorage.java:160-167`)
    /// and is then dropped by the caller (`CustomRecipe.java:265-269`); this port
    /// drops it here instead of carrying a hole.
    pub fn parse(value: &Value) -> Result<Option<Self>, Refusal> {
        if !super::has(value, "item") {
            return Ok(None);
        }
        let item = ItemId::parse("inputs", super::text(value, "item")?)?;
        let count = int_or(value, "count", 1)?;
        let count = u32::try_from(count)
            .ok()
            .filter(|count| *count > 0)
            .ok_or(Refusal::CountNotPositive { field: "count" })?;
        let ignore_nbt = matches!(super::text_or(value, "matchType")?, Some("ignore"));
        Ok(Some(Self {
            item,
            count,
            ignore_nbt,
        }))
    }
}

/// An item and a count, for the two output lists.
#[derive(Clone, Debug, PartialEq)]
pub struct CountedItem {
    pub item: ItemId,
    pub count: u32,
}

impl CountedItem {
    /// One `additional-output` or `alternate-output` element, which has no
    /// `matchType` (`CustomRecipe.java:298-317`).
    fn parse(value: &Value, field: &'static str) -> Result<Option<Self>, Refusal> {
        if !super::has(value, "item") {
            return Ok(None);
        }
        let item = ItemId::parse(field, super::text(value, "item")?)?;
        let count = int_or(value, "count", 1)?;
        let count = u32::try_from(count)
            .ok()
            .filter(|count| *count > 0)
            .ok_or(Refusal::CountNotPositive { field: "count" })?;
        Ok(Some(Self { item, count }))
    }
}

/// One `crafterrecipes/` file of the `recipe`, `recipe-multi-out` or
/// `recipe-multi-in` kind.
#[derive(Clone, Debug, PartialEq)]
pub struct CrafterRecipe {
    /// The file's own id, `minecolonies:baker/bread`. Java passes it in from the
    /// resource location and uses it for `remove` lookups
    /// (`CustomRecipe.java:256`, `:402-403`), so it is not in the file.
    pub id: Option<Identifier>,
    /// The crafter that can make it — `<hut>_<module>`, `"unknown"` by default
    /// (`:258-261`).
    pub crafter: String,
    pub inputs: Vec<Input>,
    /// `None` for a recipe that makes nothing directly: a loot-table recipe
    /// (`:278-285` sets an empty stack, not air).
    pub result: Option<ItemId>,
    /// The output count, read only when there is a result (`:341-344`).
    pub count: u32,
    pub loot_table: Option<Identifier>,
    /// The tool the worker must hold; [`ToolKind::None`] when the file says
    /// nothing (`:232`, `:292-296`).
    pub tool: ToolKind,
    /// Extra outputs that come out with the result (`:298-317`).
    pub additional_output: Vec<CountedItem>,
    /// Alternative results, one of which is produced (`:320-339`). Their presence
    /// is what makes the storage type `multi_output` — see
    /// [`CrafterRecipe::storage_type`].
    pub alternate_output: Vec<CountedItem>,
    /// The block the worker must stand at, `minecraft:air` for none
    /// (`:345-352`).
    pub intermediate: Identifier,
    /// Research effects that must all be unlocked (`:353-362`).
    pub research_ids: Vec<Identifier>,
    /// Research effects that invalidate it (`:363-372`).
    pub excluded_research_ids: Vec<Identifier>,
    pub min_level: i32,
    pub max_level: i32,
    /// Whether a recipe with the same output and inputs must already be in the
    /// building (`:381-384`, read at `:771-797`).
    pub must_exist: bool,
    pub show_tooltip: bool,
}

impl CrafterRecipe {
    /// A crafter recipe from its file.
    ///
    /// Nothing here is fatal in Java — `parse` never throws for a missing field
    /// except through `GsonHelper` on the two it reads unconditionally, and the
    /// fields a recipe can do without have defaults. The two that can make a file
    /// useless are a `tool` the registry does not know (`:294` leaves a `null`)
    /// and a count a stack cannot hold.
    pub fn parse(value: &Value) -> Result<Self, Refusal> {
        let crafter = super::text_or(value, "crafter")?
            .unwrap_or("unknown")
            .to_owned();

        let mut inputs = Vec::new();
        for element in array_or(value, "inputs")? {
            if let Some(parsed) = Input::parse(element)? {
                inputs.push(parsed);
            }
        }

        let result = match super::text_or(value, "result")? {
            Some(text) => Some(ItemId::parse("result", text)?),
            None => None,
        };
        let count = int_or(value, "count", 1)?;
        let count = u32::try_from(count)
            .ok()
            .filter(|count| *count > 0)
            .ok_or(Refusal::CountNotPositive { field: "count" })?;

        let loot_table = match super::text_or(value, "loot-table")? {
            Some(text) => Some(super::ident("loot-table", text)?),
            None => None,
        };

        let tool = match super::text_or(value, "tool")? {
            Some(text) => ToolKind::from_name(path_of(text)).ok_or_else(|| Refusal::UnknownTool {
                found: text.to_owned(),
            })?,
            None => ToolKind::None,
        };

        let mut additional_output = Vec::new();
        for element in array_or(value, "additional-output")? {
            if let Some(parsed) = CountedItem::parse(element, "additional-output")? {
                additional_output.push(parsed);
            }
        }
        let mut alternate_output = Vec::new();
        for element in array_or(value, "alternate-output")? {
            if let Some(parsed) = CountedItem::parse(element, "alternate-output")? {
                alternate_output.push(parsed);
            }
        }

        let intermediate = match super::text_or(value, "intermediate")? {
            Some(text) => super::ident("intermediate", text)?,
            None => super::ident("intermediate", "minecraft:air")?,
        };

        Ok(Self {
            id: None,
            crafter,
            inputs,
            result,
            count,
            loot_table,
            tool,
            additional_output,
            alternate_output,
            intermediate,
            research_ids: ids(value, "research-id")?,
            excluded_research_ids: ids(value, "not-research-id")?,
            min_level: int_or(value, "min-building-level", 0)? as i32,
            max_level: int_or(value, "max-building-level", 5)? as i32,
            must_exist: flag_or(value, "must-exist", false)?,
            show_tooltip: flag_or(value, "show-tooltip", false)?,
        })
    }

    /// The same recipe with its file id, which is how Java holds it
    /// (`CustomRecipeManager.java:87-93` keys by crafter and by id).
    pub fn with_id(mut self, id: Identifier) -> Self {
        self.id = Some(id);
        self
    }

    /// `minecolonies:multi_output` when there is at least one alternate output,
    /// `minecolonies:classic` otherwise (`RecipeStorage.java:336-338`).
    pub fn storage_type(&self) -> super::StorageType {
        super::StorageType::for_outputs(!self.alternate_output.is_empty())
    }

    /// Whether this recipe may be shown to a hut of `gate.level` now.
    ///
    /// Line for line `CustomRecipe.isValidForBuilding` (`:717-733`):
    ///
    /// ```text
    /// required  = every research-id is unlocked
    /// excluded  = every not-research-id is unlocked
    /// available = !precursor_missing && required && (excluded empty || !excluded)
    ///             && level >= min && level <= max
    /// ```
    ///
    /// **The `not-research-id` rule is an ALL, and that reads like a bug.** Java
    /// computes `allMatch` (`:725-726`) and then negates it (`:729-732`), so a recipe
    /// with two exclusions stays available while only one of them is researched —
    /// it takes *all* of them to disable it. The port keeps Java's behaviour
    /// because a data pack may already rely on it; changing it is an upstream
    /// decision, not a port one.
    pub fn available(&self, gate: &Gate<'_>) -> bool {
        if self.must_exist && !(gate.holds)(self) {
            return false;
        }
        let required = self
            .research_ids
            .iter()
            .all(|id| (gate.unlocked)(&id.to_string()));
        let excluded = self
            .excluded_research_ids
            .iter()
            .all(|id| (gate.unlocked)(&id.to_string()));
        required
            && (self.excluded_research_ids.is_empty() || !excluded)
            && gate.level >= self.min_level
            && gate.level <= self.max_level
    }

    /// Whether this is the same recipe as `other`, for the `must-exist` rule.
    ///
    /// `isPrecursorRecipeMissing` (`:771-796`) matches a stored recipe when the
    /// outputs are the same item ignoring counts and the two input **sets** are
    /// equal — `containsAll` both ways. A set of `ItemStorage` compares **items**,
    /// not amounts: `ItemStorage.equals` is
    /// `compareItemStacksIgnoreStackSize` (`ItemStorage.java:255-267`), so one
    /// wool and three wool are the same element of that set.
    pub fn same_recipe(&self, other: &CrafterRecipe) -> bool {
        let same_output = match (&self.result, &other.result) {
            (None, None) => true,
            (Some(mine), Some(theirs)) => mine.name() == theirs.name(),
            _ => false,
        };
        same_output && self.cleaned_items() == other.cleaned_items()
    }

    /// The input items with air removed and duplicates merged, sorted — the set
    /// `same_recipe` compares.
    pub fn cleaned_items(&self) -> Vec<String> {
        let mut items: Vec<String> = self
            .inputs
            .iter()
            .filter(|input| input.item.name().as_str() != "minecraft:air")
            .map(|input| input.item.to_string())
            .collect();
        items.sort();
        items.dedup();
        items
    }

    /// The inputs with air removed and equal items merged, sorted — Java's
    /// `getCleanedInput` (`RecipeStorage.java:348-359`, read back at `:423-440`).
    pub fn cleaned_inputs(&self) -> Vec<(String, u32)> {
        let mut merged: Vec<(String, u32)> = Vec::new();
        for input in &self.inputs {
            if input.item.name().as_str() == "minecraft:air" {
                continue;
            }
            let name = input.item.to_string();
            match merged.iter_mut().find(|(existing, _)| *existing == name) {
                Some((_, count)) => *count += input.count,
                None => merged.push((name, input.count)),
            }
        }
        merged.sort();
        merged
    }

    /// Whether the recipe makes nothing at all: no result, no loot table and no
    /// alternate output. Java accepts such a file and it simply never completes
    /// (`CustomRecipe.java:278-285`), unlike a template, which is rejected
    /// (`:543-551`).
    pub fn outputs_nothing(&self) -> bool {
        self.result.is_none() && self.loot_table.is_none() && self.alternate_output.is_empty()
    }
}

/// What a hut answers about itself when [`CrafterRecipe::available`] asks.
///
/// The two questions are the two Java sends to the building and the colony
/// (`CustomRecipe.java:718-733`); both are closures, so a test decides them and
/// the colony decides them in a world.
pub struct Gate<'a> {
    /// `IBuilding.getBuildingLevel()`.
    pub level: i32,
    /// `isUnlockEffectResearched`: an effect strength above zero, or a completed
    /// research with that id (`:740-764`).
    pub unlocked: &'a dyn Fn(&str) -> bool,
    /// The building already holds a recipe this one duplicates — the inverse of
    /// `isPrecursorRecipeMissing` (`:771-797`). Only asked when
    /// [`CrafterRecipe::must_exist`] is set.
    pub holds: &'a dyn Fn(&CrafterRecipe) -> bool,
}

/// One `remove` file: the id of a crafter recipe to delete.
///
/// A `remove` file holds no recipe, which is why it is its own type rather than
/// an empty [`CrafterRecipe`].
#[derive(Clone, Debug, PartialEq)]
pub struct Removal {
    pub recipe: Identifier,
}

impl Removal {
    /// `recipe-id-to-remove` (`CrafterRecipeListener.java:60-61`).
    ///
    /// Java reads it with a default of `""` and then builds a
    /// `ResourceLocation` from it, which **throws** — and the listener catches
    /// `JsonParseException` only (`:65-68`), so a `remove` file without an id
    /// aborts the whole crafter-recipe load rather than being skipped. The port
    /// refuses the file by name instead.
    pub fn parse(value: &Value) -> Result<Self, Refusal> {
        Ok(Self {
            recipe: super::ident(
                "recipe-id-to-remove",
                super::text(value, "recipe-id-to-remove")?,
            )?,
        })
    }
}

/// A `recipe-template` file: one file, one recipe per member of a tag.
///
/// The two shipped templates (`lumberjack/strip_logs.json`,
/// `lumberjack/strip_stems.json`) turn every log into three or five stripped
/// variants, which is why they are written by hand rather than by the datagen:
/// the id of each produced recipe depends on the item.
#[derive(Clone, Debug, PartialEq)]
pub struct Template {
    /// The tag whose members are expanded, `minecraft:logs` for both shipped
    /// files (`:407`).
    pub tag: String,
    /// The recipe to expand, kept as JSON because every member rewrites it
    /// (`:408`).
    pub base: Value,
    /// Which members to keep (`:410-421`).
    pub filter: Filter,
}

impl Template {
    /// A template from its file. Both `tag` and `recipe` are required: Java's
    /// `GsonHelper.getAsString` throws without them (`:407-408`).
    pub fn parse(value: &Value) -> Result<Self, Refusal> {
        let tag = super::text(value, "tag")?
            .trim_start_matches('#')
            .to_owned();
        let base = value
            .get("recipe")
            .ok_or(Refusal::MissingField { field: "recipe" })?
            .clone();
        if !base.is_object() {
            return Err(Refusal::WrongType { field: "recipe" });
        }
        Ok(Self {
            tag,
            base,
            filter: Filter::parse(value.get("filter"))?,
        })
    }

    /// Expand the template for `item`.
    ///
    /// `known` is Java's `ForgeRegistries.ITEMS.containsKey` (`:47` of
    /// `ItemStackUtils.parseIdTemplate`): whether an id names an item at all.
    /// Java asks the item registry; a caller here asks
    /// `Host::item_key` (`crates/horizon-mod-vocab/src/host/items.rs:22`).
    ///
    /// The reject/ignore difference is Java's, entry for entry
    /// (`:470-558`): a bad **input**, **result** or **additional-output** rejects
    /// the whole member, a bad **alternate-output** only drops that alternative,
    /// and a member left with no output at all is rejected
    /// ([`Refusal::TemplateNoOutput`], `:543-551`).
    pub fn expand_one(
        &self,
        item: &str,
        known: &dyn Fn(&str) -> bool,
    ) -> Result<CrafterRecipe, Refusal> {
        let base = Identifier::parse_namespaced(item)
            .ok_or(Refusal::UnparsableItem { field: "tag" })?;
        let mut recipe = self.base.clone();
        let object = recipe.as_object_mut().ok_or(Refusal::NotAnObject)?;

        // Inputs: every one must resolve, or the member is rejected.
        if let Some(inputs) = object.get_mut("inputs").and_then(Value::as_array_mut) {
            for input in inputs.iter_mut() {
                substitute_item(input, &base, "inputs", known)?;
            }
        }
        // The result must resolve too.
        if let Some(result) = object.get_mut("result") {
            substitute_value(result, &base, "result", known)?;
        }
        // Additional outputs must; alternate outputs may not.
        if let Some(additional) = object
            .get_mut("additional-output")
            .and_then(Value::as_array_mut)
        {
            for element in additional.iter_mut() {
                substitute_item(element, &base, "additional-output", known)?;
            }
        }
        if let Some(alternate) = object
            .get_mut("alternate-output")
            .and_then(Value::as_array_mut)
        {
            let mut kept = Vec::with_capacity(alternate.len());
            for mut element in alternate.drain(..) {
                if substitute_item(&mut element, &base, "alternate-output", known).is_ok() {
                    kept.push(element);
                }
            }
            *alternate = kept;
        }

        let expanded = CrafterRecipe::parse(&recipe)?;
        if expanded.outputs_nothing() {
            return Err(Refusal::TemplateNoOutput);
        }
        Ok(expanded)
    }

    /// Every recipe this template makes from `items`, each with the id Java
    /// builds: `<template ns>:<template path>/<item ns>/<item path>`
    /// (`:430`).
    ///
    /// A member the filter drops, or one whose expansion refuses, is skipped —
    /// Java logs it and moves on.
    pub fn expand(
        &self,
        template: &Identifier,
        items: &[String],
        known: &dyn Fn(&str) -> bool,
    ) -> Vec<(Identifier, CrafterRecipe)> {
        let mut out = Vec::new();
        for item in items {
            if !self.filter.accepts(item) {
                continue;
            }
            let Ok(recipe) = self.expand_one(item, known) else {
                continue;
            };
            let Some(item) = Identifier::parse_namespaced(item) else {
                continue;
            };
            let path = format!(
                "{}/{}/{}",
                template.path(),
                item.namespace(),
                item.path()
            );
            let Ok(id) = Identifier::from_parts(template.namespace(), &path) else {
                continue;
            };
            // L identifiant part dans le tuple ET dans la recette : on clone pour le tuple, on deplace dans
            // la recette (l ordre inverse deplacerait `id` avant le clone).
            out.push((id.clone(), recipe.with_id(id)));
        }
        out
    }
}

/// Which tag members a template keeps (`CustomRecipe.java:410-421`, `:441-460`).
///
/// Java's filter is **substring** matching on the whole id, not a glob: the two
/// shipped files use `"_log"` and `"stripped_"` that way, and the stem template
/// lists `"_stem"`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    /// Any of these substrings keeps the id; empty keeps everything, which is
    /// Java's `defaultResult = true` for an absent `include`.
    pub include: Vec<String>,
    /// Any of these drops it; empty drops nothing (`defaultResult = false`).
    pub exclude: Vec<String>,
}

impl Filter {
    /// `filter` may be absent, one string, an array, or an object with those two
    /// keys (`:410-421`).
    pub fn parse(value: Option<&Value>) -> Result<Self, Refusal> {
        let Some(value) = value else {
            return Ok(Self::default());
        };
        if let Some(object) = value.as_object() {
            return Ok(Self {
                include: substrings(object.get("include"))?,
                exclude: substrings(object.get("exclude"))?,
            });
        }
        Ok(Self {
            include: substrings(Some(value))?,
            exclude: Vec::new(),
        })
    }

    /// Whether an id survives the filter.
    pub fn accepts(&self, id: &str) -> bool {
        let included = self.include.is_empty() || self.include.iter().any(|needle| id.contains(needle));
        let excluded = self.exclude.iter().any(|needle| id.contains(needle));
        included && !excluded
    }
}

fn substrings(value: Option<&Value>) -> Result<Vec<String>, Refusal> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    if let Some(one) = value.as_str() {
        return Ok(vec![one.to_owned()]);
    }
    let items = value
        .as_array()
        .ok_or(Refusal::WrongType { field: "filter" })?;
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        out.push(
            item.as_str()
                .ok_or(Refusal::WrongType { field: "filter" })?
                .to_owned(),
        );
    }
    Ok(out)
}

/// Rewrite one `{"item": …}` or one bare string through the template.
fn substitute_item(
    object: &mut Value,
    base: &Identifier,
    field: &'static str,
    known: &dyn Fn(&str) -> bool,
) -> Result<(), Refusal> {
    if let Some(id) = object.get_mut("item") {
        return substitute_value(id, base, field, known);
    }
    substitute_value(object, base, field, known)
}

fn substitute_value(
    value: &mut Value,
    base: &Identifier,
    field: &'static str,
    known: &dyn Fn(&str) -> bool,
) -> Result<(), Refusal> {
    let text = value.as_str().ok_or(Refusal::WrongType { field })?;
    let expanded = substitute(text, base);
    if !known(&expanded) {
        return Err(Refusal::UnparsableItem { field });
    }
    *value = Value::String(expanded);
    Ok(())
}

/// `ItemStackUtils.parseIdTemplate` (`:845-868`), without the registry check.
///
/// Three placeholders, and only three:
///
/// | Written | Becomes |
/// | --- | --- |
/// | `[NS]` | the base item's namespace |
/// | `[PATH]` | its path |
/// | `[PATH:from=to]` | its path with `from` replaced by `to` — all occurrences, like `String.replace` |
///
/// An NBT suffix survives: Java cuts the id at the first `{`, substitutes in what
/// is left, and glues the suffix back (`:854-865`). A `[PATH:…]` whose `from` is
/// not in the path yields the path unchanged, which is why
/// `[PATH:_stem=_hyphae]` and `[PATH:_stem=_wood]` on a stem produce one real
/// rename and one duplicate.
pub fn substitute(value: &str, base: &Identifier) -> String {
    let (pattern, suffix) = match value.find('{') {
        Some(at) => (&value[..at], &value[at..]),
        None => (value, ""),
    };
    let mut out = String::with_capacity(pattern.len() + suffix.len() + base.path().len());
    let mut rest = pattern;
    loop {
        match rest.find('[') {
            None => {
                out.push_str(rest);
                break;
            }
            Some(at) => {
                out.push_str(&rest[..at]);
                let after = &rest[at..];
                let Some(end) = after.find(']') else {
                    // An unclosed placeholder is copied through: Java's regex
                    // does not match it either.
                    out.push_str(after);
                    break;
                };
                let placeholder = &after[..=end];
                out.push_str(&resolve(placeholder, base));
                rest = &after[end + 1..];
            }
        }
    }
    out.push_str(suffix);
    out
}

fn resolve(placeholder: &str, base: &Identifier) -> String {
    let inner = &placeholder[1..placeholder.len() - 1];
    if inner == "NS" {
        return base.namespace().to_owned();
    }
    let Some(rest) = inner.strip_prefix("PATH") else {
        return placeholder.to_owned();
    };
    let Some(pair) = rest.strip_prefix(':') else {
        return base.path().to_owned();
    };
    match pair.split_once('=') {
        Some((from, to)) => base.path().replace(from, to),
        None => base.path().to_owned(),
    }
}

fn path_of(id: &str) -> &str {
    id.split_once(':').map(|(_, path)| path).unwrap_or(id)
}

fn ids(value: &Value, field: &'static str) -> Result<Vec<Identifier>, Refusal> {
    let mut out = Vec::new();
    for text in texts(value, field)? {
        out.push(super::ident(field, &text)?);
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

    fn ident(text: &str) -> Identifier {
        Identifier::parse_namespaced(text).expect("a literal id parses")
    }

    // Four shipped files, verbatim.
    const BREAD: &str = r#"{
        "type": "recipe",
        "crafter": "baker_smelting",
        "inputs": [{"item": "minecolonies:bread_dough"}],
        "intermediate": "minecraft:furnace",
        "result": "minecraft:bread"
    }"#;

    const CAKE: &str = r#"{
        "type": "recipe",
        "crafter": "baker_smelting",
        "inputs": [{"item": "minecolonies:cake_batter"}],
        "intermediate": "minecraft:furnace",
        "min-building-level": 4,
        "result": "minecraft:cake"
    }"#;

    const PADDING: &str = r#"{
        "type": "recipe",
        "crafter": "mechanic_crafting",
        "inputs": [{"item": "minecraft:white_wool", "count": 2}],
        "additional-output": [{"item": "minecraft:string", "count": 3}],
        "alternate-output": [{"item": "minecraft:white_carpet"}],
        "intermediate": "minecraft:air",
        "result": "minecraft:white_bed"
    }"#;

    const STRIP_LOGS: &str = r#"{
      "type": "recipe-template",
      "tag": "minecraft:logs",
      "filter": {"include": ["_log"], "exclude": ["stripped_"]},
      "recipe": {
        "crafter": "lumberjack_custom",
        "inputs": [{"item": "[NS]:[PATH]"}],
        "alternate-output": [
          {"item": "[NS]:stripped_[PATH]"},
          {"item": "[NS]:[PATH:_log=_wood]"},
          {"item": "[NS]:stripped_[PATH:_log=_wood]"}
        ],
        "intermediate": "minecraft:air",
        "tool": "minecolonies:axe"
      }
    }"#;

    #[test]
    fn a_shipped_recipe_parses_with_every_default_it_omits() {
        let recipe = CrafterRecipe::parse(&json(BREAD)).expect("bread must parse");
        assert_eq!(recipe.crafter, "baker_smelting");
        assert_eq!(recipe.inputs.len(), 1);
        assert_eq!(recipe.inputs[0].count, 1);
        assert!(!recipe.inputs[0].ignore_nbt);
        assert_eq!(recipe.result.as_ref().map(|id| id.name().as_str()), Some("minecraft:bread"));
        assert_eq!(recipe.count, 1);
        assert_eq!(recipe.min_level, 0);
        assert_eq!(recipe.max_level, 5);
        assert_eq!(recipe.tool, ToolKind::None);
        assert_eq!(recipe.intermediate.to_string(), "minecraft:furnace");
        assert!(recipe.research_ids.is_empty());
        assert!(recipe.excluded_research_ids.is_empty());
        assert!(!recipe.must_exist);
        assert!(!recipe.show_tooltip);
        assert!(!recipe.outputs_nothing());
    }

    #[test]
    fn the_building_level_gate_is_read_from_the_file() {
        let recipe = CrafterRecipe::parse(&json(CAKE)).expect("cake must parse");
        assert_eq!(recipe.min_level, 4);
        assert_eq!(recipe.max_level, 5);
        let unlocked = |_: &str| false;
        let holds = |_: &CrafterRecipe| false;
        assert!(!recipe.available(&Gate {
            level: 3,
            unlocked: &unlocked,
            holds: &holds
        }));
        assert!(recipe.available(&Gate {
            level: 4,
            unlocked: &unlocked,
            holds: &holds
        }));
        assert!(recipe.available(&Gate {
            level: 5,
            unlocked: &unlocked,
            holds: &holds
        }));

        let bread = CrafterRecipe::parse(&json(BREAD)).expect("bread must parse");
        assert!(bread.available(&Gate {
            level: 1,
            unlocked: &unlocked,
            holds: &holds
        }));
    }

    #[test]
    fn both_output_lists_parse_and_the_alternate_one_picks_the_storage_type() {
        let recipe = CrafterRecipe::parse(&json(PADDING)).expect("parses");
        assert_eq!(recipe.inputs[0].count, 2);
        assert_eq!(recipe.additional_output.len(), 1);
        assert_eq!(recipe.additional_output[0].count, 3);
        assert_eq!(recipe.alternate_output.len(), 1);
        assert_eq!(
            recipe.storage_type(),
            super::super::StorageType::MultiOutput,
            "RecipeStorage.java:336-338"
        );
        let bread = CrafterRecipe::parse(&json(BREAD)).expect("parses");
        assert_eq!(bread.storage_type(), super::super::StorageType::Classic);
    }

    #[test]
    fn a_crafter_key_defaults_to_unknown() {
        let recipe = CrafterRecipe::parse(&json(
            r#"{"inputs": [], "intermediate": "minecraft:air"}"#,
        ))
        .expect("parses");
        assert_eq!(recipe.crafter, "unknown");
        assert!(recipe.result.is_none());
        assert!(recipe.outputs_nothing(), "no result, no loot table, no alternate");
    }

    #[test]
    fn a_result_count_is_count_and_never_part_of_the_id() {
        let recipe = CrafterRecipe::parse(&json(
            r#"{"inputs": [], "result": "minecolonies:compost", "count": 6}"#,
        ))
        .expect("parses");
        assert_eq!(recipe.count, 6);
        assert_eq!(recipe.result.map(|id| id.name().as_str().to_owned()), Some("minecolonies:compost".to_owned()));
    }

    #[test]
    fn a_bad_tool_id_is_named_not_left_null() {
        assert_eq!(
            CrafterRecipe::parse(&json(
                r#"{"inputs": [], "result": "minecraft:bread", "tool": "minecolonies:spoon"}"#
            )),
            Err(Refusal::UnknownTool {
                found: "minecolonies:spoon".into()
            })
        );
        let axe = CrafterRecipe::parse(&json(
            r#"{"inputs": [], "result": "minecraft:bread", "tool": "minecolonies:axe"}"#,
        ))
        .expect("parses");
        assert_eq!(axe.tool, ToolKind::Axe);
    }

    #[test]
    fn an_input_without_an_item_is_dropped_not_carried() {
        let recipe = CrafterRecipe::parse(&json(
            r#"{"inputs": [{"count": 4}, {"item": "minecraft:white_wool"}], "result": "minecraft:bread"}"#,
        ))
        .expect("parses");
        assert_eq!(recipe.inputs.len(), 1, "CustomRecipe.java:265-269 drops it");
        assert_eq!(recipe.inputs[0].item.name().as_str(), "minecraft:white_wool");
    }

    #[test]
    fn a_match_type_of_ignore_is_carried_on_the_input() {
        let recipe = CrafterRecipe::parse(&json(
            r#"{"inputs": [{"item": "minecraft:potion", "matchType": "ignore"}], "result": "minecraft:bread"}"#,
        ))
        .expect("parses");
        assert!(recipe.inputs[0].ignore_nbt);
        let exact = CrafterRecipe::parse(&json(
            r#"{"inputs": [{"item": "minecraft:potion", "matchType": "exact"}], "result": "minecraft:bread"}"#,
        ))
        .expect("parses");
        assert!(!exact.inputs[0].ignore_nbt);
    }

    #[test]
    fn a_research_id_may_be_one_string_or_an_array() {
        let one = CrafterRecipe::parse(&json(
            r#"{"inputs": [], "result": "minecolonies:magicpotion",
                "research-id": "minecolonies:effects/consumepotions"}"#,
        ))
        .expect("parses");
        assert_eq!(one.research_ids, vec![ident("minecolonies:effects/consumepotions")]);

        let many = CrafterRecipe::parse(&json(
            r#"{"inputs": [], "result": "minecraft:bread",
                "research-id": ["a:b", "c:d"], "not-research-id": ["e:f"]}"#,
        ))
        .expect("parses");
        assert_eq!(many.research_ids, vec![ident("a:b"), ident("c:d")]);
        assert_eq!(many.excluded_research_ids, vec![ident("e:f")]);
    }

    #[test]
    fn a_research_gate_needs_every_required_id() {
        let recipe = CrafterRecipe::parse(&json(
            r#"{"inputs": [], "result": "minecraft:bread", "research-id": ["a:b", "c:d"]}"#,
        ))
        .expect("parses");
        let holds = |_: &CrafterRecipe| false;
        // Nothing researched.
        let none = |_: &str| false;
        assert!(!recipe.available(&Gate {
            level: 1,
            unlocked: &none,
            holds: &holds
        }));
        // One of the two: still not available, `allMatch`.
        let half = |id: &str| id == "a:b";
        assert!(!recipe.available(&Gate {
            level: 1,
            unlocked: &half,
            holds: &holds
        }));
        let both = |_: &str| true;
        assert!(recipe.available(&Gate {
            level: 1,
            unlocked: &both,
            holds: &holds
        }));
    }

    #[test]
    fn one_exclusion_is_enough_to_disable_and_two_are_not() {
        let one = CrafterRecipe::parse(&json(
            r#"{"inputs": [], "result": "minecraft:bread", "not-research-id": "a:b"}"#,
        ))
        .expect("parses");
        let holds = |_: &CrafterRecipe| false;
        let yes = |_: &str| true;
        let no = |_: &str| false;
        assert!(!one.available(&Gate {
            level: 1,
            unlocked: &yes,
            holds: &holds
        }));
        assert!(one.available(&Gate {
            level: 1,
            unlocked: &no,
            holds: &holds
        }));

        let two = CrafterRecipe::parse(&json(
            r#"{"inputs": [], "result": "minecraft:bread", "not-research-id": ["a:b", "c:d"]}"#,
        ))
        .expect("parses");
        let one_of_two = |id: &str| id == "a:b";
        assert!(
            two.available(&Gate {
                level: 1,
                unlocked: &one_of_two,
                holds: &holds
            }),
            "Java's allMatch: it takes both exclusions to disable a recipe"
        );
    }

    #[test]
    fn must_exist_needs_the_precursor_recipe_in_the_building() {
        let recipe = CrafterRecipe::parse(&json(
            r#"{"inputs": [{"item": "minecraft:white_wool"}], "result": "minecraft:bread",
                "must-exist": true}"#,
        ))
        .expect("parses");
        let unlocked = |_: &str| false;
        let absent = |_: &CrafterRecipe| false;
        assert!(!recipe.available(&Gate {
            level: 1,
            unlocked: &unlocked,
            holds: &absent
        }));
        let present = |_: &CrafterRecipe| true;
        assert!(recipe.available(&Gate {
            level: 1,
            unlocked: &unlocked,
            holds: &present
        }));
    }

    #[test]
    fn the_same_recipe_ignores_counts_but_not_inputs() {
        let one = CrafterRecipe::parse(&json(
            r#"{"inputs": [{"item": "minecraft:white_wool", "count": 1}], "result": "minecraft:bread", "count": 1}"#,
        ))
        .expect("parses");
        let two = CrafterRecipe::parse(&json(
            r#"{"inputs": [{"item": "minecraft:white_wool", "count": 3}], "result": "minecraft:bread", "count": 9}"#,
        ))
        .expect("parses");
        assert!(one.same_recipe(&two), "compareItemStacksIgnoreStackSize");

        let other = CrafterRecipe::parse(&json(
            r#"{"inputs": [{"item": "minecraft:string"}], "result": "minecraft:bread"}"#,
        ))
        .expect("parses");
        assert!(!one.same_recipe(&other));

        let wool_and_string = CrafterRecipe::parse(&json(
            r#"{"inputs": [{"item": "minecraft:white_wool"}, {"item": "minecraft:string"}],
                "result": "minecraft:bread"}"#,
        ))
        .expect("parses");
        assert!(!one.same_recipe(&wool_and_string), "containsAll both ways");
    }

    #[test]
    fn the_cleaned_inputs_merge_the_same_item_and_drop_air() {
        let recipe = CrafterRecipe::parse(&json(
            r#"{"inputs": [
                {"item": "minecraft:white_wool", "count": 2},
                {"item": "minecraft:white_wool", "count": 3},
                {"item": "minecraft:air"}
            ], "result": "minecraft:bread"}"#,
        ))
        .expect("parses");
        assert_eq!(recipe.cleaned_inputs(), vec![("minecraft:white_wool".to_owned(), 5)]);
    }

    #[test]
    fn the_file_id_is_attached_from_outside() {
        let recipe = CrafterRecipe::parse(&json(BREAD))
            .expect("parses")
            .with_id(ident("minecolonies:baker/bread"));
        assert_eq!(
            recipe.id.map(|id| id.to_string()),
            Some("minecolonies:baker/bread".to_owned())
        );
    }

    #[test]
    fn a_remove_file_names_the_recipe_it_deletes() {
        let removal = Removal::parse(&json(
            r#"{"type": "remove", "recipe-id-to-remove": "minecolonies:baker/bread"}"#,
        ))
        .expect("parses");
        assert_eq!(removal.recipe.to_string(), "minecolonies:baker/bread");
        // Java defaults to "" and then throws building an id from it, which
        // escapes its JsonParseException catch.
        assert_eq!(
            Removal::parse(&json(r#"{"type": "remove"}"#)),
            Err(Refusal::MissingField {
                field: "recipe-id-to-remove"
            })
        );
    }

    #[test]
    fn the_shipped_strip_logs_template_parses() {
        let template = Template::parse(&json(STRIP_LOGS)).expect("parses");
        assert_eq!(template.tag, "minecraft:logs");
        assert_eq!(template.filter.include, vec!["_log".to_owned()]);
        assert_eq!(template.filter.exclude, vec!["stripped_".to_owned()]);
        assert!(template.filter.accepts("minecraft:oak_log"));
        assert!(!template.filter.accepts("minecraft:stripped_oak_log"));
        assert!(!template.filter.accepts("minecraft:oak_planks"));
    }

    #[test]
    fn the_three_placeholders_substitute_the_way_java_does() {
        let base = ident("minecraft:oak_log");
        assert_eq!(substitute("[NS]:[PATH]", &base), "minecraft:oak_log");
        assert_eq!(
            substitute("[NS]:stripped_[PATH]", &base),
            "minecraft:stripped_oak_log"
        );
        assert_eq!(
            substitute("[NS]:[PATH:_log=_wood]", &base),
            "minecraft:oak_wood"
        );
        assert_eq!(
            substitute("[NS]:stripped_[PATH:_log=_wood]", &base),
            "minecraft:stripped_oak_wood"
        );
    }

    #[test]
    fn a_replacement_whose_pattern_is_absent_leaves_the_path_alone() {
        // `String.replace` cannot be "absent" when the pattern is a substring of
        // every wood path: `crimson_stem` with `_stem` → `_wood` really is
        // `crimson_wood` (`ItemStackUtils.java:42`,
        // `baseItemId.getPath().replace(m.group(1), m.group(2))`).
        let stem = ident("minecraft:crimson_stem");
        assert_eq!(
            substitute("[NS]:[PATH:_stem=_hyphae]", &stem),
            "minecraft:crimson_hyphae"
        );
        assert_eq!(
            substitute("[NS]:[PATH:_stem=_wood]", &stem),
            "minecraft:crimson_wood"
        );
        // What is absent stays absent: a log has no `_stem` in its path, so both
        // renames of the stem template leave it alone. That is why the two
        // hand-written templates are two files and not one.
        let log = ident("minecraft:oak_log");
        assert_eq!(
            substitute("[NS]:[PATH:_stem=_hyphae]", &log),
            "minecraft:oak_log"
        );
    }

    #[test]
    fn an_nbt_suffix_survives_the_substitution() {
        let base = ident("minecraft:oak_log");
        assert_eq!(
            substitute("[NS]:stripped_[PATH]{Damage:0}", &base),
            "minecraft:stripped_oak_log{Damage:0}"
        );
    }

    #[test]
    fn an_unclosed_placeholder_is_copied_through_rather_than_guessed() {
        let base = ident("minecraft:oak_log");
        // Java's regex `\[PATH(?::([^=]*)=([^]]*))?]` does not match this, so the
        // text survives untouched and the registry check rejects it.
        assert_eq!(substitute("[NS]:[PATH", &base), "minecraft:[PATH");
    }

    #[test]
    fn the_template_expands_one_member_into_three_alternates() {
        let template = Template::parse(&json(STRIP_LOGS)).expect("parses");
        let known = |id: &str| id.starts_with("minecraft:");
        let recipe = template
            .expand_one("minecraft:oak_log", &known)
            .expect("oak_log expands");
        assert_eq!(recipe.crafter, "lumberjack_custom");
        assert_eq!(recipe.tool, ToolKind::Axe);
        assert_eq!(
            recipe.inputs[0].item.name().as_str(),
            "minecraft:oak_log",
            "the input is the member itself"
        );
        let alternates: Vec<String> = recipe
            .alternate_output
            .iter()
            .map(|output| output.item.name().as_str().to_owned())
            .collect();
        assert_eq!(
            alternates,
            vec![
                "minecraft:stripped_oak_log".to_owned(),
                "minecraft:oak_wood".to_owned(),
                "minecraft:stripped_oak_wood".to_owned()
            ]
        );
        assert!(
            recipe.result.is_none(),
            "the template has no result; its outputs are the alternates"
        );
    }

    #[test]
    fn a_member_the_game_does_not_know_is_ignored_not_kept() {
        let template = Template::parse(&json(STRIP_LOGS)).expect("parses");

        // The input resolves, so the member gets as far as the alternates; one of
        // the three names a real item, the other two do not, and Java drops those
        // two without rejecting the member (CustomRecipe.java:531-541).
        let one_alternate = |id: &str| matches!(id, "minecraft:oak_log" | "minecraft:stripped_oak_log");
        let recipe = template
            .expand_one("minecraft:oak_log", &one_alternate)
            .expect("one alternate survives");
        assert_eq!(recipe.alternate_output.len(), 1);
        assert_eq!(
            recipe.alternate_output[0].item.name().as_str(),
            "minecraft:stripped_oak_log"
        );

        // With no alternate left, the member makes nothing and is rejected
        // (:543-551, "rejecting, no outputs").
        let only_input = |id: &str| id == "minecraft:oak_log";
        assert_eq!(
            template.expand_one("minecraft:oak_log", &only_input),
            Err(Refusal::TemplateNoOutput)
        );

        // A bad **input** rejects earlier, with its own field name (:470-478).
        let nothing_known = |_: &str| false;
        assert_eq!(
            template.expand_one("minecraft:oak_log", &nothing_known),
            Err(Refusal::UnparsableItem { field: "inputs" })
        );
    }

    #[test]
    fn expand_builds_the_id_java_builds() {
        let template = Template::parse(&json(STRIP_LOGS)).expect("parses");
        let known = |id: &str| id.starts_with("minecraft:");
        let recipes = template.expand(
            &ident("minecolonies:lumberjack/strip_logs"),
            &[
                "minecraft:oak_log".to_owned(),
                "minecraft:stripped_oak_log".to_owned(),
                "minecraft:oak_planks".to_owned(),
            ],
            &known,
        );
        assert_eq!(recipes.len(), 1, "only oak_log passes the filter");
        assert_eq!(
            recipes[0].0.to_string(),
            "minecolonies:lumberjack/strip_logs/minecraft/oak_log"
        );
        assert_eq!(
            recipes[0].1.id.as_ref().map(ToString::to_string),
            Some("minecolonies:lumberjack/strip_logs/minecraft/oak_log".to_owned())
        );
    }

    #[test]
    fn a_filter_is_a_substring_test_and_an_absent_one_keeps_everything() {
        let absent = Filter::parse(None).expect("absent is fine");
        assert!(absent.accepts("minecolonies:anything"));
        assert!(Filter::default().accepts("minecraft:stone"));

        let bare = Filter::parse(Some(&json(r#""_stem""#))).expect("parses");
        assert!(bare.accepts("minecraft:crimson_stem"));
        assert!(!bare.accepts("minecraft:oak_log"));

        let array = Filter::parse(Some(&json(r#"["_log", "_stem"]"#))).expect("parses");
        assert!(array.accepts("minecraft:oak_log"));
        assert!(array.accepts("minecraft:warped_stem"));

        let object = Filter::parse(Some(&json(
            r#"{"include": ["_stem"], "exclude": ["stripped_"]}"#,
        )))
        .expect("parses");
        assert!(object.accepts("minecraft:crimson_stem"));
        assert!(!object.accepts("minecraft:stripped_crimson_stem"));
    }
}
