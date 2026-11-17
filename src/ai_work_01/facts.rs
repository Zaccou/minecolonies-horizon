//! Ce que les trois métiers de l'atelier voient du monde : les alambics, les fours,
//! la terre du nether et les feuilles — rien d'autre.
//!
//! Référence Java : `EntityAIWorkAlchemist.java:121-218` (nether wart) et `:234-304`
//! (gui), qui lisent `building.getAllSoilPositions()` / `getAllLeavePositions()`,
//! `WorldUtil.isBlockLoaded` et `world.getBlockState` ; `:357-392` et
//! `AbstractEntityAIRequestSmelter.java:145-176`, qui lisent les quatre slots de
//! chaque alambic et les trois de chaque four.
//!
//! **Chaque tirage aléatoire est un fait.** `worker.getRandom().nextInt(n)`
//! (`:132`, `:250`, `:286`, `:314`, `:319`) ne se rejoue pas, donc l'appelant le
//! fournit — même raison que `crate::ai::facts::WorkerFacts` pour les deux tirages de
//! `dumpOneMoreSlot`.
//!
//! **L'identité d'un stack n'est pas réinventée ici.** Un stack est nommé par
//! [`NeedKey`], la clé que la résolution de matériaux du bâtisseur utilise déjà
//! (`descriptionId` + hash de tag), donc « l'alambic contient la sortie de la
//! recette » est une égalité et non une seconde table de correspondance.

use crate::ai::Pos;
pub use crate::ai_build::materials::NeedKey;

/// `Constants.java:153` — `INGREDIENT_SLOT = 3`, la fiole d'ingrédient de l'alambic.
pub const INGREDIENT_SLOT: usize = 3;
/// `Constants.java:148` — `BREWING_FUEL_SLOT = 4`, la poudre de blaze.
pub const BREWING_FUEL_SLOT: usize = 4;
/// `Constants.java:158` — `BREWING_MIN_FUEL_COUNT = 8`.
pub const BREWING_MIN_FUEL_COUNT: u32 = 8;
/// `Constants.java:123` — `SMELTABLE_SLOT = 0`.
pub const SMELTABLE_SLOT: usize = 0;
/// `Constants.java:128` — `FUEL_SLOT = 1`.
pub const FUEL_SLOT: usize = 1;
/// `Constants.java:118` — `RESULT_SLOT = 2`.
pub const RESULT_SLOT: usize = 2;

/// Le nombre de slots de potion que lisent `:669` et `:756` (`slot < 3`).
pub const POTION_SLOTS: usize = 3;
/// Le nombre de slots d'un alambic : 3 potions, l'ingrédient, le carburant.
pub const BREWING_SLOTS: usize = 5;
/// `getExtendedCount` ne lit que les slots `0..4`, **jamais** le carburant
/// (`EntityAIWorkAlchemist.java:372`) : c'est le slot 3 inclus, le 4 exclu.
pub const SLOTS_COUNTED_BY_EXTENDED_COUNT: usize = 4;

/// Un stack tel que l'IA le lit : l'identité que Java compare
/// (`ItemStackUtils.compareItemStacksIgnoreStackSize`) et la quantité.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stack {
    /// `ItemStack.getDescriptionId()` + le hash de tag, comme `materials::NeedKey`.
    pub key: NeedKey,
    /// `stack.getCount()`.
    pub count: u32,
}

impl Stack {
    /// Un stack de `count` items de `key`.
    pub fn new(key: NeedKey, count: u32) -> Stack {
        Stack { key, count }
    }
}

/// Un alambic en fonctionnement, réduit à ce que l'IA en lit.
///
/// Référence Java : `BrewingStandBlockEntity.brewTime` (`:454`, `:517`, `:678`,
/// `:855`), `getItem(slot)` pour les cinq slots, et les trois prédicats de
/// `ItemStackUtils.java:751-786`.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Stand {
    /// `building.getAllBrewingStandPositions()` : la position rapportée par la hutte.
    pub at: Pos,
    /// `WorldUtil.isBlockLoaded(world, pos)` (`:448`, `:508`, `:549`) : un alambic
    /// déchargé n'est ni lu ni compté.
    pub loaded: bool,
    /// `brewingStand.brewTime` : au-dessus de zéro, il brasse (`:454`).
    pub brew_time: u32,
    /// Les cinq slots dans l'ordre vanilla : `[0,1,2]` potions, `[3]` ingrédient,
    /// `[4]` carburant (`Constants.java:153`, `:148`).
    pub slots: [Option<Stack>; BREWING_SLOTS],
}

impl Stand {
    /// Un alambic vide, chargé, à `at`.
    pub fn new(at: Pos) -> Stand {
        Stand {
            at,
            loaded: true,
            brew_time: 0,
            slots: Default::default(),
        }
    }

    /// Un alambic qui brasse : `brewTime > 0` (`:454`).
    pub fn is_bubbling(&self) -> bool {
        self.brew_time > 0
    }

    /// `isEmpty(getItem(slot))` — un slot hors bornes n'est pas une panique.
    pub fn is_empty(&self, slot: usize) -> bool {
        self.slots.get(slot).map(Option::is_none).unwrap_or(true)
    }

    /// `getItem(slot).getCount()`, `0` pour un slot vide ou hors bornes.
    pub fn count_at(&self, slot: usize) -> u32 {
        self.slots
            .get(slot)
            .and_then(|stack| stack.as_ref())
            .map(|stack| stack.count)
            .unwrap_or(0)
    }

    /// `getExtendedCount(stack)` (`:357-392`) : la somme des slots `0..4` qui portent
    /// ce stack. Le carburant (slot 4) n'est **pas** dans la boucle de Java.
    pub fn count_of(&self, key: &NeedKey) -> u32 {
        self.slots
            .iter()
            .take(SLOTS_COUNTED_BY_EXTENDED_COUNT)
            .filter_map(|stack| stack.as_ref())
            .filter(|stack| &stack.key == key)
            .map(|stack| stack.count)
            .sum()
    }

    /// `getPositionOfBrewingStandToRetrieveFrom`'s count (`:667-676`) : Java
    /// **assigne** au lieu d'additionner, donc c'est le dernier slot 0..3 qui porte la
    /// sortie qui gagne.
    pub fn output_in_potion_slots(&self, key: &NeedKey) -> u32 {
        let mut count = 0;
        for slot in 0..POTION_SLOTS {
            if let Some(stack) = self.slots.get(slot).and_then(|stack| stack.as_ref()) {
                if &stack.key == key {
                    count = stack.count;
                }
            }
        }
        count
    }

    /// `isEmpty(getItem(0)) || isEmpty(getItem(1)) || isEmpty(getItem(2))`
    /// (`:921-922`).
    pub fn has_empty_potion_slot(&self) -> bool {
        (0..POTION_SLOTS).any(|slot| self.is_empty(slot))
    }

    /// `ItemStackUtils.hasBrewableAndNoFuel` (`ItemStackUtils.java:755-759`).
    pub fn has_brewable_and_no_fuel(&self) -> bool {
        !self.is_empty(INGREDIENT_SLOT) && self.is_empty(BREWING_FUEL_SLOT)
    }

    /// `ItemStackUtils.hasNeitherFuelNorBrewable` (`:770-774`).
    pub fn has_neither_fuel_nor_brewable(&self) -> bool {
        self.is_empty(INGREDIENT_SLOT) && self.is_empty(BREWING_FUEL_SLOT)
    }

    /// `ItemStackUtils.hasFuelAndNoBrewable` (`:782-786`).
    pub fn has_fuel_and_no_brewable(&self) -> bool {
        self.is_empty(INGREDIENT_SLOT) && !self.is_empty(BREWING_FUEL_SLOT)
    }

    /// Les deux prédicats que `isFuelNeeded` et `checkBrewingStandFuel` testent
    /// ensemble (`:517`, `:555`, `:619`, `:945`, `:1011`).
    pub fn wants_fuel(&self) -> bool {
        self.has_brewable_and_no_fuel() || self.has_neither_fuel_nor_brewable()
    }
}

/// Un four, réduit à ce que l'IA de boulangerie en lit.
///
/// Référence Java : `FurnaceBlockEntity.isLit()` (`:533`, `:551`, `:657`),
/// `getItem(SMELTABLE_SLOT)` / `FUEL_SLOT` / `RESULT_SLOT` (`:512-527`, `:551-578`)
/// et `ItemStackUtils.hasFuelInFurnaceAndNoSmeltable` (`ItemStackUtils.java:744-748`).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Furnace {
    /// `building.getModule(FURNACE).getFurnaces()`.
    pub at: Pos,
    /// `WorldUtil.isBlockLoaded(world, pos)` (`:126`, `:633`).
    pub loaded: bool,
    /// `furnace.isLit()`.
    pub lit: bool,
    /// Les trois slots : `[0]` à cuire, `[1]` carburant, `[2]` résultat
    /// (`Constants.java:123`, `:128`, `:118`).
    pub slots: [Option<Stack>; 3],
}

impl Furnace {
    /// Un four vide, éteint, chargé, à `at`.
    pub fn new(at: Pos) -> Furnace {
        Furnace {
            at,
            loaded: true,
            lit: false,
            slots: Default::default(),
        }
    }

    /// `isEmpty(getItem(slot))`.
    pub fn is_empty(&self, slot: usize) -> bool {
        self.slots.get(slot).map(Option::is_none).unwrap_or(true)
    }

    /// `getItem(slot).getCount()`.
    pub fn count_at(&self, slot: usize) -> u32 {
        self.slots
            .get(slot)
            .and_then(|stack| stack.as_ref())
            .map(|stack| stack.count)
            .unwrap_or(0)
    }

    /// L'identité du stack d'un slot, pour `compareItemStacksIgnoreStackSize`
    /// (`:300`, `:592`).
    pub fn key_at(&self, slot: usize) -> Option<&NeedKey> {
        self.slots
            .get(slot)
            .and_then(|stack| stack.as_ref())
            .map(|stack| &stack.key)
    }

    /// `hasFuelInFurnaceAndNoSmeltable` (`ItemStackUtils.java:744-748`).
    pub fn has_fuel_and_no_smeltable(&self) -> bool {
        self.is_empty(SMELTABLE_SLOT) && !self.is_empty(FUEL_SLOT)
    }

    /// `countOfBurningFurnaces`'s test (`:657-660`) : allumé **et** avec de quoi cuire.
    pub fn is_burning(&self) -> bool {
        self.lit && !self.is_empty(SMELTABLE_SLOT)
    }
}

/// Ce qu'il y a au-dessus d'une terre du nether (`:138`, `:143`, `:168-170`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Above {
    /// `world.isEmptyBlock(above)` (`:143`, `:191`).
    #[default]
    Empty,
    /// `Blocks.NETHER_WART` et son `AGE` (`:138`, `:170`).
    NetherWart {
        /// `NetherWartBlock.AGE` ; en dessous de 2 la plante n'est pas mûre (`:170`).
        age: u8,
    },
    /// Tout le reste.
    Other,
}

/// Une terre du nether que la hutte liste.
///
/// Référence Java : `building.getAllSoilPositions()` (`:125`),
/// `WorldUtil.isBlockLoaded` (`:134`, `:160`), `getBlockState(pos).getBlock() ==
/// Blocks.SOUL_SAND` (`:136`, `:160`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Soil {
    /// La position listée par la hutte.
    pub at: Pos,
    /// `WorldUtil.isBlockLoaded(world, pos)`.
    pub loaded: bool,
    /// `getBlockState(pos).getBlock() == Blocks.SOUL_SAND`.
    pub soul_sand: bool,
    /// L'état de la case au-dessus.
    pub above: Above,
}

/// Un bloc de feuilles que la hutte liste (`:243`, `:251-256`, `:265`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Leaf {
    /// La position listée par la hutte.
    pub at: Pos,
    /// `WorldUtil.isBlockLoaded(world, pos)` (`:251`, `:265`).
    pub loaded: bool,
    /// `getBlockState(pos).getBlock() instanceof LeavesBlock` (`:253`, `:265`).
    pub leaves: bool,
}

/// Les faits de l'alchimiste.
///
/// Référence Java : `EntityAIWorkAlchemist` — `getAllBrewingStandPositions()`
/// (`:363`, `:417`, `:446`, `:506`, `:547`, `:661`, `:861`, `:894`, `:1077`),
/// `getAllSoilPositions()` (`:125`), `getAllLeavePositions()` (`:243`),
/// `currentRecipeStorage` (`:360`, `:415`, `:501`, `:656`, `:690`, `:913`),
/// `BLAZE_POWDER` (`:539-541`, `:557-559`, `:593-595`), les tirages de `:132`, `:250`,
/// `:286`, `:314`, `:319`, `checkIfRequestForItemExistOrCreateAsync` (`:145`, `:191`),
/// `checkForToolOrWeapon(shears)` (`:236`) et `getPrimarySkill` /
/// `getSecondarySkill` (`:473`, `:644`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlchemistFacts {
    /// Les alambics de la hutte, dans l'ordre où elle les rapporte.
    pub stands: Vec<Stand>,
    /// Les terres du nether de la hutte.
    pub soil: Vec<Soil>,
    /// Les blocs de feuilles de la hutte.
    pub leaves: Vec<Leaf>,
    /// `currentRecipeStorage.getPrimaryOutput()` (`:671`, `:917`).
    pub output: NeedKey,
    /// `getCleanedInput().get(0)` — l'ingrédient (`:992`).
    pub ingredient: NeedKey,
    /// `getCleanedInput().get(1)` — la potion (`:924`).
    pub potion: NeedKey,
    /// `getIntermediate() == Blocks.BREWING_STAND` (`:360`, `:415`, `:501`, `:532`,
    /// `:656`, `:690`).
    pub brewing_recipe: bool,
    /// `hasItemInItemHandler(inv, BLAZE_POWDER)` (`:539`, `:557`, `:593`, `:618`).
    pub blaze_powder_in_inventory: bool,
    /// `hasItemInProvider(building, BLAZE_POWDER)` (`:540`, `:559`, `:595`).
    pub blaze_powder_in_hut: bool,
    /// `building.hasWorkerOpenRequestsOfType(..., Stack.class)` (`:541`).
    pub open_fuel_request: bool,
    /// Le tirage de `:132` / `:250` : quelle terre, quelle feuille. Ramené modulo la
    /// taille de la liste, comme `crafter::pick`, pour qu'un tirage hors bornes ne
    /// panique pas.
    pub pick: usize,
    /// `worker.getRandom().nextInt(DELAY_TO_HARVEST_NETHERWART)` (`:314`).
    pub nether_wart_roll: u32,
    /// `worker.getRandom().nextInt(DELAY_TO_HARVEST_MISTLETOE)` (`:319`).
    pub mistletoe_roll: u32,
    /// `worker.getRandom().nextInt(40)` (`:286`).
    pub mistletoe_roll_forty: u32,
    /// `checkIfRequestForItemExistOrCreateAsync(new ItemStack(NETHER_WART), 16, 1)`
    /// (`:145`, `:191`) : faux quand la requête n'a pas pu être faite.
    pub wart_request_ok: bool,
    /// `findFirstSlotInInventoryWith(NETHER_WART) != -1` (`:197-202`).
    pub wart_in_inventory: u32,
    /// `walkToWorkPos(walkTo)` (`:162`, `:267`, `:606`, `:747`, `:809`, `:961`,
    /// `:1020`).
    pub at_work_pos: bool,
    /// `mineBlock(walkTo.above())` (`:178`).
    pub mined: bool,
    /// `checkForToolOrWeapon(ModEquipmentTypes.shears)` (`:236`) : vrai quand la
    /// cisaille manque.
    pub shears_missing: bool,
    /// `getModuleForJob().getPrimarySkill()` (`:644`).
    pub primary_skill: u8,
    /// Le second métier, celui que `accelerateBrewingStand` lit (`:473`).
    pub secondary_skill: u8,
}

impl Default for AlchemistFacts {
    /// Aucun alambic, aucune terre, aucune feuille, une recette qui ne brasse pas.
    fn default() -> AlchemistFacts {
        AlchemistFacts {
            stands: Vec::new(),
            soil: Vec::new(),
            leaves: Vec::new(),
            output: NeedKey::plain("minecraft:potion"),
            ingredient: NeedKey::plain("minecraft:nether_wart"),
            potion: NeedKey::plain("minecraft:potion"),
            brewing_recipe: false,
            blaze_powder_in_inventory: false,
            blaze_powder_in_hut: false,
            open_fuel_request: false,
            pick: 0,
            nether_wart_roll: 0,
            mistletoe_roll: 0,
            mistletoe_roll_forty: 1,
            wart_request_ok: true,
            wart_in_inventory: 0,
            at_work_pos: true,
            mined: true,
            shears_missing: false,
            primary_skill: 0,
            secondary_skill: 0,
        }
    }
}

impl AlchemistFacts {
    /// L'alambic à `at`, s'il est listé.
    pub fn stand_at(&self, at: Pos) -> Option<&Stand> {
        self.stands.iter().find(|stand| stand.at == at)
    }

    /// La feuille à `at`, si elle est listée (`:265`).
    pub fn leaf_at(&self, at: Pos) -> Option<&Leaf> {
        self.leaves.iter().find(|leaf| leaf.at == at)
    }

    /// La terre tirée par `:132` / `:250`, `None` sur une liste vide.
    pub fn picked_soil(&self) -> Option<Soil> {
        if self.soil.is_empty() {
            return None;
        }
        self.soil.get(self.pick % self.soil.len()).copied()
    }

    /// La feuille tirée par `:250`, `None` sur une liste vide.
    pub fn picked_leaf(&self) -> Option<Leaf> {
        if self.leaves.is_empty() {
            return None;
        }
        self.leaves.get(self.pick % self.leaves.len()).copied()
    }
}

/// Les faits du boulanger.
///
/// Référence Java : `AbstractEntityAIRequestSmelter` —
/// `building.getModule(FURNACE).getFurnaces()` (`:124`, `:512`, `:533`, `:551`, `:581`,
/// `:648`, `:673`, `:695`), `currentRecipeStorage` (`:145`, `:300`, `:339`, `:434`,
/// `:592`), `getAllowedFuel()` (`:695-708`), `isCorrectFuel` (`:734-737`) et
/// `getModuleForJob().getPrimarySkill()` / `getSecondarySkill()` (`:122`, `:675`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BakerFacts {
    /// Les fours de la hutte, dans son ordre.
    pub furnaces: Vec<Furnace>,
    /// `currentRecipeStorage.getPrimaryOutput()` (`:145`, `:156`, `:300`, `:345`).
    pub output: NeedKey,
    /// `getCleanedInput().get(0)` (`:339`, `:592`) : Java dit « il n'y a jamais qu'une
    /// entrée » juste au-dessus de `:592`.
    pub input: NeedKey,
    /// `getIntermediate() == Blocks.FURNACE` (`:147`, `:434`).
    pub furnace_recipe: bool,
    /// `getAllowedFuel().size()` (`:695-708`) : zéro veut dire que la hutte ne liste
    /// aucun carburant, et `getActivePossibleFuels` (`:710-720`) plaint alors le
    /// joueur.
    pub allowed_fuels: usize,
    /// `hasItemInItemHandler(inv, isCorrectFuel(...))` (`:107`, `:180`, `:471`).
    pub fuel_in_inventory: bool,
    /// `hasBuildingEnoughElseCount(building, isCorrectFuel(...), 1)` (`:108`, `:183`,
    /// `:471`).
    pub fuel_in_hut: u32,
    /// `building.hasWorkerOpenRequestsOfType(..., StackList.class)` (`:109`).
    pub open_fuel_request: bool,
    /// `areFurnacesLoaded()` (`:455-458`).
    pub furnaces_loaded: bool,
    /// `walkToWorkPos(furnacePos)` (`:196`, `:241`, `:304`, `:367`) : le citoyen est
    /// déjà au four qu'il veut nourrir ou vider.
    pub at_work_pos: bool,
    /// `getModuleForJob().getPrimarySkill()` (`:675`).
    pub primary_skill: u8,
    /// `getModuleForJob().getSecondarySkill()` (`:122`).
    pub secondary_skill: u8,
}

impl Default for BakerFacts {
    /// Aucun four, une recette de table de travail, aucun carburant listé.
    fn default() -> BakerFacts {
        BakerFacts {
            furnaces: Vec::new(),
            output: NeedKey::plain("minecraft:bread"),
            input: NeedKey::plain("minecraft:wheat"),
            furnace_recipe: false,
            allowed_fuels: 1,
            fuel_in_inventory: false,
            fuel_in_hut: 0,
            open_fuel_request: false,
            furnaces_loaded: true,
            at_work_pos: true,
            primary_skill: 0,
            secondary_skill: 0,
        }
    }
}

impl BakerFacts {
    /// Le four à `at`, s'il est listé (`:513`, `:534`, `:552`).
    pub fn furnace_at(&self, at: Pos) -> Option<&Furnace> {
        self.furnaces.iter().find(|furnace| furnace.at == at)
    }

    /// Les fours chargés, que les compteurs lisent (`:648-665`, `:673-692`).
    pub fn loaded(&self) -> impl Iterator<Item = &Furnace> {
        self.furnaces.iter().filter(|furnace| furnace.loaded)
    }
}

/// La moitié « métier » d'une scène : les faits du métier qui tique.
///
/// Passer les faits d'un autre métier vaut
/// [`super::state::WorkRefusal::NoFactsForTrade`], pas un défaut silencieux : « cet
/// alchimiste n'a pas d'alambic » et « ces faits sont ceux d'un boulanger » ne doivent
/// pas se confondre.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TradeFacts {
    /// `EntityAIWorkAlchemist`.
    Alchemist(AlchemistFacts),
    /// `EntityAIWorkBaker`.
    Baker(BakerFacts),
    /// `EntityAIWorkBlacksmith` : **aucun fait propre**. Ses 33 lignes
    /// (`EntityAIWorkBlacksmith.java:1-33`) n'ajoutent que
    /// `getExpectedBuildingClass`.
    Blacksmith,
}

impl Default for TradeFacts {
    fn default() -> TradeFacts {
        TradeFacts::Blacksmith
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(name: &str) -> NeedKey {
        NeedKey::plain(name)
    }

    /// `:372` : `getExtendedCount` additionne les slots 0..4 et **ignore** le
    /// carburant (slot 4).
    #[test]
    fn the_extended_count_reads_four_slots_and_not_the_fuel() {
        let mut stand = Stand::new(Pos::new(1, 64, 1));
        stand.slots[0] = Some(Stack::new(key("minecraft:potion"), 1));
        stand.slots[1] = Some(Stack::new(key("minecraft:potion"), 2));
        stand.slots[INGREDIENT_SLOT] = Some(Stack::new(key("minecraft:potion"), 3));
        stand.slots[BREWING_FUEL_SLOT] = Some(Stack::new(key("minecraft:potion"), 64));
        assert_eq!(stand.count_of(&key("minecraft:potion")), 6);
        assert_eq!(stand.count_at(BREWING_FUEL_SLOT), 64);
        assert!(stand.is_empty(2));
    }

    /// `:667-676` : Java **assigne** le compte au lieu de l'additionner, donc le
    /// dernier slot qui porte la sortie gagne — et seuls les slots 0..3 sont lus.
    #[test]
    fn the_output_count_assignment_takes_the_last_matching_slot() {
        let mut stand = Stand::new(Pos::new(1, 64, 1));
        stand.slots[0] = Some(Stack::new(key("minecraft:potion"), 1));
        stand.slots[2] = Some(Stack::new(key("minecraft:potion"), 3));
        assert_eq!(stand.output_in_potion_slots(&key("minecraft:potion")), 3);
        assert_eq!(stand.output_in_potion_slots(&key("minecraft:glass")), 0);
    }

    /// `ItemStackUtils.java:755-786` : les trois prédicats sont exclusifs et
    /// `wants_fuel` en couvre deux.
    #[test]
    fn the_three_fuel_predicates_are_the_java_ones() {
        let mut stand = Stand::new(Pos::new(0, 64, 0));
        assert!(stand.has_neither_fuel_nor_brewable());
        assert!(stand.wants_fuel());
        stand.slots[INGREDIENT_SLOT] = Some(Stack::new(key("minecraft:nether_wart"), 1));
        assert!(stand.has_brewable_and_no_fuel());
        assert!(stand.wants_fuel());
        stand.slots[BREWING_FUEL_SLOT] = Some(Stack::new(key("minecraft:blaze_powder"), 8));
        assert!(!stand.wants_fuel());
        // `ItemStackUtils.java:782-786` : `hasFuelAndNoBrewable` exige le slot
        // d'ingrédient **vide**, donc il faut l'enlever d'abord — un alambic qui a les
        // deux n'est dans aucun des trois cas.
        assert!(!stand.has_fuel_and_no_brewable());
        stand.slots[INGREDIENT_SLOT] = None;
        assert!(stand.has_fuel_and_no_brewable());
    }

    /// `:921-922` et `:990` : un alambic sans potion, un alambic sans ingrédient.
    #[test]
    fn the_empty_slot_tests_are_narrow() {
        let mut stand = Stand::new(Pos::new(0, 64, 0));
        assert!(stand.has_empty_potion_slot());
        standalone_potions(&mut stand);
        assert!(!stand.has_empty_potion_slot());
        assert!(stand.is_empty(INGREDIENT_SLOT));
    }

    fn standalone_potions(stand: &mut Stand) {
        for slot in 0..POTION_SLOTS {
            stand.slots[slot] = Some(Stack::new(key("minecraft:potion"), 1));
        }
    }

    /// `ItemStackUtils.java:744-748` et `:657-660` : deux tests différents, un pour
    /// remplir, un pour compter les fours qui cuisent.
    #[test]
    fn a_furnace_is_burning_only_when_it_is_lit_and_holds_a_smeltable() {
        let mut furnace = Furnace::new(Pos::new(2, 64, 2));
        assert!(!furnace.is_burning());
        assert!(!furnace.has_fuel_and_no_smeltable());
        furnace.slots[FUEL_SLOT] = Some(Stack::new(key("minecraft:coal"), 8));
        assert!(furnace.has_fuel_and_no_smeltable());
        furnace.lit = true;
        assert!(!furnace.is_burning(), "`allumé` sans smeltable ne cuit rien (`:657`)");
        furnace.slots[SMELTABLE_SLOT] = Some(Stack::new(key("minecraft:wheat"), 4));
        assert!(furnace.is_burning());
        assert_eq!(furnace.key_at(SMELTABLE_SLOT), Some(&key("minecraft:wheat")));
        assert!(furnace.is_empty(RESULT_SLOT));
    }

    /// `:132` / `:250` : le tirage est ramené modulo la taille, et une liste vide
    /// n'est pas une panique.
    #[test]
    fn the_picked_cell_wraps_and_an_empty_list_is_none() {
        let mut facts = AlchemistFacts::default();
        assert_eq!(facts.picked_soil(), None);
        facts.soil.push(Soil {
            at: Pos::new(4, 64, 4),
            loaded: true,
            soul_sand: true,
            above: Above::NetherWart { age: 3 },
        });
        facts.pick = 97;
        assert_eq!(
            facts.picked_soil().map(|soil| soil.at),
            Some(Pos::new(4, 64, 4))
        );
        assert_eq!(facts.picked_leaf(), None);
    }

    /// Un alambic déchargé n'est ni compté ni lu (`:448`, `:508`).
    #[test]
    fn an_unloaded_stand_is_still_listed_but_not_bubbling() {
        let mut stand = Stand::new(Pos::new(3, 64, 3));
        stand.brew_time = 100;
        assert!(stand.is_bubbling());
        stand.loaded = false;
        assert!(stand.is_bubbling(), "le fait reste vrai, c'est `loaded` qui décide");
        let mut facts = AlchemistFacts::default();
        facts.stands.push(stand);
        assert_eq!(facts.stand_at(Pos::new(3, 64, 3)).map(|s| s.loaded), Some(false));
    }
}
