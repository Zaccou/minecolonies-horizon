//! Le cycle d'un but de `production/agriculture/` : les quatre phases, le vidage de
//! l'inventaire, et les durées de travail.
//!
//! Java référence : `AbstractEntityAIBasic.java:409-416` (le vidage, entré par un
//! événement `STATE_BLOCKING` à `:225` et exécuté par `dumpInventory` à `:226`),
//! `EntityAIWorkFlorist.java:324-327` (le seuil du fleuriste, `10 × niveau`),
//! `EntityAIWorkFarmer.java:198-201` (soixante-quatre blocs) et
//! `EntityAIWorkFisherman.java:225-229` (dix prises).
//!
//! **Le cycle, en une phrase.** À chaque tick, le but est dans une *phase* : il cherche
//! quoi faire, il marche vers ce qu'il a trouvé, il agit dessus, ou il rend ce qu'il a
//! ramassé (et prend ce qui lui manque) dans les coffres de la hutte. Java n'écrit pas
//! ces quatre noms : il les replie dans ses états, un déplacement n'étant qu'un
//! `return getState()` tant que `walkTo…` répond faux. [`phase_of`] est donc une
//! **lecture** de `(état, réponses du tick)` — pas un second automate, et pas une
//! invention : chaque phase cite la ligne où Java la montre.
//!
//! **Le vidage n'est pas réécrit, il est cité.** `crate::ai::worker` porte déjà
//! `inventoryNeedsDump` (`inventory_needs_dump`, `:409-416`) pour les métiers dont c'est
//! la table. Le fleuriste a une machine à part (`Machine<AgriState, FloristScene>` ne
//! peut pas contenir `Machine<WorkerState, Scene>` : le moteur est monomorphique en ses
//! deux paramètres), donc il lui faut sa règle — et cette règle est la même, avec le
//! seuil du métier. [`inventory_needs_dump`] la donne pour le fleuriste, et son seuil
//! est la seule chose qui change ([`TradeThreshold`] dit où vit celui de chacun).

use crate::ai::state::WorkerState;
use crate::ai_gather::{GatherState, GatherTrade};

use super::facts::{FloristScene, GoalAnswers};
use super::state::{AgriState, Phase};
use super::{AgriTrade, florist};

/// La phase d'un but pendant ce tick : une lecture de son état et des réponses du
/// déplacement.
///
/// **Les six états qui agissent.** `FARMER_HOE`/`PLANT`/`HARVEST`
/// (`EntityAIWorkFarmer.java:138-140`), `FISHERMAN_START_FISHING`
/// (`EntityAIWorkFisherman.java:175`), `FLORIST_HARVEST`/`COMPOST`
/// (`EntityAIWorkFlorist.java:132-133`). Java les laisse tels quels pendant le
/// déplacement — `if (!walkToSafePos(...)) { return getState(); }`
/// (`EntityAIWorkFarmer.java:529-533`) — donc c'est [`GoalAnswers::at_target`] qui
/// tranche entre [`Phase::Travel`] et [`Phase::Act`].
///
/// **Les douze états que ce module ne porte pas.** `LUMBERJACK_*` et `MINER_*`
/// appartiennent à deux métiers dont les tables sont dans `crate::ai_gather`. La phase
/// se lit quand même, avec les mêmes quatre noms, mais **aucune règle de ce module n'en
/// dépend** : c'est une réponse de lecture, pas une règle de plus.
pub fn phase_of(state: AgriState, answers: GoalAnswers) -> Phase {
    match state {
        // Les six états dont le corps fait une action du monde.
        AgriState::Gather(
            GatherState::FarmerHoe
            | GatherState::FarmerPlant
            | GatherState::FarmerHarvest
            | GatherState::FishermanStartFishing,
        )
        | AgriState::FloristHarvest
        | AgriState::FloristCompost => {
            if answers.at_target {
                Phase::Act
            } else {
                Phase::Travel
            }
        }
        // `FISHERMAN_WALKING_TO_WATER` (`EntityAIWorkFisherman.java:174`) : ici le
        // déplacement **est** un état, `walkToWater` (`:312-317`) ne faisant que le
        // retenir.
        AgriState::Gather(GatherState::FishermanWalkingToWater) => Phase::Travel,
        // Le vidage part, la prise de compost revient : deux transferts avec les coffres
        // de la hutte, un seul nom (`:225`, `:310`).
        AgriState::Gather(GatherState::Base(WorkerState::InventoryFull)) => Phase::Deliver,
        AgriState::Gather(GatherState::Base(WorkerState::GatheringRequiredMaterials)) => {
            Phase::Deliver
        }
        // Deux états où le citoyen ne décide rien : la requête n'est pas servie, ou le
        // joueur l'a mis en pause (`AbstractEntityAIBasic.java:251`).
        AgriState::Gather(GatherState::Base(WorkerState::NeedsItem))
        | AgriState::Gather(GatherState::Base(WorkerState::Paused)) => Phase::Wait,
        // `LUMBERJACK_NO_TREES_FOUND` attend le prochain passage
        // (`EntityAIWorkLumberjack.java:864-874`) ; les deux cueillettes ramassent au
        // sol, c'est un transfert vers l'inventaire.
        AgriState::Gather(GatherState::LumberjackNoTreesFound) => Phase::Wait,
        AgriState::Gather(GatherState::LumberjackGathering | GatherState::LumberjackGathering2) => {
            Phase::Deliver
        }
        // Les états de chantier : ils agissent, comme les six du haut, mais le
        // déplacement du bûcheron et du mineur est déjà porté par les leurs.
        AgriState::Gather(
            GatherState::LumberjackChopTree
            | GatherState::MinerMiningShaft
            | GatherState::MinerMiningNode
            | GatherState::MinerBuildingShaft
            | GatherState::MinerRepairingLadder,
        ) => {
            if answers.at_target {
                Phase::Act
            } else {
                Phase::Travel
            }
        }
        AgriState::Gather(GatherState::MinerWalkingToLadder) => Phase::Travel,
        // Tout le reste cherche : `IDLE`, `START_WORKING`, `PREPARING`, `DECIDE`,
        // `MINE_BLOCK`, `LOAD_STRUCTURE`, `BUILDING_STEP`, et les deux recherches du
        // bûcheron et du mineur.
        _ => Phase::Search,
    }
}

/// Où vit le seuil de vidage de chaque métier, avec la ligne qui le décide.
///
/// Java référence : `AbstractEntityAIBasic.getActionsDoneUntilDumping` (`:424-428`)
/// répond trente-deux pour tout le monde, et chaque métier surcharge — le fermier à
/// soixante-quatre (`EntityAIWorkFarmer.java:198-201`), le pêcheur à dix
/// (`EntityAIWorkFisherman.java:225-229`), le fleuriste à `10 × niveau`
/// (`EntityAIWorkFlorist.java:324-327`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TradeThreshold {
    /// `MAX_BLOCKS_MINED` et `MAX_FISHES_IN_INV` : un nombre fixe.
    Fixed {
        /// Le nombre d'actions avant le vidage, pris à la constante de
        /// `crate::ai_gather` pour ne pas l'écrire deux fois.
        actions: u32,
        /// La ligne Java qui le décide.
        java: &'static str,
    },
    /// `HARVEST_ACTIONS_TO_DUMP * building.getBuildingLevel()` : proportionnel au
    /// niveau de la hutte.
    PerBuildingLevel {
        /// `HARVEST_ACTIONS_TO_DUMP` (`EntityAIWorkFlorist.java:55`), dix.
        per_level: u32,
        /// La ligne Java qui le décide.
        java: &'static str,
    },
}

impl TradeThreshold {
    /// Le seuil pour une hutte de niveau `building_level`.
    pub fn actions(self, building_level: u32) -> u32 {
        match self {
            TradeThreshold::Fixed { actions, .. } => actions,
            TradeThreshold::PerBuildingLevel { per_level, .. } => per_level * building_level,
        }
    }

    /// La ligne Java qui le décide.
    pub fn java(self) -> &'static str {
        match self {
            TradeThreshold::Fixed { java, .. } | TradeThreshold::PerBuildingLevel { java, .. } => {
                java
            }
        }
    }
}

/// Le seuil de vidage d'un métier.
pub fn threshold(trade: AgriTrade) -> TradeThreshold {
    match trade {
        // :198-201 — `return MAX_BLOCKS_MINED;`, soixante-quatre.
        AgriTrade::Farmer => TradeThreshold::Fixed {
            actions: crate::ai_gather::farmer::MAX_BLOCKS_MINED,
            java: "EntityAIWorkFarmer.java:198-201",
        },
        // :225-229 — `return MAX_FISHES_IN_INV;`, dix.
        AgriTrade::Fisherman => TradeThreshold::Fixed {
            actions: crate::ai_gather::fisherman::MAX_FISHES_IN_INV,
            java: "EntityAIWorkFisherman.java:225-229",
        },
        // :324-327 — `return HARVEST_ACTIONS_TO_DUMP * building.getBuildingLevel();`.
        AgriTrade::Florist => TradeThreshold::PerBuildingLevel {
            per_level: florist::HARVEST_ACTIONS_TO_DUMP,
            java: "EntityAIWorkFlorist.java:324-327",
        },
    }
}

/// La partie chiffrée de `inventoryNeedsDump` (`AbstractEntityAIBasic.java:412-413`)
/// pour le fleuriste : l'inventaire est plein, ou le seuil du métier est atteint.
///
/// `wantInventoryDumped()` (`:414`) n'y est pas : le fleuriste ne la surcharge pas, elle
/// répond donc toujours faux — c'est le fermier qui en a une (`EntityAIWorkFarmer.java:
/// 156-165`), et sa mémoire la porte déjà (`crate::ai_gather::FarmerMemory::
/// should_dump_inventory`).
pub fn dump_due(scene: &FloristScene) -> bool {
    // :412 — `getCitizenInventoryHandler().isInventoryFull()`.
    if scene.world.inventory.full {
        return true;
    }
    // :413 — `job.getActionsDone() >= getActionsDoneUntilDumping()`.
    scene.world.inventory.actions_done >= florist::actions_until_dumping(scene.facts.building_level)
}

/// `inventoryNeedsDump` (`AbstractEntityAIBasic.java:409-416`), en entier, pour le
/// fleuriste.
///
/// **Restituée, pas inventée.** Les cinq conditions sont celles de Java, dans son ordre :
/// l'état n'est pas déjà `INVENTORY_FULL` (`:411`), le citoyen peut être interrompu
/// (`canBeInterrupted()`, même ligne — c'est le drapeau `isOkayToEat` de l'état), une des
/// trois raisons de vider est vraie (`:412-414`), et le citoyen n'est pas livreur
/// (`:415`).
pub fn inventory_needs_dump(scene: &FloristScene, state: AgriState) -> bool {
    state != AgriState::Gather(GatherState::Base(WorkerState::InventoryFull)) // :411
        && state.okay_to_eat()                                                // canBeInterrupted()
        && dump_due(scene)                                                    // :412-414
        && !scene.world.worker.deliveryman                                    // :415
}

/// Une durée de travail : quel état, quelle règle, quelle ligne, combien de ticks.
///
/// Java référence : le troisième argument de chaque `AITarget` — `registerTargets`
/// (`EntityAIWorkFarmer.java:136-142`, `EntityAIWorkFisherman.java:168-176`,
/// `EntityAIWorkFlorist.java:128-134`) et les deux cibles de base du vidage
/// (`AbstractEntityAIBasic.java:225-226`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Duration {
    /// L'état dont la liste porte la cible, `None` pour un événement global.
    pub state: Option<AgriState>,
    /// Le nom de la règle, tel qu'une trace le montre.
    pub rule: &'static str,
    /// `fichier.java:ligne`.
    pub java: &'static str,
    /// Le nombre de ticks entre deux passages de la cible.
    pub ticks: u32,
}

/// Les durées d'un métier, dans l'ordre où Java les enregistre.
///
/// Les nombres ne sont pas réécrits : ceux du fermier et du pêcheur viennent des
/// constantes de `crate::ai_gather` (`TICKS_SECOND`, `farmer::FIELD_DELAY`), ceux du
/// fleuriste des constantes de [`super::florist`] citées à leur ligne.
pub fn durations(trade: AgriTrade) -> Vec<Duration> {
    match trade {
        // `EntityAIWorkFarmer.java:136-142`, la table du constructeur.
        AgriTrade::Farmer => vec![
            Duration {
                state: Some(AgriState::Gather(GatherState::Base(WorkerState::Preparing))),
                rule: "prepareForFarming",
                java: "EntityAIWorkFarmer.java:137",
                ticks: crate::ai_gather::TICKS_SECOND,
            },
            Duration {
                state: Some(AgriState::Gather(GatherState::FarmerHoe)),
                rule: "workAtField",
                java: "EntityAIWorkFarmer.java:138",
                ticks: crate::ai_gather::farmer::FIELD_DELAY,
            },
            Duration {
                state: Some(AgriState::Gather(GatherState::FarmerPlant)),
                rule: "workAtField",
                java: "EntityAIWorkFarmer.java:139",
                ticks: crate::ai_gather::farmer::FIELD_DELAY,
            },
            Duration {
                state: Some(AgriState::Gather(GatherState::FarmerHarvest)),
                rule: "workAtField",
                java: "EntityAIWorkFarmer.java:140",
                ticks: crate::ai_gather::farmer::FIELD_DELAY,
            },
        ],
        // `EntityAIWorkFisherman.java:168-176`, la table du constructeur.
        AgriTrade::Fisherman => vec![
            Duration {
                state: Some(AgriState::Gather(GatherState::Base(WorkerState::StartWorking))),
                rule: "startWorkingAtOwnBuilding",
                java: "EntityAIWorkFisherman.java:170",
                ticks: crate::ai_gather::TICKS_SECOND,
            },
            Duration {
                state: Some(AgriState::Gather(GatherState::Base(WorkerState::Preparing))),
                rule: "prepareForFishing",
                java: "EntityAIWorkFisherman.java:171",
                ticks: crate::ai_gather::TICKS_SECOND,
            },
            Duration {
                state: Some(AgriState::Gather(GatherState::FishermanCheckWater)),
                rule: "tryDifferentAngles",
                java: "EntityAIWorkFisherman.java:172",
                ticks: 1,
            },
            Duration {
                state: Some(AgriState::Gather(GatherState::FishermanSearchingWater)),
                rule: "findWater",
                java: "EntityAIWorkFisherman.java:173",
                ticks: crate::ai_gather::TICKS_SECOND,
            },
            Duration {
                state: Some(AgriState::Gather(GatherState::FishermanWalkingToWater)),
                rule: "getToWater",
                java: "EntityAIWorkFisherman.java:174",
                ticks: crate::ai_gather::TICKS_SECOND,
            },
            Duration {
                state: Some(AgriState::Gather(GatherState::FishermanStartFishing)),
                rule: "doFishing",
                java: "EntityAIWorkFisherman.java:175",
                ticks: crate::ai_gather::TICKS_SECOND,
            },
        ],
        // `EntityAIWorkFlorist.java:128-134`, la table du constructeur, plus les deux
        // cibles de base que ce métier franchit (`AbstractEntityAIBasic.java:225-226`)
        // et les deux états de transfert.
        AgriTrade::Florist => vec![
            Duration {
                state: Some(AgriState::Gather(GatherState::Base(WorkerState::Idle))),
                rule: "idleToStartWorking",
                java: "EntityAIWorkFlorist.java:129",
                ticks: florist::IDLE_DELAY,
            },
            Duration {
                state: Some(AgriState::Gather(GatherState::Base(WorkerState::StartWorking))),
                rule: "startWorkingToDecide",
                java: "EntityAIWorkFlorist.java:130",
                ticks: florist::START_WORKING_DELAY,
            },
            Duration {
                state: Some(AgriState::Gather(GatherState::Base(WorkerState::Decide))),
                rule: "decide",
                java: "EntityAIWorkFlorist.java:131",
                ticks: florist::DECIDE_DELAY,
            },
            Duration {
                state: Some(AgriState::FloristHarvest),
                rule: "harvest",
                java: "EntityAIWorkFlorist.java:132",
                ticks: florist::HARVEST_DELAY,
            },
            Duration {
                state: Some(AgriState::FloristCompost),
                rule: "compost",
                java: "EntityAIWorkFlorist.java:133",
                ticks: florist::COMPOST_DELAY,
            },
            Duration {
                state: None,
                rule: "inventoryNeedsDump",
                java: "AbstractEntityAIBasic.java:225",
                ticks: florist::DUMP_CHECK_DELAY,
            },
            Duration {
                state: Some(AgriState::Gather(GatherState::Base(WorkerState::InventoryFull))),
                rule: "dumpInventory",
                java: "AbstractEntityAIBasic.java:226",
                ticks: florist::DUMP_DELAY,
            },
        ],
    }
}

/// Le métier de `crate::ai_gather` qui porte les états de celui-ci, quand il y en a un.
///
/// Le fleuriste n'en a pas : ses états sont les seuls que ce module nomme.
pub fn gather_trade_of(trade: AgriTrade) -> Option<GatherTrade> {
    match trade {
        AgriTrade::Farmer => Some(GatherTrade::Farmer),
        AgriTrade::Fisherman => Some(GatherTrade::Fisherman),
        AgriTrade::Florist => None,
    }
}

/// L'état dans lequel un métier commence sa table — celui que `IDLE` quitte au premier
/// tick.
///
/// Java référence : les trois tables s'ouvrent sur `IDLE → START_WORKING`, à des lignes
/// différentes (`EntityAIWorkFlorist.java:129`, et `AbstractEntityAICrafting.java:164`
/// pour le fermier, `EntityAIWorkFisherman.java:169` pour le pêcheur) — la ligne est
/// [`entry_java`], l'état est le même.
pub fn entry_state(_trade: AgriTrade) -> AgriState {
    AgriState::Gather(GatherState::Base(WorkerState::StartWorking))
}

/// La ligne qui enregistre l'entrée d'un métier, pour le rapport d'un refus.
pub fn entry_java(trade: AgriTrade) -> &'static str {
    match trade {
        AgriTrade::Farmer => "AbstractEntityAICrafting.java:164",
        AgriTrade::Fisherman => "EntityAIWorkFisherman.java:169",
        AgriTrade::Florist => "EntityAIWorkFlorist.java:129",
    }
}

/// Une suite d'états, pour comparer un scénario à ce que Java fait.
///
/// **Le test qui compte.** Un scénario de faits produit une suite d'états ; [`AgriTrace`]
/// la garde avec la phase de chaque tick, pour que l'assertion porte sur la suite
/// entière et pas sur un tick isolé — c'est la seule façon de voir qu'un `return
/// getState()` a bien retenu le citoyen au lieu de le renvoyer chercher.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AgriTrace {
    /// Un `(état, phase)` par tick qui a changé d'état ou de phase.
    steps: Vec<(AgriState, Phase)>,
}

impl AgriTrace {
    /// Une trace vide.
    pub fn new() -> AgriTrace {
        AgriTrace::default()
    }

    /// Ajouter un tick, s'il apprend quelque chose.
    ///
    /// Un tick sans transition et sans changement de phase est Java qui attend
    /// (`TickRateStateMachine.java:114-125`) : le noter une fois suffit, le noter
    /// vingt fois ne dit rien de plus sur la décision.
    pub fn push(&mut self, state: AgriState, phase: Phase) {
        if self.steps.last() != Some(&(state, phase)) {
            self.steps.push((state, phase));
        }
    }

    /// Les états, dans l'ordre.
    pub fn states(&self) -> Vec<AgriState> {
        self.steps.iter().map(|(state, _)| *state).collect()
    }

    /// Les phases, dans l'ordre.
    pub fn phases(&self) -> Vec<Phase> {
        self.steps.iter().map(|(_, phase)| *phase).collect()
    }

    /// Les états, en identifiants Java, pour un message de test lisible.
    pub fn java(&self) -> Vec<&'static str> {
        self.states().iter().map(|state| state.java()).collect()
    }

    /// Combien de pas distincts.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// La trace est-elle vide.
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}
