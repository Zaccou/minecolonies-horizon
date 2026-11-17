//! The store: which recipes a crafter can make, which recipes produce an item, and
//! when a `remove` file takes effect.
//!
//! Java reference: `core/colony/crafting/CustomRecipeManager.java` (445 lines) and
//! the two types it holds — `CustomRecipe` (`core/colony/crafting/CustomRecipe.java`)
//! and `LootTableAnalyzer.LootDrop` (`:468-534`).
//!
//! **This is the layer `src/crafting/` names as missing, and it does not re-port
//! the model.** `CrafterRecipe` already holds one `crafterrecipes/` file
//! (`src/crafting/crafter.rs:128-360`); this file holds *many* of them, in the two
//! indexes Java keeps (`:46`, `:51`), plus the three side tables the same class
//! owns: the removals waiting to be applied (`:57`), the loot drops per table
//! (`:62`) and the templates waiting for the item tags (`:67`).
//!
//! | Java | Here |
//! | --- | --- |
//! | `addRecipe` (`:87-109`) | [`Store::add`] |
//! | `removeRecipe` (`:115-121`) — queues | [`Store::remove`] |
//! | `removeRecipes` (`:249-275`) — applies | [`Store::flush`], called by [`Store::recipes`] |
//! | `getRecipes` (`:152-157`) | [`Store::recipes`] |
//! | `getAllRecipes` (`:162-165`) | [`Store::all`] |
//! | `getRecipeByOutput` ×3 (`:172-233`) | [`Store::recipes_for_item`], [`Store::recipes_for_stack`], [`Store::recipes_for_storage`] |
//! | `reset` (`:138-145`) | [`Store::reset`] |
//! | `addRecipeTemplate` / `resolveTemplates` (`:128-132`, `:282-300`) | [`Store::add_template`], [`Store::resolve_templates`] |
//! | `buildLootData` (`:306-366`), `getLootDrops` (`:242-247`) | [`Store::loot_ids`], [`Store::replace_loot`], [`Store::drops`] |
//! | `handleCustomRecipeManagerMessage` (`:412-444`) | [`Store::ingest`] |
//!
//! **Four behaviours of this class are surprising, and all four are kept.**
//!
//! 1. **A removal is lazy, and only one accessor applies it.** `removeRecipe`
//!    appends to a list; `removeRecipes` runs from `getRecipes` and from nowhere
//!    else (`:154`). So `getAllRecipes` and the three output lookups still see a
//!    removed recipe until a caller has asked for a crafter's set
//!    ([`Store::pending`], and the test that holds it).
//! 2. **A removal reaches one crafter map.** `removeRecipes` filters
//!    `recipeMap.values()` and stops at `findFirst` (`:255-258`), so an id present
//!    under two crafters is removed from one. Java's `HashMap` makes "which one"
//!    unspecified; here the maps are a `BTreeMap` and it is the alphabetically
//!    first crafter. Pinned, not fixed.
//! 3. **Re-adding an id duplicates the output index.** `addRecipe` overwrites the
//!    crafter's entry (`:94`) and *then appends* to the output list without
//!    consulting it (`:100`), so a recipe registered twice appears once per
//!    crafter and twice per output — and a later removal takes one copy out, as
//!    `List.remove(Object)` removes the first equal element. [`Store::add`] returns
//!    the entry it replaced so a caller can see it happen.
//! 4. **An absent `result` is indexed under air.** `CustomRecipe.parse` sets
//!    `result = ItemStack.EMPTY` when the file has no `result`
//!    (`CustomRecipe.java:278-283`), and the manager keys the output map by
//!    `getPrimaryOutput().getItem()` (`:96-100`) — which for `EMPTY` is
//!    `minecraft:air`. Those recipes are in the index and match no stack query.
//!
//! **The order of writes is the point of half the tests.** Java refuses nothing:
//! it returns `null` and logs, and what makes that harmless is that the refusal
//! comes *before* the write. `add` keys the crafter map only after it knows the id
//! (`:89-94`), `resolveTemplates` adds each recipe as it parses it and clears the
//! template map last (`:284-299`), `buildLootData` clears the loot map before it
//! fills it (`:360-365`), and `handleCustomRecipeManagerMessage` resets **first**
//! and posts `CustomRecipesReloadedEvent` **last** (`:414`, `:436-443`). Every one
//! of those is a test below.
//!
//! **What is not here**: `sendCustomRecipeManagerPackets`/`serializeNetworkData`
//! (`:372-406`), because the message has no port layout yet
//! (`src/channel/registry.rs:313` declares id 129 with none) and `CustomRecipe`'s
//! 35-field serializer (`CustomRecipe.java:886-969`) is `src/crafting/`'s gap, not this
//! one — `MANQUANT.md` §4.
//! `buildLootData`'s *analysis* (`LootTableAnalyzer.toDrops`, 468 lines of loot-table
//! walking) is §5: this file keeps the *set of tables* it is built from and the
//! shape of one drop, and nothing of the JSON walking.
//!
//! **Two maps, one copy per recipe.** Java holds the same object in both indexes;
//! a `BTreeMap` keyed by id cannot hold a value of the key's own type without a
//! clone, so [`Store::add`] clones once into the output list. That is the only
//! place this port costs memory Java does not, and it is what makes `remove`'s
//! "first **equal** element" rule expressible at all.

use std::collections::BTreeMap;

use crate::crafting::{CrafterRecipe, ItemId, Removal};
use crate::inventory::ItemName;
use mod_api::serde_json::Value;
use mod_api::Identifier;

/// One entry of `recipeOutputMap`, Java's `LootTableAnalyzer.LootDrop`
/// (`:468-534`).
///
/// The four fields are the constructor's (`:475-481`), and the `stacks` are the
/// alternatives of one drop. **Java declares no `equals`** — `hashCode` is
/// `Objects.hash(probability, quality, conditional)` (`:501-505`) while `equals`
/// stays `Object`'s identity — so a `Set<LootDrop>` never merges two drops with the
/// same numbers. Nothing in this module puts drops in a set, and the derived
/// `PartialEq` here is for tests and round trips only; it is *not* Java's identity.
#[derive(Clone, Debug, PartialEq)]
pub struct LootDrop {
    /// `getItemStacks()`: the alternatives, each an item and a count (`:470`).
    pub stacks: Vec<(ItemId, u32)>,
    /// `getProbability()` (`:494`).
    pub probability: f32,
    /// `getQuality()`: non-zero means the citizen's skills move it (`:496`).
    pub quality: f32,
    /// `getConditional()`: special conditions apply (`:498`).
    pub conditional: bool,
}

impl LootDrop {
    /// A drop with no alternative list: the shape a test needs, and what a caller
    /// that only knows the numbers can build.
    pub fn new(probability: f32, quality: f32, conditional: bool) -> Self {
        Self {
            stacks: Vec::new(),
            probability,
            quality,
            conditional,
        }
    }
}

/// What one template lost when it was expanded (`:293-297`): the id, and whatever
/// the parser or the store said.
///
/// Java logs and moves on, so a bad template costs its own recipes and nothing
/// else. The cause is a `String` because the parser is outside this module —
/// `CustomRecipe.parseTemplate` is not ported (`MANQUANT.md` §3) and its error type
/// is `src/crafting`'s.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TemplateFailure {
    pub id: Identifier,
    pub cause: String,
}

/// What an ingest applied, so a caller can log it and know the event is due.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ingested {
    /// Recipes stored, which is Java's `crafterNum`/`recipeNum` loop read back
    /// (`:416-422`).
    pub recipes: usize,
    /// Loot tables stored (`:424-434`).
    pub loot: usize,
}

/// Why an ingest stopped (`:412-444`): which recipe, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IngestRefusal {
    /// The position of the refused recipe in the list the caller handed over.
    pub index: usize,
    pub refusal: Refusal,
}

/// The size of every table, for a log line or a test.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    /// Crafters that have at least one recipe (`recipeMap.size()`).
    pub crafters: usize,
    /// Recipes, all crafters together.
    pub recipes: usize,
    /// Items in `recipeOutputMap`, each with at least one entry — except after a
    /// removal, where the list can be left empty and the key stays, as in Java.
    pub outputs: usize,
    /// Removals queued and not yet applied.
    pub pending: usize,
    /// Loot tables.
    pub loot: usize,
    /// Templates waiting for the tags.
    pub templates: usize,
}

/// Why the store refused a recipe, named.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: one refusal, and it is the one `id == null` Java would silently key
pub enum Refusal {
    /// A recipe with no file id. Java's `recipeId` is never null on the load path
    /// (`CustomRecipe.java:591-593` takes it in the constructor and `:402-403`
    /// builds it per file), but a `HashMap` would accept a null key without a
    /// word — so the port refuses instead of keying "no id" under one name.
    RecipeWithoutId { crafter: String },
}

/// The manager: two indexes, three side tables, one pending list.
#[derive(Clone, Debug, Default)]
pub struct Store {
    /// `recipeMap` (`:46`): crafter → id → recipe.
    by_crafter: BTreeMap<String, BTreeMap<Identifier, CrafterRecipe>>,
    /// `recipeOutputMap` (`:51`): item → the recipes that output it, **in
    /// insertion order and with duplicates** (behaviour 3 of the module note).
    by_output: BTreeMap<ItemName, Vec<CrafterRecipe>>,
    /// `removedRecipes` (`:57`): ids queued by [`Store::remove`].
    pending: Vec<Identifier>,
    /// `lootTables` (`:62`).
    loot: BTreeMap<Identifier, Vec<LootDrop>>,
    /// `recipeTemplates` (`:67`).
    templates: BTreeMap<Identifier, Value>,
}

impl Store {
    /// An empty store, as the singleton starts (`:41`).
    pub fn new() -> Self {
        Self::default()
    }

    /// Java's `addRecipe` (`:87-109`), with the id's presence checked **before**
    /// either map is touched.
    ///
    /// The two writes, in Java's order: the crafter's map is created if it does
    /// not exist and the recipe is put under its id (`:89-94`), then the recipe is
    /// *appended* to the output index of its primary output and of each alternate
    /// output (`:96-108`). The replaced entry is returned — Java's `put` returns
    /// it and ignores it; a caller here can see a duplicate about to be created.
    pub fn add(&mut self, recipe: CrafterRecipe) -> Result<Option<CrafterRecipe>, Refusal> {
        let Some(id) = recipe.id.clone() else {
            return Err(Refusal::RecipeWithoutId {
                crafter: recipe.crafter.clone(),
            });
        };
        let replaced = self
            .by_crafter
            .entry(recipe.crafter.clone())
            .or_default()
            .insert(id, recipe.clone());
        for item in indexed_items(&recipe) {
            self.by_output
                .entry(item)
                .or_default()
                .push(recipe.clone());
        }
        Ok(replaced)
    }

    /// Java's `removeRecipe` (`:115-121`): **queues**, and does nothing else.
    ///
    /// The same id queued twice is queued once (`if (!removedRecipes.contains(..))`).
    pub fn remove(&mut self, id: Identifier) {
        if !self.pending.contains(&id) {
            self.pending.push(id);
        }
    }

    /// The `remove` file of `src/crafting/`, which is the same id wrapped
    /// (`CrafterRecipeListener.java:60-61`).
    pub fn remove_file(&mut self, removal: &Removal) {
        self.remove(removal.recipe.clone());
    }

    /// The removals that have been queued and not applied yet — behaviour 1 of the
    /// module note, in one accessor.
    pub fn pending(&self) -> &[Identifier] {
        &self.pending
    }

    /// Java's `removeRecipes` (`:249-275`): apply every queued id, then empty the
    /// list.
    ///
    /// For each id it finds the **first** crafter map holding it and removes it
    /// there only (`:255-258`), then removes the recipe from the output list of its
    /// primary item and of each alternate — `List.remove(Object)`, so **one** equal
    /// element, not every one (`:264-268`).
    pub fn flush(&mut self) {
        let queued = std::mem::take(&mut self.pending);
        for id in queued {
            let found = {
                let map = self
                    .by_crafter
                    .values_mut()
                    .find(|recipes| recipes.contains_key(&id));
                map.and_then(|recipes| recipes.remove(&id))
            };
            let Some(recipe) = found else {
                // Java's `findFirst().ifPresent(..)` on an id no crafter holds:
                // the queue entry is dropped and nothing is written.
                continue;
            };
            let mut items = vec![primary_item(&recipe)];
            items.extend(recipe.alternate_output.iter().map(|out| out.item.name().clone()));
            for item in items {
                let Some(held) = self.by_output.get_mut(&item) else {
                    continue;
                };
                if let Some(position) = held.iter().position(|stored| stored == &recipe) {
                    held.remove(position);
                }
            }
        }
    }

    /// Java's `getRecipes` (`:152-157`): **flush first**, then the crafter's set,
    /// sorted by id.
    ///
    /// Java returns a `HashSet` of the map's values, so its order is unspecified;
    /// the map is a `BTreeMap` here and the order is the id's. A crafter nobody
    /// registered answers an empty list, which is what `getOrDefault(new HashMap<>())`
    /// does (`:156`).
    pub fn recipes(&mut self, crafter: &str) -> Vec<&CrafterRecipe> {
        self.flush();
        self.by_crafter
            .get(crafter)
            .map(|recipes| recipes.values().collect::<Vec<&CrafterRecipe>>())
            .unwrap_or_default()
    }

    /// Java's `getAllRecipes` (`:162-165`): the map itself, **and no flush** —
    /// behaviour 1.
    pub fn all(&self) -> &BTreeMap<String, BTreeMap<Identifier, CrafterRecipe>> {
        &self.by_crafter
    }

    /// Java's `getRecipeByOutput(Item)` (`:172-179`): the live list for one item,
    /// or empty. Keyed by the item, so two recipes differing only by NBT share a
    /// key, as they do in Java.
    pub fn recipes_for_item(&self, item: &ItemName) -> &[CrafterRecipe] {
        self.by_output
            .get(item)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// Java's `getRecipeByOutput(ItemStack)` (`:186-205`): `ItemStack.matches`, so
    /// **item, count and tags** are compared — and a recipe whose primary output
    /// matches is added *and* its alternates are still scanned, so one recipe can
    /// appear twice in the answer.
    pub fn recipes_for_stack(&self, item: &ItemId, count: u32) -> Vec<&CrafterRecipe> {
        let mut found = Vec::new();
        for recipe in self.recipes_for_item(item.name()) {
            if primary_matches(recipe, item, count) {
                found.push(recipe);
            }
            for output in &recipe.alternate_output {
                if output.item.same_item(item, false) && output.count == count {
                    found.push(recipe);
                }
            }
        }
        found
    }

    /// Java's `getRecipeByOutput(ItemStorage)` (`:212-233`): the count is
    /// **ignored** (`ItemStorage.equals` is `compareItemStacksIgnoreStackSize`,
    /// `ItemStorage.java:255-267`) and the tags are compared unless either side
    /// ignores them (`:266`).
    ///
    /// The alternate loop `break`s after its first match (`:229`) where the primary
    /// test does not — so with two matching alternates this answers one recipe
    /// once, and with a matching primary *and* alternate, twice.
    pub fn recipes_for_storage(&self, item: &ItemId, ignore_nbt: bool) -> Vec<&CrafterRecipe> {
        let mut found = Vec::new();
        for recipe in self.recipes_for_item(item.name()) {
            if primary_matches_storage(recipe, item, ignore_nbt) {
                found.push(recipe);
            }
            for output in &recipe.alternate_output {
                if output.item.same_item(item, ignore_nbt) {
                    found.push(recipe);
                    break;
                }
            }
        }
        found
    }

    /// Java's `addRecipeTemplate` (`:128-132`): stored **raw**, because the item
    /// tags are not loaded yet. A second template under one id replaces the first,
    /// which is what a `Map.put` does. Returns the replaced JSON.
    pub fn add_template(&mut self, id: Identifier, json: Value) -> Option<Value> {
        self.templates.insert(id, json)
    }

    /// Java's `resolveTemplates` (`:282-300`), with the expansion handed in.
    ///
    /// `expand` is `CustomRecipe.parseTemplate` — one file, one recipe per member
    /// of the named tag — which this port does not have (`MANQUANT.md` §3), so it
    /// is a parameter and not an invented implementation.
    ///
    /// Three of Java's rules, in its order: each template is expanded and its
    /// recipes are added **immediately** (`:288-291`), a template that throws costs
    /// only its own recipes and is reported (`:293-297`), and the template map is
    /// cleared **after** the loop whether or not anything failed (`:299`).
    pub fn resolve_templates<E: std::fmt::Display>(
        &mut self,
        mut expand: impl FnMut(&Identifier, &Value) -> Result<Vec<CrafterRecipe>, E>,
    ) -> Vec<TemplateFailure> {
        let ids: Vec<Identifier> = self.templates.keys().cloned().collect();
        let mut failures = Vec::new();
        for id in ids {
            // Cloned because `self.add` needs the store mutably while the template
            // is read. Java holds the object and mutates the same map safely.
            let Some(json) = self.templates.get(&id).cloned() else {
                continue;
            };
            match expand(&id, &json) {
                Ok(recipes) => {
                    for recipe in recipes {
                        if let Err(refusal) = self.add(recipe) {
                            failures.push(TemplateFailure {
                                id: id.clone(),
                                cause: format!("{refusal:?}"),
                            });
                        }
                    }
                }
                Err(cause) => failures.push(TemplateFailure {
                    id: id.clone(),
                    cause: cause.to_string(),
                }),
            }
        }
        // `:299`: unconditional, so a template that failed is not retried forever.
        self.templates.clear();
        failures
    }

    /// The ids a loot analysis would be built from: Java's `buildLootData` first
    /// half (`:311-358`).
    ///
    /// Every recipe's own `loot-table` in crafter order then id order, then the
    /// caller's `extra` — which is where Java puts the crop blocks' tables
    /// (`:324-330`), the herding and crafting modules' tables (`:332-355`) and the
    /// fishing table (`:357-358`). `distinct()` in Java (`:363`) is a filter that
    /// keeps the first occurrence, so the order is the order of collection.
    pub fn loot_ids(&self, extra: impl IntoIterator<Item = Identifier>) -> Vec<Identifier> {
        let mut ids: Vec<Identifier> = Vec::new();
        for recipes in self.by_crafter.values() {
            for recipe in recipes.values() {
                let Some(table) = &recipe.loot_table else {
                    continue;
                };
                if !ids.contains(table) {
                    ids.push(table.clone());
                }
            }
        }
        for id in extra {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        ids
    }

    /// Java's `buildLootData` second half (`:360-365`): **clear, then fill**, so a
    /// second build replaces the first instead of merging into it.
    pub fn replace_loot(&mut self, tables: impl IntoIterator<Item = (Identifier, Vec<LootDrop>)>) {
        self.loot.clear();
        for (id, drops) in tables {
            self.loot.insert(id, drops);
        }
    }

    /// Java's `getLootDrops` (`:242-247`): the drops of one table, and an **empty
    /// list for a null id** rather than every table's.
    pub fn drops(&self, id: Option<&Identifier>) -> &[LootDrop] {
        let Some(id) = id else {
            return &[];
        };
        self.loot.get(id).map(Vec::as_slice).unwrap_or_default()
    }

    /// Java's `reset` (`:138-145`): all five tables, in its order.
    pub fn reset(&mut self) {
        self.by_output.clear();
        self.by_crafter.clear();
        self.loot.clear();
        self.pending.clear();
        self.templates.clear();
    }

    /// Java's `handleCustomRecipeManagerMessage` (`:412-444`), with the buffer read
    /// already done.
    ///
    /// **The order is the rule, and it is why this is one function.** `reset()`
    /// comes first, before a single recipe is stored (`:414`); the recipes follow,
    /// each `addRecipe`-ed in packet order (`:416-422`); the loot tables come
    /// **after every recipe** (`:424-434`); and the caller posts
    /// `CustomRecipesReloadedEvent` **after all of it** (`:436-443`). A failure
    /// aborts the ingest where Java would throw, leaving what was already stored —
    /// the reset is not rolled back, and neither is the partial fill, because Java's
    /// is not either.
    pub fn ingest(
        &mut self,
        recipes: Vec<CrafterRecipe>,
        loot: Vec<(Identifier, Vec<LootDrop>)>,
    ) -> Result<Ingested, IngestRefusal> {
        self.reset();
        let mut ingested = Ingested::default();
        for (index, recipe) in recipes.into_iter().enumerate() {
            match self.add(recipe) {
                Ok(_) => ingested.recipes += 1,
                Err(refusal) => return Err(IngestRefusal { index, refusal }),
            }
        }
        for (id, drops) in loot {
            self.loot.insert(id, drops);
            ingested.loot += 1;
        }
        Ok(ingested)
    }

    /// The size of every table, in one value.
    pub fn counts(&self) -> Counts {
        Counts {
            crafters: self.by_crafter.len(),
            recipes: self
                .by_crafter
                .values()
                .map(|recipes| recipes.len())
                .sum::<usize>(),
            outputs: self.by_output.len(),
            pending: self.pending.len(),
            loot: self.loot.len(),
            templates: self.templates.len(),
        }
    }
}

/// The item Java keys a recipe's primary output under: `getPrimaryOutput().getItem()`
/// (`:96-100`), which is air for the recipes whose file has no `result`
/// (`CustomRecipe.java:278-283`).
fn primary_item(recipe: &CrafterRecipe) -> ItemName {
    recipe
        .result
        .as_ref()
        .map(|id| id.name().clone())
        .unwrap_or_else(air)
}

/// `Items.AIR`, as the engine names it.
fn air() -> ItemName {
    ItemName::new("minecraft:air").expect("air is an item name the engine accepts")
}

/// What `addRecipe` indexes a recipe under, in its order: the primary output, then
/// each alternate (`:96-108`). The secondary outputs are **not** indexed — Java's
/// loop is over `getAltOutputs()` only.
fn indexed_items(recipe: &CrafterRecipe) -> Vec<ItemName> {
    let mut items = vec![primary_item(recipe)];
    items.extend(
        recipe
            .alternate_output
            .iter()
            .map(|output| output.item.name().clone()),
    );
    items
}

/// `ItemStack.matches(primary, stack)`: the same item, the same count, the same
/// tags. A recipe with no `result` holds `EMPTY`, which is air with a count of
/// zero and matches no real stack.
fn primary_matches(recipe: &CrafterRecipe, item: &ItemId, count: u32) -> bool {
    match &recipe.result {
        Some(result) => result.same_item(item, false) && recipe.count == count,
        None => item.name() == &air() && count == 0,
    }
}

/// `new ItemStorage(primary).equals(storage)`: the item and the tags, **not** the
/// count (`ItemStorage.java:255-267`).
fn primary_matches_storage(recipe: &CrafterRecipe, item: &ItemId, ignore_nbt: bool) -> bool {
    match &recipe.result {
        Some(result) => result.same_item(item, ignore_nbt),
        None => item.name() == &air(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ident(text: &str) -> Identifier {
        Identifier::parse_namespaced(text).expect("a literal id parses")
    }

    fn item(text: &str) -> ItemId {
        ItemId::parse("item", text).expect("a literal item parses")
    }

    fn name(text: &str) -> ItemName {
        ItemName::new(text).expect("a literal item name parses")
    }

    /// A recipe through the real parser, so a fixture cannot disagree with the
    /// format: `{"type": "recipe", "crafter": …, "result": …, "count": …}`.
    fn recipe(id: &str, crafter: &str, result: &str, count: u32) -> CrafterRecipe {
        let json = format!(
            r#"{{"type": "recipe", "crafter": "{crafter}", "result": "{result}", "count": {count}}}"#
        );
        let value: Value = mod_api::serde_json::from_str(&json).expect("a test literal is JSON");
        CrafterRecipe::parse(&value)
            .expect("a minimal recipe parses")
            .with_id(ident(id))
    }

    /// The same, with one alternate output.
    fn recipe_alt(
        id: &str,
        crafter: &str,
        result: &str,
        count: u32,
        alt: &str,
        alt_count: u32,
    ) -> CrafterRecipe {
        let json = format!(
            r#"{{"type": "recipe", "crafter": "{crafter}", "result": "{result}", "count": {count},
                 "alternate-output": [{{"item": "{alt}", "count": {alt_count}}}]}}"#
        );
        let value: Value = mod_api::serde_json::from_str(&json).expect("a test literal is JSON");
        CrafterRecipe::parse(&value)
            .expect("a recipe with an alternate parses")
            .with_id(ident(id))
    }

    fn bread() -> CrafterRecipe {
        recipe("minecolonies:baker/bread", "baker_crafting", "minecraft:bread", 2)
    }

    #[test]
    fn add_keys_by_crafter_and_by_id_and_indexes_the_primary_and_the_alternates() {
        // `:89-108`.
        let mut store = Store::new();
        let alt = recipe_alt(
            "minecolonies:baker/cake",
            "baker_crafting",
            "minecraft:cake",
            1,
            "minecraft:sweet_berries",
            3,
        );
        assert_eq!(store.add(alt).expect("an id is set"), None);
        assert_eq!(store.add(bread()).expect("an id is set"), None);

        assert_eq!(store.counts().crafters, 1);
        assert_eq!(store.counts().recipes, 2);
        assert_eq!(store.counts().outputs, 3, "bread, cake and the alternate");
        assert_eq!(store.recipes_for_item(&name("minecraft:bread")).len(), 1);
        assert_eq!(store.recipes_for_item(&name("minecraft:sweet_berries")).len(), 1);
        assert_eq!(store.recipes_for_item(&name("minecraft:cake")).len(), 1);
        // The order inside a crafter is the id's, which is the map's.
        let ids: Vec<String> = store
            .recipes("baker_crafting")
            .iter()
            .map(|recipe| recipe.id.as_ref().expect("an id").to_string())
            .collect();
        assert_eq!(
            ids,
            ["minecolonies:baker/bread", "minecolonies:baker/cake"]
        );
    }

    #[test]
    fn re_adding_one_id_replaces_the_crafter_entry_and_duplicates_the_output_list() {
        // `:94` puts, `:100` appends: behaviour 3 of the module note.
        let mut store = Store::new();
        assert_eq!(store.add(bread()).expect("an id is set"), None);
        let replaced = store.add(bread()).expect("an id is set");
        assert!(replaced.is_some(), "the first copy came back");
        assert_eq!(store.counts().recipes, 1, "one entry per crafter and id");
        assert_eq!(
            store.recipes_for_item(&name("minecraft:bread")).len(),
            2,
            "and two in the output index"
        );
    }

    #[test]
    fn a_recipe_with_no_id_is_refused_and_nothing_is_written() {
        let value: Value = mod_api::serde_json::from_str(
            r#"{"type": "recipe", "crafter": "baker_crafting", "result": "minecraft:bread"}"#,
        )
        .expect("JSON");
        let id_less = CrafterRecipe::parse(&value).expect("a recipe without an id parses");
        let mut store = Store::new();
        assert_eq!(
            store.add(id_less),
            Err(Refusal::RecipeWithoutId {
                crafter: "baker_crafting".to_owned()
            })
        );
        // Java's `put` would key a null; the refusal comes before both maps.
        assert_eq!(store.counts(), Counts::default());
        assert!(store.all().is_empty());
        assert!(store.recipes_for_item(&name("minecraft:bread")).is_empty());
    }

    #[test]
    fn a_recipe_with_no_result_is_indexed_under_air() {
        // `CustomRecipe.java:278-283` sets `ItemStack.EMPTY`, and `:96-100` keys by
        // its item. Behaviour 4.
        let value: Value = mod_api::serde_json::from_str(
            r#"{"type": "recipe", "crafter": "sifter_custom", "inputs": [{"item": "minecraft:sand"}]}"#,
        )
        .expect("JSON");
        let no_result = CrafterRecipe::parse(&value)
            .expect("a recipe without a result parses")
            .with_id(ident("minecolonies:sifter/nothing"));
        let mut store = Store::new();
        store.add(no_result).expect("an id is set");
        assert_eq!(store.recipes_for_item(&name("minecraft:air")).len(), 1);
        // And a stack query never matches it: EMPTY has a count of zero, and no
        // real stack has one.
        assert!(store.recipes_for_stack(&item("minecraft:bread"), 1).is_empty());
        assert!(store.recipes_for_stack(&item("minecraft:air"), 1).is_empty());
    }

    #[test]
    fn a_removal_is_queued_and_applied_only_by_the_crafter_accessor() {
        // `:115-121` queues, `:154` applies. Behaviour 1.
        let mut store = Store::new();
        store.add(bread()).expect("an id is set");
        store.remove(ident("minecolonies:baker/bread"));
        store.remove(ident("minecolonies:baker/bread"));
        assert_eq!(store.pending().len(), 1, "queued once, not twice");
        // Still there for everyone else.
        assert_eq!(store.recipes_for_item(&name("minecraft:bread")).len(), 1);
        assert_eq!(store.counts().recipes, 1);
        assert_eq!(store.all()["baker_crafting"].len(), 1);

        assert!(store.recipes("baker_crafting").is_empty());
        assert!(store.pending().is_empty(), "the queue is emptied");
        assert_eq!(store.recipes_for_item(&name("minecraft:bread")).len(), 0);
        assert_eq!(store.counts().crafters, 1, "the crafter's map stays, empty");
    }

    #[test]
    fn a_removal_reaches_one_crafter_map_and_it_is_the_alphabetically_first() {
        // `:255-258`: `findFirst`, and Java's `HashMap` order is unspecified; the
        // port pins the first crafter by name.
        let mut store = Store::new();
        store
            .add(recipe("minecolonies:shared/bread", "zulu_crafting", "minecraft:bread", 1))
            .expect("an id is set");
        store
            .add(recipe("minecolonies:shared/bread", "alpha_crafting", "minecraft:bread", 1))
            .expect("an id is set");
        assert_eq!(store.counts().recipes, 2);

        store.remove(ident("minecolonies:shared/bread"));
        store.flush();
        // `alpha_crafting` lost it; `zulu_crafting` kept it — the divergence the
        // module note owns, and a Java caller sees the same because it removes one.
        assert!(store.all()["alpha_crafting"].is_empty());
        assert_eq!(store.all()["zulu_crafting"].len(), 1);
    }

    #[test]
    fn a_removal_takes_one_equal_copy_out_of_the_output_index() {
        // `List.remove(Object)` on a doubled list (`:264-268`), with a doubled
        // recipe from behaviour 3.
        let mut store = Store::new();
        store.add(bread()).expect("an id is set");
        store.add(bread()).expect("an id is set");
        assert_eq!(store.recipes_for_item(&name("minecraft:bread")).len(), 2);

        store.remove(ident("minecolonies:baker/bread"));
        store.flush();
        assert_eq!(
            store.recipes_for_item(&name("minecraft:bread")).len(),
            1,
            "one copy went, one stayed — and the key stays with its empty list"
        );
        assert_eq!(store.counts().recipes, 0);
        assert_eq!(store.counts().outputs, 1);
    }

    #[test]
    fn a_removal_of_an_id_nobody_holds_writes_nothing() {
        // `findFirst().ifPresent(..)`: no crafter, no write, and the queue empties.
        let mut store = Store::new();
        store.add(bread()).expect("an id is set");
        store.remove(ident("minecolonies:baker/ghost"));
        store.flush();
        assert!(store.pending().is_empty());
        assert_eq!(store.counts().recipes, 1);
        assert_eq!(store.recipes_for_item(&name("minecraft:bread")).len(), 1);
    }

    #[test]
    fn a_stack_query_compares_the_count_and_a_storage_query_does_not() {
        // `:192` is `ItemStack.matches` (count included); `:218` is
        // `ItemStorage.equals` (count ignored, `ItemStorage.java:266`).
        let mut store = Store::new();
        store.add(bread()).expect("an id is set");
        assert_eq!(store.recipes_for_stack(&item("minecraft:bread"), 2).len(), 1);
        assert!(store.recipes_for_stack(&item("minecraft:bread"), 1).is_empty());
        assert_eq!(store.recipes_for_storage(&item("minecraft:bread"), false).len(), 1);
        // A different item is not a match in either.
        assert!(store.recipes_for_storage(&item("minecraft:cake"), false).is_empty());
    }

    #[test]
    fn a_storage_query_matches_the_tags_unless_it_is_told_not_to() {
        // `ItemStorage.equals` passes `!(a.shouldIgnoreNBTValue || b.shouldIgnoreNBTValue)`
        // to the comparison (`:266`), and `ItemId::same_item` is the same flag.
        let plain = recipe("minecolonies:baker/bread", "baker_crafting", "minecraft:bread", 2);
        let tagged = recipe(
            "minecolonies:baker/tagged_bread",
            "baker_crafting",
            "minecraft:bread{Damage:1}",
            2,
        );
        let mut store = Store::new();
        store.add(plain).expect("an id is set");
        store.add(tagged).expect("an id is set");
        let mine = item("minecraft:bread");
        assert_eq!(store.recipes_for_storage(&mine, false).len(), 1);
        assert_eq!(store.recipes_for_storage(&mine, true).len(), 2);
        // A stack query is exact about the tags too.
        assert_eq!(store.recipes_for_stack(&mine, 2).len(), 1);
    }

    #[test]
    fn an_alternate_output_that_matches_the_query_adds_the_recipe_again_and_again() {
        // `:190-202` plus behaviour 3, and the two multiply: the recipe is indexed
        // **twice** under bread (once as the primary, once as the alternate,
        // `:96-108`), and each of those two passes adds it for its primary match
        // *and* for its alternate match. Four answers for one recipe.
        let mut store = Store::new();
        store
            .add(recipe_alt(
                "minecolonies:baker/both",
                "baker_crafting",
                "minecraft:bread",
                1,
                "minecraft:bread",
                1,
            ))
            .expect("an id is set");
        assert_eq!(store.recipes_for_item(&name("minecraft:bread")).len(), 2);
        assert_eq!(store.recipes_for_stack(&item("minecraft:bread"), 1).len(), 4);
    }

    #[test]
    fn the_storage_query_breaks_after_the_first_matching_alternate() {
        // `:229`: the `break` the stack query does not have. The same doubling as
        // above gives two passes over the list, and the `break` is what keeps each
        // pass to one answer even though both alternates match.
        let json = r#"{"type": "recipe", "crafter": "baker_crafting", "result": "minecraft:cake",
                       "alternate-output": [{"item": "minecraft:bread", "count": 1},
                                            {"item": "minecraft:bread", "count": 1}]}"#;
        let value: Value = mod_api::serde_json::from_str(json).expect("JSON");
        let recipe = CrafterRecipe::parse(&value)
            .expect("two alternates parse")
            .with_id(ident("minecolonies:baker/two_alts"));
        let mut store = Store::new();
        store.add(recipe).expect("an id is set");
        assert_eq!(store.recipes_for_item(&name("minecraft:bread")).len(), 2);
        // Two passes, one answer each — four would be the answer without the `break`.
        assert_eq!(store.recipes_for_storage(&item("minecraft:bread"), false).len(), 2);
        // The stack query has no `break`, and hits both alternates of both passes.
        assert_eq!(store.recipes_for_stack(&item("minecraft:bread"), 1).len(), 4);
    }

    #[test]
    fn reset_empties_every_table_and_ingest_resets_before_it_starts() {
        // `:138-145` and `:414`. The reset is what stops a data-pack reload from
        // carrying another world's recipes over.
        let mut store = Store::new();
        let template: Value = mod_api::serde_json::from_str(r#"{"tag": "minecraft:logs"}"#)
            .expect("JSON");
        store.add(bread()).expect("an id is set");
        store.remove(ident("minecolonies:baker/bread"));
        store.add_template(ident("minecolonies:templates/logs"), template);
        store.replace_loot(vec![(
            ident("minecraft:blocks/oak_log"),
            vec![LootDrop::new(1.0, 0.0, false)],
        )]);

        store.reset();
        assert_eq!(store.counts(), Counts::default());

        // And an ingest does it too, without the caller asking.
        store.add(recipe("minecolonies:baker/old", "baker_crafting", "minecraft:bread", 1))
            .expect("an id is set");
        let ingested = store
            .ingest(
                vec![recipe("minecolonies:baker/new", "baker_crafting", "minecraft:cake", 1)],
                vec![(ident("minecraft:blocks/oak_log"), vec![LootDrop::new(0.5, 0.0, true)])],
            )
            .expect("a good packet");
        assert_eq!(ingested, Ingested { recipes: 1, loot: 1 });
        assert_eq!(store.counts().recipes, 1, "the old batch is gone");
        assert!(store.recipes_for_item(&name("minecraft:bread")).is_empty());
        assert_eq!(store.drops(Some(&ident("minecraft:blocks/oak_log"))).len(), 1);
    }

    #[test]
    fn an_ingest_stops_at_the_first_recipe_it_cannot_key_and_never_reaches_the_loot() {
        // `:416-434`: a throw in the recipe loop aborts before the loot loop, so
        // the event at `:436` is never posted. The reset stays, and so does what
        // was already stored: Java's ingest is not atomic and neither is this.
        let value: Value = mod_api::serde_json::from_str(
            r#"{"type": "recipe", "crafter": "baker_crafting", "result": "minecraft:bread"}"#,
        )
        .expect("JSON");
        let id_less = CrafterRecipe::parse(&value).expect("parses");
        let mut store = Store::new();
        store.add(recipe("minecolonies:baker/old", "baker_crafting", "minecraft:cake", 1))
            .expect("an id is set");

        let refused = store.ingest(
            vec![
                recipe("minecolonies:baker/new", "baker_crafting", "minecraft:bread", 1),
                id_less,
            ],
            vec![(ident("minecraft:blocks/oak_log"), vec![LootDrop::new(1.0, 0.0, false)])],
        );
        assert_eq!(
            refused,
            Err(IngestRefusal {
                index: 1,
                refusal: Refusal::RecipeWithoutId {
                    crafter: "baker_crafting".to_owned()
                }
            })
        );
        // The first recipe of the packet is stored, the loot is not.
        assert_eq!(store.counts().recipes, 1);
        assert_eq!(store.counts().loot, 0);
        assert_eq!(store.drops(Some(&ident("minecraft:blocks/oak_log"))).len(), 0);
    }

    #[test]
    fn a_template_that_fails_costs_only_its_own_recipes_and_the_map_is_cleared_either_way() {
        // `:284-299`.
        let mut store = Store::new();
        let good: Value = mod_api::serde_json::from_str(r#"{"tag": "minecraft:logs"}"#).expect("JSON");
        let bad: Value = mod_api::serde_json::from_str(r#"{"tag": "minecraft:planks"}"#).expect("JSON");
        store.add_template(ident("minecolonies:templates/good"), good);
        store.add_template(ident("minecolonies:templates/bad"), bad);
        assert_eq!(store.counts().templates, 2);

        let failures = store.resolve_templates(|id, _json| {
            if id.path().ends_with("bad") {
                Err("no members for that tag".to_owned())
            } else {
                Ok(vec![recipe(
                    "minecolonies:templates/good/oak_log",
                    "lumberjack_custom",
                    "minecraft:oak_planks",
                    4,
                )])
            }
        });
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].id, ident("minecolonies:templates/bad"));
        assert_eq!(failures[0].cause, "no members for that tag");
        assert_eq!(store.counts().recipes, 1, "the good one landed");
        assert_eq!(store.counts().templates, 0, "and both are forgotten");
    }

    #[test]
    fn a_second_template_under_one_id_replaces_the_first() {
        // `:128-132` is a `Map.put`.
        let mut store = Store::new();
        let first: Value = mod_api::serde_json::from_str(r#"{"tag": "minecraft:logs"}"#).expect("JSON");
        let second: Value = mod_api::serde_json::from_str(r#"{"tag": "minecraft:planks"}"#).expect("JSON");
        assert_eq!(
            store.add_template(ident("minecolonies:templates/t"), first.clone()),
            None
        );
        assert_eq!(
            store.add_template(ident("minecolonies:templates/t"), second.clone()),
            Some(first)
        );
        assert_eq!(store.counts().templates, 1);
    }

    #[test]
    fn the_loot_ids_are_the_recipes_own_first_then_the_callers_and_distinct() {
        // `:311-358` collects, `:363` distincts while keeping the first occurrence.
        let mut store = Store::new();
        let json = r#"{"type": "recipe", "crafter": "baker_crafting", "result": "minecraft:bread",
                       "loot-table": "minecraft:blocks/oak_log"}"#;
        let value: Value = mod_api::serde_json::from_str(json).expect("JSON");
        store
            .add(
                CrafterRecipe::parse(&value)
                    .expect("parses")
                    .with_id(ident("minecolonies:baker/loggy")),
            )
            .expect("an id is set");
        store.add(bread()).expect("an id is set");

        let ids = store.loot_ids(vec![
            ident("minecraft:blocks/oak_log"),
            ident("minecraft:gameplay/fishing"),
        ]);
        assert_eq!(
            ids,
            [
                ident("minecraft:blocks/oak_log"),
                ident("minecraft:gameplay/fishing")
            ],
            "the recipe's table first, then the caller's, with the repeat dropped"
        );
    }

    #[test]
    fn a_loot_rebuild_clears_before_it_fills_and_a_null_id_answers_nothing() {
        // `:360-365` and `:242-247`.
        let mut store = Store::new();
        store.replace_loot(vec![
            (ident("minecraft:blocks/oak_log"), vec![LootDrop::new(1.0, 0.0, false)]),
            (ident("minecraft:blocks/birch_log"), vec![LootDrop::new(0.5, 0.1, true)]),
        ]);
        assert_eq!(store.counts().loot, 2);

        store.replace_loot(vec![(
            ident("minecraft:blocks/birch_log"),
            vec![LootDrop::new(0.25, 0.0, false)],
        )]);
        assert_eq!(store.counts().loot, 1, "the first build did not merge");
        assert!(store.drops(Some(&ident("minecraft:blocks/oak_log"))).is_empty());
        assert_eq!(store.drops(Some(&ident("minecraft:blocks/birch_log"))).len(), 1);
        assert!(
            store.drops(None).is_empty(),
            "a null id is an empty list, not every table"
        );
    }
}
