//! Where a colony's land is, and where a new one may be founded.
//!
//! Java has **two** answers to "is this place part of that colony", and they do
//! not agree at the corners. Both are here, each named for the question it
//! answers; the disagreement is the point of this file.
//!
//! **1. The claim square.** At founding, a colony sweeps every chunk within
//! `initialColonySize` of the town hall's chunk (`claimColonyChunks`,
//! `core/util/ChunkDataHelper.java:135-139`, whose loop is `:339-345`), then grows
//! by building (`:152-171`). "In the colony" is therefore "whose chunk owns me":
//! `Colony.isCoordInColony` asks the chunk, never a radius
//! (`core/colony/Colony.java:1298-1308`). [`Territory::contains`] is that question
//! with the square supplied instead of read from the world.
//!
//! **2. The circle.** A colony also has a *block* radius, and Java uses it twice:
//! to tell a player how many blocks to the border —
//! `(int) getDistance(center, pos) - (initialColonySize << 4)`
//! (`core/network/messages/server/GetColonyInfoMessage.java:106`) — and to refuse
//! a founding that is too close to an existing town hall
//! (`core/colony/ColonyManager.java:311-320`). [`Territory::border_distance`] is
//! the first, [`founding_allowed`] the second.
//!
//! **The two disagree by design, and here is by how much.** With the default
//! radius of 4 chunks the square is 144 blocks a side, so its corner block is
//! `sqrt(71² + 71²) ≈ 100.4` blocks from the centre while the "radius" is 64: the
//! corner is **claimed** and simultaneously **36 blocks outside the border**
//! [`Territory::border_distance`] reports. Java never asks both questions about
//! the same block, so it never notices; the port keeps both so a caller that
//! needs one cannot quietly get the other.
//!
//! **A third shape: `maxColonySize`.** Dynamic claiming refuses a chunk whose
//! chunk-distance from the town hall squared exceeds `maxColonySize²`
//! (`core/util/ChunkDataHelper.java:248`) — a *square* again, in chunk units, and
//! a **0 means no limit at all**. [`Territory::within_max_size`] is that test.

use super::constants::{
    BLOCKS_PER_CHUNK, INITIAL_COLONY_SIZE, INITIAL_COLONY_SIZE_MAX, INITIAL_COLONY_SIZE_MIN,
    MAX_DISTANCE_FROM_WORLD_SPAWN, MIN_COLONY_DISTANCE, MIN_DISTANCE_FROM_WORLD_SPAWN,
};
use super::coords::{chunk_of, ChunkPos, ChunkSquare};
use super::geom::{dist, dist_sqr_2d};

/// The land a colony holds: the town hall's block, and half the side of its
/// original claim in chunks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Territory {
    /// The town hall's position — a colony's centre, `IColony.getCenter()`
    /// (`api/colony/IColony.java:55`).
    pub center: [i32; 3],
    radius: u32,
}

impl Territory {
    /// A territory of `radius_chunks` around `center`, **clamped to what the
    /// config accepts** (`1..=15`, `api/configuration/ServerConfiguration.java:166`).
    ///
    /// Java clamps in the config layer, so a value outside the bounds cannot
    /// reach the claim code; a port with no config file has to clamp here or a
    /// command argument would decide how much land a colony takes.
    pub fn new(center: [i32; 3], radius_chunks: u32) -> Territory {
        Territory {
            center,
            radius: radius_chunks.clamp(INITIAL_COLONY_SIZE_MIN, INITIAL_COLONY_SIZE_MAX),
        }
    }

    /// A territory with Java's default radius, `initialColonySize = 4`
    /// (`ServerConfiguration.java:166`): nine by nine chunks.
    pub fn with_default_radius(center: [i32; 3]) -> Territory {
        Territory::new(center, INITIAL_COLONY_SIZE)
    }

    /// The half-width in chunks the claim uses.
    pub fn radius(&self) -> u32 {
        self.radius
    }

    /// The chunk the town hall stands in.
    pub fn center_chunk(&self) -> ChunkPos {
        chunk_of(self.center)
    }

    /// The square of chunks this colony claims — what
    /// `staticClaimInRange` sweeps (`core/util/ChunkDataHelper.java:339-345`).
    pub fn claim_square(&self) -> ChunkSquare {
        ChunkSquare::around(self.center_chunk(), self.radius as i32)
    }

    /// The square an **unclaim** sweeps, which Java makes twice as wide:
    /// `staticClaimInRange(id, add, center, add ? range : range * 2, …)`
    /// (`core/util/ChunkDataHelper.java:138`).
    ///
    /// Not a typo and not a rounding: when a colony is removed, the sweep must
    /// also reach the chunks a *bigger* claim once covered, and the claim records
    /// of those chunks cannot be trusted to still exist.
    pub fn unclaim_square(&self) -> ChunkSquare {
        ChunkSquare::around(self.center_chunk(), (self.radius * 2) as i32)
    }

    /// Is this block on the colony's land? The chunk of the block is in the claim
    /// square, which is how `Colony.isCoordInColony` decides
    /// (`core/colony/Colony.java:1305-1307`, via the chunk's owner).
    ///
    /// **This is not a distance test.** The corner of the square is in and a block
    /// just outside the middle of an edge is out, whatever their distances from
    /// the town hall are.
    pub fn contains(&self, pos: [i32; 3]) -> bool {
        self.claim_square().contains_block(pos)
    }

    /// The squared x/z distance to the town hall, which is what
    /// `IColony.getDistanceSquared` returns (`api/colony/IColony.java:88`,
    /// implemented at `core/colony/Colony.java:1311-1315`). **Two-dimensional**:
    /// a colony on a mountain has the same distance as one in the valley.
    pub fn distance_squared_to_center(&self, pos: [i32; 3]) -> i64 {
        dist_sqr_2d(self.center, pos)
    }

    /// Blocks from `pos` to the edge of the colony's circle, as Java reports it:
    /// `(int) (getDistance(center, pos) - (initialColonySize << 4))`
    /// (`core/network/messages/server/GetColonyInfoMessage.java:106`).
    ///
    /// Negative inside, positive outside. Two details are Java's and are kept:
    ///
    /// - **the distance is three-dimensional** (`getDistance`, `:453-461`), unlike
    ///   [`Self::distance_squared_to_center`];
    /// - **the cast truncates toward zero**, so a block 0.9 inside reads `0`, not
    ///   `-1`. A player reading "0 blocks to the border" is standing on land.
    pub fn border_distance(&self, pos: [i32; 3]) -> i32 {
        let radius_in_blocks = (self.radius as i32) * BLOCKS_PER_CHUNK;
        (dist(self.center, pos) - f64::from(radius_in_blocks)) as i32
    }

    /// May this chunk ever be claimed by **this** colony? Java refuses it when
    /// `chunkDistanceSquared(centerChunk, chunk) > maxColonySize²`
    /// (`core/util/ChunkDataHelper.java:248`), and **`max_colony_size == 0` means
    /// no ceiling at all** — the check is skipped, not failed (`:248`,
    /// `!force && maxColonySize != 0 && …`).
    ///
    /// The comparison is strict: at exactly `max²` the chunk is still claimable.
    pub fn within_max_size(&self, chunk: ChunkPos, max_colony_size: u32) -> bool {
        if max_colony_size == 0 {
            return true;
        }
        let limit = (max_colony_size * max_colony_size) as i32;
        self.center_chunk().distance_squared(chunk) <= limit
    }
}

/// The four numbers a founding checks, all from the server config
/// (`api/configuration/ServerConfiguration.java:164-168`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct FoundingLimits {
    pub initial_colony_size: u32,
    pub min_colony_distance: u32,
    pub min_distance_from_world_spawn: u32,
    pub max_distance_from_world_spawn: u32,
}

impl FoundingLimits {
    /// Java's defaults, which are what a server has before anyone edits it.
    pub const DEFAULT: FoundingLimits = FoundingLimits {
        initial_colony_size: INITIAL_COLONY_SIZE,
        min_colony_distance: MIN_COLONY_DISTANCE,
        min_distance_from_world_spawn: MIN_DISTANCE_FROM_WORLD_SPAWN,
        max_distance_from_world_spawn: MAX_DISTANCE_FROM_WORLD_SPAWN,
    };

    /// The blocks a new town hall must keep from every existing one:
    /// `max(minColonyDistance, initialColonySize) << 4`
    /// (`core/colony/ColonyManager.java:313`). With the defaults, `max(8, 4) = 8`
    /// chunks, so 128 blocks.
    pub fn colony_spacing(&self) -> u32 {
        self.min_colony_distance.max(self.initial_colony_size) * BLOCKS_PER_CHUNK as u32
    }
}

/// Why a town hall may not be placed here. Each variant is one branch of
/// `GetColonyInfoMessage.onExecute` (`core/network/messages/server/GetColonyInfoMessage.java:93-104`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum FoundingRefusal {
    /// Too close to an existing colony, which is the one the player is shown.
    /// Java's message names it (`:120`).
    TooCloseToColony { center: [i32; 3] },
    /// Inside `minDistanceFromWorldSpawn` of the world spawn (`:96-99`).
    TooCloseToSpawn,
    /// Past `maxDistanceFromWorldSpawn` (`:100-102`).
    TooFarFromSpawn,
}

/// May a colony be founded at `pos`? Java's order, whole, and its three
/// oddities.
///
/// 1. **Too close to another colony** — checked first (`ColonyManager.java:311-320`,
///    called at `GetColonyInfoMessage.java:93`). The *nearest* colony is chosen by
///    its **2D** squared distance (`getClosestColony`, `ColonyManager.java:497`,
///    which calls `IColony.getDistanceSquared`), then the refusal compares that
///    one colony's **3D** distance against the spacing (`:316`). Two metrics in
///    one test; see the module docs of [`super::geom`].
/// 2. **Too close to the world spawn**, **2D** this time
///    (`sqrt(getDistanceSquared2D(pos, spawn))`, `:95`).
/// 3. **Too far from the world spawn**, the same 2D number (`:100`).
///
/// Both spawn bounds are compared on the `f64` before any cast, so 29 999.5
/// against a limit of 30 000 is *allowed*; only the warning's text truncates.
///
/// `centres` is every colony's town hall; Java reads them from the world, the port
/// is given them. Cheap for a command, and it keeps this file host-free.
pub fn founding_allowed(
    limits: FoundingLimits,
    centres: &[[i32; 3]],
    pos: [i32; 3],
    world_spawn: [i32; 3],
) -> Result<(), FoundingRefusal> {
    let spacing = f64::from(limits.colony_spacing());

    // 1. The nearest colony by 2D squared distance — `getClosestColony`
    //    (`ColonyManager.java:490-500`).
    let nearest = centres
        .iter()
        .min_by_key(|center| dist_sqr_2d(**center, pos))
        .copied();

    if let Some(center) = nearest {
        // …then compared in 3D (`ColonyManager.java:316`).
        if dist(center, pos) < spacing {
            return Err(FoundingRefusal::TooCloseToColony { center });
        }
    }

    // 2 and 3. The spawn bounds, 2D (`GetColonyInfoMessage.java:95`).
    let spawn_distance = dist_2d_between(pos, world_spawn);
    if spawn_distance < f64::from(limits.min_distance_from_world_spawn) {
        return Err(FoundingRefusal::TooCloseToSpawn);
    }
    if spawn_distance > f64::from(limits.max_distance_from_world_spawn) {
        return Err(FoundingRefusal::TooFarFromSpawn);
    }

    Ok(())
}

/// `sqrt(getDistanceSquared2D(pos, spawn))` — Java's own spelling
/// (`GetColonyInfoMessage.java:95`), kept apart from [`super::geom::dist_2d`] only
/// to show that the two agree.
fn dist_2d_between(a: [i32; 3], b: [i32; 3]) -> f64 {
    (dist_sqr_2d(a, b) as f64).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    const HALL: [i32; 3] = [8, 70, 8]; // chunk (0, 0)

    #[test]
    fn a_default_colony_claims_a_hundred_and_forty_four_block_square() {
        let land = Territory::with_default_radius(HALL);
        assert_eq!(land.radius(), 4);
        assert_eq!(land.center_chunk(), ChunkPos::new(0, 0));
        let square = land.claim_square();
        assert_eq!(square.count(), 81);
        assert_eq!(square.min_block(), [-64, 0, -64]);
        assert_eq!(square.max_block(), [79, 0, 79]);
        // 144 blocks a side, so the circle's radius is 64.
        assert_eq!(square.max_block()[0] - square.min_block()[0] + 1, 144);
        assert_eq!(land.border_distance(HALL), -64);
    }

    #[test]
    fn membership_is_the_square_and_the_corner_is_claimed_while_outside_the_circle() {
        let land = Territory::with_default_radius(HALL);
        let corner = [79, 70, 79];
        // Claimed: its chunk (4, 4) is inside the square.
        assert!(land.contains(corner));
        // And 36 blocks outside the border Java reports.
        assert_eq!(land.border_distance(corner), 36);
        // The two answers disagree, and that is Java's design, not a bug here.
        assert!(!land.contains([80, 70, 8]));
        assert!(land.border_distance([80, 70, 8]) > 0);
    }

    #[test]
    fn the_border_distance_is_negative_inside_and_positive_outside() {
        let land = Territory::with_default_radius(HALL);
        assert_eq!(land.border_distance([8, 70, 8 + 10]), -54);
        assert_eq!(land.border_distance([8, 70, 8 - 10]), -54);
        assert_eq!(land.border_distance([8 + 64, 70, 8]), 0);
        assert_eq!(land.border_distance([8 + 74, 70, 8]), 10);
        // Measured in 3D: a block above the town hall is farther than the flat one.
        assert_eq!(land.border_distance([8, 70 + 64, 8]), 0);
        assert_eq!(land.border_distance([8, 70 + 100, 8]), 36);
    }

    #[test]
    fn the_border_distance_truncates_toward_zero_like_a_java_cast() {
        let land = Territory::with_default_radius(HALL);
        // One block inside the square whose 3D distance is 63.07: 63.07 - 64 is
        // -0.93, and `(int)` of that is 0, not -1. A player reads "on the border".
        let just_inside = [8 + 63, 70, 8 + 3];
        assert!(land.contains(just_inside));
        assert_eq!(land.border_distance(just_inside), 0);
        // And a bit further out, 63.51 - 64 is -0.49, which is still 0.
        assert_eq!(land.border_distance([8 + 63, 70, 8 + 8]), 0);
        // Truncation, not flooring: -0.49 floors to -1, and Java says 0.
        assert!(dist(HALL, [8 + 63, 70, 8 + 8]) - 64.0 < 0.0);
    }

    #[test]
    fn a_radius_outside_what_the_config_accepts_is_clamped_not_trusted() {
        // `ServerConfiguration.java:166` declares the range 1..15.
        assert_eq!(Territory::new(HALL, 0).radius(), 1);
        assert_eq!(Territory::new(HALL, 1).radius(), 1);
        assert_eq!(Territory::new(HALL, 15).radius(), 15);
        assert_eq!(Territory::new(HALL, 99).radius(), 15);
        assert_eq!(Territory::new(HALL, 99).claim_square().count(), 31 * 31);
    }

    #[test]
    fn an_unclaim_sweeps_twice_as_wide_as_the_claim() {
        // `ChunkDataHelper.java:138`: `add ? range : range * 2`.
        let land = Territory::with_default_radius(HALL);
        assert_eq!(land.claim_square().count(), 81);
        assert_eq!(land.unclaim_square().count(), 17 * 17);
        assert!(land.unclaim_square().contains(ChunkPos::new(8, 8)));
        assert!(!land.claim_square().contains(ChunkPos::new(8, 8)));
    }

    #[test]
    fn the_max_size_refuses_only_strictly_beyond_its_square() {
        // `ChunkDataHelper.java:248`: `> maxColonySize * maxColonySize`.
        let land = Territory::with_default_radius(HALL);
        assert!(land.within_max_size(ChunkPos::new(20, 0), 20));
        assert!(land.within_max_size(ChunkPos::new(12, 16), 20)); // 400 exactly
        assert!(!land.within_max_size(ChunkPos::new(21, 0), 20));
        assert!(!land.within_max_size(ChunkPos::new(0, -21), 20));
        // Zero is "no ceiling", not "no land".
        assert!(land.within_max_size(ChunkPos::new(9_999, 9_999), 0));
    }

    #[test]
    fn the_colony_spacing_is_the_wider_of_the_two_settings() {
        assert_eq!(FoundingLimits::DEFAULT.colony_spacing(), 128); // max(8, 4) * 16
        let narrow = FoundingLimits {
            initial_colony_size: 10,
            min_colony_distance: 2,
            ..FoundingLimits::DEFAULT
        };
        assert_eq!(narrow.colony_spacing(), 160);
    }

    #[test]
    fn founding_is_refused_inside_the_spacing_and_allowed_at_it() {
        let limits = FoundingLimits::DEFAULT;
        let spawn_far = [0, 64, 0];
        let existing = [[1_000, 70, 0]];
        // 127 blocks away: refused, and the refusal names the colony.
        assert_eq!(
            founding_allowed(limits, &existing, [873, 70, 0], spawn_far),
            Err(FoundingRefusal::TooCloseToColony {
                center: [1_000, 70, 0]
            })
        );
        // Exactly 128: `dist < spacing` is strict, so it is allowed.
        assert_eq!(founding_allowed(limits, &existing, [872, 70, 0], spawn_far), Ok(()));
    }

    #[test]
    fn founding_measures_colony_distance_in_three_dimensions_and_spawn_distance_in_two() {
        let limits = FoundingLimits::DEFAULT;
        let spawn = [0, 64, 0];
        // 100 blocks away in x, 100 up: under 128 in 2D, over it in 3D.
        assert_eq!(founding_allowed(limits, &[[0, 70, 0]], [100, 170, 0], spawn), Ok(()));
        // The same hut on the flat is refused.
        assert!(founding_allowed(limits, &[[0, 70, 0]], [100, 70, 0], spawn).is_err());
    }

    #[test]
    fn the_nearest_colony_is_chosen_in_two_dimensions_then_judged_in_three() {
        // Java's two metrics, recorded rather than repaired: the colony 100
        // blocks away in 2D but 500 up is picked over the one 120 away and level,
        // and it then passes the 3D check — so founding is allowed next to a
        // level colony that is closer in the body of the world.
        let limits = FoundingLimits::DEFAULT;
        let high = [0, 570, 0];
        let level = [120, 70, 0];
        let pos = [0, 70, 0];
        let both = [high, level];
        // Picked: `high` (2D distance 0), whose 3D distance is 500 > 128.
        assert_eq!(founding_allowed(limits, &both, pos, [9_000, 64, 9_000]), Ok(()));
        // Alone, `level` would have refused it.
        assert_eq!(
            founding_allowed(limits, &[level], pos, [9_000, 64, 9_000]),
            Err(FoundingRefusal::TooCloseToColony { center: level })
        );
    }

    #[test]
    fn founding_is_refused_too_close_to_the_spawn_and_past_the_far_limit() {
        let limits = FoundingLimits::DEFAULT;
        let spawn = [0, 64, 0];
        // Default floor is 0 (`ServerConfiguration.java:168`): nothing is too close.
        assert_eq!(founding_allowed(limits, &[], spawn, spawn), Ok(()));
        // Past 30 000 blocks in 2D is refused, and y does not help.
        assert_eq!(
            founding_allowed(limits, &[], [30_001, 64, 0], spawn),
            Err(FoundingRefusal::TooFarFromSpawn)
        );
        assert_eq!(founding_allowed(limits, &[], [30_000, 64, 0], spawn), Ok(()));
        // 5 000 blocks up does not count: the spawn distance is 2D, so this is
        // still exactly 30 000 and allowed.
        assert_eq!(founding_allowed(limits, &[], [30_000, 5_000, 0], spawn), Ok(()));
        // With a floor of 1 000, 999 is too close and 1 000 is not.
        let floored = FoundingLimits {
            min_distance_from_world_spawn: 1_000,
            ..FoundingLimits::DEFAULT
        };
        assert_eq!(
            founding_allowed(floored, &[], [999, 64, 0], spawn),
            Err(FoundingRefusal::TooCloseToSpawn)
        );
        assert_eq!(founding_allowed(floored, &[], [1_000, 64, 0], spawn), Ok(()));
    }

    #[test]
    fn the_spacing_is_checked_before_the_spawn_limits_exactly_as_java_orders_them() {
        // `GetColonyInfoMessage.java:93` before `:96`. A spot that is both too
        // close to a colony and at spawn answers "too close to the colony".
        let limits = FoundingLimits {
            min_distance_from_world_spawn: 10_000,
            ..FoundingLimits::DEFAULT
        };
        assert_eq!(
            founding_allowed(limits, &[[0, 70, 0]], [10, 70, 10], [0, 64, 0]),
            Err(FoundingRefusal::TooCloseToColony {
                center: [0, 70, 0]
            })
        );
    }
}
