//! Les scénarios : des faits, des ticks, et la suite d'états que le Java produit.
//!
//! Le test qui compte pour ce module n'est pas « la fonction rend 3 » — c'est « avec ces
//! observations, l'IA passe par ces états-là, dans cet ordre, et s'arrête pour cette
//! raison-là ». Chaque scénario nomme donc les états successifs, et deux d'entre eux
//! comparent la table de ce module à celle du module voisin qui porte la même règle
//! (`crate::ai_work_09::herder` pour l'élevage, `crate::ai_build::delivery` pour le
//! livreur) : c'est la preuve qu'aucune règle n'a été réécrite avec une variante.

use crate::ai::facts::{Pos, WorldFacts};
use crate::ai_build::delivery::{DeliveryFacts, DeliveryState, Task};
use crate::ai_work_04::facts::Furnace;
use crate::ai_work_04::states::SmeltState;
use crate::ai_work_09::facts::{HerdAnimal, HerdFacts, HerderDraws};

use super::facts::{
    CitizenHere, CookFacts, CookMemory, FoodStack, Scene, ServeCitizen, ServePlayer, SwineFacts,
    TradeJob, TradeMemory,
};
use super::state::{Refusal, TradeState};
use super::{dump_due, Decision, Effect, Trade, TradeAi, TradeOutcome};

/// Ce qu'une suite de ticks a produit : le dernier résultat, et tout ce qui a été décidé
/// ou refusé en chemin.
///
/// Un scénario a besoin des trois : l'état final ne dit pas qu'un citoyen a été servi au
/// troisième tick, et une quantité décidée ne se relit nulle part.
struct Run {
    last: TradeOutcome,
    effects: Vec<Effect>,
    refusals: Vec<Refusal>,
}

/// Faire tourner la machine `n` ticks (`n >= 1`) et ramasser ce qui s'est passé.
fn run(ai: &mut TradeAi, scene: &mut Scene, n: u32) -> Run {
    let mut last = ai.tick(scene);
    let mut effects = Vec::new();
    let mut refusals = Vec::new();
    if let Some(effect) = last.effect() {
        effects.push(effect);
    }
    if let Some(refusal) = last.why_not() {
        refusals.push(refusal);
    }
    for _ in 1..n {
        last = ai.tick(scene);
        if let Some(effect) = last.effect() {
            effects.push(effect);
        }
        if let Some(refusal) = last.why_not() {
            refusals.push(refusal);
        }
    }
    Run {
        last,
        effects,
        refusals,
    }
}

/// Un troupeau de `n` adultes alignés le long de X, dans l'enclos de la hutte.
fn herd_of(n: u32) -> Vec<HerdAnimal> {
    (0..n)
        .map(|index| HerdAnimal {
            id: index,
            at: Pos::new(index as i32, 64, 0),
            age: 0,
            baby: false,
            in_love: false,
            can_fall_in_love: true,
            alive: true,
            can_see_sky: true,
            can_mate: true,
        })
        .collect()
}

/// Les faits d'élevage d'un enclos servable : un module, des bêtes accouplables, la
/// carotte en poche, et **les trois tirages au-dessus de leur seuil** pour qu'aucun
/// ramassage, abattage ou nourrissage ne vienne voler l'accouplement.
fn herd_facts(animals: Vec<HerdAnimal>) -> HerdFacts {
    HerdFacts {
        building_level: 1,
        breeding_setting: true,
        has_module: true,
        animals,
        items_on_ground: 0,
        first_item: None,
        has_axe: true,
        breeding_item_count: 4,
        breeding_item_equipped: true,
        killed: false,
        chicken_draw: 0,
        game_time: 0,
        primary_skill: 0,
        secondary_skill: 0,
        main_hand_empty: false,
        draws: HerderDraws {
            pickup: 0.9,
            butcher: 0.9,
            feed: 0.9,
        },
    }
}

/// Un monde où le citoyen est à sa hutte, à l'origine.
fn world_at_hut() -> WorldFacts {
    let mut facts = WorldFacts::new();
    facts.building.position = Pos::new(0, 64, 0);
    facts.worker.at = Pos::new(0, 64, 0);
    facts
}

/// Un cuisinier dans sa hutte, avec un four vide et un menu non vide.
fn cook_facts() -> CookFacts {
    let mut facts = CookFacts::new();
    facts.furnace.at_building = true;
    facts
}

// ------------------------------------------------------------------------ éleveur

#[test]
fn the_pig_walks_idle_preparing_decide_breed_then_idle() {
    // `AbstractEntityAIHerder.java:140-148` : les cinq états d'un accouplement, dans
    // l'ordre du Java, avec un troupeau de deux bêtes à portée de la hutte.
    let mut ai = TradeAi::new(Trade::SwineHerder);
    let mut scene = ai.scene_with(
        world_at_hut(),
        TradeJob::Swine(SwineFacts::new(herd_facts(herd_of(2)))),
    );

    // `:140` `IDLE` → `START_WORKING`.
    assert_eq!(
        run(&mut ai, &mut scene, 1).last.goal().java_name(),
        "START_WORKING"
    );
    // `:141` `startWorkingAtOwnBuilding` : il est à la hutte, donc `PREPARING`.
    assert_eq!(
        run(&mut ai, &mut scene, 1).last.goal().java_name(),
        "PREPARING"
    );
    // `:142` `prepareForHerding` : aucun module courant encore, donc `DECIDE`.
    assert_eq!(
        run(&mut ai, &mut scene, 1).last.goal().java_name(),
        "DECIDE"
    );
    // `:143` `decideWhatToDo` : deux bêtes accouplables, l'item en main, pas de délai.
    assert_eq!(
        run(&mut ai, &mut scene, 1).last.goal().java_name(),
        "HERDER_BREED"
    );
    // `:144` `breedAnimals` : les deux partenaires sont à portée, `breedTwoAnimals` les
    // consomme dans le même tick et le Java rend `IDLE` (`:481-483`).
    let bred = run(&mut ai, &mut scene, 1);
    assert_eq!(bred.last.goal().java_name(), "IDLE");
    assert_eq!(
        scene.memory.herd.breed_time_out, 1200,
        "`:481` soixante secondes"
    );
    assert!(scene.memory.herd.animals_to_breed.is_empty());
    assert!(ai.missing().is_none(), "aucun piège en chemin");
}

#[test]
fn four_adults_make_the_pig_butcher_instead_of_breeding() {
    // `chanceToButcher` (`:673-696`) : niveau 1 → `maxAnimals = 2`, quatre adultes →
    // `0,5 × 4⁴ / 2⁴ = 8`, donc un tirage de 0,5 passe large et `:234` gagne avant le
    // test d'accouplement de `:238`.
    let mut facts = herd_facts(herd_of(4));
    facts.draws.butcher = 0.5;
    let mut ai = TradeAi::new(Trade::SwineHerder);
    let mut scene = ai.scene_with(world_at_hut(), TradeJob::Swine(SwineFacts::new(facts)));
    // IDLE, START_WORKING, PREPARING, puis `DECIDE` choisit `HERDER_BUTCHER`.
    let _ = run(&mut ai, &mut scene, 3);
    assert_eq!(
        run(&mut ai, &mut scene, 1).last.goal().java_name(),
        "HERDER_BUTCHER"
    );
    // Premier passage de `butcherAnimals` : la cible est le plus éloigné du centre, donc
    // la bête de x = 3, et le citoyen est resté à la hutte — il marche (`:622-632`).
    let walking = run(&mut ai, &mut scene, 1);
    assert_eq!(walking.last.goal().java_name(), "HERDER_BUTCHER");
    assert_eq!(
        walking.refusals,
        vec![Refusal::Herd(
            crate::ai_work_09::state::WorkRefusal::NotAtAnimal
        )]
    );
    // À portée et la bête morte, le coup du porc porte toujours (`:805-838`, pas de tirage
    // comme chez la poule) : `BUTCHER_DELAY = 20` demande quatre ticks de machine.
    scene.facts.worker.at = Pos::new(3, 64, 0);
    if let TradeJob::Swine(swine) = &mut scene.job {
        swine.herd.killed = true;
    }
    let struck = run(&mut ai, &mut scene, 4);
    assert_eq!(
        struck.last.goal().java_name(),
        "DECIDE",
        "`:379-387` : la bête est morte, il retourne décider"
    );
    assert_eq!(scene.facts.inventory.actions_done, 1, "`:383`");
}

#[test]
fn a_pig_that_has_not_arrived_yet_keeps_walking() {
    // `walkingToAnimal` (`:622-632`) : à plus de deux blocs, le Java rend `getState()`.
    let mut facts = herd_facts(herd_of(4));
    facts.draws.butcher = 0.5;
    let mut ai = TradeAi::new(Trade::SwineHerder);
    let mut scene = ai.scene_with(world_at_hut(), TradeJob::Swine(SwineFacts::new(facts)));
    let _ = run(&mut ai, &mut scene, 4); // `IDLE` → … → `HERDER_BUTCHER`
    let walking = run(&mut ai, &mut scene, 1); // premier passage de `butcherAnimals`
    assert_eq!(walking.last.goal().java_name(), "HERDER_BUTCHER");
    assert_eq!(
        walking.refusals,
        vec![Refusal::Herd(
            crate::ai_work_09::state::WorkRefusal::NotAtAnimal
        )]
    );
    assert_eq!(scene.facts.inventory.actions_done, 0, "rien n'est compté");
}

#[test]
fn the_swine_table_answers_like_the_herder_table() {
    // **Le test de non-duplication.** La table du porc est celle
    // d'`AbstractEntityAIHerder` ; `crate::ai_work_09::WorkAi` porte déjà cette table pour
    // l'éleveur de poulets. Les deux machines, sur les mêmes faits, doivent produire la
    // même suite d'états — sinon ce module a réécrit une règle au lieu de l'appeler.
    let world = world_at_hut();
    let facts = herd_facts(herd_of(2));
    let mut mine = TradeAi::new(Trade::SwineHerder);
    let mut mine_scene = mine.scene_with(
        world.clone(),
        TradeJob::Swine(SwineFacts::new(facts.clone())),
    );
    let mut theirs = crate::ai_work_09::WorkAi::new(crate::ai_work_09::WorkTrade::ChickenHerder);
    let mut their_scene =
        theirs.scene_with(world, crate::ai_work_09::WorkJob::ChickenHerder(facts));
    for tick in 0..8 {
        let left = mine.tick(&mut mine_scene);
        let right = theirs.tick(&mut their_scene);
        assert_eq!(
            left.goal().java_name(),
            right.goal().java_name(),
            "les deux tables divergent au tick {tick}"
        );
    }
    // Le second tour a commencé : le troupeau s'est accouplé au tick 5, et le congé de
    // soixante secondes de `:481` n'a pas expiré.
    assert_eq!(mine.state().java_name(), "START_WORKING");
    assert_eq!(
        mine_scene.memory.herd.breed_time_out, 1200,
        "les deux mémoires disent la même chose"
    );
}

// ------------------------------------------------------------------------ cuisinier

#[test]
fn the_cook_serves_a_hungry_citizen_then_goes_back_to_the_furnace() {
    // `EntityAIWorkCook.java:96` puis `:165-249` puis `:392-395`.
    let mut facts = cook_facts();
    facts.citizens = vec![CitizenHere::hungry(4)];
    let mut ai = TradeAi::new(Trade::Cook);
    let mut scene = ai.scene_with(world_at_hut(), TradeJob::Cook(facts));

    // `:77` `IDLE` → `START_WORKING`.
    assert_eq!(
        run(&mut ai, &mut scene, 1).last.goal().java_name(),
        "START_WORKING"
    );
    // `:78` `startWorking` → `checkForImportantJobs` → le citoyen a faim et le cuisinier a
    // son plat : `COOK_SERVE_FOOD_TO_CITIZEN` (`:394`).
    let queued = run(&mut ai, &mut scene, 1);
    assert_eq!(queued.last.goal().java_name(), "COOK_SERVE_FOOD_TO_CITIZEN");
    assert_eq!(scene.memory.cook.citizens_to_serve, vec![4]);

    // `:96` le citoyen n'est pas encore à portée : refus nommé, la file est intacte.
    if let TradeJob::Cook(cook) = &mut scene.job {
        cook.citizen_target = Some(ServeCitizen {
            at_work_pos: false,
            ..ServeCitizen::ready(4, 5.0)
        });
    }
    let walking = run(&mut ai, &mut scene, 1);
    assert_eq!(
        walking.last.goal().java_name(),
        "COOK_SERVE_FOOD_TO_CITIZEN"
    );
    assert_eq!(walking.refusals, vec![Refusal::NotAtServeTarget]);

    // À portée : la portion part. La cible a un délai de 30 ticks (`:64`), donc on laisse
    // tourner le compte à rebours ; seul le tick qui tire compte.
    if let TradeJob::Cook(cook) = &mut scene.job {
        cook.citizen_target = Some(ServeCitizen::ready(4, 5.0));
    }
    let served = run(&mut ai, &mut scene, 8);
    assert_eq!(
        served.effects,
        vec![Effect::ServePortion {
            citizen: 4,
            slot: 3,
            qty: 18
        }],
        "`:236-239` : (60 − 0) / 5 = 12, puis ceil(12 × 1,5) = 18"
    );
    assert!(
        scene.memory.cook.citizens_to_serve.is_empty(),
        "`:184` a sorti le citoyen de la file"
    );
    // Et la file vidée, il repart travailler (`:165-168`).
    let back = run(&mut ai, &mut scene, 8);
    assert_eq!(back.last.goal().java_name(), "START_WORKING");
    assert!(back.refusals.contains(&Refusal::CitizenQueueEmpty));
}

#[test]
fn the_cook_serves_a_player_and_fills_sixteen_saturation_per_level() {
    // `:97` puis `:255-304`.
    let mut facts = cook_facts();
    facts.building_level = 2;
    facts.players = vec![7];
    facts.food_in_inventory = vec![FoodStack {
        nutrition: 4.0,
        count: 32,
    }];
    let mut ai = TradeAi::new(Trade::Cook);
    let mut scene = ai.scene_with(world_at_hut(), TradeJob::Cook(facts));
    let _ = run(&mut ai, &mut scene, 1); // IDLE -> START_WORKING
    let queued = run(&mut ai, &mut scene, 1);
    assert_eq!(queued.last.goal().java_name(), "COOK_SERVE_FOOD_TO_PLAYER");
    assert_eq!(scene.memory.cook.players_to_serve, vec![7]);
    // Le joueur est dans la hutte et à portée.
    if let TradeJob::Cook(cook) = &mut scene.job {
        cook.player_target = Some(ServePlayer::ready(7));
    }
    let served = run(&mut ai, &mut scene, 7);
    assert_eq!(
        served.effects,
        vec![Effect::ServePlayer {
            player: 7,
            items: 8
        }],
        "2 niveaux × 16 = 32 de saturation, 4 par item : 8 items"
    );
    assert_eq!(served.last.goal().java_name(), "START_WORKING", "`:303`");
}

#[test]
fn the_cook_reaches_both_of_its_own_states_and_the_two_furnace_retrievals() {
    // Les deux états neufs (`AIWorkerState.java:344`, `:348`) et les deux récupérations du
    // parent (`AbstractEntityAIUsesFurnace.java:81`, `:82`) sont atteignables — trois
    // machines neuves, une par chemin, pour ne pas dépendre des comptes à rebours.
    // 1. un citoyen affamé.
    let mut facts = cook_facts();
    facts.citizens = vec![CitizenHere::hungry(1)];
    let mut ai = TradeAi::new(Trade::Cook);
    let mut scene = ai.scene_with(world_at_hut(), TradeJob::Cook(facts));
    let _ = run(&mut ai, &mut scene, 1);
    assert_eq!(
        run(&mut ai, &mut scene, 1).last.goal().java_name(),
        "COOK_SERVE_FOOD_TO_CITIZEN"
    );

    // 2. un joueur affamé.
    let mut facts = cook_facts();
    facts.players = vec![3];
    facts.worker_menu_food = 4;
    let mut ai = TradeAi::new(Trade::Cook);
    let mut scene = ai.scene_with(world_at_hut(), TradeJob::Cook(facts));
    let _ = run(&mut ai, &mut scene, 1);
    assert_eq!(
        run(&mut ai, &mut scene, 1).last.goal().java_name(),
        "COOK_SERVE_FOOD_TO_PLAYER"
    );

    // 3. un four dont le produit est cuit : `:81`, puis l'extraction compte une action.
    let mut facts = cook_facts();
    facts.furnace.furnaces = vec![Furnace {
        lit: false,
        result: 4,
        ..Furnace::empty(Pos::new(1, 65, 1))
    }];
    let mut ai = TradeAi::new(Trade::Cook);
    let mut scene = ai.scene_with(world_at_hut(), TradeJob::Cook(facts));
    let _ = run(&mut ai, &mut scene, 1);
    let retrieved = run(&mut ai, &mut scene, 1);
    assert_eq!(
        retrieved.last.goal().java_name(),
        "RETRIEVING_END_PRODUCT_FROM_FURNACE"
    );
    assert_eq!(
        scene.memory.base.walk_to,
        Some(Pos::new(1, 65, 1)),
        "`:227` a posé la cible"
    );
    let extracted = run(&mut ai, &mut scene, 1);
    assert_eq!(extracted.last.goal().java_name(), "START_WORKING");
    assert_eq!(
        scene.facts.inventory.actions_done, 1,
        "`:411` `incrementActionsDoneAndDecSaturation`"
    );
    assert_eq!(scene.memory.base.walk_to, None, "`:408` a oublié la cible");

    // 4. un carburant que le filtre refuse : `:82`, et il ne compte pas d'action.
    let mut facts = cook_facts();
    facts.furnace.furnaces = vec![Furnace {
        fuel_present: true,
        fuel_allowed: false,
        ..Furnace::empty(Pos::new(1, 65, 1))
    }];
    let mut ai = TradeAi::new(Trade::Cook);
    let mut scene = ai.scene_with(world_at_hut(), TradeJob::Cook(facts));
    let _ = run(&mut ai, &mut scene, 1);
    assert_eq!(
        run(&mut ai, &mut scene, 1).last.goal().java_name(),
        "RETRIEVING_USED_FUEL_FROM_FURNACE"
    );
    let fuel = run(&mut ai, &mut scene, 1);
    assert_eq!(fuel.last.goal().java_name(), "START_WORKING");
    assert_eq!(
        scene.facts.inventory.actions_done, 0,
        "`:443` n'appelle pas `incrementActionsDone`"
    );
    assert!(ai.missing().is_none());
}

#[test]
fn a_cook_with_a_full_backpack_is_sent_dumping_by_the_base_rule() {
    // `AbstractEntityAIBasic.java:409-416` : c'est la table de base qui choisit
    // `INVENTORY_FULL`, pas celle du métier. Ce que le métier fournit, c'est le seuil
    // (`EntityAIWorkCook.java:427-431`) et le chemin de retour (`:1213-1222`).
    let mut ai = TradeAi::new(Trade::Cook);
    let mut scene = ai.scene_with(world_at_hut(), TradeJob::Cook(cook_facts()));
    scene.facts.inventory.full = true;
    assert!(dump_due(Trade::Cook, &scene, ai.state()));
    // Le retour existe bien dans la table du cuisinier…
    assert!(ai
        .machine()
        .can_leave(TradeState::Furnace(SmeltState::InventoryFull)));
    // … alors qu'un livreur n'a même pas cet état : son sac plein est son métier.
    assert!(!TradeAi::new(Trade::Deliveryman)
        .machine()
        .can_leave(TradeState::Furnace(SmeltState::InventoryFull)));
}

// -------------------------------------------------------------------------- livreur

#[test]
fn the_courier_table_answers_like_the_delivery_table() {
    // **Le second test de non-duplication.** `crate::ai_build::delivery` porte déjà les
    // six cibles ; la machine de ce module ne doit pas en inventer une septième ni
    // répondre autrement. Les deux tournent en parallèle sur les mêmes faits.
    let mut facts = DeliveryFacts::default();
    facts.task = Task::Delivery;
    facts.at_target = false;
    let mut mine = TradeAi::new(Trade::Deliveryman);
    let mut mine_scene = mine.scene_with(world_at_hut(), TradeJob::Deliveryman(facts.clone()));
    let mut theirs = crate::ai_build::delivery::ai();
    let mut their_facts = facts;
    for tick in 0..6 {
        let left = mine.tick(&mut mine_scene);
        theirs.tick(&mut their_facts);
        assert_eq!(
            left.goal().java_name(),
            theirs.state().java_name(),
            "les deux tables divergent au tick {tick}"
        );
    }
}

#[test]
fn a_courier_without_a_warehouse_stays_stuck_and_says_why() {
    // `:638-661` : la porte de `START_WORKING`.
    let mut facts = DeliveryFacts::default();
    facts.warehouse = false;
    let mut ai = TradeAi::new(Trade::Deliveryman);
    let mut scene = ai.scene_with(world_at_hut(), TradeJob::Deliveryman(facts));
    let first = run(&mut ai, &mut scene, 1);
    assert_eq!(first.last.goal().java_name(), "START_WORKING");
    let stuck = run(&mut ai, &mut scene, 1);
    assert_eq!(stuck.last.goal().java_name(), "START_WORKING");
    assert_eq!(
        stuck.refusals,
        vec![Refusal::Courier(
            crate::ai_build::delivery::Refusal::NoWarehouse
        )]
    );
}

#[test]
fn a_courier_empties_a_hut_then_dumps_when_the_request_was_important() {
    // `:145-207` puis `:188-202`.
    let mut facts = DeliveryFacts::default();
    facts.task = Task::Pickup;
    facts.gathered = true;
    facts.request_priority = super::courier::PRIORITY_FORCING_DUMP;
    let mut ai = TradeAi::new(Trade::Deliveryman);
    let mut scene = ai.scene_with(world_at_hut(), TradeJob::Deliveryman(facts));
    let _ = run(&mut ai, &mut scene, 2); // IDLE, puis la décision -> PICKUP
    assert_eq!(ai.state(), TradeState::Courier(DeliveryState::Pickup));
    let dumped = run(&mut ai, &mut scene, 1);
    assert_eq!(
        dumped.last.goal().java_name(),
        "DUMPING",
        "`:188-191` : une priorité de 10 force le vidage"
    );
}

// ---------------------------------------------------------------------- invariants

#[test]
fn every_transition_of_every_table_names_its_java_line() {
    // La règle 3 du port, vérifiée mécaniquement : aucune cible sans source.
    for trade in Trade::ALL {
        let ai = TradeAi::new(trade);
        for transition in ai.machine().transitions() {
            assert!(!transition.rule.is_empty(), "{trade:?} : règle sans nom");
            assert!(
                transition.java.contains(".java:"),
                "{trade:?} : {} ne cite pas de fichier Java",
                transition.rule
            );
            assert!(transition.tick_rate >= 1);
        }
        assert!(
            ai.machine().transitions().len() >= 6,
            "{trade:?} n'a pas sa table"
        );
    }
}

#[test]
fn no_table_falls_into_a_trap_on_a_thirty_tick_walk() {
    // `BasicStateMachine.java:220-228` : un état sans sortie est un piège. Aucun des
    // trois métiers ne doit en produire sur les faits par défaut.
    for trade in Trade::ALL {
        let mut ai = TradeAi::new(trade);
        let mut scene = ai.scene(WorldFacts::new());
        for _ in 0..30 {
            let outcome = ai.tick(&mut scene);
            assert!(!outcome.faulted(), "{trade:?} est tombé dans un piège");
        }
        assert!(ai.missing().is_none(), "{trade:?} : {:?}", ai.missing());
    }
}

#[test]
fn the_same_facts_replayed_twice_give_the_same_states() {
    // Une décision qui dépend d'un tirage ne se rejoue pas : les trois tirages de
    // l'éleveur sont des faits, donc ce scénario doit rendre deux fois la même chose.
    let scenario = || {
        let mut ai = TradeAi::new(Trade::SwineHerder);
        let mut scene = ai.scene_with(
            world_at_hut(),
            TradeJob::Swine(SwineFacts::new(herd_facts(herd_of(2)))),
        );
        let mut goals = Vec::new();
        for _ in 0..7 {
            goals.push(ai.tick(&mut scene).goal().java_name());
        }
        goals
    };
    assert_eq!(scenario(), scenario());
    assert_eq!(
        scenario(),
        vec![
            "START_WORKING",
            "PREPARING",
            "DECIDE",
            "HERDER_BREED",
            "IDLE",
            "START_WORKING",
            "START_WORKING",
        ]
    );
}

#[test]
fn a_scene_always_starts_with_no_refusal_and_no_effect() {
    // Les deux champs volatils de `Scene` sont effacés au début de chaque tick, comme
    // `crate::ai::worker::Scene::refusal` ; sinon un refus d'il y a trois ticks passerait
    // pour celui du tick courant.
    let mut ai = TradeAi::new(Trade::Cook);
    let mut memory = CookMemory::default();
    memory.queue_citizen(4);
    let mut scene = Scene::new(world_at_hut(), TradeJob::Cook(cook_facts()));
    scene.memory = TradeMemory {
        cook: memory,
        ..TradeMemory::default()
    };
    scene.refuse(Refusal::NoFoodInMenu);
    scene.effect = Some(Effect::ServePlayer {
        player: 1,
        items: 1,
    });
    let outcome = ai.tick(&mut scene);
    assert_eq!(outcome.why_not(), None);
    assert_eq!(outcome.effect(), None);
    // Et la scène elle-même est nettoyée pour l'appelant.
    assert_eq!(scene.refusal, None);
    assert_eq!(scene.effect, None);
}

// ------------------------------------------------------- la table, pas la règle

// Ces neuf tests portent sur ce que les trois métiers partagent — le pont `Trade`, la
// forme d'une `Decision`, la quantité d'un `Effect`, le seuil de vidage — et non sur une
// règle d'une classe Java. Ils vivaient dans `mod.rs` ; ils sont ici pour qu'il n'y ait
// **qu'un** module `tests` déclaré (deux `mod tests` dans le même module parent sont une
// erreur de compilation, `E0428`).

#[test]
fn the_three_trades_name_their_java_class_and_their_job() {
    // `EntityAIWorkSwineHerder.java:16`, `EntityAIWorkCook.java:54`,
    // `EntityAIWorkDeliveryman.java:57`.
    assert_eq!(Trade::ALL.len(), 3);
    assert_eq!(Trade::SwineHerder.class(), "EntityAIWorkSwineHerder");
    assert_eq!(Trade::Cook.class(), "EntityAIWorkCook");
    assert_eq!(Trade::Deliveryman.class(), "EntityAIWorkDeliveryman");
    assert_eq!(Trade::Cook.job(), crate::jobs::job::Job::Cook);
    assert_eq!(Trade::Deliveryman.job(), crate::jobs::job::Job::Deliveryman);
    assert_eq!(Trade::SwineHerder.to_string(), "swineherder");
}

#[test]
fn each_trade_starts_idle_in_its_own_state_enumeration() {
    for trade in Trade::ALL {
        let ai = TradeAi::new(trade);
        assert_eq!(ai.state(), trade.initial_state());
        assert_eq!(ai.state().java_name(), "IDLE");
        assert_eq!(ai.trade(), trade);
    }
}

#[test]
fn the_dump_threshold_is_ten_for_the_herder_one_for_the_cook_and_none_for_the_courier() {
    // `AbstractEntityAIHerder.java:152-156`, `EntityAIWorkCook.java:427-431` et
    // l'exemption de `AbstractEntityAIBasic.java:415`.
    assert_eq!(Trade::SwineHerder.actions_until_dumping(), Some(10));
    assert_eq!(Trade::Cook.actions_until_dumping(), Some(1));
    assert_eq!(Trade::Deliveryman.actions_until_dumping(), None);
}

#[test]
fn a_courier_never_dumps_on_its_action_counter() {
    // `:415` : `job instanceof JobDeliveryman` — la limite n'existe pas pour lui, même
    // avec cent actions au compteur.
    let ai = TradeAi::new(Trade::Deliveryman);
    let mut scene = ai.scene(WorldFacts::new());
    scene.facts.inventory.actions_done = 100;
    assert!(!dump_due(Trade::Deliveryman, &scene, ai.state()));
    // Le cuisinier, lui, vide dès la première action (`:427-431`).
    let cook = TradeAi::new(Trade::Cook);
    let mut scene = cook.scene(WorldFacts::new());
    scene.facts.inventory.actions_done = 1;
    assert!(dump_due(Trade::Cook, &scene, cook.state()));
}

#[test]
fn a_full_inventory_makes_every_trade_dump() {
    // `:412` : `inventory.isInventoryFull()`.
    for trade in [Trade::SwineHerder, Trade::Cook] {
        let ai = TradeAi::new(trade);
        let mut scene = ai.scene(WorldFacts::new());
        scene.facts.inventory.full = true;
        assert!(dump_due(trade, &scene, ai.state()), "{trade:?}");
    }
    // Sauf le livreur, dont le sac plein est le métier (`EntityAIWorkDeliveryman.java:
    // 264-276`).
    let courier = TradeAi::new(Trade::Deliveryman);
    let mut scene = courier.scene(WorldFacts::new());
    scene.facts.inventory.full = true;
    assert!(!dump_due(Trade::Deliveryman, &scene, courier.state()));
}

#[test]
fn a_scene_of_another_trade_is_refused_by_name() {
    // Le premier tick d'un cuisinier à qui on donne les faits d'un livreur : `IDLE` →
    // `START_WORKING` est une constante qui ne lit rien (`AbstractEntityAIUsesFurnace.java:
    // 77`), c'est le tick suivant qui lit les faits du métier.
    let mut ai = TradeAi::new(Trade::Cook);
    let mut scene = ai.scene_with(
        WorldFacts::new(),
        TradeJob::Deliveryman(DeliveryFacts::default()),
    );
    let first = ai.tick(&mut scene);
    assert_eq!(first.goal(), TradeState::Furnace(SmeltState::StartWorking));
    assert_eq!(first.why_not(), None);
    let outcome = ai.tick(&mut scene);
    assert_eq!(outcome.why_not(), Some(Refusal::NoFactsForTrade));
    assert!(!outcome.faulted(), "la machine tient au lieu de piéger");
    assert_eq!(outcome.goal(), first.goal(), "il reste où il est");
}

#[test]
fn the_effect_carries_the_line_that_decides_it() {
    // Les trois quantités décidées, chacune avec sa ligne.
    assert_eq!(
        Effect::FeedInPlace {
            citizen: 1,
            items: 10
        }
        .java(),
        "EntityAIWorkCook.java:192-211"
    );
    assert_eq!(
        Effect::ServePortion {
            citizen: 1,
            slot: 2,
            qty: 18
        }
        .java(),
        "EntityAIWorkCook.java:236-246"
    );
    assert_eq!(
        Effect::ServePlayer {
            player: 3,
            items: 8
        }
        .java(),
        "EntityAIWorkCook.java:288-290"
    );
}

#[test]
fn a_decision_is_either_a_state_a_refusal_or_a_quantity() {
    let go = Decision::go(Trade::Cook.initial_state());
    assert_eq!(go.next, Some(Trade::Cook.initial_state()));
    assert_eq!(go.refusal, None);
    let stay = Decision::stay();
    assert_eq!(stay, Decision::default());
    let refused = Decision::refuse(Refusal::CitizenQueueEmpty);
    assert_eq!(refused.next, None);
    assert_eq!(refused.refusal, Some(Refusal::CitizenQueueEmpty));
    let decided = Decision::go_after(
        Trade::Deliveryman.initial_state(),
        Refusal::PlayerQueueEmpty,
    )
    .with_effect(Effect::ServePlayer {
        player: 1,
        items: 2,
    });
    assert!(decided.effect.is_some());
}

#[test]
fn every_trade_table_can_leave_its_starting_state() {
    // Un état sans sortie serait un piège (`BasicStateMachine.java:220-228`).
    for trade in Trade::ALL {
        let ai = TradeAi::new(trade);
        assert!(
            ai.machine().can_leave(trade.initial_state()),
            "{trade:?} ne peut pas quitter IDLE"
        );
        assert!(!ai.machine().transitions().is_empty());
    }
}
