//! The claims one chunk carries: who owns it, and which buildings put a colony there.
//!
//! Java reference: `api/colony/IColonyTagCapability.java` — the three fields
//! (`:111-125`), `addColony` (`:128-144`), `removeColony` (`:146-168`),
//! `addBuildingClaim` (`:185-218`), `removeBuildingClaim` (`:220-283`),
//! `setOwningColony` (`:285-290`), `setStaticColonyClaim` (`:170-174`) and the NBT
//! layout of `Storage.writeNBT` (`:331-360`).
//!
//! **The port keeps the book, Java keeps it on the chunk.** In Java these fields
//! are a capability *attached to the `LevelChunk`*, saved with the chunk and
//! applied on load (`:295-310`). The SDK exposes a chunk handle with nowhere to
//! hang a mod's data (`crates/horizon-mod-vocab/src/host/levels.rs:13-17`) and no
//! chunk NBT at all, so the port stores the same three fields itself, keyed by
//! dimension and chunk, and saves them beside the colonies (`super::persist`).
//! Every rule below is Java's, moved from the chunk to the book; the two places the
//! port answers differently are named in the method that answers them.
//!
//! **A chunk is claimed twice over.** A colony owns a box the day it is founded
//! (`ChunkDataHelper.java:135-139`, the *static* claim), and a building claims the
//! chunks its plot covers as it is built or upgraded (`claimBuildingChunks`,
//! `:152-162`, the *dynamic* claim). Java keeps both on the same chunk and lets the
//! owner decide who answers `getOwningColony()`.

use std::collections::{BTreeMap, BTreeSet};

use super::NO_COLONY_ID;

/// A chunk's coordinates: Java's `ChunkPos`, the two axes that exist. Block `x`
/// is in chunk `x >> 4` (`ChunkDataHelper.java:236-237`), which
/// `founding::Rules::chunk` computes with `div_euclid`.
pub type Chunk = [i32; 2];

/// One chunk's claims, the three fields of `IColonyTagCapability.Impl`
/// (`:111-125`). `owning` is `NO_COLONY_ID` when nobody owns it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct ChunkClaims {
    owning: i32,
    /// Java's `Set<Integer> colonies` (`:114`) — the static claims. A `BTreeSet`
    /// so the fallback at `:155` picks the lowest id instead of an arbitrary one.
    static_claims: BTreeSet<i32>,
    /// Java's `Map<Integer, Set<BlockPos>> claimingBuildings` (`:120`). The
    /// positions are the buildings, which is what lets `removeBuildingClaim` drop
    /// a claim whose building is gone (`:262-281`).
    buildings: BTreeMap<i32, BTreeSet<[i32; 3]>>,
}

/// What removing one building claim did to the chunk's owner. Java's
/// `removeBuildingClaim` answers nothing (`:220-283`); the port names the four
/// outcomes, because a caller that reinstates an owner has to know which one it got.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four ends of Java's one method
pub enum RemoveOutcome {
    /// That colony held no claim in this chunk: Java returns at `:222-225`.
    Absent,
    /// The claim is gone and the owner is unchanged.
    OwnerKept,
    /// The owner was that colony and moved to another one whose building still
    /// stands here (`:239-283`).
    OwnerMoved { to: i32 },
    /// Nothing claims this chunk any more (`:227-236`, the empty-list branch).
    OwnerCleared,
}

impl ChunkClaims {
    pub fn new() -> ChunkClaims {
        ChunkClaims::default()
    }

    /// The colony that owns this chunk, or `None`. Java answers
    /// `NO_COLONY_ID` (`getOwningColony`, `:292-297`), a sentinel a caller must
    /// remember to compare; the port makes "nobody" the type's job.
    pub fn owner(&self) -> Option<i32> {
        (self.owning != NO_COLONY_ID).then_some(self.owning)
    }

    /// The ids with a static claim, lowest first.
    pub fn static_claims(&self) -> impl Iterator<Item = i32> + '_ {
        self.static_claims.iter().copied()
    }

    pub fn is_static_claim(&self, id: i32) -> bool {
        self.static_claims.contains(&id)
    }

    /// Per colony, the buildings that claimed this chunk.
    pub fn claiming_buildings(&self) -> &BTreeMap<i32, BTreeSet<[i32; 3]>> {
        &self.buildings
    }

    /// Whether this chunk still says anything. Java never asks: an empty
    /// capability stays attached to the chunk. The book drops the entry instead,
    /// or a save would grow one empty record per chunk ever claimed.
    pub fn is_empty(&self) -> bool {
        self.owning == NO_COLONY_ID && self.static_claims.is_empty() && self.buildings.is_empty()
    }

    /// `addColony` (`:128-144`): the id joins the static claims, and takes the
    /// ownership when the chunk has none **or when the recorded owner no longer
    /// exists** — the repair Java makes with `getColonyByDimension(owningColony)
    /// == null` (`:134-141`). `owner_live` is that second half, decided by the
    /// caller that holds the records (`super::Manager`).
    pub fn add_static_claim(&mut self, id: i32, owner_live: bool) -> bool {
        self.static_claims.insert(id);
        if self.owning == NO_COLONY_ID || !owner_live {
            self.owning = id;
            return true;
        }
        false
    }

    /// The whole static list in one go, as `setStaticColonyClaim` (`:170-174`).
    /// No ownership is taken: Java's read repairs it afterwards (`:320-323`).
    pub fn set_static_claims(&mut self, ids: impl IntoIterator<Item = i32>) {
        self.static_claims = ids.into_iter().collect();
    }

    /// `removeColony` (`:146-168`): the id leaves both maps, and an owner that was
    /// that id falls back to a remaining building claim, then to a remaining static
    /// claim, then to nobody (`:150-167`). **Java picks `iterator().next()`** on a
    /// `HashSet` (`:155`, `:159`); the port picks the lowest id, so two runs of the
    /// same save answer the same.
    pub fn remove_static_claim(&mut self, id: i32) -> bool {
        self.static_claims.remove(&id);
        self.buildings.remove(&id);
        if self.owning != id {
            return false;
        }
        self.owning = self
            .buildings
            .keys()
            .next()
            .copied()
            .or_else(|| self.static_claims.iter().next().copied())
            .unwrap_or(NO_COLONY_ID);
        true
    }

    /// `addBuildingClaim` (`:185-218`): the building joins the colony's set and
    /// takes an ownerless chunk (`:197-205`), and does nothing at all when the
    /// chunk already has an owner. Answers whether the owner moved.
    pub fn add_building_claim(&mut self, id: i32, at: [i32; 3]) -> bool {
        let takes = self.owning == NO_COLONY_ID;
        if takes {
            self.owning = id;
        }
        self.buildings.entry(id).or_default().insert(at);
        takes
    }

    /// The same claim, put back without touching the owner: Java's `readClaims`
    /// (`:312-327`) appends to the map and nothing else.
    pub fn restore_building_claim(&mut self, id: i32, at: [i32; 3]) {
        self.buildings.entry(id).or_default().insert(at);
    }

    /// `removeBuildingClaim` (`:220-283`), the longest rule in the file.
    ///
    /// Once the colony's last claim here goes, an owner that was that colony and
    /// has no static claim here (`:237`) falls to the first remaining colony with a
    /// building that still stands (`:239-283`), and every claim whose building is
    /// gone is dropped on the way (`:277`). `standing(colony, position)` is that
    /// question, which the caller answers from the colony records.
    ///
    /// **The port clears an owner Java leaves dangling.** When the scan finds no
    /// standing building, Java exits the method with `owningColony` still naming
    /// the colony it just removed (`:239-283` sets the owner only inside the
    /// `if`); the port answers [`RemoveOutcome::OwnerCleared`] and sets nobody, so
    /// a deleted colony cannot keep answering for a chunk.
    pub fn remove_building_claim(
        &mut self,
        id: i32,
        at: [i32; 3],
        standing: &dyn Fn(i32, [i32; 3]) -> bool,
    ) -> RemoveOutcome {
        let Some(owned) = self.buildings.get_mut(&id) else {
            return RemoveOutcome::Absent;
        };
        owned.remove(&at);
        if !owned.is_empty() {
            return RemoveOutcome::OwnerKept;
        }
        self.buildings.remove(&id);
        if self.owning != id || self.static_claims.contains(&id) {
            return RemoveOutcome::OwnerKept;
        }
        if self.buildings.is_empty() {
            if let Some(static_owner) = self.static_claims.iter().next().copied() {
                self.owning = static_owner;
                return RemoveOutcome::OwnerMoved { to: static_owner };
            }
            self.owning = NO_COLONY_ID;
            return RemoveOutcome::OwnerCleared;
        }
        // Java iterates its `HashMap`; the port walks the ids in order, so the
        // same save answers the same colony on every machine.
        let candidates: Vec<i32> = self.buildings.keys().copied().collect();
        for candidate in candidates {
            let positions: Vec<[i32; 3]> = self
                .buildings
                .get(&candidate)
                .map(|set| set.iter().copied().collect())
                .unwrap_or_default();
            for position in positions {
                if standing(candidate, position) {
                    self.owning = candidate;
                    return RemoveOutcome::OwnerMoved { to: candidate };
                }
                if let Some(set) = self.buildings.get_mut(&candidate) {
                    set.remove(&position);
                }
            }
            if self.buildings.get(&candidate).is_some_and(BTreeSet::is_empty) {
                self.buildings.remove(&candidate);
            }
        }
        self.owning = NO_COLONY_ID;
        RemoveOutcome::OwnerCleared
    }

    /// `setOwningColony` (`:285-290`). `None` clears it, which Java spells `0`.
    pub fn set_owning(&mut self, id: Option<i32>) {
        self.owning = id.filter(|id| *id != NO_COLONY_ID).unwrap_or(NO_COLONY_ID);
    }
}

/// Every chunk the manager knows about, keyed by dimension and chunk.
///
/// The key order is `(dimension, x, z)`, so a save writes the same bytes for the
/// same world (`super::persist::write`). Java needs no such book: each chunk
/// carries its own claims and the world only has to load it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct ClaimBook {
    chunks: BTreeMap<(String, Chunk), ChunkClaims>,
}

impl ClaimBook {
    pub fn new() -> ClaimBook {
        ClaimBook::default()
    }

    /// How many chunks still claim something.
    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    pub fn get(&self, dimension: &str, chunk: Chunk) -> Option<&ChunkClaims> {
        self.chunks.get(&(dimension.to_owned(), chunk))
    }

    /// The claims of one chunk, created empty if this is the first claim there.
    pub fn entry(&mut self, dimension: &str, chunk: Chunk) -> &mut ChunkClaims {
        self.chunks
            .entry((dimension.to_owned(), chunk))
            .or_insert_with(ChunkClaims::new)
    }

    /// The claims of one chunk, when it has any.
    pub fn get_mut(&mut self, dimension: &str, chunk: Chunk) -> Option<&mut ChunkClaims> {
        self.chunks.get_mut(&(dimension.to_owned(), chunk))
    }

    /// Which colony owns that chunk, if any: the answer
    /// `ColonyUtils.getOwningColony` reads off the chunk capability
    /// (`api/util/ColonyUtils.java:170-175`).
    pub fn owner(&self, dimension: &str, chunk: Chunk) -> Option<i32> {
        self.get(dimension, chunk).and_then(ChunkClaims::owner)
    }

    /// Every chunk with its claims, in key order.
    pub fn chunks(&self) -> impl Iterator<Item = (&str, Chunk, &ChunkClaims)> + '_ {
        self.chunks
            .iter()
            .map(|((dimension, chunk), claims)| (dimension.as_str(), *chunk, claims))
    }

    /// Puts a chunk's claims back, replacing whatever was there: the read path of
    /// `super::persist`, and how a test builds a book no rule could produce.
    pub fn insert(&mut self, dimension: &str, chunk: Chunk, claims: ChunkClaims) -> Option<ChunkClaims> {
        self.chunks.insert((dimension.to_owned(), chunk), claims)
    }

    /// Drops the entries that claim nothing. Answers how many went.
    pub fn prune(&mut self) -> usize {
        let before = self.chunks.len();
        self.chunks.retain(|_, claims| !claims.is_empty());
        before - self.chunks.len()
    }

    /// Takes one colony out of every chunk of one dimension, answering the chunks
    /// it actually held.
    ///
    /// Java has no such sweep: a deleted colony is unclaimed by walking a box of
    /// **twice** its founding radius (`ChunkDataHelper.java:135-139`, the
    /// `range * 2` of the removal branch), and a claim outside that box survives the
    /// deletion, still naming an id the list has freed. The port keeps the box
    /// (`super::Proximity::release_radius`) **and** sweeps the book, so a deleted
    /// colony cannot answer for a chunk.
    pub fn release_colony(
        &mut self,
        dimension: &str,
        id: i32,
        standing: &dyn Fn(i32, [i32; 3]) -> bool,
    ) -> Vec<Chunk> {
        let mut held = Vec::new();
        let chunks: Vec<Chunk> = self
            .chunks
            .keys()
            .filter(|(dim, _)| dim.as_str() == dimension)
            .map(|(_, chunk)| *chunk)
            .collect();
        for chunk in chunks {
            let key = (dimension.to_owned(), chunk);
            let Some(claims) = self.chunks.get(&key) else {
                continue;
            };
            let names_it = claims.owner() == Some(id)
                || claims.is_static_claim(id)
                || claims.claiming_buildings().contains_key(&id);
            if !names_it {
                continue;
            }
            held.push(chunk);
            while let Some(position) = self
                .chunks
                .get(&key)
                .and_then(|claims| claims.claiming_buildings().get(&id))
                .and_then(|set| set.iter().next().copied())
            {
                if let Some(claims) = self.chunks.get_mut(&key) {
                    claims.remove_building_claim(id, position, standing);
                }
            }
            let remaining: Vec<i32> = self
                .chunks
                .get(&key)
                .map(|claims| claims.static_claims().filter(|held| *held != id).collect())
                .unwrap_or_default();
            if let Some(claims) = self.chunks.get_mut(&key) {
                claims.set_static_claims(remaining);
                if claims.owner() == Some(id) {
                    claims.set_owning(None);
                }
            }
            if self.chunks.get(&key).is_some_and(ChunkClaims::is_empty) {
                self.chunks.remove(&key);
            }
        }
        held
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OVERWORLD: &str = "minecraft:overworld";
    const NETHER: &str = "minecraft:the_nether";
    const HUT: [i32; 3] = [64, 70, 64];

    /// Every building stands: what a chunk sees when nothing was broken.
    fn standing(_colony: i32, _at: [i32; 3]) -> bool {
        true
    }

    /// No building stands: what a chunk sees once the huts are gone.
    fn gone(_colony: i32, _at: [i32; 3]) -> bool {
        false
    }

    #[test]
    fn a_static_claim_owns_an_ownerless_chunk_and_repairs_a_dead_owner() {
        let mut claims = ChunkClaims::new();
        assert_eq!(claims.owner(), None);
        assert!(claims.add_static_claim(7, true), "the first claim owns it");
        assert_eq!(claims.owner(), Some(7));
        assert!(
            !claims.add_static_claim(9, true),
            "a live owner is not replaced"
        );
        assert_eq!(claims.owner(), Some(7), "the first colony keeps answering");
        assert_eq!(claims.static_claims().collect::<Vec<i32>>(), vec![7, 9]);
        assert!(
            claims.add_static_claim(4, false),
            "a dead owner is repaired, as ColonyManager.java:134-141 does"
        );
        assert_eq!(claims.owner(), Some(4));
    }

    #[test]
    fn a_building_claim_takes_an_ownerless_chunk_and_keeps_one_entry_per_colony() {
        let mut claims = ChunkClaims::new();
        assert!(claims.add_building_claim(3, HUT));
        assert_eq!(claims.owner(), Some(3));
        assert!(!claims.add_building_claim(3, [80, 70, 80]), "same colony, second hut");
        assert_eq!(claims.claiming_buildings()[&3].len(), 2);
        assert!(
            !claims.add_building_claim(5, HUT),
            "an owned chunk keeps its owner, addBuildingClaim:197-205"
        );
        assert_eq!(claims.owner(), Some(3));
        assert!(claims.claiming_buildings().contains_key(&5), "but the claim is kept");
    }

    #[test]
    fn removing_a_static_claim_falls_back_to_a_building_then_to_another_colony() {
        let mut claims = ChunkClaims::new();
        claims.add_static_claim(7, true);
        claims.add_static_claim(2, true);
        claims.add_building_claim(5, HUT);
        assert!(claims.remove_static_claim(7), "7 was the owner");
        assert_eq!(
            claims.owner(),
            Some(5),
            "a building claim outranks a static one, removeColony:150-156"
        );
        claims.remove_static_claim(5);
        assert_eq!(claims.owner(), Some(2), "then the lowest remaining static claim");
        claims.remove_static_claim(2);
        assert_eq!(claims.owner(), None, "nothing claims it any more");
        assert!(!claims.remove_static_claim(2), "and there is nothing left to remove");
        assert!(claims.is_empty());
    }

    #[test]
    fn the_last_building_claim_moves_the_owner_to_a_colony_whose_building_stands() {
        let mut claims = ChunkClaims::new();
        claims.add_building_claim(4, HUT);
        claims.add_building_claim(8, [200, 70, 200]);
        assert_eq!(
            claims.remove_building_claim(4, HUT, &standing),
            RemoveOutcome::OwnerMoved { to: 8 }
        );
        assert_eq!(claims.owner(), Some(8));
        assert_eq!(
            claims.remove_building_claim(4, HUT, &standing),
            RemoveOutcome::Absent,
            "the colony had no claim left here, removeBuildingClaim:222-225"
        );
    }

    #[test]
    fn a_building_that_is_gone_loses_its_claim_while_the_owner_moves_on() {
        let mut claims = ChunkClaims::new();
        claims.add_building_claim(4, HUT);
        claims.add_building_claim(8, [200, 70, 200]);
        assert_eq!(
            claims.remove_building_claim(4, HUT, &gone),
            RemoveOutcome::OwnerCleared,
            "no building stands, so Java leaves a dangling owner and the port clears it"
        );
        assert_eq!(claims.owner(), None);
        assert!(
            claims.claiming_buildings().is_empty(),
            "the claims of buildings that are gone went with it, removeBuildingClaim:277"
        );
    }

    #[test]
    fn a_chunk_is_claimed_per_dimension_and_an_empty_entry_is_pruned() {
        let mut book = ClaimBook::new();
        book.entry(OVERWORLD, [0, 0]).add_static_claim(1, true);
        book.entry(NETHER, [0, 0]).add_static_claim(2, true);
        assert_eq!(book.owner(OVERWORLD, [0, 0]), Some(1));
        assert_eq!(book.owner(NETHER, [0, 0]), Some(2), "the same chunk, another dimension");
        assert_eq!(book.owner(OVERWORLD, [1, 0]), None);
        assert_eq!(book.len(), 2);
        book.entry(OVERWORLD, [9, 9]);
        assert_eq!(book.len(), 3, "entry() creates what it is asked for");
        assert_eq!(book.prune(), 1, "and prune() takes the empty one back out");
        assert_eq!(book.len(), 2);
    }

    #[test]
    fn releasing_a_colony_clears_every_chunk_it_held_and_keeps_the_others() {
        let mut book = ClaimBook::new();
        book.entry(OVERWORLD, [0, 0]).add_static_claim(1, true);
        book.entry(OVERWORLD, [1, 0]).add_building_claim(1, HUT);
        book.entry(OVERWORLD, [2, 0]).add_static_claim(2, true);
        book.entry(NETHER, [0, 0]).add_static_claim(1, true);
        let held = book.release_colony(OVERWORLD, 1, &standing);
        assert_eq!(held, vec![[0, 0], [1, 0]], "x ascending, this dimension only");
        assert_eq!(book.owner(OVERWORLD, [0, 0]), None);
        assert_eq!(book.owner(OVERWORLD, [2, 0]), Some(2), "the neighbour is untouched");
        assert_eq!(book.owner(NETHER, [0, 0]), Some(1), "and so is the other dimension");
        assert_eq!(book.len(), 2, "the two emptied chunks left the book");
    }
}
