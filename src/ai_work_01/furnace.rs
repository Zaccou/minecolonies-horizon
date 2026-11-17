//! Les buts du boulanger : ses fours, son carburant, ce qu'il en sort.
//!
//! Référence Java : `EntityAIWorkBaker.java` (83 lignes, dont l'essentiel est le
//! `extends`) et sa vraie classe mère
//! `AbstractEntityAIRequestSmelter.java` — les cibles ajoutées (`:71-82`),
//! `checkForLeftOvers` (`:89-98`), `checkFurnaceFuel` (`:103-115`),
//! `accelerateFurnaces` (`:120-142`), `getExtendedCount` (`:145-176`),
//! `addFuelToFurnace` (`:178-225`), `retrieveProductFromFurnace` (`:227-283`),
//! `retrieveUnrelatedProductFromFurnace` (`:285-325`), `fillUpFurnace` (`:327-424`),
//! `executeCraftingAction` (`:427-503`) et les six chercheurs de position
//! (`:508-606`, `:631-731`).
//!
//! **Le boulanger n'est pas un crafter comme les autres.** Il hérite
//! d'`AbstractEntityAICrafting` mais remplace `executeCraftingAction` : un pain ne se
//! frappe pas, il cuit. Donc `CRAFT` ne fait pas la recette, il choisit un four
//! ([`execute_crafting_action`]) — et retombe sur la table de travail si la recette
//! n'est pas une cuisson (`:434-437`).

use crate::ai::Pos;
use crate::ai_build::crafter::{CraftFacts, CraftState};

use super::facts::{BakerFacts, Furnace, FUEL_SLOT, RESULT_SLOT, SMELTABLE_SLOT};
use super::state::{craft_state, Scene, WorkRefusal, WorkState};
use super::{STANDARD_DELAY, STACKSIZE, TICKS_SECOND};

/// `accelerateFurnaces`'s formula (`:122`) : `(second métier / 10) * 2` ticks de four
/// offerts par seconde.
pub fn acceleration_ticks(secondary_skill: u8) -> u32 {
    (u32::from(secondary_skill) / 10) * 2
}

/// `countOfBurningFurnaces` (`:648-665`).
pub fn count_of_burning(facts: &BakerFacts) -> usize {
    facts.loaded().filter(|furnace| furnace.is_burning()).count()
}

/// `getMaxUsableFurnaces` (`:673-692`) : `min(niveau / 10 + 1, fours qui ont du
/// carburant)` — le second terme n'est pas le nombre de fours, mais le nombre de fours
/// **allumables**.
pub fn max_usable_furnaces(facts: &BakerFacts) -> usize {
    let by_skill = usize::from(facts.primary_skill) / 10 + 1;
    let fuelled = facts
        .loaded()
        .filter(|furnace| !furnace.is_empty(FUEL_SLOT))
        .count();
    by_skill.min(fuelled)
}

/// `getFurnaceToRetrieveOutputFrom` (`:551-578`) : le premier four qui ne cuit pas et
/// dont le slot de résultat porte quelque chose.
///
/// Java court-circuite quand le résultat est plein (`:559-562`) puis retombe sur le
/// même test (`:564-567`) : les deux branches rendent la même position.
pub fn furnace_to_retrieve_output_from(facts: &BakerFacts) -> Option<Pos> {
    facts
        .loaded()
        .find(|furnace| {
            (!furnace.lit || furnace.is_empty(SMELTABLE_SLOT))
                && furnace.count_at(RESULT_SLOT) > 0
        })
        .map(|furnace| furnace.at)
}

/// `getFurnaceWithoutFuel` (`:531-548`) : le premier four éteint dont le slot de
/// carburant est vide.
pub fn furnace_without_fuel(facts: &BakerFacts) -> Option<Pos> {
    facts
        .loaded()
        .find(|furnace| !furnace.lit && furnace.is_empty(FUEL_SLOT))
        .map(|furnace| furnace.at)
}

/// `getEmptyFurnaceWithFuel` (`:508-527`) : le premier four vide, sans résultat, mais
/// avec du carburant.
pub fn empty_furnace_with_fuel(facts: &BakerFacts) -> Option<Pos> {
    facts
        .loaded()
        .find(|furnace| {
            furnace.is_empty(SMELTABLE_SLOT)
                && furnace.is_empty(RESULT_SLOT)
                && !furnace.is_empty(FUEL_SLOT)
        })
        .map(|furnace| furnace.at)
}

/// `getFurnaceToRetrieveUnrelatedInputFrom` (`:581-606`) : un four qui tient autre
/// chose que l'entrée de la recette courante.
pub fn furnace_with_unrelated_input(facts: &BakerFacts) -> Option<Pos> {
    facts
        .loaded()
        .find(|furnace| {
            !furnace.is_empty(SMELTABLE_SLOT)
                && furnace.key_at(SMELTABLE_SLOT) != Some(&facts.input)
        })
        .map(|furnace| furnace.at)
}

/// `getActivePossibleFuels` (`:710-731`) : les carburants autorisés, **moins** la
/// sortie et l'entrée de la recette courante (« pour éviter toute interférence »,
/// `:705`). La liste vide est [`WorkRefusal::NoAllowedFuel`].
pub fn active_possible_fuels(facts: &BakerFacts) -> u32 {
    // Les deux `removeIf` de `:723-728` retirent des stacks, pas un compte : un fait
    // qui dit combien de carburants restent est donc la seule réponse honnête ici.
    facts.allowed_fuels as u32 // `:710-720`
}

/// Ce que `checkForLeftOvers` (`:89-98`) demande : la condition de l'événement.
pub fn left_overs(facts: &BakerFacts) -> Option<Pos> {
    furnace_with_unrelated_input(facts) // `:91-95`
}

/// Ce que `checkFurnaceFuel` (`:103-115`) décide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: `checkFurnaceFuel` ne rend jamais vrai, il demande ou il ne fait rien
pub enum FuelCheck {
    /// La colonie a été priée de fournir un stack de carburant (`:111`).
    Asked {
        /// `STACKSIZE * nombre de fours` (`:111`).
        amount: u32,
    },
    /// Du carburant est en main, à la hutte, ou en route (`:107-114`).
    Nothing,
}

/// `checkFurnaceFuel` (`:103-115`).
pub fn check_furnace_fuel(facts: &BakerFacts) -> FuelCheck {
    if facts.allowed_fuels == 0 {
        // `getActivePossibleFuels` plaint le joueur (`:712-720`) : rien à demander.
        return FuelCheck::Nothing;
    }
    if !facts.fuel_in_inventory && facts.fuel_in_hut == 0 && !facts.open_fuel_request {
        return FuelCheck::Asked {
            amount: STACKSIZE.saturating_mul(facts.furnaces.len() as u32), // `:111`
        };
    }
    FuelCheck::Nothing // `:114`
}

/// Ce que `addFuelToFurnace` (`:178-225`) décide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les cinq sorties de `addFuelToFurnace`
pub enum Stoke {
    /// La hutte a du carburant, le citoyen non (`:181-186`).
    Gather,
    /// Plus de carburant nulle part (`:188-189`).
    Restart,
    /// `furnacePos` est nul (`:191-194`).
    NoFurnace,
    /// Pas encore au four (`:196-199`).
    Walking,
    /// Le stack est allé dans le slot 1 (`:201-220`).
    Stoked,
}

/// `addFuelToFurnace` (`:178-225`).
pub fn add_fuel(facts: &BakerFacts, memory: &super::state::WorkMemory) -> Stoke {
    if !facts.fuel_in_inventory {
        if facts.fuel_in_hut >= 1 {
            return Stoke::Gather; // `:181-186`
        }
        return Stoke::Restart; // `:188-189`
    }
    let Some(at) = memory.furnace_pos else {
        return Stoke::NoFurnace; // `:191-194`
    };
    if !facts.at_work_pos {
        return Stoke::Walking; // `:196-199`
    }
    match facts.furnace_at(at) {
        // `:203-220` : le carburant n'entre que dans un slot vide.
        Some(furnace) if furnace.loaded && furnace.is_empty(FUEL_SLOT) => Stoke::Stoked,
        _ => Stoke::Restart, // `:222-224`
    }
}

/// Ce que `retrieveProductFromFurnace` (`:227-283`) décide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les sept sorties de `retrieveProductFromFurnace`
pub enum Product {
    /// Plus de four, de recette ou de requête (`:229-232`).
    NoWork,
    /// Le four a disparu ou son résultat est vide (`:234-239`).
    FurnaceGone,
    /// Pas encore au four (`:241-244`).
    Walking,
    /// La sortie est sortie et comptée (`:251-274`).
    Delivered {
        /// `stack.getCount()` (`:252`).
        count: u32,
        /// `job.getCraftCounter() >= job.getMaxCraftingCount()` (`:267`).
        finalized: bool,
    },
    /// Ce n'était pas la sortie de la recette : elle part en sorties secondaires
    /// (`:275-278`).
    Secondary {
        /// La quantité sortie.
        count: u32,
    },
    /// Rien n'est sorti, ou le four n'est plus là (`:282`).
    Nothing,
}

/// `retrieveProductFromFurnace` (`:227-283`).
pub fn retrieve_product(
    base: &CraftFacts,
    facts: &BakerFacts,
    memory: &super::state::WorkMemory,
) -> Product {
    if memory.furnace_pos.is_none() || !base.has_recipe || !base.has_request {
        return Product::NoWork; // `:229-232`
    }
    let Some(at) = memory.furnace_pos else {
        return Product::NoWork;
    };
    let Some(furnace) = facts.furnace_at(at) else {
        return Product::FurnaceGone; // `:234-239`
    };
    if furnace.is_empty(RESULT_SLOT) {
        return Product::FurnaceGone; // `:235-238`
    }
    if !facts.at_work_pos {
        return Product::Walking; // `:241-244`
    }
    let count = furnace.count_at(RESULT_SLOT); // `:251`
    if count == 0 {
        return Product::Nothing; // `:252`, `:282`
    }
    if furnace.key_at(RESULT_SLOT) == Some(&facts.output) {
        let after = base.craft_counter.saturating_add(count); // `:260`
        let max = if base.max_crafting_count == 0 {
            base.request_count // `:262-265`
        } else {
            base.max_crafting_count
        };
        return Product::Delivered {
            count,
            finalized: after >= max, // `:267`
        };
    }
    Product::Secondary { count } // `:275-278`
}

/// Ce que `retrieveUnrelatedProductFromFurnace` (`:285-325`) décide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les cinq sorties de `retrieveUnrelatedProductFromFurnace`
pub enum Unrelated {
    /// Plus de position (`:287-290`).
    NoWork,
    /// Le four n'a plus rien, ou tient l'entrée de la recette (`:292-302`).
    NoLeftOver,
    /// Pas encore au four (`:304-307`).
    Walking,
    /// Ce qui n'était pas de la recette est sorti, et l'inventaire est plein
    /// (`:313-322`).
    Full {
        /// La quantité sortie.
        count: u32,
    },
    /// Rien n'est sorti (`:325`).
    Nothing,
}

/// `retrieveUnrelatedProductFromFurnace` (`:285-325`).
pub fn retrieve_unrelated(facts: &BakerFacts, memory: &super::state::WorkMemory) -> Unrelated {
    let Some(at) = memory.furnace_pos else {
        return Unrelated::NoWork; // `:287-290`
    };
    let Some(furnace) = facts.furnace_at(at) else {
        return Unrelated::NoLeftOver; // `:292-297`
    };
    if furnace.is_empty(SMELTABLE_SLOT) {
        return Unrelated::NoLeftOver; // `:294-297`
    }
    if furnace.key_at(SMELTABLE_SLOT) == Some(&facts.input) {
        return Unrelated::NoLeftOver; // `:298-302`
    }
    if !facts.at_work_pos {
        return Unrelated::Walking; // `:304-307`
    }
    let count = furnace.count_at(SMELTABLE_SLOT); // `:310`
    if count == 0 {
        return Unrelated::Nothing;
    }
    Unrelated::Full { count } // `:313-322`
}

/// Ce que `fillUpFurnace` (`:327-424`) décide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les huit sorties de `fillUpFurnace`
pub enum Fill {
    /// Plus de four ou de recette (`:329-332`).
    NoWork,
    /// Le compte demandé est déjà couvert par les fours (`:348-351`).
    NothingLeft,
    /// Rien à cuire en main, la hutte est sollicitée (`:353-358`).
    Gather {
        /// `targetCount` (`:346`, `:355`).
        need: u32,
    },
    /// Plus de fours qui cuit que la limite (`:360-365`).
    TooManyBurning,
    /// Pas encore au four (`:367-370`).
    Walking,
    /// Plus rien en attente (`:372-376`).
    Pending,
    /// `toTransfer` items sont allés dans le four (`:389-420`).
    Fed {
        /// Le compte transféré (`:395-408`).
        amount: u32,
    },
    /// Rien n'est entré : le four n'a pas de carburant (`:389`) ou tous les fours
    /// utilisables cuisent déjà (`:410`). Le tick rend `CRAFT` quand même (`:423`).
    NoTransfer,
}

/// `fillUpFurnace` (`:327-424`).
pub fn fill_up(base: &CraftFacts, facts: &BakerFacts, memory: &super::state::WorkMemory) -> Fill {
    let Some(at) = memory.furnace_pos else {
        return Fill::NoWork; // `:329-332`
    };
    if !base.has_recipe {
        return Fill::NoWork; // `:329-332`
    }
    let max = if base.max_crafting_count == 0 {
        base.request_count // `:334-337`
    } else {
        base.max_crafting_count
    };
    let smeltable_in_inventory = base
        .inputs
        .first()
        .map(|input| input.in_inventory)
        .unwrap_or(0); // `:341-342`
    let smeltable_in_furnaces = base
        .inputs
        .first()
        .map(|input| input.extended)
        .unwrap_or(0); // `:344`
    let result_in_furnaces = base.in_progress_output; // `:345`
    let pending = i64::from(max) - i64::from(base.craft_counter); // `:348`, `:372`
    if pending - i64::from(smeltable_in_furnaces) - i64::from(result_in_furnaces) <= 0 {
        return Fill::NothingLeft; // `:348-351`
    }
    if smeltable_in_inventory == 0 {
        return Fill::Gather {
            need: (pending - i64::from(smeltable_in_furnaces) - i64::from(result_in_furnaces))
                .max(0) as u32, // `:346`, `:355`
        };
    }
    let burning = count_of_burning(facts); // `:360`
    let usable = max_usable_furnaces(facts); // `:361`
    if burning > usable {
        return Fill::TooManyBurning; // `:362-365`
    }
    if !facts.at_work_pos {
        return Fill::Walking; // `:367-370`
    }
    let left = pending - i64::from(smeltable_in_furnaces) - i64::from(result_in_furnaces);
    if left <= 0 {
        return Fill::Pending; // `:372-376`
    }
    let available = u32::try_from(left).unwrap_or(u32::MAX).min(smeltable_in_inventory); // `:378`
    let Some(furnace) = facts.furnace_at(at) else {
        return Fill::Pending; // `:380-382`
    };
    if !furnace.has_fuel_and_no_smeltable() {
        return Fill::NoTransfer; // `:389`
    }
    // `:391-409` : le partage entre les fours disponibles, le reste en tête.
    let free = usable.saturating_sub(burning);
    if free == 0 {
        return Fill::NoTransfer;
    }
    let to_transfer = if available > STACKSIZE.saturating_mul(free as u32) {
        STACKSIZE
    } else if free == 1 {
        available
    } else {
        (available / free as u32 + available % free as u32).min(STACKSIZE)
    };
    if to_transfer == 0 {
        return Fill::NoTransfer; // `:410`
    }
    Fill::Fed {
        amount: to_transfer, // `:395-408`, `:413-418`
    }
}

/// Ce que `executeCraftingAction` (`:427-503`) demande pour une cuisson.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les onze sorties de l'override
pub enum Execute {
    /// Plus de recette (`:429-432`).
    NoRecipe,
    /// La recette n'est pas une cuisson : la table de travail (`:434-437`).
    NotFurnace,
    /// La hutte ne liste aucun four (`:439-452`).
    NoFurnace,
    /// Au moins un four n'est pas chargé (`:455-458`).
    NotLoaded,
    /// La hutte ne liste aucun carburant (`:462-470`).
    NoFuel,
    /// Un four a de la sortie (`:472-476`).
    Retrieve {
        /// Le four.
        at: Pos,
    },
    /// Un four tient autre chose que la recette (`:478-482`).
    ClearOut {
        /// Le four.
        at: Pos,
    },
    /// Du carburant est disponible et un four en manque (`:484-491`).
    Stoke {
        /// Le four.
        at: Pos,
    },
    /// Assez de fours cuisent déjà (`:493-497`).
    Burning,
    /// Un four vide attend sa matière (`:499-502`).
    Fill {
        /// Le four.
        at: Pos,
    },
    /// Rien de tout cela (`:503`).
    Restart,
}

/// `executeCraftingAction` (`:427-503`) pour une recette de four.
pub fn execute(base: &CraftFacts, facts: &BakerFacts) -> Execute {
    if !base.has_recipe {
        return Execute::NoRecipe; // `:429-432`
    }
    if !facts.furnace_recipe {
        return Execute::NotFurnace; // `:434-437`
    }
    if facts.furnaces.is_empty() {
        return Execute::NoFurnace; // `:439-452`
    }
    if !facts.furnaces_loaded {
        return Execute::NotLoaded; // `:455-458`
    }
    if active_possible_fuels(facts) == 0 {
        return Execute::NoFuel; // `:462-470`
    }
    if let Some(at) = furnace_to_retrieve_output_from(facts) {
        return Execute::Retrieve { at }; // `:472-476`
    }
    if let Some(at) = furnace_with_unrelated_input(facts) {
        return Execute::ClearOut { at }; // `:478-482`
    }
    if facts.fuel_in_hut > 1 || facts.fuel_in_inventory {
        if let Some(at) = furnace_without_fuel(facts) {
            return Execute::Stoke { at }; // `:484-491`
        }
    }
    let burning = count_of_burning(facts); // `:493`
    if burning > 0 && burning >= max_usable_furnaces(facts) {
        return Execute::Burning; // `:493-497`
    }
    if let Some(at) = empty_furnace_with_fuel(facts) {
        return Execute::Fill { at }; // `:499-502`
    }
    Execute::Restart // `:503`
}

/// Applique [`execute`] : c'est ici que les positions entrent dans la mémoire et que
/// les refus sont nommés.
pub fn execute_crafting_action(scene: &mut Scene) -> Option<WorkState> {
    let answer = scene
        .baker()
        .map(|facts| execute(&scene.base, facts));
    let Some(answer) = answer else {
        scene.refuse(WorkRefusal::NoFactsForTrade);
        return None;
    };
    match answer {
        // `:434-437` : `super.executeCraftingAction(toolSlot)`, la table de travail.
        Execute::NotFurnace => {
            let decision = super::state::base_execute(scene);
            if let Some(refusal) = decision.refusal {
                scene.refuse(refusal);
            }
            decision
                .next
                .or(Some(craft_state(CraftState::Craft)))
        }
        Execute::NoRecipe => Some(craft_state(CraftState::StartWorking)), // `:431`
        Execute::NoFurnace => {
            scene.memory.delay = STANDARD_DELAY; // `:451`
            scene.refuse(WorkRefusal::NoFurnace);
            Some(craft_state(CraftState::StartWorking)) // `:452`
        }
        Execute::NotLoaded => {
            scene.refuse(WorkRefusal::FurnacesNotLoaded); // `:455-458`
            Some(craft_state(CraftState::StartWorking)) // `:458`
        }
        Execute::NoFuel => {
            scene.refuse(WorkRefusal::NoAllowedFuel); // `:465-469`
            None // `:470` (`return getState()`)
        }
        Execute::Retrieve { at } => {
            scene.memory.furnace_pos = Some(at); // `:474-475`
            Some(WorkState::RetrieveProductFromFurnace) // `:476`
        }
        Execute::ClearOut { at } => {
            scene.memory.furnace_pos = Some(at); // `:480-481`
            Some(WorkState::RetrieveUnrelatedProduct) // `:482`
        }
        Execute::Stoke { at } => {
            scene.memory.furnace_pos = Some(at); // `:488-489`
            Some(WorkState::AddFuelToFurnace) // `:490`
        }
        Execute::Burning => {
            scene.memory.delay = TICKS_SECOND; // `:495`
            None // `:496`
        }
        Execute::Fill { at } => {
            scene.memory.furnace_pos = Some(at); // `:501`
            Some(WorkState::FillUpFurnaces) // `:502`
        }
        Execute::Restart => Some(craft_state(CraftState::StartWorking)), // `:503`
    }
}

/// Les cibles du boulanger : les sept communes, les quatre siennes, et les trois
/// événements de fond (`AbstractEntityAIRequestSmelter.java:71-82`).
pub fn register(machine: &mut crate::ai::Machine<WorkState, Scene>) {
    super::state::register_shared(machine);
    // `:80` — `FILL_UP_FURNACES`.
    machine.add(crate::ai::Transition::target(
        WorkState::FillUpFurnaces,
        "fillUpFurnace",
        "AbstractEntityAIRequestSmelter.java:80",
        TICKS_SECOND,
        crate::ai::Action::Run(fill_action),
    ));
    // `:81` — `ADD_FUEL_TO_FURNACE`.
    machine.add(crate::ai::Transition::target(
        WorkState::AddFuelToFurnace,
        "addFuelToFurnace",
        "AbstractEntityAIRequestSmelter.java:81",
        TICKS_SECOND,
        crate::ai::Action::Run(stoke_action),
    ));
    // `:78` — `RETRIEVING_END_PRODUCT_FROM_FURNACE`.
    machine.add(crate::ai::Transition::target(
        WorkState::RetrieveProductFromFurnace,
        "retrieveProductFromFurnace",
        "AbstractEntityAIRequestSmelter.java:78",
        TICKS_SECOND,
        crate::ai::Action::Run(product_action),
    ));
    // `:79` — `RETRIEVING_UNRELATED_PRODUCT_FROM_FURNACE`.
    machine.add(crate::ai::Transition::target(
        WorkState::RetrieveUnrelatedProduct,
        "retrieveUnrelatedProductFromFurnace",
        "AbstractEntityAIRequestSmelter.java:79",
        TICKS_SECOND,
        crate::ai::Action::Run(unrelated_action),
    ));
    // `:73` — l'événement de carburant, toutes les dix secondes.
    machine.add(crate::ai::Transition::event(
        crate::ai::Blocking::Event,
        "checkFurnaceFuel",
        "AbstractEntityAIRequestSmelter.java:73",
        TICKS_SECOND * 10,
        crate::ai::Action::Run(fuel_event_action),
    ));
    // `:74` — l'accélération, toutes les secondes.
    machine.add(crate::ai::Transition::event(
        crate::ai::Blocking::Event,
        "accelerateFurnaces",
        "AbstractEntityAIRequestSmelter.java:74",
        TICKS_SECOND,
        crate::ai::Action::Run(accelerate_action),
    ));
    // `:75` — les restes d'une autre recette, et il **change** d'état.
    machine.add(crate::ai::Transition::event_when(
        crate::ai::Blocking::Event,
        "checkForLeftOvers",
        "AbstractEntityAIRequestSmelter.java:75",
        TICKS_SECOND * 10,
        |scene: &Scene, _: WorkState| match scene.baker() {
            Some(facts) => left_overs(facts).is_some(),
            None => false,
        },
        crate::ai::Action::Run(left_overs_action),
    ));
}

/// `:75` — `checkForLeftOvers`.
fn left_overs_action(scene: &mut Scene, state: WorkState) -> Option<WorkState> {
    let Some(at) = scene.baker().and_then(left_overs) else {
        return Some(state);
    };
    scene.memory.furnace_pos = Some(at); // `:94`
    Some(WorkState::RetrieveUnrelatedProduct) // `:75`
}

/// `:73` — `checkFurnaceFuel`.
fn fuel_event_action(scene: &mut Scene, state: WorkState) -> Option<WorkState> {
    let answer = scene.baker().map(check_furnace_fuel);
    let Some(answer) = answer else {
        scene.refuse(WorkRefusal::NoFactsForTrade);
        return Some(state);
    };
    match answer {
        // `:111` : la requête est créée, l'état ne bouge pas (`return false`).
        FuelCheck::Asked { .. } | FuelCheck::Nothing => Some(state),
    }
}

/// `:74` — `accelerateFurnaces` : un effet, jamais une transition (`:141`).
fn accelerate_action(_scene: &mut Scene, state: WorkState) -> Option<WorkState> {
    Some(state)
}

/// `:80` — `fillUpFurnace`.
fn fill_action(scene: &mut Scene, state: WorkState) -> Option<WorkState> {
    let answer = scene
        .baker()
        .map(|facts| fill_up(&scene.base, facts, &scene.memory));
    let Some(answer) = answer else {
        scene.refuse(WorkRefusal::NoFactsForTrade);
        return Some(state);
    };
    match answer {
        Fill::NoWork => Some(craft_state(CraftState::StartWorking)), // `:331`
        Fill::NothingLeft => {
            scene.refuse(WorkRefusal::NothingPending); // `:348-351`
            Some(craft_state(CraftState::StartWorking))
        }
        Fill::Gather { need } => {
            scene.memory.need = Some(need); // `:355`
            scene.memory.furnace_pos = None; // `:356`
            scene.refuse(WorkRefusal::NoSmeltable); // `:353-358`
            Some(craft_state(CraftState::GatheringRequiredMaterials)) // `:357`
        }
        Fill::TooManyBurning => Some(craft_state(CraftState::StartWorking)), // `:364`
        Fill::Walking => {
            scene.refuse(WorkRefusal::NotAtWorkPos); // `:367-370`
            Some(state)
        }
        Fill::Pending => Some(craft_state(CraftState::StartWorking)), // `:375`
        Fill::Fed { .. } => Some(craft_state(CraftState::Craft)), // `:423`
        Fill::NoTransfer => Some(craft_state(CraftState::Craft)), // `:389`, `:423`
    }
}

/// `:81` — `addFuelToFurnace`.
fn stoke_action(scene: &mut Scene, state: WorkState) -> Option<WorkState> {
    let answer = scene
        .baker()
        .map(|facts| add_fuel(facts, &scene.memory));
    let Some(answer) = answer else {
        scene.refuse(WorkRefusal::NoFactsForTrade);
        return Some(state);
    };
    match answer {
        Stoke::Gather => {
            scene.memory.need = Some(STACKSIZE); // `:184`
            Some(craft_state(CraftState::GatheringRequiredMaterials)) // `:185`
        }
        Stoke::Restart => {
            scene.memory.furnace_pos = None; // `:188`, `:222`
            scene.refuse(WorkRefusal::NoAllowedFuel);
            Some(craft_state(CraftState::StartWorking)) // `:189`, `:224`
        }
        Stoke::NoFurnace => Some(craft_state(CraftState::StartWorking)), // `:193`
        Stoke::Walking => {
            scene.refuse(WorkRefusal::NotAtWorkPos); // `:198`
            Some(state)
        }
        Stoke::Stoked => {
            scene.memory.furnace_pos = None; // `:222`
            Some(craft_state(CraftState::StartWorking)) // `:224`
        }
    }
}

/// `:78` — `retrieveProductFromFurnace`.
fn product_action(scene: &mut Scene, state: WorkState) -> Option<WorkState> {
    let answer = scene
        .baker()
        .map(|facts| retrieve_product(&scene.base, facts, &scene.memory));
    let Some(answer) = answer else {
        scene.refuse(WorkRefusal::NoFactsForTrade);
        return Some(state);
    };
    match answer {
        Product::NoWork | Product::FurnaceGone | Product::Nothing => {
            scene.memory.furnace_pos = None; // `:237`, `:246`
            Some(craft_state(CraftState::StartWorking)) // `:231`, `:238`, `:282`
        }
        Product::Walking => {
            scene.refuse(WorkRefusal::NotAtWorkPos); // `:243`
            Some(state)
        }
        Product::Delivered { count, finalized } => {
            scene.base.craft_counter = scene.base.craft_counter.saturating_add(count); // `:260`
            if scene.base.max_crafting_count == 0 {
                scene.base.max_crafting_count = scene.base.request_count; // `:262-265`
            }
            scene.memory.furnace_pos = None; // `:246`
            if finalized {
                Some(craft_state(CraftState::InventoryFull)) // `:268-272`
            } else {
                Some(craft_state(CraftState::StartWorking)) // `:282`
            }
        }
        Product::Secondary { .. } => {
            scene.memory.furnace_pos = None; // `:246`
            Some(craft_state(CraftState::StartWorking)) // `:282`
        }
    }
}

/// `:79` — `retrieveUnrelatedProductFromFurnace`.
fn unrelated_action(scene: &mut Scene, state: WorkState) -> Option<WorkState> {
    let answer = scene
        .baker()
        .map(|facts| retrieve_unrelated(facts, &scene.memory));
    let Some(answer) = answer else {
        scene.refuse(WorkRefusal::NoFactsForTrade);
        return Some(state);
    };
    match answer {
        Unrelated::Walking => {
            scene.refuse(WorkRefusal::NotAtWorkPos); // `:306`
            Some(state)
        }
        Unrelated::Full { .. } => {
            scene.memory.furnace_pos = None; // `:311`
            Some(craft_state(CraftState::InventoryFull)) // `:319`
        }
        Unrelated::NoWork | Unrelated::NoLeftOver | Unrelated::Nothing => {
            scene.memory.furnace_pos = None; // `:298`, `:311`
            Some(craft_state(CraftState::StartWorking)) // `:289`, `:296`, `:325`
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_build::crafter::Input;
    use crate::ai_work_01::facts::{BakerFacts, NeedKey, Stack, TradeFacts};
    use crate::ai_work_01::state::WorkMemory;

    fn facts() -> BakerFacts {
        BakerFacts::default()
    }

    fn busy() -> CraftFacts {
        CraftFacts {
            task_queue: 1,
            current_task: true,
            ..CraftFacts::default()
        }
    }

    fn foreign_key() -> NeedKey {
        NeedKey::plain("minecraft:cobblestone")
    }

    fn with_furnace(at: Pos) -> BakerFacts {
        let mut facts = facts();
        facts.furnaces.push(Furnace::new(at));
        facts
    }

    /// `:655-692` : le plafond du niveau, puis le nombre de fours **qui ont du
    /// carburant** — pas le nombre de fours.
    #[test]
    fn the_usable_furnace_count_is_the_minimum_of_two_counts() {
        let mut facts = facts();
        facts.primary_skill = 10; // 10/10 + 1 = 2
        for x in 0..3 {
            let mut furnace = Furnace::new(Pos::new(x, 64, 1));
            if x < 2 {
                furnace.slots[FUEL_SLOT] = Some(Stack::new(NeedKey::plain("minecraft:coal"), 8));
            }
            facts.furnaces.push(furnace);
        }
        assert_eq!(max_usable_furnaces(&facts), 2);
        facts.primary_skill = 40; // 5, mais deux fours seulement ont du carburant
        assert_eq!(max_usable_furnaces(&facts), 2);
        let none = BakerFacts::default();
        assert_eq!(max_usable_furnaces(&none), 0);
    }

    /// `:648-665` : cuire demande **allumé et chargé** — un four allumé vide ne compte
    /// pas.
    #[test]
    fn a_burning_furnace_is_lit_and_loaded() {
        let mut facts = facts();
        let mut lit = Furnace::new(Pos::new(1, 64, 1));
        lit.lit = true;
        facts.furnaces.push(lit);
        assert_eq!(count_of_burning(&facts), 0);
        let input = facts.input.clone();
        facts.furnaces[0].slots[SMELTABLE_SLOT] = Some(Stack::new(input, 1));
        assert_eq!(count_of_burning(&facts), 1);
        facts.furnaces[0].loaded = false;
        assert_eq!(count_of_burning(&facts), 0, "`:508`, un four déchargé n'est pas lu");
    }

    /// `:508-606` : les quatre chercheurs de position, un par question.
    #[test]
    fn the_four_position_searches_do_not_overlap() {
        let mut facts = facts();
        let mut output = Furnace::new(Pos::new(1, 64, 1));
        output.slots[RESULT_SLOT] = Some(Stack::new(facts.output.clone(), 2));
        facts.furnaces.push(output);
        let mut foreign = Furnace::new(Pos::new(2, 64, 1));
        foreign.slots[SMELTABLE_SLOT] = Some(Stack::new(foreign_key(), 3));
        facts.furnaces.push(foreign);
        let mut fuelled = Furnace::new(Pos::new(3, 64, 1));
        fuelled.slots[FUEL_SLOT] = Some(Stack::new(NeedKey::plain("minecraft:coal"), 1));
        facts.furnaces.push(fuelled);
        assert_eq!(
            furnace_to_retrieve_output_from(&facts),
            Some(Pos::new(1, 64, 1))
        );
        assert_eq!(
            furnace_with_unrelated_input(&facts),
            Some(Pos::new(2, 64, 1)),
            "`:589-592`"
        );
        assert_eq!(
            furnace_without_fuel(&facts),
            Some(Pos::new(1, 64, 1)),
            "`!lit` et carburant vide (`:533`)"
        );
        assert_eq!(
            empty_furnace_with_fuel(&facts),
            Some(Pos::new(3, 64, 1))
        );
        // Le four qui tient l'entrée de la recette n'est pas un reste (`:298-302`).
        let recipe_input = facts.input.clone();
        facts.furnaces[1].slots[SMELTABLE_SLOT] = Some(Stack::new(recipe_input, 3));
        assert_eq!(furnace_with_unrelated_input(&facts), None);
    }

    /// `:107-114` : la demande de carburant part quand ni le citoyen, ni la hutte,
    /// ni une requête ouverte n'en ont.
    #[test]
    fn the_fuel_request_covers_the_whole_hut() {
        let mut facts = with_furnace(Pos::new(1, 64, 1));
        facts.furnaces.push(Furnace::new(Pos::new(2, 64, 1)));
        assert_eq!(
            check_furnace_fuel(&facts),
            FuelCheck::Asked {
                amount: STACKSIZE * 2
            }
        );
        facts.open_fuel_request = true;
        assert_eq!(check_furnace_fuel(&facts), FuelCheck::Nothing);
        facts.open_fuel_request = false;
        facts.fuel_in_hut = 1;
        assert_eq!(check_furnace_fuel(&facts), FuelCheck::Nothing);
        let no_fuel_listed = BakerFacts {
            allowed_fuels: 0,
            ..facts
        };
        assert_eq!(
            check_furnace_fuel(&no_fuel_listed),
            FuelCheck::Nothing,
            "une hutte sans liste de carburant n'en demande pas (`:712-720`)"
        );
    }

    /// `:181-224` : quatre sorties du ravitaillement en carburant.
    #[test]
    fn fuelling_a_furnace_needs_the_fuel_in_hand_and_an_empty_slot() {
        let at = Pos::new(1, 64, 1);
        let facts = with_furnace(at);
        assert_eq!(add_fuel(&facts, &WorkMemory::default()), Stoke::Restart);
        let mut in_hand = facts.clone();
        in_hand.fuel_in_inventory = true;
        assert_eq!(
            add_fuel(&in_hand, &WorkMemory::default()),
            Stoke::NoFurnace
        );
        let memory = WorkMemory {
            furnace_pos: Some(at),
            ..WorkMemory::default()
        };
        assert_eq!(add_fuel(&in_hand, &memory), Stoke::Stoked);
        let mut away = in_hand.clone();
        away.at_work_pos = false;
        assert_eq!(add_fuel(&away, &memory), Stoke::Walking);
        let mut hut = facts.clone();
        hut.fuel_in_hut = 4;
        assert_eq!(add_fuel(&hut, &memory), Stoke::Gather);
    }

    /// `:229-278` : la sortie de la recette se compte en une fois, une autre sortie
    /// part en secondaire.
    #[test]
    fn a_product_comes_back_in_one_count_or_not_at_all() {
        let at = Pos::new(1, 64, 1);
        let mut facts = with_furnace(at);
        let output = facts.output.clone();
        facts.furnaces[0].slots[RESULT_SLOT] = Some(Stack::new(output, 5));
        let memory = WorkMemory {
            furnace_pos: Some(at),
            ..WorkMemory::default()
        };
        let base = CraftFacts {
            has_recipe: true,
            has_request: true,
            max_crafting_count: 5,
            ..busy()
        };
        assert_eq!(
            retrieve_product(&base, &facts, &memory),
            Product::Delivered {
                count: 5,
                finalized: true
            }
        );
        let other = CraftFacts {
            max_crafting_count: 9,
            ..base.clone()
        };
        assert_eq!(
            retrieve_product(&other, &facts, &memory),
            Product::Delivered {
                count: 5,
                finalized: false
            }
        );
        facts.furnaces[0].slots[RESULT_SLOT] = Some(Stack::new(foreign_key(), 2));
        assert_eq!(
            retrieve_product(&base, &facts, &memory),
            Product::Secondary { count: 2 }
        );
        assert_eq!(
            retrieve_product(&CraftFacts::default(), &facts, &memory),
            Product::NoWork
        );
    }

    /// `:292-322` : un reste d'une autre recette vide l'inventaire ; l'entrée de la
    /// recette courante n'est pas un reste.
    #[test]
    fn leftovers_of_another_recipe_fill_the_inventory() {
        let at = Pos::new(1, 64, 1);
        let mut facts = with_furnace(at);
        facts.furnaces[0].slots[SMELTABLE_SLOT] = Some(Stack::new(foreign_key(), 3));
        let memory = WorkMemory {
            furnace_pos: Some(at),
            ..WorkMemory::default()
        };
        assert_eq!(
            retrieve_unrelated(&facts, &memory),
            Unrelated::Full { count: 3 }
        );
        let recipe_input = facts.input.clone();
        facts.furnaces[0].slots[SMELTABLE_SLOT] = Some(Stack::new(recipe_input, 3));
        assert_eq!(retrieve_unrelated(&facts, &memory), Unrelated::NoLeftOver);
        assert_eq!(
            retrieve_unrelated(&facts, &WorkMemory::default()),
            Unrelated::NoWork
        );
    }

    /// `:348-408` : ce qui reste à cuire, ce que les fours tiennent déjà, et le partage
    /// entre les fours disponibles.
    #[test]
    fn filling_a_furnace_counts_what_the_furnaces_already_hold() {
        let at = Pos::new(1, 64, 1);
        let mut facts = with_furnace(at);
        facts.furnaces[0].slots[FUEL_SLOT] = Some(Stack::new(NeedKey::plain("minecraft:coal"), 8));
        facts.primary_skill = 40;
        let memory = WorkMemory {
            furnace_pos: Some(at),
            ..WorkMemory::default()
        };
        let base = CraftFacts {
            has_recipe: true,
            request_count: 64,
            max_crafting_count: 64,
            inputs: vec![Input::item(1, 0, 100)],
            ..busy()
        };
        // Un seul four utilisable : tout ce qui reste part d'un coup (`:399-402`).
        assert_eq!(
            fill_up(&base, &facts, &memory),
            Fill::Fed { amount: 64 }
        );
        // Rien en main : la hutte est sollicitée (`:353-358`).
        let empty = CraftFacts {
            inputs: vec![Input::item(1, 0, 0)],
            ..base.clone()
        };
        assert_eq!(
            fill_up(&empty, &facts, &memory),
            Fill::Gather { need: 64 }
        );
        // Les fours tiennent déjà tout (`:344-351`).
        let covered = CraftFacts {
            inputs: vec![Input::item(1, 0, 100).extended(64)],
            ..base
        };
        assert_eq!(fill_up(&covered, &facts, &memory), Fill::NothingLeft);
    }

    /// `:429-503` : l'ordre de `executeCraftingAction`, une question par sortie.
    #[test]
    fn the_smelter_execution_follows_javas_order() {
        let facts = facts();
        // `:429-432` : `currentRecipeStorage == null` passe **avant** tout le reste,
        // donc une requête sans recette rend `START_WORKING`, pas `NotFurnace`.
        assert_eq!(execute(&CraftFacts::default(), &facts), Execute::NoRecipe);
        assert_eq!(execute(&busy(), &facts), Execute::NoRecipe);
        let working = CraftFacts {
            has_recipe: true,
            ..busy()
        };
        // `:434-437` : une recette qui n'est pas une cuisson part à la table.
        assert_eq!(execute(&working, &facts), Execute::NotFurnace);
        // `:439-452` : la hutte ne liste aucun four.
        let mut furnace_recipe = facts.clone();
        furnace_recipe.furnace_recipe = true;
        assert_eq!(execute(&working, &furnace_recipe), Execute::NoFurnace);
        // `:455-458` : un four déchargé arrête la cuisson.
        let mut with = with_furnace(Pos::new(1, 64, 1));
        with.furnace_recipe = true;
        with.furnaces_loaded = false;
        assert_eq!(execute(&working, &with), Execute::NotLoaded);
        with.furnaces_loaded = true;
        // `:462-470` : aucun carburant listé.
        let mut no_fuel = with.clone();
        no_fuel.allowed_fuels = 0;
        assert_eq!(execute(&working, &no_fuel), Execute::NoFuel);
        // `:472-476` : un résultat à sortir.
        let mut with_output = with.clone();
        let output = with_output.output.clone();
        with_output.furnaces[0].slots[RESULT_SLOT] = Some(Stack::new(output, 1));
        assert_eq!(
            execute(&working, &with_output),
            Execute::Retrieve {
                at: Pos::new(1, 64, 1)
            }
        );
        // `:499-502` : un four vide qui a du carburant.
        let mut fuelled = with.clone();
        fuelled.furnaces[0].slots[FUEL_SLOT] = Some(Stack::new(NeedKey::plain("minecraft:coal"), 8));
        assert_eq!(
            execute(&working, &fuelled),
            Execute::Fill {
                at: Pos::new(1, 64, 1)
            }
        );
        // `:503` : rien de tout cela.
        assert_eq!(execute(&working, &with), Execute::Restart);
    }

    /// `:493-497` : assez de fours qui cuisent déjà, la limite du niveau est atteinte.
    #[test]
    fn too_many_burning_furnaces_wait() {
        let mut facts = with_furnace(Pos::new(1, 64, 1));
        facts.furnace_recipe = true;
        facts.furnaces[0].lit = true;
        let input = facts.input.clone();
        facts.furnaces[0].slots[SMELTABLE_SLOT] = Some(Stack::new(input, 1));
        facts.furnaces[0].slots[FUEL_SLOT] = Some(Stack::new(NeedKey::plain("minecraft:coal"), 1));
        // `:429-432` d'abord : sans recette lue, la cuisson ne commence pas.
        assert_eq!(execute(&busy(), &facts), Execute::NoRecipe);
        let working = CraftFacts {
            has_recipe: true,
            ..busy()
        };
        assert_eq!(execute(&working, &facts), Execute::Burning);
    }

    /// `:120-142` : la durée d'accélération, et le fait qu'elle ne change jamais
    /// d'état.
    #[test]
    fn the_furnace_acceleration_is_two_ticks_per_ten_levels() {
        assert_eq!(acceleration_ticks(0), 0);
        assert_eq!(acceleration_ticks(9), 0);
        assert_eq!(acceleration_ticks(10), 2);
        assert_eq!(acceleration_ticks(35), 6);
        let mut machine = super::super::state::machine();
        register(&mut machine);
        let mut scene = Scene::new(
            CraftFacts::default(),
            TradeFacts::Baker(BakerFacts::default()),
        );
        assert!(machine.tick(&mut scene).is_some());
        assert_eq!(machine.state(), craft_state(CraftState::Idle));
    }
}
