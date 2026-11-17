//! Les buts d'IA des trois éleveurs restants : le lapinier, le berger, et le
//! palefrenier.
//!
//! Java référence trois classes de `core/entity/ai/workers/production/herders/`, toutes
//! filles d'`AbstractEntityAIHerder` :
//!
//! | Métier | Classe Java | Lignes | Ce qu'elle ajoute à la table du parent |
//! | --- | --- | ---: | --- |
//! | lapinier | `EntityAIWorkRabbitHerder.java` | 69 | **aucune cible** ; une frappe au tirage (`:54-68`) et une carotte au-dessus de la tête (`:37-46`) |
//! | berger | `EntityAIWorkShepherd.java` | 188 | une cible `SHEPHERD_SHEAR` (`:50`), la surcharge de `decideWhatToDo` (`:73-87`) et la teinture (`:173-187`) |
//! | palefrenier | `EntityAIWorkStablemaster.java` | 757 | **trois** cibles (`:102-104`), la surcharge de `decide` avec deux comptes à rebours (`:232-262`), le dressage (`:268-339`), la préparation au combat (`:346-422`, `:429-514`) et la rentrée au bercail (`:563-643`) |
//!
//! ## Ce que ce module possède, et ce qu'il traverse
//!
//! Le parent `AbstractEntityAIHerder` (874 lignes) porte la table que les trois
//! partagent : huit cibles (`:139-148`), `decideWhatToDo` (`:199-249`),
//! `breedAnimals`, `butcherAnimals`, `feedAnimal`, `pickupItems`. **Cette table est
//! déjà portée**, une fois, par `crate::ai_work_09::herder` — avec sa propre note qui
//! dit que ces trois métiers « se raccordent à `herder` et n'écrivent pas une cinquième
//! copie ».
//!
//! Ce raccordement ne peut pas être un appel de fonction : la machine de là-bas a pour
//! état `crate::ai_work_09::WorkState`, une énumération **fermée** sur les onze états de
//! ses trois métiers, et sa scène [`crate::ai_work_09::WorkScene`] a pour faits de métier
//! un `WorkJob` fermé sur les siens. `SHEPHERD_SHEAR` n'y a pas de place, et un berger ne
//! peut pas se déclarer `WorkJob::Cowboy` sans mentir sur ce qu'il est. Le raccordement
//! est donc **à la frontière que Java a lui-même posée** :
//!
//! - la table du parent tourne chez `crate::ai_work_09::herder` et rend une de ses cinq
//!   réponses ([`HerdChoice`]) ; l'appelant la dépose dans
//!   [`facts::PenFacts::base_answer`] — c'est le `super.decideWhatToDo()` de
//!   `EntityAIWorkShepherd.java:75` ;
//! - cette machine-ci possède **la surcharge** et les cibles que les sous-classes
//!   ajoutent, et rien d'autre ;
//! - les états traversés ([`HerdState::is_traversed`]) n'ont ici aucune règle : quand la
//!   machine s'y trouve, ce module a fini de décider et rend la main.
//!
//! Les constantes partagées (durée d'un tick d'IA, `DECIDING_DELAY`,
//! `XP_PER_ACTION`, `BUTCHERING_ATTACK_DAMAGE`, `ANIMAL_MULTIPLIER`,
//! `ONE_HUNDRED_PERCENT`) sont importées de `crate::ai_work_09` : une deuxième copie
//! d'une constante déjà citée serait une deuxième source de vérité.
//!
//! ## La machine
//!
//! ```ignore
//! let mut ai = HerdAi::new(HerdTrade::Shepherd);
//! let mut scene = ai.scene(WorldFacts::new());
//! scene.job = HerdJob::Shepherd(ShepherdFacts { shearing: true, shears_slot: Some(3), ..Default::default() });
//! scene.pen.animals.push(PenAnimal::sheep(7, Pos::new(1, 64, 0)));
//! scene.decide(HerdChoice::StartWorking);       // ce que super.decideWhatToDo() a rendu
//! let outcome = ai.tick(&mut scene);
//! assert_eq!(outcome.goal(), HerdState::ShepherdShear);
//! ```
//!
//! ## Ce qui n'est pas ici
//!
//! - **Aucun appel au monde.** Pas de `searchForAnimals`, pas de `walkToSafePos`, pas de
//!   `sheep.setSheared`, pas de `createFromVanilla` : chaque lecture est un fait
//!   ([`facts`]) et chaque écriture une valeur de [`facts::HerdAct`] que l'appelant
//!   applique. C'est ce qui rend les scénarios de [`tests`] rejouables.
//! - **Aucune API de mod.** Ce module n'importe pas `mod_api` : il ne dépend que de
//!   `crate::ai` et de `crate::ai_work_09`, le même choix que `crate::ai_gather` et
//!   `crate::ai_build`.
//! - **Les modules d'élevage.** `isCompatible`, `getBreedingItems`,
//!   `hasWorkerOpenRequestsOfType` : le port lit leurs **réponses** (une liste d'animaux
//!   compatibles, un emplacement d'objet, un booléen) parce que rejouer leurs règles
//!   serait porter trois modules de plus pour aucun état de plus.
//! - **Les requêtes.** `createRequestAsync` (`EntityAIWorkStablemaster.java:509`) est un
//!   [`facts::HerdAct::RequestedItems`] ; créer la requête est l'affaire de
//!   `crate::requests`.
//!
//! `src/ai_work_10/MANQUANT.md` liste ce que l'API de mod ne permet pas d'écrire et les
//! constats mesurés sur le Java.

pub mod facts;
pub mod rabbit;
pub mod shepherd;
pub mod stablemaster;
pub mod state;

#[cfg(test)]
mod tests;

use core::fmt;

use crate::ai::facts::WorldFacts;
use crate::ai::state::{Action, Fired, Machine, Transition, WorkerState};

pub use facts::{
    AnimalKind, Draws, HerdAct, HerdJob, HerdMemory, HerdScene, MountMaintenance, PenAnimal,
    PenFacts, RabbitFacts, ShepherdFacts, StableFacts, SAFE_POS_RANGE, WALK_TO_ANIMAL_RANGE,
    WALK_TO_ITEM_RANGE,
};
pub use state::{Hand, HerdChoice, HerdState, TradeRefusal};

/// Les trois métiers de cette tranche.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: three classes in this tranche, one name each
pub enum HerdTrade {
    /// `EntityAIWorkRabbitHerder` — un module d'élevage, aucune cible propre.
    RabbitHerder,
    /// `EntityAIWorkShepherd` — une cible (`:50`).
    Shepherd,
    /// `EntityAIWorkStablemaster` — trois cibles (`:102-104`).
    Stablemaster,
}

impl HerdTrade {
    /// Les trois métiers.
    pub const ALL: [HerdTrade; 3] = [
        HerdTrade::RabbitHerder,
        HerdTrade::Shepherd,
        HerdTrade::Stablemaster,
    ];

    /// La classe Java de ce métier, `fichier.java:ligne` de sa déclaration.
    pub fn java(self) -> &'static str {
        match self {
            HerdTrade::RabbitHerder => "EntityAIWorkRabbitHerder.java:20",
            HerdTrade::Shepherd => "EntityAIWorkShepherd.java:34",
            HerdTrade::Stablemaster => "EntityAIWorkStablemaster.java:58",
        }
    }

    /// La ligne qui enregistre les cibles propres au métier, quand il en a.
    ///
    /// Le lapinier n'en a aucune : son constructeur (`:32-35`) ne fait que relayer
    /// `super(job)`, donc sa table **est** celle du parent.
    pub fn registration(self) -> &'static str {
        match self {
            HerdTrade::RabbitHerder => "EntityAIWorkRabbitHerder.java:32-35",
            HerdTrade::Shepherd => "EntityAIWorkShepherd.java:49-52",
            HerdTrade::Stablemaster => "EntityAIWorkStablemaster.java:101-105",
        }
    }

    /// La ligne de la surcharge de `decideWhatToDo`, ou celle du parent quand le métier
    /// ne surcharge pas.
    pub fn decide(self) -> &'static str {
        match self {
            // Le lapinier ne surcharge pas : c'est `AbstractEntityAIHerder.java:199-249`
            // qui décide, et cette machine ne fait que traverser sa réponse.
            HerdTrade::RabbitHerder => "AbstractEntityAIHerder.java:199-249",
            HerdTrade::Shepherd => "EntityAIWorkShepherd.java:73-87",
            HerdTrade::Stablemaster => "EntityAIWorkStablemaster.java:232-262",
        }
    }

    /// Une scène vide de ce métier : la hutte telle qu'on la pose.
    pub fn job_facts(self) -> HerdJob {
        match self {
            HerdTrade::RabbitHerder => HerdJob::Rabbit(RabbitFacts::default()),
            HerdTrade::Shepherd => HerdJob::Shepherd(ShepherdFacts::default()),
            HerdTrade::Stablemaster => HerdJob::Stable(StableFacts::default()),
        }
    }

    /// La description que le joueur lit (`JobStatus` de l'écran de la hutte).
    pub fn key(self) -> &'static str {
        match self {
            HerdTrade::RabbitHerder => "job.rabbitherder",
            HerdTrade::Shepherd => "job.shepherd",
            HerdTrade::Stablemaster => "job.stablemaster",
        }
    }
}

impl fmt::Display for HerdTrade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key())
    }
}

/// Ce qu'une règle de ce module répond : l'état suivant (`None` = « je reste ») et, quand
/// elle s'arrête, pourquoi.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decision {
    /// L'état rendu, `None` pour « on reste dans l'état courant » — la même forme que
    /// le `Option` d'`Action::Run`.
    pub next: Option<HerdState>,
    /// Pourquoi elle s'est arrêtée, quand elle s'est arrêtée.
    pub refusal: Option<TradeRefusal>,
}

impl Decision {
    /// Aller en `state`.
    pub fn go(state: HerdState) -> Decision {
        Decision {
            next: Some(state),
            refusal: None,
        }
    }

    /// `return getState()` : aucun changement d'état, aucune plainte.
    pub fn stay() -> Decision {
        Decision {
            next: None,
            refusal: None,
        }
    }

    /// `return getState()` avec la raison notée.
    pub fn refuse(refusal: TradeRefusal) -> Decision {
        Decision {
            next: None,
            refusal: Some(refusal),
        }
    }

    /// Aller en `state` **et** noter pourquoi.
    pub fn go_after(state: HerdState, refusal: TradeRefusal) -> Decision {
        Decision {
            next: Some(state),
            refusal: Some(refusal),
        }
    }
}

/// Ce qu'un tick de cette machine a décidé.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// L'état de la machine après ce tick : le but en cours.
    pub state: HerdState,
    /// La transition qui a tiré, si une a tiré.
    pub fired: Option<Fired<HerdState>>,
    /// Le refus enregistré pendant ce tick.
    pub refusal: Option<TradeRefusal>,
    /// L'effet appliqué pendant ce tick, s'il y en a un.
    pub act: Option<HerdAct>,
}

impl Outcome {
    /// Le but en vigueur.
    pub fn goal(&self) -> HerdState {
        self.state
    }

    /// Pourquoi rien n'a avancé, si le tick a noté une raison.
    pub fn why_not(&self) -> Option<TradeRefusal> {
        self.refusal
    }

    /// Ce que ce tick a fait au monde.
    pub fn acted(&self) -> Option<HerdAct> {
        self.act
    }

    /// Vrai quand ce tick a changé le but.
    pub fn moved(&self) -> bool {
        matches!(self.fired, Some(fired) if fired.from != fired.to)
    }

    /// Vrai quand la machine est tombée dans un état piège
    /// (`BasicStateMachine.java:220-228`).
    pub fn faulted(&self) -> bool {
        matches!(self.fired, Some(fired) if fired.reset)
    }

    /// Les états que la machine traverse sans que ce module décide : la table de base,
    /// ou celle du parent.
    pub fn handed_back(&self) -> bool {
        self.state.is_traversed()
    }
}

/// L'IA d'un de ces trois éleveurs : son métier et sa machine, rien d'autre.
///
/// Les faits et la mémoire vivent dans [`HerdScene`], que l'appelant possède : un test
/// peut donc remplir une scène, la faire tourner, et relire les deux.
pub struct HerdAi {
    trade: HerdTrade,
    machine: Machine<HerdState, HerdScene>,
}

impl fmt::Debug for HerdAi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HerdAi")
            .field("trade", &self.trade)
            .field("machine", &self.machine)
            .finish()
    }
}

impl HerdAi {
    /// L'IA de `trade`, à partir de `DECIDE` et à `ENTITY_AI_TICKRATE`
    /// (`AbstractEntityCitizen.java:64`).
    ///
    /// Java commence en `INIT`, et `initSafetyChecks` (`AbstractEntityAIBasic.java:
    /// 446-465`) l'envoie en `IDLE` ; `INIT` et ces vérifications appartiennent à
    /// `crate::ai`. Cette machine commence là où la surcharge décide, c'est-à-dire sur
    /// `DECIDE` (`AbstractEntityAIHerder.java:143`).
    pub fn new(trade: HerdTrade) -> HerdAi {
        let mut machine = Machine::new(
            HerdState::Base(WorkerState::Decide),
            crate::ai::state::ENTITY_AI_TICKRATE,
        );
        register(&mut machine, trade);
        HerdAi { trade, machine }
    }

    /// Le métier de cette IA.
    pub fn trade(&self) -> HerdTrade {
        self.trade
    }

    /// Une scène vide de ce métier, sur `world`.
    pub fn scene(&self, world: WorldFacts) -> HerdScene {
        HerdScene::new(world, self.trade)
    }

    /// Une scène portant explicitement `job` — pour le test qui se trompe de métier.
    pub fn scene_with(&self, world: WorldFacts, job: HerdJob) -> HerdScene {
        HerdScene::with_job(world, self.trade, job)
    }

    /// L'état dans lequel la machine est.
    pub fn state(&self) -> HerdState {
        self.machine.state()
    }

    /// La machine, pour un test qui veut lire ses transitions.
    pub fn machine(&self) -> &Machine<HerdState, HerdScene> {
        &self.machine
    }

    /// `AbstractAISkeleton.resetAI` (`:145-152`).
    pub fn reset(&mut self) {
        self.machine.reset();
    }

    /// Un tick : `TickRateStateMachine.tick` (`:95-150`) sur la table de ce métier.
    ///
    /// Efface le refus et l'effet du tick précédent, puis rend ce que la machine a
    /// décidé. **Le contrat d'appel** : si la machine est en `DECIDE`, l'appelant a
    /// d'abord déposé la réponse de la table de base (`HerdScene::decide`) — c'est la
    /// ligne `super.decideWhatToDo()` que la sous-classe Java exécute dans son propre
    /// corps, avant de surcharger la réponse.
    pub fn tick(&mut self, scene: &mut HerdScene) -> Outcome {
        scene.refusal = None;
        scene.memory.act = None;
        let fired = self.machine.tick(scene);
        Outcome {
            state: self.machine.state(),
            fired,
            refusal: scene.refusal,
            act: scene.memory.act,
        }
    }
}

/// La décision de la surcharge, selon le métier : `EntityAIWorkShepherd.java:73-87`,
/// `EntityAIWorkStablemaster.java:232-262`, et pour le lapinier la réponse du parent
/// rendue telle quelle (il ne surcharge pas).
pub fn decide_what_to_do(scene: &mut HerdScene, from: HerdState) -> Decision {
    match scene.trade {
        HerdTrade::RabbitHerder => rabbit::decide_what_to_do(scene, from),
        HerdTrade::Shepherd => shepherd::decide_what_to_do(scene, from),
        HerdTrade::Stablemaster => stablemaster::decide_what_to_do(scene, from),
    }
}

/// `new AITarget(state, action, tickRate)` (`AITarget.java:24-53`), écrit une fois parce
/// que les quatre cibles de cette tranche ont exactement cette forme.
fn add_target(
    machine: &mut Machine<HerdState, HerdScene>,
    state: HerdState,
    rule: &'static str,
    java: &'static str,
    tick_rate: u32,
    action: fn(&mut HerdScene, HerdState) -> Option<HerdState>,
) {
    machine.add(Transition::target(
        state,
        rule,
        java,
        tick_rate,
        Action::Run(action),
    ));
}

/// Enregistre les cibles de `trade`.
///
/// Une cible par état que le métier ajoute à la table du parent, sa cadence citée, plus
/// — pour chaque état traversé — la règle de reprise qui ramène la machine en `DECIDE`
/// quand la table propriétaire vient d'y décider.
///
/// **Ce que l'enregistrement ne fait pas.** Il n'inscrit ni `IDLE`, ni
/// `START_WORKING`, ni `PREPARING`, ni les quatre états d'élevage comme des règles : ces
/// états appartiennent à `crate::ai` (`worker::register_base`) et à
/// `crate::ai_work_09::herder` (`herder::register`), qui en sont les seuls
/// propriétaires. La reprise n'est pas une règle métier, c'est le fil qui rend la main.
pub fn register(machine: &mut Machine<HerdState, HerdScene>, trade: HerdTrade) {
    // `AbstractEntityAIHerder.java:143` — `new AITarget(DECIDE, this::decideWhatToDo,
    // DECIDING_DELAY)`, dont le `this::decideWhatToDo` se résout dynamiquement sur la
    // sous-classe : c'est la surcharge qui est enregistrée ici.
    add_target(
        machine,
        HerdState::Base(WorkerState::Decide),
        "decideWhatToDo (surcharge)",
        trade.decide(),
        crate::ai_work_09::herder::DECIDING_DELAY,
        decide_action,
    );
    // Les cibles propres au métier, avec leur cadence.
    for (state, rule, java, tick_rate) in trade_targets(trade) {
        let action = match state {
            HerdState::ShepherdShear => shear_action,
            HerdState::HerderTrain => convert_action,
            HerdState::HerderReadyForCombat => ready_action,
            HerdState::HerderGatherMounts => gather_action,
            _ => continue,
        };
        add_target(machine, state, rule, java, tick_rate, action);
    }
    // La reprise : de tout état traversé, revenir en `DECIDE` dès que la table
    // propriétaire y a déposé une réponse.
    for state in TRAVERSED {
        if let Some((rule, java)) = resume_rule(state) {
            machine.add(Transition::target_when(
                state,
                rule,
                java,
                1,
                resumes_on_decide,
                Action::Fixed(Some(HerdState::Base(WorkerState::Decide))),
            ));
        }
    }
}

/// Les cibles propres d'un métier, avec la ligne de son `registerTargets`.
///
/// `Vec` et non `&'static [..]` : la cadence et la ligne changent d'une cible à l'autre
/// et les écrire dans un tableau de `HerdTrade` aurait demandé un quatrième champ par
/// état.
fn trade_targets(trade: HerdTrade) -> Vec<(HerdState, &'static str, &'static str, u32)> {
    match trade {
        // `EntityAIWorkRabbitHerder.java:32-35` : `super(job)` et rien d'autre.
        HerdTrade::RabbitHerder => Vec::new(),
        HerdTrade::Shepherd => vec![(
            HerdState::ShepherdShear,
            "shearSheep",
            "EntityAIWorkShepherd.java:50",
            crate::ai_work_09::TICKS_SECOND,
        )],
        HerdTrade::Stablemaster => vec![
            (
                HerdState::HerderTrain,
                "convertMount",
                "EntityAIWorkStablemaster.java:102",
                10,
            ),
            (
                HerdState::HerderReadyForCombat,
                "readyMountForCombat",
                "EntityAIWorkStablemaster.java:103",
                10,
            ),
            (
                HerdState::HerderGatherMounts,
                "gatherMounts",
                "EntityAIWorkStablemaster.java:104",
                10,
            ),
        ],
    }
}

/// Les états traversés que ces trois métiers peuvent atteindre, et pourquoi.
///
/// `DECIDE` n'y est pas : c'est l'état de la surcharge, il a sa propre règle. Aucun
/// autre état de `WorkerState` n'est atteignable depuis ces trois tables — mesuré :
/// `EntityAIWorkShepherd.java:122`, `:346` et `EntityAIWorkStablemaster.java:585`
/// rendent `PREPARING` ; `:276`, `:357`, `:595`, `:622`, `:627` rendent `getState()`
/// (l'état courant) ; `:294`, `:394`, `:421`, `:575`, `:617`, `:637` rendent `DECIDE` ;
/// `AbstractEntityAIHerder.java:483` rend `IDLE` ; `:248` rend `START_WORKING`.
const TRAVERSED: [HerdState; 7] = [
    // `AbstractEntityAIHerder.java:248`, et les cinq réponses de la surcharge.
    HerdState::Base(WorkerState::StartWorking),
    // `EntityAIWorkShepherd.java:122`, `EntityAIWorkStablemaster.java:585`.
    HerdState::Base(WorkerState::Preparing),
    // `AbstractEntityAIHerder.java:483` — `breedAnimals` échoue sur le second
    // accouplement et rend `IDLE`.
    HerdState::Base(WorkerState::Idle),
    // Les quatre réponses de la décision de base (`:232`, `:236`, `:240`, `:244`).
    HerdState::HerderPickup,
    HerdState::HerderButcher,
    HerdState::HerderBreed,
    HerdState::HerderFeed,
];

/// La règle de reprise d'un état traversé : son nom et la ligne qui justifie qu'on n'y
/// décide rien ici.
fn resume_rule(state: HerdState) -> Option<(&'static str, &'static str)> {
    match state {
        HerdState::Base(WorkerState::StartWorking) => {
            Some(("handStartWorking", "AbstractEntityAIHerder.java:141"))
        }
        HerdState::Base(WorkerState::Preparing) => {
            Some(("handPreparing", "AbstractEntityAIHerder.java:142"))
        }
        HerdState::Base(WorkerState::Idle) => {
            Some(("handIdle", "AbstractEntityAIHerder.java:140"))
        }
        HerdState::HerderPickup => Some(("handPickup", "AbstractEntityAIHerder.java:146")),
        HerdState::HerderButcher => Some(("handButcher", "AbstractEntityAIHerder.java:145")),
        HerdState::HerderBreed => Some(("handBreed", "AbstractEntityAIHerder.java:144")),
        HerdState::HerderFeed => Some(("handFeed", "AbstractEntityAIHerder.java:147")),
        _ => None,
    }
}

/// « La table propriétaire vient de décider » : une réponse est déposée.
fn resumes_on_decide(scene: &HerdScene, _state: HerdState) -> bool {
    scene.pen.base_answer.is_some()
}

/// L'adaptateur d'une règle vers la forme d'`Action::Run` : appliquer, noter le refus,
/// rendre l'état suivant.
fn record(scene: &mut HerdScene, decision: Decision) -> Option<HerdState> {
    if let Some(refusal) = decision.refusal {
        scene.refusal = Some(refusal);
    }
    decision.next
}

fn decide_action(scene: &mut HerdScene, from: HerdState) -> Option<HerdState> {
    let decision = decide_what_to_do(scene, from);
    record(scene, decision)
}

fn shear_action(scene: &mut HerdScene, from: HerdState) -> Option<HerdState> {
    let decision = shepherd::shear_sheep(scene, from);
    record(scene, decision)
}

fn convert_action(scene: &mut HerdScene, from: HerdState) -> Option<HerdState> {
    let decision = stablemaster::convert_mount(scene, from);
    record(scene, decision)
}

fn ready_action(scene: &mut HerdScene, from: HerdState) -> Option<HerdState> {
    let decision = stablemaster::ready_mount_for_combat(scene, from);
    record(scene, decision)
}

fn gather_action(scene: &mut HerdScene, from: HerdState) -> Option<HerdState> {
    let decision = stablemaster::gather_mounts(scene, from);
    record(scene, decision)
}
