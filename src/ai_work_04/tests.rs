//! The test that counts: a scenario of facts produces the sequence of states Java
//! produces.
//!
//! Each of the three trades gets one transcript, written as the list of rules that
//! fired, in order, read off the Java method chain:
//!
//! | Trade | The chain in Java |
//! | --- | --- |
//! | Sifter | `IDLE.hasWorkToDo` → `START_WORKING.decide` → `SIFT.sift` (`AbstractEntityAICrafting.java:164-166`, `EntityAIWorkSifter.java:73`, `:91-94`) |
//! | Smelter | `IDLE`→`START_WORKING` (5) → `startWorking` (60) → `getStateAfterPickUp` → `startWorking` → `fillUpFurnace` (`AbstractEntityAIUsesFurnace.java:76-82`) |
//! | Sawmill | the crafter's `hasWorkToDo` → `decide` → `getRecipe` → `queryItems` → `craft` → `afterDump` (`AbstractEntityAICrafting.java:160-170`) |
//!
//! **Why a helper instead of one `tick` per assertion.** Three of the seven smelter
//! rates are 60 or 20 ticks (`:78`, `:80`, `EntityAIWorkSmelter.java:69`) and one
//! machine tick covers `ENTITY_AI_TICKRATE = 5` game ticks
//! (`AbstractEntityCitizen.java:64`), so a rate-60 target is looked at twelve machine
//! ticks after it last ran. [`fired`] ticks until something fires and answers the rule,
//! which is how the Java timeline is read without asserting on the gaps.

use crate::ai::Machine;
use crate::ai_build::crafter::CraftFacts;

use super::facts::{Scene, SiftFacts, SiftRecipe, SmelterFacts};
use super::states::{Refusal, SiftState, SmeltState};
use super::{sawmill, sifter, smelter};

/// Tick until a transition fires, and answer its rule. `None` when `max` ticks pass
/// with nothing — a rate higher than the budget, or a state that holds.
fn fired<S, F>(machine: &mut Machine<S, F>, scene: &mut F, max: usize) -> Option<&'static str>
where
    S: Copy + Eq + core::fmt::Debug,
{
    for _ in 0..max {
        if let Some(fired) = machine.tick(scene) {
            return Some(fired.rule);
        }
    }
    None
}

/// A sifter with a task, a mesh in the hut and a recipe in hand: the scene of
/// `EntityAIWorkSifter.java:107-222`.
fn a_working_sifter(progress: u32) -> SiftFacts {
    SiftFacts {
        craft: CraftFacts {
            task_queue: 1,
            current_task: true,
            ..CraftFacts::default()
        },
        building_meshes: 4,
        recipe: Some(SiftRecipe::complete()),
        progress,
        ..SiftFacts::default()
    }
}

#[test]
fn the_sifter_walks_the_same_states_as_java() {
    // `AbstractEntityAICrafting.java:164-166` then `EntityAIWorkSifter.java:73`.
    let mut machine = sifter::ai();
    let mut scene = Scene::new(a_working_sifter(10));

    assert_eq!(machine.state(), SiftState::Idle);
    assert_eq!(fired(&mut machine, &mut scene, 1), Some("hasWorkToDo"));
    assert_eq!(machine.state(), SiftState::StartWorking);

    // `:91-94` — `decide` answers `SIFT` whatever the facts say.
    assert_eq!(fired(&mut machine, &mut scene, 1), Some("decide"));
    assert_eq!(machine.state(), SiftState::Sift);

    // `:186` — one hit, and the state holds.
    assert_eq!(fired(&mut machine, &mut scene, 2), Some("sift"));
    assert_eq!(machine.state(), SiftState::Sift);
    assert_eq!(scene.facts.progress, 11);
}

#[test]
fn the_fifty_first_hit_makes_one_output_and_keeps_the_state() {
    // `:188-212` — `progress > 50` fires, and `:221` answers `getState()`.
    let mut machine = sifter::ai();
    let mut scene = Scene::new(a_working_sifter(50));

    // IDLE → START_WORKING → SIFT.
    fired(&mut machine, &mut scene, 1);
    fired(&mut machine, &mut scene, 1);
    assert_eq!(machine.state(), SiftState::Sift);

    assert_eq!(fired(&mut machine, &mut scene, 2), Some("sift"));
    assert_eq!(
        machine.state(),
        SiftState::Sift,
        "an output does not leave SIFT"
    );
    assert_eq!(scene.facts.progress, 0, ":190 resets the counter");
    assert_eq!(scene.facts.daily_quantity, 1, ":191 counts the output");
    // `roll: 99` in the default facts is above `CHANCE_TO_DUMP_INV`, so `:192` did
    // **not** count an action and the base table has nothing to dump.
    assert_eq!(scene.facts.craft.actions_done, 0);
}

#[test]
fn a_sifter_without_a_mesh_complains_waits_and_asks_again() {
    // `:133-146`, `:144` — `setDelay(NO_MESH_DELAY)` then `START_WORKING`.
    let facts = SiftFacts {
        building_meshes: 0,
        worker_meshes: 0,
        recipe: None,
        ..a_working_sifter(7)
    };
    let mut machine = sifter::ai();
    let mut scene = Scene::new(facts);
    fired(&mut machine, &mut scene, 1); // hasWorkToDo
    fired(&mut machine, &mut scene, 1); // decide

    assert_eq!(fired(&mut machine, &mut scene, 2), Some("sift"));
    assert_eq!(machine.state(), SiftState::StartWorking);
    assert_eq!(scene.why_not(), Some(Refusal::NoMesh));
    assert_eq!(scene.memory.delay, sifter::NO_MESH_DELAY as i32);
    assert_eq!(
        scene.facts.progress, 0,
        ":156 clears the progress on the way out"
    );
}

#[test]
fn a_sifter_out_of_quota_goes_idle_without_touching_anything() {
    // `:110-113` — the first test of `sift`, before the walk, the space and the recipe.
    let facts = SiftFacts {
        daily_quantity: 64,
        max_daily_quantity: 64,
        ..a_working_sifter(0)
    };
    let mut machine = sifter::ai();
    let mut scene = Scene::new(facts);
    // `hasWorkToDo` is true here: the override (`:97-100`) is an `||`, and the task
    // queue still holds a task even though the quota is spent.
    assert_eq!(fired(&mut machine, &mut scene, 1), Some("hasWorkToDo"));
    fired(&mut machine, &mut scene, 1); // decide → SIFT
    // From `SIFT`, the spent quota is what sends the sifter back to `IDLE` — with no
    // walk, no hand swap and no progress write, because `:110` comes first.
    assert_eq!(fired(&mut machine, &mut scene, 2), Some("sift"));
    assert_eq!(machine.state(), SiftState::Idle);
    assert_eq!(scene.why_not(), Some(Refusal::QuotaReached));
    assert_eq!(scene.facts.progress, 0);
}

#[test]
fn the_smelter_walks_the_same_states_as_java() {
    // `AbstractEntityAIUsesFurnace.java:76-82`, one line per rule.
    let mut machine = smelter::ai();
    let mut scene = Scene::new(SmelterFacts::new());

    assert_eq!(machine.state(), SmeltState::Idle);
    // `:77` — `IDLE → START_WORKING`, no condition, rate 5.
    assert_eq!(
        fired(&mut machine, &mut scene, 1),
        Some("idleToStartWorking")
    );
    assert_eq!(machine.state(), SmeltState::StartWorking);

    // `:78` — `startWorking`: the hut has ore and the pockets do not, so `:249-253`
    // answers `GATHERING_REQUIRED_MATERIALS`.
    assert_eq!(fired(&mut machine, &mut scene, 1), Some("startWorking"));
    assert_eq!(machine.state(), SmeltState::GatheringRequiredMaterials);

    // `AbstractEntityAIBasic.java:323-326` — the base class sends it back.
    assert_eq!(
        fired(&mut machine, &mut scene, 1),
        Some("getStateAfterPickUp")
    );
    assert_eq!(machine.state(), SmeltState::StartWorking);

    // The citizen fetched the ore; the fuel is still in the hut, so the second pass of
    // `startWorking` gathers fuel (`:254-258`). Its target is rate 60 (`:78`), i.e.
    // twelve machine ticks.
    scene.facts.smeltable_in_building = 0;
    scene.facts.smeltable_in_inventory = 8;
    assert_eq!(fired(&mut machine, &mut scene, 12), Some("startWorking"));
    assert_eq!(machine.state(), SmeltState::GatheringRequiredMaterials);

    // Same dance for the fuel, after which both sides are in the pockets and `:260`
    // finds the empty furnace able to take them (`:330-332`).
    fired(&mut machine, &mut scene, 1); // getStateAfterPickUp
    scene.facts.fuel_in_building = 0;
    scene.facts.fuel_in_inventory = 8;
    scene.facts.inventory_has_smeltable = true;
    scene.facts.inventory_has_fuel = true;
    assert_eq!(fired(&mut machine, &mut scene, 12), Some("startWorking"));
    assert_eq!(machine.state(), SmeltState::FillUpFurnaces);

    // `:79` — `fillUpFurnace` empties `walkTo` and answers `START_WORKING` (`:497`).
    assert_eq!(fired(&mut machine, &mut scene, 1), Some("fillUpFurnace"));
    assert_eq!(machine.state(), SmeltState::StartWorking);
    assert_eq!(scene.memory.walk_to, None);
}

#[test]
fn the_acceleration_event_is_checked_first_and_stops_nothing() {
    // `:80` — an `AIEventTarget` whose action returns null (`:305`), so the state's own
    // list still runs in the same tick (`TickRateStateMachine.java:75-81`, then `:99`).
    let mut machine = smelter::ai();
    let mut scene = Scene::new(SmelterFacts::new());
    // `IDLE → START_WORKING`: from here both countdowns are at zero, so the next tick
    // has to choose between the event and the state's own rule.
    assert_eq!(
        fired(&mut machine, &mut scene, 1),
        Some("idleToStartWorking")
    );
    // The very next tick: `accelerateFurnaces` is looked at (AI_BLOCKING) and returns
    // null, then `startWorking` fires — proof that the event did not consume the tick.
    assert_eq!(fired(&mut machine, &mut scene, 1), Some("startWorking"));
}

#[test]
fn a_furnace_that_disappeared_is_a_named_refusal_and_is_forgotten() {
    // `:400-406` — the position no longer holds a furnace block entity: Java forgets
    // `walkTo` and answers `START_WORKING`.
    let facts = SmelterFacts {
        furnaces: Vec::new(),
        ..SmelterFacts::new()
    };
    let gone = smelter::retrieve_product(&facts, Some(crate::ai::Pos::new(9, 65, 9)));
    assert_eq!(gone, smelter::Retrieve::Gone);
    assert!(
        gone.forgets_target(),
        ":404 sets `walkTo = null` before returning"
    );
    assert_eq!(
        gone.next(SmeltState::RetrievingEndProductFromFurnace),
        SmeltState::StartWorking // :405
    );
    // With no target at all, the same state answers without a refusal (`:390-393`).
    assert_eq!(
        smelter::retrieve_product(&facts, None),
        smelter::Retrieve::NoTarget
    );
}

#[test]
fn the_sawmill_walks_the_crafting_states_of_its_parent() {
    // `AbstractEntityAICrafting.java:164-169` — the six targets, then the base exits.
    let mut machine = sawmill::machine();
    let mut scene = CraftFacts {
        task_queue: 1,
        current_task: true,
        // `getRecipe` assigns `currentRequest` at `:366`; the port reads that as a fact.
        has_request: true,
        // A level-99 crafter needs zero hits (`:734-738`: `10 / 50 * 3 == 0`), which
        // keeps this transcript about the states and not about the clock.
        craft_speed_level: 99,
        ..CraftFacts::default()
    };

    assert_eq!(fired(&mut machine, &mut scene, 1), Some("hasWorkToDo"));
    assert_eq!(fired(&mut machine, &mut scene, 1), Some("decide"));
    assert_eq!(fired(&mut machine, &mut scene, 1), Some("getRecipe"));
    assert_eq!(fired(&mut machine, &mut scene, 1), Some("queryItems"));
    // `:609-619` — one run is all the request asked for, so the task finalizes and the
    // base table dumps (`:646`).
    assert_eq!(fired(&mut machine, &mut scene, 1), Some("craft"));
    assert_eq!(
        machine.state(),
        crate::ai_build::crafter::CraftState::InventoryFull
    );
    assert_eq!(fired(&mut machine, &mut scene, 1), Some("afterDump"));
    assert_eq!(machine.state(), crate::ai_build::crafter::CraftState::Idle);
}

#[test]
fn an_idle_sifter_with_nothing_to_do_says_why() {
    // `:164`'s condition fails (no queue, no quota) so `:165`'s `idle` runs; the pastime
    // is the caller's, and the reason is the port's.
    let mut machine = sifter::ai();
    let mut scene = Scene::new(SiftFacts {
        daily_quantity: 64,
        max_daily_quantity: 64,
        ..SiftFacts::default()
    });
    assert_eq!(machine.tick(&mut scene).map(|fired| fired.rule), None);
    assert_eq!(machine.state(), SiftState::Idle);
    assert_eq!(scene.why_not(), Some(Refusal::NothingToCraft));
}

#[test]
fn no_state_of_either_new_table_is_a_trap() {
    // `BasicStateMachine.java:220-228`: entering a state no transition names resets the
    // machine to `INIT`. Every state the two tables can enter must be leavable.
    let sift = sifter::ai();
    for state in SiftState::ALL {
        assert!(sift.can_leave(state), "sifter: {state} has no exit");
    }
    let smelt = smelter::ai();
    for state in SmeltState::ALL {
        assert!(smelt.can_leave(state), "smelter: {state} has no exit");
    }
}

#[test]
fn every_rule_of_both_tables_cites_a_java_line() {
    // The rule of the port: no rule without its source.
    let sift = sifter::ai();
    for transition in sift.transitions() {
        assert!(
            transition.java.contains(".java:"),
            "{} has no java reference",
            transition.rule
        );
    }
    let smelt = smelter::ai();
    for transition in smelt.transitions() {
        assert!(
            transition.java.contains(".java:"),
            "{} has no java reference",
            transition.rule
        );
    }
}
