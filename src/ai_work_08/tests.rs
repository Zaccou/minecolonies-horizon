//! Les tests du module : un scénario de faits, la suite d'états que Java en tire.
//!
//! **Ce que ces tests mesurent.** Pas « le code compile » mais « la décision suit
//! `EntityAIWorkFlorist.java` » : chaque assertion nomme la ligne qu'elle vérifie, et les
//! scénarios de machine comparent une **suite** d'états, pas un tick isolé — c'est la
//! seule façon de voir qu'un `return getState()` a bien retenu le citoyen au lieu de le
//! renvoyer chercher (`EntityAIWorkFarmer.java:529-533`).
//!
//! Le fermier et le pêcheur n'ont pas de test de décision ici : leur table est dans
//! `crate::ai_gather` (`farmer::tests`, `fisherman::tests`) et ce module ne fait que la
//! tiquer. Ce qui est testé pour eux, c'est la **composition** — que [`AgriAi::new`]
//! construise la bonne machine, et que [`phase_of`] lise ses états.

use super::*;
use crate::ai::facts::{Pos, WorldFacts};
use crate::ai::state::WorkerState;
use crate::ai_gather::GatherState;

// ---------------------------------------------------------------- outils de scénario

/// Le monde nu, avec une hutte et un citoyen à l'origine.
fn bare_world() -> WorldFacts {
    WorldFacts::new()
}

/// Une scène de fleuriste sur le monde nu.
fn florist(facts: FloristFacts) -> AgriScene {
    AgriScene::florist_with(bare_world(), facts)
}

/// La scène du fleuriste, en écriture. Panique avec un message clair si la scène n'est
/// pas celle du fleuriste — c'est une erreur de test, pas une donnée de fichier.
fn florist_scene(scene: &mut AgriScene) -> &mut FloristScene {
    scene
        .florist_scene_mut()
        .expect("the scenario must hand a florist scene to the florist")
}

/// Un état de base, pour les assertions.
fn base(state: WorkerState) -> AgriState {
    AgriState::Gather(GatherState::Base(state))
}

/// L'état du fermier `state`, pour les assertions.
fn farmer_state(state: GatherState) -> AgriState {
    AgriState::Gather(state)
}

// ------------------------------------------------------------------------ les phases

#[test]
fn a_walking_citizen_is_travelling_and_a_standing_one_is_acting() {
    // `EntityAIWorkFarmer.java:529-533` : Java ne change pas d'état pour marcher, il
    // reste dans `FARMER_HOE`. La phase, elle, change.
    let hoe = farmer_state(GatherState::FarmerHoe);
    assert_eq!(phase_of(hoe, GoalAnswers::walking()), Phase::Travel);
    assert_eq!(phase_of(hoe, GoalAnswers::arrived()), Phase::Act);
    // Et la même lecture vaut pour le fleuriste (`:223`, `:265`).
    assert_eq!(phase_of(AgriState::FloristCompost, GoalAnswers::default()), Phase::Travel);
    assert_eq!(phase_of(AgriState::FloristHarvest, GoalAnswers::arrived()), Phase::Act);
}

#[test]
fn the_fisherman_walks_in_a_state_of_its_own() {
    // `EntityAIWorkFisherman.java:174` : `FISHERMAN_WALKING_TO_WATER` est un état, et
    // `walkToWater` (`:312-317`) ne fait que le retenir — la phase est donc `Travel`
    // quelle que soit la réponse du tick.
    let walking = farmer_state(GatherState::FishermanWalkingToWater);
    assert_eq!(phase_of(walking, GoalAnswers::default()), Phase::Travel);
    assert_eq!(phase_of(walking, GoalAnswers::arrived()), Phase::Travel);
    // `FISHERMAN_SEARCHING_WATER` (`:173`) cherche un point d'eau : Search.
    assert_eq!(
        phase_of(
            farmer_state(GatherState::FishermanSearchingWater),
            GoalAnswers::default()
        ),
        Phase::Search
    );
}

#[test]
fn the_two_chest_transfers_are_the_deliver_phase() {
    // `AbstractEntityAIBasic.java:225` (vider) et `:242` (prendre la pile demandée) : deux
    // sens du même transfert avec les coffres de la hutte.
    assert_eq!(
        phase_of(base(WorkerState::InventoryFull), GoalAnswers::default()),
        Phase::Deliver
    );
    assert_eq!(
        phase_of(
            base(WorkerState::GatheringRequiredMaterials),
            GoalAnswers::default()
        ),
        Phase::Deliver
    );
}

#[test]
fn pause_and_a_pending_request_are_the_fifth_phase() {
    // Ni chercher, ni aller, ni agir, ni rendre : `PAUSED` (`AbstractEntityAIBasic.java:251`)
    // et `NEEDS_ITEM` (`:238`) sont un citoyen qui attend. Le brief en nomme quatre, Java
    // en a cinq.
    assert_eq!(phase_of(base(WorkerState::Paused), GoalAnswers::default()), Phase::Wait);
    assert_eq!(
        phase_of(base(WorkerState::NeedsItem), GoalAnswers::default()),
        Phase::Wait
    );
    assert_eq!(Phase::ALL.len(), 5);
}

#[test]
fn every_state_of_the_three_trades_has_a_phase_and_no_state_is_left_to_guess() {
    // Trente et un états, dont douze appartenant à deux métiers que ce module ne porte
    // pas : `phase_of` répond pour eux aussi (voir sa note), et aucune réponse n'est un
    // `unreachable!`.
    for state in AgriState::ALL {
        let phase = phase_of(state, GoalAnswers::arrived());
        assert!(Phase::ALL.contains(&phase), "{state} fell through to {phase}");
    }
    // Les quatre phases sont atteignables par les trois métiers de ce module.
    let reached: Vec<Phase> = [
        base(WorkerState::StartWorking),
        farmer_state(GatherState::FarmerHoe),
        base(WorkerState::InventoryFull),
        base(WorkerState::Paused),
    ]
    .iter()
    .map(|state| phase_of(*state, GoalAnswers::default()))
    .collect();
    assert_eq!(
        reached,
        vec![Phase::Search, Phase::Travel, Phase::Deliver, Phase::Wait]
    );
}

// --------------------------------------------------------------- les terres du fleuriste

#[test]
fn a_flower_above_a_ground_is_found_at_the_grounds_own_height_plus_one() {
    // `EntityAIWorkFlorist.java:352-355` — Java rend `pos.above()`, pas `pos`.
    let facts = FloristFacts::ready(1)
        .with_ground(PlantGround::growing(Pos::new(3, 64, 3)))
        .with_ground(PlantGround::with_flower(Pos::new(9, 70, 9)));
    assert_eq!(florist::flowers_to_gather(&facts), Some(Pos::new(9, 71, 9)));
}

#[test]
fn a_ground_that_is_not_a_composted_dirt_leaves_the_hut_list() {
    // `EntityAIWorkFlorist.java:379-382` : `building.removePlantableGround(pos)`.
    let facts = FloristFacts::ready(1).with_ground(PlantGround::bare(Pos::new(1, 64, 1)));
    let scan = florist::first_not_composted_land(&facts);
    assert_eq!(scan.position, None);
    assert_eq!(scan.removed, vec![Pos::new(1, 64, 1)]);
}

#[test]
fn an_unloaded_ground_is_skipped_without_being_removed() {
    // `EntityAIWorkFlorist.java:369` : `if (WorldUtil.isEntityBlockLoaded(world, pos))`
    // avant tout le reste — une terre dans un chunk déchargé n'est ni lue ni retirée.
    let facts = FloristFacts::ready(1)
        .with_ground(PlantGround::unloaded(Pos::new(1, 64, 1)))
        .with_ground(PlantGround::composted_dirt(Pos::new(2, 64, 2)));
    let scan = florist::first_not_composted_land(&facts);
    assert_eq!(scan.position, Some(Pos::new(2, 64, 2)));
    assert!(scan.removed.is_empty());
}

#[test]
fn the_scan_removes_what_it_passed_before_it_returned() {
    // `WorkerUtil.java:326` puis `:321` : le parcours retire en chemin, et le retrait
    // s'arrête à la terre qui répond à la question.
    let facts = FloristFacts::ready(1)
        .with_ground(PlantGround::bare(Pos::new(1, 64, 1)))
        .with_ground(PlantGround::growing(Pos::new(2, 64, 2)))
        .with_ground(PlantGround::bare(Pos::new(3, 64, 3)));
    let scan = florist::composted_land(&facts);
    assert_eq!(scan.position, Some(Pos::new(2, 64, 2)));
    assert_eq!(scan.removed, vec![Pos::new(1, 64, 1)]);
}

#[test]
fn a_ground_that_already_has_its_compost_is_not_offered_again() {
    // `EntityAIWorkFlorist.java:374` : `if (!isComposted())`, la condition inverse de
    // `WorkerUtil.java:319`.
    let facts = FloristFacts::ready(1)
        .with_ground(PlantGround::growing(Pos::new(1, 64, 1)))
        .with_ground(PlantGround::composted_dirt(Pos::new(2, 64, 2)));
    assert_eq!(
        florist::first_not_composted_land(&facts).position,
        Some(Pos::new(2, 64, 2))
    );
    assert_eq!(florist::composted_land(&facts).position, Some(Pos::new(1, 64, 1)));
}

// ------------------------------------------------------------ les seuils et les durées

#[test]
fn the_florist_threshold_grows_with_the_hut_level() {
    // `EntityAIWorkFlorist.java:326` — `HARVEST_ACTIONS_TO_DUMP * getBuildingLevel()`.
    assert_eq!(florist::actions_until_dumping(0), 0);
    assert_eq!(florist::actions_until_dumping(1), 10);
    assert_eq!(florist::actions_until_dumping(5), 50);
    assert_eq!(AgriTrade::Florist.threshold().actions(5), 50);
    assert_eq!(
        AgriTrade::Florist.threshold().java(),
        "EntityAIWorkFlorist.java:324-327"
    );
}

#[test]
fn the_farmer_and_fisherman_thresholds_are_the_ones_ai_gather_already_has() {
    // Les nombres vivent dans `crate::ai_gather` ; ce module les cite, il ne les réécrit
    // pas. `EntityAIWorkFarmer.java:198-201`, `EntityAIWorkFisherman.java:225-229`.
    assert_eq!(
        AgriTrade::Farmer.threshold().actions(1),
        crate::ai_gather::farmer::MAX_BLOCKS_MINED
    );
    assert_eq!(
        AgriTrade::Fisherman.threshold().actions(1),
        crate::ai_gather::fisherman::MAX_FISHES_IN_INV
    );
    // Le seuil du fermier ne dépend pas du niveau, celui du fleuriste si.
    assert_eq!(AgriTrade::Farmer.threshold().actions(5), 64);
    assert_ne!(AgriTrade::Florist.threshold().actions(5), 10);
}

#[test]
fn the_florist_table_is_five_targets_and_the_four_base_ones_it_crosses() {
    // `EntityAIWorkFlorist.java:129-133` plus `AbstractEntityAIBasic.java:225-226`,
    // `:242`, `:251`.
    let durations = AgriTrade::Florist.durations();
    assert_eq!(durations.len(), 7);
    assert_eq!(durations[0].ticks, 1); // :129
    assert_eq!(durations[1].ticks, 20); // :130
    assert_eq!(durations[2].ticks, 200); // :131
    assert_eq!(durations[3].ticks, 20); // :132
    assert_eq!(durations[4].ticks, 20); // :133
    // Les deux cibles du vidage : un événement global, puis l'état `INVENTORY_FULL`.
    assert_eq!(durations[5].state, None);
    assert_eq!(durations[5].ticks, 100);
    assert_eq!(durations[6].ticks, 20);
}

#[test]
fn every_duration_names_its_rule_and_its_java_line() {
    for trade in AgriTrade::ALL {
        let durations = trade.durations();
        assert!(!durations.is_empty(), "{trade} has no target at all");
        for duration in durations {
            assert!(!duration.rule.is_empty());
            assert!(
                duration.java.contains(".java:"),
                "{} cites no line: {}",
                duration.rule,
                duration.java
            );
            assert!(duration.ticks >= 1, "{} has a zero tick rate", duration.rule);
        }
    }
}

#[test]
fn every_florist_target_is_reachable_and_leavable() {
    // Une table dont un état n'a aucune sortie est le piège que `BasicStateMachine`
    // signale (`:220-228`) : le port le teste au lieu de l'espérer.
    let ai = FloristAi::new();
    for state in [
        base(WorkerState::Idle),
        base(WorkerState::StartWorking),
        base(WorkerState::Decide),
        base(WorkerState::InventoryFull),
        base(WorkerState::GatheringRequiredMaterials),
        base(WorkerState::Paused),
        AgriState::FloristHarvest,
        AgriState::FloristCompost,
    ] {
        assert!(ai.machine().can_leave(state), "{state} is a trap state");
    }
    // `PREPARING` n'est enregistré nulle part et `decide` ne le rend jamais : le
    // fleuriste n'y entre pas, contrairement au fermier (`EntityAIWorkFarmer.java:184-187`).
    assert!(!ai.machine().can_leave(base(WorkerState::Preparing)));
}

#[test]
fn the_flower_mining_time_truncates_inside_the_multiplication() {
    // `EntityAIWorkFlorist.java:339` — `BASE_BLOCK_MINING_DELAY * (int)(1 + max(0,
    // MAX_BONUS - PER_LEVEL_BONUS * (secondary / 2.0)))`. Dextérité zéro : 10 × 6 ;
    // dextérité dix : 10 × 5 (5,5 tronqué) ; au plafond : 10 × 1.
    assert_eq!(florist::mining_time(0), 60);
    assert_eq!(florist::mining_time(10), 50);
    assert_eq!(florist::mining_time(20), 50);
    assert_eq!(florist::mining_time(50), 30);
    assert_eq!(florist::mining_time(100), 10);
    assert_eq!(florist::mining_time(400), 10);
}

#[test]
fn the_growth_chance_loses_one_percent_per_hut_level() {
    // `EntityAIWorkFlorist.java:236` — `PERCENT_CHANGE_FOR_GROWTH - level * 0.01`, sur
    // `PERCENT_CHANGE_FOR_GROWTH` (`:60`).
    assert!((florist::growth_chance(0) - 0.2).abs() < 1e-12);
    assert!((florist::growth_chance(5) - 0.15).abs() < 1e-12);
    assert!((florist::growth_chance(20) - 0.0).abs() < 1e-12);
}

// ------------------------------------------------------------------- les refus de decide

#[test]
fn a_hut_without_plant_ground_sends_the_florist_idle() {
    // `EntityAIWorkFlorist.java:154-158`.
    let mut scene = florist(FloristFacts::ready(1));
    let decision = florist::decide(florist_scene(&mut scene));
    assert_eq!(decision.next, Some(base(WorkerState::Idle)));
    assert_eq!(decision.refusal, Some(AgriRefusal::NoPlantGround));
    assert_eq!(
        AgriRefusal::NoPlantGround.java(),
        "EntityAIWorkFlorist.java:154"
    );
}

#[test]
fn without_shears_the_florist_asks_and_idles() {
    // `EntityAIWorkFlorist.java:160-163` puis `:307-321`.
    let facts = FloristFacts {
        shears_slot: None,
        ..FloristFacts::ready(1)
    }
    .with_ground(PlantGround::growing(Pos::new(1, 64, 1)));
    let mut scene = florist(facts);
    let decision = florist::decide(florist_scene(&mut scene));
    assert_eq!(decision.next, Some(base(WorkerState::Idle)));
    assert_eq!(decision.refusal, Some(AgriRefusal::ShearsMissing));
    // Et une requête déjà partie donne le même refus, sans slot à chercher.
    let facts = FloristFacts {
        tool_requested: true,
        ..FloristFacts::ready(1)
    }
    .with_ground(PlantGround::growing(Pos::new(1, 64, 1)));
    assert_eq!(florist::shears_ready(&facts), Err(AgriRefusal::ShearsMissing));
}

#[test]
fn too_far_from_the_hut_the_florist_walks_back_before_gardening() {
    // `EntityAIWorkFlorist.java:166-170` — `distance > MAX_DISTANCE && !walkToBuilding()`.
    let mut world = bare_world();
    world.worker.at = Pos::new(80, 64, 0);
    let facts = FloristFacts::ready(1)
        .with_ground(PlantGround::with_flower(Pos::new(80, 64, 3)))
        .with_compost(4);
    let mut scene = AgriScene::florist_with(world, facts);
    let decision = florist::decide(florist_scene(&mut scene));
    assert_eq!(decision.next, Some(base(WorkerState::Decide)));
    assert_eq!(decision.refusal, Some(AgriRefusal::NotAtHut));
    // `walkToBuilding()` a répondu faux : il n'est pas arrivé, il continue de rentrer.
    // Le jour où il arrive, la distance n'est plus la question.
    let mut world = bare_world();
    world.worker.at = Pos::new(0, 64, 0);
    let mut scene = AgriScene::florist_with(
        world,
        FloristFacts::ready(1)
            .with_ground(PlantGround::with_flower(Pos::new(1, 64, 1)))
            .with_compost(4),
    );
    let decision = florist::decide(florist_scene(&mut scene));
    assert_eq!(decision.next, Some(AgriState::FloristHarvest));
}

#[test]
fn compost_in_the_hut_is_fetched_before_a_flower_is_picked() {
    // `EntityAIWorkFlorist.java:172-180` : le transfert passe avant la cueillette, et
    // avant même de regarder s'il y a une fleur.
    let facts = FloristFacts {
        compost_in_building: 32,
        ..FloristFacts::ready(1)
    }
    .with_ground(PlantGround::with_flower(Pos::new(1, 64, 1)));
    let mut scene = florist(facts);
    let decision = florist::decide(florist_scene(&mut scene));
    assert_eq!(
        decision.next,
        Some(base(WorkerState::GatheringRequiredMaterials))
    );
    assert_eq!(decision.refusal, Some(AgriRefusal::CompostInHut));
    assert!(decision.works.is_empty());
}

#[test]
fn no_compost_anywhere_creates_the_request_and_harvests_anyway() {
    // `EntityAIWorkFlorist.java:181-184` puis `:187-191` : la requête de seize part, et
    // le fleuriste cueille quand même ce qui a poussé.
    let facts = FloristFacts::ready(2).with_ground(PlantGround::with_flower(Pos::new(1, 64, 1)));
    let mut scene = florist(facts);
    let decision = florist::decide(florist_scene(&mut scene));
    assert_eq!(decision.next, Some(AgriState::FloristHarvest));
    assert_eq!(decision.refusal, Some(AgriRefusal::CompostRequested));
    assert_eq!(
        decision.works,
        vec![AgriWork::RequestCompost {
            quantity: florist::COMPOST_REQUEST_QTY
        }]
    );
}

#[test]
fn no_compost_and_nothing_growing_sends_the_florist_back_to_the_start() {
    // `EntityAIWorkFlorist.java:195-198` — `NO_COMPOST`, et `START_WORKING`.
    let facts = FloristFacts::ready(1).with_ground(PlantGround::composted_dirt(Pos::new(1, 64, 1)));
    let mut scene = florist(facts);
    let decision = florist::decide(florist_scene(&mut scene));
    assert_eq!(decision.next, Some(base(WorkerState::StartWorking)));
    assert_eq!(decision.refusal, Some(AgriRefusal::NoCompost));
}

#[test]
fn with_compost_the_first_bare_ground_is_the_one_to_compost() {
    // `EntityAIWorkFlorist.java:202-206` — `compostPosition = getFirstNotCompostedLand()`.
    let facts = FloristFacts::ready(1)
        .with_ground(PlantGround::growing(Pos::new(1, 64, 1)))
        .with_ground(PlantGround::composted_dirt(Pos::new(2, 64, 2)))
        .with_compost(3);
    let mut scene = florist(facts);
    let decision = florist::decide(florist_scene(&mut scene));
    assert_eq!(decision.next, Some(AgriState::FloristCompost));
    assert_eq!(decision.refusal, None);
    assert_eq!(
        florist_scene(&mut scene).memory.compost_position,
        Some(Pos::new(2, 64, 2))
    );
}

#[test]
fn a_field_that_is_all_growing_has_nothing_to_compost() {
    // `EntityAIWorkFlorist.java:204` puis `:216-219` : Java part quand même vers
    // `FLORIST_COMPOST`, qui constate le `null` et rend `START_WORKING`.
    let facts = FloristFacts::ready(1)
        .with_ground(PlantGround::growing(Pos::new(1, 64, 1)))
        .with_compost(3);
    let mut scene = florist(facts);
    let decision = florist::decide(florist_scene(&mut scene));
    assert_eq!(decision.next, Some(AgriState::FloristCompost));
    assert_eq!(decision.refusal, Some(AgriRefusal::NoGroundToCompost));
    assert_eq!(florist_scene(&mut scene).memory.compost_position, None);
}

#[test]
fn a_ground_that_is_no_longer_a_plant_ground_leaves_the_hut_list_on_the_way() {
    // `EntityAIWorkFlorist.java:379-382` : le retrait est un ordre au monde, pas un effet
    // de bord de la décision.
    let facts = FloristFacts::ready(1)
        .with_ground(PlantGround::bare(Pos::new(1, 64, 1)))
        .with_ground(PlantGround::composted_dirt(Pos::new(2, 64, 2)))
        .with_compost(3);
    let mut scene = florist(facts);
    let decision = florist::decide(florist_scene(&mut scene));
    assert_eq!(decision.next, Some(AgriState::FloristCompost));
    assert_eq!(
        decision.works,
        vec![AgriWork::RemovePlantableGround {
            position: Pos::new(1, 64, 1)
        }]
    );
}

// ------------------------------------------------------------------- les refus de compost

#[test]
fn compost_walks_then_orders_one_unit_with_its_chance() {
    // `EntityAIWorkFlorist.java:223-226` (marcher), `:231-236` (une unité, une chance),
    // `:247-248` (position oubliée, retour au début).
    let mut scene = florist(
        FloristFacts::ready(4)
            .with_ground(PlantGround::composted_dirt(Pos::new(1, 64, 1)))
            .with_compost(2),
    );
    florist_scene(&mut scene).memory.compost_position = Some(Pos::new(1, 64, 1));
    let walking = florist::compost(florist_scene(&mut scene));
    assert_eq!(walking.next, None);
    assert_eq!(walking.refusal, Some(AgriRefusal::NotAtTarget));
    assert!(walking.works.is_empty());

    florist_scene(&mut scene).answers = GoalAnswers::arrived();
    let working = florist::compost(florist_scene(&mut scene));
    assert_eq!(working.next, Some(base(WorkerState::StartWorking)));
    assert_eq!(working.refusal, None);
    assert_eq!(working.works.len(), 1);
    match &working.works[0] {
        AgriWork::Compost {
            position,
            flower,
            chance,
        } => {
            assert_eq!(*position, Pos::new(1, 64, 1));
            assert_eq!(flower, "minecraft:poppy");
            // `:236` avec une hutte de niveau quatre : 0,2 - 0,04.
            assert!((chance - 0.16).abs() < 1e-12, "chance = {chance}");
        }
        other => panic!("expected a compost order, got {other:?}"),
    }
    assert_eq!(florist_scene(&mut scene).memory.compost_position, None);
}

#[test]
fn compost_without_a_flower_in_the_config_warns_the_player() {
    // `EntityAIWorkFlorist.java:231-232`, `:239-242` — `NO_FLOWERS_IN_CONFIG`.
    let facts = FloristFacts {
        flower_to_grow: None,
        ..FloristFacts::ready(1)
    }
    .with_ground(PlantGround::composted_dirt(Pos::new(1, 64, 1)))
    .with_compost(2);
    let mut scene = florist(facts);
    florist_scene(&mut scene).memory.compost_position = Some(Pos::new(1, 64, 1));
    florist_scene(&mut scene).answers = GoalAnswers::arrived();
    let decision = florist::compost(florist_scene(&mut scene));
    assert_eq!(decision.next, Some(base(WorkerState::StartWorking)));
    assert_eq!(decision.refusal, Some(AgriRefusal::NoFlowersInConfig));
    assert!(decision.works.is_empty());
}

#[test]
fn compost_without_a_unit_left_refuses_instead_of_growing_anything() {
    // `EntityAIWorkFlorist.java:234` : `shrinkItemCountInItemHandler` répond faux, rien
    // n'est composté — le tirage `nextInt(200 - skill)` ne peut pas être négatif.
    let mut scene = florist(
        FloristFacts::ready(1)
            .with_ground(PlantGround::composted_dirt(Pos::new(1, 64, 1)))
            .with_compost(0),
    );
    florist_scene(&mut scene).memory.compost_position = Some(Pos::new(1, 64, 1));
    florist_scene(&mut scene).answers = GoalAnswers::arrived();
    let decision = florist::compost(florist_scene(&mut scene));
    assert_eq!(decision.refusal, Some(AgriRefusal::CompostGoneWhileComposting));
    assert!(decision.works.is_empty());
}

#[test]
fn compost_counts_the_action_even_when_the_block_is_not_a_composted_dirt() {
    // `EntityAIWorkFlorist.java:229` puis `:245-248` : Java ne fait rien du bloc, compte
    // une action et repart. Le port dit pourquoi il n'a rien fait.
    let mut scene = florist(
        FloristFacts::ready(1)
            .with_ground(PlantGround::bare(Pos::new(1, 64, 1)))
            .with_compost(2),
    );
    florist_scene(&mut scene).memory.compost_position = Some(Pos::new(1, 64, 1));
    florist_scene(&mut scene).answers = GoalAnswers::arrived();
    let decision = florist::compost(florist_scene(&mut scene));
    assert_eq!(decision.next, Some(base(WorkerState::StartWorking)));
    assert_eq!(decision.refusal, Some(AgriRefusal::NotCompostedDirt));
}

#[test]
fn compost_on_a_ground_the_hut_dropped_stops_instead_of_working() {
    // `EntityAIWorkFlorist.java:228` : la position retenue n'est plus dans la liste.
    let mut scene = florist(FloristFacts::ready(1).with_compost(2));
    florist_scene(&mut scene).memory.compost_position = Some(Pos::new(7, 64, 7));
    florist_scene(&mut scene).answers = GoalAnswers::arrived();
    let decision = florist::compost(florist_scene(&mut scene));
    assert_eq!(decision.refusal, Some(AgriRefusal::GroundGone));
    assert_eq!(florist_scene(&mut scene).memory.compost_position, None);
}

#[test]
fn compost_without_a_remembered_ground_does_nothing() {
    // `EntityAIWorkFlorist.java:216-219`.
    let mut scene = florist(FloristFacts::ready(1).with_compost(2));
    let decision = florist::compost(florist_scene(&mut scene));
    assert_eq!(decision.next, Some(base(WorkerState::StartWorking)));
    assert_eq!(decision.refusal, Some(AgriRefusal::NoGroundToCompost));
}

// ------------------------------------------------------------------ les refus de cueillette

#[test]
fn harvest_needs_two_passes_of_mineblock() {
    // `AbstractEntityAIInteract.java:141-149` : le premier `mineBlock` répond faux, Java
    // ressort du tick et rappelle — le port retient l'état et nomme le refus.
    let mut scene = florist(
        FloristFacts::ready(1).with_ground(PlantGround::with_flower(Pos::new(1, 64, 1))),
    );
    florist_scene(&mut scene).memory.harvest_position = Some(Pos::new(1, 65, 1));
    florist_scene(&mut scene).answers = GoalAnswers::arrived();
    let decision = florist::harvest(florist_scene(&mut scene));
    assert_eq!(decision.next, None);
    assert_eq!(decision.refusal, Some(AgriRefusal::MiningIncomplete));
    // Le second passage, quand le bloc est cassé, finit le travail.
    florist_scene(&mut scene).answers = GoalAnswers::broken();
    let decision = florist::harvest(florist_scene(&mut scene));
    assert_eq!(decision.next, Some(base(WorkerState::StartWorking)));
    assert_eq!(
        decision.works,
        vec![AgriWork::Harvest {
            position: Pos::new(1, 65, 1),
            experience: florist::XP_PER_FLOWER,
        }]
    );
    assert_eq!(florist_scene(&mut scene).memory.harvest_position, None);
}

#[test]
fn harvest_walks_first_and_agrees_to_nothing_while_walking() {
    // `EntityAIWorkFlorist.java:265-268`.
    let mut scene = florist(
        FloristFacts::ready(1).with_ground(PlantGround::with_flower(Pos::new(1, 64, 1))),
    );
    florist_scene(&mut scene).memory.harvest_position = Some(Pos::new(1, 65, 1));
    let decision = florist::harvest(florist_scene(&mut scene));
    assert_eq!(decision.next, None);
    assert_eq!(decision.refusal, Some(AgriRefusal::NotAtTarget));
}

#[test]
fn harvest_without_a_remembered_flower_does_nothing() {
    // `EntityAIWorkFlorist.java:258-261`.
    let mut scene = florist(
        FloristFacts::ready(1).with_ground(PlantGround::with_flower(Pos::new(1, 64, 1))),
    );
    let decision = florist::harvest(florist_scene(&mut scene));
    assert_eq!(decision.next, Some(base(WorkerState::StartWorking)));
    assert_eq!(decision.refusal, Some(AgriRefusal::NoFlowerToHarvest));
}

#[test]
fn a_broken_tool_is_named_where_the_tool_was_used() {
    // **Pas Java** : `damageItemInHand` casse les ciseaux sans le dire à l'IA. Le port
    // demande la réponse (`GoalAnswers::tool_broken`) et la nomme, au lieu de laisser
    // croire que le fleuriste n'a jamais eu d'outil.
    let mut scene = florist(
        FloristFacts::ready(1).with_ground(PlantGround::with_flower(Pos::new(1, 64, 1))),
    );
    florist_scene(&mut scene).memory.harvest_position = Some(Pos::new(1, 65, 1));
    florist_scene(&mut scene).answers = GoalAnswers {
        tool_broken: true,
        ..GoalAnswers::broken()
    };
    let decision = florist::harvest(florist_scene(&mut scene));
    assert_eq!(decision.refusal, Some(AgriRefusal::ToolBroken));
    // La cueillette a bien eu lieu : Java ne s'arrête pas sur un outil cassé.
    assert_eq!(decision.works.len(), 1);
}

// --------------------------------------------------------------------- les scénarios

#[test]
fn the_florist_walks_start_working_then_decide() {
    // `EntityAIWorkFlorist.java:129-131` : `IDLE → START_WORKING` (1 tick),
    // `START_WORKING → DECIDE` (20), puis `DECIDE` (200).
    let mut ai = AgriAi::new(AgriTrade::Florist);
    let mut scene = florist(
        FloristFacts::ready(1).with_ground(PlantGround::with_flower(Pos::new(1, 64, 1))),
    );
    let trace = ai.trace_constant(&mut scene, 3, GoalAnswers::default());
    assert_eq!(
        trace.java(),
        vec!["START_WORKING", "DECIDE", "FLORIST_HARVEST"]
    );
    assert_eq!(
        trace.phases(),
        vec![Phase::Search, Phase::Search, Phase::Travel]
    );
}

#[test]
fn a_harvest_scenario_searches_walks_acts_then_searches_again() {
    // Le scénario qui compte. Un tick de machine vaut cinq ticks de jeu
    // (`AbstractEntityCitizen.java:64`), donc une cible de vingt ticks repasse toutes les
    // quatre lectures : la cueillette se fait au tick 4 (arrivé, donc `Act`) et au tick 8
    // (le bloc casse, donc retour à `START_WORKING`, `EntityAIWorkFlorist.java:287`).
    let mut ai = AgriAi::new(AgriTrade::Florist);
    let mut scene = florist(
        FloristFacts::ready(1)
            .with_ground(PlantGround::with_flower(Pos::new(1, 64, 1)))
            .with_compost(2),
    );
    let trace = ai.trace(&mut scene, 10, |_, index| {
        if index >= 7 {
            GoalAnswers::broken()
        } else if index >= 3 {
            GoalAnswers::arrived()
        } else {
            GoalAnswers::default()
        }
    });
    assert_eq!(
        trace.phases(),
        vec![
            Phase::Search,
            Phase::Search,
            Phase::Travel,
            Phase::Act,
            Phase::Search,
        ]
    );
    // Et les états, dans l'ordre : `FLORIST_HARVEST` apparaît **deux fois** parce que
    // Java ne le quitte pas pour marcher — c'est la phase qui a changé (`Travel` puis
    // `Act`), pas l'état. `harvest` rend `START_WORKING` (`:287`) au tick 8.
    assert_eq!(
        trace.java(),
        vec![
            "START_WORKING",
            "DECIDE",
            "FLORIST_HARVEST",
            "FLORIST_HARVEST",
            "START_WORKING",
        ]
    );
}

#[test]
fn the_dump_interrupts_a_working_florist_and_names_the_threshold() {
    // `AbstractEntityAIBasic.java:225` : l'événement passe avant la liste de l'état, donc
    // il interrompt `IDLE` au premier tick — dix cueillettes pour une hutte de niveau un
    // (`EntityAIWorkFlorist.java:326`).
    let mut ai = AgriAi::new(AgriTrade::Florist);
    let mut scene = florist(
        FloristFacts::ready(1).with_ground(PlantGround::with_flower(Pos::new(1, 64, 1))),
    );
    florist_scene(&mut scene).world.inventory.actions_done = 10;
    let outcome = ai.tick(&mut scene, GoalAnswers::default());
    assert_eq!(outcome.state, base(WorkerState::InventoryFull));
    assert_eq!(outcome.phase, Phase::Deliver);
    assert_eq!(outcome.rule(), Some("inventoryNeedsDump"));
    // Une action de moins, et le fleuriste continue de jardiner.
    let mut ai = AgriAi::new(AgriTrade::Florist);
    let mut scene = florist(
        FloristFacts::ready(1).with_ground(PlantGround::with_flower(Pos::new(1, 64, 1))),
    );
    florist_scene(&mut scene).world.inventory.actions_done = 9;
    let outcome = ai.tick(&mut scene, GoalAnswers::default());
    assert_eq!(outcome.state, base(WorkerState::StartWorking));
}

#[test]
fn after_the_dump_the_florist_returns_to_idle_and_starts_over() {
    // `AbstractEntityAIBasic.java:1213-1222` : `afterDump` rend `IDLE`, et
    // `clearActionsDone` remet le compteur à zéro.
    let mut ai = AgriAi::new(AgriTrade::Florist);
    let mut scene = florist(
        FloristFacts::ready(1).with_ground(PlantGround::with_flower(Pos::new(1, 64, 1))),
    );
    florist_scene(&mut scene).world.inventory.actions_done = 10;
    let dump = ai.tick(&mut scene, GoalAnswers::default());
    assert_eq!(dump.state, base(WorkerState::InventoryFull));
    assert_eq!(dump.rule(), Some("inventoryNeedsDump"));
    // La première lecture du vidage ne fait que retenir l'état : rien n'est transféré.
    let waiting = ai.tick(&mut scene, GoalAnswers::default());
    assert_eq!(waiting.state, base(WorkerState::InventoryFull));
    assert_eq!(waiting.refusal, Some(AgriRefusal::TransferIncomplete));
    // `dumpInventory` est une cible de vingt ticks : elle repasse au sixième tick de
    // machine (vingt ticks de jeu, cinq par tick d'IA).
    for _ in 0..3 {
        ai.tick(&mut scene, GoalAnswers::default());
    }
    // Transfert fini et compteur remis à zéro : retour à `IDLE`.
    florist_scene(&mut scene).world.inventory.actions_done = 0;
    let done = ai.tick(&mut scene, GoalAnswers::transferred());
    assert_eq!(done.state, base(WorkerState::Idle));
    assert_eq!(done.phase, Phase::Search);
    assert_eq!(done.rule(), Some("dumpInventory"));
}

#[test]
fn a_paused_florist_goes_to_paused_after_the_dump_and_comes_back() {
    // `AbstractEntityAIBasic.java:1215-1221` : `isPaused()` → `PAUSED` ;
    // `:251` : `PAUSED → IDLE` quand le joueur le relance.
    let mut ai = AgriAi::new(AgriTrade::Florist);
    let mut scene = florist(
        FloristFacts::ready(1).with_ground(PlantGround::with_flower(Pos::new(1, 64, 1))),
    );
    florist_scene(&mut scene).world.inventory.actions_done = 10;
    florist_scene(&mut scene).world.worker.paused = true;
    ai.tick(&mut scene, GoalAnswers::default());
    let paused = ai.tick(&mut scene, GoalAnswers::transferred());
    assert_eq!(paused.state, base(WorkerState::Paused));
    assert_eq!(paused.phase, Phase::Wait);
    // Relancé : le compteur est à zéro, il repart de `IDLE`.
    florist_scene(&mut scene).world.inventory.actions_done = 0;
    florist_scene(&mut scene).world.worker.paused = false;
    let back = ai.tick(&mut scene, GoalAnswers::default());
    assert_eq!(back.state, base(WorkerState::Idle));
    assert_eq!(back.rule(), Some("bePaused"));
}

#[test]
fn a_scene_of_another_trade_is_named_instead_of_guessed() {
    // Une scène de fermier donnée à un fleuriste : le port le dit, il ne cherche pas un
    // champ qui n'existe pas.
    let mut ai = AgriAi::new(AgriTrade::Florist);
    let mut scene = AgriScene::farmer(bare_world());
    let outcome = ai.tick(&mut scene, GoalAnswers::default());
    assert_eq!(outcome.refusal, Some(AgriRefusal::SceneForAnotherTrade));
    assert!(!outcome.moved());
    // Et l'inverse, pour que le test vaille dans les deux sens.
    let mut ai = AgriAi::new(AgriTrade::Farmer);
    let mut scene = florist(FloristFacts::ready(1));
    assert_eq!(
        ai.tick(&mut scene, GoalAnswers::default()).refusal,
        Some(AgriRefusal::SceneForAnotherTrade)
    );
}

#[test]
fn the_farmer_keeps_the_machine_it_already_had() {
    // Ce module ne réécrit pas le fermier : `AgriAi::new` rend la machine de
    // `crate::ai_gather`, et la suite d'états est la sienne
    // (`AbstractEntityAICrafting.java:164`, `EntityAIWorkFarmer.java:137`).
    let mut ai = AgriAi::new(AgriTrade::Farmer);
    assert_eq!(ai.trade(), Some(AgriTrade::Farmer));
    let mut scene = AgriScene::farmer(bare_world());
    let outcome = ai.tick(&mut scene, GoalAnswers::default());
    assert_eq!(outcome.state, base(WorkerState::StartWorking));
    assert_eq!(outcome.phase, Phase::Search);
    // La table du fermier est bien celle de `ai_gather` : la machine connaît
    // `FARMER_HOE`, que ce module n'enregistre pas.
    assert_eq!(ai.state(), base(WorkerState::StartWorking));
    ai.tick(&mut scene, GoalAnswers::default());
    assert_eq!(ai.state(), base(WorkerState::Preparing));
}

#[test]
fn the_fisherman_walks_to_its_hut_then_prepares_just_like_java() {
    // `EntityAIWorkFisherman.java:169-171` : `IDLE → START_WORKING`, puis
    // `startWorkingAtOwnBuilding` : `PREPARING` au pied de la hutte (`:197`), l'état
    // retenu tant qu'il n'y est pas (`:193-196`).
    let mut ai = AgriAi::new(AgriTrade::Fisherman);
    assert_eq!(ai.trade(), Some(AgriTrade::Fisherman));
    let mut scene = AgriScene::fisherman(bare_world());
    let first = ai.tick(&mut scene, GoalAnswers::default());
    assert_eq!(first.state, base(WorkerState::StartWorking));
    assert_eq!(first.rule(), Some("idleToEntry"));
    let second = ai.tick(&mut scene, GoalAnswers::default());
    assert_eq!(second.state, base(WorkerState::Preparing));
    assert_eq!(second.rule(), Some("startWorkingAtOwnBuilding"));
    assert!(second.moved());

    // Loin de sa hutte, il ne bouge pas.
    let mut world = bare_world();
    world.worker.at = Pos::new(40, 64, 0);
    let mut ai = AgriAi::new(AgriTrade::Fisherman);
    let mut scene = AgriScene::fisherman(world);
    ai.tick(&mut scene, GoalAnswers::default());
    let walking = ai.tick(&mut scene, GoalAnswers::default());
    assert_eq!(walking.state, base(WorkerState::StartWorking));
    assert!(!walking.moved());
}

#[test]
fn an_order_lives_for_one_tick_and_no_more() {
    // Les ordres au monde sortent du tick, pas d'un effet de bord : la scène est nettoyée
    // au début de chaque tick, donc un ordre non consommé ne survit pas au suivant.
    let mut ai = AgriAi::new(AgriTrade::Florist);
    let mut scene = florist(
        FloristFacts::ready(1).with_ground(PlantGround::with_flower(Pos::new(1, 64, 1))),
    );
    ai.tick(&mut scene, GoalAnswers::default()); // IDLE → START_WORKING
    ai.tick(&mut scene, GoalAnswers::default()); // → DECIDE
    let deciding = ai.tick(&mut scene, GoalAnswers::default()); // `decide` (`:131`)
    assert_eq!(deciding.state, AgriState::FloristHarvest);
    // Aucun compost nulle part : la requête de seize part avec la cueillette (`:183`).
    assert_eq!(
        deciding.works,
        vec![AgriWork::RequestCompost {
            quantity: florist::COMPOST_REQUEST_QTY
        }]
    );
    assert_eq!(deciding.refusal, Some(AgriRefusal::CompostRequested));
    let picking = ai.tick(&mut scene, GoalAnswers::arrived());
    assert_eq!(picking.state, AgriState::FloristHarvest);
    assert_eq!(picking.phase, Phase::Act);
    assert!(picking.works.is_empty(), "the request must not be re-issued");
}

#[test]
fn a_scene_that_never_changes_keeps_the_citizen_walking() {
    // `return getState()` : sans réponse du monde, la machine retient l'état et la trace
    // ne bouge pas. Vingt ticks de `FLORIST_HARVEST` en marche = trois pas, pas vingt.
    let mut ai = AgriAi::new(AgriTrade::Florist);
    let mut scene = florist(
        FloristFacts::ready(1).with_ground(PlantGround::with_flower(Pos::new(1, 64, 1))),
    );
    let trace = ai.trace_constant(&mut scene, 30, GoalAnswers::default());
    assert_eq!(
        trace.java(),
        vec!["START_WORKING", "DECIDE", "FLORIST_HARVEST"]
    );
    assert_eq!(trace.phases(), vec![Phase::Search, Phase::Search, Phase::Travel]);
    assert_eq!(ai.state(), AgriState::FloristHarvest);
}
