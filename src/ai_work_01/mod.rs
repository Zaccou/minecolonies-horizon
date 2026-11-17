//! Les trois métiers de l'atelier de `core/entity/ai/workers/crafting/` : l'alchimiste
//! et ses alambics, le boulanger et ses fours, le forgeron — qui n'ajoute rien.
//!
//! Référence Java, une classe par métier :
//!
//! | Métier | Classe | Lignes | Ce qu'elle ajoute |
//! | --- | --- | ---: | --- |
//! | alchimiste | `EntityAIWorkAlchemist` | 1 122 | 6 états, 2 événements, 3 remplacements (`decide`, `getRecipe`, `checkForItems`, `craft`) |
//! | boulanger | `EntityAIWorkBaker` + `AbstractEntityAIRequestSmelter` | 83 + 760 | 4 états, 3 événements, 1 remplacement (`executeCraftingAction`) |
//! | forgeron | `EntityAIWorkBlacksmith` | 33 | **rien** |
//!
//! **Deux prémisses de la consigne sont fausses, et le port le dit au lieu de la
//! contourner.** (1) `EntityAIWorkBaker` n'hérite pas d'`AbstractEntityAICrafting` :
//! il hérite d'`AbstractEntityAIRequestSmelter` (`EntityAIWorkBaker.java:25`), et
//! c'est de là que viennent ses quatre états de four. (2) `EntityAIWorkBlacksmith`
//! fait 33 lignes et n'ajoute que `getExpectedBuildingClass` : sa machine à états
//! **est** celle de `AbstractEntityAICrafting`, et [`smith`] le démontre au lieu de
//! la réécrire.
//!
//! ## Ce qui existait déjà, et où ce module se raccorde
//!
//! `crate::ai_build::crafter` porte `AbstractEntityAICrafting` : les sept états
//! `IDLE` / `START_WORKING` / `QUERY_ITEMS` / `GET_RECIPE` / `CRAFT` /
//! `INVENTORY_FULL` / `GATHERING_REQUIRED_MATERIALS`, la résolution des matériaux de
//! `getRecipe` et les hits de `craft`. Ce module **ne les rejoue pas** : la table
//! commune de [`state::register_shared`] appelle `crafter::decide`,
//! `crafter::check_for_items`, `crafter::get_recipe`, `crafter::craft` et
//! `crafter::execute`, et un test (`state::tests::the_shared_table_answers_like_the_crafter`)
//! vérifie que les deux réponses coïncident sur la même scène. Ce qui est réécrit ici
//! est la *cible* (`Transition::target`), pas la règle : `Machine<S, F>` est
//! monomorphe en `S` et en `F`, donc une machine de métier ne peut pas contenir la
//! machine du crafter, qui tique sur `CraftFacts` seul.
//!
//! ```ignore
//! use ported_minecolonies::ai_work_01::{Trade, WorkAi};
//!
//! let mut ai = WorkAi::new(Trade::Alchemist);
//! let mut scene = ai.scene();                       // les faits du métier, vides
//! let outcome = ai.tick(&mut scene);
//! match (outcome.goal(), outcome.why_not()) {
//!     (WorkState::HarvestNetherWart, _) => /* récolter une verrue */,
//!     (_, Some(WorkRefusal::NoBrewingStand)) => /* la hutte n'a pas d'alambic */,
//!     _ => {}
//! }
//! ```
//!
//! ## Les durées, chacune à sa ligne Java
//!
//! | Durée | Valeur | Source |
//! | --- | ---: | --- |
//! | `TICKS_SECOND` | 20 | `Constants.java:21` |
//! | `STANDARD_DELAY` | 5 | `AbstractEntityAIBasic.java:100` |
//! | `STACKSIZE` | 64 | `Constants.java:23` |
//! | passe-temps de l'alchimiste | 400 | `EntityAIWorkAlchemist.java:326` (`TICKS_20 * 20`) |
//! | tirages de récolte | 30 | `EntityAIWorkAlchemist.java:71`, `:76` |
//! | chance du gui | 1/40 | `EntityAIWorkAlchemist.java:286` |
//! | requête de verrue | 16 (min 1) | `EntityAIWorkAlchemist.java:145` |
//! | carburant d'alambic | 8 | `Constants.java:158` |
//! | accélération (alambic et four) | `(niveau / 10) * 2` | `EntityAIWorkAlchemist.java:473`, `AbstractEntityAIRequestSmelter.java:122` |
//! | hit d'atelier | 10 | `AbstractEntityAICrafting.java:73` (`work_time::CRAFT_HIT_DELAY`) |
//! | événements de carburant | 200 | `EntityAIWorkAlchemist.java:105`, `AbstractEntityAIRequestSmelter.java:73` |
//!
//! ## Ce qui est délibérément absent
//!
//! - **Le monde.** Poser une potion, casser une verrue, frapper une feuille, faire
//!   tourner un four : des faits, jamais des appels. `MANQUANT.md` liste les trois
//!   trous d'API que cela laisse.
//! - **Le système de requêtes.** Créer une `Stack`, créer une `StackList`, marquer
//!   une requête acceptée : `crate::requests`. Ici c'est une décision
//!   (`Fuel::Asked`, `FuelCheck::Asked`) et un fait (`open_fuel_request`).
//! - **La recette.** `crate::crafting` et `crate::ai_build::crafter` la portent ; ce
//!   module lit `CraftFacts` et n'ouvre aucun fichier de recette.
//! - **Les 16 autres métiers d'atelier** (forgeron d'armes, maçon, teinturier…) : ils
//!   descendent de la même classe mère et n'ont, comme le forgeron, rien à ajouter.
//!   `RAPPORT.md` §3 les compte.

pub mod brewing;
pub mod facts;
pub mod furnace;
pub mod harvest;
pub mod smith;
pub mod state;

#[cfg(test)]
mod tests;

use core::fmt;

use crate::ai::{Fired, Machine};
use crate::ai_build::crafter::{CraftFacts, CraftState};

use facts::TradeFacts;
use state::{Scene, WorkOutcome};

pub use brewing::{
    acceleration_ticks as brewing_acceleration_ticks, able_to_brew, add_fuel as stoke_brewing_stand,
    check_for_items as brewing_items, check_fuel, count_of_bubbling, craft as brew_craft,
    decide as brewing_decide, fill_up as fill_brewing_stand, get_recipe as brewing_recipe,
    fuel_needed, max_usable_stands, retrieve_brewed, retrieve_used_fuel, stand_to_retrieve_from,
    Able, BrewCraft, BrewItems, BrewRecipe, Decide as BrewDecide, Fill as BrewFill, Fuel, Retrieve,
    Retrieval, Stoke as BrewStoke, DELAY_TO_HARVEST_MISTLETOE, DELAY_TO_HARVEST_NETHERWART,
    MISTLETOE_CHANCE, NETHER_WART_REQUEST, NETHER_WART_REQUEST_MIN, TICKS_20_TIMES_20,
};
pub use facts::{
    Above, AlchemistFacts, BakerFacts, Furnace, Leaf, Soil, Stack, Stand, TradeFacts as Facts,
    BREWING_FUEL_SLOT, BREWING_MIN_FUEL_COUNT, BREWING_SLOTS, FUEL_SLOT, INGREDIENT_SLOT,
    POTION_SLOTS, RESULT_SLOT, SMELTABLE_SLOT,
};
pub use furnace::{
    acceleration_ticks as furnace_acceleration_ticks, add_fuel as fuel_furnace,
    check_furnace_fuel, count_of_burning, empty_furnace_with_fuel, execute as smelt_execution,
    execute_crafting_action, fill_up as fill_furnaces, furnace_to_retrieve_output_from,
    furnace_with_unrelated_input, furnace_without_fuel, left_overs, max_usable_furnaces,
    retrieve_product, retrieve_unrelated, Execute, Fill as FurnaceFill, FuelCheck, Product, Stoke as FurnaceStoke,
    Unrelated,
};
pub use harvest::{harvest_mistletoe, harvest_nether_wart, Mistletoe, Wart};
pub use smith::{ACTIONS_UNTIL_DUMPING, BUILDING as BLACKSMITH_BUILDING, CLASS as BLACKSMITH_CLASS};
pub use state::{
    craft_state, has_work_to_do, Decision, WorkMemory, WorkOutcome as Outcome, WorkRefusal, WorkState,
};

/// `Constants.TICKS_SECOND` (`Constants.java:21`) : le rythme qu'une seconde de jeu
/// donne, et celui des tables de métier (`EntityAIWorkAlchemist.java:107-112`).
pub const TICKS_SECOND: u32 = 20;

/// `AbstractEntityAIBasic.STANDARD_DELAY` (`:100`) : le délai par défaut entre deux
/// actions, celui de `START_WORKING` (`AbstractEntityAICrafting.java:166`).
pub const STANDARD_DELAY: u32 = crate::ai::worker::STANDARD_DELAY;

/// `Constants.STACKSIZE` (`Constants.java:23`) : un stack plein, la quantité qu'une
/// demande de carburant vise (`EntityAIWorkAlchemist.java:597`,
/// `AbstractEntityAIRequestSmelter.java:184`).
pub const STACKSIZE: u32 = 64;

/// Les trois métiers de ce module.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: les trois classes que cette tranche porte
pub enum Trade {
    /// `EntityAIWorkAlchemist` (`crafting/EntityAIWorkAlchemist.java:65`).
    Alchemist,
    /// `EntityAIWorkBaker` (`crafting/EntityAIWorkBaker.java:25`), sur
    /// `AbstractEntityAIRequestSmelter`.
    Baker,
    /// `EntityAIWorkBlacksmith` (`crafting/EntityAIWorkBlacksmith.java:19`).
    Blacksmith,
}

impl Trade {
    /// Les trois, dans l'ordre du briefing.
    pub const ALL: [Trade; 3] = [Trade::Alchemist, Trade::Baker, Trade::Blacksmith];

    /// Le nom de la classe Java.
    pub fn class(self) -> &'static str {
        match self {
            Trade::Alchemist => "EntityAIWorkAlchemist",
            Trade::Baker => "EntityAIWorkBaker",
            Trade::Blacksmith => smith::CLASS,
        }
    }

    /// La hutte que la classe attend, `getExpectedBuildingClass()`.
    pub fn building(self) -> &'static str {
        match self {
            Trade::Alchemist => "BuildingAlchemist", // `:352-355`
            Trade::Baker => "BuildingBaker",         // `EntityAIWorkBaker.java:44-48`
            Trade::Blacksmith => smith::BUILDING,    // `:110-114`
        }
    }

    /// Le nom du métier, pour une clé de message.
    pub fn name(self) -> &'static str {
        match self {
            Trade::Alchemist => "alchemist",
            Trade::Baker => "baker",
            Trade::Blacksmith => "blacksmith",
        }
    }

    /// Les états propres au métier, dans l'ordre de `AIWorkerState`.
    pub fn own_states(self) -> &'static [WorkState] {
        match self {
            Trade::Alchemist => &[
                WorkState::FillUpBrewingStand,
                WorkState::RetrieveBrewed,
                WorkState::RetrieveUsedFuel,
                WorkState::AddFuelToBrewingStand,
                WorkState::HarvestMistletoe,
                WorkState::HarvestNetherWart,
            ],
            Trade::Baker => &[
                WorkState::FillUpFurnaces,
                WorkState::RetrieveProductFromFurnace,
                WorkState::RetrieveUnrelatedProduct,
                WorkState::AddFuelToFurnace,
            ],
            Trade::Blacksmith => &[],
        }
    }

    /// L'état où `IDLE` envoie, `START_WORKING` pour les trois
    /// (`AbstractEntityAICrafting.java:164`).
    pub fn entry(self) -> WorkState {
        craft_state(CraftState::StartWorking)
    }

    /// Les faits du métier, vides.
    pub fn facts(self) -> TradeFacts {
        match self {
            Trade::Alchemist => TradeFacts::Alchemist(facts::AlchemistFacts::default()),
            Trade::Baker => TradeFacts::Baker(facts::BakerFacts::default()),
            Trade::Blacksmith => TradeFacts::Blacksmith,
        }
    }

    /// Le métier d'un jeu de faits.
    pub fn of_facts(job: &TradeFacts) -> Trade {
        match job {
            TradeFacts::Alchemist(_) => Trade::Alchemist,
            TradeFacts::Baker(_) => Trade::Baker,
            TradeFacts::Blacksmith => Trade::Blacksmith,
        }
    }

    /// Enregistre la table du métier : la commune, puis ses propres cibles.
    pub fn register(self, machine: &mut Machine<WorkState, Scene>) {
        match self {
            Trade::Alchemist => brewing::register(machine),
            Trade::Baker => furnace::register(machine),
            Trade::Blacksmith => smith::register(machine),
        }
    }
}

impl fmt::Display for Trade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Une IA de métier : un métier et sa machine, rien d'autre.
#[derive(Debug)]
pub struct WorkAi {
    trade: Trade,
    machine: Machine<WorkState, Scene>,
}

impl WorkAi {
    /// La machine du métier, dans `IDLE` au rythme de l'IA d'entité
    /// (`AbstractEntityCitizen.java:64`).
    pub fn new(trade: Trade) -> WorkAi {
        let mut machine = state::machine();
        trade.register(&mut machine);
        WorkAi { trade, machine }
    }

    /// Le métier.
    pub fn trade(&self) -> Trade {
        self.trade
    }

    /// L'état courant (`AbstractAISkeleton.getState`, `:120-126`).
    pub fn state(&self) -> WorkState {
        self.machine.state()
    }

    /// La machine, pour un appelant qui veut lire la table.
    pub fn machine(&self) -> &Machine<WorkState, Scene> {
        &self.machine
    }

    /// `AbstractAISkeleton.resetAI` (`:145-152`).
    pub fn reset(&mut self) {
        self.machine.reset();
    }

    /// Une scène sur les faits vides de ce métier.
    pub fn scene(&self) -> Scene {
        Scene::new(CraftFacts::default(), self.trade.facts())
    }

    /// Une scène sur `base` et les faits du métier, vides.
    pub fn scene_with(&self, base: CraftFacts) -> Scene {
        Scene::new(base, self.trade.facts())
    }

    /// Un tick (`AbstractAISkeleton.tick`, `:63-66`).
    pub fn tick(&mut self, scene: &mut Scene) -> WorkOutcome {
        scene.refusal = None;
        let fired: Option<Fired<WorkState>> = self.machine.tick(scene);
        WorkOutcome {
            state: self.machine.state(),
            fired,
            refusal: scene.refusal,
        }
    }
}
