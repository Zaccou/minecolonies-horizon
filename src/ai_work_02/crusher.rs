//! Le concasseur : un quota par jour, un état `CRUSH`, et la même table d'atelier
//! dessous.
//!
//! Référence Java : `EntityAIWorkCrusher.java` — `TICK_DELAY` (`:33`),
//! `registerTargets` (`:49-52`), `decide` (`:62-88`), `hasWorkToDo` (`:90-94`),
//! `crush` (`:101-181`) et `craft` (`:188-249`), sur
//! `AbstractEntityAICrafting.java` dont il hérite tout le reste. Les deux nombres du
//! quota sont dans `BuildingCrusher.java:33` (`BUILDING_LEVEL_MULTIPLIER`) et `:43`
//! (`MAX_BUILDING_LEVEL`), et son défaut dans `BuildingModules.java:287`.
//!
//! ## Ce que le concasseur change, et rien d'autre
//!
//! | # | État | Règle | Rythme | Java |
//! | ---: | --- | --- | ---: | --- |
//! | 1 | `IDLE` | `startWorking` (aller simple, sans condition) | 1 | `:50` |
//! | 2 | `CRUSH` | `crush` | 40 | `:51` |
//!
//! Deux cibles, donc, mais **trois méthodes surchargées** : `decide` (`:62-88`),
//! `hasWorkToDo` (`:90-94`) et `craft` (`:188-249`). La machine étant commune aux
//! trois métiers, ces trois-là sont choisies dans [`super::state`] sur le métier —
//! c'est la méthode virtuelle de Java, dite autrement.
//!
//! ## Pourquoi `IDLE → START_WORKING` sans condition
//!
//! `new AITarget(IDLE, START_WORKING, 1)` (`:50`) n'a pas de prédicat : dans Java il
//! est enregistré **après** la cible de base `IDLE → hasWorkToDo → START_WORKING`
//! (`AbstractEntityAICrafting.java:164`), donc il ne tire que si celle-ci n'a pas
//! tiré — c'est-à-dire s'il n'y a pas de tâche. Et il tire alors *toujours* : c'est
//! `decide` qui décide ensuite, à l'état suivant, entre `CRUSH` et `IDLE`. Le port
//! garde exactement cet ordre d'enregistrement.
//!
//! **Le quota par défaut est zéro.** `new IntSetting(0)`
//! (`BuildingModules.java:287`) et `dailyQuantity < dailyLimit` (`:68`) : un
//! concasseur neuf n'a donc jamais de travail propre, et `decide` rend `IDLE`. C'est
//! le joueur qui pose le quota. Le port le dit au lieu de supposer un défaut
//! raisonnable.

use crate::ai::{Action, Machine, Transition};
use crate::ai_build::crafter::{self, CraftState};

use super::facts::{CrusherFacts, STACKSIZE};
use super::state::{self, Decision, Need, NeedItem, Scene, TradeRefusal, WorkState};

/// `EntityAIWorkCrusher.TICK_DELAY` (`:33`) : ce qu'un tick de `CRUSH` ajoute à la
/// progression.
pub const TICK_DELAY: u32 = 40;

/// `AbstractEntityAICrafting.MAX_LEVEL` (`:83`) : le plafond de la formule de
/// progression (`EntityAIWorkCrusher.java:127`).
pub const MAX_LEVEL: u32 = 50;

/// Le rythme de la cible `IDLE → START_WORKING` (`:50`).
pub const IDLE_RATE: u32 = 1;

/// L'expérience d'un coup réussi (`:151`).
pub const CRAFT_XP_GAIN: f64 = 0.1;

/// `getActionsDoneUntilDumping()` (`AbstractEntityAICrafting.java:724-727`) : un
/// crafter vide son inventaire **après chaque** réussite.
pub const ACTIONS_UNTIL_DUMPING: u32 = 1;

/// Où ce fichier prend ses règles.
pub const SOURCE: &str =
    "EntityAIWorkCrusher.java:33,:49-52,:62-88,:90-94,:101-181,:188-249 · BuildingCrusher.java:33,:43 · BuildingModules.java:287";

/// Les deux cibles du concasseur (`:49-52`).
pub fn register(machine: &mut Machine<WorkState, Scene>) {
    // :50 — `new AITarget(IDLE, START_WORKING, 1)`.
    machine.add(Transition::goto(
        WorkState::base(CraftState::Idle),
        WorkState::base(CraftState::StartWorking),
        "startWorking",
        "EntityAIWorkCrusher.java:50",
        IDLE_RATE,
    ));
    // :51 — `new AITarget(CRUSH, this::crush, TICK_DELAY)`.
    machine.add(Transition::target(
        WorkState::Crush,
        "crush",
        "EntityAIWorkCrusher.java:51",
        TICK_DELAY,
        Action::Run(crush_action),
    ));
}

/// `hasWorkToDo` (`:90-94`) : une tâche **ou** du quota restant.
///
/// `super.hasWorkToDo() || currentDailyQuantity < dailyLimit` — les deux moitiés sont
/// nécessaires : sans tâche, un bâtiment sous son quota fait quand même concasser.
pub fn has_work_to_do(scene: &Scene) -> bool {
    crafter::has_work_to_do(&scene.crafting_facts()) || !limit_reached(scene)
}

/// `getCurrentDailyQuantity() >= dailyLimit` (`:68`, `:121`).
pub fn limit_reached(scene: &Scene) -> bool {
    scene
        .job
        .crusher_facts()
        .is_none_or(CrusherFacts::at_limit)
}

/// `decide` (`:62-88`).
///
/// Les cinq sorties, dans l'ordre de Java :
///
/// 1. aucune tâche **et** du quota → `CRUSH` (`:68-71`) ;
/// 2. aucune tâche et plus de quota → `IDLE` (`:73`) ;
/// 3. pas encore arrivé au bâtiment → `START_WORKING`, où l'on est déjà (`:76-79`) ;
/// 4. une action faite depuis le dernier vidage → sur place (`:81-85`) ;
/// 5. sinon `getNextCraftingState()` (`:87`).
pub fn decide(scene: &mut Scene) -> Decision {
    let has_task = crafter::has_work_to_do(&scene.crafting_facts());
    if !has_task {
        // `:66` — `job.getTaskQueue().isEmpty() || job.getCurrentTask() == null`.
        if !limit_reached(scene) {
            return Decision::go(WorkState::Crush); // `:70`
        }
        return Decision::go(WorkState::base(CraftState::Idle))
            .because(TradeRefusal::DailyLimitReached); // `:73`
    }
    // `:76-79` — `!walkToBuilding()` : on reste dans `START_WORKING`.
    if !scene.craft.at_building {
        return Decision::go(WorkState::base(CraftState::StartWorking));
    }
    // `:81-85` — `getActionsDone() >= getActionsDoneUntilDumping()`, et ce nombre
    // vaut 1 (`AbstractEntityAICrafting.java:724-727`).
    if scene.memory.actions_done >= ACTIONS_UNTIL_DUMPING {
        return Decision::stay();
    }
    Decision::go(WorkState::base(crafter::next_crafting_state(
        &scene.crafting_facts(),
    ))) // `:87`
}

/// La progression qu'il faut atteindre : `MAX_LEVEL - min((secondarySkill / 2) + 1,
/// MAX_LEVEL)` (`:127`).
///
/// En `i64` parce qu'un niveau négatif donnerait un seuil au-dessus de `MAX_LEVEL` :
/// Java le calcule en `int`, le port ne le laisse pas devenir un compte de ticks
/// négatif.
pub fn progress_threshold(secondary_skill: i32, max_level: u32) -> i64 {
    let bonus = i64::from(secondary_skill) / 2 + 1;
    i64::from(max_level) - bonus.min(i64::from(max_level))
}

/// La traduction d'une réponse de `checkForItems` en état, pour les trois endroits où
/// `crush` la rend telle quelle (`:169`) ou l'utilise (`:131`, `:172`).
fn state_of(items: crafter::Items) -> WorkState {
    match items {
        crafter::Items::NoRecipe => WorkState::base(CraftState::StartWorking),
        crafter::Items::Gather { .. } => WorkState::base(CraftState::GatheringRequiredMaterials),
        crafter::Items::ReRead => WorkState::base(CraftState::GetRecipe),
        crafter::Items::Craft => WorkState::base(CraftState::Craft),
    }
}

/// `crush` (`:101-181`) : le broyage, avec son quota et sa progression.
///
/// **Le quota s'écrit, il ne s'observe pas.** Java fait
/// `crusherBuilding.setCurrentDailyQuantity(current + 1)` (`:135`) : le port le
/// compte dans [`super::state::TradeMemory::daily_increment`], que l'appelant applique
/// au bâtiment. C'est la même règle que pour un transfert d'inventaire : une règle
/// décide, l'hôte écrit.
pub fn crush(scene: &mut Scene, from: WorkState) -> Decision {
    // `:103-106` — `!walkToBuilding()`.
    if !scene.craft.at_building {
        return Decision::stay();
    }
    scene.memory.progress = scene.memory.progress.saturating_add(TICK_DELAY); // `:109`

    let Some(facts) = scene.job.crusher_facts() else {
        return Decision::stay().because(TradeRefusal::NoFactsForTrade);
    };
    let daily_limit = facts.daily_limit;
    let daily_quantity = facts.daily_quantity;
    let at_limit = facts.at_limit();
    let request_quantity = facts.request_quantity();
    let mode_recipe = facts.mode_recipe.clone();

    // `:116-119` — `if (currentRecipeStorage == null) currentRecipeStorage = recipeMode`.
    if scene.memory.recipe.is_none() {
        scene.memory.recipe = mode_recipe.clone();
    }
    // `:121-124` — `(getState() != CRAFT && quantity >= limit) || recipe == null`.
    if (from != WorkState::base(CraftState::Craft) && at_limit) || scene.memory.recipe.is_none() {
        let refusal = if scene.memory.recipe.is_none() {
            TradeRefusal::NoModeRecipe
        } else {
            TradeRefusal::DailyLimitReached
        };
        return Decision::go(WorkState::base(CraftState::StartWorking)).because(refusal);
    }

    // `:126` — `checkForItems(currentRecipeStorage)`.
    let check = crafter::check_for_items(&scene.crafting_facts());
    // `:127` — le seuil, sur la compétence **secondaire**.
    let threshold = progress_threshold(scene.job.skills().secondary, MAX_LEVEL);

    if i64::from(scene.memory.progress) > threshold {
        scene.memory.progress = 0; // `:129`
        match check {
            crafter::Items::Craft => {
                // `:133-140` — un item de plus au quota, et une action faite si le
                // quota est atteint.
                if from != WorkState::base(CraftState::Craft) {
                    scene.memory.daily_increment = scene.memory.daily_increment.saturating_add(1);
                    if daily_quantity.saturating_add(1) >= daily_limit {
                        scene.memory.actions_done = scene.memory.actions_done.saturating_add(1);
                    }
                }
                // `:141-144` — `currentRequest.addDelivery(primaryOutput)`.
                if scene.memory.request_open {
                    let produced = scene
                        .memory
                        .recipe
                        .as_ref()
                        .map_or(1, |recipe| recipe.output_count);
                    scene.memory.delivered = scene.memory.delivered.saturating_add(produced);
                }
                scene.memory.craft_counter = scene.memory.craft_counter.saturating_add(1); // `:147`
                scene.memory.experience += CRAFT_XP_GAIN; // `:151`
                // `:146-152` — `swing`, `fullfillRecipe`, saturation, statistiques :
                // des appels d'hôte, décrits par ce qui vient d'être écrit.
            }
            other if from != WorkState::base(CraftState::Craft) => {
                // `:154-166` — pas de quoi broyer : redemander la quantité du jour.
                let _ = other;
                scene.memory.recipe = mode_recipe; // `:156`
                let Some(quantity) = request_quantity else {
                    return Decision::go(WorkState::base(CraftState::StartWorking))
                        .because(TradeRefusal::RequestQuantityZero); // `:158-161`
                };
                let input = scene
                    .memory
                    .recipe
                    .as_ref()
                    .map(|recipe| recipe.input.clone());
                // `:162-164` — `checkIfRequestForItemExistOrCreateAsync(stack)`.
                scene.memory.needed = input.map(|key| Need {
                    item: NeedItem::Stack(key),
                    count: quantity.min(STACKSIZE),
                });
                return Decision::go(WorkState::base(CraftState::StartWorking));
            }
            other => {
                // `:169` — `return check`, qui n'est pas `CRAFT` ici.
                return Decision::go(state_of(other));
            }
        }
    }
    // `:172-179` — particules et son quand le broyage a réussi : des effets visuels,
    // sans effet d'état.
    Decision::stay() // `:180` — `return getState()`
}

/// `craft` (`:188-249`) : la surcharge du concasseur, qui appelle `crush` au milieu
/// d'une passe d'atelier.
///
/// La comparaison `check == getState()` de `:224` est [`Decision::stay`] ou un état
/// égal à l'état courant : c'est ainsi que le port distingue « `crush` n'a rien
/// changé » de « `crush` a renvoyé ailleurs ».
pub fn craft(scene: &mut Scene, from: WorkState) -> Decision {
    // `:191-194`
    if scene.memory.recipe.is_none() {
        return Decision::go(WorkState::base(CraftState::StartWorking));
    }
    // `:196-199`
    if !scene.memory.request_open && scene.craft.current_task {
        return Decision::go(WorkState::base(CraftState::GetRecipe));
    }
    // `:201-204`
    if !scene.craft.at_building {
        return Decision::stay();
    }
    scene.memory.progress = scene.memory.progress.saturating_add(1); // `:206`
    // `:213` — `currentRequest = job.getCurrentTask()`.
    scene.memory.request_open = scene.craft.current_task;

    // `:215-221`
    if scene.memory.request_cancelled {
        scene.memory.request_open = false;
        scene.memory.recipe = None;
        scene.memory.actions_done = scene.memory.actions_done.saturating_add(1);
        return Decision::go(WorkState::base(CraftState::StartWorking))
            .because(TradeRefusal::RequestCancelled);
    }

    // `:223` — `final IAIState check = crush()`.
    let inside = crush(scene, from);
    let stays = inside.next.is_none_or(|next| next == from);
    if stays {
        // `:226-240`
        if scene.memory.craft_counter >= scene.memory.max_crafting_count {
            scene.memory.actions_done = scene.memory.actions_done.saturating_add(1); // `:228`
            scene.memory.recipe = None; // `:229`
            state::reset_values(scene); // `:231`
            if scene.craft.inventory_needs_dump && scene.memory.request_open {
                // `:232-238` — les trois compteurs viennent d'être remis à zéro, donc
                // le test de Java ne garde que `currentRequest != null`.
                scene.memory.finished = Some(true); // `job.finishRequest(true)`
            }
        }
    } else {
        // `:241-246`
        scene.memory.request_open = false;
        scene.memory.finished = Some(false); // `job.finishRequest(false)`
        state::reset_values(scene);
    }
    Decision::stay() // `:248` — `return getState()`
}

/// La cible `CRUSH` de la table, sous la forme qu'une `Action::Run` attend.
fn crush_action(scene: &mut Scene, from: WorkState) -> Option<WorkState> {
    let decision = crush(scene, from);
    state::store(scene, &decision);
    decision.into_option(from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_build::crafter::Input;
    use crate::ai_build::materials::NeedKey;
    use crate::ai_work_02::facts::{Recipe, Skills};

    fn key(name: &str) -> NeedKey {
        NeedKey::plain(name)
    }

    fn recipe() -> Recipe {
        Recipe::crafted(key("minecraft:cobblestone"), key("minecraft:gravel"))
    }

    /// Un concasseur au travail : une recette, une requête, du quota, et une scène
    /// arrivée au bâtiment.
    fn crushing() -> Scene {
        let mut scene = Scene::crusher(CrusherFacts {
            daily_quantity: 0,
            daily_limit: 8,
            mode_recipe: Some(recipe()),
            building_level: 2,
            skills: Skills::new(0, 0, 0),
        });
        scene.memory.recipe = Some(recipe());
        scene.memory.request_open = true;
        scene.memory.max_crafting_count = 8;
        scene.craft.current_task = true;
        scene.craft.task_queue = 1;
        scene.craft.request_count = 8;
        scene.craft.output_count = 1;
        scene.craft.inputs = vec![Input::item(1, 0, 64)];
        scene
    }

    /// `:66-73` : sans tâche, le quota décide entre concasser et ne rien faire.
    #[test]
    fn the_daily_limit_decides_between_crushing_and_idling() {
        let mut free = Scene::crusher(CrusherFacts {
            daily_limit: 4,
            ..CrusherFacts::default()
        });
        assert_eq!(decide(&mut free).next, Some(WorkState::Crush), ":70");

        let mut spent = Scene::crusher(CrusherFacts {
            daily_quantity: 4,
            daily_limit: 4,
            ..CrusherFacts::default()
        });
        let decision = decide(&mut spent);
        assert_eq!(decision.next, Some(WorkState::base(CraftState::Idle)), ":73");
        assert_eq!(decision.refusal, Some(TradeRefusal::DailyLimitReached));
    }

    /// `:68` : le défaut du réglage étant zéro (`BuildingModules.java:287`), un
    /// concasseur neuf s'arrête tout de suite. Ce n'est pas un bug, c'est le réglage.
    #[test]
    fn a_default_crusher_has_no_quota_at_all() {
        let mut scene = Scene::crusher(CrusherFacts::default());
        assert_eq!(scene.job.crusher_facts().map(|f| f.daily_limit), Some(0));
        assert!(limit_reached(&scene));
        assert!(!has_work_to_do(&scene), ":93 — rien à faire");
        assert_eq!(
            decide(&mut scene).next,
            Some(WorkState::base(CraftState::Idle))
        );
    }

    /// `:91-94` : une tâche suffit à travailler même quota atteint, et `decide` suit
    /// alors `getNextCraftingState()` (`:87`).
    #[test]
    fn a_task_outranks_the_spent_quota() {
        let mut scene = crushing();
        scene.job = crate::ai_work_02::facts::TradeJob::crusher(CrusherFacts {
            daily_quantity: 8,
            daily_limit: 8,
            mode_recipe: Some(recipe()),
            building_level: 2,
            skills: Skills::new(0, 0, 0),
        });
        assert!(has_work_to_do(&scene), ":93 — `super.hasWorkToDo()` d'abord");
        assert_eq!(
            decide(&mut scene).next,
            Some(WorkState::base(CraftState::QueryItems)),
            ":87 → `:311-314` de l'atelier : une requête et une recette"
        );
    }

    /// `:76-79` : pas encore au bâtiment, `decide` renvoie là où il est.
    #[test]
    fn deciding_away_from_the_building_stays_in_place() {
        let mut scene = crushing();
        scene.craft.at_building = false;
        assert_eq!(
            decide(&mut scene).next,
            Some(WorkState::base(CraftState::StartWorking)),
            ":78"
        );
    }

    /// `:81-85` : une action faite depuis le dernier vidage gèle la décision.
    #[test]
    fn one_action_since_the_dump_waits_for_the_dump() {
        let mut scene = crushing();
        scene.memory.actions_done = 1;
        let decision = decide(&mut scene);
        assert_eq!(decision.next, None, ":84 — `return getState()`");
    }

    /// `:127` : le seuil est `50 − min(niveau/2 + 1, 50)`, sur la compétence
    /// secondaire (`Strength`, `BuildingModules.java:281`).
    #[test]
    fn the_progress_threshold_falls_with_the_secondary_skill() {
        assert_eq!(progress_threshold(0, MAX_LEVEL), 49);
        assert_eq!(progress_threshold(10, MAX_LEVEL), 44);
        assert_eq!(progress_threshold(11, MAX_LEVEL), 44, "11 / 2 + 1 = 6, 50 − 6");
        assert_eq!(progress_threshold(98, MAX_LEVEL), 0);
        assert_eq!(progress_threshold(200, MAX_LEVEL), 0, ":127 — plafonné par MAX_LEVEL");
        assert_eq!(progress_threshold(-10, MAX_LEVEL), 54, "un niveau cassé ne donne pas un seuil négatif");
    }

    /// `:103-106` : loin du bâtiment, `crush` ne touche à rien — même pas à la
    /// progression.
    #[test]
    fn crushing_away_from_the_building_does_nothing() {
        let mut scene = crushing();
        scene.craft.at_building = false;
        let decision = crush(&mut scene, WorkState::Crush);
        assert_eq!(decision.next, None);
        assert_eq!(scene.memory.progress, 0, ":109 n'est pas atteint");
    }

    /// `:109` : chaque tick de `CRUSH` ajoute `TICK_DELAY`, soit 40 — et un seul, tant
    /// que le seuil n'est pas franchi.
    #[test]
    fn each_crush_tick_advances_by_forty() {
        let mut scene = crushing();
        crush(&mut scene, WorkState::Crush);
        assert_eq!(scene.memory.progress, TICK_DELAY, ":109");
        assert_eq!(scene.memory.daily_increment, 0, "le seuil n'est pas atteint");
    }

    /// `:127-152` : au-delà du seuil, la progression retombe à zéro, le quota monte
    /// d'un, la livraison est comptée et l'item est produit.
    #[test]
    fn reaching_the_threshold_produces_one_item() {
        let mut scene = crushing();
        // Secondary 0 → seuil 49 ; 40 + 40 = 80 > 49 au second tick.
        crush(&mut scene, WorkState::Crush);
        let decision = crush(&mut scene, WorkState::Crush);
        assert_eq!(decision.next, None, ":180 — `return getState()`");
        assert_eq!(scene.memory.progress, 0, ":129");
        assert_eq!(scene.memory.daily_increment, 1, ":135");
        assert_eq!(scene.memory.delivered, 1, ":143");
        assert_eq!(scene.memory.craft_counter, 1, ":147");
        assert_eq!(scene.memory.experience, CRAFT_XP_GAIN, ":151");
    }

    /// `:133-139` : quand le quota est atteint par ce coup-ci, une action est
    /// comptée — c'est elle qui déclenchera le vidage.
    #[test]
    fn the_last_crushed_item_counts_an_action() {
        let mut scene = crushing();
        scene.job = crate::ai_work_02::facts::TradeJob::crusher(CrusherFacts {
            daily_quantity: 7,
            daily_limit: 8,
            mode_recipe: Some(recipe()),
            building_level: 2,
            skills: Skills::new(0, 0, 0),
        });
        scene.memory.progress = 49;
        let decision = crush(&mut scene, WorkState::Crush);
        assert_eq!(decision.next, None);
        assert_eq!(scene.memory.actions_done, 1, ":138");
        assert_eq!(scene.memory.daily_increment, 1);
    }

    /// `:121-124` : quota atteint et pas dans `CRAFT` → retour au travail, refus
    /// nommé.
    #[test]
    fn crush_stops_when_the_quota_is_spent() {
        let mut scene = crushing();
        scene.job = crate::ai_work_02::facts::TradeJob::crusher(CrusherFacts {
            daily_quantity: 8,
            daily_limit: 8,
            mode_recipe: Some(recipe()),
            building_level: 2,
            skills: Skills::new(0, 0, 0),
        });
        let decision = crush(&mut scene, WorkState::Crush);
        assert_eq!(decision.next, Some(WorkState::base(CraftState::StartWorking)), ":123");
        assert_eq!(decision.refusal, Some(TradeRefusal::DailyLimitReached));
    }

    /// `:116-119` : sans recette en cours, `crush` prend celle du mode ; si le mode
    /// n'en donne pas, c'est un refus nommé, pas un silence.
    #[test]
    fn the_mode_recipe_fills_an_empty_hand() {
        let mut scene = crushing();
        scene.memory.recipe = None;
        let decision = crush(&mut scene, WorkState::Crush);
        assert_eq!(scene.memory.recipe, Some(recipe()), ":118");
        assert_eq!(decision.next, None);

        let mut no_mode = Scene::crusher(CrusherFacts {
            daily_limit: 8,
            mode_recipe: None,
            ..CrusherFacts::default()
        });
        let decision = crush(&mut no_mode, WorkState::Crush);
        assert_eq!(decision.next, Some(WorkState::base(CraftState::StartWorking)));
        assert_eq!(decision.refusal, Some(TradeRefusal::NoModeRecipe), ":121-124");
    }

    /// `:154-166` : de quoi broyer manquant, hors `CRAFT` → demander
    /// `min((quota − fait) × 2, 64)` de l'entrée et repartir.
    #[test]
    fn a_shortage_outside_craft_asks_for_the_input() {
        let mut scene = crushing();
        scene.craft.inputs = vec![Input::item(1, 0, 0)];
        scene.memory.progress = 49;
        let decision = crush(&mut scene, WorkState::Crush);
        assert_eq!(decision.next, Some(WorkState::base(CraftState::StartWorking)), ":165");
        assert_eq!(
            scene.memory.needed,
            Some(Need {
                item: NeedItem::Stack(key("minecraft:cobblestone")),
                count: 16, // (8 − 0) × 2
            }),
            ":157, :162-164"
        );
        assert_eq!(scene.memory.recipe, Some(recipe()), ":156");
    }

    /// `:158-161` : le `requestQty <= 0` de Java est **inatteignable** tel quel — `:121`
    /// renvoie `START_WORKING` dès que le quota est atteint hors de `CRAFT`, donc à
    /// `:158` il reste toujours au moins un item. Le port garde le refus comme garde
    /// sur des faits que Java ne produit pas, et ce test dit pourquoi.
    #[test]
    fn the_zero_request_branch_is_preempted_by_the_limit_check() {
        let mut scene = crushing();
        scene.job = crate::ai_work_02::facts::TradeJob::crusher(CrusherFacts {
            daily_quantity: 8,
            daily_limit: 8,
            mode_recipe: Some(recipe()),
            building_level: 2,
            skills: Skills::new(0, 0, 0),
        });
        scene.craft.inputs = vec![Input::item(1, 0, 0)];
        scene.memory.progress = 49;
        let decision = crush(&mut scene, WorkState::Crush);
        assert_eq!(
            decision.refusal,
            Some(TradeRefusal::DailyLimitReached),
            ":121-124 passe avant :158"
        );
        assert_eq!(scene.memory.needed, None, "aucune demande n'est créée");
    }

    /// `:191-204` : les trois sorties de `craft` qui précèdent le broyage.
    #[test]
    fn the_crusher_craft_exits_come_in_java_order() {
        let mut no_recipe = crushing();
        no_recipe.memory.recipe = None;
        assert_eq!(
            craft(&mut no_recipe, WorkState::base(CraftState::Craft)).next,
            Some(WorkState::base(CraftState::StartWorking)),
            ":193"
        );

        let mut no_request = crushing();
        no_request.memory.request_open = false;
        assert_eq!(
            craft(&mut no_request, WorkState::base(CraftState::Craft)).next,
            Some(WorkState::base(CraftState::GetRecipe)),
            ":198"
        );

        let mut away = crushing();
        away.craft.at_building = false;
        assert_eq!(craft(&mut away, WorkState::base(CraftState::Craft)).next, None, ":203");
        assert_eq!(away.memory.progress, 0, ":206 n'est pas atteint");
    }

    /// `:206` puis `:109` : un tick de `CRAFT` avance d'**un** point, puis `crush` en
    /// ajoute quarante — 41 par tick, et non 40.
    #[test]
    fn the_crusher_craft_advances_by_one_then_crush_by_forty() {
        let mut scene = crushing();
        craft(&mut scene, WorkState::base(CraftState::Craft));
        assert_eq!(
            scene.memory.progress,
            1 + TICK_DELAY,
            ":206 (+1) puis :223 → :109 (+40)"
        );
    }

    /// `:224-240` : quand `crush` répond « sur place » et que le compteur a atteint
    /// son maximum, la tâche est terminée et la recette oubliée.
    #[test]
    fn a_finished_task_forgets_the_recipe() {
        let mut scene = crushing();
        scene.memory.max_crafting_count = 1;
        scene.memory.craft_counter = 1;
        scene.craft.inventory_needs_dump = true;
        let decision = craft(&mut scene, WorkState::base(CraftState::Craft));
        assert_eq!(decision.next, None, ":248");
        assert_eq!(scene.memory.recipe, None, ":229");
        assert_eq!(scene.memory.craft_counter, 0, ":231 — resetValues");
        assert_eq!(scene.memory.actions_done, 1, ":228");
        assert_eq!(scene.memory.finished, Some(true), ":236");
    }

    /// `:241-246` : quand `crush` renvoie ailleurs, la requête échoue et les compteurs
    /// sont remis, mais la recette reste.
    #[test]
    fn a_crush_that_moves_elsewhere_fails_the_request() {
        let mut scene = crushing();
        // Une entrée absente : `checkForItems` répond `RE_READ`, qui n'est ni
        // `CRAFT` ni l'état courant, donc `crush` renvoie `GET_RECIPE` (`:169`) et le
        // `check == getState()` de `:224` est faux.
        scene.craft.inputs = vec![Input::item(1, 0, 0)];
        scene.memory.progress = 49;
        let decision = craft(&mut scene, WorkState::base(CraftState::Craft));
        assert_eq!(decision.next, None, ":248");
        assert_eq!(scene.memory.finished, Some(false), ":244");
        assert_eq!(scene.memory.request_open, false, ":243");
        assert_eq!(scene.memory.craft_counter, 0, ":245 — resetValues");
        assert_eq!(scene.memory.recipe, Some(recipe()), "la recette survit à l'échec");
    }

    /// `:215-221` : une tâche annulée pendant le coup est abandonnée et comptée.
    #[test]
    fn a_cancelled_task_is_dropped() {
        let mut scene = crushing();
        scene.memory.request_cancelled = true;
        let decision = craft(&mut scene, WorkState::base(CraftState::Craft));
        assert_eq!(decision.next, Some(WorkState::base(CraftState::StartWorking)), ":220");
        assert_eq!(decision.refusal, Some(TradeRefusal::RequestCancelled));
        assert_eq!(scene.memory.recipe, None, ":219");
        assert_eq!(scene.memory.actions_done, 1, ":218");
    }

    /// `:50` : la cible `IDLE → START_WORKING` du concasseur n'a pas de condition —
    /// c'est la table de base qui décide d'abord, et son ordre d'enregistrement qui
    /// donne le résultat.
    #[test]
    fn the_unconditional_idle_target_is_registered_after_the_base_one() {
        let ai = crate::ai_work_02::WorkAi::new(crate::ai_work_02::Trade::Crusher);
        let idle_rules: Vec<&'static str> = ai
            .machine()
            .transitions()
            .iter()
            .filter(|transition| {
                transition.state == Some(WorkState::base(CraftState::Idle))
            })
            .map(|transition| transition.rule)
            .collect();
        assert_eq!(
            idle_rules,
            vec!["hasWorkToDo", "idle", "startWorking"],
            ":164, :165, puis EntityAIWorkCrusher.java:50"
        );
    }
}
