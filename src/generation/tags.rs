//! The tags: which biomes are cold, which entity is a raider, which damage type
//! ignores armour.
//!
//! Java reference: `defaults/DefaultBiomeTagsProvider.java` (110),
//! `DefaultEntityTypeTagsProvider.java` (37), `DefaultDamageTagsProvider.java`
//! (34) — the three providers whose whole content is a **rule**: four climate
//! groups, five entity relations, three damage flags. The other two —
//! `DefaultBlockTagsProvider.java` (232) and `DefaultItemTagsProvider.java` (596)
//! — are 89 files of id lists (the sixteen concrete colours, the seventy
//! pathable blocks, and the `<crafter>_ingredient` / `_product` / `_excluded`
//! trio of fifteen professions); that is content and it waits, `MANQUANT.md` §4.
//!
//! # What a tag file says
//!
//! `{"values": [ … ]}`, where an element is one of three things:
//!
//! | Element | Java | Written when |
//! | --- | --- | --- |
//! | `"minecraft:plains"` | `.add(Biomes.PLAINS)` | the id is loaded or owned |
//! | `"#minecraft:is_taiga"` | `.addTag(BiomeTags.IS_TAIGA)` | another tag is included whole |
//! | `{"id": "corpse:corpse", "required": false}` | `.addOptional(…)` | the id may be missing: another mod's |
//!
//! **`replace: true` appears in none of the 95 delivered files**, and that is a
//! decision, not an omission: a `.replace()` call on this mod's own tag would
//! discard the entries a data pack adds to it, and the only provider that does
//! replace is `DefaultBlockTagsProvider`'s `.remove(…)` on the temperate group
//! (`DefaultBiomeTagsProvider.java:67`), which *removes three tags from the group*
//! and writes nothing — the three removed links are absent from
//! `temperatebiomes.json`, and the file still has its 18 entries.
//!
//! # The one tag that is deliberately empty
//!
//! `dangerousblocks` (`DefaultBlockTagsProvider.java:163`): `tag(ModTags.dangerousBlocks)`
//! with no `.add`, so the file is `{"values": []}`. It is a hook for other mods —
//! a pack appends to it — and a generator that skipped it because it is empty
//! would break every pack that appends.

use super::constants::tag_path;
use super::json::{arr, s, Json};
use super::File;

/// One tag file. `values` is written in the order Java adds them, because a tag
/// list is read in order when two entries disagree.
pub fn tag(namespace: &str, registry: &str, path: &str, values: Vec<Json>) -> File {
    File {
        path: tag_path(namespace, registry, path),
        json: super::json::obj(vec![("values", arr(values))]),
    }
}

/// `.add(Biomes.X)` or `.add(EntityType.X)`: an id, in a namespace.
fn entry(id: &str) -> Json {
    s(id)
}

/// `.addTag(SomeTag)`: a tag reference, and the `#` is part of the string.
fn tag_entry(id: &str) -> Json {
    s(&format!("#{id}"))
}

/// `.addOptional(...)`: an id another mod may not provide, so the load must not
/// fail on it.
fn optional(id: &str) -> Json {
    super::json::obj(vec![
        ("id", s(id)),
        ("required", Json::Bool(false)),
    ])
}

/// The four climate groups (`DefaultBiomeTagsProvider.java:32-108`).
///
/// Every group starts with the vanilla and Forge biome tags that already mean
/// what the group means, then lists the biomes those tags miss. The cold group
/// carries `deep_frozen_ocean` **twice** (`:41`, `:44`): Java adds it once in the
/// ocean run and once after, and deduplication is not this generator's job.
pub fn biome_tags() -> Vec<File> {
    let cold = vec![
        tag_entry("minecraft:is_taiga"),
        tag_entry("minecraft:spawns_snow_foxes"),
        tag_entry("minecraft:polar_bears_spawn_on_alternate_blocks"),
        tag_entry("forge:is_cold"),
        tag_entry("minecraft:is_end"),
        tag_entry("forge:is_snowy"),
        entry("minecraft:cold_ocean"),
        entry("minecraft:deep_cold_ocean"),
        entry("minecraft:deep_frozen_ocean"),
        entry("minecraft:deep_dark"),
        entry("minecraft:frozen_ocean"),
        entry("minecraft:deep_frozen_ocean"),
        entry("minecraft:frozen_river"),
        entry("minecraft:frozen_peaks"),
        entry("minecraft:grove"),
        entry("minecraft:ice_spikes"),
        entry("minecraft:jagged_peaks"),
        entry("minecraft:old_growth_pine_taiga"),
        entry("minecraft:old_growth_spruce_taiga"),
        entry("minecraft:snowy_beach"),
        entry("minecraft:snowy_plains"),
        entry("minecraft:snowy_taiga"),
        entry("minecraft:snowy_slopes"),
        entry("minecraft:stony_peaks"),
        entry("minecraft:stony_shore"),
        entry("minecraft:taiga"),
        entry("minecraft:windswept_forest"),
        entry("minecraft:windswept_hills"),
        entry("minecraft:windswept_gravelly_hills"),
    ];
    let temperate = vec![
        tag_entry("minecraft:has_structure/village_plains"),
        tag_entry("forge:is_plains"),
        tag_entry("forge:is_swamp"),
        entry("minecraft:birch_forest"),
        entry("minecraft:cherry_grove"),
        entry("minecraft:dark_forest"),
        entry("minecraft:deep_lukewarm_ocean"),
        entry("minecraft:deep_ocean"),
        entry("minecraft:flower_forest"),
        entry("minecraft:forest"),
        entry("minecraft:lukewarm_ocean"),
        entry("minecraft:meadow"),
        entry("minecraft:mushroom_fields"),
        entry("minecraft:old_growth_birch_forest"),
        entry("minecraft:plains"),
        entry("minecraft:river"),
        entry("minecraft:sunflower_plains"),
        entry("minecraft:swamp"),
    ];
    let humid = vec![
        tag_entry("minecraft:is_jungle"),
        tag_entry("forge:is_wet/overworld"),
        entry("minecraft:bamboo_jungle"),
        entry("minecraft:beach"),
        entry("minecraft:river"),
        entry("minecraft:warm_ocean"),
        entry("minecraft:dripstone_caves"),
        entry("minecraft:jungle"),
        entry("minecraft:lush_caves"),
        entry("minecraft:mangrove_swamp"),
        entry("minecraft:sparse_jungle"),
    ];
    let dry = vec![
        tag_entry("minecraft:has_structure/desert_pyramid"),
        tag_entry("forge:is_desert"),
        tag_entry("forge:is_dry"),
        tag_entry("minecraft:is_savanna"),
        tag_entry("minecraft:is_nether"),
        entry("minecraft:badlands"),
        entry("minecraft:desert"),
        entry("minecraft:eroded_badlands"),
        entry("minecraft:savanna"),
        entry("minecraft:savanna_plateau"),
        entry("minecraft:windswept_savanna"),
    ];
    vec![
        tag("minecolonies", "worldgen/biome", "coldbiomes", cold),
        tag("minecolonies", "worldgen/biome", "temperatebiomes", temperate),
        tag("minecolonies", "worldgen/biome", "humidbiomes", humid),
        tag("minecolonies", "worldgen/biome", "drybiomes", dry),
    ]
}

/// The five entity relations (`DefaultEntityTypeTagsProvider.java:28-37`).
///
/// `raiders` is not a list written here: it is `ModEntities.getRaiders()`, the
/// eighteen raiders the mod registers, which is also exactly the set the entity
/// loot tables are built for (`DefaultEntityLootProvider.java:44-162`) — one
/// registration, two files, and a test below that says they still agree.
pub fn entity_type_tags() -> Vec<File> {
    const RAIDERS: [&str; 18] = [
        "barbarian",
        "archerbarbarian",
        "chiefbarbarian",
        "amazon",
        "amazonspearman",
        "amazonchief",
        "mummy",
        "archermummy",
        "pharao",
        "pirate",
        "archerpirate",
        "chiefpirate",
        "shieldmaiden",
        "norsemenarcher",
        "norsemenchief",
        "drownedpirate",
        "drownedarcherpirate",
        "drownedchiefpirate",
    ];
    vec![
        tag(
            "minecolonies",
            "entity_types",
            "hostile",
            vec![entry("minecraft:slime")],
        ),
        tag(
            "minecolonies",
            "entity_types",
            "raider_targets",
            vec![
                entry("minecraft:villager"),
                entry("minecraft:iron_golem"),
                entry("minecolonies:mercenary"),
                entry("minecolonies:citizen"),
            ],
        ),
        tag(
            "minecolonies",
            "entity_types",
            "mob_attack_blacklist",
            vec![entry("minecraft:enderman"), entry("minecraft:llama")],
        ),
        tag(
            "minecolonies",
            "entity_types",
            "allowinteract",
            vec![optional("corpse:corpse")],
        ),
        tag(
            "minecolonies",
            "entity_types",
            "raiders",
            RAIDERS
                .iter()
                .map(|name| entry(&format!("minecolonies:{name}")))
                .collect(),
        ),
    ]
}

/// The three damage flags (`DefaultDamageTagsProvider.java:30-32`).
///
/// They are appended to **vanilla's** tags, because the damage types are the
/// mod's but the tags are `minecraft:`'s — the only three files this module
/// writes outside its own namespace, with the 32 banner tables.
pub fn damage_tags() -> Vec<File> {
    vec![
        tag(
            "minecraft",
            "damage_type",
            "bypasses_armor",
            ["wakeywakey", "guardpvp", "pierce"]
                .into_iter()
                .map(|name| entry(&format!("minecolonies:{name}")))
                .collect(),
        ),
        tag(
            "minecraft",
            "damage_type",
            "is_projectile",
            ["spear", "pierce"]
                .into_iter()
                .map(|name| entry(&format!("minecolonies:{name}")))
                .collect(),
        ),
        tag(
            "minecraft",
            "damage_type",
            "bypasses_shield",
            vec![entry("minecolonies:pierce")],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::json::{s, Json};
    use crate::generation::{delivered, delivered_values, matches_delivered, Pack};

    #[test]
    fn the_four_climate_groups_are_the_four_the_pack_delivers() {
        let pack = Pack::build();
        for path in [
            "minecolonies/tags/worldgen/biome/coldbiomes.json",
            "minecolonies/tags/worldgen/biome/temperatebiomes.json",
            "minecolonies/tags/worldgen/biome/humidbiomes.json",
            "minecolonies/tags/worldgen/biome/drybiomes.json",
        ] {
            matches_delivered(&pack, path);
        }
        let cold = delivered("minecolonies/tags/worldgen/biome/coldbiomes.json");
        assert_eq!(cold["values"].as_array().map(|rows| rows.len()), Some(29));
    }

    #[test]
    fn the_cold_group_names_one_ocean_twice_because_java_does() {
        // `:41` and `:44` both add DEEP_FROZEN_OCEAN. A generator that deduped
        // would disagree with the pack on a file that is otherwise identical.
        let cold = delivered("minecolonies/tags/worldgen/biome/coldbiomes.json");
        let doubled = cold["values"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|value| value.as_str() == Some("minecraft:deep_frozen_ocean"))
            .count();
        assert_eq!(doubled, 2);
    }

    #[test]
    fn the_temperate_group_carries_none_of_the_three_tags_java_removes() {
        // `.remove(IS_COLD, IS_DRY, IS_DESERT)` (`:67`) takes them out of the
        // group; the file must not mention them at all.
        let temperate = delivered("minecolonies/tags/worldgen/biome/temperatebiomes.json");
        let text = temperate.to_string();
        for gone in ["forge:is_cold", "forge:is_dry", "forge:is_desert"] {
            assert!(!text.contains(gone), "{gone} survived the remove");
        }
    }

    #[test]
    fn the_five_entity_tags_are_the_five_the_pack_delivers() {
        let pack = Pack::build();
        for path in [
            "minecolonies/tags/entity_types/hostile.json",
            "minecolonies/tags/entity_types/raider_targets.json",
            "minecolonies/tags/entity_types/mob_attack_blacklist.json",
            "minecolonies/tags/entity_types/allowinteract.json",
            "minecolonies/tags/entity_types/raiders.json",
        ] {
            matches_delivered(&pack, path);
        }
    }

    #[test]
    fn the_allowinteract_tag_is_the_one_optional_entry() {
        // Another mod's entity, so `required: false` — the only one of the 12
        // files here that has it.
        let pack = Pack::build();
        let allow = pack
            .get("minecolonies/tags/entity_types/allowinteract.json")
            .unwrap();
        assert_eq!(allow["values"][0]["id"], s("corpse:corpse"));
        assert_eq!(allow["values"][0]["required"], Json::Bool(false));
    }

    #[test]
    fn the_raiders_tag_and_the_entity_loot_tables_name_the_same_eighteen() {
        // One registration, two files: if one moves without the other, a raider
        // drops loot nobody can meet, or walks around with no loot at all.
        let pack = Pack::build();
        let raiders = pack
            .get("minecolonies/tags/entity_types/raiders.json")
            .unwrap();
        let mut tagged: Vec<String> = raiders["values"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str().map(|id| id.to_owned()))
            .collect();
        let mut looted: Vec<String> = pack
            .under("minecolonies/loot_table/entities/")
            .iter()
            .map(|file| {
                file.path
                    .trim_start_matches("minecolonies/loot_table/entities/")
                    .trim_end_matches(".json")
                    .to_owned()
            })
            .map(|name| format!("minecolonies:{name}"))
            .collect();
        tagged.sort();
        looted.sort();
        assert_eq!(tagged, looted);
        assert_eq!(tagged.len(), 18);
    }

    #[test]
    fn the_three_damage_flags_are_appended_to_vanilla_tags() {
        let pack = Pack::build();
        for path in [
            "minecraft/tags/damage_type/bypasses_armor.json",
            "minecraft/tags/damage_type/is_projectile.json",
            "minecraft/tags/damage_type/bypasses_shield.json",
        ] {
            matches_delivered(&pack, path);
        }
        let armor = pack
            .get("minecraft/tags/damage_type/bypasses_armor.json")
            .unwrap();
        assert_eq!(armor["values"].as_array().map(|rows| rows.len()), Some(3));
        assert!(armor.get("replace").is_none(), "an append, not a replace");
    }

    #[test]
    fn no_generated_tag_replaces_the_list_it_extends() {
        // `.replace()` writes `"replace": true`, and none of the 12 files here
        // does it: a replace on this mod's own tag would drop a pack's additions.
        let pack = Pack::build();
        let mut tags = 0;
        for file in pack.under("minecolonies/tags/") {
            assert!(file.json.get("replace").is_none(), "{}", file.path);
            tags += 1;
        }
        for file in pack.under("minecraft/tags/") {
            assert!(file.json.get("replace").is_none(), "{}", file.path);
            tags += 1;
        }
        assert_eq!(tags, 12);
    }

    #[test]
    fn a_tag_reference_keeps_its_hash_and_an_id_does_not_gain_one() {
        let values = delivered_values("minecolonies/tags/worldgen/biome/humidbiomes.json");
        assert_eq!(values[0], "#minecraft:is_jungle");
        assert_eq!(values[2], "minecraft:bamboo_jungle");
    }
}
