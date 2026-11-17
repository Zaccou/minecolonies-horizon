//! `researches/*.json`: five phases, a removal that cascades, and the loop that
//! makes the cascade reach the bottom of a subtree.
//!
//! Java reference: `core/datalistener/ResearchListener.java:158-186` (the order of
//! the five phases) and `:564-624` (the loop this file is mostly about). The 217
//! researches themselves are `research::tree`'s — that module owns the rows, this
//! one owns the **sequencing** the listener imposes on them, which is the part a
//! reader cannot see in a table of rows.
//!
//! **The five phases, and why that order.** Java says it in one comment
//! (`:163-165`): effects first "because data packs can't assume effects are in one
//! specific location", then removes as a **separate pass** so that a `remove` does
//! not depend on where its target sits in a `HashMap` (`:168-171`), then the
//! researches that need both, then the parent/child tree, then the branch settings.
//!
//! **The cascade.** `calcResearchTree` runs a `do { } while (count changed)` loop
//! (`:573-610`): a research is dropped when its parent is missing at depth > 1
//! (`:583-587`) or when `depth − parent.depth != 1` (`:588-603`), and because
//! dropping a node can orphan its own children, the pass is repeated until nothing
//! changes. Removing a research at depth 2 therefore removes the whole subtree
//! below it — **in two passes when the map happens to list a child before its
//! parent**, which is what [`cascade`] reproduces with a deterministic order.
//!
//! **The remove property's four spellings and its one error.** `remove` may be an
//! array of ids, a single id, or `true` meaning "this file" (`:496-546`); in a file
//! that also carries a branch marker the ids name **branches** (`:493-521`),
//! otherwise researches. Anything else — `remove: false` included — is logged as
//! `"does not contain all required fields"` and ignored (`:548-552`), which is the
//! only place in the package where a *false* boolean is an error.
//!
//! **What is not here**: the effect categories (`research::effect`), the cost and
//! requirement registries (`research::cost`), and the 217 rows (`research::tree`).
//! This file never re-states a research; it decides which of them survive a reload.

use std::collections::BTreeSet;

use super::Refused;

/// The five phases, in the order `apply` runs them (`:166-183`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
// exhaustive: the five calls of `apply`, no sixth
pub enum Phase {
    /// `parseResearchEffectCategories` (`:166`): the effect ids and their levels,
    /// needed before any research can name one.
    Effects,
    /// `parseRemoveResearches` (`:171`): the ids and branches a pack deletes, as a
    /// **separate pass** so the `HashMap` order cannot matter.
    Removes,
    /// `parseResearches` (`:176`): the rows that are neither an effect nor a branch
    /// nor a removed one.
    Researches,
    /// `calcResearchTree` (`:180`): parent links, then the cascade.
    Tree,
    /// `parseResearchBranches` (`:183`): the four branches' own settings, defaulted
    /// for a branch no file describes (`:641-643`).
    Branches,
}

/// The five, in order.
pub const PHASES: &[Phase] = &[
    Phase::Effects,
    Phase::Removes,
    Phase::Researches,
    Phase::Tree,
    Phase::Branches,
];

/// Why a phase must run before another, in one line each.
pub fn purpose(phase: Phase) -> &'static str {
    match phase {
        Phase::Effects => "a research may name an effect the same pack declares",
        Phase::Removes => "a removed branch or research must be gone before rows are built",
        Phase::Researches => "needs the effects for its levels and the removes to skip",
        Phase::Tree => "needs every row to resolve parents, then cascades",
        Phase::Branches => "needs the research set to know which branches exist",
    }
}

/// The five properties that classify a file (`:240-244`, `:202`, `:490`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: the five keys the listener tests, and nothing else
pub struct Markers {
    pub effect: bool,
    pub branch_name: bool,
    pub base_time: bool,
    pub branch_type: bool,
    pub remove: bool,
}

impl Markers {
    /// `:240-244`: a file carrying any of four keys is **not** a research.
    pub fn is_research(self) -> bool {
        !self.effect && !self.is_branch() && !self.remove
    }

    /// A branch declaration: any of the three branch keys (`:240-241`).
    pub fn is_branch(self) -> bool {
        self.branch_name || self.base_time || self.branch_type
    }

    /// An effect declaration (`:202`).
    pub fn is_effect(self) -> bool {
        self.effect
    }
}

/// The `remove` property, in its four accepted shapes (`:496-546`).
///
/// No lifetime parameter: a row in this module is `&'static str` throughout, as it
/// is in `citizens::happiness::disease::Row`, because the data pack is compiled in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the shapes the branches of `:496-546` distinguish
pub enum RemoveValue {
    /// An array of ids; non-string elements are skipped in silence (`:500-504`).
    Array(&'static [&'static str]),
    /// One id (`:511-513`, `:537-540`).
    One(&'static str),
    /// `true`: the file's own id (`:516-520`, `:542-546`).
    True,
    /// `false`: Java logs `"does not contain all required fields"` (`:548-552`).
    False,
    /// Anything else: same error, same line.
    Other,
}

/// What a pack deletes. Two sets, because a research and a branch cannot collide.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: the `Tuple` of `:555` has two members
pub struct RemoveSpec {
    pub researches: Vec<&'static str>,
    pub branches: Vec<&'static str>,
}

/// Why a research file is refused, named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// A `remove` that is neither a list, a string, nor `true` (`:548-552`).
    MalformedRemove { id: &'static str },
    /// A row at depth > 1 whose parent is not in the pack (`:583-587`).
    MissingParent { id: &'static str, depth: i32 },
    /// `depth − parent.depth` is not 1 (`:588-603`): `-1` means the parent is a
    /// child or a sibling, `> 1` means a level was skipped.
    DepthGap { id: &'static str, offset: i32 },
    /// The cascade did not settle in [`MAX_PASSES`]. **Port-only**: Java's loop
    /// terminates because a pass only ever removes rows, and this bound is here to
    /// name the failure instead of hanging.
    NoFixpoint { passes: usize },
}

impl Refused for Refusal {
    fn name(&self) -> String {
        match *self {
            Refusal::MalformedRemove { id } => {
                format!("data_listeners.refusal.malformed_research_remove({id})")
            }
            Refusal::MissingParent { id, depth } => {
                format!("data_listeners.refusal.research_parent_missing({id}.d{depth})")
            }
            Refusal::DepthGap { id, offset } => {
                format!("data_listeners.refusal.research_depth_gap({id}.{offset})")
            }
            Refusal::NoFixpoint { passes } => {
                format!("data_listeners.refusal.research_cascade_no_fixpoint({passes})")
            }
        }
    }
}

/// How many passes the cascade may make before it gives up.
///
/// One more than the deepest tree Java can hold (`research::MAX_DEPTH` is 6), so a
/// legitimate pack never reaches it.
pub const MAX_PASSES: usize = 8;

/// Reads one `remove` property. `id` is the **file's** id, which `true` means.
pub fn read_remove(
    id: &'static str,
    as_branch: bool,
    value: RemoveValue,
) -> Result<RemoveSpec, Refusal> {
    let ids: Vec<&'static str> = match value {
        RemoveValue::Array(list) => list.to_vec(),
        RemoveValue::One(one) => vec![one],
        RemoveValue::True => vec![id],
        RemoveValue::False | RemoveValue::Other => return Err(Refusal::MalformedRemove { id }),
    };
    Ok(if as_branch {
        RemoveSpec { researches: Vec::new(), branches: ids }
    } else {
        RemoveSpec { researches: ids, branches: Vec::new() }
    })
}

/// One row, reduced to what the tree pass needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three fields the cascade reads
pub struct Node {
    pub id: &'static str,
    pub parent: Option<&'static str>,
    pub branch: &'static str,
    pub depth: i32,
}

/// What one cascade did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cascade {
    pub kept: Vec<Node>,
    pub refused: Vec<Refusal>,
    /// How many passes **dropped a row**. `0` is a pack that needed no cascade.
    pub passes: usize,
}

/// Drops every row a branch removal takes with it (`:265`).
///
/// One pass, no cascade: a research names its branch directly, so a removed branch
/// takes all of its rows at once.
pub fn prune_branches(nodes: &[Node], spec: &RemoveSpec) -> Vec<Node> {
    let gone: BTreeSet<&str> = spec.branches.iter().copied().collect();
    nodes
        .iter()
        .filter(|node| !gone.contains(node.branch))
        .copied()
        .collect()
}

/// The `do/while` of `calcResearchTree` (`:573-610`), one row list in, survivors
/// out.
///
/// Rows are visited **in the order given** and a row dropped during a pass is gone
/// for the rows after it in the same pass — Java's `iterator.remove()` while the
/// loop reads the same map. That is why the count of passes is observable: a child
/// listed before its parent survives the pass that kills the parent, and only the
/// next one takes it.
pub fn cascade(nodes: &[Node]) -> Cascade {
    let mut alive: Vec<Node> = nodes.to_vec();
    let mut refused: Vec<Refusal> = Vec::new();
    let mut passes = 0;

    loop {
        let mut present: BTreeSet<&str> = alive.iter().map(|node| node.id).collect();
        let mut next: Vec<Node> = Vec::with_capacity(alive.len());
        let mut dropped = false;

        for node in &alive {
            let parent = node.parent.filter(|id| present.contains(id));
            if parent.is_none() && node.depth > 1 {
                refused.push(Refusal::MissingParent { id: node.id, depth: node.depth });
                present.remove(node.id);
                dropped = true;
                continue;
            }
            let parent_depth = parent
                .and_then(|id| alive.iter().find(|candidate| candidate.id == id))
                .map(|candidate| candidate.depth)
                .unwrap_or(0);
            let offset = node.depth - parent_depth;
            if offset != 1 {
                refused.push(Refusal::DepthGap { id: node.id, offset });
                present.remove(node.id);
                dropped = true;
                continue;
            }
            next.push(*node);
        }

        alive = next;
        if !dropped {
            break;
        }
        passes += 1;
        if passes > MAX_PASSES {
            refused.push(Refusal::NoFixpoint { passes });
            break;
        }
    }

    Cascade { kept: alive, refused, passes }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &'static str, parent: Option<&'static str>, depth: i32) -> Node {
        Node { id, parent, branch: "civilian", depth }
    }

    #[test]
    fn the_five_phases_are_the_five_calls_of_apply() {
        assert_eq!(PHASES.len(), 5);
        assert_eq!(
            PHASES,
            &[Phase::Effects, Phase::Removes, Phase::Researches, Phase::Tree, Phase::Branches]
        );
        for pair in PHASES.windows(2) {
            assert!(pair[0] < pair[1], "{:?} must precede {:?}", pair[0], pair[1]);
            assert!(!purpose(pair[0]).is_empty());
        }
    }

    #[test]
    fn a_file_with_an_effect_or_a_branch_key_is_not_a_research() {
        assert!(Markers { effect: true, ..Default::default() }.is_effect());
        assert!(!Markers { effect: true, ..Default::default() }.is_research());
        for markers in [
            Markers { branch_name: true, ..Default::default() },
            Markers { base_time: true, ..Default::default() },
            Markers { branch_type: true, ..Default::default() },
        ] {
            assert!(markers.is_branch());
            assert!(!markers.is_research());
        }
        assert!(Markers::default().is_research());
        assert!(!Markers { remove: true, ..Default::default() }.is_research());
    }

    #[test]
    fn remove_false_is_an_error_and_not_a_no_op() {
        let refusal = read_remove("researches/x", false, RemoveValue::False)
            .expect_err("`remove: false` is logged as malformed");
        assert_eq!(
            refusal.name(),
            "data_listeners.refusal.malformed_research_remove(researches/x)"
        );
        assert!(read_remove("researches/x", false, RemoveValue::Other).is_err());
    }

    #[test]
    fn true_names_the_file_and_a_branch_marker_moves_the_ids_to_the_branch_set() {
        let mine = read_remove("researches/technology/x", false, RemoveValue::True).expect("valid");
        assert_eq!(mine.researches, vec!["researches/technology/x"]);
        assert!(mine.branches.is_empty());
        let theirs = read_remove("researches/pack/y", true, RemoveValue::One("minecolonies:combat"))
            .expect("valid");
        assert_eq!(theirs.branches, vec!["minecolonies:combat"]);
        assert!(theirs.researches.is_empty());
        let many = read_remove("researches/pack/z", false, RemoveValue::Array(&["a", "b"]))
            .expect("valid");
        assert_eq!(many.researches, vec!["a", "b"]);
    }

    #[test]
    fn a_removed_branch_takes_its_researches_in_one_pass() {
        let mut combat = node("combat/one", None, 1);
        combat.branch = "combat";
        let nodes = vec![node("civilian/one", None, 1), combat];
        let spec = RemoveSpec { researches: Vec::new(), branches: vec!["combat"] };
        let kept = prune_branches(&nodes, &spec);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].id, "civilian/one");
        let result = cascade(&kept);
        assert!(result.refused.is_empty(), "no depth work left to do");
        assert_eq!(result.passes, 0);
    }

    #[test]
    fn a_depth_gap_of_more_than_one_drops_the_row() {
        // `:598-603`: "parent cannot be set multiple levels deep".
        let nodes = vec![node("a", None, 1), node("c", Some("a"), 3)];
        let result = cascade(&nodes);
        assert_eq!(result.kept, vec![node("a", None, 1)]);
        assert_eq!(
            result.refused,
            vec![Refusal::DepthGap { id: "c", offset: 2 }]
        );
        assert_eq!(
            result.refused[0].name(),
            "data_listeners.refusal.research_depth_gap(c.2)"
        );
    }

    #[test]
    fn a_sibling_parent_is_refused_and_a_cycle_cannot_form() {
        // `:594-597`: `depthOffset < 1` means the chosen parent is a child or a
        // sibling. A parent cycle would need `d = p + 1` *and* `p = d + 1`, so the
        // same check is what makes one impossible.
        let nodes = vec![node("a", None, 1), node("b", Some("a"), 1)];
        let result = cascade(&nodes);
        assert_eq!(result.kept, vec![node("a", None, 1)]);
        assert_eq!(result.refused, vec![Refusal::DepthGap { id: "b", offset: 0 }]);
    }

    #[test]
    fn a_removal_at_depth_two_takes_the_subtree_below_and_needs_two_passes() {
        // The subtree, listed child-first, which a `HashMap` is free to do: this is
        // the case that makes the `do/while` necessary rather than decorative.
        let nodes = vec![
            node("d", Some("c"), 4),
            node("c", Some("b"), 3),
            node("a", None, 1),
        ];
        let survivors: Vec<Node> = nodes
            .iter()
            .copied()
            .filter(|node| node.id != "b")
            .collect();
        let result = cascade(&survivors);
        // Pass 1 keeps `d` (its parent `c` is still listed) and drops `c` (its
        // parent `b` was removed by the pack); pass 2 drops `d`.
        assert_eq!(result.kept, vec![node("a", None, 1)]);
        assert_eq!(result.passes, 2);
        assert_eq!(
            result.refused,
            vec![
                Refusal::MissingParent { id: "c", depth: 3 },
                Refusal::MissingParent { id: "d", depth: 4 },
            ]
        );
    }
}
