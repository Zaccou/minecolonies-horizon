//! The questions asked before one write, in the order they are asked.
//!
//! **Five questions here, one in [`super::intent`], and the order is the point.** A
//! caller that gets a refusal has to know what to repair, so the questions run from the
//! least transient to the most:
//!
//! 1. **the level** — `Colony.java:1300-1303`: `w.dimension() != this.dimensionId`
//!    answers `false` before anything else is asked;
//! 2. **the claim** — `Colony.java:1305-1306`:
//!    `ColonyUtils.getOwningColony(chunk) == getID()`, answered by [`crate::manager`] and
//!    not re-implemented here;
//! 3. **the colony ticks** — `Colony.isActive()`, `Colony.java:1903-1906`;
//! 4. **the position is loaded** — `Level.isLoaded`, the precondition
//!    `level_block_state` states in its own contract (`host/levels.rs:337-346`);
//! 5. **the block is not the colony's machinery** — [`Protection`];
//! 6. **the operation is meaningful there** — [`super::intent::Intent::plan`]'s, because
//!    only it knows which of the four operations was asked for.
//!
//! **Nothing here calls the host.** [`Claim`] and [`World`] are two traits this module
//! asks, `crate::manager::Manager` is the real answer to the first, and a test answers
//! both from a list — which is how the order above stays provable with no world.

use crate::colony::Colony;
use crate::manager::{LookupRefusal, Manager};

use super::refusal::{Refusal, Stop};

/// The three states a colony can be in: Java's `ColonyState`
/// (`api/colony/ColonyState.java:13-26`), value for value.
///
/// The port keeps the enum rather than a `bool is_active`, because Java's own
/// `isActive()` (`Colony.java:1903-1906`) throws two of the three away, and a write needs
/// to tell "paused" from "its chunks are gone": one is a player's decision, the other
/// is the world's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three values of ColonyState
pub enum State {
    /// `ColonyState.ACTIVE`: the colony ticks, so its jobs run.
    Active,
    /// `ColonyState.INACTIVE`: "Colony is not ticking" (`:13-15`).
    Inactive,
    /// `ColonyState.UNLOADED`: "Colony chunks are not loaded" (`:24-25`).
    Unloaded,
}

impl State {
    /// `Colony.isActive()` (`Colony.java:1903-1906`): state `!= INACTIVE`.
    ///
    /// **The port does not use this to allow a write.** `UNLOADED` answers yes to
    /// `isActive` while no chunk of the colony is there to write;
    /// [`Guard::may_write`] asks the two stopping states separately, each with its own
    /// refusal.
    pub fn is_active(self) -> bool {
        self != State::Inactive
    }

    /// Why this state stops a write, or `None` when the colony ticks.
    pub fn stopped(self) -> Option<Stop> {
        match self {
            State::Active => None,
            State::Inactive => Some(Stop::Inactive),
            State::Unloaded => Some(Stop::Unloaded),
        }
    }
}

/// What the world holds at one position, in the four shapes a write cares about.
///
/// Java reads a `BlockState` and asks it `canBeReplaced()` and `hasBlockEntity()` before
/// a placement, and nothing else. The port keeps the answers and drops the handle: a
/// test cannot fabricate a `BlockState` — its ordinal is the host's
/// (`block/block_state.rs:5-9`) — but it can answer these four.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four shapes a write distinguishes
pub enum Occupant {
    /// No loaded chunk there: `level_block_state` would answer `None`
    /// (`host/levels.rs:296-302`).
    NotLoaded,
    /// A state `canBeReplaced()` accepts — air, grass, water, a snow layer
    /// (`host/block_states.rs:120-123`).
    Replaceable,
    /// A state a placement would overwrite and a break would clear.
    Solid,
    /// A state holding a block entity (`host/block_states.rs:143-145`): the write
    /// destroys its data, and `edit_blocks` cannot carry new data (`game.rs:28`).
    Container,
}

/// A block the colony owns as machinery, which no builder may overwrite or clear.
///
/// **Three kinds, each with the line that makes it one.** Not "protected blocks" in
/// general — Java has no such list in this tree — but the three the colony itself places
/// and reads back: a **hut**, whose block entity *is* the building
/// (`api/blocks/AbstractBlockHut.java:236`); a **waypoint**, which the colony's own sweep
/// reads to know whether a path still walks (`Colony.java:1243-1252`,
/// `ModBlocks.blockWayPoint`); and **construction tape**, the same sweep (`:1246-1250`,
/// `ModBlocks.blockConstructionTape`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three kinds the colony places and reads back
pub enum Protection {
    /// A hut's block: the settlement's own record in the world.
    Hut { at: [i32; 3] },
    /// A waypoint block the pathfinder reads.
    Waypoint { at: [i32; 3] },
    /// Construction tape around a site being built.
    ConstructionTape { at: [i32; 3] },
}

impl Protection {
    /// The Java line that makes this block the colony's: a `source` a report can quote.
    pub fn source(&self) -> &'static str {
        match self {
            Self::Hut { .. } => "api/blocks/AbstractBlockHut.java:236",
            Self::Waypoint { .. } => "core/colony/Colony.java:1243-1252",
            Self::ConstructionTape { .. } => "core/colony/Colony.java:1246-1250",
        }
    }

    /// The position the protected block stands at.
    pub fn at(&self) -> [i32; 3] {
        match self {
            Self::Hut { at } | Self::Waypoint { at } | Self::ConstructionTape { at } => *at,
        }
    }
}

/// Who owns a position: the one question [`crate::manager`] already answers
/// (`Manager::owner_at`), named here so [`Guard`] can be given a fake.
pub trait Claim {
    /// The id owning the chunk `at` is in, or `None` when nobody does.
    fn owner_at(&self, dimension: &str, at: [i32; 3]) -> Option<i32>;
}

/// **The connection to the manager, and the only one in this file.** No rule is copied:
/// `owner_at` is the manager's own chunk read.
impl Claim for Manager {
    fn owner_at(&self, dimension: &str, at: [i32; 3]) -> Option<i32> {
        Manager::owner_at(self, dimension, at)
    }
}

/// The world of one level, as far as a write asks it. A real answer is
/// `level_block_state` + `block_state_replaceable` + `block_state_has_block_entity`
/// (`host/levels.rs:296-302`, `host/block_states.rs:120-123`, `:143-145`); a test
/// answers from a table.
pub trait World {
    /// What stands at `at`.
    fn occupant(&self, at: [i32; 3]) -> Occupant;

    /// The colony machinery at `at`, when the block there is one.
    ///
    /// Answered `None` so a world with no huts has one question to answer, like
    /// `founding::terrain::Ground::holds_town_hall`.
    fn protection(&self, _at: [i32; 3]) -> Option<Protection> {
        None
    }
}

/// A world with nothing in it: open air, and no colony machinery.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Open;

impl World for Open {
    fn occupant(&self, _at: [i32; 3]) -> Occupant {
        Occupant::Replaceable
    }
}

/// The five questions of the table at the top of this file, in that order.
pub struct Guard<'a> {
    /// The level being written: one [`Guard`] is one dimension, so a [`World`] cannot
    /// quietly answer for another one.
    pub dimension: &'a str,
    claim: &'a dyn Claim,
    world: &'a dyn World,
}

impl<'a> Guard<'a> {
    pub fn new(dimension: &'a str, claim: &'a dyn Claim, world: &'a dyn World) -> Guard<'a> {
        Guard {
            dimension,
            claim,
            world,
        }
    }

    /// May `colony`, in `state`, write the position `at` of this level? The five checks of
    /// the table, in that order; the first one that says no is the answer.
    ///
    /// Answers the [`Occupant`] it read on the way, because check 4 has to read it: the
    /// caller asking the sixth question gets the cell for free instead of reading the
    /// world twice.
    pub fn may_write(
        &self,
        colony: &Colony,
        state: State,
        at: [i32; 3],
    ) -> Result<Occupant, Refusal> {
        // 1. the level (`Colony.java:1300-1303`).
        if colony.dimension != self.dimension {
            return Err(Refusal::Lookup(LookupRefusal::WrongDimension {
                id: colony.id,
                expected: colony.dimension.clone(),
                asked: self.dimension.to_owned(),
            }));
        }

        // 2. the claim (`Colony.java:1305-1306`), through the manager.
        match self.claim.owner_at(self.dimension, at) {
            None => {
                return Err(Refusal::Lookup(LookupRefusal::NoColonyAt {
                    dimension: self.dimension.to_owned(),
                    at,
                }));
            }
            Some(owner) if owner != colony.id => {
                return Err(Refusal::NotOwner {
                    dimension: self.dimension.to_owned(),
                    at,
                    owner,
                    colony: colony.id,
                });
            }
            Some(_) => {}
        }

        // 3. the colony ticks (`Colony.java:1903-1906`).
        if let Some(reason) = state.stopped() {
            return Err(Refusal::Inactive { id: colony.id, reason });
        }

        // 4. the position is loaded (`host/levels.rs:337-346`).
        let occupant = self.world.occupant(at);
        if occupant == Occupant::NotLoaded {
            return Err(Refusal::Unloaded {
                dimension: self.dimension.to_owned(),
                at,
            });
        }

        // 5. the block is not the colony's machinery.
        if let Some(protection) = self.world.protection(at) {
            return Err(Refusal::Protected { at, protection });
        }

        Ok(occupant)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colony_world::fakes::{Cells, OWNED, OVERWORLD, OneOwner, Unclaimed, colony};

    #[test]
    fn a_colony_writes_its_own_claimed_loaded_position() {
        let guard = Guard::new(OVERWORLD, &OWNED, &Open);
        assert_eq!(
            guard.may_write(&colony(), State::Active, [8, 70, 8]),
            Ok(Occupant::Replaceable),
            "and the cell it read comes back with the answer"
        );
    }

    #[test]
    fn the_level_is_asked_before_the_claim() {
        // Nobody owns anything in the nether, and the answer is still the level: Java's
        // own first check (`Colony.java:1300-1303`).
        let guard = Guard::new("minecraft:the_nether", &Unclaimed, &Open);
        assert!(matches!(
            guard.may_write(&colony(), State::Active, [8, 70, 8]),
            Err(Refusal::Lookup(LookupRefusal::WrongDimension { id: 7, .. }))
        ));
    }

    #[test]
    fn an_unclaimed_position_is_the_managers_own_refusal_and_another_colonys_names_the_owner() {
        let guard = Guard::new(OVERWORLD, &Unclaimed, &Open);
        assert_eq!(
            guard
                .may_write(&colony(), State::Active, [8, 70, 8])
                .map_err(|r| r.name()),
            Err("manager.refusal.no_colony_at"),
            "not re-spelled: the manager owns it"
        );

        let other = OneOwner { id: 9, chunk: [0, 0] };
        let guard = Guard::new(OVERWORLD, &other, &Open);
        assert_eq!(
            guard.may_write(&colony(), State::Active, [8, 70, 8]),
            Err(Refusal::NotOwner {
                dimension: OVERWORLD.to_owned(),
                at: [8, 70, 8],
                owner: 9,
                colony: 7,
            })
        );
    }

    #[test]
    fn the_claim_is_asked_before_the_colony_state() {
        // Both are wrong; the claim wins, because a paused colony in somebody else's land
        // is not the caller's first mistake.
        let guard = Guard::new(OVERWORLD, &Unclaimed, &Open);
        assert!(matches!(
            guard.may_write(&colony(), State::Inactive, [8, 70, 8]),
            Err(Refusal::Lookup(LookupRefusal::NoColonyAt { .. }))
        ));
    }

    #[test]
    fn an_inactive_colony_is_refused_and_says_which_way_it_stopped() {
        let guard = Guard::new(OVERWORLD, &OWNED, &Open);
        assert_eq!(
            guard.may_write(&colony(), State::Inactive, [8, 70, 8]),
            Err(Refusal::Inactive { id: 7, reason: Stop::Inactive })
        );
        assert_eq!(
            guard.may_write(&colony(), State::Unloaded, [8, 70, 8]),
            Err(Refusal::Inactive { id: 7, reason: Stop::Unloaded })
        );
        assert_eq!(State::Active.stopped(), None);
        assert!(State::Active.is_active());
    }

    #[test]
    fn an_unloaded_position_is_refused_after_the_colony_state() {
        let world = Cells::of(Occupant::NotLoaded);
        let guard = Guard::new(OVERWORLD, &OWNED, &world);
        assert!(matches!(
            guard.may_write(&colony(), State::Active, [8, 70, 8]),
            Err(Refusal::Unloaded { .. })
        ));
        // An inactive colony is not also an unloaded position: one name per reason.
        assert!(matches!(
            guard.may_write(&colony(), State::Inactive, [8, 70, 8]),
            Err(Refusal::Inactive { .. })
        ));
    }

    #[test]
    fn a_hut_is_protected_and_the_refusal_carries_its_java_line() {
        let world = Cells::with_huts(&[[8, 70, 8]]);
        let guard = Guard::new(OVERWORLD, &OWNED, &world);
        match guard.may_write(&colony(), State::Active, [8, 70, 8]) {
            Err(Refusal::Protected { protection, .. }) => {
                assert_eq!(protection, Protection::Hut { at: [8, 70, 8] });
                assert_eq!(protection.source(), "api/blocks/AbstractBlockHut.java:236");
                assert_eq!(protection.at(), [8, 70, 8]);
            }
            other => panic!("expected a protection refusal, got {other:?}"),
        }
    }

    #[test]
    fn each_kind_of_machinery_names_its_own_java_line() {
        assert_eq!(
            Protection::Waypoint { at: [0, 0, 0] }.source(),
            "core/colony/Colony.java:1243-1252"
        );
        assert_eq!(
            Protection::ConstructionTape { at: [0, 0, 0] }.source(),
            "core/colony/Colony.java:1246-1250"
        );
    }

    #[test]
    fn the_manager_is_the_real_answer_to_the_claim_question() {
        let mut manager = Manager::new();
        manager.insert(colony()).expect("a colony of id 7");
        manager
            .claim_ground(OVERWORLD, [8, 70, 8], 0, 7)
            .expect("the colony exists");
        let guard = Guard::new(OVERWORLD, &manager, &Open);
        assert_eq!(
            guard.may_write(&colony(), State::Active, [8, 70, 8]),
            Ok(Occupant::Replaceable)
        );
        // One chunk away is outside a zero radius, and the manager says so.
        assert!(matches!(
            guard.may_write(&colony(), State::Active, [40, 70, 8]),
            Err(Refusal::Lookup(LookupRefusal::NoColonyAt { .. }))
        ));
    }
}
