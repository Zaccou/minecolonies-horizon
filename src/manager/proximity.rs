//! How far two colonies must stay apart, and how far one may reach.
//!
//! Java reference: `core/colony/ColonyManager.java:311-324`
//! (`isFarEnoughFromColonies`), `:566-571` (`getMinimumDistanceBetweenTownHalls`),
//! `core/util/ChunkDataHelper.java:135-139` (the box a colony claims, and the
//! doubled box a deletion releases), `:174-207` (`canClaimChunksInRange`),
//! `:218-262` (the ceiling on a building's claim) and
//! `api/util/BlockPosUtil.java:453-481` (the distances).
//!
//! **Three of these four numbers already exist** in `crate::founding::Rules` (the
//! chunk size, the two distances, the claim radius) and are not copied here: this
//! module is the *manager's* door onto them, so a caller that is not a founding
//! does not invent its own radius. The two rules that are new are the one Java keeps
//! on the manager ([`Proximity::minimum_distance_between_town_halls`]) and the one
//! it keeps on the chunk helper's removal branch ([`Proximity::release_radius`]).
//!
//! **The two checks of `isFarEnoughFromColonies` also live in `founding::found`**
//! (`src/founding/found.rs:200-236`), in its own order and with its own refusals.
//! That is Java's own shape — `CreateColonyMessage.java:165` asks the manager *and*
//! runs the founding — and the port keeps both because the founding has to run the
//! checks before it writes one line, while
//! `SurvivalHandler.java:115`, `ClientEventHandler.java:454` and
//! `GetColonyInfoMessage.java:93` ask the manager alone. Both answer with the same
//! `founding::Refusal` variants, so a player reads the same sentence either way.

use crate::founding::{Colonies, Refusal, Rules};

use super::claims::Chunk;

/// Why a building may not claim a chunk: the ceiling of
/// `ChunkDataHelper.buildingClaimInRange` (`:248-255`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal, closed on purpose
pub enum Reach {
    /// The chunk is further from the colony's centre than `maxColonySize` chunks.
    /// Both numbers are **squared chunk distances**, which is what Java compares
    /// (`BlockPosUtil.chunkDistanceSquared`, `:487-492`), and they are `i64` because
    /// two far-apart chunk coordinates squared do not fit an `i32`.
    TooFarFromCentre { chunk: Chunk, distance: i64, max: i64 },
}

impl Reach {
    /// A stable name, namespaced like the other refusals of this port.
    pub fn name(&self) -> &'static str {
        match self {
            Self::TooFarFromCentre { .. } => "manager.refusal.too_far_from_centre",
        }
    }
}

impl std::fmt::Display for Reach {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Reach {}

/// The distances a colony is measured with: the four founding settings plus the
/// ceiling Java keeps separately as `maxColonySize`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Proximity {
    pub rules: Rules,
    /// `maxColonySize` (`ServerConfiguration.java:164`), in **chunks**: the furthest
    /// a building may claim from its colony's centre. `0` means no ceiling, which
    /// Java spells `if (maxColonySize != 0)` (`ChunkDataHelper.java:248`).
    pub max_colony_size: u32,
}

impl Proximity {
    /// The server defaults: 8 chunks between town halls, 4 chunks of initial claim,
    /// 20 chunks of building reach (`ServerConfiguration.java:164-166`).
    pub const DEFAULT: Proximity = Proximity {
        rules: Rules::DEFAULT,
        max_colony_size: 20,
    };

    /// The least distance two town halls may stand apart, **in blocks**:
    /// `minColonyDistance * 16` (`ColonyManager.java:566-571`, whose comment draws
    /// the two radii and the padding between them).
    ///
    /// This is *not* the range [`Proximity::may_settle`] uses: that one takes the
    /// larger of the two chunk settings (`ColonyManager.java:313`), so a big initial
    /// claim refuses a founding the bare town-hall distance would allow.
    pub fn minimum_distance_between_town_halls(&self) -> i64 {
        i64::from(self.rules.min_colony_distance) * i64::from(crate::founding::BLOCKS_PER_CHUNK)
    }

    /// The range a founding must keep clear, in blocks: the larger of the two chunk
    /// settings, in blocks (`ColonyManager.java:313` → `Rules::block_range`).
    pub fn settle_range(&self) -> i64 {
        self.rules.block_range()
    }

    /// The radius a colony claims when it is founded, in chunks
    /// (`ChunkDataHelper.java:135-139`).
    pub fn claim_radius(&self) -> i32 {
        self.rules.claim_radius_chunks()
    }

    /// The radius a deletion unclaims, in chunks: **twice** the founding radius
    /// (`ChunkDataHelper.java:138`, `add ? range : range * 2`). The doubling is Java's
    /// hedge against a colony that grew its claim; a claim beyond it survives in Java,
    /// which is why the port also sweeps the book (`super::claims::ClaimBook::release_colony`).
    pub fn release_radius(&self) -> i32 {
        self.claim_radius() * 2
    }

    /// The chunks of a square box around `center`, **x first then z**, both ends
    /// included: the walk of `ChunkDataHelper.staticClaimInRange` (`:341-347`) and
    /// of `buildingClaimInRange` (`:241-243`). A negative radius is read as zero.
    pub fn chunks_in_range(center: [i32; 3], radius: i32) -> Vec<Chunk> {
        let middle = Rules::chunk(center);
        let radius = radius.max(0);
        let mut chunks = Vec::with_capacity(((2 * radius + 1) * (2 * radius + 1)) as usize);
        for x in middle[0] - radius..=middle[0] + radius {
            for z in middle[1] - radius..=middle[1] + radius {
                chunks.push([x, z]);
            }
        }
        chunks
    }

    /// May a colony be founded here? `ColonyManager.isFarEnoughFromColonies`
    /// (`:311-324`), both halves, in Java's order:
    ///
    /// 1. the nearest colony of this dimension must be at least
    ///    [`Proximity::settle_range`] away, measured in three dimensions with
    ///    `BlockPosUtil.getDistance` (`:316`, truncated by `Rules::distance`);
    /// 2. no chunk the new colony would claim may belong to another one
    ///    (`:321-323` → `canClaimChunksInRange`, `:174-207`).
    ///
    /// **The ceiling on a claim is not checked here**: a founding claims the box of
    /// its radius, which is exactly [`Proximity::claim_radius`], and only a *building*
    /// can exceed [`Proximity::max_colony_size`] (`:248`).
    pub fn may_settle(
        &self,
        colonies: &dyn Colonies,
        dimension: &str,
        at: [i32; 3],
    ) -> Result<(), Refusal> {
        if let Some(nearest) = colonies.closest(dimension, at) {
            let distance = Rules::distance(at, nearest.center);
            let required = self.settle_range();
            if distance < required {
                return Err(Refusal::TooCloseToColony {
                    id: nearest.id,
                    distance,
                    required,
                });
            }
        }
        let radius = self.claim_radius();
        for chunk in Proximity::chunks_in_range(at, radius) {
            if let Some(id) = colonies.owns_chunk(dimension, chunk, radius) {
                return Err(Refusal::GroundClaimed { chunk, id });
            }
        }
        Ok(())
    }

    /// May `colony`'s building claim `chunk`? The ceiling of
    /// `ChunkDataHelper.buildingClaimInRange` (`:248-255`) and of
    /// `buildingClaimBox` (`:299-306`), which skips a chunk too far from the centre
    /// and logs it at debug.
    ///
    /// Java measures the squared **chunk** distance in `buildingClaimInRange` and the
    /// squared **block** distance in `buildingClaimBox`; both compare to the same
    /// `maxColonySize` and the second is written as `(maxColonySize * 16)²`, which is
    /// the first multiplied by 256. The port keeps the chunk form, the one a claim
    /// works in.
    pub fn may_claim(&self, center: [i32; 3], chunk: Chunk) -> Result<(), Reach> {
        if self.max_colony_size == 0 {
            return Ok(());
        }
        let middle = Rules::chunk(center);
        let dx = i64::from(chunk[0]) - i64::from(middle[0]);
        let dz = i64::from(chunk[1]) - i64::from(middle[1]);
        let distance = dx * dx + dz * dz;
        let max = i64::from(self.max_colony_size).pow(2);
        if distance > max {
            return Err(Reach::TooFarFromCentre { chunk, distance, max });
        }
        Ok(())
    }
}

impl Default for Proximity {
    fn default() -> Proximity {
        Proximity::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colony::Colony;
    use crate::founding::Registry;

    const OVERWORLD: &str = "minecraft:overworld";

    fn registry() -> Registry {
        let mut registry = Registry::new();
        registry.insert(Colony::new(1, "Home", OVERWORLD, [0, 70, 0], 1));
        registry
    }

    #[test]
    fn the_least_distance_between_two_town_halls_is_the_chunk_setting_in_blocks() {
        assert_eq!(Proximity::DEFAULT.minimum_distance_between_town_halls(), 128);
        let wide = Proximity {
            rules: Rules {
                min_colony_distance: 20,
                ..Rules::DEFAULT
            },
            ..Proximity::DEFAULT
        };
        assert_eq!(wide.minimum_distance_between_town_halls(), 320);
        assert_eq!(
            Proximity::DEFAULT.settle_range(),
            128,
            "and the range a founding keeps clear is the same number by default"
        );
        let growing = Proximity {
            rules: Rules {
                min_colony_distance: 2,
                initial_colony_size: 15,
                ..Rules::DEFAULT
            },
            ..Proximity::DEFAULT
        };
        assert_eq!(
            growing.settle_range(),
            240,
            "a big initial claim wins over a small town-hall distance"
        );
        assert_eq!(growing.release_radius(), 30, "twice the claim, to unclaim");
    }

    #[test]
    fn a_founding_inside_the_range_of_another_colony_is_refused_by_name() {
        let colonies = registry();
        assert_eq!(
            Proximity::DEFAULT.may_settle(&colonies, OVERWORLD, [100, 70, 0]),
            Err(Refusal::TooCloseToColony {
                id: 1,
                distance: 100,
                required: 128
            })
        );
        assert_eq!(
            Proximity::DEFAULT.may_settle(&colonies, OVERWORLD, [400, 70, 0]),
            Ok(()),
            "400 blocks away, and the claim boxes do not touch either"
        );
        assert_eq!(
            Proximity::DEFAULT.may_settle(&colonies, "minecraft:the_nether", [10, 70, 10]),
            Ok(()),
            "another dimension has no neighbour, ColonyManager.java:495"
        );
    }

    #[test]
    fn ground_another_colony_claimed_is_refused_by_name_once_the_distance_passes() {
        let mut colonies = Registry::new();
        // Chunk 8, box 4..=12: exactly 128 blocks from the origin, so the distance
        // check passes (`<` is strict) and the ground check is what answers.
        colonies.insert(Colony::new(1, "Home", OVERWORLD, [128, 70, 0], 1));
        assert_eq!(
            Proximity::DEFAULT.may_settle(&colonies, OVERWORLD, [0, 70, 0]),
            Err(Refusal::GroundClaimed {
                chunk: [4, -4],
                id: 1
            }),
            "the first chunk of the new box that already belongs to colony 1"
        );
    }

    #[test]
    fn a_building_claim_past_the_max_colony_size_is_refused_and_zero_means_no_ceiling() {
        let center = [0, 70, 0];
        assert_eq!(
            Proximity::DEFAULT.may_claim(center, [20, 0]),
            Ok(()),
            "exactly maxColonySize chunks away is allowed, the comparison is strict"
        );
        assert_eq!(
            Proximity::DEFAULT.may_claim(center, [21, 0]),
            Err(Reach::TooFarFromCentre {
                chunk: [21, 0],
                distance: 441,
                max: 400
            })
        );
        assert_eq!(
            Proximity::DEFAULT.may_claim(center, [3, 4]),
            Ok(()),
            "3² + 4² = 25 chunks of squared distance, and not 5 of each"
        );
        let unlimited = Proximity {
            max_colony_size: 0,
            ..Proximity::DEFAULT
        };
        assert_eq!(unlimited.may_claim(center, [10_000, 0]), Ok(()));
        assert!(Reach::TooFarFromCentre { chunk: [1, 1], distance: 2, max: 1 }.to_string().starts_with("manager.refusal."));
    }

    #[test]
    fn the_box_a_rule_walks_is_x_first_then_z_both_ends_included() {
        assert_eq!(Proximity::chunks_in_range([0, 70, 0], 0), vec![[0, 0]]);
        assert_eq!(
            Proximity::chunks_in_range([0, 70, 0], 1),
            vec![[-1, -1], [-1, 0], [-1, 1], [0, -1], [0, 0], [0, 1], [1, -1], [1, 0], [1, 1]],
            "the same nine chunks, in the same order, on every run"
        );
        assert_eq!(Proximity::chunks_in_range([0, 70, 0], -2).len(), 1, "a negative radius is zero");
        assert_eq!(
            Proximity::chunks_in_range([-1, 0, 15], 0),
            vec![[-1, 0]],
            "a block of -1 is in chunk -1, the floor division of ChunkDataHelper.java:236-237"
        );
    }
}
