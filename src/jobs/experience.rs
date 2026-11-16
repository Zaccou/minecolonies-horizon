//! The experience curve: how much XP one skill level costs, and what a grant does.
//!
//! Java reference: `core/util/ExperienceUtils.java` (94 lines, two live call
//! sites), `core/entity/citizen/citizenhandlers/CitizenSkillHandler.java:180-240`
//! (the loop) and `CitizenExperienceHandler.java:73-125` (where a worker's XP
//! comes from).
//!
//! **There is no per-job curve, and that is the finding.** A [`Job`](super::Job)
//! carries no experience in Java: `IJob` has no XP method at all, only
//! `onLevelUp()` (`api/colony/jobs/IJob.java:112-118`). The XP goes to the **two
//! skills** the job's building module names, and every job shares one curve:
//!
//! ```java
//! data.getCitizenSkillHandler().addXpToSkill(primary, localXp, data);
//! ... addXpToSkill(primary.getComplimentary(), localXp / (100.0 / 10), data);
//! ... removeXpFromSkill(primary.getAdverse(),  localXp / (100.0 / 10), data);
//! ... addXpToSkill(secondary, localXp / 2.0, data);
//! ```
//! (`CitizenExperienceHandler.java:103-124`). A farmer and a knight at the same
//! level need the same XP; only the skills differ. So this file is one function
//! and one loop, and the job side of it is in [`super::employment`] — the two
//! skills of each module.
//!
//! **The one formula**, `ExperienceUtils.java:68-77`:
//!
//! ```text
//! cost(level) = max(1, 1 + 5 * level + 0.005 * level³)
//! ```
//!
//! Thousands of a cube: the curve is nearly linear for the levels a colony
//! reaches and turns cubic only at the top (level 20 costs 141, level 50 costs
//! 834, level 99 costs 5 348).
//!
//! **What is not ported.** `ExperienceUtils.calculateLevel` (`:79-93`) and
//! `getPercentOfLevelCompleted` (`:25-44`) are **dead code**: `ExperienceUtils.`
//! has exactly two call sites in the whole tree, both `getXPNeededForNextLevel`
//! (`CitizenSkillHandler.java:199`, `:235`). `calculateLevel` also disagrees with
//! the live loop — it compares one level's cost against *total* XP, as if the
//! curve were cumulative — so porting it would ship a second, wrong answer to a
//! question [`gain`] already answers.

use crate::building::MAX_BUILDING_LEVEL;

/// `CitizenConstants.MAX_CITIZEN_LEVEL`: no skill goes past 99
/// (`api/util/constant/CitizenConstants.java:181`, enforced at
/// `CitizenSkillHandler.java:149`, `:176-177`, `:190`).
pub const MAX_CITIZEN_LEVEL: i32 = 99;

/// The `10` of `(citizenHutLevel + 1) * 10`: how many skill levels one home
/// level buys.
pub const LEVELS_PER_HOME_LEVEL: i32 = 10;

/// `ExperienceUtils.EXPERIENCE_MULTIPLIER`, the `1D` in the formula
/// (`ExperienceUtils.java:11`). Kept as a named constant even though it is `1`:
/// a data pack that changes it changes the whole curve.
const EXPERIENCE_MULTIPLIER: f64 = 1.0;

/// The XP needed to go from `level` to `level + 1`
/// (`ExperienceUtils.java:68-77`).
///
/// Faithful to the last branch: `level <= 0` costs `1`, and the cubic term is
/// `0.005 * level³` computed in `f64`. A negative level is not an error, it is
/// "before the first level", which the formula covers.
pub fn xp_needed_for_next_level(level: i32) -> f64 {
    if level <= 0 {
        return 1.0;
    }
    let level = level as f64;
    let cost = 1.0 + EXPERIENCE_MULTIPLIER * 5.0 * level + 0.005 * (level * level * level);
    if cost < 1.0 { 1.0 } else { cost }
}

/// The XP a citizen must have earned in one skill to **reach** `level` from
/// zero: the sum of every level below it.
///
/// This is the port's own reading of the same formula, not a Java function —
/// `ExperienceUtils` only ever asks for one level at a time. It is what a table
/// of paliers needs, and what [`gain`] would produce if it were fed the total at
/// once. Summing `cost(0..level)` gives `1` for level 1, `55.5` for level 5,
/// `245.125` for level 10 and `142 015.005` for the cap.
pub fn total_xp_for_level(level: i32) -> f64 {
    let mut total = 0.0;
    let mut current = 0;
    while current < level {
        total += xp_needed_for_next_level(current);
        current += 1;
    }
    total
}

/// What one XP grant did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Progress {
    /// The level after the grant.
    pub level: i32,
    /// The XP carried inside the current level: always **below**
    /// `xp_needed_for_next_level(level)` for a normal grant, and Java stores
    /// exactly this (`CitizenSkillHandler.java:202`).
    pub into_level: f64,
    /// How many levels the grant bought. `0` is the common case.
    pub gained: i32,
}

/// What a granted `xp` does to a skill at `level` holding `into_level`, stopping
/// at `cap` — the loop of `CitizenSkillHandler.java:196-210`.
///
/// **The cap is checked once, before the loop, exactly as Java does it**
/// (`:189-193`): a skill already at or past its cap gains nothing. A single grant
/// is therefore able to **overshoot** the cap — a 1 000-XP grant to a level-19
/// citizen with a home gate of 20 lands at level 25, because the loop never looks
/// at the gate again, and a million XP takes a level-0 citizen past 99. That is
/// Java's behaviour and the port keeps it: the gate is a per-grant admission, not
/// a ceiling, and the *next* grant is the one that is refused. [`total_xp_for_level`]
/// is how a caller asks "which level should this XP have bought" without the
/// overshoot.
///
/// **The two refusals this port adds, both named in the report.**
///
/// - a negative or zero grant changes nothing (Java's `while (xpToLevelUp > 0)`
///   never runs, and it also leaves `experience` untouched — the field is only
///   written inside the loop, `:202`);
/// - a **non-finite** grant changes nothing. Java would subtract a finite cost
///   from `inf` forever and hang the server thread; a wasm mod has no thread to
///   hang, so the port answers instead. A finite grant always terminates: the
///   cost grows with the cube of the level.
pub fn gain(level: i32, into_level: f64, xp: f64, cap: i32) -> Progress {
    let cap = cap.min(MAX_CITIZEN_LEVEL);
    let unchanged = Progress {
        level,
        into_level,
        gained: 0,
    };
    if xp <= 0.0 || !xp.is_finite() || level >= cap {
        return unchanged;
    }

    let mut level = level;
    let mut left = into_level + xp;
    while left > 0.0 {
        let cost = xp_needed_for_next_level(level);
        if cost > left {
            return Progress {
                level,
                into_level: left,
                gained: level - unchanged.level,
            };
        }
        left -= cost;
        level += 1;
    }
    Progress {
        level,
        into_level: 0.0,
        gained: level - unchanged.level,
    }
}

/// The highest level a citizen's skill may reach, from the **home** hut
/// (`CitizenSkillHandler.java:184-193`).
///
/// ```java
/// if (((citizenHutLevel < citizenHutMaxLevel || citizenHutMaxLevel < MAX_BUILDING_LEVEL)
///       && (citizenHutLevel + 1) * 10 <= skillData.level)
///     || skillData.level >= MAX_CITIZEN_LEVEL) return;
/// ```
///
/// Read as a rule: a citizen learns `10` levels per home level (level 1 home → 20,
/// level 4 home → 50) and is **unbounded** by the house only when that house is
/// itself at the top of its own range and that range is the standard five. No
/// home at all is level `0`, so a homeless citizen stops at level 9 — which is
/// what makes housing matter for a colony's skills, and why this is a rule and
/// not a constant.
///
/// The answer is the level at which XP stops: [`gain`] refuses when
/// `level >= cap`, so the last level a citizen can *reach* is `cap - 1` — except
/// when `cap` is [`MAX_CITIZEN_LEVEL`], which Java checks with `>=` on the level
/// itself.
pub fn skill_cap(home_level: i32, home_max_level: i32) -> i32 {
    if home_level < home_max_level || home_max_level < MAX_BUILDING_LEVEL {
        (home_level + 1) * LEVELS_PER_HOME_LEVEL
    } else {
        MAX_CITIZEN_LEVEL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One test per palier, generated from the table so a wrong number names its
    /// own level. The values are Java's, computed from
    /// `1 + 5 * level + 0.005 * level³` in `f64` (`ExperienceUtils.java:75-76`).
    macro_rules! palier_tests {
        ($($name:ident: $level:expr => $xp:expr,)*) => {
            $(
                #[test]
                fn $name() {
                    let got = xp_needed_for_next_level($level);
                    assert!(
                        (got - $xp).abs() < 1e-9,
                        "level {} costs {} but the port answers {}",
                        $level, $xp, got
                    );
                }
            )*
        };
    }

    palier_tests! {
        palier_00_is_one: 0 => 1.0,
        palier_01_is_six: 1 => 6.005,
        palier_02: 2 => 11.04,
        palier_03: 3 => 16.135,
        palier_04: 4 => 21.32,
        palier_05: 5 => 26.625,
        palier_06: 6 => 32.08,
        palier_07: 7 => 37.715,
        palier_08: 8 => 43.56,
        palier_09: 9 => 49.645,
        palier_10_is_fifty_six: 10 => 56.0,
    }

    #[test]
    fn the_curve_keeps_growing_and_the_top_paliers_are_the_javas() {
        for level in 20..=30 {
            assert!(
                xp_needed_for_next_level(level) > xp_needed_for_next_level(level - 1),
                "level {level} must cost more than {}",
                level - 1
            );
        }
        assert!((xp_needed_for_next_level(20) - 141.0).abs() < 1e-9);
        // 876, not 834.245: the cubic term is 0.005 * 125 000 = 625, and 834.245
        // is what level **49** costs (`ExperienceUtils.java:75-76`).
        assert!((xp_needed_for_next_level(50) - 876.0).abs() < 1e-9);
        assert!((xp_needed_for_next_level(49) - 834.245).abs() < 1e-9);
        assert!((xp_needed_for_next_level(99) - 5347.495).abs() < 1e-9);
        // A negative level is "before the first", and costs the same as level 0.
        assert_eq!(xp_needed_for_next_level(-3), 1.0);
    }

    #[test]
    fn the_total_table_agrees_with_the_level_cost_one_level_at_a_time() {
        assert_eq!(total_xp_for_level(0), 0.0);
        assert!((total_xp_for_level(1) - 1.0).abs() < 1e-9);
        assert!((total_xp_for_level(5) - 55.5).abs() < 1e-9);
        assert!((total_xp_for_level(10) - 245.125).abs() < 1e-9);
        assert!((total_xp_for_level(20) - 1150.5).abs() < 1e-9);
        assert!((total_xp_for_level(99) - 142015.005).abs() < 1e-9);
        // The sum is exactly the walk: reaching level 99 is 98 level costs, not
        // 99 — level 1 is where a citizen starts, and level 0 costs one XP.
        let walked: f64 = (0..99).map(xp_needed_for_next_level).sum();
        assert!((total_xp_for_level(99) - walked).abs() < 1e-9);
    }

    #[test]
    fn a_small_grant_stays_inside_its_level_and_a_whole_one_crosses() {
        let almost = gain(1, 0.0, 5.0, MAX_CITIZEN_LEVEL);
        assert_eq!(
            almost,
            Progress {
                level: 1,
                into_level: 5.0,
                gained: 0
            }
        );
        // 6.005 is the whole level and one hundredth is left over for the next.
        let crossed = gain(1, 0.0, 7.005, MAX_CITIZEN_LEVEL);
        assert_eq!(crossed.level, 2);
        assert_eq!(crossed.gained, 1);
        assert!((crossed.into_level - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_grant_can_cross_several_levels_and_stops_below_the_next_cost() {
        // 20 XP from zero: 1 for level 0, 6.005 for level 1, 11.04 for level 2,
        // and 1.955 left over inside level 3.
        let three = gain(0, 0.0, 20.0, MAX_CITIZEN_LEVEL);
        assert_eq!(three.level, 3);
        assert_eq!(three.gained, 3);
        assert!((three.into_level - 1.955).abs() < 1e-9);
        assert!(three.into_level < xp_needed_for_next_level(3));
        // A million XP is not clamped to the cap inside the loop — it takes a
        // level-0 citizen well past 99, which is what Java's loop does too. What
        // stops it is the *next* grant: level >= cap, so nothing is admitted.
        let huge = gain(0, 0.0, 1_000_000.0, MAX_CITIZEN_LEVEL);
        assert!(huge.level > MAX_CITIZEN_LEVEL, "the loop does not clamp");
        assert_eq!(
            gain(huge.level, huge.into_level, 10.0, MAX_CITIZEN_LEVEL),
            Progress {
                level: huge.level,
                into_level: huge.into_level,
                gained: 0
            }
        );
    }

    #[test]
    fn the_cap_is_admission_not_a_ceiling_and_one_grant_can_overshoot_it() {
        // Level 19 with a home gate of 20 gains 1 000 XP: the loop never looks at
        // the gate again and stops at level 25 with 45.205 carried, which is what
        // `CitizenSkillHandler.java:196-210` does.
        let overshoot = gain(19, 0.0, 1000.0, 20);
        assert_eq!(overshoot.level, 25);
        assert!((overshoot.into_level - 45.205).abs() < 1e-9);
        // At the gate already, nothing is admitted at all.
        assert_eq!(
            gain(20, 3.0, 1000.0, 20),
            Progress {
                level: 20,
                into_level: 3.0,
                gained: 0
            }
        );
        // The hard cap is checked the same way and wins over a looser gate.
        assert_eq!(
            gain(MAX_CITIZEN_LEVEL, 0.0, 10.0, 500),
            Progress {
                level: MAX_CITIZEN_LEVEL,
                into_level: 0.0,
                gained: 0
            }
        );
    }

    #[test]
    fn a_grant_of_nothing_or_of_nonsense_changes_nothing() {
        let still = Progress {
            level: 4,
            into_level: 2.0,
            gained: 0,
        };
        assert_eq!(gain(4, 2.0, 0.0, MAX_CITIZEN_LEVEL), still, "a zero grant");
        assert_eq!(
            gain(4, 2.0, -50.0, MAX_CITIZEN_LEVEL),
            still,
            "a losing grant"
        );
        // Java would spin on `inf`; the port answers instead.
        assert_eq!(gain(4, 2.0, f64::INFINITY, MAX_CITIZEN_LEVEL), still);
        assert_eq!(gain(4, 2.0, f64::NAN, MAX_CITIZEN_LEVEL), still);
    }

    #[test]
    fn the_home_hut_is_what_stops_a_skill_from_learning_forever() {
        // No home at all: level 0, so lessons stop at 9 (`(0 + 1) * 10`).
        assert_eq!(skill_cap(0, MAX_BUILDING_LEVEL), 10);
        assert_eq!(skill_cap(1, MAX_BUILDING_LEVEL), 20);
        assert_eq!(skill_cap(4, MAX_BUILDING_LEVEL), 50);
        // A home at the top of the standard range opens the hard cap.
        assert_eq!(
            skill_cap(MAX_BUILDING_LEVEL, MAX_BUILDING_LEVEL),
            MAX_CITIZEN_LEVEL
        );
        // A home whose own range is not the standard one never opens it, even at
        // that range's top: `citizenHutMaxLevel < MAX_BUILDING_LEVEL` is true.
        assert_eq!(skill_cap(3, 3), 40);
        // `(10, 10)` does **not** open a ninth slot: the first half of Java's
        // condition is `10 < 10`, which is false, and the second half `10 < 5` is
        // false too, so the rule falls through to the hard cap
        // (`CitizenSkillHandler.java:189-193`).
        assert_eq!(skill_cap(10, 10), MAX_CITIZEN_LEVEL);
        // Above 99 is reachable, but only through the *first* branch: a widened
        // range whose home is still below its own top gives `(9 + 1) * 10`.
        assert_eq!(skill_cap(9, 10), 100);
        // The gate a level-1 home gives is exactly what makes housing matter: a
        // citizen at level 1 still learns, at 19 one more level is no longer
        // within one level's worth of XP, and at 20 nothing is admitted at all.
        let gate = skill_cap(1, MAX_BUILDING_LEVEL);
        assert_eq!(gate, 20);
        assert_eq!(gain(1, 0.0, 6.005, gate).gained, 1);
        assert_eq!(gain(19, 0.0, 6.005, gate).gained, 0);
        assert_eq!(gain(20, 0.0, 6.005, gate).gained, 0);
    }
}
