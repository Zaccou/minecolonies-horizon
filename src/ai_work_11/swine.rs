//! L'éleveur de porcs : la table d'élevage et les trois choses que ses 47 lignes
//! ajoutent.
//!
//! Java référence : `core/entity/ai/workers/production/herders/EntityAIWorkSwineHerder
//! .java` (47 lignes) et sa classe mère
//! `core/entity/ai/workers/production/herders/AbstractEntityAIHerder.java` (874),
//! plus `core/colony/buildings/workerbuildings/BuildingSwineHerder.java` (66) et
//! `EntityAIWorkRabbitHerder.java:25` (`RENDER_META_CARROT`, que l'éleveur de porcs
//! emprunte).
//!
//! ## Ce que la classe fait, mesuré
//!
//! Elle n'enregistre **aucune cible** : son constructeur est `super(job)` et rien
//! d'autre (`:20-23`). Elle surcharge exactement trois méthodes :
//!
//! | Ligne | Méthode | Ce que ça change |
//! | ---: | --- | --- |
//! | `:25-34` | `updateRenderMetaData` | l'état et une carotte en poche décident de l'icône |
//! | `:36-40` | `getExpectedBuildingClass` | sa hutte, donc aucun état |
//! | `:42-46` | `getButcheringAttackDamage` | `max(1.0, compétence primaire / 10.0)` — **pas** 5 |
//!
//! Sa machine à états **est** donc celle d'`AbstractEntityAIHerder` : les huit cibles
//! des lignes `139-148`, les quatre états `HERDER_BREED` / `HERDER_BUTCHER` /
//! `HERDER_PICKUP` / `HERDER_FEED` et le `DECIDE` de `:199-249`. Cette table est déjà
//! portée, avec ses 33 refus nommés et ses tests, dans `crate::ai_work_09::herder` : la
//! réécrire serait la cinquième copie d'une table qui n'a qu'un propriétaire. Ce fichier
//! **l'enregistre** — huit cibles, quatre retours — et chaque action appelle la fonction
//! de décision du module voisin. Un test (`tests::the_swine_table_answers_like_the_herder
//! _table`) vérifie que les deux machines produisent la même suite d'états sur les mêmes
//! faits.
//!
//! ## Le fait intéressant : le porc frappe plus fort que la poule, et sans tirage
//!
//! `EntityAIWorkChickenHerder` surcharge `butcherAnimal` avec un `nextInt(1 + (100 -
//! compétence) / 5) <= 1` (`EntityAIWorkChickenHerder.java:67`) : un coup qui ne porte
//! pas. `EntityAIWorkSwineHerder` ne le surcharge pas, donc elle hérite de
//! `AbstractEntityAIHerder.butcherAnimal` (`:805-838`), qui touche **toujours** — le seul
//! tirage du porc est donc l'effet `LOOTING` de la recherche (`:808`), qui ne change pas
//! les dégâts. Et ces dégâts valent `max(1.0, compétence / 10.0)` au lieu du
//! `BUTCHERING_ATTACK_DAMAGE = 5` de la classe mère (`:71`) : à compétence 0-9 le porc
//! frappe **un point**, cinq fois moins qu'une poule. C'est le Java, et c'est mesuré.

use crate::ai::facts::WorldFacts;
use crate::ai::state::{Machine, WorkerState};
use crate::ai_work_09::facts::{HerdFacts, WorkMemory};
use crate::ai_work_09::herder;
use crate::ai_work_09::state::WorkState as HerdState;
use crate::ai_work_09::Decision;

use super::facts::{Scene, SwineFacts, TradeJob};
use super::state::{Refusal, TradeState};
use super::{add_target, STANDARD_DELAY, TICKS_SECOND};

/// `AbstractEntityAIHerder.BUTCHERING_ATTACK_DAMAGE` (`:71`) : les dégâts de la classe
/// mère, que **l'éleveur de porcs ne garde pas** — voir [`butchering_attack_damage`].
pub const BUTCHERING_ATTACK_DAMAGE: f64 = 5.0;

/// `AbstractEntityAIInteract.RENDER_META_WORKING` (`AbstractEntityAIInteract.java:44`) :
/// la moitié « au travail » du métadonnée de rendu (`EntityAIWorkSwineHerder.java:28`).
pub const RENDER_META_WORKING: &str = "working";

/// `EntityAIWorkRabbitHerder.RENDER_META_CARROT` (`EntityAIWorkRabbitHerder.java:25`),
/// que l'éleveur de porcs emprunte nommément (`EntityAIWorkSwineHerder.java:31`).
pub const RENDER_META_CARROT: &str = "carrot";

/// `getPrimarySkillLevel()` est un `int` et `10.0` un `double` : la division est réelle
/// (`EntityAIWorkSwineHerder.java:45`), donc 15 de compétence valent 1,5 et non 1.
pub const SKILL_PER_DAMAGE_POINT: f64 = 10.0;

/// `Math.max(1.0, getPrimarySkillLevel() / 10.0)` (`EntityAIWorkSwineHerder.java:42-46`).
///
/// **Ce n'est pas [`BUTCHERING_ATTACK_DAMAGE`].** La classe mère rend 5 constant
/// (`AbstractEntityAIHerder.java:848-851`) ; le porc rend une valeur qui part de 1 et
/// n'atteint 5 qu'à 50 de compétence primaire. Le port garde les deux nombres, parce
/// qu'un futur éleveur (le mouton, la vache) retombe sur celui de la mère.
pub fn butchering_attack_damage(primary_skill_level: u32) -> f64 {
    let scaled = f64::from(primary_skill_level) / SKILL_PER_DAMAGE_POINT;
    if scaled < 1.0 {
        1.0
    } else {
        scaled
    }
}

/// Le coup de l'éleveur de porcs porte-t-il toujours ?
///
/// `EntityAIWorkSwineHerder` ne surcharge pas `butcherAnimal`
/// (`AbstractEntityAIHerder.java:805-838`) : la seule condition est
/// `animal != null && !walkingToAnimal(animal) && !mainHandItem.isEmpty()` (`:807`).
/// Contrairement à l'éleveur de poulets (`EntityAIWorkChickenHerder.java:67`), aucun
/// tirage ne décide du coup — donc oui, quand on l'appelle.
pub const HIT_IS_CERTAIN: bool = true;

/// `!ItemStackUtils.isEmpty(worker.getMainHandItem())`
/// (`AbstractEntityAIHerder.java:807`) : sans rien en main, `butcherAnimal` sort
/// immédiatement.
pub fn can_swing(facts: &SwineFacts) -> bool {
    facts.main_hand_full
}

/// `updateRenderMetaData` (`EntityAIWorkSwineHerder.java:25-34`).
///
/// `getState() == IDLE ? "" : RENDER_META_WORKING` (`:28`), puis la carotte ajoutée
/// sans séparateur (`:31`, une concaténation de chaînes : `"working"` + `"carrot"`).
pub fn render_meta(idle: bool, carrot_in_inventory: bool) -> String {
    let mut meta = String::new();
    if !idle {
        meta.push_str(RENDER_META_WORKING);
    }
    if carrot_in_inventory {
        meta.push_str(RENDER_META_CARROT);
    }
    meta
}

/// `BuildingSwineHerder.canEat` (`BuildingSwineHerder.java:59-66`) : la carotte n'est pas
/// de la nourriture pour un éleveur de porcs — c'est son item d'accouplement, et le
/// laisser la manger viderait l'enclos. Tout le reste passe par `super.canEat`.
pub fn can_eat(item_is_carrot: bool) -> bool {
    !item_is_carrot
}

/// `getActionsDoneUntilDumping` (`AbstractEntityAIHerder.java:152-156`) : dix actions
/// avant de vider le sac. La classe du porc ne la surcharge pas.
pub fn actions_until_dumping() -> u32 {
    herder::ACTIONS_FOR_DUMP
}

/// La fonction de décision d'`AbstractEntityAIHerder`, telle que le module voisin
/// l'expose : `(&mut WorldFacts, &HerdFacts, &mut WorkMemory, WorkState) -> Decision`.
type HerdDecision = fn(&mut WorldFacts, &HerdFacts, &mut WorkMemory, HerdState) -> Decision;

/// Appliquer une décision d'élevage à une scène de ce module.
///
/// **La mémoire est prêtée, pas copiée.** `crate::ai_work_09` tient `breedTimeOut`,
/// `animalsToBreed` et `fedRecently` dans son `HerdMemory`, et ce module en détient
/// exactement une instance ([`super::facts::TradeMemory::herd`]). Le `swap` la prête le
/// temps d'un appel, sans cloner les deux `Vec` à chaque tick.
fn herd_decision(
    scene: &mut Scene,
    state: TradeState,
    inner: HerdState,
    decide: HerdDecision,
) -> Option<TradeState> {
    let Scene {
        facts,
        job,
        memory,
        refusal,
        ..
    } = scene;
    let TradeJob::Swine(swine) = job else {
        // Des faits d'un autre métier : on tient la machine et on le dit, plutôt que de
        // lire un `HerdFacts` vide et de répondre « rien à faire ».
        *refusal = Some(Refusal::NoFactsForTrade);
        return Some(state);
    };
    let mut work = WorkMemory::default();
    core::mem::swap(&mut work.herd, &mut memory.herd);
    let decision = decide(facts, &swine.herd, &mut work, inner);
    core::mem::swap(&mut work.herd, &mut memory.herd);
    *refusal = decision.refusal.map(Refusal::Herd);
    decision.next.map(TradeState::Herd)
}

/// `IDLE → START_WORKING` (`AbstractEntityAIHerder.java:140`) : une constante, aucune
/// décision.
fn idle_to_work(_scene: &mut Scene, _state: TradeState) -> Option<TradeState> {
    Some(TradeState::Herd(HerdState::Base(WorkerState::StartWorking)))
}

/// `startWorkingAtOwnBuilding` (`AbstractEntityAIHerder.java:141`, corps `:288-295`).
fn start_working(scene: &mut Scene, state: TradeState) -> Option<TradeState> {
    herd_decision(
        scene,
        state,
        HerdState::Base(WorkerState::StartWorking),
        herder::start_working_at_own_building,
    )
}

/// `prepareForHerding` (`:142`, corps `:302-328`).
fn prepare_for_herding(scene: &mut Scene, state: TradeState) -> Option<TradeState> {
    herd_decision(
        scene,
        state,
        HerdState::Base(WorkerState::Preparing),
        herder::prepare_for_herding,
    )
}

/// `decideWhatToDo` (`:143`, corps `:199-249`). Le porc ne surcharge pas cette méthode :
/// c'est celle du parent, contrairement au cow-boy (`EntityAIWorkCowboy.java:96-120`).
fn decide_what_to_do(scene: &mut Scene, state: TradeState) -> Option<TradeState> {
    herd_decision(
        scene,
        state,
        HerdState::Base(WorkerState::Decide),
        herder::decide_what_to_do,
    )
}

/// `breedAnimals` (`:144`, corps `:419-484`).
fn breed_animals(scene: &mut Scene, state: TradeState) -> Option<TradeState> {
    herd_decision(scene, state, HerdState::HerderBreed, herder::breed_animals)
}

/// `butcherAnimals` (`:145`, corps `:335-390`).
fn butcher_animals(scene: &mut Scene, state: TradeState) -> Option<TradeState> {
    herd_decision(
        scene,
        state,
        HerdState::HerderButcher,
        herder::butcher_animals,
    )
}

/// `pickupItems` (`:146`, corps `:575-588`).
fn pickup_items(scene: &mut Scene, state: TradeState) -> Option<TradeState> {
    herd_decision(scene, state, HerdState::HerderPickup, herder::pickup_items)
}

/// `feedAnimal` (`:147`, corps `:517-568`).
fn feed_animal(scene: &mut Scene, state: TradeState) -> Option<TradeState> {
    herd_decision(scene, state, HerdState::HerderFeed, herder::feed_animal)
}

/// Enregistre les huit cibles d'`AbstractEntityAIHerder.java:140-147`, sur
/// [`TradeState::Herd`].
///
/// **Pourquoi les cibles sont redites et les règles non.** `Machine<S, F>` est
/// monomorphe en `S` : une machine ne peut pas contenir la machine du module voisin, qui
/// tique sur son propre `WorkState`. Ce qui est écrit ici est donc la *cible* — état,
/// nom, ligne, cadence — et rien d'autre : chaque action est un appel à la fonction de
/// décision de `crate::ai_work_09::herder`, qui reste le seul domicile de la règle. Les
/// huit cadences sont celles du Java : 1 (`:140`), 20 (`:141`, `:142`, `:146`, `:147`),
/// 80 (`:143`), 40 (`:144`), 20 (`:145`).
pub fn register(machine: &mut Machine<TradeState, Scene>) {
    // :140 — `new AITarget(IDLE, START_WORKING, 1)`.
    machine.add(crate::ai::Transition::target(
        TradeState::Herd(HerdState::Base(WorkerState::Idle)),
        "idle",
        "AbstractEntityAIHerder.java:140",
        1,
        crate::ai::Action::Run(idle_to_work),
    ));
    // :141 — `new AITarget(START_WORKING, this::startWorkingAtOwnBuilding, TICKS_SECOND)`.
    add_target(
        machine,
        TradeState::Herd(HerdState::Base(WorkerState::StartWorking)),
        "startWorkingAtOwnBuilding",
        "AbstractEntityAIHerder.java:141",
        TICKS_SECOND,
        start_working,
    );
    // :142 — `new AITarget(PREPARING, this::prepareForHerding, TICKS_SECOND)`.
    add_target(
        machine,
        TradeState::Herd(HerdState::Base(WorkerState::Preparing)),
        "prepareForHerding",
        "AbstractEntityAIHerder.java:142",
        TICKS_SECOND,
        prepare_for_herding,
    );
    // :143 — `new AITarget(DECIDE, this::decideWhatToDo, DECIDING_DELAY)`.
    add_target(
        machine,
        TradeState::Herd(HerdState::Base(WorkerState::Decide)),
        "decideWhatToDo",
        "AbstractEntityAIHerder.java:143",
        herder::DECIDING_DELAY,
        decide_what_to_do,
    );
    // :144 — `new AITarget(HERDER_BREED, this::breedAnimals, BREEDING_DELAY)`.
    add_target(
        machine,
        TradeState::Herd(HerdState::HerderBreed),
        "breedAnimals",
        "AbstractEntityAIHerder.java:144",
        herder::BREEDING_DELAY,
        breed_animals,
    );
    // :145 — `new AITarget(HERDER_BUTCHER, this::butcherAnimals, BUTCHER_DELAY)`.
    add_target(
        machine,
        TradeState::Herd(HerdState::HerderButcher),
        "butcherAnimals",
        "AbstractEntityAIHerder.java:145",
        herder::BUTCHER_DELAY,
        butcher_animals,
    );
    // :146 — `new AITarget(HERDER_PICKUP, this::pickupItems, TICKS_SECOND)`.
    add_target(
        machine,
        TradeState::Herd(HerdState::HerderPickup),
        "pickupItems",
        "AbstractEntityAIHerder.java:146",
        TICKS_SECOND,
        pickup_items,
    );
    // :147 — `new AITarget(HERDER_FEED, this::feedAnimal, TICKS_SECOND)`.
    add_target(
        machine,
        TradeState::Herd(HerdState::HerderFeed),
        "feedAnimal",
        "AbstractEntityAIHerder.java:147",
        TICKS_SECOND,
        feed_animal,
    );
    register_handover(machine);
}

/// Les quatre chemins de retour d'un état de base vers `IDLE`.
///
/// `crate::ai_work_09::register_handover` les porte déjà pour ses trois métiers ; ce
/// module ayant un type d'état différent, il doit les redire — quatre lignes, avec les
/// mêmes sources — sinon un `INVENTORY_FULL` serait un piège
/// (`BasicStateMachine.java:220-228`) au lieu d'un retour.
fn register_handover(machine: &mut Machine<TradeState, Scene>) {
    let idle = TradeState::Herd(HerdState::Base(WorkerState::Idle));
    // `afterDump` (`AbstractEntityAIBasic.java:1213-1222`) : un éleveur dont le sac est
    // plein (`:411-416`) revient de `INVENTORY_FULL` en `IDLE`.
    machine.add(crate::ai::Transition::goto(
        TradeState::Herd(HerdState::Base(WorkerState::InventoryFull)),
        idle,
        "afterDump",
        "AbstractEntityAIBasic.java:1213-1222",
        STANDARD_DELAY,
    ));
    // `waitForRequests` (`:238`, `:605-640`).
    machine.add(crate::ai::Transition::goto(
        TradeState::Herd(HerdState::Base(WorkerState::NeedsItem)),
        idle,
        "waitForRequests",
        "AbstractEntityAIBasic.java:238",
        STANDARD_DELAY,
    ));
    // `bePaused` (`:251`) : le joueur relance.
    machine.add(crate::ai::Transition::goto(
        TradeState::Herd(HerdState::Base(WorkerState::Paused)),
        idle,
        "bePaused",
        "AbstractEntityAIBasic.java:251",
        TICKS_SECOND,
    ));
    // `getNeededItem` (`:242`, `:323-326`).
    machine.add(crate::ai::Transition::goto(
        TradeState::Herd(HerdState::Base(WorkerState::GatheringRequiredMaterials)),
        idle,
        "getNeededItem",
        "AbstractEntityAIBasic.java:242",
        STANDARD_DELAY,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pig_hits_for_a_tenth_of_its_skill_and_never_below_one() {
        // `EntityAIWorkSwineHerder.java:45` : `Math.max(1.0, skill / 10.0)`.
        assert_eq!(butchering_attack_damage(0), 1.0);
        assert_eq!(butchering_attack_damage(9), 1.0, "0,9 est sous le plancher");
        assert_eq!(butchering_attack_damage(10), 1.0, "exactement 1,0");
        assert_eq!(
            butchering_attack_damage(15),
            1.5,
            "division réelle, pas entière"
        );
        assert_eq!(butchering_attack_damage(50), BUTCHERING_ATTACK_DAMAGE);
        assert_eq!(butchering_attack_damage(100), 10.0);
        // Et la constante de la classe mère reste 5 : les deux existent.
        assert_eq!(BUTCHERING_ATTACK_DAMAGE, 5.0);
    }

    #[test]
    fn the_render_meta_is_the_state_plus_an_unseparated_carrot() {
        // `EntityAIWorkSwineHerder.java:28-32`.
        assert_eq!(render_meta(true, false), "");
        assert_eq!(render_meta(true, true), RENDER_META_CARROT);
        assert_eq!(render_meta(false, false), RENDER_META_WORKING);
        assert_eq!(
            render_meta(false, true),
            "workingcarrot",
            "concaténation nue"
        );
    }

    #[test]
    fn a_pig_herder_does_not_eat_its_breeding_item() {
        // `BuildingSwineHerder.java:59-66`.
        assert!(!can_eat(true), "la carotte est exclue");
        assert!(can_eat(false), "le reste passe par super.canEat");
    }

    #[test]
    fn the_pig_swing_is_certain_and_needs_something_in_hand() {
        // `AbstractEntityAIHerder.java:807` et l'absence de surcharge de
        // `butcherAnimal` dans `EntityAIWorkSwineHerder`.
        assert!(HIT_IS_CERTAIN);
        let mut facts = SwineFacts::default();
        assert!(!can_swing(&facts), "les mains vides ne frappent pas");
        facts.main_hand_full = true;
        assert!(can_swing(&facts));
        // Le tirage de l'éleveur de poulets, lui, peut rater : c'est la différence que
        // ce test fixe (`EntityAIWorkChickenHerder.java:67`).
        assert!(!herder::chicken_hit_lands(0, 5));
    }

    #[test]
    fn the_pig_dumps_every_ten_actions_like_the_herder() {
        // `AbstractEntityAIHerder.java:152-156`, que la classe du porc ne surcharge pas.
        assert_eq!(actions_until_dumping(), 10);
        assert_eq!(actions_until_dumping(), herder::ACTIONS_FOR_DUMP);
    }
}
