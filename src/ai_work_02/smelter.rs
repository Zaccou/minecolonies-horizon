//! Le four : ce que le cuisinier et le teinturier font de leurs fours.
//!
//! Référence Java : `AbstractEntityAIRequestSmelter.java` — `registerTargets`
//! (`:71-82`), `checkForLeftOvers` (`:89-98`), `checkFurnaceFuel` (`:103-115`),
//! `accelerateFurnaces` (`:120-142`), `getExtendedCount` (`:144-172`),
//! `addFuelToFurnace` (`:178-219`), `retrieveProductFromFurnace` (`:227-277`),
//! `retrieveUnrelatedProductFromFurnace` (`:285-320`), `fillUpFurnace` (`:327-424`)
//! et `executeCraftingAction` (`:426-500`). Les sept sélecteurs de four sont
//! `getEmptyFurnaceWithFuel` (`:508-525`), `getFurnaceWithoutFuel` (`:531-542`),
//! `getFurnaceToRetrieveOutputFrom` (`:551-575`),
//! `getFurnaceToRetrieveUnrelatedInputFrom` (`:581-602`), `extractFromFurnaceSlot`
//! (`:609-625`), `areFurnacesLoaded` (`:631-641`), `countOfBurningFurnaces`
//! (`:648-667`), `getMaxUsableFurnaces` (`:673-688`) et les deux listes de carburant
//! (`:695-737`).
//!
//! ## La table que ce fichier ajoute
//!
//! | # | État | Règle | Rythme | Java |
//! | ---: | --- | --- | ---: | --- |
//! | — | *(global)* | `checkFurnaceFuel` | 200 | `:73` |
//! | — | *(global)* | `accelerateFurnaces` | 20 | `:74` |
//! | — | *(global)* | `checkForLeftOvers` → `RETRIEVING_UNRELATED_…` | 200 | `:75` |
//! | 1 | `RETRIEVING_END_PRODUCT_FROM_FURNACE` | `retrieveProductFromFurnace` | 20 | `:78` |
//! | 2 | `RETRIEVING_UNRELATED_PRODUCT_FROM_FURNACE` | `retrieveUnrelatedProductFromFurnace` | 20 | `:79` |
//! | 3 | `FILL_UP_FURNACES` | `fillUpFurnace` | 20 | `:80` |
//! | 4 | `ADD_FUEL_TO_FURNACE` | `addFuelToFurnace` | 20 | `:81` |
//!
//! ## Le point d'entrée n'est pas la table, c'est `executeCraftingAction`
//!
//! Un four ne « décide » pas de sortir ou de remplir : c'est la fin d'un coup de
//! `CRAFT` qui l'envoie là. `AbstractEntityAIRequestSmelter.executeCraftingAction`
//! (`:426-500`) teste, dans cet ordre : les fours existent (`:439-448`), ils sont
//! chargés (`:450-453`), la liste de carburant n'est pas vide (`:455-463`), un four a
//! une sortie prête (`:465-469`), un four fond un intrus (`:471-475`), un four attend
//! du carburant (`:477-484`), tous ceux qu'on peut utiliser tournent (`:486-491`), un
//! four a du carburant et rien à fondre (`:493-497`). Chaque test qui répond oui
//! renvoie l'état correspondant ; le premier qui répond non passe au suivant.
//! [`execute_crafting_action`] est cette liste, dans cet ordre.
//!
//! ## Ce qui est un fait et ce qui est une écriture
//!
//! `walkToWorkPos(furnacePos)` (`:198`, `:241`, `:302`, `:367`) est un appel d'hôte :
//! sa réponse est le fait [`SmelterFacts::walked_to_furnace`]. Les transferts
//! (`:210-212`, `:413-418`) et la demande de carburant (`:111`) sont des **écritures**
//! que la règle ne peut pas faire : elles sortent dans la mémoire
//! ([`Transfer`], `fuel_request_made`) et l'appelant les exécute. Les particules et le
//! son (`:174-179`) ne sont pas portés : ce sont des effets visuels, et ils ne
//! changent aucun état.

use crate::ai::{Action, Blocking, Machine, Transition};
use crate::ai_build::work_time;

use super::facts::{Recipe, SmelterFacts, STACKSIZE, TICKS_SECOND};
use super::state::{
    self, Decision, Need, NeedItem, Scene, TradeRefusal, Transfer, WorkState, FURNACE_IDLE,
    FURNACE_RATE, SLOW_FURNACE_RATE,
};

/// `AbstractEntityAIRequestSmelter.BASE_XP_GAIN` (`:56`) : l'expérience d'une sortie
/// de four récupérée.
pub const BASE_XP_GAIN: f64 = 5.0;

/// Où ce fichier prend ses règles.
pub const SOURCE: &str = "AbstractEntityAIRequestSmelter.java:56,:71-82,:89-98,:103-115,\
:120-142,:144-172,:178-219,:227-277,:285-320,:327-424,:426-500,:508-688,:695-737";

/// Les quatre cibles du four (`:78-81`), à `TICKS_SECOND`.
pub fn register(machine: &mut Machine<WorkState, Scene>) {
    // :78 — `new AITarget(RETRIEVING_END_PRODUCT_FROM_FURNACE, this::retrieveProductFromFurnace, TICKS_SECOND)`.
    machine.add(Transition::target(
        WorkState::RetrievingEndProduct,
        "retrieveProductFromFurnace",
        "AbstractEntityAIRequestSmelter.java:78",
        FURNACE_RATE,
        Action::Run(retrieve_product_from_furnace),
    ));
    // :79 — `RETRIEVING_UNRELATED_PRODUCT_FROM_FURNACE`.
    machine.add(Transition::target(
        WorkState::RetrievingUnrelatedProduct,
        "retrieveUnrelatedProductFromFurnace",
        "AbstractEntityAIRequestSmelter.java:79",
        FURNACE_RATE,
        Action::Run(retrieve_unrelated_product_from_furnace),
    ));
    // :80 — `FILL_UP_FURNACES`.
    machine.add(Transition::target(
        WorkState::FillUpFurnaces,
        "fillUpFurnace",
        "AbstractEntityAIRequestSmelter.java:80",
        FURNACE_RATE,
        Action::Run(fill_up_furnace),
    ));
    // :81 — `ADD_FUEL_TO_FURNACE`.
    machine.add(Transition::target(
        WorkState::AddFuelToFurnace,
        "addFuelToFurnace",
        "AbstractEntityAIRequestSmelter.java:81",
        FURNACE_RATE,
        Action::Run(add_fuel_to_furnace),
    ));
}

/// Les trois `AIEventTarget` du four (`:73-75`) : des règles globales, vérifiées dans
/// tous les états, avant la liste de l'état courant
/// (`TickRateStateMachine.tick`).
///
/// Les deux premières rendent `false` dans Java — elles ne changent pas d'état, elles
/// demandent du carburant et accélèrent des fours. La troisième, elle, change d'état :
/// c'est `new AIEventTarget(EVENT, this::checkForLeftOvers,
/// RETRIEVING_UNRELATED_PRODUCT_FROM_FURNACE, TICKS_SECOND * 10)`.
pub fn register_events(machine: &mut Machine<WorkState, Scene>) {
    // :73 — `checkFurnaceFuel`, toutes les dix secondes.
    machine.add(Transition::event(
        Blocking::Event,
        "checkFurnaceFuel",
        "AbstractEntityAIRequestSmelter.java:73",
        SLOW_FURNACE_RATE,
        Action::Run(check_furnace_fuel),
    ));
    // :74 — `accelerateFurnaces`, chaque seconde.
    machine.add(Transition::event(
        Blocking::Event,
        "accelerateFurnaces",
        "AbstractEntityAIRequestSmelter.java:74",
        TICKS_SECOND,
        Action::Run(accelerate_furnaces),
    ));
    // :75 — `checkForLeftOvers` → `RETRIEVING_UNRELATED_PRODUCT_FROM_FURNACE`.
    machine.add(Transition::event_when(
        Blocking::Event,
        "checkForLeftOvers",
        "AbstractEntityAIRequestSmelter.java:75",
        SLOW_FURNACE_RATE,
        has_left_overs,
        Action::Run(take_left_overs),
    ));
}

/// `checkForLeftOvers` (`:89-98`) : y a-t-il un four qui fond autre chose que la
/// recette en cours ?
fn has_left_overs(scene: &Scene, _: WorkState) -> bool {
    let Some(smelter) = scene.job.smelter() else {
        return false;
    };
    smelter
        .unrelated_input(scene.recipe_input())
        .is_some()
}

/// La même question, et cette fois la réponse devient `furnacePos` (`:95`).
fn take_left_overs(scene: &mut Scene, _: WorkState) -> Option<WorkState> {
    let found = scene
        .job
        .smelter()
        .and_then(|smelter| smelter.unrelated_input(scene.recipe_input()));
    match found {
        Some(at) => {
            scene.memory.furnace_pos = Some(at); // `:95`
            Some(WorkState::RetrievingUnrelatedProduct)
        }
        None => None,
    }
}

/// `checkFurnaceFuel` (`:103-115`) : demander du carburant quand le citoyen n'en a
/// pas, que le bâtiment n'en a pas, et qu'aucune demande n'est ouverte.
///
/// Java crée la demande (`createRequestAsync(new StackList(...))`) ; un mod ne crée
/// pas de requête, donc le port lève le drapeau
/// [`TradeMemory::fuel_request_made`](super::state::TradeMemory::fuel_request_made) et
/// laisse l'appelant la créer. La règle rend `false` dans tous les cas (`:114`).
fn check_furnace_fuel(scene: &mut Scene, _: WorkState) -> Option<WorkState> {
    let (carries, in_building, open, size) = match scene.job.smelter() {
        Some(smelter) => {
            let fuels = smelter.active_fuels(scene.memory.recipe.as_ref());
            (
                smelter.carries_fuel(&fuels),
                smelter.building_fuel(&fuels),
                smelter.open_fuel_request,
                smelter.fuel_request_size(),
            )
        }
        None => (true, 1, true, 0),
    };
    if !carries && in_building < 1 && !open {
        // `:111` — `new StackList(possibleFuels, REQUESTS_TYPE_BURNABLE, STACKSIZE *
        // furnaces.size(), 1)`.
        scene.memory.fuel_request_made = true;
        scene.memory.needed = Some(Need::fuel(Vec::new(), size));
    }
    None // `:114` — `return false`
}

/// `accelerateFurnaces` (`:120-142`) : `(secondarySkill / 10) * 2` ticks de cuisson
/// par four allumé, par passage.
///
/// Java appelle `FurnaceBlockEntity.serverTick` en boucle ; le port compte les ticks
/// demandés dans [`TradeMemory::accelerated`](super::state::TradeMemory::accelerated)
/// et rend `false` (`:141`).
fn accelerate_furnaces(scene: &mut Scene, _: WorkState) -> Option<WorkState> {
    let (ticks, furnaces) = match scene.job.smelter() {
        Some(smelter) => (smelter.acceleration_ticks(), smelter.accelerating_furnaces()),
        None => (0, 0),
    };
    if ticks > 0 && furnaces > 0 {
        scene.memory.accelerated = scene
            .memory
            .accelerated
            .saturating_add(ticks.saturating_mul(furnaces));
    }
    None // `:141`
}

/// `executeCraftingAction` (`:426-500`) — la porte d'entrée de toute la table du four.
///
/// Les huit tests sont ceux de Java, dans l'ordre, et chacun nomme son refus.
pub fn execute_crafting_action(scene: &mut Scene) -> Decision {
    let Some(recipe) = scene.memory.recipe.clone() else {
        // `:429-432`
        return Decision::go(FURNACE_IDLE).because(TradeRefusal::NoRecipe);
    };
    if !recipe.furnace_intermediate {
        // `:434-437` — `return super.executeCraftingAction(toolSlot)`.
        return match state::base_execute(scene) {
            Some(next) => Decision::go(next),
            None => Decision::stay(),
        };
    }
    let Some(smelter) = scene.job.smelter() else {
        return Decision::stay().because(TradeRefusal::NoFactsForTrade);
    };

    // `:439-448` — `getFurnaces().isEmpty()` : interaction bloquante
    // `BAKER_HAS_NO_FURNACES_MESSAGE`, délai `STANDARD_DELAY`, `START_WORKING`.
    if smelter.furnace_count() == 0 {
        scene.memory.delay = Some(crate::ai::worker::STANDARD_DELAY); // `:446`
        return Decision::go(FURNACE_IDLE).because(TradeRefusal::NoFurnaces);
    }
    // Le sélecteur d'intrus de `:581-602` retire du module un bloc qui n'est plus un
    // four. C'est une écriture sur le bâtiment, donc une sortie, pas un transfert.
    if let Some(stale) = smelter.stale() {
        scene.memory.dropped_from_module = Some(stale); // `:597`
        return Decision::go(FURNACE_IDLE).because(TradeRefusal::StaleFurnace);
    }
    // `:450-453`
    if !smelter.all_loaded() {
        return Decision::go(FURNACE_IDLE).because(TradeRefusal::FurnacesNotLoaded);
    }
    // `:455-463` — `getAllowedFuel()`, la liste du module, **pas** la liste active :
    // Java teste ici la configuration, pas les interférences.
    if smelter.allowed().is_empty() {
        return Decision::stay().because(TradeRefusal::NoFuel); // `:462` — `return getState()`
    }
    // `:465-469`
    if let Some(at) = smelter.output_ready() {
        scene.memory.furnace_pos = Some(at);
        return Decision::go(WorkState::RetrievingEndProduct);
    }
    // `:471-475`
    if let Some(at) = smelter.unrelated_input(Some(&recipe.input)) {
        scene.memory.furnace_pos = Some(at);
        return Decision::go(WorkState::RetrievingUnrelatedProduct);
    }
    // `:477-484` — `> 1` dans le bâtiment **ou** au moins un dans l'inventaire.
    let fuels = smelter.allowed().to_vec();
    if smelter.building_fuel(&fuels) > 1 || smelter.carries_fuel(&fuels) {
        if let Some(at) = smelter.without_fuel() {
            scene.memory.furnace_pos = Some(at);
            return Decision::go(WorkState::AddFuelToFurnace);
        }
    }
    // `:486-491` — `burning > 0 && burning >= max`, et un délai d'une seconde.
    let burning = smelter.burning();
    if burning > 0 && burning >= smelter.max_usable_furnaces() {
        scene.memory.delay = Some(TICKS_SECOND); // `:489`
        return Decision::stay().because(TradeRefusal::FurnacesBusy);
    }
    // `:493-497`
    if let Some(at) = smelter.fuel_without_smeltable() {
        scene.memory.furnace_pos = Some(at);
        return Decision::go(WorkState::FillUpFurnaces);
    }
    Decision::go(FURNACE_IDLE) // `:499`
}

/// `retrieveProductFromFurnace` (`:227-277`) : sortir la sortie prête.
pub fn retrieve_product_from_furnace(scene: &mut Scene, _: WorkState) -> Option<WorkState> {
    Decision::go(retrieve_product(scene)).into_option(FURNACE_IDLE)
}

/// Le corps de `retrieveProductFromFurnace` (`:227-277`).
pub fn retrieve_product(scene: &mut Scene) -> WorkState {
    let Some(recipe) = scene.memory.recipe.clone() else {
        return FURNACE_IDLE; // `:229`
    };
    let (Some(at), true) = (scene.memory.furnace_pos, scene.memory.request_open) else {
        return FURNACE_IDLE; // `:229-232`
    };
    let Some(furnace) = scene.job.smelter().and_then(|smelter| smelter.at(at)).cloned() else {
        scene.memory.furnace_pos = None; // `:237`
        return FURNACE_IDLE;
    };
    let Some(result) = furnace.result.clone().filter(|slot| !slot.is_empty()) else {
        scene.memory.furnace_pos = None; // `:237`
        scene.memory.refusal = Some(TradeRefusal::NoOutputToRetrieve); // `:235-239`
        return FURNACE_IDLE;
    };
    if !walked_to_furnace(scene) {
        return WorkState::RetrievingEndProduct; // `:241-244`
    }
    scene.memory.furnace_pos = None; // `:245`

    // `:247` — la copie du stack, puis `extractFromFurnaceSlot(...)` (`:248`). Le port
    // n'a pas de slot à écrire : le fait « la sortie est là » **est** l'extraction, et
    // c'est ce que la mémoire enregistre.
    let count = result.count;
    if count > 0 {
        if result.key == recipe.output {
            // `:253-258` — `currentRequest.addDelivery(requestStack)`.
            scene.memory.delivered = scene.memory.delivered.saturating_add(count);
            scene.memory.craft_counter = scene.memory.craft_counter.saturating_add(count); // `:259`
            if scene.memory.max_crafting_count == 0 {
                scene.memory.max_crafting_count = scene.craft.request_count; // `:260-263`
            }
            scene.memory.experience += BASE_XP_GAIN; // `:622`
            if scene.memory.craft_counter >= scene.memory.max_crafting_count {
                let decision = state::finalize_crafting_task(scene); // `:264-267`
                scene.memory.refusal = decision.refusal;
                return decision.next.unwrap_or(FURNACE_IDLE);
            }
        } else {
            // `:271` — `job.getSecondaryOutputs().addTo(...)`.
            scene.memory.secondary_outputs.push((result.key, count));
        }
    }
    FURNACE_IDLE // `:276`
}

/// `retrieveUnrelatedProductFromFurnace` (`:285-320`) : sortir un intrus, pour que le
/// four puisse fondre ce qu'on lui demande.
///
/// L'extraction réussie envoie le citoyen vider son inventaire (`:315`) : Java écrit
/// `return INVENTORY_FULL` juste après avoir rangé le stack dans les sorties
/// secondaires. Aucun refus n'est nommé sur les sorties silencieuses : Java ne dit
/// rien, le port non plus.
pub fn retrieve_unrelated_product_from_furnace(scene: &mut Scene, _: WorkState) -> Option<WorkState> {
    let Some(at) = scene.memory.furnace_pos else {
        return Some(FURNACE_IDLE); // `:287-290`
    };
    let input = scene.recipe_input().cloned();
    let Some(furnace) = scene.job.smelter().and_then(|smelter| smelter.at(at)).cloned() else {
        scene.memory.furnace_pos = None;
        return Some(FURNACE_IDLE); // `:292-300`
    };
    let Some(slot) = furnace.smeltable.clone().filter(|slot| !slot.is_empty()) else {
        scene.memory.furnace_pos = None;
        return Some(FURNACE_IDLE); // `:294`
    };
    if input.as_ref().is_some_and(|key| slot.is(key)) {
        scene.memory.furnace_pos = None;
        return Some(FURNACE_IDLE); // `:295-296`
    }
    if !walked_to_furnace(scene) {
        return Some(WorkState::RetrievingUnrelatedProduct); // `:302-305`
    }
    scene.memory.furnace_pos = None; // `:306`

    let count = slot.count;
    if count > 0 {
        scene.memory.secondary_outputs.push((slot.key, count)); // `:314`
        return Some(WorkState::base(crate::ai_build::crafter::CraftState::InventoryFull)); // `:315`
    }
    Some(FURNACE_IDLE) // `:319`
}

/// `fillUpFurnace` (`:327-424`) : remplir un four de ce qu'il doit fondre.
///
/// C'est la seule règle de cette table qui décide d'un **partage** : quand plusieurs
/// fours sont disponibles, Java répartit la pile en donnant l'avance au premier
/// (`:405-408`), plafonné à `STACKSIZE` par four.
pub fn fill_up_furnace(scene: &mut Scene, _: WorkState) -> Option<WorkState> {
    Decision::go(fill_up(scene)).into_option(WorkState::FillUpFurnaces)
}

/// Le corps de `fillUpFurnace` (`:327-424`).
pub fn fill_up(scene: &mut Scene) -> WorkState {
    let Some(recipe) = scene.memory.recipe.clone() else {
        return FURNACE_IDLE; // `:329-332`
    };
    let Some(at) = scene.memory.furnace_pos else {
        return FURNACE_IDLE; // `:329-332`
    };
    // `:334-337` — le maximum vient de la requête quand le compteur est à zéro.
    if scene.memory.max_crafting_count == 0 {
        scene.memory.max_crafting_count = scene.craft.request_count;
    }
    let (in_furnaces, results_in_furnaces, burning, max_furnaces, has_fuel_no_smeltable) =
        match scene.job.smelter() {
            Some(smelter) => (
                smelter.extended_count(Some(&recipe), &recipe.input), // `:344`
                smelter.extended_count(Some(&recipe), &recipe.output), // `:345`
                smelter.burning(),                                    // `:360`
                smelter.max_usable_furnaces(),                        // `:361`
                smelter
                    .at(at)
                    .is_some_and(|furnace| furnace.fuel_without_smeltable()),
            ),
            None => return FURNACE_IDLE,
        };
    let counter = scene.memory.craft_counter;
    let max = scene.memory.max_crafting_count;
    // `:341-342` — `InventoryUtils.getItemCountInItemHandler(citizen, compare(input))`.
    let smeltable_in_inventory = scene
        .craft
        .inputs
        .first()
        .map_or(0, |input| input.in_inventory);

    // `:348-351` — tout est déjà dans les fours.
    let outstanding = max.saturating_sub(counter);
    if outstanding <= in_furnaces.saturating_add(results_in_furnaces) {
        return FURNACE_IDLE;
    }
    // `:353-358` — plus rien dans l'inventaire : demander le compte exact.
    if smeltable_in_inventory == 0 {
        let target = outstanding
            .saturating_sub(in_furnaces)
            .saturating_sub(results_in_furnaces);
        scene.memory.needed = Some(Need {
            item: NeedItem::Stack(recipe.input.clone()),
            count: target,
        });
        scene.memory.furnace_pos = None; // `:356`
        return WorkState::base(crate::ai_build::crafter::CraftState::GatheringRequiredMaterials);
    }
    // `:362-365`
    if burning > max_furnaces {
        return FURNACE_IDLE;
    }
    if !walked_to_furnace(scene) {
        return WorkState::FillUpFurnaces; // `:367-370`
    }
    let pending = outstanding
        .saturating_sub(in_furnaces)
        .saturating_sub(results_in_furnaces); // `:372`
    if pending == 0 {
        return FURNACE_IDLE; // `:373-376`
    }
    let available = pending.min(smeltable_in_inventory); // `:378`
    scene.memory.furnace_pos = None; // `:381`

    if has_fuel_no_smeltable {
        // `:392-409` — le partage entre les fours libres.
        let usable = max_furnaces.saturating_sub(burning);
        let to_transfer = transfer_count(available, usable);
        if to_transfer > 0 {
            scene.memory.transfer = Some(Transfer {
                item: NeedItem::Stack(recipe.input.clone()),
                count: to_transfer, // `:413-418`
                slot: super::facts::SMELTABLE_SLOT,
            });
        }
    }
    WorkState::base(crate::ai_build::crafter::CraftState::Craft) // `:423`
}

/// `addFuelToFurnace` (`:178-219`) : ravitailler un four.
pub fn add_fuel_to_furnace(scene: &mut Scene, _: WorkState) -> Option<WorkState> {
    Decision::go(add_fuel(scene)).into_option(WorkState::AddFuelToFurnace)
}

/// Le corps de `addFuelToFurnace` (`:178-219`).
pub fn add_fuel(scene: &mut Scene) -> WorkState {
    let Some(smelter) = scene.job.smelter() else {
        return FURNACE_IDLE;
    };
    // `:180` — `getActivePossibleFuels()`. Une liste vide est l'interaction
    // `FURNACE_USER_NO_FUEL` (`:713-718`) : Java continue quand même, donc le port
    // aussi, avec une liste vide.
    let fuels = smelter.active_fuels(scene.memory.recipe.as_ref());
    let carries = smelter.carries_fuel(&fuels);
    let in_building = smelter.building_fuel(&fuels);
    // `:181-191` — pas de carburant sur soi.
    if !carries {
        if in_building >= 1 {
            scene.memory.needed = Some(Need::fuel(fuels.clone(), STACKSIZE)); // `:185`
            return WorkState::base(crate::ai_build::crafter::CraftState::GatheringRequiredMaterials); // `:186`
        }
        scene.memory.furnace_pos = None; // `:189`
        return FURNACE_IDLE; // `:190`
    }
    let Some(at) = scene.memory.furnace_pos else {
        return FURNACE_IDLE; // `:193-196`
    };
    if !walked_to_furnace(scene) {
        return WorkState::AddFuelToFurnace; // `:198-201`
    }
    let (loaded, empty_fuel_slot) = match smelter.at(at) {
        Some(furnace) => (furnace.loaded, !furnace.has_fuel()),
        None => (false, false),
    };
    // `:203-215` — seulement dans un chunk chargé, et seulement dans un slot vide.
    if loaded && empty_fuel_slot && carries {
        scene.memory.transfer = Some(Transfer {
            item: NeedItem::AnyFuel(fuels.clone()),
            count: STACKSIZE, // `:211`
            slot: super::facts::FUEL_SLOT,
        });
    }
    scene.memory.furnace_pos = None; // `:217`
    FURNACE_IDLE // `:218`
}

/// `walkToWorkPos(furnacePos)` : le fait qui dit que le citoyen est arrivé.
fn walked_to_furnace(scene: &Scene) -> bool {
    scene
        .job
        .smelter()
        .is_some_and(|smelter| smelter.walked_to_furnace)
}

/// `:392-409` : combien d'items vont dans le four visé.
///
/// Java :
/// ```text
/// availableFurnaces >= 1 :
///   availableSmeltable > STACKSIZE * availableFurnaces → STACKSIZE
///   availableFurnaces == 1                            → availableSmeltable
///   sinon → min(availableSmeltable / availableFurnaces
///               + availableSmeltable % availableFurnaces, STACKSIZE)
/// ```
/// « On répartit et on charge le reste devant » (`:405-406`), c'est-à-dire que la
/// division entière laisse un reste qui revient au four courant.
pub fn transfer_count(available_smeltable: u32, available_furnaces: u32) -> u32 {
    if available_furnaces == 0 {
        return 0; // `:393` — `>= 1` est faux, `toTransfer` reste nul
    }
    if available_smeltable > STACKSIZE.saturating_mul(available_furnaces) {
        return STACKSIZE; // `:395-398`
    }
    if available_furnaces == 1 {
        return available_smeltable; // `:399-402`
    }
    let shared = available_smeltable / available_furnaces
        + available_smeltable % available_furnaces; // `:407`
    shared.min(STACKSIZE)
}

/// Le nombre de coups qu'une recette demande, tel que le parent le calcule
/// (`AbstractEntityAICrafting.java:734-738`), pour que ce module puisse le citer sans
/// redéfinir la formule.
pub fn required_hits(scene: &Scene) -> Result<u32, work_time::Refusal> {
    work_time::craft_hits(scene.job.skills().craft_speed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::Pos;
    use crate::ai_build::crafter::CraftState;
    use crate::ai_build::materials::NeedKey;
    use crate::ai_work_02::facts::{CrusherFacts, Furnace, Skills, Slot};

    fn key(name: &str) -> NeedKey {
        NeedKey::plain(name)
    }

    fn iron_recipe() -> Recipe {
        Recipe::smelted(key("minecraft:iron_ore"), key("minecraft:iron_ingot"))
    }

    /// Un four qui a du carburant et rien à fondre, la position qu'on lui donne.
    fn fed_furnace(at: Pos) -> Furnace {
        Furnace::new(at).with_fuel(Slot::new(key("minecraft:coal"), 8))
    }

    /// Une scène de cuisinier avec une recette de fonte en cours et une tâche.
    fn cooking() -> Scene {
        let mut scene = Scene::chef(SmelterFacts {
            furnaces: vec![fed_furnace(Pos::new(0, 64, 0))],
            allowed_fuel: vec![key("minecraft:coal")],
            fuel_in_inventory: 8,
            skills: Skills::new(10, 10, 10),
            walked_to_furnace: true,
            ..SmelterFacts::default()
        });
        scene.memory.recipe = Some(iron_recipe());
        scene.memory.request_open = true;
        scene.craft.task_queue = 1;
        scene.craft.current_task = true;
        scene.craft.request_count = 8;
        scene.available = Some(iron_recipe());
        scene
    }

    /// `:429-432` : sans recette, le four rend `START_WORKING` et le dit.
    #[test]
    fn without_a_recipe_the_furnace_goes_back_to_work() {
        let mut scene = Scene::chef(SmelterFacts::default());
        let decision = execute_crafting_action(&mut scene);
        assert_eq!(decision.next, Some(FURNACE_IDLE));
        assert_eq!(decision.refusal, Some(TradeRefusal::NoRecipe));
    }

    /// `:434-437` : une recette d'établi n'entre pas dans la table du four, elle
    /// retourne à la classe mère — ici `crafter::execute`.
    #[test]
    fn a_workbench_recipe_goes_back_to_the_parent() {
        let mut scene = cooking();
        scene.memory.recipe = Some(Recipe::crafted(
            key("minecraft:iron_ingot"),
            key("minecraft:iron_helmet"),
        ));
        scene.memory.craft_counter = 0;
        scene.memory.max_crafting_count = 2;
        let decision = execute_crafting_action(&mut scene);
        assert_eq!(
            decision.next,
            Some(WorkState::base(CraftState::GetRecipe)),
            "AbstractEntityAICrafting.java:634-636 — une passe de plus"
        );
    }

    /// `:439-448` : aucun four → interaction bloquante, délai standard, on repart
    /// travailler.
    #[test]
    fn no_furnace_is_a_named_refusal_with_a_delay() {
        let mut scene = cooking();
        scene.job = crate::ai_work_02::facts::TradeJob::chef(SmelterFacts::default());
        let decision = execute_crafting_action(&mut scene);
        assert_eq!(decision.next, Some(FURNACE_IDLE));
        assert_eq!(decision.refusal, Some(TradeRefusal::NoFurnaces));
        assert_eq!(scene.memory.delay, Some(crate::ai::worker::STANDARD_DELAY));
    }

    /// `:595-599` : un bloc qui n'est plus un four sort du module.
    #[test]
    fn a_stale_block_leaves_the_module() {
        let mut scene = cooking();
        scene.job = crate::ai_work_02::facts::TradeJob::chef(SmelterFacts {
            furnaces: vec![Furnace::new(Pos::new(3, 64, 3)).not_a_furnace()],
            allowed_fuel: vec![key("minecraft:coal")],
            ..SmelterFacts::default()
        });
        let decision = execute_crafting_action(&mut scene);
        assert_eq!(decision.refusal, Some(TradeRefusal::StaleFurnace));
        assert_eq!(scene.memory.dropped_from_module, Some(Pos::new(3, 64, 3)));
    }

    /// `:450-453` : un chunk déchargé suffit à refuser le passage.
    #[test]
    fn an_unloaded_furnace_stops_the_pass() {
        let mut scene = cooking();
        scene.job = crate::ai_work_02::facts::TradeJob::chef(SmelterFacts {
            furnaces: vec![fed_furnace(Pos::new(0, 64, 0)).unloaded()],
            allowed_fuel: vec![key("minecraft:coal")],
            ..SmelterFacts::default()
        });
        let decision = execute_crafting_action(&mut scene);
        assert_eq!(decision.refusal, Some(TradeRefusal::FurnacesNotLoaded));
        assert_eq!(decision.next, Some(FURNACE_IDLE));
    }

    /// `:455-463` : la liste **autorisée** vide est un refus qui reste sur place
    /// (`return getState()`), pas un retour au travail.
    #[test]
    fn an_empty_fuel_list_stays_put() {
        let mut scene = cooking();
        scene.job = crate::ai_work_02::facts::TradeJob::chef(SmelterFacts {
            furnaces: vec![fed_furnace(Pos::new(0, 64, 0))],
            allowed_fuel: Vec::new(),
            ..SmelterFacts::default()
        });
        let decision = execute_crafting_action(&mut scene);
        assert_eq!(decision.next, None, ":462 — `return getState()`");
        assert_eq!(decision.refusal, Some(TradeRefusal::NoFuel));
    }

    /// `:465-469` : une sortie prête passe **avant** un intrus et avant le
    /// ravitaillement.
    #[test]
    fn a_ready_output_is_taken_first() {
        let mut scene = cooking();
        scene.job = crate::ai_work_02::facts::TradeJob::chef(SmelterFacts {
            furnaces: vec![
                fed_furnace(Pos::new(0, 64, 0)),
                Furnace::new(Pos::new(1, 64, 0))
                    .with_result(Slot::new(key("minecraft:iron_ingot"), 3)),
            ],
            allowed_fuel: vec![key("minecraft:coal")],
            fuel_in_inventory: 8,
            ..SmelterFacts::default()
        });
        let decision = execute_crafting_action(&mut scene);
        assert_eq!(decision.next, Some(WorkState::RetrievingEndProduct));
        assert_eq!(scene.memory.furnace_pos, Some(Pos::new(1, 64, 0)));
        assert_eq!(decision.refusal, None);
    }

    /// `:471-475` : un intrus passe avant le ravitaillement, même quand les deux sont
    /// possibles.
    #[test]
    fn an_intruder_comes_before_fuelling() {
        let mut scene = cooking();
        scene.job = crate::ai_work_02::facts::TradeJob::chef(SmelterFacts {
            furnaces: vec![
                fed_furnace(Pos::new(0, 64, 0)),
                Furnace::new(Pos::new(1, 64, 0))
                    .with_smeltable(Slot::new(key("minecraft:gold_ore"), 2)),
            ],
            allowed_fuel: vec![key("minecraft:coal")],
            fuel_in_inventory: 8,
            ..SmelterFacts::default()
        });
        let decision = execute_crafting_action(&mut scene);
        assert_eq!(decision.next, Some(WorkState::RetrievingUnrelatedProduct));
        assert_eq!(scene.memory.furnace_pos, Some(Pos::new(1, 64, 0)));
    }

    /// `:477-484` : `building > 1` **ou** l'inventaire non vide, et un four sans
    /// carburant.
    #[test]
    fn fuelling_needs_a_fuel_source_and_a_cold_furnace() {
        let mut scene = cooking();
        scene.job = crate::ai_work_02::facts::TradeJob::chef(SmelterFacts {
            furnaces: vec![Furnace::new(Pos::new(0, 64, 0))],
            allowed_fuel: vec![key("minecraft:coal")],
            fuel_in_inventory: 0,
            fuel_in_building: 1,
            ..SmelterFacts::default()
        });
        let decision = execute_crafting_action(&mut scene);
        assert_eq!(
            decision.next,
            Some(FURNACE_IDLE),
            ":477 — `> 1` dans le bâtiment, pas `>= 1`"
        );

        scene.job = crate::ai_work_02::facts::TradeJob::chef(SmelterFacts {
            furnaces: vec![Furnace::new(Pos::new(0, 64, 0))],
            allowed_fuel: vec![key("minecraft:coal")],
            fuel_in_building: 2,
            ..SmelterFacts::default()
        });
        let decision = execute_crafting_action(&mut scene);
        assert_eq!(decision.next, Some(WorkState::AddFuelToFurnace));
        assert_eq!(scene.memory.furnace_pos, Some(Pos::new(0, 64, 0)));
    }

    /// `:486-491` : tous les fours utilisables tournent → on attend une seconde.
    #[test]
    fn every_usable_furnace_burning_waits_a_second() {
        let mut scene = cooking();
        scene.job = crate::ai_work_02::facts::TradeJob::chef(SmelterFacts {
            furnaces: vec![Furnace::new(Pos::new(0, 64, 0))
                .with_fuel(Slot::new(key("minecraft:coal"), 4))
                .with_smeltable(Slot::new(key("minecraft:iron_ore"), 1))
                .burning()],
            allowed_fuel: vec![key("minecraft:coal")],
            fuel_in_inventory: 4,
            skills: Skills::new(0, 0, 0),
            ..SmelterFacts::default()
        });
        let decision = execute_crafting_action(&mut scene);
        assert_eq!(decision.next, None, ":490 — `return getState()`");
        assert_eq!(decision.refusal, Some(TradeRefusal::FurnacesBusy));
        assert_eq!(scene.memory.delay, Some(TICKS_SECOND));
    }

    /// `:493-497` : un four qui a du carburant et rien à fondre est le four à remplir.
    #[test]
    fn a_fuelled_empty_furnace_is_the_one_to_fill() {
        let mut scene = cooking();
        let decision = execute_crafting_action(&mut scene);
        assert_eq!(decision.next, Some(WorkState::FillUpFurnaces));
        assert_eq!(scene.memory.furnace_pos, Some(Pos::new(0, 64, 0)));
    }

    /// `:227-277` : sortir la sortie compte la livraison, avance `craftCounter`, et
    /// garde le compteur maximum de la requête quand il était à zéro.
    #[test]
    fn retrieving_the_output_counts_the_delivery() {
        let mut scene = cooking();
        scene.job = crate::ai_work_02::facts::TradeJob::chef(SmelterFacts {
            furnaces: vec![
                Furnace::new(Pos::new(0, 64, 0))
                    .with_result(Slot::new(key("minecraft:iron_ingot"), 3)),
            ],
            allowed_fuel: vec![key("minecraft:coal")],
            walked_to_furnace: true,
            ..SmelterFacts::default()
        });
        scene.memory.furnace_pos = Some(Pos::new(0, 64, 0));
        scene.memory.max_crafting_count = 8;
        let next = retrieve_product(&mut scene);
        assert_eq!(next, FURNACE_IDLE, ":276 — retour au travail");
        assert_eq!(scene.memory.delivered, 3, ":257");
        assert_eq!(scene.memory.craft_counter, 3, ":259");
        assert_eq!(scene.memory.furnace_pos, None, ":245 — le four est libéré");
        assert_eq!(scene.memory.experience, BASE_XP_GAIN, ":622");
        assert!(scene.memory.secondary_outputs.is_empty());
    }

    /// `:260-267` : quand la dernière sortie comble le compte, la tâche se termine et
    /// l'inventaire doit être vidé.
    #[test]
    fn the_last_output_finalizes_the_task() {
        let mut scene = cooking();
        scene.job = crate::ai_work_02::facts::TradeJob::chef(SmelterFacts {
            furnaces: vec![
                Furnace::new(Pos::new(0, 64, 0))
                    .with_result(Slot::new(key("minecraft:iron_ingot"), 8)),
            ],
            allowed_fuel: vec![key("minecraft:coal")],
            walked_to_furnace: true,
            ..SmelterFacts::default()
        });
        scene.memory.furnace_pos = Some(Pos::new(0, 64, 0));
        scene.memory.max_crafting_count = 8;
        scene.craft.inventory_needs_dump = true;
        let next = retrieve_product(&mut scene);
        assert_eq!(next, WorkState::base(CraftState::InventoryFull), ":264-267");
        assert_eq!(scene.memory.recipe, None, ":638 — la recette est oubliée");
        assert_eq!(scene.memory.craft_counter, 0, ":639 — les compteurs sont remis");
        assert_eq!(scene.memory.experience, BASE_XP_GAIN + 4.0, ":643 — 8 / 2");
    }

    /// `:253-271` : une sortie qui n'est pas celle de la recette part dans les sorties
    /// secondaires au lieu d'être livrée.
    #[test]
    fn an_unexpected_output_becomes_a_secondary_output() {
        let mut scene = cooking();
        scene.job = crate::ai_work_02::facts::TradeJob::chef(SmelterFacts {
            furnaces: vec![
                Furnace::new(Pos::new(0, 64, 0))
                    .with_result(Slot::new(key("minecraft:gold_ingot"), 2)),
            ],
            allowed_fuel: vec![key("minecraft:coal")],
            walked_to_furnace: true,
            ..SmelterFacts::default()
        });
        scene.memory.furnace_pos = Some(Pos::new(0, 64, 0));
        scene.memory.max_crafting_count = 8;
        assert_eq!(retrieve_product(&mut scene), FURNACE_IDLE);
        assert_eq!(scene.memory.delivered, 0);
        assert_eq!(
            scene.memory.secondary_outputs,
            vec![(key("minecraft:gold_ingot"), 2)],
            ":271"
        );
    }

    /// `:234-239` : un four sans sortie nomme le refus et libère la position.
    #[test]
    fn an_empty_result_slot_names_the_refusal() {
        let mut scene = cooking();
        scene.memory.furnace_pos = Some(Pos::new(0, 64, 0));
        assert_eq!(retrieve_product(&mut scene), FURNACE_IDLE);
        assert_eq!(scene.memory.refusal, Some(TradeRefusal::NoOutputToRetrieve));
        assert_eq!(scene.memory.furnace_pos, None);
    }

    /// `:241-244` : tant que le citoyen n'est pas au four, la règle ne touche à rien.
    #[test]
    fn a_walking_citizen_keeps_the_state() {
        let mut scene = cooking();
        scene.job = crate::ai_work_02::facts::TradeJob::chef(SmelterFacts {
            furnaces: vec![
                Furnace::new(Pos::new(0, 64, 0))
                    .with_result(Slot::new(key("minecraft:iron_ingot"), 1)),
            ],
            allowed_fuel: vec![key("minecraft:coal")],
            walked_to_furnace: false,
            ..SmelterFacts::default()
        });
        scene.memory.furnace_pos = Some(Pos::new(0, 64, 0));
        assert_eq!(
            retrieve_product(&mut scene),
            WorkState::RetrievingEndProduct,
            ":243 — `return getState()`"
        );
        assert_eq!(scene.memory.furnace_pos, Some(Pos::new(0, 64, 0)));
        assert_eq!(scene.memory.delivered, 0);
    }

    /// `:285-320` : retirer un intrus envoie vider l'inventaire (`:315`).
    #[test]
    fn removing_an_intruder_sends_the_citizen_to_dump() {
        let mut scene = cooking();
        scene.job = crate::ai_work_02::facts::TradeJob::chef(SmelterFacts {
            furnaces: vec![Furnace::new(Pos::new(0, 64, 0))
                .with_smeltable(Slot::new(key("minecraft:gold_ore"), 4))],
            allowed_fuel: vec![key("minecraft:coal")],
            walked_to_furnace: true,
            ..SmelterFacts::default()
        });
        scene.memory.furnace_pos = Some(Pos::new(0, 64, 0));
        let next = retrieve_unrelated_product_from_furnace(&mut scene, WorkState::RetrievingUnrelatedProduct);
        assert_eq!(next, Some(WorkState::base(CraftState::InventoryFull)));
        assert_eq!(
            scene.memory.secondary_outputs,
            vec![(key("minecraft:gold_ore"), 4)],
            ":314"
        );
    }

    /// `:294-300` : un four qui fond **la** recette n'est pas un intrus, on le laisse.
    #[test]
    fn the_recipe_own_input_is_not_an_intruder() {
        let mut scene = cooking();
        scene.job = crate::ai_work_02::facts::TradeJob::chef(SmelterFacts {
            furnaces: vec![Furnace::new(Pos::new(0, 64, 0))
                .with_smeltable(Slot::new(key("minecraft:iron_ore"), 4))],
            allowed_fuel: vec![key("minecraft:coal")],
            walked_to_furnace: true,
            ..SmelterFacts::default()
        });
        scene.memory.furnace_pos = Some(Pos::new(0, 64, 0));
        let next = retrieve_unrelated_product_from_furnace(&mut scene, WorkState::RetrievingUnrelatedProduct);
        assert_eq!(next, Some(FURNACE_IDLE), ":295-300");
        assert!(scene.memory.secondary_outputs.is_empty());
        assert_eq!(scene.memory.furnace_pos, None);
    }

    /// `:353-358` : plus rien dans l'inventaire — demander le compte exact qui
    /// manque, fours et sorties déjà comptés.
    #[test]
    fn filling_without_items_asks_for_the_exact_shortfall() {
        let mut scene = cooking();
        scene.craft.inputs = vec![crate::ai_build::crafter::Input::item(1, 20, 0)];
        scene.memory.furnace_pos = Some(Pos::new(0, 64, 0));
        scene.memory.max_crafting_count = 8;
        scene.memory.craft_counter = 2;
        let next = fill_up(&mut scene);
        assert_eq!(next, WorkState::base(CraftState::GatheringRequiredMaterials));
        assert_eq!(
            scene.memory.needed,
            Some(Need {
                item: NeedItem::Stack(key("minecraft:iron_ore")),
                count: 6, // 8 − 2 − 0 − 0
            }),
            ":346, :355"
        );
        assert_eq!(scene.memory.furnace_pos, None, ":356");
    }

    /// `:348-351` : tout ce qui manque est déjà dans les fours — rien à faire.
    #[test]
    fn everything_already_in_the_furnaces_stops_the_fill() {
        let mut scene = cooking();
        scene.job = crate::ai_work_02::facts::TradeJob::chef(SmelterFacts {
            furnaces: vec![Furnace::new(Pos::new(0, 64, 0))
                .with_smeltable(Slot::new(key("minecraft:iron_ore"), 6))],
            allowed_fuel: vec![key("minecraft:coal")],
            ..SmelterFacts::default()
        });
        scene.craft.inputs = vec![crate::ai_build::crafter::Input::item(1, 20, 3)];
        scene.memory.furnace_pos = Some(Pos::new(0, 64, 0));
        scene.memory.max_crafting_count = 8;
        scene.memory.craft_counter = 2;
        assert_eq!(fill_up(&mut scene), FURNACE_IDLE, ":348-351");
        assert_eq!(scene.memory.transfer, None);
    }

    /// `:378-418` : avec de quoi remplir, le transfert porte le compte calculé, dans
    /// le slot d'entrée, et l'état rendu est `CRAFT`.
    #[test]
    fn filling_transfers_into_the_smeltable_slot() {
        let mut scene = cooking();
        scene.craft.inputs = vec![crate::ai_build::crafter::Input::item(1, 20, 8)];
        scene.memory.furnace_pos = Some(Pos::new(0, 64, 0));
        scene.memory.max_crafting_count = 8;
        scene.memory.craft_counter = 0;
        let next = fill_up(&mut scene);
        assert_eq!(next, WorkState::base(CraftState::Craft), ":423");
        let transfer = scene.memory.transfer.clone().expect("un transfert");
        assert_eq!(transfer.count, 8, ":378 — `min(pending, inInventory)`");
        assert_eq!(transfer.slot, crate::ai_work_02::facts::SMELTABLE_SLOT);
        assert_eq!(scene.memory.furnace_pos, None, ":381");
    }

    /// `:362-365` : plus de fours qui tournent que de fours utilisables — on ne
    /// remplit pas.
    #[test]
    fn too_many_burning_furnaces_stops_the_fill() {
        let mut scene = cooking();
        scene.job = crate::ai_work_02::facts::TradeJob::chef(SmelterFacts {
            furnaces: vec![
                Furnace::new(Pos::new(0, 64, 0))
                    .with_fuel(Slot::new(key("minecraft:coal"), 4))
                    .with_smeltable(Slot::new(key("minecraft:iron_ore"), 1))
                    .burning(),
                Furnace::new(Pos::new(1, 64, 0))
                    .with_fuel(Slot::new(key("minecraft:coal"), 4))
                    .with_smeltable(Slot::new(key("minecraft:iron_ore"), 1))
                    .burning(),
            ],
            allowed_fuel: vec![key("minecraft:coal")],
            skills: Skills::new(0, 0, 0),
            ..SmelterFacts::default()
        });
        scene.craft.inputs = vec![crate::ai_build::crafter::Input::item(1, 20, 8)];
        scene.memory.furnace_pos = Some(Pos::new(0, 64, 0));
        scene.memory.max_crafting_count = 8;
        assert_eq!(
            fill_up(&mut scene),
            FURNACE_IDLE,
            ":362 — 2 fours allumés pour 1 utilisable"
        );
        assert_eq!(scene.memory.transfer, None);
    }

    /// `:392-409` : le partage entre fours, reste compris.
    #[test]
    fn the_split_gives_the_remainder_to_the_first_furnace() {
        assert_eq!(transfer_count(0, 2), 0);
        assert_eq!(transfer_count(5, 0), 0, ":393 — aucun four libre");
        assert_eq!(transfer_count(5, 1), 5, ":399-402");
        assert_eq!(transfer_count(10, 3), 4, "10 / 3 + 10 % 3 = 3 + 1");
        assert_eq!(transfer_count(9, 3), 3, ":407 — 3 + 0");
        assert_eq!(transfer_count(200, 2), STACKSIZE, ":395-398 — 2 × 64 dépassé");
        assert_eq!(transfer_count(128, 2), STACKSIZE, ":407 — `min(64 + 0, 64)`");
    }

    /// `:181-191` : pas de carburant sur soi, mais dans le bâtiment → aller en
    /// chercher.
    #[test]
    fn missing_fuel_on_hand_sends_the_citizen_to_the_chest() {
        let mut scene = cooking();
        scene.job = crate::ai_work_02::facts::TradeJob::chef(SmelterFacts {
            furnaces: vec![Furnace::new(Pos::new(0, 64, 0))],
            allowed_fuel: vec![key("minecraft:coal")],
            fuel_in_inventory: 0,
            fuel_in_building: 8,
            ..SmelterFacts::default()
        });
        scene.memory.furnace_pos = Some(Pos::new(0, 64, 0));
        let next = add_fuel(&mut scene);
        assert_eq!(next, WorkState::base(CraftState::GatheringRequiredMaterials));
        assert_eq!(
            scene.memory.needed,
            Some(Need {
                item: NeedItem::AnyFuel(vec![key("minecraft:coal")]),
                count: STACKSIZE,
            }),
            ":185"
        );
    }

    /// `:189-191` : ni sur soi, ni dans le bâtiment → renoncer et repartir.
    #[test]
    fn no_fuel_anywhere_resets_the_furnace_position() {
        let mut scene = cooking();
        scene.job = crate::ai_work_02::facts::TradeJob::chef(SmelterFacts {
            furnaces: vec![Furnace::new(Pos::new(0, 64, 0))],
            allowed_fuel: vec![key("minecraft:coal")],
            fuel_in_inventory: 0,
            fuel_in_building: 0,
            ..SmelterFacts::default()
        });
        scene.memory.furnace_pos = Some(Pos::new(0, 64, 0));
        assert_eq!(add_fuel(&mut scene), FURNACE_IDLE);
        assert_eq!(scene.memory.furnace_pos, None, ":189");
        assert_eq!(scene.memory.transfer, None);
    }

    /// `:203-215` : on ne remplit le carburant que dans un four chargé au slot vide.
    #[test]
    fn fuelling_writes_into_an_empty_fuel_slot_only() {
        let mut scene = cooking();
        scene.job = crate::ai_work_02::facts::TradeJob::chef(SmelterFacts {
            furnaces: vec![Furnace::new(Pos::new(0, 64, 0)).with_fuel(Slot::new(
                key("minecraft:coal"),
                1,
            ))],
            allowed_fuel: vec![key("minecraft:coal")],
            fuel_in_inventory: 8,
            walked_to_furnace: true,
            ..SmelterFacts::default()
        });
        scene.memory.furnace_pos = Some(Pos::new(0, 64, 0));
        assert_eq!(add_fuel(&mut scene), FURNACE_IDLE);
        assert_eq!(scene.memory.transfer, None, ":208 — le slot n'est pas vide");

        scene.job = crate::ai_work_02::facts::TradeJob::chef(SmelterFacts {
            furnaces: vec![Furnace::new(Pos::new(0, 64, 0))],
            allowed_fuel: vec![key("minecraft:coal")],
            fuel_in_inventory: 8,
            walked_to_furnace: true,
            ..SmelterFacts::default()
        });
        scene.memory.furnace_pos = Some(Pos::new(0, 64, 0));
        assert_eq!(add_fuel(&mut scene), FURNACE_IDLE);
        let transfer = scene.memory.transfer.clone().expect("un transfert de carburant");
        assert_eq!(transfer.slot, crate::ai_work_02::facts::FUEL_SLOT);
        assert_eq!(transfer.count, STACKSIZE, ":211");
    }

    /// `:193-196` : sans position de four, le ravitaillement renonce.
    #[test]
    fn fuelling_without_a_furnace_position_gives_up() {
        let mut scene = cooking();
        scene.memory.furnace_pos = None;
        assert_eq!(add_fuel(&mut scene), FURNACE_IDLE, ":193-196");
    }

    /// `:103-115` : la demande de carburant n'est levée que si les trois conditions
    /// manquent — sur soi, dans le bâtiment, déjà demandée.
    #[test]
    fn the_fuel_request_needs_all_three_gaps() {
        let mut scene = cooking();
        scene.job = crate::ai_work_02::facts::TradeJob::chef(SmelterFacts {
            allowed_fuel: vec![key("minecraft:coal")],
            fuel_in_inventory: 0,
            fuel_in_building: 0,
            open_fuel_request: false,
            furnaces: vec![Furnace::new(Pos::new(0, 64, 0))],
            ..SmelterFacts::default()
        });
        check_furnace_fuel(&mut scene, WorkState::FillUpFurnaces);
        assert!(scene.memory.fuel_request_made);

        let mut already = cooking();
        already.job = crate::ai_work_02::facts::TradeJob::chef(SmelterFacts {
            allowed_fuel: vec![key("minecraft:coal")],
            open_fuel_request: true,
            furnaces: vec![Furnace::new(Pos::new(0, 64, 0))],
            ..SmelterFacts::default()
        });
        check_furnace_fuel(&mut already, WorkState::FillUpFurnaces);
        assert!(!already.memory.fuel_request_made, ":109 — une demande est ouverte");

        let mut has_some = cooking();
        check_furnace_fuel(&mut has_some, WorkState::FillUpFurnaces);
        assert!(!has_some.memory.fuel_request_made, ":107 — le citoyen en a");
    }

    /// `:120-142` : l'accélération est le produit des ticks par four **allumé**, et
    /// elle s'accumule.
    #[test]
    fn the_acceleration_counts_ticks_per_lit_furnace() {
        let mut scene = cooking();
        scene.job = crate::ai_work_02::facts::TradeJob::chef(SmelterFacts {
            furnaces: vec![
                Furnace::new(Pos::new(0, 64, 0)).burning(),
                Furnace::new(Pos::new(1, 64, 0)).burning(),
                Furnace::new(Pos::new(2, 64, 0)),
            ],
            skills: Skills::new(0, 30, 0),
            ..SmelterFacts::default()
        });
        accelerate_furnaces(&mut scene, WorkState::FillUpFurnaces);
        assert_eq!(scene.memory.accelerated, 12, "6 ticks × 2 fours allumés");
        accelerate_furnaces(&mut scene, WorkState::FillUpFurnaces);
        assert_eq!(scene.memory.accelerated, 24, "l'accélération s'accumule");
    }

    /// `:75`, `:89-98` : un intrus trouvé par la passe globale devient `furnacePos` et
    /// change d'état ; sans intrus, la passe globale rend la main.
    #[test]
    fn leftovers_are_found_by_the_global_pass() {
        let mut scene = cooking();
        scene.job = crate::ai_work_02::facts::TradeJob::chef(SmelterFacts {
            furnaces: vec![Furnace::new(Pos::new(4, 64, 4))
                .with_smeltable(Slot::new(key("minecraft:sand"), 1))],
            allowed_fuel: vec![key("minecraft:coal")],
            ..SmelterFacts::default()
        });
        assert!(has_left_overs(&scene, WorkState::base(CraftState::Craft)));
        let next = take_left_overs(&mut scene, WorkState::base(CraftState::Craft));
        assert_eq!(next, Some(WorkState::RetrievingUnrelatedProduct));
        assert_eq!(scene.memory.furnace_pos, Some(Pos::new(4, 64, 4)));

        let mut clean = cooking();
        assert!(!has_left_overs(&clean, WorkState::base(CraftState::Craft)));
        assert_eq!(take_left_overs(&mut clean, WorkState::base(CraftState::Craft)), None);
    }

    /// Un concasseur n'a pas de fours : la table du four répond
    /// `NoFactsForTrade` au lieu de prétendre qu'il n'y a rien à faire.
    #[test]
    fn a_furnace_rule_on_a_crusher_names_the_mismatch() {
        let mut scene = Scene::crusher(CrusherFacts::default());
        scene.memory.recipe = Some(iron_recipe());
        let decision = execute_crafting_action(&mut scene);
        assert_eq!(decision.refusal, Some(TradeRefusal::NoFactsForTrade));
    }
}
