//! What a citizen needs — food, health, sleep — and how a tick moves it.
//!
//! Java reference: `core/colony/CitizenData.java:1158-1204` (the saturation
//! field and its two writers), `core/entity/citizen/EntityCitizen.java:279-288`
//! (the tick rate of each decay), `:887-910` (`checkHeal`), `:1970-1996`
//! (`decreaseIdleSaturation`), `core/entity/citizen/citizenhandlers/
//! CitizenSleepHandler.java:232-280` (`shouldGoSleep`), `api/util/FoodUtils.java:
//! 393-405` (the consumption factor) and `CitizenConstants.java` (the thresholds).
//!
//! **No clock, and that is the design.** Java calls `level.getDayTime()` and
//! schedules its decays on `TickingTransition` counters. Here the day time is a
//! field of [`World`] the caller fills, and the counters live in [`Needs`], so
//! `advance(72_000, …)` is a pure function of its arguments and a night fits in
//! three lines of test.
//!
//! | Value | Number | Where |
//! | --- | ---: | --- |
//! | `MAX_SATURATION` | 60 | `api/colony/ICitizenData.java:30` |
//! | `AVERAGE_SATURATION` / `LOW_SATURATION` | 10 / 6 | `CitizenConstants.java:50`, `:55` |
//! | `RESTAURANT_LIMIT` / `SEEK_DOCTOR_HEALTH` | 2.5 / 6 | `EntityAIEatTask.java:74`, `CitizenDiseaseHandler.java:41` |
//! | `MAX_HEALTH` / `HEAL_CITIZENS_AFTER` | 20 / 100 | `CitizenData.java:82`, `CitizenConstants.java:58` |
//! | `SATURATION_DECREASE_AFTER` | 1200 | `CitizenConstants.java:63` |
//! | `NIGHT` / `NOON` | 12600 / 6000 | `:203`, `:198` |
//!
//! **Two things are not modelled, and each is named where it bites.** `checkHeal`
//! also needs `getLastHurtByMob() == null` (`:889`) — a citizen just hit does not
//! regenerate — which is a field of the entity, not of the need; and the miner's
//! detour in `shouldGoSleep` (`:247-251`) adds the distance to its shaft, which
//! needs the job and its building.

/// `ICitizenData.MAX_SATURATION` (`api/colony/ICitizenData.java:30`).
pub const MAX_SATURATION: f64 = 60.0;
/// `CitizenConstants.AVERAGE_SATURATION` (`:50`): "smaller than this = bad".
pub const AVERAGE_SATURATION: f64 = 10.0;
/// `CitizenConstants.LOW_SATURATION` (`:55`).
pub const LOW_SATURATION: f64 = 6.0;
/// `FULL_SATURATION`, which Java writes as an alias of the maximum (`:60`).
pub const FULL_SATURATION: f64 = MAX_SATURATION;
/// `EntityAIEatTask.RESTAURANT_LIMIT` (`EntityAIEatTask.java:74`).
pub const RESTAURANT_LIMIT: f64 = 2.5;
/// `CitizenData.MAX_HEALTH` (`core/colony/CitizenData.java:82`).
pub const MAX_HEALTH: f32 = 20.0;
/// `CitizenDiseaseHandler.SEEK_DOCTOR_HEALTH` (`:41`).
pub const SEEK_DOCTOR_HEALTH: f32 = 6.0;
/// `CitizenConstants.SATURATION_DECREASE_AFTER` (`:63`): 20 × 60, one minute.
pub const SATURATION_DECREASE_AFTER: u32 = 20 * 60;
/// `CitizenConstants.HEAL_CITIZENS_AFTER` (`:58`).
pub const HEAL_CITIZENS_AFTER: u32 = 100;
/// `CitizenConstants.NIGHT` (`:203`): "point at which you can sleep".
pub const NIGHT: i64 = 12600;
/// `CitizenConstants.NOON` (`:198`).
pub const NOON: i64 = 6000;
/// One Minecraft day: the modulus Java applies before any hour comparison
/// (`WorldUtil.java:178`, `CitizenAI.java:244`).
pub const TICKS_PER_DAY: i64 = 24000;

/// The ambient facts one decision reads. A hook fills them from its level; this
/// module only reads them, which is why a whole night fits in a test.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct World {
    /// `level.getDayTime()`, the monotonic counter (`CitizenAI.java:244`).
    pub day_time: i64,
    /// `level.isRaining()` (`:232`).
    pub raining: bool,
    /// `WorldUtil.isNetherType(citizen.level)` (`:232`): it never rains there.
    pub nether: bool,
    /// `colony.getRaiderManager().isRaided()` (`:161`).
    pub raided: bool,
}

impl World {
    /// A dry, peaceful overworld at `day_time`.
    pub fn at_day_time(day_time: i64) -> World {
        World {
            day_time,
            ..World::default()
        }
    }

    /// `getDayTime() % 24000`, without the negative remainder Java's `%` gives.
    pub fn time_of_day(&self) -> i64 {
        self.day_time.rem_euclid(TICKS_PER_DAY)
    }

    /// `WorldUtil.isPastTime` (`:176-179`): `dayTime % 24000 <= pastTime`.
    ///
    /// The name reads backwards for the sleeping rule at `CitizenAI.java:169`:
    /// `!isPastTime(NIGHT - 2000)` is **true after 10 600 ticks**, i.e. from late
    /// afternoon until dawn — the window in which a citizen may go to bed.
    pub fn is_past_time(&self, past: i64) -> bool {
        self.time_of_day() <= past
    }
}

/// The arguments of `decreaseIdleSaturation` (`EntityCitizen.java:1970-1996`),
/// `decreaseSaturation` (`CitizenData.java:1164-1170`) and `checkHeal`
/// (`:887-910`), in one value so [`Needs::advance`] takes a single parameter.
#[derive(Clone, Copy, Debug, PartialEq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Advance {
    /// `level.isNight()`, vanilla's own flag (`:1976`). Taken rather than
    /// recomputed: vanilla's `isNight` is not `dayTime > NIGHT`.
    pub night: bool,
    /// The home building's level, `0` with no home (`:1977`) — the `default`
    /// branch of the factor, `0.3`.
    pub house_level: i32,
    /// `job.getSaturationFactor()` (`IJob.java:323-326`, `1.0` by default).
    pub job_factor: f64,
    /// `cachedActionSaturationDecrease`, accrued since the last check
    /// (`EntityCitizen.java:1082-1094`).
    pub actions: f64,
    /// `citizenData.isChild()` — a child consumes half (`:1990-1993`).
    pub child: bool,
    /// The server's `foodModifier`, applied to every decrease
    /// (`CitizenData.java:1168`).
    pub food_modifier: f64,
    /// `REGENERATION` research strength (`:898`, `:906`).
    pub regeneration: f64,
    /// `SATLIMIT` research strength (`:896`): it raises the doubling point.
    pub saturation_limit: f64,
}

impl Default for Advance {
    fn default() -> Advance {
        Advance {
            night: false,
            house_level: 3,
            job_factor: 1.0,
            actions: 0.0,
            child: false,
            food_modifier: 1.0,
            regeneration: 0.0,
            saturation_limit: 0.0,
        }
    }
}

impl Advance {
    /// A built level-`n` home, no research, default config. Level 3 is the
    /// `1.0` factor, so the arithmetic in a test stays readable.
    pub fn at_level(house_level: i32) -> Advance {
        Advance {
            house_level,
            ..Advance::default()
        }
    }
}

/// `FoodUtils.computeSaturationConsumptionFactor` (`:393-405`): what one idle
/// check burns, by housing level. The `default` arm (`0.3`) catches level `0` —
/// a citizen with no home — and anything outside 1..=5.
pub fn consumption_factor(house_level: i32) -> f64 {
    match house_level {
        1 => 0.6,
        2 => 0.725,
        3 => 1.0,
        4 => 1.2,
        5 => 1.5,
        _ => 0.3,
    }
}

/// What `shouldEat` reads besides the needs (`CitizenAI.java:268-298`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Appetite {
    /// `getJob() != null` (`:275`).
    pub has_job: bool,
    /// `job.canAIBeInterrupted()` (`:275` → `AbstractJob.java:353-359`).
    pub interruptible: bool,
    /// `lastState == EATING` (`:280`): a citizen already eating eats on, whatever
    /// its job says.
    pub was_eating: bool,
    /// The cook's skip: Java draws `random.nextInt(200) > 0` for the cook, who
    /// walks to the restaurant one time in two hundred (`:285-288`). A wasm mod
    /// that read a random source would not be reproducible, so the caller passes
    /// the answer instead of the port drawing it.
    pub cook_skips: bool,
}

/// The citizen's four needs, plus the two tick counters Java schedules them on.
///
/// **Saturation is the only one that moves on its own.** Health is derived from
/// it (`checkHeal`); sleep is a state, not a gauge (`shouldGoSleep` answers "is it
/// late enough for the walk home"); hunger is a *threshold* on saturation, not a
/// field. There is no separate hunger number in Java and the port invents none.
#[derive(Clone, Copy, Debug, PartialEq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Needs {
    /// `0..=MAX_SATURATION`, a `double` in Java (`CitizenData.java:196`).
    pub saturation: f64,
    /// `0..=MAX_HEALTH`, vanilla's health attribute.
    pub health: f32,
    /// `justAte`, set by the eat AI and cleared by the next decrease (`:201`, `:1169`).
    pub just_ate: bool,
    /// `isAsleep`, written by `CitizenSleepHandler`.
    pub asleep: bool,
    /// `disease != null` (`CitizenDiseaseHandler.java:182-185`).
    pub sick: bool,
    /// `sleepsAtHospital` (`:258-261`): a sick citizen in a hospital bed is
    /// `SICK` before it is anything else (`CitizenAI.java:154-158`).
    pub sleeps_at_hospital: bool,
    /// `CitizenMournHandler.isMourning()` (`:96-100`).
    pub mourning: bool,
    /// Ticks left before the next saturation check, `1..=SATURATION_DECREASE_AFTER`.
    pub until_saturation_check: u32,
    /// Ticks left before the next heal check, `1..=HEAL_CITIZENS_AFTER`.
    pub until_heal_check: u32,
}

impl Default for Needs {
    fn default() -> Needs {
        Needs::new()
    }
}

impl Needs {
    /// As `CitizenData` creates one: saturation at its maximum
    /// (`CitizenData.java:526`), health at its maximum, nothing else set.
    pub fn new() -> Needs {
        Needs {
            saturation: MAX_SATURATION,
            health: MAX_HEALTH,
            just_ate: false,
            asleep: false,
            sick: false,
            sleeps_at_hospital: false,
            mourning: false,
            until_saturation_check: SATURATION_DECREASE_AFTER,
            until_heal_check: HEAL_CITIZENS_AFTER,
        }
    }

    /// At or below the average line: the first half of `shouldEat`'s last
    /// condition (`CitizenAI.java:295`).
    pub fn hungry(&self) -> bool {
        self.saturation <= AVERAGE_SATURATION
    }

    /// `CitizenDiseaseHandler.isHurt` (`:175-180`): badly hurt, **not** a guard,
    /// and not starving — a starving citizen is sent to eat, not to a doctor.
    pub fn hurt(&self, guard: bool) -> bool {
        !guard && self.health < SEEK_DOCTOR_HEALTH && self.saturation > LOW_SATURATION
    }

    /// `CitizenAI.shouldEat` (`:268-298`), gate by gate.
    ///
    /// Six of them, and the two that surprise: a citizen that was already eating
    /// is **always** allowed to go on (`:280`, before the cook's draw), and a sick
    /// citizen asleep in bed does not wake up for food (`:290-293`).
    pub fn should_eat(&self, appetite: &Appetite) -> bool {
        if self.just_ate || self.saturation >= FULL_SATURATION {
            return false;
        }
        if appetite.has_job && !appetite.interruptible {
            return false;
        }
        if appetite.was_eating {
            return true;
        }
        if appetite.cook_skips {
            return false;
        }
        if self.sick && self.asleep {
            return false;
        }
        self.saturation <= AVERAGE_SATURATION
            && (self.saturation <= RESTAURANT_LIMIT
                || (self.saturation < LOW_SATURATION && self.health < SEEK_DOCTOR_HEALTH))
    }

    /// What one heal check would add — `checkHeal`'s three-way branch
    /// (`EntityCitizen.java:892-908`), returned rather than applied.
    ///
    /// A sick citizen only regenerates up to a third of its maximum; a starving
    /// one heals a fraction of a point; a full one heals double.
    pub fn heal_amount(&self, advance: &Advance) -> f32 {
        let cap = if self.sick {
            MAX_HEALTH / 3.0
        } else {
            MAX_HEALTH
        };
        if self.health >= cap {
            return 0.0;
        }
        if self.saturation >= FULL_SATURATION + advance.saturation_limit {
            (2.0 * (1.0 + advance.regeneration)) as f32
        } else if self.saturation < LOW_SATURATION {
            ((self.saturation / FULL_SATURATION) / 2.0) as f32
        } else {
            (1.0 + advance.regeneration) as f32
        }
    }

    /// Advance by `ticks`, one saturation check and one heal check per period.
    ///
    /// Java schedules them on `TickingTransition`s of rate 1200 and 100
    /// (`EntityCitizen.java:279`, `:288`); the countdown below is the same
    /// mechanism spelled out, so 2 400 ticks is exactly two checks.
    pub fn advance(&mut self, ticks: u32, advance: &Advance) {
        for _ in 0..ticks {
            self.until_saturation_check = self.until_saturation_check.saturating_sub(1);
            if self.until_saturation_check == 0 {
                self.until_saturation_check = SATURATION_DECREASE_AFTER;
                if !advance.night && !self.asleep {
                    self.decrease(advance);
                }
            }
            self.until_heal_check = self.until_heal_check.saturating_sub(1);
            if self.until_heal_check == 0 {
                self.until_heal_check = HEAL_CITIZENS_AFTER;
                self.health = (self.health + self.heal_amount(advance)).min(MAX_HEALTH);
            }
        }
    }

    /// One `decreaseIdleSaturation` (`EntityCitizen.java:1970-1996`), in Java's
    /// order: housing factor, the job's, at most half the base again for the
    /// accrued actions, the child halving, then the config's `foodModifier`
    /// inside `decreaseSaturation` (`CitizenData.java:1168`) which clears
    /// `justAte`.
    fn decrease(&mut self, advance: &Advance) {
        let mut decrease = consumption_factor(advance.house_level);
        decrease *= advance.job_factor;
        if advance.actions != 0.0 {
            decrease += (decrease / 2.0).min(advance.actions);
        }
        if advance.child {
            decrease /= 2.0;
        }
        self.saturation = (self.saturation - (decrease * advance.food_modifier).abs()).max(0.0);
        self.just_ate = false;
    }
}

/// `CitizenSleepHandler.shouldGoSleep` (`:232-280`): is it late enough, and is the
/// walk home short enough, to go to bed now?
///
/// - **where**: no home, `false` (`:234`); invisible, never (`:240-243`);
/// - **when**: `timeNeeded = sqrt(dx² + dz² + (1.5·dy)²) · 6` ticks (`:266-268`)
///   and `timeLeft = NIGHT + workLonger·1000 − dayTime % 24000` (`:271-272`).
///   The citizen leaves when `timeLeft <= 0` or when it would arrive after
///   `NIGHT` — "estimated arrival is 1 hour past night", says the comment.
///
/// `work_longer` is the `WORK_LONGER` research strength, in thousands of ticks.
/// The miner's detour (`:247-251`) is not ported: it needs the job and its shaft.
pub fn should_go_sleep(
    home: Option<[i32; 3]>,
    at: [i32; 3],
    world: &World,
    work_longer: i64,
    invisible: bool,
) -> bool {
    if invisible {
        return false;
    }
    let Some(home) = home else {
        return false;
    };
    let x = (home[0] - at[0]) as f64;
    let z = (home[2] - at[2]) as f64;
    let y = (home[1] - at[1]) as f64 * 1.5;
    let time_needed = (x * x + z * z + y * y).sqrt() * 6.0;
    let time_left = (NIGHT + work_longer * 1000 - world.time_of_day()) as f64;
    time_left <= 0.0 || time_left - time_needed <= 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    #[test]
    fn one_check_burns_the_housing_factor_and_only_by_day_and_awake() {
        let cases = [(0, 0.3), (1, 0.6), (2, 0.725), (3, 1.0), (4, 1.2), (5, 1.5), (9, 0.3)];
        for (level, expected) in cases {
            assert_eq!(consumption_factor(level), expected, "level {level}");
            let mut needs = Needs::new();
            needs.advance(1200, &Advance::at_level(level));
            close(needs.saturation, 60.0 - expected);
        }
        // Two periods burn twice; night and sleep burn nothing.
        let mut twice = Needs::new();
        twice.advance(2400, &Advance::at_level(3));
        close(twice.saturation, 58.0);
        for advance in [
            Advance {
                night: true,
                ..Advance::at_level(3)
            },
            Advance::at_level(3),
        ] {
            let mut needs = Needs::new();
            needs.asleep = true;
            needs.advance(2400, &advance);
            close(needs.saturation, 60.0);
        }
    }

    #[test]
    fn the_action_credit_the_child_halving_and_the_food_modifier_all_apply() {
        // 0.2 accrued against a base of 1.0: +0.2; 5.0 is capped at half the base.
        for (actions, burned) in [(0.2, 1.2), (5.0, 1.5)] {
            let mut needs = Needs::new();
            needs.advance(
                1200,
                &Advance {
                    actions,
                    ..Advance::at_level(3)
                },
            );
            close(needs.saturation, 60.0 - burned);
        }
        let mut child = Needs::new();
        child.advance(
            1200,
            &Advance {
                child: true,
                ..Advance::at_level(3)
            },
        );
        close(child.saturation, 59.5);
        let mut doubled = Needs::new();
        doubled.advance(
            1200,
            &Advance {
                food_modifier: 2.0,
                ..Advance::at_level(3)
            },
        );
        close(doubled.saturation, 58.0);
        // And the gauge floors at zero, clearing `justAte` on the way.
        let mut empty = Needs::new();
        empty.saturation = 0.5;
        empty.just_ate = true;
        empty.advance(1200, &Advance::at_level(3));
        close(empty.saturation, 0.0);
        assert!(!empty.just_ate, "CitizenData.java:1169");
    }

    #[test]
    fn hunger_is_a_threshold_with_a_restaurant_line() {
        let appetite = Appetite {
            has_job: false,
            interruptible: true,
            was_eating: false,
            cook_skips: false,
        };
        let mut needs = Needs::new();
        for (saturation, hungry, eats) in [
            (11.0, false, false),
            (10.0, true, false),
            (2.5, true, true),
            (5.0, true, false), // under 6 but above the restaurant line, and healthy
        ] {
            needs.saturation = saturation;
            assert_eq!(needs.hungry(), hungry, "at {saturation}");
            assert_eq!(needs.should_eat(&appetite), eats, "at {saturation}");
        }
        needs.health = 5.0;
        assert!(needs.should_eat(&appetite), "wounded and under 6: it eats");
    }

    #[test]
    fn should_eat_has_six_gates_and_each_one_alone_stops_it() {
        let base = Appetite {
            has_job: false,
            interruptible: true,
            was_eating: false,
            cook_skips: false,
        };
        let mut needs = Needs::new();
        needs.saturation = 2.0;
        assert!(needs.should_eat(&base));
        let mut full = needs;
        full.saturation = FULL_SATURATION;
        assert!(!full.should_eat(&base));
        let mut ate = needs;
        ate.just_ate = true;
        assert!(!ate.should_eat(&base));
        let uninterruptible = Appetite {
            has_job: true,
            interruptible: false,
            ..base
        };
        assert!(!needs.should_eat(&uninterruptible));
        // `:275` comes before `:280`: an AI that may not be interrupted stops the
        // meal even for a citizen that was already eating.
        assert!(!needs.should_eat(&Appetite {
            was_eating: true,
            ..uninterruptible
        }));
        // Where `:280` does bite: above the eat threshold, only the memory of
        // being at the table keeps it eating — `:270` passed, `:275` has no job.
        let mut sated = Needs::new();
        sated.saturation = 40.0;
        assert!(!sated.should_eat(&base));
        assert!(
            sated.should_eat(&Appetite {
                was_eating: true,
                ..base
            }),
            "a citizen already eating eats on to fullness (CitizenAI.java:280)"
        );
        assert!(!needs.should_eat(&Appetite {
            cook_skips: true,
            ..base
        }));
        let mut sick = needs;
        sick.sick = true;
        sick.asleep = true;
        assert!(!sick.should_eat(&base));
        sick.asleep = false;
        assert!(sick.should_eat(&base), "awake and sick, it still eats");
    }

    #[test]
    fn hurt_is_neither_a_guard_nor_a_starving_citizen() {
        let mut needs = Needs::new();
        needs.health = 5.0;
        needs.saturation = 30.0;
        assert!(needs.hurt(false));
        assert!(!needs.hurt(true), "a guard is never sent to the doctor");
        needs.saturation = LOW_SATURATION;
        assert!(!needs.hurt(false), "at the low line it is a meal");
        needs.saturation = 30.0;
        needs.health = SEEK_DOCTOR_HEALTH;
        assert!(!needs.hurt(false), "the comparison is strictly below");
    }

    #[test]
    fn healing_is_three_ways_capped_by_the_disease_and_climbs_every_hundred_ticks() {
        let advance = Advance::default();
        let mut full = Needs::new();
        full.health = 10.0;
        assert_eq!(full.heal_amount(&advance), 2.0);
        let mut normal = Needs::new();
        normal.saturation = 30.0;
        normal.health = 10.0;
        assert_eq!(normal.heal_amount(&advance), 1.0);
        let mut starving = Needs::new();
        starving.saturation = 3.0;
        starving.health = 10.0;
        assert!((starving.heal_amount(&advance) - 0.025).abs() < 1e-6);
        let mut sick = Needs::new();
        sick.sick = true;
        sick.health = 7.0;
        assert_eq!(sick.heal_amount(&advance), 0.0, "a third of 20 is the cap");
        sick.health = 5.0;
        assert_eq!(sick.heal_amount(&advance), 2.0);

        let mut needs = Needs::new();
        needs.health = 5.0;
        needs.saturation = 30.0;
        needs.advance(100, &Advance::at_level(3));
        close(needs.health as f64, 6.0);
        needs.advance(10_000, &Advance::at_level(3));
        assert_eq!(needs.health, MAX_HEALTH, "heal() clamps at the maximum");
    }

    #[test]
    fn a_night_heals_but_does_not_feed() {
        let mut needs = Needs::new();
        needs.health = 1.0;
        needs.saturation = 30.0;
        needs.advance(
            1200,
            &Advance {
                night: true,
                ..Advance::at_level(3)
            },
        );
        close(needs.health as f64, 13.0);
        // Started at 30.0, and `decreaseIdleSaturation` burns nothing at night
        // (`EntityCitizen.java:1976`) — the night heals, it does not feed.
        close(needs.saturation, 30.0);
    }

    #[test]
    fn the_world_takes_the_day_time_modulo_and_reads_past_time_java_s_way() {
        let world = World::at_day_time(24_000 + 12_000);
        assert_eq!(world.time_of_day(), 12_000, "day two, noon");
        assert!(world.is_past_time(12_600));
        assert!(!world.is_past_time(11_999));
        // The comparison is `<=` (`WorldUtil.java:178`), so the boundary itself is
        // still "day": the sleeping window — `!isPastTime(NIGHT - 2000)`, what
        // `CitizenAI.java:169` asks — opens at 10 601, not at 10 600.
        assert!(World::at_day_time(NIGHT - 2001).is_past_time(NIGHT - 2000));
        assert!(World::at_day_time(NIGHT - 2000).is_past_time(NIGHT - 2000));
        assert!(!World::at_day_time(NIGHT - 1999).is_past_time(NIGHT - 2000));
        assert!(
            !World::at_day_time(11_500).is_past_time(NIGHT - 2000),
            "11 500 is past 10 600, so the window is open there"
        );
        assert_eq!(World::at_day_time(-1).time_of_day(), 23_999);
    }

    #[test]
    fn should_go_sleep_needs_a_home_a_walk_and_the_hour() {
        let home = [0, 64, 0];
        let noon = World::at_day_time(NOON);
        assert!(!should_go_sleep(Some(home), home, &noon, 0, false), "noon");
        assert!(!should_go_sleep(None, home, &noon, 0, false), "no home");
        assert!(
            !should_go_sleep(Some(home), home, &noon, 0, true),
            "invisible (CitizenSleepHandler.java:240)"
        );

        // 601 ticks left: 100 blocks on the flat want 600 of them, so wait; 40
        // blocks up are weighted to 116.6 blocks (700 ticks), so leave now.
        let late = World::at_day_time(NIGHT - 601);
        assert!(!should_go_sleep(Some(home), [100, 64, 0], &late, 0, false));
        assert!(should_go_sleep(Some(home), [100, 104, 0], &late, 0, false));
        // Exactly enough is not enough: the test is `<= 0`.
        let exact = World::at_day_time(NIGHT - 600);
        assert!(should_go_sleep(Some(home), [100, 64, 0], &exact, 0, false));
        // WORK_LONGER 2 buys 2 000 ticks: at NIGHT + 1, 1 999 remain.
        let one_late = World::at_day_time(NIGHT + 1);
        assert!(should_go_sleep(Some(home), home, &one_late, 0, false));
        assert!(!should_go_sleep(Some(home), home, &one_late, 2, false));
    }
}
