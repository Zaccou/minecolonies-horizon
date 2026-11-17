//! The fourteen jobs: what each one aims at, how far it looks, what it pays.
//!
//! Java has one class per intent — walk to a place, walk away from a raider,
//! find water, find a tree, patrol a road — and every class overrides the same
//! four things: `computeHeuristic`, `isAtDestination`, `getEndNodeScore` and
//! `setPathingOptions`, plus its own node ceiling. This file is that table, one
//! [`Job`] variant per class, each with the Java line it comes from.
//!
//! **Why an enum and not fourteen types.** A job is data (a destination, a
//! radius, a direction) and no behaviour beyond these four answers: a caller
//! picks a variant, the search asks the questions. That keeps the module free
//! of a trait object per job, and lets a test build a job in one line.
//!
//! **What is not modelled.** Java's `PathJobFindWater` starts a whole second
//! search (`PathJobFindFishingPos`, `:251-306`) from inside its goal test; a
//! goal test here is a predicate, so the shore is a question asked of the caller
//! ([`GoalFacts::has_fishing_shore`]). `PathJobRaiderPathing`'s `addCost` audit
//! (`:136-144`) needs the raiders' own bookkeeping and is not reproduced; its
//! two flat surcharges are ([`Job::modify_cost`]).

use crate::pathfinding::{Costs, Pos, WorldView, down};

// ---------------------------------------------------------------- distances

/// The block distance, x plus y plus z — Java's `distManhattan`.
pub fn manhattan(a: Pos, b: Pos) -> f64 {
    f64::from((a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs())
}

/// The squared distance, all three axes — Java's `distSqr`, in `i64` because
/// Java's `int` overflows past 46 340 blocks and a refusal must not be a
/// silence.
pub fn sq(a: Pos, b: Pos) -> i64 {
    let dx = i64::from(a[0] - b[0]);
    let dy = i64::from(a[1] - b[1]);
    let dz = i64::from(a[2] - b[2]);
    dx * dx + dy * dy + dz * dz
}

/// The squared distance with `b`'s height replaced by `y` — Java's
/// `distSqr(pos, x, destination.getY(), z)` (`PathJobSignConnection.java:25`).
pub fn sq_at_height(a: Pos, b: Pos, y: i32) -> i64 {
    sq(a, [b[0], y, b[2]])
}

/// The Euclidean distance — Java's `BlockPosUtil.dist`, which is
/// `Math.sqrt(distSqr)`.
pub fn distance(a: Pos, b: Pos) -> f64 {
    (sq(a, b) as f64).sqrt()
}

/// An axis-aligned box in block coordinates: the `AABB` of block corners
/// (`PathJobFindTree.java:111-116`, `PathJobRandomPos.java:119-125`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Box3 {
    /// The low corner, per axis.
    pub min: Pos,
    /// The high corner, per axis.
    pub max: Pos,
}

impl Box3 {
    /// The box spanning two corners, in either order — Java's `Math.min` /
    /// `Math.max` per axis.
    pub fn between(a: Pos, b: Pos) -> Box3 {
        Box3 {
            min: [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])],
            max: [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])],
        }
    }

    /// Whether the box holds this position — `AABB.contains(x, y, z)`, which is
    /// inclusive on both faces.
    pub const fn holds(&self, pos: Pos) -> bool {
        pos[0] >= self.min[0]
            && pos[0] <= self.max[0]
            && pos[1] >= self.min[1]
            && pos[1] <= self.max[1]
            && pos[2] >= self.min[2]
            && pos[2] <= self.max[2]
    }

    /// The centre, floored — `BlockPos.containing(box.getCenter())`
    /// (`PathJobRandomPos.java:125`).
    pub fn centre(&self) -> Pos {
        [
            (self.min[0] + self.max[0]) / 2,
            (self.min[1] + self.max[1]) / 2,
            (self.min[2] + self.max[2]) / 2,
        ]
    }
}

// ------------------------------------------------------------------- facts

/// The questions a goal test asks that a [`WorldView`] does not answer.
///
/// Every method has a default that says "no", so a job that needs none of them
/// (walk to this position) costs a caller nothing, and a test double overrides
/// only what the job it exercises reads.
pub trait GoalFacts: WorldView {
    /// The cell is air: Java's `state.isAir()` (`PathJobEscapeWater.java:79`).
    fn is_air(&self, pos: Pos) -> bool {
        !self.is_solid(pos)
    }

    /// A tree the lumberjack may cut — `Tree.checkTree` plus
    /// `Tree.checkIfInColony` (`PathJobFindTree.java:172-181`).
    fn is_tree(&self, _pos: Pos) -> bool {
        false
    }

    /// The water cell is a usable pond: `Pond.checkPond` did not answer
    /// `INVALID` (`PathJobFindWater.java:181-183`).
    fn is_pond(&self, _water: Pos) -> bool {
        false
    }

    /// A shore within ten blocks of this pond is reachable — the answer of the
    /// nested `PathJobFindFishingPos` search (`PathJobFindWater.java:193-203`).
    fn has_fishing_shore(&self, _pond: Pos) -> bool {
        false
    }

    /// The raiders may spawn here: `RaidManager.isValidSpawnPoint`
    /// (`PathJobRaiderPathing.java:75`).
    fn is_valid_spawn(&self, _pos: Pos) -> bool {
        false
    }

    /// Nothing solid on the line from one cell's eye to another's — Java's
    /// `level.clip(..., Block.COLLIDER, ...)` returning `MISS`
    /// (`PathJobCanSee.java:254-259`).
    fn can_see(&self, _from: Pos, _to: Pos) -> bool {
        false
    }

    /// How many colony buildings claim the chunk of this position: Java reads
    /// it to divide the heuristic (`PathJobPathway.java:60`).
    fn claiming_buildings(&self, _pos: Pos) -> u32 {
        0
    }

    /// Whether this position's chunk belongs to the job's colony — the left
    /// half of `ColonyUtils.getOwningColony(chunk) == colonyid`
    /// (`PathJobPathway.java:58`).
    fn in_colony(&self, _pos: Pos) -> bool {
        false
    }

    /// There is room above this cell: `hasSpaceAbove`
    /// (`PathJobRandomPos.java:168-186`, used at `:213`).
    fn has_space_above(&self, _pos: Pos) -> bool {
        false
    }

    /// A walker would be stopped here — Java's `super.isPassable(...)` in the
    /// raider job (`PathJobRaiderPathing.java:164`).
    fn blocked(&self, pos: Pos) -> bool {
        self.is_solid(pos)
    }
}

/// One node offered to a goal test: where, where from, and how it was entered.
///
/// Java passes the whole `MNode`, which carries its parent, its ladder and
/// swimming flags (`MNode.java:34-77`), and whose own parent is reachable from
/// it (`PathJobPathway.java:116-124` walks two links up). Those are what a goal
/// test or a cost modifier reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reach {
    /// The node's position.
    pub at: Pos,
    /// The node it was entered from, `None` for the start.
    pub from: Option<Pos>,
    /// The node before that, when there is one.
    pub grandparent: Option<Pos>,
    /// The node is in water (`MNode.isSwimming`).
    pub swimming: bool,
}

impl Reach {
    /// A node with no history, out of water: what a start looks like.
    pub const fn alone(at: Pos) -> Reach {
        Reach {
            at,
            from: None,
            grandparent: None,
            swimming: false,
        }
    }

    /// A node entered from `from`, with no grandparent.
    pub const fn from(at: Pos, from: Pos) -> Reach {
        Reach {
            at,
            from: Some(from),
            grandparent: None,
            swimming: false,
        }
    }

    /// The straight step out of `from`, with `grandparent` behind it.
    pub const fn through(at: Pos, from: Pos, grandparent: Pos) -> Reach {
        Reach {
            at,
            from: Some(from),
            grandparent: Some(grandparent),
            swimming: false,
        }
    }
}

/// The two block facts a cost modifier may need, both known to the search when
/// it prices a step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    /// The body has ground under it here (Java prices a step into the air
    /// differently, `PathJobRaiderPathing.java:169-171`).
    pub on_ground: bool,
    /// A walker could occupy this cell.
    pub passable: bool,
}

impl Step {
    /// A step onto ordinary ground.
    pub const ORDINARY: Step = Step {
        on_ground: true,
        passable: true,
    };
}

// ----------------------------------------------------------------- options

/// Every knob of `PathingOptions`, not just the ones the first version's
/// [`Costs`] carries.
///
/// **Why a second struct.** `Costs` keeps the twelve fields `computeCost` reads
/// from a node alone; Java's object also holds seven prices that need the block
/// *state* (on-path, on-rail, rail exit, cave air, walking inside a shape,
/// diving, wrong facing) and five permissions the first version's search cannot
/// ask (`canUseRails`, `canOpenDoors`, `canClimbAdvanced`, `canPassDanger`,
/// `walkUnderWater`). A job sets several at once (`PathJobPathway.java:50` sets
/// five), so they are kept here, faithful, and [`Self::costs`] narrows to what
/// the search honours today — `MANQUANT.md` §2 says what is lost.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    /// `jumpCost = 2` (`PathingOptions.java:18`).
    pub jump: f64,
    /// `dropCost = 1` (`:23`).
    pub drop: f64,
    /// `onPathCost = 1/6` (`:28`).
    pub on_path: f64,
    /// `onRailCost = 1/10` (`:33`).
    pub on_rail: f64,
    /// `railsExitCost = 4` (`:38`).
    pub rails_exit: f64,
    /// `swimCost = 4` (`:43`).
    pub swim: f64,
    /// `caveAirCost = 3` (`:48`).
    pub cave_air: f64,
    /// `swimCostEnter = 24` (`:53`).
    pub swim_enter: f64,
    /// `traverseToggleAbleCost = 2` (`:58`).
    pub toggle: f64,
    /// `nonLadderClimbableCost = 3` (`:63`).
    pub climb: f64,
    /// `walkInShapesCost = 1` (`:68`).
    pub in_shapes: f64,
    /// `divingCost = 4` (`:73`).
    pub diving: f64,
    /// `badDirectionCost = 5` (`:78`).
    pub bad_direction: f64,
    /// `randomnessFactor = 0.1` (`:83`) — the port turns it off on purpose.
    pub randomness: f64,
    /// `canUseRails = false` (`:88`).
    pub rails: bool,
    /// `canSwim = false` (`:92`).
    pub swim_allowed: bool,
    /// `enterDoors = false` (`:96`).
    pub doors: bool,
    /// `enterGates = true` (`:101`).
    pub gates: bool,
    /// `canOpenDoors = false` (`:106`).
    pub open_doors: bool,
    /// `canClimbAdvanced = false` (`:110`).
    pub climb_advanced: bool,
    /// `canPassDanger = false` (`:115`).
    pub pass_danger: bool,
    /// `walkUnderWater = false` (`:120`).
    pub under_water: bool,
    /// `canDrop = true` (`:125`).
    pub drop_allowed: bool,
    /// `turnPenalty = 0` (`:130`).
    pub turn: f64,
}

impl Default for Options {
    /// The object `new PathingOptions()` gives, value by value
    /// (`PathingOptions.java:16-131`).
    fn default() -> Options {
        Options {
            jump: 2.0,
            drop: 1.0,
            on_path: 1.0 / 6.0,
            on_rail: 1.0 / 10.0,
            rails_exit: 4.0,
            swim: 4.0,
            cave_air: 3.0,
            swim_enter: 24.0,
            toggle: 2.0,
            climb: 3.0,
            in_shapes: 1.0,
            diving: 4.0,
            bad_direction: 5.0,
            randomness: 0.1,
            rails: false,
            swim_allowed: false,
            doors: false,
            gates: true,
            open_doors: false,
            climb_advanced: false,
            pass_danger: false,
            under_water: false,
            drop_allowed: true,
            turn: 0.0,
        }
    }
}

impl Options {
    /// The first version's price table, filled from this one. The fields it has
    /// no room for stay here; `MANQUANT.md` §2 lists them.
    pub fn costs(&self) -> Costs {
        Costs {
            base: 1.0,
            jump: self.jump,
            drop: self.drop,
            swim: self.swim,
            swim_enter: self.swim_enter,
            climb: self.climb,
            toggle: self.toggle,
            turn: self.turn,
            can_swim: self.swim_allowed,
            can_drop: self.drop_allowed,
            enter_doors: self.doors,
            enter_gates: self.gates,
        }
    }

    /// `importFrom` (`PathingOptions.java:368-393`), which copies every field
    /// **but two** — it forgets `walkInShapesCost` and `badDirectionCost`. The
    /// port copies all of them: a job that inherits another's options must not
    /// silently lose a price.
    pub fn imported(mut self, other: &Options) -> Options {
        self.jump = other.jump;
        self.drop = other.drop;
        self.on_path = other.on_path;
        self.on_rail = other.on_rail;
        self.rails_exit = other.rails_exit;
        self.swim = other.swim;
        self.cave_air = other.cave_air;
        self.swim_enter = other.swim_enter;
        self.toggle = other.toggle;
        self.climb = other.climb;
        self.in_shapes = other.in_shapes;
        self.diving = other.diving;
        self.bad_direction = other.bad_direction;
        self.randomness = other.randomness;
        self.rails = other.rails;
        self.swim_allowed = other.swim_allowed;
        self.doors = other.doors;
        self.gates = other.gates;
        self.open_doors = other.open_doors;
        self.climb_advanced = other.climb_advanced;
        self.pass_danger = other.pass_danger;
        self.under_water = other.under_water;
        self.drop_allowed = other.drop_allowed;
        self.turn = other.turn;
        self
    }
}

// -------------------------------------------------------------------- jobs

/// Two ponds closer than this are one pond — `WATER_POOL_WIDTH_REQUIREMENT + 2`
/// with `WATER_POOL_WIDTH_REQUIREMENT = 5` (`api/util/Pond.java:33`, used at
/// `PathJobFindWater.java:187`).
pub const POND_SEPARATION: i32 = 7;

/// How far from its hut a fisherman looks for water — `MAX_RANGE = 100`
/// (`PathJobFindWater.java:131`).
pub const FISHING_RANGE: i64 = 100;

/// What a job is: its target data, and nothing else.
#[derive(Clone, Debug, PartialEq)]
pub enum Job {
    /// `PathJobMoveToLocation` (`:39-151`): stand at, or next to, `destination`.
    MoveToLocation {
        /// Where to end up.
        destination: Pos,
        /// `maxNodes += range` (`:43`).
        range: i32,
    },
    /// `PathJobMoveToWithPassable` (`:138-201`): the same, through blocks a
    /// caller's predicate calls passable, at three times the price.
    MoveToWithPassable {
        /// Where to end up.
        destination: Pos,
        /// `maxNodes += range`.
        range: i32,
    },
    /// `PathJobMoveAwayFromLocation` (`:19-153`): get further than
    /// `avoid_distance` from `avoid`, drifting towards `preferred`.
    MoveAway {
        /// The threat.
        avoid: Pos,
        /// How far is far enough.
        avoid_distance: i32,
        /// Where the body would rather drift: the colony centre for a citizen,
        /// else its own direction away from the threat (`:59-67`).
        preferred: Pos,
        /// The search range the caller asked for (`:54`).
        range: i32,
    },
    /// `PathJobMoveTowards` (`:168-232`): travel at least `min_distance` in the
    /// direction of `target`.
    Towards {
        /// The far point the body heads for.
        target: Pos,
        /// How far to travel before the goal counts.
        min_distance: i32,
    },
    /// `PathJobMoveCloseToXNearY` (`:17-123`): come within `distance` of
    /// `desired` without standing in its column, staying near `nearby`.
    CloseTo {
        /// The block to approach (a hut door, a pile).
        desired: Pos,
        /// The block to stay around.
        nearby: Pos,
        /// The Manhattan radius that counts as close enough.
        distance: i32,
    },
    /// `PathJobEscapeWater` (`:18-99`): get out of the water, towards
    /// `preferred`. The range is 500 (`:45`).
    EscapeWater {
        /// Where the body would rather be.
        preferred: Pos,
    },
    /// `PathJobFindWater` (`:129-307`): find a pond within 100 blocks of `hut`
    /// that no previous job used.
    FindWater {
        /// The fisherman's hut.
        hut: Pos,
        /// Ponds already fished (`:185-191`).
        ponds: Vec<Pos>,
        /// The caller's range (`:154`).
        range: i32,
    },
    /// `PathJobFindTree` (`:139-192`): stand next to a cuttable tree.
    FindTree {
        /// Which way to lean the search; `None` for the start (`:135`).
        towards: Option<Pos>,
        /// The area the tree must be in, when the job has one (`:141`).
        boxed: Option<Box3>,
        /// The caller's range (`:72`, `:102-109`).
        range: i32,
    },
    /// `PathJobWalkRandomEdge` (`:220-279`): wander to an edge, within three
    /// blocks of the guard post.
    RandomEdge {
        /// The post the body patrols around.
        around: Pos,
        /// Whether the one-in-ten edge draw succeeds on this node. Java draws
        /// per node (`:262`); a deterministic port takes the draw.
        on_edge: bool,
        /// The caller's range (`:236`).
        range: i32,
    },
    /// `PathJobRandomPos` (`:196-238`): wander at least `min_distance` away,
    /// inside a box when there is one.
    RandomPos {
        /// The random destination drawn when the job was built (`:129`).
        destination: Pos,
        /// How far from the start counts as gone.
        min_distance: i32,
        /// A ceiling on the distance to `destination`, or `-1` for none.
        max_distance: i32,
        /// The area to stay in.
        boxed: Option<Box3>,
        /// Refuse a destination with room above it (`:213`).
        prefer_inside: bool,
        /// The caller's range (`:117`).
        range: i32,
    },
    /// `PathJobCanSee` (`:199-260`): stand where `target` is visible.
    CanSee {
        /// The post to stay near.
        around: Pos,
        /// The entity to see.
        target: Pos,
        /// The caller's range (`:215`).
        range: i32,
    },
    /// `PathJobPathway` (`:46-124`): walk a road towards `end`, preferably
    /// inside the colony.
    Pathway {
        /// The far end of the road.
        end: Pos,
        /// The colony the road is in.
        colony: u32,
    },
    /// `PathJobRaiderPathing` (`:53-171`): reach a raider spawn point far from
    /// where the raiders came in.
    Raider {
        /// Where the raiders started.
        from: Pos,
        /// The spawn point to reach.
        spawn: Pos,
    },
    /// `PathJobSignConnection` (`:17-31`): reach within two blocks of a sign, at
    /// the sign's own height.
    SignConnection {
        /// The sign.
        destination: Pos,
        /// The caller's range (`:17`).
        range: i32,
    },
}

impl Job {
    /// The prices this job runs at: `new PathingOptions()` plus whatever its
    /// constructor sets.
    pub fn options(&self) -> Options {
        let mut options = Options::default();
        match self {
            // `pathingOptions.dropCost = 5` (`PathJobMoveAwayFromLocation.java:130`).
            Job::MoveAway { .. } => options.drop = 5.0,
            // `withJumpCost(30).withStartSwimCost(30).withSwimCost(5)
            //  .withCanSwim(true).withCanEnterDoors(true).withCanEnterGates(true)`
            // (`PathJobPathway.java:50`).
            Job::Pathway { .. } => {
                options.jump = 30.0;
                options.swim_enter = 30.0;
                options.swim = 5.0;
                options.swim_allowed = true;
                options.doors = true;
                options.gates = true;
            }
            // `withJumpCost(1).withStartSwimCost(1).withSwimCost(1)
            //  .withCanSwim(true).withCanEnterDoors(true).withCanEnterGates(true)
            //  .withDropCost(5)` (`PathJobRaiderPathing.java:58`).
            Job::Raider { .. } => {
                options.jump = 1.0;
                options.swim_enter = 1.0;
                options.swim = 1.0;
                options.swim_allowed = true;
                options.doors = true;
                options.gates = true;
                options.drop = 5.0;
            }
            // `setWalkUnderWater(true)`, `swimCost = 1`, `swimCostEnter = 1`,
            // `nonLadderClimbableCost = 1` (`PathJobEscapeWater.java:85-92`).
            Job::EscapeWater { .. } => {
                options.under_water = true;
                options.swim = 1.0;
                options.swim_enter = 1.0;
                options.climb = 1.0;
            }
            // `swimCostEnter = 0`, `swimCost = 0` (`PathJobFindWater.java:238-239`).
            Job::FindWater { .. } => {
                options.swim_enter = 0.0;
                options.swim = 0.0;
            }
            // `canDrop = false` (`PathJobWalkRandomEdge.java:251`,
            // `PathJobRandomPos.java:246`).
            Job::RandomEdge { .. } | Job::RandomPos { .. } => options.drop_allowed = false,
            // `withCanSwim(true).withCanEnterDoors(true).withCanEnterGates(true)`
            // (`PathJobSignConnection.java:19`).
            Job::SignConnection { .. } => {
                options.swim_allowed = true;
                options.doors = true;
                options.gates = true;
            }
            Job::MoveToLocation { .. }
            | Job::MoveToWithPassable { .. }
            | Job::Towards { .. }
            | Job::CloseTo { .. }
            | Job::FindTree { .. }
            | Job::CanSee { .. } => {}
        }
        options
    }

    /// The `maxNodes` this job starts with, from `start`.
    ///
    /// Two constructors, two formulas: a job built from a start and an end gets
    /// an estimate of the corridor between them (`AbstractPathJob.java:271-283`),
    /// a job that only searches a range gets `min(8000, range²)` (`:193`, `:222`).
    /// Then the job patches it — `maxNodes += range` for a walk-to
    /// (`PathJobMoveToLocation.java:43`), a flat 20 000 for the raiders and for
    /// signs (`PathJobRaiderPathing.java:56`, `PathJobSignConnection.java:18`).
    pub fn ceiling(&self, start: Pos) -> super::Ceiling {
        use super::Ceiling;
        match self {
            Job::MoveToLocation { destination, range }
            | Job::MoveToWithPassable { destination, range } => {
                Ceiling::of_corridor(start, *destination).plus(*range)
            }
            Job::SignConnection {
                destination,
                range: _,
            } => Ceiling::of_corridor(start, *destination).override_to(20_000),
            Job::Pathway { end, .. } => Ceiling::of_corridor(start, *end),
            Job::Raider { from, spawn } => Ceiling::of_corridor(*from, *spawn).override_to(20_000),
            // `super(world, prepareStart(entity), desiredPosition, ...)`
            // (`PathJobMoveCloseToXNearY.java:41`): the corridor to the goal.
            Job::CloseTo { desired, .. } => Ceiling::of_corridor(start, *desired),
            Job::Towards { min_distance, .. } => Ceiling::of_range(min_distance * 2),
            Job::EscapeWater { .. } => Ceiling::of_range(500),
            Job::MoveAway { range, .. }
            | Job::FindWater { range, .. }
            | Job::FindTree { range, .. }
            | Job::RandomEdge { range, .. }
            | Job::RandomPos { range, .. }
            | Job::CanSee { range, .. } => Ceiling::of_range(*range),
        }
    }

    /// The nodes Java explores *after* the first goal, looking for a better
    /// end: `extraNodes` (`PathJobMoveToLocation.java:46`,
    /// `PathJobMoveCloseToXNearY.java:46`, `PathJobWalkRandomEdge.java:238`).
    ///
    /// This is not the ceiling — it is a second, narrower pass over the frontier
    /// ordered by heuristic alone (`AbstractPathJob.java:428-489`).
    pub const fn extra_nodes(&self) -> u32 {
        match self {
            Job::MoveToLocation { .. } | Job::MoveToWithPassable { .. } => 4,
            Job::CloseTo { .. } | Job::RandomEdge { .. } => 20,
            _ => 0,
        }
    }

    /// `heuristicMod = 5.0` for the raiders (`PathJobRaiderPathing.java:57`), 1
    /// elsewhere.
    pub fn heuristic_modifier(&self) -> f64 {
        match self {
            Job::Raider { .. } => 5.0,
            _ => 1.0,
        }
    }

    /// `computeHeuristic(x, y, z)`, per job, with the Java line.
    ///
    /// Every job but the raider keeps its estimate admissible; the raider
    /// multiplies it by five on purpose, which is why its paths are greedy
    /// rather than shortest.
    pub fn heuristic<W: GoalFacts + ?Sized>(&self, view: &W, start: Pos, at: Pos) -> f64 {
        let base = match self {
            // `distManhattan(destination, x, y, z)` (`:70`).
            Job::MoveToLocation { destination, .. }
            | Job::MoveToWithPassable { destination, .. }
            | Job::SignConnection { destination, .. } => manhattan(*destination, at),
            // `dist(preferredDirection, ...)` (`:78`) — Euclidean, not Manhattan.
            Job::MoveAway { preferred, .. } => distance(*preferred, at),
            // `dist(preferredDirection, ...) * 2` (`PathJobEscapeWater.java:67`).
            Job::EscapeWater { preferred } => distance(*preferred, at) * 2.0,
            // `distManhattan(target, ...)` (`:210`).
            Job::Towards { target, .. } => manhattan(*target, at),
            // `distManhattan(desired) + distManhattan(nearby) * 2` (`:52`).
            Job::CloseTo {
                desired, nearby, ..
            } => manhattan(*desired, at) + manhattan(*nearby, at) * 2.0,
            // `distManhattan(hutLocation, ...)` (`:169`).
            Job::FindWater { hut, .. } => manhattan(*hut, at),
            // `distManhattan(searchTowards ?? start, ...)` (`:133-136`).
            Job::FindTree { towards, .. } => manhattan(towards.unwrap_or(start), at),
            // `distManhattan(searchAroundPos, ...)` (`:244`, `PathJobCanSee.java:226`).
            Job::RandomEdge { around, .. } | Job::CanSee { around, .. } => manhattan(*around, at),
            // `distManhattan(destination) + distManhattan(boxCentre) / 2` when
            // there is a box (`:188-196`).
            Job::RandomPos {
                destination, boxed, ..
            } => match boxed {
                Some(b) => manhattan(*destination, at) + manhattan(b.centre(), at) / 2.0,
                None => manhattan(*destination, at),
            },
            // `sqrt(distSqr(end)) / (buildings + 1)` inside the colony, plain
            // `sqrt` outside (`PathJobPathway.java:55-64`): a road inside the
            // colony is worth walking further for.
            Job::Pathway { end, colony: _ } => {
                let d = distance(*end, at);
                if view.in_colony(at) {
                    d / f64::from(view.claiming_buildings(at) + 1)
                } else {
                    d
                }
            }
            // `distManhattan(direction, ...)` (`:62-65`).
            Job::Raider { spawn, .. } => manhattan(*spawn, at),
        };
        base * self.heuristic_modifier()
    }

    /// `isAtDestination(node)`, per job, with the Java line.
    ///
    /// The common tail is Java's: a node is a destination only if the block
    /// under it is `WALKABLE` (`SurfaceType.getSurfaceType(...) == WALKABLE`,
    /// repeated in eleven of the fourteen jobs) — applied here once, at the end
    /// of each arm — and each arm adds its own condition.
    pub fn is_at_destination<W: GoalFacts + ?Sized>(
        &self,
        view: &W,
        start: Pos,
        reach: Reach,
    ) -> bool {
        let at = reach.at;
        let walkable = view.surface(down(at)).is_walkable();
        match self {
            // Exact when the goal can be stood in, within √2 when it cannot
            // (`:58-65`, `:82-102`). The `y == destination.y - 1` case compares
            // the horizontal square at the destination's own height (`:89-95`).
            Job::MoveToLocation { destination, .. }
            | Job::MoveToWithPassable { destination, .. } => {
                if at == *destination {
                    return walkable;
                }
                let standable = view.surface(down(*destination)).is_walkable();
                let slack = if at[1] == destination[1] - 1 {
                    sq_at_height(at, *destination, destination[1]) < 2
                } else {
                    sq(at, *destination) < 2
                };
                !standable && slack && walkable
            }
            // `dist(avoid) > avoidDistance` (`:109`).
            Job::MoveAway {
                avoid,
                avoid_distance,
                ..
            } => distance(*avoid, at) > f64::from(*avoid_distance) && walkable,
            // `distManhattan(start) > minDistance` (`:222`) — from the start,
            // not from the target.
            Job::Towards { min_distance, .. } => {
                manhattan(start, at) > f64::from(*min_distance) && walkable
            }
            // Not the desired block's own column, within `distToDesired`
            // (`:58-65`).
            Job::CloseTo {
                desired,
                distance: r,
                ..
            } => {
                !(at[0] == desired[0] && at[2] == desired[2])
                    && manhattan(*desired, at) <= f64::from(*r)
                    && walkable
            }
            // Air at the feet and at the head, ground under (`:79-81`).
            Job::EscapeWater { .. } => {
                view.is_air(at) && view.is_air([at[0], at[1] + 1, at[2]]) && walkable
            }
            // A swimming node on a pond no earlier job used, with a shore
            // (`:173-204`).
            Job::FindWater { hut, ponds, .. } => {
                if sq(*hut, at) > FISHING_RANGE * FISHING_RANGE {
                    return false;
                }
                let water = down(at);
                if !reach.swimming || !view.is_pond(water) {
                    return false;
                }
                let reused = ponds.iter().any(|pond| {
                    (pond[0] - at[0]).abs() + (pond[1] - at[1]).abs() + (pond[2] - at[2]).abs()
                        < POND_SEPARATION
                });
                !reused && view.has_fishing_shore(water)
            }
            // In the box, a parent to have come from, a tree beside it
            // (`:139-149`, `:151-170`).
            Job::FindTree { boxed, .. } => {
                let inside = match boxed {
                    Some(b) => b.holds(at),
                    None => true,
                };
                inside && near_tree(view, reach) && walkable
            }
            // Within three blocks of the post, and the per-node edge draw
            // succeeded (`:255-272`).
            Job::RandomEdge {
                around, on_edge, ..
            } => {
                let dy = around[1] - at[1];
                (-3..=3).contains(&dy) && *on_edge && walkable
            }
            // Far from the start, inside the box, close enough to the drawn
            // destination, dry at the feet, and no headroom when the job asks
            // for an enclosed spot (`:198-221`).
            Job::RandomPos {
                destination,
                min_distance,
                max_distance,
                boxed,
                prefer_inside,
                ..
            } => {
                let inside = match boxed {
                    Some(b) => b.holds(at),
                    None => true,
                };
                let far = sq(start, at) > i64::from(*min_distance) * i64::from(*min_distance);
                let close = *max_distance == -1
                    || sq(*destination, at) < i64::from(*max_distance) * i64::from(*max_distance);
                let dry = self.options().under_water || !view.is_water(down(at));
                inside
                    && far
                    && close
                    && dry
                    && walkable
                    && !(*prefer_inside && view.has_space_above(at))
            }
            // No more than two blocks below the post, in sight of the target,
            // on ground (`PathJobCanSee.java:230-240`).
            Job::CanSee { around, target, .. } => {
                around[1] - at[1] <= 2 && view.can_see(at, *target) && walkable
            }
            // Within five blocks of the end (`PathJobPathway.java:67-70`).
            Job::Pathway { end, .. } => sq(*end, at) < 25,
            // At least 50 from where the raiders came in, within 50 of the
            // spawn point, on a valid spawn, on ground (`:68-78`).
            Job::Raider { from, spawn } => {
                sq(*from, at) >= 50 * 50
                    && sq(*spawn, at) < 50 * 50
                    && view.is_valid_spawn(at)
                    && walkable
            }
            // Within two blocks horizontally, at the sign's own height
            // (`PathJobSignConnection.java:23-26`).
            Job::SignConnection { destination, .. } => {
                sq_at_height(*destination, at, destination[1]) <= 4
            }
        }
    }

    /// `getEndNodeScore(node)`: which node wins when no destination is reached.
    /// Lower is better.
    ///
    /// Java's default is the node's heuristic (`AbstractPathJob.java:992-995`);
    /// a job that overrides it says what a *good enough* end looks like.
    pub fn end_score<W: GoalFacts + ?Sized>(&self, view: &W, start: Pos, at: Pos) -> f64 {
        match self {
            // `-dist(avoid)` (`PathJobMoveAwayFromLocation.java:123`).
            Job::MoveAway { avoid, .. } => -distance(*avoid, at),
            // `+1000` in the desired column, else `2 × near + far`, and 50 more
            // in water (`:69-87`).
            Job::CloseTo {
                desired, nearby, ..
            } => {
                if at[0] == desired[0] && at[2] == desired[2] {
                    1_000.0
                } else {
                    let wet = if view.is_water(down(at)) { 50.0 } else { 0.0 };
                    manhattan(*desired, at) * 2.0 + manhattan(*nearby, at) + wet
                }
            }
            // `-distManhattan(around)` (`PathJobWalkRandomEdge.java:275-278`):
            // the furthest edge found is the best one.
            Job::RandomEdge { around, .. } => -manhattan(*around, at),
            // `dist` to the end (`PathJobPathway.java:73-82`); when it is under
            // 15 Java returns the node's own cost instead, which is the caller's
            // to substitute.
            Job::Pathway { end, .. } => distance(*end, at),
            // `distManhattan(hut)` (`PathJobFindWater.java:244`).
            Job::FindWater { hut, .. } => manhattan(*hut, at),
            // `distManhattan(start)` (`PathJobCanSee.java:249-251`,
            // `PathJobRandomPos.java:225-228`, `PathJobFindTree.java:183-186`).
            Job::CanSee { .. } | Job::RandomPos { .. } | Job::FindTree { .. } => {
                manhattan(start, at)
            }
            // Java's default: the heuristic.
            _ => self.heuristic(view, start, at),
        }
    }

    /// Whether a step is charged three times its price — `PASSING_COST = 3` for
    /// `PathJobMoveToWithPassable`, applied to the block entered or to the one
    /// above it (`:143`, `:176-197`).
    pub fn passing_multiplier(&self, block_passable: bool, above_passable: bool) -> f64 {
        match self {
            Job::MoveToWithPassable { .. } if block_passable || above_passable => 3.0,
            _ => 1.0,
        }
    }

    /// The extra charge a job puts on a step — `modifyCost`, with the Java line.
    ///
    /// Four jobs charge one: a flat penalty for a region to avoid, a multiplier
    /// for leaving the road's axis, and two for a raider walking through what a
    /// walker would not.
    pub fn modify_cost<W: GoalFacts + ?Sized>(
        &self,
        view: &W,
        cost: f64,
        reach: Reach,
        step: Step,
    ) -> f64 {
        match self {
            // `dist(avoid) < 3` costs 100 more (`PathJobMoveAwayFromLocation.java:92-95`).
            Job::MoveAway { avoid, .. } => {
                if distance(*avoid, reach.at) < 3.0 {
                    cost + 100.0
                } else {
                    cost
                }
            }
            // Ten times the price for a step that leaves the axis its parent
            // kept — a road is walked straight (`PathJobPathway.java:116-124`).
            Job::Pathway { .. } => match (reach.from, reach.grandparent) {
                (Some(parent), Some(grand)) => {
                    let left_x = parent[0] == grand[0] && reach.at[0] != parent[0];
                    let left_z = parent[2] == grand[2] && reach.at[2] != parent[2];
                    if left_x || left_z { cost * 10.0 } else { cost }
                }
                _ => cost,
            },
            // Away from the hut, ten times the price (`PathJobFindWater.java:226-229`).
            Job::FindWater { hut, .. } => {
                if sq(*hut, reach.at) > FISHING_RANGE * FISHING_RANGE {
                    cost * 10.0
                } else {
                    cost
                }
            }
            // `THROUGH_BLOCK_COST = 30` for a step a walker could not take and
            // for a step without ground under it (`PathJobRaiderPathing.java:31`,
            // `:164-171`).
            Job::Raider { .. } => {
                if !step.passable
                    || !view.surface(down(reach.at)).is_walkable()
                    || !view.surface(down(reach.at)).is_walkable()
                    || !step.on_ground
                {
                    cost * 30.0
                } else {
                    cost
                }
            }
            _ => cost,
        }
    }

    /// The node ceiling policy Java overrides, `stopOnNodeLimit`.
    pub fn stop_policy(&self) -> super::StopPolicy {
        use super::StopPolicy;
        match self {
            Job::MoveToLocation { .. } | Job::MoveToWithPassable { .. } => {
                StopPolicy::MoveToLocation
            }
            Job::CloseTo { .. } => StopPolicy::CloseTo,
            _ => StopPolicy::Immediate,
        }
    }
}

/// Whether a tree stands in one of the three cells beside `reach`, in the
/// direction it came from (`PathJobFindTree.java:151-170`).
fn near_tree<W: GoalFacts + ?Sized>(view: &W, reach: Reach) -> bool {
    let Some(from) = reach.from else {
        return false;
    };
    let at = reach.at;
    if at[0] == from[0] {
        let dz = if at[2] > from[2] { 1 } else { -1 };
        view.is_tree([at[0], at[1], at[2] + dz])
            || view.is_tree([at[0] - 1, at[1], at[2]])
            || view.is_tree([at[0] + 1, at[1], at[2]])
    } else {
        let dx = if at[0] > from[0] { 1 } else { -1 };
        view.is_tree([at[0] + dx, at[1], at[2]])
            || view.is_tree([at[0], at[1], at[2] - 1])
            || view.is_tree([at[0], at[1], at[2] + 1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pathfinding::{Costs, WorldView};

    /// A floor at `y = -1`, nothing else: every goal that only asks for ground
    /// works here.
    struct Floor;
    impl WorldView for Floor {
        fn is_solid(&self, pos: Pos) -> bool {
            pos[1] < 0
        }
    }
    impl GoalFacts for Floor {}

    #[test]
    fn the_two_distances_java_uses_are_not_the_same() {
        let a = [0, 0, 0];
        let b = [3, 4, 0];
        assert_eq!(manhattan(a, b), 7.0, "BlockPosUtil.distManhattan");
        assert_eq!(sq(a, b), 25, "BlockPosUtil.distSqr");
        assert_eq!(distance(a, b), 5.0, "BlockPosUtil.dist");
    }

    #[test]
    fn a_box_holds_its_two_faces_and_its_centre_floors() {
        let b = Box3::between([5, 2, 5], [1, 0, 1]);
        assert_eq!(b.min, [1, 0, 1], "corners are ordered");
        assert!(b.holds([1, 0, 1]) && b.holds([5, 2, 5]), "both faces");
        assert!(!b.holds([6, 1, 3]), "past the high face");
        assert_eq!(b.centre(), [3, 1, 3]);
    }

    #[test]
    fn the_defaults_are_the_java_defaults() {
        let o = Options::default();
        assert_eq!(
            (o.jump, o.drop, o.swim, o.swim_enter, o.toggle, o.climb),
            (2.0, 1.0, 4.0, 24.0, 2.0, 3.0)
        );
        assert_eq!((o.cave_air, o.diving, o.bad_direction), (3.0, 4.0, 5.0));
        assert_eq!(
            (o.rails, o.swim_allowed, o.doors, o.gates, o.drop_allowed),
            (false, false, false, true, true)
        );
        assert!((o.on_path - 1.0 / 6.0).abs() < 1e-12, "onPathCost = 1/6");
    }

    #[test]
    fn options_narrow_to_the_first_versions_costs() {
        let costs = Job::Pathway {
            end: [0, 0, 0],
            colony: 1,
        }
        .options()
        .costs();
        assert_eq!(costs.jump, 30.0, "PathJobPathway.java:50");
        assert_eq!(costs.swim, 5.0);
        assert_eq!(costs.swim_enter, 30.0);
        assert!(costs.can_swim && costs.enter_doors && costs.enter_gates);
    }

    #[test]
    fn a_cost_table_built_from_options_is_the_first_versions() {
        assert_eq!(Options::default().costs(), Costs::default());
    }

    #[test]
    fn importing_options_copies_the_two_fields_the_java_forgets() {
        let other = Options {
            in_shapes: 9.0,
            bad_direction: 7.0,
            ..Options::default()
        };
        let merged = Options::default().imported(&other);
        assert_eq!(merged.in_shapes, 9.0, "Java's importFrom drops this");
        assert_eq!(merged.bad_direction, 7.0);
    }

    #[test]
    fn a_raider_is_priced_cheaply_and_five_times_more_greedy() {
        let job = Job::Raider {
            from: [0, 0, 0],
            spawn: [10, 0, 0],
        };
        let o = job.options();
        assert_eq!((o.jump, o.swim, o.swim_enter, o.drop), (1.0, 1.0, 1.0, 5.0));
        assert_eq!(job.heuristic_modifier(), 5.0, ":57");
        assert_eq!(
            job.heuristic(&Floor, [0, 0, 0], [4, 0, 0]),
            30.0,
            "6 blocks from the spawn, ×5 (:57)"
        );
    }

    #[test]
    fn an_escape_from_water_costs_a_token_and_doubles_the_pull() {
        let job = Job::EscapeWater {
            preferred: [0, 0, 10],
        };
        let o = job.options();
        assert!(o.under_water, ":88");
        assert_eq!((o.swim, o.swim_enter, o.climb), (1.0, 1.0, 1.0));
        assert_eq!(job.heuristic(&Floor, [0, 0, 0], [0, 0, 0]), 20.0, "×2");
    }

    #[test]
    fn a_wander_that_may_not_drop() {
        let pos = Job::RandomPos {
            destination: [1, 0, 1],
            min_distance: 5,
            max_distance: -1,
            boxed: None,
            prefer_inside: false,
            range: 20,
        };
        let edge = Job::RandomEdge {
            around: [0, 0, 0],
            on_edge: true,
            range: 20,
        };
        assert!(!pos.options().costs().can_drop, "canDrop = false");
        assert!(!edge.options().costs().can_drop);
    }

    #[test]
    fn walking_to_a_place_is_exact_or_one_adjacent() {
        let job = Job::MoveToLocation {
            destination: [5, 0, 5],
            range: 30,
        };
        let s = [0, 0, 0];
        assert!(
            job.is_at_destination(&Floor, s, Reach::alone([5, 0, 5])),
            "exact"
        );
        assert!(
            !job.is_at_destination(&Floor, s, Reach::alone([5, 0, 4])),
            "the goal can be stood in, so the slack stays at 0.1 (:58-65)"
        );
        assert!(
            !job.is_at_destination(&Floor, s, Reach::alone([5, 0, 3])),
            "two blocks away"
        );
        assert!(
            !job.is_at_destination(&Floor, s, Reach::alone([5, -1, 5])),
            "and one below the goal is a neighbour, not the goal"
        );
    }

    #[test]
    fn a_goal_that_wants_ground_refuses_a_node_over_a_hole() {
        struct Hole;
        impl WorldView for Hole {
            fn is_solid(&self, pos: Pos) -> bool {
                pos[1] < 0 && pos[0] != 5
            }
        }
        impl GoalFacts for Hole {}
        let job = Job::MoveToLocation {
            destination: [5, 0, 5],
            range: 10,
        };
        assert!(
            !job.is_at_destination(&Hole, [0, 0, 0], Reach::alone([5, 0, 5])),
            "the destination's own floor is missing (:98-102)"
        );
    }

    #[test]
    fn moving_away_needs_a_euclidean_radius_not_a_manhattan_one() {
        let job = Job::MoveAway {
            avoid: [0, 0, 0],
            avoid_distance: 5,
            preferred: [10, 0, 0],
            range: 20,
        };
        assert!(
            job.is_at_destination(&Floor, [0, 0, 9], Reach::alone([4, 0, 4])),
            "manhattan 8, euclidean 5.66"
        );
        assert!(
            !job.is_at_destination(&Floor, [0, 0, 9], Reach::alone([3, 0, 4])),
            "euclidean 5.0 is not *more* than 5 (:109)"
        );
    }

    #[test]
    fn an_approach_refuses_the_targets_own_column() {
        let job = Job::CloseTo {
            desired: [5, 0, 5],
            nearby: [0, 0, 0],
            distance: 3,
        };
        let s = [0, 0, 0];
        assert!(
            !job.is_at_destination(&Floor, s, Reach::alone([5, 0, 5])),
            "standing in the column is not approaching (:58-61)"
        );
        assert!(
            job.is_at_destination(&Floor, s, Reach::alone([5, 0, 2])),
            "3 away"
        );
        assert!(
            !job.is_at_destination(&Floor, s, Reach::alone([5, 0, 1])),
            "4 away"
        );
        assert_eq!(
            job.end_score(&Floor, s, [5, 0, 5]),
            1_000.0,
            "and it scores 1000 when it does (:71-74)"
        );
    }

    #[test]
    fn a_tree_must_be_beside_the_node_it_came_from() {
        struct Grove;
        impl WorldView for Grove {
            fn is_solid(&self, pos: Pos) -> bool {
                pos[1] < 0
            }
        }
        impl GoalFacts for Grove {
            fn is_tree(&self, pos: Pos) -> bool {
                pos == [4, 0, 5] || pos == [6, 0, 5]
            }
        }
        let job = Job::FindTree {
            towards: None,
            boxed: None,
            range: 20,
        };
        assert!(
            job.is_at_destination(&Grove, [0, 0, 0], Reach::from([5, 0, 5], [4, 0, 5])),
            "a tree east of it"
        );
        assert!(
            !job.is_at_destination(&Grove, [0, 0, 0], Reach::alone([5, 0, 5])),
            "no parent, no tree check (:146-149)"
        );
        assert!(
            !job.is_at_destination(&Grove, [0, 0, 0], Reach::from([9, 0, 9], [9, 0, 8])),
            "nothing beside the node it walked from"
        );
    }

    #[test]
    fn a_pond_is_refused_when_it_was_used_and_when_it_is_not_one() {
        struct Pond(pub [bool; 2]);
        impl WorldView for Pond {
            fn is_solid(&self, pos: Pos) -> bool {
                pos[1] < 0
            }
        }
        impl GoalFacts for Pond {
            fn is_pond(&self, _water: Pos) -> bool {
                self.0[0]
            }
            fn has_fishing_shore(&self, _pond: Pos) -> bool {
                self.0[1]
            }
        }
        let job = Job::FindWater {
            hut: [0, 0, 0],
            ponds: vec![[40, 0, 40]],
            range: 100,
        };
        let wet = Reach {
            at: [10, -1, 5],
            from: None,
            grandparent: None,
            swimming: true,
        };
        assert!(
            !job.is_at_destination(&Pond([false, true]), [0, 0, 0], wet),
            "not a pond"
        );
        assert!(
            !job.is_at_destination(&Pond([true, false]), [0, 0, 0], wet),
            "no shore within 10"
        );
        assert!(
            job.is_at_destination(&Pond([true, true]), [0, 0, 0], wet),
            "a pond"
        );

        let near = Reach {
            at: [10, -1, 12],
            ..wet
        };
        let fished = Job::FindWater {
            hut: [0, 0, 0],
            ponds: vec![[10, 0, 10]],
            range: 100,
        };
        assert!(
            !fished.is_at_destination(&Pond([true, true]), [0, 0, 0], near),
            "6 from the pond already fished, under POND_SEPARATION (:187)"
        );
        assert!(
            !job.is_at_destination(&Pond([true, true]), [0, 0, 0], Reach::alone([10, -1, 5])),
            "a dry node is not a pond (:183)"
        );
        assert!(
            !job.is_at_destination(
                &Pond([true, true]),
                [0, 0, 0],
                Reach {
                    at: [201, -1, 5],
                    ..wet
                }
            ),
            "past MAX_RANGE = 100 (:175)"
        );
    }

    #[test]
    fn the_raiders_want_a_spawn_far_from_where_they_came_in() {
        struct Raid(pub bool);
        impl WorldView for Raid {
            fn is_solid(&self, pos: Pos) -> bool {
                pos[1] < 0
            }
        }
        impl GoalFacts for Raid {
            fn is_valid_spawn(&self, _pos: Pos) -> bool {
                self.0
            }
        }
        let job = Job::Raider {
            from: [0, 0, 0],
            spawn: [100, 0, 0],
        };
        let s = [0, 0, 0];
        assert!(
            !job.is_at_destination(&Raid(true), s, Reach::alone([10, 0, 0])),
            "under 50 from the entry (:70)"
        );
        assert!(
            job.is_at_destination(&Raid(true), s, Reach::alone([60, 0, 0])),
            "50 from the entry, 40 from the spawn"
        );
        assert!(
            !job.is_at_destination(&Raid(false), s, Reach::alone([60, 0, 0])),
            "not a valid spawn (:75)"
        );
        assert!(
            !job.is_at_destination(&Raid(true), s, Reach::alone([160, 0, 0])),
            "past the spawn by more than 50"
        );
    }

    #[test]
    fn a_sign_is_reached_at_its_own_height() {
        let job = Job::SignConnection {
            destination: [5, 2, 5],
            range: 10,
        };
        let s = [0, 0, 0];
        assert!(
            job.is_at_destination(&Floor, s, Reach::alone([5, 2, 3])),
            "2 blocks away horizontally, 4 squared (:25)"
        );
        assert!(
            !job.is_at_destination(&Floor, s, Reach::alone([5, 2, 2])),
            "3 blocks away, 9 squared"
        );
        assert!(
            job.is_at_destination(&Floor, s, Reach::alone([5, 5, 5])),
            "the height is the destination's, not the node's"
        );
    }

    #[test]
    fn a_goal_measured_from_the_start_is_measured_from_the_start() {
        let job = Job::Towards {
            target: [100, 0, 0],
            min_distance: 10,
        };
        let s = [0, 0, 0];
        assert!(
            !job.is_at_destination(&Floor, s, Reach::alone([10, 0, 0])),
            "exactly the distance is not *more* than it (:222)"
        );
        assert!(
            job.is_at_destination(&Floor, s, Reach::alone([11, 0, 0])),
            "over"
        );
        assert!(
            !job.is_at_destination(&Floor, s, Reach::alone([6, 0, 4])),
            "exactly 10"
        );
        assert!(
            job.is_at_destination(&Floor, s, Reach::alone([6, 0, 5])),
            "11 from the start, whatever the target"
        );
    }

    #[test]
    fn the_charges_of_the_four_jobs_that_have_one() {
        let away = Job::MoveAway {
            avoid: [0, 0, 0],
            avoid_distance: 5,
            preferred: [10, 0, 0],
            range: 20,
        };
        assert_eq!(
            away.modify_cost(&Floor, 3.0, Reach::alone([1, 0, 1]), Step::ORDINARY),
            103.0,
            "within 3"
        );
        assert_eq!(
            away.modify_cost(&Floor, 3.0, Reach::alone([5, 0, 5]), Step::ORDINARY),
            3.0,
            "past it"
        );

        let water = Job::FindWater {
            hut: [0, 0, 0],
            ponds: Vec::new(),
            range: 100,
        };
        assert_eq!(
            water.modify_cost(&Floor, 2.0, Reach::alone([10, 0, 10]), Step::ORDINARY),
            2.0,
            "inside the 100"
        );
        assert_eq!(
            water.modify_cost(&Floor, 2.0, Reach::alone([90, 0, 90]), Step::ORDINARY),
            20.0,
            "×10 outside (:226-229)"
        );

        // A road that leaves the axis its parent kept.
        let path = Job::Pathway {
            end: [0, 0, 0],
            colony: 1,
        };
        assert_eq!(
            path.modify_cost(
                &Floor,
                1.0,
                Reach::through([3, 0, 0], [2, 0, 0], [1, 0, 0]),
                Step::ORDINARY
            ),
            1.0,
            "straight on: the parent kept the x axis, and so does this step"
        );
        assert_eq!(
            path.modify_cost(
                &Floor,
                1.0,
                Reach::through([3, 0, 1], [3, 0, 0], [2, 0, 0]),
                Step::ORDINARY
            ),
            10.0,
            "the parent kept z and this step leaves it (:116-124)"
        );

        let approach = Job::CloseTo {
            desired: [10, 0, 0],
            nearby: [0, 0, 0],
            distance: 3,
        };
        assert_eq!(
            approach.ceiling([0, 0, 0]).base,
            708,
            "the corridor to the goal, not to the start (:41)"
        );

        let raider = Job::Raider {
            from: [0, 0, 0],
            spawn: [10, 0, 0],
        };
        assert_eq!(
            raider.modify_cost(&Floor, 1.0, Reach::alone([1, 0, 1]), Step::ORDINARY),
            1.0,
            "ordinary ground"
        );
        assert_eq!(
            raider.modify_cost(
                &Floor,
                1.0,
                Reach::alone([1, 0, 1]),
                Step {
                    on_ground: true,
                    passable: false
                }
            ),
            30.0,
            "THROUGH_BLOCK_COST (:31, :164)"
        );

        let passable = Job::MoveToWithPassable {
            destination: [0, 0, 0],
            range: 10,
        };
        assert_eq!(passable.passing_multiplier(true, false), 3.0, ":143");
        assert_eq!(
            passable.passing_multiplier(false, true),
            3.0,
            "the block above"
        );
        assert_eq!(passable.passing_multiplier(false, false), 1.0);
    }

    #[test]
    fn a_pathway_is_cheaper_to_walk_inside_the_colony() {
        struct Town(pub bool, pub u32);
        impl WorldView for Town {
            fn is_solid(&self, pos: Pos) -> bool {
                pos[1] < 0
            }
        }
        impl GoalFacts for Town {
            fn in_colony(&self, _pos: Pos) -> bool {
                self.0
            }
            fn claiming_buildings(&self, _pos: Pos) -> u32 {
                self.1
            }
        }
        let job = Job::Pathway {
            end: [10, 0, 0],
            colony: 1,
        };
        assert_eq!(job.heuristic(&Town(true, 0), [0, 0, 0], [0, 0, 0]), 10.0);
        assert_eq!(
            job.heuristic(&Town(true, 4), [0, 0, 0], [0, 0, 0]),
            2.0,
            "divided by buildings + 1 (:60)"
        );
        assert_eq!(
            job.heuristic(&Town(false, 4), [0, 0, 0], [0, 0, 0]),
            10.0,
            "outside the colony, no discount"
        );
    }

    #[test]
    fn a_random_wander_stays_in_its_box_and_out_of_the_water() {
        struct Wet(pub bool);
        impl WorldView for Wet {
            fn is_solid(&self, pos: Pos) -> bool {
                pos[1] < 0
            }
            fn is_water(&self, pos: Pos) -> bool {
                self.0 && pos[0] == 8
            }
        }
        impl GoalFacts for Wet {}
        let job = Job::RandomPos {
            destination: [9, 0, 9],
            min_distance: 5,
            max_distance: -1,
            boxed: Some(Box3::between([0, 0, 0], [9, 2, 9])),
            prefer_inside: false,
            range: 20,
        };
        let s = [1, 0, 1];
        assert!(
            job.is_at_destination(&Wet(false), s, Reach::alone([8, 0, 8])),
            "8 from the start, in the box"
        );
        assert!(
            !job.is_at_destination(&Wet(false), s, Reach::alone([3, 0, 3])),
            "under minDistance from the start (:203)"
        );
        assert!(
            !job.is_at_destination(&Wet(true), s, Reach::alone([8, 0, 8])),
            "wet feet (:209)"
        );
        let outside = Job::RandomPos {
            boxed: Some(Box3::between([0, 0, 0], [5, 2, 5])),
            destination: [9, 0, 9],
            min_distance: 5,
            max_distance: -1,
            prefer_inside: false,
            range: 20,
        };
        assert!(
            !outside.is_at_destination(&Wet(false), s, Reach::alone([8, 0, 8])),
            "out of the box (:201)"
        );
    }

    /// A floor at `y = -1` and a terrace at `y = -4`: enough to test a node
    /// three blocks below the post and still standing on something.
    struct TwoFloors;
    impl WorldView for TwoFloors {
        fn is_solid(&self, pos: Pos) -> bool {
            pos[1] < 0 || pos[1] == -4
        }
    }
    impl GoalFacts for TwoFloors {}

    #[test]
    fn an_edge_wander_stays_within_three_blocks_of_the_post() {
        let job = Job::RandomEdge {
            around: [0, 0, 0],
            on_edge: true,
            range: 20,
        };
        let s = [0, 0, 0];
        assert!(
            job.is_at_destination(&TwoFloors, s, Reach::alone([5, 0, 5])),
            "level"
        );
        assert!(
            job.is_at_destination(&TwoFloors, s, Reach::alone([5, -3, 5])),
            "3 down"
        );
        assert!(
            !job.is_at_destination(&TwoFloors, s, Reach::alone([5, 4, 5])),
            "4 up (:257)"
        );
        let no_edge = Job::RandomEdge {
            around: [0, 0, 0],
            on_edge: false,
            range: 20,
        };
        assert!(
            !no_edge.is_at_destination(&TwoFloors, s, Reach::alone([5, 0, 5])),
            "the one-in-ten draw failed (:262)"
        );
    }

    #[test]
    fn a_lookout_wants_sight_and_no_more_than_two_blocks_below() {
        struct Platform(pub bool);
        impl WorldView for Platform {
            fn is_solid(&self, pos: Pos) -> bool {
                pos[1] < 0 || pos[1] == -4
            }
        }
        impl GoalFacts for Platform {
            fn can_see(&self, _from: Pos, _to: Pos) -> bool {
                self.0
            }
        }
        let job = Job::CanSee {
            around: [0, 2, 0],
            target: [9, 0, 9],
            range: 20,
        };
        let s = [0, 0, 0];
        assert!(
            job.is_at_destination(&Platform(true), s, Reach::alone([1, 0, 1])),
            "2 below the post"
        );
        assert!(
            !job.is_at_destination(&Platform(true), s, Reach::alone([1, -3, 1])),
            "5 below, which is over the two (:232)"
        );
        assert!(
            !job.is_at_destination(&Platform(false), s, Reach::alone([1, 0, 1])),
            "out of sight (:237)"
        );
    }

    #[test]
    fn an_escape_wants_air_above_and_below_the_body() {
        struct Cave;
        impl WorldView for Cave {
            fn is_solid(&self, pos: Pos) -> bool {
                pos[1] < 0 || pos == [9, 1, 9]
            }
        }
        impl GoalFacts for Cave {}
        let job = Job::EscapeWater {
            preferred: [0, 0, 0],
        };
        let s = [0, 0, 0];
        assert!(
            job.is_at_destination(&Cave, s, Reach::alone([1, 0, 1])),
            "open air"
        );
        assert!(
            !job.is_at_destination(&Cave, s, Reach::alone([9, 0, 9])),
            "a block at head height (:79)"
        );
    }

    #[test]
    fn the_ceilings_are_the_two_java_constructors() {
        let to = Job::MoveToLocation {
            destination: [10, 0, 10],
            range: 30,
        };
        let c = to.ceiling([0, 0, 0]);
        // direct = 10 + 1 + 10 = 21, radius = 3 + 21/20 = 4, volume = 336
        // estimate = 300 + 336 + 672 = 1308, then + range
        assert_eq!(c.base, 1_308 + 30, "corridor estimate plus the range");

        let away = Job::MoveAway {
            avoid: [0, 0, 0],
            avoid_distance: 5,
            preferred: [1, 0, 0],
            range: 20,
        };
        assert_eq!(away.ceiling([0, 0, 0]).base, 400, "range² (:193)");

        let approach = Job::CloseTo {
            desired: [10, 0, 0],
            nearby: [0, 0, 0],
            distance: 3,
        };
        assert_eq!(
            approach.ceiling([0, 0, 0]).base,
            708,
            "the corridor to the goal, not to the start (:41)"
        );

        let raider = Job::Raider {
            from: [0, 0, 0],
            spawn: [10, 0, 0],
        };
        assert_eq!(raider.ceiling([0, 0, 0]).base, 20_000, ":56");
        assert_eq!(to.extra_nodes(), 4, "PathJobMoveToLocation.java:46");
        assert_eq!(raider.extra_nodes(), 0);
    }
}
