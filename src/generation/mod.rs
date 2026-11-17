//! The data generators: what the mod **writes** into its own data pack.
//!
//! Java reference: `core/generation/` — 49 files, 9 872 lines, 40 of them under
//! `defaults/`. A generator is not a table read at runtime: it is code that runs
//! once at build time and *emits* the recipes, the loot tables and the tags the
//! game loads. This module ports that code, and each of its files answers one
//! family.
//!
//! | File | Question | Java |
//! | --- | --- | --- |
//! | [`constants`] | where does a table file go, and what is a variant's id? | `DataGeneratorConstants.java`, `DefaultRecipeProvider.java:264` |
//! | [`recipes`] | which crafting recipe does each hut cost, and how does a dough cook? | `defaults/DefaultRecipeProvider.java` (1 108) |
//! | [`loot`] | what does a block, a mob, a crate or a fishing trip drop? | `defaults/Default*LootProvider.java` (712) |
//! | [`workers`] | what shape does a worker's own recipe take? | `CustomRecipeProvider.java`, `workers/` |
//! | [`tags`] | which tag does the pack declare, and what makes an entry optional? | `defaults/Default*TagsProvider.java` (879) |
//! | [`nbt`] | which NBT keys make two stacks different? | `ItemNbtCalculator.java` (253) |
//!
//! # The oracle, and why it is two oracles
//!
//! "Port the generator, not its output" (`AGENTS.md`) has a consequence here: the
//! only way to know a rule was read right is to run it against what the generator
//! already produced. Two copies of that output exist in this repository, and they
//! are **not the same generation**:
//!
//! | Copy | Path | Shape |
//! | --- | --- | --- |
//! | the delivered content | `data/minecolonies/{recipe,loot_table,tags}` — 154 + 146 + 98 | 1.21: `recipe/`, `loot_table/`, `random_sequence` |
//! | the datagen output | `minecolonies-src/src/datagen/generated/…` — 158 + 161 + 146 + 98 | 1.20.1: `recipes/`, `loot_tables/`, a `type` on every table |
//!
//! Every comparison this module's tests do is against **the delivered content**.
//! One measured exception is in [`workers`]: the nether worker's five
//! crafterrecipes in the datagen output list raw mob drops while the loot table
//! next to them already rolls `minecolonies:adventure_token` — the recipes are
//! older than the loot. A port that trusted the older copy would deliver a
//! recipe whose `additional-output` disagrees with the table it points at.
//!
//! # What is generated, and what is not
//!
//! [`Pack::build`] emits **287** files and **refuses five**, by name (see
//! [`Refusal`]). Counted on the delivered pack:
//!
//! | Family | Emitted | Delivered | Why the rest waits |
//! | --- | --- | --- | --- |
//! | `loot_table/` | 141 | 146 | 141 of the delivered ones; the five enchanter tables are hand-written lists (`DefaultEnchanterCraftingProvider.java:51-435`) |
//! | `minecraft/` | 35 | 7 | 32 banner tables the delivered pack does not carry, 3 damage tags it does |
//! | `recipe/` | 76 | 154 | the 78 others are one entry per item in `buildOtherItems`/`buildFood` (`:367-1103`): content, not a rule |
//! | `crafterrecipes/` | 26 | 0 | the delivered pack has none; the 161 of the datagen output are content too, but the 26 that derive from a loot table are here |
//! | `tags/` | 12 | 98 + 7 | the 89 item and block tags are the crafter filters (`DefaultItemTagsProvider.java:47-596`) |
//!
//! **What the module does not do.** It does not write a byte to disk: [`File`]
//! is a path and a value, and who writes them is the pack build, which this
//! module does not own. It does not read `data/` either — the tests do, and only
//! the tests. And it does not re-derive a fact another module already holds: the
//! 86 blocks, their families and which crop grows on which farmland come from
//! [`crate::blocks`], the item ids are checked against [`crate::items`], and the
//! three compost strengths are [`crate::crafting::compost`]'s.

pub mod constants;
pub mod json;
pub mod loot;
pub mod nbt;
pub mod recipes;
pub mod tags;
pub mod workers;

use json::Json;

/// One file the generator writes: where it goes, relative to `data/`, and what
/// it contains.
#[derive(Clone, Debug)]
pub struct File {
    pub path: String,
    pub json: Json,
}

/// A rule that produced nothing, and the reason — never a silent skip.
///
/// The three blocks refused today are refused by *name*: a grave and the
/// decoration controller carry no loot table in the delivered pack, and a
/// generator that invented one would change what a broken grave drops.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// No loot table for this block, because Java's provider never registers one.
    NoLootTable {
        block: &'static str,
        why: &'static str,
        java: &'static str,
    },
    /// A family that exists in Java but whose rows are hand-written content.
    HandWritten {
        family: &'static str,
        files: usize,
        java: &'static str,
    },
    /// A rule that is ported but whose input list this port does not have.
    NoSource {
        what: &'static str,
        why: &'static str,
        java: &'static str,
    },
}

impl Refusal {
    /// The sentence a report prints.
    pub fn sentence(&self) -> String {
        match self {
            Refusal::NoLootTable { block, why, java } => {
                format!("no loot table for {block}: {why} ({java})")
            }
            Refusal::HandWritten { family, files, java } => {
                format!("{files} {family} files are content, not a rule ({java})")
            }
            Refusal::NoSource { what, why, java } => {
                format!("no {what}: {why} ({java})")
            }
        }
    }
}

/// Everything one run of the generators produces.
#[derive(Clone, Debug, Default)]
pub struct Pack {
    pub files: Vec<File>,
    /// The rules that produced nothing, so the count of missing files is
    /// explained rather than noticed later.
    pub refused: Vec<Refusal>,
}

impl Pack {
    /// Runs every generator, in the order a datagen run does: the recipes and
    /// the loot tables share one pass in Java (`CustomRecipeAndLootTableProvider`
    /// runs both children in one future), the tags come after, and the NBT
    /// calculator last, because it reads the creative tabs the item registry
    /// filled.
    pub fn build() -> Pack {
        let mut pack = Pack::default();

        // The 86 blocks drive the block loot, so the two refusals are collected
        // here rather than inside `loot`: the caller owns the "what is missing"
        // list, exactly as `LootTableProvider` owns the map it fills.
        for block in crate::blocks::all() {
            match loot::block_table(block) {
                Ok(file) => pack.files.push(file),
                Err(refusal) => pack.refused.push(refusal),
            }
        }

        pack.files.extend(loot::chest_tables());
        pack.files.extend(loot::lucky_ore_tables());
        pack.files.extend(loot::fisherman_tables());
        pack.files.extend(loot::entity_tables());
        pack.files.extend(loot::recipe_loot_tables());
        pack.files.extend(loot::sifter_tables());
        pack.files.extend(loot::nether_worker_tables());
        pack.files.extend(loot::vanilla_banner_tables());

        pack.files.extend(recipes::hut_recipes());
        pack.files.extend(recipes::cooking_recipes());

        pack.files.extend(workers::sifter_recipes());
        pack.files.extend(workers::nether_worker_recipes());
        pack.files.extend(workers::mesh_recipes());

        pack.files.extend(tags::biome_tags());
        pack.files.extend(tags::entity_type_tags());
        pack.files.extend(tags::damage_tags());

        pack.refused.push(nbt::refusal());

        pack.refused.push(loot::enchanter_refusal());

        pack
    }

    /// The file at this path, if the generator wrote one.
    pub fn get(&self, path: &str) -> Option<&Json> {
        self.files
            .iter()
            .find(|file| file.path == path)
            .map(|file| &file.json)
    }

    /// The files under a prefix, e.g. `minecolonies/loot_table/blocks/`.
    pub fn under(&self, prefix: &str) -> Vec<&File> {
        self.files
            .iter()
            .filter(|file| file.path.starts_with(prefix))
            .collect()
    }

    /// The number of files written.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Whether nothing at all was written — never true for a build, and present
    /// so `len` has a companion.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// The five measured counts of the delivered pack, so a test can say what a
/// generator is missing instead of guessing.
pub const DELIVERED: [(&str, usize); 5] = [
    ("minecolonies/recipe/", 154),
    ("minecolonies/loot_table/", 146),
    ("minecolonies/tags/", 98),
    ("minecraft/tags/", 7),
    ("compatibility/", 0),
];

/// A delivered file, parsed — tests only. Refuses a missing or malformed file by
/// name rather than falling back to an empty value: a comparison against a
/// silently-empty oracle proves nothing.
#[cfg(test)]
pub(crate) fn delivered(path: &str) -> Json {
    let full = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join(path);
    let bytes = std::fs::read(&full).unwrap_or_else(|error| {
        panic!("delivered file {}: {error}", full.display());
    });
    mod_api::serde_json::from_slice(&bytes).unwrap_or_else(|error| {
        panic!("delivered file {} is not JSON: {error}", full.display());
    })
}

/// The `values` of a delivered tag file, as strings — tests only.
#[cfg(test)]
pub(crate) fn delivered_values(path: &str) -> Vec<String> {
    match &delivered(path)["values"] {
        Json::Array(items) => items
            .iter()
            .filter_map(|value| value.as_str().map(|id| id.to_owned()))
            .collect(),
        other => panic!("{path}: values is not an array: {other}"),
    }
}

/// Compares one generated file with the delivered copy, and prints both on a
/// mismatch — a bare `assert_eq!` on two 4 KB values is unreadable.
#[cfg(test)]
pub(crate) fn matches_delivered(pack: &Pack, path: &str) {
    let generated = pack
        .get(path)
        .unwrap_or_else(|| panic!("the generator wrote no {path}"));
    let shipped = delivered(path);
    assert_eq!(
        generated,
        &shipped,
        "{path}: the generator and the pack disagree\n  generated: {generated}\n  delivered: {shipped}"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_build_writes_the_287_files_the_module_doc_counts() {
        // A move in any family shows up here before it shows up in a report:
        // 141 loot tables, 32 banner tables, 76 recipes, 26 crafter recipes and
        // 12 tags.
        let pack = Pack::build();
        assert_eq!(pack.len(), 287, "{} files", pack.len());
        assert_eq!(pack.under("minecolonies/loot_table/").len(), 141);
        assert_eq!(pack.under("minecraft/loot_table/").len(), 32);
        assert_eq!(pack.under("minecolonies/crafterrecipes/").len(), 26);
        assert_eq!(pack.under("minecolonies/compatibility/").len(), 0);
    }

    #[test]
    fn the_build_is_not_empty_and_refuses_by_name() {
        let pack = Pack::build();
        assert!(pack.len() > 200, "{} files", pack.len());
        assert_eq!(
            pack.refused.len(),
            5,
            "three blocks, one family and one missing source: {:?}",
            pack.refused
        );
        for refusal in &pack.refused {
            assert!(
                refusal.sentence().contains(':'),
                "a refusal says why: {}",
                refusal.sentence()
            );
        }
    }

    #[test]
    fn the_three_blocks_without_a_loot_table_are_the_three_the_pack_has_none_for() {
        let pack = Pack::build();
        let shipped = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data/minecolonies/loot_table/blocks");
        let mut missing = Vec::new();
        for block in crate::blocks::all() {
            let name = json::path_of(&block.name).to_owned();
            if pack
                .get(&constants::loot_path(&json::own(&format!("blocks/{name}"))))
                .is_none()
            {
                missing.push(name.clone());
            }
            assert_eq!(
                shipped.join(format!("{name}.json")).exists(),
                pack.get(&constants::loot_path(&json::own(&format!("blocks/{name}"))))
                    .is_some(),
                "{name}: the pack and the generator must agree on whether there is a table"
            );
        }
        assert_eq!(
            missing,
            vec![
                "decorationcontroller".to_owned(),
                "blockminecoloniesgrave".to_owned(),
                "blockminecoloniesnamedgrave".to_owned()
            ],
            "in `crate::blocks::all()` order: chantier, then decoration"
        );
    }

    #[test]
    fn every_written_path_is_unique() {
        // `saveStable` refuses to write the same path twice; a duplicate here is
        // two providers claiming one file.
        let pack = Pack::build();
        let mut seen: Vec<&str> = pack.files.iter().map(|file| file.path.as_str()).collect();
        seen.sort_unstable();
        let before = seen.len();
        seen.dedup();
        assert_eq!(before, seen.len(), "a path is written twice");
    }

    #[test]
    fn every_written_path_names_a_family_and_a_json_file() {
        let pack = Pack::build();
        for file in &pack.files {
            assert!(
                file.path.ends_with(".json"),
                "{} is not a .json path",
                file.path
            );
            assert!(
                file.path.starts_with("minecolonies/") || file.path.starts_with("minecraft/"),
                "{} leaves the two namespaces the generator writes",
                file.path
            );
            assert!(
                file.json.is_object() || file.json.is_array(),
                "{} is neither an object nor an array",
                file.path
            );
        }
    }

    #[test]
    fn the_delivered_counts_are_the_five_the_pack_was_measured_with() {
        // If a family count moves, the coverage table in the module doc is wrong
        // and this test says so before the report does.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        for (prefix, expected) in DELIVERED {
            let mut count = 0;
            walk(&root.join(prefix), &mut count);
            assert_eq!(count, expected, "{prefix}");
        }
    }

    fn walk(dir: &std::path::Path, count: &mut usize) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, count);
                } else if path.extension().map(|ext| ext == "json").unwrap_or(false) {
                    *count += 1;
                }
            }
        }
    }

    #[test]
    fn a_prefix_selects_a_family_and_nothing_else() {
        let pack = Pack::build();
        let blocks = pack.under("minecolonies/loot_table/blocks/");
        assert_eq!(blocks.len(), 83);
        assert!(blocks.iter().all(|file| file.path.ends_with(".json")));
        assert_eq!(pack.under("minecolonies/recipe/").len(), 76);
        assert_eq!(pack.under("minecolonies/crafterrecipes/").len(), 26);
    }
}
