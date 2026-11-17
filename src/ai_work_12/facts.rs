//! Ce qu'un but de ces trois métiers demande au monde, écrit en observations.
//!
//! Référence Java : `core/entity/ai/workers/service/{EntityAIWorkEnchanter,
//! EntityAIWorkHealer,EntityAIWorkUndertaker}.java`, plus les classes qu'ils lisent —
//! `colony/buildings/modules/EnchanterStationsModule.java` (`:94-113`),
//! `colony/buildings/modules/GraveyardManagementModule.java` (`:255-332`),
//! `core/entity/ai/workers/util/Patient.java` (les trois états),
//! `core/datalistener/model/Disease.java` (les remèdes) et `colony/jobs/JobEnchanter.java`.
//!
//! **Pourquoi la moitié de base est réutilisée et l'autre non.** Le citoyen, son
//! inventaire, sa hutte et la carte de blocs existent déjà dans `crate::ai::facts` et ce
//! module les lit là ([`WorldFacts`], [`Pos`]). La moitié du métier, elle, n'existe pas :
//! `WorldFacts::job` est une `JobFacts` à deux variantes (composteur, bâtisseur) et ce
//! fichier est hors de la zone d'écriture de ce module, donc les faits des trois métiers
//! vivent dans [`ServiceJob`] et [`ServiceScene`] **enveloppe** `WorldFacts` au lieu de
//! l'étendre. Rien n'est recopié : `ServiceScene::facts` est la même valeur.
//!
//! **Aucun tirage n'est une décision.** Chaque champ `*_draw` / `*_roll` est un nombre
//! que Java a obtenu de `worker.getRandom()` ou de `MathUtils.RANDOM`. Le port ne tire
//! jamais : une décision qu'on ne peut pas rejouer ne peut pas être testée, donc
//! l'appelant fournit le nombre et le champ nomme l'appel qu'il remplace.

use crate::ai::facts::{Pos, WorldFacts};
use crate::ai::worker::Memory;
use crate::ai_build::crafter::CraftFacts;
use crate::citizens::happiness::disease::Disease;

use super::enchanter::MANA_REQ_PER_LEVEL;
use super::healer::{
    PLAYER_HEAL_MARGIN, PLAYER_MIN_WOUND, PLAYER_RANGE, REMOTE_HEALTH, REMOTE_RANGE_PER_LEVEL,
};
use super::undertaker::{
    EFFORT_BURY, EFFORT_EMPTY_GRAVE, EFFORT_RESURRECT, TOTEM_BREAK_CHANCE, WALK_RANGE_TO_GRAVE,
};

// ------------------------------------------------------------------- enchanteur

/// Un citoyen affecté au bâtiment que l'enchanteur draine.
///
/// Référence Java : `getModuleForJob().getAssignedEntities()`
/// (`EntityAIWorkEnchanter.java:329-330`), `citizen.getCitizenData()`
/// (`:349`) et `citizenToGatherFrom.getEntity().isPresent()` (`:354`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrainWorker {
    /// `AbstractEntityCitizen.getId()` : ce que la mémoire retient du choix.
    pub id: u32,
    /// `citizen.blockPosition()` : la distance que `:363` mesure.
    pub at: Pos,
    /// `citizenToGatherFrom.getEntity().isPresent()` (`:354`).
    pub present: bool,
}

impl DrainWorker {
    /// Un citoyen présent à `at`.
    pub fn present(id: u32, at: Pos) -> DrainWorker {
        DrainWorker {
            id,
            at,
            present: true,
        }
    }
}

/// Ce que l'enchanteur lit du monde (`EntityAIWorkEnchanter.java:121-433`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EnchanterFacts {
    /// L'atelier, déjà porté : `getNextCraftingState()` lui appartient
    /// (`AbstractEntityAICrafting.java:298-317`) et `crate::ai_build::crafter` en porte
    /// la règle et les tests. L'enchanteur hérite de `AbstractEntityAICrafting`
    /// (`EntityAIWorkEnchanter.java:53`), donc il lit les mêmes faits.
    pub craft: CraftFacts,
    /// `WorldUtil.isPastTime(world, 6000)` = `world.getDayTime() % 24000 <= 6000`
    /// (`WorldUtil.java:176-179`, lu à `EntityAIWorkEnchanter.java:130`).
    pub past_six_thousand: bool,
    /// `wantInventoryDumped()` (`:135`). L'enchanteur ne l'écrase pas :
    /// `AbstractEntityAIBasic.wantInventoryDumped()` rend `false` (`:435-438`).
    pub want_inventory_dumped: bool,
    /// `getPrimarySkillLevel()` (`:141`, `:213`) : la compétence du métier, brute.
    pub primary_skill: i32,
    /// `getSecondarySkillLevel()` (`:412`, `:420`) : la mana, brute `0..99`. Elle donne
    /// `attempts = mana / 5` essais d'enchantement par drain (`:412`).
    pub mana_level: i32,
    /// `building.getBuildingLevel()` (`:141`, `:235`) : divise la durée d'un
    /// enchantement et multiplie l'exigence de mana.
    pub building_level: u8,
    /// `module.getBuildingsToGatherFrom()` (`:145`) : non vide, il y a des stations.
    pub drain_stations: Vec<Pos>,
    /// La même liste filtrée par `!k.getValue()` — les stations **pas encore** drainées
    /// (`EnchanterStationsModule.java:107`), celles parmi lesquelles
    /// `getRandomBuildingToDrainFrom` tire.
    pub drainable: Vec<Pos>,
    /// **Pas un appel Java.** `random.nextInt(drainable.size())`
    /// (`EnchanterStationsModule.java:112`) : l'appelant tire.
    pub drain_draw: usize,
    /// `InventoryUtils.getItemCountInItemHandler(worker.getInventoryCitizen(), IS_BOOK)`
    /// (`:155`).
    pub books_in_inventory: u32,
    /// `InventoryUtils.hasBuildingEnoughElseCount(building, IS_BOOK, 1)` (`:158`).
    pub books_in_building: u32,
    /// La même paire pour `ModItems.ancientTome` (`:190`, `:193`).
    pub ancient_tomes_in_inventory: u32,
    /// `:193`.
    pub ancient_tomes_in_building: u32,
    /// `castingModule.isDisabled(token)` sur une recette dont l'entrée est un tome
    /// (`:179-186`).
    pub ancient_tome_crafting_disabled: bool,
    /// `building.getColony().getServerBuildingManager().getBuilding(pos) != null`
    /// (`:313`).
    pub drain_building_exists: bool,
    /// `walkToBuilding(buildingWorker)` (`:314`) : le citoyen est arrivé.
    pub at_drain_building: bool,
    /// Les citoyens du bâtiment à drainer (`:329-330`).
    pub workers: Vec<DrainWorker>,
    /// **Pas un appel Java.** `random.nextInt(workers.size())` (`:337`).
    pub worker_draw: usize,
    /// `InventoryUtils.findFirstSlotInItemHandlerWith(worker.getInventoryCitizen(),
    /// Items.BOOK) != -1` (`:408`).
    pub book_in_hand: bool,
    /// `getFirstFulfillableRecipe(ItemStackUtils::isEmpty, 1, false) != null`
    /// (`:228`).
    pub enchant_recipe_available: bool,
    /// `enchantmentLevel` du butin, c'est-à-dire ce que
    /// `incrementLevel(Skill.Mana, -enchantmentLevel)` retire (`:274-279`).
    pub enchant_mana_cost: i32,
}

impl EnchanterFacts {
    /// La station tirée, `None` quand toutes sont déjà drainées
    /// (`EnchanterStationsModule.java:108-112`). Le tirage est ramené modulo la liste :
    /// un tirage périmé rend une station au lieu de paniquer.
    pub fn random_drain_building(&self) -> Option<Pos> {
        if self.drainable.is_empty() {
            return None;
        }
        self.drainable.get(self.drain_draw % self.drainable.len()).copied()
    }

    /// Le citoyen tiré, par identifiant (`:337`, `:349`).
    pub fn worker(&self, id: u32) -> Option<&DrainWorker> {
        self.workers.iter().find(|worker| worker.id == id)
    }

    /// `getPrimarySkillLevel() < building.getBuildingLevel() * MANA_REQ_PER_LEVEL`
    /// (`:141`) : la branche qui décide entre drainer et enchanter.
    pub fn needs_drain(&self) -> bool {
        self.primary_skill < i32::from(self.building_level) * MANA_REQ_PER_LEVEL
    }

    /// Le premier citoyen de la liste (`:346`) — le cas d'un seul travailleur, que Java
    /// prend sans tirer.
    pub fn only_worker(&self) -> Option<&DrainWorker> {
        self.workers.first()
    }
}

// ------------------------------------------------------------------- guérisseur

/// `Patient.PatientState` (`core/entity/ai/workers/util/Patient.java:16-21`) : les trois
/// états d'un dossier, dans l'ordre de la déclaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
// exhaustif : trois états dans Patient.java
pub enum PatientState {
    /// `NEW` : une demande de remède est à créer.
    New,
    /// `REQUESTED` : la demande est partie (ou le remède est là).
    Requested,
    /// `TREATED` : les remèdes ont été donnés, le patient les porte.
    Treated,
}

impl PatientState {
    /// L'identifiant Java.
    pub fn java(self) -> &'static str {
        match self {
            PatientState::New => "NEW",
            PatientState::Requested => "REQUESTED",
            PatientState::Treated => "TREATED",
        }
    }
}

/// Combien d'un remède un inventaire donné porte.
///
/// Référence Java : `InventoryUtils.getItemCountInItemHandler(handler,
/// Disease.hasCureItem(cure))` (`EntityAIWorkHealer.java:529`, `:346`, `:361`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CureStock {
    /// L'identifiant d'objet du remède, `cure.getItem()` (`Disease.java:24`).
    pub item: &'static str,
    /// Le compte observé.
    pub count: u32,
}

impl CureStock {
    /// Une ligne.
    pub fn new(item: &'static str, count: u32) -> CureStock {
        CureStock { item, count }
    }
}

/// `hasCureInInventory(disease, handler)` (`EntityAIWorkHealer.java:525-535`) : **tous**
/// les remèdes de la maladie, chacun au moins à son compte.
pub fn holds(cures: &[CureStock], disease: &Disease) -> bool {
    disease
        .cures
        .iter()
        .all(|cure| count_of(cures, cure.item) >= cure.count.max(0) as u32)
}

/// `InventoryUtils.getItemCountInItemHandler(handler, item)` : zéro quand la ligne
/// n'existe pas, comme un inventaire qui ne porte pas l'objet.
pub fn count_of(cures: &[CureStock], item: &str) -> u32 {
    cures.iter()
        .find(|stock| stock.item == item)
        .map(|stock| stock.count)
        .unwrap_or(0)
}

/// Un dossier de patient, tel que `decide` le lit (`EntityAIWorkHealer.java:118-215`).
#[derive(Clone, Debug, PartialEq)]
pub struct Patient {
    /// `patient.getId()` — l'identifiant du citoyen.
    pub id: u32,
    /// `patient.getState()` (`Patient.java:69-72`).
    pub state: PatientState,
    /// `citizen.getCitizenData().getCitizenDiseaseHandler().getDisease()` (`:127`),
    /// `None` quand il n'y a plus de maladie à soigner. `Disease` est `Copy`
    /// (`citizens/happiness/disease.rs`), donc le dossier porte la valeur et non une
    /// référence : `disease == null` de Java devient `None`.
    pub disease: Option<Disease>,
    /// `data != null && data.getEntity().isPresent()` (`:121`) : le citoyen est là.
    pub present: bool,
    /// `...getCitizenDiseaseHandler().isSick()` (`:121`).
    pub sick: bool,
    /// `citizen.getInventoryCitizen().hasSpace()` (`:149`, `:363`).
    pub has_space: bool,
    /// Les remèdes que **le patient** porte (`:361`).
    pub cures: Vec<CureStock>,
}

impl Default for Patient {
    /// Un dossier `NEW` d'un citoyen présent et malade, sans maladie connue.
    fn default() -> Patient {
        Patient {
            id: 0,
            state: PatientState::New,
            disease: None,
            present: true,
            sick: true,
            has_space: true,
            cures: Vec::new(),
        }
    }
}

impl Patient {
    /// Un patient malade à `state` de la maladie `disease`.
    pub fn sick_of(id: u32, state: PatientState, disease: Disease) -> Patient {
        Patient {
            id,
            state,
            disease: Some(disease),
            ..Patient::default()
        }
    }

    /// Le dossier désigne-t-il encore un citoyen soignable ? C'est le test de
    /// `:121`, `:249` et `:319` en une réponse.
    pub fn soignable(&self) -> bool {
        self.present && self.sick
    }
}

/// Un joueur blessé dans l'hôpital.
///
/// Référence Java : `WorldUtil.getEntitiesWithinBuilding(world, Player.class, building,
/// player -> player.getHealth() < player.getMaxHealth() - 10 - (2 *
/// building.getBuildingLevel()))` (`EntityAIWorkHealer.java:217-220`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InjuredPlayer {
    /// `player.getId()`.
    pub id: u32,
    /// `player.blockPosition()` (`:452`, `:458`).
    pub at: Pos,
    /// `player.getHealth()` (`:463`).
    pub health: f64,
    /// `player.getMaxHealth()`.
    pub max_health: f64,
}

impl InjuredPlayer {
    /// Le prédicat de `:220`, gardé ici pour être testable :
    /// `health < max - 10 - 2 * niveau`.
    pub fn wounded(&self, building_level: u8) -> bool {
        self.health < self.max_health - PLAYER_MIN_WOUND - 2.0 * f64::from(building_level)
    }

    /// `heal(maxHealth - health - 5 - buildingLevel)` (`:463`).
    pub fn healed_by(&self, building_level: u8) -> f64 {
        self.max_health - self.health - PLAYER_HEAL_MARGIN - f64::from(building_level)
    }
}

/// Le citoyen lointain que le guérisseur va trouver (`:226-232`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RemotePatient {
    /// `data.getId()`.
    pub id: u32,
    /// `data.getEntity().get().blockPosition()`.
    pub at: Pos,
    /// `...getHealth()` (`:227`).
    pub health: f64,
    /// `getMaxHealth()` (`:499`).
    pub max_health: f64,
}

impl RemotePatient {
    /// `getHealth() < 10.0` (`:227`).
    pub fn critical(&self) -> bool {
        self.health < REMOTE_HEALTH
    }

    /// `BlockPosUtil.getDistance2D(..., building.getPosition()) < niveau * 40`
    /// (`:228`).
    pub fn reachable(&self, hut: Pos, building_level: u8) -> bool {
        self.at.dist_2d(hut) < f64::from(building_level) * REMOTE_RANGE_PER_LEVEL
    }

    /// `heal(maxHealth - health - 5 - buildingLevel)` (`:499`).
    pub fn healed_by(&self, building_level: u8) -> f64 {
        self.max_health - self.health - PLAYER_HEAL_MARGIN - f64::from(building_level)
    }
}

/// Ce que le guérisseur lit du monde (`EntityAIWorkHealer.java:104-516`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HealerFacts {
    /// `walkToBuilding()` (`:106`).
    pub at_hospital: bool,
    /// Les citoyens malades **dans** le bâtiment (`:112-113`), dont Java crée le
    /// dossier (`:115`).
    pub sick_nearby: Vec<u32>,
    /// `hospital.getPatients()` (`:118`).
    pub patients: Vec<Patient>,
    /// Les remèdes du guérisseur (`:529` sur `worker.getInventoryCitizen()`).
    pub worker_cures: Vec<CureStock>,
    /// Les remèdes des coffres de l'hôpital (`:529` sur la capability de `building`).
    pub hut_cures: Vec<CureStock>,
    /// Les remèdes pour lesquels une requête `Stack` est **ouverte ou servie**
    /// (`:158-186`, `:269-293`) : `getOpenRequestsOfType` plus
    /// `getCompletedRequestsOfType`, réduits à l'identifiant de l'objet.
    pub requested_cures: Vec<&'static str>,
    /// Les joueurs dans l'hôpital (`:217`), le prédicat de `:220` appliqué par
    /// [`InjuredPlayer::wounded`].
    pub players: Vec<InjuredPlayer>,
    /// `building.getColony().getCitizenManager().getRandomCitizen()` (`:226`).
    pub remote: Option<RemotePatient>,
    /// `walkToSafePos` / `walkToUnSafePos` sur la cible de l'état courant
    /// (`:256`, `:326`, `:405`, `:458`, `:488`).
    pub at_target: bool,
    /// `getSecondarySkillLevel()` = `Skill.Mana` (`:515`) : le diviseur du tirage de
    /// guérison gratuite.
    pub mana_level: i32,
    /// `building.getBuildingLevel()` (`:220`, `:228`, `:463`).
    pub building_level: u8,
    /// **Pas un appel Java.** `worker.getRandom().nextInt(60 * 60)` (`:515`).
    pub cure_roll: i32,
}

impl HealerFacts {
    /// Le dossier de `id`, qu'il soit soignable ou non : `cure` doit pouvoir constater
    /// qu'il ne l'est plus (`:248-253`).
    pub fn patient(&self, id: u32) -> Option<&Patient> {
        self.patients.iter().find(|patient| patient.id == id)
    }

    /// Le joueur de `id`.
    pub fn player(&self, id: u32) -> Option<&InjuredPlayer> {
        self.players.iter().find(|player| player.id == id)
    }

    /// Une requête de ce remède est-elle déjà partie ? (`:170-186`, `:278-293`).
    pub fn cure_requested(&self, item: &str) -> bool {
        self.requested_cures.iter().any(|known| *known == item)
    }

    /// Le premier joueur que le prédicat de `:220` retient.
    pub fn wounded_player(&self) -> Option<&InjuredPlayer> {
        self.players
            .iter()
            .find(|player| player.wounded(self.building_level))
    }

    /// Le citoyen lointain, s'il est sous dix points de vie et dans le rayon
    /// (`:227-228`).
    pub fn remote_in_reach(&self, hut: Pos) -> Option<&RemotePatient> {
        self.remote
            .as_ref()
            .filter(|remote| remote.critical() && remote.reachable(hut, self.building_level))
    }
}

// ------------------------------------------------------------------ croque-mort

/// Ce que le croque-mort lit du monde (`EntityAIWorkUndertaker.java:114-499`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UndertakerFacts {
    /// `module.getLastGraveData() != null` (`:121`, `:445`) : une inhumation
    /// interrompue par la nuit reprend en premier.
    pub last_grave_data: bool,
    /// `building.getGraveToWorkOn()` (`:127`, `:189`).
    pub grave_to_work_on: Option<Pos>,
    /// `world.getBlockEntity(currentGrave) instanceof TileEntityGrave` (`:136`).
    pub grave_is_grave: bool,
    /// `((TileEntityGrave) entity).isEmpty()` (`:200`).
    pub grave_empty: bool,
    /// `InventoryUtils.transferAllItemHandler(grave.getInventory(), worker.getInventoryCitizen())`
    /// (`:219`) : le transfert a réussi.
    pub grave_emptied: bool,
    /// `walkWithProxy(gravePos, 3)` a rendu faux (`:192`, `:253`, `:329`) : arrivé.
    pub at_grave: bool,
    /// `checkForToolOrWeapon(ModEquipmentTypes.shovel.get())` a rendu vrai (`:180`,
    /// `:237`, `:288`, `:313`, `:445`) : pas de pelle de niveau ≥
    /// `TOOL_LEVEL_WOOD_OR_GOLD`, une requête `Tool` est ouverte.
    pub shovel_missing: bool,
    /// `mineBlock(position)` (`:292`) : la tombe est cassée.
    pub dig_done: bool,
    /// `module.getRandomFreeVisualGravePos()` (`:453`) : une place libre, `None` quand le
    /// cimetière est plein (`:266-269` de `GraveyardManagementModule`).
    pub burial_search: Option<Pos>,
    /// `world.getBlockState(burialPos.getA()).canBeReplaced()` (`:451`).
    pub burial_replaceable: bool,
    /// `walkWithProxy(burialPos.getA(), 4)` a rendu faux (`:470`).
    pub at_burial: bool,
    /// `module.buryCitizenHere(burialPos, worker)` (`:485`).
    pub bury_succeeds: bool,
    /// `getPrimarySkillLevel()` (`:213`, `:343`, `:479`) : les points d'effort par geste.
    pub primary_skill: i32,
    /// `getSecondarySkillLevel()` = `Skill.Mana` (`:343`, `:391`).
    pub mana_level: i32,
    /// `building.getBuildingLevel()` (`:390`).
    pub building_level: u8,
    /// `getServerBuildingManager().getMysticalSiteMaxBuildingLevel()` (`:396`).
    pub mystical_max_level: u8,
    /// `getResearchEffects().getEffectStrength(RESURRECT_CHANCE)` (`:392`).
    pub resurrect_research: f64,
    /// `...getEffectStrength(USE_TOTEM) > 0` (`:412`).
    pub use_totem_research: bool,
    /// `InventoryUtils.getItemCountInItemHandler(worker.getInventoryCitizen(),
    /// Items.TOTEM_OF_UNDYING)` (`:414`).
    pub totems: u32,
    /// **Pas un appel Java.** Le `random.nextDouble()` du bris de totem (`:351`).
    pub totem_roll: f64,
    /// **Pas un appel Java.** Le `random.nextDouble()` de la chance (`:357`).
    pub resurrect_roll: f64,
    /// `building.isInBuilding(worker.blockPosition())` (`:157`).
    pub in_building: bool,
}

impl UndertakerFacts {
    /// Les trois efforts que Java compare, avec la ligne qui les déclare : un test les
    /// vérifie une fois pour toutes.
    pub fn efforts() -> [(&'static str, i32, &'static str); 3] {
        [
            ("EFFORT_EMPTY_GRAVE", EFFORT_EMPTY_GRAVE, "UndertakerConstants.java:61"),
            ("EFFORT_BURY", EFFORT_BURY, "UndertakerConstants.java:66"),
            ("EFFORT_RESURRECT", EFFORT_RESURRECT, "UndertakerConstants.java:71"),
        ]
    }

    /// `checkForToolOrWeapon(shovel)` plus `getGraveToWorkOn() == null`, la condition
    /// des quatre états (`:180`, `:237`, `:313`, `:445`).
    pub fn cannot_work(&self) -> bool {
        self.shovel_missing || self.grave_to_work_on.is_none()
    }

    /// `canBeReplaced()` + la recherche d'une place (`:451-454`), en une réponse : où le
    /// croque-mort va creuser.
    pub fn burial_spot(&self, remembered: Option<Pos>) -> Option<Pos> {
        match remembered {
            Some(pos) if self.burial_replaceable => Some(pos),
            _ => self.burial_search,
        }
    }

    /// Le rayon du `walkWithProxy(gravePos, 3)` (`:192`).
    pub fn grave_walk_range() -> i32 {
        WALK_RANGE_TO_GRAVE
    }

    /// Le `random.nextDouble() <= TOTEM_BREAK_CHANCE` (`:351`).
    pub fn totem_breaks(&self) -> bool {
        self.totem_roll <= TOTEM_BREAK_CHANCE
    }
}

// ----------------------------------------------------------------------- scène

/// La moitié métier des faits.
///
/// Un métier que ce module ne porte pas rend [`ServiceJob::Unported`], pas une variante
/// vide : « ce travailleur n'a pas de tombe aujourd'hui » et « ce port ne sait pas ce que
/// fait ce travailleur » ne doivent pas se lire pareil.
#[derive(Clone, Debug, PartialEq)]
// exhaustif : les trois métiers de cette tranche, plus « non porté »
pub enum ServiceJob {
    /// L'enchanteur (`EntityAIWorkEnchanter.java:53`).
    Enchanter(EnchanterFacts),
    /// Le guérisseur (`EntityAIWorkHealer.java:38`).
    Healer(HealerFacts),
    /// Le croque-mort (`EntityAIWorkUndertaker.java:54`).
    Undertaker(UndertakerFacts),
    /// Ce métier n'est pas dans ce module ; la table de base tourne quand même.
    Unported,
}

impl Default for ServiceJob {
    /// Aucun métier : une scène sans faits de métier est celle qu'on remplit après.
    fn default() -> ServiceJob {
        ServiceJob::Unported
    }
}

/// Ce que l'enchanteur retient entre deux ticks (`EntityAIWorkEnchanter.java:93-98`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EnchanterMemory {
    /// `progressTicks` (`:98`) : où en est le drain ou l'enchantement.
    pub progress_ticks: i32,
    /// `JobEnchanter.waitingTicks` (`JobEnchanter.java:32`) : les ticks passés à
    /// attendre un citoyen trop loin.
    pub waiting_ticks: i32,
    /// `JobEnchanter.posToDrainFrom` (`JobEnchanter.java:27`) : l'état du **métier**,
    /// persisté, donc dans la mémoire et non dans les faits.
    pub drain_target: Option<Pos>,
    /// `citizenToGatherFrom` (`EntityAIWorkEnchanter.java:93`) : l'entité, réduite à son
    /// identifiant parce qu'une poignée d'entité n'est pas une donnée rejouable.
    pub drain_citizen: Option<u32>,
}

/// Ce que le guérisseur retient (`EntityAIWorkHealer.java:53-73`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HealerMemory {
    /// `currentPatient` (`:53`) : un patient à la fois, par identifiant.
    pub current_patient: Option<u32>,
    /// `progressTicks` (`:58`) : les ticks de la guérison gratuite.
    pub progress_ticks: i32,
    /// `playerToHeal` (`:73`).
    pub player_to_heal: Option<u32>,
    /// `remotePatient` (`:68`).
    pub remote_patient: Option<u32>,
}

/// Ce que le croque-mort retient (`EntityAIWorkUndertaker.java:64-79`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UndertakerMemory {
    /// `effortCounter` (`:64`) : les points d'effort du geste en cours.
    pub effort_counter: i32,
    /// `shouldDumpInventory` (`:69`), que `wantInventoryDumped` consomme (`:507-515`).
    pub should_dump_inventory: bool,
    /// `burialPos` (`:79`) : la place de tombe choisie, avec sa direction réduite à la
    /// position.
    pub burial_pos: Option<Pos>,
}

/// Toute la mémoire d'un de ces métiers. `base` est la mémoire de
/// `crate::ai::worker`, parce que `setDelay` et `hasNotDelayed` sont les mêmes deux
/// champs pour tous les métiers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ServiceMemory {
    /// `AbstractEntityAIBasic.delay` et `.hasDelayed` (`:121`, `:126`).
    pub base: Memory,
    /// L'enchanteur.
    pub enchanter: EnchanterMemory,
    /// Le guérisseur.
    pub healer: HealerMemory,
    /// Le croque-mort.
    pub undertaker: UndertakerMemory,
}

/// La scène qu'une décision lit : les faits de base, les faits du métier, la mémoire, et
/// les deux sorties.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ServiceScene {
    /// Le citoyen, son inventaire, sa hutte et ses blocs : la **même** valeur que celle
    /// que prennent les tables de `crate::ai`.
    pub facts: WorldFacts,
    /// Les observations du métier.
    pub job: ServiceJob,
    /// Ce que l'IA retient entre deux ticks.
    pub memory: ServiceMemory,
    /// Le dernier refus, remis à zéro au début de chaque tick.
    pub refusal: Option<super::state::ServiceRefusal>,
    /// Ce que le tick demande à l'appelant, remis à zéro au début de chaque tick. Ni un
    /// fait ni une mémoire : un ordre.
    pub effects: Vec<super::Effect>,
}

impl ServiceScene {
    /// Une scène sur `facts` avec une mémoire vide et les faits `job`.
    pub fn new(facts: WorldFacts, job: ServiceJob) -> ServiceScene {
        ServiceScene {
            facts,
            job,
            memory: ServiceMemory::default(),
            refusal: None,
            effects: Vec::new(),
        }
    }

    /// Les faits de l'enchanteur, si c'en est un.
    pub fn enchanter(&self) -> Option<&EnchanterFacts> {
        match &self.job {
            ServiceJob::Enchanter(facts) => Some(facts),
            _ => None,
        }
    }

    /// Les faits du guérisseur, si c'en est un.
    pub fn healer(&self) -> Option<&HealerFacts> {
        match &self.job {
            ServiceJob::Healer(facts) => Some(facts),
            _ => None,
        }
    }

    /// Les faits du croque-mort, si c'en est un.
    pub fn undertaker(&self) -> Option<&UndertakerFacts> {
        match &self.job {
            ServiceJob::Undertaker(facts) => Some(facts),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_work_12::undertaker::{resurrect_chance, totem_bonus};

    fn carrot_and_potato() -> Disease {
        Disease::INFLUENZA
    }

    #[test]
    fn the_default_scene_carries_no_trade_facts() {
        let scene = ServiceScene::default();
        assert_eq!(scene.job, ServiceJob::Unported);
        assert!(scene.enchanter().is_none());
        assert!(scene.healer().is_none());
        assert!(scene.undertaker().is_none());
        assert!(scene.effects.is_empty());
        assert_eq!(scene.memory, ServiceMemory::default());
    }

    #[test]
    fn a_drain_draw_wraps_over_the_drainable_list_and_an_empty_one_has_none() {
        // `EnchanterStationsModule.java:108-112` : `nextInt(size)` ne peut pas déborder,
        // un tirage périmé est ramené modulo.
        let facts = EnchanterFacts {
            drainable: vec![Pos::new(1, 64, 1), Pos::new(2, 64, 2)],
            drain_draw: 3,
            ..EnchanterFacts::default()
        };
        assert_eq!(facts.random_drain_building(), Some(Pos::new(2, 64, 2)));
        let all_gathered = EnchanterFacts {
            drainable: Vec::new(),
            ..EnchanterFacts::default()
        };
        assert_eq!(all_gathered.random_drain_building(), None);
        // Une liste vide de travailleurs n'a pas de « premier ».
        assert!(all_gathered.only_worker().is_none());
    }

    #[test]
    fn the_drain_branch_is_level_times_ten_mana() {
        // `EntityAIWorkEnchanter.java:141` : `MANA_REQ_PER_LEVEL` = 10 (`:83`).
        let mut facts = EnchanterFacts {
            building_level: 3,
            primary_skill: 29,
            ..EnchanterFacts::default()
        };
        assert!(facts.needs_drain(), "29 < 30");
        facts.primary_skill = 30;
        assert!(!facts.needs_drain(), "30 n'est pas < 30");
        facts.building_level = 0;
        assert!(!facts.needs_drain(), "un niveau 0 n'exige rien");
    }

    #[test]
    fn a_cure_needs_every_item_of_the_disease_and_enough_of_it() {
        // `EntityAIWorkHealer.java:525-535` : la boucle rend faux au premier remède
        // manquant, donc il faut tous les remèdes.
        let disease = carrot_and_potato();
        let complete = vec![CureStock::new("minecraft:carrot", 1), CureStock::new("minecraft:potato", 1)];
        assert!(holds(&complete, &disease));
        let missing = vec![CureStock::new("minecraft:carrot", 1)];
        assert!(!holds(&missing, &disease), "il manque la pomme de terre");
        let short = vec![CureStock::new("minecraft:carrot", 0), CureStock::new("minecraft:potato", 1)];
        assert!(!holds(&short, &disease), "zéro carotte ne suffit pas");
        assert_eq!(count_of(&short, "minecraft:kelp"), 0, "un objet absent compte zéro");
    }

    #[test]
    fn a_player_is_wounded_ten_points_below_its_max_minus_two_levels() {
        // `EntityAIWorkHealer.java:220`.
        let player = InjuredPlayer {
            id: 1,
            at: Pos::new(0, 64, 0),
            health: 9.0,
            max_health: 20.0,
        };
        // niveau 0 : le seuil est 10, donc 9 < 10 est blessé.
        assert!(player.wounded(0));
        // niveau 5 : le seuil est 0, donc 9 n'est plus blessé.
        assert!(!player.wounded(5));
        // `:463` : `max - health - 5 - niveau`.
        assert_eq!(player.healed_by(0), 6.0);
        assert_eq!(player.healed_by(2), 4.0);
    }

    #[test]
    fn a_remote_patient_must_be_under_ten_and_within_level_times_forty() {
        // `EntityAIWorkHealer.java:227-228`.
        let hut = Pos::new(0, 64, 0);
        let near = RemotePatient {
            id: 7,
            at: Pos::new(3, 64, 4), // 5 blocs
            health: 9.0,
            max_health: 20.0,
        };
        assert!(near.critical());
        assert!(near.reachable(hut, 1), "5 < 40");
        assert!(!near.reachable(hut, 0), "un niveau 0 ne voit personne");
        let healthy = RemotePatient { health: 10.0, ..near };
        assert!(!healthy.critical(), "10 n'est pas < 10");
        let far = RemotePatient {
            at: Pos::new(40, 64, 0),
            ..near
        };
        assert!(!far.reachable(hut, 1), "40 n'est pas < 40");
        // `:499` : la même formule de soin que pour un joueur.
        assert_eq!(near.healed_by(1), 20.0 - 9.0 - 5.0 - 1.0);
    }

    #[test]
    fn the_resurrection_chance_adds_three_terms_and_caps_them() {
        // `EntityAIWorkUndertaker.java:387-403`.
        let facts = UndertakerFacts {
            building_level: 2,   // 2 * 0.005  = 0.010
            mana_level: 8,       // 8 * 0.00125 = 0.010
            resurrect_research: 0.001,
            mystical_max_level: 1, // le plafond : 0.025 + 0.005 = 0.030
            ..UndertakerFacts::default()
        };
        assert!((resurrect_chance(&facts) - 0.021).abs() < 1e-9);
        // Un niveau de mana énorme dépasse le plafond, qui gagne.
        let huge = UndertakerFacts {
            mana_level: 500,
            ..facts
        };
        assert!((resurrect_chance(&huge) - 0.030).abs() < 1e-9);
        // Sans site mystique, le plafond est celui de base.
        let capped = UndertakerFacts {
            mana_level: 500,
            mystical_max_level: 0,
            ..facts
        };
        assert!((resurrect_chance(&capped) - 0.025).abs() < 1e-9);
    }

    #[test]
    fn the_totem_bonus_needs_the_research_and_counts_the_totems() {
        // `EntityAIWorkUndertaker.java:410-432`.
        let none = UndertakerFacts {
            use_totem_research: false,
            totems: 3,
            ..UndertakerFacts::default()
        };
        assert_eq!(totem_bonus(&none), 0.0, "sans la recherche, pas de bonus");
        let one = UndertakerFacts {
            use_totem_research: true,
            totems: 1,
            ..UndertakerFacts::default()
        };
        assert_eq!(totem_bonus(&one), 0.05);
        let two = UndertakerFacts { totems: 2, ..one };
        assert_eq!(totem_bonus(&two), 0.075);
        let research_only = UndertakerFacts { totems: 0, ..one };
        assert_eq!(totem_bonus(&research_only), 0.0, "aucun totem, aucun bonus");
    }

    #[test]
    fn a_remembered_burial_spot_is_kept_only_while_the_block_can_be_replaced() {
        // `EntityAIWorkUndertaker.java:451-454`.
        let remembered = Pos::new(4, 64, 4);
        let search = Pos::new(8, 64, 8);
        let facts = UndertakerFacts {
            burial_search: Some(search),
            burial_replaceable: true,
            ..UndertakerFacts::default()
        };
        assert_eq!(facts.burial_spot(Some(remembered)), Some(remembered));
        let blocked = UndertakerFacts {
            burial_replaceable: false,
            ..facts
        };
        assert_eq!(blocked.burial_spot(Some(remembered)), Some(search));
        let empty = UndertakerFacts {
            burial_search: None,
            ..blocked
        };
        assert_eq!(empty.burial_spot(Some(remembered)), None, "le cimetière est plein");
    }

    #[test]
    fn the_three_efforts_and_the_grave_walk_range_are_java_s_numbers() {
        // `UndertakerConstants.java:61-71` et `EntityAIWorkUndertaker.java:192`.
        assert_eq!(UndertakerFacts::efforts()[0].1, 100);
        assert_eq!(UndertakerFacts::efforts()[1].1, 400);
        assert_eq!(UndertakerFacts::efforts()[2].1, 400);
        assert_eq!(UndertakerFacts::grave_walk_range(), 3);
        assert_eq!(PLAYER_RANGE, 50);
    }
}
