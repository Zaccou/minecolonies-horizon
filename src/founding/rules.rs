//! How far a colony must be from what: the four settings, and the maths they are
//! read in.
//!
//! Java reference: `api/configuration/ServerConfiguration.java:165-168` (the four
//! numbers), `core/colony/ColonyManager.java:311-324` (two of them become one
//! range), `core/util/ChunkDataHelper.java:135-139` (the ground a colony claims),
//! `api/util/BlockPosUtil.java:453-481` (the two distances).
//!
//! | Field | Java | Default | Bounds |
//! | --- | --- | ---: | --- |
//! | `min_colony_distance` | `minColonyDistance` (`:165`) | 8 chunks | 1..=200 |
//! | `initial_colony_size` | `initialColonySize` (`:166`) | 4 chunks | 1..=15 |
//! | `max_from_spawn` | `maxdistancefromworldspawn` (`:167`) | 30 000 blocks | 1 000..=i32::MAX |
//! | `min_from_spawn` | `mindistancefromworldspawn` (`:168`) | 0 blocks | 0..=1 000 |
//!
//! The bounds are **not** enforced: a server owner owns them, and a rule that
//! clamped would hide a configuration mistake.
//!
//! **Two distances, two shapes, and that asymmetry is Java's.** The distance to
//! another colony is three-dimensional (`BlockPosUtil.java:453-460`, reached from
//! `ColonyManager.java:316`), the distance to the world spawn is two-dimensional
//! (`:469-481`, reached from `CreateColonyMessage.java:147`). Both are decided on
//! the squared distance, in integers; [`isqrt`] only turns it into a number a
//! refusal can print.

use std::cmp::max;

/// `Constants.java:55`: the number of blocks on a chunk's side.
pub const BLOCKS_PER_CHUNK: i32 = 16;

/// The four numbers a founding is measured with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Rules {
    /// The least distance between two town halls, in **chunks**
    /// (`ServerConfiguration.java:165`); Java multiplies it by 16
    /// (`ColonyManager.java:566-571`).
    pub min_colony_distance: u32,
    /// The radius a colony claims when it is founded, in **chunks**
    /// (`ServerConfiguration.java:166`, used at `ChunkDataHelper.java:137`).
    pub initial_colony_size: u32,
    /// The least distance from the world spawn, in **blocks** (`:168`).
    pub min_from_spawn: u32,
    /// The greatest distance from the world spawn, in **blocks** (`:167`).
    pub max_from_spawn: u32,
}

impl Rules {
    /// The four defaults of `ServerConfiguration.java:165-168`.
    pub const DEFAULT: Rules = Rules {
        min_colony_distance: 8,
        initial_colony_size: 4,
        min_from_spawn: 0,
        max_from_spawn: 30_000,
    };

    /// The range two town halls keep, **in blocks**: the larger of the two chunk
    /// settings, in blocks (`ColonyManager.java:313`). Whichever is bigger wins,
    /// so a big initial claim cannot be founded next to a small
    /// `minColonyDistance`.
    pub fn block_range(&self) -> i64 {
        i64::from(max(self.min_colony_distance, self.initial_colony_size))
            * i64::from(BLOCKS_PER_CHUNK)
    }

    /// The radius a colony claims, in chunks (`ChunkDataHelper.java:135-139`).
    pub fn claim_radius_chunks(&self) -> i32 {
        self.initial_colony_size as i32
    }

    /// The chunk a position is in: Java's `pos.getX() >> 4`
    /// (`ChunkDataHelper.java:236-237`), a floor division — which is why
    /// `div_euclid`, and not `/`, is what agrees with it on negative coordinates.
    pub fn chunk(at: [i32; 3]) -> [i32; 2] {
        [
            at[0].div_euclid(BLOCKS_PER_CHUNK),
            at[2].div_euclid(BLOCKS_PER_CHUNK),
        ]
    }

    /// Whether `at` is inside the colony centred on `center`: its chunk must be in
    /// the claimed box, both ends included (`Colony.java:1298-1308` reads
    /// `isCoordInColony` off chunk ownership; the box is
    /// `ChunkDataHelper.java:135-139`). A box in the ground, not a sphere: nothing
    /// above or below the centre's own chunk column is outside.
    pub fn inside_colony(&self, center: [i32; 3], at: [i32; 3]) -> bool {
        let middle = Rules::chunk(center);
        let there = Rules::chunk(at);
        let radius = self.claim_radius_chunks();
        (there[0] - middle[0]).abs() <= radius && (there[1] - middle[1]).abs() <= radius
    }

    /// The distance between two positions in three dimensions, rounded down:
    /// `BlockPosUtil.getDistance` (`:453-460`).
    pub fn distance(from: [i32; 3], to: [i32; 3]) -> i64 {
        isqrt(squared(from, to, true))
    }

    /// The same, ignoring the height: `BlockPosUtil.getDistanceSquared2D`
    /// (`:469-481`).
    pub fn distance_2d(from: [i32; 3], to: [i32; 3]) -> i64 {
        isqrt(squared(from, to, false))
    }
}

impl Default for Rules {
    fn default() -> Rules {
        Rules::DEFAULT
    }
}

/// The squared distance between two positions, in `i128` so two `i32` extremes
/// cannot overflow it. Java computes it in a `long` and can
/// (`BlockPosUtil.java:455-459` subtracts two `int`s before widening them), which
/// is not worth reproducing.
///
/// **The two axes are not `0..2`.** The 3D measure sums x, y and z
/// (`BlockPosUtil.java:455-459`); the 2D one sums **x and z** and drops the height
/// (`:471-473`: `xDiff` and `zDiff`, no `yDiff`). Taking the first two axes would
/// silently measure x and y, which is a wall's height, not a distance on the map.
fn squared(from: [i32; 3], to: [i32; 3], with_height: bool) -> i128 {
    let axes: &[usize] = if with_height { &[0, 1, 2] } else { &[0, 2] };
    axes.iter()
        .map(|axis| {
            let difference = i128::from(from[*axis]) - i128::from(to[*axis]);
            difference * difference
        })
        .sum()
}

/// The integer square root of a non-negative number, rounded down: Newton's method
/// on integers, exact at every step, so `isqrt(15)` is 3 and not the 3.87 a float
/// would leave behind. Capped at `i64::MAX`, which two `i32` positions can never
/// reach (√3 · 2³² is about 7.4 · 10⁹).
fn isqrt(number: i128) -> i64 {
    if number <= 0 {
        return 0;
    }
    let mut root = number;
    let mut next = (root + 1) / 2;
    while next < root {
        root = next;
        next = (root + number / root) / 2;
    }
    i64::try_from(root).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_range_between_two_town_halls_is_the_larger_chunk_setting_in_blocks() {
        assert_eq!(Rules::DEFAULT.block_range(), 128, "8 chunks of 16 blocks");
        let wide = Rules { min_colony_distance: 2, initial_colony_size: 15, ..Rules::DEFAULT };
        assert_eq!(wide.block_range(), 240, "the initial claim wins when it is bigger");
        let far = Rules { min_colony_distance: 20, initial_colony_size: 4, ..Rules::DEFAULT };
        assert_eq!(far.block_range(), 320, "and the distance wins when it is");
        assert_eq!(Rules::DEFAULT.claim_radius_chunks(), 4);
    }

    #[test]
    fn a_colony_owns_a_box_of_chunks_and_its_edge_is_where_java_puts_it() {
        let rules = Rules::DEFAULT;
        let center = [0, 70, 0];
        // Chunk radius 4 around chunk 0: blocks -64 ..= 79 on both axes.
        assert!(rules.inside_colony(center, [0, 70, 0]));
        assert!(rules.inside_colony(center, [79, 12, 79]), "the last chunk of the box");
        assert!(!rules.inside_colony(center, [80, 70, 0]), "one block further is out");
        assert!(rules.inside_colony(center, [-64, 200, -64]), "the box has a floor");
        assert!(!rules.inside_colony(center, [-65, 70, 0]));
        assert_eq!(Rules::chunk([-1, 0, -1]), [-1, -1], ">> 4 is a floor division");
        assert_eq!(Rules::chunk([15, 0, 16]), [0, 1]);
    }

    #[test]
    fn the_two_distances_are_the_two_java_measures() {
        assert_eq!(Rules::distance([0, 0, 0], [3, 4, 0]), 5);
        assert_eq!(
            Rules::distance_2d([0, 0, 0], [3, 12, 4]),
            5,
            "x and z only: the height of 12 is not in it"
        );
        assert_eq!(
            Rules::distance([0, 0, 0], [3, 12, 4]),
            13,
            "the same pair in three dimensions: 9 + 144 + 16"
        );
        assert_eq!(Rules::distance_2d([0, 0, 0], [-3, 0, -4]), 5, "sign is not a distance");
        assert_eq!(Rules::distance([0, 0, 0], [1, 1, 0]), 1, "truncated, as Java compares");
        // No overflow at the extremes of an `i32` coordinate.
        assert_eq!(
            Rules::distance_2d([i32::MAX, 0, 0], [i32::MIN, 0, 0]),
            4_294_967_295,
            "the gap between the two ends of an i32"
        );
    }
}
