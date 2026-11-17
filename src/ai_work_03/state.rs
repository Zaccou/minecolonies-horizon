//! Les états que ces trois métiers ajoutent, leurs refus nommés, et la forme d'une
//! décision.
//!
//! Référence Java : `api/entity/ai/statemachine/states/AIWorkerState.java` pour les
//! quatre états que `AbstractEntityAIRequestSmelter` enregistre — `FILL_UP_FURNACES`
//! (`:305`), `RETRIEVING_END_PRODUCT_FROM_FURNACE` (`:315`),
//! `RETRIEVING_UNRELATED_PRODUCT_FROM_FURNACE` (`:320`), `ADD_FUEL_TO_FURNACE`
//! (`:330`) — et `AbstractEntityAICrafting.java:160-170` pour les six cibles communes.
//!
//! **Pourquoi `Base(CraftState)` plutôt que huit états recopiés.** Les sept états du
//! métier d'atelier (`IDLE`, `START_WORKING`, `QUERY_ITEMS`, `GET_RECIPE`, `CRAFT`,
//! `INVENTORY_FULL`, `GATHERING_REQUIRED_MATERIALS`) sont déjà nommés par
//! [`crate::ai_build::crafter::CraftState`], et cette énumération appartient à un autre
//! dossier. Un état commun est donc [`TradeState::Base`] enveloppant le `CraftState` que
//! l'autre module connaît : aucun état n'est écrit deux fois.
//!
//! **Pourquoi la table est réécrite quand même.** `Machine<S, F>` est monomorphique en
//! `S` **et** en `F` (`crate::ai::state::Machine`) : une machine ne peut pas contenir
//! celle du crafter, qui tique sur `CraftFacts`. Ce module redéclare donc les sept
//! cibles de `AbstractEntityAICrafting.java:164-169` sur [`super::facts::Scene`], mais
//! **chaque action appelle** `crafter::decide`, `crafter::check_for_items`,
//! `crafter::get_recipe`, `crafter::craft` et `crafter::execute` : la règle n'a qu'un
//! domicile, seule la cible est dite deux fois. Le test
//! `the_trade_table_answers_like_the_crafter` tient ce contrat, la même façon que
//! `crate::ai_work_01::state`.
//!
//! **Un état self-loop est une réponse, pas une boucle.** `return getState()` est le
//! « je reste ici » de Java ; `Machine::step` le rend comme un `Fired` dont `from == to`
//! et arrête le tick (`crate::ai::state::Machine::step`). Une décision qui n'a rien à
//! faire rend donc `Some(état courant)` via [`Decision::stay`] + `apply`, jamais `None`
//! — `None` veut dire « essaie la transition suivante » dans ce moteur.

use core::fmt;

use crate::ai::Pos;
use crate::ai_build::materials::NeedKey;
use crate::ai_build::CraftState;

/// Un état d'une des trois tables : un état commun rendu au crafter, ou un état que le
/// souffleur de verre possède.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
// exhaustive: les sept états communs plus les quatre que le four ajoute
pub enum TradeState {
    /// Un état de [`CraftState`] — le métier d'atelier, déjà porté.
    Base(CraftState),
    /// `FILL_UP_FURNACES` (`AIWorkerState.java:305`) — mettre l'entrée dans un four.
    FillUpFurnaces,
    /// `RETRIEVING_END_PRODUCT_FROM_FURNACE` (`:315`) — sortir le produit cuit.
    RetrieveProductFromFurnace,
    /// `RETRIEVING_UNRELATED_PRODUCT_FROM_FURNACE` (`:320`) — sortir ce qui n'est pas
    /// de la recette.
    RetrieveUnrelatedProduct,
    /// `ADD_FUEL_TO_FURNACE` (`:330`) — remettre du carburant.
    AddFuelToFurnace,
}

impl TradeState {
    /// Les onze, états communs d'abord (dans l'ordre de `CraftState`), puis les quatre
    /// du four dans l'ordre de `AIWorkerState`.
    pub const ALL: [TradeState; 11] = [
        TradeState::Base(CraftState::Idle),
        TradeState::Base(CraftState::StartWorking),
        TradeState::Base(CraftState::QueryItems),
        TradeState::Base(CraftState::GetRecipe),
        TradeState::Base(CraftState::Craft),
        TradeState::Base(CraftState::InventoryFull),
        TradeState::Base(CraftState::GatheringRequiredMaterials),
        TradeState::FillUpFurnaces,
        TradeState::RetrieveProductFromFurnace,
        TradeState::RetrieveUnrelatedProduct,
        TradeState::AddFuelToFurnace,
    ];

    /// Les quatre que seul un métier de four enregistre (`AbstractEntityAIRequestSmelter
    /// .java:78-81`) : le bûcheron et le mécanicien ne les atteignent jamais.
    pub const FURNACE: [TradeState; 4] = [
        TradeState::FillUpFurnaces,
        TradeState::RetrieveProductFromFurnace,
        TradeState::RetrieveUnrelatedProduct,
        TradeState::AddFuelToFurnace,
    ];

    /// L'identifiant Java, pour une ligne de log ou un échec de test.
    pub fn java_name(self) -> &'static str {
        match self {
            TradeState::Base(state) => state.java_name(),
            TradeState::FillUpFurnaces => "FILL_UP_FURNACES",
            TradeState::RetrieveProductFromFurnace => "RETRIEVING_END_PRODUCT_FROM_FURNACE",
            TradeState::RetrieveUnrelatedProduct => "RETRIEVING_UNRELATED_PRODUCT_FROM_FURNACE",
            TradeState::AddFuelToFurnace => "ADD_FUEL_TO_FURNACE",
        }
    }

    /// La ligne qui déclare l'état.
    pub fn java(self) -> &'static str {
        match self {
            TradeState::Base(CraftState::Idle) => "AIWorkerState.java:15",
            TradeState::Base(CraftState::StartWorking) => "AIWorkerState.java:31",
            TradeState::Base(CraftState::QueryItems) => "AIWorkerState.java:523",
            TradeState::Base(CraftState::GetRecipe) => "AIWorkerState.java:518",
            TradeState::Base(CraftState::Craft) => "AIWorkerState.java:528",
            TradeState::Base(CraftState::InventoryFull) => "AIWorkerState.java:23",
            TradeState::Base(CraftState::GatheringRequiredMaterials) => "AIWorkerState.java:310",
            TradeState::FillUpFurnaces => "AIWorkerState.java:305",
            TradeState::RetrieveProductFromFurnace => "AIWorkerState.java:315",
            TradeState::RetrieveUnrelatedProduct => "AIWorkerState.java:320",
            TradeState::AddFuelToFurnace => "AIWorkerState.java:330",
        }
    }

    /// L'état commun que cet état rend, `None` pour un état de four.
    pub fn craft(self) -> Option<CraftState> {
        match self {
            TradeState::Base(state) => Some(state),
            _ => None,
        }
    }

    /// Vrai pour les quatre états que le four ajoute.
    pub fn is_furnace(self) -> bool {
        TradeState::FURNACE.contains(&self)
    }
}

impl fmt::Display for TradeState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.java_name())
    }
}

/// Pourquoi l'IA n'a pas pu continuer, nommé.
///
/// Java n'a pas ce type : une méthode qui ne peut pas avancer `return getState()` et
/// laisse la raison implicite dans la branche prise. Un appelant doit pourtant la
/// montrer à un joueur (« pourquoi mon souffleur de verre ne fait rien ? ») et un test
/// doit asserter dessus — d'où un variant par branche, chacun avec sa ligne.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: un variant par `return` que Java fait sur les quatre états du four
pub enum TradeRefusal {
    /// `getExpectedBuildingClass().isInstance(...)` a refusé la hutte
    /// (`AbstractEntityAIBasic.java:191-198`) — c'est le seul code des trois classes.
    WrongBuildingClass,
    /// La scène porte les faits d'un autre métier : on ne devine pas
    /// (`crate::ai_gather::GatherRefusal::NoFactsForTrade`, même choix).
    NoFactsForTrade,
    /// `building.getModule(FURNACE).getFurnaces().isEmpty()`
    /// (`AbstractEntityAIRequestSmelter.java:439-448`).
    NoFurnaces,
    /// `!areFurnacesLoaded()` (`:450-453`) : un chunk déchargé n'est pas un four vide.
    FurnacesUnloaded,
    /// `getAllowedFuel().isEmpty()` (`:455-463`) : le joueur n'a déclaré aucun carburant.
    NoFuelAllowed,
    /// `furnacePos == null` alors que l'action en demande un (`:193-196`).
    NoFurnaceChosen,
    /// Le four choisi au tick précédent n'est plus un four, ou son slot est vide
    /// (`:235-239`, `:293-300`).
    FurnaceGone,
    /// `!walkToWorkPos(furnacePos)` (`:198-201`, `:241-244`, `:302-305`, `:367-370`).
    NotAtFurnace,
    /// `isEmpty(getItem(RESULT_SLOT))` (`:235`).
    ResultSlotEmpty,
    /// Le slot d'entrée ne porte pas ce que la recette demande (`:293-296`).
    SmeltableNotEmpty,
    /// `burning > 0 && burning >= getMaxUsableFurnaces()` (`:486-491`).
    AllFurnacesBusy,
    /// Une entrée manque et la hutte n'en a pas : la recette est abandonnée
    /// (`AbstractEntityAICrafting.java:497-499`).
    NoSmeltableForTheHut,
    /// Le plating `fillUpFurnace` n'a plus rien à faire avancer cette fois
    /// (`:348-351`, `:373-376`) : la tâche est couverte par ce qui cuit déjà.
    AlreadySmelting,
}

impl TradeRefusal {
    /// La ligne Java qui décide ce refus.
    pub fn java(self) -> &'static str {
        match self {
            TradeRefusal::WrongBuildingClass => "AbstractEntityAIBasic.java:191-198",
            TradeRefusal::NoFactsForTrade => "voir MANQUANT.md §1",
            TradeRefusal::NoFurnaces => "AbstractEntityAIRequestSmelter.java:439-448",
            TradeRefusal::FurnacesUnloaded => "AbstractEntityAIRequestSmelter.java:450-453",
            TradeRefusal::NoFuelAllowed => "AbstractEntityAIRequestSmelter.java:455-463",
            TradeRefusal::NoFurnaceChosen => "AbstractEntityAIRequestSmelter.java:193-196",
            TradeRefusal::FurnaceGone => "AbstractEntityAIRequestSmelter.java:235-239",
            TradeRefusal::NotAtFurnace => "AbstractEntityAIRequestSmelter.java:241-244",
            TradeRefusal::ResultSlotEmpty => "AbstractEntityAIRequestSmelter.java:235",
            TradeRefusal::SmeltableNotEmpty => "AbstractEntityAIRequestSmelter.java:293-296",
            TradeRefusal::AllFurnacesBusy => "AbstractEntityAIRequestSmelter.java:486-491",
            TradeRefusal::NoSmeltableForTheHut => "AbstractEntityAICrafting.java:497-499",
            TradeRefusal::AlreadySmelting => "AbstractEntityAIRequestSmelter.java:348-351",
        }
    }

    /// Une clé stable pour un message que le joueur lira.
    pub fn key(self) -> &'static str {
        match self {
            TradeRefusal::WrongBuildingClass => "ai_work_03.refusal.wrong_building_class",
            TradeRefusal::NoFactsForTrade => "ai_work_03.refusal.no_facts_for_trade",
            TradeRefusal::NoFurnaces => "ai_work_03.refusal.no_furnaces",
            TradeRefusal::FurnacesUnloaded => "ai_work_03.refusal.furnaces_unloaded",
            TradeRefusal::NoFuelAllowed => "ai_work_03.refusal.no_fuel_allowed",
            TradeRefusal::NoFurnaceChosen => "ai_work_03.refusal.no_furnace_chosen",
            TradeRefusal::FurnaceGone => "ai_work_03.refusal.furnace_gone",
            TradeRefusal::NotAtFurnace => "ai_work_03.refusal.not_at_furnace",
            TradeRefusal::ResultSlotEmpty => "ai_work_03.refusal.result_slot_empty",
            TradeRefusal::SmeltableNotEmpty => "ai_work_03.refusal.smeltable_not_empty",
            TradeRefusal::AllFurnacesBusy => "ai_work_03.refusal.all_furnaces_busy",
            TradeRefusal::NoSmeltableForTheHut => "ai_work_03.refusal.no_smeltable_for_the_hut",
            TradeRefusal::AlreadySmelting => "ai_work_03.refusal.already_smelting",
        }
    }
}

impl fmt::Display for TradeRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key())
    }
}

/// Ce que le but demande au monde.
///
/// Java l'écrit dans le code (`InventoryUtils.transferX…`, `walkToWorkPos`,
/// `createRequestAsync`, `setItemInHand`) ; le port le nomme, parce qu'une décision qui
/// déplacerait un item elle-même ne serait pas testable. Chaque variant nomme la ligne
/// qui l'émet, et la quantité que Java y met.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: un variant par écriture que les quatre états du four demandent
pub enum TradeAction {
    /// `walkToWorkPos(furnacePos)` (`:198`, `:241`, `:302`, `:367`) vers ce four.
    WalkToFurnace(Pos),
    /// `extractFromFurnaceSlot(furnace, RESULT_SLOT)` (`:248`) : sortir `count` items
    /// de `key` du slot de sortie. Java crédite aussi 5 xp
    /// (`AbstractEntityAIRequestSmelter.java:622`) — côté appelant.
    ExtractProduct {
        /// Le four.
        at: Pos,
        /// Ce qui sort, tel que Java l'a lu avant d'extraire (`:247`).
        key: NeedKey,
        /// `stack.getCount()`.
        count: u32,
    },
    /// `extractFromFurnaceSlot(furnace, SMELTABLE_SLOT)` (`:309`) : sortir une entrée
    /// étrangère, qui part en sortie secondaire (`:314`).
    ExtractUnrelated {
        /// Le four.
        at: Pos,
        /// L'entrée étrangère.
        key: NeedKey,
        /// Sa quantité.
        count: u32,
    },
    /// `transferXOfFirstSlotInItemHandlerWithIntoInItemHandler(…, STACKSIZE, …,
    /// FUEL_SLOT)` (`:210-212`).
    TransferFuel {
        /// Le four.
        at: Pos,
        /// `STACKSIZE` (`:211`) : Java transfère jusqu'à 64.
        count: u32,
    },
    /// `transferXInItemHandlerIntoSlotInItemHandler(…, toTransfer, …, SMELTABLE_SLOT)`
    /// (`:413-418`), précédé de `hitBlockWithToolInHand(worker, furnacePos)` (`:412`).
    TransferSmeltable {
        /// Le four.
        at: Pos,
        /// `toTransfer` (`:391-409`), calculé par [`super::furnace::fill_split`].
        count: u32,
    },
    /// `createRequestAsync(new StackList(possibleFuels, …, STACKSIZE * furnaces.size(),
    /// 1))` (`:111`).
    RequestFuel {
        /// Les carburants déclarés, sortie et entrée de la recette retirées
        /// (`:715-719`).
        fuels: Vec<NeedKey>,
        /// `STACKSIZE * module.getFurnaces().size()`.
        size: u32,
        /// Le minimum de la requête, `1` (`:111`).
        min: u32,
    },
    /// `needsCurrently = new Tuple<>(smeltablePredicate, targetCount)` puis
    /// `GATHERING_REQUIRED_MATERIALS` (`:355-357`) : aller chercher `count` entrées.
    RequestSmeltable {
        /// L'entrée de la recette.
        key: NeedKey,
        /// `targetCount` (`:346`).
        count: u32,
    },
    /// `FurnaceBlockEntity.serverTick(...)` appelé `ticks` fois sur un four qui brûle
    /// (`:131-137`) : l'accélération par la compétence secondaire.
    Accelerate {
        /// Le four.
        at: Pos,
        /// `(secondaire / 10) * 2` (`:122`).
        ticks: u32,
    },
    /// `building.getModule(FURNACE).removeFromFurnaces(pos)` (`:597`) : la position
    /// n'est plus un four, la hutte doit l'oublier.
    ForgetFurnace(Pos),
    /// `hitBlockWithToolInHand(worker, pos)` (`:412`) : frapper le four.
    HitFurnace(Pos),
}

impl TradeAction {
    /// La ligne Java qui émet cette action.
    pub fn java(&self) -> &'static str {
        match self {
            TradeAction::WalkToFurnace(_) => "AbstractEntityAIRequestSmelter.java:198",
            TradeAction::ExtractProduct { .. } => "AbstractEntityAIRequestSmelter.java:248",
            TradeAction::ExtractUnrelated { .. } => "AbstractEntityAIRequestSmelter.java:309",
            TradeAction::TransferFuel { .. } => "AbstractEntityAIRequestSmelter.java:210-212",
            TradeAction::TransferSmeltable { .. } => "AbstractEntityAIRequestSmelter.java:413-418",
            TradeAction::RequestFuel { .. } => "AbstractEntityAIRequestSmelter.java:111",
            TradeAction::RequestSmeltable { .. } => "AbstractEntityAIRequestSmelter.java:355-357",
            TradeAction::Accelerate { .. } => "AbstractEntityAIRequestSmelter.java:131-137",
            TradeAction::ForgetFurnace(_) => "AbstractEntityAIRequestSmelter.java:597",
            TradeAction::HitFurnace(_) => "AbstractEntityAIRequestSmelter.java:412",
        }
    }

    /// Le four que cette action vise, s'il y en a un.
    pub fn at(&self) -> Option<Pos> {
        match self {
            TradeAction::WalkToFurnace(at)
            | TradeAction::ExtractProduct { at, .. }
            | TradeAction::ExtractUnrelated { at, .. }
            | TradeAction::TransferFuel { at, .. }
            | TradeAction::TransferSmeltable { at, .. }
            | TradeAction::Accelerate { at, .. }
            | TradeAction::ForgetFurnace(at)
            | TradeAction::HitFurnace(at) => Some(*at),
            TradeAction::RequestFuel { .. } | TradeAction::RequestSmeltable { .. } => None,
        }
    }
}

/// Ce qu'une action de métier a décidé : l'état qu'elle rend et le refus qui l'a
/// arrêtée, plus l'action qu'elle demande au monde.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision {
    /// L'état rendu, `None` pour « celui où je suis » — la forme de `Action::Run`.
    pub next: Option<TradeState>,
    /// Pourquoi elle s'arrête, quand elle s'arrête.
    pub refusal: Option<TradeRefusal>,
    /// Ce qu'elle demande au monde, quand elle demande.
    pub action: Option<TradeAction>,
}

impl Decision {
    /// Aller en `state`.
    pub fn go(state: TradeState) -> Decision {
        Decision {
            next: Some(state),
            refusal: None,
            action: None,
        }
    }

    /// `return getState()` : pas de changement, pas de plainte.
    pub fn stay() -> Decision {
        Decision {
            next: None,
            refusal: None,
            action: None,
        }
    }

    /// `return getState()` avec la raison notée.
    pub fn refuse(refusal: TradeRefusal) -> Decision {
        Decision {
            next: None,
            refusal: Some(refusal),
            action: None,
        }
    }

    /// Aller en `state` **et** noter pourquoi.
    pub fn go_after(state: TradeState, refusal: TradeRefusal) -> Decision {
        Decision {
            next: Some(state),
            refusal: Some(refusal),
            action: None,
        }
    }

    /// Ajouter une raison à une décision qui n'en a pas.
    pub fn because(mut self, refusal: TradeRefusal) -> Decision {
        if self.refusal.is_none() {
            self.refusal = Some(refusal);
        }
        self
    }

    /// Ajouter ce que le monde doit faire.
    pub fn asking(mut self, action: TradeAction) -> Decision {
        self.action = Some(action);
        self
    }
}

/// L'état commun, en abrégé : `base(CraftState::Craft)` plutôt que
/// `TradeState::Base(CraftState::Craft)` dans les tables.
pub fn base(state: CraftState) -> TradeState {
    TradeState::Base(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_eleven_states_are_seven_shared_and_four_furnace() {
        assert_eq!(TradeState::ALL.len(), 11);
        assert_eq!(TradeState::FURNACE.len(), 4);
        let shared = TradeState::ALL
            .iter()
            .filter(|state| state.craft().is_some())
            .count();
        assert_eq!(shared, 7, "`AbstractEntityAICrafting.java:164-169` + two exits");
        assert_eq!(shared + TradeState::FURNACE.len(), TradeState::ALL.len());
        for state in TradeState::FURNACE {
            assert!(state.is_furnace(), "{state}");
            assert_eq!(state.craft(), None, "{state} is not a crafter state");
        }
        assert!(!base(CraftState::Idle).is_furnace());
    }

    #[test]
    fn every_state_names_its_java_declaration() {
        for state in TradeState::ALL {
            assert!(
                state.java().starts_with("AIWorkerState.java:"),
                "{state} cites {}",
                state.java()
            );
            assert_eq!(state.to_string(), state.java_name());
        }
        // The four the smelter registers, in `AIWorkerState`'s order
        // (`AbstractEntityAIRequestSmelter.java:78-81`).
        assert_eq!(TradeState::FillUpFurnaces.java(), "AIWorkerState.java:305");
        assert_eq!(
            TradeState::RetrieveProductFromFurnace.java_name(),
            "RETRIEVING_END_PRODUCT_FROM_FURNACE"
        );
        assert_eq!(
            TradeState::RetrieveUnrelatedProduct.java_name(),
            "RETRIEVING_UNRELATED_PRODUCT_FROM_FURNACE"
        );
        assert_eq!(
            TradeState::AddFuelToFurnace.java_name(),
            "ADD_FUEL_TO_FURNACE"
        );
    }

    #[test]
    fn a_refusal_carries_the_line_that_decides_it_and_a_message_key() {
        for refusal in [
            TradeRefusal::WrongBuildingClass,
            TradeRefusal::NoFurnaces,
            TradeRefusal::FurnacesUnloaded,
            TradeRefusal::NoFuelAllowed,
            TradeRefusal::NoFurnaceChosen,
            TradeRefusal::FurnaceGone,
            TradeRefusal::NotAtFurnace,
            TradeRefusal::ResultSlotEmpty,
            TradeRefusal::SmeltableNotEmpty,
            TradeRefusal::AllFurnacesBusy,
            TradeRefusal::NoSmeltableForTheHut,
            TradeRefusal::AlreadySmelting,
            TradeRefusal::NoFactsForTrade,
        ] {
            assert!(
                refusal.java().contains(".java:") || refusal.java().starts_with("voir "),
                "{refusal:?} cites {}",
                refusal.java()
            );
            assert!(refusal.key().starts_with("ai_work_03.refusal."));
            assert_eq!(refusal.to_string(), refusal.key());
        }
    }

    #[test]
    fn a_decision_says_where_why_and_what_in_one_value() {
        let go = Decision::go(TradeState::AddFuelToFurnace);
        assert_eq!(go.next, Some(TradeState::AddFuelToFurnace));
        assert_eq!(go.refusal, None);
        assert_eq!(go.action, None);

        assert_eq!(Decision::stay().next, None);

        let refused = Decision::refuse(TradeRefusal::AllFurnacesBusy);
        assert_eq!(refused.next, None);
        assert_eq!(refused.refusal, Some(TradeRefusal::AllFurnacesBusy));

        let after = Decision::go_after(TradeState::FillUpFurnaces, TradeRefusal::AlreadySmelting);
        assert_eq!(after.next, Some(TradeState::FillUpFurnaces));
        assert_eq!(after.refusal, Some(TradeRefusal::AlreadySmelting));

        // `because` only fills an empty reason; `asking` only sets the action.
        assert_eq!(
            Decision::stay()
                .because(TradeRefusal::NoFurnaceChosen)
                .refusal,
            Some(TradeRefusal::NoFurnaceChosen)
        );
        assert_eq!(
            Decision::refuse(TradeRefusal::FurnaceGone)
                .because(TradeRefusal::NoFurnaces)
                .refusal,
            Some(TradeRefusal::FurnaceGone),
            "the first reason wins"
        );
        let asked = Decision::stay().asking(TradeAction::RequestSmeltable {
            key: NeedKey::plain("minecraft:sand"),
            count: 3,
        });
        assert!(asked.action.is_some());
        assert_eq!(asked.refusal, None);
    }

    #[test]
    fn every_action_names_its_line_and_the_furnace_it_visits() {
        let at = Pos::new(4, 65, 1);
        let actions = [
            TradeAction::WalkToFurnace(at),
            TradeAction::ExtractProduct {
                at,
                key: NeedKey::plain("minecraft:glass"),
                count: 2,
            },
            TradeAction::ExtractUnrelated {
                at,
                key: NeedKey::plain("minecraft:cobblestone"),
                count: 1,
            },
            TradeAction::TransferFuel { at, count: 64 },
            TradeAction::TransferSmeltable { at, count: 7 },
            TradeAction::Accelerate { at, ticks: 4 },
            TradeAction::ForgetFurnace(at),
            TradeAction::HitFurnace(at),
        ];
        for action in &actions {
            assert_eq!(action.at(), Some(at), "{action:?}");
            assert!(action.java().contains(".java:"), "{action:?}");
        }
        // The two request actions target no furnace: they ask the hut or the colony.
        assert_eq!(TradeAction::RequestFuel { fuels: Vec::new(), size: 64, min: 1 }.at(), None);
        assert_eq!(
            TradeAction::RequestSmeltable {
                key: NeedKey::plain("minecraft:sand"),
                count: 1
            }
            .at(),
            None
        );
    }
}
