//! Is the path *followable*? Not "is it the cheapest": can a body walk it.
//!
//! Java has no such check, and that is why it is here. There, the path comes
//! out of `finalizePath` (`AbstractPathJob.java:1176-1273`) as a `Path` of
//! `PathPointExtended`, and the navigator trusts it completely: one bad node and
//! the entity walks into a wall, jitters, and the stuck handler takes over
//! (`navigation/PathingStuckHandler.java`). A mod that hands a finished path to
//! the engine has nobody to catch that, so the port checks its own answer.
//!
//! **What "followable" means, rule by rule.** Each one is a rule of the Java
//! search, read backwards — a path is good when every node satisfies what the
//! search required before it created that node:
//!
//! | Rule | Java |
//! | --- | --- |
//! | the body fits in the cell, head included | `isPassable(..., head)`, `AbstractPathJob.java:1493-1504` |
//! | something carries it | `handleNotStanding` / `checkDrop`, `:1568-1640` |
//! | one block of horizontal travel per step, never diagonal | `visitNode`, `:701-719` (four directions, no diagonal) |
//! | at most one block up | `MAX_JUMP_HEIGHT`, `:1558`, `PathingConstants.java:44` |
//! | at most ten blocks down, corridor empty | `checkDrop`, `:1609-1643` |
//!
//! **What it does not check.** That the path is the cheapest (a search question,
//! not a followability one), that it starts where the entity really is, and that
//! the goal is reached — a [`super::search::Refusal`] already says when it is
//! not, and a goal that cannot be stood in is deliberately approached from a
//! neighbour (`PathJobMoveToLocation.java:56-65`).

use super::node::{Costs, MAX_JUMP_STEP, Node, Pos};
use super::search::Path;
use super::world::{WorldView, empty, free, resting};

/// Why a path cannot be followed.
///
/// One variant per rule of the table above, each naming the node it failed at,
/// so a caller can log the exact block instead of "the path is bad".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own closed set of rules
pub enum PathDefect {
    /// There is no node at all.
    Empty,
    /// The body does not fit in this node's cell: a wall, a fence, or a
    /// head-height block (`AbstractPathJob.java:1493-1504`).
    Solid {
        /// The index of the node in the path.
        at: usize,
        /// Where it is.
        pos: Pos,
    },
    /// Nothing carries the body here: air under the feet, or water without the
    /// right to swim (`AbstractPathJob.java:1582-1597`).
    NoRest {
        /// The index of the node in the path.
        at: usize,
        /// Where it is.
        pos: Pos,
    },
    /// Two consecutive nodes are not adjacent: the body would have to walk
    /// through a block, or skip one (`visitNode`, `:701-719`).
    NotAdjacent {
        /// The index of the node that could not be reached.
        at: usize,
        /// The node before it.
        from: Pos,
        /// The node itself.
        to: Pos,
    },
    /// A step that changes both x and z: the diagonal Java never takes
    /// (`:701-719`).
    Diagonal {
        /// The index of the node that could not be reached.
        at: usize,
        /// The node before it.
        from: Pos,
        /// The node itself.
        to: Pos,
    },
    /// A rise of more than one block: over `MAX_JUMP_HEIGHT`
    /// (`:1558`).
    JumpTooHigh {
        /// The index of the node that could not be reached.
        at: usize,
        /// The node before it.
        from: Pos,
        /// The node itself.
        to: Pos,
    },
    /// A fall deeper than ten blocks, or than two when the entity may not drop
    /// (`:1624`).
    FallTooDeep {
        /// The index of the node that could not be reached.
        at: usize,
        /// The node before it.
        from: Pos,
        /// The node itself.
        to: Pos,
    },
    /// A fall whose corridor is not empty: the body would land on something on
    /// the way down (`:1628-1639`).
    FallBlocked {
        /// The index of the node that could not be reached.
        at: usize,
        /// The block in the way.
        pos: Pos,
    },
}

impl PathDefect {
    /// The name this defect is written under in a log or a report.
    pub const fn name(self) -> &'static str {
        match self {
            PathDefect::Empty => "empty",
            PathDefect::Solid { .. } => "solid node",
            PathDefect::NoRest { .. } => "no footing",
            PathDefect::NotAdjacent { .. } => "not adjacent",
            PathDefect::Diagonal { .. } => "diagonal",
            PathDefect::JumpTooHigh { .. } => "jump too high",
            PathDefect::FallTooDeep { .. } => "fall too deep",
            PathDefect::FallBlocked { .. } => "fall blocked",
        }
    }
}

impl std::fmt::Display for PathDefect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            PathDefect::Empty => f.write_str("empty: the path has no node"),
            PathDefect::Solid { at, pos } => write!(
                f,
                "solid node at index {at}: [{}, {}, {}] does not fit the body",
                pos[0], pos[1], pos[2]
            ),
            PathDefect::NoRest { at, pos } => write!(
                f,
                "no footing at index {at}: nothing carries the body at [{}, {}, {}]",
                pos[0], pos[1], pos[2]
            ),
            PathDefect::NotAdjacent { at, from, to } => write!(
                f,
                "not adjacent at index {at}: [{}, {}, {}] then [{}, {}, {}]",
                from[0], from[1], from[2], to[0], to[1], to[2]
            ),
            PathDefect::Diagonal { at, from, to } => write!(
                f,
                "diagonal at index {at}: [{}, {}, {}] then [{}, {}, {}]",
                from[0], from[1], from[2], to[0], to[1], to[2]
            ),
            PathDefect::JumpTooHigh { at, from, to } => write!(
                f,
                "jump too high at index {at}: {} blocks up, from [{}, {}, {}] to [{}, {}, {}]",
                to[1] - from[1],
                from[0],
                from[1],
                from[2],
                to[0],
                to[1],
                to[2]
            ),
            PathDefect::FallTooDeep { at, from, to } => write!(
                f,
                "fall too deep at index {at}: {} blocks down, from [{}, {}, {}] to [{}, {}, {}]",
                from[1] - to[1],
                from[0],
                from[1],
                from[2],
                to[0],
                to[1],
                to[2]
            ),
            PathDefect::FallBlocked { at, pos } => write!(
                f,
                "fall blocked at index {at}: [{}, {}, {}] is in the way down",
                pos[0], pos[1], pos[2]
            ),
        }
    }
}

impl std::error::Error for PathDefect {}

/// Whether `path` can be walked in a world seen through `view`, at the default
/// prices.
///
/// The prices matter: a path with a [`super::node::NodeKind::Swim`] node is
/// followable by a swimmer and not by a walker, so a caller that planned with
/// [`super::node::Costs::with_can_swim`] validates with the same costs
/// ([`is_walkable_with`]).
pub fn is_walkable_path<W: WorldView + ?Sized>(view: &W, path: &Path) -> Result<(), PathDefect> {
    is_walkable_with(view, &Costs::default(), path)
}

/// Whether `path` can be walked by a body priced and allowed as `costs` says.
pub fn is_walkable_with<W: WorldView + ?Sized>(
    view: &W,
    costs: &Costs,
    path: &Path,
) -> Result<(), PathDefect> {
    let nodes: &[Node] = path.nodes();
    if nodes.is_empty() {
        return Err(PathDefect::Empty);
    }

    // Every step is one the search could have made. These come first because a
    // step that is not a step at all is the crudest thing that can be wrong,
    // and because it names the two nodes the caller has to look at.
    for (index, pair) in nodes.windows(2).enumerate() {
        let from = pair[0].at;
        let to = pair[1].at;
        let at = index + 1;
        let dx = to[0] - from[0];
        let dz = to[2] - from[2];
        let dy = to[1] - from[1];

        if dx != 0 && dz != 0 {
            return Err(PathDefect::Diagonal { at, from, to });
        }
        if dx.abs() + dz.abs() > 1 {
            return Err(PathDefect::NotAdjacent { at, from, to });
        }
        if dy > MAX_JUMP_STEP {
            return Err(PathDefect::JumpTooHigh { at, from, to });
        }
        if dy < -costs.deepest_fall() {
            return Err(PathDefect::FallTooDeep { at, from, to });
        }
        if dy < -1 {
            // The body falls through every cell between the two nodes, head
            // included; anything but air in one of them stops it
            // (`AbstractPathJob.java:1628-1639`).
            for depth in 1..(-dy) {
                let cell = [to[0], from[1] - depth, to[2]];
                if !empty(view, cell) {
                    return Err(PathDefect::FallBlocked { at, pos: cell });
                }
            }
        }
    }

    // Every node is somewhere the body can be, and stands on something.
    for (index, node) in nodes.iter().enumerate() {
        if !free(view, costs, node.at) {
            return Err(PathDefect::Solid {
                at: index,
                pos: node.at,
            });
        }
        if !resting(view, costs, node.at) {
            return Err(PathDefect::NoRest {
                at: index,
                pos: node.at,
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pathfinding::fake::Grid;
    use crate::pathfinding::node::{NodeKind, at};
    use crate::pathfinding::search::plan;

    fn walk(points: &[Pos]) -> Path {
        Path::new(
            points
                .iter()
                .map(|point| Node::new(*point, NodeKind::Walk, 0.0))
                .collect(),
        )
    }

    #[test]
    fn a_plain_walk_is_followable() {
        let grid = Grid::new();
        let path = walk(&[at(1, 0, 1), at(2, 0, 1), at(3, 0, 1)]);
        assert_eq!(is_walkable_path(&grid, &path), Ok(()));
    }

    #[test]
    fn a_path_with_a_node_missing_is_refused() {
        let grid = Grid::new();
        // The same walk with its middle taken out: the body would have to
        // cross two blocks in one step.
        let broken = walk(&[at(1, 0, 1), at(3, 0, 1)]);
        assert_eq!(
            is_walkable_path(&grid, &broken),
            Err(PathDefect::NotAdjacent {
                at: 1,
                from: at(1, 0, 1),
                to: at(3, 0, 1),
            })
        );
    }

    #[test]
    fn a_diagonal_step_is_refused() {
        let grid = Grid::new();
        let broken = walk(&[at(1, 0, 1), at(2, 0, 2)]);
        assert_eq!(
            is_walkable_path(&grid, &broken),
            Err(PathDefect::Diagonal {
                at: 1,
                from: at(1, 0, 1),
                to: at(2, 0, 2),
            })
        );
    }

    #[test]
    fn a_rise_of_two_blocks_is_refused() {
        let grid = Grid::new();
        let broken = walk(&[at(1, 0, 1), at(1, 2, 1)]);
        assert_eq!(
            is_walkable_path(&grid, &broken),
            Err(PathDefect::JumpTooHigh {
                at: 1,
                from: at(1, 0, 1),
                to: at(1, 2, 1),
            })
        );
    }

    #[test]
    fn a_node_inside_a_wall_is_refused() {
        let mut grid = Grid::new();
        grid.set_solid(at(2, 0, 1));
        let broken = walk(&[at(1, 0, 1), at(2, 0, 1)]);
        assert_eq!(
            is_walkable_path(&grid, &broken),
            Err(PathDefect::Solid {
                at: 1,
                pos: at(2, 0, 1),
            })
        );
    }

    #[test]
    fn a_node_with_nothing_under_it_is_refused() {
        let grid = Grid::new();
        // Reached sideways over a hole: the step is legal, the footing is not.
        let hovering = walk(&[at(1, 3, 1)]);
        assert_eq!(
            is_walkable_path(&grid, &hovering),
            Err(PathDefect::NoRest {
                at: 0,
                pos: at(1, 3, 1),
            })
        );

        let mut grid = Grid::new();
        grid.dig(2, 1, 3);
        let broken = walk(&[at(1, 0, 1), at(2, 0, 1)]);
        assert_eq!(
            is_walkable_path(&grid, &broken),
            Err(PathDefect::NoRest {
                at: 1,
                pos: at(2, 0, 1),
            })
        );
    }

    #[test]
    fn a_fall_through_something_is_refused() {
        let mut grid = Grid::new();
        grid.dig_strip(0, 10, 3);
        grid.set_solid(at(1, -1, 1));
        let broken = walk(&[at(1, 0, 1), at(1, -3, 1)]);
        assert_eq!(
            is_walkable_path(&grid, &broken),
            Err(PathDefect::FallBlocked {
                at: 1,
                pos: at(1, -1, 1),
            })
        );
    }

    #[test]
    fn a_fall_deeper_than_ten_is_refused() {
        let mut grid = Grid::new();
        grid.dig_strip(0, 10, 11);
        let broken = walk(&[at(1, 0, 1), at(1, -11, 1)]);
        assert_eq!(
            is_walkable_path(&grid, &broken),
            Err(PathDefect::FallTooDeep {
                at: 1,
                from: at(1, 0, 1),
                to: at(1, -11, 1),
            })
        );
        // And the same path is refused earlier still with drops disabled: two
        // blocks is the ceiling then (`AbstractPathJob.java:1624`).
        let strict = Costs::default().with_can_drop(false);
        assert!(matches!(
            is_walkable_with(&grid, &strict, &broken),
            Err(PathDefect::FallTooDeep { .. })
        ));
    }

    #[test]
    fn an_empty_path_is_refused() {
        let grid = Grid::new();
        assert_eq!(
            is_walkable_path(&grid, &Path::new(Vec::new())),
            Err(PathDefect::Empty)
        );
    }

    #[test]
    fn every_path_the_search_returns_passes_and_a_mutilated_one_does_not() {
        let grid = Grid::new();
        let path = plan(&grid, at(1, 0, 1), at(6, 0, 1)).expect("a straight walk");
        assert_eq!(is_walkable_path(&grid, &path), Ok(()));

        let mut nodes = path.nodes().to_vec();
        nodes.remove(3);
        assert_eq!(
            is_walkable_path(&grid, &Path::new(nodes)),
            Err(PathDefect::NotAdjacent {
                at: 3,
                from: at(3, 0, 1),
                to: at(5, 0, 1),
            })
        );
    }
}
