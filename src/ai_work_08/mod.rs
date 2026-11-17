//! Les buts d'IA des trois métiers de `core/entity/ai/workers/production/agriculture/` :
//! le fermier, le pêcheur et le fleuriste.
//!
//! Java référence, une classe par métier : `EntityAIWorkFarmer.java` (921 lignes),
//! `EntityAIWorkFisherman.java` (733) et `EntityAIWorkFlorist.java` (416), avec leurs
//! huttes (`BuildingFarmer`, `BuildingFisherman`, `BuildingFlorist`) et les blocs qu'elles
//! posent (`FarmField`, `TileEntityCompostedDirt`).
//!
//! | Métier | Table | États | Où elle vit |
//! | --- | --- | ---: | --- |
//! | fermier | `EntityAIWorkFarmer.java:136-142` | 3 + 4 de base | `crate::ai_gather::farmer` |
//! | pêcheur | `EntityAIWorkFisherman.java:168-176` | 4 + 4 de base | `crate::ai_gather::fisherman` |
//! | fleuriste | `EntityAIWorkFlorist.java:128-134` | 2 + 6 de base | **ici**, [`florist`] |
//!
//! ## Ce que ce module ajoute, et ce qu'il ne refait pas
//!
//! **Deux des trois tables existaient déjà.** `crate::ai_gather` porte le fermier et le
//! pêcheur — leurs états, leurs faits, leurs refus, leurs délais — et ce module les
//! réutilise tels quels : [`AgriAi::new`] répond [`AgriAi::Gather`] pour ces deux
//! métiers et tique la machine existante. Réécrire `FARMER_HOE` ici aurait été la
//! deuxième copie d'une table déjà citée à sa ligne Java. Le troisième métier, lui,
//! n'existait nulle part : [`florist`] est sa table, complète.
//!
//! **Ce qui manquait aux trois, et qui est ici.** Un but n'est pas seulement une liste de
//! cibles : c'est un **cycle** — chercher, aller, agir, rendre. Java l'écrit dans ses
//! classes de base et le replie dans ses états (`walkToSafePos` rend faux → `return
//! getState()`), ce qui rend la lecture d'une exécution pénible : on ne voit pas si le
//! citoyen travaille ou s'il marche. [`Phase`] nomme les quatre phases (et la cinquième,
//! `WAIT`, que Java a et que le brief ne nomme pas), [`cycle::phase_of`] les lit sur
//! `(état, réponses du tick)`, et [`AgriTrace`] garde la suite pour qu'un test compare un
//! scénario à la suite que Java produit.
//!
//! **Ce que ce module ne fait pas.** Il ne marche pas, ne casse pas de bloc, ne fait pas
//! pousser de fleur, ne déplace pas d'objet : les actions du monde sortent sous forme
//! d'ordres ([`AgriWork`]) et de refus nommés ([`AgriRefusal`]), et les réponses du monde
//! entrent sous forme de faits ([`GoalAnswers`], [`FloristFacts`]). C'est ce qui rend une
//! décision rejouable — et c'est la seule façon de vérifier qu'elle suit Java.
//!
//! ```ignore
//! let mut ai = AgriAi::new(AgriTrade::Florist);
//! let mut scene = AgriScene::florist_with(
//!     WorldFacts::new(),
//!     FloristFacts::ready(2)
//!         .with_ground(PlantGround::with_flower(Pos::new(3, 64, 3)))
//!         .with_compost(4),
//! );
//! let outcome = ai.tick(&mut scene, GoalAnswers::default());
//! // START_WORKING : le fleuriste passe par DECIDE au bout de vingt ticks.
//! assert_eq!(outcome.phase, Phase::Search);
//! ```
//!
//! ## Le cycle en une phrase
//!
//! `IDLE` → `START_WORKING` → `DECIDE` (chercher) → `FLORIST_HARVEST` /
//! `FLORIST_COMPOST` / `GATHERING_REQUIRED_MATERIALS` (aller puis agir) → `START_WORKING`,
//! avec un `INVENTORY_FULL` qui peut interrompre n'importe quoi pour rendre le butin à la
//! hutte (`AbstractEntityAIBasic.java:225`, `:226`). Le fermier et le pêcheur suivent le
//! même cycle avec leurs propres états, et [`cycle::phase_of`] le lit de la même façon.

pub mod cycle;
pub mod facts;
pub mod florist;
pub mod state;

#[cfg(test)]
mod tests;

use core::fmt;

use crate::ai::facts::WorldFacts;
use crate::ai::state::Fired;
use crate::ai_gather::{GatherAi, GatherState};

pub use cycle::{
    AgriTrace, Duration, TradeThreshold, durations, entry_java, entry_state, gather_trade_of,
    inventory_needs_dump, phase_of, threshold,
};
pub use facts::{
    AgriMemory, AgriScene, FloristFacts, FloristScene, GoalAnswers, PlantGround,
};
pub use florist::{FloristAi, FloristTick, GroundScan};
pub use state::{AgriDecision, AgriRefusal, AgriState, AgriWork, Phase};

/// Les trois métiers de `production/agriculture/` que ce module porte.
///
/// Java référence : les trois classes du paquet, et les trois `Job` qu'elles animent
/// (`core/colony/jobs/JobFarmer.java`, `JobFisherman.java`, `JobFlorist.java`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive : les trois métiers du paquet `production/agriculture/`
pub enum AgriTrade {
    /// `EntityAIWorkFarmer` (`production/agriculture/EntityAIWorkFarmer.java:75`).
    Farmer,
    /// `EntityAIWorkFisherman` (`:65`).
    Fisherman,
    /// `EntityAIWorkFlorist` (`:48`).
    Florist,
}

impl AgriTrade {
    /// Les trois métiers, dans l'ordre où le jeu les déclare.
    pub const ALL: [AgriTrade; 3] = [AgriTrade::Farmer, AgriTrade::Fisherman, AgriTrade::Florist];

    /// Le nom de la classe Java, pour une trace.
    pub fn class(self) -> &'static str {
        match self {
            AgriTrade::Farmer => "EntityAIWorkFarmer",
            AgriTrade::Fisherman => "EntityAIWorkFisherman",
            AgriTrade::Florist => "EntityAIWorkFlorist",
        }
    }

    /// Le nom du métier, pour une clé de message.
    pub fn name(self) -> &'static str {
        match self {
            AgriTrade::Farmer => "farmer",
            AgriTrade::Fisherman => "fisherman",
            AgriTrade::Florist => "florist",
        }
    }

    /// Le `Job` de `crate::jobs` que ce métier anime (`jobs/job.rs:102-113`).
    pub fn job(self) -> crate::jobs::job::Job {
        match self {
            AgriTrade::Farmer => crate::jobs::job::Job::Farmer,
            AgriTrade::Fisherman => crate::jobs::job::Job::Fisherman,
            AgriTrade::Florist => crate::jobs::job::Job::Florist,
        }
    }

    /// Le métier de `crate::ai_gather` qui porte déjà la table, pour le fermier et le
    /// pêcheur.
    pub fn gather_trade(self) -> Option<crate::ai_gather::GatherTrade> {
        cycle::gather_trade_of(self)
    }

    /// La scène vide de ce métier, sur `world`.
    pub fn scene(self, world: WorldFacts) -> AgriScene {
        match self {
            AgriTrade::Farmer => AgriScene::farmer(world),
            AgriTrade::Fisherman => AgriScene::fisherman(world),
            AgriTrade::Florist => AgriScene::florist(world),
        }
    }

    /// Les durées de travail du métier, chaque constante citée à sa ligne.
    pub fn durations(self) -> Vec<Duration> {
        cycle::durations(self)
    }

    /// Le seuil de vidage du métier (`getActionsDoneUntilDumping`).
    pub fn threshold(self) -> TradeThreshold {
        cycle::threshold(self)
    }

    /// L'état par lequel `IDLE` quitte le métier au premier tick.
    pub fn entry_state(self) -> AgriState {
        cycle::entry_state(self)
    }

    /// La ligne qui enregistre cette entrée.
    pub fn entry_java(self) -> &'static str {
        cycle::entry_java(self)
    }
}

impl fmt::Display for AgriTrade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Le but d'un métier : la machine de `crate::ai_gather` pour le fermier et le pêcheur,
/// celle de ce module pour le fleuriste.
///
/// **Une seule façade, deux moteurs.** `Machine<S, F>` est monomorphique en son état et
/// ses faits : un fermier tique une `Machine<GatherState, GatherScene>` qui existe déjà,
/// le fleuriste une `Machine<AgriState, FloristScene>`. Les deux répondent la même chose —
/// un [`AgriOutcome`] — et c'est [`Phase`] qui donne aux deux la même lecture.
#[derive(Debug)]
// exhaustive : les deux moteurs, celui de `ai_gather` et celui du fleuriste
pub enum AgriAi {
    /// Le fermier ou le pêcheur : la machine de `crate::ai_gather`, inchangée.
    Gather(GatherAi),
    /// Le fleuriste : la machine de [`florist`].
    Florist(FloristAi),
}

impl AgriAi {
    /// Le but d'un métier, dans `IDLE`, à `ENTITY_AI_TICKRATE`.
    pub fn new(trade: AgriTrade) -> AgriAi {
        match trade {
            AgriTrade::Farmer => AgriAi::Gather(GatherAi::new(crate::ai_gather::GatherTrade::Farmer)),
            AgriTrade::Fisherman => {
                AgriAi::Gather(GatherAi::new(crate::ai_gather::GatherTrade::Fisherman))
            }
            AgriTrade::Florist => AgriAi::Florist(FloristAi::new()),
        }
    }

    /// Le métier que ce but anime, quand c'est un des trois de ce module.
    ///
    /// `crate::ai_gather` porte aussi le bûcheron et le mineur ; un `AgriAi` ne les
    /// construit pas (`new` n'a que trois entrées), et un appelant qui aurait tiqué leur
    /// machine obtient `None` plutôt qu'un des trois noms par défaut.
    pub fn trade(&self) -> Option<AgriTrade> {
        match self {
            AgriAi::Gather(ai) => match ai.trade() {
                crate::ai_gather::GatherTrade::Farmer => Some(AgriTrade::Farmer),
                crate::ai_gather::GatherTrade::Fisherman => Some(AgriTrade::Fisherman),
                crate::ai_gather::GatherTrade::Lumberjack
                | crate::ai_gather::GatherTrade::Miner => None,
            },
            AgriAi::Florist(_) => Some(AgriTrade::Florist),
        }
    }

    /// L'état courant, dans le vocabulaire de ce module.
    pub fn state(&self) -> AgriState {
        match self {
            AgriAi::Gather(ai) => AgriState::Gather(ai.state()),
            AgriAi::Florist(ai) => ai.state(),
        }
    }

    /// L'état piège du dernier tick, si une table est allée quelque part sans sortie
    /// (`BasicStateMachine.java:220-228`).
    pub fn missing(&self) -> Option<crate::ai::state::MissingTransition<AgriState>> {
        match self {
            AgriAi::Gather(ai) => ai.missing().map(|missing| {
                crate::ai::state::MissingTransition {
                    state: AgriState::Gather(missing.state),
                    rule: missing.rule,
                }
            }),
            AgriAi::Florist(ai) => ai.missing(),
        }
    }

    /// `AbstractAISkeleton.resetAI` (`:145-152`).
    pub fn reset(&mut self) {
        match self {
            AgriAi::Gather(ai) => ai.reset(),
            AgriAi::Florist(ai) => ai.reset(),
        }
    }

    /// Un tick, avec les réponses du monde pour ce tick.
    ///
    /// La scène doit être celle du métier : une scène de fleuriste donnée à un fermier
    /// répond [`AgriRefusal::SceneForAnotherTrade`] au lieu de deviner.
    pub fn tick(&mut self, scene: &mut AgriScene, answers: GoalAnswers) -> AgriOutcome {
        scene.set_answers(answers);
        match (self, scene) {
            (AgriAi::Gather(ai), AgriScene::Gather(gather)) => {
                let tick = ai.tick(gather.as_mut());
                let state = AgriState::Gather(tick.state);
                AgriOutcome {
                    state,
                    phase: phase_of(state, answers),
                    fired: tick.fired.map(wrap_fired),
                    refusal: tick.refusal.map(AgriRefusal::Gather),
                    works: Vec::new(),
                }
            }
            (AgriAi::Florist(ai), AgriScene::Florist(florist)) => {
                let tick = ai.tick(florist.as_mut());
                let state = ai.state();
                AgriOutcome {
                    state,
                    phase: phase_of(state, answers),
                    fired: tick.fired,
                    refusal: tick.refusal,
                    works: tick.works,
                }
            }
            (ai, _) => {
                let state = ai.state();
                AgriOutcome {
                    state,
                    phase: phase_of(state, answers),
                    fired: None,
                    refusal: Some(AgriRefusal::SceneForAnotherTrade),
                    works: Vec::new(),
                }
            }
        }
    }

    /// Tiquer `ticks` fois et garder la suite d'états et de phases.
    ///
    /// **Le test qui compte.** Le monde ne change pas tout seul dans un scénario : c'est
    /// `per_tick` qui le fait évoluer — il reçoit la scène et le numéro du tick, et rend
    /// les réponses de ce tick. Un scénario écrit ainsi se relit comme une exécution
    /// Java, tick par tick.
    pub fn trace<F>(&mut self, scene: &mut AgriScene, ticks: usize, mut per_tick: F) -> AgriTrace
    where
        F: FnMut(&mut AgriScene, usize) -> GoalAnswers,
    {
        let mut trace = AgriTrace::new();
        for index in 0..ticks {
            let answers = per_tick(scene, index);
            let outcome = self.tick(scene, answers);
            trace.push(outcome.state, outcome.phase);
        }
        trace
    }

    /// Tiquer `ticks` fois avec les mêmes réponses à chaque tick.
    pub fn trace_constant(
        &mut self,
        scene: &mut AgriScene,
        ticks: usize,
        answers: GoalAnswers,
    ) -> AgriTrace {
        self.trace(scene, ticks, |_, _| answers)
    }
}

/// Ce qu'un tick a décidé : l'état, sa phase, la transition qui a bougé la machine, le
/// refus, et les ordres au monde.
///
/// Même forme que `crate::ai_gather::GatherOutcome` et que `crate::ai::worker::Outcome` :
/// une décision se lit en trois questions — où suis-je, pourquoi, et que faut-il faire.
#[derive(Clone, Debug, PartialEq)]
pub struct AgriOutcome {
    /// L'état en vigueur après le tick (`AbstractAISkeleton.getState`, `:120-126`).
    pub state: AgriState,
    /// La phase de cet état pour ce tick ([`cycle::phase_of`]).
    pub phase: Phase,
    /// La transition qui a fait bouger la machine, si une l'a fait.
    pub fired: Option<Fired<AgriState>>,
    /// Le refus du tick, quand il y en a un.
    pub refusal: Option<AgriRefusal>,
    /// Les ordres que le tick demande au monde ([`AgriWork`]).
    pub works: Vec<AgriWork>,
}

impl AgriOutcome {
    /// La machine a-t-elle changé d'état ? `false` couvre Java's `return getState()`, qui
    /// est une transition vers le même état (`AbstractEntityAIBasic.java:205`).
    pub fn moved(&self) -> bool {
        self.fired.map(|fired| fired.from != fired.to).unwrap_or(false)
    }

    /// La règle qui a décidé ce tick.
    pub fn rule(&self) -> Option<&'static str> {
        self.fired.map(|fired| fired.rule)
    }

    /// Pourquoi le tick s'est arrêté.
    pub fn why_not(&self) -> Option<AgriRefusal> {
        self.refusal
    }

    /// Le premier ordre au monde, quand il n'y en a qu'un.
    pub fn work(&self) -> Option<&AgriWork> {
        self.works.first()
    }
}

/// Transformer un `Fired<GatherState>` en `Fired<AgriState>` — l'état de `ai_gather` est
/// celui que ce module réutilise, le reste de la transition est recopié tel quel.
fn wrap_fired(fired: Fired<GatherState>) -> Fired<AgriState> {
    Fired {
        rule: fired.rule,
        java: fired.java,
        from: AgriState::Gather(fired.from),
        to: AgriState::Gather(fired.to),
        reset: fired.reset,
    }
}
