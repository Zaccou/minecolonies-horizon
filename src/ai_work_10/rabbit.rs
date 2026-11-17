//! Le lapinier : soixante-neuf lignes, et une seule règle qui lui appartienne.
//!
//! Java référence : `core/entity/ai/workers/production/herders/EntityAIWorkRabbitHerder.java`
//! (69 lignes), fille d'`AbstractEntityAIHerder`. Son constructeur (`:32-35`) ne fait
//! que relayer `super(job)` : **il enregistre zéro cible**, donc sa machine à états
//! *est* celle du parent, huit cibles (`AbstractEntityAIHerder.java:139-148`). Il ne
//! surcharge pas non plus `decideWhatToDo` : la décision accoupler/abattre/nourrir/
//! ramasser est celle du parent (`:199-249`).
//!
//! Ce qui reste est une **frappe** et une **icône** :
//!
//! - `butcherAnimal` (`:54-68`) remplace le coup du parent (`:805-841`, qui touche
//!   toujours) par un coup qui touche une fois sur `1 + (100 - habileté) / 5` ;
//! - `updateRenderMetaData` (`:37-46`) ajoute la carotte au-dessus de la tête quand
//!   l'inventaire en porte.
//!
//! ## Ce que le Java décide et qu'on n'attend pas
//!
//! 1. **La frappe et l'usure sont dans le même `if`** (`:61-66`). Le parent use l'outil à
//!    chaque coup (`AbstractEntityAIHerder.java:797`) ; le lapinier n'use la hache que
//!    quand le tirage passe — donc un lapinier malhabile ne casse presque jamais sa
//!    hache.
//! 2. **La borne vaut 1 à l'habileté 100**, et `nextInt(1)` rend toujours `0`, ce qui
//!    passe le test `<= 1` (`:61`) : à 100 d'habileté, la frappe touche **toujours**.
//!    À 0, la borne vaut 21 et la frappe passe pour `0` ou `1`, soit 2 sur 21.
//! 3. **Le test de main vide est `!ItemStackUtils.isEmpty(...)`** (`:57`) : un lapinier
//!    sans outil ne frappe pas du tout, il ne frappe pas à mains nues.
//! 4. **Les dégâts ne sont pas surchargés** : `getButcheringAttackDamage()` reste celui
//!    du parent, `BUTCHERING_ATTACK_DAMAGE = 5` (`AbstractEntityAIHerder.java:71`,
//!    `:848-851`). Le berger, lui, le surcharge (`EntityAIWorkShepherd.java:90-93`).

use crate::ai::state::WorkerState;

use super::facts::{arrived, HerdAct, HerdScene, WALK_TO_ANIMAL_RANGE};
use super::state::{Hand, HerdState, TradeRefusal};
use super::Decision;

/// `RENDER_META_CARROT` (`EntityAIWorkRabbitHerder.java:25`).
pub const RENDER_META_CARROT: &str = "carrot";

/// `RENDER_META_WORKING` (`AbstractEntityAIInteract.java:44`), la moitié « au travail »
/// de l'icône (`EntityAIWorkRabbitHerder.java:40`).
pub const RENDER_META_WORKING: &str = "working";

/// Le diviseur de la borne du tirage : `(ONE_HUNDRED_PERCENT - getPrimarySkillLevel()) /
/// 5` (`:61`), division **entière**.
pub const SKILL_TO_CHANCE_DIVISOR: u32 = 5;

/// La borne du tirage de `butcherAnimal` : `1 + (ONE_HUNDRED_PERCENT - habileté) / 5`
/// (`:61`).
///
/// Elle vaut `1` à l'habileté `ONE_HUNDRED_PERCENT`, et `1 + 20 = 21` à zéro. Le
/// `saturating_sub` n'est pas décoratif : une habileté au-dessus de 100 (un effet de
/// recherche peut la pousser) donnerait en Java un `nextInt` de borne négative et une
/// `IllegalArgumentException` ; le port garde `1`, la plus petite borne que `nextInt`
/// accepte.
pub fn hit_bound(primary_skill: u32) -> u32 {
    1 + crate::ai_work_09::herder::ONE_HUNDRED_PERCENT.saturating_sub(primary_skill)
        / SKILL_TO_CHANCE_DIVISOR
}

/// `worker.getRandom().nextInt(1 + (ONE_HUNDRED_PERCENT - getPrimarySkillLevel()) / 5) <=
/// 1` (`:61`) : la frappe passe-t-elle ?
///
/// Un tirage au-dessus de la borne ne peut pas venir de `nextInt` : l'appelant a menti,
/// et le port refuse de le lire comme un coup réussi.
pub fn hit_lands(primary_skill: u32, draw: u32) -> bool {
    draw < hit_bound(primary_skill) && draw <= 1
}

/// `getButcheringAttackDamage()` du lapinier : celui du parent, `5.0`
/// (`AbstractEntityAIHerder.java:71`, `:848-851` — pas de surcharge dans cette classe).
pub fn attack_damage() -> f64 {
    crate::ai_work_09::herder::BUTCHERING_ATTACK_DAMAGE
}

/// `updateRenderMetaData` (`:37-46`) : `""` au repos, `"working"` au travail, et
/// `"…carrot"` dès que l'inventaire porte une carotte — y compris au repos, parce que
/// `:41-44` est après le ternaire de `:40`.
pub fn render_meta(scene: &HerdScene) -> String {
    let Some(facts) = scene.rabbit() else {
        return String::new();
    };
    let mut meta = String::new();
    if !facts.idle {
        meta.push_str(RENDER_META_WORKING);
    }
    if facts.carrots > 0 {
        meta.push_str(RENDER_META_CARROT);
    }
    meta
}

/// `decideWhatToDo` — **non surchargé** (`:20-69` n'en contient pas).
///
/// La décision est celle d'`AbstractEntityAIHerder` (`:199-249`) ; cette fonction rend
/// donc la réponse du parent telle quelle, et c'est ce qui rend le test du lapinier
/// intéressant : sa suite d'états est *exactement* celle de la table de base, aux frappes
/// près.
pub fn decide_what_to_do(scene: &mut HerdScene, _from: HerdState) -> Decision {
    if !scene.job_matches() {
        return Decision::refuse(TradeRefusal::WrongTradeFacts);
    }
    let Some(choice) = scene.take_base_answer() else {
        return Decision::refuse(TradeRefusal::BaseDecisionMissing);
    };
    Decision::go(choice.state())
}

/// `butcherAnimal(animal)` tel que le lapinier l'écrit (`:54-68`).
///
/// Appelée par `AbstractEntityAIHerder.butcherAnimals` (`:377`), dont le port est
/// `crate::ai_work_09::herder::butcher_animals` : c'est **le point de surcharge** entre
/// les deux modules, et la raison pour laquelle cette fonction est publique.
///
/// Elle ne rend pas d'état : Java est `void`, et `:379` lit ensuite `!toKill.isAlive()`.
/// Elle rend donc l'effet, ou le refus qui explique pourquoi il n'y en a pas.
pub fn butcher_animal(scene: &mut HerdScene, animal: u32) -> Result<HerdAct, TradeRefusal> {
    let Some(target) = scene.pen.by_id(animal) else {
        return Err(TradeRefusal::AnimalGone);
    };
    let Some(facts) = scene.rabbit() else {
        return Err(TradeRefusal::WrongTradeFacts);
    };
    // `:57` — `animal != null && !walkingToAnimal(animal) &&
    // !ItemStackUtils.isEmpty(worker.getMainHandItem())`.
    if !facts.main_hand_filled {
        return Err(TradeRefusal::NoWeaponInHand);
    }
    if !arrived(&scene.facts, target.at, WALK_TO_ANIMAL_RANGE) {
        return Err(TradeRefusal::NotAtAnimal);
    }
    // `:61` — `nextInt(1 + (ONE_HUNDRED_PERCENT - primary) / 5) <= 1`.
    if !hit_lands(facts.primary_skill, scene.draws.hit) {
        // Le tirage a raté : rien n'est usé, rien n'est touché (`:61-66` est un seul
        // bloc).
        return Err(TradeRefusal::HitMissed);
    }
    // `:59-66` — `worker.swing(MAIN_HAND)`, `animal.hurt(ds, damage)`,
    // `CitizenItemUtils.damageItemInHand(worker, MAIN_HAND, 1)`.
    scene.memory.hand = Hand::Axe;
    let act = HerdAct::Butchered { id: animal };
    scene.memory.act = Some(act);
    Ok(act)
}

/// L'état vers lequel `butcherAnimals` repart après une frappe (`:379-389`).
///
/// `!toKill.isAlive()` vrai rend `DECIDE` et compte la statistique, faux rend
/// `HERDER_BUTCHER` pour un second coup — c'est le parent qui l'écrit
/// (`AbstractEntityAIHerder.java:379-389`), et le lapinier en hérite.
pub fn after_hit(killed: bool) -> HerdState {
    if killed {
        HerdState::Base(WorkerState::Decide)
    } else {
        HerdState::HerderButcher
    }
}

/// La hutte de ce métier : `getExpectedBuildingClass()` (`:48-52`) rend
/// `BuildingRabbitHutch`, qui ne peut pas manger de carottes (`BuildingRabbitHutch.java:
/// 47-55`) et en garde une pile par coffre (`:37`).
pub const EXPECTED_BUILDING: &str = "BuildingRabbitHutch.java:16";
