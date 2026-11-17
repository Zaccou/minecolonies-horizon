//! Les buts de l'alchimiste : ses alambics, sa poudre de blaze, sa verrue du nether et
//! son gui.
//!
//! Référence Java : `EntityAIWorkAlchemist.java` — le constructeur qui ajoute les
//! cibles (`:101-112`), `decide` (`:307-349`), `getRecipe` (`:395-435`),
//! `getExtendedCount` (`:357-392`), `countOfBubblingBrewingStands` (`:442-466`),
//! `accelerateBrewingStand` (`:471-494`), `isFuelNeeded` (`:499-525`),
//! `checkBrewingStandFuel` (`:530-584`), `addFuelToBrewingStand` (`:591-640`),
//! `getMaxUsableBrewingStands` (`:642-646`),
//! `getPositionOfBrewingStandToRetrieveFrom` (`:654-685`), `checkForItems`
//! (`:688-725`), `retrieveBrewableFromBrewingStand` (`:733-794`), `retrieveUsedFuel`
//! (`:802-825`), `checkIfAbleToSmelt` (`:851-885`), `fillUpBrewingStand` (`:892-1056`)
//! et `craft` (`:1059-1114`), sur `AbstractEntityAICrafting.java:160-169` pour les
//! sept états communs.
//!
//! **Ce que ce fichier ne décide pas.** Poser une potion, extraire une fiole, frapper
//! une feuille, tirer `worker.getRandom()` : tout cela est un fait
//! (`facts::AlchemistFacts`) ou une écriture de mémoire (`state::WorkMemory`). Une
//! décision qui ne se rejoue pas ne se teste pas.

use crate::ai::{Action, Blocking, Machine, Pos, Transition};
use crate::ai_build::crafter::{self, CraftFacts, CraftState};

use super::facts::{
    AlchemistFacts, Stack, BREWING_FUEL_SLOT, BREWING_MIN_FUEL_COUNT, INGREDIENT_SLOT, POTION_SLOTS,
};
use super::state::{craft_state, Decision, Scene, WorkMemory, WorkRefusal, WorkState};
use super::{STANDARD_DELAY, STACKSIZE, TICKS_SECOND};

/// `EntityAIWorkAlchemist.DELAY_TO_HARVEST_NETHERWART` (`:71`) : le diviseur du tirage
/// qui décide d'aller récolter la verrue.
pub const DELAY_TO_HARVEST_NETHERWART: u32 = 30;
/// `EntityAIWorkAlchemist.DELAY_TO_HARVEST_MISTLETOE` (`:76`) : le même pour le gui.
pub const DELAY_TO_HARVEST_MISTLETOE: u32 = 30;
/// `worker.getRandom().nextInt(40) <= 0` (`:286`) : une chance sur quarante par coup.
pub const MISTLETOE_CHANCE: u32 = 40;
/// `checkIfRequestForItemExistOrCreateAsync(new ItemStack(NETHER_WART, 1), 16, 1)`
/// (`:145`, `:191`).
pub const NETHER_WART_REQUEST: u32 = 16;
/// Le minimum de la même requête (`:145`, `:191`).
pub const NETHER_WART_REQUEST_MIN: u32 = 1;

/// `accelerateBrewingStand`'s formula (`:473`) : `(second métier / 10) * 2` ticks
/// d'alambic offerts par seconde.
pub fn acceleration_ticks(secondary_skill: u8) -> u32 {
    (u32::from(secondary_skill) / 10) * 2
}

/// `countOfBubblingBrewingStands` (`:442-466`).
pub fn count_of_bubbling(facts: &AlchemistFacts) -> usize {
    facts
        .stands
        .iter()
        .filter(|stand| stand.loaded && stand.brew_time > 0)
        .count()
}

/// `getMaxUsableBrewingStands` (`:642-646`) : `min(niveau / 10 + 1, nombre
/// d'alambics)`.
pub fn max_usable_stands(facts: &AlchemistFacts) -> usize {
    let by_skill = usize::from(facts.primary_skill) / 10 + 1;
    by_skill.min(facts.stands.len())
}

/// `getPositionOfBrewingStandToRetrieveFrom` (`:654-685`) : le premier alambic qui ne
/// brasse plus, qui porte la sortie de la recette et dont le slot d'ingrédient est
/// vide.
pub fn stand_to_retrieve_from(facts: &AlchemistFacts) -> Option<Pos> {
    if !facts.brewing_recipe {
        return None; // `:656-659`
    }
    facts
        .stands
        .iter()
        .find(|stand| {
            stand.output_in_potion_slots(&facts.output) > 0
                && !stand.is_bubbling()
                && stand.is_empty(INGREDIENT_SLOT)
        })
        .map(|stand| stand.at) // `:678-681`
}

/// `isFuelNeeded` (`:499-525`) : la condition de l'événement de carburant.
///
/// Java rend `getState() != GATHERING_REQUIRED_MATERIALS` au **premier** alambic qui
/// a besoin de carburant, donc l'état courant fait partie de la réponse.
pub fn fuel_needed(facts: &AlchemistFacts, state: WorkState) -> bool {
    if !facts.brewing_recipe {
        return false; // `:501-504`
    }
    let wants = facts
        .stands
        .iter()
        .any(|stand| stand.loaded && !stand.is_bubbling() && stand.wants_fuel()); // `:517`
    wants && state != craft_state(CraftState::GatheringRequiredMaterials) // `:520`
}

/// Ce que `checkBrewingStandFuel` (`:530-584`) décide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les quatre sorties de `checkBrewingStandFuel`
pub enum Fuel {
    /// La recette ne brasse pas (`:532-535`).
    NotBrewing,
    /// La colonie a été priée de fournir de la poudre (`:539-545`).
    Asked {
        /// `BREWING_MIN_FUEL_COUNT * nombre d'alambics` (`:543`).
        amount: u32,
    },
    /// La hutte en a, le citoyen non (`:559-564`).
    Gather,
    /// Aller au fourneau et le tisonner (`:569-574`).
    Stoke {
        /// L'alambic visé (`fuelPos`, `:569`).
        at: Pos,
        /// `preFuelState` (`:570-573`), l'état auquel revenir.
        resume: WorkState,
    },
    /// Rien à faire, ou la poudre est en route (`:566`, `:583`).
    Wait(WorkRefusal),
}

/// `checkBrewingStandFuel` (`:530-584`).
pub fn check_fuel(facts: &AlchemistFacts, state: WorkState) -> Fuel {
    if !facts.brewing_recipe {
        return Fuel::NotBrewing; // `:532-535`
    }
    // `:539-545` : la demande part **avant** la boucle, que la poudre manque à la
    // hutte comme au citoyen, et seulement s'il n'y a pas déjà une requête ouverte.
    if !facts.blaze_powder_in_inventory && !facts.blaze_powder_in_hut && !facts.open_fuel_request
    {
        return Fuel::Asked {
            amount: BREWING_MIN_FUEL_COUNT.saturating_mul(facts.stands.len() as u32),
        };
    }
    let Some(stand) = facts
        .stands
        .iter()
        .find(|stand| stand.loaded && !stand.is_bubbling() && stand.wants_fuel()) // `:555`
    else {
        return Fuel::Wait(WorkRefusal::NothingToDo); // `:583`
    };
    if !facts.blaze_powder_in_inventory {
        // `:557-567`
        if facts.blaze_powder_in_hut {
            return Fuel::Gather;
        }
        return Fuel::Wait(WorkRefusal::FuelOnItsWay);
    }
    Fuel::Stoke {
        at: stand.at,      // `:569`
        resume: state,     // `:570-573`, `preFuelState = getState()` quand il est nul
    }
}

/// Ce que `addFuelToBrewingStand` (`:591-640`) décide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les cinq sorties de `addFuelToBrewingStand`
pub enum Stoke {
    /// La hutte a de la poudre, le citoyen non (`:595-599`) — `STACKSIZE` demandés.
    Gather,
    /// Plus de poudre nulle part (`:601-603`).
    Restart,
    /// `fuelPos` est nul ou le citoyen n'y est pas encore (`:606-609`).
    Walking,
    /// La poudre est allée dans le slot 4 et l'IA reprend ce qu'elle faisait
    /// (`:621-631`).
    Stoked {
        /// `preFuelState`, ou `START_WORKING` quand il est resté nul (`:636-639`).
        resume: WorkState,
    },
    /// L'alambic n'est plus un alambic, ou ne veut plus de carburant (`:611-619`,
    /// `:636-639`).
    Confused,
}

/// `addFuelToBrewingStand` (`:591-640`).
pub fn add_fuel(facts: &AlchemistFacts, memory: &super::state::WorkMemory) -> Stoke {
    if !facts.blaze_powder_in_inventory {
        if facts.blaze_powder_in_hut {
            return Stoke::Gather; // `:595-599`
        }
        return Stoke::Restart; // `:601-603`
    }
    let Some(at) = memory.fuel_pos else {
        return Stoke::Walking; // `:606`
    };
    if !facts.at_work_pos {
        return Stoke::Walking; // `:606` (`walkToWorkPos`)
    }
    let Some(stand) = facts.stand_at(at) else {
        return Stoke::Confused; // `:611-614`
    };
    if !stand.loaded || !stand.wants_fuel() {
        return Stoke::Confused; // `:611` (`isBlockLoaded`), `:618-619`
    }
    let resume = memory
        .pre_fuel_state
        .filter(|state| *state != WorkState::AddFuelToBrewingStand)
        .unwrap_or(craft_state(CraftState::StartWorking)); // `:625-630`, sinon `:636-639`
    Stoke::Stoked { resume }
}

/// Ce que `fillUpBrewingStand` (`:892-1056`) décide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les onze sorties de `fillUpBrewingStand`
pub enum Fill {
    /// La hutte ne liste aucun alambic (`:894-902`).
    NoStand,
    /// `walkTo` n'est plus un alambic (`:904-909`).
    StandGone,
    /// Rien à verser (`:931-934`, `:998-1001`).
    Full,
    /// La hutte peut fournir les potions (`:976-980`).
    GatherPotions {
        /// `targetCount`, ce qu'il faut aller chercher (`:978`).
        need: u32,
    },
    /// La hutte peut fournir l'ingrédient (`:1034-1038`).
    GatherIngredient {
        /// `targetCount` (`:1036`).
        need: u32,
    },
    /// Une potion est allée dans un slot 0..3 (`:947-973`).
    Poured {
        /// Combien de slots vides ont reçu une potion.
        slots: usize,
    },
    /// L'ingrédient est allé dans le slot 3 (`:1013-1031`).
    Ingredient,
    /// Pas encore à la position de travail (`:961-964`, `:1020-1023`).
    Walking,
    /// Les compteurs de l'IA et l'alambic ne s'accordent plus (`:981-988`,
    /// `:1039-1046`).
    OutOfSync,
    /// La recette n'a pas l'entrée que Java lit (`:924`, `:992`) : Java lèverait
    /// `IndexOutOfBoundsException`, le port nomme le refus.
    MissingInput,
    /// La recette n'a pas d'alambic pour intermédiaire, ou plus rien à faire
    /// (`:1049-1055`).
    Done,
}

/// `fillUpBrewingStand` (`:892-1056`).
///
/// Java calcule `targetCount` en `int` et le compare à `<= 0` : il peut donc être
/// négatif. Le port calcule en `i64` pour ne pas déborder d'un `u32` et **nomme**
/// `Full` le cas `<= 0` (`:931`, `:998`).
pub fn fill_up(base: &CraftFacts, facts: &AlchemistFacts, memory: &super::state::WorkMemory) -> Fill {
    if facts.stands.is_empty() {
        return Fill::NoStand; // `:894-902`
    }
    let Some(at) = memory.walk_to else {
        return Fill::StandGone; // `:904`
    };
    let Some(stand) = facts.stand_at(at) else {
        return Fill::StandGone; // `:904`
    };
    if !stand.loaded {
        return Fill::StandGone; // `:904` (`world.getBlockState(walkTo)`)
    }
    let burning = count_of_bubbling(facts); // `:911`
    if !base.has_recipe {
        return Fill::Done; // `:913` exige `currentRecipeStorage != null`
    }
    let max = max_usable_stands(facts); // `:916`
    let result_in_stand = stand.count_of(&facts.output); // `:917`
    let result_in_inv = base.carried_output; // `:918-919`
    let target_of = |extra: i64| {
        i64::from(base.request_count) * i64::from(base.output_count) - extra
            - i64::from(result_in_stand)
            - i64::from(result_in_inv)
    };
    if stand.has_empty_potion_slot() {
        // `:921-922`, branche de la potion.
        let Some(potion) = base.inputs.get(1) else {
            return Fill::MissingInput; // `:924` lèverait `IndexOutOfBoundsException`
        };
        let in_stand = stand.count_of(&facts.potion); // `:928`
        let target = target_of(i64::from(in_stand)); // `:929-930`
        if target <= 0 {
            return Fill::Full; // `:931-934`
        }
        if potion.in_inventory > 0 {
            // `:943-975`
            if !(stand.has_fuel_and_no_brewable() || stand.has_neither_fuel_nor_brewable()) {
                return Fill::Done; // `:945` : ce fourneau ne veut pas de potion
            }
            let slots = if burning < max {
                (0..POTION_SLOTS).filter(|slot| stand.is_empty(*slot)).count() // `:947-958`
            } else {
                0
            };
            if slots == 0 {
                return Fill::Done; // `:1053-1055`
            }
            if !facts.at_work_pos {
                return Fill::Walking; // `:961-964`
            }
            return Fill::Poured { slots };
        }
        if i64::from(potion.in_building) >= target - i64::from(potion.in_inventory)
            && facts.brewing_recipe
        {
            return Fill::GatherPotions {
                need: target as u32, // `:976-980`
            };
        }
        return Fill::OutOfSync; // `:981-988`
    }
    if stand.is_empty(INGREDIENT_SLOT) {
        // `:990`, branche de l'ingrédient.
        let Some(ingredient) = base.inputs.first() else {
            return Fill::MissingInput; // `:992` lèverait `IndexOutOfBoundsException`
        };
        let in_stand = stand.count_of(&facts.ingredient); // `:994`
        let target = target_of(i64::from(in_stand) * 3); // `:995-997`
        if target <= 0 {
            return Fill::Full; // `:998-1001`
        }
        if ingredient.in_inventory > 0 {
            // `:1009-1033`
            if !(stand.has_fuel_and_no_brewable() || stand.has_neither_fuel_nor_brewable()) {
                return Fill::Done; // `:1011`
            }
            if burning >= max {
                return Fill::Done; // `:1013-1017`, `toTransfer` reste à zéro
            }
            if !facts.at_work_pos {
                return Fill::Walking; // `:1020-1023`
            }
            return Fill::Ingredient;
        }
        if i64::from(ingredient.in_building) >= target - i64::from(ingredient.in_inventory)
            && facts.brewing_recipe
        {
            return Fill::GatherIngredient {
                need: target as u32, // `:1034-1038`
            };
        }
        return Fill::OutOfSync; // `:1039-1046`
    }
    Fill::Done // `:1049-1055`
}

/// Ce que `retrieveBrewableFromBrewingStand` (`:733-794`) décide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les six sorties de `retrieveBrewableFromBrewingStand`
pub enum Retrieve {
    /// Plus d'`walkTo` ou plus de requête (`:735-738`).
    NoWork,
    /// L'alambic visé n'existe plus (`:740-745`).
    StandGone,
    /// Pas encore arrivé (`:747-750`).
    Walking,
    /// Rien n'est sorti (`:768`, donc `:792-793`).
    Nothing,
    /// La sortie est sortie et comptée (`:774-781`).
    Delivered {
        /// `resultCount / primaryOutput.getCount()` (`:774`).
        steps: u32,
    },
    /// Le compte demandé est atteint (`:782-789`) ; `:776-777` a déjà compté les pas
    /// avant de le constater.
    Finalized {
        /// Les pas servis par cette sortie (`:774`).
        steps: u32,
    },
    /// `getPrimaryOutput().getCount()` vaut zéro : `:774` diviserait par zéro.
    DegenerateOutput,
}

/// `retrieveBrewableFromBrewingStand` (`:733-794`).
pub fn retrieve_brewed(
    base: &CraftFacts,
    facts: &AlchemistFacts,
    memory: &super::state::WorkMemory,
) -> Retrieve {
    if !base.has_request {
        return Retrieve::NoWork; // `:735-738`
    }
    let Some(at) = memory.walk_to else {
        return Retrieve::NoWork; // `:735-738`
    };
    let Some(stand) = facts.stand_at(at) else {
        return Retrieve::StandGone; // `:740-745`
    };
    if !facts.at_work_pos {
        return Retrieve::Walking; // `:747-750`
    }
    let extracted: u32 = (0..POTION_SLOTS)
        .filter(|slot| {
            stand
                .slots
                .get(*slot)
                .and_then(|stack| stack.as_ref())
                .is_some_and(|stack: &Stack| stack.key == facts.output)
        })
        .map(|slot| stand.count_at(slot))
        .sum(); // `:756-762`
    if extracted == 0 {
        return Retrieve::Nothing; // `:768`
    }
    if base.output_count == 0 {
        return Retrieve::DegenerateOutput; // `:774`
    }
    let steps = extracted / base.output_count; // `:774`
    let counter = base.craft_counter.saturating_add(steps); // `:776`
    let progress = base.progress.saturating_sub(steps); // `:777`
    if counter >= base.max_crafting_count && progress == 0 {
        return Retrieve::Finalized { steps }; // `:782-789`
    }
    Retrieve::Delivered { steps }
}

/// Ce que `retrieveUsedFuel` (`:802-825`) décide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les cinq sorties de `retrieveUsedFuel`
pub enum Retrieval {
    /// Plus d'`walkTo` (`:804-807`).
    NoWork,
    /// Pas encore arrivé (`:809-812`).
    Walking,
    /// L'alambic ou sa foudre de blaze ont disparu (`:814-819`).
    StandGone,
    /// Le carburant usé est sorti (`:823-824`).
    Extracted,
}

/// `retrieveUsedFuel` (`:802-825`).
pub fn retrieve_used_fuel(
    facts: &AlchemistFacts,
    memory: &super::state::WorkMemory,
) -> Retrieval {
    let Some(at) = memory.walk_to else {
        return Retrieval::NoWork; // `:804-807`
    };
    if !facts.at_work_pos {
        return Retrieval::Walking; // `:809-812`
    }
    let Some(stand) = facts.stand_at(at) else {
        return Retrieval::StandGone; // `:814-819`
    };
    if !stand.loaded || stand.is_empty(BREWING_FUEL_SLOT) {
        return Retrieval::StandGone; // `:815`
    }
    Retrieval::Extracted // `:823`
}

/// Ce que `checkIfAbleToSmelt` (`:851-885`) décide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les quatre sorties de `checkIfAbleToSmelt`
pub enum Able {
    /// Tous les alambics brassent, ou la demande est déjà couverte (`:855-859`).
    Committed,
    /// Cet alambic a un slot d'ingrédient vide (`:861-871`).
    Fill {
        /// L'alambic à emplir.
        at: Pos,
    },
    /// Rien à emplir mais quelque chose brasse (`:879-882`).
    Brewing,
    /// Rien du tout (`:884`).
    Idle,
}

/// `checkIfAbleToSmelt` (`:851-885`).
pub fn able_to_brew(base: &CraftFacts, facts: &AlchemistFacts) -> Able {
    let burning = count_of_bubbling(facts); // `:854`
    let max = max_usable_stands(facts); // `:855`
    if burning > 0
        && (burning >= max
            || base.craft_counter.saturating_add(base.progress) >= base.max_crafting_count)
    {
        return Able::Committed; // `:855-859`
    }
    if let Some(stand) = facts
        .stands
        .iter()
        .find(|stand| stand.loaded && stand.is_empty(INGREDIENT_SLOT)) // `:861-871`
    {
        return Able::Fill { at: stand.at };
    }
    if burning > 0 {
        return Able::Brewing; // `:879-882`
    }
    Able::Idle // `:884`
}

/// Applique une réponse d'[`able_to_brew`] aux faits et à la mémoire.
pub fn apply_able(scene: &mut Scene, able: Able) -> Decision {
    match able {
        Able::Committed => {
            scene.memory.delay = TICKS_SECOND; // `:857`
            Decision::refuse(WorkRefusal::NothingToDo)
        }
        Able::Fill { at } => {
            scene.memory.walk_to = Some(at); // `:869`
            Decision::go(WorkState::FillUpBrewingStand) // `:870`
        }
        Able::Brewing => {
            scene.memory.delay = TICKS_SECOND; // `:881`
            Decision::refuse(WorkRefusal::NothingToDo)
        }
        Able::Idle => Decision::refuse(WorkRefusal::NothingToDo), // `:884`
    }
}

/// Ce que `getRecipe` de l'alchimiste (`:395-435`) demande.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les quatre sorties de l'override
pub enum BrewRecipe {
    /// Aucune tâche (`:399-403`).
    NoTask,
    /// Un alambic a de la sortie à prendre (`:407-413`).
    Retrieve {
        /// L'alambic (`:411`).
        at: Pos,
        /// `job.setMaxCraftingCount(currentTask.getRequest().getCount())` (`:405`).
        max: u32,
    },
    /// Un alambic brasse ou tient un ingrédient (`:415-426`).
    Brew,
    /// La classe de base (`:434`).
    Base,
}

/// `getRecipe` de l'alchimiste (`:395-435`).
pub fn get_recipe(base: &CraftFacts, facts: &AlchemistFacts) -> BrewRecipe {
    if !base.current_task {
        return BrewRecipe::NoTask; // `:399-403`
    }
    if let Some(at) = stand_to_retrieve_from(facts) {
        return BrewRecipe::Retrieve {
            at,
            max: base.request_count, // `:405`
        };
    }
    if facts.brewing_recipe
        && facts
            .stands
            .iter()
            .any(|stand| stand.loaded && (stand.is_bubbling() || !stand.is_empty(INGREDIENT_SLOT)))
    {
        return BrewRecipe::Brew; // `:415-426`
    }
    BrewRecipe::Base // `:434`
}

/// Ce que `checkForItems` de l'alchimiste (`:688-725`) répond.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les trois sorties de l'override
pub enum BrewItems {
    /// Pas une recette d'alambic : la classe de base (`:690-693`).
    NoRecipe,
    /// La hutte a l'entrée (`:708-712`).
    Gather {
        /// L'entrée en défaut, index dans `CraftFacts::inputs`.
        input: usize,
        /// `amount * (maxCraftingCount - countInStand - inputInStand)` (`:710`).
        remaining: u32,
    },
    /// Assez de tout, on brasse (`:724`) — `cancelled` quand `:719-720` a annulé la
    /// requête au passage.
    Craft {
        /// `job.finishRequest(false)` a été appelé (`:719`).
        cancelled: bool,
    },
}

/// L'identité de l'entrée `index` d'une recette d'alambic.
///
/// Java lit `inputStorage.getItemStack()` et le compare aux quatre slots de l'alambic
/// (`:703`). Une recette de brassage a exactement deux entrées : l'ingrédient
/// (`getCleanedInput().get(0)`, `:992`) et la potion (`.get(1)`, `:924`). Une troisième
/// entrée n'existe pas dans cette recette ; elle compte zéro plutôt que d'inventer une
/// clé.
fn input_key(facts: &AlchemistFacts, index: usize) -> Option<&crate::ai_build::materials::NeedKey> {
    match index {
        0 => Some(&facts.ingredient),
        1 => Some(&facts.potion),
        _ => None,
    }
}

/// `checkForItems` de l'alchimiste (`:688-725`).
pub fn check_for_items(base: &CraftFacts, facts: &AlchemistFacts) -> BrewItems {
    if !facts.brewing_recipe {
        return BrewItems::NoRecipe; // `:690-693`
    }
    let in_stand = facts
        .stands
        .iter()
        .map(|stand| stand.count_of(&facts.output))
        .sum::<u32>(); // `:696`
    let output_in_inv = base.carried_output; // `:697-698`
    let mut cancelled = false;
    for (index, input) in base.inputs.iter().enumerate() {
        let input_in_stand: u32 = match input_key(facts, index) {
            Some(key) => facts.stands.iter().map(|stand| stand.count_of(key)).sum(), // `:703`
            None => 0,
        };
        let input_in_inv = input.in_inventory; // `:704`
        let required = input.amount.saturating_mul(base.max_crafting_count); // `:706`, `:717`
        let present = in_stand
            .saturating_add(input_in_stand)
            .saturating_add(input_in_inv)
            .saturating_add(output_in_inv);
        if present < required && input.in_building > 0 {
            return BrewItems::Gather {
                input: index, // `:710-711`
                remaining: input
                    .amount
                    .saturating_mul(base.max_crafting_count.saturating_sub(in_stand + input_in_stand)),
            };
        }
        let count_of_input = input_in_inv
            .saturating_add(input.in_building)
            .saturating_add(in_stand)
            .saturating_add(input_in_stand)
            .saturating_add(output_in_inv);
        if count_of_input < required {
            cancelled = true; // `:719-720`
        }
    }
    BrewItems::Craft { cancelled } // `:724`
}

/// Ce que `craft` de l'alchimiste (`:1059-1114`) demande.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les sept sorties de l'override
pub enum BrewCraft {
    /// Pas encore arrivé à la hutte (`:1061-1065`).
    Walking {
        /// `setDelay(STANDARD_DELAY)` (`:1063`).
        delay: u32,
    },
    /// La recette n'est pas un brassage : la classe de base (`:1072-1075`).
    Delegated,
    /// Aucun alambic (`:1077-1085`).
    NoStand,
    /// Un alambic a de la sortie (`:1087-1092`).
    Retrieve {
        /// L'alambic (`:1090`).
        at: Pos,
    },
    /// Les compteurs disent que la demande est finie (`:1094-1102`).
    Finished,
    /// La requête a été annulée ou a échoué (`:1104-1111`).
    Cancelled,
    /// `checkIfAbleToSmelt` (`:1113`).
    Able(Able),
}

/// `craft` de l'alchimiste (`:1059-1114`).
pub fn craft(base: &CraftFacts, facts: &AlchemistFacts) -> BrewCraft {
    if !base.at_building {
        return BrewCraft::Walking {
            delay: STANDARD_DELAY, // `:1063`
        };
    }
    if !facts.brewing_recipe {
        return BrewCraft::Delegated; // `:1072-1075`
    }
    if facts.stands.is_empty() {
        return BrewCraft::NoStand; // `:1077-1085`
    }
    if let Some(at) = stand_to_retrieve_from(facts) {
        return BrewCraft::Retrieve { at }; // `:1087-1092`
    }
    if base.has_request && base.max_crafting_count > 0 && base.craft_counter >= base.max_crafting_count
    {
        return BrewCraft::Finished; // `:1094-1102`
    }
    if base.has_request && base.request_cancelled {
        return BrewCraft::Cancelled; // `:1104-1111`
    }
    BrewCraft::Able(able_to_brew(base, facts)) // `:1113`
}

/// Ce que `decide` de l'alchimiste (`:307-349`) demande.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les huit sorties de `decide`
pub enum Decide {
    /// Aucune tâche, rien à récolter, une navigation en cours (`:334`).
    Idle,
    /// Le tirage de `:314` dit d'aller à la verrue.
    HarvestNetherWart,
    /// Celui de `:319` dit d'aller au gui.
    HarvestMistletoe,
    /// Dans la hutte : `setDelay(TICKS_20 * 20)` et une marche au hasard (`:324-328`).
    Wander {
        /// Le délai posé (`:326`).
        delay: u32,
    },
    /// Dehors : `walkToBuilding()` (`:329-332`).
    WalkIn,
    /// Une tâche existe et le citoyen marche vers sa hutte (`:337-340`).
    Walking,
    /// `job.getActionsDone() >= 1` (`:342-346`).
    WaitingForDump,
    /// `getNextCraftingState()` (`:348`).
    Next(CraftState),
}

/// Le tirage `nextInt(range) <= 1` (`:314`, `:319`), ramené dans les bornes pour
/// qu'un fait hors bornes ne panique pas.
fn draw(value: u32, range: u32) -> u32 {
    if range == 0 {
        0
    } else {
        value % range
    }
}

/// `decide` de l'alchimiste (`:307-349`).
pub fn decide(base: &CraftFacts, facts: &AlchemistFacts) -> Decide {
    if base.task_queue == 0 || !base.current_task {
        // `:310`
        if !base.navigation_done {
            return Decide::Idle; // `:312`, `:334`
        }
        if draw(facts.nether_wart_roll, DELAY_TO_HARVEST_NETHERWART) <= 1 {
            return Decide::HarvestNetherWart; // `:314-317`
        }
        if draw(facts.mistletoe_roll, DELAY_TO_HARVEST_MISTLETOE) <= 1 {
            return Decide::HarvestMistletoe; // `:319-322`
        }
        if base.in_building {
            return Decide::Wander {
                delay: TICKS_20_TIMES_20,
            }; // `:324-328`
        }
        return Decide::WalkIn; // `:329-332`
    }
    if !base.at_building {
        return Decide::Walking; // `:337-340`
    }
    if base.actions_done >= 1 {
        return Decide::WaitingForDump; // `:342-346`, `getState()`
    }
    Decide::Next(crafter::next_crafting_state(base)) // `:348`
}

/// `TICKS_20 * 20` (`:326`) — `CitizenConstants.java:104` fois vingt, 400 ticks.
pub const TICKS_20_TIMES_20: u32 = 20 * 20;

/// Applique une réponse de [`decide`] : c'est ici que les deux branches de `:324-332`
/// deviennent la mémoire (`delay`, déplacement) au lieu d'un appel à
/// `EntityNavigationUtils`.
pub fn apply_decide(scene: &mut Scene, decide: Decide) -> Decision {
    match decide {
        Decide::Idle => Decision::refuse(WorkRefusal::NothingToDo), // `:334`
        Decide::HarvestNetherWart => Decision::go(WorkState::HarvestNetherWart), // `:316`
        Decide::HarvestMistletoe => Decision::go(WorkState::HarvestMistletoe), // `:321`
        Decide::Wander { delay } => {
            scene.memory.delay = delay; // `:326`
            Decision::go(craft_state(CraftState::Idle)) // `:334`
        }
        Decide::WalkIn => Decision::go(craft_state(CraftState::Idle)), // `:331-334`
        Decide::Walking => Decision::go(craft_state(CraftState::StartWorking)), // `:339`
        // `:345` rend `getState()` : l'état ne bouge pas, le tick s'arrête.
        Decide::WaitingForDump => Decision::refuse(WorkRefusal::NothingToDo),
        Decide::Next(next) => Decision::go(craft_state(next)), // `:348`
    }
}

/// Les cibles de l'alchimiste : les sept communes, les six siennes, et les deux
/// événements de fond (`EntityAIWorkAlchemist.java:101-112`).
pub fn register(machine: &mut Machine<WorkState, Scene>) {
    super::state::register_shared(machine);
    // `:107` — `START_USING_BREWINGSTAND`, au rythme d'une seconde.
    machine.add(Transition::target(
        WorkState::FillUpBrewingStand,
        "fillUpBrewingStand",
        "EntityAIWorkAlchemist.java:107",
        TICKS_SECOND,
        Action::Run(fill_action),
    ));
    // `:108` — sortir les potions.
    machine.add(Transition::target(
        WorkState::RetrieveBrewed,
        "retrieveBrewableFromBrewingStand",
        "EntityAIWorkAlchemist.java:108",
        TICKS_SECOND,
        Action::Run(retrieve_action),
    ));
    // `:109` — sortir la poudre usée.
    machine.add(Transition::target(
        WorkState::RetrieveUsedFuel,
        "retrieveUsedFuel",
        "EntityAIWorkAlchemist.java:109",
        TICKS_SECOND,
        Action::Run(fuel_retrieve_action),
    ));
    // `:110` — remettre de la poudre.
    machine.add(Transition::target(
        WorkState::AddFuelToBrewingStand,
        "addFuelToBrewingStand",
        "EntityAIWorkAlchemist.java:110",
        TICKS_SECOND,
        Action::Run(stoke_action),
    ));
    // `:111` — frapper les feuilles.
    machine.add(Transition::target(
        WorkState::HarvestMistletoe,
        "harvestMistletoe",
        "EntityAIWorkAlchemist.java:111",
        TICKS_SECOND,
        Action::Run(super::harvest::mistletoe_action),
    ));
    // `:112` — la verrue du nether.
    machine.add(Transition::target(
        WorkState::HarvestNetherWart,
        "harvestNetherWart",
        "EntityAIWorkAlchemist.java:112",
        TICKS_SECOND,
        Action::Run(super::harvest::nether_wart_action),
    ));
    // `:105` — l'événement de carburant, toutes les dix secondes, quand il en faut.
    machine.add(Transition::event_when(
        Blocking::Event,
        "isFuelNeeded",
        "EntityAIWorkAlchemist.java:105",
        TICKS_SECOND * 10,
        |scene: &Scene, state: WorkState| match scene.alchemist() {
            Some(facts) => fuel_needed(facts, state),
            None => false,
        },
        Action::Run(fuel_event_action),
    ));
    // `:106` — l'accélération, toutes les secondes, et elle ne change jamais d'état
    // (Java rend `getState()`).
    machine.add(Transition::event(
        Blocking::Event,
        "accelerateBrewingStand",
        "EntityAIWorkAlchemist.java:106",
        TICKS_SECOND,
        Action::Run(accelerate_action),
    ));
}

/// `:107` — `fillUpBrewingStand`, et les écritures de Java au passage.
fn fill_action(scene: &mut Scene, state: WorkState) -> Option<WorkState> {
    let answer = scene.alchemist().map(|facts| fill_up(&scene.base, facts, &scene.memory));
    let Some(answer) = answer else {
        scene.refuse(WorkRefusal::NoFactsForTrade);
        return Some(state);
    };
    match answer {
        Fill::NoStand => {
            scene.memory.delay = STANDARD_DELAY; // `:900`
            scene.refuse(WorkRefusal::NoBrewingStand);
            Some(craft_state(CraftState::StartWorking)) // `:901`
        }
        Fill::StandGone => {
            scene.memory.walk_to = None; // `:906`
            scene.memory.delay = STANDARD_DELAY; // `:907`
            scene.refuse(WorkRefusal::StandGone);
            Some(craft_state(CraftState::StartWorking)) // `:908`
        }
        Fill::Walking => {
            scene.refuse(WorkRefusal::NotAtWorkPos);
            Some(state) // `:963`, `:1022`
        }
        Fill::GatherPotions { need } | Fill::GatherIngredient { need } => {
            scene.memory.need = Some(need); // `needsCurrently`, `AbstractEntityAIBasic.java:156`
            Some(craft_state(CraftState::GatheringRequiredMaterials)) // `:979`, `:1037`
        }
        Fill::Poured { .. } | Fill::Ingredient => {
            scene.memory.walk_to = None; // `:1053`
            scene.memory.delay = STANDARD_DELAY; // `:1054`
            Some(craft_state(CraftState::StartWorking)) // `:1055`
        }
        Fill::Full | Fill::Done => {
            scene.memory.walk_to = None; // `:1053`
            scene.memory.delay = STANDARD_DELAY; // `:1054`
            Some(craft_state(CraftState::StartWorking)) // `:933`, `:1055`
        }
        Fill::MissingInput => {
            scene.memory.walk_to = None; // `:986`
            scene.refuse(WorkRefusal::MissingRecipeInput);
            Some(craft_state(CraftState::Idle))
        }
        Fill::OutOfSync => {
            scene.memory.walk_to = None; // `:986`
            scene.refuse(WorkRefusal::BrewingOutOfSync);
            Some(craft_state(CraftState::Idle)) // `:987`
        }
    }
}

/// `:108` — `retrieveBrewableFromBrewingStand`.
fn retrieve_action(scene: &mut Scene, state: WorkState) -> Option<WorkState> {
    let answer = scene
        .alchemist()
        .map(|facts| retrieve_brewed(&scene.base, facts, &scene.memory));
    let Some(answer) = answer else {
        scene.refuse(WorkRefusal::NoFactsForTrade);
        return Some(state);
    };
    match answer {
        Retrieve::NoWork | Retrieve::StandGone => {
            scene.memory.walk_to = None; // `:743`, `:751`
            Some(craft_state(CraftState::StartWorking)) // `:737`, `:744`
        }
        Retrieve::Walking => {
            scene.refuse(WorkRefusal::NotAtWorkPos);
            Some(state) // `:749`
        }
        Retrieve::Nothing => {
            scene.memory.delay = STANDARD_DELAY; // `:792`
            scene.refuse(WorkRefusal::NothingToDo);
            Some(craft_state(CraftState::StartWorking)) // `:793`
        }
        Retrieve::DegenerateOutput => {
            scene.memory.delay = STANDARD_DELAY; // `:792`
            scene.refuse(WorkRefusal::DegenerateOutput); // `:774`
            Some(craft_state(CraftState::StartWorking)) // `:793`
        }
        Retrieve::Delivered { steps } => {
            scene.memory.walk_to = None; // `:751`
            scene.base.craft_counter = scene.base.craft_counter.saturating_add(steps); // `:776`
            scene.base.progress = scene.base.progress.saturating_sub(steps); // `:777`
            scene.memory.delay = STANDARD_DELAY; // `:792`
            Some(craft_state(CraftState::StartWorking)) // `:793`
        }
        Retrieve::Finalized { steps } => {
            scene.memory.walk_to = None; // `:785`
            scene.base.craft_counter = scene.base.craft_counter.saturating_add(steps); // `:776`
            scene.base.progress = scene.base.progress.saturating_sub(steps); // `:777`
            scene.base.has_recipe = false; // `:786`
            Some(craft_state(CraftState::InventoryFull)) // `:788`
        }
    }
}

/// `:109` — `retrieveUsedFuel`.
fn fuel_retrieve_action(scene: &mut Scene, state: WorkState) -> Option<WorkState> {
    let answer = scene
        .alchemist()
        .map(|facts| retrieve_used_fuel(facts, &scene.memory));
    let Some(answer) = answer else {
        scene.refuse(WorkRefusal::NoFactsForTrade);
        return Some(state);
    };
    match answer {
        Retrieval::Walking => {
            scene.refuse(WorkRefusal::NotAtWorkPos);
            Some(state) // `:811`
        }
        Retrieval::NoWork | Retrieval::StandGone | Retrieval::Extracted => {
            scene.memory.walk_to = None; // `:817`, `:821`
            Some(craft_state(CraftState::StartWorking)) // `:806`, `:818`, `:824`
        }
    }
}

/// `:110` — `addFuelToBrewingStand`.
fn stoke_action(scene: &mut Scene, state: WorkState) -> Option<WorkState> {
    let answer = scene.alchemist().map(|facts| add_fuel(facts, &scene.memory));
    let Some(answer) = answer else {
        scene.refuse(WorkRefusal::NoFactsForTrade);
        return Some(state);
    };
    match answer {
        Stoke::Gather => {
            scene.memory.need = Some(STACKSIZE); // `needsCurrently = (…, STACKSIZE)` (`:597`)
            Some(craft_state(CraftState::GatheringRequiredMaterials)) // `:598`
        }
        Stoke::Walking => {
            scene.refuse(WorkRefusal::NotAtWorkPos);
            Some(state) // `:608`
        }
        Stoke::Stoked { resume } => {
            scene.memory.fuel_pos = None; // `:629`
            scene.memory.pre_fuel_state = None; // `:628`
            Some(resume) // `:630`
        }
        Stoke::Restart | Stoke::Confused => {
            scene.memory.fuel_pos = None; // `:602`, `:638`
            scene.memory.pre_fuel_state = None; // `:601`, `:637`
            scene.refuse(WorkRefusal::NoBlazePowder);
            Some(craft_state(CraftState::StartWorking)) // `:603`, `:639`
        }
    }
}

/// `:105` — `checkBrewingStandFuel`, qui ne change d'état que pour aller chercher la
/// poudre ou tisonner.
fn fuel_event_action(scene: &mut Scene, state: WorkState) -> Option<WorkState> {
    let answer = scene.alchemist().map(|facts| check_fuel(facts, state));
    let Some(answer) = answer else {
        scene.refuse(WorkRefusal::NoFactsForTrade);
        return Some(state);
    };
    match answer {
        Fuel::NotBrewing => {
            scene.refuse(WorkRefusal::NotBrewingRecipe); // `:532-535`
            Some(state)
        }
        Fuel::Wait(_) => Some(state), // `:566`, `:583`
        Fuel::Asked { .. } => Some(state),                // `:543-544`
        Fuel::Gather => {
            scene.memory.need = Some(BREWING_MIN_FUEL_COUNT); // `:561`
            scene.memory.walk_to = None; // `:562`
            Some(craft_state(CraftState::GatheringRequiredMaterials)) // `:563`
        }
        Fuel::Stoke { at, resume } => {
            scene.memory.fuel_pos = Some(at); // `:569`
            if scene.memory.pre_fuel_state.is_none() {
                scene.memory.pre_fuel_state = Some(resume); // `:570-573`
            }
            Some(WorkState::AddFuelToBrewingStand) // `:574`
        }
    }
}

/// `:106` — `accelerateBrewingStand` : un effet, jamais une transition.
fn accelerate_action(_scene: &mut Scene, state: WorkState) -> Option<WorkState> {
    Some(state) // `:493` (`return false`)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_build::crafter::Input;
    use crate::ai_build::materials::NeedKey;
    use crate::ai_work_01::facts::{Above, Soil, Stand, TradeFacts};
    use crate::ai_work_01::harvest::{self, Mistletoe, Wart};

    fn brewing() -> AlchemistFacts {
        AlchemistFacts {
            brewing_recipe: true,
            ..AlchemistFacts::default()
        }
    }

    fn busy() -> CraftFacts {
        CraftFacts {
            task_queue: 1,
            current_task: true,
            ..CraftFacts::default()
        }
    }

    fn walk_to(at: Pos) -> WorkMemory {
        WorkMemory {
            walk_to: Some(at),
            ..WorkMemory::default()
        }
    }

    /// `:310`, `:324-332` : sans tâche, l'alchimiste passe-temps puis reste `IDLE` —
    /// dedans un `setDelay`, dehors une marche vers la hutte.
    #[test]
    fn without_a_task_the_alchemist_passes_the_time_then_idles() {
        let quiet = AlchemistFacts {
            nether_wart_roll: 5,
            mistletoe_roll: 5,
            ..AlchemistFacts::default()
        };
        assert_eq!(decide(&CraftFacts::default(), &quiet), Decide::Wander {
            delay: TICKS_20_TIMES_20
        });
        let mut scene = Scene::new(CraftFacts::default(), TradeFacts::Alchemist(quiet.clone()));
        let decision = apply_decide(&mut scene, Decide::Wander { delay: 400 });
        assert_eq!(decision.next, Some(craft_state(CraftState::Idle)));
        assert_eq!(scene.memory.delay, 400);
        let outside = CraftFacts {
            in_building: false,
            ..CraftFacts::default()
        };
        assert_eq!(decide(&outside, &quiet), Decide::WalkIn);
        // Une navigation en cours court-circuite les deux tirages (`:312`).
        let walking = CraftFacts {
            navigation_done: false,
            ..CraftFacts::default()
        };
        assert_eq!(decide(&walking, &quiet), Decide::Idle);
    }

    /// `:314-322` : les deux tirages dans l'ordre de Java, `<= 1` sur trente.
    #[test]
    fn the_two_harvest_draws_are_tested_in_javas_order() {
        let wart = AlchemistFacts {
            nether_wart_roll: 1,
            mistletoe_roll: 1,
            ..AlchemistFacts::default()
        };
        assert_eq!(
            decide(&CraftFacts::default(), &wart),
            Decide::HarvestNetherWart
        );
        let mistletoe = AlchemistFacts {
            nether_wart_roll: 2,
            mistletoe_roll: 0,
            ..AlchemistFacts::default()
        };
        assert_eq!(
            decide(&CraftFacts::default(), &mistletoe),
            Decide::HarvestMistletoe
        );
        // Un tirage hors bornes est ramené, jamais une panique.
        let wild = AlchemistFacts {
            nether_wart_roll: 31,
            mistletoe_roll: 30,
            ..AlchemistFacts::default()
        };
        assert_eq!(
            decide(&CraftFacts::default(), &wild),
            Decide::HarvestNetherWart
        );
    }

    /// `:337-348` : avec une tâche, « aller à la hutte », puis « attendre le vidage »,
    /// puis l'état suivant.
    #[test]
    fn with_a_task_the_alchemist_walks_dumps_then_crafts() {
        let facts = AlchemistFacts::default();
        assert_eq!(
            decide(&CraftFacts { at_building: false, ..busy() }, &facts),
            Decide::Walking
        );
        assert_eq!(
            decide(&CraftFacts { actions_done: 1, ..busy() }, &facts),
            Decide::WaitingForDump
        );
        assert_eq!(decide(&busy(), &facts), Decide::Next(CraftState::GetRecipe));
    }

    /// `:654-685` : la sortie ne se prend que sur un alambic arrêté, porteur de la
    /// sortie et sans ingrédient.
    #[test]
    fn a_stand_is_harvested_only_when_it_is_done_and_clean() {
        let mut facts = brewing();
        let mut stand = Stand::new(Pos::new(1, 64, 1));
        stand.slots[0] = Some(Stack::new(facts.output.clone(), 1));
        facts.stands.push(stand);
        assert_eq!(stand_to_retrieve_from(&facts), Some(Pos::new(1, 64, 1)));
        assert_eq!(
            get_recipe(&busy(), &facts),
            BrewRecipe::Retrieve {
                at: Pos::new(1, 64, 1),
                max: 1
            }
        );
        let mut bubbling = facts.clone();
        bubbling.stands[0].brew_time = 200;
        assert_eq!(stand_to_retrieve_from(&bubbling), None, "`:678`");
        let mut other = facts.clone();
        other.brewing_recipe = false;
        assert_eq!(stand_to_retrieve_from(&other), None, "`:656-659`");
    }

    /// `:415-434` : un alambic qui brasse renvoie `CRAFT`, sinon la classe de base.
    #[test]
    fn a_working_stand_sends_the_alchemist_to_craft() {
        let mut facts = brewing();
        let mut stand = Stand::new(Pos::new(1, 64, 1));
        stand.brew_time = 100;
        facts.stands.push(stand);
        assert_eq!(get_recipe(&busy(), &facts), BrewRecipe::Brew);
        assert_eq!(get_recipe(&busy(), &brewing()), BrewRecipe::Base);
        assert_eq!(
            get_recipe(&CraftFacts::default(), &facts),
            BrewRecipe::NoTask
        );
    }

    /// `:855-884` : le compte des alambics qui brassent, puis le premier à emplir.
    #[test]
    fn the_smelt_check_counts_then_picks_a_stand() {
        let mut facts = brewing();
        facts.primary_skill = 10;
        for x in 0..2 {
            let mut stand = Stand::new(Pos::new(x, 64, 1));
            stand.brew_time = 100;
            stand.slots[INGREDIENT_SLOT] = Some(Stack::new(facts.ingredient.clone(), 1));
            facts.stands.push(stand);
        }
        assert_eq!(max_usable_stands(&facts), 2);
        assert_eq!(count_of_bubbling(&facts), 2);
        // `:855-859` : `burning >= getMaxUsableBrewingStands()` suffit à rendre la
        // main, même avec un troisième alambic libre — le compte passe avant la boucle.
        assert_eq!(able_to_brew(&busy(), &facts), Able::Committed);
        facts.stands.push(Stand::new(Pos::new(3, 64, 1)));
        assert_eq!(able_to_brew(&busy(), &facts), Able::Committed);
        // Avec deux niveaux de plus (3 alambics utilisables) et une demande qui n'est
        // pas encore couverte (`:855`, second terme), le troisième est empli.
        facts.primary_skill = 20;
        let wanted = CraftFacts {
            max_crafting_count: 8,
            ..busy()
        };
        assert_eq!(max_usable_stands(&facts), 3);
        assert_eq!(
            able_to_brew(&wanted, &facts),
            Able::Fill {
                at: Pos::new(3, 64, 1)
            }
        );
        // Rien qui brasse et rien à emplir (`:884`).
        assert_eq!(able_to_brew(&busy(), &brewing()), Able::Idle);
    }

    /// `:539-574` : la demande de poudre part avant la boucle, puis la cueillette,
    /// puis le tisonnage.
    #[test]
    fn the_fuel_request_goes_out_before_any_stand_is_looked_at() {
        let mut facts = brewing();
        facts.stands.push(Stand::new(Pos::new(1, 64, 1)));
        assert_eq!(
            check_fuel(&facts, craft_state(CraftState::Craft)),
            Fuel::Asked {
                amount: BREWING_MIN_FUEL_COUNT
            }
        );
        // `:565-566` : la requête est ouverte, la poudre n'est ni en main ni à la
        // hutte — Java dit « we need to wait for Fuel to arrive » et rend `getState()`.
        facts.open_fuel_request = true;
        assert_eq!(
            check_fuel(&facts, craft_state(CraftState::Craft)),
            Fuel::Wait(WorkRefusal::FuelOnItsWay)
        );
        facts.blaze_powder_in_hut = true;
        assert_eq!(check_fuel(&facts, craft_state(CraftState::Craft)), Fuel::Gather);
        facts.blaze_powder_in_inventory = true;
        assert_eq!(
            check_fuel(&facts, craft_state(CraftState::Craft)),
            Fuel::Stoke {
                at: Pos::new(1, 64, 1),
                resume: craft_state(CraftState::Craft)
            }
        );
        assert_eq!(
            check_fuel(&AlchemistFacts::default(), craft_state(CraftState::Craft)),
            Fuel::NotBrewing
        );
    }

    /// `:520` : l'événement de carburant ne se déclenche pas pendant une cueillette.
    #[test]
    fn the_fuel_event_stays_quiet_while_gathering() {
        let mut facts = brewing();
        facts.stands.push(Stand::new(Pos::new(1, 64, 1)));
        assert!(fuel_needed(&facts, craft_state(CraftState::Craft)));
        assert!(!fuel_needed(
            &facts,
            craft_state(CraftState::GatheringRequiredMaterials)
        ));
        assert!(!fuel_needed(&AlchemistFacts::default(), craft_state(CraftState::Craft)));
    }

    /// `:595-639` : trois sorties du tisonnage, dont le retour à l'état interrompu.
    #[test]
    fn stoking_reuses_the_state_it_interrupted() {
        let mut facts = brewing();
        assert_eq!(add_fuel(&facts, &WorkMemory::default()), Stoke::Restart);
        facts.blaze_powder_in_inventory = true;
        facts.stands.push(Stand::new(Pos::new(1, 64, 1)));
        let memory = WorkMemory {
            fuel_pos: Some(Pos::new(1, 64, 1)),
            pre_fuel_state: Some(craft_state(CraftState::Craft)),
            ..WorkMemory::default()
        };
        assert_eq!(
            add_fuel(&facts, &memory),
            Stoke::Stoked {
                resume: craft_state(CraftState::Craft)
            }
        );
        let looped = WorkMemory {
            pre_fuel_state: Some(WorkState::AddFuelToBrewingStand),
            ..memory
        };
        assert_eq!(
            add_fuel(&facts, &looped),
            Stoke::Stoked {
                resume: craft_state(CraftState::StartWorking)
            }
        );
        facts.blaze_powder_in_hut = true;
        facts.blaze_powder_in_inventory = false;
        assert_eq!(add_fuel(&facts, &memory), Stoke::Gather);
    }

    /// `:921-980` : une potion en main part dans les slots vides, sinon la hutte est
    /// sollicitée, sinon l'IA s'avoue désynchronisée.
    #[test]
    fn pouring_a_potion_picks_the_transfer_the_hut_can_serve() {
        let mut facts = brewing();
        let mut stand = Stand::new(Pos::new(1, 64, 1));
        stand.slots[BREWING_FUEL_SLOT] = Some(Stack::new(facts.ingredient.clone(), 8));
        facts.stands.push(stand);
        let memory = walk_to(Pos::new(1, 64, 1));
        let base = CraftFacts {
            has_recipe: true,
            request_count: 3,
            output_count: 1,
            max_crafting_count: 3,
            inputs: vec![Input::item(1, 0, 0), Input::item(1, 0, 2)],
            ..busy()
        };
        assert_eq!(fill_up(&base, &facts, &memory), Fill::Poured { slots: 3 });
        let from_hut = CraftFacts {
            inputs: vec![Input::item(1, 0, 0), Input::item(1, 9, 0)],
            ..base.clone()
        };
        assert_eq!(
            fill_up(&from_hut, &facts, &memory),
            Fill::GatherPotions { need: 3 }
        );
        let nowhere = CraftFacts {
            inputs: vec![Input::item(1, 0, 0), Input::item(1, 0, 0)],
            ..base
        };
        assert_eq!(fill_up(&nowhere, &facts, &memory), Fill::OutOfSync);
    }

    /// `:990-1001` : la branche de l'ingrédient ne s'ouvre qu'une fois les trois slots
    /// de potion pleins, et compte l'ingrédient trois fois (`:996`).
    #[test]
    fn the_ingredient_branch_triples_its_count() {
        // La potion d'entrée n'est pas la sortie primaire (eau louche -> potion
        // étrange) : `:917` compte la sortie, `:928` l'entrée.
        let mut facts = brewing();
        facts.output = NeedKey::plain("minecraft:awkward_potion");
        let mut stand = Stand::new(Pos::new(1, 64, 1));
        for slot in 0..POTION_SLOTS {
            stand.slots[slot] = Some(Stack::new(facts.potion.clone(), 1));
        }
        stand.slots[BREWING_FUEL_SLOT] = Some(Stack::new(facts.ingredient.clone(), 8));
        facts.stands.push(stand);
        let memory = walk_to(Pos::new(1, 64, 1));
        let base = CraftFacts {
            has_recipe: true,
            request_count: 1,
            output_count: 1,
            inputs: vec![Input::item(1, 5, 0), Input::item(1, 0, 0)],
            ..busy()
        };
        assert_eq!(
            fill_up(&base, &facts, &memory),
            Fill::GatherIngredient { need: 1 }
        );
        let covered = CraftFacts {
            request_count: 0,
            ..base
        };
        assert_eq!(fill_up(&covered, &facts, &memory), Fill::Full);
        // Sans potion dans le fourneau, rien ne se verse (`:945`, `:1011`).
        facts.stands[0].slots[BREWING_FUEL_SLOT] = None;
        assert_eq!(fill_up(&covered, &facts, &memory), Fill::Full);
    }

    /// `:894-909` : pas d'alambic, ou la cible disparue, sont deux refus nommés.
    #[test]
    fn a_missing_stand_is_named_before_anything_is_poured() {
        let empty = brewing();
        assert_eq!(
            fill_up(&busy(), &empty, &WorkMemory::default()),
            Fill::NoStand
        );
        // `:894-902` teste la liste **avant** `walkTo` (`:904`) : sans alambic, la
        // position mémorisée n'est même pas lue.
        assert_eq!(
            fill_up(&busy(), &empty, &walk_to(Pos::new(9, 64, 9))),
            Fill::NoStand
        );
        let mut facts = brewing();
        facts.stands.push(Stand::new(Pos::new(1, 64, 1)));
        assert_eq!(
            fill_up(&busy(), &facts, &WorkMemory::default()),
            Fill::StandGone
        );
        // `:904` : `walkTo` reste sur une position qui n'est plus un alambic.
        assert_eq!(
            fill_up(&busy(), &facts, &walk_to(Pos::new(9, 64, 9))),
            Fill::StandGone
        );
    }

    /// `:774-789` : la sortie se divise par la taille de la sortie primaire, et la
    /// demande se termine quand les deux compteurs tombent ensemble.
    #[test]
    fn the_retrieval_counts_steps_and_finalizes_when_both_counters_agree() {
        let mut facts = brewing();
        let mut stand = Stand::new(Pos::new(1, 64, 1));
        stand.slots[0] = Some(Stack::new(facts.output.clone(), 2));
        facts.stands.push(stand);
        let memory = walk_to(Pos::new(1, 64, 1));
        let base = CraftFacts {
            has_recipe: true,
            has_request: true,
            output_count: 1,
            max_crafting_count: 8,
            craft_counter: 2,
            ..busy()
        };
        assert_eq!(
            retrieve_brewed(&base, &facts, &memory),
            Retrieve::Delivered { steps: 2 }
        );
        let last = CraftFacts {
            craft_counter: 6,
            ..base.clone()
        };
        assert_eq!(
            retrieve_brewed(&last, &facts, &memory),
            Retrieve::Finalized { steps: 2 }
        );
        let degenerate = CraftFacts {
            output_count: 0,
            ..base
        };
        assert_eq!(
            retrieve_brewed(&degenerate, &facts, &memory),
            Retrieve::DegenerateOutput
        );
        assert_eq!(
            retrieve_brewed(&CraftFacts::default(), &facts, &memory),
            Retrieve::NoWork
        );
    }

    /// `:804-819` : la poudre usée se sort, mais il faut être sur place avant de
    /// regarder l'alambic.
    #[test]
    fn used_fuel_needs_the_citizen_there_first() {
        let mut facts = brewing();
        let mut stand = Stand::new(Pos::new(1, 64, 1));
        stand.slots[BREWING_FUEL_SLOT] = Some(Stack::new(NeedKey::plain("minecraft:blaze_powder"), 1));
        facts.stands.push(stand);
        let memory = walk_to(Pos::new(1, 64, 1));
        assert_eq!(retrieve_used_fuel(&facts, &memory), Retrieval::Extracted);
        let mut away = facts.clone();
        away.at_work_pos = false;
        assert_eq!(retrieve_used_fuel(&away, &memory), Retrieval::Walking);
        assert_eq!(
            retrieve_used_fuel(&facts, &WorkMemory::default()),
            Retrieval::NoWork
        );
    }

    /// `:1061-1113` : `craft` de l'alchimiste, dans l'ordre de Java.
    #[test]
    fn the_alchemist_craft_check_follows_javas_order() {
        let facts = brewing();
        assert_eq!(
            craft(&CraftFacts { at_building: false, ..busy() }, &facts),
            BrewCraft::Walking {
                delay: STANDARD_DELAY
            }
        );
        assert_eq!(craft(&busy(), &facts), BrewCraft::NoStand);
        assert_eq!(
            craft(&busy(), &AlchemistFacts::default()),
            BrewCraft::Delegated
        );
        let mut with_stand = brewing();
        with_stand.stands.push(Stand::new(Pos::new(1, 64, 1)));
        assert_eq!(
            craft(&busy(), &with_stand),
            BrewCraft::Able(Able::Fill {
                at: Pos::new(1, 64, 1)
            })
        );
        let mut done = with_stand.clone();
        let ingredient = done.ingredient.clone();
        done.stands[0].slots[INGREDIENT_SLOT] = Some(Stack::new(ingredient, 1));
        let finished = CraftFacts {
            has_request: true,
            max_crafting_count: 2,
            craft_counter: 2,
            ..busy()
        };
        assert_eq!(craft(&finished, &done), BrewCraft::Finished);
        let cancelled = CraftFacts {
            request_cancelled: true,
            craft_counter: 0,
            ..finished
        };
        assert_eq!(craft(&cancelled, &done), BrewCraft::Cancelled);
    }

    /// `:688-724` : une entrée manquante que la hutte a part en cueillette ; une entrée
    /// que personne n'a annule la requête **et** rend `CRAFT`.
    #[test]
    fn the_brewing_items_check_gathers_or_cancels_then_crafts() {
        let facts = brewing();
        let base = CraftFacts {
            has_recipe: true,
            max_crafting_count: 2,
            inputs: vec![Input::item(3, 6, 0)],
            ..busy()
        };
        assert_eq!(
            check_for_items(&base, &facts),
            BrewItems::Gather {
                input: 0,
                remaining: 6
            }
        );
        let nothing = CraftFacts {
            inputs: vec![Input::item(3, 0, 0)],
            ..base.clone()
        };
        assert_eq!(
            check_for_items(&nothing, &facts),
            BrewItems::Craft { cancelled: true }
        );
        let enough = CraftFacts {
            inputs: vec![Input::item(3, 0, 6)],
            ..base
        };
        assert_eq!(
            check_for_items(&enough, &facts),
            BrewItems::Craft { cancelled: false }
        );
        assert_eq!(
            check_for_items(&busy(), &AlchemistFacts::default()),
            BrewItems::NoRecipe
        );
    }

    /// `:473` et `:326` : les deux durées que l'alchimiste porte lui-même.
    #[test]
    fn the_brewing_durations_are_the_java_ones() {
        assert_eq!(acceleration_ticks(0), 0);
        assert_eq!(acceleration_ticks(9), 0);
        assert_eq!(acceleration_ticks(10), 2);
        assert_eq!(acceleration_ticks(55), 10);
        assert_eq!(TICKS_20_TIMES_20, 400);
        assert_eq!(DELAY_TO_HARVEST_NETHERWART, 30);
        assert_eq!(DELAY_TO_HARVEST_MISTLETOE, 30);
        assert_eq!(MISTLETOE_CHANCE, 40);
    }

    /// `:127-215` : la récolte de verrue, un tick par étape.
    #[test]
    fn the_wart_harvest_walks_one_cell_at_a_time() {
        let mut facts = brewing();
        facts.soil.push(Soil {
            at: Pos::new(5, 60, 5),
            loaded: true,
            soul_sand: true,
            above: Above::NetherWart { age: 3 },
        });
        assert_eq!(
            harvest::harvest_nether_wart(&facts, &WorkMemory::default()),
            Wart::Picked {
                at: Pos::new(5, 60, 5)
            }
        );
        let memory = walk_to(Pos::new(5, 60, 5));
        assert_eq!(
            harvest::harvest_nether_wart(&facts, &memory),
            Wart::Mined {
                at: Pos::new(5, 60, 5)
            }
        );
        let mut unripe = facts.clone();
        unripe.soil[0].above = Above::NetherWart { age: 1 };
        assert_eq!(
            harvest::harvest_nether_wart(&unripe, &memory),
            Wart::NotRipe {
                at: Pos::new(5, 60, 5)
            }
        );
        let mut empty = facts.clone();
        empty.soil[0].above = Above::Empty;
        assert_eq!(
            harvest::harvest_nether_wart(&empty, &memory),
            Wart::NoWart
        );
        assert_eq!(
            harvest::harvest_nether_wart(&brewing(), &WorkMemory::default()),
            Wart::NoSoil
        );
    }

    /// `:236-303` : le gui demande des cisailles, une feuille, et un coup sur
    /// quarante.
    #[test]
    fn the_mistletoe_harvest_asks_for_shears_then_hits() {
        let mut facts = brewing();
        assert_eq!(
            harvest::harvest_mistletoe(&facts, &WorkMemory::default()),
            Mistletoe::NoLeaves
        );
        facts.leaves.push(crate::ai_work_01::facts::Leaf {
            at: Pos::new(7, 70, 7),
            loaded: true,
            leaves: true,
        });
        assert_eq!(
            harvest::harvest_mistletoe(&facts, &WorkMemory::default()),
            Mistletoe::Picked {
                at: Pos::new(7, 70, 7)
            }
        );
        let memory = walk_to(Pos::new(7, 70, 7));
        assert_eq!(
            harvest::harvest_mistletoe(&facts, &memory),
            Mistletoe::Hit
        );
        let mut lucky = facts.clone();
        lucky.mistletoe_roll_forty = 0;
        assert_eq!(
            harvest::harvest_mistletoe(&lucky, &memory),
            Mistletoe::Found
        );
        facts.shears_missing = true;
        assert_eq!(
            harvest::harvest_mistletoe(&facts, &memory),
            Mistletoe::NoShears
        );
    }

    /// `:105-112` : la table de l'alchimiste couvre ses six états et ceux de la
    /// classe de base, et aucune cible ne tombe dans le piège.
    #[test]
    fn the_alchemist_table_covers_every_state_it_can_reach() {
        let mut machine = super::super::state::machine();
        register(&mut machine);
        for state in [
            WorkState::FillUpBrewingStand,
            WorkState::RetrieveBrewed,
            WorkState::RetrieveUsedFuel,
            WorkState::AddFuelToBrewingStand,
            WorkState::HarvestMistletoe,
            WorkState::HarvestNetherWart,
            craft_state(CraftState::Idle),
            craft_state(CraftState::Craft),
        ] {
            assert!(machine.can_leave(state), "{}", state.java());
        }
        // `:106` : l'accélération passe avant la liste de l'état et consomme le tick —
        // c'est l'ordre des trois passes de `TickRateStateMachine.java:75-97`.
        let mut scene = Scene::new(
            CraftFacts::default(),
            TradeFacts::Alchemist(AlchemistFacts::default()),
        );
        let first = machine.tick(&mut scene).expect("accelerateBrewingStand");
        assert_eq!(first.rule, "accelerateBrewingStand");
        assert_eq!(machine.state(), craft_state(CraftState::Idle));
        // Le tick suivant rend la main à `IDLE`, qui part (`hasWorkToDo` toujours vrai,
        // `:1117-1121`).
        let second = machine.tick(&mut scene).expect("hasWorkToDo");
        assert_eq!(second.rule, "hasWorkToDo");
        assert_eq!(machine.state(), craft_state(CraftState::StartWorking));
    }
}
