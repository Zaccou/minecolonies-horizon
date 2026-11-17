//! Le cow-boy : deux états de plus que l'éleveur, et une surcharge de la décision.
//!
//! Java référence : `core/entity/ai/workers/production/herders/EntityAIWorkCowboy.java`
//! — le `registerTargets` du constructeur (`:69-72`), `decideWhatToDo` (`:96-120`),
//! `getExtraItemsNeeded` (`:122-137`), `milkCows` (`:144-191`), `milkMooshrooms`
//! (`:198-249`), `getButcheringAttackDamage` (`:251-255`), `breedAnimals` (`:257-262`)
//! et `butcherAnimals` (`:264-269`) ; plus les deux compteurs du module
//! (`core/colony/buildings/workerbuildings/BuildingCowboy.java:254-267`).
//!
//! ## La table que ce fichier ajoute
//!
//! | # | État | Règle | Cadence | Java |
//! | ---: | --- | --- | ---: | --- |
//! | 1 | `COWBOY_MILK` | `milkCows` | 1 | `:70` |
//! | 2 | `COWBOY_STEW` | `milkMooshrooms` | 1 | `:71` |
//!
//! Et une cible **remplacée** : `DECIDE`. Le parent enregistre
//! `new AITarget(DECIDE, this::decideWhatToDo, DECIDING_DELAY)`
//! (`AbstractEntityAIHerder.java:143`), et `EntityAIWorkCowboy` surcharge la méthode
//! (`:96-120`) sans réenregistrer la cible : en Java, la référence de méthode
//! `this::decideWhatToDo` se résout dynamiquement sur la sous-classe. Le port ne peut
//! pas compter là-dessus — ses actions sont des pointeurs de fonction — donc le
//! cow-boy enregistre **sa** décision sur `DECIDE` ([`super::herder::register`] prend
//! la fonction en paramètre).
//!
//! ## Trois choses que le Java fait et qu'on n'attend pas
//!
//! 1. **Les deux refroidissements se décrémentent à chaque `DECIDE`, donc tous les 80
//!    ticks.** `MILK_COOL_DOWN` vaut 10 (`:56`) et `--milkCoolDown` (`:103`) est lu une
//!    fois par passage en `DECIDE` : une traite ratée bloque donc la traite pendant
//!    800 ticks de jeu, pas 10. Le commentaire de `:54` dit « min wait » et le chiffre
//!    ne veut pas dire des ticks.
//! 2. **Les deux refroidissements se décrémentent même quand la traite n'est pas
//!    tentée.** `:101-117` : les deux `if` sont indépendants, donc un `milkCoolDown`
//!    positif descend aussi les ticks où `canTryToMilk` est faux.
//! 3. **Un échec de `addItemStackToItemHandler` ne perd pas la traite du point de vue du
//!    compteur.** `:173-181` : le seau de lait est ajouté **dans** le `if`, mais
//!    `incrementActionsDone`, `decreaseSaturation`, la statistique et l'expérience sont
//!    **dehors** (`:183-187`), et `INVENTORY_FULL` est rendu dans les deux cas. Un
//!    inventaire plein consomme donc la tentative et la statistique sans rien produire.
//!
//! ## Ce qui n'est pas ici
//!
//! Aucun appel au monde : la traite elle-même (`equipItem`, `mobInteract`, le son) et
//! le transfert depuis la hutte (`checkAndTransferFromHut`) sont des faits et des
//! actions de l'appelant. `onMilked()` et `onStewed()` (`BuildingCowboy.java:271-281`)
//! ne sont pas portés non plus : ils ne font que monter `currentMilk` / `currentStew`,
//! dont la comparaison est déjà le fait [`super::facts::CowboyFacts::can_try_to_milk`].

use crate::ai::facts::{Pos, WorldFacts};
use crate::ai::state::{Machine, WorkerState};

use super::facts::{CowboyFacts, HerdFacts, WorkMemory};
use super::herder;
use super::state::{WorkRefusal, WorkState};
use super::{work_action, Decision};

/// `MILK_COOL_DOWN` (`:56`) : le refroidissement posé après une traite ratée (`:157`,
/// `:167`, `:211`, `:221`).
///
/// Mal nommé en Java : `:103` le décrémente de 1 par passage en `DECIDE`, qui a lieu
/// tous les `DECIDING_DELAY` = 80 ticks. Dix passages font 800 ticks.
pub const MILK_COOL_DOWN: i32 = 10;

/// La cadence des deux états de traite (`:70-71`) : un tick.
pub const COWBOY_DELAY: u32 = 1;

/// `BuildingModules.COWHERDER_SETTINGS` : `MILKING_AMOUNT` par défaut 1
/// (`BuildingModules.java:185`), `STEWING_AMOUNT` par défaut 1 (`:186`),
/// `MILKING_DAYS` par défaut 1 (`:187`).
pub const MILKING_AMOUNT_DEFAULT: i32 = 1;
/// La même valeur pour la soupe (`BuildingModules.java:186`).
pub const STEWING_AMOUNT_DEFAULT: i32 = 1;
/// Et pour les jours (`BuildingModules.java:187`), qui remettent les deux compteurs à
/// zéro au réveil (`BuildingCowboy.java:238-249`).
pub const MILKING_DAYS_DEFAULT: i32 = 1;

/// Le diviseur de `getButcheringAttackDamage` (`:254`) : `getPrimarySkillLevel() / 10.0`.
pub const MILK_ATTACK_DIVISOR: f64 = 10.0;

/// Le plancher de la même formule (`:254`) : `Math.max(1.0, ...)`.
pub const MILK_ATTACK_FLOOR: f64 = 1.0;

/// `getButcheringAttackDamage` (`:251-255`) : `max(1, primarySkill / 10)`.
///
/// L'éleveur de poulets ne surcharge pas la méthode et garde
/// [`herder::BUTCHERING_ATTACK_DAMAGE`] = 5 (`AbstractEntityAIHerder.java:848-851`).
pub fn butchering_attack_damage(primary_skill: u32) -> f64 {
    MILK_ATTACK_FLOOR.max(primary_skill as f64 / MILK_ATTACK_DIVISOR)
}

/// `decideWhatToDo` (`:96-120`) : la décision de l'éleveur, puis les deux traites.
pub fn decide_what_to_do(
    facts: &mut WorldFacts,
    trade: &CowboyFacts,
    memory: &mut WorkMemory,
    state: WorkState,
) -> Decision {
    // :99 — `super.decideWhatToDo()`, c'est-à-dire `AbstractEntityAIHerder.java:199-249`.
    let herd_result = herder::decide_what_to_do(facts, &trade.herd, memory, state);
    // :101-104 — `milkCoolDown` d'abord : s'il est positif, il ne tente **pas** la
    // traite, même si le module l'y autorise. `else if` : les deux ne peuvent pas
    // descendre dans le même passage.
    if memory.cowboy.milk_cool_down > 0 {
        memory.cowboy.milk_cool_down -= 1;
    } else if trade.can_try_to_milk
        && herd_result.next == Some(WorkState::Base(WorkerState::StartWorking))
    {
        return Decision::go(WorkState::CowboyMilk);
    }
    // :110-113 — la même chose pour la soupe de champignon, et indépendamment : un
    // refroidissement de traite positif n'empêche pas la soupe.
    if memory.cowboy.stew_cool_down > 0 {
        memory.cowboy.stew_cool_down -= 1;
    } else if trade.can_try_to_stew
        && herd_result.next == Some(WorkState::Base(WorkerState::StartWorking))
    {
        return Decision::go(WorkState::CowboyStew);
    }
    // :119 — `return result`.
    herd_result
}

/// `milkCows` (`:144-191`).
pub fn milk_cows(
    facts: &mut WorldFacts,
    trade: &CowboyFacts,
    memory: &mut WorkMemory,
    _state: WorkState,
) -> Decision {
    // :148 — `!hasItemInInventory(building.getMilkInputItem().getItem())`.
    if !trade.milk_input_in_inventory {
        // :150-154 — assez dans la hutte **et** `walkToBuilding()` : il transfère et
        // continue dans le même tick.
        if !(trade.milk_input_in_hut && super::at_hut(facts)) {
            // :155-159 — sinon, dix passages de refroidissement et `DECIDE`. La marche
            // vers la hutte est donc abandonnée dès le premier tick où elle n'est pas
            // finie : `walkToBuilding()` rend faux tant qu'il marche.
            memory.cowboy.milk_cool_down = MILK_COOL_DOWN;
            return Decision::go_after(WorkState::Base(WorkerState::Decide), WorkRefusal::NoMilkInput);
        }
        // :153 — `checkAndTransferFromHut(building.getMilkInputItem())` : l'appelant
        // déplace la pile, la décision n'a rien de plus à dire.
    }
    // :162-169 — une vache ou une chèvre adulte, pas une vache-champignon.
    let Some(animal) = trade.milkable_animal else {
        memory.cowboy.milk_cool_down = MILK_COOL_DOWN;
        return Decision::go_after(
            WorkState::Base(WorkerState::Decide),
            WorkRefusal::NoMilkableAnimal,
        );
    };
    // :171 — `equipItem(MAIN_HAND, milkInput) && !walkingToAnimal(animal)`.
    if trade.equipped && !herder::walking_to_pos(facts, animal) {
        // :173-181 — le seau est ajouté, `onMilked()`, le son. L'appelant ; et voir la
        // note du module : l'échec de l'ajout ne change pas la suite.
        // :183-187 — le compteur d'action et `INVENTORY_FULL`, qui vide l'inventaire
        // ensuite (`AbstractEntityAIBasic.afterDump`, `:1213-1222`).
        facts.inventory.actions_done += 1;
        return Decision::go(WorkState::Base(WorkerState::InventoryFull));
    }
    // :190 — `return getState()` : il marche encore, ou il n'a pas l'item en main.
    Decision::refuse(WorkRefusal::NotAtAnimal)
}

/// `milkMooshrooms` (`:198-249`).
pub fn milk_mooshrooms(
    facts: &mut WorldFacts,
    trade: &CowboyFacts,
    memory: &mut WorkMemory,
    _state: WorkState,
) -> Decision {
    // :202-214 — le bol, exactement comme le seau, avec son propre refroidissement.
    if !trade.bowl_in_inventory {
        if !(trade.bowl_in_hut && super::at_hut(facts)) {
            memory.cowboy.stew_cool_down = MILK_COOL_DOWN;
            return Decision::go_after(
                WorkState::Base(WorkerState::Decide),
                WorkRefusal::NoMilkInput,
            );
        }
    }
    // :216-223 — une vache-champignon non bébé.
    let Some(mooshroom) = trade.mooshroom else {
        memory.cowboy.stew_cool_down = MILK_COOL_DOWN;
        return Decision::go_after(
            WorkState::Base(WorkerState::Decide),
            WorkRefusal::NoMilkableAnimal,
        );
    };
    // :225 — `equipItem(MAIN_HAND, BOWL) && !walkingToAnimal(mooshroom)`.
    if trade.equipped && !herder::walking_to_pos(facts, mooshroom) {
        // :229-239 — `mobInteract` doit répondre `CONSUME` pour que la soupe existe ;
        // le fait [`CowboyFacts::stew_succeeded`] dit si c'est le cas. L'action est
        // comptée et `INVENTORY_FULL` rendu dans tous les cas (`:241-245`).
        facts.inventory.actions_done += 1;
        return Decision::go(WorkState::Base(WorkerState::InventoryFull));
    }
    // :248 — `return getState()`.
    Decision::refuse(WorkRefusal::NotAtAnimal)
}

work_action!(decide_action, Cowboy, decide_what_to_do);
work_action!(milk_cows_action, Cowboy, milk_cows);
work_action!(milk_mooshrooms_action, Cowboy, milk_mooshrooms);

/// Enregistre les deux cibles de `EntityAIWorkCowboy.java:70-71`.
///
/// La cible `DECIDE` est celle du parent, avec la décision du cow-boy : c'est
/// [`super::herder::register`] qui l'installe, et [`decide_action`] qui y est passée —
/// le « override par référence de méthode » que Java fait tout seul.
pub fn register(machine: &mut Machine<WorkState, super::WorkScene>) {
    // :70 — `new AITarget(COWBOY_MILK, this::milkCows, 1)`.
    super::add_target(
        machine,
        WorkState::CowboyMilk,
        "milkCows",
        "EntityAIWorkCowboy.java:70",
        COWBOY_DELAY,
        milk_cows_action,
    );
    // :71 — `new AITarget(COWBOY_STEW, this::milkMooshrooms, 1)`.
    super::add_target(
        machine,
        WorkState::CowboyStew,
        "milkMooshrooms",
        "EntityAIWorkCowboy.java:71",
        COWBOY_DELAY,
        milk_mooshrooms_action,
    );
}

/// `getActionsDoneUntilDumping` (`AbstractEntityAIHerder.java:152-156`) : le cow-boy
/// ne la surcharge pas, donc c'est dix.
pub fn actions_until_dumping() -> u32 {
    herder::ACTIONS_FOR_DUMP
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_work_09::facts::{HerdAnimal, HerderDraws};

    fn animal(id: u32, at: Pos) -> HerdAnimal {
        HerdAnimal {
            id,
            at,
            age: 0,
            baby: false,
            in_love: false,
            can_fall_in_love: true,
            alive: true,
            can_see_sky: true,
            can_mate: true,
        }
    }

    /// Un troupeau de deux animaux qui ne déclenche ni ramassage, ni abattage, ni
    /// nourrissage : `decideWhatToDo` du parent rend `START_WORKING`, ce qui est la
    /// condition des deux traites (`EntityAIWorkCowboy.java:105`, `:114`).
    fn cowboy() -> CowboyFacts {
        CowboyFacts {
            herd: HerdFacts {
                building_level: 1,
                breeding_setting: true,
                has_module: true,
                animals: vec![animal(1, Pos::new(0, 64, 0)), animal(2, Pos::new(1, 64, 0))],
                items_on_ground: 0,
                first_item: None,
                has_axe: true,
                // Un seul item d'accouplement : `AbstractEntityAIHerder.java:227-228`
                // demande `> 1`, donc ni accouplement ni nourrissage. Avec les trois
                // tirages hauts et deux adultes (trois ou moins ne s'abattent pas,
                // `:690-693`), `decideWhatToDo` du parent rend `START_WORKING` — la
                // condition des deux traites (`EntityAIWorkCowboy.java:105`, `:114`).
                breeding_item_count: 1,
                breeding_item_equipped: true,
                killed: false,
                chicken_draw: 0,
                game_time: 0,
                primary_skill: 20,
                secondary_skill: 20,
                main_hand_empty: false,
                draws: HerderDraws {
                    pickup: 0.9,
                    butcher: 0.9,
                    feed: 0.9,
                },
            },
            can_try_to_milk: true,
            can_try_to_stew: true,
            milk_input_in_inventory: true,
            milk_input_in_hut: true,
            bowl_in_inventory: true,
            bowl_in_hut: true,
            milkable_animal: Some(Pos::new(0, 64, 0)),
            mooshroom: Some(Pos::new(1, 64, 0)),
            equipped: true,
            stew_succeeded: true,
        }
    }

    fn world() -> WorldFacts {
        let mut facts = WorldFacts::new();
        facts.building.position = Pos::new(0, 64, 0);
        facts.worker.at = Pos::new(0, 64, 0);
        facts
    }

    #[test]
    fn the_two_cool_downs_and_the_cadence_are_java_s_numbers() {
        // `:56`, `:70-71`.
        assert_eq!(MILK_COOL_DOWN, 10);
        assert_eq!(COWBOY_DELAY, 1);
        // Le module de hutte qui les compare (`BuildingCowboy.java:257`, `:265`).
        assert_eq!(MILKING_AMOUNT_DEFAULT, 1, "BuildingModules.java:185");
        assert_eq!(STEWING_AMOUNT_DEFAULT, 1, "BuildingModules.java:186");
        assert_eq!(MILKING_DAYS_DEFAULT, 1, "BuildingModules.java:187");
        // Le cow-boy garde le quota de vidage de l'éleveur.
        assert_eq!(actions_until_dumping(), 10);
    }

    #[test]
    fn the_cowboy_hits_harder_than_the_chicken_herder() {
        // `:251-255` — `max(1, primarySkill / 10)`.
        assert_eq!(butchering_attack_damage(0), 1.0);
        assert_eq!(butchering_attack_damage(5), 1.0, "0,5 est relevé au plancher");
        assert_eq!(butchering_attack_damage(10), 1.0);
        assert_eq!(butchering_attack_damage(50), 5.0);
        assert_eq!(butchering_attack_damage(100), 10.0);
        // L'éleveur de poulets, lui, ne surcharge pas la méthode.
        assert_eq!(herder::BUTCHERING_ATTACK_DAMAGE, 5.0);
    }

    #[test]
    fn milking_wins_over_stewing_when_both_are_allowed() {
        // `:101-117` : la traite est testée d'abord, et `return` dessus.
        let mut facts = world();
        let mut memory = WorkMemory::default();
        let decision = decide_what_to_do(&mut facts, &cowboy(), &mut memory, WorkState::Base(WorkerState::Decide));
        assert_eq!(decision.next, Some(WorkState::CowboyMilk));
    }

    #[test]
    fn a_milk_cool_down_lets_the_stew_through_but_not_the_milk() {
        // `:101-104` puis `:110-113` : les deux `if` sont indépendants.
        let mut facts = world();
        let mut memory = WorkMemory::default();
        memory.cowboy.milk_cool_down = 1;
        let decision = decide_what_to_do(&mut facts, &cowboy(), &mut memory, WorkState::Base(WorkerState::Decide));
        assert_eq!(decision.next, Some(WorkState::CowboyStew));
        assert_eq!(memory.cowboy.milk_cool_down, 0, "`:103` décrémente");
        // Les deux en refroidissement : plus de traite du tout ce passage-ci.
        memory.cowboy.milk_cool_down = 1;
        memory.cowboy.stew_cool_down = 1;
        let decision = decide_what_to_do(&mut facts, &cowboy(), &mut memory, WorkState::Base(WorkerState::Decide));
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::StartWorking)), "`:119`");
        assert_eq!(memory.cowboy.milk_cool_down, 0);
        assert_eq!(memory.cowboy.stew_cool_down, 0);
    }

    #[test]
    fn a_full_milk_counter_stops_the_milking_without_touching_the_cool_down() {
        // `:105` — `canTryToMilk()` est `currentMilk < MILKING_AMOUNT`.
        let mut facts = world();
        let mut memory = WorkMemory::default();
        let full = CowboyFacts {
            can_try_to_milk: false,
            ..cowboy()
        };
        let decision = decide_what_to_do(&mut facts, &full, &mut memory, WorkState::Base(WorkerState::Decide));
        assert_eq!(decision.next, Some(WorkState::CowboyStew), "la soupe reste ouverte");
        assert_eq!(memory.cowboy.milk_cool_down, 0, "aucun refroidissement posé");
    }

    #[test]
    fn the_herder_result_must_be_start_working_for_a_milking_to_start() {
        // `:105` — `result.equals(START_WORKING)`.
        let mut facts = world();
        let mut memory = WorkMemory::default();
        // Un item par terre avec un tirage de ramassage bas : le parent rend
        // `HERDER_PICKUP`, donc la traite ne part pas et la décision du parent passe.
        let mut busy = cowboy();
        busy.herd.items_on_ground = 2;
        busy.herd.first_item = Some(Pos::new(9, 64, 9));
        busy.herd.draws.pickup = 0.01;
        let decision = decide_what_to_do(&mut facts, &busy, &mut memory, WorkState::Base(WorkerState::Decide));
        assert_eq!(decision.next, Some(WorkState::HerderPickup));
    }

    #[test]
    fn milking_without_a_bucket_anywhere_sets_ten_decides_of_cool_down() {
        // `:148-159`.
        let mut facts = world();
        let mut memory = WorkMemory::default();
        let empty = CowboyFacts {
            milk_input_in_inventory: false,
            milk_input_in_hut: false,
            ..cowboy()
        };
        let decision = milk_cows(&mut facts, &empty, &mut memory, WorkState::CowboyMilk);
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::Decide)));
        assert_eq!(decision.refusal, Some(WorkRefusal::NoMilkInput));
        assert_eq!(memory.cowboy.milk_cool_down, MILK_COOL_DOWN);
        assert_eq!(facts.inventory.actions_done, 0, "rien n'est compté");
    }

    #[test]
    fn a_bucket_waiting_in_the_hut_is_abandoned_while_he_is_still_walking() {
        // `:150-158` : `hasBuildingEnoughElseCount > 0 && walkToBuilding()` — et
        // `walkToBuilding()` rend faux tant qu'il marche, donc la branche `else` prend.
        let mut facts = world();
        facts.worker.at = Pos::new(40, 64, 40);
        let mut memory = WorkMemory::default();
        let in_hut = CowboyFacts {
            milk_input_in_inventory: false,
            milk_input_in_hut: true,
            ..cowboy()
        };
        let decision = milk_cows(&mut facts, &in_hut, &mut memory, WorkState::CowboyMilk);
        assert_eq!(decision.refusal, Some(WorkRefusal::NoMilkInput));
        assert_eq!(memory.cowboy.milk_cool_down, MILK_COOL_DOWN, "`:157`");
    }

    #[test]
    fn a_bucket_from_the_hut_lets_the_milking_go_on_in_the_same_tick() {
        // `:150-154` : arrivé à la hutte, il transfère et continue.
        let mut facts = world();
        let mut memory = WorkMemory::default();
        let from_hut = CowboyFacts {
            milk_input_in_inventory: false,
            milk_input_in_hut: true,
            ..cowboy()
        };
        let decision = milk_cows(&mut facts, &from_hut, &mut memory, WorkState::CowboyMilk);
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::InventoryFull)));
        assert_eq!(memory.cowboy.milk_cool_down, 0, "le transfert a suffi");
        assert_eq!(facts.inventory.actions_done, 1, "`:183`");
    }

    #[test]
    fn milking_with_no_adult_cow_freezes_the_trade_for_ten_decides() {
        // `:162-169`.
        let mut facts = world();
        let mut memory = WorkMemory::default();
        let cowless = CowboyFacts {
            milkable_animal: None,
            ..cowboy()
        };
        let decision = milk_cows(&mut facts, &cowless, &mut memory, WorkState::CowboyMilk);
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::Decide)));
        assert_eq!(decision.refusal, Some(WorkRefusal::NoMilkableAnimal));
        assert_eq!(memory.cowboy.milk_cool_down, MILK_COOL_DOWN);
    }

    #[test]
    fn a_reached_cow_is_milked_and_the_inventory_decides_the_rest() {
        // `:171-190`.
        let mut facts = world();
        let mut memory = WorkMemory::default();
        let decision = milk_cows(&mut facts, &cowboy(), &mut memory, WorkState::CowboyMilk);
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::InventoryFull)), "`:187`");
        assert_eq!(facts.inventory.actions_done, 1);
        // Trop loin : il marche et l'état ne change pas.
        facts.worker.at = Pos::new(30, 64, 30);
        let decision = milk_cows(&mut facts, &cowboy(), &mut memory, WorkState::CowboyMilk);
        assert_eq!(decision.next, None, "`:190` getState()");
        assert_eq!(decision.refusal, Some(WorkRefusal::NotAtAnimal));
        assert_eq!(facts.inventory.actions_done, 1, "la marche ne compte pas");
    }

    #[test]
    fn milking_without_the_bucket_in_hand_does_nothing_but_wait() {
        // `:171` — `equipItem(...)` doit avoir réussi.
        let mut facts = world();
        let mut memory = WorkMemory::default();
        let bare_handed = CowboyFacts {
            equipped: false,
            ..cowboy()
        };
        let decision = milk_cows(&mut facts, &bare_handed, &mut memory, WorkState::CowboyMilk);
        assert_eq!(decision.next, None);
        assert_eq!(decision.refusal, Some(WorkRefusal::NotAtAnimal));
    }

    #[test]
    fn stewing_uses_its_own_cool_down_and_its_own_item() {
        // `:202-248`.
        let mut facts = world();
        let mut memory = WorkMemory::default();
        let decision = milk_mooshrooms(&mut facts, &cowboy(), &mut memory, WorkState::CowboyStew);
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::InventoryFull)));
        assert_eq!(facts.inventory.actions_done, 1);
        // Sans bol : le refroidissement de la soupe, pas celui du lait.
        memory.cowboy.milk_cool_down = 0;
        let no_bowl = CowboyFacts {
            bowl_in_inventory: false,
            bowl_in_hut: false,
            ..cowboy()
        };
        let decision = milk_mooshrooms(&mut facts, &no_bowl, &mut memory, WorkState::CowboyStew);
        assert_eq!(decision.refusal, Some(WorkRefusal::NoMilkInput));
        assert_eq!(memory.cowboy.stew_cool_down, MILK_COOL_DOWN, "`:211`");
        assert_eq!(memory.cowboy.milk_cool_down, 0, "la traite n'est pas touchée");
        // Sans vache-champignon : même chose (`:221`).
        memory.cowboy.stew_cool_down = 0;
        let no_mooshroom = CowboyFacts {
            mooshroom: None,
            ..cowboy()
        };
        let decision = milk_mooshrooms(&mut facts, &no_mooshroom, &mut memory, WorkState::CowboyStew);
        assert_eq!(decision.refusal, Some(WorkRefusal::NoMilkableAnimal));
        assert_eq!(memory.cowboy.stew_cool_down, MILK_COOL_DOWN);
    }

    #[test]
    fn a_failed_mooshroom_interaction_still_counts_the_attempt() {
        // `:229-245` : le `CONSUME` ne garde que les effets, pas le compteur.
        let mut facts = world();
        let mut memory = WorkMemory::default();
        let no_soup = CowboyFacts {
            stew_succeeded: false,
            ..cowboy()
        };
        let decision = milk_mooshrooms(&mut facts, &no_soup, &mut memory, WorkState::CowboyStew);
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::InventoryFull)));
        assert_eq!(facts.inventory.actions_done, 1, "`:241`");
    }
}
