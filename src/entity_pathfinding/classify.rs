//! What a block *is*: surface, water, lava, ladder, danger, passability, line
//! of sight.
//!
//! Java asks these of a `BlockState` and reads collision *shapes*
//! (`ShapeUtil.max`, `ShapeUtil.isEmpty`). A mod that plans has only the
//! block's own answers, so each function here takes a small fact record the
//! caller fills from whatever it does have — `level_block_state`
//! (`crates/horizon-mod-vocab/src/host/levels.rs:304-331`) plus the block's
//! properties. Two things come out of that: the rules are checked with no game
//! and no wasm, and the *order* of the cascade is Java's, which a caller
//! assembling its own `if` chain would lose.
//!
//! **This is where the first version is thin.** `crate::pathfinding`'s default
//! `WorldView::surface` has five branches; `SurfaceType.getSurfaceType` has
//! eleven (`SurfaceType.java:56-132`), and three of the six it is missing are
//! refusals — lava, a fence, a dangerous block — that a caller's five-branch
//! answer would call walkable. [`surface_of`] is the full cascade, returning
//! the same [`Surface`] enum the search already understands, so a caller who
//! overrides `WorldView::surface` with it is done.

use crate::pathfinding::{Pos, Surface};

/// The shape height at or under which a block is "flat enough to walk over":
/// `ShapeUtil.max(shape, Axis.Y) <= 0.1` (`AbstractPathJob.java:1394`, `:1467`,
/// `:1497`).
pub const FLAT: f32 = 0.1;

/// The height under which a block is a step, not a wall
/// (`ShapeUtil.max(shape, Axis.Y) < 0.5`, `SurfaceType.java:86`).
pub const LOW: f32 = 0.5;

/// The height above which a block is a wall you cannot stand on
/// (`> 1.0`, `SurfaceType.java:101`).
pub const TALL: f32 = 1.0;

/// The width at which a shape covers a block: `>= 0.85` on both horizontal axes
/// (`SurfaceType.java:122-123`).
pub const FULL_WIDTH: f32 = 0.85;

/// What a block is, for `SurfaceType.getSurfaceType` — one field per test the
/// method makes, in its order.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BlockFacts {
    /// In `ModTags.dangerousBlocks`, or fire, a campfire, magma, a berry bush,
    /// powder snow, a lava cauldron (`PathfindingUtils.java:447-458`).
    pub dangerous: bool,
    /// A fence, a wall, a colony hut block, bamboo (`SurfaceType.java:70-77`).
    pub fence_or_wall: bool,
    /// A door or a fence gate (`:79-82`).
    pub door_or_gate: bool,
    /// A panel or a trapdoor that is closed (`:96-99`) — walkable, it is a
    /// surface.
    pub closed_panel: bool,
    /// The collision shape's height, `ShapeUtil.max(shape, Axis.Y)`.
    pub shape_top: f32,
    /// The collision shape's width on x.
    pub width_x: f32,
    /// The collision shape's width on z.
    pub width_z: f32,
    /// The collision shape is empty — air, a sign, a torch.
    pub shape_empty: bool,
    /// Pathable water (`PathfindingUtils.isWater`, `:286-322`).
    pub water: bool,
    /// Lava (`PathfindingUtils.isLava`, `:332-358`).
    pub lava: bool,
    /// A construction tape, a sign, a vine (`SurfaceType.java:117-120`).
    pub soft_climbable: bool,
    /// Snow layers, 1 being the thinnest (`:124`).
    pub snow_layers: u8,
    /// A carpet, a floating carpet, a lily pad (`:125-127`).
    pub carpet_like: bool,
    /// The options allow walking over danger (`PathingOptions.canPassDanger`).
    pub pass_danger: bool,
    /// The block under this one is dangerous (`SurfaceType.java:86`).
    pub below_dangerous: bool,
}

/// `SurfaceType.getSurfaceType`, branch for branch (`SurfaceType.java:56-132`).
///
/// The order is the spec: a dangerous block is refused before its shape is
/// looked at, a fence is refused before anything, and a door is `DROPABLE`
/// before the water test — which is why a waterlogged door is a door and not a
/// swim.
pub fn surface_of(facts: &BlockFacts) -> Surface {
    // Dangerous first, and only a caller who asked for danger gets through
    // (:56-68).
    if facts.dangerous {
        if facts.pass_danger {
            return if facts.shape_empty {
                Surface::Dropable
            } else {
                Surface::Walkable
            };
        }
        return Surface::NotPassable;
    }
    // A fence, a wall, a hut block, bamboo: never a surface (:70-77).
    if facts.fence_or_wall {
        return Surface::NotPassable;
    }
    // A door leaf is something you fall through, not something you stand on
    // (:79-82).
    if facts.door_or_gate {
        return Surface::Dropable;
    }
    // A flat block over danger: refused unless danger is allowed (:86-94).
    if facts.shape_top < LOW && facts.below_dangerous {
        return if facts.pass_danger {
            Surface::Walkable
        } else {
            Surface::NotPassable
        };
    }
    // A closed trapdoor or panel is a floor (:96-99).
    if facts.closed_panel {
        return Surface::Walkable;
    }
    // Taller than a block: a wall (:101-104).
    if facts.shape_top > TALL {
        return Surface::NotPassable;
    }
    // A swimmer's surface is the water's (:106-110).
    if facts.water {
        return Surface::Walkable;
    }
    // And a lava lake has none (:112-115).
    if facts.lava {
        return Surface::NotPassable;
    }
    // A sign, a construction tape, a vine: passable, not standable
    // (:117-120).
    if facts.soft_climbable {
        return Surface::Dropable;
    }
    // A full-width collision, a deep snow layer, a carpet, a lily pad
    // (:122-130).
    if (facts.width_x >= FULL_WIDTH && facts.width_z >= FULL_WIDTH)
        || facts.snow_layers > 1
        || facts.carpet_like
    {
        return Surface::Walkable;
    }
    // Everything else is air or something you fall through (:132).
    Surface::Dropable
}

/// Which fluid fills a cell — the three answers `FluidState` gives a walker.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fluid {
    /// `Fluids.EMPTY` (`PathfindingUtils.java:46`).
    #[default]
    Empty,
    /// Water or flowing water (`:320-321`).
    Water,
    /// Lava or flowing lava (`:356-357`).
    Lava,
    /// Something else — modded, or a fluid a walker treats as nothing.
    Other,
}

/// The tests `PathfindingUtils.isWater` makes, in its order
/// (`PathfindingUtils.java:286-322`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WaterFacts {
    /// `state.isSolid()` (`:294`).
    pub solid: bool,
    /// The block *is* `Blocks.WATER` (`:298`).
    pub water_block: bool,
    /// The fluid state's type (`:320`).
    pub fluid: Fluid,
    /// A waterlogged closed trapdoor whose half is top — the one waterlogging
    /// Java refuses (`:314-318`).
    pub waterlogged_top_trapdoor: bool,
}

/// `isWater`: solid blocks never hold swimmable water, a plain water block
/// always does, and a waterlogged block does unless it is a closed top trapdoor.
pub fn is_water(facts: &WaterFacts) -> bool {
    if facts.solid {
        return false;
    }
    if facts.water_block {
        return true;
    }
    if facts.fluid == Fluid::Empty || facts.waterlogged_top_trapdoor {
        return false;
    }
    facts.fluid == Fluid::Water
}

/// `isLava` (`PathfindingUtils.java:332-358`): a lava block always, a lava fluid
/// otherwise — and a solid block can still be lava, unlike water.
pub fn is_lava(block: bool, fluid: Fluid) -> bool {
    block || fluid == Fluid::Lava
}

/// The tests `PathfindingUtils.isLadder` makes
/// (`PathfindingUtils.java:367-376`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClimbFacts {
    /// A `LadderBlock` (`:374`).
    pub ladder: bool,
    /// In `ModTags.freeClimbBlocks` (`:375`).
    pub free_climb: bool,
    /// In `BlockTags.CLIMBABLE` — a vine, a scaffolding, a twisting vine
    /// (`:373`).
    pub climbable_tag: bool,
    /// The block is a liquid (`:369`).
    pub liquid: bool,
    /// `canClimbAdvanced` (`:373`).
    pub advanced: bool,
    /// `walkUnderWater` (`:369`).
    pub under_water: bool,
}

/// `isLadder`: a real ladder and a free-climb block always, a tagged climbable
/// only when the options allow advanced climbing, and any liquid when the body
/// walks under water.
///
/// **This is what the first version overcharges.** `Costs::climb` is applied on
/// every climb entry, because the first version's world view answers one
/// `is_climbable` and cannot tell a ladder from a vine. Java charges the
/// non-ladder price only when the block is *not* a `LadderBlock`
/// (`AbstractPathJob.java:1099-1101`): a ladder is free to start, a vine is not.
pub fn is_ladder(facts: &ClimbFacts) -> bool {
    (facts.under_water && facts.liquid)
        || facts.ladder
        || facts.free_climb
        || (facts.climbable_tag && facts.advanced)
}

/// Whether the body may enter this kind of block, from the table of
/// `isPassable(BlockState)` (`AbstractPathJob.java:1445-1455`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Barrier {
    /// A door: `canEnterDoors` (`:1445`).
    Door,
    /// A fence gate: `canEnterGates` (`:1446`).
    Gate,
    /// A colony gate block, `AbstractBlockGate`: `canEnterGates` (`:1447`).
    ColonyGate,
    /// A construction tape, a pressure plate, a decoration controller, a sign,
    /// a banner: always passable (`:1448-1453`).
    Thin,
    /// Anything with no collision: always passable (`:1454`).
    NoCollision,
    /// Something solid: not passable at all.
    Wall,
}

/// `isPassable`'s permission half, one disjunct per Java line.
///
/// **The first version collapses two switches.** Its `Costs::doors_open` is
/// `enter_doors || enter_gates`, but Java asks `canEnterDoors` for a
/// `DoorBlock` and `canEnterGates` for a `FenceGateBlock`. With Java's defaults
/// (`enterDoors = false`, `enterGates = true`) a *door* is closed and a *gate*
/// is open; the collapse opens both. `parity` pins the divergence.
pub fn passes(barrier: Barrier, doors: bool, gates: bool) -> bool {
    match barrier {
        Barrier::Door => doors,
        Barrier::Gate | Barrier::ColonyGate => gates,
        Barrier::Thin | Barrier::NoCollision => true,
        Barrier::Wall => false,
    }
}

/// `canEnterDoors` read the way `isPassable` reads it, for the parity test.
pub fn enters_door(doors: bool) -> bool {
    doors
}

/// `canEnterGates` read the way `isPassable` reads it, for the parity test.
pub fn enters_gate(gates: bool) -> bool {
    gates
}

/// What makes a block dangerous to stand on (`PathfindingUtils.java:447-458`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Danger {
    /// `ModTags.dangerousBlocks` (`:451`).
    pub tagged: bool,
    /// A fire block (`:452`).
    pub fire: bool,
    /// A campfire (`:453`).
    pub campfire: bool,
    /// Magma (`:454`).
    pub magma: bool,
    /// A sweet berry bush (`:455`).
    pub berry_bush: bool,
    /// Powder snow (`:456`).
    pub powder_snow: bool,
    /// A lava cauldron (`:457`).
    pub lava_cauldron: bool,
}

/// `isDangerous`: any of the seven, and nothing else.
pub fn is_dangerous(facts: &Danger) -> bool {
    facts.tagged
        || facts.fire
        || facts.campfire
        || facts.magma
        || facts.berry_bush
        || facts.powder_snow
        || facts.lava_cauldron
}

/// `PathfindingUtils.hasAnyCollisionAlong` (`:472-569`): is anything solid in
/// the way from one cell to another?
///
/// Java walks a 3-D DDA: at each step it tests the current cell, then advances
/// the axis whose accumulated fractional cost is smallest. With `clearance` it
/// also tests the two cells a diagonal step would pass between, which is what
/// keeps a body from squeezing past a corner.
///
/// **The clearance branches never run in Java, and that is a bug.** Its guard
/// `doubleEquals` returns `false` whenever either value is finite
/// (`:571-579`): `if (Double.isFinite(a) || Double.isFinite(b) || …) return
/// false;`. Both arguments are accumulated step costs, always finite or
/// infinite, so the first conjunct is true for every real call and the whole
/// method answers `false`. The port keeps Java *as executed* by default
/// (`clearance = false`) and offers Java *as intended* as an option
/// (`clearance = true`), so a caller can choose parity or the corner test.
pub fn has_any_collision_along<F: Fn(Pos) -> bool>(
    start: Pos,
    end: Pos,
    clearance: bool,
    blocked: F,
) -> bool {
    let dx = (end[0] - start[0]).abs();
    let dy = (end[1] - start[1]).abs();
    let dz = (end[2] - start[2]).abs();
    let step = |from: i32, to: i32| if to > from { 1 } else { -1 };
    let (sx, sy, sz) = (
        step(start[0], end[0]),
        step(start[1], end[1]),
        step(start[2], end[2]),
    );

    // `1 / d` per axis, and a half step to start, so the first move goes to the
    // longest axis (`:484-491`).
    let cost = |d: i32| {
        if d == 0 {
            f64::INFINITY
        } else {
            1.0 / f64::from(d)
        }
    };
    let mut sum_x = if dx == 0 {
        f64::INFINITY
    } else {
        0.5 / f64::from(dx)
    };
    let mut sum_y = if dy == 0 {
        f64::INFINITY
    } else {
        0.5 / f64::from(dy)
    };
    let mut sum_z = if dz == 0 {
        f64::INFINITY
    } else {
        0.5 / f64::from(dz)
    };
    let (cx, cy, cz) = (cost(dx), cost(dy), cost(dz));

    let mut at = start;
    for _ in 0..(dx + dy + dz) {
        if at == end {
            break;
        }
        if blocked(at) {
            return true;
        }
        if clearance {
            // The cells between two axes that advance together
            // (`:500-539`), with the test Java meant to write.
            if close(sum_x, sum_y)
                && (blocked([at[0] + sx, at[1], at[2]]) || blocked([at[0] + sx, at[1] + sy, at[2]]))
            {
                return true;
            }
            if close(sum_x, sum_z)
                && (blocked([at[0] + sx, at[1], at[2]]) || blocked([at[0] + sx, at[1], at[2] + sz]))
            {
                return true;
            }
            if close(sum_y, sum_z)
                && (blocked([at[0], at[1] + sy, at[2]]) || blocked([at[0], at[1] + sy, at[2] + sz]))
            {
                return true;
            }
        }
        // Advance the axis with the smallest accumulated cost (`:540-565`).
        if sum_x < sum_y {
            if sum_x < sum_z {
                at[0] += sx;
                sum_x += cx;
            } else {
                at[2] += sz;
                sum_z += cz;
            }
        } else if sum_y < sum_z {
            at[1] += sy;
            sum_y += cy;
        } else {
            at[2] += sz;
            sum_z += cz;
        }
    }
    blocked(end)
}

/// Whether two accumulated costs are close enough that two axes advance
/// together — `Math.abs(a - b) < 0.000005`, for finite values only
/// (`PathfindingUtils.java:571-579`, with the guard fixed).
fn close(a: f64, b: f64) -> bool {
    a.is_finite() && b.is_finite() && (a - b).abs() < 0.000_005
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain() -> BlockFacts {
        BlockFacts {
            shape_top: 1.0,
            width_x: 1.0,
            width_z: 1.0,
            ..BlockFacts::default()
        }
    }

    #[test]
    fn a_full_block_is_walkable_and_a_fence_is_refused() {
        assert_eq!(surface_of(&plain()), Surface::Walkable, ":122-130");
        let fence = BlockFacts {
            fence_or_wall: true,
            ..plain()
        };
        assert_eq!(surface_of(&fence), Surface::NotPassable, ":70-77");
    }

    #[test]
    fn a_dangerous_block_is_refused_before_its_shape_is_read() {
        let fire = BlockFacts {
            dangerous: true,
            shape_top: 0.2,
            ..plain()
        };
        assert_eq!(surface_of(&fire), Surface::NotPassable, ":56-67");
        let allowed = BlockFacts {
            pass_danger: true,
            ..fire
        };
        assert_eq!(surface_of(&allowed), Surface::Walkable, ":60-64");
        let airy = BlockFacts {
            shape_empty: true,
            shape_top: 0.0,
            ..allowed
        };
        assert_eq!(
            surface_of(&airy),
            Surface::Dropable,
            "danger you fall through"
        );
    }

    #[test]
    fn a_door_is_a_surface_you_fall_through() {
        let door = BlockFacts {
            door_or_gate: true,
            water: true,
            ..plain()
        };
        assert_eq!(
            surface_of(&door),
            Surface::Dropable,
            "a waterlogged gate is a gate, not water (:79-82)"
        );
    }

    #[test]
    fn a_closed_trapdoor_is_a_floor_and_an_open_one_is_a_gap() {
        let closed = BlockFacts {
            closed_panel: true,
            shape_top: 0.2,
            ..plain()
        };
        assert_eq!(surface_of(&closed), Surface::Walkable, ":96-99");
        let open = BlockFacts {
            shape_top: 0.2,
            width_x: 0.2,
            width_z: 1.0,
            ..plain()
        };
        assert_eq!(surface_of(&open), Surface::Dropable, "a gap (:132)");
    }

    #[test]
    fn water_is_walkable_lava_is_not_and_a_tall_block_is_a_wall() {
        let water = BlockFacts {
            water: true,
            shape_top: 0.9,
            width_x: 0.0,
            width_z: 0.0,
            ..plain()
        };
        assert_eq!(surface_of(&water), Surface::Walkable, ":106-110");
        let lava = BlockFacts {
            lava: true,
            water: false,
            ..water
        };
        assert_eq!(surface_of(&lava), Surface::NotPassable, ":112-115");
        let tall = BlockFacts {
            shape_top: 1.5,
            ..plain()
        };
        assert_eq!(surface_of(&tall), Surface::NotPassable, ":101-104");
    }

    #[test]
    fn a_sign_falls_through_and_a_deep_snow_layer_is_a_floor() {
        let sign = BlockFacts {
            soft_climbable: true,
            shape_top: 0.4,
            width_x: 0.2,
            ..plain()
        };
        assert_eq!(surface_of(&sign), Surface::Dropable, ":117-120");
        let snow = BlockFacts {
            shape_top: 0.3,
            width_x: 0.2,
            snow_layers: 2,
            ..plain()
        };
        assert_eq!(surface_of(&snow), Surface::Walkable, ":124");
        let thin = BlockFacts {
            snow_layers: 1,
            ..snow
        };
        assert_eq!(surface_of(&thin), Surface::Dropable, "one layer is a gap");
    }

    #[test]
    fn a_flat_block_over_lava_is_refused_unless_danger_is_allowed() {
        let carpet = BlockFacts {
            shape_top: 0.2,
            below_dangerous: true,
            ..plain()
        };
        assert_eq!(surface_of(&carpet), Surface::NotPassable, ":86-94");
        let allowed = BlockFacts {
            pass_danger: true,
            ..carpet
        };
        assert_eq!(surface_of(&allowed), Surface::Walkable);
    }

    #[test]
    fn water_is_a_solid_question_and_a_fluid_question() {
        assert!(
            !is_water(&WaterFacts {
                solid: true,
                water_block: true,
                ..WaterFacts::default()
            }),
            "a solid block is never swimmable (:294)"
        );
        assert!(
            is_water(&WaterFacts {
                water_block: true,
                ..WaterFacts::default()
            }),
            "Blocks.WATER always (:298)"
        );
        assert!(
            is_water(&WaterFacts {
                fluid: Fluid::Water,
                ..WaterFacts::default()
            }),
            "waterlogged (:320-321)"
        );
        assert!(
            !is_water(&WaterFacts {
                fluid: Fluid::Water,
                waterlogged_top_trapdoor: true,
                ..WaterFacts::default()
            }),
            "except a closed top trapdoor (:314-318)"
        );
        assert!(!is_water(&WaterFacts::default()), "no fluid, no water");
        assert_eq!(Fluid::default(), Fluid::Empty);
    }

    #[test]
    fn lava_is_a_block_or_a_fluid_and_a_solid_block_can_be_lava() {
        assert!(is_lava(true, Fluid::Empty), ":340");
        assert!(is_lava(false, Fluid::Lava), ":356-357");
        assert!(!is_lava(false, Fluid::Water));
    }

    #[test]
    fn a_ladder_is_free_and_a_vine_is_not() {
        assert!(is_ladder(&ClimbFacts {
            ladder: true,
            ..ClimbFacts::default()
        }));
        assert!(
            !is_ladder(&ClimbFacts {
                climbable_tag: true,
                advanced: false,
                ..ClimbFacts::default()
            }),
            "a vine without canClimbAdvanced (:373)"
        );
        assert!(is_ladder(&ClimbFacts {
            climbable_tag: true,
            advanced: true,
            ..ClimbFacts::default()
        }));
        assert!(
            is_ladder(&ClimbFacts {
                liquid: true,
                under_water: true,
                ..ClimbFacts::default()
            }),
            "water is a ladder when walking under it (:369)"
        );
        assert!(!is_ladder(&ClimbFacts::default()));
    }

    #[test]
    fn the_passability_table_follows_the_two_switches() {
        assert!(
            !passes(Barrier::Door, false, true),
            "a door needs canEnterDoors"
        );
        assert!(
            passes(Barrier::Gate, false, true),
            "a gate needs canEnterGates"
        );
        assert!(passes(Barrier::Gate, true, true));
        assert!(!passes(Barrier::Gate, true, false));
        assert!(
            passes(Barrier::Thin, false, false),
            "a sign is always passable"
        );
        assert!(passes(Barrier::NoCollision, false, false));
        assert!(!passes(Barrier::Wall, true, true));
        assert!(enters_door(true) && !enters_gate(false));
    }

    #[test]
    fn danger_is_any_of_seven_and_nothing_else() {
        assert!(!is_dangerous(&Danger::default()));
        for one in [
            Danger {
                tagged: true,
                ..Danger::default()
            },
            Danger {
                fire: true,
                ..Danger::default()
            },
            Danger {
                campfire: true,
                ..Danger::default()
            },
            Danger {
                magma: true,
                ..Danger::default()
            },
            Danger {
                berry_bush: true,
                ..Danger::default()
            },
            Danger {
                powder_snow: true,
                ..Danger::default()
            },
            Danger {
                lava_cauldron: true,
                ..Danger::default()
            },
        ] {
            assert!(is_dangerous(&one), "{one:?}");
        }
    }

    #[test]
    fn a_clear_line_has_no_collision_and_a_wall_in_it_does() {
        let wall = |p: Pos| p == [2, 0, 0];
        assert!(
            has_any_collision_along([0, 0, 0], [4, 0, 0], false, wall),
            "the wall is on the line (:493-498)"
        );
        assert!(
            !has_any_collision_along([0, 0, 5], [4, 0, 5], false, wall),
            "and nothing is on this one"
        );
        assert!(
            has_any_collision_along([0, 0, 0], [3, 0, 0], false, |p| p == [3, 0, 0]),
            "the end cell counts too (:568)"
        );
    }

    #[test]
    fn the_line_walks_all_three_axes() {
        // A diagonal through (1,1,1) and (2,2,2).
        let hit = |p: Pos| p == [2, 2, 2];
        assert!(has_any_collision_along([0, 0, 0], [4, 4, 4], false, hit));
        let miss = |p: Pos| p == [4, 0, 0];
        assert!(!has_any_collision_along([0, 0, 0], [4, 4, 4], false, miss));
    }

    #[test]
    fn the_corner_test_is_dead_in_java_and_offered_here() {
        // A body walking from (0,0,0) to (1,0,1) passes between (0,0,1) and
        // (1,0,0). Java's clearance test would refuse; as executed it does not.
        let corner = |p: Pos| p == [1, 0, 0];
        assert!(
            !has_any_collision_along([0, 0, 0], [1, 0, 1], false, corner),
            "Java as executed (:571-579 makes doubleEquals false)"
        );
        assert!(
            has_any_collision_along([0, 0, 0], [1, 0, 1], true, corner),
            "Java as intended (:500-539)"
        );
    }
}
