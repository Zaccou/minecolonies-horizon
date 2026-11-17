//! Marking the ground: what a colony claims, what it gives up, and what that changes
//! for [`crate::manager`].
//!
//! **The bridge touches the world twice, and this is the other half.** A block write is
//! one; a chunk claim is the other, and it is the one that decides whether a block write
//! is allowed at all — [`super::guard::Guard`] asks the manager, the manager asks its
//! [`crate::manager::ClaimBook`], and this module is the only thing that puts a chunk in
//! it.
//!
//! Java reference: `core/util/ChunkDataHelper.java` — `claimColonyChunks` (`:135-139`,
//! the founding box and its `range * 2` removal), `claimBuildingChunks` (`:152-162`),
//! `staticClaimInRange` (`:321-348`), `buildingClaimInRange` (`:241-255`) — read through
//! `Colony.java:1298-1308`, which is what makes a claim mean something to a block write.
//!
//! **Four marks, and the port adds no rule of its own.** The radii are not arguments:
//! they are the colony's own table ([`crate::manager::Proximity`], `claim_radius` and
//! `release_radius`), so a caller cannot mark a box the rest of the port does not agree
//! is the colony's. The reach of a building claim is `Proximity::may_claim` (`:248-255`),
//! where Java's `maxColonySize` ceiling already lives.
//!
//! **A building mark does not need the ground claimed first.** `addBuildingClaim` gives
//! an ownerless chunk to the building's colony (`IColonyTagCapability.java:197-205`), so
//! a hut placed in land its colony never claimed simply takes that chunk. What bounds it
//! is the ceiling, not an order. The only order the port keeps is the batch's own: marks
//! are applied in the order they were given, because a claim then a release is not the
//! same request as a release then a claim.
//!
//! **What a mark changes in the manager** is not a copy of anything: it *is* the manager's
//! own call, and [`Marks::apply`] reports what came back — the chunks a ground claim
//! changed the owner of, the chunks a release actually held, and the [`RemoveOutcome`] of
//! a building's removal, the one answer that says whether the chunk found a new owner.

use crate::colony::Colony;
use crate::manager::{Chunk, Manager, Proximity, RemoveOutcome};

use super::intent::RefusalAt;
use super::refusal::Refusal;

/// One mark a colony asks of its ground.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four marks this bridge knows, closed on purpose
pub enum Mark {
    /// The box a colony claims when it is founded: every chunk a static claim
    /// (`ChunkDataHelper.java:135-139`, `:321-348`).
    ClaimGround { center: [i32; 3] },
    /// The same box given up: Java's removal branch walks **twice** the radius (`:138`,
    /// `add ? range : range * 2`).
    UnclaimGround { center: [i32; 3] },
    /// One building's claim on the chunk its plot covers (`:152-162`), refused past
    /// `maxColonySize` (`:248-255`).
    ClaimBuilding { at: [i32; 3] },
    /// That claim removed, with the owner falling to a colony whose building still stands
    /// there (`IColonyTagCapability.java:220-283`).
    UnclaimBuilding { at: [i32; 3] },
}

/// What one mark did to the manager. **The manager's own answers, named**, so a caller
/// that logs a claim logs what actually moved rather than what it asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: one value per mark
pub enum Effect {
    /// How many chunks changed owner (`Manager::claim_ground` answers the count).
    GroundClaimed { chunks: usize },
    /// The chunks the colony actually held, in key order.
    GroundReleased { chunks: Vec<Chunk> },
    /// The building's claim is in the book.
    BuildingClaimed { at: [i32; 3] },
    /// What removing it did to the chunk's owner.
    BuildingReleased { at: [i32; 3], outcome: RemoveOutcome },
}

/// A batch of marks every question accepted, with the radii already resolved.
///
/// The radii are read from [`Proximity`] at plan time so [`Marks::apply`] needs nothing
/// but the manager: a caller cannot slip a radius between the check and the write.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Marks {
    dimension: String,
    colony: i32,
    claim_radius: i32,
    release_radius: i32,
    marks: Vec<Mark>,
}

impl Marks {
    /// Decides the batch, and builds [`Marks`] only if every mark was accepted.
    ///
    /// 1. the colony is one the manager holds — a mark naming an id nobody has is
    ///    `Manager::claim_ground`'s own `UnknownColony` (`:283-285`), asked here so it is
    ///    asked *once* for the whole batch;
    /// 2. a building **claim** is within `Proximity::may_claim` of the colony's centre
    ///    (`ChunkDataHelper.java:248-255`). A removal has no ceiling in Java.
    ///
    /// Nothing is written here: [`Marks::apply`] is the only writer, and it only exists on
    /// a value this function built.
    pub fn plan(
        manager: &Manager,
        colony: &Colony,
        proximity: &Proximity,
        marks: Vec<Mark>,
    ) -> Result<Marks, RefusalAt> {
        manager.record(colony.id).map_err(|refusal| RefusalAt {
            index: 0,
            at: colony.center,
            refusal: Refusal::Lookup(refusal),
        })?;

        for (index, mark) in marks.iter().enumerate() {
            if let Mark::ClaimBuilding { at } = mark {
                proximity
                    .may_claim(colony.center, crate::founding::Rules::chunk(*at))
                    .map_err(|reach| RefusalAt {
                        index,
                        at: *at,
                        refusal: Refusal::Reach(reach),
                    })?;
            }
        }

        Ok(Marks {
            dimension: colony.dimension.clone(),
            colony: colony.id,
            claim_radius: proximity.claim_radius(),
            release_radius: proximity.release_radius(),
            marks,
        })
    }

    /// The marks, in the order they were accepted.
    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }

    pub fn len(&self) -> usize {
        self.marks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.marks.is_empty()
    }

    /// The chunks a ground mark covers, without touching the manager: what a caller shows
    /// before it applies. A claim covers `(2r+1)²` chunks (`staticClaimInRange`,
    /// `:321-348`), a release the doubled box (`:138`). `None` for a building mark, which
    /// claims one chunk.
    pub fn ground_chunks(&self, index: usize) -> Option<Vec<Chunk>> {
        match self.marks.get(index)? {
            Mark::ClaimGround { center } => {
                Some(Proximity::chunks_in_range(*center, self.claim_radius))
            }
            Mark::UnclaimGround { center } => {
                Some(Proximity::chunks_in_range(*center, self.release_radius))
            }
            Mark::ClaimBuilding { .. } | Mark::UnclaimBuilding { .. } => None,
        }
    }

    /// Carries every mark out, in order, through [`crate::manager`]'s own methods.
    ///
    /// The manager is the only writer: this function picks the method and the radius, and
    /// answers what each call said. It cannot be partway through a batch it did not plan,
    /// because the radii were fixed in [`Marks::plan`].
    pub fn apply(self, manager: &mut Manager) -> Result<Vec<Effect>, Refusal> {
        let mut effects = Vec::with_capacity(self.marks.len());
        for mark in &self.marks {
            match mark {
                Mark::ClaimGround { center } => {
                    let chunks = manager
                        .claim_ground(&self.dimension, *center, self.claim_radius, self.colony)
                        .map_err(Refusal::Lookup)?;
                    effects.push(Effect::GroundClaimed { chunks });
                }
                Mark::UnclaimGround { center } => {
                    let released = manager.release_ground(
                        &self.dimension,
                        *center,
                        self.release_radius,
                        self.colony,
                    );
                    effects.push(Effect::GroundReleased { chunks: released });
                }
                Mark::ClaimBuilding { at } => {
                    manager
                        .claim_building(&self.dimension, *at, self.colony)
                        .map_err(Refusal::Lookup)?;
                    effects.push(Effect::BuildingClaimed { at: *at });
                }
                Mark::UnclaimBuilding { at } => {
                    let outcome = manager.release_building(&self.dimension, *at, self.colony);
                    effects.push(Effect::BuildingReleased { at: *at, outcome });
                }
            }
        }
        Ok(effects)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colony_world::fakes::{OVERWORLD, colony};
    use crate::colony_world::guard::{Guard, Open, Occupant, State};

    /// A manager holding colony 7 and nothing else.
    fn manager() -> Manager {
        let mut manager = Manager::new();
        manager.insert(colony()).expect("a colony of id 7");
        manager
    }

    #[test]
    fn a_ground_claim_puts_the_box_in_the_book_and_the_guard_starts_saying_yes() {
        let mut manager = manager();
        let marks = Marks::plan(
            &manager,
            &colony(),
            &Proximity::DEFAULT,
            vec![Mark::ClaimGround { center: [8, 70, 8] }],
        )
        .expect("the colony exists");

        // Before: the colony's own centre is not its land.
        assert!(Guard::new(OVERWORLD, &manager, &Open)
            .may_write(&colony(), State::Active, [8, 70, 8])
            .is_err());

        assert_eq!(
            marks.apply(&mut manager).expect("the colony exists"),
            vec![Effect::GroundClaimed { chunks: 81 }],
            "(2 * 4 + 1)² chunks for the default 4-chunk radius"
        );

        // After: the same question, and the manager answers its own colony.
        assert_eq!(
            Guard::new(OVERWORLD, &manager, &Open).may_write(&colony(), State::Active, [8, 70, 8]),
            Ok(Occupant::Replaceable)
        );
    }

    #[test]
    fn the_radius_is_the_colonys_table_and_never_a_callers_argument() {
        let manager = manager();
        let marks = Marks::plan(
            &manager,
            &colony(),
            &Proximity::DEFAULT,
            vec![
                Mark::ClaimGround { center: [8, 70, 8] },
                Mark::UnclaimGround { center: [8, 70, 8] },
                Mark::ClaimBuilding { at: [40, 70, 40] },
            ],
        )
        .expect("two ground marks and one building mark");
        assert_eq!(
            marks.ground_chunks(0).map(|chunks| chunks.len()),
            Some(81),
            "claim_radius is 4"
        );
        assert_eq!(
            marks.ground_chunks(1).map(|chunks| chunks.len()),
            Some(289),
            "release_radius is twice that: (2*8+1)², ChunkDataHelper.java:138"
        );
        assert_eq!(
            marks.ground_chunks(2),
            None,
            "a building mark claims one chunk, not a box"
        );
        assert_eq!(marks.len(), 3);
        assert!(!marks.is_empty());
        assert_eq!(marks.marks().len(), 3);
    }

    #[test]
    fn a_ground_box_is_walked_x_first_then_z_both_ends_included() {
        let manager = manager();
        let marks = Marks::plan(
            &manager,
            &colony(),
            &Proximity::DEFAULT,
            vec![Mark::ClaimGround { center: [8, 70, 8] }],
        )
        .expect("the centre is in chunk 0,0");
        let chunks = marks.ground_chunks(0).expect("a ground mark");
        assert_eq!(chunks.first(), Some(&[-4, -4]), "x outer, z inner");
        assert_eq!(chunks.get(1), Some(&[-4, -3]), "then z");
        assert_eq!(chunks.last(), Some(&[4, 4]));
    }

    #[test]
    fn releasing_reports_the_chunks_the_colony_actually_held() {
        let mut manager = manager();
        let marks = Marks::plan(
            &manager,
            &colony(),
            &Proximity::DEFAULT,
            vec![
                Mark::ClaimGround { center: [8, 70, 8] },
                Mark::UnclaimGround { center: [8, 70, 8] },
            ],
        )
        .expect("both are ground marks");
        let effects = marks.apply(&mut manager).expect("the colony exists");
        assert_eq!(effects[0], Effect::GroundClaimed { chunks: 81 });
        match &effects[1] {
            Effect::GroundReleased { chunks } => {
                assert_eq!(chunks.len(), 81, "it holds what it claimed, not 289");
                assert_eq!(chunks.first(), Some(&[-4, -4]));
            }
            other => panic!("expected a release, got {other:?}"),
        }
        assert_eq!(manager.claims().len(), 0, "and the book is empty again");
    }

    #[test]
    fn a_building_claim_records_the_building_on_its_chunk() {
        let mut manager = manager();
        let hut = [40, 70, 40];
        let marks = Marks::plan(
            &manager,
            &colony(),
            &Proximity::DEFAULT,
            vec![
                Mark::ClaimGround { center: [8, 70, 8] },
                Mark::ClaimBuilding { at: hut },
            ],
        )
        .expect("40,40 is chunk 2,2, inside the 20-chunk ceiling");
        assert_eq!(
            marks.apply(&mut manager).expect("the colony exists")[1],
            Effect::BuildingClaimed { at: hut }
        );
        let claims = manager
            .claims()
            .get(OVERWORLD, crate::founding::Rules::chunk(hut))
            .expect("the chunk carries a claim");
        assert_eq!(claims.claiming_buildings()[&7].len(), 1);
    }

    #[test]
    fn a_building_claim_alone_takes_an_ownerless_chunk_as_java_does() {
        let mut manager = manager();
        let hut = [40, 70, 40];
        // No ground mark: `addBuildingClaim` gives an ownerless chunk to the building's
        // colony (`IColonyTagCapability.java:197-205`), so this is legal, not a mistake.
        let marks = Marks::plan(
            &manager,
            &colony(),
            &Proximity::DEFAULT,
            vec![Mark::ClaimBuilding { at: hut }],
        )
        .expect("the ceiling allows chunk 2,2");
        let effects = marks.apply(&mut manager).expect("the colony exists");
        assert_eq!(effects, vec![Effect::BuildingClaimed { at: hut }]);
        assert_eq!(
            manager.claims().owner(OVERWORLD, crate::founding::Rules::chunk(hut)),
            Some(7),
            "the building took the chunk it stands on"
        );
    }

    #[test]
    fn a_building_claim_past_the_ceiling_is_the_managers_own_refusal() {
        let manager = manager();
        // 20 chunks is the ceiling: chunk 21,0 is one past it.
        let too_far = [21 * 16 + 1, 70, 1];
        let refusal = Marks::plan(
            &manager,
            &colony(),
            &Proximity::DEFAULT,
            vec![
                Mark::ClaimGround { center: [8, 70, 8] },
                Mark::ClaimBuilding { at: too_far },
            ],
        )
        .unwrap_err();
        assert_eq!(refusal.index, 1);
        assert_eq!(refusal.name(), "manager.refusal.too_far_from_centre");
    }

    #[test]
    fn an_unknown_colony_is_refused_before_any_mark_is_looked_at() {
        let refusal = Marks::plan(
            &Manager::new(),
            &colony(),
            &Proximity::DEFAULT,
            vec![Mark::ClaimGround { center: [8, 70, 8] }],
        )
        .unwrap_err();
        assert_eq!(
            refusal.refusal,
            Refusal::Lookup(crate::manager::LookupRefusal::UnknownColony { id: 7 })
        );
        assert_eq!(refusal.index, 0);
    }

    #[test]
    fn a_building_removal_reports_what_happened_to_the_chunk_owner() {
        let mut manager = manager();
        let hut = [40, 70, 40];
        let marks = Marks::plan(
            &manager,
            &colony(),
            &Proximity::DEFAULT,
            vec![
                Mark::ClaimGround { center: [8, 70, 8] },
                Mark::ClaimBuilding { at: hut },
                Mark::UnclaimBuilding { at: hut },
            ],
        )
        .expect("the three marks are in order");
        let effects = marks.apply(&mut manager).expect("the colony exists");
        match &effects[2] {
            Effect::BuildingReleased { at, outcome } => {
                assert_eq!(*at, hut);
                assert_eq!(
                    *outcome,
                    RemoveOutcome::OwnerKept,
                    "the static claim of the ground still owns the chunk"
                );
            }
            other => panic!("expected a removal, got {other:?}"),
        }
    }

    #[test]
    fn the_marks_are_applied_in_the_order_they_were_given() {
        let mut manager = manager();
        let claim_then_release = Marks::plan(
            &manager,
            &colony(),
            &Proximity::DEFAULT,
            vec![
                Mark::ClaimGround { center: [8, 70, 8] },
                Mark::UnclaimGround { center: [8, 70, 8] },
            ],
        )
        .expect("two ground marks");
        claim_then_release.apply(&mut manager).expect("the colony exists");
        assert_eq!(manager.claims().len(), 0, "claimed, then given up");

        // The same two marks the other way round: there is nothing to release, and the
        // claim that follows leaves the box held.
        let release_then_claim = Marks::plan(
            &manager,
            &colony(),
            &Proximity::DEFAULT,
            vec![
                Mark::UnclaimGround { center: [8, 70, 8] },
                Mark::ClaimGround { center: [8, 70, 8] },
            ],
        )
        .expect("two ground marks");
        let effects = release_then_claim.apply(&mut manager).expect("the colony exists");
        assert_eq!(effects[0], Effect::GroundReleased { chunks: Vec::new() });
        assert_eq!(effects[1], Effect::GroundClaimed { chunks: 81 });
        assert_eq!(manager.claims().len(), 81);
    }
}
