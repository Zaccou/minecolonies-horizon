//! How far apart the generator puts two colonies: `spacing`, `separation`, `salt`,
//! and the geometry they fix.
//!
//! Java reference: `data/minecolonies/worldgen/structure_set/empty_colony.json`
//! (`:53-56`, the placement of the twelve colonies), `:9-11` of the three camps
//! and of the ship. The algorithm those numbers feed is vanilla's
//! `RandomSpreadStructurePlacement` and `WorldgenRandom`, **not in this tree**: the
//! port keeps the geometric consequence, and the draw itself is the caller's (see
//! `src/worldgen/MANQUANT.md` §1).
//!
//! **The two numbers, read as vanilla reads them.** A world is cut into square
//! *regions* of `spacing` chunks; each region holds exactly one candidate, at an
//! offset inside the first `spacing - separation` chunks of the region. Everything
//! else in the placement JSON is a vanilla default the mod never writes:
//! `spread_type` is `linear`, `locate_offset` is `[0, 0, 0]`, and
//! `frequency_reduction_method` is `default`.
//!
//! So two candidates of one set differ by **at least `separation + 1` chunks on at
//! least one axis**. That `+ 1` is not a slip: the last legal offset of a region is
//! `spacing - separation - 1`, and the first offset of the next region is `spacing`,
//! so the gap between them is `separation + 1`. Spelled in blocks:
//!
//! | Set | `spacing` | `separation` | least gap | in blocks |
//! | --- | ---: | ---: | ---: | ---: |
//! | the twelve colonies | 95 | 45 | 46 chunks | **736** |
//! | the three camps | 55 | 25 | 26 chunks | **416** |
//! | the ship | 55 | 25 | 26 chunks | **416** |
//!
//! **736 blocks between two generated colonies, 128 between two founded ones.**
//! The second number is `ColonyManager.java:313` and lives in
//! `crate::founding::Rules::block_range`; a test below pins both, because they
//! answer different questions and a reader who mixed them up would conclude that
//! the generator keeps colonies 128 blocks apart, which it does not.
//!
//! **`salt` is what makes two sets independent — or not.** Two sets with the same
//! salt, spacing and separation compute their candidate for a region from the same
//! inputs, so they target the **same chunks**: that is exactly the case of the three
//! camps (`1223366777` three times), and the biome tag is then the only thing that
//! tells an amazon camp from a desert one. The ship shares the colonies' salt
//! (`1225566777`) but not their spacing, so the two grids do not line up.

use super::biomes::Structure;
use crate::base::BLOCKS_PER_CHUNK;
use crate::founding::Rules;

/// One `random_spread` placement: how wide a region is, how much of it stays empty,
/// and the salt that keeps sets apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Spread {
    /// Chunks per region — `spacing` (`structure_set/empty_colony.json:54`).
    pub spacing: i32,
    /// Chunks a candidate keeps from the region's edge — `separation` (`:55`).
    pub separation: i32,
    /// The seed of the per-region draw — `salt` (`:53`).
    pub salt: i64,
}

impl Spread {
    /// The region a chunk falls in: vanilla's `Math.floorDiv(chunk, spacing)`.
    ///
    /// **A floor division, and it has to be**: chunk `-1` is in region `-1` with a
    /// spacing of 95, not region `0`. `i32::div_euclid` is that division;
    /// `-1 / 95` would answer `0` and put two neighbouring chunks of the origin in
    /// different regions than their neighbours.
    pub fn region(&self, chunk: [i32; 2]) -> [i32; 2] {
        [chunk[0].div_euclid(self.spacing), chunk[1].div_euclid(self.spacing)]
    }

    /// The number of legal offsets per axis: `spacing - separation`, and **at least
    /// one** — vanilla clamps it (`Math.max(spacing - separation, 1)`), so a data
    /// pack that wrote `separation == spacing` still has a candidate per region
    /// instead of an empty one.
    pub fn offset_limit(&self) -> i32 {
        (self.spacing - self.separation).max(1)
    }

    /// The chunk a region starts at — its north-west candidate-free corner.
    pub fn origin(&self, region: [i32; 2]) -> [i32; 2] {
        [region[0] * self.spacing, region[1] * self.spacing]
    }

    /// The chunk range the candidate of a region may take, both ends included.
    pub fn candidate_bounds(&self, region: [i32; 2]) -> ([i32; 2], [i32; 2]) {
        let origin = self.origin(region);
        let last = self.offset_limit() - 1;
        ([origin[0], origin[1]], [origin[0] + last, origin[1] + last])
    }

    /// The least distance two candidates of this placement can have, **in chunks**:
    /// `separation + 1`, on at least one axis. See the module docs for the `+ 1`.
    pub fn minimum_gap_chunks(&self) -> i32 {
        self.separation + 1
    }

    /// The same in blocks: what a player measures between two generated colonies.
    pub fn minimum_gap_blocks(&self) -> i32 {
        self.minimum_gap_chunks() * BLOCKS_PER_CHUNK
    }

    /// Are two candidate chunks far enough apart to be two placements of this set?
    ///
    /// The guarantee is on one axis, not on the diagonal: two candidates 45 chunks
    /// apart on x **and** 45 on z are 63 blocks short of the 46-chunk gap, so the
    /// answer is `false`. Nothing in vanilla promises more than `separation + 1`,
    /// and a test below measures the diagonal case.
    pub fn apart_enough(&self, first: [i32; 2], second: [i32; 2]) -> bool {
        let gap = self.minimum_gap_chunks();
        (first[0] - second[0]).abs() >= gap || (first[1] - second[1]).abs() >= gap
    }

    /// Whether two placements target the **same** candidate chunks: same salt,
    /// spacing and separation, so the same region gives the same draw.
    pub fn same_cells_as(&self, other: &Spread) -> bool {
        self.salt == other.salt
            && self.spacing == other.spacing
            && self.separation == other.separation
    }
}

/// One structure set: an id, the file that holds its placement, and the structures
/// the placement is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Set {
    /// The set's registry name, e.g. `minecolonies:empty_colony`.
    pub id: &'static str,
    /// `file:lines` of the placement it was read from.
    pub source: &'static str,
    /// The placement.
    pub spread: Spread,
    /// The structures the set holds, in the file's order. The weights are all `1`
    /// in every MineColonies set, so the port does not carry them: which of the
    /// twelve a cell ends up holding is vanilla's weighted draw, and
    /// [`super::site::choose`] is asked for one structure at a time.
    pub structures: &'static [Structure],
}

/// The twelve colonies, in one set: `spacing` 95, `separation` 45, salt
/// `1225566777` (`worldgen/structure_set/empty_colony.json:53-56`).
pub const COLONY_SET: Set = Set {
    id: "minecolonies:empty_colony",
    source: "worldgen/structure_set/empty_colony.json:53-56",
    spread: Spread {
        spacing: 95,
        separation: 45,
        salt: 1_225_566_777,
    },
    structures: &[
        Structure::MedievalBirch,
        Structure::MedievalOak,
        Structure::MedievalSpruce,
        Structure::MedievalDarkOak,
        Structure::Asian,
        Structure::Caledonia,
        Structure::LostMesaCity,
        Structure::Desertoasis,
        Structure::Shire,
        Structure::Incan,
        Structure::Colonial,
        Structure::WarpedNetherlands,
    ],
};

/// The three camps, each in its own set, each with the **same** number: `spacing`
/// 55, `separation` 25, salt `1223366777` (`structure_set/amazon_camp.json:9-11`,
/// `barbarian_camp.json:9-11`, `desert_camp.json:9-11`).
pub const CAMP_SETS: [Set; 3] = [
    Set {
        id: "minecolonies:amazon_camp",
        source: "worldgen/structure_set/amazon_camp.json:9-11",
        spread: Spread {
            spacing: 55,
            separation: 25,
            salt: 1_223_366_777,
        },
        structures: &[Structure::AmazonCamp],
    },
    Set {
        id: "minecolonies:barbarian_camp",
        source: "worldgen/structure_set/barbarian_camp.json:9-11",
        spread: Spread {
            spacing: 55,
            separation: 25,
            salt: 1_223_366_777,
        },
        structures: &[Structure::BarbarianCamp],
    },
    Set {
        id: "minecolonies:desert_camp",
        source: "worldgen/structure_set/desert_camp.json:9-11",
        spread: Spread {
            spacing: 55,
            separation: 25,
            salt: 1_223_366_777,
        },
        structures: &[Structure::DesertCamp],
    },
];

/// The ship: `spacing` 55, `separation` 25, and the **colonies'** salt
/// (`structure_set/ship.json:9-11`).
pub const SHIP_SET: Set = Set {
    id: "minecolonies:ship",
    source: "worldgen/structure_set/ship.json:9-11",
    spread: Spread {
        spacing: 55,
        separation: 25,
        salt: 1_225_566_777,
    },
    structures: &[Structure::Ship],
};

/// The set a structure belongs to. Every one of the sixteen is in exactly one, and
/// a test walks all sixteen to say so.
pub fn set_of(structure: Structure) -> &'static Set {
    if structure == Structure::Ship {
        return &SHIP_SET;
    }
    if let Some(set) = CAMP_SETS
        .iter()
        .find(|set| set.structures.contains(&structure))
    {
        return set;
    }
    &COLONY_SET
}

/// The two distances that are *not* the same distance, for a test and a reader:
/// the generated gap of the colony set, and the gap a founding keeps. The first is
/// `spacing`-derived, the second is `ColonyManager.java:313`.
pub fn generated_gap_beats_the_founding_gap() -> (i32, i64) {
    (
        COLONY_SET.spread.minimum_gap_blocks(),
        i64::from(Rules::DEFAULT.block_range()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_colony_set_keeps_736_blocks_and_the_camps_416() {
        assert_eq!(COLONY_SET.spread.spacing, 95);
        assert_eq!(COLONY_SET.spread.separation, 45);
        assert_eq!(COLONY_SET.spread.minimum_gap_chunks(), 46);
        assert_eq!(COLONY_SET.spread.minimum_gap_blocks(), 736);
        for set in CAMP_SETS.iter().chain([&SHIP_SET]) {
            assert_eq!(set.spread.spacing, 55);
            assert_eq!(set.spread.separation, 25);
            assert_eq!(set.spread.minimum_gap_chunks(), 26);
            assert_eq!(set.spread.minimum_gap_blocks(), 416);
        }
    }

    #[test]
    fn a_region_is_a_floor_division_so_the_origin_is_in_region_minus_one() {
        let spread = COLONY_SET.spread;
        assert_eq!(spread.region([0, 0]), [0, 0]);
        assert_eq!(spread.region([94, 94]), [0, 0]);
        assert_eq!(spread.region([95, 95]), [1, 1]);
        assert_eq!(spread.origin([1, 1]), [95, 95]);
        // `-1 / 95` is `0` in Rust and Java alike; the floor is `-1`.
        assert_eq!(spread.region([-1, -1]), [-1, -1]);
        assert_eq!(spread.origin([-1, -1]), [-95, -95]);
        assert_eq!(spread.region([-95, -95]), [-1, -1]);
        assert_eq!(spread.region([-96, -96]), [-2, -2]);
    }

    #[test]
    fn the_candidate_of_a_region_stays_in_its_first_spacing_minus_separation_chunks() {
        let spread = COLONY_SET.spread;
        assert_eq!(spread.offset_limit(), 50);
        let (low, high) = spread.candidate_bounds([2, -1]);
        assert_eq!(low, [190, -95]);
        assert_eq!(high, [190 + 49, -95 + 49]);
        // The bound is derived, not guessed: `spacing - separation` offsets, 0..=49.
        assert_eq!(high[0] - low[0] + 1, spread.offset_limit());
        // A data pack that sets separation equal to spacing still gets one offset.
        let degenerate = Spread {
            spacing: 16,
            separation: 16,
            salt: 0,
        };
        assert_eq!(degenerate.offset_limit(), 1);
        assert_eq!(degenerate.candidate_bounds([0, 0]).1, [0, 0]);
    }

    #[test]
    fn the_gap_is_the_separation_plus_one_on_one_axis_and_the_diagonal_does_not_help() {
        let spread = COLONY_SET.spread;
        let at = [0, 0];
        assert!(!spread.apart_enough(at, at));
        assert!(!spread.apart_enough(at, [45, 0]), "45 chunks is one short");
        assert!(spread.apart_enough(at, [46, 0]));
        assert!(spread.apart_enough(at, [0, -46]));
        // 45 on both axes is a diagonal of 63.6 blocks in chunk units, and it is
        // still two candidates of *one* region pair, so it must be refused: the
        // guarantee is per axis.
        assert!(!spread.apart_enough(at, [45, 45]));
        assert!(spread.apart_enough(at, [46, 45]));
    }

    #[test]
    fn two_generated_colonies_are_farther_apart_than_a_founding_keeps_them() {
        let (generated, founding) = generated_gap_beats_the_founding_gap();
        assert_eq!(generated, 736);
        assert_eq!(founding, 128, "max(minColonyDistance, initialColonySize) x 16");
        assert!(
            i64::from(generated) > founding,
            "the generator's grid is the wider rule, and the port keeps both"
        );
    }

    #[test]
    fn the_three_camps_target_the_same_cells_and_the_ship_does_not_target_the_colonies() {
        assert!(CAMP_SETS[0].spread.same_cells_as(&CAMP_SETS[1].spread));
        assert!(CAMP_SETS[1].spread.same_cells_as(&CAMP_SETS[2].spread));
        assert!(
            !CAMP_SETS[0].spread.same_cells_as(&SHIP_SET.spread),
            "the ship has the camps' spacing but the colonies' salt"
        );
        assert_eq!(SHIP_SET.spread.salt, COLONY_SET.spread.salt);
        assert!(
            !SHIP_SET.spread.same_cells_as(&COLONY_SET.spread),
            "same salt, different spacing: the grids do not line up"
        );
        assert!(!COLONY_SET.spread.same_cells_as(&CAMP_SETS[0].spread));
    }

    #[test]
    fn every_structure_is_in_exactly_one_set_and_the_colony_set_holds_the_twelve() {
        assert_eq!(COLONY_SET.structures.len(), 12);
        assert_eq!(COLONY_SET.spread.salt, SHIP_SET.spread.salt);
        let mut seen: Vec<Structure> = Vec::new();
        for structure in Structure::ALL {
            let set = set_of(structure);
            assert!(
                set.structures.contains(&structure),
                "{} found in {}",
                structure.id(),
                set.id
            );
            assert!(!seen.contains(&structure));
            seen.push(structure);
            // And no other set claims it.
            let owners: Vec<&str> = [COLONY_SET, SHIP_SET]
                .iter()
                .chain(CAMP_SETS.iter())
                .filter(|set| set.structures.contains(&structure))
                .map(|set| set.id)
                .collect();
            assert_eq!(owners.len(), 1, "{} is in {owners:?}", structure.id());
            assert_eq!(owners[0], set.id);
        }
        assert_eq!(seen.len(), 16);
        assert_eq!(set_of(Structure::Ship).id, "minecolonies:ship");
        assert_eq!(set_of(Structure::AmazonCamp).id, "minecolonies:amazon_camp");
        assert_eq!(set_of(Structure::Colonial).id, "minecolonies:empty_colony");
    }

    #[test]
    fn a_candidate_chunk_is_always_inside_the_region_it_belongs_to() {
        let spread = COLONY_SET.spread;
        for region in [[0, 0], [1, -3], [-7, 12], [-1, -1]] {
            let (low, high) = spread.candidate_bounds(region);
            for chunk in [low, high, [(low[0] + high[0]) / 2, low[1]]] {
                assert_eq!(
                    spread.region(chunk),
                    region,
                    "candidate {chunk:?} must be in region {region:?}"
                );
            }
        }
    }
}
