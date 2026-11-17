//! What a harvest goal asks the world, written as observations.
//!
//! Java reference: the four trade classes and the three helpers they lean on —
//! `EntityAIWorkLumberjack.java` + `util/Tree.java`, `EntityAIStructureMiner.java` +
//! `util/MineNode.java` + `building/workerbuildings/BuildingMiner.java`,
//! `agriculture/EntityAIWorkFarmer.java` + `colony/buildingextensions/FarmField.java`,
//! `agriculture/EntityAIWorkFisherman.java` + `api/util/Pond.java`.
//!
//! **Why the base half is reused and the trade half is not.** The citizen, its
//! inventory, its hut and the block map already exist in `crate::ai::facts`, and this
//! module reads them from there ([`WorldFacts`], [`WorkerFacts`], [`InventoryFacts`],
//! [`BuildingFacts`], [`Pos`]). What does not exist is the trade half: `WorldFacts`
//! carries a `JobFacts` enum with exactly two variants (composter, builder) and that
//! file is outside this module's write boundary, so a gather trade's facts live in
//! [`GatherJob`] and [`GatherScene`] *wraps* `WorldFacts` instead of extending it.
//! Nothing is copied: `GatherScene::facts` is the same `WorldFacts` value.
//!
//! **Why a second block vocabulary.** [`super::super::ai::facts::Block`] answers the
//! five questions the builder and the composter ask ("is it air, a fluid, a barrel,
//! undestroyable, or an ordinary block"). A forester asks `BlockTags.LOGS`, a miner
//! asks `CompatibilityManager.isOre`, a farmer asks `CropBlock.isMaxAge`. Those are
//! tag questions that five-value enum cannot hold, so [`GatherBlock`] is the harvest
//! vocabulary and [`GatherBlocks`] the same shape of map [`crate::ai::facts::Blocks`]
//! is (unrecorded position answers air, a marked chunk is not air). Merging the two
//! vocabularies would need a change in `src/ai/facts.rs`, which is written down in
//! `MANQUANT.md` §2 rather than done here.
//!
//! **Absent is not false.** An unrecorded position reads [`GatherBlock::Air`], the
//! same answer Java gets from empty space; a chunk the caller marked unloaded is not
//! air, and every rule that cares asks [`GatherBlocks::is_loaded`] first.

use core::fmt;

use crate::ai::facts::{BuildingFacts, InventoryFacts, Pos, WorldFacts};
use crate::ai::worker::Memory;

/// The block questions the four harvest rules ask, and no others.
///
/// Java reference: `net.minecraft.world.level.block` + the tags the trades read —
/// `BlockTags.LOGS` (`EntityAIWorkLumberjack.java:271`), `BlockTags.LEAVES` (`:683`),
/// `BlockTags.SAPLINGS` (`:362`, `:652`), `BlockTags.DIRT` and `Blocks.FARMLAND`
/// (`EntityAIWorkFarmer.java:382`), `CropBlock`/`StemBlock`/`BlockScarecrow`
/// (`:380-384`, `:728-732`), `Blocks.PUMPKIN`/`Blocks.MELON` (`:792`),
/// `state.isLadder` (`EntityAIStructureMiner.java:366`),
/// `CompatibilityManager.isOre` (`:266`), `Blocks.TORCH` (`:763`) and
/// `Blocks.NETHERRACK` (`EntityAIWorkLumberjack.java:926`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
// exhaustive: the tags and blocks these four trades compare against
pub enum GatherBlock {
    /// `Blocks.AIR` / `state.isAir()`.
    #[default]
    Air,
    /// An ordinary solid block.
    Solid,
    /// Water or lava: a non-empty fluid state (`:561`, `:588`).
    Fluid,
    /// `BlockTags.LOGS`: what a forester cuts (`EntityAIWorkLumberjack.java:271`).
    Log,
    /// `BlockTags.LEAVES`: what `shouldDefoliate` removes (`:498`, `:556`).
    Leaves,
    /// `BlockTags.SAPLINGS`: what a forester replants (`:652`, `:806`).
    Sapling,
    /// `state.isLadder(...)`: the miner's shaft (`EntityAIStructureMiner.java:366`).
    Ladder,
    /// `CompatibilityManager.isOre(state)`: mined first, kept with silk touch
    /// (`:266`, `:283`, `:295`).
    Ore,
    /// `BlockTags.DIRT`: hoeable ground (`EntityAIWorkFarmer.java:382`).
    Dirt,
    /// `Blocks.FARMLAND` or a `MinecoloniesFarmland`: already tilled (`:382`).
    Farmland,
    /// A crop (`CropBlock`, `MinecoloniesCropBlock`, `StemBlock`), with its age.
    Crop {
        /// `CropBlock.isMaxAge(state)` / `MinecoloniesCropBlock.isMaxAge` (`:800`,
        /// `:830`).
        ripe: bool,
    },
    /// `Blocks.PUMPKIN` or `Blocks.MELON`: harvested without an age test (`:792`).
    PumpkinOrMelon,
    /// `BlockScarecrow`: never hoed or planted over (`:381`, `:730`).
    Scarecrow,
    /// `Blocks.TORCH`: `secureBlock` leaves it, everything else non-solid is filled
    /// (`EntityAIStructureMiner.java:763`).
    Torch,
    /// `Blocks.NETHERRACK`: the ladder's back fill in the nether
    /// (`EntityAIWorkLumberjack.java:926`, `EntityAIStructureMiner.java:405`).
    Netherrack,
}

impl GatherBlock {
    /// `state.isAir()`.
    pub fn is_air(self) -> bool {
        self == GatherBlock::Air
    }

    /// `!state.getFluidState().isEmpty()` (`EntityAIWorkLumberjack.java:561`).
    pub fn is_fluid(self) -> bool {
        self == GatherBlock::Fluid
    }

    /// What a forester cuts: a log (`:271`).
    pub fn is_log(self) -> bool {
        self == GatherBlock::Log
    }

    /// What `shouldDefoliate` removes (`:498`).
    pub fn is_leaves(self) -> bool {
        self == GatherBlock::Leaves
    }

    /// `state.blocksMotion()`, inverted: air, a fluid and a torch do not block, which
    /// is exactly the test `secureBlock` makes (`EntityAIStructureMiner.java:763`).
    pub fn blocks_motion(self) -> bool {
        !matches!(
            self,
            GatherBlock::Air | GatherBlock::Fluid | GatherBlock::Torch
        )
    }

    /// Whether this block is a ripe crop or a pumpkin/melon — `findHarvestableSurface`
    /// (`EntityAIWorkFarmer.java:792-802`).
    pub fn is_harvestable(self) -> bool {
        matches!(
            self,
            GatherBlock::PumpkinOrMelon | GatherBlock::Crop { ripe: true }
        )
    }

    /// Whether this block is already the tilled ground the field wants
    /// (`isRightFarmLandForCrop`, `:643-653`).
    pub fn is_farmland(self) -> bool {
        self == GatherBlock::Farmland
    }
}

impl fmt::Display for GatherBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GatherBlock::Air => f.write_str("air"),
            GatherBlock::Solid => f.write_str("solid"),
            GatherBlock::Fluid => f.write_str("fluid"),
            GatherBlock::Log => f.write_str("log"),
            GatherBlock::Leaves => f.write_str("leaves"),
            GatherBlock::Sapling => f.write_str("sapling"),
            GatherBlock::Ladder => f.write_str("ladder"),
            GatherBlock::Ore => f.write_str("ore"),
            GatherBlock::Dirt => f.write_str("dirt"),
            GatherBlock::Farmland => f.write_str("farmland"),
            GatherBlock::Crop { ripe } => {
                f.write_str(if *ripe { "crop (ripe)" } else { "crop (growing)" })
            }
            GatherBlock::PumpkinOrMelon => f.write_str("pumpkin or melon"),
            GatherBlock::Scarecrow => f.write_str("scarecrow"),
            GatherBlock::Torch => f.write_str("torch"),
            GatherBlock::Netherrack => f.write_str("netherrack"),
        }
    }
}

/// What the caller observed about blocks: the same map as
/// [`crate::ai::facts::Blocks`], with the harvest vocabulary.
///
/// Java reads the level; this port reads what a hook wrote down, because a decision
/// that can only be tested inside the game cannot be tested at all.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GatherBlocks {
    entries: Vec<(Pos, GatherBlock)>,
    unloaded: Vec<Pos>,
}

impl GatherBlocks {
    /// Nothing reported: every position reads air, every chunk reads loaded.
    pub fn new() -> GatherBlocks {
        GatherBlocks::default()
    }

    /// Report what stands at `pos` (`world.getBlockState(pos)`).
    pub fn set(&mut self, pos: Pos, block: GatherBlock) {
        if let Some(slot) = self.entries.iter_mut().find(|(at, _)| *at == pos) {
            slot.1 = block;
            return;
        }
        self.entries.push((pos, block));
    }

    /// What stands at `pos`; air when nobody reported it.
    pub fn at(&self, pos: Pos) -> GatherBlock {
        self.entries
            .iter()
            .find(|(at, _)| *at == pos)
            .map(|(_, block)| *block)
            .unwrap_or(GatherBlock::Air)
    }

    /// Mark the chunk holding `pos` as not loaded (`WorldUtil.isBlockLoaded`,
    /// `EntityAIWorkFarmer.java:436`).
    pub fn mark_unloaded(&mut self, pos: Pos) {
        if !self.unloaded.contains(&pos) {
            self.unloaded.push(pos);
        }
    }

    /// Whether `pos` is in a loaded chunk. Unreported positions are loaded.
    pub fn is_loaded(&self, pos: Pos) -> bool {
        !self.unloaded.contains(&pos)
    }

    /// How many positions were reported.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing was reported.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// `PathResult`'s four answers, which three trades read (`EntityAIWorkLumberjack.java:
/// 398-434`, `EntityAIWorkFisherman.java:446-465`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: PathResult's isComputing / isDone / isCancelled, plus "no job yet"
pub enum PathState {
    /// `pathResult == null`: no search started (`EntityAIWorkLumberjack.java:402`).
    #[default]
    None,
    /// `isComputing()` (`:398`).
    Computing,
    /// `isDone()` (`:428`).
    Done,
    /// `isCancelled()` (`EntityAIWorkFisherman.java:461`).
    Cancelled,
}

/// One tree, as `util/Tree.java` holds it.
///
/// The queues are reported as **lengths plus their head**, because the decision only
/// ever asks "is there a log left, and which one is next" — `hasLogs` (`Tree.java:887`),
/// `peekNextLog` (`:857`), `pollNextLog` (`:827`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TreeFacts {
    /// `Tree.getLocation()` (`:952`): the block the sapling goes back on.
    pub location: Pos,
    /// `Tree.isTree()` (`:1032`): the search found a tree, not a lone log.
    pub is_tree: bool,
    /// How many logs are left, i.e. `woodBlocks.size()` behind `hasLogs` (`:887`).
    pub logs: u32,
    /// How many leaves are left, behind `hasLeaves` (`:877`).
    pub leaves: u32,
    /// `peekNextLog()` (`:857`), the log the next `mineBlock` targets.
    pub next_log: Option<Pos>,
    /// `peekNextLeaf()` (`:867`).
    pub next_leaf: Option<Pos>,
    /// `getStumpLocations()` (`:922`): where the sapling must go back.
    pub stumps: Vec<Pos>,
    /// `Tree.isNetherTree()` (`:911`): leaves are broken whatever the setting.
    pub nether: bool,
    /// `Tree.isDynamicTree()` (`:903`): the Dynamic Trees compatibility path.
    pub dynamic: bool,
}

impl TreeFacts {
    /// `hasLogs()` (`Tree.java:887`).
    pub fn has_logs(&self) -> bool {
        self.logs > 0
    }

    /// `hasLeaves()` (`Tree.java:877`).
    pub fn has_leaves(&self) -> bool {
        self.leaves > 0
    }
}

/// Everything `EntityAIWorkLumberjack` reads that the base facts do not carry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LumberjackFacts {
    /// `job.getTree()` (`:375`, `:483`).
    pub tree: Option<TreeFacts>,
    /// The tree search job's state (`:398-434`).
    pub path: PathState,
    /// `pathResult.treeLocation` (`:439`): where the search says a tree is.
    pub tree_candidate: Option<Pos>,
    /// `building.shouldRestrict()` (`:328`, `:404`, `:500`).
    pub restrict: bool,
    /// `building.getStartRestriction()` / `getEndRestriction()`, the area the search
    /// and `chopTree` confine themselves to (`:406-407`, `:500`).
    pub restriction: Option<(Pos, Pos)>,
    /// `building.shouldDefoliate()` (`:498`).
    pub defoliate: bool,
    /// `building.shouldReplant()` (`:526`).
    pub replant: bool,
    /// `building.getModule(ITEMLIST_SAPLING).getList().size() <
    /// getNumberOfSaplings()` (`:243-244`): the sapling list is not filled in yet.
    pub sapling_list_incomplete: bool,
    /// `job.getTaskQueue().isEmpty()` (`:249`, `:377`).
    pub task_queue_empty: bool,
    /// `job.getCurrentTask() == null` (`:254`).
    pub no_current_task: bool,
    /// `getItemsForPickUp()` non-empty (`:337`, `:989`).
    pub items_on_ground: u32,
    /// `findSaplingSlot()` (`:894-905`): the first slot holding **the tree's own**
    /// sapling, `None` when there is none. The item comparison
    /// (`ItemStack.isSameItem`, `:972`) is the caller's, because it is an item
    /// identity and not a block.
    pub sapling_slot: Option<usize>,
    /// Whether `checkAndTransferFromHut` (`:862`) brought a sapling back.
    pub sapling_in_hut: bool,
    /// `walkToTree(...)`'s answer (`:509-516`): at the stump, within
    /// `MIN_WORKING_RANGE`.
    pub at_tree: bool,
    /// `mineBlock(...)`'s answer for the log or leaf of this tick (`:557`, `:589`).
    pub mine_done: bool,
    /// `isOnSapling()` (`:804-809`): the spawn point has to move (`:545-552`).
    pub on_sapling: bool,
    /// `checkForToolOrWeapon(ModEquipmentTypes.axe)` answered **true**, i.e. a request
    /// was created because no axe good enough is in the inventory or the hut (`:478`,
    /// `AbstractEntityAIBasic.java:993-1002`).
    pub missing_axe: bool,
    /// The shears/hoe half of `prepareForWoodcutting` (`:295`), same convention.
    pub missing_secondary_tool: bool,
    /// **Not a Java call.** `ColonyConstants.rand.nextInt(100)` in `gathering2`
    /// (`:346`); the port cannot replay a static random, so the caller draws it.
    pub draw_100: u32,
}

/// `MineNode.NodeType` (`util/MineNode.java:388-402`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: the eight styles the blueprint table declares
pub enum NodeType {
    /// `SHAFT` (`:391`) — the main shaft.
    #[default]
    Shaft,
    /// `LADDER_BACK` (`:393`).
    LadderBack,
    /// `TUNNEL` (`:395`).
    Tunnel,
    /// `CROSSROAD` (`:397`).
    Crossroad,
    /// `BEND_RIGHT` (`:399`).
    BendRight,
    /// `BEND_LEFT` (`:402`).
    BendLeft,
    /// `UNDEFINED` (`:401`).
    Undefined,
}

/// `MineNode.NodeStatus` (`util/MineNode.java:375-383`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: three statuses in Java
pub enum NodeStatus {
    /// `AVAILABLE` (`:377`): not built yet.
    #[default]
    Available,
    /// `IN_PROGRESS` (`:379`).
    InProgress,
    /// `COMPLETED` (`:381`).
    Completed,
}

/// One node of a mining level, as `MineNode` holds it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NodeFacts {
    /// `getX()` / `getZ()`, the node's column.
    pub at: (i32, i32),
    /// `getParent()` (`EntityAIStructureMiner.java:708-711`).
    pub parent: (i32, i32),
    /// `getStyle()` (`:734`).
    pub style: NodeType,
    /// `getStatus()` (`:689`, `:752`).
    pub status: NodeStatus,
}

/// Everything `EntityAIStructureMiner` reads that the base facts do not carry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MinerFacts {
    /// `building.getLadderLocation()` (`:178`, `:346`).
    pub ladder: Option<Pos>,
    /// `building.getCobbleLocation()` (`:183`, `:346`).
    pub cobble: Option<Pos>,
    /// `building.getWorkOrder()` (`:189`, `:789`, `:872`).
    pub has_work_order: bool,
    /// `building.getBuildingLevel()` (`:222-225` for the dump threshold,
    /// `BuildingMiner.java:177` for the depth limit).
    pub building_level: u32,
    /// `walkToWorkPos(building.getLadderLocation())` (`:328`): at the ladder column.
    pub at_ladder: bool,
    /// `building.getWorkOrder().getBlueprint()` (`:789`, `:820`).
    pub has_blueprint: bool,
    /// `getOrderedList(WorkOrderMiner.class, building.getPosition())` is not empty
    /// (`:190-196`).
    pub order_available: bool,
    /// `module.getNumberOfLevels()` (`:424`, `:674`).
    pub levels: u32,
    /// `module.getCurrentLevel().getDepth()` (`:743`, `:793`).
    pub current_level_depth: Option<i32>,
    /// `module.getActiveNode()` (`:691`).
    pub node: Option<NodeFacts>,
    /// `currentLevel.getNode(new Vec2i(node.getX(), node.getZ())) != null`
    /// (`:745`).
    pub node_in_level: bool,
    /// `currentLevel.getNode(workingNode.getParent())` (`:732`), when it exists.
    pub parent_node: Option<NodeFacts>,
    /// `walkWithProxy(standingPosition)` (`:752`).
    pub at_standing: bool,
    /// `building.getPosition()` — what `walkToBuilding()` walks to (`:650`).
    pub at_building: bool,
    /// `blockToMine` (`:260`, `:271`, `:276`).
    pub block_to_mine: Option<Pos>,
    /// `mineBlock(blockToMine, ...)`'s answer (`:276`).
    pub mine_done: bool,
    /// `checkIfRequestForItemExistOrCreate(cobble, COBBLE_REQUEST_BATCHES, 1)`
    /// (`:354`, `:471`, `:772`).
    pub fill_block_available: bool,
    /// `checkIfRequestForItemExistOrCreate(LADDER, LADDER_REQUEST_BATCHES, 1)`
    /// (`:368`, `:472`).
    pub ladder_available: bool,
    /// The shaft's blocks: the ladder column, the 7x7 ring, the fluids.
    pub blocks: GatherBlocks,
    /// `world.getMinBuildHeight()` (`:415`).
    pub min_build_height: i32,
    /// `getSetting(MAX_DEPTH)` (`BuildingMiner.java:206-211`).
    pub max_depth_setting: i32,
    /// Whether that setting still holds its default (`BuildingMiner.java:207`).
    pub max_depth_is_default: bool,
    /// `module.getStartingLevelShaft()` (`:510-515`), `0` before it is set.
    pub starting_level_shaft: i32,
}

/// `FarmField.Stage` (`colony/buildingextensions/FarmField.java:240-246`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: the three stages the farmer drives
pub enum FieldStage {
    /// `EMPTY` (`:242`): needs hoeing.
    #[default]
    Empty,
    /// `HOED` (`:243`): needs planting.
    Hoed,
    /// `PLANTED` (`:244`): needs harvesting.
    Planted,
}

impl FieldStage {
    /// Java's identifier, for a message.
    pub fn java(self) -> &'static str {
        match self {
            FieldStage::Empty => "EMPTY",
            FieldStage::Hoed => "HOED",
            FieldStage::Planted => "PLANTED",
        }
    }

    /// `farmField.nextState()` (`EntityAIWorkFarmer.java:275`, `:359`, `:572`).
    pub fn next(self) -> FieldStage {
        match self {
            FieldStage::Empty => FieldStage::Hoed,
            FieldStage::Hoed => FieldStage::Planted,
            FieldStage::Planted => FieldStage::Empty,
        }
    }
}

/// One field of the farmer's hut, as `FarmField` holds it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FieldFacts {
    /// `farmField.getPosition()` (`:324`, `:527`).
    pub position: Pos,
    /// `farmField.getFieldStage()` (`:263-274`).
    pub stage: FieldStage,
    /// `farmField.getRadius(direction)` for N, E, S, W, in that order
    /// (`:498-501`).
    pub radius: [i32; 4],
    /// `farmField.getSeed()` is not `ItemStack.EMPTY` (`:339`).
    pub has_seed: bool,
    /// `findFirstSlotInInventoryWith(seeds.getItem())` (`:345`), `None` when `-1`.
    pub seed_slot: Option<usize>,
    /// `farmField.getSeed().getItem() instanceof ItemCrop` (`:645`): the crop wants
    /// its own farmland rather than `Blocks.FARMLAND`.
    pub seed_is_crop: bool,
    /// `farmField.isNoPartOfField(world, position)` inverted (`:379`, `:727`).
    pub part_of_field: bool,
}

impl FieldFacts {
    /// `FarmField.MAX_RANGE` (`:40`): no radius is larger.
    pub const MAX_RANGE: i32 = 20;

    /// `getRadius(Direction.NORTH/EAST/SOUTH/WEST)` (`:498-501`).
    pub fn radius(&self, direction: usize) -> i32 {
        self.radius[direction.min(3)]
    }
}

/// Everything `EntityAIWorkFarmer` reads that the base facts do not carry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FarmerFacts {
    /// `building.getBuildingLevel()` (`:212`): level 0 is a hut nobody built.
    pub building_level: u32,
    /// `module.getExtensionToWorkOn()` (`:253`, `:914`).
    pub field: Option<FieldFacts>,
    /// `module.getOwnedExtensions()` not empty — the inverse of `hasNoExtensions()`
    /// (`:243`).
    pub has_extensions: bool,
    /// `InventoryUtils.hasBuildingEnoughElseCount(building, this::isCompost, 1)`
    /// (`:224`).
    pub compost_in_building: u32,
    /// `getItemCountInItemHandler(worker.getInventoryCitizen(), this::isCompost)`
    /// (`:225`).
    pub compost_in_inventory: u32,
    /// `checkForToolOrWeapon(ModEquipmentTypes.hoe)` (`:256`, `:601`).
    pub has_hoe: bool,
    /// `building.getWorkingOffset()` (`:525`, `:569`).
    pub working_offset: Option<(i32, i32)>,
    /// `walkToSafePos(position.above())` (`:530`).
    pub at_cell: bool,
    /// `mineBlock(position.above())` (`:603`, `:666`).
    pub mine_done: bool,
    /// `worker.getCitizenData().getJobEntry()`'s primary skill for this trade, as
    /// `getPrimarySkillLevel()` answers it (`:692`, `AbstractEntityAIBasic.java:
    /// 682-685`). Used by [`super::farmer::level_delay`].
    pub primary_skill: u32,
    /// `building.getPrevPos().above()` is not empty (`:763`): a stem already stands on
    /// the cell the farmer just planted.
    pub stem_on_prev_pos: bool,
    /// `seed == Items.MELON_SEEDS || seed == Items.PUMPKIN_SEEDS` (`:763`).
    pub seed_is_melon_or_pumpkin: bool,
    /// The field's surface and what stands above it.
    pub blocks: GatherBlocks,
}

/// `Pond.PondState` (`api/util/Pond.java:24-28`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: three states in Java
pub enum PondState {
    /// `INVALID` (`:25`).
    #[default]
    Invalid,
    /// `SUBOPTIMAL` (`:26`): narrower than 5x2, the fisherman complains.
    Suboptimal,
    /// `VALID` (`:27`).
    Valid,
}

/// `Pond.WATER_POOL_WIDTH_REQUIREMENT` and `WATER_DEPTH_REQUIREMENT`
/// (`api/util/Pond.java:33-34`), as numbers: a pond is a 5 wide, 2 deep pool.
pub const POND_WIDTH_REQUIREMENT: i32 = 5;
/// The depth half of the same pair (`:34`).
pub const POND_DEPTH_REQUIREMENT: i32 = 2;

/// `job.getWater()`: the `Tuple<BlockPos, BlockPos>` the fisherman fishes at
/// (`EntityAIWorkFisherman.java:317`, `:356`, `:553`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Water {
    /// `getA()` — the block of water the hook is thrown at (`:356`).
    pub target: Pos,
    /// `getB()` — where the fisherman stands (`:317`).
    pub stand: Pos,
}

/// The bobber, as `NewBobberEntity` answers (`EntityAIWorkFisherman.java:575-579`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hook {
    /// `entityFishHook.isInWater()` (`:577`).
    pub in_water: bool,
    /// `entityFishHook.onGround()` (`:577`).
    pub on_ground: bool,
    /// `entityFishHook.shouldStopFishing()` (`:577`).
    pub should_stop: bool,
    /// `entityFishHook.isAlive()` (`:578`).
    pub alive: bool,
    /// `entityFishHook.caughtEntity != null` (`:578`).
    pub caught_entity: bool,
    /// `entityFishHook.isReadyToCatch()` (`:660`).
    pub ready_to_catch: bool,
    /// `entityFishHook.getDamage()` (`:683`): the durability the next retrieve costs.
    pub damage: u32,
}

/// Everything `EntityAIWorkFisherman` reads that the base facts do not carry.
///
/// `PartialEq` and not `Eq`: `draw_double` is a `f64` (`worker.getRandom()
/// .nextDouble()`, `EntityAIWorkFisherman.java:490`) and `f64` is not `Eq`. Nothing in
/// the module uses a fact struct as a map key, so the weaker bound costs nothing —
/// `GatherState`, the only key a `Machine` takes, is an enum of integers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FishermanFacts {
    /// `job.getPonds().size()` (`:374`, `:411`, `:429`).
    pub ponds: u32,
    /// `job.getWater()` (`:317`, `:328`, `:356`).
    pub water: Option<Water>,
    /// The water search job's state (`:441-465`).
    pub path: PathState,
    /// `pathResult.isPathReachingDestination()` (`:450`).
    pub path_reaching: bool,
    /// `pathResult.failedToReachDestination()` (`:446`).
    pub path_failed: bool,
    /// `pathResult.pond` (`:452`).
    pub pond: Option<Pos>,
    /// `pathResult.parent` (`:454`): where the fisherman would stand.
    pub pond_stand: Option<Pos>,
    /// `lastPathResult.pondState` (`:295`).
    pub pond_state: PondState,
    /// `lastPathResult.pond == null` (`:374`).
    pub last_pond_missing: bool,
    /// `walkToSafePos(job.getWater().getB())` (`:317`).
    pub at_water: bool,
    /// `getRodSlot()` (`:603`, `:642`), `None` when `-1`.
    pub rod_slot: Option<usize>,
    /// `worker.getMainHandItem()` is that rod (`:618-625`).
    pub rod_equipped: bool,
    /// `Utils.isBlockInRange(world, Blocks.WATER, ..., MIN_DISTANCE_TO_WATER)` (`:612`).
    pub water_in_range: bool,
    /// `world.getBlockState(worker.blockPosition()).liquid()` (`:340`): the fisherman
    /// stands in the pond itself.
    pub standing_in_liquid: bool,
    /// `BlockUtils.isAnySolid(world.getBlockState(job.getWater().getB().below()))`
    /// (`:342`): the pond has a floor.
    pub pond_floor_solid: bool,
    /// `getPrimarySkillLevel()`: the lure speed (`:563`).
    pub primary_skill: u32,
    /// `getSecondarySkillLevel()`: the base of `testRandomChance`'s draw (`:592`).
    pub secondary_skill: u32,
    /// `entityFishHook` (`:515`, `:656`).
    pub hook: Option<Hook>,
    /// **Not a Java call.** `worker.getRandom().nextInt(FISHING_SKILL_CHANCE +
    /// secondary/5)` in `testRandomChance` (`:592`).
    pub draw_int: u32,
    /// **Not a Java call.** `worker.getRandom().nextDouble()` for `CHANCE_NEW_POND`
    /// (`:490`).
    pub draw_double: f64,
    /// **Not a Java call.** `worker.getRandom().nextInt(job.getPonds().size())` in
    /// `setRandomWater` (`:429`).
    pub draw_pond: u32,
}

/// The trade half of a gather scene.
///
/// A caller that hands a lumberjack's AI a miner's facts gets
/// [`super::state::GatherRefusal::NoFactsForTrade`], not a silent default: the
/// difference between "this forester has no tree today" and "these facts are another
/// trade's" must not be lost.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: the four harvest trades of this slice
pub enum GatherJob {
    /// The forester.
    Lumberjack(LumberjackFacts),
    /// The miner.
    Miner(MinerFacts),
    /// The farmer.
    Farmer(FarmerFacts),
    /// The fisherman.
    Fisherman(FishermanFacts),
}

impl Default for GatherJob {
    fn default() -> GatherJob {
        GatherJob::Lumberjack(LumberjackFacts::default())
    }
}

/// What a trade remembers between two ticks — **not** a fact about the world.
///
/// Java keeps these on the AI object (`EntityAIWorkLumberjack.searchIncrement`
/// `:170`, `.timeWaited` `:154`, `.checkedInHut` `:159`; `EntityAIStructureMiner
/// .minerWorkingLocation` `:134`, `.currentStandingPosition` `:138`,
/// `.workingNode` `:141`; `EntityAIWorkFarmer.didWork` `:121`, `.skippedState`
/// `:126`, `.shouldDumpInventory` `:116`; `EntityAIWorkFisherman.executedRotations`
/// `:135`, `.stuckCounter` `:158`). `delay` and `hasDelayed` are the base class's and
/// are reused from [`Memory`] in [`GatherMemory::base`], because `setDelay` and
/// `hasNotDelayed` are the same two fields for every trade.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GatherMemory {
    /// `AbstractEntityAIBasic.delay` and `.hasDelayed` (`:121`, `:126`).
    pub base: Memory,
    /// What the forester keeps.
    pub lumberjack: LumberjackMemory,
    /// What the miner keeps.
    pub miner: MinerMemory,
    /// What the farmer keeps.
    pub farmer: FarmerMemory,
    /// What the fisherman keeps.
    pub fisherman: FishermanMemory,
}

/// `EntityAIWorkLumberjack`'s own fields (`:149-175`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LumberjackMemory {
    /// `searchIncrement` (`:170`): how much the search radius grew.
    pub search_increment: i32,
    /// `timeWaited` (`:154`), counted in tens of ticks (`:873`).
    pub time_waited: i32,
    /// `checkedInHut` (`:159`): the hut was asked once for a sapling.
    pub checked_in_hut: bool,
    /// `workFrom` (`:149`): the block the forester works from.
    pub work_from: Option<Pos>,
}

/// `EntityAIStructureMiner`'s own fields, plus the module's three setters the AI
/// writes (`:134-141`, `:510-517`, `:674`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MinerMemory {
    /// `minerWorkingLocation` (`:134`).
    pub working_location: Option<Pos>,
    /// `currentStandingPosition` (`:138`): the last safe spot, now air.
    pub standing_position: Option<Pos>,
    /// `workingNode` (`:141`).
    pub working_node: Option<NodeFacts>,
    /// `module.setStartingLevelShaft` (`:510-513`), `0` before it is set.
    pub starting_level_shaft: i32,
    /// `module.setCurrentLevel(module.getNumberOfLevels() - 1)` (`:674`): whether the
    /// level was picked at least once.
    pub level_picked: bool,
}

/// `EntityAIWorkFarmer`'s own fields (`:116-126`), plus the two the hut holds
/// (`building.setCell`, `building.setWorkingOffset`, `:318`, `:470-482`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FarmerMemory {
    /// `shouldDumpInventory` (`:116`): set when a field is finished (`:571`).
    pub should_dump_inventory: bool,
    /// `didWork` (`:121`).
    pub did_work: bool,
    /// `skippedState` (`:126`).
    pub skipped_state: u32,
    /// `building.getWorkingOffset()` (`:318`, `:525`).
    pub working_offset: Option<(i32, i32)>,
    /// `building.getCell()` (`:472`, `:477`): the spiral index.
    pub cell: i32,
}

impl Default for FarmerMemory {
    /// A fresh farmer: `building.setCell(-1)` when no offset is set yet
    /// (`EntityAIWorkFarmer.java:470-473`), so the first scan starts at index 0.
    fn default() -> FarmerMemory {
        FarmerMemory {
            should_dump_inventory: false,
            did_work: false,
            skipped_state: 0,
            working_offset: None,
            cell: -1,
        }
    }
}

/// `EntityAIWorkFisherman`'s own fields (`:135-158`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FishermanMemory {
    /// `executedRotations` (`:135`).
    pub executed_rotations: u32,
    /// `stuckCounter` (`:158`), started at `3` in Java (`:158`).
    pub stuck_counter: u32,
    /// `pathResult` was created and is still running (`:441-444`).
    pub searching: bool,
    /// `lastPathResult != null` (`:295`, `:374`).
    pub has_last_result: bool,
}

/// The whole scene one harvest decision reads: the base facts, the trade facts, and
/// the AI's own memory.
///
/// [`WorldFacts`] is *the same value* the `crate::ai` tables take — nothing is copied
/// out of it — so a caller that also ticks `crate::ai::worker::JobAi` can hand the one
/// it already filled to both.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GatherScene {
    /// The citizen, its inventory, its hut and its blocks (`crate::ai::facts`).
    pub facts: WorldFacts,
    /// This trade's own observations.
    pub job: GatherJob,
    /// What the AI remembers between ticks.
    pub memory: GatherMemory,
    /// The last refusal, cleared at the top of every tick.
    pub refusal: Option<super::state::GatherRefusal>,
}

impl GatherScene {
    /// A scene on `facts` with an empty memory and `job`, the trade facts.
    pub fn new(facts: WorldFacts, job: GatherJob) -> GatherScene {
        GatherScene {
            facts,
            job,
            memory: GatherMemory::default(),
            refusal: None,
        }
    }

    /// The forester's facts, if this is a forester.
    pub fn lumberjack(&self) -> Option<&LumberjackFacts> {
        match &self.job {
            GatherJob::Lumberjack(facts) => Some(facts),
            _ => None,
        }
    }

    /// The miner's facts, if this is a miner.
    pub fn miner(&self) -> Option<&MinerFacts> {
        match &self.job {
            GatherJob::Miner(facts) => Some(facts),
            _ => None,
        }
    }

    /// The farmer's facts, if this is a farmer.
    pub fn farmer(&self) -> Option<&FarmerFacts> {
        match &self.job {
            GatherJob::Farmer(facts) => Some(facts),
            _ => None,
        }
    }

    /// The fisherman's facts, if this is a fisherman.
    pub fn fisherman(&self) -> Option<&FishermanFacts> {
        match &self.job {
            GatherJob::Fisherman(facts) => Some(facts),
            _ => None,
        }
    }

    /// Record why this tick cannot go further. Last one wins, as in
    /// `crate::ai::worker::Scene::refuse`.
    pub fn refuse(&mut self, refusal: super::state::GatherRefusal) {
        self.refusal = Some(refusal);
    }

    /// `walkToBuilding()` (`AbstractEntityAIBasic.java:757-766`) against the hut's
    /// own position: `isLivingAtSite(building, WOKR_IN_BUILDING_DIST)`, the same
    /// test `crate::ai::worker` makes for the base states.
    pub fn at_hut(&self) -> bool {
        self.facts
            .worker
            .at
            .is_living_at_site(
                self.facts.building.position,
                crate::ai::worker::WOKR_IN_BUILDING_DIST,
            )
    }

    /// The hut's inventory facts, for the rules that must read them.
    pub fn inventory(&self) -> &InventoryFacts {
        &self.facts.inventory
    }

    /// The hut itself.
    pub fn building(&self) -> &BuildingFacts {
        &self.facts.building
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unreported_block_is_air_and_an_unloaded_one_is_not_air() {
        // The same contract as `crate::ai::facts::Blocks`: Java asks the level, this
        // port asks what the caller wrote down.
        let mut blocks = GatherBlocks::new();
        assert_eq!(blocks.at(Pos::new(1, 2, 3)), GatherBlock::Air);
        assert!(blocks.is_loaded(Pos::new(1, 2, 3)));
        blocks.set(Pos::new(1, 2, 3), GatherBlock::Ore);
        assert_eq!(blocks.at(Pos::new(1, 2, 3)), GatherBlock::Ore);
        assert_eq!(blocks.len(), 1);
        blocks.mark_unloaded(Pos::new(1, 2, 3));
        assert!(!blocks.is_loaded(Pos::new(1, 2, 3)));
        assert_eq!(blocks.at(Pos::new(1, 2, 3)), GatherBlock::Ore, "still reported");
    }

    #[test]
    fn a_torch_and_a_fluid_do_not_block_motion_but_a_log_does() {
        // `secureBlock` fills whatever does not block and is not a torch
        // (`EntityAIStructureMiner.java:763`).
        assert!(!GatherBlock::Air.blocks_motion());
        assert!(!GatherBlock::Fluid.blocks_motion());
        assert!(!GatherBlock::Torch.blocks_motion());
        assert!(GatherBlock::Log.blocks_motion());
        assert!(GatherBlock::Leaves.blocks_motion());
    }

    #[test]
    fn only_a_ripe_crop_pumpkin_or_melon_is_harvestable() {
        // `findHarvestableSurface` (`EntityAIWorkFarmer.java:792-802`).
        assert!(GatherBlock::Crop { ripe: true }.is_harvestable());
        assert!(!GatherBlock::Crop { ripe: false }.is_harvestable());
        assert!(GatherBlock::PumpkinOrMelon.is_harvestable());
        assert!(!GatherBlock::Sapling.is_harvestable());
        assert!(!GatherBlock::Farmland.is_harvestable());
    }

    #[test]
    fn the_field_stage_cycle_is_java_s_three_step_loop() {
        // `farmField.nextState()` (`EntityAIWorkFarmer.java:275`, `:359`, `:572`).
        assert_eq!(FieldStage::Empty.next(), FieldStage::Hoed);
        assert_eq!(FieldStage::Hoed.next(), FieldStage::Planted);
        assert_eq!(FieldStage::Planted.next(), FieldStage::Empty);
        assert_eq!(FieldStage::Planted.java(), "PLANTED");
    }

    #[test]
    fn a_tree_reports_its_two_queues_through_has_logs_and_has_leaves() {
        // `Tree.hasLogs()` (`Tree.java:887`) and `hasLeaves()` (`:877`).
        let mut tree = TreeFacts {
            is_tree: true,
            ..TreeFacts::default()
        };
        assert!(!tree.has_logs());
        assert!(!tree.has_leaves());
        tree.logs = 4;
        tree.leaves = 12;
        assert!(tree.has_logs());
        assert!(tree.has_leaves());
        assert_eq!(tree.next_log, None, "the caller reports the head separately");
    }

    #[test]
    fn a_scene_refuses_to_answer_for_another_trade() {
        let scene = GatherScene::new(
            WorldFacts::new(),
            GatherJob::Farmer(FarmerFacts::default()),
        );
        assert!(scene.farmer().is_some());
        assert!(scene.lumberjack().is_none());
        assert!(scene.miner().is_none());
        assert!(scene.fisherman().is_none());
    }

    #[test]
    fn the_hut_is_reached_at_the_same_distance_the_base_table_uses() {
        // `EntityNavigationUtils.WOKR_IN_BUILDING_DIST` = 7, as in
        // `crate::ai::worker::WOKR_IN_BUILDING_DIST`.
        let mut facts = WorldFacts::new();
        facts.building.position = Pos::new(0, 64, 0);
        facts.worker.at = Pos::new(6, 64, 0);
        let scene = GatherScene::new(facts.clone(), GatherJob::Miner(MinerFacts::default()));
        assert!(scene.at_hut());
        facts.worker.at = Pos::new(7, 64, 0);
        let scene = GatherScene::new(facts, GatherJob::Miner(MinerFacts::default()));
        assert!(!scene.at_hut(), "the comparison is strict");
    }
}
