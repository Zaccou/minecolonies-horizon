//! The states the four harvest trades add to `AIWorkerState`, the flag each one
//! carries, and the refusals they name.
//!
//! Java reference: `api/entity/ai/statemachine/states/AIWorkerState.java`, one enum
//! for **every** trade in the mod (127 values). The port keeps that shape — one enum,
//! [`GatherState`], with the nineteen states of the four harvest trades plus the ten
//! base states they hand control back to — and not one enum per trade, because Java's
//! own tables are written against the single enum: `EntityAIWorkFarmer.java:265`
//! returns `IDLE` and `EntityAIWorkFisherman.java:347` returns `START_WORKING`, both
//! base states, from a trade table.
//!
//! **Why `Base(...)`.** `crate::ai::state::WorkerState` already names the eighteen
//! base states this port gives a table to, and that enum belongs to `crate::ai`, not
//! here (this module writes only inside `src/ai_gather`). So a base state is
//! [`GatherState::Base`] wrapping the `WorkerState` the AI module already knows, and
//! [`GatherState::worker`] gives it back. No state is spelled twice.
//!
//! **What is *not* here.** The behaviour of a base state (dumping the inventory,
//! waiting for a request, pausing) is `crate::ai::worker::register_base`. A gather
//! table enters and leaves those states and says so, but re-implementing them would
//! be the second copy of a table that already exists.

use core::fmt;

use crate::ai::state::WorkerState;

/// One state of a harvest trade's table: a base state handed back to `crate::ai`, or
/// a state this module names.
///
/// Java reference: `AIWorkerState.java` — the line in brackets is the declaration of
/// the constant, and that line is what [`GatherState::java`] reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
// exhaustive: the ten base states the harvest tables enter plus the nineteen they own
pub enum GatherState {
    /// A state of `crate::ai::state::WorkerState` — `IDLE`, `PREPARING`,
    /// `START_WORKING`, `INVENTORY_FULL`, `NEEDS_ITEM`, `PAUSED`,
    /// `GATHERING_REQUIRED_MATERIALS`, `MINE_BLOCK`, `LOAD_STRUCTURE`,
    /// `BUILDING_STEP`.
    Base(WorkerState),
    /// `LUMBERJACK_START_WORKING` (`AIWorkerState.java:84`) — walk to the hut.
    LumberjackStartWorking,
    /// `LUMBERJACK_SEARCHING_TREE` (`:88`) — look for a tree.
    LumberjackSearchingTree,
    /// `LUMBERJACK_CHOP_TREE` (`:92`) — cut the tree the job remembers.
    LumberjackChopTree,
    /// `LUMBERJACK_GATHERING` (`:96`) — pick the drops up.
    LumberjackGathering,
    /// `LUMBERJACK_NO_TREES_FOUND` (`:100`) — wait, then look again.
    LumberjackNoTreesFound,
    /// `LUMBERJACK_GATHERING_2` (`:104`) — the second gathering pass, saplings only.
    LumberjackGathering2,
    /// `MINER_CHECK_MINESHAFT` (`:112`) — shaft or node, decided by the depth.
    MinerCheckMineshaft,
    /// `MINER_WALKING_TO_LADDER` (`:116`).
    MinerWalkingToLadder,
    /// `MINER_REPAIRING_LADDER` (`:120`).
    MinerRepairingLadder,
    /// `MINER_MINING_SHAFT` (`:124`) — widen and deepen the shaft.
    MinerMiningShaft,
    /// `MINER_BUILDING_SHAFT` (`:128`) — run the shaft blueprint.
    MinerBuildingShaft,
    /// `MINER_MINING_NODE` (`:132`) — run one branch blueprint.
    MinerMiningNode,
    /// `FARMER_HOE` (`:150`).
    FarmerHoe,
    /// `FARMER_PLANT` (`:155`).
    FarmerPlant,
    /// `FARMER_HARVEST` (`:160`).
    FarmerHarvest,
    /// `FISHERMAN_SEARCHING_WATER` (`:70`).
    FishermanSearchingWater,
    /// `FISHERMAN_WALKING_TO_WATER` (`:74`).
    FishermanWalkingToWater,
    /// `FISHERMAN_CHECK_WATER` (`:75`).
    FishermanCheckWater,
    /// `FISHERMAN_START_FISHING` (`:76`).
    FishermanStartFishing,
}

impl GatherState {
    /// Every state this module names, trade states first in `AIWorkerState`'s
    /// declaration order, then the base states in `WorkerState`'s.
    pub const ALL: [GatherState; 29] = [
        GatherState::Base(WorkerState::Idle),
        GatherState::Base(WorkerState::Preparing),
        GatherState::Base(WorkerState::StartWorking),
        GatherState::Base(WorkerState::InventoryFull),
        GatherState::Base(WorkerState::NeedsItem),
        GatherState::Base(WorkerState::Paused),
        GatherState::Base(WorkerState::GatheringRequiredMaterials),
        GatherState::Base(WorkerState::MineBlock),
        GatherState::Base(WorkerState::LoadStructure),
        GatherState::Base(WorkerState::BuildingStep),
        GatherState::LumberjackStartWorking,
        GatherState::LumberjackSearchingTree,
        GatherState::LumberjackChopTree,
        GatherState::LumberjackGathering,
        GatherState::LumberjackNoTreesFound,
        GatherState::LumberjackGathering2,
        GatherState::MinerCheckMineshaft,
        GatherState::MinerWalkingToLadder,
        GatherState::MinerRepairingLadder,
        GatherState::MinerMiningShaft,
        GatherState::MinerBuildingShaft,
        GatherState::MinerMiningNode,
        GatherState::FarmerHoe,
        GatherState::FarmerPlant,
        GatherState::FarmerHarvest,
        GatherState::FishermanSearchingWater,
        GatherState::FishermanWalkingToWater,
        GatherState::FishermanCheckWater,
        GatherState::FishermanStartFishing,
    ];

    /// Java's identifier (`AIWorkerState.java`), which is what a state history or the
    /// citizen's nametag shows (`AbstractEntityAIBasic.java:475`).
    pub fn java(self) -> &'static str {
        match self {
            GatherState::Base(state) => state.java_name(),
            GatherState::LumberjackStartWorking => "LUMBERJACK_START_WORKING",
            GatherState::LumberjackSearchingTree => "LUMBERJACK_SEARCHING_TREE",
            GatherState::LumberjackChopTree => "LUMBERJACK_CHOP_TREE",
            GatherState::LumberjackGathering => "LUMBERJACK_GATHERING",
            GatherState::LumberjackNoTreesFound => "LUMBERJACK_NO_TREES_FOUND",
            GatherState::LumberjackGathering2 => "LUMBERJACK_GATHERING_2",
            GatherState::MinerCheckMineshaft => "MINER_CHECK_MINESHAFT",
            GatherState::MinerWalkingToLadder => "MINER_WALKING_TO_LADDER",
            GatherState::MinerRepairingLadder => "MINER_REPAIRING_LADDER",
            GatherState::MinerMiningShaft => "MINER_MINING_SHAFT",
            GatherState::MinerBuildingShaft => "MINER_BUILDING_SHAFT",
            GatherState::MinerMiningNode => "MINER_MINING_NODE",
            GatherState::FarmerHoe => "FARMER_HOE",
            GatherState::FarmerPlant => "FARMER_PLANT",
            GatherState::FarmerHarvest => "FARMER_HARVEST",
            GatherState::FishermanSearchingWater => "FISHERMAN_SEARCHING_WATER",
            GatherState::FishermanWalkingToWater => "FISHERMAN_WALKING_TO_WATER",
            GatherState::FishermanCheckWater => "FISHERMAN_CHECK_WATER",
            GatherState::FishermanStartFishing => "FISHERMAN_START_FISHING",
        }
    }

    /// The block that declares this state, `AIWorkerState.java:<line>`.
    pub fn java_declaration(self) -> &'static str {
        match self {
            GatherState::Base(_) => "crate::ai::state::WorkerState (see AIWorkerState.java)",
            GatherState::LumberjackStartWorking => "AIWorkerState.java:84",
            GatherState::LumberjackSearchingTree => "AIWorkerState.java:88",
            GatherState::LumberjackChopTree => "AIWorkerState.java:92",
            GatherState::LumberjackGathering => "AIWorkerState.java:96",
            GatherState::LumberjackNoTreesFound => "AIWorkerState.java:100",
            GatherState::LumberjackGathering2 => "AIWorkerState.java:104",
            GatherState::MinerCheckMineshaft => "AIWorkerState.java:112",
            GatherState::MinerWalkingToLadder => "AIWorkerState.java:116",
            GatherState::MinerRepairingLadder => "AIWorkerState.java:120",
            GatherState::MinerMiningShaft => "AIWorkerState.java:124",
            GatherState::MinerBuildingShaft => "AIWorkerState.java:128",
            GatherState::MinerMiningNode => "AIWorkerState.java:132",
            GatherState::FarmerHoe => "AIWorkerState.java:150",
            GatherState::FarmerPlant => "AIWorkerState.java:155",
            GatherState::FarmerHarvest => "AIWorkerState.java:160",
            GatherState::FishermanSearchingWater => "AIWorkerState.java:70",
            GatherState::FishermanWalkingToWater => "AIWorkerState.java:74",
            GatherState::FishermanCheckWater => "AIWorkerState.java:75",
            GatherState::FishermanStartFishing => "AIWorkerState.java:76",
        }
    }

    /// `AIWorkerState.isOkayToEat` — the flag `AbstractAISkeleton.canBeInterrupted()`
    /// returns (`AbstractAISkeleton.java:126-131`), which `inventoryNeedsDump` reads
    /// (`AbstractEntityAIBasic.java:411`).
    ///
    /// Two of the nineteen are `false`: the lumberjack is mid-tree, and the fisherman
    /// has a line in the water. Every other harvest state may be interrupted.
    pub fn okay_to_eat(self) -> bool {
        match self {
            GatherState::Base(state) => state.okay_to_eat(),
            // :92, :76
            GatherState::LumberjackChopTree | GatherState::FishermanStartFishing => false,
            // :84, :88, :96, :100, :104, :112, :116, :120, :124, :128, :132, :150,
            // :155, :160, :70, :74, :75
            GatherState::LumberjackStartWorking
            | GatherState::LumberjackSearchingTree
            | GatherState::LumberjackGathering
            | GatherState::LumberjackNoTreesFound
            | GatherState::LumberjackGathering2
            | GatherState::MinerCheckMineshaft
            | GatherState::MinerWalkingToLadder
            | GatherState::MinerRepairingLadder
            | GatherState::MinerMiningShaft
            | GatherState::MinerBuildingShaft
            | GatherState::MinerMiningNode
            | GatherState::FarmerHoe
            | GatherState::FarmerPlant
            | GatherState::FarmerHarvest
            | GatherState::FishermanSearchingWater
            | GatherState::FishermanWalkingToWater
            | GatherState::FishermanCheckWater => true,
        }
    }

    /// The `WorkerState` behind a base state, `None` for a trade state.
    pub fn worker(self) -> Option<WorkerState> {
        match self {
            GatherState::Base(state) => Some(state),
            _ => None,
        }
    }

    /// Whether this state belongs to `trade`.
    pub fn belongs_to(self, trade: super::GatherTrade) -> bool {
        match trade {
            super::GatherTrade::Lumberjack => matches!(
                self,
                GatherState::LumberjackStartWorking
                    | GatherState::LumberjackSearchingTree
                    | GatherState::LumberjackChopTree
                    | GatherState::LumberjackGathering
                    | GatherState::LumberjackNoTreesFound
                    | GatherState::LumberjackGathering2
            ),
            super::GatherTrade::Miner => matches!(
                self,
                GatherState::MinerCheckMineshaft
                    | GatherState::MinerWalkingToLadder
                    | GatherState::MinerRepairingLadder
                    | GatherState::MinerMiningShaft
                    | GatherState::MinerBuildingShaft
                    | GatherState::MinerMiningNode
            ),
            super::GatherTrade::Farmer => matches!(
                self,
                GatherState::FarmerHoe | GatherState::FarmerPlant | GatherState::FarmerHarvest
            ),
            super::GatherTrade::Fisherman => matches!(
                self,
                GatherState::FishermanSearchingWater
                    | GatherState::FishermanWalkingToWater
                    | GatherState::FishermanCheckWater
                    | GatherState::FishermanStartFishing
            ),
        }
    }
}

impl fmt::Display for GatherState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.java())
    }
}

/// Why a harvest AI has nothing to do, named.
///
/// Java has no such type: each branch that cannot proceed does `return getState()` or
/// `return IDLE` and the reason stays in the shape of the `if`. A player asking "why
/// is my forester standing there" needs it, and a test asserting on a state sequence
/// needs to know *which* branch held, so every variant carries the line that decides
/// it. [`super::worker::Refusal`] already names the base class's reasons (`Paused`,
/// `HutFull`, `ToolMissing`); these are the ones the four trade classes add.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: one variant per trade branch that stops without acting
pub enum GatherRefusal {
    /// The scene does not carry this trade's facts — a caller mistake, not a Java
    /// state. Named so it can never look like "nothing to harvest".
    NoFactsForTrade,
    /// `setNewTree`: the radius grew to `SEARCH_LIMIT` and still found nothing
    /// (`EntityAIWorkLumberjack.java:446-449`).
    NoTreeFound,
    /// `chopTree`: the tree is outside the hut's restriction area (`:500-505`).
    TreeOutsideRestriction,
    /// `chopTree`: still walking to the stump (`:509-516`).
    NotAtTree,
    /// `plantSapling`: no sapling of the right kind, in the inventory or the hut
    /// (`:826-828`, `:860-864`).
    NoSapling,
    /// `findTrees`: the job has a crafting task queued, so it goes crafting first
    /// (`:377-380`).
    CraftingQueued,
    /// `decide`: the trade's crafting half (planks, in `AbstractEntityAICrafting`)
    /// is not ported; the gather half answers this instead (`:260`).
    CraftingNotPorted,
    /// `startWorkingAtOwnBuilding`: the hut has no ladder or cobble anchor, the
    /// player has to move the hut (`EntityAIStructureMiner.java:183-187`).
    NoMineshaft,
    /// `checkMineShaft`: the shaft reached the depth limit but the hut has no level
    /// left to mine (`:424-428`).
    NeedsBetterHut,
    /// `searchANodeToMine`: no node is available at this level (`:694-702`).
    NoNode,
    /// `repairLadder` / `advanceLadder`: the fill block or the ladder is missing and
    /// was requested (`:354-357`, `:368-371`, `:471-475`).
    MissingShaftMaterial,
    /// `advanceLadder`: a block of the next step is not mined yet (`:520-525`).
    ShaftBlocked,
    /// `getDepthLimit` + `checkMineShaft`: the miner is not at the ladder yet
    /// (`:178-181`).
    NotAtLadder,
    /// `doMining`: `mineBlock` did not finish, so the tick swings and stays
    /// (`:276-280`).
    MiningIncomplete,
    /// The miner's structure half — `MINE_BLOCK`'s blueprint step, `LOAD_STRUCTURE`,
    /// `BUILDING_STEP`, `START_BUILDING` — is `crate::ai::builder`'s table
    /// (`AbstractEntityAIStructure.java:334-535`); the shaft machine hands it back and
    /// names it here instead of porting it twice (`EntityAIStructureMiner.java:199-206`).
    StructureTableOwned,
    /// `prepareForFarming`: the hut has no field at all (`EntityAIWorkFarmer.java:
    /// 243-251`).
    NoField,
    /// `canGoPlanting`: the field has no seed chosen (`:339-342`).
    NoSeed,
    /// `prepareForFarming`: no compost in the hut or the inventory, the request is
    /// out (`:227-236`).
    NoCompost,
    /// `prepareForFarming` / `workAtField`: still walking to the cell (`:530-533`).
    NotAtField,
    /// `prepareForFarming`: four field states skipped in a row, the current field is
    /// dropped (`:276-281`).
    FieldSkipped,
    /// `prepareForFishing`: no fishing rod good enough (`EntityAIWorkFisherman.java:
    /// 207-212`, `:604-609`).
    NoRod,
    /// `setRandomWater`: no pond known and the search found none (`:411-428`).
    NoWater,
    /// `getToWater`: the pond is suboptimal (not 5x2), the fisherman complains
    /// (`:295-304`).
    SuboptimalPond,
    /// `tryDifferentAngles`: six rotations used up, the pond is dropped
    /// (`:332-338`).
    RotationsExhausted,
    /// `isReadyToFish`: no water within three blocks, walk closer (`:612-615`).
    WaterTooFar,
    /// `throwOrRetrieveHook`: the hook is stuck and was retrieved (`:527-535`).
    HookStuck,
    /// `isReadyToFish`: the rod must be equipped before the throw (`:618-625`).
    RodNotEquipped,
    /// `walkTo...` answered false: the citizen is on its way, the tick does nothing
    /// else (`EntityAIWorkFisherman.java:285-288`).
    NotAtWater,
    /// `checkForToolOrWeapon` answered true: a tool is needed and none good enough is
    /// in the inventory or the hut, so a request was created
    /// (`AbstractEntityAIBasic.java:993-1002`). A tool that broke lands here too: the
    /// inventory slot is empty afterwards, and the check asks for a new one.
    ToolMissing,
    /// The inventory is full or the dump threshold is reached, and the dump is
    /// `AbstractEntityAIBasic`'s (`AbstractEntityAIBasic.java:409-416`).
    InventoryDumpDue,
}

impl GatherRefusal {
    /// The Java line that decides this refusal.
    pub fn java(self) -> &'static str {
        match self {
            GatherRefusal::NoFactsForTrade => "port: src/ai_gather, not a Java branch",
            GatherRefusal::NoTreeFound => "EntityAIWorkLumberjack.java:446-449",
            GatherRefusal::TreeOutsideRestriction => "EntityAIWorkLumberjack.java:500-505",
            GatherRefusal::NotAtTree => "EntityAIWorkLumberjack.java:509-516",
            GatherRefusal::NoSapling => "EntityAIWorkLumberjack.java:826-828",
            GatherRefusal::ToolMissing => "AbstractEntityAIBasic.java:993-1002",
            GatherRefusal::CraftingQueued => "EntityAIWorkLumberjack.java:377-380",
            GatherRefusal::CraftingNotPorted => "EntityAIWorkLumberjack.java:260",
            GatherRefusal::NoMineshaft => "EntityAIStructureMiner.java:183-187",
            GatherRefusal::NeedsBetterHut => "EntityAIStructureMiner.java:424-428",
            GatherRefusal::NoNode => "EntityAIStructureMiner.java:694-702",
            GatherRefusal::MissingShaftMaterial => "EntityAIStructureMiner.java:354-357",
            GatherRefusal::ShaftBlocked => "EntityAIStructureMiner.java:520-525",
            GatherRefusal::NotAtLadder => "EntityAIStructureMiner.java:178-181",
            GatherRefusal::MiningIncomplete => "EntityAIStructureMiner.java:276-280",
            GatherRefusal::StructureTableOwned => "AbstractEntityAIStructure.java:334-535",
            GatherRefusal::NoField => "EntityAIWorkFarmer.java:243-251",
            GatherRefusal::NoSeed => "EntityAIWorkFarmer.java:339-342",
            GatherRefusal::NoCompost => "EntityAIWorkFarmer.java:227-236",
            GatherRefusal::NotAtField => "EntityAIWorkFarmer.java:530-533",
            GatherRefusal::FieldSkipped => "EntityAIWorkFarmer.java:276-281",
            GatherRefusal::NoRod => "EntityAIWorkFisherman.java:207-212",
            GatherRefusal::NoWater => "EntityAIWorkFisherman.java:411-428",
            GatherRefusal::SuboptimalPond => "EntityAIWorkFisherman.java:295-304",
            GatherRefusal::RotationsExhausted => "EntityAIWorkFisherman.java:332-338",
            GatherRefusal::WaterTooFar => "EntityAIWorkFisherman.java:612-615",
            GatherRefusal::HookStuck => "EntityAIWorkFisherman.java:527-535",
            GatherRefusal::RodNotEquipped => "EntityAIWorkFisherman.java:618-625",
            GatherRefusal::NotAtWater => "EntityAIWorkFisherman.java:285-288",
            GatherRefusal::InventoryDumpDue => "AbstractEntityAIBasic.java:409-416",
        }
    }

    /// A stable key for a message the player will read, next to
    /// `crate::ai::worker::Refusal::key`.
    pub fn key(self) -> &'static str {
        match self {
            GatherRefusal::NoFactsForTrade => "ai_gather.refusal.no_facts_for_trade",
            GatherRefusal::NoTreeFound => "ai_gather.refusal.no_tree_found",
            GatherRefusal::TreeOutsideRestriction => "ai_gather.refusal.tree_outside_restriction",
            GatherRefusal::NotAtTree => "ai_gather.refusal.not_at_tree",
            GatherRefusal::NoSapling => "ai_gather.refusal.no_sapling",
            GatherRefusal::ToolMissing => "ai_gather.refusal.tool_missing",
            GatherRefusal::CraftingQueued => "ai_gather.refusal.crafting_queued",
            GatherRefusal::CraftingNotPorted => "ai_gather.refusal.crafting_not_ported",
            GatherRefusal::NoMineshaft => "ai_gather.refusal.no_mineshaft",
            GatherRefusal::NeedsBetterHut => "ai_gather.refusal.needs_better_hut",
            GatherRefusal::NoNode => "ai_gather.refusal.no_node",
            GatherRefusal::MissingShaftMaterial => "ai_gather.refusal.missing_shaft_material",
            GatherRefusal::ShaftBlocked => "ai_gather.refusal.shaft_blocked",
            GatherRefusal::NotAtLadder => "ai_gather.refusal.not_at_ladder",
            GatherRefusal::MiningIncomplete => "ai_gather.refusal.mining_incomplete",
            GatherRefusal::StructureTableOwned => "ai_gather.refusal.structure_table_owned",
            GatherRefusal::NoField => "ai_gather.refusal.no_field",
            GatherRefusal::NoSeed => "ai_gather.refusal.no_seed",
            GatherRefusal::NoCompost => "ai_gather.refusal.no_compost",
            GatherRefusal::NotAtField => "ai_gather.refusal.not_at_field",
            GatherRefusal::FieldSkipped => "ai_gather.refusal.field_skipped",
            GatherRefusal::NoRod => "ai_gather.refusal.no_rod",
            GatherRefusal::NoWater => "ai_gather.refusal.no_water",
            GatherRefusal::SuboptimalPond => "ai_gather.refusal.suboptimal_pond",
            GatherRefusal::RotationsExhausted => "ai_gather.refusal.rotations_exhausted",
            GatherRefusal::WaterTooFar => "ai_gather.refusal.water_too_far",
            GatherRefusal::HookStuck => "ai_gather.refusal.hook_stuck",
            GatherRefusal::RodNotEquipped => "ai_gather.refusal.rod_not_equipped",
            GatherRefusal::NotAtWater => "ai_gather.refusal.not_at_water",
            GatherRefusal::InventoryDumpDue => "ai_gather.refusal.inventory_dump_due",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_names_twenty_nine_states_and_no_two_share_a_name() {
        assert_eq!(GatherState::ALL.len(), 29);
        let mut names: Vec<&str> = GatherState::ALL.iter().map(|s| s.java()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 29, "two states share a Java name");
        // The nineteen trade states of `AIWorkerState.java:70-160`.
        let trade_states = GatherState::ALL
            .iter()
            .filter(|state| state.worker().is_none())
            .count();
        assert_eq!(trade_states, 19);
    }

    #[test]
    fn two_of_the_nineteen_may_not_be_interrupted() {
        // `AIWorkerState.java:92` (chop) and `:76` (rod in the water) pass `false`.
        assert!(!GatherState::LumberjackChopTree.okay_to_eat());
        assert!(!GatherState::FishermanStartFishing.okay_to_eat());
        assert!(GatherState::LumberjackGathering.okay_to_eat()); // :96
        assert!(GatherState::MinerMiningShaft.okay_to_eat()); // :124
        assert!(GatherState::FarmerHarvest.okay_to_eat()); // :160
        // A base state keeps the flag `crate::ai` already gives it.
        assert!(!GatherState::Base(WorkerState::InventoryFull).okay_to_eat());
        assert!(GatherState::Base(WorkerState::StartWorking).okay_to_eat());
    }

    #[test]
    fn each_trade_owns_only_its_own_states() {
        use super::super::GatherTrade;
        assert!(GatherState::FarmerHoe.belongs_to(GatherTrade::Farmer));
        assert!(!GatherState::FarmerHoe.belongs_to(GatherTrade::Miner));
        assert!(!GatherState::Base(WorkerState::Idle).belongs_to(GatherTrade::Miner));
        // Nineteen trade states, split 6 / 6 / 3 / 4.
        for (trade, expected) in [
            (GatherTrade::Lumberjack, 6usize),
            (GatherTrade::Miner, 6),
            (GatherTrade::Farmer, 3),
            (GatherTrade::Fisherman, 4),
        ] {
            let count = GatherState::ALL
                .iter()
                .filter(|state| state.belongs_to(trade))
                .count();
            assert_eq!(count, expected, "{}", trade.name());
        }
    }

    #[test]
    fn every_refusal_names_a_java_line_and_a_message_key() {
        let all = [
            GatherRefusal::NoTreeFound,
            GatherRefusal::TreeOutsideRestriction,
            GatherRefusal::NotAtTree,
            GatherRefusal::NoSapling,
            GatherRefusal::ToolMissing,
            GatherRefusal::CraftingQueued,
            GatherRefusal::CraftingNotPorted,
            GatherRefusal::NoMineshaft,
            GatherRefusal::NeedsBetterHut,
            GatherRefusal::NoNode,
            GatherRefusal::MissingShaftMaterial,
            GatherRefusal::ShaftBlocked,
            GatherRefusal::NotAtLadder,
            GatherRefusal::MiningIncomplete,
            GatherRefusal::StructureTableOwned,
            GatherRefusal::NoField,
            GatherRefusal::NoSeed,
            GatherRefusal::NoCompost,
            GatherRefusal::NotAtField,
            GatherRefusal::FieldSkipped,
            GatherRefusal::NoRod,
            GatherRefusal::NoWater,
            GatherRefusal::SuboptimalPond,
            GatherRefusal::RotationsExhausted,
            GatherRefusal::WaterTooFar,
            GatherRefusal::HookStuck,
            GatherRefusal::RodNotEquipped,
            GatherRefusal::NotAtWater,
            GatherRefusal::InventoryDumpDue,
            GatherRefusal::NoFactsForTrade,
        ];
        for refusal in all {
            assert!(!refusal.java().is_empty(), "{:?}", refusal);
            assert!(refusal.key().starts_with("ai_gather.refusal."), "{:?}", refusal);
        }
        // The one entry that is the port's and not Java's says so out loud.
        assert!(GatherRefusal::NoFactsForTrade.java().starts_with("port:"));
    }
}
