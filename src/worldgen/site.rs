//! Of the candidate sites, which one is kept — and why the others are not.
//!
//! **This is the port's own decision, and it says so.** Java has no "choose a site"
//! step for a colony: the generator picks one candidate per region and
//! `EmptyColonyStructure` accepts it or not, in one order (`isFeatureChunk` at
//! `EmptyColonyStructure.java:80-90`, called from `findGenerationPoint` at
//! `:121-124`); a player-found colony is placed by a click
//! (`CreateColonyMessage.java:104-201`, ported in `crate::founding`). What the port
//! adds is the loop around those rules: a list of candidates in the caller's order,
//! the five questions below, and one [`Chosen`] that carries the measurements that
//! justified it.
//!
//! The order is Java's where Java has one, and is stated where it does not:
//!
//! | # | Question | Source | In Java's order? |
//! | --- | --- | --- | --- |
//! | 1 | is the biome in the structure's tag | vanilla `Structure.generate` → `StructureSettings.biomes` | yes, first |
//! | 2 | is the surface dry and under 200 | `EmptyColonyStructure.java:84-89` | yes, inside `findGenerationPoint` |
//! | 2′ | or, with `allow_cave`, is there a 32-air pocket | `:95-119`, `:151-213` | it **replaces** 2 |
//! | 3 | does another structure start inside the box | vanilla `StructureCheck` | yes, before pieces are placed |
//! | 4 | is the ground gentle enough | **the port's**, off by default | no — see [`super::Refusal::TooSteep`] |
//! | 5 | is a colony far enough | `ColonyManager.java:311-320`, called through `crate::founding::Rules` | not for a generated site; kept anyway |
//!
//! **The facts are the caller's.** [`WorldFacts`] holds the surveyed columns, the
//! chunk coordinates where a structure already starts, the level's bounds and the
//! random draws the cave probe needs; [`Question::air`] is the block source the
//! probe walks. Nothing here calls a host, which is what lets the whole table above
//! be tested with structs — and what makes the host calls the port still needs a
//! list of their own (`src/worldgen/MANQUANT.md`).
//!
//! **A refused candidate is not a failed search.** [`choose`] keeps every refusal,
//! in the order it tried the candidates, and answers [`Refusals`]: the first
//! candidate is wet, the second one is too near a colony, and a caller that got
//! back a single "no" would have to survey again to learn which rule bit.

use crate::base::BLOCKS_PER_CHUNK;
use crate::founding::{Colonies, Rules};

use super::biomes::{Biomes, Structure};
use super::terrain::{cave_site, start_y, surface_ok, Air, Column, Form};
use super::{Refusal, Refusals};

/// What a site is chosen from: the survey, the starts, the bounds, the draws.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorldFacts {
    /// The dimension's registry name, compared with the colonies' own
    /// (`crate::founding::Colonies::closest` skips another dimension's colony).
    pub dimension: String,
    /// `LevelHeightAccessor.getMinY` (`level_height().0`).
    pub min_y: i32,
    /// The build height (`level_height().1`): the cave probe climbs at most this far.
    pub height: i32,
    /// The columns the caller surveyed, in any order.
    pub columns: Vec<Column>,
    /// The chunks that already hold a structure start.
    pub structures: Vec<[i32; 2]>,
    /// The cave probe's `nextInt(0, 30)` draws, in order; a missing one is `0`.
    pub draws: Vec<i32>,
}

impl WorldFacts {
    /// No column, no start, no draw.
    pub fn new(dimension: &str, min_y: i32, height: i32) -> WorldFacts {
        WorldFacts {
            dimension: dimension.to_owned(),
            min_y,
            height,
            columns: Vec::new(),
            structures: Vec::new(),
            draws: Vec::new(),
        }
    }

    /// Adds a surveyed column.
    pub fn survey(&mut self, column: Column) -> &mut WorldFacts {
        self.columns.push(column);
        self
    }

    /// Records that a structure already starts in that chunk.
    pub fn occupied(&mut self, chunk: [i32; 2]) -> &mut WorldFacts {
        self.structures.push(chunk);
        self
    }

    /// The column surveyed at that block column, if any.
    pub fn column(&self, at: [i32; 2]) -> Option<&Column> {
        self.columns.iter().find(|column| column.at == at)
    }
}

/// The block column at the middle of a chunk: vanilla's
/// `ChunkPos.getMiddleBlockPosition(0)`, the position `findGenerationPoint` starts
/// from (`EmptyColonyStructure.java:127`) before it replaces the y.
pub fn middle(chunk: [i32; 2]) -> [i32; 2] {
    [
        chunk[0] * BLOCKS_PER_CHUNK + BLOCKS_PER_CHUNK / 2,
        chunk[1] * BLOCKS_PER_CHUNK + BLOCKS_PER_CHUNK / 2,
    ]
}

/// One rule that passed, with the number that let it pass: the *why* of a
/// [`Chosen`].
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the five questions of the table at the top of this file
pub enum Pass {
    /// The biome is in the structure's `has_structure` tag.
    Biome,
    /// The surface is dry and under 200, at that height.
    Surface { top: i32 },
    /// The ground is dry *because* the pieces hang in a cave, at that position.
    Cave { at: [i32; 3] },
    /// No structure start is inside the box the pieces may cover.
    ClearOfStructures,
    /// The worst step between two adjacent surveyed columns, when the caller asked
    /// for the port's slope rule and the survey had two adjacent columns.
    Gentle { worst_step: i32 },
    /// The nearest colony in this dimension, if any, and how far it is.
    FarEnough { nearest: Option<(i32, i64)> },
}

impl Pass {
    /// The rule's name, namespaced like the refusals', for a log line.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Biome => "worldgen.pass.biome",
            Self::Surface { .. } => "worldgen.pass.surface",
            Self::Cave { .. } => "worldgen.pass.cave",
            Self::ClearOfStructures => "worldgen.pass.clear_of_structures",
            Self::Gentle { .. } => "worldgen.pass.gentle",
            Self::FarEnough { .. } => "worldgen.pass.far_enough",
        }
    }
}

/// The site that was kept, and every measurement that kept it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chosen {
    /// The structure being placed.
    pub structure: Structure,
    /// The candidate chunk.
    pub chunk: [i32; 2],
    /// Where the pieces attach: `[middle.x, start_y, middle.z]` on the surface,
    /// the pocket's own position in a cave.
    pub at: [i32; 3],
    /// The biome of the surveyed column, as the caller reported it.
    pub biome: String,
    /// The surface height of that column (`getFirstOccupiedHeight`).
    pub top: i32,
    /// The rules that passed, in the order they were asked.
    pub why: Vec<Pass>,
}

impl Chosen {
    /// Whether the site was accepted through the cave path.
    pub fn is_cave(&self) -> bool {
        self.why.iter().any(|pass| matches!(pass, Pass::Cave { .. }))
    }
}

/// One search: what to place, where it may go, and what the world says.
pub struct Question<'a> {
    /// The structure to place.
    pub structure: Structure,
    /// The candidate chunks, in the order they are tried.
    pub candidates: &'a [[i32; 2]],
    /// The survey, the starts, the bounds, the draws.
    pub facts: &'a WorldFacts,
    /// What the cave probe reads: `crate::ai::Blocks` in a real call, a fake in a
    /// test.
    pub air: &'a dyn Air,
    /// The level's biome tags.
    pub biomes: &'a dyn Biomes,
    /// The colonies that already exist.
    pub colonies: &'a dyn Colonies,
    /// The founding distances, read for `block_range`
    /// (`ColonyManager.java:311-320`).
    pub rules: Rules,
    /// The port's slope rule: the largest step allowed between two adjacent
    /// surveyed columns. `None` is Java's own answer, and the default.
    pub max_step: Option<i32>,
}

impl<'a> Question<'a> {
    /// A question with Java's defaults: [`Rules::DEFAULT`] and no slope rule.
    pub fn new(
        structure: Structure,
        candidates: &'a [[i32; 2]],
        facts: &'a WorldFacts,
        air: &'a dyn Air,
        biomes: &'a dyn Biomes,
        colonies: &'a dyn Colonies,
    ) -> Question<'a> {
        Question {
            structure,
            candidates,
            facts,
            air,
            biomes,
            colonies,
            rules: Rules::DEFAULT,
            max_step: None,
        }
    }

    /// The same question with a server's own distances.
    pub fn with_rules(mut self, rules: Rules) -> Question<'a> {
        self.rules = rules;
        self
    }

    /// Turns the port's slope rule on: a step above `max_step` blocks between two
    /// adjacent surveyed columns refuses the site.
    pub fn with_max_step(mut self, max_step: i32) -> Question<'a> {
        self.max_step = Some(max_step);
        self
    }
}

/// The first candidate that passes every question, or one [`Refusal`] per
/// candidate.
///
/// The candidate order is the caller's, and the first acceptable site wins: nothing
/// scores or compares two acceptable sites, because Java never does — the generator
/// has one candidate per region and the others do not exist.
pub fn choose(question: &Question<'_>) -> Result<Chosen, Refusals> {
    let form = Form::of(question.structure);
    let mut sites: Vec<([i32; 2], Refusal)> = Vec::new();

    for chunk in question.candidates {
        let chunk = *chunk;
        let at = middle(chunk);
        let Some(column) = question.facts.column(at) else {
            sites.push((chunk, Refusal::NotSurveyed { chunk }));
            continue;
        };

        // 1. The biome, which vanilla tests before the structure type is called.
        if !question.structure.hosts(&column.biome, question.biomes) {
            sites.push((
                chunk,
                Refusal::BiomeCannotHost {
                    structure: question.structure,
                    biome: column.biome.clone(),
                },
            ));
            continue;
        }

        // 2 and 2′: the surface gate, or the cave probe that replaces it.
        let mut why = vec![Pass::Biome];
        let anchor;
        if form.allow_cave {
            match cave_site(
                question.air,
                chunk,
                question.facts.min_y,
                question.facts.height,
                &question.facts.draws,
            ) {
                Err(refusal) => {
                    sites.push((chunk, refusal));
                    continue;
                }
                Ok(None) => {
                    sites.push((chunk, Refusal::NoCavePocket { chunk }));
                    continue;
                }
                Ok(Some(pocket)) => {
                    why.push(Pass::Cave { at: pocket });
                    anchor = pocket;
                }
            }
        } else {
            if let Err(refusal) = surface_ok(column) {
                sites.push((chunk, refusal));
                continue;
            }
            why.push(Pass::Surface { top: column.top });
            anchor = [at[0], start_y(column), at[1]];
        }

        // 3. Nothing may start inside the box the pieces can cover.
        let reach = form.reach_chunks();
        if let Some(holds) = question
            .facts
            .structures
            .iter()
            .copied()
            .find(|start| {
                (start[0] - chunk[0]).abs() <= reach && (start[1] - chunk[1]).abs() <= reach
            })
        {
            sites.push((chunk, Refusal::StructureInTheWay { chunk, holds }));
            continue;
        }
        why.push(Pass::ClearOfStructures);

        // 4. The port's slope rule, only when the caller asked for it.
        if let Some(limit) = question.max_step {
            if let Some((from, to, step)) = worst_slope(question.facts, chunk, reach) {
                if step > limit {
                    sites.push((
                        chunk,
                        Refusal::TooSteep {
                            from,
                            to,
                            step,
                            limit,
                        },
                    ));
                    continue;
                }
                why.push(Pass::Gentle { worst_step: step });
            }
        }

        // 5. The distance a founding would ask for, asked of a generated site too.
        let nearest = question.colonies.closest(&question.facts.dimension, anchor);
        let mut nearest_fact = None;
        if let Some(colony) = nearest {
            let distance = Rules::distance(anchor, colony.center);
            let required = question.rules.block_range();
            if distance < required {
                sites.push((
                    chunk,
                    Refusal::TooCloseToColony {
                        id: colony.id,
                        distance,
                        required,
                    },
                ));
                continue;
            }
            nearest_fact = Some((colony.id, distance));
        }
        why.push(Pass::FarEnough {
            nearest: nearest_fact,
        });

        return Ok(Chosen {
            structure: question.structure,
            chunk,
            at: anchor,
            biome: column.biome.clone(),
            top: column.top,
            why,
        });
    }

    Err(Refusals {
        structure: question.structure,
        sites,
    })
}

/// The largest height difference between two surveyed columns one block apart, in
/// the chunks the structure's pieces may cover.
///
/// Adjacent means a Chebyshev distance of exactly **one**, so a diagonal neighbour
/// counts: `beard_box` fills under the pieces on both axes, and a step across a
/// corner is a real step. Only the corner of a pair is kept, in survey order, so the
/// refusal names the same two columns on every run.
fn worst_slope(facts: &WorldFacts, chunk: [i32; 2], reach: i32) -> Option<([i32; 2], [i32; 2], i32)> {
    let covered = |at: [i32; 2]| {
        let cell = [
            at[0].div_euclid(BLOCKS_PER_CHUNK),
            at[1].div_euclid(BLOCKS_PER_CHUNK),
        ];
        (cell[0] - chunk[0]).abs() <= reach && (cell[1] - chunk[1]).abs() <= reach
    };
    let columns: Vec<&Column> = facts.columns.iter().filter(|column| covered(column.at)).collect();
    let mut worst: Option<([i32; 2], [i32; 2], i32)> = None;
    for (index, first) in columns.iter().enumerate() {
        for second in &columns[index + 1..] {
            let dx = (first.at[0] - second.at[0]).abs();
            let dz = (first.at[1] - second.at[1]).abs();
            if dx.max(dz) != 1 {
                continue;
            }
            let step = (first.top - second.top).abs();
            if worst.is_none_or(|(_, _, worst)| step > worst) {
                worst = Some((first.at, second.at, step));
            }
        }
    }
    worst
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colony::Colony;
    use crate::founding::Registry;

    use super::super::biomes::TagTable;

    const OVERWORLD: &str = "minecraft:overworld";
    const NETHER: &str = "minecraft:the_nether";

    /// The tags a vanilla-shaped world answers, for the biomes these tests use.
    fn world_biomes() -> TagTable {
        TagTable::new()
            .tag("minecraft:plains", "forge:is_plains")
            .tag("minecraft:forest", "minecraft:is_forest")
    }

    /// A flat plain surveyed at every candidate's middle, and nothing else.
    fn facts_for(candidates: &[[i32; 2]], biome: &str, top: i32) -> WorldFacts {
        let mut facts = WorldFacts::new(OVERWORLD, -64, 384);
        for chunk in candidates {
            facts.survey(Column::new(middle(*chunk), biome, top, top + 1, false));
        }
        facts
    }

    /// No colony at all.
    fn empty() -> Registry {
        Registry::new()
    }

    /// A world where every cell is solid: the cave probe finds nothing.
    struct Solid;

    impl Air for Solid {
        fn is_air(&self, _at: [i32; 3]) -> bool {
            false
        }
    }

    /// A world that is solid everywhere except the cells named air.
    struct Pocket(Vec<[i32; 3]>);

    impl Air for Pocket {
        fn is_air(&self, at: [i32; 3]) -> bool {
            self.0.contains(&at)
        }
    }

    /// The probes of the nether colony, in a world with no colony.
    fn nether_question<'a>(
        candidates: &'a [[i32; 2]],
        facts: &'a WorldFacts,
        air: &'a dyn Air,
        biomes: &'a dyn Biomes,
        colonies: &'a dyn Colonies,
    ) -> Question<'a> {
        Question::new(
            Structure::WarpedNetherlands,
            candidates,
            facts,
            air,
            biomes,
            colonies,
        )
    }

    #[test]
    fn the_first_candidate_wins_and_the_why_carries_every_measurement() {
        let candidates = [[0, 0], [1, 0]];
        let facts = facts_for(&candidates, "minecraft:plains", 63);
        let biomes = world_biomes();
        let colonies = empty();
        let question = Question::new(
            Structure::Colonial,
            &candidates,
            &facts,
            &Solid,
            &biomes,
            &colonies,
        );
        let chosen = choose(&question).expect("a plain at y=63");
        assert_eq!(chosen.structure, Structure::Colonial);
        assert_eq!(chosen.chunk, [0, 0], "the first candidate, not the closest");
        assert_eq!(chosen.at, [8, 65, 8], "middle of chunk 0, free height + 1");
        assert_eq!(chosen.biome, "minecraft:plains");
        assert_eq!(chosen.top, 63);
        assert!(!chosen.is_cave());
        assert_eq!(
            chosen.why,
            vec![
                Pass::Biome,
                Pass::Surface { top: 63 },
                Pass::ClearOfStructures,
                Pass::FarEnough { nearest: None },
            ],
            "no slope rule was asked for, so no Gentle pass"
        );
        assert_eq!(Pass::Biome.name(), "worldgen.pass.biome");
        assert!(chosen.why.iter().all(|pass| pass.name().starts_with("worldgen.pass.")));
    }

    #[test]
    fn a_wet_candidate_is_refused_and_the_next_dry_one_is_kept() {
        let candidates = [[0, 0], [1, 0]];
        let mut facts = facts_for(&candidates, "minecraft:plains", 62);
        facts.columns[0].fluid = true;
        let biomes = world_biomes();
        let colonies = empty();
        let question = Question::new(
            Structure::Colonial,
            &candidates,
            &facts,
            &Solid,
            &biomes,
            &colonies,
        );
        let chosen = choose(&question).expect("the second chunk is dry");
        assert_eq!(chosen.chunk, [1, 0]);
        assert_eq!(chosen.at, [24, 64, 8], "free height 63, then `.above`");
    }

    #[test]
    fn when_nothing_is_kept_every_site_is_named_in_the_order_it_was_tried() {
        let candidates = [[0, 0], [1, 0], [2, 0]];
        let mut facts = facts_for(&candidates, "minecraft:plains", 62);
        facts.columns[0].fluid = true;
        facts.columns[1].top = 200;
        // The third one is never surveyed: it is a candidate with no column.
        facts.columns.remove(2);
        let biomes = world_biomes();
        let colonies = empty();
        let question = Question::new(
            Structure::Colonial,
            &candidates,
            &facts,
            &Solid,
            &biomes,
            &colonies,
        );
        let refused = choose(&question).expect_err("nothing passes");
        assert_eq!(refused.structure, Structure::Colonial);
        assert_eq!(refused.sites.len(), 3);
        assert_eq!(
            refused.sites[0].1,
            Refusal::SurfaceIsFluid { at: [8, 62, 8] }
        );
        assert_eq!(
            refused.sites[1].1,
            Refusal::LandTooHigh {
                top: 200,
                limit: 200
            }
        );
        assert_eq!(refused.sites[2].1, Refusal::NotSurveyed { chunk: [2, 0] });
        assert_eq!(refused.first(), Some(&refused.sites[0].1));
        assert!(refused.to_string().contains("[2, 0]"));
    }

    #[test]
    fn a_biome_outside_the_structures_tag_is_refused_before_the_ground() {
        let candidates = [[0, 0]];
        let facts = facts_for(&candidates, "minecraft:desert", 62);
        let biomes = world_biomes();
        let colonies = empty();
        let question = Question::new(
            Structure::Colonial,
            &candidates,
            &facts,
            &Solid,
            &biomes,
            &colonies,
        );
        assert_eq!(
            choose(&question).expect_err("a desert hosts no colonial colony"),
            Refusals {
                structure: Structure::Colonial,
                sites: vec![(
                    [0, 0],
                    Refusal::BiomeCannotHost {
                        structure: Structure::Colonial,
                        biome: "minecraft:desert".to_owned()
                    }
                )]
            }
        );
    }

    #[test]
    fn an_empty_candidate_list_is_a_named_refusal_with_no_site() {
        let facts = WorldFacts::new(OVERWORLD, -64, 384);
        let biomes = world_biomes();
        let colonies = empty();
        let candidates: [[i32; 2]; 0] = [];
        let question = Question::new(
            Structure::Colonial,
            &candidates,
            &facts,
            &Solid,
            &biomes,
            &colonies,
        );
        let refused = choose(&question).expect_err("nothing to try");
        assert!(refused.sites.is_empty());
        assert_eq!(refused.name(), "worldgen.refusal.no_acceptable_site");
        assert_eq!(refused.first(), None);
    }

    #[test]
    fn a_structure_start_inside_the_box_is_refused_and_one_outside_it_is_not() {
        // A colony reaches 80 blocks = 5 chunks from its anchor.
        assert_eq!(Form::of(Structure::Colonial).reach_chunks(), 5);
        let candidates = [[0, 0]];
        let biomes = world_biomes();
        let colonies = empty();

        let mut inside = facts_for(&candidates, "minecraft:plains", 63);
        inside.occupied([2, 0]);
        let question = Question::new(
            Structure::Colonial,
            &candidates,
            &inside,
            &Solid,
            &biomes,
            &colonies,
        );
        assert_eq!(
            choose(&question).expect_err("a start two chunks away"),
            Refusals {
                structure: Structure::Colonial,
                sites: vec![(
                    [0, 0],
                    Refusal::StructureInTheWay {
                        chunk: [0, 0],
                        holds: [2, 0]
                    }
                )]
            }
        );

        let mut outside = facts_for(&candidates, "minecraft:plains", 63);
        outside.occupied([6, 0]);
        let question = Question::new(
            Structure::Colonial,
            &candidates,
            &outside,
            &Solid,
            &biomes,
            &colonies,
        );
        assert!(
            choose(&question).is_ok(),
            "six chunks away is outside the five-chunk reach"
        );
    }

    #[test]
    fn a_colony_at_exactly_the_founding_range_does_not_block_the_site() {
        let candidates = [[0, 0]];
        let facts = facts_for(&candidates, "minecraft:plains", 63);
        let biomes = world_biomes();
        let required = Rules::DEFAULT.block_range();
        assert_eq!(required, 128);

        // The site's anchor is [8, 65, 8]; a colony 128 blocks north is the edge.
        let mut edge = Registry::new();
        edge.insert(Colony::new(2, "Edge", OVERWORLD, [8, 65, 8 + 128], 3));
        let question = Question::new(
            Structure::Colonial,
            &candidates,
            &facts,
            &Solid,
            &biomes,
            &edge,
        );
        let chosen = choose(&question).expect("128 blocks is not nearer than 128");
        assert_eq!(
            chosen.why.last(),
            Some(&Pass::FarEnough {
                nearest: Some((2, 128))
            })
        );

        let mut nearer = Registry::new();
        nearer.insert(Colony::new(2, "Edge", OVERWORLD, [8, 65, 8 + 127], 3));
        let question = Question::new(
            Structure::Colonial,
            &candidates,
            &facts,
            &Solid,
            &biomes,
            &nearer,
        );
        assert_eq!(
            choose(&question).expect_err("127 blocks is too near"),
            Refusals {
                structure: Structure::Colonial,
                sites: vec![(
                    [0, 0],
                    Refusal::TooCloseToColony {
                        id: 2,
                        distance: 127,
                        required: 128
                    }
                )]
            }
        );
    }

    #[test]
    fn a_colony_in_another_dimension_does_not_count() {
        let candidates = [[0, 0]];
        let facts = facts_for(&candidates, "minecraft:plains", 63);
        let biomes = world_biomes();
        let mut colonies = Registry::new();
        colonies.insert(Colony::new(2, "Nether", NETHER, [8, 65, 8], 3));
        let question = Question::new(
            Structure::Colonial,
            &candidates,
            &facts,
            &Solid,
            &biomes,
            &colonies,
        );
        assert!(
            choose(&question).is_ok(),
            "a nether colony is not in the overworld's way"
        );
    }

    #[test]
    fn the_slope_rule_is_off_by_default_and_names_the_two_columns_when_it_bites() {
        let candidates = [[0, 0]];
        let mut facts = facts_for(&candidates, "minecraft:plains", 63);
        // A cliff one block east of the middle: 63 against 70.
        facts.survey(Column::new([9, 8], "minecraft:plains", 70, 71, false));
        let biomes = world_biomes();
        let colonies = empty();

        let off = Question::new(
            Structure::Colonial,
            &candidates,
            &facts,
            &Solid,
            &biomes,
            &colonies,
        );
        assert!(choose(&off).is_ok(), "Java has no slope rule, so no refusal");
        assert!(
            !choose(&off).expect("a site").why.iter().any(|pass| matches!(pass, Pass::Gentle { .. })),
            "and no Gentle pass either: the rule was not asked"
        );

        let on = Question::new(
            Structure::Colonial,
            &candidates,
            &facts,
            &Solid,
            &biomes,
            &colonies,
        )
        .with_max_step(4);
        assert_eq!(
            choose(&on).expect_err("a seven-block step"),
            Refusals {
                structure: Structure::Colonial,
                sites: vec![(
                    [0, 0],
                    Refusal::TooSteep {
                        from: [8, 8],
                        to: [9, 8],
                        step: 7,
                        limit: 4
                    }
                )]
            }
        );

        // A limit the cliff respects is a Gentle pass carrying the measured step.
        let gentle = Question::new(
            Structure::Colonial,
            &candidates,
            &facts,
            &Solid,
            &biomes,
            &colonies,
        )
        .with_max_step(7);
        assert_eq!(
            choose(&gentle).expect("seven is allowed").why,
            vec![
                Pass::Biome,
                Pass::Surface { top: 63 },
                Pass::ClearOfStructures,
                Pass::Gentle { worst_step: 7 },
                Pass::FarEnough { nearest: None },
            ]
        );
    }

    #[test]
    fn the_nether_colony_needs_a_pocket_and_ignores_the_surface_gate() {
        let candidates = [[0, 0]];
        // A wet, 250-block column: the surface gate would refuse it, and Java never
        // runs the surface gate when `allow_cave` is set (`:95-119`).
        let mut facts = facts_for(&candidates, "minecraft:warped_forest", 250);
        facts.columns[0].fluid = true;
        facts.draws = vec![61];
        let biomes = TagTable::new();
        let colonies = empty();

        let empty_world = nether_question(&candidates, &facts, &Solid, &biomes, &colonies);
        assert_eq!(
            choose(&empty_world).expect_err("no pocket"),
            Refusals {
                structure: Structure::WarpedNetherlands,
                sites: vec![([0, 0], Refusal::NoCavePocket { chunk: [0, 0] })]
            }
        );

        // Thirty air blocks at y 61..=90, at the probed corner (0, 0).
        let pocket: Vec<[i32; 3]> = (0..30).map(|step| [0, 61 + step, 0]).collect();
        let with_pocket = Pocket(pocket);
        let question = nether_question(&candidates, &facts, &with_pocket, &biomes, &colonies);
        let chosen = choose(&question).expect("a pocket");
        assert!(chosen.is_cave());
        assert_eq!(chosen.at, [0, 90, 0], "the probe's own position");
        assert_eq!(
            chosen.why,
            vec![
                Pass::Biome,
                Pass::Cave { at: [0, 90, 0] },
                Pass::ClearOfStructures,
                Pass::FarEnough { nearest: None },
            ],
            "the surface height is not in the why: it was never read"
        );
    }

    #[test]
    fn a_servers_own_founding_range_refuses_a_site_the_defaults_would_accept() {
        let candidates = [[0, 0]];
        let facts = facts_for(&candidates, "minecraft:plains", 63);
        let biomes = world_biomes();
        let mut colonies = Registry::new();
        colonies.insert(Colony::new(2, "Near", OVERWORLD, [8, 65, 8 + 100], 3));
        // A server that keeps colonies 16 chunks apart: 256 blocks, so 100 is too
        // near, where the defaults (128) would have accepted it.
        let rules = Rules {
            min_colony_distance: 16,
            ..Rules::DEFAULT
        };
        let question = Question::new(
            Structure::Colonial,
            &candidates,
            &facts,
            &Solid,
            &biomes,
            &colonies,
        )
        .with_rules(rules);
        assert_eq!(
            choose(&question).expect_err("100 < 256"),
            Refusals {
                structure: Structure::Colonial,
                sites: vec![(
                    [0, 0],
                    Refusal::TooCloseToColony {
                        id: 2,
                        distance: 100,
                        required: 256
                    }
                )]
            }
        );
    }
}
