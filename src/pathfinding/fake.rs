//! A ten-by-ten world, in memory: the four questions [`WorldView`] asks, and
//! nothing else.
//!
//! This is the "faux monde de 10×10" of the brief. It is a test double, not a
//! port of anything, so it has no Java reference: its only job is to make every
//! rule of `search` and `validate` checkable without a game, a chunk or a block
//! state.
//!
//! **The shape.** A floor at `y = -1` (solid at `-1` and below, free above),
//! walls of air, and a box of solid stone on the four sides so no path can
//! leave. Everything else is what a test adds: solid cells, water, doors,
//! ladders, and terraces dug by [`Grid::dig_strip`].

use std::collections::{BTreeMap, BTreeSet};

use super::node::Pos;
use super::world::WorldView;

/// The size of the square, in blocks.
const SIZE: i32 = 10;

/// The `y` of the topmost solid block of a column where nothing was dug.
const GROUND: i32 = -1;

/// The fake world: which cells say yes to what.
#[derive(Clone, Debug, Default)]
pub struct Grid {
    solid: BTreeSet<Pos>,
    water: BTreeSet<Pos>,
    doors: BTreeSet<Pos>,
    ladders: BTreeSet<Pos>,
    /// Dug columns: `(x, z)` → the `y` of their topmost solid block.
    ground: BTreeMap<(i32, i32), i32>,
}

impl Grid {
    /// A flat ten-by-ten room with a stone floor.
    pub fn new() -> Grid {
        Grid::default()
    }

    /// A block that stops the body.
    pub fn set_solid(&mut self, pos: Pos) {
        self.solid.insert(pos);
    }

    /// A water cell: the body swims where it stands.
    pub fn set_water(&mut self, pos: Pos) {
        self.water.insert(pos);
    }

    /// A door leaf, which counts as enterable when the options allow it.
    pub fn set_door(&mut self, pos: Pos) {
        self.doors.insert(pos);
    }

    /// A ladder rung.
    pub fn set_ladder(&mut self, pos: Pos) {
        self.ladders.insert(pos);
    }

    /// Lower the floor by `depth` blocks in one column: a hole in the ground,
    /// with a floor at the bottom of it.
    pub fn dig(&mut self, x: i32, z: i32, depth: i32) {
        self.ground.insert((x, z), GROUND - depth);
    }

    /// Lower the floor by `depth` blocks for every column with `z` in
    /// `z_from..z_to`, all `x`: a terrace, or a drop with no way around it.
    pub fn dig_strip(&mut self, z_from: i32, z_to: i32, depth: i32) {
        for x in 0..SIZE {
            for z in z_from..z_to {
                self.ground.insert((x, z), GROUND - depth);
            }
        }
    }

    /// The `y` of the topmost solid block of a column.
    fn floor_of(&self, x: i32, z: i32) -> i32 {
        self.ground.get(&(x, z)).copied().unwrap_or(GROUND)
    }
}

impl WorldView for Grid {
    fn is_solid(&self, pos: Pos) -> bool {
        // The box: nothing leaves the ten-by-ten.
        if pos[0] < 0 || pos[0] >= SIZE || pos[2] < 0 || pos[2] >= SIZE {
            return true;
        }
        // Water and a ladder take the cell: a ladder in a wall is a ladder.
        // **A door does not**: closed, it is a collision shape like any other
        // block, and it is `Costs::doors_open` that lets a body through.
        if self.water.contains(&pos) || self.ladders.contains(&pos) {
            return false;
        }
        if pos[1] <= self.floor_of(pos[0], pos[2]) {
            return true;
        }
        self.solid.contains(&pos)
    }

    fn is_water(&self, pos: Pos) -> bool {
        self.water.contains(&pos)
    }

    fn is_climbable(&self, pos: Pos) -> bool {
        self.ladders.contains(&pos)
    }

    fn is_door(&self, pos: Pos) -> bool {
        self.doors.contains(&pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pathfinding::node::{Costs, at};
    use crate::pathfinding::world::{free, resting};

    #[test]
    fn the_floor_is_solid_and_the_air_is_free() {
        let grid = Grid::new();
        assert!(grid.is_solid(at(0, -1, 0)), "the floor");
        assert!(!grid.is_solid(at(0, 0, 0)), "the air above it");
        assert!(grid.is_solid(at(-1, 0, 0)), "the box");
        assert!(free(&grid, &Costs::default(), at(5, 0, 5)));
        assert!(resting(&grid, &Costs::default(), at(5, 0, 5)));
    }

    #[test]
    fn a_terrace_lowers_the_floor_and_keeps_the_air_above_it() {
        let mut grid = Grid::new();
        grid.dig_strip(6, 10, 4);
        assert!(grid.is_solid(at(2, -5, 6)), "the new floor");
        assert!(!grid.is_solid(at(2, -4, 6)), "the air on top of it");
        assert!(
            grid.is_solid(at(2, -1, 5)),
            "the old floor stops at the terrace"
        );
    }

    #[test]
    fn a_ladder_in_a_wall_leaves_a_hole_to_climb() {
        let mut grid = Grid::new();
        grid.set_solid(at(3, 0, 3));
        grid.set_solid(at(3, 1, 3));
        grid.set_ladder(at(3, 1, 3));
        assert!(grid.is_solid(at(3, 0, 3)), "the wall stays");
        assert!(!grid.is_solid(at(3, 1, 3)), "the ladder does not");
        assert!(grid.is_climbable(at(3, 1, 3)));
    }
}
