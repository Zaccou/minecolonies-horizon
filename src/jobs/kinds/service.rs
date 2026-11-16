//! `service` — couriers, medicine, the grave, the university, the school and the
//! building site: eight jobs that produce **no item at all**.
//!
//! Java reference: `core/colony/jobs/{JobDeliveryman,JobHealer,JobUndertaker,
//! JobResearch,JobTeacher,JobStudent,JobPupil,JobBuilder}.java`, the AI classes
//! `service/{EntityAIWorkDeliveryman,EntityAIWorkHealer,EntityAIWorkUndertaker}.java`,
//! `education/{EntityAIWorkResearcher,EntityAIWorkTeacher,EntityAIStudy,
//! EntityAIWorkPupil}.java`, `builder/EntityAIStructureBuilder.java` and the two
//! structure parents.
//!
//! **The family's finding: `produces` is empty for six of the eight, and that is
//! the honest answer.** A courier moves a stack, a doctor restores health, a
//! teacher hands experience to somebody else, a student raises its own
//! Intelligence, a pupil sits on a carpet, a researcher writes a point into a
//! research the *colony* owns. None of those is a stack, an entity or a recipe, so
//! none of them is an [`Output`]; each row says what it does in its comment and
//! keeps its **duration**, its **XP** and its **place**, which are the three
//! things a caller needs. The two exceptions are real products: the undertaker
//! resurrects a **citizen** ([`Output::Entity`], `EntityAIWorkUndertaker.java:357-374`)
//! and the builder raises a **blueprint** ([`Output::Recipe`], the schematic being
//! data).
//!
//! **Two jobs of this family level somebody else.** The teacher pays
//! `0.1` worker XP and hands the pupil
//! `1.5 × (1 + Intelligence / 10) × (1 + teaching research) × (1 + Knowledge / 10)`
//! (`EntityAIWorkTeacher.java:163-169`) — up to **about 178 XP a lesson** at
//! Intelligence 99 and Knowledge 99. The pupil, sitting on its carpet, keeps
//! `50 × (1 + teaching research)` of Intelligence per session with a piece of paper
//! (`EntityAIWorkPupil.java:186-188`). Neither of those is capped by the job: the
//! cap is `(home level + 1) × 10` and 99, enforced in
//! `CitizenSkillHandler.addXpToSkill` (`:189-190`), which the port already carries
//! as `crate::jobs::experience`'s cap.
//!
//! **Two constants of this family are dead in Java and are not carried**: the
//! courier's `MIN_DISTANCE_TO_WAREHOUSE = 5` (`EntityAIWorkDeliveryman.java:62`,
//! never read) and the undertaker's `XP_PER_WANDER = 2`
//! (`UndertakerConstants.java:21`, never read). The report lists them.

use super::{
    chance, constant, count, dump, effort, material, place, product, tool_at_floor, xp, DumpScale,
    Family, JobRules, Output, Role, Spot, Work, MIN_OPEN_SLOTS, SATURATION_DEFAULT,
    SATURATION_HEAVY, WORK_IN_BUILDING_DIST, XP_EACH_BUILDING, XP_PER_BLOCK,
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
            family: Family::Service,
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

/// `EntityAIWorkDeliveryman.DECISION_DELAY`: five seconds between two delivery
/// decisions (`EntityAIWorkDeliveryman.java:67`, read at `:118`).
pub const DECISION_DELAY: i32 = 100;

/// `EntityAIWorkDeliveryman.PICKUP_DELAY`: the period of the pickup state
/// (`EntityAIWorkDeliveryman.java:72`, read at `:121`).
pub const PICKUP_DELAY: i32 = 5;

/// `EntityAIWorkDeliveryman.PRIORITY_FORCING_DUMP`: above this priority the courier
/// empties its hands before delivering (`EntityAIWorkDeliveryman.java:82`, read at
/// `:188`).
pub const PRIORITY_FORCING_DUMP: i32 = 10;

/// `EntityAIWorkHealer.BASE_XP_GAIN`: what one cure is worth
/// (`EntityAIWorkHealer.java:43`, granted at `:336`, `:379`, `:433`).
pub const HEALER_XP: f64 = 2.0;

/// `EntityAIWorkHealer.REQUEST_COUNT`: how many cure items one request asks for —
/// sixteen (`EntityAIWorkHealer.java:48`, read at `:296`).
pub const CURE_REQUEST_COUNT: i32 = 16;

/// `EntityAIWorkHealer.MAX_PROGRESS_TICKS`: thirty ticks times the twenty-tick cure
/// state, i.e. **600 ticks** for a free cure (`EntityAIWorkHealer.java:63`, read at
/// `:412-437`).
pub const FREE_CURE_PROGRESS_TICKS: i32 = 30;

/// `UndertakerConstants.EFFORT_EMPTY_GRAVE`: effort points to empty a grave
/// (`api/util/constant/UndertakerConstants.java:61`, read at
/// `EntityAIWorkUndertaker.java:210`).
pub const EFFORT_EMPTY_GRAVE: i32 = 100;

/// `UndertakerConstants.EFFORT_BURY`: effort points to bury a citizen
/// (`UndertakerConstants.java:66`, read at `EntityAIWorkUndertaker.java:475`).
pub const EFFORT_BURY: i32 = 400;

/// `UndertakerConstants.EFFORT_RESURRECT`: effort points to try a resurrection
/// (`UndertakerConstants.java:71`, read at `EntityAIWorkUndertaker.java:337`).
pub const EFFORT_RESURRECT: i32 = 400;

/// `UndertakerConstants.XP_PER_DIG`: what digging a grave pays, to the **Strength**
/// skill rather than to the worker total (`UndertakerConstants.java:16`, granted at
/// `EntityAIWorkUndertaker.java:270`).
pub const XP_PER_DIG: f64 = 7.5;

/// `UndertakerConstants.RESURRECT_BUILDING_LVL_WEIGHT`: the first term of the
/// resurrection chance (`UndertakerConstants.java:26`, read at
/// `EntityAIWorkUndertaker.java:387-403`).
pub const RESURRECT_BUILDING_LVL_WEIGHT: f64 = 0.005;

/// `UndertakerConstants.RESURRECT_WORKER_MANA_LVL_WEIGHT`: the second term
/// (`UndertakerConstants.java:31`).
pub const RESURRECT_WORKER_MANA_LVL_WEIGHT: f64 = 0.00125;

/// `UndertakerConstants.MAX_RESURRECTION_CHANCE`: the floor of the cap
/// (`UndertakerConstants.java:36`).
pub const MAX_RESURRECTION_CHANCE: f64 = 0.025;

/// `UndertakerConstants.MAX_RESURRECTION_CHANCE_MYSTICAL_LVL_BONUS`: what one level
/// of mystical site adds to the cap (`UndertakerConstants.java:41`).
pub const MAX_RESURRECTION_CHANCE_MYSTICAL_LVL_BONUS: f64 = 0.005;

/// `EntityAIWorkResearcher.STUDY_DELAY`: a minute between two study actions, and the
/// walking budget of the same state (`EntityAIWorkResearcher.java:24`, read at
/// `:103`, `:75`).
pub const STUDY_DELAY: i32 = 1200;

/// `EntityAIWorkResearcher.XP_PER_STUDYPOS`: what one study action pays
/// (`EntityAIWorkResearcher.java:29`, granted at `:100`).
pub const XP_PER_STUDYPOS: f64 = 2.0;

/// `JobResearch.MANA_PER_SECOND`: the budget divisor, `(4 × 60 × 60) / 100`
/// (`JobResearch.java:23`, read at `:82`).
pub const MANA_PER_SECOND: i32 = 144;

/// `EntityAIWorkTeacher.PAPER_TO_REQUEST`: sixteen sheets
/// (`EntityAIWorkTeacher.java:39`, read at `:100`, `:214`).
pub const PAPER_TO_REQUEST: i32 = 16;

/// `EntityAIWorkTeacher`: the dump threshold, fifty lessons
/// (`EntityAIWorkTeacher.java:200-204`).
pub const TEACHER_DUMP: i32 = 50;

/// `EntityAIStudy.STUDY_DELAY`: a minute per study at the library
/// (`EntityAIStudy.java:54`, read at `:117`, `:187`).
pub const LIBRARY_STUDY_DELAY: i32 = 1200;

/// `EntityAIStudy.ONE_IN_X_CHANCE`: the chance denominator a student passes to
/// `tryLevelUpIntelligence` (`EntityAIStudy.java:49`, read at `:161`, `:171`).
pub const ONE_IN_X_CHANCE: i32 = 8;

/// `EntityAIStudy`: the Intelligence a successful study buys, as
/// `CitizenSkillHandler.tryLevelUpIntelligence` grants it
/// (`CitizenSkillHandler.java:161`).
pub const STUDY_INTELLIGENCE_XP: f64 = 10.0;

/// `EntityAIWorkPupil.STUDY_TO_RECESS_RATIO`: one recess in ten
/// (`EntityAIWorkPupil.java:42`, read at `:89`, `:113`).
pub const STUDY_TO_RECESS_RATIO: i32 = 10;

/// `EntityAIWorkPupil`: what one lesson with a sheet of paper is worth, before the
/// teaching research (`EntityAIWorkPupil.java:186-188`).
pub const PUPIL_INTELLIGENCE_XP: f64 = 50.0;

/// `EntityAIStructureBuilder.ACTIONS_UNTIL_DUMP`: the builder empties its inventory
/// after **4 096** actions, the largest threshold of the port
/// (`EntityAIStructureBuilder.java:50`, read at `:290-293`).
pub const BUILDER_DUMP: i32 = 4096;

/// `EntityAIStructure.BUILD_BLOCK_DELAY`: the numerator of the placing formula, in
/// ticks (`AbstractEntityAIStructure.java:86`, read at `:531-532`).
pub const BUILD_BLOCK_DELAY: i32 = 15;

/// `EntityAIStructureBuilder.SPEED_BUFF_0`: the builder mines a block **in half the
/// time** (`EntityAIStructureBuilder.java:45`, read at `:279-282`).
pub const BUILDER_MINING_SPEED: f64 = 0.5;

/// `EntityAIStructureBuilder.LEVEL_TO_PURGE_MOBS`: from hut level four it clears the
/// monsters of its site once a day (`EntityAIStructureBuilder.java:55`, read at
/// `:168`).
pub const LEVEL_TO_PURGE_MOBS: i32 = 4;

/// The line of the builder's standing distance, four blocks from the block it works
/// on (`EntityAIStructureBuilder.java:220-224`).
const BUILDER_REACH_LINE: &str = "EntityAIStructureBuilder.java:220";

/// The eight service jobs, in `Job::ALL` order of the family.
pub const RULES: &[JobRules] = &[
    // The builder. It raises **one blueprint at a time**, one block per state pass,
    // and its placing time is a formula: `15 * 10 / (placing skill / 2 + 10)`, with
    // Java's integer division on the first term, times `(1 - block place research)`
    // — so **15 ticks at skill 0, 2 at skill 98** (`AbstractEntityAIStructure.java:531-532`).
    // Mining a block costs half the usual time (`EntityAIStructureBuilder.java:279-282`).
    // It stands **four blocks** from the block it works on (`:220-224`) and must be
    // within ten of the previous one (`:241`). It keeps **five slots open** so a
    // delivery can land (`:251`, `MIN_OPEN_SLOTS`) and dumps after 4 096 actions
    // (`:290-293`). It states **five tools** at the floor level, `BuildingBuilder.java:66-70`.
    rules!(
        Builder,
        &[place(Spot::Site, Some(4), BUILDER_REACH_LINE)],
        [
            tool_at_floor(ToolKind::Pickaxe, "BuildingBuilder.java:66"),
            tool_at_floor(ToolKind::Shovel, "BuildingBuilder.java:67"),
            tool_at_floor(ToolKind::Axe, "BuildingBuilder.java:68"),
            tool_at_floor(ToolKind::Hoe, "BuildingBuilder.java:69"),
            tool_at_floor(ToolKind::Shears, "BuildingBuilder.java:70"),
        ],
        [product(Output::Recipe, Work::Own, "AbstractEntityAIStructure.java:531")],
        [
            constant("BUILD_BLOCK_DELAY", BUILD_BLOCK_DELAY, "AbstractEntityAIStructure.java:86"),
            count("MIN_OPEN_SLOTS", MIN_OPEN_SLOTS, "CitizenConstants.java:176"),
            count("LEVEL_TO_PURGE_MOBS", LEVEL_TO_PURGE_MOBS, "EntityAIStructureBuilder.java:55"),
        ],
        [
            xp(XP_EACH_BUILDING, "XP_EACH_BUILDING", "AbstractEntityAIStructure.java:279"),
            xp(XP_PER_BLOCK, "XP_PER_BLOCK", "AbstractEntityAIInteract.java:234"),
        ],
        dump(BUILDER_DUMP, DumpScale::Fixed, "EntityAIStructureBuilder.java:290"),
        SATURATION_DEFAULT,
        "EntityAIStructureBuilder.java:1"
    ),
    // The courier. It carries **every request that shares a source and a target**,
    // up to `1 + Adaptability / 5` of them — **one at level 0, twenty at 99**
    // (`JobDeliveryman.java:511-518`, `:294-305`) — and its inventory holds
    // `2^(level - 1) + 1` stacks, unlimited at hut level 5 (`EntityAIWorkDeliveryman.java:269-276`).
    // It moves faster by `0.003` per Agility level (`JobDeliveryman.java:55`, `:95-97`).
    // A pickup pays `0.05` and a delivery **1.5** (`EntityAIWorkDeliveryman.java:186`,
    // `:461`). It refuses to eat a stack it is delivering
    // (`BuildingDeliveryman.java:56-67`). `MIN_DISTANCE_TO_WAREHOUSE = 5`
    // (`:62`) is **dead**.
    rules!(
        Deliveryman,
        &[place(Spot::Hut, None, "JobDeliveryman.java:571")],
        [],
        [],
        [
            constant("DECISION_DELAY", DECISION_DELAY, "EntityAIWorkDeliveryman.java:67"),
            constant("PICKUP_DELAY", PICKUP_DELAY, "EntityAIWorkDeliveryman.java:72"),
            count(
                "PRIORITY_FORCING_DUMP",
                PRIORITY_FORCING_DUMP,
                "EntityAIWorkDeliveryman.java:82"
            ),
        ],
        [
            xp(1.5, "addExperience(1.5)", "EntityAIWorkDeliveryman.java:461"),
            xp(0.05, "addExperience(0.05)", "EntityAIWorkDeliveryman.java:186"),
        ],
        dump(super::ACTIONS_UNTIL_DUMP, DumpScale::Fixed, "AbstractEntityAIBasic.java:425"),
        SATURATION_HEAVY,
        "EntityAIWorkDeliveryman.java:1"
    ),
    // The doctor. One patient at a time (`EntityAIWorkHealer.java:53`), from the
    // citizens **inside the building** (`:112-113`). A wounded citizen is healed
    // **10 hit points** (`:335`); a player is healed to `max - 5 - hut level`
    // (`:463`); a distant citizen, one below **10 HP** and within `level × 40`
    // blocks, is healed the same way (`:226-232`, up to 200 blocks at level 5). A
    // cure costs sixteen items per request (`:48`, `:296`), and a free cure needs
    // `30 × 20 = 600 ticks` (`:63`, `:412-437`). The free cure *roll* is
    // `1 / 3600` per decision, improved by Mana (`:513-516`). It **never falls ill**
    // itself (`JobHealer.java:73-77`).
    rules!(
        Healer,
        &[place(Spot::Hut, None, "EntityAIWorkHealer.java:112")],
        [
            material("minecraft:carrot", Role::Cure, "EntityAIWorkHealer.java:296"),
            material("minecraft:potato", Role::Cure, "EntityAIWorkHealer.java:296"),
            material("minecraft:poppy", Role::Cure, "EntityAIWorkHealer.java:296"),
            material("minecraft:dandelion", Role::Cure, "EntityAIWorkHealer.java:296"),
            material("minecraft:kelp", Role::Cure, "EntityAIWorkHealer.java:296"),
            material("minecraft:honey_bottle", Role::Cure, "EntityAIWorkHealer.java:296"),
            material("minecraft:golden_apple", Role::Cure, "EntityAIWorkHealer.java:296"),
        ],
        [],
        [
            constant(
                "MAX_PROGRESS_TICKS",
                FREE_CURE_PROGRESS_TICKS,
                "EntityAIWorkHealer.java:63"
            ),
            count("REQUEST_COUNT", CURE_REQUEST_COUNT, "EntityAIWorkHealer.java:48"),
        ],
        [
            xp(HEALER_XP, "BASE_XP_GAIN", "EntityAIWorkHealer.java:336"),
            xp(1.0, "addExperience(1)", "EntityAIWorkHealer.java:464"),
        ],
        dump(super::ACTIONS_UNTIL_DUMP, DumpScale::Fixed, "AbstractEntityAIBasic.java:425"),
        SATURATION_DEFAULT,
        "EntityAIWorkHealer.java:1"
    ),
    // The pupil. It sits on a carpet of the school (`BuildingSchool.java:126-146`) for
    // **60 to 119 seconds** (`nextInt(60) + 60`, times twenty ticks,
    // `EntityAIWorkPupil.java:149-150`), one recess in ten (`:42`), and keeps
    // **50 × (1 + teaching research)** Intelligence if it holds a sheet of paper,
    // nothing at all otherwise (`:186-188`). No worker XP: it is the one job whose
    // whole output is a skill.
    rules!(
        Pupil,
        &[place(Spot::Desk, None, "BuildingSchool.java:126")],
        [material("minecraft:paper", Role::Material, "EntityAIWorkPupil.java:47")],
        [],
        [count(
            "STUDY_TO_RECESS_RATIO",
            STUDY_TO_RECESS_RATIO,
            "EntityAIWorkPupil.java:42"
        )],
        [xp(
            PUPIL_INTELLIGENCE_XP,
            "addXpToSkill(Intelligence, 50 * (1 + TEACHING))",
            "EntityAIWorkPupil.java:188"
        )],
        dump(super::ACTIONS_UNTIL_DUMP, DumpScale::Fixed, "AbstractEntityAIBasic.java:425"),
        SATURATION_DEFAULT,
        "EntityAIWorkPupil.java:1"
    ),
    // The researcher. It studies at a bookshelf of the university
    // (`BuildingUniversity.java:100-128`) for **1 200 ticks — one minute — per
    // point** (`EntityAIWorkResearcher.java:24`), spending one mana each and paying
    // **2 XP** per position (`:29`, `:100`). Its mana budget is
    // `144 × Mana level` seconds (`JobResearch.java:23`, `:82`) refreshed at
    // `25` study slots a second (`:28`, `:85-88`); the university refills it offline
    // from hut level 3 (`BuildingUniversity.java:48`, `:181-190`). No item, no tool.
    rules!(
        Researcher,
        &[place(Spot::Desk, Some(WORK_IN_BUILDING_DIST), "BuildingUniversity.java:100")],
        [],
        [],
        [
            constant("STUDY_DELAY", STUDY_DELAY, "EntityAIWorkResearcher.java:24"),
            count("MANA_PER_SECOND", MANA_PER_SECOND, "JobResearch.java:23"),
        ],
        [xp(XP_PER_STUDYPOS, "XP_PER_STUDYPOS", "EntityAIWorkResearcher.java:100")],
        dump(super::ACTIONS_UNTIL_DUMP, DumpScale::Fixed, "AbstractEntityAIBasic.java:425"),
        SATURATION_DEFAULT,
        "EntityAIWorkResearcher.java:1"
    ),
    // The student. It studies at a **random bookshelf** of the library
    // (`EntityAIStudy.java:112-115`) for **1 200 ticks** (`:54`) and either rolls for
    // Intelligence on a `1 in 8` chance — 87.5 % of a 10-point gain — or studies a
    // book (25 % a gain, 10 % to be consumed) or a sheet (50 % and always consumed)
    // (`:140-181`, `data/minecolonies/study_items/{book,paper}.json`). It asks for
    // **100** study items at a time (`:155`). The cap is the citizen's, not the job's.
    rules!(
        Student,
        &[place(Spot::Desk, Some(4), "AbstractEntityAIBasic.java:830")],
        [
            material("minecraft:book", Role::Material, "EntityAIStudy.java:149"),
            material("minecraft:paper", Role::Material, "EntityAIStudy.java:149"),
        ],
        [],
        [
            constant("STUDY_DELAY", LIBRARY_STUDY_DELAY, "EntityAIStudy.java:54"),
            chance("ONE_IN_X_CHANCE", ONE_IN_X_CHANCE, "EntityAIStudy.java:49"),
        ],
        [xp(
            STUDY_INTELLIGENCE_XP,
            "addXpToSkill(Intelligence, 10)",
            "CitizenSkillHandler.java:161"
        )],
        dump(super::ACTIONS_UNTIL_DUMP, DumpScale::Fixed, "AbstractEntityAIBasic.java:425"),
        SATURATION_DEFAULT,
        "EntityAIStudy.java:1"
    ),
    // The teacher. It takes **one pupil at a time** from the children sitting inside
    // the school (`EntityAIWorkTeacher.java:104-110`), teaches for
    // `100 / max(1, Mana / 2)` half-seconds — **50 to 99 seconds at Mana 0, one to
    // two at Mana 99** (`:133-136`) — and hands over
    // `1.5 × (1 + Intelligence / 10) × (1 + teaching) × (1 + Knowledge / 10)`
    // (`:163-169`), plus a sheet of paper if the child has none (`:151-160`). It
    // pays itself **0.1 XP** and dumps after fifty lessons (`:169`, `:200-204`).
    rules!(
        Teacher,
        &[place(Spot::Desk, Some(WORK_IN_BUILDING_DIST), "EntityAIWorkTeacher.java:104")],
        [material("minecraft:paper", Role::Material, "EntityAIWorkTeacher.java:44")],
        [],
        [count("PAPER_TO_REQUEST", PAPER_TO_REQUEST, "EntityAIWorkTeacher.java:39")],
        [xp(0.1, "addExperience(0.1)", "EntityAIWorkTeacher.java:169")],
        dump(TEACHER_DUMP, DumpScale::Fixed, "EntityAIWorkTeacher.java:200"),
        SATURATION_DEFAULT,
        "EntityAIWorkTeacher.java:1"
    ),
    // The undertaker. The only job of the family that **produces a living thing**: a
    // resurrected citizen (`EntityAIWorkUndertaker.java:357-374`), one grave at a
    // time, on a resurrection chance of
    // `hut level × 0.005 + Mana × 0.00125 + research + totem`, capped at
    // `0.025 + mystical site level × 0.005 + totem` (`UndertakerConstants.java:26-51`,
    // read at `EntityAIWorkUndertaker.java:387-403`). Its three times are **effort
    // points, not ticks**: 100 to empty a grave, 400 to bury, 400 to try a
    // resurrection, spent `skill` at a time every `STANDARD_DELAY` — so
    // `400 / Strength × 5` ticks (`UndertakerConstants.java:61-71`). Digging pays
    // **7.5 to Strength** (`:16`), which is the one grant of the port that goes to a
    // named skill rather than to the worker total. Its shovel is the floor tool, and
    // a graveyard of level 5 takes any shovel at all
    // (`AbstractEntityAIBasic.java:966-1005`).
    rules!(
        Undertaker,
        &[place(Spot::Grave, Some(3), "EntityAIWorkUndertaker.java:192")],
        [tool_at_floor(ToolKind::Shovel, "EntityAIWorkUndertaker.java:180")],
        [product(
            Output::Entity("minecolonies:citizen"),
            Work::Own,
            "EntityAIWorkUndertaker.java:357"
        )],
        [
            effort(
                "EFFORT_EMPTY_GRAVE",
                EFFORT_EMPTY_GRAVE,
                "UndertakerConstants.java:61"
            ),
            effort("EFFORT_BURY", EFFORT_BURY, "UndertakerConstants.java:66"),
            effort(
                "EFFORT_RESURRECT",
                EFFORT_RESURRECT,
                "UndertakerConstants.java:71"
            ),
        ],
        [
            xp(XP_PER_DIG, "XP_PER_DIG", "EntityAIWorkUndertaker.java:270"),
            xp(XP_PER_BLOCK, "XP_PER_BLOCK", "AbstractEntityAIInteract.java:234"),
        ],
        dump(super::ACTIONS_UNTIL_DUMP, DumpScale::Fixed, "AbstractEntityAIBasic.java:425"),
        SATURATION_DEFAULT,
        "EntityAIWorkUndertaker.java:1"
    ),
];

/// The resurrection chance of a graveyard, `EntityAIWorkUndertaker.java:387-403`.
///
/// ```text
/// chance = hut level * 0.005 + mana * 0.00125 + research + totem
/// cap    = 0.025 + mystical site level * 0.005 + totem
/// answer = min(chance, cap)
/// ```
///
/// Every term is a named constant above, and none of them is this port's own
/// invention — which is why the function is here and not in the row: a caller with
/// a colony asks it, and a test can walk it without a grave.
pub fn resurrection_chance(
    hut_level: i32,
    mana_level: i32,
    research: f64,
    mystical_site_level: i32,
    totem_bonus: f64,
) -> f64 {
    let chance = f64::from(hut_level) * RESURRECT_BUILDING_LVL_WEIGHT
        + f64::from(mana_level) * RESURRECT_WORKER_MANA_LVL_WEIGHT
        + research
        + totem_bonus;
    let cap = MAX_RESURRECTION_CHANCE
        + f64::from(mystical_site_level) * MAX_RESURRECTION_CHANCE_MYSTICAL_LVL_BONUS
        + totem_bonus;
    if chance < cap { chance } else { cap }
}

/// The experience a teacher hands one pupil, `EntityAIWorkTeacher.java:163-165`.
///
/// ```text
/// xp = 1.5 * (1 + intelligence / 10) * (1 + teaching research) * (1 + knowledge / 10)
/// ```
pub fn lesson_xp(teacher_intelligence: i32, teacher_knowledge: i32, teaching: f64) -> f64 {
    1.5 * (1.0 + f64::from(teacher_intelligence) / 10.0)
        * (1.0 + teaching)
        * (1.0 + f64::from(teacher_knowledge) / 10.0)
}

/// How long one block takes a builder to place, in ticks —
/// `AbstractEntityAIStructure.java:531-532`.
///
/// ```java
/// final double decrease = 1 - ...getEffectStrength(BLOCK_PLACE_SPEED);
/// setDelay((int) ((BUILD_BLOCK_DELAY * PROGRESS_MULTIPLIER
///                   / (getPlaceSpeedLevel() / 2 + 10)) * decrease));
/// ```
///
/// Java divides the **first** term as an integer: `15 * 10 / (level / 2 + 10)`, so
/// 150/10 = 15 ticks at skill 0, 150/20 = 7 at skill 20, 150/59 = 2 at skill 98.
pub fn block_place_ticks(place_speed_level: i32, block_place_research: f64) -> i32 {
    let divisor = place_speed_level / 2 + 10;
    let base = BUILD_BLOCK_DELAY * super::PROGRESS_MULTIPLIER / divisor;
    let decrease = 1.0 - block_place_research;
    (f64::from(base) * decrease) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(job: Job) -> &'static JobRules {
        super::super::rules_for(job).expect("a service job has rules")
    }

    #[test]
    fn eight_service_jobs_and_no_duplicate() {
        assert_eq!(RULES.len(), 8);
        let mut seen = Vec::new();
        for row in RULES {
            assert!(!seen.contains(&row.job), "{} twice", row.job.path());
            seen.push(row.job);
            assert_eq!(row.family, Family::Service);
        }
        for job in [
            Job::Builder,
            Job::Deliveryman,
            Job::Healer,
            Job::Pupil,
            Job::Researcher,
            Job::Student,
            Job::Teacher,
            Job::Undertaker,
        ] {
            assert!(seen.contains(&job), "{} is missing", job.path());
        }
    }

    #[test]
    fn six_of_the_eight_produce_no_item_and_the_two_that_do_say_what_they_make() {
        for job in [
            Job::Deliveryman,
            Job::Healer,
            Job::Pupil,
            Job::Researcher,
            Job::Student,
            Job::Teacher,
        ] {
            assert!(rules(job).produces.is_empty(), "{} names a product", job.path());
        }
        assert_eq!(
            rules(Job::Undertaker).produces[0].output,
            Output::Entity("minecolonies:citizen"),
            "it resurrects one (`EntityAIWorkUndertaker.java:357-374`)"
        );
        assert_eq!(rules(Job::Builder).produces[0].output, Output::Recipe);
    }

    #[test]
    fn only_the_builder_states_five_tools_and_the_undertaker_its_shovel() {
        let builder = rules(Job::Builder);
        let tools: Vec<&str> = builder
            .accepts
            .iter()
            .filter(|accepted| matches!(accepted.role, Role::Tool(_)))
            .map(|accepted| accepted.item)
            .collect();
        assert_eq!(tools, vec!["pickaxe", "shovel", "axe", "hoe", "shears"]);
        assert_eq!(builder.places[0].reach, Some(4), "four blocks from the block");
        for row in RULES {
            if row.job == Job::Builder || row.job == Job::Undertaker {
                continue;
            }
            assert!(
                !row.accepts
                    .iter()
                    .any(|accepted| matches!(accepted.role, Role::Tool(_))),
                "{} states a tool",
                row.job.path()
            );
        }
        // the undertaker's single row is its shovel, at the floor level
        let undertaker = rules(Job::Undertaker);
        assert_eq!(undertaker.accepts.len(), 1);
        assert_eq!(undertaker.accepts[0].item, "shovel");
        assert_eq!(undertaker.accepts[0].tool_level, Some(0));
    }

    #[test]
    fn the_builder_places_a_block_in_fifteen_ticks_and_two_at_high_skill() {
        assert_eq!(block_place_ticks(0, 0.0), 15);
        assert_eq!(block_place_ticks(20, 0.0), 7, "150 / 20, integer division");
        assert_eq!(block_place_ticks(98, 0.0), 2, "150 / 59");
        assert_eq!(block_place_ticks(0, 0.5), 7);
        assert_eq!(block_place_ticks(0, 1.0), 0);
        assert_eq!(BUILD_BLOCK_DELAY, 15);
        assert_eq!(super::super::PROGRESS_MULTIPLIER, 10);
    }

    #[test]
    fn the_builder_dumps_after_the_largest_threshold_of_the_port() {
        assert_eq!(rules(Job::Builder).dump.at(5), Some(4096));
        let largest = super::super::all()
            .filter_map(|row| row.dump.at(5))
            .max()
            .expect("some threshold");
        assert_eq!(largest, BUILDER_DUMP);
    }

    #[test]
    fn the_teacher_s_lesson_is_one_and_a_half_xp_at_level_zero_and_a_hundred_and_seventy_eight_at_ninety_nine() {
        assert!((lesson_xp(0, 0, 0.0) - 1.5).abs() < 1e-9);
        // 1.5 * 10.9 * 10.9 = 178.215
        assert!((lesson_xp(99, 99, 0.0) - 178.215).abs() < 1e-9);
        // the teaching research multiplies the whole thing
        assert!((lesson_xp(0, 0, 1.0) - 3.0).abs() < 1e-9);
        assert!((lesson_xp(10, 0, 0.0) - 3.0).abs() < 1e-9, "intelligence 10 doubles");
    }

    #[test]
    fn the_pupil_keeps_fifty_intelligence_a_lesson_and_the_teacher_only_a_tenth_of_xp() {
        assert_eq!(rules(Job::Pupil).xp[0].per_action, PUPIL_INTELLIGENCE_XP);
        assert_eq!(rules(Job::Teacher).xp[0].per_action, 0.1);
        assert_eq!(PUPIL_INTELLIGENCE_XP / rules(Job::Teacher).xp[0].per_action, 500.0);
        assert_eq!(STUDY_TO_RECESS_RATIO, 10);
        // the teacher's dump threshold is its own, not the base 32
        assert_eq!(rules(Job::Teacher).dump.at(1), Some(50));
    }

    #[test]
    fn the_student_rolls_a_chance_and_the_constant_says_it_is_a_denominator() {
        let student = rules(Job::Student);
        let denominator = student
            .durations
            .iter()
            .find(|constant| constant.name == "ONE_IN_X_CHANCE")
            .expect("the chance");
        assert_eq!(denominator.value, 8);
        assert_eq!(denominator.unit, super::super::Unit::ChanceDenominator);
        // tryLevelUpIntelligence refuses with probability 1 / chance, so the gain is
        // 1 - 1/8 = 87.5 %
        let gain = 1.0 - 1.0 / f64::from(ONE_IN_X_CHANCE);
        assert!((gain - 0.875).abs() < 1e-9);
        assert_eq!(STUDY_INTELLIGENCE_XP, 10.0);
        // a book is 8 * (10 / 60) = 1.33, i.e. 25 % of a gain; a sheet is 8 * (10/40)
        let book = 1.0 - 1.0 / (f64::from(ONE_IN_X_CHANCE) * (10.0 / 60.0));
        let paper = 1.0 - 1.0 / (f64::from(ONE_IN_X_CHANCE) * (10.0 / 40.0));
        assert!((book - 0.25).abs() < 1e-9);
        assert!((paper - 0.5).abs() < 1e-9);
    }

    #[test]
    fn the_undertaker_s_three_numbers_are_effort_points_not_ticks() {
        let undertaker = rules(Job::Undertaker);
        assert_eq!(undertaker.durations.len(), 3);
        for constant in undertaker.durations {
            assert_eq!(constant.unit, super::super::Unit::Effort, "{}", constant.name);
        }
        assert_eq!(undertaker.durations[0].value, 100);
        assert_eq!(undertaker.durations[1].value, 400);
        assert_eq!(undertaker.durations[2].value, 400);
        // 400 effort at skill 10 is 40 passes of STANDARD_DELAY, i.e. 200 ticks
        assert_eq!(EFFORT_BURY / 10 * super::super::STANDARD_DELAY, 200);
        assert_eq!(XP_PER_DIG, 7.5);
    }

    #[test]
    fn the_resurrection_chance_is_capped_and_never_reaches_one_in_twenty_five() {
        // a level 1 graveyard with no skill: 0.005, well under the cap
        assert!((resurrection_chance(1, 0, 0.0, 0, 0.0) - 0.005).abs() < 1e-9);
        // a level 5 graveyard and 99 Mana: 0.025 + 0.12375, capped at 0.025
        assert!((resurrection_chance(5, 99, 0.0, 0, 0.0) - 0.025).abs() < 1e-9);
        // one mystical site level buys another half a percent of cap
        assert!((resurrection_chance(5, 99, 0.0, 1, 0.0) - 0.03).abs() < 1e-9);
        // a totem raises the cap as well as the chance, so it always adds
        assert!((resurrection_chance(1, 0, 0.0, 0, 0.05) - 0.055).abs() < 1e-9);
        assert_eq!(MAX_RESURRECTION_CHANCE, 0.025);
        assert_eq!(RESURRECT_WORKER_MANA_LVL_WEIGHT, 0.00125);
    }

    #[test]
    fn the_courier_earns_thirty_times_more_for_a_delivery_than_for_a_pickup() {
        let courier = rules(Job::Deliveryman);
        assert_eq!(courier.xp[0].per_action, 1.5);
        assert_eq!(courier.xp[1].per_action, 0.05);
        assert_eq!(1.5 / 0.05, 30.0);
        assert_eq!(courier.saturation, SATURATION_HEAVY);
        // the parallel-delivery formula is `1 + Adaptability / 5`
        let parallel = |adaptability: i32| 1 + adaptability / 5;
        assert_eq!(parallel(0), 1);
        assert_eq!(parallel(99), 20);
        assert_eq!(DECISION_DELAY, 100);
        assert_eq!(PICKUP_DELAY, 5);
    }

    #[test]
    fn the_doctor_heals_ten_and_never_falls_ill() {
        let healer = rules(Job::Healer);
        assert_eq!(healer.xp[0].per_action, HEALER_XP);
        assert_eq!(healer.xp[1].per_action, 1.0, "a player is worth half a cure");
        assert_eq!(CURE_REQUEST_COUNT, 16);
        assert_eq!(FREE_CURE_PROGRESS_TICKS * 20, 600);
        // the four cures of the data are listed as materials, and the request line is
        // the same for all of them: `new Stack(cure.getItemStack(), 16, 1)` at :296
        assert_eq!(healer.accepts.len(), 7);
        assert!(healer.accepts.iter().all(|accepted| accepted.role == Role::Cure));
        // JobHealer.getDiseaseModifier answers 0, so a doctor never falls ill; the
        // port has no disease module to carry that into, so the row does not state it
        assert_eq!(
            healer.accepts
                .iter()
                .filter(|accepted| accepted.item == "minecraft:honey_bottle")
                .count(),
            1
        );
    }
}
