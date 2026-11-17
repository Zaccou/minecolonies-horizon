//! `DataGeneratorConstants`, and the two naming rules every provider shares.
//!
//! Java reference: `core/generation/DataGeneratorConstants.java` (23 lines) and
//! the four `append`/id helpers at `defaults/DefaultRecipeProvider.java:264-276`.
//! Three facts, and each one is measured against the pack rather than copied:
//!
//! 1. **the directories** — Java writes `data/minecolonies/loot_tables/blocks`
//!    (`DataGeneratorConstants.java:16`), and the pack delivered here is
//!    `data/minecolonies/loot_table/blocks`: 1.20.5 renamed the directory, and
//!    **146** files sit under the new name. [`LOOT_TABLE_DIR`] carries the
//!    delivered name, and [`constants::JAVA_LOOT_TABLE_DIR`] carries Java's, so
//!    the difference is visible instead of being silently resolved (both are
//!    [`LOOT_TABLE_DIR`] and [`JAVA_LOOT_TABLE_DIR`]);
//! 2. **the namespace** — every id the generator writes is `minecolonies:` and
//!    every file it writes sits under `data/minecolonies/`, except the two
//!    providers that *append* to vanilla (`DefaultBlockLootTableProvider`
//!    writes 32 banner tables into `data/minecraft/`);
//! 3. **the suffix rule** — a recipe id is its output's id plus two strings, and
//!    the second being empty is what makes the suffix disappear
//!    (`DefaultRecipeProvider.java:264-271`). That is why
//!    `blockhutguardtower` has one file and `blockhutdeliveryman` has two.

/// `Constants.MOD_ID`, the namespace of everything the mod registers.
pub const MOD_ID: &str = "minecolonies";
/// `DataGeneratorConstants.DATAPACK_DIR` — `data/` + the mod id + `/`.
pub const DATAPACK_DIR: &str = "data/minecolonies/";

/// What 1.20.5 renamed the loot directory to, and the name the pack uses.
///
/// Java still says `loot_tables` (`DataGeneratorConstants.java:16`); the 146
/// delivered files are under `loot_table/`. A generator that wrote Java's name
/// would deliver zero loot tables to the pinned engine.
pub const LOOT_TABLE_DIR: &str = "loot_table";

/// Java's spelling, kept so the rename is a measured fact and not a comment.
pub const JAVA_LOOT_TABLE_DIR: &str = "loot_tables";

/// `DataGeneratorConstants.ASSETS_DIR`.
pub const ASSETS_DIR: &str = "assets/minecolonies/";

/// The three colony-side directories of `DataGeneratorConstants.java:20-22`,
/// each relative to [`DATAPACK_DIR`].
pub const COLONY_STORIES_DIR: &str = "colony/stories";
pub const COLONY_QUESTS_DIR: &str = "colony/quests";
pub const COLONY_RECRUITMENT_ITEMS_DIR: &str = "colony/recruitment_items";

/// The loot context a table is built for, which decides the `type` field.
///
/// Java passes a `LootContextParamSet` to `LootTableRegistrar.register`
/// (`SimpleLootTableProvider.java:76`), and `LootTableProvider` writes
/// `type` from it — **except** for `ALL_PARAMS`, which has no registry name and
/// so writes no `type` at all. Measured on the pack: the 83 block tables say
/// `minecraft:block`, the 2 supply chests say `minecraft:chest`, the 4 fishing
/// tables say `minecraft:fishing`, the 2 empty bonus tables say
/// `minecraft:empty`, and the 18 entity tables, the 5 nether worker trips and
/// the 16 sifter tables — all registered with `ALL_PARAMS` — say nothing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Context {
    /// `LootContextParamSets.BLOCK`: `blocks/*`.
    Block,
    /// `LootContextParamSets.CHEST`: the two supply deployers.
    Chest,
    /// `LootContextParamSets.FISHING`: `fisherman/*`.
    Fishing,
    /// `LootContextParamSets.ALL_PARAMS`: no `type` field.
    All,
    /// `LootContextParamSets.EMPTY`: `minecraft:empty`, and no pool at all is
    /// legal (the two fisher levels that drop nothing).
    Empty,
}

impl Context {
    /// The `type` value this context writes, or `None` for [`Context::All`].
    pub fn type_field(self) -> Option<&'static str> {
        match self {
            Context::Block => Some("minecraft:block"),
            Context::Chest => Some("minecraft:chest"),
            Context::Fishing => Some("minecraft:fishing"),
            Context::All => None,
            Context::Empty => Some("minecraft:empty"),
        }
    }
}

/// The file a loot table with this id is delivered as, relative to `data/`:
/// `minecolonies:blocks/x` → `minecolonies/loot_table/blocks/x.json`.
pub fn loot_path(id: &str) -> String {
    format!(
        "{}/{}/{}.json",
        super::json::namespace_of(id),
        LOOT_TABLE_DIR,
        super::json::path_of(id)
    )
}

/// A recipe file under `data/minecolonies/recipe/`, from its path.
pub fn recipe_path(path: &str) -> String {
    format!("minecolonies/recipe/{path}.json")
}

/// A tag file: `<namespace>/tags/<registry>/<path>.json`, where `registry` is
/// `blocks`, `items`, `entity_types`, `worldgen/biome` or `damage_type`.
pub fn tag_path(namespace: &str, registry: &str, path: &str) -> String {
    format!("{namespace}/tags/{registry}/{path}.json")
}

/// `DefaultRecipeProvider.append` (`:264-270`), and its `ItemLike` overload
/// (`:272-276`): the recipe id of a variant, which is the base path plus two
/// strings, and **the base alone** when the second string is empty (`:268`).
///
/// The two callers spell it differently, and the difference is the point. The
/// six doughs of the cooking matrix pass `("_", "smoking")` (`:455-544`); the
/// four `1x2` hut variants go through the `ItemLike` overload, which passes
/// `""` as the *first* string (`:275`) — hence `blockhutminerstone` and not
/// `blockhutminer_stone`. `registerHutRecipe1x2(blockHutGuardTower, BOW, "")`
/// (`:92`) is that overload with an empty second string too, which is how the
/// guard tower gets one file, two huts per craft, and no suffix at all.
pub fn append(base: &str, text1: &str, text2: &str) -> String {
    if text2.is_empty() {
        base.to_owned()
    } else {
        format!("{base}{text1}{text2}")
    }
}

/// The recipe id of a cooking variant: `baked_bread` + `smoking` →
/// `baked_bread_smoking`, and the plain variant stays `baked_bread`
/// (`DefaultRecipeProvider.java:455-544`, where every `.save(…)` spells
/// `append(…, "_", "smoking")` or `append(…, "_", "")`).
pub fn cooking_variant(base: &str, variant: &str) -> String {
    append(base, "_", variant)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_folder_java_writes_is_not_the_folder_the_pack_has() {
        // The rename is the one constant that, taken from Java, loses all 146
        // tables. Both spellings are kept, and they must not drift back together.
        assert_ne!(LOOT_TABLE_DIR, JAVA_LOOT_TABLE_DIR);
        assert_eq!(LOOT_TABLE_DIR, "loot_table");
        let path = loot_path("minecolonies:blocks/blockhutbaker");
        assert_eq!(path, "minecolonies/loot_table/blocks/blockhutbaker.json");
        assert!(
            !path.contains("loot_tables"),
            "the delivered name has no s: {path}"
        );
    }

    #[test]
    fn a_loot_id_keeps_its_namespace_and_its_sub_folders() {
        assert_eq!(
            loot_path("minecolonies:recipes/netherworker/trip1"),
            "minecolonies/loot_table/recipes/netherworker/trip1.json"
        );
        assert_eq!(
            loot_path("minecraft:blocks/black_banner"),
            "minecraft/loot_table/blocks/black_banner.json",
            "the 32 banner tables are the only files outside minecolonies/"
        );
    }

    #[test]
    fn the_suffix_rule_drops_both_strings_when_the_second_is_empty() {
        // Java: `if (text2.isEmpty()) return base;`
        assert_eq!(append("minecolonies:blockhutguardtower", "", ""), "minecolonies:blockhutguardtower");
        assert_eq!(
            append("minecolonies:blockhutdeliveryman", "iron", ""),
            "minecolonies:blockhutdeliveryman",
            "one string and no second is still the base"
        );
        assert_eq!(
            append("minecolonies:blockhutdeliveryman", "iron", "x"),
            "minecolonies:blockhutdeliverymanironx"
        );
    }

    #[test]
    fn the_four_cooking_variants_name_their_method() {
        assert_eq!(cooking_variant("baked_bread", ""), "baked_bread");
        assert_eq!(cooking_variant("baked_bread", "smoking"), "baked_bread_smoking");
        assert_eq!(
            cooking_variant("baked_bread", "campfire_cooking"),
            "baked_bread_campfire_cooking"
        );
    }

    #[test]
    fn only_all_params_writes_no_type() {
        assert_eq!(Context::All.type_field(), None);
        assert_eq!(Context::Block.type_field(), Some("minecraft:block"));
        assert_eq!(Context::Chest.type_field(), Some("minecraft:chest"));
        assert_eq!(Context::Fishing.type_field(), Some("minecraft:fishing"));
        assert_eq!(Context::Empty.type_field(), Some("minecraft:empty"));
    }

    #[test]
    fn the_tag_path_spells_the_registry_as_a_folder() {
        assert_eq!(
            tag_path("minecolonies", "worldgen/biome", "coldbiomes"),
            "minecolonies/tags/worldgen/biome/coldbiomes.json"
        );
        assert_eq!(
            tag_path("minecraft", "damage_type", "bypasses_armor"),
            "minecraft/tags/damage_type/bypasses_armor.json"
        );
    }

    #[test]
    fn the_colony_directories_hang_off_the_datapack_dir() {
        assert!(DATAPACK_DIR.ends_with('/'));
        assert_eq!(DATAPACK_DIR, "data/minecolonies/");
        assert!(!COLONY_STORIES_DIR.starts_with("data/"));
        assert_eq!(ASSETS_DIR, "assets/minecolonies/");
    }
}
