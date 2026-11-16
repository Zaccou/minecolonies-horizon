//! A position, the kind of move that entered it, and what that move costs.
//!
//! Java reference: `core/entity/pathfinding/MNode.java` (407 lines),
//! `PathPointExtended.java` (168), `SurfaceType.java` (134) and
//! `PathingOptions.java` (394) — plus the cost function itself,
//! `pathjobs/AbstractPathJob.java:1018-1121`.
//!
//! **What a node is.** Java's `Node` is a block position, four booleans —
//! ladder, swimming, rails, corner node (`MNode.java:59-77`) — and a cost,
//! which in Java is the A\* `g` value: the cost *from the start*, not of the
//! last step (`MNode.java:44`). [`Node`] keeps the position, the cost, and the
//! one flag MineColonies prices: [`NodeKind`].
//!
//! **What a node is not.** It carries no parent pointer. Java's `MNode.parent`
//! (`:34`) is the search's business, not the path's: the frontier keeps the
//! parent in its own table, so a finished [`Node`] is a plain value that can be
//! copied into a `Vec`, compared, and handed to the engine.
//!
//! **The five kinds** are the ones MineColonies actually distinguishes when it
//! builds a path: the surface test says walkable or dropable
//! (`SurfaceType.java:22-23`), the node flags say ladder and swimming
//! (`MNode.java:59-66`), and a door is a toggleable whose traversal is priced
//! apart (`PathingOptions.java:58`). Rails and corner nodes are **not** here —
//! see `RAPPORT_PORT_PATHFINDING.md` for what each would cost.

/// A block position: `[x, y, z]`, integer block coordinates.
///
/// A plain array, as everywhere else in this port: it is `Copy`, it compares
/// and orders, and it keys a `BTreeMap` without a wrapper.
pub type Pos = [i32; 3];

/// `[x, y, z]`, in the order the arrays read.
pub const fn at(x: i32, y: i32, z: i32) -> Pos {
    [x, y, z]
}

/// The position one block up.
pub const fn up(pos: Pos) -> Pos {
    [pos[0], pos[1] + 1, pos[2]]
}

/// The position one block down: for a node, the block it stands on.
pub const fn down(pos: Pos) -> Pos {
    [pos[0], pos[1] - 1, pos[2]]
}

/// `a + delta`, coordinate by coordinate.
pub const fn offset(a: Pos, delta: Pos) -> Pos {
    [a[0] + delta[0], a[1] + delta[1], a[2] + delta[2]]
}

/// The four horizontal neighbours, in Java's exploration order: north, east,
/// south, west (`AbstractPathJob.java:701-719`).
///
/// **There is no diagonal here, on purpose.** Java never steps diagonally
/// either: a diagonal move is two straight nodes, and the "corner node" that
/// joins them is an artefact of its shape check, not a move
/// (`AbstractPathJob.java:786-840`).
pub const HORIZONTAL: [Pos; 4] = [[0, 0, -1], [1, 0, 0], [0, 0, 1], [-1, 0, 0]];

/// The block distance, x plus y plus z — Java's `distManhattan`
/// (`PathJobMoveToLocation.java:72-75`).
pub fn manhattan(a: Pos, b: Pos) -> i32 {
    (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs()
}

/// The squared Euclidean distance, all three axes — Java's `distSqr`
/// (`PathJobMoveToLocation.java:91-95`).
pub fn squared_distance(a: Pos, b: Pos) -> i32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    dx * dx + dy * dy + dz * dz
}

/// The horizontal direction of a step, as signs: Java's
/// `Integer.signum(from.x - prev.x)` (`AbstractPathJob.java:1777-1783`).
pub fn horizontal_direction(from: Pos, to: Pos) -> (i32, i32) {
    ((to[0] - from[0]).signum(), (to[2] - from[2]).signum())
}

/// The highest step up a body can take, in blocks.
///
/// Java measures it: `targetMaxY - parentMaxY < MAX_JUMP_HEIGHT`, with
/// `MAX_JUMP_HEIGHT = 1.3` (`AbstractPathJob.java:1558`,
/// `api/util/constant/PathingConstants.java:44`). At block resolution that is
/// **one** block and never two — which is why the port keeps it as an integer,
/// and why [`MAX_JUMP_STEP`] is 1 rather than a float.
pub const MAX_JUMP_HEIGHT: f64 = 1.3;

/// [`MAX_JUMP_HEIGHT`] rounded down to whole blocks: the port's step ceiling.
pub const MAX_JUMP_STEP: i32 = 1;

/// The deepest fall Java will take, in blocks: the scan of `checkDrop` runs to
/// 10 when `PathingOptions.canDrop` is true (`AbstractPathJob.java:1624`).
pub const MAX_DROP: i32 = 10;

/// The deepest fall when the entity may not drop: the same scan stops at 2
/// (`AbstractPathJob.java:1624`, `PathingOptions.java:125`).
pub const MAX_DROP_FORBIDDEN: i32 = 2;

/// The largest step cost Java will relax its node ceiling for: `MAX_COST = 25`
/// (`PathingOptions.java:13`, used at `AbstractPathJob.java:891`).
pub const MAX_COST: f64 = 25.0;

/// How many nodes a search may pull off the frontier before the relaxation of
/// [`super::search::Limits::limit`]: `public static final int MAX_NODES = 8000`
/// (`AbstractPathJob.java:59`).
pub const MAX_NODES: u32 = 8000;

/// How a node was entered, which is what MineColonies prices differently.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: this port's own closed set, one variant per Java price
pub enum NodeKind {
    /// Ordinary ground: `SurfaceType.WALKABLE` (`SurfaceType.java:22`), base
    /// cost 1 (`AbstractPathJob.java:1030`).
    Walk,
    /// In water: `calculateSwimming` is true (`AbstractPathJob.java:1738-1748`)
    /// and the entity may swim (`PathingOptions.java:92`), priced by
    /// `swimCost` / `swimCostEnter` (`:43`, `:53`).
    Swim,
    /// On a ladder or a vine: `PathPointExtended.onLadder`
    /// (`PathPointExtended.java:22-27`), priced by the jump cost when the climb
    /// starts (`AbstractPathJob.java:1055-1060`) plus `nonLadderClimbableCost`.
    Climb,
    /// A door or a gate: `SurfaceType` marks both `DROPABLE`
    /// (`SurfaceType.java:79-82`), and entering one costs
    /// `traverseToggleAbleCost` (`PathingOptions.java:58`).
    Door,
    /// A fall onto the first standable block below: `SurfaceType.DROPABLE`
    /// (`SurfaceType.java:23`), chosen by `checkDrop`
    /// (`AbstractPathJob.java:1609-1643`), priced by `dropCost` on the cube of
    /// the depth (`:1062-1065`).
    Fall,
}

impl NodeKind {
    /// The name this kind is written under in a refusal or a report.
    pub const fn name(self) -> &'static str {
        match self {
            NodeKind::Walk => "walk",
            NodeKind::Swim => "swim",
            NodeKind::Climb => "climb",
            NodeKind::Door => "door",
            NodeKind::Fall => "fall",
        }
    }
}

impl std::fmt::Display for NodeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// What a block can be to a walker: the three answers of `SurfaceType`
/// (`SurfaceType.java:20-29`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one variant per Java surface type, no more
pub enum Surface {
    /// Standable: the block's top carries a body (`SurfaceType.java:22`).
    Walkable,
    /// Not standable, but a body falls through it: air, a sign, a door leaf
    /// (`SurfaceType.java:23`).
    Dropable,
    /// Neither: a fence, a wall, the colony's own blocks, lava
    /// (`SurfaceType.java:70-77`, `:100-103`).
    NotPassable,
}

impl Surface {
    /// Whether a body can stand on this block's top.
    pub const fn is_walkable(self) -> bool {
        matches!(self, Surface::Walkable)
    }

    /// Whether falling through it is allowed at all.
    pub const fn is_dropable(self) -> bool {
        matches!(self, Surface::Dropable)
    }

    /// The name this surface is written under in a refusal or a report.
    pub const fn name(self) -> &'static str {
        match self {
            Surface::Walkable => "walkable",
            Surface::Dropable => "dropable",
            Surface::NotPassable => "not passable",
        }
    }
}

/// One node of a path: where, how it was entered, and the cost so far.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Node {
    /// The block the body's feet are in.
    pub at: Pos,
    /// The kind of move that entered it. The first node of a path is
    /// [`NodeKind::Walk`] unless the start cell says otherwise.
    pub kind: NodeKind,
    /// The A\* `g` value: the cost from the start of the path to here, this
    /// node included (`MNode.java:44`).
    pub cost: f64,
}

impl Node {
    /// A node with its cumulative cost.
    pub const fn new(at: Pos, kind: NodeKind, cost: f64) -> Node {
        Node { at, kind, cost }
    }

    /// The start of a path: cost 0, as Java sets it
    /// (`AbstractPathJob.java:317`).
    pub const fn start(at: Pos, kind: NodeKind) -> Node {
        Node {
            at,
            kind,
            cost: 0.0,
        }
    }
}

/// Every price `PathingOptions` puts on a move, in the same units.
///
/// **Why a struct and not constants.** `PathingOptions` is per-job in Java
/// (`PathingOptions.java:16-131`): a builder walks at the default prices, a
/// raider drops further, a swimmer swims cheaper. The port keeps the same
/// object, so a caller can price a job without touching the search.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Costs {
    /// Base cost of any step: the literal `1` `computeCost` starts from
    /// (`AbstractPathJob.java:1030`).
    pub base: f64,
    /// Added for a step up: `jumpCost = 2D` (`PathingOptions.java:18`).
    pub jump: f64,
    /// Added for a fall, multiplied by the cube of its depth:
    /// `dropCost = 1D` (`PathingOptions.java:23`, used at
    /// `AbstractPathJob.java:1062-1065`).
    pub drop: f64,
    /// Added for entering water: `swimCost = 4D` (`PathingOptions.java:43`).
    pub swim: f64,
    /// Added for the first wet step, instead of [`Self::swim`]:
    /// `swimCostEnter = 24D` (`PathingOptions.java:53`).
    pub swim_enter: f64,
    /// Added for entering a climbable that is not a plain ladder:
    /// `nonLadderClimbableCost = 3D` (`PathingOptions.java:63`).
    ///
    /// The port charges it on **every** climb entry, because [`super::world::WorldView`]
    /// has one `is_climbable` and does not tell a ladder from a vine; Java tells
    /// them apart (`PathfindingUtils.isLadder`, `PathfindingUtils.java:367-445`)
    /// and charges a ladder nothing.
    pub climb: f64,
    /// Added for passing a toggleable: `traverseToggleAbleCost = 2D`
    /// (`PathingOptions.java:58`, used at `AbstractPathJob.java:1080-1083`).
    pub toggle: f64,
    /// Added per turn, by [`Self::turn`]: `turnPenalty` is **0** by default
    /// (`PathingOptions.java:130`), which is what every job that does not ask
    /// for it gets.
    pub turn: f64,
    /// Whether water may be entered at all: `canSwim = false` by default
    /// (`PathingOptions.java:92`), and a wet node is then simply not created
    /// (`AbstractPathJob.java:858-861`).
    pub can_swim: bool,
    /// Whether a fall of more than two blocks is allowed: `canDrop = true` by
    /// default (`PathingOptions.java:125`).
    pub can_drop: bool,
    /// Whether a door may be entered: `enterDoors = false` by default
    /// (`PathingOptions.java:96`).
    pub enter_doors: bool,
    /// Whether a gate may be entered: `enterGates = true` by default
    /// (`PathingOptions.java:101`).
    pub enter_gates: bool,
}

impl Default for Costs {
    /// The defaults of `PathingOptions` (`PathingOptions.java:16-131`), value
    /// by value.
    fn default() -> Costs {
        Costs {
            base: 1.0,
            jump: 2.0,
            drop: 1.0,
            swim: 4.0,
            swim_enter: 24.0,
            climb: 3.0,
            toggle: 2.0,
            turn: 0.0,
            can_swim: false,
            can_drop: true,
            enter_doors: false,
            enter_gates: true,
        }
    }
}

impl Costs {
    /// `withCanSwim` (`PathingOptions.java:337-347`).
    pub fn with_can_swim(mut self, can_swim: bool) -> Costs {
        self.can_swim = can_swim;
        self
    }

    /// `withCanEnterDoors` (`PathingOptions.java:349-357`).
    pub fn with_enter_doors(mut self, enter_doors: bool) -> Costs {
        self.enter_doors = enter_doors;
        self
    }

    /// `setEnterGates` for the gate half of the same switch
    /// (`PathingOptions.java:209-212`).
    pub fn with_enter_gates(mut self, enter_gates: bool) -> Costs {
        self.enter_gates = enter_gates;
        self
    }

    /// `canDrop` (`PathingOptions.java:125`).
    pub fn with_can_drop(mut self, can_drop: bool) -> Costs {
        self.can_drop = can_drop;
        self
    }

    /// `withTurnPenalty` (`PathingOptions.java:325-333`).
    pub fn with_turn_penalty(mut self, turn: f64) -> Costs {
        self.turn = turn;
        self
    }

    /// Whether a door or a gate may be walked through.
    ///
    /// Java has two switches, `canEnterDoors` and `canEnterGates`
    /// (`PathingOptions.java:96`, `:101`); the port's world view answers one
    /// `is_door`, so the rule is "either switch opens it".
    pub const fn doors_open(&self) -> bool {
        self.enter_doors || self.enter_gates
    }

    /// The deepest fall this entity may take: [`MAX_DROP`] or
    /// [`MAX_DROP_FORBIDDEN`].
    pub const fn deepest_fall(&self) -> i32 {
        if self.can_drop {
            MAX_DROP
        } else {
            MAX_DROP_FORBIDDEN
        }
    }

    /// The `g` cost of one step, `computeCost` read line by line
    /// (`AbstractPathJob.java:1018-1121`).
    ///
    /// **What is not in it**, because the port's world view cannot answer:
    /// the on-path and on-rail discounts (`:1037-1045`), the cave-air surcharge
    /// (`:1051`), the "wrong facing" cost of a directional block
    /// (`:1068-1077`, `:1131-1147`), the shingle and rail-exit surcharges
    /// (`:1094-1103`), the stair exception to the jump cost (`:1050`), and the
    /// small random jitter Java adds to every step (`:1032-1035`) — the port is
    /// deterministic on purpose, so a test can assert a cost and not a range.
    pub fn step(&self, from: NodeKind, to: NodeKind, dy: i32, swim_start: bool) -> f64 {
        let mut cost = self.base;

        // A step up costs a jump, unless the climb simply continues: Java skips
        // the jump when both nodes are ladders (`AbstractPathJob.java:1055-1060`).
        if dy > 0 && !(to == NodeKind::Climb && from == NodeKind::Climb) {
            cost += self.jump;
        } else if dy < 0 && self.drop != 0.0 {
            // A fall is priced on the cube of its depth, so a 3-deep drop costs
            // 27 and a 1-deep one 1 (`AbstractPathJob.java:1062-1065`).
            let depth = f64::from(dy.abs());
            cost += self.drop * depth * depth * depth;
        }

        if to == NodeKind::Door {
            cost += self.toggle;
        }

        if to == NodeKind::Climb && from != NodeKind::Climb {
            cost += self.climb;
        }

        if to == NodeKind::Swim {
            cost += if swim_start {
                self.swim_enter
            } else {
                self.swim
            };
        }

        cost
    }

    /// The extra cost of a turn, `computeTurnPenalty`
    /// (`AbstractPathJob.java:1763-1807`).
    ///
    /// Both arguments are horizontal **signs**, as [`horizontal_direction`]
    /// gives them. A vertical move on either side is not a turn at all, a
    /// reversal costs double, a right angle the full penalty, and the
    /// diagonal-to-cardinal case half of it (`:1791-1806`).
    pub fn turn(&self, previous: (i32, i32), current: (i32, i32)) -> f64 {
        if self.turn == 0.0 {
            return 0.0;
        }
        if (previous.0 == 0 && previous.1 == 0) || (current.0 == 0 && current.1 == 0) {
            return 0.0;
        }
        if previous == current {
            return 0.0;
        }
        match previous.0 * current.0 + previous.1 * current.1 {
            1 => self.turn * 0.5,
            0 => self.turn,
            _ => self.turn * 2.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_level_step_costs_one_and_a_jump_two_more() {
        let costs = Costs::default();
        assert_eq!(costs.step(NodeKind::Walk, NodeKind::Walk, 0, false), 1.0);
        assert_eq!(costs.step(NodeKind::Walk, NodeKind::Walk, 1, false), 3.0);
    }

    #[test]
    fn a_fall_costs_the_cube_of_its_depth() {
        let costs = Costs::default();
        assert_eq!(costs.step(NodeKind::Walk, NodeKind::Fall, -1, false), 2.0);
        assert_eq!(costs.step(NodeKind::Walk, NodeKind::Fall, -3, false), 28.0);
    }

    #[test]
    fn water_costs_twenty_four_to_enter_and_four_after() {
        let costs = Costs::default();
        assert_eq!(costs.step(NodeKind::Walk, NodeKind::Swim, 0, true), 25.0);
        assert_eq!(costs.step(NodeKind::Swim, NodeKind::Swim, 0, false), 5.0);
    }

    #[test]
    fn continuing_a_climb_is_cheaper_than_starting_one() {
        let costs = Costs::default();
        assert_eq!(costs.step(NodeKind::Walk, NodeKind::Climb, 1, false), 6.0);
        assert_eq!(costs.step(NodeKind::Climb, NodeKind::Climb, 1, false), 1.0);
    }

    #[test]
    fn a_turn_penalty_only_exists_when_it_is_asked_for() {
        let straight = Costs::default();
        assert_eq!(straight.turn((1, 0), (0, 1)), 0.0, "off by default");

        let turning = Costs::default().with_turn_penalty(2.0);
        assert_eq!(turning.turn((1, 0), (1, 0)), 0.0, "straight ahead");
        assert_eq!(turning.turn((1, 0), (0, 1)), 2.0, "a right angle");
        assert_eq!(turning.turn((1, 0), (-1, 0)), 4.0, "a reversal");
        assert_eq!(
            turning.turn((0, 0), (1, 0)),
            0.0,
            "a vertical move is no turn"
        );
    }

    #[test]
    fn the_fall_ceiling_follows_the_switch() {
        assert_eq!(Costs::default().deepest_fall(), MAX_DROP);
        assert_eq!(
            Costs::default().with_can_drop(false).deepest_fall(),
            MAX_DROP_FORBIDDEN
        );
    }
}
