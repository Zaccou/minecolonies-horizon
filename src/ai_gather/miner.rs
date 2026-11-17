//! The miner: a shaft with a ladder, then one branch per blueprint.
//!
//! Java reference: `core/entity/ai/workers/production/EntityAIStructureMiner.java` —
//! the constructor's `registerTargets` (`:151-163`), `startWorkingAtOwnBuilding`
//! (`:174-207`), `doMining` (`:258-290`), `getSurroundingOreOrDefault` (`:298-309`),
//! `goToLadder` (`:317-324`), `repairLadder` (`:343-384`), `checkMineShaft`
//! (`:411-434`), `doShaftMining` (`:449-467`), `advanceLadder` (`:469-537`),
//! `getNextBlockInShaftToMine` (`:550-645`), `doShaftBuilding` (`:648-664`),
//! `executeNodeMining` (`:667-678`), `searchANodeToMine` (`:680-758`), `secureBlock`
//! (`:760-782`), `executeStructurePlacement` (`:784-826`), `getNodeMiningPosition`
//! (`:956-980`) and `ladderDamaged` (`:1055-1061`), on top of `util/MineNode.java`,
//! `util/MinerLevel.java` and `colony/buildings/workerbuildings/BuildingMiner.java`.
//!
//! ## The table this adds
//!
//! | # | State | Rule | Rate | Java |
//! | ---: | --- | --- | ---: | --- |
//! | 1 | `PREPARING` | → `MINER_CHECK_MINESHAFT` (constant) | 1 | `:155` |
//! | 2 | `MINER_WALKING_TO_LADDER` | `goToLadder` | 20 | `:156` |
//! | 3 | `MINER_REPAIRING_LADDER` | `repairLadder` | 5 | `:157` |
//! | 4 | `MINER_CHECK_MINESHAFT` | `checkMineShaft` | 20 | `:158` |
//! | 5 | `MINER_MINING_SHAFT` | `doShaftMining` | 5 | `:159` |
//! | 6 | `MINER_BUILDING_SHAFT` | `doShaftBuilding` | 5 | `:160` |
//! | 7 | `MINER_MINING_NODE` | `executeNodeMining` | 5 | `:161` |
//! | 8 | `MINE_BLOCK` | `doMining` (overridden) | 10 | `AbstractEntityAIStructure.java:179` |
//! | 9 | `START_WORKING` | `startWorkingAtOwnBuilding` (overridden) | 20 | `AbstractEntityAIStructure.java:186` |
//!
//! **There is no `EntityAIWorkMiner`.** The miner is
//! `AbstractEntityAIStructureWithWorkOrder`: it builds a mineshaft blueprint, so
//! `LOAD_STRUCTURE` and `BUILDING_STEP` are the structure AI's states and are carried
//! by `crate::ai::builder` over `WorkerState`. This module hands those two back
//! ([`super::register_handover`]) and ports the shaft machine; `MINE_BLOCK` stays here
//! because `doMining` is *overridden* by the miner (`:258-290`), and
//! `afterStructureLoading` sends `BUILDING_STEP` back to `MINER_CHECK_MINESHAFT`
//! (`:828-832`).
//!
//! **One number is not a constant.** `getActionsDoneUntilDumping()` is
//! `building.getBuildingLevel() * MAX_BLOCKS_MINED` (`:222-225`): a level-3 miner dumps
//! after 192 blocks, a level-1 after 64 — [`actions_until_dumping`].
//!
//! **The depth, as a pure function.** `getDepthLimit` (`BuildingMiner.java:173-193`)
//! walks `MINING_LEVELS = {48, 16, -16, -100}` down by one hut level, then
//! `normalizeMaxDepth` (`:203-213`) clamps to `getMinBuildHeight() + 5` and to the
//! hut's `MAX_DEPTH` setting. [`depth_limit`] is that, without a world.
//!
//! **The shaft's geometry is a scan, and the scan is the decision.**
//! `getLastLadder` (`WorkerUtil.java:340-350`), `ladderDamaged` (`:1055-1061`) and the
//! 7x7 "nearest block" loop of `getNextBlockInShaftToMine` (`:596-645`) are ported as
//! functions over [`super::GatherBlocks`]; what they *change* stays the caller's.

use crate::ai::facts::{Pos, WorldFacts};
use crate::ai::state::WorkerState;

use super::facts::{GatherBlock, GatherBlocks, GatherMemory, MinerFacts, NodeFacts, NodeStatus, NodeType};
use super::state::{GatherRefusal, GatherState};
use super::{at_hut, gather_action, Decision, TICKS_SECOND};

/// `EntityAIStructureMiner.OTHER_SIDE_OF_SHAFT` (`:83`): how far across the shaft the
/// miner stands to work a node.
pub const OTHER_SIDE_OF_SHAFT: i32 = 6;
/// `COBBLE_REQUEST_BATCHES` (`:88`): the fill block request size.
pub const COBBLE_REQUEST_BATCHES: u32 = 32;
/// `LADDER_REQUEST_BATCHES` (`:93`): the ladder request size.
pub const LADDER_REQUEST_BATCHES: u32 = 10;
/// `NODE_DISTANCE` (`:100`): a node's footprint, used by the liquid sweep.
pub const NODE_DISTANCE: i32 = 7;
/// `MAX_BLOCKS_MINED` (`:104`): multiplied by the hut's level (`:222-225`).
pub const MAX_BLOCKS_MINED: u32 = 64;
/// `SHAFT_RADIUS` (`:105`): the shaft is 7x7.
pub const SHAFT_RADIUS: i32 = 3;
/// `SAFE_CHECK_RANGE` (`:106`): the floor sweep before a ladder step.
pub const SAFE_CHECK_RANGE: i32 = 5;
/// `SHAFT_BASE_DEPTH` (`:111`): how far under the world floor the shaft counts as deep.
pub const SHAFT_BASE_DEPTH: i32 = 8;
/// `LIQUID_CHECK_RANGE` (`:124`): how far above the node the liquid sweep reaches.
pub const LIQUID_CHECK_RANGE: i32 = 5;
/// `MIN_WORKING_RANGE` (`api/util/constant/CitizenConstants.java:276`): close enough
/// to build from, where the miner does not look for a safe spot (`:339`).
pub const MIN_WORKING_RANGE: i32 = 12;
/// `STANDARD_WORKING_RANGE` (`CitizenConstants.java:271`): the walk's target distance.
pub const STANDARD_WORKING_RANGE: i32 = 5;

/// `BuildingMiner.MINING_LEVELS` (`:63`): the depths a hut level unlocks.
pub const MINING_LEVELS: [i32; 4] = [48, 16, -16, -100];

/// `ROTATE_TWICE` (`:117`): a node pointing west.
pub const ROTATE_TWICE: u8 = 2;
/// `ROTATE_THREE_TIMES` (`:118`): a node pointing north.
pub const ROTATE_THREE_TIMES: u8 = 3;
/// `ROTATE_ONCE` (`:116`): a node pointing south.
pub const ROTATE_ONCE: u8 = 1;

/// `getActionsDoneUntilDumping` (`:222-225`): the hut's level times `MAX_BLOCKS_MINED`.
pub fn actions_until_dumping(scene: &super::GatherScene) -> u32 {
    let level = scene.miner().map(|facts| facts.building_level).unwrap_or(0);
    level.saturating_mul(MAX_BLOCKS_MINED)
}

/// `BuildingMiner.getDepthLimit` (`:173-193`) and `normalizeMaxDepth` (`:203-213`).
///
/// The hut's `ladder_y` minus five is where its levels start counting; each hut level
/// takes the next entry of [`MINING_LEVELS`] that is below it. `min_build_height + 5`
/// is the floor the world itself imposes, and a non-default `MAX_DEPTH` setting wins
/// only when it is deeper.
pub fn depth_limit(
    ladder_y: i32,
    building_level: u32,
    min_build_height: i32,
    max_depth_setting: i32,
    setting_is_default: bool,
) -> i32 {
    let building_y = ladder_y - 5; // :175
    let mut levels = building_level as i32; // :177
    let mut y_level = 0; // :178
    for mining_level in MINING_LEVELS {
        if mining_level < building_y {
            y_level = mining_level; // :181-182
            levels -= 1; // :183
        }
        if levels == 0 {
            break; // :186-189
        }
    }
    let world_max_depth = min_build_height + 5; // :205
    if setting_is_default {
        world_max_depth.max(y_level) // :208
    } else {
        world_max_depth.max(y_level.max(max_depth_setting)) // :211
    }
}

/// `WorkerUtil.getLastLadder` (`:340-350`): the y of the lowest ladder of the column at
/// `pos`, or `pos.y + 1` when there is no ladder there at all.
///
/// Java recurses; the port loops, because a caller can report a ladder column of any
/// height and a recursion would be a stack overflow waiting for one bad fact. The `512`
/// bound is the port's, not Java's, and it only bites on half a kilometre of ladders.
pub fn last_ladder(blocks: &GatherBlocks, pos: Pos) -> i32 {
    let mut y = pos.y;
    let mut seen = 0;
    while blocks.at(Pos::new(pos.x, y, pos.z)) == GatherBlock::Ladder {
        y -= 1;
        seen += 1;
        if seen > 512 {
            break;
        }
    }
    y + 1
}

/// `SHAFT_RADIUS * (ladder - cobble)` (`:482-484`, `:573-575`): how far the shaft's far
/// wall is from the ladder column, in `x` and `z`.
pub fn shaft_offset(ladder: Pos, cobble: Pos) -> (i32, i32) {
    (
        SHAFT_RADIUS * (ladder.x - cobble.x),
        SHAFT_RADIUS * (ladder.z - cobble.z),
    )
}

/// `ladderDamaged` (`:1055-1061`): the block under the lowest ladder is neither a ladder
/// nor solid, so the shaft cannot be climbed.
pub fn ladder_damaged(blocks: &GatherBlocks, ladder: Pos) -> bool {
    let last = last_ladder(blocks, ladder);
    let next = blocks.at(Pos::new(ladder.x, last - 1, ladder.z));
    next != GatherBlock::Ladder && !next.blocks_motion()
}

/// What [`next_block_in_shaft`] decided: what to mine, where to stand, and which fluids
/// the caller has to fill first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShaftTarget {
    /// `nextBlockToMine` (`:624`, `:644`): `None` when the ring is already clear, which
    /// sends the miner to `advanceLadder` (`:453-456`).
    pub to_mine: Option<Pos>,
    /// `currentStandingPosition` (`:638`): where the miner stands to mine it.
    pub standing: Option<Pos>,
    /// What the "remove water for safety" sweep found (`:577-594`, `:612-616`):
    /// `BlockUtils.removeFluid` + `setBlockFromInventory`, which is the caller's.
    pub fluids: Vec<Pos>,
}

/// `getNextBlockInShaftToMine` (`:550-645`), as a scan over the reported blocks.
///
/// Three steps, in Java's order:
///
/// 1. the block the miner is already working (`minerWorkingLocation`, `:554-568`), if it
///    is neither air, nor a ladder, nor a fluid;
/// 2. the ring one level under the lowest ladder, whole 7x7 plus a two-block margin for
///    water (`:577-594`) — every fluid there has to be filled;
/// 3. inside that ring, the nearest non-air block by
///    `distSqr(cur, ladder) + distSqr(cur, working)²` (`:598-621`), then the closest air
///    cell around it as the standing position (`:622-643`).
///
/// **The `x == 0 && z == 0` skip tests absolute coordinates, not the ring's centre**
/// (`:582`, `:602`): with an offset — the ladder is not the ring's origin — Java skips
/// the ladder's own column and *not* the cell at the offset. The port keeps that,
/// because the miner's walking depends on it.
pub fn next_block_in_shaft(facts: &MinerFacts, memory: &GatherMemory) -> ShaftTarget {
    let (Some(ladder), Some(cobble)) = (facts.ladder, facts.cobble) else {
        return ShaftTarget::default();
    };
    let last = last_ladder(&facts.blocks, ladder);
    let (x_offset, z_offset) = shaft_offset(ladder, cobble);
    let mut fluids: Vec<Pos> = Vec::new();

    // :554-568 — the block being worked right now.
    let working = memory
        .miner
        .working_location
        .unwrap_or(Pos::new(ladder.x, last + 1, ladder.z));
    let at_working = facts.blocks.at(working);
    if !at_working.is_air() && at_working != GatherBlock::Ladder && !at_working.is_fluid() {
        return ShaftTarget {
            to_mine: Some(working),
            standing: Some(memory.miner.standing_position.unwrap_or(working)),
            fluids,
        };
    }

    // :577-594 — the water sweep, 7x7 plus two on every side.
    let mut x = SHAFT_RADIUS + x_offset + 2;
    while x >= -SHAFT_RADIUS + x_offset - 2 {
        let mut z = -SHAFT_RADIUS + z_offset - 2;
        while z <= SHAFT_RADIUS + z_offset + 2 {
            if !(x == 0 && z == 0) {
                let cur = Pos::new(ladder.x + x, last, ladder.z + z);
                if facts.blocks.at(cur).is_fluid() && !fluids.contains(&cur) {
                    fluids.push(cur);
                }
            }
            z += 1;
        }
        x -= 1;
    }

    // :596-621 — the nearest block of the 7x7 ring.
    let mut best: Option<(Pos, f64)> = None;
    let mut x = SHAFT_RADIUS + x_offset;
    while x >= -SHAFT_RADIUS + x_offset {
        let mut z = -SHAFT_RADIUS + z_offset;
        while z <= SHAFT_RADIUS + z_offset {
            if !(x == 0 && z == 0) {
                let cur = Pos::new(ladder.x + x, last, ladder.z + z);
                let block = facts.blocks.at(cur);
                if !block.is_air() {
                    let distance =
                        cur.dist_sqr(ladder) as f64 + (cur.dist_sqr(working) as f64).powi(2);
                    if best.map(|(_, best)| distance < best).unwrap_or(true) {
                        if block.is_fluid() && !fluids.contains(&cur) {
                            fluids.push(cur);
                        }
                        best = Some((cur, distance));
                    }
                }
            }
            z += 1;
        }
        x -= 1;
    }

    let Some((to_mine, _)) = best else {
        return ShaftTarget {
            to_mine: None,
            standing: memory.miner.standing_position,
            fluids,
        };
    };

    // :622-643 — the closest air cell around the block, as the standing position.
    let mut standing = memory.miner.standing_position;
    let mut best_distance = f64::MAX;
    let mut x = 1;
    while x >= -1 {
        let mut z = -1;
        while z <= 1 {
            if !(x == 0 && z == 0) {
                let cur = Pos::new(to_mine.x + x, last, to_mine.z + z);
                if facts.blocks.at(cur).is_air() {
                    let distance = cur.dist_sqr(ladder) as f64;
                    if distance < best_distance {
                        standing = Some(cur);
                        best_distance = distance;
                    }
                }
            }
            z += 1;
        }
        x -= 1;
    }

    ShaftTarget {
        to_mine: Some(to_mine),
        standing,
        fluids,
    }
}

/// `secureBlock` (`:760-782`): a block that does not block motion (and is not a torch),
/// a fluid, or an ore has to be mined out and filled before the next ladder step.
pub fn needs_securing(block: GatherBlock) -> bool {
    (!block.blocks_motion() && block != GatherBlock::Torch) || block.is_fluid() || block == GatherBlock::Ore
}

/// `getSurroundingOreOrDefault` (`:298-309`): the first ore of the six neighbours, else
/// `pos` itself. `Direction.values()` order is down, up, north, south, west, east.
pub fn surrounding_ore_or(blocks: &GatherBlocks, pos: Pos) -> Pos {
    const OFFSETS: [(i32, i32, i32); 6] = [
        (0, -1, 0),
        (0, 1, 0),
        (0, 0, -1),
        (0, 0, 1),
        (-1, 0, 0),
        (1, 0, 0),
    ];
    for (dx, dy, dz) in OFFSETS {
        let offset = Pos::new(pos.x + dx, pos.y + dy, pos.z + dz);
        if blocks.at(offset) == GatherBlock::Ore {
            return offset;
        }
    }
    pos
}

/// `searchANodeToMine`'s rotation (`:706-724`): which way the node's blueprint faces,
/// from the node's parent. `0` is +x, `1` south, `2` west, `3` north.
pub fn rotation_for(node: NodeFacts) -> u8 {
    let (x, z) = node.at;
    let (px, pz) = node.parent;
    let working_x = if x > px { 1 } else { 0 };
    let working_z = if z > pz { 1 } else { 0 };
    let vector_x = if x < px { -1 } else { working_x };
    let vector_z = if z < pz { -1 } else { working_z };
    if vector_x == -1 {
        ROTATE_TWICE
    } else if vector_z == -1 {
        ROTATE_THREE_TIMES
    } else if vector_z == 1 {
        ROTATE_ONCE
    } else {
        0
    }
}

/// `getNodeMiningPosition` (`:956-980`): where the miner stands to work a node — across
/// the shaft when the parent is the shaft, on the parent otherwise.
pub fn node_mining_position(facts: &MinerFacts, depth: i32) -> Option<Pos> {
    let node = facts.node?;
    let (ladder, cobble) = (facts.ladder?, facts.cobble?);
    let parent_is_shaft = facts
        .parent_node
        .map(|parent| parent.style == NodeType::Shaft)
        .unwrap_or(false);
    if parent_is_shaft {
        let (x_offset, z_offset) = shaft_offset(ladder, cobble);
        return Some(Pos::new(
            ladder.x + x_offset.signum() * OTHER_SIDE_OF_SHAFT,
            depth,
            ladder.z + z_offset.signum() * OTHER_SIDE_OF_SHAFT,
        ));
    }
    Some(Pos::new(node.parent.0, depth, node.parent.1))
}

/// `startWorkingAtOwnBuilding` (`:174-207`), the miner's override.
pub fn start_working(facts: &WorldFacts, trade: &MinerFacts, memory: &mut GatherMemory) -> Decision {
    let _ = memory;
    // :178-181 — no ladder yet, or the citizen is still above the hut: walk there.
    if (trade.ladder.is_none() || facts.worker.at.y >= facts.building.position.y) && !at_hut(facts) {
        return Decision::stay();
    }
    // :183-187 — the hut has no anchor at all: the player has to move it.
    if trade.ladder.is_none() || trade.cobble.is_none() {
        return Decision::go_after(
            GatherState::Base(WorkerState::StartWorking),
            GatherRefusal::NoMineshaft,
        );
    }
    // :189-197 — a queued miner work order is loaded before anything else.
    if !trade.has_work_order && trade.order_available {
        return Decision::go(GatherState::Base(WorkerState::LoadStructure));
    }
    // :199-206 — `super.startWorkingAtOwnBuilding()` (`AbstractEntityAIStructure:207-214`):
    // a structure to build wins, otherwise the miner prepares.
    if trade.has_work_order && trade.has_blueprint {
        return Decision::go_after(
            GatherState::Base(WorkerState::StartBuilding),
            GatherRefusal::StructureTableOwned,
        );
    }
    Decision::go(GatherState::Base(WorkerState::Preparing)) // :206
}

/// `checkMineShaft` (`:411-434`): shaft or node, by the depth of the lowest ladder.
pub fn check_mine_shaft(
    facts: &WorldFacts,
    trade: &MinerFacts,
    memory: &mut GatherMemory,
) -> Decision {
    let _ = (facts, memory);
    let Some(ladder) = trade.ladder else {
        return Decision::go_after(
            GatherState::MinerWalkingToLadder,
            GatherRefusal::NoMineshaft,
        );
    };
    let last = last_ladder(&trade.blocks, ladder);
    // :420-431 — past the depth limit, the miner works the level's branches.
    let limit = depth_limit(
        ladder.y,
        trade.building_level,
        trade.min_build_height,
        trade.max_depth_setting,
        trade.max_depth_is_default,
    );
    if last < limit {
        if trade.levels == 0 {
            // :424-428 — the hut is too shallow to mine anything at this depth.
            return Decision::go_after(GatherState::Base(WorkerState::Idle), GatherRefusal::NeedsBetterHut);
        }
        return Decision::go(GatherState::MinerMiningNode);
    }
    Decision::go(GatherState::MinerMiningShaft) // :433
}

/// `doShaftMining` (`:449-467`): widen the ring, or advance the ladder.
pub fn do_shaft_mining(
    _facts: &WorldFacts,
    trade: &MinerFacts,
    memory: &mut GatherMemory,
) -> Decision {
    let target = next_block_in_shaft(trade, memory);
    if target.to_mine.is_none() {
        return advance_ladder(trade, memory); // :453-456
    }
    // :452 — `minerWorkingLocation` is kept for the next tick.
    memory.miner.working_location = target.to_mine.or(memory.miner.working_location);
    memory.miner.standing_position = target.standing.or(memory.miner.standing_position);
    // :461-464 — the mine result and the saturation drop are the caller's; Java carries
    // on in `MINER_MINING_SHAFT` either way ("omitted because end of method", `:459-460`).
    Decision::go(GatherState::MinerMiningShaft)
}

/// `advanceLadder` (`:469-537`): one ladder step down, if the ground under it is safe.
///
/// Java has **no `AITarget`** for it: `doShaftMining` calls it when the ring is clear
/// (`:453-456`), which is why there is no `gather_action!` line for this function beside
/// the other seven.
pub fn advance_ladder(trade: &MinerFacts, memory: &mut GatherMemory) -> Decision {
    // :471-475 — both materials have to be there, or a request is out.
    if !trade.fill_block_available || !trade.ladder_available {
        return Decision::refuse(GatherRefusal::MissingShaftMaterial);
    }
    if let Some(ladder) = trade.ladder {
        // :477-480
        if ladder_damaged(&trade.blocks, ladder) {
            return Decision::go(GatherState::MinerRepairingLadder);
        }
    }
    let (Some(ladder), Some(cobble)) = (trade.ladder, trade.cobble) else {
        return Decision::refuse(GatherRefusal::NoMineshaft);
    };
    let last = last_ladder(&trade.blocks, ladder);
    let (x_offset, z_offset) = shaft_offset(ladder, cobble);
    let next_cobble = Pos::new(cobble.x, last - 1, cobble.z);
    let next_ladder = Pos::new(ladder.x, last - 1, ladder.z);
    let safe_cobble = Pos::new(ladder.x, last - 2, ladder.z);

    // :492-502 — the floor under the next step has to be solid, 11x11 around it.
    let mut x = -SAFE_CHECK_RANGE;
    while x <= SAFE_CHECK_RANGE {
        let mut z = -SAFE_CHECK_RANGE;
        while z <= SAFE_CHECK_RANGE {
            let cur = Pos::new(
                safe_cobble.x + x + x_offset,
                safe_cobble.y,
                safe_cobble.z + z + z_offset,
            );
            if needs_securing(trade.blocks.at(cur)) {
                // `setDelay(1)` then `return state` (`:768-771`): securing goes fast.
                memory.base.delay = 1;
                return Decision::stay();
            }
            z += 1;
        }
        x -= 1;
    }

    // :509-518 — the shaft's starting level, four blocks under the first step.
    if memory.miner.starting_level_shaft == 0 {
        memory.miner.starting_level_shaft = next_cobble.y - 4;
    }
    if next_cobble.y < memory.miner.starting_level_shaft {
        return Decision::go(GatherState::MinerBuildingShaft); // the level's blueprint now
    }

    // :520-525 — the two blocks have to be air, or mined out first.
    let cobble_replaceable = trade.blocks.at(next_cobble);
    let ladder_replaceable = trade.blocks.at(next_ladder);
    let cobble_ok = cobble_replaceable.is_air() || trade.mine_done;
    let ladder_ok = ladder_replaceable.is_air() || trade.mine_done;
    if !cobble_ok || !ladder_ok {
        return Decision::refuse(GatherRefusal::ShaftBlocked);
    }

    // :531-536 — both blocks go in (the caller's `setBlockFromInventory`), one action.
    Decision::go(GatherState::MinerCheckMineshaft)
}

/// `repairLadder` (`:343-384`): the cobble behind the ladder, then the ladder itself.
pub fn repair_ladder(facts: &WorldFacts, trade: &MinerFacts, memory: &mut GatherMemory) -> Decision {
    let _ = (facts, memory);
    let (Some(ladder), Some(cobble)) = (trade.ladder, trade.cobble) else {
        return Decision::refuse(GatherRefusal::NoMineshaft);
    };
    let last = last_ladder(&trade.blocks, ladder);
    let next_cobble = Pos::new(cobble.x, last - 1, cobble.z);
    let next_ladder = Pos::new(ladder.x, last - 1, ladder.z);

    // :352-363 — the back of the ladder first.
    let behind = trade.blocks.at(next_cobble);
    if !behind.blocks_motion() {
        if !trade.fill_block_available {
            return Decision::refuse(GatherRefusal::MissingShaftMaterial);
        }
        if !behind.is_air() && !trade.mine_done {
            return Decision::stay();
        }
        return Decision::stay(); // `setBlockFromInventory(nextCobble, ladderBack)` `:362`
    }

    // :366-382 — then the ladder.
    let at = trade.blocks.at(next_ladder);
    if at != GatherBlock::Ladder && !at.blocks_motion() {
        if !trade.ladder_available {
            return Decision::refuse(GatherRefusal::MissingShaftMaterial);
        }
        if !at.is_air() && !trade.mine_done {
            return Decision::stay();
        }
        return Decision::stay(); // `setBlockFromInventory(nextLadder, LADDER)` `:380`
    }
    Decision::go(GatherState::MinerCheckMineshaft) // :383
}

/// `goToLadder` (`:317-324`): walk to the ladder, then repair it.
pub fn go_to_ladder(facts: &WorldFacts, trade: &MinerFacts, memory: &mut GatherMemory) -> Decision {
    let _ = (facts, memory, trade);
    if !trade.at_ladder {
        return Decision::stay(); // :319-322, still `MINER_WALKING_TO_LADDER`
    }
    Decision::go(GatherState::MinerRepairingLadder) // :323
}

/// `doShaftBuilding` (`:648-664`): walk to the hut, then run the level's blueprint.
pub fn do_shaft_building(facts: &WorldFacts, trade: &MinerFacts, memory: &mut GatherMemory) -> Decision {
    let _ = (trade, memory);
    if !at_hut(facts) {
        return Decision::stay(); // :650-653
    }
    Decision::go(GatherState::Base(WorkerState::LoadStructure)) // :662-663
}

/// `executeNodeMining` (`:667-678`) and `searchANodeToMine` (`:680-758`).
pub fn execute_node_mining(
    facts: &WorldFacts,
    trade: &MinerFacts,
    memory: &mut GatherMemory,
) -> Decision {
    let _ = facts;
    // :671-676 — no level picked yet: Java takes the last one and recurses. The port
    // records the pick and answers next tick, because the level's depth is a fact the
    // caller has to refresh.
    if trade.current_level_depth.is_none() {
        memory.miner.level_picked = true;
        return Decision::stay();
    }
    // :689-702 — the node was completed or never set.
    let Some(node) = trade.node else {
        return Decision::go_after(GatherState::MinerCheckMineshaft, GatherRefusal::NoNode);
    };
    if node.status == NodeStatus::Completed {
        return Decision::go_after(GatherState::MinerCheckMineshaft, GatherRefusal::NoNode);
    }
    // :732-742 — an unfinished, non-shaft parent is worked first.
    if let Some(parent) = trade.parent_node {
        if parent.style != NodeType::Shaft && parent.status != NodeStatus::Completed {
            return Decision::go(GatherState::MinerCheckMineshaft);
        }
    }
    // :745-750 — the node is not part of the level's grid any more.
    if !trade.node_in_level {
        return Decision::go(GatherState::MinerMiningShaft);
    }
    // :752-757 — walk to the standing position, then place the node's blueprint.
    if matches!(node.status, NodeStatus::Available | NodeStatus::InProgress) && !trade.at_standing {
        return Decision::go(GatherState::Base(WorkerState::LoadStructure));
    }
    Decision::go(GatherState::MinerCheckMineshaft) // :757
}

/// `doMining` (`:258-290`), the miner's override of the structure AI's `MINE_BLOCK`.
pub fn do_mining(facts: &WorldFacts, trade: &MinerFacts, memory: &mut GatherMemory) -> Decision {
    let _ = (facts, memory);
    // :260-263 — nothing to mine: the structure step comes next.
    let Some(block) = trade.block_to_mine else {
        return Decision::go(GatherState::Base(WorkerState::BuildingStep));
    };
    // :265-269 — an ore chain: if the block is not ore, look at its neighbours first.
    let target = if trade.blocks.at(block) == GatherBlock::Ore {
        block
    } else {
        surrounding_ore_or(&trade.blocks, block)
    };
    // :271-274
    if trade.blocks.at(target).is_air() {
        return Decision::go(GatherState::Base(WorkerState::BuildingStep));
    }
    // :276-280 — `mineBlock` did not finish: swing and stay.
    if !trade.mine_done {
        return Decision::refuse(GatherRefusal::MiningIncomplete);
    }
    // :282-289 — the chain continues on the next ore, or the structure step resumes.
    let next = surrounding_ore_or(&trade.blocks, target);
    if trade.blocks.at(next) == GatherBlock::Ore {
        return Decision::stay();
    }
    Decision::go(GatherState::Base(WorkerState::BuildingStep))
}

gather_action!(start_working_action, Miner, start_working);
gather_action!(check_action, Miner, check_mine_shaft);
gather_action!(shaft_mining_action, Miner, do_shaft_mining);
gather_action!(repair_action, Miner, repair_ladder);
gather_action!(ladder_action, Miner, go_to_ladder);
gather_action!(shaft_building_action, Miner, do_shaft_building);
gather_action!(node_action, Miner, execute_node_mining);
gather_action!(mining_action, Miner, do_mining);

/// Register the nine targets of `EntityAIStructureMiner.java:151-163` plus the two
/// structure states the miner claims.
pub fn register(machine: &mut crate::ai::state::Machine<GatherState, super::GatherScene>) {
    use crate::ai::state::Transition;
    // :155 — `new AITarget(PREPARING, MINER_CHECK_MINESHAFT, 1)`: a constant supplier.
    machine.add(Transition::goto(
        GatherState::Base(WorkerState::Preparing),
        GatherState::MinerCheckMineshaft,
        "preparingToCheckMineshaft",
        "EntityAIStructureMiner.java:155",
        1,
    ));
    // :156 — `new AITarget(MINER_WALKING_TO_LADDER, this::goToLadder, TICKS_SECOND)`.
    super::add_target(
        machine,
        GatherState::MinerWalkingToLadder,
        "goToLadder",
        "EntityAIStructureMiner.java:156",
        TICKS_SECOND,
        ladder_action,
    );
    // :157 — `new AITarget(MINER_REPAIRING_LADDER, this::repairLadder, STANDARD_DELAY)`.
    super::add_target(
        machine,
        GatherState::MinerRepairingLadder,
        "repairLadder",
        "EntityAIStructureMiner.java:157",
        super::STANDARD_DELAY,
        repair_action,
    );
    // :158 — `new AITarget(MINER_CHECK_MINESHAFT, this::checkMineShaft, TICKS_SECOND)`.
    super::add_target(
        machine,
        GatherState::MinerCheckMineshaft,
        "checkMineShaft",
        "EntityAIStructureMiner.java:158",
        TICKS_SECOND,
        check_action,
    );
    // :159 — `new AITarget(MINER_MINING_SHAFT, this::doShaftMining, STANDARD_DELAY)`.
    super::add_target(
        machine,
        GatherState::MinerMiningShaft,
        "doShaftMining",
        "EntityAIStructureMiner.java:159",
        super::STANDARD_DELAY,
        shaft_mining_action,
    );
    // :160 — `new AITarget(MINER_BUILDING_SHAFT, this::doShaftBuilding, STANDARD_DELAY)`.
    super::add_target(
        machine,
        GatherState::MinerBuildingShaft,
        "doShaftBuilding",
        "EntityAIStructureMiner.java:160",
        super::STANDARD_DELAY,
        shaft_building_action,
    );
    // :161 — `new AITarget(MINER_MINING_NODE, this::executeNodeMining, STANDARD_DELAY)`.
    super::add_target(
        machine,
        GatherState::MinerMiningNode,
        "executeNodeMining",
        "EntityAIStructureMiner.java:161",
        super::STANDARD_DELAY,
        node_action,
    );
    // `AbstractEntityAIStructure.java:179` — `new AITarget(MINE_BLOCK, this::doMining, 10)`,
    // with `doMining` overridden at `EntityAIStructureMiner.java:258`.
    super::add_target(
        machine,
        GatherState::Base(WorkerState::MineBlock),
        "doMining",
        "EntityAIStructureMiner.java:258-290",
        10,
        mining_action,
    );
    // `AbstractEntityAIStructure.java:186` — `new AITarget(START_WORKING,
    // this::startWorkingAtOwnBuilding, TICKS_SECOND)`, overridden at `:174-207`.
    super::add_target(
        machine,
        GatherState::Base(WorkerState::StartWorking),
        "startWorkingAtOwnBuilding",
        "EntityAIStructureMiner.java:174-207",
        TICKS_SECOND,
        start_working_action,
    );
    // The remaining exit of `START_BUILDING`: the structure table owns it
    // (`AbstractEntityAIStructure.java:207-214`) and answers `IDLE`, which is where the
    // miner's `decide` picks the shaft up again.
    machine.add(Transition::goto(
        GatherState::Base(WorkerState::StartBuilding),
        GatherState::Base(WorkerState::Idle),
        "startBuilding",
        "AbstractEntityAIStructure.java:207-214",
        super::STANDARD_DELAY,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_gather::facts::GatherJob;
    use crate::ai_gather::GatherScene;

    fn ladder_at() -> (Pos, Pos) {
        // A ladder column at x=0,z=0 and the cobble one block east.
        (Pos::new(0, 60, 0), Pos::new(1, 60, 0))
    }

    fn shaft_facts() -> MinerFacts {
        let (ladder, cobble) = ladder_at();
        let mut blocks = GatherBlocks::new();
        blocks.set(Pos::new(0, 60, 0), GatherBlock::Ladder);
        blocks.set(Pos::new(0, 59, 0), GatherBlock::Ladder);
        MinerFacts {
            ladder: Some(ladder),
            cobble: Some(cobble),
            building_level: 1,
            levels: 1,
            min_build_height: -64,
            max_depth_setting: 0,
            max_depth_is_default: true,
            blocks,
            fill_block_available: true,
            ladder_available: true,
            has_work_order: false,
            ..MinerFacts::default()
        }
    }

    #[test]
    fn the_depth_limit_follows_the_hut_level_and_the_world_floor() {
        // `BuildingMiner.java:173-193`: level 1 unlocks 48, level 2 the next below.
        // A hut whose ladder starts at y=70 counts its levels from y=65, so 48 < 65.
        assert_eq!(depth_limit(70, 1, -64, 0, true), 48);
        assert_eq!(depth_limit(70, 2, -64, 0, true), 16);
        assert_eq!(depth_limit(70, 3, -64, 0, true), -16);
        // Level 4 asks for -100, and the world floor `getMinBuildHeight() + 5` = -59
        // wins over it (`normalizeMaxDepth`, `BuildingMiner.java:205-208`).
        assert_eq!(depth_limit(70, 4, -64, 0, true), -59);
        // A world with a shallower floor lets the fourth level through.
        assert_eq!(depth_limit(70, 4, -200, 0, true), -100);
        // A deeper `MAX_DEPTH` setting only when it is not the default (`:207-211`).
        assert_eq!(depth_limit(70, 1, -64, -80, false), 48, "48 is still deeper");
        assert_eq!(depth_limit(70, 1, -64, 0, false), 48, "0 is above 48");
        // The world floor still wins over a very deep setting: -64 + 5 = -59.
        assert_eq!(depth_limit(-40, 4, -64, -110, false), -59);
    }

    #[test]
    fn the_last_ladder_is_the_lowest_ladder_of_the_column() {
        // `WorkerUtil.java:340-350`: recurse down while it is a ladder, answer y + 1.
        let mut blocks = GatherBlocks::new();
        for y in 55..=60 {
            blocks.set(Pos::new(0, y, 0), GatherBlock::Ladder);
        }
        assert_eq!(last_ladder(&blocks, Pos::new(0, 60, 0)), 55);
        // No ladder at all: `pos.y + 1`.
        assert_eq!(last_ladder(&blocks, Pos::new(5, 60, 0)), 61);
    }

    #[test]
    fn a_ladder_with_nothing_solid_under_it_is_damaged() {
        // `:1055-1061`.
        let (ladder, _) = ladder_at();
        let mut blocks = GatherBlocks::new();
        blocks.set(Pos::new(0, 60, 0), GatherBlock::Ladder);
        assert!(ladder_damaged(&blocks, ladder), "air below the ladder");
        blocks.set(Pos::new(0, 59, 0), GatherBlock::Solid);
        assert!(!ladder_damaged(&blocks, ladder));
    }

    #[test]
    fn the_shaft_offset_is_the_radius_along_the_ladder_to_cobble_vector() {
        // `:482-484`: `SHAFT_RADIUS * vector`.
        assert_eq!(shaft_offset(Pos::new(0, 60, 0), Pos::new(1, 60, 0)), (-3, 0));
        assert_eq!(shaft_offset(Pos::new(0, 60, 0), Pos::new(0, 60, -1)), (0, 3));
        assert_eq!(shaft_offset(Pos::new(4, 60, 4), Pos::new(4, 60, 4)), (0, 0));
    }

    #[test]
    fn the_ring_scan_picks_the_nearest_block_and_the_closest_air_cell() {
        // `getNextBlockInShaftToMine` (`:596-643`).
        let mut facts = shaft_facts();
        // The ladder is at x=0 and the cobble at x=1, so the 7x7 ring spans x = -6..0
        // and z = -3..3 one level under the lowest ladder (y = 59).
        facts.blocks.set(Pos::new(0, 54, 2), GatherBlock::Solid);
        facts.blocks.set(Pos::new(-2, 54, 0), GatherBlock::Solid);
        let memory = GatherMemory::default();
        let target = next_block_in_shaft(&facts, &memory);
        // Both blocks are a level too high: the ring itself is clear.
        assert_eq!(target.to_mine, None);
        assert!(target.fluids.is_empty());

        // Now put a block *in* the ring, at y = last (59).
        facts.blocks.set(Pos::new(-2, 59, 0), GatherBlock::Solid);
        let target = next_block_in_shaft(&facts, &memory);
        assert_eq!(target.to_mine, Some(Pos::new(-2, 59, 0)));
        // Its neighbours on the same level are air, so a standing cell is found.
        assert!(target.standing.is_some());
    }

    #[test]
    fn water_in_the_ring_is_reported_for_filling_before_it_is_mined() {
        // `:577-594` (the two-block margin) and `:612-616` (inside the ring).
        let mut facts = shaft_facts();
        facts.blocks.set(Pos::new(-4, 59, 0), GatherBlock::Fluid);
        let memory = GatherMemory::default();
        let target = next_block_in_shaft(&facts, &memory);
        assert!(target.fluids.contains(&Pos::new(-4, 59, 0)));
        assert_eq!(target.to_mine, Some(Pos::new(-4, 59, 0)), "a fluid is not air");
    }

    #[test]
    fn securing_is_needed_for_air_fluids_and_ore_but_not_for_a_torch_or_stone() {
        // `secureBlock` (`:760-782`).
        assert!(needs_securing(GatherBlock::Air));
        assert!(needs_securing(GatherBlock::Fluid));
        assert!(needs_securing(GatherBlock::Ore));
        assert!(!needs_securing(GatherBlock::Torch));
        assert!(!needs_securing(GatherBlock::Solid));
        assert!(!needs_securing(GatherBlock::Dirt));
    }

    #[test]
    fn an_ore_chain_walks_to_the_first_neighbour_that_is_ore() {
        // `getSurroundingOreOrDefault` (`:298-309`): down, up, north, south, west, east.
        let mut blocks = GatherBlocks::new();
        let pos = Pos::new(10, 30, 10);
        assert_eq!(surrounding_ore_or(&blocks, pos), pos, "no ore at all");
        blocks.set(Pos::new(10, 29, 10), GatherBlock::Ore);
        blocks.set(Pos::new(11, 30, 10), GatherBlock::Ore);
        assert_eq!(surrounding_ore_or(&blocks, pos), Pos::new(10, 29, 10), "down first");
        blocks.set(Pos::new(10, 29, 10), GatherBlock::Solid);
        assert_eq!(surrounding_ore_or(&blocks, pos), Pos::new(11, 30, 10));
    }

    #[test]
    fn the_node_rotation_points_at_the_parent() {
        // `:706-724`.
        let node = |at: (i32, i32), parent: (i32, i32)| NodeFacts {
            at,
            parent,
            style: NodeType::Tunnel,
            status: NodeStatus::Available,
        };
        assert_eq!(rotation_for(node((5, 0), (0, 0))), 0, "+x");
        assert_eq!(rotation_for(node((0, 0), (5, 0))), ROTATE_TWICE, "west");
        assert_eq!(rotation_for(node((0, 5), (0, 0))), ROTATE_ONCE, "south");
        assert_eq!(rotation_for(node((0, 0), (0, 5))), ROTATE_THREE_TIMES, "north");
    }

    #[test]
    fn the_shaft_check_sends_a_shallow_shaft_to_the_shaft_mining() {
        // `checkMineShaft` (`:411-434`).
        let mut facts = shaft_facts();
        facts.ladder = Some(Pos::new(0, 60, 0));
        let world = WorldFacts::new();
        let mut memory = GatherMemory::default();
        // The lowest ladder is y=59 and the limit is 48 (level 1, ladder at y=60):
        // 59 is above it, so the shaft keeps going down.
        assert_eq!(
            check_mine_shaft(&world, &facts, &mut memory).next,
            Some(GatherState::MinerMiningShaft)
        );
        // A ladder column already below the limit sends the miner to the branches. The
        // limit here is the world floor, -59 (`min_build_height` -64), so the lowest
        // ladder has to be under it.
        let mut deep = facts.clone();
        deep.ladder = Some(Pos::new(0, -68, 0));
        deep.blocks = GatherBlocks::new();
        for y in -70..=-68 {
            deep.blocks.set(Pos::new(0, y, 0), GatherBlock::Ladder);
        }
        deep.levels = 1;
        assert_eq!(
            check_mine_shaft(&world, &deep, &mut memory).next,
            Some(GatherState::MinerMiningNode),
            "the lowest ladder is y=-70, under the limit of -59"
        );
        // At the limit with no level left in the hut, the player has to upgrade it.
        deep.levels = 0;
        let decision = check_mine_shaft(&world, &deep, &mut memory);
        assert_eq!(
            decision.next,
            Some(GatherState::Base(WorkerState::Idle)),
            "`:427`"
        );
        assert_eq!(decision.refusal, Some(GatherRefusal::NeedsBetterHut));
    }

    #[test]
    fn a_mine_with_no_ladder_anchor_cannot_work() {
        // `:183-187`: the player has to move the hut.
        let mut facts = MinerFacts {
            ladder: Some(Pos::new(0, 60, 0)),
            ..MinerFacts::default()
        };
        facts.cobble = None;
        let mut world = WorldFacts::new();
        world.worker.at = Pos::new(0, 60, 0);
        world.building.position = Pos::new(0, 60, 0);
        let mut memory = GatherMemory::default();
        let decision = start_working(&world, &facts, &mut memory);
        assert_eq!(decision.next, Some(GatherState::Base(WorkerState::StartWorking)));
        assert_eq!(decision.refusal, Some(GatherRefusal::NoMineshaft));
    }

    #[test]
    fn the_ladder_repair_fills_the_back_before_the_rung() {
        // `repairLadder` (`:352-382`).
        let (ladder, cobble) = ladder_at();
        let mut facts = shaft_facts();
        facts.ladder = Some(Pos::new(0, 60, 0));
        facts.cobble = Some(cobble);
        let mut memory = GatherMemory::default();
        let world = WorldFacts::new();
        // The lowest ladder is y=59, so `repairLadder` works on y=58: the back of the
        // ladder first (`:352-363`), then the rung (`:366-382`).
        facts.blocks.set(Pos::new(1, 58, 0), GatherBlock::Air);
        assert_eq!(repair_ladder(&world, &facts, &mut memory).next, None);
        // With the back solid and the rung missing, the ladder goes in: still `getState()`.
        facts.blocks.set(Pos::new(1, 58, 0), GatherBlock::Solid);
        facts.blocks.set(Pos::new(0, 58, 0), GatherBlock::Air);
        assert_eq!(repair_ladder(&world, &facts, &mut memory).next, None);
        // Both solid: nothing to repair, back to the shaft check (`:383`).
        facts.blocks.set(Pos::new(0, 58, 0), GatherBlock::Solid);
        assert_eq!(
            repair_ladder(&world, &facts, &mut memory).next,
            Some(GatherState::MinerCheckMineshaft)
        );
        let _ = ladder;
    }

    #[test]
    fn advancing_the_ladder_needs_both_materials_then_a_safe_floor() {
        // `advanceLadder` (`:471-502`).
        let mut facts = shaft_facts();
        let mut memory = GatherMemory::default();
        facts.fill_block_available = false;
        assert_eq!(
            advance_ladder(&facts, &mut memory).refusal,
            Some(GatherRefusal::MissingShaftMaterial)
        );
        facts.fill_block_available = true;
        // The ladder column stands on solid ground, so `ladderDamaged` is false
        // (`:477-480`) and the step is considered.
        facts.blocks.set(Pos::new(0, 58, 0), GatherBlock::Solid);
        // A hole in the floor under the next step stops the advance.
        facts.blocks.set(Pos::new(-3, 57, -3), GatherBlock::Air);
        assert_eq!(advance_ladder(&facts, &mut memory).next, None);
        assert_eq!(memory.base.delay, 1, "`setDelay(1)` (`:768-770`)");
    }

    #[test]
    fn a_node_without_a_level_asks_for_one_more_tick_and_stays() {
        // `executeNodeMining` (`:671-676`).
        let mut facts = shaft_facts();
        facts.node = None;
        let mut memory = GatherMemory::default();
        let decision = execute_node_mining(&WorldFacts::new(), &facts, &mut memory);
        assert_eq!(decision.next, None);
        assert!(memory.miner.level_picked);
        // With a depth but no node, the shaft check takes over.
        facts.current_level_depth = Some(20);
        let decision = execute_node_mining(&WorldFacts::new(), &facts, &mut memory);
        assert_eq!(decision.next, Some(GatherState::MinerCheckMineshaft));
        assert_eq!(decision.refusal, Some(GatherRefusal::NoNode));
    }

    #[test]
    fn a_completed_parent_is_not_dug_again_but_an_unfinished_one_is() {
        // `searchANodeToMine` (`:732-742`).
        let mut facts = shaft_facts();
        facts.current_level_depth = Some(20);
        facts.node = Some(NodeFacts {
            at: (2, 0),
            parent: (0, 0),
            style: NodeType::Tunnel,
            status: NodeStatus::Available,
        });
        facts.node_in_level = true;
        facts.parent_node = Some(NodeFacts {
            at: (0, 0),
            parent: (0, 0),
            style: NodeType::Shaft,
            status: NodeStatus::Completed,
        });
        let mut memory = GatherMemory::default();
        // The parent is the shaft: the node itself is worked, but the walk has not
        // happened yet.
        facts.at_standing = false;
        assert_eq!(
            execute_node_mining(&WorldFacts::new(), &facts, &mut memory).next,
            Some(GatherState::Base(WorkerState::LoadStructure))
        );
        // An unfinished tunnel parent is dug first.
        facts.parent_node = Some(NodeFacts {
            at: (0, 0),
            parent: (0, 0),
            style: NodeType::Tunnel,
            status: NodeStatus::InProgress,
        });
        assert_eq!(
            execute_node_mining(&WorldFacts::new(), &facts, &mut memory).next,
            Some(GatherState::MinerCheckMineshaft)
        );
    }

    #[test]
    fn the_miner_dumps_after_level_times_sixty_four_blocks() {
        // `:222-225`.
        let scene = GatherScene::new(WorldFacts::new(), GatherJob::Miner(MinerFacts::default()));
        let mut facts = MinerFacts::default();
        for level in [1u32, 2, 3] {
            facts.building_level = level;
            let scene = GatherScene::new(WorldFacts::new(), GatherJob::Miner(facts.clone()));
            assert_eq!(actions_until_dumping(&scene), level * 64);
        }
        assert_eq!(actions_until_dumping(&scene), 0, "a level-0 hut dumps at once");
    }

    #[test]
    fn the_structure_half_of_the_miner_is_named_not_silently_dropped() {
        // `startWorkingAtOwnBuilding` (`:199-206`): a work order with a blueprint is
        // the structure table's, and the refusal says so.
        let mut facts = shaft_facts();
        facts.has_work_order = true;
        facts.has_blueprint = true;
        let mut world = WorldFacts::new();
        world.worker.at = Pos::new(0, 60, 0);
        world.building.position = Pos::new(0, 60, 0);
        let mut memory = GatherMemory::default();
        let decision = start_working(&world, &facts, &mut memory);
        assert_eq!(decision.next, Some(GatherState::Base(WorkerState::StartBuilding)));
        assert_eq!(decision.refusal, Some(GatherRefusal::StructureTableOwned));
    }

    #[test]
    fn mining_follows_the_ore_chain_and_then_hands_back_to_the_structure_step() {
        // `doMining` (`:260-290`).
        let mut facts = shaft_facts();
        let mut memory = GatherMemory::default();
        let step = Some(GatherState::Base(WorkerState::BuildingStep));
        assert_eq!(
            do_mining(&WorldFacts::new(), &facts, &mut memory).next,
            step,
            "`blockToMine == null` (`:260-263`)"
        );
        facts.block_to_mine = Some(Pos::new(10, 20, 10));
        facts.mine_done = true;
        assert_eq!(
            do_mining(&WorldFacts::new(), &facts, &mut memory).next,
            step,
            "air at the target (`:271-274`)"
        );
        facts.blocks.set(Pos::new(10, 20, 10), GatherBlock::Ore);
        facts.mine_done = false;
        assert_eq!(
            do_mining(&WorldFacts::new(), &facts, &mut memory).refusal,
            Some(GatherRefusal::MiningIncomplete),
            "`mineBlock` did not finish (`:276-280`)"
        );
        // The block is an ore and still there: the chain holds the state (`:282-286`).
        facts.mine_done = true;
        assert_eq!(do_mining(&WorldFacts::new(), &facts, &mut memory).next, None);
        // A neighbour that is also ore keeps the chain held (`:283-286`).
        facts.blocks.set(Pos::new(10, 21, 10), GatherBlock::Ore);
        assert_eq!(do_mining(&WorldFacts::new(), &facts, &mut memory).next, None);
        // Nothing left of that chain: the structure step resumes (`:288-289`). In Java
        // the mined block is air by now, so `getSurroundingOreOrDefault` answers the
        // block itself and it is not an ore; here the caller reports it as stone.
        facts.blocks.set(Pos::new(10, 21, 10), GatherBlock::Air);
        facts.blocks.set(Pos::new(10, 20, 10), GatherBlock::Solid);
        assert_eq!(
            do_mining(&WorldFacts::new(), &facts, &mut memory).next,
            step
        );
    }

    #[test]
    fn the_node_standing_position_crosses_the_shaft_when_the_parent_is_the_shaft() {
        // `getNodeMiningPosition` (`:956-980`).
        let mut facts = shaft_facts();
        facts.node = Some(NodeFacts {
            at: (10, 0),
            parent: (0, 0),
            style: NodeType::Tunnel,
            status: NodeStatus::Available,
        });
        facts.parent_node = Some(NodeFacts {
            at: (0, 0),
            parent: (0, 0),
            style: NodeType::Shaft,
            status: NodeStatus::Completed,
        });
        assert_eq!(
            node_mining_position(&facts, 20),
            Some(Pos::new(0 + (-1) * OTHER_SIDE_OF_SHAFT, 20, 0))
        );
        facts.parent_node = Some(NodeFacts {
            at: (4, 0),
            parent: (0, 0),
            style: NodeType::Tunnel,
            status: NodeStatus::Completed,
        });
        facts.node = Some(NodeFacts {
            at: (10, 0),
            parent: (4, 0),
            style: NodeType::Tunnel,
            status: NodeStatus::Available,
        });
        assert_eq!(node_mining_position(&facts, 20), Some(Pos::new(4, 20, 0)));
    }
}
