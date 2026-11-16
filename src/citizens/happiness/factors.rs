//! The ten functions that produce a happiness factor, and the facts they read.
//!
//! Java reference: `apiimp/initializer/ModHappinessFactorTypeInitializer.java:39-51`
//! (the ten `HappinessFunctionEntry` lambdas) and
//! `core/entity/citizen/citizenhandlers/CitizenHappinessHandler.java:214-311`
//! (the three static helpers two of them call).
//!
//! **A factor is a ratio, not a score.** `1.0` is neutral, below `1.0` drags the
//! citizen down, above `1.0` lifts it — `IHappinessModifier.java:19-24` says so
//! in one line. Nothing here clamps: the cap lives on the aggregate
//! ([`super::Happiness::total`]), and Java applies it there too
//! (`CitizenHappinessHandler.java:154`).
//!
//! | Function | Java | What it reads |
//! | --- | --- | --- |
//! | [`housing`] | `:44` | the home's **level** ÷ 3 |
//! | [`unemployment`] | `:45` | child, no hut, hut level over 3 |
//! | [`health`] | `:46-47` | loaded and ill |
//! | [`idle_at_job`] | `:48` | `JobStatus.STUCK` |
//! | [`slept_tonight`] | `:50` | guard or not — the ramp is the modifier's |
//! | [`food`] | `:51` | the last ten meals, the home's level |
//! | [`security`] | `:40` | guards ÷ workers, both counted from 1 |
//! | [`social`] | `:41` | how many neighbours are out of work, bed, health, food |
//! | [`mystical_site`] | `:42` | the best mystical site's level ÷ 2 |
//! | [`school`] | `:39` | a child with the pupil job |
//!
//! **The colony counts arrive pre-counted.** Java loops
//! `colony.getCitizenManager().getCitizens()` inside `getSocialModifier` and
//! `getGuardFactor`; a wasm mod has no colony handle here and the citizen list
//! belongs to `crate::colony`, so [`ColonyFacts`] carries the four spoilt counts
//! and the two head counts, and [`ColonyFacts::counted`] is the loop Java runs —
//! one place, so a caller with a list of citizens never re-derives it.
//!
//! **A factor of exactly `1.0` is not neutral, it is absent.** The aggregate
//! skips it *and its weight* (`CitizenHappinessHandler.java:146-147`), so
//! `1.0` removes the modifier from the denominator instead of adding a neutral
//! term. That is the single most surprising line in the whole calculation and
//! [`super::Happiness::total`] is where it is spelled out.

use crate::jobs::Job;

// `is_guard` is `crate::citizens::employment`'s rule, re-exported by
// `crate::citizens` (`src/citizens/mod.rs:62`). It is *not* in `happiness`, so
// the path is absolute: `mod/employment.rs:54-58` defines it, citing
// `CitizenAI.java:136`.
use crate::citizens::is_guard;

/// `housing`: `homeLevel / 3.0` (`ModHappinessFactorTypeInitializer.java:44`).
///
/// The divisor is the **number of levels a house is allowed to have**, so a
/// level-5 house answers `1.667` — above neutral, which is how housing lifts a
/// citizen rather than merely failing to drag it.
pub const HOME_LEVEL_DIVISOR: f64 = 3.0;

/// `unemployment`: the hut level from which a job is a *bonus* rather than
/// neutral (`:45`): `getBuildingLevel() > 3 ? 2.0 : 1.0`.
pub const GOOD_JOB_LEVEL: i32 = 3;

/// `unemployment`: what no hut is worth (`:45`).
pub const NO_JOB: f64 = 0.5;

/// `health`: what being ill is worth (`:47`).
pub const ILL: f64 = 0.5;

/// `idle_at_job`: what standing idle at a hut is worth (`:48`).
pub const IDLE: f64 = 0.5;

/// `slept_tonight`: what a citizen that is not a guard is worth before the
/// ramp (`:50`).
pub const NO_SLEEP: f64 = 0.5;

/// `security`: the two sides are counted from one, not from zero
/// (`CitizenHappinessHandler.java:256-257`), so a colony of one has a
/// denominator of two.
pub const SECURITY_START: f64 = 1.0;

/// `security`: the ceiling, `Math.min(…, 2)` (`:269`).
pub const SECURITY_CAP: f64 = 2.0;

/// `security`: the ratio's divisor, `workers * 2 / 3` (`:269`).
pub const SECURITY_PER_WORKER: f64 = 2.0 / 3.0;

/// `mystical_site`: a second divisor, `level / 2.0` (`:310`).
pub const MYSTICAL_DIVISOR: f64 = 2.0;

/// `food`: the cap on each of the two halves, `Math.min(5.0, …)`
/// (`CitizenHappinessHandler.java:294-295`).
pub const FOOD_HALF_CAP: f64 = 5.0;

/// `food`: how many meals make a history (`CitizenFoodHandler.java:36`,
/// read by `hasFullFoodHistory` at `:135-138`).
pub const FOOD_HISTORY: usize = 10;

/// The home a citizen lives in, as the two factors that read it need it.
///
/// Java asks `getHomeBuilding().getBuildingLevel()` for the housing factor
/// (`:44`) and `getBuildingLevelEquivalent()` for the food one (`:285`). The
/// second is a one-line default returning the first (`ICommonBuilding.java:42-45`)
/// and only the gatehouse overrides it — a residence does not — so the port keeps
/// one number and cites both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Housing {
    /// A **built** level, `1..=5`; `0` is a hut placed and not finished.
    pub level: i32,
}

/// The last ten meals, in the shape the food factor and the disease modifier
/// both read (`ICitizenFoodHandler.CitizenFoodStats`, `:17-20`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum FoodHistory {
    /// Fewer than [`FOOD_HISTORY`] meals eaten: Java answers `1.0` and refuses to
    /// judge (`CitizenHappinessHandler.java:287-290`).
    #[default]
    Short,
    /// A full history: `diversity` distinct items, `quality` of them
    /// MineColonies food (`CitizenFoodHandler.java:112-129`).
    Full {
        /// Distinct items among the last ten, `max(1, unique)` in Java (`:127`).
        diversity: i32,
        /// How many of the ten were `IMinecoloniesFoodItem` (`:120-123`).
        quality: i32,
    },
}

/// One citizen, as the factors read it. Everything here is a field Java reads
/// off an `ICitizenData`, copied so the module stays pure.
#[derive(Clone, Copy, Debug, PartialEq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct CitizenFacts {
    /// `isChild()` (`:39`, `:45`).
    pub child: bool,
    /// `getJob()` — one job and no job, never a family (`:45`, `:48`, `:50`).
    pub job: Option<Job>,
    /// `getHomeBuilding()` (`:44`, `:285`).
    pub housing: Option<Housing>,
    /// `getWorkBuilding()` (`:45`).
    pub work_level: Option<i32>,
    /// `getCitizenDiseaseHandler().isSick()` (`:47`).
    pub sick: bool,
    /// `getEntity().isPresent()` (`:47`): a citizen with no entity in the world
    /// is **never** ill as far as happiness is concerned — Java says `1.0`, not
    /// `0.5`, for it.
    pub loaded: bool,
    /// `isIdleAtJob()` — `jobStatus == JobStatus.STUCK` (`:48`,
    /// `CitizenData.java:1730-1733`).
    pub idle_at_job: bool,
    /// The last ten meals (`:51`).
    pub food: FoodHistory,
}

impl Default for CitizenFacts {
    fn default() -> CitizenFacts {
        CitizenFacts {
            child: false,
            job: None,
            housing: None,
            work_level: None,
            sick: false,
            loaded: true,
            idle_at_job: false,
            food: FoodHistory::Short,
        }
    }
}

impl CitizenFacts {
    /// A grown, loaded, healthy citizen with no home and no hut — the state a
    /// newcomer is in before anything is built.
    pub fn new() -> CitizenFacts {
        CitizenFacts::default()
    }

    /// Housed at a built level.
    pub fn housed(mut self, level: i32) -> CitizenFacts {
        self.housing = Some(Housing { level });
        self
    }

    /// Hired, working in a hut of that level.
    pub fn at(mut self, job: Job, work_level: i32) -> CitizenFacts {
        self.job = Some(job);
        self.work_level = Some(work_level);
        self
    }

    /// Whether this citizen takes the guard branch — `instanceof AbstractJobGuard`,
    /// which is what `slept_tonight` asks (`:50`).
    pub fn guard(&self) -> bool {
        self.job.map(is_guard).unwrap_or(false)
    }
}

/// One citizen as a colony-wide count sees it: the four ways a citizen spoils
/// its neighbours' happiness, plus the guard flag. `starving` rather than the
/// saturation itself, so the threshold stays in one place
/// ([`super::saturation::starving`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Head {
    /// `citizen.isChild()` (`:224`) — a child out of work does not count.
    pub child: bool,
    /// `citizen.getJob() != null`.
    pub employed: bool,
    /// `citizen.getHomeBuilding() != null`.
    pub housed: bool,
    /// `isSick()` **and** the entity is present (`:234`).
    pub sick: bool,
    /// `citizen.getSaturation() <= 1` (`:239`).
    pub starving: bool,
    /// `citizen.getJob() instanceof AbstractJobGuard` (`:260`).
    pub guard: bool,
}

/// The colony's side of the calculation: four spoilt counts, two head counts,
/// and the best mystical site. Java derives all of them by looping the citizen
/// list; [`ColonyFacts::counted`] is that loop.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct ColonyFacts {
    /// `getCitizens().size()` (`:216`).
    pub citizens: usize,
    /// Not a child and no job (`:224-227`).
    pub unemployed: usize,
    /// No home (`:229-232`).
    pub homeless: usize,
    /// Ill, with an entity in the world (`:234-237`).
    pub sick: usize,
    /// Saturation at or below one (`:239-242`) — a **different** threshold from
    /// the hunger line, and the one the social factor uses.
    pub starving: usize,
    /// `AbstractJobGuard` (`:260-263`).
    pub guards: usize,
    /// Everybody else (`:264-267`), children included.
    pub workers: usize,
    /// `getMysticalSiteMaxBuildingLevel()` (`:310`), `0` with none.
    pub mystical_level: i32,
}

impl ColonyFacts {
    /// The loop `getSocialModifier` and `getGuardFactor` each run over the
    /// citizen list (`:216-246`, `:254-270`), done once.
    pub fn counted(citizens: &[Head], mystical_level: i32) -> ColonyFacts {
        let mut facts = ColonyFacts {
            citizens: citizens.len(),
            mystical_level,
            ..ColonyFacts::default()
        };
        for head in citizens {
            if !head.child && !head.employed {
                facts.unemployed += 1;
            }
            if !head.housed {
                facts.homeless += 1;
            }
            if head.sick {
                facts.sick += 1;
            }
            if head.starving {
                facts.starving += 1;
            }
            if head.guard {
                facts.guards += 1;
            } else {
                facts.workers += 1;
            }
        }
        facts
    }
}

/// What both halves of one calculation need.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Facts {
    /// The citizen the factor is *for*; `null` in Java is the client GUI
    /// (`AbstractHappinessModifier.java:93`), which reads a cached value the
    /// port does not keep.
    pub citizen: CitizenFacts,
    /// The colony it lives in.
    pub colony: ColonyFacts,
}

impl Facts {
    /// A citizen in a colony, the two halves together.
    pub fn new(citizen: CitizenFacts, colony: ColonyFacts) -> Facts {
        Facts { citizen, colony }
    }
}

/// `housing`: `home == null ? 0.0 : home.getBuildingLevel() / 3.0`
/// (`ModHappinessFactorTypeInitializer.java:44`).
///
/// A homeless citizen gets **`0.0`**, not `1.0`: this factor never says
/// "neutral", it says "nothing".
pub fn housing(facts: &CitizenFacts) -> f64 {
    match facts.housing {
        None => 0.0,
        Some(home) => home.level as f64 / HOME_LEVEL_DIVISOR,
    }
}

/// `unemployment` (`:45`), in Java's order:
///
/// ```java
/// data.isChild() ? 1.0
///   : (data.getWorkBuilding() == null ? 0.5
///      : data.getWorkBuilding().getBuildingLevel() > 3 ? 2.0 : 1.0)
/// ```
///
/// A child is `1.0` and therefore **excluded** from the average; a grown citizen
/// in a level-3 hut is too, which is why only a level-4 hut and above is worth
/// anything.
pub fn unemployment(facts: &CitizenFacts) -> f64 {
    if facts.child {
        return 1.0;
    }
    match facts.work_level {
        None => NO_JOB,
        Some(level) if level > GOOD_JOB_LEVEL => 2.0,
        Some(_) => 1.0,
    }
}

/// `health` (`:46-47`): `entityPresent ? (sick ? 0.5 : 1.0) : 1.0`.
///
/// The three-way answer matters: an unloaded citizen is not treated as ill even
/// when the saved data says it is.
pub fn health(facts: &CitizenFacts) -> f64 {
    if !facts.loaded {
        return 1.0;
    }
    if facts.sick { ILL } else { 1.0 }
}

/// `idle_at_job` (`:48`): `isIdleAtJob() ? 0.5 : 1.0`, and "idle at job" is
/// strictly `jobStatus == STUCK` (`CitizenData.java:1730-1733`) — not "has
/// nothing to do".
pub fn idle_at_job(facts: &CitizenFacts) -> f64 {
    if facts.idle_at_job { IDLE } else { 1.0 }
}

/// `slept_tonight` (`:50`): `job instanceof AbstractJobGuard ? 1 : 0.5`.
///
/// A guard is **`1.0` on purpose**: guards work the night, so the modifier is
/// excluded from their average rather than penalising them. The ramp that makes
/// the `0.5` worse the longer a citizen has not slept belongs to the modifier
/// (`super::modifier::Modifier::slept_tonight`).
pub fn slept_tonight(facts: &CitizenFacts) -> f64 {
    if facts.guard() { 1.0 } else { NO_SLEEP }
}

/// `food` (`CitizenHappinessHandler.java:283-298`):
///
/// ```java
/// if (homeBuildingLevel == 0 || !foodHandler.hasFullFoodHistory()) return 1.0;
/// diversityFactor = min(5.0, diversity / homeBuildingLevel);
/// qualityFactor   = min(5.0, quality / max(1.0, homeBuildingLevel - 2.0));
/// return (diversityFactor + qualityFactor) / 2.0;
/// ```
///
/// So food only has an opinion about a citizen with a home **and** ten meals
/// behind it, and the two halves are judged against different divisors: variety
/// is expected to scale with the house, the good stuff only with the last two
/// levels of it.
pub fn food(facts: &CitizenFacts) -> f64 {
    let Some(home) = facts.housing else {
        return 1.0;
    };
    if home.level <= 0 {
        return 1.0;
    }
    let FoodHistory::Full {
        diversity,
        quality,
    } = facts.food
    else {
        return 1.0;
    };
    let level = home.level as f64;
    let diversity_factor = (diversity as f64 / level).min(FOOD_HALF_CAP);
    let quality_factor = (quality as f64 / (level - 2.0).max(1.0)).min(FOOD_HALF_CAP);
    (diversity_factor + quality_factor) / 2.0
}

/// `security` (`CitizenHappinessHandler.java:254-270`), a static helper:
///
/// ```java
/// double guards = 1; double workers = 1;
/// for (citizen) if (job instanceof AbstractJobGuard) guards++; else workers++;
/// return Math.min(guards / (workers * 2 / 3), 2);
/// ```
///
/// Both counters start at **one**, so a colony of one worker and no guard
/// answers `1 / (2·2/3) = 0.75` — the lone citizen already feels undefended —
/// and neutral is **one guard for two workers**, not two for three: one guard
/// and two workers enter the ratio as `2 / (3·2/3) = 1.0`, while two guards for
/// three workers is already ahead at `3 / (4·2/3) = 1.125`. The two free ones
/// (`:256-257`) are why the head counts and the ratio's counters differ by one
/// on each side.
pub fn security(colony: &ColonyFacts) -> f64 {
    let guards = SECURITY_START + colony.guards as f64;
    let workers = SECURITY_START + colony.workers as f64;
    (guards / (workers * SECURITY_PER_WORKER)).min(SECURITY_CAP)
}

/// `social` (`:214-246`): `(total - (unemployed + homeless + sick + hungry)) / total`.
///
/// Four counts are **added**, not de-duplicated: a citizen with no home, no job
/// and no health subtracts three times from the same colony. With no citizens at
/// all the ratio is `0/0`, and Java's answer is `NaN` — [`total`] keeps that
/// answer instead of pretending otherwise.
///
/// [`total`]: super::Happiness::total
pub fn social(colony: &ColonyFacts) -> f64 {
    let total = colony.citizens as f64;
    let spoilt =
        (colony.unemployed + colony.homeless + colony.sick + colony.starving) as f64;
    (total - spoilt) / total
}

/// `mystical_site` (`:308-311`): `Math.max(1, level / 2.0)`.
///
/// Never a penalty: a colony with no mystical site answers `1.0`, which the
/// aggregate then skips entirely, so the modifier costs nothing until the first
/// site is built — and a level-5 site is worth `2.5`.
pub fn mystical_site(colony: &ColonyFacts) -> f64 {
    (colony.mystical_level as f64 / MYSTICAL_DIVISOR).max(1.0)
}

/// `school` (`:39`): `isChild() ? (job instanceof JobPupil ? 2.0 : 0.0) : 1.0`.
///
/// A child without the pupil job is a flat **`0.0`** — worse than illness, worse
/// than homelessness, on a weight of one. That is the whole reason a pupil job
/// exists.
pub fn school(facts: &CitizenFacts) -> f64 {
    if !facts.child {
        return 1.0;
    }
    if facts.job == Some(Job::Pupil) { 2.0 } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    /// The same check with a sentence attached, for a boundary whose *value* is
    /// not the interesting part but whose reason is.
    fn close_at(a: f64, b: f64, what: &str) {
        assert!((a - b).abs() < 1e-9, "{what}: {a} != {b}");
    }

    #[test]
    fn housing_divides_the_built_level_by_three_and_a_missing_home_is_zero() {
        close(housing(&CitizenFacts::new()), 0.0);
        for (level, expected) in [
            (1, 1.0 / 3.0),
            (2, 2.0 / 3.0),
            (3, 1.0),
            (4, 4.0 / 3.0),
            (5, 5.0 / 3.0),
        ] {
            close(housing(&CitizenFacts::new().housed(level)), expected);
        }
    }

    #[test]
    fn a_child_is_never_unemployed_and_only_a_fourth_level_hut_pays() {
        let child = CitizenFacts {
            child: true,
            ..CitizenFacts::new()
        };
        close(unemployment(&child), 1.0);
        close(unemployment(&CitizenFacts::new()), NO_JOB);
        for (level, expected) in [(1, 1.0), (2, 1.0), (3, 1.0), (4, 2.0), (5, 2.0)] {
            close(
                unemployment(&CitizenFacts::new().at(Job::Farmer, level)),
                expected,
            );
        }
    }

    #[test]
    fn an_unloaded_citizen_is_never_ill_for_the_health_factor() {
        let sick = CitizenFacts {
            sick: true,
            ..CitizenFacts::new()
        };
        close(health(&sick), ILL);
        close_at(
            health(&CitizenFacts {
                loaded: false,
                ..sick
            }),
            1.0,
            "ModHappinessFactorTypeInitializer.java:47",
        );
        close(health(&CitizenFacts::new()), 1.0);
    }

    #[test]
    fn standing_idle_at_the_hut_halves_the_factor_and_nothing_else_does() {
        close(idle_at_job(&CitizenFacts::new()), 1.0);
        close(
            idle_at_job(&CitizenFacts {
                idle_at_job: true,
                ..CitizenFacts::new()
            }),
            IDLE,
        );
    }

    #[test]
    fn a_guard_is_neutral_at_slept_tonight_and_everybody_else_starts_at_half() {
        close(slept_tonight(&CitizenFacts::new()), NO_SLEEP);
        close_at(
            slept_tonight(&CitizenFacts::new().at(Job::Knight, 1)),
            1.0,
            "a guard is excluded, not rewarded (:50)",
        );
        // Every guard job takes the branch, including the two that inherit it.
        for job in crate::citizens::GUARD_JOBS {
            close(slept_tonight(&CitizenFacts::new().at(job, 1)), 1.0);
        }
        close(slept_tonight(&CitizenFacts::new().at(Job::Farmer, 5)), NO_SLEEP);
    }

    #[test]
    fn food_says_nothing_without_a_home_or_without_ten_meals() {
        close(food(&CitizenFacts::new()), 1.0);
        let full = FoodHistory::Full {
            diversity: 5,
            quality: 3,
        };
        close_at(
            food(&CitizenFacts {
                food: full,
                ..CitizenFacts::new()
            }),
            1.0,
            "no home: homeBuildingLevel == 0 (:287)",
        );
        close_at(
            food(&CitizenFacts::new().housed(3)),
            1.0,
            "a short history is not judged (:287)",
        );
    }

    #[test]
    fn food_averages_variety_and_quality_on_two_different_divisors() {
        let facts = CitizenFacts {
            food: FoodHistory::Full {
                diversity: 5,
                quality: 3,
            },
            ..CitizenFacts::new().housed(5)
        };
        // diversity 5/5 = 1.0; quality 3/max(1, 3) = 1.0 — a level-5 house fed
        // five kinds, three of them MineColonies food, is exactly neutral.
        close(food(&facts), 1.0);

        let facts = CitizenFacts {
            food: FoodHistory::Full {
                diversity: 10,
                quality: 10,
            },
            ..CitizenFacts::new().housed(1)
        };
        // A level-1 house fed ten kinds, all of them MineColonies food: both
        // halves would be 10.0 and are capped at 5.0, so the answer is 5.0.
        close(food(&facts), FOOD_HALF_CAP);
        // The same ten meals in a level-5 house are judged against 5 and 3.
        let grand = CitizenFacts {
            food: FoodHistory::Full {
                diversity: 10,
                quality: 10,
            },
            ..CitizenFacts::new().housed(5)
        };
        close(food(&grand), (2.0 + 10.0 / 3.0) / 2.0);

        let facts = CitizenFacts {
            food: FoodHistory::Full {
                diversity: 2,
                quality: 0,
            },
            ..CitizenFacts::new().housed(1)
        };
        // 2/1 = 2.0 and 0/max(1, -1) = 0.0 → 1.0.
        close(food(&facts), 1.0);
    }

    #[test]
    fn a_lone_citizen_already_feels_undefended() {
        let alone = ColonyFacts::counted(&[Head::default()], 0);
        close_at(security(&alone), 0.75, "1 / (2 · 2/3), :256-257");
        // One guard for two workers is the neutral line: both counters already
        // hold one before the loop, so the ratio is 2 / (3·2/3) = 1.0
        // (`CitizenHappinessHandler.java:256-257`, `:269`).
        let guarded = ColonyFacts {
            guards: 1,
            workers: 2,
            ..ColonyFacts::default()
        };
        close(security(&guarded), 1.0);
        // And two guards for three workers is already past it: 3 / (4·2/3).
        let ahead = ColonyFacts {
            guards: 2,
            workers: 3,
            ..ColonyFacts::default()
        };
        close(security(&ahead), 1.125);
        // And it stops at two.
        let fortress = ColonyFacts {
            guards: 20,
            workers: 1,
            ..ColonyFacts::default()
        };
        close(security(&fortress), SECURITY_CAP);
        // No guards at all in a colony of ten: 1 / (11 · 2/3).
        let undefended = ColonyFacts {
            guards: 0,
            workers: 10,
            ..ColonyFacts::default()
        };
        close(security(&undefended), 1.0 / (11.0 * 2.0 / 3.0));
    }

    #[test]
    fn social_subtracts_every_spoilt_count_and_double_counts_the_wretched() {
        let citizens = [
            Head {
                employed: true,
                housed: true,
                ..Head::default()
            },
            Head {
                employed: true,
                housed: true,
                sick: true,
                ..Head::default()
            },
        ];
        let facts = ColonyFacts::counted(&citizens, 0);
        close(social(&facts), 0.5);

        // One citizen with no home, no job and no food counts three times.
        let wretched = [Head {
            starving: true,
            ..Head::default()
        }];
        close(social(&ColonyFacts::counted(&wretched, 0)), -2.0);
        // And a colony with nobody in it divides zero by zero, like Java.
        assert!(social(&ColonyFacts::default()).is_nan());
    }

    #[test]
    fn a_child_counts_as_out_of_work_only_when_it_is_not_a_pupil() {
        let lacking = ColonyFacts::counted(
            &[Head {
                child: true,
                ..Head::default()
            }],
            0,
        );
        assert_eq!(lacking.unemployed, 0, ":224 — the job check is adult only");
        assert_eq!(lacking.workers, 1);
    }

    #[test]
    fn mystical_site_never_penalises_and_pays_two_and_a_half_at_the_top() {
        for level in -1..=1 {
            let facts = ColonyFacts {
                mystical_level: level,
                ..ColonyFacts::default()
            };
            close(mystical_site(&facts), 1.0);
        }
        for (level, expected) in [(2, 1.0), (3, 1.5), (4, 2.0), (5, 2.5)] {
            let facts = ColonyFacts {
                mystical_level: level,
                ..ColonyFacts::default()
            };
            close(mystical_site(&facts), expected);
        }
    }

    #[test]
    fn a_child_out_of_school_is_a_flat_zero_and_a_adult_is_never_judged() {
        let adult = CitizenFacts::new();
        close(school(&adult), 1.0);
        let child = CitizenFacts {
            child: true,
            ..CitizenFacts::new()
        };
        close(school(&child), 0.0);
        close(
            school(&CitizenFacts {
                child: true,
                job: Some(Job::Pupil),
                ..CitizenFacts::new()
            }),
            2.0,
        );
    }

    #[test]
    fn the_counted_loop_reproduces_six_numbers_from_one_list() {
        let citizens = [
            Head {
                guard: true,
                employed: true,
                ..Head::default()
            },
            Head {
                child: true,
                housed: true,
                employed: false,
                ..Head::default()
            },
            Head {
                employed: true,
                housed: true,
                starving: true,
                ..Head::default()
            },
        ];
        let facts = ColonyFacts::counted(&citizens, 4);
        assert_eq!(facts.citizens, 3);
        assert_eq!(facts.unemployed, 0, "the child is the only one hired by nobody");
        assert_eq!(facts.homeless, 1, "the guard and the worker have a bed");
        assert_eq!(facts.sick, 0);
        assert_eq!(facts.starving, 1);
        assert_eq!(facts.guards, 1);
        assert_eq!(facts.workers, 2);
        assert_eq!(facts.mystical_level, 4);
    }
}
