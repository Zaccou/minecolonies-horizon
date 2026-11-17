//! One claim, one world, one colony, for the tests of this module.
//!
//! **Shared once rather than copied four times.** `guard`, `intent` and `mark` all need
//! the same three things — a claim book owning one chunk, a world answering one occupant
//! per cell, a colony record — and a copy in each `mod tests` would let the fakes drift
//! apart. Test support only: nothing here is compiled outside `cfg(test)`.
//!
//! Java reference for the shapes: `Colony.java:1298-1308` (the claim is the chunk's
//! owner, not a radius) and `BlockPosUtil.java:591-598` (a chunk is 16 blocks on a side).

use crate::colony::Colony;

use super::guard::{Claim, Occupant, Protection, World};

/// The dimension every fake answers for.
pub const OVERWORLD: &str = "minecraft:overworld";

/// Colony 7, centred on `[8, 70, 8]`, founded by player `1`.
pub fn colony() -> Colony {
    Colony::new(7, "Bourg", OVERWORLD, [8, 70, 8], 1)
}

/// The chunk a position is in: `pos.getX() >> 4` (`ChunkDataHelper.java:236-237`), a
/// floor division, so `div_euclid`.
pub fn chunk_of(at: [i32; 3]) -> [i32; 2] {
    [at[0].div_euclid(16), at[2].div_euclid(16)]
}

/// A claim book where one colony owns exactly one chunk.
pub struct OneOwner {
    pub id: i32,
    pub chunk: [i32; 2],
}

impl Claim for OneOwner {
    fn owner_at(&self, _dimension: &str, at: [i32; 3]) -> Option<i32> {
        (chunk_of(at) == self.chunk).then_some(self.id)
    }
}

/// Colony 7 owning the chunk of `[8, 70, 8]`, which is `[0, 0]`.
///
/// A `static`, not a function: `Guard` borrows its [`Claim`] for the whole lifetime of the
/// guard, and a temporary from a call would not live long enough to be stored in a `let`.
pub static OWNED: OneOwner = OneOwner { id: 7, chunk: [0, 0] };

/// A claim book in which nobody owns anything: open wilderness.
pub struct Unclaimed;

impl Claim for Unclaimed {
    fn owner_at(&self, _dimension: &str, _at: [i32; 3]) -> Option<i32> {
        None
    }
}

/// A world holding one kind of cell everywhere, with the colony's huts where it is told.
pub struct Cells {
    pub occupant: Occupant,
    pub huts: Vec<[i32; 3]>,
}

impl Cells {
    pub fn of(occupant: Occupant) -> Cells {
        Cells { occupant, huts: Vec::new() }
    }

    /// A world of solid blocks with a hut at each position given.
    pub fn with_huts(huts: &[[i32; 3]]) -> Cells {
        Cells { occupant: Occupant::Solid, huts: huts.to_vec() }
    }
}

impl World for Cells {
    fn occupant(&self, _at: [i32; 3]) -> Occupant {
        self.occupant
    }

    fn protection(&self, at: [i32; 3]) -> Option<Protection> {
        self.huts.contains(&at).then_some(Protection::Hut { at })
    }
}

/// A world answering per cell: the ones it was given, replaceable everywhere else. What a
/// test needs when one batch touches a cell that holds a block and another that does not.
pub struct Mixed {
    pub cells: Vec<([i32; 3], Occupant)>,
}

impl Mixed {
    pub fn new(cells: &[([i32; 3], Occupant)]) -> Mixed {
        Mixed { cells: cells.to_vec() }
    }
}

impl World for Mixed {
    fn occupant(&self, at: [i32; 3]) -> Occupant {
        self.cells
            .iter()
            .find(|(cell, _)| *cell == at)
            .map(|(_, occupant)| *occupant)
            .unwrap_or(Occupant::Replaceable)
    }
}
