//! The distances Java has, and the two boxes it tests a point against.
//!
//! `api/util/BlockPosUtil.java` offers **seven** ways to say "how far", and the
//! mod uses each one for a different question. Porting one and calling it
//! `distance` would silently change a rule, so they are all here, named after
//! what they compute rather than after the Java method:
//!
//! | Here | Java | Shape | Who asks it |
//! | --- | --- | --- | --- |
//! | [`dist_sqr`] | `getDistanceSquared` (`:401-417`) | 3D, squared, `i64` | the request resolver's suitability |
//! | [`dist_sqr_2d`] | `getDistanceSquared2D` (`:469-483`) | x/z squared, `i64` | `IColony.getDistanceSquared` (`IColony.java:88`) |
//! | [`dist_2d_sum`] | `getDistance2D` (`:423-431`) | \|dx\| + \|dz\| | the zero-chunk warning (`IColonyTagCapability.java:190`) |
//! | [`dist_2d_max`] | `getMaxDistance2D` (`:438-446`) | max(\|dx\|, \|dz\|) | a square scan |
//! | [`dist`] | `getDistance` (`:453-461`) | 3D, root, `f64` | founding: too close to another colony (`ColonyManager.java:316`) |
//! | [`dist_2d`] | `MathUtils.twoDimDistance` (`:43-55`) | x/z, `hypot` | founding: distance to world spawn (`GetColonyInfoMessage.java:95`) |
//! | [`dist_manhattan`] | `distManhattan` (`:486-492`) | \|dx\|+\|dy\|+\|dz\| | walking cost |
//!
//! **The 3D/2D split is not cosmetic.** Founding a colony compares a **3D**
//! distance to the nearest town hall (`ColonyManager.java:316`) but a **2D** one
//! to the world spawn (`GetColonyInfoMessage.java:95`). A town hall 200 blocks up
//! a mountain is farther from its neighbour than the same hut on the flat, and
//! Java says so; a colony dug into a hill is not farther from spawn. `territory`
//! keeps both, each named for the question.
//!
//! **`hypot`, not `sqrt(dx² + dz²)`.** [`dist_2d`] uses `f64::hypot` because
//! Java does, and the difference is visible at the world border: `dx = 30 000 000`
//! squared is `9e14`, which `f64` holds and a naive `i32` product does not
//! (`MathUtils.java:51-54` says exactly this in a comment).

use super::constants::BLOCKS_PER_CHUNK;

/// Three-dimensional distance, squared, in `i64` — `BlockPosUtil.getDistanceSquared`
/// (`api/util/BlockPosUtil.java:401-417`).
///
/// Java throws `IllegalStateException` when the sum comes out negative, which is
/// its overflow detector (`:410-415`). Vanilla's ±30 M border keeps the worst case
/// at `3 × (6e7)² ≈ 1.1e16`, well under `i64::MAX`, so the port keeps the `i64`
/// sum and asserts the sign instead of throwing.
pub fn dist_sqr(a: [i32; 3], b: [i32; 3]) -> i64 {
    let dx = a[0] as i64 - b[0] as i64;
    let dy = a[1] as i64 - b[1] as i64;
    let dz = a[2] as i64 - b[2] as i64;
    let sum = dx * dx + dy * dy + dz * dz;
    debug_assert!(sum >= 0, "max-sqrt is too high: {dx} | {dy} | {dz}");
    sum
}

/// Distance in the x/z plane, squared, in `i64` — `getDistanceSquared2D`
/// (`api/util/BlockPosUtil.java:469-483`). What `IColony.getDistanceSquared`
/// returns (`api/colony/IColony.java:88`), so a colony's "distance to its centre"
/// is never a 3D one.
pub fn dist_sqr_2d(a: [i32; 3], b: [i32; 3]) -> i64 {
    let dx = a[0] as i64 - b[0] as i64;
    let dz = a[2] as i64 - b[2] as i64;
    let sum = dx * dx + dz * dz;
    debug_assert!(sum >= 0, "max-sqrt is too high: {dx} | {dz}");
    sum
}

/// `|dx| + |dz|` — `getDistance2D` (`api/util/BlockPosUtil.java:423-431`). Not a
/// distance anyone would call one, but Java uses it as a cheap upper bound.
pub fn dist_2d_sum(a: [i32; 3], b: [i32; 3]) -> i64 {
    let dx = (a[0] as i64 - b[0] as i64).abs();
    let dz = (a[2] as i64 - b[2] as i64).abs();
    dx + dz
}

/// `max(|dx|, |dz|)` — `getMaxDistance2D` (`api/util/BlockPosUtil.java:438-446`):
/// the side of the smallest square, aligned on the axes, that holds both points.
pub fn dist_2d_max(a: [i32; 3], b: [i32; 3]) -> i32 {
    let dx = (a[0] - b[0]).abs();
    let dz = (a[2] - b[2]).abs();
    dx.max(dz)
}

/// Three-dimensional distance, root taken: `getDistance`
/// (`api/util/BlockPosUtil.java:453-461`). A `sqrt` of [`dist_sqr`], which is what
/// Java computes.
pub fn dist(a: [i32; 3], b: [i32; 3]) -> f64 {
    (dist_sqr(a, b) as f64).sqrt()
}

/// Distance in the x/z plane, root taken with `hypot` — `MathUtils.twoDimDistance`
/// (`api/util/MathUtils.java:43-55`).
pub fn dist_2d(a: [i32; 3], b: [i32; 3]) -> f64 {
    let dx = a[0] as f64 - b[0] as f64;
    let dz = a[2] as f64 - b[2] as f64;
    dx.hypot(dz)
}

/// `|dx| + |dy| + |dz|` — `distManhattan` (`api/util/BlockPosUtil.java:486-492`).
pub fn dist_manhattan(a: [i32; 3], b: [i32; 3]) -> i32 {
    let dx = (a[0] - b[0]).abs();
    let dy = (a[1] - b[1]).abs();
    let dz = (a[2] - b[2]).abs();
    dx + dy + dz
}

/// `value` held between `min` and `max` — `MathUtils.clamp`
/// (`api/util/MathUtils.java:62-65`). Java's `Math.max(min, Math.min(max, value))`
/// keeps a reversed pair answering `min`, and so does this.
pub fn clamp(value: i32, min: i32, max: i32) -> i32 {
    // The nesting order is Java's: inner `min(max, value)`, outer `max(min, …)`.
    value.min(max).max(min)
}

/// An axis-aligned box of blocks, inclusive on both ends: vanilla's `BoundingBox`,
/// which MineColonies uses for areas (`api/util/BlockPosUtil.java:591-600`,
/// `:1189-1232`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct BlockBox {
    pub min: [i32; 3],
    pub max: [i32; 3],
}

impl BlockBox {
    /// The box between two corners **in either order**: `isInArea` sorts each
    /// axis before comparing (`api/util/BlockPosUtil.java:1192-1211`), so a caller
    /// that hands the corners the other way round gets the same box.
    pub fn from_corners(a: [i32; 3], b: [i32; 3]) -> BlockBox {
        BlockBox {
            min: [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])],
            max: [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])],
        }
    }

    /// The box of a chunk-aligned scan: `pos`'s chunk, widened by `chunk_radius`
    /// chunks on every axis **including y** — `BlockPosUtil.getChunkAlignedBB`
    /// (`api/util/BlockPosUtil.java:591-600`).
    ///
    /// Java's `pos.getX() & ~15` is a floor, and so is `pos[0] & !15`; `y` is
    /// treated like the other two, which is why the result can leave the world's
    /// height — Java's own comment says to clip before using it (`:588-590`).
    pub fn chunk_aligned(pos: [i32; 3], chunk_radius: i32) -> BlockBox {
        let radius = chunk_radius * BLOCKS_PER_CHUNK;
        let x = pos[0] & !(BLOCKS_PER_CHUNK - 1);
        let y = pos[1] & !(BLOCKS_PER_CHUNK - 1);
        let z = pos[2] & !(BLOCKS_PER_CHUNK - 1);
        BlockBox {
            min: [x - radius, y - radius, z - radius],
            max: [
                x + radius + BLOCKS_PER_CHUNK - 1,
                y + radius + BLOCKS_PER_CHUNK - 1,
                z + radius + BLOCKS_PER_CHUNK - 1,
            ],
        }
    }

    /// Is `pos` in the box, **with a block of slack on every side**? This is
    /// `BlockPosUtil.isInArea` (`api/util/BlockPosUtil.java:1213-1232`): Java
    /// compares `>= x1 - 1` and `<= x2 + 1` on all three axes, so a position one
    /// block outside the corner still counts. That slack is deliberate — it is
    /// what lets a blueprint hanging one block over its own corner still be
    /// "in area" — and [`BlockBox::contains_exact`] is the question Java never
    /// asks.
    pub fn contains(&self, pos: [i32; 3]) -> bool {
        (0..3_usize).all(|axis| pos[axis] >= self.min[axis] - 1 && pos[axis] <= self.max[axis] + 1)
    }

    /// Is `pos` in the box with no slack? Not a Java method: it is here so a
    /// caller that wants the plain reading does not have to subtract one from a
    /// bound it already has.
    pub fn contains_exact(&self, pos: [i32; 3]) -> bool {
        (0..3_usize).all(|axis| pos[axis] >= self.min[axis] && pos[axis] <= self.max[axis])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_squared_distance_is_the_three_axis_one() {
        assert_eq!(dist_sqr([0, 0, 0], [0, 0, 0]), 0);
        assert_eq!(dist_sqr([0, 0, 0], [3, 0, 4]), 25);
        assert_eq!(dist_sqr([0, 0, 0], [1, 2, 2]), 9);
        assert_eq!(dist_sqr([10, 70, -5], [10, 70, -5]), 0);
        // Symmetric, and the sign of the difference does not matter.
        assert_eq!(dist_sqr([-7, 3, 2], [5, -1, 9]), dist_sqr([5, -1, 9], [-7, 3, 2]));
    }

    #[test]
    fn the_two_dimensional_squared_distance_ignores_y_while_the_other_counts_it() {
        let a = [0, 0, 0];
        let tall = [3, 400, 4];
        assert_eq!(dist_sqr_2d(a, tall), 25);
        assert_eq!(dist_sqr(a, tall), 25 + 160_000);
        assert_ne!(dist_sqr(a, tall), dist_sqr_2d(a, tall));
    }

    #[test]
    fn the_two_cheap_variants_answer_two_other_numbers() {
        // `getDistance2D` (`:423-431`) and `getMaxDistance2D` (`:438-446`).
        assert_eq!(dist_2d_sum([0, 0, 0], [3, 100, 4]), 7);
        assert_eq!(dist_2d_max([0, 0, 0], [3, 100, 4]), 4);
        assert_eq!(dist_2d_max([0, 0, 0], [-9, 0, 2]), 9);
        // Neither is the euclidean one, which is 5.
        assert!((dist_2d([0, 0, 0], [3, 100, 4]) - 5.0).abs() < 1e-12);
    }

    #[test]
    fn the_three_dimensional_distance_is_the_root_of_its_square() {
        assert!((dist([0, 0, 0], [3, 4, 12]) - 13.0).abs() < 1e-12);
        assert_eq!(dist([5, 5, 5], [5, 5, 5]), 0.0);
        let a = [1_000, -64, 2_000];
        let b = [-3_000, 320, 4_000];
        assert!((dist(a, b) - (dist_sqr(a, b) as f64).sqrt()).abs() < 1e-9);
    }

    #[test]
    fn the_hypot_form_holds_the_world_border_where_an_int_product_would_not() {
        // `MathUtils.java:51-54`: hypot exists so the squares do not overflow.
        // 30 M is vanilla's border; dx² alone is 9e14, past `i32::MAX` (2.1e9).
        let far = dist_2d([0, 0, 0], [30_000_000, 0, 30_000_000]);
        assert!(far.is_finite());
        assert!((far - 42_426_406.871_192_85).abs() < 1e-3, "{far}");
        // And it is wider than the 2D squared distance is representable in i32.
        assert!(dist_sqr_2d([0, 0, 0], [30_000_000, 0, 30_000_000]) > i64::from(i32::MAX));
    }

    #[test]
    fn the_manhattan_distance_counts_every_axis() {
        assert_eq!(dist_manhattan([0, 0, 0], [1, 2, 3]), 6);
        assert_eq!(dist_manhattan([0, 0, 0], [-1, -2, -3]), 6);
        assert_eq!(dist_manhattan([7, 7, 7], [7, 7, 7]), 0);
    }

    #[test]
    fn clamp_keeps_the_bound_and_answers_min_for_a_reversed_pair() {
        // `MathUtils.java:62-65` is `max(min, min(max, value))`.
        assert_eq!(clamp(5, 0, 10), 5);
        assert_eq!(clamp(-5, 0, 10), 0);
        assert_eq!(clamp(50, 0, 10), 10);
        assert_eq!(clamp(5, 10, 0), 10);
    }

    #[test]
    fn a_box_sorts_its_own_corners_whichever_way_it_is_given_them() {
        // `isInArea` orders each axis itself (`:1192-1211`).
        let box_ = BlockBox::from_corners([10, 70, 10], [0, 64, 0]);
        assert_eq!(box_.min, [0, 64, 0]);
        assert_eq!(box_.max, [10, 70, 10]);
        assert_eq!(box_, BlockBox::from_corners([0, 64, 0], [10, 70, 10]));
    }

    #[test]
    fn a_box_holds_one_block_of_slack_because_is_in_area_does() {
        let box_ = BlockBox::from_corners([0, 64, 0], [10, 70, 10]);
        assert!(box_.contains_exact([0, 64, 0]));
        assert!(box_.contains_exact([10, 70, 10]));
        assert!(!box_.contains_exact([11, 70, 5]));
        // The slack: one block out on any axis is still "in area".
        assert!(box_.contains([11, 70, 5]));
        assert!(box_.contains([-1, 64, 0]));
        assert!(box_.contains([0, 63, 0]));
        assert!(box_.contains([5, 71, 5]));
        // Two blocks out on one axis is not.
        assert!(!box_.contains([12, 70, 5]));
        assert!(!box_.contains([5, 72, 5]));
    }

    #[test]
    fn the_chunk_aligned_box_is_the_whole_chunk_and_its_neighbours() {
        // `getChunkAlignedBB` (`:591-600`) with radius 0 is one chunk, and the
        // position may sit anywhere inside it.
        let one = BlockBox::chunk_aligned([7, 70, -3], 0);
        assert_eq!(one.min, [0, 64, -16]);
        assert_eq!(one.max, [15, 79, -1]);
        assert!(one.contains_exact([0, 64, -16]));
        assert!(one.contains_exact([15, 79, -1]));
        assert!(!one.contains_exact([16, 64, -16]));

        // Radius 1 is nine chunks: 48 blocks a side.
        let nine = BlockBox::chunk_aligned([7, 70, -3], 1);
        assert_eq!(nine.min, [-16, 48, -32]);
        assert_eq!(nine.max, [31, 95, 15]);
        assert_eq!(nine.max[0] - nine.min[0] + 1, 3 * BLOCKS_PER_CHUNK);
        assert_eq!(nine.max[2] - nine.min[2] + 1, 3 * BLOCKS_PER_CHUNK);
        // Two positions in the same chunk **column of sections** give the same
        // box: y = 5 and y = 15 are the same 16-block section, so `& ~15` lands
        // on 0 both times.
        assert_eq!(
            BlockBox::chunk_aligned([0, 5, 0], 1),
            BlockBox::chunk_aligned([15, 15, 15], 1)
        );
        // y is aligned like x and z (`BlockPosUtil.java:594-598`: `y1 = pos.getY()
        // & ~15`, then `blockRadius` added on all three axes), so another section
        // is another box: y = 200 sits in the section starting at 192.
        let up = BlockBox::chunk_aligned([0, 20, 0], 1);
        assert_eq!(up.min[1], 0);
        assert_eq!(up.max[1], 47);
        let higher = BlockBox::chunk_aligned([0, 200, 15], 1);
        assert_eq!(higher.min[1], 176);
        assert_eq!(higher.max[1], 223);
        assert_ne!(
            BlockBox::chunk_aligned([0, 5, 0], 1),
            BlockBox::chunk_aligned([15, 200, 15], 1)
        );
    }
}
