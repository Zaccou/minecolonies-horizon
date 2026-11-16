//! The university's research tree: what a colony unlocks, what it costs, and
//! how far along it is.
//!
//! Java reference: `core/research/` (11 files, 2 234 lines) and the generated
//! data under
//! `src/datagen/generated/minecolonies/data/minecolonies/researches/`.
//!
//! **The count, measured on the data.** `researches/` holds **327** JSON files:
//! **4** branch declarations (`civilian.json`, `combat.json`,
//! `technology.json`, `unlockable.json`), **106** effect categories
//! (`effects/*.json`) and **217** researches — `technology/` 76, `civilian/` 69,
//! `combat/` 68, `unlockable/` 4. `effects/` and `unlockable/` are *folders* that
//! hold researches, not extra files: the 4 unlockables are a branch like any
//! other, only `hidden` and `autostart`.
//!
//! | File | Question |
//! | --- | --- |
//! | `branch` | the 4 branches, their base time, and how long a depth takes |
//! | `effect` | the 106 effect categories, level by level |
//! | `tree` | the 217 researches: parent, depth, flags, costs, effects, requirements |
//! | `cost` | what a research costs, and what is still missing |
//! | `gate` | why a research cannot start, or cannot be reset — named |
//! | `progress` | what is done, what is running, and its compound |
//! | `nbt` | the tag reads and writes `progress` uses |
//!
//! **The tree is data, and it is loaded, not guessed.** Java reads the 217 files
//! at data-pack load, validates the relationships and *drops* the rows that do
//! not hold (`core/datalistener/ResearchListener.java:564-620`, "could not be
//! attached to tree"); it does not trust the JSON either. So the table here is
//! one line per research in a packed form (`tree::TABLE`), read by
//! [`tree::Tree::load`], which refuses — with a named [`tree::LoadError`] — a
//! missing parent, a depth that is not the parent's plus one, an effect that no
//! category declares, or a duplicate id. A `const` table of typed structs was
//! the other option and lost on size: the same 217 rows average 101 characters
//! packed and 158 typed, and the typed form cannot refuse anything.
//!
//! **What is *not* here**, and what each gap costs:
//!
//! - **no GUI.** `WindowResearchTree` (1 100 lines) draws the tree, hides the
//!   rows whose requirements are not met (`:285`) and suppresses the undo button
//!   for an `immutable` or `autostart` research (`:228-243`). Those two rules
//!   are *client* rules in Java and the port folds them into [`gate`], because a
//!   wasm mod has no second place to put them.
//! - **no packet.** `GlobalResearchTreeMessage` (72 lines) ships the whole tree
//!   to the client on join; a colony here runs on one side and the tree is a
//!   compile-time constant.
//! - **no reset cost.** Undoing a finished research costs
//!   `server.researchResetCost` (`GlobalResearchTree.java:302-344`), a server
//!   config list of item ids with NBT; the port has no config and [`gate`] says
//!   so rather than pricing it at zero.
//! - **no random pick.** Java's researcher picks *any* in-progress research at
//!   random (`EntityAIWorkResearcher.java:86`); [`progress::Progress::advance`]
//!   takes the id, because a port with no clock and no RNG cannot pretend to
//!   choose.

pub mod branch;
mod cost;
pub mod effect;
pub mod gate;
mod nbt;
pub mod progress;
pub mod tree;

pub use branch::{BRANCHES, Branch, BranchInfo, BranchKind};
pub use cost::{Buildings, Cost, Requirement};
pub use effect::{EFFECTS, EffectCategory};
pub use gate::Refusal;
pub use progress::{Effects, Progress, State};
pub use tree::{Flags, LoadError, Research, Tree, Unlock};

/// How many research files the Java data holds, and how many rows
/// [`tree::TABLE`] carries. Kept next to the table so a row added without the
/// count shows up as a failing test rather than as a silent difference.
pub const RESEARCH_COUNT: usize = 217;

/// How many branch declarations `researches/*.json` holds, one per branch.
pub const BRANCH_COUNT: usize = 4;

/// How many effect categories `researches/effects/*.json` holds.
pub const EFFECT_COUNT: usize = 106;

/// How many researches have no parent — the tree's 19 roots, by branch 6
/// civilian, 5 combat, 7 technology and 1 unlockable.
pub const ROOT_COUNT: usize = 19;

/// The deepest `researchLevel` the data uses, and the ceiling Java enforces:
/// `MAX_DEPTH` in `api/research/util/ResearchConstants.java:74`. A depth-6
/// research is also the one that closes its branch (one per branch, see
/// [`Progress::branch_ceiling`]).
pub const MAX_DEPTH: u8 = 6;

/// One unit of research time, `BASE_RESEARCH_TIME`
/// (`api/research/util/ResearchConstants.java:25`): `60 * 60 / 25 / 2` = **72**.
/// A depth-1 research of a branch with `base-time: 1.0` therefore needs 72
/// study actions, a depth-`d` one `72 * 2^(d-1)`.
pub const BASE_RESEARCH_TIME: u32 = 60 * 60 / 25 / 2;

/// The top built level of a university, `CONST_DEFAULT_MAX_BUILDING_LEVEL`
/// (`api/util/constant/BuildingConstants.java:11`; `BuildingUniversity` does not
/// override `AbstractBuilding.getMaxBuildingLevel`, `:1154-1158`). It matters
/// twice: a research of depth `d` needs a university of level `d`, and depth 6
/// is out of reach of a level-6 check — see [`gate::university_level`].
pub const MAX_UNIVERSITY_LEVEL: i32 = 5;

/// The delay between two study actions, `STUDY_DELAY`
/// (`core/entity/ai/workers/education/EntityAIWorkResearcher.java:24`), in ticks.
/// [`Branch::ticks`] counts those actions, not ticks: the two are 1 200 apart.
pub const STUDY_TICKS: u32 = 1200;

#[cfg(test)]
mod tests {
    use super::*;

    /// The three counts are the ones the data directory actually holds. A row
    /// added to `tree::TABLE` without this number moves it, and the four
    /// branch lines break with it.
    #[test]
    fn the_counts_match_the_data_directory() {
        let tree = Tree::load().expect("the shipped table loads");
        assert_eq!(tree.len(), RESEARCH_COUNT);
        assert_eq!(BRANCHES.len(), BRANCH_COUNT);
        assert_eq!(EFFECTS.len(), EFFECT_COUNT);
        assert_eq!(tree.roots().count(), ROOT_COUNT);
        assert_eq!(RESEARCH_COUNT, 327 - BRANCH_COUNT - EFFECT_COUNT);
    }

    /// The four counts per branch — and the one row where the two readings of
    /// "which branch" disagree. Java keys the tree by the `branch` field
    /// (`ResearchListener.java:245`), never by the folder, and `civilian/moq`
    /// declares `minecolonies:technology` while sitting in `civilian/`.
    #[test]
    fn the_branches_hold_their_rows_and_one_row_moved_branch() {
        let tree = Tree::load().expect("the shipped table loads");
        let by_field: Vec<(Branch, usize)> = BRANCHES
            .iter()
            .map(|info| (info.branch, tree.in_branch(info.branch).count()))
            .collect();
        assert_eq!(
            by_field,
            vec![
                (Branch::Civilian, 68),
                (Branch::Combat, 68),
                (Branch::Technology, 77),
                (Branch::Unlockable, 4),
            ]
        );
        let by_folder = |folder: &str| {
            tree.all()
                .filter(|research| research.id.starts_with(folder))
                .count()
        };
        assert_eq!(
            (
                by_folder("civilian/"),
                by_folder("combat/"),
                by_folder("technology/"),
                by_folder("unlockable/")
            ),
            (69, 68, 76, 4),
            "the folders, which is not the same count"
        );
    }

    /// `BASE_RESEARCH_TIME` is Java's own arithmetic, not a number retyped from
    /// a comment: `60 * 60 / 25 / 2`.
    #[test]
    fn the_base_time_is_java_arithmetic() {
        assert_eq!(BASE_RESEARCH_TIME, 72);
        assert_eq!(60 * 60 / 25 / 2, 72);
    }

    /// No research reaches past `MAX_DEPTH`, and every depth from 1 to 6 is used
    /// — the table is not a sparse sample.
    #[test]
    fn the_depths_reach_the_maximum_and_no_further() {
        let tree = Tree::load().expect("the shipped table loads");
        let mut seen = [0usize; 7];
        for research in tree.all() {
            assert!(
                research.depth >= 1 && research.depth <= MAX_DEPTH,
                "{} is at depth {}",
                research.id,
                research.depth
            );
            seen[research.depth as usize] += 1;
        }
        assert_eq!(seen[1], 19);
        assert_eq!(seen[2], 48);
        assert_eq!(seen[3], 54);
        assert_eq!(seen[4], 38);
        assert_eq!(seen[5], 31);
        assert_eq!(seen[6], 27);
        assert_eq!(seen.iter().sum::<usize>(), RESEARCH_COUNT);
    }
}
