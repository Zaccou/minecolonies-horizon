//! Les scénarios de faits qui doivent rendre la même suite d'états que le Java.
//!
//! Chaque test nomme la ligne qu'il vérifie. Quand un test a dû être tranché, c'est
//! contre le Java — la ligne est dans le commentaire, et deux tests portent sur des
//! comportements que le Java a et qu'on n'attend pas : la monture réservée qui est
//! ramassée quand même (`EntityAIWorkStablemaster.java:753`) et l'absence de stalle qui
//! lève une `NullPointerException` (`:591` sur `BuildingStable.java:128-131`).

use crate::ai::facts::{Pos, WorldFacts};
use crate::ai::state::WorkerState;

use super::facts::{
    arrived, at_hut, HerdAct, HerdJob, HerdScene, MountMaintenance, PenAnimal, RabbitFacts,
    ShepherdFacts, StableFacts,
};
use super::state::{Hand, HerdChoice, HerdState, TradeRefusal};
use super::{rabbit, shepherd, stablemaster, Decision, HerdAi, HerdTrade};

// ---------------------------------------------------------------------------
// Outils
// ---------------------------------------------------------------------------

/// Une IA et sa scène vide, sans faits de métier particuliers.
fn empty(trade: HerdTrade) -> (HerdAi, HerdScene) {
    let ai = HerdAi::new(trade);
    let scene = ai.scene(WorldFacts::new());
    (ai, scene)
}

/// Une scène de berger avec une brebis à un bloc, des cisailles et la tonte active.
fn shearing_scene() -> (HerdAi, HerdScene) {
    let (ai, mut scene) = empty(HerdTrade::Shepherd);
    scene.job = HerdJob::Shepherd(ShepherdFacts {
        shearing: true,
        shears_slot: Some(3),
        ..ShepherdFacts::default()
    });
    scene.pen.animals.push(PenAnimal::sheep(7, Pos::new(1, 64, 0)));
    (ai, scene)
}

/// Une scène d'écurie dont les trois tirages de `decide` sont hauts : aucun des trois buts
/// n'est ouvert par le hasard.
fn calm_stable() -> (HerdAi, HerdScene) {
    let (ai, mut scene) = empty(HerdTrade::Stablemaster);
    scene.draws.training = 0.9;
    scene.draws.ready = 0.9;
    scene.draws.roundup = 0.9;
    (ai, scene)
}

/// Avance jusqu'au tick où la cible a réellement travaillé, et rend cet état.
///
/// Deux pièges expliquent cette forme. Une cible à dix ticks n'est **regardée** qu'un
/// tick d'IA sur deux (`10 / ENTITY_AI_TICKRATE`), donc deux appels à [`HerdAi::tick`]
/// séparent deux exécutions. Et une action qui rend `null` — le `return getState()` de
/// Java — ne produit aucun `Fired` : `fired.is_some()` ne dit donc pas qu'elle a tourné.
/// Ce qui le dit, c'est un changement d'état, un refus nommé ou un effet sur le monde.
fn until_work(ai: &mut HerdAi, scene: &mut HerdScene) -> super::Outcome {
    for _ in 0..16 {
        let outcome = ai.tick(scene);
        if outcome.fired.is_some() || outcome.why_not().is_some() || outcome.acted().is_some() {
            return outcome;
        }
    }
    panic!("la cible n'a pas travaillé en seize ticks d'IA");
}

// ---------------------------------------------------------------------------
// Les états, les réponses de base et les refus
// ---------------------------------------------------------------------------

#[test]
fn the_eight_trade_states_carry_their_java_name_and_eat_flag() {
    assert_eq!(HerdState::ALL.len(), 8);
    // `AIWorkerState.java` — le drapeau est celui du constructeur.
    assert!(!HerdState::HerderBreed.okay_to_eat()); // :357
    assert!(!HerdState::HerderButcher.okay_to_eat()); // :362
    assert!(HerdState::HerderPickup.okay_to_eat()); // :367
    assert!(!HerdState::HerderFeed.okay_to_eat()); // :372
    assert!(HerdState::HerderTrain.okay_to_eat()); // :381
    assert!(HerdState::HerderReadyForCombat.okay_to_eat()); // :386
    assert!(HerdState::HerderGatherMounts.okay_to_eat()); // :391
    assert!(!HerdState::ShepherdShear.okay_to_eat()); // :414

    let mut names: Vec<&str> = HerdState::ALL.iter().map(|state| state.java_name()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 8, "deux états portent le même nom");
    assert_eq!(HerdState::ShepherdShear.java_name(), "SHEPHERD_SHEAR");
    assert_eq!(
        HerdState::HerderGatherMounts.java_declaration(),
        "AIWorkerState.java:391"
    );
}

#[test]
fn a_base_state_delegates_its_eat_flag_and_its_name() {
    // `WorkerState::okay_to_eat` porte déjà les dix-huit valeurs : ce module ne les
    // réécrit pas.
    assert_eq!(
        HerdState::Base(WorkerState::Idle).okay_to_eat(),
        WorkerState::Idle.okay_to_eat()
    );
    assert!(!HerdState::Base(WorkerState::InventoryFull).okay_to_eat());
    assert_eq!(HerdState::Base(WorkerState::Decide).java_name(), "DECIDE");
    assert_eq!(
        HerdState::Base(WorkerState::Decide).java_declaration(),
        "AIWorkerState.java:59"
    );
    assert!(HerdState::Base(WorkerState::Decide).is_traversed());
    assert!(!HerdState::HerderTrain.is_traversed());
}

#[test]
fn the_base_decision_has_exactly_five_answers() {
    // `AbstractEntityAIHerder.decideWhatToDo` n'a que ces cinq sorties (`:232`, `:236`,
    // `:240`, `:244`, `:248`).
    let choices = [
        (HerdChoice::Pickup, HerdState::HerderPickup, ":232"),
        (HerdChoice::Butcher, HerdState::HerderButcher, ":236"),
        (HerdChoice::Breed, HerdState::HerderBreed, ":240"),
        (HerdChoice::Feed, HerdState::HerderFeed, ":244"),
        (
            HerdChoice::StartWorking,
            HerdState::Base(WorkerState::StartWorking),
            ":248",
        ),
    ];
    let mut seen = Vec::new();
    for (choice, state, line) in choices {
        assert_eq!(choice.state(), state);
        assert!(choice.java().ends_with(line), "{}", choice.java());
        seen.push(choice.java());
    }
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), 5);
}

#[test]
fn every_refusal_carries_a_line_and_a_unique_key() {
    let mut keys: Vec<&str> = TradeRefusal::ALL.iter().map(|refusal| refusal.key()).collect();
    assert_eq!(keys.len(), 26);
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), 26, "deux refus partagent une clé");
    for refusal in TradeRefusal::ALL {
        assert!(!refusal.java().is_empty());
        assert!(refusal.key().starts_with("herd.refusal."));
    }
    // Le refus propre au trou de Java : la stalle absente.
    assert_eq!(TradeRefusal::NoStall.java(), "BuildingStable.java:128-131");
}

// ---------------------------------------------------------------------------
// Les faits
// ---------------------------------------------------------------------------

#[test]
fn arrival_is_a_less_or_equal_test_like_walk_to_pos() {
    // `EntityNavigationUtils.java:174` — `dist <= distToDesired`, et
    // `AbstractEntityAIBasic.java:830` passe `4`.
    let facts = WorldFacts::new();
    assert!(arrived(&facts, Pos::new(2, 64, 0), 2));
    assert!(!arrived(&facts, Pos::new(3, 64, 0), 2));
    // La nuance avec `EntityUtils.isLivingAtSite`, qui est strict : à exactement deux
    // blocs, `walkToPos` dit arrivé et `isLivingAtSite(.., 2)` dit non.
    assert!(arrived(&facts, Pos::new(0, 66, 0), 2));
    assert!(!Pos::new(0, 64, 0).is_living_at_site(Pos::new(0, 66, 0), 2));
}

#[test]
fn the_hut_is_reached_within_the_building_range() {
    let (_, mut scene) = empty(HerdTrade::Stablemaster);
    assert!(at_hut(&scene));
    scene.facts.building.position = Pos::new(20, 64, 0);
    assert!(!at_hut(&scene));
    scene.facts.worker.at = Pos::new(14, 64, 0);
    assert!(at_hut(&scene)); // `WOKR_IN_BUILDING_DIST = 7`, strict
}

#[test]
fn a_sheep_is_only_shearable_once_and_only_as_an_adult() {
    let (_, mut scene) = empty(HerdTrade::Shepherd);
    scene.pen.animals.push(PenAnimal::sheep(1, Pos::new(1, 64, 0)));
    assert_eq!(scene.pen.shearable_sheep().map(|sheep| sheep.id), Some(1));

    scene.pen.animals[0].sheared = true;
    assert!(scene.pen.shearable_sheep().is_none());

    scene.pen.animals[0].sheared = false;
    scene.pen.animals[0].baby = true;
    assert!(scene.pen.shearable_sheep().is_none());

    // `instanceof Sheep` (`EntityAIWorkShepherd.java:101`) : une chèvre ne compte pas.
    scene.pen.animals[0].baby = false;
    scene.pen.animals[0].kind = super::facts::AnimalKind::Other;
    assert!(scene.pen.shearable_sheep().is_none());
}

#[test]
fn the_pen_answers_by_id_and_knows_when_it_is_empty() {
    let (_, mut scene) = empty(HerdTrade::Stablemaster);
    assert!(scene.pen.is_empty());
    scene.pen.animals.push(PenAnimal::horse(4, Pos::new(3, 64, 0)));
    assert!(!scene.pen.is_empty());
    assert_eq!(scene.pen.by_id(4).map(|animal| animal.at), Some(Pos::new(3, 64, 0)));
    assert!(scene.pen.by_id(5).is_none());
}

#[test]
fn a_scene_whose_facts_belong_to_another_trade_is_refused() {
    let mut ai = HerdAi::new(HerdTrade::RabbitHerder);
    let mut scene = ai.scene_with(
        WorldFacts::new(),
        HerdJob::Shepherd(ShepherdFacts::default()),
    );
    assert!(!scene.job_matches());
    scene.decide(HerdChoice::StartWorking);
    let outcome = ai.tick(&mut scene);
    assert_eq!(outcome.why_not(), Some(TradeRefusal::WrongTradeFacts));
    assert_eq!(outcome.goal(), HerdState::Base(WorkerState::Decide));
}

// ---------------------------------------------------------------------------
// L'enregistrement de la table
// ---------------------------------------------------------------------------

#[test]
fn the_rabbit_registers_no_trade_target() {
    // `EntityAIWorkRabbitHerder.java:32-35` — `super(job)` et rien d'autre.
    let ai = HerdAi::new(HerdTrade::RabbitHerder);
    for state in [
        HerdState::ShepherdShear,
        HerdState::HerderTrain,
        HerdState::HerderReadyForCombat,
        HerdState::HerderGatherMounts,
    ] {
        assert!(!ai.machine().can_leave(state), "{} est enregistré", state);
    }
}

#[test]
fn the_shepherd_registers_one_target_at_twenty_ticks() {
    // `EntityAIWorkShepherd.java:50` — `new AITarget(SHEPHERD_SHEAR, this::shearSheep,
    // TICKS_SECOND)`.
    let ai = HerdAi::new(HerdTrade::Shepherd);
    assert!(ai.machine().can_leave(HerdState::ShepherdShear));
    assert!(!ai.machine().can_leave(HerdState::HerderTrain));
    let shear = ai
        .machine()
        .transitions()
        .iter()
        .find(|transition| transition.rule == "shearSheep")
        .expect("la cible de tonte est enregistrée");
    assert_eq!(shear.tick_rate, 20);
    assert_eq!(shear.java, "EntityAIWorkShepherd.java:50");
}

#[test]
fn the_stablemaster_registers_three_targets_at_ten_ticks() {
    // `EntityAIWorkStablemaster.java:102-104`.
    let ai = HerdAi::new(HerdTrade::Stablemaster);
    for (rule, java) in [
        ("convertMount", "EntityAIWorkStablemaster.java:102"),
        ("readyMountForCombat", "EntityAIWorkStablemaster.java:103"),
        ("gatherMounts", "EntityAIWorkStablemaster.java:104"),
    ] {
        let target = ai
            .machine()
            .transitions()
            .iter()
            .find(|transition| transition.rule == rule)
            .expect("les trois cibles sont enregistrées");
        assert_eq!(target.tick_rate, 10, "{rule}");
        assert_eq!(target.java, java);
    }
    assert!(!ai.machine().can_leave(HerdState::ShepherdShear));
}

#[test]
fn the_decision_is_looked_at_every_deciding_delay() {
    // `AbstractEntityAIHerder.java:143` — `DECIDING_DELAY` vaut 80.
    let ai = HerdAi::new(HerdTrade::Shepherd);
    let decide = ai
        .machine()
        .transitions()
        .iter()
        .find(|transition| transition.rule == "decideWhatToDo (surcharge)")
        .expect("la surcharge est sur DECIDE");
    assert_eq!(decide.tick_rate, 80);
    assert_eq!(decide.java, "EntityAIWorkShepherd.java:73-87");
    assert!(ai.machine().can_leave(HerdState::Base(WorkerState::Decide)));
}

// ---------------------------------------------------------------------------
// Le lapinier
// ---------------------------------------------------------------------------

#[test]
fn the_rabbit_never_changes_the_base_answer() {
    // Il ne surcharge pas `decideWhatToDo` : la réponse du parent est la sienne.
    for choice in [
        HerdChoice::Pickup,
        HerdChoice::Butcher,
        HerdChoice::Breed,
        HerdChoice::Feed,
        HerdChoice::StartWorking,
    ] {
        let (_, mut scene) = empty(HerdTrade::RabbitHerder);
        scene.decide(choice);
        let decision = rabbit::decide_what_to_do(&mut scene, HerdState::Base(WorkerState::Decide));
        assert_eq!(decision.next, Some(choice.state()));
        assert_eq!(decision.refusal, None);
        assert!(scene.pen.base_answer.is_none(), "la réponse est consommée");
    }
}

#[test]
fn a_rabbit_decision_without_a_base_answer_is_refused() {
    let (_, mut scene) = empty(HerdTrade::RabbitHerder);
    let decision = rabbit::decide_what_to_do(&mut scene, HerdState::Base(WorkerState::Decide));
    assert_eq!(decision.next, None);
    assert_eq!(decision.refusal, Some(TradeRefusal::BaseDecisionMissing));
}

#[test]
fn the_hit_bound_shrinks_with_the_primary_skill() {
    // `EntityAIWorkRabbitHerder.java:61` — `1 + (100 - skill) / 5`, division entière.
    assert_eq!(rabbit::hit_bound(0), 21);
    assert_eq!(rabbit::hit_bound(50), 11);
    assert_eq!(rabbit::hit_bound(95), 2);
    // À 100 la borne vaut 1, donc `nextInt(1)` rend toujours 0 et la frappe touche.
    assert_eq!(rabbit::hit_bound(100), 1);
    // Au-dessus de 100, Java lèverait une `IllegalArgumentException` ; le port garde 1.
    assert_eq!(rabbit::hit_bound(140), 1);
}

#[test]
fn the_hit_lands_on_zero_or_one_only() {
    assert!(rabbit::hit_lands(0, 0));
    assert!(rabbit::hit_lands(0, 1));
    assert!(!rabbit::hit_lands(0, 2));
    // Un tirage hors de la borne ne vient pas de `nextInt`.
    assert!(!rabbit::hit_lands(0, 21));
    // À 100, seul 0 est possible.
    assert!(rabbit::hit_lands(100, 0));
    assert!(!rabbit::hit_lands(100, 1));
}

#[test]
fn a_rabbit_without_a_weapon_does_not_strike() {
    // `:57` — `!ItemStackUtils.isEmpty(worker.getMainHandItem())`.
    let (_, mut scene) = empty(HerdTrade::RabbitHerder);
    scene.job = HerdJob::Rabbit(RabbitFacts {
        main_hand_filled: false,
        ..RabbitFacts::default()
    });
    scene.pen.animals.push(PenAnimal::other(2, Pos::new(1, 64, 0)));
    assert_eq!(
        rabbit::butcher_animal(&mut scene, 2),
        Err(TradeRefusal::NoWeaponInHand)
    );
}

#[test]
fn a_rabbit_that_is_not_at_the_animal_does_not_strike() {
    let (_, mut scene) = empty(HerdTrade::RabbitHerder);
    scene.pen.animals.push(PenAnimal::other(2, Pos::new(9, 64, 0)));
    assert_eq!(
        rabbit::butcher_animal(&mut scene, 2),
        Err(TradeRefusal::NotAtAnimal)
    );
    // Un identifiant qui n'est plus dans l'enclos.
    assert_eq!(
        rabbit::butcher_animal(&mut scene, 99),
        Err(TradeRefusal::AnimalGone)
    );
}

#[test]
fn a_landed_hit_butchers_and_a_miss_does_not() {
    let (_, mut scene) = empty(HerdTrade::RabbitHerder);
    scene.pen.animals.push(PenAnimal::other(2, Pos::new(1, 64, 0)));
    // `:61` — le tirage 0 passe à toute habileté.
    assert_eq!(
        rabbit::butcher_animal(&mut scene, 2),
        Ok(HerdAct::Butchered { id: 2 })
    );
    assert_eq!(scene.memory.hand, Hand::Axe);
    assert_eq!(scene.memory.act, Some(HerdAct::Butchered { id: 2 }));

    // Le tirage 5 rate : rien n'est usé, la main ne change pas.
    scene.memory.hand = Hand::Empty;
    scene.memory.act = None;
    scene.draws.hit = 5;
    assert_eq!(
        rabbit::butcher_animal(&mut scene, 2),
        Err(TradeRefusal::HitMissed)
    );
    assert_eq!(scene.memory.hand, Hand::Empty);
    assert_eq!(scene.memory.act, None);
}

#[test]
fn the_rabbit_render_meta_adds_the_carrot_in_both_cases() {
    // `EntityAIWorkRabbitHerder.java:37-46`.
    let (_, mut scene) = empty(HerdTrade::RabbitHerder);
    assert_eq!(rabbit::render_meta(&scene), "working");
    scene.job = HerdJob::Rabbit(RabbitFacts {
        carrots: 3,
        ..RabbitFacts::default()
    });
    assert_eq!(rabbit::render_meta(&scene), "workingcarrot");
    scene.job = HerdJob::Rabbit(RabbitFacts {
        carrots: 3,
        idle: true,
        ..RabbitFacts::default()
    });
    assert_eq!(rabbit::render_meta(&scene), "carrot");
    scene.job = HerdJob::Rabbit(RabbitFacts {
        carrots: 0,
        idle: true,
        ..RabbitFacts::default()
    });
    assert_eq!(rabbit::render_meta(&scene), "");
}

#[test]
fn the_rabbit_keeps_the_parents_attack_damage() {
    // `AbstractEntityAIHerder.java:71` — le lapinier ne surcharge pas
    // `getButcheringAttackDamage()`.
    assert_eq!(rabbit::attack_damage(), 5.0);
    assert_eq!(rabbit::after_hit(true), HerdState::Base(WorkerState::Decide));
    assert_eq!(rabbit::after_hit(false), HerdState::HerderButcher);
}

// ---------------------------------------------------------------------------
// Le berger
// ---------------------------------------------------------------------------

#[test]
fn shearing_off_hands_the_decision_back() {
    // `EntityAIWorkShepherd.java:79` — le réglage est la première condition.
    let (_, mut scene) = shearing_scene();
    scene.job = HerdJob::Shepherd(ShepherdFacts {
        shearing: false,
        shears_slot: Some(3),
        ..ShepherdFacts::default()
    });
    scene.decide(HerdChoice::StartWorking);
    let decision = shepherd::decide_what_to_do(&mut scene, HerdState::Base(WorkerState::Decide));
    assert_eq!(
        decision.next,
        Some(HerdState::Base(WorkerState::StartWorking))
    );
    // `:84` — la main est vidée dans ce chemin.
    assert_eq!(scene.memory.hand, Hand::Empty);
}

#[test]
fn shearing_needs_the_base_answer_to_be_start_working() {
    // `:79` — `result.equals(START_WORKING)`. Un troupeau qui a mieux à faire passe
    // devant la tonte.
    for choice in [
        HerdChoice::Breed,
        HerdChoice::Butcher,
        HerdChoice::Feed,
        HerdChoice::Pickup,
    ] {
        let (_, mut scene) = shearing_scene();
        scene.decide(choice);
        let decision = shepherd::decide_what_to_do(&mut scene, HerdState::Base(WorkerState::Decide));
        assert_eq!(decision.next, Some(choice.state()));
        assert_eq!(scene.memory.hand, Hand::Empty);
    }
}

#[test]
fn shearing_needs_a_shearable_sheep() {
    // `:77-82` — `findShearableSheep() != null`.
    let (_, mut scene) = shearing_scene();
    scene.pen.animals[0].sheared = true;
    scene.decide(HerdChoice::StartWorking);
    let decision = shepherd::decide_what_to_do(&mut scene, HerdState::Base(WorkerState::Decide));
    assert_eq!(
        decision.next,
        Some(HerdState::Base(WorkerState::StartWorking))
    );
}

#[test]
fn a_shepherd_that_leaves_to_shear_keeps_its_shears_in_hand() {
    // `:81` est un `return` : `:84` n'est pas atteint, donc la main garde les cisailles
    // posées au tick précédent.
    let (_, mut scene) = shearing_scene();
    scene.memory.hand = Hand::Shears;
    scene.decide(HerdChoice::StartWorking);
    let decision = shepherd::decide_what_to_do(&mut scene, HerdState::Base(WorkerState::Decide));
    assert_eq!(decision.next, Some(HerdState::ShepherdShear));
    assert_eq!(scene.memory.hand, Hand::Shears);
}

#[test]
fn shearing_without_a_sheep_goes_back_to_decide() {
    // `:115-118`.
    let (_, mut scene) = shearing_scene();
    scene.pen.animals.clear();
    let decision = shepherd::shear_sheep(&mut scene, HerdState::ShepherdShear);
    assert_eq!(decision.next, Some(HerdState::Base(WorkerState::Decide)));
    assert_eq!(decision.refusal, Some(TradeRefusal::NoShearableSheep));
}

#[test]
fn shearing_without_shears_goes_to_preparing() {
    // `:120-123` — `equipTool` faux rend `PREPARING`, et `:309-315` d'
    // `AbstractEntityAIHerder` y fait la requête.
    let (_, mut scene) = shearing_scene();
    scene.job = HerdJob::Shepherd(ShepherdFacts {
        shearing: true,
        shears_slot: None,
        ..ShepherdFacts::default()
    });
    let decision = shepherd::shear_sheep(&mut scene, HerdState::ShepherdShear);
    assert_eq!(decision.next, Some(HerdState::Base(WorkerState::Preparing)));
    assert_eq!(decision.refusal, Some(TradeRefusal::NoShears));
    assert_eq!(scene.facts.inventory.actions_done, 0);
}

#[test]
fn shearing_waits_for_the_sheep_it_walks_to() {
    // `:127-130` — `walkingToAnimal` vrai rend `getState()`, donc la tonte reste l'état
    // et la cadence de vingt ticks est consommée par la marche.
    let (_, mut scene) = shearing_scene();
    scene.pen.animals[0].at = Pos::new(9, 64, 0);
    let decision = shepherd::shear_sheep(&mut scene, HerdState::ShepherdShear);
    assert_eq!(decision.next, None);
    assert_eq!(decision.refusal, Some(TradeRefusal::NotAtAnimal));
    assert!(scene.memory.act.is_none());
}

#[test]
fn shearing_lands_the_wool_and_counts_one_action() {
    // `:140-165`.
    let (_, mut scene) = shearing_scene();
    // `:141` — `1 + nextInt(enchantmentLevel + 1)`, et `:132-133` multiplie Fortune par
    // l'habileté : sans enchantement la borne vaut 1, donc la quantité est **toujours** 1,
    // quel que soit le tirage. Le scénario donne donc une cisaille Fortune III et 30
    // d'habileté primaire — 3 × (30 / 5,0) = 18 — pour que le tirage puisse monter.
    scene.job = HerdJob::Shepherd(ShepherdFacts {
        shearing: true,
        shears_slot: Some(3),
        shears_fortune: 3,
        primary_skill: 30,
        ..ShepherdFacts::default()
    });
    scene.draws.wool = 2;
    let decision = shepherd::shear_sheep(&mut scene, HerdState::ShepherdShear);
    assert_eq!(decision.next, Some(HerdState::Base(WorkerState::Decide)));
    assert_eq!(
        scene.memory.act,
        Some(HerdAct::Sheared {
            id: 7,
            wool: 3,
            dyed: None
        })
    );
    assert_eq!(scene.facts.inventory.actions_done, 1);
    assert_eq!(scene.memory.hand, Hand::Shears);
}

#[test]
fn the_fortune_multiplier_is_a_truncated_double_product() {
    // `:132-133` — `int *= Math.max(1.0, primary / 5.0)`.
    assert_eq!(shepherd::fortune_scaled(3, 30), 18);
    assert_eq!(shepherd::fortune_scaled(3, 0), 3);
    assert_eq!(shepherd::fortune_scaled(3, 4), 3);
    assert_eq!(shepherd::fortune_scaled(0, 100), 0);
    assert_eq!(shepherd::fortune_scaled(1, 7), 1);
}

#[test]
fn the_wool_quantity_is_one_plus_the_draw() {
    // `:141` — `1 + nextInt(enchantmentLevel + 1)`.
    assert_eq!(shepherd::wool_qty(18, 0), 1);
    assert_eq!(shepherd::wool_qty(18, 5), 6);
    assert_eq!(shepherd::wool_qty(18, 18), 19);
    // Un tirage hors borne ne vient pas de `nextInt`.
    assert_eq!(shepherd::wool_qty(18, 40), 19);
    assert_eq!(shepherd::wool_qty(0, 9), 1);
}

#[test]
fn dyeing_passes_on_the_building_level_and_stops_there() {
    // `:180` — `rand <= chanceToDye`, chanceToDye = niveau.
    let level_one = ShepherdFacts {
        dyeing: true,
        building_level: 1,
        ..ShepherdFacts::default()
    };
    assert_eq!(shepherd::dye_sheep_chance(level_one, 0, 4), Some(4));
    assert_eq!(shepherd::dye_sheep_chance(level_one, 1, 4), Some(4));
    assert_eq!(shepherd::dye_sheep_chance(level_one, 2, 4), None);

    // `:175` — le réglage ferme la teinture.
    let no_dye = ShepherdFacts {
        dyeing: false,
        ..level_one
    };
    assert_eq!(shepherd::dye_sheep_chance(no_dye, 0, 4), None);

    // `:182-184` — seize couleurs.
    let level_five = ShepherdFacts {
        dyeing: true,
        building_level: 5,
        ..ShepherdFacts::default()
    };
    assert_eq!(shepherd::dye_sheep_chance(level_five, 5, 3), Some(3));
    assert_eq!(shepherd::dye_sheep_chance(level_five, 6, 3), None);
    assert_eq!(shepherd::dye_sheep_chance(level_five, 0, 16), Some(0));
}

#[test]
fn the_shepherd_butchers_for_skill_over_ten() {
    // `:90-93` — `Math.max(1.0, secondary / 10.0)`.
    assert_eq!(shepherd::attack_damage(0), 1.0);
    assert_eq!(shepherd::attack_damage(9), 1.0);
    assert_eq!(shepherd::attack_damage(25), 2.5);
    assert_eq!(shepherd::attack_damage(100), 10.0);
    // Le parent, pour comparaison : `AbstractEntityAIHerder.java:71`.
    assert_eq!(rabbit::attack_damage(), 5.0);
}

#[test]
fn the_shears_join_the_axe_when_shearing_is_on() {
    // `EntityAIWorkShepherd.java:56-64` : la liste du parent contient déjà la hache
    // (`AbstractEntityAIHerder.java:176-181`), et `prepareForHerding` exige toutes les
    // entrées (`:309-315`).
    assert!(!shepherd::needs_shears(false));
    assert!(shepherd::needs_shears(true));
}

// ---------------------------------------------------------------------------
// Le palefrenier : la décision
// ---------------------------------------------------------------------------

#[test]
fn the_first_decision_never_trains() {
    // `:76` initialise `trainingCooldown` à 50 et `:235` le décrémente **avant** le test
    // de `:238` : le premier passage le met à 49, donc `<= 0` est faux.
    let (mut ai, mut scene) = calm_stable();
    scene.decide(HerdChoice::StartWorking);
    ai.tick(&mut scene);
    assert_eq!(scene.memory.train_cooldown, 49);
    assert_eq!(scene.memory.roundup_cooldown, 99);
    assert_eq!(
        ai.state(),
        HerdState::Base(WorkerState::StartWorking),
        "aucun des trois buts ne s'ouvre"
    );
}

#[test]
fn training_fires_at_the_end_of_its_cooldown_and_resets() {
    // `:238-242`.
    let (_, mut scene) = calm_stable();
    scene.memory.train_cooldown = 1;
    scene.draws.training = 0.24;
    scene.decide(HerdChoice::StartWorking);
    let decision =
        stablemaster::decide_what_to_do(&mut scene, HerdState::Base(WorkerState::Decide));
    assert_eq!(decision.next, Some(HerdState::HerderTrain));
    assert_eq!(scene.memory.train_cooldown, stablemaster::MAX_TRAINING_COOLDOWN);

    // Un tirage au-dessus de la chance ne l'ouvre pas.
    scene.memory.train_cooldown = 1;
    scene.draws.training = 0.25;
    scene.decide(HerdChoice::StartWorking);
    let decision =
        stablemaster::decide_what_to_do(&mut scene, HerdState::Base(WorkerState::Decide));
    assert_eq!(
        decision.next,
        Some(HerdState::Base(WorkerState::StartWorking))
    );
}

#[test]
fn the_ready_draw_is_consumed_at_every_decision() {
    // `:244-247` — pas de compte à rebours sur ce tirage.
    let (_, mut scene) = calm_stable();
    scene.draws.ready = 0.39;
    scene.decide(HerdChoice::StartWorking);
    let decision =
        stablemaster::decide_what_to_do(&mut scene, HerdState::Base(WorkerState::Decide));
    assert_eq!(decision.next, Some(HerdState::HerderReadyForCombat));
    // Le compte à rebours d'entraînement n'a pas bougé : 50 - 1.
    assert_eq!(scene.memory.train_cooldown, 49);
}

#[test]
fn the_roundup_needs_its_cooldown_and_a_horse() {
    // `:250-259`.
    let (_, mut scene) = calm_stable();
    scene.memory.roundup_cooldown = 1;
    scene.draws.roundup = 0.14;
    scene.pen.animals.push(PenAnimal::horse(6, Pos::new(4, 64, 0)));
    scene.decide(HerdChoice::StartWorking);
    let decision =
        stablemaster::decide_what_to_do(&mut scene, HerdState::Base(WorkerState::Decide));
    assert_eq!(decision.next, Some(HerdState::HerderGatherMounts));
    assert_eq!(scene.memory.wandering, vec![6]);
    assert_eq!(scene.memory.roundup_cooldown, stablemaster::MAX_ROUNDUP_COOLDOWN);
}

#[test]
fn the_roundup_list_is_assigned_even_when_it_is_empty() {
    // `:253` est une affectation, pas un ajout : une liste vide écrase la précédente.
    let (_, mut scene) = calm_stable();
    scene.memory.wandering = vec![9, 10];
    scene.memory.roundup_cooldown = 0;
    scene.draws.roundup = 0.0;
    scene.decide(HerdChoice::StartWorking);
    let decision =
        stablemaster::decide_what_to_do(&mut scene, HerdState::Base(WorkerState::Decide));
    assert_eq!(
        decision.next,
        Some(HerdState::Base(WorkerState::StartWorking))
    );
    assert!(scene.memory.wandering.is_empty());
}

#[test]
fn the_cooldowns_are_counted_in_decisions_and_that_is_eight_thousand_ticks() {
    // `:75` × `AbstractEntityAIHerder.java:143` et `:72` × la même ligne.
    assert_eq!(stablemaster::train_cooldown_ticks(), 4000);
    assert_eq!(stablemaster::roundup_cooldown_ticks(), 8000);
    assert_eq!(stablemaster::train_cooldown_ticks() / 20, 200, "secondes");
    assert_eq!(stablemaster::MAX_TRAINING_COOLDOWN, 50);
    assert_eq!(stablemaster::MAX_ROUNDUP_COOLDOWN, 100);
}

// ---------------------------------------------------------------------------
// Le palefrenier : le dressage
// ---------------------------------------------------------------------------

#[test]
fn a_training_candidate_is_a_living_adult_horse_without_a_rider() {
    // `:306-335` — premier `a` tel que pas déjà monture, `AbstractHorse`, vivant,
    // adulte, sans passager.
    let (_, mut scene) = calm_stable();
    scene.pen.animals.push(PenAnimal::cavalry(1, Pos::new(2, 64, 0)));
    scene.pen.animals.push(PenAnimal::horse(2, Pos::new(3, 64, 0)));
    scene.pen.animals[1].baby = true;
    scene.pen.animals.push(PenAnimal::horse(3, Pos::new(4, 64, 0)));
    scene.pen.animals[2].passengers = 1;
    scene.pen.animals.push(PenAnimal::horse(4, Pos::new(5, 64, 0)));
    assert_eq!(stablemaster::first_training_candidate(&scene), Some(4));

    scene.pen.animals[3].alive = false;
    assert_eq!(stablemaster::first_training_candidate(&scene), None);
}

#[test]
fn the_mount_limit_is_two_per_building_level() {
    // `:270` — `Math.max(0, getBuildingLevel() * 2)`.
    assert_eq!(stablemaster::mount_limit(0), 0);
    assert_eq!(stablemaster::mount_limit(1), 2);
    assert_eq!(stablemaster::mount_limit(5), 10);
}

#[test]
fn training_walks_before_it_converts() {
    // `:273-295`.
    let (_, mut scene) = calm_stable();
    scene.pen.animals.push(PenAnimal::horse(4, Pos::new(1, 64, 0)));
    scene.pen.animals.push(PenAnimal::horse(5, Pos::new(12, 64, 0)));
    scene.memory.to_convert = Some(5);
    let far = stablemaster::convert_mount(&mut scene, HerdState::HerderTrain);
    assert_eq!(far.next, None, "la marche continue dans le même état");
    assert_eq!(far.refusal, Some(TradeRefusal::NotAtMount));
    assert_eq!(scene.facts.inventory.actions_done, 0);

    // Arrivé : la conversion, une action comptée.
    scene.pen.animals[1].at = Pos::new(2, 64, 0);
    let landed = stablemaster::convert_mount(&mut scene, HerdState::HerderTrain);
    assert_eq!(landed.next, Some(HerdState::Base(WorkerState::Decide)));
    assert_eq!(scene.memory.act, Some(HerdAct::Converted { id: 5 }));
    assert_eq!(scene.memory.to_convert, None);
    assert_eq!(scene.facts.inventory.actions_done, 1);
}

#[test]
fn a_full_stable_does_not_train() {
    // `:297-304` — `current >= limit` rend `DECIDE`.
    let (_, mut scene) = calm_stable();
    scene.job = HerdJob::Stable(StableFacts {
        mounts: 2,
        building_level: 1,
        ..StableFacts::default()
    });
    scene.pen.animals.push(PenAnimal::horse(4, Pos::new(1, 64, 0)));
    let decision = stablemaster::convert_mount(&mut scene, HerdState::HerderTrain);
    assert_eq!(decision.next, Some(HerdState::Base(WorkerState::Decide)));
    assert_eq!(decision.refusal, Some(TradeRefusal::MountLimitReached));
    assert_eq!(scene.memory.to_convert, None);
}

#[test]
fn a_horse_that_cannot_be_converted_is_reported_not_retried() {
    // `:279-295` — `createFromVanilla` rend `null`, Java avertit puis rend `DECIDE` et
    // remet `horseToConvert` à `null` (`:293`) : la bête n'est pas réessayée en boucle.
    let (_, mut scene) = calm_stable();
    scene.pen.animals.push(PenAnimal::horse(4, Pos::new(1, 64, 0)));
    scene.pen.animals[0].convertible = false;
    scene.memory.to_convert = Some(4);
    let decision = stablemaster::convert_mount(&mut scene, HerdState::HerderTrain);
    assert_eq!(decision.next, Some(HerdState::Base(WorkerState::Decide)));
    assert_eq!(decision.refusal, Some(TradeRefusal::MountNotConvertible));
    assert_eq!(scene.memory.to_convert, None);
    assert!(scene.memory.act.is_none());
}

#[test]
fn a_lost_horse_is_dropped_from_the_memory() {
    // `:273` — la bête choisie n'est plus dans l'enclos.
    let (_, mut scene) = calm_stable();
    scene.memory.to_convert = Some(42);
    let decision = stablemaster::convert_mount(&mut scene, HerdState::HerderTrain);
    assert_eq!(decision.next, Some(HerdState::Base(WorkerState::Decide)));
    assert_eq!(decision.refusal, Some(TradeRefusal::MountGone));
    assert_eq!(scene.memory.to_convert, None);
}

// ---------------------------------------------------------------------------
// Le palefrenier : la préparation au combat
// ---------------------------------------------------------------------------

#[test]
fn health_training_closes_the_tick() {
    // `:361-374`.
    let (_, mut scene) = calm_stable();
    scene.pen.animals.push(PenAnimal::cavalry(8, Pos::new(1, 64, 0)));
    scene.pen.animals[0].needs_health_training = true;
    scene.memory.to_get_ready = Some(8);
    let decision = stablemaster::ready_mount_for_combat(&mut scene, HerdState::HerderReadyForCombat);
    assert_eq!(decision.next, Some(HerdState::Base(WorkerState::Decide)));
    assert_eq!(scene.memory.act, Some(HerdAct::HealthTrained { id: 8 }));
    assert_eq!(scene.memory.to_get_ready, None);
    assert_eq!(scene.facts.inventory.actions_done, 1);
}

#[test]
fn feeding_takes_the_item_from_the_hut_when_the_worker_has_none() {
    // `:438-451` — la hutte fournit, sans attendre l'arrivée.
    let (_, mut scene) = calm_stable();
    scene.job = HerdJob::Stable(StableFacts {
        feed_in_inventory: 0,
        feed_in_hut: 2,
        ..StableFacts::default()
    });
    scene.pen.animals.push(PenAnimal::cavalry(8, Pos::new(1, 64, 0)));
    scene.pen.animals[0].health = 10.0;
    scene.pen.animals[0].combat_cooldown = 0.0; // seule la faim reste
    scene.memory.to_get_ready = Some(8);
    let decision = stablemaster::ready_mount_for_combat(&mut scene, HerdState::HerderReadyForCombat);
    assert_eq!(decision.next, Some(HerdState::Base(WorkerState::Decide)));
    assert_eq!(
        scene.memory.act,
        Some(HerdAct::Fed {
            id: 8,
            healed: true
        })
    );
    assert_eq!(scene.memory.hand, Hand::Feed);
    assert_eq!(scene.facts.inventory.actions_done, 1);
    assert_eq!(scene.memory.to_get_ready, None);
}

#[test]
fn without_feed_anywhere_the_worker_asks_once_and_does_nothing() {
    // `:504-511` — la requête ne part que s'il n'y en a pas déjà une.
    let (_, mut scene) = calm_stable();
    scene.pen.animals.push(PenAnimal::cavalry(8, Pos::new(1, 64, 0)));
    scene.pen.animals[0].health = 10.0;
    scene.pen.animals[0].combat_cooldown = 0.0;
    scene.memory.to_get_ready = Some(8);
    let decision = stablemaster::ready_mount_for_combat(&mut scene, HerdState::HerderReadyForCombat);
    assert_eq!(
        scene.memory.act,
        Some(HerdAct::RequestedItems {
            task: MountMaintenance::Feeding
        })
    );
    assert_eq!(scene.refusal, Some(TradeRefusal::NoFeedItem));
    assert_eq!(decision.next, Some(HerdState::Base(WorkerState::Decide)));
    assert_eq!(scene.facts.inventory.actions_done, 0);
    assert_eq!(
        scene.memory.to_get_ready,
        Some(8),
        "didWork faux laisse la bête choisie"
    );

    // Une requête déjà en vol : aucune seconde.
    let (_, mut scene) = calm_stable();
    scene.job = HerdJob::Stable(StableFacts {
        open_stacklist_request: true,
        ..StableFacts::default()
    });
    scene.pen.animals.push(PenAnimal::cavalry(8, Pos::new(1, 64, 0)));
    scene.pen.animals[0].health = 10.0;
    scene.pen.animals[0].combat_cooldown = 0.0;
    scene.memory.to_get_ready = Some(8);
    stablemaster::ready_mount_for_combat(&mut scene, HerdState::HerderReadyForCombat);
    assert!(scene.memory.act.is_none());
}

#[test]
fn leather_reduces_the_combat_cooldown_and_armour_zeroes_it() {
    // `:476-499`.
    let (_, mut scene) = calm_stable();
    scene.job = HerdJob::Stable(StableFacts {
        leather_in_inventory: 3,
        primary_skill: 20,
        ..StableFacts::default()
    });
    scene.pen.animals.push(PenAnimal::cavalry(8, Pos::new(1, 64, 0)));
    scene.memory.to_get_ready = Some(8);
    let decision = stablemaster::ready_mount_for_combat(&mut scene, HerdState::HerderReadyForCombat);
    // À 20 d'habileté la récupération vaut 4.0, donc 20 - 4 = 16 > 13.2 : pas encore
    // prête (`CavalryHorseEntity.java:970-975`).
    assert_eq!(
        scene.memory.act,
        Some(HerdAct::Readied {
            id: 8,
            ready: false
        })
    );
    assert_eq!(scene.memory.hand, Hand::Leather);
    assert_eq!(decision.next, Some(HerdState::Base(WorkerState::Decide)));

    // `:480-483` — l'armure de cheval en cuir pose zéro.
    scene.job = HerdJob::Stable(StableFacts {
        leather_in_inventory: 1,
        leather_horse_armor: true,
        ..StableFacts::default()
    });
    scene.memory.to_get_ready = Some(8);
    stablemaster::ready_mount_for_combat(&mut scene, HerdState::HerderReadyForCombat);
    assert_eq!(
        scene.memory.act,
        Some(HerdAct::Readied {
            id: 8,
            ready: true
        })
    );
}

#[test]
fn the_leather_a_mount_needs_is_a_counted_duration() {
    // `:486` et `CavalryHorseEntity.java:107` : le seuil est `maxHealth * .66`.
    assert_eq!(stablemaster::combat_recovery(20), 4.0);
    assert_eq!(stablemaster::combat_recovery(40), 8.0);
    // À 0 d'habileté la récupération est nulle : Java ne le corrige pas, donc aucun
    // nombre d'objets ne suffit.
    assert_eq!(stablemaster::combat_recovery(0), 0.0);
    assert_eq!(stablemaster::leathers_to_ready(20, 20.0), Some(2));
    assert_eq!(stablemaster::leathers_to_ready(40, 20.0), Some(1));
    assert_eq!(stablemaster::leathers_to_ready(0, 20.0), None);
    // Le soin d'un nourrissage, `:523`.
    assert!((stablemaster::heal_amount(20.0) - 4.0).abs() < 0.001);
    assert!(!stablemaster::feed_horse(PenAnimal::cavalry(
        1,
        Pos::new(0, 64, 0)
    )));
}

#[test]
fn a_mount_that_needs_nothing_is_not_chosen() {
    // `:406-421` — les trois conditions de `:412`.
    let (_, mut scene) = calm_stable();
    scene.pen.animals.push(PenAnimal::cavalry(8, Pos::new(1, 64, 0)));
    scene.pen.animals[0].combat_cooldown = 0.0;
    assert_eq!(stablemaster::mount_to_ready(&scene), None);
    scene.pen.animals[0].health = 12.0;
    assert_eq!(stablemaster::mount_to_ready(&scene), Some(8));

    // Une monture qui a besoin de son bonus de santé compte aussi.
    let (_, mut scene) = calm_stable();
    scene.pen.animals.push(PenAnimal::cavalry(9, Pos::new(1, 64, 0)));
    scene.pen.animals[0].combat_cooldown = 0.0;
    scene.pen.animals[0].needs_health_training = true;
    assert_eq!(stablemaster::mount_to_ready(&scene), Some(9));
}

#[test]
fn without_a_herding_module_the_search_is_refused() {
    // `:398-404`.
    let (_, mut scene) = calm_stable();
    scene.job = HerdJob::Stable(StableFacts {
        has_herding_module: false,
        ..StableFacts::default()
    });
    let decision = stablemaster::ready_mount_for_combat(&mut scene, HerdState::HerderReadyForCombat);
    assert_eq!(decision.next, Some(HerdState::Base(WorkerState::Decide)));
    assert_eq!(decision.refusal, Some(TradeRefusal::NoHerdingModule));
}

#[test]
fn a_rider_that_mounts_during_the_walk_ends_the_goal() {
    // `:349-358`.
    let (_, mut scene) = calm_stable();
    scene.pen.animals.push(PenAnimal::cavalry(8, Pos::new(30, 64, 0)));
    scene.pen.animals[0].has_cavalry_rider = true;
    scene.memory.to_get_ready = Some(8);
    let decision = stablemaster::ready_mount_for_combat(&mut scene, HerdState::HerderReadyForCombat);
    assert_eq!(decision.next, Some(HerdState::Base(WorkerState::Decide)));
    assert_eq!(decision.refusal, Some(TradeRefusal::MountRidden));
    // `:352-355` est un `return DECIDE` **sans** remise à `null` : `horseToGetReady` survit
    // à ce chemin. Les deux autres sorties de la méthode l'effacent, elles — `:372` (bonus
    // de santé appliqué) et `:390-392` (entretien fait) —, et c'est la différence que ce
    // test mesure. Java garde donc la bête choisie après avoir renoncé à cause du cavalier.
    assert_eq!(scene.memory.to_get_ready, Some(8));
}

// ---------------------------------------------------------------------------
// Le palefrenier : la rentrée au bercail
// ---------------------------------------------------------------------------

#[test]
fn a_reserved_mount_outside_the_stable_is_still_rounded_up() {
    // `:753` teste `horseToRetrieve`, pas `horse` : le filtre de réservation ne mord
    // jamais sur un candidat. Le port reproduit le Java et le mesure.
    let (_, mut scene) = calm_stable();
    scene.pen.animals.push(PenAnimal::cavalry(3, Pos::new(4, 64, 0)));
    scene.pen.animals[0].has_reservation = true;
    assert_eq!(stablemaster::find_nearby_unstabled_horses(&scene), vec![3]);
}

#[test]
fn the_roundup_skips_riders_stabled_and_dead_horses_and_the_far_ones() {
    // `:749-754`.
    let (_, mut scene) = calm_stable();
    scene.pen.animals.push(PenAnimal::horse(1, Pos::new(2, 64, 0)));
    scene.pen.animals.push(PenAnimal::cavalry(2, Pos::new(3, 64, 0)));
    scene.pen.animals[1].has_cavalry_rider = true;
    scene.pen.animals.push(PenAnimal::cavalry(3, Pos::new(4, 64, 0)));
    scene.pen.animals[2].in_stable = true;
    scene.pen.animals.push(PenAnimal::horse(4, Pos::new(5, 64, 0)));
    scene.pen.animals[3].removed = true;
    scene.pen.animals.push(PenAnimal::horse(5, Pos::new(6, 64, 0)));
    scene.pen.animals[4].alive = false;
    // `:737` — rayon 30, donc 31 blocs est dehors.
    scene.pen.animals.push(PenAnimal::horse(6, Pos::new(31, 64, 0)));
    scene.pen.animals.push(PenAnimal::horse(7, Pos::new(29, 64, 0)));
    assert_eq!(stablemaster::find_nearby_unstabled_horses(&scene), vec![1, 7]);
}

#[test]
fn gathering_walks_to_the_horse_and_leashes_it() {
    // `:620-632`.
    let (_, mut scene) = calm_stable();
    scene.pen.animals.push(PenAnimal::horse(5, Pos::new(2, 64, 0)));
    scene.memory.to_retrieve = Some(5);
    let decision = stablemaster::gather_mounts(&mut scene, HerdState::HerderGatherMounts);
    assert_eq!(decision.next, Some(HerdState::HerderGatherMounts));
    assert_eq!(scene.memory.act, Some(HerdAct::Leashed { id: 5 }));
    assert!(scene.memory.lead_in_offhand);
    assert_eq!(scene.memory.hand, Hand::Lead);

    // `:589` — `if (horseToRetrieve.isLeashed())` passe **avant** la marche et
    // `attachHorse` (`:620-625`). Une bête déjà à la laisse ne repasse donc jamais par
    // `attachHorse` : c'est la branche de la stalle qui la prend, et sans stalle étiquetée
    // (`getNextStallPosition()` nul, `BuildingStable.java:128-131`) c'est `:591` qui
    // décide. Le test de `:677` se fait donc sur `attachHorse` elle-même, juste après.
    scene.pen.animals[0].leashed = true;
    scene.pen.animals[0].leash_holder = true;
    scene.memory.act = None;
    let leashed = stablemaster::gather_mounts(&mut scene, HerdState::HerderGatherMounts);
    assert_eq!(leashed.next, Some(HerdState::Base(WorkerState::Decide)));
    assert_eq!(leashed.refusal, Some(TradeRefusal::NoStall));

    // `:677` — `horse.isLeashed() && worker.equals(horse.getLeashHolder())` : rien à
    // faire, et c'est un succès (`return true`), sans effet sur le monde.
    scene.memory.act = None;
    let horse = scene.pen.by_id(5).expect("la bête est dans l'enclos");
    assert!(stablemaster::attach_horse(&mut scene, horse));
    assert!(scene.memory.act.is_none());
}

#[test]
fn a_horse_too_far_to_reach_keeps_the_goal_waiting() {
    // `:620-623`.
    let (_, mut scene) = calm_stable();
    scene.pen.animals.push(PenAnimal::horse(5, Pos::new(20, 64, 0)));
    scene.memory.to_retrieve = Some(5);
    let decision = stablemaster::gather_mounts(&mut scene, HerdState::HerderGatherMounts);
    assert_eq!(decision.next, None);
    assert_eq!(decision.refusal, Some(TradeRefusal::NotAtMount));
}

#[test]
fn leading_the_horse_to_the_stall_completes_the_roundup() {
    // `:589-618`.
    let (_, mut scene) = calm_stable();
    scene.job = HerdJob::Stable(StableFacts {
        stall: Some(Pos::new(0, 64, 2)),
        ..StableFacts::default()
    });
    scene.pen.animals.push(PenAnimal::horse(5, Pos::new(1, 64, 0)));
    scene.pen.animals[0].leashed = true;
    scene.pen.animals[0].leash_holder = true;
    scene.memory.to_retrieve = Some(5);
    scene.memory.wandering = vec![5];
    scene.memory.lead_in_offhand = true;
    let decision = stablemaster::gather_mounts(&mut scene, HerdState::HerderGatherMounts);
    assert_eq!(decision.next, Some(HerdState::Base(WorkerState::Decide)));
    // La rentrée remplace le relâchement qu'elle implique : le dernier effet gagne.
    assert_eq!(scene.memory.act, Some(HerdAct::RoundupDone));
    assert_eq!(scene.facts.inventory.actions_done, 1);
    assert!(scene.memory.wandering.is_empty());
    assert_eq!(scene.memory.to_retrieve, None);
    assert!(!scene.memory.lead_in_offhand);
}

#[test]
fn a_stable_without_a_stall_is_refused_instead_of_crashing() {
    // `:591` sur `BuildingStable.java:128-131` : Java déréférence un `null` ici.
    let (_, mut scene) = calm_stable();
    scene.pen.animals.push(PenAnimal::horse(5, Pos::new(1, 64, 0)));
    scene.pen.animals[0].leashed = true;
    scene.memory.to_retrieve = Some(5);
    scene.memory.wandering = vec![5];
    let decision = stablemaster::gather_mounts(&mut scene, HerdState::HerderGatherMounts);
    assert_eq!(decision.next, Some(HerdState::Base(WorkerState::Decide)));
    assert_eq!(decision.refusal, Some(TradeRefusal::NoStall));
    assert_eq!(scene.memory.to_retrieve, None);
}

#[test]
fn a_walk_to_the_stall_that_is_not_finished_keeps_the_goal() {
    // `:591-596`.
    let (_, mut scene) = calm_stable();
    scene.job = HerdJob::Stable(StableFacts {
        stall: Some(Pos::new(0, 64, 20)),
        ..StableFacts::default()
    });
    scene.pen.animals.push(PenAnimal::horse(5, Pos::new(1, 64, 0)));
    scene.pen.animals[0].leashed = true;
    scene.memory.to_retrieve = Some(5);
    let decision = stablemaster::gather_mounts(&mut scene, HerdState::HerderGatherMounts);
    assert_eq!(decision.next, None);
    assert_eq!(decision.refusal, Some(TradeRefusal::NotAtStall));
}

#[test]
fn a_missing_lead_sends_the_worker_back_to_preparing() {
    // `:578-587` — `checkForToolOrWeapon(lead)` vrai vide la bête suivie et rend
    // `PREPARING`.
    let (_, mut scene) = calm_stable();
    scene.job = HerdJob::Stable(StableFacts {
        lead_ok: false,
        ..StableFacts::default()
    });
    scene.pen.animals.push(PenAnimal::horse(5, Pos::new(1, 64, 0)));
    scene.memory.to_retrieve = Some(5);
    let decision = stablemaster::gather_mounts(&mut scene, HerdState::HerderGatherMounts);
    assert_eq!(decision.next, Some(HerdState::Base(WorkerState::Preparing)));
    assert_eq!(decision.refusal, Some(TradeRefusal::NoLead));
    assert_eq!(scene.memory.to_retrieve, None);
}

#[test]
fn an_empty_wandering_list_has_nothing_to_round_up() {
    // `:635-642`.
    let (_, mut scene) = calm_stable();
    let decision = stablemaster::gather_mounts(&mut scene, HerdState::HerderGatherMounts);
    assert_eq!(decision.next, Some(HerdState::Base(WorkerState::Decide)));
    assert_eq!(decision.refusal, Some(TradeRefusal::NoRoundupTarget));

    scene.memory.wandering = vec![11, 12];
    let started = stablemaster::gather_mounts(&mut scene, HerdState::HerderGatherMounts);
    assert_eq!(started.next, Some(HerdState::HerderGatherMounts));
    assert_eq!(scene.memory.to_retrieve, Some(11), "la première de la liste");
}

#[test]
fn detaching_a_horse_that_is_not_leashed_changes_nothing() {
    // `:710-715` — `clearRestriction()`, puis `return` si la laisse est absente.
    let (_, mut scene) = calm_stable();
    scene.pen.animals.push(PenAnimal::horse(5, Pos::new(1, 64, 0)));
    scene.memory.lead_in_offhand = true;
    stablemaster::detach_horse(&mut scene, 5);
    assert!(scene.memory.act.is_none());
    assert!(scene.memory.lead_in_offhand, "la laisse reste en main");

    scene.pen.animals[0].leashed = true;
    stablemaster::detach_horse(&mut scene, 5);
    assert_eq!(scene.memory.act, Some(HerdAct::Unleashed { id: 5 }));
    assert!(!scene.memory.lead_in_offhand);
    assert_eq!(scene.memory.hand, Hand::Empty);
}

// ---------------------------------------------------------------------------
// Le palefrenier : ce qu'il refuse de faire
// ---------------------------------------------------------------------------

#[test]
fn mounts_are_never_butchered() {
    // `:220-225` — `chanceToButcher` rend zéro, quel que soit le troupeau.
    assert_eq!(stablemaster::chance_to_butcher(), 0.0);
}

#[test]
fn breeding_stops_at_two_mounts_per_building_level() {
    // `:189-209`.
    let base = StableFacts {
        breeding_setting: true,
        building_level: 1,
        mounts: 0,
        ..StableFacts::default()
    };
    assert!(stablemaster::can_breed_children(base));
    assert!(stablemaster::can_breed_children(StableFacts { mounts: 1, ..base }));
    assert!(!stablemaster::can_breed_children(StableFacts { mounts: 2, ..base }));
    // Le réglage ferme l'accouplement même dans une écurie vide (`:192`).
    assert!(!stablemaster::can_breed_children(StableFacts {
        breeding_setting: false,
        ..base
    }));
    // `Math.max(0, 0 * 2)` : une hutte de niveau nul ne peut rien accueillir.
    assert!(!stablemaster::can_breed_children(StableFacts {
        building_level: 0,
        ..base
    }));
}

// ---------------------------------------------------------------------------
// Les suites d'états, tick par tick
// ---------------------------------------------------------------------------

#[test]
fn the_shepherd_sequence_is_decide_shear_decide() {
    // La suite que le Java produit : `DECIDE → SHEPHERD_SHEAR → DECIDE`
    // (`EntityAIWorkShepherd.java:81`, `:165`).
    let (mut ai, mut scene) = shearing_scene();
    scene.decide(HerdChoice::StartWorking);

    let first = ai.tick(&mut scene);
    assert_eq!(first.goal(), HerdState::ShepherdShear);
    assert_eq!(
        first.fired.map(|fired| fired.rule),
        Some("decideWhatToDo (surcharge)")
    );

    let second = ai.tick(&mut scene);
    assert_eq!(second.goal(), HerdState::Base(WorkerState::Decide));
    assert_eq!(second.fired.map(|fired| fired.rule), Some("shearSheep"));
    assert_eq!(
        second.acted(),
        Some(HerdAct::Sheared {
            id: 7,
            wool: 1,
            dyed: None
        })
    );
    assert!(!second.faulted());
}

#[test]
fn the_stablemaster_sequence_walks_then_converts() {
    // `decide` ouvre le dressage (`:241`), `convertMount` choisit (`:334`), marche
    // (`:275`), puis convertit (`:294`).
    let mut ai = HerdAi::new(HerdTrade::Stablemaster);
    let mut scene = ai.scene(WorldFacts::new());
    scene.draws.training = 0.0;
    scene.draws.ready = 0.9;
    scene.memory.train_cooldown = 1;
    scene.pen.animals.push(PenAnimal::horse(4, Pos::new(9, 64, 0)));
    scene.decide(HerdChoice::StartWorking);

    // Tick 1 : la décision ouvre le dressage. Aucun cheval n'est encore choisi — c'est
    // `convertMount` qui choisit, à son tick.
    let decided = ai.tick(&mut scene);
    assert_eq!(decided.goal(), HerdState::HerderTrain);
    assert_eq!(scene.memory.to_convert, None);

    // Tick 2 : `convertMount` retient le candidat et repart en marche.
    let chosen = until_work(&mut ai, &mut scene);
    assert_eq!(chosen.goal(), HerdState::HerderTrain);
    assert_eq!(scene.memory.to_convert, Some(4));
    assert_eq!(chosen.why_not(), None, "l'état ne change pas, sans refus");

    // Tick suivant : le cheval est à neuf blocs, donc la marche continue.
    let walking = until_work(&mut ai, &mut scene);
    assert_eq!(walking.goal(), HerdState::HerderTrain);
    assert_eq!(walking.why_not(), Some(TradeRefusal::NotAtMount));
    assert_eq!(scene.facts.inventory.actions_done, 0);

    // L'appelant applique la marche : le cheval est à portée, la conversion se fait.
    scene.pen.animals[0].at = Pos::new(2, 64, 0);
    let converted = until_work(&mut ai, &mut scene);
    assert_eq!(converted.goal(), HerdState::Base(WorkerState::Decide));
    assert_eq!(converted.acted(), Some(HerdAct::Converted { id: 4 }));
    assert_eq!(scene.memory.to_convert, None);
    assert_eq!(scene.facts.inventory.actions_done, 1);
    assert!(!converted.faulted());
}

#[test]
fn the_stablemaster_sequence_rounds_up_a_wandering_horse() {
    // `decide` remplit `wanderingHorses` (`:253`), `gatherMounts` prend la première
    // (`:640`), l'attache (`:625`), puis la ramène à la stalle (`:591-617`).
    let mut ai = HerdAi::new(HerdTrade::Stablemaster);
    let mut scene = ai.scene(WorldFacts::new());
    scene.job = HerdJob::Stable(StableFacts {
        stall: Some(Pos::new(0, 64, 2)),
        ..StableFacts::default()
    });
    scene.draws.training = 0.9;
    scene.draws.ready = 0.9;
    scene.draws.roundup = 0.0;
    scene.memory.roundup_cooldown = 1;
    scene.pen.animals.push(PenAnimal::horse(5, Pos::new(1, 64, 0)));
    scene.decide(HerdChoice::StartWorking);

    let decided = ai.tick(&mut scene);
    assert_eq!(decided.goal(), HerdState::HerderGatherMounts);
    assert_eq!(scene.memory.wandering, vec![5]);

    // La cible prend la première de la liste, puis l'attache au tick suivant.
    let picked = until_work(&mut ai, &mut scene);
    assert_eq!(picked.goal(), HerdState::HerderGatherMounts);
    assert_eq!(scene.memory.to_retrieve, Some(5));
    let leashed = until_work(&mut ai, &mut scene);
    assert_eq!(leashed.goal(), HerdState::HerderGatherMounts);
    assert_eq!(leashed.acted(), Some(HerdAct::Leashed { id: 5 }));

    // L'appelant applique la laisse : les faits changent, la bête est à la laisse.
    scene.pen.animals[0].leashed = true;
    scene.pen.animals[0].leash_holder = true;
    let home = until_work(&mut ai, &mut scene);
    assert_eq!(home.goal(), HerdState::Base(WorkerState::Decide));
    assert_eq!(home.acted(), Some(HerdAct::RoundupDone));
    assert!(scene.memory.wandering.is_empty());
}

#[test]
fn a_decision_runs_every_sixteen_ai_ticks() {
    // `DECIDING_DELAY = 80` ticks de jeu, et la machine avance de `ENTITY_AI_TICKRATE`
    // (`AbstractEntityCitizen.java:64`) par tick : seize ticks d'IA par décision.
    let (mut ai, mut scene) = calm_stable();
    scene.decide(HerdChoice::StartWorking);
    let first = ai.tick(&mut scene);
    assert_eq!(first.goal(), HerdState::Base(WorkerState::StartWorking));

    // La table de base décide à nouveau : la reprise ramène la machine en `DECIDE`.
    scene.decide(HerdChoice::StartWorking);
    let resumed = ai.tick(&mut scene);
    assert_eq!(resumed.goal(), HerdState::Base(WorkerState::Decide));
    assert_eq!(resumed.fired.map(|fired| fired.rule), Some("handStartWorking"));

    // Puis le compte à rebours de 80 court : le tick suivant ne décide pas.
    let waiting = ai.tick(&mut scene);
    assert!(waiting.fired.is_none());
    assert_eq!(waiting.goal(), HerdState::Base(WorkerState::Decide));
}

#[test]
fn a_handed_back_state_waits_for_the_base_table() {
    // `AbstractEntityAIHerder.java:248` — la décision de base rend `START_WORKING` et
    // c'est la table du parent qui le fait tourner ; ce module n'a rien à y dire.
    let (mut ai, mut scene) = calm_stable();
    scene.decide(HerdChoice::StartWorking);
    let handed = ai.tick(&mut scene);
    assert_eq!(handed.goal(), HerdState::Base(WorkerState::StartWorking));
    assert!(handed.handed_back());
    // Sans nouvelle réponse de la table de base, rien ne bouge.
    let idle = ai.tick(&mut scene);
    assert!(idle.fired.is_none());
    assert!(idle.handed_back());
}

#[test]
fn the_four_base_herder_answers_are_traversed_not_decided() {
    // Les quatre états d'`AbstractEntityAIHerder` (`:232`, `:236`, `:240`, `:244`) sont
    // traversés : la machine peut s'y trouver, ce module n'y décide rien.
    for choice in [
        HerdChoice::Pickup,
        HerdChoice::Butcher,
        HerdChoice::Breed,
        HerdChoice::Feed,
    ] {
        let (_, mut scene) = empty(HerdTrade::Shepherd);
        scene.decide(choice);
        let decision = shepherd::decide_what_to_do(&mut scene, HerdState::Base(WorkerState::Decide));
        assert_eq!(decision.next, Some(choice.state()));
        assert!(choice.state().is_traversed());
    }
}

#[test]
fn every_trade_starts_on_decide_and_can_leave_its_own_states() {
    for trade in HerdTrade::ALL {
        let ai = HerdAi::new(trade);
        assert_eq!(ai.state(), HerdState::Base(WorkerState::Decide));
        assert!(ai.machine().can_leave(HerdState::Base(WorkerState::Decide)));
        assert_eq!(ai.trade(), trade);
        assert_eq!(ai.scene(WorldFacts::new()).job.trade(), trade);
        // `register` ne laisse en piège ni les états traversés, ni les cibles du métier.
        for state in HerdState::ALL {
            if state.belongs_to_other_trade(trade) {
                assert!(!ai.machine().can_leave(state), "{trade:?} ne devrait pas porter {state}");
                continue;
            }
            assert!(ai.machine().can_leave(state), "{trade:?} ne peut pas sortir de {state}");
        }
        assert!(ai.machine().can_leave(HerdState::Base(WorkerState::StartWorking)));
        assert!(ai.machine().can_leave(HerdState::Base(WorkerState::Preparing)));
        assert!(ai.machine().can_leave(HerdState::Base(WorkerState::Idle)));
    }
}

#[test]
fn a_reset_sends_the_machine_back_to_decide() {
    // `AbstractAISkeleton.resetAI` (`:145-152`) → `BasicStateMachine.reset` (`:373-380`).
    let (mut ai, mut scene) = shearing_scene();
    scene.decide(HerdChoice::StartWorking);
    ai.tick(&mut scene);
    assert_ne!(ai.state(), HerdState::Base(WorkerState::Decide));
    ai.reset();
    assert_eq!(ai.state(), HerdState::Base(WorkerState::Decide));
}

#[test]
fn every_act_names_its_java_line_and_its_animal() {
    let acts = [
        HerdAct::Sheared {
            id: 1,
            wool: 2,
            dyed: Some(3),
        },
        HerdAct::Converted { id: 2 },
        HerdAct::HealthTrained { id: 3 },
        HerdAct::Fed {
            id: 4,
            healed: true,
        },
        HerdAct::Readied {
            id: 5,
            ready: false,
        },
        HerdAct::Leashed { id: 6 },
        HerdAct::Unleashed { id: 7 },
        HerdAct::RequestedItems {
            task: MountMaintenance::Readying,
        },
        HerdAct::RoundupDone,
        HerdAct::Butchered { id: 8 },
    ];
    for act in acts {
        assert!(act.java().starts_with("EntityAIWork"), "{}", act.java());
    }
    assert_eq!(acts[0].animal(), Some(1));
    assert_eq!(acts[7].animal(), None);
    assert_eq!(acts[8].animal(), None);
    assert_eq!(MountMaintenance::Feeding.java_name(), "FEEDING");
    assert_eq!(MountMaintenance::Readying.item_name(), "ModTags.leather");
}

#[test]
fn a_decision_that_goes_nowhere_is_distinguishable_from_one_that_hands_back() {
    // `Decision::stay` est `return getState()`, `Decision::go` est un vrai changement :
    // les deux ont un `next` différent, et c'est ce que lit `Action::Run`.
    assert_eq!(Decision::stay().next, None);
    assert_eq!(
        Decision::go(HerdState::HerderTrain).next,
        Some(HerdState::HerderTrain)
    );
    let refused = Decision::refuse(TradeRefusal::NoStall);
    assert_eq!(refused.next, None);
    assert_eq!(refused.refusal, Some(TradeRefusal::NoStall));
    let after = Decision::go_after(HerdState::Base(WorkerState::Preparing), TradeRefusal::NoLead);
    assert_eq!(after.next, Some(HerdState::Base(WorkerState::Preparing)));
    assert_eq!(after.refusal, Some(TradeRefusal::NoLead));
}
