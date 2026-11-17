//! Les scénarios de bout en bout : une suite de faits, un tick à la fois, et la suite
//! d'états que Java produirait sur la même scène.
//!
//! Les décisions sont testées une par une dans [`super::brewing`], [`super::furnace`]
//! et [`super::harvest`]. Ce fichier vérifie ce que ces tests ne peuvent pas voir : que
//! la machine **enchaîne** ces décisions dans l'ordre de Java, sans tomber dans le
//! piège de `BasicStateMachine.java:220-228` (un état que personne ne sait quitter) et
//! sans qu'une cible prenne le dessus sur une autre.

use crate::ai_build::crafter::{CraftFacts, CraftState, Input};
use crate::ai_work_01::facts::{
    Above, AlchemistFacts, BakerFacts, Furnace, NeedKey, Soil, Stack, TradeFacts,
};
use crate::ai_work_01::state::{craft_state, Scene};
use crate::ai_work_01::{Trade, WorkAi, WorkState, FUEL_SLOT};

/// Les buts successifs d'une scène, doublons consécutifs retirés : la suite qu'un
/// journal d'états de Java montrerait.
fn trace(ai: &mut WorkAi, scene: &mut Scene, ticks: usize) -> Vec<WorkState> {
    let mut states: Vec<WorkState> = Vec::new();
    for _ in 0..ticks {
        let outcome = ai.tick(scene);
        if states.last() != Some(&outcome.goal()) {
            states.push(outcome.goal());
        }
        assert!(
            ai.machine().missing().is_none(),
            "un état sans sortie a été atteint : {:?}",
            ai.machine().missing()
        );
    }
    states
}

fn busy() -> CraftFacts {
    CraftFacts {
        task_queue: 1,
        current_task: true,
        ..CraftFacts::default()
    }
}

/// `EntityAIWorkAlchemist.java:310-322` : sans tâche, l'alchimiste quitte `IDLE` (son
/// `hasWorkToDo` rend toujours vrai, `:1117-1121`), passe par `START_WORKING`, et le
/// tirage de `:314` l'envoie à la verrue.
#[test]
fn the_alchemist_walks_idle_to_the_wart_field() {
    let mut ai = WorkAi::new(Trade::Alchemist);
    let facts = AlchemistFacts {
        nether_wart_roll: 0,   // `:314` : `<= 1`, donc la verrue
        mistletoe_roll: 5,
        soil: vec![Soil {
            at: crate::ai::Pos::new(4, 60, 4),
            loaded: true,
            soul_sand: true,
            above: Above::NetherWart { age: 3 },
        }],
        ..AlchemistFacts::default()
    };
    let mut scene = Scene::new(CraftFacts::default(), TradeFacts::Alchemist(facts));
    let mut states: Vec<WorkState> = vec![ai.state()];
    let mut ticks = 0;
    while ai.state() != WorkState::HarvestNetherWart && ticks < 40 {
        let outcome = ai.tick(&mut scene);
        assert!(ai.machine().missing().is_none());
        if states.last() != Some(&outcome.goal()) {
            states.push(outcome.goal());
        }
        ticks += 1;
    }
    assert_eq!(ai.state(), WorkState::HarvestNetherWart);
    assert_eq!(
        states,
        vec![
            craft_state(CraftState::Idle),
            craft_state(CraftState::StartWorking),
            WorkState::HarvestNetherWart,
        ]
    );
    // Le tick suivant choisit la terre et en fait la cible de marche (`:150`).
    ai.tick(&mut scene);
    assert_eq!(scene.memory.walk_to, Some(crate::ai::Pos::new(4, 60, 4)));
}

/// `AbstractEntityAIRequestSmelter.java:427-503` : une recette de four, tout en main,
/// et le boulanger finit sur `FILL_UP_FURNACES` — sans jamais passer par `CRAFT` pour
/// la frapper.
#[test]
fn the_baker_walks_idle_to_the_furnaces() {
    let mut ai = WorkAi::new(Trade::Baker);
    let mut facts = BakerFacts {
        furnace_recipe: true,
        allowed_fuels: 1,
        furnaces: vec![Furnace::new(crate::ai::Pos::new(2, 64, 2))],
        ..BakerFacts::default()
    };
    facts.furnaces[0].slots[FUEL_SLOT] = Some(Stack::new(NeedKey::plain("minecraft:coal"), 8));
    let base = CraftFacts {
        has_recipe: true,
        has_request: true,
        progress: 30, // `work_time::craft_hits(0)`, donc le premier `CRAFT` exécute
        max_crafting_count: 8,
        request_count: 8,
        inputs: vec![Input::item(1, 0, 8)],
        ..busy()
    };
    let mut scene = Scene::new(base, TradeFacts::Baker(facts));
    let states = trace(&mut ai, &mut scene, 60);
    let head: Vec<WorkState> = states
        .iter()
        .copied()
        .take_while(|state| *state != WorkState::FillUpFurnaces)
        .collect();
    // `AbstractEntityAICrafting.java:311-314` : la recette **et** la requête sont
    // déjà en main, donc `getNextCraftingState` rend `QUERY_ITEMS` sans repasser par
    // `GET_RECIPE` — la recette ne se relit que quand `currentRequest` manque.
    assert_eq!(
        head,
        vec![
            craft_state(CraftState::Idle),
            craft_state(CraftState::StartWorking),
            craft_state(CraftState::QueryItems),
            craft_state(CraftState::Craft),
        ]
    );
    assert!(states.contains(&WorkState::FillUpFurnaces));
    assert_eq!(scene.memory.furnace_pos, Some(crate::ai::Pos::new(2, 64, 2)));
}

/// `EntityAIWorkAlchemist.java:395-435` puis `:733-794` : un alambic fini se vide, la
/// demande se termine, et le vidage de l'inventaire prend la main.
#[test]
fn the_alchemist_empties_a_finished_stand_then_dumps() {
    let mut ai = WorkAi::new(Trade::Alchemist);
    let mut facts = AlchemistFacts {
        brewing_recipe: true,
        ..AlchemistFacts::default()
    };
    let mut stand = crate::ai_work_01::facts::Stand::new(crate::ai::Pos::new(1, 64, 1));
    stand.slots[0] = Some(Stack::new(facts.output.clone(), 1));
    facts.stands.push(stand);
    let base = CraftFacts {
        task_queue: 1,
        current_task: true,
        request_count: 1,
        output_count: 1,
        max_crafting_count: 1,
        ..CraftFacts::default()
    };
    let mut scene = Scene::new(base, TradeFacts::Alchemist(facts));
    let states = trace(&mut ai, &mut scene, 60);
    let head: Vec<WorkState> = states.iter().copied().take(4).collect();
    assert_eq!(
        head,
        vec![
            craft_state(CraftState::Idle),
            craft_state(CraftState::StartWorking),
            craft_state(CraftState::GetRecipe),
            WorkState::RetrieveBrewed,
        ]
    );
    assert!(states.contains(&craft_state(CraftState::InventoryFull)));
    assert!(scene.base.craft_counter >= 1, "`:776` compte les pas servis");
}

/// `EntityAIWorkBlacksmith.java:1-33` : le forgeron n'a que la table commune. Sans
/// tâche il tourne entre `IDLE` et `START_WORKING` — et son `idle` de `:165` le tient.
#[test]
fn the_blacksmith_stays_on_the_shared_path() {
    let mut ai = WorkAi::new(Trade::Blacksmith);
    assert_eq!(ai.trade().own_states().len(), 0);
    let mut scene = Scene::new(CraftFacts::default(), TradeFacts::Blacksmith);
    let states = trace(&mut ai, &mut scene, 20);
    for state in &states {
        assert!(
            state.craft().is_some(),
            "un forgeron n'entre jamais dans un état de métier : {:?}",
            state
        );
    }
    assert_eq!(ai.state(), craft_state(CraftState::Idle));
}

/// Les trois métiers répondent à la même question « qui es-tu » avec leur hutte, et
/// chacun a sa propre table : aucun ne partage les états d'un autre.
#[test]
fn the_three_trades_have_distinct_tables_and_huts() {
    let mut seen: Vec<&'static str> = Vec::new();
    for trade in Trade::ALL {
        assert!(!seen.contains(&trade.building()));
        seen.push(trade.building());
        let ai = WorkAi::new(trade);
        for state in trade.own_states() {
            assert!(ai.machine().can_leave(*state), "{}", state.java());
        }
        // Les sept états communs sont dans les trois tables.
        for shared in [
            CraftState::Idle,
            CraftState::StartWorking,
            CraftState::QueryItems,
            CraftState::GetRecipe,
            CraftState::Craft,
        ] {
            assert!(ai.machine().can_leave(craft_state(shared)), "{}", trade);
        }
    }
    assert_eq!(Trade::Blacksmith.own_states().len(), 0);
}
