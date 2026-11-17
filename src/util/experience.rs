//! `ExperienceUtils`: the two functions the live experience code does not call.
//!
//! Java reference: `core/util/ExperienceUtils.java` (94 lines). **The curve is
//! not here** — it lives in [`crate::jobs::experience`], which ports
//! `getXPNeededForNextLevel` (`:68-77`) and the level-up loop of
//! `CitizenSkillHandler.java:196-210`. This file is the two functions that loop
//! never calls, ported because the file they sit in is part of this mission and
//! because both are *wrong* in a way that is worth writing down instead of
//! leaving for the next reader to rediscover.
//!
//! # Both are dead code, and one of them is also incorrect
//!
//! `ExperienceUtils` has exactly **two** call sites in the whole Java tree, both
//! `getXPNeededForNextLevel`:
//!
//! ```text
//! CitizenSkillHandler.java:199   final double nextLevel = ExperienceUtils.getXPNeededForNextLevel(skillData.level);
//! CitizenSkillHandler.java:235   skillData.experience = ExperienceUtils.getXPNeededForNextLevel(skillData.level - 1);
//! ```
//!
//! Neither `getPercentOfLevelCompleted` (`:32-44`) nor `calculateLevel`
//! (`:85-93`) is called by anything. That alone would be a reason to skip them,
//! but they are *also* built on a reading of `experience` that the live code
//! contradicts:
//!
//! - `CitizenSkillHandler` stores `skillData.experience` as the XP **inside the
//!   current level** — the loop compares it against one level's cost and, when
//!   it pays, subtracts and increments (`:196-209`), and on the way down it
//!   *sets* it to the cost of the level below (`:235`). That is an into-level
//!   counter.
//! - `getPercentOfLevelCompleted` computes `lastLvlExp = cost(level) -
//!   costOfOnly(level)` — the cumulative cost of everything below `level` — and
//!   subtracts it from `experience`. That reads `experience` as **cumulative**.
//!
//! So the two functions are not exports of the same quantity: fed the field that
//! actually holds XP, `getPercentOfLevelCompleted` returns a negative percentage
//! for every level above 0, and `calculateLevel` returns a level that is not the
//! level the citizen is. Both are ported **as written** — the port's job is to
//! be diffable against Java — and the tests pin the disagreement so that nobody
//! "repairs" one of them by accident. The honest answer to "how far into this
//! level am I" is `into_level / xp_needed_for_next_level(level)` over the
//! [`crate::jobs::experience::Progress`] the live loop returns, and the honest
//! inverse of the curve is [`crate::jobs::experience::total_xp_for_level`].

use crate::jobs::experience::{MAX_CITIZEN_LEVEL, xp_needed_for_next_level};

/// The `100D` of `ExperienceUtils.java:16`: the denominator of a percentage.
pub const PERCENT_MULTIPLIER: f64 = 100.0;

/// `Math.min` on two `f64`, including the one case Rust gets differently.
///
/// `f64::min` **ignores** a NaN operand and returns the other one; Java's
/// `Math.min` propagates it (`Math.min(100, NaN) == NaN`). The difference is
/// reachable here: a negative `level` makes [`xp_for_only_level`] zero, the
/// percentage divides by it, and the result is NaN at a level that is exactly a
/// boundary. Reproducing Java's answer is the whole point of the file.
fn java_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a < b {
        a
    } else {
        b
    }
}

/// The XP the **level `level` alone** costs:
/// `getXPNeededForOnlyLevel` (`ExperienceUtils.java:52-61`).
///
/// ```java
/// if (currentLevel == 0) return getXPNeededForNextLevel(currentLevel);
/// return getXPNeededForNextLevel(currentLevel) - getXPNeededForNextLevel(currentLevel - 1);
/// ```
///
/// At level 0 the two branches agree anyway — `cost(-1)` is `1` and `cost(0)` is
/// `1` — so the special case is a guard against nothing; it is kept because a
/// data pack that changes the multiplier would separate them.
///
/// A **negative** level gives `0`: `cost` is flat at `1` below level 1
/// (`:68-77`), so the difference cancels. Java then divides by that zero in
/// [`percent_of_level_completed`]; so does the port.
pub fn xp_for_only_level(level: i32) -> f64 {
    if level == 0 {
        return xp_needed_for_next_level(0);
    }
    xp_needed_for_next_level(level) - xp_needed_for_next_level(level - 1)
}

/// How much of `level` is done, in percent, capped at 100:
/// `getPercentOfLevelCompleted` (`ExperienceUtils.java:32-44`).
///
/// ```java
/// final double thisLvlExp = getXPNeededForOnlyLevel(level);
/// final double lastLvlExp = getXPNeededForNextLevel(level) - thisLvlExp;
/// final double currentExp = experience - lastLvlExp;
/// return Math.min(100, 100 - ((thisLvlExp - currentExp) / thisLvlExp) * 100);
/// ```
///
/// **`experience` is read as cumulative XP**, and that is the bug: the field of
/// the same name in `CitizenSkillHandler` is the XP *inside* the level. See the
/// module note. This function is ported because it exists, not because it is
/// right.
///
/// The three answers Java can give, all kept:
///
/// - `0.0` at the bottom of a level (`experience == the cumulative cost below
///   it`), `100.0` at its top;
/// - `100.0` for anything past the top — the `Math.min` is a cap, not a check;
/// - **negative** below the bottom, and **NaN** at a negative level (0/0). No
///   caller exists to care, and inventing a clamp would be inventing a rule.
pub fn percent_of_level_completed(experience: f64, level: i32) -> f64 {
    let this_level = xp_for_only_level(level);
    let last_level = xp_needed_for_next_level(level) - this_level;
    let current = experience - last_level;
    let percent = PERCENT_MULTIPLIER
        - ((this_level - current) / this_level) * PERCENT_MULTIPLIER;
    java_min(PERCENT_MULTIPLIER, percent)
}

/// The level a **cumulative** XP total corresponds to:
/// `calculateLevel` (`ExperienceUtils.java:85-93`).
///
/// ```java
/// int startLevel = 0;
/// while (getXPNeededForNextLevel(startLevel) < xp) startLevel++;
/// return startLevel;
/// ```
///
/// This compares **one level's cost** against the whole total, so its answer is
/// the level whose *single* cost first exceeds the total — not the level the
/// total buys. `total_xp_for_level(5)` is `55.5`, and `cost(10)` is `56`, so this
/// function answers `10`; the citizen that total actually produces is level 5.
/// The test `calculate_level_disagrees_with_the_curve_it_inverts` pins that.
///
/// **Two refusals, both the port's, both named.**
///
/// - `None` for a **non-finite** `xp`: Java's loop never ends (`inf` is greater
///   than every finite cost), which on a server thread is a hang, not an
///   exception.
/// - `None` for an `xp` above `cost(99) = 5 347.495`: Java's loop would run past
///   the level cap and eventually wrap `startLevel`, and no citizen can be past
///   [`MAX_CITIZEN_LEVEL`] anyway (`CitizenSkillHandler.java:181`).
///
/// Everything else terminates within 99 iterations, because `cost` grows with
/// the cube of the level.
pub fn calculate_level(xp: f64) -> Option<i32> {
    let ceiling = xp_needed_for_next_level(MAX_CITIZEN_LEVEL);
    if !xp.is_finite() || xp > ceiling {
        return None;
    }
    let mut level = 0;
    while level < MAX_CITIZEN_LEVEL && xp_needed_for_next_level(level) < xp {
        level += 1;
    }
    Some(level)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::experience::{Progress, gain, total_xp_for_level};

    /// The curve's own values (`:75-76`), so a test that fails names the level it
    /// disagrees about.
    #[test]
    fn the_level_only_costs_are_the_differences_of_the_curve() {
        assert_eq!(xp_for_only_level(0), 1.0);
        assert!((xp_for_only_level(1) - 5.005).abs() < 1e-9);
        assert!((xp_for_only_level(2) - 5.035).abs() < 1e-9);
    }

    /// `cost(10) - cost(9)` is `56 - 49.645`, checked against the curve itself so
    /// a change in `jobs::experience` fails here rather than silently.
    #[test]
    fn level_ten_costs_the_curve_stepped_once() {
        let stepped = xp_needed_for_next_level(10) - xp_needed_for_next_level(9);
        assert!((stepped - 6.355).abs() < 1e-9);
        assert!((xp_for_only_level(10) - stepped).abs() < 1e-9);
    }

    /// Below level 1 the curve is flat at `1`, so the difference is `0` — and
    /// Java does not guard it.
    #[test]
    fn a_negative_level_costs_nothing_at_all() {
        assert_eq!(xp_for_only_level(-1), 0.0);
        assert_eq!(xp_for_only_level(-7), 0.0);
    }

    /// Level 0: the bottom is `0` XP, the top is `1`, and the answer is linear.
    #[test]
    fn level_zero_runs_from_zero_to_a_hundred_percent() {
        assert_eq!(percent_of_level_completed(0.0, 0), 0.0);
        assert_eq!(percent_of_level_completed(0.25, 0), 25.0);
        assert_eq!(percent_of_level_completed(0.5, 0), 50.0);
        assert_eq!(percent_of_level_completed(1.0, 0), 100.0);
    }

    /// Level 1: cumulative XP `1` is the bottom of the level, `6.005` its top,
    /// `3.5025` its middle.
    #[test]
    fn level_one_runs_from_its_cumulative_base_to_its_top() {
        assert!((percent_of_level_completed(1.0, 1) - 0.0).abs() < 1e-9);
        assert!((percent_of_level_completed(3.5025, 1) - 50.0).abs() < 1e-9);
        assert!((percent_of_level_completed(6.005, 1) - 100.0).abs() < 1e-9);
    }

    /// The cap is a `Math.min`, not a refusal: past the top the answer is 100.
    #[test]
    fn past_the_top_the_percentage_stops_at_a_hundred() {
        assert_eq!(percent_of_level_completed(50.0, 1), 100.0);
        assert_eq!(percent_of_level_completed(1e9, 0), 100.0);
    }

    /// Below the bottom it goes **negative** — nothing clamps it.
    #[test]
    fn below_the_bottom_the_percentage_is_negative() {
        assert!(percent_of_level_completed(0.0, 1) < 0.0);
    }

    /// The one place `java_min` earns its keep: `0/0` must stay NaN, where
    /// `f64::min` would have answered `100`.
    #[test]
    fn a_negative_level_makes_the_percentage_nan_not_a_hundred() {
        // `xp_for_only_level(-1)` is `0`, and `experience == 1` is the cumulative
        // cost below it, so the quotient is `0/0`.
        assert!(percent_of_level_completed(1.0, -1).is_nan());
    }

    /// A negative level with any other XP divides by zero into an infinity,
    /// which the cap then turns into `100`. Java gives the same, which is why the
    /// guard above tests `is_nan` and not `level < 0`.
    #[test]
    fn a_negative_level_away_from_the_pole_saturates_at_a_hundred() {
        assert_eq!(percent_of_level_completed(5.0, -1), 100.0);
    }

    /// `calculateLevel` compares one cost against the total, level by level.
    #[test]
    fn calculate_level_answers_the_first_cost_the_total_does_not_cover() {
        assert_eq!(calculate_level(0.0), Some(0));
        assert_eq!(calculate_level(1.0), Some(0)); // `cost(0) = 1` is not `< 1`
        assert_eq!(calculate_level(1.5), Some(1)); // `cost(1) = 6.005` is not `< 1.5`
        assert_eq!(calculate_level(6.005), Some(1)); // the comparison is strict
        assert_eq!(calculate_level(6.006), Some(2));
    }

    /// The bug, pinned: the total for level 5 buys level 10 from this function
    /// and stops at level 4 in the live loop.
    ///
    /// **Not 5**, and the missing level is arithmetic, not a rule: the loop
    /// subtracts one cost at a time (`CitizenSkillHandler.java:207`) and compares
    /// with a strict `>` (`:200`). In `f64`,
    /// `55.5 - 1 - 6.005 - 11.04 - 16.135` is `21.319999999999997`, a hair
    /// **under** `cost(4) = 21.32`, so `nextLevel > xpToLevelUp` fires one level
    /// early and the citizen keeps that `21.32`-minus-an-epsilon *inside* level 4.
    /// Java's `double` is the same IEEE-754 value in the same order, so Java
    /// answers 4 too.
    #[test]
    fn calculate_level_disagrees_with_the_curve_it_inverts() {
        let total_for_five = total_xp_for_level(5);
        assert!((total_for_five - 55.5).abs() < 1e-9);
        assert_eq!(calculate_level(total_for_five), Some(10));
        let Progress { level, into_level, .. } = gain(0, 0.0, total_for_five, MAX_CITIZEN_LEVEL);
        assert_eq!(level, 4);
        // The level 5 that the exact arithmetic promises is still in the
        // citizen's hand: the whole of `cost(4)`, short by the rounding.
        assert!((into_level - 21.32).abs() < 1e-9);
    }

    /// `inf` and `NaN` are refused instead of looping for ever.
    #[test]
    fn a_non_finite_total_is_refused() {
        assert_eq!(calculate_level(f64::INFINITY), None);
        assert_eq!(calculate_level(f64::NAN), None);
        assert_eq!(calculate_level(f64::NEG_INFINITY), None);
    }

    /// The top of the range is answerable and the next step past it is not.
    #[test]
    fn the_level_cap_is_the_last_answerable_total() {
        let ceiling = xp_needed_for_next_level(MAX_CITIZEN_LEVEL);
        assert!((ceiling - 5347.495).abs() < 1e-6);
        assert_eq!(calculate_level(ceiling), Some(MAX_CITIZEN_LEVEL));
        assert_eq!(calculate_level(ceiling + 0.001), None);
    }

    /// A negative total is not refused: `cost(0) = 1` is never `< xp`, so the
    /// answer is level 0. Java agrees, and `0` is the honest answer for "no XP".
    #[test]
    fn a_negative_total_answers_level_zero() {
        assert_eq!(calculate_level(-100.0), Some(0));
    }
}
