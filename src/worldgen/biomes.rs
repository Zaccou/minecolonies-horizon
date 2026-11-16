//! Which biomes host which MineColonies structure: the sixteen tag files, and
//! nothing else.
//!
//! Java reference: a structure JSON points at a tag
//! (`worldgen/structure/colonial_colony.json:9`: `"biomes":
//! "#minecolonies:has_structure/colonial_colony"`) and the tag holds the list
//! (`tags/worldgen/biome/has_structure/colonial_colony.json:4-26`). Vanilla's
//! `Structure.generate` tests that list **before** the structure type's own code, so
//! a biome outside it never reaches the ground rules — the order [`super::choose`]
//! keeps.
//!
//! **An entry is a string, and its `#` is the tag marker**, exactly as the JSON
//! spells it: `"minecraft:plains"` is an id, `"#forge:is_plains"` is a tag. Only a
//! level can answer the second (`Host::level_biome_in_tag`,
//! `host/levels.rs:145-158`), which is the [`Biomes`] trait; [`TagTable`] is the
//! offline answer.
//!
//! **The lists are not sixteen independent lists.** Measured entry by entry
//! (`python3` diff of the sixteen files, 16/11/2026):
//!
//! | Tag | Entries | Relation to the colonial list (23) |
//! | --- | ---: | --- |
//! | `colonial_colony`, `medieval_oak_colony` | 23 | identical |
//! | `asian_colony`, `caledonia_colony`, `medieval_birch_colony` | 25 | + the two birch forests |
//! | `medieval_dark_oak_colony` | 24 | + dark forest |
//! | `medieval_spruce_colony` | 24 | + grove |
//! | `shire_colony` | 13 | a shorter list, written out |
//! | `barbarian_camp` | 12 | the shire list without jagged peaks |
//! | `incan_colony` | 8 | the windswept and peak entries only |
//!
//! The port keeps the relation instead of copying 208 strings, so a data pack that
//! moves one entry fails a test here.

/// Whether a biome belongs to a biome tag — the one question the caller answers.
///
/// Java reads `Holder.is(TagKey)` through the level's merged tag tables; a server
/// implementation is one `level_biome_in_tag(level, biome, tag)` per member.
pub trait Biomes {
    /// `biome` is in `tag`, given **without** its leading `#`.
    fn in_tag(&self, biome: &str, tag: &str) -> bool;
}

/// A fixed biome→tags table: the offline [`Biomes`], for tests and for a caller
/// that already read the tags it needs. A biome nobody registered is in no tag,
/// which is what a data pack that dropped `#forge:is_plains` would produce.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TagTable {
    entries: Vec<(String, Vec<String>)>,
}

impl TagTable {
    /// A table with no tag at all: every `#…` member answers `false`.
    pub fn new() -> TagTable {
        TagTable::default()
    }

    /// Records that `biome` is in `tag` (with or without the `#`).
    pub fn tag(mut self, biome: &str, tag: &str) -> TagTable {
        let tag = tag.trim_start_matches('#');
        match self.entries.iter_mut().find(|entry| entry.0.as_str() == biome) {
            Some(entry) => {
                if !entry.1.iter().any(|known| known.as_str() == tag) {
                    entry.1.push(tag.to_owned());
                }
            }
            None => self.entries.push((biome.to_owned(), vec![tag.to_owned()])),
        }
        self
    }
}

impl Biomes for TagTable {
    fn in_tag(&self, biome: &str, tag: &str) -> bool {
        self.entries
            .iter()
            .find(|entry| entry.0.as_str() == biome)
            .is_some_and(|entry| entry.1.iter().any(|known| known.as_str() == tag))
    }
}

/// One `has_structure` tag: an id, the file and line range it was read from, and
/// the members.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BiomeTag {
    /// The tag id as the structure JSON writes it, without the `#`.
    pub id: &'static str,
    /// `file:lines` of the tag file this list was read from.
    pub source: &'static str,
    /// The members written in the base list: an id, or a `#tag`.
    pub base: &'static [&'static str],
    /// The members this tag adds to [`Self::base`], each with its own line.
    pub extra: &'static [&'static str],
}

impl BiomeTag {
    /// Every member, base then extra.
    pub fn members(&self) -> impl Iterator<Item = &&'static str> + '_ {
        self.base.iter().chain(self.extra.iter())
    }

    /// How many entries the file holds — the number the table above measured.
    pub fn len(&self) -> usize {
        self.base.len() + self.extra.len()
    }

    /// No file is empty: each of the sixteen has at least one entry.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Does this tag hold `biome`? A `#` entry is asked through [`Biomes`]; a tag
    /// nothing defines holds nothing, which is the host's documented answer
    /// (`host/levels.rs:150-153`).
    pub fn accepts(&self, biome: &str, biomes: &dyn Biomes) -> bool {
        self.members().any(|entry| match entry.strip_prefix('#') {
            Some(tag) => biomes.in_tag(biome, tag),
            None => *entry == biome,
        })
    }
}

/// A MineColonies worldgen structure: the twelve colony styles, and the four
/// supply structures that go with them.
///
/// The order is `structure_set/empty_colony.json:3-51`, then the three camps and the
/// ship. [`Self::id`] is the registry name, which is also the file's stem under
/// `worldgen/structure/`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: the sixteen structures the mod's data declares
pub enum Structure {
    MedievalBirch,
    MedievalOak,
    MedievalSpruce,
    MedievalDarkOak,
    Asian,
    Caledonia,
    LostMesaCity,
    Desertoasis,
    Shire,
    Incan,
    Colonial,
    WarpedNetherlands,
    AmazonCamp,
    BarbarianCamp,
    DesertCamp,
    Ship,
}

impl Structure {
    /// The twelve that are colonies — the ones `structure_set/empty_colony.json`
    /// lists.
    pub const COLONIES: [Structure; 12] = [
        Structure::MedievalBirch,
        Structure::MedievalOak,
        Structure::MedievalSpruce,
        Structure::MedievalDarkOak,
        Structure::Asian,
        Structure::Caledonia,
        Structure::LostMesaCity,
        Structure::Desertoasis,
        Structure::Shire,
        Structure::Incan,
        Structure::Colonial,
        Structure::WarpedNetherlands,
    ];

    /// The sixteen, colonies first.
    pub const ALL: [Structure; 16] = [
        Structure::MedievalBirch,
        Structure::MedievalOak,
        Structure::MedievalSpruce,
        Structure::MedievalDarkOak,
        Structure::Asian,
        Structure::Caledonia,
        Structure::LostMesaCity,
        Structure::Desertoasis,
        Structure::Shire,
        Structure::Incan,
        Structure::Colonial,
        Structure::WarpedNetherlands,
        Structure::AmazonCamp,
        Structure::BarbarianCamp,
        Structure::DesertCamp,
        Structure::Ship,
    ];

    /// The registry name, e.g. `minecolonies:colonial_colony`.
    pub fn id(self) -> &'static str {
        match self {
            Structure::MedievalBirch => "minecolonies:medieval_birch_colony",
            Structure::MedievalOak => "minecolonies:medieval_oak_colony",
            Structure::MedievalSpruce => "minecolonies:medieval_spruce_colony",
            Structure::MedievalDarkOak => "minecolonies:medieval_dark_oak_colony",
            Structure::Asian => "minecolonies:asian_colony",
            Structure::Caledonia => "minecolonies:caledonia_colony",
            Structure::LostMesaCity => "minecolonies:lostmesacity_colony",
            Structure::Desertoasis => "minecolonies:desertoasis_colony",
            Structure::Shire => "minecolonies:shire_colony",
            Structure::Incan => "minecolonies:incan_colony",
            Structure::Colonial => "minecolonies:colonial_colony",
            Structure::WarpedNetherlands => "minecolonies:warpednetherlands_colony",
            Structure::AmazonCamp => "minecolonies:amazon_camp",
            Structure::BarbarianCamp => "minecolonies:barbarian_camp",
            Structure::DesertCamp => "minecolonies:desert_camp",
            Structure::Ship => "minecolonies:ship",
        }
    }

    /// A colony, as opposed to a supply camp or a ship.
    pub fn is_colony(self) -> bool {
        Structure::COLONIES.contains(&self)
    }

    /// The `has_structure` tag this structure is placed by.
    pub fn biomes(self) -> &'static BiomeTag {
        match self {
            Structure::MedievalBirch => &MEDIEVAL_BIRCH,
            Structure::MedievalOak => &MEDIEVAL_OAK,
            Structure::MedievalSpruce => &MEDIEVAL_SPRUCE,
            Structure::MedievalDarkOak => &MEDIEVAL_DARK_OAK,
            Structure::Asian => &ASIAN,
            Structure::Caledonia => &CALEDONIA,
            Structure::LostMesaCity => &LOSTMESA,
            Structure::Desertoasis => &DESERTOASIS,
            Structure::Shire => &SHIRE,
            Structure::Incan => &INCAN,
            Structure::Colonial => &COLONIAL,
            Structure::WarpedNetherlands => &WARPED,
            Structure::AmazonCamp => &AMAZON,
            Structure::BarbarianCamp => &BARBARIAN,
            Structure::DesertCamp => &DESERT,
            Structure::Ship => &OCEAN,
        }
    }

    /// May this structure generate in `biome`?
    pub fn hosts(self, biome: &str, biomes: &dyn Biomes) -> bool {
        self.biomes().accepts(biome, biomes)
    }
}

/// The twenty-three entries `colonial_colony` and `medieval_oak_colony` hold —
/// `tags/worldgen/biome/has_structure/colonial_colony.json:4-26`, verified entry by
/// entry against `medieval_oak_colony.json:4-26`.
const PLAINS: [&str; 23] = [
    "minecraft:plains",
    "#forge:is_plains",
    "minecraft:sunflower_plains",
    "minecraft:snowy_plains",
    "minecraft:forest",
    "#minecraft:is_forest",
    "minecraft:flower_forest",
    "minecraft:windswept_hills",
    "minecraft:windswept_gravelly_hills",
    "minecraft:windswept_forest",
    "minecraft:meadow",
    "minecraft:snowy_slopes",
    "minecraft:frozen_peaks",
    "minecraft:jagged_peaks",
    "minecraft:stony_peaks",
    "#forge:is_peak",
    "minecraft:river",
    "minecraft:frozen_river",
    "minecraft:beach",
    "#minecraft:is_beach",
    "minecraft:snowy_beach",
    "minecraft:stony_shore",
    "minecraft:mushroom_fields",
];

/// The two birches `asian_colony`, `caledonia_colony` and `medieval_birch_colony`
/// add to [`PLAINS`] (`…/asian_colony.json:11-12`).
const BIRCH: [&str; 2] = [
    "minecraft:birch_forest",
    "minecraft:old_growth_birch_forest",
];

/// `…/medieval_birch_colony.json:4-28` — 25.
const MEDIEVAL_BIRCH: BiomeTag = BiomeTag {
    id: "minecolonies:has_structure/medieval_birch_colony",
    source: "tags/worldgen/biome/has_structure/medieval_birch_colony.json:4-28",
    base: &PLAINS,
    extra: &BIRCH,
};

/// `…/medieval_oak_colony.json:4-26` — 23, the base list whole.
const MEDIEVAL_OAK: BiomeTag = BiomeTag {
    id: "minecolonies:has_structure/medieval_oak_colony",
    source: "tags/worldgen/biome/has_structure/medieval_oak_colony.json:4-26",
    base: &PLAINS,
    extra: &[],
};

/// `…/medieval_spruce_colony.json:4-27` — 24: `grove` (`:27`) on the base list.
const MEDIEVAL_SPRUCE: BiomeTag = BiomeTag {
    id: "minecolonies:has_structure/medieval_spruce_colony",
    source: "tags/worldgen/biome/has_structure/medieval_spruce_colony.json:4-27",
    base: &PLAINS,
    extra: &["minecraft:grove"],
};

/// `…/medieval_dark_oak_colony.json:4-27` — 24: `dark_forest` (`:4`), then the base
/// list.
const MEDIEVAL_DARK_OAK: BiomeTag = BiomeTag {
    id: "minecolonies:has_structure/medieval_dark_oak_colony",
    source: "tags/worldgen/biome/has_structure/medieval_dark_oak_colony.json:4-27",
    base: &PLAINS,
    extra: &["minecraft:dark_forest"],
};

/// `…/asian_colony.json:4-28` — 25.
const ASIAN: BiomeTag = BiomeTag {
    id: "minecolonies:has_structure/asian_colony",
    source: "tags/worldgen/biome/has_structure/asian_colony.json:4-28",
    base: &PLAINS,
    extra: &BIRCH,
};

/// `…/caledonia_colony.json:4-28` — 25, the same two birches as `asian_colony`.
const CALEDONIA: BiomeTag = BiomeTag {
    id: "minecolonies:has_structure/caledonia_colony",
    source: "tags/worldgen/biome/has_structure/caledonia_colony.json:4-28",
    base: &PLAINS,
    extra: &BIRCH,
};

/// `…/lostmesacity_colony.json:4-4` — the badlands tag, and nothing else.
const LOSTMESA: BiomeTag = BiomeTag {
    id: "minecolonies:has_structure/lostmesacity_colony",
    source: "tags/worldgen/biome/has_structure/lostmesacity_colony.json:4-4",
    base: &["#minecraft:is_badlands"],
    extra: &[],
};

/// `…/desertoasis_colony.json:4-4` — the desert tag, and nothing else.
const DESERTOASIS: BiomeTag = BiomeTag {
    id: "minecolonies:has_structure/desertoasis_colony",
    source: "tags/worldgen/biome/has_structure/desertoasis_colony.json:4-4",
    base: &["#forge:is_desert"],
    extra: &[],
};

/// `…/shire_colony.json:4-16` — 13: the base list without its eleven river, peak,
/// beach, mushroom and gravelly entries.
const SHIRE: BiomeTag = BiomeTag {
    id: "minecolonies:has_structure/shire_colony",
    source: "tags/worldgen/biome/has_structure/shire_colony.json:4-16",
    base: &[
        "minecraft:plains",
        "#forge:is_plains",
        "minecraft:sunflower_plains",
        "minecraft:snowy_plains",
        "minecraft:forest",
        "#minecraft:is_forest",
        "minecraft:flower_forest",
        "minecraft:birch_forest",
        "minecraft:old_growth_birch_forest",
        "minecraft:windswept_hills",
        "minecraft:windswept_forest",
        "minecraft:meadow",
        "minecraft:jagged_peaks",
    ],
    extra: &[],
};

/// `…/incan_colony.json:4-11` — 8: the windswept and peak entries, nothing else.
const INCAN: BiomeTag = BiomeTag {
    id: "minecolonies:has_structure/incan_colony",
    source: "tags/worldgen/biome/has_structure/incan_colony.json:4-11",
    base: &[
        "minecraft:windswept_hills",
        "minecraft:windswept_gravelly_hills",
        "minecraft:windswept_forest",
        "minecraft:snowy_slopes",
        "minecraft:frozen_peaks",
        "minecraft:jagged_peaks",
        "minecraft:stony_peaks",
        "#forge:is_peak",
    ],
    extra: &[],
};

/// `…/colonial_colony.json:4-26` — 23, the base list whole.
const COLONIAL: BiomeTag = BiomeTag {
    id: "minecolonies:has_structure/colonial_colony",
    source: "tags/worldgen/biome/has_structure/colonial_colony.json:4-26",
    base: &PLAINS,
    extra: &[],
};

/// `…/warpednetherlands_colony.json:4-4` — **the only nether colony**, and the only
/// structure whose file sets `allow_cave`
/// (`worldgen/structure/warpednetherlands_colony.json:16`).
const WARPED: BiomeTag = BiomeTag {
    id: "minecolonies:has_structure/warpednetherlands_colony",
    source: "tags/worldgen/biome/has_structure/warpednetherlands_colony.json:4-4",
    base: &["minecraft:warped_forest"],
    extra: &[],
};

/// `…/amazon_camp.json:4-4` — the jungle tag.
const AMAZON: BiomeTag = BiomeTag {
    id: "minecolonies:has_structure/amazon_camp",
    source: "tags/worldgen/biome/has_structure/amazon_camp.json:4-4",
    base: &["#minecraft:is_jungle"],
    extra: &[],
};

/// `…/barbarian_camp.json:4-15` — 12: [`SHIRE`] without `jagged_peaks`.
const BARBARIAN: BiomeTag = BiomeTag {
    id: "minecolonies:has_structure/barbarian_camp",
    source: "tags/worldgen/biome/has_structure/barbarian_camp.json:4-15",
    base: &[
        "minecraft:plains",
        "#forge:is_plains",
        "minecraft:sunflower_plains",
        "minecraft:snowy_plains",
        "minecraft:forest",
        "#minecraft:is_forest",
        "minecraft:flower_forest",
        "minecraft:birch_forest",
        "minecraft:old_growth_birch_forest",
        "minecraft:windswept_hills",
        "minecraft:windswept_forest",
        "minecraft:meadow",
    ],
    extra: &[],
};

/// `…/desert_camp.json:4-4` — the desert tag, and nothing else.
const DESERT: BiomeTag = BiomeTag {
    id: "minecolonies:has_structure/desert_camp",
    source: "tags/worldgen/biome/has_structure/desert_camp.json:4-4",
    base: &["#forge:is_desert"],
    extra: &[],
};

/// `…/ship.json:4-4` — the ocean tag: the only structure that wants water.
const OCEAN: BiomeTag = BiomeTag {
    id: "minecolonies:has_structure/ship",
    source: "tags/worldgen/biome/has_structure/ship.json:4-4",
    base: &["#minecraft:is_ocean"],
    extra: &[],
};

#[cfg(test)]
mod tests {
    use super::*;

    /// The tags a vanilla-shaped world answers, for the biomes these tests use.
    fn world() -> TagTable {
        TagTable::new()
            .tag("minecraft:plains", "forge:is_plains")
            .tag("minecraft:sunflower_plains", "forge:is_plains")
            .tag("minecraft:forest", "minecraft:is_forest")
            .tag("minecraft:dark_forest", "minecraft:is_forest")
            .tag("minecraft:badlands", "minecraft:is_badlands")
            .tag("minecraft:desert", "forge:is_desert")
            .tag("minecraft:jungle", "minecraft:is_jungle")
            .tag("minecraft:deep_ocean", "minecraft:is_ocean")
    }

    /// Every structure that hosts `biome`, in [`Structure::ALL`] order.
    fn hosts(biome: &str, world: &dyn Biomes) -> Vec<Structure> {
        Structure::ALL
            .iter()
            .copied()
            .filter(|structure| structure.hosts(biome, world))
            .collect()
    }

    #[test]
    fn the_measured_entry_count_of_every_tag_is_the_one_the_port_holds() {
        // `grep -c '^    "'` over the sixteen files, 16/11/2026.
        for (structure, count) in [
            (Structure::MedievalBirch, 25),
            (Structure::MedievalOak, 23),
            (Structure::MedievalSpruce, 24),
            (Structure::MedievalDarkOak, 24),
            (Structure::Asian, 25),
            (Structure::Caledonia, 25),
            (Structure::LostMesaCity, 1),
            (Structure::Desertoasis, 1),
            (Structure::Shire, 13),
            (Structure::Incan, 8),
            (Structure::Colonial, 23),
            (Structure::WarpedNetherlands, 1),
            (Structure::AmazonCamp, 1),
            (Structure::BarbarianCamp, 12),
            (Structure::DesertCamp, 1),
            (Structure::Ship, 1),
        ] {
            assert_eq!(structure.biomes().len(), count, "{}", structure.id());
            assert!(!structure.biomes().is_empty());
        }
    }

    #[test]
    fn the_variant_tags_are_the_base_list_plus_their_delta() {
        assert_eq!(PLAINS.len(), 23);
        assert_eq!(MEDIEVAL_BIRCH.len(), PLAINS.len() + BIRCH.len());
        assert_eq!(MEDIEVAL_SPRUCE.len(), PLAINS.len() + 1);
        assert_eq!(MEDIEVAL_DARK_OAK.len(), PLAINS.len() + 1);
        // The two that are the base list alone.
        assert!(MEDIEVAL_OAK.extra.is_empty());
        assert!(COLONIAL.extra.is_empty());
        assert_eq!(MEDIEVAL_OAK.base, COLONIAL.base);
    }

    #[test]
    fn an_id_entry_matches_and_a_tag_entry_is_asked_through_the_level() {
        let world = world();
        assert!(Structure::Colonial.hosts("minecraft:plains", &world), "by id");
        assert!(Structure::Colonial.hosts("minecraft:flower_forest", &world));
        // A data pack's biome is hosted the moment the level says it is a plains.
        assert!(!Structure::Colonial.hosts("biomesoplenty:lavender_field", &world));
        let tagged = world.tag("biomesoplenty:lavender_field", "#forge:is_plains");
        assert!(Structure::Colonial.hosts("biomesoplenty:lavender_field", &tagged));
        assert!(!Structure::Colonial.hosts("minecraft:jungle", &tagged));
    }

    #[test]
    fn the_birch_forests_and_the_grove_are_what_separate_the_medieval_styles() {
        let world = world();
        for biome in ["minecraft:birch_forest", "minecraft:old_growth_birch_forest"] {
            assert!(Structure::Asian.hosts(biome, &world), "{biome}");
            assert!(Structure::Caledonia.hosts(biome, &world), "{biome}");
            assert!(Structure::MedievalBirch.hosts(biome, &world), "{biome}");
            assert!(!Structure::Colonial.hosts(biome, &world), "{biome}");
        }
        assert_eq!(
            hosts("minecraft:dark_forest", &world).len(),
            9,
            "dark_oak names it, and eight styles hold #minecraft:is_forest"
        );
        assert_eq!(hosts("minecraft:grove", &world), vec![Structure::MedievalSpruce]);
    }

    #[test]
    fn a_biome_no_tag_hosts_is_a_biome_no_colony_can_appear_in() {
        let world = world();
        for biome in [
            "minecraft:the_end",
            "minecraft:small_end_islands",
            "minecraft:nether_wastes",
            "minecraft:soul_sand_valley",
            "minecraft:basalt_deltas",
            "minecraft:deep_dark",
            "minecraft:swamp",
        ] {
            assert!(
                !Structure::COLONIES
                    .iter()
                    .any(|structure| structure.hosts(biome, &world)),
                "{biome} must host no colony"
            );
        }
        // The end and two of the nether biomes host nothing at all, not even a camp.
        for biome in ["minecraft:the_end", "minecraft:nether_wastes", "minecraft:basalt_deltas"] {
            assert!(hosts(biome, &world).is_empty(), "{biome}");
        }
    }

    #[test]
    fn the_warped_forest_is_the_only_nether_biome_and_the_only_cave_colony() {
        let world = world();
        assert_eq!(
            hosts("minecraft:warped_forest", &world),
            vec![Structure::WarpedNetherlands]
        );
        assert!(Structure::WarpedNetherlands.is_colony());
        assert_eq!(
            Structure::WarpedNetherlands.biomes().len(),
            1,
            "one id, no tag: the nether colony has no fallback"
        );
    }

    #[test]
    fn the_biome_specific_structures_are_exclusive() {
        let world = world();
        assert_eq!(hosts("minecraft:badlands", &world), vec![Structure::LostMesaCity]);
        assert_eq!(
            hosts("minecraft:desert", &world),
            vec![Structure::Desertoasis, Structure::DesertCamp],
            "one colony and one camp, so only the biome tells them apart"
        );
        assert_eq!(hosts("minecraft:jungle", &world), vec![Structure::AmazonCamp]);
        assert_eq!(hosts("minecraft:deep_ocean", &world), vec![Structure::Ship]);
        // `minecraft:ocean` is in `#minecraft:is_ocean` in vanilla, but this
        // fixture only tags `deep_ocean`: the tag is the answer, so an untagged
        // ocean hosts nothing — and no colony is ever placed on open water.
        assert!(hosts("minecraft:ocean", &world).is_empty());
    }

    #[test]
    fn the_shire_and_the_barbarian_camp_differ_by_jagged_peaks_alone() {
        let world = world();
        assert!(Structure::Shire.hosts("minecraft:jagged_peaks", &world));
        assert!(!Structure::BarbarianCamp.hosts("minecraft:jagged_peaks", &world));
        assert_eq!(Structure::Shire.biomes().len(), 13);
        assert_eq!(Structure::BarbarianCamp.biomes().len(), 12);
        for entry in Structure::BarbarianCamp.biomes().members() {
            assert!(
                Structure::Shire.biomes().members().any(|other| other == entry),
                "the barbarian camp adds {entry} to the shire list"
            );
        }
    }

    #[test]
    fn the_twelve_colonies_are_the_set_the_ids_are_namespaced_and_the_files_are_stems() {
        assert_eq!(Structure::COLONIES.len(), 12);
        assert_eq!(Structure::ALL.len(), 16);
        let mut ids: Vec<&str> = Vec::new();
        for structure in Structure::ALL {
            assert!(structure.id().starts_with("minecolonies:"), "{}", structure.id());
            let stem = structure.id().trim_start_matches("minecolonies:");
            assert_eq!(
                structure.biomes().id,
                format!("minecolonies:has_structure/{stem}")
            );
            assert!(
                structure.biomes().source.starts_with("tags/worldgen/biome/has_structure/")
            );
            assert!(!ids.contains(&structure.id()));
            ids.push(structure.id());
        }
        assert_eq!(ids.len(), 16);
    }

    #[test]
    fn a_table_that_knows_no_tag_hosts_only_the_entries_that_name_an_id() {
        let empty = TagTable::new();
        // Tag-only tags accept nothing...
        for structure in [
            Structure::LostMesaCity,
            Structure::Desertoasis,
            Structure::Ship,
            Structure::AmazonCamp,
            Structure::DesertCamp,
        ] {
            assert!(!structure.hosts("minecraft:plains", &empty), "{}", structure.id());
        }
        // ...while the lists that name ids still work, because ids need no table.
        assert!(Structure::Colonial.hosts("minecraft:plains", &empty));
        assert!(!Structure::Colonial.hosts("minecraft:meadow_extra", &empty));
    }
}
