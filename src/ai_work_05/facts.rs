//! Ce que le but de l'écolier observe : l'école, ses sièges, le papier, et les cinq
//! tirages dont il dépend.
//!
//! Référence Java : `EntityAIWorkPupil.java` — `decide` (`:87-104`), `recess`
//! (`:111-120`), `study` (`:127-203`), `startWorkingAtOwnBuilding` (`:216-223`) — et
//! `BuildingSchool.java:125-144` (`getRandomPlaceToSit`), `:138-143` (le tapis
//! re-testé) et `:37` (`MAX_BUILDING_LEVEL = 5`).
//!
//! **Tout ce que l'IA lit du monde est ici, et rien d'autre.** Aucune de ces fonctions
//! n'appelle le niveau, ne déplace le citoyen, ni ne pose un bloc : la décision se
//! rejoue donc sans monde, ce qui est la seule façon de la tester. Les effets que
//! Java produit au passage (s'asseoir, changer l'objet en main, envoyer une
//! particule) sont des **demandes** rendues par la décision, pas des écritures
//! ([`super::pupil::PupilAsk`]).
//!
//! **Les cinq tirages sont des faits, et c'est volontaire.** `worker.getRandom()`
//! (`:89`, `:113`, `:149`) et `ColonyConstants.rand` (`BuildingSchool.java:130`,
//! `:137`, `:142`) sont des générateurs globaux : une décision qui les lit ne se
//! rejoue pas, donc ne se teste pas. Le port les reçoit du monde sous forme de
//! valeurs ([`PupilFacts::recess_roll`], [`PupilFacts::seat_pick`],
//! [`PupilFacts::recess_exit_roll`], [`PupilFacts::sit_draw`]) et les ramène dans
//! l'intervalle Java (`% 10`, `% len`, `% 60`) : un tirage hors bornes ne panique
//! jamais, il est ramené.

use crate::ai::facts::{Pos, WorldFacts};

use super::state::PupilRefusal;

/// Un siège de l'école, tel que Java le garde : une position et le fait qu'elle soit
/// encore un tapis.
///
/// Référence Java : `BuildingSchool.carpet` (`:53-55`), rempli à l'enregistrement du
/// bloc (`:84-91`), et re-testé au tirage (`:143-145`) — un tapis cassé depuis est
/// retiré de la liste, ce que le port rend comme une demande
/// ([`super::pupil::SeatChoice::forgot`]) plutôt que comme une écriture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Seat {
    /// La position enregistrée (`registerBlockPosition`, `:84-91`).
    pub pos: Pos,
    /// `isCarpet(colony.getWorld().getBlockState(pos))` (`:143`) : le bloc est
    /// toujours un `WoolCarpetBlock`.
    pub still_a_carpet: bool,
}

impl Seat {
    /// Un tapis encore valable.
    pub const fn carpet(pos: Pos) -> Seat {
        Seat {
            pos,
            still_a_carpet: true,
        }
    }

    /// Un tapis qui n'est plus un tapis : Java le retire de l'école (`:145`).
    pub const fn stale(pos: Pos) -> Seat {
        Seat {
            pos,
            still_a_carpet: false,
        }
    }
}

/// Ce que l'écolier observe de son école et de son inventaire, à cet instant.
#[derive(Clone, Debug, PartialEq)]
pub struct PupilFacts {
    /// `walkToBuilding()` (`EntityAIWorkPupil.java:218`) : le citoyen est arrivé à
    /// son école. C'est `EntityNavigationUtils.walkToBuilding` (`:115-124`) qui
    /// répond, et sa doc dit « True when arrived » : le `true` est l'arrivée, pas la
    /// marche en cours — la doc de `AbstractEntityAIBasic.walkToWorkPos` (`:797`,
    /// « true while walking ») est périmée sur ce point.
    pub at_building: bool,
    /// `walkToWorkPos(studyPos)` (`:134`) : le citoyen est à portée de son siège
    /// (`WOKR_IN_BUILDING_DIST = 7`, `EntityNavigationUtils.java:25`).
    pub at_seat: bool,
    /// `worker.getNavigation().isDone()` (`:113`) : la marche en cours est finie.
    pub navigation_done: bool,
    /// `building.getLocationsFromTag(TAG_SITTING)` (`BuildingSchool.java:127`),
    /// c'est-à-dire les positions marquées `sit` dans le schéma de l'école
    /// (`SchematicTagConstants.java:11`). Priorité absolue sur les tapis.
    pub sitting_tag: Vec<Pos>,
    /// Les tapis enregistrés (`:53-55`, `:84-91`), dans l'ordre d'enregistrement.
    pub carpets: Vec<Seat>,
    /// `!world.getEntitiesOfClass(EntityCitizen.class, new AABB(pos, pos)).isEmpty()`
    /// (`EntityAIWorkPupil.java:139-141`). La boîte de Java est **dégénérée** —
    /// `AABB(x, y, z, x, y, z)`, un point — donc elle ne contient qu'un citoyen dont
    /// la boîte englobante touche ce point : en pratique celui qui est assis là.
    pub other_citizen_at_seat: bool,
    /// `InventoryUtils.findFirstSlotInItemHandlerWith(inv, PAPER) != -1` (`:153`).
    /// Le prédicat est `stack.getItem() == Items.PAPER` (`:47`).
    pub has_paper: bool,
    /// `getResearchEffects().getEffectStrength(TEACHING)` (`:186`), avec
    /// `TEACHING = minecolonies:effects/teachingmultiplier`
    /// (`ResearchConstants.java:138`). Vaut `0.0` sans recherche.
    pub teaching: f64,
    /// `worker.getRandom().nextInt(STUDY_TO_RECESS_RATIO)` (`:89`) : le tirage qui
    /// envoie un écolier sur dix en récréation. **`0` est le tirage rare** — le
    /// défaut d'une scène est `1`, c'est-à-dire le cas des neuf autres.
    pub recess_roll: u32,
    /// `ColonyConstants.rand.nextInt(size)` (`BuildingSchool.java:130`, `:137`) :
    /// l'indice tiré dans la liste des sièges — celle du tag d'abord, celle des tapis
    /// ensuite.
    pub seat_pick: usize,
    /// `worker.getRandom().nextInt(STUDY_TO_RECESS_RATIO)` (`:113`) : le tirage qui
    /// met fin à une récréation. **`0` est le tirage rare**, comme pour `:89`.
    pub recess_exit_roll: u32,
    /// `worker.getRandom().nextInt(120 / 2)` (`:149`) : la base de la durée de la
    /// leçon, en secondes, avant l'ajout des 60 secondes plancher.
    pub sit_draw: u32,
    /// `building.getCorners()` (`:118`), la boîte de l'école dans laquelle
    /// `walkToRandomPosWithin` promène l'écolier. Sert seulement à décrire la
    /// demande de marche ; la décision ne lit ni les coins ni la distance.
    pub corners: Option<(Pos, Pos)>,
}

impl Default for PupilFacts {
    /// Un écolier arrivé à son école, assis à portée, sans papier, avec une école
    /// sans siège et les tirages du cas courant — c'est-à-dire le cas le plus pauvre,
    /// à partir duquel chaque test ajoute le fait qu'il étudie.
    ///
    /// Les deux tirages de récréation valent `1`, pas `0` : `0` est le tirage **rare**
    /// (`nextInt(10) < 1`, `:89` et `:113`), et un défaut qui enverrait tout le monde
    /// en récréation serait un piège de test, pas une commodité.
    fn default() -> PupilFacts {
        PupilFacts {
            at_building: true,
            at_seat: true,
            navigation_done: true,
            sitting_tag: Vec::new(),
            carpets: Vec::new(),
            other_citizen_at_seat: false,
            has_paper: false,
            teaching: 0.0,
            recess_roll: 1,
            seat_pick: 0,
            recess_exit_roll: 1,
            sit_draw: 0,
            corners: None,
        }
    }
}

impl PupilFacts {
    /// Les sièges que `getRandomPlaceToSit` peut rendre : le tag `sit` d'abord, puis
    /// les tapis (`BuildingSchool.java:127-143`).
    ///
    /// Le compte est le dénominateur du tirage : Java tire dans **l'une ou l'autre**
    /// liste, jamais dans leur réunion (`:130-141`), et le port garde cette
    /// séparation parce qu'un tapis périmé ne doit pas décaler l'indice du tag.
    pub fn seat_sources(&self) -> (usize, usize) {
        (self.sitting_tag.len(), self.carpets.len())
    }

    /// Le nombre total de places assises possibles, tapis périmés compris : sert aux
    /// messages et aux tests, pas au tirage.
    pub fn seats(&self) -> usize {
        self.sitting_tag.len() + self.carpets.len()
    }
}

/// Ce que l'IA retient entre deux ticks — les quatre champs privés du métier.
///
/// Référence Java : `maxSittingTicks` (`EntityAIWorkPupil.java:52`), `sittingTicks`
/// (`:57`), `studyPos` (`:62`) et `worker.vehicle` (`:146`, `:173`). Le dernier n'est
/// pas un champ de l'IA mais un état du citoyen ; il est ici parce que la décision le
/// lit (`:146`) et l'écrit (`:175`) et qu'un fait sur lequel on écrit n'est pas un
/// fait : c'est de la mémoire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PupilMemory {
    /// `studyPos` (`:62`) : le siège choisi par `decide` (`:102`), effacé quand il est
    /// occupé (`:142`) ou qu'il n'y en a pas (`:129-132`).
    pub study_pos: Option<Pos>,
    /// `sittingTicks` (`:57`) : **des secondes**, pas des ticks. Chaque passage de
    /// `study` dure `TICKS_SECOND = 20` ticks (`:76`) et incrémente de un (`:166`),
    /// donc l'unité est la seconde de jeu — c'est ce qui rend la comparaison avec
    /// `maxSittingTicks` juste.
    pub sitting_ticks: u32,
    /// `maxSittingTicks` (`:52`) : la durée de la leçon **en secondes**, tirée entre
    /// 60 et 119 (`:149`), remise à zéro à la fin de la leçon (`:200`).
    pub max_sitting_ticks: u32,
    /// `worker.vehicle != null` (`:146`, `:173`) : le citoyen est assis
    /// (`SittingEntity.sitDown`, `:150`). Quand le véhicule disparaît, Java le
    /// rassoit et retire une durée (`:146-151`).
    pub riding: bool,
}

impl Default for PupilMemory {
    /// Un écolier qui n'a encore rien choisi, n'est pas assis, et n'a pas de leçon en
    /// cours — l'état d'un `EntityAIWorkPupil` fraîchement construit (`:52`, `:57`).
    fn default() -> PupilMemory {
        PupilMemory {
            study_pos: None,
            sitting_ticks: 0,
            max_sitting_ticks: 0,
            riding: false,
        }
    }
}

/// La scène d'un tick : le monde, les faits du métier, la mémoire, et le dernier refus.
///
/// Même découpage que `crate::ai_gather::GatherScene` (`facts`, `job`, `memory`,
/// `refusal`) : le monde est [`WorldFacts`], ce que le module `ai` sait déjà lire —
/// la position de la hutte, celle du citoyen, son inventaire, son métier.
#[derive(Clone, Debug, PartialEq)]
pub struct PupilScene {
    /// Le citoyen, son inventaire et sa hutte (`crate::ai::facts`).
    pub world: WorldFacts,
    /// Ce que l'écolier observe de son école.
    pub facts: PupilFacts,
    /// Ce que l'IA retient d'un tick à l'autre.
    pub memory: PupilMemory,
    /// Le dernier refus, remis à `None` au début de chaque tick.
    pub refusal: Option<PupilRefusal>,
    /// Ce que le but a demandé au monde pendant ce tick, remis à zéro au début de
    /// chaque tick — les [`super::pupil::PupilAsk`], dans l'ordre des appels Java.
    pub asks: Vec<super::pupil::PupilAsk>,
}

impl PupilScene {
    /// Une scène au monde par défaut et aux faits fournis.
    pub fn new(facts: PupilFacts) -> PupilScene {
        PupilScene {
            world: WorldFacts::new(),
            facts,
            memory: PupilMemory::default(),
            refusal: None,
            asks: Vec::new(),
        }
    }

    /// Une école de `seats` tapis valables, prête pour une leçon : les faits que la
    /// plupart des tests veulent.
    pub fn with_carpets(seats: &[Pos]) -> PupilScene {
        PupilScene::new(PupilFacts {
            carpets: seats.iter().copied().map(Seat::carpet).collect(),
            has_paper: true,
            sit_draw: 0,
            ..PupilFacts::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_poorest_school_is_the_default() {
        let facts = PupilFacts::default();
        assert_eq!(facts.seats(), 0, "ni tag ni tapis");
        assert_eq!(facts.seat_sources(), (0, 0));
        assert!(!facts.has_paper, "`findFirstSlotInItemHandlerWith` à -1");
        assert_eq!(facts.teaching, 0.0, "sans recherche TEACHING");
        assert!(facts.at_building, "un écolier est à son école par défaut");
        // Les tirages par défaut sont ceux du cas courant : `0` est le tirage rare.
        assert_eq!(facts.recess_roll, 1);
        assert_eq!(facts.recess_exit_roll, 1);
        let memory = PupilMemory::default();
        assert_eq!(memory.sitting_ticks, 0);
        assert_eq!(memory.max_sitting_ticks, 0);
        assert!(!memory.riding, "`worker.vehicle == null`");
        assert_eq!(memory.study_pos, None);
    }

    #[test]
    fn the_two_seat_lists_stay_separate() {
        // `BuildingSchool.java:127-141` : le tag est tiré d'abord, les tapis ensuite,
        // et seulement si le tag est vide. `seat_sources` garde les deux longueurs
        // distinctes pour que le tirage ne mélange jamais les deux listes.
        let facts = PupilFacts {
            sitting_tag: vec![Pos::new(1, 64, 1), Pos::new(2, 64, 2)],
            carpets: vec![Seat::carpet(Pos::new(3, 64, 3))],
            ..PupilFacts::default()
        };
        assert_eq!(facts.seat_sources(), (2, 1));
        assert_eq!(facts.seats(), 3);
    }

    #[test]
    fn a_stale_carpet_is_still_a_seat_in_the_list() {
        // Il n'est retiré qu'au tirage (`BuildingSchool.java:138-143`), donc il compte
        // encore dans la liste tant que le tirage ne l'a pas désigné.
        let facts = PupilFacts {
            carpets: vec![Seat::stale(Pos::new(3, 64, 3))],
            ..PupilFacts::default()
        };
        assert_eq!(facts.seat_sources(), (0, 1));
        assert!(!facts.carpets[0].still_a_carpet);
    }

    #[test]
    fn a_scene_carries_the_world_the_school_and_the_memory() {
        let scene = PupilScene::with_carpets(&[Pos::new(0, 64, 0)]);
        assert_eq!(scene.world, WorldFacts::new());
        assert_eq!(scene.memory, PupilMemory::default());
        assert_eq!(scene.refusal, None);
        assert!(scene.asks.is_empty(), "un tick n'a encore rien demandé");
        assert_eq!(scene.facts.seat_sources(), (0, 1));
        assert!(scene.facts.has_paper, "l'école de test a du papier");
    }
}
