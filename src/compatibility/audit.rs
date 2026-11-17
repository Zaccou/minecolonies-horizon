//! What `CraftingTagAuditor` measures, column by column: **ten CSV reports** of
//! the data the colony's crafting depends on, written once into the world.
//!
//! Java reference: `core/compatibility/CraftingTagAuditor.java` (619 lines).
//!
//! # The published list, measured
//!
//! | File | Header | Source |
//! | --- | --- | --- |
//! | `tag_item_audit.csv` | `item,name,tags…` | `:120-148` |
//! | `tag_block_audit.csv` | `block,name,tags…` | `:150-183` |
//! | `tag_path_audit.csv` | `block,name,path,climbable,dangerous` | `:185-215` |
//! | `tag_tier_audit.csv` | `block,name,score,tier0…tier6` | `:218-238` |
//! | `tag_biome_audit.csv` | `biome,name,tags…` | `:241-275` |
//! | `recipe_audit.csv` | `item,name,player,<crafter>…` | `:277-345` |
//! | `domum_audit.csv` | `type,item,name,<crafter>…` | `:347-397` |
//! | `tools_audit.csv` | `item,name,<tool>…` | `:399-432` |
//! | `food_audit.csv` | `item,name,nutrition,maxlevel,tier,foodvalue,fullhealth` | `:434-465` |
//! | `compost_audit.csv` | `item,name,vanilla,mcol` | `:467-497` |
//!
//! All ten land under the world folder, not the game's config: `<world>/minecolonies/
//! <name>.csv` (`:85`, `server.getWorldPath(LevelResource.ROOT).resolve(MOD_ID)`).
//!
//! **This is the integration that survives its mod.** JEI prints the recipes a
//! player may want; the auditor writes down whether the *tags* behind them are
//! complete — which item falls into no crafter, which block has no path, what tier
//! the schematic analyser gives a block, which food feeds a citizen for how long —
//! and it does it server-side with no client mod at all (`:55-57`: "we want this to
//! happen server-side without depending on anything from that"). Of the 33 files
//! measured here it is the only one worth running on a Horizon server as it is.
//!
//! # The three things a port has to get right
//!
//! 1. **Quoting.** Java writes CSV by hand, and the only escaping is `"` → `""`
//!    (`:508-515`, `:521-523`). A name with a comma is not quoted further; a name
//!    with a quote is doubled. [`quoted`] is that rule.
//! 2. **The tier row.** Seven columns for seven tiers, with the one the block
//!    reached marked — a comma-repeated prefix then `-` or `*`s (`:234-236`).
//! 3. **The crafter order.** Simple (personal 2×2) modules first, then by recipe
//!    key, then the herders (`:285-289`) — the columns of `recipe_audit.csv` and
//!    `domum_audit.csv` are in that order, so a diff between two runs of the same
//!    world lines up.

use crate::citizens::FULL_SATURATION;
use crate::compatibility::Documented;

/// The folder the ten files go to, `MOD_ID` (`:85`).
pub const WORLD_FOLDER: &str = "minecolonies";

/// One of the ten reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum Report {
    ItemTags,
    BlockTags,
    PathBlocks,
    BlockTiers,
    BiomeTags,
    Recipes,
    Domum,
    Tools,
    Food,
    Compost,
}

/// The ten reports in the order Java writes them (`:68-77`).
pub const AUDITS: Documented<[Report; 10]> = Documented::new(
    [
        Report::ItemTags,
        Report::BlockTags,
        Report::PathBlocks,
        Report::BlockTiers,
        Report::BiomeTags,
        Report::Recipes,
        Report::Domum,
        Report::Tools,
        Report::Food,
        Report::Compost,
    ],
    "CraftingTagAuditor.java:68-77",
);

impl Report {
    /// The file name, exactly as `createFile` receives it (`:68-77`).
    pub fn file(self) -> &'static str {
        match self {
            Self::ItemTags => "tag_item_audit.csv",
            Self::BlockTags => "tag_block_audit.csv",
            Self::PathBlocks => "tag_path_audit.csv",
            Self::BlockTiers => "tag_tier_audit.csv",
            Self::BiomeTags => "tag_biome_audit.csv",
            Self::Recipes => "recipe_audit.csv",
            Self::Domum => "domum_audit.csv",
            Self::Tools => "tools_audit.csv",
            Self::Food => "food_audit.csv",
            Self::Compost => "compost_audit.csv",
        }
    }

    /// The header line, before any dynamic column. Written out as Java writes it,
    /// quotes and dots included (`:123-125`, `:153`, `:188`, `:220`, `:244`, `:302`,
    /// `:358`, `:438`, `:471`, `:501`).
    pub fn header(self) -> &'static str {
        match self {
            // `writeItemHeaders` then `",tags..."` (`:123-125`, `:501`).
            Self::ItemTags => "item,name,tags...",
            Self::BlockTags => "block,name,tags...",
            Self::PathBlocks => "block,name,path,climbable,dangerous",
            Self::BlockTiers => {
                "block,name,score,tier0,tier1,tier2,tier3,tier4,tier5,tier6"
            }
            Self::BiomeTags => "biome,name,tags...",
            // `writeItemHeaders` then `",player"` then one column per crafter
            // (`:302`, `:104-125`).
            Self::Recipes => "item,name,player",
            // `"type,"` then the item headers, then the crafters (`:358-165`).
            Self::Domum => "type,item,name",
            // The item headers, then one column per tool (`:205-211` -> `:403-409`).
            Self::Tools => "item,name",
            // `writeItemHeaders` then `",nutrition,maxlevel,tier,foodvalue,fullhealth"`
            // (`:438`).
            Self::Food => "item,name,nutrition,maxlevel,tier,foodvalue,fullhealth",
            // `writeItemHeaders` then `",vanilla,mcol"` (`:471`).
            Self::Compost => "item,name,vanilla,mcol",
        }
    }

    /// How many columns the **fixed** part of the header has, before the dynamic
    /// tail (`tags...` for the four tag reports, one column per crafter or per tool
    /// for the other three). A test counts the commas, so a header that drops a
    /// column cannot pass.
    pub fn columns(self) -> usize {
        match self {
            Self::ItemTags | Self::BlockTags | Self::BiomeTags => 3,
            Self::PathBlocks => 5,
            Self::BlockTiers => 10,
            // `writeItemHeaders` then `",player"` (`:501`, `:302`), then one column
            // per crafter (`:104-125`).
            Self::Recipes => 3,
            // `"type,"` then `writeItemHeaders` (`:358`, `:501`), then the crafters.
            Self::Domum => 3,
            // `writeItemHeaders` **only** (`:403`): the tools are all in the dynamic
            // tail (`:405-409`), so the fixed part is the two item columns.
            Self::Tools => 2,
            Self::Food => 7,
            Self::Compost => 4,
        }
    }
}

/// The world-relative path of a report (`:85`).
pub fn path(report: Report) -> String {
    format!("{}/{}", WORLD_FOLDER, report.file())
}

/// Java's CSV cell: wrapped in quotes, with every inner `"` doubled — and nothing
/// else escaped (`:508-515`, `:521-523`).
pub fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\"\""))
}

/// The tier marker Java writes after the score (`:234-236`): `tier` empty columns,
/// then `tier` stars — or a single `-` at tier 0.
///
/// `tier` is clamped to 0..=6 first (`:234`), so a schematic analyser that answers
/// 9 is reported as tier 6 rather than as eleven columns.
///
/// **The marker is the tier's own column, and the row stops there.** Java writes
/// `",".repeat(tier)` then `"*".repeat(tier)`, so tier 3 is `,,,***`: three empty
/// fields, then `***` — which lands under the header's `tier3`, and repeats the
/// count rather than marking it once. The row is therefore **shorter than the
/// 10-column header** for every tier below 6 (legal CSV: a missing trailing field
/// is empty), which is what the file really contains.
pub fn tier_row(tier: i32) -> String {
    let tier = tier.clamp(0, 6);
    let marker = if tier == 0 {
        "-".to_owned()
    } else {
        "*".repeat(tier as usize)
    };
    format!("{}{}", ",".repeat(tier as usize), marker)
}

/// How many of a food item a citizen needs to reach full saturation: `FULL_SATURATION
/// / FoodUtils.getFoodValue(item, properties, 0)` (`:462`).
///
/// `None` when the food value is zero or below: Java writes `Infinity` (a `double`
/// division, `:462`) into the CSV, which no reader can use and no test can compare.
pub fn full_health(food_value: f64) -> Option<f64> {
    if food_value <= 0.0 {
        return None;
    }
    Some(FULL_SATURATION / food_value)
}

/// The crafter columns of `recipe_audit.csv` and `domum_audit.csv`, in the order
/// Java sorts them: the simple (personal 2×2) modules first, then by recipe key
/// (`:285-289`).
///
/// The key is `getCustomRecipeKey`, which is what the header shows. Java's sort is
/// `comparing(simple).reversed().thenComparing(key)`: a `Boolean` compares false <
/// true, so `.reversed()` puts `true` — simple — first.
pub fn crafter_columns<'a>(crafters: &'a [(bool, &'a str)]) -> Vec<&'a str> {
    let mut sorted = crafters.to_vec();
    sorted.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.cmp(right.1))
    });
    sorted.into_iter().map(|(_, key)| key).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_ten_reports_and_they_are_written_in_that_order() {
        assert_eq!(AUDITS.value.len(), 10);
        let files: Vec<&str> = AUDITS.value.iter().map(|report| report.file()).collect();
        assert_eq!(files[0], "tag_item_audit.csv");
        assert_eq!(files[9], "compost_audit.csv");
        assert_eq!(files.len(), 10);
        let mut unique = files.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 10, "two reports share a file");
    }

    #[test]
    fn every_fixed_header_has_as_many_columns_as_it_chains() {
        for report in AUDITS.value {
            let commas = report.header().matches(',').count();
            assert_eq!(commas + 1, report.columns(), "{report:?}");
        }
    }

    #[test]
    fn the_three_dynamic_reports_differ_in_where_their_tail_starts() {
        // `recipe_audit.csv` has a `player` column of its own before the crafters
        // (`:302`), `domum_audit.csv` a leading `type` (`:358`), and
        // `tools_audit.csv` none at all: its header is only `item,name` (`:403-409`).
        assert_eq!(Report::Recipes.columns(), 3);
        assert_eq!(Report::Domum.columns(), 3);
        assert_eq!(Report::Tools.columns(), 2);
    }

    #[test]
    fn the_four_headers_with_a_dynamic_tail_stop_before_it() {
        // The tail is one column per crafter (`recipe_audit`, `domum_audit`,
        // `tools_audit`) or one per tag (`tag_item_audit` and friends, which end in
        // the literal `tags...`).
        assert!(Report::ItemTags.header().ends_with("tags..."));
        assert!(Report::BiomeTags.header().ends_with("tags..."));
        assert!(Report::BlockTags.header().ends_with("tags..."));
        assert_eq!(Report::Recipes.header(), "item,name,player");
        assert_eq!(Report::Domum.header(), "type,item,name");
        assert_eq!(Report::Tools.header(), "item,name");
    }

    #[test]
    fn the_food_header_is_the_seven_measurements_java_writes() {
        assert_eq!(
            Report::Food.header(),
            "item,name,nutrition,maxlevel,tier,foodvalue,fullhealth"
        );
        assert_eq!(Report::Compost.header(), "item,name,vanilla,mcol");
    }

    #[test]
    fn the_files_go_under_the_worlds_own_minecolonies_folder() {
        // `<world>/minecolonies/<name>.csv` (`:85`).
        assert_eq!(path(Report::Food), "minecolonies/food_audit.csv");
        for report in AUDITS.value {
            let path = path(report);
            assert!(path.starts_with("minecolonies/"), "{path}");
            assert!(path.ends_with(".csv"), "{path}");
            assert!(!path.contains(".."), "{path}");
        }
    }

    #[test]
    fn a_quote_is_doubled_and_nothing_else_is_escaped() {
        // `replace("\"", "\"\"")` (`:510`, `:521`).
        assert_eq!(quoted("Wheat"), "\"Wheat\"");
        assert_eq!(quoted("a \"quoted\" name"), "\"a \"\"quoted\"\" name\"");
        // A comma is *not* escaped: the cell is already quoted, which is enough for
        // a CSV reader and is what Java writes (`:508-515`).
        assert_eq!(quoted("one, two"), "\"one, two\"");
        assert_eq!(quoted(""), "\"\"");
    }

    #[test]
    fn the_tier_row_marks_the_tier_s_own_column() {
        // `",".repeat(tier)` then `"*".repeat(tier)` (`:235-236`) — the marker
        // repeats the count, so tier 3 is `,,,***` and not `,,,*`.
        assert_eq!(tier_row(0), "-");
        assert_eq!(tier_row(1), ",*");
        assert_eq!(tier_row(3), ",,,***");
        assert_eq!(tier_row(6), ",,,,,,******");
        for tier in 0..=6 {
            let row = tier_row(tier);
            // The marker sits one column further right per tier, and the empty
            // fields before it are exactly `tier`.
            assert_eq!(row.matches(',').count(), tier as usize, "{tier}");
            if tier > 0 {
                assert_eq!(row, format!("{}{}", ",".repeat(tier as usize), "*".repeat(tier as usize)));
            }
        }
    }

    #[test]
    fn a_tier_outside_the_table_is_clamped_not_dropped() {
        // `Math.max(0, Math.min(6, …))` (`:234`).
        assert_eq!(tier_row(-2), "-");
        assert_eq!(tier_row(9), tier_row(6));
    }

    #[test]
    fn a_food_items_full_health_is_sixty_over_its_value() {
        // `FULL_SATURATION / getFoodValue(item, properties, 0)` (`:462`), with
        // `FULL_SATURATION = MAX_SATURATION = 60`.
        assert_eq!(FULL_SATURATION, 60.0);
        assert_eq!(full_health(6.0), Some(10.0));
        assert_eq!(full_health(1.0), Some(60.0));
        // Java divides by zero and writes `Infinity`; the port refuses by name.
        assert_eq!(full_health(0.0), None);
        assert_eq!(full_health(-1.0), None);
    }

    #[test]
    fn the_simple_modules_come_first_then_the_keys_alphabetically() {
        // `comparing(simple).reversed().thenComparing(getCustomRecipeKey)`
        // (`:286-287`).
        let modules = [
            (false, "minecolonies:baker"),
            (true, "minecolonies:crafting"),
            (false, "minecolonies:smelter"),
            (true, "minecolonies:analytics"),
        ];
        assert_eq!(
            crafter_columns(&modules),
            [
                "minecolonies:analytics",
                "minecolonies:crafting",
                "minecolonies:baker",
                "minecolonies:smelter",
            ]
        );
    }

    #[test]
    fn the_audits_order_and_the_source_agree() {
        assert_eq!(AUDITS.value[5], Report::Recipes);
        assert!(AUDITS.source.contains(".java:"));
    }
}
