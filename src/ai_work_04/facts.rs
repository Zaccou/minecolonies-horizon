//! What the three trades are allowed to read, and nothing else.
//!
//! Java reference: `EntityAIWorkSifter.java:107-222` reads its hut's daily quota
//! (`:110`), the citizen's hands (`:147-154`), its progress counter (`:186-192`), the
//! recipe the module picked (`:125-129`), the mesh counts (`:133-146`) and one random
//! draw (`:192`). `AbstractEntityAIUsesFurnace.java:182-499` reads the furnace module
//! (`:189`, `:201`), the fuel filter (`:190`, `:193`), every registered furnace's three
//! slots and its lit flag (`:137-143`, `:166`, `:330-332`), the building's smeltable and
//! fuel counts (`:230-235`), its empty slots (`:119`) and the request system
//! (`:242-247`). `EntityAIWorkSmelter.java:78-122` adds the ore-breaking module.
//!
//! **Why observations and not calls.** Every field below is something a hook *saw*.
//! Nothing here can read a block entity, resolve a recipe or draw a random number: the
//! caller does it once and writes the answer down, and the decision becomes a pure
//! function of this struct. That is what makes sixty-nine tests possible with no world,
//! and it is the same choice `crate::ai::facts` and
//! `crate::ai_build::crafter::CraftFacts` made.
//!
//! **What is reused instead of restated.** The sifter is a crafter: its `IDLE` targets
//! (`AbstractEntityAICrafting.java:164-165`) read `in_building`, `navigation_done`, the
//! five sitting/standing tag lists and the two coin flips. Those fields already exist
//! in [`crate::ai_build::crafter::CraftFacts`], with their Java lines and their own
//! tests, so [`SiftFacts::craft`] embeds that struct rather than copying twelve fields.
//! The AI's own memory (`delay`, `walk_to`, `ticks_to_complain`) is
//! [`crate::ai::worker::Memory`], reused whole.
//!
//! **Two facts that are not Java calls, and are marked as such.** `SiftFacts::roll` is
//! `worker.getRandom().nextInt(ONE_HUNDRED_PERCENT)` (`EntityAIWorkSifter.java:192`);
//! an unreplayable draw cannot be tested, so the caller supplies it. The second is
//! the ore-break fulfil result (`SmelterFacts::ore_fulfilled`), which is the answer of
//! a write (`module.fullFillRecipe`, `:101`) the port cannot perform — it is an
//! observation of an action the host ran, exactly like `crate::ai`'s `Placement`.

use crate::ai::Pos;
use crate::ai::worker::Memory;
use crate::ai_build::crafter::CraftFacts;

use super::states::Refusal;

/// The trade facts, the AI's memory, and the reason this tick could not go further:
/// the three things one [`crate::ai::state::Machine`] tick reads and writes.
///
/// This is this module's `Scene`. `crate::ai::worker::Scene` is the same triple for the
/// base table, with `WorldFacts` in place of `F`; the two cannot be one type because
/// `WorldFacts::job` (`crate::ai::facts::JobFacts`) has no variant for these trades and
/// that file is not this agent's to edit — see `MANQUANT.md` §1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scene<F> {
    /// What the caller observed.
    pub facts: F,
    /// What the AI remembers. Reused from the base table: `delay` is Java's `delay`
    /// (`AbstractEntityAIBasic.java:121`) and `walk_to` its `walkTo` (`:161`), which the
    /// smelter's three furnace states read, mirror and clear. The struct is taken whole
    /// rather than re-declared; the fields this module does not touch are the base
    /// table's (`slot_at`, `kept`, `pickup_counter`, …).
    pub memory: Memory,
    /// Why this tick did not move. Java keeps the reason implicit in the branch it
    /// took; the port records it so a player can be told and a test can assert on it.
    pub refusal: Option<Refusal>,
}

impl<F> Scene<F> {
    /// A scene on `facts` with an empty memory and no refusal.
    pub fn new(facts: F) -> Scene<F> {
        Scene {
            facts,
            memory: Memory::default(),
            refusal: None,
        }
    }

    /// Record why this tick could not go further. The last one wins: Java only ever
    /// has one branch in flight.
    pub fn refuse(&mut self, refusal: Refusal) {
        self.refusal = Some(refusal);
    }

    /// The reason recorded this tick, if any.
    pub fn why_not(&self) -> Option<Refusal> {
        self.refusal
    }
}

/// What the sifter's recipe gave it: the mesh in one hand and the input in the other.
///
/// Java reference: `EntityAIWorkSifter.java:160-170` —
/// `currentRecipeStorage.getCraftingTools().get(0)` for the mesh (`:160`) and
/// `getCleanedInput()` filtered against the mesh for the input (`:161-164`), then
/// `if (meshItem.isEmpty() || inputItem.isEmpty())` (`:166`). Java keeps the two
/// `ItemStack`s; the port keeps the two answers, because it never moves an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SiftRecipe {
    /// `!meshItem.isEmpty()`.
    pub mesh_present: bool,
    /// `!inputItem.isEmpty()`.
    pub input_present: bool,
}

impl SiftRecipe {
    /// A recipe with both hands filled: the sifter can sieve.
    pub fn complete() -> SiftRecipe {
        SiftRecipe {
            mesh_present: true,
            input_present: true,
        }
    }

    /// A recipe missing its input (`:161-164` found nothing outside the mesh).
    pub fn without_input() -> SiftRecipe {
        SiftRecipe {
            mesh_present: true,
            input_present: false,
        }
    }
}

/// Everything `EntityAIWorkSifter` reads, as observations.
///
/// Java reference: `:97-100` (`hasWorkToDo`), `:107-222` (`sift`), `:85-88`
/// (`getActionsDoneUntilDumping`), `:42-62` (the four constants and the progress
/// counter).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiftFacts {
    /// The crafter's half: `at_building` is `walkToBuilding()` (`:115`), `actions_done`
    /// is `job.getActionsDone()`, and the `IDLE` targets read the rest
    /// (`AbstractEntityAICrafting.java:164-165`).
    pub craft: CraftFacts,
    /// `building.getCurrentDailyQuantity()` (`:110`, `:191`).
    pub daily_quantity: u32,
    /// `building.getMaxDailyQuantity()` (`:110`, `:192`) — see
    /// [`super::sifter::max_daily_quantity`] for the hut's own formula.
    pub max_daily_quantity: u32,
    /// `InventoryUtils.hasBuildingEnoughElseCount(building, i -> i.is(ModTags.meshes),
    /// 1)` (`:133`): Java's answer is a count, and the branch tests `== 0`, so the port
    /// keeps the count and lets the rule compare.
    pub building_meshes: u32,
    /// `InventoryUtils.getItemCountInProvider(worker, i -> i.is(ModTags.meshes))`
    /// (`:135`): a mesh the citizen should not be carrying.
    pub worker_meshes: u32,
    /// `currentRecipeStorage`, `None` before the module answered (`:125-129`).
    pub recipe: Option<SiftRecipe>,
    /// `worker.getInventoryCitizen().hasSpace()` (`:120`).
    pub has_space: bool,
    /// `progress` (`:62`, `:156`, `:186-190`): the sieve's hit counter.
    pub progress: u32,
    /// `getSecondarySkillLevel()` (`:188`): the skill the sieve speed is read from.
    pub secondary_skill_level: i32,
    /// **Not a Java call.** `worker.getRandom().nextInt(ONE_HUNDRED_PERCENT)` (`:192`):
    /// the draw that decides whether the sifter dumps after this output.
    pub roll: u32,
    /// `currentRecipeStorage.fullfillRecipeAndCopy(...) == null` (`:199-203`): the
    /// recipe refused to run. An observation of an action, not a rule.
    pub fulfil_failed: bool,
}

impl Default for SiftFacts {
    /// A sifter standing in its hut, quota empty, no mesh in the racks, no recipe, a
    /// free inventory, no output made, skill zero, and a draw that does **not** dump
    /// (`roll: 99` against `CHANCE_TO_DUMP_INV = 10`).
    fn default() -> SiftFacts {
        SiftFacts {
            craft: CraftFacts::default(),
            daily_quantity: 0,
            max_daily_quantity: 64,
            building_meshes: 0,
            worker_meshes: 0,
            recipe: None,
            has_space: true,
            progress: 0,
            secondary_skill_level: 0,
            roll: 99,
            fulfil_failed: false,
        }
    }
}

/// One item of a recipe, for `isSmeltable`'s three tests.
///
/// Java reference: `ItemStackUtils.isEmpty(stack)` (`EntityAIWorkSmelter.java:188`),
/// `IColonyManager…isOre(stack)` (`:188`), `…isBreakableOre(stack)` (`:192`) and
/// `ItemListModule.isItemInList(new ItemStorage(stack))` (`:196`). The port never holds
/// an `ItemStack`: the caller answers the four questions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ore {
    /// `ItemStackUtils.isEmpty(stack)`.
    pub empty: bool,
    /// `ItemStackUtils.IS_SMELTABLE.and(…)` (`:188`): the item is fuel-able at all.
    pub smeltable: bool,
    /// `IColonyManager.getInstance().getCompatibilityManager().isOre(stack)` (`:188`).
    pub is_ore: bool,
    /// `…isBreakableOre(stack)` (`:192`): the ore-breaking module can crack it.
    pub breakable: bool,
    /// The hut's `"ores"` list contains it (`:196`, the module id at `:59`).
    pub in_ore_list: bool,
}

impl Ore {
    /// A plain ore the smelter should smelt.
    pub fn ore() -> Ore {
        Ore {
            empty: false,
            smeltable: true,
            is_ore: true,
            breakable: false,
            in_ore_list: false,
        }
    }
}

/// The ore-breaking module's answer, as the AI reads it.
///
/// Java reference: `EntityAIWorkSmelter.java:80-104` —
/// `module.getFirstFulfillableRecipe(ItemStackUtils::isEmpty, 1, false)` (`:81`), the
/// first cleaned input (`:88-90`) and `module.fullFillRecipe(...)` (`:101`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OreRecipe {
    /// `inputItem.getCount()` (`:97`): what one break consumes. Zero is Java's
    /// `inputItem.isEmpty()` (`:92`).
    pub input_count: u32,
}

/// One registered furnace, as `AbstractEntityAIUsesFurnace` reads it.
///
/// Java reference: `world.getBlockEntity(pos) instanceof FurnaceBlockEntity`
/// (`:134`, `:162`, `:327`), `furnace.isLit()` (`:140`, `:297`),
/// `getItem(RESULT_SLOT)` / `getItem(SMELTABLE_SLOT)` (`:137-138`, `:402`),
/// `getItem(FUEL_SLOT)` (`:166`, `:435`) and `world.getBlockState(pos).getBlock()
/// instanceof FurnaceBlock` (`:340`, `:464`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Furnace {
    /// Where it stands: the entry of `furnaceModule.getFurnaces()`.
    pub at: Pos,
    /// `world.getBlockEntity(pos) instanceof FurnaceBlockEntity`.
    pub entity: bool,
    /// `world.getBlockState(pos).getBlock() instanceof FurnaceBlock` (`:340`): when
    /// this is false the module drops the position (`:342`).
    pub block_is_furnace: bool,
    /// `furnace.isLit()`.
    pub lit: bool,
    /// `furnace.getItem(RESULT_SLOT).getCount()`, `RESULT_SLOT = 2`
    /// (`Constants.java:118`).
    pub result: u32,
    /// `furnace.getItem(SMELTABLE_SLOT).getCount()`, `SMELTABLE_SLOT = 0`
    /// (`Constants.java:123`).
    pub input: u32,
    /// `!furnace.getItem(FUEL_SLOT).isEmpty()`, `FUEL_SLOT = 1` (`Constants.java:128`).
    pub fuel_present: bool,
    /// `!fuelListModule.isItemInList(new ItemStorage(furnace.getItem(FUEL_SLOT)))`
    /// (`:166`): a fuel the hut's filter does not know, which the worker takes back.
    pub fuel_allowed: bool,
}

impl Furnace {
    /// A furnace holding `input` of smeltable, fuelled and lit: the busy furnace that
    /// `getPositionOfOvenToRetrieveFrom` skips until its result slot passes ten
    /// (`:140-143`).
    pub fn burning(at: Pos, input: u32, result: u32) -> Furnace {
        Furnace {
            at,
            entity: true,
            block_is_furnace: true,
            lit: true,
            result,
            input,
            fuel_present: true,
            fuel_allowed: true,
        }
    }

    /// A furnace with nothing in it: ready for both transfers
    /// (`hasNeitherFuelNorSmeltAble`, `ItemStackUtils.java:731-735`).
    pub fn empty(at: Pos) -> Furnace {
        Furnace {
            at,
            entity: true,
            block_is_furnace: true,
            ..Furnace::default()
        }
    }

    /// `!ItemStackUtils.isEmpty(getItem(SMELTABLE_SLOT))`.
    pub fn has_smeltable(self) -> bool {
        self.input > 0
    }

    /// `!ItemStackUtils.isEmpty(getItem(FUEL_SLOT))`.
    pub fn has_fuel(self) -> bool {
        self.fuel_present
    }

    /// `ItemStackUtils.hasSmeltableInFurnaceAndNoFuel` (`ItemStackUtils.java:719-723`):
    /// smeltable in, fuel out.
    pub fn has_smeltable_and_no_fuel(self) -> bool {
        self.has_smeltable() && !self.has_fuel()
    }

    /// `ItemStackUtils.hasFuelInFurnaceAndNoSmeltable` (`:743-747`).
    pub fn has_fuel_and_no_smeltable(self) -> bool {
        !self.has_smeltable() && self.has_fuel()
    }

    /// `ItemStackUtils.hasNeitherFuelNorSmeltAble` (`:731-735`).
    pub fn has_neither(self) -> bool {
        !self.has_smeltable() && !self.has_fuel()
    }
}

/// Everything `EntityAIWorkSmelter` and `AbstractEntityAIUsesFurnace` read, as
/// observations.
///
/// Java reference: `AbstractEntityAIUsesFurnace.java:182-261` (`startWorking`),
/// `:117-121` (`reachedMaxToKeep`), `:129-173` (the two oven scans), `:230-247` (the
/// four counts and the fuel request), `:320-348` (`checkIfAbleToSmelt`), `:388-499`
/// (the three actions), plus `EntityAIWorkSmelter.java:155-238` (the important job and
/// the smeltable request).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SmelterFacts {
    /// `walkToBuilding()` (`AbstractEntityAIUsesFurnace.java:184`).
    pub at_building: bool,
    /// `itemListModule.getList().isEmpty()` (`:193`): no fuel filter, so no fuel.
    pub fuel_list_empty: bool,
    /// `furnaceModule.getFurnaces()`, in the order the hut reports them — the order is
    /// the rule: the first matching furnace is the one worked on (`:323-337`).
    pub furnaces: Vec<Furnace>,
    /// `InventoryUtils.getCountFromBuilding(building, this::isSmeltable)` (`:230`).
    pub smeltable_in_building: u32,
    /// `InventoryUtils.getItemCountInItemHandler(worker.getInventoryCitizen(),
    /// this::isSmeltable)` (`:231`).
    pub smeltable_in_inventory: u32,
    /// `InventoryUtils.getCountFromBuilding(building, itemListModule.getList())`
    /// (`:233`).
    pub fuel_in_building: u32,
    /// `InventoryUtils.getItemCountInItemHandler(…, itemListModule::isItemInList)`
    /// (`:234-235`).
    pub fuel_in_inventory: u32,
    /// `InventoryUtils.countEmptySlotsInBuilding(building)` (`:119`), compared against
    /// `STORAGE_BUFFER = 3` (`:66`, `:120`).
    pub empty_slots_in_building: u32,
    /// `building.hasWorkerOpenRequestsFiltered(…, REQUESTS_TYPE_BURNABLE)` (`:242-244`).
    pub open_fuel_request: bool,
    /// `worker.getInventoryCitizen().hasSpace()` (`EntityAIWorkSmelter.java:162`).
    pub has_space: bool,
    /// `!ItemStackUtils.isEmpty(worker.getMainHandItem())`
    /// (`EntityAIWorkSmelter.java:157`): the important job starts by emptying the hand.
    pub main_hand_full: bool,
    /// `module.getFirstFulfillableRecipe(...)` (`EntityAIWorkSmelter.java:81`).
    pub ore_recipe: Option<OreRecipe>,
    /// `!allowedItems.isEmpty()` (`EntityAIWorkSmelter.java:208-219`): the hut's `"ores"`
    /// list is not empty, so the request is a `StackList` of the ores outside it.
    pub ore_list_allows_any: bool,
    /// The ores outside the hut's list that could still be asked for
    /// (`EntityAIWorkSmelter.java:219`): zero means `FURNACE_USER_NO_ORE`.
    pub ore_outside_list: u32,
    /// `InventoryUtils.hasBuildingEnoughElseCount(building, s -> new
    /// SmeltableOre(1).matches(s), 1)` (`EntityAIWorkSmelter.java:202`).
    pub smeltable_request_already_covered: bool,
    /// `building.hasWorkerOpenRequestsOfType(…, getSmeltAbleClass().getClass())`
    /// (`EntityAIWorkSmelter.java:203`) — the two request checks of `:203-205` folded
    /// into one answer.
    pub open_smeltable_request: bool,
    /// `InventoryUtils.hasItemInItemHandler(inv, this::isSmeltable)` (`:480`).
    pub inventory_has_smeltable: bool,
    /// `InventoryUtils.hasItemInItemHandler(inv, itemListModule::isItemInList)`
    /// (`:489`).
    pub inventory_has_fuel: bool,
    /// **Not a Java call**, an observation of one: `module.fullFillRecipe(
    /// currentRecipeStorage)` (`EntityAIWorkSmelter.java:101`) answered true. The write
    /// is the host's; the rule `breakOres` applies to its answer is this module's.
    ///
    /// The protocol, because it is not obvious: on every `BREAK_ORES` tick the caller
    /// attempts the fulfil for the recipe it reports in [`SmelterFacts::ore_recipe`] and
    /// writes the answer here **before** ticking. The port never resets it — a stale
    /// answer is a stale observation, not a rule of this module.
    pub ore_fulfilled: bool,
    /// `getPrimarySkillLevel()` (`AbstractEntityAIUsesFurnace.java:285`): the furnace
    /// acceleration is read from it.
    pub primary_skill_level: u8,
    /// `walkToWorkPos(walkTo)` (`:395`, `:428`, `:470`): the citizen stands where it
    /// can reach the position it is walking to. A fact, because arriving is the host's
    /// answer, not a rule.
    pub at_walk_target: bool,
    /// `job.getActionsDone()` (`AbstractEntityAIBasic.java:413`): incremented by an
    /// extraction (`AbstractEntityAIUsesFurnace.java:411`, `:145`) so that the base
    /// table dumps — `getActionsDoneUntilDumping()` is `1` (`:376-380`).
    pub actions_done: u32,
    /// What the last `startWorking` decided to ask for (`EntityAIWorkSmelter.java:239`
    /// calls `requestSmeltable`): the request itself is the caller's. Kept here because
    /// a decision that is not observable cannot be tested.
    pub asked_smeltable: bool,
    /// What the last `startWorking` decided to ask for as fuel
    /// (`AbstractEntityAIUsesFurnace.java:245-247`).
    pub asked_fuel: bool,
}

impl SmelterFacts {
    /// A smelter that walked to its hut, whose fuel filter is set, with one empty
    /// furnace, one ore in the hut and nothing in its pockets.
    pub fn new() -> SmelterFacts {
        SmelterFacts {
            at_building: true,
            fuel_list_empty: false,
            furnaces: vec![Furnace::empty(Pos::new(1, 65, 1))],
            smeltable_in_building: 8,
            smeltable_in_inventory: 0,
            fuel_in_building: 8,
            fuel_in_inventory: 0,
            empty_slots_in_building: 27,
            open_fuel_request: false,
            has_space: true,
            main_hand_full: false,
            ore_recipe: None,
            ore_list_allows_any: false,
            ore_outside_list: 0,
            smeltable_request_already_covered: false,
            open_smeltable_request: false,
            inventory_has_smeltable: false,
            inventory_has_fuel: false,
            ore_fulfilled: false,
            primary_skill_level: 0,
            at_walk_target: true,
            actions_done: 0,
            asked_smeltable: false,
            asked_fuel: false,
        }
    }

    /// The smeltable count Java keeps in two places (`:230-231`).
    pub fn smeltable_total(&self) -> u32 {
        self.smeltable_in_building
            .saturating_add(self.smeltable_in_inventory)
    }

    /// The fuel count, same two places (`:233-235`).
    pub fn fuel_total(&self) -> u32 {
        self.fuel_in_building.saturating_add(self.fuel_in_inventory)
    }

    /// The registered furnaces whose block entity is a furnace (`:327`): the only ones
    /// any of the three scans looks inside.
    pub fn furnaces_with_entity(&self) -> impl Iterator<Item = &Furnace> + '_ {
        self.furnaces.iter().filter(|furnace| furnace.entity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_scene_has_no_memory_and_no_refusal() {
        let scene = Scene::new(SiftFacts::default());
        assert_eq!(scene.memory.delay, 0);
        assert_eq!(scene.memory.walk_to, None);
        assert_eq!(scene.why_not(), None);
        // `Memory::default` is the base table's, not a copy of it
        // (`AbstractEntityAIBasic.java:136`, `:181`).
        assert_eq!(scene.memory.exception_timer, 1);
        assert_eq!(scene.memory.work_pos_index, -1);
    }

    #[test]
    fn the_last_refusal_wins() {
        let mut scene = Scene::new(SmelterFacts::new());
        scene.refuse(Refusal::NoFuelList);
        assert_eq!(scene.why_not(), Some(Refusal::NoFuelList));
        scene.refuse(Refusal::NoFurnace);
        assert_eq!(scene.why_not(), Some(Refusal::NoFurnace));
    }

    #[test]
    fn a_default_sifter_has_a_quota_of_sixty_four_and_no_mesh_anywhere() {
        // `BuildingSifter.java:87` at level 1: `1² × 64`.
        let facts = SiftFacts::default();
        assert_eq!(facts.max_daily_quantity, 64);
        assert_eq!(facts.daily_quantity, 0);
        assert_eq!(facts.building_meshes, 0);
        assert_eq!(facts.worker_meshes, 0);
        assert!(facts.has_space);
        assert!(
            facts.craft.at_building,
            "the crafter default stands in the hut"
        );
        // The draw Java would have made: above `CHANCE_TO_DUMP_INV`, so no dump.
        assert_eq!(facts.roll, 99);
    }

    #[test]
    fn a_complete_sift_recipe_fills_both_hands() {
        assert!(SiftRecipe::complete().mesh_present);
        assert!(SiftRecipe::complete().input_present);
        // `:166` — either hand empty is enough to drop the recipe.
        assert!(!SiftRecipe::without_input().input_present);
    }

    #[test]
    fn the_three_furnace_predicates_are_the_ones_item_stack_utils_defines() {
        let empty = Furnace::empty(Pos::new(0, 64, 0));
        assert!(empty.has_neither());
        assert!(!empty.has_smeltable_and_no_fuel());
        assert!(!empty.has_fuel_and_no_smeltable());

        let unfuelled = Furnace { input: 3, ..empty };
        assert!(unfuelled.has_smeltable_and_no_fuel()); // ItemStackUtils.java:719-723
        assert!(!unfuelled.has_neither());

        let unfed = Furnace {
            fuel_present: true,
            ..empty
        };
        assert!(unfed.has_fuel_and_no_smeltable()); // :743-747

        let both = Furnace {
            input: 1,
            fuel_present: true,
            ..empty
        };
        assert!(!both.has_neither());
        assert!(!both.has_smeltable_and_no_fuel());
        assert!(!both.has_fuel_and_no_smeltable());
    }

    #[test]
    fn a_furnace_without_an_entity_is_not_scanned_inside() {
        // `:327` — `entity instanceof FurnaceBlockEntity` is the gate of every scan.
        let mut facts = SmelterFacts::new();
        facts.furnaces.push(Furnace {
            at: Pos::new(2, 65, 2),
            entity: false,
            block_is_furnace: true,
            ..Furnace::default()
        });
        let seen: Vec<Pos> = facts.furnaces_with_entity().map(|f| f.at).collect();
        assert_eq!(seen, vec![Pos::new(1, 65, 1)]);
        // The block is still a furnace: `:340-343` only removes a position whose
        // *block* is not a furnace block.
        assert!(facts.furnaces[1].block_is_furnace);
    }

    #[test]
    fn the_smelter_totals_add_both_places_like_java_does() {
        // `:230-235`, then `:237` and `:249-258` compare the sums.
        let facts = SmelterFacts {
            smeltable_in_building: 5,
            smeltable_in_inventory: 2,
            fuel_in_building: 0,
            fuel_in_inventory: 3,
            ..SmelterFacts::new()
        };
        assert_eq!(facts.smeltable_total(), 7);
        assert_eq!(facts.fuel_total(), 3);
    }

    #[test]
    fn an_ore_carries_the_four_questions_is_smeltable_asks() {
        let ore = Ore::ore();
        assert!(!ore.empty);
        assert!(ore.smeltable && ore.is_ore);
        assert!(
            !ore.breakable,
            "a breakable ore is not smelted (`:192-195`)"
        );
        assert!(
            !ore.in_ore_list,
            "and one in the list is not smelted either (`:196`)"
        );
        // The default is "a stack that is nothing": not empty (nothing was said), but
        // not smeltable either, and not in the hut's list.
        assert_eq!(
            Ore::default(),
            Ore {
                empty: false,
                smeltable: false,
                is_ore: false,
                breakable: false,
                in_ore_list: false,
            }
        );
    }

    #[test]
    fn the_base_refusal_type_is_still_the_one_of_the_base_table() {
        // The two refusal types are separate on purpose: this module names reasons
        // `AbstractEntityAIBasic` never returns, and does not re-export the base set.
        assert_ne!(
            crate::ai::Refusal::ToolMissing.java(),
            Refusal::NoMesh.java(),
            "the two types cite different files"
        );
    }
}
