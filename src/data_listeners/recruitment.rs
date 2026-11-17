//! `colony/recruitment_items/*.json`: what a recruit costs, and the tier a tavern
//! level can ask for that the table does not have.
//!
//! Java reference: `core/datalistener/RecruitmentItemsListener.java` (172 lines),
//! its one caller `core/colony/buildings/modules/TavernBuildingModule.java:194-206`,
//! and the 52 shipped files. Three constants rule the whole file:
//! `BASE_RECRUIT_LEVEL = 15` (`:41`), `MAX_RARITY = 9` (`:46`),
//! `BASE_ITEM_COUNT = 5` (`:51`).
//!
//! **Two formulas, and the nine boot rungs.**
//!
//! | Quantity | Formula | Line |
//! | --- | --- | --- |
//! | how many items | `5 · (10 − rarity)` — 45 at tier 1, 5 at tier 9 | `:158` |
//! | the visitor's starting job level | `15 + rarity² / 2` — 15 at tier 1, 55 at tier 9 | `:159` |
//! | the boots it wears | leather 1-2, gold 3-4, iron 5-6, diamond 7-8, netherite 9 | `:72-82` |
//!
//! **The refusal Java performs, and the one it forgets.** Two row-level checks are
//! explicit and named in the log: `Items.AIR` is refused (`:146-150`) and a rarity
//! outside 1..=9 is refused (`:152-156`). Both `continue`, so the table is built
//! without them. A whole empty pack is refused too, with the previous table kept
//! (`:134-138` — the only `return` that reads like a deliberate one).
//!
//! What is missing is the other direction: [`band`]. `getRandomRecruitCost` draws
//! a rarity from a gaussian around `limit/2` and clamps it to `limit − 1`, where
//! `limit = 3 · buildingLevel + 1` (`:96-104`). At tavern level 1 the band is
//! `1..=3`, at level 3 it is `1..=9` — exactly the table — and at level **4** it is
//! `1..=12`. `RECRUIT_COSTS.get(rarity)` is then `null` and the next call
//! (`recruitCostsAtTier.get(...)`, `:101`) throws a `NullPointerException`.
//!
//! **The null check that proves the intent.** `TavernBuildingModule.java:197-201`
//! writes `if (cost == null) return null;` — a guard for exactly this case, placed
//! where it can never trigger, because the callee throws before returning. So the
//! behaviour Java *meant* is "no tier, no visitor", and [`pick`] returns that as a
//! named [`Refusal::TierAbsent`]. At level 4 the draw is `P(z > 1.5) ≈ 6.7 %` per
//! attempt (mean 6.5, σ 2), so the tavern does not fail every time — it fails
//! sometimes, which is worse to diagnose.
//!
//! **What is not here**: the max-stack clamp, which is the caller's
//! (`TavernBuildingModule.java:205`: `min(maxStackSize, count + nextInt(3))`), and
//! `ColonyConstants.rand`, because the port takes the draw instead of making it.

use super::Refused;

/// `BASE_RECRUIT_LEVEL` (`:41`).
pub const BASE_RECRUIT_LEVEL: i32 = 15;
/// Its source, for the one constant a reader will want to move.
pub const BASE_RECRUIT_LEVEL_SOURCE: &str = "RecruitmentItemsListener.java:41";
/// `MAX_RARITY` (`:46`): the table has nine tiers, no more.
pub const MAX_RARITY: i32 = 9;
/// `MAX_RARITY_SOURCE`.
pub const MAX_RARITY_SOURCE: &str = "RecruitmentItemsListener.java:46";
/// `BASE_ITEM_COUNT` (`:51`).
pub const BASE_ITEM_COUNT: i32 = 5;
/// `BASE_ITEM_COUNT_SOURCE`.
pub const BASE_ITEM_COUNT_SOURCE: &str = "RecruitmentItemsListener.java:51";

/// One accepted row: a tier's item, how many, the level it starts the visitor at,
/// and the boots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four fields of `RecruitCost` (`:167-172`) plus the tier
pub struct Cost {
    pub rarity: i32,
    pub item: &'static str,
    pub count: i32,
    pub recruit_level: i32,
    pub boots: &'static str,
}

/// Why a recruitment row is refused, named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// `Items.AIR` — an id no item registry knows (`:146-150`).
    AirItem { id: &'static str },
    /// A rarity outside `1..=9` (`:152-156`).
    RarityOutOfRange { id: &'static str, rarity: i32 },
    /// The pack is empty. Java refuses it and keeps the previous table
    /// (`:134-138`); a visitor needs at least one row to be spawned at all.
    EmptyPack,
    /// A tier the table does not contain, asked for by `getRandomRecruitCost`
    /// (`:100-101`). Java throws; `TavernBuildingModule.java:197` shows the guard
    /// it meant to hit.
    TierAbsent { rarity: i32 },
}

impl Refused for Refusal {
    fn name(&self) -> String {
        match *self {
            Refusal::AirItem { id } => {
                format!("data_listeners.refusal.recruit_item_is_air({id})")
            }
            Refusal::RarityOutOfRange { id, rarity } => {
                format!("data_listeners.refusal.recruit_rarity_out_of_range({id}.{rarity})")
            }
            Refusal::EmptyPack => "data_listeners.refusal.recruit_pack_is_empty".to_string(),
            Refusal::TierAbsent { rarity } => {
                format!("data_listeners.refusal.recruit_tier_absent({rarity})")
            }
        }
    }
}

/// How many items a tier asks for: `5 · (10 − rarity)` (`:158`).
pub fn count(rarity: i32) -> i32 {
    BASE_ITEM_COUNT * (MAX_RARITY + 1 - rarity)
}

/// The job level a recruit of this tier starts at: `15 + rarity² / 2` (`:159`,
/// integer division, so tier 2 gives 17 and not 17.5).
pub fn recruit_level(rarity: i32) -> i32 {
    BASE_RECRUIT_LEVEL + rarity * rarity / 2
}

/// The boots of a tier, from `RARITY_TO_BOOT_MAP` (`:72-82`): two rungs each for
/// leather, gold, iron and diamond, then netherite alone.
pub fn boots(rarity: i32) -> &'static str {
    match rarity {
        1 | 2 => "minecraft:leather_boots",
        3 | 4 => "minecraft:golden_boots",
        5 | 6 => "minecraft:iron_boots",
        7 | 8 => "minecraft:diamond_boots",
        _ => "minecraft:netherite_boots",
    }
}

/// Reads one row, `known_item` being the registry's answer.
pub fn cost(id: &'static str, item: &'static str, rarity: i32, known_item: &dyn Fn(&str) -> bool) -> Result<Cost, Refusal> {
    if !known_item(item) {
        return Err(Refusal::AirItem { id });
    }
    if !(1..=MAX_RARITY).contains(&rarity) {
        return Err(Refusal::RarityOutOfRange { id, rarity });
    }
    Ok(Cost {
        rarity,
        item,
        count: count(rarity),
        recruit_level: recruit_level(rarity),
        boots: boots(rarity),
    })
}

/// The rarity band a tavern level can draw from: `1 ..= 3 · buildingLevel`
/// (`:96-104`, where `limit = 3 · level + 1` and the clamp is `limit − 1`).
///
/// **This is where a global table meets a per-colony number.** The nine tiers are
/// the same in every colony; how many of them a tavern can *reach* is not, and at
/// level 4 the band asks for tiers 10 to 12 that no pack may declare.
pub fn band(building_level: i32) -> (i32, i32) {
    (1, 3 * building_level)
}

/// The cost for one draw, refusing the tier the table lacks instead of throwing.
///
/// `rarity` is the clamped draw; the caller hands it in so the rule stays pure.
pub fn pick(table: &[Cost], rarity: i32) -> Result<Cost, Refusal> {
    table
        .iter()
        .find(|row| row.rarity == rarity)
        .copied()
        .ok_or(Refusal::TierAbsent { rarity })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known(_: &str) -> bool {
        true
    }

    fn table_of_nine() -> Vec<Cost> {
        (1..=MAX_RARITY)
            .map(|rarity| {
                cost("row", "minecraft:bread", rarity, &known).expect("tier 1..=9 is valid")
            })
            .collect()
    }

    #[test]
    fn the_item_count_falls_and_the_school_level_rises_with_the_tier() {
        assert_eq!(count(1), 45);
        assert_eq!(count(9), 5);
        assert_eq!(recruit_level(1), 15);
        assert_eq!(recruit_level(2), 17); // 15 + 4/2, integer division
        assert_eq!(recruit_level(9), 55);
        // Monotonic in both directions, over the whole table.
        for rarity in 2..=MAX_RARITY {
            assert!(count(rarity) < count(rarity - 1));
            assert!(recruit_level(rarity) > recruit_level(rarity - 1));
        }
    }

    #[test]
    fn the_boot_ladder_has_nine_rungs_and_four_pairs() {
        assert_eq!(boots(1), boots(2));
        assert_eq!(boots(3), boots(4));
        assert_eq!(boots(5), boots(6));
        assert_eq!(boots(7), boots(8));
        assert_ne!(boots(8), boots(9));
        assert_eq!(boots(9), "minecraft:netherite_boots");
    }

    #[test]
    fn an_unknown_item_is_refused_by_name() {
        let refusal = cost("tavern/x", "minecraft:air", 1, &|_| false).expect_err("air is refused");
        assert_eq!(
            refusal.name(),
            "data_listeners.refusal.recruit_item_is_air(tavern/x)"
        );
    }

    #[test]
    fn a_rarity_outside_the_nine_tiers_is_refused_by_name() {
        for rarity in [0, 10, 12] {
            let refusal = cost("tavern/x", "minecraft:bread", rarity, &known)
                .expect_err("1..=9 only");
            assert_eq!(
                refusal.name(),
                format!("data_listeners.refusal.recruit_rarity_out_of_range(tavern/x.{rarity})")
            );
        }
    }

    #[test]
    fn the_reachable_band_grows_by_three_a_level_and_overruns_the_table_at_four() {
        assert_eq!(band(1), (1, 3));
        assert_eq!(band(2), (1, 6));
        assert_eq!(band(3), (1, 9)); // exactly the table
        assert_eq!(band(4), (1, 12)); // three tiers no pack declares
        assert_eq!(band(5), (1, 15));
    }

    #[test]
    fn a_level_n_the_tavern_is_refused_where_java_throws() {
        let table = table_of_nine();
        assert_eq!(pick(&table, 9).map(|row| row.recruit_level), Ok(55));
        let refusal = pick(&table, 12).expect_err("TavernBuildingModule:197 expects a null here");
        assert_eq!(
            refusal.name(),
            "data_listeners.refusal.recruit_tier_absent(12)"
        );
        // And the whole band of level 4 is servable only where a pack declares it.
        let (_, high) = band(4);
        assert!(high > MAX_RARITY);
    }
}
