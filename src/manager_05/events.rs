//! What a colony's event book holds, and who an entity belongs to.
//!
//! Java reference: `core/colony/managers/EventManager.java` (271 lines), with
//! `api/colony/colonyEvents/registry/ColonyEventTypeRegistryEntry.java:73-76`
//! (the deserialiser), `api/colony/colonyEvents/IColonyRaidEvent.java:12` and
//! `IColonyEntitySpawnEvent.java` (both interfaces), the eight event ids of
//! `apiimp/initializer/ModColonyEventTypeInitializer.java:33-40`, and
//! `api/util/constant/NbtTagConstants.java:9`, `:122`.
//!
//! **What was already covered, and is therefore not repeated here.**
//! `crate::events::raid` ports `EventManager`'s *live* half: the map, the id
//! counter's arithmetic (`take_from`), `addEvent`'s id-zero refusal, and
//! `onColonyTick`'s start/update/finish/drop (`Lifecycle`, `Raid`, `Status`).
//! This file ports the other three quarters, which that module deliberately
//! leaves out:
//!
//! | Question | Answer | Java |
//! | --- | --- | --- |
//! | what a save holds | one `event_manager` compound: the counter **and** a list of records | `:249-264` |
//! | what a record carries | the event's own payload, plus its type **path** under `name` | `:256` |
//! | what an unknown type costs on load | that record only, named and skipped | `:232-237` |
//! | what an entity is bound to | the event's id — or removal, when that id is no spawn event | `:104-113` |
//! | when the colony is saved | after the counter moves, after a put, after a removal | `:93`, `:76`, `:204` |
//!
//! **Three asymmetries, all Java's, all pinned by a test.**
//!
//! 1. **`addEvent` refuses id zero; the read does not.** `:70-74` refuses a
//!    zero id, but `:239-240` stores whatever `deserializeEvent` produced —
//!    `events.put(colonyEvent.getID(), colonyEvent)` — and an event whose payload
//!    has no `mc_event_id` reads `getInt`'s default, **zero**. A save can
//!    therefore hold key `0`, which no `addEvent` could ever create.
//! 2. **A save with a list but no counter restarts the counter at zero.** `:243`
//!    reads `event_currentID` with `getInt`, so an absent tag answers `0`, and
//!    the next [`EventBook::take_id`] hands out `0` — an id [`add_event`] will
//!    refuse. Java has exactly this hole; the port names the consequence instead
//!    of papering over it.
//! 3. **The counter is saved, the record ids are not renumbered.** The key of the
//!    map is the id inside the record (`:239`), never the record's position in
//!    the list, so a list written out of order comes back keyed by its own ids.
//!
//! **The counter cannot be handed back to a `Lifecycle`.** `Lifecycle`'s counter
//! is private, its `new()` starts at `FIRST_ID`, and it has no setter
//! (`src/events/raid.rs:211-215`): a `Lifecycle` reconstructed from a save would
//! hand out id 1 again and collide with every event the save already holds.
//! [`IdCounter`] is therefore the counter a save round-trips, and it calls
//! `take_from` so the wrap rule has **one** source. `MANQUANT.md` §1 records the
//! missing setter.
//!
//! **Pure data, pure rules**: no host call, no clock, no file. `markDirty()` is a
//! [`writes`](EventBook::writes) tally, so a test can hold Java to its own order
//! and prove that a refused call wrote nothing.

use std::collections::BTreeMap;

use mod_api::{Compound, Tag};

use super::nbt;
use crate::events::raid::{take_from, FIRST_ID};

/// The compound the book lives in (`EventManager.java:38`).
pub const KEY_EVENT_MANAGER: &str = "event_manager";
/// The counter, inside that compound (`:37`). **Not** `mc_event_id`.
pub const KEY_EVENT_ID: &str = "event_currentID";
/// The list of records (`:39`).
pub const KEY_EVENT_LIST: &str = "events_list";
/// The type path, inside each record (`:256`, `NbtTagConstants.java:9`).
pub const KEY_NAME: &str = "name";
/// The event's own id, inside its payload (`AbstractShipRaidEvent.java:503`,
/// `NbtTagConstants.java:122`) — the id the read keys on.
pub const KEY_SAVED_ID: &str = "mc_event_id";

/// The namespace every type id is built in (`:230`, `Constants.MOD_ID`).
pub const NAMESPACE: &str = "minecolonies";

/// The eight registered event types, by registry path
/// (`ModColonyEventTypeInitializer.java:33-40`).
pub const REGISTERED_TYPES: [&str; 8] = [
    "pirate_raid",
    "barbarian_raid",
    "egyptian_raid",
    "amazon_raid",
    "norsemen_raid",
    "norsemen_ship_raid",
    "pirate_ground_raid",
    "drowned_pirate_raid",
];

/// Whether a type's events take entities.
///
/// Java tests `instanceof IColonyEntitySpawnEvent` (`:107`, `:125`, `:141`).
/// **Every one of the eight registered types passes it**, because every one is an
/// `IColonyRaidEvent` and that interface extends `IColonyEntitySpawnEvent`
/// (`IColonyRaidEvent.java:12`). So the answer is a property of the type, and in
/// this tree it is true for all eight — the only false answers are a mod's event
/// type and a type the port does not know, which is exactly when Java's
/// `instanceof` would fail too.
pub fn spawns_entities(kind: &str) -> bool {
    REGISTERED_TYPES.contains(&kind)
}

/// Whether a name can be a `ResourceLocation` **path**.
///
/// Java writes `getEventTypeID().getPath()` (`:256`) and reads it back with
/// `new ResourceLocation(MOD_ID, name)` (`:230`). A hand-edited name holding a
/// `:` makes that constructor throw, which costs the colony's whole load; the
/// port skips the one record instead and names it ([`Skipped::NotAPath`]).
pub fn is_a_path(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_-./".contains(character))
}

/// The counter, and the round trip Java gives it.
///
/// `getAndTakeNextEventID` (`:85-95`) is `take_from`'s arithmetic:
/// [`FIRST_ID`] at birth, the value handed out is the value *before* the
/// increment, and the wrap resets to [`FIRST_ID`] when the counter passes
/// `MAX_VALUE - 100`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct IdCounter {
    value: i32,
}

impl Default for IdCounter {
    fn default() -> IdCounter {
        IdCounter::new()
    }
}

impl IdCounter {
    /// A colony that has never had an event (`:44`).
    pub fn new() -> IdCounter {
        IdCounter { value: FIRST_ID }
    }

    /// The counter a save carries (`:243`), **unclamped**: Java's `getInt`
    /// answers whatever the tag says, `0` included, and rule 2 of the module note
    /// is what that costs.
    pub fn from_saved(value: i32) -> IdCounter {
        IdCounter { value }
    }

    pub fn value(&self) -> i32 {
        self.value
    }

    /// `getAndTakeNextEventID` (`:85-95`), the wrap and the increment shared with
    /// [`crate::events::raid::take_from`].
    pub fn take(&mut self) -> i32 {
        let (id, next) = take_from(self.value);
        self.value = next;
        id
    }

    /// The counter as a save holds it (`:260`).
    pub fn to_compound(&self) -> Compound {
        Compound::from([(KEY_EVENT_ID.into(), Tag::Int(self.value))])
    }
}

/// One record of the book: the id Java keys on, the type, and the payload the
/// event's own `serializeNBT` produced.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Event {
    /// `IColonyEvent.getID()` (`:34`), from the payload's `mc_event_id`.
    pub id: i32,
    /// `getEventTypeID().getPath()` (`:256`) — the path, never the namespace.
    pub kind: String,
    /// Everything the event saves.
    pub payload: Compound,
}

impl Event {
    pub fn new(id: i32, kind: &str) -> Event {
        let mut payload = Compound::new();
        payload.insert(KEY_SAVED_ID.into(), Tag::Int(id));
        Event {
            id,
            kind: kind.to_owned(),
            payload,
        }
    }

    /// The record as the list holds it: the payload, with its type written over
    /// `name` (`:255-257`). `putString` overwrites, so a payload that already had
    /// a `name` loses it.
    pub fn to_record(&self) -> Compound {
        let mut record = self.payload.clone();
        record.insert(KEY_NAME.into(), Tag::String(self.kind.as_str().into()));
        record
    }
}

/// Why the book refused a write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum Refusal {
    /// `:70-74`: an event with no id is logged and **not stored**, and the colony
    /// is not marked dirty.
    NoId,
    /// The id names no stored event. Java dereferences `null` and would throw;
    /// the port names it ([`EventBook::tile_entity_broken`]).
    Unknown(i32),
}

impl Refusal {
    pub fn name(&self) -> &'static str {
        match self {
            Refusal::NoId => "no_id",
            Refusal::Unknown(_) => "unknown_event",
        }
    }
}

/// What an entity registration did (`:104-113`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two branches of Java's `registerEntity`
pub enum Route {
    /// The event exists and takes entities: the caller hands the entity over.
    Registered,
    /// The id names no event, **or** names one that does not take entities: Java
    /// removes the entity from the world (`:109`) and so must the caller.
    Discarded,
}

impl Route {
    pub fn name(self) -> &'static str {
        match self {
            Route::Registered => "registered",
            Route::Discarded => "discarded",
        }
    }
}

/// A record the read did not keep, named.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the four ways one list item can be wrong
pub enum Skipped {
    /// Not a compound. Java casts and throws (`:229`), costing the colony.
    NotACompound { index: usize },
    /// No `name`, or an empty one: `getString` answers `""` and the registry has
    /// nothing there (`:230-237`).
    NoName { index: usize },
    /// A `name` that cannot be a path (`:230`), which throws in Java.
    NotAPath { name: String, index: usize },
    /// A name the registry does not hold (`:232-237`): logged and skipped.
    UnknownType { kind: String, index: usize },
}

impl Skipped {
    pub fn name(&self) -> &'static str {
        match self {
            Skipped::NotACompound { .. } => "not_a_compound",
            Skipped::NoName { .. } => "no_name",
            Skipped::NotAPath { .. } => "not_a_path",
            Skipped::UnknownType { .. } => "unknown_type",
        }
    }

    pub fn index(&self) -> usize {
        match self {
            Skipped::NotACompound { index }
            | Skipped::NoName { index }
            | Skipped::NotAPath { index, .. }
            | Skipped::UnknownType { index, .. } => *index,
        }
    }
}

/// What a read produced (`:221-246`).
#[derive(Clone, Debug, Default, PartialEq)]
// exhaustive: what came back and what was dropped
pub struct Loaded {
    pub book: EventBook,
    pub skipped: Vec<Skipped>,
}

/// The colony's event book: the counter, the records, and the routing Java does
/// around them.
#[derive(Clone, Debug, Default, PartialEq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct EventBook {
    counter: IdCounter,
    events: BTreeMap<i32, Event>,
    writes: u32,
}

impl EventBook {
    pub fn new() -> EventBook {
        EventBook::default()
    }

    pub fn counter(&self) -> i32 {
        self.counter.value()
    }

    /// How many times Java would have called `colony.markDirty()`. Ten calls is a
    /// dirty flag, not a counter, in Java; the port counts so that a test can say
    /// "nothing was written" without reading a private field.
    pub fn writes(&self) -> u32 {
        self.writes
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// The ids the book holds, ascending — Java iterates a `HashMap`
    /// (`:54`), whose order is unspecified; a save that depends on it is not
    /// reproducible, so the port sorts.
    pub fn ids(&self) -> Vec<i32> {
        self.events.keys().copied().collect()
    }

    /// `getEventByID` (`:173-176`).
    pub fn get(&self, id: i32) -> Option<&Event> {
        self.events.get(&id)
    }

    /// The type of a stored event, which is the only fact the routing needs.
    pub fn kind_of(&self, id: i32) -> Option<&str> {
        self.events.get(&id).map(|event| event.kind.as_str())
    }

    /// `getAndTakeNextEventID` (`:85-95`): the wrap, the increment, and
    /// `markDirty` (`:93`).
    pub fn take_id(&mut self) -> i32 {
        let id = self.counter.take();
        self.writes += 1;
        id
    }

    /// `addEvent` (`:68-77`), **in Java's order**: the id-zero test first, then
    /// the put, then the save. An id already present is overwritten, because
    /// `events.put` is a put and not a check.
    pub fn add_event(&mut self, event: Event) -> Result<(), Refusal> {
        if event.id == 0 {
            return Err(Refusal::NoId);
        }
        self.events.insert(event.id, event);
        self.writes += 1;
        Ok(())
    }

    /// `remove` is not this type's business — the tick owns it, in
    /// `crate::events::raid::Lifecycle` — but a removal must still be saved,
    /// which is what `:195` and `:204` are for. The tick calls this.
    pub fn note_removal(&mut self) {
        self.writes += 1;
    }

    /// `registerEntity` (`:104-113`): the event must exist **and** take entities,
    /// or the entity is discarded. Nothing is written either way — Java does not
    /// mark the colony dirty here, because removing an entity does not change the
    /// colony.
    pub fn register_entity(&self, id: i32) -> Route {
        match self.kind_of(id) {
            Some(kind) if spawns_entities(kind) => Route::Registered,
            _ => Route::Discarded,
        }
    }

    /// `unregisterEntity` (`:122-129`): **silent** when the event is unknown, and
    /// silent when it takes no entities. Answers whether the event was reached.
    pub fn unregister_entity(&self, id: i32) -> bool {
        matches!(self.kind_of(id), Some(kind) if spawns_entities(kind))
    }

    /// `onEntityDeath` (`:138-145`): same two silences as the unregister.
    pub fn entity_died(&self, id: i32) -> bool {
        self.unregister_entity(id)
    }

    /// `onTileEntityBreak` (`:148-155`): the one routing that refuses by name
    /// instead of staying silent — Java tests `event != null` and does nothing.
    pub fn tile_entity_broken(&self, id: i32) -> Result<(), Refusal> {
        match self.events.get(&id) {
            Some(_) => Ok(()),
            None => Err(Refusal::Unknown(id)),
        }
    }

    /// `onNightFall` (`:158-164`): every event, in id order. Java's order is the
    /// map's, and no event in this tree reads another from here.
    pub fn night_fall(&self) -> Vec<i32> {
        self.ids()
    }

    /// `writeToNBT` (`:249-264`), the part this type owns: the compound, the
    /// counter, the list. Java then calls `structureManager.writeToNBT(compound)`
    /// **on the colony's compound** (`:263`) — see
    /// [`crate::manager_05::structure::Backups::write_into`]; that write is
    /// outside `event_manager`, and a port that nested it here would put it where
    /// Java's read does not look.
    pub fn to_compound(&self) -> Compound {
        let records: Vec<Compound> = self
            .events
            .values()
            .map(|event| event.to_record())
            .collect();
        let mut book = self.counter.to_compound();
        book.insert(KEY_EVENT_LIST.into(), nbt::compound_list(records));
        Compound::from([(KEY_EVENT_MANAGER.into(), Tag::Compound(book))])
    }

    /// `readFromNBT` (`:221-246`).
    ///
    /// The guard is the rule: **without an `event_manager` compound nothing at
    /// all is read** — not the counter, and (in Java, `:244`) not the structure
    /// backups either. The counter is then `getInt`'s answer, `0`.
    pub fn read(compound: &Compound) -> Loaded {
        let mut book = EventBook::new();
        let mut skipped = Vec::new();
        let Some(Tag::Compound(manager)) = compound.get(&KEY_EVENT_MANAGER.into()) else {
            return Loaded { book, skipped };
        };

        for (index, item) in nbt::items(manager, KEY_EVENT_LIST).iter().enumerate() {
            let Tag::Compound(record) = item else {
                skipped.push(Skipped::NotACompound { index });
                continue;
            };
            let name = nbt::text(record, KEY_NAME).unwrap_or_default();
            if name.is_empty() {
                skipped.push(Skipped::NoName { index });
                continue;
            }
            if !is_a_path(&name) {
                skipped.push(Skipped::NotAPath { name, index });
                continue;
            }
            if !spawns_entities(&name) {
                skipped.push(Skipped::UnknownType { kind: name, index });
                continue;
            }
            // `:239-240`: the key is the event's **own** id, and the read never
            // checks it for zero (rule 1 of the module note).
            let id = nbt::int(record, KEY_SAVED_ID).unwrap_or(0);
            book.events.insert(
                id,
                Event {
                    id,
                    kind: name,
                    payload: record.clone(),
                },
            );
        }

        // `:243`, unconditional inside the guard: absent `event_currentID`
        // answers zero, and rule 2 is what the next `take_id` does with it.
        book.counter = IdCounter::from_saved(nbt::int(manager, KEY_EVENT_ID).unwrap_or(0));
        Loaded { book, skipped }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raid(id: i32) -> Event {
        Event::new(id, "pirate_raid")
    }

    /// A colony compound holding one `event_manager`, built by hand so a test can
    /// write a save Java would never produce.
    fn book_of(counter: i32, records: Vec<Compound>) -> Compound {
        let mut manager = Compound::new();
        manager.insert(KEY_EVENT_ID.into(), Tag::Int(counter));
        manager.insert(KEY_EVENT_LIST.into(), nbt::compound_list(records));
        Compound::from([(KEY_EVENT_MANAGER.into(), Tag::Compound(manager))])
    }

    #[test]
    fn the_first_id_taken_is_one_and_the_wrap_is_the_raid_module_s() {
        let mut book = EventBook::new();
        assert_eq!(book.take_id(), 1, "EventManager.java:44, :92-94");
        assert_eq!(book.take_id(), 2);
        assert_eq!(book.writes(), 2, ":93 marks the colony dirty on each take");

        let mut near = EventBook::new();
        near.counter = IdCounter::from_saved(i32::MAX - 99);
        assert_eq!(near.take_id(), 1, ":87-90 wraps a hundred short of the top");
    }

    #[test]
    fn an_event_with_no_id_is_refused_before_anything_is_written() {
        let mut book = EventBook::new();
        let refused = book.add_event(Event::new(0, "pirate_raid"));
        assert_eq!(refused, Err(Refusal::NoId), ":70-74");
        assert_eq!(book.writes(), 0, "no put, no markDirty");
        assert!(book.is_empty(), "and no record");
    }

    #[test]
    fn a_second_event_on_the_same_id_overwrites_the_first() {
        let mut book = EventBook::new();
        assert_eq!(book.add_event(raid(3)), Ok(()));
        let mut second = raid(3);
        second.kind = String::from("barbarian_raid");
        assert_eq!(book.add_event(second), Ok(()));
        assert_eq!(book.len(), 1, ":75 is a put, not a check");
        assert_eq!(book.kind_of(3), Some("barbarian_raid"));
    }

    #[test]
    fn a_record_carries_its_path_under_name_and_its_id_inside_the_payload() {
        let mut book = EventBook::new();
        assert_eq!(book.add_event(raid(4)), Ok(()));
        let saved = book.to_compound();
        let Some(Tag::Compound(manager)) = saved.get(&KEY_EVENT_MANAGER.into()) else {
            panic!("the book writes event_manager");
        };
        assert_eq!(nbt::int(manager, KEY_EVENT_ID), Some(1), ":260 saves the counter");
        let records = nbt::compounds(manager, KEY_EVENT_LIST);
        assert_eq!(records.len(), 1);
        assert_eq!(
            nbt::text(records[0], KEY_NAME).as_deref(),
            Some("pirate_raid"),
            ":256 writes the path, with no namespace"
        );
        assert_eq!(nbt::int(records[0], KEY_SAVED_ID), Some(4), "and the payload keeps its id");
    }

    #[test]
    fn a_read_keys_on_the_record_s_own_id_not_on_its_place_in_the_list() {
        let saved = book_of(
            9,
            vec![raid(7).to_record(), raid(2).to_record(), {
                let mut lost = raid(5).to_record();
                lost.remove(&KEY_SAVED_ID.into());
                lost
            }],
        );

        let loaded = EventBook::read(&saved);
        assert_eq!(loaded.skipped, vec![], "all three records are well formed");
        assert_eq!(
            loaded.book.ids(),
            vec![0, 2, 7],
            ":239 keys on the id inside the payload; a payload without one answers getInt's 0"
        );
        assert_eq!(loaded.book.counter(), 9, ":243 reads the counter from the tag");
        assert_eq!(loaded.book.writes(), 0, "a read is not a write");
    }

    #[test]
    fn two_records_with_no_id_collapse_onto_one_key() {
        let mut first = raid(7).to_record();
        let mut second = raid(8).to_record();
        first.remove(&KEY_SAVED_ID.into());
        second.remove(&KEY_SAVED_ID.into());
        let loaded = EventBook::read(&book_of(4, vec![first, second]));
        assert_eq!(loaded.book.ids(), vec![0], "the put is a put: the second wins");
        assert_eq!(loaded.book.get(0).map(|event| event.id), Some(0));
        assert_eq!(loaded.book.len(), 1);
    }

    #[test]
    fn a_save_with_a_list_but_no_counter_hands_out_zero_next() {
        let mut manager = Compound::new();
        manager.insert(
            KEY_EVENT_LIST.into(),
            nbt::compound_list(vec![raid(5).to_record()]),
        );
        let saved = Compound::from([(KEY_EVENT_MANAGER.into(), Tag::Compound(manager))]);
        let mut loaded = EventBook::read(&saved);
        assert_eq!(loaded.book.counter(), 0, "getInt on an absent tag answers 0");
        let id = loaded.book.take_id();
        assert_eq!(id, 0, "so the next id handed out is zero");
        assert_eq!(
            loaded.book.add_event(Event::new(id, "pirate_raid")),
            Err(Refusal::NoId),
            "and addEvent refuses it: the colony cannot start another event"
        );
    }

    #[test]
    fn a_compound_without_event_manager_reads_nothing_at_all() {
        let mut strangers = Compound::new();
        strangers.insert(KEY_EVENT_ID.into(), Tag::Int(41));
        let loaded = EventBook::read(&strangers);
        assert!(loaded.book.is_empty(), ":223 is a guard, not a default");
        assert_eq!(loaded.book.counter(), 1, "the counter is not read either");
        assert_eq!(loaded.book.counter(), FIRST_ID);
    }

    #[test]
    fn the_three_bad_records_are_skipped_by_name_and_cost_only_themselves() {
        let mut manager = Compound::new();
        manager.insert(KEY_EVENT_ID.into(), Tag::Int(3));
        manager.insert(
            KEY_EVENT_LIST.into(),
            Tag::List(vec![
                Tag::Int(12),
                Tag::Compound(Compound::new()),
                Tag::Compound(Compound::from([(
                    KEY_NAME.into(),
                    Tag::String("minecraft:pirate_raid".into()),
                )])),
                Tag::Compound(Compound::from([(
                    KEY_NAME.into(),
                    Tag::String("void_raid".into()),
                )])),
                Tag::Compound(raid(6).to_record()),
            ]),
        );
        let saved = Compound::from([(KEY_EVENT_MANAGER.into(), Tag::Compound(manager))]);

        let loaded = EventBook::read(&saved);
        let names: Vec<&str> = loaded.skipped.iter().map(Skipped::name).collect();
        assert_eq!(
            names,
            ["not_a_compound", "no_name", "not_a_path", "unknown_type"],
            ":229-237 skips each, and the port names it"
        );
        assert_eq!(loaded.skipped[3].index(), 3, "an index a log line can use");
        assert_eq!(loaded.book.ids(), vec![6], "the good record survives");
        assert_eq!(loaded.book.len(), 1);
    }

    #[test]
    fn an_entity_is_bound_to_an_event_or_discarded_and_nothing_is_written() {
        let mut book = EventBook::new();
        assert_eq!(book.add_event(raid(1)), Ok(()));
        let before = book.writes();

        assert_eq!(book.register_entity(1), Route::Registered, ":112");
        assert_eq!(
            book.register_entity(99),
            Route::Discarded,
            ":107-110 discards on an id that names nothing"
        );
        assert_eq!(
            EventBook::new().register_entity(1),
            Route::Discarded,
            "an empty book discards every entity"
        );
        assert_eq!(book.writes(), before, "routing writes nothing");

        assert!(book.unregister_entity(1), ":127");
        assert!(!book.unregister_entity(99), ":124-128 does nothing when absent");
        assert!(book.entity_died(1), ":141-144");
        assert!(!book.entity_died(99));
        assert_eq!(book.tile_entity_broken(1), Ok(()), ":150-154");
        assert_eq!(book.tile_entity_broken(99), Err(Refusal::Unknown(99)));
        assert_eq!(book.night_fall(), vec![1], ":160-163 visits the events it has");
    }

    #[test]
    fn every_registered_type_takes_entities_so_the_discard_is_a_mod_s_branch() {
        assert_eq!(REGISTERED_TYPES.len(), 8, "ModColonyEventTypeInitializer.java:33-40");
        for kind in REGISTERED_TYPES {
            assert!(spawns_entities(kind), "{kind} is an IColonyRaidEvent, so a spawn event");
        }
        assert!(!spawns_entities("void_raid"));
        assert!(is_a_path("norsemen_ship_raid"));
        assert!(!is_a_path("minecraft:pirate_raid"), "a colon cannot be a path");
    }

    #[test]
    fn a_book_round_trips_through_its_own_compound() {
        let mut book = EventBook::new();
        let first = book.take_id();
        assert_eq!(book.add_event(Event::new(first, "pirate_raid")), Ok(()));
        let second = book.take_id();
        assert_eq!(book.add_event(Event::new(second, "norsemen_raid")), Ok(()));

        let loaded = EventBook::read(&book.to_compound());
        assert_eq!(loaded.skipped, vec![]);
        assert_eq!(loaded.book.ids(), book.ids());
        assert_eq!(loaded.book.counter(), book.counter());
        assert_eq!(loaded.book.kind_of(second), Some("norsemen_raid"));
    }
}
