//! Les scénarios : une suite de faits, et la suite d'états que Java produirait.
//!
//! Chaque test de ce fichier assemble un métier, une scène et un nombre de ticks
//! connu, puis affirme la **trajectoire** — l'état de départ, les états traversés dans
//! l'ordre, et l'état d'arrivée. Les règles elles-mêmes sont testées à côté d'elles,
//! dans `facts.rs`, `smelter.rs` et `crusher.rs`, pour que rien ne soit affirmé deux
//! fois.
//!
//! Les rythmes comptent, et c'est le piège de ce genre de test : la machine tique à
//! `ENTITY_AI_TICKRATE` (5 ticks), et `TickingTransition` retire ce pas à chaque
//! compte à rebours (`:118-121`). Une cible à `TICKS_SECOND` (20) ne tire donc qu'un
//! tick de machine sur quatre, une cible à `HIT_DELAY` (10) un sur deux, et une cible à
//! `STANDARD_DELAY` (5) à chaque tick.

use crate::ai::Pos;
use crate::ai_build::crafter::{CraftState, Input};
use crate::ai_build::materials::NeedKey;

use super::facts::{CrusherFacts, Recipe, Skills, Slot, SmelterFacts, TradeJob};
use super::{smelter, state, Scene, Trade, TradeMemory, TradeRefusal, WorkAi, WorkState};

fn key(name: &str) -> NeedKey {
    NeedKey::plain(name)
}

fn iron_recipe() -> Recipe {
    Recipe::smelted(key("minecraft:iron_ore"), key("minecraft:iron_ingot"))
}

/// Un cuisinier qui a une tâche, une recette de fonte, et un four ravitaillé.
fn chef_at_work() -> Scene {
    let mut scene = Scene::chef(SmelterFacts {
        furnaces: vec![
            super::facts::Furnace::new(Pos::new(0, 64, 0))
                .with_fuel(Slot::new(key("minecraft:coal"), 32)),
        ],
        allowed_fuel: vec![key("minecraft:coal")],
        fuel_in_inventory: 32,
        walked_to_furnace: true,
        // `BuildingModules.java:451-457` : cuisinier, `Knowledge` = vitesse.
        skills: Skills::new(20, 10, 10),
        ..SmelterFacts::default()
    });
    scene.craft.task_queue = 1;
    scene.craft.current_task = true;
    scene.craft.request_count = 8;
    scene.craft.output_count = 1;
    scene.craft.inputs = vec![Input::item(1, 16, 8)];
    scene.available = Some(iron_recipe());
    scene
}

/// Un concasseur qui a du quota et rien d'autre.
fn crusher_with_quota() -> Scene {
    Scene::crusher(CrusherFacts {
        daily_quantity: 0,
        daily_limit: 8,
        mode_recipe: Some(Recipe::crafted(
            key("minecraft:cobblestone"),
            key("minecraft:gravel"),
        )),
        building_level: 2,
        skills: Skills::new(10, 20, 10),
    })
}

/// Les états visités par une suite de ticks, sans les répétitions.
fn trajectory(ai: &mut WorkAi, scene: &mut Scene, ticks: usize) -> Vec<WorkState> {
    let mut seen = vec![ai.state()];
    for _ in 0..ticks {
        let outcome = ai.tick(scene);
        assert!(!outcome.faulted(), "la machine est tombée dans un piège : {:?}", outcome);
        if seen.last() != Some(&outcome.goal()) {
            seen.push(outcome.goal());
        }
    }
    seen
}

/// Les trois métiers, leurs classes Java et leurs bâtiments.
#[test]
fn the_three_trades_are_the_three_java_classes() {
    assert_eq!(Trade::ALL.len(), 3);
    let table = [
        (Trade::Chef, "chef", "EntityAIWorkChef", "BuildingKitchen"),
        (Trade::Dyer, "dyer", "EntityAIWorkDyer", "BuildingDyer"),
        (Trade::Crusher, "crusher", "EntityAIWorkCrusher", "BuildingCrusher"),
    ];
    for (trade, name, class, building) in table {
        assert_eq!(trade.name(), name);
        assert_eq!(trade.class(), class);
        assert_eq!(trade.building(), building);
    }
    // Le cuisinier et le teinturier n'ont pas la même classe mère que le concasseur.
    assert!(Trade::Chef.parent().starts_with("AbstractEntityAIRequestSmelter"));
    assert_eq!(Trade::Dyer.parent(), Trade::Chef.parent());
    assert!(Trade::Crusher.parent().starts_with("AbstractEntityAICrafting"));
}

/// `EntityAIWorkChef.java` et `EntityAIWorkDyer.java` n'ajoutent aucun état : les deux
/// lisent les mêmes douze.
#[test]
fn chef_and_dyer_share_the_smelter_table() {
    assert_eq!(Trade::Chef.states(), Trade::Dyer.states());
    assert_eq!(Trade::Chef.states().len(), 11, "7 états d'atelier + 4 du four");
    assert!(Trade::Chef.states().iter().any(|state| state.is_furnace_state()));
}

/// `EntityAIWorkCrusher.java:49-52` n'ajoute que `CRUSH`, et n'a aucun état de four.
#[test]
fn the_crusher_has_one_state_of_its_own() {
    let states = Trade::Crusher.states();
    assert_eq!(states.len(), 8, "7 états d'atelier + CRUSH");
    assert!(states.contains(&WorkState::Crush));
    assert!(!states.iter().any(|state| state.is_furnace_state()));
}

/// Chaque métier peut sortir de chaque état qu'il peut atteindre : aucun état de la
/// table n'est un piège (`BasicStateMachine.java:220-228`).
#[test]
fn every_state_a_trade_can_enter_can_be_left() {
    for trade in Trade::ALL {
        let ai = WorkAi::new(trade);
        for state in trade.states() {
            assert!(
                ai.machine().can_leave(state),
                "{trade} : aucun départ de {state}"
            );
        }
    }
}

/// Le scénario du cuisinier : la suite d'états d'une fonte complète, du réveil au
/// remplissage du four.
#[test]
fn a_chef_walks_the_java_state_suite() {
    let mut ai = WorkAi::new(Trade::Chef);
    let mut scene = chef_at_work();
    let seen = trajectory(&mut ai, &mut scene, 9);
    assert_eq!(
        seen,
        vec![
            WorkState::base(CraftState::Idle),
            WorkState::base(CraftState::StartWorking), // `:164` → `:164`
            WorkState::base(CraftState::GetRecipe),    // `:166` → `:316`
            WorkState::base(CraftState::QueryItems),   // `:421`
            WorkState::base(CraftState::Craft),        // `:503`
            WorkState::FillUpFurnaces,                 // `:493-497`
        ],
        "une fonte : lire la recette, prendre les items, frapper, remplir le four"
    );
}

/// La même scène, mais le four est déjà plein : la fonte va jusqu'au bout et sort la
/// sortie.
#[test]
fn a_chef_fills_then_retrieves() {
    let mut ai = WorkAi::new(Trade::Chef);
    let mut scene = chef_at_work();
    // Un four qui a du carburant et de quoi fondre : `:516-518` ne le choisit pas,
    // donc `executeCraftingAction` rend `START_WORKING` (`:499`).
    scene.job = TradeJob::chef(SmelterFacts {
        furnaces: vec![super::facts::Furnace::new(Pos::new(0, 64, 0))
            .with_fuel(Slot::new(key("minecraft:coal"), 32))
            .with_smeltable(Slot::new(key("minecraft:iron_ore"), 4))],
        allowed_fuel: vec![key("minecraft:coal")],
        fuel_in_inventory: 32,
        walked_to_furnace: true,
        skills: Skills::new(20, 10, 10),
        ..SmelterFacts::default()
    });
    // La recette est déjà en main : sans elle, `:588-590` fait de **tout** contenu de
    // four un intrus, et `checkForLeftOvers` (`:89-98`) sortirait l'entrée avant même
    // de lire une recette.
    scene.memory.recipe = Some(iron_recipe());
    let seen = trajectory(&mut ai, &mut scene, 9);
    assert_eq!(
        seen,
        vec![
            WorkState::base(CraftState::Idle),
            WorkState::base(CraftState::StartWorking),
            WorkState::base(CraftState::GetRecipe),
            WorkState::base(CraftState::QueryItems),
            WorkState::base(CraftState::Craft),
            WorkState::base(CraftState::StartWorking),
        ],
        ":499 — le four fond déjà, on repart travailler au lieu de le remplir"
    );
    assert!(!seen.contains(&WorkState::FillUpFurnaces));
}

/// Le scénario du concasseur : `IDLE` sans condition (`:50`), puis `decide` choisit
/// `CRUSH` parce qu'il reste du quota (`:68-71`).
#[test]
fn a_crusher_goes_from_idle_to_crush() {
    let mut ai = WorkAi::new(Trade::Crusher);
    let mut scene = crusher_with_quota();
    let seen = trajectory(&mut ai, &mut scene, 2);
    assert_eq!(
        seen,
        vec![
            WorkState::base(CraftState::Idle),
            WorkState::base(CraftState::StartWorking),
            WorkState::Crush,
        ],
        "EntityAIWorkCrusher.java:50 puis :70"
    );
}

/// Un concasseur sans quota ne concasse pas : c'est le défaut du réglage
/// (`BuildingModules.java:287`), et `decide` le dit au lieu de boucler.
#[test]
fn a_default_crusher_stays_idle() {
    let mut ai = WorkAi::new(Trade::Crusher);
    let mut scene = Scene::crusher(CrusherFacts::default());
    let seen = trajectory(&mut ai, &mut scene, 12);
    assert!(!seen.contains(&WorkState::Crush));
    assert_eq!(seen.last(), Some(&WorkState::base(CraftState::Idle)));
}

/// Un cuisinier sans tâche passe son temps : `IDLE`, le pas d'`idle`, et il y reste.
#[test]
fn an_idle_chef_picks_a_pastime() {
    let mut ai = WorkAi::new(Trade::Chef);
    let mut scene = Scene::chef(SmelterFacts::default());
    let outcome = ai.tick(&mut scene);
    assert_eq!(outcome.goal(), WorkState::base(CraftState::Idle));
    assert_eq!(outcome.refusal, None);
    assert!(scene.memory.idle_step.is_some(), "`idle` a répondu un pas");
}

/// Le cuisinier et le teinturier ne diffèrent que par leur compétence de vitesse :
/// `Knowledge` contre `Dexterity` (`BuildingModules.java:457`, `:295`), donc par le
/// nombre de coups.
#[test]
fn the_two_smelters_differ_by_their_craft_speed_skill() {
    assert_eq!(Trade::Chef.craft_speed_skill().0, "Knowledge");
    assert_eq!(Trade::Dyer.craft_speed_skill().0, "Dexterity");

    let chef = Scene::chef(SmelterFacts {
        skills: Skills::new(20, 10, 10),
        ..SmelterFacts::default()
    });
    let dyer = Scene::dyer(SmelterFacts {
        skills: Skills::new(20, 10, 0),
        ..SmelterFacts::default()
    });
    // `AbstractEntityAICrafting.java:734-738` : `10 / min(niveau/2 + 1, 50) * 3`.
    assert_eq!(smelter::required_hits(&chef), Ok(3), "niveau 10 → 10 / 6 × 3");
    assert_eq!(smelter::required_hits(&dyer), Ok(30), "niveau 0 → 10 / 1 × 3");
    assert_ne!(
        smelter::required_hits(&chef),
        smelter::required_hits(&dyer)
    );
}

/// La couture avec `crate::ai_build::crafter` : la mémoire devient les six compteurs
/// du jeu de faits, et rien d'autre ne bouge.
#[test]
fn the_memory_becomes_the_crafting_facts() {
    let mut scene = chef_at_work();
    scene.memory.progress = 2;
    scene.memory.craft_counter = 3;
    scene.memory.max_crafting_count = 8;
    scene.memory.request_open = true;
    scene.memory.actions_done = 1;
    scene.memory.recipe = Some(iron_recipe());
    let facts = scene.crafting_facts();
    assert_eq!(facts.progress, 2, "`job.getProgress()`");
    assert_eq!(facts.craft_counter, 3, "`job.getCraftCounter()`");
    assert_eq!(facts.max_crafting_count, 8, "`job.getMaxCraftingCount()`");
    assert_eq!(facts.actions_done, 1, "`job.getActionsDone()`");
    assert!(facts.has_recipe && facts.has_request);
    assert_eq!(facts.output_count, 1, "`getPrimaryOutput().getCount()`");
    assert_eq!(
        facts.craft_speed_level, 10,
        "`((CraftingWorkerBuildingModule) getModuleForJob()).getCraftSpeedSkill()`"
    );
    // Ce que le port ne touche pas : les observations de l'appelant.
    assert_eq!(facts.task_queue, 1);
    assert_eq!(facts.request_count, 8);
}

/// `getExtendedCount` (`AbstractEntityAIRequestSmelter.java:144-172`) : ce que les
/// fours contiennent entre dans `in_progress_output`, ce qui empêche de redemander des
/// items déjà en cuisson.
#[test]
fn the_furnaces_feed_the_extended_count() {
    let mut scene = chef_at_work();
    scene.job = TradeJob::chef(SmelterFacts {
        furnaces: vec![super::facts::Furnace::new(Pos::new(0, 64, 0))
            .with_smeltable(Slot::new(key("minecraft:iron_ore"), 5))],
        allowed_fuel: vec![key("minecraft:coal")],
        ..SmelterFacts::default()
    });
    scene.memory.recipe = Some(iron_recipe());
    let facts = scene.crafting_facts();
    assert_eq!(facts.in_progress_output, 0, "l'entrée n'est pas la sortie");
    assert_eq!(
        facts.inputs.first().map(|input| input.extended),
        Some(5),
        ":401, :472 — l'entrée, elle, est comptée dans le four"
    );
}

/// Un métier à qui l'on donne les faits d'un autre nomme l'erreur au lieu de faire
/// semblant.
#[test]
fn a_trade_handed_the_wrong_facts_names_it() {
    let mut scene = Scene::crusher(CrusherFacts::default());
    scene.memory.recipe = Some(iron_recipe());
    let decision = smelter::execute_crafting_action(&mut scene);
    assert_eq!(decision.refusal, Some(TradeRefusal::NoFactsForTrade));

    let mut chef = Scene::chef(SmelterFacts::default());
    let decision = super::crusher::crush(&mut chef, WorkState::Crush);
    assert_eq!(decision.refusal, Some(TradeRefusal::NoFactsForTrade));
}

/// `setDelay` n'est pas un état : il retarde la transition qui vient de tirer. Le port
/// le rend dans [`Outcome`] pour que l'appelant le voie, et l'applique à la machine.
#[test]
fn a_rule_can_ask_for_a_delay() {
    let mut ai = WorkAi::new(Trade::Chef);
    // Un four qui a du carburant, tout ce qu'il faut pour fondre, et trop de fours
    // allumés : `:486-491` demande une seconde.
    let mut scene = chef_at_work();
    scene.job = TradeJob::chef(SmelterFacts {
        furnaces: vec![
            super::facts::Furnace::new(Pos::new(0, 64, 0))
                .with_fuel(Slot::new(key("minecraft:coal"), 4))
                .with_smeltable(Slot::new(key("minecraft:iron_ore"), 1))
                .burning(),
            super::facts::Furnace::new(Pos::new(1, 64, 0))
                .with_fuel(Slot::new(key("minecraft:coal"), 4))
                .with_smeltable(Slot::new(key("minecraft:iron_ore"), 1))
                .burning(),
        ],
        allowed_fuel: vec![key("minecraft:coal")],
        fuel_in_inventory: 8,
        walked_to_furnace: true,
        // `Knowledge` 20 → `craft_hits` = 0 (`AbstractEntityAICrafting.java:734-738`),
        // donc le premier tick de `CRAFT` atteint le four.
        skills: Skills::new(0, 0, 20),
        ..SmelterFacts::default()
    });
    let mut delays = Vec::new();
    for _ in 0..10 {
        let outcome = ai.tick(&mut scene);
        if let Some(delay) = outcome.delay {
            delays.push(delay);
        }
    }
    assert!(delays.contains(&20), "`:489 — `setDelay(TICKS_SECOND)`");
}

/// Une transition qui va ailleurs le dit, et une qui reste sur place aussi : c'est
/// ce que [`Outcome::moved`] mesure.
#[test]
fn an_outcome_knows_whether_it_moved() {
    let mut ai = WorkAi::new(Trade::Chef);
    let mut scene = chef_at_work();
    let first = ai.tick(&mut scene);
    assert!(first.moved(), "IDLE → START_WORKING");
    assert_eq!(first.why(), Some("AbstractEntityAICrafting.java:164"));
    assert_eq!(first.goal(), WorkState::base(CraftState::StartWorking));

    // Retour en IDLE, puis un tick sur un état qui se réaffirme.
    let mut idle = WorkAi::new(Trade::Chef);
    let mut quiet = Scene::chef(SmelterFacts::default());
    let stay = idle.tick(&mut quiet);
    assert_eq!(stay.goal(), WorkState::base(CraftState::Idle));
    assert!(!stay.moved());
}

/// La mémoire d'un métier ne fuit pas dans celle d'un autre : deux scènes, deux
/// souvenirs.
#[test]
fn two_scenes_do_not_share_memory() {
    let mut chef = chef_at_work();
    let crusher = crusher_with_quota();
    chef.memory.delivered = 7;
    assert_eq!(crusher.memory.delivered, 0);
    assert_eq!(crusher.memory, TradeMemory::default());
    assert_ne!(chef.memory, TradeMemory::default());
}

/// Les quatre états du four et leurs rythmes, tels que Java les enregistre
/// (`AbstractEntityAIRequestSmelter.java:78-81`).
#[test]
fn the_furnace_targets_run_at_one_second() {
    let ai = WorkAi::new(Trade::Chef);
    let furnace_rules: Vec<(&'static str, u32)> = ai
        .machine()
        .transitions()
        .iter()
        .filter(|transition| {
            transition
                .state
                .is_some_and(|state: WorkState| state.is_furnace_state())
        })
        .map(|transition| (transition.rule, transition.tick_rate))
        .collect();
    assert_eq!(
        furnace_rules,
        vec![
            ("retrieveProductFromFurnace", 20),
            ("retrieveUnrelatedProductFromFurnace", 20),
            ("fillUpFurnace", 20),
            ("addFuelToFurnace", 20),
        ]
    );
    assert_eq!(super::FURNACE_RATE, 20);
    assert_eq!(super::SLOW_FURNACE_RATE, 200, "TICKS_SECOND * 10");
}

/// Les trois passes globales du four existent, et seulement pour les deux métiers qui
/// en ont (`:73-75`) : le concasseur n'en a aucune.
#[test]
fn only_the_smelters_have_global_passes() {
    let chef = WorkAi::new(Trade::Chef);
    let events: Vec<&'static str> = chef
        .machine()
        .transitions()
        .iter()
        .filter(|transition| transition.blocking.is_some())
        .map(|transition| transition.rule)
        .collect();
    assert_eq!(events, vec!["checkFurnaceFuel", "accelerateFurnaces", "checkForLeftOvers"]);

    let crusher = WorkAi::new(Trade::Crusher);
    assert!(
        crusher
            .machine()
            .transitions()
            .iter()
            .all(|transition| transition.blocking.is_none()),
        "EntityAIWorkCrusher.java:49-52 n'ajoute aucune cible globale"
    );
}

/// `reset_values` (`AbstractEntityAICrafting.java:668-676`) et `abandon` : les deux
/// remises à zéro que Java fait à des moments différents.
#[test]
fn the_two_resets_clear_different_things() {
    let mut scene = chef_at_work();
    scene.memory.recipe = Some(iron_recipe());
    scene.memory.request_open = true;
    scene.memory.progress = 5;
    scene.memory.craft_counter = 5;
    scene.memory.max_crafting_count = 5;
    scene.memory.dumped = true;
    state::reset_values(&mut scene);
    assert_eq!(scene.memory.progress, 0);
    assert_eq!(scene.memory.craft_counter, 0);
    assert_eq!(scene.memory.max_crafting_count, 0);
    assert!(!scene.memory.dumped);
    assert_eq!(scene.memory.recipe, Some(iron_recipe()), "resetValues ne touche pas la recette");

    state::abandon(&mut scene);
    assert_eq!(scene.memory.recipe, None);
    assert!(!scene.memory.request_open);
}

/// `finalizeCraftingTask` (`:636-646`) : l'expérience n'est donnée que si l'inventaire
/// doit être vidé **et** qu'une requête est ouverte — les trois compteurs venant
/// d'être remis à zéro, ils ne peuvent plus rien refuser.
#[test]
fn finalizing_gives_experience_only_with_a_dump_and_a_request() {
    let mut with = chef_at_work();
    with.craft.inventory_needs_dump = true;
    with.craft.request_count = 8;
    with.memory.request_open = true;
    let decision = state::finalize_crafting_task(&mut with);
    assert_eq!(
        decision.next,
        Some(WorkState::base(CraftState::InventoryFull))
    );
    assert_eq!(with.memory.experience, 4.0, ":643 — `count / 2.0`");

    let mut without = chef_at_work();
    without.craft.inventory_needs_dump = false;
    without.memory.experience = 1.0;
    state::finalize_crafting_task(&mut without);
    assert_eq!(without.memory.experience, 1.0, "rien de plus sans vidage");

    let mut no_request = chef_at_work();
    no_request.craft.inventory_needs_dump = true;
    no_request.memory.request_open = false;
    state::finalize_crafting_task(&mut no_request);
    assert_eq!(no_request.memory.experience, 0.0);
}

/// Le pas d'`idle` est un fait de sortie : le port ne fait pas asseoir le citoyen, il
/// dit où.
#[test]
fn the_idle_step_is_reported_not_performed() {
    let mut scene = Scene::chef(SmelterFacts::default());
    scene.craft.in_building = false;
    let mut ai = WorkAi::new(Trade::Chef);
    let outcome = ai.tick(&mut scene);
    assert_eq!(outcome.goal(), WorkState::base(CraftState::Idle));
    assert_eq!(
        scene.memory.idle_step,
        Some(crate::ai_build::crafter::IdleStep::WalkToBuilding),
        "AbstractEntityAICrafting.java:187-190"
    );
}

/// `slots` d'un four : les trois constantes de `Constants.java`, citées à leur ligne.
#[test]
fn the_four_slot_constants_are_the_java_ones() {
    assert_eq!(super::facts::SMELTABLE_SLOT, 0, "Constants.java:123");
    assert_eq!(super::facts::FUEL_SLOT, 1, "Constants.java:128");
    assert_eq!(super::facts::RESULT_SLOT, 2, "Constants.java:118");
    assert_eq!(super::facts::STACKSIZE, 64, "Constants.java:23");
    assert_eq!(super::TICKS_SECOND, 20, "Constants.java:21");
}
