//! Les buts d'IA de l'éleveur de porcs, du cuisinier et du livreur : leur machine à
//! états, ce qu'elle demande au monde, et pourquoi elle s'arrête.
//!
//! Java référence trois classes de `core/entity/ai/workers/` :
//!
//! | Métier | Classe Java | Lignes | Ce qu'elle ajoute |
//! | --- | --- | ---: | --- |
//! | éleveur de porcs | `production/herders/EntityAIWorkSwineHerder.java` | 47 | trois surcharges, aucun état |
//! | cuisinier | `service/EntityAIWorkCook.java` | 438 | deux états et six remplacements |
//! | livreur | `service/EntityAIWorkDeliveryman.java` | 662 | six états, déjà portés |
//!
//! ## Les deux prémisses du brief qui ne tiennent pas
//!
//! 1. **« chercher, aller, agir, rendre » n'est la forme d'aucun des trois.** L'éleveur
//!    de porcs a les quatre états qui se chevauchent d'`AbstractEntityAIHerder` (accoupler,
//!    abattre, nourrir, ramasser) plus les quatre états de base ; le cuisinier a les six
//!    cibles du four **et** ses deux états de service, et son point d'entrée est
//!    `START_WORKING`, pas `IDLE` ; le livreur est un **dispatcheur à quatre courses** dont
//!    deux sont des détours (`DUMPING` avant une livraison, `:615-621`, et avant une
//!    collecte, `:156-161`). Aplatir ces tables sur quatre cases perdrait des états.
//!    Chaque table est donc portée telle quelle, un état par cible Java.
//! 2. **Le livreur et l'éleveur de porcs ont déjà leur table.** Les six cibles du livreur
//!    sont dans `crate::ai_build::delivery` (avec `checkIfExecute`, `prepareDelivery`,
//!    `deliver`, `pickup`, `dump` et les deux limites), et les huit cibles de l'éleveur
//!    sont dans `crate::ai_work_09::herder` : `EntityAIWorkSwineHerder` n'enregistre
//!    **aucune** cible, son constructeur est `super(job)` et rien de plus. Les réécrire
//!    serait la deuxième copie d'une table qui a déjà ses tests. Ce module les
//!    **raccorde** ([`swine::register`], [`courier::register`]) et le prouve par deux
//!    tests de parité qui font tourner les deux machines sur les mêmes faits
//!    ([`tests`]). Le cuisinier, lui, est neuf : six cibles héritées du four et deux qui
//!    n'existaient nulle part.
//!
//! ## La machine, et ce qu'elle est
//!
//! Une table par métier, sur le moteur [`crate::ai::state::Machine`] : trois passes
//! globales, puis la liste de l'état courant, et le premier qui tire arrête le tick.
//! `S` vaut [`TradeState`], `F` vaut [`Scene`].
//!
//! ```ignore
//! let mut ai = TradeAi::new(Trade::Cook);
//! let mut scene = ai.scene_with(WorldFacts::new(), TradeJob::Cook(CookFacts::new()));
//! let outcome = ai.tick(&mut scene);
//! match (outcome.goal(), outcome.why_not()) {
//!     (TradeState::Cook(CookState::ServeFoodToCitizen), _) => /* servir la file */,
//!     (_, Some(Refusal::NoFurnace)) => /* la hutte n'a aucun four */,
//!     (_, Some(Refusal::Courier(Refusal::NoWarehouse))) => /* pas d'entrepôt */,
//!     _ => {}
//! }
//! ```
//!
//! ## Ce qui est réutilisé, et d'où
//!
//! - **Le moteur** : `crate::ai::state::Machine`, ses trois passes, son compte à rebours
//!   et sa règle de piège (`BasicStateMachine.java:220-228`). Ce module ajoute des
//!   tables, pas une deuxième boucle de tick.
//! - **Les faits de base** : `crate::ai::facts::WorldFacts` — le citoyen, son inventaire,
//!   sa hutte, ses blocs. [`Scene::facts`] est cette *même* valeur, rien n'en est copié.
//! - **La mémoire de base** : `crate::ai::worker::Memory` (`delay`, `walk_to`, `need`),
//!   prise dans [`facts::TradeMemory::base`].
//! - **Les états** : [`TradeState`] enveloppe `crate::ai_work_09::state::WorkState`
//!   (éleveur), `crate::ai_work_04::states::SmeltState` (four) et
//!   `crate::ai_build::delivery::DeliveryState` (livreur). Seuls les deux états du
//!   cuisinier sont neufs.
//! - **Les refus** : [`Refusal`] enveloppe les trois énumérations des voisins et n'ajoute
//!   que les quatorze raisons du cuisinier et les deux erreurs d'appelant.
//! - **Les règles du four** : `crate::ai_work_04::smelter` — `fill_up_furnace`,
//!   `retrieve_product`, `retrieve_used_fuel`, `check_if_able_to_smelt`,
//!   `oven_to_retrieve_from`, `oven_to_retrieve_fuel_from`, `reached_max_to_keep`,
//!   `acceleration_ticks`.
//! - **Les règles d'élevage** : `crate::ai_work_09::herder` — les sept décisions.
//! - **Les règles du livreur** : `crate::ai_build::delivery` — les six décisions et les
//!   deux portes.
//!
//! ## Ce qui n'est pas ici, et pourquoi
//!
//! - **Aucun appel au monde.** Pas de `walkToBuilding`, pas de `transferXOfItemStack`,
//!   pas de `mobInteract`, pas de `serverTick`. Chaque lecture est un fait ([`facts`]),
//!   chaque écriture une sortie de la mémoire, du compteur d'actions ou d'un [`Effect`].
//! - **Aucune API de mod.** Ce module n'importe pas `mod_api` : il ne dépend que de
//!   `crate::ai`, `crate::ai_work_04`, `crate::ai_work_09`, `crate::ai_build` et de ses
//!   propres fichiers, le même choix que `crate::ai_gather`, `crate::ai_build` et
//!   `crate::ai_work_09`.
//! - **Le système de requêtes.** Un `StackList` de carburant, une `Food(STACKSIZE,
//!   niveau)`, un `needsCurrently` : ce sont des [`cook::Gather`] et des booléens de
//!   [`cook::Start`], pas des appels. `crate::requests` les crée.
//! - **`FoodUtils`.** `getBestFoodForCitizen` (200 lignes de score de diversité et de
//!   qualité), `hasBestOptionInInv` et `checkForFoodInBuilding` sont des **réponses** lues
//!   dans les faits ; `transferFoodUpToSaturation`, lui, est porté
//!   ([`cook::plan_food_transfer`]) parce que la quantité qu'il rend est une décision.
//!
//! `MANQUANT.md` liste les trous d'API et les constats mesurés sur le Java ;
//! `RAPPORT.md` dit ce qui est couvert, ce qui ne l'est pas, et pourquoi.

pub mod cook;
pub mod courier;
pub mod facts;
pub mod state;
pub mod swine;

#[cfg(test)]
mod tests;

use core::fmt;

use crate::ai::facts::WorldFacts;
use crate::ai::state::{
    Action, Fired, Machine, MissingTransition, Transition, WorkerState, ENTITY_AI_TICKRATE,
};

pub use facts::{
    CitizenHere, CookFacts, CookMemory, FoodStack, Scene, ServeCitizen, ServePlayer, SwineFacts,
    TradeJob, TradeMemory,
};
pub use state::{CookState, Refusal, TradeState};

/// `Constants.TICKS_SECOND` (`api/util/constant/Constants.java:21`) : la cadence des
/// cibles d'élevage (`AbstractEntityAIHerder.java:141`, `:142`, `:146`, `:147`), de
/// l'événement d'accélération (`AbstractEntityAIUsesFurnace.java:80`) et de `DUMPING`
/// (`EntityAIWorkDeliveryman.java:122`).
pub const TICKS_SECOND: u32 = 20;

/// `Constants.STACKSIZE` (`Constants.java:23`) : la taille d'une requête
/// (`AbstractEntityAIUsesFurnace.java:246`, `:251`, `:256` ; `EntityAIWorkCook.java:386`,
/// `:404`, `:436`).
pub const STACKSIZE: u32 = 64;

/// `AbstractEntityAIBasic.STANDARD_DELAY` (`AbstractEntityAIBasic.java:100`) : la
/// cadence des cibles qui ne lisent que des faits (`AbstractEntityAIUsesFurnace.java:77`,
/// `:79`, `:81`, `:82` ; `EntityAIWorkDeliveryman.java:119`, `:120`).
pub const STANDARD_DELAY: u32 = crate::ai::worker::STANDARD_DELAY;

/// Les trois métiers dont ce module porte le but d'IA.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: the three trades of this slice
pub enum Trade {
    /// `EntityAIWorkSwineHerder` (`EntityAIWorkSwineHerder.java:16`).
    SwineHerder,
    /// `EntityAIWorkCook` (`EntityAIWorkCook.java:54`).
    Cook,
    /// `EntityAIWorkDeliveryman` (`EntityAIWorkDeliveryman.java:57`).
    Deliveryman,
}

impl Trade {
    /// Les trois, dans l'ordre où ce module les documente.
    pub const ALL: [Trade; 3] = [Trade::SwineHerder, Trade::Cook, Trade::Deliveryman];

    /// Le nom de la classe Java, pour une ligne de journal.
    pub fn class(self) -> &'static str {
        match self {
            Trade::SwineHerder => "EntityAIWorkSwineHerder",
            Trade::Cook => "EntityAIWorkCook",
            Trade::Deliveryman => "EntityAIWorkDeliveryman",
        }
    }

    /// Le nom du métier, pour une clé de message.
    pub fn name(self) -> &'static str {
        match self {
            Trade::SwineHerder => "swineherder",
            Trade::Cook => "cook",
            Trade::Deliveryman => "deliveryman",
        }
    }

    /// Le métier correspondant dans `crate::jobs`, le pont pour un appelant qui a déjà
    /// un `Job`.
    pub fn job(self) -> crate::jobs::job::Job {
        match self {
            Trade::SwineHerder => crate::jobs::job::Job::SwineHerder,
            Trade::Cook => crate::jobs::job::Job::Cook,
            Trade::Deliveryman => crate::jobs::job::Job::Deliveryman,
        }
    }

    /// L'état de départ de la table : `IDLE` pour les trois, chacun dans son énumération.
    ///
    /// Java commence en `INIT`, et `initSafetyChecks` (`AbstractEntityAIBasic.java:
    /// 446-465`) l'envoie en `IDLE` ; `INIT` et ces vérifications appartiennent à
    /// `crate::ai`, donc ces machines commencent là où l'autre arrive.
    pub fn initial_state(self) -> TradeState {
        match self {
            Trade::SwineHerder => {
                TradeState::Herd(crate::ai_work_09::state::WorkState::Base(WorkerState::Idle))
            }
            Trade::Cook => TradeState::Furnace(crate::ai_work_04::states::SmeltState::Idle),
            Trade::Deliveryman => {
                TradeState::Courier(crate::ai_build::delivery::DeliveryState::Idle)
            }
        }
    }

    /// Les faits du métier, vides.
    pub fn job_facts(self) -> TradeJob {
        match self {
            Trade::SwineHerder => TradeJob::Swine(SwineFacts::default()),
            Trade::Cook => TradeJob::Cook(CookFacts::default()),
            Trade::Deliveryman => {
                TradeJob::Deliveryman(crate::ai_build::delivery::DeliveryFacts::default())
            }
        }
    }

    /// `getActionsDoneUntilDumping`, quand le métier en a un.
    ///
    /// Les trois métiers ne répondent pas la même chose : l'éleveur de porcs hérite des
    /// dix de `AbstractEntityAIHerder.java:152-156`, le cuisinier des **un** de
    /// `EntityAIWorkCook.java:427-431`, et le livreur **aucun** — il est exempté
    /// nommément par `AbstractEntityAIBasic.java:415` (`job instanceof JobDeliveryman`).
    /// `None` est donc la bonne réponse pour lui, pas un chiffre au hasard.
    pub fn actions_until_dumping(self) -> Option<u32> {
        match self {
            Trade::SwineHerder => Some(swine::actions_until_dumping()),
            Trade::Cook => Some(cook::ACTIONS_UNTIL_DUMP),
            Trade::Deliveryman => None,
        }
    }

    /// Enregistre la table de ce métier — le corps des `registerTargets` de ses
    /// constructeurs.
    pub fn register(self, machine: &mut Machine<TradeState, Scene>) {
        match self {
            Trade::SwineHerder => swine::register(machine),
            Trade::Cook => cook::register(machine),
            Trade::Deliveryman => courier::register(machine),
        }
    }
}

impl fmt::Display for Trade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// L'effet d'une décision, quand sa **quantité** ne se lit nulle part ailleurs.
///
/// Une action d'IA ne rend qu'un état ; Java, lui, écrit dans le monde au passage. Trois
/// des branches de ce module décident d'un nombre d'items (deux chez le cuisinier, une
/// pour un joueur) : sans le porter, un appelant devrait redeviner la formule, et un test
/// ne pourrait pas l'assertir. Les transferts de four, eux, ne sont pas ici : ils sont
/// visibles dans les faits du four, que l'appelant relit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three quantities these three trades decide and cannot re-read
pub enum Effect {
    /// Un citoyen au sac plein mange sur place (`EntityAIWorkCook.java:192-211`) :
    /// `items` items sont extraits et sa saturation monte d'autant.
    FeedInPlace {
        /// Le citoyen servi.
        citizen: u32,
        /// Combien d'items il consomme, voir [`cook::feed_items`].
        items: u32,
    },
    /// `transferXOfItemStackIntoNextFreeSlotInItemHandler(...)` (`:240`) : `qty` items du
    /// slot `slot` passent dans l'inventaire du citoyen.
    ServePortion {
        /// Le citoyen servi.
        citizen: u32,
        /// Le slot du sac du cuisinier, celui que `getBestFoodForCitizen` a rendu.
        slot: u32,
        /// Combien d'items, voir [`cook::portion_for`].
        qty: u32,
    },
    /// `transferFoodUpToSaturation(...)` (`:288`) : `items` items partent vers le joueur.
    ServePlayer {
        /// Le joueur servi.
        player: u32,
        /// Combien d'items, voir [`cook::plan_food_transfer`].
        items: u32,
    },
}

impl Effect {
    /// La ligne Java qui décide cette quantité.
    pub fn java(self) -> &'static str {
        match self {
            Effect::FeedInPlace { .. } => "EntityAIWorkCook.java:192-211",
            Effect::ServePortion { .. } => "EntityAIWorkCook.java:236-246",
            Effect::ServePlayer { .. } => "EntityAIWorkCook.java:288-290",
        }
    }
}

/// Ce qu'une action de métier a décidé, dans les trois moitiés que Java garde séparées :
/// l'état qu'elle rend (`return getState()` quand elle ne peut pas aller plus loin), la
/// branche qui a refusé, et la quantité qu'elle a posée.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Decision {
    /// L'état rendu, `None` pour « on reste dans l'état courant » — la même forme que le
    /// `Option` d'`Action::Run`.
    pub next: Option<TradeState>,
    /// Pourquoi il s'est arrêté, quand il s'est arrêté.
    pub refusal: Option<Refusal>,
    /// La quantité décidée, quand elle se perd ailleurs.
    pub effect: Option<Effect>,
}

impl Decision {
    /// Aller en `state`.
    pub fn go(state: TradeState) -> Decision {
        Decision {
            next: Some(state),
            refusal: None,
            effect: None,
        }
    }

    /// `return getState()` : aucun changement d'état, aucune plainte.
    pub fn stay() -> Decision {
        Decision::default()
    }

    /// `return getState()` avec la raison notée.
    pub fn refuse(refusal: Refusal) -> Decision {
        Decision {
            refusal: Some(refusal),
            ..Decision::default()
        }
    }

    /// Aller en `state` **et** noter pourquoi.
    pub fn go_after(state: TradeState, refusal: Refusal) -> Decision {
        Decision {
            next: Some(state),
            refusal: Some(refusal),
            effect: None,
        }
    }

    /// Ajouter la quantité décidée.
    pub fn with_effect(mut self, effect: Effect) -> Decision {
        self.effect = Some(effect);
        self
    }
}

/// Ce qu'un tick de ces trois buts a décidé : le but en vigueur, la règle qui l'a choisi,
/// le refus s'il n'est pas allé plus loin, et la quantité qu'il a posée — les mêmes
/// réponses qu'`crate::ai::worker::Outcome`, pour un [`TradeState`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TradeOutcome {
    /// Le but en vigueur après ce tick.
    pub state: TradeState,
    /// La transition qui a tiré, si une a tiré.
    pub fired: Option<Fired<TradeState>>,
    /// Le refus noté pendant ce tick, s'il y en a un.
    pub refusal: Option<Refusal>,
    /// La quantité décidée pendant ce tick, s'il y en a une.
    pub effect: Option<Effect>,
}

impl TradeOutcome {
    /// Le but en vigueur.
    pub fn goal(&self) -> TradeState {
        self.state
    }

    /// Pourquoi rien n'a bougé, si le tick a noté une raison.
    pub fn why_not(&self) -> Option<Refusal> {
        self.refusal
    }

    /// La quantité décidée, si le tick en a décidé une.
    pub fn effect(&self) -> Option<Effect> {
        self.effect
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
pub struct TradeAi {
    trade: Trade,
    machine: Machine<TradeState, Scene>,
}

impl fmt::Debug for TradeAi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TradeAi")
            .field("trade", &self.trade)
            .field("machine", &self.machine)
            .finish()
    }
}

impl TradeAi {
    /// Le but d'IA du métier, à partir de `IDLE` et à `ENTITY_AI_TICKRATE`
    /// (`AbstractEntityCitizen.java:64`).
    pub fn new(trade: Trade) -> TradeAi {
        let mut machine = Machine::new(trade.initial_state(), ENTITY_AI_TICKRATE);
        trade.register(&mut machine);
        TradeAi { trade, machine }
    }

    /// Une scène sur `world` portant les faits vides de ce métier.
    pub fn scene(&self, world: WorldFacts) -> Scene {
        Scene::new(world, self.trade.job_facts())
    }

    /// Une scène sur `world` et `job`. Un cuisinier à qui on donne les faits d'un livreur
    /// répond [`Refusal::NoFactsForTrade`] à son premier tick au lieu de faire semblant
    /// de n'avoir personne à servir.
    pub fn scene_with(&self, world: WorldFacts, job: TradeJob) -> Scene {
        Scene::new(world, job)
    }

    /// Le métier de cette IA.
    pub fn trade(&self) -> Trade {
        self.trade
    }

    /// L'état de la machine (`AbstractAISkeleton.getState`, `:120-126`).
    pub fn state(&self) -> TradeState {
        self.machine.state()
    }

    /// Le dernier piège, si un tick en a déclenché un
    /// (`BasicStateMachine.java:220-228`).
    pub fn missing(&self) -> Option<MissingTransition<TradeState>> {
        self.machine.missing()
    }

    /// `AbstractAISkeleton.resetAI` (`:145-152`).
    pub fn reset(&mut self) {
        self.machine.reset();
    }

    /// La machine elle-même, pour un appelant qui veut lire la table.
    pub fn machine(&self) -> &Machine<TradeState, Scene> {
        &self.machine
    }

    /// Un tick d'IA (`AbstractAISkeleton.tick`, `:63-66`).
    pub fn tick(&mut self, scene: &mut Scene) -> TradeOutcome {
        scene.refusal = None;
        scene.effect = None;
        let fired = self.machine.tick(scene);
        TradeOutcome {
            state: self.machine.state(),
            fired,
            refusal: scene.refusal,
            effect: scene.effect,
        }
    }
}

/// `walkToBuilding()` (`AbstractEntityAIBasic.java:757-766`) : le citoyen est à moins de
/// `WOKR_IN_BUILDING_DIST` de sa hutte, le même test que `crate::ai::worker`.
pub fn at_hut(facts: &WorldFacts) -> bool {
    facts.worker.at.is_living_at_site(
        facts.building.position,
        crate::ai::worker::WOKR_IN_BUILDING_DIST,
    )
}

/// `inventoryNeedsDump` (`AbstractEntityAIBasic.java:409-416`) pour une de ces scènes.
///
/// Les trois conditions sont celles de la classe de base — pas en `INVENTORY_FULL`, donc
/// interruptible (`:411`) ; l'inventaire est plein ou le compteur d'actions a atteint le
/// seuil du métier (`:412-413`) ; et le citoyen n'est pas un livreur (`:415`) — avec une
/// réponse de plus : un métier sans seuil ([`Trade::actions_until_dumping`] `== None`)
/// n'est jamais concerné, ce qui est exactement l'exemption du livreur.
pub fn dump_due(trade: Trade, scene: &Scene, state: TradeState) -> bool {
    let Some(limit) = trade.actions_until_dumping() else {
        return false; // `:415`
    };
    let dumpable_state = state.base() != Some(WorkerState::InventoryFull) && state.okay_to_eat(); // `:411`
    dumpable_state
        && (scene.facts.inventory.full // `:412`
            || scene.facts.inventory.actions_done >= limit) // `:413`
        && !scene.facts.worker.deliveryman // `:415`
}

/// `new AITarget(state, action, tickRate)` — la forme qu'utilisent les trois
/// constructeurs (`AITarget.java:24-53`), écrite une fois parce que les trois tables
/// ajoutent le même genre de cible et qu'une fermeture tiendrait `machine` emprunté.
pub(crate) fn add_target(
    machine: &mut Machine<TradeState, Scene>,
    state: TradeState,
    rule: &'static str,
    java: &'static str,
    tick_rate: u32,
    action: fn(&mut Scene, TradeState) -> Option<TradeState>,
) {
    machine.add(Transition::target(
        state,
        rule,
        java,
        tick_rate,
        Action::Run(action),
    ));
}
