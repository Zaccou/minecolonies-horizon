//! Le palefrenier : trois cibles, deux comptes à rebours, un dressage, une préparation au
//! combat et une rentrée au bercail.
//!
//! Java référence : `core/entity/ai/workers/production/herders/EntityAIWorkStablemaster.java`
//! (757 lignes), fille d'`AbstractEntityAIHerder`. C'est de loin le plus gros des trois,
//! et le seul dont la surcharge de `decideWhatToDo` ne se contente pas de filtrer une
//! réponse : elle ouvre trois buts de plus, avec deux comptes à rebours et trois tirages.
//!
//! ## Les trois cibles, et ce qu'elles font
//!
//! | Cible | Cadence | Rôle |
//! | --- | ---: | --- |
//! | `HERDER_TRAIN` (`:102`) | 10 | `convertMount` (`:268-339`) : marcher jusqu'à un cheval vanilla, le dresser en monture de cavalerie |
//! | `HERDER_READY_FOR_COMBAT` (`:103`) | 10 | `readyMountForCombat` (`:346-422`) : marcher jusqu'à un étalon, appliquer le bonus de santé, le nourrir, réduire son compte à rebours de combat |
//! | `HERDER_GATHER_MOUNTS` (`:104`) | 10 | `gatherMounts` (`:563-643`) : ramener au bercail un cheval errant, à la laisse |
//!
//! ## La surcharge de la décision, dans l'ordre des lignes
//!
//! ```text
//! result = super.decideWhatToDo()                        :234
//! trainingCooldown--; roundupCooldown--                  :235-236
//! si trainingCooldown <= 0 && rand < .25                 :238
//!     trainingCooldown = 50; → HERDER_TRAIN              :240-241
//! si rand < .40  → HERDER_READY_FOR_COMBAT               :244-247
//! si roundupCooldown <= 0 && rand < .15                  :250
//!     roundupCooldown = 100; wanderingHorses = …         :252-253
//!     si la liste n'est pas vide → HERDER_GATHER_MOUNTS  :255-258
//! → result                                               :261
//! ```
//!
//! **Les comptes à rebours se comptent en passages, pas en ticks.** Chaque `DECIDE` vaut
//! `DECIDING_DELAY = 80` ticks (`AbstractEntityAIHerder.java:143`), donc
//! `MAX_TRAINING_COOLDOWN = 50` (`:75`) vaut 4 000 ticks de jeu — 200 secondes — et
//! `MAX_ROUNDUP_COOLDOWN = 100` (`:72`) en vaut 8 000, soit 400 secondes. Un joueur qui
//! compte « cinquante ticks » se trompe de deux ordres de grandeur.
//!
//! **Trois tirages, mais pas toujours trois.** `&&` court-circuite : les tirages de
//! `:238` et `:250` ne sont **pas** consommés quand le compte à rebours est positif, alors
//! que celui de `:244` l'est à chaque passage. C'est ce qui fait qu'un générateur partagé
//! ne rend pas la même suite selon l'historique — le port nomme donc chaque tirage par son
//! usage ([`super::facts::Draws`]).
//!
//! ## Ce que le Java décide et qu'on n'attend pas
//!
//! 1. **`getNextStallPosition()` peut valoir `null`** quand aucune stalle n'est étiquetée
//!    (`BuildingStable.java:128-131`), et `walkToSafePos(null)` (`:591`) le déréférence
//!    aussitôt : sans stalle, le palefrenier lève une `NullPointerException` en pleine
//!    boucle de tick. Le port nomme [`TradeRefusal::NoStall`] et rend `DECIDE`.
//! 2. **`findNearbyUnstabledHorses` filtre la mauvaise variable.** `:753` teste
//!    `!(horseToRetrieve instanceof CavalryHorseEntity cav && cav.hasReservation())` — le
//!    champ, pas `horse`, la bête du flux. Réservé ou non, un étalon hors de l'écurie est
//!    donc ramassé ; le filtre ne mord que sur la bête déjà suivie, qui est justement
//!    celle qu'on ne cherche pas. Le port garde le comportement et le dit.
//! 3. **`readyMount` prend l'objet dans la hutte sans vérifier l'arrivée.**
//!    `walkToBuilding()` (`:442`) rend un booléen que personne ne lit, et
//!    `findFirstSlotInProviderNotEmptyWith(building, …)` (`:444`) lit l'inventaire **de la
//!    hutte** : la pile est retirée même si le citoyen est encore à vingt blocs. Le port
//!    reproduit la prise, et `at_hut` reste disponible pour l'appelant qui voudra, lui,
//!    attendre.
//! 4. **`chanceToButcher` rend zéro** (`:220-225`) : les montures ne sont jamais abattues,
//!    quoi que fasse le troupeau.
//! 5. **`canBreedChildren` est plafonné par la hutte** (`:189-209`) : `niveau * 2`
//!    étalons au maximum, et `Math.max(0, …)` pour un niveau nul.
//! 6. **`feedHorse` soigne 20 % de la santé maximale et donne trois points de tempérament**
//!    (`:523-537`) ; le soin n'a lieu que si la bête est blessée, le tempérament est
//!    toujours donné (`temper != 0` est une constante).
//! 7. **L'armure de cheval en cuir court-circuite tout** (`:480-483`) : elle pose le
//!    compte à rebours de combat à zéro au lieu de le réduire de la récupération.

use crate::ai::state::WorkerState;

use super::facts::{arrived, HerdAct, HerdScene, MountMaintenance, PenAnimal, StableFacts};
use super::state::{Hand, HerdState, TradeRefusal};
use super::Decision;

/// `TRAINING_CHANCE` (`EntityAIWorkStablemaster.java:66`).
pub const TRAINING_CHANCE: f64 = 0.25;
/// `READY_MOUNT_FOR_COMBAT_CHANCE` (`:67`).
pub const READY_MOUNT_FOR_COMBAT_CHANCE: f64 = 0.40;
/// `ROUND_UP_CHANCE` (`:68`).
pub const ROUND_UP_CHANCE: f64 = 0.15;
/// `BASE_COMBAT_READINESS_RECOVERY` (`:70`) : la récupération par objet de cuir, avant la
/// part d'habileté (`:486`).
pub const BASE_COMBAT_READINESS_RECOVERY: f32 = 4.0;
/// `MAX_ROUNDUP_COOLDOWN` (`:72`), en passages de `DECIDE`.
pub const MAX_ROUNDUP_COOLDOWN: i32 = 100;
/// `MAX_TRAINING_COOLDOWN` (`:75`), en passages de `DECIDE`.
pub const MAX_TRAINING_COOLDOWN: i32 = 50;
/// `RECOVERY_SKILL_PAR` (`:82`) : l'habileté principale qui donne la récupération pleine.
pub const RECOVERY_SKILL_PAR: u32 = 20;
/// `healPercentage = .20F` de `feedHorse` (`:523`).
pub const HEAL_PERCENTAGE: f32 = 0.20;
/// `temper = 3` de `feedHorse` (`:524`).
pub const TEMPER: u32 = 3;
/// La limite d'étalons par niveau de hutte : `building.getBuildingLevel() * 2` (`:199`,
/// `:270`).
pub fn mount_limit(building_level: u32) -> u32 {
    building_level
        .saturating_mul(crate::ai_work_09::herder::ANIMAL_MULTIPLIER)
}

/// `CavalryHorseEntity.COMBAT_READINESS_THRESHOLD` (`CavalryHorseEntity.java:107`) : le
/// seuil, en fraction de la santé maximale, sous lequel une monture est prête.
pub const COMBAT_READINESS_THRESHOLD: f32 = 0.66;

/// `restrictTo(worker.blockPosition(), 3)` (`:594`, `:691`) : le rayon auquel une bête à
/// la laisse est retenue.
pub const LEASH_RESTRICT_RANGE: i32 = 3;

/// `StackList(neededItem, level, component, 8, 4, 0)` (`:508`) : la taille de la requête
/// envoyée quand ni l'inventaire ni la hutte n'ont l'objet.
pub const STACKLIST_REQUEST_COUNT: u32 = 8;
/// Le minimum de la même requête (`:508`).
pub const STACKLIST_REQUEST_MIN: u32 = 4;

/// `findNearbyUnstabledHorses` : le rayon de recherche, `30.0` (`:737`).
pub const ROUNDUP_RADIUS: f64 = 30.0;

/// Le nombre de ticks que dure un compte à rebours d'entraînement : 50 passages de
/// `DECIDE` (`:75`) fois `DECIDING_DELAY` ticks (`AbstractEntityAIHerder.java:143`).
pub fn train_cooldown_ticks() -> u32 {
    MAX_TRAINING_COOLDOWN as u32 * crate::ai_work_09::herder::DECIDING_DELAY
}

/// Le même calcul pour la rentrée : 100 (`:72`) × 80 = 8 000 ticks, 400 secondes.
pub fn roundup_cooldown_ticks() -> u32 {
    MAX_ROUNDUP_COOLDOWN as u32 * crate::ai_work_09::herder::DECIDING_DELAY
}

/// La récupération de combat qu'un objet de cuir rend : `BASE_COMBAT_READINESS_RECOVERY *
/// (getPrimarySkillLevel() / RECOVERY_SKILL_PAR)` (`:486`).
///
/// À habileté 0, la récupération vaut `0.0` : Java ne le corrige pas, donc un palefrenier
/// sans habileté peut brûler du cuir sans jamais préparer une monture. Ce n'est pas une
/// erreur de portage, c'est `:486`.
pub fn combat_recovery(primary_skill: u32) -> f32 {
    BASE_COMBAT_READINESS_RECOVERY * (primary_skill as f32 / RECOVERY_SKILL_PAR as f32)
}

/// Le nombre d'objets de cuir qu'il faut pour amener une monture au seuil, à partir du
/// compte à rebours d'une bête fraîche (`= maxHealth`, `CavalryHorseEntity.java:752`).
///
/// C'est la « durée de travail » de ce but, en unités de cuir : chaque objet rend
/// [`combat_recovery`] et le seuil est `maxHealth * .66`
/// (`CavalryHorseEntity.java:107`, `:970-975`). Rend `None` quand la récupération est
/// nulle — le cas de l'habileté 0, où aucun nombre d'objets ne suffit.
pub fn leathers_to_ready(primary_skill: u32, max_health: f32) -> Option<u32> {
    let recovery = combat_recovery(primary_skill);
    if recovery <= 0.0 {
        return None;
    }
    let threshold = max_health * COMBAT_READINESS_THRESHOLD;
    let mut cooldown = max_health;
    let mut items = 0;
    while cooldown > threshold {
        cooldown -= recovery;
        items += 1;
        if items > 10_000 {
            // Garde-fou : une récupération dénoyautée ferait boucler sans fin.
            return None;
        }
    }
    Some(items)
}

/// `chanceToButcher(allAnimals)` (`:220-225`) : **zéro**, toujours. La javadoc dit « No
/// butchering the cavalry steeds! » et le corps rend `0`.
pub fn chance_to_butcher() -> f64 {
    0.0
}

/// `canBreedChildren()` (`:189-209`) : le réglage du parent, moins la limite d'étalons.
pub fn can_breed_children(facts: StableFacts) -> bool {
    // `:192` — `super.canBreedChildren()`, c'est-à-dire le réglage BREEDING
    // (`AbstractEntityAIHerder.java:278-281`).
    if !facts.breeding_setting {
        return false;
    }
    // `:199-200` — `Math.max(0, level * 2)` et le compte courant.
    let limit = mount_limit(facts.building_level);
    facts.mounts < limit
}

/// `decideWhatToDo` (`:232-262`).
pub fn decide_what_to_do(scene: &mut HerdScene, _from: HerdState) -> Decision {
    if scene.stable().is_none() {
        return Decision::refuse(TradeRefusal::WrongTradeFacts);
    }
    // `:234` — `super.decideWhatToDo()` d'abord, puis les comptes à rebours : l'ordre
    // n'a pas d'importance pour la réponse, puisque `result` n'est rendu qu'à `:261`.
    let base = scene.take_base_answer();
    // `:235-236` — les deux décréments, inconditionnels.
    scene.memory.train_cooldown -= 1;
    scene.memory.roundup_cooldown -= 1;

    // `:238-242` — `trainingCooldown <= 0 && rand < .25`.
    if scene.memory.train_cooldown <= 0 && scene.draws.training < TRAINING_CHANCE {
        scene.memory.train_cooldown = MAX_TRAINING_COOLDOWN;
        return Decision::go(HerdState::HerderTrain);
    }
    // `:244-247` — ce tirage est consommé à chaque passage, sans condition.
    if scene.draws.ready < READY_MOUNT_FOR_COMBAT_CHANCE {
        return Decision::go(HerdState::HerderReadyForCombat);
    }
    // `:249-259` — la recherche de chevaux errants est bridée par le compte à rebours.
    if scene.memory.roundup_cooldown <= 0 && scene.draws.roundup < ROUND_UP_CHANCE {
        scene.memory.roundup_cooldown = MAX_ROUNDUP_COOLDOWN;
        // `:253` — l'affectation a lieu même si la liste est vide.
        let wandering = find_nearby_unstabled_horses(scene);
        scene.memory.wandering = wandering;
        if !scene.memory.wandering.is_empty() {
            return Decision::go(HerdState::HerderGatherMounts);
        }
    }
    // `:261`
    let Some(base) = base else {
        return Decision::refuse(TradeRefusal::BaseDecisionMissing);
    };
    Decision::go(base.state())
}

/// `findNearbyUnstabledHorses()` (`:733-755`).
///
/// Le centre est `building.getPosition()` plus un demi-bloc (`:736-743`), le rayon `30.0`
/// (`:737`), et les filtres sont ceux de `:749-754` : vivant, pas retiré, pas de cavalier,
/// pas dans l'écurie pour une monture de cavalerie.
///
/// **Le dernier terme de Java teste `horseToRetrieve`, pas `horse`** (`:753`) : le port
/// ne peut pas recopier une variable qui n'est pas dans les faits, et le test qui compte
/// ([`super::tests`]) mesure que la bête réservée **est** rendue, ce que le Java fait
/// aussi.
pub fn find_nearby_unstabled_horses(scene: &HerdScene) -> Vec<u32> {
    let centre = scene.facts.building.position;
    let radius_squared = ROUNDUP_RADIUS * ROUNDUP_RADIUS;
    scene
        .pen
        .animals
        .iter()
        .filter(|horse| !horse.removed && horse.alive)
        // `:751` — `!(horse instanceof CavalryHorseEntity cav && cav.hasCavalryRider())`.
        .filter(|horse| !(horse.kind.is_cavalry() && horse.has_cavalry_rider))
        // `:752` — `!(horse instanceof CavalryHorseEntity cav && cav.isInStable())`.
        .filter(|horse| !(horse.kind.is_cavalry() && horse.in_stable))
        // `:754` — `horse.distanceToSqr(cx, cy, cz) <= r2`, le centre étant le bloc de la
        // hutte plus un demi.
        .filter(|horse| {
            let dx = horse.at.x as f64 + 0.5 - (centre.x as f64 + 0.5);
            let dy = horse.at.y as f64 + 0.5 - (centre.y as f64 + 0.5);
            let dz = horse.at.z as f64 + 0.5 - (centre.z as f64 + 0.5);
            dx * dx + dy * dy + dz * dz <= radius_squared
        })
        .map(|horse| horse.id)
        .collect()
}

/// `convertMount()` (`:268-339`).
pub fn convert_mount(scene: &mut HerdScene, _from: HerdState) -> Decision {
    let Some(facts) = scene.stable() else {
        return Decision::refuse(TradeRefusal::WrongTradeFacts);
    };
    // `:270` — `Math.max(0, building.getBuildingLevel() * 2)`.
    let limit = mount_limit(facts.building_level);

    // `:273-295` — une conversion en cours : marcher, puis convertir.
    if let Some(id) = scene.memory.to_convert {
        let Some(horse) = scene.pen.by_id(id) else {
            scene.memory.to_convert = None;
            return Decision::go_after(
                HerdState::Base(WorkerState::Decide),
                TradeRefusal::MountGone,
            );
        };
        // `:273-276` — `walkToSafePos(horse.blockPosition())` faux rend `HERDER_TRAIN`.
        if !arrived(&scene.facts, horse.at, super::facts::SAFE_POS_RANGE) {
            return Decision::refuse(TradeRefusal::NotAtMount);
        }
        // `:279-295` — la conversion elle-même.
        scene.memory.to_convert = None;
        if !horse.convertible {
            // `:281-285` — `createFromVanilla` a rendu `null`, Java avertit et continue.
            return Decision::go_after(
                HerdState::Base(WorkerState::Decide),
                TradeRefusal::MountNotConvertible,
            );
        }
        // `:289` — `addExperience(XP_PER_ACTION)` ; `:290` — la statistique ;
        // `:291` — `incrementActionsDoneAndDecSaturation()`.
        scene.facts.inventory.actions_done += 1;
        scene.memory.act = Some(HerdAct::Converted { id });
        // `:294`
        return Decision::go(HerdState::Base(WorkerState::Decide));
    }

    // `:297-304` — à la limite, on ne dresse pas.
    if facts.mounts >= limit {
        return Decision::go_after(
            HerdState::Base(WorkerState::Decide),
            TradeRefusal::MountLimitReached,
        );
    }

    // `:306-335` — le premier candidat convertible.
    match first_training_candidate(scene) {
        Some(id) => {
            // `:333` — `horseToConvert = firstCandidate`.
            scene.memory.to_convert = Some(id);
            Decision::go(HerdState::HerderTrain)
        }
        // `:337-338`
        None => Decision::go_after(
            HerdState::Base(WorkerState::Decide),
            TradeRefusal::NoTrainableMount,
        ),
    }
}

/// Le premier candidat au dressage (`:306-335`).
///
/// Java parcourt `searchForAnimals(module::isCompatible)` et retient le **premier** `a`
/// tel que : pas déjà une monture de cavalerie (`:316-319`), `instanceof AbstractHorse`
/// (`:322`), `isAlive()` et `!isBaby()` et `getPassengers().isEmpty()` (`:324`).
pub fn first_training_candidate(scene: &HerdScene) -> Option<u32> {
    scene
        .pen
        .animals
        .iter()
        .find(|animal| {
            !animal.kind.is_cavalry()
                && animal.kind.is_horse()
                && animal.alive
                && !animal.baby
                && animal.passengers == 0
        })
        .map(|animal| animal.id)
}

/// `readyMountForCombat()` (`:346-422`).
pub fn ready_mount_for_combat(scene: &mut HerdScene, _from: HerdState) -> Decision {
    let Some(facts) = scene.stable() else {
        return Decision::refuse(TradeRefusal::WrongTradeFacts);
    };
    // `:349-396` — une monture déjà choisie.
    if let Some(id) = scene.memory.to_get_ready {
        let Some(mount) = scene.pen.by_id(id) else {
            scene.memory.to_get_ready = None;
            return Decision::go_after(
                HerdState::Base(WorkerState::Decide),
                TradeRefusal::MountGone,
            );
        };
        // `:349-358` — la marche.
        if !arrived(&scene.facts, mount.at, super::facts::SAFE_POS_RANGE) {
            if mount.has_cavalry_rider {
                // `:351-355` — un cavalier est monté pendant la marche : terminé.
                return Decision::go_after(
                    HerdState::Base(WorkerState::Decide),
                    TradeRefusal::MountRidden,
                );
            }
            return Decision::refuse(TradeRefusal::NotAtMount);
        }
        // `:361-374` — le bonus de santé d'abord, et il clôt le tick.
        if mount.needs_health_training {
            scene.memory.to_get_ready = None;
            // `:365-370` — `applyHealthTraining()` ; `:367` garde l'expérience et `:369`
            // le compteur d'actions derrière son booléen. `applyHealthTraining`
            // (`CavalryHorseEntity.java:794-799`) re-teste `needsHealthTraining()` et rend
            // faux si elle est fausse : ici elle est vraie, donc l'application aussi.
            scene.facts.inventory.actions_done += 1;
            scene.memory.act = Some(HerdAct::HealthTrained { id });
            return Decision::go(HerdState::Base(WorkerState::Decide));
        }
        // `:376-392` — l'entretien.
        let needs_feed = mount.health < mount.max_health;
        let needs_ready = mount.combat_cooldown > 0.0;
        let mut did_work = false;
        if needs_feed {
            did_work = ready_mount(scene, facts, MountMaintenance::Feeding, id);
        }
        if needs_ready {
            let readied = ready_mount(scene, facts, MountMaintenance::Readying, id);
            did_work = did_work || readied;
        }
        if did_work {
            // `:390-391`
            scene.memory.to_get_ready = None;
        }
        // `:394`
        return Decision::go(HerdState::Base(WorkerState::Decide));
    }

    // `:398-404` — le module d'élevage.
    if !facts.has_herding_module {
        return Decision::go_after(
            HerdState::Base(WorkerState::Decide),
            TradeRefusal::NoHerdingModule,
        );
    }

    // `:406-418` — la première monture qui demande quelque chose.
    match mount_to_ready(scene) {
        Some(id) => {
            scene.memory.to_get_ready = Some(id);
            Decision::go(HerdState::HerderReadyForCombat)
        }
        None => Decision::go_after(
            HerdState::Base(WorkerState::Decide),
            TradeRefusal::NoMountToReady,
        ),
    }
}

/// La première monture qui demande un entretien (`:406-418`).
///
/// Java ne retient que les `CavalryHorseEntity` (`:410`) et seulement celles qui
/// remplissent une des trois conditions de `:412` : bonus de santé à corriger, compte à
/// rebours de combat positif, ou blessure.
pub fn mount_to_ready(scene: &HerdScene) -> Option<u32> {
    scene
        .pen
        .animals
        .iter()
        .find(|mount| {
            mount.kind.is_cavalry()
                && (mount.needs_health_training
                    || mount.combat_cooldown > 0.0
                    || mount.health < mount.max_health)
        })
        .map(|mount| mount.id)
}

/// `readyMount(task, horse)` (`:429-514`) : consommer un objet et l'appliquer.
///
/// Rend `didWork` : vrai quand un objet a été retiré et l'effet appliqué (`:501`). Le
/// `false` de `:465` (la pile extraite était vide) et le `false` final (`:513`, ni
/// inventaire ni hutte) sont les deux autres sorties.
fn ready_mount(
    scene: &mut HerdScene,
    facts: StableFacts,
    task: MountMaintenance,
    horse: u32,
) -> bool {
    // `:431` — l'objet, selon le travail.
    let (in_inventory, in_hut) = match task {
        MountMaintenance::Feeding => (facts.feed_in_inventory, facts.feed_in_hut),
        MountMaintenance::Readying => (facts.leather_in_inventory, facts.leather_in_hut),
    };
    let mut count = in_inventory;
    // `:438-451` — si l'inventaire est vide et que la hutte a l'objet, la pile est prise
    // **sans attendre l'arrivée** : `walkToBuilding()` (`:442`) rend une valeur que
    // personne ne lit.
    if count == 0 && in_hut > 0 {
        // `:444-448` — `findFirstSlotInProviderNotEmptyWith` puis
        // `takeItemStackFromProvider`, sur l'inventaire **de la hutte** : la prise ne
        // dépend pas de la position du citoyen.
        count = 1;
    }
    // `:456-458` — la pile est dans l'inventaire du citoyen.
    if count == 0 {
        // `:504-511` — rien nulle part : une requête de liste, une seule à la fois.
        if !facts.open_stacklist_request {
            scene.memory.act = Some(HerdAct::RequestedItems { task });
        }
        let refusal = match task {
            MountMaintenance::Feeding => TradeRefusal::NoFeedItem,
            MountMaintenance::Readying => TradeRefusal::NoReadyItem,
        };
        scene.refuse(refusal);
        return false;
    }
    // `:460-461` — `extractItem(slot, 1, false)` puis `setItemInHand(MAIN_HAND, …)`.
    scene.memory.hand = match task {
        MountMaintenance::Feeding => Hand::Feed,
        MountMaintenance::Readying => Hand::Leather,
    };
    match task {
        MountMaintenance::Feeding => {
            // `:468-474` — `feedHorse(horse)`, la statistique, `shrink(1)`, les effets.
            let healed = scene
                .pen
                .by_id(horse)
                .map(|mount| mount.health < mount.max_health)
                .unwrap_or(false);
            scene.memory.act = Some(HerdAct::Fed { id: horse, healed });
        }
        MountMaintenance::Readying => {
            // `:476-499` — soit l'armure de cuir qui remet le compte à rebours à zéro,
            // soit la récupération partielle.
            let ready = scene
                .pen
                .by_id(horse)
                .map(|mount| readied(mount, facts))
                .unwrap_or(false);
            scene.memory.act = Some(HerdAct::Readied { id: horse, ready });
        }
    }
    // `:502` — `incrementActionsDone()`.
    scene.facts.inventory.actions_done += 1;
    true
}

/// L'état d'une monture après un objet de cuir (`:480-498`).
fn readied(mount: PenAnimal, facts: StableFacts) -> bool {
    if facts.leather_horse_armor {
        // `:480-483` — `setCombatCooldown(0)`, donc prête.
        return true;
    }
    // `:486-487` — `prepareForCombat(recovery)` puis `isReadyForCombat()` (`:495`).
    let remaining = mount.combat_cooldown - combat_recovery(facts.primary_skill);
    remaining <= mount.max_health * COMBAT_READINESS_THRESHOLD
}

/// `feedHorse(horse)` (`:519-546`) : soigne 20 % de la santé maximale si la bête est
/// blessée, et donne trois points de tempérament dans tous les cas.
///
/// Rend `didSomething` : vrai seulement quand le soin a eu lieu (`:529-533`). Le
/// tempérament, lui, est donné même à pleine santé, parce que `temper != 0` (`:535`) est
/// une constante.
pub fn feed_horse(mount: PenAnimal) -> bool {
    mount.health < mount.max_health
}

/// La santé qu'un nourrissage rend : `maxHealth * healPercentage` (`:531`).
pub fn heal_amount(max_health: f32) -> f32 {
    max_health * HEAL_PERCENTAGE
}

/// `gatherMounts()` (`:563-643`).
pub fn gather_mounts(scene: &mut HerdScene, _from: HerdState) -> Decision {
    let Some(facts) = scene.stable() else {
        return Decision::refuse(TradeRefusal::WrongTradeFacts);
    };
    // `:566-633` — une bête déjà suivie.
    if let Some(id) = scene.memory.to_retrieve {
        let Some(horse) = scene.pen.by_id(id) else {
            detach_horse(scene, id);
            scene.memory.to_retrieve = None;
            return Decision::go_after(
                HerdState::Base(WorkerState::Decide),
                TradeRefusal::MountGone,
            );
        };
        // `:568-576` — morte, retirée, montée ou réservée : on lâche tout.
        if !horse.alive
            || horse.removed
            || (horse.kind.is_cavalry() && horse.has_cavalry_rider)
            || (horse.kind.is_cavalry() && horse.has_reservation)
        {
            detach_horse(scene, id);
            scene.memory.to_retrieve = None;
            return Decision::go_after(
                HerdState::Base(WorkerState::Decide),
                TradeRefusal::MountGone,
            );
        }
        // `:578-587` — la laisse.
        if !facts.lead_ok {
            detach_horse(scene, id);
            scene.memory.to_retrieve = None;
            return Decision::go_after(
                HerdState::Base(WorkerState::Preparing),
                TradeRefusal::NoLead,
            );
        }
        // `:589-618` — la bête est à la laisse : la ramener à la stalle.
        if horse.leashed {
            let Some(stall) = facts.stall else {
                // `:591` — `walkToSafePos(building.getNextStallPosition())` sur un nul :
                // Java lève une `NullPointerException` ici.
                detach_horse(scene, id);
                scene.memory.to_retrieve = None;
                return Decision::go_after(
                    HerdState::Base(WorkerState::Decide),
                    TradeRefusal::NoStall,
                );
            };
            // `:591-596` — la marche vers la stalle.
            if !arrived(&scene.facts, stall, super::facts::SAFE_POS_RANGE) {
                // `:593-594` — `clearRestriction()` puis `restrictTo(worker, 3)`.
                return Decision::refuse(TradeRefusal::NotAtStall);
            }
            // `:598` — `detachHorse(horseToRetrieve)`.
            detach_horse(scene, id);
            // `:601` — `incrementActionsDoneAndDecSaturation()`.
            scene.facts.inventory.actions_done += 1;
            // `:603-606` — la bête sort de la liste des errants.
            scene.memory.wandering.retain(|other| *other != id);
            scene.memory.to_retrieve = None;
            // `:610-613` — il en reste : on continue.
            if !scene.memory.wandering.is_empty() {
                return Decision::go(HerdState::HerderGatherMounts);
            }
            // `:615` — la statistique de rentrée.
            scene.memory.act = Some(HerdAct::RoundupDone);
            // `:617`
            return Decision::go(HerdState::Base(WorkerState::Decide));
        }
        // `:620-632` — la bête n'est pas encore à la laisse.
        if !arrived(&scene.facts, horse.at, super::facts::SAFE_POS_RANGE) {
            return Decision::refuse(TradeRefusal::NotAtMount);
        }
        if attach_horse(scene, horse) {
            // `:626-628` — attachée : un tick de plus dans le même état.
            return Decision::go(HerdState::HerderGatherMounts);
        }
        // `:629-632`
        return Decision::go_after(
            HerdState::Base(WorkerState::Decide),
            TradeRefusal::HorseLostLeash,
        );
    }

    // `:635-638` — rien à suivre.
    if scene.memory.wandering.is_empty() {
        return Decision::go_after(
            HerdState::Base(WorkerState::Decide),
            TradeRefusal::NoRoundupTarget,
        );
    }
    // `:640-642` — la première de la liste.
    let first = scene.memory.wandering[0];
    scene.memory.to_retrieve = Some(first);
    Decision::go(HerdState::HerderGatherMounts)
}

/// `attachHorse(horse)` (`:670-694`) : vrai quand la bête est à la laisse du citoyen après
/// l'appel.
pub fn attach_horse(scene: &mut HerdScene, horse: PenAnimal) -> bool {
    // `:674` — `!horse.isAlive() || horse.isRemoved()`.
    if !horse.alive || horse.removed {
        return false;
    }
    // `:677` — déjà à la laisse de ce citoyen : rien à faire.
    if horse.leashed && horse.leash_holder {
        return true;
    }
    // `:680-683` — une laisse tenue par quelqu'un d'autre est lâchée d'abord.
    // `:685-688` — une laisse est posée en main secondaire si elle est vide.
    if !scene.memory.lead_in_offhand {
        scene.memory.lead_in_offhand = true;
        scene.memory.hand = Hand::Lead;
    }
    // `:690-691` — `setLeashedTo(worker, true)` et `restrictTo(worker, 3)`.
    scene.memory.act = Some(HerdAct::Leashed { id: horse.id });
    true
}

/// `detachHorse(horse)` (`:703-723`) : enlève la laisse et range celle de la main
/// secondaire.
pub fn detach_horse(scene: &mut HerdScene, horse: u32) {
    let leashed = scene
        .pen
        .by_id(horse)
        .map(|mount| mount.leashed)
        .unwrap_or(false);
    // `:710-715` — `clearRestriction()`, puis `return` si la bête n'est pas à la laisse.
    if !leashed {
        return;
    }
    // `:717` — `dropLeash(true, false)`.
    scene.memory.act = Some(HerdAct::Unleashed { id: horse });
    // `:719-722` — la laisse de la main secondaire est rangée.
    if scene.memory.lead_in_offhand {
        scene.memory.lead_in_offhand = false;
        scene.memory.hand = Hand::Empty;
    }
}
