//! Ce que ces trois buts demandent au monde, écrit comme des observations.
//!
//! Java référence : `core/entity/ai/workers/production/agriculture/EntityAIWorkPlanter.java`
//! (490 lignes) et `core/entity/ai/workers/production/herders/AbstractEntityAIHerder.java`
//! (874) avec ses deux sous-classes `EntityAIWorkChickenHerder.java` (74) et
//! `EntityAIWorkCowboy.java` (270), plus les deux modules de champ et d'élevage qu'ils
//! interrogent (`api/colony/buildingextensions/plantation/IPlantationModule.java`,
//! `core/colony/buildings/modules/AnimalHerdingModule.java`).
//!
//! **Pourquoi la moitié de base est réutilisée et la moitié métier non.** Le citoyen,
//! son inventaire, sa hutte et la carte de blocs existent déjà dans `crate::ai::facts`
//! et ce module les lit là-bas ([`WorldFacts`], [`Pos`]). Ce qui n'existe pas, c'est la
//! moitié métier : `WorldFacts` porte un `JobFacts` à exactement deux variantes
//! (composteur, constructeur) et ce fichier est hors de la limite d'écriture de ce
//! module, donc les faits d'un de ces métiers vivent dans [`WorkJob`] et
//! [`WorkScene`] **enveloppe** `WorldFacts` au lieu de l'étendre. Rien n'est copié :
//! `WorkScene::facts` est la même valeur `WorldFacts`.
//!
//! **Ce qui est un fait et ce qui est une mémoire.** Une observation que le monde
//! répond est un fait (`PlanterFacts::plant_count` = ce que contient l'inventaire) ;
//! ce que l'IA écrit elle-même est une mémoire (`PlanterMemory::active_result` =
//! `activeModuleResult`, `EntityAIWorkPlanter.java:66`). La distinction n'est pas
//! cosmétique : `activeModuleResult` est *assigné* par `decideFieldWork` (`:197`) et
//! *effacé* par `workField` (`:246`, `:251`), donc il appartient à l'IA.
//!
//! **Le module répond, il ne décide pas.** `decideFieldWork(world, position)` et
//! `getNextWorkingPosition(world)` sont des appels au module de plantation, qui a ses
//! propres règles (`PercentageHarvestPlantModule`, `BoneMealedPlantModule`, …). Le
//! port ne les rejoue pas : il lit la **réponse** ([`FieldWork`]), exactement comme
//! `crate::ai::facts::Placement` lit la réponse du placeur de structure.

use crate::ai::facts::{Pos, WorldFacts};
use crate::ai::worker::Memory;

/// `IPlantationModule.ActionToPerform` (`IPlantationModule.java:174-204`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
// exhaustive: the five actions Java declares
pub enum PlantAction {
    /// `NONE` (`:176`) : rien à faire ici, et le champ est rendu.
    #[default]
    None,
    /// `PLANT` (`:177`).
    Plant,
    /// `BONEMEAL` (`:178`).
    Bonemeal,
    /// `HARVEST` (`:179`).
    Harvest,
    /// `CLEAR` (`:180`) : un bloc obstrue la position de travail.
    Clear,
}

impl PlantAction {
    /// L'identifiant Java.
    pub fn java(self) -> &'static str {
        match self {
            PlantAction::None => "NONE",
            PlantAction::Plant => "PLANT",
            PlantAction::Bonemeal => "BONEMEAL",
            PlantAction::Harvest => "HARVEST",
            PlantAction::Clear => "CLEAR",
        }
    }

    /// `ActionToPerform.increasesActionCount` (`:200-203`) : `true` sauf pour `NONE`
    /// et `CLEAR` (`:176-180`).
    pub fn increases_action_count(self) -> bool {
        !matches!(self, PlantAction::None | PlantAction::Clear)
    }
}

/// `IPlantationModule.PlanterAIModuleResultResetState` (`IPlantationModule.java:
/// 155-169`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
// exhaustive: the three reset states Java declares
pub enum ResetState {
    /// `NONE` (`:160`) : ne rien remettre à zéro.
    #[default]
    None,
    /// `POSITION` (`:164`) : rendre la position courante.
    Position,
    /// `FIELD` (`:168`) : rendre le champ courant.
    Field,
}

impl ResetState {
    /// `shouldResetWorkingPosition()` (`:310-313`) : `POSITION` **ou** `FIELD`.
    pub fn resets_position(self) -> bool {
        matches!(self, ResetState::Position | ResetState::Field)
    }

    /// `shouldResetCurrentField()` (`:320-323`) : `FIELD` seulement.
    pub fn resets_field(self) -> bool {
        self == ResetState::Field
    }
}

/// La réponse d'un module de plantation pour une position de travail — tout ce que
/// l'IA du planteur a besoin de savoir de lui.
///
/// Java référence : `IPlantationModule.PlantationModuleResult` (`:209-323`), construite
/// par `decideFieldWork` puis lue par `workField` (`EntityAIWorkPlanter.java:206-256`).
/// Le port lit la réponse, pas la règle : c'est le module qui décide qu'une position
/// se récolte ou se bêche.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldWork {
    /// `getWorkingPosition()` : la position que `getNextWorkingPosition` a choisie.
    pub working_position: Pos,
    /// `getAction()` : quoi faire.
    pub action: PlantAction,
    /// `getActionPosition()` : où le faire, `None` quand il n'y a rien à faire.
    pub action_position: Option<Pos>,
    /// `getResetState()` : ce qu'il faut rendre ensuite.
    pub reset: ResetState,
}

impl FieldWork {
    /// La réponse `PlantationModuleResult.NONE` (`IPlantationModule.java:214`) : aucune
    /// action, et le champ est rendu (`new Builder().pickNewField()`).
    pub fn none(working_position: Pos) -> FieldWork {
        FieldWork {
            working_position,
            action: PlantAction::None,
            action_position: None,
            reset: ResetState::Field,
        }
    }

    /// `new Builder().plant(position)` avec un reset de position (`:361-366`), la
    /// combinaison la plus courante d'un module de plantation.
    pub fn plant(working_position: Pos, at: Pos) -> FieldWork {
        FieldWork {
            working_position,
            action: PlantAction::Plant,
            action_position: Some(at),
            reset: ResetState::Position,
        }
    }

    /// `shouldResetWorkingPosition()` (`:310-313`).
    pub fn should_reset_working_position(&self) -> bool {
        self.reset.resets_position()
    }

    /// `shouldResetCurrentField()` (`:320-323`).
    pub fn should_reset_current_field(&self) -> bool {
        self.reset.resets_field()
    }
}

/// Le champ courant, tel que `PlantationField` le tient.
///
/// Java référence : `core/colony/buildingextensions/PlantationField.java` (`getPosition`,
/// `getModule`) et `IPlantationModule` (`getNextWorkingPosition`, `getActionLimit`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FieldFacts {
    /// `PlantationField.getPosition()` — où le planteur marche (`EntityAIWorkPlanter.java:
    /// 167`).
    pub position: Pos,
    /// `getModule().getActionLimit()` (`:239`, `:458`) : le nombre d'actions avant de
    /// rendre le champ. Ce n'est **pas** une constante du port : chaque module répond
    /// la sienne (5, 1, 10, 10, 5 — voir [`super::planter::ACTION_LIMITS`]).
    pub action_limit: u32,
    /// `getNextWorkingPosition(world)`, `None` quand le champ est fini
    /// (`:189-194`).
    pub work_position: Option<Pos>,
}

/// Tout ce qu'`EntityAIWorkPlanter` lit et que les faits de base ne portent pas.
///
/// Java référence : `EntityAIWorkPlanter.java` — `pickField` (`:102-142`),
/// `moveToField` (`:159-173`), `decideFieldWork` (`:180-199`), `workField`
/// (`:206-256`), `returnToBuilding` (`:263-276`), `checkIfItemsUnavailable`
/// (`:393-405`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlanterFacts {
    /// `building.getBuildingLevel()` (`:105`) : le niveau 0 est une hutte que personne
    /// n'a construite.
    pub building_level: u32,
    /// `module.hasNoExtensions()` inversé (`:112`) : la hutte a au moins un champ.
    pub has_extensions: bool,
    /// `module.getCurrentExtension()` **lu avant** `getExtensionToWorkOn()`
    /// (`:123-124`) : c'est le champ d'avant la sélection, et la comparaison des deux
    /// est la règle qui remet `currentFieldActionCount` à zéro (`:131-134`).
    pub previous_field: Option<Pos>,
    /// `module.getExtensionToWorkOn()` (`:124`) : le champ à travailler, avec sa
    /// position de travail. `None` est la branche `:141` qui rend `IDLE`.
    pub field: Option<FieldFacts>,
    /// Ce que `decideFieldWork(world, position)` a répondu pour
    /// `field.work_position` (`:196-197`).
    pub decided: Option<FieldWork>,
    /// `walkToSafePos(field.getPosition())` (`:167-170`) : il est arrivé au champ.
    pub at_field: bool,
    /// `walkToSafePos(module.getPositionToWalkTo(world, actionPosition))`
    /// (`:209-212`) : il est arrivé là où l'action se fait.
    pub at_action: bool,
    /// `holdEfficientTool(state, pos)` (`:366-369`) : il tient un outil assez bon.
    pub has_tool: bool,
    /// `getItemCountInItemHandler(inventory, deliverable::matches)` pour la graine du
    /// module (`:395-397`).
    pub plant_count: u32,
    /// `module.getPlantsToRequest()` (`:308`) : combien en demander quand il en manque.
    pub plants_to_request: u32,
    /// Le même compte pour le bonemeal valide (`:340-351`).
    pub bonemeal_count: u32,
    /// `world.setBlockAndUpdate(...)` (`:317`) pour `PLANT`, `mineBlock(...)` (`:371`)
    /// pour `HARVEST` et `CLEAR`.
    pub action_done: bool,
}

/// Les champs qu'`EntityAIWorkPlanter` garde entre deux ticks.
///
/// Java référence : `EntityAIWorkPlanter.java:61` (`currentFieldActionCount`), `:66`
/// (`activeModuleResult`), `:68` (`currentDeliverable`), plus le `needsCurrently` de
/// la classe de base (`AbstractEntityAIBasic.java:156`), qui est déjà
/// [`Memory::need`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlanterMemory {
    /// `currentFieldActionCount` (`:61`) : combien d'actions sur ce champ, remis à
    /// zéro quand le champ change (`:133`) ou qu'il est rendu (`:293`).
    pub current_field_action_count: u32,
    /// `activeModuleResult` (`:66`) : la réponse du module pour l'action en cours.
    pub active_result: Option<FieldWork>,
    /// `currentDeliverable` (`:68`) : ce qui a été demandé quand un item manquait, et
    /// que `getStateAfterPickUp` renvoie au système de requêtes (`:430-438`).
    pub deliverable: Option<Deliverable>,
}

impl PlanterMemory {
    /// `resetActiveField()` (`:289-294`) : la position de travail et le compteur
    /// d'actions du champ sont rendus au module.
    pub fn reset_active_field(&mut self) {
        self.current_field_action_count = 0;
    }
}

/// Ce que `checkIfItemsUnavailable` demande (`EntityAIWorkPlanter.java:402-403`) :
/// `needsCurrently = new Tuple<>(deliverable::matches, deliverable.getCount())`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Deliverable {
    /// `deliverable.getCount()` : combien la requête demande.
    pub count: u32,
    /// `deliverable.getMinimumCount()` : en dessous, la requête part (`:397-400`).
    pub min_count: u32,
}

/// Un animal de l'enclos, tel que ces deux buts le lisent.
///
/// Java référence : `AbstractEntityAIHerder.java` lit `getAge` (`:259`, `:270`),
/// `isInLove` (`:259`, `:363`, `:644`), `canFallInLove` (`:259`), `isBaby` (`:363`,
/// `:548`, `:684`), `isAlive` / `isDeadOrDying` (`:379`, `:644`),
/// `level.canSeeSky(blockPosition())` (`:365`), `distanceTo` (`:452`) et
/// `canMate` (`:505`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HerdAnimal {
    /// `animal.getUUID()`, réduit à la clé stable que l'appelant sait fournir : c'est
    /// ce qui permet à [`HerdMemory::animals_to_breed`] de retrouver un partenaire
    /// d'un tick à l'autre.
    pub id: u32,
    /// `animal.blockPosition()` (`:357`, `:626`).
    pub at: Pos,
    /// `animal.getAge()` (`:259`).
    pub age: i32,
    /// `animal.isBaby()` (`:363`, `:548`).
    pub baby: bool,
    /// `animal.isInLove()` (`:259`, `:363`, `:644`).
    pub in_love: bool,
    /// `animal.canFallInLove()` (`:259`).
    pub can_fall_in_love: bool,
    /// `!animal.isDeadOrDying()` (`:644`).
    pub alive: bool,
    /// `animal.level.canSeeSky(animal.blockPosition())` (`:365`).
    pub can_see_sky: bool,
    /// `first.canMate(this)`, mesuré par l'appelant avec `canMate` (`:493-510`) : Java
    /// pose `setInLoveTime(5)` sur les deux, demande `canMate`, puis restaure. Le port
    /// lit l'**answer** et ne rejoue pas la manipulation, qui touche deux entités
    /// vivantes.
    pub can_mate: bool,
}

impl HerdAnimal {
    /// `isBreedAble` (`AbstractEntityAIHerder.java:257-260`) : `getAge() == 0 &&
    /// (isInLove() || canFallInLove())`.
    pub fn is_breed_able(&self) -> bool {
        self.age == 0 && (self.in_love || self.can_fall_in_love)
    }

    /// `isFeedAble` (`:268-271`).
    ///
    /// **Cette méthode n'est appelée nulle part dans MineColonies 1.20.1** — mesuré :
    /// `grep -rn isFeedAble src/main/java/` ne rend que sa déclaration. Le port la
    /// garde pour la même raison que Java la garde, et le dit ici plutôt que de laisser
    /// croire qu'elle décide quelque chose. La division par `age` vaut `-∞` quand
    /// `age == 0` (nourrisson), ce qui rend la condition vraie : c'est le Java.
    pub fn is_feed_able(&self, secondary_skill: u32) -> bool {
        self.baby
            && (super::herder::MAX_ENTITY_AGE / self.age as f64)
                <= 1.0 + secondary_skill as f64 / 100.0
    }

    /// `animal.distanceTo(other)` (`:452`) : la distance euclidienne en 3D.
    pub fn distance_to(&self, other: HerdAnimal) -> f64 {
        let dx = (self.at.x - other.at.x) as f64;
        let dy = (self.at.y - other.at.y) as f64;
        let dz = (self.at.z - other.at.z) as f64;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

/// Les tirages que `decideWhatToDo` fait sur `ColonyConstants.rand`.
///
/// **Pas un appel Java.** `ColonyConstants.rand.nextDouble()` (`:230`, `:234`, `:242`)
/// n'est pas une donnée qu'un hôte peut rejouer, donc c'est l'appelant qui tire. Les
/// trois tirages sont nommés par la branche qui les consomme.
///
/// **Pourquoi trois valeurs et pas un curseur.** Java en tire un, deux ou trois selon
/// les branches atteintes (`&&` court-circuite). Trois `nextDouble()` successifs d'un
/// `Random` sont indépendants et de loi uniforme, donc pré-tirer les trois revient au
/// même pour toute suite de décisions — c'est la seule façon de rejouer un scénario
/// sans embarquer un `Random` dans la décision.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HerderDraws {
    /// `ColonyConstants.rand.nextDouble() < 0.1` à `:230`, la chance de ramassage.
    pub pickup: f64,
    /// Le même tirage à `:234`, comparé à `chanceToButcher`.
    pub butcher: f64,
    /// Le même tirage à `:242`, comparé à `FEED_CHANCE`.
    pub feed: f64,
}

/// Tout ce qu'`AbstractEntityAIHerder` lit et que les faits de base ne portent pas.
///
/// Java référence : `decideWhatToDo` (`:199-249`), `prepareForHerding` (`:302-328`),
/// `butcherAnimals` (`:335-390`), `breedAnimals` (`:419-484`), `feedAnimal`
/// (`:517-568`), `pickupItems` (`:575-588`) et les deux recherches d'entités
/// (`:596-614`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HerdFacts {
    /// `building.getBuildingLevel()` (`:675`) : le plafond d'animaux en dépend.
    pub building_level: u32,
    /// `canBreedChildren()` (`:278-281`) : le réglage `BREEDING` de la hutte, `true`
    /// par défaut (`BuildingModules.java:170-171`).
    pub breeding_setting: bool,
    /// `building.getModulesByType(AnimalHerdingModule.class)` n'est pas vide : la
    /// boucle de `:208` a au moins un tour.
    pub has_module: bool,
    /// `searchForAnimals(module::isCompatible)` (`:210`, `:342`, `:433`, `:529`).
    pub animals: Vec<HerdAnimal>,
    /// `searchForItemsInArea()` (`:230`, `:577`), réduit à son compte : la règle ne
    /// lit que « c'est vide ou non ».
    pub items_on_ground: u32,
    /// `items.get(0).blockPosition()` (`:579`) : l'item vers lequel il marche. Le
    /// tableau [`super::herder::WALK_TO_ITEM_RANGE`] avec
    /// `WorldFacts::worker::at` donne la réponse de `walkWithProxy(pos, 1)`, donc
    /// l'arrivée n'est pas un fait séparé.
    pub first_item: Option<Pos>,
    /// `equipTool(MAIN_HAND, ModEquipmentTypes.axe)` (`:344-347`) : il tient une hache
    /// assez bonne.
    pub has_axe: bool,
    /// `getItemCountInItemHandler(inventory, breedingItem) > 1` (`:227-228`).
    pub breeding_item_count: u32,
    /// `equipItem(MAIN_HAND, breedingItems)` (`:466`, `:524`).
    pub breeding_item_equipped: bool,
    /// `!toKill.isAlive()` après `butcherAnimal(toKill)` (`:379`) : le coup a tué.
    pub killed: bool,
    /// Le tirage de l'éleveur de poulets (`EntityAIWorkChickenHerder.java:67`).
    pub chicken_draw: u32,
    /// `worker.level.getGameTime()` (`:478`, `:534`, `:561`).
    pub game_time: u64,
    /// `getPrimarySkillLevel()` : la portée d'attaque du cowboy (`EntityAIWorkCowboy.java:
    /// 254`).
    pub primary_skill: u32,
    /// `getSecondarySkillLevel()` (`:270`, `:548`) : le seuil de nourrissage des
    /// petits, et le tirage de l'éleveur de poulets.
    pub secondary_skill: u32,
    /// `!ItemStackUtils.isEmpty(worker.getMainHandItem())`
    /// (`EntityAIWorkChickenHerder.java:63`) : sans objet en main, l'éleveur de poulets
    /// ne frappe pas du tout.
    pub main_hand_empty: bool,
    /// Les trois tirages de `decideWhatToDo`.
    pub draws: HerderDraws,
}

/// Les champs qu'`AbstractEntityAIHerder` garde entre deux ticks.
///
/// Java référence : `current_module` (`:114`), `animalsToBreed` (`:119`),
/// `fedRecently` (`:124`), `breedTimeOut` (`:129`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HerdMemory {
    /// `breedTimeOut` (`:129`) : posé à 20 ou 60 secondes selon l'échec
    /// (`:438`, `:462`, `:481`) et décrémenté de `DECIDING_DELAY` à chaque `DECIDE`
    /// (`:203-206`).
    pub breed_time_out: i32,
    /// `current_module != null` (`:216`) : le module d'élevage a déjà été choisi une
    /// fois.
    pub has_current_module: bool,
    /// `animalsToBreed` (`:119`), en clés d'animaux et dans l'ordre d'insertion
    /// (`:473-474`).
    pub animals_to_breed: Vec<u32>,
    /// `fedRecently` (`:124`) : `(uuid, gameTime)`.
    pub fed_recently: Vec<(u32, u64)>,
}

impl HerdMemory {
    /// `fedRecently.getOrDefault(uuid, 0L)` (`:534`).
    pub fn fed_at(&self, id: u32) -> u64 {
        self.fed_recently
            .iter()
            .find(|(known, _)| *known == id)
            .map(|(_, at)| *at)
            .unwrap_or(0)
    }

    /// `fedRecently.put(uuid, gameTime)` (`:561`).
    pub fn record_fed(&mut self, id: u32, game_time: u64) {
        match self.fed_recently.iter_mut().find(|(known, _)| *known == id) {
            Some(slot) => slot.1 = game_time,
            None => self.fed_recently.push((id, game_time)),
        }
    }

    /// `fedRecently.remove(uuid)` (`:385`) : un animal abattu ne se nourrit plus.
    pub fn forget_fed(&mut self, id: u32) {
        self.fed_recently.retain(|(known, _)| *known != id);
    }
}

/// Les deux compteurs qu'`EntityAIWorkCowboy` garde (`EntityAIWorkCowboy.java:58-59`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CowboyMemory {
    /// `milkCoolDown` (`:58`).
    pub milk_cool_down: i32,
    /// `stewCoolDown` (`:59`).
    pub stew_cool_down: i32,
}

/// Ce que `EntityAIWorkCowboy` lit en plus de l'élevage.
///
/// Java référence : `decideWhatToDo` (`:96-120`), `milkCows` (`:144-191`),
/// `milkMooshrooms` (`:198-249`), et les deux compteurs du module
/// (`BuildingCowboy.java:254-267`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CowboyFacts {
    /// La moitié élevage : la classe étend `AbstractEntityAIHerder`
    /// (`EntityAIWorkCowboy.java:39`).
    pub herd: HerdFacts,
    /// `HerdingModule.canTryToMilk()` (`BuildingCowboy.java:254-257`) :
    /// `currentMilk < MILKING_AMOUNT`.
    pub can_try_to_milk: bool,
    /// `HerdingModule.canTryToStew()` (`:262-267`).
    pub can_try_to_stew: bool,
    /// `hasItemInInventory(building.getMilkInputItem().getItem())` (`:148`).
    pub milk_input_in_inventory: bool,
    /// `hasBuildingEnoughElseCount(building, milkInput, 1) > 0` (`:150`).
    pub milk_input_in_hut: bool,
    /// `hasItemInInventory(Items.BOWL)` (`:202`).
    pub bowl_in_inventory: bool,
    /// Le même compte pour le bol (`:204`).
    pub bowl_in_hut: bool,
    /// `searchForAnimals` : la première vache ou chèvre adulte qui n'est pas une
    /// vache-champignon (`:162-164`).
    pub milkable_animal: Option<Pos>,
    /// `searchForAnimals` : la première vache-champignon non bébé (`:216-217`).
    pub mooshroom: Option<Pos>,
    /// `equipItem(MAIN_HAND, milkInput)` / `(BOWL)` (`:171`, `:225`).
    pub equipped: bool,
    /// `mooshroom.mobInteract(fakePlayer, MAIN_HAND) == InteractionResult.CONSUME`
    /// (`:229`) : la traite de la vache-champignon a réussi.
    pub stew_succeeded: bool,
}

/// La moitié métier d'une scène.
///
/// Un appelant qui donne à un planteur les faits d'un cow-boy reçoit
/// [`super::state::WorkRefusal::NoFactsForTrade`], pas un défaut silencieux : la
/// différence entre « ce planteur n'a pas de champ aujourd'hui » et « ces faits sont
/// ceux d'un autre métier » ne doit pas se perdre.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: the three trades of this slice
pub enum WorkJob {
    /// Le planteur (`EntityAIWorkPlanter.java:46`).
    Planter(PlanterFacts),
    /// L'éleveur de poulets (`EntityAIWorkChickenHerder.java:22`).
    ChickenHerder(HerdFacts),
    /// Le cow-boy (`EntityAIWorkCowboy.java:39`).
    Cowboy(CowboyFacts),
}

impl Default for WorkJob {
    fn default() -> WorkJob {
        WorkJob::Planter(PlanterFacts::default())
    }
}

impl WorkJob {
    /// La moitié élevage, si ce métier en a une.
    ///
    /// Les deux éleveurs la partagent : `EntityAIWorkCowboy.java:39` étend
    /// `AbstractEntityAIHerder`, donc la table d'élevage est la même pour les deux et
    /// ses actions acceptent les deux variantes. Seules les décisions propres à un
    /// métier (`decideWhatToDo` du cow-boy, `milkCows`, `milkMooshrooms`, ou la
    /// `decide` du planteur) refusent les faits de l'autre.
    pub fn herd(&self) -> Option<&HerdFacts> {
        match self {
            WorkJob::ChickenHerder(facts) => Some(facts),
            WorkJob::Cowboy(facts) => Some(&facts.herd),
            WorkJob::Planter(_) => None,
        }
    }
}

/// Ce qu'une de ces trois IA retient entre deux ticks.
///
/// `delay`, `hasDelayed` et `need` sont ceux de la classe de base et sont réutilisés
/// depuis [`Memory`] dans [`WorkMemory::base`] : `setDelay`, `hasNotDelayed` et
/// `needsCurrently` sont les mêmes trois champs pour tous les métiers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkMemory {
    /// `AbstractEntityAIBasic.delay`, `.hasDelayed`, `.needsCurrently` (`:121`,
    /// `:126`, `:156`).
    pub base: Memory,
    /// Ce que le planteur retient.
    pub planter: PlanterMemory,
    /// Ce que l'éleveur retient.
    pub herd: HerdMemory,
    /// Ce que le cow-boy retient.
    pub cowboy: CowboyMemory,
}

/// La scène entière qu'une de ces décisions lit : les faits de base, les faits du
/// métier, et la mémoire de l'IA.
///
/// [`WorldFacts`] est *la même valeur* que prennent les tables de `crate::ai` — rien
/// n'en est copié — donc un appelant qui fait aussi tourner
/// `crate::ai::worker::JobAi` peut donner à chacun celle qu'il a déjà remplie.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkScene {
    /// Le citoyen, son inventaire, sa hutte et ses blocs (`crate::ai::facts`).
    pub facts: WorldFacts,
    /// Les observations de ce métier.
    pub job: WorkJob,
    /// Ce que l'IA retient entre deux ticks.
    pub memory: WorkMemory,
    /// Le dernier refus, effacé au début de chaque tick.
    pub refusal: Option<super::state::WorkRefusal>,
}

impl WorkScene {
    /// Une scène sur `facts` avec une mémoire vide et `job`, les faits du métier.
    pub fn new(facts: WorldFacts, job: WorkJob) -> WorkScene {
        WorkScene {
            facts,
            job,
            memory: WorkMemory::default(),
            refusal: None,
        }
    }

    /// Les faits du planteur, si c'est un planteur.
    pub fn planter(&self) -> Option<&PlanterFacts> {
        match &self.job {
            WorkJob::Planter(facts) => Some(facts),
            _ => None,
        }
    }

    /// Les faits d'élevage, si c'est un éleveur de poulets.
    pub fn chicken_herder(&self) -> Option<&HerdFacts> {
        match &self.job {
            WorkJob::ChickenHerder(facts) => Some(facts),
            _ => None,
        }
    }

    /// Les faits du cow-boy, si c'est un cow-boy.
    pub fn cowboy(&self) -> Option<&CowboyFacts> {
        match &self.job {
            WorkJob::Cowboy(facts) => Some(facts),
            _ => None,
        }
    }

    /// Enregistrer pourquoi ce tick ne peut pas avancer. Le dernier gagne, comme
    /// `crate::ai::worker::Scene::refuse`.
    pub fn refuse(&mut self, refusal: super::state::WorkRefusal) {
        self.refusal = Some(refusal);
    }

    /// `walkToBuilding()` (`AbstractEntityAIBasic.java:757-766`) : le citoyen est à
    /// moins de `WOKR_IN_BUILDING_DIST` de sa hutte, le même test que la table de
    /// base.
    pub fn at_hut(&self) -> bool {
        self.facts.worker.at.is_living_at_site(
            self.facts.building.position,
            crate::ai::worker::WOKR_IN_BUILDING_DIST,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_five_actions_copy_java_s_two_flag_values() {
        // `IPlantationModule.java:176-180` : NONE(false), PLANT(true), BONEMEAL(true),
        // HARVEST(true), CLEAR(false).
        assert!(!PlantAction::None.increases_action_count());
        assert!(PlantAction::Plant.increases_action_count());
        assert!(PlantAction::Bonemeal.increases_action_count());
        assert!(PlantAction::Harvest.increases_action_count());
        assert!(!PlantAction::Clear.increases_action_count());
        assert_eq!(PlantAction::Bonemeal.java(), "BONEMEAL");
    }

    #[test]
    fn position_and_field_are_not_the_same_reset() {
        // `shouldResetWorkingPosition` (`:310-313`) et `shouldResetCurrentField`
        // (`:320-323`).
        assert!(!ResetState::None.resets_position());
        assert!(!ResetState::None.resets_field());
        assert!(ResetState::Position.resets_position());
        assert!(!ResetState::Position.resets_field(), "POSITION ne rend pas le champ");
        assert!(ResetState::Field.resets_position(), "FIELD implique POSITION");
        assert!(ResetState::Field.resets_field());
    }

    #[test]
    fn the_none_result_picks_a_new_field_and_has_nothing_to_do() {
        // `PlantationModuleResult.NONE` (`:214`).
        let none = FieldWork::none(Pos::new(1, 64, 2));
        assert_eq!(none.action, PlantAction::None);
        assert_eq!(none.action_position, None);
        assert!(none.should_reset_current_field());
        let plant = FieldWork::plant(Pos::new(1, 64, 2), Pos::new(1, 65, 2));
        assert_eq!(plant.action, PlantAction::Plant);
        assert_eq!(plant.action_position, Some(Pos::new(1, 65, 2)));
        assert!(plant.should_reset_working_position());
        assert!(!plant.should_reset_current_field(), "on reste sur le champ");
    }

    #[test]
    fn a_newborn_and_an_adult_are_breedable_but_a_growing_one_is_not() {
        // `isBreedAble` (`AbstractEntityAIHerder.java:257-260`).
        let adult = HerdAnimal {
            age: 0,
            can_fall_in_love: true,
            ..animal(1)
        };
        assert!(adult.is_breed_able());
        let already = HerdAnimal {
            age: 0,
            in_love: true,
            can_fall_in_love: false,
            ..animal(2)
        };
        assert!(already.is_breed_able(), "isInLove suffit");
        let growing = HerdAnimal {
            age: -600,
            can_fall_in_love: true,
            ..animal(3)
        };
        assert!(!growing.is_breed_able(), "l'âge doit être exactement 0");
    }

    #[test]
    fn is_feed_able_is_ported_although_nothing_calls_it() {
        // `isFeedAble` (`:268-271`) : déclarée, jamais appelée dans 1.20.1.
        let baby = HerdAnimal {
            baby: true,
            age: -12000,
            ..animal(4)
        };
        // -24000 / -12000 = 2.0, et 1 + 20/100 = 1.2 : trop tôt.
        assert!(!baby.is_feed_able(20));
        // À 100 de compétence le seuil vaut 2.0, et la comparaison est `<=`.
        assert!(baby.is_feed_able(100));
        // Un nourrisson à l'âge 0 divise par zéro : -∞, donc vrai (le Java aussi).
        let newborn = HerdAnimal {
            baby: true,
            age: 0,
            ..animal(5)
        };
        assert!(newborn.is_feed_able(0));
    }

    #[test]
    fn an_animal_measures_the_distance_the_way_distance_to_does() {
        // `animal.distanceTo(other)` (`:452`).
        let one = HerdAnimal {
            at: Pos::new(0, 64, 0),
            ..animal(6)
        };
        let two = HerdAnimal {
            at: Pos::new(3, 68, 0),
            ..animal(7)
        };
        assert_eq!(one.distance_to(two), 5.0);
    }

    #[test]
    fn the_fed_map_is_a_get_or_default_and_a_put() {
        // `fedRecently.getOrDefault(uuid, 0L)` (`:534`) et `.put(...)` (`:561`).
        let mut memory = HerdMemory::default();
        assert_eq!(memory.fed_at(9), 0);
        memory.record_fed(9, 1200);
        assert_eq!(memory.fed_at(9), 1200);
        memory.record_fed(9, 2400);
        assert_eq!(memory.fed_at(9), 2400, "la seconde écriture remplace");
        assert_eq!(memory.fed_recently.len(), 1);
        memory.forget_fed(9);
        assert_eq!(memory.fed_at(9), 0, "`:385`");
    }

    #[test]
    fn resetting_the_active_field_drops_the_action_count_only() {
        // `resetActiveField()` (`EntityAIWorkPlanter.java:289-294`) : `:293` remet le
        // compteur à zéro, et rien d'autre.
        let mut memory = PlanterMemory {
            current_field_action_count: 4,
            deliverable: Some(Deliverable {
                count: 16,
                min_count: 1,
            }),
            ..PlanterMemory::default()
        };
        memory.reset_active_field();
        assert_eq!(memory.current_field_action_count, 0);
        assert!(memory.deliverable.is_some(), "la requête en cours survit");
    }

    #[test]
    fn a_scene_refuses_to_answer_for_another_trade() {
        let scene = WorkScene::new(
            WorldFacts::new(),
            WorkJob::Cowboy(CowboyFacts::default()),
        );
        assert!(scene.cowboy().is_some());
        assert!(scene.planter().is_none());
        assert!(scene.chicken_herder().is_none());
    }

    fn animal(id: u32) -> HerdAnimal {
        HerdAnimal {
            id,
            at: Pos::new(0, 64, 0),
            age: 0,
            baby: false,
            in_love: false,
            can_fall_in_love: true,
            alive: true,
            can_see_sky: true,
            can_mate: true,
        }
    }
}
