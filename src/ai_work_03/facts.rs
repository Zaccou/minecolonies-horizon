//! Ce que ces trois métiers lisent du monde : les fours, la liste de carburant
//! autorisée, et les onze niveaux de compétence du citoyen. Rien d'autre.
//!
//! Référence Java : `EntityAIWorkFletcher.java`, `EntityAIWorkGlassblower.java` et
//! `EntityAIWorkMechanic.java` n'ajoutent **aucun champ** ; tout ce qu'ils lisent vient
//! de `AbstractEntityAICrafting` (`currentRecipeStorage`, `currentRequest`, le `job` et
//! l'inventaire), et, pour le souffleur de verre seul, de
//! `AbstractEntityAIRequestSmelter` : `building.getModule(FURNACE).getFurnaces()`
//! (`:124`, `:150`, `:511`, `:533`, `:553`, `:583`, `:633`, `:652`, `:678`), les trois
//! slots d'un `FurnaceBlockEntity` (`Constants.java:118`, `:123`, `:128`),
//! `BuildingModules.ITEMLIST_FUEL` (`:698`) et
//! `worker.getCitizenData().getCitizenSkillHandler().getLevel(skill)` (`:122`, `:675`).
//!
//! **L'identité d'un stack n'est pas réinventée ici.** Java compare avec
//! `ItemStackUtils.compareItemStacksIgnoreStackSize`, c'est-à-dire *l'item et ses tags,
//! sans la quantité* ; c'est exactement la clé que `crate::ai_build::materials::NeedKey`
//! nomme déjà pour le bâtisseur (`descriptionId` + hash de tag). « Le four contient
//! notre entrée » est donc une égalité de `NeedKey`, pas une seconde table.
//!
//! **Rien de ce fichier n'appelle le monde.** Un four est une observation : sa
//! position, s'il est chargé, s'il brûle, et le contenu de ses trois slots. L'appelant
//! remplit ; la décision lit.

use core::fmt;

use crate::ai::Pos;
use crate::ai_build::materials::NeedKey;
use crate::ai_build::CraftFacts;
use crate::jobs::Skill;

/// `Constants.java:123` — `SMELTABLE_SLOT = 0`, l'entrée du four.
pub const SMELTABLE_SLOT: usize = 0;
/// `Constants.java:128` — `FUEL_SLOT = 1`, le carburant.
pub const FUEL_SLOT: usize = 1;
/// `Constants.java:118` — `RESULT_SLOT = 2`, la sortie.
pub const RESULT_SLOT: usize = 2;
/// `Constants.java:23` — `STACKSIZE = 64`, la taille d'un transfert et le plafond de
/// la requête de carburant (`AbstractEntityAIRequestSmelter.java:111`, `:211`).
pub const STACKSIZE: u32 = 64;

/// Un stack tel que l'IA le lit : l'identité que Java compare et la quantité, plus le
/// plafond du slot — que `:562` lit vraiment (`getItem(RESULT_SLOT).getMaxStackSize()`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FurnaceStack {
    /// `ItemStackUtils.compareItemStacksIgnoreStackSize` ramené à une clé
    /// (`ai_build/materials.rs:71`).
    pub key: NeedKey,
    /// `stack.getCount()`.
    pub count: u32,
    /// `stack.getMaxStackSize()` (`:562`). Un plafond inconnu se pose à
    /// [`STACKSIZE`], la valeur vanilla par défaut.
    pub max_stack: u32,
}

impl FurnaceStack {
    /// Un stack plein au plafond vanilla.
    pub fn new(key: NeedKey, count: u32) -> FurnaceStack {
        FurnaceStack {
            key,
            count,
            max_stack: STACKSIZE,
        }
    }

    /// Le même stack avec un autre plafond.
    pub fn capped(mut self, max_stack: u32) -> FurnaceStack {
        self.max_stack = max_stack;
        self
    }

    /// `stack.getCount() >= stack.getMaxStackSize()` — le test de `:562`.
    pub fn is_full(&self) -> bool {
        self.count >= self.max_stack
    }
}

/// Un four, réduit à ce que l'IA en lit.
///
/// Référence Java : `FurnaceBlockEntity.getItem(slot)` pour les trois slots,
/// `isLit()` (`:536`, `:556`, `:659`), et `WorldUtil.isBlockLoaded` (`:126`, `:203`,
/// `:635`, `:654`). Le four est un `BlockEntity` : s'il n'y est plus, Java le retire de
/// la liste de la hutte (`:595-599`) — ce n'est pas un four vide, c'est un four absent,
/// et [`SmelterFacts::furnaces`] le distingue par [`Furnace::loaded`] seul.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Furnace {
    /// La position rapportée par `building.getModule(FURNACE).getFurnaces()`.
    pub at: Pos,
    /// `WorldUtil.isBlockLoaded(world, pos)` : un four dans un chunk déchargé n'est ni
    /// lu ni compté (`:126`, `:654`). Les scans qui ne testent pas le chargement sont
    /// ceux de Java aussi (`:511-523`, `:533-541`, `:553-573`, `:583-593`, `:678-685`).
    pub loaded: bool,
    /// `furnace.isLit()`.
    pub lit: bool,
    /// `getItem(SMELTABLE_SLOT)`.
    pub smeltable: Option<FurnaceStack>,
    /// `getItem(FUEL_SLOT)`.
    pub fuel: Option<FurnaceStack>,
    /// `getItem(RESULT_SLOT)`.
    pub result: Option<FurnaceStack>,
}

impl Furnace {
    /// Un four chargé, éteint, aux trois slots vides.
    pub fn empty(at: Pos) -> Furnace {
        Furnace {
            at,
            loaded: true,
            lit: false,
            smeltable: None,
            fuel: None,
            result: None,
        }
    }

    /// Le stack d'un slot, quel qu'il soit.
    pub fn slot(&self, slot: usize) -> Option<&FurnaceStack> {
        match slot {
            SMELTABLE_SLOT => self.smeltable.as_ref(),
            FUEL_SLOT => self.fuel.as_ref(),
            RESULT_SLOT => self.result.as_ref(),
            _ => None,
        }
    }

    /// `ItemStackUtils.isEmpty(getItem(slot))` — un slot hors bornes est vide, pas une
    /// panique.
    pub fn is_empty(&self, slot: usize) -> bool {
        self.slot(slot).is_none()
    }

    /// `getItem(slot).getCount()`, `0` pour un slot vide.
    pub fn count(&self, slot: usize) -> u32 {
        self.slot(slot).map(|stack| stack.count).unwrap_or(0)
    }

    /// `furnace.isLit() && !isEmpty(getItem(SMELTABLE_SLOT))` (`:659`) : le four qui
    /// compte comme « en train de cuire », qui est ce que `countOfBurningFurnaces`
    /// compte et ce qui plafonne le parallélisme.
    pub fn is_burning(&self) -> bool {
        self.lit && !self.is_empty(SMELTABLE_SLOT)
    }

    /// `ItemStackUtils.hasFuelInFurnaceAndNoSmeltable(furnace)`
    /// (`ItemStackUtils.java:743-747`) : le seul état où `fillUpFurnace` transfère
    /// (`:389`).
    pub fn has_fuel_and_no_smeltable(&self) -> bool {
        self.is_empty(SMELTABLE_SLOT) && !self.is_empty(FUEL_SLOT)
    }
}

/// Ce que le souffleur de verre lit en plus du crafter : sa hutte de fours.
///
/// Référence Java : `AbstractEntityAIRequestSmelter` — `getFurnaces()` (`:124`),
/// `ITEMLIST_FUEL` (`:698`, `:710-733`), `currentRecipeStorage.getIntermediate() ==
/// Blocks.FURNACE` (`:147`, `:434`), `getCleanedInput().get(0)` (`:339`, `:590`),
/// `getItemCountInItemHandler(inv, matches(input))` (`:341-342`),
/// `hasItemInItemHandler(inv, isCorrectFuel)` (`:107`, `:181`, `:208`, `:477`),
/// `hasBuildingEnoughElseCount(building, isCorrectFuel, 1)` (`:108`, `:183`, `:477`,
/// qui teste `< 1`, `>= 1` puis `> 1` : d'où un compte et pas un booléen) et
/// `hasWorkerOpenRequestsOfType(..., StackList.class)` (`:109`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SmelterFacts {
    /// `building.getModule(FURNACE).getFurnaces()`, **dans l'ordre de la hutte** : cet
    /// ordre est la règle, chaque scan retient le premier four qui correspond
    /// (`:511-523`, `:533-541`, `:553-573`, `:583-593`).
    pub furnaces: Vec<Furnace>,
    /// `building.getModule(ITEMLIST_FUEL).getList()` (`:698`) : les items que le joueur
    /// a déclarés comme carburant. Vide, le métier refuse par son nom (`:456-463`).
    pub fuel_list: Vec<NeedKey>,
    /// `currentRecipeStorage.getIntermediate() == Blocks.FURNACE` (`:147`, `:434`) :
    /// faux, la recette est une fabrication d'établi et la couche four n'a rien à dire.
    pub intermediate_is_furnace: bool,
    /// `currentRecipeStorage.getCleanedInput().get(0).getItemStack()` (`:339`, `:590`) :
    /// un four ne fond qu'une entrée, la première. `None` quand aucune recette n'est
    /// chargée (`:329`, `:589`).
    pub smeltable: Option<NeedKey>,
    /// `currentRecipeStorage.getPrimaryOutput()` (`:253`, `:345`) : ce que la recette
    /// produit, à comparer avec ce qui sort du four.
    pub output: Option<NeedKey>,
    /// `InventoryUtils.hasItemInItemHandler(worker.getInventoryCitizen(),
    /// isCorrectFuel(possibleFuels))`.
    pub fuel_in_inventory: bool,
    /// `InventoryUtils.hasBuildingEnoughElseCount(building, isCorrectFuel(...), 1)` :
    /// combien la hutte en tient.
    pub fuel_in_building: u32,
    /// `building.hasWorkerOpenRequestsOfType(citizenId, TypeToken.of(StackList.class))`
    /// (`:109`) : une requête de carburant est déjà en vol, on n'en crée pas une
    /// seconde.
    pub open_fuel_request: bool,
    /// `InventoryUtils.getItemCountInItemHandler(inv, matches(inputStack))`
    /// (`:341-342`).
    pub smeltable_in_inventory: u32,
    /// `walkToWorkPos(furnacePos)` (`:198`, `:241`, `:302`, `:367`) : le citoyen est-il
    /// assez près du four que [`Memory::furnace_pos`] désigne ? Une observation de
    /// l'appelant — c'est le seul fait des quatre états qui parle d'un déplacement.
    pub at_furnace: bool,
}

// La moitié « métier » des faits n'a pas de type à elle : c'est `Scene::smelter`, qui
// vaut `Some` pour le seul métier qui a des fours. Ce n'est pas un oubli — les classes
// Java du bûcheron et du mécanicien ne déclarent **aucun** champ (27 et 28 lignes, dont
// la moitié de javadoc) ; une énumération de faits par métier aurait inventé deux
// variantes vides.

/// Les trois métiers de ce lot, et la seule chose que leurs classes Java disent
/// vraiment.
///
/// Référence Java : les trois fichiers mesurés font 27, 28 et 28 lignes, dont
/// l'essentiel est la javadoc, et **le seul code** est un `@Override` de
/// `getExpectedBuildingClass()`. Tout le reste est dans les parents :
/// `AbstractEntityAICrafting` pour le bûcheron et le mécanicien,
/// `AbstractEntityAIRequestSmelter` (donc `AbstractEntityAICrafting` plus la couche
/// four) pour le souffleur de verre.
///
/// **Où est déclaré ce que ce tableau ajoute.** Les deux compétences du module
/// (`primary`, `secondary`) existent déjà dans `crate::jobs::employment` et ne sont
/// **pas** recopiées ici : `Trade::primary_skill` les relit par `crate::jobs::employ`.
/// Ce qui n'existait nulle part, et que ce module porte, est dans le constructeur du
/// module de travail : `craftingSpeedSkill` et `recipeImprovementSkill`
/// (`CraftingWorkerBuildingModule.java:18-19`, `:21-32`, `:49-61`), que
/// `BuildingModules.java` passe en sixième et septième argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: the three trades of this slice
pub enum Trade {
    /// `EntityAIWorkFletcher` (`core/entity/ai/workers/crafting/EntityAIWorkFletcher.java:11`).
    Fletcher,
    /// `EntityAIWorkGlassblower` (`.../EntityAIWorkGlassblower.java:11`).
    Glassblower,
    /// `EntityAIWorkMechanic` (`.../EntityAIWorkMechanic.java:11`).
    Mechanic,
}

impl Trade {
    /// Les trois, dans l'ordre du brief.
    pub const ALL: [Trade; 3] = [Trade::Fletcher, Trade::Glassblower, Trade::Mechanic];

    /// Le nom Java de la classe, pour une ligne de log.
    pub fn class(self) -> &'static str {
        match self {
            Trade::Fletcher => "EntityAIWorkFletcher",
            Trade::Glassblower => "EntityAIWorkGlassblower",
            Trade::Mechanic => "EntityAIWorkMechanic",
        }
    }

    /// La hutte que `getExpectedBuildingClass()` rend — le seul code des trois
    /// fichiers, avec sa ligne.
    pub fn building(self) -> &'static str {
        match self {
            Trade::Fletcher => "BuildingFletcher",
            Trade::Glassblower => "BuildingGlassblower",
            Trade::Mechanic => "BuildingMechanic",
        }
    }

    /// La ligne du `@Override` qui nomme cette hutte.
    pub fn expected_building_java(self) -> &'static str {
        match self {
            Trade::Fletcher => "EntityAIWorkFletcher.java:21-25",
            Trade::Glassblower => "EntityAIWorkGlassblower.java:24-28",
            Trade::Mechanic => "EntityAIWorkMechanic.java:24-28",
        }
    }

    /// La classe parente : ce qui décide vraiment.
    pub fn parent(self) -> &'static str {
        match self {
            Trade::Fletcher | Trade::Mechanic => "AbstractEntityAICrafting",
            Trade::Glassblower => "AbstractEntityAIRequestSmelter",
        }
    }

    /// Vrai pour le seul des trois qui enregistre les sept cibles du four
    /// (`AbstractEntityAIRequestSmelter.java:71-82`).
    pub fn uses_furnaces(self) -> bool {
        self == Trade::Glassblower
    }

    /// La compétence de vitesse de fabrication :
    /// `CraftingWorkerBuildingModule.getCraftSpeedSkill()` (`:49-52`), lue par
    /// `AbstractEntityAICrafting.java:735`.
    ///
    /// **Ce n'est pas toujours la primaire du métier.** Le bûcheron fabrique avec sa
    /// `Creativity` alors que sa primaire est `Dexterity` ; le souffleur de verre avec
    /// sa `Focus` alors que sa primaire est `Creativity`. Seul le mécanicien retombe sur
    /// sa primaire, parce que son module utilise le constructeur à cinq arguments, qui
    /// pose `craftingSpeedSkill = primary` (`CraftingWorkerBuildingModule.java:34-43`).
    pub fn craft_speed_skill(self) -> Skill {
        match self {
            // `BuildingModules.java:302-309` — `Skill.Creativity` en sixième position.
            Trade::Fletcher => Skill::Creativity,
            // `BuildingModules.java:316-323` — `Skill.Focus` en sixième position.
            Trade::Glassblower => Skill::Focus,
            // `BuildingModules.java:332-334` — constructeur à cinq arguments.
            Trade::Mechanic => Skill::Knowledge,
        }
    }

    /// La ligne qui pose cette compétence.
    pub fn craft_speed_java(self) -> &'static str {
        match self {
            Trade::Fletcher => "BuildingModules.java:306",
            Trade::Glassblower => "BuildingModules.java:320",
            Trade::Mechanic => "BuildingModules.java:333",
        }
    }

    /// La compétence d'amélioration de recette :
    /// `CraftingWorkerBuildingModule.getRecipeImprovementSkill()` (`:54-61`), lue par
    /// `ICraftingBuildingModule.improveRecipe` au moment de finaliser
    /// (`AbstractEntityAICrafting.java:612-616`).
    pub fn recipe_improvement_skill(self) -> Skill {
        match self {
            Trade::Fletcher => Skill::Dexterity,    // `BuildingModules.java:307`
            Trade::Glassblower => Skill::Creativity, // `BuildingModules.java:321`
            Trade::Mechanic => Skill::Agility,       // `BuildingModules.java:334`
        }
    }

    /// La hutte, dans l'énumération que `crate::jobs` connaît déjà.
    pub fn hut(self) -> crate::building::Kind {
        match self {
            Trade::Fletcher => crate::building::Kind::Fletcher,
            Trade::Glassblower => crate::building::Kind::Glassblower,
            Trade::Mechanic => crate::building::Kind::Mechanic,
        }
    }

    /// Le métier, dans l'énumération que `crate::jobs` connaît déjà.
    pub fn job(self) -> crate::jobs::Job {
        match self {
            Trade::Fletcher => crate::jobs::Job::Fletcher,
            Trade::Glassblower => crate::jobs::Job::Glassblower,
            Trade::Mechanic => crate::jobs::Job::Mechanic,
        }
    }

    /// La compétence primaire du module de travail — **relue**, pas recopiée, dans
    /// `crate::jobs::employment` (`WorkerBuildingModule`'s `primary`).
    ///
    /// `None` si la hutte n'emploie pas ce métier dans cette table, ce qui serait une
    /// incohérence des deux modules et non un cas de jeu.
    pub fn primary_skill(self) -> Option<Skill> {
        crate::jobs::employ(self.hut(), self.job())
            .ok()
            .and_then(|employment| employment.skills())
            .map(|(primary, _)| primary)
    }

    /// La compétence secondaire, même source.
    ///
    /// C'est elle que `accelerateFurnaces` lit (`AbstractEntityAIRequestSmelter.java:
    /// 122`) — et donc `Focus` pour le souffleur de verre, pas sa primaire.
    pub fn secondary_skill(self) -> Option<Skill> {
        crate::jobs::employ(self.hut(), self.job())
            .ok()
            .and_then(|employment| employment.skills())
            .map(|(_, secondary)| secondary)
    }
}

impl fmt::Display for Trade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.class())
    }
}

/// Les onze niveaux de compétence d'un citoyen, tels que
/// `CitizenSkillHandler.getLevel(Skill)` les rend.
///
/// Référence Java : `api/entity/citizen/Skill.java:8-163` pour les onze compétences et
/// leur ordre de déclaration, `AbstractEntityAICrafting.java:735` et
/// `AbstractEntityAIRequestSmelter.java:122`, `:675` pour les trois lectures qui
/// comptent ici. Le port garde les onze : la compétence qui décide n'est **pas** la
/// même d'un métier à l'autre (voir `Trade::craft_speed_skill`), c'est tout l'intérêt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Skills {
    levels: [u8; 11],
}

impl Skills {
    /// Onze niveaux à zéro.
    pub fn new() -> Skills {
        Skills { levels: [0; 11] }
    }

    /// La position d'une compétence dans `Skill.ALL`, c'est-à-dire son indice dans le
    /// tableau ci-dessus.
    fn slot(skill: Skill) -> usize {
        Skill::ALL
            .iter()
            .position(|known| *known == skill)
            .unwrap_or(0)
    }

    /// `getLevel(skill)`.
    pub fn level(&self, skill: Skill) -> u8 {
        self.levels[Skills::slot(skill)]
    }

    /// Poser un niveau — l'appelant lit le citoyen, le port ne le fait pas.
    pub fn set(&mut self, skill: Skill, level: u8) {
        let slot = Skills::slot(skill);
        self.levels[slot] = level;
    }

    /// Le même tableau avec `skill` à `level`, pour écrire un cas de test en une ligne.
    pub fn with(mut self, skill: Skill, level: u8) -> Skills {
        self.set(skill, level);
        self
    }
}

impl Default for Skills {
    fn default() -> Skills {
        Skills::new()
    }
}

/// La mémoire du métier : les champs de Java qui ne sont **pas** des faits du monde.
///
/// Référence Java : `furnacePos` (`AbstractEntityAIRequestSmelter.java:61`), `delay` et
/// `hasDelayed` (`AbstractEntityAIBasic.java:121`, `:126`) et
/// `job.getSecondaryOutputs()` vu comme un simple drapeau « il y en a »
/// (`AbstractEntityAIRequestSmelter.java:271`, `:314`, lu par
/// `AbstractEntityAICrafting.afterDump` `:691`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Memory {
    /// `delay` (`AbstractEntityAIBasic.java:121`) : les ticks restants avant la
    /// prochaine action. Écrit par `setDelay` (`:446`, `:489`).
    pub delay: i32,
    /// `hasDelayed` (`:126`).
    pub has_delayed: bool,
    /// `furnacePos` (`:61`) : le four choisi par un scan au tick précédent, que
    /// l'action du tick suivant consomme. `None` = aucun four en cours.
    pub furnace_pos: Option<Pos>,
    /// Combien de stacks la tâche a mis en sortie secondaire (`:271-272`, `:314`).
    pub secondary_outputs: u32,
}

impl Default for Memory {
    /// Aucun délai, aucun four choisi, aucune sortie secondaire.
    fn default() -> Memory {
        Memory {
            delay: 0,
            has_delayed: false,
            furnace_pos: None,
            secondary_outputs: 0,
        }
    }
}

/// La scène entière d'une décision : le métier, la moitié crafter, la moitié four, les
/// compétences, la mémoire, et ce que le tick a refusé ou demandé.
///
/// **Pourquoi deux moitiés.** `AbstractEntityAICrafting` est déjà porté
/// (`crate::ai_build::crafter`) et ses six cibles lisent [`CraftFacts`] ; ce module
/// ajoute la couche four et la table des compétences. Mettre `CraftFacts` en champ
/// plutôt qu'en copie est ce qui permet aux deux de partager la même règle : les
/// fonctions de `crafter` sont appelées, pas recopiées.
///
/// **Pourquoi `smelter: Option<…>` et non un variant par métier.** Une seule des trois
/// classes a des fours ; le champ dit lequel, et [`Trade::uses_furnaces`] dit lequel
/// doit en avoir. Une scène qui se contredit est refusée par son nom
/// ([`TradeRefusal::NoFactsForTrade`](super::state::TradeRefusal::NoFactsForTrade)),
/// jamais complétée par défaut.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scene {
    /// Le métier : ce que la classe Java dit, et rien de plus.
    pub trade: Trade,
    /// La moitié `AbstractEntityAICrafting`.
    pub base: CraftFacts,
    /// La hutte de fours — `Some` pour le souffleur de verre, `None` pour les deux
    /// autres.
    pub smelter: Option<SmelterFacts>,
    /// Les onze niveaux du citoyen : c'est d'ici que vient `base.craft_speed_level`,
    /// via la compétence que le métier déclare (`AbstractEntityAICrafting.java:735`).
    pub skills: Skills,
    /// `getExpectedBuildingClass().isInstance(citizenData.getWorkBuilding())`
    /// (`AbstractEntityAIBasic.java:191`) : le citoyen travaille-t-il dans une hutte de
    /// son métier ? Faux, le constructeur refuse d'avancer (`:191-198`) — et les trois
    /// classes de ce lot sont **exactement** ce test, seul code qu'elles écrivent.
    pub hut_right_class: bool,
    /// Ce que l'IA se rappelle entre deux ticks.
    pub memory: Memory,
    /// Le refus du tick en cours, s'il y en a un.
    pub refusal: Option<super::state::TradeRefusal>,
    /// Ce que le tick demande au monde, s'il demande quelque chose.
    pub action: Option<super::state::TradeAction>,
}

impl Scene {
    /// Une scène pour `trade`, tout le reste au défaut : pas de tâche, pas de recette,
    /// pas de four, onze compétences à zéro.
    ///
    /// Le souffleur de verre reçoit une hutte de fours vide, les deux autres `None` :
    /// c'est [`Trade::uses_furnaces`] qui décide, pas l'appelant.
    pub fn new(trade: Trade) -> Scene {
        Scene {
            trade,
            base: CraftFacts::default(),
            smelter: if trade.uses_furnaces() {
                Some(SmelterFacts::default())
            } else {
                None
            },
            skills: Skills::new(),
            hut_right_class: true,
            memory: Memory::default(),
            refusal: None,
            action: None,
        }
    }

    /// `getLevel(skill)` du citoyen (`CitizenSkillHandler`, lu à
    /// `AbstractEntityAICrafting.java:735` et `AbstractEntityAIRequestSmelter.java:122`,
    /// `:675`).
    pub fn level(&self, skill: Skill) -> u8 {
        self.skills.level(skill)
    }

    /// Le niveau de la compétence de vitesse de fabrication du métier
    /// (`Trade::craft_speed_skill`) — la valeur que `getRequiredProgressForMakingRawMaterial`
    /// divise (`AbstractEntityAICrafting.java:735`).
    pub fn craft_speed_level(&self) -> u8 {
        self.level(self.trade.craft_speed_skill())
    }

    /// `walkToWorkPos(furnacePos)` : le citoyen peut-il atteindre le four que la
    /// mémoire désigne ? C'est une observation de l'appelant, pas un déplacement.
    pub fn at_chosen_furnace(&self) -> bool {
        self.smelter
            .as_ref()
            .map(|facts| facts.at_furnace)
            .unwrap_or(false)
    }

    /// Noter pourquoi ce tick ne va pas plus loin. Le dernier gagne : Java n'a qu'une
    /// branche en vol.
    pub fn refuse(&mut self, refusal: super::state::TradeRefusal) {
        self.refusal = Some(refusal);
    }

    /// Noter ce que le tick demande au monde.
    pub fn ask(&mut self, action: super::state::TradeAction) {
        self.action = Some(action);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_furnace_slots_are_the_vanilla_ones() {
        // `Constants.java:123`, `:128`, `:118`.
        assert_eq!(SMELTABLE_SLOT, 0);
        assert_eq!(FUEL_SLOT, 1);
        assert_eq!(RESULT_SLOT, 2);
        assert_eq!(STACKSIZE, 64, "Constants.java:23");
        // Un slot hors bornes est vide, pas une panique.
        let furnace = Furnace::empty(Pos::new(0, 64, 0));
        assert!(furnace.is_empty(7));
        assert_eq!(furnace.count(7), 0);
    }

    #[test]
    fn a_furnace_is_burning_only_when_lit_and_holding_a_smeltable() {
        // `:659` — `furnace.isLit() && !isEmpty(getItem(SMELTABLE_SLOT))`.
        let mut furnace = Furnace::empty(Pos::new(1, 2, 3));
        assert!(!furnace.is_burning());
        furnace.lit = true;
        assert!(!furnace.is_burning(), "lit but empty is not burning");
        furnace.smeltable = Some(FurnaceStack::new(NeedKey::plain("minecraft:sand"), 3));
        assert!(furnace.is_burning());
        furnace.lit = false;
        assert!(!furnace.is_burning(), "an unlit furnace is not burning");
    }

    #[test]
    fn fuel_and_no_smeltable_is_the_fill_target() {
        // `ItemStackUtils.java:743-747`.
        let mut furnace = Furnace::empty(Pos::new(0, 0, 0));
        assert!(!furnace.has_fuel_and_no_smeltable(), "nothing at all");
        furnace.fuel = Some(FurnaceStack::new(NeedKey::plain("minecraft:coal"), 8));
        assert!(furnace.has_fuel_and_no_smeltable());
        furnace.smeltable = Some(FurnaceStack::new(NeedKey::plain("minecraft:sand"), 1));
        assert!(!furnace.has_fuel_and_no_smeltable(), "a smeltable is in the way");
    }

    #[test]
    fn a_result_stack_knows_when_it_is_full() {
        // `:562` reads the slot's own maximum, not a constant.
        let stack = FurnaceStack::new(NeedKey::plain("minecraft:glass"), 63);
        assert!(!stack.is_full());
        assert!(stack.capped(63).is_full());
        assert!(FurnaceStack::new(NeedKey::plain("minecraft:glass"), 64).is_full());
    }

    #[test]
    fn the_eleven_skills_are_addressed_by_their_java_identity() {
        // `Skill.java:8-163` : onze compétences, et `getLevel` les distingue.
        let mut skills = Skills::new();
        assert_eq!(skills.level(Skill::Creativity), 0);
        skills.set(Skill::Creativity, 42);
        assert_eq!(skills.level(Skill::Creativity), 42);
        assert_eq!(skills.level(Skill::Dexterity), 0, "one skill is not another");
        // Les onze tiennent, et `with` est le même écrivain.
        for (index, skill) in Skill::ALL.iter().enumerate() {
            let skills = Skills::new().with(*skill, index as u8);
            assert_eq!(skills.level(*skill), index as u8);
        }
    }

    #[test]
    fn a_scene_starts_empty_and_carries_the_right_trade_half() {
        let glass = Scene::new(Trade::Glassblower);
        assert!(glass.smelter.is_some(), "a glassblower has furnaces");
        assert!(glass.hut_right_class, "`AbstractEntityAIBasic.java:191`");
        assert_eq!(glass.memory, Memory::default());
        assert!(!glass.at_chosen_furnace(), "no furnace chosen yet");

        let fletcher = Scene::new(Trade::Fletcher);
        assert!(fletcher.smelter.is_none(), "a fletcher has no furnace");
        assert_eq!(fletcher.trade, Trade::Fletcher);
        // The two trades without furnaces are the two `AbstractEntityAICrafting`s.
        for trade in Trade::ALL {
            let scene = Scene::new(trade);
            assert_eq!(trade.uses_furnaces(), scene.smelter.is_some(), "{trade}");
        }
    }

    #[test]
    fn a_scene_records_a_refusal_and_an_action_separately() {
        use super::super::state::{TradeAction, TradeRefusal};
        let mut scene = Scene::new(Trade::Glassblower);
        assert_eq!(scene.refusal, None);
        assert_eq!(scene.action, None);
        scene.refuse(TradeRefusal::NoFurnaces);
        scene.ask(TradeAction::HitFurnace(Pos::new(1, 2, 3)));
        assert_eq!(scene.refusal, Some(TradeRefusal::NoFurnaces));
        assert_eq!(scene.action, Some(TradeAction::HitFurnace(Pos::new(1, 2, 3))));
    }
}
