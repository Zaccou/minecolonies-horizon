//! Who owns a chunk: the claim record, as a value.
//!
//! Java keeps this on the chunk itself, as a Forge capability:
//! `IColonyTagCapability.Impl` (`api/colony/IColonyTagCapability.java:110-353`) is
//! three fields — an owning colony id, a set of colonies with a *static* claim,
//! and a map of colony to the buildings claiming the chunk — and the whole answer
//! to "is this block in that colony" goes through it
//! (`Colony.isCoordInColony`, `core/colony/Colony.java:1307`).
//!
//! **Why it is a value here.** A wasm mod has no chunk and no capability to hang
//! one on; what it has is a compound it can save and read. [`ChunkClaims`] is the
//! three fields as data, with the transitions Java applies, so a caller that can
//! read a chunk's compound (or that wants to replay a claim log) gets the same
//! owner.
//!
//! **The ownership rules, which are more than "first come".**
//!
//! 1. **A colony claiming an unowned chunk owns it** (`:137-140`), and claiming
//!    again does not move it: the test is `owningColony == NO_COLONY_ID`.
//! 2. **A building claiming an unowned chunk takes it too** (`:196-200`) — a hut
//!    built on the edge of the land brings the chunk with it.
//! 3. **Releasing the owner hands over**, in this order: a colony that still has a
//!    building claiming the chunk, then a colony with a static claim, then nobody
//!    (`:150-163`).
//! 4. **Releasing a colony drops its building claims as well** (`:149`) — a colony
//!    that is gone cannot be the reason a chunk is claimed.
//!
//! **Where the port had to choose, and says so.** Java picks the successor with
//! `iterator().next()` on a `HashSet` (`:154`, `:160`), which is *any* element and
//! can differ between runs and JVMs; the same for the owner adopted on load
//! (`:325`, a list built from a set). [`ChunkClaims`] walks a `BTreeSet`, so the
//! answer is the **lowest id** — deterministic, and the only difference from Java
//! in a case where Java does not promise an order anyway.
//!
//! **What is not here.** `chunk.setUnsaved(true)`, which ends every setter
//! (`:142`, `:166`, `:181`): a mod has no chunk to mark, so each transition
//! reports whether anything changed and the caller decides. And the "is this
//! colony still registered" lookups (`:137`, `:152`, `:246`) need the colony
//! registry, which a mod does not have.

use std::collections::{BTreeMap, BTreeSet};

use super::constants::NO_COLONY_ID;

/// The claim record of one chunk: `IColonyTagCapability.Impl`
/// (`api/colony/IColonyTagCapability.java:110-353`).
///
/// `Default` is an unclaimed chunk: owner `NO_COLONY_ID`, no static claims, no
/// building claims — which is what Java's field initialisers give
/// (`:120`, `:125`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct ChunkClaims {
    owner: i32,
    /// Java's `colonies`: the colonies whose land reaches this chunk without a
    /// building (`:118-120`, `:170-173`).
    colonies: BTreeSet<i32>,
    /// Java's `claimingBuildings`: colony id to the positions claiming for it
    /// (`:125`).
    buildings: BTreeMap<i32, BTreeSet<[i32; 3]>>,
}

impl ChunkClaims {
    /// An unclaimed chunk.
    pub fn new() -> ChunkClaims {
        ChunkClaims::default()
    }

    /// The owning colony, or `None` when nobody owns the chunk.
    ///
    /// Java's answer is the `i32` `NO_COLONY_ID` (`:282-284`); `None` is the same
    /// fact with the "no colony" case impossible to use as one.
    pub fn owner(&self) -> Option<i32> {
        (self.owner != NO_COLONY_ID).then_some(self.owner)
    }

    /// The raw owner id, `NO_COLONY_ID` when unclaimed — what the compound
    /// stores (`:317`) and what `ColonyUtils.getOwningColony` returns
    /// (`api/util/ColonyUtils.java:157-160`).
    pub fn owner_id(&self) -> i32 {
        self.owner
    }

    /// Is the chunk owned by `colony`? **The owner only**, because that is what
    /// `isCoordInColony` compares (`core/colony/Colony.java:1307`): a colony with
    /// a static claim on someone else's chunk does *not* own the blocks.
    pub fn claimed_by(&self, colony: i32) -> bool {
        self.owner != NO_COLONY_ID && self.owner == colony
    }

    /// The colonies with a static claim on this chunk. Java returns a `List` built
    /// from a `HashSet` (`:299-303`), so its order is its own; here it is
    /// ascending.
    pub fn claims(&self) -> impl Iterator<Item = i32> + '_ {
        self.colonies.iter().copied()
    }

    /// The buildings of `colony` claiming this chunk (`getAllClaimingBuildings`,
    /// `:310-313`).
    pub fn claiming_buildings(&self, colony: i32) -> impl Iterator<Item = [i32; 3]> + '_ {
        self.buildings
            .get(&colony)
            .into_iter()
            .flat_map(|positions| positions.iter().copied())
    }

    /// Is nothing claimed here at all? Not a Java method: Java asks
    /// `getOwningColony() == NO_COLONY_ID` and tolerates the rest.
    pub fn is_empty(&self) -> bool {
        self.owner == NO_COLONY_ID && self.colonies.is_empty() && self.buildings.is_empty()
    }

    /// A colony claims the chunk — `addColony` (`:128-143`).
    ///
    /// **`NO_COLONY_ID` is refused**, because Java looks the colony up and returns
    /// when it does not exist (`:131-134`); an id of zero names no colony by
    /// definition. Returns whether anything changed.
    pub fn claim(&mut self, colony: i32) -> bool {
        if colony == NO_COLONY_ID {
            return false;
        }
        let inserted = self.colonies.insert(colony);
        let took_ownership = self.owner == NO_COLONY_ID;
        if took_ownership {
            self.owner = colony;
        }
        inserted || took_ownership
    }

    /// A colony stops claiming the chunk — `removeColony` (`:145-166`). Its
    /// building claims go with it (`:149`), and if it was the owner a successor
    /// takes over (`:150-163`).
    ///
    /// Returns whether anything changed; `false` for a colony that had nothing
    /// here.
    pub fn unclaim(&mut self, colony: i32) -> bool {
        let had_colonies = self.colonies.remove(&colony);
        let had_buildings = self.buildings.remove(&colony).is_some();
        if !(had_colonies || had_buildings) {
            return false;
        }
        if self.owner == colony {
            self.owner = self.successor();
        }
        true
    }

    /// A building of `colony` claims the chunk — `addBuildingClaim`
    /// (`:185-215`). An unowned chunk becomes this colony's (`:196-200`).
    pub fn claim_building(&mut self, colony: i32, at: [i32; 3]) -> bool {
        if colony == NO_COLONY_ID {
            return false;
        }
        if self.owner == NO_COLONY_ID {
            self.owner = colony;
        }
        self.buildings.entry(colony).or_default().insert(at)
    }

    /// That building releases the chunk — `removeBuildingClaim` (`:220-281`).
    ///
    /// Only the **last** building of a colony can hand the chunk over, and only if
    /// the colony has no static claim on it either (`:231-235`): a colony whose
    /// buildings are all gone but whose land still reaches the chunk keeps it.
    pub fn unclaim_building(&mut self, colony: i32, at: [i32; 3]) -> bool {
        let Some(positions) = self.buildings.get_mut(&colony) else {
            return false;
        };
        if !positions.remove(&at) {
            return false;
        }
        let last_one = self
            .buildings
            .get(&colony)
            .is_some_and(|positions| positions.is_empty());
        if last_one {
            self.buildings.remove(&colony);
            if self.owner == colony && !self.colonies.contains(&colony) {
                self.owner = self.successor();
            }
        }
        true
    }

    /// Forces the owner, which Java does on a forced claim
    /// (`tryClaim(…, forceOwnerChange)`, `core/util/ChunkDataHelper.java:404-410`,
    /// through `setOwningColony`, `:285-289`). `NO_COLONY_ID` clears it.
    pub fn set_owner(&mut self, colony: i32) -> bool {
        if self.owner == colony {
            return false;
        }
        self.owner = colony;
        true
    }

    /// Everything releases the chunk — `reset` (`:175-181`).
    pub fn reset(&mut self) {
        self.owner = NO_COLONY_ID;
        self.colonies.clear();
        self.buildings.clear();
    }

    /// Adopts an owner if the record has none but has a static claim — the repair
    /// Java does when it loads a chunk (`:323-326`).
    ///
    /// Without it a save written by an older version, or one whose owner was
    /// cleared, would read as unclaimed land with colonies still listed on it. The
    /// port takes the lowest claimant; Java takes the first of a list built from a
    /// `HashSet`.
    pub fn adopt_owner(&mut self) -> bool {
        if self.owner != NO_COLONY_ID {
            return false;
        }
        match self.colonies.first().copied() {
            Some(colony) => {
                self.owner = colony;
                true
            }
            None => false,
        }
    }

    /// The colony that should own the chunk now: the first with a building claim,
    /// else the first with a static claim, else nobody (`:150-163`).
    fn successor(&self) -> i32 {
        self.buildings
            .keys()
            .next()
            .copied()
            .or_else(|| self.colonies.first().copied())
            .unwrap_or(NO_COLONY_ID)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: i32 = 1;
    const B: i32 = 2;

    #[test]
    fn a_new_chunk_has_no_owner_and_says_so_as_none_and_as_zero() {
        let claims = ChunkClaims::new();
        assert_eq!(claims.owner(), None);
        assert_eq!(claims.owner_id(), NO_COLONY_ID);
        assert!(claims.is_empty());
        assert!(!claims.claimed_by(NO_COLONY_ID));
    }

    #[test]
    fn the_first_colony_to_claim_the_chunk_owns_it() {
        // `addColony` sets the owner only when there is none (`:137-140`).
        let mut claims = ChunkClaims::new();
        assert!(claims.claim(A));
        assert_eq!(claims.owner(), Some(A));
        assert!(claims.claim(B));
        assert_eq!(claims.owner(), Some(A), "a second claim does not move the owner");
        assert_eq!(claims.claims().collect::<Vec<_>>(), vec![A, B]);
        assert!(claims.claimed_by(A));
        assert!(!claims.claimed_by(B), "only the owner owns the blocks");
    }

    #[test]
    fn a_colony_that_is_no_colony_cannot_claim() {
        // Java returns early when the id names no registered colony (`:131-134`).
        let mut claims = ChunkClaims::new();
        assert!(!claims.claim(NO_COLONY_ID));
        assert!(!claims.claim_building(NO_COLONY_ID, [0, 64, 0]));
        assert!(claims.is_empty());
    }

    #[test]
    fn releasing_the_owner_hands_over_to_a_building_then_a_static_claim() {
        // `removeColony` (`:146-166`).
        let mut claims = ChunkClaims::new();
        claims.claim(A);
        claims.claim(B);
        claims.claim_building(B, [4, 70, 4]);
        assert_eq!(claims.owner(), Some(A));
        assert!(claims.unclaim(A));
        assert_eq!(claims.owner(), Some(B), "B has a building here, so B takes it");
        assert!(claims.unclaim(B));
        assert_eq!(claims.owner(), None, "nothing is left claiming the chunk");
        assert!(claims.is_empty());
    }

    #[test]
    fn releasing_a_colony_that_was_not_the_owner_leaves_the_owner_alone() {
        let mut claims = ChunkClaims::new();
        claims.claim(A);
        claims.claim(B);
        assert!(claims.unclaim(B));
        assert_eq!(claims.owner(), Some(A));
        // Releasing something that claimed nothing is not a change.
        assert!(!claims.unclaim(B));
        assert!(!claims.unclaim(99));
    }

    #[test]
    fn releasing_the_last_claimant_without_an_owner_yields_no_owner_at_all() {
        let mut claims = ChunkClaims::new();
        claims.claim(A);
        assert!(claims.unclaim(A));
        assert_eq!(claims.owner(), None);
        assert_eq!(claims.owner_id(), NO_COLONY_ID);
    }

    #[test]
    fn a_building_claim_takes_an_unowned_chunk_but_never_a_claimed_one() {
        // `addBuildingClaim` (`:196-200`).
        let mut claims = ChunkClaims::new();
        assert!(claims.claim_building(A, [10, 70, 10]));
        assert_eq!(claims.owner(), Some(A));
        assert!(claims.claim_building(B, [20, 70, 20]));
        assert_eq!(claims.owner(), Some(A), "B's hut does not steal A's land");
        assert_eq!(
            claims.claiming_buildings(A).collect::<Vec<_>>(),
            vec![[10, 70, 10]]
        );
        // The same building claiming twice is one claim, and no change.
        assert!(!claims.claim_building(A, [10, 70, 10]));
    }

    #[test]
    fn releasing_the_last_building_clears_the_owner_unless_the_land_still_reaches_it() {
        // `removeBuildingClaim` (`:231-249`).
        let mut claims = ChunkClaims::new();
        claims.claim_building(A, [10, 70, 10]);
        assert!(claims.unclaim_building(A, [10, 70, 10]));
        assert_eq!(claims.owner(), None);
        assert!(claims.is_empty());

        // Same, but the colony also has a static claim: it keeps the chunk.
        let mut claims = ChunkClaims::new();
        claims.claim(A);
        claims.claim_building(A, [10, 70, 10]);
        assert!(claims.unclaim_building(A, [10, 70, 10]));
        assert_eq!(claims.owner(), Some(A));
        assert_eq!(claims.claims().collect::<Vec<_>>(), vec![A]);
    }

    #[test]
    fn one_of_two_buildings_going_does_not_hand_the_chunk_over() {
        let mut claims = ChunkClaims::new();
        claims.claim_building(A, [1, 70, 1]);
        claims.claim_building(A, [2, 70, 2]);
        assert!(claims.unclaim_building(A, [1, 70, 1]));
        assert_eq!(claims.owner(), Some(A));
        assert!(!claims.unclaim_building(A, [9, 70, 9]), "never claimed here");
        assert!(claims.unclaim_building(A, [2, 70, 2]));
        assert_eq!(claims.owner(), None);
    }

    #[test]
    fn releasing_a_colony_drops_the_buildings_it_claimed_with() {
        // `:149`.
        let mut claims = ChunkClaims::new();
        claims.claim(A);
        claims.claim_building(A, [3, 70, 3]);
        assert!(claims.unclaim(A));
        assert_eq!(claims.claiming_buildings(A).count(), 0);
        assert!(claims.is_empty());
    }

    #[test]
    fn the_successor_is_the_lowest_id_where_java_promises_none() {
        // Java takes `iterator().next()` of a `HashSet` (`:154`, `:160`).
        let mut claims = ChunkClaims::new();
        for colony in [7, 3, 5] {
            claims.claim(colony);
        }
        claims.claim_building(7, [0, 70, 0]);
        claims.claim_building(5, [0, 70, 1]);
        claims.set_owner(7);
        assert!(claims.unclaim(7));
        assert_eq!(claims.owner(), Some(5), "5 is the lowest colony with a building");
        claims.unclaim(5);
        assert_eq!(claims.owner(), Some(3), "then the lowest static claim");
    }

    #[test]
    fn a_forced_owner_can_clear_the_chunk_and_a_repair_adopts_one() {
        // `setOwningColony` (`:285-289`) and the load-time repair (`:323-326`).
        let mut claims = ChunkClaims::new();
        claims.claim(4);
        assert!(claims.set_owner(9));
        assert_eq!(claims.owner(), Some(9));
        assert!(!claims.set_owner(9));
        assert!(claims.set_owner(NO_COLONY_ID));
        assert_eq!(claims.owner(), None);

        assert!(claims.adopt_owner());
        assert_eq!(claims.owner(), Some(4));
        assert!(!claims.adopt_owner(), "there is an owner now");
        // An empty chunk has nothing to adopt.
        let mut empty = ChunkClaims::new();
        assert!(!empty.adopt_owner());
    }

    #[test]
    fn a_reset_forgets_everything_about_the_chunk() {
        // `reset` (`:175-181`).
        let mut claims = ChunkClaims::new();
        claims.claim(A);
        claims.claim_building(B, [1, 70, 1]);
        claims.reset();
        assert!(claims.is_empty());
        assert_eq!(claims.owner(), None);
        assert_eq!(claims.claims().count(), 0);
        assert_eq!(claims.claiming_buildings(B).count(), 0);
    }
}
