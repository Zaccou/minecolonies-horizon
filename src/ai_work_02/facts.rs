//! Ce que ces trois métiers voient du monde : les fours du cuisinier et du
//! teinturier, le quota quotidien du concasseur, et rien d'autre.
//!
//! Référence Java : `AbstractEntityAIRequestSmelter.java:508-602` (les quatre
//! sélecteurs de four et les deux compteurs), `:144-172` (`getExtendedCount`, ce que
//! les fours contiennent déjà), `:695-729` (les deux listes de carburant) ;
//! `EntityAIWorkCrusher.java:68`, `:93`, `:115`, `:157` (le quota du jour) et
//! `BuildingCrusher.java:88-96` (le maximum du bâtiment).
//!
//! **Chaque champ nomme un appel Java.** Un four est une lecture de
//! `FurnaceBlockEntity.getItem(slot)` et de `isLit()` ; un quota est
//! `building.getSetting(DAILY_LIMIT).getValue()`. Ce que ce fichier ajoute, ce sont
//! les *questions* que les règles posent à ces lectures — `result_ready`,
//! `fuel_without_smeltable`, `without_fuel` — écrites une fois ici plutôt que
//! recopiées dans chaque règle.
//!
//! **L'identité d'un stack n'est pas réinventée** : c'est [`NeedKey`], la clé que la
//! résolution de matériaux du bâtisseur utilise déjà, donc « le four contient la
//! sortie de la recette » est une égalité et non une seconde table de correspondance
//! (`ItemStackUtils.compareItemStacksIgnoreStackSize`).
//!
//! **Le tirage aléatoire est un fait.** `EntityAIWorkCrusher.java:209` tire
//! `worker.getRandom().nextInt(...)` pour choisir l'item tenu en main ; c'est un
//! affichage, décrit ici par un index fourni par l'appelant, jamais tiré ici.

use crate::ai::Pos;
use crate::ai_build::materials::NeedKey;

/// `Constants.java:21` — `TICKS_SECOND`.
pub const TICKS_SECOND: u32 = 20;
/// `Constants.java:23` — `STACKSIZE`, la taille des demandes de carburant et de
/// minerai.
pub const STACKSIZE: u32 = 64;
/// `Constants.java:123` — `SMELTABLE_SLOT`, l'entrée du four.
pub const SMELTABLE_SLOT: usize = 0;
/// `Constants.java:128` — `FUEL_SLOT`, le carburant du four.
pub const FUEL_SLOT: usize = 1;
/// `Constants.java:118` — `RESULT_SLOT`, la sortie du four.
pub const RESULT_SLOT: usize = 2;

/// Les trois compétences qu'un atelier d'artisan lit.
///
/// Référence Java : `CraftingWorkerBuildingModule.getCraftSpeedSkill()`
/// (`CraftingWorkerBuildingModule.java:59-62`) et les deux compétences du module de
/// travail. Le module de chaque métier les nomme :
///
/// | Métier | `primary` | `secondary` | `craft_speed` | Source |
/// | --- | --- | --- | --- | --- |
/// | cuisinier | `Creativity` | `Knowledge` | `Knowledge` | `BuildingModules.java:451-457` |
/// | teinturier | `Creativity` | `Dexterity` | `Dexterity` | `:290-295` |
/// | concasseur | `Stamina` | `Strength` | `Stamina` (primaire, constructeur à cinq arguments) | `:280-281` |
///
/// Les trois ne servent pas la même chose : `primary` plafonne le nombre de fours
/// utilisables (`AbstractEntityAIRequestSmelter.java:675`), `secondary` donne la
/// vitesse d'accélération des fours (`:122`) et le seuil de progression du concasseur
/// (`EntityAIWorkCrusher.java:127`), `craft_speed` donne le nombre de coups d'une
/// recette (`AbstractEntityAICrafting.java:736`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Skills {
    /// `getModuleForJob().getPrimarySkill()`.
    pub primary: i32,
    /// `getModuleForJob().getSecondarySkill()`.
    pub secondary: i32,
    /// `((CraftingWorkerBuildingModule) getModuleForJob()).getCraftSpeedSkill()`.
    pub craft_speed: i32,
}

impl Skills {
    /// Les trois niveaux.
    pub fn new(primary: i32, secondary: i32, craft_speed: i32) -> Skills {
        Skills {
            primary,
            secondary,
            craft_speed,
        }
    }
}

/// Un stack posé dans un slot de four : son identité, son compte, et le maximum que
/// ce stack accepte.
///
/// Référence Java : `furnace.getItem(slot)` (`AbstractEntityAIRequestSmelter.java:155`,
/// `:208`, `:235`, `:294`, `:516-518`, `:556-570`), `stack.getCount()` et
/// `stack.getMaxStackSize()` (`:562`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    /// `ItemStack` — l'identité, sans le compte.
    pub key: NeedKey,
    /// `stack.getCount()`.
    pub count: u32,
    /// `stack.getMaxStackSize()` (`:562`).
    pub max_stack: u32,
}

impl Slot {
    /// Un stack plein à 64, la taille de pile par défaut de vanilla.
    pub fn new(key: NeedKey, count: u32) -> Slot {
        Slot {
            key,
            count,
            max_stack: STACKSIZE,
        }
    }

    /// Un stack dont le maximum de pile n'est pas 64 (un seau, une potion…).
    pub fn with_max_stack(mut self, max_stack: u32) -> Slot {
        self.max_stack = max_stack;
        self
    }

    /// `ItemStackUtils.isEmpty(stack)` — un slot vide, ou un compte nul.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// `ItemStackUtils.compareItemStacksIgnoreStackSize(a, b)` : deux stacks de même
    /// identité, quel que soit leur compte.
    pub fn is(&self, key: &NeedKey) -> bool {
        &self.key == key
    }
}

/// Un four du module `FURNACE`, tel que l'IA le lit.
///
/// Référence Java : `world.getBlockEntity(pos)` puis
/// `entity instanceof FurnaceBlockEntity furnace` (`AbstractEntityAIRequestSmelter.java:
/// 128-141`, `:152-165`, `:205-215`, `:513-521`, `:533-541`, `:553-575`, `:584-601`,
/// `:653-666`, `:679-685`). Le port ne peut pas tester un `instanceof` : le fait
/// [`Furnace::is_furnace`] porte la réponse, et c'est ce qui permet d'écrire la
/// suppression du module (`:595-599`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Furnace {
    /// `entity.getBlockPos()` — la position que `FurnaceUserModule.getFurnaces()`
    /// rend.
    pub at: Pos,
    /// `entity instanceof FurnaceBlockEntity`. Faux, le four n'est plus un four : le
    /// module le retire (`:595-599`).
    pub is_furnace: bool,
    /// `WorldUtil.isBlockLoaded(world, pos)` (`:126`, `:203`, `:654`).
    pub loaded: bool,
    /// `furnace.isLit()` (`:133`, `:536`, `:556`, `:659`).
    pub lit: bool,
    /// `furnace.getItem(SMELTABLE_SLOT)`, `None` quand le slot est vide.
    pub smeltable: Option<Slot>,
    /// `furnace.getItem(FUEL_SLOT)`.
    pub fuel: Option<Slot>,
    /// `furnace.getItem(RESULT_SLOT)`.
    pub result: Option<Slot>,
}

impl Furnace {
    /// Un four vide, allumé nulle part, chargé.
    pub fn new(at: Pos) -> Furnace {
        Furnace {
            at,
            is_furnace: true,
            loaded: true,
            lit: false,
            smeltable: None,
            fuel: None,
            result: None,
        }
    }

    /// L'entrée du four, la seule chose qu'il sait fondre.
    pub fn with_smeltable(mut self, slot: Slot) -> Furnace {
        self.smeltable = Some(slot);
        self
    }

    /// Le carburant du four.
    pub fn with_fuel(mut self, slot: Slot) -> Furnace {
        self.fuel = Some(slot);
        self
    }

    /// La sortie du four.
    pub fn with_result(mut self, slot: Slot) -> Furnace {
        self.result = Some(slot);
        self
    }

    /// Le four tourne.
    pub fn burning(mut self) -> Furnace {
        self.lit = true;
        self
    }

    /// Un bloc qui n'est plus un four (`:595-599`).
    pub fn not_a_furnace(mut self) -> Furnace {
        self.is_furnace = false;
        self
    }

    /// Le four est dans un chunk déchargé (`:126`, `:635`).
    pub fn unloaded(mut self) -> Furnace {
        self.loaded = false;
        self
    }

    /// `!furnace.getItem(FUEL_SLOT).isEmpty()` (`:518`, `:681`).
    pub fn has_fuel(&self) -> bool {
        self.fuel.as_ref().is_some_and(|slot| !slot.is_empty())
    }

    /// `hasFuelInFurnaceAndNoSmeltable(furnace)` (`ItemStackUtils.java:743-747`) :
    /// le carburant est là, rien à fondre.
    pub fn fuel_without_smeltable(&self) -> bool {
        self.smeltable.as_ref().is_none_or(Slot::is_empty) && self.has_fuel()
    }

    /// `!furnace.isLit() && furnace.getItem(FUEL_SLOT).isEmpty()`
    /// (`:536`) : ce four attend du carburant.
    pub fn without_fuel(&self) -> bool {
        !self.lit && !self.has_fuel()
    }

    /// `furnace.isLit() && !furnace.getItem(SMELTABLE_SLOT).isEmpty()` (`:659`) :
    /// ce four compte parmi ceux qui tournent.
    pub fn is_burning(&self) -> bool {
        self.lit && self.smeltable.as_ref().is_some_and(|slot| !slot.is_empty())
    }

    /// `getEmptyFurnaceWithFuel` (`:516-518`) : rien à fondre, rien à sortir, du
    /// carburant. C'est le four qu'on remplit.
    pub fn fuel_without_smeltable_and_result(&self) -> bool {
        !self.smeltable.as_ref().is_some_and(|slot| !slot.is_empty())
            && !self.result.as_ref().is_some_and(|slot| !slot.is_empty())
            && self.has_fuel()
    }

    /// `getFurnaceToRetrieveOutputFrom` (`:556-572`) : le four s'est arrêté ou n'a
    /// plus rien à fondre, et son slot de sortie n'est pas vide.
    pub fn result_ready(&self) -> bool {
        let stopped = !self.lit || !self.smeltable.as_ref().is_some_and(|slot| !slot.is_empty());
        stopped && self.result.as_ref().is_some_and(|slot| !slot.is_empty())
    }

    /// `getFurnaceToRetrieveUnrelatedInputFrom` (`:588-590`) : le four fond autre
    /// chose que l'entrée de la recette en cours.
    ///
    /// Java teste `currentRecipeStorage == null || !compare(...)`, donc une recette
    /// absente fait de **tout** contenu un intrus : c'est exactement ce que le port
    /// écrit, sans raccourci.
    pub fn holds_unrelated_input(&self, input: Option<&NeedKey>) -> bool {
        let Some(slot) = &self.smeltable else {
            return false;
        };
        if slot.is_empty() {
            return false;
        }
        match input {
            None => true,
            Some(key) => !slot.is(key),
        }
    }
}

/// La recette en cours — `currentRecipeStorage`.
///
/// Référence Java : `AbstractEntityAICrafting.java:98` (le champ),
/// `getCleanedInput().get(0)` (`AbstractEntityAIRequestSmelter.java:296`, `:339`,
/// `:590`), `getPrimaryOutput()` (`:253`, `:345`), `getIntermediate() ==
/// Blocks.FURNACE` (`:147`, `:434`), `getCraftingToolsAndSecondaryOutputs()` (`:383`)
/// et `getRequiredTool() != ModEquipmentTypes.none.get()` (`AbstractEntityAICrafting.java:
/// 355`).
///
/// C'est une **mémoire**, pas un fait : Java la garde dans un champ privé de l'IA, et
/// elle survit d'un tick à l'autre. Les comptes, eux, sont des faits (l'inventaire,
/// les fours) et vivent dans [`crate::ai_build::crafter::CraftFacts`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recipe {
    /// `getCleanedInput().get(0).getItemStack()` — il n'y a jamais qu'une entrée pour
    /// un four (`AbstractEntityAIRequestSmelter.java:725` le dit explicitement).
    pub input: NeedKey,
    /// `getPrimaryOutput()`.
    pub output: NeedKey,
    /// `getPrimaryOutput().getCount()` — ce qu'une passe produit.
    pub output_count: u32,
    /// `getIntermediate() == Blocks.FURNACE` : la recette passe par les fours du
    /// bâtiment (`:147`, `:434`).
    pub furnace_intermediate: bool,
    /// L'entrée est aussi un outil ou un conteneur
    /// (`getCraftingToolsAndSecondaryOutputs`, `:383`) : elle ne se multiplie pas par
    /// le nombre de passes.
    pub tool_is_input: bool,
}

impl Recipe {
    /// Une recette qui passe par un four.
    pub fn smelted(input: NeedKey, output: NeedKey) -> Recipe {
        Recipe {
            input,
            output,
            output_count: 1,
            furnace_intermediate: true,
            tool_is_input: false,
        }
    }

    /// Une recette d'établi, sans four (`AbstractEntityAIRequestSmelter.java:434-437`
    /// renvoie alors à la classe mère).
    pub fn crafted(input: NeedKey, output: NeedKey) -> Recipe {
        Recipe {
            furnace_intermediate: false,
            ..Recipe::smelted(input, output)
        }
    }

    /// Le nombre d'items produits par une passe.
    pub fn with_output_count(mut self, count: u32) -> Recipe {
        self.output_count = count;
        self
    }

    /// L'entrée est un outil ou un conteneur.
    pub fn with_tool_input(mut self) -> Recipe {
        self.tool_is_input = true;
        self
    }
}

/// Ce que le cuisinier et le teinturier lisent de leurs fours.
///
/// Référence Java : `building.getModule(FURNACE)` (`:106`, `:124`, `:150`, `:439`,
/// `:510`, `:533`, `:553`, `:583`, `:633`, `:652`, `:678`), `getModuleForJob()
/// .getPrimarySkill()` / `getSecondarySkill()` (`:675`, `:122`), et les deux listes de
/// carburant (`:695-729`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SmelterFacts {
    /// `FurnaceUserModule.getFurnaces()`, dans l'ordre du module : les sélecteurs
    /// Java rendent le **premier** four qui répond (`:511`, `:533`, `:553`, `:583`).
    pub furnaces: Vec<Furnace>,
    /// `getAllowedFuel()` (`:695-705`) : `ITEMLIST_FUEL` du bâtiment, en identités.
    pub allowed_fuel: Vec<NeedKey>,
    /// `InventoryUtils.hasItemInItemHandler(worker.getInventoryCitizen(),
    /// isCorrectFuel(...))` (`:107`, `:181`, `:208`, `:477`).
    pub fuel_in_inventory: u32,
    /// `InventoryUtils.hasBuildingEnoughElseCount(building, isCorrectFuel(...), 1)`
    /// (`:108`, `:183`, `:477`).
    pub fuel_in_building: u32,
    /// `building.hasWorkerOpenRequestsOfType(id, TypeToken.of(StackList.class))`
    /// (`:109`) : une demande de carburant est déjà ouverte.
    pub open_fuel_request: bool,
    /// `walkToWorkPos(furnacePos)` (`:198`, `:241`, `:302`, `:367`) : le citoyen est
    /// arrivé au four. C'est une réponse d'hôte, donc un fait.
    pub walked_to_furnace: bool,
    /// `getModuleForJob().getPrimarySkill()` et `getSecondarySkill()`, et la
    /// compétence de vitesse de la recette.
    pub skills: Skills,
    /// `worker.getItemInHand(MAIN_HAND).isEmpty()` (`:384`).
    pub empty_main_hand: bool,
}

impl SmelterFacts {
    /// Les fours que le module déclare, chargés ou non.
    pub fn furnace_count(&self) -> usize {
        self.furnaces.len()
    }

    /// `areFurnacesLoaded` (`:631-641`) : **tous** les fours sont chargés.
    pub fn all_loaded(&self) -> bool {
        self.furnaces.iter().all(|furnace| furnace.loaded)
    }

    /// Le premier four qui n'est plus un four, pour le retirer du module
    /// (`:595-599`).
    pub fn stale(&self) -> Option<Pos> {
        self.furnaces
            .iter()
            .find(|furnace| !furnace.is_furnace)
            .map(|furnace| furnace.at)
    }

    /// Le premier four dont la sortie est prête (`:551-575`).
    pub fn output_ready(&self) -> Option<Pos> {
        self.furnaces
            .iter()
            .find(|furnace| furnace.is_furnace && furnace.result_ready())
            .map(|furnace| furnace.at)
    }

    /// Le premier four qui fond un intrus (`:581-602`).
    pub fn unrelated_input(&self, input: Option<&NeedKey>) -> Option<Pos> {
        self.furnaces
            .iter()
            .find(|furnace| furnace.is_furnace && furnace.holds_unrelated_input(input))
            .map(|furnace| furnace.at)
    }

    /// Le premier four sans carburant (`:531-542`).
    pub fn without_fuel(&self) -> Option<Pos> {
        self.furnaces
            .iter()
            .find(|furnace| furnace.is_furnace && furnace.without_fuel())
            .map(|furnace| furnace.at)
    }

    /// Le premier four qui a du carburant et rien à fondre (`:508-525`).
    pub fn fuel_without_smeltable(&self) -> Option<Pos> {
        self.furnaces
            .iter()
            .find(|furnace| furnace.is_furnace && furnace.fuel_without_smeltable_and_result())
            .map(|furnace| furnace.at)
    }

    /// Un four par sa position, tel que la règle le relit (`:234`, `:292`, `:380`).
    pub fn at(&self, pos: Pos) -> Option<&Furnace> {
        self.furnaces
            .iter()
            .find(|furnace| furnace.at == pos && furnace.is_furnace)
    }

    /// `countOfBurningFurnaces` (`:648-667`) : un four chargé, allumé, avec quelque
    /// chose à fondre.
    pub fn burning(&self) -> u32 {
        self.furnaces
            .iter()
            .filter(|furnace| furnace.loaded && furnace.is_furnace && furnace.is_burning())
            .count() as u32
    }

    /// `getMaxUsableFurnaces` (`:673-688`) : `primarySkill / 10 + 1`, plafonné par le
    /// nombre de fours qui ont du carburant.
    ///
    /// Un niveau négatif ne peut pas produire un plafond négatif : Java divise en
    /// `int` et obtient `0` pour les niveaux `-9..0`, un nombre négatif en dessous.
    /// Le port garde le plancher de Java (`max(0)`) et n'invente pas de four
    /// utilisable quand la compétence est cassée.
    pub fn max_usable_furnaces(&self) -> u32 {
        let from_skill = (self.skills.primary / 10 + 1).max(0) as u32;
        let fuelled = self
            .furnaces
            .iter()
            .filter(|furnace| furnace.is_furnace && furnace.has_fuel())
            .count() as u32;
        from_skill.min(fuelled)
    }

    /// `accelerateFurnaces` (`:120-122`) : `(secondarySkill / 10) * 2` ticks de
    /// cuisson par four, par passage.
    pub fn acceleration_ticks(&self) -> u32 {
        (self.skills.secondary / 10).max(0) as u32 * 2
    }

    /// Les fours que ce passage accélère : chargés, allumés, et un `serverTick` par
    /// tick d'accélération (`:124-140`).
    pub fn accelerating_furnaces(&self) -> u32 {
        self.furnaces
            .iter()
            .filter(|furnace| furnace.loaded && furnace.is_furnace && furnace.lit)
            .count() as u32
    }

    /// La taille de la demande de carburant : `STACKSIZE * module.getFurnaces()
    /// .size()` (`:111`).
    pub fn fuel_request_size(&self) -> u32 {
        STACKSIZE * self.furnace_count() as u32
    }

    /// `getAllowedFuel()` (`:695-705`), en identités.
    pub fn allowed(&self) -> &[NeedKey] {
        &self.allowed_fuel
    }

    /// `getActivePossibleFuels` (`:710-729`) : la liste autorisée, moins la sortie de
    /// la recette et moins son entrée, « pour éviter les interférences » (`:708`).
    /// Une recette absente ne retire rien, et une liste vide est une liste vide — Java
    /// lève alors l'interaction `FURNACE_USER_NO_FUEL` (`:715-718`).
    pub fn active_fuels(&self, recipe: Option<&Recipe>) -> Vec<NeedKey> {
        let mut fuels = self.allowed_fuel.clone();
        if let Some(recipe) = recipe {
            fuels.retain(|fuel| fuel != &recipe.output); // `:724`
            fuels.retain(|fuel| fuel != &recipe.input); // `:726`
        }
        fuels
    }

    /// `InventoryUtils.hasItemInItemHandler(citizen, isCorrectFuel(possibleFuels))`
    /// (`:107`, `:181`, `:208`, `:477`), pour une liste de carburants.
    pub fn carries_fuel(&self, fuels: &[NeedKey]) -> bool {
        self.fuel_in_inventory > 0 && !fuels.is_empty()
    }

    /// `InventoryUtils.hasBuildingEnoughElseCount(building, isCorrectFuel(...), 1)`
    /// (`:108`, `:183`, `:477`), le compte que Java compare à `1` puis à `2`.
    pub fn building_fuel(&self, fuels: &[NeedKey]) -> u32 {
        if fuels.is_empty() {
            0
        } else {
            self.fuel_in_building
        }
    }

    /// `getExtendedCount(stack)` (`:144-172`) : ce que les fours contiennent déjà de
    /// ce stack, entrées et sorties confondues.
    ///
    /// Java ne compte que si `currentRecipeStorage.getIntermediate() ==
    /// Blocks.FURNACE` (`:147`) ; le port prend le drapeau de la recette en argument,
    /// `None` valant « pas de recette », donc zéro.
    pub fn extended_count(&self, recipe: Option<&Recipe>, key: &NeedKey) -> u32 {
        let Some(recipe) = recipe else {
            return 0;
        };
        if !recipe.furnace_intermediate {
            return 0; // `:147`
        }
        self.furnaces
            .iter()
            .filter(|furnace| furnace.is_furnace)
            .map(|furnace| {
                let from_smeltable = furnace
                    .smeltable
                    .as_ref()
                    .filter(|slot| slot.is(key))
                    .map_or(0, |slot| slot.count);
                let from_result = furnace
                    .result
                    .as_ref()
                    .filter(|slot| slot.is(key))
                    .map_or(0, |slot| slot.count);
                from_smeltable + from_result
            })
            .sum()
    }
}

/// Ce que le concasseur lit de son bâtiment : le quota du jour et la recette du mode.
///
/// Référence Java : `building.getCurrentDailyQuantity()` et
/// `building.getSetting(BuildingCrusher.DAILY_LIMIT).getValue()`
/// (`EntityAIWorkCrusher.java:68`, `:93`, `:115`, `:121`, `:135-139`),
/// `building.getSetting(BuildingCrusher.MODE).getValue(building)` (`:114`, `:156`) et
/// `getBuildingLevel()` (`BuildingCrusher.java:88-96`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrusherFacts {
    /// `getCurrentDailyQuantity()` — remis à zéro au réveil du bâtiment
    /// (`BuildingCrusher.java:98-103`).
    pub daily_quantity: u32,
    /// `getSetting(DAILY_LIMIT).getValue()`. Le défaut est **0**
    /// (`BuildingModules.java:287`), donc un concasseur neuf ne fait rien tant que le
    /// joueur n'a pas posé de quota.
    pub daily_limit: u32,
    /// `getSetting(MODE).getValue(building)` : la recette que le mode impose, `None`
    /// quand le mode n'en donne aucune.
    pub mode_recipe: Option<Recipe>,
    /// `getBuildingLevel()` (`BuildingCrusher.java:90`).
    pub building_level: u8,
    /// La compétence primaire et secondaire du module de travail.
    pub skills: Skills,
}

impl CrusherFacts {
    /// Un bâtiment de niveau 5, dont le quota est illimité
    /// (`BuildingCrusher.java:90-93`).
    pub const UNLIMITED_LEVEL: u8 = MAX_BUILDING_LEVEL;

    /// `getCurrentDailyQuantity() >= dailyLimit` (`:121`).
    pub fn at_limit(&self) -> bool {
        self.daily_quantity >= self.daily_limit
    }

    /// Le quota restant, sans passer sous zéro.
    pub fn remaining(&self) -> u32 {
        self.daily_limit.saturating_sub(self.daily_quantity)
    }

    /// `getMaxDailyQuantity()` (`BuildingCrusher.java:88-96`) : `niveau² × 16`, et
    /// `Integer.MAX_VALUE` au niveau 5.
    ///
    /// L'IA du concasseur ne lit **pas** ce maximum — c'est le sifflet de sécurité du
    /// GUI ; la règle qui l'utilise est celle du cribleur
    /// (`EntityAIWorkSifter.java:110`). Le port le garde parce qu'il explique le
    /// défaut à zéro et le plafond du réglage, et dit ici qu'il n'entre dans aucune
    /// transition.
    pub fn max_daily_quantity(&self) -> u32 {
        if self.building_level >= MAX_BUILDING_LEVEL {
            return u32::MAX; // `:92`
        }
        let level = u32::from(self.building_level);
        level * level * BUILDING_LEVEL_MULTIPLIER
    }

    /// `Math.min((dailyLimit - currentDailyQuantity) * 2, STACKSIZE)`
    /// (`EntityAIWorkCrusher.java:157`), et le `requestQty <= 0` de `:158`.
    ///
    /// Java calcule en `int` puis teste `<= 0` : un quota déjà atteint donne un
    /// nombre négatif. Le port rend ce cas comme un refus nommé au lieu d'un compte
    /// négatif, donc `None` est le `<= 0` de Java et `Some(n)` son `n > 0`.
    pub fn request_quantity(&self) -> Option<u32> {
        if self.at_limit() {
            return None; // `:158-161`
        }
        let doubled = self.remaining().saturating_mul(2);
        Some(doubled.min(STACKSIZE))
    }
}

impl Default for CrusherFacts {
    /// Un bâtiment de niveau 1, quota zéro (le défaut du réglage), aucune recette de
    /// mode, aucune compétence.
    fn default() -> CrusherFacts {
        CrusherFacts {
            daily_quantity: 0,
            daily_limit: 0, // `BuildingModules.java:287`
            mode_recipe: None,
            building_level: 1,
            skills: Skills::default(),
        }
    }
}

/// `BuildingCrusher.BUILDING_LEVEL_MULTIPLIER` (`BuildingCrusher.java:33`).
pub const BUILDING_LEVEL_MULTIPLIER: u32 = 16;
/// `BuildingCrusher.MAX_BUILDING_LEVEL` (`BuildingCrusher.java:43`).
pub const MAX_BUILDING_LEVEL: u8 = 5;

/// Les faits du métier, selon celui qu'on exerce : un cuisinier et un teinturier
/// lisent des fours, un concasseur lit un quota.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: les trois métiers de cette tranche
pub enum TradeJob {
    /// `EntityAIWorkChef` — `AbstractEntityAIRequestSmelter`, un bâtiment
    /// `BuildingKitchen` (`EntityAIWorkChef.java:14`, `:27-30`).
    Chef(Box<SmelterFacts>),
    /// `EntityAIWorkDyer` — le même parent, un `BuildingDyer`
    /// (`EntityAIWorkDyer.java:10`, `:22-26`).
    Dyer(Box<SmelterFacts>),
    /// `EntityAIWorkCrusher` — `AbstractEntityAICrafting` plus l'état `CRUSH`
    /// (`EntityAIWorkCrusher.java:28`, `:49-52`).
    Crusher(Box<CrusherFacts>),
}

impl TradeJob {
    /// Le cuisinier, avec ses fours.
    pub fn chef(facts: SmelterFacts) -> TradeJob {
        TradeJob::Chef(Box::new(facts))
    }

    /// Le teinturier, avec ses fours.
    pub fn dyer(facts: SmelterFacts) -> TradeJob {
        TradeJob::Dyer(Box::new(facts))
    }

    /// Le concasseur, avec son quota.
    pub fn crusher(facts: CrusherFacts) -> TradeJob {
        TradeJob::Crusher(Box::new(facts))
    }

    /// La moitié four, quand le métier en a une.
    pub fn smelter(&self) -> Option<&SmelterFacts> {
        match self {
            TradeJob::Chef(facts) | TradeJob::Dyer(facts) => Some(facts),
            TradeJob::Crusher(_) => None,
        }
    }

    /// La moitié four, mutable.
    pub fn smelter_mut(&mut self) -> Option<&mut SmelterFacts> {
        match self {
            TradeJob::Chef(facts) | TradeJob::Dyer(facts) => Some(facts),
            TradeJob::Crusher(_) => None,
        }
    }

    /// La moitié concasseur, quand le métier en a une.
    pub fn crusher_facts(&self) -> Option<&CrusherFacts> {
        match self {
            TradeJob::Chef(_) | TradeJob::Dyer(_) => None,
            TradeJob::Crusher(facts) => Some(facts),
        }
    }

    /// Les compétences du métier, quel qu'il soit.
    pub fn skills(&self) -> Skills {
        match self {
            TradeJob::Chef(facts) | TradeJob::Dyer(facts) => facts.skills,
            TradeJob::Crusher(facts) => facts.skills,
        }
    }

    /// Ce que les fours contiennent déjà de `key` (`getExtendedCount`, `:144-172`).
    /// Zéro pour un concasseur, dont le parent ne compte rien d'étendu
    /// (`AbstractEntityAICrafting.java:430-433`).
    pub fn extended_count(&self, recipe: Option<&Recipe>, key: &NeedKey) -> u32 {
        match self {
            TradeJob::Chef(facts) | TradeJob::Dyer(facts) => facts.extended_count(recipe, key),
            TradeJob::Crusher(_) => 0,
        }
    }

    /// La recette que le mode du bâtiment impose, quand il en impose une
    /// (`EntityAIWorkCrusher.java:114`, `:156`).
    pub fn mode_recipe(&self) -> Option<&Recipe> {
        match self {
            TradeJob::Chef(_) | TradeJob::Dyer(_) => None,
            TradeJob::Crusher(facts) => facts.mode_recipe.as_ref(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(name: &str) -> NeedKey {
        NeedKey::plain(name)
    }

    /// `AbstractEntityAIRequestSmelter.java:556-572` : il faut **les deux** — le four
    /// s'est arrêté, **et** la sortie n'est pas vide.
    #[test]
    fn a_finished_furnace_needs_a_result_and_no_heat() {
        let running = Furnace::new(Pos::new(1, 2, 3))
            .with_smeltable(Slot::new(key("minecraft:iron_ore"), 3))
            .with_result(Slot::new(key("minecraft:iron_ingot"), 2))
            .burning();
        assert!(!running.result_ready(), ":556 — allumé, il fond encore");

        let stopped = Furnace::new(Pos::new(1, 2, 3))
            .with_result(Slot::new(key("minecraft:iron_ingot"), 2));
        assert!(stopped.result_ready());

        let lit_but_empty = Furnace::new(Pos::new(1, 2, 3))
            .with_result(Slot::new(key("minecraft:iron_ingot"), 2))
            .burning();
        assert!(lit_but_empty.result_ready(), ":556 — `|| smeltable vide`");
    }

    /// `:562` compare le compte au maximum de pile, ce qui n'est pas la même chose
    /// que « non vide » : le port garde les deux lectures séparées.
    #[test]
    fn the_slot_carries_its_own_max_stack() {
        let slot = Slot::new(key("minecraft:iron_ingot"), 8).with_max_stack(16);
        assert_eq!(slot.max_stack, 16);
        assert_eq!(Slot::new(key("minecraft:iron_ingot"), 8).max_stack, STACKSIZE);
        assert!(Slot::new(key("minecraft:iron_ingot"), 0).is_empty());
    }

    /// `ItemStackUtils.java:743-747` : du carburant, rien à fondre.
    #[test]
    fn fuel_without_smeltable_is_the_filling_condition() {
        let fed = Furnace::new(Pos::new(0, 0, 0)).with_fuel(Slot::new(key("minecraft:coal"), 5));
        assert!(fed.fuel_without_smeltable());
        assert!(fed.fuel_without_smeltable_and_result(), ":516-518");
        assert!(fed.has_fuel());

        let busy = fed
            .clone()
            .with_smeltable(Slot::new(key("minecraft:iron_ore"), 1));
        assert!(!busy.fuel_without_smeltable());
        assert!(!busy.fuel_without_smeltable_and_result(), ":516 — le slot d'entrée");

        let with_result = fed
            .clone()
            .with_result(Slot::new(key("minecraft:iron_ingot"), 1));
        assert!(!with_result.fuel_without_smeltable_and_result(), ":517 — le slot de sortie");
    }

    /// `:536` : un four éteint et sans carburant est celui qu'on va ravitailler.
    #[test]
    fn a_furnace_without_fuel_is_the_one_to_feed() {
        let cold = Furnace::new(Pos::new(0, 0, 0));
        assert!(cold.without_fuel());
        let cold_but_full = cold.clone().with_fuel(Slot::new(key("minecraft:coal"), 1));
        assert!(!cold_but_full.without_fuel());
        let lit = cold.clone().burning();
        assert!(!lit.without_fuel(), ":536 — `!isLit()`");
    }

    /// `:659` : « tourne » veut dire allumé **et** avec quelque chose à fondre.
    #[test]
    fn burning_needs_heat_and_a_smeltable() {
        let lit_empty = Furnace::new(Pos::new(0, 0, 0)).burning();
        assert!(!lit_empty.is_burning());
        let lit_full = lit_empty
            .clone()
            .with_smeltable(Slot::new(key("minecraft:iron_ore"), 1));
        assert!(lit_full.is_burning());
    }

    /// `:588-590` : sans recette, **tout** contenu est un intrus. C'est la lecture
    /// littérale de `currentRecipeStorage == null ||`, pas un raccourci.
    #[test]
    fn without_a_recipe_every_input_is_unrelated() {
        let ore = Furnace::new(Pos::new(0, 0, 0))
            .with_smeltable(Slot::new(key("minecraft:iron_ore"), 1));
        assert!(ore.holds_unrelated_input(None));
        assert!(!ore.holds_unrelated_input(Some(&key("minecraft:iron_ore"))));
        assert!(ore.holds_unrelated_input(Some(&key("minecraft:gold_ore"))));

        let empty = Furnace::new(Pos::new(0, 0, 0));
        assert!(!empty.holds_unrelated_input(None), ":588 — un slot vide n'est pas un intrus");
    }

    /// `getActivePossibleFuels` (`:710-729`) : la sortie et l'entrée de la recette
    /// sortent de la liste, et une recette absente ne retire rien.
    #[test]
    fn the_active_fuel_list_drops_the_recipe() {
        let facts = SmelterFacts {
            allowed_fuel: vec![key("minecraft:coal"), key("minecraft:oak_planks")],
            ..SmelterFacts::default()
        };
        let recipe = Recipe::smelted(key("minecraft:oak_log"), key("minecraft:charcoal"));
        assert_eq!(facts.active_fuels(None).len(), 2);
        assert_eq!(
            facts.active_fuels(Some(&recipe)),
            vec![key("minecraft:coal"), key("minecraft:oak_planks")],
            "ni l'entrée ni la sortie de la recette ne sont dans la liste : rien ne bouge"
        );

        let plank_recipe = Recipe::smelted(key("minecraft:cobblestone"), key("minecraft:oak_planks"));
        assert_eq!(
            facts.active_fuels(Some(&plank_recipe)),
            vec![key("minecraft:coal")],
            ":724 — la sortie de la recette sort de la liste"
        );
        let coal_recipe = Recipe::smelted(key("minecraft:coal"), key("minecraft:torch"));
        assert_eq!(
            facts.active_fuels(Some(&coal_recipe)),
            vec![key("minecraft:oak_planks")],
            ":726 — l'entrée aussi"
        );
    }

    /// `getMaxUsableFurnaces` (`:673-688`) : `primary / 10 + 1`, plafonné par les
    /// fours qui ont du carburant.
    #[test]
    fn the_usable_furnaces_are_skill_capped_by_fuelled_ones() {
        let coal = Slot::new(key("minecraft:coal"), 1);
        let three_fed = SmelterFacts {
            furnaces: vec![
                Furnace::new(Pos::new(0, 0, 0)).with_fuel(coal.clone()),
                Furnace::new(Pos::new(1, 0, 0)).with_fuel(coal.clone()),
                Furnace::new(Pos::new(2, 0, 0)).with_fuel(coal.clone()),
                Furnace::new(Pos::new(3, 0, 0)),
            ],
            skills: Skills::new(0, 0, 0),
            ..SmelterFacts::default()
        };
        assert_eq!(three_fed.max_usable_furnaces(), 1, "niveau 0 → 1 four");
        assert_eq!(
            SmelterFacts {
                skills: Skills::new(25, 0, 0),
                ..three_fed.clone()
            }
            .max_usable_furnaces(),
            3,
            "niveau 25 → 3 fours"
        );
        assert_eq!(
            SmelterFacts {
                skills: Skills::new(90, 0, 0),
                ..three_fed.clone()
            }
            .max_usable_furnaces(),
            3,
            ":687 — plafonné par les fours ravitaillés"
        );
        assert_eq!(
            SmelterFacts {
                skills: Skills::new(-10, 0, 0),
                ..three_fed
            }
            .max_usable_furnaces(),
            0,
            "un niveau cassé ne donne pas un plafond négatif"
        );
    }

    /// `:120-122` : `(secondary / 10) * 2`, plancher zéro.
    #[test]
    fn the_acceleration_is_two_ticks_per_ten_levels() {
        let facts = |secondary| SmelterFacts {
            skills: Skills::new(0, secondary, 0),
            ..SmelterFacts::default()
        };
        assert_eq!(facts(0).acceleration_ticks(), 0);
        assert_eq!(facts(9).acceleration_ticks(), 0);
        assert_eq!(facts(10).acceleration_ticks(), 2);
        assert_eq!(facts(30).acceleration_ticks(), 6);
        assert_eq!(facts(-5).acceleration_ticks(), 0);
    }

    /// `:144-172` : ce que les fours contiennent déjà, **seulement** pour une recette
    /// qui passe par un four.
    #[test]
    fn the_extended_count_only_counts_for_a_furnace_recipe() {
        let facts = SmelterFacts {
            furnaces: vec![
                Furnace::new(Pos::new(0, 0, 0))
                    .with_smeltable(Slot::new(key("minecraft:iron_ore"), 4)),
                Furnace::new(Pos::new(1, 0, 0))
                    .with_result(Slot::new(key("minecraft:iron_ore"), 2)),
                Furnace::new(Pos::new(2, 0, 0))
                    .with_smeltable(Slot::new(key("minecraft:gold_ore"), 9)),
            ],
            ..SmelterFacts::default()
        };
        let smelted = Recipe::smelted(key("minecraft:iron_ore"), key("minecraft:iron_ingot"));
        assert_eq!(
            facts.extended_count(Some(&smelted), &key("minecraft:iron_ore")),
            6,
            ":152-164 — entrées et sorties confondues"
        );
        let crafted = Recipe::crafted(key("minecraft:iron_ore"), key("minecraft:iron_ingot"));
        assert_eq!(facts.extended_count(Some(&crafted), &key("minecraft:iron_ore")), 0);
        assert_eq!(facts.extended_count(None, &key("minecraft:iron_ore")), 0);
    }

    /// `EntityAIWorkCrusher.java:157` : `min((quota − fait) × 2, 64)`, et le `<= 0`
    /// de `:158`.
    #[test]
    fn the_crusher_asks_for_twice_what_is_left() {
        let facts = |quantity, limit| CrusherFacts {
            daily_quantity: quantity,
            daily_limit: limit,
            ..CrusherFacts::default()
        };
        assert_eq!(facts(0, 8).request_quantity(), Some(16));
        assert_eq!(facts(0, 100).request_quantity(), Some(STACKSIZE), ":157 — plafond 64");
        assert_eq!(facts(5, 8).request_quantity(), Some(6));
        assert_eq!(facts(8, 8).request_quantity(), None, ":158 — quota atteint");
        assert_eq!(facts(9, 8).request_quantity(), None, "un dépassement ne donne pas un compte négatif");
        assert_eq!(facts(0, 0).request_quantity(), None, ":68 — le défaut du réglage est 0");
        assert!(facts(0, 8).remaining() == 8 && !facts(0, 8).at_limit());
    }

    /// `BuildingCrusher.java:88-96` : `niveau² × 16`, et pas de maximum au niveau 5.
    #[test]
    fn the_building_level_caps_the_daily_quantity() {
        let at = |level| CrusherFacts {
            building_level: level,
            ..CrusherFacts::default()
        };
        assert_eq!(at(1).max_daily_quantity(), 16);
        assert_eq!(at(2).max_daily_quantity(), 64);
        assert_eq!(at(4).max_daily_quantity(), 256);
        assert_eq!(at(5).max_daily_quantity(), u32::MAX, ":90-93");
        assert_eq!(CrusherFacts::UNLIMITED_LEVEL, MAX_BUILDING_LEVEL);
    }

    /// Les quatre sélecteurs rendent le **premier** four du module qui répond, et un
    /// bloc qui n'est plus un four est écarté de tous.
    #[test]
    fn the_selectors_stop_at_the_first_furnace_that_answers() {
        let facts = SmelterFacts {
            furnaces: vec![
                Furnace::new(Pos::new(0, 0, 0)).not_a_furnace(),
                Furnace::new(Pos::new(1, 0, 0)).with_result(Slot::new(key("minecraft:iron_ingot"), 1)),
                Furnace::new(Pos::new(2, 0, 0)).with_result(Slot::new(key("minecraft:iron_ingot"), 1)),
            ],
            ..SmelterFacts::default()
        };
        assert_eq!(facts.stale(), Some(Pos::new(0, 0, 0)), ":595-599");
        assert_eq!(facts.output_ready(), Some(Pos::new(1, 0, 0)), ":553");
        assert_eq!(facts.at(Pos::new(0, 0, 0)), None, "un vestige n'est pas un four");
        assert_eq!(facts.at(Pos::new(2, 0, 0)).map(|f| f.at), Some(Pos::new(2, 0, 0)));
        assert_eq!(facts.furnace_count(), 3, "le module les compte tous");
    }

    /// `:631-641` : il suffit d'**un** chunk déchargé pour refuser le passage.
    #[test]
    fn one_unloaded_furnace_stops_the_whole_pass() {
        let facts = SmelterFacts {
            furnaces: vec![
                Furnace::new(Pos::new(0, 0, 0)),
                Furnace::new(Pos::new(1, 0, 0)).unloaded(),
            ],
            ..SmelterFacts::default()
        };
        assert!(!facts.all_loaded());
        assert!(SmelterFacts::default().all_loaded(), "aucun four, donc rien de déchargé");
    }

    /// `:648-667` : seuls les fours **chargés** comptent, et il faut allumé + entrée.
    #[test]
    fn the_burning_count_skips_unloaded_furnaces() {
        let ore = Slot::new(key("minecraft:iron_ore"), 1);
        let facts = SmelterFacts {
            furnaces: vec![
                Furnace::new(Pos::new(0, 0, 0)).with_smeltable(ore.clone()).burning(),
                Furnace::new(Pos::new(1, 0, 0)).with_smeltable(ore.clone()).burning().unloaded(),
                Furnace::new(Pos::new(2, 0, 0)).with_smeltable(ore).burning().not_a_furnace(),
                Furnace::new(Pos::new(3, 0, 0)).burning(),
            ],
            ..SmelterFacts::default()
        };
        assert_eq!(facts.burning(), 1);
        assert_eq!(facts.accelerating_furnaces(), 2, ":126 — `isLit()` suffit pour accélérer");
    }

    /// Les deux moitiés d'un métier : un cuisinier n'a pas de quota, un concasseur
    /// n'a pas de fours.
    #[test]
    fn each_trade_reads_only_its_own_half() {
        let chef = TradeJob::chef(SmelterFacts::default());
        assert!(chef.smelter().is_some());
        assert!(chef.crusher_facts().is_none());
        assert!(chef.mode_recipe().is_none());

        let crusher = TradeJob::crusher(CrusherFacts::default());
        assert!(crusher.smelter().is_none());
        assert!(crusher.crusher_facts().is_some());
        assert_eq!(
            crusher.extended_count(None, &key("minecraft:iron_ore")),
            0,
            "AbstractEntityAICrafting.java:430-433 — le parent ne compte rien d'étendu"
        );

        let dyer = TradeJob::dyer(SmelterFacts {
            skills: Skills::new(1, 2, 3),
            ..SmelterFacts::default()
        });
        assert_eq!(dyer.skills(), Skills::new(1, 2, 3));
    }
}
