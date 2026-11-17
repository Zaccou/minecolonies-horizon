//! The crafter: six targets, the recipe it reads, the items it fetches, the hits it
//! gives, and the request it answers.
//!
//! Java reference: `core/entity/ai/workers/crafting/AbstractEntityAICrafting.java`
//! — `registerTargets` (`:160-170`), `idle` (`:180-255`), `hasWorkToDo` (`:261-264`),
//! `decide` (`:271-291`), `getNextCraftingState` (`:298-317`), `getRecipe`
//! (`:324-422`), `checkForItems`/`queryItems` (`:446-504`), `craft` (`:511-572`),
//! `executeCraftingAction` (`:579-634`), `finalizeCraftingTask` (`:636-646`) and
//! `getRequiredProgressForMakingRawMaterial` (`:734-738`). The one concrete trade it
//! stands for here is `EntityAIWorkSawmill` (`crafting/EntityAIWorkSawmill.java`),
//! which adds nothing to the state machine.
//!
//! ## Why this file has its own state enum
//!
//! `crate::ai::state::WorkerState` names **eighteen** of `AIWorkerState`'s 127 states,
//! and `CRAFT`, `QUERY_ITEMS` and `GET_RECIPE` are not among them: the builder and
//! the composter never enter them. `crate::ai/` is another agent's file, so this
//! module brings [`CraftState`] — the six states of *this* table plus the two the
//! base class sends a crafter to (`INVENTORY_FULL` and
//! `GATHERING_REQUIRED_MATERIALS`). The engine underneath is `crate::ai::Machine`,
//! unchanged: the four passes, the tick-rate countdowns and the trap rule are the
//! same code the builder runs on.
//!
//! ## The shape of the trade
//!
//! | # | State | Rule | Rate | Java |
//! | ---: | --- | --- | ---: | --- |
//! | 1 | `IDLE` | `hasWorkToDo` → `START_WORKING` | 20 | `:164` |
//! | 2 | `IDLE` | `idle` (pastimes, `NO_CHANGE`) | 20 | `:165` |
//! | 3 | `START_WORKING` | `decide` | 5 | `:166` |
//! | 4 | `QUERY_ITEMS` | `queryItems` | 5 | `:167` |
//! | 5 | `GET_RECIPE` | `getRecipe` | 5 | `:168` |
//! | 6 | `CRAFT` | `craft` | 10 | `:169` |
//! | — | `INVENTORY_FULL` | base `afterDump` → `IDLE` | — | `AbstractEntityAIBasic.java:1213-1222` |
//! | — | `GATHERING_REQUIRED_MATERIALS` | `getStateAfterPickUp` → `START_WORKING` | — | `:436-439` |
//!
//! **The one number that decides everything.** `craft` only fires when
//! `job.getProgress()` has reached `getRequiredProgressForMakingRawMaterial()`
//! (`:561`), and that is `crate::ai_build::work_time::craft_hits`: 30 hits at skill
//! 0, 0 hits from skill 20 on. `CRAFT` is looked at every 10 ticks (`HIT_DELAY`), so
//! a level-1 crafter spends 300 ticks on one recipe and a level-20 one spends none.
//!
//! **`getRecipe` is a material resolver too.** `:371-421` computes, before a single
//! hit, how many times the recipe can run: `availableOpsCount` is the request's
//! count, narrowed by every input's available amount
//! (`availableCount / inputStorage.getAmount()`), then capped by the request. An
//! input that cannot cover `minRemainingOpsCount` ends the attempt immediately
//! (`:403-409`) — that shortfall is [`Recipe::ShortOfIngredient`], named with the
//! input, what is available and what is needed, instead of a bare `finishRequest(false)`.
//!
//! **What is not here.** Walking (`walkToBuilding`, `walkToTaggedWorkPos`), the item
//! handler transfers, the particle packet and the tool damage: all host calls. Each
//! is a fact (`at_building`, `at_work_position`, `in_building`, `tool_broke`), so the
//! decision is replayable. The three random draws of `idle` (`:208`, `:231`,
//! `MathUtils.RANDOM.nextInt(size)`) are facts too, for the reason
//! `crate::ai::facts` gives about `dumpOneMoreSlot`.

use crate::ai::{Action, Machine, Pos, Transition};

use super::work_time;

/// Where this file's rules come from, as one string a log line can print.
pub const SOURCE: &str = "AbstractEntityAICrafting.java:160-170,:180-255,:261-264,:271-291,:298-317,\
:324-422,:446-504,:511-572,:579-646,:734-738";

/// The states of the crafting table.
///
/// Java reference: `AIWorkerState`'s `CRAFT` (`:73`), `QUERY_ITEMS` (`:69`),
/// `GET_RECIPE` (`:77`), plus the two general states the base class sends any worker
/// to — `INVENTORY_FULL` (`:23`) and `GATHERING_REQUIRED_MATERIALS` (`:310`). The
/// line numbers are the Java enum's, not this module's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: the six targets of this table plus the two exits the base class adds
pub enum CraftState {
    /// `IDLE` — look for work, or pass the time (`:164-165`).
    Idle,
    /// `START_WORKING` — the general decide state (`:166`).
    StartWorking,
    /// `QUERY_ITEMS` — take the recipe's inputs in hand (`:167`).
    QueryItems,
    /// `GET_RECIPE` — read the next task's recipe (`:168`).
    GetRecipe,
    /// `CRAFT` — one hit on the hut's work surface (`:169`).
    Craft,
    /// `INVENTORY_FULL` — the base table dumps, then comes back (`AbstractEntityAIBasic.java:225-226`).
    InventoryFull,
    /// `GATHERING_REQUIRED_MATERIALS` — walk to the hut's chest for one input (`:494`).
    GatheringRequiredMaterials,
}

impl CraftState {
    /// The seven, in the order Java declares them.
    pub const ALL: [CraftState; 7] = [
        CraftState::Idle,
        CraftState::StartWorking,
        CraftState::QueryItems,
        CraftState::GetRecipe,
        CraftState::Craft,
        CraftState::InventoryFull,
        CraftState::GatheringRequiredMaterials,
    ];

    /// Java's identifier, for a log line or a test failure.
    pub fn java_name(self) -> &'static str {
        match self {
            CraftState::Idle => "IDLE",
            CraftState::StartWorking => "START_WORKING",
            CraftState::QueryItems => "QUERY_ITEMS",
            CraftState::GetRecipe => "GET_RECIPE",
            CraftState::Craft => "CRAFT",
            CraftState::InventoryFull => "INVENTORY_FULL",
            CraftState::GatheringRequiredMaterials => "GATHERING_REQUIRED_MATERIALS",
        }
    }
}

/// One input of the recipe being crafted, and where it is.
///
/// Java reference: `currentRecipeStorage.getCleanedInput()` walked at `:377-417`
/// (the request) and `:468-501` (the pickup), `getCraftingToolsAndSecondaryOutputs`
/// (`:383`, `:475`), `getCraftingRemainingItem` (`:380`, `:473`),
/// `InventoryUtils.getCountFromBuilding` (`:398`),
/// `getItemCountInItemHandler` (`:399`, `:472`) and `getExtendedCount`
/// (`:401`, `:472`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Input {
    /// `inputStorage.getAmount()`: how many one run of the recipe eats.
    pub amount: u32,
    /// The input is a tool or a container (`:381-396`): it is **not** multiplied by
    /// the number of runs, because a tool is used up once and a bucket comes back.
    pub tool_or_container: bool,
    /// `InventoryUtils.getCountFromBuilding(building, ...)` (`:398`): what the hut
    /// holds.
    pub in_building: u32,
    /// `getItemCountInItemHandler(worker.getInventoryCitizen(), ...)`.
    pub in_inventory: u32,
    /// `getExtendedCount(...)` (`:401`, `:430-433`): counts the subclass adds, e.g.
    /// what already sits in a furnace.
    pub extended: u32,
}

impl Input {
    /// An ordinary ingredient.
    pub fn item(amount: u32, in_building: u32, in_inventory: u32) -> Input {
        Input {
            amount,
            tool_or_container: false,
            in_building,
            in_inventory,
            extended: 0,
        }
    }

    /// A tool or a container, used once per recipe whatever the number of runs.
    pub fn tool(amount: u32, in_inventory: u32) -> Input {
        Input {
            amount,
            tool_or_container: true,
            in_building: 0,
            in_inventory,
            extended: 0,
        }
    }

    /// The same input, with what a subclass counts elsewhere.
    pub fn extended(mut self, extended: u32) -> Input {
        self.extended = extended;
        self
    }

    /// `in_building + in_inventory + extended` (`:398-401`, `:472`).
    pub fn available(self) -> u32 {
        self.in_building
            .saturating_add(self.in_inventory)
            .saturating_add(self.extended)
    }
}

/// Everything the crafting AI reads, as observations.
///
/// Java reference: `AbstractEntityAICrafting`'s own fields (`currentRequest`,
/// `currentRecipeStorage`, `dumped`, `idlePos`, `job.getProgress()`,
/// `job.getMaxCraftingCount()`, `job.getCraftCounter()`) and the base class's
/// (`job.getTaskQueue()`, `job.getCurrentTask()`, `job.getActionsDone()`,
/// `building.isInBuilding`, `worker.getNavigation().isDone()`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CraftFacts {
    /// `citizenData.getWorkBuilding() != null`.
    pub building_exists: bool,
    /// `walkToBuilding()` (`:279`): the citizen reached its own hut.
    pub at_building: bool,
    /// `building.isInBuilding(worker.blockPosition())` (`:187`).
    pub in_building: bool,
    /// `worker.getNavigation().isDone()` (`:182`).
    pub navigation_done: bool,
    /// `idlePos` (`:182`): the pastime position picked last time, still to be walked
    /// to.
    pub idle_pos: Option<Pos>,
    /// `walkToSafePos(idlePos)` (`:182`): the stored position is still reachable.
    pub idle_pos_reachable: bool,
    /// `worker.level.isRaining()` (`:212`, `:234`): rain sends the citizen inside.
    pub raining: bool,
    /// `building.getLocationsFromTag(TAG_SITTING)` (`:197`, `:210`).
    pub sitting: Vec<Pos>,
    /// `TAG_SIT_IN` (`:198`, `:211`).
    pub sit_in: Vec<Pos>,
    /// `TAG_SIT_OUT` (`:199`, `:222`).
    pub sit_out: Vec<Pos>,
    /// `TAG_STAND_IN` (`:233`).
    pub stand_in: Vec<Pos>,
    /// `TAG_STAND_OUT` (`:244`).
    pub stand_out: Vec<Pos>,
    /// **Not a Java call.** `MathUtils.RANDOM.nextBoolean()` (`:208`): the draw that
    /// decides whether a sitting position is looked for at all.
    pub coin_a: bool,
    /// **Not a Java call.** The second `nextBoolean` (`:231`), for the standing
    /// positions.
    pub coin_b: bool,
    /// **Not a Java call.** `MathUtils.RANDOM.nextInt(size)` (`:216`, `:225`,
    /// `:238`, `:247`): the index drawn inside the chosen list. The port takes it
    /// modulo the list's length, so any value is safe.
    pub pick: usize,
    /// `job.getActionsDone()` (`:284`).
    pub actions_done: u32,
    /// `job.getTaskQueue().size()` (`:263`).
    pub task_queue: usize,
    /// `job.getCurrentTask() != null` (`:263`, `:300`, `:326`, `:513`).
    pub current_task: bool,
    /// `currentRecipeStorage != null` (`:305`, `:311`, `:448`, `:513`).
    pub has_recipe: bool,
    /// `currentRequest != null` (`:311`, `:518`, `:638`).
    pub has_request: bool,
    /// `dumped` (`:305`, `:348`): the worker already emptied its pockets once for
    /// this recipe.
    pub dumped: bool,
    /// `hasTooManyExternalItemsInInv(currentRecipeStorage, inv)` (`:305`, `:348` →
    /// `WorkerUtil.java:360-374`: more than three non-recipe stacks).
    pub too_many_external: bool,
    /// `building.getCraftingModuleForRecipe(recipeID) != null` (`:333-339`).
    pub module_found: bool,
    /// `module.getFirstFulfillableRecipe(...) != null` (`:340-346`).
    pub recipe_found: bool,
    /// `checkForToolOrWeapon(recipe.getRequiredTool())` (`:355-364`): true when the
    /// tool is missing.
    pub tool_missing: bool,
    /// `walkToTaggedWorkPos()` (`:523-526`).
    pub at_work_position: bool,
    /// `currentRequest.getState()` is `CANCELLED` or `FAILED` (`:553-559`).
    pub request_cancelled: bool,
    /// `fullfillRecipeAndCopy(...) == null` (`:581-589`).
    pub recipe_failed: bool,
    /// The tool broke during the hit and the hand is empty (`:621-631`).
    pub tool_broke: bool,
    /// `inventoryNeedsDump()` (`:638`, the base rule of `AbstractEntityAIBasic.java:
    /// 406-417`).
    pub inventory_needs_dump: bool,
    /// `job.getProgress()` (`:528`, `:561`).
    pub progress: u32,
    /// `job.getMaxCraftingCount()` (`:486`, `:609`).
    pub max_crafting_count: u32,
    /// `job.getCraftCounter()` (`:490`, `:609`).
    pub craft_counter: u32,
    /// `currentRecipeStorage.getPrimaryOutput().getCount()` (`:371`, `:465`).
    pub output_count: u32,
    /// `InventoryUtils.getItemCountInItemHandler(inv, matches(primaryOutput))`
    /// (`:367-368`).
    pub carried_output: u32,
    /// `getExtendedCount(primaryOutput)` (`:369`, `:464`).
    pub in_progress_output: u32,
    /// `currentRequest.getRequest().getCount()` (`:376`).
    pub request_count: u32,
    /// `currentRequest.getRequest().getMinCount()` (`:375`).
    pub request_min_count: u32,
    /// The skill `getCraftSpeedSkill()` names (`:735`), read from the citizen.
    pub craft_speed_level: i32,
    /// `currentRecipeStorage.getCleanedInput()` (`:377`, `:468`).
    pub inputs: Vec<Input>,
}

impl Default for CraftFacts {
    /// A hired crafter standing in its hut, with no task, no recipe and no input.
    fn default() -> CraftFacts {
        CraftFacts {
            building_exists: true,
            at_building: true,
            in_building: true,
            navigation_done: true,
            idle_pos: None,
            idle_pos_reachable: true,
            raining: false,
            sitting: Vec::new(),
            sit_in: Vec::new(),
            sit_out: Vec::new(),
            stand_in: Vec::new(),
            stand_out: Vec::new(),
            coin_a: false,
            coin_b: false,
            pick: 0,
            actions_done: 0,
            task_queue: 0,
            current_task: false,
            has_recipe: false,
            has_request: false,
            dumped: false,
            too_many_external: false,
            module_found: true,
            recipe_found: true,
            tool_missing: false,
            at_work_position: true,
            request_cancelled: false,
            recipe_failed: false,
            tool_broke: false,
            inventory_needs_dump: false,
            progress: 0,
            max_crafting_count: 0,
            craft_counter: 0,
            output_count: 1,
            carried_output: 0,
            in_progress_output: 0,
            request_count: 1,
            request_min_count: 1,
            craft_speed_level: 0,
            inputs: Vec::new(),
        }
    }
}

/// `hasWorkToDo` (`:261-264`): a queue that is not empty **and** a task it can read.
/// Both halves matter: Java calls `getCurrentTask()` on the queue, and an empty queue
/// throws.
pub fn has_work_to_do(facts: &CraftFacts) -> bool {
    facts.task_queue > 0 && facts.current_task
}

/// What `idle` did with the tick (`:180-255`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the five exits of `idle`
pub enum IdleStep {
    /// `NO_CHANGE` with no position picked: a walk in flight, a delay just set, or
    /// nothing to sit on.
    Stay,
    /// `!building.isInBuilding(...)` → `walkToBuilding()` (`:187-190`).
    WalkToBuilding,
    /// `SittingEntity.sitDown(idlePos, worker, ...)` (`:201`).
    SitDown(Pos),
    /// A standing position was picked (`:236-249`); the walk happens next tick.
    StandAt(Pos),
    /// `EntityNavigationUtils.walkToRandomPosWithin(worker, 10, ...)` (`:253`).
    Wander,
}

/// The step, and the `idlePos` the next tick will find.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdleDecision {
    /// What happens this tick.
    pub step: IdleStep,
    /// `idlePos` after the call. `None` clears it, which is what Java does after a
    /// sitting position is used (`:202`) or rejected (`:205`).
    pub idle_pos: Option<Pos>,
}

/// The index Java draws, made safe: `MathUtils.RANDOM.nextInt(size)` (`:216`) is a
/// fact here, and a fact that is out of range must not panic.
fn pick(list: &[Pos], index: usize) -> Option<Pos> {
    if list.is_empty() {
        return None;
    }
    list.get(index % list.len()).copied()
}

/// `idle` (`:180-255`): what a crafter does when there is nothing to craft.
///
/// The order of the checks is Java's, and each one exits:
///
/// 1. a walk that is not finished, or a stored position that became unreachable
///    (`:182-185`) → stay;
/// 2. outside the hut (`:187-191`) → walk in;
/// 3. `setDelay(TICKS_20 * 20)`, 400 ticks, before anything is picked (`:193`);
/// 4. a stored position that is a sitting tag → sit down (`:195-206`);
/// 5. a coin flip for a sitting position, inside when it rains (`:208-229`);
/// 6. a coin flip for a standing position, same rain rule (`:231-251`);
/// 7. otherwise wander within ten blocks of the hut (`:253`).
pub fn idle(facts: &CraftFacts) -> IdleDecision {
    let unreachable = facts.idle_pos.is_some() && !facts.idle_pos_reachable;
    if unreachable || !facts.navigation_done {
        return IdleDecision {
            step: IdleStep::Stay,
            idle_pos: facts.idle_pos, // `:182-185`
        };
    }
    if !facts.in_building {
        return IdleDecision {
            step: IdleStep::WalkToBuilding, // `:187-190`
            idle_pos: facts.idle_pos,
        };
    }
    if let Some(at) = facts.idle_pos {
        // `:195-206` — a stored position that is one of the three sitting tags.
        let sits = facts.sitting.contains(&at)
            || facts.sit_in.contains(&at)
            || facts.sit_out.contains(&at);
        if sits {
            return IdleDecision {
                step: IdleStep::SitDown(at), // `:201`
                idle_pos: None,              // `:202`
            };
        }
        return IdleDecision {
            step: IdleStep::Stay,
            idle_pos: None, // `:205`
        };
    }
    if facts.coin_a {
        // `:210-211` then `:222`: inside when it rains, inside and outside when it
        // does not.
        let mut sits = facts.sitting.clone();
        sits.extend_from_slice(&facts.sit_in);
        if !facts.raining {
            sits.extend_from_slice(&facts.sit_out);
        }
        if let Some(at) = pick(&sits, facts.pick) {
            return IdleDecision {
                step: IdleStep::Stay, // `:217`, `:226`: the walk starts next tick
                idle_pos: Some(at),
            };
        }
    }
    if facts.coin_b {
        // `:233-250`, the same rain rule for the standing tags.
        let mut stands = facts.stand_in.clone();
        if !facts.raining {
            stands.extend_from_slice(&facts.stand_out);
        }
        if let Some(at) = pick(&stands, facts.pick) {
            return IdleDecision {
                step: IdleStep::StandAt(at),
                idle_pos: Some(at),
            };
        }
    }
    IdleDecision {
        step: IdleStep::Wander, // `:253`
        idle_pos: None,
    }
}

/// `decide` (`:271-291`), the four exits named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one variant per `return` of `decide`
pub enum Decide {
    /// No work at all (`:273-276`) → `IDLE`.
    Idle,
    /// Still walking to the hut (`:279-282`) → `START_WORKING`, i.e. stay.
    Walking,
    /// `job.getActionsDone() >= getActionsDoneUntilDumping()` (`:284-288`): the base
    /// table's dump is pending, so spend the tick waiting.
    WaitingForDump,
    /// `getNextCraftingState()` (`:290`).
    Next(CraftState),
}

/// `getNextCraftingState` (`:298-317`): which of the three work states comes next.
pub fn next_crafting_state(facts: &CraftFacts) -> CraftState {
    if !facts.current_task {
        return CraftState::Idle; // `:300-303`
    }
    if facts.has_recipe && !facts.dumped && facts.too_many_external {
        return CraftState::InventoryFull; // `:305-309`
    }
    if facts.has_request && facts.has_recipe {
        return CraftState::QueryItems; // `:311-314`
    }
    CraftState::GetRecipe // `:316`
}

/// `decide` (`:271-291`).
pub fn decide(facts: &CraftFacts) -> Decide {
    if !has_work_to_do(facts) {
        return Decide::Idle; // `:273-276`
    }
    if !facts.at_building {
        return Decide::Walking; // `:279-282`
    }
    // `getActionsDoneUntilDumping()` is 1 for every crafter
    // (`AbstractEntityAICrafting.java:724-727`), so this is "one craft since the last
    // dump".
    if facts.actions_done >= 1 {
        return Decide::WaitingForDump; // `:284-288`
    }
    Decide::Next(next_crafting_state(facts)) // `:290`
}

/// What `getRecipe` found (`:324-422`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one variant per `return` of `getRecipe`
pub enum Recipe {
    /// `job.getCurrentTask() == null` (`:328-331`).
    NoTask,
    /// `building.getCraftingModuleForRecipe(recipeID) == null` (`:333-339`): Java
    /// finishes the request as failed.
    NoModule,
    /// `getFirstFulfillableRecipe(...) == null` (`:341-346`): nothing in the hut can
    /// make this.
    NoRecipe,
    /// `hasTooManyExternalItemsInInv` (`:348-353`): dump first, then read the recipe
    /// again.
    Dump,
    /// `checkForToolOrWeapon` said the tool is missing (`:355-364`).
    ToolMissing,
    /// An input cannot cover `minRemainingOpsCount` (`:403-409`): the request is
    /// finished as failed, with the input named.
    ShortOfIngredient {
        /// Index into [`CraftFacts::inputs`].
        input: usize,
        /// `availableCount`, what the hut and the citizen have.
        available: u32,
        /// `inputStorage.getAmount() * minRemainingOpsCount`.
        remaining: u32,
    },
    /// `getPrimaryOutput().getCount()` is zero, so `:372` would divide by zero. Java
    /// throws `ArithmeticException` here; the port names it.
    DegenerateOutput,
    /// The recipe can run (`:419-421`): the two counters the `CRAFT` state reads.
    Ready {
        /// `job.setMaxCraftingCount(...)`.
        max_crafting_count: u32,
        /// `job.setCraftCounter(doneOpsCount)`.
        craft_counter: u32,
    },
}

/// The number of runs the recipe can do, or the input that stops it (`:366-421`).
///
/// This is the crafter's material resolution: it answers *how many times*, from the
/// request's count and every input's available amount, before a single item is
/// moved.
pub fn get_recipe(facts: &CraftFacts) -> Recipe {
    if !facts.current_task {
        return Recipe::NoTask; // `:328-331`
    }
    if !facts.module_found {
        return Recipe::NoModule; // `:333-339`
    }
    if !facts.recipe_found {
        return Recipe::NoRecipe; // `:341-346`
    }
    if !facts.dumped && facts.too_many_external {
        return Recipe::Dump; // `:348-353`
    }
    if facts.tool_missing {
        return Recipe::ToolMissing; // `:355-364`
    }
    if facts.output_count == 0 {
        return Recipe::DegenerateOutput; // `:371-372` would divide by zero
    }
    let per_iteration = facts.output_count; // `:371`
    let done_ops = facts.carried_output / per_iteration; // `:372`
    let progress_ops = facts.in_progress_output / per_iteration; // `:373`
    // `:375` — `minCount - doneOpsCount - progressOpsCount`, which can go negative
    // when the citizen already carries more than the minimum asked for. Java then
    // multiplies by it and compares a positive available count against a negative
    // requirement, i.e. no constraint at all; clamping at zero is the same decision.
    let min_remaining = facts
        .request_min_count
        .saturating_sub(done_ops)
        .saturating_sub(progress_ops);
    let mut available_ops = facts.request_count; // `:376`
    for (index, input) in facts.inputs.iter().enumerate() {
        // `:383-396` — a tool or a container is needed once, not once per run.
        let remaining = if input.tool_or_container {
            input.amount
        } else {
            input.amount.saturating_mul(min_remaining)
        };
        let available = input.available(); // `:398-401`
        if available < remaining {
            return Recipe::ShortOfIngredient {
                input: index, // `:403-409`
                available,
                remaining,
            };
        }
        if !input.tool_or_container {
            // `:414-415` — `availableCount / inputStorage.getAmount()`, capped by the
            // request's own count.
            let iterations = available / input.amount.max(1);
            available_ops = available_ops.min(iterations);
        }
    }
    // `:419-420` — `Math.min(availableOpsCount + doneOpsCount, request.getCount())`.
    Recipe::Ready {
        max_crafting_count: available_ops.saturating_add(done_ops).min(facts.request_count),
        craft_counter: done_ops,
    }
}

/// What `checkForItems` found (`:446-504`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one variant per `return` of `checkForItems`
pub enum Items {
    /// `currentRecipeStorage == null` (`:448-451`).
    NoRecipe,
    /// An input is short and the hut has it (`:492-496`): walk to the chest.
    Gather {
        /// Index into [`CraftFacts::inputs`].
        input: usize,
        /// `remaining`, what the chest should give.
        remaining: u32,
    },
    /// An input is short and the hut does not have it (`:497-500`): forget the
    /// recipe and read the next one.
    ReRead,
    /// Everything is in hand (`:503`).
    Craft,
}

/// `checkForItems` (`:462-504`), the per-input test before every hit.
pub fn check_for_items(facts: &CraftFacts) -> Items {
    if !facts.has_recipe {
        return Items::NoRecipe; // `:448-451`
    }
    // `:465-466` — `Math.max(countPerIteration, 1)`: Java guards this one itself.
    let per_iteration = facts.output_count.max(1);
    let progress_ops = facts.in_progress_output / per_iteration; // `:466`
    for (index, input) in facts.inputs.iter().enumerate() {
        // `:475-487` — a tool or container is needed once; anything else once per
        // planned run, and at least once.
        let remaining = if input.tool_or_container {
            input.amount
        } else {
            input
                .amount
                .saturating_mul(facts.max_crafting_count.max(1))
        };
        // `:472` — the count in the inventory, plus what the subclass counts.
        let in_inventory = input.in_inventory.saturating_add(input.extended);
        // `:489-490` — nothing at all, or not enough for the runs already happened.
        let nothing = in_inventory.saturating_add(facts.in_progress_output) == 0;
        let short = in_inventory
            .saturating_add(
                facts
                    .craft_counter
                    .saturating_add(progress_ops)
                    .saturating_mul(input.amount),
            )
            < remaining;
        if nothing || short {
            if input.in_building > 0 {
                return Items::Gather {
                    input: index,
                    remaining, // `:494`
                };
            }
            return Items::ReRead; // `:497-500`
        }
    }
    Items::Craft // `:503`
}

/// What one `CRAFT` tick decided (`:511-572`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one variant per `return` of `craft`
pub enum Craft {
    /// `currentRecipeStorage == null || job.getCurrentTask() == null` (`:513-516`).
    NoWork,
    /// A task but no request (`:518-521`) → `GET_RECIPE`.
    NeedRecipe,
    /// `!walkToTaggedWorkPos()` (`:523-526`): stay.
    Walking,
    /// The request was cancelled or failed while the citizen was hitting
    /// (`:553-559`).
    Cancelled,
    /// Still hitting: `progress < getRequiredProgressForMakingRawMaterial()`
    /// (`:561`, `:571`).
    Hitting,
    /// The progress is reached and the inputs are still there (`:563-568`) → run the
    /// recipe.
    Execute,
    /// The progress is reached and an input is gone (`:563-568`) → `START_WORKING`.
    OutOfItems,
    /// The craft-speed skill level is negative, so the progress formula has no answer
    /// (`:734-738` → [`work_time::craft_hits`]). Java would divide by a nonsense
    /// number; the port names the broken fact and stays.
    BadSkill,
}

/// `craft` (`:511-572`).
///
/// Note what is **not** decided here: `:528` increments `job.getProgress()` on every
/// call that reaches the hit, and that write is the caller's (it is a fact,
/// [`CraftFacts::progress`]). The port therefore reads the progress as it stands at
/// the top of the tick; the comparison against the required number is the decision.
pub fn craft(facts: &CraftFacts) -> Craft {
    if !facts.has_recipe || !facts.current_task {
        return Craft::NoWork; // `:513-516`
    }
    if !facts.has_request {
        return Craft::NeedRecipe; // `:518-521`
    }
    if !facts.at_work_position {
        return Craft::Walking; // `:523-526`
    }
    if facts.request_cancelled {
        return Craft::Cancelled; // `:553-559`
    }
    let required = match work_time::craft_hits(facts.craft_speed_level) {
        Ok(hits) => hits,
        Err(_) => return Craft::BadSkill,
    };
    if facts.progress < required {
        return Craft::Hitting; // `:571`
    }
    // `:562-568` — `checkForItems(currentRecipeStorage) == CRAFT` decides.
    match check_for_items(facts) {
        Items::Craft => Craft::Execute,
        _ => Craft::OutOfItems,
    }
}

/// What `executeCraftingAction` did (`:579-634`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one variant per `return` of `executeCraftingAction`
pub enum Execution {
    /// `fullfillRecipeAndCopy(...) == null` (`:581-589`): the recipe could not run.
    RecipeFailed,
    /// `job.getCraftCounter() >= job.getMaxCraftingCount()` (`:609-619`) → the task is
    /// finalized.
    Finalized,
    /// The tool broke and the hand is empty (`:621-631`).
    ToolBroke,
    /// One more run to make: `progress` is reset and the recipe is re-read
    /// (`:634-637`).
    Again,
}

/// `executeCraftingAction` (`:579-634`).
///
/// `facts.craft_counter` is the counter **before** this run: Java increments it at
/// `:603` and compares at `:609`.
pub fn execute(facts: &CraftFacts) -> Execution {
    if facts.recipe_failed {
        return Execution::RecipeFailed; // `:581-589`
    }
    let after = facts.craft_counter.saturating_add(1); // `:603`
    if after >= facts.max_crafting_count {
        return Execution::Finalized; // `:609-619`
    }
    if facts.tool_broke {
        return Execution::ToolBroke; // `:621-631`
    }
    Execution::Again // `:634-637`
}

/// `finalizeCraftingTask`'s experience rule (`:636-646`).
///
/// Java resets the three job counters **first** (`:648-654`) and then tests them, so
/// only two conditions are ever false: `inventoryNeedsDump()` and `currentRequest !=
/// null`. The port says so instead of repeating a test that cannot fail.
pub fn completion_experience(
    request_count: u32,
    inventory_needs_dump: bool,
    has_request: bool,
) -> Option<f64> {
    if inventory_needs_dump && has_request {
        Some(f64::from(request_count) / 2.0) // `:642`
    } else {
        None
    }
}

// ------------------------------------------------------------------- actions

/// `:164` and `:165`: the two targets of `IDLE`. The first one's condition is
/// `hasWorkToDo`, so the second only runs when there is nothing to craft.
fn idle_action(facts: &mut CraftFacts, _: CraftState) -> Option<CraftState> {
    let decision = idle(facts);
    facts.idle_pos = decision.idle_pos;
    // `NO_CHANGE` is `null` (`:68`), and a null supplier does not stop the tick
    // (`BasicStateMachine.java:199-236`).
    None
}

fn idle_has_work(facts: &CraftFacts, _: CraftState) -> bool {
    has_work_to_do(facts)
}

/// `:166` — `decide`.
fn decide_action(facts: &mut CraftFacts, state: CraftState) -> Option<CraftState> {
    match decide(facts) {
        Decide::Idle => Some(CraftState::Idle),
        // `:281` returns `START_WORKING` and `:287` returns `getState()`: both are
        // this same state, so the tick ends where it started.
        Decide::Walking | Decide::WaitingForDump => Some(state),
        Decide::Next(next) => Some(next),
    }
}

/// `:168` — `getRecipe`, whose seven exits each land somewhere different.
fn recipe_action(facts: &mut CraftFacts, _: CraftState) -> Option<CraftState> {
    match get_recipe(facts) {
        Recipe::Ready {
            max_crafting_count,
            craft_counter,
        } => {
            facts.max_crafting_count = max_crafting_count;
            facts.craft_counter = craft_counter;
            facts.has_recipe = true;
            Some(CraftState::QueryItems) // `:421`
        }
        Recipe::Dump => {
            facts.dumped = true; // `:350`
            facts.has_recipe = false; // `:351`
            Some(CraftState::InventoryFull) // `:352`
        }
        // `:330`, `:338`, `:345`, `:362`, `:408` all answer `START_WORKING`.
        Recipe::NoTask
        | Recipe::NoModule
        | Recipe::NoRecipe
        | Recipe::ToolMissing
        | Recipe::ShortOfIngredient { .. }
        | Recipe::DegenerateOutput => Some(CraftState::StartWorking),
    }
}

/// `:167` — `queryItems`.
fn query_action(facts: &mut CraftFacts, _: CraftState) -> Option<CraftState> {
    match check_for_items(facts) {
        Items::NoRecipe => Some(CraftState::StartWorking), // `:450`
        Items::Gather { .. } => Some(CraftState::GatheringRequiredMaterials), // `:495`
        Items::ReRead => {
            facts.has_recipe = false; // `:497-498`
            facts.has_request = false;
            Some(CraftState::GetRecipe) // `:499`
        }
        Items::Craft => Some(CraftState::Craft), // `:503`
    }
}

/// `:169` — `craft`, then `executeCraftingAction` when the progress is reached.
fn craft_action(facts: &mut CraftFacts, state: CraftState) -> Option<CraftState> {
    match craft(facts) {
        Craft::NoWork | Craft::Cancelled | Craft::OutOfItems => Some(CraftState::StartWorking),
        Craft::NeedRecipe => Some(CraftState::GetRecipe), // `:520`
        Craft::Walking | Craft::Hitting | Craft::BadSkill => Some(state), // `:525`, `:571`
        Craft::Execute => match execute(facts) {
            Execution::RecipeFailed => Some(CraftState::StartWorking), // `:588`
            Execution::Finalized => Some(CraftState::InventoryFull),   // `:646`
            Execution::ToolBroke => Some(CraftState::StartWorking),    // `:628`
            Execution::Again => {
                facts.progress = 0; // `:635`
                Some(CraftState::GetRecipe) // `:636`
            }
        },
    }
}

/// Register the six targets of `AbstractEntityAICrafting.java:160-170`, plus the two
/// exits the base class adds.
pub fn register(machine: &mut Machine<CraftState, CraftFacts>) {
    // :164 — `new AITarget(IDLE, this::hasWorkToDo, () -> START_WORKING, TICKS_SECOND)`.
    machine.add(Transition::target_when(
        CraftState::Idle,
        "hasWorkToDo",
        "AbstractEntityAICrafting.java:164",
        20,
        idle_has_work,
        Action::Fixed(Some(CraftState::StartWorking)),
    ));
    // :165 — `new AITarget(IDLE, this::idle, TICKS_SECOND)`.
    machine.add(Transition::target(
        CraftState::Idle,
        "idle",
        "AbstractEntityAICrafting.java:165",
        20,
        Action::Run(idle_action),
    ));
    // :166 — `new AITarget(START_WORKING, this::decide, STANDARD_DELAY)`.
    machine.add(Transition::target(
        CraftState::StartWorking,
        "decide",
        "AbstractEntityAICrafting.java:166",
        5,
        Action::Run(decide_action),
    ));
    // :167 — `new AITarget(QUERY_ITEMS, this::queryItems, STANDARD_DELAY)`.
    machine.add(Transition::target(
        CraftState::QueryItems,
        "queryItems",
        "AbstractEntityAICrafting.java:167",
        5,
        Action::Run(query_action),
    ));
    // :168 — `new AITarget(GET_RECIPE, this::getRecipe, STANDARD_DELAY)`.
    machine.add(Transition::target(
        CraftState::GetRecipe,
        "getRecipe",
        "AbstractEntityAICrafting.java:168",
        5,
        Action::Run(recipe_action),
    ));
    // :169 — `new AITarget(CRAFT, this::craft, HIT_DELAY)`.
    machine.add(Transition::target(
        CraftState::Craft,
        "craft",
        "AbstractEntityAICrafting.java:169",
        work_time::CRAFT_HIT_DELAY,
        Action::Run(craft_action),
    ));
    // The base class's `afterDump` (`AbstractEntityAIBasic.java:1213-1222`): a
    // crafter is never paused in this port's facts, so the base rule is `IDLE`.
    machine.add(Transition::goto(
        CraftState::InventoryFull,
        CraftState::Idle,
        "afterDump",
        "AbstractEntityAIBasic.java:1213-1222",
        1,
    ));
    // `getStateAfterPickUp()` (`AbstractEntityAICrafting.java:436-439`).
    machine.add(Transition::goto(
        CraftState::GatheringRequiredMaterials,
        CraftState::StartWorking,
        "getStateAfterPickUp",
        "AbstractEntityAICrafting.java:436-439",
        1,
    ));
}

/// A machine with this table on it, starting in `IDLE`.
pub fn ai() -> Machine<CraftState, CraftFacts> {
    let mut machine = Machine::new(CraftState::Idle, crate::ai::ENTITY_AI_TICKRATE);
    register(&mut machine);
    machine
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One task queued and readable, with everything else at its default: the
    /// starting point of most tests below.
    fn busy() -> CraftFacts {
        CraftFacts {
            task_queue: 1,
            current_task: true,
            ..CraftFacts::default()
        }
    }

    /// `:261-264`: both halves of the test matter.
    #[test]
    fn work_needs_a_queue_and_a_readable_task() {
        assert!(!has_work_to_do(&CraftFacts::default()));
        assert!(!has_work_to_do(&CraftFacts {
            task_queue: 1,
            current_task: false,
            ..CraftFacts::default()
        }));
        assert!(!has_work_to_do(&CraftFacts {
            task_queue: 0,
            current_task: true,
            ..CraftFacts::default()
        }));
        assert!(has_work_to_do(&busy()));
    }

    /// `:273-276` and `:279-282`: no work goes to `IDLE`, a walk in flight stays.
    #[test]
    fn decide_starts_with_work_and_building() {
        assert_eq!(decide(&CraftFacts::default()), Decide::Idle);
        assert_eq!(
            decide(&CraftFacts {
                at_building: false,
                ..busy()
            }),
            Decide::Walking
        );
    }

    /// `:284-288`: `getActionsDoneUntilDumping()` is 1, so one craft sends the tick
    /// back to wait for the base table's dump.
    #[test]
    fn one_action_since_the_last_dump_waits() {
        assert_eq!(
            decide(&CraftFacts {
                actions_done: 1,
                ..busy()
            }),
            Decide::WaitingForDump
        );
        assert_eq!(
            decide(&CraftFacts {
                actions_done: 0,
                ..busy()
            }),
            Decide::Next(CraftState::GetRecipe)
        );
        // The threshold really is one: `AbstractEntityAICrafting.java:724-727`.
        assert_eq!(CraftFacts::default().actions_done, 0);
    }

    /// `:298-317`: the four exits, in order. A recipe plus a request asks the items;
    /// a recipe alone reads a new one; too many foreign stacks dump first.
    #[test]
    fn the_next_crafting_state_follows_the_four_exits() {
        assert_eq!(
            next_crafting_state(&CraftFacts::default()),
            CraftState::Idle
        );
        assert_eq!(next_crafting_state(&busy()), CraftState::GetRecipe);
        let with_recipe = CraftFacts {
            has_recipe: true,
            has_request: true,
            ..busy()
        };
        assert_eq!(next_crafting_state(&with_recipe), CraftState::QueryItems);
        let too_many = CraftFacts {
            too_many_external: true,
            ..with_recipe.clone()
        };
        assert_eq!(next_crafting_state(&too_many), CraftState::InventoryFull);
        // Unless the citizen already dumped once for this recipe (`:305`).
        let already = CraftFacts {
            dumped: true,
            ..too_many
        };
        assert_eq!(next_crafting_state(&already), CraftState::QueryItems);
    }

    /// `:324-346`: no task, no module, no recipe — three different answers, each one a
    /// `finishRequest(false)` in Java.
    #[test]
    fn a_recipe_needs_a_task_a_module_and_a_fulfillable_entry() {
        assert_eq!(get_recipe(&CraftFacts::default()), Recipe::NoTask);
        assert_eq!(
            get_recipe(&CraftFacts {
                module_found: false,
                ..busy()
            }),
            Recipe::NoModule
        );
        assert_eq!(
            get_recipe(&CraftFacts {
                recipe_found: false,
                ..busy()
            }),
            Recipe::NoRecipe
        );
    }

    /// `:348-353`: too many foreign stacks dump **once**, and only once — `dumped`
    /// is what stops the second visit.
    #[test]
    fn too_many_foreign_stacks_dump_once() {
        let facts = CraftFacts {
            too_many_external: true,
            ..busy()
        };
        assert_eq!(get_recipe(&facts), Recipe::Dump);
        let after = CraftFacts {
            dumped: true,
            ..facts
        };
        assert_ne!(get_recipe(&after), Recipe::Dump);
    }

    /// `:355-364`: a missing tool finishes the request as failed.
    #[test]
    fn a_missing_tool_stops_the_recipe() {
        assert_eq!(
            get_recipe(&CraftFacts {
                tool_missing: true,
                ..busy()
            }),
            Recipe::ToolMissing
        );
    }

    /// `:403-409`: an input that cannot cover the minimum number of runs stops the
    /// attempt, and the port says which input and by how much.
    #[test]
    fn an_input_short_of_the_minimum_run_count_is_named() {
        let facts = CraftFacts {
            request_min_count: 2,
            request_count: 4,
            inputs: vec![
                Input::item(1, 4, 0),  // 4 oak planks: enough for 2 runs
                Input::item(2, 1, 0),  // 1 stone: enough for 0 runs but Java asks 2×2=4
            ],
            ..busy()
        };
        assert_eq!(
            get_recipe(&facts),
            Recipe::ShortOfIngredient {
                input: 1,
                available: 1,
                remaining: 4,
            }
        );
    }

    /// `:366-421`, the worked example: a request for 4, two runs already done, and
    /// two inputs — one limiting the runs to 3, the other to 5. The answer is
    /// `min(3, 5, 4) + 2 = 4`, capped at the request's 4.
    #[test]
    fn the_run_count_is_the_narrowest_input_plus_what_is_already_made() {
        let facts = CraftFacts {
            request_count: 4,
            request_min_count: 1,
            output_count: 1,
            carried_output: 2, // two runs are in the inventory already
            inputs: vec![
                Input::item(1, 3, 0),          // 3 in the hut → 3 runs
                Input::item(1, 2, 3),          // 5 everywhere → 5 runs
            ],
            ..busy()
        };
        assert_eq!(
            get_recipe(&facts),
            Recipe::Ready {
                max_crafting_count: 4, // min(3, 5, 4) + 2 = 4, capped at 4
                craft_counter: 2,
            }
        );
    }

    /// A tool or a container is needed once, not once per run (`:381-396`), so it
    /// never narrows the run count.
    #[test]
    fn a_tool_is_needed_once_and_does_not_narrow_the_runs() {
        let facts = CraftFacts {
            request_count: 8,
            request_min_count: 8,
            output_count: 1,
            inputs: vec![Input::tool(1, 1), Input::item(1, 8, 0)],
            ..busy()
        };
        assert_eq!(
            get_recipe(&facts),
            Recipe::Ready {
                max_crafting_count: 8,
                craft_counter: 0,
            }
        );
    }

    /// `:371-372` divides by the primary output's count; a zero there is Java's
    /// `ArithmeticException`, and a named refusal here.
    #[test]
    fn a_recipe_with_no_output_count_is_refused_instead_of_dividing_by_zero() {
        assert_eq!(
            get_recipe(&CraftFacts {
                output_count: 0,
                ..busy()
            }),
            Recipe::DegenerateOutput
        );
    }

    /// `:492-500`: an input the hut holds sends the citizen to the chest; an input
    /// nobody holds drops the recipe and reads the next task.
    #[test]
    fn a_short_input_goes_to_the_chest_or_back_to_the_recipe() {
        let from_hut = CraftFacts {
            has_recipe: true,
            max_crafting_count: 2,
            inputs: vec![Input::item(4, 64, 0)],
            ..busy()
        };
        assert_eq!(
            check_for_items(&from_hut),
            Items::Gather {
                input: 0,
                remaining: 8,
            }
        );
        let from_nowhere = CraftFacts {
            has_recipe: true,
            max_crafting_count: 2,
            inputs: vec![Input::item(4, 0, 0)],
            ..busy()
        };
        assert_eq!(check_for_items(&from_nowhere), Items::ReRead);
    }

    /// `:462-504`: with everything in hand the answer is `CRAFT`, and an extended
    /// count (a furnace's contents) counts as in hand.
    #[test]
    fn enough_items_in_hand_mean_craft() {
        let in_hand = CraftFacts {
            has_recipe: true,
            max_crafting_count: 2,
            inputs: vec![Input::item(4, 0, 8)],
            ..busy()
        };
        assert_eq!(check_for_items(&in_hand), Items::Craft);
        let extended = CraftFacts {
            has_recipe: true,
            max_crafting_count: 2,
            inputs: vec![Input::item(4, 0, 0).extended(8)],
            ..busy()
        };
        assert_eq!(check_for_items(&extended), Items::Craft);
        // Without a recipe there is nothing to check (`:448-451`).
        assert_eq!(check_for_items(&busy()), Items::NoRecipe);
    }

    /// `:511-572`: the five early exits, then the progress test.
    #[test]
    fn craft_hits_only_once_the_progress_is_reached() {
        let working = CraftFacts {
            has_recipe: true,
            has_request: true,
            ..busy()
        };
        assert_eq!(craft(&working), Craft::Hitting); // progress 0 < 30 hits
        assert_eq!(
            craft(&CraftFacts {
                at_work_position: false,
                ..working.clone()
            }),
            Craft::Walking
        );
        assert_eq!(
            craft(&CraftFacts {
                has_request: false,
                ..working.clone()
            }),
            Craft::NeedRecipe
        );
        assert_eq!(
            craft(&CraftFacts {
                request_cancelled: true,
                ..working.clone()
            }),
            Craft::Cancelled
        );
        assert_eq!(craft(&busy()), Craft::NoWork);
        // At 30 hits (skill 0) the recipe runs; `work_time::craft_hits(0) == 30`.
        let ready = CraftFacts {
            progress: 30,
            ..working.clone()
        };
        assert_eq!(craft(&ready), Craft::Execute);
        // At skill 20 the required progress is zero, so the first tick fires.
        let fast = CraftFacts {
            progress: 0,
            craft_speed_level: 20,
            ..working
        };
        assert_eq!(craft(&fast), Craft::Execute);
    }

    /// `:562-568`: the progress is reached but an input vanished → `START_WORKING`,
    /// not a hit.
    #[test]
    fn a_recipe_whose_input_vanished_stops_instead_of_running() {
        let facts = CraftFacts {
            has_recipe: true,
            has_request: true,
            progress: 30,
            max_crafting_count: 1,
            inputs: vec![Input::item(4, 0, 0)],
            ..busy()
        };
        assert_eq!(craft(&facts), Craft::OutOfItems);
    }

    /// `:579-637`: the four exits of one run, and the counter is compared **after**
    /// the increment (`:603`, `:609`).
    #[test]
    fn one_run_finalizes_at_the_planned_count() {
        let facts = CraftFacts {
            has_recipe: true,
            has_request: true,
            max_crafting_count: 2,
            craft_counter: 0,
            ..busy()
        };
        assert_eq!(execute(&facts), Execution::Again);
        assert_eq!(
            execute(&CraftFacts {
                craft_counter: 1,
                ..facts.clone()
            }),
            Execution::Finalized
        );
        assert_eq!(
            execute(&CraftFacts {
                recipe_failed: true,
                ..facts.clone()
            }),
            Execution::RecipeFailed
        );
        assert_eq!(
            execute(&CraftFacts {
                tool_broke: true,
                ..facts.clone()
            }),
            Execution::ToolBroke
        );
    }

    /// `:636-646`: the experience is half the request's count, and only when the
    /// inventory asks for a dump and a request is still in hand.
    #[test]
    fn the_completion_experience_is_half_the_request() {
        assert_eq!(completion_experience(8, true, true), Some(4.0));
        assert_eq!(completion_experience(7, true, true), Some(3.5));
        assert_eq!(completion_experience(8, false, true), None);
        assert_eq!(completion_experience(8, true, false), None);
    }

    /// `:187-193`: outside the hut the citizen walks in; inside, a walk in flight or
    /// an unreachable position keeps it still.
    #[test]
    fn the_idle_routine_walks_in_before_it_sits() {
        let outside = idle(&CraftFacts {
            in_building: false,
            ..CraftFacts::default()
        });
        assert_eq!(outside.step, IdleStep::WalkToBuilding);
        let walking = idle(&CraftFacts {
            navigation_done: false,
            ..CraftFacts::default()
        });
        assert_eq!(walking.step, IdleStep::Stay);
        let unreachable = idle(&CraftFacts {
            idle_pos: Some(Pos::new(1, 64, 1)),
            idle_pos_reachable: false,
            ..CraftFacts::default()
        });
        assert_eq!(unreachable.step, IdleStep::Stay);
    }

    /// `:195-206`: a stored `idlePos` that is a sitting tag is sat on, and the field
    /// is cleared; anything else is dropped.
    #[test]
    fn a_stored_sitting_position_is_used_then_cleared() {
        let seat = Pos::new(3, 65, 3);
        let decision = idle(&CraftFacts {
            idle_pos: Some(seat),
            sitting: vec![seat],
            ..CraftFacts::default()
        });
        assert_eq!(decision.step, IdleStep::SitDown(seat));
        assert_eq!(decision.idle_pos, None);
        let elsewhere = idle(&CraftFacts {
            idle_pos: Some(Pos::new(9, 65, 9)),
            sitting: vec![seat],
            ..CraftFacts::default()
        });
        assert_eq!(elsewhere.step, IdleStep::Stay);
        assert_eq!(elsewhere.idle_pos, None);
    }

    /// `:208-229`: the first coin looks for a seat — inside when it rains, inside or
    /// outside otherwise.
    #[test]
    fn the_first_coin_looks_for_a_seat_and_the_rain_decides_where() {
        let in_seat = Pos::new(1, 64, 1);
        let out_seat = Pos::new(2, 64, 2);
        let facts = CraftFacts {
            coin_a: true,
            sit_in: vec![in_seat],
            sit_out: vec![out_seat],
            ..CraftFacts::default()
        };
        // Drawn index 0: the inside list comes first (`:210-211`).
        assert_eq!(idle(&facts).idle_pos, Some(in_seat));
        let sunny = idle(&CraftFacts {
            pick: 1,
            ..facts.clone()
        });
        assert_eq!(sunny.idle_pos, Some(out_seat));
        // In the rain the outside list is never added (`:212-219`).
        let raining = idle(&CraftFacts {
            raining: true,
            pick: 1,
            ..facts
        });
        assert_eq!(raining.idle_pos, Some(in_seat));
    }

    /// `:231-251`, then `:253`: the second coin looks for a standing spot, and with
    /// neither coin the citizen wanders.
    #[test]
    fn the_second_coin_stands_and_no_coin_wanders() {
        let stand = Pos::new(4, 64, 4);
        let facts = CraftFacts {
            coin_b: true,
            stand_in: vec![stand],
            ..CraftFacts::default()
        };
        let decision = idle(&facts);
        assert_eq!(decision.step, IdleStep::StandAt(stand));
        assert_eq!(decision.idle_pos, Some(stand));
        assert_eq!(
            idle(&CraftFacts::default()).step,
            IdleStep::Wander,
            "no coin and no position: `:253`"
        );
        // A coin with no position to pick falls through to the wander.
        assert_eq!(
            idle(&CraftFacts {
                coin_a: true,
                coin_b: true,
                ..CraftFacts::default()
            })
            .step,
            IdleStep::Wander
        );
    }

    /// `:182`: a drawn index out of range must not panic — it is a fact, and facts
    /// come from a caller. `pick % len` keeps it in range.
    #[test]
    fn an_out_of_range_pick_is_wrapped_not_panicked_on() {
        let seat = Pos::new(1, 64, 1);
        let decision = idle(&CraftFacts {
            coin_a: true,
            pick: 97,
            sit_in: vec![seat],
            ..CraftFacts::default()
        });
        assert_eq!(decision.idle_pos, Some(seat));
    }

    /// The machine: a crafter with work leaves `IDLE` on its first tick, and a
    /// crafter without work stays there without a refusal.
    #[test]
    fn the_machine_leaves_idle_when_a_task_is_queued() {
        let mut machine = ai();
        assert_eq!(machine.state(), CraftState::Idle);
        let mut facts = busy();
        let fired = machine.tick(&mut facts).expect("hasWorkToDo fires");
        assert_eq!(fired.rule, "hasWorkToDo");
        assert_eq!(fired.java, "AbstractEntityAICrafting.java:164");
        assert_eq!(machine.state(), CraftState::StartWorking);

        let mut idle_machine = ai();
        let mut none = CraftFacts::default();
        // No work: `hasWorkToDo` is false and `idle` answers `NO_CHANGE`, which is
        // `None` — the tick moves nothing.
        assert!(idle_machine.tick(&mut none).is_none());
        assert_eq!(idle_machine.state(), CraftState::Idle);
    }

    /// The whole path of one recipe: `IDLE` → `START_WORKING` → `GET_RECIPE` →
    /// `QUERY_ITEMS` → `CRAFT`, with the counters the recipe step wrote.
    #[test]
    fn the_machine_walks_idle_to_craft() {
        let mut machine = ai();
        let mut facts = CraftFacts {
            has_recipe: true,
            has_request: true,
            progress: 30, // skill 0 needs 30 hits
            max_crafting_count: 2,
            inputs: vec![Input::item(1, 0, 8)],
            ..busy()
        };
        // IDLE → START_WORKING.
        machine.tick(&mut facts).expect("hasWorkToDo");
        assert_eq!(machine.state(), CraftState::StartWorking);
        // START_WORKING → GET_RECIPE (a recipe is in hand but no request is being
        // queried yet — `:311-316` reads `QUERY_ITEMS` only when both are set, and
        // both are, so the path goes straight to the items).
        machine.tick(&mut facts).expect("decide");
        assert_eq!(machine.state(), CraftState::QueryItems);
        // QUERY_ITEMS → CRAFT.
        machine.tick(&mut facts).expect("queryItems");
        assert_eq!(machine.state(), CraftState::Craft);
        // CRAFT → INVENTORY_FULL when the planned runs are done.
        facts.craft_counter = 1; // one run already made, one to go
        machine.tick(&mut facts).expect("craft");
        assert_eq!(machine.state(), CraftState::InventoryFull);
        // INVENTORY_FULL → IDLE, the base `afterDump`.
        machine.tick(&mut facts).expect("afterDump");
        assert_eq!(machine.state(), CraftState::Idle);
        // The four `java` fields on the path name the class that owns each rule.
        assert!(machine.transitions().iter().any(|t| t
            .java
            .starts_with("AbstractEntityAICrafting.java")));
        assert!(SOURCE.contains(":734-738"));
    }

    /// A state nobody names is Java's trap (`BasicStateMachine.java:220-228`): the
    /// six targets must cover every state they can reach, or the machine resets.
    #[test]
    fn every_reachable_state_can_be_left() {
        let machine = ai();
        for state in CraftState::ALL {
            assert!(
                machine.can_leave(state),
                "{} has no transition",
                state.java_name()
            );
        }
        // And the names are Java's, once each.
        let mut names: Vec<&str> = CraftState::ALL.iter().map(|s| s.java_name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 7);
    }
}
