//! Les états, les phases et les refus d'un métier de `production/agriculture/`.
//!
//! Java référence : `api/entity/ai/statemachine/states/AIWorkerState.java` — **un seul**
//! enum pour tous les métiers du mod (127 valeurs), et chaque constructeur de métier y
//! *ajoute* ses cibles (`EntityAIWorkFlorist.java:128-134`). Le port garde cette forme :
//! [`AgriState`] est un seul enum pour les trois métiers de ce module, et il n'ajoute au
//! vocabulaire existant que ce qui n'existait nulle part.
//!
//! | État | Qui le nomme | Ligne |
//! | --- | --- | --- |
//! | `IDLE`, `PREPARING`, `START_WORKING`, `INVENTORY_FULL`, `NEEDS_ITEM`, `PAUSED`, `GATHERING_REQUIRED_MATERIALS`, `DECIDE` | `crate::ai::state::WorkerState` | `AIWorkerState.java:15-59` |
//! | `FARMER_HOE`, `FARMER_PLANT`, `FARMER_HARVEST` | `crate::ai_gather::GatherState` | `AIWorkerState.java:150-160` |
//! | `FISHERMAN_*` | `crate::ai_gather::GatherState` | `AIWorkerState.java:70-76` |
//! | `FLORIST_HARVEST`, `FLORIST_COMPOST` | **ce module** | `AIWorkerState.java:585`, `:590` |
//!
//! **Pourquoi `Gather(...)` et pas un second `FarmerHoe`.** Le fermier et le pêcheur ont
//! déjà leur table dans `crate::ai_gather` (`farmer::register` cite
//! `EntityAIWorkFarmer.java:136-142`, `fisherman::register` cite
//! `EntityAIWorkFisherman.java:168-176`). Redéclarer leurs états ici serait la deuxième
//! copie d'un vocabulaire qui existe : [`AgriState::Gather`] réutilise le nom, et
//! [`AgriState::java`] rapporte l'identifiant Java tel quel. Les deux états du fleuriste
//! sont les seuls que ce module crée, parce que personne ne les avait portés.
//!
//! **Les phases.** Java ne nomme pas « chercher / aller / agir / rendre » : il les
//! replie dans ses états. Un déplacement n'est pas un état, c'est un `return getState()`
//! tant que `walkToSafePos` répond faux (`EntityAIWorkFarmer.java:529-533`,
//! `EntityAIWorkFlorist.java:223-226`, `EntityAIWorkFisherman.java:312-317`). La phase
//! est donc une **lecture** de `(état, réponses du tick)` — [`Phase`] — et non un second
//! automate. C'est ce qui permet d'écrire le test qui compte : un scénario de faits
//! produit la même suite d'états *et* la même suite de phases que le Java.

use core::fmt;

use crate::ai::state::WorkerState;
use crate::ai_gather::GatherState;

/// Un état d'un métier de `production/agriculture/` : un état que `crate::ai_gather` a
/// déjà nommé, ou un des deux états du fleuriste.
///
/// Java référence : `AIWorkerState.java`, la ligne entre parenthèses est la déclaration
/// de la constante et c'est ce que [`AgriState::java`] rapporte.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
// exhaustive : les vingt-neuf états de `GatherState` plus les deux du fleuriste
pub enum AgriState {
    /// Un état déjà nommé par `crate::ai_gather` — les dix états de base et les dix-neuf
    /// états de métier qu'il porte.
    Gather(GatherState),
    /// `FLORIST_HARVEST` (`AIWorkerState.java:585`) : cueillir une fleur.
    FloristHarvest,
    /// `FLORIST_COMPOST` (`AIWorkerState.java:590`) : composter une terre.
    FloristCompost,
}

impl AgriState {
    /// Tous les états que ce module peut nommer, dans l'ordre de déclaration de
    /// `AIWorkerState` : les vingt-neuf de `crate::ai_gather`, puis les deux du
    /// fleuriste (déclarés bien plus loin dans le fichier Java, `:585` et `:590`).
    pub const ALL: [AgriState; 31] = [
        AgriState::Gather(GatherState::Base(WorkerState::Idle)),
        AgriState::Gather(GatherState::Base(WorkerState::Preparing)),
        AgriState::Gather(GatherState::Base(WorkerState::StartWorking)),
        AgriState::Gather(GatherState::Base(WorkerState::InventoryFull)),
        AgriState::Gather(GatherState::Base(WorkerState::NeedsItem)),
        AgriState::Gather(GatherState::Base(WorkerState::Paused)),
        AgriState::Gather(GatherState::Base(WorkerState::GatheringRequiredMaterials)),
        AgriState::Gather(GatherState::Base(WorkerState::MineBlock)),
        AgriState::Gather(GatherState::Base(WorkerState::LoadStructure)),
        AgriState::Gather(GatherState::Base(WorkerState::BuildingStep)),
        AgriState::Gather(GatherState::LumberjackStartWorking),
        AgriState::Gather(GatherState::LumberjackSearchingTree),
        AgriState::Gather(GatherState::LumberjackChopTree),
        AgriState::Gather(GatherState::LumberjackGathering),
        AgriState::Gather(GatherState::LumberjackNoTreesFound),
        AgriState::Gather(GatherState::LumberjackGathering2),
        AgriState::Gather(GatherState::MinerCheckMineshaft),
        AgriState::Gather(GatherState::MinerWalkingToLadder),
        AgriState::Gather(GatherState::MinerRepairingLadder),
        AgriState::Gather(GatherState::MinerMiningShaft),
        AgriState::Gather(GatherState::MinerBuildingShaft),
        AgriState::Gather(GatherState::MinerMiningNode),
        AgriState::Gather(GatherState::FarmerHoe),
        AgriState::Gather(GatherState::FarmerPlant),
        AgriState::Gather(GatherState::FarmerHarvest),
        AgriState::Gather(GatherState::FishermanSearchingWater),
        AgriState::Gather(GatherState::FishermanWalkingToWater),
        AgriState::Gather(GatherState::FishermanCheckWater),
        AgriState::Gather(GatherState::FishermanStartFishing),
        AgriState::FloristHarvest,
        AgriState::FloristCompost,
    ];

    /// L'identifiant Java de l'état — celui d'une trace ou du nom flottant au-dessus du
    /// citoyen (`AbstractEntityAIBasic.java:475`).
    pub fn java(self) -> &'static str {
        match self {
            AgriState::Gather(state) => state.java(),
            AgriState::FloristHarvest => "FLORIST_HARVEST",
            AgriState::FloristCompost => "FLORIST_COMPOST",
        }
    }

    /// L'état de `crate::ai_gather` que celui-ci réutilise, si c'en est un.
    pub fn gather(self) -> Option<GatherState> {
        match self {
            AgriState::Gather(state) => Some(state),
            AgriState::FloristHarvest | AgriState::FloristCompost => None,
        }
    }

    /// L'état de base que celui-ci réutilise, si c'en est un — la porte vers
    /// `crate::ai::state::WorkerState` et ses règles (dont [`AgriState::okay_to_eat`]).
    pub fn worker(self) -> Option<WorkerState> {
        match self.gather()? {
            GatherState::Base(state) => Some(state),
            _ => None,
        }
    }

    /// `AIWorkerState`'s `isOkayToEat` flag, que `canBeInterrupted()` lit
    /// (`AbstractAISkeleton.java:126-131`) et que `inventoryNeedsDump` refuse de
    /// franchir (`AbstractEntityAIBasic.java:411`).
    pub fn okay_to_eat(self) -> bool {
        match self {
            // :585, :590 — les deux constantes Java valent `true`.
            AgriState::FloristHarvest | AgriState::FloristCompost => true,
            AgriState::Gather(state) => match state {
                GatherState::Base(base) => base.okay_to_eat(),
                // Les états de métier de `crate::ai_gather` portent déjà ce drapeau et
                // le rapportent par leur propre table ; les quatre du pêcheur et les
                // trois du fermier valent `true` sauf `FISHERMAN_START_FISHING`
                // (`AIWorkerState.java:76`, `false`).
                GatherState::FishermanStartFishing => false,
                _ => true,
            },
        }
    }

    /// L'état appartient-il à une table de travail — c'est-à-dire Java y fait-il une
    /// action (`mineBlock`, `compost`, lancer la ligne) plutôt que de chercher ou de
    /// marcher ?
    ///
    /// Java référence : les six états dont le corps appelle une action du monde —
    /// `workAtField` pour les trois du fermier (`EntityAIWorkFarmer.java:138-140`),
    /// `doFishing` (`EntityAIWorkFisherman.java:175`), `harvest` et `compost`
    /// (`EntityAIWorkFlorist.java:132-133`).
    pub fn is_work(self) -> bool {
        matches!(
            self,
            AgriState::Gather(
                GatherState::FarmerHoe
                    | GatherState::FarmerPlant
                    | GatherState::FarmerHarvest
                    | GatherState::FishermanStartFishing
            ) | AgriState::FloristHarvest
                | AgriState::FloristCompost
        )
    }

    /// L'état est-il un état de base — c'est-à-dire appartient-il à
    /// `crate::ai::worker::register_base` plutôt qu'à une table de métier ?
    pub fn is_base(self) -> bool {
        self.worker().is_some()
    }

    /// L'état est-il un des deux que ce module a portés ?
    pub fn is_florist(self) -> bool {
        matches!(self, AgriState::FloristHarvest | AgriState::FloristCompost)
    }
}

impl fmt::Display for AgriState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.java())
    }
}

/// Les quatre phases d'un but, plus la cinquième que Java a et que la mission ne nomme
/// pas.
///
/// **Ce n'est pas un état.** Java ne change pas d'état pour marcher : il reste dans
/// `FARMER_HOE` et rend `getState()` tant que `walkToSafePos` répond faux
/// (`EntityAIWorkFarmer.java:529-533`). La phase se lit donc sur l'état **et** sur la
/// réponse du déplacement, ce que [`super::cycle::phase_of`] fait en une fonction pure.
///
/// | Phase | Ce que fait le citoyen | Ce qui la décide |
/// | --- | --- | --- |
/// | [`Phase::Search`] | cherche quoi faire | `decide`, `prepareForFarming`, `findWater` |
/// | [`Phase::Travel`] | marche vers le travail | `walkTo…` a répondu faux ce tick |
/// | [`Phase::Act`] | agit sur le bloc | `mineBlock`, `compost`, la ligne lancée |
/// | [`Phase::Deliver`] | vide ou prend dans le coffre de la hutte | `inventoryNeedsDump`, `GATHERING_REQUIRED_MATERIALS` |
/// | [`Phase::Wait`] | attend le joueur ou une requête | `PAUSED`, `NEEDS_ITEM` |
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive : les quatre phases de la mission plus `Wait`, que Java a et qui n'est
// aucune des quatre (un citoyen en pause ne cherche, ne marche, n'agit ni ne vide)
pub enum Phase {
    /// Chercher : choisir le travail du tick.
    Search,
    /// Aller : le déplacement du tick n'est pas fini.
    Travel,
    /// Agir : le citoyen est sur place et l'action se fait.
    Act,
    /// Rendre : le transfert avec les coffres de la hutte, dans un sens ou dans l'autre.
    Deliver,
    /// Attendre : `PAUSED` ou `NEEDS_ITEM`, deux états où le citoyen ne décide rien.
    Wait,
}

impl Phase {
    /// Les cinq phases, dans l'ordre d'un cycle complet.
    pub const ALL: [Phase; 5] = [
        Phase::Search,
        Phase::Travel,
        Phase::Act,
        Phase::Deliver,
        Phase::Wait,
    ];

    /// Le nom de la phase, pour une trace de test.
    pub fn name(self) -> &'static str {
        match self {
            Phase::Search => "search",
            Phase::Travel => "travel",
            Phase::Act => "act",
            Phase::Deliver => "deliver",
            Phase::Wait => "wait",
        }
    }

    /// La ligne Java qui montre la phase dans le code — celle que la phase replie.
    pub fn java(self) -> &'static str {
        match self {
            Phase::Search => "EntityAIWorkFlorist.java:151",
            Phase::Travel => "EntityAIWorkFlorist.java:223",
            Phase::Act => "EntityAIWorkFlorist.java:272",
            Phase::Deliver => "AbstractEntityAIBasic.java:225",
            Phase::Wait => "AbstractEntityAIBasic.java:251",
        }
    }
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Ce qu'un but **demande au monde**, quand il ne se contente pas de changer d'état.
///
/// Java référence : les trois effets que le fleuriste déclenche et que le port ne peut
/// pas exécuter lui-même — une demande de compost (`EntityAIWorkFlorist.java:183`), la
/// croissance d'une terre (`:236`) et la cueillette d'une fleur (`:272-282`). Le port
/// les rend à l'appelant : une décision qui n'exécute rien est une décision testable.
///
/// Une liste, pas un seul élément : `decide` peut créer une requête **et** partir
/// cueillir dans le même tick (`:183`, puis `:187-191`).
#[derive(Clone, Debug, PartialEq)]
// exhaustive : les trois ordres que ce module nomme
pub enum AgriWork {
    /// `checkIfRequestForItemExistOrCreateAsync(new ItemStack(ModItems.compost, …))`
    /// (`EntityAIWorkFlorist.java:183`).
    RequestCompost {
        /// `COMPOST_REQUEST_QTY` (`:70`), seize.
        quantity: u32,
    },
    /// `TileEntityCompostedDirt.compost(chance, flower)`
    /// (`EntityAIWorkFlorist.java:236`).
    Compost {
        /// La terre à composter.
        position: crate::ai::facts::Pos,
        /// `building.getFlowerToGrow()` (`:231`) : l'identifiant d'objet à faire pousser.
        flower: String,
        /// `PERCENT_CHANGE_FOR_GROWTH - buildingLevel * 0.01` (`:236`).
        chance: f64,
    },
    /// Une fleur à cueillir : `mineBlock(harvestPosition)` (`:272`), l'expérience
    /// (`:277`) et la statistique (`:279-282`).
    Harvest {
        /// La position de la fleur, c'est-à-dire `pos.above()` de la terre (`:354`).
        position: crate::ai::facts::Pos,
        /// `XP_PER_FLOWER` (`:96`), deux points.
        experience: f64,
    },
    /// `building.removePlantableGround(pos)` (`:381`, et `WorkerUtil.java:326` pour
    /// l'autre scan) : la terre n'est plus une terre du fleuriste, la hutte l'oublie.
    RemovePlantableGround {
        /// La position à retirer de la liste de la hutte.
        position: crate::ai::facts::Pos,
    },
}

/// Ce qu'une action de métier a décidé : l'état qu'elle rend et la branche qui a refusé
/// — la même forme que `crate::ai_gather::Decision`, pour un [`AgriState`].
///
/// Java référence : `IAIState` rendu par `decide` / `compost` / `harvest`
/// (`EntityAIWorkFlorist.java:151`, `:214`, `:256`). `next: None` est Java's
/// `return getState()` : l'action a tourné, l'état ne change pas, et le tick continue
/// (`BasicStateMachine.java:199-236`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AgriDecision {
    /// L'état rendu, `None` pour « rester où on est ».
    pub next: Option<AgriState>,
    /// Pourquoi l'action s'est arrêtée, quand elle s'est arrêtée.
    pub refusal: Option<AgriRefusal>,
    /// Ce que la décision demande au monde.
    pub works: Vec<AgriWork>,
}

impl AgriDecision {
    /// Aller dans `state`.
    pub fn go(state: AgriState) -> AgriDecision {
        AgriDecision {
            next: Some(state),
            refusal: None,
            works: Vec::new(),
        }
    }

    /// `return getState()` : aucun changement d'état, aucune plainte.
    pub fn stay() -> AgriDecision {
        AgriDecision::refuse_without_state(None)
    }

    /// Rester, en nommant la raison.
    pub fn refuse(refusal: AgriRefusal) -> AgriDecision {
        AgriDecision::refuse_without_state(Some(refusal))
    }

    /// Aller dans `state` et nommer la raison.
    pub fn go_after(state: AgriState, refusal: AgriRefusal) -> AgriDecision {
        AgriDecision {
            next: Some(state),
            refusal: Some(refusal),
            works: Vec::new(),
        }
    }

    fn refuse_without_state(refusal: Option<AgriRefusal>) -> AgriDecision {
        AgriDecision {
            next: None,
            refusal,
            works: Vec::new(),
        }
    }

    /// Ajouter un ordre au monde.
    pub fn with_work(mut self, work: AgriWork) -> AgriDecision {
        self.works.push(work);
        self
    }

    /// Ajouter une raison à une décision qui n'en porte pas encore.
    pub fn because(mut self, refusal: AgriRefusal) -> AgriDecision {
        if self.refusal.is_none() {
            self.refusal = Some(refusal);
        }
        self
    }
}

/// Pourquoi un but de métier n'a pas pu avancer. Chaque variante porte sa ligne.
///
/// Java référence : les branches de sortie de `decide`, `compost` et `harvest`
/// (`EntityAIWorkFlorist.java:151-288`), plus les refus déjà nommés par
/// `crate::ai_gather::GatherRefusal` pour le fermier et le pêcheur — que ce module
/// réutilise au lieu de les réécrire ([`AgriRefusal::Gather`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive : les refus du fleuriste et du cycle, plus ceux de `ai_gather`
pub enum AgriRefusal {
    /// Un refus que `crate::ai_gather` nomme déjà pour le fermier ou le pêcheur.
    Gather(crate::ai_gather::GatherRefusal),
    /// `building.getPlantGround().isEmpty()` : la hutte n'a aucune terre à travailler,
    /// le joueur est prévenu (`NO_PLANT_GROUND_FLORIST`) et le fleuriste s'arrête
    /// (`EntityAIWorkFlorist.java:154-158`).
    NoPlantGround,
    /// `checkOrEquipShears()` a répondu faux : les ciseaux manquent et une requête est
    /// partie (ou le joueur doit en fournir) — `:160-163`, `:307-321`.
    ShearsMissing,
    /// `distance > MAX_DISTANCE && !walkToBuilding()` : le fleuriste est trop loin de sa
    /// hutte et rentre avant de jardiner (`:166-170`).
    NotAtHut,
    /// Le compost est dans les coffres de la hutte mais pas dans l'inventaire :
    /// `GATHERING_REQUIRED_MATERIALS` (`:172-180`).
    CompostInHut,
    /// Aucun compost nulle part : une requête de `COMPOST_REQUEST_QTY` est créée et le
    /// fleuriste continue avec ce que le sol donne déjà (`:181-184`).
    CompostRequested,
    /// Aucun compost porté et aucune terre déjà compostée : le joueur est prévenu
    /// (`NO_COMPOST`) (`:193-198`).
    NoCompost,
    /// Toute la terre est déjà compostée : `getFirstNotCompostedLand` a rendu `null`
    /// (`:204`, puis `:216-219`).
    NoGroundToCompost,
    /// La terre visée n'est plus dans la liste de la hutte : plus rien à faire dessus.
    GroundGone,
    /// `world.getBlockEntity(pos)` n'est pas un `TileEntityCompostedDirt` : le bloc à
    /// composter n'est pas un bloc à composter (`:229`).
    NotCompostedDirt,
    /// `building.getFlowerToGrow()` est vide : le joueur est prévenu
    /// (`NO_FLOWERS_IN_CONFIG`) (`:231-232`, `:239-242`).
    NoFlowersInConfig,
    /// Plus une seule unité de compost dans l'inventaire au moment d'en poser une :
    /// `shrinkItemCountInItemHandler` aurait répondu faux (`:234`).
    CompostGoneWhileComposting,
    /// Aucune position de cueillette retenue : `harvestPosition == null`
    /// (`:258-261`).
    NoFlowerToHarvest,
    /// `walkToWorkPos(...)` a répondu faux : le citoyen est en route, le tick ne fait
    /// rien d'autre (`:223-226`, `:265-268`).
    NotAtTarget,
    /// `mineBlock(...)` n'a pas fini — Java demande de sortir du tick et de rappeler
    /// (`AbstractEntityAIInteract.java:141-149`, `EntityAIWorkFlorist.java:272-275`).
    MiningIncomplete,
    /// Le transfert avec le coffre de la hutte n'est pas fini : `walkToWorkPos` en cours
    /// (`AbstractEntityAIBasic.java:1230-1235`).
    TransferIncomplete,
    /// **Pas un appel Java.** Les ciseaux se sont cassés pendant le tick
    /// (`CitizenItemUtils.damageItemInHand`, `EntityAIWorkFlorist.java:294-298`) : Java
    /// ne distingue pas « cassé » de « jamais eu », et le port demande la réponse au
    /// appelant plutôt que de la deviner.
    ToolBroken,
    /// La scène n'est pas celle du métier qui tique — une erreur d'appelant, nommée pour
    /// ne jamais ressembler à « rien à faire ».
    SceneForAnotherTrade,
}

impl AgriRefusal {
    /// La ligne Java qui décide ce refus.
    pub fn java(self) -> &'static str {
        match self {
            AgriRefusal::Gather(refusal) => refusal.java(),
            AgriRefusal::NoPlantGround => "EntityAIWorkFlorist.java:154",
            AgriRefusal::ShearsMissing => "EntityAIWorkFlorist.java:160",
            AgriRefusal::NotAtHut => "EntityAIWorkFlorist.java:167",
            AgriRefusal::CompostInHut => "EntityAIWorkFlorist.java:176",
            AgriRefusal::CompostRequested => "EntityAIWorkFlorist.java:183",
            AgriRefusal::NoCompost => "EntityAIWorkFlorist.java:195",
            AgriRefusal::NoGroundToCompost => "EntityAIWorkFlorist.java:216",
            AgriRefusal::GroundGone => "EntityAIWorkFlorist.java:228",
            AgriRefusal::NotCompostedDirt => "EntityAIWorkFlorist.java:229",
            AgriRefusal::NoFlowersInConfig => "EntityAIWorkFlorist.java:231",
            AgriRefusal::CompostGoneWhileComposting => "EntityAIWorkFlorist.java:234",
            AgriRefusal::NoFlowerToHarvest => "EntityAIWorkFlorist.java:258",
            AgriRefusal::NotAtTarget => "EntityAIWorkFlorist.java:223",
            AgriRefusal::MiningIncomplete => "EntityAIWorkFlorist.java:272",
            AgriRefusal::TransferIncomplete => "AbstractEntityAIBasic.java:1230",
            AgriRefusal::ToolBroken => "EntityAIWorkFlorist.java:294",
            AgriRefusal::SceneForAnotherTrade => "EntityAIWorkFlorist.java:151",
        }
    }

    /// La clé stable du message que le joueur lira, à côté de
    /// `crate::ai_gather::GatherRefusal::key`.
    pub fn key(self) -> &'static str {
        match self {
            AgriRefusal::Gather(refusal) => refusal.key(),
            AgriRefusal::NoPlantGround => "ai_work_08.refusal.no_plant_ground",
            AgriRefusal::ShearsMissing => "ai_work_08.refusal.shears_missing",
            AgriRefusal::NotAtHut => "ai_work_08.refusal.not_at_hut",
            AgriRefusal::CompostInHut => "ai_work_08.refusal.compost_in_hut",
            AgriRefusal::CompostRequested => "ai_work_08.refusal.compost_requested",
            AgriRefusal::NoCompost => "ai_work_08.refusal.no_compost",
            AgriRefusal::NoGroundToCompost => "ai_work_08.refusal.no_ground_to_compost",
            AgriRefusal::GroundGone => "ai_work_08.refusal.ground_gone",
            AgriRefusal::NotCompostedDirt => "ai_work_08.refusal.not_composted_dirt",
            AgriRefusal::NoFlowersInConfig => "ai_work_08.refusal.no_flowers_in_config",
            AgriRefusal::CompostGoneWhileComposting => "ai_work_08.refusal.compost_gone",
            AgriRefusal::NoFlowerToHarvest => "ai_work_08.refusal.no_flower_to_harvest",
            AgriRefusal::NotAtTarget => "ai_work_08.refusal.not_at_target",
            AgriRefusal::MiningIncomplete => "ai_work_08.refusal.mining_incomplete",
            AgriRefusal::TransferIncomplete => "ai_work_08.refusal.transfer_incomplete",
            AgriRefusal::ToolBroken => "ai_work_08.refusal.tool_broken",
            AgriRefusal::SceneForAnotherTrade => "ai_work_08.refusal.scene_for_another_trade",
        }
    }
}

impl From<crate::ai_gather::GatherRefusal> for AgriRefusal {
    fn from(refusal: crate::ai_gather::GatherRefusal) -> AgriRefusal {
        AgriRefusal::Gather(refusal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_florist_states_are_the_only_ones_this_module_adds() {
        let added: Vec<AgriState> = AgriState::ALL
            .iter()
            .copied()
            .filter(|state| state.is_florist())
            .collect();
        assert_eq!(added, vec![AgriState::FloristHarvest, AgriState::FloristCompost]);
        // Les vingt-neuf autres viennent de `ai_gather`, et le port le dit.
        assert_eq!(AgriState::ALL.len(), 29 + added.len());
        assert!(AgriState::ALL
            .iter()
            .filter(|state| !state.is_florist())
            .all(|state| state.gather().is_some()));
    }

    #[test]
    fn every_state_has_a_java_identifier_and_a_phase() {
        for state in AgriState::ALL {
            assert!(!state.java().is_empty(), "{state:?} has no Java name");
            // La phase est une lecture de l'état : elle ne dépend pas du métier.
            assert!(Phase::ALL.contains(&super::super::cycle::phase_of(
                state,
                super::super::GoalAnswers::default()
            )));
        }
    }

    #[test]
    fn the_eat_flag_follows_the_java_constant() {
        // `AIWorkerState.java:76` — `FISHERMAN_START_FISHING(false)`.
        assert!(!AgriState::Gather(GatherState::FishermanStartFishing).okay_to_eat());
        // `:585`, `:590` — les deux constantes du fleuriste valent `true`.
        assert!(AgriState::FloristHarvest.okay_to_eat());
        assert!(AgriState::FloristCompost.okay_to_eat());
        // `:23` — `INVENTORY_FULL(false)`.
        assert!(!AgriState::Gather(GatherState::Base(WorkerState::InventoryFull)).okay_to_eat());
        // `:15` — `IDLE(true)`.
        assert!(AgriState::Gather(GatherState::Base(WorkerState::Idle)).okay_to_eat());
    }

    #[test]
    fn only_the_six_acting_states_are_work() {
        let work: Vec<AgriState> = AgriState::ALL
            .iter()
            .copied()
            .filter(|state| state.is_work())
            .collect();
        assert_eq!(
            work,
            vec![
                AgriState::Gather(GatherState::FarmerHoe),
                AgriState::Gather(GatherState::FarmerPlant),
                AgriState::Gather(GatherState::FarmerHarvest),
                AgriState::Gather(GatherState::FishermanStartFishing),
                AgriState::FloristHarvest,
                AgriState::FloristCompost,
            ]
        );
    }

    #[test]
    fn a_decision_without_a_state_is_java_returning_get_state() {
        let stay = AgriDecision::stay();
        assert_eq!(stay.next, None);
        assert_eq!(stay.refusal, None);
        assert!(stay.works.is_empty());

        let refuse = AgriDecision::refuse(AgriRefusal::NotAtTarget);
        assert_eq!(refuse.next, None);
        assert_eq!(refuse.refusal, Some(AgriRefusal::NotAtTarget));
    }

    #[test]
    fn a_decision_can_ask_for_two_things_at_once() {
        // `EntityAIWorkFlorist.java:183` puis `:187-191` : la requête de compost et la
        // cueillette partent du même tick.
        let decision = AgriDecision::go(AgriState::FloristHarvest)
            .with_work(AgriWork::RequestCompost { quantity: 16 })
            .with_work(AgriWork::Harvest {
                position: crate::ai::facts::Pos::new(1, 64, 2),
                experience: 2.0,
            });
        assert_eq!(decision.works.len(), 2);
        assert_eq!(decision.refusal, None);
    }

    #[test]
    fn a_refusal_keeps_the_line_that_decides_it() {
        for refusal in [
            AgriRefusal::NoPlantGround,
            AgriRefusal::ShearsMissing,
            AgriRefusal::NoCompost,
            AgriRefusal::ToolBroken,
        ] {
            assert!(refusal.java().contains(':'), "{refusal:?} has no line");
            assert!(refusal.key().starts_with("ai_work_08.refusal."));
        }
    }
}
