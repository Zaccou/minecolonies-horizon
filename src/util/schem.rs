//! `SchemAnalyzerUtil` and `SchemFixerUtil`: what a blueprint costs, and what a
//! rename has to rewrite.
//!
//! Java reference: `core/util/SchemAnalyzerUtil.java` (248 lines) and
//! `core/util/SchemFixerUtil.java` (145). The first has one real rule — a block's
//! *complexity score* — and a summing loop; the second is a one-shot file
//! migration for the `home`/`citizen` → `residence` rename that happened years
//! ago, whose arithmetic (a name rewrite and two corners) is the part a port can
//! actually use.
//!
//! # The score, and why the tag order matters
//!
//! `getScoreFor` (`SchemAnalyzerUtil.java:73-120`) is a cube of the block's tier
//! followed by a chain of `else if`s over vanilla block tags. The chain is the
//! rule: a **wooden slab** is `WOODEN_SLABS` *and* `SLABS`, and it is caught by the
//! wooden branch — `min(27, score) / 4` — one line before the plain `SLABS` branch
//! would have halved it. [`BlockFamily`] is that chain turned into a value, and
//! [`family`] takes the tag booleans **in Java's order** so a block that is in two
//! families cannot be resolved the wrong way by accident.
//!
//! # Not here
//!
//! - `getBlockTier` itself asks `ModTags.tier1blocks … tier6blocks`
//!   (`:39-62`), which is a tag membership table the host answers. The rule
//!   ("the highest tier tag wins, else 0, and a `null` block is `-1`") is
//!   [`block_tier`].
//! - The Domum Ornamentum branch (`:145-164`) needs the texture data of a DO
//!   block, which this SDK does not export: `MANQUANT.md` §1 gives the formula
//!   (`max(2, sumOfParts / 3)`) and why it cannot be written here.
//! - `SchemFixerUtil.fixSchematics` (`:140-208`) is a directory walk with
//!   `File.renameTo` and `NbtIo.readCompressed`; a mod has neither. The two
//!   decisions inside it are here.

use std::collections::BTreeSet;

/// `structurize:blocksolidsubstitution`, excluded from an analysis
/// (`SchemAnalyzerUtil.java:209`).
///
/// The id is Structurize's: `ModBlocks.java:65` registers it as
/// `"blockSolidSubstitution".toLowerCase()`. `placement/kind.rs:28` measured the
/// same string in the 9 515-file corpus, and it is repeated here only because
/// that module is not declared in `lib.rs` right now — when it is, this constant
/// should read `crate::placement::kind::SOLID_SUBSTITUTION`.
pub const SOLID_SUBSTITUTION: &str = "structurize:blocksolidsubstitution";

/// `structurize:blocksubstitution` (`ModBlocks.java:65`, same measurement).
pub const SUBSTITUTION: &str = "structurize:blocksubstitution";

/// The three ids `BlockState.isAir()` answers `true` for
/// (`SchemAnalyzerUtil.java:209`).
pub const AIR_BLOCKS: [&str; 3] = [
    "minecraft:air",
    "minecraft:cave_air",
    "minecraft:void_air",
];

/// Is this cell left out of the analysis: `isExcludedBlock`
/// (`SchemAnalyzerUtil.java:207-210`).
///
/// ```java
/// return blockState == null || blockState.isAir()
///     || blockState.getBlock() instanceof BlockSubstitution
///     || blockState.getBlock() instanceof BlockSolidSubstitution;
/// ```
///
/// Air is three ids in vanilla, not one, and `isAir()` accepts all three. The two
/// substitutions are placeholders a structure leaves for the builder to fill
/// with whatever the colony has; counting them would price a building on blocks
/// nobody can buy.
///
/// A `null` state is Java's fourth case; a `&str` cannot be null, so the caller
/// that has no state simply filters it out first.
pub fn is_excluded(block: &str) -> bool {
    AIR_BLOCKS.contains(&block) || block == SUBSTITUTION || block == SOLID_SUBSTITUTION
}

/// The nine tag families `getScoreFor` tests, in the order it tests them
/// (`SchemAnalyzerUtil.java:79-117`).
// exhaustive: the whole else-if chain, in Java's order
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockFamily {
    /// `BlockTags.LOGS` (`:79`).
    Logs,
    /// `WOODEN_STAIRS | WOODEN_BUTTONS | WOODEN_DOORS | WOODEN_SLABS | WOODEN_FENCES |
    /// WOODEN_TRAPDOORS | WOODEN_PRESSURE_PLATES` (`:83-84`).
    WoodenFittings,
    /// `BlockTags.PLANKS` (`:89`).
    Planks,
    /// `BlockTags.SLABS` (`:94`).
    Slabs,
    /// `BlockTags.BANNERS` (`:98`).
    Banners,
    /// `Tags.Blocks.STORAGE_BLOCKS`, the Forge `forge:storage_blocks` tag (`:102`).
    StorageBlocks,
    /// `BlockTags.ANVIL` (`:106`).
    Anvil,
    /// `Tags.Blocks.BOOKSHELVES` (`:110`).
    Bookshelves,
    /// `BlockTags.BASE_STONE_NETHER` (`:114`).
    NetherBaseStone,
    /// Every block the chain never names: the plain cube of the tier.
    Other,
}

/// Which family a block falls in, from its tag memberships, **tested in Java's
/// order** (`SchemAnalyzerUtil.java:79-117`).
///
/// The arguments are the nine predicates the chain asks, in the order it asks
/// them; the first `true` wins. The order is the reason this is not a
/// `HashSet<BlockFamily>`: `oak_slab` is wooden **and** a slab, and Java has
/// already decided which of the two it means by the time the second test would
/// run.
///
/// A block in no family is [`BlockFamily::Other`], which is not a failure — most
/// blocks are (stone, glass, wool).
pub fn family(
    logs: bool,
    wooden_fittings: bool,
    planks: bool,
    slabs: bool,
    banners: bool,
    storage_blocks: bool,
    anvil: bool,
    bookshelves: bool,
    nether_base_stone: bool,
) -> BlockFamily {
    if logs {
        BlockFamily::Logs
    } else if wooden_fittings {
        BlockFamily::WoodenFittings
    } else if planks {
        BlockFamily::Planks
    } else if slabs {
        BlockFamily::Slabs
    } else if banners {
        BlockFamily::Banners
    } else if storage_blocks {
        BlockFamily::StorageBlocks
    } else if anvil {
        BlockFamily::Anvil
    } else if bookshelves {
        BlockFamily::Bookshelves
    } else if nether_base_stone {
        BlockFamily::NetherBaseStone
    } else {
        BlockFamily::Other
    }
}

/// The tier of a block: `getBlockTier` (`SchemAnalyzerUtil.java:32-65`).
///
/// ```java
/// if (block == null) return -1;
/// if (state.is(ModTags.tier6blocks)) return 6;
/// ... else if (state.is(ModTags.tier1blocks)) return 1;
/// return 0;
/// ```
///
/// Takes the tier tags the block carries and keeps the **highest**, which is what
/// Java's descending scan produces. No tag gives `0` — a block any tool breaks.
/// Java's `-1` is its `null` guard and has no `&str` counterpart: it exists so
/// that a missing block cannot be priced, and a caller here has a name.
///
/// A tier above `6` is not possible in Java (six tags) but is not refused here
/// either: the answer is whatever the caller claims, and clamping would hide a
/// caller's mistake instead of pricing it.
pub fn block_tier(tiers_present: impl IntoIterator<Item = u8>) -> i32 {
    let mut highest = 0i32;
    for tier in tiers_present {
        if i32::from(tier) > highest {
            highest = i32::from(tier);
        }
    }
    highest
}

/// What one block contributes to a blueprint's cost:
/// `getScoreFor` (`SchemAnalyzerUtil.java:73-120`).
///
/// ```java
/// double score = Math.pow(getBlockTier(block) + 1, 3);
/// if (logs) { score = min(27, score); }
/// else if (wooden fittings) { score = min(27, score); score *= 1.0 / 4; }
/// else if (planks) { ... same two steps ... }
/// else if (slabs) { score *= 1.0 / 2; }
/// else if (banners) { score *= 6; }
/// else if (storage blocks) { score *= 9; }
/// else if (anvil) { score *= 31; }
/// else if (bookshelves) { score *= 3; }
/// else if (nether base stone) { score = 8; }
/// ```
///
/// Note what the cheap branches do **not** do: `SLABS` halves the *cube*, so a
/// tier-3 slab is `64 / 2 = 32`, still more than a log's capped `27`. And the two
/// `min(27, ...)` caps apply to logs, wooden fittings and planks only — a
/// diamond block (tier 4, storage) is `125 * 9 = 1 125`, which is the point of
/// the multiplier.
///
/// The one branch that **ignores** the cube entirely is nether base stone, fixed
/// at `8`: netherrack is tier 0 (`1`), and Java prices it at eight times that.
pub fn score_for(tier: i32, family: BlockFamily) -> f64 {
    let score = f64::from(tier + 1).powi(3);
    let capped = score.min(27.0);
    match family {
        BlockFamily::Logs => capped,
        BlockFamily::WoodenFittings => capped * (1.0 / 4.0),
        BlockFamily::Planks => capped * (1.0 / 4.0),
        BlockFamily::Slabs => score * (1.0 / 2.0),
        BlockFamily::Banners => score * 6.0,
        BlockFamily::StorageBlocks => score * 9.0,
        BlockFamily::Anvil => score * 31.0,
        BlockFamily::Bookshelves => score * 3.0,
        BlockFamily::NetherBaseStone => 8.0,
        BlockFamily::Other => score,
    }
}

/// One cell of a blueprint, reduced to what the analysis reads.
// exhaustive: the three facts `analyzeSchematic` reads per cell
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell<'a> {
    /// The block state's name, which is also the item it yields — `asItem()`
    /// throws the properties away, and so does this.
    pub block: &'a str,
    /// [`score_for`] of this block, or the Domum Ornamentum estimate
    /// (`SchemAnalyzerUtil.java:158`).
    pub score: f64,
    /// `block instanceof AbstractBlockHut` (`:183`).
    pub is_hut: bool,
}

/// What `analyzeSchematic` answers.
// exhaustive: the three numbers the result carries
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Analysis {
    /// `(int) complexityScore` (`SchemAnalyzerUtil.java:198`) — **truncated**, not
    /// rounded.
    pub complexity: i32,
    /// `blocks.keySet().size()` (`:193`): how many distinct blocks.
    pub distinct_blocks: usize,
    /// `containedBuildings` (`:190`): how many hut blocks the structure holds.
    pub buildings: usize,
}

/// How much a building costs and how complex it is: the summing half of
/// `analyzeSchematic` (`SchemAnalyzerUtil.java:128-199`).
///
/// ```java
/// complexityScore += blockComplexity;                    // per cell
/// complexityScore += containedBuildings * 500;           // :190
/// complexityScore += blocks.keySet().size() * 40;        // :193
/// complexityScore += (sizeX * sizeY * sizeZ) / 10.0;     // :196
/// return new SchematicAnalyzationResult((int) complexityScore, ...);
/// ```
///
/// The three bonuses are what make a big hollow shell cost less than a small
/// intricate one: a **building inside** is worth 500, every **distinct block** 40,
/// and a cell of volume a tenth. Excluded cells must already be filtered out by
/// the caller ([`is_excluded`]); Java's `continue` (`:136-139`) is that filter.
///
/// `distinct` is the count of distinct [`Cell::block`] **names**, computed here so
/// that a caller cannot pass a number that disagrees with its own cells. That is
/// Java's `HashMap<ItemStorage, …>` reduced to what it actually distinguishes: the
/// item, since `asItem()` maps every facing of a stair to the same item.
///
/// The cast is Java's: `(int)` **truncates towards zero**, so a score of `99.9`
/// reports `99` and a structure is always one complexity point cheaper than its
/// arithmetic. Rounding here would change every blueprint's price.
pub fn complexity<'a>(
    cells: impl IntoIterator<Item = Cell<'a>>,
    size: [u16; 3],
) -> Analysis {
    let mut score = 0.0f64;
    let mut buildings = 0usize;
    let mut distinct: BTreeSet<&str> = BTreeSet::new();
    for cell in cells {
        score += cell.score;
        if cell.is_hut {
            buildings += 1;
        }
        distinct.insert(cell.block);
    }
    score += (buildings as f64) * 500.0;
    score += (distinct.len() as f64) * 40.0;
    let volume = u64::from(size[0]) * u64::from(size[1]) * u64::from(size[2]);
    score += (volume as f64) / 10.0;
    Analysis {
        complexity: score as i32,
        distinct_blocks: distinct.len(),
        buildings,
    }
}

/// Does this schematic name need the `home`/`citizen` → `residence` rename:
/// `fixSchematicNameAndCorners` (`SchemFixerUtil.java:240-245`).
///
/// ```java
/// if (name.contains("citizen") || name.contains("home")) {
///     blueprint.setName(blueprint.getName().replace("home", "residence"));
///     blueprint.setName(blueprint.getName().replace("citizen", "residence"));
/// }
/// ```
///
/// The *stored* name decides whether to act; the **blueprint's** name is what
/// gets rewritten. Those are two different strings in practice — the stored one
/// carries the pack path, the blueprint name is the file's own — and a port that
/// rewrote the trigger string instead would produce a name the placer cannot find.
pub fn residence_rename(blueprint_name: &str, stored_name: &str) -> Option<String> {
    if !(stored_name.contains("citizen") || stored_name.contains("home")) {
        return None;
    }
    Some(
        blueprint_name
            .replace("home", "residence")
            .replace("citizen", "residence"),
    )
}

/// The two component corners of a schematic, relative to its anchor:
/// `fixSchematicNameAndCorners` (`SchemFixerUtil.java:249-252`).
///
/// ```java
/// BlockPosUtil.writeToNBT(schemDataCompound, TAG_CORNER_ONE, BlockPos.ZERO.subtract(blueprint.getPrimaryBlockOffset()));
/// BlockPosUtil.writeToNBT(schemDataCompound, TAG_CORNER_TWO,
///     new BlockPos(sizeX - 1, sizeY - 1, sizeZ - 1).subtract(blueprint.getPrimaryBlockOffset()));
/// ```
///
/// Corner one is the **negated** anchor; corner two is the last cell minus the
/// anchor. Both are stored *in the anchor's frame*, which is why the anchor's own
/// cell comes out as `[0, 0, 0]` and its opposite as `[size - 1, …] - offset`.
/// A structure with no anchor is `[0,0,0]` and gets the same corners a size-only
/// read would give.
pub fn component_corners(size: [i32; 3], primary_offset: [i32; 3]) -> [[i32; 3]; 2] {
    [
        [
            -primary_offset[0],
            -primary_offset[1],
            -primary_offset[2],
        ],
        [
            size[0] - 1 - primary_offset[0],
            size[1] - 1 - primary_offset[1],
            size[2] - 1 - primary_offset[2],
        ],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Air is three ids, and the two substitutions are Structurize's.
    #[test]
    fn the_excluded_blocks_are_air_and_the_two_substitutions() {
        assert!(is_excluded("minecraft:air"));
        assert!(is_excluded("minecraft:cave_air"));
        assert!(is_excluded("minecraft:void_air"));
        assert!(is_excluded("structurize:blocksubstitution"));
        assert!(is_excluded("structurize:blocksolidsubstitution"));
    }

    /// Everything else is priced, including a fluid and a structure void.
    #[test]
    fn a_real_block_is_not_excluded() {
        assert!(!is_excluded("minecraft:stone"));
        assert!(!is_excluded("minecraft:water"));
        assert!(!is_excluded("minecolonies:blockhutbuilder"));
    }

    /// The chain's order: a wooden slab is caught by the wooden branch, not by
    /// the plain slab one.
    #[test]
    fn a_wooden_slab_is_wooden_before_it_is_a_slab() {
        assert_eq!(
            family(false, true, false, true, false, false, false, false, false),
            BlockFamily::WoodenFittings
        );
        assert_eq!(
            family(false, false, false, true, false, false, false, false, false),
            BlockFamily::Slabs
        );
    }

    /// Every arm of the chain is reachable, in order.
    #[test]
    fn the_family_chain_follows_javas_order() {
        assert_eq!(
            family(true, true, true, true, true, true, true, true, true),
            BlockFamily::Logs
        );
        assert_eq!(
            family(false, false, true, false, false, false, false, false, false),
            BlockFamily::Planks
        );
        assert_eq!(
            family(false, false, false, false, true, false, false, false, false),
            BlockFamily::Banners
        );
        assert_eq!(
            family(false, false, false, false, false, true, false, false, false),
            BlockFamily::StorageBlocks
        );
        assert_eq!(
            family(false, false, false, false, false, false, true, false, false),
            BlockFamily::Anvil
        );
        assert_eq!(
            family(false, false, false, false, false, false, false, true, false),
            BlockFamily::Bookshelves
        );
        assert_eq!(
            family(false, false, false, false, false, false, false, false, true),
            BlockFamily::NetherBaseStone
        );
        assert_eq!(
            family(false, false, false, false, false, false, false, false, false),
            BlockFamily::Other
        );
    }

    /// The tier is the highest tag present, and no tag is `0`.
    #[test]
    fn the_tier_is_the_highest_tag_present() {
        assert_eq!(block_tier([]), 0);
        assert_eq!(block_tier([1]), 1);
        assert_eq!(block_tier([1, 4]), 4);
        assert_eq!(block_tier([4, 1, 6]), 6);
    }

    /// The cube, and the three capped branches.
    #[test]
    fn the_plain_score_is_the_cube_of_the_tier_plus_one() {
        assert_eq!(score_for(0, BlockFamily::Other), 1.0);
        assert_eq!(score_for(1, BlockFamily::Other), 8.0);
        assert_eq!(score_for(2, BlockFamily::Other), 27.0);
        assert_eq!(score_for(3, BlockFamily::Other), 64.0);
        assert_eq!(score_for(4, BlockFamily::Other), 125.0);
    }

    /// Logs are capped at `27`; planks and wooden fittings at `27 / 4`.
    #[test]
    fn the_wooden_branches_are_capped_before_being_divided() {
        assert_eq!(score_for(4, BlockFamily::Logs), 27.0);
        assert_eq!(score_for(0, BlockFamily::Logs), 1.0);
        assert_eq!(score_for(4, BlockFamily::Planks), 27.0 / 4.0);
        assert_eq!(score_for(4, BlockFamily::WoodenFittings), 27.0 / 4.0);
        assert_eq!(score_for(0, BlockFamily::Planks), 0.25);
    }

    /// Slabs halve the **cube**, so a tier-3 slab (32) still beats a log (27).
    #[test]
    fn a_slab_halves_the_cube_and_is_not_capped() {
        assert_eq!(score_for(3, BlockFamily::Slabs), 32.0);
        assert_eq!(score_for(0, BlockFamily::Slabs), 0.5);
        assert!(score_for(3, BlockFamily::Slabs) > score_for(3, BlockFamily::Logs));
    }

    /// The four multipliers, each on the raw cube.
    #[test]
    fn the_multipliers_are_six_nine_thirty_one_and_three() {
        assert_eq!(score_for(1, BlockFamily::Banners), 48.0);
        assert_eq!(score_for(1, BlockFamily::StorageBlocks), 72.0);
        assert_eq!(score_for(1, BlockFamily::Anvil), 248.0);
        assert_eq!(score_for(1, BlockFamily::Bookshelves), 24.0);
    }

    /// Nether base stone ignores the cube entirely.
    #[test]
    fn nether_base_stone_is_always_eight() {
        for tier in 0..=6 {
            assert_eq!(score_for(tier, BlockFamily::NetherBaseStone), 8.0);
        }
    }

    /// The three bonuses, and the volume's tenth.
    #[test]
    fn the_complexity_adds_five_hundred_per_hut_and_forty_per_distinct_block() {
        let cells = [
            Cell { block: "minecraft:stone", score: 10.0, is_hut: true },
            Cell { block: "minecraft:stone", score: 10.0, is_hut: false },
            Cell { block: "minecraft:oak_planks", score: 5.0, is_hut: false },
        ];
        let analysis = complexity(cells, [10, 10, 10]);
        // 25 of blocks + 500 per hut + 40 × 2 distinct + 1000/10 volume
        assert_eq!(analysis.complexity, 25 + 500 + 80 + 100);
        assert_eq!(analysis.buildings, 1);
        assert_eq!(analysis.distinct_blocks, 2);
    }

    /// The cast truncates: a fractional total loses its fraction, always down.
    #[test]
    fn the_complexity_is_truncated_like_a_java_cast() {
        let cells = [Cell { block: "minecraft:stone", score: 0.9, is_hut: false }];
        // 0.9 + 40 (one distinct block) + 0.1 (one cell) == 41.0
        assert_eq!(complexity(cells, [1, 1, 1]).complexity, 41);

        let cells = [Cell { block: "minecraft:stone", score: 0.05, is_hut: false }];
        // 0.05 + 40 + 0.1 == 40.15, and `(int)` gives 40
        assert_eq!(complexity(cells, [1, 1, 1]).complexity, 40);
    }

    /// An empty structure still costs its (zero) volume and nothing else.
    #[test]
    fn an_empty_structure_is_free() {
        let analysis = complexity(std::iter::empty(), [0, 0, 0]);
        assert_eq!(analysis, Analysis::default());
    }

    /// A hut inside a hut is two buildings, and the same block name is one
    /// distinct block however often it appears.
    #[test]
    fn the_building_count_is_per_hut_cell_not_per_distinct_hut() {
        let cells = [
            Cell { block: "minecolonies:blockhutbuilder", score: 1.0, is_hut: true },
            Cell { block: "minecolonies:blockhutbuilder", score: 1.0, is_hut: true },
        ];
        let analysis = complexity(cells, [2, 1, 1]);
        assert_eq!(analysis.buildings, 2);
        assert_eq!(analysis.distinct_blocks, 1);
        assert_eq!(analysis.complexity, 2 + 1000 + 40 + 0);
    }

    /// The rename fires on the stored name and rewrites the blueprint's.
    #[test]
    fn the_residence_rename_rewrites_the_blueprint_name_only() {
        assert_eq!(
            residence_rename("home0.blueprint", "home0.blueprint"),
            Some("residence0.blueprint".to_owned())
        );
        assert_eq!(
            residence_rename("residence0.blueprint", "home0.blueprint"),
            Some("residence0.blueprint".to_owned())
        );
        assert_eq!(
            residence_rename("medieval/citizen2", "medieval/citizen2"),
            Some("medieval/residence2".to_owned())
        );
    }

    /// A name with neither word is left alone, whatever the other string says.
    #[test]
    fn a_name_without_home_or_citizen_is_not_renamed() {
        assert_eq!(residence_rename("builder0.blueprint", "builder0.blueprint"), None);
        // The trigger is the *stored* name: a stored name without the words does
        // not fire even when the blueprint name has one.
        assert_eq!(residence_rename("home0", "builder0"), None);
    }

    /// The corners are expressed in the anchor's frame, so the anchor is zero.
    #[test]
    fn the_component_corners_are_relative_to_the_anchor() {
        let corners = component_corners([9, 5, 9], [4, 2, 4]);
        assert_eq!(corners[0], [-4, -2, -4]);
        assert_eq!(corners[1], [4, 2, 4]);
    }

    /// An anchor in a corner gives the whole box, and no anchor gives the size.
    #[test]
    fn a_corner_anchor_and_a_missing_anchor_both_make_sense() {
        assert_eq!(component_corners([9, 5, 9], [0, 0, 0]), [[0, 0, 0], [8, 4, 8]]);
        assert_eq!(component_corners([9, 5, 9], [8, 4, 8]), [[-8, -4, -8], [0, 0, 0]]);
        assert_eq!(component_corners([1, 1, 1], [0, 0, 0]), [[0, 0, 0], [0, 0, 0]]);
    }
}
