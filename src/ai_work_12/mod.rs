//! Les buts d'IA de l'enchanteur, du guérisseur et du croque-mort : la machine à états
//! de chacun, ce qu'elle demande au monde, pourquoi elle s'arrête, et combien de temps
//! chaque geste prend.
//!
//! Référence Java : `core/entity/ai/workers/service/EntityAIWorkEnchanter.java` (498
//! lignes), `.../EntityAIWorkHealer.java` (552) et `.../EntityAIWorkUndertaker.java`
//! (553), plus les classes qu'ils lisent — `colony/jobs/JobEnchanter.java`,
//! `colony/buildings/modules/EnchanterStationsModule.java`,
//! `colony/buildings/modules/GraveyardManagementModule.java`,
//! `core/entity/ai/workers/util/Patient.java` et
//! `api/util/constant/UndertakerConstants.java`.
//!
//! ## Ce que ce module ajoute
//!
//! | Fichier | Question | Java |
//! | --- | --- | --- |
//! | [`state`] | les onze états que ces métiers ajoutent, et les quarante-cinq refus qu'ils nomment | `AIWorkerState.java:169-617` |
//! | [`facts`] | ce qu'une décision peut lire, en observations | les trois AI, `Patient.java`, `Disease.java` |
//! | [`enchanter`] | une journée d'atelier, une nuit de drain, l'enchantement des tomes | `EntityAIWorkEnchanter.java:120-433` |
//! | [`healer`] | trois façons de soigner un patient à la fois | `EntityAIWorkHealer.java:104-516` |
//! | [`undertaker`] | vider, creuser, enterrer, ressusciter | `EntityAIWorkUndertaker.java:114-499` |
//!
//! **La décision, en une phrase.** Étant donné un [`ServiceScene`] — le même
//! [`WorldFacts`] que prend `crate::ai`, les faits du métier et la mémoire de l'IA — un
//! [`ServiceAi`] fait un tick et rend un [`ServiceOutcome`] : l'état maintenant en
//! vigueur, la règle qui l'a choisi avec son `fichier.java:ligne`, le refus quand il n'a
//! pas pu aller plus loin, et les [`Effect`]s que l'appelant doit exécuter. Rien ici ne
//! déplace un citoyen, ne casse une tombe, ne transfère un remède ni ne ressuscite
//! personne.
//!
//! ```ignore
//! let mut ai = ServiceAi::new(ServiceTrade::Undertaker);
//! let mut scene = ai.scene(WorldFacts::new());
//! scene.job = ServiceJob::Undertaker(UndertakerFacts::default());
//! match (ai.tick(&mut scene).goal(), /* … */) {
//!     (ServiceState::EmptyGrave, _) => /* walk to the grave and empty it */,
//!     (state, _) => { /* the goal in force */ }
//! }
//! ```
//!
//! ## Le moteur est celui de `crate::ai`, inchangé
//!
//! [`ServiceState`] est une énumération unique pour les trois métiers, comme
//! `AIWorkerState` en est une pour les 127 états du mod ; l'ordre des passes, le
//! compte-à-rebours de cadence et la règle de piège sont
//! [`crate::ai::state::Machine`], qui a déjà ses tests. Ce module ajoute des tables, pas
//! une seconde boucle de tick. [`register_handover`] est la couture vers la table de
//! base : une transition par état de base que ces métiers traversent, chacune citant la
//! ligne de base qui la possède — et **ce n'est pas** un second `register_base`
//! (`crate::ai::worker` porte les quatorze cibles de `AbstractEntityAIBasic.java:204-259`).
//!
//! Deux conséquences, écrites dans `MANQUANT.md` §1 :
//!
//! 1. `Machine<S, F>` est monomorphe en `S` et `F`, donc une machine de métier ne peut
//!    pas *contenir* la table de base. Un appelant fait tourner l'une, puis l'autre ; les
//!    états de base ([`ServiceState::Base`]) sont la couture.
//! 2. `waitingForSomething` (`AbstractEntityAIBasic.java:496-517`), qui dépense
//!    `memory.base.delay`, appartient à la table de base : une machine de métier
//!    **enregistre** le délai et ne le dépense pas. Un appelant qui veut que la seconde
//!    passe fait aussi tourner `crate::ai::worker::JobAi`.
//!
//! ## Pas de monde, pas d'horloge, pas de hasard
//!
//! Chaque lecture est un champ de [`ServiceScene`] ; chaque tirage que Java fait avec
//! `worker.getRandom()` — la station à drainer (`EnchanterStationsModule.java:112`), le
//! citoyen à drainer (`EntityAIWorkEnchanter.java:337`), la guérison gratuite
//! (`EntityAIWorkHealer.java:515`), la résurrection et le totem
//! (`EntityAIWorkUndertaker.java:351`, `:357`) — est un fait que l'appelant fournit :
//! une décision qui dépend d'un tirage ne se rejoue pas, donc ne se teste pas.
//! **Aucun appel au SDK** : rien ici n'importe `mod_api`.

pub mod enchanter;
pub mod facts;
pub mod healer;
pub mod state;
pub mod undertaker;

#[cfg(test)]
mod tests;

use core::fmt;

use crate::ai::facts::{Pos, WorldFacts};
use crate::ai::state::{Action, Fired, Machine, Transition, WorkerState, ENTITY_AI_TICKRATE};
use crate::ai::worker::{STANDARD_DELAY as BASE_STANDARD_DELAY, WOKR_IN_BUILDING_DIST};
use crate::ai_build::crafter::CraftState;
use crate::jobs::job::Job;

pub use facts::{
    count_of, holds, CureStock, DrainWorker, EnchanterFacts, EnchanterMemory, HealerFacts,
    HealerMemory, InjuredPlayer, Patient, PatientState, RemotePatient, ServiceJob, ServiceMemory,
    ServiceScene, UndertakerFacts, UndertakerMemory,
};
pub use state::{ServiceRefusal, ServiceState};

/// `Constants.TICKS_SECOND` (`api/util/constant/Constants.java:21`) : la cadence que les
/// trois constructeurs passent à `registerTargets`, et l'unité du repos de l'enchanteur.
pub const TICKS_SECOND: u32 = 20;

/// `AbstractEntityAIBasic.STANDARD_DELAY` (`:100`), tel que `crate::ai::worker` le fixe.
pub const STANDARD_DELAY: u32 = BASE_STANDARD_DELAY;

/// Les trois métiers de cette tranche.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustif : les trois métiers de `core/entity/ai/workers/service/`
pub enum ServiceTrade {
    /// `EntityAIWorkEnchanter` (`EntityAIWorkEnchanter.java:53`).
    Enchanter,
    /// `EntityAIWorkHealer` (`EntityAIWorkHealer.java:38`).
    Healer,
    /// `EntityAIWorkUndertaker` (`EntityAIWorkUndertaker.java:54`).
    Undertaker,
}

impl ServiceTrade {
    /// Les trois, dans l'ordre où ce module les documente.
    pub const ALL: [ServiceTrade; 3] =
        [ServiceTrade::Enchanter, ServiceTrade::Healer, ServiceTrade::Undertaker];

    /// Le nom de la classe Java, pour une ligne de log.
    pub fn class(self) -> &'static str {
        match self {
            ServiceTrade::Enchanter => "EntityAIWorkEnchanter",
            ServiceTrade::Healer => "EntityAIWorkHealer",
            ServiceTrade::Undertaker => "EntityAIWorkUndertaker",
        }
    }

    /// Le nom du métier, pour une clé de message.
    pub fn name(self) -> &'static str {
        match self {
            ServiceTrade::Enchanter => "enchanter",
            ServiceTrade::Healer => "healer",
            ServiceTrade::Undertaker => "undertaker",
        }
    }

    /// Le métier dans `crate::jobs` (`jobs/job.rs:101`, `:107`, `:131`).
    pub fn job(self) -> Job {
        match self {
            ServiceTrade::Enchanter => Job::Enchanter,
            ServiceTrade::Healer => Job::Healer,
            ServiceTrade::Undertaker => Job::Undertaker,
        }
    }

    /// Le premier état que la table du métier revendique — où `IDLE` passe la main.
    ///
    /// Les trois passent par `START_WORKING` : l'enchanteur y décide (`:166`), le
    /// guérisseur y passe pour aller vers `DECIDE` (`:87`) et le croque-mort y travaille
    /// (`:91`).
    pub fn entry(self) -> ServiceState {
        ServiceState::Base(WorkerState::StartWorking)
    }

    /// La ligne qui enregistre `IDLE → START_WORKING` pour ce métier.
    pub fn entry_java(self) -> &'static str {
        match self {
            ServiceTrade::Enchanter => "AbstractEntityAICrafting.java:164",
            ServiceTrade::Healer => "EntityAIWorkHealer.java:86",
            ServiceTrade::Undertaker => "EntityAIWorkUndertaker.java:90",
        }
    }

    /// La cadence de `IDLE → START_WORKING` : 20 ticks pour l'enchanteur
    /// (`AbstractEntityAICrafting.java:164`), **1** pour le guérisseur
    /// (`EntityAIWorkHealer.java:86), **60** pour le croque-mort
    /// (`EntityAIWorkUndertaker.java:90`).
    pub fn idle_rate(self) -> u32 {
        match self {
            ServiceTrade::Enchanter => TICKS_SECOND,
            ServiceTrade::Healer => 1,
            ServiceTrade::Undertaker => crate::ai::worker::REQUEST_DELAY,
        }
    }

    /// `getStateAfterPickUp()` après une cueillette : le guérisseur va **soigner**
    /// (`EntityAIWorkHealer.java:470-473`), les deux autres reprennent le travail
    /// (`AbstractEntityAIBasic.java:323-326`).
    pub fn state_after_pickup(self) -> ServiceState {
        match self {
            ServiceTrade::Healer => healer::state_after_pickup(),
            _ => ServiceState::Base(WorkerState::StartWorking),
        }
    }

    /// La ligne de cette réponse.
    pub fn state_after_pickup_java(self) -> &'static str {
        match self {
            ServiceTrade::Healer => "EntityAIWorkHealer.java:470-473",
            _ => "AbstractEntityAIBasic.java:323-326",
        }
    }

    /// `getActionsDoneUntilDumping()` : 1 pour l'enchanteur
    /// (`EntityAIWorkEnchanter.java:214-218`), le seuil de base de 32 pour les deux
    /// autres (`AbstractEntityAIBasic.java:425-428`).
    pub fn actions_until_dumping(self) -> u32 {
        match self {
            ServiceTrade::Enchanter => enchanter::ACTIONS_UNTIL_DUMPING,
            ServiceTrade::Healer | ServiceTrade::Undertaker => crate::ai::worker::ACTIONS_UNTIL_DUMP,
        }
    }

    /// Les faits du métier, vides.
    pub fn job_facts(self) -> ServiceJob {
        match self {
            ServiceTrade::Enchanter => ServiceJob::Enchanter(EnchanterFacts::default()),
            ServiceTrade::Healer => ServiceJob::Healer(HealerFacts::default()),
            ServiceTrade::Undertaker => ServiceJob::Undertaker(UndertakerFacts::default()),
        }
    }

    /// Enregistre les cibles du métier — le corps du `super.registerTargets(...)` de son
    /// constructeur.
    pub fn register(self, machine: &mut Machine<ServiceState, ServiceScene>) {
        register_handover(machine, self);
        match self {
            ServiceTrade::Enchanter => enchanter::register(machine),
            ServiceTrade::Healer => healer::register(machine),
            ServiceTrade::Undertaker => undertaker::register(machine),
        }
    }
}

impl fmt::Display for ServiceTrade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Les états que la machine de `trade` peut prendre : ses états de métier, plus les états
/// de base que **son** relais traverse.
///
/// `DECIDE` n'est là que pour le guérisseur : c'est le seul des trois dont le constructeur
/// enregistre `START_WORKING → DECIDE` (`EntityAIWorkHealer.java:87`). L'enchanteur décide
/// **dans** `START_WORKING` (`AbstractEntityAICrafting.java:166`) et le croque-mort aussi
/// (`EntityAIWorkUndertaker.java:91`), donc aucun des deux ne peut se trouver dans
/// `DECIDE`.
pub fn states_of(trade: ServiceTrade) -> Vec<ServiceState> {
    let mut states: Vec<ServiceState> = ServiceState::ALL
        .iter()
        .copied()
        .filter(|state| state.belongs_to(trade))
        .collect();
    for base in [
        WorkerState::Idle,
        WorkerState::StartWorking,
        WorkerState::InventoryFull,
        WorkerState::NeedsItem,
        WorkerState::Paused,
        WorkerState::GatheringRequiredMaterials,
    ] {
        states.push(ServiceState::Base(base));
    }
    // `WANDER` (`AIWorkerState.java:613`) est enregistré par le guérisseur (`:93`) et par
    // le croque-mort (`:92`) : il n'appartient à aucun des deux, donc
    // `belongs_to` ne le rend pas et il s'ajoute ici.
    if matches!(trade, ServiceTrade::Healer | ServiceTrade::Undertaker) {
        states.push(ServiceState::Wander);
    }
    if trade == ServiceTrade::Healer {
        states.push(ServiceState::Base(WorkerState::Decide));
    }
    states
}

/// La couture entre la table d'un métier et la table de base : une transition par état de
/// base que ces métiers traversent, chacune citant la ligne de base qui la possède.
///
/// **Ce n'est pas un second `register_base`.** `crate::ai::worker` porte déjà les
/// quatorze cibles de `AbstractEntityAIBasic.java:204-259` sur `WorkerState`, et une
/// machine de métier ne peut pas les contenir (type d'état différent). Ce qui est
/// enregistré ici est seulement le chemin du **retour**.
pub fn register_handover(machine: &mut Machine<ServiceState, ServiceScene>, trade: ServiceTrade) {
    use WorkerState::*;
    // `dumpInventory` finit en `IDLE` ou `PAUSED` — `afterDump`
    // (`AbstractEntityAIBasic.java:1213-1222`). Aucun des trois métiers ne l'écrase.
    machine.add(Transition::goto(
        ServiceState::Base(InventoryFull),
        ServiceState::Base(Idle),
        "afterDump",
        "AbstractEntityAIBasic.java:1213-1222",
        STANDARD_DELAY,
    ));
    // `waitForRequests` rend `afterRequestPickUp()`, `IDLE` dans la classe de base
    // (`AbstractEntityAIBasic.java:238`, `:736-739`).
    machine.add(Transition::goto(
        ServiceState::Base(NeedsItem),
        ServiceState::Base(Idle),
        "afterRequestPickUp",
        "AbstractEntityAIBasic.java:736-739",
        STANDARD_DELAY,
    ));
    // Un joueur qui remet le citoyen au travail (`AbstractEntityAIBasic.java:251`).
    machine.add(Transition::goto(
        ServiceState::Base(Paused),
        ServiceState::Base(Idle),
        "unpause",
        "AbstractEntityAIBasic.java:251",
        TICKS_SECOND,
    ));
    // `getNeededItem` finit dans `getStateAfterPickUp()` — `START_WORKING` chez deux de
    // ces métiers, `CURE` chez le guérisseur (`AbstractEntityAIBasic.java:242`).
    machine.add(Transition::goto(
        ServiceState::Base(GatheringRequiredMaterials),
        trade.state_after_pickup(),
        "getStateAfterPickUp",
        trade.state_after_pickup_java(),
        STANDARD_DELAY,
    ));
    // `IDLE` sort vers l'entrée du métier : chaque IA concrète l'enregistre, ce qui est
    // la raison pour laquelle la classe de base est `abstract`
    // (`AbstractEntityAIBasic.java:204-259`).
    machine.add(Transition::goto(
        ServiceState::Base(Idle),
        trade.entry(),
        "idleToEntry",
        trade.entry_java(),
        trade.idle_rate(),
    ));
}

/// Ce qu'un tick demande à l'appelant — un ordre, ni un fait ni une mémoire.
///
/// Java exécute chacun de ces gestes sur place, avec le citoyen, le niveau et la colonie
/// en main : il crée une requête (`EntityAIWorkEnchanter.java:164`), il retire de la mana
/// (`:279`), il transfère un remède (`EntityAIWorkHealer.java:369-373`), il casse une
/// tombe (`EntityAIWorkUndertaker.java:292`). Aucun de ces gestes n'est une décision,
/// donc chacun sort de la machine comme un [`Effect`] que l'appelant exécute ou
/// abandonne.
#[derive(Clone, Debug, PartialEq)]
// exhaustif : un variant par geste Java que la décision ordonne
pub enum Effect {
    /// `StandardInteraction(...)` (`EntityAIWorkEnchanter.java:150`,
    /// `EntityAIWorkHealer.java:197`, `:365`) : le message au joueur.
    Say {
        /// La clé de traduction.
        key: &'static str,
    },
    /// `incrementActionsDoneAndDecSaturation` (`EntityAIWorkEnchanter.java:281`, `:445`) :
    /// une action comptée, donc un vidage d'inventaire à venir.
    ActionsDone {
        /// Le nombre d'actions (`getActionRewardForCraftingSuccess` = 1).
        count: u32,
    },
    // ---------------------------------------------------------------- enchanteur
    /// `worker.setItemInHand(MAIN_HAND, ItemStack.EMPTY)` (`:123`).
    EmptyMainHand,
    /// La main passe à la table d'atelier : `QUERY_ITEMS`, `GET_RECIPE` ou `CRAFT`
    /// (`AbstractEntityAICrafting.java:300-317`). L'état vient de
    /// `crate::ai_build::crafter`, qui le possède.
    Crafted {
        /// L'état que la table d'atelier doit prendre (`:132`).
        state: CraftState,
    },
    /// `checkIfRequestForItemExistOrCreateAsync(...)` (`:164`, `:199`) : une requête
    /// asynchrone.
    RequestItem {
        /// L'identifiant d'objet demandé.
        item: &'static str,
        /// La taille de la requête (`:164` demande 1).
        count: u32,
    },
    /// `needsCurrently = new Tuple<>(predicate, count)` (`:161`, `:196`) : la cueillette
    /// de la classe de base.
    FetchItem {
        /// L'identifiant d'objet à prendre.
        item: &'static str,
        /// Combien en prendre.
        count: u32,
    },
    /// `job.setBuildingToDrainFrom(posToDrainFrom)` (`:173`).
    SetDrainTarget {
        /// La station tirée.
        at: Pos,
    },
    /// `setAsGathered(pos)` (`EnchanterStationsModule.java:120-123`, appelé à `:441`).
    MarkStationGathered {
        /// La station drainée.
        at: Pos,
    },
    /// `removeWorker(pos)` (`EnchanterStationsModule.java:141-145`, appelé à `:322`).
    DropStation {
        /// La station qui n'existe plus.
        at: Pos,
    },
    /// `fullfillRecipeAndCopy(...)` puis `incrementLevel(Skill.Mana, -level)`
    /// (`:271-281`).
    Enchanted {
        /// Le niveau d'enchantement du butin, retiré de la mana.
        mana_spent: i32,
    },
    /// `:408-430` : un livre consommé, un niveau de mana et dix points d'expérience.
    Drained {
        /// `XP_PER_DRAIN` (`:88`), zéro sans livre.
        xp: f64,
        /// `incrementLevel(Skill.Mana, 1)` (`:426`), zéro sans livre.
        mana_gain: i32,
        /// Le livre a-t-il été dépensé.
        book_spent: bool,
    },
    // ----------------------------------------------------------------- guérisseur
    /// `hospital.checkOrCreatePatientFile(id)` (`:115`).
    CreatePatientFile {
        /// L'identifiant du citoyen malade.
        citizen: u32,
    },
    /// `hospital.removePatientFile(patient)` (`:123`).
    DropPatientFile {
        /// L'identifiant du dossier retiré.
        citizen: u32,
    },
    /// `patient.setState(...)` (`Patient.java:79-82`).
    SetPatientState {
        /// Le citoyen du dossier.
        patient: u32,
        /// Le nouvel état.
        state: PatientState,
    },
    /// `createRequestAsync(new Stack(cure, REQUEST_COUNT, 1))` (`:296`).
    RequestCure {
        /// L'identifiant du remède.
        item: &'static str,
        /// `REQUEST_COUNT` = 16 (`:48`).
        count: u32,
    },
    /// `transferXOfFirstSlotInItemHandlerWithIntoNextFreeSlotInItemHandler(...)`
    /// (`:369-373`) : un remède du guérisseur vers le patient.
    GiveCure {
        /// Le citoyen qui reçoit.
        patient: u32,
        /// L'identifiant du remède.
        item: &'static str,
        /// Le compte transféré.
        count: u32,
    },
    /// `citizen.getCitizenData().getCitizenDiseaseHandler().cure()` (`:434`) : la
    /// guérison magique.
    CureDisease {
        /// Le citoyen guéri.
        patient: u32,
    },
    /// `recordTreatmentStats(citizen)` puis `addExperience(BASE_XP_GAIN)` (`:378-379`).
    Treated {
        /// Le citoyen soigné.
        patient: u32,
        /// `BASE_XP_GAIN` = 2 (`:43`).
        xp: f64,
    },
    /// `playerToHeal.heal(max - health - 5 - niveau)` (`:463`).
    HealedPlayer {
        /// L'identifiant du joueur.
        player: u32,
        /// Les points rendus.
        amount: f64,
    },
    /// `citizen.heal(max - health - 5 - niveau)` (`:499`).
    HealedRemote {
        /// L'identifiant du citoyen.
        citizen: u32,
        /// Les points rendus.
        amount: f64,
    },
    // ---------------------------------------------------------------- croque-mort
    /// `building.ClearCurrentGrave()` (`EntityAIWorkUndertaker.java:141`, `:295`).
    ClearCurrentGrave,
    /// `setLastGraveData(graveData)` (`:296`) : la tombe creusée, à inhumer.
    KeepGraveData,
    /// `setLastGraveData(null)` (`:372`, `:464`, `:494`).
    ForgetGraveData,
    /// `transferAllItemHandler(grave.getInventory(), worker.getInventoryCitizen())`
    /// (`:219`).
    EmptiedGrave {
        /// La tombe vidée.
        at: Pos,
    },
    /// `addXpToSkill(getPrimarySkill(), XP_PER_DIG, ...)` (`:270`) : la Force, pas
    /// l'expérience de métier.
    Dug {
        /// La tombe cassée.
        at: Pos,
        /// `XP_PER_DIG` = 7,5.
        xp: f64,
    },
    /// `resurrectCivilianData(...)` (`:363-373`) : le citoyen revient.
    Resurrected {
        /// La tombe qui disparaît.
        at: Pos,
    },
    /// `module.buryCitizenHere(burialPos, worker)` (`:485`).
    Buried {
        /// La place de la tombe posée.
        at: Pos,
    },
    /// `extractItem(totemSlot, 1, false)` puis `playSound(TOTEM_USE, ...)` (`:353-354`).
    TotemBurned,
    /// `shouldDumpInventory = true` (`:348`, `:496`), que `wantInventoryDumped`
    /// consommera (`:507-515`).
    DumpInventory,
    /// `walkToRandomPosWithin(worker, 10, ...)` (`:159`) : dix blocs de flânerie.
    Wander {
        /// `WANDER_RANGE` = 10 (`:159`).
        range: u32,
    },
    /// `walkToBuilding()` (`:163`).
    WalkToHut,
}

/// Ce qu'une passe décide, dans les trois moitiés que Java garde séparées : l'état qu'elle
/// rend (`return getState()` quand elle ne peut pas aller plus loin), la branche qui a
/// refusé, et les ordres qu'elle donne.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Decision {
    /// L'état que la passe rend, `None` pour « rester dans l'état courant » — la forme de
    /// l'`Option` d'`Action::Run`.
    pub next: Option<ServiceState>,
    /// Pourquoi elle s'est arrêtée, ou quel côté de la passe est dégradé.
    pub refusal: Option<ServiceRefusal>,
    /// Ce que l'appelant doit faire, dans l'ordre où Java l'a fait.
    pub effects: Vec<Effect>,
}

impl Decision {
    /// Aller en `state`.
    pub fn go(state: ServiceState) -> Decision {
        Decision {
            next: Some(state),
            refusal: None,
            effects: Vec::new(),
        }
    }

    /// `return getState()` : aucun changement d'état, aucune plainte.
    pub fn stay() -> Decision {
        Decision::default()
    }

    /// `return getState()` avec la raison notée.
    pub fn refuse(refusal: ServiceRefusal) -> Decision {
        Decision {
            next: None,
            refusal: Some(refusal),
            effects: Vec::new(),
        }
    }

    /// Aller en `state` en gardant le refus et les ordres déjà notés.
    pub fn going(mut self, state: ServiceState) -> Decision {
        self.next = Some(state);
        self
    }

    /// Noter une raison, si aucune ne l'est encore : la première branche qui échoue est
    /// celle qu'un joueur doit voir.
    pub fn because(mut self, refusal: ServiceRefusal) -> Decision {
        if self.refusal.is_none() {
            self.refusal = Some(refusal);
        }
        self
    }

    /// Ajouter un ordre pour l'appelant.
    pub fn effect(mut self, effect: Effect) -> Decision {
        self.effects.push(effect);
        self
    }
}

/// Ce qu'un tick d'une de ces trois IA a décidé : le but en vigueur, la règle qui l'a
/// choisi, le refus s'il n'a pas avancé, et les ordres qu'il laisse.
#[derive(Clone, Debug, PartialEq)]
pub struct ServiceOutcome {
    /// Le but en vigueur après ce tick.
    pub state: ServiceState,
    /// La transition qui a tourné, s'il y en a une.
    pub fired: Option<Fired<ServiceState>>,
    /// Le refus noté pendant ce tick.
    pub refusal: Option<ServiceRefusal>,
    /// Les ordres que ce tick laisse.
    pub effects: Vec<Effect>,
}

impl ServiceOutcome {
    /// Le but en vigueur.
    pub fn goal(&self) -> ServiceState {
        self.state
    }

    /// Pourquoi rien n'a bougé, si le tick a noté une raison.
    pub fn why_not(&self) -> Option<ServiceRefusal> {
        self.refusal
    }

    /// Ce tick a-t-il changé le but ?
    pub fn moved(&self) -> bool {
        matches!(self.fired, Some(fired) if fired.from != fired.to)
    }

    /// La machine est-elle tombée dans un état piège
    /// (`BasicStateMachine.java:220-228`) ?
    pub fn faulted(&self) -> bool {
        matches!(self.fired, Some(fired) if fired.reset)
    }

    /// La ligne Java de la règle qui a tourné, pour une ligne de log.
    pub fn why(&self) -> Option<&'static str> {
        self.fired.map(|fired| fired.java)
    }
}

/// Une IA de ces trois métiers : un métier et sa machine, rien d'autre.
pub struct ServiceAi {
    trade: ServiceTrade,
    machine: Machine<ServiceState, ServiceScene>,
}

impl fmt::Debug for ServiceAi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceAi")
            .field("trade", &self.trade)
            .field("machine", &self.machine)
            .finish()
    }
}

impl ServiceAi {
    /// L'IA du métier, qui part de `IDLE` à `ENTITY_AI_TICKRATE`
    /// (`AbstractEntityCitizen.java:64`).
    ///
    /// Java part d'`INIT` et `initSafetyChecks` (`AbstractEntityAIBasic.java:446-465`)
    /// l'envoie en `IDLE` ; `INIT` et ces vérifications appartiennent à `crate::ai`, donc
    /// cette machine commence là où l'autre arrive.
    pub fn new(trade: ServiceTrade) -> ServiceAi {
        let mut machine = Machine::new(ServiceState::Base(WorkerState::Idle), ENTITY_AI_TICKRATE);
        trade.register(&mut machine);
        ServiceAi { trade, machine }
    }

    /// Une scène sur `world` portant les faits du métier, vides.
    pub fn scene(&self, world: WorldFacts) -> ServiceScene {
        ServiceScene::new(world, self.trade.job_facts())
    }

    /// Une scène sur `world` et `job`. Un guérisseur à qui l'on donne les faits du
    /// croque-mort répond [`ServiceRefusal::NoFactsForTrade`] à sa première passe au lieu
    /// de prétendre qu'il n'a pas de patient.
    pub fn scene_with(&self, world: WorldFacts, job: ServiceJob) -> ServiceScene {
        ServiceScene::new(world, job)
    }

    /// Le métier de cette IA.
    pub fn trade(&self) -> ServiceTrade {
        self.trade
    }

    /// L'état de la machine (`AbstractAISkeleton.getState`, `:120-126`).
    pub fn state(&self) -> ServiceState {
        self.machine.state()
    }

    /// Le dernier piège, si un tick en a déclenché un (`BasicStateMachine.java:220-228`).
    pub fn missing(&self) -> Option<crate::ai::state::MissingTransition<ServiceState>> {
        self.machine.missing()
    }

    /// `AbstractAISkeleton.resetAI` (`:145-152`).
    pub fn reset(&mut self) {
        self.machine.reset();
    }

    /// La machine elle-même, pour un appelant qui veut lire la table.
    pub fn machine(&self) -> &Machine<ServiceState, ServiceScene> {
        &self.machine
    }

    /// Un tick d'IA (`AbstractAISkeleton.tick`, `:63-66`).
    pub fn tick(&mut self, scene: &mut ServiceScene) -> ServiceOutcome {
        scene.refusal = None;
        scene.effects.clear();
        let fired = self.machine.tick(scene);
        ServiceOutcome {
            state: self.machine.state(),
            fired,
            refusal: scene.refusal,
            effects: core::mem::take(&mut scene.effects),
        }
    }
}

/// `walkToBuilding()` (`AbstractEntityAIBasic.java:757-766`) : le citoyen est à moins de
/// `WOKR_IN_BUILDING_DIST` de sa hutte, le même test que fait `crate::ai::worker`.
pub fn at_hut(facts: &WorldFacts) -> bool {
    facts
        .worker
        .at
        .is_living_at_site(facts.building.position, WOKR_IN_BUILDING_DIST)
}

/// `inventoryNeedsDump` (`AbstractEntityAIBasic.java:406-417`) pour une scène de ces trois
/// métiers.
///
/// **Reformulée, parce qu'elle n'est pas atteignable telle quelle.** Le module `ai` a
/// cette règle en privé sur sa propre `Scene` (`crate::ai::worker::inventory_needs_dump`).
/// Les conditions sont les mêmes, moins les deux qui ne s'appliquent jamais ici : aucun de
/// ces trois métiers n'est `JobDeliveryman` (`:415`), et leurs états de travail répondent
/// `isOkayToEat = true` sauf `ENCHANT` et les quatre du croque-mort (voir
/// [`ServiceState::okay_to_eat`]).
///
/// Le drapeau `shouldDumpInventory` du croque-mort (`EntityAIWorkUndertaker.java:507-515`)
/// est lu ici et **consommé** par [`undertaker::want_inventory_dumped`], que la table de
/// base appelle.
pub fn dump_due(trade: ServiceTrade, scene: &ServiceScene) -> bool {
    let mut due = scene.facts.inventory.full // `:412`
        || scene.facts.inventory.actions_done >= trade.actions_until_dumping(); // `:413`
    if let ServiceJob::Undertaker(_) = &scene.job {
        due = due || scene.memory.undertaker.should_dump_inventory; // `:414`
    }
    due
}

/// `new AITarget(state, action, tickRate)` — la forme que les trois constructeurs
/// utilisent (`AITarget.java:24-53`), écrite une fois parce que les trois tables ajoutent
/// le même genre de transition.
pub(crate) fn add_target(
    machine: &mut Machine<ServiceState, ServiceScene>,
    state: ServiceState,
    rule: &'static str,
    java: &'static str,
    tick_rate: u32,
    action: fn(&mut ServiceScene, ServiceState) -> Option<ServiceState>,
) {
    machine.add(Transition::target(
        state,
        rule,
        java,
        tick_rate,
        Action::Run(action),
    ));
}

/// La forme d'`Action::Run` que chaque passe de métier a : découper la scène en ses
/// champs, lancer la décision sur les faits du métier, noter le refus et les ordres.
///
/// Écrite en macro parce que l'emprunt est la partie délicate : les faits du métier et la
/// mémoire sont deux champs d'une même structure, et seul un découpage permet de lire le
/// premier en écrivant la seconde.
///
/// **Chaque nom est qualifié par `$crate` à dessein.** `macro_rules!` résout les chemins
/// au site d'expansion, donc un `ServiceScene` nu ici ne compilerait que dans un module
/// qui l'importe par hasard.
macro_rules! service_action {
    ($name:ident, $job:ident, $decide:path) => {
        fn $name(
            scene: &mut $crate::ai_work_12::ServiceScene,
            state: $crate::ai_work_12::ServiceState,
        ) -> Option<$crate::ai_work_12::ServiceState> {
            let $crate::ai_work_12::ServiceScene {
                facts,
                job,
                memory,
                refusal,
                effects,
            } = scene;
            let $crate::ai_work_12::ServiceJob::$job(trade) = job else {
                *refusal = Some($crate::ai_work_12::ServiceRefusal::NoFactsForTrade);
                return Some(state);
            };
            let decision: $crate::ai_work_12::Decision = $decide(facts, trade, memory);
            *refusal = decision.refusal;
            *effects = decision.effects;
            decision.next.or(Some(state))
        }
    };
}

pub(crate) use service_action;

#[cfg(test)]
mod trade_tests {
    use super::*;

    #[test]
    fn each_trade_names_its_java_class_its_job_and_its_idle_rate() {
        assert_eq!(ServiceTrade::Enchanter.class(), "EntityAIWorkEnchanter");
        assert_eq!(ServiceTrade::Healer.class(), "EntityAIWorkHealer");
        assert_eq!(ServiceTrade::Undertaker.class(), "EntityAIWorkUndertaker");
        assert_eq!(ServiceTrade::Enchanter.job(), Job::Enchanter);
        assert_eq!(ServiceTrade::Healer.job(), Job::Healer);
        assert_eq!(ServiceTrade::Undertaker.job(), Job::Undertaker);
        assert_eq!(ServiceTrade::ALL.len(), 3);
        assert_eq!(ServiceTrade::Undertaker.to_string(), "undertaker");
        // Les trois cadences de `IDLE`, qui ne se ressemblent pas : 20, 1, 60.
        assert_eq!(ServiceTrade::Enchanter.idle_rate(), 20);
        assert_eq!(ServiceTrade::Healer.idle_rate(), 1);
        assert_eq!(ServiceTrade::Undertaker.idle_rate(), 60);
        assert_eq!(
            ServiceTrade::Enchanter.entry_java(),
            "AbstractEntityAICrafting.java:164"
        );
        for trade in ServiceTrade::ALL {
            assert_eq!(trade.entry(), ServiceState::Base(WorkerState::StartWorking));
        }
    }

    #[test]
    fn only_the_healer_changes_what_a_pickup_returns_to() {
        // `EntityAIWorkHealer.java:470-473` contre `AbstractEntityAIBasic.java:323-326`.
        assert_eq!(ServiceTrade::Healer.state_after_pickup(), ServiceState::Cure);
        assert_eq!(
            ServiceTrade::Enchanter.state_after_pickup(),
            ServiceState::Base(WorkerState::StartWorking)
        );
        assert_eq!(
            ServiceTrade::Undertaker.state_after_pickup(),
            ServiceState::Base(WorkerState::StartWorking)
        );
        assert_eq!(
            ServiceTrade::Healer.state_after_pickup_java(),
            "EntityAIWorkHealer.java:470-473"
        );
    }

    #[test]
    fn the_dump_threshold_is_one_action_for_the_enchanter_and_thirty_two_for_the_others() {
        // `EntityAIWorkEnchanter.java:214-218` contre `AbstractEntityAIBasic.java:425-428`.
        assert_eq!(ServiceTrade::Enchanter.actions_until_dumping(), 1);
        assert_eq!(ServiceTrade::Healer.actions_until_dumping(), 32);
        assert_eq!(ServiceTrade::Undertaker.actions_until_dumping(), 32);

        let scene = ServiceScene::new(WorldFacts::new(), ServiceTrade::Enchanter.job_facts());
        assert!(!dump_due(ServiceTrade::Enchanter, &scene), "zéro action");
        let mut full = scene.clone();
        full.facts.inventory.actions_done = 1;
        assert!(dump_due(ServiceTrade::Enchanter, &full));
        // Le même compteur ne dit rien aux deux autres métiers.
        assert!(!dump_due(ServiceTrade::Healer, &full));
        // Le drapeau du croque-mort, lui, suffit (`EntityAIWorkUndertaker.java:507-515`).
        let mut grave = ServiceScene::new(
            WorldFacts::new(),
            ServiceTrade::Undertaker.job_facts(),
        );
        assert!(!dump_due(ServiceTrade::Undertaker, &grave));
        grave.memory.undertaker.should_dump_inventory = true;
        assert!(dump_due(ServiceTrade::Undertaker, &grave));
    }

    #[test]
    fn the_hand_over_gives_every_base_state_one_way_out_and_idle_one_exit() {
        for trade in ServiceTrade::ALL {
            let ai = ServiceAi::new(trade);
            let machine = ai.machine();
            for state in [
                WorkerState::Idle,
                WorkerState::InventoryFull,
                WorkerState::NeedsItem,
                WorkerState::Paused,
                WorkerState::GatheringRequiredMaterials,
            ] {
                assert!(
                    machine.can_leave(ServiceState::Base(state)),
                    "{trade} n'a pas de sortie de {state}"
                );
            }
            // Cinq transitions de relais, et `IDLE` n'en reçoit qu'une.
            let idle = machine
                .transitions()
                .iter()
                .filter(|transition| transition.state == Some(ServiceState::Base(WorkerState::Idle)))
                .count();
            assert_eq!(idle, 1, "{trade}");
            assert_eq!(
                machine
                    .transitions()
                    .iter()
                    .filter(|transition| transition.rule == "idleToEntry")
                    .count(),
                1
            );
        }
        assert_eq!(ServiceAi::new(ServiceTrade::Enchanter).machine().transitions().len(), 8);
        assert_eq!(ServiceAi::new(ServiceTrade::Healer).machine().transitions().len(), 12);
        assert_eq!(
            ServiceAi::new(ServiceTrade::Undertaker).machine().transitions().len(),
            11
        );
    }

    #[test]
    fn a_trade_action_on_another_trade_s_facts_refuses_by_name() {
        // Le `let … else` de la macro : un guérisseur à qui l'on donne les faits du
        // croque-mort. `IDLE → START_WORKING` (`:86`), `START_WORKING → DECIDE` (`:87`),
        // puis la cible `DECIDE → decide` (`:88`) qui voit les mauvais faits.
        let mut ai = ServiceAi::new(ServiceTrade::Healer);
        let mut scene = ai.scene_with(
            WorldFacts::new(),
            ServiceJob::Undertaker(UndertakerFacts::default()),
        );
        let walking = ai.tick(&mut scene);
        assert_eq!(walking.state, ServiceState::Base(WorkerState::StartWorking));
        let deciding = ai.tick(&mut scene);
        assert_eq!(deciding.state, ServiceState::Base(WorkerState::Decide));
        let wrong = ai.tick(&mut scene);
        assert_eq!(wrong.refusal, Some(ServiceRefusal::NoFactsForTrade));
        assert!(wrong.effects.is_empty());
        assert!(ServiceRefusal::NoFactsForTrade.java().starts_with("port:"));
        assert!(scene.undertaker().is_some());
        assert!(scene.healer().is_none());
    }

    #[test]
    fn states_of_lists_the_trade_states_then_the_base_ones_its_hand_over_crosses() {
        // `DECIDE` n'apparaît que chez le guérisseur (`EntityAIWorkHealer.java:87`), et
        // `WANDER` chez les deux métiers qui l'enregistrent (`:93`, `:92`).
        let enchanter = states_of(ServiceTrade::Enchanter);
        assert_eq!(enchanter.len(), 2 + 6);
        assert!(enchanter.contains(&ServiceState::Enchant));
        assert!(!enchanter.contains(&ServiceState::Cure));
        assert!(!enchanter.contains(&ServiceState::Wander));
        assert!(!enchanter.contains(&ServiceState::Base(WorkerState::Decide)));
        let healer = states_of(ServiceTrade::Healer);
        assert_eq!(healer.len(), 4 + 6 + 1 + 1);
        assert!(healer.contains(&ServiceState::Wander));
        assert!(healer.contains(&ServiceState::Base(WorkerState::Decide)));
        let undertaker = states_of(ServiceTrade::Undertaker);
        assert_eq!(undertaker.len(), 4 + 6 + 1);
        assert!(undertaker.contains(&ServiceState::Wander));
        assert!(!undertaker.contains(&ServiceState::Cure));
    }

    #[test]
    fn the_decision_builder_keeps_the_first_reason_and_the_effect_order() {
        let decision = Decision::stay()
            .effect(Effect::KeepGraveData)
            .because(ServiceRefusal::NoGraveToWorkOn)
            .because(ServiceRefusal::NoPatient)
            .effect(Effect::WalkToHut)
            .going(ServiceState::Wander);
        assert_eq!(decision.next, Some(ServiceState::Wander));
        assert_eq!(
            decision.refusal,
            Some(ServiceRefusal::NoGraveToWorkOn),
            "la première raison gagne"
        );
        assert_eq!(
            decision.effects,
            vec![Effect::KeepGraveData, Effect::WalkToHut],
            "les ordres restent dans l'ordre où ils ont été donnés"
        );
        assert_eq!(Decision::refuse(ServiceRefusal::NoPatient).next, None);
    }

    #[test]
    fn the_two_huts_are_measured_the_same_way_walk_to_building_is() {
        let facts = WorldFacts::new();
        assert!(at_hut(&facts));
        let mut far = facts;
        far.worker.at = Pos::new(7, 64, 0);
        assert!(!at_hut(&far), "7 blocs n'est pas dans un rayon de 7");
    }
}
