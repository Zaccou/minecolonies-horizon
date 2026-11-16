//! `bois` — what grows out of the ground: four jobs, and the four surfaces the
//! port treats as "growing" rather than "farming".
//!
//! Java reference: `core/colony/jobs/{JobLumberjack,JobPlanter,JobFlorist,
//! JobBeekeeper}.java`, the AI classes `EntityAIWorkLumberjack`,
//! `production/agriculture/{EntityAIWorkPlanter,EntityAIWorkFlorist,
//! EntityAIWorkBeekeeper}.java`, and the huts that hold the settings —
//! `BuildingLumberjack`, `BuildingPlantation`, `BuildingFlorist`,
//! `BuildingBeekeeper`.
//!
//! **The forester is the one job of the port that hides a shared constant.** It
//! redeclares `RANGE_HORIZONTAL_PICKUP = 5.0F` and `RANGE_VERTICAL_PICKUP = 2.0F`
//! (`EntityAIWorkLumberjack.java:91`, `:96`), shadowing the `45.0` and `3.0` its
//! parent uses for every other worker (`AbstractEntityAIInteract.java:74`, `:79`).
//! A port that reached for the parent's value would let the forester sweep fifteen
//! blocks sideways for a leaf log; the report keeps both numbers and this file
//! does not use the parent's for it.
//!
//! **Two of the four have a mining time of their own.** The forester and the
//! planter use [`super::block_mining_ticks`] like everyone else (one log, one
//! sugar cane), but the **florist** overrides the whole formula with
//! `BASE_BLOCK_MINING_DELAY = 10` (`EntityAIWorkFlorist.java:75`, `:336-340`), so a
//! flower costs **60 ticks at level 0 and 10 at level 99** — six times faster than
//! a block. And the **beekeeper** never breaks anything: it takes honeycomb or a
//! bottle straight out of the hive.
//!
//! **The beekeeper is not a herder.** It does not extend `AbstractEntityAIHerder`
//! and shares none of that class's constants; its numbers are its own, which is
//! why its row is here and the six animal jobs are in `ferme`.

use super::{
    constant, dump, material, place, product, tool_at_floor, xp, DumpScale, Family, JobRules,
    Output, Role, Spot, Work, SATURATION_DEFAULT, SATURATION_HEAVY, XP_PER_BLOCK,
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
            family: Family::Wood,
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

/// `EntityAIWorkLumberjack.XP_PER_TREE`: what one finished tree is worth
/// (`EntityAIWorkLumberjack.java:144`, granted at `:538`). Public because it is
/// the one grant of the family a test can feed to the experience curve.
pub const XP_PER_TREE: f64 = 1.0;

/// `EntityAIWorkFlorist.XP_PER_FLOWER`: what one harvested flower is worth
/// (`EntityAIWorkFlorist.java:96`, granted at `:277`).
pub const XP_PER_FLOWER: f64 = 2.0;

/// `EntityAIWorkPlanter.XP_PER_HARVEST`: what one harvested plant is worth
/// (`EntityAIWorkPlanter.java:56`, granted at `:378`).
pub const XP_PER_HARVEST: f64 = 1.0;

/// `EntityAIWorkBeekeeper.EXP_PER_HARVEST`: what one hive is worth, whether the
/// worker takes honeycomb or a bottle (`EntityAIWorkBeekeeper.java:74`, granted at
/// `:381` and `:395`).
pub const EXP_PER_HARVEST: f64 = 5.0;

/// `EntityAIWorkBeekeeper.BEES_PER_LEVEL`: how many bees one hut level supports
/// (`EntityAIWorkBeekeeper.java:59`, read at `:134`).
pub const BEES_PER_LEVEL: i32 = 3;

/// The line where any worker's broken block pays its `0.05`
/// (`AbstractEntityAIInteract.java:234`).
const XP_PER_BLOCK_LINE: &str = "AbstractEntityAIInteract.java:234";

/// The four wood jobs, in `Job::ALL` order of the family.
pub const RULES: &[JobRules] = &[
    // The beekeeper. It works **hives it was given with a sceptre**, up to
    // `2^(hut level - 1)` of them — 1, 2, 4, 8, 16 (`BuildingBeekeeper.java:189-192`)
    // — and takes either **three honeycomb** or **one bottle of honey** from a hive
    // whose honey level is at least 5 (`EntityAIWorkBeekeeper.java:321`, `:375`,
    // `:384`, `:603-611`). It looks for bees within **22 blocks** of each hive
    // (`:84`, `:597`), breeds two at a time when it is not holding honey
    // (`:58`, `:453`), and keeps **16 flowers** wanted (`:69`) from the hut's own
    // flower list (`:456`, `:565`). Each harvest pays 5 XP, and it dumps after five
    // (`:119-122`). It states shears only when its mode is not honey
    // (`:166`): the shears row below is the shearing branch.
    rules!(
        Beekeeper,
        &[place(Spot::Hive, Some(22), "EntityAIWorkBeekeeper.java:597")],
        [
            material("#minecraft:flowers", Role::Material, "EntityAIWorkBeekeeper.java:561"),
            material(
                "minecraft:glass_bottle",
                Role::Material,
                "EntityAIWorkBeekeeper.java:174"
            ),
            tool_at_floor(ToolKind::Shears, "EntityAIWorkBeekeeper.java:166"),
        ],
        [
            product(
                Output::Fixed { item: "minecraft:honeycomb", count: 3 },
                Work::Own,
                "EntityAIWorkBeekeeper.java:608"
            ),
            product(
                Output::Fixed { item: "minecraft:honey_bottle", count: 1 },
                Work::Own,
                "EntityAIWorkBeekeeper.java:603"
            ),
        ],
        [
            constant("DECIDING_DELAY", 40, "EntityAIWorkBeekeeper.java:89"),
            constant("BREEDING_DELAY", 40, "EntityAIWorkBeekeeper.java:93"),
            constant("NO_HIVES_DELAY", 100, "EntityAIWorkBeekeeper.java:91"),
            constant("NO_ANIMALS_DELAY", 100, "EntityAIWorkBeekeeper.java:90"),
            constant("NO_FLOWERS_DELAY", 100, "EntityAIWorkBeekeeper.java:92"),
        ],
        [xp(EXP_PER_HARVEST, "EXP_PER_HARVEST", "EntityAIWorkBeekeeper.java:381")],
        dump(5, DumpScale::Fixed, "EntityAIWorkBeekeeper.java:119"),
        SATURATION_DEFAULT,
        "EntityAIWorkBeekeeper.java:1"
    ),
    // The florist. It farms **composted dirt**, not a field: the hut registers the
    // `blockCompostedDirt` blocks of its own blueprint (`BuildingFlorist.java:99-104`)
    // and a position with no compost is composted, a position with a flower above
    // it is harvested (`EntityAIWorkFlorist.java:187-206`). It must stay within
    // **50 blocks** of the hut (`:50`, `:167`), requests **16** compost at a time
    // (`:70`, `:183`) and dumps after `10 × hut level` flowers (`:55`, `:324-327`).
    // Its mining time is its own formula, `10 × (1 + max(0, 5 - 0.1 × skill / 2))`
    // (`:75`, `:336-340`), which is why the product is [`Work::Own`] and the base
    // delay is a named constant instead of the shared 500. The `BASE_XP_GAIN = 0.5`
    // of `:65` is **dead code** and is not carried.
    rules!(
        Florist,
        &[place(Spot::Hut, Some(50), "EntityAIWorkFlorist.java:167")],
        [
            material("minecolonies:compost", Role::Compost, "EntityAIWorkFlorist.java:172"),
            material("#minecraft:flowers", Role::Material, "EntityAIWorkFlorist.java:142"),
            tool_at_floor(ToolKind::Shears, "EntityAIWorkFlorist.java:309"),
        ],
        [product(Output::Drops, Work::Own, "EntityAIWorkFlorist.java:272")],
        [
            constant(
                "BASE_BLOCK_MINING_DELAY",
                10,
                "EntityAIWorkFlorist.java:75"
            ),
            constant("DECIDE_DELAY", 200, "EntityAIWorkFlorist.java:131"),
        ],
        [
            xp(XP_PER_FLOWER, "XP_PER_FLOWER", "EntityAIWorkFlorist.java:277"),
            xp(XP_PER_BLOCK, "XP_PER_BLOCK", XP_PER_BLOCK_LINE),
        ],
        dump(10, DumpScale::PerBuildingLevel, "EntityAIWorkFlorist.java:324"),
        SATURATION_DEFAULT,
        "EntityAIWorkFlorist.java:1"
    ),
    // The forester. It searches for trees in **50 blocks, then 150 in steps of 5**
    // (`EntityAIWorkLumberjack.java:72`, `:77`, `:81`) once every **400 ticks**
    // (`:118`), must be within **2 blocks** of the trunk to chop it (`:101`, `:657`)
    // and plants one sapling per stump (`:916-950`). It pays **1 XP per finished
    // tree** plus the generic block grant, keeps a sapling list from its hut
    // (`:243`) and only accepts a plantable from `#minecraft:saplings` (or a fungus,
    // `CompatibilityManager.java:715-720`). Staying inside its own restriction zone
    // is a hut setting (`BuildingLumberjack.java:57`). Its pick-up box is the
    // **5 × 2** one, not the parent's 45 × 3 (`:91`, `:96`).
    rules!(
        Lumberjack,
        &[place(Spot::Tree, Some(2), "EntityAIWorkLumberjack.java:657")],
        [
            material("#minecraft:saplings", Role::Seed, "CompatibilityManager.java:715"),
            tool_at_floor(ToolKind::Axe, "EntityAIWorkLumberjack.java:295"),
            tool_at_floor(ToolKind::Hoe, "EntityAIWorkLumberjack.java:295"),
        ],
        [product(Output::Drops, Work::Block, XP_PER_BLOCK_LINE)],
        [
            constant("WAIT_BEFORE_SEARCH", 400, "EntityAIWorkLumberjack.java:118"),
            constant("WAIT_BEFORE_SAPLING", 50, "EntityAIWorkLumberjack.java:105"),
            constant("MAX_WAITING_TIME", 50, "EntityAIWorkLumberjack.java:109"),
            constant("WAIT_BEFORE_INCREMENT", 20, "EntityAIWorkLumberjack.java:123"),
            constant("TIMEOUT_DELAY", 10, "EntityAIWorkLumberjack.java:114"),
        ],
        [
            xp(XP_PER_TREE, "XP_PER_TREE", "EntityAIWorkLumberjack.java:538"),
            xp(XP_PER_BLOCK, "XP_PER_BLOCK", XP_PER_BLOCK_LINE),
        ],
        dump(32, DumpScale::Fixed, "EntityAIWorkLumberjack.java:999"),
        SATURATION_HEAVY,
        "EntityAIWorkLumberjack.java:1"
    ),
    // The planter. A plantation field, up to `ceil(hut level / 2)` of them
    // (`BuildingPlantation.java:244-250`), each module carrying **at most 20 plant
    // positions** (`AbstractPlantationModule.java:29`, `:115-118`) and its own
    // action limit: 5 for a percentage or a side harvest, 10 for an upwards or
    // downwards one, **1** for bonemeal (`PercentageHarvestPlantModule.java:177-180`,
    // `UpwardsGrowingPlantModule.java:211-214`, `DownwardsGrowingPlantModule.java:211-214`,
    // `TreeSidePlantModule.java:144-147`, `BoneMealedPlantModule.java:169-172`).
    // That limit is the dump threshold, which is why it is [`DumpScale::HutOwned`]
    // and not a number. It requests a quarter stack of whatever it plants
    // (`AbstractPlantationModule.java:82-84`).
    rules!(
        Planter,
        &[place(Spot::Field, None, "EntityAIWorkPlanter.java:161")],
        [
            material("#minecolonies:plantation", Role::Seed, "TagConstants.java:69"),
            material("minecolonies:compost", Role::Compost, "EntityAIWorkPlanter.java:340"),
        ],
        [product(Output::Drops, Work::Block, "EntityAIWorkPlanter.java:371")],
        [],
        [
            xp(XP_PER_HARVEST, "XP_PER_HARVEST", "EntityAIWorkPlanter.java:378"),
            xp(XP_PER_BLOCK, "XP_PER_BLOCK", XP_PER_BLOCK_LINE),
        ],
        dump(0, DumpScale::HutOwned, "EntityAIWorkPlanter.java:445"),
        SATURATION_DEFAULT,
        "EntityAIWorkPlanter.java:1"
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(job: Job) -> &'static JobRules {
        super::super::rules_for(job).expect("a wood job has rules")
    }

    #[test]
    fn four_wood_jobs_and_no_duplicate() {
        assert_eq!(RULES.len(), 4);
        let mut seen = Vec::new();
        for row in RULES {
            assert!(!seen.contains(&row.job), "{} twice", row.job.path());
            seen.push(row.job);
            assert_eq!(row.family, Family::Wood);
        }
        for job in [Job::Lumberjack, Job::Planter, Job::Florist, Job::Beekeeper] {
            assert!(seen.contains(&job), "{} is missing", job.path());
        }
    }

    #[test]
    fn the_forester_works_at_a_tree_and_the_other_three_do_not() {
        assert_eq!(rules(Job::Lumberjack).places[0].spot, Spot::Tree);
        assert_eq!(rules(Job::Lumberjack).places[0].reach, Some(2), ":657");
        assert_eq!(rules(Job::Florist).places[0].spot, Spot::Hut);
        assert_eq!(rules(Job::Florist).places[0].reach, Some(50), "MAX_DISTANCE");
        assert_eq!(rules(Job::Planter).places[0].spot, Spot::Field);
        assert_eq!(rules(Job::Beekeeper).places[0].spot, Spot::Hive);
        assert_eq!(rules(Job::Beekeeper).places[0].reach, Some(22), "HIVE_BEE_RADIUS");
    }

    #[test]
    fn the_florist_pays_a_flower_two_experience_and_a_tree_one() {
        assert_eq!(rules(Job::Lumberjack).xp[0].per_action, XP_PER_TREE);
        assert_eq!(rules(Job::Florist).xp[0].per_action, XP_PER_FLOWER);
        assert_eq!(rules(Job::Planter).xp[0].per_action, XP_PER_HARVEST);
        assert_eq!(rules(Job::Beekeeper).xp[0].per_action, EXP_PER_HARVEST);
        // and the three that break a block say so a second time
        for job in [Job::Lumberjack, Job::Florist, Job::Planter] {
            assert_eq!(rules(job).xp[1].per_action, XP_PER_BLOCK, "{}", job.path());
            assert_eq!(rules(job).xp[1].name, "XP_PER_BLOCK");
        }
        assert_eq!(rules(Job::Beekeeper).xp.len(), 1, "it breaks nothing");
    }

    #[test]
    fn the_forester_is_the_only_heavy_one_and_the_only_fixed_dump_of_the_family() {
        assert_eq!(rules(Job::Lumberjack).saturation, SATURATION_HEAVY);
        for job in [Job::Planter, Job::Florist, Job::Beekeeper] {
            assert_eq!(rules(job).saturation, SATURATION_DEFAULT, "{}", job.path());
        }
        assert_eq!(rules(Job::Lumberjack).dump.at(5), Some(32), "fixed, not per level");
        assert_eq!(rules(Job::Florist).dump.at(5), Some(50), "10 * 5");
        assert_eq!(rules(Job::Beekeeper).dump.at(5), Some(5));
        assert_eq!(
            rules(Job::Planter).dump.at(5),
            None,
            "the plantation module answers"
        );
    }

    #[test]
    fn the_florist_computes_its_own_mining_time_from_ten_ticks() {
        let florist = rules(Job::Florist);
        assert_eq!(florist.produces[0].work, Work::Own);
        let base = florist
            .durations
            .iter()
            .find(|constant| constant.name == "BASE_BLOCK_MINING_DELAY")
            .expect("the florist overrides the block delay");
        assert_eq!(base.value, 10);
        assert_eq!(base.source, "EntityAIWorkFlorist.java:75");
        // 10 * (1 + max(0, 5 - 0.1 * skill/2)): 60 ticks at 0, 50 at 20, 10 at 100
        for (skill, ticks) in [(0, 60), (20, 50), (100, 10)] {
            let bonus = 5.0 - 0.1 * (f64::from(skill) / 2.0);
            let ticks_expected = 10 * (1 + if bonus > 0.0 { bonus as i32 } else { 0 });
            assert_eq!(ticks_expected, ticks, "skill {skill}");
        }
        // and it is the only job of the family with a mining time of its own
        assert!(rules(Job::Lumberjack).produces[0].work.is_block());
        assert!(rules(Job::Planter).produces[0].work.is_block());
    }

    #[test]
    fn the_beekeeper_takes_three_comb_or_one_bottle() {
        let beekeeper = rules(Job::Beekeeper);
        assert_eq!(beekeeper.produces.len(), 2);
        let comb = beekeeper.produces[0].output;
        assert_eq!(
            comb,
            Output::Fixed { item: "minecraft:honeycomb", count: 3 },
            "Compatibility.getCombsFromHive(hive, world, 3)"
        );
        let honey = beekeeper.produces[1].output;
        assert_eq!(honey, Output::Fixed { item: "minecraft:honey_bottle", count: 1 });
        assert_eq!(BEES_PER_LEVEL, 3);
        // five of its six delays are its own names
        let names: Vec<&str> = beekeeper.durations.iter().map(|constant| constant.name).collect();
        assert!(names.contains(&"NO_HIVES_DELAY"));
        assert!(names.contains(&"BREEDING_DELAY"));
    }

    #[test]
    fn the_two_tag_rows_name_a_tag_and_the_two_tool_rows_name_the_registry() {
        for row in RULES {
            for accepted in row.accepts {
                if let Role::Tool(kind) = accepted.role {
                    assert_eq!(accepted.item, kind.name(), "{}", row.job.path());
                    assert_eq!(accepted.tool_level, Some(0), "{}", row.job.path());
                }
            }
        }
        assert!(
            rules(Job::Lumberjack)
                .accepts
                .iter()
                .any(|accepted| accepted.item == "#minecraft:saplings")
        );
        // the forester is the only job here that states two tools
        let lumberjack_tools = rules(Job::Lumberjack)
            .accepts
            .iter()
            .filter(|accepted| matches!(accepted.role, Role::Tool(_)))
            .count();
        assert_eq!(lumberjack_tools, 2, "axe and hoe, EntityAIWorkLumberjack.java:295");
        assert_eq!(rules(Job::Lumberjack).job.tools(), &[ToolKind::Axe, ToolKind::Hoe]);
    }
}
