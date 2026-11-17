//! Ce que ces trois buts lisent du monde, écrit comme des faits, et ce qu'ils retiennent
//! entre deux ticks.
//!
//! Java référence : `AbstractEntityAIHerder.searchForAnimals` (`:596-599`) et
//! `searchForItemsInArea` (`:611-614`) pour l'enclos ; `EntityAIWorkShepherd` pour la
//! brebis (`isSheared`, `getColor`) et la hache ; `EntityAIWorkStablemaster` pour les
//! étalons (`isAlive`, `isRemoved`, `isLeashed`, `hasCavalryRider`, `hasReservation`,
//! `isInStable`, la santé, `getCombatCooldown`) et les deux listes d'objets de la hutte
//! (`ModTags.feed`, `ModTags.leather`).
//!
//! **Pourquoi des données et pas des appels.** Le Java a un `Level`, un
//! `AnimalHerdingModule` et des entités vivantes en main ; un mod était ici sans monde.
//! Une décision qui ne peut être rejouée ne peut pas être testée : chaque champ est donc
//! une réponse que l'appelant a observée, et le module ne pose aucune question.
//!
//! **Deux champs ne sont pas des appels Java** : les tirages aléatoires
//! ([`Draws`]). `worker.getRandom()` n'est pas une donnée rejouable ; les nommer est ce
//! qui rend la décision d'un tirage testable, et c'est le même choix que
//! `crate::ai::facts::WorkerFacts::coin`.
//!
//! **Absent n'est pas faux.** Un animal absent de [`PenFacts::animals`] est un animal
//! que l'appelant n'a pas vu ; l'enclos vide (`animals.is_empty()`) est un fait à part
//! entière, et c'est `AbstractEntityAIHerder.java:349-352` qui le traite.

use crate::ai::facts::{Pos, WorldFacts};
use crate::ai::worker::Memory;

use super::state::{Hand, HerdChoice, HerdState, TradeRefusal};

/// Ce que ces trois métiers demandent à la marche : `walkToPos(worker, pos, 2, true)`
/// (`AbstractEntityAIHerder.java:626`).
pub const WALK_TO_ANIMAL_RANGE: i32 = 2;

/// `walkToSafePos(pos)` = `EntityNavigationUtils.walkToPos(worker, pos, 4, true)`
/// (`AbstractEntityAIBasic.java:830`). Le `4` est écrit en dur là-bas, pas pris dans
/// `BUILDING_REACH_DIST` (`EntityNavigationUtils.java:20`).
pub const SAFE_POS_RANGE: i32 = 4;

/// `walkWithProxy(items.get(0).blockPosition(), 1)` (`AbstractEntityAIHerder.java:579`).
pub const WALK_TO_ITEM_RANGE: i32 = 1;

/// Vrai quand le citoyen est arrivé à `range` blocs de `pos`.
///
/// Java référence : `EntityNavigationUtils.java:169-176` — `BlockPosUtil.dist(pos,
/// desiredPosition) <= distToDesired`, où `dist` est la racine de la somme des carrés
/// (`BlockPosUtil.java:563-569`). Comparé au carré ici, ce qui évite une racine sans
/// changer la réponse pour des positions entières.
///
/// **Ce n'est pas `crate::ai::facts::Pos::is_living_at_site`.** Ce dernier teste
/// `distSqr < range²` (`EntityUtils.java:301-305`), et `walkToPos` teste
/// `dist <= range` : à exactement `range` blocs, `EntityUtils` dit non et `walkToPos`
/// dit oui. La nuance est invisible la plupart du temps et fausse un test de bord, donc
/// elle est écrite ici plutôt qu'héritée par accident.
pub fn arrived(facts: &WorldFacts, pos: Pos, range: i32) -> bool {
    facts.worker.at.dist_sqr(pos) <= (range as i64) * (range as i64)
}

/// Le même test, sur une scène.
pub fn arrived_at(scene: &HerdScene, pos: Pos, range: i32) -> bool {
    arrived(&scene.facts, pos, range)
}

/// `walkToBuilding()` (`AbstractEntityAIBasic.java:757-766`) : à moins de
/// `WOKR_IN_BUILDING_DIST` de la hutte.
pub fn at_hut(scene: &HerdScene) -> bool {
    scene.facts.worker.at.is_living_at_site(
        scene.facts.building.position,
        crate::ai::worker::WOKR_IN_BUILDING_DIST,
    )
}

/// L'espèce d'un animal de l'enclos, réduite à ce que ces trois métiers distinguent.
///
/// Java référence : `a instanceof Sheep` (`EntityAIWorkShepherd.java:101`),
/// `a instanceof CavalryHorseEntity` (`EntityAIWorkStablemaster.java:316`, `:410`),
/// `a instanceof AbstractHorse` (`:322`). Le lapin, lui, n'est jamais distingué : sa
/// hutte n'a qu'un module d'élevage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
// exhaustive: the four answers these three classes compare against
pub enum AnimalKind {
    /// `Animal`, ni `Sheep` ni cheval — ce que l'enclos du lapin contient.
    #[default]
    Other,
    /// `net.minecraft.world.entity.animal.Sheep`.
    Sheep,
    /// `AbstractHorse` qui n'est pas un `CavalryHorseEntity`.
    VanillaHorse,
    /// `CavalryHorseEntity` — l'étalon déjà dressé.
    CavalryMount,
}

impl AnimalKind {
    /// `instanceof Sheep` (`EntityAIWorkShepherd.java:101`).
    pub fn is_sheep(self) -> bool {
        self == AnimalKind::Sheep
    }

    /// `instanceof AbstractHorse` (`EntityAIWorkStablemaster.java:322`) : la monture
    /// dressable comprend les chevaux vanilla.
    pub fn is_horse(self) -> bool {
        matches!(self, AnimalKind::VanillaHorse | AnimalKind::CavalryMount)
    }

    /// `instanceof CavalryHorseEntity` (`:316`, `:410`, `:570`).
    pub fn is_cavalry(self) -> bool {
        self == AnimalKind::CavalryMount
    }
}

/// Un animal que l'enclos de la hutte contient, réduit aux lectures de ces trois
/// métiers.
///
/// Java référence : une entité `Animal` de `WorldUtil.getEntitiesWithinBuilding`
/// (`AbstractEntityAIHerder.java:598`) ; chaque champ nomme l'appelant qui le lit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PenAnimal {
    /// `animal.getUUID()`, réduit à la clé stable que l'appelant sait fournir : c'est ce
    /// qui permet à la mémoire de retrouver le même étalon d'un tick à l'autre
    /// (`EntityAIWorkStablemaster.java:78-80`).
    pub id: u32,
    /// `animal.blockPosition()` (`:273`, `:349`, `:620`).
    pub at: Pos,
    /// L'espèce.
    pub kind: AnimalKind,
    /// `animal.getAge()` (`AbstractEntityAIHerder.java:259`).
    pub age: i32,
    /// `animal.isBaby()` (`:363`, `:548`, `EntityAIWorkStablemaster.java:324`).
    pub baby: bool,
    /// `animal.isAlive()` (`EntityAIWorkStablemaster.java:324`, `:568`).
    pub alive: bool,
    /// `animal.isRemoved()` (`:569`, `:749`).
    pub removed: bool,
    /// `animal.isInLove()` (`AbstractEntityAIHerder.java:363`).
    pub in_love: bool,
    /// `animal.getPassengers().isEmpty()` (`EntityAIWorkStablemaster.java:324`) : zéro
    /// veut dire « personne dessus ».
    pub passengers: u32,
    /// `sheep.isSheared()` (`EntityAIWorkShepherd.java:101`).
    pub sheared: bool,
    /// `sheep.getColor()` (`:145`, `:150`) : l'indice de `DyeColor`, `0` = blanc.
    pub dye: u8,
    /// `animal.isLeashed()` (`EntityAIWorkStablemaster.java:589`, `:677`).
    pub leashed: bool,
    /// `worker.equals(animal.getLeashHolder())` (`:677`) : la laisse est tenue par ce
    /// citoyen.
    pub leash_holder: bool,
    /// `cav.hasCavalryRider()` (`:352`, `:570`).
    pub has_cavalry_rider: bool,
    /// `cav.hasReservation()` (`:571`).
    pub has_reservation: bool,
    /// `cav.isInStable()` (`:752`).
    pub in_stable: bool,
    /// `horse.getHealth()` (`:378`, `:412`, `:529`).
    pub health: f32,
    /// `horse.getMaxHealth()` (`:378`, `:412`, `:525`).
    pub max_health: f32,
    /// `horse.getCombatCooldown()` (`:383`, `:412`) — `CavalryHorseEntity.java:983-988`.
    pub combat_cooldown: f32,
    /// `cav.needsHealthTraining()` (`:363`, `:412`) —
    /// `CavalryHorseEntity.java:763-788` : le bonus de recherche `MOUNT_HEALTH` de la
    /// colonie ne correspond pas à celui que la bête porte.
    pub needs_health_training: bool,
    /// `CavalryHorseEntity.createFromVanilla(...) != null` (`:281`) : la bête est
    /// dressable. `createFromVanilla` rend `null` pour une espèce inconnue et Java se
    /// contente d'un avertissement (`:284`).
    pub convertible: bool,
}

impl PenAnimal {
    /// Un animal nu, à `at`, en vie, avec la santé pleine.
    fn bare(id: u32, at: Pos, kind: AnimalKind) -> PenAnimal {
        PenAnimal {
            id,
            at,
            kind,
            age: 0,
            baby: false,
            alive: true,
            removed: false,
            in_love: false,
            passengers: 0,
            sheared: false,
            dye: 0,
            leashed: false,
            leash_holder: false,
            has_cavalry_rider: false,
            has_reservation: false,
            in_stable: false,
            health: 20.0,
            max_health: 20.0,
            // `CavalryHorseEntity.createFromVanilla` pose le compte à rebours de combat
            // à la santé maximale (`CavalryHorseEntity.java:752`), donc une monture
            // fraîche n'est jamais prête (`:970-975`, seuil `.66`).
            combat_cooldown: 20.0,
            needs_health_training: false,
            convertible: true,
        }
    }

    /// L'animal que l'enclos d'un lapin contient.
    pub fn other(id: u32, at: Pos) -> PenAnimal {
        PenAnimal::bare(id, at, AnimalKind::Other)
    }

    /// Une brebis (`EntityAIWorkShepherd.java:101`), non tondue, blanche.
    pub fn sheep(id: u32, at: Pos) -> PenAnimal {
        PenAnimal::bare(id, at, AnimalKind::Sheep)
    }

    /// Un cheval vanilla, candidat au dressage (`EntityAIWorkStablemaster.java:306-335`).
    pub fn horse(id: u32, at: Pos) -> PenAnimal {
        PenAnimal::bare(id, at, AnimalKind::VanillaHorse)
    }

    /// Un étalon de cavalerie déjà dressé (`:410`).
    pub fn cavalry(id: u32, at: Pos) -> PenAnimal {
        PenAnimal::bare(id, at, AnimalKind::CavalryMount)
    }

    /// `CavalryHorseEntity.isReadyForCombat` (`CavalryHorseEntity.java:970-975`) :
    /// `combatCooldown <= maxHealth * COMBAT_READINESS_THRESHOLD`.
    pub fn ready_for_combat(&self, threshold: f32) -> bool {
        self.combat_cooldown <= self.max_health * threshold
    }
}

/// L'enclos et la décision de base : les faits que ces trois métiers partagent.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PenFacts {
    /// Ce que `super.decideWhatToDo()` a répondu, déposé par l'appelant.
    ///
    /// Java référence : `final IAIState result = super.decideWhatToDo()`
    /// (`EntityAIWorkShepherd.java:75`, `EntityAIWorkStablemaster.java:234`). Le
    /// parent est la table d'`AbstractEntityAIHerder` (`:199-249`), portée une seule
    /// fois par `crate::ai_work_09::herder` ; l'appelant la fait tourner, écrit sa
    /// réponse ici, et **cette machine la consomme** : le champ est vidé par la
    /// surcharge (`HerdScene::take_base_answer`). `None` veut donc dire « la table de
    /// base n'a rien décidé depuis le dernier passage », pas « rien à faire ».
    pub base_answer: Option<HerdChoice>,
    /// `searchForAnimals(module::isCompatible)` (`:596-599`), dans l'ordre où le monde
    /// les rend — `gatherMounts` prend le premier (`EntityAIWorkStablemaster.java:640`).
    pub animals: Vec<PenAnimal>,
    /// `searchForItemsInArea()` (`:611-614`), la liste des items au sol de l'enclos.
    pub ground_items: Vec<Pos>,
}

impl PenFacts {
    /// L'animal d'identifiant `id`, s'il est encore dans l'enclos.
    pub fn by_id(&self, id: u32) -> Option<PenAnimal> {
        self.animals.iter().find(|animal| animal.id == id).copied()
    }

    /// `findShearableSheep()` (`EntityAIWorkShepherd.java:99-103`) : `instanceof Sheep
    /// && !isSheared() && !isBaby()`, la première trouvée.
    ///
    /// Java rend `searchForAnimals(...).stream()…findAny()`, donc **n'importe laquelle**
    /// du flux ; le port prend la première de l'ordre que l'appelant a écrit, ce qui est
    /// la seule réponse rejouable.
    pub fn shearable_sheep(&self) -> Option<PenAnimal> {
        self.animals
            .iter()
            .find(|animal| animal.kind.is_sheep() && !animal.sheared && !animal.baby)
            .copied()
    }

    /// `searchForAnimals(module::isCompatible)` non vide (`:349`, `:211`).
    pub fn is_empty(&self) -> bool {
        self.animals.is_empty()
    }
}

/// Ce que lit `EntityAIWorkRabbitHerder` — sa seule règle propre est une frappe.
///
/// Java référence : `butcherAnimal` (`:54-68`) et `updateRenderMetaData` (`:37-46`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RabbitFacts {
    /// `!ItemStackUtils.isEmpty(worker.getMainHandItem())` (`:57`).
    pub main_hand_filled: bool,
    /// `worker.getCitizenInventoryHandler().hasItemInInventory(Items.CARROT)` (`:41`).
    pub carrots: u32,
    /// `getState() == IDLE` (`:40`) : l'icône de travail ne s'affiche pas au repos.
    pub idle: bool,
    /// `getPrimarySkillLevel()`, l'habileté principale du métier (`:61`).
    pub primary_skill: u32,
}

impl Default for RabbitFacts {
    /// Un lapinier qui tient une hache et aucune carotte : l'état d'un citoyen qui vient
    /// d'être embauché après son premier passage à la hutte.
    fn default() -> RabbitFacts {
        RabbitFacts {
            main_hand_filled: true,
            carrots: 0,
            idle: false,
            primary_skill: 0,
        }
    }
}

/// Ce que lit `EntityAIWorkShepherd` au-delà de l'enclos.
///
/// Java référence : `getExtraToolsNeeded` (`:56-64`), `decideWhatToDo` (`:73-87`),
/// `shearSheep` (`:110-166`), `dyeSheepChance` (`:173-187`),
/// `getButcheringAttackDamage` (`:90-93`) et les deux réglages de `BuildingShepherd`
/// (`BuildingShepherd.java:35`, `:40`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShepherdFacts {
    /// `building.getSetting(BuildingShepherd.SHEARING).getValue()` (`:59`, `:79`).
    pub shearing: bool,
    /// `building.getSetting(BuildingShepherd.DYEING).getValue()` (`:175`).
    pub dyeing: bool,
    /// `building.getBuildingLevel()`, la chance de teinture en pour cent (`:177`) et la
    /// limite d'animaux (`AbstractEntityAIHerder.java:675`).
    pub building_level: u32,
    /// `getPrimarySkillLevel()` : le multiplicateur de Fortune (`:133`) et le tirage de
    /// la tonte par le lapinier (`EntityAIWorkRabbitHerder.java:61`).
    pub primary_skill: u32,
    /// `getSecondarySkillLevel()` : les dégâts d'abattage (`:92`).
    pub secondary_skill: u32,
    /// `getToolSlot(shears)` (`AbstractEntityAIHerder.java:721-731`) : l'emplacement de
    /// la cisaille dans l'inventaire, `None` si `getFirstSlotOfItemHandlerContainingEquipment`
    /// a rendu `-1` — auquel cas `checkForToolOrWeapon(shears)` a posé la requête.
    pub shears_slot: Option<usize>,
    /// `worker.getMainHandItem().getEnchantmentLevel(Enchantments.BLOCK_FORTUNE)`
    /// (`:132`).
    pub shears_fortune: u32,
    /// `equipItem(MAIN_HAND, module.getBreedingItems())` aurait réussi
    /// (`AbstractEntityAIHerder.java:740-752`) : le citoyen a de quoi nourrir.
    pub breeding_item: bool,
}

impl Default for ShepherdFacts {
    /// Un berger sans cisaille, sans réglage actif, au niveau 1 : la hutte telle qu'on
    /// la pose.
    fn default() -> ShepherdFacts {
        ShepherdFacts {
            shearing: false,
            dyeing: false,
            building_level: 1,
            primary_skill: 0,
            secondary_skill: 0,
            shears_slot: None,
            shears_fortune: 0,
            breeding_item: false,
        }
    }
}

/// Le travail de hutte qu'un entretien d'étalon consomme (`EntityAIWorkStablemaster.java:
/// 60-64`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: the two enum constants of MountMaintenance
pub enum MountMaintenance {
    /// `FEEDING` (`:62`) — `ModTags.feed`, soigner.
    Feeding,
    /// `READYING` (`:63`) — `ModTags.leather`, réduire le compte à rebours de combat.
    Readying,
}

impl MountMaintenance {
    /// Java's name for it.
    pub fn java_name(self) -> &'static str {
        match self {
            MountMaintenance::Feeding => "FEEDING",
            MountMaintenance::Readying => "READYING",
        }
    }

    /// `task == MountMaintenance.FEEDING ? ModTags.feed : ModTags.leather` (`:431`).
    pub fn item_name(self) -> &'static str {
        match self {
            MountMaintenance::Feeding => "ModTags.feed",
            MountMaintenance::Readying => "ModTags.leather",
        }
    }
}

/// Ce que lit `EntityAIWorkStablemaster` au-delà de l'enclos.
///
/// Java référence : `decideWhatToDo` (`:232-262`), `canBreedChildren` (`:190-209`),
/// `convertMount` (`:268-339`), `readyMountForCombat` (`:346-422`), `readyMount`
/// (`:429-514`), `gatherMounts` (`:563-643`) et `findNearbyUnstabledHorses`
/// (`:733-755`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StableFacts {
    /// `building.getSetting(AbstractBuilding.BREEDING).getValue()`
    /// (`AbstractEntityAIHerder.java:280`).
    pub breeding_setting: bool,
    /// `building.getBuildingLevel()` : la limite d'étalons est `niveau * 2`
    /// (`EntityAIWorkStablemaster.java:270`, `:199`).
    pub building_level: u32,
    /// `getPrimarySkillLevel()`, la part de récupération de combat (`:486`).
    pub primary_skill: u32,
    /// `building.getModule(BuildingModules.STABLEMASTER_HERDING) != null` (`:398-404`).
    pub has_herding_module: bool,
    /// `countCurrentMounts()` (`:553-557`) : les `CavalryHorseEntity` dont la hutte est
    /// la maison.
    pub mounts: u32,
    /// `building.getNextStallPosition()` (`BuildingStable.java:124-141`), `None` quand
    /// aucune stalle n'est étiquetée (`:128-131`).
    pub stall: Option<Pos>,
    /// `InventoryUtils.getItemCountInProvider(worker, i -> i.is(ModTags.feed))`
    /// (`:438`).
    pub feed_in_inventory: u32,
    /// `InventoryUtils.hasBuildingEnoughElseCount(building, i -> i.is(ModTags.feed), 1)`
    /// (`:440`).
    pub feed_in_hut: u32,
    /// La même lecture pour `ModTags.leather` (`:431`, l'autre branche).
    pub leather_in_inventory: u32,
    /// La même lecture pour la hutte.
    pub leather_in_hut: u32,
    /// `building.hasWorkerOpenRequestsOfType(citizen, TypeToken.of(StackList.class))`
    /// (`:506`) : une requête de liste est déjà en vol.
    pub open_stacklist_request: bool,
    /// `!checkForToolOrWeapon(lead)` (`:158-170`, `:578-587`) : la laisse est là.
    pub lead_ok: bool,
    /// `stackToUse.getItem() == Items.LEATHER_HORSE_ARMOR` (`:480`) : l'armure pose le
    /// compte à rebours à zéro au lieu de le réduire.
    pub leather_horse_armor: bool,
    /// `worker` porte un cavalier : `horseToRetrieve` doit être relâché hors de
    /// `HERDER_GATHER_MOUNTS` (`:120-136`).
    pub holds_leash: bool,
}

impl Default for StableFacts {
    /// Une écurie de niveau 1 sans stalle étiquetée, sans module d'élevage : ce qu'un
    /// joueur qui vient de poser la hutte a.
    fn default() -> StableFacts {
        StableFacts {
            breeding_setting: false,
            building_level: 1,
            primary_skill: 0,
            has_herding_module: true,
            mounts: 0,
            stall: None,
            feed_in_inventory: 0,
            feed_in_hut: 0,
            leather_in_inventory: 0,
            leather_in_hut: 0,
            open_stacklist_request: false,
            lead_ok: true,
            leather_horse_armor: false,
            holds_leash: false,
        }
    }
}

/// Les faits du métier, un par métier.
///
/// Un `enum` et non trois champs optionnels : une scène de berger qui porterait les
/// chiffres d'une écurie déciderait sur des faits qui n'existent pas, et le port répond
/// [`TradeRefusal::WrongTradeFacts`] au lieu de lire un zéro par accident.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HerdJob {
    /// Le lapinier (`EntityAIWorkRabbitHerder.java:20`).
    Rabbit(RabbitFacts),
    /// Le berger (`EntityAIWorkShepherd.java:34`).
    Shepherd(ShepherdFacts),
    /// Le palefrenier (`EntityAIWorkStablemaster.java:58`).
    Stable(StableFacts),
}

impl Default for HerdJob {
    fn default() -> HerdJob {
        HerdJob::Rabbit(RabbitFacts::default())
    }
}

impl HerdJob {
    /// Le métier auquel ces faits appartiennent.
    pub fn trade(self) -> super::HerdTrade {
        match self {
            HerdJob::Rabbit(_) => super::HerdTrade::RabbitHerder,
            HerdJob::Shepherd(_) => super::HerdTrade::Shepherd,
            HerdJob::Stable(_) => super::HerdTrade::Stablemaster,
        }
    }

    /// Java's name for the class these facts come from.
    pub fn java(self) -> &'static str {
        match self {
            HerdJob::Rabbit(_) => "EntityAIWorkRabbitHerder.java:20",
            HerdJob::Shepherd(_) => "EntityAIWorkShepherd.java:34",
            HerdJob::Stable(_) => "EntityAIWorkStablemaster.java:58",
        }
    }
}

/// Les tirages que ces trois métiers consomment, nommés.
///
/// **Aucun n'est un appel Java.** `worker.getRandom()` (`EntityAIWorkRabbitHerder.java:
/// 61`, `EntityAIWorkShepherd.java:141`, `:178`, `:183`, `EntityAIWorkStablemaster.java:
/// 238`, `:244`, `:250`) rend une suite que rien ne rejoue ; l'appelant fournit donc le
/// tirage de chaque usage, et l'ordre de consommation est celui de Java.
///
/// **Le court-circuit de `&&` se lit dans les règles, pas ici.** `trainingCooldown <= 0
/// && rand.nextDouble() < TRAINING_CHANCE` (`:238`) ne consomme **aucun** tirage quand
/// le compte à rebours est positif ; comme les tirages sont nommés par usage et non par
/// position dans la suite, la décision reste la même sans avoir à simuler le générateur.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Draws {
    /// `worker.getRandom().nextDouble()` de `EntityAIWorkStablemaster.java:238`, comparé
    /// à [`super::stablemaster::TRAINING_CHANCE`].
    pub training: f64,
    /// Celui de `:244`, comparé à [`super::stablemaster::READY_MOUNT_FOR_COMBAT_CHANCE`].
    pub ready: f64,
    /// Celui de `:250`, comparé à [`super::stablemaster::ROUND_UP_CHANCE`].
    pub roundup: f64,
    /// `worker.getRandom().nextInt(HUNDRED_PERCENT_CHANCE)` de
    /// `EntityAIWorkShepherd.java:178` : la teinture passe si `<= buildingLevel`.
    pub dye_percent: u32,
    /// `worker.getRandom().nextInt(colors.length)` de `:183`, `0..16`.
    pub dye_colour: u32,
    /// `worker.getRandom().nextInt(enchantmentLevel + 1)` de `:141` : la quantité de
    /// laine au-dessus de une.
    pub wool: u32,
    /// `worker.getRandom().nextInt(1 + (100 - primary) / 5)` de
    /// `EntityAIWorkRabbitHerder.java:61` : la frappe passe si `<= 1`.
    pub hit: u32,
}

/// Ce qu'un de ces trois buts a fait au monde pendant un tick.
///
/// Java applique l'effet tout de suite (`sheep.setSheared(true)`,
/// `sheep.setColor(...)`, `horse.setLeashedTo(...)`, `horse.heal(...)`). Le port ne peut
/// pas : le monde n'est pas là. Il **nomme** donc l'effet, et l'appelant l'applique — le
/// même partage que `crate::ai::worker::Refusal`, et ce qui rend le test d'un scénario
/// vérifiable jusqu'à la conséquence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: one variant per world write these three classes make
pub enum HerdAct {
    /// `sheep.setSheared(true)` + le transfert de `qty` laines
    /// (`EntityAIWorkShepherd.java:140`, `:143-147`, `:158-162`), et la teinture qui
    /// suit dans le même appel (`:151`, `:173-187`).
    Sheared {
        /// La brebis tondue.
        id: u32,
        /// `qty` : le nombre de laines produites (`:141`).
        wool: u32,
        /// `Some(DyeColor)` quand `dyeSheepChance` a teint la bête (`:184`).
        dyed: Option<u32>,
    },
    /// `CavalryHorseEntity.createFromVanilla(...)` puis le rattachement à la hutte
    /// (`EntityAIWorkStablemaster.java:281-288`).
    Converted {
        /// L'étalon dressé.
        id: u32,
    },
    /// `horse.applyHealthTraining()` (`:365`).
    HealthTrained {
        /// L'étalon dont le bonus de santé a été remplacé.
        id: u32,
    },
    /// `feedHorse(horse)` (`:470`) : `heal(maxHealth * .20)` et `setTemper(+3)`
    /// (`:531`, `:537`).
    Fed {
        /// L'étalon nourri.
        id: u32,
        /// `horse.getHealth() < maxHealth` : Java n'applique le soin que dans ce cas
        /// (`:529-533`).
        healed: bool,
    },
    /// `prepareForCombat(recovery)` ou `setCombatCooldown(0)` (`:482`, `:487`).
    Readied {
        /// L'étalon préparé.
        id: u32,
        /// `horse.isReadyForCombat()` après coup (`:495`) — le moment où Java compte la
        /// statistique `MOUNTS_READIED`.
        ready: bool,
    },
    /// `horse.setLeashedTo(worker, true)` (`:690`).
    Leashed {
        /// L'étalon attaché.
        id: u32,
    },
    /// `horse.dropLeash(true, false)` (`:717`).
    Unleashed {
        /// L'étalon relâché.
        id: u32,
    },
    /// `createRequestAsync(new StackList(...))` (`:506-510`) : une requête de liste est
    /// partie, l'appelant la crée.
    RequestedItems {
        /// Le travail dont l'objet manque.
        task: MountMaintenance,
    },
    /// `StatsUtil.trackStat(building, ROUNDUPS_COMPLETED, 1)` (`:615`) : une rentrée
    /// d'étalon au bercail.
    RoundupDone,
    /// `animal.hurt(ds, damage)` du lapinier, quand le tirage passe
    /// (`EntityAIWorkRabbitHerder.java:64`).
    Butchered {
        /// L'animal touché.
        id: u32,
    },
}

impl HerdAct {
    /// La ligne de Java qui écrit cet effet.
    pub fn java(self) -> &'static str {
        match self {
            HerdAct::Sheared { .. } => "EntityAIWorkShepherd.java:140",
            HerdAct::Converted { .. } => "EntityAIWorkStablemaster.java:281",
            HerdAct::HealthTrained { .. } => "EntityAIWorkStablemaster.java:365",
            HerdAct::Fed { .. } => "EntityAIWorkStablemaster.java:470",
            HerdAct::Readied { .. } => "EntityAIWorkStablemaster.java:487",
            HerdAct::Leashed { .. } => "EntityAIWorkStablemaster.java:690",
            HerdAct::Unleashed { .. } => "EntityAIWorkStablemaster.java:717",
            HerdAct::RequestedItems { .. } => "EntityAIWorkStablemaster.java:509",
            HerdAct::RoundupDone => "EntityAIWorkStablemaster.java:615",
            HerdAct::Butchered { .. } => "EntityAIWorkRabbitHerder.java:64",
        }
    }

    /// L'animal que cet effet concerne, quand il y en a un.
    pub fn animal(self) -> Option<u32> {
        match self {
            HerdAct::Sheared { id, .. }
            | HerdAct::Converted { id }
            | HerdAct::HealthTrained { id }
            | HerdAct::Fed { id, .. }
            | HerdAct::Readied { id, .. }
            | HerdAct::Leashed { id }
            | HerdAct::Unleashed { id }
            | HerdAct::Butchered { id } => Some(id),
            HerdAct::RequestedItems { .. } | HerdAct::RoundupDone => None,
        }
    }
}

/// Ce que ces trois buts retiennent entre deux ticks, et qui n'est pas un fait du monde.
///
/// Java référence : les champs de `EntityAIWorkStablemaster` (`:73`, `:76`, `:78-80`,
/// `:84`) et la main du citoyen, plus la mémoire de base réutilisée telle quelle
/// ([`Memory`] : `delay`, `hasDelayed`, `needsCurrently` sont les mêmes champs pour tous
/// les métiers, et `crate::ai::worker` les possède déjà).
#[derive(Clone, Debug, PartialEq)]
pub struct HerdMemory {
    /// `AbstractEntityAIBasic.delay`, `.hasDelayed`, `.needsCurrently` (`:121`, `:126`,
    /// `:156`).
    pub base: Memory,
    /// Ce que le citoyen tient en main, tel que ce module l'a décidé.
    pub hand: Hand,
    /// `EntityAIWorkStablemaster.trainingCooldown` (`:76`), initialisé à
    /// `MAX_TRAINING_COOLDOWN` (`:76`) et décrémenté à chaque passage en `DECIDE`
    /// (`:235`).
    pub train_cooldown: i32,
    /// `EntityAIWorkStablemaster.roundupCooldown` (`:73`), initialisé à
    /// `MAX_ROUNDUP_COOLDOWN` (`:73`) et décrémenté à chaque `DECIDE` (`:236`).
    pub roundup_cooldown: i32,
    /// `horseToConvert` (`:78`).
    pub to_convert: Option<u32>,
    /// `horseToGetReady` (`:79`).
    pub to_get_ready: Option<u32>,
    /// `horseToRetrieve` (`:80`).
    pub to_retrieve: Option<u32>,
    /// `wanderingHorses` (`:84`) : la liste ramenée par le dernier
    /// `findNearbyUnstabledHorses` (`:253`), vidée au fur et à mesure des rentrées
    /// (`:603-606`).
    pub wandering: Vec<u32>,
    /// `worker.getOffhandItem()` porte une laisse (`:685-688`, `:719-722`).
    pub lead_in_offhand: bool,
    /// Le dernier effet appliqué, celui de ce tick.
    pub act: Option<HerdAct>,
}

impl Default for HerdMemory {
    /// Une mémoire neuve : les deux comptes à rebours à leur maximum — donc **aucun**
    /// dressage ni rentrée avant cinquante, respectivement cent, passages en `DECIDE`.
    fn default() -> HerdMemory {
        HerdMemory {
            base: Memory::default(),
            hand: Hand::Empty,
            train_cooldown: super::stablemaster::MAX_TRAINING_COOLDOWN,
            roundup_cooldown: super::stablemaster::MAX_ROUNDUP_COOLDOWN,
            to_convert: None,
            to_get_ready: None,
            to_retrieve: None,
            wandering: Vec::new(),
            lead_in_offhand: false,
            act: None,
        }
    }
}

/// La scène entière qu'une de ces décisions lit : les faits de base, l'enclos, les
/// tirages, les faits du métier et la mémoire de l'IA.
///
/// [`WorldFacts`] est *la même valeur* que prennent les tables de `crate::ai` — rien n'en
/// est copié — donc un appelant qui fait aussi tourner `crate::ai::worker::JobAi` peut
/// donner à chacun celle qu'il a déjà remplie. Le champ `job` de [`WorldFacts`] n'est pas
/// lu par ce module : les faits du métier sont [`HerdScene::job`].
#[derive(Clone, Debug, PartialEq)]
pub struct HerdScene {
    /// Le métier de cette scène.
    pub trade: super::HerdTrade,
    /// Le citoyen, son inventaire, sa hutte et ses blocs (`crate::ai::facts`).
    pub facts: WorldFacts,
    /// L'enclos et la décision de base.
    pub pen: PenFacts,
    /// Les tirages de ce tick.
    pub draws: Draws,
    /// Les faits du métier.
    pub job: HerdJob,
    /// Ce que l'IA retient.
    pub memory: HerdMemory,
    /// Le refus du tick, effacé au début de chaque tick.
    pub refusal: Option<TradeRefusal>,
}

impl HerdScene {
    /// Une scène sur `facts` et les faits vides du métier `trade`.
    pub fn new(facts: WorldFacts, trade: super::HerdTrade) -> HerdScene {
        HerdScene {
            trade,
            facts,
            pen: PenFacts::default(),
            draws: Draws::default(),
            job: trade.job_facts(),
            memory: HerdMemory::default(),
            refusal: None,
        }
    }

    /// Une scène portant explicitement `job` — pour le test qui donne les faits d'un
    /// métier à l'IA d'un autre.
    pub fn with_job(facts: WorldFacts, trade: super::HerdTrade, job: HerdJob) -> HerdScene {
        HerdScene {
            job,
            ..HerdScene::new(facts, trade)
        }
    }

    /// Les faits du lapinier, si c'en est un.
    pub fn rabbit(&self) -> Option<RabbitFacts> {
        match self.job {
            HerdJob::Rabbit(facts) => Some(facts),
            _ => None,
        }
    }

    /// Les faits du berger, si c'en est un.
    pub fn shepherd(&self) -> Option<ShepherdFacts> {
        match self.job {
            HerdJob::Shepherd(facts) => Some(facts),
            _ => None,
        }
    }

    /// Les faits du palefrenier, si c'en est un.
    pub fn stable(&self) -> Option<StableFacts> {
        match self.job {
            HerdJob::Stable(facts) => Some(facts),
            _ => None,
        }
    }

    /// Vrai quand les faits portés sont bien ceux du métier de la scène.
    pub fn job_matches(&self) -> bool {
        self.job.trade() == self.trade
    }

    /// Déposer la réponse de la table de base : `super.decideWhatToDo()` a rendu
    /// `choice` (`EntityAIWorkShepherd.java:75`).
    pub fn decide(&mut self, choice: HerdChoice) {
        self.pen.base_answer = Some(choice);
    }

    /// Consommer cette réponse. `None` veut dire que la table de base n'a rien déposé
    /// depuis le dernier passage de la surcharge.
    pub fn take_base_answer(&mut self) -> Option<HerdChoice> {
        self.pen.base_answer.take()
    }

    /// Enregistrer pourquoi ce tick ne peut pas avancer. Le dernier gagne, comme
    /// `crate::ai::worker::Scene::refuse`.
    pub fn refuse(&mut self, refusal: TradeRefusal) {
        self.refusal = Some(refusal);
    }

    /// L'état de la machine après un tick, quand elle est dans un état de cette tranche.
    pub fn enters(&self, state: HerdState) -> bool {
        !state.is_traversed() && !state.belongs_to_other_trade(self.trade)
    }
}
