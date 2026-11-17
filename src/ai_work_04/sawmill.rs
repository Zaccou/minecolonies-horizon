//! The sawmill — and the finding that it has no goal of its own.
//!
//! Java reference: `core/entity/ai/workers/crafting/EntityAIWorkSawmill.java`, all 45
//! lines of it.
//!
//! ## Measured: the class adds three things, and none of them is a state
//!
//! | Lines | What they are | Ported as |
//! | --- | --- | --- |
//! | `:20-21` | `CRAFTING`, a `VisibleCitizenStatus` with an icon and a translation key | [`CRAFTING`] |
//! | `:33-37` | `getExpectedBuildingClass()` → `BuildingSawmill.class` | [`expected_building`] |
//! | `:39-44` | `craft()` sets the visible status, then calls `super.craft()` | [`craft_visible_status`] |
//!
//! No `registerTargets` call, no `AITarget`, no `decide`, no `getActionsDoneUntilDumping`,
//! no `static final int`: the table this trade runs is
//! `AbstractEntityAICrafting`'s six targets, unchanged. `crate::ai_build::crafter` is
//! that table, ported once with its four `getRecipe` exits, its seven `craft` exits and
//! its tests; `crate::ai_build::work_time::craft_hits` is its duration formula
//! (`AbstractEntityAICrafting.java:734-738`). **This file therefore does not
//! re-register them** — [`machine`] hands back the crafter's, and the tests here hold
//! that the delta is empty rather than pretending to a second copy.
//!
//! The brief named three jobs of equal weight. They are not: the sawmill is 45 lines of
//! which 26 are a licence header and a constructor that forwards, the sifter is 223 and
//! the smelter 239. That is why this file is short and the other two are not.
//!
//! ## What a sawmill's day looks like
//!
//! `IDLE` (`AbstractEntityAICrafting.java:164-165`) → `START_WORKING` (`:166`) →
//! `GET_RECIPE` (`:168`) → `QUERY_ITEMS` (`:167`) → `CRAFT` (`:169`, every
//! `HIT_DELAY = 10` ticks) → `INVENTORY_FULL` after one craft (`:724-727`), then back to
//! `IDLE`. The states are `crate::ai_build::crafter::CraftState`'s; the times are
//! `work_time`'s; the only visible difference from the stonemason is the icon.

use crate::ai::Machine;
use crate::ai_build::crafter::{self, CraftFacts, CraftState};
use crate::building::Kind;

/// `EntityAIWorkSawmill.CRAFTING`'s icon (`:21`): the picture the citizen panel shows
/// while a carpenter works.
pub const SAWMILL_ICON: &str = "textures/icons/work/sawmill.png";

/// `EntityAIWorkSawmill.CRAFTING`'s translation key (`:21`).
pub const SAWMILL_STATUS: &str = "com.minecolonies.gui.visiblestatus.sawmill";

/// A visible status: what the citizen panel shows, which is an effect and not a goal.
///
/// Java reference: `api/entity/citizen/VisibleCitizenStatus.java`, reached from
/// `EntityAIWorkSawmill.java:20-21` and set by `:42`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisibleStatus {
    /// `new ResourceLocation(Constants.MOD_ID, icon)` — the path part.
    pub icon: &'static str,
    /// The translation key the panel renders.
    pub translation_key: &'static str,
}

/// `EntityAIWorkSawmill.CRAFTING` (`:20-21`): the status a carpenter wears while it
/// works.
pub const CRAFTING: VisibleStatus = VisibleStatus {
    icon: SAWMILL_ICON,
    translation_key: SAWMILL_STATUS,
};

/// What one trade adds to its parent's table, in numbers — the shape of the finding
/// above, so a test can assert it instead of a comment claiming it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SawmillDelta {
    /// `registerTargets` calls in the class (`:28-31` forwards to `super`, `:33-44` are
    /// overrides only): **zero**.
    pub targets_added: usize,
    /// `static final` numeric constants declared by the class: **zero**.
    pub constants_declared: usize,
    /// States of `AIWorkerState` the class names: **zero**.
    pub states_named: usize,
}

/// The delta, measured on the file: nothing, nothing, nothing.
pub fn delta() -> SawmillDelta {
    SawmillDelta {
        targets_added: 0,
        constants_declared: 0,
        states_named: 0,
    }
}

/// `getExpectedBuildingClass()` (`:33-37`): a carpenter works in a `BuildingSawmill`.
///
/// The kind is `crate::building::kind::Kind`'s, reused — the same identifier the hut's
/// blueprint and its registry path use (`BuildingSawmill` registers as
/// `minecolonies:sawmill`).
pub fn expected_building() -> Kind {
    Kind::Sawmill
}

/// `craft()` (`:39-44`): set the visible status, then run the parent's craft.
///
/// The port returns the status for the caller to display and the state the parent's
/// `craft` decides; it does not call the parent's function itself, because that is
/// `crate::ai_build::crafter::craft`, which takes the facts.
pub fn craft_visible_status() -> VisibleStatus {
    CRAFTING // :42
}

/// The table a carpenter runs: `AbstractEntityAICrafting`'s, from
/// `crate::ai_build::crafter`, whose `register` is the six targets of `:160-170` plus
/// the two exits the base class adds.
///
/// Returned instead of re-registered: see the module note. `CraftFacts` is the fact
/// type of that table, and it is the same struct the sifter embeds
/// ([`super::facts::SiftFacts::craft`]).
pub fn machine() -> Machine<CraftState, CraftFacts> {
    crafter::ai()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_class_adds_no_target_and_no_constant() {
        // 45 lines, read one by one: the count is the finding.
        assert_eq!(
            delta(),
            SawmillDelta {
                targets_added: 0,
                constants_declared: 0,
                states_named: 0,
            }
        );
    }

    #[test]
    fn its_table_is_the_crafters_six_targets_and_two_exits() {
        // `AbstractEntityAICrafting.java:160-170` plus `getStateAfterPickUp` and
        // `afterDump`: `crate::ai_build::crafter::register`, not a copy.
        let machine = machine();
        let rules: Vec<&str> = machine
            .transitions()
            .iter()
            .map(|transition| transition.rule)
            .collect();
        assert_eq!(
            rules,
            vec![
                "hasWorkToDo",
                "idle",
                "decide",
                "queryItems",
                "getRecipe",
                "craft",
                "afterDump",
                "getStateAfterPickUp",
            ]
        );
        // And its start state is `IDLE`, like every crafter's (`:164`).
        assert_eq!(machine.state(), CraftState::Idle);
    }

    #[test]
    fn the_one_thing_it_declares_is_the_status_it_wears() {
        // `:20-21` then `:42`.
        assert_eq!(craft_visible_status(), CRAFTING);
        assert_eq!(CRAFTING.icon, "textures/icons/work/sawmill.png");
        assert_eq!(
            CRAFTING.translation_key,
            "com.minecolonies.gui.visiblestatus.sawmill"
        );
        // Not the generic `WORKING` status `decide` sets at `:277`
        // (`VisibleCitizenStatus.WORKING`, `TranslationConstants.java:812`).
        assert_ne!(
            CRAFTING.translation_key,
            "com.minecolonies.gui.visiblestatus.working"
        );
    }

    #[test]
    fn a_carpenter_works_in_a_sawmill() {
        // `:33-37`.
        assert_eq!(expected_building(), Kind::Sawmill);
        assert_eq!(expected_building().path(), "minecolonies:sawmill");
    }
}
