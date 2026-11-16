//! One happiness modifier: its id, its weight, where its factor comes from, and
//! how many days it has been in force.
//!
//! Java reference: `api/entity/citizen/happiness/IHappinessModifier.java:10-48`
//! (the interface, five methods), `AbstractHappinessModifier.java:60-140` (id,
//! weight, the supplier behind the factor), `StaticHappinessModifier.java:149-177`,
//! `TimeBasedHappinessModifier.java:19-161` and
//! `ExpirationBasedHappinessModifier.java:174-256` (the two that move), plus the
//! ten constructions in
//! `core/entity/citizen/citizenhandlers/CitizenHappinessHandler.java:53-80`.
//!
//! **Three shapes, not one.** Java registers three types and the differences are
//! real, so [`Kind`] keeps them apart:
//!
//! | Kind | `getFactor` | `dayEnd` | Built with |
//! | --- | --- | --- | --- |
//! | [`Kind::Static`] | the supplier, unchanged | nothing | five at birth |
//! | [`Kind::Time`] | the supplier, **decayed by a ramp** while it is below 1 | `days++` or reset | five at birth |
//! | [`Kind::Expiration`] | the supplier while `days > 0`, else `1.0` | `days--` | the five event ones |
//!
//! **A time-based factor only ever decays downwards.** `if (baseFactor < 1.0)`
//! guards the whole ramp (`TimeBasedHappinessModifier.java:86`), so a citizen in a
//! level-5 home or working a level-4 hut keeps the supplier's value forever and a
//! homeless one keeps decaying. Nothing in Java lets a ramp lift a factor.
//!
//! **The ramp is read top to bottom and the last match wins**
//! (`:88-94`): `if (days >= step.getA()) factor = base * step.getB()`. The steps
//! are stored in ascending day order, so "at seven days" is `0.75·base` and "at
//! fourteen" is `0.5·base`, not both.
//!
//! **`SLEPTTONIGHT` is the one with a predicate instead of a test.** Its
//! constructor passes `(modifier, data) -> true` (`CitizenHappinessHandler.java:80`),
//! so its day counter climbs every single night and only `resetModifier` — called
//! when the citizen actually lies down (`EntityAISleep.java:213`) — puts it back
//! to zero. Its ramp starts at day **zero**, which is why its own base of `0.5`
//! comes out as `1.0` on the night it slept and as `0.5` two days later.
//!
//! **What is not here.** The NBT codec (`read`/`write`, `:111-133`), the forge
//! registries (`HappinessRegistry.java:27-40`) and the GUI's
//! `getFactor(null)` cached read (`:93`) all need a game this module does not
//! touch; the fields they persist are the ones this struct holds.

use super::factors::{self, Facts};

/// The fifteen ids of `HappinessConstants.java:47-77`, in its declaration order.
///
/// `Java` keeps them as strings in a `Set` and looks a modifier up by name; the
/// enum makes the list closed at compile time and [`Source::name`] gives back the
/// string, so NBT and the GUI's translation keys still line up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: `VALID_HAPPINESS_MODIFIERS` is a closed set in Java (:63-77)
pub enum Source {
    /// `HappinessConstants.HOMELESSNESS` (`:47`).
    Homelessness,
    /// `HappinessConstants.UNEMPLOYMENT` (`:48`).
    Unemployment,
    /// `HappinessConstants.HEALTH` (`:49`).
    Health,
    /// `HappinessConstants.IDLEATJOB` (`:50`).
    IdleAtJob,
    /// `HappinessConstants.SCHOOL` (`:51`).
    School,
    /// `HappinessConstants.MYSTICAL_SITE` (`:52`).
    MysticalSite,
    /// `HappinessConstants.SECURITY` (`:53`).
    Security,
    /// `HappinessConstants.SOCIAL` (`:54`).
    Social,
    /// `HappinessConstants.DAMAGE` (`:55`).
    Damage,
    /// `HappinessConstants.DEATH` (`:56`).
    Death,
    /// `HappinessConstants.RAIDWITHOUTDEATH` (`:57`).
    RaidWithoutDeath,
    /// `HappinessConstants.SLEPTTONIGHT` (`:58`).
    SleptTonight,
    /// `HappinessConstants.QUEST` (`:59`).
    Quest,
    /// `HappinessConstants.FOOD` (`:60`).
    Food,
    /// `HappinessConstants.HADGREATFOOD` (`:61`).
    GreatFood,
}

impl Source {
    /// The fifteen, in `HappinessConstants.java` declaration order.
    pub const ALL: [Source; 15] = [
        Source::Homelessness,
        Source::Unemployment,
        Source::Health,
        Source::IdleAtJob,
        Source::School,
        Source::MysticalSite,
        Source::Security,
        Source::Social,
        Source::Damage,
        Source::Death,
        Source::RaidWithoutDeath,
        Source::SleptTonight,
        Source::Quest,
        Source::Food,
        Source::GreatFood,
    ];

    /// The ten a citizen is born with, in the order
    /// `CitizenHappinessHandler.java:53-80` adds them: five static, then five
    /// time-based.
    pub const AT_BIRTH: [Source; 10] = [
        Source::School,
        Source::Security,
        Source::Social,
        Source::MysticalSite,
        Source::Food,
        Source::Homelessness,
        Source::Unemployment,
        Source::Health,
        Source::IdleAtJob,
        Source::SleptTonight,
    ];

    /// The registry string, without the mod namespace — what
    /// `VALID_HAPPINESS_MODIFIERS` holds and what the lang keys concatenate
    /// (`CitizenHappinessHandler.java:124-131`).
    pub fn name(self) -> &'static str {
        match self {
            Source::Homelessness => "homelessness",
            Source::Unemployment => "unemployment",
            Source::Health => "health",
            Source::IdleAtJob => "idleatjob",
            Source::School => "school",
            Source::MysticalSite => "mysticalsite",
            Source::Security => "security",
            Source::Social => "social",
            Source::Damage => "damage",
            Source::Death => "death",
            Source::RaidWithoutDeath => "raidwithoutdeath",
            Source::SleptTonight => "slepttonight",
            Source::Quest => "quest",
            Source::Food => "food",
            Source::GreatFood => "greatfood",
        }
    }

    /// The source from a saved name, `None` when the name is not one of the
    /// fifteen — Java drops such an entry and keeps the defaults
    /// (`CitizenHappinessHandler.java:174-181`).
    pub fn from_name(name: &str) -> Option<Source> {
        Source::ALL.iter().copied().find(|src| src.name() == name)
    }

    /// Whether a citizen starts with it. The other five only arrive from an
    /// event: a wound, a death, a raid, a quest, a great meal.
    pub fn at_birth(self) -> bool {
        Source::AT_BIRTH.contains(&self)
    }
}

/// Which of the three registered modifier types this is
/// (`HappinessRegistry.java:129-131`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: three registered types, one `DeferredRegister` entry each
pub enum Kind {
    /// `minecolonies:static` — the factor never moves.
    Static,
    /// `minecolonies:time` — `days` climbs and the ramp decays the factor.
    Time,
    /// `minecolonies:expiration` — `days` falls and the factor expires to `1.0`.
    Expiration,
}

impl Kind {
    /// The registry path, without namespace (`HappinessRegistry.java:129-131`).
    pub fn name(self) -> &'static str {
        match self {
            Kind::Static => "static",
            Kind::Time => "time",
            Kind::Expiration => "expiration",
        }
    }
}

/// One step of a ramp: from `days` on, the base factor is multiplied by
/// `multiplier`.
///
/// `source` is the Java expression that states the pair, because every ramp in
/// this file is a pair of magic numbers and a reader deserves the line rather
/// than the author's word for it.
#[derive(Clone, Copy, Debug, PartialEq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Step {
    /// `Tuple.getA()`, the first day the step applies (`:88-90`).
    pub days: i32,
    /// `Tuple.getB()`, what the base factor is multiplied by (`:92`).
    pub multiplier: f64,
    /// `fichier.java:ligne` that states the pair.
    pub source: &'static str,
}

/// Where the factor's *base* value comes from: a named function, or a constant
/// for the five event modifiers.
///
/// Java splits this in two classes — `DynamicHappinessSupplier` holding a
/// registry key and `StaticHappinessSupplier` holding a double
/// (`DynamicHappinessSupplier.java:177-239`, `StaticHappinessSupplier.java:250-299`).
/// The port closes the dynamic half into a match, which is what makes a factor
/// testable without a registry.
#[derive(Clone, Copy, Debug, PartialEq)]
// exhaustive: the ten registered functions plus the constant supplier
pub enum Supplier {
    /// `minecolonies:housing` (`ModHappinessFactorTypeInitializer.java:44`).
    Housing,
    /// `minecolonies:unemployment` (`:45`).
    Unemployment,
    /// `minecolonies:health` (`:46-47`).
    Health,
    /// `minecolonies:idleatjob` (`:48`).
    IdleAtJob,
    /// `minecolonies:slepttonight` (`:50`).
    SleptTonight,
    /// `minecolonies:food` (`:51`).
    Food,
    /// `minecolonies:security` (`:40`).
    Security,
    /// `minecolonies:social` (`:41`).
    Social,
    /// `minecolonies:mystical` (`:42`).
    MysticalSite,
    /// `minecolonies:school` (`:39`).
    School,
    /// A fixed number: what `StaticHappinessSupplier` carries for the five
    /// event modifiers, whose value never varies with the citizen.
    Fixed(f64),
}

impl Supplier {
    /// The base factor, before any ramp or expiry.
    pub fn value(self, facts: &Facts) -> f64 {
        match self {
            Supplier::Housing => factors::housing(&facts.citizen),
            Supplier::Unemployment => factors::unemployment(&facts.citizen),
            Supplier::Health => factors::health(&facts.citizen),
            Supplier::IdleAtJob => factors::idle_at_job(&facts.citizen),
            Supplier::SleptTonight => factors::slept_tonight(&facts.citizen),
            Supplier::Food => factors::food(&facts.citizen),
            Supplier::Security => factors::security(&facts.colony),
            Supplier::Social => factors::social(&facts.colony),
            Supplier::MysticalSite => factors::mystical_site(&facts.colony),
            Supplier::School => factors::school(&facts.citizen),
            Supplier::Fixed(value) => value,
        }
    }
}

/// One modifier, as `AbstractHappinessModifier` plus the two subclasses' extra
/// fields, in a single shape.
#[derive(Clone, Copy, Debug, PartialEq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Modifier {
    src: Source,
    weight: f64,
    supplier: Supplier,
    kind: Kind,
    days: i32,
    period: i32,
    ramp: &'static [Step],
    /// `(modifier, data) -> true` instead of the default
    /// `modifier.getFactor(data) < 1` (`TimeBasedHappinessModifier.java:47`).
    always_rolls_over: bool,
    /// `fichier.java:ligne` that builds this modifier, with its weight and ramp.
    source: &'static str,
}

impl Modifier {
    /// Nine fields, one constructor, in the order Java's three classes take
    /// them: `AbstractHappinessModifier`'s four, `TimeBased`'s two, and this
    /// file's own source citation. Written as one call rather than a builder
    /// because the ten of [`Self::defaults`] are a table, and a table reads
    /// better as rows.
    #[allow(clippy::too_many_arguments)]
    fn new(
        src: Source,
        weight: f64,
        supplier: Supplier,
        kind: Kind,
        days: i32,
        period: i32,
        ramp: &'static [Step],
        always_rolls_over: bool,
        source: &'static str,
    ) -> Modifier {
        Modifier {
            src,
            weight,
            supplier,
            kind,
            days,
            period,
            ramp,
            always_rolls_over,
            source,
        }
    }

    /// The ten of `CitizenHappinessHandler.java:53-80`, in registration order.
    pub fn defaults() -> [Modifier; 10] {
        [
            Modifier::new(
                Source::School,
                1.0,
                Supplier::School,
                Kind::Static,
                0,
                0,
                &[],
                false,
                "CitizenHappinessHandler.java:53",
            ),
            Modifier::new(
                Source::Security,
                4.0,
                Supplier::Security,
                Kind::Static,
                0,
                0,
                &[],
                false,
                "CitizenHappinessHandler.java:54",
            ),
            Modifier::new(
                Source::Social,
                2.0,
                Supplier::Social,
                Kind::Static,
                0,
                0,
                &[],
                false,
                "CitizenHappinessHandler.java:55",
            ),
            Modifier::new(
                Source::MysticalSite,
                1.0,
                Supplier::MysticalSite,
                Kind::Static,
                0,
                0,
                &[],
                false,
                "CitizenHappinessHandler.java:56",
            ),
            Modifier::new(
                Source::Food,
                3.0,
                Supplier::Food,
                Kind::Static,
                0,
                0,
                &[],
                false,
                "CitizenHappinessHandler.java:57",
            ),
            Modifier::new(
                Source::Homelessness,
                3.0,
                Supplier::Housing,
                Kind::Time,
                0,
                0,
                &HOMELESSNESS_RAMP,
                false,
                "CitizenHappinessHandler.java:60-63",
            ),
            Modifier::new(
                Source::Unemployment,
                2.0,
                Supplier::Unemployment,
                Kind::Time,
                0,
                0,
                &UNEMPLOYMENT_RAMP,
                false,
                "CitizenHappinessHandler.java:65-68",
            ),
            Modifier::new(
                Source::Health,
                2.0,
                Supplier::Health,
                Kind::Time,
                0,
                0,
                &HEALTH_RAMP,
                false,
                "CitizenHappinessHandler.java:70-73",
            ),
            Modifier::new(
                Source::IdleAtJob,
                1.0,
                Supplier::IdleAtJob,
                Kind::Time,
                0,
                0,
                &IDLE_AT_JOB_RAMP,
                false,
                "CitizenHappinessHandler.java:75-78",
            ),
            Modifier::slept_tonight(),
        ]
    }

    /// `SLEPTTONIGHT`: weight `1.5`, base `0.5`, and a ramp that starts on day
    /// zero (`CitizenHappinessHandler.java:80`).
    pub fn slept_tonight() -> Modifier {
        Modifier::new(
            Source::SleptTonight,
            1.5,
            Supplier::SleptTonight,
            Kind::Time,
            0,
            0,
            &SLEPT_TONIGHT_RAMP,
            true,
            "CitizenHappinessHandler.java:80",
        )
    }

    /// `DAMAGE`: weight `2`, factor `0`, **one** day, added on every wound
    /// (`EntityCitizen.java:1428`).
    pub fn damage() -> Modifier {
        Modifier::new(
            Source::Damage,
            2.0,
            Supplier::Fixed(0.0),
            Kind::Expiration,
            1,
            1,
            &[],
            false,
            "EntityCitizen.java:1428",
        )
    }

    /// `DEATH`: weight `3`, factor `0`, **three** days, injected into the whole
    /// colony when a non-guard citizen dies (`EntityCitizen.java:1558-1561`).
    pub fn death() -> Modifier {
        Modifier::new(
            Source::Death,
            3.0,
            Supplier::Fixed(0.0),
            Kind::Expiration,
            3,
            3,
            &[],
            false,
            "EntityCitizen.java:1560",
        )
    }

    /// `RAIDWITHOUTDEATH`: weight `1`, factor **`2.0`**, three days, injected
    /// when a raid ends with nobody lost (`RaidManager.java:1078-1081`).
    ///
    /// The one event modifier that is a reward, and it clears the same way: after
    /// three nights it expires back to `1.0`.
    pub fn raid_without_death() -> Modifier {
        Modifier::new(
            Source::RaidWithoutDeath,
            1.0,
            Supplier::Fixed(2.0),
            Kind::Expiration,
            3,
            3,
            &[],
            false,
            "RaidManager.java:1080",
        )
    }

    /// `HADGREATFOOD`: weight `2`, factor `2.0`, five days, added when a citizen
    /// eats a tier-3 MineColonies dish (`ItemStackUtils.java:966-968`).
    pub fn great_food() -> Modifier {
        Modifier::new(
            Source::GreatFood,
            2.0,
            Supplier::Fixed(2.0),
            Kind::Expiration,
            5,
            5,
            &[],
            false,
            "ItemStackUtils.java:968",
        )
    }

    /// `QUEST`: weight `2`, factor and period straight from the reward's JSON
    /// (`HappinessRewardTemplate.java:55-70`).
    ///
    /// `amount` is what the reward says and is **not** clamped by Java, so a
    /// quest file can ask for `0.0` or `3.0`.
    pub fn quest(amount: f64, days: i32) -> Modifier {
        Modifier::new(
            Source::Quest,
            2.0,
            Supplier::Fixed(amount),
            Kind::Expiration,
            days,
            days,
            &[],
            false,
            "HappinessRewardTemplate.java:65",
        )
    }

    /// The id this modifier is stored and looked up under.
    pub fn source_of(&self) -> Source {
        self.src
    }

    /// `IHappinessModifier.getWeight` (`:31`).
    pub fn weight(&self) -> f64 {
        self.weight
    }

    /// Which of the three shapes this is.
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// `ITimeBasedHappinessModifier.getDays` (`:196-203`)
    /// `AbstractHappinessModifier.getFactor` (`:90-94`) with the two subclasses'
    /// overrides: the ramp for [`Kind::Time`] (`:81-97`), the expiry for
    /// [`Kind::Expiration`] (`:209-217`).
    pub fn factor(&self, facts: &Facts) -> f64 {
        let base = self.supplier.value(facts);
        match self.kind {
            Kind::Static => base,
            Kind::Time => {
                // `if (baseFactor < 1.0)`: written as a negation so `NaN` takes
                // the same branch Java's comparison gives it — straight out.
                if !(base < 1.0) {
                    return base;
                }
                let mut factor = base;
                for step in self.ramp {
                    if self.days >= step.days {
                        factor = base * step.multiplier;
                    }
                }
                factor
            }
            Kind::Expiration => {
                if self.days > 0 && self.days <= self.period {
                    base
                } else {
                    1.0
                }
            }
        }
    }

    /// The day counter: how long this has been in force, or how long it has left.
    pub fn days(&self) -> i32 {
        self.days
    }

    /// `ExpirationBasedHappinessModifier`'s `period` (`:184`): `0` for the other
    /// two shapes.
    pub fn period(&self) -> i32 {
        self.period
    }

    /// The ramp, for a caller that wants to show "seven days without a bed" the
    /// way the town hall does.
    pub fn ramp(&self) -> &'static [Step] {
        self.ramp
    }

    /// `fichier.java:ligne` that builds this modifier.
    pub fn source(&self) -> &'static str {
        self.source
    }

    /// `ITimeBasedHappinessModifier.reset` — zero days for a ramp, the full
    /// period for an expiry (`:99-103`, `:219-223`).
    pub fn reset(&mut self) {
        self.days = match self.kind {
            Kind::Static => self.days,
            Kind::Time => 0,
            Kind::Expiration => self.period,
        };
    }

    /// One night (`ITimeBasedHappinessModifier.dayEnd`, `:111-122`).
    ///
    /// The default predicate is `getFactor(data) < 1`, evaluated **before** the
    /// counter moves, so the first night of a decay still counts as day zero.
    pub fn day_end(&mut self, facts: &Facts) {
        match self.kind {
            Kind::Static => {}
            Kind::Time => {
                if self.always_rolls_over || self.factor(facts) < 1.0 {
                    self.days += 1;
                } else {
                    self.days = 0;
                }
            }
            Kind::Expiration => {
                if self.days > 0 {
                    self.days -= 1;
                }
            }
        }
    }
}

/// `HOMELESSNESS`: seven days to complain, fourteen to demand
/// (`HappinessConstants.java:15-16`, read at `CitizenHappinessHandler.java:63`).
pub static HOMELESSNESS_RAMP: [Step; 2] = [
    Step {
        days: 7,
        multiplier: 0.75,
        source: "CitizenHappinessHandler.java:63",
    },
    Step {
        days: 14,
        multiplier: 0.5,
        source: "CitizenHappinessHandler.java:63",
    },
];

/// `UNEMPLOYMENT`: same two thresholds (`HappinessConstants.java:24-25`, read at
/// `CitizenHappinessHandler.java:68`).
pub static UNEMPLOYMENT_RAMP: [Step; 2] = [
    Step {
        days: 7,
        multiplier: 0.75,
        source: "CitizenHappinessHandler.java:68",
    },
    Step {
        days: 14,
        multiplier: 0.5,
        source: "CitizenHappinessHandler.java:68",
    },
];

/// `HEALTH`: it hurts faster than homelessness — half after a week, a tenth
/// after two (`HappinessConstants.java:18-19`, read at
/// `CitizenHappinessHandler.java:73`).
pub static HEALTH_RAMP: [Step; 2] = [
    Step {
        days: 7,
        multiplier: 0.5,
        source: "CitizenHappinessHandler.java:73",
    },
    Step {
        days: 14,
        multiplier: 0.1,
        source: "CitizenHappinessHandler.java:73",
    },
];

/// `IDLEATJOB`: the health ramp, on a smaller weight
/// (`HappinessConstants.java:35-36`, read at `CitizenHappinessHandler.java:78`).
pub static IDLE_AT_JOB_RAMP: [Step; 2] = [
    Step {
        days: 7,
        multiplier: 0.5,
        source: "CitizenHappinessHandler.java:78",
    },
    Step {
        days: 14,
        multiplier: 0.1,
        source: "CitizenHappinessHandler.java:78",
    },
];

/// `SLEPTTONIGHT`: the only ramp that starts on day **zero**, so a base of `0.5`
/// reads as a neutral `1.0` on the night it slept
/// (`CitizenHappinessHandler.java:80`).
pub static SLEPT_TONIGHT_RAMP: [Step; 3] = [
    Step {
        days: 0,
        multiplier: 2.0,
        source: "CitizenHappinessHandler.java:80",
    },
    Step {
        days: 2,
        multiplier: 1.6,
        source: "CitizenHappinessHandler.java:80",
    },
    Step {
        days: 3,
        multiplier: 1.0,
        source: "CitizenHappinessHandler.java:80",
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::citizens::happiness::factors::{CitizenFacts, ColonyFacts};

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    /// The same check with a sentence attached, for a boundary whose *value* is
    /// not the interesting part but whose reason is.
    fn close_at(a: f64, b: f64, what: &str) {
        assert!((a - b).abs() < 1e-9, "{what}: {a} != {b}");
    }

    /// A citizen with a home and a hut, in a colony of ten with everything fine —
    /// the baseline the ramps are read against.
    fn plain() -> Facts {
        Facts::new(
            CitizenFacts::new().housed(3).at(crate::jobs::Job::Farmer, 3),
            ColonyFacts::default(),
        )
    }

    fn find(modifiers: &[Modifier], src: Source) -> Modifier {
        *modifiers
            .iter()
            .find(|m| m.source_of() == src)
            .expect("the ten of :53-80 hold it")
    }

    #[test]
    fn the_fifteen_sources_carry_the_java_strings() {
        assert_eq!(Source::ALL.len(), 15, "HappinessConstants.java:63-77");
        let mut names: Vec<&str> = Source::ALL.iter().map(|s| s.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 15, "two sources share a name");
        for (src, name) in [
            (Source::IdleAtJob, "idleatjob"),
            (Source::RaidWithoutDeath, "raidwithoutdeath"),
            (Source::GreatFood, "greatfood"),
            (Source::MysticalSite, "mysticalsite"),
        ] {
            assert_eq!(src.name(), name);
        }
    }

    #[test]
    fn ten_are_present_at_birth_and_the_five_event_ones_are_not() {
        assert_eq!(Source::AT_BIRTH.len(), 10, "CitizenHappinessHandler.java:53-80");
        for src in Source::ALL {
            let expected = !matches!(
                src,
                Source::Damage
                    | Source::Death
                    | Source::RaidWithoutDeath
                    | Source::Quest
                    | Source::GreatFood
            );
            assert_eq!(src.at_birth(), expected, "{}", src.name());
        }
    }

    #[test]
    fn a_name_round_trips_and_an_unknown_one_is_dropped() {
        for src in Source::ALL {
            assert_eq!(Source::from_name(src.name()), Some(src));
        }
        assert_eq!(Source::from_name("homelessnes"), None, "one s, no match");
        assert_eq!(Source::from_name(""), None);
        assert_eq!(Source::from_name("Homelessness"), None, "the ids are lower case");
    }

    #[test]
    fn the_ten_weights_are_the_javas_and_sum_to_twenty_and_a_half() {
        let modifiers = Modifier::defaults();
        assert_eq!(modifiers.len(), 10);
        for (src, weight) in [
            (Source::School, 1.0),
            (Source::Security, 4.0),
            (Source::Social, 2.0),
            (Source::MysticalSite, 1.0),
            (Source::Food, 3.0),
            (Source::Homelessness, 3.0),
            (Source::Unemployment, 2.0),
            (Source::Health, 2.0),
            (Source::IdleAtJob, 1.0),
            (Source::SleptTonight, 1.5),
        ] {
            close(find(&modifiers, src).weight(), weight);
        }
        let sum: f64 = modifiers.iter().map(|m| m.weight()).sum();
        close(sum, 20.5);
        // Registration order, not the enum's: five static, then five time-based.
        let order: Vec<Source> = modifiers.iter().map(|m| m.source_of()).collect();
        assert_eq!(order, Source::AT_BIRTH.to_vec());
    }

    #[test]
    fn every_modifier_cites_the_line_that_builds_it() {
        for modifier in Modifier::defaults() {
            assert!(
                modifier.source().contains(".java:"),
                "{}",
                modifier.source_of().name()
            );
        }
        for (modifier, expected) in [
            (Modifier::damage(), "EntityCitizen.java:1428"),
            (Modifier::death(), "EntityCitizen.java:1560"),
            (Modifier::great_food(), "ItemStackUtils.java:968"),
            (Modifier::raid_without_death(), "RaidManager.java:1080"),
        ] {
            assert_eq!(modifier.source(), expected);
            assert_eq!(modifier.kind(), Kind::Expiration);
        }
        // And every ramp step cites its own line, so a number can be checked
        // without reading this file's doc comment.
        for ramp in [
            &HOMELESSNESS_RAMP[..],
            &UNEMPLOYMENT_RAMP[..],
            &HEALTH_RAMP[..],
            &IDLE_AT_JOB_RAMP[..],
            &SLEPT_TONIGHT_RAMP[..],
        ] {
            for step in ramp {
                assert!(step.source.contains(".java:"), "{}", step.source);
            }
        }
    }

    #[test]
    fn a_static_modifier_never_moves_however_many_nights_pass() {
        let facts = plain();
        let mut modifier = find(&Modifier::defaults(), Source::Security);
        let before = modifier.factor(&facts);
        for _ in 0..100 {
            modifier.day_end(&facts);
        }
        close(modifier.factor(&facts), before);
        assert_eq!(modifier.days(), 0, "Kind::Static has no counter");
        modifier.reset();
        assert_eq!(modifier.days(), 0);
    }

    #[test]
    fn a_neutral_time_factor_resets_the_counter_instead_of_climbing() {
        let facts = plain();
        let mut homeless = find(&Modifier::defaults(), Source::Homelessness);
        // Housed at level 3: 3/3 = 1.0, not below one, so the ramp is never
        // entered and the counter is put back to zero every night.
        close(homeless.factor(&facts), 1.0);
        for _ in 0..20 {
            homeless.day_end(&facts);
            assert_eq!(homeless.days(), 0);
        }
    }

    #[test]
    fn a_decaying_time_factor_climbs_by_one_a_night_and_the_last_step_wins() {
        let facts = Facts::new(
            CitizenFacts::new().at(crate::jobs::Job::Farmer, 3),
            ColonyFacts::default(),
        );
        let mut homeless = find(&Modifier::defaults(), Source::Homelessness);
        close(homeless.factor(&facts), 0.0);
        for night in 1..=6 {
            homeless.day_end(&facts);
            assert_eq!(homeless.days(), night);
            close(homeless.factor(&facts), 0.0);
        }
        homeless.day_end(&facts);
        assert_eq!(homeless.days(), 7);
        close_at(homeless.factor(&facts), 0.0, "0 · 0.75 is still 0");
        for _ in 0..7 {
            homeless.day_end(&facts);
        }
        assert_eq!(homeless.days(), 14);
        close(homeless.factor(&facts), 0.0);
        // A home at level 1 decays the visible way: 1/3 → 0.25 → 1/6.
        let cost = Facts::new(
            CitizenFacts::new().housed(1).at(crate::jobs::Job::Farmer, 3),
            ColonyFacts::default(),
        );
        let mut housed = find(&Modifier::defaults(), Source::Homelessness);
        close(housed.factor(&cost), 1.0 / 3.0);
        for _ in 0..7 {
            housed.day_end(&cost);
        }
        close(housed.factor(&cost), 0.25);
        for _ in 0..7 {
            housed.day_end(&cost);
        }
        close(housed.factor(&cost), 1.0 / 6.0);
    }

    #[test]
    fn health_decays_faster_than_homelessness_on_the_same_seven_days() {
        let ill = Facts::new(
            CitizenFacts {
                sick: true,
                ..CitizenFacts::new().housed(3).at(crate::jobs::Job::Farmer, 3)
            },
            ColonyFacts::default(),
        );
        let mut health = find(&Modifier::defaults(), Source::Health);
        close(health.factor(&ill), 0.5);
        for _ in 0..7 {
            health.day_end(&ill);
        }
        close(health.factor(&ill), 0.25);
        for _ in 0..7 {
            health.day_end(&ill);
        }
        close(health.factor(&ill), 0.05);
        // Cured: the factor is 1.0, the ramp is not entered, the counter is zero.
        let cured = Facts::new(
            CitizenFacts::new().housed(3).at(crate::jobs::Job::Farmer, 3),
            ColonyFacts::default(),
        );
        health.day_end(&cured);
        assert_eq!(health.days(), 0);
        close(health.factor(&cured), 1.0);
    }

    #[test]
    fn slept_tonight_rolls_over_whatever_its_factor_says_and_starts_neutral() {
        let grown = plain();
        let mut slept = Modifier::slept_tonight();
        close_at(slept.factor(&grown), 1.0, "0.5 × 2.0 on day zero");
        slept.day_end(&grown);
        assert_eq!(slept.days(), 1, "the predicate always answers true");
        close_at(slept.factor(&grown), 1.0, "still 0.5 × 2.0");
        slept.day_end(&grown);
        close_at(slept.factor(&grown), 0.8, "0.5 × 1.6");
        slept.day_end(&grown);
        close_at(slept.factor(&grown), 0.5, "0.5 × 1.0");
        slept.day_end(&grown);
        close_at(slept.factor(&grown), 0.5, "and there it stays");
        // Sleeping puts it back to day zero.
        slept.reset();
        assert_eq!(slept.days(), 0);
        close(slept.factor(&grown), 1.0);
        // A guard is 1.0 at the supplier, so the ramp never runs at all.
        let guard = Facts::new(
            CitizenFacts::new()
                .housed(3)
                .at(crate::jobs::Job::Knight, 1),
            ColonyFacts::default(),
        );
        let mut guarded = Modifier::slept_tonight();
        for _ in 0..5 {
            guarded.day_end(&guard);
        }
        close(guarded.factor(&guard), 1.0);
    }

    #[test]
    fn an_expiry_starts_at_its_period_counts_down_and_then_reads_neutral() {
        let facts = plain();
        let mut damage = Modifier::damage();
        assert_eq!(damage.days(), 1);
        assert_eq!(damage.period(), 1);
        close(damage.factor(&facts), 0.0);
        damage.day_end(&facts);
        assert_eq!(damage.days(), 0);
        close_at(damage.factor(&facts), 1.0, "expired: the aggregate skips it");
        damage.day_end(&facts);
        assert_eq!(damage.days(), 0, "it floors at zero rather than going negative");
        // And resetting brings it all back — what re-wounding does, since
        // `addModifier` replaces by id (`CitizenHappinessHandler.java:92-96`).
        damage.reset();
        assert_eq!(damage.days(), 1);
        close(damage.factor(&facts), 0.0);

        let mut death = Modifier::death();
        assert_eq!(death.days(), 3);
        close(death.factor(&facts), 0.0);
        for remaining in [2, 1, 0] {
            death.day_end(&facts);
            assert_eq!(death.days(), remaining);
        }
        close(death.factor(&facts), 1.0);
    }

    #[test]
    fn the_reward_modifiers_are_above_neutral_and_expire_the_same_way() {
        let facts = plain();
        let raid = Modifier::raid_without_death();
        close(raid.factor(&facts), 2.0);
        close(raid.weight(), 1.0);
        let food = Modifier::great_food();
        close(food.factor(&facts), 2.0);
        close(food.weight(), 2.0);
        assert_eq!(food.period(), 5);
        let quest = Modifier::quest(2.5, 7);
        close(quest.factor(&facts), 2.5);
        assert_eq!(quest.period(), 7);
        assert_eq!(quest.days(), 7);
        assert_eq!(quest.source_of(), Source::Quest);
        // A quest that asks for nothing is a modifier at 0.0 — Java does not
        // clamp it, and neither does the port.
        close(Modifier::quest(0.0, 1).factor(&facts), 0.0);
    }

    #[test]
    fn the_three_kinds_keep_their_registry_paths() {
        assert_eq!(Kind::Static.name(), "static");
        assert_eq!(Kind::Time.name(), "time");
        assert_eq!(Kind::Expiration.name(), "expiration");
        // And each of the ten is one of the first two, never the third.
        for modifier in Modifier::defaults() {
            assert_ne!(modifier.kind(), Kind::Expiration);
            // The five static ones have no ramp at all.
            if modifier.kind() == Kind::Static {
                assert!(modifier.ramp().is_empty());
            } else {
                assert!(!modifier.ramp().is_empty());
            }
        }
    }
}
