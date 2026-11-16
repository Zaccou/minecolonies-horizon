//! Does an advancement's condition hold, and the named refusal when it does not.
//!
//! Java reference: the 16 criterion instances of `api/advancements/*/`. Each has
//! a `test(...)` that returns a boolean; the port keeps the test and replaces
//! the boolean with a refusal that says **which** measurement is short, because
//! "false" cannot be told apart from "wrong trigger" in a log.
//!
//! **Two shapes, both pure.** A condition is either a measurement of the colony
//! (a population, an army, the level a hut type reached) or a recorded event.
//! Nothing reads the clock, the world or the player: [`Condition::met`] takes a
//! [`ColonyState`] and answers. A caller — a hook, later — records what happened
//! and asks again.
//!
//! **Conjunctions come from `requirements`, not from a combinator.** Minecraft's
//! `requirements` is "one criterion per group, all groups", so an advancement
//! with five criteria needs five conditions to hold: [`Condition::BuiltAll`] and
//! [`Condition::HappenedAll`] carry that list, and a single criterion is
//! [`Condition::Built`] or [`Condition::Happened`]. The one OR in the data
//! (`minecolonies/fulfill_request`: two buttons of the same group) is
//! [`Condition::HappenedAny`].

use super::state::{ColonyState, Event};

/// What an advancement asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum Condition {
    /// `colony_population` with `population_count`
    /// (`ColonyPopulationCriterionInstance.java:36-40`): at least this many
    /// citizens, **not** exactly this many — a missed increase must not lose
    /// the advancement.
    Population { at_least: u32 },
    /// `army_population` with `population_count`
    /// (`ArmyPopulationCriterionInstance.java:36-40`), same rule on guards.
    Army { at_least: u32 },
    /// `complete_build_request` with `hut_name` (+ `level`): the hut type
    /// stands at least at that level (`CompleteBuildRequestCriterionInstance.java:56-67`).
    ///
    /// Java answers this from the trigger — it fires once, at completion
    /// (`WorkOrderBuilding.java:256`) — but what the criterion compares is the
    /// **level the structure now has**, so the port asks the colony instead and
    /// the answer survives the trigger (`production/build_smeltery_5` stays
    /// true at level 5 even if the trigger was missed).
    Built { hut: &'static str, level: i32 },
    /// One `Built` per criterion, all required: five herders
    /// (`production/build_all_herders`).
    BuiltAll(&'static [(&'static str, i32)]),
    /// The one criterion a row has, which 14 of the 62 rows use: this event,
    /// at least once.
    Happened(Event),
    /// One in [`Event`] per criterion, all required: six doors, two blocks.
    HappenedAll(&'static [Event]),
    /// At least one of them: `fulfill_request`'s two windows.
    HappenedAny(&'static [Event]),
}

impl Condition {
    /// **The resolution.** `Ok(())` when the condition holds; the error names
    /// the measurement that does not.
    pub fn met(&self, state: &ColonyState) -> Result<(), Refusal> {
        match self {
            Self::Population { at_least } => {
                if state.citizens >= *at_least {
                    Ok(())
                } else {
                    Err(Refusal::PopulationTooLow {
                        need: *at_least,
                        have: state.citizens,
                    })
                }
            }
            Self::Army { at_least } => {
                if state.army >= *at_least {
                    Ok(())
                } else {
                    Err(Refusal::ArmyTooSmall {
                        need: *at_least,
                        have: state.army,
                    })
                }
            }
            Self::Built { hut, level } => built(state, *hut, *level),
            Self::BuiltAll(huts) => {
                for &(hut, level) in *huts {
                    built(state, hut, level)?;
                }
                Ok(())
            }
            Self::Happened(event) => {
                if state.saw(event) {
                    Ok(())
                } else {
                    Err(Refusal::NeverHappened { event: *event })
                }
            }
            Self::HappenedAll(events) => {
                for event in *events {
                    if !state.saw(event) {
                        return Err(Refusal::NeverHappened { event: *event });
                    }
                }
                Ok(())
            }
            Self::HappenedAny(events) => {
                if events.iter().any(|event| state.saw(event)) {
                    Ok(())
                } else {
                    Err(Refusal::NoneHappened { events: *events })
                }
            }
        }
    }
}

/// One hut's level check, shared by `Built` and `BuiltAll`. A hut that was
/// never built is `have: 0`, which is the truth: no level, not level one.
fn built(state: &ColonyState, hut: &'static str, level: i32) -> Result<(), Refusal> {
    let have = state.level_of(hut).unwrap_or(0);
    if have >= level {
        Ok(())
    } else {
        Err(Refusal::HutTooLow { hut, need: level, have })
    }
}

/// Why a condition does not hold, named.
///
/// The `need`/`have` pairs are the two numbers the criterion compares; the port
/// keeps both because a player-facing message wants them and a test wants them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// Too few citizens (`ColonyPopulationCriterionInstance.java:38`).
    PopulationTooLow { need: u32, have: u32 },
    /// Too few guards (`ArmyPopulationCriterionInstance.java:38`).
    ArmyTooSmall { need: u32, have: u32 },
    /// A hut type that does not stand, or stands lower
    /// (`CompleteBuildRequestCriterionInstance.java:56-67`).
    HutTooLow {
        hut: &'static str,
        need: i32,
        have: i32,
    },
    /// One criterion of a conjunction never fired. The event says which.
    NeverHappened { event: Event },
    /// None of a disjunction fired. The list says which were expected.
    NoneHappened { events: &'static [Event] },
}

impl Refusal {
    /// A stable name for logs and tests, namespaced like a registry entry so it
    /// can never be mistaken for a player-facing sentence.
    pub fn name(self) -> &'static str {
        match self {
            Self::PopulationTooLow { .. } => "advancement.refusal.population",
            Self::ArmyTooSmall { .. } => "advancement.refusal.army",
            Self::HutTooLow { .. } => "advancement.refusal.hut_level",
            Self::NeverHappened { .. } => "advancement.refusal.event",
            Self::NoneHappened { .. } => "advancement.refusal.any_event",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

#[cfg(test)]
mod tests {
    use super::super::tree;
    use super::*;

    fn condition_of(id: &str) -> Condition {
        tree::find(id).expect("the row exists").condition
    }

    #[test]
    fn a_population_condition_names_the_gap_it_found() {
        let condition = condition_of("minecolonies/colony_population_5");
        let mut state = ColonyState::with_citizens(3);
        assert_eq!(
            condition.met(&state),
            Err(Refusal::PopulationTooLow { need: 5, have: 3 })
        );
        state.recruit();
        state.recruit();
        assert_eq!(condition.met(&state), Ok(()));
        // 10 civilians satisfy the "5" row: at least, never exactly.
        assert_eq!(condition.met(&ColonyState::with_citizens(10)), Ok(()));
    }

    #[test]
    fn a_hut_at_a_higher_level_satisfies_a_lower_requirement() {
        let condition = condition_of("production/build_smeltery");
        let mut state = ColonyState::new();
        assert_eq!(
            condition.met(&state),
            Err(Refusal::HutTooLow {
                hut: "smeltery",
                need: 1,
                have: 0
            })
        );
        state.build("smeltery", 5);
        assert_eq!(condition.met(&state), Ok(()));
        assert_eq!(condition_of("production/build_smeltery_5").met(&state), Ok(()));
        // A different hut type is a different building.
        assert!(condition_of("production/build_baker").met(&state).is_err());
    }

    #[test]
    fn five_herders_are_five_conditions_and_the_last_one_is_named() {
        let condition = condition_of("production/build_all_herders");
        let mut state = ColonyState::new();
        for hut in ["chickenherder", "cowboy", "swineherder", "rabbithutch"] {
            state.build(hut, 1);
        }
        assert_eq!(
            condition.met(&state),
            Err(Refusal::HutTooLow {
                hut: "shepherd",
                need: 1,
                have: 0
            })
        );
        state.build("shepherd", 1);
        assert_eq!(condition.met(&state), Ok(()));
    }

    #[test]
    fn fulfilling_a_request_takes_either_window_not_both() {
        let condition = condition_of("minecolonies/fulfill_request");
        let mut state = ColonyState::new();
        assert!(matches!(
            condition.met(&state),
            Err(Refusal::NoneHappened { .. })
        ));
        state.record(Event::GuiClicked {
            button: "fulfill",
            window: "minecolonies:gui/windowrequestdetail.xml",
        });
        assert_eq!(condition.met(&state), Ok(()));
    }

    #[test]
    fn six_doors_are_one_conjunction_and_five_of_them_are_not_enough() {
        let condition = condition_of("minecolonies/building_add_recipe_door");
        let mut state = ColonyState::new();
        for output in [
            "minecraft:acacia_door",
            "minecraft:birch_door",
            "minecraft:dark_oak_door",
            "minecraft:jungle_door",
            "minecraft:oak_door",
        ] {
            state.record(Event::RecipeAdded { output, grid: 3 });
        }
        assert_eq!(
            condition.met(&state),
            Err(Refusal::NeverHappened {
                event: Event::RecipeAdded {
                    output: "minecraft:spruce_door",
                    grid: 3
                }
            })
        );
        state.record(Event::RecipeAdded {
            output: "minecraft:spruce_door",
            grid: 3,
        });
        assert_eq!(condition.met(&state), Ok(()));
    }

    #[test]
    fn eating_any_food_satisfies_the_condition_without_items() {
        let any = condition_of("minecolonies/citizen_eat_food_first");
        let flesh = condition_of("minecolonies/citizen_eat_food_rotten_flesh");
        let mut state = ColonyState::new();
        state.record(Event::FoodEaten {
            item: Some("minecraft:bread"),
        });
        assert_eq!(any.met(&state), Ok(()));
        assert!(flesh.met(&state).is_err());
        state.record(Event::FoodEaten {
            item: Some("minecraft:rotten_flesh"),
        });
        assert_eq!(flesh.met(&state), Ok(()));
    }

    #[test]
    fn every_refusal_has_its_own_name() {
        let all = [
            Refusal::PopulationTooLow { need: 5, have: 1 },
            Refusal::ArmyTooSmall { need: 8, have: 1 },
            Refusal::HutTooLow {
                hut: "baker",
                need: 1,
                have: 0,
            },
            Refusal::NeverHappened {
                event: Event::DeepMine,
            },
            Refusal::NoneHappened {
                events: &[Event::DeepMine],
            },
        ];
        for (index, refusal) in all.iter().enumerate() {
            for other in &all[index + 1..] {
                assert_ne!(
                    refusal.name(),
                    other.name(),
                    "two refusals share a name, so a log line cannot tell them apart"
                );
            }
            assert!(refusal.name().starts_with("advancement.refusal."));
        }
    }
}
