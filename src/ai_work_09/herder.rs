//! L'éleveur : accoupler, abattre, nourrir, ramasser — et le tirage propre à
//! l'éleveur de poulets.
//!
//! Java référence : `core/entity/ai/workers/production/herders/AbstractEntityAIHerder.java`
//! — le `registerTargets` du constructeur (`:139-148`), `decideWhatToDo` (`:199-249`),
//! `isBreedAble` (`:257-260`), `canBreedChildren` (`:278-281`),
//! `startWorkingAtOwnBuilding` (`:288-295`), `prepareForHerding` (`:302-328`),
//! `butcherAnimals` (`:335-390`), `getCenterOfHerd` (`:398-412`), `breedAnimals`
//! (`:419-484`), `canMate` (`:493-510`), `feedAnimal` (`:517-568`), `pickupItems`
//! (`:575-588`), `searchForAnimals` (`:596-599`), `searchForItemsInArea` (`:611-614`),
//! `walkingToAnimal` (`:622-632`), `breedTwoAnimals` (`:639-665`), `chanceToButcher`
//! (`:673-696`), `equipTool` (`:705-713`), `equipItem` (`:740-752`),
//! `getButcheringAttackDamage` (`:848-851`) et `getRequestBreedingItems` (`:859-873`) ;
//! plus `EntityAIWorkChickenHerder.java:60-73` pour la frappe au tirage.
//!
//! ## La table que ce fichier ajoute, et pourquoi les deux métiers la partagent
//!
//! | # | État | Règle | Cadence | Java |
//! | ---: | --- | --- | ---: | --- |
//! | 1 | `IDLE` | `START_WORKING` (constante) | 1 | `:140` |
//! | 2 | `START_WORKING` | `startWorkingAtOwnBuilding` | 20 | `:141` |
//! | 3 | `PREPARING` | `prepareForHerding` | 20 | `:142` |
//! | 4 | `DECIDE` | `decideWhatToDo` | 80 | `:143` |
//! | 5 | `HERDER_BREED` | `breedAnimals` | 40 | `:144` |
//! | 6 | `HERDER_BUTCHER` | `butcherAnimals` | 20 | `:145` |
//! | 7 | `HERDER_PICKUP` | `pickupItems` | 20 | `:146` |
//! | 8 | `HERDER_FEED` | `feedAnimal` | 20 | `:147` |
//!
//! **Un seul fichier pour les deux métiers.** `EntityAIWorkChickenHerder` (74 lignes)
//! et `EntityAIWorkCowboy` (270) étendent tous les deux `AbstractEntityAIHerder` et ne
//! changent la table que par le cowboy, qui ajoute deux états
//! (`EntityAIWorkCowboy.java:69-72`) et surcharge `decideWhatToDo` (`:96-120`). Écrire
//! la table deux fois serait la deuxième copie d'une table qui n'a qu'un propriétaire ;
//! le cowboy se raccorde ici ([`register`]) et pose ses deux cibles dans
//! [`super::cowboy`].
//!
//! ## Ce que le Java décide et qu'on n'attend pas
//!
//! 1. **Trois tirages, dans cet ordre, et le premier qui passe gagne.** `:230`
//!    (ramassage, `< 0.1`), puis `:234` (abattage, `< chanceToButcher`), puis `:242`
//!    (nourrissage, `< 0.1`). L'accouplement (`:238`) n'a **pas** de tirage : c'est une
//!    conjonction de quatre faits. Donc un troupeau qui remplit les conditions
//!    d'accouplement ne s'accouple que si les trois tirages ont échoué — et un tirage
//!    d'abattage bas vole l'accouplement.
//! 2. **`chanceToButcher` peut valoir `Infinity`.** Avec `getBuildingLevel() == 0`, le
//!    `maxAnimals` vaut 0 et `:695` divise par `0⁴` (`Math.pow(0, 4) == 0.0`). Java
//!    rend `Infinity` et le troupeau est abattu. Le port ne corrige pas : c'est le Java.
//! 3. **`isFeedAble` n'est appelée nulle part** (`:268-271`). Mesuré :
//!    `grep -rn isFeedAble src/main/java/` ne rend que la déclaration. Le port la garde
//!    et le dit.
//! 4. **`butcherAnimals` abat le plus loin du centre du troupeau** (`:357`), puis
//!    remplace la cible par un animal qui ne voit pas le ciel si la cible le voit
//!    (`:365`). Le premier animal retenu n'est donc pas toujours le plus loin.
//! 5. **`breedTimeOut` n'est décrémenté que de `DECIDING_DELAY`** (`:205`), donc un
//!    délai de 60 secondes (`:438`, `:481`) demande 15 passages en `DECIDE`, soit 1 200
//!    ticks de jeu. Et il n'est jamais remis à zéro explicitement : il traverse zéro.
//! 6. **Un ramassage raté compte quand même une action.** `:579-587` : seule l'arrivée
//!    à l'item rend `getState()`. En marchant, `walkWithProxy` rend faux et le `if`
//!    tombe dans `incrementActionsDone()` — donc chaque tick de marche vers un item
//!    vide un cran du quota de dix actions et renvoie `DECIDE`, qui retire le tirage de
//!    ramassage à 10 %.
//!
//! ## Ce qui n'est pas ici
//!
//! Aucun appel au monde : chercher les animaux, marcher, frapper, nourrir et ramasser
//! sont des faits ([`super::facts::HerdFacts`]) que l'appelant remplit. L'arrivée près
//! d'un animal est **calculée** ([`walking_to_animal`]) et non lue, parce que
//! `walkToPos(worker, pos, 2, true)` n'a de sens que dans le monde : le port en garde
//! le test géométrique et cite la ligne — voir `MANQUANT.md` §1.

use crate::ai::facts::{Pos, WorldFacts};
use crate::ai::state::{Machine, WorkerState};

use super::facts::{HerdAnimal, HerdFacts, WorkMemory};
use super::state::{WorkRefusal, WorkState};
use super::{herd_action, Decision, TICKS_SECOND};

/// `ANIMAL_MULTIPLIER` (`:56`) : combien d'animaux par niveau de hutte (`:675`).
pub const ANIMAL_MULTIPLIER: u32 = 2;

/// `NUM_OF_ANIMALS_TO_BREED` (`:61`) : deux animaux au moins pour accoupler (`:238`).
pub const NUM_OF_ANIMALS_TO_BREED: usize = 2;

/// `EXTRA_BREEDING_ITEMS_REQUEST` (`:66`) : la requête couvre huit portions pour
/// épargner le livreur (`:319`, `:868`).
pub const EXTRA_BREEDING_ITEMS_REQUEST: u32 = 8;

/// `BUTCHERING_ATTACK_DAMAGE` (`:71`) : les dégâts de l'éleveur de poulets, qui ne
/// surcharge pas `getButcheringAttackDamage` (`:848-851`).
pub const BUTCHERING_ATTACK_DAMAGE: f64 = 5.0;

/// `DISTANCE_TO_BREED` (`:76`) : les deux partenaires doivent être à dix blocs ou
/// moins l'un de l'autre (`:452`).
pub const DISTANCE_TO_BREED: f64 = 10.0;

/// `BUTCHER_DELAY` (`:81`) : la cadence de `HERDER_BUTCHER` (`:145`).
pub const BUTCHER_DELAY: u32 = 20;

/// `DECIDING_DELAY` (`:82`) : la cadence de `DECIDE` (`:143`) **et** le pas de
/// décrémentation de `breedTimeOut` (`:205`).
pub const DECIDING_DELAY: u32 = 80;

/// `BREEDING_DELAY` (`:83`) : la cadence de `HERDER_BREED` (`:144`).
pub const BREEDING_DELAY: u32 = 40;

/// `LIMIT_TO_FEED_CHILDREN` (`:88`) : à partir de cette compétence secondaire, il fait
/// grandir les petits au lieu de seulement les nourrir (`:548`).
pub const LIMIT_TO_FEED_CHILDREN: u32 = 10;

/// `ACTIONS_FOR_DUMP` (`:93`), la réponse de `getActionsDoneUntilDumping`
/// (`:152-156`).
pub const ACTIONS_FOR_DUMP: u32 = 10;

/// `MAX_ENTITY_AGE = AgeableMob.BABY_START_AGE` (`:98`) : l'âge d'un nouveau-né, en
/// ticks négatifs.
pub const MAX_ENTITY_AGE: f64 = -24000.0;

/// `XP_PER_ACTION` (`:103`) : l'expérience d'un accouplement, d'un nourrissage ou d'un
/// abattage (`:382`, `:557`, `:659`).
pub const XP_PER_ACTION: f64 = 0.5;

/// `FEED_CHANCE` (`:108`) : la probabilité de nourrir quand tout le reste a échoué
/// (`:242`).
pub const FEED_CHANCE: f64 = 0.1;

/// La probabilité de ramassage de `:230`, écrite `0.1` en clair dans le Java et sans
/// constante : elle n'a pas de nom là-bas, le port lui en donne un pour la citer.
pub const PICKUP_CHANCE: f64 = 0.1;

/// `TICKS_SECOND * 20` (`:462`) : le délai après n'avoir trouvé aucun partenaire
/// valable — 20 secondes.
pub const BREED_TIMEOUT_NO_MATE: i32 = (TICKS_SECOND * 20) as i32;

/// `TICKS_SECOND * 60` (`:438`, `:481`) : le délai après un accouplement réussi ou
/// après n'avoir pas trouvé deux animaux — 60 secondes.
pub const BREED_TIMEOUT_LONG: i32 = (TICKS_SECOND * 60) as i32;

/// `TICKS_SECOND * 60 * 5` (`:534`) : un animal nourri l'est pour cinq minutes.
pub const FEED_COOLDOWN: u64 = (TICKS_SECOND as u64) * 60 * 5;

/// `EntityAIWorkChickenHerder.ONE_HUNDRED_PERCENT` (`:17`, la constante de
/// `Constants.java:50`) : le plafond du calcul de la frappe.
pub const ONE_HUNDRED_PERCENT: u32 = 100;

/// La portée de `walkingToAnimal` : `EntityNavigationUtils.walkToPos(worker,
/// animal.blockPosition(), 2, true)` (`:626`).
pub const WALK_TO_ANIMAL_RANGE: i32 = 2;

/// La portée de `walkWithProxy(items.get(0).blockPosition(), 1)` (`:579`).
pub const WALK_TO_ITEM_RANGE: i32 = 1;

/// `walkingToAnimal(animal)` (`:622-632`).
///
/// Java demande au chemin ; le port en garde le test géométrique —
/// `worker.blockPosition()` à moins de [`WALK_TO_ANIMAL_RANGE`] de
/// `animal.blockPosition()` — et le dit, parce qu'un résultat de chemin n'est pas une
/// donnée qu'un hôte peut rejouer.
pub fn walking_to_animal(facts: &WorldFacts, animal: HerdAnimal) -> bool {
    walking_to_pos(facts, animal.at)
}

/// La même marche, vers une position : `walkToPos(worker, pos, 2, true)`
/// (`EntityNavigationUtils.java:142-147`). Le cow-boy n'a que la position de son
/// animal ([`super::facts::CowboyFacts::milkable_animal`]).
pub fn walking_to_pos(facts: &WorldFacts, pos: Pos) -> bool {
    !facts.worker.at.is_living_at_site(pos, WALK_TO_ANIMAL_RANGE)
}

/// `walkWithProxy(items.get(0).blockPosition(), 1)` (`:579`) : il est arrivé sur
/// l'item.
pub fn at_item(facts: &WorldFacts, item: Pos) -> bool {
    facts.worker.at.is_living_at_site(item, WALK_TO_ITEM_RANGE)
}

/// `getCenterOfHerd` (`:398-412`) : la moyenne des positions, tronquée par
/// `BlockPos.containing` (`:411`).
pub fn center_of_herd(animals: &[HerdAnimal]) -> Pos {
    // :400-403 — un troupeau vide rend l'origine.
    if animals.is_empty() {
        return Pos::new(0, 0, 0);
    }
    let count = animals.len() as f64;
    let sum = animals.iter().fold((0.0, 0.0, 0.0), |acc, animal| {
        (
            acc.0 + animal.at.x as f64,
            acc.1 + animal.at.y as f64,
            acc.2 + animal.at.z as f64,
        )
    });
    // `BlockPos.containing` tronque vers le bas, comme `MathHelper.floor`.
    Pos::new(
        (sum.0 / count).floor() as i32,
        (sum.1 / count).floor() as i32,
        (sum.2 / count).floor() as i32,
    )
}

/// `chanceToButcher` (`:673-696`).
///
/// Le `Infinity` du niveau 0 est le Java : voir la note du module.
pub fn chance_to_butcher(building_level: u32, breeding_setting: bool, animals: &[HerdAnimal]) -> f64 {
    // :675
    let max_animals = (building_level * ANIMAL_MULTIPLIER) as f64;
    // :676-679 — sans le réglage d'accouplement, un troupeau dans la limite n'est pas
    // abattu.
    if !breeding_setting && (animals.len() as f64) <= max_animals {
        return 0.0;
    }
    // :681-688
    let grown_up = animals.iter().filter(|animal| !animal.baby).count() as f64;
    // :690-693 — trois adultes ou moins ne s'abattent pas.
    if grown_up <= 3.0 {
        return 0.0;
    }
    // :695 — `0.5 * (grownUp⁴ / maxAnimals⁴)`.
    0.5 * (grown_up.powi(4) / max_animals.powi(4))
}

/// La cible de `butcherAnimals` (`:354-375`), ou `None` quand `toKill` reste nul.
pub fn butcher_target(animals: &[HerdAnimal]) -> Option<HerdAnimal> {
    let center = center_of_herd(animals); // :354
    // :356-357 — le plus **loin** du centre d'abord.
    let mut sorted: Vec<HerdAnimal> = animals.to_vec();
    sorted.sort_by(|left, right| {
        right
            .at
            .dist_sqr(center)
            .cmp(&left.at.dist_sqr(center))
    });
    let mut to_kill: Option<HerdAnimal> = None;
    for entity in sorted {
        // :363 — ni bébé, ni amoureux.
        if entity.baby || entity.in_love {
            continue;
        }
        match to_kill {
            None => to_kill = Some(entity), // :365, premier terme
            Some(current) => {
                // :365, second terme — `!entity.canSeeSky && toKill.canSeeSky`.
                if !entity.can_see_sky && current.can_see_sky {
                    to_kill = Some(entity);
                }
            }
        }
    }
    to_kill // :372-375
}

/// `EntityAIWorkChickenHerder.butcherAnimal` (`:60-73`) : la borne du tirage.
///
/// `1 + (ONE_HUNDRED_PERCENT - getSecondarySkillLevel()) / 5` (`:67`), en division
/// entière. À 100 de compétence la borne vaut 1, donc `nextInt(1)` rend toujours 0 et
/// la condition est toujours vraie.
pub fn chicken_hit_bound(secondary_skill: u32) -> u32 {
    1 + ONE_HUNDRED_PERCENT.saturating_sub(secondary_skill) / 5
}

/// La frappe de l'éleveur de poulets porte-t-elle ? (`EntityAIWorkChickenHerder.java:
/// 67-71`).
///
/// `worker.getRandom().nextInt(bound) <= 1`, où `bound` est
/// [`chicken_hit_bound`]. Le `draw < bound` est une garde que Java n'a pas : un tirage
/// hors bornes ferait lever `IllegalArgumentException` à `nextInt` dans le jeu, et un
/// fait hors bornes est une observation cassée, pas un coup qui porte.
pub fn chicken_hit_lands(secondary_skill: u32, draw: u32) -> bool {
    let bound = chicken_hit_bound(secondary_skill);
    draw < bound && draw <= 1
}

/// `decideWhatToDo` (`:199-249`).
pub fn decide_what_to_do(
    _facts: &mut WorldFacts,
    trade: &HerdFacts,
    memory: &mut WorkMemory,
    _state: WorkState,
) -> Decision {
    // :203-206 — `breedTimeOut` descend de `DECIDING_DELAY` à chaque passage, jamais
    // sous zéro par une remise à zéro : il traverse zéro et la condition `:238`
    // (`== 0`) n'est donc jamais vraie juste après — voir `MANQUANT.md` §3.
    if memory.herd.breed_time_out > 0 {
        memory.herd.breed_time_out -= DECIDING_DELAY as i32;
    }
    // :208 puis :248 — la boucle sur les modules d'élevage. Le port a les faits d'un
    // seul module : sans module, la boucle ne tourne pas et `:248` rend
    // `START_WORKING`.
    if !trade.has_module {
        return Decision::go(WorkState::Base(WorkerState::StartWorking));
    }
    // :210-214 — une liste d'animaux vide `continue`, donc la boucle finit aussi sur
    // `:248`.
    if trade.animals.is_empty() {
        return Decision::go(WorkState::Base(WorkerState::StartWorking));
    }
    // :216 — `current_module = module`.
    memory.herd.has_current_module = true;
    // :218-225
    let breedable = trade
        .animals
        .iter()
        .filter(|animal| animal.is_breed_able())
        .count();
    // :227-228 — `> 1`, pas `>= 1`.
    let has_breeding_item = trade.breeding_item_count > 1;
    // :230-233
    if trade.draws.pickup < PICKUP_CHANCE && trade.items_on_ground > 0 {
        return Decision::go(WorkState::HerderPickup);
    }
    // :234-237
    let butcher = chance_to_butcher(trade.building_level, trade.breeding_setting, &trade.animals);
    if trade.draws.butcher < butcher {
        return Decision::go(WorkState::HerderButcher);
    }
    // :238-241 — quatre conditions, dont `breedTimeOut == 0`.
    if trade.breeding_setting
        && breedable >= NUM_OF_ANIMALS_TO_BREED
        && has_breeding_item
        && memory.herd.breed_time_out == 0
    {
        return Decision::go(WorkState::HerderBreed);
    }
    // :242-245
    if trade.draws.feed < FEED_CHANCE && has_breeding_item {
        return Decision::go(WorkState::HerderFeed);
    }
    // :248
    Decision::go(WorkState::Base(WorkerState::StartWorking))
}

/// `startWorkingAtOwnBuilding` (`:288-295`).
pub fn start_working_at_own_building(
    facts: &mut WorldFacts,
    _trade: &HerdFacts,
    _memory: &mut WorkMemory,
    _state: WorkState,
) -> Decision {
    // :290-293 — `!walkToBuilding()`.
    if !super::at_hut(facts) {
        return Decision::refuse(WorkRefusal::NotAtHut);
    }
    // :294
    Decision::go(WorkState::Base(WorkerState::Preparing))
}

/// `prepareForHerding` (`:302-328`).
pub fn prepare_for_herding(
    _facts: &mut WorldFacts,
    trade: &HerdFacts,
    memory: &mut WorkMemory,
    _state: WorkState,
) -> Decision {
    // :304-307
    if !memory.herd.has_current_module {
        return Decision::go(WorkState::Base(WorkerState::Decide));
    }
    // :309-315 — `getExtraToolsNeeded()` ne rend que la hache (`:176-181`), et
    // `checkForToolOrWeapon` vrai veut dire « la requête est partie, j'attends ».
    if !trade.has_axe {
        return Decision::refuse(WorkRefusal::NoAxe);
    }
    // :317-325 — les requêtes d'objets d'accouplement et d'objets supplémentaires, à
    // huit portions (`:319`). Créer une requête est `crate::requests` ; le port ne fait
    // que le dire, et `MANQUANT.md` §2 nomme ce qui manque.
    // :327
    Decision::go(WorkState::Base(WorkerState::Decide))
}

/// `butcherAnimals` (`:335-390`).
pub fn butcher_animals(
    facts: &mut WorldFacts,
    trade: &HerdFacts,
    memory: &mut WorkMemory,
    _state: WorkState,
) -> Decision {
    // :337-340
    if !memory.herd.has_current_module {
        return Decision::go(WorkState::Base(WorkerState::Decide));
    }
    // :344-347 — `!equipTool(MAIN_HAND, axe)` : `getToolSlot` a rendu `-1`, la
    // requête est partie (`:721-731`).
    if !trade.has_axe {
        return Decision::go_after(
            WorkState::Base(WorkerState::StartWorking),
            WorkRefusal::NoAxe,
        );
    }
    // :349-352
    if trade.animals.is_empty() {
        return Decision::go_after(WorkState::Base(WorkerState::Decide), WorkRefusal::NoAnimals);
    }
    // :354-375
    let Some(target) = butcher_target(&trade.animals) else {
        return Decision::go_after(
            WorkState::Base(WorkerState::Decide),
            WorkRefusal::NoButcherTarget,
        );
    };
    // :377 — `butcherAnimal(toKill)` ; l'appelant frappe. Pour l'éleveur de poulets, un
    // tirage décide même que le coup porte (`EntityAIWorkChickenHerder.java:67-71`).
    if walking_to_animal(facts, target) {
        return Decision::refuse(WorkRefusal::NotAtAnimal);
    }
    // :379-387 — l'animal est mort : la statistique, l'expérience, le compteur d'action
    // et l'oubli du nourrissage, puis `DECIDE`.
    if trade.killed {
        facts.inventory.actions_done += 1; // :383
        memory.herd.forget_fed(target.id); // :385
        return Decision::go(WorkState::Base(WorkerState::Decide));
    }
    // :389 — toujours vivant : on refrappe.
    Decision::go_after(WorkState::HerderButcher, WorkRefusal::AnimalSurvived)
}

/// `breedTwoAnimals` (`:639-665`) : rend `true` s'il reste des partenaires à joindre.
fn breed_two_animals(facts: &WorldFacts, trade: &HerdFacts, memory: &mut WorkMemory) -> bool {
    let queue = memory.herd.animals_to_breed.clone();
    for id in queue {
        let animal = trade.animals.iter().find(|known| known.id == id).copied();
        match animal {
            // L'entité n'est plus dans l'enclos : elle est morte (ou sortie), et Java
            // la retirerait par `isDeadOrDying` (`:644`). Le port la retire aussi.
            None => memory.herd.animals_to_breed.retain(|known| *known != id),
            // :644-647
            Some(animal) if animal.in_love || !animal.alive => {
                memory.herd.animals_to_breed.retain(|known| *known != id)
            }
            // :648-651 — encore en marche vers celui-là : on s'arrête là.
            Some(animal) if walking_to_animal(facts, animal) => break,
            // :652-662 — il l'a rejoint, il accouple, il retire.
            Some(_) => memory.herd.animals_to_breed.retain(|known| *known != id),
        }
    }
    // :664
    !memory.herd.animals_to_breed.is_empty()
}

/// `breedAnimals` (`:419-484`).
pub fn breed_animals(
    facts: &mut WorldFacts,
    trade: &HerdFacts,
    memory: &mut WorkMemory,
    _state: WorkState,
) -> Decision {
    // :421-425
    if !memory.herd.has_current_module {
        return Decision::go(WorkState::Base(WorkerState::Decide));
    }
    // :427-430 — il reste un partenaire à rejoindre : l'état ne change pas.
    if breed_two_animals(facts, trade, memory) {
        return Decision::refuse(WorkRefusal::BreedingInProgress);
    }
    // :432-433 — la liste filtrée par `isBreedAble`.
    let mut breedables: Vec<HerdAnimal> = trade
        .animals
        .iter()
        .filter(|animal| animal.is_breed_able())
        .copied()
        .collect();
    // :435-440 — moins de deux : 60 secondes de délai.
    if breedables.len() < NUM_OF_ANIMALS_TO_BREED {
        memory.herd.breed_time_out = BREED_TIMEOUT_LONG;
        return Decision::go_after(
            WorkState::Base(WorkerState::Decide),
            WorkRefusal::NotEnoughBreedables,
        );
    }
    // :442-445 — le plus **proche** du centre d'abord.
    let center = center_of_herd(&breedables);
    breedables.sort_by(|left, right| left.at.dist_sqr(center).cmp(&right.at.dist_sqr(center)));
    // :447
    let first = breedables.remove(0);
    // :449-457 — le premier à dix blocs ou moins qui accepte l'accouplement.
    let second = breedables.iter().find(|animal| {
        animal.distance_to(first) <= DISTANCE_TO_BREED && can_mate(first, **animal)
    });
    // :459-464 — aucun : 20 secondes de délai.
    let Some(second) = second.copied() else {
        memory.herd.breed_time_out = BREED_TIMEOUT_NO_MATE;
        return Decision::go_after(WorkState::Base(WorkerState::Decide), WorkRefusal::NoMate);
    };
    // :466-470 — il faut l'item d'accouplement en main.
    if !trade.breeding_item_equipped {
        return Decision::go_after(
            WorkState::Base(WorkerState::StartWorking),
            WorkRefusal::NoBreedingItem,
        );
    }
    // :473-474
    memory.herd.animals_to_breed.push(first.id);
    memory.herd.animals_to_breed.push(second.id);
    // :476-479 — les deux partenaires sont à rejoindre : l'état ne change pas.
    if breed_two_animals(facts, trade, memory) {
        return Decision::refuse(WorkRefusal::BreedingInProgress);
    }
    // :481-483 — les deux ont été accouplés d'un coup : 60 secondes et `IDLE`.
    memory.herd.breed_time_out = BREED_TIMEOUT_LONG;
    Decision::go(WorkState::Base(WorkerState::Idle))
}

/// `canMate(first, second)` (`:493-510`), réduit à la réponse.
///
/// Java pose `setInLoveTime(5)` sur les deux bêtes, demande `canMate`, puis restaure
/// (`:500-509`). Le port lit la réponse que l'appelant a mesurée
/// ([`HerdAnimal::can_mate`]) : la manipulation touche deux entités vivantes et n'est
/// pas rejouable.
fn can_mate(first: HerdAnimal, second: HerdAnimal) -> bool {
    // :495-498 — les deux doivent d'abord être accouplables.
    first.is_breed_able() && second.is_breed_able() && second.can_mate
}

/// `feedAnimal` (`:517-568`).
pub fn feed_animal(
    facts: &mut WorldFacts,
    trade: &HerdFacts,
    memory: &mut WorkMemory,
    _state: WorkState,
) -> Decision {
    // :519-522
    if !memory.herd.has_current_module {
        return Decision::go(WorkState::Base(WorkerState::Decide));
    }
    // :524-527
    if !trade.breeding_item_equipped {
        return Decision::go_after(
            WorkState::Base(WorkerState::StartWorking),
            WorkRefusal::NoBreedingItem,
        );
    }
    // :529-544 — le premier animal nourri il y a plus de cinq minutes ; `0` par défaut
    // pour le `getOrDefault`, donc un animal jamais nourri passe si `gameTime > 6000`.
    let to_feed = trade
        .animals
        .iter()
        .find(|animal| (trade.game_time.saturating_sub(memory.herd.fed_at(animal.id))) > FEED_COOLDOWN);
    let Some(to_feed) = to_feed.copied() else {
        return Decision::go_after(WorkState::Base(WorkerState::Decide), WorkRefusal::FeedNotDue);
    };
    // :546-547 — `!walkingToAnimal(toFeed)` : sinon, `:566-567` rend `getState()`.
    if walking_to_animal(facts, to_feed) {
        return Decision::refuse(WorkRefusal::NotAtAnimal);
    }
    // :548-551 — `toFeed.isBaby() && getSecondarySkillLevel() >= LIMIT_TO_FEED_CHILDREN`,
    // l'appelant fait grandir ; :554-561, il nourrit, décrémente la pile, compte
    // l'expérience et note l'heure.
    memory.herd.record_fed(to_feed.id, trade.game_time); // :561
    // :563
    Decision::go(WorkState::Base(WorkerState::Decide))
}

/// `pickupItems` (`:575-588`).
pub fn pickup_items(
    facts: &mut WorldFacts,
    trade: &HerdFacts,
    _memory: &mut WorkMemory,
    _state: WorkState,
) -> Decision {
    // :577-582 — des items par terre **et** il est arrivé : `return getState()`, et
    // rien n'est compté. Java ne compte pas le ramassage lui-même, qui est
    // `worker.setCanPickUpLoot(true)` (`:149`).
    if trade.items_on_ground > 0 {
        if let Some(item) = trade.first_item {
            if at_item(facts, item) {
                return Decision::stay();
            }
        }
    }
    // :584-587 — **tout le reste compte une action**, y compris la marche vers l'item :
    // `walkWithProxy` faux tombe dans `incrementActionsDone`, donc un ramassage qui
    // n'arrive pas vide quand même un cran du quota de vidage.
    facts.inventory.actions_done += 1;
    if trade.items_on_ground == 0 {
        return Decision::go_after(
            WorkState::Base(WorkerState::Decide),
            WorkRefusal::NothingToPickUp,
        );
    }
    // La marche continue mais l'état passe à `DECIDE` : `:587` est atteint dans tous
    // les cas sauf l'arrivée.
    Decision::go_after(WorkState::Base(WorkerState::Decide), WorkRefusal::NotAtItem)
}

herd_action!(decide_what_to_do_action, decide_what_to_do);
herd_action!(start_working_action, start_working_at_own_building);
herd_action!(prepare_for_herding_action, prepare_for_herding);
herd_action!(butcher_animals_action, butcher_animals);
herd_action!(breed_animals_action, breed_animals);
herd_action!(feed_animal_action, feed_animal);
herd_action!(pickup_items_action, pickup_items);

/// Enregistre les huit cibles d'`AbstractEntityAIHerder.java:140-147`.
///
/// `decide` est un paramètre et non une constante : `EntityAIWorkCowboy` surcharge
/// `decideWhatToDo` (`:96-120`) et le cowboy doit donc enregistrer **sa** version sur
/// `DECIDE`, exactement comme Java dont le `this::decideWhatToDo` du parent se résout
/// dynamiquement sur la sous-classe.
///
/// Les actions passées en paramètre ont la signature d'un fait d'élevage, pas d'un
/// fait de cow-boy : `cowboy::decide_action` lit donc `cowboy.herd` et appelle ce
/// fichier.
pub fn register(
    machine: &mut Machine<WorkState, super::WorkScene>,
    decide: fn(&mut super::WorkScene, WorkState) -> Option<WorkState>,
) {
    // :140 — `new AITarget(IDLE, START_WORKING, 1)`.
    machine.add(crate::ai::state::Transition::goto(
        WorkState::Base(WorkerState::Idle),
        WorkState::Base(WorkerState::StartWorking),
        "idle",
        "AbstractEntityAIHerder.java:140",
        1,
    ));
    // :141 — `new AITarget(START_WORKING, this::startWorkingAtOwnBuilding, TICKS_SECOND)`.
    super::add_target(
        machine,
        WorkState::Base(WorkerState::StartWorking),
        "startWorkingAtOwnBuilding",
        "AbstractEntityAIHerder.java:141",
        TICKS_SECOND,
        start_working_action,
    );
    // :142 — `new AITarget(PREPARING, this::prepareForHerding, TICKS_SECOND)`.
    super::add_target(
        machine,
        WorkState::Base(WorkerState::Preparing),
        "prepareForHerding",
        "AbstractEntityAIHerder.java:142",
        TICKS_SECOND,
        prepare_for_herding_action,
    );
    // :143 — `new AITarget(DECIDE, this::decideWhatToDo, DECIDING_DELAY)`.
    super::add_target(
        machine,
        WorkState::Base(WorkerState::Decide),
        "decideWhatToDo",
        "AbstractEntityAIHerder.java:143",
        DECIDING_DELAY,
        decide,
    );
    // :144 — `new AITarget(HERDER_BREED, this::breedAnimals, BREEDING_DELAY)`.
    super::add_target(
        machine,
        WorkState::HerderBreed,
        "breedAnimals",
        "AbstractEntityAIHerder.java:144",
        BREEDING_DELAY,
        breed_animals_action,
    );
    // :145 — `new AITarget(HERDER_BUTCHER, this::butcherAnimals, BUTCHER_DELAY)`.
    super::add_target(
        machine,
        WorkState::HerderButcher,
        "butcherAnimals",
        "AbstractEntityAIHerder.java:145",
        BUTCHER_DELAY,
        butcher_animals_action,
    );
    // :146 — `new AITarget(HERDER_PICKUP, this::pickupItems, TICKS_SECOND)`.
    super::add_target(
        machine,
        WorkState::HerderPickup,
        "pickupItems",
        "AbstractEntityAIHerder.java:146",
        TICKS_SECOND,
        pickup_items_action,
    );
    // :147 — `new AITarget(HERDER_FEED, this::feedAnimal, TICKS_SECOND)`.
    super::add_target(
        machine,
        WorkState::HerderFeed,
        "feedAnimal",
        "AbstractEntityAIHerder.java:147",
        TICKS_SECOND,
        feed_animal_action,
    );
}

/// `getActionsDoneUntilDumping` (`:152-156`).
pub fn actions_until_dumping() -> u32 {
    ACTIONS_FOR_DUMP
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn herd(animals: Vec<HerdAnimal>) -> HerdFacts {
        HerdFacts {
            building_level: 1,
            breeding_setting: true,
            has_module: true,
            animals,
            items_on_ground: 0,
            first_item: None,
            has_axe: true,
            breeding_item_count: 4,
            breeding_item_equipped: true,
            killed: false,
            chicken_draw: 0,
            game_time: 0,
            primary_skill: 10,
            secondary_skill: 10,
            main_hand_empty: false,
            // Les trois tirages au-dessus de leur seuil : par défaut, ce troupeau ne
            // déclenche ni ramassage (`:230`, < 0,1), ni abattage (`:234`), ni
            // nourrissage (`:242`, < 0,1). Chaque test qui veut un tirage le pose.
            draws: crate::ai_work_09::facts::HerderDraws {
                pickup: 0.9,
                butcher: 0.9,
                feed: 0.9,
            },
        }
    }

    fn world_at_hut() -> WorldFacts {
        let mut facts = WorldFacts::new();
        facts.building.position = Pos::new(0, 64, 0);
        facts.worker.at = Pos::new(0, 64, 0);
        facts
    }

    #[test]
    fn the_eight_cadences_are_java_s_four_constants() {
        // `AbstractEntityAIHerder.java:81-83`, `:82` réutilisé par `:205`.
        assert_eq!(BUTCHER_DELAY, 20);
        assert_eq!(DECIDING_DELAY, 80);
        assert_eq!(BREEDING_DELAY, 40);
        assert_eq!(TICKS_SECOND, 20);
        assert_eq!(ACTIONS_FOR_DUMP, 10);
        assert_eq!(actions_until_dumping(), 10, "`:153-156`");
        // Les délais et la fenêtre de nourrissage.
        assert_eq!(BREED_TIMEOUT_NO_MATE, 400, "TICKS_SECOND * 20");
        assert_eq!(BREED_TIMEOUT_LONG, 1200, "TICKS_SECOND * 60");
        assert_eq!(FEED_COOLDOWN, 6000, "TICKS_SECOND * 60 * 5");
        // `MAX_ENTITY_AGE` est négatif comme `AgeableMob.BABY_START_AGE`.
        assert_eq!(MAX_ENTITY_AGE, -24000.0);
    }

    #[test]
    fn the_herd_centre_is_the_average_floored_the_way_blockpos_containing_does() {
        // `getCenterOfHerd` (`:398-412`) : moyenne puis `BlockPos.containing`.
        let two = vec![animal(1, Pos::new(0, 64, 0)), animal(2, Pos::new(3, 64, 0))];
        assert_eq!(center_of_herd(&two), Pos::new(1, 64, 0), "1,5 tronqué vers le bas");
        assert_eq!(center_of_herd(&[]), Pos::new(0, 0, 0), "`:400-403`");
    }

    #[test]
    fn a_small_herd_is_never_butchered() {
        // `:690-693` — trois adultes ou moins rendent 0.
        let adults = vec![
            animal(1, Pos::new(0, 64, 0)),
            animal(2, Pos::new(1, 64, 0)),
            animal(3, Pos::new(2, 64, 0)),
        ];
        assert_eq!(chance_to_butcher(5, true, &adults), 0.0);
        // Un bébé ne compte pas comme adulte.
        let mut mixed = adults.clone();
        mixed.push(HerdAnimal {
            baby: true,
            age: -600,
            ..animal(4, Pos::new(3, 64, 0))
        });
        assert_eq!(chance_to_butcher(5, true, &mixed), 0.0, "toujours trois adultes");
    }

    #[test]
    fn the_butcher_chance_grows_with_the_fourth_power_and_can_be_infinite() {
        // `:695` — `0.5 * (grownUp⁴ / maxAnimals⁴)`.
        let four = vec![
            animal(1, Pos::new(0, 64, 0)),
            animal(2, Pos::new(1, 64, 0)),
            animal(3, Pos::new(2, 64, 0)),
            animal(4, Pos::new(3, 64, 0)),
        ];
        // Niveau 1 → maxAnimals = 2 ; 4⁴ / 2⁴ = 256 / 16 = 16 ; 0,5 * 16 = 8.
        assert_eq!(chance_to_butcher(1, true, &four), 8.0);
        // Niveau 5 → maxAnimals = 10 ; 4⁴ / 10⁴ = 0,0256 ; 0,5 * 0,0256 = 0,0128.
        assert!((chance_to_butcher(5, true, &four) - 0.0128).abs() < 1e-12);
        // Niveau 0 → division par zéro : `Infinity`, comme le Java.
        assert!(chance_to_butcher(0, true, &four).is_infinite());
        // Sans le réglage d'accouplement, un troupeau dans la limite n'est pas abattu
        // (`:676-679`).
        assert_eq!(chance_to_butcher(5, false, &four), 0.0, "4 <= 10");
    }

    #[test]
    fn the_butcher_takes_the_furthest_adult_that_cannot_see_the_sky() {
        // `:354-375`.
        let animals = vec![
            animal(1, Pos::new(0, 64, 0)),
            animal(2, Pos::new(6, 64, 0)),
            HerdAnimal {
                can_see_sky: false,
                ..animal(3, Pos::new(3, 64, 0))
            },
            HerdAnimal {
                baby: true,
                age: -600,
                ..animal(4, Pos::new(9, 64, 0))
            },
            HerdAnimal {
                in_love: true,
                ..animal(5, Pos::new(12, 64, 0))
            },
        ];
        let target = butcher_target(&animals).expect("une cible");
        // Le plus loin est le n°5 (amoureux, écarté) puis le bébé n°4 (écarté) : la
        // première cible est le n°2, mais le n°3 ne voit pas le ciel et le remplace
        // (`:365`).
        assert_eq!(target.id, 3);
    }

    #[test]
    fn a_herd_of_babies_and_lovers_has_no_butcher_target() {
        // `:361-375`.
        let animals = vec![
            HerdAnimal {
                baby: true,
                age: -600,
                ..animal(1, Pos::new(0, 64, 0))
            },
            HerdAnimal {
                in_love: true,
                ..animal(2, Pos::new(1, 64, 0))
            },
        ];
        assert_eq!(butcher_target(&animals), None);
    }

    #[test]
    fn the_chicken_butcher_draw_is_gated_by_the_secondary_skill() {
        // `EntityAIWorkChickenHerder.java:67` — `nextInt(1 + (100 - skill) / 5) <= 1`.
        assert_eq!(chicken_hit_bound(0), 21);
        assert_eq!(chicken_hit_bound(10), 19);
        assert_eq!(chicken_hit_bound(100), 1);
        // À 100, la seule valeur possible est 0, donc le coup porte toujours.
        assert!(chicken_hit_lands(100, 0));
        // À 0, seuls 0 et 1 portent.
        assert!(chicken_hit_lands(0, 0));
        assert!(chicken_hit_lands(0, 1));
        assert!(!chicken_hit_lands(0, 2));
        // Un tirage hors bornes ne porte pas, au lieu de lever dans le jeu.
        assert!(!chicken_hit_lands(100, 5));
    }

    #[test]
    fn picking_a_state_needs_a_module_and_at_least_one_animal() {
        // `:208-217` puis `:248`.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        let no_module = HerdFacts {
            has_module: false,
            ..herd(vec![animal(1, Pos::new(0, 64, 0))])
        };
        assert_eq!(
            decide_what_to_do(&mut facts, &no_module, &mut memory, WorkState::Base(WorkerState::Decide))
                .next,
            Some(WorkState::Base(WorkerState::StartWorking))
        );
        let empty = herd(Vec::new());
        assert_eq!(
            decide_what_to_do(&mut facts, &empty, &mut memory, WorkState::Base(WorkerState::Decide)).next,
            Some(WorkState::Base(WorkerState::StartWorking))
        );
        assert!(!memory.herd.has_current_module, "`:216` n'a pas tourné");
    }

    #[test]
    fn the_pickup_draw_wins_before_the_butcher_one() {
        // `:230-237` — le premier `if` de la chaîne gagne.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        // Un troupeau de dix adultes au niveau 0 rend `chanceToButcher` infini, donc
        // l'abattage gagnerait toujours s'il était testé en premier.
        let mut facts_herd = herd((0..10).map(|id| animal(id, Pos::new(id as i32, 64, 0))).collect());
        facts_herd.building_level = 0;
        facts_herd.items_on_ground = 3;
        facts_herd.draws.pickup = 0.05;
        facts_herd.draws.butcher = 0.0;
        assert_eq!(
            decide_what_to_do(&mut facts, &facts_herd, &mut memory, WorkState::Base(WorkerState::Decide))
                .next,
            Some(WorkState::HerderPickup)
        );
        // Le même sans item par terre : l'abattage gagne.
        facts_herd.items_on_ground = 0;
        assert_eq!(
            decide_what_to_do(&mut facts, &facts_herd, &mut memory, WorkState::Base(WorkerState::Decide))
                .next,
            Some(WorkState::HerderButcher)
        );
    }

    #[test]
    fn breeding_is_a_conjunction_with_no_draw_and_a_timeout_blocks_it() {
        // `:238-241` puis `:242-245`.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        let mut ready = herd(vec![
            animal(1, Pos::new(0, 64, 0)),
            animal(2, Pos::new(1, 64, 0)),
        ]);
        // Les trois tirages hauts : ni ramassage, ni abattage, ni nourrissage.
        ready.draws.pickup = 0.9;
        ready.draws.butcher = 0.9;
        ready.draws.feed = 0.9;
        assert_eq!(
            decide_what_to_do(&mut facts, &ready, &mut memory, WorkState::Base(WorkerState::Decide)).next,
            Some(WorkState::HerderBreed)
        );
        // Un item d'accouplement unique ne suffit pas : le Java teste `> 1`.
        let one = HerdFacts {
            breeding_item_count: 1,
            ..ready.clone()
        };
        assert_eq!(
            decide_what_to_do(&mut facts, &one, &mut memory, WorkState::Base(WorkerState::Decide)).next,
            Some(WorkState::Base(WorkerState::StartWorking))
        );
        // Un seul animal accouplable : pas d'accouplement non plus.
        let lonely = herd(vec![animal(1, Pos::new(0, 64, 0))]);
        assert_eq!(
            decide_what_to_do(&mut facts, &lonely, &mut memory, WorkState::Base(WorkerState::Decide)).next,
            Some(WorkState::Base(WorkerState::StartWorking))
        );
        // Et un délai en cours l'empêche, même avec tout le reste.
        memory.herd.breed_time_out = 400;
        assert_eq!(
            decide_what_to_do(&mut facts, &ready, &mut memory, WorkState::Base(WorkerState::Decide)).next,
            Some(WorkState::Base(WorkerState::StartWorking))
        );
        assert_eq!(memory.herd.breed_time_out, 320, "`:205` retire DECIDING_DELAY");
    }

    #[test]
    fn the_feed_draw_is_the_last_resort() {
        // `:242-245`.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        // Un seul animal : pas d'accouplement possible, donc le nourrissage est atteint.
        let mut facts_herd = herd(vec![animal(1, Pos::new(0, 64, 0))]);
        facts_herd.draws.pickup = 0.9;
        facts_herd.draws.butcher = 0.9;
        facts_herd.draws.feed = 0.05;
        assert_eq!(
            decide_what_to_do(&mut facts, &facts_herd, &mut memory, WorkState::Base(WorkerState::Decide))
                .next,
            Some(WorkState::HerderFeed)
        );
        facts_herd.draws.feed = 0.5;
        assert_eq!(
            decide_what_to_do(&mut facts, &facts_herd, &mut memory, WorkState::Base(WorkerState::Decide))
                .next,
            Some(WorkState::Base(WorkerState::StartWorking))
        );
    }

    #[test]
    fn preparing_asks_for_an_axe_before_anything_else() {
        // `:304-315`.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        // Sans module courant, `prepareForHerding` rend `DECIDE` sans rien demander
        // (`:304-307`).
        assert_eq!(
            prepare_for_herding(&mut facts, &herd(Vec::new()), &mut memory, WorkState::Base(WorkerState::Preparing))
                .next,
            Some(WorkState::Base(WorkerState::Decide))
        );
        memory.herd.has_current_module = true;
        let no_axe = HerdFacts {
            has_axe: false,
            ..herd(Vec::new())
        };
        let decision = prepare_for_herding(&mut facts, &no_axe, &mut memory, WorkState::Base(WorkerState::Preparing));
        assert_eq!(decision.next, None, "getState()");
        assert_eq!(decision.refusal, Some(WorkRefusal::NoAxe));
    }

    #[test]
    fn butchering_needs_an_axe_and_a_target() {
        // `:344-352`.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        memory.herd.has_current_module = true;
        let no_axe = HerdFacts {
            has_axe: false,
            ..herd(vec![animal(1, Pos::new(0, 64, 0))])
        };
        let decision = butcher_animals(&mut facts, &no_axe, &mut memory, WorkState::HerderButcher);
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::StartWorking)));
        assert_eq!(decision.refusal, Some(WorkRefusal::NoAxe));
        let empty = herd(Vec::new());
        let decision = butcher_animals(&mut facts, &empty, &mut memory, WorkState::HerderButcher);
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::Decide)));
        assert_eq!(decision.refusal, Some(WorkRefusal::NoAnimals));
    }

    #[test]
    fn a_butchered_animal_forgets_that_it_was_fed() {
        // `:379-387` — `fedRecently.remove(toKill.getUUID())`.
        let mut facts = world_at_hut();
        // Le citoyen est près du n°1 (le plus loin du centre, donc la cible) ; le n°2 est
        // plus près du centre et ne le remplace pas, les deux voyant le ciel.
        facts.worker.at = Pos::new(5, 64, 0);
        let mut memory = WorkMemory::default();
        memory.herd.has_current_module = true;
        memory.herd.record_fed(1, 500);
        let mut facts_herd = herd(vec![
            animal(1, Pos::new(5, 64, 0)),
            animal(2, Pos::new(0, 64, 0)),
        ]);
        facts_herd.killed = true;
        let decision = butcher_animals(&mut facts, &facts_herd, &mut memory, WorkState::HerderButcher);
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::Decide)));
        assert_eq!(memory.herd.fed_at(1), 0, "`:385`");
        assert_eq!(facts.inventory.actions_done, 1, "`:383`");
    }

    #[test]
    fn a_swing_that_does_not_kill_stays_on_the_butcher_state() {
        // `:389` puis `EntityAIWorkChickenHerder.java:67-71` : le tirage a décidé que
        // le coup ne portait pas.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        memory.herd.has_current_module = true;
        let mut facts_herd = herd(vec![animal(1, Pos::new(0, 64, 0))]);
        facts_herd.killed = false;
        let decision = butcher_animals(&mut facts, &facts_herd, &mut memory, WorkState::HerderButcher);
        assert_eq!(decision.next, Some(WorkState::HerderButcher));
        assert_eq!(decision.refusal, Some(WorkRefusal::AnimalSurvived));
        // Et le tirage qui ne porte pas, à 0 de compétence secondaire, est le cas
        // courant : 19 valeurs sur 21.
        assert!(!chicken_hit_lands(0, 5));
    }

    #[test]
    fn an_unreachable_animal_keeps_the_herder_walking() {
        // `:622-632` puis `:377-389`.
        let mut facts = world_at_hut();
        facts.worker.at = Pos::new(30, 64, 30);
        let mut memory = WorkMemory::default();
        memory.herd.has_current_module = true;
        let mut facts_herd = herd(vec![animal(1, Pos::new(0, 64, 0))]);
        facts_herd.killed = true;
        let decision = butcher_animals(&mut facts, &facts_herd, &mut memory, WorkState::HerderButcher);
        assert_eq!(decision.next, None);
        assert_eq!(decision.refusal, Some(WorkRefusal::NotAtAnimal));
        assert_eq!(facts.inventory.actions_done, 0, "rien n'est compté");
    }

    #[test]
    fn breeding_with_fewer_than_two_breedables_sets_a_minute_of_delay() {
        // `:435-440`.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        memory.herd.has_current_module = true;
        let decision = breed_animals(
            &mut facts,
            &herd(vec![animal(1, Pos::new(0, 64, 0))]),
            &mut memory,
            WorkState::HerderBreed,
        );
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::Decide)));
        assert_eq!(decision.refusal, Some(WorkRefusal::NotEnoughBreedables));
        assert_eq!(memory.herd.breed_time_out, 1200);
    }

    #[test]
    fn breeding_with_no_partner_in_range_sets_twenty_seconds() {
        // `:449-464` — `DISTANCE_TO_BREED` est une distance de dix blocs.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        memory.herd.has_current_module = true;
        let far = herd(vec![
            animal(1, Pos::new(0, 64, 0)),
            animal(2, Pos::new(40, 64, 0)),
        ]);
        let decision = breed_animals(&mut facts, &far, &mut memory, WorkState::HerderBreed);
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::Decide)));
        assert_eq!(decision.refusal, Some(WorkRefusal::NoMate));
        assert_eq!(memory.herd.breed_time_out, 400);
    }

    #[test]
    fn breeding_two_close_animals_queues_both_and_then_idles() {
        // `:466-483` : les deux partenaires sont à portée, donc `breedTwoAnimals` les
        // consomme dans le même tick et rend `IDLE`.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        memory.herd.has_current_module = true;
        let close = herd(vec![
            animal(1, Pos::new(0, 64, 0)),
            animal(2, Pos::new(1, 64, 0)),
        ]);
        let decision = breed_animals(&mut facts, &close, &mut memory, WorkState::HerderBreed);
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::Idle)), "`:483`");
        assert_eq!(memory.herd.breed_time_out, 1200);
        assert!(memory.herd.animals_to_breed.is_empty(), "les deux sont faits");
    }

    #[test]
    fn breeding_two_distant_animals_keeps_the_state() {
        // `:476-479` — au moins un partenaire reste à rejoindre.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        memory.herd.has_current_module = true;
        let split = herd(vec![
            animal(1, Pos::new(0, 64, 0)),
            animal(2, Pos::new(8, 64, 0)),
        ]);
        let decision = breed_animals(&mut facts, &split, &mut memory, WorkState::HerderBreed);
        assert_eq!(decision.next, None, "getState()");
        assert_eq!(decision.refusal, Some(WorkRefusal::BreedingInProgress));
        // Le premier a été accouplé dans le même tick (`:652-662`), le second est
        // encore à rejoindre : la file garde le second seulement.
        assert_eq!(memory.herd.animals_to_breed, vec![2]);
    }

    #[test]
    fn feeding_needs_the_item_and_a_five_minute_gap() {
        // `:524-544`.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        memory.herd.has_current_module = true;
        let mut facts_herd = herd(vec![animal(1, Pos::new(0, 64, 0))]);
        facts_herd.game_time = 7000;
        // Jamais nourri : `getOrDefault` rend 0, et 7000 > 6000.
        let decision = feed_animal(&mut facts, &facts_herd, &mut memory, WorkState::HerderFeed);
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::Decide)));
        assert_eq!(memory.herd.fed_at(1), 7000, "`:561`");
        // Nourri il y a une seconde : plus rien à faire.
        facts_herd.game_time = 7020;
        let decision = feed_animal(&mut facts, &facts_herd, &mut memory, WorkState::HerderFeed);
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::Decide)));
        assert_eq!(decision.refusal, Some(WorkRefusal::FeedNotDue));
        // Sans item d'accouplement, il repart travailler.
        facts_herd.game_time = 14000;
        let no_item = HerdFacts {
            breeding_item_equipped: false,
            ..facts_herd
        };
        let decision = feed_animal(&mut facts, &no_item, &mut memory, WorkState::HerderFeed);
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::StartWorking)));
        assert_eq!(decision.refusal, Some(WorkRefusal::NoBreedingItem));
    }

    #[test]
    fn picking_up_counts_an_action_even_while_walking_to_the_item() {
        // `:577-587` : seule l'arrivée rend `getState()` **sans** compter.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        // Un item loin : il marche, et la marche compte quand même un cran.
        let mut far = herd(Vec::new());
        far.items_on_ground = 1;
        far.first_item = Some(Pos::new(20, 64, 20));
        let decision = pickup_items(&mut facts, &far, &mut memory, WorkState::HerderPickup);
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::Decide)), "`:587`");
        assert_eq!(decision.refusal, Some(WorkRefusal::NotAtItem));
        assert_eq!(facts.inventory.actions_done, 1, "`:584`");
        // Un item sur place : il ramasse et ne compte rien.
        far.first_item = Some(Pos::new(0, 64, 0));
        let decision = pickup_items(&mut facts, &far, &mut memory, WorkState::HerderPickup);
        assert_eq!(decision.next, None, "`:581` getState()");
        assert_eq!(facts.inventory.actions_done, 1, "l'arrivée ne compte pas");
        // Rien par terre : il compte et passe son chemin.
        let nothing = herd(Vec::new());
        let decision = pickup_items(&mut facts, &nothing, &mut memory, WorkState::HerderPickup);
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::Decide)));
        assert_eq!(decision.refusal, Some(WorkRefusal::NothingToPickUp));
        assert_eq!(facts.inventory.actions_done, 2);
    }

    #[test]
    fn the_herder_walks_to_its_hut_before_preparing() {
        // `:288-295`.
        let mut facts = world_at_hut();
        let mut memory = WorkMemory::default();
        assert_eq!(
            start_working_at_own_building(&mut facts, &herd(Vec::new()), &mut memory, WorkState::Base(WorkerState::StartWorking)).next,
            Some(WorkState::Base(WorkerState::Preparing))
        );
        facts.worker.at = Pos::new(40, 64, 40);
        let decision =
            start_working_at_own_building(&mut facts, &herd(Vec::new()), &mut memory, WorkState::Base(WorkerState::StartWorking));
        assert_eq!(decision.next, None);
        assert_eq!(decision.refusal, Some(WorkRefusal::NotAtHut));
    }
}
