//! The bounds a message's **receiver** checks, which the wire never checks.
//!
//! [`crate::network`] refuses a field that is not the NBT kind its reader wanted, a
//! name it cannot resolve, a text past 32 767 units. What it cannot refuse is a value
//! that is a perfectly good `int` and still wrong: a field radius of 40, a quantity of
//! zero, a miner level that indexes past the array of levels, a colour whose ordinal is
//! off the end of `ChatFormatting.values()`. Java checks those **inside `onExecute`**,
//! after the packet has been accepted, and the check is one of three things:
//!
//! 1. **A clamp**, done silently: `Math.min(MAX_CITIZEN_LEVEL, Math.max(level + delta, 1))`
//!    (`CitizenSkillHandler.java:176`), `Math.min(radius, MAX_RANGE)`
//!    (`FarmField.java:211`).
//! 2. **A drop**, done with a log line and a `return`, so the sender is told nothing:
//!    `if (quantity <= 0) { Log…("quantity below 0"); return; }`
//!    (`TransferItemsRequestMessage.java:89-93`).
//! 3. **Nothing at all**, where the value then indexes a table: `levels.get(level)`
//!    (`MinerLevelManagementModule.java:277`) throws on a level the building does not
//!    have, and `player.getInventory().removeItem(slot, 1)` with the `-1` a failed
//!    search returns (`UpgradeWarehouseMessage.java:59-62`) throws for a player with no
//!    emerald block.
//!
//! **This module ports the rules, not the messages.** Each function here is the check
//! Java makes, named, with its constants and its line, and it returns a
//! [`Refusal`] when a *field* is out of bounds and a [`Gate`] when the world says no
//! (the module is full, the player has no gold). Nothing here reads a file, spawns
//! anything or takes a lock; every function is pure, which is why it can be tested with
//! numbers.
//!
//! **Where a rule is stricter than Java, it says so, and why.** Two of them are:
//! [`rank_kind`] refuses a `rankType` Java would silently treat as "plain"
//! (`PermissionsMessage.java:714-720`), and [`transfer_to_citizen`] refuses a negative
//! count in creative mode, which Java would write into a stack
//! (`TransferItemsToCitizenRequestMessage.java:120-126`). Both are argued on the
//! function.

use crate::network::building_modules::Face;
use crate::network::Refusal;

/// `FarmField.MAX_RANGE` — the total a scarecrow's four radii may add up to
/// (`FarmField.java:40`), which `FarmFieldPlotResizeMessage.java:74` calls the bound.
pub const MAX_FIELD_RANGE: i32 = 20;

/// `BuildingBarracks.SPIES_GOLD_COST` — what hiring spies costs, in gold ingots
/// (`BuildingBarracks.java:65`, spent by `HireSpiesMessage.java:38-41`).
pub const SPIES_GOLD_COST: i32 = 5;

/// `CitizenConstants.MAX_CITIZEN_LEVEL` — the top of a skill, and the floor is `1`
/// (`CitizenConstants.java:181`, clamped by `CitizenSkillHandler.java:176`).
pub const MAX_CITIZEN_LEVEL: i32 = 99;

/// `Constants.STACKSIZE` — the gold nuggets one stack holds, which
/// `TeleportToColonyMessage.java:99-113` cuts its payout into.
pub const STACKSIZE: i32 = 64;

/// `BlockParticleEffectMessage.BREAK_BLOCK` — the `side` that means "the block broke"
/// rather than "this face cracked" (`BlockParticleEffectMessage.java:22,76-83`).
pub const BREAK_BLOCK: i32 = -1;

/// `TownHallRenameMessage.MAX_NAME_LENGTH` — a colony name is at most this long, and a
/// longer one is cut to the length below it (`TownHallRenameMessage.java:17-18,38,56`).
pub const MAX_NAME_LENGTH: usize = 25;

/// The distance a `DirectPlace` may reach: `player.blockPosition().distSqr(pos) > 5 * 5`
/// refuses (`DirectPlaceMessage.java:117`).
pub const PLACE_REACH_SQUARED: i32 = 25;

/// A refusal that is not about the wire: the value parsed, and the world says no.
///
/// Separate from [`Refusal`] on purpose. `Refusal`'s five variants all describe a
/// *malformed message* — a field that never arrived, the wrong kind, a name unknown to
/// this port, a text too long, a number outside its documented range — and a log line
/// that reports one as the other sends a reader looking for a codec bug that is not
/// there. These are the refusals Java answers with a chat message, a log line, or an
/// exception it does not catch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: closed on purpose, one variant per refusal a message's receiver has
pub enum Gate {
    /// The UUID names no player of this colony — Java dereferences the null
    /// (`PermissionsMessage.java:567-568`).
    UnknownPlayer,
    /// The module already holds as many citizens as it takes
    /// (`AssignUnassignMessage.java:119`: `!module.isFull()`).
    ModuleFull,
    /// The citizen already lives here (`AssignUnassignMessage.java:119`:
    /// `!building.equals(citizen.getHomeBuilding())`).
    AlreadyAssigned,
    /// The building at that position is not a miner (`GuardSetMinePosMessage.java:88-91`).
    NoSuchMiner,
    /// The sender cannot pay (`HireSpiesMessage.java:38`).
    NotEnoughGold { need: i32, has: i32 },
    /// The sender is not in creative mode, and this message is a debug tool
    /// (`AdjustSkillCitizenMessage.java:100-105`).
    NotCreative,
    /// The sender holds no emerald block, which Java does not check before spending it
    /// (`UpgradeWarehouseMessage.java:57-62`).
    NoEmeraldBlock,
    /// The target position is not empty (`DirectPlaceMessage.java:117`).
    NotAir,
    /// The block there is not a hut (`DirectPlaceMessage.java:118`).
    NotAHut,
    /// The stack's block state is not the one being placed
    /// (`DirectPlaceMessage.java:117`).
    StateMismatch,
    /// The sender holds neither the rank nor the identity this needs: Java's third
    /// branch simply does nothing (`PermissionsMessage.java:570-574`).
    NotAllowed,
}

/// One side of a scarecrow's field: the four radii after the resize, or the refusal.
///
/// Java's rule (`FarmFieldPlotResizeMessage.java:71-79`) is *not* "each side is at most
/// twenty". It is:
///
/// - a negative size is dropped (`:74`);
/// - a size that **grows** the field is dropped when the four radii would then add up
///   to more than [`MAX_FIELD_RANGE`] (`:74`);
/// - a size that **shrinks** it is always accepted, whatever the sum was.
///
/// That asymmetry is the whole rule, and it is why the check needs the other three
/// radii: a message that carries only `size` cannot be validated on its own. The
/// clamp `Math.min(radius, MAX_RANGE)` in `FarmField.setRadius` (`FarmField.java:211`)
/// then applies a second, weaker bound on top, which this function also applies so
/// that the value returned is the value stored.
pub fn field_plot_radius(
    current: [i32; 4],
    direction: Face,
    size: i32,
) -> Result<[i32; 4], Refusal> {
    if size < 0 {
        return Err(Refusal::Number {
            key: "size",
            min: 0,
            max: i64::from(MAX_FIELD_RANGE),
        });
    }
    let index = direction.index() as usize;
    let previous = current[index];
    let sum: i32 = current.iter().sum::<i32>() - previous + size;
    if size > previous && sum > MAX_FIELD_RANGE {
        return Err(Refusal::Size {
            key: "size",
            max: MAX_FIELD_RANGE as usize,
        });
    }
    let mut radii = current;
    radii[index] = size.min(MAX_FIELD_RANGE);
    Ok(radii)
}

/// How many items a `TransferItemsRequest` may move: strictly more than zero
/// (`TransferItemsRequestMessage.java:89-93` drops the message otherwise).
pub fn transfer_quantity(quantity: i32) -> Result<i32, Refusal> {
    if quantity <= 0 {
        return Err(Refusal::Number {
            key: "quantity",
            min: 1,
            max: i64::from(i32::MAX),
        });
    }
    Ok(quantity)
}

/// How many items may be handed to a citizen: positive unless the sender is in
/// creative mode (`TransferItemsToCitizenRequestMessage.java:119-124`).
///
/// **Stricter than Java on one value**: Java's check is `quantity <= 0 && !isCreative`,
/// then `itemStack.setCount(quantity)` (`:126`), so a creative sender can put a
/// negative count into an `ItemStack` — a stack that does not exist. The port refuses
/// it: creative mode means "you need not own it", not "a count may be negative".
pub fn transfer_to_citizen(quantity: i32, creative: bool) -> Result<i32, Refusal> {
    if quantity < 0 || (quantity == 0 && !creative) {
        return Err(Refusal::Number {
            key: "quantity",
            min: if creative { 0 } else { 1 },
            max: i64::from(i32::MAX),
        });
    }
    Ok(quantity)
}

/// The two counts a post box request carries: what was asked for, and the minimum the
/// request will accept (`PostBoxRequestMessage.java:76-79`).
///
/// Java: `minCount = deliverAvailable ? 1 : reqQuantity`, then a `Stack(item, req,
/// minCount)`. A request for zero items is refused here; Java builds the `Stack` and
/// leaves the request system to reject it later.
pub fn post_box_counts(req_quantity: i32, deliver_available: bool) -> Result<(i32, i32), Refusal> {
    if req_quantity <= 0 {
        return Err(Refusal::Number {
            key: "reqQuantity",
            min: 1,
            max: i64::from(i32::MAX),
        });
    }
    let minimum = if deliver_available { 1 } else { req_quantity };
    Ok((req_quantity, minimum))
}

/// Whether the sender can pay for spies, and how much is taken
/// (`HireSpiesMessage.java:38-41`).
pub fn spies_cost(gold_ingots: i32) -> Result<i32, Gate> {
    if gold_ingots < SPIES_GOLD_COST {
        return Err(Gate::NotEnoughGold {
            need: SPIES_GOLD_COST,
            has: gold_ingots,
        });
    }
    Ok(SPIES_GOLD_COST)
}

/// The gold nuggets a gate house is given back after a paid teleport
/// (`TeleportToColonyMessage.java:94-116`).
///
/// Java pays **half** the cost into the gate's inventory (`:98`) and cuts it into
/// stacks of [`STACKSIZE`]. The two branches do not agree, and the difference is a real
/// loss: under one stack the whole half is given (`:99-102`), above it Java loops
/// `output / STACKSIZE` times (`:105`) and **drops the remainder**. Half of `259` is
/// `129`, `129 / 64 = 2`, so `128` nuggets are paid and one is destroyed. This function
/// returns what Java actually gives, not what it should give, so that a caller can see
/// the difference; `RAPPORT.md` records it as a defect found on the way.
pub fn teleport_payout(cost: i32) -> Vec<i32> {
    if cost <= 0 {
        return Vec::new();
    }
    let half = cost / 2;
    if half <= STACKSIZE {
        return if half > 0 { vec![half] } else { Vec::new() };
    }
    let stacks = half / STACKSIZE;
    let mut paid = Vec::new();
    let mut left = half;
    for _ in 0..stacks {
        let quantity = STACKSIZE.min(left);
        paid.push(quantity);
        left -= quantity;
    }
    paid
}

/// What a `BlockParticleEffect`'s `side` asks for: the block broke, or this face
/// cracked (`BlockParticleEffectMessage.java:76-83`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the message carries one of these two acts, nothing else
pub enum Crack {
    /// The whole block (`side == BREAK_BLOCK`): `particleEngine.destroy(pos, state)`.
    Destroy,
    /// One face: `particleEngine.crack(pos, Direction.from3DDataValue(side))`.
    Face(Face),
}

/// Reads `side` into the act it names.
///
/// Java's `else` branch calls `Direction.from3DDataValue(side)`, which throws for
/// anything outside `0..=5` (`BlockParticleEffectMessage.java:82`). Vanilla's
/// `Direction` has six values and only four of them are horizontal, so the four
/// vertical faces (down and up are `0` and `1`) are folded onto the horizontal one
/// that shares their axis — the port keeps the four `Face` values it has
/// ([`Face`]), and refuses the rest rather than pretend.
pub fn crack_target(side: i32) -> Result<Crack, Refusal> {
    match side {
        BREAK_BLOCK => Ok(Crack::Destroy),
        2 => Ok(Crack::Face(Face::North)),
        3 => Ok(Crack::Face(Face::South)),
        4 => Ok(Crack::Face(Face::West)),
        5 => Ok(Crack::Face(Face::East)),
        _ => Err(Refusal::Number {
            key: "side",
            min: i64::from(BREAK_BLOCK),
            max: 5,
        }),
    }
}

/// The two step numbers a stream particle walks, `stage` and `maxStage`
/// (`StreamParticleEffectMessage.java:136-146`).
///
/// Java divides by `maxStage` three times (`:136-138`) and by `maxStage / 3.0`
/// (`:140`). A `maxStage` of `0` therefore makes `xDif`, `yDif` and `zDif` infinite and
/// `curve` zero: the loop still runs, and every particle lands at `NaN`. The message
/// carries an `int` from a sender, so the port refuses it at the door.
pub fn stream_window(stage: i32, max_stage: i32) -> Result<[i32; 2], Refusal> {
    if max_stage < 1 {
        return Err(Refusal::Number {
            key: "maxStage",
            min: 1,
            max: i64::from(i32::MAX),
        });
    }
    if stage < 0 {
        return Err(Refusal::Number {
            key: "stage",
            min: 0,
            max: i64::from(max_stage),
        });
    }
    // `for (step = Math.max(0, stage - 1); step <= Math.min(maxStage, stage + 1); step++)`
    Ok([(stage - 1).max(0), (stage + 1).min(max_stage)])
}

/// `player.blockPosition().distSqr(pos)` against the five-block reach of a direct
/// placement (`DirectPlaceMessage.java:117`).
///
/// Java compares squared integers, so the bound is `25` exactly and a position at
/// `[5, 0, 0]` is allowed while `[5, 1, 0]` (26) is not.
pub fn direct_place_reach(dx: i32, dy: i32, dz: i32) -> Result<i32, Refusal> {
    let distance = i64::from(dx) * i64::from(dx)
        + i64::from(dy) * i64::from(dy)
        + i64::from(dz) * i64::from(dz);
    if distance > i64::from(PLACE_REACH_SQUARED) {
        return Err(Refusal::Number {
            key: "pos",
            min: 0,
            max: i64::from(PLACE_REACH_SQUARED),
        });
    }
    Ok(distance as i32)
}

/// The three world checks a direct placement makes before it writes anything
/// (`DirectPlaceMessage.java:117-121`).
pub fn direct_place_target(is_air: bool, is_hut: bool, state_matches: bool) -> Result<(), Gate> {
    if !is_air {
        return Err(Gate::NotAir);
    }
    if !is_hut {
        return Err(Gate::NotAHut);
    }
    if !state_matches {
        return Err(Gate::StateMismatch);
    }
    Ok(())
}

/// What a rank's `rankType` asks for (`PermissionsMessage.java:703-721`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: Java's three cases, of which the third is a catch-all
pub enum RankKind {
    /// `0`: a colony manager.
    Manager,
    /// `1`: hostile.
    Hostile,
    /// `2`: neither.
    Plain,
}

/// Reads `rankType` into the rank it describes.
///
/// Java's `switch` has `case 0`, `case 1` and `default` — so `rankType = 7` silently
/// makes the rank neither manager nor hostile (`:714-720`). The port refuses anything
/// above `2` instead: an out-of-range field means the sender and this port disagree
/// about what a rank is, and demoting a rank on that disagreement is the kind of silent
/// change that costs a colony its officers.
pub fn rank_kind(rank_type: i32) -> Result<RankKind, Refusal> {
    match rank_type {
        0 => Ok(RankKind::Manager),
        1 => Ok(RankKind::Hostile),
        2 => Ok(RankKind::Plain),
        _ => Err(Refusal::Number {
            key: "rankType",
            min: 0,
            max: 2,
        }),
    }
}

/// A skill after an increment: clamped to `1..=MAX_CITIZEN_LEVEL`
/// (`CitizenSkillHandler.java:173-177`), and refused for a sender who is not in
/// creative mode (`AdjustSkillCitizenMessage.java:100-105`).
pub fn skill_increment(current: i32, quantity: i32, creative: bool) -> Result<i32, Gate> {
    if !creative {
        return Err(Gate::NotCreative);
    }
    Ok((current + quantity).clamp(1, MAX_CITIZEN_LEVEL))
}

/// The miner level a repair may name, as an index into the building's levels
/// (`MinerRepairLevelMessage.java:52-55` → `MinerLevelManagementModule.java:266-280`).
///
/// Java passes the `int` straight to `levels.get(level)`, which throws
/// `IndexOutOfBoundsException` for a level the shaft does not have yet — reachable from
/// a client whose copy of the shaft is one level behind the server's.
pub fn miner_repair_level(level: i32, levels: usize) -> Result<usize, Refusal> {
    if level < 0 || level as usize >= levels {
        return Err(Refusal::Number {
            key: "level",
            min: 0,
            max: levels.saturating_sub(1) as i64,
        });
    }
    Ok(level as usize)
}

/// The two recipe indexes a priority change swaps (`ChangeRecipePriorityMessage.java:95-102`).
///
/// Java calls `module.switchOrder(recipeLocation, recipeLocation ± 1, fullMove)` and
/// lets the module decide what to do with an index at the edge. This is the same check
/// made explicit: the swap has two valid ends and nothing else.
pub fn recipe_swap(index: i32, up: bool, len: usize) -> Result<(usize, usize), Refusal> {
    let bound = len.saturating_sub(1) as i64;
    if index < 0 || index as usize >= len {
        return Err(Refusal::Number {
            key: "recipeLocation",
            min: 0,
            max: bound,
        });
    }
    let other = if up { index - 1 } else { index + 1 };
    if other < 0 || other as usize >= len {
        return Err(Refusal::Number {
            key: "recipeLocation",
            min: 0,
            max: bound,
        });
    }
    Ok((index as usize, other as usize))
}

/// The slot an emerald block is taken from when a warehouse grows
/// (`UpgradeWarehouseMessage.java:57-63`).
///
/// Java searches for the block and then removes it without checking that the search
/// found one: `findFirstSlotInItemHandlerWith` answers `-1`, and
/// `player.getInventory().removeItem(-1, 1)` throws on a player who has no emerald
/// block — which is every player who clicks the button before paying.
pub fn warehouse_upgrade_slot(slot: i32) -> Result<usize, Gate> {
    if slot < 0 {
        return Err(Gate::NoEmeraldBlock);
    }
    Ok(slot as usize)
}

/// Which citizen a hire or fire acts on, and what the module does with it
/// (`HireFireMessage.java:83-97`).
pub fn hire_fire(hire: bool, module_full: bool) -> Result<bool, Gate> {
    if hire && module_full {
        return Err(Gate::ModuleFull);
    }
    Ok(hire)
}

/// Whether an assignment to a home or a job module may happen
/// (`AssignUnassignMessage.java:119`).
pub fn assign_module(assign: bool, module_full: bool, already_assigned: bool) -> Result<(), Gate> {
    if assign && module_full {
        return Err(Gate::ModuleFull);
    }
    if assign && already_assigned {
        return Err(Gate::AlreadyAssigned);
    }
    Ok(())
}

/// The mine position a guard keeps: the one the message names, or the one it already
/// had when the message names none (`GuardSetMinePosMessage.java:79-91`).
///
/// Java then checks that the building at that position is a miner and drops the message
/// if it is not — so a guard told to guard a house keeps guarding its old mine.
pub fn guard_mine_pos(
    named: Option<[i32; 3]>,
    current: [i32; 3],
    is_miner: bool,
) -> Result<[i32; 3], Gate> {
    let position = named.unwrap_or(current);
    if !is_miner {
        return Err(Gate::NoSuchMiner);
    }
    Ok(position)
}

/// Which player a permission removal may act on (`PermissionsMessage.java:551-575`).
///
/// Java's conditions are three, and one of them dereferences a lookup that can be null
/// (`:567`: `getPlayers().get(playerID)` then `.getRank()`), so removing an unknown UUID
/// throws instead of being refused. This function is the same three conditions with the
/// fourth case — nobody by that id — named.
pub fn remove_player_allowed(
    target_is_sender: bool,
    target_known: bool,
    target_hostile: bool,
    can_edit: bool,
    sender_is_manager: bool,
) -> Result<(), Gate> {
    if !target_known {
        return Err(Gate::UnknownPlayer);
    }
    if target_is_sender {
        return Ok(());
    }
    if target_hostile && can_edit {
        return Ok(());
    }
    if !target_hostile && can_edit && sender_is_manager {
        return Ok(());
    }
    Err(Gate::NotAllowed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE: [i32; 4] = [5, 5, 5, 5];

    #[test]
    fn a_field_grows_while_the_four_radii_stay_under_twenty() {
        assert_eq!(
            field_plot_radius(SQUARE, Face::North, 5),
            Ok([5, 5, 5, 5]),
            "5 + 5 + 5 + 5 is 20, which is the bound itself"
        );
        assert_eq!(
            field_plot_radius([1, 1, 1, 1], Face::East, 17),
            Ok([1, 1, 1, 17]),
            "1 + 1 + 1 + 17 is 20"
        );
    }

    #[test]
    fn a_field_that_would_pass_twenty_is_refused() {
        assert_eq!(
            field_plot_radius(SQUARE, Face::North, 6),
            Err(Refusal::Size {
                key: "size",
                max: 20
            }),
            "6 + 5 + 5 + 5 is 21"
        );
    }

    #[test]
    fn shrinking_is_always_allowed_even_above_the_bound() {
        // Java only tests the sum when the side grows (`:74`), so a field already over
        // the bound can be shrunk but not grown.
        assert_eq!(
            field_plot_radius([9, 9, 9, 9], Face::West, 8),
            Ok([9, 8, 9, 9])
        );
    }

    #[test]
    fn a_negative_radius_is_refused() {
        assert_eq!(
            field_plot_radius(SQUARE, Face::South, -1),
            Err(Refusal::Number {
                key: "size",
                min: 0,
                max: 20
            })
        );
    }

    #[test]
    fn the_stored_radius_is_clamped_like_set_radius() {
        // `FarmField.setRadius` clamps, so even an accepted 40 is stored as 20; the
        // message path never reaches it because the sum check fires first.
        assert_eq!(field_plot_radius([0, 0, 0, 0], Face::East, 20), Ok([0, 0, 0, 20]));
    }

    #[test]
    fn a_transfer_of_zero_items_is_refused() {
        assert!(transfer_quantity(1).is_ok());
        for refused in [0, -1, i32::MIN] {
            assert_eq!(
                transfer_quantity(refused),
                Err(Refusal::Number {
                    key: "quantity",
                    min: 1,
                    max: i64::from(i32::MAX)
                })
            );
        }
    }

    #[test]
    fn a_creative_citizen_transfer_may_be_zero_but_never_negative() {
        assert_eq!(transfer_to_citizen(0, true), Ok(0));
        assert_eq!(transfer_to_citizen(0, false), Err(Refusal::Number {
            key: "quantity",
            min: 1,
            max: i64::from(i32::MAX)
        }));
        assert_eq!(transfer_to_citizen(-1, true), Err(Refusal::Number {
            key: "quantity",
            min: 0,
            max: i64::from(i32::MAX)
        }));
    }

    #[test]
    fn a_post_box_request_takes_one_or_all_of_it() {
        assert_eq!(post_box_counts(32, true), Ok((32, 1)), "deliver what there is");
        assert_eq!(post_box_counts(32, false), Ok((32, 32)), "all of it or nothing");
        assert!(post_box_counts(0, true).is_err());
    }

    #[test]
    fn five_gold_ingots_are_the_price_of_spies() {
        assert_eq!(spies_cost(5), Ok(5));
        assert_eq!(
            spies_cost(4),
            Err(Gate::NotEnoughGold { need: 5, has: 4 })
        );
    }

    #[test]
    fn a_free_teleport_pays_the_gate_nothing() {
        assert!(teleport_payout(0).is_empty());
        assert!(teleport_payout(-8).is_empty());
        assert_eq!(teleport_payout(1), Vec::<i32>::new(), "half of 1 is 0");
    }

    #[test]
    fn a_paid_teleport_pays_half_and_loses_the_remainder_above_a_stack() {
        assert_eq!(teleport_payout(2), vec![1]);
        assert_eq!(teleport_payout(128), vec![64], "half of 128 is one stack");
        assert_eq!(teleport_payout(130), vec![64], "half of 130 is 65: 64 paid, 1 lost");
        assert_eq!(
            teleport_payout(258),
            vec![64, 64],
            "half of 258 is 129: 128 paid, 1 lost"
        );
        assert_eq!(teleport_payout(260), vec![64, 64], "half is 130: 128 paid, 2 lost");
    }

    #[test]
    fn the_break_side_is_not_a_face() {
        assert_eq!(crack_target(BREAK_BLOCK), Ok(Crack::Destroy));
        assert_eq!(crack_target(2), Ok(Crack::Face(Face::North)));
        assert_eq!(crack_target(5), Ok(Crack::Face(Face::East)));
    }

    #[test]
    fn a_side_outside_the_six_faces_is_refused_rather_than_thrown() {
        for refused in [-2, 0, 1, 6, 99] {
            assert_eq!(
                crack_target(refused),
                Err(Refusal::Number {
                    key: "side",
                    min: -1,
                    max: 5
                }),
                "side {refused}"
            );
        }
    }

    #[test]
    fn a_stream_of_zero_length_is_refused() {
        assert_eq!(
            stream_window(0, 0),
            Err(Refusal::Number {
                key: "maxStage",
                min: 1,
                max: i64::from(i32::MAX)
            })
        );
    }

    #[test]
    fn a_stream_looks_one_step_either_side_and_stays_in_range() {
        assert_eq!(stream_window(3, 10), Ok([2, 4]));
        assert_eq!(stream_window(0, 10), Ok([0, 1]), "the first step has no left side");
        assert_eq!(stream_window(10, 10), Ok([9, 10]), "the last has no right side");
        assert!(stream_window(-1, 10).is_err());
    }

    #[test]
    fn a_direct_place_reaches_five_blocks_and_no_further() {
        assert_eq!(direct_place_reach(5, 0, 0), Ok(25));
        assert!(direct_place_reach(5, 0, 0).is_ok());
        assert_eq!(
            direct_place_reach(5, 1, 0),
            Err(Refusal::Number {
                key: "pos",
                min: 0,
                max: 25
            })
        );
        assert!(direct_place_reach(-5, 0, 0).is_ok(), "a sign is not a distance");
    }

    #[test]
    fn a_direct_place_needs_air_a_hut_and_the_matching_state() {
        assert_eq!(direct_place_target(true, true, true), Ok(()));
        assert_eq!(direct_place_target(false, true, true), Err(Gate::NotAir));
        assert_eq!(direct_place_target(true, false, true), Err(Gate::NotAHut));
        assert_eq!(direct_place_target(true, true, false), Err(Gate::StateMismatch));
    }

    #[test]
    fn the_three_rank_kinds_are_read_and_nothing_else_is_guessed() {
        assert_eq!(rank_kind(0), Ok(RankKind::Manager));
        assert_eq!(rank_kind(1), Ok(RankKind::Hostile));
        assert_eq!(rank_kind(2), Ok(RankKind::Plain));
        assert_eq!(
            rank_kind(7),
            Err(Refusal::Number {
                key: "rankType",
                min: 0,
                max: 2
            }),
            "Java's default branch would demote the rank in silence"
        );
    }

    #[test]
    fn a_skill_is_clamped_to_one_and_ninety_nine() {
        assert_eq!(skill_increment(50, 10, true), Ok(60));
        assert_eq!(skill_increment(99, 5, true), Ok(99));
        assert_eq!(skill_increment(1, -5, true), Ok(1));
        assert_eq!(skill_increment(50, 10, false), Err(Gate::NotCreative));
    }

    #[test]
    fn a_miner_level_past_the_shaft_is_refused() {
        assert_eq!(miner_repair_level(0, 3), Ok(0));
        assert_eq!(miner_repair_level(2, 3), Ok(2));
        assert_eq!(
            miner_repair_level(3, 3),
            Err(Refusal::Number {
                key: "level",
                min: 0,
                max: 2
            }),
            "levels.get(3) on a three-level shaft throws in Java"
        );
        assert!(miner_repair_level(-1, 3).is_err());
    }

    #[test]
    fn a_recipe_swap_has_two_valid_ends() {
        assert_eq!(recipe_swap(1, true, 3), Ok((1, 0)));
        assert_eq!(recipe_swap(1, false, 3), Ok((1, 2)));
        assert!(recipe_swap(0, true, 3).is_err(), "nothing above the first");
        assert!(recipe_swap(2, false, 3).is_err(), "nothing below the last");
        assert!(recipe_swap(3, true, 3).is_err());
    }

    #[test]
    fn a_warehouse_upgrade_without_an_emerald_block_is_refused_not_thrown() {
        assert_eq!(warehouse_upgrade_slot(4), Ok(4));
        assert_eq!(warehouse_upgrade_slot(-1), Err(Gate::NoEmeraldBlock));
    }

    #[test]
    fn a_full_module_takes_no_more_citizens_and_a_fire_always_frees_one() {
        assert_eq!(hire_fire(true, false), Ok(true));
        assert_eq!(hire_fire(true, true), Err(Gate::ModuleFull));
        assert_eq!(hire_fire(false, true), Ok(false), "firing frees a slot");
    }

    #[test]
    fn an_assignment_is_refused_for_a_full_module_or_a_second_home() {
        assert_eq!(assign_module(true, false, false), Ok(()));
        assert_eq!(assign_module(true, true, false), Err(Gate::ModuleFull));
        assert_eq!(assign_module(true, false, true), Err(Gate::AlreadyAssigned));
        assert_eq!(assign_module(false, true, true), Ok(()), "an unassign is always allowed");
    }

    #[test]
    fn a_guard_keeps_its_mine_when_the_message_names_none() {
        assert_eq!(
            guard_mine_pos(None, [10, 20, 30], true),
            Ok([10, 20, 30]),
            "the fallback is the guard's own mine, not the origin"
        );
        assert_eq!(guard_mine_pos(Some([1, 2, 3]), [9, 9, 9], true), Ok([1, 2, 3]));
        assert_eq!(
            guard_mine_pos(Some([1, 2, 3]), [9, 9, 9], false),
            Err(Gate::NoSuchMiner)
        );
    }

    #[test]
    fn a_permission_removal_names_its_four_cases() {
        assert_eq!(remove_player_allowed(false, true, false, true, true), Ok(()));
        assert_eq!(remove_player_allowed(true, true, false, false, false), Ok(()), "oneself");
        assert_eq!(remove_player_allowed(false, true, true, true, false), Ok(()), "a hostile");
        assert_eq!(
            remove_player_allowed(false, true, false, true, false),
            Err(Gate::NotAllowed),
            "a non-hostile needs a manager to remove it"
        );
        assert_eq!(
            remove_player_allowed(false, false, false, true, true),
            Err(Gate::UnknownPlayer),
            "Java throws here; the port names it"
        );
    }

    #[test]
    fn the_constants_are_the_javas() {
        assert_eq!(MAX_FIELD_RANGE, 20, "FarmField.java:40");
        assert_eq!(SPIES_GOLD_COST, 5, "BuildingBarracks.java:65");
        assert_eq!(MAX_CITIZEN_LEVEL, 99, "CitizenConstants.java:181");
        assert_eq!(STACKSIZE, 64, "Constants.STACKSIZE");
        assert_eq!(BREAK_BLOCK, -1, "BlockParticleEffectMessage.java:22");
        assert_eq!(MAX_NAME_LENGTH, 25, "TownHallRenameMessage.java:17");
        assert_eq!(PLACE_REACH_SQUARED, 25, "DirectPlaceMessage.java:117");
    }

    /// The two doors in order: the codec refuses a `size` outside `0..=20`
    /// (`FarmFieldPlotResize::from_compound`, written from
    /// `FarmFieldPlotResizeMessage.java:22-106`), and a `size` the codec accepts can
    /// still be refused by the *sum* rule. One message, three answers — decoded, refused
    /// at the wire, refused by the rule — and the round trip in between.
    #[test]
    fn the_codec_and_the_sum_rule_refuse_at_different_doors() {
        use crate::network::building_modules::FarmFieldPlotResize;
        use crate::network::common::{int, position, text, Message};
        use mod_api::Compound;

        let wire = |size: i32| {
            Compound::from([
                ("size".into(), int(size)),
                ("direction".into(), text("north")),
                ("position".into(), position([1, 2, 3])),
            ])
        };
        assert_eq!(
            FarmFieldPlotResize::from_compound(&wire(40)),
            Err(Refusal::Number {
                key: "size",
                min: 0,
                max: 20
            }),
            "the codec stops at MAX_FIELD_RANGE, where Java's setRadius clamps"
        );
        let decoded = match FarmFieldPlotResize::from_compound(&wire(15)) {
            Ok(decoded) => decoded,
            Err(refusal) => panic!("15 is a valid size: {refusal}"),
        };
        assert_eq!(
            field_plot_radius([5, 5, 5, 5], decoded.direction, decoded.size),
            Err(Refusal::Size {
                key: "size",
                max: 20
            }),
            "5 + 5 + 5 + 15 is 30: the wire was right, the field is not"
        );
        assert_eq!(
            FarmFieldPlotResize::from_compound(&decoded.to_compound()),
            Ok(decoded),
            "and what the codec accepts round-trips"
        );
    }

    #[test]
    fn a_gate_and_a_refusal_are_not_the_same_answer() {
        // The distinction the module note argues: a value can be a fine int and still
        // be refused by the world, and a log line must not confuse the two.
        assert!(field_plot_radius(SQUARE, Face::North, 40).is_err());
        assert!(matches!(
            field_plot_radius(SQUARE, Face::North, 40),
            Err(Refusal::Size { .. })
        ));
        assert!(matches!(spies_cost(0), Err(Gate::NotEnoughGold { .. })));
    }
}
