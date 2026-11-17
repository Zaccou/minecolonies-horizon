//! The visitors: who is in the colony without living there, and the rules that
//! decide whether an entity is allowed to represent one of them.
//!
//! Java reference: `core/colony/managers/VisitorManager.java` (337 lines) and
//! `core/colony/VisitorData.java` (the record, which adds three fields to a
//! citizen's: `rcost`, `rcostqty`, `sitting`).
//!
//! **What a visitor is here: an id, an identity, and an entity handle.** Java's
//! record is keyed by an id that is **negative** — `nextVisitorID` starts at `-1`
//! and every creation post-decrements it (`VisitorManager.java:65`, `:298`),
//! which is how a visitor id can never collide with a citizen's, and why
//! `manager_01/views.rs:40` routes a non-positive id to the visitor map. The
//! identity Java compares is the entity's UUID against `CitizenData.getUUID()`,
//! and that UUID is **derived, never saved**:
//! `UUID.nameUUIDFromBytes((id + ":" + colonyId + ":" + dimension))`
//! (`CitizenData.java:2100-2104`). The port keeps the pre-hash text, because the
//! SDK has no MD5 and a hashed UUID could not be produced here; two identities
//! Java would call different stay different here (`MANQUANT.md` §1).
//!
//! **Two "same entity" tests, and Java needs both.** `registerCivilian` compares
//! the **identity** to the record's (`:87`) *and* the **object** to the one
//! attached (`:106`). The first is a UUID check, the second a reference check —
//! and they are not the same test, because a visitor entity is respawned with the
//! record's own UUID (`:270` `citizenEntity.setUUID(data.getUUID())`), so the
//! normal case after a respawn is *a different object with the same identity*,
//! which is exactly the replacement at `:111-117`. A port that compared only
//! identities could never take that branch, which is why the caller hands in a
//! handle as well: a wasm mod has no entity reference
//! (`crates/horizon-mod-vocab/src/host/entities.rs`), only what it was given.
//!
//! **Three tables, each taken from one method.**
//!
//! | Rule | Java | Port |
//! | --- | --- | --- |
//! | who may represent which record | `registerCivilian` `:73-124` | [`Visitors::register`] → [`Outcome`] |
//! | what is sent, and when the flags clear | `sendPackets` `:171-216` | [`Visitors::send_plan`] |
//! | where a visitor may appear | `spawnOrCreateCivilian` `:238-292` | [`spawn`] → [`Refusal`] |
//!
//! **The order in `registerCivilian` is the rule, and the port keeps all six
//! steps.** An id of `0` or an id nobody holds is discarded *before* anything
//! else (`:75-83`); a record whose identity is not this entity's is discarded
//! next (`:87-95`); a record with no entity adopts this one (`:99-104`); the same
//! object again is a no-op (`:106-109`); a live entity **replaces** whatever was
//! attached (`:111-117`); and a dead one is discarded (`:119-123`). The second
//! null test at `:87` can never be true — the lookup at `:75` already answered
//! non-null — and a branch Java cannot take is not reproduced.
//!
//! **What the port does not do: touch the world.** Every discard in Java calls
//! `visitor.remove(DISCARDED)` (`:81`, `:93`, `:123`) and every replacement calls
//! `existingCitizen.get().remove(...)` (`:113`). The port answers *what* the
//! caller must discard ([`Outcome`], [`Visitors::remove`]) and never calls the
//! entity. The `isAddedToWorld()` warnings at `:77-80`, `:89-92` and `:119-122`
//! are log lines with no decision in them, so they are dropped rather than
//! carried as a field nobody reads.
//!
//! **One divergence, and it is the port's own.** A counter that has drifted to
//! `0` — which is what a save without `nextID` gives, since `getInt` answers `0`
//! (`:149`) — would hand out id `0`, and `registerCivilian` discards every entity
//! whose id is `0` (`:75`): the visitor would exist in the map and could never be
//! represented. [`Visitors::create`] refuses that allocation instead.

use std::collections::{BTreeMap, BTreeSet};

use mod_api::{Compound, Tag};

use super::nbt;

/// `TAG_VISIT_MANAGER` (`VisitorManager.java:43`).
pub const TAG_VISIT_MANAGER: &str = "visitManager";
/// `TAG_VISITORS` (`:44`), the list inside it.
pub const TAG_VISITORS: &str = "visitors";
/// `TAG_NEXTID` (`:45`).
pub const TAG_NEXTID: &str = "nextID";
/// `TAG_ID` (`NbtTagConstants.java:8`), the only field of a visitor record this
/// port saves — `VisitorData.serializeNBT` writes its three extra fields beside
/// the citizen's own (`VisitorData.java:53-62`), none of which is modelled.
pub const KEY_ID: &str = "id";

/// Where the counter starts: `nextVisitorID = -1` (`VisitorManager.java:65`), so
/// the first visitor ever created is `-1` (`:298` post-decrements).
pub const FIRST_VISITOR_ID: i32 = -1;

/// `PathingConstants.HALF_A_BLOCK` (`api/util/constant/PathingConstants.java:49`),
/// added to both horizontal coordinates of the spawn position (`:271`).
pub const HALF_A_BLOCK: f64 = 0.5;

/// `Constants.SLIGHTLY_UP` (`api/util/constant/Constants.java:49`), added to the
/// spawn height (`:271`).
pub const SLIGHTLY_UP: f64 = 0.1;

/// One visitor, as this port keeps it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Visitor {
    id: i32,
    identity: String,
    entity: Option<u64>,
    dirty: bool,
}

impl Visitor {
    /// The id Java keys the map by (`:50`).
    pub fn id(&self) -> i32 {
        self.id
    }

    /// What Java derives into the entity's UUID
    /// (`CitizenData.java:2100-2104`), as text.
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// The handle of the attached entity, if any (`getEntity().isPresent()`,
    /// `:99`).
    pub fn entity(&self) -> Option<u64> {
        self.entity
    }

    /// Whether an entity is attached.
    pub fn has_entity(&self) -> bool {
        self.entity.is_some()
    }

    /// Whether this record changed since the last send (`isDirty`, `:189`).
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
}

/// What `registerCivilian` did with an entity (`:73-124`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four ends of the six-branch method
pub enum Outcome {
    /// The record had no entity; this one is attached (`:99-104`).
    Adopted,
    /// The record already holds this very object; nothing happens (`:106-109`).
    AlreadyThere,
    /// A live entity replaced the one attached (`:111-117`); the caller discards
    /// the old one.
    Replaced,
    /// The entity may not represent anything (`:75-83`, `:87-95`, `:119-123`);
    /// the caller discards it.
    Discarded(Discard),
}

impl Outcome {
    /// A stable name, namespaced so it can never be mistaken for a sentence.
    pub fn name(self) -> &'static str {
        match self {
            Self::Adopted => "visitors.outcome.adopted",
            Self::AlreadyThere => "visitors.outcome.already_there",
            Self::Replaced => "visitors.outcome.replaced",
            Self::Discarded(reason) => reason.name(),
        }
    }
}

/// Why an entity was discarded, one variant per discard site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one variant per discard site in `registerCivilian`
pub enum Discard {
    /// Id `0`, or no record with that id (`:75-83`).
    UnknownVisitor,
    /// The record exists but its identity is not this entity's (`:87-95`).
    IdentityMismatch,
    /// The entity is not alive and something else is attached (`:119-123`).
    NotAlive,
}

impl Discard {
    pub fn name(self) -> &'static str {
        match self {
            Self::UnknownVisitor => "visitors.discard.unknown_visitor",
            Self::IdentityMismatch => "visitors.discard.identity",
            Self::NotAlive => "visitors.discard.not_alive",
        }
    }
}

/// Why this manager refused an operation, named.
///
/// A variant is **not** a `lang` key: those live in `assets/minecolonies/lang/`,
/// which this module does not own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// The counter has no id left to give: Java's `nextVisitorID--` would wrap
    /// from `i32::MIN` to `i32::MAX` (`:298`) and every id from there on is
    /// discarded at `:75`.
    IdsExhausted,
    /// The counter holds a value that is not a visitor id, so the next id would
    /// be `0` or positive. Java hands it out and the entity is then discarded at
    /// `:75`, losing the visitor in silence.
    CounterNotVisitor { next: i32 },
    /// No town hall to arrive at (`:240`, first half).
    NoTownHall,
    /// A town hall exists but `MOVE_IN` is off and nobody forced the spawn
    /// (`:240`, second half).
    MoveInDisabled,
    /// Nowhere to stand. `announced` is Java's own test at `:286`: the warning is
    /// sent only when the town hall's chunk is loaded.
    NoArrivalSpace { announced: bool },
}

impl Refusal {
    pub fn name(self) -> &'static str {
        match self {
            Self::IdsExhausted => "visitors.refusal.ids_exhausted",
            Self::CounterNotVisitor { .. } => "visitors.refusal.counter",
            Self::NoTownHall => "visitors.refusal.no_town_hall",
            Self::MoveInDisabled => "visitors.refusal.move_in",
            Self::NoArrivalSpace { .. } => "visitors.refusal.no_arrival_space",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())?;
        match self {
            Self::CounterNotVisitor { next } => write!(f, " (next {next})"),
            Self::NoArrivalSpace { announced } => write!(f, " (announced {announced})"),
            Self::IdsExhausted | Self::NoTownHall | Self::MoveInDisabled => Ok(()),
        }
    }
}

impl std::error::Error for Refusal {}

/// A record this read could not take, and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Skipped {
    pub index: usize,
    pub reason: SkipReason,
}

/// Why a saved visitor record was dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipReason {
    /// No `id` at all. Java's `getInt` answers `0` (`VisitorData.java:94`), an id
    /// `registerCivilian` discards (`:75`) — a visitor that can never appear.
    NoId,
}

impl SkipReason {
    pub fn name(self) -> &'static str {
        match self {
            Self::NoId => "visitors.skip.no_id",
        }
    }
}

/// What one tick does to the records, in Java's order
/// (`updateEntityIfNecessary`, `CitizenData.java:922-940`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TickPlan {
    /// Records whose entity was detached because it is dead or in an unloaded
    /// chunk (`:931-933`). The caller discards the entity.
    pub detach: Vec<u64>,
    /// Records that need an entity spawned, detached ones and never-spawned ones
    /// together (`:935`).
    pub respawn: Vec<i32>,
}

/// What one `sendPackets` sends (`:171-216`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// Whether every record goes, not just the changed ones (`:174-184`).
    pub refresh: bool,
    /// The record ids, in id order.
    pub records: Vec<i32>,
    /// Who receives them: the new subscribers and the old ones (`:207-208`), in
    /// name order.
    pub players: Vec<String>,
}

/// The colony's visitors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Visitors {
    next_id: i32,
    records: BTreeMap<i32, Visitor>,
    dirty: bool,
}

impl Default for Visitors {
    fn default() -> Self {
        Visitors::new()
    }
}

impl Visitors {
    /// An empty manager with the counter at its Java start (`:65`).
    pub fn new() -> Visitors {
        Visitors { next_id: FIRST_VISITOR_ID, records: BTreeMap::new(), dirty: false }
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The id the counter would hand out next (`-1` on a fresh manager).
    pub fn next_id(&self) -> i32 {
        self.next_id
    }

    pub fn get(&self, id: i32) -> Option<&Visitor> {
        self.records.get(&id)
    }

    /// The records, in id order — which is not the order Java's `HashMap` gives
    /// (the same divergence `manager_01/views.rs` states).
    pub fn iter(&self) -> impl Iterator<Item = &Visitor> {
        self.records.values()
    }

    /// Whether anything changed since the last send or save (`isDirty`, `:315`).
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// `markDirty` (`:315-318`).
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// `clearDirty` (`:320-324`).
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// Whether an entity is attached to this record.
    pub fn attached(&self, id: i32) -> bool {
        self.records.get(&id).is_some_and(Visitor::has_entity)
    }

    /// A record's own `markDirty` (`CitizenData.java:430`), which a change to one
    /// of its fields calls; `true` when the record exists.
    pub fn touch(&mut self, id: i32) -> bool {
        let Some(record) = self.records.get_mut(&id) else {
            return false;
        };
        record.dirty = true;
        true
    }

    /// A new record with the next negative id: `createAndRegisterCivilianData`
    /// (`:295-302`).
    ///
    /// Java marks the manager dirty *before* creating (`:297`); the port marks it
    /// after the counter agreed, so a refusal leaves the manager clean. Nothing
    /// Java can do refuses, so the order is unobservable there. `initForNewCivilian`
    /// (`:299`) is not called: it fills a name, a family and a saturation, and
    /// this crate's citizen model owns those (`src/persistence/roster.rs`).
    pub fn create(&mut self, colony: i32, dimension: &str) -> Result<i32, Refusal> {
        let id = self.next_id;
        if id >= 0 {
            return Err(Refusal::CounterNotVisitor { next: id });
        }
        let next = id.checked_sub(1).ok_or(Refusal::IdsExhausted)?;
        self.next_id = next;
        self.records.insert(
            id,
            Visitor { id, identity: identity_of(id, colony, dimension), entity: None, dirty: false },
        );
        self.mark_dirty();
        Ok(id)
    }

    /// `removeCivilian` (`:305-312`): the record goes, and the entity it held is
    /// the caller's to discard.
    pub fn remove(&mut self, id: i32) -> Option<Visitor> {
        self.records.remove(&id)
    }

    /// `registerCivilian` (`:73-124`), six steps in Java's order.
    pub fn register(&mut self, id: i32, identity: &str, handle: u64, alive: bool) -> Outcome {
        if id == 0 {
            return Outcome::Discarded(Discard::UnknownVisitor);
        }
        let Some(record) = self.records.get_mut(&id) else {
            return Outcome::Discarded(Discard::UnknownVisitor);
        };
        if record.identity != identity {
            return Outcome::Discarded(Discard::IdentityMismatch);
        }
        match record.entity {
            None => {
                record.entity = Some(handle);
                Outcome::Adopted
            }
            Some(attached) if attached == handle => Outcome::AlreadyThere,
            Some(_) if alive => {
                record.entity = Some(handle);
                Outcome::Replaced
            }
            Some(_) => Outcome::Discarded(Discard::NotAlive),
        }
    }

    /// `unregisterCivilian` (`:127-134`): the record loses its entity **only**
    /// when the object handed in is the one attached; `true` says it was.
    pub fn unregister(&mut self, id: i32, handle: u64) -> bool {
        let Some(record) = self.records.get_mut(&id) else {
            return false;
        };
        if record.entity != Some(handle) {
            return false;
        }
        record.entity = None;
        true
    }

    /// `read` (`:137-152`): the records first, then the counter, then a
    /// `markDirty` that happens even when the tag is absent (`:151`).
    ///
    /// A record with no `id` is skipped and reported; Java reads it as id `0`
    /// (`VisitorData.java:94`), which the map then holds and `registerCivilian`
    /// can never fill (`:75`).
    pub fn read(&mut self, compound: &Compound, colony: i32, dimension: &str) -> Vec<Skipped> {
        let mut skipped = Vec::new();
        if nbt::has(compound, TAG_VISIT_MANAGER) {
            let empty = Compound::new();
            let manager = nbt::compound(compound, TAG_VISIT_MANAGER).unwrap_or(&empty);
            for (index, record) in nbt::compounds(manager, TAG_VISITORS).into_iter().enumerate() {
                match nbt::int(record, KEY_ID) {
                    Some(id) => {
                        self.records.insert(
                            id,
                            Visitor {
                                id,
                                identity: identity_of(id, colony, dimension),
                                entity: None,
                                dirty: false,
                            },
                        );
                    }
                    None => skipped.push(Skipped { index, reason: SkipReason::NoId }),
                }
            }
            // `getInt` answers 0 for a missing key (`:149`); `create` then refuses
            // to hand out an id nobody can represent.
            self.next_id = nbt::int(manager, TAG_NEXTID).unwrap_or(0);
        }
        self.mark_dirty();
        skipped
    }

    /// `write` (`:154-168`): the records, then the counter, inside
    /// `visitManager`.
    pub fn write(&self) -> Compound {
        let records: Vec<Compound> = self
            .records
            .values()
            .map(|visitor| Compound::from([(KEY_ID.into(), Tag::Int(visitor.id))]))
            .collect();
        let mut manager = Compound::new();
        manager.insert(TAG_VISITORS.into(), nbt::compound_list(records));
        manager.insert(TAG_NEXTID.into(), Tag::Int(self.next_id));
        Compound::from([(TAG_VISIT_MANAGER.into(), Tag::Compound(manager))])
    }

    /// `sendPackets` (`:171-216`), decided and not sent.
    ///
    /// **Three rules, in this order.** A refresh is due when there is a new
    /// subscriber or the manager itself changed (`:174`); a refresh sends
    /// **every** record and clears every record's flag plus the manager's
    /// (`:176-184`); otherwise only the dirty records go, and **every** record's
    /// flag is cleared whether it went or not (`:187-200`). The empty test comes
    /// last (`:202-205`), so flags are cleared even when nothing is sent.
    ///
    /// Java builds the receiver set with `new HashSet<>(newSubscribers)` then
    /// `addAll(closeSubscribers)` (`:207-208`) — one set, no order. The port
    /// sorts it, so a test can say who receives.
    pub fn send_plan(&mut self, close: &BTreeSet<String>, new: &BTreeSet<String>) -> Option<Plan> {
        let refresh = !new.is_empty() || self.dirty;
        let mut records = Vec::new();
        if refresh {
            for (id, record) in &mut self.records {
                record.dirty = false;
                records.push(*id);
            }
            self.dirty = false;
        } else {
            for (id, record) in &mut self.records {
                if record.dirty {
                    records.push(*id);
                }
                record.dirty = false;
            }
        }
        if records.is_empty() {
            return None;
        }
        let mut players: Vec<String> = new.iter().chain(close.iter()).cloned().collect();
        players.sort();
        players.dedup();
        Some(Plan { refresh, records, players })
    }

    /// `onColonyTick` (`:327-336`) and the half of `updateEntityIfNecessary`
    /// that needs no world (`CitizenData.java:922-940`).
    ///
    /// Without a town hall nothing is touched at all (`:329`). With one, a record
    /// whose entity is not alive in a loaded chunk is detached first (`:931-933`)
    /// and then asked for a new one (`:935`); the travelling gate at `:936-939` is
    /// not ported — there is no travelling manager here (`MANQUANT.md` §2).
    pub fn tick_plan(&mut self, has_town_hall: bool, usable: &dyn Fn(i32) -> bool) -> TickPlan {
        let mut plan = TickPlan::default();
        if !has_town_hall {
            return plan;
        }
        for (id, record) in &mut self.records {
            if let Some(handle) = record.entity {
                if usable(*id) {
                    continue;
                }
                record.entity = None;
                plan.detach.push(handle);
            }
            plan.respawn.push(*id);
        }
        plan
    }
}

/// What Java derives into a visitor's UUID (`CitizenData.java:2100-2104`), before
/// the hash: `id + ":" + colonyId + ":" + dimension`.
pub fn identity_of(id: i32, colony: i32, dimension: &str) -> String {
    format!("{id}:{colony}:{dimension}")
}

/// Where a visitor may appear: `spawnOrCreateCivilian` (`:238-292`), with the
/// world reduced to two questions.
///
/// **The order is Java's.** No town hall, or `MOVE_IN` off without `force`, and
/// nothing is attempted at all (`:240-243`). The positions are tried in the order
/// given and the town hall's own position is **appended after them**
/// (`:245-249`) — a caller's list always wins over the tower. A position is
/// skipped when it is the origin (`:253-256`) or its chunk is not loaded
/// (`:258`); a loaded position whose spawn point cannot be found is skipped too
/// (`:260-262`). Past the loop, the announcement is Java's own test: only a
/// loaded town hall warns (`:286-290`).
pub fn spawn(
    given: &[[i32; 3]],
    town_hall: Option<[i32; 3]>,
    move_in: bool,
    force: bool,
    loaded: &dyn Fn([i32; 3]) -> bool,
    point: &dyn Fn([i32; 3]) -> Option<[i32; 3]>,
) -> Result<[i32; 3], Refusal> {
    let Some(town_hall) = town_hall else {
        return Err(Refusal::NoTownHall);
    };
    if !move_in && !force {
        return Err(Refusal::MoveInDisabled);
    }
    let mut candidates: Vec<[i32; 3]> = given.to_vec();
    candidates.push(town_hall);
    for at in candidates {
        if at == [0, 0, 0] || !loaded(at) {
            continue;
        }
        if let Some(calculated) = point(at) {
            return Ok(calculated);
        }
    }
    Err(Refusal::NoArrivalSpace { announced: loaded(town_hall) })
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLONY: i32 = 4;
    const DIM: &str = "minecraft:overworld";
    const HALL: [i32; 3] = [100, 70, 100];

    fn three() -> Visitors {
        let mut visitors = Visitors::new();
        for _ in 0..3 {
            visitors.create(COLONY, DIM).expect("a fresh counter");
        }
        visitors
    }

    fn ids(visitors: &Visitors) -> Vec<i32> {
        visitors.iter().map(Visitor::id).collect()
    }

    fn nowhere() -> impl Fn([i32; 3]) -> bool {
        |_| false
    }

    fn no_point() -> impl Fn([i32; 3]) -> Option<[i32; 3]> {
        |_| None
    }

    #[test]
    fn the_counter_starts_at_minus_one_and_only_goes_down() {
        let mut visitors = Visitors::new();
        assert_eq!(visitors.next_id(), FIRST_VISITOR_ID, ":65");
        assert_eq!(visitors.create(COLONY, DIM), Ok(-1), ":298 post-decrements");
        assert_eq!(visitors.create(COLONY, DIM), Ok(-2));
        assert_eq!(visitors.len(), 2);
        assert_eq!(
            visitors.get(-1).expect("just created").identity(),
            identity_of(-1, COLONY, DIM).as_str()
        );
    }

    #[test]
    fn a_counter_at_zero_refuses_instead_of_handing_out_an_id_register_discards() {
        // What a save without `nextID` gives: `getInt` answers 0 (:149).
        let mut empty = Visitors::new();
        assert!(empty.read(&Compound::new(), COLONY, DIM).is_empty());
        assert_eq!(empty.next_id(), -1, "an absent tag leaves the counter alone");
        assert!(empty.is_dirty(), ":151 marks dirty even when the tag is absent");

        let mut manager = Compound::new();
        manager.insert(TAG_NEXTID.into(), Tag::Int(0));
        let saved = Compound::from([(TAG_VISIT_MANAGER.into(), Tag::Compound(manager))]);
        let mut visitors = Visitors::new();
        visitors.read(&saved, COLONY, DIM);
        assert_eq!(visitors.next_id(), 0);
        assert_eq!(visitors.create(COLONY, DIM), Err(Refusal::CounterNotVisitor { next: 0 }));
        assert!(visitors.is_empty(), "a refused creation files nobody");
    }

    #[test]
    fn an_exhausted_counter_refuses_rather_than_wrapping_to_positive_ids() {
        let mut manager = Compound::new();
        manager.insert(TAG_NEXTID.into(), Tag::Int(i32::MIN));
        let saved = Compound::from([(TAG_VISIT_MANAGER.into(), Tag::Compound(manager))]);
        let mut visitors = Visitors::new();
        visitors.read(&saved, COLONY, DIM);
        assert_eq!(visitors.create(COLONY, DIM), Err(Refusal::IdsExhausted));
        assert_eq!(visitors.next_id(), i32::MIN, "a refusal does not move the counter");
    }

    #[test]
    fn register_walks_java_six_steps_in_order() {
        let mut visitors = Visitors::new();
        let id = visitors.create(COLONY, DIM).expect("a fresh counter");
        let me = identity_of(id, COLONY, DIM);
        let impostor = identity_of(id + 100, COLONY, DIM);

        // :75 — nobody by that id, and 0 is never a visitor.
        assert_eq!(visitors.register(0, &me, 1, true), Outcome::Discarded(Discard::UnknownVisitor));
        assert_eq!(visitors.register(99, &me, 1, true), Outcome::Discarded(Discard::UnknownVisitor));
        assert!(!visitors.attached(id), "the first pass attached nothing");
        // :87 — the identity is not this record's.
        assert_eq!(
            visitors.register(id, &impostor, 1, true),
            Outcome::Discarded(Discard::IdentityMismatch)
        );
        // :99 — no entity yet: adopt.
        assert_eq!(visitors.register(id, &me, 7, true), Outcome::Adopted);
        assert_eq!(visitors.get(id).expect("just adopted").entity(), Some(7));
        // :106 — the same object again: nothing.
        assert_eq!(visitors.register(id, &me, 7, true), Outcome::AlreadyThere);
        // :111 — another live object with the same identity replaces it: this is
        // the normal respawn (`:270` gives the new entity the record's UUID).
        assert_eq!(visitors.register(id, &me, 8, true), Outcome::Replaced);
        assert_eq!(visitors.get(id).expect("just replaced").entity(), Some(8));
        // :119 — a dead object does not replace a live one.
        assert_eq!(
            visitors.register(id, &me, 9, false),
            Outcome::Discarded(Discard::NotAlive)
        );
        assert_eq!(visitors.get(id).expect("untouched").entity(), Some(8));
    }

    #[test]
    fn a_record_with_no_entity_adopts_a_dead_one_because_that_branch_comes_first() {
        let mut visitors = Visitors::new();
        let id = visitors.create(COLONY, DIM).expect("a fresh counter");
        let me = identity_of(id, COLONY, DIM);
        assert_eq!(visitors.register(id, &me, 1, false), Outcome::Adopted, ":99 is tested first");
    }

    #[test]
    fn unregister_only_unhooks_the_object_that_is_attached() {
        let mut visitors = Visitors::new();
        let id = visitors.create(COLONY, DIM).expect("a fresh counter");
        let me = identity_of(id, COLONY, DIM);
        assert_eq!(visitors.register(id, &me, 5, true), Outcome::Adopted);
        assert!(!visitors.unregister(id, 6), ":130 compares the object");
        assert!(visitors.attached(id));
        assert!(visitors.unregister(id, 5));
        assert!(!visitors.attached(id));
        assert!(!visitors.unregister(99, 5), "an unknown id is not an error");
    }

    #[test]
    fn remove_hands_the_entity_back_to_the_caller() {
        let mut visitors = three();
        let id = *ids(&visitors).first().expect("three records");
        assert!(!visitors.attached(id));
        let taken = visitors.remove(id).expect("just there");
        assert_eq!(taken.id(), id);
        assert_eq!(visitors.len(), 2);
        assert!(visitors.remove(id).is_none(), ":307 answers null the second time");
    }

    #[test]
    fn a_save_round_trips_the_records_and_the_counter() {
        let mut visitors = three();
        visitors.create(COLONY, DIM).expect("a fourth");
        let saved = visitors.write();
        let mut read = Visitors::new();
        assert!(read.read(&saved, COLONY, DIM).is_empty());
        assert_eq!(read.len(), 4);
        assert_eq!(read.next_id(), visitors.next_id());
        assert_eq!(ids(&read), ids(&visitors));
        let id = *ids(&read).first().expect("four records");
        assert_eq!(
            read.get(id).expect("read back").identity(),
            identity_of(id, COLONY, DIM).as_str()
        );
    }

    #[test]
    fn a_record_without_an_id_is_skipped_and_reported() {
        let mut manager = Compound::new();
        manager.insert(
            TAG_VISITORS.into(),
            nbt::compound_list(vec![
                Compound::from([(KEY_ID.into(), Tag::Int(-3))]),
                Compound::new(),
            ]),
        );
        manager.insert(TAG_NEXTID.into(), Tag::Int(-4));
        let saved = Compound::from([(TAG_VISIT_MANAGER.into(), Tag::Compound(manager))]);
        let mut visitors = Visitors::new();
        let skipped = visitors.read(&saved, COLONY, DIM);
        assert_eq!(ids(&visitors), vec![-3]);
        assert_eq!(skipped, vec![Skipped { index: 1, reason: SkipReason::NoId }]);
        assert_eq!(visitors.next_id(), -4);
    }

    #[test]
    fn a_refresh_sends_everything_and_a_quiet_pass_sends_only_what_changed() {
        let mut visitors = three();
        visitors.clear_dirty();
        let new: BTreeSet<String> = ["bob".to_owned()].into_iter().collect();
        // A new subscriber is a refresh on its own (:174).
        let plan = visitors
            .send_plan(&BTreeSet::new(), &new)
            .expect("a refresh is due");
        assert!(plan.refresh, ":174 a new subscriber forces it");
        assert_eq!(plan.records, ids(&visitors));
        assert_eq!(plan.players, vec!["bob".to_owned()]);
        assert!(!visitors.is_dirty(), ":183 clears the manager");

        // Nothing new, nothing dirty: no message at all (:202-205).
        assert_eq!(visitors.send_plan(&BTreeSet::new(), &BTreeSet::new()), None);

        // One record changed: one record goes, to the close subscriber.
        let id = *ids(&visitors).first().expect("three records");
        assert!(visitors.touch(id));
        let close: BTreeSet<String> = ["alice".to_owned()].into_iter().collect();
        let plan = visitors
            .send_plan(&close, &BTreeSet::new())
            .expect("one record is dirty");
        assert!(!plan.refresh);
        assert_eq!(plan.records, vec![id]);
        assert_eq!(plan.players, vec!["alice".to_owned()]);
        assert!(!visitors.get(id).expect("still there").is_dirty(), ":198 clears it");
        assert!(!visitors.touch(99), "an unknown record has nothing to mark");
    }

    #[test]
    fn nothing_ticks_without_a_town_hall() {
        let mut visitors = three();
        assert_eq!(visitors.tick_plan(false, &|_: i32| false), TickPlan::default(), ":329");
        assert_eq!(visitors.tick_plan(true, &|_: i32| false).respawn, ids(&visitors));
    }

    #[test]
    fn a_dead_or_unloaded_entity_is_detached_before_being_respawned() {
        let mut visitors = Visitors::new();
        let id = visitors.create(COLONY, DIM).expect("a fresh counter");
        let me = identity_of(id, COLONY, DIM);
        assert_eq!(visitors.register(id, &me, 3, true), Outcome::Adopted);
        // Usable: no detach, no respawn (:927-929).
        assert_eq!(visitors.tick_plan(true, &|_: i32| true), TickPlan::default());
        // Not usable: detached first (:931-933), then respawned (:935).
        let plan = visitors.tick_plan(true, &|_: i32| false);
        assert_eq!(plan.detach, vec![3]);
        assert_eq!(plan.respawn, vec![id]);
        assert!(!visitors.attached(id));
    }

    #[test]
    fn no_town_hall_and_move_in_off_are_two_different_refusals() {
        let refused = spawn(&[[1, 70, 1]], None, true, true, &nowhere(), &no_point());
        assert_eq!(refused, Err(Refusal::NoTownHall), ":240 first half");
        let refused = spawn(&[[1, 70, 1]], Some(HALL), false, false, &nowhere(), &no_point());
        assert_eq!(refused, Err(Refusal::MoveInDisabled), ":240 second half");
    }

    #[test]
    fn force_walks_past_the_move_in_setting_and_the_given_position_wins() {
        let here = [1, 70, 1];
        let spawned = spawn(
            &[here, [0, 0, 0]],
            Some(HALL),
            false,
            true,
            &|at| at == here || at == HALL,
            &|at| Some([at[0], at[1] + 1, at[2]]),
        );
        assert_eq!(spawned, Ok([1, 71, 1]), ":251-282 stops at the first position");
    }

    #[test]
    fn the_origin_is_skipped_and_the_town_hall_is_tried_last() {
        let spawned = spawn(&[[0, 0, 0]], Some(HALL), true, false, &|at| at == HALL, &|at| Some(at));
        assert_eq!(spawned, Ok(HALL), ":253-256 skips the origin, :247-248 appends the hall");
    }

    #[test]
    fn nowhere_to_stand_is_one_refusal_and_the_announcement_depends_on_the_hall() {
        let silent = spawn(&[], Some(HALL), true, false, &nowhere(), &no_point());
        assert_eq!(silent, Err(Refusal::NoArrivalSpace { announced: false }), ":286");
        let announced = spawn(&[], Some(HALL), true, false, &|_: [i32; 3]| true, &no_point());
        assert_eq!(announced, Err(Refusal::NoArrivalSpace { announced: true }), ":286-290");
    }

    #[test]
    fn a_spawn_position_needs_both_a_loaded_chunk_and_a_point() {
        let here = [1, 70, 1];
        // Loaded but no point: nothing spawns there.
        let blocked = spawn(&[here], Some(HALL), true, false, &|_: [i32; 3]| true, &no_point());
        assert_eq!(blocked, Err(Refusal::NoArrivalSpace { announced: true }), ":260-262");
        // A point without a loaded chunk is not enough either.
        let unloaded = spawn(&[here], Some(HALL), true, false, &nowhere(), &|at| Some(at));
        assert_eq!(unloaded, Err(Refusal::NoArrivalSpace { announced: false }), ":258");
    }

    #[test]
    fn the_two_spawn_offsets_are_java_s_own_values() {
        assert_eq!(HALF_A_BLOCK, 0.5, "PathingConstants.java:49");
        assert_eq!(SLIGHTLY_UP, 0.1, "Constants.java:49");
    }

    #[test]
    fn every_refusal_and_discard_has_its_own_name() {
        let names = [
            Refusal::IdsExhausted.name(),
            Refusal::CounterNotVisitor { next: 0 }.name(),
            Refusal::NoTownHall.name(),
            Refusal::MoveInDisabled.name(),
            Refusal::NoArrivalSpace { announced: false }.name(),
            Discard::UnknownVisitor.name(),
            Discard::IdentityMismatch.name(),
            Discard::NotAlive.name(),
            SkipReason::NoId.name(),
        ];
        for (index, name) in names.iter().enumerate() {
            for other in &names[index + 1..] {
                assert_ne!(name, other, "two names collide, so a log line lies");
            }
            assert!(name.starts_with("visitors."), "{name} is not in this namespace");
        }
        assert_eq!(Outcome::Discarded(Discard::NotAlive).name(), "visitors.discard.not_alive");
    }
}
