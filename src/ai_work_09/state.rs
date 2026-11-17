//! Les états que ces trois buts ajoutent à `AIWorkerState`, leur drapeau
//! `isOkayToEat`, et les refus nommés.
//!
//! Java référence : `api/entity/ai/statemachine/states/AIWorkerState.java`, une seule
//! énumération pour **tous** les métiers du mod (127 valeurs). Le port garde cette
//! forme — une énumération, [`WorkState`], avec les onze états de ces trois métiers
//! plus les huit états de base auxquels ils rendent la main — et pas une énumération
//! par métier : les tables Java elles-mêmes sont écrites contre l'énumération unique.
//! `EntityAIWorkPlanter.java:108` rend `IDLE` et `AbstractEntityAIHerder.java:483`
//! rend `IDLE` aussi, tous les deux depuis une table de métier.
//!
//! **Pourquoi `Base(...)`.** `crate::ai::state::WorkerState` nomme déjà les dix-huit
//! états de base de ce port, et cette énumération appartient à `crate::ai`, pas ici
//! (ce module n'écrit que dans `src/ai_work_09`). Un état de base est donc
//! [`WorkState::Base`] enveloppant le `WorkerState` que le module `ai` connaît déjà :
//! aucun état n'est écrit deux fois.
//!
//! **Ce qui n'est pas ici.** Le comportement d'un état de base (vider l'inventaire,
//! attendre une requête, se mettre en pause) est `crate::ai::worker::register_base`.
//! Une table de métier entre et sort de ces états et le dit, mais les réimplémenter
//! serait la deuxième copie d'une table qui existe déjà et qui a ses tests.

use core::fmt;

use crate::ai::state::WorkerState;

/// Un état de la table d'un de ces trois métiers : un état de base rendu à
/// `crate::ai`, ou un état que ce module nomme.
///
/// Java référence : `AIWorkerState.java` — la ligne entre parenthèses est la
/// déclaration de la constante, et c'est cette ligne que [`WorkState::java_declaration`]
/// rapporte.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
// exhaustive: the eight base states these tables enter plus the eleven they own
pub enum WorkState {
    /// Un état de `crate::ai::state::WorkerState` — `IDLE`, `START_WORKING`,
    /// `PREPARING`, `DECIDE`, `INVENTORY_FULL`, `NEEDS_ITEM`, `PAUSED`,
    /// `GATHERING_REQUIRED_MATERIALS`.
    Base(WorkerState),
    /// `PLANTATION_PICK_FIELD` (`AIWorkerState.java:629`) — choisir un champ.
    PlantationPickField,
    /// `PLANTATION_MOVE_TO_FIELD` (`:630`) — marcher jusqu'au champ.
    PlantationMoveToField,
    /// `PLANTATION_DECIDE_FIELD_WORK` (`:631`) — demander au module la prochaine
    /// position et l'action.
    PlantationDecideFieldWork,
    /// `PLANTATION_WORK_FIELD` (`:632`) — exécuter l'action sur la position.
    PlantationWorkField,
    /// `PLANTATION_RETURN_TO_BUILDING` (`:633`) — rentrer chercher ce qui manque.
    PlantationReturnToBuilding,
    /// `HERDER_BREED` (`:357`) — accoupler deux animaux.
    HerderBreed,
    /// `HERDER_BUTCHER` (`:362`) — abattre un animal.
    HerderButcher,
    /// `HERDER_PICKUP` (`:367`) — ramasser ce qui traîne dans l'enclos.
    HerderPickup,
    /// `HERDER_FEED` (`:372`) — nourrir un animal.
    HerderFeed,
    /// `COWBOY_MILK` (`:400`) — traire une vache ou une chèvre.
    CowboyMilk,
    /// `COWBOY_STEW` (`:405`) — traire une vache-champignon.
    CowboyStew,
}

impl WorkState {
    /// Tous les états que ce module nomme, états de base d'abord dans l'ordre de
    /// `WorkerState`, puis les états de métier dans l'ordre d'`AIWorkerState`.
    pub const ALL: [WorkState; 19] = [
        WorkState::Base(WorkerState::Idle),
        WorkState::Base(WorkerState::StartWorking),
        WorkState::Base(WorkerState::Preparing),
        WorkState::Base(WorkerState::Decide),
        WorkState::Base(WorkerState::InventoryFull),
        WorkState::Base(WorkerState::NeedsItem),
        WorkState::Base(WorkerState::GatheringRequiredMaterials),
        WorkState::Base(WorkerState::Paused),
        WorkState::PlantationPickField,
        WorkState::PlantationMoveToField,
        WorkState::PlantationDecideFieldWork,
        WorkState::PlantationWorkField,
        WorkState::PlantationReturnToBuilding,
        WorkState::HerderBreed,
        WorkState::HerderButcher,
        WorkState::HerderPickup,
        WorkState::HerderFeed,
        WorkState::CowboyMilk,
        WorkState::CowboyStew,
    ];

    /// L'identifiant Java (`AIWorkerState.java`), ce qu'affiche l'historique d'état
    /// et le nametag du citoyen (`AbstractEntityAIBasic.java:475`).
    pub fn java_name(self) -> &'static str {
        match self {
            WorkState::Base(state) => state.java_name(),
            WorkState::PlantationPickField => "PLANTATION_PICK_FIELD",
            WorkState::PlantationMoveToField => "PLANTATION_MOVE_TO_FIELD",
            WorkState::PlantationDecideFieldWork => "PLANTATION_DECIDE_FIELD_WORK",
            WorkState::PlantationWorkField => "PLANTATION_WORK_FIELD",
            WorkState::PlantationReturnToBuilding => "PLANTATION_RETURN_TO_BUILDING",
            WorkState::HerderBreed => "HERDER_BREED",
            WorkState::HerderButcher => "HERDER_BUTCHER",
            WorkState::HerderPickup => "HERDER_PICKUP",
            WorkState::HerderFeed => "HERDER_FEED",
            WorkState::CowboyMilk => "COWBOY_MILK",
            WorkState::CowboyStew => "COWBOY_STEW",
        }
    }

    /// Le bloc qui déclare cet état, `AIWorkerState.java:<ligne>`.
    pub fn java_declaration(self) -> &'static str {
        match self {
            WorkState::Base(_) => "crate::ai::state::WorkerState (voir AIWorkerState.java)",
            WorkState::PlantationPickField => "AIWorkerState.java:629",
            WorkState::PlantationMoveToField => "AIWorkerState.java:630",
            WorkState::PlantationDecideFieldWork => "AIWorkerState.java:631",
            WorkState::PlantationWorkField => "AIWorkerState.java:632",
            WorkState::PlantationReturnToBuilding => "AIWorkerState.java:633",
            WorkState::HerderBreed => "AIWorkerState.java:357",
            WorkState::HerderButcher => "AIWorkerState.java:362",
            WorkState::HerderPickup => "AIWorkerState.java:367",
            WorkState::HerderFeed => "AIWorkerState.java:372",
            WorkState::CowboyMilk => "AIWorkerState.java:400",
            WorkState::CowboyStew => "AIWorkerState.java:405",
        }
    }

    /// `AIWorkerState.isOkayToEat` — le drapeau que `AbstractAISkeleton.canBeInterrupted`
    /// rend (`AbstractAISkeleton.java:126-131`), et que
    /// `AbstractEntityAIBasic.inventoryNeedsDump` lit (`AbstractEntityAIBasic.java:411`).
    ///
    /// Deux des onze états de métier sont `true` : le planteur en pleine action de
    /// champ, et l'éleveur qui ramasse ce qui traîne. Les neuf autres sont `false` :
    /// un accouplement, un abattage, une traite ou un déplacement de champ en cours
    /// ne s'interrompent pas.
    pub fn okay_to_eat(self) -> bool {
        match self {
            WorkState::Base(state) => state.okay_to_eat(),
            // :632, :367
            WorkState::PlantationWorkField | WorkState::HerderPickup => true,
            // :629, :630, :631, :633, :357, :362, :372, :400, :405
            WorkState::PlantationPickField
            | WorkState::PlantationMoveToField
            | WorkState::PlantationDecideFieldWork
            | WorkState::PlantationReturnToBuilding
            | WorkState::HerderBreed
            | WorkState::HerderButcher
            | WorkState::HerderFeed
            | WorkState::CowboyMilk
            | WorkState::CowboyStew => false,
        }
    }

    /// Le `WorkerState` derrière un état de base, `None` pour un état de métier.
    pub fn worker(self) -> Option<WorkerState> {
        match self {
            WorkState::Base(state) => Some(state),
            _ => None,
        }
    }

    /// Cet état appartient-il à `trade` ?
    ///
    /// Le cowboy porte les deux états de traite **et** ceux de l'éleveur : sa classe
    /// Java étend `AbstractEntityAIHerder` (`EntityAIWorkCowboy.java:39`) et sa table
    /// est la somme des deux constructeurs (`AbstractEntityAIHerder.java:139-148`
    /// puis `EntityAIWorkCowboy.java:69-72`).
    pub fn belongs_to(self, trade: super::WorkTrade) -> bool {
        match trade {
            super::WorkTrade::Planter => matches!(
                self,
                WorkState::PlantationPickField
                    | WorkState::PlantationMoveToField
                    | WorkState::PlantationDecideFieldWork
                    | WorkState::PlantationWorkField
                    | WorkState::PlantationReturnToBuilding
            ),
            super::WorkTrade::ChickenHerder => matches!(
                self,
                WorkState::HerderBreed
                    | WorkState::HerderButcher
                    | WorkState::HerderPickup
                    | WorkState::HerderFeed
            ),
            super::WorkTrade::Cowboy => matches!(
                self,
                WorkState::HerderBreed
                    | WorkState::HerderButcher
                    | WorkState::HerderPickup
                    | WorkState::HerderFeed
                    | WorkState::CowboyMilk
                    | WorkState::CowboyStew
            ),
        }
    }
}

impl fmt::Display for WorkState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.java_name())
    }
}

/// Pourquoi un de ces trois buts n'a rien pu faire, nommé.
///
/// Java n'a pas de tel type : chaque branche qui ne peut pas avancer fait
/// `return getState()` ou `return IDLE` et la raison reste dans la forme du `if`. Un
/// joueur qui demande « pourquoi mon planteur reste planté là » en a besoin, et un
/// test qui vérifie une suite d'états a besoin de savoir *quelle* branche a tenu —
/// donc chaque variante porte la ligne qui la décide.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: one variant per branch of these three classes that stops without acting
pub enum WorkRefusal {
    /// La scène ne porte pas les faits de ce métier — une erreur d'appelant, pas un
    /// état Java. Nommée pour qu'elle ne puisse jamais passer pour « rien à faire ».
    NoFactsForTrade,
    /// `pickField` : le bâtiment est nul ou de niveau 0 (`EntityAIWorkPlanter.java:
    /// 105-109`).
    HutNotBuilt,
    /// `pickField` : le module de champs n'a aucun champ — l'interaction
    /// `NO_FREE_FIELDS` part et le statut passe à `STUCK` (`:112-120`).
    NoFreeField,
    /// `pickField` : `getExtensionToWorkOn()` a répondu `null`, le planteur rend
    /// `IDLE` (`:141`).
    NoFieldToWork,
    /// `getCurrentField()` a répondu `null` — la position courante n'est pas un
    /// `PlantationField` (`:162-165`, `:182-186`).
    FieldNotHeld,
    /// `getNextWorkingPosition(world)` a répondu `null` : le champ est fini, il est
    /// rendu au module (`:190-194`).
    NoWorkPosition,
    /// `activeModuleResult` est nul alors que `WORK_FIELD` tourne : Java lève un
    /// `NullPointerException` à `:208`.
    NoActiveResult,
    /// `decideFieldWork` n'a pas répondu, alors que `getNextWorkingPosition` avait
    /// donné une position (`:196-197`).
    ModuleAnsweredNothing,
    /// `walkToSafePos` n'a pas répondu : il marche encore (`:167-170`, `:209-212`).
    NotAtField,
    /// `walkToBuilding()` a répondu faux (`:265-268`).
    NotAtHut,
    /// `checkIfItemsUnavailable` a répondu vrai pour `PLANT` : il manque la graine,
    /// la requête part (`:308-311`).
    NoPlantItem,
    /// La même chose pour `BONEMEAL` (`:341-347`).
    NoBonemeal,
    /// `world.setBlockAndUpdate` a répondu faux : la plantation n'est pas finie
    /// (`:317-324`).
    PlantBlocked,
    /// `holdEfficientTool` a répondu faux : il n'a plus d'outil assez bon, la
    /// requête part (`:366-369`). Un outil **cassé** arrive ici aussi : la case est
    /// vide après la casse et la vérification en redemande un.
    ToolMissing,
    /// `mineBlock` n'a pas fini : le tick frappe et reste (`:371-382`).
    MiningIncomplete,
    /// `job.getActionsDone()` a atteint le seuil de vidage : le planteur s'arrête et
    /// attend de vider son inventaire (`AbstractEntityAIBasic.java:409-416`, lu par
    /// `AbstractEntityAICrafting.decide` à `:284-288`).
    InventoryDumpDue,
    /// Le module d'élevage courant est nul : `current_module == null`
    /// (`AbstractEntityAIHerder.java:304-307`, `:337-340`, `:421-425`, `:519-522`).
    NoHerdModule,
    /// `searchForAnimals(module::isCompatible)` a rendu une liste vide
    /// (`:210-214`, `:349-352`).
    NoAnimals,
    /// `equipTool(MAIN_HAND, axe)` a répondu faux : aucune hache assez bonne, la
    /// requête part (`:344-347`, `:721-731`).
    NoAxe,
    /// `equipItem(MAIN_HAND, breedingItems)` a répondu faux (`:466-470`, `:524-527`).
    NoBreedingItem,
    /// Les animaux `isBreedAble` sont moins de deux : `breedTimeOut` passe à
    /// 60 secondes et il rend `DECIDE` (`:435-440`).
    NotEnoughBreedables,
    /// Aucun second animal à moins de `DISTANCE_TO_BREED` qui accepte
    /// l'accouplement : `breedTimeOut` passe à 20 secondes (`:449-464`).
    NoMate,
    /// `breedTwoAnimals()` a répondu vrai : il reste des partenaires à rejoindre
    /// (`:427-430`, `:476-479`, `:639-665`).
    BreedingInProgress,
    /// `breedTimeOut > 0` empêche `HERDER_BREED` (`:238`).
    BreedingCoolingDown,
    /// Aucun animal adulte et pas amoureux à abattre : `toKill == null`
    /// (`:359-375`).
    NoButcherTarget,
    /// `walkingToAnimal` : il marche encore vers l'animal (`:622-632`).
    NotAtAnimal,
    /// `butcherAnimal` a frappé mais l'animal est toujours vivant — c'est le tirage
    /// de l'éleveur de poulets (`EntityAIWorkChickenHerder.java:67-71`, puis
    /// `AbstractEntityAIHerder.java:379-389` renvoie `HERDER_BUTCHER`).
    AnimalSurvived,
    /// `searchForItemsInArea()` est vide (`:577-587`).
    NothingToPickUp,
    /// `walkWithProxy(items.get(0).blockPosition(), 1)` a répondu faux (`:579-582`).
    NotAtItem,
    /// Tous les animaux ont été nourris il y a moins de cinq minutes (`:532-544`).
    FeedNotDue,
    /// `milkCoolDown > 0` : la traite est en attente (`EntityAIWorkCowboy.java:
    /// 101-104`), et il est posé à `MILK_COOL_DOWN` quand quelque chose manque
    /// (`:157`, `:167`).
    MilkCoolingDown,
    /// La même chose pour la soupe de champignon (`:110-113`, `:211`, `:221`).
    StewCoolingDown,
    /// Aucun seau vide dans l'inventaire, et `walkToBuilding()` n'a pas abouti donc
    /// le ravitaillement depuis la hutte est abandonné (`:148-160`).
    NoMilkInput,
    /// Aucun animal traitable : pas de vache ou de chèvre adulte, et pas de
    /// vache-champignon (`:162-169`, `:216-223`).
    NoMilkableAnimal,
}

impl WorkRefusal {
    /// La ligne Java qui décide ce refus.
    pub fn java(self) -> &'static str {
        match self {
            WorkRefusal::NoFactsForTrade => "port: src/ai_work_09, pas une branche Java",
            WorkRefusal::HutNotBuilt => "EntityAIWorkPlanter.java:105-109",
            WorkRefusal::NoFreeField => "EntityAIWorkPlanter.java:112-120",
            WorkRefusal::NoFieldToWork => "EntityAIWorkPlanter.java:141",
            WorkRefusal::FieldNotHeld => "EntityAIWorkPlanter.java:162-165",
            WorkRefusal::NoWorkPosition => "EntityAIWorkPlanter.java:190-194",
            WorkRefusal::NoActiveResult => "EntityAIWorkPlanter.java:208",
            WorkRefusal::ModuleAnsweredNothing => "EntityAIWorkPlanter.java:196-197",
            WorkRefusal::NotAtField => "EntityAIWorkPlanter.java:167-170",
            WorkRefusal::NotAtHut => "EntityAIWorkPlanter.java:265-268",
            WorkRefusal::NoPlantItem => "EntityAIWorkPlanter.java:308-311",
            WorkRefusal::NoBonemeal => "EntityAIWorkPlanter.java:341-347",
            WorkRefusal::PlantBlocked => "EntityAIWorkPlanter.java:317-324",
            WorkRefusal::ToolMissing => "EntityAIWorkPlanter.java:366-369",
            WorkRefusal::MiningIncomplete => "EntityAIWorkPlanter.java:371-382",
            WorkRefusal::InventoryDumpDue => "AbstractEntityAIBasic.java:409-416",
            WorkRefusal::NoHerdModule => "AbstractEntityAIHerder.java:304-307",
            WorkRefusal::NoAnimals => "AbstractEntityAIHerder.java:210-214",
            WorkRefusal::NoAxe => "AbstractEntityAIHerder.java:344-347",
            WorkRefusal::NoBreedingItem => "AbstractEntityAIHerder.java:466-470",
            WorkRefusal::NotEnoughBreedables => "AbstractEntityAIHerder.java:435-440",
            WorkRefusal::NoMate => "AbstractEntityAIHerder.java:449-464",
            WorkRefusal::BreedingInProgress => "AbstractEntityAIHerder.java:427-430",
            WorkRefusal::BreedingCoolingDown => "AbstractEntityAIHerder.java:238",
            WorkRefusal::NoButcherTarget => "AbstractEntityAIHerder.java:359-375",
            WorkRefusal::NotAtAnimal => "AbstractEntityAIHerder.java:622-632",
            WorkRefusal::AnimalSurvived => "EntityAIWorkChickenHerder.java:63-72",
            WorkRefusal::NothingToPickUp => "AbstractEntityAIHerder.java:577-587",
            WorkRefusal::NotAtItem => "AbstractEntityAIHerder.java:579-582",
            WorkRefusal::FeedNotDue => "AbstractEntityAIHerder.java:532-544",
            WorkRefusal::MilkCoolingDown => "EntityAIWorkCowboy.java:101-104",
            WorkRefusal::StewCoolingDown => "EntityAIWorkCowboy.java:110-113",
            WorkRefusal::NoMilkInput => "EntityAIWorkCowboy.java:148-160",
            WorkRefusal::NoMilkableAnimal => "EntityAIWorkCowboy.java:162-169",
        }
    }

    /// Une clé stable pour un message que le joueur lira, à côté de
    /// `crate::ai::worker::Refusal::key`.
    pub fn key(self) -> &'static str {
        match self {
            WorkRefusal::NoFactsForTrade => "ai_work_09.refusal.no_facts_for_trade",
            WorkRefusal::HutNotBuilt => "ai_work_09.refusal.hut_not_built",
            WorkRefusal::NoFreeField => "ai_work_09.refusal.no_free_field",
            WorkRefusal::NoFieldToWork => "ai_work_09.refusal.no_field_to_work",
            WorkRefusal::FieldNotHeld => "ai_work_09.refusal.field_not_held",
            WorkRefusal::NoWorkPosition => "ai_work_09.refusal.no_work_position",
            WorkRefusal::NoActiveResult => "ai_work_09.refusal.no_active_result",
            WorkRefusal::ModuleAnsweredNothing => "ai_work_09.refusal.module_answered_nothing",
            WorkRefusal::NotAtField => "ai_work_09.refusal.not_at_field",
            WorkRefusal::NotAtHut => "ai_work_09.refusal.not_at_hut",
            WorkRefusal::NoPlantItem => "ai_work_09.refusal.no_plant_item",
            WorkRefusal::NoBonemeal => "ai_work_09.refusal.no_bonemeal",
            WorkRefusal::PlantBlocked => "ai_work_09.refusal.plant_blocked",
            WorkRefusal::ToolMissing => "ai_work_09.refusal.tool_missing",
            WorkRefusal::MiningIncomplete => "ai_work_09.refusal.mining_incomplete",
            WorkRefusal::InventoryDumpDue => "ai_work_09.refusal.inventory_dump_due",
            WorkRefusal::NoHerdModule => "ai_work_09.refusal.no_herd_module",
            WorkRefusal::NoAnimals => "ai_work_09.refusal.no_animals",
            WorkRefusal::NoAxe => "ai_work_09.refusal.no_axe",
            WorkRefusal::NoBreedingItem => "ai_work_09.refusal.no_breeding_item",
            WorkRefusal::NotEnoughBreedables => "ai_work_09.refusal.not_enough_breedables",
            WorkRefusal::NoMate => "ai_work_09.refusal.no_mate",
            WorkRefusal::BreedingInProgress => "ai_work_09.refusal.breeding_in_progress",
            WorkRefusal::BreedingCoolingDown => "ai_work_09.refusal.breeding_cooling_down",
            WorkRefusal::NoButcherTarget => "ai_work_09.refusal.no_butcher_target",
            WorkRefusal::NotAtAnimal => "ai_work_09.refusal.not_at_animal",
            WorkRefusal::AnimalSurvived => "ai_work_09.refusal.animal_survived",
            WorkRefusal::NothingToPickUp => "ai_work_09.refusal.nothing_to_pick_up",
            WorkRefusal::NotAtItem => "ai_work_09.refusal.not_at_item",
            WorkRefusal::FeedNotDue => "ai_work_09.refusal.feed_not_due",
            WorkRefusal::MilkCoolingDown => "ai_work_09.refusal.milk_cooling_down",
            WorkRefusal::StewCoolingDown => "ai_work_09.refusal.stew_cooling_down",
            WorkRefusal::NoMilkInput => "ai_work_09.refusal.no_milk_input",
            WorkRefusal::NoMilkableAnimal => "ai_work_09.refusal.no_milkable_animal",
        }
    }
}

impl fmt::Display for WorkRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nineteen_states_and_their_eat_flag_follow_java() {
        assert_eq!(WorkState::ALL.len(), 19);
        // Les drapeaux que Java passe au constructeur de l'énumération
        // (`AIWorkerState.java:357-405`, `:629-633`).
        assert!(WorkState::PlantationWorkField.okay_to_eat());
        assert!(WorkState::HerderPickup.okay_to_eat());
        assert!(!WorkState::PlantationPickField.okay_to_eat());
        assert!(!WorkState::HerderBreed.okay_to_eat());
        assert!(!WorkState::CowboyMilk.okay_to_eat());
        assert!(!WorkState::CowboyStew.okay_to_eat());
    }

    #[test]
    fn every_state_has_exactly_one_java_name() {
        let mut names: Vec<&str> = WorkState::ALL.iter().map(|s| s.java_name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 19, "deux états partagent un nom");
        // L'état de base délègue à `crate::ai`, il ne réécrit pas le nom.
        assert_eq!(
            WorkState::Base(WorkerState::GatheringRequiredMaterials).java_name(),
            "GATHERING_REQUIRED_MATERIALS"
        );
        assert_eq!(
            WorkState::Base(WorkerState::GatheringRequiredMaterials).worker(),
            Some(WorkerState::GatheringRequiredMaterials)
        );
        assert_eq!(WorkState::HerderFeed.worker(), None);
    }

    #[test]
    fn the_cowboy_carries_both_tables_and_the_others_do_not() {
        // `EntityAIWorkCowboy.java:39` étend `AbstractEntityAIHerder` : sa table est
        // celle de l'éleveur plus ses deux états (`:69-72`).
        assert!(WorkState::CowboyMilk.belongs_to(super::super::WorkTrade::Cowboy));
        assert!(WorkState::HerderBreed.belongs_to(super::super::WorkTrade::Cowboy));
        assert!(!WorkState::CowboyMilk.belongs_to(super::super::WorkTrade::ChickenHerder));
        assert!(WorkState::HerderButcher.belongs_to(super::super::WorkTrade::ChickenHerder));
        assert!(WorkState::PlantationWorkField.belongs_to(super::super::WorkTrade::Planter));
        assert!(!WorkState::PlantationWorkField.belongs_to(super::super::WorkTrade::Cowboy));
    }

    #[test]
    fn every_refusal_names_the_line_that_decides_it() {
        for refusal in [
            WorkRefusal::HutNotBuilt,
            WorkRefusal::NoFreeField,
            WorkRefusal::NoFieldToWork,
            WorkRefusal::FieldNotHeld,
            WorkRefusal::NoWorkPosition,
            WorkRefusal::NoActiveResult,
            WorkRefusal::ModuleAnsweredNothing,
            WorkRefusal::NotAtField,
            WorkRefusal::NotAtHut,
            WorkRefusal::NoPlantItem,
            WorkRefusal::NoBonemeal,
            WorkRefusal::PlantBlocked,
            WorkRefusal::ToolMissing,
            WorkRefusal::MiningIncomplete,
            WorkRefusal::InventoryDumpDue,
            WorkRefusal::NoHerdModule,
            WorkRefusal::NoAnimals,
            WorkRefusal::NoAxe,
            WorkRefusal::NoBreedingItem,
            WorkRefusal::NotEnoughBreedables,
            WorkRefusal::NoMate,
            WorkRefusal::BreedingInProgress,
            WorkRefusal::BreedingCoolingDown,
            WorkRefusal::NoButcherTarget,
            WorkRefusal::NotAtAnimal,
            WorkRefusal::AnimalSurvived,
            WorkRefusal::NothingToPickUp,
            WorkRefusal::NotAtItem,
            WorkRefusal::FeedNotDue,
            WorkRefusal::MilkCoolingDown,
            WorkRefusal::StewCoolingDown,
            WorkRefusal::NoMilkInput,
            WorkRefusal::NoMilkableAnimal,
        ] {
            assert!(
                refusal.java().ends_with(".java") || refusal.java().contains(".java:"),
                "{refusal:?} ne cite pas de fichier Java"
            );
            assert!(refusal.key().starts_with("ai_work_09.refusal."));
        }
        // La seule qui ne cite aucune ligne Java, et elle le dit : c'est un refus du
        // port, pas une branche du jeu — des faits d'un autre métier que ceux de la
        // table qui les lit.
        assert_eq!(
            WorkRefusal::NoFactsForTrade.java(),
            "port: src/ai_work_09, pas une branche Java"
        );
    }
}
