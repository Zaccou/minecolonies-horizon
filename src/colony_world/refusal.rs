//! Why a colony's write to the world was not made.
//!
//! Java answers a refused placement with a chat message, a sound and a `return`
//! (`DirectPlaceMessage.java:117-126`), and a refused break by not breaking. The reason
//! is lost on the way: `ColonyManager.getColonyByPosFromWorld` answers `null` and cannot
//! tell "no colony here" from "a colony here, but deleted" (`:289-302`).
//!
//! **One type, and it reuses the manager's.** The question "who owns this position" is
//! [`crate::manager`]'s, and its four answers — nothing there, a stale owner, an unknown
//! id, the wrong dimension — are already named in [`LookupRefusal`]. Re-spelling them
//! would give one situation two names, and a log could match only one.

use crate::manager::{LookupRefusal, Reach};

use super::guard::Protection;

/// A write the bridge declined to hand to the engine.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// The claim question, carried unchanged from [`crate::manager`].
    Lookup(LookupRefusal),
    /// A terrain mark outside the reach `maxColonySize` allows
    /// (`ChunkDataHelper.buildingClaimInRange`, `:248-255`), carried from the manager.
    Reach(Reach),
    /// The position is claimed, but by **another** colony. Java folds this into `false`
    /// (`Colony.java:1305-1306`: `getOwningColony(chunk) == getID()`); the port names the
    /// owner, because "not yours" and "nobody's" are repaired differently.
    NotOwner {
        dimension: String,
        at: [i32; 3],
        /// The colony that owns the chunk the position is in.
        owner: i32,
        /// The colony that asked to write it.
        colony: i32,
    },
    /// The colony is not ticking: `Colony.isActive()` is `getState() != INACTIVE`
    /// (`Colony.java:1903-1906`), and [`super::guard::State::Inactive`] is
    /// `ColonyState.INACTIVE` — "Colony is not ticking" (`api/colony/ColonyState.java:13-15`).
    ///
    /// **A rule of the port, on a Java premise.** Java never asks `isActive` before a
    /// block write; what it gates is the tick, and a world write only ever happens
    /// *inside* a tick — the builder's job, the waypoint sweep (`Colony.java:1230-1256`).
    /// A port whose writes can be asked for outside a tick has to ask the question Java
    /// gets for free, and this is its name.
    Inactive { id: i32, reason: Stop },
    /// No loaded chunk there: `Level.isLoaded` false, which is what makes
    /// `level_block_state` answer `None` (`host/levels.rs:296-302`, `:337-346`). Java's
    /// `BlockPosUtil.setBlock` would load the chunk (`:664-667`); `edit_blocks` leaves an
    /// unloaded run out **silently** (`host/world_edit.rs:10-20`), so the port says it.
    Unloaded { dimension: String, at: [i32; 3] },
    /// The block there belongs to the colony's own machinery: a hut, a waypoint or
    /// construction tape. See [`Protection`].
    Protected { at: [i32; 3], protection: Protection },
    /// A placement where `BlockState.canBeReplaced()` is false. Java tests the narrow air
    /// form before a hut goes down (`DirectPlaceMessage.java:117`); the port asks the
    /// general one (`host/block_states.rs:120-123`), because grass and water are
    /// replaceable and stone is not.
    Occupied { at: [i32; 3] },
    /// A replacement or a break where nothing stands — the same set that is replaceable.
    /// The engine would write air over air and report a block changed.
    Empty { at: [i32; 3] },
    /// The cell holds a block **entity**, and the write destroys its data. `edit_blocks`
    /// says it plainly: a replaced block loses its block entity, and it takes no NBT at
    /// all (`host/world_edit.rs:12-14`, `game.rs:28`). Java writes the block **and then**
    /// re-applies the saved data (`RackPlacementHandler.java:46-52`);
    /// [`super::intent::Change::Keep`] is the port's answer where the data must survive.
    WouldDropContents { at: [i32; 3] },
    /// One cell named twice in a batch. `edit_blocks` runs a batch in one go, so the
    /// second change would describe a world the first one made.
    ConflictingWrite { at: [i32; 3], first: usize, second: usize },
}

/// Why a colony stopped ticking, as [`super::guard::State`] reports it. Java's
/// `ColonyState` has three values (`api/colony/ColonyState.java:13-26`); the two that
/// stop a write stay apart because a name says more than a bool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two stopping values of ColonyState
pub enum Stop {
    /// `ColonyState.INACTIVE`: the colony is not ticking.
    Inactive,
    /// `ColonyState.UNLOADED`: the colony's chunks are gone, so nothing of it can run.
    Unloaded,
}

impl Refusal {
    /// A stable name, namespaced like every other refusal of the port. [`Refusal::Lookup`]
    /// and [`Refusal::Reach`] hand back the **manager's** own name: one name per
    /// situation, and the manager owns those.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Lookup(inner) => inner.name(),
            Self::Reach(inner) => inner.name(),
            Self::NotOwner { .. } => "colony_world.refusal.not_owner",
            Self::Inactive { reason: Stop::Inactive, .. } => "colony_world.refusal.inactive",
            Self::Inactive { reason: Stop::Unloaded, .. } => "colony_world.refusal.unloaded_colony",
            Self::Unloaded { .. } => "colony_world.refusal.unloaded_position",
            Self::Protected { .. } => "colony_world.refusal.protected",
            Self::Occupied { .. } => "colony_world.refusal.occupied",
            Self::Empty { .. } => "colony_world.refusal.empty",
            Self::WouldDropContents { .. } => "colony_world.refusal.would_drop_contents",
            Self::ConflictingWrite { .. } => "colony_world.refusal.conflicting_write",
        }
    }

    /// The position the refusal is about, when it is about one.
    pub fn at(&self) -> Option<[i32; 3]> {
        match self {
            Self::Lookup(LookupRefusal::NoColonyAt { at, .. }) => Some(*at),
            Self::Lookup(_) | Self::Reach(_) | Self::Inactive { .. } => None,
            Self::NotOwner { at, .. }
            | Self::Unloaded { at, .. }
            | Self::Protected { at, .. }
            | Self::Occupied { at }
            | Self::Empty { at }
            | Self::WouldDropContents { at }
            | Self::ConflictingWrite { at, .. } => Some(*at),
        }
    }
}

impl From<LookupRefusal> for Refusal {
    fn from(value: LookupRefusal) -> Refusal {
        Refusal::Lookup(value)
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_claim_refusal_keeps_the_manager_name_and_does_not_get_a_second_one() {
        let refusal = Refusal::from(LookupRefusal::NoColonyAt {
            dimension: "minecraft:overworld".to_owned(),
            at: [8, 70, 8],
        });
        assert_eq!(refusal.name(), "manager.refusal.no_colony_at");
        assert_eq!(refusal.at(), Some([8, 70, 8]));
    }

    #[test]
    fn the_two_ways_a_colony_stops_ticking_have_two_names() {
        let inactive = Refusal::Inactive { id: 3, reason: Stop::Inactive };
        let unloaded = Refusal::Inactive { id: 3, reason: Stop::Unloaded };
        assert_eq!(inactive.name(), "colony_world.refusal.inactive");
        assert_eq!(unloaded.name(), "colony_world.refusal.unloaded_colony");
        assert_eq!(inactive.at(), None, "a stopped colony is not a position");
    }

    #[test]
    fn a_wrong_dimension_keeps_the_manager_name_and_points_at_no_position() {
        let refusal = Refusal::Lookup(LookupRefusal::WrongDimension {
            id: 4,
            expected: "minecraft:overworld".to_owned(),
            asked: "minecraft:the_nether".to_owned(),
        });
        assert_eq!(refusal.name(), "manager.refusal.wrong_dimension");
        assert_eq!(refusal.at(), None, "a level is not a position");
    }

    #[test]
    fn every_refusal_about_a_position_says_which_and_the_others_say_none() {
        let at = [10, 64, -3];
        let positions = [
            Refusal::Unloaded { dimension: "minecraft:overworld".to_owned(), at },
            Refusal::Occupied { at },
            Refusal::Empty { at },
            Refusal::WouldDropContents { at },
            Refusal::Protected { at, protection: Protection::Waypoint { at } },
            Refusal::ConflictingWrite { at, first: 0, second: 2 },
            Refusal::NotOwner {
                dimension: "minecraft:overworld".to_owned(),
                at,
                owner: 9,
                colony: 4,
            },
        ];
        for refusal in positions {
            assert_eq!(refusal.at(), Some(at), "{refusal} should name its position");
        }
        assert_eq!(
            Refusal::Reach(Reach::TooFarFromCentre {
                chunk: [9, 9],
                distance: 162,
                max: 400
            })
            .at(),
            None,
            "a reach refusal is about a chunk, and a chunk is not a position"
        );
    }
}
