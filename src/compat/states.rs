//! Block states: does 26.3 declare the property a blueprint wrote?
//!
//! # The measurement, and its answer
//!
//! The palettes hold **4 047 distinct (block, property, value) triples** — 3 400
//! of them `minecraft` — over 629 blocks (526 `minecraft`) and 79 property names
//! (`facing`, `axis`, `waterlogged`, `half`, `shape`, `age`, the sixteen
//! `has_bottle_*` of the brewing stand…). Each was compared with the game's own
//! block report: `properties` per block, in `blocks.json` of the vanilla datagen
//! output for the pinned 26.3 build, value by value.
//!
//! **Zero.** Not one property, not one value, falls outside what 26.3 declares,
//! once the two renamed blocks are read under their new name
//! ([`crate::compat::table`]). The case the brief singled out confirms it: the
//! pack's 1 313 `chain` entries write `axis` (x, y, z) and `waterlogged` (false),
//! and `iron_chain` declares exactly those, plus `waterlogged` (true). The block
//! changed its **name**, not its shape.
//!
//! # Why the check exists when the answer is zero
//!
//! A measurement is a fact about one pack and one build; a check keeps it true.
//! The port converts a blueprint on the player's machine, against the game that
//! machine runs, and a state the game cannot express either crashes the placement
//! or silently loses a stair's shape. So the check stays, and it takes the
//! registry as a caller-supplied [`StateOracle`]: nothing here hard-codes 1 286
//! blocks, and the game answers for itself. [`Report`] is the oracle a test — or a
//! dump of `blocks.json` — fills; [`SPECS_26_3`] holds the two blocks this port
//! reaches under a 1.20.1 name, transcribed with their source.
//!
//! # It reports, it does not repair
//!
//! [`audit`] returns [`Gap`]s and leaves every state as it found it. Dropping a
//! property, or picking a neighbouring value, is a decision about how a building
//! should look: a staircase whose `shape` vanished should be **seen** by the port,
//! not quietly turned into a straight one.

use std::collections::{BTreeMap, BTreeSet};

use crate::blueprint::{BlockState, StateRef};
use crate::compat::{resolve, Resolution};

/// One property of a 26.3 block: its name and the values the game accepts.
#[derive(Clone, Copy, Debug)]
pub struct PropertySpec {
    pub name: &'static str,
    pub values: &'static [&'static str],
}

/// One 26.3 block and the properties it declares.
#[derive(Clone, Copy, Debug)]
pub struct BlockSpec {
    pub id: &'static str,
    pub properties: &'static [PropertySpec],
    /// Where the property set is written down.
    pub source: &'static str,
}

/// The 26.3 blocks this port reaches under a 1.20.1 name — the whole of what a
/// rename can cost in block state.
pub const SPECS_26_3: &[BlockSpec] = &[
    BlockSpec {
        id: "minecraft:short_grass",
        properties: &[],
        source: "minecraft.wiki/w/Short_Grass § Data values: no block state, only an id; the \
                 pinned build's blocks.json lists no `properties` for it",
    },
    BlockSpec {
        id: "minecraft:iron_chain",
        properties: &[
            PropertySpec { name: "axis", values: &["x", "y", "z"] },
            PropertySpec { name: "waterlogged", values: &["true", "false"] },
        ],
        source: "minecraft.wiki/w/Iron_Chain § Block states: axis (x, y, z) and waterlogged \
                 (true, false) — the four values 1.20.1's chain already had",
    },
];

/// What a caller must be able to answer about 26.3 to audit a state.
///
/// The game is the only real implementation: `blocks.json` of a build, or the
/// registry the host exposes. Keeping it a trait is what lets the audit run on the
/// host target in a test with no game at all.
pub trait StateOracle {
    /// Does this build hold a block with this id?
    fn knows(&self, block: &str) -> bool;
    /// The values this block accepts for this property, or `None` when it does not
    /// declare the property.
    fn values(&self, block: &str, property: &str) -> Option<&[String]>;
}

/// The game's block report, as much of it as a check needs: id → property →
/// allowed values.
#[derive(Clone, Debug, Default)]
pub struct Report {
    blocks: BTreeMap<String, BTreeMap<String, Vec<String>>>,
}

impl Report {
    pub fn new() -> Self {
        Self::default()
    }

    /// A report holding exactly the transcribed blocks of [`SPECS_26_3`].
    pub fn from_specs(specs: &[BlockSpec]) -> Self {
        let mut report = Self::new();
        for spec in specs {
            report.declare_block(spec.id);
            for property in spec.properties {
                report.declare(spec.id, property.name, property.values);
            }
        }
        report
    }

    /// Declare one property of one block. A second call on the same pair adds
    /// values instead of replacing them, so a report can be filled in pieces.
    pub fn declare(&mut self, block: &str, property: &str, values: &[&str]) -> &mut Self {
        let entry = self
            .blocks
            .entry(block.to_owned())
            .or_default()
            .entry(property.to_owned())
            .or_default();
        for value in values {
            let value = (*value).to_owned();
            if !entry.contains(&value) {
                entry.push(value);
            }
        }
        self
    }

    /// Declare a block without any property, so [`Report::knows_block`] answers.
    pub fn declare_block(&mut self, block: &str) -> &mut Self {
        self.blocks.entry(block.to_owned()).or_default();
        self
    }

    /// Does the report hold a block of this id? The answer is the presence of
    /// `block` as a key of `Report::blocks` — the map the report is built on — so a
    /// block declared with no property at all answers `true` and a block nobody
    /// declared answers `false`. This is the 26.3 half of every [`Gap`]: a state
    /// whose block is unknown here is [`Gap::NotIn26_3`], not a property problem.
    pub fn knows_block(&self, block: &str) -> bool {
        self.blocks.contains_key(block)
    }

    /// Every block the report holds.
    pub fn blocks(&self) -> impl Iterator<Item = &str> + '_ {
        self.blocks.keys().map(String::as_str)
    }
}

impl StateOracle for Report {
    fn knows(&self, block: &str) -> bool {
        self.knows_block(block)
    }

    fn values(&self, block: &str, property: &str) -> Option<&[String]> {
        self.blocks.get(block)?.get(property).map(Vec::as_slice)
    }
}

/// One thing 26.3 cannot express, or cannot express without a mod.
///
/// Ordered so the same problem met in two blueprints reports once.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Gap {
    /// A `minecraft` id this build does not hold at all.
    NotIn26_3 { block: String },
    /// The table marks this 1.20.1 id as gone: there is no 26.3 block to use.
    Gone { block: String },
    /// The table cannot place this id: neither renamed nor proven gone.
    Unproven { block: String },
    /// A property the 26.3 block does not declare.
    UndeclaredProperty { block: String, property: String },
    /// A property it declares, holding a value it does not accept.
    UndeclaredValue {
        block: String,
        property: String,
        value: String,
        allowed: Vec<String>,
    },
    /// A namespace that is not `minecraft`: content a mod has to bring.
    Foreign { block: String, namespace: String },
}

impl Gap {
    /// The block the gap is about, as it should be looked up in 26.3.
    pub fn block(&self) -> &str {
        match self {
            Self::NotIn26_3 { block }
            | Self::Gone { block }
            | Self::Unproven { block }
            | Self::UndeclaredProperty { block, .. }
            | Self::UndeclaredValue { block, .. }
            | Self::Foreign { block, .. } => block,
        }
    }
}

impl std::fmt::Display for Gap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotIn26_3 { block } => write!(f, "{block} is not a block of 26.3"),
            Self::Gone { block } => write!(f, "{block} no longer exists in 26.3"),
            Self::Unproven { block } => write!(f, "{block} has no proven 26.3 name"),
            Self::UndeclaredProperty { block, property } => {
                write!(f, "{block} declares no property {property}")
            }
            Self::UndeclaredValue { block, property, value, allowed } => write!(
                f,
                "{block}[{property}={value}] is not a state of 26.3 (it accepts {allowed:?})"
            ),
            Self::Foreign { block, namespace } => {
                write!(f, "{block} belongs to {namespace}, which must provide it")
            }
        }
    }
}

/// One rename the audit applied before checking a state.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Rewrite {
    pub from: String,
    pub to: &'static str,
}

/// What one pass over a palette found.
#[derive(Clone, Debug, Default)]
pub struct Audit {
    pub states: usize,
    pub rewrites: Vec<Rewrite>,
    pub gaps: Vec<Gap>,
}

impl Audit {
    /// No state needs a decision: every one is placeable as it stands, possibly
    /// under a renamed block.
    pub fn is_clean(&self) -> bool {
        self.gaps.is_empty()
    }

    pub fn summary(&self) -> String {
        format!(
            "{} states, {} renames, {} gaps",
            self.states,
            self.rewrites.len(),
            self.gaps.len()
        )
    }
}

/// Audit `states` against `oracle`, applying the table on the way.
///
/// A state the table renames is checked under its 26.3 name; a state in a mod
/// namespace is reported as [`Gap::Foreign`] and not probed further, because a
/// vanilla registry could never hold it. Gaps and rewrites are deduped, so a
/// palette naming the same broken state twice reports it once.
pub fn audit<'a>(
    states: impl IntoIterator<Item = &'a BlockState>,
    oracle: &dyn StateOracle,
) -> Audit {
    let mut audit = Audit::default();
    let mut seen_gaps: BTreeSet<Gap> = BTreeSet::new();
    let mut seen_rewrites: BTreeSet<Rewrite> = BTreeSet::new();

    for state in states {
        audit.states += 1;

        let target = match resolve(&state.name) {
            Resolution::Kept => state.name.as_str(),
            Resolution::Foreign(namespace) => {
                push_gap(&mut audit, &mut seen_gaps, Gap::Foreign {
                    block: state.name.clone(),
                    namespace: namespace.to_owned(),
                });
                continue;
            }
            Resolution::Renamed(row) => {
                let Some(to) = row.to_26_3 else {
                    push_gap(&mut audit, &mut seen_gaps, Gap::Unproven {
                        block: row.from_1_20_1.to_owned(),
                    });
                    continue;
                };
                let rewrite = Rewrite { from: state.name.clone(), to };
                if seen_rewrites.insert(rewrite.clone()) {
                    audit.rewrites.push(rewrite);
                }
                to
            }
            Resolution::Gone(row) => {
                push_gap(&mut audit, &mut seen_gaps, Gap::Gone {
                    block: row.from_1_20_1.to_owned(),
                });
                continue;
            }
            Resolution::Uncertain(row) => {
                push_gap(&mut audit, &mut seen_gaps, Gap::Unproven {
                    block: row.from_1_20_1.to_owned(),
                });
                continue;
            }
        };

        if !oracle.knows(target) {
            push_gap(&mut audit, &mut seen_gaps, Gap::NotIn26_3 {
                block: target.to_owned(),
            });
            continue;
        }

        // `properties` is a `BTreeMap`, so the gaps come out in name order and the
        // report is stable from run to run.
        for (property, value) in &state.properties {
            match oracle.values(target, property) {
                None => push_gap(&mut audit, &mut seen_gaps, Gap::UndeclaredProperty {
                    block: target.to_owned(),
                    property: property.clone(),
                }),
                Some(allowed) => {
                    if !allowed.iter().any(|candidate| candidate == value) {
                        push_gap(&mut audit, &mut seen_gaps, Gap::UndeclaredValue {
                            block: target.to_owned(),
                            property: property.clone(),
                            value: value.clone(),
                            allowed: allowed.to_vec(),
                        });
                    }
                }
            }
        }
    }

    audit
}

/// [`audit`] over a blueprint's palette, which is what [`crate::blueprint`] holds.
pub fn audit_palette(palette: &[StateRef], oracle: &dyn StateOracle) -> Audit {
    audit(palette.iter().map(|state| &**state), oracle)
}

fn push_gap(audit: &mut Audit, seen: &mut BTreeSet<Gap>, gap: Gap) {
    if seen.insert(gap.clone()) {
        audit.gaps.push(gap);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn state(name: &str, properties: &[(&str, &str)]) -> BlockState {
        BlockState {
            name: name.to_owned(),
            properties: properties
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        }
    }

    /// The report a test uses: the two transcribed blocks, plus ordinary ones so a
    /// kept state has something to be checked against.
    fn oracle() -> Report {
        let mut report = Report::from_specs(SPECS_26_3);
        report.declare("minecraft:oak_stairs", "facing", &["north", "south", "west", "east"]);
        report.declare("minecraft:oak_stairs", "half", &["top", "bottom"]);
        report.declare_block("minecraft:stone");
        report
    }

    #[test]
    fn the_transcribed_blocks_are_the_two_the_table_renames() {
        let ids: Vec<&str> = SPECS_26_3.iter().map(|spec| spec.id).collect();
        assert_eq!(ids, ["minecraft:short_grass", "minecraft:iron_chain"]);
        for spec in SPECS_26_3 {
            assert!(spec.source.contains("minecraft.wiki/"), "{} has no page", spec.id);
        }
        let chain = &SPECS_26_3[1];
        let names: Vec<&str> = chain.properties.iter().map(|p| p.name).collect();
        assert_eq!(names, ["axis", "waterlogged"]);
        assert!(SPECS_26_3[0].properties.is_empty(), "the plant has no state");
    }

    #[test]
    fn a_state_26_3_declares_passes_untouched() {
        let states = [
            state("minecraft:short_grass", &[]),
            state("minecraft:oak_stairs", &[("facing", "north"), ("half", "top")]),
        ];
        let audit = audit(states.iter(), &oracle());
        assert_eq!(audit.states, 2);
        assert!(audit.is_clean(), "gaps: {:?}", audit.gaps);
        assert!(audit.rewrites.is_empty());
    }

    #[test]
    fn the_two_known_cases_are_rewritten_and_then_pass() {
        let states = [
            state("minecraft:grass", &[]),
            state("minecraft:chain", &[("axis", "y"), ("waterlogged", "false")]),
            state("minecraft:chain", &[("axis", "x"), ("waterlogged", "false")]),
            state("minecraft:chain", &[("axis", "z"), ("waterlogged", "false")]),
        ];
        let audit = audit(states.iter(), &oracle());
        assert!(audit.is_clean(), "gaps: {:?}", audit.gaps);
        assert_eq!(audit.rewrites.len(), 2, "one rewrite per block, not per state");
        assert_eq!(
            audit.rewrites[0],
            Rewrite {
                from: "minecraft:grass".into(),
                to: "minecraft:short_grass"
            }
        );
        assert_eq!(
            audit.rewrites[1],
            Rewrite {
                from: "minecraft:chain".into(),
                to: "minecraft:iron_chain"
            }
        );
        assert_eq!(audit.summary(), "4 states, 2 renames, 0 gaps");
    }

    #[test]
    fn an_axis_that_does_not_exist_is_reported_and_not_fixed() {
        let bad = state("minecraft:chain", &[("axis", "w")]);
        let audit = audit(std::iter::once(&bad), &oracle());
        assert_eq!(audit.gaps, vec![Gap::UndeclaredValue {
            block: "minecraft:iron_chain".to_owned(),
            property: "axis".to_owned(),
            value: "w".to_owned(),
            allowed: vec!["x".to_owned(), "y".to_owned(), "z".to_owned()],
        }]);
        assert_eq!(bad.properties.get("axis").map(String::as_str), Some("w"),
            "the audit reports; it never rewrites a state");
        assert!(audit.gaps[0].to_string().contains("accepts"));
    }

    #[test]
    fn a_property_the_26_3_block_does_not_declare_is_reported() {
        let bad = state("minecraft:chain", &[("facing", "north")]);
        let audit = audit(std::iter::once(&bad), &oracle());
        assert_eq!(audit.gaps, vec![Gap::UndeclaredProperty {
            block: "minecraft:iron_chain".to_owned(),
            property: "facing".to_owned(),
        }]);
        assert_eq!(audit.gaps[0].block(), "minecraft:iron_chain");
    }

    #[test]
    fn a_block_or_an_item_the_build_does_not_hold_is_reported() {
        // `bread` is an item a raw scan of the pack does meet; the palette never
        // does, and if one ever did the audit refuses it here.
        let states = [state("minecraft:stone_brick_wall", &[]), state("minecraft:bread", &[])];
        let audit = audit(states.iter(), &oracle());
        assert_eq!(audit.gaps, vec![
            Gap::NotIn26_3 { block: "minecraft:stone_brick_wall".to_owned() },
            Gap::NotIn26_3 { block: "minecraft:bread".to_owned() },
        ]);
        assert!(!audit.is_clean());
    }

    #[test]
    fn a_mod_block_is_foreign_not_missing() {
        let states = [
            state("structurize:blocksubstitution", &[]),
            state("minecolonies:blockhutbuilder", &[]),
        ];
        let audit = audit(states.iter(), &oracle());
        assert_eq!(
            audit.gaps,
            vec![
                Gap::Foreign {
                    block: "structurize:blocksubstitution".to_owned(),
                    namespace: "structurize".to_owned(),
                },
                Gap::Foreign {
                    block: "minecolonies:blockhutbuilder".to_owned(),
                    namespace: "minecolonies".to_owned(),
                },
            ]
        );
        assert!(
            audit.gaps.iter().all(|gap| !matches!(gap, Gap::NotIn26_3 { .. })),
            "a mod block is content to bring, not a block 26.3 lost"
        );
    }

    #[test]
    fn the_same_gap_is_reported_once_however_often_it_is_met() {
        let states = [
            state("minecraft:stone_brick_wall", &[]),
            state("minecraft:stone_brick_wall", &[]),
            state("minecraft:chain", &[("axis", "w")]),
            state("minecraft:chain", &[("axis", "w")]),
        ];
        let audit = audit(states.iter(), &oracle());
        assert_eq!(audit.states, 4);
        assert_eq!(audit.gaps.len(), 2, "gaps: {:?}", audit.gaps);
    }

    #[test]
    fn a_blueprint_palette_can_be_audited_directly() {
        let palette: Vec<StateRef> = vec![
            Arc::new(state("minecraft:chain", &[("axis", "y"), ("waterlogged", "false")])),
            Arc::new(state("minecraft:grass", &[])),
            Arc::new(state("minecraft:oak_stairs", &[("facing", "north"), ("half", "top")])),
        ];
        let audit = audit_palette(&palette, &oracle());
        assert_eq!(audit.states, 3);
        assert_eq!(audit.rewrites.len(), 2);
        assert!(audit.is_clean(), "gaps: {:?}", audit.gaps);
    }

    #[test]
    fn an_empty_palette_audits_clean() {
        let states: [BlockState; 0] = [];
        let audit = audit(states.iter(), &oracle());
        assert_eq!(audit.states, 0);
        assert!(audit.is_clean());
        assert_eq!(audit.summary(), "0 states, 0 renames, 0 gaps");
    }

    #[test]
    fn a_report_answers_what_it_was_given_and_nothing_else() {
        let mut report = oracle();
        assert!(report.knows_block("minecraft:iron_chain"));
        assert!(report.knows_block("minecraft:stone"));
        assert!(!report.knows_block("minecraft:dirt"));
        assert_eq!(report.values("minecraft:iron_chain", "axis"),
            Some(&["x".to_owned(), "y".to_owned(), "z".to_owned()][..]));
        assert_eq!(report.values("minecraft:short_grass", "axis"), None);
        assert_eq!(report.values("minecraft:dirt", "axis"), None);
        assert_eq!(report.blocks().count(), 4);
        // A second declaration adds values instead of replacing them.
        report.declare("minecraft:iron_chain", "axis", &["y", "w"]);
        assert_eq!(report.values("minecraft:iron_chain", "axis"),
            Some(&["x".to_owned(), "y".to_owned(), "z".to_owned(), "w".to_owned()][..]));
    }
}
