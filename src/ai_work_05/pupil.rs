//! Le but de l'écolier : deux états à lui, cinq décisions, et ce qu'il demande au
//! monde pour les mener.
//!
//! Référence Java : `core/entity/ai/workers/education/EntityAIWorkPupil.java`, tout
//! le fichier — le constructeur (`:72-79`), `decide` (`:87-104`), `recess`
//! (`:111-120`), `study` (`:127-203`) et `startWorkingAtOwnBuilding` (`:216-223`),
//! plus la seule méthode de la hutte qu'il appelle, `BuildingSchool.getRandomPlaceToSit`
//! (`:126-152`).
//!
//! ## La table que ce fichier porte
//!
//! | # | État | Règle | Rythme | Java |
//! | ---: | --- | --- | ---: | --- |
//! | 1 | `IDLE` | `→ START_WORKING`, sans condition | 1 | `:73` |
//! | 2 | `START_WORKING` | `startWorkingAtOwnBuilding` | 20 | `:74` |
//! | 3 | `DECIDE` | `decide` | 20 | `:75` |
//! | 4 | `STUDY` | `study` | 20 | `:76` |
//! | 5 | `RECESS` | `recess` | 20 | `:77` |
//! | — | `INVENTORY_FULL` | `afterDump` → `IDLE` | 5 | `AbstractEntityAIBasic.java:1213-1222` |
//! | — | `NEEDS_ITEM` | `waitForRequests` → `IDLE` | 5 | `AbstractEntityAIBasic.java:238` |
//! | — | `PAUSED` | `bePaused` → `IDLE` | 20 | `AbstractEntityAIBasic.java:251` |
//!
//! **Le tour complet d'un écolier.** `IDLE` sort au tick suivant (`:73`, rythme 1 :
//! il n'y a rien à attendre), `START_WORKING` marche jusqu'à l'école puis va
//! **directement** en `STUDY` (`:222`) — pas en `DECIDE`. Le premier `study` trouve
//! `studyPos == null` et renvoie `DECIDE` (`:129-132`), qui choisit : une fois sur dix
//! une récréation (`:89`), sinon un siège (`:95-103`). Une fois assis, il enchaîne les
//! leçons **sans repasser par `DECIDE`** : `study` finit sur `return null` (`:202`),
//! donc l'état ne change pas et le tick suivant rassoit le citoyen (`:146-151`). Seul
//! un autre citoyen sur le siège (`:139-144`) ou une récréation le ramène en `DECIDE`.
//!
//! **Les deux unités de temps.** `maxSittingTicks` (`:149`) est tiré **en secondes**
//! (`nextInt(60) + 60`) et `sittingTicks` (`:166`) compte les passages de `study`,
//! dont chacun dure `TICKS_SECOND = 20` ticks (`:76`) — donc une seconde de jeu. La
//! comparaison de `:167` est juste parce que les deux unités sont des secondes ; le
//! nom du champ dit « ticks » et c'est un piège que ce module nomme.
//!
//! **Aucun appel au monde.** S'asseoir, changer l'objet en main, envoyer la particule,
//! consommer la feuille, monter l'Intelligence : chacun est une [`PupilAsk`] rendue
//! par la décision. Le test qui compte est un scénario de faits qui produit la même
//! suite d'états que le Java ; `tests.rs` en déroule trois.
//!
//! **Ce qui n'est pas dans ce fichier.** Les quatorze cibles de base
//! (`AbstractEntityAIBasic.java:204-259`) sont `crate::ai::worker::register_base`,
//! sur `WorkerState` ; le partage est celui de `crate::ai_gather` — une machine ne
//! peut pas *contenir* l'autre, donc ce module déclare les trois états de base où
//! Java renvoie l'écolier (`INVENTORY_FULL`, `NEEDS_ITEM`, `PAUSED`) et où le
//! contrôle revient (`IDLE`).

use crate::ai::facts::Pos;
use crate::ai::state::{Action, Fired, Machine, Transition, WorkerState};
use crate::ai::ENTITY_AI_TICKRATE;

use super::facts::{PupilFacts, PupilMemory, PupilScene};
use super::state::{Decision, PupilRefusal, PupilState};
use super::{PUPIL_NO_CARPET, TICKS_SECOND};

/// `EntityAIWorkPupil.STUDY_TO_RECESS_RATIO` (`:42`) : une récréation sur dix, lu
/// deux fois — à l'entrée (`:89`) et à la sortie (`:113`).
pub const STUDY_TO_RECESS_RATIO: u32 = 10;

/// Le plancher de la durée d'une leçon, en secondes : le `+ 60` de
/// `nextInt(120 / 2) + 60` (`:149`).
pub const SIT_SECONDS_FLOOR: u32 = 60;

/// L'amplitude du tirage de la durée, en secondes : le `120 / 2` de la même ligne
/// (`:149`). Le tirage rend donc `0..60`, et la durée `60..119` secondes.
pub const SIT_SECONDS_SPREAD: u32 = 120 / 2;

/// Le rayon de la promenade de récréation : `walkToRandomPosWithin(worker, 10, …)`
/// (`:118`).
pub const WANDER_RADIUS: i32 = 10;

/// Ce que l'écolier consomme pour étudier : `stack.getItem() == Items.PAPER`
/// (`:47`).
pub const PAPER: &str = "minecraft:paper";

/// Les points d'Intelligence d'une leçon **avec** papier, avant la recherche
/// `TEACHING` : le `50.0` de `:186`.
pub const LESSON_INTELLIGENCE_XP: f64 = 50.0;

/// Les particules du métier : `ParticleTypes.ENCHANT` quand l'écolier a de quoi
/// écrire (`:158`), `HAPPY_VILLAGER` quand il n'a rien (`:163`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive : les deux particules que le fichier nomme
pub enum Particle {
    /// `ParticleTypes.ENCHANT`, un anneau de `sittingTicks` particules (`:158`).
    Enchant,
    /// `ParticleTypes.HAPPY_VILLAGER`, le même anneau (`:163`).
    HappyVillager,
}

/// Ce que le but demande au monde pour appliquer sa décision.
///
/// Java fait ces appels en ligne dans `decide`, `recess` et `study` ; ce port les
/// rend, parce qu'un but qui agit ne se teste pas sans monde. L'ordre du vecteur est
/// l'ordre des appels Java, et il compte : la main est vidée (`:172`) **avant** que la
/// feuille ne soit consommée (`:183`).
#[derive(Clone, Copy, Debug, PartialEq)]
// exhaustive : les treize effets que `EntityAIWorkPupil` produit
pub enum PupilAsk {
    /// `EntityNavigationUtils.walkToBuilding(worker, building)` (`:218`).
    WalkToBuilding,
    /// `walkToWorkPos(studyPos)` (`:134`).
    WalkToSeat(Pos),
    /// `walkToRandomPosWithin(worker, 10, DEFAULT_SPEED, building.getCorners())`
    /// (`:118`).
    Wander {
        /// Le rayon, [`WANDER_RADIUS`].
        radius: i32,
        /// Les coins de l'école (`building.getCorners()`), quand ils sont connus.
        corners: Option<(Pos, Pos)>,
    },
    /// `SittingEntity.sitDown(studyPos, worker, maxSittingTicks * 20)` (`:150`) :
    /// asseoir le citoyen pour `ticks` ticks de jeu.
    Sit {
        /// Le siège.
        pos: Pos,
        /// `maxSittingTicks * 20` — entre 1 200 et 2 380 ticks.
        ticks: u32,
    },
    /// `worker.stopRiding()` puis `worker.setPos(y + 1)` (`:175-176`) : se lever.
    StandUp,
    /// `worker.setItemSlot(MAINHAND, <feuille>)` (`:157`).
    HoldItem(&'static str),
    /// `worker.setItemSlot(MAINHAND, ItemStack.EMPTY)` (`:162`, `:172`).
    EmptyHand,
    /// `CircleParticleEffectMessage(worker.position().add(0, 1, 0), kind, count)`
    /// (`:158`, `:163`) : un anneau de `count` particules au-dessus du citoyen.
    Particle {
        /// La particule.
        kind: Particle,
        /// `sittingTicks`, la quantité passée au message.
        count: u32,
    },
    /// `InventoryUtils.reduceStackInItemHandler(inv, PAPER, 1)` et la statistique
    /// `ITEM_USED` (`:183-184`).
    ConsumePaper,
    /// `addXpToSkill(Intelligence, 50 * (1 + TEACHING))` (`:188`), avec la
    /// statistique `LEVELS_GAINED` (`:192-195`) : le gain de niveau est calculé par
    /// le porteur de compétences, pas ici.
    PayLesson {
        /// Le montant, [`lesson_xp`].
        xp: f64,
    },
    /// `worker.decreaseSaturationForContinuousAction()` (`:198`).
    DecreaseSaturation,
    /// `carpet.remove(returnPos)` sur l'école (`BuildingSchool.java:142`) : le tapis
    /// tiré au sort n'est plus un tapis, la hutte doit l'oublier.
    ForgetCarpet(Pos),
    /// `triggerInteraction(new StandardInteraction(PUPIL_NO_CARPET, BLOCKING))`
    /// (`:98`) : la seule plainte du métier.
    Complaint(&'static str),
}

/// Ce qu'une décision a produit : l'état qu'elle rend et ce qu'elle demande au monde.
#[derive(Clone, Debug, PartialEq)]
pub struct PupilStep {
    /// L'état rendu et le refus éventuel.
    pub decision: Decision,
    /// Ce que le monde doit faire pour que la décision s'applique, dans l'ordre Java.
    pub asks: Vec<PupilAsk>,
}

impl PupilStep {
    /// `return getState()` : rester, sans rien demander.
    pub fn stay() -> PupilStep {
        PupilStep {
            decision: Decision::stay(),
            asks: Vec::new(),
        }
    }

    /// Rester et demander `asks`.
    pub fn stay_with(asks: Vec<PupilAsk>) -> PupilStep {
        PupilStep {
            decision: Decision::stay(),
            asks,
        }
    }

    /// Aller vers `state`.
    pub fn go(state: PupilState) -> PupilStep {
        PupilStep {
            decision: Decision::go(state),
            asks: Vec::new(),
        }
    }

    /// `return getState()` avec la raison notée.
    pub fn refuse(refusal: PupilRefusal) -> PupilStep {
        PupilStep {
            decision: Decision::refuse(refusal),
            asks: Vec::new(),
        }
    }

    /// Aller vers `state` avec la raison notée.
    pub fn refuse_and_go(state: PupilState, refusal: PupilRefusal) -> PupilStep {
        PupilStep {
            decision: Decision::go_after(state, refusal),
            asks: Vec::new(),
        }
    }

    /// La même décision, avec une demande de plus, ajoutée en fin de liste.
    pub fn and_ask(mut self, ask: PupilAsk) -> PupilStep {
        self.asks.push(ask);
        self
    }
}

/// La durée d'une leçon, en secondes : `nextInt(120 / 2) + 60` (`:149`).
///
/// Le tirage est un fait ; il est ramené dans `0..60` pour qu'une valeur hors bornes
/// ne panique pas et ne change pas la loi de probabilité (chaque valeur de `0..60`
/// reste équiprobable).
pub fn sit_seconds(sit_draw: u32) -> u32 {
    sit_draw % SIT_SECONDS_SPREAD + SIT_SECONDS_FLOOR
}

/// La même durée en ticks de jeu : `maxSittingTicks * 20` (`:150`), le `20` étant
/// `TICKS_SECOND`.
pub fn sit_ticks(sit_draw: u32) -> u32 {
    sit_seconds(sit_draw) * TICKS_SECOND
}

/// Ce qu'une leçon rapporte : `50.0 * (1 + TEACHING)` (`:186`), et **zéro sans
/// papier** (`:179-196` : le bloc d'XP entier est dans le `if (slot != -1)`).
pub fn lesson_xp(has_paper: bool, teaching_strength: f64) -> f64 {
    if !has_paper {
        return 0.0;
    }
    LESSON_INTELLIGENCE_XP * (1.0 + teaching_strength)
}

/// Où en est la leçon en cours, d'après la mémoire seule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive : les trois situations que `:146-170` distingue
pub enum LessonProgress {
    /// `sittingTicks == 0` ou le citoyen n'est plus assis (`:146`) : il faut le
    /// rasseoir, ce qui retire une durée.
    NeedsSeat,
    /// `sittingTicks < maxSittingTicks` (`:167`) : la leçon continue.
    Studying,
    /// `sittingTicks >= maxSittingTicks` (`:167`) : la leçon est finie, `:172-202`
    /// paie et remet les compteurs à zéro.
    Over,
}

/// `:146` et `:167` : ce que la mémoire dit de la leçon en cours.
pub fn lesson_progress(memory: &PupilMemory) -> LessonProgress {
    if memory.sitting_ticks == 0 || !memory.riding {
        LessonProgress::NeedsSeat
    } else if memory.sitting_ticks < memory.max_sitting_ticks {
        LessonProgress::Studying
    } else {
        LessonProgress::Over
    }
}

/// Le siège que `BuildingSchool.getRandomPlaceToSit` rend, et le tapis qu'il faut
/// oublier au passage.
///
/// Référence Java : `BuildingSchool.java:125-144`. L'ordre est celui du fichier : le
/// tag `sit` du schéma d'abord (`:127-131`), les tapis ensuite (`:133-143`), et un
/// tapis qui n'est plus un tapis est **retiré** puis rend `null` (`:138-143`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeatChoice {
    /// Le siège, `None` quand l'école n'en a aucun de valable.
    pub seat: Option<Pos>,
    /// Le tapis que la hutte doit oublier (`:142`), quand le tirage est tombé sur un
    /// tapis qui n'en est plus un.
    pub forgot: Option<Pos>,
}

/// `BuildingSchool.getRandomPlaceToSit` (`:126-152`), sur les faits.
pub fn seat_to_sit(facts: &PupilFacts) -> SeatChoice {
    // :127-131 — le tag du schéma passe avant les tapis.
    if !facts.sitting_tag.is_empty() {
        let index = facts.seat_pick % facts.sitting_tag.len();
        return SeatChoice {
            seat: Some(facts.sitting_tag[index]),
            forgot: None,
        };
    }
    // :133-136 — pas de tapis enregistré : `null`.
    if facts.carpets.is_empty() {
        return SeatChoice {
            seat: None,
            forgot: None,
        };
    }
    // :137-143 — le tapis tiré est re-testé ; s'il a disparu, il est retiré de la
    // liste et la méthode rend `null` *pour ce tick*.
    let index = facts.seat_pick % facts.carpets.len();
    let drawn = facts.carpets[index];
    if drawn.still_a_carpet {
        SeatChoice {
            seat: Some(drawn.pos),
            forgot: None,
        }
    } else {
        SeatChoice {
            seat: None,
            forgot: Some(drawn.pos),
        }
    }
}

/// `decide` (`:87-104`) : récréation, ou siège, ou plainte.
///
/// Écrit `studyPos` (`:102`) dans la mémoire quand un siège est trouvé, et **le laisse
/// tel quel** sinon : Java ne l'efface pas sur cet échec, et c'est ce qui permet à
/// `START_WORKING → STUDY` (`:222`) de se rasseoir au siège de la leçon précédente
/// après une récréation.
pub fn decide(facts: &PupilFacts, memory: &mut PupilMemory) -> PupilStep {
    // :89-92 — une fois sur dix, récréation, et le siège n'est même pas cherché.
    if facts.recess_roll % STUDY_TO_RECESS_RATIO < 1 {
        return PupilStep::go(PupilState::Recess);
    }

    let choice = seat_to_sit(facts);
    let mut asks = Vec::new();
    if let Some(forgot) = choice.forgot {
        asks.push(PupilAsk::ForgetCarpet(forgot));
    }

    match choice.seat {
        Some(pos) => {
            memory.study_pos = Some(pos); // :102
            PupilStep {
                decision: Decision::go(PupilState::Study), // :103
                asks,
            }
        }
        None => {
            // :96-100 — l'interaction bloquante, puis on reste en `DECIDE` : le
            // rythme de 20 ticks (`:75`) fait retenter une fois par seconde.
            asks.push(PupilAsk::Complaint(PUPIL_NO_CARPET));
            let refusal = if choice.forgot.is_some() {
                PupilRefusal::StaleCarpet
            } else {
                PupilRefusal::NoSeat
            };
            PupilStep {
                decision: Decision::refuse(refusal),
                asks,
            }
        }
    }
}

/// `recess` (`:111-120`) : courir dans l'école, une fois sur dix s'arrêter.
pub fn recess(facts: &PupilFacts) -> PupilStep {
    // :113-116 — il faut que la marche précédente soit finie **et** que le tirage
    // tombe : sinon un écolier qui n'a jamais fini de marcher ne sortirait jamais.
    if facts.navigation_done && facts.recess_exit_roll % STUDY_TO_RECESS_RATIO < 1 {
        return PupilStep::go(PupilState::Base(WorkerState::StartWorking));
    }
    // :118-119 — la promenade, dont le résultat n'est pas lu.
    PupilStep::stay_with(vec![PupilAsk::Wander {
        radius: WANDER_RADIUS,
        corners: facts.corners,
    }])
}

/// `startWorkingAtOwnBuilding` (`:216-223`) : marcher jusqu'à l'école, puis `STUDY`.
///
/// **Pas `DECIDE`.** Java va droit au `STUDY` (`:222`) ; c'est le `studyPos == null`
/// du premier passage qui le renvoie en `DECIDE` (`:129-132`). Faire le saut ici
/// changerait la suite d'états d'un tick.
pub fn start_working(facts: &PupilFacts) -> PupilStep {
    if !facts.at_building {
        // :218-221 — encore en chemin : on reste en `START_WORKING`.
        return PupilStep::refuse(PupilRefusal::NotAtBuilding).and_ask(PupilAsk::WalkToBuilding);
    }
    PupilStep::go(PupilState::Study) // :222
}

/// `study` (`:127-203`) : s'asseoir, tenir la feuille, étudier, payer.
///
/// La mémoire est modifiée sur place comme en Java — c'est l'état propre de l'IA, pas
/// le monde. Cinq sorties, dans l'ordre du fichier :
///
/// 1. `studyPos == null` → `DECIDE` (`:129-132`) ;
/// 2. pas arrivé au siège → on reste, en marchant (`:134-137`) ;
/// 3. un citoyen occupe le siège → `studyPos` effacé, `DECIDE` (`:139-144`) ;
/// 4. leçon en cours → on reste (`:167-170`) ;
/// 5. leçon finie → `return null` (`:202`), c'est-à-dire **on reste en `STUDY`** et le
///    tick suivant rassoit le citoyen avec une durée neuve.
pub fn study(facts: &PupilFacts, memory: &mut PupilMemory) -> PupilStep {
    // 1. `:129-132`.
    let Some(seat) = memory.study_pos else {
        return PupilStep::refuse_and_go(
            PupilState::Base(WorkerState::Decide),
            PupilRefusal::NoStudyPos,
        );
    };

    // 2. `:134-137` — la marche est demandée, l'état ne change pas.
    if !facts.at_seat {
        return PupilStep::refuse(PupilRefusal::NotAtSeat).and_ask(PupilAsk::WalkToSeat(seat));
    }

    // 3. `:139-144` — la boîte d'un point : seul un citoyen posé là la touche.
    if facts.other_citizen_at_seat {
        memory.study_pos = None; // :142
        return PupilStep::refuse_and_go(
            PupilState::Base(WorkerState::Decide),
            PupilRefusal::SeatTaken,
        );
    }

    let mut asks = Vec::new();

    // 4a. `:146-151` — s'asseoir, et retirer la durée de la leçon.
    if lesson_progress(memory) == LessonProgress::NeedsSeat {
        let seconds = sit_seconds(facts.sit_draw);
        memory.max_sitting_ticks = seconds; // :149
        memory.riding = true; // `SittingEntity.sitDown` (:150)
        asks.push(PupilAsk::Sit {
            pos: seat,
            ticks: seconds * TICKS_SECOND, // :150
        });
    }

    // 4b. `:153-164` — la feuille en main (ou rien), et l'anneau de particules.
    if facts.has_paper {
        asks.push(PupilAsk::HoldItem(PAPER)); // :157
        asks.push(PupilAsk::Particle {
            kind: Particle::Enchant,               // :158
            count: memory.sitting_ticks,           // `sittingTicks` avant l'incrément
        });
    } else {
        asks.push(PupilAsk::EmptyHand); // :162
        asks.push(PupilAsk::Particle {
            kind: Particle::HappyVillager, // :163
            count: memory.sitting_ticks,
        });
    }

    // 4c. `:166-170` — une seconde de plus, et on continue.
    memory.sitting_ticks += 1; // :166
    if memory.sitting_ticks < memory.max_sitting_ticks {
        return PupilStep::stay_with(asks); // :167-170
    }

    // 5. `:172-202` — la leçon est finie.
    asks.push(PupilAsk::EmptyHand); // :172
    if memory.riding {
        asks.push(PupilAsk::StandUp); // :173-177
    }

    let mut refusal = None;
    if facts.has_paper {
        asks.push(PupilAsk::ConsumePaper); // :183-184
        asks.push(PupilAsk::PayLesson {
            xp: lesson_xp(true, facts.teaching), // :186-188
        });
    } else {
        // Pas d'arrêt : juste une leçon qui ne vaut rien (`:160-164`, `:179`).
        refusal = Some(PupilRefusal::NoPaper);
    }
    asks.push(PupilAsk::DecreaseSaturation); // `:198`

    memory.max_sitting_ticks = 0; // :200
    memory.sitting_ticks = 0; // :201
    memory.riding = false; // `stopRiding`, `:175`

    PupilStep {
        // `:202` — `return null` : aucun changement d'état, le tick s'arrête là.
        decision: Decision {
            next: None,
            refusal,
        },
        asks,
    }
}

// ---------------------------------------------------------------------------
// Le câblage : les cinq décisions deviennent cinq actions de machine.
// ---------------------------------------------------------------------------

/// Pousse les demandes d'un pas dans la scène et applique sa décision.
fn apply(scene: &mut PupilScene, state: PupilState, step: PupilStep) -> Option<PupilState> {
    scene.asks.extend(step.asks);
    if step.decision.refusal.is_some() {
        scene.refusal = step.decision.refusal;
    }
    step.decision.next.or(Some(state))
}

fn start_working_action(scene: &mut PupilScene, state: PupilState) -> Option<PupilState> {
    let step = start_working(&scene.facts);
    apply(scene, state, step)
}

fn decide_action(scene: &mut PupilScene, state: PupilState) -> Option<PupilState> {
    let step = decide(&scene.facts, &mut scene.memory);
    apply(scene, state, step)
}

fn study_action(scene: &mut PupilScene, state: PupilState) -> Option<PupilState> {
    let step = study(&scene.facts, &mut scene.memory);
    apply(scene, state, step)
}

fn recess_action(scene: &mut PupilScene, state: PupilState) -> Option<PupilState> {
    let step = recess(&scene.facts);
    apply(scene, state, step)
}

/// Enregistre les cinq cibles de `EntityAIWorkPupil.java:72-78` et les trois états de
/// base où le contrôle revient.
pub fn register(machine: &mut Machine<PupilState, PupilScene>) {
    // :73 — `new AITarget(IDLE, START_WORKING, 1)`. Rythme 1 : rien à attendre.
    machine.add(Transition::goto(
        PupilState::Base(WorkerState::Idle),
        PupilState::Base(WorkerState::StartWorking),
        "startWorking",
        "EntityAIWorkPupil.java:73",
        1,
    ));
    // :74 — `new AITarget(START_WORKING, this::startWorkingAtOwnBuilding, TICKS_SECOND)`.
    machine.add(Transition::target(
        PupilState::Base(WorkerState::StartWorking),
        "startWorkingAtOwnBuilding",
        "EntityAIWorkPupil.java:74",
        TICKS_SECOND,
        Action::Run(start_working_action),
    ));
    // :75 — `new AITarget(DECIDE, this::decide, TICKS_SECOND)`.
    machine.add(Transition::target(
        PupilState::Base(WorkerState::Decide),
        "decide",
        "EntityAIWorkPupil.java:75",
        TICKS_SECOND,
        Action::Run(decide_action),
    ));
    // :76 — `new AITarget(STUDY, this::study, TICKS_SECOND)`.
    machine.add(Transition::target(
        PupilState::Study,
        "study",
        "EntityAIWorkPupil.java:76",
        TICKS_SECOND,
        Action::Run(study_action),
    ));
    // :77 — `new AITarget(RECESS, this::recess, TICKS_SECOND)`.
    machine.add(Transition::target(
        PupilState::Recess,
        "recess",
        "EntityAIWorkPupil.java:77",
        TICKS_SECOND,
        Action::Run(recess_action),
    ));

    // Les trois états de base où la table de base renvoie l'écolier, et le retour.
    // `afterDump` (`AbstractEntityAIBasic.java:1213-1222`).
    machine.add(Transition::goto(
        PupilState::Base(WorkerState::InventoryFull),
        PupilState::Base(WorkerState::Idle),
        "afterDump",
        "AbstractEntityAIBasic.java:1213-1222",
        crate::ai::worker::STANDARD_DELAY,
    ));
    // `waitForRequests` → `IDLE` (`AbstractEntityAIBasic.java:238`).
    machine.add(Transition::goto(
        PupilState::Base(WorkerState::NeedsItem),
        PupilState::Base(WorkerState::Idle),
        "waitForRequests",
        "AbstractEntityAIBasic.java:238",
        crate::ai::worker::STANDARD_DELAY,
    ));
    // `PAUSED` → `IDLE` au dégel (`AbstractEntityAIBasic.java:251`).
    machine.add(Transition::goto(
        PupilState::Base(WorkerState::Paused),
        PupilState::Base(WorkerState::Idle),
        "bePaused",
        "AbstractEntityAIBasic.java:251",
        TICKS_SECOND,
    ));
}

/// Une machine avec cette table dessus, qui démarre en `IDLE` (`:73`).
pub fn ai() -> Machine<PupilState, PupilScene> {
    let mut machine = Machine::new(PupilState::Base(WorkerState::Idle), ENTITY_AI_TICKRATE);
    register(&mut machine);
    machine
}

/// Ce qu'un tick a décidé : l'état en force, la transition qui a tiré, et le refus —
/// les trois mêmes réponses que [`crate::ai::worker::Outcome`], pour un
/// [`PupilState`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PupilOutcome {
    /// Le but en force après ce tick.
    pub state: PupilState,
    /// La transition qui a tiré, si une a tiré.
    pub fired: Option<Fired<PupilState>>,
    /// Le refus noté pendant ce tick.
    pub refusal: Option<PupilRefusal>,
}

impl PupilOutcome {
    /// Le but en force.
    pub fn goal(&self) -> PupilState {
        self.state
    }

    /// Pourquoi rien n'a bougé.
    pub fn why_not(&self) -> Option<PupilRefusal> {
        self.refusal
    }

    /// Si ce tick a changé le but.
    pub fn moved(&self) -> bool {
        matches!(self.fired, Some(fired) if fired.from != fired.to)
    }

    /// Si la machine est tombée dans un piège (`BasicStateMachine.java:220-228`).
    pub fn faulted(&self) -> bool {
        matches!(self.fired, Some(fired) if fired.reset)
    }

    /// La règle qui a décidé.
    pub fn why(&self) -> Option<&'static str> {
        self.fired.map(|fired| fired.rule)
    }
}

/// L'IA de l'écolier : la machine, et le tick qui vide les demandes.
pub struct PupilAi {
    machine: Machine<PupilState, PupilScene>,
}

impl Default for PupilAi {
    fn default() -> PupilAi {
        PupilAi::new()
    }
}

impl PupilAi {
    /// Une IA neuve, en `IDLE`.
    pub fn new() -> PupilAi {
        PupilAi { machine: ai() }
    }

    /// L'état en force.
    pub fn state(&self) -> PupilState {
        self.machine.state()
    }

    /// La machine, pour un test qui veut la table.
    pub fn machine(&self) -> &Machine<PupilState, PupilScene> {
        &self.machine
    }

    /// `AbstractAISkeleton.resetAI` (`:145-152`).
    pub fn reset(&mut self) {
        self.machine.reset();
    }

    /// Un tick : vide les demandes et le refus, puis laisse la machine décider.
    pub fn tick(&mut self, scene: &mut PupilScene) -> PupilOutcome {
        scene.asks.clear();
        scene.refusal = None;
        let fired = self.machine.tick(scene);
        PupilOutcome {
            state: self.machine.state(),
            fired,
            refusal: scene.refusal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_work_05::facts::Seat;

    fn school() -> PupilFacts {
        PupilFacts {
            carpets: vec![Seat::carpet(Pos::new(2, 64, 2))],
            has_paper: true,
            ..PupilFacts::default()
        }
    }

    #[test]
    fn the_lesson_lasts_between_sixty_and_one_hundred_nineteen_seconds() {
        // `:149` : `nextInt(120 / 2) + 60`. Onze secondes de plus que 119 n'existent
        // pas ; 59 non plus.
        assert_eq!(sit_seconds(0), 60);
        assert_eq!(sit_seconds(59), 119);
        assert_eq!(sit_seconds(60), 60, "un tirage hors bornes revient dedans");
        assert_eq!(sit_seconds(u32::MAX), 60 + u32::MAX % 60);
        for draw in 0..60 {
            let seconds = sit_seconds(draw);
            assert!((60..120).contains(&seconds), "{draw} -> {seconds}");
        }
        // `:150` : la durée passée à `sitDown` est en ticks.
        assert_eq!(sit_ticks(0), 1_200);
        assert_eq!(sit_ticks(59), 2_380);
        assert_eq!(sit_ticks(0), sit_seconds(0) * TICKS_SECOND);
    }

    #[test]
    fn a_lesson_pays_fifty_intelligence_times_the_teaching_research() {
        // `:186` : `50.0 * (1 + getEffectStrength(TEACHING))`.
        assert_eq!(lesson_xp(false, 0.0), 0.0);
        assert_eq!(lesson_xp(true, 0.0), 50.0);
        assert_eq!(lesson_xp(true, 0.5), 75.0);
        assert_eq!(lesson_xp(true, 1.0), 100.0);
        // Sans papier, la recherche ne sert à rien : le bloc d'XP est dans le `if`.
        assert_eq!(lesson_xp(false, 3.0), 0.0);
    }

    #[test]
    fn the_tag_sit_wins_over_the_carpets() {
        // `BuildingSchool.java:127-131` sort avant même de regarder les tapis.
        let facts = PupilFacts {
            sitting_tag: vec![Pos::new(9, 64, 9), Pos::new(8, 64, 8)],
            carpets: vec![Seat::carpet(Pos::new(2, 64, 2))],
            seat_pick: 1,
            ..PupilFacts::default()
        };
        assert_eq!(seat_to_sit(&facts).seat, Some(Pos::new(8, 64, 8)));
        // Un indice hors bornes revient dans la liste au lieu de paniquer.
        let out = PupilFacts {
            seat_pick: 7,
            ..facts
        };
        assert_eq!(seat_to_sit(&out).seat, Some(Pos::new(8, 64, 8)), "7 % 2");
    }

    #[test]
    fn a_carpet_that_is_no_longer_one_is_forgotten_and_refused() {
        // `:143-146` : le tapis est retiré de l'école, et la méthode rend `null` pour
        // ce tick — donc `DECIDE`, et la plainte bloquante.
        let facts = PupilFacts {
            carpets: vec![Seat::carpet(Pos::new(1, 64, 1)), Seat::stale(Pos::new(2, 64, 2))],
            seat_pick: 1,
            ..PupilFacts::default()
        };
        let choice = seat_to_sit(&facts);
        assert_eq!(choice.seat, None);
        assert_eq!(choice.forgot, Some(Pos::new(2, 64, 2)));
    }

    #[test]
    fn one_pupil_in_ten_takes_a_recess() {
        // `:89` : `nextInt(10) < 1`, donc seulement le tirage 0.
        let mut memory = PupilMemory::default();
        let step = decide(
            &PupilFacts {
                recess_roll: 0,
                ..school()
            },
            &mut memory,
        );
        assert_eq!(step.decision.next, Some(PupilState::Recess));
        assert_eq!(memory.study_pos, None, "le siège n'est même pas cherché");
        // Et les neuf autres vont en STUDY, siège en mémoire.
        let mut memory = PupilMemory::default();
        let step = decide(
            &PupilFacts {
                recess_roll: 1,
                ..school()
            },
            &mut memory,
        );
        assert_eq!(step.decision.next, Some(PupilState::Study));
        assert_eq!(memory.study_pos, Some(Pos::new(2, 64, 2)));
    }

    #[test]
    fn an_empty_school_keeps_the_pupil_in_decide_and_complains() {
        // `:96-100`.
        let mut memory = PupilMemory::default();
        let step = decide(&PupilFacts::default(), &mut memory);
        assert_eq!(step.decision.next, None, "`return DECIDE`");
        assert_eq!(step.decision.refusal, Some(PupilRefusal::NoSeat));
        assert_eq!(step.asks.len(), 1);
        assert_eq!(
            step.asks[0],
            PupilAsk::Complaint("com.minecolonies.coremod.school.nocarpet")
        );
        // L'école existante est conservée : Java n'efface pas `studyPos` ici (`:99`).
        memory.study_pos = Some(Pos::new(5, 64, 5));
        let again = decide(&PupilFacts::default(), &mut memory);
        assert_eq!(again.decision.refusal, Some(PupilRefusal::NoSeat));
        assert_eq!(memory.study_pos, Some(Pos::new(5, 64, 5)));
    }

    #[test]
    fn a_recess_ends_when_the_walk_is_done_and_the_draw_comes_up() {
        // `:113` : les deux conditions, et le tirage seul ne suffit pas.
        let facts = |navigation_done, recess_exit_roll| PupilFacts {
            navigation_done,
            recess_exit_roll,
            ..PupilFacts::default()
        };
        assert_eq!(
            recess(&facts(true, 0)).decision.next,
            Some(PupilState::Base(WorkerState::StartWorking))
        );
        assert_eq!(recess(&facts(false, 0)).decision.next, None, "marche en cours");
        assert_eq!(recess(&facts(true, 1)).decision.next, None, "9 fois sur 10");
        // Tant qu'il court, il redemande une promenade (`:118`).
        let running = recess(&facts(true, 1));
        assert_eq!(
            running.asks,
            vec![PupilAsk::Wander {
                radius: WANDER_RADIUS,
                corners: None
            }]
        );
    }

    #[test]
    fn the_first_study_of_a_lesson_sits_the_pupil_and_counts_one_second() {
        // `:146-151` puis `:166-170`.
        let mut memory = PupilMemory {
            study_pos: Some(Pos::new(2, 64, 2)),
            ..PupilMemory::default()
        };
        let facts = PupilFacts {
            sit_draw: 0,
            has_paper: true,
            ..PupilFacts::default()
        };
        let step = study(&facts, &mut memory);
        assert_eq!(memory.max_sitting_ticks, 60, "`:149`, en secondes");
        assert!(memory.riding, "`sitDown` a été demandée");
        assert_eq!(memory.sitting_ticks, 1, "`:166`");
        assert_eq!(step.decision.next, None, "`:167-170`, on reste en STUDY");
        assert_eq!(
            step.asks,
            vec![
                PupilAsk::Sit {
                    pos: Pos::new(2, 64, 2),
                    ticks: 1_200
                },
                PupilAsk::HoldItem("minecraft:paper"),
                PupilAsk::Particle {
                    kind: Particle::Enchant,
                    count: 0
                },
            ]
        );
    }

    #[test]
    fn a_lesson_without_paper_still_runs_and_pays_nothing() {
        // `:160-164` : main vide et particule joyeuse, aucune XP.
        let mut memory = PupilMemory {
            study_pos: Some(Pos::new(2, 64, 2)),
            ..PupilMemory::default()
        };
        let facts = PupilFacts {
            has_paper: false,
            ..PupilFacts::default()
        };
        let step = study(&facts, &mut memory);
        assert!(step.asks.contains(&PupilAsk::EmptyHand));
        assert!(step
            .asks
            .contains(&PupilAsk::Particle {
                kind: Particle::HappyVillager,
                count: 0
            }));
        assert!(!step.asks.contains(&PupilAsk::HoldItem(PAPER)));
        assert!(!step.asks.iter().any(|ask| matches!(ask, PupilAsk::PayLesson { .. })));
        assert_eq!(memory.sitting_ticks, 1);
    }

    #[test]
    fn the_end_of_a_lesson_pays_then_stays_in_study() {
        // `:172-202` : main vidée, levé, feuille consommée, XP, satiété, compteurs à
        // zéro, `return null`.
        let mut memory = PupilMemory {
            study_pos: Some(Pos::new(2, 64, 2)),
            sitting_ticks: 1,
            max_sitting_ticks: 1,
            riding: true,
        };
        let facts = PupilFacts {
            has_paper: true,
            teaching: 0.5,
            ..PupilFacts::default()
        };
        let step = study(&facts, &mut memory);
        assert_eq!(step.decision.next, None, "`:202` rend null");
        assert_eq!(step.decision.refusal, None);
        assert_eq!(
            step.asks,
            vec![
                PupilAsk::HoldItem("minecraft:paper"),
                PupilAsk::Particle {
                    kind: Particle::Enchant,
                    count: 1
                },
                PupilAsk::EmptyHand,
                PupilAsk::StandUp,
                PupilAsk::ConsumePaper,
                PupilAsk::PayLesson { xp: 75.0 },
                PupilAsk::DecreaseSaturation,
            ]
        );
        assert_eq!(memory.sitting_ticks, 0);
        assert_eq!(memory.max_sitting_ticks, 0);
        assert!(!memory.riding);
        assert_eq!(memory.study_pos, Some(Pos::new(2, 64, 2)), "Java le garde");
    }

    #[test]
    fn another_citizen_on_the_seat_forgets_it_and_goes_back_to_decide() {
        // `:139-144`.
        let mut memory = PupilMemory {
            study_pos: Some(Pos::new(2, 64, 2)),
            ..PupilMemory::default()
        };
        let facts = PupilFacts {
            other_citizen_at_seat: true,
            ..school()
        };
        let step = study(&facts, &mut memory);
        assert_eq!(
            step.decision.next,
            Some(PupilState::Base(WorkerState::Decide))
        );
        assert_eq!(step.decision.refusal, Some(PupilRefusal::SeatTaken));
        assert_eq!(memory.study_pos, None, "`:142`");
        assert!(step.asks.is_empty(), "aucune particule avant de s'asseoir");
    }

    #[test]
    fn a_study_without_a_seat_goes_back_to_decide() {
        // `:129-132`.
        let mut memory = PupilMemory::default();
        let step = study(&school(), &mut memory);
        assert_eq!(
            step.decision.next,
            Some(PupilState::Base(WorkerState::Decide))
        );
        assert_eq!(step.decision.refusal, Some(PupilRefusal::NoStudyPos));
    }

    #[test]
    fn a_pupil_who_is_not_at_the_seat_keeps_walking() {
        // `:134-137`.
        let mut memory = PupilMemory {
            study_pos: Some(Pos::new(2, 64, 2)),
            ..PupilMemory::default()
        };
        let facts = PupilFacts {
            at_seat: false,
            ..school()
        };
        let step = study(&facts, &mut memory);
        assert_eq!(step.decision.next, None);
        assert_eq!(step.decision.refusal, Some(PupilRefusal::NotAtSeat));
        assert_eq!(step.asks, vec![PupilAsk::WalkToSeat(Pos::new(2, 64, 2))]);
    }

    #[test]
    fn start_working_goes_straight_to_study_not_to_decide() {
        // `:222` : c'est `study` qui renverra en `DECIDE`, au tick suivant.
        let arrived = start_working(&school());
        assert_eq!(arrived.decision.next, Some(PupilState::Study));
        assert!(arrived.asks.is_empty());
        let walking = start_working(&PupilFacts {
            at_building: false,
            ..school()
        });
        assert_eq!(walking.decision.next, None, "`:220`");
        assert_eq!(walking.decision.refusal, Some(PupilRefusal::NotAtBuilding));
        assert_eq!(walking.asks, vec![PupilAsk::WalkToBuilding]);
    }

    #[test]
    fn the_machine_registers_the_five_targets_and_three_base_exits() {
        let machine = ai();
        let rules: Vec<&str> = machine
            .transitions()
            .iter()
            .map(|transition| transition.rule)
            .collect();
        assert_eq!(
            rules,
            vec![
                "startWorking",
                "startWorkingAtOwnBuilding",
                "decide",
                "study",
                "recess",
                "afterDump",
                "waitForRequests",
                "bePaused",
            ]
        );
        // Les rythmes sont ceux du fichier : 1 pour IDLE (`:73`), 20 pour les autres.
        let rates: Vec<u32> = machine
            .transitions()
            .iter()
            .map(|transition| transition.tick_rate)
            .collect();
        assert_eq!(rates, vec![1, 20, 20, 20, 20, 5, 5, 20]);
        assert_eq!(machine.state(), PupilState::Base(WorkerState::Idle));
    }

    #[test]
    fn every_state_the_pupil_can_be_in_has_a_way_out() {
        // Le piège de `BasicStateMachine.java:220-228` remet l'IA en `INIT` ; un état
        // sans transition est un bug du port, pas un événement de jeu.
        let machine = ai();
        for state in PupilState::ALL {
            assert!(
                machine.can_leave(state),
                "{} n'a aucune transition",
                state.java()
            );
        }
    }

    #[test]
    fn lesson_progress_follows_the_two_tests_of_java() {
        // `:146` d'abord, `:167` ensuite.
        assert_eq!(
            lesson_progress(&PupilMemory::default()),
            LessonProgress::NeedsSeat
        );
        assert_eq!(
            lesson_progress(&PupilMemory {
                sitting_ticks: 30,
                max_sitting_ticks: 60,
                riding: false,
                study_pos: None,
            }),
            LessonProgress::NeedsSeat,
            "le véhicule a disparu"
        );
        assert_eq!(
            lesson_progress(&PupilMemory {
                sitting_ticks: 30,
                max_sitting_ticks: 60,
                riding: true,
                study_pos: None,
            }),
            LessonProgress::Studying
        );
        assert_eq!(
            lesson_progress(&PupilMemory {
                sitting_ticks: 60,
                max_sitting_ticks: 60,
                riding: true,
                study_pos: None,
            }),
            LessonProgress::Over
        );
    }
}
