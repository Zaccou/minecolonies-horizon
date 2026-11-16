//! The manager: which colony owns a position, and the three questions around it.
//!
//! Java reference: `core/colony/ColonyManager.java` (875 lines) and
//! `api/colony/IColonyManager.java` (456 lines). This file is the server half —
//! `getColonyByPosFromWorld` (`:289-302`), `getBuilding` (`:263-286`),
//! `getClosestColony` (`:481-507`), `getIColonyByOwner` (`:547-564`),
//! `isCoordinateInAnyColony` (`:837-841`), `isFarEnoughFromColonies` (`:311-324`,
//! in `super::proximity`) and `deleteColony` (`:140-220`, in `super::lifecycle`).
//! The client half — the view cache, the packet handlers, the sound manager — has
//! nothing to answer in a port with no view packets (`MANQUANT.md` §2).
//!
//! **The one question everything else asks.** In Java a position is resolved by
//! reading the owning colony off the *chunk's* capability and then looking that id
//! up (`ColonyUtils.getOwningColony`, `api/util/ColonyUtils.java:170-175`). The
//! port reads the same id out of [`super::claims::ClaimBook`] and then out of the
//! colony store, and it names the two ways that can fail instead of answering
//! `null`:
//!
//! * [`LookupRefusal::NoColonyAt`] — nobody claimed that chunk;
//! * [`LookupRefusal::StaleOwner`] — a chunk names an id the store does not hold,
//!   which is what a save deleted under a chunk that kept its claims looks like.
//!
//! **`founding::Registry` is the store, not a copy of it.** The ids, the freed-id
//! list and the "one colony per id" refusal are `ColonyList`'s (`ColonyList.java:63-71`,
//! `:85-113`, `:148-162`) and were ported there; the manager adds the claims and
//! answers `founding::Colonies` over both, so a founding rule and a position lookup
//! cannot disagree about which colony is where.

use std::collections::BTreeMap;

use crate::colony::{Building, Colony};
use crate::founding::{Colonies, ColonyAt, Founded, Registry, Rules, Write};

use super::claims::{Chunk, ClaimBook, ChunkClaims, RemoveOutcome};
use super::lifecycle::{Attendance, Dissolution};
use super::proximity::Proximity;
use super::NO_COLONY_ID;

/// Why a position, an id or a chunk has no answer. Java answers `null` from
/// `getColonyByPosFromWorld` (`:289-302`), `getBuilding` (`:263-286`),
/// `getColonyByOwner` (`:547-564`) and silently does nothing when a deletion names
/// an id nobody holds (`:142-145`); the port names the failure in each case.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum LookupRefusal {
    /// No chunk there names a colony: the position is outside every claim.
    NoColonyAt { dimension: String, at: [i32; 3] },
    /// The chunk names a colony the store does not hold — a deleted colony whose
    /// claims outlived it, which Java's `range * 2` unclaim (`:159`) can leave.
    StaleOwner { dimension: String, chunk: Chunk, id: i32 },
    /// No colony carries that id.
    UnknownColony { id: i32 },
    /// An id a colony may not have: `0` is the tombstone, the id
    /// `Dissolution::tombstone` writes so a deleted record reads as no colony
    /// (`NO_COLONY_ID`), and the same value `getOwningColony` answers for an
    /// unowned chunk. A colony of id `0` would be indistinguishable from both.
    InvalidId { id: i32 },
    /// The id exists, in another dimension: `getColonyByDimension` asks that
    /// world's capability and finds nothing (`:244-260`).
    WrongDimension { id: i32, expected: String, asked: String },
    /// Two colonies with one id, which `ColonyList.add` refuses and logs
    /// (`:85-95`).
    IdTaken { id: i32 },
    /// No building stands at that position, in the colony there or in any other
    /// of the dimension (`getBuilding`, `:263-286`).
    NoBuilding { dimension: String, at: [i32; 3] },
}

impl LookupRefusal {
    /// A stable name, namespaced like a registry entry so a log line can be
    /// searched for it and never confuses it with a player-facing sentence.
    pub fn name(&self) -> &'static str {
        match self {
            Self::NoColonyAt { .. } => "manager.refusal.no_colony_at",
            Self::StaleOwner { .. } => "manager.refusal.stale_owner",
            Self::UnknownColony { .. } => "manager.refusal.unknown_colony",
            Self::InvalidId { .. } => "manager.refusal.invalid_id",
            Self::WrongDimension { .. } => "manager.refusal.wrong_dimension",
            Self::IdTaken { .. } => "manager.refusal.id_taken",
            Self::NoBuilding { .. } => "manager.refusal.no_building",
        }
    }
}

impl std::fmt::Display for LookupRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for LookupRefusal {}

/// The colonies of one server, their claims, and the two per-colony facts the
/// manager keeps beside them: how long nobody has come, and whether the colony may
/// be auto-deleted (`Colony.canBeAutoDeleted`, `core/colony/Colony.java:1361-1364`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Manager {
    records: Registry,
    claims: ClaimBook,
    attendance: BTreeMap<i32, Attendance>,
    auto_deletable: BTreeMap<i32, bool>,
}

impl Manager {
    pub fn new() -> Manager {
        Manager::default()
    }

    /// The colony records, ids and the freed-id list (`founding::Registry`).
    pub fn records(&self) -> &Registry {
        &self.records
    }

    /// The chunk claims.
    pub fn claims(&self) -> &ClaimBook {
        &self.claims
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Every colony, id ascending: the order `ColonyList` iterates
    /// (`ColonyList.java:259-273`) and the order a save is written in.
    pub fn colonies(&self) -> impl Iterator<Item = &Colony> + '_ {
        let mut ids = Colonies::ids(self);
        ids.sort_unstable();
        ids.into_iter().filter_map(|id| self.records.get(id))
    }

    /// Adds a colony and starts its away timer at zero, which is what
    /// `colony.getPermissions().setOwner(player)` leaves behind
    /// (`ColonyManager.java:105`). An id of `0` is refused: that is the tombstone
    /// `Dissolution::tombstone` writes and the value an unowned chunk answers, so a
    /// colony carrying it could never be told from either.
    pub fn insert(&mut self, colony: Colony) -> Result<(), LookupRefusal> {
        let id = colony.id;
        if id <= NO_COLONY_ID {
            return Err(LookupRefusal::InvalidId { id });
        }
        if !self.records.insert(colony) {
            return Err(LookupRefusal::IdTaken { id });
        }
        self.attendance.insert(id, Attendance::new());
        self.auto_deletable.insert(id, true);
        Ok(())
    }

    /// Takes a colony out of the list and frees its id, nothing else: the list
    /// operation of `ColonyList.remove` (`:148-162`), which
    /// `IColonyManagerCapability.deleteColony` is (`:88-91`). The rest of a deletion
    /// is [`Manager::dissolve`].
    pub fn remove(&mut self, id: i32) -> Result<Colony, LookupRefusal> {
        let colony = self
            .records
            .remove(id)
            .ok_or(LookupRefusal::UnknownColony { id })?;
        self.attendance.remove(&id);
        self.auto_deletable.remove(&id);
        Ok(colony)
    }

    /// A colony by id, whatever dimension it is in (`getColonyByWorld`, `:233-242`).
    pub fn record(&self, id: i32) -> Result<&Colony, LookupRefusal> {
        self.records.get(id).ok_or(LookupRefusal::UnknownColony { id })
    }

    /// A colony by id, in one dimension (`getColonyByDimension`, `:244-260`).
    pub fn record_in(&self, dimension: &str, id: i32) -> Result<&Colony, LookupRefusal> {
        let colony = self.record(id)?;
        if colony.dimension != dimension {
            return Err(LookupRefusal::WrongDimension {
                id,
                expected: colony.dimension.clone(),
                asked: dimension.to_owned(),
            });
        }
        Ok(colony)
    }

    /// The raw claim: which id a chunk names, live or not. This is
    /// `ColonyUtils.getOwningColony` without the follow-up lookup.
    pub fn owner_at(&self, dimension: &str, at: [i32; 3]) -> Option<i32> {
        self.claims.owner(dimension, Rules::chunk(at))
    }

    /// **The question.** `getColonyByPosFromWorld` (`:289-302`): the owning id of
    /// the chunk, then the record. A position in a chunk nobody owns, and a chunk
    /// whose colony is gone, are two different refusals.
    pub fn record_at(&self, dimension: &str, at: [i32; 3]) -> Result<&Colony, LookupRefusal> {
        let chunk = Rules::chunk(at);
        let Some(id) = self.claims.owner(dimension, chunk) else {
            return Err(LookupRefusal::NoColonyAt {
                dimension: dimension.to_owned(),
                at,
            });
        };
        self.records
            .get(id)
            .ok_or_else(|| LookupRefusal::StaleOwner {
                dimension: dimension.to_owned(),
                chunk,
                id,
            })
    }

    /// The colony an entity stands in. **Java has no separate rule for entities**:
    /// they ask with their block position (`ClientEventHandler.java:198`,
    /// `EventHandler.java:830`), and a citizen carries its own colony id instead. The
    /// name is here so a caller does not have to remember that.
    pub fn entity_record(&self, dimension: &str, at: [i32; 3]) -> Result<&Colony, LookupRefusal> {
        self.record_at(dimension, at)
    }

    /// `isCoordinateInAnyColony` (`:837-841`): a claim there, live or not.
    pub fn is_in_any_colony(&self, dimension: &str, at: [i32; 3]) -> bool {
        self.owner_at(dimension, at).is_some()
    }

    /// `getBuilding` (`:263-286`): the colony at the position first, then **every
    /// colony of the dimension**, because a hut may stand outside the box its own
    /// colony claimed — Java's own comment says so at `:275`.
    pub fn building_at(
        &self,
        dimension: &str,
        at: [i32; 3],
    ) -> Result<(i32, &Building), LookupRefusal> {
        if let Ok(colony) = self.record_at(dimension, at) {
            if let Some(building) = colony.building_at(at) {
                return Ok((colony.id, building));
            }
        }
        for colony in self.colonies() {
            if colony.dimension != dimension {
                continue;
            }
            if let Some(building) = colony.building_at(at) {
                return Ok((colony.id, building));
            }
        }
        Err(LookupRefusal::NoBuilding {
            dimension: dimension.to_owned(),
            at,
        })
    }

    /// `getClosestColony` (`:481-507`): **the owning colony of the chunk wins
    /// outright**, whatever the distance; only an unowned position scans for the
    /// nearest colony of this dimension. Ties keep the first sealed one, so the
    /// lowest id answers (`:498`, `:501`).
    pub fn closest(&self, dimension: &str, at: [i32; 3]) -> Option<ColonyAt<'_>> {
        if let Ok(colony) = self.record_at(dimension, at) {
            return Some(ColonyAt::of(colony));
        }
        Colonies::closest(&self.records, dimension, at)
    }

    /// The colony a player owns, in any dimension (`getIColonyByOwner`, `:547-564`).
    pub fn owned_by(&self, player: u128) -> Option<ColonyAt<'_>> {
        Colonies::owned_by(&self.records, player)
    }

    /// The id the next colony takes (`ColonyList.getNextColonyID`, `:63-71`).
    pub fn next_id(&self) -> i32 {
        Colonies::next_id(&self.records)
    }

    /// Claims a square box of chunks for `colony`: `claimColonyChunks`
    /// (`ChunkDataHelper.java:135-139`) over `staticClaimInRange` (`:321-348`), as
    /// one static claim per chunk. Answers how many chunks changed owner.
    pub fn claim_ground(
        &mut self,
        dimension: &str,
        center: [i32; 3],
        radius: i32,
        id: i32,
    ) -> Result<usize, LookupRefusal> {
        if self.records.get(id).is_none() {
            return Err(LookupRefusal::UnknownColony { id });
        }
        let mut claimed = 0;
        for chunk in Proximity::chunks_in_range(center, radius) {
            if self.claim_static_chunk(dimension, chunk, id) {
                claimed += 1;
            }
        }
        Ok(claimed)
    }

    /// One static claim. The repair of `addColony` (`IColonyTagCapability.java:134-141`)
    /// needs to know whether the recorded owner still exists, which the book cannot
    /// know and the manager can.
    fn claim_static_chunk(&mut self, dimension: &str, chunk: Chunk, id: i32) -> bool {
        let owner_live = self
            .claims
            .owner(dimension, chunk)
            .map(|owner| self.records.get(owner).is_some())
            .unwrap_or(true);
        self.claims.entry(dimension, chunk).add_static_claim(id, owner_live)
    }

    /// One building's claim on the chunk its plot covers (`claimBuildingChunks`,
    /// `ChunkDataHelper.java:152-162` → `addBuildingClaim`, `:185-218`). The ceiling
    /// of [`Proximity::may_claim`] is the caller's: it is a rule about a position,
    /// not about the book.
    pub fn claim_building(
        &mut self,
        dimension: &str,
        at: [i32; 3],
        id: i32,
    ) -> Result<(), LookupRefusal> {
        if self.records.get(id).is_none() {
            return Err(LookupRefusal::UnknownColony { id });
        }
        self.claims
            .entry(dimension, Rules::chunk(at))
            .add_building_claim(id, at);
        Ok(())
    }

    /// One building's claim removed, with the owner falling to a colony whose
    /// building still stands (`removeBuildingClaim`, `:220-283`).
    pub fn release_building(&mut self, dimension: &str, at: [i32; 3], id: i32) -> RemoveOutcome {
        let records = &self.records;
        let standing = move |colony: i32, position: [i32; 3]| {
            records
                .get(colony)
                .and_then(|record| record.building_at(position))
                .is_some()
        };
        let chunk = Rules::chunk(at);
        let outcome = match self.claims.get_mut(dimension, chunk) {
            Some(claims) => claims.remove_building_claim(id, at, &standing),
            None => RemoveOutcome::Absent,
        };
        self.claims.prune();
        outcome
    }

    /// Unclaims a box of chunks for one colony, answering the chunks it held. The
    /// removal branch of `claimColonyChunks` (`:135-139`) with Java's doubled radius.
    pub fn release_ground(
        &mut self,
        dimension: &str,
        center: [i32; 3],
        radius: i32,
        id: i32,
    ) -> Vec<Chunk> {
        let mut held = Vec::new();
        for chunk in Proximity::chunks_in_range(center, radius) {
            let names_it = self.claims.get(dimension, chunk).is_some_and(|claims| {
                claims.owner() == Some(id) || claims.is_static_claim(id)
            });
            if !names_it {
                continue;
            }
            if let Some(claims) = self.claims.get_mut(dimension, chunk) {
                claims.remove_static_claim(id);
            }
            held.push(chunk);
        }
        self.claims.prune();
        held
    }

    /// Takes one colony out of every chunk of a dimension, answering the chunks it
    /// held. See [`ClaimBook::release_colony`] for why this exists on top of the box.
    pub fn release_colony(&mut self, dimension: &str, id: i32) -> Vec<Chunk> {
        let records = &self.records;
        let standing = move |colony: i32, position: [i32; 3]| {
            records
                .get(colony)
                .and_then(|record| record.building_at(position))
                .is_some()
        };
        self.claims.release_colony(dimension, id, &standing)
    }

    /// Applies the journal a founding wrote: the record, then the ground
    /// (`ColonyManager.createColony`, `:101-118`). Read-only on `founded`, so a
    /// refused settlement leaves the manager untouched — the insert happens first,
    /// and a claim that fails afterwards cannot fail (the id is known).
    ///
    /// A journal with no [`Write::GroundClaimed`] registers the colony and claims
    /// nothing: the position rule then falls back to the colony's own box
    /// ([`Manager::owns_chunk`]), which is the floor a colony never falls below.
    pub fn settle(&mut self, founded: &Founded) -> Result<usize, LookupRefusal> {
        let colony = &founded.colony;
        let radius = founded.journal().iter().find_map(|write| match write {
            Write::GroundClaimed { id, chunks } if *id == colony.id => Some(*chunks),
            _ => None,
        });
        self.insert(colony.clone())?;
        match radius {
            Some(radius) => self.claim_ground(&colony.dimension, colony.center, radius, colony.id),
            None => Ok(0),
        }
    }

    /// Deletes a colony and answers **what the deletion takes down**, in Java's
    /// order: the ground (`:159`), the citizens (`:161-165`), the buildings
    /// (`:168-189`), then the record and its id (`:200-214`).
    ///
    /// Nothing here touches a world: the returned [`Dissolution`] is the work, and
    /// `Dissolution::tombstone` is the record to leave behind. Java's last three
    /// steps — the deleted event, the `.deleted` file, the view-remove packet
    /// (`:209-213`) — have no counterpart in a port with no event bus, no save file
    /// and no views (`MANQUANT.md` §2, §4).
    pub fn dissolve(
        &mut self,
        id: i32,
        can_destroy: bool,
        proximity: &Proximity,
    ) -> Result<Dissolution, LookupRefusal> {
        let colony = self.record(id)?.clone();
        let dimension = colony.dimension.clone();
        let mut chunks = self.release_ground(&dimension, colony.center, proximity.release_radius(), id);
        for chunk in self.release_colony(&dimension, id) {
            if !chunks.contains(&chunk) {
                chunks.push(chunk);
            }
        }
        chunks.sort_unstable();
        chunks.dedup();
        self.remove(id)?;
        Ok(Dissolution {
            id,
            dimension,
            center: colony.center,
            chunks,
            citizens: colony.citizens,
            buildings: colony.buildings,
            cleared: can_destroy,
            freed_id: self.records.next_id(),
        })
    }

    /// The away timer of one colony.
    pub fn attendance(&self, id: i32) -> Option<&Attendance> {
        self.attendance.get(&id)
    }

    /// The saved hour count (`Colony.java:993`).
    pub fn hours(&self, id: i32) -> Option<i32> {
        self.attendance.get(&id).map(Attendance::hours)
    }

    /// Puts a saved hour count back (`Colony.java:828`).
    pub fn set_hours(&mut self, id: i32, hours: i32) {
        if self.attendance.contains_key(&id) {
            self.attendance.insert(id, Attendance::from_hours(hours));
        }
    }

    /// One away-timer update for one colony (`ColonyPackageManager.java:141-160`).
    /// Answers whether the saved count moved.
    pub fn update_away_time(&mut self, id: i32, watched: bool) -> Result<bool, LookupRefusal> {
        let attendance = self
            .attendance
            .get_mut(&id)
            .ok_or(LookupRefusal::UnknownColony { id })?;
        Ok(attendance.update(watched))
    }

    /// The same for every colony, `watched` deciding per colony whether somebody is
    /// near it (Java's `importantColonyPlayers` being empty, `:143`). Answers the
    /// colonies whose hour count moved, ascending.
    pub fn tick_all(&mut self, watched: &dyn Fn(i32) -> bool) -> Vec<i32> {
        let ids: Vec<i32> = self.attendance.keys().copied().collect();
        let mut changed = Vec::new();
        for id in ids {
            let near = watched(id);
            if let Some(attendance) = self.attendance.get_mut(&id) {
                if attendance.update(near) {
                    changed.push(id);
                }
            }
        }
        changed
    }

    /// `getColoniesAbandonedSince` (`:353-365`): every colony whose last contact is
    /// **at least** that many hours old, id ascending. Nothing calls it in the Java
    /// tree of this version (`super::lifecycle`), so nothing deletes on its own here
    /// either.
    pub fn abandoned_since(&self, hours: i32) -> Vec<i32> {
        let mut ids: Vec<i32> = self
            .attendance
            .iter()
            .filter(|(_, attendance)| attendance.hours() >= hours)
            .map(|(id, _)| *id)
            .collect();
        ids.sort_unstable();
        ids
    }

    /// `Colony.canBeAutoDeleted` (`core/colony/Colony.java:1361-1364`).
    pub fn can_be_auto_deleted(&self, id: i32) -> Option<bool> {
        self.auto_deletable.get(&id).copied()
    }

    /// Sets it, as `/colony setdeletable` does (`CommandSetDeletable`).
    pub fn set_auto_deletable(&mut self, id: i32, allowed: bool) {
        if self.auto_deletable.contains_key(&id) {
            self.auto_deletable.insert(id, allowed);
        }
    }

    /// Puts a chunk's claims back wholesale: the read path of [`super::persist`].
    /// A chunk whose colony the store does not hold is kept as it is, so a
    /// [`LookupRefusal::StaleOwner`] reports the save instead of the read erasing it.
    pub fn insert_claims(&mut self, dimension: &str, chunk: Chunk, claims: ChunkClaims) {
        self.claims.insert(dimension, chunk, claims);
    }

    /// Drops the chunk entries that claim nothing. Answers how many went.
    pub fn prune(&mut self) -> usize {
        self.claims.prune()
    }
}

impl Colonies for Manager {
    fn ids(&self) -> Vec<i32> {
        Colonies::ids(&self.records)
    }

    fn colony(&self, id: i32) -> Option<ColonyAt<'_>> {
        Colonies::colony(&self.records, id)
    }

    fn next_id(&self) -> i32 {
        Colonies::next_id(&self.records)
    }

    /// `getClosestColony`, owner first (see [`Manager::closest`]).
    ///
    /// **This overrides the trait's default, on purpose, and `founding::found`
    /// notices.** The default is the pure distance scan; Java's `getClosestColony` —
    /// the method `CreateColonyMessage.java:115` calls before a founding — reads the
    /// chunk's owner first (`:483-488`). A store that answered the pure scan would
    /// make a founding measure against a different neighbour than Java does.
    fn closest(&self, dimension: &str, at: [i32; 3]) -> Option<ColonyAt<'_>> {
        Manager::closest(self, dimension, at)
    }

    /// Which colony owns that chunk. **A recorded claim answers; the box is the
    /// floor.** The claim book is the truth Java reads off the chunk, and the box
    /// `founding::Colonies` computes is used only for a chunk nobody recorded —
    /// which is every chunk of a colony whose claims were never saved, and what
    /// keeps a record read out of a block entity (no claims beside it) from
    /// answering "nobody" for its own town hall.
    ///
    /// Java's answer is the claim alone (`canClaimChunksInRange`, `:200-203`), so
    /// the fallback is stricter than Java: it refuses a founding next to a colony
    /// whose claims are missing, where Java would allow it.
    fn owns_chunk(&self, dimension: &str, chunk: Chunk, radius: i32) -> Option<i32> {
        if let Some(id) = self.claims.owner(dimension, chunk) {
            return Some(id);
        }
        Colonies::owns_chunk(&self.records, dimension, chunk, radius)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::founding::{found, Founder, Founding, Ground, Refusal, Registry};

    const OWNER: u128 = 1;
    const FRIEND: u128 = 2;
    const OVERWORLD: &str = "minecraft:overworld";
    const NETHER: &str = "minecraft:the_nether";
    const HUT: [i32; 3] = [10, 64, 10];
    /// A town hall 141 blocks from the spawn at the origin: past the 128-block
    /// range, so a founding there is allowed (`founding/found.rs` uses the same spot).
    const HALL: [i32; 3] = [100, 70, 100];
    const SPAWN: [i32; 3] = [0, 70, 0];

    fn colony(id: i32, dimension: &str, center: [i32; 3]) -> Colony {
        Colony::new(id, "Bridgewater", dimension, center, OWNER)
    }

    /// A manager with one colony at the origin, its box claimed.
    fn manager() -> Manager {
        let mut manager = Manager::new();
        manager.insert(colony(1, OVERWORLD, [0, 70, 0])).expect("a free id");
        manager
            .claim_ground(OVERWORLD, [0, 70, 0], 4, 1)
            .expect("a known colony");
        manager
    }

    #[test]
    fn a_new_colony_owns_nothing_until_the_ground_is_claimed() {
        let mut manager = Manager::new();
        manager.insert(colony(1, OVERWORLD, [0, 70, 0])).expect("a free id");
        assert_eq!(manager.claims().len(), 0, "no claim, no position");
        assert_eq!(
            manager.record_at(OVERWORLD, [0, 70, 0]).map(|colony| colony.id),
            Err(LookupRefusal::NoColonyAt {
                dimension: OVERWORLD.to_owned(),
                at: [0, 70, 0]
            })
        );
        assert_eq!(manager.claim_ground(OVERWORLD, [0, 70, 0], 0, 1), Ok(1));
        assert!(manager.is_in_any_colony(OVERWORLD, [0, 70, 0]));
    }

    #[test]
    fn a_claimed_chunk_answers_which_colony_owns_a_position() {
        let manager = manager();
        for at in [[0, 70, 0], [79, 12, 79], [-64, 255, -64], [0, 70, -64]] {
            assert_eq!(
                manager.record_at(OVERWORLD, at).map(|colony| colony.id),
                Ok(1),
                "{at:?} is inside the 4-chunk box"
            );
            assert_eq!(
                manager.entity_record(OVERWORLD, at).map(|colony| colony.id),
                Ok(1),
                "an entity asks with its block position"
            );
        }
        assert_eq!(
            manager.record_at(OVERWORLD, [80, 70, 0]).map(|colony| colony.id),
            Err(LookupRefusal::NoColonyAt {
                dimension: OVERWORLD.to_owned(),
                at: [80, 70, 0]
            }),
            "one block further is out"
        );
        assert!(!manager.is_in_any_colony(NETHER, [0, 70, 0]), "the same blocks, no colony");
    }

    #[test]
    fn the_box_a_claim_walks_is_the_radius_squared_in_chunks() {
        let mut manager = Manager::new();
        manager.insert(colony(1, OVERWORLD, [0, 70, 0])).expect("a free id");
        assert_eq!(
            manager.claim_ground(OVERWORLD, [0, 70, 0], 4, 1),
            Ok(81),
            "radius 4 is a 9x9 box, ChunkDataHelper.java:321-348"
        );
        assert_eq!(manager.claims().len(), 81);
        let mut manager = Manager::new();
        manager.insert(colony(1, OVERWORLD, [0, 70, 0])).expect("a free id");
        assert_eq!(manager.claim_ground(OVERWORLD, [0, 70, 0], 0, 1), Ok(1), "radius 0 is one chunk");
        assert_eq!(
            manager.claim_ground(OVERWORLD, [0, 70, 0], 0, 9),
            Err(LookupRefusal::UnknownColony { id: 9 }),
            "a claim needs a colony"
        );
    }

    #[test]
    fn a_chunk_whose_colony_is_gone_names_the_stale_owner() {
        let mut manager = Manager::new();
        let mut claims = ChunkClaims::new();
        claims.set_owning(Some(42));
        manager.insert_claims(OVERWORLD, [0, 0], claims);
        assert_eq!(manager.owner_at(OVERWORLD, [0, 70, 0]), Some(42), "the claim is kept");
        assert_eq!(
            manager.record_at(OVERWORLD, [0, 70, 0]).map(|colony| colony.id),
            Err(LookupRefusal::StaleOwner {
                dimension: OVERWORLD.to_owned(),
                chunk: [0, 0],
                id: 42
            })
        );
        assert_eq!(
            manager.closest(OVERWORLD, [0, 70, 0]),
            None,
            "a stale owner is no closest colony, so the distance scan answers"
        );
    }

    #[test]
    fn a_colony_of_another_dimension_is_a_named_refusal() {
        let manager = manager();
        assert_eq!(manager.record_in(OVERWORLD, 1).map(|colony| colony.id), Ok(1));
        assert_eq!(
            manager.record_in(NETHER, 1).map(|colony| colony.id),
            Err(LookupRefusal::WrongDimension {
                id: 1,
                expected: OVERWORLD.to_owned(),
                asked: NETHER.to_owned()
            })
        );
        assert_eq!(
            manager.record(9).map(|colony| colony.id),
            Err(LookupRefusal::UnknownColony { id: 9 })
        );
    }

    #[test]
    fn the_second_colony_of_one_id_is_refused_and_the_first_one_stands() {
        let mut manager = manager();
        assert_eq!(
            manager.insert(colony(1, NETHER, [0, 70, 0])),
            Err(LookupRefusal::IdTaken { id: 1 }),
            "ColonyList.add refuses the clash and logs it, ColonyList.java:85-95"
        );
        assert_eq!(manager.len(), 1);
        assert_eq!(manager.record(1).map(|colony| colony.dimension.as_str()), Ok(OVERWORLD));
        assert_eq!(
            manager.insert(colony(0, OVERWORLD, [0, 70, 0])),
            Err(LookupRefusal::InvalidId { id: 0 }),
            "id 0 is the tombstone, not a colony"
        );
        assert_eq!(LookupRefusal::InvalidId { id: 0 }.to_string(), "manager.refusal.invalid_id");
    }

    #[test]
    fn a_building_is_found_through_the_colony_there_and_then_in_the_whole_dimension() {
        let mut manager = Manager::new();
        let mut home = colony(1, OVERWORLD, [0, 70, 0]);
        home.buildings.push(Building::new("minecolonies:blockhutbuilder", HUT, 1));
        manager.insert(home).expect("a free id");
        manager.claim_ground(OVERWORLD, [0, 70, 0], 0, 1).expect("a known colony");
        assert_eq!(manager.building_at(OVERWORLD, HUT).map(|(id, _)| id), Ok(1));

        // A hut outside its own colony's box: getBuilding's fallback, :275-283.
        let far = [900, 64, 900];
        let mut other = colony(2, OVERWORLD, [880, 70, 880]);
        other.buildings.push(Building::new("minecolonies:blockhutfarm", far, 3));
        manager.insert(other).expect("a free id");
        assert!(!manager.is_in_any_colony(OVERWORLD, far));
        let (id, building) = manager.building_at(OVERWORLD, far).expect("the fallback finds it");
        assert_eq!((id, building.kind.as_str()), (2, "minecolonies:blockhutfarm"));
        assert_eq!(
            manager.building_at(OVERWORLD, [1, 2, 3]).map(|(id, _)| id),
            Err(LookupRefusal::NoBuilding {
                dimension: OVERWORLD.to_owned(),
                at: [1, 2, 3]
            })
        );
        assert_eq!(
            manager.building_at(NETHER, HUT).map(|(id, _)| id),
            Err(LookupRefusal::NoBuilding {
                dimension: NETHER.to_owned(),
                at: HUT
            }),
            "a building is not reachable from another dimension"
        );
    }

    #[test]
    fn the_owner_of_the_chunk_wins_over_a_nearer_colony_that_does_not_own_it() {
        let mut manager = Manager::new();
        manager.insert(colony(1, OVERWORLD, [0, 70, 0])).expect("a free id");
        manager.insert(colony(2, OVERWORLD, [500, 70, 0])).expect("a free id");
        // Only colony 2 claimed the chunk at the origin, which is 500 blocks from
        // its own centre: getClosestColony reads the chunk first, :483-488.
        manager
            .claim_ground(OVERWORLD, [0, 70, 0], 0, 2)
            .expect("a known colony");
        assert_eq!(
            manager.closest(OVERWORLD, [0, 70, 0]).map(|colony| colony.id),
            Some(2),
            "the owning colony, distance and all"
        );
        assert_eq!(
            manager.closest(OVERWORLD, [40, 70, 0]).map(|colony| colony.id),
            Some(1),
            "nobody owns that chunk, so the nearest colony answers: 40 blocks away"
        );
        assert_eq!(
            manager.closest(NETHER, [0, 70, 0]),
            None,
            "no colony of that dimension at all"
        );
        assert_eq!(manager.owned_by(OWNER).map(|colony| colony.id), Some(1));
        assert_eq!(manager.owned_by(FRIEND), None);
    }

    #[test]
    fn the_next_id_is_the_lowest_freed_one_and_a_deletion_frees_it() {
        let mut manager = Manager::new();
        for id in 1..=3 {
            manager.insert(colony(id, OVERWORLD, [id * 400, 70, 0])).expect("a free id");
        }
        assert_eq!(manager.next_id(), 4, "one past the highest ever handed out");
        assert_eq!(manager.remove(2).map(|colony| colony.id), Ok(2));
        assert_eq!(manager.next_id(), 2, "the freed id comes back first");
        assert_eq!(manager.attendance(2), None, "a removed colony has no away timer");
        assert_eq!(
            manager.remove(2).map(|colony| colony.id),
            Err(LookupRefusal::UnknownColony { id: 2 })
        );
    }

    #[test]
    fn a_deletion_unclaims_the_ground_and_answers_what_it_takes_down() {
        let mut manager = Manager::new();
        let mut hall = colony(1, OVERWORLD, [0, 70, 0]);
        hall.citizens.push(crate::colony::Citizen::new(1, "Ada"));
        hall.buildings.push(Building::new("minecolonies:blockhuttownhall", [0, 70, 0], 1));
        manager.insert(hall).expect("a free id");
        manager.claim_ground(OVERWORLD, [0, 70, 0], 4, 1).expect("a known colony");
        // A claim outside the founding box, which Java's 2x box would leave behind.
        manager
            .claim_ground(OVERWORLD, [400, 70, 400], 0, 1)
            .expect("a known colony");
        let dissolution = manager
            .dissolve(1, true, &Proximity::DEFAULT)
            .expect("the colony exists");
        assert_eq!(dissolution.id, 1);
        assert_eq!(dissolution.dimension, OVERWORLD);
        assert_eq!(dissolution.chunks.len(), 82, "the 8-chunk box plus the far chunk");
        assert_eq!(dissolution.chunks.last(), Some(&[25, 25]), "x then z, sorted");
        assert_eq!(dissolution.citizens.len(), 1);
        assert_eq!(dissolution.buildings.len(), 1);
        assert!(dissolution.cleared, "canDestroy was asked for");
        assert_eq!(dissolution.freed_id, 1, "the only id, freed and handed back");
        assert!(manager.is_empty());
        assert_eq!(manager.claims().len(), 0, "no claim outlives the record");
        assert!(!manager.is_in_any_colony(OVERWORLD, [0, 70, 0]));
        assert_eq!(Colony::from_compound(&dissolution.tombstone()).id, 0);
        assert_eq!(
            manager.dissolve(1, false, &Proximity::DEFAULT).map(|d| d.id),
            Err(LookupRefusal::UnknownColony { id: 1 })
        );
    }

    #[test]
    fn the_away_timer_counts_per_colony_and_names_the_abandoned_ones() {
        let mut manager = Manager::new();
        manager.insert(colony(1, OVERWORLD, [0, 70, 0])).expect("a free id");
        manager.insert(colony(2, OVERWORLD, [400, 70, 0])).expect("a free id");
        manager.set_hours(1, 12);
        assert_eq!(manager.hours(1), Some(12));
        assert_eq!(manager.abandoned_since(12), vec![1], "at least twelve hours, >= in Java :358");
        assert_eq!(manager.abandoned_since(13), Vec::<i32>::new());
        assert_eq!(
            manager.update_away_time(1, false),
            Ok(false),
            "twelve hours and one update is still twelve"
        );
        assert_eq!(
            manager.update_away_time(9, false),
            Err(LookupRefusal::UnknownColony { id: 9 })
        );
        assert_eq!(
            manager.tick_all(&|id| id == 2),
            Vec::<i32>::new(),
            "colony 2 is being watched and has nothing to save"
        );
        manager.set_hours(2, 3);
        assert_eq!(manager.tick_all(&|id| id == 2), vec![2], "and a watcher resets it");
        assert_eq!(manager.hours(2), Some(0));
        assert_eq!(manager.can_be_auto_deleted(1), Some(true), "autoDelete defaults to true");
        manager.set_auto_deletable(1, false);
        assert_eq!(manager.can_be_auto_deleted(1), Some(false));
        assert_eq!(manager.can_be_auto_deleted(9), None, "a colony nobody has is not deletable");
    }

    #[test]
    fn a_dynamic_claim_outside_the_box_refuses_a_founding_there() {
        let mut manager = Manager::new();
        manager.insert(colony(1, OVERWORLD, [0, 70, 0])).expect("a free id");
        // A building 320 blocks east claims its chunk: dynamic claiming, well
        // outside the 4-chunk box of colony 1's founding.
        manager
            .claim_building(OVERWORLD, [320, 70, 0], 1)
            .expect("a known colony");
        assert_eq!(manager.owner_at(OVERWORLD, [320, 70, 0]), Some(1));
        assert_eq!(manager.closest(OVERWORLD, [320, 70, 0]).map(|colony| colony.id), Some(1));
        assert_eq!(
            Proximity::DEFAULT.may_settle(&manager, OVERWORLD, [300, 70, 0]),
            Err(Refusal::GroundClaimed {
                chunk: [20, 0],
                id: 1
            }),
            "the claim answers where the founding box does not: that is the override"
        );
        assert_eq!(
            Proximity::DEFAULT.may_settle(&manager, OVERWORLD, [0, 70, 0]),
            Err(Refusal::TooCloseToColony {
                id: 1,
                distance: 0,
                required: 128
            }),
            "and the distance check comes first, as in ColonyManager.java:313-320"
        );
    }

    /// The world a founding runs in: its town hall is where the click says.
    struct Hall;
    impl Ground for Hall {
        fn is_free(&self, _at: [i32; 3]) -> bool {
            true
        }

        fn holds_town_hall(&self, _at: [i32; 3]) -> bool {
            true
        }
    }

    const WITH_HALL: Hall = Hall;

    #[test]
    fn a_founding_is_settled_by_its_journal_and_registers_and_claims_the_ground() {
        let registry = Registry::new();
        let founding = Founding::new(
            Founder::new(OWNER, "Simon"),
            HALL,
            OVERWORLD,
            SPAWN,
            &WITH_HALL,
            &registry,
        );
        let founded = found(&founding).expect("an empty world, a free spot");
        let mut manager = Manager::new();
        assert_eq!(
            manager.settle(&founded),
            Ok(81),
            "Write::GroundClaimed carries the radius, and radius 4 is 81 chunks"
        );
        assert_eq!(manager.len(), 1);
        assert_eq!(
            manager.record_at(OVERWORLD, HALL).map(|colony| colony.id),
            Ok(founded.colony.id)
        );
        assert_eq!(manager.owned_by(OWNER).map(|colony| colony.id), Some(founded.colony.id));
        // The same founding twice: the id is taken, and the manager says which.
        assert_eq!(
            manager.settle(&founded),
            Err(LookupRefusal::IdTaken { id: founded.colony.id })
        );
    }
}
