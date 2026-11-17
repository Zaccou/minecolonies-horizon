//! The farmer: hoe a field, plant it, harvest it, cell by cell.
//!
//! Java reference: `core/entity/ai/workers/production/agriculture/
//! EntityAIWorkFarmer.java` — the constructor's `registerTargets` (`:136-142`),
//! `decide` (`:180-189`), `prepareForFarming` (`:209-289`), `checkIfShouldExecute`
//! (`:313-329`), `canGoPlanting` (`:337-362`), `findHoeableSurface` (`:371-414`),
//! `getSurfacePos` (`:422-458`), `nextValidCell` (`:465-505`), `getLargestCell`
//! (`:507-510`), `workAtField` (`:517-589`), `hoeIfAble` (`:598-618`),
//! `harvestIfAble` (`:661-678`), `getLevelDelay` (`:690-693`), `tryToPlant`
//! (`:702-706`), `findPlantableSurface` (`:723-738`), `plantCrop` (`:747-774`),
//! `findHarvestableSurface` (`:782-859`) and `canGoIdle` (`:911-920`), on top of
//! `colony/buildingextensions/FarmField.java`.
//!
//! ## The table this adds
//!
//! | # | State | Rule | Rate | Java |
//! | ---: | --- | --- | ---: | --- |
//! | 1 | `PREPARING` | `prepareForFarming` | 20 | `:137` |
//! | 2 | `FARMER_HOE` | `workAtField` | 5 | `:138` |
//! | 3 | `FARMER_PLANT` | `workAtField` | 5 | `:139` |
//! | 4 | `FARMER_HARVEST` | `workAtField` | 5 | `:140` |
//! | 5 | `START_WORKING` | `decide` (overridden) | 5 | `:180-189` |
//!
//! **Three states, one method.** `FARMER_HOE`, `FARMER_PLANT` and `FARMER_HARVEST` all
//! run `workAtField` (`:138-140`), which switches on `getState()` (`:536-563`). The port
//! keeps Java's shape — three targets, one function taking the state — because the
//! walk, the rate and the offset bookkeeping are shared and only the middle step
//! differs.
//!
//! **The field is walked along a spiral, and the spiral is arithmetic.** `nextValidCell`
//! (`:465-505`) turns a cell index into an `(x, z)` offset by walking rings outward from
//! the centre, in the order south, west, north, east, and skipping whatever falls
//! outside the four radii the player set in the field's GUI. That is a pure function of
//! an integer and four radii — [`next_valid_cell`].
//!
//! **Compost.** A crop that is not ripe is grown with compost before it is cut
//! (`:804-826`): one compost leaves the inventory, `growCrops` runs, the block is read
//! again. The port cannot grow a crop, so [`find_harvestable_surface`] answers
//! [`HarvestSurface::NeedsCompost`] and the caller grows it; the next tick sees a ripe
//! crop.
//!
//! **Two questions the port cannot ask.** `isRightFarmLandForCrop` (`:643-653`) tells
//! an `ItemCrop`'s own farmland from `Blocks.FARMLAND`, and `getToolModifiedState` with
//! `HOE_TILL` (`:407-411`) asks whether the hoe turns a block into farmland. Both are
//! answered here by the block vocabulary — `Farmland` and `Dirt` — which is exact for
//! vanilla and approximate for a mod that adds a third farmland; `MANQUANT.md` §2 says
//! so.

use crate::ai::facts::{Pos, WorldFacts};
use crate::ai::state::WorkerState;

use super::facts::{FarmerFacts, FieldFacts, FieldStage, GatherBlock, GatherMemory};
use super::state::{GatherRefusal, GatherState};
use super::{gather_action, Decision, TICKS_SECOND};

/// `EntityAIWorkFarmer.MAX_BLOCKS_MINED` (`:80`): `getActionsDoneUntilDumping()`
/// (`:197-201`).
pub const MAX_BLOCKS_MINED: u32 = 64;
/// `DEFAULT_DELAY` (`:85`): the delay at skill level zero.
pub const DEFAULT_DELAY: u32 = 40;
/// `SMALLEST_DELAY` (`:90`): the floor `getLevelDelay` clamps to.
pub const SMALLEST_DELAY: u32 = 1;
/// `DELAY_DIVIDER` (`:95`): the divisor of the skill bonus.
pub const DELAY_DIVIDER: f64 = 1.0;
/// `XP_PER_HARVEST` (`:100`): the experience one harvested crop is worth.
pub const XP_PER_HARVEST: f64 = 0.5;
/// `MAX_DEPTH` (`:105`): how far `getSurfacePos` walks up or down looking for ground.
pub const MAX_DEPTH: i32 = 5;
/// The rate of the three field states (`:138-140`).
pub const FIELD_DELAY: u32 = 5;
/// `Constants.STACKSIZE` (`api/util/constant/Constants.java:23`): the compost request
/// size (`:234`, `:239`).
pub const STACKSIZE: u32 = 64;
/// `FarmField.MAX_RANGE` (`FarmField.java:40`): the largest radius a field can have.
pub const MAX_RANGE: i32 = 20;

/// `getLargestCell` (`:507-510`): `(MAX_RANGE * 2 + 1)²`, the number of cells a field
/// is scanned for before it is declared finished.
pub const LARGEST_CELL: i32 = (MAX_RANGE * 2 + 1) * (MAX_RANGE * 2 + 1);

/// `Direction.from2DDataValue`'s four values (`:483`), the order the spiral walks:
/// south, west, north, east.
pub const SPIRAL_ORDER: [&str; 4] = ["south", "west", "north", "east"];

/// What [`next_valid_cell`] answered: the cell index it reached, and the offset it landed
/// on — `None` once the whole field has been walked (`:477-480`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellStep {
    /// `building.setCell(...)` (`:477`): the index the scan stopped at.
    pub cell: i32,
    /// `building.setWorkingOffset(...)` (`:318`, `:568`): `(x, z)` relative to the field,
    /// or `None` when `cell` reached [`LARGEST_CELL`].
    pub offset: Option<(i32, i32)>,
}

/// `nextValidCell` (`:465-505`) and `getLargestCell` (`:507-510`), as arithmetic.
///
/// `radius` is N, E, S, W in that order ([`FieldFacts::radius`]). The loop is Java's
/// `do … while`: a cell outside the four radii is skipped and the next index tried, so a
/// field with a hole produces a contiguous walk of the rest.
///
/// **The centre cell is never worked.** Ring one starts at index 0, and index 0 maps to
/// `(1, 1)`, not to `(0, 0)` — the eight cells of ring one are indices 0…7 and the
/// origin is not one of them. That is Java's arithmetic (`:481-495`), not a port slip:
/// a farmer leaves the block under the field's own position alone. [`CellStep::offset`]
/// is what the decision uses, so the port keeps it.
///
/// **`>=`, not `==`, and no wraparound.** Java tests `setCell(...) == getLargestCell()`,
/// so a caller that asks *again* after the `null` walks past the end of the field
/// forever — with `int` arithmetic that wraps in silence, and in a debug Rust build it
/// panics on overflow. A field that has been walked once answers `None` from then on:
/// the comparison is `>=` and the index saturates.
pub fn next_valid_cell(cell: i32, radius: [i32; 4]) -> CellStep {
    let mut cell = cell;
    loop {
        cell = cell.saturating_add(1);
        if cell >= LARGEST_CELL {
            return CellStep { cell, offset: None }; // :477-480
        }
        // :481 — `Math.max(1, floor((sqrt(cell + 1) + 1) / 2))`.
        let ring = 1.max(((((cell + 1) as f64).sqrt() + 1.0) / 2.0) as i32);
        // :482 — the index inside the ring.
        let ring_cell = cell - (4 * (ring - 1) * (ring - 1) + 4 * (ring - 1));
        // :483 — `Direction.from2DDataValue(floorDiv(ringCell, 2 * ring))`.
        let facing = ring_cell.div_euclid(2 * ring).rem_euclid(4);
        let edge = ring - ring_cell.rem_euclid(2 * ring);
        let (x, z) = match facing {
            0 => (edge, ring),   // SOUTH: +z
            1 => (-ring, edge),  // WEST: -x
            2 => (-edge, -ring), // NORTH: -z
            _ => (ring, -edge),  // EAST: +x
        };
        // :497-502 — `-z > N || x > E || z > S || -x > W` sends it round again.
        if -z > radius[0] || x > radius[1] || z > radius[2] || -x > radius[3] {
            continue;
        }
        return CellStep {
            cell,
            offset: Some((x, z)),
        };
    }
}

/// `farmField.getPosition().below().south(offset.z).east(offset.x)` (`:324`, `:527`).
pub fn cell_position(field: &FieldFacts, offset: (i32, i32)) -> Pos {
    Pos::new(
        field.position.x + offset.0,
        field.position.y - 1,
        field.position.z + offset.1,
    )
}

/// `getSurfacePos` (`:422-458`): walk up or down at most [`MAX_DEPTH`] blocks looking for
/// the ground the field sits on.
///
/// Java tests `state.isSolid()` and excludes pumpkin, melon and cobweb; a fluid counts
/// as ground. The port tests [`GatherBlock::blocks_motion`] with the same exclusions
/// plus the crop kinds, whose states are not solid either.
pub fn surface_pos(blocks: &super::GatherBlocks, pos: Pos, depth: i32) -> Option<Pos> {
    if depth.abs() > MAX_DEPTH || !blocks.is_loaded(pos) {
        return None; // :436-439
    }
    let block = blocks.at(pos);
    if is_solid_surface(block) || block.is_fluid() {
        if depth < 0 {
            return Some(pos); // :444-447
        }
        return surface_pos(blocks, Pos::new(pos.x, pos.y + 1, pos.z), depth + 1);
    }
    if depth > 0 {
        return Some(Pos::new(pos.x, pos.y - 1, pos.z)); // :452-455
    }
    surface_pos(blocks, Pos::new(pos.x, pos.y - 1, pos.z), depth - 1)
}

/// `state.isSolid() && !(PumpkinBlock) && !(MelonBlock) && !(WebBlock)` (`:442`), with
/// the crop kinds added: a growing crop's state is not solid either, and a field of
/// crops must not read as its own ground.
fn is_solid_surface(block: GatherBlock) -> bool {
    block.blocks_motion()
        && !matches!(
            block,
            GatherBlock::PumpkinOrMelon
                | GatherBlock::Crop { .. }
                | GatherBlock::Scarecrow
                | GatherBlock::Sapling
        )
}

/// What [`find_harvestable_surface`] found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: Java's three answers — cut, grow first, nothing here
pub enum HarvestSurface {
    /// A ripe crop, a pumpkin or a melon: cut the block above `pos` (`:792-802`).
    Harvestable(Pos),
    /// A crop that is not ripe, with compost in the inventory: `growCrops` first
    /// (`:804-826`), which is the caller's.
    NeedsCompost(Pos),
    /// Nothing here.
    Nothing,
}

/// `findHarvestableSurface` (`:782-859`).
///
/// A pumpkin or a melon is harvested whatever its age (`:792-795`); an ordinary or
/// MineColonies crop only when `isMaxAge` (`:800`, `:830`), and otherwise with compost,
/// which grows it by one stage (`:804-826`). The port answers which of the three and
/// lets the caller cut or grow.
pub fn find_harvestable_surface(facts: &FarmerFacts, pos: Pos) -> HarvestSurface {
    let Some(ground) = surface_pos(&facts.blocks, pos, 0) else {
        return HarvestSurface::Nothing; // :784-788
    };
    let above = Pos::new(ground.x, ground.y + 1, ground.z);
    match facts.blocks.at(above) {
        GatherBlock::PumpkinOrMelon => HarvestSurface::Harvestable(ground), // :792-795
        GatherBlock::Crop { ripe: true } => HarvestSurface::Harvestable(ground), // :800-802
        GatherBlock::Crop { ripe: false } => {
            if facts.compost_in_inventory == 0 {
                return HarvestSurface::Nothing; // :804-808
            }
            HarvestSurface::NeedsCompost(ground) // :810-826
        }
        _ => HarvestSurface::Nothing,
    }
}

/// `findHoeableSurface` (`:371-414`).
///
/// Five ways to answer `null`, in Java's order: no ground (`:374-377`), not part of the
/// field, a crop above, a scarecrow above, ground that is not dirt/farmland, or ground
/// that is *already* the right farmland (`:379-388`). The hoe test (`:407-411`) is the
/// `Dirt` tag: in vanilla the hoe tills exactly the dirt-tagged blocks the same `if`
/// already tests.
pub fn find_hoeable_surface(facts: &FarmerFacts, field: &FieldFacts, pos: Pos) -> Option<Pos> {
    let ground = surface_pos(&facts.blocks, pos, 0)?; // :373-377
    let above = Pos::new(ground.x, ground.y + 1, ground.z);
    let above_block = facts.blocks.at(above);
    let ground_block = facts.blocks.at(ground);
    if !field.part_of_field // :379
        || matches!(above_block, GatherBlock::Crop { .. }) // `CropBlock` `:380`
        || above_block == GatherBlock::Scarecrow // :381
        || !matches!(
            ground_block,
            GatherBlock::Dirt | GatherBlock::Farmland | GatherBlock::Solid
        ) // :382
        || is_right_farmland_for_crop(ground_block) // :383
    {
        return None;
    }
    // :407-411 — the hoe has to be able to turn it into farmland.
    if ground_block != GatherBlock::Dirt {
        return None;
    }
    Some(ground) // :413
}

/// `findPlantableSurface` (`:723-738`).
pub fn find_plantable_surface(facts: &FarmerFacts, field: &FieldFacts, pos: Pos) -> Option<Pos> {
    let ground = surface_pos(&facts.blocks, pos, 0)?; // :725-726
    let above = Pos::new(ground.x, ground.y + 1, ground.z);
    let above_block = facts.blocks.at(above);
    if !field.part_of_field // :727
        || matches!(above_block, GatherBlock::Crop { .. }) // `CropBlock`/`StemBlock` `:728-729`
        || above_block == GatherBlock::Scarecrow // :730
        || !is_right_farmland_for_crop(facts.blocks.at(ground)) // :731
    {
        return None;
    }
    Some(ground) // :737
}

/// `isRightFarmLandForCrop` (`:643-653`): an `ItemCrop` wants its own farmland,
/// every other seed wants `Blocks.FARMNLAND`. Both are `Farmland` here — see the module
/// note and `MANQUANT.md` §2.
fn is_right_farmland_for_crop(ground: GatherBlock) -> bool {
    ground == GatherBlock::Farmland
}

/// `getLevelDelay` (`:690-693`): `max(1, 40 - primarySkill / 2)`.
pub fn level_delay(primary_skill: u32) -> u32 {
    let delay = DEFAULT_DELAY as f64 - ((primary_skill as f64 / 2.0) * DELAY_DIVIDER);
    SMALLEST_DELAY.max(delay as u32)
}

/// `canGoIdle` (`:911-920`): a farmer with a field never idles, and its own
/// `hasWorkToDo` answers `true` (`:191-195`), so `!hasWorkToDo` is always false.
pub fn can_go_idle() -> bool {
    false
}

/// `decide` (`:180-189`): `AbstractEntityAICrafting.decide` (`:271-291`) with `IDLE`
/// mapped to `PREPARING`.
///
/// The crafting half of the parent — `QUERY_ITEMS`, `GET_RECIPE`, `CRAFT` — is not
/// ported and is never reached by a farmer: `getNextCraftingState` answers `IDLE` when
/// the job has no current task (`:300-302`), and a farmer's job has no crafting module.
pub fn decide(facts: &WorldFacts, trade: &FarmerFacts, memory: &mut GatherMemory) -> Decision {
    let _ = (trade, memory);
    // `:281-284` — `!walkToBuilding()`.
    if !super::at_hut(facts) {
        return Decision::go(GatherState::Base(WorkerState::StartWorking));
    }
    // `:286-290` — "wait to dump before continuing".
    if facts.inventory.actions_done >= MAX_BLOCKS_MINED {
        return Decision::go_after(
            GatherState::Base(WorkerState::StartWorking),
            GatherRefusal::InventoryDumpDue,
        );
    }
    // `:184-187` — `IDLE` becomes `PREPARING`.
    Decision::go(GatherState::Base(WorkerState::Preparing))
}

/// `prepareForFarming` (`:209-289`).
pub fn prepare_for_farming(
    facts: &WorldFacts,
    trade: &FarmerFacts,
    memory: &mut GatherMemory,
) -> Decision {
    let _ = facts;
    // :212-216 — a hut nobody built cannot farm.
    if trade.building_level < 1 {
        return Decision::refuse(GatherRefusal::NoField);
    }
    // :224-241 — compost: none anywhere means a request, none *carried* means a walk.
    if trade.compost_in_building + trade.compost_in_inventory == 0 {
        return Decision::refuse(GatherRefusal::NoCompost); // :227-236
    }
    if trade.compost_in_inventory == 0 && trade.compost_in_building > 0 {
        return Decision::go(GatherState::Base(WorkerState::GatheringRequiredMaterials)); // :239-240
    }
    // :243-251 — a hut with no field at all.
    let Some(field) = trade.field.as_ref() else {
        return Decision::go_after(GatherState::Base(WorkerState::Idle), GatherRefusal::NoField);
    };
    if !trade.has_extensions {
        return Decision::go_after(GatherState::Base(WorkerState::Idle), GatherRefusal::NoField);
    }
    // :256-260 — the hoe comes before the field.
    if !trade.has_hoe {
        return Decision::refuse(GatherRefusal::ToolMissing);
    }
    // :263-274 — the stage picks the work, and the work has to exist somewhere.
    match field.stage {
        FieldStage::Planted => {
            let executable = first_executable_cell(trade, field, &mut memory.farmer, |facts, pos| {
                match find_harvestable_surface(facts, pos) {
                    HarvestSurface::Nothing => None,
                    HarvestSurface::Harvestable(pos) => Some(pos),
                    HarvestSurface::NeedsCompost(pos) => Some(pos),
                }
            });
            if executable {
                return Decision::go(GatherState::FarmerHarvest);
            }
        }
        FieldStage::Hoed => return can_go_planting(field),
        FieldStage::Empty => {
            let executable = first_executable_cell(trade, field, &mut memory.farmer, |facts, pos| {
                find_hoeable_surface(facts, field, pos)
            });
            if executable {
                return Decision::go(GatherState::FarmerHoe);
            }
        }
    }
    // :275-282 — nothing to do on this field: move it on, and drop it after four
    // skipped stages.
    memory.farmer.skipped_state += 1;
    if memory.farmer.skipped_state >= 4 {
        memory.farmer.skipped_state = 0;
        memory.farmer.did_work = true;
    }
    Decision::go_after(GatherState::Base(WorkerState::Idle), GatherRefusal::FieldSkipped)
}

/// `canGoPlanting` (`:337-362`): the field wants a seed the farmer must carry.
pub fn can_go_planting(field: &FieldFacts) -> Decision {
    // :339-342 — the field has no seed chosen.
    if !field.has_seed {
        return Decision::go(GatherState::Base(WorkerState::Preparing));
    }
    // :344-349 — the seed is in the inventory.
    if field.seed_slot.is_some() {
        return Decision::go(GatherState::FarmerPlant);
    }
    // :351-361 — walk to the hut and ask for it; the field state moves on.
    Decision::go_after(
        GatherState::Base(WorkerState::Preparing),
        GatherRefusal::NoSeed,
    )
}

/// `checkIfShouldExecute` (`:313-329`): walk the spiral until `predicate` holds.
fn first_executable_cell<F>(
    facts: &FarmerFacts,
    field: &FieldFacts,
    memory: &mut super::FarmerMemory,
    predicate: F,
) -> bool
where
    F: Fn(&FarmerFacts, Pos) -> Option<Pos>,
{
    loop {
        let step = next_valid_cell(memory.cell, field.radius); // :318
        memory.cell = step.cell;
        let Some(offset) = step.offset else {
            memory.working_offset = None; // :319-322
            return false;
        };
        memory.working_offset = Some(offset);
        if predicate(facts, cell_position(field, offset)).is_some() {
            return true; // :328
        }
    }
}

/// `workAtField` (`:517-589`), the body of all three field states — `state` is
/// `getState()`, which Java switches on at `:536`.
pub fn work_at_field(
    facts: &WorldFacts,
    trade: &FarmerFacts,
    memory: &mut GatherMemory,
    state: GatherState,
) -> Decision {
    let _ = facts;
    let Some(field) = trade.field.as_ref() else {
        return Decision::go(GatherState::Base(WorkerState::Idle)); // :584-587
    };
    // :525-566 — the cell the last tick picked.
    if let Some(offset) = memory.farmer.working_offset {
        let position = cell_position(field, offset);
        // :530-533 — still walking.
        if !trade.at_cell {
            return Decision::refuse(GatherRefusal::NotAtField);
        }
        // :536-563 — the state decides the middle step.
        let step = match state {
            GatherState::FarmerHoe => hoe_if_able(trade, field, position, &mut memory.farmer),
            GatherState::FarmerPlant => {
                match find_plantable_surface(trade, field, position) {
                    // `:547-550` — nothing to plant there: back to `PREPARING`.
                    None => return Decision::go(GatherState::Base(WorkerState::Preparing)),
                    Some(ground) => plant_crop(trade, ground, &mut memory.farmer),
                }
            }
            GatherState::FarmerHarvest => harvest_if_able(trade, position, &mut memory.farmer),
            // :559-562 — any other state is a caller mistake Java answers with
            // `PREPARING`.
            _ => return Decision::go(GatherState::Base(WorkerState::Preparing)),
        };
        if !step {
            return Decision::stay();
        }
        // :564-565 — `setDelay(getLevelDelay())`.
        memory.base.delay = level_delay(trade.primary_skill) as i32;
    }
    // :568-582 — the next cell, or the field is finished.
    let step = next_valid_cell(memory.farmer.cell, field.radius);
    memory.farmer.cell = step.cell;
    memory.farmer.working_offset = step.offset;
    if step.offset.is_none() {
        memory.farmer.should_dump_inventory = true; // :571
        memory.farmer.skipped_state += 1; // `++skippedState` `:574`
        if memory.farmer.did_work || memory.farmer.skipped_state >= 4 {
            memory.farmer.skipped_state = 0; // :576-578
        }
        memory.farmer.did_work = false; // :579
        return Decision::go(GatherState::Base(WorkerState::Idle)); // :581
    }
    Decision::stay() // :588
}

/// `hoeIfAble` (`:598-618`): till the ground, or say this cell is done anyway.
fn hoe_if_able(
    trade: &FarmerFacts,
    field: &FieldFacts,
    position: Pos,
    memory: &mut super::FarmerMemory,
) -> bool {
    let Some(_ground) = find_hoeable_surface(trade, field, position) else {
        return true; // :600-601 — nothing to hoe here
    };
    if !trade.has_hoe {
        return true; // `checkForToolOrWeapon` answered true: the request is out
    }
    // :603-614 — `mineBlock(position.above())` has to finish.
    if !trade.mine_done {
        return false;
    }
    memory.did_work = true;
    true
}

/// `harvestIfAble` (`:661-678`).
fn harvest_if_able(
    trade: &FarmerFacts,
    position: Pos,
    memory: &mut super::FarmerMemory,
) -> bool {
    match find_harvestable_surface(trade, position) {
        HarvestSurface::Nothing => true, // :663-665, nothing here
        HarvestSurface::NeedsCompost(_) => true, // the caller grows it (`:810-826`)
        HarvestSurface::Harvestable(_) => {
            if trade.mine_done {
                memory.did_work = true; // :666-671
                true
            } else {
                false // :672-675
            }
        }
    }
}

/// `plantCrop` (`:747-774`).
fn plant_crop(trade: &FarmerFacts, ground: Pos, memory: &mut super::FarmerMemory) -> bool {
    let _ = ground;
    // :749-751 — no seed at all.
    if !trade.field.as_ref().map(|field| field.has_seed).unwrap_or(false) {
        return false;
    }
    // :753-757 — the seed is not carried.
    let Some(field) = trade.field.as_ref() else {
        return false;
    };
    if field.seed_slot.is_none() {
        return false;
    }
    // :763-766 — a melon or a pumpkin whose stem already stands next door: nothing to
    // do, but the tick counts as done.
    if trade.seed_is_melon_or_pumpkin && trade.stem_on_prev_pos {
        return true;
    }
    memory.did_work = true; // :771
    true // :773
}

/// `FARMER_HOE`'s body: `workAtField` with `getState() == FARMER_HOE`.
pub fn hoe_step(facts: &WorldFacts, trade: &FarmerFacts, memory: &mut GatherMemory) -> Decision {
    work_at_field(facts, trade, memory, GatherState::FarmerHoe)
}

/// `FARMER_PLANT`'s body.
pub fn plant_step(facts: &WorldFacts, trade: &FarmerFacts, memory: &mut GatherMemory) -> Decision {
    work_at_field(facts, trade, memory, GatherState::FarmerPlant)
}

/// `FARMER_HARVEST`'s body.
pub fn harvest_step(facts: &WorldFacts, trade: &FarmerFacts, memory: &mut GatherMemory) -> Decision {
    work_at_field(facts, trade, memory, GatherState::FarmerHarvest)
}

gather_action!(prepare_action, Farmer, prepare_for_farming);
gather_action!(decide_action, Farmer, decide);
gather_action!(hoe_action, Farmer, hoe_step);
gather_action!(plant_action, Farmer, plant_step);
gather_action!(harvest_action, Farmer, harvest_step);

/// Register the five targets of `EntityAIWorkFarmer.java:136-142` plus the
/// `START_WORKING` override.
pub fn register(machine: &mut crate::ai::state::Machine<GatherState, super::GatherScene>) {
    // :137 — `new AITarget(PREPARING, this::prepareForFarming, TICKS_SECOND)`.
    super::add_target(
        machine,
        GatherState::Base(WorkerState::Preparing),
        "prepareForFarming",
        "EntityAIWorkFarmer.java:137",
        TICKS_SECOND,
        prepare_action,
    );
    // :138 — `new AITarget(FARMER_HOE, this::workAtField, 5)`.
    super::add_target(
        machine,
        GatherState::FarmerHoe,
        "workAtField",
        "EntityAIWorkFarmer.java:138",
        FIELD_DELAY,
        hoe_action,
    );
    // :139 — `new AITarget(FARMER_PLANT, this::workAtField, 5)`.
    super::add_target(
        machine,
        GatherState::FarmerPlant,
        "workAtField",
        "EntityAIWorkFarmer.java:139",
        FIELD_DELAY,
        plant_action,
    );
    // :140 — `new AITarget(FARMER_HARVEST, this::workAtField, 5)`.
    super::add_target(
        machine,
        GatherState::FarmerHarvest,
        "workAtField",
        "EntityAIWorkFarmer.java:140",
        FIELD_DELAY,
        harvest_action,
    );
    // `AbstractEntityAICrafting.java:166` — `new AITarget(START_WORKING, this::decide,
    // STANDARD_DELAY)`, with `decide` overridden at `EntityAIWorkFarmer.java:180-189`.
    super::add_target(
        machine,
        GatherState::Base(WorkerState::StartWorking),
        "decide",
        "EntityAIWorkFarmer.java:180-189",
        super::STANDARD_DELAY,
        decide_action,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_gather::facts::GatherBlocks;

    fn field(stage: FieldStage) -> FieldFacts {
        FieldFacts {
            position: Pos::new(0, 65, 0),
            stage,
            radius: [3, 3, 3, 3],
            has_seed: true,
            seed_slot: Some(4),
            seed_is_crop: false,
            part_of_field: true,
        }
    }

    fn farmer(field: FieldFacts) -> FarmerFacts {
        FarmerFacts {
            building_level: 1,
            field: Some(field),
            has_extensions: true,
            compost_in_building: 2,
            compost_in_inventory: 2,
            has_hoe: true,
            primary_skill: 20,
            ..FarmerFacts::default()
        }
    }

    #[test]
    fn the_level_delay_shrinks_by_half_a_tick_per_skill_point() {
        // `getLevelDelay` (`:690-693`): `max(1, 40 - primary/2)`.
        assert_eq!(level_delay(0), 40);
        assert_eq!(level_delay(10), 35);
        assert_eq!(level_delay(78), 1, "40 - 39 = 1");
        assert_eq!(level_delay(100), 1, "the floor is SMALLEST_DELAY");
    }

    #[test]
    fn the_spiral_starts_one_to_the_south_east_and_skips_the_centre() {
        // `nextValidCell` (`:465-505`) from `cell = -1` (`:470-473`): index 0 is ring 1,
        // `Direction.from2DDataValue(0)` = south, edge = 1, so the first offset is
        // `(1, 1)` — never `(0, 0)`, which is the block under the field's position.
        let first = next_valid_cell(-1, [5, 5, 5, 5]);
        assert_eq!(first.cell, 0);
        assert_eq!(first.offset, Some((1, 1)));
        // Then the rest of the ring: index 1 is the south edge without the corner,
        // index 2 is the west corner, index 3 the west edge.
        assert_eq!(next_valid_cell(0, [5, 5, 5, 5]).offset, Some((0, 1)));
        assert_eq!(next_valid_cell(1, [5, 5, 5, 5]).offset, Some((-1, 1)));
        assert_eq!(next_valid_cell(2, [5, 5, 5, 5]).offset, Some((-1, 0)));
        // Eight cells to the first ring, and the origin is not one of them.
        for cell in 0..8 {
            assert_ne!(next_valid_cell(cell, [5, 5, 5, 5]).offset, Some((0, 0)));
        }
    }

    #[test]
    fn the_spiral_skips_the_cells_outside_the_four_radii() {
        // `:497-502` — `-z > N || x > E || z > S || -x > W`.
        let step = next_valid_cell(0, [0, 1, 0, 1]);
        let (x, z) = step.offset.expect("a cell inside the box");
        assert_eq!(z, 0, "no south radius, so z is zero");
        assert!(x.abs() <= 1);
        assert!(step.cell >= 3, "cells 1 and 2 were skipped");
    }
    #[test]
    fn a_field_of_the_largest_size_ends_at_the_forty_first_ring() {
        // `getLargestCell` (`:507-510`): `(20 * 2 + 1)² = 1681`.
        assert_eq!(LARGEST_CELL, 1681);
        assert_eq!(MAX_RANGE, 20);
        let step = next_valid_cell(LARGEST_CELL - 1, [20, 20, 20, 20]);
        assert_eq!(step.cell, LARGEST_CELL);
        assert_eq!(step.offset, None);
    }

    #[test]
    fn the_surface_walk_stops_five_blocks_away() {
        // `getSurfacePos` (`:434-439`): `Math.abs(depth) > MAX_DEPTH` answers `null`.
        let air = GatherBlocks::new();
        assert_eq!(surface_pos(&air, Pos::new(0, 65, 0), 0), None);
        let mut ground = GatherBlocks::new();
        ground.set(Pos::new(0, 60, 0), GatherBlock::Dirt);
        assert_eq!(
            surface_pos(&ground, Pos::new(0, 65, 0), 0),
            Some(Pos::new(0, 60, 0)),
            "five blocks down is still inside MAX_DEPTH"
        );
        let mut far = GatherBlocks::new();
        far.set(Pos::new(0, 59, 0), GatherBlock::Dirt);
        assert_eq!(surface_pos(&far, Pos::new(0, 65, 0), 0), None, "six is too far");
        // An unloaded chunk is not "no ground", it is "cannot say" (`:436`).
        let mut unloaded = GatherBlocks::new();
        unloaded.set(Pos::new(0, 64, 0), GatherBlock::Dirt);
        unloaded.mark_unloaded(Pos::new(0, 65, 0));
        assert_eq!(surface_pos(&unloaded, Pos::new(0, 65, 0), 0), None);
    }

    #[test]
    fn a_ripe_crop_is_harvested_and_an_unripe_one_needs_compost() {
        // `findHarvestableSurface` (`:792-826`).
        let mut facts = farmer(field(FieldStage::Planted));
        facts.blocks.set(Pos::new(0, 64, 0), GatherBlock::Dirt);
        facts.blocks.set(Pos::new(0, 65, 0), GatherBlock::Crop { ripe: true });
        assert_eq!(
            find_harvestable_surface(&facts, Pos::new(0, 65, 0)),
            HarvestSurface::Harvestable(Pos::new(0, 64, 0))
        );
        facts.blocks.set(Pos::new(0, 65, 0), GatherBlock::Crop { ripe: false });
        assert_eq!(
            find_harvestable_surface(&facts, Pos::new(0, 65, 0)),
            HarvestSurface::NeedsCompost(Pos::new(0, 64, 0)),
            "two compost in the inventory"
        );
        facts.compost_in_inventory = 0;
        assert_eq!(
            find_harvestable_surface(&facts, Pos::new(0, 65, 0)),
            HarvestSurface::Nothing,
            "`:804-808`"
        );
    }

    #[test]
    fn a_pumpkin_is_harvested_without_an_age_test() {
        // `:792-795`.
        let mut facts = farmer(field(FieldStage::Planted));
        facts.blocks.set(Pos::new(0, 64, 0), GatherBlock::Dirt);
        facts.blocks.set(Pos::new(0, 65, 0), GatherBlock::PumpkinOrMelon);
        assert_eq!(
            find_harvestable_surface(&facts, Pos::new(0, 65, 0)),
            HarvestSurface::Harvestable(Pos::new(0, 64, 0))
        );
    }

    #[test]
    fn dirt_is_hoeable_but_farmland_and_stone_are_not() {
        // `findHoeableSurface` (`:379-388`).
        let empty = field(FieldStage::Empty);
        let mut facts = farmer(empty.clone());
        facts.blocks.set(Pos::new(0, 64, 0), GatherBlock::Dirt);
        assert_eq!(
            find_hoeable_surface(&facts, &empty, Pos::new(0, 65, 0)),
            Some(Pos::new(0, 64, 0))
        );
        facts.blocks.set(Pos::new(0, 64, 0), GatherBlock::Farmland);
        assert_eq!(
            find_hoeable_surface(&facts, &empty, Pos::new(0, 65, 0)),
            None,
            "`:383` — already the right farmland"
        );
        facts.blocks.set(Pos::new(0, 64, 0), GatherBlock::Ore);
        assert_eq!(find_hoeable_surface(&facts, &empty, Pos::new(0, 65, 0)), None);
    }

    #[test]
    fn a_plantable_cell_needs_farmland_and_nothing_above_it() {
        // `findPlantableSurface` (`:723-738`).
        let hoed = field(FieldStage::Hoed);
        let mut facts = farmer(hoed.clone());
        facts.blocks.set(Pos::new(0, 64, 0), GatherBlock::Farmland);
        assert_eq!(
            find_plantable_surface(&facts, &hoed, Pos::new(0, 65, 0)),
            Some(Pos::new(0, 64, 0))
        );
        facts.blocks.set(Pos::new(0, 65, 0), GatherBlock::Crop { ripe: false });
        assert_eq!(
            find_plantable_surface(&facts, &hoed, Pos::new(0, 65, 0)),
            None,
            "`:728-729` — a crop is already there"
        );
        facts.blocks.set(Pos::new(0, 65, 0), GatherBlock::Air);
        facts.blocks.set(Pos::new(0, 64, 0), GatherBlock::Dirt);
        assert_eq!(
            find_plantable_surface(&facts, &hoed, Pos::new(0, 65, 0)),
            None,
            "`:731` — dirt is not farmland yet"
        );
    }

    #[test]
    fn a_field_with_no_compost_asks_for_it_before_anything_else() {
        // `prepareForFarming` (`:224-241`).
        let mut facts = farmer(field(FieldStage::Empty));
        facts.compost_in_building = 0;
        facts.compost_in_inventory = 0;
        let mut memory = GatherMemory::default();
        assert_eq!(
            prepare_for_farming(&WorldFacts::new(), &facts, &mut memory).refusal,
            Some(GatherRefusal::NoCompost)
        );
        // Compost in the hut but not carried: go and fetch it.
        facts.compost_in_building = 3;
        assert_eq!(
            prepare_for_farming(&WorldFacts::new(), &facts, &mut memory).next,
            Some(GatherState::Base(WorkerState::GatheringRequiredMaterials))
        );
    }

    #[test]
    fn a_hut_with_no_field_reports_it_and_idles() {
        // `:243-251`.
        let mut facts = farmer(field(FieldStage::Empty));
        facts.has_extensions = false;
        facts.field = None;
        let mut memory = GatherMemory::default();
        let decision = prepare_for_farming(&WorldFacts::new(), &facts, &mut memory);
        assert_eq!(decision.next, Some(GatherState::Base(WorkerState::Idle)));
        assert_eq!(decision.refusal, Some(GatherRefusal::NoField));
        // A hut nobody built has no field either (`:212-216`).
        let mut unbuilt = farmer(field(FieldStage::Empty));
        unbuilt.building_level = 0;
        assert_eq!(
            prepare_for_farming(&WorldFacts::new(), &unbuilt, &mut memory).refusal,
            Some(GatherRefusal::NoField)
        );
    }

    #[test]
    fn the_field_stage_picks_the_state() {
        // `:263-274` — planted → harvest, hoed → plant, empty → hoe. Each case gets a
        // fresh memory: the spiral does not go back, so a shared cell index would send
        // the second case past the cell the first one consumed.
        let mut planted = farmer(field(FieldStage::Planted));
        // The first cell the spiral reaches is `(1, 1)` from the field's position.
        planted.blocks.set(Pos::new(1, 64, 1), GatherBlock::Dirt);
        planted.blocks.set(Pos::new(1, 65, 1), GatherBlock::Crop { ripe: true });
        assert_eq!(
            prepare_for_farming(&WorldFacts::new(), &planted, &mut GatherMemory::default()).next,
            Some(GatherState::FarmerHarvest)
        );
        assert_eq!(
            prepare_for_farming(
                &WorldFacts::new(),
                &farmer(field(FieldStage::Hoed)),
                &mut GatherMemory::default()
            )
            .next,
            Some(GatherState::FarmerPlant)
        );
        let mut empty = farmer(field(FieldStage::Empty));
        empty.blocks.set(Pos::new(1, 64, 1), GatherBlock::Dirt);
        assert_eq!(
            prepare_for_farming(&WorldFacts::new(), &empty, &mut GatherMemory::default()).next,
            Some(GatherState::FarmerHoe)
        );
    }

    #[test]
    fn four_skipped_stages_drop_the_field_and_reset_the_counter() {
        // `:275-281` — `if (++skippedState >= 4)`.
        let mut facts = farmer(field(FieldStage::Empty));
        // A field whose whole box is unloaded: no cell is executable (`:319-322`).
        for x in -3..=3 {
            for z in -3..=3 {
                facts.blocks.mark_unloaded(Pos::new(x, 64, z));
            }
        }
        let mut memory = GatherMemory::default();
        for expected in [1u32, 2, 3] {
            let decision = prepare_for_farming(&WorldFacts::new(), &facts, &mut memory);
            assert_eq!(decision.refusal, Some(GatherRefusal::FieldSkipped));
            assert_eq!(memory.farmer.skipped_state, expected);
            assert!(!memory.farmer.did_work);
        }
        let decision = prepare_for_farming(&WorldFacts::new(), &facts, &mut memory);
        assert_eq!(decision.refusal, Some(GatherRefusal::FieldSkipped));
        assert_eq!(memory.farmer.skipped_state, 0, "four in a row reset it");
        assert!(memory.farmer.did_work, "`:279`");
    }

    #[test]
    fn planting_without_a_seed_goes_back_to_preparing() {
        // `canGoPlanting` (`:337-362`).
        let no_seed = FieldFacts {
            has_seed: false,
            ..field(FieldStage::Hoed)
        };
        assert_eq!(
            can_go_planting(&no_seed).next,
            Some(GatherState::Base(WorkerState::Preparing))
        );
        assert_eq!(
            can_go_planting(&field(FieldStage::Hoed)).next,
            Some(GatherState::FarmerPlant)
        );
        let not_carried = FieldFacts {
            seed_slot: None,
            ..field(FieldStage::Hoed)
        };
        let decision = can_go_planting(&not_carried);
        assert_eq!(decision.next, Some(GatherState::Base(WorkerState::Preparing)));
        assert_eq!(decision.refusal, Some(GatherRefusal::NoSeed));
    }

    #[test]
    fn a_finished_field_dumps_the_inventory_and_goes_idle() {
        // `workAtField` (`:568-582`).
        let facts = farmer(field(FieldStage::Empty));
        let mut memory = GatherMemory::default();
        memory.farmer.cell = LARGEST_CELL - 1;
        memory.farmer.working_offset = None;
        memory.farmer.did_work = true;
        let decision = work_at_field(&WorldFacts::new(), &facts, &mut memory, GatherState::FarmerHoe);
        assert_eq!(decision.next, Some(GatherState::Base(WorkerState::Idle)));
        assert!(memory.farmer.should_dump_inventory, "`:571`");
        assert_eq!(memory.farmer.skipped_state, 0, "`:576-578` resets it after work");
        assert!(!memory.farmer.did_work);
    }

    #[test]
    fn a_farmer_still_walking_keeps_its_cell_and_says_so() {
        // `:530-533` — `!walkToSafePos(position.above())` keeps `getState()`.
        let facts = farmer(field(FieldStage::Empty));
        let mut memory = GatherMemory::default();
        memory.farmer.working_offset = Some((1, 1));
        memory.farmer.cell = 1;
        let decision = work_at_field(&WorldFacts::new(), &facts, &mut memory, GatherState::FarmerHoe);
        assert_eq!(decision.next, None);
        assert_eq!(decision.refusal, Some(GatherRefusal::NotAtField));
        assert_eq!(memory.farmer.working_offset, Some((1, 1)), "the cell is kept");
    }

    #[test]
    fn a_planted_cell_records_work_and_waits_the_level_delay() {
        // `:545-551`, `:564-565`.
        let mut facts = farmer(field(FieldStage::Hoed));
        facts.at_cell = true;
        facts.blocks.set(Pos::new(1, 64, 1), GatherBlock::Farmland);
        let mut memory = GatherMemory::default();
        memory.farmer.working_offset = Some((1, 1));
        let decision = work_at_field(&WorldFacts::new(), &facts, &mut memory, GatherState::FarmerPlant);
        assert_eq!(decision.next, None, "the next cell is picked, state holds");
        assert!(memory.farmer.did_work);
        assert_eq!(memory.base.delay, level_delay(20) as i32, "`:565`");
    }

    #[test]
    fn a_farmer_never_hands_its_citizen_back_to_idle_by_itself() {
        // `canGoIdle` (`:911-920`) with `hasWorkToDo` always true (`:191-195`).
        assert!(!can_go_idle());
    }
}
