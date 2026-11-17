//! La table du module et les quatre états du four : ce que ces trois métiers décident,
//! et rien d'autre.
//!
//! Les scans de fours sont testés dans `furnace.rs`, à côté de la règle ; ce fichier
//! teste la couture (quelle machine, quels états, quelle compétence) et les quatre états
//! que le souffleur de verre ajoute. Java référence : `AbstractEntityAICrafting.java:
//! 160-170`, `:724-744`, `AbstractEntityAIBasic.java:191-198`, `:1921-1924` et
//! `AbstractEntityAIRequestSmelter.java:73-81`, `:178-500`.

use crate::ai::Pos;
use crate::ai_build::crafter::{self, CraftFacts, CraftState};
use crate::ai_build::materials::NeedKey;
use crate::jobs::Skill;

use super::facts::{Furnace, FurnaceStack, Scene, SmelterFacts, Trade};
use super::state::{TradeAction, TradeRefusal, TradeState};
use super::{
    actions_until_dumping, after_dump_pickup_allowed, can_go_idle, craft_hits_for,
    state_after_pickup, states_of, TradeAi, STANDARD_DELAY, STACKSIZE, TICKS_SECOND,
};

/// Une scène de souffleur de verre avec un four vide à `(1, 64, 1)`.
fn glass_with_one_furnace() -> Scene {
    let mut scene = Scene::new(Trade::Glassblower);
    let facts = facts_of(&mut scene);
    facts.furnaces = vec![Furnace::empty(Pos::new(1, 64, 1))];
    scene
}

/// Les faits du four d'une scène de souffleur de verre.
///
/// `expect` plutôt qu'`unwrap` : la scène est construite par ce fichier, et un
/// souffleur de verre sans faits de four est une erreur de test, pas une donnée du
/// monde.
fn facts_of(scene: &mut Scene) -> &mut SmelterFacts {
    scene
        .smelter
        .as_mut()
        .expect("a glassblower scene carries furnace facts")
}

/// Une recette de cuisson chargée : du sable qui devient du verre, une demande ouverte.
fn load_furnace_recipe(scene: &mut Scene, max: u32) {
    let facts = facts_of(scene);
    facts.intermediate_is_furnace = true;
    facts.smeltable = Some(NeedKey::plain("minecraft:sand"));
    facts.output = Some(NeedKey::plain("minecraft:glass"));
    scene.base.has_recipe = true;
    scene.base.has_request = true;
    scene.base.max_crafting_count = max;
    scene.base.request_count = max;
}

// ------------------------------------------------------------------- the table

#[test]
fn the_three_trades_are_the_three_java_classes() {
    assert_eq!(Trade::ALL.len(), 3);
    let table = [
        (Trade::Fletcher, "EntityAIWorkFletcher", "BuildingFletcher"),
        (
            Trade::Glassblower,
            "EntityAIWorkGlassblower",
            "BuildingGlassblower",
        ),
        (Trade::Mechanic, "EntityAIWorkMechanic", "BuildingMechanic"),
    ];
    for (trade, class, building) in table {
        assert_eq!(trade.class(), class);
        assert_eq!(trade.building(), building);
        assert_eq!(trade.to_string(), class);
        assert!(trade.expected_building_java().contains(".java:"));
    }
}

#[test]
fn the_stubs_add_nothing_so_the_parents_carry_everything() {
    // Measured on the tree: 27, 28 and 28 lines, and the only code is the
    // `@Override` of `getExpectedBuildingClass()`.
    assert_eq!(Trade::Fletcher.parent(), "AbstractEntityAICrafting");
    assert_eq!(Trade::Mechanic.parent(), "AbstractEntityAICrafting");
    assert_eq!(Trade::Glassblower.parent(), "AbstractEntityAIRequestSmelter");
    // The one asymmetry of the lot: the parent pair of the glassblower is two classes
    // deep, and it is the only one that registers the furnace targets
    // (`AbstractEntityAIRequestSmelter.java:71-82`).
    for trade in Trade::ALL {
        assert_eq!(trade.uses_furnaces(), trade == Trade::Glassblower, "{trade}");
    }
}

#[test]
fn the_craft_speed_skill_is_not_the_primary_for_two_of_three() {
    // `CraftingWorkerBuildingModule.getCraftSpeedSkill()` as `BuildingModules.java`
    // passes it — and `AbstractEntityAICrafting.java:735` reads it.
    for (trade, skill, java) in [
        (Trade::Fletcher, Skill::Creativity, "BuildingModules.java:306"),
        (Trade::Glassblower, Skill::Focus, "BuildingModules.java:320"),
        (Trade::Mechanic, Skill::Knowledge, "BuildingModules.java:333"),
    ] {
        assert_eq!(trade.craft_speed_skill(), skill, "{trade}");
        assert_eq!(trade.craft_speed_java(), java, "{trade}");
    }
    // The two that are **not** the primary of their module: that is the whole point.
    assert_ne!(
        Trade::Fletcher.craft_speed_skill(),
        Trade::Fletcher.primary_skill().expect("the fletcher is employed")
    );
    assert_ne!(
        Trade::Glassblower.craft_speed_skill(),
        Trade::Glassblower.primary_skill().expect("the glassblower is employed")
    );
    // The mechanic uses the five-argument constructor, so its craft speed **is** its
    // primary (`CraftingWorkerBuildingModule.java:34-43`).
    assert_eq!(
        Trade::Mechanic.craft_speed_skill(),
        Trade::Mechanic.primary_skill().expect("the mechanic is employed")
    );
}

#[test]
fn the_primary_and_secondary_come_from_the_employment_table() {
    // `crate::jobs::employment` already holds the two skills of the worker module
    // (`BuildingModules.java:302-334`); this reads them rather than restating them.
    assert_eq!(Trade::Fletcher.primary_skill(), Some(Skill::Dexterity));
    assert_eq!(Trade::Fletcher.secondary_skill(), Some(Skill::Creativity));
    assert_eq!(Trade::Glassblower.primary_skill(), Some(Skill::Creativity));
    assert_eq!(Trade::Glassblower.secondary_skill(), Some(Skill::Focus));
    assert_eq!(Trade::Mechanic.primary_skill(), Some(Skill::Knowledge));
    assert_eq!(Trade::Mechanic.secondary_skill(), Some(Skill::Agility));
    // The acceleration reads the **secondary**, so the glassblower is accelerated by
    // `Focus` (`AbstractEntityAIRequestSmelter.java:122`), while the furnace count
    // reads the **primary** (`:675`). Two lines, two different skills.
    assert_ne!(
        Trade::Glassblower.primary_skill(),
        Trade::Glassblower.secondary_skill()
    );
}

#[test]
fn the_recipe_improvement_skill_is_the_other_one() {
    assert_eq!(Trade::Fletcher.recipe_improvement_skill(), Skill::Dexterity);
    assert_eq!(
        Trade::Glassblower.recipe_improvement_skill(),
        Skill::Creativity
    );
    assert_eq!(Trade::Mechanic.recipe_improvement_skill(), Skill::Agility);
    // For the fletcher and the glassblower it is the primary; for the mechanic the
    // secondary. Never the craft speed skill of the same trade.
    for trade in Trade::ALL {
        assert_ne!(
            trade.recipe_improvement_skill(),
            trade.craft_speed_skill(),
            "{trade}"
        );
    }
}

#[test]
fn only_the_glassblower_reaches_the_four_furnace_states() {
    assert_eq!(states_of(Trade::Glassblower).len(), 11);
    for trade in [Trade::Fletcher, Trade::Mechanic] {
        let states = states_of(trade);
        assert_eq!(states.len(), 7, "{trade}");
        for state in TradeState::FURNACE {
            assert!(!states.contains(&state), "{trade} must not know {state}");
        }
    }
    for state in TradeState::FURNACE {
        assert!(states_of(Trade::Glassblower).contains(&state), "{state}");
    }
}

#[test]
fn every_state_of_every_trade_has_a_way_out() {
    // A state no transition names is Java's trap (`BasicStateMachine.java:220-228`),
    // which is a bug here, not a game event.
    for trade in Trade::ALL {
        let ai = TradeAi::new(trade);
        for state in states_of(trade) {
            assert!(
                ai.machine().can_leave(state),
                "{} leaves {state} with no transition",
                trade.class()
            );
        }
    }
}

#[test]
fn every_trade_dumps_after_one_action() {
    // `AbstractEntityAICrafting.getActionsDoneUntilDumping()` (`:724-727`): 1, for the
    // three of them — the parent is the same class for all three.
    for trade in Trade::ALL {
        assert_eq!(actions_until_dumping(trade), 1, "{trade}");
        assert_eq!(
            super::actions_until_dumping_java(trade),
            "AbstractEntityAICrafting.java:724-727"
        );
    }
    // The base class's answer is 32 (`AbstractEntityAIBasic.java:425-428`), so this is
    // an override and not a detail.
    assert_ne!(actions_until_dumping(Trade::Fletcher), crate::ai::worker::ACTIONS_UNTIL_DUMP);
}

#[test]
fn a_dump_allows_a_pickup_only_without_an_open_request() {
    // `isAfterDumpPickupAllowed()` (`:741-744`) — `currentRequest == null`.
    let mut base = CraftFacts::default();
    assert!(after_dump_pickup_allowed(&base));
    base.has_request = true;
    assert!(!after_dump_pickup_allowed(&base));
    // None of the three classes overrides it, so the answer is the same for all three.
    for trade in Trade::ALL {
        assert!(!after_dump_pickup_allowed(&base), "{trade}");
    }
}

#[test]
fn a_pickup_hands_back_to_start_working() {
    // `getStateAfterPickUp()` (`:436-439`) — `START_WORKING`, not the base class's
    // `IDLE` (`AbstractEntityAIBasic.java:325`).
    for trade in Trade::ALL {
        assert_eq!(
            state_after_pickup(trade),
            TradeState::Base(CraftState::StartWorking)
        );
        assert!(!can_go_idle(trade), "`AbstractEntityAIBasic.java:1923`");
    }
}

#[test]
fn a_machine_starts_in_idle_and_leaves_it_only_for_work() {
    // `AbstractEntityCitizen` starts in `INIT`; this port starts where
    // `initSafetyChecks` arrives (`AbstractEntityAIBasic.java:446-465`).
    for trade in Trade::ALL {
        let mut ai = TradeAi::new(trade);
        assert_eq!(ai.state(), TradeState::Base(CraftState::Idle), "{trade}");
        assert_eq!(ai.trade(), trade);

        // No work at all: `hasWorkToDo` is false (`:261-264`) and `idle` answers
        // `NO_CHANGE`, so the state does not move.
        let mut idle = ai.scene();
        let outcome = ai.tick(&mut idle);
        assert_eq!(outcome.goal(), TradeState::Base(CraftState::Idle));
        assert!(!outcome.moved(), "{trade} stays idle with nothing to do");
    }
    // A fresh machine per trade: the transitions' countdowns are real, so the second
    // phase must not inherit the first one's.
    for trade in Trade::ALL {
        let mut ai = TradeAi::new(trade);
        // A task in the queue: `:164` sends it to `START_WORKING`.
        let mut working = ai.scene();
        working.base.task_queue = 1;
        working.base.current_task = true;
        let mut outcome = ai.tick(&mut working);
        // `IDLE` is looked at once every twenty ticks, so give the countdown its ticks.
        for _ in 0..(TICKS_SECOND / crate::ai::state::ENTITY_AI_TICKRATE) {
            if outcome.moved() {
                break;
            }
            outcome = ai.tick(&mut working);
        }
        assert_eq!(outcome.goal(), TradeState::Base(CraftState::StartWorking));
        assert!(outcome.moved());
        assert_eq!(outcome.why(), Some("AbstractEntityAICrafting.java:164"));
    }
}

#[test]
fn a_wrong_hut_never_leaves_idle() {
    // `AbstractEntityAIBasic.java:191-198` — Java refuses to leave `INIT`; this
    // machine starts in `IDLE` and holds it, saying why.
    let mut ai = TradeAi::new(Trade::Mechanic);
    let mut scene = ai.scene();
    scene.hut_right_class = false;
    scene.base.task_queue = 1;
    scene.base.current_task = true;
    let outcome = ai.tick(&mut scene);
    assert_eq!(outcome.goal(), TradeState::Base(CraftState::Idle));
    assert!(!outcome.moved(), "work to do, and still no move");
    assert_eq!(outcome.why_not(), Some(TradeRefusal::WrongBuildingClass));
    assert_eq!(
        outcome.why(),
        Some("AbstractEntityAIBasic.java:191-198")
    );
}

#[test]
fn the_trade_table_answers_like_the_crafter_it_delegates_to() {
    // The seven shared targets are declared twice (`Machine<S, F>` is monomorphic),
    // and every action calls `crafter::*`. This holds the contract: on the same facts,
    // the two machines walk the same states, tick for tick.
    let mut facts = CraftFacts {
        task_queue: 1,
        current_task: true,
        ..CraftFacts::default()
    };
    let mut theirs = crafter::ai();
    let mut ai = TradeAi::new(Trade::Fletcher);
    let mut scene = ai.scene();
    scene.base = facts.clone();

    let mut mine = Vec::new();
    let mut other = Vec::new();
    for _ in 0..(TICKS_SECOND * 2) {
        mine.push(ai.tick(&mut scene).goal().java_name());
        theirs.tick(&mut facts);
        other.push(theirs.state().java_name());
        assert_eq!(
            scene.base.has_recipe, facts.has_recipe,
            "the two halves must agree on the recipe flag"
        );
    }
    assert_eq!(mine, other, "same facts, same state sequence");
    // And the sequence is not a constant: it visits more than one state.
    assert!(
        mine.iter().any(|state| *state != mine[0]),
        "the scenario must move: {mine:?}"
    );
}

#[test]
fn the_tick_refills_the_craft_speed_level_from_the_trade_s_skill() {
    // `AbstractEntityAICrafting.java:735` reads `getCraftSpeedSkill()`; the trade owns
    // which skill that is, so a caller cannot get it wrong.
    let mut fletcher = TradeAi::new(Trade::Fletcher);
    let mut scene = fletcher.scene();
    scene.skills = scene
        .skills
        .with(Skill::Creativity, 40)
        .with(Skill::Dexterity, 90);
    assert_eq!(scene.craft_speed_level(), 40, "creativity, not dexterity");
    fletcher.tick(&mut scene);
    assert_eq!(scene.base.craft_speed_level, 40);

    let mut glass = TradeAi::new(Trade::Glassblower);
    let mut scene = glass.scene();
    scene.skills = scene.skills.with(Skill::Focus, 12);
    glass.tick(&mut scene);
    assert_eq!(scene.base.craft_speed_level, 12, "focus, not creativity");
}

#[test]
fn two_citizens_with_the_same_points_do_not_hit_the_same_number_of_times() {
    // `getRequiredProgressForMakingRawMaterial` (`:734-738`) divides by the craft speed
    // skill: 30 hits at level 0, none from level 40 on.
    let mut a = Scene::new(Trade::Fletcher);
    a.skills = a.skills.with(Skill::Dexterity, 40);
    let mut b = Scene::new(Trade::Fletcher);
    b.skills = b.skills.with(Skill::Creativity, 40);

    assert_eq!(craft_hits_for(&a), Ok(30), "40 points in the wrong skill");
    assert_eq!(craft_hits_for(&b), Ok(0), "40 points in the right one");
    let a_hits = craft_hits_for(&a).expect("a level is never negative");
    let b_hits = craft_hits_for(&b).expect("a level is never negative");
    assert!(b_hits < a_hits);
}

#[test]
fn a_scene_is_the_trade_its_facts_and_its_memory() {
    let ai = TradeAi::new(Trade::Glassblower);
    let mut scene = ai.scene();
    assert_eq!(scene.trade, Trade::Glassblower);
    assert!(scene.smelter.is_some());
    assert!(!scene.at_chosen_furnace());
    assert_eq!(scene.memory.delay, 0);
    scene.memory.furnace_pos = Some(Pos::new(1, 64, 1));
    let facts = facts_of(&mut scene);
    facts.at_furnace = true;
    assert!(scene.at_chosen_furnace(), "the caller reports the walk");

    // A fletcher's scene has no furnace facts, and asking for them is not a silent
    // empty answer.
    let fletcher = TradeAi::new(Trade::Fletcher).scene();
    assert!(fletcher.smelter.is_none());
    assert!(!fletcher.at_chosen_furnace());
}

// ------------------------------------------------------------ the four states

#[test]
fn no_furnaces_is_a_named_refusal_with_the_standard_delay() {
    // `AbstractEntityAIRequestSmelter.java:439-448` — an interaction, a delay, and
    // `START_WORKING`.
    let mut scene = Scene::new(Trade::Glassblower);
    load_furnace_recipe(&mut scene, 3);
    let decision = super::execute_crafting_action(&mut scene);
    assert_eq!(decision.next, Some(TradeState::Base(CraftState::StartWorking)));
    assert_eq!(decision.refusal, Some(TradeRefusal::NoFurnaces));
    assert_eq!(scene.memory.delay, STANDARD_DELAY as i32, "`:446`");
}

#[test]
fn unloaded_furnaces_stop_the_furnace_path() {
    // `:450-453` — one unloaded furnace and nothing else is asked.
    let mut scene = glass_with_one_furnace();
    load_furnace_recipe(&mut scene, 3);
    let facts = facts_of(&mut scene);
    facts.furnaces[0].loaded = false;
    facts.furnaces[0].result = Some(FurnaceStack::new(NeedKey::plain("minecraft:glass"), 2));
    let decision = super::execute_crafting_action(&mut scene);
    assert_eq!(decision.refusal, Some(TradeRefusal::FurnacesUnloaded));
    assert_eq!(decision.next, Some(TradeState::Base(CraftState::StartWorking)));
    assert_eq!(decision.action, None, "not even the result is taken");
}

#[test]
fn an_empty_fuel_list_refuses_by_name_and_holds_the_state() {
    // `:455-463` — `getAllowedFuel().isEmpty()`, an interaction, `getState()`.
    let mut scene = glass_with_one_furnace();
    load_furnace_recipe(&mut scene, 3);
    assert!(facts_of(&mut scene).fuel_list.is_empty());
    let decision = super::execute_crafting_action(&mut scene);
    assert_eq!(decision.refusal, Some(TradeRefusal::NoFuelAllowed));
    assert_eq!(decision.next, None, "`getState()` is a self-transition");
}

#[test]
fn the_output_wins_over_the_unrelated_input_and_fuel_comes_after_both() {
    // `:465-497`, in Java's order.
    let mut scene = glass_with_one_furnace();
    load_furnace_recipe(&mut scene, 3);
    let facts = facts_of(&mut scene);
    facts.fuel_list = vec![NeedKey::plain("minecraft:coal")];
    facts.fuel_in_inventory = true;
    facts.fuel_in_building = 4;
    // A cold furnace with a result and a foreign input: the result wins.
    facts.furnaces[0].result = Some(FurnaceStack::new(NeedKey::plain("minecraft:glass"), 1));
    facts.furnaces[0].smeltable = Some(FurnaceStack::new(NeedKey::plain("minecraft:cobblestone"), 1));

    let decision = super::execute_crafting_action(&mut scene);
    assert_eq!(decision.next, Some(TradeState::RetrieveProductFromFurnace));
    assert_eq!(scene.memory.furnace_pos, Some(Pos::new(1, 64, 1)));

    // Result taken away, the foreign input is next (`:471-475`).
    let facts = facts_of(&mut scene);
    facts.furnaces[0].result = None;
    let decision = super::execute_crafting_action(&mut scene);
    assert_eq!(decision.next, Some(TradeState::RetrieveUnrelatedProduct));

    // Foreign input gone, a fuelled furnace with an empty fuel slot is next
    // (`:477-484`).
    let facts = facts_of(&mut scene);
    facts.furnaces[0].smeltable = None;
    facts.furnaces[0].fuel = None;
    let decision = super::execute_crafting_action(&mut scene);
    assert_eq!(decision.next, Some(TradeState::AddFuelToFurnace));
}

#[test]
fn a_saturated_set_of_furnaces_waits_a_second() {
    // `:486-491` — `burning >= getMaxUsableFurnaces()`, `setDelay(TICKS_SECOND)`.
    let mut scene = glass_with_one_furnace();
    load_furnace_recipe(&mut scene, 3);
    scene.skills = scene.skills.with(Skill::Creativity, 0);
    let facts = facts_of(&mut scene);
    facts.fuel_list = vec![NeedKey::plain("minecraft:coal")];
    facts.intermediate_is_furnace = true;
    facts.furnaces[0].fuel = Some(FurnaceStack::new(NeedKey::plain("minecraft:coal"), 8));
    facts.furnaces[0].lit = true;
    facts.furnaces[0].smeltable = Some(FurnaceStack::new(NeedKey::plain("minecraft:sand"), 3));

    let decision = super::execute_crafting_action(&mut scene);
    assert_eq!(decision.refusal, Some(TradeRefusal::AllFurnacesBusy));
    assert_eq!(scene.memory.delay, TICKS_SECOND as i32, "`:489`");
}

#[test]
fn an_empty_fuelled_furnace_is_what_gets_filled() {
    // `:493-497` — `getEmptyFurnaceWithFuel`.
    let mut scene = glass_with_one_furnace();
    load_furnace_recipe(&mut scene, 8);
    let facts = facts_of(&mut scene);
    facts.fuel_list = vec![NeedKey::plain("minecraft:coal")];
    facts.furnaces[0].fuel = Some(FurnaceStack::new(NeedKey::plain("minecraft:coal"), 8));
    facts.smeltable_in_inventory = 8;
    facts.at_furnace = true;

    let decision = super::execute_crafting_action(&mut scene);
    assert_eq!(decision.next, Some(TradeState::FillUpFurnaces));
    // And the filling transfers, then hands back to `CRAFT` (`:423`).
    let filling = super::fill_up(&mut scene);
    assert_eq!(filling.next, Some(TradeState::Base(CraftState::Craft)));
    match filling.action {
        Some(TradeAction::TransferSmeltable { count, at }) => {
            assert_eq!(count, 8, "all of it goes into the only furnace");
            assert_eq!(at, Pos::new(1, 64, 1));
        }
        other => panic!("expected a transfer, got {other:?}"),
    }
}

#[test]
fn filling_needs_a_smeltable_in_the_inventory() {
    // `:353-358` — no input in the hand: ask the hut, forget the furnace.
    let mut scene = glass_with_one_furnace();
    load_furnace_recipe(&mut scene, 8);
    scene.memory.furnace_pos = Some(Pos::new(1, 64, 1));
    let facts = facts_of(&mut scene);
    facts.smeltable_in_inventory = 0;

    let decision = super::fill_up(&mut scene);
    assert_eq!(
        decision.next,
        Some(TradeState::Base(CraftState::GatheringRequiredMaterials))
    );
    assert_eq!(decision.refusal, Some(TradeRefusal::NoSmeltableForTheHut));
    assert_eq!(scene.memory.furnace_pos, None, "`:356`");
    match decision.action {
        Some(TradeAction::RequestSmeltable { count, key }) => {
            assert_eq!(count, 8);
            assert_eq!(key, NeedKey::plain("minecraft:sand"));
        }
        other => panic!("expected a smeltable request, got {other:?}"),
    }
}

#[test]
fn a_task_already_covered_by_the_furnaces_is_not_filled_again() {
    // `:348-351` — `(max - counter) - inFurnaces - resultInFurnaces <= 0`.
    let mut scene = glass_with_one_furnace();
    load_furnace_recipe(&mut scene, 4);
    scene.memory.furnace_pos = Some(Pos::new(1, 64, 1));
    let facts = facts_of(&mut scene);
    facts.smeltable_in_inventory = 4;
    facts.furnaces[0].smeltable = Some(FurnaceStack::new(NeedKey::plain("minecraft:sand"), 4));

    let decision = super::fill_up(&mut scene);
    assert_eq!(decision.refusal, Some(TradeRefusal::AlreadySmelting));
    assert_eq!(decision.action, None);
}

#[test]
fn filling_a_furnace_never_swings_at_a_null_position() {
    // `:412`'s `hitBlockWithToolInHand(worker, furnacePos)` is dead code: `:381` has
    // just set `furnacePos = null`, and `CitizenItemUtils.java:135-141` returns on
    // null. The transfer goes out; the swing does not.
    let mut scene = glass_with_one_furnace();
    load_furnace_recipe(&mut scene, 8);
    scene.memory.furnace_pos = Some(Pos::new(1, 64, 1));
    let facts = facts_of(&mut scene);
    facts.smeltable_in_inventory = 8;
    facts.at_furnace = true;
    facts.furnaces[0].fuel = Some(FurnaceStack::new(NeedKey::plain("minecraft:coal"), 8));

    let decision = super::fill_up(&mut scene);
    assert!(
        matches!(decision.action, Some(TradeAction::TransferSmeltable { .. })),
        "the transfer is the action"
    );
    assert!(
        !matches!(decision.action, Some(TradeAction::HitFurnace(_))),
        "`:412` never reaches a non-null position"
    );
}

#[test]
fn the_walk_is_asked_before_anything_is_taken() {
    // `:241-244`, `:302-305`, `:367-370`, `:198-201` — `walkToWorkPos` first, and the
    // chosen furnace is kept for the next tick.
    let mut scene = glass_with_one_furnace();
    load_furnace_recipe(&mut scene, 4);
    scene.memory.furnace_pos = Some(Pos::new(1, 64, 1));
    let facts = facts_of(&mut scene);
    facts.at_furnace = false;
    facts.furnaces[0].result = Some(FurnaceStack::new(NeedKey::plain("minecraft:glass"), 1));

    let decision = super::retrieve_product(&mut scene);
    assert_eq!(decision.next, None, "`return getState()`");
    assert_eq!(
        decision.action,
        Some(TradeAction::WalkToFurnace(Pos::new(1, 64, 1)))
    );
    assert_eq!(
        scene.memory.furnace_pos,
        Some(Pos::new(1, 64, 1)),
        "`:245` is not reached"
    );
}

#[test]
fn retrieving_the_product_moves_the_counter_and_finalises_at_the_maximum() {
    // `:248-267` — the delivery, `craftCounter += count`, and
    // `finalizeCraftingTask()` at the maximum.
    let mut scene = glass_with_one_furnace();
    load_furnace_recipe(&mut scene, 4);
    scene.base.craft_counter = 1;
    scene.memory.furnace_pos = Some(Pos::new(1, 64, 1));
    let facts = facts_of(&mut scene);
    facts.at_furnace = true;
    facts.furnaces[0].result = Some(FurnaceStack::new(NeedKey::plain("minecraft:glass"), 2));

    let decision = super::retrieve_product(&mut scene);
    assert_eq!(scene.base.craft_counter, 3, "`:259`");
    assert_eq!(decision.next, Some(TradeState::Base(CraftState::StartWorking)));
    assert!(matches!(
        decision.action,
        Some(TradeAction::ExtractProduct { count: 2, .. })
    ));

    // One more extraction reaches the maximum of 4: the task is finalised
    // (`AbstractEntityAICrafting.java:636-646`), the counters reset.
    scene.memory.furnace_pos = Some(Pos::new(1, 64, 1));
    let facts = facts_of(&mut scene);
    facts.at_furnace = true;
    facts.furnaces[0].result = Some(FurnaceStack::new(NeedKey::plain("minecraft:glass"), 1));
    let decision = super::retrieve_product(&mut scene);
    assert_eq!(decision.next, Some(TradeState::Base(CraftState::InventoryFull)));
    assert_eq!(scene.base.craft_counter, 0, "`resetValues`, `:672`");
    assert_eq!(scene.base.max_crafting_count, 0, "`:670`");
}

#[test]
fn a_foreign_result_becomes_a_secondary_output() {
    // `:270-272` — what is not the recipe's output goes to `getSecondaryOutputs()`.
    let mut scene = glass_with_one_furnace();
    load_furnace_recipe(&mut scene, 4);
    scene.memory.furnace_pos = Some(Pos::new(1, 64, 1));
    let facts = facts_of(&mut scene);
    facts.at_furnace = true;
    facts.furnaces[0].result = Some(FurnaceStack::new(NeedKey::plain("minecraft:brick"), 3));

    let decision = super::retrieve_product(&mut scene);
    assert_eq!(scene.memory.secondary_outputs, 1, "`:271`");
    assert_eq!(scene.base.craft_counter, 0, "no craft counter for a leftover");
    assert_eq!(decision.next, Some(TradeState::Base(CraftState::StartWorking)));
}

#[test]
fn a_leftover_input_is_taken_and_the_inventory_dumped() {
    // `:285-320` — `SMELTABLE_SLOT`, secondary outputs, and `INVENTORY_FULL` (`:315`).
    let mut scene = glass_with_one_furnace();
    load_furnace_recipe(&mut scene, 4);
    scene.memory.furnace_pos = Some(Pos::new(1, 64, 1));
    let facts = facts_of(&mut scene);
    facts.at_furnace = true;
    facts.furnaces[0].smeltable = Some(FurnaceStack::new(NeedKey::plain("minecraft:cobblestone"), 2));

    let decision = super::retrieve_unrelated(&mut scene);
    assert_eq!(decision.next, Some(TradeState::Base(CraftState::InventoryFull)));
    assert_eq!(scene.memory.secondary_outputs, 1, "`:314`");
    match decision.action {
        Some(TradeAction::ExtractUnrelated { count, key, .. }) => {
            assert_eq!(count, 2);
            assert_eq!(key, NeedKey::plain("minecraft:cobblestone"));
        }
        other => panic!("expected an unrelated extraction, got {other:?}"),
    }

    // Our own input is not a leftover (`:295-296`).
    scene.memory.furnace_pos = Some(Pos::new(1, 64, 1));
    let facts = facts_of(&mut scene);
    facts.furnaces[0].smeltable = Some(FurnaceStack::new(NeedKey::plain("minecraft:sand"), 2));
    let decision = super::retrieve_unrelated(&mut scene);
    assert_eq!(decision.refusal, Some(TradeRefusal::SmeltableNotEmpty));
    assert_eq!(scene.memory.furnace_pos, None, "`:298`");
}

#[test]
fn a_furnace_that_is_gone_is_refused_by_name() {
    // `:235-239` and `:293-300` — the chosen position is no longer a furnace.
    let mut scene = glass_with_one_furnace();
    load_furnace_recipe(&mut scene, 4);
    scene.memory.furnace_pos = Some(Pos::new(9, 64, 9));
    assert_eq!(
        super::retrieve_product(&mut scene).refusal,
        Some(TradeRefusal::FurnaceGone)
    );
    scene.memory.furnace_pos = Some(Pos::new(9, 64, 9));
    assert_eq!(
        super::retrieve_unrelated(&mut scene).refusal,
        Some(TradeRefusal::FurnaceGone)
    );
    // With nothing chosen at all, both go back to work (`:229`, `:287`)
    // without inventing a refusal.
    scene.memory.furnace_pos = None;
    assert_eq!(
        super::retrieve_product(&mut scene).next,
        Some(TradeState::Base(CraftState::StartWorking))
    );
}

#[test]
fn a_non_furnace_recipe_hands_back_to_the_crafter() {
    // `:434-437` — `super.executeCraftingAction(toolSlot)`: the base class's rule,
    // called and not copied.
    let mut scene = glass_with_one_furnace();
    load_furnace_recipe(&mut scene, 4);
    facts_of(&mut scene).intermediate_is_furnace = false;
    scene.base.craft_counter = 1;
    let decision = super::execute_crafting_action(&mut scene);
    // `Execution::Again` — one more run, so the recipe is re-read (`:634-637`).
    assert_eq!(decision.next, Some(TradeState::Base(CraftState::GetRecipe)));
    assert_eq!(scene.base.progress, 0, "`:635`");
}

#[test]
fn the_fuel_request_is_the_stack_size_times_the_furnaces_and_waits_for_an_open_one() {
    // `:103-115` — `STACKSIZE * getFurnaces().size()`, minimum 1, and nothing asked
    // while a request is open.
    let mut scene = glass_with_one_furnace();
    load_furnace_recipe(&mut scene, 4);
    let facts = facts_of(&mut scene);
    facts.fuel_list = vec![NeedKey::plain("minecraft:coal")];

    match super::check_furnace_fuel(facts_of(&mut scene)) {
        Some(TradeAction::RequestFuel { size, min, fuels }) => {
            assert_eq!(size, STACKSIZE, "one furnace, one stack");
            assert_eq!(min, 1);
            assert_eq!(fuels, vec![NeedKey::plain("minecraft:coal")]);
        }
        other => panic!("expected a fuel request, got {other:?}"),
    }
    // An open request stops it.
    facts_of(&mut scene).open_fuel_request = true;
    assert_eq!(super::check_furnace_fuel(facts_of(&mut scene)), None);
    // So does any fuel at all, in the hand or in the hut (`:107-108`).
    let facts = facts_of(&mut scene);
    facts.open_fuel_request = false;
    facts.fuel_in_inventory = true;
    assert_eq!(super::check_furnace_fuel(facts), None);
    let facts = facts_of(&mut scene);
    facts.fuel_in_inventory = false;
    facts.fuel_in_building = 1;
    assert_eq!(super::check_furnace_fuel(facts), None);
}

#[test]
fn a_fuel_request_can_be_asked_with_an_empty_list() {
    // `getActivePossibleFuels` returns an empty list when the player declared no fuel
    // (`:711-717`), and `:107-109` does not test the list: Java asks for nothing.
    let mut scene = glass_with_one_furnace();
    load_furnace_recipe(&mut scene, 4);
    let facts = facts_of(&mut scene);
    assert!(facts.fuel_list.is_empty());
    match super::check_furnace_fuel(facts) {
        Some(TradeAction::RequestFuel { size, fuels, .. }) => {
            assert_eq!(size, STACKSIZE);
            assert!(fuels.is_empty(), "Java sends an empty StackList too");
        }
        other => panic!("expected the request Java creates, got {other:?}"),
    }
}

#[test]
fn add_fuel_asks_the_hut_when_the_hand_is_empty_and_fills_an_empty_slot() {
    // `:178-219`.
    let mut scene = glass_with_one_furnace();
    load_furnace_recipe(&mut scene, 4);
    let facts = facts_of(&mut scene);
    facts.fuel_list = vec![NeedKey::plain("minecraft:coal")];
    facts.fuel_in_building = 3;
    facts.fuel_in_inventory = false;

    let decision = super::add_fuel(&mut scene);
    assert_eq!(
        decision.next,
        Some(TradeState::Base(CraftState::GatheringRequiredMaterials)),
        "`:183-187`"
    );
    match decision.action {
        Some(TradeAction::RequestSmeltable { count, key }) => {
            assert_eq!(count, STACKSIZE, "`:185`");
            assert_eq!(key, NeedKey::plain("minecraft:coal"));
        }
        other => panic!("expected a fetch, got {other:?}"),
    }

    // With fuel in the hand and an empty slot, the transfer goes out (`:208-213`).
    scene.memory.furnace_pos = Some(Pos::new(1, 64, 1));
    let facts = facts_of(&mut scene);
    facts.fuel_in_inventory = true;
    facts.at_furnace = true;
    let decision = super::add_fuel(&mut scene);
    assert_eq!(decision.next, Some(TradeState::Base(CraftState::StartWorking)));
    match decision.action {
        Some(TradeAction::TransferFuel { count, .. }) => assert_eq!(count, STACKSIZE),
        other => panic!("expected a fuel transfer, got {other:?}"),
    }
    assert_eq!(scene.memory.furnace_pos, None, "`:217`");
}

#[test]
fn the_acceleration_skips_an_unlit_furnace_and_a_skill_under_ten() {
    // `:120-142` — `(secondaire / 10) * 2`, and only on a lit, loaded furnace.
    let mut scene = glass_with_one_furnace();
    let facts = facts_of(&mut scene);
    facts.furnaces = vec![
        Furnace::empty(Pos::new(0, 64, 0)),
        Furnace::empty(Pos::new(1, 64, 0)),
    ];
    facts.furnaces[0].lit = true;
    facts.furnaces[1].lit = false;

    assert_eq!(
        super::accelerate_furnaces(facts_of(&mut scene), 9),
        Vec::new(),
        "under ten there is nothing to do"
    );
    let actions = super::accelerate_furnaces(facts_of(&mut scene), 20);
    assert_eq!(actions.len(), 1, "only the lit furnace");
    assert_eq!(
        actions[0],
        TradeAction::Accelerate {
            at: Pos::new(0, 64, 0),
            ticks: 4
        }
    );
}

#[test]
fn the_background_pass_finds_a_leftover_and_forgets_a_missing_furnace() {
    // `:89-98` and `:595-599`.
    let mut scene = Scene::new(Trade::Glassblower);
    load_furnace_recipe(&mut scene, 4);
    let facts = facts_of(&mut scene);
    facts.furnaces = vec![
        Furnace::empty(Pos::new(0, 64, 0)),
        Furnace::empty(Pos::new(1, 64, 0)),
    ];
    facts.furnaces[1].smeltable = Some(FurnaceStack::new(NeedKey::plain("minecraft:cobblestone"), 1));

    let next = super::check_for_left_overs(&mut scene);
    assert_eq!(next, Some(TradeState::RetrieveUnrelatedProduct));
    assert_eq!(scene.memory.furnace_pos, Some(Pos::new(1, 64, 0)), "`:94`");

    // A position that is no longer a furnace is removed from the hut, and the pass
    // does not transition (`:595-599`).
    scene.memory.furnace_pos = None;
    let facts = facts_of(&mut scene);
    facts.furnaces[0].loaded = false;
    let next = super::check_for_left_overs(&mut scene);
    assert_eq!(next, None);
    assert_eq!(
        scene.action,
        Some(TradeAction::ForgetFurnace(Pos::new(0, 64, 0)))
    );
    assert_eq!(
        facts_of(&mut scene).furnaces.len(),
        1,
        "the hut forgets the position"
    );
}

#[test]
fn the_glassblower_machine_reaches_a_furnace_state_from_the_observation_only() {
    // The end-to-end shape: a task, a recipe in a furnace, one empty fuelled furnace,
    // fuel in the hand — and `CRAFT` becomes `FILL_UP_FURNACES` once the recipe's hits
    // are done.
    let mut ai = TradeAi::new(Trade::Glassblower);
    let mut scene = ai.scene();
    load_furnace_recipe(&mut scene, 8);
    scene.base.task_queue = 1;
    scene.base.current_task = true;
    scene.base.craft_counter = 0;
    let facts = facts_of(&mut scene);
    facts.furnaces = vec![Furnace::empty(Pos::new(1, 64, 1))];
    facts.fuel_list = vec![NeedKey::plain("minecraft:coal")]; // `:455`, `:698`
    facts.furnaces[0].fuel = Some(FurnaceStack::new(NeedKey::plain("minecraft:coal"), 8));
    facts.smeltable_in_inventory = 8;
    facts.at_furnace = true;

    // Thirty points in `Creativity` — the glassblower's **primary**, and *not* the skill
    // its recipe speed reads (`getCraftSpeedSkill()` is `Focus`,
    // `BuildingModules.java:320`): the recipe still wants thirty hits, the caller never
    // adds any, so the machine holds `CRAFT`.
    scene.skills = scene.skills.with(Skill::Creativity, 30);
    for _ in 0..(TICKS_SECOND * 2) {
        ai.tick(&mut scene);
    }
    assert_eq!(
        ai.state(),
        TradeState::Base(CraftState::Craft),
        "the primary skill is not the craft speed skill"
    );

    // The right skill, and the hits are free: `10 / (30 / 2 + 1) = 0`
    // (`AbstractEntityAICrafting.java:737`), so the first `CRAFT` tick executes.
    scene.skills = scene.skills.with(Skill::Focus, 30);
    let mut reached = None;
    let mut seen: Vec<(String, Option<TradeRefusal>)> = Vec::new();
    for _ in 0..(TICKS_SECOND * 4) {
        let outcome = ai.tick(&mut scene);
        seen.push((outcome.goal().java_name().to_string(), outcome.why_not()));
        if outcome.goal() == TradeState::FillUpFurnaces {
            reached = Some(outcome);
            break;
        }
    }
    let Some(outcome) = reached else {
        panic!("FILL_UP_FURNACES never reached; sequence was {seen:?}");
    };
    assert!(!outcome.faulted(), "no trap state");
    assert_eq!(
        outcome.why(),
        Some("AbstractEntityAICrafting.java:169"),
        "it was reached through the CRAFT target, whose rule delegates to `:427-500`"
    );
}
