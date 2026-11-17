//! Ce que ces trois buts demandent au monde, écrit comme des observations.
//!
//! Java référence : `core/entity/ai/workers/production/herders/EntityAIWorkSwineHerder
//! .java` (47 lignes), `service/EntityAIWorkCook.java` (438) et
//! `service/EntityAIWorkDeliveryman.java` (662), plus les trois classes mères dont ils
//! héritent réellement — `production/herders/AbstractEntityAIHerder.java` (874),
//! `workers/AbstractEntityAIUsesFurnace.java` (507) et `AbstractEntityAIInteract.java`.
//!
//! **La moitié de base ne se réécrit pas.** Le citoyen, son inventaire, sa hutte et sa
//! carte de blocs sont dans `crate::ai::facts` ([`WorldFacts`], [`Pos`]), et
//! [`Scene::facts`] est *cette même valeur*. La moitié métier manquait : `JobFacts` n'a
//! que deux variantes et son fichier appartient à un autre module, donc les faits de
//! ces trois métiers vivent dans [`TradeJob`], que [`Scene`] ajoute à côté.
//!
//! **Ce qui est un fait et ce qui est une décision.** Une observation que le monde
//! répond est un fait (`CitizenHere::saturation` = `citizenData.getSaturation()`) ;
//! ce que l'IA écrit est une mémoire (`CookMemory::citizens_to_serve` =
//! `citizenToServe`, le champ `EntityAIWorkCook.java:74`). La distinction compte : la
//! file est *remplie* par `checkForImportantJobs` et *vidée* par `serveFoodToCitizen`,
//! donc elle appartient à l'IA et survit au tick.
//!
//! **Les sous-systèmes répondent, ils ne décident pas.** `FoodUtils.getBestFoodForCitizen`
//! (206 lignes de score de diversité et de qualité), `FoodUtils.hasBestOptionInInv`,
//! `FoodUtils.checkForFoodInBuilding` et `InventoryUtils.hasItemInProvider` sont des
//! règles qui vivent dans `api/util/` et que le port lit comme des **réponses** — un
//! index de slot, trois booléens — exactement comme `crate::ai::facts::Placement` lit la
//! réponse du placeur de structure. Rejouer le score de `getBestFoodForCitizen` serait
//! porter deux cents lignes pour aucun état de plus.

use crate::ai::facts::{Pos, WorldFacts};
use crate::ai::worker::Memory;
use crate::ai_build::delivery::DeliveryFacts;
use crate::ai_work_04::facts::SmelterFacts;
use crate::ai_work_09::facts::{HerdFacts, HerdMemory};

/// Ce qu'`EntityAIWorkSwineHerder` ajoute aux faits d'élevage.
///
/// Java référence : `EntityAIWorkSwineHerder.java:25-34` (`updateRenderMetaData` fait
/// deux lectures : l'état et `hasItemInInventory(Items.CARROT)`) et `:42-46`
/// (`getButcheringAttackDamage` lit `getPrimarySkillLevel()`). Le reste de son but d'IA
/// **est** celui d'`AbstractEntityAIHerder`, dont les faits sont déjà
/// [`HerdFacts`] : les recopier ici serait la deuxième copie d'une structure de
/// dix-huit champs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SwineFacts {
    /// La moitié élevage : `AbstractEntityAIHerder` (la classe mère,
    /// `EntityAIWorkSwineHerder.java:16`).
    pub herd: HerdFacts,
    /// `worker.getCitizenInventoryHandler().hasItemInInventory(Items.CARROT)`
    /// (`EntityAIWorkSwineHerder.java:29`) : la carotte en poche se voit sur le dos du
    /// citoyen (`:31`) — et c'est aussi l'item d'accouplement que
    /// `BuildingSwineHerder.canEat` retire de son alimentation.
    pub carrot_in_inventory: bool,
    /// `worker.getMainHandItem()` non vide (`AbstractEntityAIHerder.java:807`) : sans
    /// objet en main, `butcherAnimal` ne frappe pas du tout.
    pub main_hand_full: bool,
}

impl SwineFacts {
    /// Un élevage de porcs sans enclos, sans carotte, les mains vides.
    pub fn new(herd: HerdFacts) -> SwineFacts {
        SwineFacts {
            herd,
            carrot_in_inventory: false,
            main_hand_full: true,
        }
    }
}

/// Un citoyen présent dans la hutte, tel que `checkForImportantJobs` le lit.
///
/// Java référence : `EntityAIWorkCook.java:368-390` — `getEntitiesWithinBuilding(...,
/// EntityCitizen.class, ...)`, puis les quatre tests du `continue` (`:370-375`) et les
/// deux réponses de `:377-389`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CitizenHere {
    /// `citizen.getUUID()`, réduit à la clé que l'appelant sait fournir : c'est ce qui
    /// permet à [`CookMemory::citizens_to_serve`] de survivre au tick.
    pub id: u32,
    /// `citizen.getCitizenJobHandler().getColonyJob() instanceof JobCook` (`:370`) : le
    /// cuisinier ne se sert pas lui-même (et ne sert pas un confrère).
    pub cook: bool,
    /// `citizenData.isWorking()` (`:422`).
    pub working: bool,
    /// `citizenData.getSaturation()` (`:423`).
    pub saturation: f64,
    /// `citizenData.justAte()` (`:424`).
    pub just_ate: bool,
    /// `InventoryUtils.hasItemInItemHandler(citizen.getItemHandlerCitizen(),
    /// stack -> canEat(stack, citizen))` (`:372`).
    pub has_menu_food: bool,
    /// `FoodUtils.hasBestOptionInInv(worker inv, citizenData, menu, building)` (`:377`) :
    /// le cuisinier a de quoi le servir dans son propre sac.
    pub has_best_option: bool,
    /// `FoodUtils.checkForFoodInBuilding(citizenData, menu, building) != null` (`:383`) :
    /// la hutte a ce qu'il faut, il doit aller le chercher.
    pub food_in_building: bool,
}

impl CitizenHere {
    /// Un citoyen affamé, au travail à l'extérieur, que la hutte peut nourrir.
    pub fn hungry(id: u32) -> CitizenHere {
        CitizenHere {
            id,
            cook: false,
            working: false,
            saturation: 4.0,
            just_ate: false,
            has_menu_food: false,
            has_best_option: true,
            food_in_building: false,
        }
    }
}

/// Le citoyen en tête de `citizenToServe`, observé au tick où il est servi.
///
/// Java référence : `EntityAIWorkCook.java:172-246` — `isInBuilding` (`:172`),
/// `walkToWorkPos` (`:179`), `hasSpace` (`:190`), `hasItemInItemHandler(handler,
/// canEat)` (`:215`), `getBestFoodForCitizen` (`:220`), `getFoodValue` (`:237`) et
/// `getHomeBuilding().getBuildingLevelEquivalent()` (`:231`).
///
/// **La file de Java est une file d'entités** ; ici c'est une file d'identifiants
/// ([`CookMemory::citizens_to_serve`]) et le tick reçoit l'observation de celui qui est
/// en tête. C'est la seule façon de rejouer la décision sans un monde.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ServeCitizen {
    /// La clé du citoyen en tête de file.
    pub id: u32,
    /// `building.isInBuilding(citizen.blockPosition())` (`:172`).
    pub in_building: bool,
    /// `walkToWorkPos(citizen.blockPosition())` (`:179`) : le cuisinier est arrivé à sa
    /// portée.
    pub at_work_pos: bool,
    /// `citizen.getCitizenData().getSaturation()` (`:237`).
    pub saturation: f64,
    /// `handler.hasSpace()` (`:190`).
    pub has_space: bool,
    /// `InventoryUtils.hasItemInItemHandler(handler, canEat)` (`:215`).
    pub has_menu_food: bool,
    /// `FoodUtils.getBestFoodForCitizen(worker inv, citizenData, menu)`, `None` pour
    /// `-1` (`:194`, `:220`).
    pub food_slot: Option<u32>,
    /// `FoodUtils.getFoodValue(worker-inventory stack, worker)` de ce slot (`:237`) : le
    /// nombre de points de saturation qu'un item rend.
    pub food_value: f64,
    /// `citizenData.getHomeBuilding().getBuildingLevelEquivalent()`, `0` sans maison
    /// (`:231`).
    pub home_level: u32,
    /// `InventoryUtils.getItemCountInItemHandler(worker inv, canEat)` (`:223`) : ce qui
    /// reste de mangeable dans le sac quand le slot est `-1`.
    pub worker_menu_food: u32,
}

impl ServeCitizen {
    /// Un citoyen à portée, affamé, dont le cuisinier a le plat.
    pub fn ready(id: u32, food_value: f64) -> ServeCitizen {
        ServeCitizen {
            id,
            in_building: true,
            at_work_pos: true,
            saturation: 0.0,
            has_space: true,
            has_menu_food: false,
            food_slot: Some(3),
            food_value,
            home_level: 0,
            worker_menu_food: 8,
        }
    }
}

/// Le joueur en tête de `playerToServe`, observé au tick où il est servi.
///
/// Java référence : `EntityAIWorkCook.java:263-296` — `isInBuilding` (`:263`),
/// `walkToWorkPos` (`:270`), `isItemHandlerFull(player inv)` (`:279`) et
/// `hasItemInItemHandler(player inv, canEat)` (`:283`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ServePlayer {
    /// La clé du joueur en tête de file.
    pub id: u32,
    /// `building.isInBuilding(player.blockPosition())` (`:263`).
    pub in_building: bool,
    /// `walkToWorkPos(player.blockPosition())` (`:270`).
    pub at_work_pos: bool,
    /// `InventoryUtils.isItemHandlerFull(new InvWrapper(player.getInventory()))`
    /// (`:279`).
    pub inventory_full: bool,
    /// `InventoryUtils.hasItemInItemHandler(handler, canEat)` (`:283`).
    pub has_menu_food: bool,
}

impl ServePlayer {
    /// Un joueur à portée, avec de la place et rien dans son sac.
    pub fn ready(id: u32) -> ServePlayer {
        ServePlayer {
            id,
            in_building: true,
            at_work_pos: true,
            inventory_full: false,
            has_menu_food: false,
        }
    }
}

/// Une pile de nourriture du sac du cuisinier, telle que
/// `InventoryUtils.transferFoodUpToSaturation` la lit.
///
/// Java référence : `InventoryUtils.java:3222-3282` — `getStackInSlot`
/// (`:3225`), `getFoodProperties(stack, null)` (`:3231`) et
/// `itemFood.getNutrition()` (`:3237`, `:3243`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FoodStack {
    /// `foodProperties.getNutrition()` : les points de saturation par item.
    pub nutrition: f64,
    /// `stack.getCount()`.
    pub count: u32,
}

/// Tout ce qu'`EntityAIWorkCook` lit et que les faits de base ne portent pas.
///
/// Java référence : `startWorking` d'`AbstractEntityAIUsesFurnace.java:182-261` (dont la
/// moitié four est [`SmelterFacts`], déjà portée), puis les six remplacements du
/// cuisinier — `isSmeltable` (`:130-134`), `reachedMaxToKeep` (`:136-147`),
/// `requestSmeltable` (`:149-157`), `extractFromFurnace` (`:113-121`),
/// `checkForImportantJobs` (`:338-412`), `getActionsDoneUntilDumping` (`:427-431`) — et
/// `getSmeltAbleClass` (`:433-437`).
#[derive(Clone, Debug, PartialEq)]
pub struct CookFacts {
    /// La moitié four : les six cibles d'`AbstractEntityAIUsesFurnace.java:76-82` lisent
    /// exactement ces observations, et `crate::ai_work_04::smelter` porte déjà leurs
    /// règles. Les champs de `SmelterFacts` propres au mineur d'`EntityAIWorkSmelter`
    /// (`ore_recipe`, `ore_outside_list`, `ore_fulfilled`, …) restent à leur défaut : le
    /// cuisinier ne les lit pas.
    pub furnace: SmelterFacts,
    /// `building.getBuildingLevel()` (`:143`, `:231`, `:350`, `:436`).
    pub building_level: u32,
    /// `menuModule.getMenu().isEmpty()` (`:153`) : sans menu, il n'y a rien à cuisiner
    /// ni à servir, et la requête de nourriture ne peut pas être formulée.
    pub menu_empty: bool,
    /// Un plat du menu est un `IMinecoloniesFoodItem` (`:352-360`), testé seulement au
    /// niveau 3 et plus.
    pub menu_has_minecolonies_food: bool,
    /// `WorldUtil.getEntitiesWithinBuilding(world, EntityCitizen.class, building, null)`
    /// (`:368`), dans l'ordre où le monde les rend — l'ordre est la règle : le premier
    /// citoyen à qui il manque un plat renvoie la décision.
    pub citizens: Vec<CitizenHere>,
    /// `WorldUtil.getEntitiesWithinBuilding(world, Player.class, building, predicate)`
    /// (`:341-345`), déjà filtré par `getFoodLevel() < LEVEL_TO_FEED_PLAYER` et par
    /// `hasPermission(player, Action.MANAGE_HUTS)`. Une clé par joueur : ils sont tous
    /// mis en file (`:347`).
    pub players: Vec<u32>,
    /// `InventoryUtils.getItemCountInItemHandler(worker inv, canEat)` —
    /// `hasItemInItemHandler(worker.getInventoryCitizen(), foodPredicate)` (`:400`) et le
    /// compte de `:223`.
    pub worker_menu_food: u32,
    /// `InventoryUtils.hasItemInProvider(building, foodPredicate)` (`:402`).
    pub building_has_menu_food: bool,
    /// `InventoryUtils.getCountFromBuildingWithLimit(building, EDIBLE && canEatLevel(level
    /// - 1), stack -> stack.getMaxStackSize() * 6)` (`:144-146`) : ce que la hutte garde
    /// en réserve de mangeable, plafonné par pile.
    pub food_kept: u32,
    /// Les piles mangeables du sac du cuisinier, dans l'ordre des slots : c'est ce
    /// qu'`InventoryUtils.transferFoodUpToSaturation` parcourt (`:288`).
    pub food_in_inventory: Vec<FoodStack>,
    /// L'observation du citoyen en tête de file, `None` quand la file est vide.
    pub citizen_target: Option<ServeCitizen>,
    /// L'observation du joueur en tête de file, `None` quand la file est vide.
    pub player_target: Option<ServePlayer>,
}

impl CookFacts {
    /// Un cuisinier de niveau 1, dans sa hutte, avec un four vide, un menu non vide et
    /// personne à servir.
    pub fn new() -> CookFacts {
        CookFacts {
            furnace: SmelterFacts::new(),
            building_level: 1,
            menu_empty: false,
            menu_has_minecolonies_food: false,
            citizens: Vec::new(),
            players: Vec::new(),
            worker_menu_food: 0,
            building_has_menu_food: false,
            food_kept: 0,
            food_in_inventory: Vec::new(),
            citizen_target: None,
            player_target: None,
        }
    }

    /// La même chose, sans four : le cas d'une hutte qu'on vient de poser.
    pub fn without_furnace() -> CookFacts {
        let mut facts = CookFacts::new();
        facts.furnace.furnaces.clear();
        facts
    }
}

impl Default for CookFacts {
    /// Les faits nus : `SmelterFacts::default()`, donc hors de la hutte, sans four.
    fn default() -> CookFacts {
        CookFacts {
            furnace: SmelterFacts::default(),
            building_level: 1,
            menu_empty: false,
            menu_has_minecolonies_food: false,
            citizens: Vec::new(),
            players: Vec::new(),
            worker_menu_food: 0,
            building_has_menu_food: false,
            food_kept: 0,
            food_in_inventory: Vec::new(),
            citizen_target: None,
            player_target: None,
        }
    }
}

/// Les deux files du cuisinier : ses seuls champs d'IA propres.
///
/// Java référence : `EntityAIWorkCook.java:74` (`citizenToServe`) et `:79`
/// (`playerToServe`), deux `ArrayDeque`. **Java ne les vide pas au début de
/// `checkForImportantJobs`** : il ajoute (`:347`, `:379`) et ne retire qu'en servant
/// (`:175`, `:184`, `:266`, `:275`) ou en abandonnant (`:225`, `:294`). Le port copie ce
/// comportement, y compris sa conséquence : un citoyen encore dans la file au tick
/// suivant y est ajouté **une seconde fois**. Voir `MANQUANT.md` §3.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CookMemory {
    /// `citizenToServe` (`:74`), dans l'ordre d'insertion.
    pub citizens_to_serve: Vec<u32>,
    /// `playerToServe` (`:79`), dans l'ordre d'insertion.
    pub players_to_serve: Vec<u32>,
}

impl CookMemory {
    /// `citizenToServe.add(citizen)` (`:379`).
    pub fn queue_citizen(&mut self, id: u32) {
        self.citizens_to_serve.push(id);
    }

    /// `playerToServe.addAll(playerList)` (`:347`).
    pub fn queue_players(&mut self, ids: &[u32]) {
        self.players_to_serve.extend_from_slice(ids);
    }

    /// `citizenToServe.poll()` (`:175`, `:184`).
    pub fn poll_citizen(&mut self) -> Option<u32> {
        if self.citizens_to_serve.is_empty() {
            return None;
        }
        Some(self.citizens_to_serve.remove(0))
    }

    /// `playerToServe.poll()` (`:266`, `:275`).
    pub fn poll_player(&mut self) -> Option<u32> {
        if self.players_to_serve.is_empty() {
            return None;
        }
        Some(self.players_to_serve.remove(0))
    }

    /// `citizenToServe.peek()` (`:172`, `:179`).
    pub fn first_citizen(&self) -> Option<u32> {
        self.citizens_to_serve.first().copied()
    }

    /// `citizenToServe.clear()` (`:225`).
    pub fn clear_citizens(&mut self) {
        self.citizens_to_serve.clear();
    }

    /// `playerToServe.clear()` (`:294`).
    pub fn clear_players(&mut self) {
        self.players_to_serve.clear();
    }
}

/// La moitié métier d'une scène.
///
/// Un appelant qui donne à un cuisinier les faits d'un livreur reçoit
/// [`super::state::Refusal::NoFactsForTrade`], pas un défaut silencieux : la différence
/// entre « ce cuisinier n'a personne à servir » et « ces faits sont ceux d'un autre
/// métier » ne doit pas se perdre.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: the three trades of this slice
pub enum TradeJob {
    /// L'éleveur de porcs (`EntityAIWorkSwineHerder.java:16`).
    Swine(SwineFacts),
    /// Le cuisinier (`EntityAIWorkCook.java:54`).
    Cook(CookFacts),
    /// Le livreur (`EntityAIWorkDeliveryman.java:57`) — ses faits sont déjà portés par
    /// `crate::ai_build::delivery`, et ce module ne les réécrit pas.
    Deliveryman(DeliveryFacts),
}

impl Default for TradeJob {
    /// Le premier métier de la tranche, faits vides.
    fn default() -> TradeJob {
        TradeJob::Swine(SwineFacts::default())
    }
}

impl TradeJob {
    /// Les faits d'élevage, si ce métier en a.
    pub fn swine(&self) -> Option<&SwineFacts> {
        match self {
            TradeJob::Swine(facts) => Some(facts),
            _ => None,
        }
    }

    /// Les faits du cuisinier, si c'est un cuisinier.
    pub fn cook(&self) -> Option<&CookFacts> {
        match self {
            TradeJob::Cook(facts) => Some(facts),
            _ => None,
        }
    }

    /// Les faits du livreur, si c'est un livreur.
    pub fn courier(&self) -> Option<&DeliveryFacts> {
        match self {
            TradeJob::Deliveryman(facts) => Some(facts),
            _ => None,
        }
    }

    /// Le métier de ces faits.
    pub fn trade(&self) -> super::Trade {
        match self {
            TradeJob::Swine(_) => super::Trade::SwineHerder,
            TradeJob::Cook(_) => super::Trade::Cook,
            TradeJob::Deliveryman(_) => super::Trade::Deliveryman,
        }
    }
}

/// Ce qu'une de ces trois IA retient entre deux ticks.
///
/// `delay`, `hasDelayed`, `walk_to` et `need` sont ceux de la classe de base et sont
/// réutilisés depuis [`Memory`] : `setDelay`, `walkTo` et `needsCurrently` sont les mêmes
/// champs pour les trois métiers. [`HerdMemory`] vient de `crate::ai_work_09` (les
/// `breedTimeOut`, `animalsToBreed` et `fedRecently` de l'éleveur), [`CookMemory`] est
/// neuf, et le livreur n'a **aucune** mémoire propre : `EntityAIWorkDeliveryman` ne
/// déclare que quatre constantes et deux files de requêtes qui vivent dans le job.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TradeMemory {
    /// `AbstractEntityAIBasic.delay`, `.hasDelayed`, `.walkTo`, `.needsCurrently`
    /// (`:121`, `:126`, `:161`, `:156`).
    pub base: Memory,
    /// Ce que l'éleveur retient (`AbstractEntityAIHerder.java:114-129`).
    pub herd: HerdMemory,
    /// Ce que le cuisinier retient (`EntityAIWorkCook.java:74`, `:79`).
    pub cook: CookMemory,
}

/// La scène entière qu'une de ces décisions lit : les faits de base, les faits du
/// métier, et la mémoire de l'IA.
///
/// [`WorldFacts`] est *la même valeur* que prennent les tables de `crate::ai` — rien n'en
/// est copié — donc un appelant qui fait aussi tourner `crate::ai::worker::JobAi` peut
/// donner à chacun celle qu'il a déjà remplie.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    /// Le citoyen, son inventaire, sa hutte et ses blocs (`crate::ai::facts`).
    pub facts: WorldFacts,
    /// Les observations du métier.
    pub job: TradeJob,
    /// Ce que l'IA retient entre deux ticks.
    pub memory: TradeMemory,
    /// Le dernier refus, effacé au début de chaque tick.
    pub refusal: Option<super::state::Refusal>,
    /// La dernière quantité décidée, effacée au début de chaque tick. Voir
    /// [`super::Effect`] : trois branches du cuisinier décident d'un nombre d'items qui ne
    /// se relit nulle part.
    pub effect: Option<super::Effect>,
}

impl Scene {
    /// Une scène sur `facts` avec une mémoire vide et `job`, les faits du métier.
    pub fn new(facts: WorldFacts, job: TradeJob) -> Scene {
        Scene {
            facts,
            job,
            memory: TradeMemory::default(),
            refusal: None,
            effect: None,
        }
    }

    /// Les faits d'élevage, si c'est un éleveur de porcs.
    pub fn swine(&self) -> Option<&SwineFacts> {
        self.job.swine()
    }

    /// Les faits du cuisinier, si c'est un cuisinier.
    pub fn cook(&self) -> Option<&CookFacts> {
        self.job.cook()
    }

    /// Les faits du livreur, si c'est un livreur.
    pub fn courier(&self) -> Option<&DeliveryFacts> {
        self.job.courier()
    }

    /// Enregistrer pourquoi ce tick ne peut pas avancer. Le dernier gagne, comme
    /// `crate::ai::worker::Scene::refuse`.
    pub fn refuse(&mut self, refusal: super::state::Refusal) {
        self.refusal = Some(refusal);
    }

    /// `walkToBuilding()` (`AbstractEntityAIBasic.java:757-766`) : le citoyen est à
    /// moins de `WOKR_IN_BUILDING_DIST` de sa hutte, le même test que la table de base.
    pub fn at_hut(&self) -> bool {
        self.facts.worker.at.is_living_at_site(
            self.facts.building.position,
            crate::ai::worker::WOKR_IN_BUILDING_DIST,
        )
    }

    /// `walkToWorkPos(pos)` (`AbstractEntityAIBasic.java:800-806`) : à portée d'une
    /// position de travail, le test de `EntityAIWorkCook.java:179`, `:270`.
    pub fn at_work_pos(&self, pos: Pos) -> bool {
        self.facts
            .worker
            .at
            .is_living_at_site(pos, crate::ai::worker::WOKR_IN_BUILDING_DIST)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_swine_scene_carries_the_herd_facts_and_refuses_the_others() {
        let scene = Scene::new(WorldFacts::new(), TradeJob::Swine(SwineFacts::default()));
        assert!(scene.swine().is_some());
        assert!(scene.cook().is_none());
        assert!(scene.courier().is_none());
        assert_eq!(scene.job.trade(), super::super::Trade::SwineHerder);
    }

    #[test]
    fn the_cook_queues_are_fifo_and_can_be_cleared() {
        // `ArrayDeque.add` en queue et `poll` en tête (`EntityAIWorkCook.java:347`,
        // `:175`, `:225`).
        let mut memory = CookMemory::default();
        memory.queue_players(&[7, 9]);
        memory.queue_citizen(3);
        memory.queue_citizen(4);
        assert_eq!(memory.first_citizen(), Some(3));
        assert_eq!(memory.poll_citizen(), Some(3));
        assert_eq!(memory.poll_citizen(), Some(4));
        assert_eq!(memory.poll_citizen(), None);
        assert_eq!(memory.poll_player(), Some(7));
        memory.clear_players();
        assert!(memory.players_to_serve.is_empty());
        memory.queue_citizen(5);
        memory.clear_citizens();
        assert!(memory.citizens_to_serve.is_empty());
    }

    #[test]
    fn queueing_the_same_citizen_twice_keeps_both_copies() {
        // Java n'a pas de test de doublon dans `:379` ; le port le copie (voir la note
        // de `CookMemory` et `MANQUANT.md` §3).
        let mut memory = CookMemory::default();
        memory.queue_citizen(3);
        memory.queue_citizen(3);
        assert_eq!(memory.citizens_to_serve, vec![3, 3]);
    }

    #[test]
    fn a_fresh_cook_has_a_furnace_an_empty_menu_false_and_nobody_to_serve() {
        // `CookFacts::new` : niveau 1, un four vide (`SmelterFacts::new`), personne.
        let facts = CookFacts::new();
        assert_eq!(facts.building_level, 1);
        assert_eq!(facts.furnace.furnaces.len(), 1);
        assert!(facts.furnace.furnaces[0].entity);
        assert!(!facts.menu_empty);
        assert!(facts.citizens.is_empty());
        assert!(facts.players.is_empty());
        assert_eq!(facts.citizen_target, None);
        // Et la variante sans four, qui est le cas d'une hutte neuve.
        assert!(CookFacts::without_furnace().furnace.furnaces.is_empty());
    }

    #[test]
    fn a_scene_measures_the_distance_to_its_hut_and_to_a_work_position() {
        // `walkToBuilding` (`AbstractEntityAIBasic.java:757-766`) et `walkToWorkPos`
        // (`:800-806`) partagent `EntityUtils.isLivingAtSite` (`EntityUtils.java:
        // 301-305`) : strictement moins de 7 blocs.
        let mut scene = Scene::default();
        scene.facts.building.position = Pos::new(0, 64, 0);
        scene.facts.worker.at = Pos::new(3, 64, 0);
        assert!(scene.at_hut());
        assert!(scene.at_work_pos(Pos::new(3, 68, 0)), "4 blocs en Y");
        scene.facts.worker.at = Pos::new(7, 64, 0);
        assert!(!scene.at_hut(), "7 n'est pas strictement moins que 7");
        scene.facts.worker.at = Pos::new(6, 64, 0);
        assert!(scene.at_hut());
    }

    #[test]
    fn a_hungry_citizen_is_the_one_the_cook_should_feed() {
        let citizen = CitizenHere::hungry(11);
        assert!(!citizen.cook);
        assert!(!citizen.working);
        assert!(!citizen.just_ate);
        assert!(!citizen.has_menu_food);
        assert!(citizen.has_best_option);
        let target = ServeCitizen::ready(11, 5.0);
        assert_eq!(target.id, 11);
        assert!(target.in_building);
        assert!(target.at_work_pos);
        assert_eq!(target.food_slot, Some(3));
        let player = ServePlayer::ready(2);
        assert!(!player.inventory_full);
        assert!(!player.has_menu_food);
    }
}
