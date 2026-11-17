//! `TeleportHelper`: where a teleport lands.
//!
//! Java reference: `core/util/TeleportHelper.java` (179 lines, six methods) plus
//! the one rule of `WorkerUtil.setSpawnPoint` (`WorkerUtil.java:118-133`) that it
//! shares. Five of the six methods move a `ServerPlayer` — `teleportTo`, a chunk
//! ticket, `stopSleepInBed`, a chat message — and none of that is a decision. What
//! is a decision is **where**, and that is three functions:
//!
//! | Question | Java |
//! | --- | --- |
//! | where does a citizen stand on this block? | `WorkerUtil.java:125-130`, `TeleportHelper.java:56-61` |
//! | which height in this column is open sky? | `BlockPosUtil.findLand`, `:330-361` |
//! | which block does a colony teleport aim at? | `colonyTeleport`, `:140-178` |
//!
//! # The two spawn offsets are **not** the same rule
//!
//! `WorkerUtil.setSpawnPoint` and `TeleportHelper.teleportCitizen` move a citizen
//! to `(x + 0.5, y, z + 0.5)` — the middle of the block, feet on its floor. That is
//! [`spawn_position`]. `CitizenManager` spawns a *new* citizen at
//! `(x + 0.5, y + 0.1, z + 0.5)` — `manager_04/citizens.rs:74`, from
//! `Constants.SLIGHTLY_UP` — which is a different constant for a different
//! situation (`CitizenManager.java:313`). Both are `0.5` on the horizontal axes;
//! only one of them lifts. They are deliberately **not** merged, and the 0.1 is
//! visible in `spawn_position`'s signature: there isn't one.
//!
//! # `findAround` is the gap
//!
//! `colonyTeleport` finds its landing spot with
//! `BlockPosUtil.findAround(world, position, 5, 5, predicate)`
//! (`api/util/BlockPosUtil.java:1035-1112`) — an eighty-line outward spiral that
//! tests a predicate on live block states. It is **not** ported here: it belongs to
//! the transverse floor (`crate::base`, where `BlockPosUtil`'s distances already
//! live) and this mission does not own that file. [`colony_teleport_target`] takes
//! it as a callback, so the fallback order is ported and testable while the spiral
//! waits for its own module. `MANQUANT.md` §5 records the gap.

use super::worker::MIDDLE_BLOCK_OFFSET;

/// The height `surfaceTeleport` starts its search at, `250`
/// (`TeleportHelper.java:87`): above the build limit of every shipped dimension,
/// so the binary search starts in open air.
pub const SURFACE_SCAN_CEILING: i32 = 250;

/// How far above the ground `surfaceTeleport` puts the player, `2.0`
/// (`TeleportHelper.java:100`).
pub const SURFACE_LIFT: f64 = 2.0;

/// Where a citizen stands on a block: `WorkerUtil.setSpawnPoint`
/// (`WorkerUtil.java:125-130`) and `TeleportHelper.teleportCitizen`
/// (`TeleportHelper.java:56-61`), which compute the same three numbers.
///
/// ```java
/// citizen.moveTo(spawnPoint.getX() + 0.5, spawnPoint.getY(), spawnPoint.getZ() + 0.5, yaw, pitch);
/// ```
///
/// `y` is the block's own — feet on the floor, not a half-block up, and **not** the
/// `+0.1` a newly spawned citizen gets (`constants::SLIGHTLY_UP`, a different
/// rule; module note).
pub fn spawn_position(spawn: [i32; 3]) -> [f64; 3] {
    [
        f64::from(spawn[0]) + MIDDLE_BLOCK_OFFSET,
        f64::from(spawn[1]),
        f64::from(spawn[2]) + MIDDLE_BLOCK_OFFSET,
    ]
}

/// Where `surfaceTeleport` starts looking, before the search descends
/// (`TeleportHelper.java:87`): the player's own column, at
/// [`SURFACE_SCAN_CEILING`].
pub fn surface_start(at: [i32; 3]) -> [i32; 3] {
    [at[0], SURFACE_SCAN_CEILING, at[2]]
}

/// Where the player ends up, once the search found the ground
/// (`TeleportHelper.java:100`): [`SURFACE_LIFT`] above it.
pub fn surface_target(ground_y: i32) -> f64 {
    f64::from(ground_y) + SURFACE_LIFT
}

/// The y to land on in this column: `BlockPosUtil.findLand`
/// (`api/util/BlockPosUtil.java:330-361`).
///
/// ```java
/// int top = y, bot = 0, mid = y;
/// BlockPos foundland = null, tempPos = blockPos;
/// while (top >= bot) {
///     tempPos = new BlockPos(x, mid, z);
///     if (block instanceof AirBlock && world.canSeeSkyFromBelowWater(tempPos)) { top = mid - 1; foundland = tempPos; }
///     else { bot = mid + 1; foundland = tempPos; }
///     mid = (bot + top) / 2;
/// }
/// if (BlockUtils.isAnySolid(world.getBlockState(tempPos))) return foundland.above();
/// return foundland;
/// ```
///
/// A binary search for the **lowest open-sky air block** in `0..=start_y`, in at
/// most nine probes. `is_open_sky(y)` is Java's `block instanceof AirBlock &&
/// canSeeSkyFromBelowWater(...)`; `is_solid(y)` decides whether the landing spot
/// has to be lifted one block, which is what makes the player stand *on* the
/// ground instead of inside it.
///
/// The two answers:
///
/// - the found cell is solid (the search ended on the ground) → that y **plus
///   one**: the player stands on top;
/// - the found cell is air → that y, unchanged.
///
/// **`start_y < 0` is refused.** Java's loop never runs (`top < bot`), so
/// `foundland` stays `null`, and `foundland.above()` throws a
/// `NullPointerException` if the block at `start_y` is solid — a real crash on a
/// negative-Y dimension with a solid floor above the search start. `None` is the
/// named refusal; nothing above `start_y` is searched either way.
///
/// The mid-point arithmetic is Java's, and so is the truncation: `(0 + -1) / 2`
/// is `0` in both languages, which is what lets an all-air column answer `0`
/// without a special case.
pub fn find_land(
    start_y: i32,
    is_open_sky: impl Fn(i32) -> bool,
    is_solid: impl Fn(i32) -> bool,
) -> Option<i32> {
    if start_y < 0 {
        return None;
    }
    let mut top = start_y;
    let mut bot = 0;
    let mut mid = start_y;
    let mut found = start_y;
    while top >= bot {
        found = mid;
        if is_open_sky(mid) {
            top = mid - 1;
        } else {
            bot = mid + 1;
        }
        mid = (bot + top) / 2;
    }
    if is_solid(found) {
        Some(found + 1)
    } else {
        Some(found)
    }
}

/// The block a colony teleport aims at, before the spiral search:
/// `colonyTeleport` (`TeleportHelper.java:142-153`).
///
/// ```java
/// BlockPos position = pos;
/// if (pos == null) {
///     if (colony.getServerBuildingManager().getTownHall() != null) position = ...getTownHall().getPosition();
///     else position = colony.getCenter();
/// }
/// ```
///
/// Three steps, in order: the **preferred** position the caller named (a
/// building's coordinates, `/colony tp <id> <pos>`), then the **town hall**, then
/// the colony's **center**. A colony always has a center — it is what founding
/// writes — so there is always an anchor, and the refusal happens later, at
/// [`colony_teleport_target`].
pub fn teleport_anchor(
    preferred: Option<[i32; 3]>,
    town_hall: Option<[i32; 3]>,
    center: [i32; 3],
) -> [i32; 3] {
    preferred.or(town_hall).unwrap_or(center)
}

/// Where the player actually lands, or nothing: `colonyTeleport`
/// (`TeleportHelper.java:142-178`).
///
/// The anchor above, then `findAround(world, position, 5, 5, walksOn)` — a landing
/// spot with air for the player **and air above it** (`:157-162`). `find_around`
/// is the caller's spiral (module note); this function only decides what a failure
/// means: `None`, and **no teleport at all**. Java returns from the method without
/// a message (`:163-166`), so a colony whose town hall is buried in a mountain
/// silently refuses rather than dropping the player into stone.
pub fn colony_teleport_target(
    preferred: Option<[i32; 3]>,
    town_hall: Option<[i32; 3]>,
    center: [i32; 3],
    find_around: impl FnOnce([i32; 3]) -> Option<[i32; 3]>,
) -> Option<[i32; 3]> {
    find_around(teleport_anchor(preferred, town_hall, center))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The middle of the block, feet on its floor.
    #[test]
    fn a_spawn_position_is_the_middle_of_its_block() {
        assert_eq!(spawn_position([10, 64, -3]), [10.5, 64.0, -2.5]);
        assert_eq!(spawn_position([0, 0, 0]), [0.5, 0.0, 0.5]);
        // Negative zero is still zero in the middle of the block.
        assert_eq!(spawn_position([-1, -60, -1]), [-0.5, -60.0, -0.5]);
    }

    /// The surface search starts above the build limit and lifts by two.
    #[test]
    fn the_surface_search_starts_at_250_and_lands_two_above_the_ground() {
        assert_eq!(surface_start([7, 12, -9]), [7, SURFACE_SCAN_CEILING, -9]);
        assert_eq!(surface_target(63), 65.0);
    }

    /// The trace a colony teleport actually runs: sky from `y = 10` up, ground
    /// below, so the player lands **on** the ground at `y = 10`.
    #[test]
    fn the_landing_height_is_the_lowest_open_sky_when_the_ground_is_solid() {
        // Open air from 10 up, solid under it.
        let sky = |y: i32| y >= 10;
        let solid = |y: i32| y < 10;
        assert_eq!(find_land(20, sky, solid), Some(10));
    }

    /// A column that is open all the way down answers `0`, and the mid-point
    /// arithmetic truncates towards zero exactly as Java's does.
    #[test]
    fn an_all_air_column_answers_zero() {
        assert_eq!(find_land(3, |_| true, |_| false), Some(0));
        assert_eq!(find_land(0, |_| true, |_| false), Some(0));
    }

    /// A column that is solid to the top answers just above the top.
    #[test]
    fn a_fully_solid_column_answers_one_above_its_top() {
        assert_eq!(find_land(5, |_| false, |_| true), Some(6));
    }

    /// The last cell being air means no lift: the player was already standing in
    /// the open.
    #[test]
    fn a_found_air_cell_is_not_lifted() {
        // Everything is open and nothing is solid: `found == 0`, no lift.
        assert_eq!(find_land(10, |_| true, |_| false), Some(0));
    }

    /// Lifting only happens when the *last probed* cell is solid — not when the
    /// `y` above it is.
    #[test]
    fn the_lift_reads_the_last_probed_cell() {
        // The probe sequence for `start = 10` ends on 4; make only 4 solid.
        let sky = |y: i32| y != 4;
        let solid = |y: i32| y == 4;
        assert_eq!(find_land(10, sky, solid), Some(5));
    }

    /// A negative start is refused instead of throwing like Java.
    #[test]
    fn a_negative_start_is_refused() {
        assert_eq!(find_land(-1, |_| true, |_| true), None);
        assert_eq!(find_land(-64, |_| true, |_| false), None);
    }

    /// The anchor goes preferred, then town hall, then centre.
    #[test]
    fn the_anchor_falls_back_from_the_preferred_block_to_the_town_hall_to_the_centre() {
        let centre = [0, 70, 0];
        assert_eq!(teleport_anchor(Some([1, 2, 3]), Some([4, 5, 6]), centre), [1, 2, 3]);
        assert_eq!(teleport_anchor(None, Some([4, 5, 6]), centre), [4, 5, 6]);
        assert_eq!(teleport_anchor(None, None, centre), centre);
    }

    /// A failed search means no teleport, not a teleport to the anchor.
    #[test]
    fn a_failed_landing_search_refuses_the_teleport() {
        let refused = colony_teleport_target(Some([1, 2, 3]), None, [0, 0, 0], |_| None);
        assert_eq!(refused, None);
    }

    /// A successful search moves away from the anchor, and the anchor is what was
    /// handed to it.
    #[test]
    fn the_search_starts_from_the_anchor_it_resolved() {
        let seen = std::cell::Cell::new([0, 0, 0]);
        let found = colony_teleport_target(None, Some([4, 5, 6]), [0, 70, 0], |anchor| {
            seen.set(anchor);
            Some([5, 5, 6])
        });
        assert_eq!(seen.get(), [4, 5, 6]);
        assert_eq!(found, Some([5, 5, 6]));
    }
}
