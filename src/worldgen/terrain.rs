//! What the ground must be: the surface gate, the cave path, and what each
//! structure asks the generator for.
//!
//! Java reference: `core/structures/EmptyColonyStructure.java` — the codec
//! (`:31-40`), `isFeatureChunk` (`:80-90`), `findGenerationPoint` (`:92-149`) and
//! the cave probe `isFeatureChunkCave` (`:151-213`) — plus the shape each structure
//! file declares (`worldgen/structure/*.json`).
//!
//! **Three rules, and one of them is not a number in the file.**
//!
//! 1. **the surface must not be a fluid.** `topBlock.getFluidState().isEmpty()`
//!    (`:87-89`): water or lava on the highest occupied block means no colony,
//!    which is why no colony of this mod generates on an ocean and all eleven
//!    overworld styles sit on the land tags of [`super::biomes`].
//! 2. **the surface must be under 200.** `landHeight < 200` (`:89`), a **fixed**
//!    bound: not the level's height, not the build height, and not relative to the
//!    world's minimum. A peak at exactly 200 is refused, a valley at 199 is not.
//! 3. **`allow_cave` swaps the whole gate for a probe.** When the file sets it
//!    (`:95-119`), `isFeatureChunk` is never called: the structure looks for a
//!    pocket of [`CAVE_AIR`] contiguous air blocks instead, ten times, stepping down
//!    in random increments of 0..30 from y=0. Exactly one structure sets it —
//!    `warpednetherlands_colony.json:16` — and that is why the nether colony is the
//!    only one that can appear inside a cave.
//!
//! **`start_height` is parsed and never read.** The codec takes it
//! (`EmptyColonyStructure.java:36`), the constructor stores it (`:65`), and
//! `findGenerationPoint` never touches the field: on the surface path the y comes
//! from `getFirstFreeHeight(...)` and `.above(...)` (`:129-130`), on the cave path
//! from the probe. So `above_bottom: 32` on eleven files and `below_top: 120` on
//! the warped one change nothing, and this port does not model a height provider —
//! it models the y the code actually uses ([`start_y`]) and says so, rather than
//! inventing a resolution vanilla would apply to a structure type that ignores it.
//!
//! **The cave probe reads the wrong column, and the port does not.** In
//! `isFeatureChunkCave` the `NoiseColumn` is fetched **before** the mutable position
//! is moved (`:166-167`: `getBaseColumn(mutable.getX(), mutable.getZ(), …)` then
//! `mutable.set(curChunkX << 4, currentY, curChunkZ << 4)`), so every probe after
//! the first reads the chunk corner of the *previous* probe — and the first one
//! reads the world origin `(0, 0)`. [`cave_site`] probes the corner it is
//! examining; the divergence is deliberate and is the one place this module does not
//! follow Java, because reproducing a stale read would make the rule untestable.

use crate::ai::{Block, Pos};

use super::biomes::Structure;
use super::Refusal;

/// Where the surface gate stops: `landHeight < 200`
/// (`EmptyColonyStructure.java:89`), a fixed constant.
pub const MAX_LAND_HEIGHT: i32 = 200;

/// How many contiguous air blocks the cave probe wants: `airCount >= 32`
/// (`EmptyColonyStructure.java:192`, `:204`).
pub const CAVE_AIR: i32 = 32;

/// How many probes the cave path makes: `for (int i = 0; i < 10; i++)` (`:159`).
pub const CAVE_PROBES: usize = 10;

/// The upper bound of one probe's fall: `nextInt(0, 30)` (`:161`), exclusive.
pub const CAVE_STEP: i32 = 30;

/// One surveyed column, with the three facts `isFeatureChunk` reads.
///
/// `top` is `ChunkGenerator.getFirstOccupiedHeight(...)` (`:84`), `free` is
/// `getFirstFreeHeight(...)` (`:129`) — they differ by the terrain's own air, and
/// the port keeps both because Java uses one for the gate and the other for the y.
/// `fluid` is `!topBlock.getFluidState().isEmpty()` at `top` (`:87`), already
/// resolved by the caller: [`Block::Fluid`] is that answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    /// The block column, `[x, z]`.
    pub at: [i32; 2],
    /// The biome at this column, as `level_biome_at` reports it.
    pub biome: String,
    /// `getFirstOccupiedHeight`: the highest block that is not air.
    pub top: i32,
    /// `getFirstFreeHeight`: the lowest air block at or above the surface.
    pub free: i32,
    /// The block at `top` holds a fluid.
    pub fluid: bool,
}

impl Column {
    /// The three facts, in the order the gate reads them.
    pub fn new(at: [i32; 2], biome: &str, top: i32, free: i32, fluid: bool) -> Column {
        Column {
            at,
            biome: biome.to_owned(),
            top,
            free,
            fluid,
        }
    }

    /// The position `isFeatureChunk` would refuse at: the column, at its surface.
    pub fn surface(&self) -> [i32; 3] {
        [self.at[0], self.top, self.at[1]]
    }
}

/// The surface gate of `EmptyColonyStructure.java:80-90`, in Java's order.
///
/// A fluid is refused first because Java's expression tests it first and
/// short-circuits (`:89`); a column that is both wet and above 200 therefore reads
/// as wet, which is the message a player can act on.
pub fn surface_ok(column: &Column) -> Result<(), Refusal> {
    if column.fluid {
        return Err(Refusal::SurfaceIsFluid {
            at: column.surface(),
        });
    }
    if column.top >= MAX_LAND_HEIGHT {
        return Err(Refusal::LandTooHigh {
            top: column.top,
            limit: MAX_LAND_HEIGHT,
        });
    }
    Ok(())
}

/// The y the pieces are attached at — **not** an absolute y.
///
/// `findGenerationPoint` takes the chunk's middle block position at y=0, asks for
/// `getFirstFreeHeight`, then calls `.above(topLandY)` (`:127-130`), so the y is
/// `free + 1` and the height of the whole structure is added by the jigsaw on top of
/// it. `start_height` plays no part (see the module docs).
pub fn start_y(column: &Column) -> i32 {
    column.free + 1
}

/// What a block source answers the cave probe: is this cell air, and was the cell
/// ever looked at.
pub trait Air {
    /// `blockState.isAir()` at a block position.
    fn is_air(&self, at: [i32; 3]) -> bool;

    /// Whether the cell's chunk was loaded when the caller looked. Default `true`,
    /// so a fake only has one question to answer; `crate::ai::Blocks` reports what
    /// its caller marked, and a probe that walks an unloaded chunk gets
    /// [`Refusal::NotSurveyed`] rather than a cave it never saw.
    fn is_loaded(&self, _at: [i32; 3]) -> bool {
        true
    }
}

impl Air for crate::ai::Blocks {
    fn is_air(&self, at: [i32; 3]) -> bool {
        self.at(Pos::new(at[0], at[1], at[2])) == Block::Air
    }

    fn is_loaded(&self, at: [i32; 3]) -> bool {
        self.is_loaded(Pos::new(at[0], at[1], at[2]))
    }
}

/// The cave probe of `EmptyColonyStructure.java:151-213`: a pocket of [`CAVE_AIR`]
/// air blocks somewhere in the three-by-three chunks around `chunk`, or `None`.
///
/// **The random draw is the caller's.** Java steps down by `nextInt(0, 30)` before
/// each of the ten probes (`:161`); a wasm mod cannot replay a `RandomSource`, so
/// `draws` is the list of those steps — a missing entry counts as `0`, which is a
/// legal draw and is why a caller may pass fewer than [`CAVE_PROBES`].
///
/// **The count is Java's, quirks included**: the starting cell is counted twice
/// (`airCount = 1` at `:170`, then the downward loop counts it again at `:173-177`,
/// then the upward loop counts it a third time at `:185-190`), so a pocket of 30 air
/// blocks can pass where 32 are asked for. The port keeps the arithmetic and a test
/// measures it, rather than quietly asking for "a 32-block pocket".
///
/// **The column is the one being probed.** Java reads the column of the *previous*
/// position (`:166-167`); see the module docs.
pub fn cave_site(
    air: &dyn Air,
    chunk: [i32; 2],
    min_y: i32,
    depth: i32,
    draws: &[i32],
) -> Result<Option<[i32; 3]>, Refusal> {
    let mut current = 0;
    for probe in 0..CAVE_PROBES {
        current += draws.get(probe).copied().unwrap_or(0);
        for corner_x in -1..=1 {
            for corner_z in -1..=1 {
                // `curChunkX << 4`: the *corner* of the neighbouring chunk, not its
                // middle — one probe per chunk, at its north-west block.
                let at = [(chunk[0] + corner_x) << 4, current, (chunk[1] + corner_z) << 4];
                if !air.is_loaded(at) {
                    return Err(Refusal::NotSurveyed { chunk: [at[0] >> 4, at[2] >> 4] });
                }
                if !air.is_air(at) {
                    continue;
                }
                let mut air_count = 1;
                // Down (`:171-184`): count the air, stop at the world floor.
                let mut y = current;
                while y > min_y {
                    if air.is_air([at[0], y, at[2]]) {
                        air_count += 1;
                    } else {
                        break;
                    }
                    y -= 1;
                }
                // Up (`:186-202`), from the same starting cell.
                y = current;
                while y < min_y + depth {
                    if air.is_air([at[0], y, at[2]]) {
                        air_count += 1;
                        if air_count >= CAVE_AIR {
                            break;
                        }
                    } else {
                        break;
                    }
                    y += 1;
                }
                if air_count >= CAVE_AIR {
                    return Ok(Some([at[0], y, at[2]]));
                }
            }
        }
    }
    Ok(None)
}

/// How the generator is asked to reshape the ground under the pieces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two values the sixteen files use
pub enum Adaptation {
    /// `"terrain_adaptation": "beard_box"` — fill and shave the ground under the
    /// pieces (`colonial_colony.json:14`).
    BeardBox,
    /// No `terrain_adaptation` at all: the ship only (`ship.json`, which has none
    /// and `"adapt_noise": false`).
    None,
}

/// What one structure file asks the generator for, as the port reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Form {
    /// The structure the form belongs to.
    pub structure: Structure,
    /// `size` (`colonial_colony.json:5`): the jigsaw's depth of pieces.
    pub size: u8,
    /// `max_distance_from_center` (`:6`): the furthest a piece may be placed from
    /// the anchor, in blocks. The box the colony can cover is that far around the
    /// anchor, which is what [`super::site::choose`] refuses a structure start in.
    pub reach: i32,
    /// `allow_cave` (`warpednetherlands_colony.json:16`).
    pub allow_cave: bool,
    /// `terrain_adaptation` (`:14`), and `adapt_noise` (`:16`).
    pub adaptation: Adaptation,
}

impl Form {
    /// The reach in chunks: `max_distance_from_center` (a block count) divided by
    /// 16. 80 gives 5 and 16 gives 1, so the division is exact for all sixteen
    /// files and no rounding question comes up.
    pub fn reach_chunks(&self) -> i32 {
        self.reach.div_euclid(crate::base::BLOCKS_PER_CHUNK)
    }

    /// The form a structure file declares. Measured from the sixteen files: eleven
    /// colonies are `size 5` / `reach 80` / no cave, the warped one adds the cave,
    /// the three camps are `reach 16` (`size 7`, `1`, `1`), the ship is `size 1`
    /// with no adaptation.
    pub fn of(structure: Structure) -> Form {
        match structure {
            Structure::WarpedNetherlands => Form {
                structure,
                size: 5,
                reach: 80,
                allow_cave: true,
                adaptation: Adaptation::BeardBox,
            },
            Structure::AmazonCamp => Form {
                structure,
                size: 7,
                reach: 16,
                allow_cave: false,
                adaptation: Adaptation::BeardBox,
            },
            Structure::BarbarianCamp | Structure::DesertCamp => Form {
                structure,
                size: 1,
                reach: 16,
                allow_cave: false,
                adaptation: Adaptation::BeardBox,
            },
            Structure::Ship => Form {
                structure,
                size: 1,
                reach: 16,
                allow_cave: false,
                adaptation: Adaptation::None,
            },
            _ => Form {
                structure,
                size: 5,
                reach: 80,
                allow_cave: false,
                adaptation: Adaptation::BeardBox,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::Blocks;

    #[test]
    fn a_wet_surface_is_refused_before_the_height_and_at_its_own_y() {
        let column = Column::new([8, 8], "minecraft:ocean", 62, 63, true);
        assert_eq!(
            surface_ok(&column),
            Err(Refusal::SurfaceIsFluid { at: [8, 62, 8] })
        );
        // Java short-circuits on the fluid, so a wet column at y=300 is wet, not
        // too high (`EmptyColonyStructure.java:89`).
        let high_and_wet = Column::new([0, 0], "minecraft:ocean", 300, 301, true);
        assert_eq!(
            surface_ok(&high_and_wet),
            Err(Refusal::SurfaceIsFluid { at: [0, 300, 0] })
        );
    }

    #[test]
    fn the_height_bound_is_199_in_200_out_and_it_is_not_the_build_height() {
        let at = |top: i32| Column::new([1, 2], "minecraft:plains", top, top + 1, false);
        assert_eq!(surface_ok(&at(199)), Ok(()));
        assert_eq!(
            surface_ok(&at(200)),
            Err(Refusal::LandTooHigh {
                top: 200,
                limit: MAX_LAND_HEIGHT
            })
        );
        assert_eq!(
            surface_ok(&at(320)),
            Err(Refusal::LandTooHigh {
                top: 320,
                limit: MAX_LAND_HEIGHT
            }),
            "a level that builds to 320 refuses nothing: the bound is 200"
        );
        // Below sea level is fine as long as it is dry, and no lower bound exists.
        assert_eq!(surface_ok(&at(-40)), Ok(()));
        assert_eq!(MAX_LAND_HEIGHT, 200);
    }

    #[test]
    fn the_pieces_hang_one_block_above_the_first_free_height() {
        // `getFirstFreeHeight` then `.above(...)` (`:129-130`).
        assert_eq!(start_y(&Column::new([0, 0], "minecraft:plains", 63, 64, false)), 65);
        // On a column whose top block is air, `free` is the same y as `top`.
        assert_eq!(start_y(&Column::new([0, 0], "minecraft:plains", 70, 71, false)), 72);
    }

    #[test]
    fn the_eleven_overworld_colonies_and_the_warpped_one_are_the_only_forms_that_differ() {
        for structure in Structure::COLONIES {
            let form = Form::of(structure);
            assert_eq!(form.structure, structure);
            assert_eq!(form.size, 5, "{}", structure.id());
            assert_eq!(form.reach, 80, "{}", structure.id());
            assert_eq!(
                form.adaptation,
                Adaptation::BeardBox,
                "{}",
                structure.id()
            );
            assert_eq!(
                form.allow_cave,
                structure == Structure::WarpedNetherlands,
                "{}: only the nether colony asks for a cave",
                structure.id()
            );
        }
    }

    #[test]
    fn the_camps_and_the_ship_reach_sixteen_blocks_and_the_ship_does_not_adapt() {
        assert_eq!(Form::of(Structure::AmazonCamp).size, 7);
        for structure in [
            Structure::AmazonCamp,
            Structure::BarbarianCamp,
            Structure::DesertCamp,
            Structure::Ship,
        ] {
            let form = Form::of(structure);
            assert_eq!(form.reach, 16, "{}", structure.id());
            assert!(!form.allow_cave);
        }
        assert_eq!(Form::of(Structure::BarbarianCamp).size, 1);
        assert_eq!(Form::of(Structure::DesertCamp).size, 1);
        assert_eq!(Form::of(Structure::Ship).size, 1);
        assert_eq!(Form::of(Structure::Ship).adaptation, Adaptation::None);
        assert!(!Structure::Ship.is_colony());
    }

    /// A world that is solid everywhere except the cells named air — the shape a
    /// probe needs, since a world that is air everywhere answers on the first corner.
    struct Pocket(Vec<[i32; 3]>);

    impl Air for Pocket {
        fn is_air(&self, at: [i32; 3]) -> bool {
            self.0.contains(&at)
        }
    }

    /// Solid everywhere, one chunk reported unloaded.
    struct Unloaded([i32; 2]);

    impl Air for Unloaded {
        fn is_air(&self, _at: [i32; 3]) -> bool {
            true
        }

        fn is_loaded(&self, at: [i32; 3]) -> bool {
            [at[0] >> 4, at[2] >> 4] != self.0
        }
    }

    /// The air of a full column of `count` cells starting at `from`, at x=z=0.
    fn column(from: i32, count: i32) -> Pocket {
        Pocket((0..count).map(|step| [0, from + step, 0]).collect())
    }

    #[test]
    fn a_cave_pocket_of_thirty_air_passes_because_the_starting_cell_is_counted_thrice() {
        // Java counts the starting cell at `:170`, again while descending (`:173`),
        // and once more while climbing (`:187`): 30 distinct air blocks reach 32.
        let found = cave_site(&column(61, 30), [0, 0], -64, 384, &[61]).expect("a loaded probe");
        assert_eq!(
            found,
            Some([0, 90, 0]),
            "the y where the count reached 32 is the 30th air block"
        );

        // Two air blocks fewer, and nothing is found on any of the ten probes.
        assert_eq!(
            cave_site(&column(61, 28), [0, 0], -64, 384, &[61; 10]).expect("a loaded probe"),
            None
        );
        assert_eq!(CAVE_AIR, 32);
        assert_eq!(CAVE_PROBES, 10);
    }

    #[test]
    fn a_probe_tests_one_y_per_chunk_and_does_not_search_for_a_pocket_below_it() {
        // The first check is `blockView.getBlock(currentY).isAir()` (`:168`): the
        // probe's y is its draw, so a pocket at y=10..=29 is invisible to a probe
        // at 61.
        assert_eq!(
            cave_site(&column(10, 20), [0, 0], -64, 384, &[61]).expect("loaded"),
            None
        );
        assert_eq!(
            cave_site(&column(61, 40), [0, 0], -64, 384, &[61]).expect("loaded"),
            Some([0, 61 + 29, 0])
        );
    }

    #[test]
    fn the_ten_probes_accumulate_the_callers_draws() {
        // Ten draws of 30 put the tenth probe at y=300, not at 300 + 9 steps
        // (`for (int i = 0; i < 10; i++) { currentY += … }`, `:159-161`: the first
        // probe already carries its draw).
        //
        // The pocket has to fit under the world's ceiling: the upward walk stops at
        // `getMinY() + getGenDepth()` (`:186`), here -64 + 384 = 320. So 30 air
        // blocks at 290..=319, and the 32nd count lands on y=319 — where the
        // downward walk from 300 has already added eleven (count 12) and the upward
        // walk adds the twenty more.
        let found = cave_site(&column(290, 30), [0, 0], -64, 384, &[30; 10]).expect("loaded");
        assert_eq!(found, Some([0, 319, 0]), "the probe at 300, counted up to 319");
        // The same pocket three blocks higher would need y=322, above the ceiling:
        // nothing is found, because the climb is cut at 320 before the 32nd block.
        assert_eq!(
            cave_site(&column(293, 30), [0, 0], -64, 384, &[30; 10]).expect("loaded"),
            None
        );
        // A pocket one block below the last probe is never seen.
        assert_eq!(
            cave_site(&column(290, 10), [0, 0], -64, 384, &[30; 10]).expect("loaded"),
            None
        );
        // A caller that supplies no draw probes y=0 ten times.
        assert_eq!(
            cave_site(&column(0, 40), [0, 0], -64, 384, &[]).expect("loaded"),
            Some([0, 29, 0])
        );
    }

    #[test]
    fn an_unloaded_probe_column_is_a_refusal_and_never_a_cave() {
        assert_eq!(
            cave_site(&Unloaded([-1, -1]), [0, 0], -64, 384, &[0]),
            Err(Refusal::NotSurveyed { chunk: [-1, -1] }),
            "the first corner probed is the chunk to the north-west"
        );
        let first_corner = cave_site(&Unloaded([0, 0]), [0, 0], -64, 384, &[0]).expect("loaded");
        assert_eq!(
            first_corner.map(|at| [at[0], at[2]]),
            Some([-16, -16]),
            "the first corner probed is the chunk to the north-west"
        );
    }

    #[test]
    fn the_three_by_three_chunks_around_the_candidate_are_all_probed() {
        // Air only in the chunk one step east, at its corner `(16, y, 0)`.
        let east = Pocket((0..40).map(|step| [16, step, 0]).collect());
        assert_eq!(
            cave_site(&east, [0, 0], -64, 384, &[0]).expect("loaded"),
            Some([16, 29, 0]),
            "the east neighbour is probed after the nine corners before it"
        );
        // And the same pocket two chunks away is out of the probe's reach.
        let far = Pocket((0..40).map(|step| [32, step, 0]).collect());
        assert_eq!(
            cave_site(&far, [0, 0], -64, 384, &[0]).expect("loaded"),
            None
        );
    }

    #[test]
    fn the_air_bridge_reads_air_and_loaded_off_the_ai_blocks() {
        let mut blocks = Blocks::new();
        assert!(Air::is_air(&blocks, [0, 0, 0]), "an unreported cell is air");
        assert!(Air::is_loaded(&blocks, [0, 0, 0]));
        blocks.set(Pos::new(0, 0, 0), Block::Solid);
        assert!(!Air::is_air(&blocks, [0, 0, 0]));
        blocks.mark_unloaded(Pos::new(1, 0, 0));
        assert!(!Air::is_loaded(&blocks, [1, 0, 0]));
    }
}
