//! The states the three trades of this slice add to `AIWorkerState`, the flag each
//! one carries, and the refusals they name.
//!
//! Java reference: `api/entity/ai/statemachine/states/AIWorkerState.java` — **one**
//! enum for every trade in the mod (127 values). The port keeps that shape:
//! [`WorkState`] carries the seven states of the nether miner, the beekeeper and the
//! composter, plus the six base states their tables enter and leave. It does not
//! declare one enum per trade, because Java's own tables are written against the
//! single enum: `EntityAIWorkNether.java:311` returns `NETHER_LEAVE` from a method of
//! the crafting parent, and `EntityAIWorkBeekeeper.java:258` returns `START_WORKING`,
//! a base state, from the beekeeper's own `decide`.
//!
//! **Why `Base(...)`.** `crate::ai::state::WorkerState` already names the eighteen
//! base states this port gives a table to, and that enum belongs to `crate::ai` — this
//! module writes only inside `src/ai_work_07`, so a base state is
//! [`WorkState::Base`] wrapping the `WorkerState` the AI module already knows, and
//! [`WorkState::worker`] gives it back. No state is spelled twice.
//!
//! **What is *not* here.** The behaviour of a base state (dumping the inventory,
//! waiting for a request, pausing) is `crate::ai::worker::register_base`. A trade
//! table enters and leaves those states and says so — [`super::register_handover`] —
//! but re-implementing them would be the second copy of a table that already exists.
//!
//! **The composter is absent on purpose.** Its six targets are already carried by
//! `crate::ai::composter` over `WorkerState`; this module adds no state for it and
//! only reaches it through [`super::WorkerAi`]. `RAPPORT.md` §1 says why.

use core::fmt;

use crate::ai::state::WorkerState;
use crate::jobs::tool::ToolKind;

/// One state of the three trades' tables: a base state handed back to `crate::ai`, or
/// a state this module names.
///
/// Java reference: `AIWorkerState.java` — the line in brackets is the declaration of
/// the constant, and that line is what [`WorkState::java_declaration`] reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
// exhaustive: the six base states a table may enter plus the seven own states
pub enum WorkState {
    /// A state of `crate::ai::state::WorkerState` — `IDLE`, `PREPARING`,
    /// `START_WORKING`, `DECIDE`, `INVENTORY_FULL`, `GATHERING_REQUIRED_MATERIALS`.
    Base(WorkerState),
    /// `HERDER_BREED` (`AIWorkerState.java:357`) — the beekeeper pairs two bees.
    HerderBreed,
    /// `NETHER_LEAVE` (`:555`) — walk to the portal and travel.
    NetherLeave,
    /// `NETHER_AWAY` (`:560`) — the citizen is in the nether; process the queues.
    NetherAway,
    /// `NETHER_RETURN` (`:565`) — walk home, go visible, hand the loot over.
    NetherReturn,
    /// `NETHER_OPENPORTAL` (`:570`) — light the portal with flint and steel.
    NetherOpenPortal,
    /// `NETHER_CLOSEPORTAL` (`:575`) — put the portal out.
    NetherClosePortal,
    /// `BEEKEEPER_HARVEST` (`:638`) — harvest one full hive.
    BeekeeperHarvest,
}

impl WorkState {
    /// Every state this module names, base states first in `WorkerState`'s order, then
    /// the trade states in `AIWorkerState`'s — which is the order Java declares them
    /// in (15, 27, 31, 59, 23 is the one inversion, `INVENTORY_FULL` being declared
    /// before `PREPARING` and listed here after it for readability).
    pub const ALL: [WorkState; 13] = [
        WorkState::Base(WorkerState::Idle),
        WorkState::Base(WorkerState::StartWorking),
        WorkState::Base(WorkerState::Preparing),
        WorkState::Base(WorkerState::Decide),
        WorkState::Base(WorkerState::InventoryFull),
        WorkState::Base(WorkerState::GatheringRequiredMaterials),
        WorkState::HerderBreed,
        WorkState::NetherLeave,
        WorkState::NetherAway,
        WorkState::NetherReturn,
        WorkState::NetherOpenPortal,
        WorkState::NetherClosePortal,
        WorkState::BeekeeperHarvest,
    ];

    /// Java's identifier, which is what a state history or the citizen's nametag shows
    /// (`AbstractEntityAIBasic.java:475`).
    pub fn java(self) -> &'static str {
        match self {
            WorkState::Base(state) => state.java_name(),
            WorkState::HerderBreed => "HERDER_BREED",
            WorkState::NetherLeave => "NETHER_LEAVE",
            WorkState::NetherAway => "NETHER_AWAY",
            WorkState::NetherReturn => "NETHER_RETURN",
            WorkState::NetherOpenPortal => "NETHER_OPENPORTAL",
            WorkState::NetherClosePortal => "NETHER_CLOSEPORTAL",
            WorkState::BeekeeperHarvest => "BEEKEEPER_HARVEST",
        }
    }

    /// The declaration of the constant, `AIWorkerState.java:NNN`.
    pub fn java_declaration(self) -> &'static str {
        match self {
            WorkState::Base(_) => "api/entity/ai/statemachine/states/AIWorkerState.java",
            WorkState::HerderBreed => "AIWorkerState.java:357",
            WorkState::NetherLeave => "AIWorkerState.java:555",
            WorkState::NetherAway => "AIWorkerState.java:560",
            WorkState::NetherReturn => "AIWorkerState.java:565",
            WorkState::NetherOpenPortal => "AIWorkerState.java:570",
            WorkState::NetherClosePortal => "AIWorkerState.java:575",
            WorkState::BeekeeperHarvest => "AIWorkerState.java:638",
        }
    }

    /// `AIWorkerState.isOkayToEat` for this state — the flag
    /// `AbstractAISkeleton.canBeInterrupted()` returns (`AbstractAISkeleton.java:
    /// 126-131`).
    ///
    /// True is "the citizen may walk away right now". The nether states are all true
    /// because the worker is either walking or away; `HERDER_BREED` is false, the one
    /// trade state of this slice Java marks as uninterruptible (`:357`).
    pub fn okay_to_eat(self) -> bool {
        match self {
            WorkState::Base(state) => state.okay_to_eat(),
            WorkState::HerderBreed => false, // AIWorkerState.java:357
            WorkState::NetherLeave
            | WorkState::NetherAway
            | WorkState::NetherReturn
            | WorkState::NetherOpenPortal
            | WorkState::NetherClosePortal => true, // :555-575
            WorkState::BeekeeperHarvest => true,     // :638
        }
    }

    /// The base state behind it, `None` for a state this module owns.
    pub fn worker(self) -> Option<WorkerState> {
        match self {
            WorkState::Base(state) => Some(state),
            _ => None,
        }
    }

    /// Whether `trade`'s table can be in this state.
    ///
    /// The seven own states are split by Java's own two constructors:
    /// `EntityAIWorkNether.java:102-106` registers the five nether ones,
    /// `EntityAIWorkBeekeeper.java:113-114` the two beekeeping ones.
    ///
    /// The base states are split the same way, and not by "they all belong to everyone":
    /// `PREPARING` and `DECIDE` are states **only the beekeeper registers**
    /// (`EntityAIWorkBeekeeper.java:111-112`) — the nether miner's parent never returns
    /// them (`AbstractEntityAICrafting.java:164-169`), so a nether machine can never be in
    /// them and a table that claims otherwise would name a state nothing can leave.
    pub fn belongs_to(self, trade: super::WorkTrade) -> bool {
        use super::WorkTrade;
        match self {
            // Registered by the hand-over of every trade (`super::register_handover`) and
            // by the base table: `afterDump` (`:1213-1222`) and `getNeededItem` (`:242`).
            WorkState::Base(WorkerState::Idle)
            | WorkState::Base(WorkerState::StartWorking)
            | WorkState::Base(WorkerState::InventoryFull)
            | WorkState::Base(WorkerState::GatheringRequiredMaterials) => true,
            // The beekeeper's own two base targets.
            WorkState::Base(WorkerState::Preparing) | WorkState::Base(WorkerState::Decide) => {
                trade == WorkTrade::Beekeeper
            }
            WorkState::Base(_) => false,
            WorkState::NetherLeave
            | WorkState::NetherAway
            | WorkState::NetherReturn
            | WorkState::NetherOpenPortal
            | WorkState::NetherClosePortal => trade == WorkTrade::Nether,
            WorkState::HerderBreed | WorkState::BeekeeperHarvest => trade == WorkTrade::Beekeeper,
        }
    }
}

impl fmt::Display for WorkState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.java())
    }
}

/// Why the AI went no further, named.
///
/// Java has no such type: a worker that cannot proceed answers `return getState()` and
/// leaves the reason implicit in the branch it took. A player asks "why is my nether
/// miner idle?" and a test has to assert on a name, so every variant carries the line
/// that decides it — and the two trades carry different reasons under the same state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: one variant per `return getState()` / named exit Java can take here
pub enum WorkRefusal {
    /// `isTravelling(citizenData)` — the colony is walking the citizen to a portal
    /// (`EntityAIWorkNether.java:186`).
    Travelling,
    /// `job.isInNether()` — nothing of the hut's business happens while away (`:186`).
    InNether,
    /// The crafting half answered a state of its own (`:203-206`); the port hands that
    /// half back to `src/crafting` instead of inventing its states.
    CraftingHalfOwnsIt,
    /// An input of the hut's first recipe is neither in the hut nor requested
    /// (`:225-236`).
    MissingInputs,
    /// The hut has no `portal` tag, or the tagged block is not where a portal stands
    /// (`:239-244`).
    NoPortal,
    /// `checkForToolOrWeapon` found no good enough tool among the five the job states
    /// (`:248-258`). The kind is the one Java asks for, in `Job::tools()`'s order.
    MissingTool(ToolKind),
    /// No recipe is left to send the worker to the nether with (`:278`, `:324-329`).
    NoRecipe,
    /// `building.isReadyForTrip()` is false: the hut already used its trip for this
    /// period (`:282-287`).
    TripNotReady,
    /// `!walkToBuilding()` — still walking to the hut (`:288-291`).
    NotAtHut,
    /// `!worker.getInventoryCitizen().hasSpace()` (`:292-295`).
    NoSpace,
    /// `checkForItems` answered `GET_RECIPE`: the inputs vanished mid-craft
    /// (`:299-305`).
    RecipeLost,
    /// The beekeeper's hut lists no hive at all (`EntityAIWorkBeekeeper.java:206-211`).
    NoHives,
    /// The breeding setting is on and the flower list is empty (`:214-219`).
    NoFlowers,
    /// No bee around the hives, and none in the hives either (`:231-247`).
    NoBees,
    /// Fewer than `NUM_OF_FLOWERS_TO_BREED` flowers and a request just went out
    /// (`:459-469`).
    FlowersMissing,
    /// The colony already has `buildingLevel * BEES_PER_LEVEL` bees (`:453` with
    /// `:130-137`).
    TooManyBees,
    /// `equipBreedItem` found no flower to hold (`:296-299`).
    NoBreedItem,
    /// `equipTool(shears)` or `equipItem(glass bottle)` failed (`:345-355`).
    NoHarvestItem,
    /// No hive of the hut is at honey level 5 or more (`:359-362`).
    NoHiveReady,
    /// The breeding setting is off (`:443-446`).
    BreedingOff,
    /// The fight in the nether killed the citizen (`:479-490`): Java clears both queues
    /// and answers `IDLE`, dropping whatever was left in the crafted queue.
    KilledInNether,
    /// The composter's six targets are `crate::ai::composter`'s; this module only
    /// reaches them through [`super::WorkerAi`], and this is the refusal a caller gets
    /// when it asks the wrong one for a table.
    NotThisTrade,
}

impl WorkRefusal {
    /// The Java line that decides this refusal, for a log line or a test failure.
    pub fn java(self) -> &'static str {
        match self {
            WorkRefusal::Travelling => "EntityAIWorkNether.java:186",
            WorkRefusal::InNether => "EntityAIWorkNether.java:186",
            WorkRefusal::CraftingHalfOwnsIt => "EntityAIWorkNether.java:203-206",
            WorkRefusal::MissingInputs => "EntityAIWorkNether.java:225-236",
            WorkRefusal::NoPortal => "EntityAIWorkNether.java:239-244",
            WorkRefusal::MissingTool(_) => "EntityAIWorkNether.java:248-258",
            WorkRefusal::NoRecipe => "EntityAIWorkNether.java:278",
            WorkRefusal::TripNotReady => "EntityAIWorkNether.java:282-287",
            WorkRefusal::NotAtHut => "EntityAIWorkNether.java:288-291",
            WorkRefusal::NoSpace => "EntityAIWorkNether.java:292-295",
            WorkRefusal::RecipeLost => "EntityAIWorkNether.java:299-305",
            WorkRefusal::NoHives => "EntityAIWorkBeekeeper.java:206-211",
            WorkRefusal::NoFlowers => "EntityAIWorkBeekeeper.java:214-219",
            WorkRefusal::NoBees => "EntityAIWorkBeekeeper.java:231-247",
            WorkRefusal::FlowersMissing => "EntityAIWorkBeekeeper.java:459-469",
            WorkRefusal::TooManyBees => "EntityAIWorkBeekeeper.java:453",
            WorkRefusal::NoBreedItem => "EntityAIWorkBeekeeper.java:296-299",
            WorkRefusal::NoHarvestItem => "EntityAIWorkBeekeeper.java:345-355",
            WorkRefusal::NoHiveReady => "EntityAIWorkBeekeeper.java:359-362",
            WorkRefusal::BreedingOff => "EntityAIWorkBeekeeper.java:443-446",
            WorkRefusal::KilledInNether => "EntityAIWorkNether.java:479-490",
            WorkRefusal::NotThisTrade => "see RAPPORT.md §1",
        }
    }

    /// A stable key for the message a player reads, the way `crate::ai::worker::Refusal`
    /// spells its own.
    pub fn key(self) -> &'static str {
        match self {
            WorkRefusal::Travelling => "ai_work.nether.refusal.travelling",
            WorkRefusal::InNether => "ai_work.nether.refusal.in_nether",
            WorkRefusal::CraftingHalfOwnsIt => "ai_work.nether.refusal.crafting_half",
            WorkRefusal::MissingInputs => "ai_work.nether.refusal.missing_inputs",
            WorkRefusal::NoPortal => "ai_work.nether.refusal.no_portal",
            WorkRefusal::MissingTool(_) => "ai_work.nether.refusal.missing_tool",
            WorkRefusal::NoRecipe => "ai_work.nether.refusal.no_recipe",
            WorkRefusal::TripNotReady => "ai_work.nether.refusal.trip_not_ready",
            WorkRefusal::NotAtHut => "ai_work.nether.refusal.not_at_hut",
            WorkRefusal::NoSpace => "ai_work.nether.refusal.no_space",
            WorkRefusal::RecipeLost => "ai_work.nether.refusal.recipe_lost",
            WorkRefusal::NoHives => "ai_work.beekeeper.refusal.no_hives",
            WorkRefusal::NoFlowers => "ai_work.beekeeper.refusal.no_flowers",
            WorkRefusal::NoBees => "ai_work.beekeeper.refusal.no_bees",
            WorkRefusal::FlowersMissing => "ai_work.beekeeper.refusal.flowers_missing",
            WorkRefusal::TooManyBees => "ai_work.beekeeper.refusal.too_many_bees",
            WorkRefusal::NoBreedItem => "ai_work.beekeeper.refusal.no_breed_item",
            WorkRefusal::NoHarvestItem => "ai_work.beekeeper.refusal.no_harvest_item",
            WorkRefusal::NoHiveReady => "ai_work.beekeeper.refusal.no_hive_ready",
            WorkRefusal::BreedingOff => "ai_work.beekeeper.refusal.breeding_off",
            WorkRefusal::KilledInNether => "ai_work.nether.refusal.killed",
            WorkRefusal::NotThisTrade => "ai_work.refusal.not_this_trade",
        }
    }

    /// Whether the reason is the player's to fix (a setting, a list) rather than the
    /// worker's to wait out.
    ///
    /// Java draws no such line; this port needs one, because a refusal a player can
    /// act on is the only one worth a chat message
    /// (`EntityAIWorkBeekeeper.java:208`, `:216`, `:238` and
    /// `EntityAIWorkComposter.java:395-396` all message the colony).
    pub fn asks_the_player(self) -> bool {
        matches!(
            self,
            WorkRefusal::NoHives
                | WorkRefusal::NoFlowers
                | WorkRefusal::NoBees
                | WorkRefusal::FlowersMissing
                | WorkRefusal::NoPortal
                | WorkRefusal::NoRecipe
                | WorkRefusal::MissingTool(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_owned_state_cites_a_line_of_the_java_enum() {
        for state in WorkState::ALL {
            if state.worker().is_some() {
                continue;
            }
            assert!(
                state.java_declaration().starts_with("AIWorkerState.java:"),
                "{state} must cite its declaration, got {}",
                state.java_declaration()
            );
            assert_eq!(state.java(), state.java().to_uppercase(), "{state}");
        }
    }

    #[test]
    fn the_seven_owned_states_are_split_by_the_two_java_constructors() {
        // `EntityAIWorkNether.java:102-106` registers five, the beekeeper's
        // `:113-114` two, and no trade owns the other's.
        let mut nether = 0;
        let mut beekeeper = 0;
        for state in WorkState::ALL {
            let to_nether = state.belongs_to(super::super::WorkTrade::Nether);
            let to_beekeeper = state.belongs_to(super::super::WorkTrade::Beekeeper);
            if to_nether && !to_beekeeper {
                nether += 1;
            }
            if to_beekeeper && !to_nether {
                beekeeper += 1;
            }
        }
        // The nether miner's five, plus nothing else: its parent never registers
        // `PREPARING` or `DECIDE` (`AbstractEntityAICrafting.java:164-169`).
        assert_eq!(nether, 5, "NETHER_LEAVE/AWAY/RETURN/OPENPORTAL/CLOSEPORTAL");
        // The beekeeper's two, plus the two base states only its own constructor adds
        // (`EntityAIWorkBeekeeper.java:111-112`).
        assert_eq!(beekeeper, 4, "HERDER_BREED, BEEKEEPER_HARVEST, PREPARING, DECIDE");
        // And a base state belongs to both (`AbstractEntityAIBasic.java:204-260`).
        let base = WorkState::Base(WorkerState::Idle);
        assert!(base.belongs_to(super::super::WorkTrade::Nether));
        assert!(base.belongs_to(super::super::WorkTrade::Beekeeper));
    }

    #[test]
    fn herder_breed_is_the_one_owned_state_that_is_not_interruptible() {
        // `AIWorkerState.java:357` — `HERDER_BREED(false)`, where every other state this
        // module *owns* passes `true` (`:555-575`, `:638`). The base states keep their
        // own flags: `INVENTORY_FULL` is `false` too (`:23`), and that is
        // `crate::ai`'s answer, not this module's.
        assert!(!WorkState::HerderBreed.okay_to_eat());
        for state in WorkState::ALL {
            if state.worker().is_some() || state == WorkState::HerderBreed {
                continue;
            }
            assert!(state.okay_to_eat(), "{state} may be interrupted");
        }
        // Both trade states a beekeeper can be in the middle of are interruptible; only
        // the breeding pair is not.
        assert!(WorkState::BeekeeperHarvest.okay_to_eat());
        assert!(WorkState::NetherAway.okay_to_eat());
    }

    #[test]
    fn a_refusal_names_the_line_that_decides_it() {
        for refusal in [
            WorkRefusal::Travelling,
            WorkRefusal::MissingInputs,
            WorkRefusal::NoPortal,
            WorkRefusal::NoHives,
            WorkRefusal::NoBees,
            WorkRefusal::NoHiveReady,
        ] {
            let java = refusal.java();
            assert!(
                java.starts_with("EntityAIWork") && java.contains(".java:"),
                "{refusal:?} must cite a Java line, got {java}"
            );
            assert!(
                refusal.key().starts_with("ai_work."),
                "{refusal:?} needs a message key"
            );
        }
        // The tool refusal carries which tool, because "a tool is missing" is not
        // actionable and "the pickaxe is missing" is.
        assert_eq!(
            WorkRefusal::MissingTool(ToolKind::Pickaxe).java(),
            WorkRefusal::MissingTool(ToolKind::Axe).java()
        );
        assert_ne!(
            WorkRefusal::MissingTool(ToolKind::Pickaxe).key(),
            WorkRefusal::NoSpace.key()
        );
    }
}
