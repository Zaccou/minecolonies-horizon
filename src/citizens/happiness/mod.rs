//! Happiness: ten weighted ratios, five event modifiers, one nightly counter —
//! and the four things the number actually changes.
//!
//! Java reference: `core/entity/citizen/citizenhandlers/CitizenHappinessHandler.java`
//! (all 312 lines), `api/util/constant/HappinessConstants.java` (the fifteen ids
//! and the seven/fourteen-day thresholds), the three modifier classes under
//! `api/entity/citizen/happiness/`, and `core/colony/managers/CitizenManager.java:
//! 551-567` (the nightly pass over every citizen).
//!
//! | File | Question |
//! | --- | --- |
//! | `modifier` | what one modifier is: id, weight, factor, day counter |
//! | `factors` | the ten factor functions, and the facts they read |
//! | `saturation` | what a meal is worth and where the hunger lines are |
//! | `disease` | falling ill, spreading it, and the cure |
//! | `effects` | the four things happiness changes — and the two it does not |
//!
//! **The formula is one line, and its two oddities are the whole file.**
//!
//! ```java
//! for (modifier : happinessFactors) {
//!     final double factor = modifier.getFactor(citizenData);
//!     if (factor == 1.0) continue;                  // :146  ← skips the weight too
//!     total += factor * modifier.getWeight();       // :148
//!     totalWeight += modifier.getWeight();          // :149
//! }
//! return Math.min(10.0 * ((total / totalWeight) * (1 + HAPPINESS)), 10);  // :152-154
//! ```
//!
//! 1. **A factor of exactly `1.0` is dropped from the numerator *and* the
//!    denominator.** `1.0` means "this does not apply", not "neutral". Ten
//!    modifiers at `1.0` therefore leave `0.0 / 0.0`.
//! 2. **That division is `NaN`**, and Java keeps it: `Math.min(NaN, 10)` is `NaN`,
//!    the town hall prints it, and `(int) NaN` is `0` wherever it is cast. The
//!    port reproduces it ([`Happiness::total`]) rather than inventing a `0.0`,
//!    because `0` is a happiness a colony can really have and `NaN` is not.
//!
//! **The count, because two numbers look right.**
//!
//! | How many | What |
//! | ---: | --- |
//! | **10** | modifiers a citizen is born with (`CitizenHappinessHandler.java:53-80`) |
//! | **5** | added by events: [`Modifier::damage`], [`Modifier::death`], [`Modifier::raid_without_death`], [`Modifier::great_food`], [`Modifier::quest`] |
//! | **15** | ids in `VALID_HAPPINESS_MODIFIERS` (`HappinessConstants.java:63-77`) |
//! | **20.5** | the sum of the ten weights |
//!
//! **The clock is nightfall, not dawn.** `dayEnd` reads like a daily tick, but
//! `Colony.checkDayTime` calls `checkCitizensForHappiness` when the day **ends**
//! (`Colony.java:643-651`), and only when the colony has close subscribers — a
//! colony nobody is standing in does not advance its happiness counters at all.
//! [`Happiness::night_fall`] is that call; the hook that owns a clock will make
//! it once per dusk.
//!
//! **What is *not* here.** The NBT codec (`:160-198`), the chat interactions a
//! day end triggers (`:124-131`) and the cached value (`:43`, `:139`) all need
//! the game. The cache is dropped on purpose: ten factor evaluations are cheaper
//! than a dirty flag, and a recomputation cannot go stale.
//!
//! **Two premises of the brief are false, and `effects` carries the evidence.** A
//! citizen never refuses to work because of happiness and never leaves because of
//! it; the four things the number changes are two skill ceilings, a breeding
//! ceiling and a sound. Saturation and disease *do* stop work — through the state
//! machine (`priority`), not through happiness.

pub mod disease;
pub mod effects;
pub mod factors;
mod modifier;
pub mod saturation;

pub use disease::{
    Collision, Contact, Cure, Disease, Draw, Gate, Refusal as DiseaseRefusal, Row, Sickness,
};
pub use effects::{
    COMPLAINT_LINE, EMPTY_COLONY_HAPPINESS, as_int, breeding_ceiling, complains,
    intelligence_ceiling, label, overall, starting_ceiling,
};
pub use factors::{CitizenFacts, ColonyFacts, Facts, FoodHistory, Head, Housing};
pub use modifier::{Kind, Modifier, Source, Step};

/// One citizen's happiness: the ten modifiers it was born with, plus whatever
/// events have added since.
///
/// A `Vec` rather than a fixed array because Java's map is one too — five more
/// ids can arrive, and `add` replaces by id instead of growing
/// (`CitizenHappinessHandler.java:92-96`). The order is the registration order
/// and never changes, so a sum is reproducible; Java's `HashMap` order is not,
/// and only floating-point rounding could tell the two apart.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Happiness {
    modifiers: Vec<Modifier>,
}

impl Default for Happiness {
    fn default() -> Happiness {
        Happiness::new()
    }
}

impl Happiness {
    /// A citizen that has just been created: the ten of
    /// `CitizenHappinessHandler.java:53-80`, each counter at zero except the
    /// ramps that have not started.
    pub fn new() -> Happiness {
        Happiness {
            modifiers: Modifier::defaults().to_vec(),
        }
    }

    /// How many modifiers are in force, `10` to `15`.
    pub fn len(&self) -> usize {
        self.modifiers.len()
    }

    /// Never: a citizen always has the ten.
    pub fn is_empty(&self) -> bool {
        self.modifiers.is_empty()
    }

    /// The modifier for an id, `None` when the citizen does not carry it (the
    /// five event ones, before the event).
    pub fn modifier(&self, src: Source) -> Option<&Modifier> {
        self.modifiers
            .iter()
            .find(|modifier| modifier.source_of() == src)
    }

    /// How many days the modifier has been in force — what the town hall's
    /// "7 days without a bed" line reads (`InteractionValidatorInitializer.java:266-285`).
    pub fn days(&self, src: Source) -> Option<i32> {
        self.modifier(src).map(Modifier::days)
    }

    /// Add a modifier, replacing any with the same id
    /// (`CitizenHappinessHandler.java:92-96`).
    ///
    /// Replacing is how re-wounding works: a second wound resets `DAMAGE` to its
    /// one day instead of stacking two debuffs.
    pub fn add(&mut self, modifier: Modifier) {
        match self
            .modifiers
            .iter_mut()
            .find(|existing| existing.source_of() == modifier.source_of())
        {
            Some(slot) => *slot = modifier,
            None => self.modifiers.push(modifier),
        }
    }

    /// `resetModifier` (`:99-107`): put the counter back — zero days for a ramp,
    /// the full period for an expiry, nothing at all for a static one.
    ///
    /// `EntityAISleep.java:213` is the caller that matters: lying down resets
    /// `SLEPTTONIGHT`.
    pub fn reset(&mut self, src: Source) {
        if let Some(modifier) = self
            .modifiers
            .iter_mut()
            .find(|modifier| modifier.source_of() == src)
        {
            modifier.reset();
        }
    }

    /// One night for every modifier (`processDailyHappiness`, `:116-134`),
    /// without the chat interactions.
    ///
    /// Called at **dusk** by `Colony.checkDayTime` (`:643-651`). The predicates
    /// are evaluated before any counter moves, so an event modifier injected at
    /// noon gets its first night the same evening.
    pub fn night_fall(&mut self, facts: &Facts) {
        for modifier in &mut self.modifiers {
            modifier.day_end(facts);
        }
    }

    /// The citizen's happiness, `0` to `10`, with the `HAPPINESS` research
    /// applied and the `1.0` skip of `:146-150`.
    ///
    /// `research` is the `HAPPINESS` effect strength, `0.0` with no research —
    /// it multiplies the mean, so `+20 %` research is `1.2`.
    ///
    /// Returns [`f64::NAN`] when every factor sits at exactly `1.0`: Java divides
    /// zero by zero there and the port does not paper over it. See the module
    /// note, and [`effects::as_int`] for what a caller does with it.
    pub fn total(&self, facts: &Facts, research: f64) -> f64 {
        let mut total = 0.0;
        let mut weight = 0.0;
        for modifier in &self.modifiers {
            let factor = modifier.factor(facts);
            if factor == 1.0 {
                continue;
            }
            total += factor * modifier.weight();
            weight += modifier.weight();
        }
        let mean = total / weight;
        let scaled = mean * (1.0 + research);
        // `Math.min(10.0 * scaled, 10)` — and `Math.min` keeps a `NaN`, where
        // Rust's `f64::min` would answer `10`.
        let capped = 10.0 * scaled;
        if capped.is_nan() {
            f64::NAN
        } else {
            capped.min(10.0)
        }
    }

    /// Every modifier with its factor and weight, in registration order — the
    /// list the happiness window draws (`HappinessWindowCitizen`).
    ///
    /// The fourth field, `bool`, is whether the modifier was **counted**: a
    /// factor of exactly `1.0` is in this list and not in the average, which is
    /// the one thing a player looking at the screen cannot tell by eye.
    pub fn breakdown(&self, facts: &Facts) -> Vec<(Source, f64, f64, bool)> {
        self.modifiers
            .iter()
            .map(|modifier| {
                let factor = modifier.factor(facts);
                (
                    modifier.source_of(),
                    factor,
                    modifier.weight(),
                    factor != 1.0,
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::Job;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    /// The same check with a sentence attached, for a boundary whose *value* is
    /// not the interesting part but whose reason is.
    fn close_at(a: f64, b: f64, what: &str) {
        assert!((a - b).abs() < 1e-9, "{what}: {a} != {b}");
    }

    /// A grown citizen with no home, working a level-1 hut, in a colony of one
    /// that has nothing either.
    ///
    /// What it produces: homelessness `0.0`, security `0.75`, social `0.0`; every
    /// other factor is exactly `1.0` and therefore skipped.
    fn homeless() -> Facts {
        Facts::new(
            CitizenFacts::new().at(Job::Farmer, 1),
            ColonyFacts::counted(
                &[Head {
                    employed: true,
                    ..Head::default()
                }],
                0,
            ),
        )
    }

    /// A citizen every factor calls neutral: housed at three, hired at three, in
    /// a colony with two guards per three workers and nothing wrong.
    fn perfect() -> Facts {
        Facts::new(
            CitizenFacts::new().housed(3).at(Job::Farmer, 3),
            ColonyFacts::counted(
                &[
                    Head {
                        employed: true,
                        housed: true,
                        ..Head::default()
                    },
                    Head {
                        employed: true,
                        housed: true,
                        guard: true,
                        ..Head::default()
                    },
                    Head {
                        employed: true,
                        housed: true,
                        ..Head::default()
                    },
                ],
                0,
            ),
        )
    }

    #[test]
    fn a_new_citizen_carries_the_ten_and_none_of_the_five_events() {
        let happiness = Happiness::new();
        assert_eq!(happiness.len(), 10);
        assert!(!happiness.is_empty());
        assert_eq!(happiness, Happiness::default());
        for src in Source::AT_BIRTH {
            assert!(happiness.modifier(src).is_some(), "{}", src.name());
        }
        for src in [
            Source::Damage,
            Source::Death,
            Source::RaidWithoutDeath,
            Source::Quest,
            Source::GreatFood,
        ] {
            assert!(happiness.modifier(src).is_none(), "{}", src.name());
            assert_eq!(happiness.days(src), None);
        }
    }

    #[test]
    fn a_factor_of_exactly_one_leaves_both_the_sum_and_the_weight() {
        let happiness = Happiness::new();
        let total = happiness.total(&homeless(), 0.0);
        // 0.0·3 (homelessness) + 0.75·4 (security) + 0.0·2 (social) = 3.0 over 9.
        close(total, 10.0 * 3.0 / 9.0);
        // If the seven skipped modifiers had counted as neutral the same sum
        // would be spread over the full 20.5 and the answer would be this:
        assert!((total - 10.0 * 3.0 / 20.5).abs() > 1.0);
        // And the breakdown says which three were counted.
        let counted: Vec<Source> = happiness
            .breakdown(&homeless())
            .into_iter()
            .filter(|(_, _, _, counted)| *counted)
            .map(|(src, _, _, _)| src)
            .collect();
        assert_eq!(
            counted,
            vec![Source::Security, Source::Social, Source::Homelessness]
        );
    }

    #[test]
    fn a_colony_where_everything_is_neutral_divides_zero_by_zero() {
        // Every factor is exactly 1.0 in the `perfect` colony: housed at three
        // (1.0), hired at three (1.0), healthy, not idle, a guard elsewhere
        // balancing security, no mystical site, no food history, not a guard and
        // slept last night (0.5 × 2.0).
        let facts = perfect();
        let happiness = Happiness::new();
        for (src, factor, _, counted) in happiness.breakdown(&facts) {
            assert_eq!(factor, 1.0, "{} is {}", src.name(), factor);
            assert!(!counted, "{}", src.name());
        }
        assert!(happiness.total(&facts, 0.0).is_nan());
        // And the NaN travels: `(int) NaN` is 0, so the intelligence ceiling
        // collapses to zero. Java does exactly this.
        assert_eq!(
            effects::intelligence_ceiling(happiness.total(&facts, 0.0)),
            0
        );
    }

    #[test]
    fn research_multiplies_the_mean_and_the_cap_is_ten() {
        let facts = homeless();
        let happiness = Happiness::new();
        let plain = happiness.total(&facts, 0.0);
        close(happiness.total(&facts, 1.0), plain * 2.0);
        close(happiness.total(&facts, 0.2), plain * 1.2);
        close_at(happiness.total(&facts, -1.0), 0.0, "no research can zero it out");

        // A colony that beats every line is capped: level-5 home, level-4 hut,
        // maximum variety, a level-5 mystical site and a fortress of guards.
        let superb = Facts::new(
            CitizenFacts {
                food: FoodHistory::Full {
                    diversity: 10,
                    quality: 10,
                },
                ..CitizenFacts::new().housed(5).at(Job::Farmer, 4)
            },
            ColonyFacts {
                citizens: 1,
                homeless: 0,
                unemployed: 0,
                sick: 0,
                starving: 0,
                guards: 20,
                workers: 1,
                mystical_level: 5,
            },
        );
        let before_cap = happiness.total(&superb, 0.0);
        close_at(before_cap, 10.0, "21.15 is capped, not reported");
        // Without the cap the mean would be far above ten; the cap is the only
        // thing standing between a good colony and a nonsense number.
        let weighted: f64 = happiness
            .breakdown(&superb)
            .into_iter()
            .filter(|(_, factor, _, _)| *factor != 1.0)
            .map(|(_, factor, weight, _)| factor * weight)
            .sum();
        let weights: f64 = happiness
            .breakdown(&superb)
            .into_iter()
            .filter(|(_, factor, _, _)| *factor != 1.0)
            .map(|(_, _, weight, _)| weight)
            .sum();
        assert!(10.0 * weighted / weights > 10.0);
    }

    #[test]
    fn adding_the_same_id_twice_replaces_it_instead_of_stacking() {
        let facts = homeless();
        let mut happiness = Happiness::new();
        let plain = happiness.total(&facts, 0.0);
        happiness.add(Modifier::damage());
        assert_eq!(happiness.len(), 11);
        let wounded = happiness.total(&facts, 0.0);
        assert!(wounded < plain);
        // One night and the wound is forgotten.
        happiness.night_fall(&facts);
        assert_eq!(happiness.days(Source::Damage), Some(0));
        close(happiness.total(&facts, 0.0), plain);
        // Wounded again on the same evening: the counter is back to one day, and
        // there is still exactly one DAMAGE modifier.
        happiness.add(Modifier::damage());
        assert_eq!(happiness.len(), 11);
        assert_eq!(happiness.days(Source::Damage), Some(1));
        assert!(happiness.total(&facts, 0.0) < plain);
    }

    #[test]
    fn a_night_moves_every_counter_once_and_at_dusk() {
        let facts = homeless();
        let mut happiness = Happiness::new();
        assert_eq!(happiness.days(Source::Homelessness), Some(0));
        assert_eq!(happiness.days(Source::SleptTonight), Some(0));
        assert_eq!(happiness.days(Source::Security), Some(0), "static");
        happiness.night_fall(&facts);
        assert_eq!(happiness.days(Source::Homelessness), Some(1));
        assert_eq!(happiness.days(Source::SleptTonight), Some(1));
        assert_eq!(happiness.days(Source::Security), Some(0));
        // Seven nights of decay on a homeless citizen: still 0.0, but the
        // counter is what the "no bed" complaint reads.
        for _ in 0..6 {
            happiness.night_fall(&facts);
        }
        assert_eq!(happiness.days(Source::Homelessness), Some(7));
        // Resetting `SLEPTTONIGHT` is what lying down does.
        happiness.reset(Source::SleptTonight);
        assert_eq!(happiness.days(Source::SleptTonight), Some(0));
        // And the slept-tonight factor is neutral on the night it slept, then
        // drags: 1.0 → 1.0 → 0.8 → 0.5.
        let mut seen = Vec::new();
        for _ in 0..4 {
            seen.push(happiness.modifier(Source::SleptTonight).map(|m| m.factor(&facts)));
            happiness.night_fall(&facts);
        }
        assert_eq!(seen[0], Some(1.0));
        assert_eq!(seen[1], Some(1.0));
        assert_eq!(seen[2], Some(0.8));
        assert_eq!(seen[3], Some(0.5));
    }

    #[test]
    fn an_event_modifier_rides_above_neutral_and_expires_on_its_own() {
        let facts = homeless();
        let mut happiness = Happiness::new();
        let plain = happiness.total(&facts, 0.0);
        // A raid survived without a loss is the one reward modifier.
        happiness.add(Modifier::raid_without_death());
        let rewarded = happiness.total(&facts, 0.0);
        assert!(rewarded > plain, "{rewarded} must beat {plain}");
        for night in 1..=3 {
            happiness.night_fall(&facts);
            let days = happiness.days(Source::RaidWithoutDeath);
            assert_eq!(days, Some(3 - night));
        }
        // Three nights have passed, so `SLEPTTONIGHT` has decayed too: the
        // reference is a fresh citizen that lived the same three nights.
        let mut reference = Happiness::new();
        for _ in 0..3 {
            reference.night_fall(&facts);
        }
        close_at(
            happiness.total(&facts, 0.0),
            reference.total(&facts, 0.0),
            "expired, so skipped",
        );
        assert_eq!(happiness.days(Source::RaidWithoutDeath), Some(0));

        // A great meal is worth five nights.
        let mut fed = Happiness::new();
        fed.add(Modifier::great_food());
        assert_eq!(fed.days(Source::GreatFood), Some(5));
        assert!(fed.total(&facts, 0.0) > plain);
        // A death in the colony hurts the most and for three nights.
        let mut bereaved = Happiness::new();
        bereaved.add(Modifier::death());
        assert!(bereaved.total(&facts, 0.0) < plain);
        assert_eq!(bereaved.modifier(Source::Death).map(Modifier::weight), Some(3.0));
    }

    #[test]
    fn a_quarter_of_the_effects_are_reachable_from_one_total() {
        // The seam the hooks will use: a facts value in, one number out, and
        // every effect of `effects` fed from it.
        let happiness = Happiness::new();
        let total = happiness.total(&homeless(), 0.0);
        assert!(total > 0.0 && total < 10.0);
        assert!(effects::complains(total), "under five, it complains");
        assert_eq!(effects::intelligence_ceiling(total), 27, "(int)3.33 · 9");
        assert_eq!(effects::label(total), "3/10");
        assert_eq!(effects::hearts(total), 6);
        assert_eq!(effects::overall(&[total, total]), total);
        assert_eq!(effects::starting_ceiling(total, 10, 4), 6);
    }
}
