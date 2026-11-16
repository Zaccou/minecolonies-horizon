//! The numbers Java hard-codes, each with the line that says so.
//!
//! Only the ones this module reads, plus the two distance tolerances the request
//! system is built on. **Nothing here is duplicated from a later module**: a
//! constant another module already owns is not repeated — `MAX_JUMP_HEIGHT` lives
//! in `pathfinding::node` (`1.3`, `PathingConstants.java:44`) and
//! `HEAL_CITIZENS_AFTER` in `citizens::needs` (`100`,
//! `CitizenConstants.java:58`); base refers to them instead of spelling them a
//! second time.
//!
//! **A constant is not a setting.** The claim sizes are `ForgeConfigSpec.IntValue`s
//! in Java, not `static final`: their *defaults* and their *bounds* are what
//! `ServerConfiguration.java` fixes (`api/configuration/ServerConfiguration.java:164-168`),
//! and a server owner may move them inside those bounds. The port keeps both — the
//! default for a place that has no config, the bounds so [`Territory::new`] clamps
//! instead of trusting a command argument.
//!
//! [`Territory::new`]: super::territory::Territory::new

// ---------------------------------------------------------------- geometry --

/// Blocks along one side of a chunk, `Constants.BLOCKS_PER_CHUNK`
/// (`api/util/constant/Constants.java:55`). Vanilla's `16`; every conversion in
/// `coords` is this number as a shift.
pub const BLOCKS_PER_CHUNK: i32 = 16;

/// Half a block, `Constants.HALF_BLOCK` (`api/util/constant/Constants.java:48`):
/// the offset that centres an entity on a block.
pub const HALF_BLOCK: f64 = 0.5;

/// A tenth of a block up, `Constants.SLIGHTLY_UP`
/// (`api/util/constant/Constants.java:49`): the nudge that keeps a spawned
/// citizen from standing inside the floor.
pub const SLIGHTLY_UP: f64 = 0.1;

// -------------------------------------------------------------- tolerances --

/// Blocks from its home a citizen may be and still count as home,
/// `CitizenConstants.RANGE_TO_BE_HOME`
/// (`api/util/constant/CitizenConstants.java:113`). A `double` in Java — the
/// request system compares a 3D distance against it.
pub const RANGE_TO_BE_HOME: f64 = 16.0;

/// One side of a fence or a pane, `PathingConstants.ONE_SIDE`
/// (`api/util/constant/PathingConstants.java:24`): a position at or below
/// `0.25` of the block is on the near side.
pub const ONE_SIDE: f64 = 0.25;

/// The other side of that fence, `PathingConstants.OTHER_SIDE`
/// (`api/util/constant/PathingConstants.java:29`). Java's two values sum to one,
/// which is what makes "on one side of it" and "past it" cover the block.
pub const OTHER_SIDE: f64 = 0.75;

// ----------------------------------------------------------------- claims ---

/// The id a chunk answers with when **no** colony owns it,
/// `ColonyManagerConstants.NO_COLONY_ID`
/// (`api/util/constant/ColonyManagerConstants.java:56`).
///
/// **Zero, not `-1`**, and that is load-bearing: every "is this chunk claimed"
/// test in Java is `== 0` (`core/util/ChunkDataHelper.java:89`,
/// `api/colony/IColonyTagCapability.java:196`), and a colony id is never zero
/// because the manager hands them out from one. A port that used `-1` would read
/// every unclaimed chunk as claimed by colony `-1`.
pub const NO_COLONY_ID: i32 = 0;

/// Default half-width, in chunks, of the square a new colony claims,
/// `initialColonySize` (`api/configuration/ServerConfiguration.java:166`).
/// Nine by nine chunks — 144 blocks a side — around the town hall's chunk.
pub const INITIAL_COLONY_SIZE: u32 = 4;

/// The smallest `initialColonySize` the config accepts (`:166`).
pub const INITIAL_COLONY_SIZE_MIN: u32 = 1;

/// The largest `initialColonySize` the config accepts (`:166`).
pub const INITIAL_COLONY_SIZE_MAX: u32 = 15;

/// Default half-width, in chunks, of everything a colony may ever claim,
/// `maxColonySize` (`api/configuration/ServerConfiguration.java:164`).
pub const MAX_COLONY_SIZE: u32 = 20;

/// The smallest `maxColonySize` the config accepts (`:164`).
pub const MAX_COLONY_SIZE_MIN: u32 = 1;

/// The largest `maxColonySize` the config accepts (`:164`).
pub const MAX_COLONY_SIZE_MAX: u32 = 250;

/// Default distance, in chunks, two colonies must keep between their town halls,
/// `minColonyDistance` (`api/configuration/ServerConfiguration.java:165`).
pub const MIN_COLONY_DISTANCE: u32 = 8;

/// The smallest `minColonyDistance` the config accepts (`:165`).
pub const MIN_COLONY_DISTANCE_MIN: u32 = 1;

/// The largest `minColonyDistance` the config accepts (`:165`).
pub const MIN_COLONY_DISTANCE_MAX: u32 = 200;

/// How far from the world spawn a colony may be founded,
/// `maxDistanceFromWorldSpawn` (`api/configuration/ServerConfiguration.java:167`).
pub const MAX_DISTANCE_FROM_WORLD_SPAWN: u32 = 30_000;

/// How close to the world spawn a colony must be founded,
/// `minDistanceFromWorldSpawn` (`api/configuration/ServerConfiguration.java:168`).
/// Zero by default: no floor at all.
pub const MIN_DISTANCE_FROM_WORLD_SPAWN: u32 = 0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_colony_is_zero_and_a_colony_id_never_is() {
        // `ColonyManagerConstants.java:56`; the claim rules test `== 0`.
        assert_eq!(NO_COLONY_ID, 0);
        assert_ne!(NO_COLONY_ID, -1);
    }

    #[test]
    fn a_chunk_is_sixteen_blocks_or_it_is_not_a_chunk() {
        assert_eq!(BLOCKS_PER_CHUNK, 16);
        // The shift `coords` uses, checked against the number it stands for.
        assert_eq!(1 << 4, BLOCKS_PER_CHUNK);
        assert_eq!(HALF_BLOCK, 0.5);
        assert!(SLIGHTLY_UP < HALF_BLOCK);
    }

    #[test]
    fn the_claim_defaults_sit_inside_the_bounds_the_config_declares() {
        // `ServerConfiguration.java:164-166` gives each value and its range.
        assert!((INITIAL_COLONY_SIZE_MIN..=INITIAL_COLONY_SIZE_MAX).contains(&INITIAL_COLONY_SIZE));
        assert!((MAX_COLONY_SIZE_MIN..=MAX_COLONY_SIZE_MAX).contains(&MAX_COLONY_SIZE));
        assert!((MIN_COLONY_DISTANCE_MIN..=MIN_COLONY_DISTANCE_MAX).contains(&MIN_COLONY_DISTANCE));
        assert_eq!(INITIAL_COLONY_SIZE, 4);
        assert_eq!(MAX_COLONY_SIZE, 20);
        assert_eq!(MIN_COLONY_DISTANCE, 8);
        // A colony that may only claim 20 chunks away but says two colonies must
        // be 8 chunks apart is Java's own arithmetic: 20 > 8, so no contradiction.
        assert!(MAX_COLONY_SIZE > MIN_COLONY_DISTANCE);
    }

    #[test]
    fn the_fence_tolerances_are_complementary_and_the_home_range_is_blocks() {
        assert_eq!(ONE_SIDE, 0.25);
        assert_eq!(OTHER_SIDE, 0.75);
        assert!((ONE_SIDE + OTHER_SIDE - 1.0).abs() < 1e-12);
        assert_eq!(RANGE_TO_BE_HOME, 16.0);
        assert_eq!(RANGE_TO_BE_HOME as i32, BLOCKS_PER_CHUNK);
    }
}
