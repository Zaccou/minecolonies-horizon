//! The renames this content needs, applied to the registry each id belongs to.
//!
//! # One rename, two surfaces
//!
//! A blueprint palette names **blocks**; a recipe names **items**. Both ids that
//! changed between 1.20.1 and 26.3 changed in both registries, and neither old
//! name is left in either one. The table itself is not written here:
//! [`crate::compat::table::RENAMES`] holds it with the version and the page that
//! prove each row, and [`crate::compat::resolve`] is how it is asked. What this
//! module adds is what a *content* reader needs on top — which registry the id is
//! read from, what that registry says, and how much shipped content depends on it:
//!
//! | Question | Answer | Proof |
//! | --- | --- | --- |
//! | what does this id become? | [`apply`] | the compat row |
//! | in which registry? | [`Target`] | block and item exports of the pinned client |
//! | how much content depends on it? | [`RECIPE_USES`], [`MEASURED_RECIPES`] | the 154 shipped recipe files |
//!
//! # The item side is measured, not assumed
//!
//! `minecraft:grass` and `minecraft:chain` are the names of a block **and** of an
//! item, and a recipe reads the item. That the same rename holds on the item side
//! is a claim about the item registry, so it is checked against it:
//! `minecraft:short_grass` and `minecraft:iron_chain` are keys of
//! `["minecraft:item"]["entries"]` (1 658 keys) and the two 1.20.1 names are not,
//! exactly as on the block side. The test that reads the export re-derives both,
//! and skips when the file is not on the machine rather than pretending.
//!
//! # The recipe pack's own measurement
//!
//! Walking the 154 files of `data/minecolonies/recipe/` and counting every `item`
//! / `tag` value gives **603 references to 233 distinct ids** — 144
//! `minecolonies`, 78 `minecraft`, 2 `structurize`, 9 tags. Of the 78 vanilla
//! items, **exactly two** are names 26.3 does not hold, and they are the two rows
//! of [`RECIPE_USES`]: six references over five files. The other 597 are already
//! 26.3 names or belong to a mod. This count is per pack and is never reused for
//! the blueprint pack, where the same raw walk finds 238 because entity data also
//! names armour.

use crate::compat::{namespace_of, resolve, Resolution, Rename};

/// Which registry an id is read from. The rename is the same on both; the
/// evidence is not, and a reader of a recipe file needs the second.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// `data/<ns>/recipe/*.json`, and every `item` inside it: 26.3's item registry.
    Item,
    /// A blueprint palette, and every cell that indexes it: 26.3's block registry.
    Block,
}

impl Target {
    /// The key of the pinned client's export this target is read from.
    pub fn key(self) -> &'static str {
        match self {
            Self::Item => "minecraft:item",
            Self::Block => "minecraft:block",
        }
    }

    /// The file, and the section of it, that holds this target's 26.3 keys.
    pub fn registry(self) -> &'static str {
        match self {
            Self::Item => REGISTRY_ITEM_26_3,
            Self::Block => crate::compat::REGISTRY_26_3,
        }
    }

    /// How a report names this target, in a sentence.
    pub fn label(self) -> &'static str {
        match self {
            Self::Item => "an item",
            Self::Block => "a block",
        }
    }
}

/// Where the 26.3 side of the item half is written down: the pinned client's
/// exported registry, one key per item under `["minecraft:item"]["entries"]`
/// (1 658 of them). A rename is only applied to an item if its target is a key
/// there and its 1.20.1 name is not.
pub const REGISTRY_ITEM_26_3: &str =
    ".local/horizon-client/26.3/registries.json, [\"minecraft:item\"][\"entries\"] (1 658 keys)";

/// The number of keys of that section, so a changed build shows up as a number.
pub const ITEM_KEYS_26_3: usize = 1_658;

/// FNV-1a 64 ([`crate::blueprint::fnv1a64`]) over those keys sorted ascending and
/// joined by one `\n`, no trailing newline — the same digest recipe
/// [`crate::compat::Measured::registry_digest`] uses for the block side.
pub const ITEM_DIGEST_26_3: u64 = 0x2f63_516e_0e39_2bd8;

/// One palette id, the 26.3 name to use instead, and the registry that says so.
///
/// The resolution is [`crate::compat::Resolution`] rather than a copy of it, so
/// the table stays the only place a rename is written down: this type adds the
/// target and the evidence, never a second opinion about what an id becomes.
#[derive(Clone, Copy, Debug)]
pub struct Application<'a> {
    pub target: Target,
    pub resolution: Resolution<'a>,
    /// [`Target::registry`] for the target this was asked for.
    pub registry: &'static str,
}

impl Application<'_> {
    /// The 26.3 name to write, when the table renames this id.
    pub fn target_id(&self) -> Option<&'static str> {
        self.resolution.target()
    }

    /// True exactly when the id has to be rewritten. A `Gone` or `Unproven` row
    /// is a decision for a human, not a rewrite, and answers `false`.
    pub fn needs_replace(&self) -> bool {
        matches!(self.resolution, Resolution::Renamed(_))
    }

    /// The compat row behind this answer, if it came from one.
    pub fn row(&self) -> Option<&'static Rename> {
        self.resolution.row()
    }
}

/// What the table says about `id`, read as a `target`.
pub fn apply(target: Target, id: &str) -> Application<'_> {
    Application {
        target,
        resolution: resolve(id),
        registry: target.registry(),
    }
}

/// The 26.3 name of a 1.20.1 id read as `target`, when the table renames it.
/// `None` for an id already 26.3, a mod's id, and an id with no proven name.
pub fn renamed(target: Target, id: &str) -> Option<&'static str> {
    apply(target, id).target_id()
}

/// Whether this id belongs to a mod's namespace: content a mod must bring, never
/// a name 26.3 lost.
pub fn is_foreign(id: &str) -> bool {
    namespace_of(id) != "minecraft"
}

/// The compat row for `id`, without asking which surface it was read from —
/// for a report that has already decided.
pub fn row(id: &str) -> Option<&'static Rename> {
    crate::compat::table::find(id)
}

/// How much of the recipe pack one rename touches.
#[derive(Clone, Copy, Debug)]
pub struct RecipeUse {
    /// The 1.20.1 id, as the recipe files write it.
    pub id: &'static str,
    /// `item` values naming it, counts included: 2 in one file is two references.
    pub references: u32,
    /// Files naming it at least once.
    pub files: u32,
    /// Where those references are, so a reader can check without a scan.
    pub sites: &'static str,
}

/// The renames the recipe pack needs — the whole of it.
///
/// Both rows are **ingredients**, never results: a result is what the recipe
/// makes, and none of the 154 files makes either of these two. That matters,
/// because a rename on a result would change what a player receives.
pub const RECIPE_USES: &[RecipeUse] = &[
    RecipeUse {
        id: "minecraft:grass",
        references: 2,
        files: 1,
        sites: "data/minecolonies/recipe/doublegrass.json, both `ingredients[]` entries; the \
                result is `minecraft:tall_grass`, which kept its name",
    },
    RecipeUse {
        id: "minecraft:chain",
        references: 4,
        files: 4,
        sites: "data/minecolonies/recipe/chainmail{boots,chestplate,helmet,leggings}.json, \
                `key.C` of each",
    },
];

/// The recipe pack as measured, as numbers a test re-derives from the files.
#[derive(Clone, Copy, Debug)]
pub struct MeasuredRecipes {
    /// `data/minecolonies/recipe/*.json` — the four excluded files are not here.
    pub files: usize,
    /// `item` and `tag` values counted, duplicates included.
    pub references: usize,
    /// Distinct ids, tags written with a leading `#`.
    pub distinct: usize,
    pub minecraft: usize,
    pub minecolonies: usize,
    pub structurize: usize,
    pub tags: usize,
    /// FNV-1a 64 over the distinct ids sorted and joined by `\n`.
    pub digest: u64,
}

pub const MEASURED_RECIPES: MeasuredRecipes = MeasuredRecipes {
    files: 154,
    references: 603,
    distinct: 233,
    minecraft: 78,
    minecolonies: 144,
    structurize: 2,
    tags: 9,
    digest: 0x4e95_79ee_aa55_4b84,
};

/// The row of [`RECIPE_USES`] for `id`, if the pack uses this 1.20.1 name.
pub fn uses_of(id: &str) -> Option<&'static RecipeUse> {
    RECIPE_USES.iter().find(|row| row.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blueprint::fnv1a64;
    use mod_api::serde_json::Value;
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::{Path, PathBuf};

    const PACK: &str = "data/minecolonies/recipe";

    fn recipe_paths() -> Vec<PathBuf> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(PACK);
        let entries = std::fs::read_dir(&dir)
            .unwrap_or_else(|error| panic!("recipe pack {}: {error}", dir.display()));
        let mut out: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.to_string_lossy().ends_with(".json"))
            .collect();
        out.sort();
        out
    }

    /// Every `item` and `tag` value of one file, recursively — the form the
    /// loader reads them in, and the form a rename on an item has to reach.
    fn collect_refs(value: &Value, out: &mut BTreeMap<String, usize>) {
        match value {
            Value::Object(object) => {
                if let Some(item) = object.get("item").and_then(Value::as_str) {
                    *out.entry(item.to_owned()).or_default() += 1;
                }
                if let Some(tag) = object.get("tag").and_then(Value::as_str) {
                    *out.entry(format!("#{tag}")).or_default() += 1;
                }
                for nested in object.values() {
                    collect_refs(nested, out);
                }
            }
            Value::Array(items) => items.iter().for_each(|item| collect_refs(item, out)),
            _ => {}
        }
    }

    /// The keys of one registry section of the pinned client, when the build tree
    /// is on this machine. An export of the game is not content this mod ships, so
    /// a tree without it skips — never fails.
    fn registry_keys(section: &str) -> Option<BTreeSet<String>> {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Ok(path) = std::env::var("HORIZON_REGISTRIES_26_3") {
            candidates.push(PathBuf::from(path));
        }
        for path in [
            "/home/zaccou/horizon-modapi/.local/horizon-client/26.3/registries.json",
            "/workspace/horizon/.local/horizon-client/26.3/registries.json",
        ] {
            candidates.push(PathBuf::from(path));
        }
        let path = candidates.into_iter().find(|candidate| candidate.is_file())?;
        let text = std::fs::read_to_string(path).ok()?;
        let json: Value = mod_api::serde_json::from_str(&text).ok()?;
        let entries = json.get(section)?.get("entries")?.as_object()?;
        Some(entries.keys().cloned().collect())
    }

    fn digest(ids: &BTreeSet<String>) -> u64 {
        let joined: Vec<&str> = ids.iter().map(String::as_str).collect();
        fnv1a64(joined.join("\n").as_bytes())
    }

    #[test]
    fn the_table_here_is_the_compat_table_and_adds_no_row() {
        let compat: Vec<&str> = crate::compat::RENAMES
            .iter()
            .map(|row| row.from_1_20_1)
            .collect();
        let uses: Vec<&str> = RECIPE_USES.iter().map(|row| row.id).collect();
        assert_eq!(
            compat, uses,
            "a content row the compat table does not prove is an invention"
        );
        for used in RECIPE_USES {
            let proof = row(used.id).unwrap_or_else(|| panic!("{} has no compat row", used.id));
            assert_eq!(proof.to_26_3, renamed(Target::Item, used.id), "{}", used.id);
            assert!(used.sites.contains(".json"), "{} cites no file", used.id);
            assert!(used.references >= used.files, "{}: refs < files", used.id);
            assert!(used.files > 0, "{} is not used at all", used.id);
        }
    }

    #[test]
    fn a_block_and_an_item_share_the_rename_and_cite_their_own_registry() {
        for (id, target) in [
            ("minecraft:grass", "minecraft:short_grass"),
            ("minecraft:chain", "minecraft:iron_chain"),
        ] {
            let block = apply(Target::Block, id);
            let item = apply(Target::Item, id);
            assert_eq!(block.target_id(), Some(target));
            assert_eq!(item.target_id(), Some(target));
            assert_eq!(block.target, Target::Block);
            assert_eq!(item.target, Target::Item);
            assert!(block.needs_replace() && item.needs_replace());
            assert_ne!(block.registry, item.registry);
            assert!(block.registry.contains("[\"minecraft:block\"]"), "{}", block.registry);
            assert!(item.registry.contains("[\"minecraft:item\"]"), "{}", item.registry);
            assert_eq!(block.row(), item.row(), "one row, two surfaces");
        }
        assert_eq!(Target::Block.key(), "minecraft:block");
        assert_eq!(Target::Item.key(), "minecraft:item");
        assert_eq!(Target::Item.label(), "an item");
    }

    #[test]
    fn a_26_3_name_is_never_renamed_back_on_either_surface() {
        for id in [
            "minecraft:short_grass",
            "minecraft:iron_chain",
            "minecraft:grass_block",
            "minecraft:tall_grass",
            "minecraft:chainmail_helmet",
        ] {
            for target in [Target::Block, Target::Item] {
                let application = apply(target, id);
                assert_eq!(application.target_id(), None, "{id}");
                assert!(!application.needs_replace(), "{id}");
                assert_eq!(application.row(), None, "{id}");
                assert_eq!(renamed(target, id), None, "{id}");
            }
        }
    }

    #[test]
    fn an_exact_lookup_never_catches_a_neighbour() {
        for neighbour in [
            "minecraft:chain_command_block",
            "minecraft:chainmail_boots",
            "minecraft:grass_block",
            "minecraft:short_grass",
            "",
        ] {
            assert_eq!(row(neighbour), None, "{neighbour}");
            assert_eq!(renamed(Target::Item, neighbour), None, "{neighbour}");
        }
    }

    #[test]
    fn a_mod_id_is_foreign_and_not_a_rename() {
        for id in [
            "minecolonies:large_water_bottle",
            "structurize:blocksubstitution",
            "domum_ornamentum:shingle",
        ] {
            assert!(is_foreign(id), "{id}");
            assert_eq!(renamed(Target::Item, id), None, "{id}");
            assert!(!apply(Target::Item, id).needs_replace(), "{id}");
        }
        assert!(!is_foreign("minecraft:chain"));
        assert!(!is_foreign("stone"), "a bare id is read as minecraft");
    }

    #[test]
    fn the_recipe_pack_re_derives_its_own_measurement() {
        let paths = recipe_paths();
        let mut refs: BTreeMap<String, usize> = BTreeMap::new();
        for path in &paths {
            let text = std::fs::read_to_string(path)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            let value: Value = mod_api::serde_json::from_str(&text)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            collect_refs(&value, &mut refs);
        }
        assert_eq!(paths.len(), MEASURED_RECIPES.files);
        assert_eq!(refs.values().sum::<usize>(), MEASURED_RECIPES.references);
        assert_eq!(refs.len(), MEASURED_RECIPES.distinct);

        let distinct: BTreeSet<String> = refs.keys().cloned().collect();
        assert_eq!(digest(&distinct), MEASURED_RECIPES.digest);

        let count = |prefix: &str| {
            distinct
                .iter()
                .filter(|id| id.starts_with(prefix))
                .count()
        };
        assert_eq!(count("minecraft:"), MEASURED_RECIPES.minecraft);
        assert_eq!(count("minecolonies:"), MEASURED_RECIPES.minecolonies);
        assert_eq!(count("structurize:"), MEASURED_RECIPES.structurize);
        assert_eq!(count("#"), MEASURED_RECIPES.tags);
        assert_eq!(
            MEASURED_RECIPES.minecraft
                + MEASURED_RECIPES.minecolonies
                + MEASURED_RECIPES.structurize
                + MEASURED_RECIPES.tags,
            MEASURED_RECIPES.distinct,
            "the four namespaces account for every distinct id"
        );

        // The two rows of `RECIPE_USES` are exactly the references the pack makes
        // to an id the table renames.
        for row in RECIPE_USES {
            let counted = refs.get(row.id).copied().unwrap_or(0);
            assert_eq!(counted, row.references as usize, "{}", row.id);
        }
    }

    #[test]
    fn only_the_two_tabulated_ids_are_missing_from_26_3_on_the_item_side() {
        let Some(registry) = registry_keys("minecraft:item") else {
            return;
        };
        let paths = recipe_paths();
        let mut absent = BTreeSet::new();
        for path in &paths {
            let text = std::fs::read_to_string(path)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            let value: Value = mod_api::serde_json::from_str(&text)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            let mut refs = BTreeMap::new();
            collect_refs(&value, &mut refs);
            for id in refs.keys() {
                if id.starts_with("minecraft:") && !registry.contains(id) {
                    absent.insert(id.clone());
                }
            }
        }
        let tabulated: BTreeSet<String> = RECIPE_USES.iter().map(|row| row.id.to_owned()).collect();
        assert_eq!(
            absent, tabulated,
            "an id 26.3 lacks without a row is a recipe that cannot be crafted"
        );
    }

    #[test]
    fn the_item_export_of_the_pinned_client_confirms_both_targets() {
        let Some(items) = registry_keys("minecraft:item") else {
            return;
        };
        assert_eq!(items.len(), ITEM_KEYS_26_3);
        assert_eq!(digest(&items), ITEM_DIGEST_26_3);
        for row in RECIPE_USES {
            let target = renamed(Target::Item, row.id)
                .unwrap_or_else(|| panic!("{} is not renamed", row.id));
            assert!(items.contains(target), "{target} is not an item of 26.3");
            assert!(
                !items.contains(row.id),
                "{} is still an item of 26.3, so the row is wrong",
                row.id
            );
        }
        assert!(
            items.contains("minecraft:tall_grass"),
            "the result that kept its name is still there: the two renames are not a prefix match"
        );
    }
}
