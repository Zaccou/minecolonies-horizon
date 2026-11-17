//! Two mods that do not exist here, measured instead of imitated: what the JEI
//! and JourneyMap integrations **publish**, and what Horizon offers for it.
//!
//! Java reference: `core/compatibility/` — 33 files, 5 695 lines:
//! `jei/` (19 files, 2 056 lines, `transfer/` included), `journeymap/` (13 files,
//! 2 253 lines: `v5/` 971, `v6/` 1 270, two `package-info`), and
//! `CraftingTagAuditor.java` (619).
//!
//! **The premise this module was given, and what is true of it.** JEI and
//! JourneyMap are absent from a Horizon game, so the two integrations cannot be
//! ported as code: there is no `mezz.jei` and no `journeymap.api.v2` to call. What
//! survives them is what they *say* — which item a colony job makes from which
//! inputs at which building level, and where a colony's land and its graves are —
//! and that is worth having even with neither mod installed. Measured, both
//! surfaces are small: **6 recipe categories, 5 of them named by a fixed id and
//! one per job**, and **4 marker families** (border polygon, loaded-chunk polygon,
//! deathpoint waypoint, entity-radar entry) plus **one info slot** and **13
//! options**.
//!
//! # What Horizon offers, per published thing
//!
//! | Java publishes | Horizon primitive | State |
//! | --- | --- | --- |
//! | a recipe category with its own layout, slots and animation | none: `Recipe` "does not define display pages, per-result chances or fluid input/output slots" (`crates/horizon-mod-vocab/src/hooks/notes.rs:2744-2748`) | **`MANQUANT.md` §1** |
//! | the *output items* of those recipes in a panel | `RecipesClientHost::panel_items`, `panel_style(PanelStyle::Classic)` | covered |
//! | a recipe transfer handler (`IRecipeTransferHandler`) | the `menu_transfer` hook, named after JEI in its own `neoforge` field | covered |
//! | a GUI area / ghost-ingredient handler | `screen_exclusion_areas` (named after `IGuiContainerHandler.getGuiExtraAreas`) | covered for the *areas*, not for the ghost drop |
//! | the recipes themselves, as data | `RecipesHost::recipes_of_type` + `recipe_ingredient` + `recipe_result` | covered for vanilla recipe types only |
//! | a colony-border polygon overlay on the map | none | **`MANQUANT.md` §2** |
//! | a deathpoint waypoint | none: Horizon's `waypoint` is the locator-bar icon of a living entity (`host/living.rs:667-684`) | **`MANQUANT.md` §3** |
//! | an entity-radar override (name, colour, hide) | none | **`MANQUANT.md` §4** |
//! | an info slot on the map's HUD | none | **`MANQUANT.md` §5** |
//! | the 13 map options | `ConfigHost::setting` / `set_setting` (a string per key) | covered, as mod settings |
//! | a per-mod map data folder and `border.json` | `Host::data_files()` → `ConfigFiles` (bulk root, world data too) | covered |
//! | the 10 CSV reports of the auditor | `ServerHost::world_data_files()` (same `ConfigFiles`) | covered |
//!
//! **The one thing this module does not do is guess.** Nothing here writes a
//! `border.json`, a JEI plugin descriptor or a JourneyMap option file: a format
//! that cannot be written without a codec the crate does not have (no `serde`, by
//! `Cargo.toml`) is a format named field by field, and `MANQUANT.md` says so.
//!
//! # The model, and why it is the deliverable
//!
//! | File | Question |
//! | --- | --- |
//! | `recipes` | which recipe a colony job publishes, at which level, with which tool, inputs, outputs and chances |
//! | `markers` | where a colony's land, its loaded chunks and its graves are, and which of them the player may see |
//! | `audit` | what the auditor measures, column by column, and the CSV quoting it must keep |
//! | `MANQUANT.md` | what cannot be written, with the search that established it |
//!
//! Every number is a [`Documented`] — a value and the line that says so — and
//! [`PROVENANCE`] is the list a test walks to refuse a constant with no source.

pub mod audit;
pub mod markers;
pub mod recipes;

pub use audit::{
    AUDITS, Report, WORLD_FOLDER, crafter_columns, full_health, path as audit_path, quoted,
    tier_row,
};
pub use markers::{
    BorderOverlay, BorderStyle, ChunkSet, Color, Deathpoint, DeathpointLabel, Deathpoints,
    GraveChange, MarkerOptions, RaiderColor, SORT_ORDER, intensify, may_see_border,
    may_see_deaths, radar,
};
pub use recipes::{
    Category, Chance, CompostEntry, CropRecipe, FishingRecipe, FloristRecipe, Frame, Grid, JobRecipe,
    Title, ToolUsage, chance_key, chance_label, compactify, consolidate, consolidate_needed,
    enchant_levels, is_loot_based, job_category_id,
};

/// A value the port must keep identical to Java's, and the Java line that says
/// so. A constant without its source is a constant nobody can check, so none of
/// the ones this module exports is bare — [`PROVENANCE`] is the list, and a test
/// refuses an entry that does not name a `.java` line.
///
/// (The list is written from the constants themselves, so removing a constant
/// breaks the build here instead of leaving a stale manifest behind.)
///
/// The `source` field is a `&'static str` rather than a parsed location because
/// the interesting part is often a *range* or two files at once
/// (`GenericRecipeCategory.java:344, FishermanRecipeCategory.java:226`), which no
/// `file:line` type can hold without losing the second half.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Documented<T> {
    pub value: T,
    pub source: &'static str,
}

impl<T> Documented<T> {
    pub const fn new(value: T, source: &'static str) -> Self {
        Documented { value, source }
    }
}

/// Every constant this module exports, with its Java line: the manifest a test
/// walks. Built from the constants, so a deleted constant is a compile error here
/// rather than a stale row, and a *new* constant that skips the list is caught by
/// `PROVENANCE.len()`.
pub const PROVENANCE: &[(&str, &str)] = &[
    ("PARTIAL_JEI_INFO", recipes::PARTIAL_JEI_INFO.source),
    ("MAX_BUILDING_LEVEL", recipes::MAX_BUILDING_LEVEL.source),
    ("CONSOLIDATE_ABOVE", recipes::CONSOLIDATE_ABOVE.source),
    ("MAX_LOOT_SLOTS", recipes::MAX_LOOT_SLOTS.source),
    ("WIDTH", recipes::WIDTH.source),
    ("HEIGHT", recipes::HEIGHT.source),
    ("CITIZEN_H", recipes::CITIZEN_H.source),
    ("ENCHANTMENTS", recipes::ENCHANTMENTS.source),
    ("BORDER_MINIMAP_DEFAULT", markers::BORDER_MINIMAP_DEFAULT.source),
    ("BORDER_LOADED_DEFAULT", markers::BORDER_LOADED_DEFAULT.source),
    ("DEATHPOINTS_DEFAULT", markers::DEATHPOINTS_DEFAULT.source),
    ("RAIDER_DEFAULT", markers::RAIDER_DEFAULT.source),
    ("INFO_SLOT_PERIOD", markers::INFO_SLOT_PERIOD.source),
    ("AUDIT_NAMES", audit::AUDITS.source),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The list [`PROVENANCE`] promises, walked: every exported constant names a
    /// `.java` line, and the count is the real one.
    #[test]
    fn every_exported_constant_names_its_line() {
        assert_eq!(PROVENANCE.len(), 14, "a constant was added without its row");
        for (name, source) in PROVENANCE {
            assert!(source.contains(".java:"), "{name} has no .java line: {source}");
        }
    }

    #[test]
    fn a_documented_value_keeps_both_halves() {
        let (width, height) = (recipes::WIDTH, recipes::HEIGHT);
        assert_eq!(width.value, 167);
        assert_eq!(height.value, 120);
        assert_eq!(width.source, "JobBasedRecipeCategory.java:81");
        assert_eq!(recipes::MAX_BUILDING_LEVEL.value, 5);
        assert_eq!(
            recipes::PARTIAL_JEI_INFO.value,
            "com.minecolonies.coremod.jei."
        );
    }

    #[test]
    fn the_partial_keys_build_the_keys_java_builds() {
        let partial = recipes::PARTIAL_JEI_INFO.value;
        // `PARTIAL_JEI_INFO + "tools"` (`ToolRecipeCategory.java:71`).
        assert_eq!(
            format!("{partial}tools"),
            "com.minecolonies.coremod.jei.tools"
        );
        // `PARTIAL_JEI_INFO + "onelevelrestriction"` (`ToolRecipeCategory.java:133`).
        assert_eq!(
            format!("{partial}onelevelrestriction"),
            "com.minecolonies.coremod.jei.onelevelrestriction"
        );
    }
}
