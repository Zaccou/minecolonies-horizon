//! Who is on the road, how far along he is, and what a recall sends him back to.
//!
//! Java reference: `core/colony/managers/TravellingManager.java` (222 lines, of
//! which `TravelerData` is the second half), `Colony.java:406` (the transition that
//! calls it) and `api/util/constant/ColonyConstants.java:51` (its interval).
//!
//! | Question | Answer | Java |
//! | --- | --- | --- |
//! | is this citizen travelling | an entry exists **and** its time is not zero | `:39-42`, `:197-200` |
//! | how much time is left | minus 20 per update, never below zero | `:158-165` |
//! | what does "percentage" mean | the share **remaining**, not the share travelled | `:172-175` |
//! | when is the colony saved | on a start, on a *real* finish, always on a recall | `:54`, `:63`, `:96` |
//! | where does a recall put him | his work building, else the town hall | `:74-81` |
//!
//! **Three of Java's four write rules are asymmetric, and all three are kept.**
//! `startTravellingTo` marks the colony dirty unconditionally (`:54`), even when it
//! replaces an identical entry; `finishTravellingFor` marks it **only** when the map
//! held the citizen (`:60-64`); `recallAllTravellingCitizens` marks it after
//! clearing, **even when the map was empty** (`:95-96`). [`Book`] carries a `dirty`
//! flag with exactly those three rules, and a test pins each.
//!
//! **`getTravelPercentage` does not answer what its name says.** `:174` returns
//! `remaining * 100 / initial`, which is **100 at departure and 0 on arrival** — the
//! share still to go. [`Traveller::remaining_percent`] keeps that number under the
//! name it deserves; a caller wanting the share travelled subtracts from 100.
//!
//! **A zero travel time is "arrived", not "travelling".** `hasReachedTarget` tests
//! `remaining == 0` (`:169`), so `startTravellingTo(id, pos, 0)` records a citizen
//! who is already there: [`Book::is_travelling`] answers `false` while
//! [`Book::target_for`] answers the position. Java's callers pass
//! `getDistance`-derived times, so this is an edge, and the port states it.
//!
//! **A negative travel time is refused, where Java hangs on it.** `:160` only
//! decrements while `remaining > 0`, so a negative time never moves and
//! `hasReachedTarget` never becomes true: the citizen is travelling for ever. The
//! port answers [`Refusal::NegativeTravelTime`] instead of reproducing a permanent
//! stuck state.
//!
//! **A corrupt record is refused by name, not read as zero.** Java reads a missing
//! `citizen` as `0` and a missing target as `NbtUtils.readBlockPos`'s zeroes
//! (`:216-219`), which files a traveller nobody can look up under the id 0 and sends
//! him to the origin. [`Book::from_compound`] refuses the list and says which record,
//! and the caller keeps the book it had — Java's `:126` clears first and loses the
//! previous one either way.

use std::collections::BTreeMap;

use mod_api::{Compound, Tag};

use super::nbt;

/// One update of the travel timer, in ticks (`ColonyConstants.java:51`): the
/// colony state machine calls `tickTravellers` this often (`Colony.java:406`).
pub const UPDATE_TRAVELING_INTERVAL: i32 = 20;

/// The list of travellers in the colony's compound
/// (`NbtTagConstants.java:762`: `travelerData`).
pub const KEY_TRAVELERS: &str = "travelerData";
/// One record's citizen id (`NbtTagConstants.java:69`).
pub const KEY_CITIZEN: &str = "citizen";
/// One record's destination (`NbtTagConstants.java:556`).
pub const KEY_TARGET: &str = "target";
/// One record's starting time (`NbtTagConstants.java:763`).
pub const KEY_INITIAL: &str = "initialTravelTime";
/// One record's time left (`NbtTagConstants.java:764`).
pub const KEY_REMAINING: &str = "remainingTravelTime";

/// Why a traveller was refused, named.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// A travel time below zero, which Java turns into a citizen who never arrives
    /// (`TravellingManager.java:160`, `:169`).
    NegativeTravelTime { citizen: i32, ticks: i32 },
    /// A record of the list is not a compound (`:130`). Java skips it silently; the
    /// port names it so a hand-edited save can be found.
    Malformed { at: usize },
    /// A record without a `citizen` id (`:216`).
    NoCitizen { at: usize },
    /// A record without a readable `target` (`:217`).
    NoTarget { at: usize },
    /// A record without a starting time (`:218`).
    NoInitialTime { at: usize },
    /// A record without a remaining time (`:219`).
    NoRemainingTime { at: usize },
    /// A record whose times are impossible: negative, or more left than started.
    ImpossibleTimes {
        at: usize,
        initial: i32,
        remaining: i32,
    },
}

impl Refusal {
    /// A stable name for logs and tests, namespaced like a registry entry.
    pub fn name(&self) -> &'static str {
        match self {
            Refusal::NegativeTravelTime { .. } => "minecolonies:travelling/negative-time",
            Refusal::Malformed { .. } => "minecolonies:travelling/malformed-record",
            Refusal::NoCitizen { .. } => "minecolonies:travelling/no-citizen",
            Refusal::NoTarget { .. } => "minecolonies:travelling/no-target",
            Refusal::NoInitialTime { .. } => "minecolonies:travelling/no-initial-time",
            Refusal::NoRemainingTime { .. } => "minecolonies:travelling/no-remaining-time",
            Refusal::ImpossibleTimes { .. } => "minecolonies:travelling/impossible-times",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

/// One party on the road: Java's `TravelerData` (`TravellingManager.java:138-221`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four fields Java serialises
pub struct Traveller {
    citizen: i32,
    target: [i32; 3],
    initial: i32,
    remaining: i32,
}

impl Traveller {
    /// `new TravelerData(citizenId, target, initialTravelTime)` (`:145-151`): the
    /// remaining time starts at the initial one.
    pub fn new(citizen: i32, target: [i32; 3], ticks: i32) -> Result<Traveller, Refusal> {
        if ticks < 0 {
            return Err(Refusal::NegativeTravelTime { citizen, ticks });
        }
        Ok(Traveller {
            citizen,
            target,
            initial: ticks,
            remaining: ticks,
        })
    }

    pub fn citizen(&self) -> i32 {
        self.citizen
    }

    pub fn target(&self) -> [i32; 3] {
        self.target
    }

    pub fn initial(&self) -> i32 {
        self.initial
    }

    pub fn remaining(&self) -> i32 {
        self.remaining
    }

    /// One update (`:158-165`): subtract the interval, and stop at zero rather than
    /// below it.
    pub fn on_tick(&mut self) {
        if self.remaining > 0 {
            self.remaining = (self.remaining - UPDATE_TRAVELING_INTERVAL).max(0);
        }
    }

    /// `hasReachedTarget` (`:167-170`): **exactly** zero, which is why a zero-time
    /// travel is over before it starts.
    pub fn has_arrived(&self) -> bool {
        self.remaining == 0
    }

    /// `isTraveling` (`:197-200`).
    pub fn is_travelling(&self) -> bool {
        !self.has_arrived()
    }

    /// The share **remaining**, `remaining * 100 / initial` (`:172-175`), or `None`
    /// when there is no initial time to divide by — Java answers `NaN` there.
    pub fn remaining_percent(&self) -> Option<f64> {
        if self.initial == 0 {
            return None;
        }
        Some(f64::from(self.remaining) * 100.0 / f64::from(self.initial))
    }

    /// `serializeNBT` (`:203-211`), tag for tag.
    pub fn to_compound(&self) -> Compound {
        Compound::from([
            (KEY_CITIZEN.into(), Tag::Int(self.citizen)),
            (KEY_TARGET.into(), nbt::java_position(self.target)),
            (KEY_INITIAL.into(), Tag::Int(self.initial)),
            (KEY_REMAINING.into(), Tag::Int(self.remaining)),
        ])
    }

    /// The inverse (`:214-220`), refusing a record it cannot read whole.
    pub fn from_compound(data: &Compound, at: usize) -> Result<Traveller, Refusal> {
        let citizen = nbt::int(data, KEY_CITIZEN).ok_or(Refusal::NoCitizen { at })?;
        let target = nbt::position_in(data, KEY_TARGET).ok_or(Refusal::NoTarget { at })?;
        let initial = nbt::int(data, KEY_INITIAL).ok_or(Refusal::NoInitialTime { at })?;
        let remaining =
            nbt::int(data, KEY_REMAINING).ok_or(Refusal::NoRemainingTime { at })?;
        if initial < 0 || remaining < 0 || remaining > initial {
            return Err(Refusal::ImpossibleTimes {
                at,
                initial,
                remaining,
            });
        }
        Ok(Traveller {
            citizen,
            target,
            initial,
            remaining,
        })
    }
}

/// Where a recall sends one traveller (`:72-93`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Recall {
    pub citizen: i32,
    /// His work building, else the town hall (`:74-81`).
    pub at: [i32; 3],
    /// He had no entity to teleport, so Java logs "returned very confused" and sets
    /// his respawn point instead (`:84-90`).
    pub entity_missing: bool,
}

/// The colony's travellers, keyed by citizen id (`:34`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Book {
    travellers: BTreeMap<i32, Traveller>,
    dirty: bool,
}

impl Book {
    pub fn new() -> Book {
        Book::default()
    }

    /// `isTravelling` (`:39-42`): an unknown citizen is not travelling, which is the
    /// answer for every citizen of a colony that never sent one away.
    pub fn is_travelling(&self, citizen: i32) -> bool {
        self.travellers
            .get(&citizen)
            .map(Traveller::is_travelling)
            .unwrap_or(false)
    }

    /// `getTravellingTargetFor` (`:45-48`).
    pub fn target_for(&self, citizen: i32) -> Option<[i32; 3]> {
        self.travellers.get(&citizen).map(Traveller::target)
    }

    pub fn get(&self, citizen: i32) -> Option<&Traveller> {
        self.travellers.get(&citizen)
    }

    /// `startTravellingTo` (`:51-55`): the entry is replaced, and the colony is
    /// marked dirty **even when the new one is identical**.
    pub fn start_to(
        &mut self,
        citizen: i32,
        target: [i32; 3],
        ticks: i32,
    ) -> Result<Option<Traveller>, Refusal> {
        let traveller = Traveller::new(citizen, target, ticks)?;
        let replaced = self.travellers.insert(citizen, traveller);
        self.dirty = true;
        Ok(replaced)
    }

    /// `finishTravellingFor` (`:58-65`): the citizen is forgotten, and the colony is
    /// marked dirty **only if he was there**.
    pub fn finish(&mut self, citizen: i32) -> bool {
        let removed = self.travellers.remove(&citizen).is_some();
        if removed {
            self.dirty = true;
        }
        removed
    }

    /// One colony update (`:99-103`), all travellers at once. Java's `onTick`
    /// answers `true` unconditionally, so the transition stays; there is nothing to
    /// answer here.
    pub fn on_tick(&mut self) {
        for traveller in self.travellers.values_mut() {
            traveller.on_tick();
        }
    }

    /// Who is still travelling, in citizen order — Java iterates its `HashMap`
    /// `keySet` (`:70`), whose order is unspecified; a `BTreeMap` pins it so a test
    /// can read it.
    pub fn travelling(&self) -> Vec<i32> {
        self.travellers
            .values()
            .filter(|traveller| traveller.is_travelling())
            .map(Traveller::citizen)
            .collect()
    }

    pub fn len(&self) -> usize {
        self.travellers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.travellers.is_empty()
    }

    /// `recallAllTravellingCitizens` (`:67-97`): where each one goes, then the book is
    /// emptied and the colony marked dirty — **even when nobody was travelling**,
    /// because Java's `:96` is outside its loop.
    ///
    /// `work` answers a citizen's work building position, which is a fact about the
    /// buildings; `town_hall` is where a citizen with no job goes back to.
    pub fn recall(
        &mut self,
        town_hall: [i32; 3],
        work: &dyn Fn(i32) -> Option<[i32; 3]>,
        entity_missing: &dyn Fn(i32) -> bool,
    ) -> Vec<Recall> {
        let recalls: Vec<Recall> = self
            .travellers
            .values()
            .map(|traveller| Recall {
                citizen: traveller.citizen,
                at: work(traveller.citizen).unwrap_or(town_hall),
                entity_missing: entity_missing(traveller.citizen),
            })
            .collect();
        self.travellers.clear();
        self.dirty = true;
        recalls
    }

    /// Has anything happened since the last save? Java's `colony.markDirty()`
    /// (`:54`, `:63`, `:96`).
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Takes the flag away, the way a save does.
    pub fn take_dirty(&mut self) -> bool {
        let was = self.dirty;
        self.dirty = false;
        was
    }

    /// `serializeNBT` (`:105-120`): one list under `travelerData`.
    pub fn to_compound(&self) -> Compound {
        let records: Vec<Compound> = self
            .travellers
            .values()
            .map(Traveller::to_compound)
            .collect();
        Compound::from([(KEY_TRAVELERS.into(), nbt::compound_list(records))])
    }

    /// The inverse (`:122-136`), refusing rather than reading zeroes.
    ///
    /// Java clears its map before reading and drops every item that is not a
    /// compound (`:126`, `:130`); the port builds a fresh book and refuses the whole
    /// list on the first record it cannot read, so a caller that assigns only on `Ok`
    /// keeps the book it had.
    pub fn from_compound(data: &Compound) -> Result<Book, Refusal> {
        let mut book = Book::new();
        for (at, item) in nbt::items(data, KEY_TRAVELERS).iter().enumerate() {
            let Tag::Compound(record) = item else {
                return Err(Refusal::Malformed { at });
            };
            let traveller = Traveller::from_compound(record, at)?;
            // `:133`: keyed by the record's own id, so a duplicate id keeps the last
            // record, exactly as a `HashMap.put` does.
            book.travellers.insert(traveller.citizen, traveller);
        }
        Ok(book)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book_with(citizen: i32, ticks: i32) -> Book {
        let mut book = Book::new();
        book.start_to(citizen, [10, 64, -20], ticks)
            .expect("a positive travel time");
        book.take_dirty();
        book
    }

    /// The interval is the constant `ColonyConstants.java:51` declares, and the five
    /// keys are the ones `NbtTagConstants` declares.
    #[test]
    fn the_interval_and_the_five_keys_are_java_s() {
        assert_eq!(UPDATE_TRAVELING_INTERVAL, 20);
        assert_eq!(KEY_TRAVELERS, "travelerData");
        assert_eq!(KEY_CITIZEN, "citizen");
        assert_eq!(KEY_TARGET, "target");
        assert_eq!(KEY_INITIAL, "initialTravelTime");
        assert_eq!(KEY_REMAINING, "remainingTravelTime");
    }

    /// An unknown citizen is not travelling and has no target (`:41-47`).
    #[test]
    fn an_unknown_citizen_is_not_travelling_and_has_no_target() {
        let book = Book::new();
        assert!(!book.is_travelling(4));
        assert_eq!(book.target_for(4), None);
        assert!(book.is_empty());
    }

    /// A start marks the colony dirty, records the target, and the citizen is
    /// travelling until the timer reaches zero (`:51-55`, `:169`).
    #[test]
    fn a_start_is_dirty_and_the_target_is_the_one_given() {
        let mut book = Book::new();
        let replaced = book.start_to(7, [1, 2, 3], 200).expect("a positive time");
        assert_eq!(replaced, None, "nothing was replaced");
        assert!(book.is_dirty());
        assert_eq!(book.target_for(7), Some([1, 2, 3]));
        assert!(book.is_travelling(7));
        assert_eq!(book.travelling(), vec![7]);
    }

    /// A second start on the same citizen replaces the record — and marks the colony
    /// dirty again even though the book already was (`:53`).
    #[test]
    fn a_second_start_replaces_the_record_and_is_dirty_again() {
        let mut book = book_with(7, 200);
        assert!(!book.is_dirty(), "the flag was taken");
        let replaced = book.start_to(7, [0, 70, 0], 40).expect("a positive time");
        assert_eq!(replaced.map(|traveller| traveller.target()), Some([10, 64, -20]));
        assert!(book.is_dirty());
        assert_eq!(book.len(), 1);
        assert_eq!(book.target_for(7), Some([0, 70, 0]));
    }

    /// Twenty ticks per update, stopping at zero rather than below it, and the
    /// citizen is then arrived (`:158-169`).
    #[test]
    fn the_timer_loses_twenty_per_update_and_stops_at_zero() {
        let mut book = book_with(7, 45);
        book.on_tick();
        assert_eq!(book.get(7).expect("recorded").remaining(), 25);
        book.on_tick();
        assert_eq!(book.get(7).expect("recorded").remaining(), 5);
        book.on_tick();
        assert_eq!(book.get(7).expect("recorded").remaining(), 0, "never below zero");
        assert!(book.get(7).expect("recorded").has_arrived());
        assert!(!book.is_travelling(7));
        assert!(book.travelling().is_empty());
        assert_eq!(
            book.target_for(7),
            Some([10, 64, -20]),
            "the record stays until it is finished (`:58-65`)"
        );
    }

    /// A zero travel time is over before it starts, while the target is still
    /// recorded (`:169`).
    #[test]
    fn a_zero_time_is_arrived_at_once() {
        let book = book_with(7, 0);
        assert!(!book.is_travelling(7));
        assert_eq!(book.target_for(7), Some([10, 64, -20]));
        assert_eq!(book.len(), 1);
    }

    /// A negative time is refused: Java would leave the citizen travelling for ever
    /// (`:160`, `:169`).
    #[test]
    fn a_negative_time_is_refused_and_writes_nothing() {
        let mut book = Book::new();
        assert_eq!(
            book.start_to(7, [0, 64, 0], -1),
            Err(Refusal::NegativeTravelTime {
                citizen: 7,
                ticks: -1
            })
        );
        assert!(book.is_empty());
        assert!(!book.is_dirty(), "a refused start does not mark the colony");
        assert_eq!(
            Refusal::NegativeTravelTime {
                citizen: 7,
                ticks: -1
            }
            .name(),
            "minecolonies:travelling/negative-time"
        );
    }

    /// A finish marks the colony **only** when the citizen was there (`:58-65`).
    #[test]
    fn a_finish_is_dirty_only_when_something_was_removed() {
        let mut book = book_with(7, 100);
        assert!(!book.finish(8), "nobody of that id");
        assert!(!book.is_dirty(), "so the colony is not marked");
        assert!(book.finish(7));
        assert!(book.is_dirty());
        assert!(book.is_empty());
    }

    /// A recall answers one destination per traveller, empties the book, and marks
    /// the colony **even when nobody was travelling** (`:95-96`).
    #[test]
    fn a_recall_names_where_each_one_goes_and_is_dirty_even_when_empty() {
        let mut book = Book::new();
        let town_hall = [0, 70, 0];
        let none = |_: i32| None;
        let present = |_: i32| false;
        let empty = book.recall(town_hall, &none, &present);
        assert!(empty.is_empty());
        assert!(book.is_dirty(), "Java marks outside the loop");

        let mut book = book_with(7, 100);
        book.start_to(8, [3, 64, 3], 100).expect("a positive time");
        let work = |citizen: i32| (citizen == 7).then_some([50, 70, 50]);
        let missing = |citizen: i32| citizen == 8;
        let recalls = book.recall(town_hall, &work, &missing);
        assert_eq!(
            recalls,
            vec![
                Recall {
                    citizen: 7,
                    at: [50, 70, 50],
                    entity_missing: false
                },
                Recall {
                    citizen: 8,
                    at: town_hall,
                    entity_missing: true
                },
            ],
            "the work building wins, the town hall is the fallback"
        );
        assert!(book.is_empty());
        assert!(book.is_dirty());
    }

    /// The percentage is the share **remaining**: 100 at departure, 0 on arrival,
    /// and no percentage at all when the initial time is zero (`:172-175`).
    #[test]
    fn the_percentage_is_the_share_remaining_not_the_share_travelled() {
        let mut book = book_with(7, 200);
        assert_eq!(book.get(7).expect("recorded").remaining_percent(), Some(100.0));
        book.on_tick();
        assert_eq!(book.get(7).expect("recorded").remaining_percent(), Some(90.0));
        while book.is_travelling(7) {
            book.on_tick();
        }
        assert_eq!(book.get(7).expect("recorded").remaining_percent(), Some(0.0));
        assert_eq!(book_with(7, 0).get(7).expect("recorded").remaining_percent(), None);
    }

    /// A book survives its compound and comes back identical, with Java's five keys.
    #[test]
    fn the_book_round_trips_through_the_compound_java_writes() {
        let mut book = book_with(7, 120);
        book.start_to(8, [-4, 12, 900], 0)
            .expect("a zero time is allowed");
        book.take_dirty();
        let data = book.to_compound();
        let Some(Tag::List(records)) = data.get(&KEY_TRAVELERS.into()) else {
            panic!("one list of records");
        };
        assert_eq!(records.len(), 2);
        assert_eq!(
            nbt::int(
                match &records[0] {
                    Tag::Compound(record) => record,
                    other => panic!("a record is a compound, not {other:?}"),
                },
                KEY_CITIZEN
            ),
            Some(7)
        );
        let read = Book::from_compound(&data).expect("the book reads back");
        assert_eq!(read, book);
        assert_eq!(read.target_for(8), Some([-4, 12, 900]));
        assert!(!read.is_dirty(), "a book read from a save has nothing to save");
    }

    /// A record the port cannot read whole is refused by name, and the caller keeps
    /// the book it had — where Java would read zeroes into it (`:216-220`).
    #[test]
    fn a_corrupt_record_is_refused_by_name() {
        let no_target = Compound::from([
            (KEY_TRAVELERS.into(), Tag::List(vec![Tag::Compound(Compound::from([
                (KEY_CITIZEN.into(), Tag::Int(4)),
                (KEY_INITIAL.into(), Tag::Int(20)),
                (KEY_REMAINING.into(), Tag::Int(20)),
            ]))])),
        ]);
        assert_eq!(
            Book::from_compound(&no_target),
            Err(Refusal::NoTarget { at: 0 })
        );

        let not_compound = Compound::from([(KEY_TRAVELERS.into(), Tag::List(vec![Tag::Int(1)]))]);
        assert_eq!(
            Book::from_compound(&not_compound),
            Err(Refusal::Malformed { at: 0 })
        );

        let backwards = Compound::from([(
            KEY_TRAVELERS.into(),
            nbt::compound_list(vec![Compound::from([
                (KEY_CITIZEN.into(), Tag::Int(4)),
                (KEY_TARGET.into(), nbt::java_position([0, 0, 0])),
                (KEY_INITIAL.into(), Tag::Int(20)),
                (KEY_REMAINING.into(), Tag::Int(40)),
            ])]),
        )]);
        assert_eq!(
            Book::from_compound(&backwards),
            Err(Refusal::ImpossibleTimes {
                at: 0,
                initial: 20,
                remaining: 40
            })
        );

        // An empty or absent list is an empty book, not a refusal.
        assert!(Book::from_compound(&Compound::new()).expect("nothing to read").is_empty());
    }

    /// A save carrying the same citizen twice keeps the last record, as a
    /// `HashMap.put` does (`:133`).
    #[test]
    fn a_duplicate_citizen_keeps_the_last_record() {
        let twice = Compound::from([(
            KEY_TRAVELERS.into(),
            nbt::compound_list(vec![
                Compound::from([
                    (KEY_CITIZEN.into(), Tag::Int(4)),
                    (KEY_TARGET.into(), nbt::java_position([1, 1, 1])),
                    (KEY_INITIAL.into(), Tag::Int(20)),
                    (KEY_REMAINING.into(), Tag::Int(20)),
                ]),
                Compound::from([
                    (KEY_CITIZEN.into(), Tag::Int(4)),
                    (KEY_TARGET.into(), nbt::java_position([2, 2, 2])),
                    (KEY_INITIAL.into(), Tag::Int(40)),
                    (KEY_REMAINING.into(), Tag::Int(40)),
                ]),
            ]),
        )]);
        let book = Book::from_compound(&twice).expect("both records read");
        assert_eq!(book.len(), 1);
        assert_eq!(book.target_for(4), Some([2, 2, 2]));
        assert_eq!(book.get(4).expect("recorded").initial(), 40);
    }
}
