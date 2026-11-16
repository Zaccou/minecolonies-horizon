//! `ferme` — the field, the herd, the water: nine jobs that work outside, plus
//! the composter that turns their waste into compost.
//!
//! Java reference: `core/colony/jobs/{JobFarmer,JobComposter,JobChickenHerder,
//! JobCowboy,JobShepherd,JobSwineHerder,JobRabbitHerder,JobStablemaster,
//! JobFisherman}.java`; the AI classes `production/agriculture/{EntityAIWorkFarmer,
//! EntityAIWorkComposter,EntityAIWorkFisherman}`, `production/herders/*` and, for
//! the six animal jobs, `AbstractEntityAIHerder`.
//!
//! **Six of the nine share one base class and therefore one set of numbers.** The
//! chicken herder, the cowboy, the shepherd, the swineherd, the rabbit hutch and
//! the stable all extend `AbstractEntityAIHerder`, whose constants this file
//! declares **once** ([`XP_PER_ACTION`], [`DECIDING_DELAY`], [`BREEDING_DELAY`],
//! [`BUTCHER_DELAY`], [`DISTANCE_TO_BREED`], [`ACTIONS_FOR_DUMP`]…) instead of
//! repeating them in six rows. Three of the six — chicken, swine, rabbit — add
//! **not one constant of their own**; their whole rule is the herder's, so their
//! `durations` are empty and the report says so.
//!
//! **An enclosure is not a radius.** For all six, the animals are the ones inside
//! the **axis-aligned box of the building's own schematic corners**
//! (`WorldUtil.getEntitiesWithinBuilding`, `api/util/WorldUtil.java:321-337`, read
//! at `AbstractEntityAIHerder.java:598`). Only two jobs of the family state a
//! distance: the stablemaster hunts mounts within **30 blocks**
//! (`EntityAIWorkStableMaster.java:738`) and the fisherman looks for a pond within
//! **50, then 150 blocks** (`EntityAIWorkFisherman.java:115`, `:424`, `:443`).
//!
//! **Three jobs are not herders at all** and each says so: the farmer works
//! cropland (`FarmField`), the composter works barrels, the fisherman works water.
//!
//! **What is not here.** The breeding item of an animal module is data
//! (`BuildingModules.java` names it per module, one line each, and this file
//! quotes the line); the crops a farmer grows are the hut's fields, not code; and
//! the wool a shepherd shears carries the sheep's colour, so its row says
//! [`Output::Drops`] rather than invent sixteen item names.

use super::{
    constant, dump, material, place, product, tool_at_floor, xp, DumpScale, Family, JobRules,
    Output, Role, Spot, Work, SATURATION_DEFAULT, SATURATION_HEAVY, XP_PER_BLOCK,
};
use crate::jobs::job::Job;
use crate::jobs::tool::{ToolKind, TOOL_LEVEL_HAND};

/// Build one row of [`RULES`]; the family is fixed by the file.
macro_rules! rules {
    ($job:ident, $places:expr, [$($accepted:expr),* $(,)?], [$($product:expr),* $(,)?],
     [$($duration:expr),* $(,)?], [$($xp:expr),* $(,)?], $dump:expr, $saturation:expr,
     $source:literal) => {
        JobRules {
            job: Job::$job,
            family: Family::Farm,
            places: $places,
            accepts: &[$($accepted),*],
            produces: &[$($product),*],
            durations: &[$($duration),*],
            xp: &[$($xp),*],
            dump: $dump,
            saturation: $saturation,
            source: $source,
        }
    };
}

/// `AbstractEntityAIHerder.XP_PER_ACTION`: what one breeding, one feeding or one
/// butchering is worth (`AbstractEntityAIHerder.java:103`, granted at `:382`,
/// `:557`, `:659`). Every herder shares it, including the stablemaster's four
/// grants (`EntityAIWorkStableMaster.java:289`, `:368`, `:390`, `:600`).
pub const XP_PER_ACTION: f64 = 0.5;

/// `AbstractEntityAIHerder.NUM_OF_ANIMALS_TO_BREED`: two at a time
/// (`AbstractEntityAIHerder.java:61`, read at `:238`).
pub const NUM_OF_ANIMALS_TO_BREED: i32 = 2;

/// `AbstractEntityAIHerder.EXTRA_BREEDING_ITEMS_REQUEST`: how many extra breeding
/// items a herder asks for, so that a starving hut can still breed
/// (`AbstractEntityAIHerder.java:66`, read at `:319`, `:868`).
pub const EXTRA_BREEDING_ITEMS_REQUEST: i32 = 8;

/// `AbstractEntityAIHerder.BUTCHERING_ATTACK_DAMAGE`: the damage of one butchering
/// swing, for the jobs that do not compute their own
/// (`AbstractEntityAIHerder.java:71`, read at `:850`).
pub const BUTCHERING_ATTACK_DAMAGE: i32 = 5;

/// `AbstractEntityAIHerder.DISTANCE_TO_BREED`: how far apart two animals may be and
/// still be paired (`AbstractEntityAIHerder.java:76`, read at `:452`).
pub const DISTANCE_TO_BREED: i32 = 10;

/// `AbstractEntityAIHerder.DECIDING_DELAY`: how often a herder picks its next
/// action, in ticks (`AbstractEntityAIHerder.java:82`, read at `:143`, `:205`).
pub const DECIDING_DELAY: i32 = 80;

/// `AbstractEntityAIHerder.BREEDING_DELAY`: how long a breeding attempt takes
/// before the herder moves on (`AbstractEntityAIHerder.java:83`, read at `:144`).
pub const BREEDING_DELAY: i32 = 40;

/// `AbstractEntityAIHerder.BUTCHER_DELAY`: the butchering state's period
/// (`AbstractEntityAIHerder.java:81`, read at `:145`).
pub const BUTCHER_DELAY: i32 = 20;

/// `AbstractEntityAIHerder.LIMIT_TO_FEED_CHILDREN`: past this many children it
/// stops accelerating them (`AbstractEntityAIHerder.java:88`, read at `:548`).
pub const LIMIT_TO_FEED_CHILDREN: i32 = 10;

/// `AbstractEntityAIHerder.ACTIONS_FOR_DUMP`: the dump threshold of every herder
/// (`AbstractEntityAIHerder.java:93`, returned at `:153-156`).
pub const ACTIONS_FOR_DUMP: i32 = 10;

/// `AbstractEntityAIHerder.ANIMAL_MULTIPLIER`: how many animals one hut level
/// supports, `level × 2` (`AbstractEntityAIHerder.java:56`, read at `:603`, `:675`).
pub const ANIMAL_MULTIPLIER: i32 = 2;

/// `EntityAIWorkFarmer.XP_PER_HARVEST`: what one harvested crop is worth
/// (`EntityAIWorkFarmer.java:100`, granted at `:670`).
pub const XP_PER_HARVEST: f64 = 0.5;

/// `EntityAIWorkFarmer.DEFAULT_DELAY`: the slowest hoe/plant/harvest cycle, in
/// ticks (`EntityAIWorkFarmer.java:85`, read at `:692`).
pub const FARMER_DEFAULT_DELAY: i32 = 40;

/// `EntityAIWorkFarmer.SMALLEST_DELAY`: the fastest, at skill 78 and above
/// (`EntityAIWorkFarmer.java:90`, read at `:692`).
pub const FARMER_SMALLEST_DELAY: i32 = 1;

/// `EntityAIWorkComposter.BASE_XP_GAIN`: what inserting one item into a barrel is
/// worth, and what one harvest is worth (`EntityAIWorkComposter.java:49`, granted at
/// `:285` and `:346`).
pub const COMPOSTER_XP: f64 = 1.0;

/// `AbstractTileEntityBarrel.MAX_ITEMS`: how many compost points one barrel holds
/// (`api/tileentities/AbstractTileEntityBarrel.java:14`, filled by
/// `EntityAIWorkComposter.java:284`).
pub const BARREL_CAPACITY: i32 = 64;

/// `TileEntityBarrel.TIMER_END`: two Minecraft days, the time a full barrel takes
/// (`core/tileentities/TileEntityBarrel.java:50`, read at `:101`).
pub const BARREL_TIMER_END: i32 = 24000;

/// `TileEntityBarrel.AVERAGE_TICKS`: how the barrel spreads that timer
/// (`core/tileentities/TileEntityBarrel.java:54`, read at `:69`).
pub const BARREL_AVERAGE_TICKS: i32 = 20;

/// `EntityAIWorkFisherman.XP_PER_CATCH`, declared on the bobber and granted by both
/// the bobber and the worker (`NewBobberEntity.java:50`, `:535`,
/// `EntityAIWorkFisherman.java:719`).
pub const XP_PER_CATCH: f64 = 2.0;

/// `EntityAIWorkFisherman.MAX_FISHES_IN_INV`: it dumps after ten fish
/// (`EntityAIWorkFisherman.java:105`, read at `:228`).
pub const MAX_FISHES_IN_INV: i32 = 10;

/// `EntityAIWorkFisherman.MIN_DISTANCE_TO_WATER`: how close a water block must be to
/// fish from it (`EntityAIWorkFisherman.java:100`, read at `:612`).
pub const MIN_DISTANCE_TO_WATER: i32 = 3;

/// `EntityAIWorkFisherman.SEARCH_RANGE`: the pond search radius, tripled on a second
/// pass (`EntityAIWorkFisherman.java:115`, read at `:424`, `:443`).
pub const FISHERMAN_SEARCH_RANGE: i32 = 50;

/// The line where any worker's broken block pays its `0.05`
/// (`AbstractEntityAIInteract.java:234`).
const XP_PER_BLOCK_LINE: &str = "AbstractEntityAIInteract.java:234";

/// The nine farm jobs, in `Job::ALL` order of the family.
pub const RULES: &[JobRules] = &[
    // The chicken herder. The only job of the family with **no constant of its
    // own**: everything it does is `AbstractEntityAIHerder`'s. What it adds is a
    // pickup rule that fails at low skill — `rand.nextInt(primary) > 1` on a feather
    // or an egg (`JobChickenHerder.java:49-50`) — so at skill 0 or 1 it never picks
    // one up. Its breeding item is two wheat seeds
    // (`BuildingChickenHerder.java:65`), named by the module and not a tag.
    rules!(
        ChickenHerder,
        &[place(Spot::Stock, None, "WorldUtil.java:321")],
        [
            material(
                "minecraft:wheat_seeds",
                Role::Food,
                "BuildingChickenHerder.java:65"
            ),
            tool_at_floor(ToolKind::Axe, "AbstractEntityAIHerder.java:176"),
        ],
        [product(Output::Drops, Work::Ticks(BUTCHER_DELAY), "AbstractEntityAIHerder.java:850")],
        [constant("BUTCHER_DELAY", BUTCHER_DELAY, "AbstractEntityAIHerder.java:81")],
        [xp(XP_PER_ACTION, "XP_PER_ACTION", "AbstractEntityAIHerder.java:382")],
        dump(ACTIONS_FOR_DUMP, DumpScale::Fixed, "AbstractEntityAIHerder.java:153"),
        SATURATION_DEFAULT,
        "JobChickenHerder.java:1"
    ),
    // The composter. It fills **barrels** — one per barrel of its own schematic, the
    // code has no "layer" constant (`BuildingComposter.java:120-127`) — with up to
    // **64 points** of compostables, and a full barrel becomes **6 compost times a
    // multiplier** (`TileEntityBarrel.java:316-327`). The multiplier is a roll on
    // the secondary skill: `skill/2` percent for **2×**, twice that for **1.5×**,
    // four times it for **1.25×**, otherwise **1×**
    // (`EntityAIWorkComposter.java:359-378`). A full barrel needs **24 000 ticks**
    // (`TileEntityBarrel.java:50`) and the composter accelerates it by
    // `(skill / 10) * 2` ticks a second (`EntityAIWorkComposter.java:111`). The
    // `PRODUCE_DIRT` setting turns the output into one podzol (5 %) or one dirt
    // (`:316-339`, `BuildingModules.java:119`).
    rules!(
        Composter,
        &[place(Spot::Hut, None, "BuildingComposter.java:120")],
        [material(
            "#minecolonies:compostables",
            Role::Compost,
            "EntityAIWorkComposter.java:147"
        )],
        [product(
            Output::Fixed { item: "minecolonies:compost", count: 6 },
            Work::Own,
            "TileEntityBarrel.java:316"
        )],
        [
            constant("DECIDE_DELAY", 40, "EntityAIWorkComposter.java:69"),
            constant("AFTER_TASK_DELAY", 5, "EntityAIWorkComposter.java:74"),
            constant("TICKS_UNTIL_COMPLAIN", 12000, "EntityAIWorkComposter.java:59"),
            constant("TIMER_END", BARREL_TIMER_END, "TileEntityBarrel.java:50"),
            constant("AVERAGE_TICKS", BARREL_AVERAGE_TICKS, "TileEntityBarrel.java:54"),
        ],
        [xp(COMPOSTER_XP, "BASE_XP_GAIN", "EntityAIWorkComposter.java:285")],
        dump(1, DumpScale::Fixed, "EntityAIWorkComposter.java:381"),
        SATURATION_DEFAULT,
        "EntityAIWorkComposter.java:1"
    ),
    // The cowboy. It milks a cow or a goat and stews a mushroom cow, one bucket or
    // one bowl at a time, in **one tick** (`EntityAIWorkCowboy.java:70-71`), with a
    // **10-tick** cool-down between attempts (`:56`). Both actions pay **1.0 XP**,
    // not the herder's half (`:186`, `:244`). Its butchering damage is
    // `max(1, primary / 10)` (`:252-255`), not the base 5. The bucket-or-bottle it
    // consumes is a setting (`BuildingCowboy.java:120-127`) and its milking quota is
    // a module (`:255-266`).
    rules!(
        Cowboy,
        &[place(Spot::Stock, None, "WorldUtil.java:321")],
        [
            material("minecraft:wheat", Role::Food, "BuildingCowboy.java:153"),
            material("minecraft:bucket", Role::Material, "BuildingCowboy.java:120"),
            material("minecraft:bowl", Role::Material, "BuildingCowboy.java:134"),
            tool_at_floor(ToolKind::Axe, "AbstractEntityAIHerder.java:176"),
        ],
        [
            product(
                Output::Fixed { item: "minecraft:milk_bucket", count: 1 },
                Work::Ticks(1),
                "EntityAIWorkCowboy.java:173"
            ),
            product(
                Output::Fixed { item: "minecraft:mushroom_stew", count: 1 },
                Work::Ticks(1),
                "EntityAIWorkCowboy.java:229"
            ),
        ],
        [
            constant("MILK_COOL_DOWN", 10, "EntityAIWorkCowboy.java:56"),
            constant("COWBOY_MILK", 1, "EntityAIWorkCowboy.java:70"),
            constant("COWBOY_STEW", 1, "EntityAIWorkCowboy.java:71"),
            constant("DECIDING_DELAY", DECIDING_DELAY, "AbstractEntityAIHerder.java:82"),
        ],
        [
            xp(1.0, "addExperience(1.0)", "EntityAIWorkCowboy.java:186"),
            xp(XP_PER_ACTION, "XP_PER_ACTION", "AbstractEntityAIHerder.java:382"),
        ],
        dump(ACTIONS_FOR_DUMP, DumpScale::Fixed, "AbstractEntityAIHerder.java:153"),
        SATURATION_DEFAULT,
        "EntityAIWorkCowboy.java:1"
    ),
    // The farmer. Not a herder: he works **cropland**, as many fields as his hut has
    // levels (`BuildingFarmer.java:281-284`), each field a square whose radius is
    // the scarecrow's, **5 by default and 20 at most** (`FarmField.java:40-41`).
    // He hoes, plants and harvests in one cycle whose delay is
    // `max(1, 40 - skill / 2)` ticks (`EntityAIWorkFarmer.java:690-693`) — the
    // fastest job of the family at high skill — and he abandons a field after four
    // fruitless states (`:276`, `:574`). Compost or bone meal doubles growth
    // (`:804-856`). His mining skill is the **secondary** one, unlike everyone else
    // (`:885-888`).
    rules!(
        Farmer,
        &[place(Spot::Field, None, "EntityAIWorkFarmer.java:243")],
        [
            material("minecolonies:compost", Role::Compost, "EntityAIWorkFarmer.java:297"),
            material("minecraft:bone_meal", Role::Compost, "EntityAIWorkFarmer.java:297"),
            tool_at_floor(ToolKind::Hoe, "EntityAIWorkFarmer.java:256"),
        ],
        [product(Output::Drops, Work::Block, "EntityAIWorkFarmer.java:666")],
        [
            constant("DEFAULT_DELAY", FARMER_DEFAULT_DELAY, "EntityAIWorkFarmer.java:85"),
            constant("SMALLEST_DELAY", FARMER_SMALLEST_DELAY, "EntityAIWorkFarmer.java:90"),
        ],
        [
            xp(XP_PER_HARVEST, "XP_PER_HARVEST", "EntityAIWorkFarmer.java:670"),
            xp(XP_PER_BLOCK, "XP_PER_BLOCK", XP_PER_BLOCK_LINE),
        ],
        dump(64, DumpScale::Fixed, "EntityAIWorkFarmer.java:198"),
        SATURATION_HEAVY,
        "EntityAIWorkFarmer.java:1"
    ),
    // The fisherman. It stands within **3 blocks of water** (`:100`, `:612`), finds a
    // pond within **50 then 150 blocks** (`:115`, `:424`, `:443`), remembers up to
    // **20** of them (`:85`) and gives up on a bad angle after **6 rotations**
    // (`:110`). A catch pays **2 XP** from the vanilla fishing table
    // (`NewBobberEntity.java:50`, `:535`) and **2 more** if it landed on the
    // building-level bonus table (`EntityAIWorkFisherman.java:715-720`). It dumps
    // after ten fish (`:228`). The rod is its only stated tool (`:207`).
    rules!(
        Fisherman,
        &[place(Spot::Water, Some(MIN_DISTANCE_TO_WATER), "EntityAIWorkFisherman.java:612")],
        [tool_at_floor(ToolKind::Rod, "EntityAIWorkFisherman.java:207")],
        [product(Output::Drops, Work::Own, "EntityAIWorkFisherman.java:719")],
        [],
        [xp(XP_PER_CATCH, "XP_PER_CATCH", "EntityAIWorkFisherman.java:719")],
        dump(MAX_FISHES_IN_INV, DumpScale::Fixed, "EntityAIWorkFisherman.java:226"),
        SATURATION_DEFAULT,
        "EntityAIWorkFisherman.java:1"
    ),
    // The rabbit herder. Two constants of its own in the whole class, both
    // render metadata: its rule is the herder's, and its only addition is the
    // butchering roll `rand.nextInt(1 + (100 - primary) / 5) <= 1`
    // (`EntityAIWorkRabbitHerder.java:61`). Breeding pair: two carrots
    // (`BuildingModules.java:223`).
    rules!(
        RabbitHerder,
        &[place(Spot::Stock, None, "WorldUtil.java:321")],
        [
            material("minecraft:carrot", Role::Food, "BuildingModules.java:223"),
            tool_at_floor(ToolKind::Axe, "AbstractEntityAIHerder.java:176"),
        ],
        [product(Output::Drops, Work::Ticks(BUTCHER_DELAY), "AbstractEntityAIHerder.java:850")],
        [constant("BUTCHER_DELAY", BUTCHER_DELAY, "AbstractEntityAIHerder.java:81")],
        [xp(XP_PER_ACTION, "XP_PER_ACTION", "AbstractEntityAIHerder.java:382")],
        dump(ACTIONS_FOR_DUMP, DumpScale::Fixed, "AbstractEntityAIHerder.java:153"),
        SATURATION_DEFAULT,
        "EntityAIWorkRabbitHerder.java:1"
    ),
    // The shepherd. It shears a sheep for `1 + rand(Fortune × max(1, primary / 5))`
    // wool in the sheep's own colour (`EntityAIWorkShepherd.java:132-145`), twenty
    // ticks at a time (`:50`), and dyes it with probability `hut level / 100`
    // (`:173-187`). Its shears are conditional on the `SHEARING` setting
    // (`BuildingShepherd.java:40`, `EntityAIWorkShepherd.java:59-62`); the base tool
    // is the herder's axe. It butchers with its **secondary** skill, where the
    // cowboy uses its primary (`:89-93`).
    rules!(
        Shepherd,
        &[place(Spot::Stock, None, "WorldUtil.java:321")],
        [
            material("minecraft:wheat", Role::Food, "BuildingShepherd.java:98"),
            tool_at_floor(ToolKind::Axe, "AbstractEntityAIHerder.java:176"),
            tool_at_floor(ToolKind::Shears, "EntityAIWorkShepherd.java:59"),
        ],
        [product(Output::Drops, Work::Ticks(20), "EntityAIWorkShepherd.java:145")],
        [
            constant("SHEPHERD_SHEAR", 20, "EntityAIWorkShepherd.java:50"),
            constant("BUTCHER_DELAY", BUTCHER_DELAY, "AbstractEntityAIHerder.java:81"),
        ],
        [xp(XP_PER_ACTION, "XP_PER_ACTION", "EntityAIWorkShepherd.java:155")],
        dump(ACTIONS_FOR_DUMP, DumpScale::Fixed, "AbstractEntityAIHerder.java:153"),
        SATURATION_DEFAULT,
        "EntityAIWorkShepherd.java:1"
    ),
    // The stablemaster. The odd one out of the herd: it **makes mounts** out of
    // vanilla horses (`EntityAIWorkStableMaster.java:281`), so its product is an
    // entity and its row says so. It hunts mounts within **30 blocks**
    // (`:738`, `:754`), trains at 25 % per decision, readies at 40 % and rounds up
    // at 15 % (`:66-68`), keeps `level × 2` mounts (`:199`, `:270`), never butchers
    // (`:220-225`) and takes **only a lead** as a tool — at level `-1`, the one row
    // of the port where a tool has no level at all (`:158-170`,
    // `TOOL_LEVEL_HAND`). It eats through the `minecolonies:feed` tag and re-arms
    // with the `minecolonies:leather` tag (`:431`).
    rules!(
        Stablemaster,
        &[place(Spot::Stock, Some(30), "EntityAIWorkStableMaster.java:738")],
        [
            material("#minecolonies:feed", Role::Food, "EntityAIWorkStableMaster.java:431"),
            material("#minecolonies:leather", Role::Material, "EntityAIWorkStableMaster.java:431"),
            material("minecraft:golden_apple", Role::Food, "BuildingStable.java:98"),
            super::tool(ToolKind::Lead, TOOL_LEVEL_HAND, "EntityAIWorkStableMaster.java:160"),
        ],
        [product(Output::Entity("minecraft:horse"), Work::Ticks(10), "EntityAIWorkStableMaster.java:281")],
        [
            constant("HERDER_TRAIN", 10, "EntityAIWorkStableMaster.java:102"),
            constant("HERDER_READY_FOR_COMBAT", 10, "EntityAIWorkStableMaster.java:103"),
            constant("HERDER_GATHER_MOUNTS", 10, "EntityAIWorkStableMaster.java:104"),
            constant("MAX_ROUNDUP_COOLDOWN", 100, "EntityAIWorkStableMaster.java:72"),
            constant("MAX_TRAINING_COOLDOWN", 50, "EntityAIWorkStableMaster.java:75"),
        ],
        [xp(XP_PER_ACTION, "XP_PER_ACTION", "EntityAIWorkStableMaster.java:289")],
        dump(ACTIONS_FOR_DUMP, DumpScale::Fixed, "AbstractEntityAIHerder.java:153"),
        SATURATION_DEFAULT,
        "EntityAIWorkStableMaster.java:1"
    ),
    // The swineherd. No constant of its own either, and one rule it alone carries:
    // its citizens catch disease twice as fast (`JobSwineHerder.java:53-57`,
    // `getDiseaseModifier` answering `2`). Breeding pair: two carrots
    // (`BuildingModules.java:244`).
    rules!(
        SwineHerder,
        &[place(Spot::Stock, None, "WorldUtil.java:321")],
        [
            material("minecraft:carrot", Role::Food, "BuildingModules.java:244"),
            tool_at_floor(ToolKind::Axe, "AbstractEntityAIHerder.java:176"),
        ],
        [product(Output::Drops, Work::Ticks(BUTCHER_DELAY), "AbstractEntityAIHerder.java:850")],
        [constant("BUTCHER_DELAY", BUTCHER_DELAY, "AbstractEntityAIHerder.java:81")],
        [xp(XP_PER_ACTION, "XP_PER_ACTION", "AbstractEntityAIHerder.java:382")],
        dump(ACTIONS_FOR_DUMP, DumpScale::Fixed, "AbstractEntityAIHerder.java:153"),
        SATURATION_DEFAULT,
        "EntityAIWorkSwineHerder.java:1"
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(job: Job) -> &'static JobRules {
        super::super::rules_for(job).expect("a farm job has rules")
    }

    #[test]
    fn nine_farm_jobs_and_no_duplicate() {
        assert_eq!(RULES.len(), 9);
        let mut seen = Vec::new();
        for row in RULES {
            assert!(!seen.contains(&row.job), "{} twice", row.job.path());
            seen.push(row.job);
            assert_eq!(row.family, Family::Farm);
        }
        for job in [
            Job::ChickenHerder,
            Job::Composter,
            Job::Cowboy,
            Job::Farmer,
            Job::Fisherman,
            Job::RabbitHerder,
            Job::Shepherd,
            Job::Stablemaster,
            Job::SwineHerder,
        ] {
            assert!(seen.contains(&job), "{} is missing", job.path());
        }
    }

    #[test]
    fn the_six_herders_share_one_xp_grant_and_the_farmer_and_fisherman_do_not() {
        for job in [
            Job::ChickenHerder,
            Job::RabbitHerder,
            Job::Shepherd,
            Job::SwineHerder,
            Job::Stablemaster,
        ] {
            assert_eq!(rules(job).xp[0].per_action, XP_PER_ACTION, "{}", job.path());
        }
        assert_eq!(rules(Job::Cowboy).xp[1].per_action, XP_PER_ACTION);
        assert_eq!(rules(Job::Cowboy).xp[0].per_action, 1.0, "milking pays double");
        assert_eq!(rules(Job::Farmer).xp[0].per_action, XP_PER_HARVEST);
        assert_eq!(rules(Job::Fisherman).xp[0].per_action, XP_PER_CATCH);
        assert_eq!(rules(Job::Composter).xp[0].per_action, COMPOSTER_XP);
    }

    #[test]
    fn every_herder_dumps_after_ten_actions_except_the_composter() {
        for job in [
            Job::ChickenHerder,
            Job::Cowboy,
            Job::RabbitHerder,
            Job::Shepherd,
            Job::Stablemaster,
            Job::SwineHerder,
        ] {
            assert_eq!(rules(job).dump.at(4), Some(ACTIONS_FOR_DUMP), "{}", job.path());
        }
        assert_eq!(rules(Job::Composter).dump.at(4), Some(1));
        assert_eq!(rules(Job::Farmer).dump.at(4), Some(64));
        assert_eq!(rules(Job::Fisherman).dump.at(4), Some(MAX_FISHES_IN_INV));
    }

    #[test]
    fn the_enclosure_is_the_building_box_and_only_two_jobs_state_a_distance() {
        for job in [
            Job::ChickenHerder,
            Job::Cowboy,
            Job::RabbitHerder,
            Job::Shepherd,
            Job::SwineHerder,
        ] {
            let place = rules(job).places[0];
            assert_eq!(place.spot, Spot::Stock, "{}", job.path());
            assert_eq!(place.reach, None, "{} has no radius", job.path());
        }
        assert_eq!(rules(Job::Stablemaster).places[0].reach, Some(30));
        assert_eq!(rules(Job::Fisherman).places[0].reach, Some(MIN_DISTANCE_TO_WATER));
        assert_eq!(FISHERMAN_SEARCH_RANGE, 50, "the pond search, not the fishing reach");
    }

    #[test]
    fn the_three_herders_with_no_constants_of_their_own_borrow_every_duration() {
        // chicken, swine and rabbit add nothing: their only duration is the shared
        // butchering period, and its line is the herder base class's
        for job in [Job::ChickenHerder, Job::SwineHerder, Job::RabbitHerder] {
            let row = rules(job);
            assert_eq!(row.durations.len(), 1, "{}", job.path());
            assert_eq!(row.durations[0].name, "BUTCHER_DELAY");
            assert_eq!(row.durations[0].value, BUTCHER_DELAY);
            assert!(row.durations[0].source.starts_with("AbstractEntityAIHerder"));
        }
        // and the three that do have their own numbers say so
        assert!(rules(Job::Cowboy).durations.len() > 1);
        assert!(rules(Job::Shepherd).durations.len() > 1);
        assert!(rules(Job::Stablemaster).durations.len() > 1);
    }

    #[test]
    fn the_stablemaster_produces_a_mount_and_is_the_one_tool_with_no_level() {
        let stable = rules(Job::Stablemaster);
        assert_eq!(stable.produces[0].output, Output::Entity("minecraft:horse"));
        let lead = stable
            .accepts
            .iter()
            .find(|accepted| matches!(accepted.role, Role::Tool(_)))
            .expect("a lead");
        assert_eq!(lead.item, "lead");
        assert_eq!(lead.tool_level, Some(TOOL_LEVEL_HAND), "checkForToolOrWeapon(tool, -1)");
        assert_eq!(TOOL_LEVEL_HAND, -1);
        // every other tool row in the family is at the floor, 0
        for row in RULES {
            if row.job == Job::Stablemaster {
                continue;
            }
            for accepted in row.accepts {
                if matches!(accepted.role, Role::Tool(_)) {
                    assert_eq!(accepted.tool_level, Some(0), "{}", row.job.path());
                }
            }
        }
    }

    #[test]
    fn the_composter_is_the_only_job_whose_product_is_counted_in_points() {
        let composter = rules(Job::Composter);
        assert_eq!(
            composter.produces[0].output,
            Output::Fixed { item: "minecolonies:compost", count: 6 },
            "TileEntityBarrel.java:316-327 gives six"
        );
        assert_eq!(BARREL_CAPACITY, 64);
        assert_eq!(BARREL_TIMER_END, 24_000, "two Minecraft days");
        assert_eq!(composter.durations[3].name, "TIMER_END");
        assert_eq!(composter.durations[3].value, BARREL_TIMER_END);
        assert_eq!(composter.durations[4].name, "AVERAGE_TICKS");
    }

    #[test]
    fn the_farmer_is_the_one_heavy_job_of_the_family_and_the_only_hoe() {
        assert_eq!(rules(Job::Farmer).saturation, SATURATION_HEAVY);
        for row in RULES {
            if row.job != Job::Farmer {
                assert_eq!(row.saturation, SATURATION_DEFAULT, "{}", row.job.path());
            }
        }
        let hoes = RULES
            .iter()
            .filter(|row| {
                row.accepts
                    .iter()
                    .any(|accepted| matches!(accepted.role, Role::Tool(ToolKind::Hoe)))
            })
            .map(|row| row.job.path())
            .collect::<Vec<&str>>();
        assert_eq!(hoes, vec!["farmer"]);
        assert_eq!(rules(Job::Farmer).job.tools(), &[ToolKind::Hoe]);
    }

    #[test]
    fn the_farmer_s_cycle_is_forty_ticks_and_ends_at_one() {
        assert_eq!(FARMER_DEFAULT_DELAY, 40);
        assert_eq!(FARMER_SMALLEST_DELAY, 1);
        // max(1, 40 - skill / 2), Java's integer division
        let delay = |skill: i32| (FARMER_DEFAULT_DELAY - skill / 2).max(FARMER_SMALLEST_DELAY);
        assert_eq!(delay(0), 40);
        assert_eq!(delay(20), 30);
        assert_eq!(delay(78), 1);
        assert_eq!(delay(99), 1);
        assert_eq!(rules(Job::Farmer).durations[0].value, 40);
        assert_eq!(rules(Job::Farmer).durations[1].value, 1);
    }

    #[test]
    fn the_breeding_constants_of_the_herders_are_declared_once_for_six_jobs() {
        assert_eq!(NUM_OF_ANIMALS_TO_BREED, 2);
        assert_eq!(EXTRA_BREEDING_ITEMS_REQUEST, 8);
        assert_eq!(DISTANCE_TO_BREED, 10);
        assert_eq!(ANIMAL_MULTIPLIER, 2);
        assert_eq!(LIMIT_TO_FEED_CHILDREN, 10);
        // mounts a hut of level 5 supports: 5 * 2
        assert_eq!(5 * ANIMAL_MULTIPLIER, 10);
        // a breeding pair is 2, and a herder asks for 8 more
        assert_eq!(NUM_OF_ANIMALS_TO_BREED + EXTRA_BREEDING_ITEMS_REQUEST, 10);
        // and the butchering damage the herders that do not compute one swing for
        assert_eq!(BUTCHERING_ATTACK_DAMAGE, 5);
    }
}
