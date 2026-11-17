//! The forester: find a tree, cut it, plant the sapling back, gather the drops.
//!
//! Java reference: `core/entity/ai/workers/production/EntityAIWorkLumberjack.java` —
//! the constructor's `registerTargets` (`:193-201`), `decide` (`:224-261`),
//! `startWorkingAtOwnBuilding` (`:279-286`), `prepareForWoodcutting` (`:293-301`),
//! `waitBeforeCheckingAgain` (`:308-320`), `gathering2` (`:326-354`), `findTrees`
//! (`:373-387`), `findTree` (`:394-435`), `setNewTree` (`:437-469`), `chopWood`
//! (`:476-488`), `chopTree` (`:496-598`), `plantSapling` (`:817-875`) and `gathering`
//! (`:981-996`), on top of `util/Tree.java`.
//!
//! ## The table this adds
//!
//! | # | State | Rule | Rate | Java |
//! | ---: | --- | --- | ---: | --- |
//! | 1 | `LUMBERJACK_START_WORKING` | `startWorkingAtOwnBuilding` | 20 | `:194` |
//! | 2 | `PREPARING` | `prepareForWoodcutting` | 20 | `:195` |
//! | 3 | `LUMBERJACK_SEARCHING_TREE` | `findTrees` | 20 | `:196` |
//! | 4 | `LUMBERJACK_CHOP_TREE` | `chopWood` | 20 | `:197` |
//! | 5 | `LUMBERJACK_GATHERING` | `gathering` | 20 | `:198` |
//! | 6 | `LUMBERJACK_NO_TREES_FOUND` | `waitBeforeCheckingAgain` | 20 | `:199` |
//! | 7 | `LUMBERJACK_GATHERING_2` | `gathering2` | 20 | `:200` |
//! | 8 | `START_WORKING` | `decide` (overridden) | 5 | `AbstractEntityAICrafting.java:166` |
//!
//! **The one number that changes the base table.** `getActionsDoneUntilDumping()` is
//! `MAX_BLOCKS_MINED` = 32 (`:128`, `:999-1002`): the forester walks its inventory to
//! the hut after half a stack of logs.
//!
//! **Where the tree lives.** Java keeps it on the job (`job.getTree()`, `:375`), and a
//! `Tree` owns two work queues — `woodBlocks` (`Tree.java:88`) and `leaves` (`:93`) —
//! plus the stumps the sapling goes back on (`:108`). The port does not keep the
//! queues: it reads their **lengths and head** as facts ([`super::TreeFacts`]) and
//! leaves `pollNextLog` (`Tree.java:827`) to the caller that actually broke the block.
//! A decision never needs more than "is there a log left, and which one".
//!
//! **The two waits, and why they are functions.** `hasNotDelayed(WAIT_BEFORE_SAPLING)`
//! (`:521`) and `hasNotDelayed(WAIT_BEFORE_SEARCH)` (`:312`) are one-shot delays on the
//! base class's `hasDelayed` flag, which the port reuses through
//! [`super::GatherMemory::base`]. `timeWaited` (`:154`) is not a flag: it grows by ten
//! (`:873`), asks the hut once at half the limit (`:860-864`) and gives up at
//! `MAX_WAITING_TIME` (`:866-872`) — [`sapling_step`] is that state machine, and it is
//! the only way to test it without a world.

use crate::ai::facts::WorldFacts;
use crate::ai::state::WorkerState;

use super::state::{GatherRefusal, GatherState};
use super::facts::{GatherMemory, LumberjackFacts};
use super::{at_hut, gather_action, Decision, STANDARD_DELAY, TICKS_SECOND};

/// `EntityAIWorkLumberjack.SEARCH_RANGE` (`:72`): the radius the first search uses.
pub const SEARCH_RANGE: i32 = 50;
/// `SEARCH_INCREMENT` (`:77`): how much each failed search adds.
pub const SEARCH_INCREMENT: i32 = 5;
/// `SEARCH_LIMIT` (`:81`): past this, no tree is found at all.
pub const SEARCH_LIMIT: i32 = 150;
/// `WAIT_BEFORE_SAPLING` (`:105`): one-shot wait after the last log, so the saplings
/// on the ground are collected before the replant.
pub const WAIT_BEFORE_SAPLING: u32 = 50;
/// `MAX_WAITING_TIME` (`:109`): the sapling wait limit, in the same units as
/// `timeWaited`.
pub const MAX_WAITING_TIME: i32 = 50;
/// `TIMEOUT_DELAY` (`:114`): the delay after a plant or a give-up.
pub const TIMEOUT_DELAY: u32 = 10;
/// `WAIT_BEFORE_SEARCH` (`:118`): how long `NO_TREES_FOUND` waits.
pub const WAIT_BEFORE_SEARCH: u32 = 400;
/// `WAIT_BEFORE_INCREMENT` (`:123`): the delay after growing the search radius.
pub const WAIT_BEFORE_INCREMENT: u32 = 20;
/// `MAX_BLOCKS_MINED` (`:128`): `getActionsDoneUntilDumping()` (`:999-1002`).
pub const MAX_BLOCKS_MINED: u32 = 32;
/// `GATHERING_DELAY` (`:133`): the pause before gathering, in seconds of ticks.
pub const GATHERING_DELAY: u32 = 3;
/// `XP_PER_TREE` (`:144`): the experience one finished tree is worth.
pub const XP_PER_TREE: f64 = 1.0;
/// `MIN_WORKING_RANGE` (`:101`): close enough to the stump to cut.
pub const MIN_WORKING_RANGE: i32 = 2;
/// `RANGE_HORIZONTAL_PICKUP` (`:96`): how far the drop search reaches sideways.
pub const RANGE_HORIZONTAL_PICKUP: f64 = 5.0;
/// `RANGE_VERTICAL_PICKUP` (`:91`): and up or down.
pub const RANGE_VERTICAL_PICKUP: f64 = 2.0;
/// The step `timeWaited += 10` (`:873`).
pub const TIME_WAITED_STEP: i32 = 10;
/// `ColonyConstants.rand.nextInt(100) <= 10` (`:346`): the chance `gathering2` goes
/// back to `START_WORKING` rather than searching again.
pub const START_WORKING_CHANCE: u32 = 10;

/// `BlockPosUtil.isInArea(start, end, pos)` (`:500`): inside the box the two corners
/// span, bounds inclusive on all three axes.
pub fn in_area(start: crate::ai::facts::Pos, end: crate::ai::facts::Pos, pos: crate::ai::facts::Pos) -> bool {
    let (x0, x1) = (start.x.min(end.x), start.x.max(end.x));
    let (y0, y1) = (start.y.min(end.y), start.y.max(end.y));
    let (z0, z1) = (start.z.min(end.z), start.z.max(end.z));
    (x0..=x1).contains(&pos.x) && (y0..=y1).contains(&pos.y) && (z0..=z1).contains(&pos.z)
}

/// What `setNewTree` (`:437-469`) does to the search radius when the search came back
/// empty: grow it by five and wait, or declare the range exhausted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchStep {
    /// `searchIncrement` afterwards.
    pub increment: i32,
    /// `setDelay(WAIT_BEFORE_INCREMENT)` when the radius grew, zero otherwise.
    pub delay: u32,
    /// `searchIncrement + SEARCH_RANGE > SEARCH_LIMIT`, or a restricted hut: the
    /// answer is `LUMBERJACK_NO_TREES_FOUND` (`:446-449`).
    pub exhausted: bool,
}

/// `setNewTree`'s two branches (`:441-449`), as a pure function.
///
/// A hut with a restriction never grows its radius: the restricted search either finds
/// a tree inside the box or there is none (`:441`).
pub fn search_step(restrict: bool, search_increment: i32) -> SearchStep {
    if !restrict && search_increment + SEARCH_RANGE <= SEARCH_LIMIT {
        return SearchStep {
            increment: search_increment + SEARCH_INCREMENT,
            delay: WAIT_BEFORE_INCREMENT,
            exhausted: false,
        };
    }
    SearchStep {
        increment: search_increment,
        delay: 0,
        exhausted: true,
    }
}

/// What one attempt at replanting did: `plantSapling` (`:817-875`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SaplingStep {
    /// `timeWaited` afterwards (`:838`, `:868`, `:873`).
    pub time_waited: i32,
    /// `checkedInHut` afterwards (`:863`).
    pub checked_in_hut: bool,
    /// Java's `return true` (`:841`, `:871`): the tree is over, whatever the outcome.
    pub done: bool,
    /// `setDelay(TIMEOUT_DELAY)` when Java sets it (`:840`, `:870`).
    pub delay: u32,
    /// `None` when a sapling went in or when there was nothing to replant; the two
    /// branches that gave up without a sapling answer `NoSapling`.
    pub refusal: Option<GatherRefusal>,
}

/// `plantSapling(location)` (`:817-875`) and the wrapper that clears the tree
/// (`:789-797`), as a decision.
///
/// The three exits Java has:
///
/// 1. a sapling of the tree's own kind is in the inventory (`findSaplingSlot`, `:826`,
///    `:894-905`): it is planted, `timeWaited` resets and the delay is
///    `TIMEOUT_DELAY` (`:838-841`, `:866-872`);
/// 2. the stumps are gone (`:866`): nothing to plant, the tree is over;
/// 3. neither, so `timeWaited` grows by ten (`:873`) until `MAX_WAITING_TIME`
///    (`:866`), asking the hut once at half the limit (`:860-864`).
pub fn sapling_step(trade: &LumberjackFacts, time_waited: i32, checked_in_hut: bool) -> SaplingStep {
    let stumps_empty = trade
        .tree
        .as_ref()
        .map(|tree| tree.stumps.is_empty())
        .unwrap_or(true);

    // :826-841 — a sapling is in the inventory: it goes in, and the tree is done.
    if trade.sapling_slot.is_some() {
        return SaplingStep {
            time_waited: 0,
            checked_in_hut,
            done: true,
            delay: TIMEOUT_DELAY,
            refusal: None,
        };
    }

    // :860-864 — ask the hut for one, once, at half the waiting time.
    let mut checked = checked_in_hut;
    if time_waited >= MAX_WAITING_TIME / 2 && !checked_in_hut {
        checked = true;
    }

    // :866-872 — nothing to plant on, or the wait is over: give up, the tree is done.
    if stumps_empty || time_waited >= MAX_WAITING_TIME {
        return SaplingStep {
            time_waited: 0,
            checked_in_hut: checked,
            done: true,
            delay: TIMEOUT_DELAY,
            refusal: if stumps_empty { None } else { Some(GatherRefusal::NoSapling) },
        };
    }

    // :873 — wait another ten ticks.
    SaplingStep {
        time_waited: time_waited + TIME_WAITED_STEP,
        checked_in_hut: checked,
        done: false,
        delay: 0,
        refusal: Some(GatherRefusal::NoSapling),
    }
}

/// `startWorkingAtOwnBuilding` (`:279-286`): walk to the hut, then `PREPARING`.
pub fn start_working(facts: &WorldFacts, trade: &LumberjackFacts, memory: &mut GatherMemory) -> Decision {
    let _ = (trade, memory);
    if !at_hut(facts) {
        return Decision::stay(); // :281-284
    }
    Decision::go(GatherState::Base(WorkerState::Preparing)) // :285
}

/// `prepareForWoodcutting` (`:293-301`): an axe, plus shears or a hoe.
///
/// `checkForToolOrWeapon` answers **true when the tool is missing** (it creates the
/// request as a side effect, `AbstractEntityAIBasic.java:993-1002`), so the fact is
/// named after the check.
pub fn prepare_for_woodcutting(
    facts: &WorldFacts,
    trade: &LumberjackFacts,
    memory: &mut GatherMemory,
) -> Decision {
    let _ = (facts, memory);
    if trade.missing_axe || trade.missing_secondary_tool {
        // :295-299 — "reset everything, maybe there are new crafting requests".
        return Decision::go_after(
            GatherState::Base(WorkerState::StartWorking),
            GatherRefusal::ToolMissing,
        );
    }
    Decision::go(GatherState::LumberjackSearchingTree) // :300
}

/// `decide` (`:224-261`), the lumberjack's override of
/// `AbstractEntityAICrafting.decide` (`:271-291`).
///
/// Four tests, in Java's order: stuck, walked to the hut, dump threshold, sapling list.
/// The fifth branch — the crafting half (`getNextCraftingState`, `:260`) — is **not**
/// ported: this module carries the harvest goals, and a forester that has planks to
/// make is `crate::ai`'s crafting table's business.
pub fn decide(facts: &WorldFacts, trade: &LumberjackFacts, memory: &mut GatherMemory) -> Decision {
    let _ = memory;
    // :226-231 — `checkIfStuck()` and `tryUnstuck()`: both are pathfinding and belong
    // to the caller, who reports the walk's progress as `at_tree`/`at_hut`.
    // :232-235
    if !at_hut(facts) {
        return Decision::go(GatherState::Base(WorkerState::StartWorking));
    }
    // :237-241 — "wait to dump before continuing": the dump itself is the base table's.
    if facts.inventory.actions_done >= MAX_BLOCKS_MINED {
        return Decision::go_after(
            GatherState::Base(WorkerState::StartWorking),
            GatherRefusal::InventoryDumpDue,
        );
    }
    // :243-258 — the sapling list is not filled in, and there is nothing to craft.
    if trade.sapling_list_incomplete && (trade.task_queue_empty || trade.no_current_task) {
        return Decision::go(GatherState::LumberjackStartWorking); // :251, :256
    }
    // :260 — `getNextCraftingState()`.
    Decision::go_after(
        GatherState::Base(WorkerState::StartWorking),
        GatherRefusal::CraftingNotPorted,
    )
}

/// `findTrees` (`:373-387`) and `findTree` (`:394-435`).
pub fn find_trees(facts: &WorldFacts, trade: &LumberjackFacts, memory: &mut GatherMemory) -> Decision {
    let _ = facts;
    // :375-387 — a tree is already known.
    if trade.tree.is_some() {
        return Decision::go(GatherState::LumberjackChopTree); // :386
    }
    // :377-380 — a queued crafting task goes first.
    if !trade.task_queue_empty {
        return Decision::go_after(
            GatherState::Base(WorkerState::StartWorking),
            GatherRefusal::CraftingQueued,
        );
    }
    // :394-435 — `findTree()`.
    match trade.path {
        super::PathState::Computing => Decision::stay(), // :398-401
        super::PathState::None => Decision::stay(),      // :402-427, the search is started
        super::PathState::Done => set_new_tree(trade, memory), // :428-431
        super::PathState::Cancelled => Decision::go_after(
            GatherState::LumberjackNoTreesFound,
            GatherRefusal::NoTreeFound,
        ), // :434
    }
}

/// `setNewTree` (`:437-469`): the search answered, so build the tree or grow the
/// radius.
fn set_new_tree(trade: &LumberjackFacts, memory: &mut GatherMemory) -> Decision {
    if trade.tree_candidate.is_none() {
        // :441-450 — no tree at this radius.
        let step = search_step(trade.restrict, memory.lumberjack.search_increment);
        memory.lumberjack.search_increment = step.increment;
        if step.delay > 0 {
            memory.base.delay = step.delay as i32;
        }
        if step.exhausted {
            return Decision::go_after(
                GatherState::LumberjackNoTreesFound,
                GatherRefusal::NoTreeFound,
            );
        }
        return Decision::stay(); // :468
    }
    // :453-464 — the caller built the tree from the candidate; the port reads it.
    match &trade.tree {
        Some(tree) if tree.is_tree => Decision::go(GatherState::LumberjackChopTree), // :459
        _ => Decision::stay(), // :462-464, `job.setTree(null)` is the caller's
    }
}

/// `chopWood` (`:476-488`): the axe, the tree, then `chopTree`.
pub fn chop_wood(facts: &WorldFacts, trade: &LumberjackFacts, memory: &mut GatherMemory) -> Decision {
    let _ = facts;
    // :478-481 — "return to IDLE so the tool request is served".
    if trade.missing_axe {
        return Decision::go_after(GatherState::Base(WorkerState::Idle), GatherRefusal::ToolMissing);
    }
    // :483-486
    if trade.tree.is_none() {
        return Decision::go(GatherState::LumberjackSearchingTree);
    }
    chop_tree(trade, memory)
}

/// `chopTree` (`:496-598`): the order of the four tests is the rule.
fn chop_tree(trade: &LumberjackFacts, memory: &mut GatherMemory) -> Decision {
    let Some(tree) = trade.tree.as_ref() else {
        return Decision::go(GatherState::LumberjackSearchingTree);
    };
    let should_break_leaves = trade.defoliate || tree.nether; // :498

    // :500-505 — a restricted hut drops a tree outside its box.
    if trade.restrict {
        if let Some((start, end)) = trade.restriction {
            if !in_area(start, end, tree.location) {
                return Decision::go_after(
                    GatherState::Base(WorkerState::StartWorking),
                    GatherRefusal::TreeOutsideRestriction,
                );
            }
        }
    }

    // :507-517 — walk to the stump first, when there is still wood (or a leaf) to cut.
    let work_left =
        tree.has_logs() || (should_break_leaves && tree.has_leaves()) || memory.lumberjack.checked_in_hut;
    if work_left && !trade.at_tree {
        return Decision::refuse(GatherRefusal::NotAtTree);
    }

    // :519-543 — nothing left to cut: wait for the saplings on the ground, replant.
    if !tree.has_logs() && (!should_break_leaves || !tree.has_leaves()) {
        if crate::ai::worker::has_not_delayed(&mut memory.base, WAIT_BEFORE_SAPLING) {
            return Decision::stay(); // :521-524
        }
        if trade.replant {
            let step = sapling_step(trade, memory.lumberjack.time_waited, memory.lumberjack.checked_in_hut);
            memory.lumberjack.time_waited = step.time_waited;
            memory.lumberjack.checked_in_hut = step.checked_in_hut;
            if step.delay > 0 {
                memory.base.delay = step.delay as i32;
            }
            if !step.done {
                // Still waiting for a sapling: Java stays in `CHOP_TREE` (`:874`).
                return Decision::refuse(GatherRefusal::NoSapling);
            }
        } else {
            // :531-535 — no replant: forget the tree, the caller clears it.
            memory.lumberjack.checked_in_hut = false;
        }
        // :537-542 — statistics, experience, one action done, and the gathering delay.
        memory.base.delay = (TICKS_SECOND * GATHERING_DELAY) as i32;
        return Decision::go(GatherState::LumberjackGathering);
    }

    // :554-596 — cut one leaf (defoliation) or one log.
    if tree.has_leaves() && should_break_leaves {
        if !trade.mine_done {
            return Decision::stay(); // :557-560
        }
        return Decision::stay(); // the caller polls the leaf queue (`Tree.java:847`)
    }
    if tree.has_logs() {
        if !trade.mine_done {
            return Decision::stay(); // :589-592
        }
        // :585 — a Dynamic Tree waits five seconds for the falling logs.
        if tree.dynamic {
            memory.base.delay = 100;
        }
        return Decision::go(GatherState::LumberjackChopTree); // `return getState()` `:597`
    }
    Decision::stay()
}

/// `gathering` (`:981-996`): pick the drops up, then search again.
pub fn gathering(facts: &WorldFacts, trade: &LumberjackFacts, memory: &mut GatherMemory) -> Decision {
    let _ = (facts, memory);
    if trade.items_on_ground > 0 {
        return Decision::stay(); // :989-993
    }
    Decision::go(GatherState::LumberjackSearchingTree) // :995
}

/// `waitBeforeCheckingAgain` (`:308-320`): one long delay, then the sapling sweep.
pub fn wait_before_checking_again(
    facts: &WorldFacts,
    trade: &LumberjackFacts,
    memory: &mut GatherMemory,
) -> Decision {
    let _ = (facts, trade);
    if crate::ai::worker::has_not_delayed(&mut memory.base, WAIT_BEFORE_SEARCH) {
        return Decision::stay(); // :312-315
    }
    Decision::go(GatherState::LumberjackGathering2) // :319
}

/// `gathering2` (`:326-354`): the restricted second sweep, then a one-in-ten draw back
/// to `START_WORKING`.
pub fn gathering2(facts: &WorldFacts, trade: &LumberjackFacts, memory: &mut GatherMemory) -> Decision {
    let _ = (facts, memory);
    // :328-342 — inside a restriction, keep picking up what is still on the ground.
    if trade.restrict && trade.items_on_ground > 0 {
        return Decision::stay();
    }
    // :346-353 — `ColonyConstants.rand.nextInt(100) <= 10`.
    if trade.draw_100 <= START_WORKING_CHANCE {
        Decision::go(GatherState::Base(WorkerState::StartWorking))
    } else {
        Decision::go(GatherState::LumberjackSearchingTree)
    }
}

gather_action!(start_working_action, Lumberjack, start_working);
gather_action!(prepare_action, Lumberjack, prepare_for_woodcutting);
gather_action!(decide_action, Lumberjack, decide);
gather_action!(find_trees_action, Lumberjack, find_trees);
gather_action!(chop_wood_action, Lumberjack, chop_wood);
gather_action!(gathering_action, Lumberjack, gathering);
gather_action!(wait_action, Lumberjack, wait_before_checking_again);
gather_action!(gathering2_action, Lumberjack, gathering2);

/// Register the eight targets of `EntityAIWorkLumberjack.java:193-201` plus the
/// override of `START_WORKING`.
pub fn register(machine: &mut crate::ai::state::Machine<GatherState, super::GatherScene>) {
    // :194 — `new AITarget(LUMBERJACK_START_WORKING, this::startWorkingAtOwnBuilding,
    // TICKS_SECOND)`.
    super::add_target(
        machine,
        GatherState::LumberjackStartWorking,
        "startWorkingAtOwnBuilding",
        "EntityAIWorkLumberjack.java:194",
        TICKS_SECOND,
        start_working_action,
    );
    // :195 — `new AITarget(PREPARING, this::prepareForWoodcutting, TICKS_SECOND)`.
    super::add_target(
        machine,
        GatherState::Base(WorkerState::Preparing),
        "prepareForWoodcutting",
        "EntityAIWorkLumberjack.java:195",
        TICKS_SECOND,
        prepare_action,
    );
    // :196 — `new AITarget(LUMBERJACK_SEARCHING_TREE, this::findTrees, TICKS_SECOND)`.
    super::add_target(
        machine,
        GatherState::LumberjackSearchingTree,
        "findTrees",
        "EntityAIWorkLumberjack.java:196",
        TICKS_SECOND,
        find_trees_action,
    );
    // :197 — `new AITarget(LUMBERJACK_CHOP_TREE, this::chopWood, TICKS_SECOND)`.
    super::add_target(
        machine,
        GatherState::LumberjackChopTree,
        "chopWood",
        "EntityAIWorkLumberjack.java:197",
        TICKS_SECOND,
        chop_wood_action,
    );
    // :198 — `new AITarget(LUMBERJACK_GATHERING, this::gathering, TICKS_SECOND)`.
    super::add_target(
        machine,
        GatherState::LumberjackGathering,
        "gathering",
        "EntityAIWorkLumberjack.java:198",
        TICKS_SECOND,
        gathering_action,
    );
    // :199 — `new AITarget(LUMBERJACK_NO_TREES_FOUND, this::waitBeforeCheckingAgain,
    // TICKS_SECOND)`.
    super::add_target(
        machine,
        GatherState::LumberjackNoTreesFound,
        "waitBeforeCheckingAgain",
        "EntityAIWorkLumberjack.java:199",
        TICKS_SECOND,
        wait_action,
    );
    // :200 — `new AITarget(LUMBERJACK_GATHERING_2, this::gathering2, TICKS_SECOND)`.
    super::add_target(
        machine,
        GatherState::LumberjackGathering2,
        "gathering2",
        "EntityAIWorkLumberjack.java:200",
        TICKS_SECOND,
        gathering2_action,
    );
    // `AbstractEntityAICrafting.java:166` — `new AITarget(START_WORKING, this::decide,
    // STANDARD_DELAY)`, with `decide` overridden at `EntityAIWorkLumberjack.java:224`.
    super::add_target(
        machine,
        GatherState::Base(WorkerState::StartWorking),
        "decide",
        "EntityAIWorkLumberjack.java:224-261",
        STANDARD_DELAY,
        decide_action,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::facts::Pos;
    use crate::ai_gather::facts::{GatherJob, GatherScene, TreeFacts};

    fn tree(logs: u32, leaves: u32) -> TreeFacts {
        TreeFacts {
            location: Pos::new(4, 64, 4),
            is_tree: true,
            logs,
            leaves,
            next_log: Some(Pos::new(4, 65, 4)),
            next_leaf: Some(Pos::new(5, 66, 4)),
            stumps: vec![Pos::new(4, 64, 4)],
            ..TreeFacts::default()
        }
    }

    fn trade(t: TreeFacts) -> LumberjackFacts {
        LumberjackFacts {
            tree: Some(t),
            missing_axe: false,
            missing_secondary_tool: false,
            ..LumberjackFacts::default()
        }
    }

    #[test]
    fn the_search_radius_grows_by_five_until_one_hundred_and_fifty() {
        // `setNewTree` (`EntityAIWorkLumberjack.java:441-449`).
        let first = search_step(false, 0);
        assert_eq!(first.increment, 5);
        assert_eq!(first.delay, WAIT_BEFORE_INCREMENT);
        assert!(!first.exhausted);
        // 50 + 100 = 150 is still allowed: `<= SEARCH_LIMIT`.
        let at_limit = search_step(false, 100);
        assert_eq!(at_limit.increment, 105, "the last step still grows");
        assert!(!at_limit.exhausted);
        // 105 + 50 > 150: there is nothing left to look at.
        let exhausted = search_step(false, 105);
        assert_eq!(exhausted.increment, 105, "the radius stops growing");
        assert_eq!(exhausted.delay, 0);
        assert!(exhausted.exhausted);
    }

    #[test]
    fn a_restricted_hut_never_grows_only_waits_for_the_box() {
        // `:441` — the growth branch is `!building.shouldRestrict()`.
        let step = search_step(true, 0);
        assert!(step.exhausted);
        assert_eq!(step.increment, 0);
    }

    #[test]
    fn a_sapling_in_the_inventory_ends_the_tree_and_resets_the_wait() {
        // `:826-841`: `findSaplingSlot` found one, the delay is `TIMEOUT_DELAY`.
        let mut facts = trade(tree(0, 0));
        facts.sapling_slot = Some(12);
        let step = sapling_step(&facts, 30, false);
        assert!(step.done);
        assert_eq!(step.time_waited, 0);
        assert_eq!(step.delay, TIMEOUT_DELAY);
        assert_eq!(step.refusal, None);
    }

    #[test]
    fn without_a_sapling_the_wait_grows_by_ten_and_asks_the_hut_at_twenty_five() {
        // `:860-864` (`MAX_WAITING_TIME / 2`) and `:873` (`timeWaited += 10`).
        let facts = trade(tree(0, 0));
        let first = sapling_step(&facts, 0, false);
        assert!(!first.done);
        assert_eq!(first.time_waited, 10);
        assert!(!first.checked_in_hut, "the hut is asked at 25, not at 0");
        assert_eq!(first.refusal, Some(GatherRefusal::NoSapling));

        let at_ten = sapling_step(&facts, 10, false);
        assert_eq!(at_ten.time_waited, 20);
        assert!(!at_ten.checked_in_hut);

        let at_twenty = sapling_step(&facts, 20, false);
        assert_eq!(at_twenty.time_waited, 30);
        assert!(!at_twenty.checked_in_hut, "20 is still under the half limit of 25");

        let at_thirty = sapling_step(&facts, 30, false);
        assert_eq!(at_thirty.time_waited, 40);
        assert!(at_thirty.checked_in_hut, "30 >= 25: ask the hut, once");

        let asked = sapling_step(&facts, 40, true);
        assert!(asked.checked_in_hut, "and only once");
    }

    #[test]
    fn at_fifty_ticks_of_waiting_the_tree_is_given_up() {
        // `:866-872` — `timeWaited >= MAX_WAITING_TIME`.
        let facts = trade(tree(0, 0));
        let step = sapling_step(&facts, 50, true);
        assert!(step.done);
        assert_eq!(step.time_waited, 0);
        assert_eq!(step.delay, TIMEOUT_DELAY);
        assert_eq!(step.refusal, Some(GatherRefusal::NoSapling));
    }

    #[test]
    fn no_stump_left_means_nothing_to_replant_not_a_missing_sapling() {
        // `:866` — `stumpLocations.isEmpty()` returns true with no complaint.
        let mut t = tree(0, 0);
        t.stumps.clear();
        let facts = trade(t);
        let step = sapling_step(&facts, 10, false);
        assert!(step.done);
        assert_eq!(step.refusal, None);
    }

    #[test]
    fn the_hut_comes_before_the_axe_and_the_axe_before_the_tree() {
        // `:279-286` then `:293-301`: `START_WORKING` walks, `PREPARING` checks tools.
        let mut world = WorldFacts::new();
        world.building.position = Pos::new(0, 64, 0);
        world.worker.at = Pos::new(20, 64, 0);
        let mut memory = GatherMemory::default();
        let facts = trade(tree(3, 0));
        assert_eq!(
            start_working(&world, &facts, &mut memory).next,
            None,
            "still walking: `return getState()` (`:283`)"
        );
        world.worker.at = Pos::new(1, 64, 0);
        assert_eq!(
            start_working(&world, &facts, &mut memory).next,
            Some(GatherState::Base(WorkerState::Preparing))
        );
        let mut no_axe = facts.clone();
        no_axe.missing_axe = true;
        assert_eq!(
            prepare_for_woodcutting(&world, &no_axe, &mut memory).next,
            Some(GatherState::Base(WorkerState::StartWorking)),
            "`:298` resets to START_WORKING"
        );
        assert_eq!(
            prepare_for_woodcutting(&world, &facts, &mut memory).next,
            Some(GatherState::LumberjackSearchingTree),
            "`:300`"
        );
    }

    #[test]
    fn a_tree_outside_the_restriction_is_dropped() {
        // `chopTree` (`:500-505`).
        let mut facts = trade(tree(3, 0));
        facts.restrict = true;
        facts.restriction = Some((Pos::new(0, 60, 0), Pos::new(3, 70, 3)));
        let mut memory = GatherMemory::default();
        let decision = chop_wood(&WorldFacts::new(), &facts, &mut memory);
        assert_eq!(
            decision.next,
            Some(GatherState::Base(WorkerState::StartWorking))
        );
        assert_eq!(decision.refusal, Some(GatherRefusal::TreeOutsideRestriction));
        // The same tree inside a box big enough to hold it is not dropped: the walk is
        // the only thing left, since `at_tree` is false.
        let mut inside = facts.clone();
        inside.restriction = Some((Pos::new(0, 60, 0), Pos::new(8, 70, 8)));
        let decision = chop_wood(&WorldFacts::new(), &inside, &mut memory);
        assert_ne!(decision.refusal, Some(GatherRefusal::TreeOutsideRestriction));
        assert_eq!(decision.refusal, Some(GatherRefusal::NotAtTree));
    }

    #[test]
    fn a_full_tree_is_cut_before_it_is_replanted_or_gathered() {
        // `:507-517` then `:519-543`: logs first, the sapling wait only once the tree
        // is down.
        let facts = trade(tree(3, 0));
        let mut memory = GatherMemory::default();
        // Not at the stump yet: nothing else happens.
        assert_eq!(
            chop_wood(&WorldFacts::new(), &facts, &mut memory).refusal,
            Some(GatherRefusal::NotAtTree)
        );
        // At the stump: one log per tick, still `CHOP_TREE`.
        let mut at_tree = facts.clone();
        at_tree.at_tree = true;
        at_tree.mine_done = true;
        assert_eq!(
            chop_wood(&WorldFacts::new(), &at_tree, &mut memory).next,
            Some(GatherState::LumberjackChopTree)
        );
        // No log left: the `WAIT_BEFORE_SAPLING` one-shot delay holds the tick.
        let cut = trade(tree(0, 0));
        let decision = chop_wood(&WorldFacts::new(), &cut, &mut memory);
        assert_eq!(decision.next, None, "`:521-524` waits");
        assert!(memory.base.has_delayed);
        // Second call: the delay has passed, the tree is gathered.
        let decision = chop_wood(&WorldFacts::new(), &cut, &mut memory);
        assert_eq!(decision.next, Some(GatherState::LumberjackGathering));
        assert_eq!(
            memory.base.delay,
            (TICKS_SECOND * GATHERING_DELAY) as i32,
            "`:541` setDelay(TICKS_SECOND * GATHERING_DELAY)"
        );
    }

    #[test]
    fn a_defoliated_tree_loses_its_leaves_before_its_logs() {
        // `:554-562` runs before `:563-596`, and the two branches do not answer the same
        // way: the leaf one falls through to `return getState()` (`:597`), the log one
        // asks for the same state explicitly.
        let mut facts = trade(tree(2, 5));
        facts.defoliate = true;
        facts.at_tree = true;
        facts.mine_done = true;
        let mut memory = GatherMemory::default();
        assert_eq!(
            chop_wood(&WorldFacts::new(), &facts, &mut memory).next,
            None,
            "a leaf went first (`:556-561`)"
        );
        // No leaf left (or no defoliation): the log branch runs.
        let mut logs_only = facts.clone();
        logs_only.defoliate = false;
        assert_eq!(
            chop_wood(&WorldFacts::new(), &logs_only, &mut memory).next,
            Some(GatherState::LumberjackChopTree)
        );
    }

    #[test]
    fn a_lumberjack_with_a_queued_task_goes_crafting_before_searching() {
        // `findTrees` (`:377-380`).
        let facts = LumberjackFacts {
            task_queue_empty: false,
            ..LumberjackFacts::default()
        };
        let mut memory = GatherMemory::default();
        let decision = find_trees(&WorldFacts::new(), &facts, &mut memory);
        assert_eq!(decision.next, Some(GatherState::Base(WorkerState::StartWorking)));
        assert_eq!(decision.refusal, Some(GatherRefusal::CraftingQueued));
    }

    #[test]
    fn an_empty_search_that_reaches_the_limit_waits_four_hundred_ticks() {
        // `findTree` → `setNewTree` → `:446-449`, then `waitBeforeCheckingAgain`
        // (`:308-320`).
        let mut facts = LumberjackFacts {
            path: super::super::PathState::Done,
            task_queue_empty: true,
            ..LumberjackFacts::default()
        };
        let mut memory = GatherMemory::default();
        memory.lumberjack.search_increment = 105;
        let decision = find_trees(&WorldFacts::new(), &facts, &mut memory);
        assert_eq!(decision.next, Some(GatherState::LumberjackNoTreesFound));
        assert_eq!(decision.refusal, Some(GatherRefusal::NoTreeFound));

        let waiting = wait_before_checking_again(&WorldFacts::new(), &facts, &mut memory);
        assert_eq!(waiting.next, None);
        assert_eq!(memory.base.delay, WAIT_BEFORE_SEARCH as i32);
        let after = wait_before_checking_again(&WorldFacts::new(), &facts, &mut memory);
        assert_eq!(after.next, Some(GatherState::LumberjackGathering2));

        // A found tree is cut directly (`:428-431` → `:459`).
        facts.path = super::super::PathState::Done;
        facts.tree_candidate = Some(Pos::new(4, 64, 4));
        facts.tree = Some(tree(2, 4));
        let decision = find_trees(&WorldFacts::new(), &facts, &mut memory);
        assert_eq!(decision.next, Some(GatherState::LumberjackChopTree));
    }

    #[test]
    fn the_second_gathering_pass_draws_one_in_ten_to_restart() {
        // `gathering2` (`:346-353`).
        let mut facts = LumberjackFacts {
            draw_100: 10,
            ..LumberjackFacts::default()
        };
        let mut memory = GatherMemory::default();
        assert_eq!(
            gathering2(&WorldFacts::new(), &facts, &mut memory).next,
            Some(GatherState::Base(WorkerState::StartWorking)),
            "10 <= 10"
        );
        facts.draw_100 = 11;
        assert_eq!(
            gathering2(&WorldFacts::new(), &facts, &mut memory).next,
            Some(GatherState::LumberjackSearchingTree)
        );
        // Inside a restriction, the drops on the ground come first (`:337-341`).
        facts.restrict = true;
        facts.items_on_ground = 3;
        assert_eq!(gathering2(&WorldFacts::new(), &facts, &mut memory).next, None);
    }

    #[test]
    fn a_trade_action_on_another_trade_s_facts_refuses_by_name() {
        // The macro's `let ... else`: a forester handed a miner's facts.
        let mut scene = GatherScene::new(WorldFacts::new(), GatherJob::Miner(Default::default()));
        let next = chop_wood_action(&mut scene, GatherState::LumberjackChopTree);
        assert_eq!(next, Some(GatherState::LumberjackChopTree), "the state holds");
        assert_eq!(scene.refusal, Some(GatherRefusal::NoFactsForTrade));
    }

    #[test]
    fn a_tree_location_inside_the_box_counts_and_one_outside_does_not() {
        // `BlockPosUtil.isInArea` (`:500`), bounds inclusive.
        let start = Pos::new(0, 60, 0);
        let end = Pos::new(10, 70, 10);
        assert!(in_area(start, end, Pos::new(0, 60, 0)));
        assert!(in_area(start, end, Pos::new(10, 70, 10)));
        assert!(in_area(end, start, Pos::new(5, 65, 5)), "order does not matter");
        assert!(!in_area(start, end, Pos::new(11, 65, 5)));
        assert!(!in_area(start, end, Pos::new(5, 59, 5)));
    }
}
