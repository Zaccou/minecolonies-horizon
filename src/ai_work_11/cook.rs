//! Le cuisinier : servir les citoyens, servir les joueurs, et les six cibles de four
//! qu'il hérite.
//!
//! Java référence : `core/entity/ai/workers/service/EntityAIWorkCook.java` (438 lignes),
//! sa classe mère `core/entity/ai/workers/AbstractEntityAIUsesFurnace.java` (507) et les
//! quatre utilitaires qu'il interroge — `api/util/FoodUtils.java`,
//! `api/util/InventoryUtils.java` (`transferFoodUpToSaturation`, `:3222-3282`),
//! `api/util/constant/CitizenConstants.java` et `api/colony/ICitizenData.java`
//! (`MAX_SATURATION`).
//!
//! ## La table, et ce qu'elle est
//!
//! | # | État | Règle | Cadence | Java |
//! | ---: | --- | --- | ---: | --- |
//! | 1 | `IDLE` | `START_WORKING` (constante) | 5 | `AbstractEntityAIUsesFurnace.java:77` |
//! | 2 | `START_WORKING` | `startWorking` | 60 | `:78` |
//! | 3 | `FILL_UP_FURNACES` | `fillUpFurnace` | 5 | `:79` |
//! | 4 | `AI_BLOCKING` | `accelerateFurnaces` (rend `null`) | 20 | `:80` |
//! | 5 | `RETRIEVING_END_PRODUCT_FROM_FURNACE` | `retrieveSmeltableFromFurnace` | 5 | `:81` |
//! | 6 | `RETRIEVING_USED_FUEL_FROM_FURNACE` | `retrieveUsedFuel` | 5 | `:82` |
//! | 7 | `COOK_SERVE_FOOD_TO_CITIZEN` | `serveFoodToCitizen` | 30 | `EntityAIWorkCook.java:96` |
//! | 8 | `COOK_SERVE_FOOD_TO_PLAYER` | `serveFoodToPlayer` | 30 | `:97` |
//!
//! **Le point d'entrée est `START_WORKING`, pas `IDLE`.** Le cuisinier est un
//! utilisateur de four : son `startWorking` vérifie d'abord s'il a une tâche plus
//! importante (`checkForImportantJobs`, `:338-412`), et c'est là que les deux états du
//! métier se décident. Les six autres règles sont celles du parent, **déjà portées** par
//! `crate::ai_work_04::smelter` : ce fichier les appelle (`fill_up_furnace`,
//! `retrieve_product`, `retrieve_used_fuel`, `check_if_able_to_smelt`,
//! `oven_to_retrieve_from`, `oven_to_retrieve_fuel_from`, `reached_max_to_keep`) au lieu
//! de les réécrire, et un test vérifie que les deux tables répondent pareil sur les
//! mêmes faits.
//!
//! **`startWorking` est la seule règle réécrite, et elle le doit.** Le parent
//! (`:182-261`) appelle `checkForImportantJobs()` au milieu de son corps (`:210-214`) ;
//! le cuisinier surcharge cette méthode, donc `crate::ai_work_04::smelter::start_working`
//! — qui code la version du mineur — ne peut pas servir : elle renverrait
//! `BREAK_ORES`. Les sept étapes du parent sont ici dans son ordre, mais chaque analyse
//! (les deux scans de four, les quatre comptes, `checkIfAbleToSmelt`) est un appel au
//! module voisin ; ce qui est écrit ici est la **séquence**, pas les règles.
//!
//! ## Les trois formules qui décident d'une quantité
//!
//! 1. **La portion transférée** (`:236-239`) : `ceil(max(1, (60 − saturation) /
//!    valeur) × 1,5)`, avec la troncature en `int` **avant** la multiplication — un
//!    `(int)` et un `Math.ceil` dans cet ordre, ce n'est pas la même chose.
//! 2. **Les items consommés sur place** quand l'inventaire du citoyen est plein
//!    (`:192-211`) : au plus dix tentatives, une par item, arrêt dès
//!    `FULL_SATURATION`.
//! 3. **Le transfert à un joueur** (`InventoryUtils.java:3222-3282`) :
//!    `round(ceil((16 × niveau − déjà trouvé) / nutrition))` par pile, jusqu'à saturation
//!    de la cible.
//!
//! ## Ce que le Java fait et qu'on n'attend pas
//!
//! 1. **Le cuisinier ne nourrit pas les citoyens qui travaillent.** `shouldBeFed`
//!    exige `!isWorking()` (`:422`) : un citoyen à son poste avec 2 de saturation
//!    attend la fin de sa journée. Copié.
//! 2. **La file n'est jamais vidée au début de `checkForImportantJobs`** : `:379` ajoute
//!    sans tester les doublons, donc un citoyen encore en file au tick suivant y entre
//!    deux fois et peut être servi deux fois. Copié — voir `MANQUANT.md` §3.
//! 3. **Le menu vide n'arrête rien.** `requestSmeltable` (`:149-157`) lève une
//!    interaction et ne change aucun état : le cuisinier continue de cuisiner ce qu'il a.
//!    Copié, et c'est pourquoi la plainte est un booléen de [`Start`] et non un refus.
//! 4. **`POOR_MENU_INTERACTION` ne dépend pas de la faim de qui que ce soit** : au
//!    niveau 3, il suffit que le menu n'ait aucun plat MineColonies (`:350-366`).
//! 5. **La comparaison du logement est `>` et non `>=`** (`:231`) : un citoyen logé un
//!    niveau au-dessus du restaurant ne se plaint pas.

use crate::ai::facts::Pos;
use crate::ai::state::{Blocking, Machine};
use crate::ai_work_04::facts::SmelterFacts;
use crate::ai_work_04::smelter::{self, Fill, Retrieve};
use crate::ai_work_04::states::SmeltState;

use super::facts::{CitizenHere, CookFacts, CookMemory, FoodStack, Scene, TradeJob};
use super::state::{CookState, Refusal, TradeState};
use super::{add_target, Decision, Effect, STACKSIZE, STANDARD_DELAY, TICKS_SECOND};

/// `EntityAIWorkCook.SATURATION_TO_SERVE` (`:59`) : la saturation offerte à un joueur
/// par niveau de hutte (`:288`).
pub const SATURATION_TO_SERVE: u32 = 16;

/// `EntityAIWorkCook.SERVE_DELAY` (`:64`) : la cadence des deux cibles de service
/// (`:96`, `:97`).
pub const SERVE_DELAY: u32 = 30;

/// `EntityAIWorkCook.LEVEL_TO_FEED_PLAYER` (`:69`) : un joueur n'est servi que sous ce
/// niveau de faim (`:343`).
pub const LEVEL_TO_FEED_PLAYER: u32 = 10;

/// Le `10` écrit en clair dans `:192` : le nombre d'items qu'un citoyen au sac plein peut
/// consommer en une visite.
pub const FEEDING_ATTEMPTS: u32 = 10;

/// Le `1.5` écrit en clair dans `:239` : « hand out a bit extra ».
pub const PORTION_FACTOR: f64 = 1.5;

/// `CitizenConstants.AVERAGE_SATURATION` (`CitizenConstants.java:43`) : le seuil sous
/// lequel un citoyen est nourri (`:423`).
pub const AVERAGE_SATURATION: f64 = 10.0;

/// `ICitizenData.MAX_SATURATION` (`ICitizenData.java:30`), c'est-à-dire
/// `CitizenConstants.FULL_SATURATION` (`:53`), la cible des deux formules de portion
/// (`:207`, `:237`).
pub const FULL_SATURATION: f64 = 60.0;

/// `Constants.SLOT_PER_LINE` (`Constants.java:163`) : la limite de stockage de nourriture
/// est un nombre de piles, converti en items par ce facteur (`:143`).
pub const SLOT_PER_LINE: u32 = 9;

/// `AbstractEntityAIUsesFurnace.BASE_XP_GAIN` (`:55`) : l'expérience d'une sortie de
/// four (`EntityAIWorkCook.java:119`).
pub const BASE_XP_GAIN: f64 = 2.0;

/// `EntityAIWorkCook.getActionsDoneUntilDumping` (`:427-431`) : une action, puis on vide.
pub const ACTIONS_UNTIL_DUMP: u32 = 1;

/// `AbstractEntityAIUsesFurnace.java:78` : la cadence de `START_WORKING`, plus lente que
/// les autres parce qu'elle balaie les fours.
pub const START_WORKING_RATE: u32 = 60;

/// `EntityAIWorkCook.java:350` : le niveau de hutte à partir duquel le menu doit
/// contenir un plat MineColonies.
pub const LEVEL_TO_CHECK_MENU: u32 = 3;

/// `isSmeltable` (`EntityAIWorkCook.java:130-134`) : `ISCOOKABLE.test(stack)` **et** le
/// résultat de la cuisson est dans le menu du restaurant. Un cuisinier ne cuit donc pas
/// ce que personne n'a commandé.
pub fn is_smeltable(cookable: bool, menu_has_cooked_result: bool) -> bool {
    cookable && menu_has_cooked_result
}

/// La limite de nourriture gardée en hutte (`:143`) : `max(1, niveau²) × 9` items.
pub fn max_food_to_keep(building_level: u32) -> u32 {
    let squared = building_level.saturating_mul(building_level);
    squared.max(1).saturating_mul(SLOT_PER_LINE)
}

/// `reachedMaxToKeep` (`EntityAIWorkCook.java:136-147`).
///
/// Le parent dit « la hutte est pleine à trois piles près »
/// (`AbstractEntityAIUsesFurnace.java:117-121`, réutilisé tel quel) ; le cuisinier ajoute
/// « elle garde déjà plus de nourriture que sa limite ». Les deux conditions sont un
/// `||`, dans cet ordre (`:139-146`).
pub fn reached_max_to_keep(facts: &CookFacts) -> bool {
    smelter::reached_max_to_keep(facts.furnace.empty_slots_in_building)
        || facts.food_kept > max_food_to_keep(facts.building_level)
}

/// Le `>`, et non `>=`, de `:146` : la limite atteinte n'est pas la limite dépassée.
pub fn food_kept_over_limit(building_level: u32, food_kept: u32) -> bool {
    food_kept > max_food_to_keep(building_level)
}

/// Le seuil de faim d'un joueur (`:343`) : `getFoodLevel() < LEVEL_TO_FEED_PLAYER`.
/// La permission `Action.MANAGE_HUTS` du même prédicat est un fait que l'appelant a déjà
/// filtré ([`super::facts::CookFacts::players`]).
pub fn player_is_hungry(food_level: u32) -> bool {
    food_level < LEVEL_TO_FEED_PLAYER
}

/// `shouldBeFed` (`EntityAIWorkCook.java:419-425`) : pas en train de travailler, sous la
/// moyenne, et pas déjà repu de la seconde.
///
/// Le `citizenData != null` de `:421` n'est pas un fait : l'appelant ne rapporte pas un
/// citoyen sans données (`EntityAIWorkCook.java:368` les prend chez `EntityCitizen`, qui
/// en a toujours).
pub fn should_be_fed(citizen: &CitizenHere) -> bool {
    !citizen.working && citizen.saturation <= AVERAGE_SATURATION && !citizen.just_ate
}

/// La comparaison de `:231` : `homeBuilding.getBuildingLevelEquivalent() >
/// building.getBuildingLevel() + 1`.
pub fn home_is_too_fancy(home_level: u32, building_level: u32) -> bool {
    home_level > building_level.saturating_add(1)
}

/// `qty = (int) Math.ceil((int) Math.max(1.0, (FULL − saturation) / valeur) × 1.5)`
/// (`EntityAIWorkCook.java:236-239`).
///
/// **L'ordre des deux conversions est le Java.** Le premier `(int)` tronque vers le bas
/// avant que le `1,5` ne s'applique : à saturation 55 avec un aliment qui vaut 5,
/// `(60 − 55) / 5 = 1`, `max(1, 1) = 1`, puis `ceil(1 × 1,5) = 2`. Sans la troncature on
/// aurait `ceil(1,5) = 2` aussi — mais à saturation 50, `10 / 5 = 2`, donc 3 items ; par
/// le calcul direct ce serait aussi 3. La différence apparaît sur les valeurs non
/// entières : `(60 − 51) / 4 = 2,25`, tronqué à 2, puis `ceil(3,0) = 3` — contre
/// `ceil(3,375) = 4` sans troncature.
pub fn portion_for(saturation: f64, food_value: f64) -> u32 {
    if food_value <= 0.0 {
        // Une valeur nulle rendrait la division infinie ; Java prendrait
        // `(int) Double.POSITIVE_INFINITY`, qui vaut `Integer.MAX_VALUE`. Le port refuse
        // de propager un fait cassé : la portion minimale est le seul choix sûr.
        return 1;
    }
    let wanted = ((FULL_SATURATION - saturation) / food_value).max(1.0);
    let truncated = wanted.trunc();
    (truncated * PORTION_FACTOR).ceil() as u32
}

/// Les items qu'un citoyen au sac plein consomme sur place
/// (`EntityAIWorkCook.java:192-211`).
///
/// La boucle Java s'arrête après `FEEDING_ATTEMPTS` tours (`:192`) **ou** dès que
/// `getSaturation() >= FULL_SATURATION` (`:207-210`) ; le premier tour a toujours lieu
/// quand un plat existe (`:194-201`). Un aliment de valeur nulle ne fait jamais monter la
/// saturation : Java consomme donc les dix items, et le port aussi.
pub fn feed_items(saturation: f64, food_value: f64) -> u32 {
    if food_value <= 0.0 {
        return FEEDING_ATTEMPTS;
    }
    let needed = ((FULL_SATURATION - saturation) / food_value).ceil();
    if needed < 1.0 {
        return 1;
    }
    if needed > f64::from(FEEDING_ATTEMPTS) {
        return FEEDING_ATTEMPTS;
    }
    needed as u32
}

/// La saturation qu'un joueur doit recevoir : `building.getBuildingLevel() ×
/// SATURATION_TO_SERVE` (`EntityAIWorkCook.java:288`).
pub fn required_saturation(building_level: u32) -> u32 {
    building_level.saturating_mul(SATURATION_TO_SERVE)
}

/// Le résultat d'`InventoryUtils.transferFoodUpToSaturation` (`:3222-3282`), réduit à ce
/// que le cuisinier en lit : le nombre d'items transférés (`:289-290`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Transfer {
    /// `count`, la somme des valeurs de `transferredItemMap` (`:289-290`).
    pub items: u32,
    /// `foundSaturation >= requiredSaturation` : la boucle s'est arrêtée sur une cible
    /// rassasiée (`:3260`), et non sur un sac vide.
    pub saturated: bool,
}

/// `InventoryUtils.transferFoodUpToSaturation` (`:3222-3282`), privé de ses transferts.
///
/// `amount = round(ceil((required − found) / nutrition))` (`:3237`) ; si la pile ne suffit
/// pas, tout part et `found += count × nutrition` (`:3241-3244`) ; sinon la pile suffit,
/// `found = required` et la fonction s'arrête (`:3246-3249`, `:3260`). Le `Math.round`
/// autour d'un `Math.ceil` ne change rien : il est recopié pour que la ligne soit
/// reconnaissable.
pub fn plan_food_transfer(required_saturation: u32, stacks: &[FoodStack]) -> Transfer {
    let required = f64::from(required_saturation);
    let mut found = 0.0_f64;
    let mut items = 0u32;
    for stack in stacks {
        if stack.count == 0 || stack.nutrition <= 0.0 {
            // `itemFood == null` (`:3231-3234`) et une pile vide (`:3226`) sont sautées.
            continue;
        }
        let missing = (required - found).max(0.0);
        let amount = (missing / stack.nutrition).ceil().round();
        if amount > f64::from(stack.count) {
            found += f64::from(stack.count) * stack.nutrition;
            items = items.saturating_add(stack.count);
        } else {
            items = items.saturating_add(amount as u32);
            found = required;
            return Transfer {
                items,
                saturated: true,
            };
        }
        if found >= required {
            return Transfer {
                items,
                saturated: true,
            };
        }
    }
    Transfer {
        items,
        saturated: found >= required,
    }
}

/// Ce que `startWorking` demande au système de requêtes, quand il demande quelque chose.
///
/// Java écrit `needsCurrently` (une `Tuple<Predicate, Integer>`, `:251`, `:256`,
/// `:386`, `:404`) ; le port nomme les quatre prédicats parce qu'un `needsCurrently`
/// qu'on ne peut pas nommer ne se teste pas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four `needsCurrently` tuples the furnace user and the cook build
pub enum Gather {
    /// `new Tuple<>(this::isSmeltable, STACKSIZE)` (`AbstractEntityAIUsesFurnace.java:251`).
    Smeltable,
    /// `new Tuple<>(itemListModule::isItemInList, STACKSIZE)` (`:256`).
    Fuel,
    /// `new Tuple<>(stack -> new ItemStorage(stack).equals(storage), STACKSIZE)`
    /// (`EntityAIWorkCook.java:386`) : le plat précis d'un citoyen.
    CitizenFood {
        /// `citizen.getUUID()`, la clé que la file utilise.
        citizen: u32,
    },
    /// `new Tuple<>(foodPredicate, STACKSIZE)` (`:404`) : n'importe quel plat du menu,
    /// pour un joueur.
    MenuFood,
}

/// Ce que `checkForImportantJobs` a décidé (`EntityAIWorkCook.java:338-412`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four early returns plus `super`'s START_WORKING
pub enum ImportantJob {
    /// `GATHERING_REQUIRED_MATERIALS` pour le plat d'un citoyen (`:383-388`).
    GatherCitizenFood {
        /// Le citoyen dont le plat manque.
        citizen: u32,
    },
    /// `COOK_SERVE_FOOD_TO_CITIZEN` (`:392-395`).
    ServeCitizens,
    /// `GATHERING_REQUIRED_MATERIALS` pour un plat du menu, à servir à un joueur
    /// (`:400-407`).
    GatherMenuFood,
    /// `COOK_SERVE_FOOD_TO_PLAYER` (`:408`).
    ServePlayers,
    /// `START_WORKING` (`:411`) : rien de plus important, le four reprend la main.
    None,
}

/// Le retour complet de `checkForImportantJobs`, plaintes comprises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Important {
    /// L'état à prendre, ou [`ImportantJob::None`].
    pub job: ImportantJob,
    /// Le menu d'une hutte de niveau 3 et plus n'a aucun plat MineColonies
    /// (`:350-366`) : `POOR_MENU_INTERACTION`, priorité `IMPORTANT`, **et on continue**.
    pub poor_menu: bool,
}

/// `checkForImportantJobs` (`EntityAIWorkCook.java:338-412`), dans l'ordre du Java.
///
/// Les deux files sont dans `memory` et Java **les modifie ici** : les joueurs sont tous
/// ajoutés (`:347`), les citoyens servables un par un (`:379`). Le premier citoyen qui a
/// faim, qui n'a rien de mangeable et dont la hutte a son plat renvoie immédiatement
/// `GATHERING_REQUIRED_MATERIALS` (`:383-388`) — les citoyens suivants ne sont donc pas
/// examinés à ce tick.
pub fn important_jobs(facts: &CookFacts, memory: &mut CookMemory) -> Important {
    let mut important = Important {
        job: ImportantJob::None,
        poor_menu: false,
    };
    // :341-347 — les joueurs affamés et autorisés, tous mis en file.
    memory.queue_players(&facts.players);
    // :350-366 — le menu d'une hutte de niveau 3 et plus doit avoir un plat MineColonies.
    if facts.building_level >= LEVEL_TO_CHECK_MENU && !facts.menu_has_minecolonies_food {
        important.poor_menu = true;
    }
    // :368-390 — les citoyens de la hutte, dans l'ordre du monde.
    for citizen in &facts.citizens {
        // :370-375 — un cuisinier, un citoyen repu, ou un citoyen qui a déjà de quoi
        // manger : `continue`.
        if citizen.cook || !should_be_fed(citizen) || citizen.has_menu_food {
            continue;
        }
        if citizen.has_best_option {
            // :377-380 — le cuisinier a son plat : il entre en file.
            memory.queue_citizen(citizen.id);
        } else if citizen.food_in_building {
            // :383-388 — il doit aller le chercher, et on s'arrête là.
            important.job = ImportantJob::GatherCitizenFood {
                citizen: citizen.id,
            };
            return important;
        }
        // Sinon (`storage == null`) Java retombe sur le `continue` de la boucle : le
        // citoyen est ignoré à ce tick sans être mis en file.
    }
    // :392-395
    if !memory.citizens_to_serve.is_empty() {
        important.job = ImportantJob::ServeCitizens;
        return important;
    }
    // :397-409
    if !memory.players_to_serve.is_empty() {
        if facts.worker_menu_food == 0 && facts.building_has_menu_food {
            important.job = ImportantJob::GatherMenuFood;
            return important;
        }
        important.job = ImportantJob::ServePlayers;
    }
    important
}

/// L'état de sortie de `startWorking`, avec les effets que Java pose en chemin
/// (`walkTo`, `needsCurrently`, les deux requêtes, les trois interactions).
///
/// Une action d'IA ne peut rendre qu'un état ; un appelant qui ne verrait pas le reste
/// ne pourrait ni marcher, ni demander, ni prévenir. Tout ce que le corps de
/// `startWorking` écrit est donc ici.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Start {
    /// L'état rendu.
    pub next: TradeState,
    /// `walkTo = pos` (`AbstractEntityAIUsesFurnace.java:219`, `:227`, `:334`).
    pub walk_to: Option<Pos>,
    /// Le `needsCurrently` posé (`:251`, `:256`, `EntityAIWorkCook.java:386`, `:404`).
    pub gather: Option<Gather>,
    /// `requestSmeltable()` a été appelé (`:239`) : c'est-à-dire, pour un cuisinier,
    /// `FURNACE_USER_NO_FOOD` parce que le menu est vide (`EntityAIWorkCook.java:153-156`).
    pub ask_food: bool,
    /// Un `StackList` de carburant a été demandé (`:245-247`), de
    /// `STACKSIZE × nombre de fours` items.
    pub ask_fuel: u32,
    /// `FURNACE_USER_NO_FUEL` a été levée (`:193-199`).
    pub complain_no_fuel_list: bool,
    /// `BAKER_HAS_NO_FURNACES_MESSAGE` a été levée (`:201-208`).
    pub complain_no_furnaces: bool,
    /// `POOR_MENU_INTERACTION` a été levée (`EntityAIWorkCook.java:350-366`).
    pub poor_menu: bool,
    /// Pourquoi le tick n'a pas pu aller plus loin, quand il n'a pas pu.
    pub refusal: Option<Refusal>,
}

impl Start {
    /// « Rester en `START_WORKING` sans rien faire d'autre » : le `getState()` d'une
    /// branche sans effet.
    fn stay() -> Start {
        Start {
            next: TradeState::Furnace(SmeltState::StartWorking),
            walk_to: None,
            gather: None,
            ask_food: false,
            ask_fuel: 0,
            complain_no_fuel_list: false,
            complain_no_furnaces: false,
            poor_menu: false,
            refusal: None,
        }
    }
}

/// `startWorking` (`AbstractEntityAIUsesFurnace.java:182-261`) tel que le cuisinier le
/// vit : les sept étapes du parent, avec **son** `checkForImportantJobs` au milieu.
pub fn start_working(facts: &CookFacts, memory: &mut CookMemory) -> Start {
    let mut start = Start::stay();
    // :184-187 — `!walkToBuilding()`.
    if !facts.furnace.at_building {
        start.refusal = Some(Refusal::Walking);
        return start;
    }
    // :189-199 — la liste de carburant vide : message, et on continue.
    start.complain_no_fuel_list = facts.furnace.fuel_list_empty;
    // :201-208 — aucun four : `BAKER_HAS_NO_FURNACES_MESSAGE` et on reste.
    if facts.furnace.furnaces.is_empty() {
        start.complain_no_furnaces = true;
        start.refusal = Some(Refusal::NoFurnace);
        return start;
    }
    // :210-214 — `checkForImportantJobs()` ; le cuisinier le surcharge.
    let important = important_jobs(facts, memory);
    start.poor_menu = important.poor_menu;
    match important.job {
        ImportantJob::GatherCitizenFood { citizen } => {
            start.next = TradeState::Furnace(SmeltState::GatheringRequiredMaterials);
            start.gather = Some(Gather::CitizenFood { citizen });
            return start; // `EntityAIWorkCook.java:387`
        }
        ImportantJob::GatherMenuFood => {
            start.next = TradeState::Furnace(SmeltState::GatheringRequiredMaterials);
            start.gather = Some(Gather::MenuFood);
            return start; // `:405`
        }
        ImportantJob::ServeCitizens => {
            start.next = TradeState::Cook(CookState::ServeFoodToCitizen);
            return start; // `:394`
        }
        ImportantJob::ServePlayers => {
            start.next = TradeState::Cook(CookState::ServeFoodToPlayer);
            return start; // `:408`
        }
        ImportantJob::None => {}
    }
    // :216-221 — un carburant que le filtre refuse revient **avant** les produits.
    if let Some(at) = smelter::oven_to_retrieve_fuel_from(&facts.furnace) {
        start.next = TradeState::Furnace(SmeltState::RetrievingUsedFuelFromFurnace);
        start.walk_to = Some(at);
        return start;
    }
    // :223-228 — puis les produits cuits.
    if let Some(at) = smelter::oven_to_retrieve_from(&facts.furnace) {
        start.next = TradeState::Furnace(SmeltState::RetrievingEndProductFromFurnace);
        start.walk_to = Some(at);
        return start;
    }
    // :230-235 — les quatre comptes. `isSmeltable` est celui du cuisinier : l'appelant a
    // rempli `smeltable_in_building` et `smeltable_in_inventory` avec son prédicat.
    let smeltable_total = facts.furnace.smeltable_total();
    let fuel_total = facts.furnace.fuel_total();
    // :237-240 — plus rien à cuire, et la hutte peut encore en garder : `requestSmeltable`.
    start.ask_food = smeltable_total == 0 && !reached_max_to_keep(facts);
    // :242-247 — plus de carburant, et aucune requête de carburant ouverte.
    if fuel_total == 0 && !facts.furnace.open_fuel_request {
        start.ask_fuel = STACKSIZE.saturating_mul(facts.furnace.furnaces.len() as u32);
    }
    // :249-253 — la hutte a de quoi cuire, les poches non.
    if facts.furnace.smeltable_in_building > 0 && facts.furnace.smeltable_in_inventory == 0 {
        start.next = TradeState::Furnace(SmeltState::GatheringRequiredMaterials);
        start.gather = Some(Gather::Smeltable);
        return start;
    }
    // :254-258 — le même pour le carburant.
    if facts.furnace.fuel_in_building > 0 && facts.furnace.fuel_in_inventory == 0 {
        start.next = TradeState::Furnace(SmeltState::GatheringRequiredMaterials);
        start.gather = Some(Gather::Fuel);
        return start;
    }
    // :260 — `checkIfAbleToSmelt(fuel, smeltable)`, puis `checkForAdditionalJobs()` que
    // le cuisinier ne surcharge pas (`:355-358` du parent) et qui rend `START_WORKING`.
    let able = smelter::check_if_able_to_smelt(&facts.furnace);
    match able.fill {
        Some(at) => {
            start.next = TradeState::Furnace(SmeltState::FillUpFurnaces);
            start.walk_to = Some(at); // :334
        }
        None => {
            start.refusal = Some(Refusal::Furnace(
                crate::ai_work_04::states::Refusal::NothingToFill,
            ));
        }
    }
    start
}

/// `getSmeltAbleClass` (`EntityAIWorkCook.java:433-437`) : `new Food(STACKSIZE,
/// building.getBuildingLevel())`.
pub fn food_request(building_level: u32) -> (u32, u32) {
    (STACKSIZE, building_level)
}

/// `extractFromFurnace` (`EntityAIWorkCook.java:113-121`) : le résultat passe dans le
/// sac, l'expérience tombe, et le compteur d'actions avance d'un cran
/// (`incrementActionsDoneAndDecSaturation`, `:120`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extraction {
    /// `getCitizenExperienceHandler().addExperience(BASE_XP_GAIN)` (`:119`).
    pub xp: f64,
    /// `incrementActionsDone` (`:120`).
    pub actions: u32,
}

/// Ce que la sortie d'un four donne au cuisinier, en expérience et en actions.
pub fn extraction() -> Extraction {
    Extraction {
        xp: BASE_XP_GAIN,
        actions: 1,
    }
}

// ------------------------------------------------------------------- décisions

/// `serveFoodToCitizen` (`EntityAIWorkCook.java:163-249`).
pub fn serve_food_to_citizen(facts: &CookFacts, memory: &mut CookMemory) -> Decision {
    // :165-168 — file vide.
    if memory.citizens_to_serve.is_empty() {
        return Decision::go_after(
            TradeState::Furnace(SmeltState::StartWorking),
            Refusal::CitizenQueueEmpty,
        );
    }
    // Une file d'identifiants sans observation de la tête : l'appelant n'a pas rempli
    // `citizen_target`. Nommé plutôt que deviné.
    let Some(target) = facts.citizen_target else {
        return Decision::refuse(Refusal::NoTargetObservation);
    };
    // :172-177 — il a quitté la hutte : sorti de la file, et on reste dans l'état.
    if !target.in_building {
        memory.poll_citizen();
        return Decision::refuse(Refusal::CitizenLeftBuilding);
    }
    // :179-182 — `!walkToWorkPos(...)` : on marche encore.
    if !target.at_work_pos {
        return Decision::refuse(Refusal::NotAtServeTarget);
    }
    memory.poll_citizen(); // :184
                           // :190-214 — le citoyen est plein : la nourriture est consommée sur place.
    if !target.has_space {
        let Some(_slot) = target.food_slot else {
            // `getBestFoodForCitizen` a rendu `-1` au premier tour : `break` immédiat.
            return Decision::refuse(Refusal::NoBestFood);
        };
        return Decision::refuse(Refusal::TargetInventoryFull).with_effect(Effect::FeedInPlace {
            citizen: target.id,
            items: feed_items(target.saturation, target.food_value),
        });
    }
    // :215-218 — il a déjà de quoi manger dans son sac.
    if target.has_menu_food {
        return Decision::refuse(Refusal::TargetAlreadyHasFood);
    }
    // :220-229 — plus de plat en tête de liste.
    let Some(slot) = target.food_slot else {
        if target.worker_menu_food == 0 {
            memory.clear_citizens(); // :225
            return Decision::go_after(
                TradeState::Furnace(SmeltState::StartWorking),
                Refusal::NoFoodLeft,
            );
        }
        return Decision::refuse(Refusal::NoBestFood); // :228
    };
    // :231-234 — le logement du citoyen est trop beau pour ce restaurant :
    // `POOR_RESTAURANT_INTERACTION`, et **on sert quand même**.
    let poor_restaurant = home_is_too_fancy(target.home_level, facts.building_level);
    // :236-246 — la portion, le transfert, les statistiques, l'expérience.
    let qty = portion_for(target.saturation, target.food_value);
    let mut decision = Decision::stay().with_effect(Effect::ServePortion {
        citizen: target.id,
        slot,
        qty,
    });
    if poor_restaurant {
        decision.refusal = Some(Refusal::PoorRestaurant);
    }
    decision
}

/// `serveFoodToPlayer` (`EntityAIWorkCook.java:255-304`).
pub fn serve_food_to_player(facts: &CookFacts, memory: &mut CookMemory) -> Decision {
    // :257-260
    if memory.players_to_serve.is_empty() {
        return Decision::go_after(
            TradeState::Furnace(SmeltState::StartWorking),
            Refusal::PlayerQueueEmpty,
        );
    }
    let Some(target) = facts.player_target else {
        return Decision::refuse(Refusal::NoTargetObservation);
    };
    // :263-268 — il a quitté la hutte : sorti de la file, et retour au travail.
    if !target.in_building {
        memory.poll_player();
        return Decision::go_after(
            TradeState::Furnace(SmeltState::StartWorking),
            Refusal::PlayerLeftBuilding,
        );
    }
    // :270-273
    if !target.at_work_pos {
        return Decision::refuse(Refusal::NotAtServeTarget);
    }
    memory.poll_player(); // :275
                          // :279-282
    if target.inventory_full {
        return Decision::refuse(Refusal::PlayerInventoryFull);
    }
    // :283-286
    if target.has_menu_food {
        return Decision::refuse(Refusal::TargetAlreadyHasFood);
    }
    // :288-290 — `transferFoodUpToSaturation(worker, handler, level × 16, canEat)`.
    let transfer = plan_food_transfer(
        required_saturation(facts.building_level),
        &facts.food_in_inventory,
    );
    // :292-296 — rien n'est passé : la file des joueurs est vidée.
    if transfer.items == 0 {
        memory.clear_players();
        return Decision::go_after(
            TradeState::Furnace(SmeltState::StartWorking),
            Refusal::NothingTransferred,
        );
    }
    // :297-303 — statistiques, expérience, saturation, et retour au travail.
    Decision::go(TradeState::Furnace(SmeltState::StartWorking)).with_effect(Effect::ServePlayer {
        player: target.id,
        items: transfer.items,
    })
}

// --------------------------------------------------------------------- actions

/// La forme commune des huit actions du cuisinier : découper la scène, refuser les faits
/// d'un autre métier, noter le refus et l'effet.
macro_rules! cook_action {
    ($name:ident, $cook:path, $state:expr) => {
        fn $name(
            scene: &mut crate::ai_work_11::Scene,
            state: crate::ai_work_11::TradeState,
        ) -> Option<crate::ai_work_11::TradeState> {
            let crate::ai_work_11::Scene {
                job,
                memory,
                refusal,
                effect,
                ..
            } = scene;
            let crate::ai_work_11::facts::TradeJob::Cook(facts) = job else {
                *refusal = Some(crate::ai_work_11::state::Refusal::NoFactsForTrade);
                return Some(state);
            };
            let decision: crate::ai_work_11::Decision = $cook(facts, &mut memory.cook, $state);
            *refusal = decision.refusal;
            *effect = decision.effect;
            decision.next.or(Some(state))
        }
    };
}

/// Le cuisinier passe `state` en paramètre à ses décisions : les branches « il a quitté
/// la hutte » et « il a déjà mangé » rendent `getState()`.
cook_action!(
    serve_citizen_action,
    serve_food_to_citizen_with_state,
    CookState::ServeFoodToCitizen
);
cook_action!(
    serve_player_action,
    serve_food_to_player_with_state,
    CookState::ServeFoodToPlayer
);

/// `serveFoodToCitizen` enveloppé : la décision elle-même ne lit pas l'état, seule la
/// macro en a besoin pour un `getState()` éventuel.
fn serve_food_to_citizen_with_state(
    facts: &CookFacts,
    memory: &mut CookMemory,
    _state: CookState,
) -> Decision {
    serve_food_to_citizen(facts, memory)
}

/// La même enveloppe pour les joueurs.
fn serve_food_to_player_with_state(
    facts: &CookFacts,
    memory: &mut CookMemory,
    _state: CookState,
) -> Decision {
    serve_food_to_player(facts, memory)
}

/// `:77` — `new AITarget(IDLE, START_WORKING, STANDARD_DELAY)`.
fn idle_to_work(_scene: &mut Scene, _state: TradeState) -> Option<TradeState> {
    Some(TradeState::Furnace(SmeltState::StartWorking))
}

/// `:78` — `new AITarget(START_WORKING, this::startWorking, 60)`.
fn start_working_action(scene: &mut Scene, state: TradeState) -> Option<TradeState> {
    let Scene {
        job,
        memory,
        refusal,
        ..
    } = scene;
    let TradeJob::Cook(facts) = job else {
        *refusal = Some(Refusal::NoFactsForTrade);
        return Some(state);
    };
    let start = start_working(facts, &mut memory.cook);
    // `walkTo = pos` (`:219`, `:227`, `:334`) : Java n'écrit que s'il y a une position,
    // sinon il laisse la valeur précédente.
    if let Some(at) = start.walk_to {
        memory.base.walk_to = Some(at);
    }
    *refusal = start.refusal;
    Some(start.next)
}

/// `:79` — `new AITarget(FILL_UP_FURNACES, this::fillUpFurnace, STANDARD_DELAY)`.
fn fill_up_action(scene: &mut Scene, state: TradeState) -> Option<TradeState> {
    let Scene {
        job,
        memory,
        refusal,
        ..
    } = scene;
    let TradeJob::Cook(facts) = job else {
        *refusal = Some(Refusal::NoFactsForTrade);
        return Some(state);
    };
    match smelter::fill_up_furnace(&facts.furnace, memory.base.walk_to) {
        Fill::NoFurnaces => {
            // `:454-462` : `BAKER_HAS_NO_FURNACES_MESSAGE` et retour au travail.
            *refusal = Some(Refusal::NoFurnace);
            Some(TradeState::Furnace(SmeltState::StartWorking))
        }
        Fill::NoTarget => {
            memory.base.walk_to = None; // `:464-468`
            Some(TradeState::Furnace(SmeltState::StartWorking))
        }
        Fill::NotArrived => {
            *refusal = Some(Refusal::Furnace(
                crate::ai_work_04::states::Refusal::NotArrived,
            ));
            Some(state) // `:470-473` `return getState()`
        }
        Fill::Filled { .. } => {
            // `:497-498` : `walkTo = null` et retour au travail. Ce qui est entré est
            // visible dans les faits du four, pas dans un effet séparé.
            memory.base.walk_to = None;
            Some(TradeState::Furnace(SmeltState::StartWorking))
        }
    }
}

/// `:81` — `new AITarget(RETRIEVING_END_PRODUCT_FROM_FURNACE,
/// this::retrieveSmeltableFromFurnace, STANDARD_DELAY)`.
fn retrieve_product_action(scene: &mut Scene, state: TradeState) -> Option<TradeState> {
    let Scene {
        facts,
        job,
        memory,
        refusal,
        ..
    } = scene;
    let TradeJob::Cook(cook) = job else {
        *refusal = Some(Refusal::NoFactsForTrade);
        return Some(state);
    };
    match smelter::retrieve_product(&cook.furnace, memory.base.walk_to) {
        Retrieve::NoTarget => Some(TradeState::Furnace(SmeltState::StartWorking)), // :390-393
        Retrieve::NotArrived => {
            *refusal = Some(Refusal::Furnace(
                crate::ai_work_04::states::Refusal::NotArrived,
            ));
            Some(state) // `:395-398` `return getState()`
        }
        Retrieve::Gone => {
            memory.base.walk_to = None; // `:404`
            *refusal = Some(Refusal::Furnace(
                crate::ai_work_04::states::Refusal::FurnaceGone,
            ));
            Some(TradeState::Furnace(SmeltState::StartWorking))
        }
        Retrieve::Empty => {
            memory.base.walk_to = None; // `:404`
            *refusal = Some(Refusal::Furnace(
                crate::ai_work_04::states::Refusal::ResultEmpty,
            ));
            Some(TradeState::Furnace(SmeltState::StartWorking))
        }
        Retrieve::Extracted => {
            memory.base.walk_to = None; // `:408`
                                        // `:410` `extractFromFurnace` puis `:411` `incrementActionsDoneAndDecSaturation`.
            facts.inventory.actions_done += extraction().actions;
            Some(TradeState::Furnace(SmeltState::StartWorking)) // `:412`
        }
    }
}

/// `:82` — `new AITarget(RETRIEVING_USED_FUEL_FROM_FURNACE, this::retrieveUsedFuel,
/// STANDARD_DELAY)`.
fn retrieve_fuel_action(scene: &mut Scene, state: TradeState) -> Option<TradeState> {
    let Scene {
        job,
        memory,
        refusal,
        ..
    } = scene;
    let TradeJob::Cook(cook) = job else {
        *refusal = Some(Refusal::NoFactsForTrade);
        return Some(state);
    };
    match smelter::retrieve_used_fuel(&cook.furnace, memory.base.walk_to) {
        Retrieve::NoTarget => Some(TradeState::Furnace(SmeltState::StartWorking)),
        Retrieve::NotArrived => {
            *refusal = Some(Refusal::Furnace(
                crate::ai_work_04::states::Refusal::NotArrived,
            ));
            Some(state)
        }
        Retrieve::Gone => {
            memory.base.walk_to = None;
            *refusal = Some(Refusal::Furnace(
                crate::ai_work_04::states::Refusal::FurnaceGone,
            ));
            Some(TradeState::Furnace(SmeltState::StartWorking))
        }
        Retrieve::Empty => {
            memory.base.walk_to = None;
            *refusal = Some(Refusal::Furnace(
                crate::ai_work_04::states::Refusal::FuelEmpty,
            ));
            Some(TradeState::Furnace(SmeltState::StartWorking))
        }
        Retrieve::Extracted => {
            // `:443` : `extractFuelFromFurnace` — pas d'expérience, pas de compteur
            // d'action, contrairement à la sortie de produit (`:411`).
            memory.base.walk_to = None;
            Some(TradeState::Furnace(SmeltState::StartWorking))
        }
    }
}

/// `:80` — `new AIEventTarget(AI_BLOCKING, this::accelerateFurnaces, TICKS_SECOND)`.
///
/// `accelerateFurnaces` rend `null` (`:305`) : l'événement ne change **jamais** d'état et
/// ne coupe jamais le tick. Ce qu'il fait est un effet que l'appelant exécute :
/// [`smelter::acceleration_ticks`] ticks de four par four allumé (`:285-304`).
fn accelerate_action(_scene: &mut Scene, _state: TradeState) -> Option<TradeState> {
    None
}

/// Enregistre les huit cibles du cuisinier : les six d'`AbstractEntityAIUsesFurnace.java:
/// 77-82 et les deux d'`EntityAIWorkCook.java:96-97`.
pub fn register(machine: &mut Machine<TradeState, Scene>) {
    // :77 — `new AITarget(IDLE, START_WORKING, STANDARD_DELAY)`.
    machine.add(crate::ai::Transition::goto(
        TradeState::Furnace(SmeltState::Idle),
        TradeState::Furnace(SmeltState::StartWorking),
        "idle",
        "AbstractEntityAIUsesFurnace.java:77",
        STANDARD_DELAY,
    ));
    // :78 — `new AITarget(START_WORKING, this::startWorking, 60)`.
    add_target(
        machine,
        TradeState::Furnace(SmeltState::StartWorking),
        "startWorking",
        "AbstractEntityAIUsesFurnace.java:78",
        START_WORKING_RATE,
        start_working_action,
    );
    // :79 — `new AITarget(FILL_UP_FURNACES, this::fillUpFurnace, STANDARD_DELAY)`.
    add_target(
        machine,
        TradeState::Furnace(SmeltState::FillUpFurnaces),
        "fillUpFurnace",
        "AbstractEntityAIUsesFurnace.java:79",
        STANDARD_DELAY,
        fill_up_action,
    );
    // :80 — l'événement de fond, jamais bloquant.
    machine.add(crate::ai::Transition::event(
        Blocking::AiBlocking,
        "accelerateFurnaces",
        "AbstractEntityAIUsesFurnace.java:80",
        TICKS_SECOND,
        crate::ai::Action::Run(accelerate_action),
    ));
    // :81 — `new AITarget(RETRIEVING_END_PRODUCT_FROM_FURNACE, …, STANDARD_DELAY)`.
    add_target(
        machine,
        TradeState::Furnace(SmeltState::RetrievingEndProductFromFurnace),
        "retrieveSmeltableFromFurnace",
        "AbstractEntityAIUsesFurnace.java:81",
        STANDARD_DELAY,
        retrieve_product_action,
    );
    // :82 — `new AITarget(RETRIEVING_USED_FUEL_FROM_FURNACE, …, STANDARD_DELAY)`.
    add_target(
        machine,
        TradeState::Furnace(SmeltState::RetrievingUsedFuelFromFurnace),
        "retrieveUsedFuel",
        "AbstractEntityAIUsesFurnace.java:82",
        STANDARD_DELAY,
        retrieve_fuel_action,
    );
    // :96 — `new AITarget(COOK_SERVE_FOOD_TO_CITIZEN, this::serveFoodToCitizen, SERVE_DELAY)`.
    add_target(
        machine,
        TradeState::Cook(CookState::ServeFoodToCitizen),
        "serveFoodToCitizen",
        "EntityAIWorkCook.java:96",
        SERVE_DELAY,
        serve_citizen_action,
    );
    // :97 — `new AITarget(COOK_SERVE_FOOD_TO_PLAYER, this::serveFoodToPlayer, SERVE_DELAY)`.
    add_target(
        machine,
        TradeState::Cook(CookState::ServeFoodToPlayer),
        "serveFoodToPlayer",
        "EntityAIWorkCook.java:97",
        SERVE_DELAY,
        serve_player_action,
    );
    register_handover(machine);
}

/// Les deux retours d'état de base du parent : `INVENTORY_FULL` → `IDLE` par `afterDump`
/// (`AbstractEntityAIBasic.java:1213-1222`) et `GATHERING_REQUIRED_MATERIALS` → `IDLE`
/// par `getStateAfterPickUp` (`:323-326`).
fn register_handover(machine: &mut Machine<TradeState, Scene>) {
    machine.add(crate::ai::Transition::goto(
        TradeState::Furnace(SmeltState::InventoryFull),
        TradeState::Furnace(SmeltState::Idle),
        "afterDump",
        "AbstractEntityAIBasic.java:1213-1222",
        STANDARD_DELAY,
    ));
    machine.add(crate::ai::Transition::goto(
        TradeState::Furnace(SmeltState::GatheringRequiredMaterials),
        TradeState::Furnace(SmeltState::Idle),
        "getNeededItem",
        "AbstractEntityAIBasic.java:323-326",
        STANDARD_DELAY,
    ));
}

/// La table du four, répond sur les mêmes faits — utilisée par le test de parité.
pub fn furnace_facts(facts: &CookFacts) -> &SmelterFacts {
    &facts.furnace
}

#[cfg(test)]
mod tests {
    use super::super::facts::{ServeCitizen, ServePlayer};
    use super::*;

    #[test]
    fn the_eight_cadences_and_the_saturation_constants_are_java_s() {
        // `EntityAIWorkCook.java:59`, `:64`, `:69`, `:192`, `:239`, `:350`, `:427-431` ;
        // `AbstractEntityAIUsesFurnace.java:55`, `:78` ; `CitizenConstants.java:43`,
        // `:53` ; `ICitizenData.java:30` ; `Constants.java:163`.
        assert_eq!(SATURATION_TO_SERVE, 16);
        assert_eq!(required_saturation(1), 16);
        assert_eq!(required_saturation(5), 80);
        assert_eq!(SERVE_DELAY, 30);
        assert_eq!(LEVEL_TO_FEED_PLAYER, 10);
        assert_eq!(FEEDING_ATTEMPTS, 10);
        assert_eq!(PORTION_FACTOR, 1.5);
        assert_eq!(AVERAGE_SATURATION, 10.0);
        assert_eq!(FULL_SATURATION, 60.0);
        assert_eq!(SLOT_PER_LINE, 9);
        assert_eq!(BASE_XP_GAIN, 2.0);
        assert_eq!(ACTIONS_UNTIL_DUMP, 1);
        assert_eq!(START_WORKING_RATE, 60);
        assert_eq!(LEVEL_TO_CHECK_MENU, 3);
        assert_eq!(extraction().actions, ACTIONS_UNTIL_DUMP);
    }

    #[test]
    fn a_cook_only_cooks_what_the_menu_ordered() {
        // `EntityAIWorkCook.java:130-134` : `ISCOOKABLE && menu.contains(result)`.
        assert!(is_smeltable(true, true));
        assert!(!is_smeltable(true, false), "cuitable mais pas au menu");
        assert!(
            !is_smeltable(false, true),
            "une pierre n'est pas cuisinable"
        );
    }

    #[test]
    fn the_food_ceiling_is_the_square_of_the_level_in_stacks() {
        // `:143` : `max(1, niveau²) × SLOT_PER_LINE`.
        assert_eq!(max_food_to_keep(0), 9, "niveau 0 : max(1, 0²) = 1");
        assert_eq!(max_food_to_keep(1), 9);
        assert_eq!(max_food_to_keep(3), 81);
        assert_eq!(max_food_to_keep(5), 225);
        // Et le `>` de `:146`.
        assert!(!food_kept_over_limit(1, 9));
        assert!(food_kept_over_limit(1, 10));
    }

    #[test]
    fn reached_max_to_keep_is_the_parent_or_the_food_ceiling() {
        // `:136-147`, avec la règle du parent `AbstractEntityAIUsesFurnace.java:117-121`
        // (`empty_slots <= 3`) réutilisée.
        let mut facts = CookFacts::new();
        facts.furnace.empty_slots_in_building = 3;
        assert!(reached_max_to_keep(&facts), "le parent suffit");
        facts.furnace.empty_slots_in_building = 27;
        assert!(!reached_max_to_keep(&facts));
        facts.food_kept = 10;
        assert!(reached_max_to_keep(&facts), "9 items de limite, 10 gardés");
    }

    #[test]
    fn a_working_citizen_is_not_fed_however_hungry_it_is() {
        // `:419-425` — quatre conjonctions.
        let hungry = CitizenHere::hungry(1);
        assert!(should_be_fed(&hungry));
        assert!(!should_be_fed(&CitizenHere {
            working: true,
            ..hungry
        }));
        assert!(!should_be_fed(&CitizenHere {
            just_ate: true,
            ..hungry
        }));
        assert!(!should_be_fed(&CitizenHere {
            saturation: 11.0,
            ..hungry
        }));
        assert!(
            should_be_fed(&CitizenHere {
                saturation: 10.0,
                ..hungry
            }),
            "10 est encore sous la moyenne : `<=`"
        );
    }

    #[test]
    fn the_portions_truncate_before_multiplying_by_one_and_a_half() {
        // `:236-239`.
        assert_eq!(portion_for(0.0, 5.0), 18, "12 tronqués, puis 12 × 1,5 = 18");
        assert_eq!(portion_for(55.0, 5.0), 2, "1 puis ceil(1,5) = 2");
        assert_eq!(portion_for(59.0, 5.0), 2, "le plancher de 1 est multiplié");
        assert_eq!(portion_for(50.0, 4.0), 3, "2,5 tronqué à 2, puis ceil(3,0)");
        assert_eq!(portion_for(51.0, 4.0), 3, "2,25 tronqué à 2, puis 3");
        // La troncature se voit : 60 / 7 = 8,571 ; tronqué à 8, puis 12. Sans elle on
        // aurait ceil(12,857) = 13.
        assert_eq!(portion_for(0.0, 7.0), 12);
        // Une valeur nulle ne peut pas être divisée : la portion minimale.
        assert_eq!(portion_for(0.0, 0.0), 1);
    }

    #[test]
    fn a_full_inventory_drinks_at_most_ten_items() {
        // `:192-211`.
        assert_eq!(feed_items(0.0, 5.0), 10, "12 nécessaires, plafonnées à 10");
        assert_eq!(feed_items(55.0, 5.0), 1, "5 manquants, un item");
        assert_eq!(feed_items(0.0, 20.0), 3, "60 / 20");
        assert_eq!(feed_items(59.0, 5.0), 1, "le premier tour a toujours lieu");
        assert_eq!(
            feed_items(0.0, 0.0),
            10,
            "valeur nulle : aucun progrès, dix items"
        );
    }

    #[test]
    fn the_player_transfer_fills_up_to_the_required_saturation() {
        // `InventoryUtils.java:3222-3282`.
        let one = vec![FoodStack {
            nutrition: 4.0,
            count: 10,
        }];
        let plan = plan_food_transfer(16, &one);
        assert_eq!(plan.items, 4);
        assert!(plan.saturated);
        // Une pile trop petite : tout part, et la suivante complète.
        let two = vec![
            FoodStack {
                nutrition: 8.0,
                count: 1,
            },
            FoodStack {
                nutrition: 4.0,
                count: 10,
            },
        ];
        let plan = plan_food_transfer(16, &two);
        assert_eq!(plan.items, 3, "1 item de 8, puis 2 items de 4");
        assert!(plan.saturated);
        // Rien de mangeable : aucun transfert, et la file sera vidée.
        let none = plan_food_transfer(16, &[]);
        assert_eq!(none.items, 0);
        assert!(!none.saturated);
        // Une pile vide ou une nutrition nulle sont sautées (`:3226`, `:3231-3234`).
        let broken = vec![
            FoodStack {
                nutrition: 0.0,
                count: 12,
            },
            FoodStack {
                nutrition: 4.0,
                count: 0,
            },
        ];
        assert_eq!(plan_food_transfer(16, &broken).items, 0);
    }

    #[test]
    fn a_fancier_home_and_a_hungry_player_are_named_thresholds() {
        // `:231` pour le logement, `:343` pour la faim d'un joueur.
        assert!(!home_is_too_fancy(2, 1), "2 > 1 + 1 est faux");
        assert!(home_is_too_fancy(3, 1), ":231 est un `>` strict");
        assert!(player_is_hungry(9));
        assert!(!player_is_hungry(10), "`:343` est un `<` strict");
    }

    #[test]
    fn a_hungry_citizen_with_a_plate_in_hand_enters_the_queue() {
        // `:377-380` puis `:392-395`.
        let mut facts = CookFacts::new();
        facts.citizens = vec![CitizenHere::hungry(4)];
        let mut memory = CookMemory::default();
        let important = important_jobs(&facts, &mut memory);
        assert_eq!(important.job, ImportantJob::ServeCitizens);
        assert_eq!(memory.citizens_to_serve, vec![4]);
        assert!(!important.poor_menu);
    }

    #[test]
    fn a_hungry_citizen_without_a_plate_sends_the_cook_to_the_chest() {
        // `:383-388` — le retour coupe la boucle : le citoyen suivant n'est pas examiné.
        let mut facts = CookFacts::new();
        facts.citizens = vec![
            CitizenHere {
                has_best_option: false,
                food_in_building: true,
                ..CitizenHere::hungry(4)
            },
            CitizenHere::hungry(5),
        ];
        let mut memory = CookMemory::default();
        let important = important_jobs(&facts, &mut memory);
        assert_eq!(
            important.job,
            ImportantJob::GatherCitizenFood { citizen: 4 }
        );
        assert!(
            memory.citizens_to_serve.is_empty(),
            "le 4 n'est pas en file"
        );
    }

    #[test]
    fn a_cook_a_working_citizen_and_a_fed_citizen_are_all_skipped() {
        // `:370-375`.
        let mut facts = CookFacts::new();
        facts.citizens = vec![
            CitizenHere {
                cook: true,
                ..CitizenHere::hungry(1)
            },
            CitizenHere {
                working: true,
                ..CitizenHere::hungry(2)
            },
            CitizenHere {
                has_menu_food: true,
                ..CitizenHere::hungry(3)
            },
            CitizenHere::hungry(4),
        ];
        let mut memory = CookMemory::default();
        let important = important_jobs(&facts, &mut memory);
        assert_eq!(important.job, ImportantJob::ServeCitizens);
        assert_eq!(memory.citizens_to_serve, vec![4], "seul le 4 est servable");
    }

    #[test]
    fn players_are_queued_and_served_only_with_food_in_the_backpack() {
        // `:341-347` puis `:397-409`.
        let mut facts = CookFacts::new();
        facts.players = vec![7, 8];
        let mut memory = CookMemory::default();
        let important = important_jobs(&facts, &mut memory);
        assert_eq!(important.job, ImportantJob::ServePlayers);
        assert_eq!(memory.players_to_serve, vec![7, 8]);
        assert!(memory.citizens_to_serve.is_empty());

        // Sans plat en poche mais avec du plat en hutte : il va le chercher (`:402-406`).
        let mut memory = CookMemory::default();
        facts.building_has_menu_food = true;
        let important = important_jobs(&facts, &mut memory);
        assert_eq!(important.job, ImportantJob::GatherMenuFood);

        // Et si le citoyen passe avant, il gagne : `:392-395` précède `:397`.
        let mut facts = CookFacts::new();
        facts.players = vec![7];
        facts.citizens = vec![CitizenHere::hungry(4)];
        let mut memory = CookMemory::default();
        assert_eq!(
            important_jobs(&facts, &mut memory).job,
            ImportantJob::ServeCitizens
        );
    }

    #[test]
    fn a_level_three_menu_without_minecolonies_food_complains() {
        // `:350-366`.
        let mut facts = CookFacts::new();
        facts.building_level = 3;
        let mut memory = CookMemory::default();
        assert!(important_jobs(&facts, &mut memory).poor_menu);
        facts.menu_has_minecolonies_food = true;
        assert!(!important_jobs(&facts, &mut memory).poor_menu);
        facts.building_level = 2;
        facts.menu_has_minecolonies_food = false;
        assert!(
            !important_jobs(&facts, &mut memory).poor_menu,
            "niveau 2 : rien"
        );
    }

    #[test]
    fn serving_a_citizen_walks_then_hands_over_the_portion() {
        // `:163-249` : la file vide, la marche, puis le transfert.
        let mut facts = CookFacts::new();
        let mut memory = CookMemory::default();
        let empty = serve_food_to_citizen(&facts, &mut memory);
        assert_eq!(
            empty.next,
            Some(TradeState::Furnace(SmeltState::StartWorking))
        );
        assert_eq!(empty.refusal, Some(Refusal::CitizenQueueEmpty));

        memory.queue_citizen(4);
        facts.citizen_target = Some(ServeCitizen::ready(4, 5.0));
        let served = serve_food_to_citizen(&facts, &mut memory);
        assert_eq!(served.next, None, "`:248` getState()");
        assert_eq!(
            served.effect,
            Some(Effect::ServePortion {
                citizen: 4,
                slot: 3,
                qty: 18
            })
        );
        assert!(memory.citizens_to_serve.is_empty(), "`:184` poll");
    }

    #[test]
    fn a_citizen_who_left_the_building_is_dropped_without_serving() {
        // `:172-177`.
        let mut facts = CookFacts::new();
        let mut memory = CookMemory::default();
        memory.queue_citizen(4);
        facts.citizen_target = Some(ServeCitizen {
            in_building: false,
            ..ServeCitizen::ready(4, 5.0)
        });
        let decision = serve_food_to_citizen(&facts, &mut memory);
        assert_eq!(decision.next, None);
        assert_eq!(decision.refusal, Some(Refusal::CitizenLeftBuilding));
        assert!(memory.citizens_to_serve.is_empty(), "`:175` poll");
        assert_eq!(decision.effect, None);
    }

    #[test]
    fn a_citizen_still_walking_keeps_the_state_and_the_queue() {
        // `:179-182`.
        let mut facts = CookFacts::new();
        let mut memory = CookMemory::default();
        memory.queue_citizen(4);
        facts.citizen_target = Some(ServeCitizen {
            at_work_pos: false,
            ..ServeCitizen::ready(4, 5.0)
        });
        let decision = serve_food_to_citizen(&facts, &mut memory);
        assert_eq!(decision.refusal, Some(Refusal::NotAtServeTarget));
        assert_eq!(memory.citizens_to_serve, vec![4], "rien n'est sorti");
    }

    #[test]
    fn a_full_citizen_eats_on_the_spot_instead_of_receiving_a_stack() {
        // `:190-214`.
        let mut facts = CookFacts::new();
        let mut memory = CookMemory::default();
        memory.queue_citizen(4);
        facts.citizen_target = Some(ServeCitizen {
            has_space: false,
            saturation: 55.0,
            ..ServeCitizen::ready(4, 5.0)
        });
        let decision = serve_food_to_citizen(&facts, &mut memory);
        assert_eq!(decision.next, None, "`:213` getState()");
        assert_eq!(decision.refusal, Some(Refusal::TargetInventoryFull));
        assert_eq!(
            decision.effect,
            Some(Effect::FeedInPlace {
                citizen: 4,
                items: 1
            })
        );
    }

    #[test]
    fn a_citizen_with_nothing_left_to_eat_empties_the_queue() {
        // `:221-227` : `foodSlot == -1` et plus rien de mangeable dans le sac.
        let mut facts = CookFacts::new();
        let mut memory = CookMemory::default();
        memory.queue_citizen(4);
        memory.queue_citizen(5);
        facts.citizen_target = Some(ServeCitizen {
            food_slot: None,
            worker_menu_food: 0,
            ..ServeCitizen::ready(4, 5.0)
        });
        let decision = serve_food_to_citizen(&facts, &mut memory);
        assert_eq!(
            decision.next,
            Some(TradeState::Furnace(SmeltState::StartWorking))
        );
        assert_eq!(decision.refusal, Some(Refusal::NoFoodLeft));
        assert!(memory.citizens_to_serve.is_empty(), "`:225` clear");
    }

    #[test]
    fn a_plate_that_does_not_fit_the_citizen_is_dropped_without_serving() {
        // `:228` : `foodSlot == -1` mais le sac a encore du plat — `return getState()`.
        //
        // **Le citoyen est déjà sorti de la file**, et c'est le Java : `:184` fait
        // `citizenToServe.poll()` juste après le test de marche de `:179-182`, donc
        // *avant* le test de plat de `:220`. Mon premier test attendait l'inverse (une
        // file encore pleine) : c'est mon attente qui était fausse, pas la règle.
        let mut facts = CookFacts::new();
        let mut memory = CookMemory::default();
        memory.queue_citizen(4);
        facts.citizen_target = Some(ServeCitizen {
            food_slot: None,
            worker_menu_food: 3,
            ..ServeCitizen::ready(4, 5.0)
        });
        let decision = serve_food_to_citizen(&facts, &mut memory);
        assert_eq!(decision.refusal, Some(Refusal::NoBestFood));
        assert_eq!(decision.next, None, "`:228` getState()");
        assert_eq!(decision.effect, None, "rien n'a été servi");
        assert!(
            memory.citizens_to_serve.is_empty(),
            "`:184` a sorti le citoyen avant le test de `:220`"
        );
    }

    #[test]
    fn a_citizen_already_carrying_food_is_not_served() {
        // `:215-218`.
        let mut facts = CookFacts::new();
        let mut memory = CookMemory::default();
        memory.queue_citizen(4);
        facts.citizen_target = Some(ServeCitizen {
            has_menu_food: true,
            ..ServeCitizen::ready(4, 5.0)
        });
        let decision = serve_food_to_citizen(&facts, &mut memory);
        assert_eq!(decision.refusal, Some(Refusal::TargetAlreadyHasFood));
        assert_eq!(decision.effect, None);
    }

    #[test]
    fn a_richer_house_than_the_restaurant_is_flagged_but_still_served() {
        // `:231-234` : l'interaction part, et le transfert a lieu quand même.
        let mut facts = CookFacts::new();
        facts.building_level = 1;
        let mut memory = CookMemory::default();
        memory.queue_citizen(4);
        facts.citizen_target = Some(ServeCitizen {
            home_level: 3,
            ..ServeCitizen::ready(4, 5.0)
        });
        let decision = serve_food_to_citizen(&facts, &mut memory);
        assert_eq!(decision.refusal, Some(Refusal::PoorRestaurant));
        assert!(matches!(decision.effect, Some(Effect::ServePortion { .. })));
    }

    #[test]
    fn serving_a_player_transfers_up_to_sixteen_saturation_per_level() {
        // `:255-304`.
        let mut facts = CookFacts::new();
        facts.building_level = 2;
        let mut memory = CookMemory::default();
        let empty = serve_food_to_player(&facts, &mut memory);
        assert_eq!(empty.refusal, Some(Refusal::PlayerQueueEmpty));

        memory.queue_players(&[7]);
        facts.player_target = Some(ServePlayer::ready(7));
        facts.food_in_inventory = vec![FoodStack {
            nutrition: 4.0,
            count: 16,
        }];
        let served = serve_food_to_player(&facts, &mut memory);
        assert_eq!(
            served.next,
            Some(TradeState::Furnace(SmeltState::StartWorking)),
            "`:303`"
        );
        // 2 niveaux × 16 = 32 de saturation, à 4 par item : 8 items.
        assert_eq!(
            served.effect,
            Some(Effect::ServePlayer {
                player: 7,
                items: 8
            })
        );
        assert!(memory.players_to_serve.is_empty());
    }

    #[test]
    fn a_player_with_a_full_bag_or_left_the_hut_is_dropped() {
        // `:263-268` et `:279-282`.
        let mut facts = CookFacts::new();
        let mut memory = CookMemory::default();
        memory.queue_players(&[7]);
        facts.player_target = Some(ServePlayer {
            inventory_full: true,
            ..ServePlayer::ready(7)
        });
        let full = serve_food_to_player(&facts, &mut memory);
        assert_eq!(full.refusal, Some(Refusal::PlayerInventoryFull));
        assert_eq!(full.next, None, "`:281` getState()");
        // **Déjà sorti de la file** : `:275` fait `playerToServe.poll()` avant le test
        // `isItemHandlerFull` de `:279`. Mon premier test attendait que le joueur reste
        // en file : c'est mon attente qui était fausse, pas la règle.
        assert!(
            memory.players_to_serve.is_empty(),
            "`:275` a sorti le joueur avant le test de `:279`"
        );

        // Et s'il quitte la hutte avant d'être servi (`:263-268`) : il faut le remettre
        // en file, la sortie de `:275` l'a consommé.
        memory.queue_players(&[7]);
        facts.player_target = Some(ServePlayer {
            in_building: false,
            ..ServePlayer::ready(7)
        });
        let gone = serve_food_to_player(&facts, &mut memory);
        assert_eq!(gone.refusal, Some(Refusal::PlayerLeftBuilding));
        assert_eq!(
            gone.next,
            Some(TradeState::Furnace(SmeltState::StartWorking))
        );
        assert!(
            memory.players_to_serve.is_empty(),
            "`:266` a sorti le joueur"
        );
    }

    #[test]
    fn a_player_with_nothing_to_receive_empties_the_queue() {
        // `:292-296`.
        let mut facts = CookFacts::new();
        let mut memory = CookMemory::default();
        memory.queue_players(&[7, 8]);
        facts.player_target = Some(ServePlayer::ready(7));
        let decision = serve_food_to_player(&facts, &mut memory);
        assert_eq!(decision.refusal, Some(Refusal::NothingTransferred));
        assert_eq!(
            decision.next,
            Some(TradeState::Furnace(SmeltState::StartWorking))
        );
        assert!(memory.players_to_serve.is_empty(), "`:294` clear");
    }

    #[test]
    fn start_working_walks_complains_and_asks_in_java_s_order() {
        // `AbstractEntityAIUsesFurnace.java:182-261`.
        let mut facts = CookFacts::new();
        let mut memory = CookMemory::default();
        // :184-187 — hors de la hutte.
        facts.furnace.at_building = false;
        assert_eq!(
            start_working(&facts, &mut memory).refusal,
            Some(Refusal::Walking)
        );
        // :201-208 — aucun four.
        let mut no_furnace = CookFacts::without_furnace();
        no_furnace.furnace.at_building = true;
        let start = start_working(&no_furnace, &mut memory);
        assert!(start.complain_no_furnaces);
        assert_eq!(start.refusal, Some(Refusal::NoFurnace));
        assert_eq!(start.next, TradeState::Furnace(SmeltState::StartWorking));
    }

    #[test]
    fn an_empty_menu_asks_for_food_without_changing_the_state() {
        // `EntityAIWorkCook.java:149-157` : `requestSmeltable` lève et continue.
        let mut facts = CookFacts::new();
        let mut memory = CookMemory::default();
        facts.menu_empty = true;
        facts.furnace.smeltable_in_building = 0;
        facts.furnace.smeltable_in_inventory = 0;
        facts.furnace.fuel_in_building = 0;
        facts.furnace.fuel_in_inventory = 0;
        let start = start_working(&facts, &mut memory);
        assert!(start.ask_food, "`requestSmeltable` a été appelé");
        assert_eq!(start.ask_fuel, STACKSIZE, "un four, donc un stack de 64");
        // Le cuisinier ne s'arrête pas pour autant : il n'y a simplement plus rien à
        // mettre dans le four, ce que `checkIfAbleToSmelt` dit.
        assert_eq!(
            start.refusal,
            Some(Refusal::Furnace(
                crate::ai_work_04::states::Refusal::NothingToFill
            ))
        );
    }

    #[test]
    fn the_food_ceiling_stops_the_food_request() {
        // `:237-240` — `reachedMaxToKeep()` vrai bloque `requestSmeltable`.
        let mut facts = CookFacts::new();
        let mut memory = CookMemory::default();
        facts.furnace.smeltable_in_building = 0;
        facts.furnace.smeltable_in_inventory = 0;
        facts.food_kept = max_food_to_keep(1) + 1;
        assert!(!start_working(&facts, &mut memory).ask_food);
    }

    #[test]
    fn the_fuel_retrieval_wins_over_the_product_one() {
        // `:216-228` : le carburant refusé par le filtre passe avant les produits.
        let mut facts = CookFacts::new();
        let mut memory = CookMemory::default();
        facts.furnace.furnaces = vec![
            crate::ai_work_04::facts::Furnace::burning(Pos::new(1, 65, 1), 4, 8),
            crate::ai_work_04::facts::Furnace {
                fuel_present: true,
                fuel_allowed: false,
                ..crate::ai_work_04::facts::Furnace::empty(Pos::new(2, 65, 1))
            },
        ];
        let start = start_working(&facts, &mut memory);
        assert_eq!(
            start.next,
            TradeState::Furnace(SmeltState::RetrievingUsedFuelFromFurnace)
        );
        assert_eq!(start.walk_to, Some(Pos::new(2, 65, 1)));
    }

    #[test]
    fn a_hungry_citizen_beats_the_furnace_work() {
        // `:210-214` : `checkForImportantJobs()` passe avant tout le reste.
        let mut facts = CookFacts::new();
        let mut memory = CookMemory::default();
        facts.furnace.furnaces = vec![crate::ai_work_04::facts::Furnace::burning(
            Pos::new(1, 65, 1),
            4,
            8,
        )];
        facts.citizens = vec![CitizenHere::hungry(4)];
        let start = start_working(&facts, &mut memory);
        assert_eq!(start.next, TradeState::Cook(CookState::ServeFoodToCitizen));
        assert!(memory.citizens_to_serve == vec![4]);
    }

    #[test]
    fn a_hut_chest_full_of_food_sends_the_cook_gathering() {
        // `:249-253` — la hutte a de quoi cuire, les poches non.
        let mut facts = CookFacts::new();
        let mut memory = CookMemory::default();
        facts.furnace.smeltable_in_building = 12;
        facts.furnace.smeltable_in_inventory = 0;
        let start = start_working(&facts, &mut memory);
        assert_eq!(
            start.next,
            TradeState::Furnace(SmeltState::GatheringRequiredMaterials)
        );
        assert_eq!(start.gather, Some(Gather::Smeltable));
        // Et le carburant, s'il manque, prend le même chemin (`:254-258`).
        facts.furnace.smeltable_in_inventory = 12;
        facts.furnace.fuel_in_building = 8;
        facts.furnace.fuel_in_inventory = 0;
        assert_eq!(
            start_working(&facts, &mut memory).gather,
            Some(Gather::Fuel)
        );
    }

    #[test]
    fn a_furnace_that_wants_something_puts_the_cook_to_work() {
        // `:320-348` — `checkIfAbleToSmelt` répond `FILL_UP_FURNACES`.
        let mut facts = CookFacts::new();
        let mut memory = CookMemory::default();
        facts.furnace.smeltable_in_inventory = 8;
        facts.furnace.fuel_in_inventory = 8;
        let start = start_working(&facts, &mut memory);
        assert_eq!(start.next, TradeState::Furnace(SmeltState::FillUpFurnaces));
        assert_eq!(start.walk_to, Some(Pos::new(1, 65, 1)));
    }

    #[test]
    fn the_food_request_is_a_stack_at_the_hut_level() {
        // `:433-437` — `new Food(STACKSIZE, building.getBuildingLevel())`.
        assert_eq!(food_request(1), (64, 1));
        assert_eq!(food_request(4), (64, 4));
        assert_eq!(STACKSIZE, 64);
    }

    #[test]
    fn the_furnace_half_is_the_one_ai_work_04_already_answered() {
        // Le contrat de non-duplication : sur les mêmes faits, les deux scans et le
        // choix de four donnent la même réponse que le module voisin.
        let facts = CookFacts::new();
        let smelt = furnace_facts(&facts);
        assert_eq!(
            smelter::oven_to_retrieve_from(smelt),
            None,
            "le four vide de `SmelterFacts::new` n'a pas de résultat"
        );
        assert_eq!(smelter::oven_to_retrieve_fuel_from(smelt), None);
        assert_eq!(
            smelter::check_if_able_to_smelt(smelt).fill,
            Some(Pos::new(1, 65, 1)),
            "un four vide avec 8 de minerai et 8 de carburant veut les deux"
        );
        assert!(
            smelter::reached_max_to_keep(3),
            "trois piles libres : le parent dit plein"
        );
        assert!(!smelter::reached_max_to_keep(27));
        assert_eq!(smelter::acceleration_ticks(50), 10, "`(50 / 10) * 2`");
    }
}
