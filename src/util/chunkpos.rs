//! `MutableChunkPos`: a chunk coordinate that can be moved, and the three
//! divisions worth checking.
//!
//! Java reference: `core/util/MutableChunkPos.java` (162 lines). It exists
//! because `ChunkPos` is immutable and Java's chunk scans walk a cursor:
//! `getMinBlockX`, `getBlockX(offset)`, `getRegionX`, `getRegionLocalX`,
//! `getChessboardDistance`, `setX`, `setZ`, `from`.
//!
//! **Nothing here is a second `ChunkPos`.** [`crate::base::coords::ChunkPos`]
//! already holds the pair, the pack, the origin and the squared distance, and it
//! is what `claims` keys a `BTreeMap` on. [`MutableChunk`] wraps it and adds what
//! only a cursor needs; [`MutableChunk::to_chunk`] hands it back. When the answer
//! is already on `ChunkPos` — [`MutableChunk::origin`], [`MutableChunk::pack`] —
//! this file calls it instead of repeating it.
//!
//! # The three divisions
//!
//! | Question | Java | Answer for `x = -1` |
//! | --- | --- | --- |
//! | which block starts this chunk? | `SectionPos.sectionToBlockCoord(x)` | `-16` |
//! | which region holds it? | `x >> 5` | `-1` |
//! | where in that region? | `x & 31` | `31` |
//!
//! All three are **floors**, and all three are wrong if written with `/` and `%`:
//! `-1 / 16` is `0`, `-1 >> 4` is `-1`, and `-1 & 31` is `31` where `-1 % 32` is
//! `-1`. Java and Rust agree on `>>`, `&` and `%` for `i32` (all three are
//! two's-complement operations in both languages), so the port is a transcription
//! and not a translation — which is exactly the case where a transcription is
//! the right answer.
//!
//! # `ChunkDataHelper` is not here
//!
//! `core/util/ChunkDataHelper.java` (476 lines) is the other chunk file of this
//! mission and it is **already ported**, in three pieces of
//! [`crate::base`]: the conversions and the square in `base/coords.rs`
//! (from `:339-345` and `:248`), the claim sweep in `base/territory.rs`
//! (from `:135-139`), and who owns a chunk in `base/claims.rs`
//! (from `api/colony/IColonyTagCapability.java`). Writing it again here would be
//! a second answer to the same question; the report says so, and `MANQUANT.md`
//! §4 lists the two `ChunkDataHelper` functions that have *no* port yet
//! (`loadChunk`'s capability migration and `unloadChunk`'s loaded-chunk book,
//! both of which need a chunk capability a wasm mod does not have).

use crate::base::coords::ChunkPos;

/// A chunk coordinate that can be reassigned — `MutableChunkPos`
/// (`MutableChunkPos.java:7-162`).
///
/// Ordered on `(x, z)` via the conversion to [`ChunkPos`], which is what makes a
/// `BTreeSet<MutableChunk>` sweep deterministic in the port where Java's cursor is
/// just iterated.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
// exhaustive: this port's own cursor, not a `mod_api` type
pub struct MutableChunk {
    x: i32,
    z: i32,
}

impl MutableChunk {
    /// `MutableChunkPos(int, int)` (`:12-17`).
    pub const fn new(x: i32, z: i32) -> MutableChunk {
        MutableChunk { x, z }
    }

    /// `MutableChunkPos(BlockPos)` (`:19-24`): the block's chunk, by floor
    /// division. `pos[0] >> 4` and **not** `pos[0] / 16`: a block at `x = -1` is
    /// in chunk `-1`.
    pub const fn of(pos: [i32; 3]) -> MutableChunk {
        MutableChunk {
            x: pos[0] >> 4,
            z: pos[2] >> 4,
        }
    }

    /// `MutableChunkPos(long)` (`:26-31`): the low half is `x`, the high half
    /// is `z` — vanilla's `ChunkPos.asLong`, which
    /// [`ChunkPos::unpack`](crate::base::coords::ChunkPos::unpack) already does.
    pub const fn unpack(packed: i64) -> MutableChunk {
        let chunk = ChunkPos::unpack(packed);
        MutableChunk {
            x: chunk.x,
            z: chunk.z,
        }
    }

    /// `from(ChunkPos)` (`:152-156`) and the immutable copy Java calls
    /// `toImmutable` (`:158-161`).
    pub const fn to_chunk(self) -> ChunkPos {
        ChunkPos::new(self.x, self.z)
    }

    /// The same cursor, moved. Java spells it `setX` then `setZ`
    /// (`:137-150`); a value in, a value out, because a caller that holds the
    /// cursor across a scan is the only one that ever *wanted* mutation.
    pub const fn moved_to(self, x: i32, z: i32) -> MutableChunk {
        MutableChunk { x, z }
    }

    pub const fn x(self) -> i32 {
        self.x
    }

    pub const fn z(self) -> i32 {
        self.z
    }

    /// `toLong()` (`:33-37`): `ChunkPos.asLong(x, z)` of the **current** pair,
    /// which is what the mutable subclass is for — the inherited method would
    /// pack the values it was constructed with.
    pub const fn pack(self) -> i64 {
        self.to_chunk().pack()
    }

    /// `getMinBlockX` / `getMinBlockZ` (`:68-77`):
    /// `SectionPos.sectionToBlockCoord(x)`, which is `x << 4`.
    pub const fn min_block(self) -> [i32; 2] {
        [self.x << 4, self.z << 4]
    }

    /// The same, through [`ChunkPos::origin`](crate::base::coords::ChunkPos::origin),
    /// which is the version the rest of the port already uses.
    pub const fn origin(self) -> [i32; 3] {
        self.to_chunk().origin()
    }

    /// `getBlockX(offset)` / `getBlockZ(offset)` (`:79-89`):
    /// `SectionPos.sectionToBlockCoord(x, offset)`, which is `(x << 4) + offset`.
    ///
    /// An offset outside `0..16` is not refused — vanilla's version is a plain
    /// addition and a caller that scans a chunk and one block past it depends on
    /// that.
    pub const fn block_x(self, offset: i32) -> i32 {
        (self.x << 4) + offset
    }

    pub const fn block_z(self, offset: i32) -> i32 {
        (self.z << 4) + offset
    }

    /// `getRegionX` / `getRegionZ` (`:91-101`): `x >> 5`. A region is 32 chunks
    /// square, so this is a floor division by 32 — negative for a chunk west or
    /// north of the origin.
    pub const fn region(self) -> [i32; 2] {
        [self.x >> 5, self.z >> 5]
    }

    /// `getRegionLocalX` / `getRegionLocalZ` (`:103-113`): `x & 31`, which is the
    /// **non-negative** remainder in `0..=31` even for a negative chunk.
    pub const fn region_local(self) -> [i32; 2] {
        [self.x & 31, self.z & 31]
    }

    /// `getChessboardDistance(ChunkPos)` (`:121-125`): the larger of the two
    /// coordinate differences — the number of chunks a square sweep of that
    /// radius covers, which is what a colony's chunk square is measured in.
    ///
    /// The difference is taken in `i32` and negated when negative (`abs` by hand,
    /// so the function stays `const`). A chunk coordinate is bounded by vanilla's
    /// ±1.9 M, so the negation is nowhere near `i32::MIN`.
    pub const fn chessboard(self, other: ChunkPos) -> i32 {
        let dx = self.x - other.x;
        let dz = self.z - other.z;
        let dx = if dx < 0 { -dx } else { dx };
        let dz = if dz < 0 { -dz } else { dz };
        if dx > dz {
            dx
        } else {
            dz
        }
    }

    /// `hashCode()` (`:39-45`), transcribed:
    ///
    /// ```java
    /// final int i = 1664525 * this.mutableX + 1013904223;
    /// final int j = 1664525 * (this.mutableZ ^ -559038737) + 1013904223;
    /// return i ^ j;
    /// ```
    ///
    /// `1664525` and `1013904223` are the Numerical Recipes LCG constants.
    ///
    /// Kept because a save that stores chunk keys in a `HashMap` and a port that
    /// stores them in a `BTreeMap` still have to answer the same question about
    /// **equality**, and Java's `equals` here is unusual enough to be worth
    /// pinning: a `MutableChunkPos` equals a plain `ChunkPos` with the same
    /// coordinates (`:58-61`), which is why the port's `PartialEq` is derived
    /// on the pair and not on the type name.
    ///
    /// Every operation wraps as an `i32`, like Java's `int` arithmetic.
    pub const fn java_hash(self) -> i32 {
        let i = 1664525i32
            .wrapping_mul(self.x)
            .wrapping_add(1013904223);
        let j = 1664525i32
            .wrapping_mul(self.z ^ -559038737)
            .wrapping_add(1013904223);
        i ^ j
    }
}

impl From<ChunkPos> for MutableChunk {
    fn from(chunk: ChunkPos) -> Self {
        MutableChunk {
            x: chunk.x,
            z: chunk.z,
        }
    }
}

impl From<MutableChunk> for ChunkPos {
    fn from(cursor: MutableChunk) -> Self {
        cursor.to_chunk()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A block at `x = -1` is in chunk `-1`; `/ 16` would answer `0`.
    #[test]
    fn the_block_to_chunk_conversion_is_a_floor() {
        assert_eq!(MutableChunk::of([0, 64, 0]), MutableChunk::new(0, 0));
        assert_eq!(MutableChunk::of([15, 64, 15]), MutableChunk::new(0, 0));
        assert_eq!(MutableChunk::of([16, 64, 16]), MutableChunk::new(1, 1));
        assert_eq!(MutableChunk::of([-1, 64, -1]), MutableChunk::new(-1, -1));
        assert_eq!(MutableChunk::of([-16, 64, -16]), MutableChunk::new(-1, -1));
        assert_eq!(MutableChunk::of([-17, 64, -17]), MutableChunk::new(-2, -2));
    }

    /// The mutable cursor packs **its current** pair, which is the whole reason
    /// the Java class overrides `toLong` (`:33-37`).
    #[test]
    fn the_cursor_packs_where_it_now_is() {
        let moved = MutableChunk::new(0, 0).moved_to(3, 7);
        assert_eq!(moved.pack(), ChunkPos::new(3, 7).pack());
        assert_ne!(moved.pack(), MutableChunk::new(0, 0).pack());
    }

    /// Round-tripping through the pack loses nothing, `z` sign included.
    #[test]
    fn the_pack_round_trips_through_the_cursor() {
        for (x, z) in [(0, 0), (1, -1), (-1, 1), (i32::MAX, i32::MIN), (-30_000_000 >> 4, 7)] {
            let cursor = MutableChunk::new(x, z);
            assert_eq!(MutableChunk::unpack(cursor.pack()), cursor);
        }
    }

    /// A chunk's first block is a floor shift, negative chunks included.
    #[test]
    fn the_first_block_of_a_chunk_is_a_shift() {
        assert_eq!(MutableChunk::new(0, 0).min_block(), [0, 0]);
        assert_eq!(MutableChunk::new(1, 2).min_block(), [16, 32]);
        assert_eq!(MutableChunk::new(-1, -2).min_block(), [-16, -32]);
    }

    /// `origin()` is the three-coordinate form `base::coords` already answers,
    /// and it must agree with `min_block()`.
    #[test]
    fn the_origin_agrees_with_the_shift() {
        for (x, z) in [(0, 0), (5, -5), (-1, 1)] {
            let cursor = MutableChunk::new(x, z);
            let [min_x, min_z] = cursor.min_block();
            assert_eq!(cursor.origin(), [min_x, 0, min_z]);
            assert_eq!(cursor.origin(), cursor.to_chunk().origin());
        }
    }

    /// An offset is a plain addition, and it may leave the chunk.
    #[test]
    fn a_block_offset_may_leave_the_chunk() {
        let cursor = MutableChunk::new(-1, 2);
        assert_eq!(cursor.block_x(0), -16);
        assert_eq!(cursor.block_x(15), -1);
        assert_eq!(cursor.block_x(16), 0);
        assert_eq!(cursor.block_z(0), 32);
    }

    /// A region is 32 chunks square, and the division is a floor.
    #[test]
    fn the_region_is_a_floor_division_by_thirty_two() {
        assert_eq!(MutableChunk::new(0, 0).region(), [0, 0]);
        assert_eq!(MutableChunk::new(31, 31).region(), [0, 0]);
        assert_eq!(MutableChunk::new(32, 32).region(), [1, 1]);
        assert_eq!(MutableChunk::new(-1, -1).region(), [-1, -1]);
        assert_eq!(MutableChunk::new(-32, -32).region(), [-1, -1]);
        assert_eq!(MutableChunk::new(-33, -33).region(), [-2, -2]);
    }

    /// `x & 31` is a **non-negative** remainder, which `x % 32` is not.
    #[test]
    fn the_region_local_index_is_never_negative() {
        assert_eq!(MutableChunk::new(0, 0).region_local(), [0, 0]);
        assert_eq!(MutableChunk::new(31, 31).region_local(), [31, 31]);
        assert_eq!(MutableChunk::new(32, 32).region_local(), [0, 0]);
        assert_eq!(MutableChunk::new(-1, -1).region_local(), [31, 31]);
        assert_eq!(MutableChunk::new(-32, -32).region_local(), [0, 0]);
        assert_eq!(MutableChunk::new(-33, -33).region_local(), [31, 31]);
    }

    /// The chessboard distance is the larger axis, not the sum and not the
    /// Euclidean length.
    #[test]
    fn the_chessboard_distance_is_the_larger_axis() {
        let origin = ChunkPos::new(0, 0);
        assert_eq!(MutableChunk::new(0, 0).chessboard(origin), 0);
        assert_eq!(MutableChunk::new(3, 4).chessboard(origin), 4);
        assert_eq!(MutableChunk::new(-4, 3).chessboard(origin), 4);
        assert_eq!(MutableChunk::new(-4, -4).chessboard(origin), 4);
    }

    /// The chessboard distance is symmetric and matches the square a colony
    /// sweeps: radius `r` is `2r + 1` chunks a side, so the corner is at `r`.
    #[test]
    fn the_chessboard_distance_matches_the_corner_of_a_claim_square() {
        let centre = ChunkPos::new(10, -10);
        let square = crate::base::coords::ChunkSquare::around(centre, 4);
        for x in square.min.x..=square.max.x {
            for z in square.min.z..=square.max.z {
                assert!(MutableChunk::new(x, z).chessboard(centre) <= 4);
            }
        }
        assert_eq!(MutableChunk::new(square.max.x, square.max.z).chessboard(centre), 4);
    }

    /// Java's hash, pinned against an independent computation of the same
    /// arithmetic. A change here would be a change of the save format's key.
    #[test]
    fn the_java_hash_is_the_lcg_of_the_two_coordinates() {
        assert_eq!(MutableChunk::new(0, 0).java_hash(), 1455762653);
        assert_eq!(MutableChunk::new(1, 0).java_hash(), 1445168878);
        assert_eq!(MutableChunk::new(0, 1).java_hash(), 1459391018);
        assert_eq!(MutableChunk::new(3, 7).java_hash(), 1430335649);
        assert_eq!(MutableChunk::new(-1, -1).java_hash(), 843918461);
    }

    /// Two cursors on the same chunk are equal **and** hash alike, and a
    /// `ChunkPos` converts both ways without a loss.
    #[test]
    fn a_cursor_and_a_chunk_pos_of_the_same_cell_are_interchangeable() {
        let cursor = MutableChunk::new(-3, 12);
        let chunk = ChunkPos::new(-3, 12);
        assert_eq!(cursor.to_chunk(), chunk);
        assert_eq!(MutableChunk::from(chunk), cursor);
        assert_eq!(cursor, MutableChunk::from(chunk));
        assert_eq!(cursor.java_hash(), MutableChunk::from(chunk).java_hash());
        assert_ne!(MutableChunk::new(1, 2), MutableChunk::new(2, 1));
    }
}
