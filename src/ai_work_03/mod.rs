//! Les buts de trois métiers d'atelier : le bûcheron, le souffleur de verre et le
//! mécanicien.
//!
//! Référence Java : `core/entity/ai/workers/crafting/EntityAIWorkFletcher.java`,
//! `EntityAIWorkGlassblower.java` et `EntityAIWorkMechanic.java` — trois fichiers de
//! **27, 28 et 28 lignes**, dont le seul code est un `@Override` de
//! `getExpectedBuildingClass()`. Mesuré sur l'arbre, pas supposé.
//!
//! ## La prémisse du brief est fausse, et voici la mesure
//!
//! Le brief demande « les buts d'IA de ces métiers » avec ces trois fichiers comme
//! sources exactes. Ces fichiers ne contiennent pas de but : ils déclarent la hutte et
//! s'arrêtent. Tout ce qui décide est dans les parents :
//!
//! | Métier | Classe parente | Lignes | Ce qu'elle ajoute |
//! | --- | --- | ---: | --- |
//! | `EntityAIWorkFletcher` | `AbstractEntityAICrafting` | 785 | **rien** |
//! | `EntityAIWorkMechanic` | `AbstractEntityAICrafting` | 785 | **rien** |
//! | `EntityAIWorkGlassblower` | `AbstractEntityAIRequestSmelter` → `AbstractEntityAICrafting` | 760 + 785 | sept cibles, quatre états |
//!
//! `AbstractEntityAICrafting` est **déjà porté** : `crate::ai_build::crafter` porte ses
//! six cibles (`:164-169`), `crate::ai_build::work_time::craft_hits` son nombre de
//! coups (`:734-738`), et `crate::ai_build::crafter::CraftState` ses états. Ce module ne
//! les recopie donc pas : il les **appelle**. Ce qu'il apporte est ce qui n'existait
//! nulle part dans l'arbre :
//!
//! 1. **la couche four de `AbstractEntityAIRequestSmelter`** ([`smelter`], [`furnace`]) :
//!    les quatre états `FILL_UP_FURNACES`, `RETRIEVING_END_PRODUCT_FROM_FURNACE`,
//!    `RETRIEVING_UNRELATED_PRODUCT_FROM_FURNACE` et `ADD_FUEL_TO_FURNACE`, leurs sept
//!    cibles, et les six scans de fours qui les décident ;
//! 2. **la table des compétences de fabrication** : `getCraftSpeedSkill()` et
//!    `getRecipeImprovementSkill()` (`CraftingWorkerBuildingModule.java:49-61`) ne sont
//!    nulle part dans le port, et elles ne valent pas la même chose pour les trois
//!    métiers — voir [`Trade::craft_speed_skill`] ;
//! 3. **les quatre règles du métier d'atelier que `crate::ai_build` laisse à l'appelant** :
//!    le vidage après **une** action (`AbstractEntityAICrafting.java:724-727`), la
//!    reprise après un vidage (`:741-744`), le retour après une cueillette (`:436-439`)
//!    et le refus d'inactivité (`AbstractEntityAIBasic.java:1921-1924`).
//!
//! ## Ce que ce module ne fait pas
//!
//! - **Il ne touche pas au monde.** Aucun `mod_api` importé : le module est données et
//!   règles, comme `crate::ai`, `crate::ai_gather` et `crate::ai_build`.
//! - **Il ne recopie pas le crafter.** Les sept cibles communes sont redéclarées sur
//!   [`state::TradeState`] parce que `Machine<S, F>` est monomorphique en `S` et en `F`
//!   (`crate::ai::state::Machine`) et qu'une machine ne peut pas en contenir une autre ;
//!   mais **chaque action appelle** `crafter::decide`, `crafter::check_for_items`,
//!   `crafter::get_recipe`, `crafter::craft` et `crafter::execute`, et le test
//!   `the_trade_table_answers_like_the_crafter` tient ce contrat sur une séquence.
//! - **Il ne devine pas.** Une scène de souffleur de verre sans faits de four, ou
//!   l'inverse, est refusée par son nom ([`state::TradeRefusal::NoFactsForTrade`]).
//!
//! ## Un chevauchement à réconcilier, dit ici plutôt que caché
//!
//! `AbstractEntityAIRequestSmelter` n'est pas la classe du seul souffleur de verre :
//! **cinq** métiers en descendent — `EntityAIWorkBaker`, `EntityAIWorkChef`,
//! `EntityAIWorkDyer`, `EntityAIWorkGlassblower`, `EntityAIWorkStoneSmeltery`. Le lot
//! voisin (`crate::ai_work_01`) porte le boulanger et écrit sa propre couche four dans
//! `ai_work_01/furnace.rs`. Les deux existent parce que les deux agents ignoraient
//! l'autre ; **le domicile définitif doit être unique**, et les six scans de
//! [`furnace`] sont écrits pour ça : ce sont des fonctions pures sur
//! [`facts::SmelterFacts`] qui ne savent pas quel métier les appelle. `RAPPORT.md` §4
//! chiffre la réconciliation.
//!
//! ```ignore
//! let mut ai = TradeAi::new(Trade::Glassblower);
//! let mut scene = ai.scene();
//! scene.skills.set(Skill::Focus, 20);
//! if let Some(furnace_facts) = scene.smelter.as_mut() {
//!     furnace_facts.furnaces = vec![Furnace::empty(Pos::new(1, 64, 1))];
//!     furnace_facts.fuel_list = vec![NeedKey::plain("minecraft:coal")];
//! }
//! let outcome = ai.tick(&mut scene);
//! match (outcome.goal(), outcome.why_not()) {
//!     (TradeState::AddFuelToFurnace, _) => /* put coal in */,
//!     (_, Some(TradeRefusal::NoFurnaces)) => /* ask the player to place a furnace */,
//!     _ => {}
//! }
//! ```

pub mod facts;
pub mod furnace;
pub mod smelter;
pub mod state;

#[cfg(test)]
mod tests;

use core::fmt;

use crate::ai::state::{Action, Fired, Machine, Transition, ENTITY_AI_TICKRATE};
use crate::ai_build::crafter::{self, Craft as CraftExecution, CraftFacts, CraftState, Decide, Execution, Items, Recipe};
use crate::ai_build::work_time;

pub use facts::{Furnace, FurnaceStack, Memory, Scene, Skills, SmelterFacts, Trade, FUEL_SLOT, RESULT_SLOT, SMELTABLE_SLOT, STACKSIZE};
pub use furnace::{
    OutputScan, UnrelatedScan, acceleration_ticks, active_fuels, are_all_loaded, count_of_burning,
    empty_with_fuel, extended_count, fill_split, fuel_request_size, has_spare_fuel, is_correct_fuel,
    max_usable_furnaces, more_burning_than_usable, output_target, unrelated_target, without_fuel,
    FUEL_REQUEST_MIN,
};
pub use smelter::{
    accelerate_furnaces, add_fuel, check_for_left_overs, check_furnace_fuel, execute_crafting_action,
    fill_up, retrieve_product, retrieve_unrelated, BASE_XP_GAIN, SLOW_TICK_RATE, TICK_RATE,
};
pub use state::{base, Decision, TradeAction, TradeRefusal, TradeState};

/// `Constants.TICKS_SECOND` (`api/util/constant/Constants.java:21`) : le rythme que les
/// constructeurs passent à `registerTargets`.
pub const TICKS_SECOND: u32 = 20;

/// `AbstractEntityAIBasic.STANDARD_DELAY` (`:100`), tel que `crate::ai` l'a déjà fixé.
pub const STANDARD_DELAY: u32 = crate::ai::worker::STANDARD_DELAY;

/// `AbstractEntityAICrafting.getActionsDoneUntilDumping()` (`:724-727`) : **une**
/// action, pour les trois métiers.
///
/// C'est le nombre qui change tout le rythme : un bûcheron vide ses poches après chaque
/// fabrication réussie, là où un bâtisseur attend 32 actions.
pub fn actions_until_dumping(_trade: Trade) -> u32 {
    1 // `:726`
}

/// La ligne du même nombre, pour un test ou une ligne de log.
pub fn actions_until_dumping_java(_trade: Trade) -> &'static str {
    "AbstractEntityAICrafting.java:724-727"
}

/// `isAfterDumpPickupAllowed()` (`:741-744`) : après un vidage, la reprise de matériaux
/// n'est permise que si **aucune** requête n'est en cours.
///
/// Les trois métiers héritent de la même réponse — aucun des trois ne la surcharge, et
/// c'est vérifié : `grep -c isAfterDumpPickupAllowed` sur les trois fichiers rend zéro.
pub fn after_dump_pickup_allowed(base_facts: &CraftFacts) -> bool {
    !base_facts.has_request // `:743`
}

/// `getStateAfterPickUp()` (`:436-439`) : après une cueillette de matériaux, le métier
/// d'atelier revient à `START_WORKING` — et non à `IDLE` comme la classe de base
/// (`AbstractEntityAIBasic.java:325`).
pub fn state_after_pickup(_trade: Trade) -> TradeState {
    base(CraftState::StartWorking) // `:438`
}

/// `canGoIdle()` (`AbstractEntityAIBasic.java:1921-1924`) : aucun des trois ne le
/// surcharge, donc un citoyen de ces métiers ne rend jamais sa journée à la colonie.
pub fn can_go_idle(_trade: Trade) -> bool {
    false // `:1923`
}

/// Les états qu'une machine de `trade` peut atteindre : les sept communs, plus les
/// quatre du four pour le seul souffleur de verre.
pub fn states_of(trade: Trade) -> Vec<TradeState> {
    TradeState::ALL
        .into_iter()
        .filter(|state| trade.uses_furnaces() || !state.is_furnace())
        .collect()
}

/// Le nombre de coups qu'une recette demande à ce métier, pour ce citoyen.
///
/// `getRequiredProgressForMakingRawMaterial` (`AbstractEntityAICrafting.java:734-738`)
/// lit `getCraftSpeedSkill()` du module — [`Trade::craft_speed_skill`] —, pas la
/// compétence primaire du métier. Deux métiers au même total de points ne frappent donc
/// pas le même nombre de fois, et la scène est le seul chemin pour poser le niveau : la
/// fonction ne peut pas recevoir la mauvaise compétence.
pub fn craft_hits_for(scene: &Scene) -> Result<u32, work_time::Refusal> {
    work_time::craft_hits(i32::from(scene.craft_speed_level())) // `:737`
}

/// La même chose en ticks : `CRAFT` est regardé tous les `HIT_DELAY` (`:73`)
/// (`work_time::CRAFT_HIT_DELAY`).
pub fn craft_ticks_for(scene: &Scene) -> Result<u32, work_time::Refusal> {
    work_time::craft_ticks(i32::from(scene.craft_speed_level()))
}

/// Ce qu'un tick d'IA a décidé : l'état en vigueur, la règle qui l'y a mis, le refus si
/// elle n'a pas pu avancer, et l'action demandée au monde.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeOutcome {
    /// Le but en vigueur après ce tick.
    pub state: TradeState,
    /// La transition qui a tourné, s'il y en a une.
    pub fired: Option<Fired<TradeState>>,
    /// Le refus enregistré pendant ce tick.
    pub refusal: Option<TradeRefusal>,
    /// Ce que le but demande au monde.
    pub action: Option<TradeAction>,
}

impl TradeOutcome {
    /// Le but en vigueur.
    pub fn goal(&self) -> TradeState {
        self.state
    }

    /// Pourquoi rien n'a avancé, si le tick a une raison.
    pub fn why_not(&self) -> Option<TradeRefusal> {
        self.refusal
    }

    /// Ce que le but demande au monde, s'il demande.
    pub fn asked(&self) -> Option<&TradeAction> {
        self.action.as_ref()
    }

    /// Si ce tick a changé de but.
    pub fn moved(&self) -> bool {
        matches!(self.fired, Some(fired) if fired.from != fired.to)
    }

    /// Si la machine est tombée dans un état piège
    /// (`BasicStateMachine.java:220-228`).
    pub fn faulted(&self) -> bool {
        matches!(self.fired, Some(fired) if fired.reset)
    }

    /// La ligne Java de la règle qui a tourné, pour un log.
    pub fn why(&self) -> Option<&'static str> {
        self.fired.map(|fired| fired.java)
    }
}

/// Une IA de métier : son métier et sa machine, rien d'autre.
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
    /// L'IA du métier, qui démarre en `IDLE` au rythme de l'IA d'entité
    /// (`AbstractEntityCitizen.java:64`).
    ///
    /// Java démarre en `INIT` et `initSafetyChecks` (`AbstractEntityAIBasic.java:446-465`)
    /// l'envoie en `IDLE` ; `INIT` et ces contrôles appartiennent à `crate::ai`, donc
    /// cette machine démarre là où l'autre arrive.
    pub fn new(trade: Trade) -> TradeAi {
        let mut machine = Machine::new(base(CraftState::Idle), ENTITY_AI_TICKRATE);
        register_targets(trade, &mut machine);
        TradeAi { trade, machine }
    }

    /// Une scène vide pour ce métier.
    pub fn scene(&self) -> Scene {
        Scene::new(self.trade)
    }

    /// Le métier.
    pub fn trade(&self) -> Trade {
        self.trade
    }

    /// L'état de la machine (`AbstractAISkeleton.getState`, `:120-126`).
    pub fn state(&self) -> TradeState {
        self.machine.state()
    }

    /// Le dernier piège, si un tick en a déclenché un.
    pub fn missing(&self) -> Option<crate::ai::state::MissingTransition<TradeState>> {
        self.machine.missing()
    }

    /// `AbstractAISkeleton.resetAI` (`:145-152`).
    pub fn reset(&mut self) {
        self.machine.reset();
    }

    /// La machine, pour lire la table.
    pub fn machine(&self) -> &Machine<TradeState, Scene> {
        &self.machine
    }

    /// Un tick d'IA (`AbstractAISkeleton.tick`, `:63-66`).
    ///
    /// Avant la machine, la compétence de vitesse est recopiée dans `base` : Java la lit
    /// à chaque appel de `getRequiredProgressForMakingRawMaterial`
    /// (`AbstractEntityAICrafting.java:735`), le port l'écrit une fois par tick, ce qui
    /// est la même valeur au même moment.
    pub fn tick(&mut self, scene: &mut Scene) -> TradeOutcome {
        scene.refusal = None;
        scene.action = None;
        scene.base.craft_speed_level = i32::from(scene.craft_speed_level());
        let fired = self.machine.tick(scene);
        TradeOutcome {
            state: self.machine.state(),
            fired,
            refusal: scene.refusal,
            action: scene.action.take(),
        }
    }
}

/// Enregistrer les cibles de `trade` : les sept communes, plus les sept du four pour le
/// souffleur de verre.
pub fn register_targets(trade: Trade, machine: &mut Machine<TradeState, Scene>) {
    register_crafting(machine);
    if trade.uses_furnaces() {
        smelter::register(machine);
    }
}

/// Les cibles de `AbstractEntityAICrafting.java:160-170`, redéclarées sur
/// [`TradeState`] et déléguant chaque règle au module qui la possède.
///
/// C'est la partie « dire deux fois la cible, une seule fois la règle » : chaque action
/// ci-dessous appelle `crafter::*` et ne réimplémente rien.
pub fn register_crafting(machine: &mut Machine<TradeState, Scene>) {
    // `AbstractEntityAIBasic.java:191-198` — the hut's class. Java refuses to leave
    // `INIT`; this machine starts in `IDLE`, so it holds `IDLE` and says why.
    machine.add(Transition::target(
        base(CraftState::Idle),
        "checkExpectedBuildingClass",
        "AbstractEntityAIBasic.java:191-198",
        1,
        Action::Run(hold_wrong_hut),
    ));
    // `:164` — `new AITarget(IDLE, this::hasWorkToDo, () -> START_WORKING, TICKS_SECOND)`.
    machine.add(Transition::target_when(
        base(CraftState::Idle),
        "hasWorkToDo",
        "AbstractEntityAICrafting.java:164",
        TICKS_SECOND,
        idle_has_work,
        Action::Fixed(Some(base(CraftState::StartWorking))),
    ));
    // `:165` — `new AITarget(IDLE, this::idle, TICKS_SECOND)`.
    machine.add(Transition::target(
        base(CraftState::Idle),
        "idle",
        "AbstractEntityAICrafting.java:165",
        TICKS_SECOND,
        Action::Run(idle_action),
    ));
    // `:166` — `new AITarget(START_WORKING, this::decide, STANDARD_DELAY)`.
    machine.add(Transition::target(
        base(CraftState::StartWorking),
        "decide",
        "AbstractEntityAICrafting.java:166",
        STANDARD_DELAY,
        Action::Run(decide_action),
    ));
    // `:167` — `new AITarget(QUERY_ITEMS, this::queryItems, STANDARD_DELAY)`.
    machine.add(Transition::target(
        base(CraftState::QueryItems),
        "queryItems",
        "AbstractEntityAICrafting.java:167",
        STANDARD_DELAY,
        Action::Run(query_action),
    ));
    // `:168` — `new AITarget(GET_RECIPE, this::getRecipe, STANDARD_DELAY)`.
    machine.add(Transition::target(
        base(CraftState::GetRecipe),
        "getRecipe",
        "AbstractEntityAICrafting.java:168",
        STANDARD_DELAY,
        Action::Run(recipe_action),
    ));
    // `:169` — `new AITarget(CRAFT, this::craft, HIT_DELAY)`.
    machine.add(Transition::target(
        base(CraftState::Craft),
        "craft",
        "AbstractEntityAICrafting.java:169",
        work_time::CRAFT_HIT_DELAY,
        Action::Run(craft_action),
    ));
    // `afterDump` (`AbstractEntityAIBasic.java:1213-1222`) : le vidage rend la main à
    // `IDLE`, le profil d'atelier ne surchargeant pas `afterDump`.
    machine.add(Transition::goto(
        base(CraftState::InventoryFull),
        base(CraftState::Idle),
        "afterDump",
        "AbstractEntityAIBasic.java:1213-1222",
        STANDARD_DELAY,
    ));
    // `getStateAfterPickUp()` (`AbstractEntityAICrafting.java:436-439`).
    machine.add(Transition::goto(
        base(CraftState::GatheringRequiredMaterials),
        base(CraftState::StartWorking),
        "getStateAfterPickUp",
        "AbstractEntityAICrafting.java:436-439",
        STANDARD_DELAY,
    ));
}

/// Écrire le refus et l'action d'une décision dans la scène, et rendre le successeur.
///
/// `None` de la décision devient « l'état où je suis », ce qui est exactement
/// `return getState()` de Java.
pub(crate) fn apply(scene: &mut Scene, state: TradeState, decision: Decision) -> Option<TradeState> {
    if let Some(refusal) = decision.refusal {
        scene.refusal = Some(refusal);
    }
    if let Some(action) = decision.action {
        scene.action = Some(action);
    }
    decision.next.or(Some(state))
}

/// `AbstractEntityAICrafting.executeCraftingAction` (`:579-637`), la moitié que le
/// souffleur de verre délègue quand sa recette ne passe pas par un four
/// (`AbstractEntityAIRequestSmelter.java:434-437`).
pub(crate) fn execute_base(base_facts: &mut CraftFacts) -> Decision {
    match crafter::execute(base_facts) {
        Execution::RecipeFailed => Decision::go(base(CraftState::StartWorking)), // `:588`
        Execution::Finalized => {
            base_facts.max_crafting_count = 0; // `:670`
            base_facts.progress = 0; // `:671`
            base_facts.craft_counter = 0; // `:672`
            base_facts.dumped = false; // `:676`
            Decision::go(base(CraftState::InventoryFull)) // `:646`
        }
        Execution::ToolBroke => Decision::go(base(CraftState::StartWorking)), // `:628`
        Execution::Again => {
            base_facts.progress = 0; // `:635`
            Decision::go(base(CraftState::GetRecipe)) // `:636`
        }
    }
}

// --------------------------------------------------------------- les actions

/// `AbstractEntityAIBasic.java:191` : la hutte n'est pas celle du métier, on ne bouge pas.
fn hold_wrong_hut(scene: &mut Scene, state: TradeState) -> Option<TradeState> {
    if scene.hut_right_class {
        // Not a refusal: the action runs, records nothing, and the tick goes on to the
        // next transition, which is Java's `false` predicate.
        return None;
    }
    scene.refuse(TradeRefusal::WrongBuildingClass);
    Some(state)
}

/// `:164` — `hasWorkToDo` est la condition, la cible est constante.
fn idle_has_work(scene: &Scene, _: TradeState) -> bool {
    crafter::has_work_to_do(&scene.base)
}

/// `:165` — `idle` (`:180-255`) : les passe-temps, et `NO_CHANGE`.
fn idle_action(scene: &mut Scene, _: TradeState) -> Option<TradeState> {
    let decision = crafter::idle(&scene.base);
    scene.base.idle_pos = decision.idle_pos; // `:202`, `:205`
    None // `:196`, `:207`, `:229`, `:250`, `:254` — all `NO_CHANGE`
}

/// `:166` — `decide` (`:271-291`).
fn decide_action(scene: &mut Scene, state: TradeState) -> Option<TradeState> {
    match crafter::decide(&scene.base) {
        Decide::Idle => Some(base(CraftState::Idle)), // `:276`
        // `:281` and `:287` both answer the state we are already in.
        Decide::Walking | Decide::WaitingForDump => Some(state),
        Decide::Next(next) => Some(base(next)), // `:290`
    }
}

/// `:167` — `queryItems` (`:446-451`) puis `checkForItems` (`:462-504`).
fn query_action(scene: &mut Scene, _: TradeState) -> Option<TradeState> {
    match crafter::check_for_items(&scene.base) {
        Items::NoRecipe => Some(base(CraftState::StartWorking)), // `:450`
        Items::Gather { .. } => Some(base(CraftState::GatheringRequiredMaterials)), // `:495`
        Items::ReRead => {
            scene.base.has_recipe = false; // `:497-498`
            scene.base.has_request = false;
            Some(base(CraftState::GetRecipe)) // `:499`
        }
        Items::Craft => Some(base(CraftState::Craft)), // `:503`
    }
}

/// `:168` — `getRecipe` (`:324-422`), avec ses deux écritures (`:350-351`, `:419-420`).
fn recipe_action(scene: &mut Scene, _: TradeState) -> Option<TradeState> {
    match crafter::get_recipe(&scene.base) {
        Recipe::Ready {
            max_crafting_count,
            craft_counter,
        } => {
            scene.base.max_crafting_count = max_crafting_count; // `:419`
            scene.base.craft_counter = craft_counter; // `:420`
            scene.base.has_recipe = true;
            Some(base(CraftState::QueryItems)) // `:421`
        }
        Recipe::Dump => {
            scene.base.dumped = true; // `:350`
            scene.base.has_recipe = false; // `:351`
            Some(base(CraftState::InventoryFull)) // `:352`
        }
        // `:330`, `:338`, `:345`, `:362`, `:408` — all `START_WORKING`.
        Recipe::NoTask
        | Recipe::NoModule
        | Recipe::NoRecipe
        | Recipe::ToolMissing
        | Recipe::ShortOfIngredient { .. }
        | Recipe::DegenerateOutput => Some(base(CraftState::StartWorking)),
    }
}

/// `:169` — `craft` (`:511-572`), puis `executeCraftingAction` (`:579-637`).
///
/// Le souffleur de verre **surcharge** `executeCraftingAction`
/// (`AbstractEntityAIRequestSmelter.java:427`) : c'est le four qui décide, et il délègue
/// à la classe de base quand la recette n'est pas une cuisson (`:434-437`). Les deux
/// autres métiers n'ont que la classe de base.
fn craft_action(scene: &mut Scene, state: TradeState) -> Option<TradeState> {
    match crafter::craft(&scene.base) {
        CraftExecution::NoWork | CraftExecution::Cancelled | CraftExecution::OutOfItems => {
            Some(base(CraftState::StartWorking)) // `:513-516`, `:553-559`, `:568`
        }
        CraftExecution::NeedRecipe => Some(base(CraftState::GetRecipe)), // `:520`
        CraftExecution::Walking | CraftExecution::Hitting | CraftExecution::BadSkill => {
            Some(state) // `:525`, `:571`
        }
        CraftExecution::Execute => {
            let decision = if scene.trade.uses_furnaces() {
                smelter::execute_crafting_action(scene)
            } else {
                execute_base(&mut scene.base)
            };
            apply(scene, state, decision)
        }
    }
}
