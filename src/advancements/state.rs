//! What a colony can be asked, and what happened in it.
//!
//! Java reference: the criterion instances of `api/advancements/*/`, each of
//! which is a `test(...)` over one or two values, plus the call sites that fire
//! them. The port splits that into two: the *fact* (a level reached, a
//! population) lives in [`ColonyState`], the *event* (a criterion fired) lives
//! in [`Event`], and both are data — no player, no level, no `ServerPlayer`.
//!
//! **Why events are kept rather than re-tested.** Three triggers ask what the
//! player *did* (`click_gui_button`, `citizen_eat_food`, `building_add_recipe`)
//! and nothing in the colony record says it. Java fires the criterion at the
//! moment it happens and the criterion's `test` is what decides; the port keeps
//! that test ([`Event::accepts`]) and lets the state remember what passed. A
//! record is stored once per *shape*: [`ColonyState::record`] drops an event
//! whose every condition is already answered by a stored one, which is what
//! stops a `complete_build_request` at level 1 from being stored again after a
//! record at level 3.
//!
//! **Two things this state does not have.** No citizen list: an advancement
//! only ever asks the *count* (`colony_population`, `army_population`), and the
//! list is `crate::colony::Colony`'s. No recipe storage either: a
//! `building_add_recipe` condition is an event, not a query.

use std::collections::BTreeMap;

use crate::colony::colonie::{LEVEL_MAX, LEVEL_MIN};

/// One criterion firing, with the parameters the advancement JSON can ask for.
///
/// The parameter names are the JSON's (`hut_name`, `level`, `items`,
/// `crafting_size`, `button_id`, `window_resource_location`, `block`), so a row
/// of [`super::tree::TREE`] can be read against the file it came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum Event {
    /// `minecolonies:supply_placed` — the supply ship landed
    /// (`PlaceSupplyCriterionInstance.java:13-16`, no condition).
    SupplyPlaced,
    /// `minecolonies:structure_placed` with `hut_name`
    /// (`PlaceStructureCriterionInstance.java:42-48`).
    StructurePlaced { hut: &'static str },
    /// `minecolonies:create_build_request` with `hut_name` and `level`
    /// (`CreateBuildRequestCriterionInstance.java:56-68`).
    BuildRequested { hut: &'static str, level: i32 },
    /// `minecolonies:complete_build_request` with `hut_name` and `level`
    /// (`CompleteBuildRequestCriterionInstance.java:56-67`).
    BuildCompleted { hut: &'static str, level: i32 },
    /// `minecolonies:building_add_recipe` with the recipe's primary output and
    /// its grid size (`BuildingAddRecipeCriterionInstance.java:57-80`).
    RecipeAdded { output: &'static str, grid: i32 },
    /// `minecolonies:click_gui_button` with `button_id` and
    /// `window_resource_location` (`ClickGuiButtonCriterionInstance.java:56-68`).
    GuiClicked {
        button: &'static str,
        window: &'static str,
    },
    /// `minecolonies:open_gui_window` (`OpenGuiWindowCriterionInstance.java:43-45`).
    /// No generated advancement uses it; it is here because the trigger exists
    /// (`AdvancementTriggers.java:30`, registered at `:52`).
    GuiOpened { window: &'static str },
    /// `minecolonies:citizen_eat_food`, with the eaten item when the JSON
    /// narrows it (`CitizenEatFoodCriterionInstance.java:39-54`).
    FoodEaten { item: Option<&'static str> },
    /// `minecolonies:citizen_bury` — no condition (`CitizenBuryCriterionInstance.java:13-16`).
    CitizenBuried,
    /// `minecolonies:citizen_resurrect` (`CitizenResurrectCriterionInstance.java:13-16`).
    CitizenResurrected,
    /// `minecolonies:undertaker_totem` (`UndertakerTotemCriterionInstance.java:13-16`).
    UndertakerTotem,
    /// `minecolonies:deep_mine` (`DeepMineCriterionInstance.java:13-16`).
    DeepMine,
    /// `minecolonies:max_fields` (`MaxFieldsCriterionInstance.java:13-16`).
    MaxFields,
    /// `minecolonies:all_barracks_towers`, id `all_barracks_towers`
    /// (`Constants.java:83`, `AllTowersTrigger.java:18-37`).
    AllBarracksTowers,
    /// Vanilla `minecraft:placed_block` with a block id, which
    /// `production/post_and_stash` is the only row to use.
    BlockPlaced { block: &'static str },
}

impl Event {
    /// Does this *wanted* event accept `happened`?
    ///
    /// Each arm is one criterion instance's `test`, including its looseness:
    /// names compare case-insensitively (`equalsIgnoreCase`) and a level
    /// compares `required <= reached`, because a missed increase must not lose
    /// the advancement (`ColonyPopulationCriterionInstance.java:38` says so).
    /// An exact comparison would be a different rule, not a cleaner one.
    pub fn accepts(&self, happened: &Event) -> bool {
        match (self, happened) {
            (Self::StructurePlaced { hut }, Self::StructurePlaced { hut: reached }) => {
                same_name(*hut, *reached)
            }
            (
                Self::BuildRequested { hut, level },
                Self::BuildRequested {
                    hut: reached,
                    level: at,
                },
            )
            | (
                Self::BuildCompleted { hut, level },
                Self::BuildCompleted {
                    hut: reached,
                    level: at,
                },
            ) => same_name(*hut, *reached) && level_reached(*level, *at),
            (
                Self::RecipeAdded { output, grid },
                Self::RecipeAdded {
                    output: made,
                    grid: size,
                },
            ) => *output == *made && (*grid == -1 || *grid == *size),
            (
                Self::GuiClicked { button, window },
                Self::GuiClicked {
                    button: clicked,
                    window: opened,
                },
            ) => button.eq_ignore_ascii_case(*clicked) && *window == *opened,
            (Self::GuiOpened { window }, Self::GuiOpened { window: opened }) => *window == *opened,
            (Self::FoodEaten { item: None }, Self::FoodEaten { .. }) => true,
            (Self::FoodEaten { item: Some(item) }, Self::FoodEaten { item: eaten }) => {
                *eaten == Some(*item)
            }
            (Self::BlockPlaced { block }, Self::BlockPlaced { block: placed }) => *block == *placed,
            // The seven condition-less criteria: the trigger fires, the
            // advancement is handed over.
            (Self::SupplyPlaced, Self::SupplyPlaced)
            | (Self::CitizenBuried, Self::CitizenBuried)
            | (Self::CitizenResurrected, Self::CitizenResurrected)
            | (Self::UndertakerTotem, Self::UndertakerTotem)
            | (Self::DeepMine, Self::DeepMine)
            | (Self::MaxFields, Self::MaxFields)
            | (Self::AllBarracksTowers, Self::AllBarracksTowers) => true,
            // Two different triggers never accept one another.
            _ => false,
        }
    }
}

/// `this.hutName.equalsIgnoreCase(structureName)`.
fn same_name(wanted: &str, reached: &str) -> bool {
    wanted.eq_ignore_ascii_case(reached)
}

/// `this.level <= level`, where `-1` is the JSON's "no level condition"
/// (`CompleteBuildRequestCriterionInstance.java:56-67`).
fn level_reached(wanted: i32, reached: i32) -> bool {
    wanted == -1 || wanted <= reached
}

/// The colony as an advancement can see it: a population, an army, the level
/// each hut type reached, and what happened.
///
/// A hut type is named by its **building type path** (`baker`, `townhall`,
/// `barrackstower`), which is what `WorkOrderBuilding.java:256` passes to the
/// completion trigger — not the hut block's id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct ColonyState {
    pub citizens: u32,
    /// Guards, a number the colony record does not keep: Java sums the guard
    /// buildings' assigned citizens (`ArmyPopulationTrigger`). It is a field
    /// here so a caller that knows it can say it.
    pub army: u32,
    hut_levels: BTreeMap<String, i32>,
    happened: Vec<Event>,
}

impl ColonyState {
    pub fn new() -> ColonyState {
        ColonyState::default()
    }

    /// A colony of `citizens`, no hut, nothing done.
    pub fn with_citizens(citizens: u32) -> ColonyState {
        ColonyState {
            citizens,
            ..ColonyState::default()
        }
    }

    /// One more colonist; returns the new population.
    pub fn recruit(&mut self) -> u32 {
        self.citizens += 1;
        self.citizens
    }

    /// Records that a hut of this type stands at `level`, keeping the highest
    /// level seen: a building only goes up, and a level outside `1..=5` is not
    /// a level (`api/util/constant/Constants.java:20`).
    pub fn build(&mut self, hut: &str, level: i32) {
        if level < LEVEL_MIN {
            return;
        }
        let level = level.min(LEVEL_MAX);
        let known = self.hut_levels.entry(hut.to_owned()).or_insert(level);
        *known = (*known).max(level);
    }

    /// The level that hut type reached, if it stands at all.
    pub fn level_of(&self, hut: &str) -> Option<i32> {
        self.hut_levels.get(hut).copied()
    }

    /// Stores an event, unless what is already stored answers every condition
    /// the new one would.
    ///
    /// **The direction matters.** `accepts` is `required <= reached`
    /// (`CompleteBuildRequestCriterionInstance.java:60`): a criterion asking
    /// level 1 is satisfied by a record at level 3, so a record at level 1 adds
    /// nothing once level 3 is stored — it is the **new** event used as the
    /// requirement, satisfied by a stored one, that says so. The other direction
    /// would drop the level 3 record instead, and lose the only level a
    /// `build_smeltery_5` asks for.
    ///
    /// The set is therefore bounded by the distinct shapes an advancement can
    /// ask for, not by how often the trigger fires.
    pub fn record(&mut self, event: Event) {
        if self.happened.iter().any(|stored| event.accepts(stored)) {
            return;
        }
        self.happened.push(event);
    }

    /// Did something that `wanted` accepts ever happen?
    pub fn saw(&self, wanted: &Event) -> bool {
        self.happened.iter().any(|stored| wanted.accepts(stored))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hut_keeps_the_highest_level_and_ignores_a_level_below_one() {
        let mut state = ColonyState::new();
        assert_eq!(state.level_of("smeltery"), None);
        state.build("smeltery", 1);
        state.build("smeltery", 3);
        state.build("smeltery", 2);
        assert_eq!(state.level_of("smeltery"), Some(3));
        state.build("smeltery", 9);
        assert_eq!(state.level_of("smeltery"), Some(5));
        state.build("tavern", 0);
        assert_eq!(state.level_of("tavern"), None);
    }

    #[test]
    fn an_event_a_stored_one_already_accepts_is_not_stored_again() {
        let mut state = ColonyState::new();
        state.record(Event::BuildCompleted {
            hut: "townhall",
            level: 3,
        });
        state.record(Event::BuildCompleted {
            hut: "townhall",
            level: 1,
        });
        state.record(Event::BuildCompleted {
            hut: "townhall",
            level: 2,
        });
        // Three records, one shape: the level 3 one already accepts 1 and 2.
        assert_eq!(state.happened.len(), 1);
        assert!(state.saw(&Event::BuildCompleted {
            hut: "townhall",
            level: 1
        }));
        state.record(Event::BuildCompleted {
            hut: "townhall",
            level: 4,
        });
        // 4 is not covered by the stored 3, so it is kept as a second shape.
        assert_eq!(state.happened.len(), 2);
        assert!(state.saw(&Event::BuildCompleted {
            hut: "townhall",
            level: 4
        }));
        assert!(!state.saw(&Event::BuildCompleted {
            hut: "smeltery",
            level: 1
        }));
    }

    #[test]
    fn two_triggers_of_different_kinds_never_answer_for_each_other() {
        let mut state = ColonyState::new();
        state.record(Event::BuildRequested {
            hut: "builder",
            level: 1,
        });
        assert!(state.saw(&Event::BuildRequested {
            hut: "builder",
            level: 1
        }));
        // A request is not a completion, even with the same hut and level.
        assert!(!state.saw(&Event::BuildCompleted {
            hut: "builder",
            level: 1
        }));
        assert!(!state.saw(&Event::GuiClicked {
            button: "fulfill",
            window: "minecolonies:gui/citizen/requests.xml"
        }));
    }

    #[test]
    fn a_name_counts_in_any_case_and_a_grid_size_matches_exactly() {
        let wanted = Event::StructurePlaced { hut: "townhall" };
        assert!(wanted.accepts(&Event::StructurePlaced { hut: "TownHall" }));
        assert!(!wanted.accepts(&Event::StructurePlaced { hut: "tavern" }));
        let grid = Event::RecipeAdded {
            output: "minecraft:oak_door",
            grid: 3,
        };
        assert!(grid.accepts(&Event::RecipeAdded {
            output: "minecraft:oak_door",
            grid: 3
        }));
        assert!(!grid.accepts(&Event::RecipeAdded {
            output: "minecraft:oak_door",
            grid: 2
        }));
        let any_food = Event::FoodEaten { item: None };
        assert!(any_food.accepts(&Event::FoodEaten {
            item: Some("minecraft:rotten_flesh")
        }));
    }
}
