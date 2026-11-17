//! Les buts d'IA de trois métiers : la pierre taillée, le tailleur de pierre, et
//! l'écolier.
//!
//! ## La prémisse du brief, mesurée
//!
//! Le brief nomme trois fichiers sources. Deux d'entre eux **ne contiennent aucune
//! machine à états** :
//!
//! | Fichier nommé | Lignes | Ce qu'il contient vraiment |
//! | --- | ---: | --- |
//! | `crafting/EntityAIWorkStoneSmeltery.java` | 27 | un constructeur qui relaie, et `getExpectedBuildingClass()` (`:22-26`) |
//! | `crafting/EntityAIWorkStonemason.java` | 28 | la même chose (`:23-27`) |
//! | `education/EntityAIWorkPupil.java` | 224 | **cinq `AITarget`**, quatre méthodes de décision, deux états neufs |
//!
//! Aucun des deux premiers n'appelle `registerTargets`, ne déclare une constante
//! numérique, ni ne surcharge `decide` : leur table est **celle de leur classe mère**.
//! La vraie source du tailleur de pierre est
//! `crafting/AbstractEntityAICrafting.java` (`:160-170`, six cibles), celle du
//! tailleur de pierre taillée est
//! `crafting/AbstractEntityAIRequestSmelter.java` (`:71-82`, sept cibles de plus).
//!
//! **Où ces deux tables sont déjà portées.** `crate::ai_build::crafter` porte
//! `AbstractEntityAICrafting` en entier (ses six cibles, les quatre sorties de
//! `getRecipe`, les sept de `craft`, la formule de durée) et c'est stable : c'est le
//! module auquel ce fichier se raccorde. La table du `RequestSmelter` est écrite par
//! des modules voisins en cours d'écriture (`src/ai_work_01`, `_02`, `_03`) ; ce
//! module **n'en ajoute pas une quatrième copie** — voir [`crafting_shop`], qui porte
//! ce que ces deux métiers ont **en propre** et rend la main pour le reste.
//!
//! ## Ce que ce module porte
//!
//! | Métier | Classe Java | Table portée ici | États | Cibles |
//! | --- | --- | --- | ---: | ---: |
//! | écolier | `EntityAIWorkPupil` | la sienne, entière ([`pupil`]) | 8 | 5 + 3 |
//! | tailleur de pierre | `EntityAIWorkStonemason` | son delta seul ([`crafting_shop`]) | 0 | 0 |
//! | pierre taillée | `EntityAIWorkStoneSmeltery` | son delta seul ([`crafting_shop`]) | 0 | 0 |
//!
//! **L'écolier est le seul des trois qui ait un but à lui**, et c'est pour ça que
//! `pupil.rs` est quatre fois plus long que `crafting_shop.rs` : la mesure du fichier
//! Java est la mesure du port.
//!
//! ## Ce que ce module ne fait jamais
//!
//! Aucun appel au monde : la décision est une fonction des faits, et ce qu'elle veut
//! faire faire au monde est une liste de demandes rendue avec elle
//! ([`pupil::PupilAsk`]). Pas d'`unwrap` non plus : la seule donnée venue de
//! l'extérieur est un tirage aléatoire, ramené dans ses bornes par `%` plutôt que par
//! une panique.
//!
//! ## Le partage avec la table de base
//!
//! Les quatorze cibles que tout travailleur hérite sont
//! `crate::ai::worker::register_base`, sur `crate::ai::state::WorkerState`. Une
//! `Machine<S, F>` ne peut pas en contenir une autre (deux types d'état, deux types de
//! faits), donc [`pupil`] déclare les trois états de base où Java renvoie l'écolier et
//! les trois retours vers `IDLE` : c'est le même pont que `crate::ai_gather`.
//!
//! ```ignore
//! use ai_work_05::{PupilAi, PupilFacts, PupilScene, PupilState, Seat};
//! use ai_work_05::facts::Pos;
//!
//! let mut ai = PupilAi::new();                       // IDLE, `EntityAIWorkPupil.java:73`
//! let mut scene = PupilScene::with_carpets(&[Pos::new(2, 64, 2)]);
//! let first = ai.tick(&mut scene);                   // IDLE → START_WORKING
//! let second = ai.tick(&mut scene);                  // START_WORKING → STUDY (`:222`)
//! let third = ai.tick(&mut scene);                   // STUDY → DECIDE (`:130`)
//! assert_eq!(third.goal(), PupilState::Base(ai::state::WorkerState::Decide));
//! ```

pub mod crafting_shop;
pub mod facts;
pub mod pupil;
pub mod state;

#[cfg(test)]
mod tests;

use crate::building::Kind;
use crate::jobs::job::Job;

pub use crafting_shop::{
    active_fuels, bench_machine, chain, check_gate, delta, job_sounds, machine, Chain, Errand,
    ErrandGuard, FuelRefusal, FurnaceRecipe, GateRefusal, PreGuard, RecipeGate, RecipePath,
    SoundRequest, TradeDelta, ACCELERATE_TICKS, BASE_XP_GAIN, BURNING_WAIT_TICKS,
    CRAFTING_TARGETS, ERRAND_ORDER, FUEL_CHECK_TICKS, FURNACE_VISIT_TICKS, LEFTOVER_CHECK_TICKS,
    NO_FURNACES_MESSAGE, NO_FUEL_MESSAGE, NO_FURNACE_DELAY, PRE_GUARDS, STACKSIZE,
    STONEMASON_TAG, STONE_SMELTERY_TAG,
};
pub use facts::{PupilFacts, PupilMemory, PupilScene, Seat};
pub use pupil::{
    LessonProgress, Particle, PupilAi, PupilAsk, PupilOutcome, PupilStep, SeatChoice, ai, decide,
    lesson_progress, lesson_xp, recess, register, seat_to_sit, sit_seconds, sit_ticks, start_working,
    study, LESSON_INTELLIGENCE_XP, PAPER, SIT_SECONDS_FLOOR, SIT_SECONDS_SPREAD,
    STUDY_TO_RECESS_RATIO, WANDER_RADIUS,
};
pub use state::{Decision, PupilRefusal, PupilState};

/// `TranslationConstants.PUPIL_NO_CARPET` (`api/util/constant/TranslationConstants.java:
/// 344`) : le seul message que ce métier adresse au joueur, brandi comme interaction
/// bloquante quand l'école n'a pas de place assise (`EntityAIWorkPupil.java:98`).
pub const PUPIL_NO_CARPET: &str = "com.minecolonies.coremod.school.nocarpet";

/// `Constants.TICKS_SECOND` (`api/util/constant/Constants.java:21`) : une seconde de
/// jeu, et le rythme commun des cibles du métier (`EntityAIWorkPupil.java:74-77`).
pub const TICKS_SECOND: u32 = 20;

/// `AbstractEntityAIBasic.STANDARD_DELAY` (`:100`), tel que le module `ai` l'a déjà
/// fixé : le délai après une action qui a changé le monde, et le rythme du retour de
/// `INVENTORY_FULL` vers `IDLE`.
pub const STANDARD_DELAY: u32 = crate::ai::worker::STANDARD_DELAY;

/// `BuildingSchool.MAX_BUILDING_LEVEL` (`BuildingSchool.java:37`, rendu à `:71`) : le
/// niveau maximal de l'école. C'est la borne qui dit combien de tapis un joueur peut
/// espérer — et donc combien de sièges `getRandomPlaceToSit` peut tirer.
pub const SCHOOL_MAX_LEVEL: u8 = 5;

/// Les trois métiers de ce module.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive : les trois fichiers que le brief nomme
pub enum Trade {
    /// `core/entity/ai/workers/crafting/EntityAIWorkStoneSmeltery.java` (27 lignes).
    StoneSmeltery,
    /// `core/entity/ai/workers/crafting/EntityAIWorkStonemason.java` (28 lignes).
    Stonemason,
    /// `core/entity/ai/workers/education/EntityAIWorkPupil.java` (224 lignes).
    Pupil,
}

impl Trade {
    /// Les trois métiers, dans l'ordre du brief.
    pub const ALL: [Trade; 3] = [Trade::StoneSmeltery, Trade::Stonemason, Trade::Pupil];

    /// La classe d'IA de Java, telle qu'un journal la nomme.
    pub fn class(self) -> &'static str {
        match self {
            Trade::StoneSmeltery => "EntityAIWorkStoneSmeltery",
            Trade::Stonemason => "EntityAIWorkStonemason",
            Trade::Pupil => "EntityAIWorkPupil",
        }
    }

    /// Le métier, dans `crate::jobs` (`Job::StoneSmeltery` s'appelle « Brickmaker » à
    /// l'affichage, `jobs/job.rs:127` — mais son chemin reste `stonesmeltery`).
    pub fn job(self) -> Job {
        match self {
            Trade::StoneSmeltery => Job::StoneSmeltery,
            Trade::Stonemason => Job::StoneMason,
            Trade::Pupil => Job::Pupil,
        }
    }

    /// La hutte du métier, dans `crate::building` : c'est ce que rend
    /// `getExpectedBuildingClass()` (la seule méthode que les deux fichiers de 27 et
    /// 28 lignes surchargent).
    pub fn building(self) -> Kind {
        match self {
            Trade::StoneSmeltery => Kind::StoneSmeltery,
            Trade::Stonemason => Kind::StoneMason,
            Trade::Pupil => Kind::School,
        }
    }

    /// Le nombre de cibles que la table du métier enregistre au total — les quatorze
    /// de base plus les siennes.
    ///
    /// `AbstractEntityAIBasic.java:204-259` en enregistre quatorze (le tableau de
    /// `crate::ai::worker`) ; `AbstractEntityAICrafting.java:160-170` six de plus ;
    /// `AbstractEntityAIRequestSmelter.java:71-82` sept de plus ; et
    /// `AbstractEntityAIInteract.java:110-113`, la mère de l'écolier, en ajoute zéro
    /// (« no new targets for now »).
    pub fn targets(self) -> u32 {
        match self {
            Trade::StoneSmeltery => 14 + 6 + 7,
            Trade::Stonemason => 14 + 6,
            Trade::Pupil => 14 + 5,
        }
    }

    /// Les lignes Java des `registerTargets` de la chaîne, de la mère à la fille.
    pub fn registers(self) -> &'static [&'static str] {
        match self {
            Trade::StoneSmeltery => &[
                "AbstractEntityAIBasic.java:204-259",
                "AbstractEntityAIInteract.java:110-113",
                "AbstractEntityAICrafting.java:160-170",
                "AbstractEntityAIRequestSmelter.java:71-82",
            ],
            Trade::Stonemason => &[
                "AbstractEntityAIBasic.java:204-259",
                "AbstractEntityAIInteract.java:110-113",
                "AbstractEntityAICrafting.java:160-170",
            ],
            Trade::Pupil => &[
                "AbstractEntityAIBasic.java:204-259",
                "AbstractEntityAIInteract.java:110-113",
                "EntityAIWorkPupil.java:72-79",
            ],
        }
    }

    /// La ligne qui dit où la table du métier est écrite *en propre* — celle que le
    /// brief cherchait, et qui n'existe pas pour les deux métiers de la pierre.
    pub fn own_table(self) -> Option<&'static str> {
        match self {
            Trade::StoneSmeltery | Trade::Stonemason => None,
            Trade::Pupil => Some("EntityAIWorkPupil.java:72-78"),
        }
    }
}

impl core::fmt::Display for Trade {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.class())
    }
}
