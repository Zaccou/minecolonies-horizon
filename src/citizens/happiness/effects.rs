//! What a happiness number actually changes — and what it does **not**.
//!
//! Java reference: `core/entity/citizen/citizenhandlers/CitizenSkillHandler.java:158`
//! and `:92`, `core/colony/CitizenData.java:528`, `core/colony/Colony.java:1438-1452`,
//! `api/util/SoundUtils.java:135`, and the two GUI reads
//! (`WindowCitizenPage.java:143`, `CitizenWindowUtils.java:304`).
//!
//! **Four effects exist in this tree, and that is the whole list.** Every one of
//! them was found by reading the six call sites of `getHappiness` /
//! `getOverallHappiness`; nothing else in MineColonies consults the number.
//!
//! | Effect | Rule | Java |
//! | --- | --- | --- |
//! | a ceiling on learning | `skillMap[Intelligence].level < (int)happiness · 9` gates the intelligence tick | `CitizenSkillHandler.java:154-164` |
//! | a starting ceiling | a newborn starts at `(int)overall · 2`, floored to 5 while the colony is still filling | `CitizenData.java:526-538` |
//! | a breeding ceiling | a child's skills are capped at `(int)overall` | `CitizenSkillHandler.java:91-92` |
//! | a complaint | below `5`, an idle citizen plays the unhappy sound instead of the happy one | `SoundUtils.java:135` |
//!
//! **The brief asks at which level a citizen refuses to work or leaves the
//! colony. Neither rule exists.** The evidence, because a negative needs one:
//!
//! - the citizen's state machine (`CitizenAI.calculateNextState`,
//!   `:134-261`, ported in `crate::citizens::priority`) reads health, hunger,
//!   weather, raid, mourning and the clock — **never** happiness;
//! - `removeCivilian` has two callers in the tree: `EntityCitizen.die`
//!   (`:1620`, a death) and the **visitor** manager (`VisitorManager.java:305`);
//! - no `getHappiness()` result is ever compared against a threshold other than
//!   the `5` of the unhappy sound, and the two `(int)` casts.
//!
//! So a colony at happiness `0` keeps working exactly as fast as one at `10`,
//! except that its citizens learn nine times fewer intelligence levels and
//! complain out loud. That is a real cost and a different one from the one the
//! brief expected.
//!
//! **`(int)` is a rule, not a formality.** Java casts the `double` before using
//! it, and `(int)` on a `NaN` is `0`, not an error — so the `NaN` the aggregate
//! can produce ([`super::Happiness::total`]) silently becomes "no ceiling at
//! all". [`as_int`] spells that out instead of hiding it behind `as`.

/// `Colony.getOverallHappiness`'s answer for a colony with nobody in it
/// (`Colony.java:1439-1443`): `5.5`, the only hard-coded happiness in the tree.
pub const EMPTY_COLONY_HAPPINESS: f64 = 5.5;

/// The `9` of `(int)happiness * 9` (`CitizenSkillHandler.java:159`): how many
/// intelligence levels one point of happiness buys.
pub const INTELLIGENCE_PER_HAPPINESS: i32 = 9;

/// The `2` of `(int)overall * 2` (`CitizenData.java:528`).
pub const STARTING_PER_HAPPINESS: i32 = 2;

/// The floor a colony's first citizens get (`CitizenData.java:530-533`).
pub const STARTING_FLOOR: i32 = 5;

/// The line under which an idle citizen complains (`SoundUtils.java:135`).
pub const COMPLAINT_LINE: f64 = 5.0;

/// The research effect that scales happiness
/// (`ResearchConstants.HAPPINESS`, `effects/happinessmultiplier`), read as
/// `crate::research::Effects::strength(HAPPINESS_EFFECT)` — the port's ids drop
/// the `effects/` prefix (`research/effect.rs:114-160`).
///
/// [`super::Happiness::total`] takes the strength as a number, the same way
/// `needs::Advance` takes the regeneration and saturation-limit strengths: this
/// module stays free of the research tree, and one test below proves the id the
/// caller is told to read really exists.
pub const HAPPINESS_EFFECT: &str = "happinessmultiplier";

/// The colony's happiness: the mean of its citizens', and `5.5` when there are
/// none (`Colony.java:1438-1452`).
pub fn overall(citizens: &[f64]) -> f64 {
    if citizens.is_empty() {
        return EMPTY_COLONY_HAPPINESS;
    }
    let sum: f64 = citizens.iter().sum();
    sum / citizens.len() as f64
}

/// Java's `(int)` cast on a `double`: truncation towards zero, `NaN` → `0`,
/// and an infinity saturates.
///
/// The `NaN` case is the one that matters here: it is what a colony whose ten
/// factors are all exactly `1.0` produces, and Java answers `0` — a ceiling of
/// zero, i.e. no learning at all. Rust's `as` already does all three; the
/// function exists so the rule is stated once, with its source.
pub fn as_int(value: f64) -> i32 {
    value as i32
}

/// The intelligence ceiling (`CitizenSkillHandler.java:158-164`):
/// `(int)happiness * 9`.
///
/// Below it the tick adds ten intelligence XP; at or above it, nothing. At
/// happiness `10` the ceiling is `90`, which is where the top skill levels live.
pub fn intelligence_ceiling(happiness: f64) -> i32 {
    as_int(happiness).saturating_mul(INTELLIGENCE_PER_HAPPINESS)
}

/// The starting ceiling of a citizen created now (`CitizenData.java:526-538`):
/// `(int)colony.getOverallHappiness() * 2`, raised to [`STARTING_FLOOR`] while
/// the colony still has fewer citizens than `initialCitizenAmount`.
///
/// The floor is why a colony's first four citizens are not stuck at level 2: the
/// mean happiness of a fresh colony is low, and Java does not want that to
/// cripple the people who build it.
pub fn starting_ceiling(overall: f64, citizens: usize, initial_citizens: usize) -> i32 {
    let ceiling = as_int(overall).saturating_mul(STARTING_PER_HAPPINESS);
    if citizens < initial_citizens {
        ceiling.max(STARTING_FLOOR)
    } else {
        ceiling
    }
}

/// The breeding ceiling (`CitizenSkillHandler.java:91-92`): `(int)overall`, with
/// no multiplier. A child born in an unhappy colony starts worse.
pub fn breeding_ceiling(overall: f64) -> i32 {
    as_int(overall)
}

/// Whether an idle citizen complains instead of humming
/// (`SoundUtils.java:135`): `happiness < 5`.
///
/// Strictly below, and `NaN` is **not** a complaint — Java's `<` on `NaN` is
/// `false`, so a colony the aggregate could not compute is treated as content.
pub fn complains(happiness: f64) -> bool {
    happiness < COMPLAINT_LINE
}

/// The town hall's short label: `(int)happiness + "/10"`
/// (`WindowCitizenPage.java:143`).
pub fn label(happiness: f64) -> String {
    format!("{}/10", as_int(happiness))
}

/// The heart scale of the citizen window: `(int)happiness * 2` out of ten
/// half-hearts (`CitizenWindowUtils.java:304`).
///
/// **The cast binds to the `double`, not to the product.** Java writes
/// `int happiness = (int) citizen.getHappiness() * 2;` and a cast is a unary
/// operator, so `7.9` becomes `7` and the bar lights fourteen half-hearts —
/// exactly the seven the town hall prints as `7/10`
/// (`WindowCitizenPage.java:143`). Rounding the *product* instead
/// (`(int) (7.9 · 2) = 15`) would show half a heart no label ever mentions, and
/// rounding the *value* (`Math.round(7.9) · 2 = 16`) again differs. Neither form
/// appears in the tree.
pub fn hearts(happiness: f64) -> i32 {
    as_int(happiness).saturating_mul(2).clamp(0, 20)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_colony_with_nobody_in_it_is_five_and_a_half_by_decree() {
        assert_eq!(overall(&[]), EMPTY_COLONY_HAPPINESS);
        assert_eq!(overall(&[]), 5.5);
        assert_eq!(overall(&[10.0]), 10.0);
        assert_eq!(overall(&[0.0, 10.0]), 5.0);
        assert!((overall(&[1.0, 2.0, 4.0]) - 7.0 / 3.0).abs() < 1e-12);
        // The empty answer is not a rounded zero and not a stat: it is above the
        // complaint line, so a colony with no citizens never complains.
        assert!(!complains(overall(&[])));
    }

    #[test]
    fn the_int_cast_truncates_floors_nan_and_saturates_an_infinity() {
        assert_eq!(as_int(9.99), 9);
        assert_eq!(as_int(9.0), 9);
        assert_eq!(as_int(-1.5), -1, "truncation towards zero, not a floor");
        assert_eq!(as_int(f64::NAN), 0, "Java: (int) Double.NaN == 0");
        assert_eq!(as_int(f64::INFINITY), i32::MAX);
        assert_eq!(as_int(f64::NEG_INFINITY), i32::MIN);
    }

    #[test]
    fn happiness_buys_nine_intelligence_levels_a_point() {
        assert_eq!(intelligence_ceiling(0.0), 0);
        assert_eq!(intelligence_ceiling(1.0), 9);
        assert_eq!(intelligence_ceiling(5.0), 45);
        assert_eq!(intelligence_ceiling(10.0), 90);
        // Fractional happiness buys nothing: the cast happens first.
        assert_eq!(intelligence_ceiling(9.9), 81);
        // And the unknowable NaN is a ceiling of zero, in Java as here.
        assert_eq!(intelligence_ceiling(f64::NAN), 0);
    }

    #[test]
    fn a_new_citizen_starts_at_twice_the_happiness_with_a_floor_for_the_first_ones() {
        // A colony still filling up: the floor of 5 wins over a low mean.
        assert_eq!(starting_ceiling(1.0, 2, 4), STARTING_FLOOR);
        assert_eq!(starting_ceiling(0.0, 0, 4), STARTING_FLOOR);
        // Once it is at the initial count, the strict comparison stops the floor.
        assert_eq!(starting_ceiling(1.0, 4, 4), 2);
        assert_eq!(starting_ceiling(4.5, 4, 4), 8);
        // And a happy colony's newcomer is already above the floor.
        assert_eq!(starting_ceiling(10.0, 0, 4), 20);
    }

    #[test]
    fn a_child_is_capped_at_the_bare_happiness_with_no_multiplier() {
        assert_eq!(breeding_ceiling(6.0), 6);
        assert_eq!(breeding_ceiling(5.9), 5);
        assert_eq!(breeding_ceiling(0.0), 0);
        // Which is a third of what a newcomer gets, for the same colony.
        assert!(breeding_ceiling(9.0) < starting_ceiling(9.0, 10, 4));
    }

    #[test]
    fn the_complaint_line_is_five_and_nan_does_not_complain() {
        assert!(complains(4.999));
        assert!(!complains(5.0), "strictly below");
        assert!(!complains(10.0));
        assert!(!complains(f64::NAN), "Java's < on NaN is false");
        // The empty colony sits above the line on purpose.
        assert!(!complains(overall(&[])));
    }

    #[test]
    fn the_two_reads_the_gui_makes_are_the_same_cast() {
        assert_eq!(label(7.9), "7/10");
        assert_eq!(label(0.0), "0/10");
        assert_eq!(label(10.0), "10/10");
        // `(int)h * 2`, not `(int) (h * 2)`: a cast binds to the value, so the
        // bar lights fourteen half-hearts — the seven the label printed, doubled
        // (`CitizenWindowUtils.java:304`, `WindowCitizenPage.java:143`).
        assert_eq!(hearts(7.9), 14);
        assert_eq!(hearts(0.0), 0);
        assert_eq!(hearts(10.0), 20);
        assert_eq!(hearts(f64::NAN), 0);
    }

    #[test]
    fn the_effect_id_the_caller_is_told_to_read_exists_in_the_research_tree() {
        assert!(
            crate::research::effect::category(HAPPINESS_EFFECT).is_some(),
            "the port's research catalogue must know {HAPPINESS_EFFECT}"
        );
        let category = crate::research::effect::category(HAPPINESS_EFFECT).expect("checked above");
        assert!(category.max_level() >= 1);
    }

    #[test]
    fn the_four_effects_all_read_one_number_and_never_the_facts() {
        // Every function above takes an `f64`, so the aggregate is computed once
        // per citizen per call and the effects cannot disagree with it. The
        // colony mean is the only place two numbers meet.
        let colony = overall(&[4.0, 8.0]);
        assert_eq!(as_int(colony), 6);
        assert!(complains(4.0));
        assert!(!complains(colony));
        assert_eq!(intelligence_ceiling(colony), 54);
        assert_eq!(breeding_ceiling(colony), 6);
    }
}
