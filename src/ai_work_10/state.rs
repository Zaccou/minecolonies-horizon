//! Les états que ces trois éleveurs ajoutent à `AIWorkerState`, leurs refus nommés, et
//! ce que le citoyen tient en main.
//!
//! Java référence : `api/entity/ai/statemachine/states/AIWorkerState.java` — une seule
//! énumération pour **tous** les métiers du mod (127 valeurs). Le port garde cette
//! forme, [`HerdState`], au lieu d'une énumération par métier : les tables Java
//! elles-mêmes sont écrites contre l'énumération unique, et `EntityAIWorkShepherd.java:
//! 81` rend `SHEPHERD_SHEAR` quand `EntityAIWorkStablemaster.java:241` rend
//! `HERDER_TRAIN`.
//!
//! **Les neuf états de cette tranche, et à qui ils appartiennent.**
//!
//! | État | Ligne | Propriétaire |
//! | --- | --- | --- |
//! | `Base(WorkerState)` | `AbstractEntityAIBasic.java` | `crate::ai` — traversé, jamais décidé ici |
//! | `HERDER_BREED` | `:357` | `crate::ai_work_09::herder` (`:144`) — traversé |
//! | `HERDER_BUTCHER` | `:362` | idem (`:145`) |
//! | `HERDER_PICKUP` | `:367` | idem (`:146`) |
//! | `HERDER_FEED` | `:372` | idem (`:147`) |
//! | `HERDER_TRAIN` | `:381` | **ce module**, `EntityAIWorkStablemaster.java:102` |
//! | `HERDER_READY_FOR_COMBAT` | `:386` | **ce module**, `:103` |
//! | `HERDER_GATHER_MOUNTS` | `:391` | **ce module**, `:104` |
//! | `SHEPHERD_SHEAR` | `:414` | **ce module**, `EntityAIWorkShepherd.java:50` |
//!
//! Les quatre états d'`AbstractEntityAIHerder` sont **nommés** ici parce que cette
//! machine doit pouvoir s'y trouver — la décision de base les rend
//! (`AbstractEntityAIHerder.java:232`, `:236`, `:240`, `:244`), donc `HerdState` doit
//! les porter. Leur **règle** reste dans `crate::ai_work_09::herder` : ce module n'en
//! écrit pas une deuxième implémentation, il les traverse.
//!
//! **`Base(...)` et pas une copie des états de base.** Les huit états généraux
//! (`IDLE`, `START_WORKING`, `PREPARING`, `DECIDE`, …) appartiennent à
//! `crate::ai::state::WorkerState` ; les réécrire ici en ferait une deuxième
//! définition. [`HerdState::Base`] les enveloppe et [`HerdState::okay_to_eat`] leur
//! délègue la question.

use core::fmt;

use crate::ai::state::WorkerState;

/// Un état de la table d'un de ces trois éleveurs.
///
/// Java référence : `AIWorkerState.java` — la ligne entre parenthèses est la
/// déclaration de la constante, et c'est elle que [`HerdState::java_declaration`]
/// rapporte.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
// exhaustive: the eight base states these tables can enter plus the eight they name
pub enum HerdState {
    /// Un état de `crate::ai::state::WorkerState` — `IDLE`, `START_WORKING`,
    /// `PREPARING`, `DECIDE`, `INVENTORY_FULL`, `NEEDS_ITEM`, `PAUSED`,
    /// `GATHERING_REQUIRED_MATERIALS`.
    Base(WorkerState),
    /// `HERDER_BREED` (`AIWorkerState.java:357`) — `AbstractEntityAIHerder.java:240`.
    HerderBreed,
    /// `HERDER_BUTCHER` (`:362`) — `AbstractEntityAIHerder.java:236`.
    HerderButcher,
    /// `HERDER_PICKUP` (`:367`) — `AbstractEntityAIHerder.java:232`.
    HerderPickup,
    /// `HERDER_FEED` (`:372`) — `AbstractEntityAIHerder.java:244`.
    HerderFeed,
    /// `SHEPHERD_SHEAR` (`:414`) — la seule cible qu'ajoute `EntityAIWorkShepherd.java:50`.
    ShepherdShear,
    /// `HERDER_TRAIN` (`:381`) — `EntityAIWorkStablemaster.java:102`.
    HerderTrain,
    /// `HERDER_READY_FOR_COMBAT` (`:386`) — `EntityAIWorkStablemaster.java:103`.
    HerderReadyForCombat,
    /// `HERDER_GATHER_MOUNTS` (`:391`) — `EntityAIWorkStablemaster.java:104`.
    HerderGatherMounts,
}

impl HerdState {
    /// Les huit états de métier que cette tranche nomme, dans l'ordre d'`AIWorkerState`.
    ///
    /// `Base(...)` n'y est pas : c'est une famille de dix-huit valeurs, celles de
    /// `crate::ai::state::WorkerState`, et l'énumérer ici la redéfinirait.
    pub const ALL: [HerdState; 8] = [
        HerdState::HerderBreed,
        HerdState::HerderButcher,
        HerdState::HerderPickup,
        HerdState::HerderFeed,
        HerdState::HerderTrain,
        HerdState::HerderReadyForCombat,
        HerdState::HerderGatherMounts,
        HerdState::ShepherdShear,
    ];

    /// L'identifiant Java, ce qu'affiche l'historique d'état et le nametag
    /// (`AbstractEntityAIBasic.java:475`).
    pub fn java_name(self) -> &'static str {
        match self {
            HerdState::Base(state) => state.java_name(),
            HerdState::HerderBreed => "HERDER_BREED",
            HerdState::HerderButcher => "HERDER_BUTCHER",
            HerdState::HerderPickup => "HERDER_PICKUP",
            HerdState::HerderFeed => "HERDER_FEED",
            HerdState::ShepherdShear => "SHEPHERD_SHEAR",
            HerdState::HerderTrain => "HERDER_TRAIN",
            HerdState::HerderReadyForCombat => "HERDER_READY_FOR_COMBAT",
            HerdState::HerderGatherMounts => "HERDER_GATHER_MOUNTS",
        }
    }

    /// La ligne de `AIWorkerState.java` qui déclare la constante.
    pub fn java_declaration(self) -> &'static str {
        match self {
            HerdState::Base(state) => match state {
                WorkerState::Idle => "AIWorkerState.java:15",
                WorkerState::Init => "AIWorkerState.java:19",
                WorkerState::InventoryFull => "AIWorkerState.java:23",
                WorkerState::Preparing => "AIWorkerState.java:27",
                WorkerState::StartWorking => "AIWorkerState.java:31",
                WorkerState::NeedsItem => "AIWorkerState.java:35",
                WorkerState::StartBuilding => "AIWorkerState.java:39",
                WorkerState::MineBlock => "AIWorkerState.java:43",
                WorkerState::LoadStructure => "AIWorkerState.java:47",
                WorkerState::BuildingStep => "AIWorkerState.java:51",
                WorkerState::CompleteBuild => "AIWorkerState.java:55",
                WorkerState::Decide => "AIWorkerState.java:59",
                WorkerState::Paused => "AIWorkerState.java:63",
                WorkerState::GatheringRequiredMaterials => "AIWorkerState.java:310",
                WorkerState::PickUp => "AIWorkerState.java:141",
                WorkerState::ComposterFill => "AIWorkerState.java:423",
                WorkerState::ComposterHarvest => "AIWorkerState.java:428",
                WorkerState::GetMaterials => "AIWorkerState.java:433",
            },
            HerdState::HerderBreed => "AIWorkerState.java:357",
            HerdState::HerderButcher => "AIWorkerState.java:362",
            HerdState::HerderPickup => "AIWorkerState.java:367",
            HerdState::HerderFeed => "AIWorkerState.java:372",
            HerdState::ShepherdShear => "AIWorkerState.java:414",
            HerdState::HerderTrain => "AIWorkerState.java:381",
            HerdState::HerderReadyForCombat => "AIWorkerState.java:386",
            HerdState::HerderGatherMounts => "AIWorkerState.java:391",
        }
    }

    /// `AIWorkerState.isOkayToEat` — le drapeau que lit
    /// `AbstractAISkeleton.canBeInterrupted()` (`AbstractAISkeleton.java:126-131`).
    ///
    /// Un état de base délègue à `crate::ai::state::WorkerState::okay_to_eat`, qui
    /// porte déjà les dix-huit valeurs ; les huit états de métier portent celle que
    /// leur constructeur d'énumération reçoit.
    pub fn okay_to_eat(self) -> bool {
        match self {
            HerdState::Base(state) => state.okay_to_eat(),
            // `:367` HERDER_PICKUP(true), `:381` HERDER_TRAIN(true),
            // `:386` HERDER_READY_FOR_COMBAT(true), `:391` HERDER_GATHER_MOUNTS(true).
            HerdState::HerderPickup
            | HerdState::HerderTrain
            | HerdState::HerderReadyForCombat
            | HerdState::HerderGatherMounts => true,
            // `:357` HERDER_BREED(false), `:362` HERDER_BUTCHER(false),
            // `:372` HERDER_FEED(false), `:414` SHEPHERD_SHEAR(false).
            HerdState::HerderBreed
            | HerdState::HerderButcher
            | HerdState::HerderFeed
            | HerdState::ShepherdShear => false,
        }
    }

    /// L'état de base que cet état est, quand il en est un.
    pub fn base(self) -> Option<WorkerState> {
        match self {
            HerdState::Base(state) => Some(state),
            _ => None,
        }
    }

    /// Vrai quand cet état appartient à une autre table que celle de ce module : la
    /// table générique de `crate::ai`, ou celle d'`AbstractEntityAIHerder` portée par
    /// `crate::ai_work_09::herder`. Cette machine ne décide rien pour eux — elle les
    /// traverse et attend que la table propriétaire rende `DECIDE`.
    pub fn is_traversed(self) -> bool {
        !matches!(
            self,
            HerdState::ShepherdShear
                | HerdState::HerderTrain
                | HerdState::HerderReadyForCombat
                | HerdState::HerderGatherMounts
        )
    }

    /// Vrai quand cet état appartient **à un autre métier** que `trade` : la machine du
    /// berger peut porter la cible d'une écurie sans que ce soit un but à elle.
    pub fn belongs_to_other_trade(self, trade: super::HerdTrade) -> bool {
        match self {
            HerdState::ShepherdShear => trade != super::HerdTrade::Shepherd,
            HerdState::HerderTrain
            | HerdState::HerderReadyForCombat
            | HerdState::HerderGatherMounts => trade != super::HerdTrade::Stablemaster,
            _ => false,
        }
    }
}

impl fmt::Display for HerdState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.java_name())
    }
}

/// Les cinq réponses que peut donner `AbstractEntityAIHerder.decideWhatToDo()`, et
/// rien d'autre.
///
/// Java référence : `:232` (`HERDER_PICKUP`), `:236` (`HERDER_BUTCHER`), `:240`
/// (`HERDER_BREED`), `:244` (`HERDER_FEED`), `:248` (`START_WORKING`). La méthode a
/// exactement ces cinq sorties — le corps est une boucle sur les modules d'élevage qui
/// `continue` (`:211-214`) puis ce `return` final —, donc le type du port est cet
/// ensemble et pas un `WorkerState` général : une valeur impossible ne doit pas être
/// représentable.
///
/// C'est **l'entrée** de la surcharge qu'écrivent les trois sous-classes : `final IAIState
/// result = super.decideWhatToDo()` (`EntityAIWorkShepherd.java:75`,
/// `EntityAIWorkStablemaster.java:234`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: the five returns of AbstractEntityAIHerder.decideWhatToDo
pub enum HerdChoice {
    /// `:232` — quelque chose traîne dans l'enclos.
    Pickup,
    /// `:236` — le troupeau est au-dessus du maximum.
    Butcher,
    /// `:240` — deux animaux accouplables, de quoi les nourrir, pas de délai en cours.
    Breed,
    /// `:244` — le tirage de nourrissage à 10 % est passé.
    Feed,
    /// `:248` — rien de tout ça : aller travailler.
    StartWorking,
}

impl HerdChoice {
    /// L'état dans lequel cette réponse met la machine.
    pub fn state(self) -> HerdState {
        match self {
            HerdChoice::Pickup => HerdState::HerderPickup,
            HerdChoice::Butcher => HerdState::HerderButcher,
            HerdChoice::Breed => HerdState::HerderBreed,
            HerdChoice::Feed => HerdState::HerderFeed,
            HerdChoice::StartWorking => HerdState::Base(WorkerState::StartWorking),
        }
    }

    /// La ligne qui produit cette réponse.
    pub fn java(self) -> &'static str {
        match self {
            HerdChoice::Pickup => "AbstractEntityAIHerder.java:232",
            HerdChoice::Butcher => "AbstractEntityAIHerder.java:236",
            HerdChoice::Breed => "AbstractEntityAIHerder.java:240",
            HerdChoice::Feed => "AbstractEntityAIHerder.java:244",
            HerdChoice::StartWorking => "AbstractEntityAIHerder.java:248",
        }
    }

    /// Java's name for the state it leads to.
    pub fn java_name(self) -> &'static str {
        self.state().java_name()
    }
}

/// Ce que le citoyen tient en main, tel que ces trois métiers le posent.
///
/// Java référence : les `setItemInHand(MAIN_HAND, …)` de la hiérarchie —
/// `EntityAIWorkShepherd.java:84` (vider), `:120` (`equipTool(shears)`),
/// `AbstractEntityAIHerder.java:466` (`equipItem(breeding items)`, `:524`),
/// `:560` (`removeHeldItem` après nourrissage), `EntityAIWorkStablemaster.java:461`
/// (la pile extraite de la hutte), `:687` (la laisse en main secondaire),
/// `:721` (la laisse rangée).
///
/// **Pourquoi un type et pas un `ItemStack`.** Le port n'a pas de pile d'objets : un
/// `ItemStack` vivant, c'est l'inventaire du citoyen, et l'inventaire est un fait que
/// l'appelant remplit (`facts.inventory`). La mémoire retient donc *quelle* catégorie
/// d'objet ce module a décidé de tenir ; c'est ce que le test peut vérifier et ce dont
/// l'appelant a besoin pour appliquer la décision.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
// exhaustive: the seven categories these tables set in hand
pub enum Hand {
    /// `ItemStack.EMPTY`.
    #[default]
    Empty,
    /// `ModEquipmentTypes.shears` — `EntityAIWorkShepherd.java:120`.
    Shears,
    /// `ModEquipmentTypes.axe` — `AbstractEntityAIHerder.java:344`, l'arme d'abattage
    /// que les trois métiers passent en main avant de frapper.
    Axe,
    /// Un des `module.getBreedingItems()` — `AbstractEntityAIHerder.java:466`.
    BreedingItem,
    /// `ModTags.feed` — `EntityAIWorkStablemaster.java:431`.
    Feed,
    /// `ModTags.leather` — la même ligne, l'autre branche.
    Leather,
    /// `Items.LEAD` — `EntityAIWorkStablemaster.java:687`.
    Lead,
}

impl Hand {
    /// Java's identifier for the item, for a log line or a test failure.
    pub fn java_name(self) -> &'static str {
        match self {
            Hand::Empty => "minecraft:air",
            Hand::Shears => "minecraft:shears",
            Hand::Axe => "axe",
            Hand::BreedingItem => "breeding items",
            Hand::Feed => "ModTags.feed",
            Hand::Leather => "ModTags.leather",
            Hand::Lead => "minecraft:lead",
        }
    }

    /// `CitizenItemUtils.removeHeldItem` (`:560`) remet la pile à l'inventaire : la
    /// main redevient vide.
    pub fn cleared(self) -> Hand {
        Hand::Empty
    }
}

/// Pourquoi un de ces trois buts n'a rien pu faire, nommé.
///
/// Java n'a pas de tel type : chaque branche qui ne peut pas avancer fait
/// `return getState()` ou `return DECIDE`, et la raison reste dans la forme du `if`.
/// Un joueur qui demande « pourquoi mon palefrenier tourne en rond » en a besoin, et un
/// test qui vérifie une suite d'états a besoin de savoir *quelle* branche a tenu — donc
/// chaque variante porte la ligne qui la décide.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: one variant per branch of these three classes that stops without acting
pub enum TradeRefusal {
    /// La scène ne porte pas les faits de ce métier — une erreur d'appelant, pas un
    /// état Java. Nommée pour qu'elle ne puisse jamais passer pour « rien à faire ».
    WrongTradeFacts,
    /// `super.decideWhatToDo()` n'a pas encore répondu : cette surcharge décide *sur*
    /// sa réponse (`EntityAIWorkShepherd.java:75`), donc sans elle il n'y a rien à
    /// surcharger.
    BaseDecisionMissing,
    /// `findShearableSheep()` a rendu `null` (`EntityAIWorkShepherd.java:115-118`).
    NoShearableSheep,
    /// `equipTool(MAIN_HAND, shears)` a rendu faux (`EntityAIWorkShepherd.java:120-123`).
    NoShears,
    /// `walkingToAnimal` est encore vrai : il marche (`EntityAIWorkShepherd.java:127-130`).
    NotAtAnimal,
    /// Le rayon de l'enclos est vide (`AbstractEntityAIHerder.java:349-352`).
    NoAnimals,
    /// La carcasse visée a quitté l'enclos entre deux ticks.
    AnimalGone,
    /// La main principale est vide : `butcherAnimal` ne frappe pas
    /// (`EntityAIWorkRabbitHerder.java:57`).
    NoWeaponInHand,
    /// Le tirage a raté : `nextInt(bound) <= 1` est faux
    /// (`EntityAIWorkRabbitHerder.java:61`).
    HitMissed,
    /// `building.getSetting(SHEARING)` est faux : la surcharge ne tond pas
    /// (`EntityAIWorkShepherd.java:79`).
    ShearingOff,
    /// `building.getModule(BuildingModules.STABLEMASTER_HERDING)` est nul
    /// (`EntityAIWorkStablemaster.java:400-404`).
    NoHerdingModule,
    /// `current >= limit`, la limite étant `niveau * 2` (`:270`, `:299-304`).
    MountLimitReached,
    /// Aucun candidat convertible dans l'enclos (`:306-338`).
    NoTrainableMount,
    /// L'étalon suivi a quitté l'enclos (`:273`, `:566-576`).
    MountGone,
    /// `CavalryHorseEntity.createFromVanilla` a rendu `null` (`:281-285`).
    MountNotConvertible,
    /// `walkToSafePos` n'est pas arrivé (`:273`, `:349`, `:620`).
    NotAtMount,
    /// L'étalon a pris un cavalier pendant la marche (`:352-355`).
    MountRidden,
    /// Aucun monture ne demande à être préparée (`:406-421`).
    NoMountToReady,
    /// Ni l'inventaire du citoyen ni la hutte n'ont d'objet de nourrissage
    /// (`:438-451`, `:456-458`) : une requête est partie.
    NoFeedItem,
    /// Même chose pour le cuir (`:431`, l'autre branche).
    NoReadyItem,
    /// `checkForToolOrWeapon(lead)` est vrai (`:578-587`) : la laisse manque.
    NoLead,
    /// `building.getNextStallPosition()` a rendu `null` : pas de stalle étiquetée
    /// (`BuildingStable.java:128-131`).
    NoStall,
    /// La marche vers la stalle n'est pas finie (`:591-596`).
    NotAtStall,
    /// `wanderingHorses` est vide (`:635-638`).
    NoRoundupTarget,
    /// `attachHorse` a rendu faux : la bête n'est plus là (`:625-632`).
    HorseLostLeash,
    /// `walkWithProxy` n'est pas arrivé sur l'item (`AbstractEntityAIHerder.java:579`).
    NotAtItem,
}

impl TradeRefusal {
    /// La ligne de Java qui décide ce refus, pour un journal ou un échec de test.
    pub fn java(self) -> &'static str {
        match self {
            TradeRefusal::WrongTradeFacts => "ai_work_10: scene.trade (erreur d'appelant)",
            TradeRefusal::BaseDecisionMissing => "EntityAIWorkShepherd.java:75",
            TradeRefusal::NoShearableSheep => "EntityAIWorkShepherd.java:115-118",
            TradeRefusal::NoShears => "EntityAIWorkShepherd.java:120-123",
            TradeRefusal::NotAtAnimal => "EntityAIWorkShepherd.java:127-130",
            TradeRefusal::NoAnimals => "AbstractEntityAIHerder.java:349-352",
            TradeRefusal::AnimalGone => "EntityAIWorkRabbitHerder.java:57",
            TradeRefusal::NoWeaponInHand => "EntityAIWorkRabbitHerder.java:57",
            TradeRefusal::HitMissed => "EntityAIWorkRabbitHerder.java:61",
            TradeRefusal::ShearingOff => "EntityAIWorkShepherd.java:79",
            TradeRefusal::NoHerdingModule => "EntityAIWorkStablemaster.java:400-404",
            TradeRefusal::MountLimitReached => "EntityAIWorkStablemaster.java:299-304",
            TradeRefusal::NoTrainableMount => "EntityAIWorkStablemaster.java:337-338",
            TradeRefusal::MountGone => "EntityAIWorkStablemaster.java:566-576",
            TradeRefusal::MountNotConvertible => "EntityAIWorkStablemaster.java:281-285",
            TradeRefusal::NotAtMount => "EntityAIWorkStablemaster.java:273",
            TradeRefusal::MountRidden => "EntityAIWorkStablemaster.java:352-355",
            TradeRefusal::NoMountToReady => "EntityAIWorkStablemaster.java:406-421",
            TradeRefusal::NoFeedItem => "EntityAIWorkStablemaster.java:504-511",
            TradeRefusal::NoReadyItem => "EntityAIWorkStablemaster.java:504-511",
            TradeRefusal::NoLead => "EntityAIWorkStablemaster.java:578-587",
            TradeRefusal::NoStall => "BuildingStable.java:128-131",
            TradeRefusal::NotAtStall => "EntityAIWorkStablemaster.java:591-596",
            TradeRefusal::NoRoundupTarget => "EntityAIWorkStablemaster.java:635-638",
            TradeRefusal::HorseLostLeash => "EntityAIWorkStablemaster.java:625-632",
            TradeRefusal::NotAtItem => "AbstractEntityAIHerder.java:579",
        }
    }

    /// Une clé stable pour un message que le joueur lira.
    pub fn key(self) -> &'static str {
        match self {
            TradeRefusal::WrongTradeFacts => "herd.refusal.wrong_trade_facts",
            TradeRefusal::BaseDecisionMissing => "herd.refusal.base_decision_missing",
            TradeRefusal::NoShearableSheep => "herd.refusal.no_shearable_sheep",
            TradeRefusal::NoShears => "herd.refusal.no_shears",
            TradeRefusal::NotAtAnimal => "herd.refusal.not_at_animal",
            TradeRefusal::NoAnimals => "herd.refusal.no_animals",
            TradeRefusal::AnimalGone => "herd.refusal.animal_gone",
            TradeRefusal::NoWeaponInHand => "herd.refusal.no_weapon_in_hand",
            TradeRefusal::HitMissed => "herd.refusal.hit_missed",
            TradeRefusal::ShearingOff => "herd.refusal.shearing_off",
            TradeRefusal::NoHerdingModule => "herd.refusal.no_herding_module",
            TradeRefusal::MountLimitReached => "herd.refusal.mount_limit_reached",
            TradeRefusal::NoTrainableMount => "herd.refusal.no_trainable_mount",
            TradeRefusal::MountGone => "herd.refusal.mount_gone",
            TradeRefusal::MountNotConvertible => "herd.refusal.mount_not_convertible",
            TradeRefusal::NotAtMount => "herd.refusal.not_at_mount",
            TradeRefusal::MountRidden => "herd.refusal.mount_ridden",
            TradeRefusal::NoMountToReady => "herd.refusal.no_mount_to_ready",
            TradeRefusal::NoFeedItem => "herd.refusal.no_feed_item",
            TradeRefusal::NoReadyItem => "herd.refusal.no_ready_item",
            TradeRefusal::NoLead => "herd.refusal.no_lead",
            TradeRefusal::NoStall => "herd.refusal.no_stall",
            TradeRefusal::NotAtStall => "herd.refusal.not_at_stall",
            TradeRefusal::NoRoundupTarget => "herd.refusal.no_roundup_target",
            TradeRefusal::HorseLostLeash => "herd.refusal.horse_lost_leash",
            TradeRefusal::NotAtItem => "herd.refusal.not_at_item",
        }
    }

    /// Toutes les variantes, pour le test qui vérifie les clés.
    pub const ALL: [TradeRefusal; 26] = [
        TradeRefusal::WrongTradeFacts,
        TradeRefusal::BaseDecisionMissing,
        TradeRefusal::NoShearableSheep,
        TradeRefusal::NoShears,
        TradeRefusal::NotAtAnimal,
        TradeRefusal::NoAnimals,
        TradeRefusal::AnimalGone,
        TradeRefusal::NoWeaponInHand,
        TradeRefusal::HitMissed,
        TradeRefusal::ShearingOff,
        TradeRefusal::NoHerdingModule,
        TradeRefusal::MountLimitReached,
        TradeRefusal::NoTrainableMount,
        TradeRefusal::MountGone,
        TradeRefusal::MountNotConvertible,
        TradeRefusal::NotAtMount,
        TradeRefusal::MountRidden,
        TradeRefusal::NoMountToReady,
        TradeRefusal::NoFeedItem,
        TradeRefusal::NoReadyItem,
        TradeRefusal::NoLead,
        TradeRefusal::NoStall,
        TradeRefusal::NotAtStall,
        TradeRefusal::NoRoundupTarget,
        TradeRefusal::HorseLostLeash,
        TradeRefusal::NotAtItem,
    ];
}

impl fmt::Display for TradeRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key())
    }
}
