//! The world, as this module is allowed to see it.
//!
//! **The module never reads the world itself.** Everything it knows about a
//! block comes from [`WorldView`], which the caller provides: four questions,
//! and the derived rules that read them. That is what makes a path testable —
//! a ten-by-ten grid of booleans is a whole world here, and a real one is
//! whatever an eventual `level_block_state` call
//! (`crates/horizon-mod-vocab/src/host/levels.rs:304-331`) answers through it.
//!
//! Java reference: `core/entity/pathfinding/SurfaceType.java` (the surface
//! test, `:42-132`), `PathfindingUtils.java:262-470` (`isLiquid`, `isWater`,
//! `isLadder`, `isDangerous`), and `AbstractPathJob.java:1280-1643` — the four
//! `isPassable` / `getGroundHeight` / `handleNotStanding` / `checkDrop`
//! methods that turn world questions into a yes or no.
//!
//! **The collapse, stated once.** Java asks about a `BlockState` and reads
//! collision *shapes*: a fence is 1.5 blocks tall in the middle and 0.375 at
//! the edge, a snow layer is a fraction of a block. This port asks about a
//! block coordinate and gets a boolean per feature, so every shape question is
//! resolved by whoever implements [`WorldView`] — it is the caller's job to
//! answer "solid" for a fence. [`WorldView::surface`] is a provided method
//! precisely so that a caller who models full `SurfaceType` can override it.

use super::node::{Costs, NodeKind, Pos, Surface, down, up};

/// What this module may ask the world.
///
/// Four questions, all about one block coordinate. Every method but
/// [`Self::is_solid`] has a default, so an ordinary floor of stone needs one
/// method and a ten-by-ten test grid needs three.
pub trait WorldView {
    /// The body cannot be *inside* this cell: `ShapeUtil.isEmpty` is false on it,
    /// i.e. the block has a collision shape (`AbstractPathJob.java:1380-1400`).
    ///
    /// A fence, a wall and a full cube all answer `true` here, and the
    /// difference between them — you can stand on one and not the other — is
    /// the [`Self::surface`] question.
    fn is_solid(&self, pos: Pos) -> bool;

    /// The cell holds water: `PathfindingUtils.isWater`
    /// (`PathfindingUtils.java:286-330`). A body in water swims
    /// (`AbstractPathJob.java:1738-1748`), so this is what decides
    /// [`NodeKind::Swim`].
    fn is_water(&self, _pos: Pos) -> bool {
        false
    }

    /// The cell can be climbed: `PathfindingUtils.isLadder`
    /// (`PathfindingUtils.java:367-445`), which accepts ladders and, when the
    /// options allow it, vines and other climbables.
    fn is_climbable(&self, _pos: Pos) -> bool {
        false
    }

    /// The cell is a door or a gate: the two blocks `SurfaceType` marks
    /// `DROPABLE` for being enterable when the options allow it
    /// (`SurfaceType.java:79-82`).
    ///
    /// **One question for two blocks.** Java separates `DoorBlock` from
    /// `FenceGateBlock` and prices them with two switches
    /// (`PathingOptions.java:96`, `:101`); the port's [`Costs::doors_open`]
    /// accepts either switch.
    fn is_door(&self, _pos: Pos) -> bool {
        false
    }

    /// What this block is to a walker: the port of
    /// `SurfaceType.getSurfaceType` (`SurfaceType.java:42-132`).
    ///
    /// The default derives it from the four questions above, in the order that
    /// method tests:
    ///
    /// 1. water is `Walkable` (`SurfaceType.java:107-110`) — Java answers
    ///    "walkable" for water because a swimmer's surface is the water's;
    /// 2. a door or a gate is `Dropable` (`:79-82`);
    /// 3. a climbable is `Dropable`, as signs and vines are (`:112-115`);
    /// 4. a solid block is `Walkable`, its top carrying a body (`:129-141`);
    /// 5. anything else is `Dropable`, which is what air is (`:132`).
    ///
    /// A caller with fences and lava overrides this method and answers
    /// [`Surface::NotPassable`] for them.
    fn surface(&self, pos: Pos) -> Surface {
        if self.is_water(pos) {
            Surface::Walkable
        } else if self.is_door(pos) || self.is_climbable(pos) {
            Surface::Dropable
        } else if self.is_solid(pos) {
            Surface::Walkable
        } else {
            Surface::Dropable
        }
    }
}

/// A closure is a world: `|pos| solid_cells.contains(&pos)` answers
/// [`WorldView::is_solid`] and nothing else.
///
/// This is the shortest way to plan over a world a test or a caller already
/// holds, and it is exactly the "une fermeture" of the brief.
impl<F: Fn(Pos) -> bool> WorldView for F {
    fn is_solid(&self, pos: Pos) -> bool {
        self(pos)
    }
}

/// A body may be in this cell: nothing solid at the feet or at the head, or a
/// door it may open, or a ladder.
///
/// Java asks this twice per move — `isPassable(x, y, z, head)` for the feet and
/// `isPassable(..., true)` for the head (`AbstractPathJob.java:1493-1504`) —
/// because a body occupies two cells, a citizen being 1.8 blocks tall.
pub fn enterable<W: WorldView + ?Sized>(view: &W, costs: &Costs, pos: Pos) -> bool {
    view.is_climbable(pos) || (view.is_door(pos) && costs.doors_open()) || !view.is_solid(pos)
}

/// The body fits in this cell, head included.
pub fn free<W: WorldView + ?Sized>(view: &W, costs: &Costs, pos: Pos) -> bool {
    enterable(view, costs, pos) && enterable(view, costs, up(pos))
}

/// Nothing at all in this cell: Java's `state.isAir()`
/// (`AbstractPathJob.java:1636`), which is what stops a fall through a fence,
/// a sign or a lily pad.
///
/// **Stricter than a boolean can be.** A tall-grass block is not air in Java
/// (`AbstractPathJob.java:1636` refuses the drop) and is not solid either, so
/// this answers `true` for it; the drop then goes through where Java's stops.
/// A `WorldView` that cares overrides [`WorldView::is_solid`] to say so.
pub fn empty<W: WorldView + ?Sized>(view: &W, pos: Pos) -> bool {
    !view.is_solid(pos) && !view.is_water(pos) && !view.is_door(pos) && !view.is_climbable(pos)
}

/// The body is in water here: `calculateSwimming`
/// (`AbstractPathJob.java:1738-1748`), which looks at the feet cell, the head
/// cell and the block below.
pub fn swimming<W: WorldView + ?Sized>(view: &W, pos: Pos) -> bool {
    view.is_water(down(pos)) || view.is_water(pos) || view.is_water(up(pos))
}

/// Something carries the body here: a standable block under the feet, a
/// climbable to hold on to, or water to swim in when that is allowed.
///
/// Java's `handleNotStanding` (`AbstractPathJob.java:1582-1597`): liquid under
/// the feet means swimming and needs `canSwim` (`:1586-1589`), a ladder under
/// the feet means you may stand (`:1591-1594`), and everything else is a
/// question for `SurfaceType`.
pub fn resting<W: WorldView + ?Sized>(view: &W, costs: &Costs, pos: Pos) -> bool {
    if swimming(view, pos) {
        return costs.can_swim;
    }
    let floor = down(pos);
    view.is_climbable(floor) || view.surface(floor).is_walkable()
}

/// A node the search may stop on: the body fits, and something carries it.
pub fn standable<W: WorldView + ?Sized>(view: &W, costs: &Costs, pos: Pos) -> bool {
    free(view, costs, pos) && resting(view, costs, pos)
}

/// The kind of node a step into `pos` creates.
///
/// Java sets the same flags on the node it creates: `setLadder` from
/// `isLadder(state)` (`AbstractPathJob.java:867`, `:908-911`), `setSwimming`
/// from `calculateSwimming` (`:857`, `:903-906`), and the surface type decides
/// the rest. The order below is the order in which a cell can be two of these:
/// a door is a door whatever is behind it, a ladder is a ladder before it is
/// water, and a fall is only a fall if it is not one of the others.
pub fn kind_of<W: WorldView + ?Sized>(view: &W, costs: &Costs, pos: Pos, dy: i32) -> NodeKind {
    if view.is_door(pos) && costs.doors_open() {
        NodeKind::Door
    } else if view.is_climbable(pos) {
        NodeKind::Climb
    } else if swimming(view, pos) {
        NodeKind::Swim
    } else if dy < 0 {
        NodeKind::Fall
    } else {
        NodeKind::Walk
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pathfinding::fake::Grid;

    #[test]
    fn a_stone_floor_is_walkable_and_the_air_above_is_dropable() {
        let grid = Grid::new();
        assert_eq!(grid.surface([1, -1, 1]), Surface::Walkable, "the floor");
        assert_eq!(grid.surface([1, 1, 1]), Surface::Dropable, "the air");
        assert!(free(&grid, &Costs::default(), [1, 0, 1]));
    }

    #[test]
    fn a_body_takes_two_cells() {
        let mut grid = Grid::new();
        grid.set_solid([2, 1, 2]);
        assert!(!free(&grid, &Costs::default(), [2, 0, 2]), "head blocked");
        assert!(enterable(&grid, &Costs::default(), [2, 0, 2]), "feet free");
    }

    #[test]
    fn water_is_a_surface_you_swim_on() {
        let mut grid = Grid::new();
        grid.set_water([2, 0, 2]);
        assert_eq!(grid.surface([2, 0, 2]), Surface::Walkable);
        assert!(swimming(&grid, [2, 0, 2]));
        assert!(!resting(&grid, &Costs::default(), [2, 0, 2]), "no swimming");
        assert!(resting(
            &grid,
            &Costs::default().with_can_swim(true),
            [2, 0, 2]
        ));
    }

    #[test]
    fn a_closure_is_a_world() {
        let costs = Costs::default();
        let blocked = |pos: Pos| pos[1] < 0;
        assert!(free(&blocked, &costs, [0, 0, 0]));
        assert!(blocked.surface([0, -1, 0]).is_walkable(), "the floor");
        assert!(
            !blocked.surface([0, 0, 0]).is_walkable(),
            "the air on top of it"
        );
    }

    #[test]
    fn the_kind_says_what_the_cell_is_before_what_the_step_is() {
        let mut grid = Grid::new();
        grid.set_water([2, 0, 2]);
        grid.set_door([3, 0, 2]);
        grid.set_ladder([4, 0, 2]);
        let costs = Costs::default().with_can_swim(true).with_enter_doors(true);
        assert_eq!(kind_of(&grid, &costs, [2, 0, 2], 0), NodeKind::Swim);
        assert_eq!(kind_of(&grid, &costs, [3, 0, 2], 0), NodeKind::Door);
        assert_eq!(kind_of(&grid, &costs, [4, 0, 2], 0), NodeKind::Climb);
        assert_eq!(kind_of(&grid, &costs, [1, 0, 1], -4), NodeKind::Fall);
        assert_eq!(kind_of(&grid, &costs, [1, 0, 1], 0), NodeKind::Walk);
    }
}
