//! Le berger : une cible de plus, une surcharge de la décision, et une teinture.
//!
//! Java référence : `core/entity/ai/workers/production/herders/EntityAIWorkShepherd.java`
//! (188 lignes), fille d'`AbstractEntityAIHerder`. Il ajoute **une** cible —
//! `new AITarget(SHEPHERD_SHEAR, this::shearSheep, TICKS_SECOND)` (`:49-52`) —, surcharge
//! `decideWhatToDo` (`:73-87`), `getExtraToolsNeeded` (`:56-64`) et
//! `getButcheringAttackDamage` (`:90-93`).
//!
//! ## La surcharge, dans l'ordre des lignes
//!
//! ```text
//! result = super.decideWhatToDo()                       :75
//! sheep  = findShearableSheep()                         :77
//! si SHEARING && result == START_WORKING && sheep != null   :79
//!     → SHEPHERD_SHEAR                                  :81   (la main N'EST PAS vidée)
//! worker.setItemInHand(MAIN_HAND, EMPTY)                :84
//! → result                                              :86
//! ```
//!
//! **La main n'est vidée que dans le chemin qui rend la main.** `:84` est après le
//! `return` de `:81` : un berger qui part tondre garde ses cisailles en main d'un tick à
//! l'autre, un berger qui va abattre ne garde rien. C'est un détail que le port doit
//! garder, parce qu'il décide si `equipTool` a quelque chose à faire au tick suivant.
//!
//! **Le test `result == START_WORKING` est étroit.** Java ne tond que si la décision de
//! base n'a rien trouvé de mieux : un troupeau qui remplit les conditions d'accouplement
//! (`:238`) ou un tirage d'abattage qui passe (`:234`) passent devant la tonte. Un
//! réglage `SHEARING` actif ne suffit donc pas à tondre — il faut que la table de base
//! ait rendu `START_WORKING`, qui est son `return` final (`:248`).
//!
//! ## Ce que le Java décide et qu'on n'attend pas
//!
//! 1. **`if (worker.getMainHandItem() != null)` (`:125`) est toujours vrai.** En 1.20.1
//!    `getMainHandItem()` rend un `ItemStack`, jamais `null` — c'est `isEmpty()` qu'il
//!    faudrait tester. Le test est mort, le corps s'exécute toujours, et le port le dit
//!    au lieu de garder une branche qui n'existe pas.
//! 2. **Fortune est multiplié puis tronqué.** `enchantmentLevel *= Math.max(1.0,
//!    getPrimarySkillLevel() / 5.0)` (`:132-133`) : `int *= double` tronque vers zéro.
//!    À Fortune III et habileté 30, le facteur vaut 6.0 et la borne passe à 18.
//! 3. **La quantité de laine est `1 + nextInt(fortune + 1)`** (`:141`), donc **au moins
//!    une** et au plus `fortune + 1`.
//! 4. **La teinture passe pour `rand <= buildingLevel`** (`:180`), pas `<` : au niveau 1,
//!    deux tirages sur cent (`0` et `1`) teignent.
//! 5. **Les dégâts d'abattage sont surchargés** : `max(1.0, secondary / 10.0)`
//!    (`:90-93`), donc 1.0 avant 10 d'habileté secondaire et 10.0 à 100 — au lieu du
//!    `5.0` du parent.

use crate::ai::state::WorkerState;

use super::facts::{arrived, HerdAct, HerdScene, ShepherdFacts, WALK_TO_ANIMAL_RANGE};
use super::state::{Hand, HerdChoice, HerdState, TradeRefusal};
use super::Decision;

/// `HUNDRED_PERCENT_CHANCE` (`EntityAIWorkShepherd.java:39`) — la borne du tirage de
/// teinture, `nextInt(100)` (`:178`).
pub const HUNDRED_PERCENT_CHANCE: u32 = 100;

/// `XP_PER_ACTION` (`AbstractEntityAIHerder.java:103`) : une demi-expérience par action,
/// tonte comprise (`EntityAIWorkShepherd.java:155`).
pub fn experience_per_action() -> f64 {
    crate::ai_work_09::herder::XP_PER_ACTION
}

/// `getExtraToolsNeeded()` (`:56-64`) : la cisaille s'ajoute à la liste du parent quand
/// le réglage `SHEARING` est actif.
///
/// Le parent demande déjà une hache (`AbstractEntityAIHerder.java:176-181`) ; Java rend
/// donc une liste de taille 1 ou 2, et `prepareForHerding` (`:309-315`) exige **toutes**
/// les entrées : un berger qui tond avec une cisaille doit aussi porter une hache.
pub fn needs_shears(shearing: bool) -> bool {
    shearing
}

/// `getButcheringAttackDamage()` (`:90-93`) : `Math.max(1.0, getSecondarySkillLevel() /
/// 10.0)`.
pub fn attack_damage(secondary_skill: u32) -> f64 {
    let scaled = secondary_skill as f64 / 10.0;
    if scaled < 1.0 { 1.0 } else { scaled }
}

/// `findShearableSheep()` (`:99-103`) : une brebis ni tondue ni bébé, ou rien.
pub fn find_shearable_sheep(scene: &HerdScene) -> Option<u32> {
    scene.pen.shearable_sheep().map(|sheep| sheep.id)
}

/// `decideWhatToDo` (`:73-87`).
pub fn decide_what_to_do(scene: &mut HerdScene, _from: HerdState) -> Decision {
    let Some(facts) = scene.shepherd() else {
        return Decision::refuse(TradeRefusal::WrongTradeFacts);
    };
    // `:75` — `final IAIState result = super.decideWhatToDo()`.
    let Some(base) = scene.take_base_answer() else {
        return Decision::refuse(TradeRefusal::BaseDecisionMissing);
    };
    // `:77` — la brebis est cherchée **avant** le test, donc à chaque décision.
    let sheep = find_shearable_sheep(scene);
    // `:79-82` — trois conditions ; si elles tiennent, `:84` n'est pas atteint.
    if facts.shearing && base == HerdChoice::StartWorking && sheep.is_some() {
        return Decision::go(HerdState::ShepherdShear);
    }
    // `:84` — `worker.setItemInHand(MAIN_HAND, ItemStack.EMPTY)`.
    scene.memory.hand = Hand::Empty;
    // `:86`
    Decision::go(base.state())
}

/// `shearSheep()` (`:110-166`).
pub fn shear_sheep(scene: &mut HerdScene, _from: HerdState) -> Decision {
    let Some(facts) = scene.shepherd() else {
        return Decision::refuse(TradeRefusal::WrongTradeFacts);
    };
    // `:113-118` — `findShearableSheep() == null` rend `DECIDE`.
    let Some(sheep) = scene.pen.shearable_sheep() else {
        return Decision::go_after(
            HerdState::Base(WorkerState::Decide),
            TradeRefusal::NoShearableSheep,
        );
    };
    // `:120-123` — `equipTool(MAIN_HAND, shears)` faux rend `PREPARING`.
    if facts.shears_slot.is_none() {
        return Decision::go_after(
            HerdState::Base(WorkerState::Preparing),
            TradeRefusal::NoShears,
        );
    }
    // `CitizenItemUtils.setHeldItem(worker, hand, getToolSlot(toolType))`
    // (`AbstractEntityAIHerder.java:709`).
    scene.memory.hand = Hand::Shears;
    // `:127-130` — `walkingToAnimal(sheep)` rend `getState()`, c'est-à-dire
    // `SHEPHERD_SHEAR` : la marche consomme une cadence entière (vingt ticks).
    if !arrived(&scene.facts, sheep.at, WALK_TO_ANIMAL_RANGE) {
        return Decision::refuse(TradeRefusal::NotAtAnimal);
    }
    // `:132-133` — `enchantmentLevel *= Math.max(1.0, primary / 5.0)`, tronqué.
    let fortune = fortune_scaled(facts.shears_fortune, facts.primary_skill);
    // `:141` — `int qty = 1 + worker.getRandom().nextInt(enchantmentLevel + 1)`.
    let wool = wool_qty(fortune, scene.draws.wool);
    // `:151` — la teinture, qui peut changer la couleur de la bête après la tonte.
    let dyed = dye_sheep_chance(facts, scene.draws.dye_percent, scene.draws.dye_colour);
    // `:140`, `:143-147`, `:158-162` — `setSheared(true)` puis `qty` laines dans
    // l'inventaire. L'appelant applique les deux : c'est un [`HerdAct`].
    scene.memory.act = Some(HerdAct::Sheared {
        id: sheep.id,
        wool,
        dyed,
    });
    // `:153` — `CitizenItemUtils.damageItemInHand(worker, MAIN_HAND, 1)` : la cisaille
    // perd un point de durabilité, que l'inventaire du citoyen porte
    // (`facts.inventory`).
    // `:155` — `addExperience(XP_PER_ACTION)`.
    // `:156` — `incrementActionsDoneAndDecSaturation()`.
    scene.facts.inventory.actions_done += 1;
    // `:165` — `return DECIDE`.
    Decision::go(HerdState::Base(WorkerState::Decide))
}

/// La quantité de laine : `1 + nextInt(enchantmentLevel + 1)` (`:141`).
///
/// Un tirage au-dessus de la borne ne vient pas de `nextInt` : le port le ramène à la
/// borne haute plutôt que de rendre `1 + un tirage impossible`.
pub fn wool_qty(fortune: u32, draw: u32) -> u32 {
    1 + if draw > fortune { fortune } else { draw }
}

/// `enchantmentLevel` après `enchantmentLevel *= Math.max(1.0, (getPrimarySkillLevel() /
/// 5.0))` (`:132-133`).
///
/// C'est une multiplication en `double` affectée à un `int`, donc une troncature vers
/// zéro. Le facteur ne descend jamais sous `1.0` — un berger de niveau 0 garde le
/// nombre d'enchantements tel quel.
pub fn fortune_scaled(fortune: u32, primary_skill: u32) -> u32 {
    let factor = primary_skill as f64 / 5.0;
    let factor = if factor < 1.0 { 1.0 } else { factor };
    (fortune as f64 * factor) as u32
}

/// `dyeSheepChance(sheep)` (`:173-187`) : la couleur `DyeColor` tirée, quand la teinture
/// passe.
///
/// Deux conditions : le réglage `DYEING` (`:175`) et `rand <= chanceToDye` (`:180`), où
/// `chanceToDye` est le **niveau de la hutte**. Le tirage est donc comparé au niveau, pas
/// à `100 - niveau` : au niveau 5, six tirages sur cent teignent (`0` à `5`), et au
/// niveau 0 aucun — `nextInt(100)` ne rend jamais moins de zéro.
///
/// Pure à dessein : l'effet est écrit par l'appelant dans [`HerdAct::Sheared`], avec la
/// tonte du même tick.
pub fn dye_sheep_chance(facts: ShepherdFacts, dye_percent: u32, dye_colour: u32) -> Option<u32> {
    // `:175` — `building != null && building.getSetting(DYEING).getValue()`.
    if !facts.dyeing {
        return None;
    }
    // `:178-180`
    if dye_percent > facts.building_level {
        return None;
    }
    // `:182-184` — `DyeColor.values()`, seize couleurs, et `nextInt(colors.length)`.
    Some(dye_colour % DYE_COLOURS)
}

/// `DyeColor.values().length` (`:182`) : seize couleurs en 1.20.1.
pub const DYE_COLOURS: u32 = 16;
