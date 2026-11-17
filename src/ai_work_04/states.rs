//! The states of the two new tables, and the refusals the three trades name.
//!
//! Java reference: `api/entity/ai/statemachine/states/AIWorkerState.java` for the
//! states (`SIFT` at `:546`, `BREAK_ORES` at `:335`, `FILL_UP_FURNACES` at `:305`,
//! `RETRIEVING_END_PRODUCT_FROM_FURNACE` at `:315`,
//! `RETRIEVING_USED_FUEL_FROM_FURNACE` at `:325`), and the three AI classes plus their
//! two parents for the refusals.
//!
//! **Why two enums and not `crate::ai::state::WorkerState`.** That enum names eighteen
//! of `AIWorkerState`'s 127 states — the builder's and the composter's. `SIFT`,
//! `BREAK_ORES` and the three furnace states are not among them, and that file belongs
//! to another agent. Each trade therefore gets its own enum, like
//! `crate::ai_build::crafter::CraftState` did, and [`SiftState::base`] /
//! [`SmeltState::base`] say which of its states the base table also knows — that
//! mapping is the whole join between the two tables.
//!
//! **The refusal type is this module's, not `crate::ai::worker::Refusal`.** That one
//! names the base table's twenty reasons (`NotHired`, `HutFull`, `ToolMissing`, …).
//! None of the reasons below is one of them: a sifter with no mesh and a smelter whose
//! ore filter hides every ore are new, and a `Refusal::Base(...)` wrapper would make
//! every `match` two levels deep for no gain. Where the base class already names the
//! same situation, the doc says so instead of copying the name.

use crate::ai::state::WorkerState;

/// The four states the sifter's table can be in.
///
/// Java reference: `EntityAIWorkSifter` registers `SIFT` on top of its parent's six
/// (`EntityAIWorkSifter.java:72-74`), but its `decide()` answers `SIFT` for every
/// branch (`:91-94`), so `QUERY_ITEMS`, `GET_RECIPE` and `CRAFT` — registered at
/// `AbstractEntityAICrafting.java:167-169` — are **unreachable** for this trade and are
/// not given a state here. `GATHERING_REQUIRED_MATERIALS` is the same: only
/// `checkForItems` (`:494`) sends a crafter there, and `checkForItems` is only reached
/// from `queryItems`/`craft`, both unreachable. `sifter::tests::
/// the_table_has_four_targets_and_each_state_can_be_left` holds the target list, and the
/// test below holds the state list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: the four states a sifter can be in
pub enum SiftState {
    /// `IDLE` (`AIWorkerState.java:15`): look for work, or pass the time.
    Idle,
    /// `START_WORKING` (`:31`): `decide`, which this trade overrides to `SIFT`.
    StartWorking,
    /// `SIFT` (`:546`): one sieve step.
    Sift,
    /// `INVENTORY_FULL` (`:23`): no space, or a mesh in the pockets
    /// (`EntityAIWorkSifter.java:122`, `:139`).
    InventoryFull,
}

impl SiftState {
    /// Every state of this table, in Java's declaration order.
    pub const ALL: [SiftState; 4] = [
        SiftState::Idle,
        SiftState::StartWorking,
        SiftState::Sift,
        SiftState::InventoryFull,
    ];

    /// Java's identifier, which is what the nametag and the state history show
    /// (`AbstractEntityAIBasic.java:475`).
    pub fn java_name(self) -> &'static str {
        match self {
            SiftState::Idle => "IDLE",
            SiftState::StartWorking => "START_WORKING",
            SiftState::Sift => "SIFT",
            SiftState::InventoryFull => "INVENTORY_FULL",
        }
    }

    /// The state of the base table this one corresponds to, when there is one.
    ///
    /// This is the join: `crate::ai::worker` runs the fourteen targets of
    /// `AbstractEntityAIBasic` over `WorkerState`, this module runs the trade's
    /// targets over [`SiftState`], and a host that drives both must agree on the four
    /// names they share. `SIFT` has no counterpart — the base table never sends a
    /// citizen to a trade state.
    pub fn base(self) -> Option<WorkerState> {
        match self {
            SiftState::Idle => Some(WorkerState::Idle),
            SiftState::StartWorking => Some(WorkerState::StartWorking),
            SiftState::InventoryFull => Some(WorkerState::InventoryFull),
            SiftState::Sift => None,
        }
    }
}

impl core::fmt::Display for SiftState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.java_name())
    }
}

/// The eight states the smelter's table can be in.
///
/// Java reference: `AbstractEntityAIUsesFurnace.java:76-82` (six targets) and
/// `EntityAIWorkSmelter.java:69` (`BREAK_ORES`), plus the two the base class adds —
/// `INVENTORY_FULL`, reached from `checkForImportantJobs` (`EntityAIWorkSmelter.java:
/// 162-165`) and `GATHERING_REQUIRED_MATERIALS`, reached from `startWorking`
/// (`AbstractEntityAIUsesFurnace.java:252`, `:257`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: the eight states a smelter can be in
pub enum SmeltState {
    /// `IDLE` (`AIWorkerState.java:15`).
    Idle,
    /// `START_WORKING` (`:31`): the furnace user's decision point.
    StartWorking,
    /// `FILL_UP_FURNACES` (`:305`): put smeltable and fuel into one furnace.
    FillUpFurnaces,
    /// `RETRIEVING_END_PRODUCT_FROM_FURNACE` (`:315`): take the bars out.
    RetrievingEndProductFromFurnace,
    /// `RETRIEVING_USED_FUEL_FROM_FURNACE` (`:325`): take back a fuel the filter does
    /// not allow.
    RetrievingUsedFuelFromFurnace,
    /// `BREAK_ORES` (`:335`): the ore-breaking module's turn.
    BreakOres,
    /// `INVENTORY_FULL` (`:23`).
    InventoryFull,
    /// `GATHERING_REQUIRED_MATERIALS` (`:310`): walk to the hut's chest for ore or
    /// fuel.
    GatheringRequiredMaterials,
}

impl SmeltState {
    /// Every state of this table, in Java's declaration order where one exists.
    pub const ALL: [SmeltState; 8] = [
        SmeltState::Idle,
        SmeltState::StartWorking,
        SmeltState::FillUpFurnaces,
        SmeltState::RetrievingEndProductFromFurnace,
        SmeltState::RetrievingUsedFuelFromFurnace,
        SmeltState::BreakOres,
        SmeltState::InventoryFull,
        SmeltState::GatheringRequiredMaterials,
    ];

    /// Java's identifier.
    pub fn java_name(self) -> &'static str {
        match self {
            SmeltState::Idle => "IDLE",
            SmeltState::StartWorking => "START_WORKING",
            SmeltState::FillUpFurnaces => "FILL_UP_FURNACES",
            SmeltState::RetrievingEndProductFromFurnace => "RETRIEVING_END_PRODUCT_FROM_FURNACE",
            SmeltState::RetrievingUsedFuelFromFurnace => "RETRIEVING_USED_FUEL_FROM_FURNACE",
            SmeltState::BreakOres => "BREAK_ORES",
            SmeltState::InventoryFull => "INVENTORY_FULL",
            SmeltState::GatheringRequiredMaterials => "GATHERING_REQUIRED_MATERIALS",
        }
    }

    /// The base table's name for this state, when it has one — see
    /// [`SiftState::base`] for why the join is this small.
    pub fn base(self) -> Option<WorkerState> {
        match self {
            SmeltState::Idle => Some(WorkerState::Idle),
            SmeltState::StartWorking => Some(WorkerState::StartWorking),
            SmeltState::InventoryFull => Some(WorkerState::InventoryFull),
            SmeltState::GatheringRequiredMaterials => Some(WorkerState::GatheringRequiredMaterials),
            SmeltState::FillUpFurnaces
            | SmeltState::RetrievingEndProductFromFurnace
            | SmeltState::RetrievingUsedFuelFromFurnace
            | SmeltState::BreakOres => None,
        }
    }
}

impl core::fmt::Display for SmeltState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.java_name())
    }
}

/// Why a decision could not go further, named.
///
/// Java has no such type: a worker that cannot proceed `return <the same state>` or
/// `return IDLE`, and the reason stays implicit in the branch it took. A player asking
/// "why is my sifter idle?" needs the reason, and a test has to assert on it, so every
/// variant names the `return` it stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: one variant per "I cannot go further" branch of the three trades
pub enum Refusal {
    /// The queue is empty or the current task cannot be read
    /// (`AbstractEntityAICrafting.java:273-276`), or the ore module has nothing to
    /// propose (`EntityAIWorkSmelter.java:83-86`).
    NothingToCraft,
    /// `building.getCurrentDailyQuantity() >= building.getMaxDailyQuantity()`
    /// (`EntityAIWorkSifter.java:110-113`).
    QuotaReached,
    /// `!worker.getInventoryCitizen().hasSpace()` (`EntityAIWorkSifter.java:120-123`,
    /// `EntityAIWorkSmelter.java:162-165`).
    InventoryFull,
    /// The hut's racks hold no mesh and the worker holds none either
    /// (`EntityAIWorkSifter.java:133-146`): Java messages the player
    /// (`SIFTER_NO_MESH`) and waits `NO_MESH_DELAY`.
    NoMesh,
    /// The building has no mesh, but the worker carries one
    /// (`EntityAIWorkSifter.java:135-140`): the mesh goes back to the hut before
    /// anything else.
    MeshInInventory,
    /// No mesh in the hut's racks, so `getFirstFulfillableRecipe` answered null and
    /// nothing was asked for (`EntityAIWorkSifter.java:131-158`).
    NoRecipe,
    /// The recipe lacks its mesh or its input (`EntityAIWorkSifter.java:166-170`):
    /// Java forgets the recipe and stays.
    RecipeIncomplete,
    /// `fullfillRecipeAndCopy(...)` answered null after the sifter spent its progress
    /// (`EntityAIWorkSifter.java:199-203`).
    RecipeFailed,
    /// The sieve-speed skill level is negative, so the hit count has no answer
    /// (`EntityAIWorkSifter.java:188` through `AbstractEntityAIBasic.java:682-685`).
    /// Java would compute a meaningless threshold; the port names the broken fact.
    BrokenSkillLevel,
    /// `!walkToBuilding()`: still walking to the hut (`EntityAIWorkSifter.java:
    /// 115-118`, `AbstractEntityAIUsesFurnace.java:184-187`).
    Walking,
    /// `!walkToWorkPos(pos)`: still walking to the target furnace
    /// (`AbstractEntityAIBasic.java:800-806`).
    NotArrived,
    /// `furnaceModule.getFurnaces().isEmpty()` (`AbstractEntityAIUsesFurnace.java:
    /// 201-208`).
    NoFurnace,
    /// `itemListModule.getList().isEmpty()` (`AbstractEntityAIUsesFurnace.java:
    /// 193-199`): the hut has no fuel filter, so it will never be fuelled.
    NoFuelList,
    /// Every ore outside the filter is already broken, so there is nothing left to
    /// ask for (`EntityAIWorkSmelter.java:219-226`, `FURNACE_USER_NO_ORE`).
    NoOre,
    /// The ore-breaking module has no fulfillable recipe
    /// (`EntityAIWorkSmelter.java:83-86`).
    NoOreRecipe,
    /// The recipe's first input is empty (`EntityAIWorkSmelter.java:92-95`).
    NoOreInput,
    /// `module.fullFillRecipe(...)` answered false (`EntityAIWorkSmelter.java:
    /// 101-104`).
    OreBreakFailed,
    /// The furnace in `walkTo` is gone or is not a furnace block entity
    /// (`AbstractEntityAIUsesFurnace.java:400-406`, `:434-439`).
    FurnaceGone,
    /// The result slot is empty when the worker arrives
    /// (`AbstractEntityAIUsesFurnace.java:402`).
    ResultEmpty,
    /// The fuel slot is empty when the worker arrives
    /// (`AbstractEntityAIUsesFurnace.java:435`).
    FuelEmpty,
    /// `walkTo` is null or is no longer a furnace block
    /// (`AbstractEntityAIUsesFurnace.java:464-468`).
    TargetNotAFurnace,
    /// The worker carries neither smeltable nor fuel for this furnace
    /// (`AbstractEntityAIUsesFurnace.java:480-495`).
    NothingToFill,
}

impl Refusal {
    /// The Java line that decides this refusal, for a log line or a test failure.
    pub fn java(self) -> &'static str {
        match self {
            Refusal::NothingToCraft => "AbstractEntityAICrafting.java:273-276",
            Refusal::QuotaReached => "EntityAIWorkSifter.java:110-113",
            Refusal::InventoryFull => "EntityAIWorkSifter.java:120-123",
            Refusal::NoMesh => "EntityAIWorkSifter.java:133-146",
            Refusal::MeshInInventory => "EntityAIWorkSifter.java:135-140",
            Refusal::NoRecipe => "EntityAIWorkSifter.java:131-158",
            Refusal::RecipeIncomplete => "EntityAIWorkSifter.java:166-170",
            Refusal::RecipeFailed => "EntityAIWorkSifter.java:199-203",
            Refusal::BrokenSkillLevel => "EntityAIWorkSifter.java:188",
            Refusal::Walking => "EntityAIWorkSifter.java:115-118",
            Refusal::NotArrived => "AbstractEntityAIBasic.java:800-806",
            Refusal::NoFurnace => "AbstractEntityAIUsesFurnace.java:201-208",
            Refusal::NoFuelList => "AbstractEntityAIUsesFurnace.java:193-199",
            Refusal::NoOre => "EntityAIWorkSmelter.java:219-226",
            Refusal::NoOreRecipe => "EntityAIWorkSmelter.java:83-86",
            Refusal::NoOreInput => "EntityAIWorkSmelter.java:92-95",
            Refusal::OreBreakFailed => "EntityAIWorkSmelter.java:101-104",
            Refusal::FurnaceGone => "AbstractEntityAIUsesFurnace.java:400-406",
            Refusal::ResultEmpty => "AbstractEntityAIUsesFurnace.java:402",
            Refusal::FuelEmpty => "AbstractEntityAIUsesFurnace.java:435",
            Refusal::TargetNotAFurnace => "AbstractEntityAIUsesFurnace.java:464-468",
            Refusal::NothingToFill => "AbstractEntityAIUsesFurnace.java:480-495",
        }
    }

    /// A stable key for a message the player will read.
    pub fn key(self) -> &'static str {
        match self {
            Refusal::NothingToCraft => "ai.work.refusal.nothing_to_craft",
            Refusal::QuotaReached => "ai.work.refusal.quota_reached",
            Refusal::InventoryFull => "ai.work.refusal.inventory_full",
            Refusal::NoMesh => "ai.work.refusal.no_mesh",
            Refusal::MeshInInventory => "ai.work.refusal.mesh_in_inventory",
            Refusal::NoRecipe => "ai.work.refusal.no_recipe",
            Refusal::RecipeIncomplete => "ai.work.refusal.recipe_incomplete",
            Refusal::RecipeFailed => "ai.work.refusal.recipe_failed",
            Refusal::BrokenSkillLevel => "ai.work.refusal.broken_skill_level",
            Refusal::Walking => "ai.work.refusal.walking",
            Refusal::NotArrived => "ai.work.refusal.not_arrived",
            Refusal::NoFurnace => "ai.work.refusal.no_furnace",
            Refusal::NoFuelList => "ai.work.refusal.no_fuel_list",
            Refusal::NoOre => "ai.work.refusal.no_ore",
            Refusal::NoOreRecipe => "ai.work.refusal.no_ore_recipe",
            Refusal::NoOreInput => "ai.work.refusal.no_ore_input",
            Refusal::OreBreakFailed => "ai.work.refusal.ore_break_failed",
            Refusal::FurnaceGone => "ai.work.refusal.furnace_gone",
            Refusal::ResultEmpty => "ai.work.refusal.result_empty",
            Refusal::FuelEmpty => "ai.work.refusal.fuel_empty",
            Refusal::TargetNotAFurnace => "ai.work.refusal.target_not_a_furnace",
            Refusal::NothingToFill => "ai.work.refusal.nothing_to_fill",
        }
    }

    /// Every refusal this module can name — the list a message table has to cover.
    pub const ALL: [Refusal; 22] = [
        Refusal::NothingToCraft,
        Refusal::QuotaReached,
        Refusal::InventoryFull,
        Refusal::NoMesh,
        Refusal::MeshInInventory,
        Refusal::NoRecipe,
        Refusal::RecipeIncomplete,
        Refusal::RecipeFailed,
        Refusal::BrokenSkillLevel,
        Refusal::Walking,
        Refusal::NotArrived,
        Refusal::NoFurnace,
        Refusal::NoFuelList,
        Refusal::NoOre,
        Refusal::NoOreRecipe,
        Refusal::NoOreInput,
        Refusal::OreBreakFailed,
        Refusal::FurnaceGone,
        Refusal::ResultEmpty,
        Refusal::FuelEmpty,
        Refusal::TargetNotAFurnace,
        Refusal::NothingToFill,
    ];
}

impl core::fmt::Display for Refusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.key())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sifter_has_four_states_and_three_of_them_exist_in_the_base_table() {
        assert_eq!(SiftState::ALL.len(), 4);
        assert_eq!(SiftState::Sift.java_name(), "SIFT"); // AIWorkerState.java:546
        assert_eq!(SiftState::Idle.base(), Some(WorkerState::Idle));
        assert_eq!(
            SiftState::StartWorking.base(),
            Some(WorkerState::StartWorking)
        );
        assert_eq!(
            SiftState::InventoryFull.base(),
            Some(WorkerState::InventoryFull)
        );
        // The base table never sends a citizen to a trade state.
        assert_eq!(SiftState::Sift.base(), None);
    }

    #[test]
    fn the_smelter_has_eight_states_and_four_of_them_exist_in_the_base_table() {
        assert_eq!(SmeltState::ALL.len(), 8);
        assert_eq!(SmeltState::BreakOres.java_name(), "BREAK_ORES");
        assert_eq!(
            SmeltState::RetrievingEndProductFromFurnace.java_name(),
            "RETRIEVING_END_PRODUCT_FROM_FURNACE"
        );
        assert_eq!(
            SmeltState::RetrievingUsedFuelFromFurnace.java_name(),
            "RETRIEVING_USED_FUEL_FROM_FURNACE"
        );
        assert_eq!(SmeltState::FillUpFurnaces.java_name(), "FILL_UP_FURNACES");
        let named = SmeltState::ALL
            .iter()
            .filter(|state| state.base().is_some())
            .count();
        assert_eq!(named, 4, "IDLE, START_WORKING, INVENTORY_FULL, GATHERING…");
        assert_eq!(
            SmeltState::GatheringRequiredMaterials.base(),
            Some(WorkerState::GatheringRequiredMaterials)
        );
    }

    #[test]
    fn no_two_states_share_a_name_or_a_key() {
        let mut sift: Vec<&str> = SiftState::ALL.iter().map(|s| s.java_name()).collect();
        sift.sort_unstable();
        sift.dedup();
        assert_eq!(sift.len(), SiftState::ALL.len());
        let mut smelt: Vec<&str> = SmeltState::ALL.iter().map(|s| s.java_name()).collect();
        smelt.sort_unstable();
        smelt.dedup();
        assert_eq!(smelt.len(), SmeltState::ALL.len());
        let mut keys: Vec<&str> = Refusal::ALL.iter().map(|r| r.key()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), Refusal::ALL.len(), "two refusals share a key");
        // Every refusal cites a file and a line, and every Java citation is one.
        for refusal in Refusal::ALL {
            assert!(
                refusal.java().contains(".java:"),
                "{} has no java reference",
                refusal.key()
            );
        }
    }
}
