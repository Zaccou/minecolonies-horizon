//! What a colony counts, day by day, and its compound.
//!
//! Java reference: `api/colony/managers/interfaces/IStatisticsManager.java`
//! (a map `id` → `day` → `quantity`) and `core/colony/managers/StatisticsManager.java`.
//! The shape is not the port's invention: every stat is kept **per colony day**,
//! and the total is a sum over days (`StatisticsManager.java:55-65`).
//!
//! **The day is a parameter, never a read.** Java passes
//! `colony.getDay()` at each call site (`EntityAIWorkCook.java:242`,
//! `ReproductionManager.java:217`); a wasm mod has no clock at all, so
//! [`Stats::increment`] takes the day and gets it exactly as wrong as its
//! caller. A colony's day is the number of days since it was founded — that is
//! `crate::colony::Colony`'s business, not this file's.
//!
//! **What the brief's four are called in Java.** Recruited colonists are
//! `visitors_recruited`, incremented when a visitor is hired off the tavern
//! (`RecruitmentInteraction.java:181`) — `birth` is a colonist *born*
//! (`ReproductionManager.java:217`), a different event. Buildings posed are
//! `build_built` (`AbstractEntityAIStructureWithWorkOrder.java:403-410`, which
//! counts an upgrade as `build_upgraded`). Deaths are `death`
//! (`EntityCitizen.java:1569`). Food served is `food_served`
//! (`EntityAIWorkCook.java:242`, `:297`). All four names are constants here.
//!
//! **Tolerant reads.** A stat written as `Byte` or `Short` reads as the same
//! number, like `crate::colony::nbt` does for the colony record.

use std::collections::BTreeMap;

use mod_api::{Compound, Tag};

/// `StatisticsConstants.java:10`.
pub const BIRTH: &str = "birth";
/// `StatisticsConstants.java:9`.
pub const DEATH: &str = "death";
/// `StatisticsConstants.java:22` — a hut finished its build request.
pub const BUILD_BUILT: &str = "build_built";
/// `StatisticsConstants.java:23` — a hut went up a level.
pub const BUILD_UPGRADED: &str = "build_upgraded";
/// `StatisticsConstants.java:17` — what the cook served.
pub const FOOD_SERVED: &str = "food_served";
/// `StatisticsConstants.java:15`.
pub const ITEMS_DELIVERED: &str = "items_delivered";
/// `StatisticsConstants.java:124` — a visitor was hired.
pub const VISITORS_RECRUITED: &str = "visitors_recruited";
/// `StatisticsConstants.java:125` — a visitor left before being hired.
pub const VISITORS_ABSCONDED: &str = "visitors_absconded";

/// The list of stat records (`StatisticsManager.java:27`).
pub const KEY_STATS: &str = "stat_manager";
/// One record's id (`NbtTagConstants.java:8`).
pub const KEY_ID: &str = "id";
/// One record's day list (`StatisticsManager.java:28`).
pub const KEY_STAT: &str = "stat";
/// One day's number (`NbtTagConstants.java:521`).
pub const KEY_DAY: &str = "time";
/// One day's count (`NbtTagConstants.java:588`).
pub const KEY_QUANTITY: &str = "quantity";

/// The colony's counters: for each stat id, the quantity per day.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Stats {
    days: BTreeMap<String, BTreeMap<i32, i32>>,
}

impl Stats {
    pub fn new() -> Stats {
        Stats::default()
    }

    /// One more of `id` on `day` (`StatisticsManager.java:41-44`).
    pub fn increment(&mut self, id: &str, day: i32) -> Result<(), Refusal> {
        self.increment_by(id, 1, day)
    }

    /// `qty` more of `id` on `day`. Two increments on the same day add up:
    /// Java's `innerMap.addTo(day, qty)` adds to what the day holds
    /// (`StatisticsManager.java:47-51`).
    ///
    /// Divergence, on purpose: Java accepts a zero quantity and quietly creates
    /// the id with a zero entry, so `getStatTypes()` grows a counter nobody
    /// incremented. The port refuses, like `crate::inventory` refuses an empty
    /// stack.
    pub fn increment_by(&mut self, id: &str, qty: i32, day: i32) -> Result<(), Refusal> {
        if id.is_empty() {
            return Err(Refusal::EmptyId);
        }
        if qty == 0 {
            return Err(Refusal::EmptyQuantity);
        }
        self.days
            .entry(id.to_owned())
            .or_default()
            .entry(day)
            .and_modify(|count| *count += qty)
            .or_insert(qty);
        Ok(())
    }

    /// The sum over every day (`StatisticsManager.java:55-65`). A stat never
    /// incremented is `0`, not an error: Java returns 0 from an empty map.
    pub fn total(&self, id: &str) -> i32 {
        self.days
            .get(id)
            .map(|days| days.values().sum())
            .unwrap_or(0)
    }

    /// The sum over `from..=to` (`StatisticsManager.java:67-77`). The bounds are
    /// inclusive, and `from > to` is an empty period.
    pub fn in_period(&self, id: &str, from: i32, to: i32) -> i32 {
        if from > to {
            return 0;
        }
        self.days
            .get(id)
            .map(|days| days.range(from..=to).map(|(_, count)| *count).sum())
            .unwrap_or(0)
    }

    /// The highest day this stat was touched on, which is what Java's partial
    /// sync sends (`dataEntry.lastIntKey()`, `StatisticsManager.java:139`).
    ///
    /// Java reads *insertion* order there; a `BTreeMap` has none, so the port
    /// answers with the highest day. The two agree as long as the day counter
    /// only grows, which is the only way a colony day is ever written.
    pub fn latest_day(&self, id: &str) -> Option<i32> {
        self.days.get(id)?.keys().next_back().copied()
    }

    /// Each stat id that has at least one day (`StatisticsManager.java:79-82`).
    pub fn ids(&self) -> impl Iterator<Item = &str> + '_ {
        self.days.keys().map(String::as_str)
    }

    /// Forgets everything (`StatisticsManager.java:98-103`).
    pub fn clear(&mut self) {
        self.days.clear();
    }

    /// Adds `other`'s counts day by day, which is Java's `aggregateStats`
    /// (`IStatisticsManager.java:105-120`). Used when one colony's record is
    /// folded into another's. A day of zero is dropped by the same refusal
    /// [`Stats::increment_by`] gives: it counts nothing.
    pub fn absorb(&mut self, other: &Stats) {
        for (id, days) in &other.days {
            for (day, count) in days {
                let _ = self.increment_by(id, *count, *day);
            }
        }
    }

    /// The compound Java writes (`StatisticsManager.java:181-205`):
    /// `stat_manager` is a list of `{id, stat: [{time, quantity}]}`.
    pub fn to_compound(&self) -> Compound {
        let mut records = Vec::new();
        for (id, days) in &self.days {
            let stamps: Vec<Tag> = days
                .iter()
                .map(|(day, count)| {
                    Tag::compound([
                        (KEY_DAY, Tag::Int(*day)),
                        (KEY_QUANTITY, Tag::Int(*count)),
                    ])
                })
                .collect();
            records.push(Tag::compound([
                (KEY_ID, Tag::String(id.as_str().into())),
                (KEY_STAT, Tag::List(stamps)),
            ]));
        }
        Compound::from([(KEY_STATS.into(), Tag::List(records))])
    }

    /// The inverse (`StatisticsManager.java:208-233`). A record without an id
    /// or without a day is dropped rather than stored under a made-up key.
    pub fn from_compound(data: &Compound) -> Stats {
        let mut stats = Stats::new();
        let Some(Tag::List(records)) = data.get(&KEY_STATS.into()) else {
            return stats;
        };
        for record in records {
            let Tag::Compound(fields) = record else {
                continue;
            };
            let Some(id) = text(fields, KEY_ID) else {
                continue;
            };
            let mut days = BTreeMap::new();
            if let Some(Tag::List(stamps)) = fields.get(&KEY_STAT.into()) {
                for stamp in stamps {
                    let Tag::Compound(pair) = stamp else {
                        continue;
                    };
                    if let (Some(day), Some(count)) =
                        (number(pair, KEY_DAY), number(pair, KEY_QUANTITY))
                    {
                        days.insert(day, count);
                    }
                }
            }
            stats.days.insert(id, days);
        }
        stats
    }
}

impl Refusal {
    /// A stable name for logs and tests.
    pub fn name(self) -> &'static str {
        match self {
            Self::EmptyId => "stats.refusal.empty_id",
            Self::EmptyQuantity => "stats.refusal.empty_quantity",
        }
    }
}

/// Why a count was refused. Both are caller mistakes, not colony states.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// No stat id at all: a counter with no name is unreadable.
    EmptyId,
    /// A zero quantity: it would create a counter that counts nothing.
    EmptyQuantity,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

/// A string under `key`, the same tolerance as `crate::colony::nbt::text`
/// (which is private to that module, so the two lines are repeated).
fn text(data: &Compound, key: &str) -> Option<String> {
    match data.get(&key.into())? {
        Tag::String(value) => Some(value.to_string_lossy()),
        _ => None,
    }
}

/// An integer in whichever width it was saved.
fn number(data: &Compound, key: &str) -> Option<i32> {
    match data.get(&key.into())? {
        Tag::Int(value) => Some(*value),
        Tag::Byte(value) => Some(i32::from(*value)),
        Tag::Short(value) => Some(i32::from(*value)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stat name this test only uses to prove `absorb` carries unknown ids.
    const CROPS: &str = "crops_harvested";

    #[test]
    fn increments_land_on_their_day_and_the_total_adds_the_days() {
        let mut stats = Stats::new();
        stats.increment(DEATH, 3).expect("a death");
        stats.increment(DEATH, 3).expect("another");
        stats.increment(DEATH, 4).expect("a third");
        assert_eq!(stats.total(DEATH), 3);
        assert_eq!(stats.in_period(DEATH, 3, 3), 2);
        assert_eq!(stats.in_period(DEATH, 3, 4), 3);
        assert_eq!(stats.in_period(DEATH, 4, 3), 0);
        assert_eq!(stats.in_period(DEATH, 0, 2), 0);
        assert_eq!(stats.latest_day(DEATH), Some(4));
        assert_eq!(stats.total(BUILD_BUILT), 0);
        assert_eq!(stats.latest_day(BUILD_BUILT), None);
    }

    #[test]
    fn the_four_stats_the_brief_names_all_start_at_zero() {
        let stats = Stats::new();
        for id in [VISITORS_RECRUITED, BUILD_BUILT, DEATH, FOOD_SERVED] {
            assert_eq!(stats.total(id), 0);
            assert_eq!(stats.latest_day(id), None);
        }
        assert_eq!(stats.ids().count(), 0);
    }

    #[test]
    fn a_zero_quantity_and_an_empty_id_are_refused_and_create_nothing() {
        let mut stats = Stats::new();
        assert_eq!(
            stats.increment_by(DEATH, 0, 1),
            Err(Refusal::EmptyQuantity)
        );
        assert_eq!(stats.increment_by("", 2, 1), Err(Refusal::EmptyId));
        assert_eq!(stats.ids().count(), 0);
        assert_eq!(stats.total(DEATH), 0);
    }

    #[test]
    fn the_stats_survive_a_compound_and_come_back_identical() {
        let mut stats = Stats::new();
        stats.increment_by(FOOD_SERVED, 12, 3).expect("a meal");
        stats.increment_by(FOOD_SERVED, 4, 5).expect("another");
        stats.increment(BIRTH, 5).expect("a birth");
        stats.increment(BUILD_BUILT, 9).expect("a hut");
        let data = stats.to_compound();
        let read = Stats::from_compound(&data);
        assert_eq!(read, stats);
        assert_eq!(read.total(FOOD_SERVED), 16);
        // And back out again, which is the round trip a save really makes.
        assert_eq!(Stats::from_compound(&read.to_compound()), stats);
    }

    #[test]
    fn the_compound_holds_the_layout_java_writes() {
        let mut stats = Stats::new();
        stats.increment(DEATH, 3).expect("a death");
        let data = stats.to_compound();
        let Some(Tag::List(records)) = data.get(&KEY_STATS.into()) else {
            panic!("stat_manager is a list");
        };
        assert_eq!(records.len(), 1);
        let Tag::Compound(record) = &records[0] else {
            panic!("a record is a compound");
        };
        assert_eq!(text(record, KEY_ID), Some(DEATH.to_owned()));
        let Some(Tag::List(stamps)) = record.get(&KEY_STAT.into()) else {
            panic!("stat is a list");
        };
        let Tag::Compound(stamp) = &stamps[0] else {
            panic!("a stamp is a compound");
        };
        assert_eq!(number(stamp, KEY_DAY), Some(3));
        assert_eq!(number(stamp, KEY_QUANTITY), Some(1));
    }

    #[test]
    fn absorbing_another_colony_adds_day_by_day_and_forgets_nothing() {
        let mut mine = Stats::new();
        mine.increment_by(DEATH, 2, 1).expect("two deaths");
        let mut theirs = Stats::new();
        theirs.increment_by(DEATH, 3, 1).expect("three deaths");
        theirs.increment(DEATH, 7).expect("one more");
        theirs.increment(CROPS, 7).expect("a harvest");
        mine.absorb(&theirs);
        assert_eq!(mine.in_period(DEATH, 1, 1), 5);
        assert_eq!(mine.total(DEATH), 6);
        assert_eq!(mine.total(CROPS), 1);
        assert_eq!(mine.ids().count(), 2);
    }
}
