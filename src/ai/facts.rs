//! The facts a job's AI is allowed to read, and nothing else.
//!
//! Java reference: everything a worker AI touches on the way to a decision —
//! `AbstractEntityAIBasic.java` reads its `worker`, its `building`, its `world` and
//! the request system; `EntityAIWorkComposter.java` adds the barrel block entities;
//! `AbstractEntityAIStructure.java` adds the work order and the structure placer's
//! last result. This module is the *shape* of those reads, written as data.
//!
//! **Why data and not calls.** Java's AI is a class with a live `EntityCitizen`, a
//! `Level` and a `TileEntityBarrel` in hand. A wasm mod has none of that here, and a
//! decision that can only be tested inside the game cannot be tested at all — the
//! reporter of a wrong goal would have to be the game. So every field below is
//! something the caller *observed*: this port never asks the world a question,
//! it reads an answer the caller wrote down.
//!
//! **Every field says who fills it and what it means in Java.** A fact that does not
//! name a Java call is a fact this port invented, and there is exactly one class of
//! those: the two random draws (`WorkerFacts::coin`, `WorkerFacts::roll`), because
//! `worker.getRandom()` is not data a host can replay. They are marked as such.
//!
//! **Absent is not false.** [`Blocks::at`] answers [`Block::Air`] for a position
//! nobody reported; that is the answer Java gives for an unrecorded position too
//! (`Level.getBlockState` on air), except that it is *not* the answer for an
//! unloaded chunk — hence [`Blocks::unloaded`], and every rule that cares about
//! loading asks [`Blocks::is_loaded`] rather than guessing.

use core::fmt;

/// A block position. `net.minecraft.core.BlockPos` is the same three `int`s.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Pos {
    /// `BlockPos.getX`.
    pub x: i32,
    /// `BlockPos.getY`.
    pub y: i32,
    /// `BlockPos.getZ`.
    pub z: i32,
}

impl Pos {
    /// A position.
    pub const fn new(x: i32, y: i32, z: i32) -> Pos {
        Pos { x, y, z }
    }

    /// `BlockPosUtil.distSqr` (`api/util/BlockPosUtil.java`), the squared distance
    /// `EntityUtils.isLivingAtSite` compares against `range²`
    /// (`api/util/EntityUtils.java:301-305`).
    pub fn dist_sqr(self, other: Pos) -> i64 {
        let dx = (self.x - other.x) as i64;
        let dy = (self.y - other.y) as i64;
        let dz = (self.z - other.z) as i64;
        dx * dx + dy * dy + dz * dz
    }

    /// `EntityUtils.isLivingAtSite(worker, x, y, z, range)`
    /// (`api/util/EntityUtils.java:301-305`): strictly inside `range`.
    pub fn is_living_at_site(self, other: Pos, range: i32) -> bool {
        self.dist_sqr(other) < (range as i64) * (range as i64)
    }

    /// `BlockPosUtil.getDistance2D`, the horizontal distance the builder measures
    /// before it decides it is close enough to build (`EntityAIStructureBuilder.java:
    /// 256`).
    pub fn dist_2d(self, other: Pos) -> f64 {
        let dx = (self.x - other.x) as f64;
        let dz = (self.z - other.z) as f64;
        (dx * dx + dz * dz).sqrt()
    }
}

impl fmt::Display for Pos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{},{}", self.x, self.y, self.z)
    }
}

/// The five kinds of block the ported rules tell apart, named after the question
/// they answer.
///
/// Java reference: `net.minecraft.world.level.block.Blocks` for the sentinels the
/// AI compares against — `mineBlock` asks "is this air, bedrock or
/// `IBuilderUndestroyable`?" (`AbstractEntityAIInteract.java:160-166`),
/// `skipRemoval` asks the same (`AbstractEntityAIStructure.java:661-668`), and the
/// composter asks "is there a `TileEntityBarrel` here?"
/// (`EntityAIWorkComposter.java:217-219`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
// exhaustive: the five answers the ported rules ask for, not the block registry
pub enum Block {
    /// `Blocks.AIR`.
    #[default]
    Air,
    /// Water or lava: a fluid state that is not empty.
    Fluid,
    /// Any ordinary block.
    Solid,
    /// `TileEntityBarrel`, the composter's work station.
    Barrel,
    /// `Blocks.BEDROCK` or an `IBuilderUndestroyable`: nothing to mine, and nothing
    /// a builder may skip.
    Undestroyable,
}

impl Block {
    /// `state.isAir()`.
    pub fn is_air(self) -> bool {
        self == Block::Air
    }

    /// `!state.getFluidState().isEmpty()`.
    pub fn is_fluid(self) -> bool {
        self == Block::Fluid
    }

    /// What `mineBlock` answers at `AbstractEntityAIInteract.java:160-166`: an air,
    /// bedrock or undestroyable block needs no mining, so the action is already
    /// done.
    pub fn needs_no_mining(self) -> bool {
        matches!(self, Block::Air | Block::Undestroyable)
    }
}

/// What the AI has been told about blocks: its own map, filled by the caller.
///
/// Java reads the level itself; this port reads what a hook wrote down. A position
/// nobody recorded is [`Block::Air`] (Java's own answer for a position that has no
/// block), and a position the caller marks unloaded is *not* air — see
/// [`Blocks::is_loaded`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Blocks {
    entries: Vec<(Pos, Block)>,
    unloaded: Vec<Pos>,
}

impl Blocks {
    /// No blocks at all: every position reads as air, and every chunk reads loaded.
    pub fn new() -> Blocks {
        Blocks::default()
    }

    /// Report what stands at `pos`.
    pub fn set(&mut self, pos: Pos, block: Block) {
        if let Some(slot) = self.entries.iter_mut().find(|(at, _)| *at == pos) {
            slot.1 = block;
            return;
        }
        self.entries.push((pos, block));
    }

    /// What stands at `pos`, air when the caller said nothing.
    ///
    /// Java reference: `world.getBlockState(pos)` — an unrecorded position in this
    /// port is the same "nothing here" Java gets from empty space, and the one case
    /// where that is wrong is a chunk the caller marks unloaded
    /// ([`Blocks::mark_unloaded`]).
    pub fn at(&self, pos: Pos) -> Block {
        self.entries
            .iter()
            .find(|(at, _)| *at == pos)
            .map(|(_, block)| *block)
            .unwrap_or(Block::Air)
    }

    /// Mark the chunk holding `pos` as not loaded.
    ///
    /// Java reference: `WorldUtil.isBlockLoaded(world, pos)`
    /// (`AbstractEntityAIBasic.java:583`, `EntityAIWorkComposter.java:115`,
    /// `AbstractEntityAIStructureWithWorkOrder.java:496`). A mod cannot load a
    /// chunk, so the port treats "unloaded" as a fact the caller reports; every rule
    /// that reads it asks [`Blocks::is_loaded`] first.
    pub fn mark_unloaded(&mut self, pos: Pos) {
        if !self.unloaded.contains(&pos) {
            self.unloaded.push(pos);
        }
    }

    /// Whether `pos` is in a loaded chunk. Unrecorded positions are loaded.
    ///
    /// The port compares positions, not chunk keys: a caller that wants whole-chunk
    /// behaviour reports the positions the rules will read (the hut, the barrel,
    /// the work order's site), which is what every Java call site does.
    pub fn is_loaded(&self, pos: Pos) -> bool {
        !self.unloaded.contains(&pos)
    }

    /// How many positions the caller reported.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when the caller reported no block at all.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Everything `AbstractEntityAIBasic` reads about the citizen itself.
///
/// Java reference: `AbstractEntityAIBasic.initSafetyChecks` (`:446-465`),
/// `waitingForSomething` (`:497-517`), `isPaused` (`:1831-1834`),
/// `shouldRestart` (`:1877-1880`), `isStartingPaused` (`:1841-1844`),
/// `inventoryNeedsDump`'s deliveryman exemption (`:415`) and
/// `AbstractAISkeleton.canBeInterrupted` (`:126-131`, which reads the state, not a
/// field).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkerFacts {
    /// `citizen.getJob() != null && citizenData != null`: the citizen is hired.
    /// `initSafetyChecks` refuses to leave `INIT` when this is false
    /// (`AbstractEntityAIBasic.java:448-451`).
    pub hired: bool,
    /// `getExpectedBuildingClass().isInstance(citizenData.getWorkBuilding())`
    /// (`AbstractEntityAIBasic.java:191`): the citizen works in a hut of its own
    /// trade. False is the constructor's refusal (`:191-198`), not a state.
    pub job_matches_building: bool,
    /// `citizenData.getJob() != job` (`:453`): the job record on the citizen is this
    /// AI's own. If a player swaps the citizen's job while the AI runs, the two
    /// disagree and `initSafetyChecks` refuses to leave `INIT` (`:453-457`).
    pub job_is_this_ai: bool,
    /// `citizenData.isPaused()` (`:1833`).
    pub paused: bool,
    /// `citizenData.shouldRestart()` (`:1879`): a player asked for a clean restart.
    pub should_restart: bool,
    /// `worker.getNavigation().isDone()` (`:1853`): the walk finished, so a paused
    /// worker may pick a new pastime instead of walking into a wall.
    pub navigation_done: bool,
    /// `job instanceof JobDeliveryman` (`:415`): a courier never dumps, its whole
    /// trade is carrying things.
    pub deliveryman: bool,
    /// `worker.blockPosition()` — the position `isLivingAtSite` measures from
    /// (`:501-505`).
    pub at: Pos,
    /// `currentWorkingLocation` (`:116`): the block being worked on, `None` when the
    /// last action cleared it (`clearWorkTarget`, `:522-526`).
    pub working_at: Option<Pos>,
    /// **Not a Java call.** `worker.getRandom().nextBoolean()` in `dumpOneMoreSlot`
    /// (`:1260`): the port cannot replay a random draw, so the caller supplies it.
    /// `false` is the draw that keeps the item.
    pub coin: bool,
    /// **Not a Java call.** `worker.getRandom().nextInt(stackCount)` in the same
    /// method (`:1264`); meaningful only when the stack is at least
    /// [`super::worker::CHANCE_TO_DUMP_50`] items.
    pub roll: u32,
}

impl Default for WorkerFacts {
    /// A hired, unpaused worker of the right trade, standing at the origin, holding
    /// nothing, with both random draws at zero.
    fn default() -> WorkerFacts {
        WorkerFacts {
            hired: true,
            job_matches_building: true,
            job_is_this_ai: true,
            paused: false,
            should_restart: false,
            navigation_done: true,
            deliveryman: false,
            at: Pos::new(0, 64, 0),
            working_at: None,
            coin: false,
            roll: 0,
        }
    }
}

/// The citizen's own inventory, as the AI reads it.
///
/// Java reference: `worker.getInventoryCitizen()` — `isInventoryFull` (`:412`),
/// `getSlots`/`getStackInSlot` (`:1239-1249`), `openSlotCount` (`:1255`),
/// `getItemCountInItemHandler` (`:1683`), and `job.getActionsDone()` (`:413`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InventoryFacts {
    /// `InventoryCitizen.isInventoryFull()` (`:412`).
    pub full: bool,
    /// `InventoryCitizen.getSlots()` (`:1240`).
    pub slots: usize,
    /// `InventoryUtils.openSlotCount(...)` (`:1255`).
    pub open_slots: usize,
    /// `job.getActionsDone()` (`:413`), reset by `clearActionsDone` (`:1311-1314`).
    pub actions_done: u32,
    /// The count of the stack the worker is currently fetching, i.e.
    /// `InventoryUtils.getItemCountInItemHandler(inventory, needsCurrently.getA())`
    /// (`:1809`).
    pub needed_count: u32,
    /// `worker.getItemInHand(MAIN_HAND) == ItemStack.EMPTY`
    /// (`EntityAIWorkComposter.java:254`).
    pub empty_main_hand: bool,
    /// How many items of the hut's compostable list the worker carries
    /// (`EntityAIWorkComposter.java:256-258`). Zero means "go ask the hut".
    pub compostable: u32,
    /// `InventoryCitizen.hasSpace()` (`AbstractEntityAIStructure.java:346`).
    pub has_space: bool,
    /// The non-empty slots, ascending, each with what the hut wants of it. An empty
    /// vector means the inventory holds nothing the dump can move.
    pub dump: Vec<DumpSlot>,
}

impl Default for InventoryFacts {
    /// An empty inventory of 27 slots, no action done, nothing in hand.
    fn default() -> InventoryFacts {
        InventoryFacts {
            full: false,
            slots: 27,
            open_slots: 27,
            actions_done: 0,
            needed_count: 0,
            empty_main_hand: true,
            compostable: 0,
            has_space: true,
            dump: Vec::new(),
        }
    }
}

/// One non-empty slot of the worker's inventory, with what the hut wants of it.
///
/// Java reference: `AbstractEntityAIBasic.dumpOneMoreSlot` (`:1230-1294`) reads
/// `getStackInSlot(slotAt).isEmpty()` (`:1242-1250`), `stackToDump.getCount()`
/// (`:1258`, `:1264`) and `building.buildingRequiresCertainAmountOfItem(stack,
/// alreadyKept, true, jobEntry)` (`:1271`). The last one is the hut's own rule and
/// stays the caller's: this port reads the *answer*, not the rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DumpSlot {
    /// `slotAt`: the slot index.
    pub index: usize,
    /// `stackToDump.getCount()`.
    pub count: u32,
    /// How many of that stack the hut still wants, i.e. what
    /// `buildingRequiresCertainAmountOfItem` answers.
    pub wanted: u32,
}

/// What the worker's hut answers about its chests, its requests and its equipment.
///
/// Java reference: `AbstractEntityAIBasic`'s `building` reads — `hasCitizenCompleted
/// Requests` (`:236`), `hasOpenSyncRequest` (`:270`), `getPickUpPriority` (`:1179`),
/// `getMaxEquipmentLevel` (`:1086`), `getPositionOfChestWithItemStack` (`:292`),
/// `getOpenRequestsOfTypeFiltered` (`:982`), and `InventoryUtils.isBuildingFull`
/// (`:1167`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildingFacts {
    /// `citizenData.getWorkBuilding() != null` (`:191`, `:448`).
    pub exists: bool,
    /// `InventoryUtils.isBuildingFull(building)` (`:1167`): no room left in any of
    /// its chests.
    pub full: bool,
    /// `building.getPickUpPriority()` (`:1179`): above zero, a pickup request is
    /// worth creating when the worker dumps.
    pub pickup_priority: i32,
    /// `building.getMaxEquipmentLevel()` (`:1086`, `:997`).
    pub max_equipment_level: u8,
    /// `building.hasOpenSyncRequest(citizenData)` (`:270`): something the worker is
    /// waiting for is still being fetched.
    pub has_open_sync_request: bool,
    /// `building.hasCitizenCompletedRequests(citizenData)` (`:236`, `:553`).
    pub completed_requests: u32,
    /// Of those, the ones that are **not** async — `hasCitizenCompletedRequestsToPickup`
    /// (`api/colony/buildings/AbstractBuilding.java:1565-1579`), which is what
    /// `checkIfNeedsItem` reads (`:271`).
    pub completed_sync: u32,
    /// Of those, the async ones — `cleanAsync` walks exactly them
    /// (`AbstractEntityAIBasic.java:703-716`).
    pub completed_async: u32,
    /// Of those, the ones `canBeDelivered()` (`:559-567`). The rest are marked
    /// accepted and forgotten.
    pub completed_deliverable: u32,
    /// `building.getTileEntity().getPositionOfChestWithItemStack(predicate)`
    /// (`:292`): where to walk to pick the needed stack up, `None` when no chest
    /// holds one.
    pub chest_with_needed: Option<Pos>,
    /// `InventoryUtils.isEquipmentInItemHandler(inventory, tool, min, max)`
    /// (`:1088`): the worker already carries a good enough tool.
    pub citizen_has_tool: bool,
    /// `retrieveToolInHut` (`:1104-1140`): a chest of the hut holds one.
    pub container_has_tool: bool,
    /// `building.getOpenRequestsOfTypeFiltered(citizen, Tool.class, …)` (`:982`).
    pub open_tool_requests: u32,
    /// `building.getCompletedRequestsOfTypeFiltered(…, Tool.class, …)` (`:987`).
    pub completed_tool_requests: u32,
    /// `building.getPosition()` — where `walkToBuilding` walks (`:765`).
    pub position: Pos,
    /// `world.getBlockEntity(pickupLocation) != null` (`:587-591`): the hut, or the
    /// station resolver's block, is there to take the delivery from.
    pub pickup_has_entity: bool,
    /// `InventoryUtils.areAllItemsInItemHandlerList(request.getDeliveries(),
    /// validHandlers)` (`:607`): the requested items really are within reach.
    pub pickup_items_present: bool,
}

impl Default for BuildingFacts {
    /// A hut of the right trade, empty and not full, that asks for no request and
    /// carries no tool, at the origin.
    fn default() -> BuildingFacts {
        BuildingFacts {
            exists: true,
            full: false,
            pickup_priority: 0,
            max_equipment_level: 1,
            has_open_sync_request: false,
            completed_requests: 0,
            completed_sync: 0,
            completed_async: 0,
            completed_deliverable: 0,
            chest_with_needed: None,
            citizen_has_tool: false,
            container_has_tool: false,
            open_tool_requests: 0,
            completed_tool_requests: 0,
            position: Pos::new(0, 64, 0),
            pickup_has_entity: false,
            pickup_items_present: false,
        }
    }
}

/// One barrel of a composter's hut.
///
/// Java reference: `TileEntityBarrel` — `isDone()` (`EntityAIWorkComposter.java:222`),
/// `checkIfWorking()` (`:234`), `addItem` (`:284`), `retrieveCompost` (`:314`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Barrel {
    /// Where it stands in the hut.
    pub at: Pos,
    /// `isDone()`: the compost is ready; harvesting wins over filling
    /// (`:215-229`).
    pub done: bool,
    /// `checkIfWorking()`: it is already composting, so it is not fillable
    /// (`:234`).
    pub working: bool,
}

impl Barrel {
    /// A finished barrel at `at`.
    pub fn done(at: Pos) -> Barrel {
        Barrel {
            at,
            done: true,
            working: false,
        }
    }

    /// A barrel that will take compostables now.
    pub fn empty(at: Pos) -> Barrel {
        Barrel {
            at,
            done: false,
            working: false,
        }
    }

    /// A barrel that is composting and cannot be filled or harvested yet.
    pub fn busy(at: Pos) -> Barrel {
        Barrel {
            at,
            done: false,
            working: true,
        }
    }
}

/// What the composter's hut and worker say, in addition to [`BuildingFacts`].
///
/// Java reference: `EntityAIWorkComposter` — `building.getBarrels()` (`:113`, `:215`,
/// `:231`), the `compostables` item list module (`:147-152`), and
/// `building.hasWorkerOpenRequests(citizen.getId())` (`:174`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ComposterFacts {
    /// `building.getBarrels()`, in the order the hut reports them — the order is the
    /// rule: the first finished barrel is the one harvested (`:215-229`).
    pub barrels: Vec<Barrel>,
    /// The `compostables` item list is empty (`:147-152`): the worker complains and
    /// waits, it does not invent a list.
    pub compostable_list_empty: bool,
    /// `building.hasWorkerOpenRequests(citizen.getId())` (`:174`).
    pub has_worker_open_requests: bool,
    /// `worker.getCitizenSkillHandler().getLevel(primarySkill)` (`:111`), which sets
    /// how many extra barrel ticks one AI tick buys.
    pub primary_skill: u8,
}

/// The ten stages of `BuildingProgressStage`, in the order of one build.
///
/// Java reference: `core/entity/ai/workers/util/BuildingProgressStage.java:6-17`.
/// The order the placer walks them in is **not** the declaration order: it is the
/// array each work order type is given (`AbstractEntityAIStructure.java:767-786`),
/// which [`Stages::of`] reproduces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: the ten stages Java declares
pub enum Stage {
    /// `CLEAR` — break what was there before.
    Clear,
    /// `BUILD_FRAME` — the scaffold, if the blueprint tags one.
    BuildFrame,
    /// `BUILD_SOLID` — the walls and floors.
    BuildSolid,
    /// `WEAK_SOLID` — panes, fences, things that are not fully solid.
    WeakSolid,
    /// `CLEAR_WATER` — drain water inside the outline.
    ClearWater,
    /// `CLEAR_NON_SOLIDS` — remove plants and snow.
    ClearNonSolids,
    /// `DECORATE`.
    Decorate,
    /// `SPAWN` — the blueprint's entities.
    Spawn,
    /// `REMOVE` — a deconstruction.
    Remove,
    /// `REMOVE_WATER`.
    RemoveWater,
}

impl Stage {
    /// Java's identifier, which is also what a saved work order stores.
    pub fn java_name(self) -> &'static str {
        match self {
            Stage::Clear => "CLEAR",
            Stage::BuildFrame => "BUILD_FRAME",
            Stage::BuildSolid => "BUILD_SOLID",
            Stage::WeakSolid => "WEAK_SOLID",
            Stage::ClearWater => "CLEAR_WATER",
            Stage::ClearNonSolids => "CLEAR_NON_SOLIDS",
            Stage::Decorate => "DECORATE",
            Stage::Spawn => "SPAWN",
            Stage::Remove => "REMOVE",
            Stage::RemoveWater => "REMOVE_WATER",
        }
    }
}

/// The stage list a work order type gets, and the count the hut shows.
///
/// Java reference: `AbstractEntityAIStructure.java:762-786` — three arrays and three
/// `setTotalStages` calls, and they do not agree with each other: the six-stage array
/// for an upgrade reports **5**, the eight-stage array for a new build reports **6**.
/// Both are copied as they are, because the town hall's progress bar reads the
/// reported number, not the array's length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stages {
    /// The array the placer walks.
    pub stages: &'static [Stage],
    /// `building.setTotalStages(...)`, what the hut reports as the total.
    pub reported_total: u8,
}

impl Stages {
    /// A deconstruction: `{REMOVE_WATER, REMOVE}`, reported as 2 (`:767-770`).
    pub const REMOVAL: Stages = Stages {
        stages: &[Stage::RemoveWater, Stage::Remove],
        reported_total: 2,
    };
    /// A repair or an upgrade: `{BUILD_SOLID, WEAK_SOLID, CLEAR_WATER,
    /// CLEAR_NON_SOLIDS, DECORATE, SPAWN}`, reported as 5 (`:775-778`).
    pub const UPGRADE: Stages = Stages {
        stages: &[
            Stage::BuildSolid,
            Stage::WeakSolid,
            Stage::ClearWater,
            Stage::ClearNonSolids,
            Stage::Decorate,
            Stage::Spawn,
        ],
        reported_total: 5,
    };
    /// A new building: `{CLEAR, BUILD_FRAME, BUILD_SOLID, WEAK_SOLID, CLEAR_WATER,
    /// CLEAR_NON_SOLIDS, DECORATE, SPAWN}`, reported as 6 (`:782-786`).
    pub const NEW_BUILD: Stages = Stages {
        stages: &[
            Stage::Clear,
            Stage::BuildFrame,
            Stage::BuildSolid,
            Stage::WeakSolid,
            Stage::ClearWater,
            Stage::ClearNonSolids,
            Stage::Decorate,
            Stage::Spawn,
        ],
        reported_total: 6,
    };

    /// Every stage of this list, in the order the placer walks it.
    pub fn list(&self) -> &'static [Stage] {
        self.stages
    }

    /// `BuildingStructureHandler.nextStage` (`BuildingStructureHandler.java:144-147`):
    /// `++stage < stages.length`, so the last stage answers `false` and the work order
    /// is complete.
    pub fn next(&self, stage: Stage) -> Option<Stage> {
        let at = self.stages.iter().position(|known| *known == stage)?;
        self.stages.get(at + 1).copied()
    }

    /// `BuildingStructureHandler.getStage() != null && != CLEAR`
    /// (`BuildingStructureHandler.java:317`): a stage that is still work.
    pub fn is_work(&self, stage: Stage) -> bool {
        stage != Stage::Clear
    }
}

/// What kind of order a builder is working on.
///
/// Java reference: `api/colony/workorders/WorkOrderType.java` — the four the builder
/// meets, and the one rule they change: a `REMOVE` order is requested as soon as its
/// blueprint loads (`AbstractEntityAIStructureWithWorkOrder.java:117`, `:139-141`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four order types a builder's structure AI distinguishes
pub enum OrderKind {
    /// `BUILD` — a new building.
    Build,
    /// `UPGRADE` — the same hut, one level up.
    Upgrade,
    /// `REPAIR`.
    Repair,
    /// `REMOVE` — a deconstruction.
    Remove,
}

impl OrderKind {
    /// Which stage list this order walks (`AbstractEntityAIStructure.java:762-786`).
    pub fn stages(self) -> Stages {
        match self {
            OrderKind::Remove => Stages::REMOVAL,
            OrderKind::Upgrade | OrderKind::Repair => Stages::UPGRADE,
            OrderKind::Build => Stages::NEW_BUILD,
        }
    }

    /// `workOrder.getWorkOrderType() == WorkOrderType.REMOVE` (`:117`).
    pub fn is_removal(self) -> bool {
        self == OrderKind::Remove
    }
}

/// A builder's work order, as the AI reads it.
///
/// Java reference: `IBuilderWorkOrder` — `getLocation` (`EntityAIStructureBuilder.java:
/// 169`), `isRequested` (`AbstractEntityAIStructureWithWorkOrder.java:131`),
/// `getBlueprint` (`:110`), `getIteratorType` (`EntityAIStructureBuilder.java:128`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkOrder {
    /// `wo.getLocation()` — the hut being built, and where the builder walks.
    pub at: Pos,
    /// `wo.getWorkOrderType()`.
    pub kind: OrderKind,
    /// `wo.isRequested()`: the materials have been asked for once already
    /// (`:131`, `:139`).
    pub requested: bool,
    /// `wo.getBlueprint() != null`.
    pub has_blueprint: bool,
}

impl WorkOrder {
    /// A new building at `at`, blueprint loaded, materials not yet requested.
    pub fn new_build(at: Pos) -> WorkOrder {
        WorkOrder {
            at,
            kind: OrderKind::Build,
            requested: false,
            has_blueprint: true,
        }
    }
}

/// The answer of **one** structure step, which is all the AI needs from the placer.
///
/// Java reference: `BlockPlacementResult.Result` (`blockui/placement/
/// BlockPlacementResult.java`) as `AbstractEntityAIStructure.structureStep` reads it
/// (`:461-535`): `FAIL` is only logged, `FINISHED` advances the stage, `LIMIT_REACHED`
/// stores the iterator position, `MISSING_ITEMS` asks for materials, `BREAK_BLOCK`
/// sends the builder to `MINE_BLOCK`, and everything else keeps the state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the outcomes `structureStep` branches on
pub enum Placement {
    /// A block was placed, or a step taken: stay in `BUILDING_STEP` (`:517-534`).
    Placed,
    /// `FINISHED`: the stage is over (`:467-482`).
    Finished,
    /// `FAIL`: logged and treated as "not finished" (`:461-464`).
    Fail,
    /// `LIMIT_REACHED`: the step budget ran out; remember where we stopped
    /// (`:483-487`).
    LimitReached,
    /// `MISSING_ITEMS`: the blueprint wants something the builder does not have
    /// (`:507-515`).
    MissingItems,
    /// `BREAK_BLOCK`: there is a block in the way, go mine it (`:520-525`).
    BreakBlock(Pos),
}

/// Everything `EntityAIStructureBuilder` and its two parents read.
///
/// Java reference: `AbstractEntityAIStructure` (the structure AI),
/// `AbstractEntityAIStructureWithWorkOrder` (the material request) and
/// `EntityAIStructureBuilder` (the mob purge, `canGoIdle`, `isAfterDumpPickupAllowed`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuilderFacts {
    /// `building.getWorkOrder()`, `None` when the hut has none (`:98`).
    pub work_order: Option<WorkOrder>,
    /// The stage the placer is on: `building.getProgress()`'s second half
    /// (`:360-363`), `None` before the first step.
    pub stage: Option<Stage>,
    /// `building.getProgress()`'s first half: where the iterator stopped
    /// (`:357`).
    pub progress: Option<Pos>,
    /// `limitReached` (`:365`, `:372`): the previous step hit its budget, so the walk
    /// is skipped once.
    pub limit_reached: bool,
    /// The result of the step the caller just executed, `None` before any.
    pub placement: Option<Placement>,
    /// `hasListOfResInInvOrRequest(...) == RECALC` (`:509`): the material list must be
    /// recomputed, which sends the builder back to `LOAD_STRUCTURE`.
    pub recalculate: bool,
    /// `building.getNeededResources()` is empty: nothing left to fetch
    /// (`AbstractEntityAIStructureWithWorkOrder.java:213-215`).
    pub needed_resources_empty: bool,
    /// The hut's chest holds the resource `PICK_UP` is looking for
    /// (`AbstractEntityAIStructure.java:262`).
    pub needed_in_hut: bool,
    /// How many resources `PICK_UP` still wants to fetch
    /// (`neededItemsList.size()`, `:251`).
    pub pickup_pending: usize,
    /// The building the work order points at still exists in the colony
    /// (`EntityAIStructureBuilder.java:115-120`) — Java asks the colony's building
    /// manager, which is not a fact this AI can read itself.
    pub order_building_exists: bool,
    /// `building.hasPurgedMobsToday()` (`EntityAIStructureBuilder.java:182`).
    pub purged_mobs_today: bool,
    /// `building.getBuildingLevel() >= LEVEL_TO_PURGE_MOBS` (`:168`).
    pub level_purges_mobs: bool,
    /// `worker.getCitizenData().isRequestAsync(requestId)` (`AbstractEntityAIBasic.java:
    /// 594`) — whether the pickup in flight was asked for asynchronously.
    pub pickup_is_async: bool,
    /// `structurePlacer != null` (`AbstractEntityAIStructure.java:294`, `:1057`): the
    /// blueprint is loaded into a placer this worker holds.
    pub placer: bool,
    /// `loadingBlueprint` (`AbstractEntityAIStructure.java:101`): the asynchronous
    /// load is in flight, so `LOAD_STRUCTURE` must wait
    /// (`AbstractEntityAIStructureWithWorkOrder.java:113-116`).
    pub loading_blueprint: bool,
    /// `walkToConstructionSite(block)` (`EntityAIStructureBuilder.java:208-270`):
    /// true once the builder stands where it can reach the block.
    pub at_construction_site: bool,
    /// `mineBlock(blockToMine, workFrom)` (`AbstractEntityAIInteract.java:151-243`):
    /// true once the block is actually broken — which takes two calls, because the
    /// first one only starts the swing (`:139-145`).
    pub mining_done: bool,
}

impl Default for BuilderFacts {
    /// A hut with no work order, no stage, and nothing to fetch — the idle builder.
    fn default() -> BuilderFacts {
        BuilderFacts {
            work_order: None,
            stage: None,
            progress: None,
            limit_reached: false,
            placement: None,
            recalculate: false,
            needed_resources_empty: true,
            needed_in_hut: false,
            pickup_pending: 0,
            order_building_exists: true,
            purged_mobs_today: false,
            level_purges_mobs: false,
            pickup_is_async: false,
            placer: false,
            loading_blueprint: false,
            at_construction_site: false,
            mining_done: false,
        }
    }
}

/// The job-specific half of the facts.
///
/// A job whose AI is not ported gets [`JobFacts::Unported`], not an empty
/// [`JobFacts::Composter`]: the difference between "this worker has no barrels
/// today" and "this port cannot say what this worker does" must not be lost.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the two jobs this slice gives a table to, plus "not ported"
pub enum JobFacts {
    /// A composter's hut and barrels.
    Composter(ComposterFacts),
    /// A builder's work order and structure.
    Builder(BuilderFacts),
    /// This job's AI is not ported; the base table still runs, and this is how a
    /// caller knows not to expect a trade-specific goal.
    Unported,
}

impl Default for JobFacts {
    fn default() -> JobFacts {
        JobFacts::Unported
    }
}

/// The whole scene one worker AI decision reads.
///
/// This is the port's `WorldFacts`: a worker, its inventory, its hut, its blocks, and
/// the facts of its trade. [`Scene`] in `worker` pairs it with the AI's own working
/// memory, which is not a fact about the world.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorldFacts {
    /// The citizen.
    pub worker: WorkerFacts,
    /// Its inventory.
    pub inventory: InventoryFacts,
    /// Its hut.
    pub building: BuildingFacts,
    /// The blocks the rules will ask about.
    pub blocks: Blocks,
    /// Its trade.
    pub job: JobFacts,
}

impl WorldFacts {
    /// A hired worker of the right trade, at the origin, with an empty hut and an
    /// empty inventory — the shortest way to write one fact.
    pub fn new() -> WorldFacts {
        WorldFacts::default()
    }

    /// The composter's facts, if this is a composter.
    pub fn composter(&self) -> Option<&ComposterFacts> {
        match &self.job {
            JobFacts::Composter(facts) => Some(facts),
            _ => None,
        }
    }

    /// The builder's facts, if this is a builder.
    pub fn builder(&self) -> Option<&BuilderFacts> {
        match &self.job {
            JobFacts::Builder(facts) => Some(facts),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_position_measures_the_way_the_java_helpers_do() {
        let origin = Pos::new(0, 64, 0);
        assert_eq!(origin.dist_sqr(Pos::new(3, 68, 0)), 9 + 16);
        // `EntityUtils.isLivingAtSite` is a strict comparison
        // (`EntityUtils.java:301-305`).
        assert!(origin.is_living_at_site(Pos::new(3, 64, 0), 4));
        assert!(!origin.is_living_at_site(Pos::new(4, 64, 0), 4));
        // `getDistance2D` ignores Y (`EntityAIStructureBuilder.java:256`).
        assert_eq!(Pos::new(0, 64, 0).dist_2d(Pos::new(3, 200, 4)), 5.0);
    }

    #[test]
    fn an_unreported_block_is_air_and_an_unloaded_one_is_not_air() {
        // Java asks the level; this port asks what the caller wrote down, and the
        // difference between "empty" and "not loaded" is `is_loaded`.
        let mut blocks = Blocks::new();
        assert_eq!(blocks.at(Pos::new(1, 2, 3)), Block::Air);
        assert!(blocks.is_loaded(Pos::new(1, 2, 3)));

        blocks.set(Pos::new(1, 2, 3), Block::Solid);
        assert_eq!(blocks.at(Pos::new(1, 2, 3)), Block::Solid);
        assert_eq!(blocks.len(), 1);

        blocks.mark_unloaded(Pos::new(1, 2, 3));
        assert!(!blocks.is_loaded(Pos::new(1, 2, 3)));
        assert_eq!(blocks.at(Pos::new(1, 2, 3)), Block::Solid, "still reported");
    }

    #[test]
    fn only_air_and_bedrock_need_no_mining() {
        // `AbstractEntityAIInteract.java:160-166`.
        assert!(Block::Air.needs_no_mining());
        assert!(Block::Undestroyable.needs_no_mining());
        assert!(!Block::Solid.needs_no_mining());
        assert!(!Block::Barrel.needs_no_mining());
        assert!(!Block::Fluid.needs_no_mining(), "a fluid is removed, not mined");
    }

    #[test]
    fn the_three_stage_arrays_are_copied_with_their_two_disagreeing_counts() {
        // `AbstractEntityAIStructure.java:767-786`: 2/2, 6 stages reported as 5, and
        // 8 stages reported as 6.
        assert_eq!(Stages::REMOVAL.list().len(), 2);
        assert_eq!(Stages::REMOVAL.reported_total, 2);
        assert_eq!(Stages::UPGRADE.list().len(), 6);
        assert_eq!(Stages::UPGRADE.reported_total, 5);
        assert_eq!(Stages::NEW_BUILD.list().len(), 8);
        assert_eq!(Stages::NEW_BUILD.reported_total, 6);
        assert_eq!(OrderKind::Build.stages(), Stages::NEW_BUILD);
        assert_eq!(OrderKind::Upgrade.stages(), Stages::UPGRADE);
        assert_eq!(OrderKind::Repair.stages(), Stages::UPGRADE);
        assert_eq!(OrderKind::Remove.stages(), Stages::REMOVAL);
    }

    #[test]
    fn the_last_stage_has_no_successor_and_the_first_one_is_clear() {
        // `BuildingStructureHandler.nextStage` (`:144-147`).
        assert_eq!(
            Stages::NEW_BUILD.next(Stage::Clear),
            Some(Stage::BuildFrame)
        );
        assert_eq!(Stages::NEW_BUILD.next(Stage::Spawn), None);
        assert_eq!(Stages::REMOVAL.next(Stage::Remove), None);
        assert_eq!(Stages::REMOVAL.next(Stage::RemoveWater), Some(Stage::Remove));
        // A stage that is not in the list has no successor either.
        assert_eq!(Stages::REMOVAL.next(Stage::Decorate), None);
        // `is_work` is false only for CLEAR (`BuildingStructureHandler.java:317`).
        assert!(!Stages::NEW_BUILD.is_work(Stage::Clear));
        assert!(Stages::NEW_BUILD.is_work(Stage::BuildSolid));
        assert_eq!(Stage::ClearNonSolids.java_name(), "CLEAR_NON_SOLIDS");
    }

    #[test]
    fn a_barrel_is_harvestable_when_done_and_fillable_only_when_not_working() {
        let at = Pos::new(4, 65, 0);
        assert!(Barrel::done(at).done);
        assert!(!Barrel::done(at).working);
        assert!(!Barrel::empty(at).done);
        assert!(!Barrel::empty(at).working);
        assert!(Barrel::busy(at).working, "a working barrel is not fillable");
    }

    #[test]
    fn the_default_world_is_a_hired_worker_in_the_right_hut_with_nothing_to_do() {
        let facts = WorldFacts::new();
        assert!(facts.worker.hired);
        assert!(facts.worker.job_matches_building);
        assert!(!facts.worker.paused);
        assert!(!facts.worker.deliveryman);
        assert!(!facts.inventory.full);
        assert_eq!(facts.inventory.slots, 27);
        assert!(facts.building.exists);
        assert!(!facts.building.full);
        assert_eq!(facts.building.max_equipment_level, 1);
        assert_eq!(facts.building.pickup_priority, 0);
        assert!(facts.blocks.is_empty());
        // A job whose AI is not ported is not the same as "no facts today".
        assert_eq!(facts.job, JobFacts::Unported);
        assert!(facts.composter().is_none());
        assert!(facts.builder().is_none());
    }
}
