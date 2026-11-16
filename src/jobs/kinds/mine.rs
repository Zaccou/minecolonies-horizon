//! `mine` — what is dug out and what is fired: eight jobs, and the only family
//! where **breaking a block is the work**.
//!
//! Java reference: `core/colony/jobs/{JobMiner,JobQuarrier,JobSifter,
//! JobNetherWorker,JobSmelter,JobStoneSmeltery,JobCrusher,JobConcreteMixer}.java`
//! and the AI classes `EntityAIStructureMiner`, `EntityAIQuarrier`,
//! `EntityAIWorkSifter`, `EntityAIWorkNether`, `EntityAIWorkSmelter`,
//! `EntityAIWorkStoneSmeltery`, `EntityAIWorkCrusher`, `EntityAIConcreteMixer`,
//! plus the shared `AbstractEntityAIInteract` (mining),
//! `AbstractEntityAIStructure` (a finished structure),
//! `AbstractEntityAIUsesFurnace` and `AbstractEntityAIRequestSmelter` (firing).
//!
//! **The family's duration is [`super::block_mining_ticks`], not a number.**
//! `AbstractEntityAIInteract.getBlockMiningTime` (`:319-343`) is the same call for
//! the miner, the quarrier, the miner of the nether, the crusher and the concrete
//! mixer, and it reads the block's own hardness:
//!
//! ```text
//! ticks = 500 * 0.85^(breaking skill / 2) * hardness / tool speed * (1 - research)
//! ```
//!
//! So a stone block at skill 0 with a wooden pickaxe takes 500 ticks, at skill 20
//! it takes 98, and a pvp-mode colony pays a flat 250 whatever the block. That is
//! why no row below writes a mining time: a row writes [`Work::Block`] and the
//! number comes out of the level and the block.
//!
//! **Three jobs keep their own counter instead** ([`Work::Own`]), and each says so
//! in its comment with the arithmetic:
//!
//! - the **sifter**: `progress++` every `TICK_DELAY = 10` ticks until it passes
//!   `50 - skill / 2`, i.e. **510 ticks at level 0, 10 ticks at level 99**
//!   (`EntityAIWorkSifter.java:47`, `:186-188`);
//! - the **crusher**: `progress += 40` every `TICK_DELAY = 40` ticks against
//!   `50 - min(skill / 2 + 1, 50)`, so **two passes, 80 ticks** at any level
//!   (`EntityAIWorkCrusher.java:33`, `:109`, `:127`) — an inconsistency with the
//!   sifter the port keeps and reports;
//! - the **nether miner**: `TICK_DELAY = 40` per block mined and 40 × count for a
//!   non-block item (`EntityAIWorkNether.java:71`, `:543`, `:552`).
//!
//! **Two jobs are a furnace, and a furnace's time is vanilla's.** The smelter and
//! the stone smeltery both pay [`Work::Smelt`] and accelerate the block by
//! `(skill / 10) * 2` ticks every twenty (`AbstractEntityAIUsesFurnace.java:283-306`,
//! `AbstractEntityAIRequestSmelter.java:120-142`) — but **not with the same
//! skill**: the smelter uses its primary skill, the stone smeltery its secondary
//! (`AbstractEntityAIUsesFurnace.java:285` against
//! `AbstractEntityAIRequestSmelter.java:122`). The port keeps both and the report
//! names the disagreement.
//!
//! **What is not here.** The mine shaft's geometry is content, not code: the
//! corridor sizes are the blueprints (`minerx2top`, `minerx4`, …) and Java only
//! carries the shaft radius and the node spacing (`SHAFT_RADIUS = 3`,
//! `NODE_DISTANCE = 7`), which the miner's comment quotes instead of inventing a
//! block count. The quarry's dig zone is a `QuarryModule(32)` or `(64)` height
//! (`BuildingModules.java:510-511`), likewise a number the building owns.

use super::{
    constant, dump, material, place, product, tool_for_block, xp, DumpScale, Family, JobRules,
    Output, Role, Spot, Work, FURNACE_START_DELAY, SATURATION_DEFAULT, SATURATION_HEAVY,
    TICKS_SECOND, XP_EACH_BUILDING, XP_PER_BLOCK,
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
            family: Family::Mine,
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

/// The line where a broken block pays its `0.05` (`AbstractEntityAIInteract.java:234`).
const XP_PER_BLOCK_LINE: &str = "AbstractEntityAIInteract.java:234";

/// The line where a finished structure pays its `8.0`
/// (`AbstractEntityAIStructure.java:279`).
const XP_PER_STRUCTURE_LINE: &str = "AbstractEntityAIStructure.java:279";

/// The line of the dump threshold every crafter-shaped job inherits
/// (`AbstractEntityAICrafting.java:724-727`).
const CRAFT_DUMP_LINE: &str = "AbstractEntityAICrafting.java:724";

/// The line where a furnace worker stops for lack of a furnace
/// (`AbstractEntityAIUsesFurnace.java:201-208`).
const NO_FURNACE_LINE: &str = "AbstractEntityAIUsesFurnace.java:201";

/// The line where a furnace user gives up because it may burn nothing
/// (`AbstractEntityAIUsesFurnace.java:193-199`).
const NO_FUEL_LINE: &str = "AbstractEntityAIUsesFurnace.java:193";

/// The eight mine jobs, in `Job::ALL` order of the family.
pub const RULES: &[JobRules] = &[
    // The concrete mixer. It fills a tracked water block with powder and then
    // breaks the concrete that set beside it: **20 ticks to place, 20 ticks to
    // harvest** (`EntityAIConcreteMixer.java:48-49`), one block at a time, at the
    // first water position the building knows (`BuildingConcreteMixer.java:193-208`).
    // Its dump threshold is the building's count of tracked positions, not a
    // constant (`EntityAIConcreteMixer.java:183-186`). It is the only job of the
    // family with **no XP of its own**: the harvest pays the generic block grant
    // and the placing pays nothing. Default recipes: 4 sand + 4 gravel + 1 dye →
    // 8 powder, powder → concrete (`DefaultConcreteMixerCraftingProvider.java:41-69`).
    rules!(
        ConcreteMixer,
        &[place(Spot::Water, None, "BuildingConcreteMixer.java:66")],
        [
            material(
                "#minecraft:concrete_powder",
                Role::Material,
                "EntityAIConcreteMixer.java:34"
            ),
            tool_for_block(ToolKind::Pickaxe, "BuildingConcreteMixer.java:62"),
        ],
        [product(
            Output::Recipe,
            Work::Ticks(TICKS_SECOND),
            "EntityAIConcreteMixer.java:104"
        )],
        [
            constant("CONCRETE_MIXER_PLACING", 20, "EntityAIConcreteMixer.java:48"),
            constant("CONCRETE_MIXER_HARVESTING", 20, "EntityAIConcreteMixer.java:49"),
            constant("WATER_DEPTH_SUPPORT", 5, "BuildingConcreteMixer.java:46"),
        ],
        [xp(XP_PER_BLOCK, "XP_PER_BLOCK", XP_PER_BLOCK_LINE)],
        dump(0, DumpScale::HutOwned, "EntityAIConcreteMixer.java:183"),
        SATURATION_DEFAULT,
        "EntityAIConcreteMixer.java:1"
    ),
    // The crusher. Its own counter, and the arithmetic is a curiosity worth
    // keeping: `progress += TICK_DELAY (40)` each pass against a threshold of
    // `MAX_LEVEL (50) - min(skill / 2 + 1, 50)` (`EntityAIWorkCrusher.java:109`,
    // `:127`). So it takes **two passes — 80 ticks — below skill 20 and one pass —
    // 40 ticks — from skill 20 up**, a much steeper curve than the sifter's for the
    // same nominal counter; the port keeps both and the report names the
    // inconsistency. Its daily quota is a **setting whose default is 0**
    // (`BuildingModules.java:287`), so a freshly built crusher idles until a
    // player raises it — and `getMaxDailyQuantity` (`BuildingCrusher.java:88-95`)
    // is dead code the AI never calls.
    rules!(
        Crusher,
        &[place(Spot::Bench, None, "EntityAIWorkCrusher.java:51")],
        [
            material("minecraft:bone", Role::Material, "DefaultCrusherCraftingProvider.java:42"),
            material(
                "minecraft:bone_block",
                Role::Material,
                "DefaultCrusherCraftingProvider.java:44"
            ),
            material(
                "minecraft:cobblestone",
                Role::Material,
                "DefaultCrusherCraftingProvider.java:46"
            ),
            material("minecraft:gravel", Role::Material, "DefaultCrusherCraftingProvider.java:48"),
            material("minecraft:sand", Role::Material, "DefaultCrusherCraftingProvider.java:50"),
            material("minecraft:clay", Role::Material, "DefaultCrusherCraftingProvider.java:52"),
            material("minecraft:tuff", Role::Material, "DefaultCrusherCraftingProvider.java:54"),
            material(
                "minecraft:cobbled_deepslate",
                Role::Material,
                "DefaultCrusherCraftingProvider.java:54"
            ),
        ],
        [product(
            Output::Recipe,
            Work::Own,
            "EntityAIWorkCrusher.java:147"
        )],
        [constant("TICK_DELAY", 40, "EntityAIWorkCrusher.java:33")],
        [xp(0.1, "addExperience(0.1)", "EntityAIWorkCrusher.java:151")],
        dump(1, DumpScale::Fixed, CRAFT_DUMP_LINE),
        SATURATION_DEFAULT,
        "EntityAIWorkCrusher.java:1"
    ),
    // The miner. It breaks whatever the shaft asks for and **silk-touches every
    // ore** (`EntityAIStructureMiner.java:293-296`), so its product is the block's
    // own drops and not a table of its own. Geometry lives in the blueprints; Java
    // adds `SHAFT_RADIUS = 3` (a 7×7 shaft, `:105`), `NODE_DISTANCE = 7` between
    // two node centres (`:100`), a `SAFE_CHECK_RANGE = 5` platform (`:106`) and the
    // four `MINING_LEVELS = {48, 16, -16, -100}` one per hut level
    // (`BuildingMiner.java:63`). It needs both a ladder and a cobblestone position
    // or it reports `INVALID_MINESHAFT` (`:183-187`), fills with the hut's
    // `FILL_BLOCK` (`cobblestone` by default, `BuildingModules.java:503`) and
    // dumps after `64 × hut level` blocks (`:222-225`).
    rules!(
        Miner,
        &[place(Spot::Mine, None, "EntityAIStructureMiner.java:183")],
        [
            material("minecraft:cobblestone", Role::Material, "BuildingModules.java:503"),
            tool_for_block(ToolKind::Pickaxe, "AbstractEntityAIBasic.java:1386"),
        ],
        [product(Output::Drops, Work::Block, XP_PER_BLOCK_LINE)],
        [],
        [
            xp(XP_PER_BLOCK, "XP_PER_BLOCK", XP_PER_BLOCK_LINE),
            xp(XP_EACH_BUILDING, "XP_EACH_BUILDING", XP_PER_STRUCTURE_LINE),
        ],
        dump(64, DumpScale::PerBuildingLevel, "EntityAIStructureMiner.java:222"),
        SATURATION_HEAVY,
        "EntityAIStructureMiner.java:1"
    ),
    // The nether miner. It builds and lights a portal, disappears into the nether
    // and comes back with what it found: **one trip every three days**
    // (`BuildingNetherWorker.java:61`, `:66`), a simulated travel of
    // `400 ticks per item carried` (`EntityAIWorkNether.java:176`), and an
    // adventure loop where each item is either a mob fought or a block mined
    // (`:384-548`). Its own counter runs on `TICK_DELAY = 40` (`:71`). It states
    // **five** tools at the floor level (`:248-252`) and asks its building for
    // armour from five level ranges (`:110-114`), and it eats from its own menu,
    // sixteen food at a time (`:938`, `:946`).
    rules!(
        NetherWorker,
        &[place(Spot::Nether, None, "BuildingNetherWorker.java:243")],
        [
            super::tool_at_floor(ToolKind::Axe, "EntityAIWorkNether.java:248"),
            super::tool_at_floor(ToolKind::Pickaxe, "EntityAIWorkNether.java:249"),
            super::tool_at_floor(ToolKind::Shovel, "EntityAIWorkNether.java:250"),
            super::tool_at_floor(ToolKind::Sword, "EntityAIWorkNether.java:251"),
            super::tool_at_floor(ToolKind::FlintAndSteel, "EntityAIWorkNether.java:252"),
            material("#minecolonies:netherminer_menu", Role::Food, "EntityAIWorkNether.java:946"),
        ],
        [product(Output::Drops, Work::Own, "EntityAIWorkNether.java:513")],
        [constant("TICK_DELAY", 40, "EntityAIWorkNether.java:71")],
        [xp(0.2, "addExperience(0.2)", "EntityAIWorkNether.java:578")],
        dump(super::ACTIONS_UNTIL_DUMP, DumpScale::Fixed, "AbstractEntityAIBasic.java:425"),
        SATURATION_DEFAULT,
        "EntityAIWorkNether.java:1"
    ),
    // The quarrier. The same block formula as the miner, on a shaft the size of the
    // hut's `QuarryModule` — **32 or 64 blocks down** (`BuildingModules.java:510-511`)
    // and three stages `BUILD_SOLID`, `DECORATE`, `CLEAR` (`EntityAIQuarrier.java:230-231`),
    // ending at the world's floor (`:304-308`). It works within **10 blocks** of the
    // dig site (`5 + 5`, `:661`, `:668-678`) and dumps after `128 × hut level`
    // blocks — twice the miner's (`:518`).
    rules!(
        Quarrier,
        &[place(Spot::Quarry, Some(10), "EntityAIQuarrier.java:668")],
        [
            material("minecraft:cobblestone", Role::Material, "BuildingModules.java:503"),
            tool_for_block(ToolKind::Pickaxe, "AbstractEntityAIBasic.java:1386"),
        ],
        [product(Output::Drops, Work::Block, XP_PER_BLOCK_LINE)],
        [],
        [
            xp(XP_PER_BLOCK, "XP_PER_BLOCK", XP_PER_BLOCK_LINE),
            xp(XP_EACH_BUILDING, "XP_EACH_BUILDING", XP_PER_STRUCTURE_LINE),
        ],
        dump(128, DumpScale::PerBuildingLevel, "EntityAIQuarrier.java:518"),
        SATURATION_HEAVY,
        "EntityAIQuarrier.java:1"
    ),
    // The sifter. Four inputs and one tool: **dirt, gravel, sand and soul sand**
    // (`DefaultSifterCraftingProvider.java:57`, `:123`, `:167`, `:199`) through a
    // mesh of the `minecolonies:meshes` tag (`ModTags.java:62`), four tiers of mesh
    // worth 500, 1 000, 1 500 and 2 000 uses
    // (`ModItemsInitializer.java:131-134`) and gated on hut level 1, 3, 4
    // (`:59`, `:75`, `:91`, `:107`). Its counter is its own: **510 ticks at skill 0,
    // 10 at skill 99** (`:186-188`), and it dumps after every sift (`:85-88`). Its
    // daily quota is `hut level² × 64`, unlimited at level 5
    // (`BuildingSifter.java:80-88`).
    rules!(
        Sifter,
        &[place(Spot::Bench, None, "EntityAIWorkSifter.java:73")],
        [
            material("minecraft:dirt", Role::Material, "DefaultSifterCraftingProvider.java:57"),
            material("minecraft:sand", Role::Material, "DefaultSifterCraftingProvider.java:167"),
            material("minecraft:gravel", Role::Material, "DefaultSifterCraftingProvider.java:123"),
            material("minecraft:soul_sand", Role::Material, "DefaultSifterCraftingProvider.java:199"),
            // The mesh is the recipe's *tool* (`EntityAIWorkSifter.java:160`) and it
            // wears out — 500 to 2 000 uses — but it is not one of the fourteen
            // equipment kinds, so its row is a material and the comment says what it
            // does.
            material("#minecolonies:meshes", Role::Material, "ModTags.java:62"),
        ],
        [product(Output::Recipe, Work::Own, "EntityAIWorkSifter.java:198")],
        [
            constant("TICK_DELAY", 10, "EntityAIWorkSifter.java:47"),
            constant("NO_MESH_DELAY", 100, "EntityAIWorkSifter.java:57"),
        ],
        [xp(0.2, "addExperience(0.2)", "EntityAIWorkSifter.java:211")],
        dump(1, DumpScale::Fixed, "EntityAIWorkSifter.java:85"),
        SATURATION_DEFAULT,
        "EntityAIWorkSifter.java:1"
    ),
    // The smelter. Fires ore and nothing else: the accept test is
    // `isSmeltable && isOre && !isBreakableOre && !excluded` (`EntityAIWorkSmelter.java:186-197`),
    // where the exclusion list is the `ores` item list (`:59`). It loads up to a
    // stack of 64 per furnace per trip (`AbstractEntityAIUsesFurnace.java:483-486`)
    // and empties a furnace once it holds more than **10**
    // (`AbstractEntityAIUsesFurnace.java:60`, `:140-143`). Breaking an ore pays
    // **0.2** and extracting a result pays its own `BASE_XP_GAIN = 5`
    // (`EntityAIWorkSmelter.java:109`, `:144`). Its ore-breaking tool is a
    // **virtual** diamond pickaxe with Fortune at `hut level - 1`
    // (`BuildingSmeltery.java:241-249`), not a tool the citizen holds.
    rules!(
        Smelter,
        &[place(Spot::Furnace, None, NO_FURNACE_LINE)],
        [
            material("#minecolonies:itemlist_fuel", Role::Fuel, NO_FUEL_LINE),
            material("minecolonies:ores", Role::Material, "EntityAIWorkSmelter.java:196"),
        ],
        [product(
            Output::Recipe,
            Work::Smelt,
            "AbstractEntityAIUsesFurnace.java:483"
        )],
        [constant(
            "FURNACE_START_DELAY",
            FURNACE_START_DELAY,
            "AbstractEntityAIUsesFurnace.java:78"
        )],
        [
            xp(5.0, "BASE_XP_GAIN", "EntityAIWorkSmelter.java:144"),
            xp(0.2, "addExperience(0.2)", "EntityAIWorkSmelter.java:109"),
        ],
        dump(1, DumpScale::Fixed, "AbstractEntityAIUsesFurnace.java:379"),
        SATURATION_DEFAULT,
        "EntityAIWorkSmelter.java:1"
    ),
    // The stone smeltery. It only ever fires through a `Blocks.FURNACE`
    // intermediate, and delegates anything else to the ordinary crafting loop
    // (`AbstractEntityAIRequestSmelter.java:434-437`). What it may fire is a tag
    // pair: the stonemason's products in, a fixed list of fired goods out
    // (`DefaultItemTagsProvider.java:515-535`). It spreads up to 64 per furnace
    // across `min(skill / 10 + 1, fuelled furnaces)` furnaces (`:389-419`,
    // `:673-688`) — and it accelerates with its **secondary** skill, where the
    // smelter uses its primary.
    rules!(
        StoneSmeltery,
        &[place(Spot::Furnace, None, NO_FURNACE_LINE)],
        [
            material("#minecolonies:itemlist_fuel", Role::Fuel, NO_FUEL_LINE),
            material(
                "#minecolonies:stonesmeltery",
                Role::Material,
                "DefaultItemTagsProvider.java:515"
            ),
        ],
        [product(
            Output::Recipe,
            Work::Smelt,
            "AbstractEntityAIRequestSmelter.java:622"
        )],
        [],
        [xp(
            5.0,
            "BASE_XP_GAIN",
            "AbstractEntityAIRequestSmelter.java:622"
        )],
        dump(1, DumpScale::Fixed, CRAFT_DUMP_LINE),
        SATURATION_DEFAULT,
        "EntityAIWorkStoneSmeltery.java:1"
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(job: Job) -> &'static JobRules {
        super::super::rules_for(job).expect("a mine job has rules")
    }

    #[test]
    fn eight_mine_jobs_and_no_duplicate() {
        assert_eq!(RULES.len(), 8);
        let mut seen = Vec::new();
        for row in RULES {
            assert!(!seen.contains(&row.job), "{} twice", row.job.path());
            seen.push(row.job);
            assert_eq!(row.family, Family::Mine);
        }
        for job in [
            Job::Miner,
            Job::Quarrier,
            Job::Sifter,
            Job::NetherWorker,
            Job::Smelter,
            Job::StoneSmeltery,
            Job::Crusher,
            Job::ConcreteMixer,
        ] {
            assert!(seen.contains(&job), "{} is missing", job.path());
        }
    }

    #[test]
    fn the_miner_and_the_quarrier_pay_the_block_formula_and_the_quarrier_twice_as_long() {
        for job in [Job::Miner, Job::Quarrier] {
            let row = rules(job);
            assert!(row.produces[0].work.is_block(), "{}", job.path());
            assert_eq!(row.xp.len(), 2, "a block and a structure");
            assert_eq!(row.xp[1].per_action, XP_EACH_BUILDING);
            assert_eq!(row.saturation, SATURATION_HEAVY);
        }
        assert_eq!(rules(Job::Miner).dump.at(3), Some(192));
        assert_eq!(rules(Job::Quarrier).dump.at(3), Some(384));
        assert_eq!(
            rules(Job::Quarrier).dump.at(1),
            rules(Job::Miner).dump.at(2),
            "128 is twice 64"
        );
    }

    #[test]
    fn the_three_own_counters_each_declare_their_period() {
        for job in [Job::Sifter, Job::Crusher, Job::NetherWorker] {
            let row = rules(job);
            assert_eq!(row.produces[0].work, Work::Own, "{}", job.path());
            assert!(
                row.durations.iter().any(|constant| constant.name == "TICK_DELAY"),
                "{} runs its own counter with no period",
                job.path()
            );
        }
        assert_eq!(rules(Job::Sifter).durations[0].value, 10);
        assert_eq!(rules(Job::Crusher).durations[0].value, 40);
        assert_eq!(rules(Job::NetherWorker).durations[0].value, 40);
        // nobody else invents a counter
        for row in RULES {
            if ![Job::Sifter, Job::Crusher, Job::NetherWorker].contains(&row.job) {
                assert_ne!(row.produces[0].work, Work::Own, "{}", row.job.path());
            }
        }
    }

    #[test]
    fn the_sifter_and_the_crusher_keep_two_different_counters_and_the_port_says_so() {
        // sifter: progress + 1 per 10-tick state, threshold 50 - skill/2
        let sifter_threshold = |skill: i32| 50 - skill / 2;
        assert_eq!(sifter_threshold(0), 50);
        assert_eq!(sifter_threshold(98), 1);
        assert_eq!(sifter_threshold(0) * rules(Job::Sifter).durations[0].value, 500);
        // crusher: progress + 40 per 40-tick state, threshold 50 - min(skill/2+1, 50)
        let crusher_passes = |skill: i32| {
            let threshold = 50 - (skill / 2 + 1).min(50);
            let mut progress = 0;
            let mut passes = 0;
            while progress <= threshold {
                progress += 40;
                passes += 1;
                assert!(passes < 10, "the loop must terminate");
            }
            passes
        };
        assert_eq!(crusher_passes(0), 2, "40 then 80, against 49");
        assert_eq!(crusher_passes(18), 2, "threshold 41, still two passes");
        assert_eq!(crusher_passes(20), 1, "threshold 39: one pass of 40 clears it");
        assert_eq!(crusher_passes(99), 1);
        assert_eq!(crusher_passes(0) * rules(Job::Crusher).durations[0].value, 80);
        assert_eq!(crusher_passes(99) * rules(Job::Crusher).durations[0].value, 40);
    }

    #[test]
    fn the_two_furnace_jobs_fire_and_neither_states_a_pickaxe() {
        for job in [Job::Smelter, Job::StoneSmeltery] {
            let row = rules(job);
            assert_eq!(row.places[0].spot, Spot::Furnace, "{}", job.path());
            assert_eq!(row.produces[0].work, Work::Smelt, "{}", job.path());
            assert_eq!(row.xp[0].per_action, 5.0, "{}", job.path());
            assert!(
                row.accepts.iter().any(|accepted| accepted.item == "#minecolonies:itemlist_fuel"),
                "{} does not say what it burns",
                job.path()
            );
            assert!(
                !row.accepts
                    .iter()
                    .any(|accepted| accepted.role == Role::Tool(ToolKind::Pickaxe)),
                "{} smelts, it does not mine",
                job.path()
            );
        }
        // the smelter breaks ore and pays for it; the stone smeltery does not
        assert_eq!(rules(Job::Smelter).xp.len(), 2);
        assert_eq!(rules(Job::StoneSmeltery).xp.len(), 1);
    }

    #[test]
    fn only_the_nether_miner_states_five_tools() {
        let nether = rules(Job::NetherWorker);
        let tools: Vec<&str> = nether
            .accepts
            .iter()
            .filter(|accepted| matches!(accepted.role, Role::Tool(_)))
            .map(|accepted| accepted.item)
            .collect();
        assert_eq!(tools, vec!["axe", "pickaxe", "shovel", "sword", "flintandsteel"]);
        for row in RULES {
            if row.job != Job::NetherWorker {
                let count = row
                    .accepts
                    .iter()
                    .filter(|accepted| matches!(accepted.role, Role::Tool(_)))
                    .count();
                assert!(count <= 1, "{} states {count} tools", row.job.path());
            }
        }
        // and its trip is rationed by the building, not by a delay
        assert_eq!(nether.places[0].spot, Spot::Nether);
    }

    #[test]
    fn the_concrete_mixer_is_the_one_job_whose_dump_threshold_the_building_owns() {
        let mixer = rules(Job::ConcreteMixer);
        assert_eq!(mixer.dump.scale, DumpScale::HutOwned);
        assert_eq!(mixer.dump.at(5), None, "the building answers, not a constant");
        for row in RULES {
            if row.job != Job::ConcreteMixer {
                assert!(row.dump.at(1).is_some(), "{}", row.job.path());
            }
        }
        // its two 20-tick states are named once, and the product pays one of them
        let placing = mixer
            .durations
            .iter()
            .find(|constant| constant.name == "CONCRETE_MIXER_PLACING")
            .expect("placing");
        let harvesting = mixer
            .durations
            .iter()
            .find(|constant| constant.name == "CONCRETE_MIXER_HARVESTING")
            .expect("harvesting");
        assert_eq!(placing.value, TICKS_SECOND);
        assert_eq!(harvesting.value, TICKS_SECOND);
        assert_eq!(mixer.produces[0].work, Work::Ticks(TICKS_SECOND));
    }

    #[test]
    fn the_sifter_accepts_four_inputs_and_a_mesh_and_no_fifth() {
        let sifter = rules(Job::Sifter);
        let inputs: Vec<&str> = sifter
            .accepts
            .iter()
            .filter(|accepted| accepted.role == Role::Material && !accepted.item.starts_with('#'))
            .map(|accepted| accepted.item)
            .collect();
        assert_eq!(
            inputs,
            vec![
                "minecraft:dirt",
                "minecraft:sand",
                "minecraft:gravel",
                "minecraft:soul_sand"
            ]
        );
        assert!(
            sifter
                .accepts
                .iter()
                .any(|accepted| accepted.item == "#minecolonies:meshes"),
            "the mesh is the recipe's tool (`EntityAIWorkSifter.java:160`)"
        );
        // it never states a tool of the equipment registry: the mesh is its tool
        assert!(sifter.job.tools().is_empty());
        assert_eq!(sifter.job.display(), "Sifter");
    }

    #[test]
    fn the_crusher_names_all_eight_of_its_default_inputs() {
        let crusher = rules(Job::Crusher);
        assert_eq!(crusher.accepts.len(), 8);
        for item in [
            "minecraft:bone",
            "minecraft:bone_block",
            "minecraft:cobblestone",
            "minecraft:gravel",
            "minecraft:sand",
            "minecraft:clay",
            "minecraft:tuff",
            "minecraft:cobbled_deepslate",
        ] {
            assert!(
                crusher.accepts.iter().any(|accepted| accepted.item == item),
                "{item} is missing"
            );
        }
        assert_eq!(crusher.xp[0].per_action, 0.1);
    }
}
