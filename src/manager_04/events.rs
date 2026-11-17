//! What a colony remembers of what happened to it: the ring, the cap, the day, and the
//! evening summary.
//!
//! Java reference: `core/colony/managers/EventDescriptionManager.java` (150 lines), the
//! nine description classes under `core/colony/eventhooks/` it is fed with, their two
//! abstract parents (`AbstractEvent.java:12-84`, `AbstractCitizenEvent.java:15`,
//! `AbstractBuildingEvent.java:17`), the registry that names them
//! (`apiimp/initializer/ModColonyEventDescriptionTypeInitializer.java:27-35`) and the
//! two constants it obeys (`ColonyConstants.java:69`, `NbtTagConstants.java:9,73,98,126-130`).
//!
//! | Question | Answer | Java |
//! | --- | --- | --- |
//! | how many events a colony keeps | **100**, the oldest dropped first | `:55-58` |
//! | what day an event happened | the colony's day **at the moment it is added**, never the caller's | `:59` |
//! | which events are counted in the summary | `includeInSummary` **and** today | `:125` |
//! | what a save carries | one list, each record naming its own type | `:104-117` |
//! | what an unknown type does on load | it is **skipped**, not refused | `:92-96` |
//!
//! **The cap is enforced on add and not on load, and that is Java's own asymmetry.**
//! `addEventDescription` polls the front when the ring is full (`:55-58`), while
//! `deserializeNBT` appends with no check at all (`:99`). So a save written by a version
//! with a bigger constant comes back bigger, and **stays** bigger: every later event
//! costs one of the oldest — one `poll` and one `add`, so the count never moves and never
//! comes back under the cap. The port keeps both halves and a test pins the reload; the
//! consequence, a ring that rolls at a size it should not have, is Java's too.
//!
//! **`includeInSummary` is per instance, not per type.** It is a constructor argument
//! serialised with the event (`AbstractEvent.java:36-47`) and only three of the nine
//! types are built with `true` (`CitizenBornEvent.java:37`, `CitizenDiedEvent.java:42`,
//! `CitizenGrownUpEvent.java:37`). So [`Kind::default_summarised`] is a *default*, and a
//! record read from a save keeps the flag it was written with — a hand-edited save can
//! ask for a summary of a building event, and Java would obey.
//!
//! **Three divergences, each named where it costs something.**
//!
//! 1. **The summary is ordered.** Java groups into an `Object2IntOpenHashMap` and
//!    iterates it (`:132`), so its order is a hash order. The port keeps the order the
//!    keys were first seen, which is the order the events happened — the only order a
//!    test can pin, and the one a player reads.
//! 2. **A summarised event with no summary key is dropped.** Java would count it under
//!    the key `""` (the default of `getSummaryTranslationKey`, `IColonyEventDescription.java:88-92`)
//!    and the client would show an empty translation. No registered type can reach it;
//!    a hand-edited save can, and the port says so instead of writing a blank.
//! 3. **A list item that is not a compound is skipped by name.** Java casts and throws
//!    (`:88`), which costs the colony's whole load.
//!
//! **Pure data, pure rules**: no host call, no clock, no file. The day is a number the
//! caller owns, the message of `computeNews` is a list of keys — sending it needs a
//! recipient, and [`MANQUANT.md`](MANQUANT) says which one is missing.

use std::collections::{BTreeMap, VecDeque};

use mod_api::{Compound, Tag};

use super::nbt;

/// How many descriptions a colony keeps (`ColonyConstants.java:69`).
pub const MAX_COLONY_EVENTS: usize = 100;

/// The list's key in the colony's compound (`EventDescriptionManager.java:35`).
pub const KEY_EVENTS: &str = "event_descs_list";
/// The type's path inside one record (`:111` / `:89`, `NbtTagConstants.java:9`).
pub const KEY_NAME: &str = "name";
/// `AbstractEvent.java:37`, `NbtTagConstants.java:73`.
pub const KEY_DAY: &str = "day";
/// `AbstractEvent.java:38`, `NbtTagConstants.java:98`.
pub const KEY_SUMMARISE: &str = "summarize";
/// `AbstractCitizenEvent.java:70`, `NbtTagConstants.java:126`.
pub const KEY_EVENT_POS: &str = "eventPos";
/// `AbstractCitizenEvent.java:71`, `NbtTagConstants.java:127`.
pub const KEY_CITIZEN_NAME: &str = "citizenName";
/// `AbstractBuildingEvent.java:63`, `NbtTagConstants.java:129`.
pub const KEY_BUILDING_NAME: &str = "buildingName";
/// `AbstractBuildingEvent.java:64`, `NbtTagConstants.java:130`.
pub const KEY_BUILDING_LEVEL: &str = "buildingLevel";
/// `CitizenDiedEvent.java:62`, `NbtTagConstants.java:128`.
pub const KEY_DEATH_CAUSE: &str = "deathCause";

/// The namespace every description id is built in (`Constants.MOD_ID`).
pub const NAMESPACE: &str = "minecolonies";
/// `EventDescriptionManager.java:136`: the sentence every summary starts with.
pub const SUMMARY_PREFIX_KEY: &str = "com.minecolonies.core.event.summary.prefix";
/// `:140`: what Java joins two counts with.
pub const SUMMARY_SEPARATOR: &str = ", ";
/// `:146`: what Java ends the sentence with, **outside** the translation.
pub const SUMMARY_TERMINATOR: &str = "!";

/// One of the nine registered description types: its id's path, and the summary key it
/// answers.
///
/// The order is `ModColonyEventDescriptionTypeInitializer.java:27-35`, which is the
/// order the registry is filled and therefore the order `getValue` would search it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: nine registrations, and a tenth type is a migration
pub enum Kind {
    CitizenBorn,
    CitizenSpawn,
    VisitorSpawn,
    CitizenDied,
    CitizenGrownUp,
    BuildingBuilt,
    BuildingUpgraded,
    BuildingRepaired,
    BuildingDeconstructed,
}

impl Kind {
    /// The nine, in registration order.
    pub const ALL: [Kind; 9] = [
        Kind::CitizenBorn,
        Kind::CitizenSpawn,
        Kind::VisitorSpawn,
        Kind::CitizenDied,
        Kind::CitizenGrownUp,
        Kind::BuildingBuilt,
        Kind::BuildingUpgraded,
        Kind::BuildingRepaired,
        Kind::BuildingDeconstructed,
    ];

    /// The path of its `ResourceLocation`, which is what the save writes
    /// (`EventDescriptionManager.java:111`) and what the registry keys on
    /// (`ModColonyEventDescriptionTypeInitializer.java:27-35`).
    pub fn path(self) -> &'static str {
        match self {
            Kind::CitizenBorn => "citizen_born",
            Kind::CitizenSpawn => "citizen_spawn",
            Kind::VisitorSpawn => "visitor_spawn",
            Kind::CitizenDied => "citizen_died",
            Kind::CitizenGrownUp => "citizen_grown_up",
            Kind::BuildingBuilt => "building_built",
            Kind::BuildingUpgraded => "building_upgraded",
            Kind::BuildingRepaired => "building_repaired",
            Kind::BuildingDeconstructed => "building_deconstructed",
        }
    }

    /// The path read back, **the only door**: a save names a type by path and Java
    /// answers `null` for a path the registry does not hold (`:91-96`).
    pub fn from_path(path: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|kind| kind.path() == path)
    }

    /// Whether this type is a citizen's event, which decides the key its subject is
    /// saved under (`AbstractCitizenEvent.java:71` against `AbstractBuildingEvent.java:63`).
    pub fn is_citizen(self) -> bool {
        matches!(
            self,
            Kind::CitizenBorn
                | Kind::CitizenSpawn
                | Kind::VisitorSpawn
                | Kind::CitizenDied
                | Kind::CitizenGrownUp
        )
    }

    /// The summary key, or `None` for the six types that never override
    /// `getSummaryTranslationKey` (`IColonyEventDescription.java:88-92`).
    pub fn summary_key(self) -> Option<&'static str> {
        match self {
            Kind::CitizenBorn => Some("com.minecolonies.core.event.summary.citizen.born"),
            Kind::CitizenDied => Some("com.minecolonies.core.event.summary.citizen.died"),
            Kind::CitizenGrownUp => Some("com.minecolonies.core.event.summary.citizen.grownup"),
            _ => None,
        }
    }

    /// What the constructor passes to `AbstractEvent` (`super(true, …)` / `super(false, …)`).
    ///
    /// A **default**: the flag travels with the record, so a loaded description keeps
    /// whatever the save said, exactly as `deserializeNBT` reads it
    /// (`AbstractEvent.java:43-47`).
    pub fn default_summarised(self) -> bool {
        matches!(
            self,
            Kind::CitizenBorn | Kind::CitizenDied | Kind::CitizenGrownUp
        )
    }
}

/// One thing that happened, with the fields Java writes for it.
///
/// `subject` is the citizen's or the building's name — the two abstract parents disagree
/// only on the key, not the shape. `level` is a building event's level after the fact
/// (`AbstractBuildingEvent.java:38`), `cause` a death's damage type
/// (`CitizenDiedEvent.java:23`), and both are `0` / `None` on a record that has no such
/// field, which is what Java's `getInt` and `getString` answer for an absent key.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Description {
    pub kind: Kind,
    pub at: [i32; 3],
    pub subject: String,
    pub level: i32,
    pub cause: Option<String>,
    /// The colony's day when it was added (`EventDescriptionManager.java:59`).
    pub day: i32,
    /// Java's `includeInSummary`, per instance (`AbstractEvent.java:14`).
    pub summarised: bool,
}

impl Description {
    /// A description with Java's constructor defaults: the type's own summary flag,
    /// position zero, no subject, level zero.
    pub fn new(kind: Kind) -> Description {
        Description {
            kind,
            at: [0, 0, 0],
            subject: String::new(),
            level: 0,
            cause: None,
            day: 0,
            summarised: kind.default_summarised(),
        }
    }

    /// `new CitizenBornEvent(pos, name)` — the position and the name
    /// (`CitizenBornEvent.java:37`).
    pub fn with_position(mut self, at: [i32; 3]) -> Description {
        self.at = at;
        self
    }

    pub fn with_subject(mut self, subject: &str) -> Description {
        self.subject = subject.to_owned();
        self
    }

    pub fn with_level(mut self, level: i32) -> Description {
        self.level = level;
        self
    }

    pub fn with_cause(mut self, cause: &str) -> Description {
        self.cause = Some(cause.to_owned());
        self
    }

    /// Overrides the type's default, which is what a loaded record does.
    pub fn with_summarised(mut self, summarised: bool) -> Description {
        self.summarised = summarised;
        self
    }

    /// The one record, as Java writes it: the two abstract keys, then the position and
    /// the one name the type has.
    ///
    /// The type's own path is **not** written here; the manager adds it
    /// (`EventDescriptionManager.java:111`), which is why [`Events::to_compound`] and
    /// this function are two.
    fn to_compound(&self) -> Compound {
        let mut record = Compound::from([
            (KEY_DAY.into(), Tag::Int(self.day)),
            (KEY_SUMMARISE.into(), Tag::Byte(self.summarised as i8)),
            (KEY_EVENT_POS.into(), nbt::position(self.at)),
        ]);
        if self.kind.is_citizen() {
            record.insert(
                KEY_CITIZEN_NAME.into(),
                Tag::String(self.subject.as_str().into()),
            );
        } else {
            record.insert(
                KEY_BUILDING_NAME.into(),
                Tag::String(self.subject.as_str().into()),
            );
            record.insert(KEY_BUILDING_LEVEL.into(), Tag::Int(self.level));
        }
        if let Some(cause) = &self.cause {
            record.insert(KEY_DEATH_CAUSE.into(), Tag::String(cause.as_str().into()));
        }
        record
    }

    /// One record read back, the type already resolved by the caller.
    ///
    /// Every default is Java's: `getInt` answers `0` for an absent number, `getString`
    /// an empty string, `BlockPosUtil.read` the origin, `getBoolean` false.
    fn from_compound(kind: Kind, record: &Compound) -> Description {
        let subject = if kind.is_citizen() {
            nbt::text(record, KEY_CITIZEN_NAME)
        } else {
            nbt::text(record, KEY_BUILDING_NAME)
        };
        Description {
            kind,
            at: nbt::position_in(record, KEY_EVENT_POS).unwrap_or([0, 0, 0]),
            subject: subject.unwrap_or_default(),
            level: nbt::int(record, KEY_BUILDING_LEVEL).unwrap_or(0),
            cause: nbt::text(record, KEY_DEATH_CAUSE),
            day: nbt::int(record, KEY_DAY).unwrap_or(0),
            summarised: nbt::boolean(record, KEY_SUMMARISE),
        }
    }
}

/// One line of the evening summary: a translation key and how many times it happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one key and one count, which is Java's `Object2IntMap.Entry`
pub struct Summary {
    pub key: &'static str,
    pub count: i32,
}

/// What `computeNews` sends, as data: the four things a caller needs to build the
/// sentence, in Java's order.
///
/// Java builds a `MessageBuilder` (`EventDescriptionManager.java:131-147`) and sends it
/// to `colony.getImportantMessageEntityPlayers()` — a recipient the port does not have
/// (`MANQUANT.md` §2). What is here is the sentence's *content*: a prefix key, the
/// entries, `", "` between them, and `"!"` after the last.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the prefix and the entries, which is the whole sentence
pub struct News {
    pub entries: Vec<Summary>,
}

impl News {
    pub fn prefix_key(&self) -> &'static str {
        SUMMARY_PREFIX_KEY
    }

    pub fn separator(&self) -> &'static str {
        SUMMARY_SEPARATOR
    }

    pub fn terminator(&self) -> &'static str {
        SUMMARY_TERMINATOR
    }

    /// The keys in the order they will be shown, each with its count: what the client
    /// has to translate, and in which order.
    pub fn keys(&self) -> Vec<(&'static str, i32)> {
        self.entries.iter().map(|entry| (entry.key, entry.count)).collect()
    }
}

/// Why a record of a save did not become a description — three answers, each one a
/// line Java writes to its log before moving on.
///
/// This is a [`Skip`](Skipped), not a refusal: Java's load of the events never fails,
/// it loses records (`EventDescriptionManager.java:92-96`). A refusal here would cost
/// the colony's whole save for one hand-edited record.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: three ways a record is dropped, closed on purpose
pub enum Skipped {
    /// No `name`: Java builds `minecolonies:` with an empty path and finds no entry
    /// (`:89-96`).
    NoName { index: usize },
    /// A path the registry does not hold (`:91-96`).
    UnknownType { path: String, index: usize },
    /// A list item that is not a compound: Java's cast throws (`:88`).
    NotACompound { index: usize },
}

impl Skipped {
    /// A stable name, namespaced like the other refusals of this port.
    pub fn name(&self) -> &'static str {
        match self {
            Skipped::NoName { .. } => "manager_04.events.skipped.no_name",
            Skipped::UnknownType { .. } => "manager_04.events.skipped.unknown_type",
            Skipped::NotACompound { .. } => "manager_04.events.skipped.not_a_compound",
        }
    }

    pub fn index(&self) -> usize {
        match self {
            Skipped::NoName { index }
            | Skipped::UnknownType { index, .. }
            | Skipped::NotACompound { index } => *index,
        }
    }
}

impl std::fmt::Display for Skipped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// What a read produced: the events that survived, and the records that did not.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: what came back and what was dropped
pub struct Loaded {
    pub events: Events,
    pub skipped: Vec<Skipped>,
}

/// The colony's event ring: at most [`MAX_COLONY_EVENTS`] descriptions, plus the day the
/// colony is on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Events {
    ring: VecDeque<Description>,
    day: i32,
}

impl Events {
    pub fn new() -> Events {
        Events::default()
    }

    /// A colony that already has a day, which is what a load must pass before reading a
    /// save: Java reads the day from the colony and each record keeps its own
    /// (`AbstractEvent.java:37`), so the two are independent.
    pub fn at_day(day: i32) -> Events {
        Events {
            ring: VecDeque::new(),
            day,
        }
    }

    pub fn day(&self) -> i32 {
        self.day
    }

    pub fn set_day(&mut self, day: i32) {
        self.day = day;
    }

    pub fn len(&self) -> usize {
        self.ring.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }

    /// The descriptions, oldest first — the order Java's `ArrayDeque` iterates
    /// (`EventDescriptionManager.java:75`, `:108`).
    pub fn descriptions(&self) -> impl Iterator<Item = &Description> {
        self.ring.iter()
    }

    /// `addEventDescription` (`EventDescriptionManager.java:52-69`), **in Java's order**:
    /// drop the oldest if the ring is full, stamp the day, then append.
    ///
    /// The order is the rule: the day is the *manager's*, not the caller's, so an event
    /// built yesterday and added today is dated today — which is what the summary
    /// counts. Returns the description that was dropped, if any.
    ///
    /// Java also marks the town hall dirty here (`:61-68`); the port has no dirty flag
    /// on a block entity, so the caller saves ([`MANQUANT.md`](MANQUANT) §3).
    pub fn push(&mut self, description: Description) -> Option<Description> {
        let evicted = if self.ring.len() >= MAX_COLONY_EVENTS {
            self.ring.pop_front()
        } else {
            None
        };
        let mut stamped = description;
        stamped.day = self.day;
        self.ring.push_back(stamped);
        evicted
    }

    /// `computeNews`'s counting half (`EventDescriptionManager.java:120-130`): events
    /// that ask to be summarised **and** happened today, one line per translation key.
    ///
    /// The order is the order the keys were first seen (divergence 1 of the module
    /// note); a summarised event whose type has no key is not counted (divergence 2).
    pub fn summary(&self) -> Vec<Summary> {
        let mut order: Vec<&'static str> = Vec::new();
        let mut counts: BTreeMap<&'static str, i32> = BTreeMap::new();
        for description in &self.ring {
            if !description.summarised || description.day != self.day {
                continue;
            }
            let Some(key) = description.kind.summary_key() else {
                continue;
            };
            match counts.get_mut(key) {
                Some(count) => *count += 1,
                None => {
                    counts.insert(key, 1);
                    order.push(key);
                }
            }
        }
        order
            .into_iter()
            .map(|key| Summary {
                key,
                count: counts.get(key).copied().unwrap_or(0),
            })
            .collect()
    }

    /// The whole of `computeNews` that can be said without a recipient
    /// (`EventDescriptionManager.java:131-148`): `None` when nothing happened today,
    /// which is Java's `if (builder != null)`.
    pub fn news(&self) -> Option<News> {
        let entries = self.summary();
        if entries.is_empty() {
            None
        } else {
            Some(News { entries })
        }
    }

    /// `serializeNBT` (`EventDescriptionManager.java:104-117`): one list, each record
    /// carrying its own type path under `name`.
    pub fn to_compound(&self) -> Compound {
        let mut records = Vec::with_capacity(self.ring.len());
        for description in &self.ring {
            let mut record = description.to_compound();
            record.insert(KEY_NAME.into(), Tag::String(description.kind.path().into()));
            records.push(record);
        }
        Compound::from([(KEY_EVENTS.into(), nbt::compound_list(records))])
    }

    /// `deserializeNBT` (`EventDescriptionManager.java:82-101`), with the three skips
    /// named instead of logged.
    ///
    /// **The cap is not enforced here**, and that is Java's own behaviour (`:99`): a
    /// save with a hundred and fifty records comes back with a hundred and fifty, and
    /// the next [`push`](Self::push) drops one. The day is not read either — a colony's
    /// day lives in the colony (`Colony.java:882`), so the caller sets it first.
    pub fn read(data: &Compound) -> Loaded {
        let mut events = Events::default();
        let mut skipped = Vec::new();
        for (index, item) in nbt::items(data, KEY_EVENTS).iter().enumerate() {
            let Tag::Compound(record) = item else {
                skipped.push(Skipped::NotACompound { index });
                continue;
            };
            let Some(path) = nbt::text(record, KEY_NAME) else {
                skipped.push(Skipped::NoName { index });
                continue;
            };
            let Some(kind) = Kind::from_path(&path) else {
                skipped.push(Skipped::UnknownType { path, index });
                continue;
            };
            events.ring.push_back(Description::from_compound(kind, record));
        }
        Loaded { events, skipped }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn born(at: [i32; 3], name: &str) -> Description {
        Description::new(Kind::CitizenBorn)
            .with_position(at)
            .with_subject(name)
    }

    #[test]
    fn the_nine_types_keep_their_registration_order_and_their_paths() {
        assert_eq!(Kind::ALL.len(), 9, "ModColonyEventDescriptionTypeInitializer.java:27-35");
        let paths: Vec<&str> = Kind::ALL.iter().map(|kind| kind.path()).collect();
        assert_eq!(
            paths,
            [
                "citizen_born",
                "citizen_spawn",
                "visitor_spawn",
                "citizen_died",
                "citizen_grown_up",
                "building_built",
                "building_upgraded",
                "building_repaired",
                "building_deconstructed"
            ]
        );
        for kind in Kind::ALL {
            assert_eq!(Kind::from_path(kind.path()), Some(kind));
        }
        assert_eq!(Kind::from_path("citizen_bornn"), None);
        assert_eq!(Kind::from_path(""), None);
        // Three types answer a summary key, and they are exactly the three whose
        // constructor passes `true` (`:37`, `:37`, `:42`).
        let keyed: Vec<Kind> = Kind::ALL
            .into_iter()
            .filter(|kind| kind.summary_key().is_some())
            .collect();
        assert_eq!(keyed.len(), 3);
        let summarised: Vec<Kind> = Kind::ALL
            .into_iter()
            .filter(|kind| kind.default_summarised())
            .collect();
        assert_eq!(keyed, summarised, "a count with no key is a blank sentence");
        assert!(Kind::CitizenBorn.is_citizen());
        assert!(!Kind::BuildingBuilt.is_citizen());
    }

    #[test]
    fn the_ring_keeps_a_hundred_from_the_end_and_dates_each_one_today() {
        let mut events = Events::at_day(4);
        for index in 0..MAX_COLONY_EVENTS as i32 {
            assert!(events.push(born([index, 64, 0], "Ana")).is_none());
        }
        assert_eq!(events.len(), 100);
        assert_eq!(events.descriptions().next().map(|d| d.at[0]), Some(0));
        // The hundred-and-first drops the front, in that order: poll, then add.
        let evicted = events.push(born([999, 64, 0], "Bo"));
        assert_eq!(evicted.map(|d| d.at[0]), Some(0));
        assert_eq!(events.len(), 100);
        assert_eq!(events.descriptions().next().map(|d| d.at[0]), Some(1));
        assert_eq!(
            events.descriptions().last().map(|d| d.at[0]),
            Some(999),
            "the newcomer is last, at the back of the ring"
        );
        // Stamping: a description built with an old day is dated the manager's.
        let stale = born([1, 1, 1], "Cy").with_level(0);
        let mut old = stale.clone();
        old.day = 1;
        events.push(old);
        assert_eq!(
            events.descriptions().last().map(|d| d.day),
            Some(4),
            "EventDescriptionManager.java:59"
        );
    }

    #[test]
    fn the_summary_counts_today_and_only_what_asks_for_it() {
        let mut events = Events::at_day(7);
        events.push(born([0, 64, 0], "Ana"));
        events.push(born([0, 64, 0], "Bo"));
        events.push(Description::new(Kind::CitizenDied).with_subject("Ana"));
        // Not counted: a spawn does not ask for it (`CitizenSpawnedEvent.java:37`).
        events.push(Description::new(Kind::CitizenSpawn).with_subject("Cy"));
        // Counted although its type does not ask: the flag is per instance.
        events.push(
            Description::new(Kind::BuildingBuilt)
                .with_subject("Town Hall")
                .with_level(1)
                .with_summarised(true),
        );
        let summary = events.summary();
        assert_eq!(
            summary,
            vec![
                Summary {
                    key: "com.minecolonies.core.event.summary.citizen.born",
                    count: 2
                },
                Summary {
                    key: "com.minecolonies.core.event.summary.citizen.died",
                    count: 1
                },
            ],
            "an event with no key is not counted, even when it asks"
        );
        // Yesterday's events are yesterday's news.
        events.set_day(8);
        assert_eq!(events.summary(), Vec::new());
        assert_eq!(events.news(), None, "Java: builder == null sends nothing");
        events.push(born([0, 64, 0], "Dee"));
        let news = events.news().expect("one birth today");
        assert_eq!(news.prefix_key(), SUMMARY_PREFIX_KEY);
        assert_eq!(news.separator(), ", ");
        assert_eq!(news.terminator(), "!");
        assert_eq!(news.keys(), vec![("com.minecolonies.core.event.summary.citizen.born", 1)]);
    }

    #[test]
    fn a_summarised_event_whose_type_has_no_key_is_dropped_not_shown_blank() {
        let mut events = Events::at_day(2);
        events.push(Description::new(Kind::VisitorSpawn).with_summarised(true));
        assert_eq!(events.summary(), Vec::new());
        assert_eq!(events.len(), 1, "the event is kept, only the count is lost");
    }

    #[test]
    fn a_record_round_trips_with_the_name_and_level_of_its_kind() {
        let mut events = Events::at_day(3);
        events.push(
            Description::new(Kind::BuildingUpgraded)
                .with_position([10, 70, -4])
                .with_subject("Bâtiment des fermiers")
                .with_level(4),
        );
        events.push(
            Description::new(Kind::CitizenDied)
                .with_position([11, 70, -4])
                .with_subject("Ana")
                .with_cause("minecraft:fall"),
        );
        let compound = events.to_compound();
        let read = Events::read(&compound);
        assert!(read.skipped.is_empty());
        assert_eq!(read.events.len(), 2);
        assert_eq!(read.events.day(), 0, "the day lives in the colony, not here");
        let first = read.events.descriptions().next().expect("one record");
        assert_eq!(first.kind, Kind::BuildingUpgraded);
        assert_eq!(first.at, [10, 70, -4]);
        assert_eq!(first.subject, "Bâtiment des fermiers");
        assert_eq!(first.level, 4);
        assert_eq!(first.cause, None);
        assert_eq!(first.day, 3);
        assert!(!first.summarised);
        let second = read.events.descriptions().nth(1).expect("two records");
        assert_eq!(second.cause.as_deref(), Some("minecraft:fall"));
        assert!(second.summarised, "kept from the save, AbstractEvent.java:46");
        assert_eq!(second.level, 0, "a citizen event writes no level");
    }

    #[test]
    fn three_ways_a_record_is_skipped_and_none_of_them_refuses_the_save() {
        let good = Compound::from([
            (KEY_NAME.into(), Tag::String("citizen_spawn".into())),
            (KEY_DAY.into(), Tag::Int(9)),
            (KEY_CITIZEN_NAME.into(), Tag::String("Ana".into())),
        ]);
        let unnamed = Compound::from([(KEY_DAY.into(), Tag::Int(9))]);
        let unknown = Compound::from([(KEY_NAME.into(), Tag::String("raid_won".into()))]);
        let data = Compound::from([(
            KEY_EVENTS.into(),
            Tag::List(vec![
                Tag::Compound(good),
                Tag::Int(7),
                Tag::Compound(unnamed),
                Tag::Compound(unknown),
            ]),
        )]);
        let read = Events::read(&data);
        assert_eq!(read.events.len(), 1, "the one good record survived");
        assert_eq!(
            read.skipped,
            vec![
                Skipped::NotACompound { index: 1 },
                Skipped::NoName { index: 2 },
                Skipped::UnknownType {
                    path: "raid_won".to_owned(),
                    index: 3
                },
            ]
        );
        for skipped in &read.skipped {
            assert!(skipped.name().starts_with("manager_04.events.skipped."));
        }
        assert_eq!(read.skipped[1].index(), 2);
    }

    #[test]
    fn the_cap_is_not_applied_on_load_and_the_ring_never_shrinks_back() {
        let mut records = Vec::new();
        for index in 0..150 {
            records.push(Tag::Compound(Compound::from([
                (KEY_NAME.into(), Tag::String("citizen_born".into())),
                (KEY_DAY.into(), Tag::Int(index)),
            ])));
        }
        let data = Compound::from([(KEY_EVENTS.into(), Tag::List(records))]);
        let mut read = Events::read(&data).events;
        assert_eq!(read.len(), 150, "deserializeNBT has no cap, :99");
        // `push` is one `poll` and one `add` (`:55-58`): the oldest goes, the newcomer
        // arrives, and the count does not move — 150 stays 150 for ever.
        let evicted = read
            .push(Description::new(Kind::CitizenBorn))
            .expect("the ring is over the cap");
        assert_eq!(evicted.day, 0, "the oldest event is the one dropped");
        assert_eq!(read.len(), 150, "one in, one out");
        assert_eq!(read.descriptions().next().map(|d| d.day), Some(1));
        for _ in 0..200 {
            let _ = read.push(Description::new(Kind::CitizenBorn));
        }
        assert_eq!(read.len(), 150, "and it never comes back under the cap");
        assert_eq!(
            read.descriptions().next().map(|d| d.day),
            Some(0),
            "the 200 pushes rolled past the end: the stamped day is the manager's, 0 here"
        );
    }

    #[test]
    fn an_empty_compound_reads_as_no_events_and_the_day_starts_at_zero() {
        let read = Events::read(&Compound::new());
        assert!(read.events.is_empty());
        assert!(read.skipped.is_empty());
        assert_eq!(read.events.day(), 0);
        assert_eq!(Events::new(), Events::default());
        assert_eq!(read.events.to_compound().len(), 1, "the empty list is still written");
    }
}
