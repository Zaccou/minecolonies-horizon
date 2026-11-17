//! The recipes a colony has learned: which ones are written, which ones are
//! dropped on the way in, and the four caches Java keeps beside them.
//!
//! Java reference: `core/colony/requestsystem/management/manager/StandardRecipeManager.java`
//! (148 lines), with the recipe's own shape in `core/colony/crafting/RecipeStorageFactory.java`
//! (`serialize` `:104-150`, `deserialize` `:153-210`) and the equality the book is
//! keyed by in `api/crafting/RecipeStorage.java:529-594`.
//!
//! **A recipe is a token and a body, and the body is the identity.** Java keeps a
//! `HashBiMap` in both directions (`:30`): token → storage for a lookup by token,
//! storage → token for `getRecipeId` (`:91-94`). The second direction is a lookup
//! **by content**, so [`RecipeBook::get_id`] compares the stored body, and a
//! [`Storage`] is exactly `token + the serialized recipe minus its token`.
//!
//! **`RecipeStorage.equals` does not look at the token** (`:529-594`: grid size,
//! cleaned inputs, primary output, alternates, secondaries, tools, source, loot
//! table, type, intermediate — the token is not in the list). That is why "same
//! body" is the right identity here, and the two places where tag equality is
//! **stricter** than `equals` are named rather than hidden:
//!
//! 1. `equals` compares the primary output with
//!    `compareItemStacksIgnoreStackSize(..., false, true)` (`:547`), so two
//!    recipes differing only by output *count* are equal in Java and different
//!    here.
//! 2. `getCleanedInput()` is built by merging duplicate inputs and dropping the
//!    build tool (`RecipeStorage.java:350-420`), so two `input` lists that merge
//!    to the same cleaned list are equal in Java and different here.
//!
//! Both directions of the port use the same identity, which is the point: Java's
//! two directions do not. `hashCode` (`:598-614`) hashes the cleaned inputs, the
//! primary output's **item and count**, the intermediate, the grid size, the tool
//! and the three stack lists — it ignores `recipeSource`, the loot table, the
//! type and the primary output's NBT, all of which `equals` compares. A
//! `HashMap`-backed inverse built on that pair can therefore **miss** a recipe
//! `containsValue` finds, so `checkOrAddRecipe` can file a duplicate that `read`
//! would have dropped. The port answers one thing in both directions.
//!
//! **The write is gated, and that is the rule worth reading twice.** Only recipes
//! **used** since the manager was created are written (`:101`), and `used` is
//! filled by `addRecipe` and `registerUse` (`:72`, `:139-147`) — so a recipe this
//! session added but never used is not in the file. Java then rewrites the tag it
//! read when nothing changed (`:99-106`: `dirty` or an absent cache), which is
//! what lets a save survive a load/print/store round trip with entries this port
//! refused — [`RecipeBook::read`] keeps that list and [`RecipeBook::write`] hands
//! it back untouched.
//!
//! **One divergence, and it is the port's own.** `reset()` clears the recipes and
//! the used set (`:132-136`) and leaves the tag cache and the dirty flag alone —
//! so a Java `write` right after a `reset` puts the **pre-reset list** back,
//! resurrecting every recipe the reset deleted. [`RecipeBook::reset`] clears both
//! caches, because a reset that can be undone by the next save is not a reset.
//! The cost is stated in `MANQUANT.md` §4: a port instance that resets and then
//! writes produces the same file Java would have produced *before* its own bug,
//! not the same file Java produces with it.

use std::collections::BTreeSet;

use mod_api::{Compound, Tag};

use super::datastores::{Token, token_of, token_tag};
use super::nbt;

/// `TAG_RECIPES` (`StandardRecipeManager.java:24`).
pub const TAG_RECIPES: &str = "recipes";
/// `TAG_TOKEN` (`NbtTagConstants.java:236`), written inside each recipe by
/// `RecipeStorageFactory.serialize` (`:135`).
pub const TAG_TOKEN: &str = "Token";
/// `INPUT_TAG` (`RecipeStorageFactory.java:58`).
pub const KEY_INPUT: &str = "input";
/// `ALTOUTPUT_TAG` (`:70`).
pub const KEY_ALTERNATE: &str = "alternate-output";
/// `LOOT_TAG` (`:82`).
pub const KEY_LOOT: &str = "loot-table";
/// The key an `ItemStack` saves its item under, which both branches of
/// `RecipeStorageFactory.deserialize` read (`:163-172`).
pub const KEY_ITEM: &str = "id";
/// What `ItemStack.EMPTY` names: an item id of air is an empty stack
/// (`ItemStack.isEmpty` is `getItem() == AIR && count == 0`), and an absent `id`
/// reads as empty too (`ItemStack.of`).
pub const AIR: &str = "minecraft:air";

/// One stored recipe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Storage {
    token: Token,
    body: Compound,
}

impl Storage {
    /// A recipe read from its saved compound, or the reason it is not one.
    pub fn from_tag(tag: &Compound) -> Result<Storage, SkipReason> {
        let Some(token_tag_value) = tag.get(&TAG_TOKEN.into()) else {
            // Java reads an absent `Token` as an empty compound and hands it to
            // the controller, which cannot name a factory for it: the whole
            // `deserialize` throws and the entry is dropped.
            return Err(SkipReason::NoToken);
        };
        let token = token_of(token_tag_value).map_err(|_| SkipReason::NoToken)?;
        let mut body = tag.clone();
        body.remove(&TAG_TOKEN.into());
        Ok(Storage { token, body })
    }

    /// A recipe the caller has already built, from whatever it read or crafted.
    pub fn new(token: Token, body: Compound) -> Storage {
        let mut body = body;
        body.remove(&TAG_TOKEN.into());
        Storage { token, body }
    }

    pub fn token(&self) -> Token {
        self.token
    }

    /// Everything Java's `equals` compares, and nothing else.
    pub fn body(&self) -> &Compound {
        &self.body
    }

    /// The saved form: the body, with the token back where Java writes it.
    pub fn to_tag(&self) -> Compound {
        let mut tag = Compound::from([(TAG_TOKEN.into(), token_tag(self.token))]);
        for (key, value) in &self.body {
            tag.insert(key.clone(), value.clone());
        }
        tag
    }
}

/// A recipe the read refused, and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Skip {
    pub index: usize,
    pub reason: SkipReason,
}

/// Why one saved recipe is not in the book. Every variant is a term of Java's one
/// condition (`:115`) or of its `catch` (`:121-124`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one variant per way out of Java's filter
pub enum SkipReason {
    /// The entry is not a compound, so `deserialize` has nothing to read: Java's
    /// `recipe == null` (`:114-115`).
    NotARecipe,
    /// No `Token`, or one this port cannot express: `deserialize` throws and the
    /// catch at `:121-124` eats it.
    NoToken,
    /// A recipe already in the book has the same body (`:115`
    /// `!recipes.containsValue(recipe)`).
    Duplicate { token: Token },
    /// No cleaned input at all (`:115`): a recipe that consumes nothing.
    NoInput,
    /// No primary output, no loot table and no alternate output (`:115`): a recipe
    /// that produces nothing.
    NoOutput,
}

impl SkipReason {
    pub fn name(self) -> &'static str {
        match self {
            Self::NotARecipe => "recipes.skip.not_a_recipe",
            Self::NoToken => "recipes.skip.no_token",
            Self::Duplicate { .. } => "recipes.skip.duplicate",
            Self::NoInput => "recipes.skip.no_input",
            Self::NoOutput => "recipes.skip.no_output",
        }
    }
}

/// The colony's recipes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecipeBook {
    entries: Vec<Storage>,
    used: BTreeSet<Token>,
    dirty: bool,
    /// Java's `ImmutableMap` snapshot (`:35`, `:55-60`), dropped on every change
    /// (`:73`, `:127`).
    snapshot: Option<Vec<Storage>>,
    /// Java's `nbtCache` (`:50`, `:99-104`): the list last read or written, kept
    /// as the tags themselves so a mix this port refuses still round trips.
    saved: Option<Vec<Tag>>,
}

impl RecipeBook {
    pub fn new() -> RecipeBook {
        RecipeBook::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Whether something changed since the last save (`:43-45`, `:105`).
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// The recipes, in the order they were added or read.
    pub fn iter(&self) -> impl Iterator<Item = &Storage> {
        self.entries.iter()
    }

    /// `getRecipe` (`:63-66`).
    pub fn get(&self, token: Token) -> Option<&Storage> {
        self.entries.iter().find(|storage| storage.token == token)
    }

    /// `getRecipeId` (`:91-94`): the token of the recipe with this content, if the
    /// colony has it. `None` is what makes `checkOrAddRecipe` add.
    pub fn get_id(&self, body: &Compound) -> Option<Token> {
        self.entries
            .iter()
            .find(|storage| &storage.body == body)
            .map(|storage| storage.token)
    }

    /// The snapshot Java caches (`:53-60`), dropped on `addRecipe` and on `read`
    /// (`:73`, `:127`).
    pub fn snapshot(&mut self) -> &[Storage] {
        if self.snapshot.is_none() {
            self.snapshot = Some(self.entries.clone());
        }
        match &self.snapshot {
            Some(stored) => stored,
            None => &[],
        }
    }

    /// `addRecipe` (`:69-76`): file it, count it as used, drop the snapshot, mark
    /// dirty — in that order.
    ///
    /// `HashBiMap.put` **replaces** the value under an existing key and drops the
    /// old value's inverse, so a second recipe under one token wins and the first
    /// one is gone; the port does the same.
    pub fn add(&mut self, storage: Storage) -> Token {
        match self.entries.iter_mut().find(|known| known.token == storage.token) {
            Some(slot) => *slot = storage.clone(),
            None => self.entries.push(storage.clone()),
        }
        self.register_use(storage.token);
        self.snapshot = None;
        self.dirty = true;
        storage.token
    }

    /// `checkOrAddRecipe` (`:79-88`): the recipe with this content if the colony
    /// has it, otherwise a new one. A hit marks the known one used and adds
    /// nothing.
    pub fn check_or_add(&mut self, storage: Storage) -> Token {
        match self.get_id(&storage.body) {
            Some(token) => {
                self.register_use(token);
                token
            }
            None => self.add(storage),
        }
    }

    /// `registerUse` (`:139-147`): the first use of a recipe marks the manager
    /// dirty, the following ones change nothing.
    pub fn register_use(&mut self, token: Token) -> bool {
        if self.used.contains(&token) {
            return false;
        }
        self.used.insert(token);
        self.dirty = true;
        true
    }

    /// Whether this recipe would be written: Java only saves the ones used since
    /// the manager was created (`:101`).
    pub fn is_used(&self, token: Token) -> bool {
        self.used.contains(&token)
    }

    /// `write` (`:97-106`): the used recipes, or the list last read, and the dirty
    /// flag cleared.
    ///
    /// The filter walks the recipes in book order and keeps the used ones, which
    /// is what Java's stream over a `HashBiMap` entry set does in insertion order.
    pub fn write(&mut self) -> Compound {
        if self.dirty || self.saved.is_none() {
            let list: Vec<Tag> = self
                .entries
                .iter()
                .filter(|storage| self.used.contains(&storage.token))
                .map(|storage| Tag::Compound(storage.to_tag()))
                .collect();
            self.saved = Some(list);
        }
        self.dirty = false;
        let list = self.saved.clone().unwrap_or_default();
        Compound::from([(TAG_RECIPES.into(), Tag::List(list))])
    }

    /// `read` (`:109-129`), with every term of the condition named.
    ///
    /// **The tag cache is the list read, not the list kept** (`:128`): a Java
    /// `write` with nothing changed puts the dropped entries back. That is the
    /// behaviour this keeps, so a save round trips.
    pub fn read(&mut self, compound: &Compound) -> Vec<Skip> {
        let mut skipped = Vec::new();
        let raw: Vec<Tag> = nbt::list(compound, TAG_RECIPES).cloned().unwrap_or_default();
        for (index, item) in raw.iter().enumerate() {
            let Tag::Compound(entry) = item else {
                skipped.push(Skip { index, reason: SkipReason::NotARecipe });
                continue;
            };
            let storage = match Storage::from_tag(entry) {
                Ok(storage) => storage,
                Err(reason) => {
                    skipped.push(Skip { index, reason });
                    continue;
                }
            };
            // `:115` `!recipes.containsValue(recipe)`.
            if let Some(token) = self.get_id(&storage.body) {
                skipped.push(Skip { index, reason: SkipReason::Duplicate { token } });
                continue;
            }
            if !has_input(entry) {
                skipped.push(Skip { index, reason: SkipReason::NoInput });
                continue;
            }
            if !has_output(entry) {
                skipped.push(Skip { index, reason: SkipReason::NoOutput });
                continue;
            }
            self.entries.push(storage);
        }
        self.snapshot = None;
        self.saved = Some(raw);
        skipped
    }

    /// `reset` (`:132-136`): the recipes and the used set go. The two caches go
    /// with them here — see the module note; Java leaves its tag cache, and then
    /// writes the pre-reset recipes back.
    pub fn reset(&mut self) {
        self.entries.clear();
        self.used.clear();
        self.snapshot = None;
        self.saved = None;
    }
}

/// Whether a recipe has at least one input that names an item
/// (`!getCleanedInput().isEmpty()`, `:115`).
///
/// Java asks the built `ItemStorage`s, after merging duplicates and dropping the
/// build tool (`RecipeStorage.java:350-420`). The port cannot rebuild that list,
/// so it answers the coarse question: does any entry of `input` name an item that
/// is not air? A recipe whose only input is the build tool would be called
/// non-empty here and empty by Java; no shipped recipe has one.
fn has_input(entry: &Compound) -> bool {
    match entry.get(&KEY_INPUT.into()) {
        Some(Tag::List(items)) => items.iter().any(names_an_item),
        _ => false,
    }
}

/// Whether a recipe produces anything (`:115`, the three-way disjunction).
fn has_output(entry: &Compound) -> bool {
    let primary = entry
        .get(&KEY_ITEM.into())
        .and_then(nbt::text_of)
        .is_some_and(|item| item != AIR);
    let loot = nbt::has(entry, KEY_LOOT);
    let alternates = match entry.get(&KEY_ALTERNATE.into()) {
        Some(Tag::List(items)) => !items.is_empty(),
        _ => false,
    };
    primary || loot || alternates
}

/// Whether anything inside this tag names an item that is not air, which is what
/// an `ItemStorage` (`stack{id}`) and a bare `ItemStack` (`id`) both do.
fn names_an_item(tag: &Tag) -> bool {
    match tag {
        Tag::Compound(map) => {
            if let Some(Tag::String(item)) = map.get(&KEY_ITEM.into()) {
                if item.to_string_lossy() != AIR {
                    return true;
                }
            }
            map.values().any(names_an_item)
        }
        Tag::List(items) => items.iter().any(names_an_item),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A recipe body: one input and a primary output, which is the smallest thing
    /// Java keeps.
    fn body(name: &str) -> Compound {
        let output = format!("minecraft:{name}");
        Compound::from([
            (
                KEY_INPUT.into(),
                Tag::List(vec![Tag::Compound(Compound::from([(
                    KEY_ITEM.into(),
                    Tag::String("minecraft:ladder".into()),
                )]))]),
            ),
            (KEY_ITEM.into(), Tag::String(output.as_str().into())),
            ("grid".into(), Tag::Int(3)),
        ])
    }

    fn storage(token: Token, name: &str) -> Storage {
        Storage::new(token, body(name))
    }

    #[test]
    fn add_files_counts_and_invalidates_in_javas_order() {
        let mut book = RecipeBook::new();
        assert!(!book.is_dirty());
        assert_eq!(book.add(storage(1, "stick")), 1, ":75 returns the token");
        assert_eq!(book.len(), 1);
        assert!(book.is_used(1), ":72 registerUse after the put");
        assert!(book.is_dirty(), ":74");
        assert_eq!(book.get(1).map(Storage::token), Some(1));
    }

    #[test]
    fn a_recipe_added_but_never_used_is_not_written() {
        let mut book = RecipeBook::new();
        book.add(storage(1, "stick"));
        // `add` always registers the use (:72), so drop it to model the one case
        // Java does not have: a recipe filed by a path that did not use it.
        book.used.clear();
        let written = book.write();
        assert!(
            nbt::compounds(&written, TAG_RECIPES).is_empty(),
            ":101 filters on usedRecipes"
        );
        assert!(!book.is_dirty(), ":105 clears the flag");
        // Used again, and the next write has it — but the cache is only rebuilt
        // when the manager is dirty, so `register_use` is the other way in.
        assert!(book.register_use(1));
        let written = book.write();
        assert_eq!(nbt::compounds(&written, TAG_RECIPES).len(), 1);
    }

    #[test]
    fn only_the_first_use_of_a_recipe_marks_the_manager_dirty() {
        let mut book = RecipeBook::new();
        book.add(storage(1, "stick"));
        book.write();
        assert!(!book.is_dirty());
        assert!(!book.register_use(1), ":141 an already-used token returns early");
        assert!(!book.is_dirty(), ":146 is under the early return");
    }

    #[test]
    fn check_or_add_finds_a_recipe_by_content_and_not_by_token() {
        let mut book = RecipeBook::new();
        assert_eq!(book.add(storage(1, "stick")), 1);
        // Same body, another token: Java's inverse lookup answers the first one.
        assert_eq!(book.check_or_add(storage(2, "stick")), 1, ":81 getRecipeId");
        assert_eq!(book.len(), 1, ":86 a hit adds nothing");
        assert_eq!(book.check_or_add(storage(2, "plank")), 2, ":83 a miss adds");
        assert_eq!(book.len(), 2);
    }

    #[test]
    fn a_second_recipe_under_one_token_replaces_the_first_as_a_bimap_does() {
        let mut book = RecipeBook::new();
        book.add(storage(1, "stick"));
        book.add(storage(1, "plank"));
        assert_eq!(book.len(), 1, ":71 put on an existing key");
        assert!(book.get_id(&body("stick")).is_none(), "the old body's inverse went too");
        assert_eq!(book.get_id(&body("plank")), Some(1));
    }

    #[test]
    fn the_snapshot_is_a_snapshot_until_something_changes() {
        let mut book = RecipeBook::new();
        book.add(storage(1, "stick"));
        assert_eq!(book.snapshot().len(), 1);
        book.add(storage(2, "plank"));
        assert_eq!(book.snapshot().len(), 2, ":73 addRecipe drops the cache");
        book.read(&Compound::new());
        assert_eq!(book.snapshot().len(), 2, ":127 read drops it too");
    }

    #[test]
    fn a_saved_recipe_round_trips_with_its_token_where_java_puts_it() {
        let mut book = RecipeBook::new();
        book.add(storage(3, "stick"));
        let written = book.write();
        let entries = nbt::compounds(&written, TAG_RECIPES);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].get(&TAG_TOKEN.into()), Some(&token_tag(3)), ":135");
        let mut read = RecipeBook::new();
        assert!(read.read(&written).is_empty());
        assert_eq!(read.get(3), book.get(3), "the body survives the trip");
    }

    #[test]
    fn read_refuses_a_recipe_without_a_token_a_body_or_an_output() {
        let mut no_token = body("stick");
        no_token.remove(&TAG_TOKEN.into());
        let mut no_input = body("stick");
        no_input.remove(&KEY_INPUT.into());
        let mut no_output = body("stick");
        no_output.remove(&KEY_ITEM.into());

        let saved = Compound::from([(
            TAG_RECIPES.into(),
            Tag::List(vec![
                Tag::Compound(no_token),
                Tag::Compound(with_token(1, no_input)),
                Tag::Compound(with_token(2, no_output)),
                Tag::Int(4),
            ]),
        )]);
        let mut book = RecipeBook::new();
        let skipped = book.read(&saved);
        assert!(book.is_empty());
        assert_eq!(
            skipped,
            vec![
                Skip { index: 0, reason: SkipReason::NoToken },
                Skip { index: 1, reason: SkipReason::NoInput },
                Skip { index: 2, reason: SkipReason::NoOutput },
                Skip { index: 3, reason: SkipReason::NotARecipe },
            ],
            ":115 has three terms and each one is a name here"
        );
    }

    #[test]
    fn a_loot_table_or_an_alternate_output_is_enough_when_the_primary_is_empty() {
        let mut loot = body("stick");
        loot.remove(&KEY_ITEM.into());
        loot.insert(KEY_LOOT.into(), Tag::String("minecraft:chests/simple".into()));
        let mut alternate = body("stick");
        alternate.remove(&KEY_ITEM.into());
        alternate.insert(
            KEY_ALTERNATE.into(),
            Tag::List(vec![Tag::Compound(Compound::from([(
                KEY_ITEM.into(),
                Tag::String("minecraft:plank".into()),
            )]))]),
        );
        let saved = Compound::from([(
            TAG_RECIPES.into(),
            nbt::compound_list(vec![with_token(1, loot), with_token(2, alternate)]),
        )]);
        let mut book = RecipeBook::new();
        assert!(book.read(&saved).is_empty());
        assert_eq!(book.len(), 2, ":115 the disjunction's other two arms");
    }

    #[test]
    fn a_second_copy_of_a_recipe_the_book_holds_is_dropped() {
        let saved = Compound::from([(
            TAG_RECIPES.into(),
            nbt::compound_list(vec![with_token(1, body("stick")), with_token(2, body("stick"))]),
        )]);
        let mut book = RecipeBook::new();
        let skipped = book.read(&saved);
        assert_eq!(book.len(), 1);
        assert_eq!(skipped, vec![Skip { index: 1, reason: SkipReason::Duplicate { token: 1 } }]);
    }

    #[test]
    fn a_read_then_a_write_puts_the_dropped_entries_back_as_java_does() {
        let mut no_output = body("nothing");
        no_output.remove(&KEY_ITEM.into());
        let saved = Compound::from([(
            TAG_RECIPES.into(),
            nbt::compound_list(vec![with_token(1, body("stick")), with_token(2, no_output)]),
        )]);
        let mut book = RecipeBook::new();
        assert_eq!(book.read(&saved).len(), 1);
        assert_eq!(book.len(), 1);
        // Nothing changed, so the tag cache is handed back as it was read (:128).
        let written = book.write();
        assert_eq!(nbt::compounds(&written, TAG_RECIPES).len(), 2, ":99-104 the cache");
    }

    #[test]
    fn reset_empties_the_book_and_the_next_write_is_empty() {
        let mut book = RecipeBook::new();
        book.add(storage(1, "stick"));
        book.write();
        book.reset();
        assert!(book.is_empty(), ":134");
        assert!(book.write().get(&TAG_RECIPES.into()).is_some());
        assert!(
            nbt::compounds(&book.write(), TAG_RECIPES).is_empty(),
            "the port clears the tag cache with the recipes (module note)"
        );
    }

    #[test]
    fn every_skip_has_its_own_name() {
        let names = [
            SkipReason::NotARecipe.name(),
            SkipReason::NoToken.name(),
            SkipReason::Duplicate { token: 1 }.name(),
            SkipReason::NoInput.name(),
            SkipReason::NoOutput.name(),
        ];
        for (index, name) in names.iter().enumerate() {
            for other in &names[index + 1..] {
                assert_ne!(name, other, "two names collide, so a log line lies");
            }
            assert!(name.starts_with("recipes."), "{name} is not in this namespace");
        }
    }

    fn with_token(token: Token, body: Compound) -> Compound {
        let mut tag = Compound::from([(TAG_TOKEN.into(), token_tag(token))]);
        for (key, value) in body {
            tag.insert(key, value);
        }
        tag
    }
}
