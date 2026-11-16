//! The four branches, their base time, and how long a research of a given depth
//! takes.
//!
//! Java reference: `core/research/GlobalResearchBranch.java` (225 lines),
//! `api/research/ResearchBranchType.java` (the two branch kinds), and the four
//! declarations `researches/{civilian,combat,technology,unlockable}.json`.
//!
//! **What the JSON actually says.** All four declare `branch-name` and
//! `sortOrder`; three declare `base-time: 1.0` and only `unlockable` declares
//! `0.0`, `branch-type: "unlockables"` and `hidden: true`. None declares a
//! `subtitle`. So the branch table is four rows and the defaults of
//! `GlobalResearchBranch.java:105-131` — a branch without a declared `sortOrder`
//! sorts at 1000, `base-time` at 1.0, type `DEFAULT`, not hidden — only ever
//! apply to a branch a data pack adds.
//!
//! **The time formula is Java's, and it is quadratic per depth.**
//! `getBaseTime(depth)` is `(int)(BASE_RESEARCH_TIME * baseTime * 2^(depth-1))`
//! (`:83-86`), so 72 study actions at depth 1, 144 at depth 2, and 2 304 at depth
//! 6. [`Branch::ticks`] is that number: **study actions**, not ticks — one
//! action is one pass through `EntityAIWorkResearcher.study`
//! (`core/entity/ai/workers/education/EntityAIWorkResearcher.java:84-99`, one per
//! 1 200 ticks and one point of mana).
//!
//! **`hours` is the GUI's estimate, and it is not the same clock.** Java's
//! `getHoursTime` multiplies by 25 and divides by 3 600 (`:89-92`), i.e. it
//! prices one study action at 25 seconds, while the worker takes 1 200 ticks
//! (60 s) per action — the GUI's own comment says "treat BASE_RESEARCH_TIME as
//! _roughly_ equal to one half-hour" (`core/client/gui/WindowResearchTree.java:719-722`).
//! Both numbers are ported, [`Branch::seconds`] says how far apart they are, and
//! the report states it rather than picking one.

use super::{BASE_RESEARCH_TIME, STUDY_TICKS};

/// The four branches the data declares, in `sortOrder` order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: the four files the data ships, not a `mod_api` type
pub enum Branch {
    Civilian,
    Combat,
    Technology,
    Unlockable,
}

/// What a branch's declaration says, field for field
/// (`GlobalResearchBranch.java:105-131`, `:78-96` for the two readers).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BranchInfo {
    pub branch: Branch,
    /// The file this row was read from, relative to the repository root. The
    /// row is a constant, so rule 3 wants its source next to it rather than in a
    /// header; `branch` is also the file's name.
    pub source: &'static str,
    /// `branch-name`, a translation key in all four files.
    pub name_key: &'static str,
    /// `base-time`, the multiplier [`Branch::ticks`] applies.
    pub base_time: f64,
    /// `sortOrder`: 50, 100, 150, 200 — higher goes toward the bottom of the
    /// university GUI (`GlobalResearchBranch.java:44-46`).
    pub sort: u16,
    /// `branch-type`; only `unlockable` is anything but `DEFAULT`.
    pub kind: BranchKind,
    /// `hidden`: the branch is not listed until one of its researches is
    /// visible. Only `unlockable` sets it.
    pub hidden: bool,
}

/// `ResearchBranchType` (`api/research/ResearchBranchType.java:13-33`). The
/// distinction is not cosmetic: an `UNLOCKABLES` branch skips the university
/// level check when its research autostarts
/// (`core/colony/managers/ResearchManager.java:152-166`), which is the only
/// reason the four `unlockable/` rows can fire at all — they are depth 1, 2, 3
/// and 4 and one of them wants a level-5 sifter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two tags the enum declares
pub enum BranchKind {
    /// `"default"` — tree view, depth enforced, one depth-6 research.
    Default,
    /// `"unlockables"` — same tree, but no university-level gate on autostart.
    Unlockables,
}

/// The four declarations, in file order.
pub const BRANCHES: &[BranchInfo] = &[
    // researches/civilian.json
    BranchInfo {
        branch: Branch::Civilian,
        source: "src/datagen/generated/minecolonies/data/minecolonies/researches/civilian.json",
        name_key: "com.minecolonies.research.civilian.name",
        base_time: 1.0,
        sort: 50,
        kind: BranchKind::Default,
        hidden: false,
    },
    // researches/combat.json
    BranchInfo {
        branch: Branch::Combat,
        source: "src/datagen/generated/minecolonies/data/minecolonies/researches/combat.json",
        name_key: "com.minecolonies.research.combat.name",
        base_time: 1.0,
        sort: 100,
        kind: BranchKind::Default,
        hidden: false,
    },
    // researches/technology.json
    BranchInfo {
        branch: Branch::Technology,
        source: "src/datagen/generated/minecolonies/data/minecolonies/researches/technology.json",
        name_key: "com.minecolonies.research.technology.name",
        base_time: 1.0,
        sort: 150,
        kind: BranchKind::Default,
        hidden: false,
    },
    // researches/unlockable.json — the only one with a zero base time, a
    // branch type and `hidden`.
    BranchInfo {
        branch: Branch::Unlockable,
        source: "src/datagen/generated/minecolonies/data/minecolonies/researches/unlockable.json",
        name_key: "com.minecolonies.research.unlockable.name",
        base_time: 0.0,
        sort: 200,
        kind: BranchKind::Unlockables,
        hidden: true,
    },
];

impl Branch {
    /// The branch id's path, as `minecolonies:civilian` spells it.
    pub fn path(self) -> &'static str {
        match self {
            Branch::Civilian => "civilian",
            Branch::Combat => "combat",
            Branch::Technology => "technology",
            Branch::Unlockable => "unlockable",
        }
    }

    /// The one-letter tag [`super::tree::TABLE`] uses. Not Java's: Java always
    /// writes the full `namespace:path` (`GlobalResearchFactory.java:66-68`), and
    /// the short tag is only the table's own encoding, read back by
    /// [`Self::from_tag`].
    pub fn tag(self) -> char {
        match self {
            Branch::Civilian => 'c',
            Branch::Combat => 'k',
            Branch::Technology => 't',
            Branch::Unlockable => 'u',
        }
    }

    /// The branch a path names, or `None`. A data pack may move a research to a
    /// branch of its own; this port knows the four the shipped data declares and
    /// says so instead of inventing a fifth.
    pub fn from_path(path: &str) -> Option<Branch> {
        BRANCHES
            .iter()
            .find(|info| info.branch.path() == path)
            .map(|info| info.branch)
    }

    /// The branch a one-letter tag names.
    pub fn from_tag(tag: &str) -> Option<Branch> {
        let mut chars = tag.chars();
        let first = chars.next()?;
        if chars.next().is_some() {
            return None;
        }
        BRANCHES
            .iter()
            .find(|info| info.branch.tag() == first)
            .map(|info| info.branch)
    }

    /// This branch's declaration.
    pub fn info(self) -> &'static BranchInfo {
        BRANCHES
            .iter()
            .find(|info| info.branch == self)
            // Unreachable: `BRANCHES` has one row per variant, checked by a test.
            .unwrap_or(&BRANCHES[0])
    }

    /// How many study actions a research of this depth needs:
    /// `(int)(BASE_RESEARCH_TIME * baseTime * 2^(depth-1))`
    /// (`GlobalResearchBranch.java:83-86`).
    ///
    /// The truncation is Java's `(int)` cast, and it is visible: a `base-time` of
    /// `0.6` at depth 1 gives 43, not 43.2. `depth` is not policed here — the
    /// tree is the only producer of a depth, and it refuses anything outside
    /// `1..=MAX_DEPTH`. A hypothetical depth 0 halves, exactly as `Math.pow(2,
    /// -1)` would.
    pub fn ticks(self, depth: u8) -> u32 {
        let factor = 2f64.powi(i32::from(depth) - 1);
        let value = f64::from(BASE_RESEARCH_TIME) * self.info().base_time * factor;
        if value <= 0.0 {
            // Java casts -0.5 or 0.0 to 0; `as` would saturate a negative to 0
            // as well, so this is only here to keep the intent readable.
            return 0;
        }
        value as u32
    }

    /// The GUI's estimate of that time in hours: `ticks * 25 / 60 / 60`
    /// (`GlobalResearchBranch.java:89-92`).
    pub fn hours(self, depth: u8) -> f64 {
        f64::from(self.ticks(depth)) * 25.0 / 60.0 / 60.0
    }

    /// The same time as a wall clock, at one study action per [`STUDY_TICKS`]
    /// (1 200 ticks, 20 ticks per second). **This is not Java's `getHoursTime`**
    /// — the gap is exactly `STUDY_TICKS / 20 / 25` = 2.4: a level-1 civilian
    /// research is half an hour by Java's estimate and 1.2 h of study actions.
    pub fn seconds(self, depth: u8) -> u64 {
        u64::from(self.ticks(depth)) * u64::from(STUDY_TICKS) / 20
    }

    /// A branch whose researches are unlockables rather than a tree a player
    /// buys into (`ResearchBranchType.UNLOCKABLES`).
    pub fn is_showcase(self) -> bool {
        self.info().kind == BranchKind::Unlockables
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_branch_has_one_row_and_one_tag() {
        assert_eq!(BRANCHES.len(), 4);
        for info in BRANCHES {
            assert_eq!(info.branch.info().branch, info.branch);
            assert!(
                info.source.ends_with(".json")
                    && info.source.contains(info.branch.path()),
                "{} cites {}",
                info.branch.path(),
                info.source
            );
            assert_eq!(Branch::from_path(info.branch.path()), Some(info.branch));
            assert_eq!(
                Branch::from_tag(&info.branch.tag().to_string()),
                Some(info.branch)
            );
        }
    }

    #[test]
    fn the_sort_orders_are_the_files_own() {
        assert_eq!(Branch::Civilian.info().sort, 50);
        assert_eq!(Branch::Combat.info().sort, 100);
        assert_eq!(Branch::Technology.info().sort, 150);
        assert_eq!(Branch::Unlockable.info().sort, 200);
    }

    #[test]
    fn only_the_unlockable_branch_is_zero_time_hidden_and_typed() {
        let unlockable = Branch::Unlockable.info();
        assert_eq!(unlockable.base_time, 0.0);
        assert!(unlockable.hidden);
        assert_eq!(unlockable.kind, BranchKind::Unlockables);
        for branch in [Branch::Civilian, Branch::Combat, Branch::Technology] {
            let info = branch.info();
            assert_eq!(info.base_time, 1.0, "{}", branch.path());
            assert!(!info.hidden, "{}", branch.path());
            assert_eq!(info.kind, BranchKind::Default, "{}", branch.path());
        }
    }

    #[test]
    fn the_time_doubles_with_each_depth() {
        assert_eq!(Branch::Civilian.ticks(1), 72);
        assert_eq!(Branch::Civilian.ticks(2), 144);
        assert_eq!(Branch::Civilian.ticks(3), 288);
        assert_eq!(Branch::Civilian.ticks(4), 576);
        assert_eq!(Branch::Civilian.ticks(5), 1152);
        assert_eq!(Branch::Civilian.ticks(6), 2304);
        // `2^(d-1)`, so depth 0 halves rather than saturating.
        assert_eq!(Branch::Civilian.ticks(0), 36);
    }

    #[test]
    fn all_three_time_branches_share_the_same_scale() {
        for branch in [Branch::Civilian, Branch::Combat, Branch::Technology] {
            for depth in 1..=6u8 {
                assert_eq!(branch.ticks(depth), 72 << (depth - 1), "{branch:?}");
            }
        }
    }

    #[test]
    fn every_unlockable_depth_is_instant() {
        for depth in 1..=6u8 {
            assert_eq!(Branch::Unlockable.ticks(depth), 0);
            assert_eq!(Branch::Unlockable.hours(depth), 0.0);
            assert_eq!(Branch::Unlockable.seconds(depth), 0);
        }
    }

    #[test]
    fn hours_is_javas_estimate_and_seconds_is_the_real_clock() {
        assert_eq!(Branch::Civilian.hours(1), 0.5);
        assert_eq!(Branch::Civilian.hours(2), 1.0);
        // 72 actions at 1 200 ticks, 20 ticks per second.
        assert_eq!(Branch::Civilian.seconds(1), 4_320);
        // The GUI's 25 s per action against the worker's 60 s.
        assert_eq!(f64::from(STUDY_TICKS) / 20.0 / 25.0, 2.4);
        assert_eq!(
            Branch::Civilian.seconds(1) as f64 / Branch::Civilian.hours(1) / 3600.0,
            2.4
        );
    }

    #[test]
    fn the_unlockable_branch_is_the_only_showcase() {
        assert!(Branch::Unlockable.is_showcase());
        assert!(!Branch::Civilian.is_showcase());
        assert!(!Branch::Combat.is_showcase());
        assert!(!Branch::Technology.is_showcase());
    }

    #[test]
    fn a_tag_of_the_wrong_shape_is_no_branch() {
        assert_eq!(Branch::from_tag(""), None);
        assert_eq!(Branch::from_tag("civilian"), None);
        assert_eq!(Branch::from_tag("z"), None);
        assert_eq!(Branch::from_path("CIVILIAN"), None);
        assert_eq!(Branch::from_path("unlockables"), None);
    }
}
