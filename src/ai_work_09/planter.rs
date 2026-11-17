//! Le planteur : choisir un champ, y marcher, demander au module quoi faire, le faire,
//! et rentrer quand il manque quelque chose.
//!
//! Java référence : `core/entity/ai/workers/production/agriculture/EntityAIWorkPlanter.java`
//! — le `registerTargets` du constructeur (`:78-84`), `prepare` (`:88-95`),
//! `pickField` (`:102-142`), `moveToField` (`:159-173`), `decideFieldWork`
//! (`:180-199`), `workField` (`:206-256`), `returnToBuilding` (`:263-276`),
//! `handlePlantingAction` (`:301-327`), `handleBonemealAction` (`:334-355`),
//! `handleMiningAction` (`:362-385`), `checkIfItemsUnavailable` (`:393-405`),
//! `decide` (`:416-425`), `getActionsDoneUntilDumping` (`:445-459`), `hasWorkToDo`
//! (`:468-471`) et `canGoIdle` (`:474-482`).
//!
//! ## La table que ce fichier ajoute
//!
//! | # | État | Règle | Cadence | Java |
//! | ---: | --- | --- | ---: | --- |
//! | 1 | `PREPARING` | `prepare` | 5 | `:79` |
//! | 2 | `PLANTATION_PICK_FIELD` | `pickField` | 20 | `:80` |
//! | 3 | `PLANTATION_MOVE_TO_FIELD` | `moveToField` | 20 | `:81` |
//! | 4 | `PLANTATION_DECIDE_FIELD_WORK` | `decideFieldWork` | 20 | `:82` |
//! | 5 | `PLANTATION_WORK_FIELD` | `workField` | 20 | `:83` |
//! | 6 | `PLANTATION_RETURN_TO_BUILDING` | `returnToBuilding` | 20 | `:84` |
//! | 7 | `IDLE` | `hasWorkToDo` → `START_WORKING` | 20 | `AbstractEntityAICrafting.java:164` |
//! | 8 | `START_WORKING` | `decide` (surchargé) | 5 | `AbstractEntityAICrafting.java:165`, `:416-425` |
//!
//! ## Cinq choses que le Java fait et qu'on n'attend pas
//!
//! 1. **`PREPARING` n'est atteint que par un détour.** `EntityAIWorkPlanter.decide`
//!    (`:418`) appelle `super.decide()`, qui rend `IDLE` par
//!    `getNextCraftingState()` quand le métier n'a pas de tâche d'artisanat
//!    (`AbstractEntityAICrafting.java:300-302`) — et un planteur n'en a jamais. C'est
//!    **ce** `IDLE` que la surcharge de `:420-423` transforme en `PREPARING`. La
//!    branche `IDLE` de `super.decide` (`:269-271`) est morte pour lui, parce que
//!    `hasWorkToDo` est surchargé en `true` (`:468-471`).
//! 2. **Le seuil de vidage dépend de l'état *et* du champ.** `getActionsDoneUntilDumping`
//!    (`:445-459`) rend la limite du module quand l'état est `DECIDE_FIELD_WORK` ou
//!    `WORK_FIELD` et qu'un champ existe, et `ACTIONS_UNTIL_DUMP` (32) sinon — donc le
//!    même planteur vide son inventaire tous les 32 coups hors du champ et tous les
//!    `getActionLimit()` coups dessus.
//! 3. **`NONE` et `CLEAR` ne comptent pas.** `ActionToPerform.increasesActionCount`
//!    (`IPlantationModule.java:176-180`) est faux pour ces deux-là : dégager une
//!    position obstruée ne consomme pas le quota du champ, donc un champ dont toutes
//!    les positions sont obstruées ne se rend jamais par le quota.
//! 4. **`FIELD` implique `POSITION`.** `shouldResetWorkingPosition` (`:310-313`) est
//!    vrai pour `POSITION` **et** pour `FIELD`, et `workField` teste les deux à la
//!    suite : un `FIELD` remet la position *puis* rend le champ.
//! 5. **`getCurrentExtension()` est lu avant `getExtensionToWorkOn()`** (`:123-124`),
//!    et c'est cette comparaison-là qui remet `currentFieldActionCount` à zéro. Un
//!    module qui rend le même champ deux fois de suite ne remet donc rien à zéro.
//!
//! ## Ce qui n'est pas ici
//!
//! Aucun appel au monde : marcher, poser un bloc, miner, appliquer du bonemeal et
//! récolter sont des [`super::facts::PlanterFacts`] que l'appelant remplit. Le module
//! de plantation lui-même (`decideFieldWork`, `getNextWorkingPosition`, les cinq
//! `getActionLimit`) n'est pas rejoué : sa **réponse** est lue
//! ([`super::facts::FieldWork`]).
//!
//! La durée d'un minage n'est pas réécrite ici : `workField` tourne à `TICKS_20`
//! (`:83`) mais un `CLEAR` ou un `HARVEST` prend le temps que
//! `crate::ai_build::work_time::mining_ticks` calcule — voir [`mining_ticks`].

use crate::ai::facts::{Pos, WorldFacts};
use crate::ai::state::{Machine, WorkerState};

use super::facts::{Deliverable, FieldWork, PlanterFacts, PlantAction, WorkMemory};
use super::state::{WorkRefusal, WorkState};
use super::{work_action, Decision, TICKS_SECOND};

/// `EntityAIWorkPlanter.BONEMEAL_TO_KEEP` (`:51`) : combien de bonemeal il demande
/// quand il n'en a plus (`:343`).
pub const BONEMEAL_TO_KEEP: u32 = 16;

/// `XP_PER_HARVEST` (`:56`) : l'expérience d'une récolte.
pub const XP_PER_HARVEST: f64 = 1.0;

/// La cadence des cinq états de champ (`:80-84`, `TICKS_20` —
/// `CitizenConstants.java:104`).
pub const FIELD_DELAY: u32 = 20;

/// La cadence de `PREPARING` (`:79`, `STANDARD_DELAY` = 5 —
/// `AbstractEntityAIBasic.java:100`).
pub const PREPARE_DELAY: u32 = super::STANDARD_DELAY;

/// `CitizenConstants.ACTIONS_UNTIL_DUMP` (`:166`), la réponse de
/// `AbstractEntityAIBasic.getActionsDoneUntilDumping` (`:425-428`) que le planteur
/// utilise hors du champ (`EntityAIWorkPlanter.java:449`, `:455`).
pub const ACTIONS_UNTIL_DUMP: u32 = 32;

/// Les cinq limites d'action que les modules de plantation de 1.20.1 répondent.
///
/// `getActionLimit()` n'est **pas** une constante du port : la limite vient du module
/// du champ ([`super::facts::FieldFacts::action_limit`]). Ces cinq valeurs sont celles
/// des cinq modules livrés, citées pour qu'un test puisse les vérifier sans ouvrir le
/// Java — et pour qu'on voie qu'elles ne s'accordent pas.
pub const ACTION_LIMITS: [(&str, u32); 5] = [
    // `PercentageHarvestPlantModule.java:177-180` (canne à sucre, cactus).
    ("PercentageHarvestPlantModule", 5),
    // `BoneMealedPlantModule.java:169-172` (une seule action, le bonemeal est le
    // travail).
    ("BoneMealedPlantModule", 1),
    // `UpwardsGrowingPlantModule.java:211-214` (bambou).
    ("UpwardsGrowingPlantModule", 10),
    // `DownwardsGrowingPlantModule.java:211-214`.
    ("DownwardsGrowingPlantModule", 10),
    // `TreeSidePlantModule.java:144-147`.
    ("TreeSidePlantModule", 5),
];

/// `ActionHandlerResult` (`:484-489`) : les trois réponses d'un handler d'action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three results Java declares
enum HandlerResult {
    /// `FINISHED` (`:486`) : l'action est faite, on passe au compte et au reset.
    Finished,
    /// `BUSY` (`:487`) : l'action est commencée, on reste sur `WORK_FIELD`.
    Busy,
    /// `NEEDS_ITEM` (`:488`) : il manque quelque chose, on rentre à la hutte. Le refus
    /// dit **quoi** manque, ce que Java perd dans le `return` de `workField`.
    NeedsItem(WorkRefusal),
}

/// `actionsUntilDumping` pour un planteur, c'est-à-dire `getActionsDoneUntilDumping`
/// (`:445-459`).
///
/// **Deux appelants, deux portées.** `decide` l'appelle (`AbstractEntityAICrafting.java:
/// 284`) alors que la machine est en `START_WORKING`, donc la première branche
/// (`:447-450`) rend toujours `ACTIONS_UNTIL_DUMP` = 32 : la limite du module n'y est
/// jamais lue. La branche du champ (`:458`) n'est atteinte que depuis
/// `AbstractEntityAIBasic.inventoryNeedsDump` (`:413`), qui lit `getState()` dans
/// l'état courant — `PLANTATION_WORK_FIELD` ou `PLANTATION_DECIDE_FIELD_WORK`. Le port
/// garde la fonction telle quelle, avec son paramètre d'état, pour que ce fait reste
/// visible au lieu d'être lissé.
pub fn actions_until_dumping(trade: &PlanterFacts, state: WorkState) -> u32 {
    // :447-450 — hors des deux états de champ, c'est la constante de la classe de base.
    if state != WorkState::PlantationDecideFieldWork && state != WorkState::PlantationWorkField {
        return ACTIONS_UNTIL_DUMP;
    }
    // :452-456 — un champ null rend la constante aussi.
    match trade.field.as_ref() {
        // :458
        Some(field) => field.action_limit,
        None => ACTIONS_UNTIL_DUMP,
    }
}

/// `prepare` (`:88-95`).
pub fn prepare(
    _facts: &mut WorldFacts,
    _trade: &PlanterFacts,
    memory: &mut WorkMemory,
    _state: WorkState,
) -> Decision {
    // :90-94 — une réponse de module encore en main : on reprend le travail.
    if memory.planter.active_result.is_some() {
        return Decision::go(WorkState::PlantationWorkField);
    }
    Decision::go(WorkState::PlantationPickField)
}

/// `pickField` (`:102-142`).
pub fn pick_field(
    _facts: &mut WorldFacts,
    trade: &PlanterFacts,
    memory: &mut WorkMemory,
    _state: WorkState,
) -> Decision {
    // :105-109 — `building == null || building.getBuildingLevel() < 1`.
    if trade.building_level < 1 {
        return Decision::go_after(WorkState::Base(WorkerState::Idle), WorkRefusal::HutNotBuilt);
    }
    // :112-120 — `module.hasNoExtensions()` : l'interaction `NO_FREE_FIELDS` part et le
    // statut du citoyen passe à `STUCK`.
    if !trade.has_extensions {
        return Decision::go_after(WorkState::Base(WorkerState::Idle), WorkRefusal::NoFreeField);
    }
    // :123-124 puis :141 — `fieldToWork == null` rend `IDLE`.
    let Some(field) = trade.field.as_ref() else {
        return Decision::go_after(WorkState::Base(WorkerState::Idle), WorkRefusal::NoFieldToWork);
    };
    // :131-134 — un champ différent de celui d'avant remet le compteur du champ à
    // zéro. La comparaison est faite sur le champ d'**avant** la sélection.
    if trade.previous_field != Some(field.position) {
        memory.planter.current_field_action_count = 0;
    }
    // :136-138
    Decision::go(WorkState::PlantationMoveToField)
}

/// `moveToField` (`:159-173`).
pub fn move_to_field(
    _facts: &mut WorldFacts,
    trade: &PlanterFacts,
    _memory: &mut WorkMemory,
    _state: WorkState,
) -> Decision {
    // :161-165 — `getCurrentField() == null` : il n'y a plus de champ à rejoindre.
    if trade.field.is_none() {
        return Decision::go(WorkState::PlantationPickField);
    }
    // :167-170 — `!walkToSafePos(field.getPosition())` : il marche encore.
    if !trade.at_field {
        return Decision::refuse(WorkRefusal::NotAtField);
    }
    // :172
    Decision::go(WorkState::PlantationDecideFieldWork)
}

/// `decideFieldWork` (`:180-199`).
pub fn decide_field_work(
    _facts: &mut WorldFacts,
    trade: &PlanterFacts,
    memory: &mut WorkMemory,
    _state: WorkState,
) -> Decision {
    // :182-186
    let Some(field) = trade.field.as_ref() else {
        return Decision::go(WorkState::PlantationPickField);
    };
    // :189-194 — `getNextWorkingPosition(world) == null` : le champ est fini.
    if field.work_position.is_none() {
        memory.planter.reset_active_field(); // :192
        return Decision::go_after(
            WorkState::Base(WorkerState::Idle),
            WorkRefusal::NoWorkPosition,
        );
    }
    // :196-197 — `decideFieldWork(world, position).build(module, position)`. Java ne
    // teste pas la réponse avant de la construire ; le port nomme l'absence au lieu de
    // laisser `workField` lever une `NullPointerException` à `:208`.
    let Some(decided) = trade.decided else {
        return Decision::go_after(
            WorkState::Base(WorkerState::Idle),
            WorkRefusal::ModuleAnsweredNothing,
        );
    };
    memory.planter.active_result = Some(decided);
    // :198
    Decision::go(WorkState::PlantationWorkField)
}

/// `workField` (`:206-256`).
pub fn work_field(
    facts: &mut WorldFacts,
    trade: &PlanterFacts,
    memory: &mut WorkMemory,
    _state: WorkState,
) -> Decision {
    // :208 — `activeModuleResult.getModule()`. `prepare` (`:90-94`) garantit qu'il
    // existe ; un appelant qui pose `WORK_FIELD` à la main ne le garantit pas.
    let Some(result) = memory.planter.active_result else {
        return Decision::go_after(
            WorkState::PlantationPickField,
            WorkRefusal::NoActiveResult,
        );
    };
    // :209-212 — `!walkToSafePos(module.getPositionToWalkTo(world, actionPosition))` :
    // il marche encore vers la position de l'action, et l'état ne change pas.
    if result.action_position.is_some() && !trade.at_action {
        return Decision::refuse(WorkRefusal::NotAtField);
    }
    // :214-221 — le `switch` sur l'action.
    let handler = match result.action {
        PlantAction::None => HandlerResult::Finished, // :216
        PlantAction::Plant => handle_planting(trade, result, memory), // :217
        PlantAction::Bonemeal => handle_bonemeal(trade, result, memory), // :218
        PlantAction::Harvest => handle_mining(trade, result),         // :219
        PlantAction::Clear => handle_mining(trade, result),           // :220
    };
    match handler {
        // :223-248 — `FINISHED`
        HandlerResult::Finished => {
            // :225 `CitizenItemUtils.removeHeldItem(worker)` — l'appelant.
            // :227-232 — seules `PLANT`, `BONEMEAL` et `HARVEST` comptent.
            if result.action.increases_action_count() {
                memory.planter.current_field_action_count += 1; // :229
                // `:230` `incrementActionsDone()` écrit le compteur du **métier**, que
                // `InventoryFacts::actions_done` reflète. Le port l'écrit ici pour que
                // la règle de vidage du prochain tick le voie : c'est une écriture de
                // l'IA sur son propre compte, pas une observation du monde.
                facts.inventory.actions_done += 1;
            }
            // :234-238 — une position de plus, ou le champ entier.
            let mut next = WorkState::PlantationWorkField;
            if result.should_reset_working_position() {
                next = WorkState::PlantationDecideFieldWork;
            }
            // :239-244 — `shouldResetCurrentField() || currentFieldActionCount >=
            // getActionLimit()`.
            let limit = trade.field.as_ref().map(|field| field.action_limit).unwrap_or(0);
            if result.should_reset_current_field()
                || memory.planter.current_field_action_count >= limit
            {
                memory.planter.reset_active_field(); // :241
                next = WorkState::PlantationPickField; // :243
            }
            memory.planter.active_result = None; // :246
            Decision::go(next)
        }
        // :249-253 — `NEEDS_ITEM` : le résultat est jeté et il rentre.
        HandlerResult::NeedsItem(refusal) => {
            memory.planter.active_result = None; // :251
            Decision::go_after(WorkState::PlantationReturnToBuilding, refusal)
        }
        // :255
        HandlerResult::Busy => Decision::go(WorkState::PlantationWorkField),
    }
}

/// `returnToBuilding` (`:263-276`).
pub fn return_to_building(
    facts: &mut WorldFacts,
    _trade: &PlanterFacts,
    memory: &mut WorkMemory,
    _state: WorkState,
) -> Decision {
    // :265-268 — `!walkToBuilding()` : il marche encore.
    if !super::at_hut(facts) {
        return Decision::refuse(WorkRefusal::NotAtHut);
    }
    // :270-273 — `needsCurrently != null` : il va chercher la pile demandée.
    if memory.base.need.is_some() {
        return Decision::go(WorkState::Base(WorkerState::GatheringRequiredMaterials));
    }
    // :275
    Decision::go(WorkState::Base(WorkerState::Idle))
}

/// `decide` d'`AbstractEntityAICrafting` (`:268-291`) avec la surcharge du planteur
/// (`:416-425`).
pub fn decide(
    facts: &mut WorldFacts,
    trade: &PlanterFacts,
    _memory: &mut WorkMemory,
    state: WorkState,
) -> Decision {
    // `:269-271` — `!hasWorkToDo()` : **inatteignable** pour un planteur, qui surcharge
    // `hasWorkToDo` en `true` (`EntityAIWorkPlanter.java:468-471`). Le port garde la
    // ligne et dit qu'elle est morte ; la supprimer cacherait le fait.
    //
    // `:281-284` — `!walkToBuilding()`.
    if !super::at_hut(facts) {
        return Decision::go(WorkState::Base(WorkerState::StartWorking));
    }
    // `:286-290` — `job.getActionsDone() >= getActionsDoneUntilDumping()` : il attend de
    // vider avant de continuer, sans changer d'état.
    if facts.inventory.actions_done >= actions_until_dumping(trade, state) {
        return Decision::refuse(WorkRefusal::InventoryDumpDue);
    }
    // `:300-302` — `getNextCraftingState()` rend `IDLE` quand `job.getCurrentTask()`
    // est nul. Un planteur n'a pas de module d'artisanat alimenté, donc c'est toujours
    // le cas.
    //
    // `:420-423` — la surcharge : cet `IDLE` devient `PREPARING`.
    Decision::go(WorkState::Base(WorkerState::Preparing))
}

/// `handlePlantingAction` (`:301-327`).
fn handle_planting(
    trade: &PlanterFacts,
    result: FieldWork,
    memory: &mut WorkMemory,
) -> HandlerResult {
    let _ = result;
    // :303 — `!Objects.isNull(activeModuleResult.getActionPosition())`.
    if result.action_position.is_none() {
        return HandlerResult::Finished; // :326
    }
    // :307-311 — `checkIfItemsUnavailable(new Stack(item, getPlantsToRequest(), 1))`.
    if check_if_items_unavailable(
        trade.plant_count,
        Deliverable {
            count: trade.plants_to_request,
            min_count: 1,
        },
        memory,
    ) {
        return HandlerResult::NeedsItem(WorkRefusal::NoPlantItem);
    }
    // :313-322 — `setMainHeldItem`, `getPlantingBlockState`, `setBlockAndUpdate`. La
    // pose réussie rend `FINISHED` et retire une graine.
    if trade.action_done {
        HandlerResult::Finished
    } else {
        HandlerResult::Busy // :324
    }
}

/// `handleBonemealAction` (`:334-355`).
fn handle_bonemeal(
    trade: &PlanterFacts,
    result: FieldWork,
    memory: &mut WorkMemory,
) -> HandlerResult {
    // :336 — `!Objects.isNull(activeModuleResult.getActionPosition())`.
    if result.action_position.is_none() {
        return HandlerResult::Finished; // :354
    }
    // :340-347 — `new StackList(bonemeal, REQUEST_TYPE_FERTILIZER, BONEMEAL_TO_KEEP, 1)`.
    if check_if_items_unavailable(
        trade.bonemeal_count,
        Deliverable {
            count: BONEMEAL_TO_KEEP,
            min_count: 1,
        },
        memory,
    ) {
        return HandlerResult::NeedsItem(WorkRefusal::NoBonemeal);
    }
    // :349-352 `applyBonemeal(...)`, puis :354 : le bonemeal ne peut pas échouer de
    // façon que Java teste, donc c'est toujours `FINISHED`.
    HandlerResult::Finished
}

/// `handleMiningAction` (`:362-385`), pour `HARVEST` (`:219`) et `CLEAR` (`:220`).
fn handle_mining(trade: &PlanterFacts, result: FieldWork) -> HandlerResult {
    // :364
    if result.action_position.is_none() {
        return HandlerResult::Finished; // :384
    }
    // :366-369 — `!holdEfficientTool(state, pos)` : il repose l'outil et il rentre. Un
    // outil cassé passe par ici.
    if !trade.has_tool {
        return HandlerResult::NeedsItem(WorkRefusal::ToolMissing);
    }
    // :371-382 — `mineBlock(pos)` : `FINISHED` s'il a fini, `BUSY` sinon.
    if trade.action_done {
        HandlerResult::Finished
    } else {
        HandlerResult::Busy
    }
}

/// `checkIfItemsUnavailable` (`:393-405`).
///
/// Rend `true` quand il manque la chose, et note dans la mémoire ce qu'il faut
/// demander : `currentDeliverable` (`:402`) et `needsCurrently` (`:403`) — ce dernier
/// est [`crate::ai::worker::Memory::need`], le même champ que la classe de base.
fn check_if_items_unavailable(
    count: u32,
    deliverable: Deliverable,
    memory: &mut WorkMemory,
) -> bool {
    // :395-400 — `invCount >= deliverable.getMinimumCount()` : il en a assez.
    if count >= deliverable.min_count {
        return false;
    }
    memory.planter.deliverable = Some(deliverable); // :402
    memory.base.need = Some(deliverable.count); // :403
    true
}

/// La durée d'un `CLEAR` ou d'un `HARVEST`, déléguée à
/// `crate::ai_build::work_time` : `handleMiningAction` passe par `mineBlock`
/// (`:371`), c'est-à-dire `AbstractEntityAIInteract.mineBlock` et sa formule de minage.
///
/// Le port ne réécrit pas la formule : deux copies d'une même durée finiraient par
/// diverger, et celle-là est déjà testée là-bas.
pub fn mining_ticks(
    time: crate::ai_build::work_time::MiningTime,
) -> Result<u32, crate::ai_build::work_time::Refusal> {
    crate::ai_build::work_time::mining_ticks(time)
}

/// `canGoIdle` (`:474-482`) : `false`, toujours.
///
/// Java écrit `if (getExtensionToWorkOn() == null) return !super.hasWorkToDo();`, et
/// `hasWorkToDo` est surchargé en `true` (`:468-471`) : les deux branches rendent donc
/// `false`. Le planteur ne rend jamais son citoyen à `IDLE` de lui-même.
pub fn can_go_idle() -> bool {
    false
}

work_action!(prepare_action, Planter, prepare);
work_action!(pick_field_action, Planter, pick_field);
work_action!(move_to_field_action, Planter, move_to_field);
work_action!(decide_field_work_action, Planter, decide_field_work);
work_action!(work_field_action, Planter, work_field);
work_action!(return_to_building_action, Planter, return_to_building);
work_action!(decide_action, Planter, decide);

/// Enregistre les six cibles de `EntityAIWorkPlanter.java:79-84` et les deux que le
/// parent d'artisanat fournit (`AbstractEntityAICrafting.java:164-165`).
pub fn register(machine: &mut Machine<WorkState, super::WorkScene>) {
    // `:164` — `new AITarget(IDLE, this::hasWorkToDo, () -> START_WORKING, TICKS_SECOND)`,
    // avec `hasWorkToDo` surchargé en `true` (`EntityAIWorkPlanter.java:468-471`).
    machine.add(crate::ai::state::Transition::target_when(
        WorkState::Base(WorkerState::Idle),
        "hasWorkToDo",
        "EntityAIWorkPlanter.java:468-471",
        TICKS_SECOND,
        has_work_to_do,
        crate::ai::state::Action::Fixed(Some(WorkState::Base(WorkerState::StartWorking))),
    ));
    // `:165` — `new AITarget(START_WORKING, this::decide, STANDARD_DELAY)`.
    super::add_target(
        machine,
        WorkState::Base(WorkerState::StartWorking),
        "decide",
        "EntityAIWorkPlanter.java:416-425",
        super::STANDARD_DELAY,
        decide_action,
    );
    // `:79` — `new AITarget(PREPARING, this::prepare, STANDARD_DELAY)`.
    super::add_target(
        machine,
        WorkState::Base(WorkerState::Preparing),
        "prepare",
        "EntityAIWorkPlanter.java:79",
        PREPARE_DELAY,
        prepare_action,
    );
    // `:80` — `new AITarget(PLANTATION_PICK_FIELD, this::pickField, TICKS_20)`.
    super::add_target(
        machine,
        WorkState::PlantationPickField,
        "pickField",
        "EntityAIWorkPlanter.java:80",
        FIELD_DELAY,
        pick_field_action,
    );
    // `:81` — `new AITarget(PLANTATION_MOVE_TO_FIELD, this::moveToField, TICKS_20)`.
    super::add_target(
        machine,
        WorkState::PlantationMoveToField,
        "moveToField",
        "EntityAIWorkPlanter.java:81",
        FIELD_DELAY,
        move_to_field_action,
    );
    // `:82` — `new AITarget(PLANTATION_DECIDE_FIELD_WORK, this::decideFieldWork,
    // TICKS_20)`.
    super::add_target(
        machine,
        WorkState::PlantationDecideFieldWork,
        "decideFieldWork",
        "EntityAIWorkPlanter.java:82",
        FIELD_DELAY,
        decide_field_work_action,
    );
    // `:83` — `new AITarget(PLANTATION_WORK_FIELD, this::workField, TICKS_20)`.
    super::add_target(
        machine,
        WorkState::PlantationWorkField,
        "workField",
        "EntityAIWorkPlanter.java:83",
        FIELD_DELAY,
        work_field_action,
    );
    // `:84` — `new AITarget(PLANTATION_RETURN_TO_BUILDING, this::returnToBuilding,
    // TICKS_20)`.
    super::add_target(
        machine,
        WorkState::PlantationReturnToBuilding,
        "returnToBuilding",
        "EntityAIWorkPlanter.java:84",
        FIELD_DELAY,
        return_to_building_action,
    );
}

/// `hasWorkToDo` (`EntityAIWorkPlanter.java:468-471`) : `true`, sans condition.
fn has_work_to_do(_scene: &super::WorkScene, _state: WorkState) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(action_limit: u32) -> super::super::facts::FieldFacts {
        super::super::facts::FieldFacts {
            position: Pos::new(0, 65, 0),
            action_limit,
            work_position: Some(Pos::new(1, 65, 1)),
        }
    }

    fn planter() -> PlanterFacts {
        PlanterFacts {
            building_level: 1,
            has_extensions: true,
            previous_field: None,
            field: Some(field(5)),
            decided: Some(FieldWork::plant(Pos::new(1, 65, 1), Pos::new(1, 66, 1))),
            at_field: true,
            at_action: true,
            has_tool: true,
            plant_count: 4,
            plants_to_request: 8,
            bonemeal_count: 0,
            action_done: true,
        }
    }

    fn world_at_hut() -> WorldFacts {
        let mut facts = WorldFacts::new();
        facts.building.position = Pos::new(0, 64, 0);
        facts.worker.at = Pos::new(0, 64, 0);
        facts
    }

    #[test]
    fn the_action_limit_is_the_modules_and_not_a_port_constant() {
        // `EntityAIWorkPlanter.java:458` rend `module.getActionLimit()`.
        let trade = planter();
        assert_eq!(actions_until_dumping(&trade, WorkState::PlantationWorkField), 5);
        assert_eq!(
            actions_until_dumping(&trade, WorkState::PlantationDecideFieldWork),
            5
        );
        // Hors des deux états de champ, c'est la constante de base (`:449`, `:455`).
        assert_eq!(actions_until_dumping(&trade, WorkState::Base(WorkerState::Idle)), 32);
        assert_eq!(actions_until_dumping(&trade, WorkState::PlantationPickField), 32);
        // Sans champ, la constante aussi (`:452-456`).
        let blind = PlanterFacts {
            field: None,
            ..planter()
        };
        assert_eq!(actions_until_dumping(&blind, WorkState::PlantationWorkField), 32);
        // Et la limite suit bien le module choisi, pas une valeur fixe.
        let bamboo = PlanterFacts {
            field: Some(field(10)),
            ..planter()
        };
        assert_eq!(actions_until_dumping(&bamboo, WorkState::PlantationWorkField), 10);
    }

    #[test]
    fn the_five_shipped_modules_do_not_agree_on_a_limit() {
        assert_eq!(ACTION_LIMITS.len(), 5);
        assert_eq!(ACTION_LIMITS[0].1, 5, "PercentageHarvestPlantModule.java:180");
        assert_eq!(ACTION_LIMITS[1].1, 1, "BoneMealedPlantModule.java:172");
        assert_eq!(ACTION_LIMITS[2].1, 10, "UpwardsGrowingPlantModule.java:213");
        assert_ne!(ACTION_LIMITS[1].1, ACTION_LIMITS[2].1);
    }

    #[test]
    fn a_hut_nobody_built_or_with_no_field_ends_in_idle_and_says_why() {
        // `:105-109` puis `:112-120`.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        let mut unbuilt = planter();
        unbuilt.building_level = 0;
        let decision = pick_field(&mut facts, &unbuilt, &mut memory, WorkState::PlantationPickField);
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::Idle)));
        assert_eq!(decision.refusal, Some(WorkRefusal::HutNotBuilt));
        // Le refus de champ passe avant celui de niveau : le Java teste le niveau
        // d'abord, et le port garde l'ordre.
        let mut empty = planter();
        empty.has_extensions = false;
        assert_eq!(
            pick_field(&mut facts, &empty, &mut memory, WorkState::PlantationPickField).refusal,
            Some(WorkRefusal::NoFreeField)
        );
        // `:141` — `getExtensionToWorkOn()` a rendu null.
        let mut no_field = planter();
        no_field.field = None;
        assert_eq!(
            pick_field(&mut facts, &no_field, &mut memory, WorkState::PlantationPickField).refusal,
            Some(WorkRefusal::NoFieldToWork)
        );
    }

    #[test]
    fn a_different_field_resets_the_action_count_and_the_same_one_does_not() {
        // `:131-134` : `lastField != fieldToWork`.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        memory.planter.current_field_action_count = 3;
        // Le champ d'avant était ailleurs : le compte repart à zéro.
        let moved = PlanterFacts {
            previous_field: Some(Pos::new(9, 65, 9)),
            ..planter()
        };
        assert_eq!(
            pick_field(&mut facts, &moved, &mut memory, WorkState::PlantationPickField).next,
            Some(WorkState::PlantationMoveToField)
        );
        assert_eq!(memory.planter.current_field_action_count, 0);
        // Le même champ : rien n'est remis à zéro.
        memory.planter.current_field_action_count = 3;
        let same = PlanterFacts {
            previous_field: Some(Pos::new(0, 65, 0)),
            ..planter()
        };
        pick_field(&mut facts, &same, &mut memory, WorkState::PlantationPickField);
        assert_eq!(memory.planter.current_field_action_count, 3);
    }

    #[test]
    fn walking_to_the_field_and_to_the_action_keep_the_state() {
        // `:167-170` puis `:209-212`.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        let walking = PlanterFacts {
            at_field: false,
            ..planter()
        };
        let decision = move_to_field(&mut facts, &walking, &mut memory, WorkState::PlantationMoveToField);
        assert_eq!(decision.next, None, "getState()");
        assert_eq!(decision.refusal, Some(WorkRefusal::NotAtField));
        assert_eq!(
            move_to_field(&mut facts, &planter(), &mut memory, WorkState::PlantationMoveToField).next,
            Some(WorkState::PlantationDecideFieldWork)
        );

        memory.planter.active_result = Some(FieldWork::plant(Pos::new(1, 65, 1), Pos::new(1, 66, 1)));
        let not_there = PlanterFacts {
            at_action: false,
            ..planter()
        };
        let decision = work_field(&mut facts, &not_there, &mut memory, WorkState::PlantationWorkField);
        assert_eq!(decision.next, None);
        assert_eq!(decision.refusal, Some(WorkRefusal::NotAtField));
    }

    #[test]
    fn a_finished_field_with_no_work_position_goes_idle_and_resets_the_count() {
        // `:189-194`.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        memory.planter.current_field_action_count = 2;
        let finished = PlanterFacts {
            field: Some(super::super::facts::FieldFacts {
                work_position: None,
                ..field(5)
            }),
            ..planter()
        };
        let decision = decide_field_work(
            &mut facts,
            &finished,
            &mut memory,
            WorkState::PlantationDecideFieldWork,
        );
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::Idle)));
        assert_eq!(decision.refusal, Some(WorkRefusal::NoWorkPosition));
        assert_eq!(memory.planter.current_field_action_count, 0, "`:292`");
    }

    #[test]
    fn a_planting_action_counts_once_and_asks_for_the_next_position() {
        // `:227-248` : `PLANT` augmente le compte (jusqu'ici 0), puis
        // `shouldResetWorkingPosition()` envoie sur `DECIDE_FIELD_WORK`.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        memory.planter.active_result = Some(FieldWork::plant(Pos::new(1, 65, 1), Pos::new(1, 66, 1)));
        let decision = work_field(&mut facts, &planter(), &mut memory, WorkState::PlantationWorkField);
        assert_eq!(decision.next, Some(WorkState::PlantationDecideFieldWork));
        assert_eq!(memory.planter.current_field_action_count, 1);
        assert_eq!(facts.inventory.actions_done, 1, "`:230`");
        assert!(memory.planter.active_result.is_none(), "`:246`");
    }

    #[test]
    fn reaching_the_action_limit_renders_the_field_even_without_a_reset() {
        // `:239-244` : `currentFieldActionCount >= planterModule.getActionLimit()`.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        memory.planter.current_field_action_count = 4;
        memory.planter.active_result = Some(FieldWork::plant(Pos::new(1, 65, 1), Pos::new(1, 66, 1)));
        let decision = work_field(&mut facts, &planter(), &mut memory, WorkState::PlantationWorkField);
        assert_eq!(decision.next, Some(WorkState::PlantationPickField), "5 == limite");
        assert_eq!(memory.planter.current_field_action_count, 0);
    }

    #[test]
    fn a_none_action_counts_nothing_and_renders_the_whole_field() {
        // `IPlantationModule.java:176` : `NONE(false)` ; et
        // `PlantationModuleResult.NONE` (`:214`) pose `pickNewField()`.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        memory.planter.active_result = Some(FieldWork::none(Pos::new(1, 65, 1)));
        let decision = work_field(&mut facts, &planter(), &mut memory, WorkState::PlantationWorkField);
        assert_eq!(decision.next, Some(WorkState::PlantationPickField));
        assert_eq!(memory.planter.current_field_action_count, 0, "NONE ne compte pas");
        assert_eq!(facts.inventory.actions_done, 0);
    }

    #[test]
    fn a_clear_action_counts_nothing_although_it_works() {
        // `IPlantationModule.java:180` : `CLEAR(false)`.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        memory.planter.active_result = Some(FieldWork {
            working_position: Pos::new(1, 65, 1),
            action: PlantAction::Clear,
            action_position: Some(Pos::new(1, 66, 1)),
            reset: super::super::facts::ResetState::None,
        });
        let decision = work_field(&mut facts, &planter(), &mut memory, WorkState::PlantationWorkField);
        assert_eq!(decision.next, Some(WorkState::PlantationWorkField));
        assert_eq!(memory.planter.current_field_action_count, 0);
        assert_eq!(facts.inventory.actions_done, 0);
    }

    #[test]
    fn a_missing_plant_asks_for_the_modules_count_and_sends_him_home() {
        // `:307-311` puis `:249-253`.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        memory.planter.active_result = Some(FieldWork::plant(Pos::new(1, 65, 1), Pos::new(1, 66, 1)));
        let empty = PlanterFacts {
            plant_count: 0,
            plants_to_request: 8,
            ..planter()
        };
        let decision = work_field(&mut facts, &empty, &mut memory, WorkState::PlantationWorkField);
        assert_eq!(decision.next, Some(WorkState::PlantationReturnToBuilding));
        assert_eq!(decision.refusal, Some(WorkRefusal::NoPlantItem));
        // `:402-403` : ce qui part en requête, et ce qu'il manque.
        assert_eq!(
            memory.planter.deliverable,
            Some(Deliverable {
                count: 8,
                min_count: 1
            })
        );
        assert_eq!(memory.base.need, Some(8));
        assert!(memory.planter.active_result.is_none(), "`:251`");
    }

    #[test]
    fn bonemeal_is_requested_sixteen_at_a_time_although_one_is_enough() {
        // `:341-344` : `new StackList(bonemeal, ..., BONEMEAL_TO_KEEP, 1)`.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        memory.planter.active_result = Some(FieldWork {
            working_position: Pos::new(1, 65, 1),
            action: PlantAction::Bonemeal,
            action_position: Some(Pos::new(1, 66, 1)),
            reset: super::super::facts::ResetState::Position,
        });
        let decision = work_field(&mut facts, &planter(), &mut memory, WorkState::PlantationWorkField);
        assert_eq!(decision.refusal, Some(WorkRefusal::NoBonemeal));
        assert_eq!(
            memory.planter.deliverable,
            Some(Deliverable {
                count: 16,
                min_count: 1
            })
        );
        // Avec un seul bonemeal, il en a assez pour agir.
        memory.planter.active_result = Some(FieldWork {
            working_position: Pos::new(1, 65, 1),
            action: PlantAction::Bonemeal,
            action_position: Some(Pos::new(1, 66, 1)),
            reset: super::super::facts::ResetState::Position,
        });
        let one = PlanterFacts {
            bonemeal_count: 1,
            ..planter()
        };
        let decision = work_field(&mut facts, &one, &mut memory, WorkState::PlantationWorkField);
        assert_eq!(decision.next, Some(WorkState::PlantationDecideFieldWork));
    }

    #[test]
    fn a_broken_tool_sends_the_planter_back_to_the_hut() {
        // `:366-369` — `!holdEfficientTool(...)` rend `NEEDS_ITEM`.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        memory.planter.active_result = Some(FieldWork {
            working_position: Pos::new(1, 65, 1),
            action: PlantAction::Harvest,
            action_position: Some(Pos::new(1, 66, 1)),
            reset: super::super::facts::ResetState::Position,
        });
        let blunt = PlanterFacts {
            has_tool: false,
            ..planter()
        };
        let decision = work_field(&mut facts, &blunt, &mut memory, WorkState::PlantationWorkField);
        assert_eq!(decision.next, Some(WorkState::PlantationReturnToBuilding));
        assert_eq!(decision.refusal, Some(WorkRefusal::ToolMissing));
    }

    #[test]
    fn an_unfinished_mine_stays_on_the_field() {
        // `:371-382` — `mineResult` faux rend `BUSY`, donc `WORK_FIELD`.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        memory.planter.active_result = Some(FieldWork {
            working_position: Pos::new(1, 65, 1),
            action: PlantAction::Harvest,
            action_position: Some(Pos::new(1, 66, 1)),
            reset: super::super::facts::ResetState::Position,
        });
        let slow = PlanterFacts {
            action_done: false,
            ..planter()
        };
        let decision = work_field(&mut facts, &slow, &mut memory, WorkState::PlantationWorkField);
        assert_eq!(decision.next, Some(WorkState::PlantationWorkField));
        assert_eq!(memory.planter.current_field_action_count, 0, "rien n'est compté");
    }

    #[test]
    fn the_dump_threshold_of_decide_is_always_the_base_constant() {
        // `AbstractEntityAICrafting.java:284` appelle `getActionsDoneUntilDumping()`
        // alors que la machine est en `START_WORKING` : la branche
        // `EntityAIWorkPlanter.java:447-450` rend donc 32, et la limite du module
        // (5 ici) n'est pas lue. Un planteur qui vide son inventaire le fait tous les
        // 32 coups, pas tous les 5.
        let mut facts = world_at_hut();
        facts.inventory.actions_done = 32;
        let mut memory = WorkMemory::default();
        let decision = decide(
            &mut facts,
            &planter(),
            &mut memory,
            WorkState::Base(WorkerState::StartWorking),
        );
        assert_eq!(decision.next, None, "il reste sur START_WORKING");
        assert_eq!(decision.refusal, Some(WorkRefusal::InventoryDumpDue));
        // Cinq actions — la limite du module — ne suffisent pas à le faire attendre.
        facts.inventory.actions_done = 5;
        let decision = decide(
            &mut facts,
            &planter(),
            &mut memory,
            WorkState::Base(WorkerState::StartWorking),
        );
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::Preparing)));
    }

    #[test]
    fn a_planter_with_no_distance_left_stays_on_start_working() {
        // `AbstractEntityAICrafting.java:281-284` — `!walkToBuilding()`.
        let mut facts = WorldFacts::new();
        facts.building.position = Pos::new(0, 64, 0);
        facts.worker.at = Pos::new(40, 64, 40);
        let mut memory = WorkMemory::default();
        let decision = decide(
            &mut facts,
            &planter(),
            &mut memory,
            WorkState::Base(WorkerState::StartWorking),
        );
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::StartWorking)));
    }

    #[test]
    fn a_planter_reaches_preparing_through_the_crafting_idle() {
        // `AbstractEntityAICrafting.java:300-302` puis `EntityAIWorkPlanter.java:420-423`.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        let decision = decide(
            &mut facts,
            &planter(),
            &mut memory,
            WorkState::Base(WorkerState::StartWorking),
        );
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::Preparing)));
    }

    #[test]
    fn returning_to_the_hut_goes_fetching_or_idles() {
        // `:265-276`.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        // Pas encore arrivé.
        facts.worker.at = Pos::new(40, 64, 40);
        let decision = return_to_building(&mut facts, &planter(), &mut memory, WorkState::PlantationReturnToBuilding);
        assert_eq!(decision.next, None);
        assert_eq!(decision.refusal, Some(WorkRefusal::NotAtHut));
        // Arrivé, une pile manquante : il va la chercher.
        facts.worker.at = Pos::new(0, 64, 0);
        memory.base.need = Some(8);
        assert_eq!(
            return_to_building(&mut facts, &planter(), &mut memory, WorkState::PlantationReturnToBuilding).next,
            Some(WorkState::Base(WorkerState::GatheringRequiredMaterials))
        );
        // Arrivé, plus rien à chercher.
        memory.base.need = None;
        assert_eq!(
            return_to_building(&mut facts, &planter(), &mut memory, WorkState::PlantationReturnToBuilding).next,
            Some(WorkState::Base(WorkerState::Idle))
        );
    }

    #[test]
    fn the_planter_never_hands_its_citizen_back_to_idle() {
        // `canGoIdle` (`:474-482`) avec `hasWorkToDo` à `true` (`:468-471`).
        assert!(!can_go_idle());
    }

    #[test]
    fn the_mining_duration_is_the_one_work_time_computes() {
        // `handleMiningAction` (`:371`) passe par `mineBlock`, donc par la formule
        // d'`AbstractEntityAIInteract.java:315-325` : le port délègue.
        let time = crate::ai_build::work_time::MiningTime::default();
        assert_eq!(mining_ticks(time), Ok(crate::ai_build::work_time::BLOCK_MINING_DELAY));
    }
}
