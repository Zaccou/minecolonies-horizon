//! `WorkerUtil`: which tool breaks a block, which way a worker looks, and where a
//! ladder column ends.
//!
//! Java reference: `core/util/WorkerUtil.java` (408 lines). Twelve of its
//! fourteen methods take a live `AbstractEntityCitizen`, a `Level` or a
//! `Blueprint` and act on it; those are the caller's. What is left is the
//! arithmetic and the tables, and three of them were **already re-implemented by
//! hand** in the modules that needed them — that is what this file replaces:
//!
//! | Rule | Duplicated at | Here |
//! | --- | --- | --- |
//! | last ladder of a shaft (`:340-350`) | `ai_gather/miner.rs:138`, over its own `GatherBlocks` | [`last_ladder`], over any predicate |
//! | `+0.5` on a spawn block (`:125-130`) | `manager_04/citizens.rs:74` (with `+0.1` on y) | [`MIDDLE_BLOCK_OFFSET`] |
//! | one tick of a turn (`:220-221`) | nowhere | [`update_rotation`] |
//!
//! The three call sites cannot be edited from here (each lives in another
//! agent's folder); the report names them, and `miner.rs` can drop its copy for
//! `last_ladder(|p| blocks.at(p) == GatherBlock::Ladder, pos)` the day someone
//! owns both files.
//!
//! # What is *not* here
//!
//! - `isPathBlock` (`:106-109`) is one tag lookup, `ModTags.pathingBlocks`, and
//!   the host answers it; there is no arithmetic to port.
//! - `isPartOfRecipe` (`:384-407`) and `hasTooManyExternalItemsInInv`'s recipe
//!   half need `IRecipeStorage` — item, damage, NBT and the cleaned inputs —
//!   which is [`crate::crafting`]'s business, not a utility's. Only the
//!   *counting* rule is here.
//! - `getBestToolForBlock`'s `IMateriallyTexturedBlock` branch (`:155-161`) needs
//!   Domum Ornamentum's block type; see `MANQUANT.md` §1 and [`super::schem`] for
//!   what *can* be decided about a DO block.

/// How far a spawn position sits past the block's corner, on `x` and `z`:
/// `WorkerUtil.MIDDLE_BLOCK_OFFSET` (`WorkerUtil.java:65`), the same `0.5D` as
/// `TeleportHelper.MIDDLE_BLOCK_OFFSET` (`TeleportHelper.java:26`) and the same
/// one `Constants.HALF_BLOCK` names (`Constants.java:48`). One value, three
/// spellings in Java; one constant here.
pub const MIDDLE_BLOCK_OFFSET: f64 = 0.5;

/// How far one tick turns a citizen towards where it wants to look, in degrees:
/// `CitizenConstants.ROTATION_MOVEMENT` (`api/util/constant/CitizenConstants.java:100`,
/// used at `WorkerUtil.java:220-221`).
pub const ROTATION_MOVEMENT: f64 = 30.0;

/// The sideways nudge that makes the client render a new rotation, in blocks:
/// `CitizenConstants.MOVE_MINIMAL` (`:80`, used at `WorkerUtil.java:223-227`).
pub const MOVE_MINIMAL: f64 = 0.001;

/// The most unrelated stacks a worker's inventory may hold before its job
/// refuses to work: the `3` of `WorkerUtil.java:369`.
pub const MAX_EXTERNAL_ITEMS: usize = 3;

/// How many rungs down [`last_ladder`] will look before giving up.
///
/// **This bound is the port's, not Java's.** Java recurses once per rung
/// (`:342-345`), so a caller that reports a ladder column of the whole world
/// height overflows the stack there and simply stops here. `512` is half a
/// kilometre of ladders, far past any shaft the miner digs (`MineColonies`
/// caps a level at `world_max_depth`), and it is the same bound the hand-written
/// copy in `ai_gather/miner.rs` chose.
pub const MAX_LADDER_SCAN: u32 = 512;

/// The tool a block wants. Java's `EquipmentTypeEntry` is a registry entry; this
/// enum is the six of them `WorkerUtil` can answer with, in the order
/// `getOrInitTestTools` names them (`:92-95`).
// exhaustive: the six answers `getBestToolForBlock` can give
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    /// `ModEquipmentTypes.shears` (`:146`), picked before any hardness test.
    Shears,
    Hoe,
    Shovel,
    Axe,
    Pickaxe,
    /// `ModEquipmentTypes.none` (`:170`): the block is broken by hand.
    None,
}

impl Tool {
    /// The registry path Java would log, `minecolonies:hoe` and friends.
    pub const fn path(self) -> &'static str {
        match self {
            Tool::Shears => "shears",
            Tool::Hoe => "hoe",
            Tool::Shovel => "shovel",
            Tool::Axe => "axe",
            Tool::Pickaxe => "pickaxe",
            Tool::None => "none",
        }
    }
}

/// The order `getOrInitTestTools` probes tools in (`WorkerUtil.java:92-95`):
/// **hoe, shovel, axe, pickaxe**. The order is load-bearing — `:151-166` returns
/// the *first* tool that is correct for the block, so a block both an axe and a
/// pickaxe could break gets the axe, and the table is what decides it.
pub const TEST_TOOL_ORDER: [Tool; 4] = [Tool::Hoe, Tool::Shovel, Tool::Axe, Tool::Pickaxe];

/// Which tool a block wants: `getBestToolForBlock` (`WorkerUtil.java:142-171`).
///
/// ```java
/// if (state.getBlock() instanceof IForgeShearable && building.hasModule(SettingsModule.class)
///     && building.getFirstModuleOccurance(SettingsModule.class).getSettingValueOrDefault(USE_SHEARS, true))
///     return ModEquipmentTypes.shears.get();
/// if (blockHardness > 0f) { for (final Tuple<...> tool : getOrInitTestTools()) { ... return tool.getA(); } }
/// return ModEquipmentTypes.none.get();
/// ```
///
/// The two host questions become the caller's: `shears_selected` is "this block
/// is shearable **and** the building's `USE_SHEARS` setting is on", and
/// `correct_tool_for_drops` is `ItemStack.isCorrectToolForDrops` for one of
/// [`TEST_TOOL_ORDER`]. Everything else is the order and the two guards.
///
/// **The guards, exactly.** Shears win over every tool, before hardness is even
/// looked at. A hardness of `0` (or less) short-circuits to [`Tool::None`]
/// *without probing*: a plant is not "hoe-correct", it is hand-broken. Without a
/// match, [`Tool::None`].
///
/// The `USE_SHEARS` default is `true`
/// (`getSettingValueOrDefault(USE_SHEARS, true)`, `:144`); a building with no
/// `SettingsModule` at all keeps the shears too, because both conjuncts fail
/// together.
pub fn best_tool(
    hardness: f32,
    shears_selected: bool,
    mut correct_tool_for_drops: impl FnMut(Tool) -> bool,
) -> Tool {
    if shears_selected {
        return Tool::Shears;
    }
    if hardness > 0.0 {
        for tool in TEST_TOOL_ORDER {
            if correct_tool_for_drops(tool) {
                return tool;
            }
        }
    }
    Tool::None
}

/// The harvest level a block demands: `getCorrectHarvestLevelForBlock`
/// (`WorkerUtil.java:179-198`).
///
/// ```java
/// int required = 0;
/// for (final Tier tier : TierSortingRegistry.getSortedTiers()) {
///     final TagKey<Block> tag = tier.getTag();
///     if (tag != null && target.is(tag)) { required = tier.getLevel(); break; }
/// }
/// if (required < 0 || target.getBlock() instanceof GlazedTerracottaBlock) return 0;
/// return required;
/// ```
///
/// The loop needs Forge's tier registry (`TierSortingRegistry`, `MANQUANT.md`
/// §2); the *answer* it produces is `required`, which is what this function
/// takes. The two corrections are Java's, and both are worth stating because
/// they look like bugs and are not: a **negative** tier level means "one below
/// wood" and is flattened to `0`, and **glazed terracotta** is flattened to `0`
/// whatever tier it was tagged with, because vanilla glazed terracotta is
/// pickaxe-mineable at level 0 and the tag lies about it.
///
/// A block in no tier tag is `required = 0` at the call site (Java's initial
/// value), so `0` in gives `0` out — the same answer.
pub fn correct_harvest_level(required: i32, glazed_terracotta: bool) -> i32 {
    if required < 0 || glazed_terracotta {
        0
    } else {
        required
    }
}

/// The y of the **lowest ladder** in the column at `pos`:
/// `getLastLadder` (`WorkerUtil.java:340-350`).
///
/// ```java
/// if (world.getBlockState(pos).getBlock().isLadder(...)) return getLastLadder(pos.below(), world);
/// else return pos.getY() + 1;
/// ```
///
/// Three behaviours worth naming, all Java's:
///
/// - the recursion goes **down** until the first block that is not a ladder, and
///   answers **that block's y plus one** — which is the lowest rung, because the
///   `+1` undoes the last step. A column of rungs at `60..=64` answers `60`, not
///   `65`;
/// - a **`pos` that is not a ladder** is the base case on its first call, so it
///   answers `pos.y + 1` — its own column contributes nothing and the answer is the
///   position itself raised by one. That is the "no ladder here" answer, and it is
///   how `miner.rs` tests for a shaft without a ladder;
/// - the search only ever goes **down**. A ladder above `pos` is not this column's.
///
/// Java recurses; the port loops, because the depth of the recursion is a fact
/// about the world and a fact from a file must not be able to overflow a stack.
/// [`MAX_LADDER_SCAN`] caps it, and a capped answer is the lowest rung it
/// **verified** — never a position it did not look at.
pub fn last_ladder(is_ladder: impl Fn([i32; 3]) -> bool, pos: [i32; 3]) -> i32 {
    let mut y = pos[1];
    let mut seen: u32 = 0;
    while is_ladder([pos[0], y, pos[2]]) {
        if seen == MAX_LADDER_SCAN {
            // The lowest rung this scan actually looked at.
            return y;
        }
        y -= 1;
        seen += 1;
    }
    y + 1
}

/// The rotation in degrees that has `from` look at `target`: the four-line
/// arithmetic of `faceBlock` (`WorkerUtil.java:213-221`).
///
/// ```java
/// final double xDifference = block.getX() - citizen.blockPosition().getX();
/// final double zDifference = block.getZ() - citizen.blockPosition().getZ();
/// final double yDifference = block.getY() - (citizen.blockPosition().getY() + citizen.getEyeHeight());
/// final double squareDifference = Math.sqrt(x * x + z * z);
/// final double intendedRotationYaw = (Math.atan2(z, x) * 180.0D / Math.PI) - 90.0;
/// final double intendedRotationPitch = -(Math.atan2(y, squareDifference) * 180.0D / Math.PI);
/// ```
///
/// Answer is `(yaw, pitch)` in Minecraft's convention, which is worth spelling
/// out because it is the one thing a reader gets wrong here: **yaw `0` faces
/// `+z` (south)**, `90` faces `-x` (west), `180` faces `-z` (north) and `-90`
/// faces `+x` (east); `pitch` is positive **down**. Neither angle is wrapped —
/// `faceBlock` hands both to [`update_rotation`], which wraps the *difference*
/// instead, and that is what makes the turn take the short way round.
///
/// `eye_height` is the citizen's (`getEyeHeight()`, `1.62` for a player-sized
/// entity); a caller that has none passes `0.0` and gets the feet-to-block
/// angle, which is what the arithmetic says.
pub fn face_rotation(from: [i32; 3], eye_height: f64, target: [i32; 3]) -> (f64, f64) {
    let x = f64::from(target[0] - from[0]);
    let z = f64::from(target[2] - from[2]);
    let y = f64::from(target[1]) - (f64::from(from[1]) + eye_height);
    let square = (x * x + z * z).sqrt();
    let yaw = z.atan2(x).to_degrees() - 90.0;
    let pitch = -(y.atan2(square).to_degrees());
    (yaw, pitch)
}

/// The sideways nudge `faceBlock` applies so the client renders the new rotation
/// (`WorkerUtil.java:223-227`): `+MOVE_MINIMAL` past the block, `-MOVE_MINIMAL`
/// on it or short of it, **on each axis independently**.
///
/// ```java
/// final double goToX = xDifference > 0 ? MOVE_MINIMAL : -MOVE_MINIMAL;
/// final double goToZ = zDifference > 0 ? MOVE_MINIMAL : -MOVE_MINIMAL;
/// ```
///
/// The `>` is strict, so a difference of exactly `0` gives the negative nudge —
/// the one case a reader would guess the other way. The y difference takes no
/// part.
pub fn minimal_move(from: [i32; 3], target: [i32; 3]) -> (f64, f64) {
    let nudged = |difference: i32| {
        if difference > 0 {
            MOVE_MINIMAL
        } else {
            -MOVE_MINIMAL
        }
    };
    (
        nudged(target[0] - from[0]),
        nudged(target[2] - from[2]),
    )
}

/// Vanilla's `Mth.wrapDegrees`: fold an angle into `[-180, 180)`.
///
/// Not a MineColonies function — it is `net.minecraft.util.Mth`, called by
/// `EntityUtils.updateRotation` (`api/util/EntityUtils.java:115`) and not present
/// in `minecolonies-src`, so there is no line to cite beyond that call. The three
/// steps are vanilla's, in vanilla's order, because the order decides the edges:
///
/// ```text
/// f = value % 360        // keeps the sign of `value`, like Java's `%` on doubles
/// if (f >= 180) f -= 360
/// if (f < -180) f += 360
/// ```
///
/// which makes `180` answer `-180` and `-180` answer `-180`, and `-190` answer
/// `170`.
pub fn wrap_degrees(value: f64) -> f64 {
    let mut wrapped = value % 360.0;
    if wrapped >= 180.0 {
        wrapped -= 360.0;
    }
    if wrapped < -180.0 {
        wrapped += 360.0;
    }
    wrapped
}

/// One tick of a turn towards an intended angle:
/// `EntityUtils.updateRotation` (`api/util/EntityUtils.java:113-128`), which
/// `faceBlock` calls twice (`WorkerUtil.java:220-221`).
///
/// ```java
/// double wrappedAngle = Mth.wrapDegrees(intendedRotation - currentRotation);
/// if (wrappedAngle > maxIncrement) wrappedAngle = maxIncrement;
/// if (wrappedAngle < -maxIncrement) wrappedAngle = -maxIncrement;
/// return currentRotation + wrappedAngle;
/// ```
///
/// The result is **not** wrapped back into `[-180, 180)`: `current` may therefore
/// drift past `180` over many ticks, and `170 + 20 = 190` in the test is Java's
/// answer, not a mistake. A caller that wants the angle at the end of the turn
/// uses [`wrap_degrees`] on the result.
pub fn update_rotation(current: f64, intended: f64, max_increment: f64) -> f64 {
    let mut wrapped = wrap_degrees(intended - current);
    if wrapped > max_increment {
        wrapped = max_increment;
    }
    if wrapped < -max_increment {
        wrapped = -max_increment;
    }
    current + wrapped
}

/// Does the inventory hold more than [`MAX_EXTERNAL_ITEMS`] stacks that are not
/// part of the recipe: `hasTooManyExternalItemsInInv` (`WorkerUtil.java:360-376`).
///
/// ```java
/// int count = 0;
/// for (int i = 0; i < inv.getSlots(); i++) {
///     final ItemStack stack = inv.getStackInSlot(i);
///     if (!stack.isEmpty() && !isPartOfRecipe(stack, currentRecipeStorage)) {
///         count++;
///         if (count > 3) return true;
///     }
/// }
/// return false;
/// ```
///
/// Takes one `bool` per **non-empty** slot — `true` when the stack belongs to the
/// recipe — so the empty/occupied test and the recipe test stay where the items
/// are. The boundary is the whole point: **three** unrelated stacks are fine,
/// the fourth refuses, and `3` in the constant is what makes it so.
pub fn has_too_many_external_items(parts_of_recipe: impl Iterator<Item = bool>) -> bool {
    let mut external = 0usize;
    for is_part in parts_of_recipe {
        if !is_part {
            external += 1;
            if external > MAX_EXTERNAL_ITEMS {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shaft of five rungs at `y = 60..=64` inside the column `(3, *, 7)`.
    fn shaft(rungs: std::ops::RangeInclusive<i32>) -> impl Fn([i32; 3]) -> bool {
        move |pos: [i32; 3]| pos[0] == 3 && pos[2] == 7 && rungs.contains(&pos[1])
    }

    #[test]
    fn the_lowest_ladder_is_the_answer() {
        // Rungs at 60..=64: the recursion stops on 59 and answers 60.
        assert_eq!(last_ladder(shaft(60..=64), [3, 64, 7]), 60);
        // A single rung under the feet answers itself, not one above.
        assert_eq!(last_ladder(shaft(64..=64), [3, 64, 7]), 64);
    }

    /// Java's `else` branch: no ladder at `pos` answers `pos.y + 1`, never
    /// `pos.y` — even when the column below is full of ladders, because the
    /// recursion never starts.
    #[test]
    fn a_position_that_is_not_a_ladder_answers_one_above_itself() {
        assert_eq!(last_ladder(shaft(60..=60), [3, 64, 7]), 65);
        assert_eq!(last_ladder(shaft(60..=64), [0, 64, 7]), 65);
    }

    /// A column that reaches `y = 0` answers its lowest rung, and the loop needs no
    /// floor to stop at.
    #[test]
    fn a_ladder_column_that_reaches_zero_answers_its_lowest_rung() {
        assert_eq!(last_ladder(shaft(-3..=0), [3, 0, 7]), -3);
    }

    /// The scan bound is finite: an endless column of ladders answers, it does
    /// not hang. Java would have recursed until the stack died. The answer is the
    /// lowest rung the scan **looked at**, never a position it skipped.
    #[test]
    fn an_endless_ladder_column_stops_at_the_scan_bound() {
        let endless = |pos: [i32; 3]| pos[1] > i32::MIN / 2;
        // Rungs `0` down to `-512` are 513 verified rungs, and `-512` is the
        // lowest one; `-513` is never claimed.
        assert_eq!(last_ladder(endless, [0, 0, 0]), -512);
        assert_eq!(-(MAX_LADDER_SCAN as i32), -512);
    }

    /// Shears win before hardness is read, even on a block no tool breaks.
    #[test]
    fn shears_win_over_every_tool_and_over_hardness() {
        let probed = std::cell::Cell::new(0);
        let tool = best_tool(0.0, true, |_| {
            probed.set(probed.get() + 1);
            false
        });
        assert_eq!(tool, Tool::Shears);
        assert_eq!(probed.get(), 0, "shears must not probe the tool table");
    }

    /// The probe order is hoe, shovel, axe, pickaxe — not the alphabet, not the
    /// enum's declaration order.
    #[test]
    fn the_first_correct_tool_in_the_table_wins() {
        assert_eq!(best_tool(1.0, false, |tool| tool == Tool::Pickaxe), Tool::Pickaxe);
        assert_eq!(
            best_tool(1.0, false, |tool| tool == Tool::Axe || tool == Tool::Pickaxe),
            Tool::Axe
        );
        assert_eq!(
            best_tool(1.0, false, |tool| tool == Tool::Hoe || tool == Tool::Shovel),
            Tool::Hoe
        );
    }

    /// Zero hardness never probes: a plant is hand-broken, and a negative
    /// hardness is the same case.
    #[test]
    fn zero_or_negative_hardness_answers_none_without_probing() {
        let probed = std::cell::Cell::new(0);
        let mut count = |_tool: Tool| {
            probed.set(probed.get() + 1);
            true
        };
        assert_eq!(best_tool(0.0, false, &mut count), Tool::None);
        assert_eq!(best_tool(-1.0, false, &mut count), Tool::None);
        assert_eq!(probed.get(), 0);
    }

    /// No tool correct ⇒ `none`, not `pickaxe` and not a refusal.
    #[test]
    fn a_block_no_tool_breaks_answers_none() {
        assert_eq!(best_tool(2.0, false, |_| false), Tool::None);
    }

    #[test]
    fn a_negative_tier_level_is_flattened_to_zero() {
        assert_eq!(correct_harvest_level(-1, false), 0);
        assert_eq!(correct_harvest_level(i32::MIN, false), 0);
    }

    #[test]
    fn glazed_terracotta_is_flattened_to_zero_whatever_its_tier() {
        assert_eq!(correct_harvest_level(0, true), 0);
        assert_eq!(correct_harvest_level(3, true), 0);
    }

    #[test]
    fn a_plain_tier_is_answered_unchanged() {
        assert_eq!(correct_harvest_level(0, false), 0);
        assert_eq!(correct_harvest_level(4, false), 4);
    }

    /// East is the `-90` yaw every Minecraft player knows.
    #[test]
    fn looking_east_is_a_yaw_of_minus_ninety() {
        let (yaw, _) = face_rotation([0, 64, 0], 1.62, [10, 64, 0]);
        assert!((yaw - -90.0).abs() < 1e-9);
    }

    /// North is yaw `180` — the sign of the answer surprises, because yaw `0`
    /// faces south. The pitch is the eye-to-block slope.
    #[test]
    fn looking_north_is_a_yaw_of_half_a_turn_with_a_downward_pitch() {
        let (yaw, pitch) = face_rotation([0, 64, 0], 1.62, [0, 63, -10]);
        assert!((yaw + 180.0).abs() < 1e-9);
        // `WorkerUtil.java:219`: -atan2(63 - (64 + 1.62), 10) in degrees, i.e.
        // -atan2(-2.62, 10) = +14.6815, not 14.6924.
        assert!((pitch - 14.6815).abs() < 0.01);
    }

    /// And yaw `0` is south (`+z`), which is the anchor of the whole convention.
    #[test]
    fn looking_south_is_a_yaw_of_zero() {
        let (yaw, _) = face_rotation([0, 64, 0], 1.62, [0, 64, 10]);
        assert!(yaw.abs() < 1e-9);
    }

    /// Eye height moves the pitch and leaves the yaw alone.
    #[test]
    fn the_eye_height_only_changes_the_pitch() {
        let (yaw_a, pitch_a) = face_rotation([0, 64, 0], 0.0, [5, 64, 5]);
        let (yaw_b, pitch_b) = face_rotation([0, 64, 0], 1.62, [5, 64, 5]);
        assert!((yaw_a - yaw_b).abs() < 1e-9);
        assert!((pitch_a - pitch_b).abs() > 1.0);
    }

    /// The nudge is per axis and the zero difference takes the negative side.
    #[test]
    fn the_minimal_move_picks_a_side_per_axis_with_zero_on_the_negative_side() {
        assert_eq!(minimal_move([0, 64, 0], [1, 64, 1]), (MOVE_MINIMAL, MOVE_MINIMAL));
        assert_eq!(minimal_move([0, 64, 0], [-1, 64, -1]), (-MOVE_MINIMAL, -MOVE_MINIMAL));
        assert_eq!(minimal_move([0, 64, 0], [0, 64, 0]), (-MOVE_MINIMAL, -MOVE_MINIMAL));
        assert_eq!(minimal_move([0, 64, 0], [1, 64, -1]), (MOVE_MINIMAL, -MOVE_MINIMAL));
    }

    #[test]
    fn wrap_degrees_folds_into_the_half_open_interval() {
        assert_eq!(wrap_degrees(0.0), 0.0);
        assert_eq!(wrap_degrees(90.0), 90.0);
        assert_eq!(wrap_degrees(180.0), -180.0);
        assert_eq!(wrap_degrees(-180.0), -180.0);
        assert_eq!(wrap_degrees(190.0), -170.0);
        assert_eq!(wrap_degrees(-190.0), 170.0);
        assert_eq!(wrap_degrees(720.0), 0.0);
    }

    /// A turn of `90` at `30` degrees a tick takes three ticks, not one.
    #[test]
    fn a_turn_is_capped_at_the_rotation_movement() {
        let mut yaw = 0.0;
        for _ in 0..3 {
            yaw = update_rotation(yaw, 90.0, ROTATION_MOVEMENT);
        }
        assert_eq!(yaw, 90.0);
        assert_eq!(update_rotation(0.0, 90.0, ROTATION_MOVEMENT), 30.0);
    }

    /// A small difference is followed exactly, with no cap applied.
    #[test]
    fn a_small_difference_is_followed_exactly() {
        assert_eq!(update_rotation(0.0, 10.0, ROTATION_MOVEMENT), 10.0);
        assert_eq!(update_rotation(0.0, -10.0, ROTATION_MOVEMENT), -10.0);
    }

    /// The short way round: `170` to `-170` is `+20`, not `-340`.
    #[test]
    fn the_turn_crosses_the_pole_the_short_way() {
        assert_eq!(update_rotation(170.0, -170.0, ROTATION_MOVEMENT), 190.0);
        assert_eq!(update_rotation(-170.0, 170.0, ROTATION_MOVEMENT), -190.0);
    }

    /// Three unrelated stacks are tolerated; the fourth is the refusal.
    #[test]
    fn three_external_stacks_are_fine_and_the_fourth_is_not() {
        assert!(!has_too_many_external_items(
            [false, false, false].into_iter()
        ));
        assert!(has_too_many_external_items(
            [false, false, false, false].into_iter()
        ));
    }

    /// Stacks that belong to the recipe never count, however many there are.
    #[test]
    fn recipe_stacks_do_not_count() {
        assert!(!has_too_many_external_items(
            std::iter::repeat_n(true, 40)
        ));
        assert!(has_too_many_external_items(
            std::iter::repeat_n(true, 3).chain(std::iter::repeat_n(false, 4))
        ));
    }

    /// An empty inventory has no too many of anything.
    #[test]
    fn an_empty_inventory_passes() {
        assert!(!has_too_many_external_items(std::iter::empty()));
    }
}
