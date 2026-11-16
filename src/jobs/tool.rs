//! The tools a job asks for, and the level of tool a hut allows.
//!
//! Java reference: `api/equipment/ModEquipmentTypes.java` (the equipment
//! registry), `api/util/constant/EquipmentLevelConstants.java` (the level
//! scale), `api/compatibility/Compatibility.java` (the quality a level names),
//! and the two request sites in `core/entity/ai/workers/AbstractEntityAIBasic`.
//!
//! **Two numbers, and they are not the same one.** A tool request carries a
//! *minimum* level, which the job states (`0` for almost every tool:
//! `AbstractEntityAIBasic.java:968`), and a *maximum*, which the hut states:
//!
//! ```java
//! new Tool(toolType, minimalLevel, building.getMaxEquipmentLevel() < minimalLevel
//!     ? minimalLevel : building.getMaxEquipmentLevel());   // :995-999
//! ```
//!
//! [`request`] is that expression and [`max_equipment_level`] is the hut half
//! (`api/colony/buildings/IBuilding.java:447-458`). Those two functions are the
//! whole rule; everything else here is the closed list they read.
//!
//! **What a level *means*.** Not a number in the air: `LEVEL_TO_TIER` maps
//! 0..=5 to wood, stone, iron, diamond, netherite, netherite
//! (`api/compatibility/Compatibility.java:46-49`), and the tool level of a stack
//! is that tier's number, `-1` when the item is not a tiered tool at all
//! (`:131-135`).
//!
//! **What is not here.** No item, no stack, no request token. Which stack *is* a
//! pickaxe is the engine's `ToolActions` question
//! (`api/equipment/ModEquipmentTypes.java:60-66`); the four armour slots of the
//! same registry (`:128-151`) are gear a guard asks for through its building's
//! `itemsNeeded` (`core/entity/ai/workers/guard/AbstractEntityAIFight.java:150-160`),
//! not a tool a job declares, so they are not a [`ToolKind`].

/// `TOOL_LEVEL_HAND`: no tool at all, what a bare hand is worth
/// (`api/util/constant/EquipmentLevelConstants.java:14`).
pub const TOOL_LEVEL_HAND: i32 = -1;

/// `TOOL_LEVEL_WOOD_OR_GOLD`: the level of a wooden or golden tool, and the
/// floor every job-`stated` request uses
/// (`api/util/constant/EquipmentLevelConstants.java:20`).
pub const TOOL_LEVEL_WOOD_OR_GOLD: i32 = 0;

/// `BASIC_TOOL_LEVEL`: stone, and what a level-0 hut accepts
/// (`api/util/constant/EquipmentLevelConstants.java:26`).
pub const BASIC_TOOL_LEVEL: i32 = 1;

/// `TOOL_LEVEL_MAXIMUM`: a hut at its top level accepts anything
/// (`api/util/constant/EquipmentLevelConstants.java:68`).
pub const TOOL_LEVEL_MAXIMUM: i32 = i32::MAX;

/// `WOOD_HUT_LEVEL`: the level a hut has before its first build completes
/// (`api/colony/buildings/IBuilding.java:48`, `:453`).
pub const WOOD_HUT_LEVEL: i32 = 0;

/// One entry of the `minecolonies:equipmenttypes` registry.
///
/// The **14** here are the registry's non-armour entries, in Java's registration
/// order (`api/equipment/ModEquipmentTypes.java:54-168`). The four armour slots
/// are deliberately absent — see the module note.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: the registry is a closed list in Java, and a new tool is a change
// to this enum
pub enum ToolKind {
    /// `none`: "any stack serves", and what a job that needs no tool declares
    /// (`ModEquipmentTypes.java:54-58`).
    None,
    Pickaxe,
    Shovel,
    Axe,
    Hoe,
    Sword,
    Bow,
    Crossbow,
    /// The fishing rod, whose registry path is `rod`, not `fishing_rod`
    /// (`ModEquipmentTypes.java:110`).
    Rod,
    Shears,
    Shield,
    /// `flintandsteel`, one word (`ModEquipmentTypes.java:152`).
    FlintAndSteel,
    Lead,
    Spear,
}

impl ToolKind {
    /// Every kind, in Java's registration order.
    pub const ALL: [ToolKind; 14] = [
        ToolKind::None,
        ToolKind::Pickaxe,
        ToolKind::Shovel,
        ToolKind::Axe,
        ToolKind::Hoe,
        ToolKind::Sword,
        ToolKind::Bow,
        ToolKind::Crossbow,
        ToolKind::Rod,
        ToolKind::Shears,
        ToolKind::Shield,
        ToolKind::FlintAndSteel,
        ToolKind::Lead,
        ToolKind::Spear,
    ];

    /// The registry path, without its namespace — what
    /// `new ResourceLocation(Constants.MOD_ID, id)` builds
    /// (`ModEquipmentTypes.java:186-188`).
    pub fn name(self) -> &'static str {
        match self {
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

    /// The kind a bare registry path names, or `None`.
    pub fn from_name(name: &str) -> Option<ToolKind> {
        ToolKind::ALL.into_iter().find(|kind| kind.name() == name)
    }

    /// The **minimum** level a request for this kind states.
    ///
    /// `0` for everything a job asks for with
    /// `checkForToolOrWeapon(type)` (`AbstractEntityAIBasic.java:966-968`,
    /// `:980`), with two exceptions that Java states by hand:
    ///
    /// - a **lead** is level-less — the stablemaster overrides the check with
    ///   `-1` because "the Lead is level-less"
    ///   (`core/entity/ai/workers/production/herders/EntityAIWorkStablemaster.java:151-160`) ;
    /// - a **spear** is level-less — the guard's gear match reads
    ///   `stack.getItem() instanceof ItemSpear ? -1 : 0`
    ///   (`core/entity/ai/workers/guard/AbstractEntityAIFight.java:126-130`).
    ///
    /// [`ToolKind::None`] answers [`TOOL_LEVEL_HAND`] like the registry entry it
    /// mirrors (`ModEquipmentTypes.java:56`), and a caller that got it should
    /// not request anything at all.
    pub fn min_level(self) -> i32 {
        match self {
            ToolKind::Lead | ToolKind::Spear | ToolKind::None => TOOL_LEVEL_HAND,
            _ => TOOL_LEVEL_WOOD_OR_GOLD,
        }
    }

    /// Whether the engine counts this stack as equipment for this kind at
    /// `[min, max]`.
    ///
    /// Java's answer is `ItemStackUtils.hasEquipmentLevel(stack, type, min, max)`
    /// (`api/colony/requestsystem/requestable/Tool.java:172-188`): the lower and
    /// upper bounds are **inclusive**, and a hand-level (`-1`) item fails a `0`
    /// floor. The port has no stack, so this is the comparison alone — what a
    /// later hook will call once it has one.
    pub fn accepts(self, level: i32, min: i32, max: i32) -> bool {
        level >= min && level <= max
    }
}

/// One tool request: what a hut's chest should hand a worker.
///
/// The shape of `api/colony/requestsystem/requestable/Tool.java:37-60`, without
/// the item it may carry (`Result`, `:46-48`), which belongs to the request
/// system and not to a rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolRequest {
    pub kind: ToolKind,
    pub min: i32,
    pub max: i32,
}

impl ToolRequest {
    /// Whether a stack of this level satisfies the request, bounds included.
    pub fn matches(&self, level: i32) -> bool {
        self.kind.accepts(level, self.min, self.max)
    }
}

/// The highest tool level a hut of `building_level` accepts
/// (`api/colony/buildings/IBuilding.java:447-458`).
///
/// Three branches, in Java's order:
///
/// 1. at or above the kind's own maximum level → [`TOOL_LEVEL_MAXIMUM`] (the hut
///    has nothing left to gain from a better tool);
/// 2. at or below [`WOOD_HUT_LEVEL`] → [`BASIC_TOOL_LEVEL`], so a hut that is
///    placed but not built still accepts stone;
/// 3. otherwise the level itself, since `WOOD_HUT_LEVEL` is `0`.
///
/// `max_building_level` is the kind's own ceiling, five for every hut in the
/// data (`crate::building::MAX_BUILDING_LEVEL`), three for a gatehouse
/// (`core/colony/buildings/workerbuildings/BuildingGateHouse.java:34`) — which is
/// why this is an argument and not the constant.
pub fn max_equipment_level(building_level: i32, max_building_level: i32) -> i32 {
    if building_level >= max_building_level {
        TOOL_LEVEL_MAXIMUM
    } else if building_level <= WOOD_HUT_LEVEL {
        BASIC_TOOL_LEVEL
    } else {
        building_level - WOOD_HUT_LEVEL
    }
}

/// The request a worker at `building_level` makes for `kind` — the single
/// expression at `AbstractEntityAIBasic.java:995-999`.
///
/// The floor is the kind's own ([`ToolKind::min_level`]). The ceiling is the
/// hut's, **raised to the floor when the hut is too low for the tool asked for**:
/// Java writes `getMaxEquipmentLevel() < minimalLevel ? minimalLevel : ...`, so a
/// builder who needs an iron pickaxe in a level-1 hut asks for `[2, 2]` rather
/// than nothing, and stays stuck until the hut grows — which is exactly the
/// behaviour `REQUEST_SYSTEM_BUILDING_LEVEL_TOO_LOW`
/// (`AbstractEntityAIBasic.java:1365-1376`) tells the player about.
pub fn request(kind: ToolKind, building_level: i32, max_building_level: i32) -> ToolRequest {
    let min = kind.min_level();
    let hut_max = max_equipment_level(building_level, max_building_level);
    ToolRequest {
        kind,
        min,
        max: hut_max.max(min),
    }
}

/// The quality a tool level names.
///
/// `Compatibility.LEVEL_TO_TIER` has **six** slots for 0..=5
/// (`api/compatibility/Compatibility.java:46-49`) and the last two are both
/// netherite: level 5 is only reachable through `durabilityBasedLevel`, which
/// clamps `maxDamage / vanillaDurability` to `5` (`ModEquipmentTypes.java:107`,
/// `:196-206`). `None` below `0`: a hand is not a quality.
pub fn quality(level: i32) -> Option<&'static str> {
    const NAMES: [&str; 6] = ["wood", "stone", "iron", "diamond", "netherite", "netherite"];
    if level < 0 {
        return None;
    }
    NAMES.get(level as usize).copied()
}

#[cfg(test)]
mod tests {
    use crate::building::MAX_BUILDING_LEVEL;

    use super::*;

    #[test]
    fn the_registry_holds_fourteen_kinds_and_every_name_is_unique() {
        assert_eq!(ToolKind::ALL.len(), 14);
        for (index, kind) in ToolKind::ALL.iter().enumerate() {
            for other in &ToolKind::ALL[index + 1..] {
                assert_ne!(kind.name(), other.name(), "two kinds share a registry path");
            }
            assert_eq!(ToolKind::from_name(kind.name()), Some(*kind));
        }
        // Two paths that a guess would get wrong.
        assert_eq!(ToolKind::Rod.name(), "rod", "not fishing_rod");
        assert_eq!(ToolKind::FlintAndSteel.name(), "flintandsteel", "one word");
        assert_eq!(ToolKind::from_name("fishing_rod"), None);
        assert_eq!(
            ToolKind::from_name("helmet"),
            None,
            "armour is not a tool here"
        );
    }

    #[test]
    fn a_hut_accepts_a_better_tool_as_it_grows_and_anything_at_the_top() {
        // 0 is a placed hut, 1 stone, 2 iron, 4 netherite.
        assert_eq!(max_equipment_level(0, MAX_BUILDING_LEVEL), BASIC_TOOL_LEVEL);
        assert_eq!(max_equipment_level(1, MAX_BUILDING_LEVEL), 1);
        assert_eq!(max_equipment_level(2, MAX_BUILDING_LEVEL), 2);
        assert_eq!(max_equipment_level(4, MAX_BUILDING_LEVEL), 4);
        assert_eq!(
            max_equipment_level(5, MAX_BUILDING_LEVEL),
            TOOL_LEVEL_MAXIMUM,
            "a top hut accepts netherite, and anything better a mod adds"
        );
        assert_eq!(
            max_equipment_level(9, MAX_BUILDING_LEVEL),
            TOOL_LEVEL_MAXIMUM
        );
        // A gatehouse stops at three, so three is already its top.
        assert_eq!(max_equipment_level(3, 3), TOOL_LEVEL_MAXIMUM);
        assert_eq!(max_equipment_level(2, 3), 2);
    }

    #[test]
    fn the_request_floor_is_the_tools_own_and_the_ceiling_is_the_huts() {
        let hoe = request(ToolKind::Hoe, 2, MAX_BUILDING_LEVEL);
        assert_eq!(
            hoe,
            ToolRequest {
                kind: ToolKind::Hoe,
                min: 0,
                max: 2
            }
        );
        assert!(hoe.matches(0), "a wooden hoe is enough");
        assert!(hoe.matches(2));
        assert!(!hoe.matches(3), "diamond is above what the hut accepts");

        // The two level-less tools, and the registry's `none`.
        assert_eq!(
            request(ToolKind::Lead, 1, MAX_BUILDING_LEVEL).min,
            TOOL_LEVEL_HAND
        );
        assert_eq!(
            request(ToolKind::Spear, 1, MAX_BUILDING_LEVEL).min,
            TOOL_LEVEL_HAND
        );
        assert_eq!(
            request(ToolKind::None, 1, MAX_BUILDING_LEVEL).min,
            TOOL_LEVEL_HAND
        );
    }

    #[test]
    fn a_tool_above_the_hut_raises_the_ceiling_to_its_floor() {
        // A level-1 hut that needs an iron pickaxe asks for [2, 2]: Java's
        // `getMaxEquipmentLevel() < minimalLevel ? minimalLevel : ...`.
        let pick = request(ToolKind::Pickaxe, 1, MAX_BUILDING_LEVEL);
        assert_eq!(
            pick.min, TOOL_LEVEL_WOOD_OR_GOLD,
            "the job states 0 by default"
        );
        assert_eq!(pick.max, 1, "the hut states 1");

        // The harvest-level floor is stated per block, so it is a caller's
        // argument and not something this module can invent.
        let forced = ToolRequest {
            kind: ToolKind::Pickaxe,
            min: 2,
            max: 2,
        };
        assert!(forced.matches(2) && !forced.matches(1));
        assert_eq!(
            request(ToolKind::Lead, 0, MAX_BUILDING_LEVEL),
            ToolRequest {
                kind: ToolKind::Lead,
                min: -1,
                max: 1
            },
            "the floor wins over a hut ceiling of 1 only when it is higher"
        );
    }

    #[test]
    fn a_level_names_a_tier_up_to_six_slots_and_hand_names_none() {
        assert_eq!(quality(-1), None, "a hand is not a quality");
        assert_eq!(quality(0), Some("wood"));
        assert_eq!(quality(1), Some("stone"));
        assert_eq!(quality(2), Some("iron"));
        assert_eq!(quality(3), Some("diamond"));
        assert_eq!(quality(4), Some("netherite"));
        assert_eq!(
            quality(5),
            Some("netherite"),
            "the sixth slot repeats the fifth"
        );
        assert_eq!(quality(6), None, "and there is no seventh");
    }
}
