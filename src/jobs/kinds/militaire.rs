//! `militaire` — six guards and the two training grounds that make them.
//!
//! Java reference: `core/colony/jobs/guard/{JobKnight,JobHuscarl,JobRanger,
//! JobMarksman,JobCavalry,JobDruid}.java`, `core/colony/jobs/{JobArcherTraining,
//! JobCombatTraining}.java`, the guard AI (`guard/AbstractEntityAIGuard.java`,
//! `AbstractEntityAIFight.java`, `EntityAIMelee`, `EntityAIRange`,
//! `EntityAICavalry`, `EntityAIDruid`, `MeleeCombatAI`, `RangeCombatAI`,
//! `CavalryCombatAI`, `DruidCombatAI`) and the shared table
//! `api/util/constant/GuardConstants.java`.
//!
//! **There is no `MAX_GUARD_RADIUS`, and that is the first thing to say.** A
//! guard's leash is the **persecution distance**, which Java computes from the
//! task it is on (`AbstractEntityAIGuard.getPersecutionDistance`, `:757-773`):
//!
//! | task | distance |
//! | --- | --- |
//! | `PATROL` | 80 (`MAX_PATROL_DERIVATION`) |
//! | `PATROL_MINE`, `FOLLOW`, rally | 30 (`MAX_FOLLOW_DERIVATION`) |
//! | `GUARD` | 10 (`MAX_GUARD_DERIVATION`), **plus 20 for a knight and a knight only** |
//!
//! Eleven blocks and a half of surprise: a **huscarl** is a knight in every other
//! respect and gets 10, because the test compares the registry entry against
//! `ModJobs.knight` (`:771`). The port keeps the asymmetry and the report names it.
//!
//! **Two attack delays, three damages, one base.** A melee guard swings every
//! `32 - Adaptability / 3` ticks — never faster than 16 — except a huscarl, who
//! divides by 2 (`MeleeCombatAI.java:361-366`). A ranged guard shoots every 60
//! ticks, or 40 for a marksman (`RangeCombatAI.java:260-264`). And the base damage
//! of any physical guard is `BASE_PHYSICAL_DAMAGE = 3` added to the weapon's own
//! (`MeleeCombatAI.java:310-352`), while an arrow adds `RANGER_BASE_DMG = 2` on top
//! of `Agility / 5` (`RangeCombatAI.java:272-324`).
//!
//! **The druid has no weapon and no damage.** It throws splash potions whose effect
//! it picks from two lists — four beneficial (`DAMAGE_BOOST`, `SATURATION`, `HEAL`,
//! `DAMAGE_RESISTANCE`, `DruidCombatAI.java:56`) and two adverse (`MOVEMENT_SLOWDOWN`,
//! `WEAKNESS`, `:61`). Without a magic potion the roll is forced to the **first**
//! element of each list (`:152`, `:157`), which is why a level-0 druid can only
//! strengthen and only slow down.
//!
//! **The two training grounds pay the trainee, not the colony.** An archery trainee
//! earns **1 XP** per arrow that lands within 1.41 blocks of the target and **0.2**
//! per miss (`EntityAIArcherTraining.java:43`, `:63`, `:221`); a combat trainee
//! earns **2 XP** per swing, at zero damage (`EntityAICombatTraining.java:39`,
//! `:213`). Their constant table also contains the port's cleanest shadowing bug:
//! `EntityAIArcherTraining.java:58` declares `RANGED_ATTACK_DELAY_BASE = 10`, hiding
//! the `60` of `GuardConstants.java:94` under a static import of the same name. The
//! port carries **both** numbers, each with its own line.

use super::{
    constant, count, distance, dump, material, place, product, tool_at_floor, xp, DumpScale,
    Family, JobRules, Output, Role, Spot, Work, SATURATION_DEFAULT, SATURATION_HEAVY,
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
            family: Family::Military,
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

/// `GuardConstants.MAX_GUARD_DERIVATION`: how far a guard chases while it is on its
/// own post (`GuardConstants` neighbour `AbstractEntityAIGuard.java:83`).
pub const MAX_GUARD_DERIVATION: i32 = 10;

/// `AbstractEntityAIGuard.MAX_PATROL_DERIVATION`: how far it chases while
/// patrolling (`AbstractEntityAIGuard.java:68`, read at `:765-766`).
pub const MAX_PATROL_DERIVATION: i32 = 80;

/// `AbstractEntityAIGuard.MAX_FOLLOW_DERIVATION`: how far it chases while
/// following, rallying or guarding a mine (`AbstractEntityAIGuard.java:78`, read at
/// `:759-769`).
pub const MAX_FOLLOW_DERIVATION: i32 = 30;

/// `GuardConstants.MAX_DISTANCE_FOR_ATTACK`: a melee guard hits at two blocks
/// (`GuardConstants.java:164`, read at `MeleeCombatAI.java:355-358`).
pub const MAX_DISTANCE_FOR_ATTACK: i32 = 2;

/// `GuardConstants.BASE_PHYSICAL_DAMAGE`: what every melee weapon adds
/// (`GuardConstants.java:173`, read at `MeleeCombatAI.java:320-333`).
pub const BASE_PHYSICAL_DAMAGE: i32 = 3;

/// `GuardConstants.RANGER_BASE_DMG`: what every arrow adds
/// (`GuardConstants.java:79`, read at `RangeCombatAI.java:323`).
pub const RANGER_BASE_DMG: i32 = 2;

/// `GuardConstants.RANGED_ATTACK_DELAY_BASE`: sixty ticks between two shots
/// (`GuardConstants.java:94`, read at `RangeCombatAI.java:263`).
pub const RANGED_ATTACK_DELAY_BASE: i32 = 60;

/// `GuardConstants.KNIGHT_ATTACK_DELAY_BASE`: thirty-two ticks, less the skill
/// (`GuardConstants.java:113`, read at `MeleeCombatAI.java:364`).
pub const KNIGHT_ATTACK_DELAY_BASE: i32 = 32;

/// `GuardConstants.KNIGHT_ATTACK_DELAY_MIN`: never faster than sixteen
/// (`GuardConstants.java:118`, read at `MeleeCombatAI.java:365`).
pub const KNIGHT_ATTACK_DELAY_MIN: i32 = 16;

/// `GuardConstants.BASE_DISTANCE_FOR_RANGED_ATTACK`: ten blocks, before the hut
/// level and the skill (`GuardConstants.java:64`, read at `RangeCombatAI.java:233`).
pub const BASE_DISTANCE_FOR_RANGED_ATTACK: i32 = 10;

/// `GuardConstants.MAX_DISTANCE_FOR_RANGED_ATTACK`: twenty-four, the ceiling
/// (`GuardConstants.java:74`, read at `RangeCombatAI.java:245`).
pub const MAX_DISTANCE_FOR_RANGED_ATTACK: i32 = 24;

/// `GuardConstants.BASE_DISTANCE_FOR_POTION_ATTACK`: three blocks, before the hut
/// level (`GuardConstants.java:69`, read at `DruidCombatAI.java:187`).
pub const BASE_DISTANCE_FOR_POTION_ATTACK: i32 = 3;

/// `GuardConstants.EXP_PER_MOB_DEATH`: what one kill is worth, for every guard
/// (`GuardConstants.java:38`, granted at `MeleeCombatAI.java:436`,
/// `RangeCombatAI.java:434`, `DruidCombatAI.java:351`).
pub const EXP_PER_MOB_DEATH: f64 = 15.0;

/// `AbstractEntityAIGuard.ACTIONS_UNTIL_DUMPING`: five actions, times the hut level
/// (`AbstractEntityAIGuard.java:63`, read at `:451-454`).
pub const GUARD_ACTIONS_UNTIL_DUMPING: i32 = 5;

/// `AbstractEntityAIGuard.COMBAT_TIME`: how long a guard stays "in combat" after its
/// last blow (`AbstractEntityAIGuard.java:88`).
pub const COMBAT_TIME: i32 = 600;

/// `AbstractEntityAIGuard.GUARD_TASK_INTERVAL`: how often it re-decides
/// (`AbstractEntityAIGuard.java:118`).
pub const GUARD_TASK_INTERVAL: i32 = 100;

/// `GuardConstants.GUARD_FOLLOW_TIGHT_RANGE`: six blocks when the colony is grouped
/// tight (`GuardConstants.java:23`).
pub const GUARD_FOLLOW_TIGHT_RANGE: i32 = 6;

/// `GuardConstants.GUARD_FOLLOW_LOSE_RANGE`: fifteen otherwise
/// (`GuardConstants.java:28`).
pub const GUARD_FOLLOW_LOSE_RANGE: i32 = 15;

/// `GuardConstants.Y_VISION`: three blocks up and down, or twenty-eight on post
/// (`GuardConstants.java:33`, read at `RangeCombatAI.java:419-428`).
pub const GUARD_Y_VISION: i32 = 3;

/// `GuardConstants.HIT_CHANCE_DIVIDER`: fifteen, over `Adaptability + 1`
/// (`GuardConstants.java:99`, read at `RangeCombatAI.java:220`).
pub const HIT_CHANCE_DIVIDER: f64 = 15.0;

/// `GuardConstants.KNIGHT_HP_BONUS`: a knight's extra health
/// (`GuardConstants.java:123`, read at `JobKnight.java:65-69`).
pub const KNIGHT_HP_BONUS: i32 = 15;

/// `GuardConstants.CAVALRY_HP_BONUS` (`GuardConstants.java:128`, read at
/// `JobCavalry.java:73-77`).
pub const CAVALRY_HP_BONUS: i32 = 20;

/// `GuardConstants.DRUID_HP_BONUS` (`GuardConstants.java:159`, read at
/// `JobDruid.java:47-51`).
pub const DRUID_HP_BONUS: i32 = 12;

/// `GuardConstants.CAVALRY_RANGE_MULTIPLIER`: a lance reaches 1.2 times a sword's
/// two blocks (`GuardConstants.java:138`, read at `CavalryCombatAI.java:52-55`).
pub const CAVALRY_RANGE_MULTIPLIER: f64 = 1.20;

/// `GuardConstants.CAVALRY_DAMAGE_MULTIPLIER`: and hits for **exactly** what a
/// knight hits for — `1.00` (`GuardConstants.java:133`, read at
/// `CavalryCombatAI.java:28-33`).
pub const CAVALRY_DAMAGE_MULTIPLIER: f64 = 1.0;

/// `JobCavalry.MOUNT_DAMAGE_SPLIT`: a fifth of every blow lands on the horse
/// (`JobCavalry.java:36`, read at `:176-179`).
pub const MOUNT_DAMAGE_SPLIT: f64 = 0.20;

/// `DruidCombatAI.PER_POTION_XP`: what one thrown potion is worth
/// (`DruidCombatAI.java:66`, granted at `:175`).
pub const PER_POTION_XP: f64 = 0.05;

/// `EntityAIArcherTraining.XP_PER_SUCCESSFUL_SHOT`
/// (`EntityAIArcherTraining.java:43`, granted at `:223`).
pub const XP_PER_SUCCESSFUL_SHOT: f64 = 1.0;

/// `EntityAIArcherTraining.XP_BASE_RATE`: what a miss is worth
/// (`EntityAIArcherTraining.java:63`, granted at `:228`).
pub const XP_BASE_RATE: f64 = 0.2;

/// `EntityAICombatTraining.XP_BASE_RATE`: what one training swing is worth, at zero
/// damage (`EntityAICombatTraining.java:39`, granted at `:183`, `:288`).
pub const COMBAT_TRAINING_XP: f64 = 2.0;

/// `EntityAICombatTraining.ACTIONS_PER_BUILDING_LEVEL`: five swings per hut level
/// (`EntityAICombatTraining.java:34`, read at `:219`, `:244`).
pub const ACTIONS_PER_BUILDING_LEVEL: i32 = 5;

/// `EntityAIWorkNether`-independent helper: the two lists the druid draws from.
///
/// `DruidCombatAI.java:56` — four effects for its friends.
pub const DRUID_SUPPORT_EFFECTS: [&str; 4] =
    ["damage_boost", "saturation", "heal", "damage_resistance"];

/// `DruidCombatAI.java:61` — two effects for its enemies.
pub const DRUID_ADVERSE_EFFECTS: [&str; 2] = ["movement_slowdown", "weakness"];

/// The eight military jobs, in `Job::ALL` order of the family.
pub const RULES: &[JobRules] = &[
    // The archery trainee. It shoots a **target block** of its own building — the
    // stands are glowstone, the targets the `TARGET` blocks
    // (`BuildingArchery.java:111-138`) — with an arrow of **zero base damage**
    // (`EntityAIArcherTraining.java:174`), a hit chance of
    // `15 / (Agility / 2 + 1)` (`:182`), and it counts a shot that lands within
    // **1.41 blocks** as a success (`MIN_DISTANCE_FOR_SUCCESS = 2.0` compared
    // against a squared distance, `:221`). It earns **1 XP** per hit and **0.2** per
    // miss, and takes a bow of any level (`:248-254`).
    rules!(
        ArcherTraining,
        &[place(Spot::Post, None, "BuildingArchery.java:111")],
        [tool_at_floor(ToolKind::Bow, "EntityAIArcherTraining.java:254")],
        [],
        [
            constant(
                "RANGED_ATTACK_DELAY_BASE",
                10,
                "EntityAIArcherTraining.java:58"
            ),
            constant("CHECK_SHOT_DELAY", 60, "EntityAIArcherTraining.java:68"),
            constant(
                "MAX_DISTANCE_FOR_RANGED_ATTACK",
                MAX_DISTANCE_FOR_RANGED_ATTACK,
                "GuardConstants.java:74"
            ),
        ],
        [
            xp(
                XP_PER_SUCCESSFUL_SHOT,
                "XP_PER_SUCCESSFUL_SHOT",
                "EntityAIArcherTraining.java:223"
            ),
            xp(XP_BASE_RATE, "XP_BASE_RATE", "EntityAIArcherTraining.java:228"),
        ],
        dump(super::ACTIONS_UNTIL_DUMP, DumpScale::Fixed, "AbstractEntityAIBasic.java:425"),
        SATURATION_DEFAULT,
        "EntityAIArcherTraining.java:1"
    ),
    // The cavalry. A knight on a horse, and the numbers say so: the damage
    // multiplier is **1.00** and the only thing the mount buys is **reach** —
    // 2 × 1.2 = **2.4 blocks** (`CavalryCombatAI.java:28-33`, `:52-55`) and
    // **20 extra hit points** (`JobCavalry.java:73-77`). A fifth of every blow it
    // takes lands on the horse (`:36`, `:176-179`), the horse takes 20 % more from
    // arrows (`GuardConstants.java:143`) and the rider blocks projectiles as well as
    // explosions (`JobCavalry.java:117`). It finds a mount within **50 blocks**
    // (`EntityAICavalry.java:52-57`, `:356`), parks near its stable within ten
    // (`:289-298`), and pursues only **10 blocks** like every guard but a knight
    // (`AbstractEntityAIGuard.java:771`).
    rules!(
        Cavalry,
        &[
            place(Spot::Post, Some(MAX_GUARD_DERIVATION), "AbstractEntityAIGuard.java:771"),
            place(Spot::Stock, Some(50), "EntityAICavalry.java:356"),
        ],
        [
            super::tool(ToolKind::Spear, 0, "MeleeCombatAI.java:160"),
            material("minecraft:shield", Role::Armour, "EntityAICavalry.java:84"),
            material("#minecolonies:guard_armour", Role::Armour, "AbstractEntityAIFight.java:80"),
        ],
        [product(Output::Drops, Work::Own, "MeleeCombatAI.java:436")],
        [
            constant("GUARD_MOUNT_INTERVAL", 50, "EntityAICavalry.java:52"),
            constant("MAX_GUARD_DERIVATION", MAX_GUARD_DERIVATION, "AbstractEntityAIGuard.java:83"),
            constant("COMBAT_TIME", COMBAT_TIME, "AbstractEntityAIGuard.java:88"),
            distance("MOUNT_SEARCH_RADIUS", 50, "EntityAICavalry.java:57"),
        ],
        [
            xp(EXP_PER_MOB_DEATH, "EXP_PER_MOB_DEATH", "MeleeCombatAI.java:436"),
            xp(1.0, "addExperience(1)", "AbstractEntityAIGuard.java:343"),
        ],
        dump(
            GUARD_ACTIONS_UNTIL_DUMPING,
            DumpScale::PerBuildingLevel,
            "AbstractEntityAIGuard.java:451"
        ),
        SATURATION_HEAVY,
        "EntityAICavalry.java:1"
    ),
    // The trainee of the combat academy. It swings at a **partner or a dummy**, for
    // **zero damage** (`EntityAICombatTraining.java:213`), twenty-five percent of the
    // time with a partner (`:44`, `:95-102`), half the swings parried behind a
    // shield (`:198-208`), and it backs off **four blocks** after each
    // (`:216`). It earns **2 XP per swing** (`:39`). Its cycle is the port's oddest
    // number: `currentAttackDelay = 60` counted down **one per evaluation** of a
    // transition that fires every **60 ticks**, i.e. **3 600 ticks** — three minutes
    // — between two swings (`:49`, `:86`, `:225`). The row keeps Java's prime number
    // and the report names the consequence. Sword and **shield required**
    // (`:340`, `:345`).
    rules!(
        CombatTraining,
        &[place(Spot::Post, None, "BuildingCombatAcademy.java:122")],
        [
            tool_at_floor(ToolKind::Sword, "EntityAICombatTraining.java:345"),
            tool_at_floor(ToolKind::Shield, "EntityAICombatTraining.java:340"),
        ],
        [],
        [
            count(
                "ACTIONS_PER_BUILDING_LEVEL",
                ACTIONS_PER_BUILDING_LEVEL,
                "EntityAICombatTraining.java:34"
            ),
            constant("TRAININGS_DELAY", 60, "EntityAICombatTraining.java:49"),
            count(
                "MIN_DISTANCE_TO_TRAIN",
                5,
                "EntityAICombatTraining.java:54"
            ),
        ],
        [xp(COMBAT_TRAINING_XP, "XP_BASE_RATE", "EntityAICombatTraining.java:183")],
        dump(super::ACTIONS_UNTIL_DUMP, DumpScale::Fixed, "AbstractEntityAIBasic.java:425"),
        SATURATION_DEFAULT,
        "EntityAICombatTraining.java:1"
    ),
    // The druid. No weapon, no damage, two lists of potions: it **strengthens** with
    // `damage_boost`, `saturation`, `heal` or `damage_resistance` and **weakens** with
    // `movement_slowdown` or `weakness` (`DruidCombatAI.java:56`, `:61`); a
    // `minecolonies:magicpotion` in the hut unlocks the second, third and fourth of
    // each list and doubles the amplifier (`:145-167`). The effect lasts
    // `Mana × 20` ticks and the throw's inaccuracy is `99 / Focus` (`:137-138`). It
    // reaches `3 + hut level + ΔY` blocks (`:184-200`), throws every 40 ticks and
    // every **80** for an instant effect (`:179-182`), and earns **0.05 per
    // potion** — the smallest grant of the port (`:66`). It has **no tool at all**
    // (`EntityAIDruid.java:29-33`).
    rules!(
        Druid,
        &[place(
            Spot::Post,
            Some(BASE_DISTANCE_FOR_POTION_ATTACK + 5),
            "DruidCombatAI.java:184"
        )],
        [
            material("minecolonies:magicpotion", Role::Potion, "EntityAIDruid.java:54"),
            material("minecraft:splash_potion", Role::Potion, "DruidCombatAI.java:142"),
            material("#minecolonies:guard_armour", Role::Armour, "AbstractEntityAIFight.java:80"),
        ],
        [],
        [
            constant("MAX_GUARD_DERIVATION", MAX_GUARD_DERIVATION, "AbstractEntityAIGuard.java:83"),
            constant("COMBAT_TIME", COMBAT_TIME, "AbstractEntityAIGuard.java:88"),
            distance("GUARD_Y_VISION", GUARD_Y_VISION, "GuardConstants.java:33"),
        ],
        [
            xp(PER_POTION_XP, "PER_POTION_XP", "DruidCombatAI.java:175"),
            xp(EXP_PER_MOB_DEATH, "EXP_PER_MOB_DEATH", "DruidCombatAI.java:351"),
            xp(1.0, "addExperience(1)", "AbstractEntityAIGuard.java:343"),
        ],
        dump(
            GUARD_ACTIONS_UNTIL_DUMPING,
            DumpScale::PerBuildingLevel,
            "AbstractEntityAIGuard.java:451"
        ),
        SATURATION_HEAVY,
        "DruidCombatAI.java:1"
    ),
    // The huscarl. `JobHuscarl extends JobKnight` and declares **not one constant**
    // (`JobHuscarl.java:9-20`): it is a knight with an axe. Three differences, all in
    // the combat AI: its swing delay divides the skill by **2** instead of 3
    // (`MeleeCombatAI.java:364`), its damage is **split** between `PIERCE` and the
    // ordinary source in `(50 + Adaptability / 2) %` (`:221-226`), and it **never
    // raises its shield while attacking** (`:121`). Its leash is **10**, not the
    // knight's 30 (`AbstractEntityAIGuard.java:771`).
    rules!(
        Huscarl,
        &[place(Spot::Post, Some(MAX_GUARD_DERIVATION), "AbstractEntityAIGuard.java:771")],
        [
            tool_at_floor(ToolKind::Axe, "EntityAIMelee.java:35"),
            material("minecraft:shield", Role::Armour, "EntityAIMelee.java:44"),
            material("#minecolonies:guard_armour", Role::Armour, "AbstractEntityAIFight.java:80"),
        ],
        [product(Output::Drops, Work::Own, "MeleeCombatAI.java:436")],
        [
            constant(
                "KNIGHT_ATTACK_DELAY_BASE",
                KNIGHT_ATTACK_DELAY_BASE,
                "GuardConstants.java:113"
            ),
            constant(
                "KNIGHT_ATTACK_DELAY_MIN",
                KNIGHT_ATTACK_DELAY_MIN,
                "GuardConstants.java:118"
            ),
            constant("MAX_GUARD_DERIVATION", MAX_GUARD_DERIVATION, "AbstractEntityAIGuard.java:83"),
        ],
        [
            xp(EXP_PER_MOB_DEATH, "EXP_PER_MOB_DEATH", "MeleeCombatAI.java:436"),
            xp(1.0, "addExperience(1)", "AbstractEntityAIGuard.java:343"),
        ],
        dump(
            GUARD_ACTIONS_UNTIL_DUMPING,
            DumpScale::PerBuildingLevel,
            "AbstractEntityAIGuard.java:451"
        ),
        SATURATION_HEAVY,
        "JobHuscarl.java:1"
    ),
    // The knight. A sword, a shield it raises while attacking and against explosions
    // (`EntityAIMelee.java:44-49`, `JobKnight.java:82-99`), and the **longest leash of
    // the port**: `MAX_GUARD_DERIVATION + 20 = 30` (`AbstractEntityAIGuard.java:771`),
    // the only job of the eight with that bonus. It swings every
    // `32 - Adaptability / 3` ticks, never under 16, hits at **2 blocks** and adds
    // **3** to its sword. With the whirlwind research it sweeps a 2 × 0.5 × 2 box
    // every 240 ticks for one chance in five (`MeleeCombatAI.java:216`, `:262-272`).
    // It also has the longest list of armour of the family: all four pieces at any
    // level from leather to diamond (`AbstractEntityAIFight.java:80-84`).
    rules!(
        Knight,
        &[place(Spot::Post, Some(MAX_GUARD_DERIVATION + 20), "AbstractEntityAIGuard.java:771")],
        [
            tool_at_floor(ToolKind::Sword, "MeleeCombatAI.java:160"),
            material("minecraft:shield", Role::Armour, "EntityAIMelee.java:44"),
            material("#minecolonies:guard_armour", Role::Armour, "AbstractEntityAIFight.java:80"),
        ],
        [product(Output::Drops, Work::Own, "MeleeCombatAI.java:436")],
        [
            constant(
                "KNIGHT_ATTACK_DELAY_BASE",
                KNIGHT_ATTACK_DELAY_BASE,
                "GuardConstants.java:113"
            ),
            constant(
                "KNIGHT_ATTACK_DELAY_MIN",
                KNIGHT_ATTACK_DELAY_MIN,
                "GuardConstants.java:118"
            ),
            distance(
                "MAX_DISTANCE_FOR_ATTACK",
                MAX_DISTANCE_FOR_ATTACK,
                "GuardConstants.java:164"
            ),
            constant("COMBAT_TIME", COMBAT_TIME, "AbstractEntityAIGuard.java:88"),
            constant("GUARD_TASK_INTERVAL", GUARD_TASK_INTERVAL, "AbstractEntityAIGuard.java:118"),
        ],
        [
            xp(EXP_PER_MOB_DEATH, "EXP_PER_MOB_DEATH", "MeleeCombatAI.java:436"),
            xp(1.0, "addExperience(1)", "AbstractEntityAIGuard.java:343"),
        ],
        dump(
            GUARD_ACTIONS_UNTIL_DUMPING,
            DumpScale::PerBuildingLevel,
            "AbstractEntityAIGuard.java:451"
        ),
        SATURATION_HEAVY,
        "JobKnight.java:1"
    ),
    // The marksman. A ranger with a crossbow and one number changed: it shoots every
    // **40 ticks** instead of 60 (`60 × 0.67`, `RangeCombatAI.java:260-264`). Its
    // damage is **split** — `(50 + Adaptability / 2) %` of it travels as `PIERCE`
    // alongside the arrow (`:335-338`) — and Java multiplies that pierced part by
    // **10** (`:299-304`), a factor the report flags as a probable bug and the port
    // keeps because it is what the code does.
    rules!(
        Marksman,
        &[place(
            Spot::Post,
            Some(MAX_DISTANCE_FOR_RANGED_ATTACK),
            "GuardConstants.java:74"
        )],
        [
            tool_at_floor(ToolKind::Crossbow, "RangeCombatAI.java:117"),
            material("#minecraft:arrows", Role::Projectile, "EntityAIRange.java:54"),
            material("#minecolonies:guard_armour", Role::Armour, "AbstractEntityAIFight.java:80"),
        ],
        [product(Output::Drops, Work::Own, "RangeCombatAI.java:434")],
        [
            constant(
                "RANGED_ATTACK_DELAY_BASE",
                RANGED_ATTACK_DELAY_BASE,
                "GuardConstants.java:94"
            ),
            constant("MAX_GUARD_DERIVATION", MAX_GUARD_DERIVATION, "AbstractEntityAIGuard.java:83"),
            distance(
                "MAX_DISTANCE_FOR_RANGED_ATTACK",
                MAX_DISTANCE_FOR_RANGED_ATTACK,
                "GuardConstants.java:74"
            ),
        ],
        [
            xp(EXP_PER_MOB_DEATH, "EXP_PER_MOB_DEATH", "RangeCombatAI.java:434"),
            xp(1.0, "addExperience(1)", "AbstractEntityAIGuard.java:343"),
        ],
        dump(
            GUARD_ACTIONS_UNTIL_DUMPING,
            DumpScale::PerBuildingLevel,
            "AbstractEntityAIGuard.java:451"
        ),
        SATURATION_HEAVY,
        "JobMarksman.java:1"
    ),
    // The ranger — "Archer" in the lang file. A bow, arrows it takes **64** at a time
    // and asks for **16** when the hut drops below 16 (`EntityAIRange.java:50-61`),
    // and a reach of `10 + hut level + (Adaptability / 50) × 15`, **capped at 24**,
    // plus **10** when it is on its own post (`RangeCombatAI.java:230-258`). Its
    // damage is `2 + Agility / 5 + enchantment + Power + research`, doubled under
    // 20 % health, times 1.5 on a critical (`:272-324`). It flees when a target is
    // inside `√7` blocks, one time in three (`:165-172`).
    rules!(
        Ranger,
        &[place(
            Spot::Post,
            Some(MAX_DISTANCE_FOR_RANGED_ATTACK),
            "GuardConstants.java:74"
        )],
        [
            tool_at_floor(ToolKind::Bow, "RangeCombatAI.java:117"),
            material("#minecraft:arrows", Role::Projectile, "EntityAIRange.java:54"),
            material("#minecolonies:guard_armour", Role::Armour, "AbstractEntityAIFight.java:80"),
        ],
        [product(Output::Drops, Work::Own, "RangeCombatAI.java:434")],
        [
            constant(
                "RANGED_ATTACK_DELAY_BASE",
                RANGED_ATTACK_DELAY_BASE,
                "GuardConstants.java:94"
            ),
            constant("BOW_HOLDING_DELAY", 40, "RangeCombatAI.java:79"),
            distance(
                "BASE_DISTANCE_FOR_RANGED_ATTACK",
                BASE_DISTANCE_FOR_RANGED_ATTACK,
                "GuardConstants.java:64"
            ),
            distance(
                "MAX_DISTANCE_FOR_RANGED_ATTACK",
                MAX_DISTANCE_FOR_RANGED_ATTACK,
                "GuardConstants.java:74"
            ),
            distance(
                "GUARD_FOLLOW_TIGHT_RANGE",
                GUARD_FOLLOW_TIGHT_RANGE,
                "GuardConstants.java:23"
            ),
            distance(
                "GUARD_FOLLOW_LOSE_RANGE",
                GUARD_FOLLOW_LOSE_RANGE,
                "GuardConstants.java:28"
            ),
        ],
        [
            xp(EXP_PER_MOB_DEATH, "EXP_PER_MOB_DEATH", "RangeCombatAI.java:434"),
            xp(1.0, "addExperience(1)", "AbstractEntityAIGuard.java:343"),
        ],
        dump(
            GUARD_ACTIONS_UNTIL_DUMPING,
            DumpScale::PerBuildingLevel,
            "AbstractEntityAIGuard.java:451"
        ),
        SATURATION_HEAVY,
        "JobRanger.java:1"
    ),
];

/// The number of ticks between two blows of a melee guard, and its two variants —
/// `MeleeCombatAI.java:361-366`.
///
/// ```java
/// final int delay = isHuscarl()
///     ? KNIGHT_ATTACK_DELAY_BASE - getPrimarySkillLevel() / 2
///     : KNIGHT_ATTACK_DELAY_BASE - getPrimarySkillLevel() / 3;
/// return Math.max(KNIGHT_ATTACK_DELAY_MIN, delay);
/// ```
pub fn melee_attack_delay(primary_level: i32, huscarl: bool) -> i32 {
    let divisor = if huscarl { 2 } else { 3 };
    let delay = KNIGHT_ATTACK_DELAY_BASE - primary_level / divisor;
    delay.max(KNIGHT_ATTACK_DELAY_MIN)
}

/// The number of ticks between two shots — `RangeCombatAI.java:260-264`.
///
/// A marksman multiplies by `0.67` and the product is truncated, so
/// `60 × 0.67 = 40` ticks, and a ranger pays the base 60.
pub fn ranged_attack_delay(marksman: bool) -> i32 {
    if marksman {
        (f64::from(RANGED_ATTACK_DELAY_BASE) * 0.67) as i32
    } else {
        RANGED_ATTACK_DELAY_BASE
    }
}

/// The reach of a ranged guard, in blocks — `RangeCombatAI.java:230-258`.
///
/// `10 + hut level + (Adaptability / 50) × 15`, capped at 24, plus 10 when the guard
/// is on its own post, plus the height difference. The height term is the caller's:
/// a guard above its target reaches further and one below reaches less, unbounded,
/// which is Java's behaviour and not a rounding of the port.
pub fn ranged_attack_distance(
    hut_level: i32,
    adaptability: i32,
    height_difference: i32,
    on_post: bool,
) -> i32 {
    let base = f64::from(BASE_DISTANCE_FOR_RANGED_ATTACK)
        + f64::from(hut_level)
        + (f64::from(adaptability) / 50.0) * 15.0;
    let capped = (base as i32).min(MAX_DISTANCE_FOR_RANGED_ATTACK);
    let post = if on_post { 10 } else { 0 };
    capped + height_difference + post
}

/// The damage of one melee blow before the weapon's own — `MeleeCombatAI.java:310-352`.
///
/// The base is [`BASE_PHYSICAL_DAMAGE`]; the enchantment adds half of its bonus; a
/// guard under a fifth of its health hits **twice as hard**; and a critical
/// multiplies by `1.5`. `weapon_damage` is the item's own, which the engine's item
/// registry owns and the port does not copy.
pub fn melee_damage(weapon_damage: f64, enchantment_bonus: f64, health_fraction: f64, critical: bool) -> f64 {
    let mut damage = weapon_damage + f64::from(BASE_PHYSICAL_DAMAGE);
    damage += enchantment_bonus / 2.5;
    if health_fraction <= 0.2 {
        damage *= 2.0;
    }
    if critical {
        damage *= 1.5;
    }
    damage
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(job: Job) -> &'static JobRules {
        super::super::rules_for(job).expect("a guard has rules")
    }

    #[test]
    fn eight_military_jobs_and_no_duplicate() {
        assert_eq!(RULES.len(), 8);
        let mut seen = Vec::new();
        for row in RULES {
            assert!(!seen.contains(&row.job), "{} twice", row.job.path());
            seen.push(row.job);
            assert_eq!(row.family, Family::Military);
        }
        for job in [
            Job::ArcherTraining,
            Job::Cavalry,
            Job::CombatTraining,
            Job::Druid,
            Job::Huscarl,
            Job::Knight,
            Job::Marksman,
            Job::Ranger,
        ] {
            assert!(seen.contains(&job), "{} is missing", job.path());
        }
    }

    #[test]
    fn the_six_guards_eat_heavily_and_the_two_trainees_do_not() {
        for job in [
            Job::Knight,
            Job::Huscarl,
            Job::Ranger,
            Job::Marksman,
            Job::Cavalry,
            Job::Druid,
        ] {
            assert_eq!(rules(job).saturation, SATURATION_HEAVY, "{}", job.path());
        }
        for job in [Job::ArcherTraining, Job::CombatTraining] {
            assert_eq!(rules(job).saturation, SATURATION_DEFAULT, "{}", job.path());
        }
    }

    #[test]
    fn the_knight_alone_has_the_thirty_block_leash_and_the_huscarl_has_ten() {
        assert_eq!(rules(Job::Knight).places[0].reach, Some(30));
        for job in [Job::Huscarl, Job::Cavalry] {
            assert_eq!(
                rules(job).places[0].reach,
                Some(MAX_GUARD_DERIVATION),
                "{}",
                job.path()
            );
        }
        assert_eq!(MAX_GUARD_DERIVATION + 20, 30);
        assert_eq!(MAX_PATROL_DERIVATION, 80);
        assert_eq!(MAX_FOLLOW_DERIVATION, 30);
        // the two ranged guards state a reach of 24, the ceiling
        for job in [Job::Ranger, Job::Marksman] {
            assert_eq!(rules(job).places[0].reach, Some(MAX_DISTANCE_FOR_RANGED_ATTACK));
        }
        // the druid's throw reaches `3 + hut level`, so eight at level five
        assert_eq!(rules(Job::Druid).places[0].reach, Some(8), "3 + hut level 5");
    }

    #[test]
    fn the_huscarl_swings_faster_than_the_knight_and_both_stop_at_sixteen() {
        assert_eq!(melee_attack_delay(0, false), 32);
        assert_eq!(melee_attack_delay(0, true), 32);
        assert_eq!(melee_attack_delay(30, false), 22);
        assert_eq!(melee_attack_delay(30, true), 17);
        assert_eq!(melee_attack_delay(99, false), 16);
        assert_eq!(melee_attack_delay(99, true), 16);
        for level in 0..=120 {
            assert!(melee_attack_delay(level, false) >= KNIGHT_ATTACK_DELAY_MIN);
            assert!(melee_attack_delay(level, true) >= KNIGHT_ATTACK_DELAY_MIN);
            assert!(melee_attack_delay(level, true) <= melee_attack_delay(level, false));
        }
    }

    #[test]
    fn a_marksman_shoots_in_forty_ticks_and_a_ranger_in_sixty() {
        assert_eq!(ranged_attack_delay(false), 60);
        assert_eq!(ranged_attack_delay(true), 40, "60 * 0.67 truncated");
        assert_eq!(RANGED_ATTACK_DELAY_BASE, 60);
    }

    #[test]
    fn the_ranger_s_reach_is_ten_plus_the_hut_and_is_capped_at_twenty_four() {
        assert_eq!(ranged_attack_distance(0, 0, 0, false), 10);
        assert_eq!(ranged_attack_distance(5, 0, 0, false), 15);
        assert_eq!(ranged_attack_distance(0, 50, 0, false), 24, "26 capped");
        assert_eq!(ranged_attack_distance(0, 99, 0, false), MAX_DISTANCE_FOR_RANGED_ATTACK);
        assert_eq!(ranged_attack_distance(0, 0, 0, true), 20, "+ GUARD_BONUS_RANGE");
        assert_eq!(ranged_attack_distance(0, 0, -2, false), 8, "the height term is unbounded");
        assert_eq!(BASE_DISTANCE_FOR_RANGED_ATTACK, 10);
    }

    #[test]
    fn a_melee_blow_adds_three_and_doubles_under_a_fifth_of_health() {
        assert!((melee_damage(3.0, 0.0, 1.0, false) - 6.0).abs() < 1e-9, "iron sword");
        assert!((melee_damage(9.0, 0.0, 1.0, false) - 12.0).abs() < 1e-9, "iron axe");
        assert!((melee_damage(3.0, 0.0, 0.2, false) - 12.0).abs() < 1e-9);
        assert!((melee_damage(3.0, 0.0, 0.21, false) - 6.0).abs() < 1e-9);
        assert!((melee_damage(3.0, 0.0, 1.0, true) - 9.0).abs() < 1e-9);
        assert!((melee_damage(3.0, 2.5, 1.0, false) - 7.0).abs() < 1e-9, "enchant over 2.5");
        assert_eq!(BASE_PHYSICAL_DAMAGE, 3);
        assert_eq!(RANGER_BASE_DMG, 2);
    }

    #[test]
    fn the_cavalry_buys_reach_and_health_and_not_damage() {
        assert_eq!(CAVALRY_DAMAGE_MULTIPLIER, 1.0);
        assert_eq!(CAVALRY_RANGE_MULTIPLIER, 1.20);
        assert_eq!(
            (f64::from(MAX_DISTANCE_FOR_ATTACK) * CAVALRY_RANGE_MULTIPLIER) as i32,
            2,
            "2 * 1.2 truncates to 2, and the port keeps the truncation"
        );
        assert_eq!(CAVALRY_HP_BONUS, 20);
        assert_eq!(KNIGHT_HP_BONUS, 15);
        assert!((MOUNT_DAMAGE_SPLIT - 0.2).abs() < 1e-9);
        assert_eq!(rules(Job::Cavalry).places[1].spot, Spot::Stock);
        assert_eq!(rules(Job::Cavalry).places[1].reach, Some(50), "the mount search");
    }

    #[test]
    fn the_druid_has_no_weapon_and_six_effects_in_two_lists() {
        let druid = rules(Job::Druid);
        assert!(
            !druid
                .accepts
                .iter()
                .any(|accepted| matches!(accepted.role, Role::Tool(_))),
            "EntityAIDruid.java:29-33 adds no tool"
        );
        assert!(druid.job.tools().is_empty());
        assert_eq!(
            DRUID_SUPPORT_EFFECTS,
            ["damage_boost", "saturation", "heal", "damage_resistance"]
        );
        assert_eq!(DRUID_ADVERSE_EFFECTS, ["movement_slowdown", "weakness"]);
        assert_eq!(DRUID_SUPPORT_EFFECTS.len() + DRUID_ADVERSE_EFFECTS.len(), 6);
        assert!(
            druid
                .accepts
                .iter()
                .any(|accepted| accepted.item == "minecolonies:magicpotion")
        );
        // it is the only job of the port that earns less than a tenth per action
        assert!(PER_POTION_XP < 0.1);
        assert_eq!(druid.xp[0].per_action, PER_POTION_XP);
        assert_eq!(DRUID_HP_BONUS, 12);
    }

    #[test]
    fn the_two_trainees_pay_a_trainee_and_not_the_colony() {
        assert_eq!(rules(Job::ArcherTraining).xp[0].per_action, XP_PER_SUCCESSFUL_SHOT);
        assert_eq!(rules(Job::ArcherTraining).xp[1].per_action, XP_BASE_RATE);
        assert_eq!(rules(Job::CombatTraining).xp[0].per_action, COMBAT_TRAINING_XP);
        // both produce nothing: the target block is not consumed and the partner does
        // not take damage (`EntityAICombatTraining.java:213`)
        assert!(rules(Job::ArcherTraining).produces.is_empty());
        assert!(rules(Job::CombatTraining).produces.is_empty());
        assert_eq!(COMBAT_TRAINING_XP / XP_BASE_RATE, 10.0);
    }

    #[test]
    fn the_archery_trainee_s_ten_is_not_the_library_s_sixty() {
        // the shadowed constant: 10 here, 60 in GuardConstants, same name
        let trainee = rules(Job::ArcherTraining);
        let shadowed = trainee
            .durations
            .iter()
            .find(|constant| constant.name == "RANGED_ATTACK_DELAY_BASE")
            .expect("the trainee's own delay");
        assert_eq!(shadowed.value, 10);
        assert_eq!(shadowed.source, "EntityAIArcherTraining.java:58");
        assert_eq!(RANGED_ATTACK_DELAY_BASE, 60);
        assert_ne!(shadowed.value, RANGED_ATTACK_DELAY_BASE);
        // and its shot check runs on sixty ticks, its own
        assert_eq!(rules(Job::ArcherTraining).durations[1].value, 60);
        assert_eq!(rules(Job::ArcherTraining).durations[1].name, "CHECK_SHOT_DELAY");
    }

    #[test]
    fn every_guard_dumps_five_actions_per_hut_level_and_both_trainees_the_base() {
        for job in [
            Job::Knight,
            Job::Huscarl,
            Job::Ranger,
            Job::Marksman,
            Job::Cavalry,
            Job::Druid,
        ] {
            assert_eq!(rules(job).dump.at(1), Some(GUARD_ACTIONS_UNTIL_DUMPING), "{}", job.path());
            assert_eq!(rules(job).dump.at(4), Some(20), "{}", job.path());
        }
        for job in [Job::ArcherTraining, Job::CombatTraining] {
            assert_eq!(
                rules(job).dump.at(4),
                Some(super::super::ACTIONS_UNTIL_DUMP),
                "{}",
                job.path()
            );
        }
    }

    #[test]
    fn every_guard_earns_fifteen_a_kill_and_one_for_waking_up() {
        for job in [
            Job::Knight,
            Job::Huscarl,
            Job::Ranger,
            Job::Marksman,
            Job::Cavalry,
            Job::Druid,
        ] {
            let row = rules(job);
            let kill = row
                .xp
                .iter()
                .find(|grant| grant.name == "EXP_PER_MOB_DEATH")
                .unwrap_or_else(|| panic!("{} has no kill grant", job.path()));
            assert_eq!(kill.per_action, EXP_PER_MOB_DEATH, "{}", job.path());
            // every guard also earns one XP for waking up (`AbstractEntityAIGuard.java:343`)
            let wake = row
                .xp
                .iter()
                .find(|grant| grant.source == "AbstractEntityAIGuard.java:343")
                .unwrap_or_else(|| panic!("{} has no wake grant", job.path()));
            assert_eq!(wake.per_action, 1.0, "{}", job.path());
        }
        // the druid is the one guard whose first listed grant is not the kill
        assert_eq!(rules(Job::Druid).xp[0].name, "PER_POTION_XP");
        assert_eq!(rules(Job::Druid).xp[0].per_action, PER_POTION_XP);
    }
}
