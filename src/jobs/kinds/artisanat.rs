//! `artisanat` — the twelve crafters: what each hut accepts, what it makes, and
//! the one formula that says how long a recipe takes.
//!
//! Java reference: the twelve `Job*` classes under `core/colony/jobs/`, the
//! twelve AI classes under `core/entity/ai/workers/crafting/` (plus
//! `service/EntityAIWorkCook.java` and `service/EntityAIWorkEnchanter.java`),
//! and the four building classes that carry the input filters —
//! `BuildingSawmill`, `BuildingStonemason`, `BuildingBlacksmith`, `BuildingBaker`,
//! `BuildingKitchen`, `BuildingCook`, `BuildingDyer`, `BuildingFletcher`,
//! `BuildingGlassblower`, `BuildingMechanic`, `BuildingAlchemist`,
//! `BuildingEnchanter`.
//!
//! **The finding of this family: eleven of the twelve have no duration of their
//! own.** `EntityAIWorkSawmill` is 45 lines and declares not one `static final
//! int`; `EntityAIWorkStonemason` is 28; `EntityAIWorkBlacksmith` 33. Every tick
//! of their work comes from `AbstractEntityAICrafting`, through
//! [`super::crafting_ticks`], so the "combien de temps de travail" column of this
//! family is **one step function** and not twelve numbers:
//!
//! | crafting skill level | hits per recipe | ticks |
//! | --- | --- | --- |
//! | 0-1 | 30 | 300 |
//! | 2-3 | 15 | 150 |
//! | 4-5 | 9 | 90 |
//! | 6-15 | 6 | 60 |
//! | 16-19 | 3 | 30 |
//! | 20-99 | 1 | 10 |
//!
//! Java's integer division (`AbstractEntityAICrafting.java:736-737`) is what makes
//! the last row a cliff rather than a slope: `10 / 11 == 0`, so a level-20
//! crafter finishes a recipe on its first hit. [`super::crafting_hits`] is that
//! function and its doc carries the arithmetic; no row here repeats it.
//!
//! **Two sub-families, and the difference is a furnace.**
//!
//! - **Bench only** — sawmill, stonemason, blacksmith, fletcher, mechanic: they
//!   extend `AbstractEntityAICrafting` and their work is the progress loop.
//! - **Bench and furnace** — baker, chef, dyer, glassblower: they extend
//!   `AbstractEntityAIRequestSmelter`, so they *also* fire what the recipe makes
//!   and earn [`super::SMELTER_XP`] per extraction
//!   (`AbstractEntityAIRequestSmelter.java:622`). Their row has two products for
//!   that reason.
//!
//! Two members are not crafters at all in the shape the others are:
//! `EntityAIWorkAlchemist` works brewing stands and harvests nether wart, and
//! `EntityAIWorkCook` is a **waiter** — `JobCook` is a plain `AbstractJob`, not an
//! `AbstractJobCrafter` (`JobCook.java:12`), and it never runs the progress loop.
//! Both are here because they are hired by a hut of this family and their rule is
//! a craft in the ordinary sense; both say so in their comment.
//!
//! **What a crafter accepts is a *recipe* question, not an item question.** The
//! eight bench crafters take any stack the recipe needs; what they refuse is
//! decided by the recipes the hut will accept, and Java expresses that as an
//! **item tag named after the job** — `minecolonies:sawmill`,
//! `minecolonies:stonemason`, `minecolonies:fletcher`… (`TagConstants.java:59-72`,
//! one constant per job, its value the job's own path). Those tags are data
//! (`data/minecolonies/tags/item/`), not code, so each row names its tag in the
//! comment and this module does not copy the contents. The two huts that filter
//! *by item class* rather than by tag — the blacksmith and the fletcher — say so
//! with lines.
//!
//! **Every crafter dumps after one action** (`AbstractEntityAICrafting.java:724-727`),
//! so the real cost of a recipe is the work plus a walk to the warehouse; the
//! four that fire as well inherit the same one
//! (`AbstractEntityAIRequestSmelter` does not override it). That is why every row
//! below writes `dump(1, ..)` and not [`super::ACTIONS_UNTIL_DUMP`].

use super::{
    constant, dump, material, place, product, tool_at_floor, xp, DumpScale, Family, JobRules,
    Output, Role, Spot, Work, SATURATION_DEFAULT, WORK_IN_BUILDING_DIST,
};
use crate::jobs::job::Job;
use crate::jobs::tool::ToolKind;

/// Build one row of [`RULES`]; the family is fixed by the file.
macro_rules! rules {
    ($job:ident, $places:expr, [$($accepted:expr),* $(,)?], [$($product:expr),* $(,)?],
     [$($duration:expr),* $(,)?], [$($xp:expr),* $(,)?], $dump:expr, $saturation:expr,
     $source:literal) => {
        JobRules {
            job: Job::$job,
            family: Family::Craft,
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

/// One XP grant of half an item: `currentRequest.getRequest().getCount() / 2.0`
/// (`AbstractEntityAICrafting.java:643`) is paid **per finished batch**, so the
/// honest per-unit number is a half. Named by the Java expression, because Java
/// gives it no constant name.
const XP_PER_CRAFTED_ITEM: f64 = 0.5;

/// The line of the one grant every bench crafter shares
/// (`AbstractEntityAICrafting.java:643`).
const CRAFT_XP_LINE: &str = "AbstractEntityAICrafting.java:643";

/// The line of the progress loop's craft step
/// (`AbstractEntityAICrafting.java:566`, `executeCraftingAction`).
const CRAFT_STEP_LINE: &str = "AbstractEntityAICrafting.java:566";

/// The line of the "dump after one action" override
/// (`AbstractEntityAICrafting.java:724-727`).
const CRAFT_DUMP_LINE: &str = "AbstractEntityAICrafting.java:724";

/// The line of the tagged work position a crafter walks to
/// (`AbstractEntityAICrafting.java:523`, `walkToTaggedWorkPos`).
const CRAFT_WORK_POS_LINE: &str = "AbstractEntityAICrafting.java:523";

/// The line where a smelter stops when its building has no furnace
/// (`AbstractEntityAIRequestSmelter.java:439-448`).
const SMELTER_FURNACE_LINE: &str = "AbstractEntityAIRequestSmelter.java:439";

/// The tag a hut's recipes must produce — spelled the way
/// `TagConstants.java:59-72` builds it, `<namespace>:<job path>`.
///
/// The tags themselves are data (`data/minecolonies/tags/item/`), so this
/// function returns the name and not the contents.

/// The twelve crafters, in `Job::ALL` order of the family.
pub const RULES: &[JobRules] = &[
    // The baker. Two products, because its hut both crafts and fires
    // (`EntityAIWorkBaker` extends `AbstractEntityAIRequestSmelter`). Three gates
    // Java states and the recipe data cannot: **nothing works below hut level 3**
    // (`BuildingBaker.java:163-166`, `:219-222` both answer an empty set of
    // supported types), **wheat is never food** for it (`:93-96`), and it keeps
    // `RECIPE_INPUT_HOLD = 128` of the current recipe's inputs (`:132`). Recipe
    // tag: `minecolonies:baker` (`TagConstants.java:59`).
    rules!(
        Baker,
        &[
            place(Spot::Bench, Some(WORK_IN_BUILDING_DIST), CRAFT_WORK_POS_LINE),
            place(Spot::Furnace, None, SMELTER_FURNACE_LINE),
        ],
        [
            material("#minecolonies:baker", Role::Material, "BuildingBaker.java:148"),
            material("minecraft:wheat", Role::Food, "BuildingBaker.java:93"),
        ],
        [
            product(Output::Recipe, Work::Craft, CRAFT_STEP_LINE),
            product(Output::Recipe, Work::Smelt, "AbstractEntityAIRequestSmelter.java:622"),
        ],
        [],
        [
            xp(5.0, "BASE_XP_GAIN", "AbstractEntityAIRequestSmelter.java:622"),
            xp(XP_PER_CRAFTED_ITEM, "currentRequest.getRequest().getCount() / 2.0", CRAFT_XP_LINE),
        ],
        dump(1, DumpScale::Fixed, CRAFT_DUMP_LINE),
        SATURATION_DEFAULT,
        "EntityAIWorkBaker.java:1"
    ),
    // The blacksmith. The only hut that filters **by equipment class** rather than
    // by tag: it takes an output whose type is one of the eleven equipment kinds
    // (`BuildingBlacksmith.java:87-97`) and refuses any recipe whose inputs
    // contain leather, "that is the fletcher's job"
    // (`BuildingBlacksmith.java:80-85`). Fishing rods and flint-and-steel are
    // deliberately outside the accepted kinds (`:98`). Recipe tag:
    // `minecolonies:blacksmith` (`TagConstants.java:60`).
    rules!(
        Blacksmith,
        &[place(Spot::Bench, Some(WORK_IN_BUILDING_DIST), CRAFT_WORK_POS_LINE)],
        [
            material("#minecolonies:blacksmith", Role::Material, "BuildingBlacksmith.java:71"),
            material("minecraft:leather", Role::Material, "BuildingBlacksmith.java:80"),
        ],
        [product(Output::Recipe, Work::Craft, CRAFT_STEP_LINE)],
        [],
        [xp(XP_PER_CRAFTED_ITEM, "currentRequest.getRequest().getCount() / 2.0", CRAFT_XP_LINE)],
        dump(1, DumpScale::Fixed, CRAFT_DUMP_LINE),
        SATURATION_DEFAULT,
        "EntityAIWorkBlacksmith.java:1"
    ),
    // The chef — a kitchen, not a restaurant: it crafts and fires food for the
    // colony's warehouse. It accepts any recipe whose input is in
    // `minecolonies:cook` (`BuildingKitchen.java:124`, `:133`) **or** whose output
    // is edible, directly or after smelting (`:137-139`, `:167`), and it excludes
    // its own current recipe's stacks from its food list (`:93-101`). Recipe tag:
    // `minecolonies:cook` (`TagConstants.java:61`).
    rules!(
        Chef,
        &[
            place(Spot::Bench, Some(WORK_IN_BUILDING_DIST), CRAFT_WORK_POS_LINE),
            place(Spot::Furnace, None, SMELTER_FURNACE_LINE),
        ],
        [material("#minecolonies:cook", Role::Material, "BuildingKitchen.java:124")],
        [
            product(Output::Recipe, Work::Craft, CRAFT_STEP_LINE),
            product(Output::Recipe, Work::Smelt, "AbstractEntityAIRequestSmelter.java:622"),
        ],
        [],
        [
            xp(5.0, "BASE_XP_GAIN", "AbstractEntityAIRequestSmelter.java:622"),
            xp(XP_PER_CRAFTED_ITEM, "currentRequest.getRequest().getCount() / 2.0", CRAFT_XP_LINE),
        ],
        dump(1, DumpScale::Fixed, CRAFT_DUMP_LINE),
        SATURATION_DEFAULT,
        "EntityAIWorkChef.java:1"
    ),
    // The waiter. **Not a crafter**: `JobCook` is a plain `AbstractJob`
    // (`JobCook.java:12`) and its AI extends `AbstractEntityAIUsesFurnace`, so it
    // runs no progress loop at all — it fires food and serves it. It serves
    // `building.getBuildingLevel() * SATURATION_TO_SERVE` where the constant is 16
    // (`EntityAIWorkCook.java:59`, read at `:288`), waits `SERVE_DELAY = 30` ticks
    // between two servings (`:64`, used at `:96-97`), only feeds a player below
    // food level `LEVEL_TO_FEED_PLAYER = 10` (`:69`, read at `:343`), and tries up
    // to ten citizens in a row (`:192`). Food must be `ISCOOKABLE` and its result
    // must be on the restaurant's menu (`:130-134`). The recipe tag
    // `minecolonies:cook` (`TagConstants.java:61`) marks the dishes it serves.
    rules!(
        Cook,
        &[
            place(Spot::Furnace, None, "AbstractEntityAIUsesFurnace.java:201"),
            place(Spot::Hut, None, "EntityAIWorkCook.java:341"),
        ],
        [
            material("#minecolonies:cook", Role::Material, "EntityAIWorkCook.java:130"),
            material("#minecolonies:itemlist_fuel", Role::Fuel, "AbstractEntityAIUsesFurnace.java:193"),
        ],
        [product(Output::Recipe, Work::Smelt, "EntityAIWorkCook.java:130")],
        [constant("SERVE_DELAY", 30, "EntityAIWorkCook.java:64")],
        [xp(2.0, "BASE_XP_GAIN", "EntityAIWorkCook.java:244")],
        dump(1, DumpScale::Fixed, "EntityAIWorkCook.java:427"),
        SATURATION_DEFAULT,
        "EntityAIWorkCook.java:1"
    ),
    // The dyer. The most item-specific hut of the family: sixteen dye tags
    // (`BuildingDyer.java:110-114`), `WHITE_WOOL` for its own un-dyeing recipe
    // (`:148`, `:188`, `:195`, `:243`) with `WHITE_DYE` as the ingredient
    // (`:242`), any dyeable leather (`:117`) and any wool that is not white
    // (`:227-228`). **It never improves a recipe** — `improveRecipe` is
    // deliberately empty (`:98-101`, `:281-284`) — and its wool path needs a
    // warehouse, answering `null` without one (`:152-155`). Recipe tags:
    // `minecolonies:dyer` and `minecolonies:dyer_smelting`
    // (`TagConstants.java:62-63`).
    rules!(
        Dyer,
        &[
            place(Spot::Bench, Some(WORK_IN_BUILDING_DIST), CRAFT_WORK_POS_LINE),
            place(Spot::Furnace, None, SMELTER_FURNACE_LINE),
        ],
        [
            material("#minecolonies:dyer", Role::Material, "BuildingDyer.java:86"),
            material("minecraft:white_wool", Role::Material, "BuildingDyer.java:148"),
            material("minecraft:white_dye", Role::Material, "BuildingDyer.java:242"),
            material("#minecraft:wool", Role::Material, "BuildingDyer.java:227"),
        ],
        [
            product(Output::Recipe, Work::Craft, CRAFT_STEP_LINE),
            product(Output::Recipe, Work::Smelt, "AbstractEntityAIRequestSmelter.java:622"),
        ],
        [],
        [
            xp(5.0, "BASE_XP_GAIN", "AbstractEntityAIRequestSmelter.java:622"),
            xp(XP_PER_CRAFTED_ITEM, "currentRequest.getRequest().getCount() / 2.0", CRAFT_XP_LINE),
        ],
        dump(1, DumpScale::Fixed, CRAFT_DUMP_LINE),
        SATURATION_DEFAULT,
        "EntityAIWorkDyer.java:1"
    ),
    // The fletcher. Filters by item class: an output that is an arrow, or a dyeable
    // armour of leather (`BuildingFletcher.java:83-86`). **The one crafter that
    // works in the rain** (`BuildingModules.java:306`). Recipe tag:
    // `minecolonies:fletcher` (`TagConstants.java:65`).
    rules!(
        Fletcher,
        &[place(Spot::Bench, Some(WORK_IN_BUILDING_DIST), CRAFT_WORK_POS_LINE)],
        [
            material("#minecolonies:fletcher", Role::Material, "BuildingFletcher.java:71"),
            material("#minecraft:arrows", Role::Projectile, "BuildingFletcher.java:83"),
        ],
        [product(Output::Recipe, Work::Craft, CRAFT_STEP_LINE)],
        [],
        [xp(XP_PER_CRAFTED_ITEM, "currentRequest.getRequest().getCount() / 2.0", CRAFT_XP_LINE)],
        dump(1, DumpScale::Fixed, CRAFT_DUMP_LINE),
        SATURATION_DEFAULT,
        "EntityAIWorkFletcher.java:1"
    ),
    // The glassblower. Crafts glass and fires it, and names no item in code: both
    // halves are tags. Recipe tags: `minecolonies:glassblower` and
    // `minecolonies:glassblower_smelting` (`TagConstants.java:66-67`, the second
    // built by appending `_smelting` to the first).
    rules!(
        Glassblower,
        &[
            place(Spot::Bench, Some(WORK_IN_BUILDING_DIST), CRAFT_WORK_POS_LINE),
            place(Spot::Furnace, None, SMELTER_FURNACE_LINE),
        ],
        [material(
            "#minecolonies:glassblower",
            Role::Material,
            "BuildingGlassblower.java:70"
        )],
        [
            product(Output::Recipe, Work::Craft, CRAFT_STEP_LINE),
            product(Output::Recipe, Work::Smelt, "AbstractEntityAIRequestSmelter.java:622"),
        ],
        [],
        [
            xp(5.0, "BASE_XP_GAIN", "AbstractEntityAIRequestSmelter.java:622"),
            xp(XP_PER_CRAFTED_ITEM, "currentRequest.getRequest().getCount() / 2.0", CRAFT_XP_LINE),
        ],
        dump(1, DumpScale::Fixed, CRAFT_DUMP_LINE),
        SATURATION_DEFAULT,
        "EntityAIWorkGlassblower.java:1"
    ),
    // The mechanic. Takes any minecart or hopper block as an output
    // (`BuildingMechanic.java:92-94`), and its warehouse module accepts every input
    // no other hut claims (`:119-126`, `handled.negate()`). Recipe tag:
    // `minecolonies:mechanic` (`TagConstants.java:68`).
    rules!(
        Mechanic,
        &[place(Spot::Bench, Some(WORK_IN_BUILDING_DIST), CRAFT_WORK_POS_LINE)],
        [
            material("#minecolonies:mechanic", Role::Material, "BuildingMechanic.java:80"),
            material("#minecraft:minecarts", Role::Material, "BuildingMechanic.java:92"),
        ],
        [product(Output::Recipe, Work::Craft, CRAFT_STEP_LINE)],
        [],
        [xp(XP_PER_CRAFTED_ITEM, "currentRequest.getRequest().getCount() / 2.0", CRAFT_XP_LINE)],
        dump(1, DumpScale::Fixed, CRAFT_DUMP_LINE),
        SATURATION_DEFAULT,
        "EntityAIWorkMechanic.java:1"
    ),
    // The sawmill. The only bench crafter with a **composition floor**: a recipe is
    // only queued when at least `MIN_PERCENTAGE_TO_CRAFT = 0.75` of its inputs are
    // wood (`BuildingSawmill.java:39`, read at `:119`). It accepts `#minecraft:logs`
    // and `#minecraft:planks`, and any other tag whose path contains "wood"
    // (`:102`, `:107-113`). Recipe tag: `minecolonies:sawmill`
    // (`TagConstants.java:70`).
    rules!(
        Sawmill,
        &[place(Spot::Bench, Some(WORK_IN_BUILDING_DIST), CRAFT_WORK_POS_LINE)],
        [
            material("#minecraft:logs", Role::Material, "BuildingSawmill.java:102"),
            material("#minecraft:planks", Role::Material, "BuildingSawmill.java:102"),
        ],
        [product(Output::Recipe, Work::Craft, CRAFT_STEP_LINE)],
        [],
        [xp(XP_PER_CRAFTED_ITEM, "currentRequest.getRequest().getCount() / 2.0", CRAFT_XP_LINE)],
        dump(1, DumpScale::Fixed, CRAFT_DUMP_LINE),
        SATURATION_DEFAULT,
        "EntityAIWorkSawmill.java:1"
    ),
    // The stonemason. The strictest filter of the twelve: a recipe is compatible
    // **only** if its output is in `minecolonies:stonemason`, `.orElse(false)` and
    // no fallback (`BuildingStonemason.java:77`). Recipe tag:
    // `minecolonies:stonemason` (`TagConstants.java:71`).
    rules!(
        StoneMason,
        &[place(Spot::Bench, Some(WORK_IN_BUILDING_DIST), CRAFT_WORK_POS_LINE)],
        [material(
            "#minecolonies:stonemason",
            Role::Material,
            "BuildingStonemason.java:69"
        )],
        [product(Output::Recipe, Work::Craft, CRAFT_STEP_LINE)],
        [],
        [xp(XP_PER_CRAFTED_ITEM, "currentRequest.getRequest().getCount() / 2.0", CRAFT_XP_LINE)],
        dump(1, DumpScale::Fixed, CRAFT_DUMP_LINE),
        SATURATION_DEFAULT,
        "EntityAIWorkStonemason.java:1"
    ),
    // The alchemist. Works **brewing stands**, not a bench, and its craft list is
    // empty by construction: `getSupportedCraftingTypes` answers an empty set and
    // `addRecipe` always refuses, so only its custom magic-potion recipe is ever
    // made (`BuildingAlchemist.java:249`, `:253-256`). It keeps
    // `BREWING_MIN_FUEL_COUNT = 8` blaze powder per stand
    // (`EntityAIWorkAlchemist.java:543`, `:561`, `:622`), plants `SOUL_SAND` and
    // `NETHER_WART` (`:136`, `:138`) and asks for sixteen nether wart at a time
    // (`:145`, `:191`). Two numbers that look like durations and are **not**: the
    // 30 of `DELAY_TO_HARVEST_NETHERWART` and `DELAY_TO_HARVEST_MISTLETOE`
    // (`:71`, `:76`) feed a `nextInt(30) <= 1` roll — one harvest in thirty — and
    // the mistletoe roll is `nextInt(40) <= 0`, one in forty (`:286`). They are
    // chances, so they are not in `durations`, which counts ticks.
    rules!(
        Alchemist,
        &[
            place(Spot::Still, None, "BuildingAlchemist.java:98"),
            place(Spot::Hut, None, "EntityAIWorkAlchemist.java:136"),
        ],
        [
            material("minecraft:blaze_powder", Role::Fuel, "EntityAIWorkAlchemist.java:539"),
            material("minecraft:soul_sand", Role::Material, "EntityAIWorkAlchemist.java:136"),
            material("minecraft:nether_wart", Role::Material, "EntityAIWorkAlchemist.java:138"),
            material("minecolonies:mistletoe", Role::Material, "EntityAIWorkAlchemist.java:289"),
            tool_at_floor(ToolKind::Shears, "EntityAIWorkAlchemist.java:236"),
        ],
        [product(Output::Recipe, Work::Craft, "EntityAIWorkAlchemist.java:841")],
        [],
        [xp(5.0, "BASE_XP_GAIN", "EntityAIWorkAlchemist.java:842")],
        dump(1, DumpScale::Fixed, "EntityAIWorkAlchemist.java:342"),
        SATURATION_DEFAULT,
        "EntityAIWorkAlchemist.java:1"
    ),
    // The enchanter. **The only job of the family with real tick constants and no
    // work block**: it walks between the colony's registered buildings, drains
    // mana from them and turns `minecolonies:ancient_tome` into enchanted books.
    // It drains only a building at least `MIN_DISTANCE_TO_DRAIN = 10` blocks away
    // (`EntityAIWorkEnchanter.java:68`, read at `:363`); an enchantment runs for
    // `MAX_ENCHANTMENT_TICKS = 300 / building level` ticks (`:78`, read at `:235`);
    // a drain pays `XP_PER_DRAIN = 10` (`:88`, granted at `:427`) and one mana
    // level (`:426`); it only works after 6 000 ticks of the day (`:130`); and it
    // waits `MAX_WAITING_TICKS = 60` before giving up on a target
    // (`JobEnchanter.java:22`, read at `:112`). `getSupportedCraftingTypes` is
    // empty and `addRecipe` refuses, so the enchanted book comes from custom
    // recipes only (`BuildingEnchanter.java:67-72`).
    rules!(
        Enchanter,
        &[place(Spot::Hut, None, "EntityAIWorkEnchanter.java:145")],
        [
            material("minecolonies:ancient_tome", Role::Material, "EntityAIWorkEnchanter.java:58"),
            material("minecraft:book", Role::Material, "EntityAIWorkEnchanter.java:63"),
            material("minecraft:enchanted_book", Role::Material, "EntityAIWorkEnchanter.java:292"),
        ],
        [product(Output::Recipe, Work::Craft, "EntityAIWorkEnchanter.java:485")],
        [
            constant("MAX_PROGRESS_TICKS", 60, "EntityAIWorkEnchanter.java:73"),
            constant("MAX_ENCHANTMENT_TICKS", 300, "EntityAIWorkEnchanter.java:78"),
            constant("MAX_WAITING_TICKS", 60, "JobEnchanter.java:22"),
        ],
        [
            xp(10.0, "XP_PER_DRAIN", "EntityAIWorkEnchanter.java:427"),
            xp(XP_PER_CRAFTED_ITEM, "currentRequest.getRequest().getCount() / 2.0", CRAFT_XP_LINE),
        ],
        dump(1, DumpScale::Fixed, "EntityAIWorkEnchanter.java:215"),
        SATURATION_DEFAULT,
        "EntityAIWorkEnchanter.java:1"
    ),
];

/// The recipe tag of one crafter, or `None` when the hut does not gate its
/// recipes by a tag.
///
/// Java builds the tag from the job's own path (`TagConstants.java:59-72`), so
/// this is `minecolonies:<path>` for the nine that have one; the cook's dishes are
/// in the same family but the waiter is a furnace user, not a crafter.
pub fn recipe_tag(job: Job) -> Option<&'static str> {
    match job {
        Job::Baker => Some(concat!("minecolonies:", "baker")),
        Job::Blacksmith => Some(concat!("minecolonies:", "blacksmith")),
        // `CRAFTING_COOK` is the *waiter's* path, and the kitchen is the hut that
        // reads it (`TagConstants.java:61`, read at `BuildingKitchen.java:124`).
        Job::Chef => Some(concat!("minecolonies:", "cook")),
        Job::Dyer => Some(concat!("minecolonies:", "dyer")),
        Job::Fletcher => Some(concat!("minecolonies:", "fletcher")),
        Job::Glassblower => Some(concat!("minecolonies:", "glassblower")),
        Job::Mechanic => Some(concat!("minecolonies:", "mechanic")),
        Job::Sawmill => Some(concat!("minecolonies:", "sawmill")),
        Job::StoneMason => Some(concat!("minecolonies:", "stonemason")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::job::Job;

    fn rules(job: Job) -> &'static JobRules {
        super::super::rules_for(job).expect("a crafter has rules")
    }

    #[test]
    fn twelve_crafters_and_no_duplicate() {
        assert_eq!(RULES.len(), 12);
        let mut seen = Vec::new();
        for row in RULES {
            assert!(!seen.contains(&row.job), "{} twice", row.job.path());
            seen.push(row.job);
            assert_eq!(row.family, Family::Craft);
        }
        for job in Job::ALL {
            let hired_here = job.path() == "baker"
                || job.path() == "blacksmith"
                || job.path() == "chef"
                || job.path() == "cook"
                || job.path() == "dyer"
                || job.path() == "fletcher"
                || job.path() == "glassblower"
                || job.path() == "mechanic"
                || job.path() == "sawmill"
                || job.path() == "stonemason"
                || job.path() == "alchemist"
                || job.path() == "enchanter";
            assert_eq!(seen.contains(&job), hired_here, "{}", job.path());
        }
    }

    #[test]
    fn every_crafter_works_at_a_bench_and_the_four_furnace_ones_also_at_a_furnace() {
        for row in RULES {
            let spots: Vec<Spot> = row.places.iter().map(|place| place.spot).collect();
            // the eleven crafters sit at a bench (or an alchemist's still); the waiter
            // is a furnace user and has neither
            if row.job == Job::Cook {
                assert_eq!(spots, vec![Spot::Furnace, Spot::Hut], "the waiter serves");
                continue;
            }
            // the enchanter has no work surface at all: it walks to the buildings it
            // drains (`EntityAIWorkEnchanter.java:145`)
            if row.job == Job::Enchanter {
                assert_eq!(spots, vec![Spot::Hut], "no work block");
                continue;
            }
            assert!(
                spots.contains(&Spot::Bench) || spots.contains(&Spot::Still),
                "{}",
                row.job.path()
            );
        }
        for job in [Job::Baker, Job::Chef, Job::Dyer, Job::Glassblower] {
            let row = rules(job);
            assert!(
                row.places.iter().any(|place| place.spot == Spot::Furnace),
                "{} fires its output",
                job.path()
            );
            assert_eq!(row.produces.len(), 2, "{}", job.path());
            assert!(row.produces.iter().any(|product| product.work == Work::Smelt));
        }
        for job in [Job::Sawmill, Job::StoneMason, Job::Blacksmith] {
            assert_eq!(rules(job).produces.len(), 1, "{} has no furnace", job.path());
        }
        // the waiter and the alchemist work at their own surface, in their hut
        assert_eq!(rules(Job::Cook).places[0].spot, Spot::Furnace);
        assert_eq!(rules(Job::Alchemist).places[0].spot, Spot::Still);
        assert_eq!(rules(Job::Enchanter).places[0].spot, Spot::Hut);
    }

    #[test]
    fn every_crafter_dumps_after_one_action() {
        // AbstractEntityAICrafting.java:724-727, read at :284
        for row in RULES {
            assert_eq!(row.dump.count, 1, "{}", row.job.path());
            assert_eq!(row.dump.scale, DumpScale::Fixed, "{}", row.job.path());
            assert_eq!(row.dump.at(5), Some(1), "a hut level changes nothing here");
        }
    }

    #[test]
    fn the_bench_crafters_pay_half_an_item_and_the_smelters_pay_five_more() {
        for row in RULES {
            let per_item = row
                .xp
                .iter()
                .find(|grant| grant.name.contains("getCount()"));
            if row.job == Job::Cook || row.job == Job::Alchemist {
                assert!(per_item.is_none(), "{} never runs the progress loop", row.job.path());
                continue;
            }
            let per_item = per_item.unwrap_or_else(|| panic!("{} has no per-item grant", row.job.path()));
            assert_eq!(per_item.per_action, XP_PER_CRAFTED_ITEM);
            assert_eq!(per_item.source, CRAFT_XP_LINE);
        }
        for job in [Job::Baker, Job::Chef, Job::Dyer, Job::Glassblower] {
            let smelt = rules(job).xp.iter().find(|grant| grant.name == "BASE_XP_GAIN");
            assert_eq!(smelt.expect("a smelter grant").per_action, super::super::SMELTER_XP);
        }
        assert_eq!(rules(Job::Cook).xp.len(), 1);
        assert_eq!(rules(Job::Cook).xp[0].per_action, super::super::FURNACE_XP);
    }

    #[test]
    fn the_three_crafting_ticks_steps_are_the_formula_and_not_a_table_of_numbers() {
        // nothing in this file writes 300, 150 or 90: they come from the formula
        assert_eq!(super::super::crafting_ticks(0), 300);
        assert_eq!(super::super::crafting_ticks(4), 90);
        assert_eq!(super::super::crafting_ticks(20), 10);
        for row in RULES {
            for product in row.produces {
                assert!(
                    !matches!(product.work, Work::Ticks(_)),
                    "{} writes a fixed craft duration",
                    row.job.path()
                );
            }
        }
    }

    #[test]
    fn the_enchanter_is_the_one_with_tick_constants_and_they_are_its_own() {
        let row = rules(Job::Enchanter);
        assert_eq!(row.durations.len(), 3);
        let names: Vec<&str> = row.durations.iter().map(|constant| constant.name).collect();
        assert_eq!(names, vec!["MAX_PROGRESS_TICKS", "MAX_ENCHANTMENT_TICKS", "MAX_WAITING_TICKS"]);
        assert_eq!(row.durations[1].value, 300, ":78 is 60 * 5");
        for other in RULES {
            if other.job != Job::Enchanter && other.job != Job::Cook {
                assert!(other.durations.is_empty(), "{} adds a tick constant", other.job.path());
            }
        }
        assert_eq!(rules(Job::Cook).durations[0].name, "SERVE_DELAY");
        assert_eq!(rules(Job::Cook).durations[0].value, 30);
    }

    #[test]
    fn the_ten_tag_gated_huts_name_their_own_job_path() {
        for job in [
            Job::Baker,
            Job::Blacksmith,
            Job::Chef,
            Job::Dyer,
            Job::Fletcher,
            Job::Glassblower,
            Job::Mechanic,
            Job::Sawmill,
            Job::StoneMason,
        ] {
            let tag = recipe_tag(job).expect("a tag");
            assert!(tag.starts_with("minecolonies:"), "{tag}");
            let path = &tag["minecolonies:".len()..];
            assert_eq!(
                path,
                if job == Job::Chef { "cook" } else { job.path() },
                "TagConstants.java builds the tag from the job's own path"
            );
        }
        // the waiter, the alchemist and the enchanter gate on item classes,
        // custom recipes or nothing at all
        assert_eq!(recipe_tag(Job::Cook), None);
        assert_eq!(recipe_tag(Job::Alchemist), None);
        assert_eq!(recipe_tag(Job::Enchanter), None);
        assert_eq!(recipe_tag(Job::Farmer), None, "a farmer is not a crafter");
    }

    #[test]
    fn the_two_huts_that_filter_by_item_class_say_so() {
        // BuildingFletcher.java:83-86 and BuildingMechanic.java:92-94 name a class,
        // not a tag; the row keeps that as a material row of the same family
        let fletcher = rules(Job::Fletcher);
        assert!(fletcher.accepts.iter().any(|accepted| accepted.item == "#minecraft:arrows"));
        let mechanic = rules(Job::Mechanic);
        assert!(mechanic.accepts.iter().any(|accepted| accepted.item == "#minecraft:minecarts"));
        // and the blacksmith is the one that refuses leather by name
        assert!(rules(Job::Blacksmith).accepts.iter().any(|accepted| accepted.item == "minecraft:leather"));
    }

    #[test]
    fn no_crafter_states_a_tool_of_its_own_except_the_alchemist_with_its_shears() {
        for row in RULES {
            let tools: Vec<&str> = row
                .accepts
                .iter()
                .filter(|accepted| matches!(accepted.role, Role::Tool(_)))
                .map(|accepted| accepted.item)
                .collect();
            if row.job == Job::Alchemist {
                assert_eq!(tools, vec!["shears"], "EntityAIWorkAlchemist.java:236");
            } else {
                assert!(tools.is_empty(), "{} states a tool", row.job.path());
            }
        }
        // the asterisk of the family: a crafter asks for the *recipe's* tool, which
        // is `Job::tools()` empty for eleven of the twelve — the alchemist is the one
        // that also keeps shears in its hut (`BuildingAlchemist.java:68`)
        assert_eq!(Job::Alchemist.tools(), &[ToolKind::Shears]);
        for row in RULES {
            if row.job == Job::Alchemist {
                continue;
            }
            assert!(
                row.job.tools().is_empty(),
                "{} declares a fixed tool",
                row.job.path()
            );
        }
    }
}
