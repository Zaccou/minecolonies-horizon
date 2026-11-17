//! The colony's managed animals: who is registered, which id is reused, what a
//! re-registration does with a duplicate, and what a save holds.
//!
//! Java reference: `core/colony/managers/AnimalManager.java` (447 lines) and
//! `core/colony/AnimalData.java` (410 lines), with the colony's part in
//! `core/colony/Colony.java:772` and `:940-942`.
//!
//! **What is here.** The registry and its two decisions: which animal an id may
//! attach to ([`Registry::register`]), and which id a new animal takes
//! ([`Registry::create`]) — plus the save, the two lookups
//! ([`Registry::animals_of`], [`Registry::get`]) and the packet plan
//! ([`Registry::send_plan`]). Everything else in the class is either a
//! `WeakReference` to a live entity (there is no entity here) or a
//! `Network.getNetwork()` call, which is the caller's.
//!
//! **The save is broken in Java, and the port does not inherit the break.** The
//! nesting is off by one on both sides, which a reader can check in three lines:
//!
//! | Where | What it does |
//! | --- | --- |
//! | `AnimalManager.write:347-357` | puts `animalManager` **inside** the compound it is given |
//! | `Colony.write:940-942` | gives it an empty compound, then puts the result under `animalManager` again |
//! | `Colony.read:772` | hands `read` the **inner** compound: `compound.getCompound(TAG_ANIMAL_MANAGER)` |
//! | `AnimalManager.read:280` | then looks for `animalManager` inside it — and returns, because it is not there |
//!
//! So a colony's file holds `animalManager: {animalManager: {animals, nextID}}`,
//! and the reader looks for `{animals, nextID}` one level below where the writer
//! left it: **no animal ever survives a restart**, and the early return at
//! `:280-283` is what makes the loss silent. [`Registry::write`] and
//! [`Registry::read`] both speak the flat shape `AnimalManager.read` was written
//! for — the colony compound, one `animalManager` key, `animals` and `nextID`
//! inside — and a test below round-trips it. Fixing Java would be a one-line
//! change on either side; porting the break would lose the data twice.
//!
//! **Two javadoc claims the code contradicts**, both kept as the code has them:
//!
//! 1. `registerAnimal`'s doc (`:70`) says the *existing* animal is removed "if it
//!    is alive". `:116` tests `animalEntity.isAlive()` on the **newcomer**: a dead
//!    newcomer is discarded and the bound one stays ([`Discard::NotAlive`]).
//! 2. `createAndRegisterAnimalData`'s loop (`:238-246`) can never fail to find a
//!    free id — `count + 1` slots for `count` entries, so one is free — which
//!    means the `nextAnimalID` left over from a previous call can never be handed
//!    out twice. The port says the proof instead of keeping the dead fallback.
//!
//! **A UUID that moves on every save.** `AnimalData.serializeNBT:155` writes
//! `uuid != null ? uuid : UUID.randomUUID()`: an animal whose UUID was never set
//! gets a **new** one each time the colony is written, so the record and the
//! entity can disagree after any save. The port writes no `uuid` key at all when
//! it does not know one ([`Entry::uuid`] is `Option`), and a read reports
//! [`Repair::NoUuid`] instead of minting one.

use std::collections::BTreeMap;

use mod_api::{Compound, Tag};

/// The colony-level key (`NbtTagConstants.java:67`).
pub const KEY_ANIMAL_MANAGER: &str = "animalManager";
/// The two keys inside it (`AnimalManager.java:37-39`, `public static String`,
/// not `final`).
pub const KEY_ANIMALS: &str = "animals";
pub const KEY_NEXT_ID: &str = "nextID";

/// One record's keys (`AnimalData.java:149-160`, `NbtTagConstants.java:8`,
/// `:43`, `:51`, `:284`, `:776-779`).
pub const KEY_ID: &str = "id";
pub const KEY_UUID: &str = "uuid";
pub const KEY_POS: &str = "pos";
pub const KEY_HOME: &str = "animalhome";
pub const KEY_MAX_HEALTH: &str = "maxhealth";
pub const KEY_COMBAT_COOLDOWN: &str = "combatcooldown";
pub const KEY_OWNER: &str = "owner";

/// The first id a colony hands out (`AnimalManager.java:232`).
pub const FIRST_ANIMAL_ID: i32 = 1;

/// A UUID, as NBT stores one: four ints (`CompoundTag.putUUID` writes an
/// `IntArrayTag` of four). Kept in that shape so a comparison needs no
/// conversion and a read cannot invent one.
pub type Uuid = [i32; 4];

/// One animal's record, the part of `AnimalData` that is data.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: i32,
    /// `AnimalData.uuid` (`:34`); `None` when the save never carried one.
    pub uuid: Option<Uuid>,
    /// `AnimalData.homeBuilding` (`:45`), kept as the building's position: a
    /// zero position is **no home**, in Java and here (`AnimalData.java:175-180`).
    pub home: Option<[i32; 3]>,
    /// `AnimalData.lastPosition` (`:70`).
    pub last_position: [i32; 3],
    /// `AnimalData.maxHealth` (`:60`), an `f32` in Java and here.
    pub max_health: f32,
    /// `AnimalData.combatCooldown` (`:50`), clamped to `[0, maxHealth]` by
    /// `setCombatCooldown` (`:404-408`).
    pub combat_cooldown: f32,
    /// `AnimalData.owner` (`:55`).
    pub owner: Option<Uuid>,
    /// Whether an entity is currently bound (`entity` is non-null, `:76`).
    pub bound: bool,
    /// `AnimalData.isDirty` (`:65`), which the packet plan reads.
    pub dirty: bool,
}

impl Entry {
    /// A record with nothing but its id and its colony (`:84-88`).
    pub fn new(id: i32) -> Entry {
        Entry {
            id,
            uuid: None,
            home: None,
            last_position: [0, 0, 0],
            max_health: 0.0,
            combat_cooldown: 0.0,
            owner: None,
            bound: false,
            dirty: false,
        }
    }
}

/// The animal as the world hands it to [`Registry::register`]: `IManagedAnimal`
/// and its entity, cut to the three facts the decision reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Managed {
    pub id: i32,
    pub uuid: Uuid,
    pub alive: bool,
    /// `existingManagedAnimal.get() == animal` (`:111`): Java compares object
    /// identity, which a value cannot. The caller is the one that knows.
    pub same_as_bound: bool,
}

impl Managed {
    pub fn new(id: i32, uuid: Uuid, alive: bool) -> Managed {
        Managed {
            id,
            uuid,
            alive,
            same_as_bound: false,
        }
    }
}

/// What a registration did, or why it refused the animal (`:74-130`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four exits of `registerAnimal`, and the reasons to discard
pub enum Verdict {
    /// A record with no entity takes this one (`:104-109`).
    Attach,
    /// It is the animal already bound: nothing to do (`:111-114`).
    Keep,
    /// Bound to another animal of the same UUID, and this one is alive: the old
    /// entity is discarded and this one takes the record (`:116-122`).
    Replace,
    /// The animal is removed from the world (`:86`, `:98`, `:129`).
    Discard(Discard),
}

impl Verdict {
    pub fn name(self) -> &'static str {
        match self {
            Self::Attach => "animal.verdict.attach",
            Self::Keep => "animal.verdict.keep",
            Self::Replace => "animal.verdict.replace",
            Self::Discard(_) => "animal.verdict.discard",
        }
    }

    /// Whether the caller must remove the incoming entity:
    /// `animalEntity.remove(Entity.RemovalReason.DISCARDED)` happens on three of
    /// the four exits, never on [`Verdict::Keep`].
    pub fn removes_the_incoming(self) -> bool {
        matches!(self, Self::Discard(_))
    }
}

/// The four reasons `registerAnimal` throws an animal away, named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one per `remove` in `:74-130`
pub enum Discard {
    /// `getManagedAnimalId() == 0` (`:79`): no id is not an id.
    NoId,
    /// An id the colony holds no record for (`:79`).
    UnknownRecord { id: i32 },
    /// The entity's UUID is not the record's (`:92`): the animal that came back
    /// is not the animal that was saved.
    UuidMismatch { id: i32 },
    /// A bound record, a different animal, and that animal is dead (`:116-129`).
    NotAlive { id: i32 },
}

impl Discard {
    pub fn reason(self) -> &'static str {
        match self {
            Self::NoId => "animal.discard.no_id",
            Self::UnknownRecord { .. } => "animal.discard.unknown_record",
            Self::UuidMismatch { .. } => "animal.discard.uuid_mismatch",
            Self::NotAlive { .. } => "animal.discard.not_alive",
        }
    }

    pub fn id(self) -> Option<i32> {
        match self {
            Self::NoId => None,
            Self::UnknownRecord { id } | Self::UuidMismatch { id } | Self::NotAlive { id } => {
                Some(id)
            }
        }
    }
}

/// Why no record was created (`:224-228`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: Java has one guard and one unreachable fallback
pub enum Refusal {
    /// `colony == null` (`:224-228`): an animal without a colony cannot be
    /// registered to one.
    NoColony,
}

impl Refusal {
    pub fn name(self) -> &'static str {
        match self {
            Self::NoColony => "animal.refusal.no_colony",
        }
    }
}

/// What a read defaulted or dropped (`:290-335`). Java repairs all of it, so
/// there is no refusal to give: the worst it does is skip one record and warn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own repair set
pub enum Repair {
    /// No `animalManager` compound: the manager is left untouched (`:279-283`).
    NoManagerTag,
    /// No `nextID`: the counter is 1 (`:295-298`).
    NoNextId,
    /// No `animals` list: the counter is still restored (`:300-335`).
    NoAnimals,
    /// An element of the list that is not a compound — vanilla's typed `getList`
    /// filters those out and Java never sees them.
    NotACompound { index: usize },
    /// Two records with the same id: the later one wins (`:319-323`).
    Duplicate { id: i32 },
    /// No `uuid`: Java mints a random one at the next save (`AnimalData.java:155`).
    NoUuid { id: i32 },
}

impl Repair {
    pub fn name(self) -> &'static str {
        match self {
            Self::NoManagerTag => "animal.repair.no_manager_tag",
            Self::NoNextId => "animal.repair.no_next_id",
            Self::NoAnimals => "animal.repair.no_animals",
            Self::NotACompound { .. } => "animal.repair.not_a_compound",
            Self::Duplicate { .. } => "animal.repair.duplicate",
            Self::NoUuid { .. } => "animal.repair.no_uuid",
        }
    }
}

/// What one `sendPackets` call owes the clients (`:400-446`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// `refresh = !newSubscribers.isEmpty() || isDirty` (`:404`): a full snapshot
    /// rather than a patch.
    pub refresh: bool,
    /// The ids to send, ascending.
    pub ids: Vec<i32>,
}

impl Plan {
    /// Java returns without sending when the set is empty (`:432-435`), even
    /// with `refresh` true — a colony with no animal has nothing to say.
    pub fn sends(&self) -> bool {
        !self.ids.is_empty()
    }
}

/// One colony's animals: `AnimalManager`'s map and its counter.
///
/// **Iteration order is the port's choice.** Java walks a `HashMap` (`:44`), so
/// every list it answers has an unspecified order; [`BTreeMap`] by id makes a
/// test and a packet reproducible, and the consequence is stated where it bites:
/// [`Registry::animals_of`] answers in id order, not in insertion order.
#[derive(Clone, Debug, PartialEq)]
pub struct Registry {
    animals: BTreeMap<i32, Entry>,
    next_id: i32,
    dirty: bool,
}

impl Default for Registry {
    fn default() -> Registry {
        Registry::new()
    }
}

impl Registry {
    /// A colony with no animal (`nextAnimalID = -1`, `:59`).
    pub fn new() -> Registry {
        Registry {
            animals: BTreeMap::new(),
            next_id: -1,
            dirty: false,
        }
    }

    /// `getAnimal` (`:265-269`).
    pub fn get(&self, id: i32) -> Option<&Entry> {
        self.animals.get(&id)
    }

    /// `getCurrentAnimalCount` (`:207-211`).
    pub fn len(&self) -> usize {
        self.animals.len()
    }

    pub fn is_empty(&self) -> bool {
        self.animals.is_empty()
    }

    /// `getAnimals` (`:137-140`), ascending by id.
    pub fn ids(&self) -> Vec<i32> {
        self.animals.keys().copied().collect()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// `markDirty` (`:379-383`).
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// `clearDirty` (`:388-392`).
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// `registerAnimal` (`:74-130`), the decision only: Java also removes the
    /// entity and re-points the record's `WeakReference`, which are the caller's
    /// two calls ([`Verdict::removes_the_incoming`] says when).
    ///
    /// The order of the four checks is Java's, and it matters: an id of zero is
    /// refused **before** the map is asked, and the UUID's comparison comes
    /// before the record is known to be bound.
    pub fn register(&mut self, animal: Managed) -> Verdict {
        if animal.id == 0 {
            return Verdict::Discard(Discard::NoId);
        }
        let Some(entry) = self.animals.get(&animal.id) else {
            return Verdict::Discard(Discard::UnknownRecord { id: animal.id });
        };
        if entry.uuid != Some(animal.uuid) {
            return Verdict::Discard(Discard::UuidMismatch { id: animal.id });
        }
        if !entry.bound {
            if let Some(entry) = self.animals.get_mut(&animal.id) {
                entry.bound = true;
            }
            return Verdict::Attach;
        }
        if animal.same_as_bound {
            return Verdict::Keep;
        }
        if !animal.alive {
            return Verdict::Discard(Discard::NotAlive { id: animal.id });
        }
        // A live newcomer takes the record from the animal that was bound.
        if let Some(entry) = self.animals.get_mut(&animal.id) {
            entry.bound = true;
        }
        Verdict::Replace
    }

    /// `createAndRegisterAnimalData` (`:220-257`), minus the entity plumbing:
    /// the record, its id, and the three associations Java sets (the id on the
    /// animal, the colony id on the entity's data, the animal on the record).
    ///
    /// `colony` is that colony id (`:253` sets it on the entity's own data), and
    /// `None` is Java's `null` colony — the one refusal this method has
    /// (`:224-228`). The registry does not store it: an animal belongs to the
    /// colony whose manager it is in, and a second copy could disagree.
    ///
    /// **The id is the lowest hole, not a counter.** With no animal it is 1;
    /// otherwise the first free slot between 1 and `count + 1` wins (`:230-246`),
    /// which reuses an id a dead animal freed instead of growing the range. The
    /// loop cannot miss: `count + 1` slots, `count` taken.
    pub fn create(&mut self, colony: Option<i32>, uuid: Uuid) -> Result<Entry, Refusal> {
        if colony.is_none() {
            return Err(Refusal::NoColony);
        }
        let id = if self.animals.is_empty() {
            FIRST_ANIMAL_ID
        } else {
            first_free(&self.animals)
        };
        self.next_id = id;
        let mut entry = Entry::new(id);
        entry.uuid = Some(uuid);
        entry.bound = true;
        self.animals.insert(id, entry.clone());
        Ok(entry)
    }

    /// The counter Java keeps (`nextAnimalID`, `:59`): **the last id handed
    /// out**, not the next one to hand out. `-1` before the first animal.
    pub fn next_id(&self) -> i32 {
        self.next_id
    }

    /// `getAnimalsOfClassByHome` (`:149-183`) with `matchesHome` (`:192-200`):
    /// the caller's `only` is Java's `animalClass.isInstance`, `home` is the
    /// building filter, and `None` means every home.
    ///
    /// A record with no bound entity is skipped (`:156-160`): Java asks the
    /// live animal, not the record, so an animal that is not in the world is not
    /// "of a class".
    pub fn animals_of(&self, home: Option<[i32; 3]>, only: impl Fn(&Entry) -> bool) -> Vec<i32> {
        self.animals
            .values()
            .filter(|entry| entry.bound)
            .filter(|entry| home.is_none() || entry.home == home)
            .filter(|entry| only(entry))
            .map(|entry| entry.id)
            .collect()
    }

    /// `sendPackets` (`:400-446`) as a plan: who to send, and the dirty flags it
    /// cleared.
    ///
    /// Both branches clear every record's flag, and the refresh branch clears the
    /// manager's too — so a second call with no change sends nothing, and
    /// `data.clearDirty()` runs even for records that were not sent (`:417-430`).
    pub fn send_plan(&mut self, new_subscribers: bool) -> Plan {
        let refresh = new_subscribers || self.dirty;
        let mut ids = Vec::new();
        if refresh {
            ids = self.ids();
        } else {
            for (id, entry) in &self.animals {
                if entry.dirty {
                    ids.push(*id);
                }
            }
        }
        for entry in self.animals.values_mut() {
            entry.dirty = false;
        }
        if refresh {
            self.dirty = false;
        }
        Plan { refresh, ids }
    }

    /// `write` (`:344-358`), in the shape `AnimalManager.read` expects to find:
    /// **one** `animalManager` key holding `animals` and `nextID`, at the colony
    /// level. See the module note for the double nesting Java actually writes.
    pub fn write(&self) -> Compound {
        let records: Vec<Tag> = self
            .animals
            .values()
            .map(|entry| Tag::Compound(entry_compound(entry)))
            .collect();
        Compound::from([(
            KEY_ANIMAL_MANAGER.into(),
            Tag::Compound(Compound::from([
                (KEY_ANIMALS.into(), Tag::List(records)),
                (KEY_NEXT_ID.into(), Tag::Int(self.next_id)),
            ])),
        )])
    }

    /// `read` (`:276-336`), against the **colony** compound.
    ///
    /// Three rules Java states and a port must keep:
    ///
    /// 1. **No `animalManager` key means no mutation at all** (`:279-283`) — not
    ///    an empty manager. A colony file written before this manager existed
    ///    keeps whatever it had.
    /// 2. **`nextID` is restored even when the list is absent** (`:290-298`).
    /// 3. **One bad record costs that record, not the colony** (`:325-329`).
    pub fn read(&mut self, compound: &Compound) -> Vec<Repair> {
        let mut repairs = Vec::new();
        let inner = match compound.get(&KEY_ANIMAL_MANAGER.into()) {
            Some(Tag::Compound(inner)) => inner,
            _ => return vec![Repair::NoManagerTag],
        };

        // `:287-288`: a reload must not accumulate stale entries.
        self.animals.clear();

        match inner.get(&KEY_NEXT_ID.into()) {
            Some(Tag::Int(next)) => self.next_id = *next,
            _ => {
                self.next_id = FIRST_ANIMAL_ID;
                repairs.push(Repair::NoNextId);
            }
        }

        let Some(Tag::List(records)) = inner.get(&KEY_ANIMALS.into()) else {
            repairs.push(Repair::NoAnimals);
            return repairs;
        };
        for (index, record) in records.iter().enumerate() {
            let Tag::Compound(record) = record else {
                repairs.push(Repair::NotACompound { index });
                continue;
            };
            let entry = entry_from(record, &mut repairs);
            let id = entry.id;
            if self.animals.insert(id, entry).is_some() {
                repairs.push(Repair::Duplicate { id });
            }
        }
        repairs
    }
}

/// The first free id in `1 ..= len + 1` (`:238-246`).
fn first_free(animals: &BTreeMap<i32, Entry>) -> i32 {
    let ceiling = i32::try_from(animals.len()).unwrap_or(i32::MAX).saturating_add(1);
    for candidate in FIRST_ANIMAL_ID..=ceiling {
        if !animals.contains_key(&candidate) {
            return candidate;
        }
    }
    // Unreachable: `len + 1` slots and `len` taken. Said, not panicked on.
    ceiling
}

/// `AnimalData.serializeNBT` (`:146-163`), with the random UUID left out.
fn entry_compound(entry: &Entry) -> Compound {
    let mut compound = Compound::from([
        (KEY_ID.into(), Tag::Int(entry.id)),
        (KEY_POS.into(), position(entry.last_position)),
        (
            KEY_HOME.into(),
            position(entry.home.unwrap_or([0, 0, 0])),
        ),
        (KEY_MAX_HEALTH.into(), Tag::Float(entry.max_health.to_bits())),
        (
            KEY_COMBAT_COOLDOWN.into(),
            Tag::Float(entry.combat_cooldown.to_bits()),
        ),
    ]);
    if let Some(uuid) = entry.uuid {
        compound.insert(KEY_UUID.into(), Tag::IntArray(uuid.to_vec()));
    }
    if let Some(owner) = entry.owner {
        compound.insert(KEY_OWNER.into(), Tag::IntArray(owner.to_vec()));
    }
    compound
}

/// `AnimalData.deserializeNBT` (`:172-186`), every default kept.
fn entry_from(record: &Compound, repairs: &mut Vec<Repair>) -> Entry {
    let id = int_at(record, KEY_ID).unwrap_or(0);
    let home = position_of(record, KEY_HOME).filter(|at| *at != [0, 0, 0]);
    let uuid = uuid_at(record, KEY_UUID);
    if uuid.is_none() {
        repairs.push(Repair::NoUuid { id });
    }
    Entry {
        id,
        uuid,
        home,
        last_position: position_of(record, KEY_POS).unwrap_or([0, 0, 0]),
        max_health: float_at(record, KEY_MAX_HEALTH).unwrap_or(0.0),
        combat_cooldown: float_at(record, KEY_COMBAT_COOLDOWN).unwrap_or(0.0),
        owner: uuid_at(record, KEY_OWNER),
        bound: false,
        dirty: false,
    }
}

fn position(at: [i32; 3]) -> Tag {
    Tag::compound([
        ("x", Tag::Int(at[0])),
        ("y", Tag::Int(at[1])),
        ("z", Tag::Int(at[2])),
    ])
}

fn position_of(record: &Compound, key: &str) -> Option<[i32; 3]> {
    match record.get(&key.into())? {
        Tag::Compound(inner) => Some([
            int_at(inner, "x").unwrap_or(0),
            int_at(inner, "y").unwrap_or(0),
            int_at(inner, "z").unwrap_or(0),
        ]),
        _ => None,
    }
}

fn uuid_at(record: &Compound, key: &str) -> Option<Uuid> {
    match record.get(&key.into())? {
        Tag::IntArray(values) if values.len() == 4 => {
            Some([values[0], values[1], values[2], values[3]])
        }
        _ => None,
    }
}

fn int_at(record: &Compound, key: &str) -> Option<i32> {
    match record.get(&key.into())? {
        Tag::Int(value) => Some(*value),
        Tag::Byte(value) => Some(i32::from(*value)),
        Tag::Short(value) => Some(i32::from(*value)),
        _ => None,
    }
}

fn float_at(record: &Compound, key: &str) -> Option<f32> {
    match record.get(&key.into())? {
        Tag::Float(bits) => Some(f32::from_bits(*bits)),
        Tag::Double(bits) => Some(f64::from_bits(*bits) as f32),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uuid(n: i32) -> Uuid {
        [n, 0, 0, 0]
    }

    /// One record, as a **read** leaves it: `create` binds the animal it is
    /// given (`AnimalManager.java:254`), a record out of a file has none.
    fn with_one() -> Registry {
        let mut registry = Registry::new();
        registry.create(Some(1), uuid(7)).unwrap();
        registry.animals.get_mut(&1).unwrap().bound = false;
        registry
    }

    #[test]
    fn an_unknown_id_is_discarded_before_the_uuid_is_even_compared() {
        let mut registry = Registry::new();
        // `:79`: id 0 is not "an id the colony does not know", it is no id.
        assert_eq!(
            registry.register(Managed::new(0, uuid(1), true)),
            Verdict::Discard(Discard::NoId)
        );
        assert_eq!(
            registry.register(Managed::new(4, uuid(1), true)),
            Verdict::Discard(Discard::UnknownRecord { id: 4 })
        );
        assert!(registry.is_empty());
        // Both exits ask the caller to remove the entity (`:86`).
        assert!(Verdict::Discard(Discard::NoId).removes_the_incoming());
        assert_eq!(Discard::NoId.id(), None);
        assert_eq!(Discard::UnknownRecord { id: 4 }.id(), Some(4));
        // And a record without a colony is refused before any id is taken
        // (`:224-228`).
        assert_eq!(registry.create(None, uuid(1)), Err(Refusal::NoColony));
        assert!(registry.is_empty());
        assert_eq!(Refusal::NoColony.name(), "animal.refusal.no_colony");
    }

    #[test]
    fn a_uuid_that_is_not_the_records_discards_the_animal() {
        let mut registry = with_one();
        // `:92`: the animal that came back is not the one that was saved.
        assert_eq!(
            registry.register(Managed::new(1, uuid(9), true)),
            Verdict::Discard(Discard::UuidMismatch { id: 1 })
        );
        assert_eq!(Discard::UuidMismatch { id: 1 }.reason(), "animal.discard.uuid_mismatch");
        // The record is untouched: still one, still unbound.
        assert_eq!(registry.len(), 1);
        assert!(!registry.get(1).unwrap().bound);
        // `create` does bind what it is given (`:248-255`: `setManagedAnimal`
        // is the third association), which is why the fixture unbinds it.
        let mut freshly = Registry::new();
        freshly.create(Some(1), uuid(1)).unwrap();
        assert!(freshly.get(1).unwrap().bound);
    }

    #[test]
    fn an_unbound_record_takes_the_animal_and_a_bound_one_keeps_it_or_is_replaced() {
        let mut registry = with_one();
        let ours = Managed::new(1, uuid(7), true);

        // `:104-109`: nothing bound yet.
        assert_eq!(registry.register(ours), Verdict::Attach);
        assert!(registry.get(1).unwrap().bound);

        // `:111-114`: the same animal handed in again does nothing, and it is
        // the only verdict that does not remove an entity.
        let again = Managed {
            same_as_bound: true,
            ..ours
        };
        assert_eq!(registry.register(again), Verdict::Keep);
        assert!(!Verdict::Keep.removes_the_incoming());

        // `:116-122`: a different, live animal of the same UUID takes over.
        assert_eq!(registry.register(ours), Verdict::Replace);
        assert_eq!(Verdict::Replace.name(), "animal.verdict.replace");
        assert!(registry.get(1).unwrap().bound);
    }

    #[test]
    fn a_dead_newcomer_is_thrown_away_and_the_bound_animal_stays() {
        // The javadoc at `:70` says the *existing* animal is removed if it is
        // alive; `:116` tests the newcomer. A dead newcomer loses.
        let mut registry = with_one();
        registry.register(Managed::new(1, uuid(7), true));
        let dead = Managed::new(1, uuid(7), false);
        assert_eq!(
            registry.register(dead),
            Verdict::Discard(Discard::NotAlive { id: 1 })
        );
        assert!(registry.get(1).unwrap().bound);
    }

    #[test]
    fn ids_are_the_lowest_hole_not_a_counter() {
        let mut registry = Registry::new();
        assert_eq!(registry.create(Some(1), uuid(1)).unwrap().id, 1);
        assert_eq!(registry.create(Some(1), uuid(2)).unwrap().id, 2);
        assert_eq!(registry.create(Some(1), uuid(3)).unwrap().id, 3);
        assert_eq!(registry.next_id(), 3);

        // Free the middle one (`:236-246` reuses ids so nothing asks for an id
        // that is not there).
        registry.animals.remove(&2);
        assert_eq!(registry.create(Some(1), uuid(4)).unwrap().id, 2);
        assert_eq!(registry.ids(), vec![1, 2, 3]);

        // The proof that the loop cannot miss: fill every hole of 1..=count+1.
        let mut full = Registry::new();
        for n in 1..=4 {
            full.create(Some(1), uuid(n)).unwrap();
        }
        assert_eq!(full.create(Some(1), uuid(5)).unwrap().id, 5);
    }

    #[test]
    fn the_lookups_skip_an_animal_that_is_not_in_the_world_and_filter_by_home() {
        let mut registry = Registry::new();
        registry.create(Some(1), uuid(1)).unwrap();
        registry.create(Some(1), uuid(2)).unwrap();
        registry.create(Some(1), uuid(3)).unwrap();
        registry.animals.get_mut(&1).unwrap().home = Some([10, 64, 10]);
        registry.animals.get_mut(&2).unwrap().home = Some([10, 64, 10]);
        // Bind only 1 and 2: 3 is a record with no entity, so `:156-160` skips it.
        registry.animals.get_mut(&3).unwrap().bound = false;

        // No home filter: every *bound* animal.
        assert_eq!(registry.animals_of(None, |_| true), vec![1, 2]);
        // A home filter, `matchesHome` (`:192-200`).
        assert_eq!(
            registry.animals_of(Some([10, 64, 10]), |_| true),
            vec![1, 2]
        );
        assert!(registry.animals_of(Some([0, 64, 0]), |_| true).is_empty());
        // The class filter is the caller's predicate.
        assert_eq!(registry.animals_of(None, |entry| entry.id == 2), vec![2]);
    }

    #[test]
    fn the_send_plan_refreshes_for_a_newcomer_and_patches_only_the_dirty_otherwise() {
        // No animal to describe: Java sends nothing (`:432-435`), even though
        // the plan says "refresh".
        let mut registry = Registry::new();
        assert!(!registry.send_plan(true).sends());
        assert!(registry.send_plan(true).refresh);

        registry.create(Some(1), uuid(1)).unwrap();
        registry.create(Some(1), uuid(2)).unwrap();
        let plan = registry.send_plan(true);
        assert!(plan.refresh);
        assert_eq!(plan.ids, vec![1, 2]);

        // Nothing dirty: nothing to patch, and no refresh.
        let plan = registry.send_plan(false);
        assert!(!plan.refresh);
        assert!(!plan.sends());

        // One record marked: only that one, and the flag is cleared either way
        // (`:417-430`).
        registry.animals.get_mut(&2).unwrap().dirty = true;
        let plan = registry.send_plan(false);
        assert!(!plan.refresh);
        assert_eq!(plan.ids, vec![2]);
        assert!(!registry.get(2).unwrap().dirty);
        assert!(registry.send_plan(false).ids.is_empty());
    }

    #[test]
    fn the_save_is_one_wrap_and_comes_back_whole() {
        let mut registry = Registry::new();
        registry.create(Some(1), uuid(1)).unwrap();
        registry.create(Some(1), uuid(2)).unwrap();
        {
            let second = registry.animals.get_mut(&2).unwrap();
            second.home = Some([10, 64, -4]);
            second.last_position = [-7, 70, 3];
            second.max_health = 20.0;
            second.combat_cooldown = 4.5;
            second.owner = Some(uuid(3));
        }
        let compound = registry.write();

        // `bound` and `dirty` are runtime state Java keeps on the object, never
        // in the file (`AnimalData.java:146-163` writes neither).
        for entry in registry.animals.values_mut() {
            entry.bound = false;
        }

        // One `animalManager` key at the colony level, `animals` and `nextID`
        // inside it — the shape `AnimalManager.read:285` reads and
        // `Colony.write:940-942` does not produce.
        let Some(Tag::Compound(inner)) = compound.get(&"animalManager".into()) else {
            panic!("no animalManager compound");
        };
        assert!(matches!(inner.get(&"animals".into()), Some(Tag::List(_))));
        assert!(matches!(inner.get(&"nextID".into()), Some(Tag::Int(2))));

        let mut read = Registry::new();
        let repairs = read.read(&compound);
        assert!(repairs.is_empty(), "{repairs:?}");
        assert_eq!(read, registry);
        // The zero home is no home, in Java and here (`AnimalData.java:175-180`).
        assert_eq!(read.get(1).unwrap().home, None);
        assert_eq!(read.get(2).unwrap().home, Some([10, 64, -4]));
    }

    #[test]
    fn a_missing_manager_tag_leaves_the_manager_alone_and_a_missing_list_keeps_the_counter() {
        let mut registry = with_one();
        // `:279-283`: no key, no mutation — not even a clear.
        let repairs = registry.read(&Compound::new());
        assert_eq!(repairs, vec![Repair::NoManagerTag]);
        assert_eq!(registry.len(), 1);

        // `:290-298`: the counter survives a lost list.
        let counter_only = Compound::from([(
            "animalManager".into(),
            Tag::Compound(Compound::from([("nextID".into(), Tag::Int(9))])),
        )]);
        let repairs = registry.read(&counter_only);
        assert_eq!(repairs, vec![Repair::NoAnimals]);
        assert_eq!(registry.next_id(), 9);
        assert!(registry.is_empty());

        // A `nextID` of the wrong kind and an element that is not a compound:
        // both repaired, neither fatal.
        let rough = Compound::from([(
            "animalManager".into(),
            Tag::Compound(Compound::from([
                ("nextID".into(), Tag::String("nine".into())),
                (
                    "animals".into(),
                    Tag::List(vec![
                        Tag::Int(1),
                        Tag::Compound(Compound::from([("id".into(), Tag::Int(4))])),
                    ]),
                ),
            ])),
        )]);
        let repairs = registry.read(&rough);
        assert!(repairs.contains(&Repair::NoNextId));
        assert!(repairs.contains(&Repair::NotACompound { index: 0 }));
        assert!(repairs.contains(&Repair::NoUuid { id: 4 }));
        assert_eq!(registry.ids(), vec![4]);
        assert_eq!(registry.next_id(), FIRST_ANIMAL_ID);
    }

    #[test]
    fn two_records_with_the_same_id_keep_the_last_one() {
        let twice = Compound::from([(
            "animalManager".into(),
            Tag::Compound(Compound::from([(
                "animals".into(),
                Tag::List(vec![
                    Tag::Compound(Compound::from([
                        ("id".into(), Tag::Int(3)),
                        ("maxhealth".into(), Tag::Float(10.0f32.to_bits())),
                    ])),
                    Tag::Compound(Compound::from([
                        ("id".into(), Tag::Int(3)),
                        ("maxhealth".into(), Tag::Float(30.0f32.to_bits())),
                    ])),
                ]),
            )])),
        )]);
        let mut registry = Registry::new();
        let repairs = registry.read(&twice);
        assert!(repairs.iter().any(|repair| matches!(
            repair,
            Repair::Duplicate { id } if *id == 3
        )));
        assert_eq!(registry.get(3).unwrap().max_health, 30.0);
    }
}
