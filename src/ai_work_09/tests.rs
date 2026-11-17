//! Les scénarios de faits : une suite d'états qui doit être celle du Java.
//!
//! Chaque test suit la même forme — une scène remplie, N ticks, la liste des états —
//! et nomme la ligne Java qui produit chaque pas. C'est le test qui compte : une
//! décision qui ne peut être rejouée que dans le jeu ne peut pas être vérifiée, et une
//! table qui bouge sans que ces suites bougent est une table qui a changé de
//! comportement.

use crate::ai::facts::{Pos, WorldFacts};
use crate::ai::state::WorkerState;

use super::facts::{
    CowboyFacts, FieldFacts, FieldWork, HerdAnimal, HerderDraws, HerdFacts, PlanterFacts, WorkJob,
};
use super::{WorkAi, WorkRefusal, WorkScene, WorkState, WorkTrade};

/// Ticks `ai` `count` fois et rend l'état en vigueur après chaque tick.
fn run(ai: &mut WorkAi, scene: &mut WorkScene, count: usize) -> Vec<WorkState> {
    let mut states = Vec::with_capacity(count);
    for _ in 0..count {
        states.push(ai.tick(scene).goal());
    }
    states
}

fn base(state: WorkerState) -> WorkState {
    WorkState::Base(state)
}

/// Un monde où le citoyen est à sa hutte, à l'origine.
fn at_hut() -> WorldFacts {
    let mut facts = WorldFacts::new();
    facts.building.position = Pos::new(0, 64, 0);
    facts.worker.at = Pos::new(0, 64, 0);
    facts
}

/// Le planteur du scénario : un champ de cinq actions, une position à travailler, une
/// graine en poche, et une réponse `PLANT` qui a réussi.
fn planter_facts() -> PlanterFacts {
    PlanterFacts {
        building_level: 1,
        has_extensions: true,
        previous_field: None,
        field: Some(FieldFacts {
            position: Pos::new(0, 65, 0),
            action_limit: 5,
            work_position: Some(Pos::new(1, 65, 1)),
        }),
        decided: Some(FieldWork::plant(Pos::new(1, 65, 1), Pos::new(1, 66, 1))),
        at_field: true,
        at_action: true,
        has_tool: true,
        plant_count: 4,
        plants_to_request: 8,
        bonemeal_count: 0,
        action_done: true,
    }
}

fn animal(id: u32, at: Pos) -> HerdAnimal {
    HerdAnimal {
        id,
        at,
        age: 0,
        baby: false,
        in_love: false,
        can_fall_in_love: true,
        alive: true,
        can_see_sky: true,
        can_mate: true,
    }
}

/// Un troupeau de quatre adultes qui ne déclenche ni accouplement ni nourrissage : un
/// seul item d'accouplement, donc `> 1` est faux (`AbstractEntityAIHerder.java:227-228`).
fn herd_facts(animals: Vec<HerdAnimal>) -> HerdFacts {
    HerdFacts {
        building_level: 1,
        breeding_setting: true,
        has_module: true,
        animals,
        items_on_ground: 0,
        first_item: None,
        has_axe: true,
        breeding_item_count: 1,
        breeding_item_equipped: true,
        killed: false,
        chicken_draw: 0,
        game_time: 0,
        primary_skill: 10,
        secondary_skill: 10,
        main_hand_empty: false,
        draws: HerderDraws {
            pickup: 0.9,
            butcher: 0.9,
            feed: 0.9,
        },
    }
}

fn cowboy_facts() -> CowboyFacts {
    CowboyFacts {
        herd: herd_facts(vec![
            animal(1, Pos::new(0, 64, 0)),
            animal(2, Pos::new(1, 64, 0)),
        ]),
        can_try_to_milk: true,
        can_try_to_stew: true,
        milk_input_in_inventory: true,
        milk_input_in_hut: true,
        bowl_in_inventory: true,
        bowl_in_hut: true,
        milkable_animal: Some(Pos::new(0, 64, 0)),
        mooshroom: Some(Pos::new(1, 64, 0)),
        equipped: true,
        stew_succeeded: true,
    }
}

#[test]
fn the_planter_walks_its_six_states_in_java_s_order() {
    // La suite que cette table doit reproduire :
    //   IDLE → START_WORKING → PREPARING → PICK_FIELD → MOVE_TO_FIELD →
    //   DECIDE_FIELD_WORK → WORK_FIELD.
    let mut ai = WorkAi::new(WorkTrade::Planter);
    let mut scene = ai.scene_with(at_hut(), WorkJob::Planter(planter_facts()));
    let states = run(&mut ai, &mut scene, 6);
    assert_eq!(
        states,
        vec![
            // `AbstractEntityAICrafting.java:164` — `hasWorkToDo`, surchargé en `true`
            // par `EntityAIWorkPlanter.java:468-471`.
            base(WorkerState::StartWorking),
            // `EntityAIWorkPlanter.java:416-425` — `decide`.
            base(WorkerState::Preparing),
            // `:79` — `prepare`, avec un `activeModuleResult` nul.
            WorkState::PlantationPickField,
            // `:80` — `pickField`, un champ existe et il est nouveau.
            WorkState::PlantationMoveToField,
            // `:81` — `moveToField`, il est arrivé.
            WorkState::PlantationDecideFieldWork,
            // `:82` — `decideFieldWork`, le module a répondu une position et une action.
            WorkState::PlantationWorkField,
        ]
    );
    // Et la règle qui a produit chaque pas est bien celle qu'on cite.
    let mut ai = WorkAi::new(WorkTrade::Planter);
    let mut scene = ai.scene_with(at_hut(), WorkJob::Planter(planter_facts()));
    let mut lines = Vec::new();
    for _ in 0..6 {
        lines.push(ai.tick(&mut scene).why());
    }
    assert_eq!(
        lines,
        vec![
            Some("EntityAIWorkPlanter.java:468-471"),
            Some("EntityAIWorkPlanter.java:416-425"),
            Some("EntityAIWorkPlanter.java:79"),
            Some("EntityAIWorkPlanter.java:80"),
            Some("EntityAIWorkPlanter.java:81"),
            Some("EntityAIWorkPlanter.java:82"),
        ]
    );
}

#[test]
fn the_seventh_tick_works_the_field_and_asks_for_the_next_position() {
    // `:206-256` — l'action `PLANT` réussit, elle compte (jusqu'ici 0 → 1) et
    // `shouldResetWorkingPosition()` renvoie sur `DECIDE_FIELD_WORK`.
    let mut ai = WorkAi::new(WorkTrade::Planter);
    let mut scene = ai.scene_with(at_hut(), WorkJob::Planter(planter_facts()));
    run(&mut ai, &mut scene, 6);
    let outcome = ai.tick(&mut scene);
    assert_eq!(outcome.goal(), WorkState::PlantationDecideFieldWork);
    assert_eq!(outcome.why(), Some("EntityAIWorkPlanter.java:83"));
    assert_eq!(scene.memory.planter.current_field_action_count, 1);
    assert_eq!(scene.facts.inventory.actions_done, 1, "`:230`");
}

#[test]
fn a_planter_with_no_field_ends_the_loop_in_idle_and_names_the_reason() {
    // `:105-109` puis `:112-120` : la hutte n'est pas construite.
    let mut ai = WorkAi::new(WorkTrade::Planter);
    let mut facts = planter_facts();
    facts.building_level = 0;
    let mut scene = ai.scene_with(at_hut(), WorkJob::Planter(facts));
    let states = run(&mut ai, &mut scene, 4);
    assert_eq!(
        states,
        vec![
            base(WorkerState::StartWorking),
            base(WorkerState::Preparing),
            WorkState::PlantationPickField,
            base(WorkerState::Idle),
        ]
    );
    assert_eq!(scene.refusal, Some(WorkRefusal::HutNotBuilt), "`:107-108`");
}

#[test]
fn the_chicken_herder_reaches_the_butcher_state_in_four_ticks() {
    //   IDLE → START_WORKING → PREPARING → DECIDE → HERDER_BUTCHER.
    // Quatre adultes au niveau 1 font `chanceToButcher` = 0,5 × 4⁴/2⁴ = 8, donc le
    // tirage 0,9 ne suffit pas — mais un tirage de 0,5 si.
    let mut facts = herd_facts(vec![
        animal(1, Pos::new(0, 64, 0)),
        animal(2, Pos::new(1, 64, 0)),
        animal(3, Pos::new(2, 64, 0)),
        animal(4, Pos::new(3, 64, 0)),
    ]);
    facts.draws.butcher = 0.5;
    let mut ai = WorkAi::new(WorkTrade::ChickenHerder);
    let mut scene = ai.scene_with(at_hut(), WorkJob::ChickenHerder(facts));
    let states = run(&mut ai, &mut scene, 4);
    assert_eq!(
        states,
        vec![
            // `AbstractEntityAIHerder.java:140` — `IDLE → START_WORKING`, cadence 1.
            base(WorkerState::StartWorking),
            // `:141` — `startWorkingAtOwnBuilding`.
            base(WorkerState::Preparing),
            // `:142` — `prepareForHerding`, sans module courant (`:304-307`).
            base(WorkerState::Decide),
            // `:143` puis `:234-237` — `decideWhatToDo` tire l'abattage.
            WorkState::HerderButcher,
        ]
    );
}

#[test]
fn a_butcher_that_kills_returns_to_decide_and_counts_one_action() {
    // `AbstractEntityAIHerder.java:379-387` : l'animal est mort, le compteur d'action
    // tourne et la cible est retirée des animaux nourris récemment.
    let mut facts = herd_facts(vec![
        animal(1, Pos::new(0, 64, 0)),
        animal(2, Pos::new(1, 64, 0)),
        animal(3, Pos::new(2, 64, 0)),
        animal(4, Pos::new(3, 64, 0)),
    ]);
    facts.draws.butcher = 0.5;
    facts.killed = true;
    // Le citoyen est sur l'animal le plus loin du centre, donc la cible (`:354-357`).
    let mut world = at_hut();
    world.building.position = Pos::new(3, 64, 0);
    world.worker.at = Pos::new(3, 64, 0);
    let mut ai = WorkAi::new(WorkTrade::ChickenHerder);
    let mut scene = ai.scene_with(world, WorkJob::ChickenHerder(facts));
    let states = run(&mut ai, &mut scene, 5);
    assert_eq!(states.last(), Some(&base(WorkerState::Decide)));
    assert_eq!(scene.facts.inventory.actions_done, 1, "`:383`");
}

#[test]
fn the_cowboy_reaches_the_milking_state_in_four_ticks() {
    //   IDLE → START_WORKING → PREPARING → DECIDE → COWBOY_MILK.
    // La traite n'est atteinte que si le parent rend `START_WORKING`
    // (`EntityAIWorkCowboy.java:105`), ce qui est le cas ici : un seul item
    // d'accouchement, deux adultes (donc pas d'abattage), trois tirages hauts.
    let mut ai = WorkAi::new(WorkTrade::Cowboy);
    let mut scene = ai.scene_with(at_hut(), WorkJob::Cowboy(cowboy_facts()));
    let states = run(&mut ai, &mut scene, 4);
    assert_eq!(
        states,
        vec![
            base(WorkerState::StartWorking),
            base(WorkerState::Preparing),
            base(WorkerState::Decide),
            // `EntityAIWorkCowboy.java:70` puis `:105-108`.
            WorkState::CowboyMilk,
        ]
    );
    // Le cinquième tick trait l'animal et rend `INVENTORY_FULL` (`:187`).
    let outcome = ai.tick(&mut scene);
    assert_eq!(outcome.goal(), base(WorkerState::InventoryFull));
    assert_eq!(outcome.why(), Some("EntityAIWorkCowboy.java:70"));
    assert_eq!(scene.facts.inventory.actions_done, 1, "`:183`");
}

#[test]
fn a_milk_that_cannot_start_leaves_the_cowboy_on_the_parent_state() {
    // `:148-159` : pas de seau, posé dix passages de refroidissement plus tard.
    let mut facts = cowboy_facts();
    facts.milk_input_in_inventory = false;
    facts.milk_input_in_hut = false;
    let mut ai = WorkAi::new(WorkTrade::Cowboy);
    let mut scene = ai.scene_with(at_hut(), WorkJob::Cowboy(facts));
    let states = run(&mut ai, &mut scene, 5);
    // Tick 5 : `COWBOY_MILK` tire, ne peut pas traire, et rend `DECIDE`.
    assert_eq!(states.last(), Some(&base(WorkerState::Decide)));
    assert_eq!(scene.refusal, Some(WorkRefusal::NoMilkInput));
    assert_eq!(scene.memory.cowboy.milk_cool_down, 10, "`:157`");
}

#[test]
fn no_trade_ever_falls_into_a_trap() {
    // `BasicStateMachine.java:220-228` : un état qu'aucune transition ne nomme est un
    // piège, signalé puis remis à `INIT`. Une table qui en contient un se voit ici.
    for trade in WorkTrade::ALL {
        let mut ai = WorkAi::new(trade);
        let mut scene = ai.scene(WorldFacts::new());
        for _ in 0..40 {
            ai.tick(&mut scene);
            assert_eq!(
                ai.missing(),
                None,
                "{trade} est tombé dans un piège après {} ticks",
                scene.facts.inventory.actions_done
            );
        }
    }
}

#[test]
fn every_trade_answers_its_own_refusal_when_given_another_s_facts() {
    // Le macro de `super` : un fait du mauvais métier rend `NoFactsForTrade` au lieu de
    // faire semblant de n'avoir rien à faire. Le rang du refus dépend de la table :
    // celle d'élevage est partagée par les deux éleveurs
    // (`EntityAIWorkCowboy.java:39`), donc un cow-boy à qui on donne des faits
    // d'éleveur de poulets la traverse et ne bute que sur `DECIDE`.
    let cases = [
        // Le planteur n'a aucune action partagée : son `START_WORKING` refuse.
        (WorkTrade::Planter, WorkJob::Cowboy(CowboyFacts::default()), 2),
        // L'éleveur de poulets non plus face à des faits de planteur.
        (
            WorkTrade::ChickenHerder,
            WorkJob::Planter(PlanterFacts::default()),
            2,
        ),
        // Le cow-boy : `START_WORKING`, `PREPARING` et `DECIDE` du parent passent
        // (faits d'élevage acceptés), puis sa propre décision refuse.
        (
            WorkTrade::Cowboy,
            WorkJob::ChickenHerder(HerdFacts::default()),
            4,
        ),
    ];
    for (trade, job, ticks) in cases {
        let mut ai = WorkAi::new(trade);
        let mut scene = ai.scene_with(at_hut(), job);
        // Tick 1 : `IDLE` part sur sa cible. Les suivants : la table du métier.
        let mut outcome = ai.tick(&mut scene);
        for _ in 1..ticks {
            outcome = ai.tick(&mut scene);
        }
        assert_eq!(
            outcome.why_not(),
            Some(WorkRefusal::NoFactsForTrade),
            "{trade}"
        );
    }
}

#[test]
fn the_three_trades_disagree_on_when_to_dump_their_inventory() {
    // `AbstractEntityAIBasic.java:413` lit `getActionsDoneUntilDumping()` : 32 hors du
    // champ, la limite du module dessus, dix pour les deux éleveurs.
    let planter_ai = WorkAi::new(WorkTrade::Planter);
    let planter_scene = planter_ai.scene_with(at_hut(), WorkJob::Planter(planter_facts()));
    assert_eq!(
        WorkTrade::Planter.actions_until_dumping(&planter_scene, base(WorkerState::StartWorking)),
        32
    );
    assert_eq!(
        WorkTrade::Planter
            .actions_until_dumping(&planter_scene, WorkState::PlantationWorkField),
        5
    );
    let herder_ai = WorkAi::new(WorkTrade::ChickenHerder);
    let herder_scene = herder_ai.scene_with(at_hut(), WorkJob::ChickenHerder(herd_facts(Vec::new())));
    assert_eq!(
        WorkTrade::ChickenHerder.actions_until_dumping(&herder_scene, base(WorkerState::Decide)),
        10
    );
    let cowboy_ai = WorkAi::new(WorkTrade::Cowboy);
    let cowboy_scene = cowboy_ai.scene_with(at_hut(), WorkJob::Cowboy(cowboy_facts()));
    assert_eq!(
        WorkTrade::Cowboy.actions_until_dumping(&cowboy_scene, base(WorkerState::Decide)),
        10
    );
}

#[test]
fn a_full_inventory_is_not_a_dump_and_a_deliveryman_never_dumps() {
    // `AbstractEntityAIBasic.java:411-415`.
    let ai = WorkAi::new(WorkTrade::Cowboy);
    let mut scene = ai.scene_with(at_hut(), WorkJob::Cowboy(cowboy_facts()));
    assert!(!super::dump_due(WorkTrade::Cowboy, &scene, base(WorkerState::Decide)));
    scene.facts.inventory.full = true;
    assert!(super::dump_due(WorkTrade::Cowboy, &scene, base(WorkerState::Decide)));
    // `getState() != INVENTORY_FULL` : y être déjà n'est pas devoir y aller.
    assert!(!super::dump_due(
        WorkTrade::Cowboy,
        &scene,
        base(WorkerState::InventoryFull)
    ));
    // Un état qui ne s'interrompt pas ne vide pas non plus (`:411`, `canBeInterrupted`).
    assert!(!super::dump_due(WorkTrade::Cowboy, &scene, WorkState::CowboyMilk));
    // Et un livreur ne vide jamais (`:415`).
    scene.facts.worker.deliveryman = true;
    assert!(!super::dump_due(WorkTrade::Cowboy, &scene, base(WorkerState::Decide)));
}

#[test]
fn the_trade_tables_name_java_s_classes_and_jobs() {
    assert_eq!(WorkTrade::Planter.class(), "EntityAIWorkPlanter");
    assert_eq!(WorkTrade::ChickenHerder.class(), "EntityAIWorkChickenHerder");
    assert_eq!(WorkTrade::Cowboy.class(), "EntityAIWorkCowboy");
    assert_eq!(WorkTrade::Cowboy.name(), "cowboy");
    assert_eq!(WorkTrade::Planter.job(), crate::jobs::job::Job::Planter);
    assert_eq!(
        WorkTrade::ChickenHerder.job(),
        crate::jobs::job::Job::ChickenHerder
    );
    assert_eq!(WorkTrade::Cowboy.job(), crate::jobs::job::Job::Cowboy);
    assert_eq!(WorkTrade::ALL.len(), 3);
}

#[test]
fn a_reset_puts_a_machine_back_to_idle_without_clearing_the_countdowns() {
    // `AbstractAISkeleton.resetAI` (`:145-152`) → `BasicStateMachine.reset`
    // (`:373-380`).
    let mut ai = WorkAi::new(WorkTrade::Planter);
    let mut scene = ai.scene_with(at_hut(), WorkJob::Planter(planter_facts()));
    run(&mut ai, &mut scene, 4);
    assert_eq!(ai.state(), WorkState::PlantationMoveToField);
    ai.reset();
    assert_eq!(ai.state(), base(WorkerState::Idle));
    assert_eq!(ai.trade(), WorkTrade::Planter);
    // Et la machine repart : le compteur d'`IDLE` a été consommé au premier tick, donc
    // elle attend son tour au lieu de tirer tout de suite.
    let _ = ai.tick(&mut scene);
}
