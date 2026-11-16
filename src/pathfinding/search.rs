//! The A\* itself: a frontier, a cost function, a ceiling, and a named refusal.
//!
//! Java reference: `core/entity/pathfinding/pathjobs/AbstractPathJob.java`
//! (1 976 lines) — the search loop at `:345-508`, the neighbour generation at
//! `:672-720`, the ground and fall rules at `:1440-1760` — and
//! `PathJobMoveToLocation.java` (168 lines) for the goal test, the heuristic
//! and the node ceiling of a "walk to this position" job.
//!
//! **What this is.** The contract is "a mod supplies a finished path, the
//! engine makes the entity follow it" (`arismoko/horizon#4443`), so the whole
//! search is ours. [`plan`] returns a [`Path`] — a `Vec<Node>` from the start
//! to the goal, every consecutive pair adjacent — or a [`Refusal`] that says
//! *which* wall stopped it.
//!
//! **Why a refusal and not Java's best effort.** When Java hits its node
//! ceiling it returns `finalizePath(bestNode)` (`AbstractPathJob.java:506`),
//! a path that stops wherever the search had got to; the navigator then
//! replans from there. A mod cannot replan behind the engine's back, so
//! returning a path that does not reach the goal would be a lie about what was
//! computed. [`Refusal::NodeLimit`] is the honest answer, and the caller
//! decides whether to try a shorter goal.
//!
//! **Determinism.** No clock, no randomness, no hash iteration: the frontier
//! is a `BinaryHeap` with a total order on its entries, the tables are
//! `BTreeMap`s, and ties are broken by insertion order exactly as
//! `MNode.compareTo` does (`MNode.java:139-164`). Two runs on the same world
//! give the same path, byte for byte — which is what lets a test assert a cost.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap};

use super::node::{
    Costs, HORIZONTAL, MAX_COST, MAX_NODES, Node, NodeKind, Pos, down, horizontal_direction,
    manhattan, offset, squared_distance, up,
};
use super::world::{WorldView, empty, enterable, free, kind_of, resting};

/// The slack Java allows around a goal that cannot be stood in, squared.
///
/// `DESTINATION_SLACK_ADJACENT = √2` (`PathJobMoveToLocation.java:24`) and
/// the test compares squared distances (`:100-104`), so the port compares
/// against 2.
pub const SLACK_SQUARED: i32 = 2;

/// How many nodes the search may pull off the frontier.
///
/// [`Limits`] carries it; the ceiling itself is [`MAX_NODES`], from
/// `AbstractPathJob.java:59` (`MAX_NODES = 8000`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    /// The ceiling before Java's relaxation: `maxNodes`, set per job and capped
    /// at [`MAX_NODES`] (`AbstractPathJob.java:193`, `:223`, `:271`).
    pub max_nodes: u32,
}

impl Default for Limits {
    /// The ceiling of the class constant, with no per-job range: what a job
    /// with an unknown range gets (`AbstractPathJob.java:59`).
    fn default() -> Limits {
        Limits {
            max_nodes: MAX_NODES,
        }
    }
}

impl Limits {
    /// A ceiling, in nodes.
    pub const fn new(max_nodes: u32) -> Limits {
        Limits { max_nodes }
    }

    /// The ceiling of a job that searches `range` blocks around its start:
    /// `min(MAX_NODES, range * range)`, plus the range itself
    /// (`AbstractPathJob.java:193`, then `PathJobMoveToLocation.java:43`).
    ///
    /// A range of 20 therefore buys 400 + 20 nodes, not 8 000 — it is the
    /// range, not the constant, that usually binds.
    pub fn for_range(range: i32) -> Limits {
        let range = range.max(0) as u32;
        Limits {
            max_nodes: range.saturating_mul(range).min(MAX_NODES) + range,
        }
    }

    /// The ceiling in force once a costly step has been seen.
    ///
    /// Java: `totalNodesVisited > maxNodes + (maxCost * maxCost) * 2`
    /// (`AbstractPathJob.java:373`), where `maxCost` is the largest step cost
    /// so far, rounded up and capped at [`MAX_COST`] (`:889-891`). A job that
    /// meets expensive terrain — water, a deep fall — therefore earns up to
    /// `25² * 2 = 1250` extra nodes, and no more.
    pub fn limit(&self, widest_step: f64) -> u32 {
        let widest = widest_step.min(MAX_COST);
        let extra = (widest * widest * 2.0) as u32;
        self.max_nodes.saturating_add(extra)
    }
}

/// What to compute: where from, where to, how hard to try, at what prices.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Request {
    /// Where the body is now.
    pub start: Pos,
    /// Where it should end up.
    pub goal: Pos,
    /// How many nodes the search may pull off the frontier.
    pub limits: Limits,
    /// What each kind of step costs.
    pub costs: Costs,
}

impl Request {
    /// A request with the default ceiling and the default prices.
    pub fn new(start: Pos, goal: Pos) -> Request {
        Request {
            start,
            goal,
            limits: Limits::default(),
            costs: Costs::default(),
        }
    }

    /// The same request, with another ceiling.
    pub fn with_limits(mut self, limits: Limits) -> Request {
        self.limits = limits;
        self
    }

    /// The same request, at other prices.
    pub fn with_costs(mut self, costs: Costs) -> Request {
        self.costs = costs;
        self
    }
}

/// Why no path was returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own closed set
pub enum Refusal {
    /// The frontier emptied without reaching the goal: every cell the body can
    /// get to has been looked at, and the goal is not one of them. Java returns
    /// `null` here and `PathResult.failedToReachDestination` reports it
    /// (`pathresults/PathResult.java:107-110`).
    Unreachable,
    /// The ceiling of [`Limits`] was hit first. Not a proof of unreachability:
    /// a larger ceiling might find a way (`AbstractPathJob.java:373-379`).
    NodeLimit {
        /// How many nodes had been pulled off the frontier.
        visited: u32,
        /// The ceiling in force at that moment, relaxation included.
        limit: u32,
    },
    /// The start cell is solid: the body is inside a block, so nothing can
    /// leave it. Java explores and fails; the port refuses before the frontier,
    /// because it costs nothing to say why.
    StartBlocked {
        /// The start it was given.
        at: Pos,
    },
}

impl Refusal {
    /// The name this refusal is written under in a log or a report.
    pub const fn name(self) -> &'static str {
        match self {
            Refusal::Unreachable => "unreachable",
            Refusal::NodeLimit { .. } => "node limit",
            Refusal::StartBlocked { .. } => "start blocked",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::Unreachable => f.write_str("unreachable: the frontier emptied"),
            Refusal::NodeLimit { visited, limit } => {
                write!(f, "node limit: {visited} visited, ceiling {limit}")
            }
            Refusal::StartBlocked { at } => write!(
                f,
                "start blocked: the body cannot be in [{}, {}, {}]",
                at[0], at[1], at[2]
            ),
        }
    }
}

impl std::error::Error for Refusal {}

/// A finished path: a start, a goal, and the moves between them.
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    nodes: Vec<Node>,
}

impl Path {
    /// A path from its nodes, in order.
    ///
    /// Public because a path is also something a caller may have to check:
    /// [`super::validate::is_walkable_path`] exists to reject one that was not
    /// built here.
    pub fn new(nodes: Vec<Node>) -> Path {
        Path { nodes }
    }

    /// The nodes, start first.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// How many nodes it has.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether it has no node at all — never true of a path [`plan`] returned.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// The cost of the whole walk: the last node's `g` value.
    pub fn total_cost(&self) -> f64 {
        self.nodes.last().map_or(0.0, |node| node.cost)
    }

    /// Where the body starts, if there is a path.
    pub fn start(&self) -> Option<Pos> {
        self.nodes.first().map(|node| node.at)
    }

    /// Where it ends — the goal, or a neighbour of it when the goal cannot be
    /// stood in (`PathJobMoveToLocation.java:56-65`).
    pub fn end(&self) -> Option<Pos> {
        self.nodes.last().map(|node| node.at)
    }
}

/// A path from `start` to `goal` in the default world rules, or a refusal.
pub fn plan<W: WorldView + ?Sized>(view: &W, start: Pos, goal: Pos) -> Result<Path, Refusal> {
    plan_with(view, &Request::new(start, goal))
}

/// A path for a whole [`Request`]: its own ceiling and its own prices.
pub fn plan_with<W: WorldView + ?Sized>(view: &W, request: &Request) -> Result<Path, Refusal> {
    let search = Search {
        view,
        costs: request.costs,
        limits: request.limits,
        goal: request.goal,
        // Java: the goal gets no slack when it can be stood in, and a slack of
        // √2 when it cannot (`PathJobMoveToLocation.java:56-65`).
        exact_goal: super::world::standable(view, &request.costs, request.goal),
    };
    search.run(request.start)
}

/// One entry of the frontier.
#[derive(Clone, Copy, Debug)]
struct Open {
    cost: f64,
    heuristic: f64,
    added: u32,
    at: Pos,
}

impl Open {
    fn score(&self) -> f64 {
        self.cost + self.heuristic
    }
}

impl PartialEq for Open {
    fn eq(&self, other: &Open) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Open {}

impl Ord for Open {
    /// `MNode.compareTo` (`MNode.java:139-164`), reversed so a `BinaryHeap`
    /// pops the *best* entry: lowest `f = g + h`, then lowest `h`, then the
    /// oldest (`counterAdded`).
    fn cmp(&self, other: &Open) -> Ordering {
        other
            .score()
            .total_cmp(&self.score())
            .then_with(|| other.heuristic.total_cmp(&self.heuristic))
            .then_with(|| other.added.cmp(&self.added))
    }
}

impl PartialOrd for Open {
    fn partial_cmp(&self, other: &Open) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// What the search remembers about a node it has seen.
#[derive(Clone, Copy, Debug)]
struct Record {
    parent: Option<Pos>,
    kind: NodeKind,
    cost: f64,
}

/// Rebuild the path by walking the parent chain back from the goal.
fn reconstruct(records: &BTreeMap<Pos, Record>, goal: Pos) -> Path {
    let mut nodes = Vec::new();
    let mut cursor = Some(goal);
    while let Some(at) = cursor {
        let Some(record) = records.get(&at) else {
            // Unreachable: every node on a chain was inserted with a parent
            // that already existed. Nothing else can happen, and a truncated
            // path would be worse than a panic a test would catch.
            break;
        };
        nodes.push(Node::new(at, record.kind, record.cost));
        cursor = record.parent;
    }
    nodes.reverse();
    Path::new(nodes)
}

/// The search, holding only what does not change between nodes.
struct Search<'a, W: WorldView + ?Sized> {
    view: &'a W,
    costs: Costs,
    limits: Limits,
    goal: Pos,
    exact_goal: bool,
}

impl<W: WorldView + ?Sized> Search<'_, W> {
    /// Java's heuristic for a "walk to this position" job: the Manhattan
    /// distance (`PathJobMoveToLocation.java:72-75`).
    ///
    /// Admissible, and that is what makes the first goal popped the cheapest
    /// one: no step costs less than the block distance it covers — a level step
    /// costs 1 for 1 block, a jump 3 for 1, a fall 1+d³ for d
    /// (`AbstractPathJob.java:1030-1065`).
    fn heuristic(&self, pos: Pos) -> f64 {
        f64::from(manhattan(pos, self.goal))
    }

    /// `isAtDestination` (`PathJobMoveToLocation.java:80-105`), slack included,
    /// and the surface under the node must be walkable (`:98-102`).
    fn is_goal(&self, pos: Pos) -> bool {
        let reached = if self.exact_goal {
            pos == self.goal
        } else if pos[1] == self.goal[1] - 1 {
            // Java compares the horizontal square at the goal's own height
            // here, not the node's (`:89-95`).
            squared_distance([pos[0], self.goal[1], pos[2]], self.goal) < SLACK_SQUARED
        } else {
            squared_distance(pos, self.goal) < SLACK_SQUARED
        };
        reached && self.view.surface(down(pos)).is_walkable()
    }

    /// Where the body lands when it steps into `target`'s column, Java's
    /// `getGroundHeight` (`AbstractPathJob.java:1280-1315`) narrowed to
    /// `handleNotStanding` and `checkDrop` (`:1582-1643`).
    fn landing(&self, target: Pos) -> Option<(Pos, NodeKind, i32)> {
        if !free(self.view, &self.costs, target) {
            return None;
        }
        if resting(self.view, &self.costs, target) {
            return Some((target, kind_of(self.view, &self.costs, target, 0), 0));
        }
        // Nothing to stand on at this height: the body falls to the first
        // standable block below, if the corridor is clear (`:1609-1643`).
        self.fall(target, 2)
    }

    /// The first standable block below `column`, starting `first` blocks down.
    ///
    /// `checkDrop` (`AbstractPathJob.java:1609-1643`) stops at the first block
    /// that is `WALKABLE` and refuses the whole drop if it meets anything that
    /// is not air on the way (`:1636`). The port checks the block directly
    /// under the feet too, which Java's `for (i = 2; ...)` skips: a fence under
    /// a gap does not let a body through here, where Java's scan would look
    /// one block lower and might.
    fn fall(&self, column: Pos, first: i32) -> Option<(Pos, NodeKind, i32)> {
        let deepest = self.costs.deepest_fall();
        for depth in 1..=deepest {
            let cell = [column[0], column[1] - depth, column[2]];
            if depth >= first && self.view.surface(cell).is_walkable() {
                let landed = up(cell);
                if !free(self.view, &self.costs, landed) || !resting(self.view, &self.costs, landed)
                {
                    return None;
                }
                let dy = landed[1] - column[1];
                return Some((landed, kind_of(self.view, &self.costs, landed, dy), dy));
            }
            if !empty(self.view, cell) {
                return None;
            }
        }
        None
    }

    /// A step up onto the block in front: `handleTargetNotPassable`
    /// (`AbstractPathJob.java:1516-1578`).
    ///
    /// The target cell itself must be standable (`:1520`), nothing may be
    /// in the way above it, and the jump needs room one cell above the body's
    /// head (`:1542-1549`). `MAX_JUMP_HEIGHT = 1.3` (`:1558`,
    /// `PathingConstants.java:44`) allows exactly one block, which is why this
    /// move rises by one and there is no two-block variant.
    fn jump(&self, from: Pos, target: Pos) -> Option<(Pos, NodeKind, i32)> {
        if !self.view.surface(target).is_walkable() {
            return None;
        }
        let landed = up(target);
        let apex = [from[0], from[1] + 2, from[2]];
        if !free(self.view, &self.costs, landed)
            || !resting(self.view, &self.costs, landed)
            || !enterable(self.view, &self.costs, apex)
        {
            return None;
        }
        Some((landed, kind_of(self.view, &self.costs, landed, 1), 1))
    }

    /// Every move out of `from`, as Java's `visitNode` explores them
    /// (`AbstractPathJob.java:669-729`): up and down first when there is a
    /// ladder, then north, east, south and west.
    ///
    /// **One Java move is missing, on purpose.** `visitNode` also explores
    /// straight down whenever the cell under the node is passable (`:695-698`).
    /// In this model that cannot happen for a node that rests: a body that rests
    /// has a standable block under its feet ([`super::world::resting`]), so the
    /// cell below is the floor and not passable, and every node of a path rests
    /// by construction — [`super::validate`] refuses one that does not. Java
    /// needs the branch for hollow supports (a carpet, a lily pad) whose own
    /// cell is passable and whose `getGroundHeight` answers the same `y` again
    /// (`:713-715` skips that case as a no-op). A descent here always happens
    /// through a horizontal column, which [`Self::landing`] sends through
    /// [`Self::fall`].
    ///
    /// The order matters only for tie-breaking, and it is the Java order so a
    /// tie resolves the same way.
    fn moves(&self, from: Pos) -> Vec<(Pos, NodeKind, i32)> {
        let mut moves = Vec::with_capacity(8);

        if self.view.is_climbable(from) {
            // "if (node.isLadder() || node.isVisited()) explore up and down"
            // (`:684-687`).
            for step in [1, -1] {
                let next = [from[0], from[1] + step, from[2]];
                if free(self.view, &self.costs, next) && resting(self.view, &self.costs, next) {
                    moves.push((next, NodeKind::Climb, step));
                }
            }
        }

        for delta in HORIZONTAL {
            let target = offset(from, delta);
            if let Some(moved) = self.landing(target) {
                moves.push(moved);
            } else if let Some(moved) = self.jump(from, target) {
                moves.push(moved);
            }
        }

        moves
    }

    /// The whole search, `AbstractPathJob.search` (`:345-508`).
    fn run(&self, start: Pos) -> Result<Path, Refusal> {
        if !free(self.view, &self.costs, start) {
            return Err(Refusal::StartBlocked { at: start });
        }

        let start_kind = kind_of(self.view, &self.costs, start, 0);
        let mut records: BTreeMap<Pos, Record> = BTreeMap::new();
        let mut open: BinaryHeap<Open> = BinaryHeap::new();
        let mut visited: u32 = 0;
        let mut added: u32 = 0;
        // Java's `maxCost`: the largest step cost seen, which relaxes the
        // ceiling (`:889-891`).
        let mut widest: f64 = 0.0;

        let heuristic = self.heuristic(start);
        records.insert(
            start,
            Record {
                parent: None,
                kind: start_kind,
                cost: 0.0,
            },
        );
        open.push(Open {
            cost: 0.0,
            heuristic,
            added,
            at: start,
        });

        while let Some(entry) = open.pop() {
            let Some(known) = records.get(&entry.at).copied() else {
                continue;
            };
            // A node is queued again when a cheaper route to it is found; the
            // older entry is then stale (`:946-960`, `updateNode`).
            if entry.cost > known.cost {
                continue;
            }

            visited += 1;
            let limit = self.limits.limit(widest);
            if visited > limit {
                return Err(Refusal::NodeLimit { visited, limit });
            }

            if self.is_goal(entry.at) {
                return Ok(reconstruct(&records, entry.at));
            }

            for (next, kind, dy) in self.moves(entry.at) {
                let swim_start = kind == NodeKind::Swim && known.kind != NodeKind::Swim;
                let mut step = self.costs.step(known.kind, kind, dy, swim_start);
                if let Some(previous) = known.parent {
                    step += self.costs.turn(
                        horizontal_direction(previous, entry.at),
                        horizontal_direction(entry.at, next),
                    );
                }
                if step > widest {
                    widest = step.ceil().min(MAX_COST);
                }

                let cost = known.cost + step;
                if let Some(existing) = records.get(&next) {
                    // "if (cost >= nextNode.getCost()) return" (`:949`).
                    if existing.cost <= cost {
                        continue;
                    }
                }

                added += 1;
                let heuristic = self.heuristic(next);
                records.insert(
                    next,
                    Record {
                        parent: Some(entry.at),
                        kind,
                        cost,
                    },
                );
                open.push(Open {
                    cost,
                    heuristic,
                    added,
                    at: next,
                });
            }
        }

        Err(Refusal::Unreachable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pathfinding::fake::Grid;
    use crate::pathfinding::node::{MAX_JUMP_STEP, at};
    use crate::pathfinding::validate::{is_walkable_path, is_walkable_with};

    #[test]
    fn a_straight_line_is_walked_one_block_at_a_time() {
        let grid = Grid::new();
        let path = plan(&grid, at(1, 0, 1), at(8, 0, 1)).expect("a flat, empty world");

        assert_eq!(path.len(), 8, "one node per block, both ends included");
        assert_eq!(path.start(), Some(at(1, 0, 1)));
        assert_eq!(path.end(), Some(at(8, 0, 1)));
        assert!(
            path.nodes().iter().all(|node| node.kind == NodeKind::Walk),
            "nothing to swim, climb, open or fall down"
        );
        assert_eq!(path.total_cost(), 7.0, "seven level steps at 1");
        assert_eq!(is_walkable_path(&grid, &path), Ok(()));
    }

    #[test]
    fn an_obstacle_is_walked_around_and_the_path_still_validates() {
        let mut grid = Grid::new();
        // A wall across the middle, three cells short of the north edge.
        for z in 0..7 {
            grid.set_solid(at(5, 0, z));
            grid.set_solid(at(5, 1, z));
        }

        let path = plan(&grid, at(1, 0, 1), at(8, 0, 1)).expect("the wall has a gap");
        assert!(path.len() > 8, "8 blocks straight, so a detour is longer");
        assert!(
            path.nodes()
                .iter()
                .all(|node| node.at[0] != 5 || node.at[2] >= 7),
            "the wall is never entered"
        );
        assert_eq!(
            is_walkable_path(&grid, &path),
            Ok(()),
            "and it is followable"
        );
    }

    #[test]
    fn water_is_crossed_only_when_swimming_is_allowed() {
        let mut grid = Grid::new();
        for z in 0..10 {
            grid.set_water(at(5, 0, z));
        }

        assert_eq!(
            plan(&grid, at(1, 0, 1), at(8, 0, 1)),
            Err(Refusal::Unreachable),
            "canSwim is false by default (PathingOptions.java:92)"
        );

        let request =
            Request::new(at(1, 0, 1), at(8, 0, 1)).with_costs(Costs::default().with_can_swim(true));
        let path = plan_with(&grid, &request).expect("a swimmer crosses it");
        assert_eq!(
            path.nodes()
                .iter()
                .filter(|n| n.kind == NodeKind::Swim)
                .count(),
            1,
            "the river is one cell wide"
        );
        assert_eq!(
            path.total_cost(),
            31.0,
            "6 dry steps + 1 + swimCostEnter 24 (PathingOptions.java:53)"
        );
        assert_eq!(
            is_walkable_with(&grid, &request.costs, &path),
            Ok(()),
            "the same prices, or a swimmer's path looks unwalkable"
        );
    }

    #[test]
    fn a_swim_that_costs_more_than_the_detour_is_walked_around() {
        let mut grid = Grid::new();
        // A pond two cells wide, which a detour can go around to the north.
        for z in 1..5 {
            grid.set_water(at(4, 0, z));
            grid.set_water(at(5, 0, z));
        }

        let request =
            Request::new(at(1, 0, 1), at(8, 0, 1)).with_costs(Costs::default().with_can_swim(true));
        let path = plan_with(&grid, &request).expect("both ways exist");
        assert!(
            path.nodes().iter().all(|node| node.kind != NodeKind::Swim),
            "24 to get wet beats a longer dry walk"
        );
        assert_eq!(is_walkable_with(&grid, &request.costs, &path), Ok(()));
    }

    #[test]
    fn a_wall_with_no_gap_is_refused_by_name() {
        let mut grid = Grid::new();
        for z in 0..10 {
            grid.set_solid(at(5, 0, z));
            grid.set_solid(at(5, 1, z));
        }

        assert_eq!(
            plan(&grid, at(1, 0, 1), at(8, 0, 1)),
            Err(Refusal::Unreachable),
            "the frontier empties, nothing else is left"
        );
    }

    #[test]
    fn the_node_ceiling_is_named_and_its_relaxation_is_reported() {
        let mut grid = Grid::new();
        for z in 0..10 {
            grid.set_solid(at(5, 0, z));
            grid.set_solid(at(5, 1, z));
        }

        let request = Request::new(at(1, 0, 1), at(8, 0, 1)).with_limits(Limits::new(8));
        match plan_with(&grid, &request) {
            Err(Refusal::NodeLimit { visited, limit }) => {
                assert_eq!(
                    limit, 10,
                    "8 asked for, plus 1² × 2 for the widest step seen (:889-891)"
                );
                assert_eq!(visited, 11, "the first node over the ceiling stops it");
            }
            other => panic!("expected a node-limit refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_start_inside_a_block_is_named() {
        let mut grid = Grid::new();
        grid.set_solid(at(1, 0, 1));
        assert_eq!(
            plan(&grid, at(1, 0, 1), at(8, 0, 1)),
            Err(Refusal::StartBlocked { at: at(1, 0, 1) })
        );
    }

    #[test]
    fn a_door_is_crossed_only_when_the_options_open_it() {
        let mut grid = Grid::new();
        for z in 0..10 {
            grid.set_solid(at(5, 0, z));
            grid.set_solid(at(5, 1, z));
        }
        grid.set_door(at(5, 0, 5));
        grid.set_door(at(5, 1, 5));

        // Both switches off: the door leaf is as good as a wall.
        let closed = Costs {
            enter_doors: false,
            enter_gates: false,
            ..Costs::default()
        };
        assert_eq!(
            plan_with(
                &grid,
                &Request::new(at(1, 0, 5), at(8, 0, 5)).with_costs(closed)
            ),
            Err(Refusal::Unreachable),
            "enterDoors is false by default (PathingOptions.java:96)"
        );

        let open = Request::new(at(1, 0, 5), at(8, 0, 5))
            .with_costs(Costs::default().with_enter_doors(true));
        let path = plan_with(&grid, &open).expect("the door opens");
        assert_eq!(path.len(), 8, "straight through, no detour");
        assert!(
            path.nodes().iter().any(|node| node.kind == NodeKind::Door),
            "the door node is labelled"
        );
        assert_eq!(
            path.total_cost(),
            9.0,
            "7 steps + traverseToggleAbleCost 2 (:1080-1083)"
        );
        assert_eq!(is_walkable_path(&grid, &path), Ok(()));
    }

    #[test]
    fn a_ledge_is_dropped_off_and_lands_on_the_first_floor() {
        let mut grid = Grid::new();
        grid.dig_strip(6, 10, 4); // a terrace four blocks down, on the far side

        let path = plan(&grid, at(1, 0, 1), at(8, -4, 8)).expect("over the edge");
        let fall = path
            .nodes()
            .iter()
            .find(|node| node.kind == NodeKind::Fall)
            .expect("a fall node");
        assert_eq!(fall.at[1], -4, "the terrace floor, and not deeper");
        assert_eq!(
            is_walkable_path(&grid, &path),
            Ok(()),
            "the fall corridor is checked, not assumed"
        );
    }

    #[test]
    fn a_ladder_is_climbed_one_block_at_a_time() {
        let mut grid = Grid::new();
        for z in 0..10 {
            for y in 0..3 {
                grid.set_solid(at(5, y, z));
            }
        }
        for y in 0..3 {
            grid.set_ladder(at(5, y, 5));
        }

        // The goal sits on the wall top, which only the ladder reaches.
        let path = plan(&grid, at(4, 0, 5), at(5, 3, 4)).expect("up the ladder");
        let climbs: Vec<Pos> = path
            .nodes()
            .iter()
            .filter(|node| node.kind == NodeKind::Climb)
            .map(|node| node.at)
            .collect();
        assert!(climbs.len() >= 2, "reached {:?}", path.nodes());
        let mut rises = 0;
        for pair in path.nodes().windows(2) {
            let dy = pair[1].at[1] - pair[0].at[1];
            if pair[1].kind == NodeKind::Climb {
                assert!(
                    dy <= MAX_JUMP_STEP,
                    "a rung at a time, never {dy}: {:?} then {:?}",
                    pair[0].at,
                    pair[1].at
                );
                if dy == 1 {
                    rises += 1;
                }
            }
        }
        assert!(rises >= 2, "the ladder is climbed, not walked past");
        assert_eq!(path.end(), Some(at(5, 3, 4)));
        assert_eq!(is_walkable_path(&grid, &path), Ok(()));
    }

    #[test]
    fn a_goal_that_cannot_be_stood_in_is_approached_from_a_neighbour() {
        let mut grid = Grid::new();
        grid.set_solid(at(5, 0, 5)); // a pillar, and the goal is inside it

        let path = plan(&grid, at(1, 0, 1), at(5, 0, 5)).expect("its neighbours can be reached");
        let end = path.end().expect("a path has an end");
        assert_ne!(end, at(5, 0, 5), "the goal cell is solid");
        assert!(
            squared_distance(end, at(5, 0, 5)) < SLACK_SQUARED,
            "within the √2 Java allows (PathJobMoveToLocation.java:24)"
        );
        assert_eq!(is_walkable_path(&grid, &path), Ok(()));
    }

    #[test]
    fn a_turn_penalty_is_charged_once_per_turn() {
        let flat = Request::new(at(1, 0, 1), at(4, 0, 4));
        let straight = plan_with(&Grid::new(), &flat).expect("six steps, one corner");
        assert_eq!(straight.total_cost(), 6.0, "six steps at 1, no penalty");

        let turning = flat.with_costs(Costs::default().with_turn_penalty(5.0));
        let bent = plan_with(&Grid::new(), &turning).expect("the same route");
        assert_eq!(
            bent.total_cost(),
            11.0,
            "one right angle, five for it (AbstractPathJob.java:1763-1807)"
        );
    }

    #[test]
    fn the_ceiling_of_a_range_is_the_range_squared() {
        assert_eq!(Limits::for_range(20).max_nodes, 420, "400 + the range");
        assert_eq!(
            Limits::for_range(200).max_nodes,
            MAX_NODES + 200,
            "capped at MAX_NODES, then the range is added"
        );
        assert_eq!(
            Limits::for_range(-1).max_nodes,
            0,
            "a negative range is no range"
        );
        assert_eq!(Limits::new(100).limit(0.0), 100);
        assert_eq!(Limits::new(100).limit(3.0), 118, "100 + 3² × 2");
        assert_eq!(
            Limits::new(100).limit(1_000.0),
            1_350,
            "and no step relaxes it past MAX_COST² × 2"
        );
    }
}
