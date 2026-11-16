//! Why a research cannot start, or cannot be undone — named.
//!
//! Java reference: `core/research/GlobalResearch.java:156-260` (`canResearch`,
//! `canDisplay`, `hasEnoughResources`) and
//! `core/research/LocalResearchTree.java:147-320` (`attemptBeginResearch`,
//! `attemptResetResearch`).
//!
//! **Java splits the gate in two, and so does this file.** `canResearch` is what
//! the window asks before drawing a button (`WindowResearchTree.java:440-460`):
//! not already known, deep enough a university, parent finished, the parent's
//! exclusive child not taken, the branch's depth-6 slot free. `attemptBeginResearch`
//! is what the server asks on the click: same presence check, then the cost,
//! then the requirements. [`may_begin`] runs the five tree rules and [`pay`]
//! the cost; [`Refusal`] has one variant per way either can say no.
//!
//! **The university level is not a level.** `canResearch` replaces the building's
//! level by `Integer.MAX_VALUE` when it is **at its maximum**
//! (`GlobalResearch.java:158`), so a level-5 university unlocks depth 6 — a level
//! the number alone would never reach, since `MAX_UNIVERSITY_LEVEL` is 5 and
//! `MAX_DEPTH` is 6. [`university_level`] is that substitution, and a test says a
//! level-5 university answers 6 and a level-4 answers 4.
//!
//! **The depth-6 slot is taken by *starting*, not by finishing.**
//! `LocalResearchTree.addResearch` adds the branch to
//! `maxLevelResearchCompleted` as soon as a depth-6 record is added, whatever its
//! state (`:113-116`), and takes it back out on a reset (`:299-305`). So the
//! second depth-6 research of a branch is refused while the first is still
//! running; [`super::progress::Progress::branch_ceiling`] keeps that meaning and
//! the name Java uses is the name it does not mean.
//!
//! **What is not here.** The `hide` rule of the window
//! (`WindowResearchTree.java:285`: a `hidden` research is drawn only when its
//! requirements are met) is a display decision, [`visible`] answers it, and
//! nothing else uses it. The reset *cost* (`GlobalResearchTree.java:302-344`, a
//! server config list) is not modelled: [`may_undo`] refuses what the tree
//! refuses and charges nothing, and the module documentation says so rather than
//! pricing the undo at zero.

use super::cost::{Buildings, Cost, Requirement};
use super::progress::{Progress, State};
use super::tree::{Research, Tree};
use super::branch::Branch;
use super::{MAX_DEPTH, MAX_UNIVERSITY_LEVEL};
use crate::inventory::{Inventory, ItemName};

/// Why a research operation was refused, named.
///
/// A variant is **not** a `lang` key: those live in `assets/minecolonies/lang/`,
/// which this module does not own. The name is for logs and tests, namespaced so
/// it can never be mistaken for a sentence.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// A record already exists for this research, whatever its state
    /// (`GlobalResearch.java:160`, `LocalResearchTree.java:200-205`:
    /// "alreadystarted").
    AlreadyStarted { id: &'static str },
    /// The id is not in the tree. Java would dereference `null`
    /// (`GlobalResearchTree.java:126-135`); the port names it.
    UnknownResearch { id: String },
    /// The university is not deep enough for this research's level
    /// (`GlobalResearch.java:167-172`, "researchLevel" in the GUI headers).
    LevelTooLow { depth: u8, university: i32 },
    /// The parent exists in the tree but is not finished
    /// (`:163-164`).
    ParentUnfinished { parent: &'static str },
    /// The parent allows one researched child and another one is already
    /// recorded (`:164-165`, `:325-341`).
    SiblingChosen {
        parent: &'static str,
        chosen: String,
    },
    /// One depth-6 research of this branch is already recorded, so this branch
    /// cannot take a second (`:165`, `MAX_DEPTH` in
    /// `api/research/util/ResearchConstants.java:74`).
    BranchCeiling { branch: Branch },
    /// The cost is not in the university and the player together
    /// (`GlobalResearch.java:183-230`, "costnotavailable").
    CostMissing {
        need: u32,
        missing: u32,
        what: String,
    },
    /// A `requirements` entry is not met (`LocalResearchTree.java:174-186`,
    /// "requirementnotmet").
    RequirementUnmet { what: String },
    /// A tag cost names a tag this port does not carry
    /// (`ForgeRegistries.ITEMS.tags()`, `TagItemCost.java:58-61`).
    TagUnknown { tag: String },
    /// An item id the engine cannot parse. Java's registry returns `null` and
    /// the research costs nothing (`SimpleItemCost.java:56-59`); the port refuses.
    ItemUnparsable { item: String },
    /// [`Progress::advance`] was handed an id that is not running.
    NotRunning { id: String },
    /// [`Progress::undo`] was handed a research that is not finished.
    NotFinished { id: String },
    /// `no-reset` (`ResearchListener.java:136`): the undo button does not exist
    /// (`WindowResearchTree.java:228-234`).
    Immutable { id: &'static str },
    /// `autostart`: undoing it would restart it on the next reload, so Java
    /// suppresses the button too (`:230-234`).
    AutoStarts { id: &'static str },
    /// A child is recorded, so the parent must be undone last
    /// (`WindowResearchTree.java:235-243`: "haschilds").
    ChildInTheWay { child: &'static str },
}

impl Refusal {
    /// A stable name for logs and tests, namespaced like a registry entry.
    pub fn name(&self) -> &'static str {
        match self {
            Refusal::AlreadyStarted { .. } => "minecolonies:research/already-started",
            Refusal::UnknownResearch { .. } => "minecolonies:research/unknown",
            Refusal::LevelTooLow { .. } => "minecolonies:research/level-too-low",
            Refusal::ParentUnfinished { .. } => "minecolonies:research/parent-unfinished",
            Refusal::SiblingChosen { .. } => "minecolonies:research/sibling-chosen",
            Refusal::BranchCeiling { .. } => "minecolonies:research/branch-ceiling",
            Refusal::CostMissing { .. } => "minecolonies:research/cost-missing",
            Refusal::RequirementUnmet { .. } => "minecolonies:research/requirement-unmet",
            Refusal::TagUnknown { .. } => "minecolonies:research/tag-unknown",
            Refusal::ItemUnparsable { .. } => "minecolonies:research/item-unparsable",
            Refusal::NotRunning { .. } => "minecolonies:research/not-running",
            Refusal::NotFinished { .. } => "minecolonies:research/not-finished",
            Refusal::Immutable { .. } => "minecolonies:research/immutable",
            Refusal::AutoStarts { .. } => "minecolonies:research/auto-starts",
            Refusal::ChildInTheWay { .. } => "minecolonies:research/child-in-the-way",
        }
    }

    /// The id the refusal is about, when it has one.
    pub fn about(&self) -> Option<&str> {
        match self {
            Refusal::AlreadyStarted { id }
            | Refusal::Immutable { id }
            | Refusal::AutoStarts { id }
            | Refusal::ChildInTheWay { child: id } => Some(id),
            Refusal::ParentUnfinished { parent } => Some(parent),
            Refusal::UnknownResearch { id } | Refusal::NotRunning { id } | Refusal::NotFinished { id } => {
                Some(id)
            }
            _ => None,
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let detail = match self {
            Refusal::AlreadyStarted { id } => format!("{id} already has a record"),
            Refusal::UnknownResearch { id } => format!("{id} is not in the tree"),
            Refusal::LevelTooLow { depth, university } => {
                format!("depth {depth} needs a university at {depth}, this one is {university}")
            }
            Refusal::ParentUnfinished { parent } => format!("{parent} is not finished"),
            Refusal::SiblingChosen { parent, chosen } => {
                format!("{parent} allows one child and {chosen} took it")
            }
            Refusal::BranchCeiling { branch } => {
                format!("{} already has a finished depth-6 research", branch.path())
            }
            Refusal::CostMissing {
                need,
                missing,
                what,
            } => format!("{what}: {missing} of {need} missing"),
            Refusal::RequirementUnmet { what } => format!("{what} is not met"),
            Refusal::TagUnknown { tag } => format!("no tag {tag}"),
            Refusal::ItemUnparsable { item } => format!("the engine cannot parse {item}"),
            Refusal::NotRunning { id } => format!("{id} is not running"),
            Refusal::NotFinished { id } => format!("{id} is not finished"),
            Refusal::Immutable { id } => format!("{id} cannot be undone"),
            Refusal::AutoStarts { id } => format!("{id} would restart itself"),
            Refusal::ChildInTheWay { child } => format!("{child} must be undone first"),
        };
        write!(formatter, "{}: {detail}", self.name())
    }
}

/// The level a research may be compared against: Java's
/// `level == max ? Integer.MAX_VALUE : level` (`GlobalResearch.java:158`), which
/// is how a maximum-level university reaches depth 6.
pub fn university_level(level: i32, max: i32) -> i32 {
    if level >= max {
        i32::MAX
    } else {
        level.max(0)
    }
}

/// Can this research be *shown* at that university level?
/// `GlobalResearch.canDisplay`, `:167-172`.
pub fn can_display(research: &Research, university_level: i32) -> bool {
    university_level >= i32::from(research.depth)
}

/// Is it drawn at all? A `hidden` research is drawn only when its requirements
/// are already met (`WindowResearchTree.java:285`) — the four `unlockable` rows
/// say so, and a hidden research a player cannot see is not a refusal.
pub fn visible(research: &Research, standing: &Buildings, finished: &dyn Fn(&str) -> bool) -> bool {
    !research.flags.hidden()
        || research
            .requirements
            .iter()
            .all(|requirement| requirement.met_with(standing, finished))
}

/// The five tree rules of `canResearch` (`:156-166`), in Java's order.
pub fn may_begin(
    tree: &Tree,
    research: &Research,
    university_level: i32,
    progress: &Progress,
) -> Result<(), Refusal> {
    if progress.state(research.id) != State::NotStarted {
        return Err(Refusal::AlreadyStarted { id: research.id });
    }
    if !can_display(research, university_level) {
        return Err(Refusal::LevelTooLow {
            depth: research.depth,
            university: university_level,
        });
    }
    if let Some(parent) = research.parent {
        let parent_row = tree.get(parent).ok_or_else(|| Refusal::UnknownResearch {
            id: parent.to_string(),
        })?;
        if progress.state(parent) != State::Finished {
            return Err(Refusal::ParentUnfinished { parent: parent_row.id });
        }
        if parent_row.flags.exclusive_child() {
            if let Some(chosen) = progress.recorded_child(tree, parent_row.id) {
                return Err(Refusal::SiblingChosen {
                    parent: parent_row.id,
                    chosen: chosen.to_string(),
                });
            }
        }
    }
    if research.depth >= MAX_DEPTH {
        if let Some(_taken) = progress.branch_ceiling(research.branch) {
            return Err(Refusal::BranchCeiling {
                branch: research.branch,
            });
        }
    }
    Ok(())
}

/// Every `requirements` entry, with the research it may name read out of the
/// same progress (`LocalResearchTree.java:174-186`).
pub fn requirements_met(research: &Research, standing: &Buildings, progress: &Progress) -> Result<(), Refusal> {
    for requirement in &research.requirements {
        if !requirement.met_with(standing, &|id| progress.is_finished(id)) {
            return Err(Refusal::RequirementUnmet {
                what: requirement.what(),
            });
        }
    }
    Ok(())
}

/// Can the university and the player pay for it, in that order
/// (`GlobalResearch.java:183-230`)?
pub fn affordable(
    research: &Research,
    university: &Inventory<'_>,
    player: &Inventory<'_>,
) -> Result<(), Refusal> {
    for cost in &research.costs {
        cost.met(university, player)?;
    }
    Ok(())
}

/// Takes the cost out of the university first, then the player —
/// `InventoryUtils.reduceBuildingThenPlayerInventory`
/// (`LocalResearchTree.java:196-202`).
///
/// Returns what was actually taken, one `(item, count)` pair per item touched,
/// so a caller can put it back. A shortfall cannot be reached here:
/// [`affordable`] is the check and Java calls it first (`:170-177`); if a caller
/// skips that, the loop simply takes what is there and the record says less.
pub fn pay(
    research: &Research,
    university: &mut Inventory<'_>,
    player: &mut Inventory<'_>,
) -> Result<Vec<(String, u32)>, Refusal> {
    let mut taken: Vec<(String, u32)> = Vec::new();
    for cost in &research.costs {
        let mut left = cost.count();
        for member in cost.members()? {
            if left == 0 {
                break;
            }
            let name = ItemName::new(member).ok_or_else(|| Refusal::ItemUnparsable {
                item: member.to_string(),
            })?;
            // L'universite d'abord, puis le joueur (`LocalResearchTree.java:196-202`). Deux blocs
            // successifs plutot qu'un tableau de deux `&mut` : le tableau unifierait leurs durees de vie.
            if left > 0 {
                if let Ok(stack) = university.extract(&name, left) {
                    left -= stack.count();
                    taken.push((member.to_string(), stack.count()));
                }
            }
            if left > 0 {
                if let Ok(stack) = player.extract(&name, left) {
                    left -= stack.count();
                    taken.push((member.to_string(), stack.count()));
                }
            }
        }
    }
    Ok(taken)
}

/// Every rule `attemptBeginResearch` applies before it writes a record
/// (`LocalResearchTree.java:147-210`), in its order: the tree, then the cost,
/// then the requirements.
pub fn may_start(
    tree: &Tree,
    research: &Research,
    university_level: i32,
    progress: &Progress,
    standing: &Buildings,
    university: &Inventory<'_>,
    player: &Inventory<'_>,
) -> Result<(), Refusal> {
    may_begin(tree, research, university_level, progress)?;
    affordable(research, university, player)?;
    requirements_met(research, standing, progress)
}

/// Why a finished research cannot be undone, or `Ok(())` when it can
/// (`WindowResearchTree.java:228-243`, `LocalResearchTree.java:225-320`).
pub fn may_undo(tree: &Tree, research: &Research, progress: &Progress) -> Result<(), Refusal> {
    // `no-reset` is the first thing Java checks (`:231`), and `autostart` the
    // second: undoing one would restart it on the next reload.
    if research.flags.immutable() {
        return Err(Refusal::Immutable { id: research.id });
    }
    if research.flags.autostarts() {
        return Err(Refusal::AutoStarts { id: research.id });
    }
    if let Some(child) = progress.recorded_child(tree, research.id) {
        return Err(Refusal::ChildInTheWay { child });
    }
    Ok(())
}

/// The one cost of a research that a tag names, for a caller that wants to show
/// it. Kept here so `cost` does not need to know about [`Refusal`]'s plumbing
/// twice.
pub fn tag_of(cost: &Cost) -> Option<&'static str> {
    match cost {
        Cost::Tag { tag, .. } => Some(tag),
        _ => None,
    }
}

/// The highest depth a university of that level can reach, which is the number
/// the GUI prints in its level headers (`WindowResearchTree.java:330-350`).
pub fn reachable_depth(level: i32, max: i32) -> u8 {
    let level = university_level(level, max);
    if level >= i32::from(MAX_DEPTH) {
        MAX_DEPTH
    } else {
        u8::try_from(level).unwrap_or(0)
    }
}

/// The maximum level of a university, re-exported so a caller comparing levels
/// does not have to know which building it is.
pub const MAX_LEVEL: i32 = MAX_UNIVERSITY_LEVEL;

/// A requirement's kind, for a caller that wants to group them. `None` for the
/// research form, which names a research rather than a building.
pub fn requirement_building(requirement: &Requirement) -> Option<&'static str> {
    match requirement {
        Requirement::Building { building, .. } | Requirement::SingleBuilding { building, .. } => {
            Some(building)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{SingleLimit, Stack};

    fn tree() -> Tree {
        Tree::load().expect("the shipped table loads")
    }

    fn row(id: &str) -> Research {
        tree().get(id).expect("declared").clone()
    }

    /// A finished record, the state most of these tests need behind them.
    fn done(progress: &mut Progress, research: &Research) {
        progress.record(research, State::Finished, research.ticks());
    }

    fn item(name: &str) -> ItemName {
        ItemName::new(name).expect("a name the engine can parse")
    }

    fn inventory(contents: &[(&str, u32)]) -> Inventory<'static> {
        static LIMITS: SingleLimit = SingleLimit(64);
        let slots = contents
            .iter()
            .map(|(name, count)| Some(Stack::new(item(name), *count).expect("a positive count")))
            .collect();
        Inventory::with_slots(slots, &LIMITS)
    }

    fn empty() -> Inventory<'static> {
        static LIMITS: SingleLimit = SingleLimit(64);
        Inventory::new(8, &LIMITS)
    }

    #[test]
    fn the_maximum_level_university_reaches_depth_six() {
        assert_eq!(university_level(5, 5), i32::MAX);
        assert_eq!(university_level(4, 5), 4);
        assert_eq!(university_level(0, 5), 0);
        assert_eq!(university_level(-3, 5), 0);
        assert_eq!(reachable_depth(5, 5), 6);
        assert_eq!(reachable_depth(4, 5), 4);
        assert_eq!(reachable_depth(1, 5), 1);
    }

    #[test]
    fn a_depth_six_research_is_displayed_only_by_a_maximum_university() {
        let academic = row("civilian/academic");
        assert_eq!(academic.depth, 6);
        assert!(!can_display(&academic, 5));
        assert!(can_display(&academic, university_level(5, 5)));
    }

    #[test]
    fn a_research_already_recorded_is_refused_first() {
        let tree = tree();
        let keen = row("civilian/keen");
        let mut progress = Progress::new();
        progress.record(&keen, State::InProgress, 0);
        assert_eq!(
            may_begin(&tree, &keen, 5, &progress),
            Err(Refusal::AlreadyStarted { id: "civilian/keen" })
        );
    }

    #[test]
    fn a_university_too_shallow_is_named_with_both_numbers() {
        let tree = tree();
        let keen = row("civilian/keen");
        let progress = Progress::new();
        assert_eq!(
            may_begin(&tree, &keen, 0, &progress),
            Err(Refusal::LevelTooLow {
                depth: 1,
                university: 0
            })
        );
    }

    #[test]
    fn an_unfinished_parent_blocks_the_child() {
        let tree = tree();
        let interesting = row("civilian/bookworm");
        let progress = Progress::new();
        assert_eq!(
            may_begin(&tree, &interesting, 5, &progress),
            Err(Refusal::ParentUnfinished {
                parent: "civilian/morebooks"
            })
        );
    }

    #[test]
    fn a_finished_parent_lets_the_child_through() {
        let tree = tree();
        let interesting = row("civilian/bookworm");
        let mut progress = Progress::new();
        done(&mut progress, &row("civilian/morebooks"));
        assert_eq!(may_begin(&tree, &interesting, 5, &progress), Ok(()));
    }

    #[test]
    fn an_exclusive_parent_takes_one_child() {
        let tree = tree();
        let precise = row("combat/preciseshot");
        let mut progress = Progress::new();
        done(&mut progress, &row("combat/accuracy"));
        done(&mut progress, &row("combat/quickdraw"));
        assert_eq!(
            may_begin(&tree, &precise, 5, &progress),
            Err(Refusal::SiblingChosen {
                parent: "combat/accuracy",
                chosen: "combat/quickdraw".to_string()
            })
        );
    }

    #[test]
    fn a_parent_that_is_not_exclusive_takes_several() {
        let tree = tree();
        // `combat/tactictraining` declares no `exclusiveChildResearch` and has
        // four children: `coffee`, `improvedbows`, `improvedswords` and
        // `mountedcombat`.
        let third = row("combat/mountedcombat");
        let mut progress = Progress::new();
        done(&mut progress, &row("combat/tactictraining"));
        done(&mut progress, &row("combat/improvedbows"));
        done(&mut progress, &row("combat/improvedswords"));
        assert_eq!(
            may_begin(&tree, &third, 5, &progress),
            Ok(()),
            "tactictraining allows every child it has"
        );
    }

    #[test]
    fn the_branch_ceiling_is_taken_by_starting_not_by_finishing() {
        let tree = tree();
        let athlete = row("civilian/athlete");
        assert_eq!(athlete.depth, 6);
        let mut progress = Progress::new();
        done(&mut progress, &row("civilian/swift"));
        let homework = row("civilian/beanstalk");
        // `beanstalk` is the other depth-6 of the civilian branch, still running.
        progress.record(&homework, State::InProgress, 10);
        assert!(
            progress.state("civilian/beanstalk") == State::InProgress,
            "a running depth-6 record closes the branch"
        );
        assert_eq!(
            may_begin(&tree, &athlete, university_level(5, 5), &progress),
            Err(Refusal::BranchCeiling {
                branch: Branch::Civilian
            })
        );
    }

    #[test]
    fn a_missing_cost_is_named_with_the_item_and_the_shortfall() {
        let tree = tree();
        let keen = row("civilian/keen");
        let progress = Progress::new();
        let mut standing = Buildings::new();
        standing.add("residence", 3);
        let refusal = may_start(
            &tree,
            &keen,
            5,
            &progress,
            &standing,
            &empty(),
            &inventory(&[("minecraft:book", 1)]),
        )
        .expect_err("two books short");
        assert_eq!(
            refusal,
            Refusal::CostMissing {
                need: 3,
                missing: 2,
                what: "minecraft:book".to_string()
            }
        );
    }

    #[test]
    fn a_requirement_that_is_not_met_comes_after_the_cost() {
        let tree = tree();
        let keen = row("civilian/keen");
        let progress = Progress::new();
        // Enough books, no residence.
        let refusal = may_start(
            &tree,
            &keen,
            5,
            &progress,
            &Buildings::new(),
            &empty(),
            &inventory(&[("minecraft:book", 3)]),
        )
        .expect_err("no residence");
        assert_eq!(
            refusal,
            Refusal::RequirementUnmet {
                what: "minecolonies:residence levels summing to 3".to_string()
            }
        );
    }

    #[test]
    fn a_research_with_everything_behind_it_starts() {
        let tree = tree();
        let keen = row("civilian/keen");
        let progress = Progress::new();
        let mut standing = Buildings::new();
        standing.add("residence", 3);
        assert_eq!(
            may_start(
                &tree,
                &keen,
                5,
                &progress,
                &standing,
                &empty(),
                &inventory(&[("minecraft:book", 3)])
            ),
            Ok(())
        );
    }

    #[test]
    fn paying_takes_from_the_university_before_the_player() {
        let keen = row("civilian/keen");
        let mut university = inventory(&[("minecraft:book", 2)]);
        let mut player = inventory(&[("minecraft:book", 5)]);
        let taken = pay(&keen, &mut university, &mut player).expect("three books, not a tag");
        // Two books out of the university, one out of the player, in that order.
        assert_eq!(
            taken,
            vec![
                ("minecraft:book".to_string(), 2),
                ("minecraft:book".to_string(), 1)
            ]
        );
        assert_eq!(university.count(&item("minecraft:book")), 0);
        assert_eq!(player.count(&item("minecraft:book")), 4);
    }

    #[test]
    fn paying_a_tag_cost_spreads_over_its_members() {
        let ore = row("technology/veinminer");
        let mut university = inventory(&[("minecraft:iron_ore", 20)]);
        let mut player = inventory(&[("minecraft:deepslate_iron_ore", 20)]);
        let taken = pay(&ore, &mut university, &mut player).expect("32 of iron ore, either kind");
        let total: u32 = taken.iter().map(|(_, count)| count).sum();
        assert_eq!(total, 32);
    }

    #[test]
    fn a_hidden_research_is_drawn_only_when_its_requirements_are_met() {
        fn nothing(_: &str) -> bool {
            false
        }
        let stringmesh = row("unlockable/stringmesh");
        assert!(stringmesh.flags.hidden());
        let mut standing = Buildings::new();
        assert!(!visible(&stringmesh, &standing, &nothing));
        standing.add("sifter", 1);
        assert!(visible(&stringmesh, &standing, &nothing));
        // A research that is not hidden is always drawn.
        let keen = row("civilian/keen");
        assert!(visible(&keen, &Buildings::new(), &nothing));
    }

    #[test]
    fn an_immutable_research_cannot_be_undone() {
        let tree = tree();
        let loaded = row("technology/loaded");
        assert!(loaded.flags.immutable());
        let mut progress = Progress::new();
        done(&mut progress, &loaded);
        assert_eq!(
            may_undo(&tree, &loaded, &progress),
            Err(Refusal::Immutable { id: "technology/loaded" })
        );
    }

    #[test]
    fn an_autostart_research_cannot_be_undone_either() {
        let tree = tree();
        let mesh = row("unlockable/stringmesh");
        let mut progress = Progress::new();
        done(&mut progress, &mesh);
        assert_eq!(
            may_undo(&tree, &mesh, &progress),
            Err(Refusal::AutoStarts {
                id: "unlockable/stringmesh"
            })
        );
    }

    #[test]
    fn a_child_in_the_way_blocks_the_parent() {
        let tree = tree();
        let keen = row("civilian/keen");
        let mut progress = Progress::new();
        done(&mut progress, &keen);
        progress.record(&row("civilian/vines"), State::InProgress, 1);
        assert_eq!(
            may_undo(&tree, &keen, &progress),
            Err(Refusal::ChildInTheWay {
                child: "civilian/vines"
            })
        );
    }

    #[test]
    fn a_research_with_no_child_is_undoable() {
        let tree = tree();
        let keen = row("civilian/keen");
        let mut progress = Progress::new();
        done(&mut progress, &keen);
        assert_eq!(may_undo(&tree, &keen, &progress), Ok(()));
    }

    #[test]
    fn every_refusal_names_itself_and_says_what_it_is_about() {
        let refusals = [
            Refusal::AlreadyStarted { id: "a" },
            Refusal::UnknownResearch {
                id: "b".to_string(),
            },
            Refusal::LevelTooLow {
                depth: 3,
                university: 1,
            },
            Refusal::ParentUnfinished { parent: "c" },
            Refusal::SiblingChosen {
                parent: "d",
                chosen: "e".to_string(),
            },
            Refusal::BranchCeiling {
                branch: Branch::Combat,
            },
            Refusal::CostMissing {
                need: 4,
                missing: 2,
                what: "f".to_string(),
            },
            Refusal::RequirementUnmet { what: "g".to_string() },
            Refusal::TagUnknown {
                tag: "h".to_string(),
            },
            Refusal::ItemUnparsable {
                item: "i".to_string(),
            },
            Refusal::NotRunning { id: "j".to_string() },
            Refusal::NotFinished { id: "k".to_string() },
            Refusal::Immutable { id: "l" },
            Refusal::AutoStarts { id: "m" },
            Refusal::ChildInTheWay { child: "n" },
        ];
        for refusal in refusals {
            assert!(
                refusal.name().starts_with("minecolonies:research/"),
                "{}",
                refusal.name()
            );
            assert!(refusal.to_string().starts_with(refusal.name()));
        }
    }

    #[test]
    fn the_about_field_names_the_id_when_there_is_one() {
        assert_eq!(
            Refusal::AlreadyStarted { id: "civilian/keen" }.about(),
            Some("civilian/keen")
        );
        assert_eq!(
            Refusal::LevelTooLow {
                depth: 2,
                university: 1
            }
            .about(),
            None
        );
    }

    #[test]
    fn only_a_tag_cost_reports_a_tag() {
        let tagged = row("technology/woodwork").costs[0];
        assert_eq!(tag_of(&tagged), Some("minecraft:planks"));
        let plain = row("civilian/keen").costs[0];
        assert_eq!(tag_of(&plain), None);
    }

    #[test]
    fn a_requirement_names_its_building_when_it_has_one() {
        let requirement = Requirement::Building {
            building: "smeltery",
            level: 1,
        };
        assert_eq!(requirement_building(&requirement), Some("smeltery"));
        assert_eq!(
            requirement_building(&Requirement::Research {
                research: "civilian/keen"
            }),
            None
        );
    }
}
