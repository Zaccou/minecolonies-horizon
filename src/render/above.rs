//! What draws above a citizen's head, where, and on what rule.
//!
//! Java reference: `RenderBipedCitizen.renderNameTag` (`:104-136`),
//! `CitizenDataView.hasVisibleStatus`/`getStatusIcon` (`:484-565`),
//! `api/entity/citizen/VisibleCitizenStatus.java` (the eight icons, already ported
//! as [`crate::citizens::Status`]), and the two item decorators
//! `ClipBoardDecorator.java` and `ColonyMapDecorator.java`.
//!
//! **The status table is not here.** `crate::citizens::Status` already carries
//! the eight statuses, their icon path, their key and the `renders_over_head`
//! flag read off `VisibleCitizenStatus.java:41-66`; this file only decides *which*
//! of the eight wins, *at what distance*, *where* the 10×10 quad lands, and what
//! is drawn when the answer is an interaction rather than a status.
//!
//! **Three answers, and the third one is a fallback chain** (`CitizenDataView.java:538-563`):
//!
//! 1. the status, when it is set and its `render` flag is true;
//! 2. the top interaction's own icon, when it has one;
//! 3. `textures/icons/blocking.png` while the citizen blocks something, else
//!    `textures/icons/warning.png` while any interaction is visible at all.
//!
//! **Two numbers decide whether it is drawn**, and they are not the same number:
//! the *visibility* comes from `hasVisibleStatus` (`:484-500`, a status or an
//! interaction at or above `CHITCHAT`) and the *distance* from the renderer
//! (`:112-114`, `distanceToSqr <= 4096`, i.e. 64 blocks). A citizen with a
//! visible status 65 blocks away draws nothing.
//!
//! **The badge on an item is a different thing again**: it is GUI, not world, and
//! it blinks — one toggle every 40 ticks (`ClipBoardDecorator.java:33-40`).

use std::collections::BTreeSet;

use crate::citizens::Status;

/// `RenderBipedCitizen.java:115` — the icon sits this far above the entity's own
/// name-tag anchor.
pub const ABOVE_NAMETAG: f32 = 0.3;

/// Half the quad's width in the scaled frame: the vertices go from `-5` to `5`
/// (`:126-129`).
pub const HALF_WIDTH: f32 = 5.0;

/// The quad's height in the same frame: `0` to `10` (`:127-128`).
pub const HEIGHT: f32 = 10.0;

/// The scale the quad is drawn at, `-0.025` on x and y, `+0.025` on z (`:124`).
pub const SCALE: f32 = 0.025;

/// `:112` — `distanceToSqr(...) <= 4096.0D`, squared blocks, so 64 blocks.
pub const MAX_DISTANCE_SQ: f64 = 4096.0;

/// `CitizenDataView.BLOCKING_RESOURCE` (`:62`).
pub const BLOCKING_ICON: &str = "textures/icons/blocking.png";

/// `CitizenDataView.PENDING_RESOURCE` (`:67`).
pub const PENDING_ICON: &str = "textures/icons/warning.png";

/// What `getStatusIcon` answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: five answers, and `None` is one of them
pub enum Above {
    /// Nothing to draw.
    Nothing,
    /// A status of [`crate::citizens::Status`].
    Status(Status),
    /// The top interaction's own icon path.
    Interaction(&'static str),
    /// The citizen blocks something.
    Blocking,
    /// Something is waiting, and nothing blocks.
    Pending,
}

impl Above {
    /// The icon path this answer draws, or `None` when there is nothing.
    ///
    /// The paths are the ones Java writes, `minecolonies:` included, because
    /// `worldEntityIcon` takes the full `ResourceLocation` (`:124`).
    pub fn icon(self) -> Option<&'static str> {
        match self {
            Above::Nothing => None,
            Above::Status(status) => Some(status.icon()),
            Above::Interaction(path) => Some(path),
            Above::Blocking => Some(BLOCKING_ICON),
            Above::Pending => Some(PENDING_ICON),
        }
    }
}

/// `CitizenDataView.hasVisibleStatus` (`:484-500`): a status that renders, or any
/// interaction at or above `CHITCHAT`.
///
/// `chitchat_or_higher` is the caller's answer for the interaction list; the
/// priority scale itself belongs to `crate::citizens::priority`.
pub fn visible(status: Option<Status>, chitchat_or_higher: bool) -> bool {
    match status {
        Some(status) if status.renders_over_head() => true,
        _ => chitchat_or_higher,
    }
}

/// `CitizenDataView.getStatusIcon` (`:538-563`), in its order.
///
/// `top_icon` is `sortedInteractions.get(0).getInteractionIcon()` — `None` when
/// there is no interaction, or when the top one has no icon of its own.
pub fn above(
    status: Option<Status>,
    blocking: bool,
    chitchat_or_higher: bool,
    top_icon: Option<&'static str>,
) -> Above {
    if let Some(status) = status {
        if status.renders_over_head() {
            return Above::Status(status);
        }
    }
    if let Some(icon) = top_icon {
        return Above::Interaction(icon);
    }
    if blocking {
        return Above::Blocking;
    }
    if visible(status, chitchat_or_higher) {
        return Above::Pending;
    }
    Above::Nothing
}

/// The distance gate (`:112-114`), on a value already squared.
pub fn within_icon_range(distance_sq: f64) -> bool {
    distance_sq <= MAX_DISTANCE_SQ
}

/// The same gate from two block positions, both in world coordinates: the
/// renderer measures from the camera, so `camera` is the player's eye block.
pub fn within_icon_range_at(camera: [i32; 3], citizen: [i32; 3]) -> bool {
    within_icon_range(crate::base::dist_sqr(camera, citizen) as f64)
}

/// Where the icon's four corners land, in blocks, above the entity.
///
/// The frame is the entity's own: Java translates to `(0, nameTagOffsetY + 0.3, 0)`
/// then turns to face the camera and scales by `-0.025` (`:116-124`), so the four
/// corners are `(-5, 0)`, `(-5, 10)`, `(5, 10)`, `(5, 0)` in that scaled frame —
/// returned here in texels of that frame, the caller applying [`SCALE`] and the
/// camera turn.
pub fn icon_corners() -> [[f32; 2]; 4] {
    [
        [-HALF_WIDTH, 0.0],
        [-HALF_WIDTH, HEIGHT],
        [HALF_WIDTH, HEIGHT],
        [HALF_WIDTH, 0.0],
    ]
}

/// The name-tag anchor plus the icon offset, which is the y Java translates to
/// before scaling (`:115-118`).
pub fn icon_anchor_y(name_tag_offset_y: f32) -> f32 {
    name_tag_offset_y + ABOVE_NAMETAG
}

/// The blink of the item badge: on for one 40-tick window out of two.
///
/// `ClipBoardDecorator.java:31-40`: `render` starts `false`, and every
/// `gametime % 40 == 0` flips it, so the badge shows for ticks 40-79, 120-159…
pub fn badge_shown(game_time: u64) -> bool {
    (game_time / BLINK_PERIOD) % 2 == 1
}

/// `gametime % 40` (`:33`): the badge's blink period, in ticks.
pub const BLINK_PERIOD: u64 = 40;

/// One of the two item badges.
#[derive(Clone, Copy, Debug, PartialEq)]
// exhaustive: five fields, all drawn
pub struct Badge {
    /// The number drawn, already counted.
    pub count: usize,
    /// `0xFFFF4500`: the clipboard's literal (`:60`) and the map's
    /// `0xFF4500 | (255 << 24)` (`:60`), the same colour written two ways.
    pub colour: u32,
    /// `xOffset + 15` (`:61`).
    pub x: f32,
    /// `yOffset - 2` (`:62`).
    pub y: f32,
    /// The `z` translate before drawing: `200` for the clipboard (`:57`), `500`
    /// for the map (`:58`), so the map's badge sits above the clipboard's.
    pub layer: f32,
}

/// The badge the clipboard shows: the player's own pending requests, minus the
/// ones a job has taken over (`ClipBoardDecorator.java:41-66`).
///
/// Java builds a set of the player resolver's and the retrying resolver's
/// requests, then removes every request any citizen's job holds. The set matters:
/// the two resolvers can hold the same token, and it is counted once.
pub fn clipboard_count(
    player: &BTreeSet<u64>,
    retrying: &BTreeSet<u64>,
    held_by_jobs: &[&BTreeSet<u64>],
) -> usize {
    let mut pending: BTreeSet<u64> = player.union(retrying).copied().collect();
    for held in held_by_jobs {
        for token in held.iter() {
            pending.remove(token);
        }
    }
    pending.len()
}

/// The clipboard's badge, `None` when there is nothing to show (`:64-72`).
pub fn clipboard_badge(count: usize) -> Option<Badge> {
    if count == 0 {
        return None;
    }
    Some(Badge {
        count,
        colour: 0xFFFF_4500,
        x: 15.0,
        y: -2.0,
        layer: 200.0,
    })
}

/// The colony map's badge: how many citizens have a blocking interaction
/// (`ColonyMapDecorator.java:41-56`).
pub fn map_badge(blocking_citizens: usize) -> Option<Badge> {
    if blocking_citizens == 0 {
        return None;
    }
    Some(Badge {
        count: blocking_citizens,
        colour: 0xFFFF_4500,
        x: 15.0,
        y: -2.0,
        layer: 500.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    // `super::*` already brings it in through the parent's private `use`; the
    // explicit one is kept so the test module does not depend on that.
    use crate::citizens::Status;

    #[test]
    fn a_status_that_renders_beats_every_other_answer() {
        assert_eq!(
            above(Some(Status::Eat), true, true, Some("textures/icons/x.png")),
            Above::Status(Status::Eat)
        );
        assert_eq!(
            Above::Status(Status::Sick).icon(),
            Some("textures/icons/small_sick_icon.png")
        );
    }

    #[test]
    fn working_never_draws_even_when_it_is_the_status() {
        // The one status with the two-argument constructor
        // (`VisibleCitizenStatus.java:41`), so `shouldRender` is false.
        assert_eq!(
            above(Some(Status::Working), false, false, None),
            Above::Nothing
        );
        assert!(!Status::Working.renders_over_head());
        // …and its icon exists anyway, which is why the flag has to be tested.
        assert_eq!(Status::Working.icon(), "textures/icons/working.png");
    }

    #[test]
    fn an_interaction_icon_beats_the_two_fallbacks() {
        let icon = Some("textures/icons/quest.png");
        assert_eq!(
            above(None, true, true, icon),
            Above::Interaction("textures/icons/quest.png")
        );
    }

    #[test]
    fn blocking_wins_over_pending_and_pending_needs_a_visible_interaction() {
        assert_eq!(above(None, true, true, None), Above::Blocking);
        assert_eq!(above(None, false, true, None), Above::Pending);
        assert_eq!(above(None, false, false, None), Above::Nothing);
        assert_eq!(Above::Blocking.icon(), Some(BLOCKING_ICON));
        assert_eq!(Above::Pending.icon(), Some(PENDING_ICON));
        assert_eq!(Above::Nothing.icon(), None);
        assert_ne!(BLOCKING_ICON, PENDING_ICON);
    }

    #[test]
    fn visibility_is_not_the_same_question_as_the_icon() {
        // A working citizen is visible for neither, but an asleep one with a
        // status is visible for both.
        assert!(!visible(Some(Status::Working), false));
        assert!(visible(Some(Status::Sleep), false));
        assert!(visible(None, true));
        assert!(!visible(None, false));
    }

    #[test]
    fn the_distance_gate_is_4096_squared_blocks() {
        assert!(within_icon_range(4096.0), "64 blocks exactly is drawn");
        assert!(!within_icon_range(4096.5));
        assert!(within_icon_range(0.0));
        // A block-position version, on the corner of the sphere.
        assert!(within_icon_range_at([0, 0, 0], [64, 0, 0]));
        assert!(!within_icon_range_at([0, 0, 0], [65, 0, 0]));
        assert!(!within_icon_range_at([0, 0, 0], [64, 1, 0]));
    }

    #[test]
    fn the_icon_is_ten_by_ten_and_sits_point_three_above_the_tag() {
        assert_eq!(
            icon_corners(),
            [
                [-5.0, 0.0],
                [-5.0, 10.0],
                [5.0, 10.0],
                [5.0, 0.0]
            ]
        );
        assert!((icon_anchor_y(2.0) - 2.3).abs() < 1e-6);
        assert!((SCALE - 0.025).abs() < f32::EPSILON);
    }

    #[test]
    fn the_badge_blinks_on_forty_ticks_and_off_forty() {
        for tick in 0..40 {
            assert!(!badge_shown(tick), "tick {tick} is the initial off window");
        }
        for tick in 40..80 {
            assert!(badge_shown(tick), "tick {tick} is the on window");
        }
        for tick in 80..120 {
            assert!(!badge_shown(tick), "tick {tick} is off again");
        }
        assert!(badge_shown(120));
        assert!(badge_shown(159));
        assert!(!badge_shown(160));
        // 1240 / 40 = 31, an odd window: on.
        assert!(badge_shown(1240));
    }

    #[test]
    fn the_two_badges_share_a_colour_and_a_corner_and_differ_in_layer() {
        let clipboard = clipboard_badge(3).expect("three requests");
        let map = map_badge(1).expect("one blocked citizen");
        assert_eq!(clipboard.colour, 0xFFFF_4500);
        assert_eq!(map.colour, clipboard.colour);
        assert_eq!((clipboard.x, clipboard.y), (15.0, -2.0));
        assert_eq!((map.x, map.y), (15.0, -2.0));
        assert_eq!(clipboard.layer, 200.0);
        assert_eq!(map.layer, 500.0);
        assert_eq!(clipboard.count, 3);
    }

    #[test]
    fn a_zero_count_draws_nothing() {
        assert_eq!(clipboard_badge(0), None);
        assert_eq!(map_badge(0), None);
    }

    #[test]
    fn the_pending_count_unions_the_two_resolvers_and_drops_what_a_job_holds() {
        let player: BTreeSet<u64> = [1, 2, 3].into_iter().collect();
        let retrying: BTreeSet<u64> = [3, 4].into_iter().collect();
        // 3 is in both resolvers and counts once.
        assert_eq!(clipboard_count(&player, &retrying, &[]), 4);
        let farmer: BTreeSet<u64> = [1].into_iter().collect();
        let builder: BTreeSet<u64> = [4, 9].into_iter().collect();
        assert_eq!(clipboard_count(&player, &retrying, &[&farmer, &builder]), 2);
        assert_eq!(
            clipboard_count(&player, &retrying, &[&farmer, &builder, &retrying]),
            1
        );
        assert_eq!(clipboard_count(&BTreeSet::new(), &BTreeSet::new(), &[]), 0);
    }
}
