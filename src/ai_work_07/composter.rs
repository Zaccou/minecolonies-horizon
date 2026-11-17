//! The composter, as a **seam**: what `crate::ai::composter` already carries, and the one
//! rule it left as a comment.
//!
//! Java reference: `core/entity/ai/workers/production/agriculture/EntityAIWorkComposter
//! .java`.
//!
//! ## What was already ported, and is not written again
//!
//! `crate::ai::composter` carries the whole trade, with fourteen tests:
//!
//! | Java | There | What |
//! | --- | --- | --- |
//! | `:95-102` | `register` | the six targets, plus the `AI_BLOCKING` acceleration |
//! | `:109-132` | `barrel_acceleration_ticks` | `(primarySkill / 10) * 2` barrel ticks |
//! | `:139-196` | `get_materials` | the five exits of `getMaterials` |
//! | `:203-245` | `decide_what_to_do` | the two barrel scans, harvest before fill |
//! | `:252-294` | `fill_barrels` | one compostable into the barrel |
//! | `:301-351` | `harvest_barrels` | the finished barrel, one action counted |
//! | `:359-378` | `loot_multiplier` | the four factors on the barrel's six compost |
//! | `:380-384` | `JobProfile::COMPOSTER` | `actionsUntilDumping = 1` |
//! | `:390-402` | `complain` | one message a day while the list is empty |
//!
//! A second copy of those six targets here would be exactly the duplication this module
//! exists to avoid, and two tables for one trade drift. So [`ai`] **returns** that table
//! and this file adds nothing to it.
//!
//! ## The one rule that was missing
//!
//! `harvestBarrels`'s product choice (`:316-344`) was a comment in
//! `crate::ai::composter::harvest_barrels`: `getSetting(PRODUCE_DIRT)` decides between the
//! barrel's own compost and a dirt block, and a second draw decides **which** dirt —
//! podzol on `5 * (1 + PODZOL_CHANCE) / 100`, dirt otherwise. That is a pure function of a
//! setting, a draw and a research strength, and it is [`harvest_product`].
//!
//! ## Why the two engines cannot be one
//!
//! `crate::ai::composter::ai()` is a `crate::ai::worker::JobAi`, i.e. a
//! `Machine<WorkerState, crate::ai::worker::Scene>`; this module's machines are
//! `Machine<WorkState, WorkScene>`. `Machine<S, F>` is monomorphic in both parameters, so a
//! scene cannot hold both — [`super::WorkerAi`] is the enum that carries the two, and it is
//! the only place they meet.

use crate::ai::worker::JobAi;

/// `TileEntityBarrel.retrieveCompost`'s base amount (`core/tileentities/
/// TileEntityBarrel.java:322`): `(int) (6 * multiplier)` compost, before the multiplier.
pub const BASE_COMPOST: i32 = 6;

/// `BuildingComposter.PRODUCE_DIRT`'s default chance (`EntityAIWorkComposter.java:323`):
/// five percent, times `1 + PODZOL_CHANCE`.
pub const PODZOL_BASE_CHANCE: u32 = 5;

/// The one item a podzol or a dirt harvest gives (`:329`, `:335`): `new ItemStack(..., 1)`.
pub const DIRT_PER_HARVEST: u32 = 1;

/// What one finished barrel gave the worker (`:314-344`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three products of a harvest
pub enum HarvestProduct {
    /// `PRODUCE_DIRT` off: the barrel's own compost, `(int) (6 * multiplier)` of it
    /// (`:342-343` over `TileEntityBarrel.java:322`).
    Compost(u32),
    /// `PRODUCE_DIRT` on and the draw above the podzol threshold: one dirt (`:335`).
    Dirt(u32),
    /// `PRODUCE_DIRT` on and the draw at or below it: one podzol (`:329`).
    Podzol(u32),
}

impl HarvestProduct {
    /// The item's Java name, which is the statistic's key (`:330`, `:336`, `:342`).
    pub fn java_name(self) -> &'static str {
        match self {
            HarvestProduct::Compost(_) => "compost",
            HarvestProduct::Dirt(_) => "dirt",
            HarvestProduct::Podzol(_) => "podzol",
        }
    }

    /// How many of it.
    pub fn count(self) -> u32 {
        match self {
            HarvestProduct::Compost(count) | HarvestProduct::Dirt(count) => count,
            HarvestProduct::Podzol(count) => count,
        }
    }
}

/// `retrieveCompost(multiplier)`'s count: `(int) (6 * multiplier)`
/// (`TileEntityBarrel.java:322`), truncated towards zero like Java's cast.
pub fn compost_count(multiplier: f64) -> u32 {
    let scaled = BASE_COMPOST as f64 * multiplier;
    if scaled <= 0.0 {
        return 0;
    }
    scaled as u32
}

/// `harvestBarrels`'s product (`:316-344`), with the draw supplied by the caller.
///
/// `draw` is `worker.getRandom().nextInt(100) + 1`, i.e. **one to a hundred** (`:323`);
/// `podzol_research` is `getEffectStrength(PODZOL_CHANCE)`
/// (`api/research/util/ResearchConstants.java:133`), which two researches raise to `1`
/// and `2`. The comparison is `<=`, so a draw of exactly `5 * (1 + research)` is podzol.
pub fn harvest_product(
    produce_dirt: bool,
    draw: u32,
    podzol_research: f64,
    multiplier: f64,
) -> HarvestProduct {
    if !produce_dirt {
        // `:340-344` — the barrel's own compost, multiplied by `getLootMultiplier`.
        return HarvestProduct::Compost(compost_count(multiplier));
    }
    let threshold = (PODZOL_BASE_CHANCE as f64) * (1.0 + podzol_research);
    if (draw as f64) <= threshold {
        HarvestProduct::Podzol(DIRT_PER_HARVEST) // :329
    } else {
        HarvestProduct::Dirt(DIRT_PER_HARVEST) // :335
    }
}

/// The composter's AI, which is `crate::ai::composter`'s.
///
/// Not a re-implementation: a caller that wants all three trades asks
/// [`super::WorkTrade::ai`], and this is the branch it takes for the composter.
pub fn ai() -> JobAi {
    crate::ai::composter::ai()
}

/// The composter's base-table profile, re-exported so a caller does not have to know which
/// module owns it: `getActionsDoneUntilDumping() == 1` (`:380-384`).
pub const PROFILE: crate::ai::worker::JobProfile = crate::ai::composter::PROFILE;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dirt_setting_off_hands_the_barrels_own_compost_over() {
        // `:340-344` — the compost count comes from the barrel, six times the multiplier
        // (`TileEntityBarrel.java:322`).
        assert_eq!(harvest_product(false, 1, 0.0, 1.0), HarvestProduct::Compost(6));
        assert_eq!(
            harvest_product(false, 1, 0.0, 2.0),
            HarvestProduct::Compost(12),
            "the 2.0 factor from `getLootMultiplier`"
        );
        // Java's `(int)` cast truncates: 6 * 1.25 is 7.5, so seven.
        assert_eq!(
            harvest_product(false, 50, 0.0, 1.25),
            HarvestProduct::Compost(7)
        );
        assert_eq!(compost_count(1.5), 9);
        assert_eq!(compost_count(0.0), 0);
    }

    #[test]
    fn podzol_is_five_percent_and_each_research_adds_five() {
        // `:323` — `nextInt(100) + 1 <= 5 * (1 + PODZOL_CHANCE)`, draw in 1..=100.
        assert_eq!(harvest_product(true, 1, 0.0, 1.0), HarvestProduct::Podzol(1));
        assert_eq!(harvest_product(true, 5, 0.0, 1.0), HarvestProduct::Podzol(1));
        assert_eq!(harvest_product(true, 6, 0.0, 1.0), HarvestProduct::Dirt(1));
        // One research doubles it to ten percent, two triple it to fifteen.
        assert_eq!(harvest_product(true, 10, 1.0, 1.0), HarvestProduct::Podzol(1));
        assert_eq!(harvest_product(true, 11, 1.0, 1.0), HarvestProduct::Dirt(1));
        assert_eq!(harvest_product(true, 15, 2.0, 1.0), HarvestProduct::Podzol(1));
        assert_eq!(harvest_product(true, 16, 2.0, 1.0), HarvestProduct::Dirt(1));
        // A draw of zero is what `nextInt(100)` alone would give, and it is podzol too.
        assert_eq!(harvest_product(true, 0, 0.0, 1.0), HarvestProduct::Podzol(1));
    }

    #[test]
    fn the_composter_is_a_job_ai_and_not_a_work_table() {
        // `crate::ai::composter` owns the six targets (`EntityAIWorkComposter.java:95-102`)
        // and the profile (`:380-384`); this module only reaches them.
        let _ai: JobAi = ai();
        assert_eq!(PROFILE.actions_until_dumping, 1);
        assert_eq!(
            super::super::WorkTrade::Composter.actions_until_dumping(),
            PROFILE.actions_until_dumping
        );
        // And a composter has no table in this module's engine.
        assert!(super::super::WorkTrade::Composter.work_ai().is_none());
        assert!(super::super::states_of(super::super::WorkTrade::Composter).is_empty());
    }

    #[test]
    fn the_product_names_are_the_statistics_keys() {
        // `StatsUtil.trackStatByName(building, PRODUCT_COLLECTED, product.getItem()
        // .getDescriptionId(), compost.getCount())` (`:330`, `:336`, `:342`).
        assert_eq!(HarvestProduct::Podzol(1).java_name(), "podzol");
        assert_eq!(HarvestProduct::Dirt(1).java_name(), "dirt");
        assert_eq!(HarvestProduct::Compost(6).java_name(), "compost");
        assert_eq!(HarvestProduct::Compost(6).count(), 6);
    }
}
