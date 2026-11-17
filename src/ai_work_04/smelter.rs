//! The smelter: five furnace decisions, one ore-breaking job, and the three slots of
//! the furnace it works at.
//!
//! Java reference: `core/entity/ai/workers/crafting/EntityAIWorkSmelter.java` — the
//! constants (`:54-59`), the `BREAK_ORES` target (`:69`), `breakOres` (`:78-122`),
//! `extractFromFurnace` (`:136-146`), `getSmeltAbleClass` (`:149-152`),
//! `checkForImportantJobs` (`:155-177`), `isSmeltable` (`:186-197`) and
//! `requestSmeltable` (`:200-238`); and its parent
//! `core/entity/ai/workers/AbstractEntityAIUsesFurnace.java` — the six targets
//! (`:76-82`), `reachedMaxToKeep` (`:117-121`), the two oven scans (`:129-173`),
//! `startWorking` (`:182-261`), `getAllowedFuel` (`:268-278`), `accelerateFurnaces`
//! (`:283-306`), `checkIfAbleToSmelt` (`:320-348`), the two retrievals (`:388-445`) and
//! `fillUpFurnace` (`:452-499`).
//!
//! ## The table
//!
//! | # | Pass / state | Rule | Rate | Java |
//! | ---: | --- | --- | ---: | --- |
//! | 1 | `IDLE` | → `START_WORKING`, no condition | 5 | `AbstractEntityAIUsesFurnace.java:77` |
//! | 2 | `START_WORKING` | `startWorking` | 60 | `:78` |
//! | 3 | `FILL_UP_FURNACES` | `fillUpFurnace` | 5 | `:79` |
//! | 4 | `AI_BLOCKING` | `accelerateFurnaces` (null: never stops the tick) | 20 | `:80` |
//! | 5 | `RETRIEVING_END_PRODUCT_FROM_FURNACE` | `retrieveSmeltableFromFurnace` | 5 | `:81` |
//! | 6 | `RETRIEVING_USED_FUEL_FROM_FURNACE` | `retrieveUsedFuel` | 5 | `:82` |
//! | 7 | `BREAK_ORES` | `breakOres` | 20 | `EntityAIWorkSmelter.java:69` |
//! | — | `INVENTORY_FULL` | base `afterDump` → `IDLE` | — | `AbstractEntityAIBasic.java:1213-1222` |
//! | — | `GATHERING_REQUIRED_MATERIALS` | base `getStateAfterPickUp` → `START_WORKING` | — | `AbstractEntityAIBasic.java:323-326` |
//!
//! **`startWorking` is one method with ten exits, and each one is a named state.** The
//! order is the rule, so [`start_working`] follows the Java line by line instead of
//! regrouping: walk (`:184`), two complaints (`:193`, `:201`), the important job
//! (`:210`), the fuel oven (`:216`), the product oven (`:223`), two requests
//! (`:237`, `:242`), two gatherings (`:249`, `:254`), and the fill (`:260`). A furnace
//! whose result slot is full is therefore emptied **before** more ore is fetched, and
//! a used fuel is retrieved **before** the bars — reversing those two is the mistake a
//! "clean" rewrite makes.
//!
//! ## The three numbers a smelter spends
//!
//! | Constant | Value | Java |
//! | --- | ---: | --- |
//! | `RETRIEVE_SMELTABLE_IF_MORE_THAN` | 10 | `:60`, read at `:141` |
//! | `STORAGE_BUFFER` | 3 | `:66`, read at `:120` |
//! | `STACKSIZE` | 64 | `Constants.java:23`, read at `:151`, `:246` |
//!
//! plus [`acceleration_ticks`], `(primarySkill / 10) * 2` extra furnace ticks per
//! second (`:285`), and the experience of one extraction: `BASE_XP_GAIN = 5`
//! (`EntityAIWorkSmelter.java:54`), not the parent's 2 (`AbstractEntityAIUsesFurnace.java:
//! 55`), because the smelter overrides `extractFromFurnace`.
//!
//! **A furnace is three slots and a lit flag.** `RESULT_SLOT = 2`, `SMELTABLE_SLOT = 0`
//! and `FUEL_SLOT = 1` (`Constants.java:118`, `:123`, `:128`), which is why
//! [`super::facts::Furnace`] carries exactly those three counts plus `lit` and the two
//! block questions. Nothing here calls a block entity.

use crate::ai::state::{Action, Blocking, Machine, Transition};
use crate::ai::worker::STANDARD_DELAY;
use crate::ai::{ENTITY_AI_TICKRATE, Pos};

use super::TICKS_SECOND;
use super::facts::{Ore, Scene, SmelterFacts};
use super::states::{Refusal, SmeltState};

/// Where this file's rules and numbers come from, as one string a log line or a doc
/// page can print instead of restating them.
pub const SOURCE: &str = "AbstractEntityAIUsesFurnace.java:55,:60,:66,:76-82,:117-121,:129-173,\
:182-261,:268-306,:320-348,:376-380,:388-445,:452-499 · EntityAIWorkSmelter.java:54-59,:69,\
:78-122,:136-152,:155-197,:200-238 · ItemStackUtils.java:719-747 · Constants.java:21,:23,:118,\
:123,:128 · AbstractEntityAIBasic.java:323-326,:1213-1222";

/// `EntityAIWorkSmelter.BASE_XP_GAIN` (`:54`): five experience per extraction, the
/// parent's 2 (`AbstractEntityAIUsesFurnace.java:55`) overridden (`:144`).
pub const BASE_XP_GAIN: f64 = 5.0;
/// `EntityAIWorkSmelter.ORE_LIST` (`:59`): the id of the hut's ore filter module.
pub const ORE_LIST: &str = "ores";
/// `EntityAIWorkSmelter`'s experience per broken ore (`:109`, `addExperience(0.2)`).
pub const ORE_BREAK_XP_GAIN: f64 = 0.2;
/// `AbstractEntityAIUsesFurnace.RETRIEVE_SMELTABLE_IF_MORE_THAN` (`:60`): above this
/// many bars, a **lit** furnace is harvested too (`:141`).
pub const RETRIEVE_SMELTABLE_IF_MORE_THAN: u32 = 10;
/// `AbstractEntityAIUsesFurnace.STORAGE_BUFFER` (`:66`): the hut's empty slots under
/// which no more smeltable is asked for (`:117-121`).
pub const STORAGE_BUFFER: u32 = 3;
/// `Constants.STACKSIZE` (`api/util/constant/Constants.java:23`): the size of one
/// smeltable or fuel request (`:151`, `:246`).
pub const STACKSIZE: u32 = 64;
/// `Constants.RESULT_SLOT` (`:118`).
pub const RESULT_SLOT: u32 = 2;
/// `Constants.SMELTABLE_SLOT` (`:123`).
pub const SMELTABLE_SLOT: u32 = 0;
/// `Constants.FUEL_SLOT` (`:128`).
pub const FUEL_SLOT: u32 = 1;
/// `AbstractEntityAIUsesFurnace.accelerateFurnaces`'s divisor: `(level / 10) * 2`
/// (`:285`).
pub const ACCELERATION_PER_TEN_LEVELS: u32 = 2;
/// `AbstractEntityAIUsesFurnace.java:78`: the rate of `startWorking`.
pub const START_WORKING_RATE: u32 = 60;

/// `(primarySkill / 10) * 2` (`AbstractEntityAIUsesFurnace.java:285`): how many extra
/// furnace ticks one AI tick buys, per lit furnace.
pub fn acceleration_ticks(primary_skill: u8) -> u32 {
    (u32::from(primary_skill) / 10) * ACCELERATION_PER_TEN_LEVELS
}

/// `reachedMaxToKeep` (`:117-121`): the hut has no more than [`STORAGE_BUFFER`] empty
/// slots, so asking for another stack would fill it.
pub fn reached_max_to_keep(empty_slots_in_building: u32) -> bool {
    empty_slots_in_building <= STORAGE_BUFFER
}

/// `getPositionOfOvenToRetrieveFrom` (`:129-149`): the first furnace whose bars are
/// ready.
///
/// Java's three conditions, in order: not lit with something in the result slot
/// (`:140`), more than [`RETRIEVE_SMELTABLE_IF_MORE_THAN`] bars (`:141`), or bars with
/// an empty input slot (`:142`). The second one intentionally harvests a **lit**
/// furnace: it is the "do not let the result slot clog" rule, and it is the reason this
/// scan runs before the ore request.
pub fn oven_to_retrieve_from(facts: &SmelterFacts) -> Option<Pos> {
    facts
        .furnaces_with_entity()
        .find(|furnace| {
            (!furnace.lit && furnace.result > 0)
                || furnace.result > RETRIEVE_SMELTABLE_IF_MORE_THAN
                || (furnace.result > 0 && furnace.input == 0)
        })
        .map(|furnace| furnace.at)
}

/// `getPositionOfOvenToRetrieveFuelFrom` (`:156-173`): the first furnace holding a fuel
/// the hut's filter does not allow (`:166`).
pub fn oven_to_retrieve_fuel_from(facts: &SmelterFacts) -> Option<Pos> {
    facts
        .furnaces_with_entity()
        .find(|furnace| furnace.fuel_present && !furnace.fuel_allowed)
        .map(|furnace| furnace.at)
}

/// The two important jobs `EntityAIWorkSmelter.checkForImportantJobs` can answer
/// (`:155-177`), before the furnace work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two early returns of `checkForImportantJobs`
pub enum ImportantJob {
    /// `!worker.getInventoryCitizen().hasSpace()` (`:162-165`).
    InventoryFull,
    /// The ore-breaking module has a fulfillable recipe (`:167-176`).
    BreakOres,
}

/// `checkForImportantJobs` (`:155-177`): something to do **before** the furnaces.
///
/// `None` is Java's `super.checkForImportantJobs()`, which answers `START_WORKING`
/// (`:365-368`). The hand is emptied first (`:157-160`) — an effect the caller performs,
/// recorded as [`SmelterFacts::main_hand_full`].
pub fn check_for_important_jobs(facts: &SmelterFacts) -> Option<ImportantJob> {
    if !facts.has_space {
        return Some(ImportantJob::InventoryFull); // :162-165
    }
    if facts.ore_recipe.is_some() {
        return Some(ImportantJob::BreakOres); // :167-176
    }
    None // `super.checkForImportantJobs()`, `:173`
}

/// What the smelter walks to the hut's chest for (`:249-258`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two `needsCurrently` tuples `startWorking` builds
pub enum Material {
    /// `needsCurrently = new Tuple<>(this::isSmeltable, STACKSIZE)` (`:251`).
    Smeltable,
    /// `needsCurrently = new Tuple<>(itemListModule::isItemInList, STACKSIZE)` (`:256`).
    Fuel,
}

/// What `startWorking` decided, side effects included.
///
/// Java returns one state and writes three other things on the way (`walkTo`, the two
/// `createRequestAsync` calls, the two interactions). A caller needs all of them, so
/// every one is a field: a request the player never sees created is a smelter that
/// silently does nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Start {
    /// The state `startWorking` returns.
    pub next: SmeltState,
    /// `walkTo = pos` (`:219`, `:227`, `:334`): the furnace to work at, when the branch
    /// picked one. `None` leaves the previous value alone, as Java does.
    pub walk_to: Option<Pos>,
    /// The gather the `GATHERING_REQUIRED_MATERIALS` branch asks for (`:251`, `:256`).
    pub gather: Option<Material>,
    /// `requestSmeltable()` was called (`:239`). **What** to ask for is
    /// [`request_smeltable`]'s answer.
    pub ask_smeltable: bool,
    /// A `StackList` of allowed fuel was requested (`:245-247`).
    pub ask_fuel: bool,
    /// `FURNACE_USER_NO_FUEL` was triggered (`:193-199`).
    pub complain_no_fuel_list: bool,
    /// `BAKER_HAS_NO_FURNACES_MESSAGE` was triggered (`:201-208`).
    pub complain_no_furnaces: bool,
    /// Why the tick cannot go further, when it cannot.
    pub refusal: Option<Refusal>,
}

impl Start {
    /// "Stay in `START_WORKING` and do nothing else": Java's `getState()` on a branch
    /// with no side effect.
    fn stay() -> Start {
        Start {
            next: SmeltState::StartWorking,
            walk_to: None,
            gather: None,
            ask_smeltable: false,
            ask_fuel: false,
            complain_no_fuel_list: false,
            complain_no_furnaces: false,
            refusal: None,
        }
    }
}

/// `startWorking` (`:182-261`), in Java's order.
pub fn start_working(facts: &SmelterFacts) -> Start {
    if !facts.at_building {
        return Start {
            refusal: Some(Refusal::Walking), // :184-187
            ..Start::stay()
        };
    }
    // :193-199 — the fuel filter is empty: message the player and **carry on**.
    let complain_no_fuel_list = facts.fuel_list_empty;
    if facts.furnaces.is_empty() {
        return Start {
            complain_no_fuel_list,
            complain_no_furnaces: true, // :201-208
            refusal: Some(Refusal::NoFurnace),
            ..Start::stay()
        };
    }
    if let Some(job) = check_for_important_jobs(facts) {
        return Start {
            complain_no_fuel_list,
            next: match job {
                ImportantJob::InventoryFull => SmeltState::InventoryFull, // :164
                ImportantJob::BreakOres => SmeltState::BreakOres,         // :176
            },
            ..Start::stay()
        };
    }
    // :216-221 — a fuel the filter refuses goes back **before** the bars.
    if let Some(at) = oven_to_retrieve_fuel_from(facts) {
        return Start {
            complain_no_fuel_list,
            next: SmeltState::RetrievingUsedFuelFromFurnace,
            walk_to: Some(at),
            ..Start::stay()
        };
    }
    // :223-228 — then the bars.
    if let Some(at) = oven_to_retrieve_from(facts) {
        return Start {
            complain_no_fuel_list,
            next: SmeltState::RetrievingEndProductFromFurnace,
            walk_to: Some(at),
            ..Start::stay()
        };
    }
    let smeltable_total = facts.smeltable_total(); // :230-231
    let fuel_total = facts.fuel_total(); // :233-235
    // :237-240 — nothing to smelt anywhere, and the hut can still store more.
    let ask_smeltable = smeltable_total == 0 && !reached_max_to_keep(facts.empty_slots_in_building);
    // :242-247 — nothing to burn anywhere, and no fuel request is already open.
    let ask_fuel = fuel_total == 0 && !facts.open_fuel_request;
    // :249-253 — the hut has smeltable, the pockets do not.
    if facts.smeltable_in_building > 0 && facts.smeltable_in_inventory == 0 {
        return Start {
            next: SmeltState::GatheringRequiredMaterials,
            gather: Some(Material::Smeltable),
            ask_smeltable,
            ask_fuel,
            complain_no_fuel_list,
            ..Start::stay()
        };
    }
    // :254-258 — the same for fuel.
    if facts.fuel_in_building > 0 && facts.fuel_in_inventory == 0 {
        return Start {
            next: SmeltState::GatheringRequiredMaterials,
            gather: Some(Material::Fuel),
            ask_smeltable,
            ask_fuel,
            complain_no_fuel_list,
            ..Start::stay()
        };
    }
    // :260 — `checkIfAbleToSmelt(fuel, smeltable)`.
    let able = check_if_able_to_smelt(facts);
    match able.fill {
        Some(at) => Start {
            next: SmeltState::FillUpFurnaces, // :335
            walk_to: Some(at),                // :334
            ask_smeltable,
            ask_fuel,
            complain_no_fuel_list,
            ..Start::stay()
        },
        None => Start {
            // `checkForAdditionalJobs()` (`:347`), which the furnace user answers
            // `START_WORKING` (`:355-358`).
            next: SmeltState::StartWorking,
            ask_smeltable,
            ask_fuel,
            complain_no_fuel_list,
            refusal: Some(Refusal::NothingToFill),
            ..Start::stay()
        },
    }
}

/// What `checkIfAbleToSmelt` found (`:320-348`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AbleToSmelt {
    /// The furnace to fill (`:334`), the first one whose contents match one of the
    /// three conditions.
    pub fill: Option<Pos>,
    /// The positions the module drops because the block is no longer a furnace
    /// (`:340-343`) — a write Java performs while scanning, kept as a list so the
    /// caller can perform it.
    pub removed: Vec<Pos>,
}

/// `checkIfAbleToSmelt(amountOfFuel, amountOfSmeltable)` (`:320-348`).
///
/// The three conditions are mutually exclusive on purpose (`:330-332`): one side is
/// there and the other is not, or both are missing. A furnace that already has both is
/// skipped — it is smelting.
pub fn check_if_able_to_smelt(facts: &SmelterFacts) -> AbleToSmelt {
    let fuel = facts.fuel_total();
    let smeltable = facts.smeltable_total();
    let mut removed = Vec::new();
    for furnace in &facts.furnaces {
        if furnace.entity {
            let wants = (fuel > 0 && furnace.has_smeltable_and_no_fuel())
                || (smeltable > 0 && furnace.has_fuel_and_no_smeltable())
                || (fuel > 0 && smeltable > 0 && furnace.has_neither());
            if wants {
                return AbleToSmelt {
                    fill: Some(furnace.at),
                    removed,
                };
            }
        } else if !furnace.block_is_furnace {
            removed.push(furnace.at); // :340-343
        }
    }
    AbleToSmelt {
        fill: None,
        removed,
    }
}

/// `isSmeltable` (`EntityAIWorkSmelter.java:186-197`): three tests, in order.
///
/// Java folds the empty test into the first one (`isEmpty(stack) || !IS_SMELTABLE.and(
/// isOre).test(stack)`), then refuses everything the ore-breaking module can crack
/// (`:192-195`) and everything the hut's `"ores"` list keeps for itself (`:196`).
pub fn is_smeltable(ore: Ore) -> bool {
    if ore.empty || !(ore.smeltable && ore.is_ore) {
        return false; // :188
    }
    if ore.breakable {
        return false; // :192-195
    }
    !ore.in_ore_list // :196
}

/// What `requestSmeltable` decided to do (`EntityAIWorkSmelter.java:200-238`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four exits of `requestSmeltable`
pub enum Demand {
    /// `createRequestAsync(getSmeltAbleClass())` (`:210`): a `SmeltableOre` for
    /// [`smelt_request_count`] items.
    SmeltableOre,
    /// `createRequestAsync(new StackList(requests, …))` (`:230-235`): the ores outside
    /// the hut's list.
    StackList,
    /// `FURNACE_USER_NO_ORE` (`:224`): the list already covers every ore there is.
    ComplainNoOre,
}

/// `requestSmeltable` (`:200-238`): `None` when the hut is already supplied.
pub fn request_smeltable(facts: &SmelterFacts) -> Option<Demand> {
    if facts.smeltable_request_already_covered {
        return None; // :202
    }
    if facts.open_smeltable_request {
        return None; // :203-205
    }
    if facts.ore_list_allows_any {
        return Some(Demand::SmeltableOre); // :208-211
    }
    if facts.ore_outside_list == 0 {
        return Some(Demand::ComplainNoOre); // :219-226
    }
    Some(Demand::StackList) // :229-235
}

/// `STACKSIZE * furnaceModule.getFurnaces().size()` (`:151`, `:232`): the size of one
/// smeltable or fuel request, one stack per furnace.
pub fn smelt_request_count(furnaces: usize) -> u32 {
    STACKSIZE.saturating_mul(furnaces as u32)
}

/// What `breakOres` did (`EntityAIWorkSmelter.java:78-122`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four exits of `breakOres`
pub enum BreakOres {
    /// `getFirstFulfillableRecipe(...) == null` (`:83-86`).
    NoRecipe,
    /// `inputItem.isEmpty()` (`:92-95`).
    NoInput,
    /// `module.fullFillRecipe(...)` answered false (`:101-104`).
    Failed,
    /// One ore broken (`:106-121`): `input` is the stack that went in.
    Broken {
        /// `inputItem.getCount()` (`:97`).
        input: u32,
    },
}

/// `breakOres` (`:78-122`).
///
/// The write (`module.fullFillRecipe`, `:101`) is the host's; it reports the answer in
/// [`SmelterFacts::ore_fulfilled`], and this rule decides what to do with it.
pub fn break_ores(facts: &SmelterFacts) -> BreakOres {
    let Some(recipe) = facts.ore_recipe else {
        return BreakOres::NoRecipe; // :83-86
    };
    if recipe.input_count == 0 {
        return BreakOres::NoInput; // :92-95
    }
    if !facts.ore_fulfilled {
        return BreakOres::Failed; // :101-104
    }
    BreakOres::Broken {
        input: recipe.input_count, // :106-121
    }
}

/// What one of the two retrievals did (`:388-445`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four exits each retrieval shares
pub enum Retrieve {
    /// `walkTo == null` (`:390-393`, `:423-426`).
    NoTarget,
    /// `!walkToWorkPos(walkTo)` (`:395-398`, `:428-431`): stay.
    NotArrived,
    /// The block entity is gone (`:400-406`, `:433-439`).
    Gone,
    /// The slot to take from is empty (`:402`, `:435`).
    Empty,
    /// The stack was taken (`:410`, `:443`).
    Extracted,
}

impl Retrieve {
    /// The state either retrieval answers: `START_WORKING`, or stay for `NotArrived`.
    pub fn next(self, state: SmeltState) -> SmeltState {
        match self {
            Retrieve::NotArrived => state,
            _ => SmeltState::StartWorking,
        }
    }

    /// Whether the target must be forgotten (`walkTo = null`, `:404`, `:408`, `:437`,
    /// `:441`).
    pub fn forgets_target(self) -> bool {
        matches!(self, Retrieve::Gone | Retrieve::Empty | Retrieve::Extracted)
    }
}

/// `retrieveSmeltableFromFurnace` (`:388-413`).
pub fn retrieve_product(facts: &SmelterFacts, walk_to: Option<Pos>) -> Retrieve {
    let Some(at) = walk_to else {
        return Retrieve::NoTarget; // :390-393
    };
    if !facts.at_walk_target {
        return Retrieve::NotArrived; // :395-398
    }
    let Some(furnace) = facts.furnaces.iter().find(|furnace| furnace.at == at) else {
        return Retrieve::Gone; // :400-401
    };
    if !furnace.entity {
        return Retrieve::Gone; // :401
    }
    if furnace.result == 0 {
        return Retrieve::Empty; // :402
    }
    Retrieve::Extracted // :410
}

/// `retrieveUsedFuel` (`:421-445`).
pub fn retrieve_used_fuel(facts: &SmelterFacts, walk_to: Option<Pos>) -> Retrieve {
    let Some(at) = walk_to else {
        return Retrieve::NoTarget; // :423-426
    };
    if !facts.at_walk_target {
        return Retrieve::NotArrived; // :428-431
    }
    let Some(furnace) = facts.furnaces.iter().find(|furnace| furnace.at == at) else {
        return Retrieve::Gone;
    };
    if !furnace.entity {
        return Retrieve::Gone; // :434
    }
    if !furnace.fuel_present {
        return Retrieve::Empty; // :435
    }
    Retrieve::Extracted // :443
}

/// What `fillUpFurnace` did (`:452-499`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four exits of `fillUpFurnace`
pub enum Fill {
    /// `furnaceModule.getFurnaces().isEmpty()` (`:454-462`).
    NoFurnaces,
    /// `walkTo == null` or its block is no longer a furnace (`:464-468`).
    NoTarget,
    /// `!walkToWorkPos(walkTo)` (`:470-473`): stay.
    NotArrived,
    /// The furnace was visited: what went in is said by the two booleans
    /// (`:480-495`). Both false means the worker carried nothing to put in
    /// (`:480-495` matched neither test).
    Filled {
        /// The smeltable went in.
        smeltable: bool,
        /// The fuel went in.
        fuel: bool,
    },
}

/// `fillUpFurnace` (`:452-499`).
///
/// **The two transfers read the furnace in sequence.** `:480` puts the smeltable in
/// when the furnace has no input; `:489` then re-reads the same block entity — which the
/// first transfer just changed — so a furnace that had **neither** receives both in one
/// visit. Worked out, the two tests collapse to "put the smeltable in if the furnace has
/// none" and "put the fuel in if the furnace has none", and the port writes them that
/// way instead of keeping a mutable snapshot. The tests pin both cases: a furnace with
/// neither gets both, a furnace with ore gets only the fuel.
pub fn fill_up_furnace(facts: &SmelterFacts, walk_to: Option<Pos>) -> Fill {
    if facts.furnaces.is_empty() {
        return Fill::NoFurnaces; // :454-462
    }
    let Some(at) = walk_to else {
        return Fill::NoTarget; // :464-468
    };
    let Some(furnace) = facts.furnaces.iter().find(|furnace| furnace.at == at) else {
        return Fill::NoTarget; // the module no longer knows this position
    };
    if !furnace.block_is_furnace {
        return Fill::NoTarget; // :464
    }
    if !facts.at_walk_target {
        return Fill::NotArrived; // :470-473
    }
    // :480-486 — `hasFuelInFurnaceAndNoSmeltable || hasNeitherFuelNorSmeltAble`, which
    // is "the furnace has no input yet".
    let smeltable = facts.inventory_has_smeltable && !furnace.has_smeltable();
    // :489-495 — `hasSmeltableInFurnaceAndNoFuel || hasNeitherFuelNorSmeltAble`, read
    // after the first transfer, which is "the furnace has no fuel yet".
    let fuel = facts.inventory_has_fuel && !furnace.has_fuel();
    Fill::Filled { smeltable, fuel }
}

// ------------------------------------------------------------------- actions

/// `:78` — `startWorking`, with the four side effects Java writes on the way.
fn start_action(scene: &mut Scene<SmelterFacts>, _: SmeltState) -> Option<SmeltState> {
    // `:157-160` — `checkForImportantJobs` empties the main hand. It is reached only
    // after the walk (`:184`), the furnace check (`:201`) and nothing else, so the port
    // guards it with the same two facts instead of clearing the hand on every tick.
    if scene.facts.at_building && !scene.facts.furnaces.is_empty() {
        scene.facts.main_hand_full = false;
    }
    let start = start_working(&scene.facts);
    if start.complain_no_fuel_list {
        scene.refuse(Refusal::NoFuelList); // :193-199
    }
    if start.complain_no_furnaces {
        scene.refuse(Refusal::NoFurnace); // :201-208
    }
    if let Some(walk_to) = start.walk_to {
        scene.memory.walk_to = Some(walk_to); // `walkTo = pos`
    }
    scene.facts.asked_smeltable = start.ask_smeltable; // :239
    scene.facts.asked_fuel = start.ask_fuel; // :245-247
    if start.ask_smeltable {
        // `:239` calls `requestSmeltable`, which is where the `FURNACE_USER_NO_ORE`
        // message is decided (`:219-226`).
        if let Some(Demand::ComplainNoOre) = request_smeltable(&scene.facts) {
            scene.refuse(Refusal::NoOre);
        }
    }
    if let Some(refusal) = start.refusal {
        scene.refuse(refusal);
    }
    Some(start.next)
}

/// `:81` — `retrieveSmeltableFromFurnace`.
fn retrieve_product_action(
    scene: &mut Scene<SmelterFacts>,
    state: SmeltState,
) -> Option<SmeltState> {
    let decision = retrieve_product(&scene.facts, scene.memory.walk_to);
    match decision {
        Retrieve::NoTarget => Some(SmeltState::StartWorking), // :392
        Retrieve::NotArrived => {
            scene.refuse(Refusal::NotArrived);
            Some(state) // :397
        }
        Retrieve::Gone => {
            scene.memory.walk_to = None; // :404
            scene.refuse(Refusal::FurnaceGone);
            Some(SmeltState::StartWorking) // :405
        }
        Retrieve::Empty => {
            scene.memory.walk_to = None; // :404
            scene.refuse(Refusal::ResultEmpty); // :402
            Some(SmeltState::StartWorking)
        }
        Retrieve::Extracted => {
            scene.memory.walk_to = None; // :408
            scene.facts.actions_done += 1; // `incrementActionsDoneAndDecSaturation`, :411
            Some(SmeltState::StartWorking) // :412
        }
    }
}

/// `:82` — `retrieveUsedFuel`.
fn retrieve_fuel_action(scene: &mut Scene<SmelterFacts>, state: SmeltState) -> Option<SmeltState> {
    let decision = retrieve_used_fuel(&scene.facts, scene.memory.walk_to);
    match decision {
        Retrieve::NoTarget => Some(SmeltState::StartWorking), // :425
        Retrieve::NotArrived => {
            scene.refuse(Refusal::NotArrived);
            Some(state) // :430
        }
        Retrieve::Gone => {
            scene.memory.walk_to = None; // :437
            scene.refuse(Refusal::FurnaceGone);
            Some(SmeltState::StartWorking) // :438
        }
        Retrieve::Empty => {
            scene.memory.walk_to = None; // :437
            scene.refuse(Refusal::FuelEmpty); // :435
            Some(SmeltState::StartWorking)
        }
        Retrieve::Extracted => {
            scene.memory.walk_to = None; // :441
            Some(SmeltState::StartWorking) // :444
        }
    }
}

/// `:79` — `fillUpFurnace`.
fn fill_action(scene: &mut Scene<SmelterFacts>, state: SmeltState) -> Option<SmeltState> {
    match fill_up_furnace(&scene.facts, scene.memory.walk_to) {
        Fill::NoFurnaces => {
            scene.refuse(Refusal::NoFurnace); // :456-460
            Some(SmeltState::StartWorking) // :461
        }
        Fill::NoTarget => {
            scene.memory.walk_to = None; // :466
            scene.refuse(Refusal::TargetNotAFurnace);
            Some(SmeltState::StartWorking) // :467
        }
        Fill::NotArrived => {
            scene.refuse(Refusal::NotArrived);
            Some(state) // :472
        }
        Fill::Filled { smeltable, fuel } => {
            if !smeltable && !fuel {
                scene.refuse(Refusal::NothingToFill); // :480-495 matched neither
            }
            scene.memory.walk_to = None; // :497
            Some(SmeltState::StartWorking) // :498
        }
    }
}

/// `EntityAIWorkSmelter.java:69` — `breakOres`.
fn break_action(scene: &mut Scene<SmelterFacts>, state: SmeltState) -> Option<SmeltState> {
    match break_ores(&scene.facts) {
        BreakOres::NoRecipe => {
            scene.refuse(Refusal::NoOreRecipe); // :85
            Some(SmeltState::Idle)
        }
        BreakOres::NoInput => {
            scene.refuse(Refusal::NoOreInput); // :94
            Some(SmeltState::Idle)
        }
        BreakOres::Failed => {
            scene.refuse(Refusal::OreBreakFailed); // :103
            Some(SmeltState::Idle)
        }
        BreakOres::Broken { .. } => Some(state), // :121, `getState()`
    }
}

/// Register the smelter's table: the six targets of `AbstractEntityAIUsesFurnace.java:
/// 76-82` in Java's order, then `EntityAIWorkSmelter.java:69`, then the two exits the
/// base class adds.
pub fn register(machine: &mut Machine<SmeltState, Scene<SmelterFacts>>) {
    // :77 — `new AITarget(IDLE, START_WORKING, STANDARD_DELAY)`, with no condition.
    machine.add(Transition::goto(
        SmeltState::Idle,
        SmeltState::StartWorking,
        "idleToStartWorking",
        "AbstractEntityAIUsesFurnace.java:77",
        STANDARD_DELAY,
    ));
    // :78 — `new AITarget(START_WORKING, this::startWorking, 60)`.
    machine.add(Transition::target(
        SmeltState::StartWorking,
        "startWorking",
        "AbstractEntityAIUsesFurnace.java:78",
        START_WORKING_RATE,
        Action::Run(start_action),
    ));
    // :79 — `new AITarget(FILL_UP_FURNACES, this::fillUpFurnace, STANDARD_DELAY)`.
    machine.add(Transition::target(
        SmeltState::FillUpFurnaces,
        "fillUpFurnace",
        "AbstractEntityAIUsesFurnace.java:79",
        STANDARD_DELAY,
        Action::Run(fill_action),
    ));
    // :80 — `new AIEventTarget(AI_BLOCKING, this::accelerateFurnaces, TICKS_SECOND)`.
    // It returns null (`:305`), so it never stops a tick; the pass order alone puts it
    // before every state's own list (`TickRateStateMachine.java:75-81`).
    machine.add(Transition::event(
        Blocking::AiBlocking,
        "accelerateFurnaces",
        "AbstractEntityAIUsesFurnace.java:80",
        TICKS_SECOND,
        Action::Run(|_, _| None),
    ));
    // :81 — `new AITarget(RETRIEVING_END_PRODUCT_FROM_FURNACE, …, STANDARD_DELAY)`.
    machine.add(Transition::target(
        SmeltState::RetrievingEndProductFromFurnace,
        "retrieveSmeltableFromFurnace",
        "AbstractEntityAIUsesFurnace.java:81",
        STANDARD_DELAY,
        Action::Run(retrieve_product_action),
    ));
    // :82 — `new AITarget(RETRIEVING_USED_FUEL_FROM_FURNACE, …, STANDARD_DELAY)`.
    machine.add(Transition::target(
        SmeltState::RetrievingUsedFuelFromFurnace,
        "retrieveUsedFuel",
        "AbstractEntityAIUsesFurnace.java:82",
        STANDARD_DELAY,
        Action::Run(retrieve_fuel_action),
    ));
    // EntityAIWorkSmelter.java:69 — `new AITarget(BREAK_ORES, this::breakOres,
    // TICKS_SECOND)`, registered after the parent's six because the subclass
    // constructor runs after `super(...)`.
    machine.add(Transition::target(
        SmeltState::BreakOres,
        "breakOres",
        "EntityAIWorkSmelter.java:69",
        TICKS_SECOND,
        Action::Run(break_action),
    ));
    // The base class's `afterDump` (`AbstractEntityAIBasic.java:1213-1222`).
    machine.add(Transition::goto(
        SmeltState::InventoryFull,
        SmeltState::Idle,
        "afterDump",
        "AbstractEntityAIBasic.java:1213-1222",
        1,
    ));
    // `getStateAfterPickUp()`, which a furnace user does not override: `START_WORKING`
    // (`AbstractEntityAIBasic.java:323-326`).
    machine.add(Transition::goto(
        SmeltState::GatheringRequiredMaterials,
        SmeltState::StartWorking,
        "getStateAfterPickUp",
        "AbstractEntityAIBasic.java:323-326",
        1,
    ));
}

/// A smelter's machine, started in `IDLE`.
pub fn ai() -> Machine<SmeltState, Scene<SmelterFacts>> {
    let mut machine = Machine::new(SmeltState::Idle, ENTITY_AI_TICKRATE);
    register(&mut machine);
    machine
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::state::Blocking;
    use crate::ai::worker::STANDARD_DELAY;
    use crate::ai_work_04::TICKS_SECOND;
    use crate::ai_work_04::facts::{Furnace, OreRecipe};

    fn at(x: i32) -> Pos {
        Pos::new(x, 65, 1)
    }

    #[test]
    fn accelerating_a_furnace_is_two_ticks_per_ten_skill_levels() {
        // `AbstractEntityAIUsesFurnace.java:285`.
        assert_eq!(acceleration_ticks(0), 0);
        assert_eq!(acceleration_ticks(9), 0);
        assert_eq!(acceleration_ticks(10), 2);
        assert_eq!(acceleration_ticks(99), 18);
        assert_eq!(ACCELERATION_PER_TEN_LEVELS, 2);
    }

    #[test]
    fn the_storage_buffer_stops_the_smeltable_request() {
        // `:117-121` with `STORAGE_BUFFER = 3` (`:66`).
        assert!(!reached_max_to_keep(27));
        assert!(!reached_max_to_keep(4));
        assert!(reached_max_to_keep(3));
        assert!(reached_max_to_keep(0));
        assert_eq!(STORAGE_BUFFER, 3);
    }

    #[test]
    fn a_cold_furnace_with_bars_is_harvested_and_a_busy_one_only_past_ten() {
        // `:140-143`, the three conditions in order.
        let cold = SmelterFacts {
            furnaces: vec![Furnace {
                lit: false,
                result: 1,
                input: 5,
                ..Furnace::empty(at(1))
            }],
            ..SmelterFacts::new()
        };
        assert_eq!(oven_to_retrieve_from(&cold), Some(at(1)));

        let busy_small = SmelterFacts {
            furnaces: vec![Furnace::burning(at(1), 5, 5)],
            ..SmelterFacts::new()
        };
        assert_eq!(oven_to_retrieve_from(&busy_small), None);

        // `:141` — a lit furnace whose result slot passed ten is harvested anyway.
        let busy_full = SmelterFacts {
            furnaces: vec![Furnace::burning(at(1), 5, 11)],
            ..SmelterFacts::new()
        };
        assert_eq!(oven_to_retrieve_from(&busy_full), Some(at(1)));
        assert_eq!(RETRIEVE_SMELTABLE_IF_MORE_THAN, 10);

        // `:142` — bars with nothing left to smelt.
        let drained = SmelterFacts {
            furnaces: vec![Furnace::burning(at(1), 0, 3)],
            ..SmelterFacts::new()
        };
        assert_eq!(oven_to_retrieve_from(&drained), Some(at(1)));
    }

    #[test]
    fn the_first_ready_furnace_wins_and_a_furnace_without_entity_is_skipped() {
        // `:131-147` walks `getFurnaces()` in the order the hut reports them.
        let facts = SmelterFacts {
            furnaces: vec![
                Furnace {
                    at: at(1),
                    entity: false,
                    block_is_furnace: true,
                    result: 4,
                    ..Furnace::default()
                },
                Furnace::burning(at(2), 5, 0),
                Furnace {
                    at: at(3),
                    lit: false,
                    result: 2,
                    ..Furnace::empty(at(3))
                },
            ],
            ..SmelterFacts::new()
        };
        assert_eq!(oven_to_retrieve_from(&facts), Some(at(3)));
    }

    #[test]
    fn a_fuel_the_filter_refuses_is_taken_back() {
        // `:156-173` — `!getItem(FUEL_SLOT).isEmpty() && !module.isItemInList(...)`.
        let refused = SmelterFacts {
            furnaces: vec![Furnace {
                fuel_present: true,
                fuel_allowed: false,
                ..Furnace::empty(at(1))
            }],
            ..SmelterFacts::new()
        };
        assert_eq!(oven_to_retrieve_fuel_from(&refused), Some(at(1)));
        let allowed = SmelterFacts {
            furnaces: vec![Furnace {
                fuel_present: true,
                fuel_allowed: true,
                ..Furnace::empty(at(1))
            }],
            ..SmelterFacts::new()
        };
        assert_eq!(oven_to_retrieve_fuel_from(&allowed), None);
    }

    #[test]
    fn the_important_job_comes_before_every_furnace_rule() {
        // `EntityAIWorkSmelter.java:155-177`.
        let full = SmelterFacts {
            has_space: false,
            ..SmelterFacts::new()
        };
        assert_eq!(
            check_for_important_jobs(&full),
            Some(ImportantJob::InventoryFull)
        );
        let breaking = SmelterFacts {
            ore_recipe: Some(OreRecipe { input_count: 1 }),
            ..SmelterFacts::new()
        };
        assert_eq!(
            check_for_important_jobs(&breaking),
            Some(ImportantJob::BreakOres)
        );
        assert_eq!(check_for_important_jobs(&SmelterFacts::new()), None);
    }

    #[test]
    fn a_smelter_away_from_its_hut_walks_first() {
        // `:184-187`.
        let facts = SmelterFacts {
            at_building: false,
            ..SmelterFacts::new()
        };
        let start = start_working(&facts);
        assert_eq!(start.next, SmeltState::StartWorking);
        assert_eq!(start.refusal, Some(Refusal::Walking));
    }

    #[test]
    fn a_hut_without_furnaces_complains_and_stays() {
        // `:201-208`.
        let facts = SmelterFacts {
            furnaces: Vec::new(),
            ..SmelterFacts::new()
        };
        let start = start_working(&facts);
        assert_eq!(start.next, SmeltState::StartWorking);
        assert!(start.complain_no_furnaces);
        assert_eq!(start.refusal, Some(Refusal::NoFurnace));
    }

    #[test]
    fn an_empty_fuel_filter_complains_but_does_not_stop_the_tick() {
        // `:193-199` has no `return`.
        let facts = SmelterFacts {
            fuel_list_empty: true,
            smeltable_in_building: 0,
            fuel_in_building: 0,
            ..SmelterFacts::new()
        };
        let start = start_working(&facts);
        assert!(start.complain_no_fuel_list);
        // The tick still reached the requests and the fill.
        assert!(start.ask_smeltable);
        assert_eq!(start.refusal, Some(Refusal::NothingToFill));
    }

    #[test]
    fn the_used_fuel_is_retrieved_before_the_bars() {
        // `:216` is checked before `:223`.
        let facts = SmelterFacts {
            furnaces: vec![
                Furnace {
                    at: at(1),
                    lit: false,
                    result: 4,
                    ..Furnace::empty(at(1))
                },
                Furnace {
                    at: at(2),
                    fuel_present: true,
                    fuel_allowed: false,
                    ..Furnace::empty(at(2))
                },
            ],
            ..SmelterFacts::new()
        };
        let start = start_working(&facts);
        assert_eq!(start.next, SmeltState::RetrievingUsedFuelFromFurnace);
        assert_eq!(start.walk_to, Some(at(2)));
    }

    #[test]
    fn bars_are_harvested_before_more_ore_is_asked_for() {
        // `:223-228` comes before `:237-240`.
        let facts = SmelterFacts {
            furnaces: vec![Furnace {
                lit: false,
                result: 2,
                ..Furnace::empty(at(1))
            }],
            smeltable_in_building: 0,
            ..SmelterFacts::new()
        };
        let start = start_working(&facts);
        assert_eq!(start.next, SmeltState::RetrievingEndProductFromFurnace);
        assert!(!start.ask_smeltable, "the harvest branch returns first");
    }

    #[test]
    fn a_hut_with_nothing_to_smelt_asks_and_the_buffer_stops_it() {
        // `:237-240`: `<= 0 && !reachedMaxToKeep()`.
        let empty = SmelterFacts {
            smeltable_in_building: 0,
            smeltable_in_inventory: 0,
            ..SmelterFacts::new()
        };
        assert!(start_working(&empty).ask_smeltable);

        let stuffed = SmelterFacts {
            empty_slots_in_building: STORAGE_BUFFER,
            ..empty
        };
        assert!(!start_working(&stuffed).ask_smeltable, "the hut is full");
    }

    #[test]
    fn a_hut_with_nothing_to_burn_asks_once() {
        // `:242-247`.
        let empty = SmelterFacts {
            fuel_in_building: 0,
            fuel_in_inventory: 0,
            ..SmelterFacts::new()
        };
        assert!(start_working(&empty).ask_fuel);
        let asked = SmelterFacts {
            open_fuel_request: true,
            ..empty
        };
        assert!(!start_working(&asked).ask_fuel);
    }

    #[test]
    fn the_hut_inventory_is_fetched_before_the_furnaces_are_filled() {
        // `:249-258`: the two gathering branches come before `:260`.
        let ore_first = SmelterFacts {
            smeltable_in_building: 4,
            smeltable_in_inventory: 0,
            ..SmelterFacts::new()
        };
        let start = start_working(&ore_first);
        assert_eq!(start.next, SmeltState::GatheringRequiredMaterials);
        assert_eq!(start.gather, Some(Material::Smeltable));

        let fuel_first = SmelterFacts {
            smeltable_in_building: 0,
            smeltable_in_inventory: 4,
            fuel_in_building: 4,
            fuel_in_inventory: 0,
            ..SmelterFacts::new()
        };
        let start = start_working(&fuel_first);
        assert_eq!(start.next, SmeltState::GatheringRequiredMaterials);
        assert_eq!(start.gather, Some(Material::Fuel));
    }

    #[test]
    fn a_furnace_missing_one_side_is_the_one_to_fill() {
        // `:320-348`, the three conditions. The pockets **already hold** the ore and the
        // fuel, because `:249-258` (the two gathering branches) come before `:260`: with
        // an empty pocket and a full hut, Java gathers and never reaches
        // `checkIfAbleToSmelt` — `the_hut_inventory_is_fetched_before_the_furnaces_are_filled`
        // holds that order.
        let stocked = SmelterFacts {
            smeltable_in_building: 0,
            smeltable_in_inventory: 8,
            fuel_in_building: 0,
            fuel_in_inventory: 8,
            inventory_has_smeltable: true,
            inventory_has_fuel: true,
            ..SmelterFacts::new()
        };
        let unfuelled = SmelterFacts {
            furnaces: vec![Furnace {
                input: 3,
                ..Furnace::empty(at(1))
            }],
            ..stocked.clone()
        };
        let start = start_working(&unfuelled);
        assert_eq!(start.next, SmeltState::FillUpFurnaces);
        assert_eq!(start.walk_to, Some(at(1)));

        // A furnace that already has both is smelting: nothing to do.
        let busy = SmelterFacts {
            furnaces: vec![Furnace::burning(at(1), 3, 0)],
            ..stocked
        };
        let start = start_working(&busy);
        assert_eq!(start.next, SmeltState::StartWorking);
        assert_eq!(start.refusal, Some(Refusal::NothingToFill));
    }

    #[test]
    fn a_position_whose_block_is_no_longer_a_furnace_is_dropped() {
        // `:338-344` — the `else` branch of the scan.
        let facts = SmelterFacts {
            furnaces: vec![Furnace {
                at: at(1),
                entity: false,
                block_is_furnace: false,
                ..Furnace::default()
            }],
            ..SmelterFacts::new()
        };
        let able = check_if_able_to_smelt(&facts);
        assert_eq!(able.fill, None);
        assert_eq!(able.removed, vec![at(1)]);
        // A position with no entity that *is* a furnace block stays in the module.
        let mut still = facts.clone();
        still.furnaces[0].block_is_furnace = true;
        assert!(check_if_able_to_smelt(&still).removed.is_empty());
    }

    #[test]
    fn an_ore_is_smelted_unless_it_is_breakable_or_kept_by_the_hut() {
        // `EntityAIWorkSmelter.java:186-197`.
        assert!(is_smeltable(Ore::ore()));
        assert!(!is_smeltable(Ore {
            breakable: true,
            ..Ore::ore()
        }));
        assert!(!is_smeltable(Ore {
            in_ore_list: true,
            ..Ore::ore()
        }));
        assert!(!is_smeltable(Ore {
            empty: true,
            ..Ore::ore()
        }));
        assert!(!is_smeltable(Ore {
            smeltable: false,
            ..Ore::ore()
        }));
        // An ore the compatibility manager does not know is not smelted either.
        assert!(!is_smeltable(Ore {
            is_ore: false,
            ..Ore::ore()
        }));
    }

    #[test]
    fn the_smeltable_request_has_three_shapes() {
        // `:200-238`.
        let covered = SmelterFacts {
            smeltable_request_already_covered: true,
            ..SmelterFacts::new()
        };
        assert_eq!(request_smeltable(&covered), None);
        let open = SmelterFacts {
            open_smeltable_request: true,
            ..SmelterFacts::new()
        };
        assert_eq!(request_smeltable(&open), None);
        // `allowedItems.isEmpty()` → the generic `SmeltableOre` (`:208-211`).
        let unfiltered = SmelterFacts {
            ore_list_allows_any: true,
            ..SmelterFacts::new()
        };
        assert_eq!(request_smeltable(&unfiltered), Some(Demand::SmeltableOre));
        // A filter that already hides every ore is a blocking message (`:219-226`).
        let all_filtered = SmelterFacts {
            ore_outside_list: 0,
            ..SmelterFacts::new()
        };
        assert_eq!(
            request_smeltable(&all_filtered),
            Some(Demand::ComplainNoOre)
        );
        let some = SmelterFacts {
            ore_outside_list: 3,
            ..SmelterFacts::new()
        };
        assert_eq!(request_smeltable(&some), Some(Demand::StackList));
    }

    #[test]
    fn one_request_carries_a_stack_per_furnace() {
        // `:151` and `:232`.
        assert_eq!(smelt_request_count(0), 0);
        assert_eq!(smelt_request_count(1), 64);
        assert_eq!(smelt_request_count(3), 192);
        assert_eq!(STACKSIZE, 64);
    }

    #[test]
    fn the_ore_breaking_module_has_four_answers() {
        // `EntityAIWorkSmelter.java:78-122`.
        assert_eq!(break_ores(&SmelterFacts::new()), BreakOres::NoRecipe);
        let empty_input = SmelterFacts {
            ore_recipe: Some(OreRecipe { input_count: 0 }),
            ..SmelterFacts::new()
        };
        assert_eq!(break_ores(&empty_input), BreakOres::NoInput);
        let unfulfilled = SmelterFacts {
            ore_recipe: Some(OreRecipe { input_count: 2 }),
            ore_fulfilled: false,
            ..SmelterFacts::new()
        };
        assert_eq!(break_ores(&unfulfilled), BreakOres::Failed);
        let broken = SmelterFacts {
            ore_fulfilled: true,
            ..unfulfilled
        };
        assert_eq!(break_ores(&broken), BreakOres::Broken { input: 2 });
    }

    #[test]
    fn retrieving_bars_needs_a_target_an_arrival_a_furnace_and_a_stack() {
        // `:388-413`.
        let facts = SmelterFacts::new();
        assert_eq!(retrieve_product(&facts, None), Retrieve::NoTarget);

        let walking = SmelterFacts {
            at_walk_target: false,
            ..SmelterFacts::new()
        };
        assert_eq!(
            retrieve_product(&walking, Some(at(1))),
            Retrieve::NotArrived
        );

        let gone = SmelterFacts {
            furnaces: Vec::new(),
            ..SmelterFacts::new()
        };
        assert_eq!(retrieve_product(&gone, Some(at(1))), Retrieve::Gone);

        let empty = SmelterFacts::new(); // one empty furnace at `at(1)`
        assert_eq!(retrieve_product(&empty, Some(at(1))), Retrieve::Empty);

        let ready = SmelterFacts {
            furnaces: vec![Furnace {
                lit: false,
                result: 4,
                ..Furnace::empty(at(1))
            }],
            ..SmelterFacts::new()
        };
        let extracted = retrieve_product(&ready, Some(at(1)));
        assert_eq!(extracted, Retrieve::Extracted);
        assert!(extracted.forgets_target());
        assert_eq!(
            extracted.next(SmeltState::RetrievingEndProductFromFurnace),
            SmeltState::StartWorking
        );
    }

    #[test]
    fn retrieving_used_fuel_needs_fuel_in_the_slot() {
        // `:421-445`.
        let empty = SmelterFacts::new();
        assert_eq!(retrieve_used_fuel(&empty, Some(at(1))), Retrieve::Empty);
        let fuelled = SmelterFacts {
            furnaces: vec![Furnace {
                fuel_present: true,
                ..Furnace::empty(at(1))
            }],
            ..SmelterFacts::new()
        };
        assert_eq!(
            retrieve_used_fuel(&fuelled, Some(at(1))),
            Retrieve::Extracted
        );
        // `NotArrived` is the only answer that keeps the state (`:431`).
        let walking = SmelterFacts {
            at_walk_target: false,
            ..fuelled
        };
        assert_eq!(
            retrieve_used_fuel(&walking, Some(at(1)))
                .next(SmeltState::RetrievingUsedFuelFromFurnace),
            SmeltState::RetrievingUsedFuelFromFurnace
        );
    }

    #[test]
    fn filling_an_empty_furnace_puts_both_the_ore_and_the_fuel_in() {
        // `:480-495` read in sequence: the first transfer changes what the second sees.
        let facts = SmelterFacts {
            inventory_has_smeltable: true,
            inventory_has_fuel: true,
            ..SmelterFacts::new()
        };
        assert_eq!(
            fill_up_furnace(&facts, Some(at(1))),
            Fill::Filled {
                smeltable: true,
                fuel: true
            }
        );
    }

    #[test]
    fn filling_only_completes_what_the_furnace_lacks() {
        // `hasSmeltableInFurnaceAndNoFuel` alone (`:489-490`).
        let facts = SmelterFacts {
            furnaces: vec![Furnace {
                input: 2,
                ..Furnace::empty(at(1))
            }],
            inventory_has_smeltable: true,
            inventory_has_fuel: true,
            ..SmelterFacts::new()
        };
        assert_eq!(
            fill_up_furnace(&facts, Some(at(1))),
            Fill::Filled {
                smeltable: false,
                fuel: true
            }
        );
    }

    #[test]
    fn filling_with_empty_pockets_says_so() {
        // `:480-495` matched neither test.
        let facts = SmelterFacts::new();
        assert_eq!(
            fill_up_furnace(&facts, Some(at(1))),
            Fill::Filled {
                smeltable: false,
                fuel: false
            }
        );
    }

    #[test]
    fn a_fill_needs_furnaces_a_target_and_an_arrival() {
        // `:452-473`.
        let none = SmelterFacts {
            furnaces: Vec::new(),
            ..SmelterFacts::new()
        };
        assert_eq!(fill_up_furnace(&none, Some(at(1))), Fill::NoFurnaces);
        assert_eq!(fill_up_furnace(&SmelterFacts::new(), None), Fill::NoTarget);
        // A position the module no longer holds is not a furnace any more.
        assert_eq!(
            fill_up_furnace(&SmelterFacts::new(), Some(at(9))),
            Fill::NoTarget
        );
        let walking = SmelterFacts {
            at_walk_target: false,
            inventory_has_fuel: true,
            ..SmelterFacts::new()
        };
        assert_eq!(fill_up_furnace(&walking, Some(at(1))), Fill::NotArrived);
    }

    #[test]
    fn the_table_has_nine_targets_with_java_rates_and_no_trap_state() {
        let machine = ai();
        let rules: Vec<&str> = machine
            .transitions()
            .iter()
            .map(|transition| transition.rule)
            .collect();
        assert_eq!(
            rules,
            vec![
                "idleToStartWorking",
                "startWorking",
                "fillUpFurnace",
                "accelerateFurnaces",
                "retrieveSmeltableFromFurnace",
                "retrieveUsedFuel",
                "breakOres",
                "afterDump",
                "getStateAfterPickUp",
            ],
            "AbstractEntityAIUsesFurnace.java:76-82 then EntityAIWorkSmelter.java:69"
        );
        // The rates: 5, 60, 5, 20, 5, 5, 20 (`:77-82`, `EntityAIWorkSmelter.java:69`).
        assert_eq!(machine.transitions()[0].tick_rate, STANDARD_DELAY);
        assert_eq!(machine.transitions()[1].tick_rate, START_WORKING_RATE);
        assert_eq!(machine.transitions()[2].tick_rate, STANDARD_DELAY);
        assert_eq!(machine.transitions()[3].tick_rate, TICKS_SECOND);
        assert_eq!(machine.transitions()[6].tick_rate, TICKS_SECOND);
        assert_eq!(START_WORKING_RATE, 60);
        // `accelerateFurnaces` is the only global pass, and it stops nothing.
        assert_eq!(
            machine.transitions()[3].blocking,
            Some(Blocking::AiBlocking)
        );
        for state in SmeltState::ALL {
            assert!(machine.can_leave(state), "{state} is a trap state");
        }
    }
}
