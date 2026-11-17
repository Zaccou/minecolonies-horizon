//! The nether miner: a hut that sends a citizen through a portal, a queue of
//! adventures it works through, and a portal it opens and closes.
//!
//! Java reference: `core/entity/ai/workers/production/EntityAIWorkNether.java` — the
//! constructor's `registerTargets` (`:101-107`), `decide` (`:182-312`),
//! `leaveForNether` (`:317-370`), `stayInNether` (`:375-596`), `xpOnDrop` (`:599-626`),
//! `returnFromNether` (`:631-652`), `openPortal` (`:657-685`), `closePortal`
//! (`:690-713`), `checkAndRequestFood` (`:936-950`), `checkHeal` (`:955-980`) and
//! `checkAndRequestArmor` (`:844-934`), on top of its parent
//! `AbstractEntityAICrafting` (`:164-169`, `:724-727`).
//!
//! ## The table this adds
//!
//! | # | State | Rule | Rate | Java |
//! | ---: | --- | --- | ---: | --- |
//! | 1 | `START_WORKING` | `decide` (overridden) | 5 | `AbstractEntityAICrafting.java:166` |
//! | 2 | `NETHER_LEAVE` | `leaveForNether` | 40 | `EntityAIWorkNether.java:102` |
//! | 3 | `NETHER_AWAY` | `stayInNether` | 40 | `:103` |
//! | 4 | `NETHER_RETURN` | `returnFromNether` | 40 | `:104` |
//! | 5 | `NETHER_OPENPORTAL` | `openPortal` | 40 | `:105` |
//! | 6 | `NETHER_CLOSEPORTAL` | `closePortal` | 40 | `:106` |
//!
//! **`decide` is the whole job.** Sixteen branches, in Java's order, each of which is a
//! named refusal here ([`WorkRefusal`]) instead of a bare `return getState()`: travelling
//! or away, then the crafting half, then food, then the recipe's inputs, then the portal,
//! then the five tools, then the recipe itself, then the trip counter, then the walk to
//! the hut, then the inventory, then `checkForItems`, then the trip. [`decide`] is that
//! list and nothing else.
//!
//! **The fight is not a state.** `stayInNether`'s per-round loop (`:409-474`) needs a
//! `LivingEntity`, a `DamageSource` and two `worker.getRandom().nextBoolean()` per round;
//! it is the caller's, and this module gives it the four numbers the loop computes:
//! [`incoming_damage`] (`:406-407`), [`attack_damage`] (`:414-432`), [`check_heal`]
//! (`:955-980`) and [`heal_saturation_cost`] (`:460-465`). What *is* ported is the queue
//! accounting around it ([`stay_in_nether`]), which is the part that decides where the
//! AI goes next.
//!
//! **Two Java null bugs, not reproduced.** `returnFromNether` (`:634`) and `closePortal`
//! (`:692-695`) call `world.getBlockState(building.getPortalLocation())` without checking
//! that the hut has a `portal` tag: a hut saved without one would throw. The port treats
//! a missing portal as "no portal block" and the state machine keeps going — the
//! divergence is deliberate and named here rather than hidden.

use crate::ai::facts::WorldFacts;
use crate::ai::state::{Machine, WorkerState};
use crate::ai::worker::JobStatus;
use crate::jobs::tool::ToolKind;

use super::facts::{CraftCheck, CraftedResult, NetherFacts, ProcessedItem, SwordDamage, WorkMemory};
use super::state::{WorkRefusal, WorkState};
use super::{add_target, at_hut, work_action, work_action_trade, Decision};

/// `EntityAIWorkNether.TICK_DELAY` (`:71`): the rate of all five nether targets
/// (`:102-106`), and the cost of one mined item in `stayInNether` (`:543`, `:552`,
/// `:559`).
pub const TICK_DELAY: u32 = 40;

/// `EntityAIWorkNether.SECONDARY_DAMAGE_REDUCTION` (`:76`): what one point of the
/// secondary skill takes off an incoming hit (`:407`).
pub const SECONDARY_DAMAGE_REDUCTION: f32 = 0.005;

/// The travelling time the vault gives per crafted item (`:176`): twenty seconds each,
/// i.e. 400 ticks.
pub const VAULT_TICKS_PER_ITEM: u32 = 400;

/// `setDelay(60)` when an input of the hut's recipe is missing (`:234`).
pub const DELAY_MISSING_INPUTS: u32 = 60;

/// `setDelay(120)` when the hut has no portal (`:242`).
pub const DELAY_NO_PORTAL: u32 = 120;

/// `setDelay(60)` when a tool or the recipe is missing (`:256`, `:303`).
pub const DELAY_STUCK: u32 = 60;

/// `setDelay(120)` when the hut already used its trip (`:285`).
pub const DELAY_TRIP_NOT_READY: u32 = 120;

/// `EntityAIWorkNether` has no `setDelay(2)` anywhere: the two-tick wait belongs to the
/// beekeeper (`EntityAIWorkBeekeeper.java:189`).

/// The food stock `checkAndRequestFood` wants in the inventory (`:938`): sixteen edibles
/// of the hut's menu.
pub const FOOD_STOCK: u32 = 16;

/// `BuildingNetherWorker.MAX_PER_PERIOD` (`:61`): one trip a day, until a research says
/// otherwise (`:258-262`).
pub const MAX_PER_PERIOD: u32 = 1;

/// `EnchantmentHelper.getDamageBonus(sword, mob.getMobType()) / 2.5` (`:432`).
pub const ENCHANT_DAMAGE_DIVISOR: f32 = 2.5;

/// `Constants.DOUBLE`'s cousin here: the saturation one point of healing costs is
/// `healAmount * 0.25` (`:460-465`).
pub const HEAL_SATURATION_FACTOR: f64 = 0.25;

/// `2 * (1.0 + REGENERATION)` — the heal at full saturation (`:965`).
pub const HEAL_FULL_SATURATION: f32 = 2.0;

/// `1 * (1.0 + REGENERATION)` — the heal between the two saturation bounds (`:973`).
pub const HEAL_NORMAL: f32 = 1.0;

/// `addExperience(0.2)` per looted stack (`:578`).
pub const XP_PER_LOOT: f64 = 0.2;

/// `GuardConstants.BASE_PHYSICAL_DAMAGE` (`api/util/constant/GuardConstants.java:173`):
/// the damage of one blow before the sword (`:414`).
pub const BASE_PHYSICAL_DAMAGE: f32 = 3.0;

/// `ICitizenData.MAX_SATURATION` (`api/colony/ICitizenData.java:30`), which
/// `CitizenConstants.FULL_SATURATION` is (`api/util/constant/CitizenConstants.java:53`).
pub const FULL_SATURATION: f64 = 60.0;

/// `CitizenConstants.LOW_SATURATION` (`:48`): below this, no healing at all (`:967-970`).
pub const LOW_SATURATION: f64 = 6.0;

/// `CitizenConstants.AVERAGE_SATURATION` (`:43`): below this, the fight eats (`:469`).
pub const AVERAGE_SATURATION: f64 = 10.0;

/// `Mesh` of the five armour tiers `EntityAIWorkNether`'s constructor builds
/// (`:110-114`), from `GuardGearBuilder.buildGearForLevel(min, max, levelRange,
/// buildingLevelRange)` (`api/entity/ai/workers/util/GuardGearBuilder.java:37-42`).
///
/// Each entry is "armour from `min_armor` to `max_armor`, for a hut of
/// `min_building`…`max_building`". The armour levels are
/// `EquipmentLevelConstants.ARMOR_LEVEL_*` (`:32-62`) and the ranges
/// `GuardConstants.*_BUILDING_LEVEL_RANGE` (`:199-203`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArmorTier {
    /// `minArmorLevel` (`:110-114`).
    pub min_armor: i32,
    /// `maxArmorLevel`, `ARMOR_LEVEL_MAX` being `Integer.MAX_VALUE`
    /// (`EquipmentLevelConstants.java:62`).
    pub max_armor: i32,
    /// The lower building level of the range (`GuardConstants.java:199-203`).
    pub min_building: i32,
    /// The upper one, inclusive (`:756`).
    pub max_building: i32,
}

/// The five lists of `:110-114`, in the order the constructor adds them.
pub const ARMOR_TIERS: [ArmorTier; 5] = [
    // `buildGearForLevel(IRON, MAX, LEATHER_BUILDING_LEVEL_RANGE, DIA_BUILDING_LEVEL_RANGE)`
    // (`:110`) — the fourth argument is the *building* range, `GuardConstants.java:203`.
    ArmorTier {
        min_armor: 3,
        max_armor: i32::MAX,
        min_building: 4,
        max_building: 5,
    },
    // `buildGearForLevel(CHAIN, DIAMOND, LEATHER…, DIA…)` (`:111`).
    ArmorTier {
        min_armor: 2,
        max_armor: 4,
        min_building: 4,
        max_building: 5,
    },
    // `buildGearForLevel(LEATHER, IRON, LEATHER…, IRON…)` (`:112`).
    ArmorTier {
        min_armor: 0,
        max_armor: 3,
        min_building: 3,
        max_building: 4,
    },
    // `buildGearForLevel(LEATHER, CHAIN, LEATHER…, CHAIN…)` (`:113`).
    ArmorTier {
        min_armor: 0,
        max_armor: 2,
        min_building: 2,
        max_building: 3,
    },
    // `buildGearForLevel(LEATHER, GOLD, LEATHER…, GOLD…)` (`:114`).
    ArmorTier {
        min_armor: 0,
        max_armor: 1,
        min_building: 1,
        max_building: 2,
    },
];

/// What `checkAndRequestFood` answered (`:936-950`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three exits of `checkAndRequestFood`
pub enum FoodOutcome {
    /// Sixteen edibles in the inventory: nothing to do (`:938-942`).
    Enough,
    /// The hut can stock the worker: `GATHERING_REQUIRED_MATERIALS` (`:944-948`).
    Gather,
    /// Neither: the worker leaves with what it has (`:949`).
    Nothing,
}

/// `checkAndRequestFood` (`:936-950`), condition by condition.
///
/// `:946` also sets `needsCurrently` to `(menu::contains, 16)`, which is
/// [`crate::ai::worker::Memory::need`] — a memory write, done by the caller that owns the
/// base table's `GATHERING_REQUIRED_MATERIALS` (`AbstractEntityAIBasic.java:242`).
pub fn check_and_request_food(nether: &NetherFacts) -> FoodOutcome {
    if nether.food_in_inventory >= FOOD_STOCK {
        return FoodOutcome::Enough; // :938-942
    }
    if nether.food_in_building >= 1 {
        return FoodOutcome::Gather; // :944-948
    }
    FoodOutcome::Nothing // :949
}

/// The first tool of the job's five that the worker does not hold (`:248-252`).
///
/// The order is `crate::jobs::job::Job::tools()`'s, which is the order the five
/// `checkForToolOrWeapon` calls are written in — axe, pickaxe, shovel, sword, flint and
/// steel. Java ORs the five booleans and cannot say which one is missing; a player can,
/// so this port names it.
pub fn first_missing_tool(nether: &NetherFacts) -> Option<ToolKind> {
    for tool in crate::jobs::job::Job::NetherWorker.tools() {
        if nether.missing_tools.contains(tool) {
            return Some(*tool);
        }
    }
    // A tool reported missing that the job does not state is still missing: report it
    // rather than swallowing it.
    nether.missing_tools.first().copied()
}

/// `building.isReadyForTrip()` (`:282`) — `currentTrips < getMaxPerPeriod()`
/// (`BuildingNetherWorker.java:215-228`, `:61`).
pub fn is_ready_for_trip(current_trips: u32) -> bool {
    current_trips < MAX_PER_PERIOD
}

/// `decide()` (`:182-312`), branch by branch, in Java's order.
///
/// The order **is** the rule: a worker that is away never checks its recipe, a worker
/// with no portal never checks its tools, and a worker whose trip is used waits instead
/// of walking.
pub fn decide(facts: &WorldFacts, nether: &NetherFacts, memory: &mut WorkMemory) -> Decision {
    use WorkState::Base;
    let idle = Base(WorkerState::Idle);

    // :186-189 — "we don't spawn an entity if we are traveling".
    if nether.travelling {
        return Decision::go(WorkState::NetherAway).because(WorkRefusal::Travelling);
    }
    if memory.nether.in_nether {
        return Decision::go(WorkState::NetherAway).because(WorkRefusal::InNether);
    }
    // :199 — `job.setInNether(false)`; already false here, kept as the write Java makes.
    memory.nether.in_nether = false;

    // :201-206 — the crafting parent's answer, when it is neither `IDLE` nor
    // `START_WORKING`. That state is `src/crafting`'s and this port does not name it.
    if nether.crafter_busy {
        return Decision::refuse(WorkRefusal::CraftingHalfOwnsIt);
    }

    // :210 — `checkAndRequestArmor()`: no return value, only requests. The gear table is
    // [`ARMOR_TIERS`] and the caller raises the requests.

    // :212-216 — food, before anything about the portal.
    if check_and_request_food(nether) == FoodOutcome::Gather {
        return Decision::go(Base(WorkerState::GatheringRequiredMaterials));
    }

    // :219-236 — the inputs of the hut's first recipe.
    if let Some(recipe) = &nether.hut_recipe {
        if !recipe.inputs_available {
            memory.base.delay = DELAY_MISSING_INPUTS as i32; // :234
            return Decision::go(idle).because(WorkRefusal::MissingInputs);
        }
    }

    // :238-244 — the portal tag.
    let Some(portal) = nether.portal else {
        memory.base.delay = DELAY_NO_PORTAL as i32; // :242
        return Decision::go(idle).because(WorkRefusal::NoPortal);
    };

    // :248-258 — five tools, one refusal.
    if let Some(tool) = first_missing_tool(nether) {
        memory.nether.job_status = JobStatus::Stuck; // :255
        memory.base.delay = DELAY_STUCK as i32; // :256
        return Decision::go(idle).because(WorkRefusal::MissingTool(tool));
    }

    // :260-279 — the recipe the trip is made with.
    if !memory.nether.recipe_set {
        if nether.fulfillable_recipe {
            memory.nether.recipe_set = true; // :263
        }
        if nether.ready_for_trip {
            memory.nether.job_status = JobStatus::Stuck; // :264-267
        }
        if !memory.nether.recipe_set
            && nether.shall_close_portal_on_return
            && nether.blocks.at(portal).is_portal()
        {
            return Decision::go(WorkState::NetherClosePortal); // :269-276
        }
        return Decision::refuse(WorkRefusal::NoRecipe); // :278
    }

    // :282-287 — the hut's trip budget.
    if !nether.ready_for_trip {
        memory.nether.job_status = JobStatus::Idle; // :284
        memory.base.delay = DELAY_TRIP_NOT_READY as i32; // :285
        return Decision::go(idle).because(WorkRefusal::TripNotReady);
    }

    // :288-291 — `walkTo != null || !walkToBuilding()`.
    if nether.walking || !at_hut(facts) {
        return Decision::refuse(WorkRefusal::NotAtHut);
    }

    // :292-295 — `!worker.getInventoryCitizen().hasSpace()`.
    if !facts.inventory.has_space {
        return Decision::go(Base(WorkerState::InventoryFull)).because(WorkRefusal::NoSpace);
    }

    // :297-309 — `checkForItems`.
    match nether.craft_check {
        CraftCheck::RecipeLost => {
            memory.nether.recipe_set = false; // :301
            memory.nether.job_status = JobStatus::Stuck; // :302
            memory.base.delay = DELAY_STUCK as i32; // :303
            Decision::go(idle).because(WorkRefusal::RecipeLost)
        }
        CraftCheck::GatherMaterials => Decision::go(Base(WorkerState::GatheringRequiredMaterials)),
        // :311
        CraftCheck::Craft => Decision::go(WorkState::NetherLeave),
    }
}

/// `leaveForNether()` (`:317-370`).
///
/// The two writes Java makes that this port cannot — `building.recordTrip()` (`:347`) and
/// `expeditionLog.setStatus(...)` (`:333`, `:350`) — are the caller's, and the crafted
/// queue is filled by it too ([`CraftedResult`]).
pub fn leave_for_nether(
    facts: &WorldFacts,
    nether: &NetherFacts,
    memory: &mut WorkMemory,
) -> Decision {
    use WorkState::Base;
    if !facts.inventory.has_space {
        return Decision::go(Base(WorkerState::InventoryFull)).because(WorkRefusal::NoSpace);
    }
    if !memory.nether.recipe_set {
        memory.nether.in_nether = false; // :326
        memory.nether.job_status = JobStatus::Stuck; // :327
        return Decision::go(Base(WorkerState::Idle)).because(WorkRefusal::NoRecipe);
    }
    // :331-334 — the expedition log is reset and marked STARTING.
    let Some(portal) = nether.portal else {
        memory.nether.job_status = JobStatus::Stuck; // :368
        return Decision::go(Base(WorkerState::Idle)).because(WorkRefusal::NoPortal);
    };
    if nether.blocks.at(portal).is_portal() {
        // :343-346 — `!walkToWorkPos(portal)`.
        if !arrived(nether, portal) {
            return Decision::stay();
        }
        memory.nether.in_nether = true; // :348
        memory.nether.job_status = JobStatus::Working; // :363
        // :353-360 — the recipe is fulfilled into the crafted queue, shuffled, by the
        // caller. :362 — `goToVault()` discards the entity for
        // `crafted.size() * VAULT_TICKS_PER_ITEM` ticks.
        return Decision::go(WorkState::NetherAway);
    }
    // :366 — the frame is there but the portal is out.
    Decision::go(WorkState::NetherOpenPortal)
}

/// `stayInNether()` (`:375-596`), as queue accounting.
///
/// Three queues in Java's order: the crafted results first (`:380-566`), then the
/// processed ones (`:568-591`), then home (`:593-595`). While either queue still holds
/// something the answer is `getState()`, i.e. stay in `NETHER_AWAY`.
///
/// **What the caller does.** For each [`CraftedResult::Adventure`] with a fight it runs
/// the round loop (`:409-474`) and rolls the mob's loot table (`:494-500`), then writes
/// the result into the token's [`loot`](super::facts::AdventureToken::loot) — which is
/// why a death is reported by `worker_dead` *after* the fight. For every processed item
/// it moves the stack and hands out [`XP_PER_LOOT`] (`:573-581`).
pub fn stay_in_nether(nether: &NetherFacts, memory: &mut WorkMemory) -> Decision {
    use WorkState::Base;
    if !memory.nether.crafted.is_empty() {
        let crafted = core::mem::take(&mut memory.nether.crafted);
        let mut delay = 0;
        for result in crafted {
            match result {
                // :384-512 — an adventure token.
                CraftedResult::Adventure(token) => {
                    if token.damage.is_some() {
                        if nether.worker_dead {
                            // :479-490 — the expedition is over, the queues are dropped.
                            memory.nether.processed.clear();
                            return Decision::go(Base(WorkerState::Idle))
                                .because(WorkRefusal::KilledInNether);
                        }
                        // :494-500 — the loot the caller rolled for this mob.
                        memory.nether.processed.extend(token.loot);
                    }
                    // :507-510 — `TAG_XP_DROPPED`: an xp grant, no queue, no delay.
                }
                // :516-553 — a block item, mined once per item when a tiered tool exists.
                CraftedResult::Block { count, has_tool } => {
                    if has_tool {
                        for _ in 0..count {
                            memory.nether.processed.push(ProcessedItem { count: 1 });
                        }
                        delay = TICK_DELAY * count.max(1); // :543
                    } else {
                        // :549-553 — no tool, one tick of work, nothing mined.
                        delay = TICK_DELAY;
                    }
                }
                // :555-560 — anything else copied straight over.
                CraftedResult::Plain { count } => {
                    memory.nether.processed.push(ProcessedItem { count });
                    delay = TICK_DELAY * count.max(1); // :559
                }
            }
        }
        // :561 — inside the loop, so the last stack's cost is the one that stands.
        memory.base.delay = delay as i32;
        return Decision::stay(); // :564-565
    }

    if !memory.nether.processed.is_empty() {
        // :570-590 — dead or alive, the queue is cleared; the difference is whether the
        // items were moved into the inventory and the xp granted, which is the caller's.
        memory.nether.processed.clear();
        return Decision::stay();
    }

    // :593-595
    Decision::go(WorkState::NetherReturn)
}

/// `returnFromNether()` (`:631-652`).
pub fn return_from_nether(
    facts: &WorldFacts,
    nether: &NetherFacts,
    memory: &mut WorkMemory,
) -> Decision {
    use WorkState::Base;
    // :634 — `shallClosePortalOnReturn() && getBlockState(portal).is(NETHER_PORTAL)`.
    // A missing portal reads as "no portal block" here where Java would throw.
    if nether.shall_close_portal_on_return {
        if let Some(portal) = nether.portal {
            if nether.blocks.at(portal).is_portal() {
                return Decision::go(WorkState::NetherClosePortal);
            }
        }
    }
    // :639-642
    if !at_hut(facts) {
        return Decision::refuse(WorkRefusal::NotAtHut);
    }
    memory.nether.job_status = JobStatus::Stuck; // :644
    memory.nether.in_nether = false; // :646
    memory.nether.recipe_set = false; // :648
    Decision::go(Base(WorkerState::InventoryFull)) // :651
}

/// `openPortal()` (`:657-685`).
pub fn open_portal(nether: &NetherFacts, memory: &mut WorkMemory) -> Decision {
    use WorkState::Base;
    let Some(portal) = nether.portal else {
        return Decision::go(Base(WorkerState::StartWorking)); // :684
    };
    if !memory.nether.recipe_set {
        return Decision::go(Base(WorkerState::StartWorking)); // :661
    }
    if !arrived(nether, portal) {
        return Decision::stay(); // :663-666
    }
    if !nether.portal_frame_valid {
        // :671-675 — "can't find the portal".
        return Decision::go(Base(WorkerState::Idle)).because(WorkRefusal::NoPortal);
    }
    if !nether.blocks.at(portal).is_portal() {
        // :679-681 — light it, then leave.
        return Decision::go(WorkState::NetherLeave);
    }
    Decision::go(Base(WorkerState::StartWorking)) // :684
}

/// `closePortal()` (`:690-713`).
pub fn close_portal(nether: &NetherFacts, memory: &mut WorkMemory) -> Decision {
    use WorkState::Base;
    if let Some(portal) = nether.portal {
        if nether.blocks.at(portal).is_portal() {
            if !arrived(nether, portal) {
                return Decision::stay(); // :697-700
            }
            // :702-703 — flint and steel, then air; the caller writes the block.
        }
    }
    if memory.nether.in_nether {
        return Decision::go(WorkState::NetherReturn); // :706-709
    }
    memory.nether.recipe_set = false; // :711
    Decision::go(Base(WorkerState::InventoryFull)) // :712
}

/// `walkToWorkPos(portal)`'s answer: the worker stands within `WOKR_IN_BUILDING_DIST`
/// of the portal (`AbstractEntityAIBasic.java:800-808`).
fn arrived(nether: &NetherFacts, target: crate::ai::facts::Pos) -> bool {
    nether.at.is_living_at_site(target, crate::ai::worker::WOKR_IN_BUILDING_DIST)
}

/// The ores `xpOnDrop` tells apart (`:599-626`), and the bounds it draws between.
///
/// `RandomSource.nextInt(origin, bound)` is exclusive of `bound`, so `nextInt(0, 2)` is
/// `{0, 1}` and `nextInt(3, 7)` is `{3, 4, 5, 6}`. Every block that is not one of the six
/// named ones is worth nothing — including a nether gold ore, whose `nextInt(0, 1)` is
/// always zero (`:624`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: the seven branches of `xpOnDrop`
pub enum NetherOre {
    /// `Blocks.COAL_ORE` (`:602`).
    Coal,
    /// `Blocks.DIAMOND_ORE` (`:606`).
    Diamond,
    /// `Blocks.EMERALD_ORE` (`:610`).
    Emerald,
    /// `Blocks.LAPIS_ORE` (`:614`).
    Lapis,
    /// `Blocks.NETHER_QUARTZ_ORE` (`:618`).
    Quartz,
    /// `Blocks.NETHER_GOLD_ORE` (`:624`).
    Gold,
    /// Anything else (`:625`).
    Other,
}

impl NetherOre {
    /// The `[origin, bound)` pair `nextInt` is called with, as Java writes it.
    pub fn xp_range(self) -> (u32, u32) {
        match self {
            NetherOre::Coal => (0, 2),     // :604
            NetherOre::Diamond => (3, 7),  // :608
            NetherOre::Emerald => (3, 7),  // :612
            NetherOre::Lapis => (2, 5),    // :616
            NetherOre::Quartz => (2, 5),   // :620
            NetherOre::Gold => (0, 1),     // :624
            NetherOre::Other => (0, 0),    // :625 — `0`, not a draw
        }
    }

    /// The block's Java name, for a log line.
    pub fn java_name(self) -> &'static str {
        match self {
            NetherOre::Coal => "coal_ore",
            NetherOre::Diamond => "diamond_ore",
            NetherOre::Emerald => "emerald_ore",
            NetherOre::Lapis => "lapis_ore",
            NetherOre::Quartz => "nether_quartz_ore",
            NetherOre::Gold => "nether_gold_ore",
            NetherOre::Other => "other",
        }
    }
}

/// `xpOnDrop(block)` (`:599-626`), with the draw supplied by the caller.
///
/// `RandomSource.nextInt(origin, bound)` is exclusive of `bound`, which is why
/// [`NetherOre`] carries the bounds rather than a list of values.
pub fn xp_on_drop(ore: NetherOre, draw: u32) -> u32 {
    let (low, high) = ore.xp_range();
    low + (draw % (high - low).max(1))
}

/// One blow's damage on the mob (`:414-432`).
///
/// `BASE_PHYSICAL_DAMAGE + sword damage + enchant bonus / 2.5`, with the sword's two
/// halves coming from the facts; an empty hand contributes nothing (`:422`).
pub fn attack_damage(sword: Option<SwordDamage>) -> f32 {
    let mut damage = BASE_PHYSICAL_DAMAGE; // :414
    if let Some(sword) = sword {
        damage += sword.item_damage; // :426, :430
        damage += sword.enchant_bonus / ENCHANT_DAMAGE_DIVISOR; // :432
    }
    damage
}

/// One blow's damage on the worker, after the secondary skill (`:406-407`).
///
/// `incomingDamage -= incomingDamage * (skill * SECONDARY_DAMAGE_REDUCTION)`: five
/// hundredths of a point of reduction per skill point, so a skill of 200 would make the
/// worker immune (`:407`).
pub fn incoming_damage(raw: f32, secondary_skill: i32) -> f32 {
    let reduction = (secondary_skill.max(0) as f32) * SECONDARY_DAMAGE_REDUCTION;
    raw - raw * reduction
}

/// How much one `checkHeal` call heals (`:955-980`).
///
/// Three branches, in Java's order: below `LOW_SATURATION` nothing at all (`:967-970`),
/// at or above `FULL_SATURATION + SATLIMIT` two points (`:963-966`), otherwise one
/// (`:971-974`). Both nonzero branches are multiplied by `1.0 + REGENERATION`.
///
/// The **zero** branch is the one that surprises: a hurt worker with six saturation is
/// not healed, and the fight's odd rounds are when it eats (`:467-473`).
pub fn check_heal(nether: &NetherFacts) -> f32 {
    if nether.health >= nether.max_health {
        return 0.0; // :959 — the whole method is inside this test
    }
    if nether.saturation < LOW_SATURATION {
        return 0.0; // :967-970
    }
    let regeneration = 1.0 + nether.regeneration_research;
    if nether.saturation >= FULL_SATURATION + nether.sat_limit_research {
        return HEAL_FULL_SATURATION * regeneration as f32; // :963-966
    }
    HEAL_NORMAL * regeneration as f32 // :971-974
}

/// The saturation one heal costs (`:460-465`): `healAmount * 0.25`.
pub fn heal_saturation_cost(heal: f32) -> f64 {
    (heal as f64) * HEAL_SATURATION_FACTOR
}

/// How long the vault keeps the citizen away: `craftedResults.size() * 400` ticks
/// (`:176`), twenty seconds per item.
pub fn vault_travel_ticks(crafted: usize) -> u32 {
    (crafted as u32).saturating_mul(VAULT_TICKS_PER_ITEM)
}

/// The armour lists that apply to a hut of `building_level` (`:850-853`).
///
/// Java walks all five lists and skips an item whose building range does not contain the
/// level; the answer is the same list, filtered.
pub fn gear_for(building_level: i32) -> Vec<ArmorTier> {
    ARMOR_TIERS
        .iter()
        .copied()
        .filter(|tier| building_level >= tier.min_building && building_level <= tier.max_building)
        .collect()
}

work_action!(decide_action, Nether, decide);
work_action!(leave_action, Nether, leave_for_nether);

work_action_trade!(stay_action, Nether, stay_in_nether);
work_action!(return_action, Nether, return_from_nether);
work_action_trade!(open_action, Nether, open_portal);
work_action_trade!(close_action, Nether, close_portal);

/// Register the nether miner's six targets on top of the base table.
///
/// Java's constructor adds five (`:101-107`) and inherits `START_WORKING → decide` from
/// `AbstractEntityAICrafting.java:166`. `IDLE → START_WORKING` is
/// [`super::register_handover`]'s.
pub fn register(machine: &mut Machine<WorkState, super::WorkScene>) {
    // `AbstractEntityAICrafting.java:166` — `new AITarget(START_WORKING, this::decide,
    // STANDARD_DELAY)`; the nether miner overrides `decide` (`:182-312`).
    add_target(
        machine,
        WorkState::Base(WorkerState::StartWorking),
        "decide",
        "AbstractEntityAICrafting.java:166",
        super::STANDARD_DELAY,
        decide_action,
    );
    // `:102-106` — the five nether targets, all at `TICK_DELAY`.
    for (state, rule, java, action) in [
        (
            WorkState::NetherLeave,
            "leaveForNether",
            "EntityAIWorkNether.java:102",
            leave_action as fn(&mut super::WorkScene, WorkState) -> Option<WorkState>,
        ),
        (
            WorkState::NetherAway,
            "stayInNether",
            "EntityAIWorkNether.java:103",
            stay_action,
        ),
        (
            WorkState::NetherReturn,
            "returnFromNether",
            "EntityAIWorkNether.java:104",
            return_action,
        ),
        (
            WorkState::NetherOpenPortal,
            "openPortal",
            "EntityAIWorkNether.java:105",
            open_action,
        ),
        (
            WorkState::NetherClosePortal,
            "closePortal",
            "EntityAIWorkNether.java:106",
            close_action,
        ),
    ] {
        add_target(machine, state, rule, java, TICK_DELAY, action);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::facts::{Pos, WorldFacts};
    use crate::ai::worker::JobStatus;
    use crate::ai_work_07::facts::{
        AdventureToken, CraftedResult, NetherRecipe, ProcessedItem, WorkBlock, WorkJob, WorkScene,
    };
    use crate::ai_work_07::{at_hut, WorkAi, WorkTrade};

    /// A nether scene whose worker stands at its hut, hut at the origin.
    fn new_scene() -> WorkScene {
        let mut world = WorldFacts::new();
        world.worker.at = Pos::new(0, 64, 0);
        world.building.position = Pos::new(0, 64, 0);
        WorkAi::new(WorkTrade::Nether)
            .expect("the nether miner has a table")
            .scene(world)
    }

    /// The same scene with a portal at `1,64,0`, open.
    fn with_portal(scene: &mut WorkScene) {
        let WorkJob::Nether(nether) = &mut scene.job else {
            panic!("a nether scene");
        };
        nether.portal = Some(Pos::new(1, 64, 0));
        nether.portal_frame_valid = true;
        nether.blocks.set(Pos::new(1, 64, 0), WorkBlock::Portal);
        nether.at = Pos::new(0, 64, 0);
    }

    fn nether(scene: &mut WorkScene) -> &mut NetherFacts {
        match &mut scene.job {
            WorkJob::Nether(facts) => facts,
            _ => panic!("a nether scene"),
        }
    }

    #[test]
    fn decide_refuses_a_travelling_worker_before_it_looks_at_anything() {
        // `:186-189`: travelling or in the nether answers `NETHER_AWAY` first, which is
        // why a hut with no portal does not block a citizen the colony is walking.
        let mut scene = new_scene();
        nether(&mut scene).travelling = true;
        let facts = scene.nether().unwrap().clone();
        let decision = decide(&scene.facts, &facts, &mut scene.memory);
        assert_eq!(decision.next, Some(WorkState::NetherAway));
        assert_eq!(decision.refusal, Some(WorkRefusal::Travelling));

        // `job.isInNether()` taken from the **job**, not from the world.
        let mut scene = new_scene();
        scene.memory.nether.in_nether = true;
        let facts = scene.nether().unwrap().clone();
        let decision = decide(&scene.facts, &facts, &mut scene.memory);
        assert_eq!(decision.next, Some(WorkState::NetherAway));
        assert_eq!(decision.refusal, Some(WorkRefusal::InNether));
    }

    #[test]
    fn decide_walks_the_sixteen_branches_in_java_order() {
        // Each case turns on exactly one fact, and each refusal names its line. The
        // order matters: a hut with no portal reports the portal, not the tools
        // (`:238-244` comes before `:248-258`).
        let cases: Vec<(fn(&mut NetherFacts), WorkRefusal)> = vec![
            (
                |facts| facts.crafter_busy = true,
                WorkRefusal::CraftingHalfOwnsIt,
            ),
            (
                |facts| {
                    facts.food_in_inventory = 0;
                    facts.food_in_building = 4;
                },
                // The food branch answers a **state**, not a refusal: it returns
                // `GATHERING_REQUIRED_MATERIALS` (`:212-216`).
                WorkRefusal::NotThisTrade,
            ),
            (
                |facts| facts.hut_recipe = Some(NetherRecipe::waiting()),
                WorkRefusal::MissingInputs,
            ),
            (|facts| facts.portal = None, WorkRefusal::NoPortal),
            (
                |facts| {
                    facts.portal = Some(Pos::new(1, 64, 0));
                    facts.missing_tools = vec![ToolKind::Sword];
                },
                WorkRefusal::MissingTool(ToolKind::Sword),
            ),
            (
                |facts| {
                    facts.portal = Some(Pos::new(1, 64, 0));
                    facts.fulfillable_recipe = false;
                },
                WorkRefusal::NoRecipe,
            ),
            (
                |facts| {
                    facts.portal = Some(Pos::new(1, 64, 0));
                    facts.ready_for_trip = false;
                },
                WorkRefusal::TripNotReady,
            ),
        ];
        for (index, (arrange, expected)) in cases.iter().enumerate() {
            let mut scene = new_scene();
            nether(&mut scene).ready_for_trip = true;
            if index != 5 {
                // The recipe is set by hand for every case but `NoRecipe`: `:260` is the
                // branch that decides whether it can be (`currentRecipeStorage != null`).
                scene.memory.nether.recipe_set = true;
            }
            arrange(nether(&mut scene));
            let facts = scene.nether().unwrap().clone();
            let decision = decide(&scene.facts, &facts, &mut scene.memory);
            if *expected == WorkRefusal::NotThisTrade {
                assert_eq!(
                    decision.next,
                    Some(WorkState::Base(WorkerState::GatheringRequiredMaterials)),
                    "case {index}: food is gathered before anything else"
                );
                continue;
            }
            assert_eq!(decision.refusal, Some(*expected), "case {index}");
        }
    }

    #[test]
    fn decide_walks_to_the_hut_then_to_the_portal_then_leaves() {
        // `:288-291` — a worker away from its hut stays in `START_WORKING` and says why.
        let mut scene = new_scene();
        with_portal(&mut scene);
        scene.memory.nether.recipe_set = true;
        scene.facts.worker.at = Pos::new(40, 64, 40);
        let facts = scene.nether().unwrap().clone();
        let decision = decide(&scene.facts, &facts, &mut scene.memory);
        assert_eq!(decision.next, None, "getState()");
        assert_eq!(decision.refusal, Some(WorkRefusal::NotAtHut));

        // At the hut, with everything, `decide` answers `NETHER_LEAVE` (`:311`).
        let mut scene = new_scene();
        with_portal(&mut scene);
        scene.memory.nether.recipe_set = true;
        let facts = scene.nether().unwrap().clone();
        let decision = decide(&scene.facts, &facts, &mut scene.memory);
        assert_eq!(decision.next, Some(WorkState::NetherLeave));
        assert_eq!(decision.refusal, None);
    }

    #[test]
    fn a_full_inventory_goes_to_inventory_full_not_to_the_nether() {
        // `:292-295` — after the walk, before `checkForItems`.
        let mut scene = new_scene();
        with_portal(&mut scene);
        scene.memory.nether.recipe_set = true;
        scene.facts.inventory.has_space = false;
        let facts = scene.nether().unwrap().clone();
        let decision = decide(&scene.facts, &facts, &mut scene.memory);
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::InventoryFull)));
        assert_eq!(decision.refusal, Some(WorkRefusal::NoSpace));
    }

    #[test]
    fn check_for_items_decides_the_last_three_exits_of_decide() {
        // `:297-309`: `GET_RECIPE` drops the recipe and answers `IDLE`, gathering answers
        // its own state, `CRAFT` answers `NETHER_LEAVE`.
        for (check, next, refusal) in [
            (
                CraftCheck::RecipeLost,
                WorkState::Base(WorkerState::Idle),
                Some(WorkRefusal::RecipeLost),
            ),
            (
                CraftCheck::GatherMaterials,
                WorkState::Base(WorkerState::GatheringRequiredMaterials),
                None,
            ),
            (CraftCheck::Craft, WorkState::NetherLeave, None),
        ] {
            let mut scene = new_scene();
            with_portal(&mut scene);
            scene.memory.nether.recipe_set = true;
            nether(&mut scene).craft_check = check;
            let facts = scene.nether().unwrap().clone();
            let decision = decide(&scene.facts, &facts, &mut scene.memory);
            assert_eq!(decision.next, Some(next), "{check:?}");
            assert_eq!(decision.refusal, refusal, "{check:?}");
        }
        // The `GET_RECIPE` branch also clears the recipe and marks the worker stuck
        // (`:301-302`).
        let mut scene = new_scene();
        with_portal(&mut scene);
        scene.memory.nether.recipe_set = true;
        nether(&mut scene).craft_check = CraftCheck::RecipeLost;
        let facts = scene.nether().unwrap().clone();
        let _ = decide(&scene.facts, &facts, &mut scene.memory);
        assert!(!scene.memory.nether.recipe_set);
        assert_eq!(scene.memory.nether.job_status, JobStatus::Stuck);
    }

    #[test]
    fn the_five_tools_are_reported_in_the_jobs_own_order() {
        // `:248-252` ORs five booleans; the port names the first missing one in
        // `Job::NetherWorker.tools()`'s order — axe, pickaxe, shovel, sword, flint.
        let mut nether = NetherFacts::default();
        nether.missing_tools = vec![ToolKind::FlintAndSteel, ToolKind::Pickaxe];
        assert_eq!(first_missing_tool(&nether), Some(ToolKind::Pickaxe));
        nether.missing_tools.clear();
        assert_eq!(first_missing_tool(&nether), None);
        // A tool the job does not state is still reported rather than swallowed.
        nether.missing_tools = vec![ToolKind::Rod];
        assert_eq!(first_missing_tool(&nether), Some(ToolKind::Rod));
    }

    #[test]
    fn leave_for_nether_lights_the_portal_or_walks_to_it() {
        // `:366` — the frame is there but the block is not a portal.
        let mut scene = new_scene();
        with_portal(&mut scene);
        scene.memory.nether.recipe_set = true;
        nether(&mut scene).blocks.set(Pos::new(1, 64, 0), WorkBlock::Air);
        let facts = scene.nether().unwrap().clone();
        let decision = leave_for_nether(&scene.facts, &facts, &mut scene.memory);
        assert_eq!(decision.next, Some(WorkState::NetherOpenPortal));

        // `:343-346` — out of reach, still walking, no state change.
        let mut scene = new_scene();
        with_portal(&mut scene);
        scene.memory.nether.recipe_set = true;
        nether(&mut scene).at = Pos::new(0, 30, 0);
        let facts = scene.nether().unwrap().clone();
        let decision = leave_for_nether(&scene.facts, &facts, &mut scene.memory);
        assert_eq!(decision.next, None);
        assert!(!scene.memory.nether.in_nether, "still here");

        // `:347-364` — arrived: inNether, WORKING, then `NETHER_AWAY`.
        let mut scene = new_scene();
        with_portal(&mut scene);
        scene.memory.nether.recipe_set = true;
        let facts = scene.nether().unwrap().clone();
        let decision = leave_for_nether(&scene.facts, &facts, &mut scene.memory);
        assert_eq!(decision.next, Some(WorkState::NetherAway));
        assert!(scene.memory.nether.in_nether);
        assert_eq!(scene.memory.nether.job_status, JobStatus::Working);
    }

    #[test]
    fn a_missing_recipe_or_portal_marks_the_worker_stuck() {
        // `:324-329` and `:368-369` — both answer `IDLE` with `STUCK` on the screen.
        let mut scene = new_scene();
        let facts = scene.nether().unwrap().clone();
        let decision = leave_for_nether(&scene.facts, &facts, &mut scene.memory);
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::Idle)));
        assert_eq!(decision.refusal, Some(WorkRefusal::NoRecipe));

        let mut scene = new_scene();
        scene.memory.nether.recipe_set = true;
        let facts = scene.nether().unwrap().clone();
        let decision = leave_for_nether(&scene.facts, &facts, &mut scene.memory);
        assert_eq!(decision.refusal, Some(WorkRefusal::NoPortal));
        assert_eq!(scene.memory.nether.job_status, JobStatus::Stuck);
    }

    #[test]
    fn one_block_item_costs_ticks_and_one_without_a_tool_costs_one() {
        // `:543` — `itemDelay += TICK_DELAY` per mined item; `:552` — no tiered tool
        // means one delay and nothing mined; `:559` — a plain item costs its count.
        for (result, processed, delay) in [
            (
                CraftedResult::Block { count: 3, has_tool: true },
                3,
                TICK_DELAY * 3,
            ),
            (CraftedResult::Block { count: 3, has_tool: false }, 0, TICK_DELAY),
            (CraftedResult::Plain { count: 2 }, 1, TICK_DELAY * 2),
        ] {
            let label = format!("{result:?}");
            let mut memory = WorkMemory::default();
            memory.nether.crafted.push(result);
            let nether = NetherFacts::default();
            let decision = stay_in_nether(&nether, &mut memory);
            assert_eq!(decision.next, None, "getState() while the queue drains");
            assert_eq!(memory.base.delay, delay as i32, "{label}");
            assert_eq!(memory.nether.processed.len(), processed, "{label}");
            assert!(memory.nether.crafted.is_empty(), "{label} was consumed");
        }
    }

    #[test]
    fn a_death_in_the_nether_drops_both_queues_and_goes_idle() {
        // `:479-490` — the expedition is over: inventory cleared, both queues cleared,
        // `IDLE`. The crafted queue is dropped whole, not just the token that killed.
        let mut memory = WorkMemory::default();
        memory.nether.crafted.push(CraftedResult::Adventure(AdventureToken {
            damage: Some(4.0),
            loot: vec![ProcessedItem { count: 1 }],
            ..AdventureToken::default()
        }));
        memory.nether.crafted.push(CraftedResult::Plain { count: 4 });
        memory.nether.processed.push(ProcessedItem { count: 2 });
        let nether = NetherFacts {
            worker_dead: true,
            ..NetherFacts::default()
        };
        let decision = stay_in_nether(&nether, &mut memory);
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::Idle)));
        assert_eq!(decision.refusal, Some(WorkRefusal::KilledInNether));
        assert!(memory.nether.crafted.is_empty());
        assert!(memory.nether.processed.is_empty());
    }

    #[test]
    fn a_token_without_a_fight_only_grants_its_xp() {
        // `:386-511` — the token half: no `TAG_DAMAGE` means no fight, no loot, no
        // delay; `TAG_XP_DROPPED` is an xp grant and nothing else.
        let mut memory = WorkMemory::default();
        memory.nether.crafted.push(CraftedResult::Adventure(AdventureToken {
            damage: None,
            xp_dropped: 7,
            ..AdventureToken::default()
        }));
        let nether = NetherFacts::default();
        let decision = stay_in_nether(&nether, &mut memory);
        assert_eq!(decision.next, None);
        assert_eq!(memory.base.delay, 0, "the adventure branch sets no delay");
        assert!(memory.nether.processed.is_empty(), "no loot without a fight");
        assert!(memory.nether.crafted.is_empty());
    }

    #[test]
    fn the_processed_queue_drains_before_the_worker_goes_home() {
        // `:568-591` then `:593-595`: while items are waiting the AI stays in
        // `NETHER_AWAY`; once the queue is empty it answers `NETHER_RETURN`.
        let mut memory = WorkMemory::default();
        memory.nether.processed.push(ProcessedItem { count: 1 });
        let nether = NetherFacts::default();
        assert_eq!(stay_in_nether(&nether, &mut memory).next, None);
        assert!(memory.nether.processed.is_empty());
        assert_eq!(
            stay_in_nether(&nether, &mut memory).next,
            Some(WorkState::NetherReturn)
        );
    }

    #[test]
    fn return_from_nether_closes_the_portal_only_when_the_setting_asks() {
        // `:634` — `shallClosePortalOnReturn() && the block is a portal`.
        let mut scene = new_scene();
        with_portal(&mut scene);
        nether(&mut scene).shall_close_portal_on_return = true;
        let facts = scene.nether().unwrap().clone();
        let decision = return_from_nether(&scene.facts, &facts, &mut scene.memory);
        assert_eq!(
            decision.next,
            Some(WorkState::NetherClosePortal)
        );

        // The setting off: walk home, then `INVENTORY_FULL` (`:651`) and `STUCK`.
        let mut scene = new_scene();
        with_portal(&mut scene);
        let facts = scene.nether().unwrap().clone();
        let decision = return_from_nether(&scene.facts, &facts, &mut scene.memory);
        assert_eq!(
            decision.next,
            Some(WorkState::Base(WorkerState::InventoryFull))
        );
        assert_eq!(scene.memory.nether.job_status, JobStatus::Stuck);
        assert!(!scene.memory.nether.recipe_set);
    }

    #[test]
    fn a_hut_saved_without_a_portal_tag_does_not_panic() {
        // Java would throw on `getBlockState(null)` at `:634`; the port reads "no
        // portal block" and walks home. This is the one deliberate divergence, and it is
        // named in the module note.
        let mut scene = new_scene();
        nether(&mut scene).shall_close_portal_on_return = true;
        assert!(scene.nether().unwrap().portal.is_none());
        let facts = scene.nether().unwrap().clone();
        let decision = return_from_nether(&scene.facts, &facts, &mut scene.memory);
        assert_eq!(
            decision.next,
            Some(WorkState::Base(WorkerState::InventoryFull))
        );
    }

    #[test]
    fn open_portal_needs_a_frame_then_leaves_once_it_is_lit() {
        // `:669-681` — no `PortalShape` means `IDLE`; a valid frame with an air block
        // means light it and leave; an already-lit portal falls through to
        // `START_WORKING` (`:684`).
        let mut scene = new_scene();
        with_portal(&mut scene);
        scene.memory.nether.recipe_set = true;
        nether(&mut scene).portal_frame_valid = false;
        let facts = scene.nether().unwrap().clone();
        assert_eq!(
            open_portal(&facts, &mut scene.memory).next,
            Some(WorkState::Base(WorkerState::Idle))
        );

        let mut scene = new_scene();
        with_portal(&mut scene);
        scene.memory.nether.recipe_set = true;
        nether(&mut scene).blocks.set(Pos::new(1, 64, 0), WorkBlock::Air);
        let facts = scene.nether().unwrap().clone();
        assert_eq!(
            open_portal(&facts, &mut scene.memory).next,
            Some(WorkState::NetherLeave)
        );

        let mut scene = new_scene();
        with_portal(&mut scene);
        scene.memory.nether.recipe_set = true;
        let facts = scene.nether().unwrap().clone();
        assert_eq!(
            open_portal(&facts, &mut scene.memory).next,
            Some(WorkState::Base(WorkerState::StartWorking))
        );
    }

    #[test]
    fn close_portal_returns_the_absent_worker_before_clearing_the_recipe() {
        // `:706-712` — in the nether, closing the portal means "come home"; at home it
        // means the trip is over: recipe cleared, `INVENTORY_FULL`.
        let mut scene = new_scene();
        with_portal(&mut scene);
        scene.memory.nether.in_nether = true;
        let facts = scene.nether().unwrap().clone();
        assert_eq!(
            close_portal(&facts, &mut scene.memory).next,
            Some(WorkState::NetherReturn)
        );

        let mut scene = new_scene();
        with_portal(&mut scene);
        scene.memory.nether.recipe_set = true;
        let facts = scene.nether().unwrap().clone();
        assert_eq!(
            close_portal(&facts, &mut scene.memory).next,
            Some(WorkState::Base(WorkerState::InventoryFull))
        );
        assert!(!scene.memory.nether.recipe_set);
    }

    #[test]
    fn the_five_armour_lists_are_the_constructors_and_filter_by_hut_level() {
        // `:110-114` — five `buildGearForLevel` calls; `:850-853` skips the ones whose
        // building range does not hold the level.
        assert_eq!(ARMOR_TIERS.len(), 5);
        // A level 5 hut: iron-to-max and chain-to-diamond only.
        let high = gear_for(5);
        assert_eq!(high.len(), 2);
        assert_eq!(high[0].min_armor, 3);
        assert_eq!(high[1].min_armor, 2);
        // A level 1 hut: leather-to-gold only (gold's range is 1..2).
        let low = gear_for(1);
        assert_eq!(low.len(), 1);
        assert_eq!(low[0].max_armor, 1);
        // A level 0 hut is below every range that has a floor of 1.
        assert_eq!(gear_for(0).len(), 0);
    }

    #[test]
    fn the_skill_reduces_an_incoming_hit_and_the_sword_adds_to_ours() {
        // `:406-407` and `:414-432`.
        // `f32` cannot hold 0.005 exactly, so the two arithmetic answers are compared
        // with a tolerance rather than bit for bit.
        assert_eq!(incoming_damage(10.0, 0), 10.0);
        assert!(
            (incoming_damage(10.0, 20) - 9.0).abs() < 1e-5,
            "20 * 0.005 = 0.1, got {}",
            incoming_damage(10.0, 20)
        );
        assert!(
            incoming_damage(10.0, 200).abs() < 1e-4,
            "immune at 200, got {}",
            incoming_damage(10.0, 200)
        );
        assert_eq!(attack_damage(None), BASE_PHYSICAL_DAMAGE);
        assert_eq!(
            attack_damage(Some(SwordDamage {
                item_damage: 7.0,
                enchant_bonus: 2.5,
            })),
            BASE_PHYSICAL_DAMAGE + 7.0 + 1.0
        );
    }

    #[test]
    fn healing_stops_below_low_saturation_and_takes_saturation_away() {
        // `:955-980`: three branches, in Java's order.
        let base = NetherFacts {
            health: 10.0,
            max_health: 20.0,
            saturation: 20.0,
            ..NetherFacts::default()
        };
        assert_eq!(check_heal(&base), 1.0, "between the bounds");
        assert_eq!(
            check_heal(&NetherFacts {
                saturation: 5.0,
                ..base.clone()
            }),
            0.0,
            "below LOW_SATURATION"
        );
        assert_eq!(
            check_heal(&NetherFacts {
                saturation: 60.0,
                ..base.clone()
            }),
            2.0,
            "at FULL_SATURATION"
        );
        // A research raises both nonzero branches and the full-saturation bound.
        assert_eq!(
            check_heal(&NetherFacts {
                saturation: 60.0,
                regeneration_research: 1.0,
                ..base.clone()
            }),
            4.0
        );
        assert_eq!(
            check_heal(&NetherFacts {
                saturation: 60.0,
                sat_limit_research: 10.0,
                ..base.clone()
            }),
            1.0,
            "60 < 60 + 10, so the normal branch"
        );
        // A whole worker is not healed at all (`:959`).
        assert_eq!(
            check_heal(&NetherFacts {
                health: 20.0,
                ..base.clone()
            }),
            0.0
        );
        // The eat that pays for it: `healAmount * 0.25` (`:460-465`).
        assert_eq!(heal_saturation_cost(2.0), 0.5);
    }

    #[test]
    fn xp_per_ore_is_a_range_and_the_vault_time_is_four_hundred_ticks_an_item() {
        // `:599-626` — coal 0..2, diamond and emerald 3..7, lapis and quartz 2..5,
        // nether gold 0..1, everything else 0.
        assert_eq!(NetherOre::Coal.xp_range(), (0, 2));
        assert_eq!(NetherOre::Diamond.xp_range(), (3, 7));
        assert_eq!(NetherOre::Emerald.xp_range(), (3, 7));
        assert_eq!(NetherOre::Lapis.xp_range(), (2, 5));
        assert_eq!(NetherOre::Quartz.xp_range(), (2, 5));
        assert_eq!(NetherOre::Gold.xp_range(), (0, 1));
        // Anything else is the literal `0` of `:625`, not a draw between zero and zero.
        assert_eq!(NetherOre::Other.xp_range(), (0, 0));
        assert_eq!(xp_on_drop(NetherOre::Coal, 0), 0);
        assert_eq!(xp_on_drop(NetherOre::Coal, 1), 1);
        assert_eq!(xp_on_drop(NetherOre::Diamond, 99), 3 + 99 % 4);
        assert_eq!(xp_on_drop(NetherOre::Other, 5), 0);
        // `:176` — twenty seconds per item.
        assert_eq!(vault_travel_ticks(0), 0);
        assert_eq!(vault_travel_ticks(3), 1200);
    }

    #[test]
    fn food_is_gathered_at_sixteen_and_only_when_the_hut_has_some() {
        // `:938-949`.
        let mut nether = NetherFacts::default();
        nether.food_in_inventory = 16;
        assert_eq!(check_and_request_food(&nether), FoodOutcome::Enough);
        nether.food_in_inventory = 15;
        nether.food_in_building = 1;
        assert_eq!(check_and_request_food(&nether), FoodOutcome::Gather);
        nether.food_in_building = 0;
        assert_eq!(check_and_request_food(&nether), FoodOutcome::Nothing);
    }

    #[test]
    fn a_trip_is_ready_below_one_per_period() {
        // `BuildingNetherWorker.java:61`, `:215-228`.
        assert!(is_ready_for_trip(0));
        assert!(!is_ready_for_trip(1));
        assert!(!is_ready_for_trip(2));
    }

    #[test]
    fn the_machine_climbs_from_idle_to_leave_in_one_tick_per_state() {
        // The end-to-end walk: `IDLE → START_WORKING → NETHER_LEAVE`, then the walk to
        // the portal keeps it there until it arrives, then `NETHER_AWAY`.
        let mut ai = WorkAi::new(WorkTrade::Nether).expect("a table");
        let mut scene = new_scene();
        with_portal(&mut scene);
        scene.memory.nether.recipe_set = true;
        assert_eq!(ai.state(), WorkState::Base(WorkerState::Idle));
        assert!(ai.tick(&mut scene).moved(), "IDLE → START_WORKING");
        assert_eq!(ai.state(), WorkState::Base(WorkerState::StartWorking));
        assert!(ai.tick(&mut scene).moved(), "START_WORKING → NETHER_LEAVE");
        assert_eq!(ai.state(), WorkState::NetherLeave);
        assert!(ai.tick(&mut scene).moved(), "arrived, so away");
        assert_eq!(ai.state(), WorkState::NetherAway);
        // `NETHER_AWAY` with both queues empty answers `NETHER_RETURN` (`:595`).
        assert!(ai.tick(&mut scene).moved());
        assert_eq!(ai.state(), WorkState::NetherReturn);
        // The default scene is at its hut, so `RETURN` finishes at `INVENTORY_FULL`.
        assert!(ai.tick(&mut scene).moved());
        assert_eq!(ai.state(), WorkState::Base(WorkerState::InventoryFull));
        // And `INVENTORY_FULL` hands back to `IDLE` (the hand-over seam).
        assert!(ai.tick(&mut scene).moved());
        assert_eq!(ai.state(), WorkState::Base(WorkerState::Idle));
        assert!(ai.missing().is_none(), "no state trapped the machine");
    }

    #[test]
    fn a_beekeepers_facts_are_refused_by_name_not_answered_as_a_missing_portal() {
        let mut ai = WorkAi::new(WorkTrade::Nether).expect("a table");
        let mut scene = ai.scene_with(
            WorldFacts::new(),
            crate::ai_work_07::WorkJob::Beekeeper(Default::default()),
        );
        let _ = ai.tick(&mut scene);
        let outcome = ai.tick(&mut scene);
        assert_eq!(outcome.why_not(), Some(WorkRefusal::NotThisTrade));
    }

    #[test]
    fn at_hut_is_the_same_seven_blocks_as_walk_to_building() {
        // `AbstractEntityAIBasic.java:757-766` over
        // `EntityNavigationUtils.WOKR_IN_BUILDING_DIST = 7`.
        let mut facts = WorldFacts::new();
        facts.building.position = Pos::new(0, 64, 0);
        assert!(at_hut(&facts));
        facts.worker.at = Pos::new(6, 64, 0);
        assert!(at_hut(&facts), "six is inside seven");
        facts.worker.at = Pos::new(7, 64, 0);
        assert!(!at_hut(&facts), "seven is not strictly inside");
    }
}
