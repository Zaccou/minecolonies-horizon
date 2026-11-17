//! Les scénarios : un paquet de faits, et la suite d'états qu'il produit doit être
//! celle du Java.
//!
//! Les règles unitaires sont testées à côté de la règle (`pupil.rs`, `state.rs`,
//! `facts.rs`, `crafting_shop.rs`) ; ce fichier ne rejoue que ce qui ne se voit qu'en
//! enchaînant les ticks — l'ordre des états, les rythmes, et les deux métiers de la
//! pierre qui n'ont pas de but à eux.
//!
//! Java reference : `EntityAIWorkPupil.java:72-79` (la table et ses rythmes), `:87-104`
//! (`decide`), `:111-120` (`recess`), `:127-203` (`study`), `:216-223`
//! (`startWorkingAtOwnBuilding`).

use crate::ai::facts::Pos;
use crate::ai::state::WorkerState;
use crate::ai_build::crafter;

use super::crafting_shop::{self, CRAFTING_TARGETS};
use super::facts::Seat;
use super::pupil::{Particle, PupilAi, PupilAsk, PupilOutcome};
use super::{PupilFacts, PupilRefusal, PupilScene, PupilState, Trade};

/// Un écolier qui a un tapis, du papier et une école vide : le cas nominal.
fn nominal() -> PupilScene {
    PupilScene::with_carpets(&[Pos::new(2, 64, 2)])
}

/// Les états par lesquels la machine passe, dans l'ordre, sur `ticks` ticks.
///
/// Seuls les **changements** sont notés : c'est exactement ce que Java fait quand
/// `transitionToNext` renvoie un état différent, et cela rend la suite lisible.
fn trail(ai: &mut PupilAi, scene: &mut PupilScene, ticks: u32) -> Vec<&'static str> {
    let mut seen = Vec::new();
    for _ in 0..ticks {
        let outcome = ai.tick(scene);
        if outcome.moved() {
            seen.push(outcome.goal().java());
        }
    }
    seen
}

/// Toutes les demandes adressées au monde sur `ticks` ticks.
fn asks(ai: &mut PupilAi, scene: &mut PupilScene, ticks: u32) -> Vec<PupilAsk> {
    let mut all = Vec::new();
    for _ in 0..ticks {
        ai.tick(scene);
        all.extend(scene.asks.iter().copied());
    }
    all
}

/// Le premier tick où la règle `rule` tire, dans la limite de `ticks` ticks.
///
/// **Le compteur d'une cible appartient à la transition, pas à l'état.** Java :
/// `TickRateStateMachine.checkTransition` (`:114-126`) décrémente le compteur par le
/// tick de la machine (`:118`) et le remet à `getTickRate()` quand il tombe à zéro
/// (`:123`) ; `TickingTransition.ticksToUpdate` (`:118-128`) est un champ de la
/// transition, qui vit aussi longtemps que l'IA. Conséquence, et c'est le piège que
/// trois tests de ce fichier ont rencontré : **sortir puis revenir dans un état ne
/// remet pas le compteur à zéro**. Une cible à `TICKS_SECOND = 20` ne tire donc pas
/// au tick suivant l'entrée dans son état, mais après ses 4 ticks de machine
/// (`ENTITY_AI_TICKRATE = 5`), et seulement s'ils ont lieu dans cet état : le
/// compteur ne se décrémente que lorsque la liste de l'état courant est parcourue
/// (`:101`).
///
/// Ce port est fidèle sur ce point (`crate::ai::state`, `Transition::count_down`), et
/// c'est pour ça que ces tests attendent le tir au lieu de supposer qu'il est immédiat.
fn until_rule(
    ai: &mut PupilAi,
    scene: &mut PupilScene,
    ticks: u32,
    rule: &str,
) -> Option<PupilOutcome> {
    for _ in 0..ticks {
        let outcome = ai.tick(scene);
        if outcome.why() == Some(rule) {
            return Some(outcome);
        }
    }
    None
}

#[test]
fn a_pupil_walks_idle_start_study_decide_study() {
    // La suite exacte, tick par tick, d'un écolier qui a une place assise :
    // `IDLE → START_WORKING` (`:73`), `→ STUDY` (`:222`), `→ DECIDE` (`:130`),
    // `→ STUDY` (`:103`). Quatre ticks, parce que chaque état nouveau est regardé
    // dès le tick suivant et que son compteur part de zéro.
    let mut ai = PupilAi::new();
    let mut scene = nominal();
    assert_eq!(ai.state(), PupilState::Base(WorkerState::Idle));
    assert_eq!(
        trail(&mut ai, &mut scene, 4),
        vec!["START_WORKING", "STUDY", "DECIDE", "STUDY"]
    );
}

#[test]
fn the_first_study_of_the_first_lesson_sits_the_citizen() {
    // `:146-151` : au premier passage utile, `sittingTicks == 0`, donc on s'assoit
    // pour `sit_ticks(0)` = 1 200 ticks (`:150`).
    //
    // « Premier passage utile » n'est pas « tick suivant » : la cible `study` a déjà
    // tiré une fois au tick 3 — c'était le `studyPos == null` qui a renvoyé en
    // `DECIDE` (`:129-132`) — donc son compteur vaut 20 et il faut ses 4 ticks de
    // machine pour revenir (`TickRateStateMachine.java:118-123`). L'attente initiale
    // « ça tire tout de suite » était fausse ; le code, lui, suit Java.
    let mut ai = PupilAi::new();
    let mut scene = nominal();
    trail(&mut ai, &mut scene, 4); // jusqu'à STUDY avec un siège en mémoire
    let outcome = until_rule(&mut ai, &mut scene, 8, "study").expect("la cible study tire");
    assert_eq!(outcome.goal(), PupilState::Study);
    assert_eq!(scene.memory.study_pos, Some(Pos::new(2, 64, 2)));
    assert!(scene.asks.contains(&PupilAsk::Sit {
        pos: Pos::new(2, 64, 2),
        ticks: 1_200
    }));
    assert!(scene.asks.contains(&PupilAsk::HoldItem("minecraft:paper")));
    assert_eq!(scene.memory.sitting_ticks, 1, "`:166`");
    assert!(scene.memory.riding);
}

#[test]
fn a_lesson_repeats_without_ever_leaving_study() {
    // `:202` rend `null` : la machine ne bouge pas, et le tick suivant rassoit. Un
    // écolier ne quitte donc `STUDY` que par un siège occupé ou une récréation.
    let mut ai = PupilAi::new();
    let mut scene = nominal();
    let seen = trail(&mut ai, &mut scene, 1_200);
    assert_eq!(seen, vec!["START_WORKING", "STUDY", "DECIDE", "STUDY"]);
    assert_eq!(
        ai.state(),
        PupilState::Study,
        "1 200 ticks plus tard, il étudie encore"
    );
    // Et il a bien fini au moins une leçon : une demande `PayLesson` a été faite.
    let paid = asks(&mut ai, &mut scene, 400);
    assert!(
        paid.iter().any(|ask| matches!(ask, PupilAsk::PayLesson { .. })),
        "les leçons s'enchaînent : {} demandes",
        paid.len()
    );
}

#[test]
fn the_lesson_lasts_as_many_study_calls_as_the_draw_says() {
    // `:149` : `maxSittingTicks = nextInt(60) + 60`, et `sittingTicks` compte les
    // passages de `study` (`:166`), un par seconde de jeu. Avec le tirage maximal, une
    // leçon dure 119 passages de `STUDY` — pas 60 : `sittingTicks++` passe avant la
    // comparaison, donc la leçon finit au 119e appel, et la suivante commence au 120e.
    let mut scene = nominal();
    scene.facts.sit_draw = 59;
    let mut ai = PupilAi::new();
    let mut study_calls = 0;
    let mut sit_calls: Vec<u32> = Vec::new();
    let mut paid = 0;
    for _ in 0..900 {
        let outcome = ai.tick(&mut scene);
        if outcome.why() == Some("study") {
            study_calls += 1;
        }
        for ask in &scene.asks {
            match ask {
                PupilAsk::Sit { ticks, .. } => {
                    assert_eq!(*ticks, 2_380, "`:150`, 119 secondes de 20 ticks");
                    sit_calls.push(study_calls);
                }
                PupilAsk::PayLesson { .. } => paid += 1,
                _ => {}
            }
        }
    }
    assert!(
        sit_calls.len() >= 2,
        "900 ticks suffisent pour deux leçons : {sit_calls:?}"
    );
    assert_eq!(
        sit_calls[1] - sit_calls[0],
        119,
        "une leçon = 119 passages de STUDY"
    );
    assert!(paid >= 1, "au moins une leçon payée");
}

#[test]
fn one_draw_in_ten_sends_the_pupil_to_recess() {
    // `:89` : `nextInt(10) < 1` dans `decide`. Le tirage est un fait, donc le test le
    // pose au lieu d'espérer.
    let mut scene = nominal();
    scene.facts.recess_roll = 0;
    let mut ai = PupilAi::new();
    assert_eq!(
        trail(&mut ai, &mut scene, 4),
        vec!["START_WORKING", "STUDY", "DECIDE", "RECESS"]
    );
    // `:113` : la récréation ne finit que si la marche est finie **et** le tirage
    // tombe. Ici il ne tombe pas : il continue de courir.
    scene.facts.recess_exit_roll = 5;
    let wandering = trail(&mut ai, &mut scene, 40);
    assert!(wandering.is_empty(), "il reste en RECESS : {wandering:?}");
    assert!(
        asks(&mut ai, &mut scene, 8)
            .iter()
            .any(|ask| matches!(ask, PupilAsk::Wander { radius: 10, .. })),
        "`:118`, une promenade de rayon 10"
    );
    // Le tirage tombe et la marche est finie : retour à la table. Encore une fois, la
    // fin de récréation rend `START_WORKING` (`:116`) mais ne fait pas tirer
    // `startWorkingAtOwnBuilding` avant ses 4 ticks de machine : son compteur a été
    // armé au tick 2, il n'a jamais été décrémenté depuis (on n'était pas dans son
    // état), et il ne repart que là (`TickRateStateMachine.java:101`, `:118-123`).
    scene.facts.recess_exit_roll = 0;
    let left = until_rule(&mut ai, &mut scene, 8, "recess").expect("la cible recess tire");
    assert_eq!(left.goal(), PupilState::Base(WorkerState::StartWorking), "`:116`");
    let table = until_rule(&mut ai, &mut scene, 8, "startWorkingAtOwnBuilding")
        .expect("la cible de l'école tire");
    assert_eq!(table.goal(), PupilState::Study, "`:222`");
}

#[test]
fn a_recess_that_is_still_walking_never_ends() {
    // `:113` exige `worker.getNavigation().isDone()` : sans ça, un écolier dont la
    // marche n'aboutit pas resterait en récréation pour toujours.
    let mut scene = nominal();
    scene.facts.recess_roll = 0;
    scene.facts.navigation_done = false;
    scene.facts.recess_exit_roll = 0;
    let mut ai = PupilAi::new();
    trail(&mut ai, &mut scene, 4); // → RECESS
    let after = trail(&mut ai, &mut scene, 200);
    assert!(after.is_empty(), "{after:?}");
    assert_eq!(ai.state(), PupilState::Recess);
}

#[test]
fn an_empty_school_keeps_the_pupil_in_decide_and_asks_for_a_carpet_once_a_second() {
    // `:96-100` : `DECIDE` est rendu à lui-même, et l'interaction bloquante part à
    // chaque passage — un par seconde de jeu, rythme de la cible (`:75`), donc un
    // tick de machine sur quatre. Le refus est relevé sur les ticks qui tirent : le
    // port l'efface au début de chaque tick (`PupilAi::tick`), il n'est pas un état.
    let mut ai = PupilAi::new();
    let mut scene = PupilScene::new(PupilFacts::default());
    let seen = trail(&mut ai, &mut scene, 20);
    assert_eq!(seen, vec!["START_WORKING", "STUDY", "DECIDE"]);
    assert_eq!(ai.state(), PupilState::Base(WorkerState::Decide));

    let mut calls = 0;
    for _ in 0..20 {
        let outcome = ai.tick(&mut scene);
        if outcome.why() == Some("decide") {
            assert_eq!(outcome.why_not(), Some(PupilRefusal::NoSeat));
            assert_eq!(outcome.goal(), PupilState::Base(WorkerState::Decide));
            calls += 1;
        }
    }
    assert_eq!(calls, 5, "20 ticks / 4 = 5 passages de DECIDE");

    // Compte les plaintes sur 100 ticks. La cible `DECIDE` a un rythme de 20 ticks
    // (`:75`) et la machine avance de 5 ticks par tick (`ENTITY_AI_TICKRATE`), donc
    // elle est regardée toutes les 4 unités : 25 passages en 100 ticks, une plainte
    // chacun.
    let complaints = asks(&mut ai, &mut scene, 100)
        .iter()
        .filter(|ask| matches!(**ask, PupilAsk::Complaint(_)))
        .count();
    assert_eq!(complaints, 25, "un message par passage de DECIDE");
}

#[test]
fn a_stale_carpet_is_reported_as_such_and_the_pupil_waits() {
    // `BuildingSchool.java:138-143` : le tapis tiré n'est plus un tapis, la hutte doit
    // l'oublier et l'écolier reste en `DECIDE`.
    let mut ai = PupilAi::new();
    let mut scene = PupilScene::new(PupilFacts {
        carpets: vec![Seat::stale(Pos::new(3, 64, 3))],
        recess_roll: 1,
        ..PupilFacts::default()
    });
    trail(&mut ai, &mut scene, 3); // → DECIDE
    let outcome = ai.tick(&mut scene);
    assert_eq!(outcome.goal(), PupilState::Base(WorkerState::Decide));
    assert_eq!(outcome.why_not(), Some(PupilRefusal::StaleCarpet));
    assert_eq!(scene.asks, vec![
        PupilAsk::ForgetCarpet(Pos::new(3, 64, 3)),
        PupilAsk::Complaint(super::PUPIL_NO_CARPET),
    ]);
}

#[test]
fn another_citizen_on_the_seat_sends_the_pupil_back_to_decide() {
    // `:139-144` : la seule sortie de `STUDY` en plein travail.
    let mut ai = PupilAi::new();
    let mut scene = nominal();
    trail(&mut ai, &mut scene, 6); // jusqu'à la première seconde de leçon
    assert_eq!(ai.state(), PupilState::Study);
    scene.facts.other_citizen_at_seat = true;

    let mut taken = false;
    for _ in 0..8 {
        let outcome = ai.tick(&mut scene);
        if outcome.moved() && outcome.goal() == PupilState::Base(WorkerState::Decide) {
            assert_eq!(outcome.why_not(), Some(PupilRefusal::SeatTaken));
            assert_eq!(scene.memory.study_pos, None, "`:142`, le siège est oublié");
            taken = true;
            break;
        }
    }
    assert!(taken, "il retourne choisir un siège");

    // Et il repart vers le seul tapis de l'école dès que la place se libère.
    scene.facts.other_citizen_at_seat = false;
    assert_eq!(trail(&mut ai, &mut scene, 8), vec!["STUDY"]);
}

#[test]
fn a_pupil_without_paper_studies_and_pays_nothing() {
    // `:153-164` puis `:179-196` : la leçon a lieu, sans feuille et sans XP.
    let mut ai = PupilAi::new();
    let mut scene = nominal();
    scene.facts.has_paper = false;
    scene.facts.sit_draw = 0;
    let mut collected = Vec::new();
    let mut saw_no_paper = false;
    for _ in 0..400 {
        let outcome = ai.tick(&mut scene);
        collected.extend(scene.asks.iter().copied());
        if outcome.why_not() == Some(PupilRefusal::NoPaper) {
            saw_no_paper = true;
        }
    }
    assert!(
        collected
            .iter()
            .any(|ask| matches!(ask, PupilAsk::Particle { kind: Particle::HappyVillager, .. })),
        "particule joyeuse (`:163`)"
    );
    assert!(
        !collected.iter().any(|ask| matches!(ask, PupilAsk::ConsumePaper)),
        "aucune feuille consommée"
    );
    assert!(
        !collected
            .iter()
            .any(|ask| matches!(ask, PupilAsk::PayLesson { .. })),
        "aucune XP"
    );
    assert!(saw_no_paper, "la leçon vide est signalée (`:160-164`)");
}

#[test]
fn a_pupil_that_is_not_in_its_school_keeps_walking_first() {
    // `:218-221` : `START_WORKING` ne rend `STUDY` qu'une fois arrivé.
    let mut ai = PupilAi::new();
    let mut scene = PupilScene::new(PupilFacts {
        at_building: false,
        ..PupilFacts::default()
    });
    let walking = trail(&mut ai, &mut scene, 40);
    assert_eq!(walking, vec!["START_WORKING"]);
    assert_eq!(ai.state(), PupilState::Base(WorkerState::StartWorking));

    // Le refus n'est pas un état : Java rend `getState()` **à chaque appel** de
    // `startWorkingAtOwnBuilding`, et cette cible a le rythme `TICKS_SECOND` (`:74`),
    // donc un appel tous les 4 ticks de machine — pas à chaque tick. Le port efface
    // le refus au début de chaque tick (`PupilAi::tick`) : l'attendre après coup,
    // 40 ticks plus tard, était l'erreur. On le relève donc sur les ticks qui tirent.
    let mut calls = 0;
    for _ in 0..40 {
        let outcome = ai.tick(&mut scene);
        if outcome.why() == Some("startWorkingAtOwnBuilding") {
            assert_eq!(outcome.why_not(), Some(PupilRefusal::NotAtBuilding));
            assert_eq!(outcome.goal(), PupilState::Base(WorkerState::StartWorking));
            calls += 1;
        }
    }
    assert_eq!(calls, 10, "rythme 20 / machine 5 = un appel tous les 4 ticks");
    // Et la marche est demandée à chacun de ces appels.
    assert!(asks(&mut ai, &mut scene, 4)
        .iter()
        .all(|ask| *ask == PupilAsk::WalkToBuilding));

    // Il arrive : `STUDY` (`:222`), puis `DECIDE` faute de siège (`:130`).
    scene.facts.at_building = true;
    assert_eq!(trail(&mut ai, &mut scene, 8), vec!["STUDY", "DECIDE"]);
}

#[test]
fn a_pupil_in_a_walk_towards_its_seat_stays_in_study() {
    // `:134-137` : la marche est demandée à chaque passage, l'état ne change pas.
    let mut ai = PupilAi::new();
    let mut scene = nominal();
    trail(&mut ai, &mut scene, 4); // jusqu'à STUDY avec un siège
    scene.facts.at_seat = false;
    let seen = trail(&mut ai, &mut scene, 40);
    assert!(seen.is_empty(), "{seen:?}");
    assert_eq!(ai.state(), PupilState::Study);
    let mut calls = 0;
    for _ in 0..40 {
        let outcome = ai.tick(&mut scene);
        if outcome.why() == Some("study") {
            assert_eq!(outcome.why_not(), Some(PupilRefusal::NotAtSeat));
            calls += 1;
        }
    }
    assert_eq!(calls, 10, "un appel de `study` tous les 4 ticks de machine");
    assert!(asks(&mut ai, &mut scene, 8)
        .contains(&PupilAsk::WalkToSeat(Pos::new(2, 64, 2))));
}

#[test]
fn every_state_of_the_pupil_machine_has_a_transition() {
    // Le piège de `BasicStateMachine.java:220-228` est un bug du port, pas un
    // événement de jeu : la table doit couvrir tous les états qu'elle annonce.
    let ai = PupilAi::new();
    for state in PupilState::ALL {
        assert!(
            ai.machine().can_leave(state),
            "{} n'a pas de sortie",
            state.java()
        );
    }
    // Et aucun état n'est annoncé sans être atteignable par une règle.
    assert_eq!(PupilState::ALL.len(), 8);
}

#[test]
fn the_three_trades_and_what_each_one_really_carries() {
    // La mesure du brief : deux alias et un vrai métier.
    assert_eq!(Trade::ALL.len(), 3);
    for (trade, class, job_id, hut) in [
        (
            Trade::StoneSmeltery,
            "EntityAIWorkStoneSmeltery",
            "stonesmeltery",
            "stonesmeltery",
        ),
        (
            Trade::Stonemason,
            "EntityAIWorkStonemason",
            "stonemason",
            "stonemason",
        ),
        (Trade::Pupil, "EntityAIWorkPupil", "pupil", "school"),
    ] {
        assert_eq!(trade.class(), class);
        assert_eq!(trade.building().name(), hut);
        // **Deux `path()` et deux contrats, et c'est ce que ce test a confondu.**
        // `Job::path` est le chemin de registre **sans** l'espace de noms
        // (`jobs/job.rs:52-56` : « `builder`, not `minecolonies:builder` »,
        // `ModJobs.java:14`) ; `Kind::path` est le chemin complet
        // (`building/kind.rs:126`). Un métier et sa hutte ne se comparent donc pas
        // de la même façon.
        assert_eq!(trade.job().path(), job_id);
        assert_eq!(trade.building().path(), format!("minecolonies:{hut}"));
    }
    // Pour l'écolier, métier et hutte n'ont même pas le même identifiant : le métier
    // est `pupil` (`ModJobs.java:47`) et la hutte `school`
    // (`BuildingSchool.java:32`) ; c'est la hutte qui embauche le métier
    // (`jobs/employment.rs:352`), et c'est pourquoi `Trade` répond deux choses
    // séparées.
    assert_ne!(Trade::Pupil.job().path(), Trade::Pupil.building().name());
    // Et le nom d'affichage n'est pas l'identifiant : la pierre taillée s'appelle
    // « Brickmaker » dans la langue du jeu (`jobs/job.rs:127`).
    assert_eq!(Trade::StoneSmeltery.job().display(), "Brickmaker");
    assert_ne!(Trade::StoneSmeltery.job().display(), "stonesmeltery");
    assert_eq!(Trade::Stonemason.targets(), 20);
    assert_eq!(Trade::StoneSmeltery.targets(), 27);
    assert_eq!(Trade::Pupil.targets(), 19);
    // Seul l'écolier a une table à lui.
    assert_eq!(Trade::Pupil.own_table(), Some("EntityAIWorkPupil.java:72-78"));
    assert_eq!(Trade::Stonemason.own_table(), None);
    assert_eq!(Trade::StoneSmeltery.own_table(), None);
}

#[test]
fn the_mason_table_is_the_crafter_table_and_not_a_second_copy() {
    // `crate::ai_build::crafter` porte `AbstractEntityAICrafting.java:160-170`. Si
    // ce test échoue, c'est que le crafter a changé : la liste ci-dessous doit être
    // mise à jour **là-bas**, pas ici.
    let reference = crafter::ai();
    let mason = crafting_shop::machine(Trade::Stonemason).expect("la table du tailleur");
    let reference_rules: Vec<&str> = reference
        .transitions()
        .iter()
        .map(|transition| transition.rule)
        .collect();
    let mason_rules: Vec<&str> = mason
        .transitions()
        .iter()
        .map(|transition| transition.rule)
        .collect();
    assert_eq!(mason_rules, reference_rules);
    assert_eq!(&mason_rules[..CRAFTING_TARGETS.len()], &CRAFTING_TARGETS[..]);
}

#[test]
fn the_smeltery_delta_is_an_errand_order_not_a_fourth_furnace_table() {
    // Ce que le module porte de la pierre taillée : six courses ordonnées et trois
    // gardes. Les sept cibles de `AbstractEntityAIRequestSmelter.java:71-82` sont
    // portées par les modules voisins, pas ici.
    assert_eq!(crafting_shop::ERRAND_ORDER.len(), 6);
    assert_eq!(crafting_shop::PRE_GUARDS.len(), 3);
    assert_eq!(Trade::StoneSmeltery.registers().len(), 4);
    assert_eq!(
        Trade::StoneSmeltery.registers()[3],
        "AbstractEntityAIRequestSmelter.java:71-82"
    );
    // Chaque course porte son état d'arrivée ou dit qu'elle n'en change pas.
    let arrivals: Vec<Option<&str>> = crafting_shop::ERRAND_ORDER
        .iter()
        .map(|guard| guard.target)
        .collect();
    assert_eq!(
        arrivals,
        vec![
            Some("RETRIEVING_END_PRODUCT_FROM_FURNACE"),
            Some("RETRIEVING_UNRELATED_PRODUCT_FROM_FURNACE"),
            Some("ADD_FUEL_TO_FURNACE"),
            None,
            Some("FILL_UP_FURNACES"),
            Some("START_WORKING"),
        ]
    );
}

#[test]
fn a_pupil_facts_change_of_trade_does_not_leak_into_the_machine() {
    // Le monde de l'écolier est un `WorldFacts` ordinaire et son métier y est
    // « non porté » : la table de base n'a donc aucune cible de métier à proposer, et
    // c'est cette machine-ci qui décide. Ce test tient la couture.
    let mut scene = nominal();
    assert_eq!(
        scene.world.job,
        crate::ai::facts::JobFacts::Unported,
        "l'écolier n'est pas dans `crate::ai::facts`"
    );
    let mut ai = PupilAi::new();
    scene.world.worker.paused = true; // ne change rien : la pause est la table de base
    let seen = trail(&mut ai, &mut scene, 4);
    assert_eq!(seen, vec!["START_WORKING", "STUDY", "DECIDE", "STUDY"]);
}
