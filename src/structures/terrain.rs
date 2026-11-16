//! Does the volume stand free, does the ground bear, and what does the builder
//! have to do first.
//!
//! Java reference: the structure stages the builder walks, because that is what
//! a hut site is in Java — work, not a precondition
//! (`core/entity/ai/workers/util/BuildingProgressStage.java`):
//!
//! | Question | Stage | Reference |
//! | --- | --- | --- |
//! | is the volume empty | `CLEAR` then `CLEAR_NON_SOLIDS` | `AbstractEntityAIStructure.java:418-424`, `:454-458` |
//! | is there water in it | `CLEAR_WATER` | `:414-417` |
//! | does the ground bear | `BUILD_SOLID`, then `WEAK_SOLID` | `:398-402`, `:403-413` |
//! | is there something that cannot be removed | `skipRemoval` | `:658-668` |
//!
//! **Three answers, and one of them is not a no.** [`Verdict::NeedsPreparation`]
//! is the ordinary case — a hut is placed on grass and the builder clears and
//! fills. [`Verdict::Impossible`] is only for what Java itself can never remove:
//! `skipRemoval` returns true for air and for the three substitution blocks, and
//! `mineBlock` returns at once for bedrock and `IBuilderUndestroyable`
//! (`AbstractEntityAIInteract.java:160-166`), so a bedrock cell inside the box
//! means the building can never be finished.
//!
//! **Facts, not calls.** Every block comes from [`WorldFacts::blocks`], the same
//! map `crate::ai` reads: the caller reports what it observed, a position nobody
//! reported is [`Block::Air`], and a chunk the caller marked unloaded is **not**
//! air — [`Refusal::NotLoaded`] is the answer there, because a survey that
//! clears a house it never saw is worse than one that refuses.
//!
//! **What this cannot say.** `BlockUtils.isWeakSolidBlock`
//! (`structurize/util/BlockUtils.java:808-822`) tells dirt from stone, and the
//! `WEAK_SOLID` stage replaces the weak floors with a solid block before building
//! on them. [`Block`] has no such distinction — `Block::Solid` is "bears" here —
//! so a site on sand is `Ready` instead of `NeedsPreparation`. The gap is named
//! in `src/structures/MANQUANT.md`; guessing it would be inventing a block tag.

use crate::ai::{Block, Pos, WorldFacts};
use crate::building::Footprint;

use super::Refusal;
use super::place::{Limits, Placing};

/// What a survey found, in one word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: three answers, and "needs work" is not a refusal
pub enum Verdict {
    /// The volume is free and every column has something bearing under it: paste.
    Ready,
    /// The builder has something to do first — clear cells, or fill a floor.
    NeedsPreparation,
    /// Something in the volume can never be removed, so the building can never
    /// be finished here (`AbstractEntityAIStructure.java:658-668`).
    Impossible,
}

/// Everything a survey measured, and where it first saw it.
///
/// The first positions are kept because a player's error message wants one
/// coordinate, not a count, and a test wants to name the block it planted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiteReport {
    /// The one-word answer.
    pub verdict: Verdict,
    /// How many cells of the box were inspected: the box's volume.
    pub volume: usize,
    /// Cells holding something the builder can clear (`Solid`, `Barrel`).
    pub blocked: usize,
    /// Columns with nothing bearing underneath: air or a fluid at `low.y - 1`.
    pub foundation: usize,
    /// Cells holding something nothing can clear (bedrock, undestroyable).
    pub stubborn: usize,
    /// The first cell to clear, in the order the survey walked the box.
    pub first_blocked: Option<Pos>,
    /// The first column that needs a floor.
    pub first_foundation: Option<Pos>,
    /// The first cell that makes the site impossible.
    pub first_impossible: Option<Pos>,
}

impl SiteReport {
    /// The short answer, for a caller that does not care about the counts.
    pub fn is_ready(&self) -> bool {
        self.verdict == Verdict::Ready
    }

    /// Whether the builder will have to walk the site before building.
    pub fn needs_work(&self) -> bool {
        self.verdict != Verdict::Ready
    }
}

/// Survey a box the caller already placed, with the engine's default height.
pub fn survey(facts: &WorldFacts, boxed: &Footprint) -> Result<SiteReport, Refusal> {
    survey_within(facts, boxed, &Limits::default())
}

/// Survey the box a [`Placing`] would cover, at the default build height.
pub fn site(facts: &WorldFacts, placing: &Placing, size: [u32; 3]) -> Result<SiteReport, Refusal> {
    site_within(facts, placing, size, &Limits::default())
}

/// The same, with the level's own build height.
pub fn site_within(
    facts: &WorldFacts,
    placing: &Placing,
    size: [u32; 3],
    limits: &Limits,
) -> Result<SiteReport, Refusal> {
    let boxed = placing.within(size, limits)?;
    survey_within(facts, &boxed, limits)
}

/// The survey itself: volume first, then the floor under it.
///
/// **The cost.** Every cell of the box is read once, and
/// [`crate::ai::Blocks::at`] is a linear scan of what the caller reported
/// (`facts.rs:156-162`), so a big blueprint against a big report is quadratic.
/// The measured worst case of the corpus is `colonial/fundamentals/townhall1.nbt`
/// at `[38, 28, 33]` — 35 112 cells. That is fine once per click; it is not
/// something to run in a loop, and a caller that surveys every tick is calling
/// the wrong function.
pub fn survey_within(
    facts: &WorldFacts,
    boxed: &Footprint,
    limits: &Limits,
) -> Result<SiteReport, Refusal> {
    let low = boxed.low();
    let high = boxed.high();

    // The box itself has to be in the world before its blocks mean anything.
    if !limits.holds(low[1], high[1]) {
        return Err(Refusal::OutOfWorld {
            low: low[1],
            high: high[1],
            min_y: limits.min_y,
            max_y: limits.max_y,
        });
    }

    let blocks = &facts.blocks;
    let mut report = SiteReport {
        verdict: Verdict::Ready,
        volume: 0,
        blocked: 0,
        foundation: 0,
        stubborn: 0,
        first_blocked: None,
        first_foundation: None,
        first_impossible: None,
    };

    for y in low[1]..=high[1] {
        for z in low[2]..=high[2] {
            for x in low[0]..=high[0] {
                let cell = Pos::new(x, y, z);
                report.volume += 1;
                if !blocks.is_loaded(cell) {
                    return Err(Refusal::NotLoaded { at: cell });
                }
                match blocks.at(cell) {
                    // Empty, or water the `CLEAR_WATER` stage pumps out.
                    Block::Air | Block::Fluid => {}
                    // `CLEAR`/`CLEAR_NON_SOLIDS`: the builder removes it.
                    Block::Solid | Block::Barrel => {
                        report.blocked += 1;
                        if report.first_blocked.is_none() {
                            report.first_blocked = Some(cell);
                        }
                    }
                    // `skipRemoval` leaves it, and `mineBlock` returns at once.
                    Block::Undestroyable => {
                        report.stubborn += 1;
                        if report.first_impossible.is_none() {
                            report.first_impossible = Some(cell);
                        }
                    }
                }
            }
        }
    }

    // The floor: one cell under each column of the box.
    let floor = low[1].saturating_sub(1);
    for z in low[2]..=high[2] {
        for x in low[0]..=high[0] {
            let under = Pos::new(x, floor, z);
            if !blocks.is_loaded(under) {
                return Err(Refusal::NotLoaded { at: under });
            }
            match blocks.at(under) {
                // Nothing bears: `BUILD_SOLID` fills the column first.
                Block::Air | Block::Fluid => {
                    report.foundation += 1;
                    if report.first_foundation.is_none() {
                        report.first_foundation = Some(under);
                    }
                }
                // A solid, a barrel or bedrock all carry a wall.
                Block::Solid | Block::Barrel | Block::Undestroyable => {}
            }
        }
    }

    report.verdict = if report.stubborn > 0 {
        Verdict::Impossible
    } else if report.blocked > 0 || report.foundation > 0 {
        Verdict::NeedsPreparation
    } else {
        Verdict::Ready
    };
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real size of `manual/fundamentals/builder1.nbt`, measured: 1 560 cells.
    const BUILDER1: [u32; 3] = [15, 8, 13];

    fn at_origin(size: [u32; 3]) -> Footprint {
        Footprint::at_size([0, 64, 0], size)
    }

    /// A world where every cell of the box reported so far is air, plus a solid
    /// floor one block under `boxed`.
    fn with_floor(boxed: &Footprint) -> WorldFacts {
        let mut facts = WorldFacts::new();
        let low = boxed.low();
        let high = boxed.high();
        for z in low[2]..=high[2] {
            for x in low[0]..=high[0] {
                facts
                    .blocks
                    .set(Pos::new(x, low[1] - 1, z), Block::Solid);
            }
        }
        facts
    }

    #[test]
    fn a_free_volume_on_a_solid_floor_is_ready() {
        let boxed = at_origin([1, 1, 1]);
        let facts = with_floor(&boxed);
        let report = survey(&facts, &boxed).unwrap();
        assert_eq!(report.verdict, Verdict::Ready);
        assert!(report.is_ready());
        assert!(!report.needs_work());
        assert_eq!(report.volume, 1);
        assert_eq!(report.blocked, 0);
        assert_eq!(report.foundation, 0);
        assert_eq!(report.stubborn, 0);
        assert_eq!(report.first_blocked, None);
        assert_eq!(report.first_foundation, None);
        assert_eq!(report.first_impossible, None);
    }

    #[test]
    fn the_volume_is_the_box_and_nothing_around_it() {
        // `manual/fundamentals/builder1.nbt`, [15, 8, 13] = 1 560 cells.
        let boxed = at_origin(BUILDER1);
        let facts = with_floor(&boxed);
        let report = survey(&facts, &boxed).unwrap();
        assert_eq!(report.volume, 15 * 8 * 13);
        assert_eq!(report.volume, 1560);
        assert_eq!(report.verdict, Verdict::Ready);
        // A block *above* the box is none of its business.
        let mut facts = facts;
        facts
            .blocks
            .set(Pos::new(0, 64 + 8, 0), Block::Solid);
        assert_eq!(survey(&facts, &boxed).unwrap().blocked, 0);
    }

    #[test]
    fn a_solid_in_the_volume_is_work_the_builder_does_not_a_refusal() {
        // `CLEAR`/`CLEAR_NON_SOLIDS` (`AbstractEntityAIStructure.java:418-424`).
        let boxed = at_origin([3, 3, 3]);
        let mut facts = with_floor(&boxed);
        facts.blocks.set(Pos::new(1, 65, 2), Block::Solid);
        let report = survey(&facts, &boxed).unwrap();
        assert_eq!(report.verdict, Verdict::NeedsPreparation);
        assert_eq!(report.blocked, 1);
        assert_eq!(report.first_blocked, Some(Pos::new(1, 65, 2)));
        assert_eq!(report.volume, 27, "the block is inside the box, not beside it");
    }

    #[test]
    fn a_barrel_in_the_volume_is_cleared_like_any_other_block() {
        // `Block::Barrel` is a block entity the rules tell apart, not a wall.
        let boxed = at_origin([2, 2, 2]);
        let mut facts = with_floor(&boxed);
        facts.blocks.set(Pos::new(0, 65, 0), Block::Barrel);
        let report = survey(&facts, &boxed).unwrap();
        assert_eq!(report.verdict, Verdict::NeedsPreparation);
        assert_eq!(report.blocked, 1);
        assert_eq!(report.stubborn, 0);
    }

    #[test]
    fn water_in_the_volume_is_pumped_not_refused() {
        // `CLEAR_WATER` (`:414-417`) is a stage like any other.
        let boxed = at_origin([2, 2, 2]);
        let mut facts = with_floor(&boxed);
        facts.blocks.set(Pos::new(1, 64, 1), Block::Fluid);
        let report = survey(&facts, &boxed).unwrap();
        assert_eq!(report.verdict, Verdict::Ready, "a pond is not a wall");
        assert_eq!(report.blocked, 0);
        assert_eq!(report.foundation, 0);
    }

    #[test]
    fn bedrock_in_the_volume_makes_the_site_impossible() {
        // `skipRemoval` returns true for the substitution blocks only
        // (`:658-668`), and `mineBlock` returns at once for bedrock and
        // `IBuilderUndestroyable` (`AbstractEntityAIInteract.java:160-166`).
        let boxed = at_origin([3, 3, 3]);
        let mut facts = with_floor(&boxed);
        facts
            .blocks
            .set(Pos::new(2, 66, 1), Block::Undestroyable);
        let report = survey(&facts, &boxed).unwrap();
        assert_eq!(report.verdict, Verdict::Impossible);
        assert!(report.needs_work());
        assert_eq!(report.stubborn, 1);
        assert_eq!(report.first_impossible, Some(Pos::new(2, 66, 1)));
        // Impossible outranks "needs work": a bedrock cell is never cleared, so
        // the blocks around it do not make the answer milder.
        let mut facts = facts;
        facts.blocks.set(Pos::new(0, 65, 0), Block::Solid);
        assert_eq!(survey(&facts, &boxed).unwrap().blocked, 1);
        assert_eq!(survey(&facts, &boxed).unwrap().verdict, Verdict::Impossible);
    }

    #[test]
    fn nothing_under_the_box_means_a_foundation_for_every_column() {
        // `BUILD_SOLID` (`:398-402`) fills the floor before the walls go up.
        let boxed = at_origin([4, 2, 3]);
        // A fresh `WorldFacts` reports no block anywhere, so everything is air.
        let report = survey(&WorldFacts::new(), &boxed).unwrap();
        assert_eq!(report.verdict, Verdict::NeedsPreparation);
        assert_eq!(report.foundation, 4 * 3, "one per column, not per cell");
        assert_eq!(report.blocked, 0);
        assert_eq!(
            report.first_foundation,
            Some(Pos::new(0, 63, 0)),
            "the first column, one under the box"
        );
    }

    #[test]
    fn a_barrel_or_bedrock_under_the_box_carries_it() {
        let boxed = at_origin([2, 1, 2]);
        for bearing in [Block::Barrel, Block::Undestroyable, Block::Solid] {
            let mut facts = WorldFacts::new();
            for z in 0..2 {
                for x in 0..2 {
                    facts.blocks.set(Pos::new(x, 63, z), bearing);
                }
            }
            let report = survey(&facts, &boxed).unwrap();
            assert_eq!(report.foundation, 0, "{bearing:?} carries a wall");
            assert_eq!(report.verdict, Verdict::Ready, "{bearing:?}");
        }
        // A fluid does not [carry] either, and it is not air: the box covers four
        // columns (2×2), one of them is water and the other three were never
        // reported — four columns without a bearing floor. `isWeakSolidBlock`
        // rejects a replaceable, collision-less block (`BlockUtils.java:815-818`)
        // and `isAnySolid` is `floatInAir || weakSolid` (`:858-861`), so water is
        // on neither side; Java's `CLEAR_WATER` stage pumps it out instead of
        // building on it (`AbstractEntityAIStructure.java:414-417`).
        let mut facts = WorldFacts::new();
        facts.blocks.set(Pos::new(0, 63, 0), Block::Fluid);
        let report = survey(&facts, &boxed).unwrap();
        assert_eq!(report.foundation, 4, "2×2 columns, and water bears like air");
        assert_eq!(report.blocked, 0, "a fluid is pumped, not cleared like a wall");
        assert_eq!(report.verdict, Verdict::NeedsPreparation);
    }

    #[test]
    fn an_unloaded_chunk_is_a_refusal_and_never_an_empty_site() {
        // Absent is not false: `Blocks::is_loaded` is the caller's fact
        // (`crate::ai::facts`, and `WorldUtil.isBlockLoaded` in Java).
        let boxed = at_origin([3, 3, 3]);
        let mut facts = with_floor(&boxed);
        facts.blocks.mark_unloaded(Pos::new(1, 65, 1));
        assert_eq!(
            survey(&facts, &boxed),
            Err(Refusal::NotLoaded {
                at: Pos::new(1, 65, 1)
            })
        );
        // The same for the floor: the column under the box is read too.
        let mut facts = with_floor(&boxed);
        facts.blocks.mark_unloaded(Pos::new(0, 63, 0));
        assert_eq!(
            survey(&facts, &boxed),
            Err(Refusal::NotLoaded {
                at: Pos::new(0, 63, 0)
            })
        );
    }

    #[test]
    fn a_box_that_crosses_the_build_height_is_refused_before_a_block_is_read() {
        // `manual/fundamentals/townhall5.nbt` is [24, 22, 18], measured.
        let boxed = Footprint::at_size([0, 300, 0], [24, 22, 18]);
        let facts = WorldFacts::new();
        assert_eq!(
            survey(&facts, &boxed),
            Err(Refusal::OutOfWorld {
                low: 300,
                high: 321,
                min_y: -64,
                max_y: 320,
            })
        );
    }

    #[test]
    fn a_survey_of_a_placing_reads_the_boxes_the_turn_makes() {
        // A quarter turn puts the box at `at.x - size_z + 1`, so the box runs
        // x -2..=10, z -3..=11 for `builder1` at (10, 64, -3): a 13×15 floor of
        // 195 columns, not the 15×13 the unrotated template would use.
        let placing = Placing::new("minecolonies:original/fundamentals/builder1", Pos::new(10, 64, -3))
            .with_turn(super::super::place::Turn::Cw90);
        let mut facts = WorldFacts::new();
        let report = site(&facts, &placing, BUILDER1).unwrap();
        assert_eq!(report.volume, 1560);
        assert_eq!(report.foundation, 13 * 15, "13 wide by 15 deep once turned");
        assert_eq!(
            report.first_foundation,
            Some(Pos::new(-2, 63, -3)),
            "the first column of the *turned* box, which the unrotated box never covers"
        );

        // A floor laid under the unrotated box leaves 13 of those columns over
        // (at x = 10) and nothing else: the survey did read the turned box.
        for z in -3..=9 {
            for x in 10..=24 {
                facts.blocks.set(Pos::new(x, 63, z), Block::Solid);
            }
        }
        let report = site(&facts, &placing, BUILDER1).unwrap();
        assert_eq!(report.foundation, 195 - 13);
        assert_eq!(report.first_foundation, Some(Pos::new(-2, 63, -3)));

        // Under the turned box itself, the site is ready.
        for z in -3..=11 {
            for x in -2..=10 {
                facts.blocks.set(Pos::new(x, 63, z), Block::Solid);
            }
        }
        let report = site(&facts, &placing, BUILDER1).unwrap();
        assert_eq!(report.foundation, 0);
        assert_eq!(report.verdict, Verdict::Ready);
    }

    #[test]
    fn the_counts_add_up_to_the_volume() {
        let boxed = at_origin([4, 4, 4]);
        let mut facts = with_floor(&boxed);
        facts.blocks.set(Pos::new(0, 64, 0), Block::Solid);
        facts.blocks.set(Pos::new(1, 64, 1), Block::Solid);
        facts.blocks.set(Pos::new(2, 66, 2), Block::Undestroyable);
        facts.blocks.set(Pos::new(3, 67, 3), Block::Air);
        let report = survey(&facts, &boxed).unwrap();
        assert_eq!(report.volume, 4 * 4 * 4);
        let empty = report.volume - report.blocked - report.stubborn;
        assert_eq!(report.blocked, 2);
        assert_eq!(report.stubborn, 1);
        assert_eq!(empty, 61);
        assert_eq!(report.verdict, Verdict::Impossible);
    }
}
