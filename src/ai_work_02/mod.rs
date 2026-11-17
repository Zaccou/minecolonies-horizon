//! Les buts d'IA du cuisinier, du teinturier et du concasseur : la machine à états de
//! chacun, ce qu'elle demande au monde, et pourquoi elle s'arrête.
//!
//! Java référence trois classes de `core/entity/ai/workers/crafting/` :
//!
//! | Métier | Classe Java | Lignes | Ce qu'elle ajoute |
//! | --- | --- | ---: | --- |
//! | cuisinier | `EntityAIWorkChef.java` | 50 | rien : deux noms de statistique (`:36-49`) |
//! | teinturier | `EntityAIWorkDyer.java` | 27 | rien : le bâtiment attendu (`:22-26`) |
//! | concasseur | `EntityAIWorkCrusher.java` | 251 | l'état `CRUSH`, un quota quotidien, deux méthodes surchargées |
//!
//! **Le cuisinier et le teinturier ne sont pas des cas particuliers.** Les deux
//! étendent `AbstractEntityAIRequestSmelter` (760 lignes) sans en surcharger une seule
//! méthode : leur but d'IA **est** la table du four, et leurs cinquante et vingt-sept
//! lignes ne changent que le bâtiment attendu et le nom de la statistique. C'est
//! pourquoi ce module porte [`smelter`] une fois et le donne aux deux, au lieu
//! d'écrire deux fichiers qui n'auraient rien à dire. Ce qui les distingue
//! réellement, ce sont leurs **compétences** — `Creativity`/`Knowledge` pour le
//! cuisinier, `Creativity`/`Dexterity` pour le teinturier
//! (`BuildingModules.java:451-457`, `:290-295`) — donc leurs durées de travail, et
//! c'est un test qui le montre.
//!
//! Le concasseur, lui, étend `AbstractEntityAICrafting` (785 lignes) et ajoute son
//! quota et son état de broyage. Il partage avec les deux autres **toute** la table
//! de l'atelier : `IDLE`, `START_WORKING`, `QUERY_ITEMS`, `GET_RECIPE`, `CRAFT`.
//!
//! ## La machine, et ce qu'elle est
//!
//! Une seule table par métier, sur le moteur [`crate::ai::state::Machine`] : trois
//! passes globales, puis la liste de l'état courant, et le premier qui tire arrête le
//! tick. `S` vaut [`WorkState`], `F` vaut [`Scene`].
//!
//! ```ignore
//! let mut ai = WorkAi::new(Trade::Chef);
//! let mut scene = Scene::chef(smelter_facts);
//! scene.memory.recipe = Some(recipe);
//! scene.craft.task_queue = 1;
//! scene.craft.current_task = true;
//! let outcome = ai.tick(&mut scene);
//! match (outcome.goal(), outcome.why_not()) {
//!     (WorkState::FillUpFurnaces, _) => /* remplir le four à memory.furnace_pos */,
//!     (_, Some(TradeRefusal::NoFurnaces)) => /* le joueur n'a posé aucun four */,
//!     _ => {}
//! }
//! ```
//!
//! ## Ce qui n'est pas ici, et pourquoi
//!
//! - **Aucun appel au monde.** Pas de `walkToBuilding`, pas de
//!   `FurnaceBlockEntity.getItem`, pas de `transferXInItemHandler...`. Chaque lecture
//!   est un fait ([`facts`]), chaque écriture une sortie de la mémoire
//!   ([`state::Transfer`], `daily_increment`, `fuel_request_made`,
//!   `dropped_from_module`, `delivered`). C'est ce qui rend les scénarios de
//!   [`tests`] rejouables.
//! - **Aucune API de mod.** Ce module n'importe pas `mod_api` : il ne dépend que de
//!   `crate::ai` (le moteur et les faits généraux), `crate::ai_build` (la table de
//!   l'atelier et ses formules de durée) et de ses propres fichiers.
//! - **Les autres métiers de fonte** — `EntityAIWorkBaker`, `EntityAIWorkBlacksmith`,
//!   `EntityAIWorkAlchemist`, `EntityAIWorkSifter`, `EntityAIWorkSmelter` — partagent
//!   la table du four mais ne sont pas dans cette tranche. Le four du cuisinier et du
//!   teinturier est écrit **une fois** dans [`smelter`] ; le jour où ces métiers
//!   arrivent, ils s'y raccordent au lieu d'en écrire une quatrième copie.
//!
//! ## Ce que la tranche ne prétend pas finir
//!
//! `src/ai_work_02/MANQUANT.md` liste cinq choses que l'API de mod ne permet pas
//! d'écrire — la requête de carburant, l'accélération d'un `FurnaceBlockEntity`, le
//! retrait d'un four du module, la remise à zéro du quota au réveil, et le délai de
//! `idle` — avec, pour chacune, ce que le port fait à la place. Il porte aussi trois
//! constats mesurés : deux citations d'`AIWorkerState.java` fausses dans
//! `crate::ai_build::crafter`, une branche de `EntityAIWorkCrusher.java:158-161` que
//! `:121` rend inatteignable, et le fait que le boulanger d'`ai_work_01` porte déjà un
//! second port de `AbstractEntityAIRequestSmelter` — deux fichiers que seul
//! l'orchestrateur peut arbitrer, puisqu'il les voit tous les deux compilés.

pub mod crusher;
pub mod facts;
pub mod smelter;
pub mod state;

#[cfg(test)]
mod tests;

use crate::ai::Machine;

pub use facts::{CrusherFacts, Furnace, Recipe, Skills, Slot, SmelterFacts, TradeJob};
pub use state::{
    abandon, finalize_crafting_task, reset_values, Decision, Need, NeedItem, Outcome, Scene,
    TradeMemory, TradeRefusal, Transfer, WorkAi, WorkState, FURNACE_RATE, SLOW_FURNACE_RATE,
};

/// `Constants.TICKS_SECOND` (`Constants.java:21`).
pub const TICKS_SECOND: u32 = facts::TICKS_SECOND;

/// Les trois métiers de cette tranche.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: les trois métiers de la tranche
pub enum Trade {
    /// `EntityAIWorkChef` — un four, un `BuildingKitchen`.
    Chef,
    /// `EntityAIWorkDyer` — un four, un `BuildingDyer`.
    Dyer,
    /// `EntityAIWorkCrusher` — un quota, un `BuildingCrusher`.
    Crusher,
}

impl Trade {
    /// Les trois métiers.
    pub const ALL: [Trade; 3] = [Trade::Chef, Trade::Dyer, Trade::Crusher];

    /// Le nom du métier, tel que le mod l'écrit.
    pub fn name(self) -> &'static str {
        match self {
            Trade::Chef => "chef",
            Trade::Dyer => "dyer",
            Trade::Crusher => "crusher",
        }
    }

    /// La classe d'IA de Java.
    pub fn class(self) -> &'static str {
        match self {
            Trade::Chef => "EntityAIWorkChef",
            Trade::Dyer => "EntityAIWorkDyer",
            Trade::Crusher => "EntityAIWorkCrusher",
        }
    }

    /// La classe mère dont la table vient, et sa longueur.
    pub fn parent(self) -> &'static str {
        match self {
            Trade::Chef | Trade::Dyer => "AbstractEntityAIRequestSmelter (760 lignes)",
            Trade::Crusher => "AbstractEntityAICrafting (785 lignes)",
        }
    }

    /// Le bâtiment attendu (`getExpectedBuildingClass`).
    pub fn building(self) -> &'static str {
        match self {
            // `EntityAIWorkChef.java:27-30`
            Trade::Chef => "BuildingKitchen",
            // `EntityAIWorkDyer.java:22-26`
            Trade::Dyer => "BuildingDyer",
            // `EntityAIWorkCrusher.java:56-60`
            Trade::Crusher => "BuildingCrusher",
        }
    }

    /// La compétence qui donne la vitesse de la recette
    /// (`CraftingWorkerBuildingModule.getCraftSpeedSkill()`), et sa ligne.
    pub fn craft_speed_skill(self) -> (&'static str, &'static str) {
        match self {
            // `BuildingModules.java:457`
            Trade::Chef => ("Knowledge", "BuildingModules.java:457"),
            // `:295`
            Trade::Dyer => ("Dexterity", "BuildingModules.java:295"),
            // `:280-281` — le constructeur à cinq arguments prend la primaire
            Trade::Crusher => ("Stamina", "BuildingModules.java:280"),
        }
    }

    /// Les faits du métier, vides.
    pub fn job_facts(self) -> TradeJob {
        match self {
            Trade::Chef => TradeJob::chef(SmelterFacts::default()),
            Trade::Dyer => TradeJob::dyer(SmelterFacts::default()),
            Trade::Crusher => TradeJob::crusher(CrusherFacts::default()),
        }
    }

    /// Ce métier lit-il des fours ?
    pub fn smelts(self) -> bool {
        !matches!(self, Trade::Crusher)
    }

    /// Les états que ce métier peut atteindre.
    pub fn states(self) -> Vec<WorkState> {
        state::states_of(self)
    }

    /// La table du métier : les sept cibles de l'atelier, puis les siennes, puis la
    /// couture du retour.
    ///
    /// **L'ordre est celui de Java** : le constructeur du parent enregistre les
    /// sept cibles de `AbstractEntityAICrafting.java:160-170`, celui de la
    /// sous-classe ajoute les siennes derrière (`AbstractEntityAIRequestSmelter.java:
    /// 71-82`, `EntityAIWorkCrusher.java:49-52`). La liste d'un état étant parcourue
    /// dans l'ordre d'enregistrement, c'est cet ordre qui donne `hasWorkToDo` avant
    /// `idle` avant `startWorking` — un test le vérifie.
    pub fn register(self, machine: &mut Machine<WorkState, Scene>) {
        state::register_base(machine);
        match self {
            Trade::Chef | Trade::Dyer => {
                smelter::register_events(machine);
                smelter::register(machine);
            }
            Trade::Crusher => crusher::register(machine),
        }
        state::register_handover(machine);
    }
}

impl core::fmt::Display for Trade {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}
