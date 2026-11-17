//! Where a body really starts: out of the fence, off the carpet, up out of the
//! water.
//!
//! A caller hands the search the block under the entity's feet, and that block
//! is often not a place a body can be: standing on a carpet, inside a door leaf,
//! half in a water block, straddling a fence post. Java corrects it before the
//! search begins — `PathfindingUtils.prepareStart`
//! (`PathfindingUtils.java:87-197`), six rules in a fixed order — and this is
//! that method.
//!
//! **Why it matters for a port.** The first version's `plan` refuses a start it
//! cannot stand in (`Refusal::StartBlocked`, `search.rs`) instead of correcting
//! it, so every caller would have to know the six rules. Porting them once here
//! is cheaper, and it is the reason a citizen on a carpet can path at all.
//!
//! **The order is the difficult part, not the rules.** A carpet is handled
//! before the collision shape (a carpet *is* a collision shape), the bounding
//! box scan before the shape test, and the sink-through loop before the water
//! lift — reverse two of them and a body in a waterlogged door goes down instead
//! of up.

use crate::pathfinding::{Pos, Surface, WorldView, down, up};

/// The questions `prepareStart` asks that [`WorldView`] does not answer.
pub trait StartWorld: WorldView {
    /// A carpet, a floating carpet, a lily pad (`PathfindingUtils.java:98`).
    fn is_carpet(&self, _pos: Pos) -> bool {
        false
    }

    /// A block a body may stand *inside*: a door, a trapdoor, an open panel,
    /// anything with no collision, a carpet (`:205-214`).
    fn standable_inside(&self, _pos: Pos) -> bool {
        false
    }

    /// The top of the block's collision shape, as a fraction of a block
    /// (`ShapeUtil.max(shape, Axis.Y)`, used at `:119`).
    fn floor_top(&self, pos: Pos) -> f32 {
        if self.is_solid(pos) { 1.0 } else { 0.0 }
    }

    /// The block holds a fluid (`bs.getFluidState().isEmpty()`, `:152`, `:166`).
    fn fluid(&self, pos: Pos) -> bool {
        self.is_water(pos)
    }

    /// A fence, a wall, or a colony hut block (`:172`).
    fn fence_like(&self, pos: Pos) -> bool {
        self.is_solid(pos)
    }

    /// The block stops motion (`bs.blocksMotion()`, `:133`, `:172`).
    fn blocks_motion(&self, pos: Pos) -> bool {
        self.is_solid(pos)
    }

    /// The collision shape is empty (`:175`).
    fn shape_empty(&self, pos: Pos) -> bool {
        !self.is_solid(pos)
    }

    /// The collision shape's height (`:133`).
    fn shape_top(&self, pos: Pos) -> f32 {
        if self.is_solid(pos) { 1.0 } else { 0.0 }
    }
}

/// What the caller knows about the body it is placing.
#[derive(Clone, Debug, PartialEq)]
pub struct Start {
    /// The block under the feet, floored (`Mth.floor(entity.getX()), …`, `:89`).
    pub at: Pos,
    /// The columns of the entity's bounding box, at the feet's height: what
    /// Java's `for x in minX..maxX` / `for z in minZ..maxZ` enumerates
    /// (`:106-127`). Each entry is `[x, at[1], z]`.
    pub area: Vec<Pos>,
    /// The entity's exact `y` (`entity.getY()`), for the 0.1 tolerance (`:119`).
    pub feet_y: f32,
    /// The entity's offset from the centre of its cell, x and z
    /// (`entity.position() - shape.bounds().getCenter()`, `:180-184`).
    pub push: (f32, f32),
    /// The world's lowest build height (`getMinBuildHeight()`, `:158`).
    pub min_y: i32,
    /// The world's highest build height, a guard the Java does not have: its
    /// water lift (`:166-170`) never checks and would loop forever in a world of
    /// nothing but water.
    pub max_y: i32,
    /// `entity.onGround()` (`:104`).
    pub on_ground: bool,
    /// `entity.isInWater()` (`:164`).
    pub in_water: bool,
}

impl Start {
    /// The simplest case: a body at `at` with nothing else known — no area, no
    /// water, standing on the ground.
    pub fn at(at: Pos, min_y: i32, max_y: i32) -> Start {
        Start {
            at,
            area: Vec::new(),
            feet_y: at[1] as f32,
            push: (0.0, 0.0),
            min_y,
            max_y,
            on_ground: true,
            in_water: false,
        }
    }
}

/// Why the start could not be corrected.
///
/// Java returns the entity's own block position when it sinks below the world
/// (`:158-161`) and has no guard at all for the water lift; here each is a named
/// refusal, so a caller can log the position instead of guessing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartDefect {
    /// The body sank out of the world: Java stops at `minBuildHeight` and
    /// answers the entity's uncorrected position (`:158-161`).
    BelowWorld {
        /// The last block it stood in before the world ended.
        fell_to: Pos,
    },
    /// The body rose out of the world looking for the surface of the water
    /// (Java's `while (!bs.getFluidState().isEmpty())`, `:166-170`, has no
    /// bound).
    AboveWorld {
        /// The last block it rose through.
        rose_to: Pos,
    },
}

impl std::fmt::Display for StartDefect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            StartDefect::BelowWorld { fell_to } => write!(
                f,
                "below the world at [{}, {}, {}]: the body sank through standable blocks",
                fell_to[0], fell_to[1], fell_to[2]
            ),
            StartDefect::AboveWorld { rose_to } => write!(
                f,
                "above the world at [{}, {}, {}]: the body never found the water's surface",
                rose_to[0], rose_to[1], rose_to[2]
            ),
        }
    }
}

impl std::error::Error for StartDefect {}

/// The cell the body should path from — `prepareStart`, rule by rule
/// (`PathfindingUtils.java:87-197`).
///
/// The rules are tried in Java's order and the first that applies changes the
/// position; they are not exclusive, so a body on a carpet inside a fence is
/// corrected twice, exactly as Java would.
pub fn prepare_start<W: StartWorld + ?Sized>(view: &W, start: &Start) -> Result<Pos, StartDefect> {
    let mut at = start.at;

    // 1. On a carpet or a lily pad: path from above it (`:98-101`). Checked
    //    first because a carpet is also a collision shape, and the shape test
    //    below would lift the body one block too far.
    if view.is_carpet(at) {
        return Ok(up(at));
    }

    // 2. Standing on something you cannot stand on — half off a slab, over a
    //    hole: take the last column of the bounding box that is a real floor at
    //    the feet's height (`:104-128`). Java keeps the *last* match because it
    //    writes `pos` without breaking, and the tolerance of 0.1 is what makes
    //    "the same floor" precise.
    if start.on_ground && view.surface(down(at)) != Surface::Walkable {
        for column in &start.area {
            if column == &at {
                continue;
            }
            let floor = down(*column);
            if view.surface(floor) != Surface::Walkable {
                continue;
            }
            let top = view.floor_top(floor) + floor[1] as f32;
            if (top - start.feet_y).abs() < 0.1 {
                at = *column;
            }
        }
    }

    // 3. Inside a collision shape with height — a fence post under the feet, a
    //    wall at body height: one block up (`:130-149`). Java refines this with
    //    the entity's fractional position inside the shape; a block-resolution
    //    world has no fraction, so a solid cell means "inside" (`MANQUANT.md` §4).
    if view.blocks_motion(at) && !view.standable_inside(at) && view.shape_top(at) > 0.0 {
        at = up(at);
    }

    // 4. Sink through what you may stand in: two standable blocks in a row —
    //    a door under a door — mean the body is in the upper one and belongs in
    //    the lower, until the ground or a ladder or a fluid stops it
    //    (`:151-162`).
    while view.standable_inside(at)
        && view.standable_inside(down(at))
        && !view.is_climbable(down(at))
        && !view.fluid(down(at))
    {
        at = down(at);
        if at[1] < start.min_y {
            return Err(StartDefect::BelowWorld { fell_to: at });
        }
    }

    if start.in_water {
        // 5. In water, path along the surface: rise until the block is dry
        //    (`:164-171`). A drowned pirate keeps its own level and is not
        //    lifted (`!(entity instanceof AbstractDrownedEntityPirateRaider)`);
        //    that is a property of the caller's entity, so it passes
        //    `in_water = false`.
        while view.fluid(at) {
            at = up(at);
            if at[1] > start.max_y {
                return Err(StartDefect::AboveWorld { rose_to: at });
            }
        }
    } else if view.fence_like(start.at) || (view.blocks_motion(at) && !view.standable_inside(at)) {
        // 6. In a fence or a wall: step off it, away from where the body stands
        //    inside the shape (`:172-194`). Java compares the two offsets and
        //    moves along the larger one, which is the shorter way out.
        if view.shape_empty(at) {
            return Ok(at);
        }
        let (dx, dz) = start.push;
        if dx.abs() < dz.abs() {
            at[2] += if dz < 0.0 { -1 } else { 1 };
        } else {
            at[0] += if dx < 0.0 { -1 } else { 1 };
        }
    }

    Ok(at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pathfinding::at;

    /// A flat world: solid under `y = 0`, plus whatever a test adds.
    #[derive(Default)]
    struct Flat {
        carpets: Vec<Pos>,
        doors: Vec<Pos>,
        water: Vec<Pos>,
        fences: Vec<Pos>,
        holes: Vec<(i32, i32)>,
        flat: Vec<Pos>,
    }

    impl WorldView for Flat {
        fn is_solid(&self, pos: Pos) -> bool {
            if self.carpets.contains(&pos) || self.doors.contains(&pos) {
                return false;
            }
            if self.water.contains(&pos) {
                return false;
            }
            if self.fences.contains(&pos) {
                return true;
            }
            if pos[1] == -1 && self.holes.contains(&(pos[0], pos[2])) {
                return false;
            }
            pos[1] < 0
        }
        fn is_water(&self, pos: Pos) -> bool {
            self.water.contains(&pos)
        }
        fn is_climbable(&self, _pos: Pos) -> bool {
            false
        }
    }

    impl StartWorld for Flat {
        fn is_carpet(&self, pos: Pos) -> bool {
            self.carpets.contains(&pos)
        }
        fn standable_inside(&self, pos: Pos) -> bool {
            self.doors.contains(&pos) || self.carpets.contains(&pos)
        }
        fn fluid(&self, pos: Pos) -> bool {
            self.water.contains(&pos)
        }
        fn fence_like(&self, pos: Pos) -> bool {
            self.fences.contains(&pos)
        }
        fn shape_top(&self, pos: Pos) -> f32 {
            if self.flat.contains(&pos) { 0.0 } else { 1.0 }
        }
    }

    #[test]
    fn a_body_on_a_carpet_starts_above_it() {
        let mut world = Flat::default();
        world.carpets.push(at(2, 0, 2));
        let start = Start::at(at(2, 0, 2), -64, 320);
        assert_eq!(
            prepare_start(&world, &start),
            Ok(at(2, 1, 2)),
            "PathfindingUtils.java:98-101"
        );
    }

    #[test]
    fn a_body_half_off_a_hole_takes_the_column_with_a_floor() {
        let mut world = Flat::default();
        world.holes.push((2, 2));
        let start = Start {
            at: at(2, 0, 2),
            area: vec![at(2, 0, 2), at(3, 0, 2), at(1, 0, 2)],
            ..Start::at(at(2, 0, 2), -64, 320)
        };
        let fixed = prepare_start(&world, &start).expect("a floor nearby");
        assert_ne!(fixed, at(2, 0, 2), "the hole is not a start (:104-128)");
        assert!(world.is_solid(down(fixed)), "the chosen column has a floor");
        assert_eq!(
            fixed,
            at(1, 0, 2),
            "Java keeps the last match, and 1 is last"
        );
    }

    #[test]
    fn a_body_whose_floor_is_too_different_in_height_is_not_moved_there() {
        let mut world = Flat::default();
        world.holes.push((2, 2));
        let start = Start {
            at: at(2, 0, 2),
            area: vec![at(2, 0, 2), at(3, 0, 2)],
            feet_y: 3.5, // the body is nowhere near the floor: no candidate
            ..Start::at(at(2, 0, 2), -64, 320)
        };
        assert_eq!(
            prepare_start(&world, &start),
            Ok(at(2, 0, 2)),
            "the 0.1 tolerance rejects every column (:119)"
        );
    }

    #[test]
    fn a_body_inside_a_wall_is_lifted_one_block() {
        let mut world = Flat::default();
        let wall = at(4, 0, 4);
        world.fences.push(wall);
        assert_eq!(
            prepare_start(&world, &Start::at(wall, -64, 320)),
            Ok(at(4, 1, 4)),
            "one up, then the push rule finds air above and stops (:130-149)"
        );
    }

    #[test]
    fn a_body_in_a_shapeless_fence_cell_is_pushed_out_on_the_shorter_side() {
        let mut world = Flat::default();
        // A fence whose shape has no height: the lift rule does not fire, so
        // the push rule decides (:172-194).
        world.fences.push(at(7, 0, 7));
        world.flat.push(at(7, 0, 7));
        let east = Start {
            push: (0.4, 0.1),
            ..Start::at(at(7, 0, 7), -64, 320)
        };
        assert_eq!(prepare_start(&world, &east), Ok(at(8, 0, 7)));
        let north = Start {
            push: (0.1, -0.4),
            ..Start::at(at(7, 0, 7), -64, 320)
        };
        assert_eq!(prepare_start(&world, &north), Ok(at(7, 0, 6)));
    }

    #[test]
    fn a_body_in_a_door_sinks_to_the_door_below() {
        let mut world = Flat::default();
        // A door two blocks tall: the body is in the upper one, head included.
        world.doors.push(at(5, 0, 5));
        world.doors.push(at(5, 1, 5));
        assert_eq!(
            prepare_start(&world, &Start::at(at(5, 1, 5), -64, 320)),
            Ok(at(5, 0, 5)),
            "standable in standable, no ladder, no fluid (:151-162)"
        );
    }

    #[test]
    fn a_body_in_water_rises_to_the_surface() {
        let mut world = Flat::default();
        for y in 0..4 {
            world.water.push(at(6, y, 6));
        }
        let start = Start {
            in_water: true,
            ..Start::at(at(6, 0, 6), -64, 320)
        };
        assert_eq!(
            prepare_start(&world, &start),
            Ok(at(6, 4, 6)),
            "the first dry block above the water (:164-171)"
        );
    }

    #[test]
    fn sinking_out_of_the_world_is_a_named_refusal() {
        let mut world = Flat::default();
        for y in -2..=0 {
            world.doors.push(at(8, y, 8));
        }
        let start = Start::at(at(8, 0, 8), -1, 320);
        assert_eq!(
            prepare_start(&world, &start),
            Err(StartDefect::BelowWorld {
                fell_to: at(8, -2, 8)
            }),
            "Java returns the uncorrected position here (:158-161)"
        );
    }

    #[test]
    fn rising_out_of_the_world_is_a_named_refusal() {
        let mut world = Flat::default();
        for y in 0..=3 {
            world.water.push(at(9, y, 9));
        }
        let start = Start {
            in_water: true,
            ..Start::at(at(9, 0, 9), -64, 2)
        };
        assert_eq!(
            prepare_start(&world, &start),
            Err(StartDefect::AboveWorld {
                rose_to: at(9, 3, 9)
            }),
            "the Java loop has no bound (:166-170)"
        );
    }

    #[test]
    fn an_ordinary_body_is_left_where_it_stands() {
        let world = Flat::default();
        assert_eq!(
            prepare_start(&world, &Start::at(at(1, 0, 1), -64, 320)),
            Ok(at(1, 0, 1)),
            "a floor under the feet and air around: nothing to correct"
        );
    }
}
