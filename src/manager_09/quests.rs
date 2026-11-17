//! The colony's quest book: which quests a colony may start, which one it holds,
//! how often each was finished, and what a save round-trips.
//!
//! Java reference: `core/quests/QuestManager.java` (367 lines) — the manager
//! itself — plus the two interfaces it *asks* rather than implements:
//! `api/quests/IQuestTemplate.java` (`getMaxOccurrence`, `getParents`,
//! `attemptStart`) and `api/quests/IQuestInstance.java` (`isValid`,
//! `serializeNBT`, `onDeletion`, `onWorldLoad`).
//!
//! | Java | Here |
//! | --- | --- |
//! | `GLOBAL_SERVER_QUESTS` (`IQuestManager.java:22`) | [`Registry`] |
//! | `availableQuests` (`:27`), `inProgressQuests` (`:37`) | [`QuestBook::available`], [`QuestBook::in_progress`] |
//! | `finishedQuests` (`:32`), `getFinishedQuests` (`:343-359`) | [`QuestBook::finished`], [`Finished`] |
//! | `unlockedQuests` (`:42`), `unlockQuest` / `isUnlocked` (`:196-206`) | [`QuestBook::unlock`], [`QuestBook::is_unlocked`] |
//! | `attemptAcceptQuest` (`:70-81`) | [`QuestBook::accept`] |
//! | `completeQuest` (`:97-114`) | [`QuestBook::complete`] |
//! | `onColonyTick` (`:117-170`) | [`QuestBook::tick`] |
//! | `deleteQuest` (`:173-178`) | [`QuestBook::delete`] |
//! | `getAvailableOrInProgressQuest` (`:181-184`) | [`QuestBook::held`] |
//! | `onWorldLoad` (`:187-193`) | [`QuestBook::world_load`] |
//! | `alterReputation` / `getReputation` (`:84-94`) | [`QuestBook::alter_reputation`], [`QuestBook::reputation`] |
//! | `serializeNBT` / `deserializeNBT` (`:209-301`) | [`QuestBook::serialize`], [`QuestBook::read`] |
//! | `serialize` / `deserialize` on a buffer (`:304-322`) | [`QuestBook::take_frame`] |
//! | `injectAvailableQuest` (`:362-366`) | [`QuestBook::inject`] |
//!
//! **Four rules of this class are invisible until someone writes a test for
//! them, and all four are kept.**
//!
//! 1. **The refusal comes before the write, in that order, in both mutators.**
//!    `attemptAcceptQuest` asks the map *first* (`:72`) and `isValid` only after
//!    (`:73`) — Java's `||` short-circuits, so an id that is not available never
//!    has its validity asked. [`QuestBook::accept`] keeps that order, and the test
//!    that holds it gives an actor that panics if it is asked.
//! 2. **A completion wipes the finished cache even when it completed nothing.**
//!    `finishedQuestsCache = null` sits *after* the `if`/`else if`, not inside
//!    them (`:113`), so `completeQuest` on an id the colony never held still drops
//!    the list.
//! 3. **`deleteQuest` marks the colony dirty unconditionally** (`:175-177`): no
//!    check that anything was removed. `unlockQuest` likewise.
//! 4. **The write flag clears whether or not a payload was written.**
//!    `serialize` sets `isDirty = false` outside its own `if` (`:306-311`), so a
//!    clean colony with no new subscriber clears a flag that was already clear and
//!    a dirty one clears it after sending. [`QuestBook::take_frame`] is that
//!    decision without the buffer.
//!
//! **Read and write are not symmetric, and that is Java's rule, not a slip.**
//! `deserializeNBT` drops an available or in-progress instance whose id is not in
//! `GLOBAL_SERVER_QUESTS` (`:258`, `:274`) — the quest's content is gone, so the
//! instance has no template to run — but it keeps a **finished** or **unlocked**
//! id it has never heard of (`:290`, `:298`), because those two are counters and
//! flags, not runnable quests. A save that outlives a removed quest file therefore
//! keeps its history and loses its pending work.
//!
//! **The manager is not a tick of its own.** `Colony.worldTickSlow`
//! (`Colony.java:487`, "Called every 500 ticks") calls `questManager.onColonyTick()`
//! ninth in its list (`:497`), which is [`crate::manager_06::COLONY_TICK`] and not
//! an interval this file owns. Nothing here reads a clock.
//!
//! **What is deliberately not here** (each in `MANQUANT.md`): the six
//! `QuestInstance` fields and their compound — the manager hands an instance's
//! compound through untouched, so [`Instance`] *is* that compound and Java's
//! `serializeNBT()` (`QuestInstance.java:262-287`) stays the instance's job; the
//! `FriendlyByteBuf` the flag guards; and the `Player` argument of
//! `attemptAcceptQuest`, which Java accepts and never reads (`:70-81` — the
//! accepting player is stored by `QuestInstance.onStart`, `:104-106`).
//!
//! **One premise of the brief is false, and it is Java's own comment.** The class
//! has no refusal at all: it answers `false`, or does nothing, and logs nothing.
//! What keeps that harmless is the order, so the port names each cause —
//! [`Refusal::NotAvailable`], [`Refusal::Invalid`], [`Refusal::InstanceWithoutId`]
//! — and proves the store is untouched when one is returned.

use std::collections::BTreeMap;

use mod_api::{Compound, Identifier, Tag};

use super::nbt;

/// Java's `TAG_AVAILABLE` (`NbtTagConstants.java:739`).
pub const KEY_AVAILABLE: &str = "availquests";
/// Java's `TAG_IN_PROGRESS` (`NbtTagConstants.java:738`).
pub const KEY_IN_PROGRESS: &str = "inprogquests";
/// Java's `TAG_FINISHED` (`NbtTagConstants.java:740`).
pub const KEY_FINISHED: &str = "finishedquests";
/// Java's `TAG_UNLOCKED` (`NbtTagConstants.java:748`).
pub const KEY_UNLOCKED: &str = "unlockedquests";
/// Java's `TAG_REPUTATION` (`NbtTagConstants.java:749`).
pub const KEY_REPUTATION: &str = "questreputation";
/// `TAG_ID` (`NbtTagConstants.java:8`), used by three of the four lists.
pub const KEY_ID: &str = "id";
/// `TAG_QUANTITY` (`NbtTagConstants.java:588`), the count of a finished quest.
pub const KEY_QUANTITY: &str = "quantity";
/// Where the colony compound keeps this manager: `quest_manager`
/// (`NbtTagConstants.java:59`, written at `Colony.java:955`, read at `:783`).
pub const KEY_MANAGER: &str = "quest_manager";

/// What a quest template answers the tick: `IQuestTemplate.getMaxOccurrence()`
/// (`IQuestTemplate.java:32`) and `getParents()` (`:66`). The two fields the
/// manager reads; `getQuestTimeout()` (`:25`) is read by `QuestInstance.isValid`
/// (`QuestInstance.java:152`), not here.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Template {
    /// How many times the colony may finish this quest. `0` means never, because
    /// the tick's test is `finished >= max` (`QuestManager.java:123`).
    pub max_occurrence: i32,
    /// Every quest that must be **finished** before this one may start. Unlocked
    /// is not enough (`:129-136`).
    pub parents: Vec<Identifier>,
}

/// `GLOBAL_SERVER_QUESTS` (`IQuestManager.java:22`): the table every colony shares.
///
/// Java keeps it as a `static HashMap` filled by the resource reload. A wasm mod
/// has no global mutable state, so it is passed in — which is also what makes a
/// test say *which* table it is testing against.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Registry {
    templates: BTreeMap<Identifier, Template>,
}

impl Registry {
    pub fn new() -> Registry {
        Registry::default()
    }

    /// `GLOBAL_SERVER_QUESTS.put(id, template)`, returning what was there.
    pub fn insert(&mut self, id: Identifier, template: Template) -> Option<Template> {
        self.templates.insert(id, template)
    }

    pub fn get(&self, id: &Identifier) -> Option<&Template> {
        self.templates.get(id)
    }

    pub fn contains(&self, id: &Identifier) -> bool {
        self.templates.contains_key(id)
    }

    pub fn len(&self) -> usize {
        self.templates.len()
    }

    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }

    /// Every entry, in id order — Java's `HashMap` order is unspecified, and the
    /// tick's spawn loop is written so that the order does not matter (`:119-149`
    /// only ever inserts).
    pub fn iter(&self) -> impl Iterator<Item = (&Identifier, &Template)> {
        self.templates.iter()
    }
}

/// One live quest, which for this manager is **its own compound**.
///
/// `QuestManager` never looks inside it: it writes `getValue().serializeNBT()`
/// (`:216`, `:223`), reads the id back out (`:257`, `:273`) and calls the
/// instance's `isValid`, `onDeletion` and `onWorldLoad`. Keeping the compound
/// verbatim is therefore the faithful shape, and it leaves the seven fields of
/// `QuestInstance` where Java put them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instance {
    data: Compound,
}

impl Instance {
    pub fn new(data: Compound) -> Instance {
        Instance { data }
    }

    /// The compound as the instance wrote it, which is what the manager saves.
    pub fn data(&self) -> &Compound {
        &self.data
    }

    /// `IQuestInstance.getId()` (`IQuestInstance.java`), read out of the compound
    /// where `QuestInstance.serializeNBT` put it (`QuestInstance.java:265`).
    /// `None` for a compound that is not an instance.
    pub fn id(&self) -> Option<Identifier> {
        nbt::text(&self.data, KEY_ID).and_then(|text| Identifier::parse(&text).ok())
    }
}

/// One entry of `getFinishedQuests` (`:343-359`), Java's `FinishedQuest` record
/// (`api/quests/FinishedQuest.java:9`): the template id and how often.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finished {
    pub template: Identifier,
    pub count: i32,
}

/// Why `accept` refused. Java answers `false` for both causes (`:73-76`), which is
/// one answer for two facts.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the two causes of attemptAcceptQuest
pub enum Refusal {
    /// No quest with this id is available to the colony (`:72`).
    NotAvailable { id: Identifier },
    /// The instance exists and `isValid(colony)` said no (`:73`).
    Invalid { id: Identifier },
    /// `injectAvailableQuest` was handed an instance that is not one (`:364`
    /// keys the map by `questInstance.getId()`, which such a compound cannot
    /// answer).
    InstanceWithoutId,
}

/// Which list a completion emptied — Java's two branches (`:99-111`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two branches of completeQuest
pub enum Completion {
    /// A quest the colony had accepted (`:99`).
    InProgress,
    /// A quest it never accepted, closed by the quest itself — Java's comment
    /// says why: "a player short-cut quits a job without accepting it" (`:107`).
    Shortcut,
}

/// Why a `quest_manager` compound could not be read.
///
/// Java throws `ResourceLocationException` out of `deserializeNBT` for all three
/// of these (`:257`, `:273`, `:290`, `:298` all construct a `ResourceLocation`
/// from the stored string), taking the whole colony load with it. The port
/// refuses with the list's name instead, so the caller can say which key broke.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the one cause a read has
pub enum ReadError {
    MalformedId { list: &'static str, index: usize },
}

/// What the manager asks the colony and its quests, reduced to the four calls
/// `QuestManager` makes into other classes.
///
/// All four are Java methods on `IQuestTemplate` / `IQuestInstance`, not on the
/// manager: `attemptStart` (`IQuestTemplate.java:23`), `isValid`
/// (`IQuestInstance.java`), `onDeletion` (`QuestInstance.java:162`) and
/// `onWorldLoad` (`:192`). They need citizens, a day and a registry of objective
/// types, so they come in as an actor and not as world access.
pub trait Actor {
    /// `IQuestTemplate.attemptStart(colony)`: the instance's compound, or `None`
    /// when the colony does not qualify (`:143-144`).
    fn attempt_start(&mut self, template: &Identifier) -> Option<Compound>;

    /// `IQuestInstance.isValid(colony)` (`QuestInstance.java:138-153`): the giver
    /// and every participant still live, and the day is inside
    /// `getQuestTimeout()`.
    fn valid(&mut self, instance: &Instance) -> bool;

    /// `IQuestInstance.onDeletion()` (`QuestInstance.java:162-190`): the citizens
    /// forget the quest and the current objective is cancelled.
    fn deleted(&mut self, instance: &Instance);

    /// `IQuestInstance.onWorldLoad()` (`QuestInstance.java:192-195`).
    fn reloaded(&mut self, instance: &Instance);
}

/// One colony's quest state — Java's `QuestManager`.
#[derive(Clone, Debug)]
pub struct QuestBook {
    /// `availableQuests` (`:27`).
    available: BTreeMap<Identifier, Instance>,
    /// `inProgressQuests` (`:37`).
    in_progress: BTreeMap<Identifier, Instance>,
    /// `finishedQuests` (`:32`), id → count.
    finished: BTreeMap<Identifier, i32>,
    /// `unlockedQuests` (`:42`), a **list**: Java appends without checking (`:198`).
    unlocked: Vec<Identifier>,
    /// `questReputation` (`:52`).
    reputation: f64,
    /// `isDirty` (`:62`), which starts `true` — a manager that has never been
    /// synced owes its clients one frame.
    dirty: bool,
    /// `finishedQuestsCache` (`:47`): `None` means "rebuild on next read".
    cache: Option<Vec<Finished>>,
    /// How many instances a read dropped because their template is gone
    /// (`:258`, `:274`). Not Java's field: Java drops them silently, and the
    /// count is the only way a test can hold that rule.
    dropped: u32,
}

impl Default for QuestBook {
    fn default() -> QuestBook {
        QuestBook::new()
    }
}

impl QuestBook {
    /// A fresh manager. `isDirty = true` is the field's initialiser (`:62`), not a
    /// write of the constructor (`:64-67` only stores the colony).
    pub fn new() -> QuestBook {
        QuestBook {
            available: BTreeMap::new(),
            in_progress: BTreeMap::new(),
            finished: BTreeMap::new(),
            unlocked: Vec::new(),
            reputation: 0.0,
            dirty: true,
            cache: None,
            dropped: 0,
        }
    }

    /// Whether the colony owes its clients a frame (`isDirty`).
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// `markDirty()` (`:325-328`).
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// How many instances the last [`QuestBook::read`] dropped.
    pub fn dropped(&self) -> u32 {
        self.dropped
    }

    pub fn available(&self) -> impl Iterator<Item = &Instance> {
        self.available.values()
    }

    pub fn in_progress(&self) -> impl Iterator<Item = &Instance> {
        self.in_progress.values()
    }

    /// `attemptAcceptQuest` (`:70-81`). The `Player` argument of Java's signature
    /// is not taken: the method never reads it.
    ///
    /// Refusal order is Java's: availability first (`:72`), validity second
    /// (`:73`). Nothing is written before both have answered.
    pub fn accept<A: Actor>(&mut self, id: &Identifier, actor: &mut A) -> Result<(), Refusal> {
        let Some(instance) = self.available.get(id) else {
            return Err(Refusal::NotAvailable { id: id.clone() });
        };
        if !actor.valid(instance) {
            return Err(Refusal::Invalid { id: id.clone() });
        }
        // Java puts into `inProgressQuests` and then removes from
        // `availableQuests` (`:77-78`); both maps hold the same object, so one
        // move is the same state with no observable order.
        let Some(instance) = self.available.remove(id) else {
            return Err(Refusal::NotAvailable { id: id.clone() });
        };
        self.in_progress.insert(id.clone(), instance);
        self.dirty = true;
        Ok(())
    }

    /// `completeQuest` (`:97-114`), called **by** the quest, not by a player.
    ///
    /// The finished cache is dropped on every call (`:113`), including the one
    /// that completed nothing.
    pub fn complete(&mut self, id: &Identifier) -> Option<Completion> {
        let how = if self.in_progress.remove(id).is_some() {
            Some(Completion::InProgress)
        } else if self.available.remove(id).is_some() {
            Some(Completion::Shortcut)
        } else {
            None
        };
        if how.is_some() {
            let count = self.finished.entry(id.clone()).or_insert(0);
            // Java's `getOrDefault(..) + 1` on an `int` wraps at `MAX_VALUE`;
            // a colony cannot reach that, and saturating says so without a panic.
            *count = count.saturating_add(1);
            self.dirty = true;
        }
        self.cache = None;
        how
    }

    /// `onColonyTick` (`:117-170`), in Java's three passes and in Java's order.
    ///
    /// 1. **Start**: every template not already held and under its occurrence
    ///    limit whose parents are all **finished** is offered to
    ///    `attemptStart` (`:119-149`).
    /// 2. **Purge available** (`:151-159`).
    /// 3. **Purge in progress** (`:161-169`).
    ///
    /// Pass 1 running before the purges is a rule, not an accident: a quest
    /// started in pass 1 can be invalid by pass 2 and is then deleted in the same
    /// tick.
    ///
    /// **Each purge asks and writes one quest at a time**, which is the body of
    /// Java's loop and not an optimisation detail: `isValid` (`:153`), then
    /// `onDeletion` (`:155`), then the removal (`:156`), then `markDirty`
    /// (`:157`) — for `q:a1` before `q:a2` is even looked at. `new
    /// ArrayList<>(..)` (`:151`, `:161`) copies because Java removes while it
    /// iterates; the copied key list is the same thing here.
    pub fn tick<A: Actor>(&mut self, registry: &Registry, actor: &mut A) {
        for (id, template) in registry.iter() {
            // `availableQuests.containsKey || inProgressQuests.containsKey ||
            // finished >= max` (`:121-124`) — one `continue` for three causes.
            if self.available.contains_key(id) || self.in_progress.contains_key(id) {
                continue;
            }
            if self.finished.get(id).copied().unwrap_or(0) >= template.max_occurrence {
                continue;
            }
            // A single missing parent skips the quest, and the loop breaks at the
            // first one (`:128-141`).
            if template
                .parents
                .iter()
                .any(|parent| !self.finished.contains_key(parent))
            {
                continue;
            }
            if let Some(data) = actor.attempt_start(id) {
                // Keyed by the *template* id, as Java keys it (`:146`) — not by
                // the id inside the compound. `read` re-keys by the compound's id
                // (`:262`), so the two can only disagree on a hand-edited save.
                self.available.insert(id.clone(), Instance::new(data));
                self.dirty = true;
            }
        }

        // `new ArrayList<>(availableQuests.entrySet())` (`:151`).
        for id in self.available.keys().cloned().collect::<Vec<_>>() {
            // `!containsKey(..) || !isValid(..)` (`:153`) — the registry first, so
            // a quest whose template was removed never has its validity asked.
            let doomed = !registry.contains(&id)
                || match self.available.get(&id) {
                    Some(instance) => !actor.valid(instance),
                    None => false,
                };
            if !doomed {
                continue;
            }
            if let Some(instance) = self.available.get(&id) {
                // `onDeletion` runs before the removal (`:155-156`).
                actor.deleted(instance);
            }
            self.available.remove(&id);
            self.dirty = true;
        }
        // The same body over the other map (`:161-169`).
        for id in self.in_progress.keys().cloned().collect::<Vec<_>>() {
            let doomed = !registry.contains(&id)
                || match self.in_progress.get(&id) {
                    Some(instance) => !actor.valid(instance),
                    None => false,
                };
            if !doomed {
                continue;
            }
            if let Some(instance) = self.in_progress.get(&id) {
                actor.deleted(instance);
            }
            self.in_progress.remove(&id);
            self.dirty = true;
        }
    }

    /// `deleteQuest` (`:173-178`): both maps, then `markDirty` — **always**,
    /// even when the colony held neither.
    pub fn delete(&mut self, id: &Identifier) {
        self.available.remove(id);
        self.in_progress.remove(id);
        self.dirty = true;
    }

    /// `getAvailableOrInProgressQuest` (`:181-184`): available wins the tie.
    pub fn held(&self, id: &Identifier) -> Option<&Instance> {
        self.available
            .get(id)
            .or_else(|| self.in_progress.get(id))
    }

    /// `onWorldLoad` (`:187-193`): in-progress only, and nothing is marked.
    pub fn world_load<A: Actor>(&self, actor: &mut A) {
        for instance in self.in_progress.values() {
            actor.reloaded(instance);
        }
    }

    /// `unlockQuest` (`:196-200`): appended, never deduplicated, always dirty.
    pub fn unlock(&mut self, id: Identifier) {
        self.unlocked.push(id);
        self.dirty = true;
    }

    /// `isUnlocked` (`:203-206`).
    pub fn is_unlocked(&self, id: &Identifier) -> bool {
        self.unlocked.contains(id)
    }

    /// `injectAvailableQuest` (`:362-366`): no validity check, keyed by the
    /// instance's own id, always dirty.
    pub fn inject(&mut self, instance: Instance) -> Result<(), Refusal> {
        let Some(id) = instance.id() else {
            return Err(Refusal::InstanceWithoutId);
        };
        self.available.insert(id, instance);
        self.dirty = true;
        Ok(())
    }

    /// `alterReputation` (`:84-88`): an addition, never a clamp.
    pub fn alter_reputation(&mut self, difference: f64) {
        self.reputation += difference;
        self.dirty = true;
    }

    /// `getReputation` (`:91-94`).
    pub fn reputation(&self) -> f64 {
        self.reputation
    }

    /// `getFinishedQuests` (`:343-359`), rebuilt at most once between writes.
    ///
    /// Only ids the registry still knows are listed (`:350-354`): a finished
    /// quest whose file was removed vanishes from the log but stays counted in
    /// the save, because `read` keeps it (`:290`).
    pub fn finished(&mut self, registry: &Registry) -> &[Finished] {
        if self.cache.is_none() {
            let list = self
                .finished
                .iter()
                .filter(|(id, _)| registry.contains(id))
                .map(|(id, count)| Finished {
                    template: id.clone(),
                    count: *count,
                })
                .collect();
            self.cache = Some(list);
        }
        self.cache.as_deref().unwrap_or(&[])
    }

    /// Whether the finished cache is currently built — the fact rule 2 above is
    /// about.
    pub fn cached(&self) -> bool {
        self.cache.is_some()
    }

    /// `serializeNBT` (`:209-248`): the five keys, in Java's insertion order
    /// (a `BTreeMap` then sorts them, which no reader can see).
    pub fn serialize(&self) -> Compound {
        let mut manager = Compound::new();
        manager.insert(
            KEY_AVAILABLE.into(),
            nbt::compound_list(self.available.values().map(|i| i.data.clone()).collect()),
        );
        manager.insert(
            KEY_IN_PROGRESS.into(),
            nbt::compound_list(self.in_progress.values().map(|i| i.data.clone()).collect()),
        );
        manager.insert(
            KEY_FINISHED.into(),
            nbt::compound_list(
                self.finished
                    .iter()
                    .map(|(id, count)| {
                        Compound::from([
                            (KEY_ID.into(), Tag::String(id.to_string().as_str().into())),
                            (KEY_QUANTITY.into(), Tag::Int(*count)),
                        ])
                    })
                    .collect(),
            ),
        );
        manager.insert(
            KEY_UNLOCKED.into(),
            nbt::compound_list(
                self.unlocked
                    .iter()
                    .map(|id| {
                        Compound::from([(KEY_ID.into(), Tag::String(id.to_string().as_str().into()))])
                    })
                    .collect(),
            ),
        );
        manager.insert(KEY_REPUTATION.into(), Tag::Double(self.reputation.to_bits()));
        manager
    }

    /// `deserializeNBT` (`:251-301`).
    ///
    /// Java is called on a live manager and reuses an existing instance when the
    /// id is already held (`:260`, `:276`); this reads into a **fresh** book,
    /// which is the state a colony load starts from. The difference only shows on
    /// a re-read of a colony that already ticked, and Java's own reuse exists so
    /// that the instance's live objectives are not replaced mid-flight.
    pub fn read(data: &Compound, registry: &Registry) -> Result<QuestBook, ReadError> {
        let mut book = QuestBook::new();

        for (index, record) in nbt::compounds(data, KEY_AVAILABLE).into_iter().enumerate() {
            let id = read_id(record, KEY_AVAILABLE, index)?;
            if registry.contains(&id) {
                book.available.insert(id, Instance::new(record.clone()));
            } else {
                // `if (GLOBAL_SERVER_QUESTS.containsKey(key))` (`:258`) — else the
                // instance is dropped, and Java says nothing about it.
                book.dropped += 1;
            }
        }
        for (index, record) in nbt::compounds(data, KEY_IN_PROGRESS).into_iter().enumerate() {
            let id = read_id(record, KEY_IN_PROGRESS, index)?;
            if registry.contains(&id) {
                book.in_progress.insert(id, Instance::new(record.clone()));
            } else {
                book.dropped += 1;
            }
        }

        // Finished and unlocked take any id, known or not (`:290`, `:298`).
        book.finished.clear();
        for (index, record) in nbt::compounds(data, KEY_FINISHED).into_iter().enumerate() {
            let id = read_id(record, KEY_FINISHED, index)?;
            // `getInt` answers 0 for an absent key (`:290`).
            let count = nbt::int(record, KEY_QUANTITY).unwrap_or(0);
            book.finished.insert(id, count);
        }

        book.unlocked.clear();
        for (index, record) in nbt::compounds(data, KEY_UNLOCKED).into_iter().enumerate() {
            let id = read_id(record, KEY_UNLOCKED, index)?;
            book.unlocked.push(id);
        }

        // `getDouble` answers 0.0 for an absent key (`:300`).
        book.reputation = nbt::double(data, KEY_REPUTATION).unwrap_or(0.0);
        // `finishedQuestsCache = null` (`:292`); the cache starts empty anyway.
        book.cache = None;
        Ok(book)
    }

    /// The decision `serialize(FriendlyByteBuf, boolean)` makes (`:304-312`):
    /// a payload when the colony is dirty **or** a subscriber just arrived, and
    /// the flag cleared in all three cases.
    ///
    /// The buffer itself is `MANQUANT.md` §1; this is the half a test can hold.
    pub fn take_frame(&mut self, has_new_subscribers: bool) -> Option<Compound> {
        let send = self.dirty || has_new_subscribers;
        let payload = if send { Some(self.serialize()) } else { None };
        self.dirty = false;
        payload
    }
}

fn read_id(record: &Compound, list: &'static str, index: usize) -> Result<Identifier, ReadError> {
    let text = nbt::text(record, KEY_ID).ok_or(ReadError::MalformedId { list, index })?;
    // `getString` answers `""` for an absent key, and vanilla's `isValidPath`
    // accepts the empty path (it iterates nothing), so Java would carry
    // `minecraft:` — an id no lookup can ever match. Refused rather than carried.
    if text.is_empty() {
        return Err(ReadError::MalformedId { list, index });
    }
    Identifier::parse(&text).map_err(|_| ReadError::MalformedId { list, index })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn id(text: &str) -> Identifier {
        Identifier::parse(text).expect("a literal is a valid id")
    }

    fn instance(text: &str) -> Compound {
        Compound::from([(KEY_ID.into(), Tag::String(text.into()))])
    }

    /// An actor that counts what it is asked and answers from two sets.
    #[derive(Default)]
    struct Fake {
        /// Templates that will start, and the id they start under.
        startable: BTreeSet<Identifier>,
        /// Instances whose `isValid` says no.
        invalid: BTreeSet<Identifier>,
        journal: Vec<String>,
    }

    impl Fake {
        fn starting(names: &[&str]) -> Fake {
            Fake {
                startable: names.iter().map(|name| id(name)).collect(),
                ..Fake::default()
            }
        }

        fn refusing(names: &[&str]) -> Fake {
            Fake {
                invalid: names.iter().map(|name| id(name)).collect(),
                ..Fake::default()
            }
        }
    }

    impl Actor for Fake {
        fn attempt_start(&mut self, template: &Identifier) -> Option<Compound> {
            self.journal.push(format!("start {template}"));
            self.startable
                .contains(template)
                .then(|| instance(&template.to_string()))
        }

        fn valid(&mut self, instance: &Instance) -> bool {
            match instance.id() {
                Some(id) => {
                    self.journal.push(format!("valid {id}"));
                    !self.invalid.contains(&id)
                }
                None => false,
            }
        }

        fn deleted(&mut self, instance: &Instance) {
            self.journal.push(format!(
                "delete {}",
                instance.id().map(|id| id.to_string()).unwrap_or_default()
            ));
        }

        fn reloaded(&mut self, instance: &Instance) {
            self.journal.push(format!(
                "reload {}",
                instance.id().map(|id| id.to_string()).unwrap_or_default()
            ));
        }
    }

    /// An actor that panics on every question, for the short-circuit tests.
    struct Blind;

    impl Actor for Blind {
        fn attempt_start(&mut self, _: &Identifier) -> Option<Compound> {
            panic!("a skipped template is not offered")
        }

        fn valid(&mut self, _: &Instance) -> bool {
            panic!("an unavailable quest has its validity asked only after the map")
        }

        fn deleted(&mut self, _: &Instance) {
            panic!("nothing is held, so nothing is deleted")
        }

        fn reloaded(&mut self, _: &Instance) {
            panic!("nothing is held, so nothing is reloaded")
        }
    }

    fn registry(names: &[(&str, i32, &[&str])]) -> Registry {
        let mut registry = Registry::new();
        for (name, max, parents) in names {
            registry.insert(
                id(name),
                Template {
                    max_occurrence: *max,
                    parents: parents.iter().map(|parent| id(parent)).collect(),
                },
            );
        }
        registry
    }

    fn book_with(available: &[&str], in_progress: &[&str]) -> QuestBook {
        let mut book = QuestBook::new();
        book.dirty = false;
        for name in available {
            book.available.insert(id(name), Instance::new(instance(name)));
        }
        for name in in_progress {
            book.in_progress.insert(id(name), Instance::new(instance(name)));
        }
        book
    }

    #[test]
    fn a_fresh_book_owes_a_frame_and_a_read_book_still_does() {
        let registry = registry(&[]);
        assert!(QuestBook::new().is_dirty(), "isDirty starts true (`:62`)");
        let read = QuestBook::read(&Compound::new(), &registry).expect("an empty compound reads");
        assert!(
            read.is_dirty(),
            "Java's field initialiser runs before deserializeNBT, which never clears it"
        );
    }

    #[test]
    fn accept_refuses_an_unavailable_id_without_asking_whether_it_is_valid() {
        let mut book = book_with(&[], &["q:held"]);
        assert_eq!(
            book.accept(&id("q:absent"), &mut Blind),
            Err(Refusal::NotAvailable { id: id("q:absent") })
        );
        assert!(!book.is_dirty(), "a refused accept writes nothing");
        assert_eq!(book.in_progress().count(), 1);
    }

    #[test]
    fn accept_refuses_an_invalid_instance_and_writes_nothing() {
        let mut book = book_with(&["q:a"], &[]);
        let mut actor = Fake::refusing(&["q:a"]);
        assert_eq!(
            book.accept(&id("q:a"), &mut actor),
            Err(Refusal::Invalid { id: id("q:a") })
        );
        assert!(!book.is_dirty());
        assert_eq!(book.available().count(), 1, "still available");
        assert_eq!(book.in_progress().count(), 0);
    }

    #[test]
    fn accept_moves_the_one_instance_and_marks_the_colony() {
        let mut book = book_with(&["q:a"], &[]);
        assert_eq!(book.accept(&id("q:a"), &mut Fake::default()), Ok(()));
        assert!(book.is_dirty());
        assert_eq!(book.available().count(), 0);
        assert_eq!(book.in_progress().count(), 1);
    }

    #[test]
    fn a_completion_counts_from_either_map_and_an_unknown_id_counts_nothing() {
        let mut book = book_with(&["q:available"], &["q:running"]);
        assert_eq!(
            book.complete(&id("q:running")),
            Some(Completion::InProgress)
        );
        assert_eq!(
            book.complete(&id("q:available")),
            Some(Completion::Shortcut)
        );
        assert_eq!(book.complete(&id("q:unknown")), None);
        let registry = registry(&[("q:running", 1, &[]), ("q:available", 1, &[])]);
        assert_eq!(book.finished(&registry).len(), 2, "one each");
        assert_eq!(book.in_progress().count(), 0);
        assert_eq!(book.available().count(), 0);
    }

    #[test]
    fn a_completion_drops_the_finished_cache_even_when_it_completed_nothing() {
        let mut book = book_with(&[], &[]);
        let registry = registry(&[]);
        book.finished(&registry);
        assert!(book.cached());
        book.complete(&id("q:absent"));
        assert!(
            !book.cached(),
            "`finishedQuestsCache = null` sits outside the if (`:113`)"
        );
    }

    #[test]
    fn the_finished_list_names_only_templates_the_registry_knows() {
        let mut book = book_with(&[], &["q:a", "q:gone"]);
        book.complete(&id("q:a"));
        book.complete(&id("q:gone"));
        let listed = book.finished(&registry(&[("q:a", 1, &[])]));
        assert_eq!(
            listed,
            &[Finished {
                template: id("q:a"),
                count: 1
            }][..]
        );
    }

    #[test]
    fn delete_marks_the_colony_even_when_the_quest_was_in_neither_map() {
        let mut book = book_with(&[], &[]);
        book.delete(&id("q:absent"));
        assert!(book.is_dirty(), "no check that anything was removed (`:175-177`)");
    }

    #[test]
    fn unlock_appends_without_deduplicating() {
        let mut book = book_with(&[], &[]);
        book.unlock(id("q:a"));
        book.unlock(id("q:a"));
        assert!(book.is_unlocked(&id("q:a")));
        assert_eq!(book.unlocked.len(), 2, "`unlockedQuests.add` does not check (`:198`)");
    }

    #[test]
    fn reputation_is_an_addition_and_marks_the_colony() {
        let mut book = book_with(&[], &[]);
        book.alter_reputation(2.5);
        book.alter_reputation(-10.0);
        assert_eq!(book.reputation(), -7.5);
        assert!(book.is_dirty());
    }

    #[test]
    fn inject_keys_by_the_instances_own_id_and_refuses_a_compound_without_one() {
        let mut book = book_with(&[], &[]);
        assert_eq!(
            book.inject(Instance::new(Compound::new())),
            Err(Refusal::InstanceWithoutId)
        );
        assert!(!book.is_dirty(), "the refusal came before the write");
        assert_eq!(book.inject(Instance::new(instance("q:tutorial"))), Ok(()));
        assert!(book.held(&id("q:tutorial")).is_some());
        assert!(book.is_dirty());
    }

    #[test]
    fn tick_skips_available_held_and_reached_max_occurrence() {
        let registry = registry(&[("q:available", 5, &[]), ("q:held", 5, &[]), ("q:done", 1, &[])]);
        let mut book = book_with(&["q:available"], &["q:held"]);
        book.finished.insert(id("q:done"), 1);
        let mut actor = Fake::starting(&["q:available", "q:held", "q:done"]);
        book.tick(&registry, &mut actor);
        assert_eq!(
            actor.journal,
            [
                // The spawn loop offers nothing — it never reaches
                // `attemptStart` — but the two purge loops still ask every held
                // quest whether it is valid (`:153`, `:163`). A skip in pass 1
                // does not skip pass 2.
                "valid q:available".to_owned(),
                "valid q:held".to_owned()
            ],
            "no template is started"
        );
    }

    #[test]
    fn tick_waits_for_a_finished_parent_and_not_for_an_unlocked_one() {
        let registry = registry(&[("q:child", 1, &["q:parent"])]);
        let mut book = book_with(&[], &[]);
        book.unlock(id("q:parent"));
        let mut actor = Fake::starting(&["q:child"]);
        book.tick(&registry, &mut actor);
        assert!(book.available().count() == 0, "unlocked is not finished (`:131`)");

        book.finished.insert(id("q:parent"), 1);
        book.tick(&registry, &mut actor);
        assert_eq!(book.available().count(), 1, "a finished parent opens the child");
    }

    #[test]
    fn tick_offers_every_template_it_does_not_skip() {
        let registry = registry(&[("q:a", 2, &[]), ("q:b", 2, &[])]);
        let mut book = book_with(&[], &[]);
        let mut actor = Fake::starting(&["q:a"]);
        book.tick(&registry, &mut actor);
        assert_eq!(
            actor.journal,
            [
                // BTreeMap order: `q:a` before `q:b`.
                "start q:a".to_owned(),
                "start q:b".to_owned(),
                // `q:a` was inserted by pass 1, and pass 2 asks every available
                // quest `isValid` (`:153`) in the same tick.
                "valid q:a".to_owned()
            ],
            "the template that answers None starts nothing"
        );
        assert_eq!(book.available().count(), 1);
        assert!(book.is_dirty());
    }

    #[test]
    fn tick_purges_available_then_in_progress_and_announces_before_removing() {
        let registry = registry(&[
            ("q:a1", 1, &[]),
            ("q:a2", 1, &[]),
            ("q:p1", 1, &[]),
            ("q:p2", 1, &[]),
        ]);
        let mut book = book_with(&["q:a1", "q:a2"], &["q:p1", "q:p2"]);
        let mut actor = Fake::refusing(&["q:a1", "q:a2", "q:p1", "q:p2"]);
        book.tick(&registry, &mut actor);
        assert_eq!(
            actor.journal,
            [
                "valid q:a1".to_owned(),
                "delete q:a1".to_owned(),
                "valid q:a2".to_owned(),
                "delete q:a2".to_owned(),
                "valid q:p1".to_owned(),
                "delete q:p1".to_owned(),
                "valid q:p2".to_owned(),
                "delete q:p2".to_owned(),
            ]
        );
        assert_eq!(book.available().count(), 0);
        assert_eq!(book.in_progress().count(), 0);
    }

    #[test]
    fn a_quest_started_in_pass_one_can_be_purged_by_pass_two_of_the_same_tick() {
        let registry = registry(&[("q:stillborn", 1, &[])]);
        let mut book = book_with(&[], &[]);
        let mut actor = Fake {
            startable: [id("q:stillborn")].into_iter().collect(),
            invalid: [id("q:stillborn")].into_iter().collect(),
            journal: Vec::new(),
        };
        book.tick(&registry, &mut actor);
        assert_eq!(
            actor.journal,
            [
                "start q:stillborn".to_owned(),
                "valid q:stillborn".to_owned(),
                "delete q:stillborn".to_owned()
            ],
            "the spawn loop runs before both purges (`:119-169`)"
        );
        assert_eq!(book.available().count(), 0);
    }

    #[test]
    fn a_quest_dropped_from_the_registry_is_purged_without_asking_validity() {
        let mut book = book_with(&["q:orphan"], &[]);
        let mut actor = Fake::default();
        book.tick(&Registry::new(), &mut actor);
        assert_eq!(
            actor.journal,
            ["delete q:orphan".to_owned()],
            "the registry test short-circuits `isValid` (`:153`)"
        );
    }

    #[test]
    fn the_compound_round_trips_under_javas_five_keys() {
        let registry = registry(&[("q:a", 2, &[]), ("q:b", 2, &[])]);
        let mut book = book_with(&["q:a"], &["q:b"]);
        book.finished.insert(id("q:a"), 3);
        book.unlock(id("q:b"));
        book.alter_reputation(-4.25);
        let data = book.serialize();
        for key in [KEY_AVAILABLE, KEY_IN_PROGRESS, KEY_FINISHED, KEY_UNLOCKED, KEY_REPUTATION] {
            assert!(data.contains_key(&key.into()), "{key} is written");
        }

        let read = QuestBook::read(&data, &registry).expect("its own compound reads");
        assert_eq!(read.available().count(), 1);
        assert_eq!(read.in_progress().count(), 1);
        assert_eq!(read.reputation(), -4.25);
        assert!(read.is_unlocked(&id("q:b")));
        let mut read = read;
        assert_eq!(read.finished(&registry).len(), 1);
        assert_eq!(read.finished(&registry)[0].count, 3);
    }

    #[test]
    fn read_drops_unknown_instances_but_keeps_finished_and_unlocked_ids() {
        let mut data = Compound::new();
        data.insert(
            KEY_AVAILABLE.into(),
            nbt::compound_list(vec![instance("q:gone"), instance("q:kept")]),
        );
        data.insert(
            KEY_IN_PROGRESS.into(),
            nbt::compound_list(vec![instance("q:gone")]),
        );
        data.insert(
            KEY_FINISHED.into(),
            nbt::compound_list(vec![Compound::from([
                (KEY_ID.into(), Tag::String("q:gone".into())),
                (KEY_QUANTITY.into(), Tag::Int(4)),
            ])]),
        );
        data.insert(
            KEY_UNLOCKED.into(),
            nbt::compound_list(vec![Compound::from([(
                KEY_ID.into(),
                Tag::String("q:gone".into()),
            )])]),
        );

        let book = QuestBook::read(&data, &registry(&[("q:kept", 1, &[])])).expect("reads");
        assert_eq!(book.available().count(), 1, "only the known instance survives");
        assert_eq!(book.in_progress().count(), 0);
        assert_eq!(book.dropped(), 2, "the two unknown instances are counted");
        assert!(book.is_unlocked(&id("q:gone")), "a flag survives its quest file");
        assert_eq!(
            book.serialize().get(&KEY_FINISHED.into()).cloned(),
            Some(nbt::compound_list(vec![Compound::from([
                (KEY_ID.into(), Tag::String("q:gone".into())),
                (KEY_QUANTITY.into(), Tag::Int(4)),
            ])])),
            "a counter survives its quest file"
        );
    }

    #[test]
    fn read_refuses_a_malformed_id_and_names_the_list() {
        let mut data = Compound::new();
        data.insert(
            KEY_FINISHED.into(),
            nbt::compound_list(vec![Compound::from([(
                KEY_ID.into(),
                Tag::String("".into()),
            )])]),
        );
        match QuestBook::read(&data, &registry(&[])) {
            Err(error) => assert_eq!(
                error,
                ReadError::MalformedId {
                    list: KEY_FINISHED,
                    index: 0
                },
                "the refusal names which list broke"
            ),
            Ok(_) => panic!("a malformed id is refused, not dropped"),
        }
    }

    #[test]
    fn a_frame_is_dirty_or_new_subscriber_and_the_flag_always_clears() {
        let mut dirty = QuestBook::new();
        assert!(dirty.take_frame(false).is_some(), "a fresh book sends");
        assert!(!dirty.is_dirty());

        let mut clean = book_with(&[], &[]);
        assert!(
            clean.take_frame(true).is_some(),
            "a new subscriber gets a full frame"
        );
        assert!(!clean.is_dirty());

        assert!(
            clean.take_frame(false).is_none(),
            "clean and nobody new: no payload"
        );
        assert!(!clean.is_dirty(), "and the flag is cleared anyway (`:311`)");

        dirty.mark_dirty();
        assert!(dirty.take_frame(false).is_some());
    }

    #[test]
    fn available_wins_over_in_progress_for_the_same_id() {
        // Java's ternary (`:183`); the state is reachable through `inject` after
        // an `accept`, which is what the tutorial injection can do.
        let mut book = book_with(&[], &["q:tie"]);
        book.inject(Instance::new(instance("q:tie"))).expect("an id");
        let held = book.held(&id("q:tie")).expect("one of the two");
        assert_eq!(held.id(), Some(id("q:tie")));
        assert_eq!(book.available().count(), 1);
        assert_eq!(book.in_progress().count(), 1);
    }

    #[test]
    fn world_load_touches_only_the_quests_in_progress() {
        let book = book_with(&["q:available"], &["q:running"]);
        let mut actor = Fake::default();
        book.world_load(&mut actor);
        assert_eq!(actor.journal, ["reload q:running".to_owned()]);
        assert!(!book.is_dirty(), "`onWorldLoad` marks nothing (`:187-193`)");
    }
}
