//! Les états que ces trois métiers ajoutent à `AIWorkerState`, leurs refus nommés, et
//! la table commune aux trois — qui n'est **pas** recopiée de
//! `crate::ai_build::crafter` : chaque action appelle la fonction de décision du
//! module voisin, et un test vérifie que les deux tables répondent pareil.
//!
//! Référence Java : `api/entity/ai/statemachine/states/AIWorkerState.java`
//! (`START_USING_BREWINGSTAND` `:647`, `RETRIEVING_END_PRODUCT_FROM_BREWINGSTAMD`
//! `:652`, `RETRIEVING_USED_FUEL_FROM_BREWINGSTAND` `:657`, `ADD_FUEL_TO_BREWINGSTAND`
//! `:662`, `HARVEST_MISTLETOE` `:667`, `HARVEST_NETHERWART` `:672`,
//! `FILL_UP_FURNACES` `:305`, `RETRIEVING_END_PRODUCT_FROM_FURNACE` `:315`,
//! `RETRIEVING_UNRELATED_PRODUCT_FROM_FURNACE` `:320`, `ADD_FUEL_TO_FURNACE` `:330`).
//!
//! **Pourquoi `Base(CraftState)`.** Les sept états du métier d'atelier (`IDLE`,
//! `START_WORKING`, `QUERY_ITEMS`, `GET_RECIPE`, `CRAFT`, `INVENTORY_FULL`,
//! `GATHERING_REQUIRED_MATERIALS`) sont déjà nommés par
//! [`crate::ai_build::crafter::CraftState`], et cette énumération appartient à un
//! autre dossier. Un état commun est donc [`WorkState::Base`] enveloppant le
//! `CraftState` que l'autre module connaît — aucun état n'est écrit deux fois.
//!
//! **Pourquoi la table est réécrite quand même.** `Machine<S, F>` est monomorphe en
//! `S` **et** en `F` : une machine ne peut pas contenir la machine du crafter, qui
//! tique sur `CraftFacts`. Ce fichier redéclare donc les sept cibles de
//! `AbstractEntityAICrafting.java:164-169` sur [`Scene`], mais **chaque action
//! appelle** `crafter::decide`, `crafter::check_for_items`, `crafter::get_recipe`,
//! `crafter::craft` et `crafter::execute` : la règle n'a qu'un domicile, seule la
//! cible est dite deux fois. Les tests `the_shared_table_answers_like_the_crafter`
//! tiennent ce contrat.

use core::fmt;

use crate::ai::{Action, Fired, Machine, Transition, ENTITY_AI_TICKRATE};
use crate::ai_build::crafter::{self, CraftFacts, CraftState};

use super::brewing;
use super::facts::TradeFacts;
use super::furnace;
use super::{STANDARD_DELAY, TICKS_SECOND};

/// Un état d'une des trois tables : un état commun rendu au crafter, ou un état que ce
/// module nomme.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
// exhaustive: les sept états communs plus les dix que ces trois métiers possèdent
pub enum WorkState {
    /// Un état de [`CraftState`] — `IDLE`, `START_WORKING`, `QUERY_ITEMS`,
    /// `GET_RECIPE`, `CRAFT`, `INVENTORY_FULL`, `GATHERING_REQUIRED_MATERIALS`.
    Base(CraftState),
    /// `START_USING_BREWINGSTAND` (`AIWorkerState.java:647`) — emplir l'alambic.
    FillUpBrewingStand,
    /// `RETRIEVING_END_PRODUCT_FROM_BREWINGSTAMD` (`:652`, la faute de frappe est
    /// dans Java et se recopie) — sortir les potions.
    RetrieveBrewed,
    /// `RETRIEVING_USED_FUEL_FROM_BREWINGSTAND` (`:657`) — sortir la poudre usée.
    RetrieveUsedFuel,
    /// `ADD_FUEL_TO_BREWINGSTAND` (`:662`) — remettre de la poudre de blaze.
    AddFuelToBrewingStand,
    /// `HARVEST_MISTLETOE` (`:667`) — frapper des feuilles.
    HarvestMistletoe,
    /// `HARVEST_NETHERWART` (`:672`) — récolter ou replanter la verrue du nether.
    HarvestNetherWart,
    /// `FILL_UP_FURNACES` (`:305`) — emplir un four.
    FillUpFurnaces,
    /// `RETRIEVING_END_PRODUCT_FROM_FURNACE` (`:315`) — sortir le pain cuit.
    RetrieveProductFromFurnace,
    /// `RETRIEVING_UNRELATED_PRODUCT_FROM_FURNACE` (`:320`) — sortir ce qui n'est pas
    /// de la recette.
    RetrieveUnrelatedProduct,
    /// `ADD_FUEL_TO_FURNACE` (`:330`) — remettre du charbon.
    AddFuelToFurnace,
}

impl WorkState {
    /// Les dix-sept, états communs puis états de métier dans l'ordre de `AIWorkerState`.
    pub const ALL: [WorkState; 17] = [
        WorkState::Base(CraftState::Idle),
        WorkState::Base(CraftState::StartWorking),
        WorkState::Base(CraftState::QueryItems),
        WorkState::Base(CraftState::GetRecipe),
        WorkState::Base(CraftState::Craft),
        WorkState::Base(CraftState::InventoryFull),
        WorkState::Base(CraftState::GatheringRequiredMaterials),
        WorkState::FillUpFurnaces,
        WorkState::RetrieveProductFromFurnace,
        WorkState::RetrieveUnrelatedProduct,
        WorkState::AddFuelToFurnace,
        WorkState::FillUpBrewingStand,
        WorkState::RetrieveBrewed,
        WorkState::RetrieveUsedFuel,
        WorkState::AddFuelToBrewingStand,
        WorkState::HarvestMistletoe,
        WorkState::HarvestNetherWart,
    ];

    /// L'identifiant Java, celui que l'historique d'états et le nametag affichent
    /// (`AbstractEntityAIBasic.java:475`).
    pub fn java(self) -> &'static str {
        match self {
            WorkState::Base(state) => state.java_name(),
            WorkState::FillUpBrewingStand => "START_USING_BREWINGSTAND",
            WorkState::RetrieveBrewed => "RETRIEVING_END_PRODUCT_FROM_BREWINGSTAMD",
            WorkState::RetrieveUsedFuel => "RETRIEVING_USED_FUEL_FROM_BREWINGSTAND",
            WorkState::AddFuelToBrewingStand => "ADD_FUEL_TO_BREWINGSTAND",
            WorkState::HarvestMistletoe => "HARVEST_MISTLETOE",
            WorkState::HarvestNetherWart => "HARVEST_NETHERWART",
            WorkState::FillUpFurnaces => "FILL_UP_FURNACES",
            WorkState::RetrieveProductFromFurnace => "RETRIEVING_END_PRODUCT_FROM_FURNACE",
            WorkState::RetrieveUnrelatedProduct => "RETRIEVING_UNRELATED_PRODUCT_FROM_FURNACE",
            WorkState::AddFuelToFurnace => "ADD_FUEL_TO_FURNACE",
        }
    }

    /// La ligne qui déclare l'état, `AIWorkerState.java:<ligne>`.
    pub fn java_declaration(self) -> &'static str {
        match self {
            WorkState::Base(_) => "crate::ai_build::crafter::CraftState",
            WorkState::FillUpBrewingStand => "AIWorkerState.java:647",
            WorkState::RetrieveBrewed => "AIWorkerState.java:652",
            WorkState::RetrieveUsedFuel => "AIWorkerState.java:657",
            WorkState::AddFuelToBrewingStand => "AIWorkerState.java:662",
            WorkState::HarvestMistletoe => "AIWorkerState.java:667",
            WorkState::HarvestNetherWart => "AIWorkerState.java:672",
            WorkState::FillUpFurnaces => "AIWorkerState.java:305",
            WorkState::RetrieveProductFromFurnace => "AIWorkerState.java:315",
            WorkState::RetrieveUnrelatedProduct => "AIWorkerState.java:320",
            WorkState::AddFuelToFurnace => "AIWorkerState.java:330",
        }
    }

    /// `AIWorkerState.isOkayToEat` : le drapeau que `AbstractAISkeleton.java:126-131`
    /// rend et que `inventoryNeedsDump` lit (`AbstractEntityAIBasic.java:411`).
    ///
    /// Dix-sept de ces états sont déclarés `true` par Java (`:305`, `:315`, `:320`,
    /// `:330`, `:518`, `:523`, `:528`, `:647`, `:652`, `:657`, `:662`, `:667`, `:672`,
    /// plus `IDLE`, `START_WORKING`, `GATHERING_REQUIRED_MATERIALS`) ; un seul ne
    /// l'est pas : `INVENTORY_FULL` (`:23`).
    pub fn okay_to_eat(self) -> bool {
        !matches!(self, WorkState::Base(CraftState::InventoryFull))
    }

    /// L'état commun derrière, `None` pour un état de métier.
    pub fn craft(self) -> Option<CraftState> {
        match self {
            WorkState::Base(state) => Some(state),
            _ => None,
        }
    }
}

impl fmt::Display for WorkState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.java())
    }
}

/// Ce qu'un état commun devient dans cette table.
pub fn craft_state(state: CraftState) -> WorkState {
    WorkState::Base(state)
}

/// Pourquoi une de ces IA ne peut pas avancer, nommé.
///
/// Java n'a pas de tel type : la branche qui ne peut pas continuer fait
/// `return getState()` ou `return IDLE` et la raison reste dans la forme du `if`. Un
/// joueur qui demande « pourquoi mon alchimiste tourne en rond » y a droit, et un test
/// qui compare une suite d'états doit savoir *quelle* branche a tenu. Chaque variante
/// porte la ligne qui la décide.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: une variante par branche de ces trois classes qui s'arrête sans agir
pub enum WorkRefusal {
    /// La scène ne porte pas les faits de ce métier — erreur d'appelant, pas un état
    /// de Java.
    NoFactsForTrade,
    /// Rien à faire : aucune potion à sortir, aucun four à emplir (`:583`, `:884`).
    NothingToDo,
    /// La hutte ne liste aucun alambic (`:894-902`, `:1077-1085`) : Java déclenche
    /// `BAKER_HAS_NO_FURNACES_MESSAGE` (`TranslationConstants`).
    NoBrewingStand,
    /// L'alambic visé n'est plus là, ou n'est plus un alambic (`:740-745`,
    /// `:814-819`, `:904-909`).
    StandGone,
    /// Le citoyen n'est pas encore à la position de travail (`:606-609`, `:747-750`,
    /// `:809-812`, `:961-964`, `:1020-1023`, `:162-165`, `:267-270`).
    NotAtWorkPos,
    /// La poudre est en route : une requête est ouverte ou la hutte en a (`:566`).
    FuelOnItsWay,
    /// Plus de poudre de blaze nulle part (`:601-603`).
    NoBlazePowder,
    /// Pas de cisailles assez bonnes (`:236-239`).
    NoShears,
    /// La hutte ne liste aucune feuille (`:245-248`).
    NoLeaves,
    /// La hutte ne liste aucune terre du nether (`:127-130`).
    NoSoil,
    /// Aucune verrue du nether à replanter (`:197-202`).
    NoNetherWart,
    /// La verrue n'est pas mûre (`:170-174`).
    WartNotRipe,
    /// Les compteurs de l'IA et l'alambic ne s'accordent plus (`:981-988`,
    /// `:1039-1046`).
    BrewingOutOfSync,
    /// La recette ne porte pas l'entrée numéro 1 (la potion, `:924`) ou numéro 0
    /// (l'ingrédient, `:992`). Java lèverait `IndexOutOfBoundsException`.
    MissingRecipeInput,
    /// `getPrimaryOutput().getCount()` vaut zéro et `:774` diviserait par zéro.
    DegenerateOutput,
    /// La hutte ne liste aucun four (`:439-452`) — même message que l'alambic.
    NoFurnace,
    /// La hutte ne liste aucun carburant (`:710-720`) : Java déclenche
    /// `FURNACE_USER_NO_FUEL`.
    NoAllowedFuel,
    /// Au moins un four n'est pas chargé (`:455-458`).
    FurnacesNotLoaded,
    /// Rien à cuire dans l'inventaire (`:353-358`).
    NoSmeltable,
    /// Le compte demandé est déjà couvert par les fours (`:348-351`, `:373-376`).
    NothingPending,
    /// La recette n'a pas l'alambic pour intermédiaire (`:501-504`, `:532-535`).
    NotBrewingRecipe,
}

impl WorkRefusal {
    /// La ligne Java qui décide ce refus.
    pub fn java(self) -> &'static str {
        match self {
            WorkRefusal::NoFactsForTrade => "voir src/ai_work_01/RAPPORT.md",
            WorkRefusal::NothingToDo => "EntityAIWorkAlchemist.java:583,:884",
            WorkRefusal::NoBrewingStand => "EntityAIWorkAlchemist.java:894-902,:1077-1085",
            WorkRefusal::StandGone => "EntityAIWorkAlchemist.java:740-745,:814-819,:904-909",
            WorkRefusal::NotAtWorkPos => "EntityAIWorkAlchemist.java:162-165,:747-750",
            WorkRefusal::FuelOnItsWay => "EntityAIWorkAlchemist.java:566",
            WorkRefusal::NoBlazePowder => "EntityAIWorkAlchemist.java:601-603",
            WorkRefusal::NoShears => "EntityAIWorkAlchemist.java:236-239",
            WorkRefusal::NoLeaves => "EntityAIWorkAlchemist.java:245-248",
            WorkRefusal::NoSoil => "EntityAIWorkAlchemist.java:127-130",
            WorkRefusal::NoNetherWart => "EntityAIWorkAlchemist.java:197-202",
            WorkRefusal::WartNotRipe => "EntityAIWorkAlchemist.java:170-174",
            WorkRefusal::BrewingOutOfSync => "EntityAIWorkAlchemist.java:981-988",
            WorkRefusal::MissingRecipeInput => "EntityAIWorkAlchemist.java:924,:992",
            WorkRefusal::DegenerateOutput => "EntityAIWorkAlchemist.java:774",
            WorkRefusal::NoFurnace => "AbstractEntityAIRequestSmelter.java:439-452",
            WorkRefusal::NoAllowedFuel => "AbstractEntityAIRequestSmelter.java:710-720",
            WorkRefusal::FurnacesNotLoaded => "AbstractEntityAIRequestSmelter.java:455-458",
            WorkRefusal::NoSmeltable => "AbstractEntityAIRequestSmelter.java:353-358",
            WorkRefusal::NothingPending => "AbstractEntityAIRequestSmelter.java:348-351",
            WorkRefusal::NotBrewingRecipe => "EntityAIWorkAlchemist.java:501-504",
        }
    }

    /// Une clé stable pour le message que le joueur lira.
    pub fn key(self) -> &'static str {
        match self {
            WorkRefusal::NoFactsForTrade => "ai_work.refusal.no_facts_for_trade",
            WorkRefusal::NothingToDo => "ai_work.refusal.nothing_to_do",
            WorkRefusal::NoBrewingStand => "ai_work.refusal.no_brewing_stand",
            WorkRefusal::StandGone => "ai_work.refusal.stand_gone",
            WorkRefusal::NotAtWorkPos => "ai_work.refusal.not_at_work_pos",
            WorkRefusal::FuelOnItsWay => "ai_work.refusal.fuel_on_its_way",
            WorkRefusal::NoBlazePowder => "ai_work.refusal.no_blaze_powder",
            WorkRefusal::NoShears => "ai_work.refusal.no_shears",
            WorkRefusal::NoLeaves => "ai_work.refusal.no_leaves",
            WorkRefusal::NoSoil => "ai_work.refusal.no_soil",
            WorkRefusal::NoNetherWart => "ai_work.refusal.no_nether_wart",
            WorkRefusal::WartNotRipe => "ai_work.refusal.wart_not_ripe",
            WorkRefusal::BrewingOutOfSync => "ai_work.refusal.brewing_out_of_sync",
            WorkRefusal::MissingRecipeInput => "ai_work.refusal.missing_recipe_input",
            WorkRefusal::DegenerateOutput => "ai_work.refusal.degenerate_output",
            WorkRefusal::NoFurnace => "ai_work.refusal.no_furnace",
            WorkRefusal::NoAllowedFuel => "ai_work.refusal.no_allowed_fuel",
            WorkRefusal::FurnacesNotLoaded => "ai_work.refusal.furnaces_not_loaded",
            WorkRefusal::NoSmeltable => "ai_work.refusal.no_smeltable",
            WorkRefusal::NothingPending => "ai_work.refusal.nothing_pending",
            WorkRefusal::NotBrewingRecipe => "ai_work.refusal.not_brewing_recipe",
        }
    }
}

impl fmt::Display for WorkRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.key(), self.java())
    }
}

/// Ce qu'une action a décidé, dans les deux moitiés que Java garde séparées : l'état
/// qu'elle rend et la branche qui a refusé.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decision {
    /// L'état rendu ; `None` pour « reste dans l'état courant », comme le `null` de
    /// `NO_CHANGE` (`AbstractEntityAICrafting.java:68`).
    pub next: Option<WorkState>,
    /// Pourquoi elle s'est arrêtée, quand elle s'est arrêtée.
    pub refusal: Option<WorkRefusal>,
}

impl Decision {
    /// Aller à `state`.
    pub fn go(state: WorkState) -> Decision {
        Decision {
            next: Some(state),
            refusal: None,
        }
    }

    /// `return getState()` : aucun changement, aucune plainte.
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
}

/// Ce qu'un tick de ces trois IA a décidé : le but en vigueur, la règle qui l'a choisi
/// et le refus éventuel — les trois réponses de `crate::ai::worker::Outcome`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkOutcome {
    /// Le but en vigueur après ce tick.
    pub state: WorkState,
    /// La transition qui a tourné, s'il y en a une.
    pub fired: Option<Fired<WorkState>>,
    /// Le refus enregistré pendant ce tick.
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

    /// La ligne Java de la règle qui a tourné.
    pub fn why(&self) -> Option<&'static str> {
        self.fired.map(|fired| fired.java)
    }

    /// Si le tick a changé de but.
    pub fn moved(&self) -> bool {
        matches!(self.fired, Some(fired) if fired.from != fired.to)
    }
}

/// Ce que l'IA retient d'un tick à l'autre — **pas** un fait sur le monde.
///
/// Référence Java : `AbstractEntityAIBasic.delay` (`:121`),
/// `EntityAIWorkAlchemist.walkTo` (`:91`), `.fuelPos` (`:81`), `.preFuelState`
/// (`:86`), `AbstractEntityAIRequestSmelter.furnacePos` (`:61`).
///
/// `delay` est celui de la classe de base : `crate::ai::worker::Memory` le porte aussi,
/// mais une `Scene` de ce module ne contient pas cette structure (voir
/// `MANQUANT.md` §1) — l'appelant qui tique les deux tables doit les raccorder.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorkMemory {
    /// `delay` (`AbstractEntityAIBasic.java:121`) : ticks restants avant l'action.
    pub delay: u32,
    /// `walkTo` (`EntityAIWorkAlchemist.java:91`) : la terre ou la feuille visée.
    pub walk_to: Option<crate::ai::Pos>,
    /// `furnacePos` (`AbstractEntityAIRequestSmelter.java:61`).
    pub furnace_pos: Option<crate::ai::Pos>,
    /// `fuelPos` (`EntityAIWorkAlchemist.java:81`).
    pub fuel_pos: Option<crate::ai::Pos>,
    /// `preFuelState` (`EntityAIWorkAlchemist.java:86`) : l'état auquel revenir après
    /// avoir fait le plein de poudre.
    pub pre_fuel_state: Option<WorkState>,
    /// `needsCurrently` (`AbstractEntityAIBasic.java:156`) : la quantité de ce que la
    /// cueillette en cours doit ramener. `None` veut dire « aucune cueillette ».
    pub need: Option<u32>,
}

/// Les faits plus la mémoire : ce qu'une transition lit et écrit.
///
/// [`CraftFacts`] est **la même valeur** que celle du crafter — rien n'en est recopié —
/// donc un appelant qui tique aussi `crate::ai_build::crafter` peut partager la
/// structure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scene {
    /// Les faits communs, lus par les sept états de `crate::ai_build::crafter`.
    pub base: CraftFacts,
    /// Les faits propres au métier.
    pub job: TradeFacts,
    /// Ce que l'IA retient.
    pub memory: WorkMemory,
    /// Le dernier refus, remis à zéro au début de chaque tick.
    pub refusal: Option<WorkRefusal>,
}

impl Scene {
    /// Une scène sur `base` et `job`, mémoire vide.
    pub fn new(base: CraftFacts, job: TradeFacts) -> Scene {
        Scene {
            base,
            job,
            memory: WorkMemory::default(),
            refusal: None,
        }
    }

    /// Les faits de l'alchimiste, si c'en est un.
    pub fn alchemist(&self) -> Option<&super::facts::AlchemistFacts> {
        match &self.job {
            TradeFacts::Alchemist(facts) => Some(facts),
            _ => None,
        }
    }

    /// Les faits du boulanger, si c'en est un.
    pub fn baker(&self) -> Option<&super::facts::BakerFacts> {
        match &self.job {
            TradeFacts::Baker(facts) => Some(facts),
            _ => None,
        }
    }

    /// Note un refus, sans écraser celui qui est déjà là.
    pub fn refuse(&mut self, refusal: WorkRefusal) {
        if self.refusal.is_none() {
            self.refusal = Some(refusal);
        }
    }
}

/// `hasWorkToDo` — celui de la classe de base, sauf pour l'alchimiste.
///
/// `AbstractEntityAICrafting.java:261-264` : une file non vide **et** une tâche
/// lisible. `EntityAIWorkAlchemist.java:1117-1121` répond toujours vrai : il a
/// toujours de quoi brasser ou aller récolter.
pub fn has_work_to_do(scene: &Scene) -> bool {
    match &scene.job {
        TradeFacts::Alchemist(_) => true,             // `:1117-1121`
        _ => crafter::has_work_to_do(&scene.base),    // `:261-264`
    }
}

// ---------------------------------------------------------------- les sept cibles

/// `AbstractEntityAICrafting.java:164` : `new AITarget(IDLE, this::hasWorkToDo, () ->
/// START_WORKING, TICKS_SECOND)`.
fn idle_has_work(scene: &Scene, _: WorkState) -> bool {
    has_work_to_do(scene)
}

/// `:165` : `idle`, les passe-temps. L'alchimiste ne l'atteint jamais puisque son
/// `hasWorkToDo` rend toujours vrai.
fn idle_action(scene: &mut Scene, _: WorkState) -> Option<WorkState> {
    let decision = crafter::idle(&scene.base);
    scene.base.idle_pos = decision.idle_pos;
    None
}

/// `:166` : `decide`, que l'alchimiste remplace (`EntityAIWorkAlchemist.java:307-349`).
fn decide_action(scene: &mut Scene, state: WorkState) -> Option<WorkState> {
    let answer = match &scene.job {
        TradeFacts::Alchemist(facts) => Some(brewing::decide(&scene.base, facts)),
        _ => None,
    };
    let decision = match answer {
        Some(answer) => brewing::apply_decide(scene, answer),
        None => match crafter::decide(&scene.base) {
            crafter::Decide::Idle => Decision::go(craft_state(CraftState::Idle)), // `:273-276`
            crafter::Decide::Walking | crafter::Decide::WaitingForDump => Decision::stay(),
            crafter::Decide::Next(next) => Decision::go(craft_state(next)), // `:290`
        },
    };
    apply(scene, state, decision)
}

/// Ce que `queryItems` doit demander : la recette d'alambic ou la classe de base.
fn query_answer(scene: &Scene) -> Option<brewing::BrewItems> {
    match &scene.job {
        TradeFacts::Alchemist(facts) if facts.brewing_recipe => {
            Some(brewing::check_for_items(&scene.base, facts))
        }
        _ => None,
    }
}

/// `:167` : `queryItems`, que l'alchimiste remplace pour ses recettes d'alambic
/// (`EntityAIWorkAlchemist.java:688-725`).
fn query_action(scene: &mut Scene, state: WorkState) -> Option<WorkState> {
    let answer = query_answer(scene);
    let decision = match answer {
        None => base_query(scene),
        Some(brewing::BrewItems::NoRecipe) => base_query(scene), // `:690-693`
        Some(brewing::BrewItems::Gather { .. }) => {
            Decision::go(craft_state(CraftState::GatheringRequiredMaterials)) // `:711`
        }
        Some(brewing::BrewItems::Craft { cancelled }) => {
            // `:719-720` annule la requête, puis `:724` rend `CRAFT` quand même.
            if cancelled {
                Decision {
                    next: Some(craft_state(CraftState::Craft)),
                    refusal: Some(WorkRefusal::BrewingOutOfSync),
                }
            } else {
                Decision::go(craft_state(CraftState::Craft))
            }
        }
    };
    apply(scene, state, decision)
}

/// `checkForItems` de la classe de base (`AbstractEntityAICrafting.java:462-504`).
fn base_query(scene: &mut Scene) -> Decision {
    match crafter::check_for_items(&scene.base) {
        crafter::Items::NoRecipe => Decision::go(craft_state(CraftState::StartWorking)), // `:450`
        crafter::Items::Gather { .. } => {
            Decision::go(craft_state(CraftState::GatheringRequiredMaterials)) // `:495`
        }
        crafter::Items::ReRead => {
            scene.base.has_recipe = false; // `:497-498`
            scene.base.has_request = false;
            Decision::go(craft_state(CraftState::GetRecipe)) // `:499`
        }
        crafter::Items::Craft => Decision::go(craft_state(CraftState::Craft)), // `:503`
    }
}

/// `:168` : `getRecipe`, que l'alchimiste remplace (`EntityAIWorkAlchemist.java:
/// 395-435`).
fn recipe_action(scene: &mut Scene, state: WorkState) -> Option<WorkState> {
    let answer = match &scene.job {
        TradeFacts::Alchemist(facts) => Some(brewing::get_recipe(&scene.base, facts)),
        _ => None,
    };
    let decision = match answer {
        None => base_get_recipe(scene),
        Some(brewing::BrewRecipe::NoTask) => {
            Decision::go(craft_state(CraftState::StartWorking)) // `:399-403`
        }
        Some(brewing::BrewRecipe::Retrieve { at, max }) => {
            scene.base.max_crafting_count = max; // `:405`
            scene.base.has_request = true; // `:410`
            scene.memory.walk_to = Some(at); // `:411`
            Decision::go(WorkState::RetrieveBrewed) // `:412`
        }
        Some(brewing::BrewRecipe::Brew) => Decision::go(craft_state(CraftState::Craft)), // `:415-426`
        Some(brewing::BrewRecipe::Base) => base_get_recipe(scene), // `:434`
    };
    apply(scene, state, decision)
}

/// `:169` : `craft`, que l'alchimiste remplace (`:1059-1114`) et que le boulanger
/// étend par `executeCraftingAction` (`AbstractEntityAIRequestSmelter.java:427-503`).
fn craft_action(scene: &mut Scene, state: WorkState) -> Option<WorkState> {
    let answer = match &scene.job {
        TradeFacts::Alchemist(facts) => Some(brewing::craft(&scene.base, facts)),
        _ => None,
    };
    let Some(answer) = answer else {
        return base_craft_action(scene, state);
    };
    let decision = match answer {
        // `:1074` : une recette qui n'est pas un brassage repart à la classe de base.
        brewing::BrewCraft::Delegated => return base_craft_action(scene, state),
        brewing::BrewCraft::Walking { delay } => {
            scene.memory.delay = delay; // `:1063`
            Decision::refuse(WorkRefusal::NotAtWorkPos) // `:1064`
        }
        brewing::BrewCraft::NoStand => {
            scene.memory.delay = STANDARD_DELAY; // `:1083`
            Decision {
                next: Some(craft_state(CraftState::StartWorking)), // `:1084`
                refusal: Some(WorkRefusal::NoBrewingStand),
            }
        }
        brewing::BrewCraft::Retrieve { at } => {
            scene.memory.walk_to = Some(at); // `:1090`
            Decision::go(WorkState::RetrieveBrewed) // `:1091`
        }
        brewing::BrewCraft::Finished => Decision::go(craft_state(CraftState::InventoryFull)), // `:1101`
        brewing::BrewCraft::Cancelled => Decision::go(craft_state(CraftState::StartWorking)), // `:1110`
        brewing::BrewCraft::Able(able) => brewing::apply_able(scene, able),
    };
    apply(scene, state, decision)
}

/// `getRecipe` de la classe de base (`AbstractEntityAICrafting.java:324-422`), avec
/// les écritures que Java fait au passage (`:369-373`, `:419-420`).
fn base_get_recipe(scene: &mut Scene) -> Decision {
    match crafter::get_recipe(&scene.base) {
        crafter::Recipe::Ready {
            max_crafting_count,
            craft_counter,
        } => {
            scene.base.max_crafting_count = max_crafting_count; // `:419`
            scene.base.craft_counter = craft_counter; // `:420`
            scene.base.has_recipe = true;
            Decision::go(craft_state(CraftState::QueryItems)) // `:421`
        }
        crafter::Recipe::Dump => {
            scene.base.dumped = true; // `:350`
            scene.base.has_recipe = false; // `:351`
            Decision::go(craft_state(CraftState::InventoryFull)) // `:352`
        }
        crafter::Recipe::DegenerateOutput => {
            Decision::refuse(WorkRefusal::DegenerateOutput)
        }
        crafter::Recipe::NoTask
        | crafter::Recipe::NoModule
        | crafter::Recipe::NoRecipe
        | crafter::Recipe::ToolMissing
        | crafter::Recipe::ShortOfIngredient { .. } => {
            Decision::go(craft_state(CraftState::StartWorking)) // `:330`, `:338`, `:345`, `:362`, `:408`
        }
    }
}

/// `executeCraftingAction` de la classe de base (`:579-637`).
pub fn base_execute(scene: &mut Scene) -> Decision {
    match crafter::execute(&scene.base) {
        crafter::Execution::RecipeFailed => Decision::go(craft_state(CraftState::StartWorking)), // `:588`
        crafter::Execution::Finalized => Decision::go(craft_state(CraftState::InventoryFull)),   // `:646`
        crafter::Execution::ToolBroke => Decision::go(craft_state(CraftState::StartWorking)), // `:628`
        crafter::Execution::Again => {
            scene.base.progress = 0; // `:635`
            Decision::go(craft_state(CraftState::GetRecipe)) // `:636`
        }
    }
}

/// `craft` de la classe de base (`:511-572`), puis `executeCraftingAction`.
fn base_craft_action(scene: &mut Scene, state: WorkState) -> Option<WorkState> {
    let is_baker = matches!(&scene.job, TradeFacts::Baker(_));
    let decision = match crafter::craft(&scene.base) {
        crafter::Craft::NoWork | crafter::Craft::Cancelled | crafter::Craft::OutOfItems => {
            Decision::go(craft_state(CraftState::StartWorking)) // `:513-516`, `:553-559`, `:568`
        }
        crafter::Craft::NeedRecipe => Decision::go(craft_state(CraftState::GetRecipe)), // `:520`
        crafter::Craft::Walking | crafter::Craft::Hitting | crafter::Craft::BadSkill => {
            Decision::stay() // `:525`, `:571`
        }
        // `AbstractEntityAIRequestSmelter.java:427` remplace l'exécution : c'est le
        // four qui décide, et il délègue à la classe de base si la recette n'est pas
        // une cuisson (`:434-437`).
        crafter::Craft::Execute if is_baker => {
            match furnace::execute_crafting_action(scene) {
                Some(next) => Decision::go(next),
                None => Decision::stay(),
            }
        }
        crafter::Craft::Execute => base_execute(scene),
    };
    apply(scene, state, decision)
}

/// Écrit le refus dans la scène et rend le successeur.
fn apply(scene: &mut Scene, state: WorkState, decision: Decision) -> Option<WorkState> {
    if let Some(refusal) = decision.refusal {
        scene.refuse(refusal);
    }
    decision.next.or(Some(state))
}

/// Les sept cibles de `AbstractEntityAICrafting.java:164-169` plus les deux sorties
/// que la classe de base ajoute (`AbstractEntityAIBasic.java:1213-1222`,
/// `AbstractEntityAICrafting.java:436-439`).
pub fn register_shared(machine: &mut Machine<WorkState, Scene>) {
    // :164 — `IDLE` part dès qu'il y a du travail.
    machine.add(Transition::target_when(
        craft_state(CraftState::Idle),
        "hasWorkToDo",
        "AbstractEntityAICrafting.java:164",
        TICKS_SECOND,
        idle_has_work,
        Action::Fixed(Some(craft_state(CraftState::StartWorking))),
    ));
    // :165 — `idle`, sinon.
    machine.add(Transition::target(
        craft_state(CraftState::Idle),
        "idle",
        "AbstractEntityAICrafting.java:165",
        TICKS_SECOND,
        Action::Run(idle_action),
    ));
    // :166 — `decide`, au rythme de `STANDARD_DELAY`.
    machine.add(Transition::target(
        craft_state(CraftState::StartWorking),
        "decide",
        "AbstractEntityAICrafting.java:166",
        STANDARD_DELAY,
        Action::Run(decide_action),
    ));
    // :167 — `queryItems`.
    machine.add(Transition::target(
        craft_state(CraftState::QueryItems),
        "queryItems",
        "AbstractEntityAICrafting.java:167",
        STANDARD_DELAY,
        Action::Run(query_action),
    ));
    // :168 — `getRecipe`.
    machine.add(Transition::target(
        craft_state(CraftState::GetRecipe),
        "getRecipe",
        "AbstractEntityAICrafting.java:168",
        STANDARD_DELAY,
        Action::Run(recipe_action),
    ));
    // :169 — `craft`, au rythme de `HIT_DELAY` (`work_time::CRAFT_HIT_DELAY`).
    machine.add(Transition::target(
        craft_state(CraftState::Craft),
        "craft",
        "AbstractEntityAICrafting.java:169",
        crate::ai_build::work_time::CRAFT_HIT_DELAY,
        Action::Run(craft_action),
    ));
    // Le vidage de l'inventaire rend la main à `IDLE`
    // (`AbstractEntityAIBasic.java:1213-1222`).
    machine.add(Transition::goto(
        craft_state(CraftState::InventoryFull),
        craft_state(CraftState::Idle),
        "afterDump",
        "AbstractEntityAIBasic.java:1213-1222",
        STANDARD_DELAY,
    ));
    // `getStateAfterPickUp()` (`AbstractEntityAICrafting.java:436-439`).
    machine.add(Transition::goto(
        craft_state(CraftState::GatheringRequiredMaterials),
        craft_state(CraftState::StartWorking),
        "getStateAfterPickUp",
        "AbstractEntityAICrafting.java:436-439",
        STANDARD_DELAY,
    ));
}

/// Une machine qui démarre dans `IDLE` au rythme de l'IA d'entité
/// (`AbstractEntityCitizen.java:64`).
pub fn machine() -> Machine<WorkState, Scene> {
    Machine::new(craft_state(CraftState::Idle), ENTITY_AI_TICKRATE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_build::crafter::{Craft, Input};

    fn busy() -> CraftFacts {
        CraftFacts {
            task_queue: 1,
            current_task: true,
            ..CraftFacts::default()
        }
    }

    fn baker_scene() -> Scene {
        Scene::new(
            busy(),
            TradeFacts::Baker(super::super::facts::BakerFacts::default()),
        )
    }

    /// `AbstractEntityAICrafting.java:261-264` contre
    /// `EntityAIWorkAlchemist.java:1117-1121` : deux réponses différentes à la même
    /// question, et c'est la seule différence de condition entre les trois tables.
    #[test]
    fn the_alchemist_always_has_work_and_the_others_do_not() {
        let idle = CraftFacts::default();
        let baker_idle = Scene::new(idle.clone(), TradeFacts::Blacksmith);
        assert!(!has_work_to_do(&baker_idle));
        let alchemist_idle = Scene::new(
            idle,
            TradeFacts::Alchemist(super::super::facts::AlchemistFacts::default()),
        );
        assert!(has_work_to_do(&alchemist_idle));
    }

    /// Le contrat de non-duplication : là où la table commune répond, le crafter
    /// répond pareil. Si l'un des deux dérive, ce test tombe.
    #[test]
    fn the_shared_table_answers_like_the_crafter() {
        let cases = [
            busy(),
            CraftFacts {
                at_building: false,
                ..busy()
            },
            CraftFacts {
                actions_done: 1,
                ..busy()
            },
            CraftFacts {
                has_recipe: true,
                has_request: true,
                ..busy()
            },
        ];
        for facts in cases {
            let mut scene = Scene::new(facts.clone(), TradeFacts::Blacksmith);
            let decision = decide_action(&mut scene, craft_state(CraftState::StartWorking));
            let expected = match crafter::decide(&facts) {
                crafter::Decide::Idle => craft_state(CraftState::Idle),
                crafter::Decide::Walking | crafter::Decide::WaitingForDump => {
                    craft_state(CraftState::StartWorking)
                }
                crafter::Decide::Next(next) => craft_state(next),
            };
            assert_eq!(decision, Some(expected));
        }
    }

    /// `:511-572` : les cinq sorties de `craft` et l'exécution, une par une.
    #[test]
    fn the_shared_craft_follows_javas_five_exits() {
        let mut scene = baker_scene();
        scene.base = CraftFacts {
            has_recipe: false,
            ..busy()
        };
        assert_eq!(
            craft_action(&mut scene, craft_state(CraftState::Craft)),
            Some(craft_state(CraftState::StartWorking))
        );

        // `:523-526` : avec une recette et une requête, mais pas encore à la
        // position de travail, le tick reste sur `CRAFT`.
        let mut walking = baker_scene();
        walking.base.has_recipe = true;
        walking.base.has_request = true;
        walking.base.at_work_position = false;
        assert_eq!(
            craft_action(&mut walking, craft_state(CraftState::Craft)),
            Some(craft_state(CraftState::Craft))
        );

        // Sans requête on relit la recette (`:518-521`).
        let mut need = baker_scene();
        need.base.has_recipe = true;
        need.base.has_request = false;
        assert_eq!(
            craft_action(&mut need, craft_state(CraftState::Craft)),
            Some(craft_state(CraftState::GetRecipe))
        );

        // Une exécution qui n'est pas une cuisson pour un boulanger délègue à la
        // classe de base (`AbstractEntityAIRequestSmelter.java:434-437`).
        let mut run = baker_scene();
        run.base = CraftFacts {
            has_recipe: true,
            has_request: true,
            progress: 30,
            max_crafting_count: 2,
            inputs: vec![Input::item(1, 0, 8)],
            ..busy()
        };
        assert_eq!(crafter::craft(&run.base), Craft::Execute);
        assert_eq!(
            craft_action(&mut run, craft_state(CraftState::Craft)),
            Some(craft_state(CraftState::GetRecipe)),
            "`Execution::Again` remet le progrès à zéro et relit la recette (`:635`)"
        );
    }

    /// Les dix-sept états ont un nom Java, et `INVENTORY_FULL` est le seul à ne pas
    /// pouvoir être interrompu (`AIWorkerState.java:23`).
    #[test]
    fn the_seventeen_states_are_named_once_and_only_the_dump_is_uninterruptible() {
        let mut names: Vec<&str> = WorkState::ALL.iter().map(|state| state.java()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 17);
        assert!(!craft_state(CraftState::InventoryFull).okay_to_eat());
        for state in WorkState::ALL {
            if state != craft_state(CraftState::InventoryFull) {
                assert!(state.okay_to_eat(), "{}", state.java());
            }
        }
        assert_eq!(
            WorkState::RetrieveBrewed.java_declaration(),
            "AIWorkerState.java:652",
            "la faute de frappe de Java est recopiée"
        );
    }

    /// `Machine::tick` exige que chaque état atteignable puisse être quitté, sinon il
    /// tombe dans le piège de `BasicStateMachine.java:220-228`.
    #[test]
    fn every_shared_state_can_be_left() {
        let mut machine = machine();
        register_shared(&mut machine);
        for state in [
            craft_state(CraftState::Idle),
            craft_state(CraftState::StartWorking),
            craft_state(CraftState::QueryItems),
            craft_state(CraftState::GetRecipe),
            craft_state(CraftState::Craft),
            craft_state(CraftState::InventoryFull),
            craft_state(CraftState::GatheringRequiredMaterials),
        ] {
            assert!(machine.can_leave(state), "{}", state.java());
        }
    }

    /// Un refus porte sa ligne Java et sa clé de message ; aucun n'est vide.
    #[test]
    fn every_refusal_cites_a_line_and_a_key() {
        for refusal in [
            WorkRefusal::NoFactsForTrade,
            WorkRefusal::NothingToDo,
            WorkRefusal::NoBrewingStand,
            WorkRefusal::StandGone,
            WorkRefusal::NotAtWorkPos,
            WorkRefusal::FuelOnItsWay,
            WorkRefusal::NoBlazePowder,
            WorkRefusal::NoShears,
            WorkRefusal::NoLeaves,
            WorkRefusal::NoSoil,
            WorkRefusal::NoNetherWart,
            WorkRefusal::WartNotRipe,
            WorkRefusal::BrewingOutOfSync,
            WorkRefusal::MissingRecipeInput,
            WorkRefusal::DegenerateOutput,
            WorkRefusal::NoFurnace,
            WorkRefusal::NoAllowedFuel,
            WorkRefusal::FurnacesNotLoaded,
            WorkRefusal::NoSmeltable,
            WorkRefusal::NothingPending,
            WorkRefusal::NotBrewingRecipe,
        ] {
            assert!(!refusal.java().is_empty());
            assert!(refusal.key().starts_with("ai_work.refusal."));
        }
    }
}
