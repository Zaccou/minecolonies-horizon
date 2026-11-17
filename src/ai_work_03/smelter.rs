//! Les huit décisions du souffleur de verre, et les sept cibles qui les appellent.
//!
//! Référence Java : `core/entity/ai/workers/crafting/AbstractEntityAIRequestSmelter.java`
//! — le constructeur (`:68-83`), `checkForLeftOvers` (`:89-98`), `checkFurnaceFuel`
//! (`:103-115`), `accelerateFurnaces` (`:120-142`), `getExtendedCount` (`:145-172`),
//! `addFuelToFurnace` (`:178-219`), `retrieveProductFromFurnace` (`:227-277`),
//! `retrieveUnrelatedProductFromFurnace` (`:285-320`), `fillUpFurnace` (`:327-424`) et
//! `executeCraftingAction` (`:427-500`), plus les six scans de [`super::furnace`].
//!
//! ## La table que ce fichier ajoute à celle du crafter
//!
//! | # | Passe / état | Règle | Rythme | Java |
//! | ---: | --- | --- | ---: | --- |
//! | 1 | `EVENT` | `checkFurnaceFuel` | 200 | `:73` |
//! | 2 | `EVENT` | `accelerateFurnaces` | 20 | `:74` |
//! | 3 | `EVENT` | `checkForLeftOvers` → `RETRIEVING_UNRELATED_PRODUCT_FROM_FURNACE` | 200 | `:75` |
//! | 4 | `RETRIEVING_END_PRODUCT_FROM_FURNACE` | `retrieveProductFromFurnace` | 20 | `:78` |
//! | 5 | `RETRIEVING_UNRELATED_PRODUCT_FROM_FURNACE` | `retrieveUnrelatedProductFromFurnace` | 20 | `:79` |
//! | 6 | `FILL_UP_FURNACES` | `fillUpFurnace` | 20 | `:80` |
//! | 7 | `ADD_FUEL_TO_FURNACE` | `addFuelToFurnace` | 20 | `:81` |
//!
//! **Les trois passes de fond ne changent presque jamais d'état.** `checkFurnaceFuel` et
//! `accelerateFurnaces` rendent `false` (`:114`, `:141`), ce que l'`AIEventTarget` de
//! Java lit comme « ne transitionne pas » ; `checkForLeftOvers` rend `true` seulement
//! quand il a trouvé un four à nettoyer (`:92-96`), et la cible constante fait le reste.
//! Le port garde la même forme : une action qui rend `Some(état)` transitionne, une
//! action qui rend `None` laisse le tick continuer (`Action::Run`, `None` = le `return
//! null` de Java).
//!
//! **Ce que ce fichier n'écrit pas.** Aucun slot, aucun item, aucun tick de four :
//! chaque écriture de Java devient une [`TradeAction`] que l'appelant exécute. C'est la
//! seule façon de tester « le souffleur sort le verre du four » sans un monde.
//!
//! **Une trouvaille, pas une correction.** Le `hitBlockWithToolInHand(worker,
//! furnacePos)` de `:412` est **mort** : `:381` vient de poser `furnacePos = null`, et
//! `CitizenItemUtils.hitBlockWithToolInHand` rend immédiatement quand la position est
//! nulle (`CitizenItemUtils.java:135-141`). Le port ne l'émet donc pas, et
//! `filling_a_furnace_never_swings_at_a_null_position` le tient : le transfert part, le
//! coup d'épée non.
//!
//! ## La discipline d'emprunt, et pourquoi elle est visible
//!
//! Chaque décision déstructure la scène en ses champs (`let Scene { base, smelter,
//! memory, .. } = scene;`). Ce n'est pas un style : les quatre états lisent
//! `smelter` **et** écrivent `memory` dans le même appel, et deux emprunts d'un même
//! `struct` ne cohabitent que s'ils portent sur des champs différents. C'est la même
//! raison que le `gather_action!` de `crate::ai_gather`.

use crate::ai::Pos;
use crate::ai_build::{CraftFacts, CraftState};

use super::facts::{Furnace, Scene, SmelterFacts, FUEL_SLOT, STACKSIZE};
use super::furnace::{self, OutputScan, UnrelatedScan};
use super::state::{base, Decision, TradeAction, TradeRefusal, TradeState};
use super::{STANDARD_DELAY, TICKS_SECOND};

/// `AbstractEntityAIRequestSmelter.BASE_XP_GAIN` (`:56`) : 5 xp par extraction de la
/// sortie d'un four (`:622`). Ce n'est pas une décision — l'appelant crédite — mais la
/// constante est citée parce qu'elle accompagne l'action.
pub const BASE_XP_GAIN: f64 = 5.0;
/// Le rythme des quatre états et de l'accélération (`:74`, `:78-81`) :
/// `Constants.TICKS_SECOND` (`Constants.java:21`).
pub const TICK_RATE: u32 = TICKS_SECOND;
/// `:73` et `:75` — `TICKS_SECOND * 10`, une fois toutes les dix secondes.
pub const SLOW_TICK_RATE: u32 = TICKS_SECOND * 10;

/// `checkFurnaceFuel` (`:103-115`) : demander du carburant à la colonie.
///
/// Trois conditions, dans l'ordre de Java : rien dans l'inventaire, moins d'un dans la
/// hutte, aucune requête de `StackList` déjà ouverte. La liste de carburants est
/// [`furnace::active_fuels`] — celle de `getActivePossibleFuels` (`:105`), pas la liste
/// brute.
///
/// **La liste peut être vide et Java demande quand même.** `getActivePossibleFuels`
/// rend `ImmutableList.of()` quand le joueur n'a déclaré aucun carburant (`:711-717`),
/// et la condition de `:107-109` ne teste pas la liste : la requête part avec zéro
/// item. Le port garde ce comportement — c'est `getAllowedFuel` qui refuse, plus loin,
/// à `:456-463` — et `a_fuel_request_can_be_asked_with_an_empty_list` le fige.
pub fn check_furnace_fuel(facts: &SmelterFacts) -> Option<TradeAction> {
    let fuels = furnace::active_fuels(
        &facts.fuel_list,
        facts.output.as_ref(),
        facts.smeltable.as_ref(),
    ); // `:105`
    // `:107` — `hasItemInItemHandler(inv, isCorrectFuel(possibleFuels))`.
    let in_inventory = facts.fuel_in_inventory
        && facts
            .fuel_list
            .iter()
            .any(|fuel| furnace::is_correct_fuel(&fuels, fuel));
    let in_building = facts.fuel_in_building > 0; // `:108`, `hasBuildingEnoughElseCount(...) < 1`
    if in_inventory || in_building || facts.open_fuel_request {
        return None; // `:107-109` — nothing to ask
    }
    Some(TradeAction::RequestFuel {
        size: furnace::fuel_request_size(facts.furnaces.len()), // `:111`
        min: furnace::FUEL_REQUEST_MIN,
        fuels,
    })
}

/// `accelerateFurnaces` (`:120-142`) : les ticks de four que la compétence secondaire
/// offre, un par four **chargé et allumé**.
///
/// Java boucle `accelerationTicks` fois et n'appelle `serverTick` que sur un four qui
/// brûle (`:131-136`) ; un four éteint reçoit donc zéro tick, et une compétence sous 10
/// n'en offre aucun. Le port ne rend pas d'action pour zéro tick : une action qui ne
/// fait rien n'est pas une action.
pub fn accelerate_furnaces(facts: &SmelterFacts, secondary_skill: u8) -> Vec<TradeAction> {
    let ticks = furnace::acceleration_ticks(secondary_skill); // `:122`
    if ticks == 0 {
        return Vec::new(); // `:131` — the loop body never runs
    }
    facts
        .furnaces
        .iter()
        .filter(|four| four.loaded && four.lit) // `:126`, `:133`
        .map(|four| TradeAction::Accelerate {
            at: four.at,
            ticks,
        })
        .collect()
}

/// `checkForLeftOvers` (`:89-98`) : la passe de fond qui envoie vers
/// `RETRIEVING_UNRELATED_PRODUCT_FROM_FURNACE` quand un four porte une entrée qui n'est
/// pas la nôtre.
///
/// Elle fait ce que Java fait au passage : retirer de la hutte une position qui n'est
/// plus un four (`:597`, via [`furnace::unrelated_target`]).
pub fn check_for_left_overs(scene: &mut Scene) -> Option<TradeState> {
    let found = {
        let Scene { smelter, action, .. } = scene;
        unrelated_position(smelter.as_mut(), action)
    };
    let at = found?; // `:91`
    scene.memory.furnace_pos = Some(at); // `:94`
    Some(TradeState::RetrieveUnrelatedProduct) // `:95`
}

/// Le four à nettoyer, s'il y en a un, avec le retrait d'une position disparue.
///
/// Les deux appelants de Java passent par ici : `checkForLeftOvers` (`:91`) et
/// `executeCraftingAction` (`:471`).
fn unrelated_position(
    smelter: Option<&mut SmelterFacts>,
    action: &mut Option<TradeAction>,
) -> Option<Pos> {
    let smelter = smelter?;
    let scan = furnace::unrelated_target(&smelter.furnaces, smelter.smeltable.as_ref());
    match scan {
        UnrelatedScan::None => None, // `:601`
        UnrelatedScan::At(index) => smelter.furnaces.get(index).map(|four| four.at),
        UnrelatedScan::Forget(index) => {
            // `:595-599` — the hut forgets the position, and the scan stops there.
            let gone = if index < smelter.furnaces.len() {
                Some(smelter.furnaces.remove(index).at)
            } else {
                None
            };
            if let Some(at) = gone {
                *action = Some(TradeAction::ForgetFurnace(at));
            }
            None
        }
    }
}

/// Le four à vider, s'il y en a un (`:465-469`).
fn output_position(smelter: Option<&SmelterFacts>) -> Option<Pos> {
    let facts = smelter?;
    match furnace::output_target(&facts.furnaces) {
        OutputScan::None => None, // `:574`
        OutputScan::At(index) => facts.furnaces.get(index).map(|four| four.at),
    }
}

/// Le four que la mémoire désigne, s'il est encore là.
fn chosen_furnace<'a>(
    smelter: Option<&'a SmelterFacts>,
    at: Option<Pos>,
) -> Option<(Pos, &'a Furnace)> {
    let at = at?;
    let facts = smelter?;
    let four = facts.furnaces.iter().find(|four| four.at == at)?;
    Some((at, four))
}

/// `executeCraftingAction` (`:427-500`) : ce que `CRAFT` devient quand la recette passe
/// par un four.
///
/// **L'ordre est la règle**, et c'est celui de Java : une recette qui ne passe pas par
/// un four rend la main au crafter (`:434-437`) ; puis aucune hutte de fours
/// (`:439-448`), des fours déchargés (`:450-453`), aucun carburant autorisé
/// (`:455-463`), une sortie à prendre (`:465-469`), une entrée étrangère à sortir
/// (`:471-475`), du carburant à remettre (`:477-484`), un jeu de fours déjà saturé
/// (`:486-491`), un four vide et alimenté à remplir (`:493-497`), et sinon le retour au
/// travail (`:499`).
pub fn execute_crafting_action(scene: &mut Scene) -> Decision {
    if !scene.base.has_recipe {
        return Decision::go(base(CraftState::StartWorking)); // `:429-432`
    }
    // The furnace path is the glassblower's; the base class's execution is the
    // crafter's, and it is called rather than copied (`:434-437`).
    let delegates = match scene.smelter.as_ref() {
        Some(facts) => !facts.intermediate_is_furnace,
        None => true,
    };
    if delegates {
        return super::execute_base(&mut scene.base);
    }
    // `:439-448` — no furnace in the hut at all.
    let empty_hut = scene
        .smelter
        .as_ref()
        .map(|facts| facts.furnaces.is_empty())
        .unwrap_or(true);
    if empty_hut {
        scene.memory.delay = STANDARD_DELAY as i32; // `:446`
        return Decision::go_after(base(CraftState::StartWorking), TradeRefusal::NoFurnaces);
    }
    let unloaded = scene
        .smelter
        .as_ref()
        .map(|facts| !furnace::are_all_loaded(&facts.furnaces))
        .unwrap_or(false);
    if unloaded {
        return Decision::go_after(base(CraftState::StartWorking), TradeRefusal::FurnacesUnloaded);
    }
    let no_fuel = scene
        .smelter
        .as_ref()
        .map(|facts| facts.fuel_list.is_empty())
        .unwrap_or(false);
    if no_fuel {
        // `:455-463` — `getAllowedFuel().isEmpty()`: an interaction and `getState()`.
        return Decision::refuse(TradeRefusal::NoFuelAllowed);
    }
    if let Some(at) = output_position(scene.smelter.as_ref()) {
        scene.memory.furnace_pos = Some(at); // `:465`
        return Decision::go(TradeState::RetrieveProductFromFurnace); // `:468`
    }
    let left_over = {
        let Scene { smelter, action, .. } = scene;
        unrelated_position(smelter.as_mut(), action)
    };
    if let Some(at) = left_over {
        scene.memory.furnace_pos = Some(at); // `:471`
        return Decision::go(TradeState::RetrieveUnrelatedProduct); // `:474`
    }
    let Some(facts) = scene.smelter.as_ref() else {
        return Decision::refuse(TradeRefusal::NoFactsForTrade);
    };
    if furnace::has_spare_fuel(facts.fuel_in_building, facts.fuel_in_inventory) {
        // `:477-484`
        if let Some(index) = furnace::without_fuel(&facts.furnaces) {
            if let Some(four) = facts.furnaces.get(index) {
                scene.memory.furnace_pos = Some(four.at); // `:479`
                return Decision::go(TradeState::AddFuelToFurnace); // `:482`
            }
        }
    }
    let burning = furnace::count_of_burning(&facts.furnaces); // `:486`
    let primary = scene
        .trade
        .primary_skill()
        .map(|skill| scene.skills.level(skill))
        .unwrap_or(0);
    let max_furnaces = furnace::max_usable_furnaces(primary, &facts.furnaces); // `:487`
    if burning > 0 && burning >= max_furnaces {
        scene.memory.delay = TICKS_SECOND as i32; // `:489`
        return Decision::refuse(TradeRefusal::AllFurnacesBusy); // `:490`
    }
    if let Some(index) = furnace::empty_with_fuel(&facts.furnaces) {
        if let Some(four) = facts.furnaces.get(index) {
            scene.memory.furnace_pos = Some(four.at); // `:493`
            return Decision::go(TradeState::FillUpFurnaces); // `:496`
        }
    }
    Decision::go(base(CraftState::StartWorking)) // `:499`
}

/// `retrieveProductFromFurnace` (`:227-277`) : sortir ce que le four a cuit.
///
/// Le produit de la recette part en livraison et incrémente le compteur de fabrications ;
/// quand le compteur atteint le maximum demandé, la tâche est finalisée
/// (`:264-267`). Ce qui n'est pas le produit de la recette part en sortie secondaire
/// (`:270-272`) — et c'est le cas du verre d'une autre recette.
pub fn retrieve_product(scene: &mut Scene) -> Decision {
    if scene.memory.furnace_pos.is_none() || !scene.base.has_recipe || !scene.base.has_request {
        return Decision::go(base(CraftState::StartWorking)); // `:229-232`
    }
    let Some((at, four)) = chosen_furnace(scene.smelter.as_ref(), scene.memory.furnace_pos) else {
        scene.memory.furnace_pos = None; // `:237`
        return Decision::refuse(TradeRefusal::FurnaceGone); // `:235-239`
    };
    let Some(result) = four.result.clone() else {
        scene.memory.furnace_pos = None; // `:237`
        return Decision::refuse(TradeRefusal::ResultSlotEmpty); // `:235`
    };
    if !scene.at_chosen_furnace() {
        return Decision::stay().asking(TradeAction::WalkToFurnace(at)); // `:241-244`
    }
    scene.memory.furnace_pos = None; // `:245`

    // `:248` — `extractFromFurnaceSlot(furnace, RESULT_SLOT)` succeeded.
    let ours = scene
        .smelter
        .as_ref()
        .and_then(|facts| facts.output.clone())
        .map(|output| output == result.key)
        .unwrap_or(false);
    let extraction = TradeAction::ExtractProduct {
        at,
        key: result.key,
        count: result.count,
    };
    if !ours {
        scene.memory.secondary_outputs += 1; // `:271`
        return Decision::go(base(CraftState::StartWorking)).asking(extraction);
    }
    // `:255-267` — the request takes the delivery, the counters move.
    scene.base.craft_counter = scene.base.craft_counter.saturating_add(result.count); // `:259`
    if scene.base.max_crafting_count == 0 {
        scene.base.max_crafting_count = scene.base.request_count; // `:260-263`
    }
    if scene.base.craft_counter >= scene.base.max_crafting_count {
        return finalize(&mut scene.base).asking(extraction); // `:264-267`
    }
    Decision::go(base(CraftState::StartWorking)).asking(extraction) // `:276`
}

/// `retrieveUnrelatedProductFromFurnace` (`:285-320`) : sortir une entrée qui n'est pas
/// celle de la recette.
///
/// Le stack part en sortie secondaire et l'inventaire est vidé dans la foulée
/// (`:314-315`), parce qu'il vient d'arriver quelque chose qui n'a rien à y faire.
pub fn retrieve_unrelated(scene: &mut Scene) -> Decision {
    if scene.memory.furnace_pos.is_none() {
        return Decision::go(base(CraftState::StartWorking)); // `:287-290`
    }
    let Some((at, four)) = chosen_furnace(scene.smelter.as_ref(), scene.memory.furnace_pos) else {
        scene.memory.furnace_pos = None; // `:298`
        return Decision::refuse(TradeRefusal::FurnaceGone); // `:293-300`
    };
    let Some(input) = four.smeltable.clone() else {
        scene.memory.furnace_pos = None; // `:298`
        return Decision::refuse(TradeRefusal::SmeltableNotEmpty); // `:294`
    };
    // `:295-296` — our own input is not a leftover.
    let ours = scene
        .smelter
        .as_ref()
        .and_then(|facts| facts.smeltable.clone())
        .map(|key| key == input.key)
        .unwrap_or(false);
    if ours {
        scene.memory.furnace_pos = None; // `:298`
        return Decision::refuse(TradeRefusal::SmeltableNotEmpty);
    }
    if !scene.at_chosen_furnace() {
        return Decision::stay().asking(TradeAction::WalkToFurnace(at)); // `:302-305`
    }
    scene.memory.furnace_pos = None; // `:306`
    scene.memory.secondary_outputs += 1; // `:314`
    Decision::go(base(CraftState::InventoryFull)).asking(TradeAction::ExtractUnrelated {
        at,
        key: input.key,
        count: input.count,
    }) // `:315`
}

/// `fillUpFurnace` (`:327-424`) : mettre l'entrée de la recette dans un four.
///
/// **L'arithmétique de la répartition est dans [`furnace::fill_split`]** ; ce qui reste
/// ici est l'ordre des refus, et il compte : « la tâche est déjà couverte »
/// (`:348-351`) passe avant « je n'ai pas d'entrée » (`:353-358`), qui passe avant « tous
/// les fours brûlent » (`:362-365`).
pub fn fill_up(scene: &mut Scene) -> Decision {
    if scene.memory.furnace_pos.is_none() || !scene.base.has_recipe {
        return Decision::go(base(CraftState::StartWorking)); // `:329-332`
    }
    if scene.base.max_crafting_count == 0 {
        scene.base.max_crafting_count = scene.base.request_count; // `:334-337`
    }
    let Some((at, four)) = chosen_furnace(scene.smelter.as_ref(), scene.memory.furnace_pos) else {
        scene.memory.furnace_pos = None;
        return Decision::refuse(TradeRefusal::FurnaceGone);
    };
    let Some(facts) = scene.smelter.as_ref() else {
        return Decision::refuse(TradeRefusal::NoFactsForTrade);
    };
    // `:344-345` — what the furnaces already count, input and result side.
    let in_furnaces = facts
        .smeltable
        .as_ref()
        .map(|key| furnace::extended_count(facts, key))
        .unwrap_or(0);
    let result_in_furnaces = facts
        .output
        .as_ref()
        .map(|key| furnace::extended_count(facts, key))
        .unwrap_or(0);
    let in_inventory = facts.smeltable_in_inventory; // `:341-342`
    let remaining = i64::from(scene.base.max_crafting_count)
        .saturating_sub(i64::from(scene.base.craft_counter));
    // `:348` — `(max - counter) - inFurnaces - resultInFurnaces`.
    let pending = remaining - i64::from(in_furnaces) - i64::from(result_in_furnaces);
    if pending <= 0 {
        scene.memory.furnace_pos = None;
        return Decision::refuse(TradeRefusal::AlreadySmelting); // `:348-351`
    }
    if in_inventory == 0 {
        // `:353-358` — ask the hut for the input, and forget the furnace.
        let target = pending - i64::from(in_inventory); // `:346`
        let action = facts
            .smeltable
            .clone()
            .map(|key| TradeAction::RequestSmeltable {
                key,
                count: u32::try_from(target).unwrap_or(0),
            });
        let burning = furnace::count_of_burning(&facts.furnaces); // `:360`
        let primary = scene
            .trade
            .primary_skill()
            .map(|skill| scene.skills.level(skill))
            .unwrap_or(0);
        let max_furnaces = furnace::max_usable_furnaces(primary, &facts.furnaces); // `:361`
        let busy = furnace::more_burning_than_usable(burning, max_furnaces); // `:362`
        scene.memory.furnace_pos = None; // `:356`
        let decision = if busy {
            Decision::go_after(base(CraftState::StartWorking), TradeRefusal::AllFurnacesBusy)
        } else {
            Decision::go_after(
                base(CraftState::GatheringRequiredMaterials),
                TradeRefusal::NoSmeltableForTheHut,
            )
        };
        return match action {
            Some(action) => decision.asking(action),
            None => decision,
        };
    }
    let burning = furnace::count_of_burning(&facts.furnaces); // `:360`
    let primary = scene
        .trade
        .primary_skill()
        .map(|skill| scene.skills.level(skill))
        .unwrap_or(0);
    let max_furnaces = furnace::max_usable_furnaces(primary, &facts.furnaces); // `:361`
    if furnace::more_burning_than_usable(burning, max_furnaces) {
        scene.memory.furnace_pos = None;
        return Decision::refuse(TradeRefusal::AllFurnacesBusy); // `:362-365`
    }
    if !scene.at_chosen_furnace() {
        return Decision::stay().asking(TradeAction::WalkToFurnace(at)); // `:367-370`
    }
    scene.memory.furnace_pos = None; // `:381`
    if !four.has_fuel_and_no_smeltable() {
        // `:389` — the transfer only happens into a fuelled, empty furnace.
        return Decision::go(base(CraftState::Craft)); // `:423`
    }
    let to_transfer = furnace::fill_split(pending, in_inventory, max_furnaces, burning); // `:391-409`
    if to_transfer == 0 {
        return Decision::go(base(CraftState::Craft)); // `:410`
    }
    // `:412`'s hit is dead code (`furnacePos` is null there, `:381`), so no
    // `HitFurnace` action is emitted: see the module note.
    Decision::go(base(CraftState::Craft)).asking(TradeAction::TransferSmeltable {
        at,
        count: to_transfer,
    }) // `:413-418`, then `:423`
}

/// `addFuelToFurnace` (`:178-219`) : remettre du carburant dans le four choisi.
///
/// Deux façons de ne rien faire : le citoyen n'en porte pas et la hutte en a (il part en
/// chercher, `:183-187`), ou personne n'en a (`:189-190`) ; et une façon de remplir : il
/// en porte et le slot est vide (`:208-213`).
pub fn add_fuel(scene: &mut Scene) -> Decision {
    let Some(facts) = scene.smelter.as_ref() else {
        return Decision::refuse(TradeRefusal::NoFactsForTrade);
    };
    let fuels = furnace::active_fuels(
        &facts.fuel_list,
        facts.output.as_ref(),
        facts.smeltable.as_ref(),
    ); // `:180`
    // `:181` — `hasItemInItemHandler(inv, isCorrectFuel(possibleFuels))`.
    let carries = facts.fuel_in_inventory && !fuels.is_empty();
    if !carries {
        // `:183-190` — the hut has some (go fetch), or nobody does.
        let from_hut = facts.fuel_in_building >= 1;
        let key = facts
            .fuel_list
            .iter()
            .find(|fuel| furnace::is_correct_fuel(&fuels, fuel))
            .cloned();
        scene.memory.furnace_pos = None; // `:189`
        return if from_hut {
            match key {
                Some(key) => Decision::go(base(CraftState::GatheringRequiredMaterials))
                    .asking(TradeAction::RequestSmeltable {
                        key,
                        count: STACKSIZE, // `:185`
                    }),
                None => Decision::go_after(
                    base(CraftState::GatheringRequiredMaterials),
                    TradeRefusal::NoFuelAllowed,
                ),
            }
        } else {
            Decision::go_after(base(CraftState::StartWorking), TradeRefusal::NoFuelAllowed)
        };
    }
    if scene.memory.furnace_pos.is_none() {
        return Decision::refuse(TradeRefusal::NoFurnaceChosen); // `:193-196`
    }
    let Some((at, four)) = chosen_furnace(scene.smelter.as_ref(), scene.memory.furnace_pos) else {
        scene.memory.furnace_pos = None;
        return Decision::go_after(base(CraftState::StartWorking), TradeRefusal::FurnaceGone);
    };
    let empty_slot = four.is_empty(FUEL_SLOT); // `:208`
    let loaded = four.loaded; // `:203`
    if !scene.at_chosen_furnace() {
        return Decision::stay().asking(TradeAction::WalkToFurnace(at)); // `:198-201`
    }
    scene.memory.furnace_pos = None; // `:217`
    if loaded && empty_slot {
        return Decision::go(base(CraftState::StartWorking)).asking(TradeAction::TransferFuel {
            at,
            count: STACKSIZE, // `:211`
        }); // `:218`
    }
    Decision::go(base(CraftState::StartWorking)) // `:218`
}

/// `finalizeCraftingTask` (`AbstractEntityAICrafting.java:636-646`) : la demande est
/// servie, les compteurs repartent à zéro (`resetValues`, `:668-677`) et l'inventaire
/// passe au vidage (`:646`).
fn finalize(base_facts: &mut CraftFacts) -> Decision {
    base_facts.max_crafting_count = 0; // `:670`
    base_facts.progress = 0; // `:671`
    base_facts.craft_counter = 0; // `:672`
    base_facts.dumped = false; // `:676`
    Decision::go(base(CraftState::InventoryFull)) // `:646`
}

/// Enregistrer les sept cibles de `AbstractEntityAIRequestSmelter.java:73-81`.
///
/// Les trois premières sont des passes globales (`Blocking::Event`) : elles sont vues
/// sur **tous** les états, à leur propre rythme. Les quatre suivantes n'existent que
/// pour un métier de four.
pub fn register(machine: &mut crate::ai::Machine<TradeState, Scene>) {
    use crate::ai::state::{Action, Blocking, Transition};
    // `:73` — `new AIEventTarget(EVENT, this::checkFurnaceFuel, this::getState,
    // TICKS_SECOND * 10)`. The predicate always answers false (`:114`), so the tick
    // goes on; the action only asks for fuel.
    machine.add(Transition::event(
        Blocking::Event,
        "checkFurnaceFuel",
        "AbstractEntityAIRequestSmelter.java:73",
        SLOW_TICK_RATE,
        Action::Run(fuel_action),
    ));
    // `:74` — `accelerateFurnaces`, once a second, on every state.
    machine.add(Transition::event(
        Blocking::Event,
        "accelerateFurnaces",
        "AbstractEntityAIRequestSmelter.java:74",
        TICK_RATE,
        Action::Run(accelerate_action),
    ));
    // `:75` — `checkForLeftOvers` → `RETRIEVING_UNRELATED_PRODUCT_FROM_FURNACE`, when
    // the predicate finds a furnace holding a foreign input.
    machine.add(Transition::event(
        Blocking::Event,
        "checkForLeftOvers",
        "AbstractEntityAIRequestSmelter.java:75",
        SLOW_TICK_RATE,
        Action::Run(leftovers_action),
    ));
    // `:78` — `new AITarget(RETRIEVING_END_PRODUCT_FROM_FURNACE, …)`.
    machine.add(Transition::target(
        TradeState::RetrieveProductFromFurnace,
        "retrieveProductFromFurnace",
        "AbstractEntityAIRequestSmelter.java:78",
        TICK_RATE,
        Action::Run(retrieve_product_action),
    ));
    // `:79` — `new AITarget(RETRIEVING_UNRELATED_PRODUCT_FROM_FURNACE, …)`.
    machine.add(Transition::target(
        TradeState::RetrieveUnrelatedProduct,
        "retrieveUnrelatedProductFromFurnace",
        "AbstractEntityAIRequestSmelter.java:79",
        TICK_RATE,
        Action::Run(retrieve_unrelated_action),
    ));
    // `:80` — `new AITarget(FILL_UP_FURNACES, …)`.
    machine.add(Transition::target(
        TradeState::FillUpFurnaces,
        "fillUpFurnace",
        "AbstractEntityAIRequestSmelter.java:80",
        TICK_RATE,
        Action::Run(fill_up_action),
    ));
    // `:81` — `new AITarget(ADD_FUEL_TO_FURNACE, …)`.
    machine.add(Transition::target(
        TradeState::AddFuelToFurnace,
        "addFuelToFurnace",
        "AbstractEntityAIRequestSmelter.java:81",
        TICK_RATE,
        Action::Run(add_fuel_action),
    ));
}

// --------------------------------------------------------------- les actions

/// `:73` — la passe de carburant ne transitionne jamais (`:114` rend `false`).
fn fuel_action(scene: &mut Scene, _: TradeState) -> Option<TradeState> {
    let asked = scene.smelter.as_ref().and_then(check_furnace_fuel);
    if let Some(action) = asked {
        scene.ask(action);
    }
    None
}

/// `:74` — l'accélération ne transitionne jamais non plus (`:141`).
fn accelerate_action(scene: &mut Scene, _: TradeState) -> Option<TradeState> {
    let secondary = scene
        .trade
        .secondary_skill()
        .map(|skill| scene.skills.level(skill))
        .unwrap_or(0);
    let first = scene
        .smelter
        .as_ref()
        .and_then(|facts| accelerate_furnaces(facts, secondary).into_iter().next());
    if let Some(action) = first {
        // One action per tick is what `scene.action` holds; a caller walks the rest
        // with `accelerate_furnaces`, which is pure and needs no machine.
        scene.ask(action);
    }
    None
}

/// `:75` — la seule des trois passes qui transitionne, et seulement quand elle a trouvé
/// quelque chose (`:92-96`).
fn leftovers_action(scene: &mut Scene, _: TradeState) -> Option<TradeState> {
    check_for_left_overs(scene)
}

fn retrieve_product_action(scene: &mut Scene, state: TradeState) -> Option<TradeState> {
    let decision = retrieve_product(scene);
    super::apply(scene, state, decision)
}

fn retrieve_unrelated_action(scene: &mut Scene, state: TradeState) -> Option<TradeState> {
    let decision = retrieve_unrelated(scene);
    super::apply(scene, state, decision)
}

fn fill_up_action(scene: &mut Scene, state: TradeState) -> Option<TradeState> {
    let decision = fill_up(scene);
    super::apply(scene, state, decision)
}

fn add_fuel_action(scene: &mut Scene, state: TradeState) -> Option<TradeState> {
    let decision = add_fuel(scene);
    super::apply(scene, state, decision)
}
