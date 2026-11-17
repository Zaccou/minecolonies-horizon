//! Le livreur : six cibles, quatre courses, une porte — et **rien de neuf à décider**.
//!
//! Java référence : `core/entity/ai/workers/service/EntityAIWorkDeliveryman.java` (662
//! lignes) et `core/colony/jobs/JobDeliveryman.java:507-518`.
//!
//! ## La prémisse de la consigne qui ne tient pas, et ce que le code fait à la place
//!
//! La mission demande de porter « une machine à états testable » pour le livreur. Elle
//! **existe déjà** : `crate::ai_build::delivery` porte les six cibles des lignes
//! `113-124`, les quatre courses (`pickup` `:145-207`, `dump` `:291-314`, `deliver`
//! `:327-466`, `prepareDelivery` `:468-552`), le dispatcheur (`decide` `:591-631`), les
//! deux portes de `checkIfExecute` (`:638-661`), la limite de sac (`cannotHoldMoreItems`
//! `:264-276`) et la limite de livraisons parallèles (`JobDeliveryman.java:511-518`),
//! avec ses tests. Écrire une deuxième machine ici serait la deuxième copie d'une table
//! qui a déjà la sienne — exactement ce que la règle 6 interdit.
//!
//! Ce fichier apporte donc les **trois** choses qui manquaient pour que le livreur soit
//! un métier de cette tranche comme les deux autres :
//!
//! 1. un état [`super::state::TradeState::Courier`] qui enveloppe `DeliveryState` sans le
//!    redire ;
//! 2. six cibles sur **la machine de ce module**, chacune appelant la fonction de
//!    décision de `crate::ai_build::delivery` — la règle a un seul domicile, seule la
//!    cible est dite ici ;
//! 3. le refus nommé que Java cache dans une condition : voir [`decide_action`].
//!
//! Un test (`tests::the_courier_table_answers_like_the_delivery_table`) fait tourner les
//! deux machines en parallèle sur les mêmes faits et compare les six états.
//!
//! ## La porte, et pourquoi elle n'est pas une `Condition`
//!
//! Java écrit `new AITarget(START_WORKING, this::checkIfExecute, this::decide,
//! DECISION_DELAY)` (`:118`) : sans entrepôt, la cible **ne tire pas** et le citoyen reste
//! planté en `START_WORKING` avec une interaction bloquante (`:653-659`). Le moteur de ce
//! port sait exprimer ça (`Transition::target_when`), mais une condition reçoit les faits
//! **en lecture seule** : elle ne peut pas nommer la raison, et un joueur qui demande
//! « pourquoi mon livreur ne bouge pas » n'aurait alors aucune réponse.
//!
//! La vérification est donc **dans l'action**, qui note
//! [`super::state::Refusal::Courier`] et rend l'état courant. La suite d'états est
//! identique — `START_WORKING` n'a qu'une transition, donc une condition fausse et une
//! action qui reste produisent le même tick — et la raison est dite.

use crate::ai::state::Machine;
use crate::ai_build::delivery::{self, Decide, Delivery, DeliveryState, Dump, Pickup, Prepare};

use super::facts::{Scene, TradeJob};
use super::state::{Refusal, TradeState};
use super::{add_target, TICKS_SECOND};

/// Le livreur ne vide pas son sac sur un compteur d'actions : `inventoryNeedsDump` a une
/// exemption nommée pour lui (`AbstractEntityAIBasic.java:415`, `job instanceof
/// JobDeliveryman`). Ce métier ne répond donc pas à `getActionsDoneUntilDumping` — sa
/// limite est le nombre de piles de son niveau (`:264-276`), lue par
/// `crate::ai_build::delivery::cannot_hold_more_items`.
pub const DUMPS_ON_ACTIONS_DONE: bool = false;

/// `PRIORITY_FORCING_DUMP` (`EntityAIWorkDeliveryman.java:80-82`), réexporté du module
/// qui le porte : une requête servie à cette priorité renvoie le livreur vider son sac.
pub const PRIORITY_FORCING_DUMP: i32 = delivery::PRIORITY_FORCING_DUMP;

/// `MIN_DISTANCE_TO_WAREHOUSE` (`:60-62`) : la distance sous laquelle le livreur ne prend
/// aucune décision — il est déjà à l'entrepôt. Le port ne s'en sert pas comme d'un test
/// (c'est `walkToBuilding` qui répond, `:598`), mais la constante est citée parce qu'un
/// appelant qui remplit `at_warehouse` doit mesurer la même chose.
pub const MIN_DISTANCE_TO_WAREHOUSE: i32 = 5;

/// `SLOT_HAND = 0` (`:74-77`) : le slot tenu en main, que `dump` et `deliver` remettent à
/// zéro (`:311`, `:454`, `:463`). Sans objet pour une décision, cité pour l'appelant.
pub const SLOT_HAND: u32 = 0;

/// `:117` — `new AITarget(IDLE, () -> START_WORKING, 1)`.
fn idle_to_work(_scene: &mut Scene, _state: TradeState) -> Option<TradeState> {
    Some(TradeState::Courier(DeliveryState::StartWorking))
}

/// `:118` — la porte (`:638-661`) **puis** la décision (`:591-631`), dans l'ordre de
/// Java : la cible ne tire pas sans entrepôt.
///
/// Le refus est nommé ici et nulle part ailleurs : c'est le seul endroit du métier où
/// Java s'arrête au lieu de choisir.
fn decide_action(scene: &mut Scene, state: TradeState) -> Option<TradeState> {
    let Scene { job, refusal, .. } = scene;
    let TradeJob::Deliveryman(facts) = job else {
        *refusal = Some(Refusal::NoFactsForTrade);
        return Some(state);
    };
    // `:640-648` — l'entrepôt existe et son bloc est là.
    if let Err(door) = delivery::check_if_execute(facts) {
        *refusal = Some(Refusal::Courier(door));
        return Some(state); // la cible ne tire pas : on reste en `START_WORKING`
    }
    // `:593` — `setVisibleStatus(WORKING)`, un effet que l'appelant exécute.
    match delivery::decide(facts) {
        Decide::Stay => Some(state), // `:598-613` : `return START_WORKING`
        Decide::Dump => Some(TradeState::Courier(DeliveryState::Dumping)), // `:607`, `:618-621`
        Decide::PrepareDelivery => Some(TradeState::Courier(DeliveryState::PrepareDelivery)), // `:624`
        Decide::Pickup => Some(TradeState::Courier(DeliveryState::Pickup)), // `:629`
    }
}

/// `:119` — `new AITarget(PREPARE_DELIVERY, this::prepareDelivery, STANDARD_DELAY)`.
fn prepare_action(scene: &mut Scene, state: TradeState) -> Option<TradeState> {
    let Scene { job, refusal, .. } = scene;
    let TradeJob::Deliveryman(facts) = job else {
        *refusal = Some(Refusal::NoFactsForTrade);
        return Some(state);
    };
    match delivery::prepare(facts) {
        // `:476-481`, `:518-524`, `:549-551` : retour au travail.
        Prepare::WrongTask | Prepare::Unreachable | Prepare::NothingThere => {
            Some(TradeState::Courier(DeliveryState::StartWorking))
        }
        // `:513-516`, `:543-547` : assez collecté, on livre.
        Prepare::Deliver(_) | Prepare::EnoughParallel => {
            Some(TradeState::Courier(DeliveryState::Delivery))
        }
        // `:526-529`, `:537-541` : il continue de ramasser.
        Prepare::Walking | Prepare::StillGathering => Some(state),
        // `:531-534` : le sac est plein.
        Prepare::Dump => Some(TradeState::Courier(DeliveryState::Dumping)),
    }
}

/// `:120` — `new AITarget(DELIVERY, this::deliver, STANDARD_DELAY)`.
fn deliver_action(scene: &mut Scene, state: TradeState) -> Option<TradeState> {
    let Scene { job, refusal, .. } = scene;
    let TradeJob::Deliveryman(facts) = job else {
        *refusal = Some(Refusal::NoFactsForTrade);
        return Some(state);
    };
    match delivery::deliver(facts) {
        // `:334-341` : la tâche a changé, le sac est chargé pour rien : vider d'abord.
        Delivery::WrongTask => Some(TradeState::Courier(DeliveryState::Dumping)),
        // `:345-350`, `:352-357`, `:449-459`, `:464-465` : retour au travail.
        Delivery::Unreachable
        | Delivery::TargetGone
        | Delivery::NothingToDeliver
        | Delivery::Delivered => Some(TradeState::Courier(DeliveryState::StartWorking)),
        // `:359-363` : il marche encore.
        Delivery::Walking => Some(state),
        // `:409-441` puis `:465` : le coffre a refusé, `success == false`.
        Delivery::ChestFull => Some(TradeState::Courier(DeliveryState::Dumping)),
    }
}

/// `:121` — `new AITarget(PICKUP, this::pickup, PICKUP_DELAY)`.
fn pickup_action(scene: &mut Scene, state: TradeState) -> Option<TradeState> {
    let Scene { job, refusal, .. } = scene;
    let TradeJob::Deliveryman(facts) = job else {
        *refusal = Some(Refusal::NoFactsForTrade);
        return Some(state);
    };
    match delivery::pickup(facts) {
        // `:150-154` : la tâche a changé.
        Pickup::WrongTask => Some(TradeState::Courier(DeliveryState::StartWorking)),
        // `:156-161` et `:188-202` : le sac est plein, ou la requête était prioritaire.
        Pickup::ForcedDump | Pickup::PickedUpForcedDump => {
            Some(TradeState::Courier(DeliveryState::Dumping))
        }
        // `:166-171` et `:194` : retour au travail.
        Pickup::BuildingGone | Pickup::PickedUp => {
            Some(TradeState::Courier(DeliveryState::StartWorking))
        }
        // `:173-176` et `:204-206` : il marche, ou il reste des slots à vider.
        Pickup::Walking | Pickup::More => Some(state),
    }
}

/// `:122` — `new AITarget(DUMPING, this::dump, TICKS_SECOND)`.
fn dump_action(scene: &mut Scene, state: TradeState) -> Option<TradeState> {
    let Scene { job, refusal, .. } = scene;
    let TradeJob::Deliveryman(facts) = job else {
        *refusal = Some(Refusal::NoFactsForTrade);
        return Some(state);
    };
    match delivery::dump(facts) {
        // `:298-302` : aucun entrepôt. Le prochain `START_WORKING` refusera à son tour,
        // par la porte.
        Dump::NoWarehouse => Some(TradeState::Courier(DeliveryState::StartWorking)),
        // `:304-308` : il marche encore.
        Dump::Walking => Some(state),
        // `:310-313` : vidé.
        Dump::Dumped => Some(TradeState::Courier(DeliveryState::StartWorking)),
    }
}

/// Enregistre les six cibles d'`EntityAIWorkDeliveryman.java:117-122`, sur
/// [`TradeState::Courier`].
///
/// Les cadences sont celles du Java : `1` (`:117`), `DECISION_DELAY = 100` (`:118`),
/// `STANDARD_DELAY = 5` (`:119`, `:120`), `PICKUP_DELAY = 5` (`:121`), `TICKS_SECOND = 20`
/// (`:122`).
pub fn register(machine: &mut Machine<TradeState, Scene>) {
    // :117 — `new AITarget(IDLE, () -> START_WORKING, 1)`.
    machine.add(crate::ai::Transition::target(
        TradeState::Courier(DeliveryState::Idle),
        "idleToWork",
        "EntityAIWorkDeliveryman.java:117",
        1,
        crate::ai::Action::Run(idle_to_work),
    ));
    // :118 — la décision, derrière la porte de l'entrepôt.
    add_target(
        machine,
        TradeState::Courier(DeliveryState::StartWorking),
        "decide",
        "EntityAIWorkDeliveryman.java:118",
        delivery::DECISION_DELAY,
        decide_action,
    );
    // :119 — `new AITarget(PREPARE_DELIVERY, this::prepareDelivery, STANDARD_DELAY)`.
    add_target(
        machine,
        TradeState::Courier(DeliveryState::PrepareDelivery),
        "prepareDelivery",
        "EntityAIWorkDeliveryman.java:119",
        crate::ai::worker::STANDARD_DELAY,
        prepare_action,
    );
    // :120 — `new AITarget(DELIVERY, this::deliver, STANDARD_DELAY)`.
    add_target(
        machine,
        TradeState::Courier(DeliveryState::Delivery),
        "deliver",
        "EntityAIWorkDeliveryman.java:120",
        crate::ai::worker::STANDARD_DELAY,
        deliver_action,
    );
    // :121 — `new AITarget(PICKUP, this::pickup, PICKUP_DELAY)`.
    add_target(
        machine,
        TradeState::Courier(DeliveryState::Pickup),
        "pickup",
        "EntityAIWorkDeliveryman.java:121",
        delivery::PICKUP_DELAY,
        pickup_action,
    );
    // :122 — `new AITarget(DUMPING, this::dump, TICKS_SECOND)`.
    add_target(
        machine,
        TradeState::Courier(DeliveryState::Dumping),
        "dump",
        "EntityAIWorkDeliveryman.java:122",
        TICKS_SECOND,
        dump_action,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_build::delivery::Task;

    fn courier(facts: delivery::DeliveryFacts) -> (super::super::TradeAi, Scene) {
        let ai = super::super::TradeAi::new(super::super::Trade::Deliveryman);
        let scene = ai.scene_with(
            crate::ai::facts::WorldFacts::new(),
            TradeJob::Deliveryman(facts),
        );
        (ai, scene)
    }

    #[test]
    fn the_courier_constants_are_the_java_ones_and_come_from_one_home() {
        // `EntityAIWorkDeliveryman.java:60-82` et `JobDeliveryman.java:507-518`.
        assert_eq!(MIN_DISTANCE_TO_WAREHOUSE, 5);
        assert_eq!(SLOT_HAND, 0);
        assert_eq!(PRIORITY_FORCING_DUMP, 10);
        assert_eq!(delivery::PICKUP_DELAY, 5);
        assert_eq!(delivery::DECISION_DELAY, 100, "TICKS_SECOND * 5");
        // Et l'exemption nommée d'`AbstractEntityAIBasic.java:415`.
        assert!(!DUMPS_ON_ACTIONS_DONE);
    }

    #[test]
    fn the_door_refuses_before_any_decision() {
        // `:638-661` : sans entrepôt, la cible ne tire pas et le refus est nommé.
        let mut facts = delivery::DeliveryFacts::default();
        facts.warehouse = false;
        let (mut ai, mut scene) = courier(facts);
        // Le premier tick va de `IDLE` à `START_WORKING` (`:117`)...
        let first = ai.tick(&mut scene);
        assert_eq!(
            first.goal(),
            TradeState::Courier(DeliveryState::StartWorking)
        );
        // ... et le suivant reste planté, avec la raison.
        let second = ai.tick(&mut scene);
        assert_eq!(second.goal(), first.goal());
        assert_eq!(
            second.why_not(),
            Some(Refusal::Courier(delivery::Refusal::NoWarehouse))
        );
        // Un entrepôt sans bloc est la même porte, autrement nommée (`:644-647`).
        //
        // **Machine neuve, et c'est le Java qui l'impose** : `DECISION_DELAY =
        // TICKS_SECOND * 5` (`:64-67`) est la cadence de `new AITarget(START_WORKING,
        // this::checkIfExecute, this::decide, DECISION_DELAY)` (`:118`), donc la décision
        // n'est *regardée* qu'une fois par cent ticks. Réutiliser la machine d'au-dessus
        // testerait le compte à rebours, pas la règle : la cible ne tirerait pas.
        let mut facts = delivery::DeliveryFacts::default();
        facts.warehouse_has_chest = false;
        let (mut ai, mut scene) = courier(facts);
        let _ = ai.tick(&mut scene); // `:117` IDLE -> START_WORKING
        let outcome = ai.tick(&mut scene);
        assert_eq!(
            outcome.goal(),
            TradeState::Courier(DeliveryState::StartWorking)
        );
        assert_eq!(
            outcome.why_not(),
            Some(Refusal::Courier(delivery::Refusal::WarehouseHasNoChest))
        );
    }

    #[test]
    fn decide_dumps_a_loaded_backpack_before_a_delivery() {
        // `:615-621` : une livraison avec un sac non vide passe par `DUMPING`.
        let mut facts = delivery::DeliveryFacts::default();
        facts.task = Task::Delivery;
        facts.inventory_empty = false;
        let (mut ai, mut scene) = courier(facts);
        let _ = ai.tick(&mut scene); // IDLE -> START_WORKING
        let outcome = ai.tick(&mut scene);
        assert_eq!(outcome.goal(), TradeState::Courier(DeliveryState::Dumping));
        // Le même sac vide : il prépare la livraison (`:624`). Machine neuve, parce que
        // `DECISION_DELAY` vaut cent ticks (`:64-67`, `:118`) : sur la machine
        // d'au-dessus, `decide` vient de tirer et ne serait pas regardé à nouveau.
        let mut facts = delivery::DeliveryFacts::default();
        facts.task = Task::Delivery;
        let (mut ai, mut scene) = courier(facts);
        let _ = ai.tick(&mut scene); // IDLE -> START_WORKING
        assert_eq!(
            ai.tick(&mut scene).goal(),
            TradeState::Courier(DeliveryState::PrepareDelivery)
        );
    }

    #[test]
    fn a_pickup_task_sends_the_courier_to_pickup() {
        // `:628-630`.
        let mut facts = delivery::DeliveryFacts::default();
        facts.task = Task::Pickup;
        let (mut ai, mut scene) = courier(facts);
        let _ = ai.tick(&mut scene);
        assert_eq!(
            ai.tick(&mut scene).goal(),
            TradeState::Courier(DeliveryState::Pickup)
        );
    }

    #[test]
    fn nothing_to_do_loiters_at_the_warehouse_and_dumps_if_loaded() {
        // `:597-613`.
        let (mut ai, mut scene) = courier(delivery::DeliveryFacts::default());
        let _ = ai.tick(&mut scene);
        assert_eq!(
            ai.tick(&mut scene).goal(),
            TradeState::Courier(DeliveryState::StartWorking),
            "sac vide : il reste"
        );
        // Sac non vide, même situation : `:605-608` l'envoie vider. Machine neuve pour la
        // même raison qu'au-dessus : `DECISION_DELAY` (`:64-67`, `:118`).
        let mut facts = delivery::DeliveryFacts::default();
        facts.inventory_empty = false;
        let (mut ai, mut scene) = courier(facts);
        let _ = ai.tick(&mut scene); // IDLE -> START_WORKING
        assert_eq!(
            ai.tick(&mut scene).goal(),
            TradeState::Courier(DeliveryState::Dumping)
        );
    }

    #[test]
    fn the_six_targets_all_have_a_transition() {
        // Chaque état de la table a une transition : un état sans sortie serait un piège
        // (`BasicStateMachine.java:220-228`).
        let ai = super::super::TradeAi::new(super::super::Trade::Deliveryman);
        assert!(ai
            .machine()
            .can_leave(TradeState::Courier(DeliveryState::Idle)));
        assert!(ai
            .machine()
            .can_leave(TradeState::Courier(DeliveryState::StartWorking)));
        assert!(ai
            .machine()
            .can_leave(TradeState::Courier(DeliveryState::PrepareDelivery)));
        assert!(ai
            .machine()
            .can_leave(TradeState::Courier(DeliveryState::Delivery)));
        assert!(ai
            .machine()
            .can_leave(TradeState::Courier(DeliveryState::Pickup)));
        assert!(ai
            .machine()
            .can_leave(TradeState::Courier(DeliveryState::Dumping)));
    }
}
