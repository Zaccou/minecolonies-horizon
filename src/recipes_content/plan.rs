//! What one palette still needs before the pack can be placed: the ids to rewrite,
//! and the ids nobody knows yet — each with the cells it costs.
//!
//! # The question, and the one it is not
//!
//! [`crate::compat::states::audit`] asks *"can 26.3 express this exact state?"*,
//! property by property. This module asks the migration's question instead:
//! **which ids do I touch, and how much of the building moves with them?** So a
//! plan is keyed by **id** — one broken id is one line of work for a human — and
//! weighs each one by the cells it fills, which is the only order work can be
//! taken in. Properties are deliberately not read here: that is
//! `compat::states`' question, and keeping the two apart is what stops them from
//! disagreeing.
//!
//! | Bucket | Meaning | What a human does |
//! | --- | --- | --- |
//! | [`Verdict::Replace`] | the table renames it | nothing: it is applied mechanically |
//! | [`Verdict::Kept`] | the table is silent and the build knows it | nothing |
//! | [`Verdict::Unknown`] | the table is silent **and the build does not know it** | find what it became, or drop it |
//! | [`Verdict::Blocked`] | the table itself has no 26.3 name | a judgement call, not a rewrite |
//! | [`Verdict::Foreign`] | a mod's namespace | content a mod must bring |
//!
//! [`plan_blueprint`] weighs by cells, [`plan_palette`] weighs every entry once,
//! and [`plan_ids`] is the same walk over a plain list of ids. One implementation:
//! a second copy of this rule is how two answers start to differ.

use std::collections::BTreeMap;

use crate::blueprint::{Blueprint, StateRef};
use crate::compat::{Resolution, StateOracle};
use crate::recipes_content::renames::{self, Target};

/// The one question a plan asks about 26.3: is this id known to the build?
///
/// It is [`StateOracle::knows`], borrowed rather than re-declared, so a caller
/// that already has a `compat::Report` — or the game's block report — needs no
/// adapter. The blanket impl below is that bridge and adds nothing else.
pub trait Registry {
    /// Does 26.3 hold an id of this name?
    fn knows(&self, id: &str) -> bool;
}

impl<T: StateOracle> Registry for T {
    fn knows(&self, id: &str) -> bool {
        <T as StateOracle>::knows(self, id)
    }
}

/// Why an id the table mentions cannot be rewritten by a machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blocked {
    /// The table proves there is no 26.3 block: a substitute has to be chosen.
    Gone,
    /// The table has no proven name either way. Never guessed into a rewrite.
    Unproven,
}

/// What a plan says about one id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The table renames it; `to` is the name to write. `since` and `source` are
    /// the compat row's, so a report can cite them without a second lookup.
    Replace {
        to: &'static str,
        since: &'static str,
        source: &'static str,
    },
    /// The table has no row and the build knows the id as written.
    Kept,
    /// The table has no row and the build does not know the id: the one case a
    /// migration cannot decide alone.
    Unknown,
    /// The table has a row but no 26.3 name to offer.
    Blocked { why: Blocked },
    /// A namespace other than `minecraft`: a mod's own content.
    Foreign,
}

impl Verdict {
    /// Needs a human before the cell can be placed.
    pub fn needs_a_decision(&self) -> bool {
        matches!(self, Self::Unknown | Self::Blocked { .. })
    }
}

/// One id of the palette, everything a plan says about it, and what it costs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The id as the file writes it.
    pub id: String,
    /// Cells of the blueprint it fills, or references of a list — [`plan_ids`]
    /// counts occurrences, [`plan_palette`] counts one per entry.
    pub weight: u64,
    /// Palette indices holding it, ascending. Empty for [`plan_ids`], which has
    /// no palette.
    pub indices: Vec<usize>,
    pub verdict: Verdict,
}

/// The answer for a whole palette: one [`Entry`] per distinct id, in id order.
///
/// Id order is the map's, not the palette's, so two runs on the same pack print
/// the same report; [`Entry::indices`] keeps the palette order a rewrite needs.
#[derive(Clone, Debug, Default)]
pub struct Plan {
    pub entries: Vec<Entry>,
    /// Palette entries, or occurrences, walked.
    pub states: usize,
    /// Sum of every [`Entry::weight`].
    pub weight: u64,
    /// Cells whose `palette_index` points outside the palette: a malformed file,
    /// counted rather than silently dropped, because their state is unknown.
    pub unplaced: u64,
}

impl Plan {
    /// Every entry the table renames.
    pub fn replacements(&self) -> impl Iterator<Item = &Entry> {
        self.entries
            .iter()
            .filter(|entry| matches!(entry.verdict, Verdict::Replace { .. }))
    }

    /// Every vanilla id 26.3 does not know and the table does not rename.
    pub fn unknowns(&self) -> impl Iterator<Item = &Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.verdict == Verdict::Unknown)
    }

    /// Every id the table has a row for but no 26.3 name.
    pub fn blocked(&self) -> impl Iterator<Item = &Entry> {
        self.entries
            .iter()
            .filter(|entry| matches!(entry.verdict, Verdict::Blocked { .. }))
    }

    /// Every id a mod namespace has to provide.
    pub fn foreign(&self) -> impl Iterator<Item = &Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.verdict == Verdict::Foreign)
    }

    /// The entries a human has to look at: unknown or blocked.
    pub fn work(&self) -> impl Iterator<Item = &Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.verdict.needs_a_decision())
    }

    /// True when every id is either renamed or known: nothing is left to decide.
    /// The weight of the renames does not matter — they are applied mechanically.
    pub fn is_ready(&self) -> bool {
        self.work().next().is_none() && self.unplaced == 0
    }

    /// The palette rewrite, as `(index, new id)`, ascending by index — what a
    /// caller assigns into the palette it handed in.
    pub fn palette_edits(&self) -> Vec<(usize, &'static str)> {
        let mut edits = Vec::new();
        for entry in self.replacements() {
            if let Verdict::Replace { to, .. } = entry.verdict {
                for index in &entry.indices {
                    edits.push((*index, to));
                }
            }
        }
        edits.sort_by_key(|(index, _)| *index);
        edits
    }

    /// The work left, as one line, for a log or a report header.
    pub fn summary(&self) -> String {
        format!(
            "{} states, {} cells, {} to replace, {} unknown, {} blocked, {} foreign, {} unplaced",
            self.states,
            self.weight,
            self.replacements().count(),
            self.unknowns().count(),
            self.blocked().count(),
            self.foreign().count(),
            self.unplaced
        )
    }
}

/// What 26.3 says about one id, given the compat table and the build.
///
/// The order matters: the table is asked first because it is the only source of
/// a *proven* new name, and the build second because it can only answer "I know
/// this" — never "this used to be something else".
fn verdict(id: &str, oracle: &dyn Registry) -> Verdict {
    match renames::apply(Target::Block, id).resolution {
        Resolution::Renamed(row) => match row.to_26_3 {
            Some(to) => Verdict::Replace {
                to,
                since: row.since,
                source: row.source,
            },
            // A `Renamed` row without a target is a table bug; it is reported as
            // what it is rather than rewritten to the name it already had.
            None => Verdict::Blocked {
                why: Blocked::Unproven,
            },
        },
        Resolution::Gone(_) => Verdict::Blocked {
            why: Blocked::Gone,
        },
        Resolution::Uncertain(_) => Verdict::Blocked {
            why: Blocked::Unproven,
        },
        Resolution::Foreign(_) => Verdict::Foreign,
        Resolution::Kept => {
            if oracle.knows(id) {
                Verdict::Kept
            } else {
                Verdict::Unknown
            }
        }
    }
}

/// The shared walk: one verdict per distinct id, weights summed per id.
fn collect<'a>(
    items: impl IntoIterator<Item = (&'a str, u64, Option<usize>)>,
    oracle: &dyn Registry,
) -> Plan {
    let mut plan = Plan::default();
    let mut by_id: BTreeMap<String, (u64, Vec<usize>)> = BTreeMap::new();
    for (id, weight, index) in items {
        plan.states += 1;
        plan.weight += weight;
        let slot = by_id.entry(id.to_owned()).or_insert_with(|| (0, Vec::new()));
        slot.0 += weight;
        if let Some(index) = index {
            // A palette index appears once per distinct state, but a caller can
            // hand the same index twice; the list stays a set in palette order.
            if slot.1.last() != Some(&index) {
                slot.1.push(index);
            }
        }
    }
    plan.entries = by_id
        .into_iter()
        .map(|(id, (weight, indices))| Entry {
            verdict: verdict(&id, oracle),
            id,
            weight,
            indices,
        })
        .collect();
    plan
}

/// The plan for a palette: every entry weighed once, its index kept.
pub fn plan_palette(palette: &[StateRef], oracle: &dyn Registry) -> Plan {
    collect(
        palette
            .iter()
            .enumerate()
            .map(|(index, state)| (state.name.as_str(), 1u64, Some(index))),
        oracle,
    )
}

/// The plan for a plain list of ids — a recipe's ingredients, one building's ids
/// — where every occurrence weighs one and there is no palette to index.
pub fn plan_ids<'a>(ids: impl IntoIterator<Item = &'a str>, oracle: &dyn Registry) -> Plan {
    collect(ids.into_iter().map(|id| (id, 1u64, None)), oracle)
}

/// The plan for a whole blueprint: each palette entry weighed by the cells that
/// index it, which is the only weight a migration can order its work by.
pub fn plan_blueprint(blueprint: &Blueprint, oracle: &dyn Registry) -> Plan {
    let mut cells = vec![0u64; blueprint.palette.len()];
    let mut unplaced = 0u64;
    for block in &blueprint.blocks {
        match cells.get_mut(block.palette_index as usize) {
            Some(count) => *count += 1,
            // `format` accepts the file as written; a cell pointing past the
            // palette is counted here so a caller sees it instead of a wrong
            // weight on a real entry.
            None => unplaced += 1,
        }
    }
    let walked = collect(
        blueprint
            .palette
            .iter()
            .enumerate()
            .map(|(index, state)| (state.name.as_str(), cells[index], Some(index))),
        oracle,
    );
    Plan {
        unplaced,
        ..walked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blueprint::{Block, BlockState, Pos, Size};
    use crate::compat::{Report, StateOracle, SPECS_26_3};
    use std::collections::BTreeSet;
    use std::sync::Arc;

    /// The build a test pretends to have: the two 26.3 blocks the table reaches,
    /// plus ordinary ones so a kept state has something to be known by.
    fn report() -> Report {
        let mut report = Report::from_specs(SPECS_26_3);
        report
            .declare_block("minecraft:stone")
            .declare_block("minecraft:air")
            .declare_block("minecraft:oak_stairs")
            .declare_block("minecraft:tall_grass");
        report
    }

    fn state(name: &str) -> BlockState {
        BlockState {
            name: name.to_owned(),
            properties: Default::default(),
        }
    }

    fn states(names: &[&str]) -> Vec<StateRef> {
        names.iter().map(|name| Arc::new(state(name))).collect()
    }

    /// A blueprint whose `blocks` fill the palette in the given index order, one
    /// cell per index — the shape `format` reads.
    fn blueprint(names: &[&str], indices: &[u16]) -> Blueprint {
        let palette = states(names);
        let blocks = indices
            .iter()
            .enumerate()
            .map(|(y, index)| Block {
                pos: Pos::new(0, y as i32, 0),
                palette_index: *index,
                state: palette.get(*index as usize).cloned().unwrap_or_else(|| {
                    Arc::new(state("minecraft:air"))
                }),
            })
            .collect();
        Blueprint {
            format_version: 1,
            data_version: 3465,
            size: Size {
                x: 1,
                y: indices.len() as u16,
                z: 1,
            },
            name: Some("test".to_owned()),
            architects: Vec::new(),
            required_mods: Vec::new(),
            primary_offset: None,
            palette,
            blocks,
            block_entities: Vec::new(),
            entities: Vec::new(),
        }
    }

    #[test]
    fn the_two_old_blocks_are_replacements_and_the_rest_is_kept() {
        let palette = states(&["minecraft:grass", "minecraft:chain", "minecraft:stone"]);
        let plan = plan_palette(&palette, &report());
        assert_eq!(plan.states, 3);
        assert_eq!(plan.weight, 3);
        assert!(plan.is_ready(), "{}", plan.summary());

        let edits = plan.palette_edits();
        assert_eq!(
            edits,
            vec![(0, "minecraft:short_grass"), (1, "minecraft:iron_chain")]
        );
        assert_eq!(plan.replacements().count(), 2);
        assert_eq!(plan.unknowns().count(), 0);
        assert_eq!(plan.foreign().count(), 0);
        assert_eq!(plan.blocked().count(), 0);
        for entry in plan.replacements() {
            match entry.verdict {
                Verdict::Replace { since, source, .. } => {
                    assert!(since.contains("1.2"), "{since} has no version");
                    assert!(source.contains("minecraft.wiki/"), "{source} has no page");
                }
                other => panic!("{} should be a replacement, got {other:?}", entry.id),
            }
        }
    }

    #[test]
    fn an_id_the_table_keeps_and_the_build_lacks_is_unknown() {
        let palette = states(&["minecraft:stone", "minecraft:made_up_block"]);
        let plan = plan_palette(&palette, &report());
        let unknown: Vec<&str> = plan.unknowns().map(|entry| entry.id.as_str()).collect();
        assert_eq!(unknown, ["minecraft:made_up_block"]);
        assert!(!plan.is_ready(), "an unknown id is work");
        assert!(plan.summary().contains("1 unknown"), "{}", plan.summary());
        assert_eq!(
            plan.work().count(),
            1,
            "only the unknown id needs a decision"
        );
        assert!(plan.palette_edits().is_empty(), "nothing to rewrite");
    }

    #[test]
    fn a_mod_block_is_foreign_and_never_unknown() {
        let palette = states(&[
            "minecolonies:blockhutbuilder",
            "domum_ornamentum:shingle",
            "minecraft:stone",
        ]);
        let plan = plan_palette(&palette, &report());
        let foreign: Vec<&str> = plan.foreign().map(|entry| entry.id.as_str()).collect();
        assert_eq!(
            foreign,
            ["domum_ornamentum:shingle", "minecolonies:blockhutbuilder"],
            "id order, not palette order"
        );
        assert_eq!(plan.unknowns().count(), 0, "a mod id is not a loss");
        assert!(plan.is_ready(), "a foreign id is not the port's work");
    }

    #[test]
    fn a_plan_is_keyed_by_id_so_a_palette_naming_one_twice_reports_once() {
        let palette = states(&["minecraft:grass", "minecraft:stone", "minecraft:grass"]);
        let plan = plan_palette(&palette, &report());
        assert_eq!(plan.entries.len(), 2, "two distinct ids");
        assert_eq!(plan.states, 3, "three palette entries walked");
        let grass = plan
            .entries
            .iter()
            .find(|entry| entry.id == "minecraft:grass")
            .expect("grass is in the palette");
        assert_eq!(grass.weight, 2);
        assert_eq!(grass.indices, [0, 2]);
        assert_eq!(
            plan.palette_edits(),
            vec![(0, "minecraft:short_grass"), (2, "minecraft:short_grass")]
        );
    }

    #[test]
    fn the_weight_is_the_cells_a_palette_index_fills() {
        // Index 0 fills three cells, index 1 one cell, index 2 four cells.
        let pack = blueprint(
            &["minecraft:grass", "minecraft:stone", "minecraft:chain"],
            &[0, 0, 0, 1, 2, 2, 2, 2],
        );
        let plan = plan_blueprint(&pack, &report());
        assert_eq!(plan.states, 3, "three palette entries walked");
        assert_eq!(
            plan.weight, 8,
            "the cells of the eight blocks, summed per palette index"
        );

        let grass = plan
            .entries
            .iter()
            .find(|entry| entry.id == "minecraft:grass")
            .expect("grass is in the palette");
        assert_eq!(grass.weight, 3, "three cells fill index 0");
        let chain = plan
            .entries
            .iter()
            .find(|entry| entry.id == "minecraft:chain")
            .expect("chain is in the palette");
        assert_eq!(chain.weight, 4, "four cells fill index 2");
        assert_eq!(plan.unplaced, 0);
    }

    #[test]
    fn a_cell_pointing_past_the_palette_is_counted_not_dropped() {
        let pack = blueprint(&["minecraft:stone"], &[0, 7]);
        let plan = plan_blueprint(&pack, &report());
        assert_eq!(plan.unplaced, 1);
        assert!(!plan.is_ready(), "an unplaced cell is a state nobody knows");
        assert_eq!(plan.states, 1, "the palette entry, and nothing for the bad index");
        assert_eq!(plan.weight, 1, "the one cell that lands inside the palette");
    }

    #[test]
    fn plan_ids_counts_occurrences_and_keeps_no_index() {
        let plan = plan_ids(
            [
                "minecraft:chain",
                "minecraft:chain",
                "minecolonies:soybean",
                "minecraft:stone",
            ],
            &report(),
        );
        assert_eq!(plan.states, 4);
        assert_eq!(plan.weight, 4);
        let chain = plan
            .entries
            .iter()
            .find(|entry| entry.id == "minecraft:chain")
            .expect("chain is in the list");
        assert_eq!(chain.weight, 2);
        assert!(chain.indices.is_empty(), "a list has no palette");
        assert_eq!(plan.replacements().count(), 1);
        assert_eq!(plan.foreign().count(), 1);
        assert!(plan.is_ready(), "{}", plan.summary());
    }

    #[test]
    fn the_report_is_in_id_order_and_the_same_every_run() {
        let names = [
            "minecraft:stone",
            "minecraft:grass",
            "minecolonies:blockhutbuilder",
            "minecraft:chain",
            "minecraft:air",
        ];
        let first = plan_palette(&states(&names), &report());
        let mut reversed = names;
        reversed.reverse();
        let second = plan_palette(&states(&reversed), &report());
        let order: Vec<&str> = first.entries.iter().map(|entry| entry.id.as_str()).collect();
        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert_eq!(order, sorted);
        let second_order: Vec<&str> = second.entries.iter().map(|entry| entry.id.as_str()).collect();
        assert_eq!(order, second_order, "the palette order does not show through");
        assert_eq!(first.summary(), second.summary());
    }

    /// The pinned client's block section, when the build tree is on this machine.
    /// An export of the game is not content this mod ships, so its absence skips.
    fn block_registry() -> Option<BTreeSet<String>> {
        let mut candidates: Vec<std::path::PathBuf> = Vec::new();
        if let Ok(path) = std::env::var("HORIZON_REGISTRIES_26_3") {
            candidates.push(std::path::PathBuf::from(path));
        }
        for path in [
            "/home/zaccou/horizon-modapi/.local/horizon-client/26.3/registries.json",
            "/workspace/horizon/.local/horizon-client/26.3/registries.json",
        ] {
            candidates.push(std::path::PathBuf::from(path));
        }
        let path = candidates
            .into_iter()
            .find(|candidate| candidate.is_file())?;
        let text = std::fs::read_to_string(path).ok()?;
        let json: mod_api::serde_json::Value = mod_api::serde_json::from_str(&text).ok()?;
        let entries = json.get("minecraft:block")?.get("entries")?.as_object()?;
        Some(entries.keys().cloned().collect())
    }

    /// A build whose only answer is "these ids exist", which is all a plan asks.
    struct Export(BTreeSet<String>);

    impl StateOracle for Export {
        fn knows(&self, block: &str) -> bool {
            self.0.contains(block)
        }
        fn values(&self, _block: &str, _property: &str) -> Option<&[String]> {
            None
        }
    }

    #[test]
    fn the_real_registry_of_26_3_turns_the_pack_ids_into_replacements() {
        let Some(registry) = block_registry() else {
            return;
        };
        let oracle = Export(registry);
        let plan = plan_ids(
            [
                "minecraft:grass",
                "minecraft:chain",
                "minecraft:stone",
                "minecraft:not_a_block_of_26_3",
            ],
            &oracle,
        );
        assert_eq!(
            plan.palette_edits(),
            Vec::new(),
            "plan_ids keeps no palette index"
        );
        assert_eq!(plan.replacements().count(), 2);
        assert_eq!(
            plan.unknowns().map(|entry| entry.id.as_str()).collect::<Vec<_>>(),
            ["minecraft:not_a_block_of_26_3"],
            "the build answers for everything except what it lacks"
        );
        assert!(!plan.is_ready());
        assert!(
            plan.blocked().count() == 0,
            "26.3 renamed both, so nothing is blocked"
        );
    }
}
