//! Les buts d'IA du planteur, de l'éleveur de poulets et du cow-boy : la machine à
//! états de chacun, ce qu'elle demande au monde, et pourquoi elle s'arrête.
//!
//! Java référence trois classes de `core/entity/ai/workers/production/` :
//!
//! | Métier | Classe Java | Lignes | Ce qu'elle ajoute |
//! | --- | --- | ---: | --- |
//! | planteur | `agriculture/EntityAIWorkPlanter.java` | 490 | cinq états de champ et une surcharge de `decide` |
//! | éleveur de poulets | `herders/EntityAIWorkChickenHerder.java` | 74 | rien qu'une frappe au tirage et une icône |
//! | cow-boy | `herders/EntityAIWorkCowboy.java` | 270 | deux états de traite et une surcharge de `decideWhatToDo` |
//!
//! **L'éleveur de poulets ne porte presque rien, et c'est le fait intéressant.** Ses
//! soixante-quatorze lignes n'enregistrent aucune cible : tout son but d'IA **est** la
//! table d'`AbstractEntityAIHerder` (874 lignes), et sa seule règle propre est
//! `butcherAnimal` (`:60-73`), qui remplace un coup qui touche toujours par un coup
//! qui touche une fois sur dix. Le cow-boy, lui, étend la même classe
//! (`:39`), ajoute deux cibles (`:69-72`) et surcharge la décision (`:96-120`). C'est
//! pourquoi [`herder`] porte la table une fois et la donne aux deux, au lieu d'écrire
//! deux fichiers dont l'un n'aurait rien à dire.
//!
//! ## La machine, et ce qu'elle est
//!
//! Une table par métier, sur le moteur [`crate::ai::state::Machine`] : trois passes
//! globales, puis la liste de l'état courant, et le premier qui tire arrête le tick.
//! `S` vaut [`WorkState`], `F` vaut [`WorkScene`].
//!
//! ```ignore
//! let mut ai = WorkAi::new(WorkTrade::Cowboy);
//! let mut scene = ai.scene_with(WorldFacts::new(), WorkJob::Cowboy(facts));
//! let outcome = ai.tick(&mut scene);
//! match (outcome.goal(), outcome.why_not()) {
//!     (WorkState::CowboyMilk, _) => /* traire l'animal de milkable_animal */,
//!     (_, Some(WorkRefusal::NoMilkInput)) => /* il n'y a pas de seau, même dans la hutte */,
//!     _ => {}
//! }
//! ```
//!
//! ## Ce qui est réutilisé, et ce qui n'est pas réécrit
//!
//! - **Le moteur** : `crate::ai::state::Machine`, ses trois passes, son compte à
//!   rebours et sa règle de piège (`BasicStateMachine.java:220-228`). Ce module ajoute
//!   une table, pas une deuxième boucle de tick.
//! - **Les faits de base** : `crate::ai::facts::WorldFacts` — le citoyen, son
//!   inventaire, sa hutte et ses blocs. [`WorkScene`] enveloppe cette *même* valeur.
//! - **La mémoire de base** : `crate::ai::worker::Memory` — `delay`, `hasDelayed`,
//!   `needsCurrently`. Un métier qui pose un délai ou attend une pile réutilise les
//!   trois champs au lieu d'en inventer trois autres.
//! - **Les durées de travail** : `crate::ai_build::work_time` — la formule de minage
//!   que le planteur déclenche par `mineBlock` (`EntityAIWorkPlanter.java:371`). Voir
//!   [`planter::mining_ticks`].
//!
//! ## Ce qui n'est pas ici, et pourquoi
//!
//! - **Aucun appel au monde.** Pas de `walkToSafePos`, pas de `searchForAnimals`, pas
//!   de `setBlockAndUpdate`, pas de `mobInteract`. Chaque lecture est un fait
//!   ([`facts`]), chaque écriture une sortie de la mémoire ou du compteur d'actions.
//!   C'est ce qui rend les scénarios de [`tests`] rejouables.
//! - **Aucune API de mod.** Ce module n'importe pas `mod_api` : il ne dépend que de
//!   `crate::ai`, de `crate::ai_build` et de ses propres fichiers, le même choix que
//!   `crate::ai_gather` et `crate::ai_build`.
//! - **Les modules de champ et d'élevage.** `decideFieldWork`, `getNextWorkingPosition`,
//!   `getActionLimit` d'un côté ; `isCompatible`, `getBreedingItems`, `canTryToMilk` de
//!   l'autre. Le port lit leurs **réponses** — une [`facts::FieldWork`], une liste
//!   d'animaux compatibles, deux booléens — parce que rejouer leurs règles serait
//!   porter six modules de plus pour aucun état de plus.
//! - **Les requêtes.** `checkIfRequestForItemExistOrCreateAsync` apparaît cinq fois
//!   dans ces trois classes ; le port note *qu'il manque quelque chose* et le nomme,
//!   la requête elle-même est `crate::requests`.
//! - **Les autres éleveurs** — `EntityAIWorkShepherd`, `EntityAIWorkSwineHerder`,
//!   `EntityAIWorkRabbitHerder`, `EntityAIWorkStablemaster` — partagent la table
//!   d'`AbstractEntityAIHerder` mais ne sont pas dans cette tranche. Le jour où ils
//!   arrivent, ils se raccordent à [`herder`] et n'écrivent pas une cinquième copie.
//!
//! ## Deux prémisses du brief qui ne tiennent pas, et ce que le code fait à la place
//!
//! 1. **« chercher, aller, agir, rendre » n'est pas la forme de ces trois buts.** Le
//!    planteur a cinq états qui ne se réduisent pas à quatre (choisir, marcher, faire
//!    décider par le module, agir, rentrer), et l'éleveur en a quatre qui se
//!    chevauchent (accoupler, abattre, nourrir, ramasser) sans étape « rendre ». Les
//!    tables sont donc portées telles quelles — un état par cible Java — et non
//!    aplaties sur un schéma de quatre cases qui perdrait `PLANTATION_PICK_FIELD` et
//!    `PLANTATION_RETURN_TO_BUILDING`.
//! 2. **`EntityAIWorkPlanter` n'est pas un métier de champ simple : il hérite d'un
//!    artisan.** Sa classe mère est `AbstractEntityAICrafting`
//!    (`EntityAIWorkPlanter.java:46`), donc son `IDLE` passe par `hasWorkToDo`, son
//!    `START_WORKING` par `decide`, et `PREPARING` n'est atteint que par le `IDLE` que
//!    `getNextCraftingState` rend faute de tâche d'artisanat
//!    (`AbstractEntityAICrafting.java:300-302` puis `EntityAIWorkPlanter.java:420-423`).
//!    Le port enregistre donc ces deux cibles-là aussi, avec leur ligne, au lieu de
//!    commencer directement à `PREPARING`.
//!
//! `src/ai_work_09/MANQUANT.md` liste ce que l'API de mod ne permet pas d'écrire et
//! les constats mesurés sur le Java.

pub mod cowboy;
pub mod facts;
pub mod herder;
pub mod planter;
pub mod state;

#[cfg(test)]
mod tests;

use core::fmt;

use crate::ai::facts::WorldFacts;
use crate::ai::state::{
    Action, Fired, Machine, MissingTransition, Transition, WorkerState, ENTITY_AI_TICKRATE,
};

pub use facts::{
    CowboyFacts, CowboyMemory, Deliverable, FieldFacts, FieldWork, HerdAnimal, HerderDraws,
    HerdFacts, HerdMemory, PlanterFacts, PlanterMemory, PlantAction, ResetState, WorkJob,
    WorkMemory, WorkScene,
};
pub use state::{WorkRefusal, WorkState};

/// `Constants.TICKS_SECOND` (`api/util/constant/Constants.java:21`) : la cadence que
/// les deux constructeurs d'élevage passent à `registerTargets`
/// (`AbstractEntityAIHerder.java:141`, `:142`, `:146`, `:147`), et celle du parent
/// d'artisanat (`AbstractEntityAICrafting.java:164`).
pub const TICKS_SECOND: u32 = 20;

/// `AbstractEntityAIBasic.STANDARD_DELAY` (`AbstractEntityAIBasic.java:100`), tel que
/// `crate::ai` l'a déjà fixé : la cadence de `prepare`
/// (`EntityAIWorkPlanter.java:79`) et de `decide` (`EntityAIWorkPlanter.java:416-425`).
pub const STANDARD_DELAY: u32 = crate::ai::worker::STANDARD_DELAY;

/// Les trois métiers dont ce module porte le but d'IA.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: the three trades of this slice
pub enum WorkTrade {
    /// `EntityAIWorkPlanter` (`EntityAIWorkPlanter.java:46`).
    Planter,
    /// `EntityAIWorkChickenHerder` (`EntityAIWorkChickenHerder.java:22`).
    ChickenHerder,
    /// `EntityAIWorkCowboy` (`EntityAIWorkCowboy.java:39`).
    Cowboy,
}

impl WorkTrade {
    /// Les trois métiers, dans l'ordre où ce module les documente.
    pub const ALL: [WorkTrade; 3] = [
        WorkTrade::Planter,
        WorkTrade::ChickenHerder,
        WorkTrade::Cowboy,
    ];

    /// Le nom de la classe Java, pour une ligne de journal.
    pub fn class(self) -> &'static str {
        match self {
            WorkTrade::Planter => "EntityAIWorkPlanter",
            WorkTrade::ChickenHerder => "EntityAIWorkChickenHerder",
            WorkTrade::Cowboy => "EntityAIWorkCowboy",
        }
    }

    /// Le nom du métier, pour une clé de message.
    pub fn name(self) -> &'static str {
        match self {
            WorkTrade::Planter => "planter",
            WorkTrade::ChickenHerder => "chickenherder",
            WorkTrade::Cowboy => "cowboy",
        }
    }

    /// Le métier correspondant dans `crate::jobs`, le pont pour un appelant qui a déjà
    /// un `Job`.
    pub fn job(self) -> crate::jobs::job::Job {
        match self {
            WorkTrade::Planter => crate::jobs::job::Job::Planter,
            WorkTrade::ChickenHerder => crate::jobs::job::Job::ChickenHerder,
            WorkTrade::Cowboy => crate::jobs::job::Job::Cowboy,
        }
    }

    /// Les faits du métier, vides.
    pub fn job_facts(self) -> WorkJob {
        match self {
            WorkTrade::Planter => WorkJob::Planter(PlanterFacts::default()),
            WorkTrade::ChickenHerder => WorkJob::ChickenHerder(HerdFacts::default()),
            WorkTrade::Cowboy => WorkJob::Cowboy(CowboyFacts::default()),
        }
    }

    /// `getActionsDoneUntilDumping()` pour ce métier, que
    /// `AbstractEntityAIBasic.inventoryNeedsDump` lit (`:413`).
    ///
    /// Les trois métiers répondent un nombre, mais pas le même genre de nombre : le
    /// planteur dépend de l'état et du champ (`EntityAIWorkPlanter.java:445-459`), les
    /// deux éleveurs répondent dix (`AbstractEntityAIHerder.java:152-156`). L'état est
    /// passé en paramètre parce que c'est `getState()` que le Java lit — la scène ne le
    /// porte pas, la machine le tient.
    pub fn actions_until_dumping(self, scene: &WorkScene, state: WorkState) -> u32 {
        match self {
            // `EntityAIWorkPlanter.java:445-459` : la limite du module sur le champ,
            // 32 ailleurs.
            WorkTrade::Planter => match scene.planter() {
                Some(trade) => planter::actions_until_dumping(trade, state),
                // Un planteur qui lit les faits d'un autre métier n'a pas de champ :
                // la constante de base est la seule réponse honnête.
                None => planter::ACTIONS_UNTIL_DUMP,
            },
            // `AbstractEntityAIHerder.java:152-156`, que le cow-boy ne surcharge pas.
            WorkTrade::ChickenHerder => herder::actions_until_dumping(),
            WorkTrade::Cowboy => cowboy::actions_until_dumping(),
        }
    }

    /// Enregistre la table de ce métier — le corps des `registerTargets` de ses
    /// constructeurs.
    pub fn register(self, machine: &mut Machine<WorkState, WorkScene>) {
        register_handover(machine);
        match self {
            WorkTrade::Planter => planter::register(machine),
            // `AbstractEntityAIHerder.java:139-148` : les huit cibles du parent, dont
            // `DECIDE → this::decideWhatToDo`, qui se résout sur la sous-classe en Java.
            WorkTrade::ChickenHerder => herder::register(machine, herder::decide_what_to_do_action),
            // La même table, mais `DECIDE` porte la décision du cow-boy
            // (`EntityAIWorkCowboy.java:96-120`), puis les deux cibles de `:70-71`.
            WorkTrade::Cowboy => {
                herder::register(machine, cowboy::decide_action);
                cowboy::register(machine);
            }
        }
    }
}

impl fmt::Display for WorkTrade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Les états de base auxquels ces tables rendent la main, et par où elles reviennent.
///
/// **Ce n'est pas un second `register_base`.** `crate::ai::worker` porte déjà les
/// quatorze cibles de base sur `WorkerState`, et une table de métier ne peut pas les
/// contenir (type d'état différent — voir la note de `crate::ai_gather`). Ce qui est
/// enregistré ici n'est que le chemin du **retour** : un état de base qu'aucune action
/// de métier ne réclame rend la main à `IDLE`, avec la ligne de base qui le décide.
fn register_handover(machine: &mut Machine<WorkState, WorkScene>) {
    use WorkerState::*;
    // `dumpInventory` finit en `IDLE` (ou `PAUSED`) — `afterDump`
    // (`AbstractEntityAIBasic.java:1213-1222`). Le planteur et le cow-boy y arrivent
    // vraiment : `EntityAIWorkPlanter.java:187` n'est pas le seul, la traite rend
    // `INVENTORY_FULL` (`EntityAIWorkCowboy.java:187`, `:245`).
    machine.add(Transition::goto(
        WorkState::Base(InventoryFull),
        WorkState::Base(Idle),
        "afterDump",
        "AbstractEntityAIBasic.java:1213-1222",
        STANDARD_DELAY,
    ));
    // `waitForRequests` rend `IDLE` une fois la requête servie
    // (`AbstractEntityAIBasic.java:238`, `:605-640`).
    machine.add(Transition::goto(
        WorkState::Base(NeedsItem),
        WorkState::Base(Idle),
        "waitForRequests",
        "AbstractEntityAIBasic.java:238",
        STANDARD_DELAY,
    ));
    // `PAUSED` → `IDLE` quand le joueur relance (`AbstractEntityAIBasic.java:251`).
    machine.add(Transition::goto(
        WorkState::Base(Paused),
        WorkState::Base(Idle),
        "bePaused",
        "AbstractEntityAIBasic.java:251",
        TICKS_SECOND,
    ));
    // `GATHERING_REQUIRED_MATERIALS` → `IDLE` une fois la pile dans l'inventaire
    // (`AbstractEntityAIBasic.java:242`, `:323-326`). Le planteur y passe par
    // `returnToBuilding` (`EntityAIWorkPlanter.java:272`).
    machine.add(Transition::goto(
        WorkState::Base(GatheringRequiredMaterials),
        WorkState::Base(Idle),
        "getNeededItem",
        "AbstractEntityAIBasic.java:242",
        STANDARD_DELAY,
    ));
}

/// Ce qu'une action de métier a décidé, dans les deux moitiés que Java garde séparées :
/// l'état qu'elle rend (`return getState()` quand elle ne peut pas aller plus loin) et
/// la branche qui a refusé.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decision {
    /// L'état rendu, `None` pour « on reste dans l'état courant » — la même forme que
    /// le `Option` d'`Action::Run`.
    pub next: Option<WorkState>,
    /// Pourquoi il s'est arrêté, quand il s'est arrêté.
    pub refusal: Option<WorkRefusal>,
}

impl Decision {
    /// Aller en `state`.
    pub fn go(state: WorkState) -> Decision {
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
    pub fn refuse(refusal: WorkRefusal) -> Decision {
        Decision {
            next: None,
            refusal: Some(refusal),
        }
    }

    /// Aller en `state` **et** noter pourquoi.
    pub fn go_after(state: WorkState, refusal: WorkRefusal) -> Decision {
        Decision {
            next: Some(state),
            refusal: Some(refusal),
        }
    }

    /// Ajouter une raison à une décision qui n'en a pas.
    pub fn because(mut self, refusal: WorkRefusal) -> Decision {
        if self.refusal.is_none() {
            self.refusal = Some(refusal);
        }
        self
    }
}

/// Ce qu'un tick de ces trois buts a décidé : le but en vigueur, la règle qui l'a
/// choisi, et le refus s'il n'est pas allé plus loin — les trois mêmes réponses
/// qu'`crate::ai::worker::Outcome`, pour un [`WorkState`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkOutcome {
    /// Le but en vigueur après ce tick.
    pub state: WorkState,
    /// La transition qui a tiré, si une a tiré.
    pub fired: Option<Fired<WorkState>>,
    /// Le refus noté pendant ce tick, s'il y en a un.
    pub refusal: Option<WorkRefusal>,
}

impl WorkOutcome {
    /// Le but en vigueur.
    pub fn goal(&self) -> WorkState {
        self.state
    }

    /// Pourquoi rien n'a bougé, si le tick a noté une raison.
    pub fn why_not(&self) -> Option<WorkRefusal> {
        self.refusal
    }

    /// Ce tick a-t-il changé de but ?
    pub fn moved(&self) -> bool {
        matches!(self.fired, Some(fired) if fired.from != fired.to)
    }

    /// La machine est-elle tombée dans un piège (`BasicStateMachine.java:220-228`) ?
    pub fn faulted(&self) -> bool {
        matches!(self.fired, Some(fired) if fired.reset)
    }

    /// La ligne Java de la règle qui a tiré, pour une ligne de journal.
    pub fn why(&self) -> Option<&'static str> {
        self.fired.map(|fired| fired.java)
    }
}

/// Une de ces trois IA : un métier et sa machine, rien de plus.
pub struct WorkAi {
    trade: WorkTrade,
    machine: Machine<WorkState, WorkScene>,
}

impl fmt::Debug for WorkAi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorkAi")
            .field("trade", &self.trade)
            .field("machine", &self.machine)
            .finish()
    }
}

impl WorkAi {
    /// Le but d'IA du métier, à partir de `IDLE` et à `ENTITY_AI_TICKRATE`
    /// (`AbstractEntityCitizen.java:64`).
    ///
    /// Java commence en `INIT`, et `initSafetyChecks` (`AbstractEntityAIBasic.java:
    /// 446-465`) l'envoie en `IDLE` ; `INIT` et ces vérifications appartiennent à
    /// `crate::ai`, donc cette machine commence là où l'autre arrive.
    pub fn new(trade: WorkTrade) -> WorkAi {
        let mut machine = Machine::new(WorkState::Base(WorkerState::Idle), ENTITY_AI_TICKRATE);
        trade.register(&mut machine);
        WorkAi { trade, machine }
    }

    /// Une scène sur `world` portant les faits vides de ce métier.
    pub fn scene(&self, world: WorldFacts) -> WorkScene {
        WorkScene::new(world, self.trade.job_facts())
    }

    /// Une scène sur `world` et `job`. Un planteur à qui on donne les faits d'un
    /// cow-boy répond [`WorkRefusal::NoFactsForTrade`] à son premier tick au lieu de
    /// faire semblant de n'avoir pas de champ.
    pub fn scene_with(&self, world: WorldFacts, job: WorkJob) -> WorkScene {
        WorkScene::new(world, job)
    }

    /// Le métier de cette IA.
    pub fn trade(&self) -> WorkTrade {
        self.trade
    }

    /// L'état de la machine (`AbstractAISkeleton.getState`, `:120-126`).
    pub fn state(&self) -> WorkState {
        self.machine.state()
    }

    /// Le dernier piège, si un tick en a déclenché un (`BasicStateMachine.java:
    /// 220-228`).
    pub fn missing(&self) -> Option<MissingTransition<WorkState>> {
        self.machine.missing()
    }

    /// `AbstractAISkeleton.resetAI` (`:145-152`).
    pub fn reset(&mut self) {
        self.machine.reset();
    }

    /// La machine elle-même, pour un appelant qui veut lire la table.
    pub fn machine(&self) -> &Machine<WorkState, WorkScene> {
        &self.machine
    }

    /// Un tick d'IA (`AbstractAISkeleton.tick`, `:63-66`).
    pub fn tick(&mut self, scene: &mut WorkScene) -> WorkOutcome {
        scene.refusal = None;
        let fired = self.machine.tick(scene);
        WorkOutcome {
            state: self.machine.state(),
            fired,
            refusal: scene.refusal,
        }
    }
}

/// `walkToBuilding()` (`AbstractEntityAIBasic.java:757-766`) : le citoyen est à moins
/// de `WOKR_IN_BUILDING_DIST` de sa hutte, le même test que `crate::ai::worker`.
pub fn at_hut(facts: &WorldFacts) -> bool {
    facts.worker.at.is_living_at_site(
        facts.building.position,
        crate::ai::worker::WOKR_IN_BUILDING_DIST,
    )
}

/// `inventoryNeedsDump` (`AbstractEntityAIBasic.java:409-416`) pour une de ces scènes.
///
/// **Reformulée, parce qu'elle n'est pas atteignable d'ici.** La règle est privée dans
/// `crate::ai::worker` (`inventory_needs_dump`). Les trois conditions sont les mêmes,
/// à ceci près que le planteur remplace la troisième par un seuil qui dépend de son
/// état — c'est [`WorkTrade::actions_until_dumping`], et c'est pour cela que la
/// fonction prend le métier **et** l'état courant.
pub fn dump_due(trade: WorkTrade, scene: &WorkScene, state: WorkState) -> bool {
    // :411 — `getState() != INVENTORY_FULL && canBeInterrupted()`.
    let interruptible = state.okay_to_eat() && state != WorkState::Base(crate::ai::state::WorkerState::InventoryFull);
    interruptible
        // :412
        && (scene.facts.inventory.full
            // :413
            || scene.facts.inventory.actions_done >= trade.actions_until_dumping(scene, state))
    // :415 — `!(job instanceof JobDeliveryman)`.
    && !scene.facts.worker.deliveryman
}

/// `new AITarget(state, action, tickRate)` — la forme qu'utilisent les trois
/// constructeurs (`AITarget.java:24-53`), écrite une fois parce que les trois tables
/// ajoutent le même genre de transition et qu'une fermeture tiendrait `machine`
/// emprunté pendant qu'on ajoute un `goto` à côté.
pub(crate) fn add_target(
    machine: &mut Machine<WorkState, WorkScene>,
    state: WorkState,
    rule: &'static str,
    java: &'static str,
    tick_rate: u32,
    action: fn(&mut WorkScene, WorkState) -> Option<WorkState>,
) {
    machine.add(Transition::target(
        state,
        rule,
        java,
        tick_rate,
        Action::Run(action),
    ));
}

/// La forme d'`Action::Run` que toutes les actions de ces trois buts ont : découper la
/// scène en ses quatre champs, faire la décision sur les faits du métier, noter le
/// refus.
///
/// Écrite en macro parce que l'emprunt est le point délicat — les faits du métier et la
/// mémoire sont deux champs d'une même structure, et seule une déstructuration permet
/// à la décision de lire le premier en écrivant la seconde.
///
/// **Chaque nom est qualifié `$crate` à dessein** : `macro_rules!` résout les chemins
/// au site d'expansion, donc un `WorkScene` nu ne compilerait que dans un module qui
/// l'importe par hasard. Un corps qualifié n'a besoin de rien de son appelant.
macro_rules! work_action {
    ($name:ident, $variant:ident, $decide:path) => {
        pub(crate) fn $name(
            scene: &mut $crate::ai_work_09::WorkScene,
            state: $crate::ai_work_09::WorkState,
        ) -> Option<$crate::ai_work_09::WorkState> {
            let $crate::ai_work_09::WorkScene {
                facts,
                job,
                memory,
                refusal,
            } = scene;
            let $crate::ai_work_09::WorkJob::$variant(trade_facts) = job else {
                *refusal = Some($crate::ai_work_09::WorkRefusal::NoFactsForTrade);
                return Some(state);
            };
            let decision: $crate::ai_work_09::Decision = $decide(facts, trade_facts, memory, state);
            *refusal = decision.refusal;
            decision.next.or(Some(state))
        }
    };
}

pub(crate) use work_action;

/// La même forme que [`work_action`], pour les actions de la table **d'élevage**.
///
/// Elles sont partagées par les deux éleveurs : `EntityAIWorkCowboy` étend
/// `AbstractEntityAIHerder` (`EntityAIWorkCowboy.java:39`), donc Java n'a qu'un jeu de
/// méthodes et deux sous-classes. Une action d'élevage accepte donc les deux variantes
/// de faits ([`WorkJob::herd`]) ; ce qui refuse l'autre métier, c'est la décision qui
/// lui est propre — `decideWhatToDo` du cow-boy sur `DECIDE`, et les deux traites.
macro_rules! herd_action {
    ($name:ident, $decide:path) => {
        pub(crate) fn $name(
            scene: &mut $crate::ai_work_09::WorkScene,
            state: $crate::ai_work_09::WorkState,
        ) -> Option<$crate::ai_work_09::WorkState> {
            let $crate::ai_work_09::WorkScene {
                facts,
                job,
                memory,
                refusal,
            } = scene;
            let Some(trade_facts) = job.herd() else {
                *refusal = Some($crate::ai_work_09::WorkRefusal::NoFactsForTrade);
                return Some(state);
            };
            let decision: $crate::ai_work_09::Decision = $decide(facts, trade_facts, memory, state);
            *refusal = decision.refusal;
            decision.next.or(Some(state))
        }
    };
}

pub(crate) use herd_action;
