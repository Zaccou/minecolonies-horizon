//! Saturation: what a citizen eats, what that is worth, and where the lines are.
//!
//! Java reference: `api/colony/ICitizenData.java:105` and
//! `core/colony/CitizenData.java:93` (`MIN_SATURATION`), `:1157-1170`
//! (`increaseSaturation` and `decreaseSaturation`), `api/util/FoodUtils.java:92-116`
//! (what one meal is worth), `api/util/ItemStackUtils.java:929-970` (the meal
//! itself), `core/entity/ai/minimal/EntityAIEatTask.java:220-242` (`justAte`),
//! and the per-job factors `IJob.java:323-326` with its three overrides.
//!
//! **The fall is not here, and that is deliberate.** `needs.rs` already owns the
//! decay — `needs::Needs::advance` runs `decreaseIdleSaturation`
//! (`EntityCitizen.java:1970-1996`) with `Advance`, and `needs::consumption_factor`
//! is its housing table. This file owns the **rise** and the **reading**, so the
//! two halves stay in one place each instead of meeting in the middle.
//!
//! | Question | Answer | Where |
//! | --- | --- | --- |
//! | what one meal adds | `nutrition × (2 if MineColonies food) × (1 + research)` | [`food_value`] |
//! | what the meal does to the gauge | `min(60, sat + |value|)` | [`feed`] |
//! | what ends a meal | `justAte`, cleared by the next decay | [`finish_meal`] |
//! | what a job costs | `1.0`, or `1.2` for three jobs | [`job_factor`] |
//! | when is a citizen hungry | `≤ 10`, and `≤ 2.5` sends it to a restaurant | `needs::Needs` |
//! | when is it *starving* | `≤ 1` — a **different**, much lower line | [`starving`] |
//!
//! **Two hunger lines, and they are not the same question.** `needs.rs` reads
//! `≤ 10` to decide whether a citizen walks to a restaurant; the social happiness
//! factor reads `≤ 1` to decide whether its neighbours are unhappy about it
//! (`CitizenHappinessHandler.java:239-242`). A colony can therefore be full of
//! "hungry" citizens at 8.0 with the social factor saying nothing at all. The
//! port keeps both constants and says which is which.
//!
//! **Nothing here draws a random number or reads a clock.** A meal is a value the
//! caller provides, like the tick in `needs.rs`.

use crate::jobs::Job;

use crate::citizens::needs::{
    AVERAGE_SATURATION, FULL_SATURATION, LOW_SATURATION, MAX_SATURATION, Needs, RESTAURANT_LIMIT,
};

/// `CitizenData.MIN_SATURATION` (`core/colony/CitizenData.java:93`): the gauge's
/// floor, and what `decreaseSaturation` clamps to (`:1168`).
pub const MIN_SATURATION: f64 = 0.0;

/// The line the **social** happiness factor calls hungry:
/// `citizen.getSaturation() <= 1` (`CitizenHappinessHandler.java:239`).
///
/// Not `LOW_SATURATION` (`6`) and not `AVERAGE_SATURATION` (`10`): this is a
/// citizen who has eaten nothing for a long while, not one that is peckish.
pub const STARVING: f64 = 1.0;

/// What a MineColonies dish multiplies its nutrition by
/// (`FoodUtils.java:101`).
pub const FOOD_SATURATION_BONUS: f64 = 2.0;

/// The research effect that scales what a meal is worth
/// (`ResearchConstants.SATURATION`, `effects/saturationmultiplier`), read as
/// `crate::research::Effects::strength(SATURATION_EFFECT)`.
pub const SATURATION_EFFECT: &str = "saturationmultiplier";

/// The nutrition of one item, as `getFoodValue` reads it — a `FoodProperties`
/// the game owns, passed in as a number.
///
/// `getFoodValue(foodStack, itemFood, researchBonus)`
/// (`FoodUtils.java:92-103`):
///
/// ```java
/// double saturationBonus = foodStack.getItem() instanceof IMinecoloniesFoodItem ? 2.0 : 1.0;
/// return itemFood.getNutrition() * saturationBonus * (1.0 + researchBonus);
/// ```
///
/// The `2.0` is why the comment above the method says one top-tier dish fills the
/// whole bar: saturation tops out at `60`, and no vanilla food has a nutrition of
/// `30`.
pub fn food_value(nutrition: f64, minecolonies: bool, research: f64) -> f64 {
    let bonus = if minecolonies { FOOD_SATURATION_BONUS } else { 1.0 };
    nutrition * bonus * (1.0 + research)
}

/// `CitizenData.increaseSaturation` (`:1157-1161`):
/// `this.saturation = Math.min(MAX_SATURATION, this.saturation + Math.abs(extraSaturation))`.
///
/// Two things the `Math.abs` and the `min` do: a negative value **feeds** (Java
/// takes the magnitude, so a caller cannot starve a citizen through this door),
/// and the gauge stops at the maximum instead of overflowing.
///
/// **One divergence, because Java's answer is a broken save.** A `NaN` meal
/// would set the gauge to `NaN` in Java — `Math.min` keeps it — and every
/// comparison against it then reads false forever. Rust's `f64::min` answers the
/// other operand, so a `NaN` meal here tops the citizen up. Neither is a rule
/// Java meant; this one cannot corrupt a colony.
pub fn feed(needs: &mut Needs, value: f64) {
    needs.saturation = MAX_SATURATION.min(needs.saturation + value.abs());
}

/// What the eating AI does when the meal is over: `citizenData.setJustAte(true)`
/// (`EntityAIEatTask.java:241`, and `:381`, `:505` for the two early exits).
///
/// `justAte` is what stops a citizen from walking back to the restaurant in the
/// same day (`CitizenAI.java:270`); the next decay clears it
/// (`CitizenData.java:1169`), which `needs.rs` already does.
pub fn finish_meal(needs: &mut Needs) {
    needs.just_ate = true;
}

/// Whether the neighbour-counting threshold holds: `saturation <= 1`
/// (`CitizenHappinessHandler.java:239`).
pub fn starving(needs: &Needs) -> bool {
    needs.saturation <= STARVING
}

/// `IJob.getSaturationFactor` (`IJob.java:323-326`) and the three jobs that
/// override it — all three to the same `1.2`.
///
/// | Job | Factor | Java |
/// | --- | ---: | --- |
/// | courier | 1.2 | `JobDeliveryman.java:603-606` |
/// | farmer | 1.2 | `JobFarmer.java:42-45` |
/// | miner | 1.2 | `JobMiner.java:65-68` |
/// | every other job, and no job at all | 1.0 | `IJob.java:326` |
///
/// The result is what `needs::Advance::job_factor` expects, so a caller wires the
/// two with `Advance { job_factor: saturation::job_factor(job), .. }` — this
/// module never repeats the decay itself.
pub fn job_factor(job: Option<Job>) -> f64 {
    match job {
        Some(Job::Deliveryman | Job::Farmer | Job::Miner) => 1.2,
        _ => 1.0,
    }
}

/// Where a citizen sits on the gauge, read as a name.
///
/// Every boundary is a constant another file already owns: [`STARVING`] here,
/// `RESTAURANT_LIMIT`, `LOW_SATURATION`, `AVERAGE_SATURATION` and
/// `FULL_SATURATION` from `needs.rs`. The ladder is **a reading only** — the
/// decision to eat is `needs::Needs::should_eat` and stays there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
// exhaustive: the ladder is closed and ordered from worst to best
pub enum Stage {
    /// `≤ 1`: the social factor counts it (`CitizenHappinessHandler.java:239`).
    Starving,
    /// `≤ 2.5`: `shouldEat` sends it to the restaurant even above the low line
    /// (`EntityAIEatTask.java:74`, `CitizenAI.java:296`).
    Restaurant,
    /// `≤ 6`: `LOW_SATURATION` (`CitizenConstants.java:55`).
    Low,
    /// `≤ 10`: `AVERAGE_SATURATION`, the ordinary hunger line (`:50`).
    Average,
    /// Above `10` and below the top.
    Satisfied,
    /// At the maximum: `FULL_SATURATION`, which is `MAX_SATURATION` (`:53`).
    Full,
}

impl Stage {
    /// The stage's lower edge in saturation points, so two stages can be
    /// compared as numbers as well as names.
    pub fn floor(self) -> f64 {
        match self {
            Stage::Starving => MIN_SATURATION,
            Stage::Restaurant => STARVING,
            Stage::Low => RESTAURANT_LIMIT,
            Stage::Average => LOW_SATURATION,
            Stage::Satisfied => AVERAGE_SATURATION,
            Stage::Full => FULL_SATURATION,
        }
    }
}

/// Which [`Stage`] this gauge is in. Ordered from the worst line up, so the first
/// match is the tightest one.
pub fn stage(needs: &Needs) -> Stage {
    if needs.saturation <= STARVING {
        Stage::Starving
    } else if needs.saturation <= RESTAURANT_LIMIT {
        Stage::Restaurant
    } else if needs.saturation <= LOW_SATURATION {
        Stage::Low
    } else if needs.saturation <= AVERAGE_SATURATION {
        Stage::Average
    } else if needs.saturation >= FULL_SATURATION {
        Stage::Full
    } else {
        Stage::Satisfied
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::citizens::needs::{Advance, Appetite};

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    /// The same check with a sentence attached, for a boundary whose *value* is
    /// not the interesting part but whose reason is.
    fn close_at(a: f64, b: f64, what: &str) {
        assert!((a - b).abs() < 1e-9, "{what}: {a} != {b}");
    }

    #[test]
    fn one_meal_is_its_nutrition_doubled_for_minecolonies_food_and_scaled_by_research() {
        close_at(food_value(5.0, false, 0.0), 5.0, "FoodUtils.java:102");
        close_at(food_value(5.0, true, 0.0), 10.0, "the 2.0 at :101");
        close(food_value(5.0, false, 0.5), 7.5);
        close(food_value(5.0, true, 0.5), 15.0);
        // A top-tier dish at nutrition 30 fills the entire bar from empty.
        assert!(food_value(30.0, true, 0.0) >= MAX_SATURATION);
        close(food_value(0.0, true, 1.0), 0.0);
    }

    #[test]
    fn the_saturation_effect_id_exists_in_the_research_tree() {
        assert!(
            crate::research::effect::category(SATURATION_EFFECT).is_some(),
            "the port's research catalogue must know {SATURATION_EFFECT}"
        );
    }

    #[test]
    fn feeding_adds_the_magnitude_and_stops_at_the_maximum() {
        let mut needs = Needs::new();
        needs.saturation = 10.0;
        feed(&mut needs, 5.0);
        close(needs.saturation, 15.0);
        // Java's `Math.abs`: a negative meal still feeds.
        feed(&mut needs, -5.0);
        close(needs.saturation, 20.0);
        // And the gauge does not overflow its maximum.
        feed(&mut needs, 1_000.0);
        close(needs.saturation, MAX_SATURATION);
        // Feeding does not clear `justAte`; only the decay does
        // (`CitizenData.java:1169`).
        let mut fed = Needs::new();
        fed.just_ate = true;
        feed(&mut fed, 1.0);
        assert!(fed.just_ate);
    }

    #[test]
    fn a_nan_meal_tops_the_citizen_up_instead_of_poisoning_the_gauge() {
        let mut needs = Needs::new();
        needs.saturation = 10.0;
        feed(&mut needs, f64::NAN);
        // Java would store `NaN` here and every comparison would read false for
        // the rest of the save; the port answers the maximum instead.
        assert!(needs.saturation.is_finite());
        close(needs.saturation, MAX_SATURATION);
    }

    #[test]
    fn a_finished_meal_blocks_the_next_one_until_the_decay_clears_it() {
        let mut needs = Needs::new();
        needs.saturation = 2.0;
        let appetite = Appetite {
            has_job: false,
            interruptible: true,
            was_eating: false,
            cook_skips: false,
        };
        assert!(needs.should_eat(&appetite));
        feed(&mut needs, 8.0);
        finish_meal(&mut needs);
        assert!(!needs.should_eat(&appetite), "CitizenAI.java:270");
        // One decay of the needs model clears it and the citizen is hungry again.
        needs.saturation = 2.0;
        needs.advance(1200, &Advance::at_level(3));
        assert!(!needs.just_ate);
        assert!(needs.should_eat(&appetite));
    }

    #[test]
    fn the_three_exhausting_jobs_burn_a_fifth_more_and_the_rest_nothing_more() {
        for job in [Job::Deliveryman, Job::Farmer, Job::Miner] {
            close(job_factor(Some(job)), 1.2);
        }
        for job in [Job::Builder, Job::Knight, Job::Pupil, Job::Cook, Job::Healer] {
            close(job_factor(Some(job)), 1.0);
        }
        close_at(job_factor(None), 1.0, "IJob.java:326, the default");
        // And it plugs straight into the decay that lives in `needs.rs`.
        let mut needs = Needs::new();
        needs.advance(
            1200,
            &Advance {
                job_factor: job_factor(Some(Job::Farmer)),
                ..Advance::at_level(3)
            },
        );
        close(needs.saturation, 60.0 - 1.2);
    }

    #[test]
    fn starving_is_one_point_which_is_well_under_the_hunger_line() {
        let mut needs = Needs::new();
        needs.saturation = STARVING;
        assert!(starving(&needs));
        needs.saturation = STARVING + 0.001;
        assert!(!starving(&needs));
        // The distinction that matters: a citizen at 5.0 is hungry enough for
        // `should_eat` and not starving enough for the social factor.
        needs.saturation = LOW_SATURATION;
        assert!(needs.hungry(), "needs.rs reads the ≤ 10 line");
        assert!(!starving(&needs), "CitizenHappinessHandler.java:239 reads ≤ 1");
        assert!(STARVING < LOW_SATURATION);
        assert!(LOW_SATURATION < AVERAGE_SATURATION);
        assert!(RESTAURANT_LIMIT < LOW_SATURATION);
    }

    #[test]
    fn the_stage_ladder_walks_up_through_every_boundary() {
        let mut needs = Needs::new();
        for (saturation, expected) in [
            (0.0, Stage::Starving),
            (1.0, Stage::Starving),
            (1.5, Stage::Restaurant),
            (2.5, Stage::Restaurant),
            (4.0, Stage::Low),
            (6.0, Stage::Low),
            (8.0, Stage::Average),
            (10.0, Stage::Average),
            (30.0, Stage::Satisfied),
            (59.999, Stage::Satisfied),
            (60.0, Stage::Full),
        ] {
            needs.saturation = saturation;
            assert_eq!(stage(&needs), expected, "at {saturation}");
        }
        // A fresh citizen is full.
        assert_eq!(stage(&Needs::new()), Stage::Full);
    }

    #[test]
    fn the_ladder_is_ordered_from_worst_to_best_and_its_floors_agree() {
        let ladder = [
            Stage::Starving,
            Stage::Restaurant,
            Stage::Low,
            Stage::Average,
            Stage::Satisfied,
            Stage::Full,
        ];
        for pair in ladder.windows(2) {
            assert!(pair[0] < pair[1], "{:?} must rank under {:?}", pair[0], pair[1]);
            assert!(pair[0].floor() < pair[1].floor());
        }
        close(Stage::Starving.floor(), MIN_SATURATION);
        close(Stage::Restaurant.floor(), STARVING);
        close(Stage::Low.floor(), RESTAURANT_LIMIT);
        close(Stage::Average.floor(), LOW_SATURATION);
        close(Stage::Satisfied.floor(), AVERAGE_SATURATION);
        close(Stage::Full.floor(), FULL_SATURATION);
        // The gauge floor is what `decreaseSaturation` clamps to, and `needs.rs`
        // reaches the same number with `.max(0.0)`.
        close(MIN_SATURATION, 0.0);
    }
}
