//! What each of the 49 jobs **does**: the items it takes, what it makes and in
//! how long, where it must stand, and what one action is worth in experience.
//!
//! Java reference, one file per job class under `core/colony/jobs/` plus the AI
//! class under `core/entity/ai/workers/` that holds the durations; the shared
//! halves are `AbstractEntityAIInteract.java` (breaking a block),
//! `AbstractEntityAICrafting.java` (a recipe), `AbstractEntityAIBasic.java` (the
//! delays and the dump threshold) and
//! `core/entity/citizen/citizenhandlers/CitizenExperienceHandler.java` (where a
//! worker's XP lands).
//!
//! **The split, and why it is a split.** `crate::jobs` already answers *who is
//! hired where* (`employment`), *what one level costs* (`experience`), *which
//! tool a job states* (`job`) and *which level a hut allows* (`tool`). None of
//! those says what the job **does**. This module does, and only that: no state
//! machine, no world, no clock, no host call. Every row is a `&'static` table
//! entry, so a rule can be read and tested with no colony, no citizen and no
//! game — the same choice `crate::colony`, `crate::inventory` and `crate::jobs`
//! made.
//!
//! | File | Family | Jobs |
//! | --- | --- | --- |
//! | `bois` | what grows out of the ground | 4 |
//! | `mine` | what is dug out and fired | 8 |
//! | `ferme` | the field, the herd, the water | 9 |
//! | `artisanat` | the crafters | 12 |
//! | `service` | couriers, medicine, teaching, building | 8 |
//! | `militaire` | guards and their two training grounds | 8 |
//!
//! **49**, which is the count `crate::jobs::job` states for "jobs that work in a
//! hut". The fiftieth [`Job`] variant, `Placeholder`, has **no entry** and the
//! report says why (`RAPPORT.md`, *Ce qui est écarté*): Java gives it no AI
//! (`JobPlaceholder.java:31-34`), no translation key and no hut, so there is no
//! rule to carry. [`rules_for`] answers `None` for it, and
//! [`tests::every_job_that_works_has_rules_and_only_the_placeholder_does_not`]
//! is what keeps that from silently rotting.
//!
//! **Two durations are formulas, not numbers.** A job that breaks a block pays
//! [`block_mining_ticks`] (`AbstractEntityAIInteract.java:319-343`) and a crafter
//! pays [`crafting_ticks`] (`AbstractEntityAICrafting.java:734-741`); both depend
//! on the citizen's level, so a table cell holds [`Work::Block`] or
//! [`Work::Craft`] and the number comes out of the level at the time of the
//! action. Every other duration is a constant, with its own line.
//!
//! **What is *not* here, and what each gap costs.**
//!
//! - **the AI itself**: no state, no target, no path. The port has no per-tick
//!   citizen hook yet, so a job can be described but not performed — the same
//!   gap `crate::jobs` names in its own header.
//! - **the recipe data**: which sawmill recipe turns which log into which
//!   planks is `data/minecolonies/recipe/*` (one file per recipe), not code.
//!   Where Java names no item, [`Output::Recipe`] says so instead of inventing
//!   a product.
//! - **the block drops**: a miner produces whatever the block's loot table says.
//!   [`Output::Drops`] is that, and the port deliberately does not copy the loot
//!   tables into a second place.
//! - **the model and the particles**: `IJob.getModel()` is a client asset.
//! - **the request system**: [`Accepted`] says what a job takes; turning that
//!   into an `IRequest` a courier carries is `crate::requests`' neighbour.
//!
//! **Where a number comes from when two Java lines disagree.** They do not here:
//! one number, one line, quoted above the row that uses it. When two jobs share
//! a value it is a single constant in this file — [`SHARED`] lists them with
//! their Java line, and the family tables point at them rather than repeat them.

use crate::jobs::employment::{Employment, Skill};
use crate::jobs::job::Job;
use crate::jobs::tool::{ToolKind, TOOL_LEVEL_WOOD_OR_GOLD};

pub mod artisanat;
pub mod bois;
pub mod ferme;
pub mod militaire;
pub mod mine;
pub mod service;

// ---------------------------------------------------------------------------
// The shared constants: one number, one Java line, used by several families.
// ---------------------------------------------------------------------------

/// `Constants.TICKS_SECOND`: one second of game time
/// (`api/util/constant/Constants.java:30`).
pub const TICKS_SECOND: i32 = 20;

/// `CitizenConstants.TICKS_20`, the same twenty ticks under another name
/// (`api/util/constant/CitizenConstants.java:104`). Kept apart from
/// [`TICKS_SECOND`] because Java keeps them apart: one is a delay unit, the
/// other a slot in a cycle, and a data pack could move one without the other.
pub const TICKS_20: i32 = 20;

/// `AbstractEntityAIBasic.STANDARD_DELAY`: the state machine's own tick, and the
/// delay of every transition that has nothing better to say
/// (`AbstractEntityAIBasic.java:100`).
pub const STANDARD_DELAY: i32 = 5;

/// `AbstractEntityCitizen.ENTITY_AI_TICKRATE`: how many game ticks one AI tick is
/// worth (`api/entity/citizen/AbstractEntityCitizen.java:64`). A `STANDARD_DELAY`
/// transition therefore fires every 25 game ticks.
pub const ENTITY_AI_TICKRATE: i32 = 5;

/// `AbstractEntityAIBasic.WALK_DELAY`: how often a walking worker re-plans
/// (`AbstractEntityAIBasic.java:151`).
pub const WALK_DELAY: i32 = 20;

/// `AbstractEntityAIBasic.REQUEST_DELAY`: `TICKS_20 * 3`, i.e. three seconds
/// between two requests for the same stack
/// (`AbstractEntityAIBasic.java:105`).
pub const REQUEST_DELAY: i32 = TICKS_20 * 3;

/// `AbstractEntityAIBasic.PICKUP_ATTEMPTS`: how many times a worker tries to
/// pick a stack up before giving up (`AbstractEntityAIBasic.java:110`).
pub const PICKUP_ATTEMPTS: i32 = 10;

/// `CitizenConstants.ACTIONS_UNTIL_DUMP`: the default number of actions a worker
/// takes before walking back to its hut to empty its inventory
/// (`api/util/constant/CitizenConstants.java:166`, read at
/// `AbstractEntityAIBasic.java:425-428`).
pub const ACTIONS_UNTIL_DUMP: i32 = 32;

/// `AbstractEntityAIInteract.XP_PER_BLOCK`: what breaking one block is worth,
/// for **every** worker that breaks one (`AbstractEntityAIInteract.java:49`,
/// granted at `:234`).
pub const XP_PER_BLOCK: f64 = 0.05;

/// `AbstractEntityAIInteract.BLOCK_MINING_DELAY`: the numerator of the mining
/// formula, in ticks (`AbstractEntityAIInteract.java:100`).
pub const BLOCK_MINING_DELAY: i32 = 500;

/// `AbstractEntityAIInteract.LEVEL_MODIFIER`: each two levels of the breaking
/// skill multiply the mining time by `0.85`
/// (`AbstractEntityAIInteract.java:54`, used at `:338`).
pub const LEVEL_MODIFIER: f64 = 0.85;

/// `AbstractEntityAIInteract.MIN_WORKING_RANGE`: how close a worker must be to a
/// block before it counts as working on it
/// (`AbstractEntityAIInteract.java:59`).
pub const MIN_WORKING_RANGE: i32 = 12;

/// `AbstractEntityAIInteract.ITEM_PICKUP_RANGE`: how close a dropped stack must
/// be for a worker to take it (`AbstractEntityAIInteract.java:64`).
pub const ITEM_PICKUP_RANGE: i32 = 3;

/// `AbstractEntityAIInteract.STUCK_WAIT_TICKS`: how long a worker waits when its
/// path is blocked before it gives up (`AbstractEntityAIInteract.java:69`).
pub const STUCK_WAIT_TICKS: i32 = 20;

/// `AbstractEntityAIInteract.RANGE_HORIZONTAL_PICKUP`: the horizontal radius of
/// the loot sweep (`AbstractEntityAIInteract.java:74`).
pub const RANGE_HORIZONTAL_PICKUP: f64 = 45.0;

/// `AbstractEntityAIInteract.RANGE_VERTICAL_PICKUP`: the vertical half of the same
/// sweep (`AbstractEntityAIInteract.java:79`).
pub const RANGE_VERTICAL_PICKUP: f64 = 3.0;

/// `AbstractEntityAICrafting.HIT_DELAY`: how often one hit of a craft lands
/// (`core/entity/ai/workers/crafting/AbstractEntityAICrafting.java:73`).
pub const HIT_DELAY: i32 = 10;

/// `AbstractEntityAICrafting.PROGRESS_MULTIPLIER`: the `10` of the progress
/// formula (`AbstractEntityAICrafting.java:78`).
pub const PROGRESS_MULTIPLIER: i32 = 10;

/// `AbstractEntityAICrafting.MAX_LEVEL`: the ceiling of the skill term in the
/// progress formula, **50 levels**, not the citizen cap of 99
/// (`AbstractEntityAICrafting.java:83`).
pub const CRAFT_SPEED_LEVEL_CAP: i32 = 50;

/// `AbstractEntityAICrafting.HITTING_TIME`: the `3` of the progress formula
/// (`AbstractEntityAICrafting.java:88`).
pub const HITTING_TIME: i32 = 3;

/// `CitizenExperienceHandler.PRIMARY_DEPENDENCY_SHARE`: a tenth of the XP a
/// skill earns goes to its complimentary skill and a tenth leaves its adverse
/// one (`CitizenExperienceHandler.java:31`, used at `:107-115`).
pub const PRIMARY_DEPENDENCY_SHARE: i32 = 10;

/// `CitizenExperienceHandler.SECONDARY_DEPENDENCY_SHARE`: a twentieth for the
/// secondary skill's two neighbours (`CitizenExperienceHandler.java:36`, used at
/// `:120-128`).
pub const SECONDARY_DEPENDENCY_SHARE: i32 = 5;

/// `EntityNavigationUtils.WOKR_IN_BUILDING_DIST` (Java's own spelling): how close
/// to a tagged work position a worker must be to start working there
/// (`core/entity/pathfinding/navigation/EntityNavigationUtils.java:25`, read by
/// `AbstractEntityAIBasic.walkToTaggedWorkPos` at `:773-792`).
pub const WORK_IN_BUILDING_DIST: i32 = 7;

/// `AbstractEntityAICrafting`'s idle wait: `setDelay(TICKS_20 * 20)`, i.e. four
/// hundred ticks before a crafter without work looks again
/// (`AbstractEntityAICrafting.java:193`).
pub const CRAFT_IDLE_DELAY: i32 = TICKS_20 * 20;

/// The idle wander radius of a crafter, `walkToRandomPosWithin(worker, 10, ..)`
/// (`AbstractEntityAICrafting.java:253`).
pub const CRAFT_WANDER_RADIUS: i32 = 10;

/// `AbstractEntityAIRequestSmelter.BASE_XP_GAIN`: what **one furnace extraction**
/// is worth, for the four jobs that fire their own output
/// (`AbstractEntityAIRequestSmelter.java:56`, granted at `:622`).
pub const SMELTER_XP: f64 = 5.0;

/// `AbstractEntityAIUsesFurnace.BASE_XP_GAIN`: the waiter's grant, a fifth of the
/// smelter's (`AbstractEntityAIUsesFurnace.java:55`, granted by
/// `EntityAIWorkCook.java:119`, `:244`, `:301`).
pub const FURNACE_XP: f64 = 2.0;

/// `AbstractEntityAIRequestSmelter`: the four furnace states all run on
/// `TICKS_SECOND` (`AbstractEntityAIRequestSmelter.java:78-81`).
pub const FURNACE_VISIT_DELAY: i32 = TICKS_SECOND;

/// `AbstractEntityAIRequestSmelter`: how often a smelter re-checks its fuel —
/// `TICKS_SECOND * 10` (`:73`).
pub const FURNACE_FUEL_CHECK_DELAY: i32 = TICKS_SECOND * 10;

/// `AbstractEntityAIRequestSmelter`: how often a smelter picks the leftovers up —
/// `TICKS_SECOND * 10` (`:75`).
pub const FURNACE_REMAINS_DELAY: i32 = TICKS_SECOND * 10;

/// `AbstractEntityAIRequestSmelter`: the acceleration one furnace gets per ten
/// levels of the primary skill, `(level / 10) * 2` ticks a second (`:122`).
pub const FURNACE_ACCELERATION_PER_TEN_LEVELS: i32 = 2;

/// `AbstractEntityAIUsesFurnace.RETRIEVE_SMELTABLE_IF_MORE_THAN`: a waiter only
/// empties a furnace that holds more than ten (`:60`, read at `:141`).
pub const RETRIEVE_SMELTABLE_IF_MORE_THAN: i32 = 10;

/// `AbstractEntityAIUsesFurnace.STORAGE_BUFFER`: the room a waiter leaves in its
/// own inventory (`:66`, read at `:120`).
pub const STORAGE_BUFFER: i32 = 3;

/// `Constants.BREWING_MIN_FUEL_COUNT`: how much blaze powder an alchemist keeps
/// per stand (`api/util/constant/Constants.java:158`, read at
/// `EntityAIWorkAlchemist.java:543`, `:561`, `:622`).
pub const BREWING_MIN_FUEL_COUNT: i32 = 8;

/// `CitizenConstants.XP_EACH_BUILDING`: what a **finished structure** pays, to the
/// builder, the miner and the quarrier alike
/// (`api/util/constant/CitizenConstants.java:256`, granted at
/// `AbstractEntityAIStructure.java:279`).
pub const XP_EACH_BUILDING: f64 = 8.0;

/// `AbstractEntityAIUsesFurnace`: the `START_WORKING` state runs on `60` ticks,
/// not on `STANDARD_DELAY` (`AbstractEntityAIUsesFurnace.java:78`). Shared by the
/// smelter and the waiter, which are the two families' only furnace users.
pub const FURNACE_START_DELAY: i32 = 60;

/// `CitizenConstants.MIN_OPEN_SLOTS`: how many slots a builder always leaves free
/// so that a delivery can land (`CitizenConstants.java:176`, read at
/// `AbstractEntityAIStructure.java:251`).
pub const MIN_OPEN_SLOTS: i32 = 5;



/// `IJob.getSaturationFactor`'s default: a job that says nothing eats normally
/// (`api/colony/jobs/IJob.java:317-326`).
pub const SATURATION_DEFAULT: f64 = 1.0;

/// `AbstractJob.getSaturationFactor`'s override value, used by **six** jobs and
/// by every guard (`core/colony/jobs/AbstractJob.java:86-90` for the guards,
/// `JobFarmer.java:42-44`, `JobLumberjack.java:124-126`, `JobMiner.java:65-67`,
/// `JobQuarrier.java:114-116`, `JobDeliveryman.java:603-605`).
pub const SATURATION_HEAVY: f64 = 1.2;

/// The six shared constants a family table is allowed to point at by name, with
/// the Java line each one carries, so that
/// [`tests::every_shared_constant_quotes_a_java_line`] can check the citations
/// and a reader can find them without leaving the module.
pub const SHARED: &[(&str, &str)] = &[
    ("TICKS_SECOND", "Constants.java:30"),
    ("TICKS_20", "CitizenConstants.java:104"),
    ("STANDARD_DELAY", "AbstractEntityAIBasic.java:100"),
    ("ENTITY_AI_TICKRATE", "AbstractEntityCitizen.java:64"),
    ("WALK_DELAY", "AbstractEntityAIBasic.java:151"),
    ("REQUEST_DELAY", "AbstractEntityAIBasic.java:105"),
    ("PICKUP_ATTEMPTS", "AbstractEntityAIBasic.java:110"),
    ("ACTIONS_UNTIL_DUMP", "CitizenConstants.java:166"),
    ("XP_PER_BLOCK", "AbstractEntityAIInteract.java:49"),
    ("BLOCK_MINING_DELAY", "AbstractEntityAIInteract.java:100"),
    ("LEVEL_MODIFIER", "AbstractEntityAIInteract.java:54"),
    ("MIN_WORKING_RANGE", "AbstractEntityAIInteract.java:59"),
    ("ITEM_PICKUP_RANGE", "AbstractEntityAIInteract.java:64"),
    ("STUCK_WAIT_TICKS", "AbstractEntityAIInteract.java:69"),
    ("HIT_DELAY", "AbstractEntityAICrafting.java:73"),
    ("PROGRESS_MULTIPLIER", "AbstractEntityAICrafting.java:78"),
    ("CRAFT_SPEED_LEVEL_CAP", "AbstractEntityAICrafting.java:83"),
    ("HITTING_TIME", "AbstractEntityAICrafting.java:88"),
    ("PRIMARY_DEPENDENCY_SHARE", "CitizenExperienceHandler.java:31"),
    ("SECONDARY_DEPENDENCY_SHARE", "CitizenExperienceHandler.java:36"),
    ("SATURATION_DEFAULT", "IJob.java:317-326"),
    ("SATURATION_HEAVY", "AbstractJob.java:86-90"),
    ("WORK_IN_BUILDING_DIST", "EntityNavigationUtils.java:25"),
    ("CRAFT_IDLE_DELAY", "AbstractEntityAICrafting.java:193"),
    ("CRAFT_WANDER_RADIUS", "AbstractEntityAICrafting.java:253"),
    ("SMELTER_XP", "AbstractEntityAIRequestSmelter.java:56"),
    ("FURNACE_XP", "AbstractEntityAIUsesFurnace.java:55"),
    ("FURNACE_VISIT_DELAY", "AbstractEntityAIRequestSmelter.java:78-81"),
    ("FURNACE_FUEL_CHECK_DELAY", "AbstractEntityAIRequestSmelter.java:73"),
    ("FURNACE_REMAINS_DELAY", "AbstractEntityAIRequestSmelter.java:75"),
    ("FURNACE_ACCELERATION_PER_TEN_LEVELS", "AbstractEntityAIRequestSmelter.java:122"),
    ("RETRIEVE_SMELTABLE_IF_MORE_THAN", "AbstractEntityAIUsesFurnace.java:60"),
    ("STORAGE_BUFFER", "AbstractEntityAIUsesFurnace.java:66"),
    ("BREWING_MIN_FUEL_COUNT", "Constants.java:158"),
    ("XP_EACH_BUILDING", "CitizenConstants.java:256"),
    ("FURNACE_START_DELAY", "AbstractEntityAIUsesFurnace.java:78"),
    ("MIN_OPEN_SLOTS", "CitizenConstants.java:176"),
];

// ---------------------------------------------------------------------------
// The table types.
// ---------------------------------------------------------------------------

/// The family a job belongs to — the file its rules live in.
///
/// Java has no such enum: the six families are this port's reading of how the 49
/// classes group under `core/colony/jobs/` (the `Job*` prefix and the AI package
/// each one builds). It exists so that a caller can ask "which jobs work
/// outside" without listing them, and so that the tables stay small enough to
/// read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: six files, one per family, and a seventh family is a seventh file
pub enum Family {
    /// `bois`: the forester, the planter, the florist and the beekeeper.
    Wood,
    /// `mine`: the miner, the quarrier, the sifter, the nether miner and the
    /// four that fire or crush what comes out.
    Mine,
    /// `ferme`: the farmer, the six herders, the fisherman and the composter.
    Farm,
    /// `artisanat`: the twelve crafters.
    Craft,
    /// `service`: the courier, the doctor, the undertaker, the university, the
    /// school and the builder.
    Service,
    /// `militaire`: the six guards and the two training grounds.
    Military,
}

impl Family {
    /// Every family, in file order.
    pub const ALL: [Family; 6] = [
        Family::Wood,
        Family::Mine,
        Family::Farm,
        Family::Craft,
        Family::Service,
        Family::Military,
    ];

    /// The name the file carries, without accent — a path fragment, not prose.
    pub fn name(self) -> &'static str {
        match self {
            Family::Wood => "bois",
            Family::Mine => "mine",
            Family::Farm => "ferme",
            Family::Craft => "artisanat",
            Family::Service => "service",
            Family::Military => "militaire",
        }
    }

    /// The module's own rows, in table order.
    pub fn rules(self) -> &'static [JobRules] {
        match self {
            Family::Wood => bois::RULES,
            Family::Mine => mine::RULES,
            Family::Farm => ferme::RULES,
            Family::Craft => artisanat::RULES,
            Family::Service => service::RULES,
            Family::Military => militaire::RULES,
        }
    }
}

/// A work surface a job has to reach, and how far from it Java lets it stray.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: the twelve work surfaces the 49 job classes name, in the order a
// reader meets them
pub enum Spot {
    /// Nothing but the hut: the worker stands at or around its own building.
    Hut,
    /// A plantation field, a scarecrow's plot or a farmland block the farmer
    /// assigned himself.
    Field,
    /// A tree, or the spot where one will be planted.
    Tree,
    /// A water block to cast into — the fisherman and the herder's trough.
    Water,
    /// A beehive or a bee nest.
    Hive,
    /// A mine node at a level of the shaft.
    Mine,
    /// The quarry's dig site.
    Quarry,
    /// A furnace or a smoker the hut owns.
    Furnace,
    /// A brewing stand the alchemist owns.
    Still,
    /// The hut's own work surface: a barrel, a cauldron, a crafting bench.
    Bench,
    /// An animal inside the hut's enclosure.
    Stock,
    /// A hospital bed.
    Bed,
    /// A library desk or a school desk.
    Desk,
    /// A guard tower's post, or the rally point the player set.
    Post,
    /// A grave the undertaker must dig.
    Grave,
    /// A work order's site: anywhere in the colony, wherever the blueprint is
    /// being raised or taken down.
    Site,
    /// The nether, through the portal the worker builds.
    Nether,
}

/// A work surface, the reach Java states for it, and the line that says so.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    /// The surface itself.
    pub spot: Spot,
    /// How far from it the worker may be, in blocks, when Java names a bound.
    pub reach: Option<i32>,
    /// `fichier.java:ligne` of the bound (or of the spot, when there is none).
    pub source: &'static str,
}

/// Build a [`Place`] in a table row.
pub const fn place(spot: Spot, reach: Option<i32>, source: &'static str) -> Place {
    Place { spot, reach, source }
}

/// What a job does with an item it accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the eight kinds of stack the 49 job classes filter on
pub enum Role {
    /// A tool, with the level the job demands of it.
    Tool(ToolKind),
    /// A seed or a sapling to plant.
    Seed,
    /// Compost, bone meal, or the hut's fertiliser.
    Compost,
    /// Fuel for a furnace.
    Fuel,
    /// The ore, log, crop or block the job turns into something else.
    Material,
    /// Food the worker takes from the hut to eat.
    Food,
    /// A cure, a bandage, or the honey an apothecary needs.
    Cure,
    /// A potion the druid throws.
    Potion,
    /// A projectile.
    Projectile,
    /// A piece of armour a guard asks its building for.
    Armour,
}

/// One stack a job accepts, and what it does with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Accepted {
    /// The registry name (`minecraft:wheat`) or tag (`#minecraft:saplings`) Java
    /// names, spelled as Java spells it.
    pub item: &'static str,
    /// What the job does with it.
    pub role: Role,
    /// The minimum tool level, for a [`Role::Tool`]; `None` for a material.
    pub tool_level: Option<i32>,
    /// `fichier.java:ligne` of the filter or of the items list.
    pub source: &'static str,
}

/// The registry path of a [`ToolKind`], as a `const` function.
///
/// A copy of [`ToolKind::name`] that a table row can call: `name` is an ordinary
/// method and a `const` row cannot wait for one, and `src/jobs/tool.rs` is not this
/// module's to change. The duplication is checked rather than trusted —
/// [`tests::the_tool_name_table_agrees_with_the_equipment_registry`] walks all
/// fourteen kinds and fails if the two ever disagree.
pub const fn tool_name(kind: ToolKind) -> &'static str {
    match kind {
        ToolKind::None => "none",
        ToolKind::Pickaxe => "pickaxe",
        ToolKind::Shovel => "shovel",
        ToolKind::Axe => "axe",
        ToolKind::Hoe => "hoe",
        ToolKind::Sword => "sword",
        ToolKind::Bow => "bow",
        ToolKind::Crossbow => "crossbow",
        ToolKind::Rod => "rod",
        ToolKind::Shears => "shears",
        ToolKind::Shield => "shield",
        ToolKind::FlintAndSteel => "flintandsteel",
        ToolKind::Lead => "lead",
        ToolKind::Spear => "spear",
    }
}

/// A material a job accepts.
pub const fn material(item: &'static str, role: Role, source: &'static str) -> Accepted {
    Accepted { item, role, tool_level: None, source }
}

/// A tool a job accepts, with the level it demands.
///
/// The item name is the equipment registry's path through [`tool_name`], so a tool
/// row cannot disagree with [`ToolKind`].
pub const fn tool(kind: ToolKind, level: i32, source: &'static str) -> Accepted {
    Accepted { item: tool_name(kind), role: Role::Tool(kind), tool_level: Some(level), source }
}

/// A tool a job states at the floor level every job-stated request uses,
/// `CheckForToolOrWeapon`'s `0` (`AbstractEntityAIBasic.java:968`).
pub const fn tool_at_floor(kind: ToolKind, source: &'static str) -> Accepted {
    tool(kind, TOOL_LEVEL_WOOD_OR_GOLD, source)
}

/// A tool whose minimum level is the **block's** harvest level rather than a
/// constant: the miner, the quarrier and the builder all ask `holdEfficientTool`
/// for the best tool the block needs (`AbstractEntityAIBasic.java:1336-1355`,
/// `:1405-1432`), so this row says "a pickaxe, at whatever level this block
/// wants" and [`Accepted::tool_level`] is `None`.
pub const fn tool_for_block(kind: ToolKind, source: &'static str) -> Accepted {
    Accepted { item: tool_name(kind), role: Role::Tool(kind), tool_level: None, source }
}

/// What comes out of one piece of work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: four shapes, and Java has exactly four — a named stack, the block's
// drops, whatever the recipe says, or a living entity
pub enum Output {
    /// A named stack and its count. Used only where Java names both.
    Fixed { item: &'static str, count: u32 },
    /// Whatever the block's loot table drops. The miner, the forester and every
    /// herder produce this; the port does not copy the tables.
    Drops,
    /// Whatever the recipe in `data/minecolonies/recipe/` says. Java names no
    /// item at the call site either.
    Recipe,
    /// An **entity**, not a stack: the stablemaster's trained mount, the
    /// undertaker's resurrected citizen. Java builds it with `createFromVanilla` /
    /// `resurrectCivilianData` and never puts it in an inventory.
    Entity(&'static str),
}

/// How long one unit of work takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: five shapes, because Java computes a duration in exactly five ways
pub enum Work {
    /// A fixed number of game ticks.
    Ticks(i32),
    /// One block, through [`block_mining_ticks`] and the citizen's level.
    Block,
    /// One recipe, through [`crafting_ticks`] and the citizen's level.
    Craft,
    /// One item through a furnace: vanilla's own smelting time, which the port
    /// does not re-state.
    Smelt,
    /// The job's **own** progress counter — the sifter, the crusher and the nether
    /// miner each run a step function of their own instead of the crafting loop.
    /// The counter's period is the `durations` entry the row carries; the formula
    /// is in the row's comment and in the report, because the three are not the
    /// same formula and folding them into one would invent a fourth.
    Own,
    /// No work at all — a delivery, a lesson, a cure.
    Instant,
}

impl Work {
    /// The number of ticks, when it does not depend on a level or on a counter.
    pub fn fixed_ticks(self) -> Option<i32> {
        match self {
            Work::Ticks(ticks) => Some(ticks),
            Work::Block | Work::Craft | Work::Smelt | Work::Own | Work::Instant => None,
        }
    }

    /// Whether the duration is [`block_mining_ticks`]'s.
    pub fn is_block(self) -> bool {
        matches!(self, Work::Block)
    }
}

/// One product: what comes out, in how long, and the line that says the duration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Product {
    /// What comes out.
    pub output: Output,
    /// How long one unit takes.
    pub work: Work,
    /// `fichier.java:ligne` of the duration or of the produce call.
    pub source: &'static str,
}

/// Build a [`Product`] in a table row.
pub const fn product(output: Output, work: Work, source: &'static str) -> Product {
    Product { output, work, source }
}

/// What one of a job's numbers counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: five units, and every job number of the port is one of them
pub enum Unit {
    /// Game ticks — a duration.
    Ticks,
    /// Effort points, spent one state pass at a time: the undertaker's
    /// `EFFORT_BURY = 400` becomes `400 / skill` passes of `STANDARD_DELAY`
    /// (`EntityAIWorkUndertaker.java:475-479`).
    Effort,
    /// The denominator of a one-in-N roll — the alchemist's
    /// `DELAY_TO_HARVEST_NETHERWART = 30`, read as `nextInt(30) <= 1`
    /// (`EntityAIWorkAlchemist.java:71`, `:314`). A name that says "delay" and a
    /// value that is a chance, which is exactly why the port records the unit.
    ChanceDenominator,
    /// A count of blocks, items or positions.
    Count,
    /// A distance in blocks.
    Blocks,
}

/// One named number a job declares, with its Java line.
///
/// Not every constant a job declares is a duration — a radius, a count and a
/// factor live in [`Place`], [`Dump`] and [`Xp`] respectively. This is the
/// "combien de temps de travail" column of the mission, and [`Unit`] says what the
/// number counts so that a 30 that means "one harvest in thirty" is never read as
/// thirty ticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Constant {
    /// The constant's name, as Java spells it (`WAIT_BEFORE_SEARCH`).
    pub name: &'static str,
    /// Its value, in the unit [`Self::unit`] names.
    pub value: i32,
    /// What the value counts.
    pub unit: Unit,
    /// `fichier.java:ligne` of its declaration.
    pub source: &'static str,
}

/// A duration, in game ticks.
pub const fn constant(name: &'static str, ticks: i32, source: &'static str) -> Constant {
    Constant { name, value: ticks, unit: Unit::Ticks, source }
}

/// Effort points, spent `STANDARD_DELAY` at a time divided by the skill.
pub const fn effort(name: &'static str, points: i32, source: &'static str) -> Constant {
    Constant { name, value: points, unit: Unit::Effort, source }
}

/// The denominator of a one-in-N roll.
pub const fn chance(name: &'static str, denominator: i32, source: &'static str) -> Constant {
    Constant { name, value: denominator, unit: Unit::ChanceDenominator, source }
}

/// A count of blocks, items or positions.
pub const fn count(name: &'static str, amount: i32, source: &'static str) -> Constant {
    Constant { name, value: amount, unit: Unit::Count, source }
}

/// A distance in blocks.
pub const fn distance(name: &'static str, blocks: i32, source: &'static str) -> Constant {
    Constant { name, value: blocks, unit: Unit::Blocks, source }
}

/// What one unit of work is worth, and where the grant comes from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Xp {
    /// The XP one action grants, before the modifiers of [`xp_grant`].
    pub per_action: f64,
    /// The constant's name, as Java spells it.
    pub name: &'static str,
    /// `fichier.java:ligne` of the grant — the `addExperience` call, not the
    /// declaration, because that is what says *when* the XP lands.
    pub source: &'static str,
}

/// Build an [`Xp`] in a table row.
pub const fn xp(per_action: f64, name: &'static str, source: &'static str) -> Xp {
    Xp { per_action, name, source }
}

/// How many actions a worker takes before it walks back to empty its inventory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DumpScale {
    /// The same count at every hut level.
    Fixed,
    /// Multiplied by the hut's level, as four jobs do.
    PerBuildingLevel,
    /// As many as the **building** answers, not a constant: the concrete mixer's
    /// count of tracked water positions (`EntityAIConcreteMixer.java:183-186`) and
    /// the planter's per-module action limit (`EntityAIWorkPlanter.java:445-457`).
    /// The count is the building's, so [`Dump::at`] answers `None`.
    HutOwned,
}

/// The dump threshold of one job, and the line that sets it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dump {
    /// The count, before the scale.
    pub count: i32,
    /// Whether the count scales with the hut.
    pub scale: DumpScale,
    /// `fichier.java:ligne` of the override.
    pub source: &'static str,
}

impl Dump {
    /// The number of actions before a dump, in a hut of `level` — or `None` when
    /// the building answers (`DumpScale::HutOwned`).
    pub fn at(self, level: i32) -> Option<i32> {
        match self.scale {
            DumpScale::Fixed => Some(self.count),
            DumpScale::PerBuildingLevel => Some(self.count * level),
            DumpScale::HutOwned => None,
        }
    }
}

/// Build a [`Dump`] in a table row.
pub const fn dump(count: i32, scale: DumpScale, source: &'static str) -> Dump {
    Dump { count, scale, source }
}

/// Everything the port knows about what one job does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JobRules {
    /// The job these rules are about.
    pub job: Job,
    /// The family, i.e. the file the row lives in.
    pub family: Family,
    /// Where it works — **one or two surfaces**, never zero: a baker works at a
    /// bench *and* at the furnaces of its hut, and a shape that carried one place
    /// would have to drop one of them.
    pub places: &'static [Place],
    /// The stacks it takes, tools included.
    pub accepts: &'static [Accepted],
    /// What it makes, and how long each unit takes.
    pub produces: &'static [Product],
    /// The durations it declares beyond [`Self::produces`].
    pub durations: &'static [Constant],
    /// What one action is worth — **one or two grants**, because four jobs earn a
    /// flat amount per furnace extraction *and* half the request count per
    /// finished batch.
    pub xp: &'static [Xp],
    /// When it walks back to its hut.
    pub dump: Dump,
    /// `IJob.getSaturationFactor`: how fast it gets hungry.
    pub saturation: f64,
    /// `fichier.java:ligne` of the one line that best says what the job is —
    /// the class whose AI this row summarises.
    pub source: &'static str,
}

/// Why a rule cannot be applied — the port's own refusals, each named.
///
/// Never an `unwrap`: every one of these is reachable from a caller-supplied
/// value (a level, a research effect, a saturation), so each is a value and not
/// a panic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RuleRefusal {
    /// The job is `Placeholder`, or has no hiring module — Java grants it no XP
    /// because `getWorkBuilding()` answers `null`
    /// (`CitizenExperienceHandler.java:78-82`).
    Unemployed { job: Job },
    /// A citizen with no saturation earns nothing: Java returns before the split
    /// (`CitizenExperienceHandler.java:90-93`).
    NoSaturation,
    /// The hand tool has a destroy speed of `0`, so the mining formula's division
    /// has no answer. Java produces an infinity there; the port refuses instead of
    /// returning `i32::MAX` ticks.
    NoToolSpeed,
    /// A research effect above `1` would make the mining time negative
    /// (`AbstractEntityAIInteract.java:336-343`).
    BlockBreakResearchAboveOne { effect: f64 },
    /// A negative raw XP grant. The curve has no answer below zero, and
    /// `experience::gain` already refuses it.
    NegativeXp { xp: f64 },
}

impl RuleRefusal {
    /// A stable name, for a log line or a test.
    pub fn name(self) -> &'static str {
        match self {
            RuleRefusal::Unemployed { .. } => "unemployed",
            RuleRefusal::NoSaturation => "no_saturation",
            RuleRefusal::NoToolSpeed => "no_tool_speed",
            RuleRefusal::BlockBreakResearchAboveOne { .. } => "research_above_one",
            RuleRefusal::NegativeXp { .. } => "negative_xp",
        }
    }
}

// ---------------------------------------------------------------------------
// The registry.
// ---------------------------------------------------------------------------

/// Every job that works, in family order then table order — **49** rows.
pub fn all() -> impl Iterator<Item = &'static JobRules> {
    Family::ALL.into_iter().flat_map(|family| family.rules().iter())
}

/// What `job` does, or `None` for the one job that does nothing.
///
/// `None` for [`Job::Placeholder`] and for nothing else; the test
/// [`tests::every_job_that_works_has_rules_and_only_the_placeholder_does_not`]
/// holds both halves.
pub fn rules_for(job: Job) -> Option<&'static JobRules> {
    all().find(|rules| rules.job == job)
}

/// The rules of one family, in table order.
pub fn family(family: Family) -> &'static [JobRules] {
    family.rules()
}

/// Every job in `family`, without its rules.
pub fn jobs_of(family: Family) -> impl Iterator<Item = Job> {
    family.rules().iter().map(|rules| rules.job)
}

// ---------------------------------------------------------------------------
// The two duration formulas.
// ---------------------------------------------------------------------------

/// How long one block takes, in ticks — `AbstractEntityAIInteract.java:319-343`.
///
/// ```java
/// if (worker.getMainHandItem() == null) return (int) state.getDestroySpeed(world, pos);
/// return pvp_mode.get()
///          ? BLOCK_MINING_DELAY / 2
///          : calculateWorkerMiningDelay(state, pos);
/// // ...
/// final double reduction = 1 - ...getEffectStrength(BLOCK_BREAK_SPEED);
/// return (int) (((BLOCK_MINING_DELAY * Math.pow(LEVEL_MODIFIER, getBreakSpeedLevel() / 2.0))
///                  * (double) world.getBlockState(pos).getDestroySpeed(world, pos)
///                  / (double) (worker.getMainHandItem().getItem().getDestroySpeed(...)))
///                * reduction);
/// ```
///
/// Faithful to four details that matter:
///
/// - **the level term is `level / 2.0`**, a *double*, so every level halves the
///   `0.85` step: level 1 costs `0.85^0.5` of the time, level 2 `0.85`;
/// - **the cast is a truncation**, `(int)` in Java and `as i32` here, so the
///   result is never rounded up;
/// - **`pvp_mode` and "no tool in hand" short-circuit the formula**, `250` and
///   the raw hardness respectively;
/// - **the hardness is read from the world**, `getDestroySpeed`, not from a
///   table: the caller passes it in, in whatever unit the world uses.
///
/// `hardness` is `BlockState.getDestroySpeed` — negative for a block that cannot
/// be broken (`-1` in vanilla), which the port passes through as Java does.
pub fn block_mining_ticks(
    hardness: f64,
    tool_speed: f64,
    break_speed_level: i32,
    block_break_research: f64,
    has_tool: bool,
    pvp_mode: bool,
) -> Result<i32, RuleRefusal> {
    // `if (worker.getMainHandItem() == null) return (int) state.getDestroySpeed(...)`
    if !has_tool {
        return Ok(hardness as i32);
    }
    // `pvp_mode.get() ? BLOCK_MINING_DELAY / 2 : calculateWorkerMiningDelay(..)`
    if pvp_mode {
        return Ok(BLOCK_MINING_DELAY / 2);
    }
    if tool_speed == 0.0 {
        return Err(RuleRefusal::NoToolSpeed);
    }
    let reduction = 1.0 - block_break_research;
    if reduction < 0.0 {
        return Err(RuleRefusal::BlockBreakResearchAboveOne { effect: block_break_research });
    }
    let level = f64::from(break_speed_level) / 2.0;
    let ticks = (f64::from(BLOCK_MINING_DELAY) * LEVEL_MODIFIER.powf(level) * hardness / tool_speed)
        * reduction;
    Ok(ticks as i32)
}

/// How many hits one recipe takes — `AbstractEntityAICrafting.java:734-741`,
/// read at `:561`.
///
/// ```java
/// final int jobModifier = ...getLevel(getCraftSpeedSkill()) / 2;
/// return PROGRESS_MULTIPLIER / Math.min(jobModifier + 1, MAX_LEVEL) * HITTING_TIME;
/// ```
///
/// **Every term is an `int`, and that is the finding.** Java divides `10` by the
/// skill term *before* multiplying by `3`, so the count is a step function that
/// **collapses to zero** at level 20 — `10 / 11 == 0` — and a progress of `0` is
/// reached after the first hit (`:528`). A port that wrote `30 / term` would
/// spread the same eight jobs over a curve Java does not have. The port keeps
/// Java's steps, floors the count at one hit because a recipe always takes at
/// least one, and returns it.
///
/// | crafting skill | term | hits | ticks (`HIT_DELAY` = 10) |
/// | --- | --- | --- | --- |
/// | 0-1 | 1 | 30 | 300 |
/// | 2-3 | 2 | 15 | 150 |
/// | 4-5 | 3 | 9 | 90 |
/// | 6-7 | 4 | 6 | 60 |
/// | 8-9 | 5 | 6 | 60 |
/// | 10-11 | 6 | 6 | 60 |
/// | 12-13 | 7 | 6 | 60 |
/// | 14-15 | 8 | 6 | 60 |
/// | 16-17 | 9 | 3 | 30 |
/// | 18-19 | 10 | 3 | 30 |
/// | 20+ | 11..=50 | 1 | 10 |
pub fn crafting_hits(craft_speed_level: i32) -> i32 {
    let term = (craft_speed_level / 2 + 1).min(CRAFT_SPEED_LEVEL_CAP);
    let hits = PROGRESS_MULTIPLIER / term * HITTING_TIME;
    hits.max(1)
}

/// How long one recipe takes, in ticks: `crafting_hits * HIT_DELAY`
/// (`AbstractEntityAICrafting.java:169` registers the `CRAFT` transition with
/// `HIT_DELAY`).
pub fn crafting_ticks(craft_speed_level: i32) -> i32 {
    crafting_hits(craft_speed_level) * HIT_DELAY
}

// ---------------------------------------------------------------------------
// The experience split.
// ---------------------------------------------------------------------------

/// The citizen facts one XP grant reads — everything
/// `CitizenExperienceHandler.addExperience` touches outside the job itself
/// (`CitizenExperienceHandler.java:73-128`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XpContext {
    /// The work building's level, `getBuildingLevelEquivalent`.
    pub work_level: i32,
    /// The citizen's home level, `0` when it has no home
    /// (`CitizenExperienceHandler.java:74-75`).
    pub home_level: i32,
    /// The citizen's Intelligence level, which scales the grant
    /// (`:87-88`).
    pub intelligence: i32,
    /// The citizen's saturation; at or below `0` nothing lands (`:90-93`).
    pub saturation: f64,
    /// The colony's `LEVELING` research effect, `0.0` when none
    /// (`:95-96`).
    pub leveling: f64,
}

/// Where one grant landed, per skill.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XpGrant {
    /// The XP after the building, Intelligence and research modifiers — the
    /// `localXp` of `CitizenExperienceHandler.java:86-96`.
    pub local: f64,
    /// The primary skill and what it gains (all of `local`).
    pub primary: (Skill, f64),
    /// The primary's complimentary skill, a tenth of `local`.
    pub primary_complimentary: Option<(Skill, f64)>,
    /// The primary's adverse skill, which **loses** the same tenth.
    pub primary_adverse: Option<(Skill, f64)>,
    /// The secondary skill, half of `local`.
    pub secondary: (Skill, f64),
    /// The secondary's complimentary skill, a twentieth.
    pub secondary_complimentary: Option<(Skill, f64)>,
    /// The secondary's adverse skill, which loses the same twentieth.
    pub secondary_adverse: Option<(Skill, f64)>,
}

/// What a raw XP grant does to a citizen, per skill —
/// `CitizenExperienceHandler.java:73-128`.
///
/// ```java
/// final double buildingXPModifier = 1 + (workBuildingLevel + citizenHutLevel) / 10;
/// double localXp = xp * buildingXPModifier;
/// localXp += localXp * (intelligenceLevel / 100.0);
/// if (saturation <= 0) return;
/// localXp *= (1 + ...getEffectStrength(LEVELING));
/// addXpToSkill(primary, localXp, data);
/// addXpToSkill(primary.getComplimentary(), localXp / (100.0 / PRIMARY_DEPENDENCY_SHARE));
/// removeXpFromSkill(primary.getAdverse(), localXp / (100.0 / PRIMARY_DEPENDENCY_SHARE));
/// addXpToSkill(secondary, localXp / 2.0, data);
/// addXpToSkill(secondary.getComplimentary(), localXp / (100.0 / SECONDARY_DEPENDENCY_SHARE));
/// removeXpFromSkill(secondary.getAdverse(), localXp / (100.0 / SECONDARY_DEPENDENCY_SHARE));
/// ```
///
/// **The building term is the one a player feels**: a level-5 hut and a level-5
/// home double every grant, so the same tree is worth twice as much to a
/// well-housed forester. **The adverse skill really loses XP**, at the same rate
/// its neighbour gains: this split is not a redistribution of a fixed pot, it is
/// a gain and a loss that do not cancel.
///
/// The two skills come from the job's hiring module —
/// [`Employment::skills`], the same pair `crate::jobs::employment` carries. A job
/// hired by three huts (a knight: barracks, tower, gatehouse) has one module per
/// hut; the port reads the **first** in table order, which is what Java's
/// `getModuleMatching(WorkerBuildingModule.class, m -> m.getAssignedCitizen()...)`
/// resolves to when the citizen is assigned to one of them, and
/// [`tests::every_hiring_module_of_a_job_names_the_same_two_skills`] is what
/// makes that choice harmless: all of them agree.
pub fn xp_grant(job: Job, raw_xp: f64, context: &XpContext) -> Result<XpGrant, RuleRefusal> {
    if raw_xp < 0.0 {
        return Err(RuleRefusal::NegativeXp { xp: raw_xp });
    }
    let (primary, secondary) = skills_of(job)?;

    let building_modifier = 1.0 + f64::from(context.work_level + context.home_level) / 10.0;
    let mut local = raw_xp * building_modifier;
    local += local * (f64::from(context.intelligence) / 100.0);
    if context.saturation <= 0.0 {
        return Err(RuleRefusal::NoSaturation);
    }
    local *= 1.0 + context.leveling;

    let primary_share = f64::from(PRIMARY_DEPENDENCY_SHARE) / 100.0;
    let secondary_share = f64::from(SECONDARY_DEPENDENCY_SHARE) / 100.0;

    Ok(XpGrant {
        local,
        primary: (primary, local),
        primary_complimentary: primary.complimentary().map(|skill| (skill, local * primary_share)),
        primary_adverse: primary.adverse().map(|skill| (skill, local * primary_share)),
        secondary: (secondary, local / 2.0),
        secondary_complimentary: secondary
            .complimentary()
            .map(|skill| (skill, local * secondary_share)),
        secondary_adverse: secondary.adverse().map(|skill| (skill, local * secondary_share)),
    })
}

/// The two skills the job's module levels, or [`RuleRefusal::Unemployed`].
pub fn skills_of(job: Job) -> Result<(Skill, Skill), RuleRefusal> {
    job.employments()
        .find_map(Employment::skills)
        .ok_or(RuleRefusal::Unemployed { job })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::job::Job;

    /// `RAPPORT.md` and this module say "49 jobs, and the placeholder is not
    /// one". This is that sentence as an assertion, both ways: every job but the
    /// placeholder has a row, and no row exists for it.
    #[test]
    fn every_job_that_works_has_rules_and_only_the_placeholder_does_not() {
        let missing: Vec<&str> = Job::ALL
            .iter()
            .filter(|job| rules_for(**job).is_none())
            .map(|job| job.path())
            .collect();
        assert_eq!(
            missing,
            vec!["placeholder"],
            "the one job Java gives no AI is the only one without rules"
        );
        assert_eq!(all().count(), Job::ALL.len() - 1, "49 rows");
    }

    #[test]
    fn no_job_has_two_rows_and_every_row_names_itself() {
        let mut seen = Vec::new();
        for rules in all() {
            assert!(
                !seen.contains(&rules.job),
                "{} has two rows",
                rules.job.path()
            );
            seen.push(rules.job);
            assert!(!rules.source.contains(' '), "{}", rules.source);
            assert!(rules.source.contains(".java:"), "{}", rules.source);
        }
        assert_eq!(seen.len(), 49);
    }

    #[test]
    fn every_row_sits_in_the_family_of_the_file_it_is_in() {
        for family in Family::ALL {
            for rules in family.rules() {
                assert_eq!(
                    rules.family,
                    family,
                    "{} is filed under {}",
                    rules.job.path(),
                    family.name()
                );
            }
        }
        // fixed sizes: a row moved between files would silently change a family
        assert_eq!(Family::Wood.rules().len(), 4);
        assert_eq!(Family::Mine.rules().len(), 8);
        assert_eq!(Family::Farm.rules().len(), 9);
        assert_eq!(Family::Craft.rules().len(), 12);
        assert_eq!(Family::Service.rules().len(), 8);
        assert_eq!(Family::Military.rules().len(), 8);
    }

    #[test]
    fn every_row_cites_a_java_line_somewhere() {
        for rules in all() {
            assert!(
                !rules.places.is_empty(),
                "{} works nowhere",
                rules.job.path()
            );
            for place in rules.places {
                assert!(
                    place.source.contains(".java:"),
                    "{} has a place with no source",
                    rules.job.path()
                );
            }
            assert!(!rules.xp.is_empty(), "{} has no xp grant", rules.job.path());
            for grant in rules.xp {
                assert!(
                    grant.source.contains(".java:"),
                    "{} has an xp grant with no line",
                    rules.job.path()
                );
            }
            assert!(
                rules.dump.source.contains(".java:"),
                "{} has no dump source",
                rules.job.path()
            );
            for accepted in rules.accepts {
                assert!(
                    accepted.source.contains(".java:"),
                    "{} accepts {} with no line",
                    rules.job.path(),
                    accepted.item
                );
            }
            for product in rules.produces {
                assert!(
                    product.source.contains(".java:"),
                    "{} produces with no line",
                    rules.job.path()
                );
            }
            for duration in rules.durations {
                assert!(
                    duration.source.contains(".java:"),
                    "{} constant {} has no line",
                    rules.job.path(),
                    duration.name
                );
                assert!(!duration.name.is_empty(), "{}", rules.job.path());
            }
        }
    }

    #[test]
    fn every_shared_constant_quotes_a_java_line() {
        for (name, source) in SHARED {
            assert!(!name.is_empty());
            assert!(source.contains(".java:"), "{name} -> {source}");
        }
        assert_eq!(SHARED.len(), 37);
    }

    #[test]
    fn the_tool_name_table_agrees_with_the_equipment_registry() {
        for kind in ToolKind::ALL {
            assert_eq!(tool_name(kind), kind.name(), "{kind:?}");
        }
        assert_eq!(ToolKind::ALL.len(), 14);
        assert_eq!(tool_name(ToolKind::Rod), "rod", "not `fishing_rod`");
        assert_eq!(tool_name(ToolKind::FlintAndSteel), "flintandsteel", "one word");
    }

    #[test]
    fn a_tool_row_cannot_disagree_with_the_equipment_registry() {
        for rules in all() {
            for accepted in rules.accepts {
                if let Role::Tool(kind) = accepted.role {
                    assert_eq!(accepted.item, kind.name(), "{}", rules.job.path());
                } else {
                    assert_eq!(accepted.tool_level, None, "{}", rules.job.path());
                }
            }
        }
        // the two shapes: a stated level (`tool_at_floor`) and "the block decides"
        // (`tool_for_block`) — a tool row is never a material row with a level
        assert_eq!(tool_at_floor(ToolKind::Axe, "x.java:1").tool_level, Some(0));
        assert_eq!(tool_for_block(ToolKind::Pickaxe, "x.java:1").tool_level, None);
    }

    #[test]
    fn the_saturation_factor_is_the_default_or_the_heavy_one_and_nothing_else() {
        let mut heavy: Vec<&str> = all()
            .filter(|rules| rules.saturation == SATURATION_HEAVY)
            .map(|rules| rules.job.path())
            .collect();
        heavy.sort_unstable();
        assert_eq!(
            heavy,
            vec![
                "cavalry",
                "deliveryman",
                "druid",
                "farmer",
                "huscarl",
                "knight",
                "lumberjack",
                "marksman",
                "miner",
                "quarrier",
                "ranger",
            ],
            "six jobs and five guards: the guards' factor is AbstractJobGuard's"
        );
        for rules in all() {
            assert!(
                rules.saturation == SATURATION_DEFAULT || rules.saturation == SATURATION_HEAVY,
                "{} eats at {}",
                rules.job.path(),
                rules.saturation
            );
        }
    }

    #[test]
    fn the_families_cover_every_job_exactly_once() {
        let mut total = 0;
        for family in Family::ALL {
            for job in jobs_of(family) {
                assert!(Job::ALL.contains(&job));
                total += 1;
            }
        }
        assert_eq!(total, 49);
        assert_eq!(Family::ALL.map(Family::name).len(), 6);
        assert_eq!(Family::Craft.name(), "artisanat");
        assert_eq!(Family::Military.name(), "militaire");
    }

    // -- the two duration formulas -----------------------------------------

    #[test]
    fn a_bare_hand_pays_the_hardness_and_nothing_else() {
        assert_eq!(block_mining_ticks(3.0, 0.0, 40, 0.0, false, false), Ok(3));
        assert_eq!(block_mining_ticks(1.9, 8.0, 0, 0.0, false, false), Ok(1));
        assert_eq!(
            block_mining_ticks(-1.0, 0.0, 0, 0.0, false, false),
            Ok(-1),
            "an unbreakable block stays unbreakable: Java keeps the -1"
        );
    }

    #[test]
    fn pvp_mode_halves_the_delay_and_ignores_everything_else() {
        // AbstractEntityAIInteract.java:323 — `BLOCK_MINING_DELAY / 2`
        assert_eq!(block_mining_ticks(50.0, 1.0, 0, 0.0, true, true), Ok(250));
        assert_eq!(block_mining_ticks(50.0, 1.0, 99, 0.9, true, true), Ok(250));
        assert_eq!(BLOCK_MINING_DELAY / 2, 250);
    }

    #[test]
    fn the_mining_time_halves_every_two_levels_by_a_factor_of_point_eight_five() {
        let base = block_mining_ticks(1.0, 1.0, 0, 0.0, true, false);
        let level_two = block_mining_ticks(1.0, 1.0, 2, 0.0, true, false);
        let level_four = block_mining_ticks(1.0, 1.0, 4, 0.0, true, false);
        assert_eq!(base, Ok(500));
        assert_eq!(level_two, Ok(425), "500 * 0.85 = 425, truncated");
        assert_eq!(level_four, Ok(361), "500 * 0.85^2 = 361.25, truncated");
        // the level term is a double, so level 1 already buys half a step
        assert_eq!(block_mining_ticks(1.0, 1.0, 1, 0.0, true, false), Ok(460));
    }

    #[test]
    fn the_mining_time_scales_with_hardness_and_inversely_with_the_tool() {
        assert_eq!(block_mining_ticks(2.0, 1.0, 0, 0.0, true, false), Ok(1000));
        assert_eq!(block_mining_ticks(1.0, 2.0, 0, 0.0, true, false), Ok(250));
        assert_eq!(block_mining_ticks(10.0, 10.0, 0, 0.0, true, false), Ok(500));
    }

    #[test]
    fn a_block_break_research_of_point_two_takes_a_fifth_off() {
        assert_eq!(block_mining_ticks(1.0, 1.0, 0, 0.2, true, false), Ok(400));
        assert_eq!(block_mining_ticks(1.0, 1.0, 0, 1.0, true, false), Ok(0));
    }

    #[test]
    fn the_two_impossible_inputs_are_refused_by_name() {
        assert_eq!(
            block_mining_ticks(1.0, 0.0, 0, 0.0, true, false),
            Err(RuleRefusal::NoToolSpeed)
        );
        assert_eq!(
            block_mining_ticks(1.0, 1.0, 0, 1.5, true, false),
            Err(RuleRefusal::BlockBreakResearchAboveOne { effect: 1.5 })
        );
        assert_eq!(RuleRefusal::NoToolSpeed.name(), "no_tool_speed");
        assert_eq!(
            RuleRefusal::BlockBreakResearchAboveOne { effect: 1.5 }.name(),
            "research_above_one"
        );
    }

    #[test]
    fn the_crafting_count_is_java_s_integer_division_and_collapses_at_level_twenty() {
        assert_eq!(crafting_hits(0), 30, "10 / 1 * 3");
        assert_eq!(crafting_hits(1), 30);
        assert_eq!(crafting_hits(2), 15, "10 / 2 * 3");
        assert_eq!(crafting_hits(4), 9, "10 / 3 * 3");
        assert_eq!(crafting_hits(6), 6, "10 / 4 * 3");
        assert_eq!(crafting_hits(8), 6, "10 / 5 * 3");
        assert_eq!(crafting_hits(16), 3, "10 / 9 * 3");
        assert_eq!(crafting_hits(18), 3, "10 / 10 * 3");
        // 10 / 11 == 0, so the recipe is finished by the first hit (`:528`)
        assert_eq!(crafting_hits(20), 1);
        assert_eq!(crafting_hits(98), 1);
        assert_eq!(crafting_hits(200), 1, "the term is capped at 50, then floored at one hit");
    }

    #[test]
    fn the_crafting_time_is_never_zero_and_never_grows_with_the_level() {
        assert_eq!(crafting_ticks(0), 300);
        assert_eq!(crafting_ticks(20), 10);
        let mut previous = i32::MAX;
        for level in 0..=120 {
            let ticks = crafting_ticks(level);
            assert!(ticks >= HIT_DELAY, "level {level} pays {ticks}");
            assert!(ticks <= previous, "level {level} is slower than {}", level - 1);
            previous = ticks;
        }
    }

    #[test]
    fn the_level_term_of_a_craft_is_capped_at_fifty_like_java_s_max_level() {
        assert_eq!(
            crafting_hits(50 * 2 + 1),
            crafting_hits(10_000),
            "past the cap the answer never moves again"
        );
        assert_eq!(CRAFT_SPEED_LEVEL_CAP, 50);
        assert_ne!(CRAFT_SPEED_LEVEL_CAP, crate::jobs::experience::MAX_CITIZEN_LEVEL);
    }

    // -- the experience split ----------------------------------------------

    fn context() -> XpContext {
        XpContext { work_level: 0, home_level: 0, intelligence: 0, saturation: 20.0, leveling: 0.0 }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn a_level_zero_hut_and_home_leave_the_grant_alone() {
        let grant = xp_grant(Job::Lumberjack, 1.0, &context()).unwrap();
        assert!(close(grant.local, 1.0));
        // `Lumberjack: "forester_work" => Lumberjack, (Strength, Focus, false)`
        assert_eq!(grant.primary.0, Skill::Strength);
        assert_eq!(grant.secondary.0, Skill::Focus);
        assert!(close(grant.primary.1, 1.0));
        assert!(close(grant.secondary.1, 0.5), "secondary takes half");
        assert_eq!(grant.primary_complimentary.unwrap().0, Skill::Athletics);
        assert!(close(grant.primary_complimentary.unwrap().1, 0.1), "a tenth");
        assert_eq!(grant.primary_adverse.unwrap().0, Skill::Agility);
        assert!(close(grant.primary_adverse.unwrap().1, 0.1), "and the adverse loses a tenth");
        assert_eq!(grant.secondary_complimentary.unwrap().0, Skill::Mana);
        assert!(close(grant.secondary_complimentary.unwrap().1, 0.05), "a twentieth");
        assert_eq!(grant.secondary_adverse.unwrap().0, Skill::Adaptability);
        assert!(close(grant.secondary_adverse.unwrap().1, 0.05));
    }

    #[test]
    fn a_level_five_hut_and_a_level_five_home_double_the_grant() {
        let context = XpContext { work_level: 5, home_level: 5, ..context() };
        let grant = xp_grant(Job::Lumberjack, 1.0, &context).unwrap();
        assert!(close(grant.local, 2.0), "1 + (5 + 5) / 10 = 2");
        assert!(close(grant.primary.1, 2.0));
    }

    #[test]
    fn intelligence_adds_its_level_in_percent_and_the_research_multiplies_after() {
        let context = XpContext { intelligence: 50, leveling: 0.1, ..context() };
        let grant = xp_grant(Job::Lumberjack, 1.0, &context).unwrap();
        // 1 * 1.5, then * 1.1
        assert!(close(grant.local, 1.65));
    }

    #[test]
    fn a_citizen_with_no_saturation_earns_nothing_and_the_refusal_says_so() {
        let context = XpContext { saturation: 0.0, ..context() };
        assert_eq!(
            xp_grant(Job::Lumberjack, 1.0, &context),
            Err(RuleRefusal::NoSaturation)
        );
        assert_eq!(RuleRefusal::NoSaturation.name(), "no_saturation");
    }

    #[test]
    fn a_negative_grant_is_refused_rather_than_run_through_the_curve() {
        assert_eq!(
            xp_grant(Job::Lumberjack, -1.0, &context()),
            Err(RuleRefusal::NegativeXp { xp: -1.0 })
        );
    }

    #[test]
    fn the_placeholder_earns_nothing_because_it_has_no_module() {
        assert_eq!(
            xp_grant(Job::Placeholder, 1.0, &context()),
            Err(RuleRefusal::Unemployed { job: Job::Placeholder })
        );
        assert_eq!(skills_of(Job::Placeholder).is_err(), true);
    }

    #[test]
    fn every_hiring_module_of_a_job_names_the_same_two_skills() {
        // three huts hire a knight; the port reads the first, so all must agree
        for job in Job::ALL {
            let mut pairs = job.employments().filter_map(Employment::skills);
            let first = pairs.next();
            for other in pairs {
                assert_eq!(Some(other), first, "{} disagrees with itself", job.path());
            }
        }
        let knight: Vec<Skill> = Job::Knight.employments().filter_map(Employment::skills).map(|(p, _)| p).collect();
        assert!(knight.len() >= 2, "a knight is hired by more than one hut");
    }

    #[test]
    fn the_grant_it_produces_is_what_the_curve_consumes() {
        // one finished tree for a level-0 forester: 1 XP, which is one level
        let grant = xp_grant(Job::Lumberjack, bois::XP_PER_TREE, &context()).unwrap();
        let progress = crate::jobs::experience::gain(0, 0.0, grant.primary.1, 10);
        assert!(progress.level >= 1, "1 XP buys the first level, which costs 1");
    }

    #[test]
    fn every_row_s_xp_is_a_number_a_level_can_use() {
        for rules in all() {
            assert!(!rules.xp.is_empty(), "{}", rules.job.path());
            for grant in rules.xp {
                assert!(grant.per_action > 0.0, "{}", rules.job.path());
                assert!(grant.per_action <= 100.0, "{}", rules.job.path());
                assert!(!grant.name.is_empty(), "{}", rules.job.path());
                assert!(
                    grant.source.contains(".java:"),
                    "{} has an xp grant with no line",
                    rules.job.path()
                );
            }
        }
    }
}
