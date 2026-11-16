//! The colonies that already exist, asked the four questions a founding needs.
//!
//! Java reference: `core/colony/ColonyManager.java` — `getClosestColony:481-507`,
//! `getIColonyByOwner:547-564`, `isFarEnoughFromColonies:311-324` — and
//! `core/colony/ColonyList.java:63-71` (the id a new colony takes), `:148-162`
//! (what happens to an id when a colony is deleted).
//!
//! **Read-only on purpose.** [`super::found()`] takes a `&dyn Colonies`, so it
//! cannot register anything. The one place a colony reaches a store is
//! [`Registry::insert`], called by the caller once the journal says so.
//!
//! **Two of the four questions are provided** — the nearest colony, the owner's
//! colony — because they are the same walk over the same list, and a store that
//! rewrote them could answer differently. The other two are the store's: which ids
//! exist, and which id comes next.
//!
//! **A colony owns a box of chunks, not a radius in blocks.** `isCoordInColony`
//! reads chunk ownership (`Colony.java:1298-1308`), and the box is the one
//! `ChunkDataHelper.claimColonyChunks` claimed at founding (`:135-139`), which is
//! why [`Colonies::owns_chunk`] takes the radius from [`super::Rules`] instead of
//! keeping a copy.

use crate::colony::{Colony, Rank};

use super::rules::Rules;

/// A colony as the founding rules see it: five facts, no borrow of the record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct ColonyAt<'a> {
    pub id: i32,
    pub center: [i32; 3],
    pub dimension: &'a str,
    pub owner: u128,
    pub name: &'a str,
}

impl<'a> ColonyAt<'a> {
    /// The view of one recorded colony. Java asks `getOwner()` off the permissions
    /// (`ColonyManager.java:557`); the port scans the member list for the one rank
    /// a colony may hold once, and answers `0` when there is none — a colony whose
    /// owner was never written, which is what an empty compound reads back as.
    pub fn of(colony: &'a Colony) -> ColonyAt<'a> {
        ColonyAt {
            id: colony.id,
            center: colony.center,
            dimension: &colony.dimension,
            owner: colony
                .members
                .iter()
                .find(|(_, rank)| *rank == Rank::Owner)
                .map(|(player, _)| player)
                .unwrap_or(0),
            name: &colony.name,
        }
    }
}

/// The colonies a founding has to reckon with.
pub trait Colonies {
    /// Every id the store holds, in no particular order.
    fn ids(&self) -> Vec<i32>;

    /// The colony of that id, or `None` if nothing holds it.
    fn colony(&self, id: i32) -> Option<ColonyAt<'_>>;

    /// The id a colony founded now would take (`ColonyList.getNextColonyID`,
    /// `:63-71`): the lowest freed id, or one past the highest ever handed out.
    fn next_id(&self) -> i32;

    /// The nearest colony **in this dimension**, by `BlockPosUtil.getDistance`
    /// (`ColonyManager.java:481-507`, which skips every colony of another
    /// dimension at `:495`). Ties go to the first strictly-closer one (`:498`), so
    /// a store listing its ids in ascending order gives Java's answer.
    fn closest(&self, dimension: &str, at: [i32; 3]) -> Option<ColonyAt<'_>> {
        let mut best: Option<(i64, ColonyAt<'_>)> = None;
        for id in self.ids() {
            let Some(colony) = self.colony(id) else {
                continue;
            };
            if colony.dimension != dimension {
                continue;
            }
            let distance = Rules::distance(at, colony.center);
            if best.is_none_or(|(nearest, _)| distance < nearest) {
                best = Some((distance, colony));
            }
        }
        best.map(|(_, colony)| colony)
    }

    /// The colony this player owns, **in any dimension**:
    /// `ColonyManager.getIColonyByOwner` walks every colony of every level
    /// (`:547-564`), which is why a player cannot found a second colony in the
    /// nether after founding one in the overworld.
    fn owned_by(&self, player: u128) -> Option<ColonyAt<'_>> {
        self.ids()
            .into_iter()
            .filter_map(|id| self.colony(id))
            .find(|colony| colony.owner == player)
    }

    /// Which colony, if any, has claimed that chunk: the ground a new colony
    /// cannot take (`ChunkDataHelper.canClaimChunksInRange`, `:174-207`, which
    /// answers "no" when `getOwningColony() != 0`). `radius` is the claim radius in
    /// chunks, read from the rules by the caller.
    fn owns_chunk(&self, dimension: &str, chunk: [i32; 2], radius: i32) -> Option<i32> {
        for id in self.ids() {
            let Some(colony) = self.colony(id) else {
                continue;
            };
            if colony.dimension != dimension {
                continue;
            }
            let middle = Rules::chunk(colony.center);
            if (chunk[0] - middle[0]).abs() <= radius && (chunk[1] - middle[1]).abs() <= radius {
                return Some(id);
            }
        }
        None
    }

    /// Whether a colony already carries that name. **A rule of this port**: Java
    /// stores whatever the player typed (`Colony.setName`) and two colonies may
    /// share a name; here a name is a player's only handle on a colony, so
    /// [`super::default_name`] makes it unique.
    fn contains_name(&self, name: &str) -> bool {
        self.ids()
            .into_iter()
            .filter_map(|id| self.colony(id))
            .any(|colony| colony.name == name)
    }
}

/// The colonies of one server, in a plain `Vec` — what a test starts from, and what
/// a hook will fill from the world before calling a rule.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Registry {
    colonies: Vec<Colony>,
    /// The ids a deleted colony freed, oldest first. Java keeps the same list and
    /// takes its head (`ColonyList.java:63-71`, `:148-162`).
    freed: Vec<i32>,
    top: i32,
}

impl Registry {
    pub fn new() -> Registry {
        Registry::default()
    }

    /// Adds a colony, keeping the first one that holds an id: `ColonyList.add`
    /// refuses the clash and logs it (`:85-95`), which the port answers with
    /// `false` rather than a panic.
    pub fn insert(&mut self, colony: Colony) -> bool {
        if self.get(colony.id).is_some() {
            return false;
        }
        self.freed.retain(|freed| *freed != colony.id);
        self.top = self.top.max(colony.id);
        self.colonies.push(colony);
        true
    }

    /// Takes a colony out and remembers its id for reuse, as `deleteColony` does
    /// through `ColonyList.remove` (`:148-162`).
    pub fn remove(&mut self, id: i32) -> Option<Colony> {
        let index = self.colonies.iter().position(|colony| colony.id == id)?;
        let colony = self.colonies.remove(index);
        if !self.freed.contains(&id) {
            self.freed.push(id);
        }
        Some(colony)
    }

    pub fn get(&self, id: i32) -> Option<&Colony> {
        self.colonies.iter().find(|colony| colony.id == id)
    }

    pub fn len(&self) -> usize {
        self.colonies.len()
    }

    pub fn is_empty(&self) -> bool {
        self.colonies.is_empty()
    }
}

impl Colonies for Registry {
    fn ids(&self) -> Vec<i32> {
        let mut ids: Vec<i32> = self.colonies.iter().map(|colony| colony.id).collect();
        ids.sort_unstable();
        ids
    }

    fn colony(&self, id: i32) -> Option<ColonyAt<'_>> {
        self.get(id).map(ColonyAt::of)
    }

    fn next_id(&self) -> i32 {
        self.freed.first().copied().unwrap_or(self.top + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWNER: u128 = 1;
    const OVERWORLD: &str = "minecraft:overworld";
    const NETHER: &str = "minecraft:the_nether";

    fn registry() -> Registry {
        let mut registry = Registry::new();
        for (name, dimension, center, owner) in [
            ("Home", OVERWORLD, [0, 70, 0], OWNER),
            ("Far", NETHER, [10, 70, 10], OWNER),
            ("Neighbour", OVERWORLD, [400, 70, 0], 2),
        ] {
            let id = registry.next_id();
            registry.insert(Colony::new(id, name, dimension, center, owner));
        }
        registry
    }

    #[test]
    fn the_closest_colony_is_the_nearest_in_the_same_dimension() {
        let registry = registry();
        assert_eq!(
            registry.closest(OVERWORLD, [10, 70, 0]).map(|colony| colony.id),
            Some(1),
            "the nether colony at 10 blocks is not a neighbour in the overworld"
        );
        assert_eq!(
            registry.closest(OVERWORLD, [380, 70, 0]).map(|colony| colony.id),
            Some(3)
        );
        assert_eq!(registry.closest("minecraft:the_end", [0, 70, 0]), None);
        assert_eq!(
            registry.closest(OVERWORLD, [200, 70, 0]).map(|colony| colony.id),
            Some(1),
            "200 from both, and the lower id keeps the tie"
        );
    }

    #[test]
    fn the_owner_is_the_one_owner_rank_and_a_player_owns_at_most_one_colony() {
        let registry = registry();
        assert_eq!(registry.owned_by(OWNER).map(|colony| colony.id), Some(1));
        assert_eq!(registry.owned_by(2).map(|colony| colony.id), Some(3));
        assert_eq!(registry.owned_by(9), None);
        assert_eq!(registry.colony(1).map(|colony| colony.owner), Some(OWNER));
        assert_eq!(registry.colony(1).map(|colony| colony.name), Some("Home"));
    }

    #[test]
    fn a_claim_is_a_box_of_chunks_and_a_chunk_is_claimed_in_its_dimension_only() {
        let registry = registry();
        let radius = Rules::DEFAULT.claim_radius_chunks();
        assert_eq!(registry.owns_chunk(OVERWORLD, [0, 0], radius), Some(1));
        assert_eq!(
            registry.owns_chunk(OVERWORLD, [4, -4], radius),
            Some(1),
            "four chunks away is the edge of the box"
        );
        assert_eq!(registry.owns_chunk(OVERWORLD, [5, 0], radius), None);
        assert_eq!(
            registry.owns_chunk(NETHER, [0, 0], radius),
            Some(2),
            "the same chunk, in the dimension that colony is in"
        );
    }

    #[test]
    fn an_id_is_reused_only_after_a_colony_is_deleted() {
        let mut registry = registry();
        assert_eq!(registry.next_id(), 4, "one past the highest ever handed out");
        assert_eq!(registry.remove(2).map(|colony| colony.id), Some(2));
        assert_eq!(registry.next_id(), 2, "the freed id comes back first");
        assert!(
            registry.insert(Colony::new(2, "Second nether", NETHER, [0, 70, 0], 3)),
            "the freed id is taken again"
        );
        assert_eq!(registry.next_id(), 4, "and the list of freed ids is empty again");
        assert!(
            !registry.insert(Colony::new(1, "Clash", OVERWORLD, [0, 0, 0], 4)),
            "the first colony of an id stands, as ColonyList.add refuses the clash"
        );
        assert_eq!(registry.len(), 3);
    }
}
