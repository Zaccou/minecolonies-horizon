//! The datagen loot-table cache: one read per table, on demand, and a table that
//! cannot be read is **empty and not remembered**.
//!
//! Java reference: `core/generation/DatagenLootTableManager.java` (71 lines), a
//! `LootDataManager` subclass that overrides `getLootTable` with a lazy map. Its
//! two callers are datagen providers (`DefaultSifterCraftingProvider.java:47`,
//! `DefaultNetherWorkerLootProvider.java:50`), both feeding
//! `LootTableAnalyzer` (`core/colony/crafting/LootTableAnalyzer.java`).
//!
//! **This manager exists only for datagen, and the port says so rather than
//! pretending otherwise.** `GatherDataHandler.java:24` constructs it once and
//! hands it to two providers; nothing in the shipped mod reads a loot table at
//! runtime. Two consequences the port keeps:
//!
//! - **The loaded object is a `LootTable`, which the port has no parser for.**
//!   `ForgeHooks.loadLootTable` (`:56`) is vanilla's codec, and
//!   `LootTableAnalyzer.toDrops` (`manager_02/MANQUANT.md` §5) is the analysis
//!   that would consume it. What [`Tables`] therefore returns is the **JSON
//!   document** of the table file, which is the honest half: the cache, the path
//!   it looks a table up at, and the four answers Java gives.
//! - **No datagen in a wasm mod.** Neither the provider nor the writer of the
//!   file exists here, so [`Source`] is a parameter. The rules are testable with
//!   a source that answers from a map, which is exactly what a test needs.
//!
//! **Three rules of `getLootTable`, and the third is the surprising one.**
//!
//! 1. **The cache is consulted first and only by id** (`:45-46`): the same
//!    `ResourceLocation` twice is one read.
//! 2. **A table is read at a fixed place**: `PackType.SERVER_DATA` with the
//!    suffix `.json` and the directory `loot_tables` (`:50`), i.e.
//!    `data/<namespace>/loot_tables/<path>.json`. The file must parse as a JSON
//!    **object** (`:55`, `JsonObject.class`).
//! 3. **A failure is not cached.** Java's `catch (Throwable e)` (`:64-67`) prints
//!    and falls through to `LootTable.EMPTY` (`:69`) *without* the `tables.put`
//!    of `:59`, so the next call reads the file again. A miss costs a read every
//!    time; only a success is remembered.
//!
//! **The port refuses where Java returns `EMPTY`.** Java's answer keeps no
//! information at all — the only trace is `e.printStackTrace()` — while a
//! refusal can carry which id failed and why. [`Tables::get`] is that refusal,
//! [`Tables::or_empty`] is Java's `LootTable.EMPTY` for a caller that wants it,
//! and neither caches the failure.

use std::collections::{btree_map::Entry, BTreeMap};

use mod_api::serde_json::Value;
use mod_api::Identifier;

/// The directory `ExistingFileHelper.getResource(..)` is asked for
/// (`DatagenLootTableManager.java:50`).
pub const DIRECTORY: &str = "loot_tables";

/// The file suffix of the same call (`:50`).
pub const SUFFIX: &str = ".json";

/// The pack type of the same call (`:50`), spelled the way the helper's
/// `PackType` enum does: server data is `data/`, not `assets/`.
pub const PACK: &str = "server_data";

/// Where the table for `id` lives inside the pack: Java asks
/// `existingFileHelper.getResource(location, SERVER_DATA, ".json", "loot_tables")`,
/// which resolves to `data/<namespace>/loot_tables/<path>.json`.
pub fn path(id: &Identifier) -> String {
    format!("{}:{DIRECTORY}/{}{SUFFIX}", id.namespace(), id.path())
}

/// What a read of one file answers. Java's `Throwable` (`:64`) has no shape; a
/// reason string is all a caller can act on, and it is what
/// `e.printStackTrace()` would have printed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unreadable {
    pub reason: String,
}

impl Unreadable {
    pub fn new(reason: &str) -> Unreadable {
        Unreadable {
            reason: reason.to_owned(),
        }
    }
}

/// Where a table comes from. Java's `ExistingFileHelper`; a parameter here
/// because a wasm mod has no resource pack to read.
pub trait Source {
    fn read(&mut self, path: &str) -> Result<Value, Unreadable>;
}

/// Why a table could not be handed out.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the three ways `getLootTable` fails
pub enum Refusal {
    /// The file is absent, or does not parse (`:64`).
    Unreadable { id: Identifier, reason: String },
    /// The file parses and is not a JSON object (`:55` asks for
    /// `JsonObject.class`, and `GsonHelper` throws for anything else).
    NotAnObject { id: Identifier },
}

/// The cache itself, Java's `tables` map (`:33`) plus the source.
#[derive(Clone, Debug)]
pub struct Tables<S> {
    source: S,
    tables: BTreeMap<Identifier, Value>,
    misses: u32,
}

impl<S: Source> Tables<S> {
    /// A cache over `source`. Java's constructor also calls `super()`
    /// (`:37`) — "in theory we should load these too; in practice vanilla doesn't
    /// seem to use it" — and the port has no superclass to call.
    pub fn new(source: S) -> Tables<S> {
        Tables {
            source,
            tables: BTreeMap::new(),
            misses: 0,
        }
    }

    /// `getLootTable` (`:41-70`): the cache, then one read, then the cache again.
    ///
    /// The read's place in the sequence is the rule: a hit never touches the
    /// source, and a failure never reaches the map.
    pub fn get(&mut self, id: &Identifier) -> Result<&Value, Refusal> {
        // `entry` rather than `get` then `insert`: the vacant slot is the one
        // proof that the read happens on a miss and never on a hit, and it is
        // what lets the returned borrow outlive the branch without a second
        // lookup.
        match self.tables.entry(id.clone()) {
            Entry::Occupied(slot) => Ok(slot.into_mut()),
            Entry::Vacant(slot) => {
                let read = self.source.read(&path(id));
                let value = match read {
                    Ok(value) => value,
                    Err(error) => {
                        // `catch (Throwable e) { e.printStackTrace(); }` (`:64-67`) — no
                        // `tables.put`, which is the whole point.
                        self.misses += 1;
                        return Err(Refusal::Unreadable {
                            id: id.clone(),
                            reason: error.reason,
                        });
                    }
                };
                if !matches!(value, Value::Object(_)) {
                    self.misses += 1;
                    return Err(Refusal::NotAnObject { id: id.clone() });
                }
                // `this.tables.put(location, loottable)` (`:59`), after the parse.
                Ok(slot.insert(value))
            }
        }
    }

    /// Java's actual contract: `LootTable.EMPTY` for every failure (`:69`).
    ///
    /// The empty table is an empty JSON object, which is the closest thing this
    /// port has to `LootTable.EMPTY` and is *not* cached either — the next call
    /// reads again, as Java's does.
    pub fn or_empty(&mut self, id: &Identifier) -> Value {
        match self.get(id) {
            Ok(table) => table.clone(),
            Err(_) => Value::Object(Default::default()),
        }
    }

    /// Whether a table is remembered — the fact rule 3 is about.
    pub fn cached(&self, id: &Identifier) -> bool {
        self.tables.contains_key(id)
    }

    /// How many tables were read successfully.
    pub fn len(&self) -> usize {
        self.tables.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tables.is_empty()
    }

    /// How many reads failed, cached or not.
    pub fn misses(&self) -> u32 {
        self.misses
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(text: &str) -> Identifier {
        Identifier::parse(text).expect("a literal is a valid id")
    }

    /// A pack of files that records what was asked for.
    #[derive(Default)]
    struct Files {
        files: BTreeMap<String, Value>,
        asked: Vec<String>,
    }

    impl Files {
        /// A pack holding one **JSON object** per name: `getLootTable` parses to
        /// `JsonObject.class` (`:55`), so a table file that is not an object can
        /// never be a table and would test the wrong refusal.
        fn with(names: &[&str]) -> Files {
            let mut files = Files::default();
            for name in names {
                files
                    .files
                    .insert((*name).to_owned(), Value::Object(Default::default()));
            }
            files
        }
    }

    impl Source for Files {
        fn read(&mut self, path: &str) -> Result<Value, Unreadable> {
            self.asked.push(path.to_owned());
            match self.files.get(path) {
                Some(value) => Ok(value.clone()),
                None => Err(Unreadable::new("the pack has no such file")),
            }
        }
    }

    #[test]
    fn a_hit_is_read_once_and_the_path_is_javas_own() {
        let mut tables = Tables::new(Files::with(&["minecraft:loot_tables/blocks/oak_log.json"]));
        let table = id("minecraft:blocks/oak_log");
        assert!(tables.get(&table).is_ok());
        assert!(tables.get(&table).is_ok());
        assert_eq!(
            tables.source.asked,
            ["minecraft:loot_tables/blocks/oak_log.json".to_owned()],
            "the second call is the cache (`:45-46`)"
        );
        assert_eq!(tables.len(), 1);
        assert_eq!(tables.misses(), 0);
    }

    #[test]
    fn a_missing_file_is_refused_and_never_cached() {
        let mut tables = Tables::new(Files::with(&[]));
        let table = id("minecraft:blocks/oak_log");
        assert_eq!(
            tables.get(&table),
            Err(Refusal::Unreadable {
                id: table.clone(),
                reason: "the pack has no such file".to_owned()
            })
        );
        assert!(!tables.cached(&table), "the `put` of `:59` is not reached");
        assert!(tables.get(&table).is_err());
        assert_eq!(
            tables.source.asked.len(),
            2,
            "a failure costs a read every time (`:64-69`)"
        );
        assert_eq!((tables.len(), tables.misses()), (0, 2));
    }

    #[test]
    fn java_answers_empty_for_every_failure_and_the_cache_still_knows_nothing() {
        let mut tables = Tables::new(Files::with(&[]));
        let table = id("a:b");
        assert_eq!(tables.or_empty(&table), Value::Object(Default::default()));
        assert!(!tables.cached(&table));
    }

    #[test]
    fn a_file_that_is_not_an_object_is_refused_rather_than_cached() {
        let mut files = Files::default();
        files
            .files
            .insert("a:loot_tables/b.json".to_owned(), Value::Bool(true));
        let mut tables = Tables::new(files);
        let table = id("a:b");
        assert_eq!(
            tables.get(&table),
            Err(Refusal::NotAnObject { id: table.clone() })
        );
        assert!(!tables.cached(&table));
    }

    #[test]
    fn the_namespace_carries_into_the_path_and_the_directory_is_the_constant() {
        assert_eq!(path(&id("minecolonies:blocks/gravel")), "minecolonies:loot_tables/blocks/gravel.json");
        assert!(path(&id("a:b")).contains(DIRECTORY));
        assert!(path(&id("a:b")).ends_with(SUFFIX));
        assert_eq!(PACK, "server_data");
    }
}
