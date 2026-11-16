//! Where a colony may appear: the biomes, the spacing, the ground, and the site
//! that is kept.
//!
//! **The mission named two packages that do not exist in this tree.**
//! `core/worldgen/` and `api/worldgen/` **do not exist** under
//! `minecolonies-src/src/main/java/com/minecolonies/` (1.20.1,
//! `gradle.properties:21-31`) : a `find … -name '*.java'` under either answers
//! `No such file or directory`. The rules are nonetheless real, and this module
//! ports them from the four places they actually live:
//!
//! | Rule | Where it is written |
//! | --- | --- |
//! | which biome hosts which colony | `data/minecolonies/tags/worldgen/biome/has_structure/*.json` (16 files) |
//! | how far apart two generated colonies are | `data/minecolonies/worldgen/structure_set/empty_colony.json:53-56` |
//! | what the ground must be | `core/structures/EmptyColonyStructure.java:80-90`, `:151-213` |
//! | how far a *founded* colony is from another | `core/colony/ColonyManager.java:311-320`, already ported in `crate::founding` |
//!
//! The structure type itself is registered at
//! `core/structures/MineColoniesStructures.java:25` (`empty_colony`), and its codec
//! is `EmptyColonyStructure.java:31-40`.
//!
//! | File | Question |
//! | --- | --- |
//! | `biomes` | which biomes host which structure, and which host no colony at all |
//! | `spread` | how far apart the generator puts two colonies, from `spacing`/`separation` |
//! | `terrain` | what the ground must be: no fluid on top, under 200 blocks, a cave when asked |
//! | `site` | of the candidate sites, which one is kept, and why the others are not |
//!
//! **Two kinds of number, and they are not the same number.** A *generated* colony
//! (the abandoned town hall a player may reactivate) is placed by the world
//! generator, which keeps **736 blocks** between two of its own (see
//! [`spread::COLONY_SET`]); a *founded* colony is placed by a player, and Java keeps
//! it `max(minColonyDistance, initialColonySize) << 4` — **128 blocks** by default —
//! from the nearest other town hall (`ColonyManager.java:313`). [`site::choose`]
//! asks the second question of a generated site as well, through
//! `crate::founding::Rules`, so a generated ruin never lands on a player's colony;
//! the two distances differ by design and the port keeps both.
//!
//! **Nothing here reads the world.** Like `structures`, `base` and `founding`, this
//! module is data and rules: a biome is a [`String`], a column is a [`Column`]
//! the caller surveyed, and the host calls that would fill them are named in
//! `src/worldgen/MANQUANT.md`. That keeps every rule below checkable with no game,
//! no wasm and no prepared resources.
//!
//! **What is deliberately absent**, each with its cost:
//!
//! - **the random draw.** Vanilla picks a candidate chunk per `spacing`-sized region
//!   with a salted `RandomSource`; a wasm mod cannot replay it, so the caller hands
//!   over the candidates. [`spread`] ports the *geometry* that draw has to obey.
//! - **the jigsaw.** `JigsawPlacement.addPieces` (`EmptyColonyStructure.java:133-142`)
//!   walks the template pool; the port keeps the box the generator promises
//!   (`size`, `max_distance_from_center`) and nothing inside it.
//! - **the paste.** No host call is made here at all.

mod biomes;
mod site;
mod spread;
mod terrain;

pub use biomes::{BiomeTag, Biomes, Structure, TagTable};
pub use site::{choose, middle, Chosen, Pass, Question, WorldFacts};
pub use spread::{set_of, Set, Spread, CAMP_SETS, COLONY_SET, SHIP_SET};
pub use terrain::{
    cave_site, start_y, surface_ok, Adaptation, Air, Column, Form, CAVE_AIR, CAVE_PROBES,
    CAVE_STEP, MAX_LAND_HEIGHT,
};

/// Why one candidate site was not kept — named.
///
/// Every variant says which rule refused it in its own doc. Three of them are
/// Java's or vanilla's, one is the port's ([`Self::TooSteep`], off by default), and
/// the doc of each names its source.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// The caller gave no column for the chunk's middle block. Absent is not
    /// "empty ground": a site nobody surveyed cannot be accepted.
    NotSurveyed { chunk: [i32; 2] },
    /// The biome is not in the structure's `has_structure` tag, so the generator
    /// never even calls `findGenerationPoint` for it.
    ///
    /// Vanilla `Structure.generate` tests `StructureSettings.biomes` before the
    /// structure type's own code; in this tree the tag is the `biomes` field of
    /// `worldgen/structure/colonial_colony.json:9` and its contents are
    /// `tags/worldgen/biome/has_structure/colonial_colony.json:4-26`.
    BiomeCannotHost { structure: Structure, biome: String },
    /// The top block of the column holds a fluid — water or lava.
    ///
    /// `EmptyColonyStructure.java:87-89`: `topBlock.getFluidState().isEmpty()` has
    /// to be **true**, so the structure is refused at any column whose surface is
    /// a fluid.
    SurfaceIsFluid { at: [i32; 3] },
    /// The surface is at or above 200 blocks.
    ///
    /// `EmptyColonyStructure.java:89`: `landHeight < 200` is the second half of
    /// the same test, and it is a **fixed bound, not the level's height** — a
    /// world whose build height ends at 160 still refuses nothing on that ground,
    /// and an amplified world refuses every peak from 200 up.
    LandTooHigh { top: i32, limit: i32 },
    /// The structure asks for a cave (`allow_cave`, set by exactly one file:
    /// `worldgen/structure/warpednetherlands_colony.json:16`) and the ten probes
    /// of `EmptyColonyStructure.java:151-213` found no pocket of 32 air blocks.
    NoCavePocket { chunk: [i32; 2] },
    /// Another structure already starts in a chunk the colony's box would cover.
    ///
    /// Vanilla's `StructureCheck` refuses a piece that intersects a known start;
    /// the port receives the starts as a fact and asks the same question. Java
    /// itself never checks this — the nether colony relies on it, because its
    /// `allow_cave` path is the only one that can land inside a fortress.
    StructureInTheWay { chunk: [i32; 2], holds: [i32; 2] },
    /// The ground under the colony is too uneven for the box.
    ///
    /// **A rule of this port**, and off by default: Java has no slope test at all —
    /// it asks the generator for `terrain_adaptation: "beard_box"`
    /// (`worldgen/structure/colonial_colony.json:14`) and lets the adaptation fill
    /// the ground under the pieces. A server that wants a flat-site rule turns it
    /// on with [`site::Question::max_step`].
    TooSteep {
        from: [i32; 2],
        to: [i32; 2],
        step: i32,
        limit: i32,
    },
    /// A colony is nearer than `founding::Rules::block_range()`
    /// (`ColonyManager.java:311-320`, the same rule the founding applies — this
    /// module calls it rather than re-deriving it).
    TooCloseToColony {
        id: i32,
        distance: i64,
        required: i64,
    },
}

impl Refusal {
    /// The variant's own name, namespaced so it cannot be mistaken for a sentence.
    pub fn name(&self) -> &'static str {
        match self {
            Self::NotSurveyed { .. } => "worldgen.refusal.not_surveyed",
            Self::BiomeCannotHost { .. } => "worldgen.refusal.biome_cannot_host",
            Self::SurfaceIsFluid { .. } => "worldgen.refusal.surface_is_fluid",
            Self::LandTooHigh { .. } => "worldgen.refusal.land_too_high",
            Self::NoCavePocket { .. } => "worldgen.refusal.no_cave_pocket",
            Self::StructureInTheWay { .. } => "worldgen.refusal.structure_in_the_way",
            Self::TooSteep { .. } => "worldgen.refusal.too_steep",
            Self::TooCloseToColony { .. } => "worldgen.refusal.too_close_to_colony",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

/// Why **no** candidate was kept: one [`Refusal`] per site, in the order they were
/// tried.
///
/// A single refusal would hide the interesting case — the first candidate is wet
/// and the second one is too near a colony — so the whole list is kept. It is not a
/// variant of [`Refusal`] because "none of these" is a fact about the search, not
/// about a site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusals {
    /// The structure the search was for.
    pub structure: Structure,
    /// Every site tried, and why it was refused. Empty when the caller supplied no
    /// candidate at all.
    pub sites: Vec<([i32; 2], Refusal)>,
}

impl Refusals {
    /// The name of the failure, namespaced like a [`Refusal`]'s.
    pub fn name(&self) -> &'static str {
        "worldgen.refusal.no_acceptable_site"
    }

    /// The refusal of the first site tried, which is the one a player watching one
    /// spot would have seen first.
    pub fn first(&self) -> Option<&Refusal> {
        self.sites.first().map(|(_, refusal)| refusal)
    }
}

impl std::fmt::Display for Refusals {
    /// The name, then one `chunk: reason` pair per site — a log line that said
    /// only "no site" would hide which rule rejected which chunk.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name(), self.structure.id())?;
        for (chunk, refusal) in &self.sites {
            write!(f, " [{}, {}]: {refusal}", chunk[0], chunk[1])?;
        }
        Ok(())
    }
}

impl std::error::Error for Refusals {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_refusal_has_its_own_name() {
        let all = [
            Refusal::NotSurveyed { chunk: [1, 2] },
            Refusal::BiomeCannotHost {
                structure: Structure::Colonial,
                biome: "minecraft:the_end".to_owned(),
            },
            Refusal::SurfaceIsFluid { at: [8, 62, 8] },
            Refusal::LandTooHigh {
                top: 200,
                limit: MAX_LAND_HEIGHT,
            },
            Refusal::NoCavePocket { chunk: [0, 0] },
            Refusal::StructureInTheWay {
                chunk: [3, 4],
                holds: [3, 4],
            },
            Refusal::TooSteep {
                from: [0, 0],
                to: [1, 0],
                step: 12,
                limit: 4,
            },
            Refusal::TooCloseToColony {
                id: 2,
                distance: 40,
                required: 128,
            },
        ];
        let mut names: Vec<&str> = all.iter().map(Refusal::name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), all.len(), "two refusals share a name");
        for refusal in &all {
            assert_eq!(refusal.to_string(), refusal.name(), "Display is the name");
            assert!(
                refusal.name().starts_with("worldgen.refusal."),
                "a name has to be namespaced: {}",
                refusal.name()
            );
        }
    }

    #[test]
    fn the_search_refusal_names_every_site_it_tried() {
        let none = Refusals {
            structure: Structure::WarpedNetherlands,
            sites: Vec::new(),
        };
        assert_eq!(none.name(), "worldgen.refusal.no_acceptable_site");
        assert_eq!(none.to_string(), "worldgen.refusal.no_acceptable_site (minecolonies:warpednetherlands_colony)");
        assert_eq!(none.first(), None, "nothing was tried");

        let some = Refusals {
            structure: Structure::Colonial,
            sites: vec![
                ([1, 2], Refusal::SurfaceIsFluid { at: [24, 62, 40] }),
                (
                    [3, 4],
                    Refusal::TooCloseToColony {
                        id: 2,
                        distance: 40,
                        required: 128,
                    },
                ),
            ],
        };
        assert_eq!(some.first(), Some(&Refusal::SurfaceIsFluid { at: [24, 62, 40] }));
        assert_eq!(
            some.to_string(),
            "worldgen.refusal.no_acceptable_site (minecolonies:colonial_colony) \
             [1, 2]: worldgen.refusal.surface_is_fluid \
             [3, 4]: worldgen.refusal.too_close_to_colony"
        );
    }

    #[test]
    fn a_refusal_is_an_error_so_a_caller_can_use_the_question_mark() {
        fn assert_error<E: std::error::Error>(_: &E) {}
        assert_error(&Refusal::NotSurveyed { chunk: [0, 0] });
        assert_error(&Refusals {
            structure: Structure::Ship,
            sites: Vec::new(),
        });
    }
}
