//! When a body is stuck, and what it may do about it.
//!
//! `navigation/PathingStuckHandler.java` (712 lines) is the part of pathing that
//! notices a body is not getting anywhere and escalates: skip a node, throw the
//! path away, walk away from where it is, place a ladder, break a block,
//! teleport. It runs every ten ticks, on the navigator's side of the fence.
//!
//! **What is portable and what is not.** The *detection* and the *ladder* are
//! arithmetic over what a caller observes — the path index, the remaining
//! ticks, the distance to the goal — so they are here, and they are the rules
//! that decide whether a citizen is stuck at all. The rungs that write to the
//! world are **refused by name**: [`Action::WorldWrite`] for a rung in the
//! middle of the ladder, and [`Action::Complete`]'s two flags for the last
//! resort's teleport and damage. `MANQUANT.md` §3 has the hook-by-hook account.
//!
//! **Randomness is a draw, not a clock.** Java calls `rand.nextDouble()`,
//! `nextInt(6)`, `nextInt(20)` and `nextBoolean()` inside the ladder (`:307`,
//! `:318`, `:366`, `:398`). A port cannot roll and stay testable, so every draw
//! arrives in the [`Tick`] — including the range, which Java draws as
//! `20 + nextInt(20)`.

use crate::pathfinding::Pos;

/// The distance at which a body counts as arrived: `MIN_TARGET_DIST = 3`
/// (`PathingStuckHandler.java:44`).
pub const MIN_TARGET_DIST: f64 = 3.0;

/// How long a body may make no progress at all before the last resort:
/// `MIN_TP_DELAY = 120 * 20` ticks (`:47`).
pub const MIN_TP_DELAY: i32 = 2_400;

/// The shortest distance the last resort teleports over: `MIN_DIST_FOR_TP = 10`
/// (`:48`).
pub const MIN_DIST_FOR_TP: i32 = 10;

/// Rough ticks to cross one block: `TICKS_PER_BLOCK = 7` (`:51`).
pub const TICKS_PER_BLOCK: i32 = 7;

/// The level the handler starts at while it still hopes:
/// `STARTING_STUCK_LEVEL = -5` (`:54`).
pub const STARTING_STUCK_LEVEL: i32 = -5;

/// How much the per-goal timeout grows per block: `timePerBlockDistance = 200`
/// ticks (`:63`).
pub const TIME_PER_BLOCK: i32 = 200;

/// The wait before any ladder rung: `delayBeforeActions = 5 * 20` ticks
/// (`:120`).
pub const DELAY_BEFORE_ACTIONS: i32 = 100;

/// The wait the first rung imposes: `delayToNextUnstuckAction = 50` (`:322`).
pub const ACTION_DELAY: i32 = 50;

/// The wait after skipping a node: `= 30` (`:330`).
pub const SKIP_DELAY: i32 = 30;

/// The wait after clearing the path: `= 200` (`:350`).
pub const CLEAR_DELAY: i32 = 200;

/// The wait when a body *is* moving: `= Math.max(..., 100)` (`:228`).
pub const MOVING_DELAY: i32 = 100;

/// Nodes of progress that prove a body is not stuck: `progressedNodes > 5`
/// (`:233`).
pub const PROGRESS_NODES: u32 = 5;

/// How far a path target may be from the goal and still count as aiming at it:
/// `distSqr < 25` (`:230`).
pub const NEAR_GOAL_SQ: i64 = 25;

/// One look at the body, once every ten ticks: everything `checkStuck` reads
/// (`:161-247`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tick {
    /// `navigator.getPathResult() != null` (`:163`).
    pub has_result: bool,
    /// `IStuckHandlerEntity.canBeStuck()` (`:169`).
    pub can_be_stuck: bool,
    /// The body's block position.
    pub position: Pos,
    /// `navigator.getSafeDestination()`, with Java's `BlockPos.ZERO` meaning
    /// "none" (`:174`, `:204`).
    pub destination: Option<Pos>,
    /// The path the body follows, or `None` (`:212`).
    pub path: Option<PathFacts>,
    /// `rand.nextDouble()`, for the chance to skip walking away (`:361`).
    pub away_roll: f64,
    /// `rand.nextInt(6)`, for the chance to drop two levels (`:425`).
    pub level_roll: u32,
    /// `20 + rand.nextInt(20)`, the range of the walk-away path (`:366`).
    pub away_range: i32,
    /// `rand.nextBoolean()`, for ladders before leaves (`:396`).
    pub ladder_coin: bool,
}

/// The five facts about the path that the handler reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathFacts {
    /// `getNextNodeIndex()`, the node the body is walking towards (`:220`).
    pub index: usize,
    /// `getNodeCount()` (`:325`).
    pub len: usize,
    /// `getTarget()`, where the path ends (`:230`).
    pub target: Pos,
    /// `getNodePos(index - 1)`, the node just behind, when there is one
    /// (`:357`).
    pub previous: Option<Pos>,
    /// `getEndNode()`, the last node reached, when the path has one (`:233`).
    pub end: Option<Pos>,
    /// `isDone()` (`:212`).
    pub done: bool,
}

/// What the handler asks the caller to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Nothing: not stuck, or not yet.
    None,
    /// Advance the node index by one, in the hope that a different direction
    /// unsticks the body (`:325-331`).
    SkipNode,
    /// Throw the path away and compute a new one — `navigator.recalc()`
    /// (`:346-353`), after dismounting (`stopRiding`, `:351`).
    ClearPath,
    /// Walk to a block `range` away in a direction, then try again
    /// (`:355-378`). `turns` is Java's `movingAwayDir`, rotated clockwise after
    /// every attempt: 0 east, 1 south, 2 west, 3 north.
    MoveAway {
        /// How far the walk-away path goes (`:366`).
        range: i32,
        /// Which way it goes (`:370`).
        turns: u8,
    },
    /// A rung that needs to write to the world, and is therefore refused: the
    /// caller decides whether it can honour it (`MANQUANT.md` §3).
    WorldWrite(Unstuck),
    /// The last resort, after [`MIN_TP_DELAY`] of no progress:
    /// `completeStuckAction` (`:274-313`). The two flags are Java's two
    /// configurable side effects; the handler itself only forgets everything.
    Complete {
        /// `canTeleportGoal && destination != null` (`:283-288`).
        teleport: bool,
        /// `takeDamageOnCompleteStuck` (`:290-293`).
        damage: bool,
    },
}

/// The rungs that need the world, named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unstuck {
    /// Teleport forward along the path by `teleportRange` nodes (`:382-389`).
    TeleportAlongPath,
    /// Place a ladder under the body (`:394-399`).
    PlaceLadder,
    /// Bridge with leaves (`:400-403`).
    BuildLeafBridge,
    /// Break the block ahead (`:406-409`).
    BreakBlocks,
}

impl Action {
    /// The name this action is written under in a log.
    pub const fn name(self) -> &'static str {
        match self {
            Action::None => "nothing",
            Action::SkipNode => "skip node",
            Action::ClearPath => "clear path",
            Action::MoveAway { .. } => "move away",
            Action::WorldWrite(Unstuck::TeleportAlongPath) => "teleport along path",
            Action::WorldWrite(Unstuck::PlaceLadder) => "place ladder",
            Action::WorldWrite(Unstuck::BuildLeafBridge) => "build leaf bridge",
            Action::WorldWrite(Unstuck::BreakBlocks) => "break blocks",
            Action::Complete { .. } => "complete",
        }
    }
}

/// The handler's memory between ticks: `PathingStuckHandler`'s mutable fields
/// (`:57-118`).
#[derive(Clone, Debug, PartialEq)]
pub struct StuckHandler {
    level: i32,
    global_timeout: i32,
    delay: i32,
    last_index: Option<usize>,
    had_path: bool,
    progressed: u32,
    prev_destination: Option<Pos>,
    turns: u8,
    move_start: Option<Pos>,
    bypass_chance: f64,
    break_blocks: bool,
    place_ladders: bool,
    leaf_bridges: bool,
    teleport_range: usize,
    can_teleport_goal: bool,
    damage_on_stuck: bool,
}

impl Default for StuckHandler {
    /// The handler Java creates: everything that touches the world is off
    /// (`:83-118`), and only the timing is on.
    fn default() -> StuckHandler {
        StuckHandler {
            level: STARTING_STUCK_LEVEL,
            global_timeout: 0,
            delay: DELAY_BEFORE_ACTIONS,
            last_index: None,
            had_path: false,
            progressed: 0,
            prev_destination: None,
            turns: 0,
            move_start: None,
            bypass_chance: 0.0,
            break_blocks: false,
            place_ladders: false,
            leaf_bridges: false,
            teleport_range: 0,
            can_teleport_goal: false,
            damage_on_stuck: false,
        }
    }
}

impl StuckHandler {
    /// A handler that may do nothing but path (`canBreakBlocks`,
    /// `canPlaceLadders`, `canBuildLeafBridges`, `canTeleportGoal` all false,
    /// `:83-118`).
    pub fn new() -> StuckHandler {
        StuckHandler::default()
    }

    /// The same handler, with the world-writing rungs switched on
    /// (`:83-115`).
    pub fn allowing(mut self, break_blocks: bool, ladders: bool, leaves: bool) -> StuckHandler {
        self.break_blocks = break_blocks;
        self.place_ladders = ladders;
        self.leaf_bridges = leaves;
        self
    }

    /// Teleport along the path by this many nodes, 0 to disable
    /// (`teleportRange`, `:60`).
    pub fn with_teleport_range(mut self, nodes: usize) -> StuckHandler {
        self.teleport_range = nodes;
        self
    }

    /// Teleporting onto the goal when completely stuck (`canTeleportGoal`,
    /// `:99`).
    pub fn with_goal_teleport(mut self, on: bool) -> StuckHandler {
        self.can_teleport_goal = on;
        self
    }

    /// Hurting the body when completely stuck (`takeDamageOnCompleteStuck`,
    /// `:103`).
    pub fn with_damage(mut self, on: bool) -> StuckHandler {
        self.damage_on_stuck = on;
        self
    }

    /// The chance to skip walking away (`chanceToByPassMovingAway`, `:113`).
    pub fn with_bypass_chance(mut self, chance: f64) -> StuckHandler {
        self.bypass_chance = chance.clamp(0.0, 1.0);
        self
    }

    /// The level the ladder is at — `stuckLevel` (`:66`), for a test or a log.
    pub fn level(&self) -> i32 {
        self.level
    }

    /// How long the body has made no progress on this goal, in ticks
    /// (`globalTimeout`, `:69`).
    pub fn global_timeout(&self) -> i32 {
        self.global_timeout
    }

    /// `checkStuck(navigator)`, once per check — Java runs it every ten ticks
    /// (`:161-247`).
    ///
    /// **One divergence, stated.** Java keeps running after
    /// `completeStuckAction` has reset the handler (`:196-203`), so the rest of
    /// that tick sees a fresh level and a zeroed delay. Here the tick ends with
    /// the action: continuing would take a rung straight after the reset, which
    /// is noise, not a rule.
    pub fn tick(&mut self, tick: &Tick) -> Action {
        // 1. Nothing to judge (`:163-167`).
        if !tick.has_result {
            self.reset_global();
            return Action::None;
        }
        // 2. A body that may not be stuck (`:169-173`).
        if !tick.can_be_stuck {
            self.reset_global();
            return Action::None;
        }

        // 3. The global timeout: while the goal does not change it grows, and
        //    past two minutes plus two hundred ticks per block it is the last
        //    resort (`:175-202`).
        if self.prev_destination == tick.destination {
            self.global_timeout += 10;
            if self.global_timeout > MIN_TP_DELAY {
                match tick.destination {
                    Some(destination) => {
                        let blocks =
                            (distance(destination, tick.position) as i32).max(MIN_DIST_FOR_TP);
                        if self.global_timeout > TIME_PER_BLOCK * blocks {
                            return self.last_resort();
                        }
                    }
                    None => return self.last_resort(),
                }
            }
        } else {
            self.reset_global();
        }
        self.prev_destination = tick.destination;

        // 4. Close enough to count as arrived (`:204-216`).
        if let Some(destination) = tick.destination
            && distance(destination, tick.position) < MIN_TARGET_DIST
        {
            self.reset_global();
            return Action::None;
        }

        self.delay -= 10;

        // 5. The two ways of being stuck: no path (twice running), or a path
        //    whose index does not move (`:218-243`).
        match tick.path {
            Some(path) if !path.done => {
                if Some(path.index) == self.last_index {
                    let action = self.try_unstuck(tick);
                    if action == Action::SkipNode {
                        // The caller advances the index; forget the baseline so
                        // the next check does not read the skip as progress
                        // (`:245` reads the index *after* the ladder in Java).
                        self.last_index = None;
                    }
                    return action;
                }
                if self.last_index.is_some() {
                    // A body that is moving gets time (`:226-229`).
                    self.delay = self.delay.max(MOVING_DELAY);
                    let aiming_at_goal = match tick.destination {
                        Some(destination) => sq3(path.target, destination) < NEAR_GOAL_SQ,
                        None => false,
                    };
                    if self.level == 0 || aiming_at_goal {
                        if Some(path.index) > self.last_index {
                            self.progressed += 1;
                        }
                        let stayed = match (self.move_start, path.end) {
                            (Some(start), Some(end)) => start == end,
                            _ => false,
                        };
                        if self.progressed > PROGRESS_NODES && !stayed {
                            // Not stuck: everything goes back to the start
                            // (`:235-239`).
                            self.reset_stuck();
                        }
                    }
                }
                self.last_index = Some(path.index);
                self.had_path = true;
                Action::None
            }
            _ => {
                // No path, or a finished one (`:212-219`).
                self.last_index = None;
                self.progressed = 0;
                let action = if self.had_path {
                    Action::None
                } else {
                    self.try_unstuck(tick)
                };
                self.had_path = false;
                action
            }
        }
    }

    /// The last resort, `completeStuckAction` (`:274-313`): the handler forgets
    /// everything and hands the caller the two side effects it may perform.
    fn last_resort(&mut self) -> Action {
        let teleport = self.can_teleport_goal && self.prev_destination.is_some();
        let damage = self.damage_on_stuck;
        self.reset_global();
        Action::Complete { teleport, damage }
    }

    /// The ladder, `tryUnstuck` (`:315-416`).
    fn try_unstuck(&mut self, tick: &Tick) -> Action {
        if self.delay > 0 {
            return Action::None;
        }
        self.delay = ACTION_DELAY;

        // Rungs below zero: still hoping. Skip one node at a time, and once
        // there is nothing left to skip, start at zero (`:319-340`).
        if self.level < 0 {
            let skippable = tick
                .path
                .filter(|path| !path.done && path.index + 1 < path.len);
            return match skippable {
                Some(_) => {
                    self.delay = SKIP_DELAY;
                    self.level += 1;
                    Action::SkipNode
                }
                None => {
                    self.level = 0;
                    Action::None
                }
            };
        }

        // Rung zero: throw the path away (`:344-355`).
        if self.level == 0 {
            self.level += 1;
            self.delay = CLEAR_DELAY;
            return Action::ClearPath;
        }

        let last = self.level;
        self.climb(tick.level_roll);

        // Walking away: always at level 1, by chance at 3 to 8 when the handler
        // may not touch the world, and by the bypass chance (`:357-378`).
        let armless = !(self.break_blocks || self.leaf_bridges || self.place_ladders);
        let bypass = tick.away_roll < self.bypass_chance
            || last == 1
            || ((3..=8).contains(&last) && armless && tick.ladder_coin);
        if bypass {
            let range = tick.away_range;
            self.move_start = match tick.path.and_then(|path| path.previous) {
                Some(previous) => Some(previous),
                None => Some([tick.position[0], tick.position[1] + 1, tick.position[2]]),
            };
            let turns = self.turns;
            self.turns = (self.turns + 1) % 4;
            self.delay = range * TICKS_PER_BLOCK * 3 / 2;
            return Action::MoveAway { range, turns };
        }
        // Level 2: forward along the path, when the range is not zero
        // (`:380-390`).
        if last == 2 && self.teleport_range > 0 && self.had_path {
            self.delay = CLEAR_DELAY;
            return Action::WorldWrite(Unstuck::TeleportAlongPath);
        }
        // Levels 3 to 5: a ladder, else leaves (`:392-404`).
        if (3..=5).contains(&last) {
            if self.place_ladders && tick.ladder_coin {
                self.delay = CLEAR_DELAY;
                return Action::WorldWrite(Unstuck::PlaceLadder);
            }
            if self.leaf_bridges {
                self.delay = DELAY_BEFORE_ACTIONS;
                return Action::WorldWrite(Unstuck::BuildLeafBridge);
            }
        }
        // Levels 6 to 8: break what is in the way (`:406-410`).
        if (6..=8).contains(&last) && self.break_blocks {
            self.delay = CLEAR_DELAY;
            return Action::WorldWrite(Unstuck::BreakBlocks);
        }
        // Level 9: nothing left (`:412-416`).
        if last == 9 {
            return self.last_resort();
        }
        Action::None
    }

    /// `chanceStuckLevel` (`:420-428`): climb one level, and one time in six
    /// drop two — the handler backs off instead of escalating blindly.
    fn climb(&mut self, roll: u32) {
        self.level += 1;
        if self.level > 1 && roll.is_multiple_of(6) {
            self.level = (self.level - 2).max(1);
        }
    }

    /// `resetGlobalStuckTimers` (`:248-255`).
    fn reset_global(&mut self) {
        self.global_timeout = 0;
        self.prev_destination = None;
        self.reset_stuck();
    }

    /// `resetStuckTimers` (`:431-439`).
    fn reset_stuck(&mut self) {
        self.delay = DELAY_BEFORE_ACTIONS;
        self.last_index = None;
        self.progressed = 0;
        self.level = STARTING_STUCK_LEVEL;
        self.move_start = None;
    }
}

/// The 3-D distance, Java's `Vec3.distanceTo`
/// (`PathingStuckHandler.java:206-208`).
fn distance(a: Pos, b: Pos) -> f64 {
    let dx = f64::from(a[0] - b[0]);
    let dy = f64::from(a[1] - b[1]);
    let dz = f64::from(a[2] - b[2]);
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// `BlockPosUtil.distSqr`, in `i64`.
fn sq3(a: Pos, b: Pos) -> i64 {
    let dx = i64::from(a[0] - b[0]);
    let dy = i64::from(a[1] - b[1]);
    let dz = i64::from(a[2] - b[2]);
    dx * dx + dy * dy + dz * dz
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pathfinding::at;

    fn tick(at_pos: Pos, destination: Option<Pos>, path: Option<PathFacts>) -> Tick {
        Tick {
            has_result: true,
            can_be_stuck: true,
            position: at_pos,
            destination,
            path,
            away_roll: 0.9,
            level_roll: 1,
            away_range: 25,
            ladder_coin: false,
        }
    }

    fn walking(index: usize) -> PathFacts {
        PathFacts {
            index,
            len: 10,
            target: at(20, 0, 0),
            previous: Some(at(1, 0, 0)),
            end: None,
            done: false,
        }
    }

    /// Run the handler for `n` ticks and collect what it asked for.
    fn run(handler: &mut StuckHandler, n: usize, base: &Tick) -> Vec<Action> {
        (0..n).map(|_| handler.tick(base)).collect()
    }

    #[test]
    fn a_body_without_a_path_request_is_never_stuck() {
        let mut handler = StuckHandler::new();
        let mut t = tick(at(0, 0, 0), None, None);
        t.has_result = false;
        for _ in 0..20 {
            assert_eq!(handler.tick(&t), Action::None);
        }
        assert_eq!(handler.global_timeout(), 0, ":165 resets the timeout");
    }

    #[test]
    fn a_body_that_may_not_be_stuck_is_left_alone() {
        let mut handler = StuckHandler::new();
        let mut t = tick(at(0, 0, 0), None, None);
        t.can_be_stuck = false;
        for _ in 0..20 {
            assert_eq!(handler.tick(&t), Action::None);
        }
        assert_eq!(handler.level(), STARTING_STUCK_LEVEL);
    }

    #[test]
    fn a_body_close_to_its_goal_is_not_stuck() {
        let mut handler = StuckHandler::new();
        let t = tick(at(9, 0, 0), Some(at(10, 0, 0)), None);
        for _ in 0..20 {
            assert_eq!(
                handler.tick(&t),
                Action::None,
                "under MIN_TARGET_DIST (:207)"
            );
        }
        assert_eq!(
            handler.level(),
            STARTING_STUCK_LEVEL,
            "and nothing escalated"
        );
    }

    #[test]
    fn no_path_on_two_checks_running_clears_the_path_first() {
        let mut handler = StuckHandler::new();
        let t = tick(at(0, 0, 0), Some(at(50, 0, 0)), None);
        // The first ten ticks only spend the 100-tick delay.
        assert!(run(&mut handler, 10, &t).iter().all(|a| *a == Action::None));
        assert_eq!(handler.level(), 0, "the ladder enters at zero (:334-338)");
        // Rung zero waits another ACTION_DELAY of 50 ticks — five checks — before
        // it fires (`:322`).
        let actions = run(&mut handler, 5, &t);
        assert!(
            actions[..4].iter().all(|a| *a == Action::None),
            "{actions:?}"
        );
        assert_eq!(
            actions[4],
            Action::ClearPath,
            "rung zero throws the path away (:346-353)"
        );
        assert_eq!(handler.level(), 1);
    }

    #[test]
    fn a_path_that_does_not_move_first_skips_a_node() {
        let mut handler = StuckHandler::new();
        let t = tick(at(0, 0, 0), Some(at(50, 0, 0)), Some(walking(2)));
        let actions = run(&mut handler, 11, &t);
        assert!(
            actions.contains(&Action::SkipNode),
            "expected a skip, got {actions:?}"
        );
        assert_eq!(handler.level(), STARTING_STUCK_LEVEL + 1);
    }

    #[test]
    fn a_path_at_its_last_node_has_nothing_to_skip() {
        let mut handler = StuckHandler::new();
        let path = walking(9); // index + 1 == len
        let t = tick(at(0, 0, 0), Some(at(50, 0, 0)), Some(path));
        let actions = run(&mut handler, 11, &t);
        assert!(
            !actions.contains(&Action::SkipNode),
            "index + 1 < nodeCount fails (:325)"
        );
        assert_eq!(handler.level(), 0, "so the ladder starts at zero instead");
    }

    #[test]
    fn moving_through_the_nodes_resets_the_ladder() {
        let mut handler = StuckHandler::new();
        for index in 0..7 {
            let t = tick(at(0, 0, 0), Some(at(20, 0, 0)), Some(walking(index)));
            handler.tick(&t);
        }
        assert_eq!(
            handler.level(),
            STARTING_STUCK_LEVEL,
            "progress past five nodes proves it is not stuck (:233-239)"
        );
    }

    #[test]
    fn the_first_rung_walks_away_then_rotates_the_direction() {
        let mut handler = StuckHandler::new();
        let t = tick(at(0, 0, 0), Some(at(50, 0, 0)), None);
        handler.tick(&t); // the goal is new: the ladder is reset once (:199)
        handler.level = 1;
        handler.delay = 0;
        assert_eq!(
            handler.tick(&t),
            Action::MoveAway {
                range: 25,
                turns: 0
            },
            "level 1 always walks away (:362)"
        );
        handler.level = 1;
        handler.delay = 0;
        assert_eq!(
            handler.tick(&t),
            Action::MoveAway {
                range: 25,
                turns: 1
            },
            "and the direction rotates clockwise (:371)"
        );
    }

    #[test]
    fn a_world_write_is_refused_by_name_not_skipped() {
        let mut handler = StuckHandler::new().with_teleport_range(3);
        let path = walking(4);
        let t = tick(at(0, 0, 0), Some(at(50, 0, 0)), Some(path));
        handler.tick(&t); // establishes had_path and the index baseline
        handler.level = 2;
        handler.delay = 0;
        assert_eq!(
            handler.tick(&t),
            Action::WorldWrite(Unstuck::TeleportAlongPath),
            "level 2 teleports when the range is on (:382-389)"
        );
    }

    #[test]
    fn an_armless_handler_reaches_the_end_and_hands_over_the_last_resort() {
        let mut handler = StuckHandler::new();
        let t = tick(at(0, 0, 0), Some(at(50, 0, 0)), None);
        run(&mut handler, 11, &t);
        let mut last = Action::None;
        for _ in 0..40 {
            handler.delay = 0;
            last = handler.tick(&t);
            if matches!(last, Action::Complete { .. }) {
                break;
            }
        }
        assert_eq!(
            last,
            Action::Complete {
                teleport: false,
                damage: false
            },
            "level 9 has nothing left (:412-416)"
        );
        assert_eq!(
            handler.level(),
            STARTING_STUCK_LEVEL,
            "and the handler is ready again"
        );
    }

    #[test]
    fn the_last_resort_hands_over_the_two_side_effects_it_may_have() {
        let mut handler = StuckHandler::new()
            .with_goal_teleport(true)
            .with_damage(true);
        handler.prev_destination = Some(at(50, 0, 0));
        assert_eq!(
            handler.last_resort(),
            Action::Complete {
                teleport: true,
                damage: true
            },
            "both switches on, a goal to teleport to (:283-293)"
        );
        assert_eq!(handler.global_timeout(), 0, "and the memory is cleared");

        let mut no_goal = StuckHandler::new().with_goal_teleport(true);
        assert_eq!(
            no_goal.last_resort(),
            Action::Complete {
                teleport: false,
                damage: false
            },
            "nothing to teleport to (:283)"
        );
    }

    #[test]
    fn the_six_in_six_chance_of_climbing_drops_two_levels() {
        let mut handler = StuckHandler::new();
        handler.level = 5;
        handler.climb(0);
        assert_eq!(handler.level(), 4, "level 6, minus two (:425-427)");
        handler.level = 5;
        handler.climb(1);
        assert_eq!(handler.level(), 6, "a failed roll climbs one");
        handler.level = 1;
        handler.climb(0);
        assert_eq!(handler.level(), 1, "and it never drops below 1");
    }

    #[test]
    fn two_minutes_and_two_hundred_ticks_a_block_before_the_last_resort() {
        let mut handler = StuckHandler::new();
        let t = tick(at(0, 0, 0), Some(at(10, 0, 0)), None);
        // The ladder is kept asleep so this measures the *timeout* alone: with
        // no world action enabled the ladder reaches level 9 on its own in about
        // ninety ticks, well before this guard (see the test above).
        handler.tick(&t); // a new goal: the clock starts at zero (:199)
        handler.level = 1;
        handler.delay = 10_000;
        handler.global_timeout = MIN_TP_DELAY - 20;
        assert_eq!(
            handler.tick(&t),
            Action::None,
            "2 400 exactly is not *past* MIN_TP_DELAY (:196)"
        );
        handler.global_timeout = MIN_TP_DELAY;
        assert_eq!(
            handler.tick(&t),
            Action::Complete {
                teleport: false,
                damage: false
            },
            "2 410 > MIN_TP_DELAY and > 200 x 10 blocks (:196-203)"
        );
        assert_eq!(handler.global_timeout(), 0, "and the memory is cleared");
    }

    #[test]
    fn a_far_goal_buys_more_time_before_the_last_resort() {
        let mut handler = StuckHandler::new();
        let t = tick(at(0, 0, 0), Some(at(100, 0, 0)), None);
        handler.tick(&t);
        handler.level = 1;
        handler.delay = 10_000;
        handler.global_timeout = MIN_TP_DELAY;
        assert_eq!(
            handler.tick(&t),
            Action::None,
            "2 410 ticks is past two minutes, but 200 x 100 blocks is not (:198)"
        );
        handler.global_timeout = TIME_PER_BLOCK * 100;
        assert_eq!(
            handler.tick(&t),
            Action::Complete {
                teleport: false,
                damage: false
            },
            "20 000 ticks, the cost of a hundred blocks"
        );
    }

    #[test]
    fn changing_the_goal_clears_the_timeout() {
        let mut handler = StuckHandler::new();
        let first = tick(at(0, 0, 0), Some(at(10, 0, 0)), None);
        let second = tick(at(0, 0, 0), Some(at(40, 0, 0)), None);
        for _ in 0..10 {
            handler.tick(&first);
        }
        assert_eq!(handler.global_timeout(), 90, "the first tick was a reset");
        handler.tick(&second);
        assert_eq!(
            handler.global_timeout(),
            0,
            "a different destination resets the clock (:199)"
        );
    }
}
