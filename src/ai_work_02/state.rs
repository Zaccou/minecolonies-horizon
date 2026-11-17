//! Les états que ces trois métiers ajoutent à `AIWorkerState`, les refus nommés, et
//! la table qui les relie à celle de l'atelier.
//!
//! Référence Java : `api/entity/ai/statemachine/states/AIWorkerState.java` — un seul
//! énuméré pour les 127 états du mod. Le port garde cette forme : un énuméré,
//! [`WorkState`], avec les cinq états que ces trois métiers possèdent et les sept de
//! l'atelier qu'ils traversent.
//!
//! **Pourquoi `Base(CraftState)`.** Les sept états de l'atelier (`IDLE`,
//! `START_WORKING`, `QUERY_ITEMS`, `GET_RECIPE`, `CRAFT`, `INVENTORY_FULL`,
//! `GATHERING_REQUIRED_MATERIALS`) sont déjà nommés par
//! [`crate::ai_build::crafter::CraftState`], et cette énumération appartient à un
//! autre dossier : elle n'est pas réécrite ici. Un état commun est donc
//! [`WorkState::Base`] enveloppant le `CraftState` que l'autre module connaît.
//!
//! **Pourquoi la table est réécrite quand même.** `Machine<S, F>` est monophorme en
//! `S` **et** en `F` : une machine ne peut pas *contenir* celle du crafter, qui tique
//! sur `CraftFacts`. Ce fichier redéclare donc les sept cibles de
//! `AbstractEntityAICrafting.java:164-169` sur [`Scene`], mais **chaque action
//! appelle** `crafter::decide`, `crafter::check_for_items`, `crafter::get_recipe`,
//! `crafter::craft` et `crafter::execute` : la règle n'a qu'un domicile, seule la
//! cible est dite deux fois. Le test `the_shared_table_answers_like_the_crafter`
//! tient ce contrat.
//!
//! **La dispatch des méthodes virtuelles.** En Java, `decide`, `craft` et
//! `executeCraftingAction` sont surchargées par le concasseur
//! (`EntityAIWorkCrusher.java:62-88`, `:188-249`) et par le four
//! (`AbstractEntityAIRequestSmelter.java:426-500`). La liste de cibles, elle, est
//! commune : la cible `START_WORKING → decide` est celle du parent. Le port garde une
//! seule cible et choisit l'implémentation **sur le métier** ([`TradeJob`]), au même
//! endroit où Java choisirait la classe — c'est écrit dans `decide_action`,
//! `craft_action` et `execute_crafting_action`, une ligne chacun.

use core::fmt;

use crate::ai::{Action, Fired, Machine, Pos, Transition, ENTITY_AI_TICKRATE};
use crate::ai::worker::STANDARD_DELAY;
use crate::ai_build::crafter::{self, CraftFacts, CraftState};
use crate::ai_build::materials::NeedKey;

use super::crusher;
use super::facts::{Recipe, TradeJob};
use super::smelter;
use super::TICKS_SECOND;

/// Un état d'une des trois tables : un état de l'atelier rendu à `crate::ai_build`,
/// ou un état que ce module nomme.
///
/// Java référence `AIWorkerState.java` ; la ligne entre parenthèses est la
/// déclaration de la constante, et c'est elle que [`WorkState::java_declaration`]
/// rend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
// exhaustive: les sept états de l'atelier plus les cinq que ces métiers possèdent
pub enum WorkState {
    /// Un état de [`CraftState`] — `IDLE`, `START_WORKING`, `QUERY_ITEMS`,
    /// `GET_RECIPE`, `CRAFT`, `INVENTORY_FULL`, `GATHERING_REQUIRED_MATERIALS`.
    Base(CraftState),
    /// `FILL_UP_FURNACES` (`AIWorkerState.java:305`) — remplir un four.
    FillUpFurnaces,
    /// `RETRIEVING_END_PRODUCT_FROM_FURNACE` (`:315`) — sortir la sortie d'un four.
    RetrievingEndProduct,
    /// `RETRIEVING_UNRELATED_PRODUCT_FROM_FURNACE` (`:320`) — sortir un intrus.
    RetrievingUnrelatedProduct,
    /// `ADD_FUEL_TO_FURNACE` (`:330`) — ravitailler un four.
    AddFuelToFurnace,
    /// `CRUSH` (`:537`) — concasser, l'état propre au concasseur.
    Crush,
}

impl WorkState {
    /// Tous les états que ces trois tables peuvent nommer.
    pub const ALL: [WorkState; 12] = [
        WorkState::Base(CraftState::Idle),
        WorkState::Base(CraftState::StartWorking),
        WorkState::Base(CraftState::QueryItems),
        WorkState::Base(CraftState::GetRecipe),
        WorkState::Base(CraftState::Craft),
        WorkState::Base(CraftState::InventoryFull),
        WorkState::Base(CraftState::GatheringRequiredMaterials),
        WorkState::FillUpFurnaces,
        WorkState::RetrievingEndProduct,
        WorkState::RetrievingUnrelatedProduct,
        WorkState::AddFuelToFurnace,
        WorkState::Crush,
    ];

    /// Un état de l'atelier.
    pub fn base(state: CraftState) -> WorkState {
        WorkState::Base(state)
    }

    /// L'état de l'atelier sous-jacent, quand il y en a un.
    pub fn craft(self) -> Option<CraftState> {
        match self {
            WorkState::Base(state) => Some(state),
            _ => None,
        }
    }

    /// Java's identifier, tel qu'un historique d'états ou l'étiquette du citoyen
    /// l'affiche (`AbstractEntityAIBasic.java:475`).
    pub fn java(self) -> &'static str {
        match self {
            WorkState::Base(state) => state.java_name(),
            WorkState::FillUpFurnaces => "FILL_UP_FURNACES",
            WorkState::RetrievingEndProduct => "RETRIEVING_END_PRODUCT_FROM_FURNACE",
            WorkState::RetrievingUnrelatedProduct => "RETRIEVING_UNRELATED_PRODUCT_FROM_FURNACE",
            WorkState::AddFuelToFurnace => "ADD_FUEL_TO_FURNACE",
            WorkState::Crush => "CRUSH",
        }
    }

    /// La ligne qui déclare cet état, `AIWorkerState.java:<ligne>`.
    pub fn java_declaration(self) -> &'static str {
        match self {
            WorkState::Base(_) => "AIWorkerState.java (un état de l'atelier, voir CraftState)",
            WorkState::FillUpFurnaces => "AIWorkerState.java:305",
            WorkState::RetrievingEndProduct => "AIWorkerState.java:315",
            WorkState::RetrievingUnrelatedProduct => "AIWorkerState.java:320",
            WorkState::AddFuelToFurnace => "AIWorkerState.java:330",
            WorkState::Crush => "AIWorkerState.java:537",
        }
    }

    /// Le drapeau `isOkayToEat` de Java, que `AbstractAISkeleton.canBeInterrupted`
    /// lit (`AbstractAISkeleton.java:126-131`) : les cinq états de ce module sont
    /// tous `true` dans Java (`:305`, `:315`, `:320`, `:330`, `:537`).
    ///
    /// Les sept états de l'atelier sont relus ici parce que
    /// `crate::ai_build::crafter::CraftState` ne porte pas ce drapeau : sur les sept,
    /// un seul vaut `false` dans Java, `INVENTORY_FULL` (`AIWorkerState.java:23`).
    pub fn okay_to_eat(self) -> bool {
        match self {
            // `AIWorkerState.java:15`, `:31`, `:518`, `:523`, `:528`, `:310`
            WorkState::Base(state) => !matches!(state, CraftState::InventoryFull),
            // `AIWorkerState.java:305`, `:315`, `:320`, `:330`, `:537`
            WorkState::FillUpFurnaces
            | WorkState::RetrievingEndProduct
            | WorkState::RetrievingUnrelatedProduct
            | WorkState::AddFuelToFurnace
            | WorkState::Crush => true,
        }
    }

    /// Cet état appartient-il à la table des fours (cuisinier, teinturier) ?
    pub fn is_furnace_state(self) -> bool {
        matches!(
            self,
            WorkState::FillUpFurnaces
                | WorkState::RetrievingEndProduct
                | WorkState::RetrievingUnrelatedProduct
                | WorkState::AddFuelToFurnace
        )
    }
}

impl fmt::Display for WorkState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.java())
    }
}

/// L'item d'un besoin, quand il n'est pas une simple clé.
///
/// Java référence `needsCurrently = new Tuple<>(predicate, count)`
/// (`AbstractEntityAICrafting.java:494`, `AbstractEntityAIRequestSmelter.java:185`,
/// `:355`) : un prédicat, pas une clé. Un prédicat ne se sérialise pas, donc le port
/// dit *quoi* il porte — une clé, une liste de carburants, ou l'entrée d'indice `n`
/// de la recette en cours, que l'appelant résout sur la recette qu'il a lui-même
/// fournie.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: les trois prédicats que ces métiers construisent
pub enum NeedItem {
    /// Un stack nommé (`AbstractEntityAIRequestSmelter.java:340`).
    Stack(NeedKey),
    /// La liste de carburants autorisés (`:185`, `:734-737`).
    AnyFuel(Vec<NeedKey>),
    /// L'entrée d'indice `input` de `currentRecipeStorage.getCleanedInput()`
    /// (`AbstractEntityAICrafting.java:471`, `:494`).
    Input(usize),
}

/// Ce que l'IA réclame : `needsCurrently` — l'item et le compte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Need {
    /// Le prédicat.
    pub item: NeedItem,
    /// Le compte demandé.
    pub count: u32,
}

impl Need {
    /// Une demande sur l'entrée d'une recette.
    pub fn input(index: usize, count: u32) -> Need {
        Need {
            item: NeedItem::Input(index),
            count,
        }
    }

    /// Une demande de carburant (`:185`).
    pub fn fuel(fuels: Vec<NeedKey>, count: u32) -> Need {
        Need {
            item: NeedItem::AnyFuel(fuels),
            count,
        }
    }
}

/// Ce qu'un transfert d'inventaire doit faire : `InventoryUtils
/// .transferXInItemHandlerIntoSlotInItemHandler(...)` (`AbstractEntityAIRequestSmelter
/// .java:413-418`) et `transferXOfFirstSlotInItemHandlerWithIntoInItemHandler(...)`
/// (`:210-212`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transfer {
    /// Ce qu'il faut prendre.
    pub item: NeedItem,
    /// Combien.
    pub count: u32,
    /// Le slot du four qui reçoit : `SMELTABLE_SLOT` (`:418`) ou `FUEL_SLOT`
    /// (`:212`).
    pub slot: usize,
}

/// Pourquoi un métier s'est arrêté. Chaque variante nomme la ligne qui la produit.
///
/// Java n'a pas de refus : il a des `return state` nus, des interactions bloquantes
/// et des demandes au système de requêtes. Le port nomme le cas, pour qu'un test
/// puisse l'affirmer et qu'un journal puisse l'écrire.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: ce port refuse pour ces raisons, et pas d'autres
pub enum TradeRefusal {
    /// Aucune tâche et plus de quota : `hasWorkToDo` est faux
    /// (`EntityAIWorkCrusher.java:91-94`).
    NothingToDo,
    /// Le métier a reçu les faits d'un autre (un cuisinier sans fours).
    NoFactsForTrade,
    /// Le module `FURNACE` est vide : `BAKER_HAS_NO_FURNACES_MESSAGE`
    /// (`AbstractEntityAIRequestSmelter.java:439-448`).
    NoFurnaces,
    /// Un bloc du module n'est plus un four : il est retiré du module (`:595-599`).
    StaleFurnace,
    /// Un chunk de four n'est pas chargé (`:450-453`, `:631-641`).
    FurnacesNotLoaded,
    /// `FURNACE_USER_NO_FUEL` : la liste autorisée est vide (`:455-463`, `:713-718`).
    NoFuel,
    /// Tous les fours utilisables tournent déjà (`:486-491`).
    FurnacesBusy,
    /// Le slot de sortie est vide (`:234-239`).
    NoOutputToRetrieve,
    /// Aucune recette en cours (`:429-432`).
    NoRecipe,
    /// Le mode du concasseur ne donne aucune recette (`EntityAIWorkCrusher.java:121-124`).
    NoModeRecipe,
    /// `currentDailyQuantity >= dailyLimit` (`EntityAIWorkCrusher.java:68`, `:121`).
    DailyLimitReached,
    /// `requestQty <= 0` (`EntityAIWorkCrusher.java:158-161`).
    RequestQuantityZero,
    /// L'outil s'est cassé pendant le coup (`AbstractEntityAICrafting.java:621-631`).
    ToolBroken,
    /// `finalizeCraftingTask` a répondu `INVENTORY_FULL` (`:636-646`).
    InventoryFull,
    /// Une entrée de la recette ne couvre pas le minimum demandé (`:403-409`).
    ShortOfIngredient {
        /// L'indice de l'entrée dans `getCleanedInput()`.
        input: usize,
        /// Ce que le bâtiment et le citoyen ont.
        available: u32,
        /// Ce qu'il faudrait.
        remaining: u32,
    },
    /// La compétence de vitesse est négative : le nombre de coups n'a pas de sens
    /// (`:734-738`).
    BadSkill {
        /// Le niveau lu.
        level: i32,
    },
    /// La tâche a été annulée ou a échoué pendant le coup
    /// (`AbstractEntityAICrafting.java:553-559`).
    RequestCancelled,
}

impl fmt::Display for TradeRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TradeRefusal::NothingToDo => f.write_str("aucune tâche et plus de quota"),
            TradeRefusal::NoFactsForTrade => f.write_str("les faits d'un autre métier"),
            TradeRefusal::NoFurnaces => f.write_str("aucun four dans le module"),
            TradeRefusal::StaleFurnace => f.write_str("un bloc du module n'est plus un four"),
            TradeRefusal::FurnacesNotLoaded => f.write_str("un chunk de four est déchargé"),
            TradeRefusal::NoFuel => f.write_str("aucun carburant autorisé"),
            TradeRefusal::FurnacesBusy => f.write_str("tous les fours utilisables tournent"),
            TradeRefusal::NoOutputToRetrieve => f.write_str("rien à sortir du four"),
            TradeRefusal::NoRecipe => f.write_str("aucune recette en cours"),
            TradeRefusal::NoModeRecipe => f.write_str("le mode du bâtiment ne donne aucune recette"),
            TradeRefusal::DailyLimitReached => f.write_str("le quota du jour est atteint"),
            TradeRefusal::RequestQuantityZero => f.write_str("la demande calculée est nulle"),
            TradeRefusal::ToolBroken => f.write_str("l'outil s'est cassé"),
            TradeRefusal::InventoryFull => f.write_str("l'inventaire doit être vidé"),
            TradeRefusal::ShortOfIngredient {
                input,
                available,
                remaining,
            } => write!(
                f,
                "l'entrée {input} a {available} pour un minimum de {remaining}"
            ),
            TradeRefusal::BadSkill { level } => {
                write!(f, "compétence de vitesse {level} négative")
            }
            TradeRefusal::RequestCancelled => f.write_str("la tâche a été annulée"),
        }
    }
}

/// Ce qu'une règle a décidé : l'état qu'elle rend et le refus qu'elle nomme, les deux
/// moitiés que Java sépare entre un `return state` et une interaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision {
    /// L'état rendu, `None` pour `return getState()`.
    pub next: Option<WorkState>,
    /// Pourquoi elle s'est arrêtée, quand elle le nomme.
    pub refusal: Option<TradeRefusal>,
}

impl Decision {
    /// Aller dans `state`.
    pub fn go(state: WorkState) -> Decision {
        Decision {
            next: Some(state),
            refusal: None,
        }
    }

    /// `return getState()`.
    pub fn stay() -> Decision {
        Decision {
            next: None,
            refusal: None,
        }
    }

    /// `return getState()` avec la raison.
    pub fn refuse(refusal: TradeRefusal) -> Decision {
        Decision {
            next: None,
            refusal: Some(refusal),
        }
    }

    /// Aller dans `state` et dire pourquoi.
    pub fn go_after(state: WorkState, refusal: TradeRefusal) -> Decision {
        Decision {
            next: Some(state),
            refusal: Some(refusal),
        }
    }

    /// Ajouter une raison à une décision qui n'en a pas.
    pub fn because(mut self, refusal: TradeRefusal) -> Decision {
        if self.refusal.is_none() {
            self.refusal = Some(refusal);
        }
        self
    }

    /// La forme qu'une `Action::Run` attend : `Some(state)` pour un état rendu, et
    /// `Some(current)` pour un `return getState()` — Java rend l'état courant, ce qui
    /// arrête le tick sans le déplacer (`BasicStateMachine.java:199-236`).
    pub fn into_option(self, current: WorkState) -> Option<WorkState> {
        Some(self.next.unwrap_or(current))
    }
}

/// Ce qu'un tick a décidé : l'état en vigueur, la transition qui a tiré, le refus
/// nommé et le délai demandé — les quatre réponses que Java éparpille entre
/// `getState()`, le journal du `BasicStateMachine` et `setDelay`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// L'état après le tick.
    pub state: WorkState,
    /// La transition qui a tiré, s'il y en a eu une.
    pub fired: Option<Fired<WorkState>>,
    /// Le refus enregistré pendant ce tick.
    pub refusal: Option<TradeRefusal>,
    /// `setDelay(ticks)` demandé par la règle, s'il y en a eu un.
    pub delay: Option<u32>,
}

impl Outcome {
    /// Le but en vigueur.
    pub fn goal(&self) -> WorkState {
        self.state
    }

    /// Pourquoi le tick n'a pas avancé, s'il le dit.
    pub fn why_not(&self) -> Option<&TradeRefusal> {
        self.refusal.as_ref()
    }

    /// L'état a-t-il changé ?
    pub fn moved(&self) -> bool {
        matches!(&self.fired, Some(fired) if fired.from != fired.to)
    }

    /// La machine est-elle tombée dans un piège (`BasicStateMachine.java:220-228`) ?
    pub fn faulted(&self) -> bool {
        matches!(&self.fired, Some(fired) if fired.reset)
    }

    /// La ligne Java de la règle qui a tiré.
    pub fn why(&self) -> Option<&'static str> {
        self.fired.as_ref().map(|fired| fired.java)
    }
}

/// La mémoire de l'IA : exactement les champs que Java garde entre deux ticks.
///
/// Référence Java : `AbstractEntityAICrafting`'s `currentRequest` (`:93`),
/// `currentRecipeStorage` (`:98`), `dumped` (`:108`), `idlePos` (`:113`) ; les six
/// compteurs du travail (`job.getProgress()`, `getMaxCraftingCount()`,
/// `getCraftCounter()`, `getActionsDone()`, `getSecondaryOutputs()`) ; et les trois
/// champs du four : `furnacePos` (`AbstractEntityAIRequestSmelter.java:61`) et
/// `needsCurrently` (`AbstractEntityAIBasic`).
///
/// **Ce n'est pas un fait.** Une mémoire est ce que l'IA a décidé ; un fait est ce
/// qu'elle a observé. Le `Recipe` ci-dessous vient d'un fait ([`Scene::available`]),
/// mais c'est bien la mémoire qui dit quelle recette est *en cours*.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TradeMemory {
    /// `currentRecipeStorage` (`:98`).
    pub recipe: Option<Recipe>,
    /// `currentRequest != null` (`:93`).
    pub request_open: bool,
    /// `currentRequest.getState() == CANCELLED || FAILED` (`:553`).
    pub request_cancelled: bool,
    /// `job.getProgress()` (`:528`, `:561`).
    pub progress: u32,
    /// `job.getMaxCraftingCount()` (`:419`, `:609`).
    pub max_crafting_count: u32,
    /// `job.getCraftCounter()` (`:420`, `:603`).
    pub craft_counter: u32,
    /// `job.getActionsDone()` (`:284`).
    pub actions_done: u32,
    /// `dumped` (`:108`, `:305`).
    pub dumped: bool,
    /// `furnacePos` (`AbstractEntityAIRequestSmelter.java:61`).
    pub furnace_pos: Option<Pos>,
    /// `needsCurrently` — la demande au système de requêtes.
    pub needed: Option<Need>,
    /// Le transfert d'inventaire à faire, une fois sur place.
    pub transfer: Option<Transfer>,
    /// `currentRequest.addDelivery(...)` (`:257`, `:595`) : ce qui a été livré.
    pub delivered: u32,
    /// `building.setCurrentDailyQuantity(current + 1)`
    /// (`EntityAIWorkCrusher.java:135`) : l'incrément que l'appelant doit appliquer
    /// au bâtiment. Une règle décide, l'hôte écrit.
    pub daily_increment: u32,
    /// `job.finishRequest(boolean)` (`EntityAIWorkCrusher.java:236`, `:244`) : la
    /// tâche est finie (`Some(true)`) ou échouée (`Some(false)`).
    pub finished: Option<bool>,
    /// `job.getSecondaryOutputs()` (`:271`, `:314`, `AbstractEntityAICrafting.java:599`).
    pub secondary_outputs: Vec<(NeedKey, u32)>,
    /// L'expérience gagnée pendant la tranche (`:622`, `:691`, `EntityAIWorkCrusher.java:151`).
    pub experience: f64,
    /// `checkFurnaceFuel` a créé une demande de carburant (`:103-115`).
    pub fuel_request_made: bool,
    /// Les ticks de cuisson que `accelerateFurnaces` a ajoutés (`:120-142`).
    pub accelerated: u32,
    /// Le bloc que `removeFromFurnaces` doit retirer du module (`:595-599`).
    pub dropped_from_module: Option<Pos>,
    /// Le pas que `idle` a choisi (`AbstractEntityAICrafting.java:180-255`).
    pub idle_step: Option<crafter::IdleStep>,
    /// `setDelay(ticks)` (`AbstractEntityAIBasic.setDelay`), appliqué par
    /// [`WorkAi::tick`].
    pub delay: Option<u32>,
    /// Le refus du tick, remis à zéro à chaque tick.
    pub refusal: Option<TradeRefusal>,
}

/// La scène : les faits de l'atelier, ceux du métier, et la mémoire de l'IA.
///
/// **Une seule scène pour trois métiers** parce que la machine est monomorphique :
/// [`crate::ai::Machine`] ne peut pas changer de type de faits en cours de route.
/// [`Scene::job`] porte la moitié qui change, et [`Scene::craft`] est exactement le
/// jeu de faits que `crate::ai_build::crafter` attend — les observations de base ne
/// sont donc pas réinventées.
#[derive(Clone, Debug)]
pub struct Scene {
    /// Ce que `AbstractEntityAICrafting` lit du monde, déjà nommé par
    /// `crate::ai_build::crafter::CraftFacts`.
    pub craft: CraftFacts,
    /// La moitié propre au métier.
    pub job: TradeJob,
    /// Ce que l'IA se rappelle.
    pub memory: TradeMemory,
    /// `module.getFirstFulfillableRecipe(...)` : la recette que le module saurait
    /// faire maintenant (`AbstractEntityAICrafting.java:340`). Un fait, lu par
    /// `getRecipe`, que la mémoire ne fait que recopier.
    pub available: Option<Recipe>,
}

impl Scene {
    /// Une scène sur un métier, avec les faits de l'atelier par défaut.
    pub fn new(job: TradeJob) -> Scene {
        Scene {
            craft: CraftFacts::default(),
            job,
            memory: TradeMemory::default(),
            available: None,
        }
    }

    /// La scène d'un cuisinier.
    pub fn chef(facts: super::facts::SmelterFacts) -> Scene {
        Scene::new(TradeJob::chef(facts))
    }

    /// La scène d'un teinturier.
    pub fn dyer(facts: super::facts::SmelterFacts) -> Scene {
        Scene::new(TradeJob::dyer(facts))
    }

    /// La scène d'un concasseur.
    pub fn crusher(facts: super::facts::CrusherFacts) -> Scene {
        Scene::new(TradeJob::crusher(facts))
    }

    /// Les faits de l'atelier, avec les six compteurs de la mémoire écrits dedans.
    ///
    /// C'est la couture avec `crate::ai_build::crafter` : là où Java lit les mêmes
    /// champs de l'objet `job`, le port recopie la mémoire dans le jeu de faits
    /// attendu par le module voisin, et lui rend la main. Rien d'autre n'est modifié.
    pub fn crafting_facts(&self) -> CraftFacts {
        let mut facts = self.craft.clone();
        facts.has_recipe = self.memory.recipe.is_some();
        facts.has_request = self.memory.request_open;
        facts.request_cancelled = self.memory.request_cancelled;
        facts.dumped = self.memory.dumped;
        facts.actions_done = self.memory.actions_done;
        facts.progress = self.memory.progress;
        facts.max_crafting_count = self.memory.max_crafting_count;
        facts.craft_counter = self.memory.craft_counter;
        facts.craft_speed_level = self.job.skills().craft_speed; // `:735`
        facts.recipe_found = self.available.is_some(); // `:340`
        if let Some(recipe) = &self.memory.recipe {
            facts.output_count = recipe.output_count; // `:371`
            // `getExtendedCount(inputStack)` (`AbstractEntityAICrafting.java:472`,
            // `:401`) : il n'y a jamais qu'une entrée pour un four
            // (`AbstractEntityAIRequestSmelter.java:725`).
            let extended = self.job.extended_count(Some(recipe), &recipe.input);
            if let Some(input) = facts.inputs.first_mut() {
                input.extended = extended;
            }
            facts.in_progress_output = self.job.extended_count(Some(recipe), &recipe.output);
        } else {
            facts.in_progress_output = 0;
        }
        facts
    }

    /// Le four en cours, quand le métier en a.
    pub fn furnace_at(&self) -> Option<&super::facts::Furnace> {
        let at = self.memory.furnace_pos?;
        self.job.smelter()?.at(at)
    }

    /// `currentRecipeStorage.getCleanedInput().get(0).getItemStack()`.
    pub fn recipe_input(&self) -> Option<&NeedKey> {
        self.memory.recipe.as_ref().map(|recipe| &recipe.input)
    }
}

/// Un des trois métiers, avec sa machine.
pub struct WorkAi {
    machine: Machine<WorkState, Scene>,
}

impl fmt::Debug for WorkAi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorkAi")
            .field("state", &self.machine.state())
            .finish()
    }
}

impl WorkAi {
    /// La machine d'un métier, à partir de `IDLE` et au rythme de
    /// `ENTITY_AI_TICKRATE` (`AbstractEntityCitizen.java:64`).
    ///
    /// Java démarre en `INIT` et `initSafetyChecks`
    /// (`AbstractEntityAIBasic.java:446-465`) l'envoie en `IDLE` : `INIT` et ces
    /// vérifications appartiennent à `crate::ai`, donc la machine démarre là où
    /// l'autre arrive.
    pub fn new(trade: super::Trade) -> WorkAi {
        let mut machine = Machine::new(WorkState::base(CraftState::Idle), ENTITY_AI_TICKRATE);
        trade.register(&mut machine);
        WorkAi { machine }
    }

    /// L'état en vigueur.
    pub fn state(&self) -> WorkState {
        self.machine.state()
    }

    /// La machine, pour un test qui veut ses transitions.
    pub fn machine(&self) -> &Machine<WorkState, Scene> {
        &self.machine
    }

    /// Un tick : trois passes globales, puis la liste de l'état courant
    /// (`TickRateStateMachine.tick`, `:95-150`), et le `setDelay` demandé par la
    /// règle appliqué après coup — c'est ce que fait Java, qui pose le délai sur la
    /// transition qui vient de tirer.
    pub fn tick(&mut self, scene: &mut Scene) -> Outcome {
        scene.memory.refusal = None;
        scene.memory.delay = None;
        let fired = self.machine.tick(scene);
        let delay = scene.memory.delay.take();
        if let Some(ticks) = delay {
            let _ = self.machine.set_current_delay(ticks);
        }
        Outcome {
            state: self.machine.state(),
            fired,
            refusal: scene.memory.refusal.take(),
            delay,
        }
    }
}

/// Remettre à zéro les trois compteurs du travail et `dumped`.
///
/// `AbstractEntityAICrafting.resetValues` (`:668-676`) : `setMaxCraftingCount(0)`,
/// `setProgress(0)`, `setCraftCounter(0)`, les deux mains vidées (un appel d'hôte,
/// laissé à l'appelant) et `dumped = false`.
pub fn reset_values(scene: &mut Scene) {
    scene.memory.max_crafting_count = 0; // `:670`
    scene.memory.progress = 0; // `:671`
    scene.memory.craft_counter = 0; // `:672`
    scene.memory.dumped = false; // `:675`
}

/// Abandonner la tâche en cours : `currentRequest = null` et
/// `currentRecipeStorage = null`, les deux écritures que Java fait ensemble aux
/// sorties d'échec (`:557`, `:585-587`, `:623-624`).
pub fn abandon(scene: &mut Scene) {
    scene.memory.request_open = false;
    scene.memory.recipe = None;
}

/// `finalizeCraftingTask` (`AbstractEntityAICrafting.java:636-646`) : vider la
/// recette, remettre les compteurs à zéro, et dire si la tâche donne de l'expérience.
///
/// Java remet les compteurs à zéro **avant** de les tester (`:648-654`), donc les
/// trois conditions `== 0` du test sont toujours vraies ; il ne reste que
/// `inventoryNeedsDump()` et `currentRequest != null`, et c'est
/// `crafter::completion_experience` qui le dit.
pub fn finalize_crafting_task(scene: &mut Scene) -> Decision {
    let request_count = scene.craft.request_count;
    let inventory_needs_dump = scene.craft.inventory_needs_dump;
    let has_request = scene.memory.request_open;
    scene.memory.recipe = None; // `:638`
    reset_values(scene); // `:639`
    if let Some(xp) = crafter::completion_experience(request_count, inventory_needs_dump, has_request)
    {
        scene.memory.experience += xp; // `:643`
    }
    Decision::go_after(WorkState::base(CraftState::InventoryFull), TradeRefusal::InventoryFull) // `:645`
}

// ---------------------------------------------------------------- les actions

/// `hasWorkToDo` (`AbstractEntityAICrafting.java:261-264`), surchargée par le
/// concasseur (`EntityAIWorkCrusher.java:91-94`) qui ajoute le quota du jour.
fn idle_has_work(scene: &Scene, _: WorkState) -> bool {
    match scene.job {
        TradeJob::Crusher(_) => crusher::has_work_to_do(scene),
        TradeJob::Chef(_) | TradeJob::Dyer(_) => crafter::has_work_to_do(&scene.crafting_facts()),
    }
}

/// `idle` (`AbstractEntityAICrafting.java:180-255`) : les passe-temps, et
/// `NO_CHANGE` — donc `None`, le tick continue.
fn idle_action(scene: &mut Scene, _: WorkState) -> Option<WorkState> {
    let decision = crafter::idle(&scene.crafting_facts());
    scene.craft.idle_pos = decision.idle_pos; // `:217`, `:225`, `:238`, `:247`
    scene.memory.idle_step = Some(decision.step);
    None
}

/// `decide` (`AbstractEntityAICrafting.java:271-291`), surchargée par le concasseur.
fn decide_action(scene: &mut Scene, state: WorkState) -> Option<WorkState> {
    if matches!(scene.job, TradeJob::Crusher(_)) {
        // `EntityAIWorkCrusher.decide` (`:62-88`) — la méthode virtuelle de Java,
        // choisie ici sur le métier.
        let decision = crusher::decide(scene);
        store(scene, &decision);
        return decision.into_option(state);
    }
    match crafter::decide(&scene.crafting_facts()) {
        crafter::Decide::Idle => Some(WorkState::base(CraftState::Idle)), // `:275`
        // `:281` rend `START_WORKING` et `:287` rend `getState()` : les deux sont
        // l'état où l'on est, donc le tick s'arrête sans bouger.
        crafter::Decide::Walking | crafter::Decide::WaitingForDump => Some(state),
        crafter::Decide::Next(next) => Some(WorkState::base(next)), // `:290`
    }
}

/// `getRecipe` (`AbstractEntityAICrafting.java:324-422`).
fn recipe_action(scene: &mut Scene, _: WorkState) -> Option<WorkState> {
    let answer = crafter::get_recipe(&scene.crafting_facts());
    match answer {
        crafter::Recipe::Ready {
            max_crafting_count,
            craft_counter,
        } => {
            // `:340` — `currentRecipeStorage = module.getFirstFulfillableRecipe(...)`.
            scene.memory.recipe = scene.available.clone();
            scene.memory.max_crafting_count = max_crafting_count; // `:419`
            scene.memory.craft_counter = craft_counter; // `:420`
            scene.memory.request_open = true; // `:366` — `currentRequest = currentTask`
            scene.memory.request_cancelled = false;
            Some(WorkState::base(CraftState::QueryItems)) // `:421`
        }
        crafter::Recipe::Dump => {
            scene.memory.dumped = true; // `:350`
            scene.memory.recipe = None; // `:351`
            Some(WorkState::base(CraftState::InventoryFull)) // `:352`
        }
        crafter::Recipe::ShortOfIngredient {
            input,
            available,
            remaining,
        } => {
            abandon(scene); // `:405-408`
            scene.memory.refusal = Some(TradeRefusal::ShortOfIngredient {
                input,
                available,
                remaining,
            });
            Some(WorkState::base(CraftState::StartWorking))
        }
        // `:330`, `:338`, `:345`, `:362` et le `DegenerateOutput` du port : tous
        // `START_WORKING`, la recette oubliée.
        crafter::Recipe::NoTask
        | crafter::Recipe::NoModule
        | crafter::Recipe::NoRecipe
        | crafter::Recipe::ToolMissing
        | crafter::Recipe::DegenerateOutput => {
            abandon(scene);
            let refusal = match answer {
                crafter::Recipe::NoModule | crafter::Recipe::NoRecipe => Some(TradeRefusal::NoRecipe),
                crafter::Recipe::ToolMissing => Some(TradeRefusal::ToolBroken),
                _ => None,
            };
            if let Some(refusal) = refusal {
                scene.memory.refusal = Some(refusal);
            }
            Some(WorkState::base(CraftState::StartWorking))
        }
    }
}

/// `queryItems` (`AbstractEntityAICrafting.java:446-454`).
fn query_action(scene: &mut Scene, _: WorkState) -> Option<WorkState> {
    match crafter::check_for_items(&scene.crafting_facts()) {
        crafter::Items::NoRecipe => Some(WorkState::base(CraftState::StartWorking)), // `:450`
        crafter::Items::Gather { input, remaining } => {
            // `:494` — `needsCurrently = new Tuple<>(predicate, remaining)`.
            scene.memory.needed = Some(Need::input(input, remaining));
            Some(WorkState::base(CraftState::GatheringRequiredMaterials)) // `:495`
        }
        crafter::Items::ReRead => {
            scene.memory.recipe = None; // `:497`
            scene.memory.request_open = false; // `:498`
            Some(WorkState::base(CraftState::GetRecipe)) // `:499`
        }
        crafter::Items::Craft => Some(WorkState::base(CraftState::Craft)), // `:503`
    }
}

/// `craft` (`AbstractEntityAICrafting.java:511-572`), surchargée par le concasseur
/// (`EntityAIWorkCrusher.java:188-249`).
///
/// **L'ordre compte, et il est celui de Java.** Les quatre sorties qui précèdent
/// `job.setProgress(getProgress() + 1)` (`:513-526`, `:553-559`) sont décidées sur le
/// compte du début de tick ; l'incrément vient ensuite (`:528`), et c'est le compte
/// **incrémenté** que `:561` compare au nombre de coups requis. Le port fait donc les
/// deux lectures, dans cet ordre.
fn craft_action(scene: &mut Scene, state: WorkState) -> Option<WorkState> {
    if matches!(scene.job, TradeJob::Crusher(_)) {
        // `EntityAIWorkCrusher.craft` (`:188-249`), la seconde méthode virtuelle.
        let decision = crusher::craft(scene, state);
        store(scene, &decision);
        return decision.into_option(state);
    }
    match crafter::craft(&scene.crafting_facts()) {
        crafter::Craft::NoWork => return Some(WorkState::base(CraftState::StartWorking)), // `:513-516`
        crafter::Craft::NeedRecipe => return Some(WorkState::base(CraftState::GetRecipe)), // `:518-521`
        crafter::Craft::Walking => return Some(state),                                    // `:523-526`
        _ => {}
    }
    scene.memory.progress = scene.memory.progress.saturating_add(1); // `:528`
    match crafter::craft(&scene.crafting_facts()) {
        crafter::Craft::Hitting => Some(state), // `:571`
        crafter::Craft::Execute => execute_crafting_action(scene), // `:566`
        crafter::Craft::OutOfItems => Some(WorkState::base(CraftState::StartWorking)), // `:568`
        crafter::Craft::Cancelled => {
            // `:553-559` — l'annulation se lit **après** l'incrément de `:528`.
            scene.memory.request_open = false;
            scene.memory.recipe = None;
            scene.memory.refusal = Some(TradeRefusal::RequestCancelled);
            scene.memory.actions_done += 1; // `getActionRewardForCraftingSuccess()` = 1
            Some(WorkState::base(CraftState::StartWorking))
        }
        crafter::Craft::BadSkill => {
            let level = scene.job.skills().craft_speed;
            scene.memory.refusal = Some(TradeRefusal::BadSkill { level });
            Some(state)
        }
        // Les trois cas déjà traités plus haut : les atteindre signifierait que le
        // tick a changé d'avis, ce qu'aucune règle de Java ne fait.
        crafter::Craft::NoWork | crafter::Craft::NeedRecipe | crafter::Craft::Walking => {
            Some(state)
        }
    }
}

/// `executeCraftingAction` du parent (`AbstractEntityAICrafting.java:579-634`) : une
/// passe de recette d'établi, sans four.
///
/// C'est la moitié que le four appelle quand sa recette n'a pas d'entrée de four
/// (`AbstractEntityAIRequestSmelter.java:434-437`) et que le concasseur n'atteint
/// jamais (sa `craft` appelle `crush`, `EntityAIWorkCrusher.java:223`).
pub fn base_execute(scene: &mut Scene) -> Option<WorkState> {
    match crafter::execute(&scene.crafting_facts()) {
        crafter::Execution::RecipeFailed => {
            // `:581-589`
            abandon(scene);
            scene.memory.actions_done += 1;
            reset_values(scene);
            Some(WorkState::base(CraftState::StartWorking))
        }
        crafter::Execution::Finalized => {
            scene.memory.actions_done += 1; // `:611`
            let decision = finalize_crafting_task(scene); // `:618`
            store(scene, &decision);
            decision.into_option(WorkState::base(CraftState::InventoryFull))
        }
        crafter::Execution::ToolBroke => {
            // `:621-631`
            abandon(scene);
            scene.memory.actions_done += 1;
            scene.memory.refusal = Some(TradeRefusal::ToolBroken);
            reset_values(scene);
            Some(WorkState::base(CraftState::StartWorking))
        }
        crafter::Execution::Again => {
            scene.memory.progress = 0; // `:635`
            Some(WorkState::base(CraftState::GetRecipe)) // `:636`
        }
    }
}

/// `executeCraftingAction` : la troisième méthode virtuelle.
///
/// Le parent (`AbstractEntityAICrafting.java:579-634`) exécute la recette ; le four
/// (`AbstractEntityAIRequestSmelter.java:426-500`) prend la main dès qu'il a une
/// recette, four ou pas (`:429-437`), et c'est lui qui décide de sortir, de ravitailler
/// ou de remplir. Le concasseur, lui, n'atteint jamais cette méthode : sa `craft`
/// appelle `crush` directement (`EntityAIWorkCrusher.java:223`).
fn execute_crafting_action(scene: &mut Scene) -> Option<WorkState> {
    if scene.job.smelter().is_some() {
        let decision = smelter::execute_crafting_action(scene);
        store(scene, &decision);
        return decision.into_option(WorkState::base(CraftState::Craft));
    }
    base_execute(scene)
}

/// Écrire dans la scène le refus et le délai qu'une décision porte, pour que
/// [`WorkAi::tick`] les rende.
pub fn store(scene: &mut Scene, decision: &Decision) {
    if decision.refusal.is_some() {
        scene.memory.refusal = decision.refusal.clone();
    }
}

// ----------------------------------------------------------------- la table

/// Les sept cibles de `AbstractEntityAICrafting.java:160-170`, plus les deux sorties
/// que la classe de base ajoute.
///
/// Chaque action délègue à `crate::ai_build::crafter` ; ce qui est écrit ici, c'est la
/// **cible** et le rythme, pas la règle.
pub fn register_base(machine: &mut Machine<WorkState, Scene>) {
    // :164 — `new AITarget(IDLE, this::hasWorkToDo, () -> START_WORKING, TICKS_SECOND)`.
    machine.add(Transition::target_when(
        WorkState::base(CraftState::Idle),
        "hasWorkToDo",
        "AbstractEntityAICrafting.java:164",
        TICKS_SECOND,
        idle_has_work,
        Action::Fixed(Some(WorkState::base(CraftState::StartWorking))),
    ));
    // :165 — `new AITarget(IDLE, this::idle, TICKS_SECOND)`.
    machine.add(Transition::target(
        WorkState::base(CraftState::Idle),
        "idle",
        "AbstractEntityAICrafting.java:165",
        TICKS_SECOND,
        Action::Run(idle_action),
    ));
    // :166 — `new AITarget(START_WORKING, this::decide, STANDARD_DELAY)`.
    machine.add(Transition::target(
        WorkState::base(CraftState::StartWorking),
        "decide",
        "AbstractEntityAICrafting.java:166",
        STANDARD_DELAY,
        Action::Run(decide_action),
    ));
    // :167 — `new AITarget(QUERY_ITEMS, this::queryItems, STANDARD_DELAY)`.
    machine.add(Transition::target(
        WorkState::base(CraftState::QueryItems),
        "queryItems",
        "AbstractEntityAICrafting.java:167",
        STANDARD_DELAY,
        Action::Run(query_action),
    ));
    // :168 — `new AITarget(GET_RECIPE, this::getRecipe, STANDARD_DELAY)`.
    machine.add(Transition::target(
        WorkState::base(CraftState::GetRecipe),
        "getRecipe",
        "AbstractEntityAICrafting.java:168",
        STANDARD_DELAY,
        Action::Run(recipe_action),
    ));
    // :169 — `new AITarget(CRAFT, this::craft, HIT_DELAY)`.
    machine.add(Transition::target(
        WorkState::base(CraftState::Craft),
        "craft",
        "AbstractEntityAICrafting.java:169",
        crate::ai_build::work_time::CRAFT_HIT_DELAY,
        Action::Run(craft_action),
    ));
}

/// La couture avec la table de base : les deux états que ces métiers traversent sans
/// les conduire.
///
/// **Ce n'est pas un second `register_base`.** `crate::ai::worker` porte les quatorze
/// cibles de `AbstractEntityAIBasic` sur `WorkerState`, et cette machine ne peut pas
/// les héberger (autre type d'état). Ce qui est écrit ici est seulement le chemin du
/// retour : `INVENTORY_FULL` rend la main à `IDLE` après le vidage
/// (`AbstractEntityAIBasic.java:1213-1222`), et `GATHERING_REQUIRED_MATERIALS` rend la
/// main à `START_WORKING` (`AbstractEntityAICrafting.java:436-439`).
pub fn register_handover(machine: &mut Machine<WorkState, Scene>) {
    machine.add(Transition::goto(
        WorkState::base(CraftState::InventoryFull),
        WorkState::base(CraftState::Idle),
        "afterDump",
        "AbstractEntityAIBasic.java:1213-1222",
        STANDARD_DELAY,
    ));
    machine.add(Transition::goto(
        WorkState::base(CraftState::GatheringRequiredMaterials),
        WorkState::base(CraftState::StartWorking),
        "getStateAfterPickUp",
        "AbstractEntityAICrafting.java:436-439",
        STANDARD_DELAY,
    ));
}

/// Les états qu'une table peut atteindre : les sept de l'atelier, plus ceux du métier.
pub fn states_of(trade: super::Trade) -> Vec<WorkState> {
    WorkState::ALL
        .iter()
        .copied()
        .filter(|state| match trade {
            super::Trade::Chef | super::Trade::Dyer => {
                state.craft().is_some() || state.is_furnace_state()
            }
            super::Trade::Crusher => state.craft().is_some() || *state == WorkState::Crush,
        })
        .collect()
}

/// Le rythme du four : `TICKS_SECOND` pour chacun des quatre états
/// (`AbstractEntityAIRequestSmelter.java:78-81`).
pub const FURNACE_RATE: u32 = TICKS_SECOND;

/// Le rythme des deux vérifications de fond du four : `TICKS_SECOND * 10 = 200`
/// ticks (`:73`, `:75`).
pub const SLOW_FURNACE_RATE: u32 = TICKS_SECOND * 10;

/// L'état dans lequel un four entre quand il n'a plus rien à faire.
pub const FURNACE_IDLE: WorkState = WorkState::Base(CraftState::StartWorking);
