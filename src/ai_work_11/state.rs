//! Les états de ces trois métiers, les drapeaux `isOkayToEat`, et les refus nommés.
//!
//! Java référence : `api/entity/ai/statemachine/states/AIWorkerState.java`, une
//! énumération unique pour tous les métiers du mod. Ce fichier ne la réécrit pas : il
//! n'y a **que deux états neufs** (`COOK_SERVE_FOOD_TO_CITIZEN` `:344` et
//! `COOK_SERVE_FOOD_TO_PLAYER` `:348`), et les autres sont ceux des tables déjà
//! portées, enveloppés. Un état écrit deux fois est un état qui divergera.
//!
//! | Variante | Ce qu'elle enveloppe | Domicile de l'énumération |
//! | --- | --- | --- |
//! | [`TradeState::Herd`] | `HERDER_BREED`, `HERDER_BUTCHER`, `HERDER_PICKUP`, `HERDER_FEED` + les états de base | `crate::ai_work_09::state::WorkState` (éleveur) |
//! | [`TradeState::Furnace`] | `IDLE`, `START_WORKING`, `FILL_UP_FURNACES`, `RETRIEVING_END_PRODUCT_FROM_FURNACE`, `RETRIEVING_USED_FUEL_FROM_FURNACE`, `INVENTORY_FULL`, `GATHERING_REQUIRED_MATERIALS` | `crate::ai_work_04::states::SmeltState` (utilisateur de four) |
//! | [`TradeState::Cook`] | `COOK_SERVE_FOOD_TO_CITIZEN`, `COOK_SERVE_FOOD_TO_PLAYER` | ce fichier — les deux seuls que personne n'avait nommés |
//! | [`TradeState::Courier`] | `IDLE`, `START_WORKING`, `PREPARE_DELIVERY`, `DELIVERY`, `PICKUP`, `DUMPING` | `crate::ai_build::delivery::DeliveryState` (livreur) |
//!
//! **Pourquoi envelopper plutôt que recopier.** `Machine<S, F>` est monomorphe en `S` :
//! une machine ne peut pas contenir la machine d'un autre module. Mais une *variante*
//! peut contenir son énumération d'états, et c'est tout ce dont un appelant a besoin :
//! il lit `TradeState::Herd(WorkState::HerderButcher)` et sait exactement de quelle
//! table vient la décision. Aucun des 19 + 8 + 6 états des trois voisins n'est redit.
//!
//! **Ce que les refus font de même.** La moitié d'entre eux existent déjà :
//! `crate::ai_work_09::state::WorkRefusal` (33 raisons de l'éleveur),
//! `crate::ai_work_04::states::Refusal` (24 raisons du four) et
//! `crate::ai_build::delivery::Refusal` (2 portes du livreur). Les variantes
//! [`Refusal::Herd`], [`Refusal::Furnace`] et [`Refusal::Courier`] les enveloppent au
//! lieu de les retaper ; les quatorze variantes propres à ce fichier sont celles du
//! cuisinier et les erreurs d'appelant.

use core::fmt;

use crate::ai::state::WorkerState;

/// Les deux états que le cuisinier possède en propre.
///
/// Java référence : `EntityAIWorkCook.java:95-98` — le constructeur enregistre ces
/// deux cibles, et ce sont les deux seules du métier qui ne viennent pas
/// d'`AbstractEntityAIUsesFurnace` (les six autres sont celles du parent, déjà portées
/// par `crate::ai_work_04`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
// exhaustive: the two targets `EntityAIWorkCook.java:95-98` adds
pub enum CookState {
    /// `COOK_SERVE_FOOD_TO_CITIZEN` (`AIWorkerState.java:344`) : servir un citoyen.
    ServeFoodToCitizen,
    /// `COOK_SERVE_FOOD_TO_PLAYER` (`AIWorkerState.java:348`) : servir un joueur.
    ServeFoodToPlayer,
}

impl CookState {
    /// Les deux, dans l'ordre du constructeur.
    pub const ALL: [CookState; 2] = [CookState::ServeFoodToCitizen, CookState::ServeFoodToPlayer];

    /// L'identifiant Java.
    pub fn java_name(self) -> &'static str {
        match self {
            CookState::ServeFoodToCitizen => "COOK_SERVE_FOOD_TO_CITIZEN",
            CookState::ServeFoodToPlayer => "COOK_SERVE_FOOD_TO_PLAYER",
        }
    }

    /// La ligne de `AIWorkerState.java` qui déclare l'état.
    pub fn java_declaration(self) -> &'static str {
        match self {
            CookState::ServeFoodToCitizen => "AIWorkerState.java:344",
            CookState::ServeFoodToPlayer => "AIWorkerState.java:348",
        }
    }

    /// La cible qui la porte (`EntityAIWorkCook.java:96-97`).
    pub fn java_target(self) -> &'static str {
        match self {
            CookState::ServeFoodToCitizen => "EntityAIWorkCook.java:96",
            CookState::ServeFoodToPlayer => "EntityAIWorkCook.java:97",
        }
    }
}

impl fmt::Display for CookState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.java_name())
    }
}

/// Un état d'une des trois tables : celui d'un voisin, ou l'un des deux du cuisinier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
// exhaustive: the four tables these three trades run on
pub enum TradeState {
    /// L'éleveur (`AbstractEntityAIHerder.java:139-148`) — l'état vient de
    /// `crate::ai_work_09`, y compris les états de base qu'il partage avec `crate::ai`.
    Herd(crate::ai_work_09::state::WorkState),
    /// L'utilisateur de four (`AbstractEntityAIUsesFurnace.java:76-82`) — l'état vient
    /// de `crate::ai_work_04`.
    Furnace(crate::ai_work_04::states::SmeltState),
    /// Le cuisinier : les deux états neufs de ce fichier.
    Cook(CookState),
    /// Le livreur (`EntityAIWorkDeliveryman.java:113-124`) — l'état vient de
    /// `crate::ai_build::delivery`.
    Courier(crate::ai_build::delivery::DeliveryState),
}

impl TradeState {
    /// L'identifiant Java, celui que le nametag du citoyen affiche
    /// (`AbstractEntityAIBasic.java:475`) et qu'un historique d'états enregistre.
    pub fn java_name(self) -> &'static str {
        match self {
            TradeState::Herd(state) => state.java_name(),
            TradeState::Furnace(state) => state.java_name(),
            TradeState::Cook(state) => state.java_name(),
            TradeState::Courier(state) => state.java_name(),
        }
    }

    /// Où l'énumération qui porte cet état est déclarée dans `AIWorkerState.java`.
    ///
    /// Les états enveloppés renvoient au module qui les possède **et** à la plage de
    /// lignes de l'énumération Java : recopier les vingt numéros de ligne serait la
    /// quatrième copie de la même table.
    pub fn java_declaration(self) -> &'static str {
        match self {
            TradeState::Herd(state) => state.java_declaration(),
            TradeState::Furnace(_) => "crate::ai_work_04::states (AIWorkerState.java:305-330)",
            TradeState::Cook(state) => state.java_declaration(),
            TradeState::Courier(_) => "crate::ai_build::delivery (AIWorkerState.java:257-272)",
        }
    }

    /// `AIWorkerState.isOkayToEat`, le drapeau que `AbstractAISkeleton.canBeInterrupted`
    /// rend (`AbstractAISkeleton.java:126-131`) et que
    /// `AbstractEntityAIBasic.inventoryNeedsDump` lit (`:411`).
    ///
    /// Les deux états du cuisinier sont `true` (`:344`, `:348`). Pour les états
    /// enveloppés, la réponse suit la déclaration Java de chaque constante, et les cinq
    /// qui valent `false` sont `DELIVERY` (`:262`), `DUMPING` (`:272`),
    /// `HERDER_BREED` (`:357`), `HERDER_BUTCHER` (`:362`) et `INVENTORY_FULL` (`:23`).
    pub fn okay_to_eat(self) -> bool {
        match self {
            TradeState::Herd(state) => state.okay_to_eat(),
            // :23 est le seul état de four qui refuse l'interruption ; `FILL_UP_FURNACES`
            // (`:305`), les deux récupérations (`:315`, `:325`),
            // `GATHERING_REQUIRED_MATERIALS` (`:310`) et les deux états généraux
            // (`:15`, `:31`) sont `true`.
            TradeState::Furnace(state) => {
                state != crate::ai_work_04::states::SmeltState::InventoryFull
            }
            // :344, :348
            TradeState::Cook(_) => true,
            TradeState::Courier(state) => !matches!(
                state,
                crate::ai_build::delivery::DeliveryState::Delivery
                    | crate::ai_build::delivery::DeliveryState::Dumping
            ),
        }
    }

    /// L'état de base de `crate::ai::state::WorkerState` que cet état double, quand il
    /// y en a un. C'est la jointure avec la table des quatorze cibles de base.
    pub fn base(self) -> Option<WorkerState> {
        match self {
            TradeState::Herd(state) => state.worker(),
            TradeState::Furnace(state) => state.base(),
            TradeState::Cook(_) => None,
            TradeState::Courier(state) => match state {
                crate::ai_build::delivery::DeliveryState::Idle => Some(WorkerState::Idle),
                crate::ai_build::delivery::DeliveryState::StartWorking => {
                    Some(WorkerState::StartWorking)
                }
                crate::ai_build::delivery::DeliveryState::PrepareDelivery => None,
                crate::ai_build::delivery::DeliveryState::Delivery => None,
                crate::ai_build::delivery::DeliveryState::Pickup => None,
                crate::ai_build::delivery::DeliveryState::Dumping => None,
            },
        }
    }

    /// La table à laquelle cet état appartient.
    pub fn trade(self) -> super::Trade {
        match self {
            TradeState::Herd(_) => super::Trade::SwineHerder,
            TradeState::Furnace(_) | TradeState::Cook(_) => super::Trade::Cook,
            TradeState::Courier(_) => super::Trade::Deliveryman,
        }
    }
}

impl fmt::Display for TradeState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.java_name())
    }
}

/// Pourquoi un de ces trois buts n'a rien pu faire, nommé.
///
/// Java n'a pas de tel type : une branche qui ne peut pas avancer fait
/// `return getState()` et la raison reste dans la forme du `if`. Un joueur qui demande
/// « pourquoi mon cuisinier tourne en rond » en a besoin, et un test qui vérifie une
/// suite d'états a besoin de savoir *quelle* branche a tenu.
///
/// **Les trois variantes enveloppantes ne sont pas de la paresse** : les 33 refus de
/// l'éleveur sont décidés par `AbstractEntityAIHerder`, les 24 du four par
/// `AbstractEntityAIUsesFurnace`, les 2 portes du livreur par
/// `checkIfExecute`. Chacun a son propriétaire et ses tests ; ce fichier ne nomme que
/// ce que le cuisinier ajoute, et [`Refusal::NoFactsForTrade`].
/// **Pas de `Hash`, et c'est mesuré** : `crate::ai_build::delivery::Refusal` n'en dérive
/// pas (deux variantes seulement), et un `Hash` sur un type qui enveloppe un type sans
/// `Hash` ne compile pas. Rien ici ne range un refus dans une table de hachage : c'est une
/// raison, pas une clé.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one variant per branch of these three classes that stops without acting
pub enum Refusal {
    /// La scène ne porte pas les faits de ce métier — une erreur d'appelant, pas un
    /// état Java. Nommée pour qu'elle ne passe jamais pour « rien à faire ».
    NoFactsForTrade,
    /// Un refus de la table d'élevage (`crate::ai_work_09::state::WorkRefusal`).
    Herd(crate::ai_work_09::state::WorkRefusal),
    /// Un refus de la table du four (`crate::ai_work_04::states::Refusal`).
    Furnace(crate::ai_work_04::states::Refusal),
    /// Une des deux portes du livreur (`crate::ai_build::delivery::Refusal`).
    Courier(crate::ai_build::delivery::Refusal),
    /// `!walkToBuilding()` : il marche encore vers sa hutte
    /// (`AbstractEntityAIUsesFurnace.java:184-187`).
    Walking,
    /// `furnaceModule.getFurnaces().isEmpty()` (`AbstractEntityAIUsesFurnace.java:
    /// 201-208`) : la hutte n'a aucun four enregistré.
    NoFurnace,
    /// `menuModule.getMenu().isEmpty()` (`EntityAIWorkCook.java:153-156`) : le menu du
    /// restaurant est vide, la requête de nourriture part et l'interaction
    /// `FURNACE_USER_NO_FOOD` est levée.
    NoFoodInMenu,
    /// `citizenToServe.isEmpty()` (`EntityAIWorkCook.java:165-168`).
    CitizenQueueEmpty,
    /// `!building.isInBuilding(citizenToServe.peek().blockPosition())`
    /// (`EntityAIWorkCook.java:172-177`) : le citoyen a quitté la hutte, il est sorti de
    /// la file.
    CitizenLeftBuilding,
    /// `playerToServe.isEmpty()` (`EntityAIWorkCook.java:257-260`).
    PlayerQueueEmpty,
    /// Le joueur a quitté la hutte (`EntityAIWorkCook.java:263-268`).
    PlayerLeftBuilding,
    /// `!walkToWorkPos(...)` : il n'est pas encore arrivé à la personne à servir
    /// (`EntityAIWorkCook.java:179-182`, `:270-273`).
    NotAtServeTarget,
    /// `FoodUtils.getBestFoodForCitizen(...) == -1` alors que l'inventaire du citoyen est
    /// plein : la boucle de `:192-211` ne tourne pas.
    NoBestFood,
    /// `getBestFoodForCitizen(...) == -1` **et** plus rien de mangeable dans le sac du
    /// cuisinier : la file est vidée et il repart travailler
    /// (`EntityAIWorkCook.java:221-227`).
    NoFoodLeft,
    /// `!handler.hasSpace()` (`EntityAIWorkCook.java:190`) : le citoyen est plein, la
    /// nourriture est consommée sur place au lieu d'être transférée (`:192-211`).
    TargetInventoryFull,
    /// `hasItemInItemHandler(handler, canEat)` : le citoyen a déjà de quoi manger
    /// (`EntityAIWorkCook.java:215-218`, `:283-286`).
    TargetAlreadyHasFood,
    /// `InventoryUtils.isItemHandlerFull(handler)` pour un joueur
    /// (`EntityAIWorkCook.java:279-282`).
    PlayerInventoryFull,
    /// `transferFoodUpToSaturation(...)` n'a rien transféré : la file des joueurs est
    /// vidée (`EntityAIWorkCook.java:292-296`).
    NothingTransferred,
    /// Le logement du citoyen servi est deux niveaux au-dessus du restaurant :
    /// `POOR_RESTAURANT_INTERACTION` (`EntityAIWorkCook.java:231-234`). La plainte part
    /// **et le service a lieu** — ce refus accompagne un effet, il ne l'annule pas.
    PoorRestaurant,
    /// La file a un identifiant en tête mais l'appelant n'a pas fourni l'observation de
    /// ce citoyen (`citizen_target`) : une erreur d'appelant, pas une branche Java.
    NoTargetObservation,
}

impl Refusal {
    /// La ligne Java qui décide ce refus.
    pub fn java(self) -> &'static str {
        match self {
            Refusal::NoFactsForTrade => "port: src/ai_work_11, pas une branche Java",
            Refusal::Herd(refusal) => refusal.java(),
            Refusal::Furnace(refusal) => refusal.java(),
            Refusal::Courier(_) => "EntityAIWorkDeliveryman.java:638-661",
            Refusal::Walking => "AbstractEntityAIUsesFurnace.java:184-187",
            Refusal::NoFurnace => "AbstractEntityAIUsesFurnace.java:201-208",
            Refusal::NoFoodInMenu => "EntityAIWorkCook.java:153-156",
            Refusal::CitizenQueueEmpty => "EntityAIWorkCook.java:165-168",
            Refusal::CitizenLeftBuilding => "EntityAIWorkCook.java:172-177",
            Refusal::PlayerQueueEmpty => "EntityAIWorkCook.java:257-260",
            Refusal::PlayerLeftBuilding => "EntityAIWorkCook.java:263-268",
            Refusal::NotAtServeTarget => "EntityAIWorkCook.java:179-182",
            Refusal::NoBestFood => "EntityAIWorkCook.java:194-206",
            Refusal::NoFoodLeft => "EntityAIWorkCook.java:221-227",
            Refusal::TargetInventoryFull => "EntityAIWorkCook.java:190-214",
            Refusal::TargetAlreadyHasFood => "EntityAIWorkCook.java:215-218",
            Refusal::PlayerInventoryFull => "EntityAIWorkCook.java:279-282",
            Refusal::NothingTransferred => "EntityAIWorkCook.java:292-296",
            Refusal::PoorRestaurant => "EntityAIWorkCook.java:231-234",
            Refusal::NoTargetObservation => "port: src/ai_work_11/facts.rs, pas une branche Java",
        }
    }

    /// Une clé stable pour un message que le joueur lira.
    pub fn key(self) -> &'static str {
        match self {
            Refusal::NoFactsForTrade => "ai_work_11.refusal.no_facts_for_trade",
            Refusal::Herd(refusal) => refusal.key(),
            Refusal::Furnace(refusal) => refusal.key(),
            Refusal::Courier(refusal) => match refusal {
                crate::ai_build::delivery::Refusal::NoWarehouse => {
                    "ai_work_11.refusal.courier.no_warehouse"
                }
                crate::ai_build::delivery::Refusal::WarehouseHasNoChest => {
                    "ai_work_11.refusal.courier.warehouse_has_no_chest"
                }
            },
            Refusal::Walking => "ai_work_11.refusal.walking",
            Refusal::NoFurnace => "ai_work_11.refusal.no_furnace",
            Refusal::NoFoodInMenu => "ai_work_11.refusal.no_food_in_menu",
            Refusal::CitizenQueueEmpty => "ai_work_11.refusal.citizen_queue_empty",
            Refusal::CitizenLeftBuilding => "ai_work_11.refusal.citizen_left_building",
            Refusal::PlayerQueueEmpty => "ai_work_11.refusal.player_queue_empty",
            Refusal::PlayerLeftBuilding => "ai_work_11.refusal.player_left_building",
            Refusal::NotAtServeTarget => "ai_work_11.refusal.not_at_serve_target",
            Refusal::NoBestFood => "ai_work_11.refusal.no_best_food",
            Refusal::NoFoodLeft => "ai_work_11.refusal.no_food_left",
            Refusal::TargetInventoryFull => "ai_work_11.refusal.target_inventory_full",
            Refusal::TargetAlreadyHasFood => "ai_work_11.refusal.target_already_has_food",
            Refusal::PlayerInventoryFull => "ai_work_11.refusal.player_inventory_full",
            Refusal::NothingTransferred => "ai_work_11.refusal.nothing_transferred",
            Refusal::PoorRestaurant => "ai_work_11.refusal.poor_restaurant",
            Refusal::NoTargetObservation => "ai_work_11.refusal.no_target_observation",
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key())
    }
}

impl From<crate::ai_work_09::state::WorkRefusal> for Refusal {
    fn from(refusal: crate::ai_work_09::state::WorkRefusal) -> Refusal {
        Refusal::Herd(refusal)
    }
}

impl From<crate::ai_work_04::states::Refusal> for Refusal {
    fn from(refusal: crate::ai_work_04::states::Refusal) -> Refusal {
        Refusal::Furnace(refusal)
    }
}

impl From<crate::ai_build::delivery::Refusal> for Refusal {
    fn from(refusal: crate::ai_build::delivery::Refusal) -> Refusal {
        Refusal::Courier(refusal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_build::delivery::DeliveryState;
    use crate::ai_work_04::states::SmeltState;
    use crate::ai_work_09::state::WorkState as HerdState;

    #[test]
    fn the_two_new_states_carry_their_java_declaration() {
        // `AIWorkerState.java:344`, `:348`, et les deux cibles qui les enregistrent
        // (`EntityAIWorkCook.java:96-97`).
        assert_eq!(
            CookState::ServeFoodToCitizen.java_declaration(),
            "AIWorkerState.java:344"
        );
        assert_eq!(
            CookState::ServeFoodToPlayer.java_declaration(),
            "AIWorkerState.java:348"
        );
        assert_eq!(
            CookState::ServeFoodToCitizen.java_target(),
            "EntityAIWorkCook.java:96"
        );
        assert_eq!(CookState::ALL.len(), 2);
        assert_eq!(
            CookState::ServeFoodToPlayer.to_string(),
            "COOK_SERVE_FOOD_TO_PLAYER"
        );
    }

    #[test]
    fn no_state_name_is_written_twice() {
        // Les quatre tables ont des noms d'états distincts sauf les états de base, qui
        // sont volontairement partagés : le test vérifie qu'aucun nom neuf n'est
        // dupliqué à l'intérieur d'une même variante.
        let mut names: Vec<&str> = crate::ai_work_09::state::WorkState::ALL
            .iter()
            .map(|state| state.java_name())
            .collect();
        let before = names.len();
        names.extend(CookState::ALL.iter().map(|state| state.java_name()));
        names.sort_unstable();
        names.dedup();
        assert_eq!(
            names.len(),
            before + 2,
            "les deux états du cuisinier sont neufs"
        );
    }

    #[test]
    fn the_eat_flag_follows_each_constant_by_name() {
        // Les deux états du cuisinier sont `true` (`:344`, `:348`).
        assert!(TradeState::Cook(CookState::ServeFoodToCitizen).okay_to_eat());
        assert!(TradeState::Cook(CookState::ServeFoodToPlayer).okay_to_eat());
        // Les cinq `false` : `:262`, `:272`, `:357`, `:362`, `:23`.
        assert!(!TradeState::Courier(DeliveryState::Delivery).okay_to_eat());
        assert!(!TradeState::Courier(DeliveryState::Dumping).okay_to_eat());
        assert!(!TradeState::Herd(HerdState::HerderBreed).okay_to_eat());
        assert!(!TradeState::Herd(HerdState::HerderButcher).okay_to_eat());
        assert!(!TradeState::Furnace(SmeltState::InventoryFull).okay_to_eat());
        // Et les `true` qui les entourent.
        assert!(TradeState::Courier(DeliveryState::Pickup).okay_to_eat());
        assert!(TradeState::Courier(DeliveryState::PrepareDelivery).okay_to_eat());
        assert!(TradeState::Herd(HerdState::HerderPickup).okay_to_eat());
        assert!(TradeState::Furnace(SmeltState::FillUpFurnaces).okay_to_eat());
        assert!(TradeState::Furnace(SmeltState::RetrievingEndProductFromFurnace).okay_to_eat());
    }

    #[test]
    fn each_state_knows_its_trade_and_its_base_state() {
        assert_eq!(
            TradeState::Herd(HerdState::HerderFeed).trade(),
            super::super::Trade::SwineHerder
        );
        assert_eq!(
            TradeState::Cook(CookState::ServeFoodToPlayer).trade(),
            super::super::Trade::Cook
        );
        assert_eq!(
            TradeState::Furnace(SmeltState::StartWorking).trade(),
            super::super::Trade::Cook,
            "un utilisateur de four est un cuisinier dans cette tranche"
        );
        assert_eq!(
            TradeState::Courier(DeliveryState::Dumping).trade(),
            super::super::Trade::Deliveryman
        );
        // La jointure avec la table de base, et son absence.
        assert_eq!(
            TradeState::Furnace(SmeltState::Idle).base(),
            Some(WorkerState::Idle)
        );
        assert_eq!(
            TradeState::Courier(DeliveryState::StartWorking).base(),
            Some(WorkerState::StartWorking)
        );
        assert_eq!(TradeState::Cook(CookState::ServeFoodToCitizen).base(), None);
        assert_eq!(TradeState::Courier(DeliveryState::Delivery).base(), None);
    }

    #[test]
    fn every_refusal_names_a_file_and_a_key() {
        let refusals = [
            Refusal::Walking,
            Refusal::NoFurnace,
            Refusal::NoFoodInMenu,
            Refusal::CitizenQueueEmpty,
            Refusal::CitizenLeftBuilding,
            Refusal::PlayerQueueEmpty,
            Refusal::PlayerLeftBuilding,
            Refusal::NotAtServeTarget,
            Refusal::NoBestFood,
            Refusal::NoFoodLeft,
            Refusal::TargetInventoryFull,
            Refusal::TargetAlreadyHasFood,
            Refusal::PlayerInventoryFull,
            Refusal::NothingTransferred,
            Refusal::PoorRestaurant,
        ];
        for refusal in refusals {
            assert!(
                refusal.java().contains(".java:"),
                "{refusal:?} ne cite pas de fichier Java"
            );
            assert!(refusal.key().starts_with("ai_work_11.refusal."));
        }
        // Les deux refus sans ligne Java le disent, et ils sont nommés pour ça.
        assert_eq!(
            Refusal::NoFactsForTrade.java(),
            "port: src/ai_work_11, pas une branche Java"
        );
        assert!(Refusal::NoTargetObservation
            .java()
            .starts_with("port: src/ai_work_11/facts.rs"));
    }

    #[test]
    fn a_wrapped_refusal_keeps_the_other_modules_line_and_key() {
        // La raison reste celle du module propriétaire : recopier sa ligne ici ferait
        // deux vérités pour une branche.
        let herd = crate::ai_work_09::state::WorkRefusal::NoAxe;
        let wrapped = Refusal::from(herd);
        assert_eq!(wrapped.java(), herd.java());
        assert_eq!(wrapped.key(), herd.key());
        let furnace = crate::ai_work_04::states::Refusal::NothingToFill;
        assert_eq!(Refusal::from(furnace).java(), furnace.java());
        let courier = crate::ai_build::delivery::Refusal::NoWarehouse;
        assert_eq!(
            Refusal::from(courier).key(),
            "ai_work_11.refusal.courier.no_warehouse"
        );
        assert_eq!(
            Refusal::from(courier).java(),
            "EntityAIWorkDeliveryman.java:638-661"
        );
    }
}
