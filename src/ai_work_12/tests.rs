//! Les scénarios : des faits poussés dans la machine, et la suite d'états que Java
//! produirait. C'est le test qui compte — un état isolé peut être juste et la suite
//! fausse.
//!
//! Chaque test nomme la ligne Java qui décide de la transition, et les cadences sont
//! celles des constructeurs (`EntityAIWorkHealer.java:86-93`,
//! `EntityAIWorkUndertaker.java:90-96`, `AbstractEntityAICrafting.java:164`) : la
//! première passe de chaque transition tourne toujours, parce que son compte-à-rebours
//! part de zéro (`TickingTransition.java:118-121`).

use crate::ai::facts::{Pos, WorldFacts};
use crate::ai::state::{Machine, WorkerState, ENTITY_AI_TICKRATE};
use crate::ai_work_12::facts::{
    HealerFacts, Patient, PatientState, ServiceJob, ServiceScene, UndertakerFacts,
};
use crate::ai_work_12::{
    Effect, EnchanterFacts, ServiceAi, ServiceOutcome, ServiceRefusal, ServiceState, ServiceTrade,
};
use crate::citizens::happiness::disease::Disease;

/// Fait tourner `ai` `count` fois et rend les issues, dans l'ordre.
fn walk(ai: &mut ServiceAi, scene: &mut ServiceScene, count: usize) -> Vec<ServiceOutcome> {
    (0..count).map(|_| ai.tick(scene)).collect()
}

/// Les états visités, l'état de départ inclus.
fn states(start: ServiceState, outcomes: &[ServiceOutcome]) -> Vec<ServiceState> {
    let mut visited = vec![start];
    for outcome in outcomes {
        visited.push(outcome.state);
    }
    visited
}

#[test]
fn the_healer_walks_idle_start_working_decide_request_cure_and_back() {
    // `EntityAIWorkHealer.java:86-88` puis `:129-133` (`currentPatient = patient;
    // return REQUEST_CURE;`), puis `:301-303` (`return DECIDE`).
    let mut ai = ServiceAi::new(ServiceTrade::Healer);
    let mut scene = ai.scene(WorldFacts::new());
    scene.job = ServiceJob::Healer(HealerFacts {
        at_hospital: true,
        // `walkToSafePos(...)` a rendu vrai (`:256`) : le guérisseur est au chevet.
        at_target: true,
        patients: vec![Patient::sick_of(11, PatientState::New, Disease::INFLUENZA)],
        ..HealerFacts::default()
    });
    let start = ai.state();
    let outcomes = walk(&mut ai, &mut scene, 4);
    assert_eq!(
        states(start, &outcomes),
        vec![
            ServiceState::Base(WorkerState::Idle),
            ServiceState::Base(WorkerState::StartWorking),
            ServiceState::Base(WorkerState::Decide),
            ServiceState::RequestCure,
            ServiceState::Base(WorkerState::Decide),
        ],
        "IDLE → START_WORKING → DECIDE → REQUEST_CURE → DECIDE"
    );
    // La troisième issue est la passe de `DECIDE` : elle choisit le patient et rend
    // `REQUEST_CURE` sans rien demander encore (`:129-133`).
    let deciding = &outcomes[2];
    assert_eq!(deciding.goal(), ServiceState::RequestCure);
    assert!(deciding.effects.is_empty());
    // La quatrième est `requestCure` : seize de chaque remède, puis le dossier passe à
    // `REQUESTED` (`:296`, `:301`).
    let asked = &outcomes[3];
    assert_eq!(
        asked.effects,
        vec![
            Effect::RequestCure {
                item: "minecraft:carrot",
                count: 16
            },
            Effect::RequestCure {
                item: "minecraft:potato",
                count: 16
            },
            Effect::SetPatientState {
                patient: 11,
                state: PatientState::Requested
            },
        ]
    );
    assert_eq!(asked.goal(), ServiceState::Base(WorkerState::Decide));
    assert_eq!(asked.why(), Some("EntityAIWorkHealer.java:92"));
}

#[test]
fn the_undertaker_walks_idle_start_working_wander_and_back() {
    // `EntityAIWorkUndertaker.java:90-92` : la cadence de `IDLE` est `REQUEST_DELAY` =
    // 60, celle des états de travail 5, et `startWorking` sans tombe rend `WANDER`
    // (`:143-144`) ; `wander` rend `IDLE` (`:167`).
    let mut ai = ServiceAi::new(ServiceTrade::Undertaker);
    let mut scene = ai.scene(WorldFacts::new());
    scene.job = ServiceJob::Undertaker(UndertakerFacts::default());
    let start = ai.state();
    let outcomes = walk(&mut ai, &mut scene, 3);
    assert_eq!(
        states(start, &outcomes),
        vec![
            ServiceState::Base(WorkerState::Idle),
            ServiceState::Base(WorkerState::StartWorking),
            ServiceState::Wander,
            ServiceState::Base(WorkerState::Idle),
        ],
        "IDLE → START_WORKING → WANDER → IDLE"
    );
    // La marche demandée par la dernière passe est celle de `:163` : rentrer au
    // cimetière.
    assert_eq!(outcomes[2].effects, vec![Effect::WalkToHut]);
}

#[test]
fn the_enchanter_without_a_tome_asks_for_one_and_goes_back_to_idle() {
    // `EntityAIWorkEnchanter.java:190-200` : la branche du tome, puis `return IDLE`. Le
    // seuil de vidage est d'une action (`:214-218`) et rien n'a été fait, donc la passe
    // ne s'arrête pas avant.
    let mut ai = ServiceAi::new(ServiceTrade::Enchanter);
    let mut scene = ai.scene(WorldFacts::new());
    scene.job = ServiceJob::Enchanter(EnchanterFacts {
        // `:130` : `isPastTime(world, 6000)` vrai, donc la branche d'atelier est sautée et
        // la boucle `:141-204` s'applique. C'est l'heure où Java draine et enchante ; plus
        // tard dans la journée, l'atelier a la main (`the_enchanter_spends_the_afternoon_…`).
        past_six_thousand: true,
        ..EnchanterFacts::default()
    });
    let start = ai.state();
    let outcomes = walk(&mut ai, &mut scene, 2);
    assert_eq!(
        states(start, &outcomes),
        vec![
            ServiceState::Base(WorkerState::Idle),
            ServiceState::Base(WorkerState::StartWorking),
            ServiceState::Base(WorkerState::Idle),
        ],
        "IDLE → START_WORKING → IDLE"
    );
    let asked = &outcomes[1];
    assert_eq!(asked.why_not(), Some(ServiceRefusal::NoAncientTome));
    assert_eq!(
        asked.effects,
        vec![
            Effect::EmptyMainHand,
            Effect::RequestItem {
                item: crate::ai_work_12::enchanter::ANCIENT_TOME,
                count: 1
            },
        ],
        "`:123` puis `:199`"
    );
}

#[test]
fn the_enchanter_spends_the_afternoon_in_the_workshop() {
    // `EntityAIWorkEnchanter.java:129-133` : `getNextCraftingState()` ne rend jamais
    // `START_WORKING`, donc dès que la journée n'est pas « passée », l'atelier a la main.
    let mut ai = ServiceAi::new(ServiceTrade::Enchanter);
    let mut scene = ai.scene(WorldFacts::new());
    scene.job = ServiceJob::Enchanter(EnchanterFacts {
        craft: crate::ai_build::crafter::CraftFacts {
            current_task: true,
            ..Default::default()
        },
        past_six_thousand: false,
        ..EnchanterFacts::default()
    });
    ai.tick(&mut scene); // IDLE → START_WORKING
    let workshop = ai.tick(&mut scene); // START_WORKING → decide
    assert_eq!(workshop.goal(), ServiceState::Base(WorkerState::StartWorking));
    assert_eq!(workshop.why_not(), Some(ServiceRefusal::CraftingDue));
    assert!(workshop.effects.iter().any(|effect| matches!(
        effect,
        Effect::Crafted {
            state: crate::ai_build::crafter::CraftState::GetRecipe
        }
    )));
}

#[test]
fn a_foreign_trade_state_has_no_exit_in_another_table() {
    // La table du guérisseur ne nomme pas `ENCHANT` : un état de l'enchanteur n'a aucune
    // transition chez elle, ce qu'un appelant doit savoir avant de mélanger les tables.
    let healer = ServiceAi::new(ServiceTrade::Healer);
    assert!(!healer.machine().can_leave(ServiceState::Enchant));
    assert!(!healer.machine().can_leave(ServiceState::BuryCitizen));
    let enchanter = ServiceAi::new(ServiceTrade::Enchanter);
    assert!(!enchanter.machine().can_leave(ServiceState::Cure));
    // Et chacun peut sortir de ses propres états.
    assert!(enchanter.machine().can_leave(ServiceState::Enchant));
    assert!(healer.machine().can_leave(ServiceState::Cure));
}

#[test]
fn every_state_of_a_table_has_a_way_out_and_no_tick_traps() {
    // `BasicStateMachine.java:220-228` : un état qu'aucune transition ne nomme est un
    // piège, et la machine revient à son état initial en le signalant.
    for trade in ServiceTrade::ALL {
        let mut machine: Machine<ServiceState, ServiceScene> =
            Machine::new(ServiceState::Base(WorkerState::Idle), ENTITY_AI_TICKRATE);
        trade.register(&mut machine);
        let mut scene = ServiceScene::new(WorldFacts::new(), trade.job_facts());
        // Trente ticks : largement de quoi traverser une fois chaque état de repos.
        for _ in 0..30 {
            let _ = machine.tick(&mut scene);
            assert_eq!(machine.missing(), None, "{trade} est tombé dans un piège");
        }
        for state in crate::ai_work_12::states_of(trade) {
            assert!(machine.can_leave(state), "{trade} : {state} n'a pas de sortie");
        }
    }
}

#[test]
fn the_outcome_carries_the_rule_and_the_java_line_of_the_transition() {
    // Un `Fired` dit quelle règle a tourné et d'où elle vient : c'est ce qu'un panneau
    // « pourquoi mon citoyen fait ça » affiche.
    let mut ai = ServiceAi::new(ServiceTrade::Undertaker);
    let mut scene = ai.scene(WorldFacts::new());
    scene.job = ServiceJob::Undertaker(UndertakerFacts::default());
    let first = ai.tick(&mut scene);
    assert_eq!(first.why(), Some("EntityAIWorkUndertaker.java:90"));
    assert!(first.moved());
    let second = ai.tick(&mut scene);
    assert_eq!(second.why(), Some("EntityAIWorkUndertaker.java:91"));
    assert_eq!(second.goal(), ServiceState::Wander);
    // Un citoyen à vingt blocs de sa hutte n'y est pas : `at_hut` mesure comme
    // `walkToBuilding` (`AbstractEntityAIBasic.java:757-766`).
    let mut far = WorldFacts::new();
    far.worker.at = Pos::new(20, 64, 0);
    let mut ai = ServiceAi::new(ServiceTrade::Undertaker);
    let mut scene = ai.scene(far);
    scene.job = ServiceJob::Undertaker(UndertakerFacts {
        grave_to_work_on: Some(Pos::new(3, 64, 3)),
        ..UndertakerFacts::default()
    });
    ai.tick(&mut scene);
    let blocked = ai.tick(&mut scene);
    assert_eq!(blocked.why_not(), Some(ServiceRefusal::NotAtHut));
    assert_eq!(blocked.goal(), ServiceState::Base(WorkerState::StartWorking));
}
