//! Block ↔ chunk ↔ section, and the square of chunks a colony claims.
//!
//! Java's conversions live in vanilla, not in MineColonies: `ChunkPos(BlockPos)`
//! and `SectionPos.blockToSectionCoord` are net.minecraft, absent from
//! `minecolonies-src`. What MineColonies *does* own is the arithmetic on top of
//! them — the claim loop (`core/util/ChunkDataHelper.java:339-345`), the
//! chunk-distance comparison that caps a claim (`:248`), and the chunk-aligned
//! box a scan uses (`api/util/BlockPosUtil.java:591-600`) — and that is what this
//! file ports.
//!
//! **The one conversion that is easy to get wrong is the negative one.** A block
//! at `x = -1` is in chunk `-1`, not chunk `0`: the conversion is a floor, which
//! in Rust and Java alike is `>> 4` on a signed integer and **not** `/ 16`
//! (`-1 / 16 == 0`, `-1 >> 4 == -1`). The SDK says it in a comment on its own
//! `BlockPos::section` (`.horizon/sdk/api/crates/horizon-core/src/position.rs:38-46`);
//! [`chunk_of`] and [`section_of`] repeat it here rather than wrap
//! `mod_api::BlockPos`, because the rest of the port speaks `[i32; 3]`.
//!
//! **Why the port has its own `ChunkPos`.** The SDK has one
//! (`position.rs:101-117`) with the same pack, but you can only build it from a
//! `BlockPos`, which the port does not carry. Two fields and a shift are cheaper
//! than a conversion at every boundary.

use super::constants::BLOCKS_PER_CHUNK;

/// The chunk a block is in, `[chunk x, chunk z]`.
///
/// Ordered and `Copy` so it keys a `BTreeSet` — which is how [`ChunkSquare`]
/// membership stays deterministic and how `claims` keeps a sorted set of ids.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct ChunkPos {
    pub x: i32,
    pub z: i32,
}

impl ChunkPos {
    pub const fn new(x: i32, z: i32) -> ChunkPos {
        ChunkPos { x, z }
    }

    /// The chunk a block is in. A floor: `-1` is in chunk `-1`, not `0`.
    pub const fn of(pos: [i32; 3]) -> ChunkPos {
        ChunkPos {
            x: pos[0] >> 4,
            z: pos[2] >> 4,
        }
    }

    /// The first block of this chunk (vanilla's `ChunkPos.getWorldPosition`, the
    /// position `ChunkDataHelper` claims with — `:345`). `y` is `0`, as there:
    /// a claim is a column, not a box.
    pub const fn origin(self) -> [i32; 3] {
        [self.x * BLOCKS_PER_CHUNK, 0, self.z * BLOCKS_PER_CHUNK]
    }

    /// Vanilla's `ChunkPos.asLong`: x in the low half, z in the high half
    /// (`.horizon/sdk/api/crates/horizon-core/src/position.rs:112-114`). The port
    /// keeps the packing because a save and a claim log both key on it.
    pub const fn pack(self) -> i64 {
        ((self.x as u32 as u64) | ((self.z as u32 as u64) << 32)) as i64
    }

    pub const fn unpack(value: i64) -> ChunkPos {
        ChunkPos::new(value as i32, (value >> 32) as i32)
    }

    /// Chunk-to-chunk distance, squared, ignoring y:
    /// `BlockPosUtil.chunkDistanceSquared` (`api/util/BlockPosUtil.java:1237-1242`).
    /// Java compares this against `maxColonySize²` to refuse a claim
    /// (`core/util/ChunkDataHelper.java:248`); so does [`super::territory`].
    ///
    /// The difference of two chunk coordinates is an `i32`: a world wider than
    /// `2³¹` chunks would overflow it. Vanilla's border is at ±30 M blocks, so a
    /// chunk is within ±1.9 M and the subtraction cannot; a debug build would
    /// panic rather than wrap if it somehow did, which is the honest failure.
    pub const fn distance_squared(self, other: ChunkPos) -> i32 {
        let dx = self.x - other.x;
        let dz = self.z - other.z;
        dx * dx + dz * dz
    }
}

/// The 16³ section a block is in: vanilla's `SectionPos`, three chunk-sized
/// divisions including y.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct SectionPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl SectionPos {
    pub const fn new(x: i32, y: i32, z: i32) -> SectionPos {
        SectionPos { x, y, z }
    }

    /// The section a block is in — three floors, y included
    /// (`.horizon/sdk/api/crates/horizon-core/src/position.rs:41-46`).
    pub const fn of(pos: [i32; 3]) -> SectionPos {
        SectionPos {
            x: pos[0] >> 4,
            y: pos[1] >> 4,
            z: pos[2] >> 4,
        }
    }

    /// Vanilla's `SectionPos.asLong`: 22 bits of x, 22 of z, 20 of y
    /// (`.horizon/sdk/api/crates/horizon-core/src/position.rs:74-78`).
    pub const fn pack(self) -> i64 {
        (((self.x as u64 & 0x3f_ffff) << 42)
            | ((self.z as u64 & 0x3f_ffff) << 20)
            | (self.y as u64 & 0xf_ffff)) as i64
    }

    pub const fn unpack(value: i64) -> SectionPos {
        SectionPos::new(
            (value >> 42) as i32,
            (value << 44 >> 44) as i32,
            (value << 22 >> 42) as i32,
        )
    }
}

/// A square of chunks, inclusive on both ends: the shape a colony's claim makes.
///
/// Built by [`ChunkSquare::around`], which is the loop Java writes twice — once
/// to claim (`core/util/ChunkDataHelper.java:339-345`) and once to ask whether a
/// claim is possible (`:185-207`). Java never names the square; the port does,
/// because "inside the square" and "inside the circle" are two different answers
/// and both are asked (see [`super::territory`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct ChunkSquare {
    pub min: ChunkPos,
    pub max: ChunkPos,
}

impl ChunkSquare {
    /// `2 * radius + 1` chunks a side, centred on `center`.
    ///
    /// A radius of `0` is the one chunk under the town hall; a radius of `4` —
    /// Java's default `initialColonySize` — is nine by nine.
    pub const fn around(center: ChunkPos, radius: i32) -> ChunkSquare {
        ChunkSquare {
            min: ChunkPos::new(center.x - radius, center.z - radius),
            max: ChunkPos::new(center.x + radius, center.z + radius),
        }
    }

    /// Is this chunk in the square? Inclusive, like both of Java's loops.
    pub const fn contains(&self, chunk: ChunkPos) -> bool {
        chunk.x >= self.min.x
            && chunk.x <= self.max.x
            && chunk.z >= self.min.z
            && chunk.z <= self.max.z
    }

    /// Is the *chunk of* this block in the square? What
    /// `Colony.isCoordInColony` asks the world (`core/colony/Colony.java:1305-1307`).
    pub const fn contains_block(&self, pos: [i32; 3]) -> bool {
        self.contains(ChunkPos::of(pos))
    }

    /// Chunks a side: `2 * radius + 1`.
    pub const fn side(&self) -> i32 {
        self.max.x - self.min.x + 1
    }

    /// How many chunks the square holds, `side²`. `(2r+1)²`, never the `πr²` a
    /// circle would give — the corner chunks are claimed too.
    pub const fn count(&self) -> i32 {
        self.side() * self.side()
    }

    /// The first block of the square, as small as origin: `(min.x, 0, min.z) * 16`.
    pub const fn min_block(&self) -> [i32; 3] {
        [self.min.x * BLOCKS_PER_CHUNK, 0, self.min.z * BLOCKS_PER_CHUNK]
    }

    /// The last block of the square: the **end** of the last chunk, so it is
    /// `(max.x, max.z) * 16 + 15` — the corner block, not the corner chunk's
    /// origin.
    pub const fn max_block(&self) -> [i32; 3] {
        [
            self.max.x * BLOCKS_PER_CHUNK + BLOCKS_PER_CHUNK - 1,
            0,
            self.max.z * BLOCKS_PER_CHUNK + BLOCKS_PER_CHUNK - 1,
        ]
    }
}

/// The chunk a block is in: a floor shift, `-1` in chunk `-1`.
pub const fn chunk_of(pos: [i32; 3]) -> ChunkPos {
    ChunkPos::of(pos)
}

/// The section a block is in: three floor shifts, y included.
pub const fn section_of(pos: [i32; 3]) -> SectionPos {
    SectionPos::of(pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_finds_its_chunk_and_negative_coordinates_floor() {
        // The whole reason this file exists: `-1 / 16` is `0`, `-1 >> 4` is `-1`.
        assert_eq!(chunk_of([0, 64, 0]), ChunkPos::new(0, 0));
        assert_eq!(chunk_of([15, 64, 15]), ChunkPos::new(0, 0));
        assert_eq!(chunk_of([16, 64, 16]), ChunkPos::new(1, 1));
        assert_eq!(chunk_of([-1, 64, -1]), ChunkPos::new(-1, -1));
        assert_eq!(chunk_of([-16, 64, -16]), ChunkPos::new(-1, -1));
        assert_eq!(chunk_of([-17, 64, -17]), ChunkPos::new(-2, -2));
        // A short division would put [-1, -1] in chunk 0 and lose it.
        assert_ne!(chunk_of([-1, 64, -1]).x, -1 / BLOCKS_PER_CHUNK);
    }

    #[test]
    fn the_vertical_axis_never_enters_a_chunk() {
        assert_eq!(chunk_of([5, -300, 5]), chunk_of([5, 400, 5]));
        assert_eq!(chunk_of([5, 0, 5]), ChunkPos::new(0, 0));
    }

    #[test]
    fn a_chunk_origin_is_its_first_block_even_when_negative() {
        assert_eq!(ChunkPos::new(0, 0).origin(), [0, 0, 0]);
        assert_eq!(ChunkPos::new(3, -1).origin(), [48, 0, -16]);
        // Origin(block) is the block's own chunk origin, and re-reading the
        // origin must stay in the chunk it came from.
        for x in [-33, -16, -1, 0, 15, 16, 1_000] {
            let chunk = chunk_of([x, 70, x]);
            assert_eq!(chunk_of(chunk.origin()).x, chunk.x, "x = {x}");
        }
    }

    #[test]
    fn a_chunk_packs_and_unpacks_including_negative_ones() {
        for (x, z) in [(0, 0), (1, 0), (-1, 0), (0, -1), (1_234, -5_678)] {
            let chunk = ChunkPos::new(x, z);
            assert_eq!(ChunkPos::unpack(chunk.pack()), chunk, "({x}, {z})");
        }
        // The two halves do not collide: same x, different z.
        assert_ne!(ChunkPos::new(7, 0).pack(), ChunkPos::new(0, 7).pack());
    }

    #[test]
    fn a_section_floors_the_same_way_on_y() {
        assert_eq!(section_of([0, 0, 0]), SectionPos::new(0, 0, 0));
        assert_eq!(section_of([15, 63, 15]), SectionPos::new(0, 3, 0));
        assert_eq!(section_of([-1, -1, -1]), SectionPos::new(-1, -1, -1));
        assert_eq!(section_of([-16, -16, -16]), SectionPos::new(-1, -1, -1));
        assert_eq!(section_of([-17, 70, 17]), SectionPos::new(-2, 4, 1));
    }

    #[test]
    fn a_section_packs_and_unpacks_with_its_vertical_part() {
        for pos in [(0, 0, 0), (1, -1, 3), (-5, 20, 7), (1_000, -64, -1_000)] {
            let section = section_of([pos.0, pos.1, pos.2]);
            assert_eq!(SectionPos::unpack(section.pack()), section, "{pos:?}");
        }
    }

    #[test]
    fn the_chunk_distance_is_the_squared_two_axis_one() {
        // `BlockPosUtil.java:1237-1242`: (3, 4) is 25, not 5 and not 7.
        assert_eq!(ChunkPos::new(0, 0).distance_squared(ChunkPos::new(3, 4)), 25);
        assert_eq!(ChunkPos::new(3, 4).distance_squared(ChunkPos::new(0, 0)), 25);
        assert_eq!(ChunkPos::new(0, 0).distance_squared(ChunkPos::new(0, 0)), 0);
        assert_eq!(ChunkPos::new(-1, -1).distance_squared(ChunkPos::new(1, 1)), 8);
    }

    #[test]
    fn a_square_of_radius_four_holds_nine_by_nine_chunks() {
        let square = ChunkSquare::around(ChunkPos::new(10, -4), 4);
        assert_eq!(square.side(), 9);
        assert_eq!(square.count(), 81);
        assert_eq!(square.min, ChunkPos::new(6, -8));
        assert_eq!(square.max, ChunkPos::new(14, 0));
        // The four corners are in; one chunk past any of them is out.
        for corner in [
            ChunkPos::new(6, -8),
            ChunkPos::new(14, -8),
            ChunkPos::new(6, 0),
            ChunkPos::new(14, 0),
        ] {
            assert!(square.contains(corner), "{corner:?}");
        }
        assert!(!square.contains(ChunkPos::new(5, -4)));
        assert!(!square.contains(ChunkPos::new(15, -4)));
        assert!(!square.contains(ChunkPos::new(10, 1)));
    }

    #[test]
    fn a_square_of_radius_zero_is_the_one_chunk_under_the_town_hall() {
        let square = ChunkSquare::around(ChunkPos::new(-2, 3), 0);
        assert_eq!(square.side(), 1);
        assert_eq!(square.count(), 1);
        assert!(square.contains(ChunkPos::new(-2, 3)));
        assert!(!square.contains(ChunkPos::new(-1, 3)));
    }

    #[test]
    fn membership_by_block_and_by_chunk_agree_at_the_block_corner() {
        let square = ChunkSquare::around(ChunkPos::new(0, 0), 1);
        // The very first and very last block of the 3x3 square.
        assert_eq!(square.min_block(), [-16, 0, -16]);
        assert_eq!(square.max_block(), [31, 0, 31]);
        assert!(square.contains_block([-16, 70, -16]));
        assert!(square.contains_block([31, 70, 31]));
        // One block further is the next chunk, and out.
        assert!(!square.contains_block([-17, 70, 0]));
        assert!(!square.contains_block([32, 70, 0]));
        // y is irrelevant to a claim.
        assert!(square.contains_block([0, -1_000, 0]));
    }
}
