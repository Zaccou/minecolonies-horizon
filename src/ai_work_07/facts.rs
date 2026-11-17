//! The facts the three trades of this slice read, and nothing else.
//!
//! Java reference: `core/entity/ai/workers/production/EntityAIWorkNether.java` and
//! `.../production/agriculture/EntityAIWorkBeekeeper.java` — every `world`, `worker`,
//! `building` and `job` read on the way to a decision. The composter's own facts live
//! in `crate::ai::facts::ComposterFacts` and are not repeated here.
//!
//! **Why data and not calls.** Java's AI holds a live `EntityCitizen`, a `Level`, a
//! `BuildingNetherWorker` and a `JobNetherWorker`. A wasm mod has none of that at this
//! layer, and a decision that can only be tested inside the game cannot be tested at
//! all. Every field below is something the caller *observed*; this module never asks
//! the world a question.
//!
//! **What belongs where.** A fact about the world is a [`NetherFacts`] or
//! [`BeekeeperFacts`] field. What the *job* carries between two ticks —
//! `JobNetherWorker.craftedResults` (`JobNetherWorker.java:30`), the beekeeper's
//! `lastHarvestedBottle` (`EntityAIWorkBeekeeper.java:98`) — is
//! [`super::facts::WorkMemory`] instead, because it is written by this AI and not read
//! from the world.
//!
//! **The four draws.** `EntityAIWorkNether` calls `worker.getRandom()` in `stayInNether`
//! (`:417-418`, `:604-624`), the beekeeper in `harvestHoney` (`:400`) and a draw decides
//! the beehive's anger (`:409`). A random draw a host replays is not a fact a test can
//! assert on, so each one is a field here and is marked **not a Java call**.

use core::fmt;

use crate::ai::facts::{Pos, WorldFacts};
use crate::ai::worker::{JobStatus, Memory};
use crate::jobs::tool::ToolKind;

use super::state::WorkRefusal;

/// The block questions these rules ask, and no others.
///
/// Java reference: `world.getBlockState(portal).is(Blocks.NETHER_PORTAL)` — the only
/// block test the nether miner makes (`EntityAIWorkNether.java:271`, `:340`, `:634`,
/// `:677`, `:695`). `crate::ai::facts::Block` answers the builder's and the composter's
/// five questions (air/fluid/solid/barrel/undestroyable) and has no `portal` value, so
/// this module carries the three answers its own rules read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
// exhaustive: the three answers the nether rules ask of one position
pub enum WorkBlock {
    /// `Blocks.AIR` — the portal is out, or nothing stands there.
    #[default]
    Air,
    /// `Blocks.NETHER_PORTAL` (`:341`).
    Portal,
    /// Anything else, including the obsidian frame.
    Solid,
}

impl WorkBlock {
    /// `block.is(Blocks.NETHER_PORTAL)` (`:341`).
    pub fn is_portal(self) -> bool {
        self == WorkBlock::Portal
    }

    /// Java's name for the block, for a log line.
    pub fn java_name(self) -> &'static str {
        match self {
            WorkBlock::Air => "air",
            WorkBlock::Portal => "nether_portal",
            WorkBlock::Solid => "solid",
        }
    }
}

/// What the caller observed about blocks: the same map as `crate::ai::facts::Blocks`,
/// with the nether's vocabulary.
///
/// Java reads the level; this port reads what a hook wrote down. A position nobody
/// recorded is [`WorkBlock::Air`] — Java's answer for a position with no block — and a
/// position the caller marks unloaded is *not* air.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkBlocks {
    entries: Vec<(Pos, WorkBlock)>,
    unloaded: Vec<Pos>,
}

impl WorkBlocks {
    /// No blocks at all: every position reads as air, every chunk as loaded.
    pub fn new() -> WorkBlocks {
        WorkBlocks::default()
    }

    /// Report what stands at `pos`.
    pub fn set(&mut self, pos: Pos, block: WorkBlock) {
        if let Some(slot) = self.entries.iter_mut().find(|(at, _)| *at == pos) {
            slot.1 = block;
            return;
        }
        self.entries.push((pos, block));
    }

    /// What stands at `pos`, air when the caller said nothing
    /// (`world.getBlockState(pos)`).
    pub fn at(&self, pos: Pos) -> WorkBlock {
        self.entries
            .iter()
            .find(|(at, _)| *at == pos)
            .map(|(_, block)| *block)
            .unwrap_or(WorkBlock::Air)
    }

    /// Mark `pos` as not in a loaded chunk (`WorldUtil.isBlockLoaded`, which the
    /// composter asks at `EntityAIWorkComposter.java:115` but no nether rule does).
    pub fn mark_unloaded(&mut self, pos: Pos) {
        if !self.unloaded.contains(&pos) {
            self.unloaded.push(pos);
        }
    }

    /// Whether `pos` is in a loaded chunk.
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

/// One hive the apiary's hut lists, as the beekeeper reads it.
///
/// Java reference: `building.getHives()` (`EntityAIWorkBeekeeper.java:316`), the
/// `BlockTags.BEEHIVES` test (`:319`), `BeehiveBlockEntity.getHoneyLevel(state) >= 5`
/// (`:321`) and `getOccupantCount()` (`:152`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hive {
    /// The hive's position, as the hut stores it.
    pub at: Pos,
    /// `blockState.is(BlockTags.BEEHIVES)` (`:319`). A hive that fails this is removed
    /// from the hut's list (`:328`), which is the caller's write, not a decision.
    pub valid: bool,
    /// `BeehiveBlockEntity.getHoneyLevel(state)` (`:321`): 0 to 5, and 5 is full.
    pub honey_level: i32,
    /// `BeehiveBlockEntity.getOccupantCount()` (`:152`): the bees inside.
    pub occupants: i32,
}

impl Hive {
    /// A full hive at `at` (`:321`).
    pub fn full(at: Pos) -> Hive {
        Hive {
            at,
            valid: true,
            honey_level: 5,
            occupants: 3,
        }
    }

    /// An empty hive at `at`.
    pub fn empty(at: Pos) -> Hive {
        Hive {
            at,
            valid: true,
            honey_level: 0,
            occupants: 0,
        }
    }

    /// `getHoneyLevel(state) >= 5` (`:321`): the one level the rule compares against.
    pub fn is_full(self) -> bool {
        self.honey_level >= super::beekeeper::HONEY_LEVEL_FULL
    }
}

/// One bee the hut's hives attract.
///
/// Java reference: `searchForAnimals` (`EntityAIWorkBeekeeper.java:587-601`) returns
/// `Bee`s from a 22-block box around every hive (`:594-598`); `breedAnimals` then reads
/// `!isBaby()` (`:274`), `getAge() == 0` (`:287`), `isInLove()` (`:491`) and
/// `distanceTo(animalOne)` (`:285`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bee {
    /// `animal.blockPosition()` (`:428`): where the bee stands.
    pub at: Pos,
    /// `animal.getAge()` (`:287`): negative for a baby, `0` for a grown bee that will
    /// breed, positive for one still on its breeding cooldown.
    pub age: i32,
    /// `animal.isInLove()` (`:491`): already looking for a mate.
    pub in_love: bool,
}

impl Bee {
    /// A grown, idle bee at `at`.
    pub fn grown(at: Pos) -> Bee {
        Bee {
            at,
            age: 0,
            in_love: false,
        }
    }

    /// A baby at `at`.
    pub fn baby(at: Pos) -> Bee {
        Bee {
            at,
            age: -1,
            in_love: false,
        }
    }

    /// `animal.isBaby()` — `getAge() < 0`.
    pub fn is_baby(self) -> bool {
        self.age < 0
    }

    /// The breeding test of `:287`: `getAge() == 0`, **not** `!isBaby()`. A bee on its
    /// post-breeding cooldown has a positive age and passes `:274` but not this.
    pub fn will_breed(self) -> bool {
        self.age == 0
    }
}

/// What the apiary is set to collect.
///
/// Java reference: `BuildingBeekeeper.HONEYCOMB` / `HONEY` / `BOTH`
/// (`BuildingBeekeeper.java:53-55`), read through `getHarvestTypes()` (`:164`, `:172`,
/// `:343`, `:371`, `:384`). The setting is built as
/// `new BeekeeperCollectionSetting(HONEYCOMB, HONEY, BOTH)`
/// (`core/colony/buildings/modules/BuildingModules.java:162`) and `StringSetting`
/// starts at index `0` (`core/colony/buildings/modules/settings/StringSetting.java:56`),
/// so **honeycomb is the default**.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
// exhaustive: the three settings the apiary offers
pub enum HarvestType {
    /// `com.minecolonies.core.apiary.setting.honeycomb` — the default (`:56`).
    #[default]
    Honeycomb,
    /// `com.minecolonies.core.apiary.setting.honey`.
    Honey,
    /// `com.minecolonies.core.apiary.setting.both`.
    Both,
}

impl HarvestType {
    /// The setting string, which is the key `getHarvestTypes()` returns
    /// (`BuildingBeekeeper.java:53-55`).
    pub fn setting(self) -> &'static str {
        match self {
            HarvestType::Honeycomb => "com.minecolonies.core.apiary.setting.honeycomb",
            HarvestType::Honey => "com.minecolonies.core.apiary.setting.honey",
            HarvestType::Both => "com.minecolonies.core.apiary.setting.both",
        }
    }

    /// `!building.getHarvestTypes().equals(HONEY)` (`:164`): shears are needed.
    pub fn needs_shears(self) -> bool {
        self != HarvestType::Honey
    }

    /// `!building.getHarvestTypes().equals(HONEYCOMB)` (`:172`, `:384`): bottles are
    /// needed.
    pub fn needs_bottles(self) -> bool {
        self != HarvestType::Honeycomb
    }

    /// `BOTH` (`:343`): which tool is used depends on the last harvest.
    pub fn is_both(self) -> bool {
        self == HarvestType::Both
    }
}

/// What `checkForItems` answered about the hut's recipe.
///
/// Java reference: `AbstractEntityAICrafting.checkForItems`
/// (`core/entity/ai/workers/crafting/AbstractEntityAICrafting.java:462-504`) returns
/// three states, and `EntityAIWorkNether.decide` distinguishes exactly two of them
/// (`EntityAIWorkNether.java:299`, `:306`). The crafting AI itself lives in
/// `src/crafting`, which `lib.rs` does not declare, so the answer arrives as a fact.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
// exhaustive: the two answers `decide` distinguishes
pub enum CraftCheck {
    /// `return CRAFT` (`:503`): the inputs are in hand.
    #[default]
    Craft,
    /// `return GATHERING_REQUIRED_MATERIALS` (`:495`): they are in the hut.
    GatherMaterials,
    /// `return GET_RECIPE` (`:499`): they are nowhere, and the recipe is dropped.
    RecipeLost,
}

/// The recipe the nether miner is sent away with, as the hut answers it.
///
/// Java reference: `building.getFirstModuleOccurance(CraftingModule.class)
/// .getFirstRecipe(ItemStack::isEmpty)` (`EntityAIWorkNether.java:219`) and the request
/// loop over its inputs (`:223-230`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetherRecipe {
    /// `checkIfRequestForItemExistOrCreateAsync(...)` answered `true` for **every**
    /// input (`:225-229`): the items are there or on their way.
    pub inputs_available: bool,
}

impl NetherRecipe {
    /// A recipe whose inputs are all in the hut.
    pub fn ready() -> NetherRecipe {
        NetherRecipe {
            inputs_available: true,
        }
    }

    /// A recipe missing at least one input (`:232-236`).
    pub fn waiting() -> NetherRecipe {
        NetherRecipe {
            inputs_available: false,
        }
    }
}

/// Everything `EntityAIWorkNether` reads that the base facts do not carry.
///
/// The six numbers of the fight (`:403-432`, `:955-980`) are here rather than in a
/// separate struct because they all come from the same two objects Java holds.
#[derive(Clone, Debug, PartialEq)]
pub struct NetherFacts {
    /// The worker's own position, for the distance checks (`:407`). Duplicated from
    /// `WorkerFacts::at` on purpose: the rules read it without a second lookup.
    pub at: Pos,
    /// `worker.getCitizenData().getColony().getTravellingManager()
    /// .isTravelling(citizenData)` (`:186`): the colony is walking the citizen; no
    /// entity is spawned.
    pub travelling: bool,
    /// `getTravellingTargetFor(citizenData)` (`:192`): present means the trip ended and
    /// the respawn point must be moved. A fact, because moving it is the colony's write.
    pub travel_target: Option<Pos>,
    /// `building.getPortalLocation()` (`:238`): the tagged air block above `portal`,
    /// `None` when the hut has no such tag.
    pub portal: Option<Pos>,
    /// `PortalShape.findPortalShape(world, portal, p -> p.isValid(), Axis.X)`
    /// (`:669`): `false` means the frame is not a portal frame at all, and
    /// `openPortal` answers `IDLE` instead of lighting it (`:671-675`).
    pub portal_frame_valid: bool,
    /// `building.shallClosePortalOnReturn()` (`:269`, `:634`): the `closeportal` setting
    /// (`BuildingNetherWorker.java:40-41`, `:129-132`).
    pub shall_close_portal_on_return: bool,
    /// `building.isReadyForTrip()` (`:282`): `currentTrips < getMaxPerPeriod()`
    /// (`BuildingNetherWorker.java:215-228`, `:61`).
    pub ready_for_trip: bool,
    /// `building.getFirstRecipe(...)` (`:219`), `None` when the hut lists no recipe at
    /// all — in which case Java never reaches the request loop.
    pub hut_recipe: Option<NetherRecipe>,
    /// `module.getFirstFulfillableRecipe(ItemStackUtils::isEmpty, 1, false)` (`:263`)
    /// returned something, i.e. `currentRecipeStorage` can be set.
    pub fulfillable_recipe: bool,
    /// How many edibles of the menu the worker carries (`:938`): `>= 16` is enough.
    pub food_in_inventory: u32,
    /// `InventoryUtils.hasBuildingEnoughElseCount(building, ..., 1)` (`:944`): the hut
    /// can stock the worker back up.
    pub food_in_building: u32,
    /// The tools of `Job::NetherWorker.tools()` the worker does **not** hold, from the
    /// five `checkForToolOrWeapon` calls (`:248-252`). Order is the caller's; the port
    /// reports the first one Java would ask for.
    pub missing_tools: Vec<ToolKind>,
    /// `walkTo != null` (`:288`): a walk is already in flight, so `decide` does not
    /// start a new one.
    pub walking: bool,
    /// `checkForItems(currentRecipeStorage)` (`:297`).
    pub craft_check: CraftCheck,
    /// The crafting half's answer at `:201`: `true` when `super.decide()` returned
    /// something that is neither `IDLE` nor `START_WORKING` (`:203`). The port does not
    /// name that state — it belongs to `src/crafting`.
    pub crafter_busy: bool,
    /// `worker.isDeadOrDying()` (`:409`, `:479`, `:570`).
    pub worker_dead: bool,
    /// `getSecondarySkillLevel()` (`:407`), the citizen's secondary skill: the fight's
    /// damage reduction is `skill * SECONDARY_DAMAGE_REDUCTION`.
    pub secondary_skill: i32,
    /// The sword in the main hand (`:421-432`), `None` when the slot is empty.
    pub sword: Option<SwordDamage>,
    /// `worker.getHealth()` / `getMaxHealth()` (`:959`).
    pub health: f32,
    /// The maximum (`:959`).
    pub max_health: f32,
    /// `citizenData.getSaturation()` (`:963`, `:967`), and the **same** value on the
    /// fight's odd rounds (`:469`).
    pub saturation: f64,
    /// `getResearchEffects().getEffectStrength(SATLIMIT)` (`:961`).
    pub sat_limit_research: f64,
    /// `getResearchEffects().getEffectStrength(REGENERATION)` (`:965`, `:973`).
    pub regeneration_research: f64,
    /// `world.getBlockState(portal)` for the positions the rules read (`:271`,
    /// `:340`, `:634`, `:668`, `:695`).
    pub blocks: WorkBlocks,
}

/// The two numbers the sword contributes to one blow.
///
/// Java reference: `EntityAIWorkNether.java:421-432` — a `SwordItem`'s `getDamage()`
/// (`:426`) or Tinkers' (`:430`), plus `EnchantmentHelper.getDamageBonus(...) / 2.5`
/// (`:432`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwordDamage {
    /// `((SwordItem) sword.getItem()).getDamage()` (`:426`) or
    /// `TinkersToolHelper.getDamage(sword)` (`:430`).
    pub item_damage: f32,
    /// `EnchantmentHelper.getDamageBonus(sword, mob.getMobType())` (`:432`), divided by
    /// [`super::nether::ENCHANT_DAMAGE_DIVISOR`] by the rule.
    pub enchant_bonus: f32,
}

impl Default for NetherFacts {
    /// A hired nether miner at the origin: not travelling, no portal, no recipe, no
    /// food, no tool, an empty sword slot and full health — the shortest way to write
    /// one fact.
    ///
    /// "No tool" is the default because `checkForToolOrWeapon` runs before anything
    /// else a trip needs (`:248-258`); a test that wants a trip must put the five tools
    /// back, which makes the default the *refusal* case rather than the happy one.
    fn default() -> NetherFacts {
        NetherFacts {
            at: Pos::new(0, 64, 0),
            travelling: false,
            travel_target: None,
            portal: None,
            portal_frame_valid: false,
            shall_close_portal_on_return: false,
            ready_for_trip: true,
            hut_recipe: Some(NetherRecipe::ready()),
            fulfillable_recipe: true,
            food_in_inventory: 16,
            food_in_building: 0,
            missing_tools: Vec::new(),
            walking: false,
            craft_check: CraftCheck::Craft,
            crafter_busy: false,
            worker_dead: false,
            secondary_skill: 1,
            sword: None,
            health: 20.0,
            max_health: 20.0,
            saturation: 20.0,
            sat_limit_research: 0.0,
            regeneration_research: 0.0,
            blocks: WorkBlocks::new(),
        }
    }
}

/// Everything `EntityAIWorkBeekeeper` reads that the base facts do not carry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BeekeeperFacts {
    /// `worker.blockPosition()`: where the walk to the hive or to the bee is measured
    /// from (`:364`, `:428`).
    pub at: Pos,
    /// `building.getBuildingLevel()` (`:134`, `:756`): the bee budget is
    /// `level * BEES_PER_LEVEL`.
    pub building_level: i32,
    /// `building.getHarvestTypes()` (`:164`), default honeycomb.
    pub harvest_type: HarvestType,
    /// `building.getSetting(BuildingBeekeeper.BREEDING).getValue()` (`:214`, `:443`).
    pub breeding_enabled: bool,
    /// The `BUILDING_FLOWER_LIST` item list is empty (`:213-219`).
    pub flower_list_empty: bool,
    /// How many flowers of that list the worker carries (`:456`).
    pub flowers_in_inventory: u32,
    /// `InventoryUtils.getCountFromBuilding(building, flowersModule.getList())` (`:457`).
    pub flowers_in_building: u32,
    /// `hasWorkerOpenRequestsOfType(citizenId, StackList.class)` (`:459`): a flower
    /// request is already out.
    pub has_flower_request: bool,
    /// `equipTool(..., shears)` can find one (`:345`, `:166`), with
    /// `TOOL_LEVEL_WOOD_OR_GOLD` and the hut's maximum level (`:526-527`).
    pub has_shears: bool,
    /// `equipItem(..., new ItemStack(Items.GLASS_BOTTLE))` can find one (`:352`, `:384`).
    pub has_bottle: bool,
    /// `equipBreedItem(MAIN_HAND)` can put a flower of the hut's list in the worker's
    /// hand (`:559-569`): `findFirstSlotInItemHandlerWith(inventory, flowersModule
    /// ::isItemInList)` returned a slot (`:565`).
    pub has_flower: bool,
    /// `getBeesInHives()` (`:144-154`): the sum of every hive's occupant count.
    pub bees_in_hives: i32,
    /// `building.getHives()` (`:316`), in the order the hut lists them.
    pub hives: Vec<Hive>,
    /// `searchForAnimals(world, building)` (`:228`, `:270`): every bee within
    /// `HIVE_BEE_RADIUS` of a hive.
    pub bees: Vec<Bee>,
    /// `getPrimarySkillLevel()` (`:399`): how likely the bees are to be angered.
    pub primary_skill: i32,
    /// `getSecondarySkillLevel()` (`:202`): how short the decision delay is.
    pub secondary_skill: i32,
    /// **Not a Java call.** `worker.getRandom().nextDouble()` in `harvestHoney`
    /// (`:400`), which decides whether the bees come out angry.
    pub anger_draw: f64,
    /// **Not a Java call.** `worker.getRandom().nextInt(400)` for
    /// `setRemainingPersistentAngerTime(400 + ...)` (`:409`).
    pub anger_roll: u32,
}

impl BeekeeperFacts {
    /// The apiary's bee budget: `building.getBuildingLevel() * BEES_PER_LEVEL`
    /// (`:134`).
    pub fn max_bees(&self) -> i32 {
        self.building_level * super::beekeeper::BEES_PER_LEVEL
    }

    /// How many flowers the worker and the hut can offer (`:456-457`).
    pub fn flower_count(&self) -> u32 {
        self.flowers_in_inventory + self.flowers_in_building
    }
}

/// The trade half of a work scene.
///
/// A caller that hands the nether miner's AI a beekeeper's facts answers
/// [`WorkRefusal::NotThisTrade`] on its first tick instead of pretending the hut is
/// missing a portal: the difference between "this miner has no portal today" and "these
/// facts are another trade's" must not be lost.
#[derive(Clone, Debug, PartialEq)]
pub enum WorkJob {
    /// The nether miner.
    Nether(NetherFacts),
    /// The beekeeper.
    Beekeeper(BeekeeperFacts),
}

impl WorkJob {
    /// The facts as a nether miner's, `None` for another trade.
    pub fn nether(&self) -> Option<&NetherFacts> {
        match self {
            WorkJob::Nether(facts) => Some(facts),
            _ => None,
        }
    }

    /// The facts as a beekeeper's, `None` for another trade.
    pub fn beekeeper(&self) -> Option<&BeekeeperFacts> {
        match self {
            WorkJob::Beekeeper(facts) => Some(facts),
            _ => None,
        }
    }
}

impl Default for WorkJob {
    /// A nether miner's facts, empty. Not "no trade": that would need a third variant,
    /// and `JobFacts::Unported` already says it one module up — here the default is the
    /// trade whose table has a portal to miss.
    fn default() -> WorkJob {
        WorkJob::Nether(NetherFacts::default())
    }
}

/// One stack `leaveForNether` put in the job's crafted queue.
///
/// Java reference: `job.addCraftedResultsList(result)` (`EntityAIWorkNether.java:359`)
/// after `currentRecipeStorage.fullfillRecipeAndCopy(...)` (`:353`), shuffled by
/// `Collections.shuffle(result, citizenData.getRandom())` (`:358`). `stayInNether`
/// then walks the queue three ways (`:382-562`).
#[derive(Clone, Debug, PartialEq)]
pub enum CraftedResult {
    /// An `ItemAdventureToken` (`:384`) carrying a fight, when `TAG_DAMAGE` is set
    /// (`:389-390`).
    Adventure(AdventureToken),
    /// A `BlockItem` (`:516-553`): its loot table is rolled once per item, with a tool.
    Block {
        /// `currStack.getCount()` (`:525`).
        count: u32,
        /// `findTool(...).getItem() instanceof TieredItem` (`:521`): without a tiered
        /// tool the whole stack costs one `TICK_DELAY` and drops nothing (`:549-553`).
        has_tool: bool,
    },
    /// Any other item (`:555-560`): copied straight to the processed queue.
    Plain {
        /// `currStack.getCount()` (`:559`).
        count: u32,
    },
}

/// The tag half of an adventure token, as `stayInNether` reads it.
///
/// Java reference: `EntityAIWorkNether.java:384-511`.
#[derive(Clone, Debug, PartialEq)]
pub struct AdventureToken {
    /// `tag.getFloat(TAG_DAMAGE)` (`:406`), `None` when the tag has no fight.
    pub damage: Option<f32>,
    /// `tag.contains(TAG_ENTITY_TYPE)` → `EntityType.byString(...).orElse(ZOMBIE)`
    /// (`:397-401`): the mob's name, `None` for the fallback zombie.
    pub mob_type: Option<&'static str>,
    /// `mob.getHealth()` (`:403`): the health the fight has to chew through.
    pub mob_health: f32,
    /// `tag.getInt(TAG_XP_DROPPED)` (`:509`).
    pub xp_dropped: u32,
    /// What the fight and the mob's loot table produced (`:494-500`), filled by the
    /// caller: the port does not roll a loot table, and a token without a fight
    /// (`damage == None`) has no loot at all.
    pub loot: Vec<ProcessedItem>,
}

impl Default for AdventureToken {
    /// A token with no fight, no xp tag and no loot — the `if (currStack.hasTag())` case
    /// that `:386` skips.
    fn default() -> AdventureToken {
        AdventureToken {
            damage: None,
            mob_type: None,
            mob_health: 0.0,
            xp_dropped: 0,
            loot: Vec::new(),
        }
    }
}

/// One stack the job has finished with, waiting to be put in the worker's inventory.
///
/// Java reference: `job.addProcessedResultsList(...)` (`EntityAIWorkNether.java:497`,
/// `:531`, `:557`) and the transfer loop at `:573-581`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessedItem {
    /// `item.getCount()` (`:579`).
    pub count: u32,
}

/// What the nether job carries between two ticks — the two queues and the flag.
///
/// Java reference: `JobNetherWorker.craftedResults` (`JobNetherWorker.java:30`),
/// `.processedResults` (`:35`) and `.citizenInNether` (`:40`, `:147-158`); the recipe
/// slot is `AbstractEntityAICrafting.currentRecipeStorage`
/// (`AbstractEntityAICrafting.java:98`), which `EntityAIWorkNether` sets at `:263` and
/// clears at `:301`, `:648`, `:711`.
#[derive(Clone, Debug, PartialEq)]
pub struct NetherMemory {
    /// `citizenInNether` (`JobNetherWorker.java:40`): written `true` at
    /// `EntityAIWorkNether.java:348` and `false` at `:199`, `:326`, `:646`.
    pub in_nether: bool,
    /// `currentRecipeStorage != null` (`EntityAIWorkNether.java:260`).
    pub recipe_set: bool,
    /// `job.getCraftedResults()` (`:380`), in the order the shuffle left them.
    pub crafted: Vec<CraftedResult>,
    /// `job.getProcessedResults()` (`:568`).
    pub processed: Vec<ProcessedItem>,
    /// `citizenData.getJobStatus()` — `setJobStatus` is called at `:255`, `:266`,
    /// `:284`, `:302`, `:327`, `:363`, `:368`, `:644`; the building screen shows it
    /// (`api/entity/ai/JobStatus.java:3-17`).
    pub job_status: JobStatus,
}

impl Default for NetherMemory {
    /// A nether miner that has never left: at home, no recipe, both queues empty, and
    /// `IDLE` on the building screen like a freshly hired citizen.
    fn default() -> NetherMemory {
        NetherMemory {
            in_nether: false,
            recipe_set: false,
            crafted: Vec::new(),
            processed: Vec::new(),
            job_status: JobStatus::Idle,
        }
    }
}

/// What the beekeeper carries between two ticks.
///
/// Java reference: `JobBeekeeper.counter` (`JobBeekeeper.java:21`, capped at `100` by
/// `tickNoBees` `:50-56` and cleared by `resetCounter` `:61-64`) and
/// `EntityAIWorkBeekeeper.lastHarvestedBottle` (`:98`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BeekeeperMemory {
    /// `counter` (`JobBeekeeper.java:21`).
    pub counter: i32,
    /// `lastHarvestedBottle` (`EntityAIWorkBeekeeper.java:98`): written `false` after a
    /// comb harvest (`:382`) and `true` after a bottle one (`:396`).
    pub last_harvested_bottle: bool,
}

/// All the memory one work scene carries: the base class's, plus each trade's.
///
/// [`Memory`] is `crate::ai::worker`'s and is **reused, not copied**: `setDelay` and
/// `hasNotDelayed` are the same two fields for every trade
/// (`AbstractEntityAIBasic.java:397-400`, `:1441-1451`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkMemory {
    /// `AbstractEntityAIBasic.delay` and `.hasDelayed` (`:121`, `:126`).
    pub base: Memory,
    /// What the nether miner keeps.
    pub nether: NetherMemory,
    /// What the beekeeper keeps.
    pub beekeeper: BeekeeperMemory,
}

/// The whole scene one work decision reads: the base facts, the trade's facts, and the
/// AI's own memory.
///
/// [`WorldFacts`] is *the same value* the `crate::ai` tables take — nothing is copied
/// out of it — so a caller that also ticks `crate::ai::worker::JobAi` can hand the one it
/// already filled to both.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkScene {
    /// The citizen, its inventory, its hut and its blocks (`crate::ai::facts`).
    pub facts: WorldFacts,
    /// This trade's own observations.
    pub job: WorkJob,
    /// What the AI remembers between ticks.
    pub memory: WorkMemory,
    /// The last refusal, cleared at the top of every tick.
    pub refusal: Option<WorkRefusal>,
}

impl WorkScene {
    /// A scene on `world` and `job`, with fresh memory.
    pub fn new(facts: WorldFacts, job: WorkJob) -> WorkScene {
        WorkScene {
            facts,
            job,
            memory: WorkMemory::default(),
            refusal: None,
        }
    }

    /// The nether miner's facts, if this is a nether scene.
    pub fn nether(&self) -> Option<&NetherFacts> {
        self.job.nether()
    }

    /// The beekeeper's facts, if this is a beekeeper scene.
    pub fn beekeeper(&self) -> Option<&BeekeeperFacts> {
        self.job.beekeeper()
    }

    /// Record a refusal for this tick.
    pub fn refuse(&mut self, refusal: WorkRefusal) {
        self.refusal = Some(refusal);
    }
}

impl fmt::Display for HarvestType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.setting())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unrecorded_position_is_air_and_an_unloaded_one_is_not() {
        // Java reads the level (`EntityAIWorkNether.java:341`); this port reads what a
        // hook wrote down, and silence is "nothing here".
        let mut blocks = WorkBlocks::new();
        assert_eq!(blocks.at(Pos::new(1, 2, 3)), WorkBlock::Air);
        blocks.set(Pos::new(1, 2, 3), WorkBlock::Portal);
        assert!(blocks.at(Pos::new(1, 2, 3)).is_portal());
        assert_eq!(blocks.len(), 1);
        // The second write of the same position replaces, it does not append.
        blocks.set(Pos::new(1, 2, 3), WorkBlock::Solid);
        assert_eq!(blocks.len(), 1);
        assert!(!blocks.at(Pos::new(1, 2, 3)).is_portal());
        // Unloaded is a separate answer: `WorldUtil.isBlockLoaded`
        // (`EntityAIWorkComposter.java:115`).
        blocks.mark_unloaded(Pos::new(4, 5, 6));
        assert!(!blocks.is_loaded(Pos::new(4, 5, 6)));
        assert!(blocks.is_loaded(Pos::new(1, 2, 3)));
    }

    #[test]
    fn a_hive_is_full_at_level_five_and_not_at_four() {
        // `BeehiveBlockEntity.getHoneyLevel(world.getBlockState(pos)) >= 5`
        // (`EntityAIWorkBeekeeper.java:321`).
        assert!(Hive::full(Pos::new(0, 70, 0)).is_full());
        assert!(!Hive {
            honey_level: 4,
            ..Hive::full(Pos::new(0, 70, 0))
        }
        .is_full());
        assert!(!Hive::empty(Pos::new(0, 70, 0)).is_full());
    }

    #[test]
    fn a_bee_on_cooldown_passes_the_baby_test_and_fails_the_breeding_one() {
        // `:274` filters `!isBaby()`, `:287` demands `getAge() == 0`. A bee with a
        // positive age is neither a baby nor ready — the port keeps both questions.
        let grown = Bee::grown(Pos::new(1, 65, 0));
        let baby = Bee::baby(Pos::new(2, 65, 0));
        let cooling = Bee {
            age: 40,
            ..grown
        };
        assert!(!grown.is_baby() && grown.will_breed());
        assert!(baby.is_baby() && !baby.will_breed());
        assert!(!cooling.is_baby() && !cooling.will_breed());
    }

    #[test]
    fn the_apiary_defaults_to_honeycomb() {
        // `BuildingModules.java:162` builds the setting as
        // `(HONEYCOMB, HONEY, BOTH)` and `StringSetting.java:56` starts at index 0.
        assert_eq!(HarvestType::default(), HarvestType::Honeycomb);
        assert_eq!(
            HarvestType::default().setting(),
            "com.minecolonies.core.apiary.setting.honeycomb"
        );
        // The three tests `EntityAIWorkBeekeeper` makes on the setting.
        assert!(HarvestType::Honeycomb.needs_shears());
        assert!(!HarvestType::Honeycomb.needs_bottles());
        assert!(!HarvestType::Honey.needs_shears());
        assert!(HarvestType::Honey.needs_bottles());
        assert!(HarvestType::Both.needs_shears() && HarvestType::Both.needs_bottles());
        assert!(HarvestType::Both.is_both() && !HarvestType::Honey.is_both());
    }

    #[test]
    fn the_bee_budget_and_the_flower_count_are_sums() {
        // `hasMaxAnimals`: `(numOfAnimals + numOfBeesInHive) >= level * BEES_PER_LEVEL`
        // (`:136`), and `flowerCount = inventory + building` (`:456-457`).
        let facts = BeekeeperFacts {
            building_level: 3,
            flowers_in_inventory: 1,
            flowers_in_building: 4,
            ..BeekeeperFacts::default()
        };
        assert_eq!(facts.max_bees(), 3 * crate::ai_work_07::beekeeper::BEES_PER_LEVEL);
        assert_eq!(facts.flower_count(), 5);
    }

    #[test]
    fn the_nether_defaults_are_the_refusal_case() {
        // The default has every trip input, no portal and no tool: a test that wants a
        // trip must say so, and `MissingTool` is reachable from a bare default.
        let facts = NetherFacts::default();
        assert!(facts.missing_tools.is_empty());
        assert!(facts.portal.is_none());
        assert!(!facts.shall_close_portal_on_return);
        assert!(facts.ready_for_trip);
        assert_eq!(facts.health, facts.max_health);
        // `WorkJob::default` is a nether scene, so `WorkRefusal::NotThisTrade` is what
        // a beekeeper machine gets.
        assert!(WorkJob::default().nether().is_some());
        assert!(WorkJob::default().beekeeper().is_none());
    }
}
