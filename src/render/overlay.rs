//! What the mod draws into the world itself: the colony border, the boxes around
//! a build order, the pathfinding debug nodes, and how long a highlight lasts.
//!
//! Java reference: `core/client/render/worldevent/` — `ColonyBorderRenderer`,
//! `ColonyBlueprintRenderer`, `PathfindingDebugRenderer`, `HighlightManager`,
//! `ColonyWorldRenderMacros`, `RenderTypes`, `GuardTowerRallyBannerRenderer`,
//! `ItemOverlayBoxesRenderer`, `ColonyWaypointRenderer`, `ColonyPatrolPointRenderer`.
//!
//! **The colours are the rule; the vertices are not.** Every overlay here is a
//! wireframe or a line, and every one of them is drawn twice: once with a `>=`
//! depth test so it shows only *outside* blocks, once with `>` so it shows only
//! *inside* them, the second pass at **half the colour**
//! (`ColonyWorldRenderMacros.java:44-47`, `:64-66`). That is data: two passes and a
//! division, not a shader.
//!
//! | Overlay | Colour | Width | Through blocks |
//! | --- | --- | --- | --- |
//! | own colony's chunk border | white `0xFFFFFFFF` | 1 px line | n/a |
//! | another colony's chunk border | red `0xFFFF4646` | 1 px line | n/a |
//! | a work order's box | blue `0xFF0000FF` | `0.08` | no |
//! | the building's anchor | red `0xFFFF0000` | `0.02` | yes |
//! | the builder walking there | green `0xFF00FF00` | `0.02` | yes |
//! | a node of the path | six colours, see [`PathNode`] | `0.25` box | n/a |
//!
//! **The chunk border is not drawn on every chunk edge.** A line is written on a
//! side only when the neighbouring chunk belongs to another colony
//! (`ColonyBorderRenderer.java:249-256`), and inside the chunk the player stands
//! in, the grid is 4 blocks instead of 16 (`:26`, `:257-271`) so the player can see
//! where the boundary runs. Both are rules about *which* lines, which is exactly
//! what survives without a `VertexBuffer`.

use crate::base::ChunkPos;

/// `ColonyBorderRenderer.CHUNK_HEIGHT` (`:25`): the border is a full column.
pub const BORDER_HEIGHT: i32 = 256;

/// `CHUNK_SIZE` (`:24`), the vertical grid of an ordinary chunk.
pub const BORDER_STEP: i32 = 16;

/// `PLAYER_CHUNK_STEP = CHUNK_SIZE / 4` (`:26`): the grid inside the player's own
/// chunk.
pub const PLAYER_STEP: i32 = 4;

/// `RENDER_DIST_THRESHOLD` (`:23`): how many chunks short of the render distance
/// the border stops.
pub const RENDER_DIST_THRESHOLD: i32 = 3;

/// The smallest range the border is built over, `max(..., 2)` (`:91`).
pub const MIN_RANGE: i32 = 2;

/// The colour of the player's own colony: `(255, 255, 255, 255)` (`:238`).
pub const OWN_COLOUR: u32 = 0xFFFF_FFFF;

/// The colour of another colony: `(255, 70, 70, 255)` (`:242`).
pub const OTHER_COLOUR: u32 = 0xFFFF_4646;

/// The range, in chunks, the border covers: `max(render_distance - 3, 2)` (`:91`).
pub fn border_range(render_distance: i32) -> i32 {
    (render_distance - RENDER_DIST_THRESHOLD).max(MIN_RANGE)
}

/// Which of the four vertical edges a chunk draws.
///
/// The four values are the corners in `z`-ascending, `x`-ascending order — nw,
/// ne, sw, se — and each is `north || west`, `north || east`, `south || west`,
/// `south || east` (`:257-275`): a corner line is drawn only where the neighbour
/// across it belongs to somebody else.
pub fn border_corners(north: bool, south: bool, east: bool, west: bool) -> [bool; 4] {
    [
        north || west,
        north || east,
        south || west,
        south || east,
    ]
}

/// Whether a chunk draws its horizontal rings on a side (`:276-330`): the same
/// four comparisons, one line per side.
pub fn border_sides(north: bool, south: bool, east: bool, west: bool) -> [bool; 4] {
    [north, south, west, east]
}

/// The vertical spacing of a chunk's rings, in blocks: 4 inside the player's own
/// chunk, 16 elsewhere (`:257-271`).
pub fn border_step(in_player_chunk: bool) -> i32 {
    if in_player_chunk { PLAYER_STEP } else { BORDER_STEP }
}

/// The colour a chunk of colony `id` is bordered with.
///
/// `colonyteamborders` on (`ColonyBorderRenderer.java:230`): the colony's own team
/// colour. Off: white for the player's colony, red for every other. A colony the
/// client does not know falls back the same way (`:222-230`).
pub fn border_colour(own_colony: bool, team: Option<u32>) -> u32 {
    match team {
        Some(rgb) => 0xFF00_0000 | (rgb & 0x00FF_FFFF),
        None if own_colony => OWN_COLOUR,
        None => OTHER_COLOUR,
    }
}

/// One wireframe box an overlay draws.
#[derive(Clone, Copy, Debug, PartialEq)]
// exhaustive: five fields, all passed to the line box
pub struct LineBox {
    /// `argb`, as Java passes it.
    pub colour: u32,
    /// Half the line's thickness, in blocks.
    pub width: f32,
    /// Whether the box is also drawn through blocks, at half colour.
    pub through: bool,
}

/// A work order's blueprint box: blue, `0.08`, opaque
/// (`ColonyBlueprintRenderer.java:186-190`).
pub const WORK_ORDER_BOX: LineBox = LineBox {
    colour: 0xFF00_00FF,
    width: 0.08,
    through: false,
};

/// The building's anchor, while the player sneaks: red, `0.02`, through blocks
/// (`:192-198`).
pub const ANCHOR_BOX: LineBox = LineBox {
    colour: 0xFFFF_0000,
    width: 0.02,
    through: true,
};

/// The builder's position, while the player sneaks: green, `0.02`, through blocks
/// (`:200-213`).
pub const BUILDER_BOX: LineBox = LineBox {
    colour: 0xFF00_FF00,
    width: 0.02,
    through: true,
};

/// A guard tower's rally banner, in another dimension: black, `0.02`
/// (`GuardTowerRallyBannerRenderer.java:33-40`).
pub const RALLY_BOX: LineBox = LineBox {
    colour: 0xFF00_0000,
    width: 0.02,
    through: false,
};

/// The colour a box is drawn with in the through-blocks pass: every channel
/// halved, alpha included (`ColonyWorldRenderMacros.java:44-47`).
pub fn halved(argb: u32) -> u32 {
    let a = ((argb >> 24) & 0xFF) / 2;
    let r = ((argb >> 16) & 0xFF) / 2;
    let g = ((argb >> 8) & 0xFF) / 2;
    let b = (argb & 0xFF) / 2;
    (a << 24) | (r << 16) | (g << 8) | b
}

/// How far the player may walk before the blueprint cache is rebuilt, in blocks:
/// `CACHE_RESET_RANGE = 12.5` (`:34`), compared with `closerThan`.
pub const CACHE_RESET_RANGE: f64 = 12.5;

/// How often the pending blueprint queue is drained, in ticks
/// (`:158-161`, `gametime % 20 == 0`).
pub const PENDING_INTERVAL: u64 = 20;

/// The transparency of the assistant hammer's preview
/// (`:379`, `setOverridePreviewTransparency(0.4f)`).
pub const PREVIEW_TRANSPARENCY: f32 = 0.4;

/// A node set of the pathfinding debug renderer, and the colour it is drawn with
/// (`PathfindingDebugRenderer.java:49-79`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the six sets Java draws
pub enum PathNode {
    /// Nodes the search closed: `0xffff0000`, red.
    Visited,
    /// Nodes closed later: `0xffff5050`, salmon.
    VisitedLater,
    /// The nodes of the previous path: `0xff808080`, grey.
    PreviousPath,
    /// Extra nodes: `0xff9999ff`, pale blue.
    Extra,
    /// Nodes never reached: `0xff0000ff`, blue.
    NotVisited,
    /// The chosen path, not yet reached by the worker: `0xffffff00`, yellow.
    Path,
    /// The chosen path's reached node: `0xffff8800`, orange.
    Reached,
}

impl PathNode {
    /// The `argb` Java draws this set with.
    pub fn colour(self) -> u32 {
        match self {
            PathNode::Visited => 0xFFFF_0000,
            PathNode::VisitedLater => 0xFFFF_5050,
            PathNode::PreviousPath => 0xFF80_8080,
            PathNode::Extra => 0xFF99_99FF,
            PathNode::NotVisited => 0xFF00_00FF,
            PathNode::Path => 0xFFFF_FF00,
            PathNode::Reached => 0xFFFF_8800,
        }
    }

    /// Which set a path node belongs to, given whether the worker has reached it
    /// (`:73-75`): the only set that depends on a runtime fact.
    pub fn on_path(reached_by_worker: bool) -> PathNode {
        if reached_by_worker {
            PathNode::Reached
        } else {
            PathNode::Path
        }
    }
}

/// `PathfindingDebugRenderer`'s box size: the pose is scaled by `0.25` (`:112`).
pub const DEBUG_BOX_SCALE: f32 = 0.25;

/// The distance under which a node also shows its cost and heuristic, in blocks
/// squared: `5 * 5` (`:106`).
pub const DEBUG_TEXT_DIST_SQ: i64 = 25;

/// How long a highlight lasts, in ticks: `duration.getSeconds() * 20`
/// (`HighlightManager.java:90`).
pub fn highlight_ticks(seconds: u64) -> u64 {
    seconds * 20
}

/// Whether a highlight has expired at `game_time`, for a start time and a
/// duration in seconds (`HighlightManager.java:84-93`).
///
/// An **infinite** highlight is the one with no duration (`:31-32`), which Java
/// expresses with a `null` and the port with `None`.
pub fn highlight_expired(start: u64, duration: Option<u64>, game_time: u64) -> bool {
    match duration {
        Some(seconds) => start + highlight_ticks(seconds) < game_time,
        None => false,
    }
}

/// The two blueprint templates the world renders as markers, by structure pack and
/// path.
///
/// The pack is `Constants.STORAGE_STYLE` (`:45`) and both paths are fixed
/// (`ColonyWaypointRenderer.java:51`, `ColonyPatrolPointRenderer.java:66`);
/// the waypoint one is only used when the blueprint the player holds is itself a
/// waypoint (`:45`, `contains("infrastructure")`).
pub const WAYPOINT_BLUEPRINT: (&str, &str) = ("Minecolonies Original", "infrastructure/misc/waypoint.blueprint");

/// The guard scepter's patrol point template (`ColonyPatrolPointRenderer.java:66`).
pub const PATROL_BLUEPRINT: (&str, &str) = ("Minecolonies Original", "infrastructure/misc/patrolpoint.blueprint");

/// The two presets that turn blueprint rendering on, and what each needs
/// (`ColonyBlueprintRenderer.java:92-97`, the rule list, in order).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
// exhaustive: three rules, and the hotkey is not one of them
pub enum BlueprintRule {
    /// `NearBuildPreview`: the build tool in hand, a blueprint being previewed,
    /// and the neighbour-rendering setting on (`:252-262`).
    NearBuildPreview,
    /// `BuildGoggles`: the goggles on the head (`:283-287`).
    BuildGoggles,
    /// `AssistantHammerPreview`: the assistant hammer in hand (`:323-327`).
    AssistantHammer,
}

/// The rules in force, in the order they are asked; the first one that answers
/// for a position wins (`:95-97`, reversed at `:228`).
pub const BLUEPRINT_RULES: &[BlueprintRule] = &[
    BlueprintRule::NearBuildPreview,
    BlueprintRule::BuildGoggles,
    BlueprintRule::AssistantHammer,
];

/// The chunk a block belongs to, for the border's map.
///
/// A re-export so a caller of [`border_corners`] does not have to import the
/// conversion from two places.
pub fn chunk_of_block(x: i32, z: i32) -> ChunkPos {
    crate::base::chunk_of([x, 0, z])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_border_stops_three_chunks_short_and_never_below_two() {
        assert_eq!(border_range(12), 9);
        assert_eq!(border_range(5), 2);
        assert_eq!(border_range(4), 2, "1 would be too small, so 2");
        assert_eq!(border_range(2), 2);
    }

    #[test]
    fn a_corner_line_is_drawn_only_where_the_neighbour_is_another_colony() {
        // Every neighbour ours: nothing to draw.
        assert_eq!(border_corners(false, false, false, false), [false; 4]);
        // North differs: the two northern corners.
        assert_eq!(border_corners(true, false, false, false), [true, true, false, false]);
        // West differs: the two western corners.
        assert_eq!(border_corners(false, false, false, true), [true, false, true, false]);
        // All differ: the whole box.
        assert_eq!(border_corners(true, true, true, true), [true; 4]);
        // Diagonally opposite only: still two corners, because a corner is
        // `north || west`, not `north && west`.
        assert_eq!(border_corners(true, false, true, false), [true, true, false, true]);
        assert_eq!(border_sides(true, false, false, true), [true, false, true, false]);
    }

    #[test]
    fn the_grid_is_four_blocks_in_the_players_own_chunk() {
        assert_eq!(border_step(true), 4);
        assert_eq!(border_step(false), 16);
        assert_eq!(PLAYER_STEP * 4, BORDER_STEP);
        assert_eq!(BORDER_HEIGHT, 256);
        assert_eq!(BORDER_HEIGHT % BORDER_STEP, 0);
        assert_eq!(BORDER_STEP % PLAYER_STEP, 0);
    }

    #[test]
    fn a_team_colour_wins_over_white_and_red() {
        assert_eq!(border_colour(true, None), OWN_COLOUR);
        assert_eq!(border_colour(false, None), OTHER_COLOUR);
        assert_eq!(OWN_COLOUR, 0xFFFF_FFFF);
        assert_eq!(OTHER_COLOUR, 0xFFFF_4646);
        // A team colour is forced opaque: java takes `ChatFormatting.getColor()`,
        // an RGB, and builds `new Color(rgb)` which is opaque.
        assert_eq!(border_colour(true, Some(0x00FF_0000)), 0xFFFF_0000);
        assert_eq!(border_colour(false, Some(0x0000_00FF)), 0xFF00_00FF);
    }

    #[test]
    fn the_boxes_have_the_colours_and_widths_java_passes() {
        assert_eq!(WORK_ORDER_BOX.colour, 0xFF00_00FF);
        assert_eq!(WORK_ORDER_BOX.width, 0.08);
        assert!(!WORK_ORDER_BOX.through);
        assert_eq!(ANCHOR_BOX.width, 0.02);
        assert!(ANCHOR_BOX.through);
        assert_eq!(BUILDER_BOX.colour, 0xFF00_FF00);
        assert_eq!(BUILDER_BOX.width, ANCHOR_BOX.width);
        // The rally box of a tower in another dimension is black, not coloured.
        assert_eq!(RALLY_BOX.colour, 0xFF00_0000);
    }

    #[test]
    fn the_through_blocks_pass_is_half_of_the_colour_alpha_included() {
        assert_eq!(halved(0xFFFF_4646), 0x7F7F_2323);
        assert_eq!(halved(0xFF00_00FF), 0x7F00_007F);
        assert_eq!(halved(0x0000_0000), 0x0000_0000);
        assert_eq!(halved(0xFFFF_FFFF), 0x7F7F_7F7F);
    }

    #[test]
    fn the_seven_path_colours_and_the_one_that_depends_on_the_worker() {
        assert_eq!(PathNode::Visited.colour(), 0xFFFF_0000);
        assert_eq!(PathNode::VisitedLater.colour(), 0xFFFF_5050);
        assert_eq!(PathNode::PreviousPath.colour(), 0xFF80_8080);
        assert_eq!(PathNode::Extra.colour(), 0xFF99_99FF);
        assert_eq!(PathNode::NotVisited.colour(), 0xFF00_00FF);
        assert_eq!(PathNode::Path.colour(), 0xFFFF_FF00);
        assert_eq!(PathNode::Reached.colour(), 0xFFFF_8800);
        assert_eq!(PathNode::on_path(true), PathNode::Reached);
        assert_eq!(PathNode::on_path(false), PathNode::Path);
        // All seven are distinct and opaque.
        let colours = [
            PathNode::Visited,
            PathNode::VisitedLater,
            PathNode::PreviousPath,
            PathNode::Extra,
            PathNode::NotVisited,
            PathNode::Path,
            PathNode::Reached,
        ]
        .map(PathNode::colour);
        let mut unique = Vec::from(colours);
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 7, "two sets share a colour");
        assert!(unique.iter().all(|colour| colour >> 24 == 0xFF));
    }

    #[test]
    fn a_highlight_lasts_twenty_ticks_a_second_and_none_lasts_forever() {
        assert_eq!(highlight_ticks(0), 0);
        assert_eq!(highlight_ticks(5), 100);
        assert!(!highlight_expired(100, Some(5), 199), "start + 100 > 199");
        assert!(!highlight_expired(100, Some(5), 200), "the boundary is inclusive");
        assert!(highlight_expired(100, Some(5), 201));
        assert!(!highlight_expired(100, None, 1_000_000), "an infinite highlight");
    }

    #[test]
    fn the_blueprint_cache_numbers_are_the_ones_java_compares() {
        assert_eq!(CACHE_RESET_RANGE, 12.5);
        assert_eq!(PENDING_INTERVAL, 20);
        assert!((PREVIEW_TRANSPARENCY - 0.4).abs() < f32::EPSILON);
        assert_eq!(WAYPOINT_BLUEPRINT.0, PATROL_BLUEPRINT.0);
        assert_ne!(WAYPOINT_BLUEPRINT.1, PATROL_BLUEPRINT.1);
        assert!(WAYPOINT_BLUEPRINT.1.contains("infrastructure"));
        assert!(PATROL_BLUEPRINT.1.contains("infrastructure"));
    }

    #[test]
    fn the_three_blueprint_rules_are_asked_in_java_order() {
        assert_eq!(BLUEPRINT_RULES.len(), 3);
        assert_eq!(BLUEPRINT_RULES[0], BlueprintRule::NearBuildPreview);
        assert_eq!(BLUEPRINT_RULES[2], BlueprintRule::AssistantHammer);
        let mut sorted = BLUEPRINT_RULES.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 3);
    }

    #[test]
    fn the_debug_renderer_boxes_are_a_quarter_block_and_its_text_needs_five() {
        assert_eq!(DEBUG_BOX_SCALE, 0.25);
        assert_eq!(DEBUG_TEXT_DIST_SQ, 25);
        assert_eq!(chunk_of_block(-1, -1), ChunkPos::new(-1, -1));
        assert_eq!(chunk_of_block(16, 31), ChunkPos::new(1, 1));
    }
}
