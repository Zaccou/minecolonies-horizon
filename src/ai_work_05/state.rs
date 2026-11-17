//! Les états du but de l'écolier, les refus qu'il nomme, et la forme d'une décision.
//!
//! Référence Java : `api/entity/ai/statemachine/states/AIWorkerState.java` — un seul
//! enum pour **tous** les métiers (127 valeurs). Trois états de ce fichier y sont
//! déjà : `IDLE` (`:15`), `START_WORKING` (`:31`) et `DECIDE` (`:59`) ; l'écolier en
//! ajoute deux, `STUDY` (`:439`) et `RECESS` (`:624`), et `EntityAIWorkPupil.java:
//! 72-78` est le seul endroit qui les nomme.
//!
//! **Pourquoi `Base(...)` et pas une copie.** `crate::ai::state::WorkerState` nomme
//! déjà les dix-huit états de base que ce port donne à une table, et cet enum
//! appartient à `crate::ai` (ce module n'écrit que dans `src/ai_work_05`, règle 1).
//! Un état de base est donc [`PupilState::Base`] enveloppant le `WorkerState` que le
//! module `ai` connaît déjà : `DECIDE` en fait partie, il n'est pas réinventé.
//!
//! **Ce qui n'est pas ici.** Le comportement d'un état de base — vider l'inventaire,
//! attendre une requête, se mettre en pause — est
//! `crate::ai::worker::register_base` (les quatorze cibles de
//! `AbstractEntityAIBasic.java:204-259`). La table de l'écolier entre et sort de ces
//! états et le dit ; la réécrire serait la deuxième copie d'une table qui a déjà ses
//! tests.

use core::fmt;

use crate::ai::state::WorkerState;

/// Un état de la table de l'écolier : un état de base rendu à `crate::ai`, ou un des
/// deux états que ce métier nomme.
///
/// Référence Java : `AIWorkerState.java` — la ligne entre parenthèses est la
/// déclaration de la constante, et c'est cette ligne que [`PupilState::java`] rend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
// exhaustive : les sept états de base que cette table touche plus les deux qu'elle crée
pub enum PupilState {
    /// Un état de `crate::ai::state::WorkerState` : `IDLE`, `START_WORKING`,
    /// `DECIDE`, `INVENTORY_FULL`, `NEEDS_ITEM`, `PAUSED` et
    /// `GATHERING_REQUIRED_MATERIALS`.
    Base(WorkerState),
    /// `STUDY` (`AIWorkerState.java:439`) — s'asseoir sur un siège de l'école et
    /// étudier le temps d'une leçon (`EntityAIWorkPupil.java:127-203`).
    Study,
    /// `RECESS` (`:624`) — courir dans l'école jusqu'à ce que la récréation soit
    /// finie (`:111-120`).
    Recess,
}

impl PupilState {
    /// Les huit états que cette table peut avoir en force.
    ///
    /// L'ordre est celui de `AIWorkerState` pour les états de base, puis celui des
    /// deux états du métier. Les autres états de base
    /// (`INIT`, `PREPARING`, `START_BUILDING`, `MINE_BLOCK`, `LOAD_STRUCTURE`,
    /// `BUILDING_STEP`, `COMPLETE_BUILD`, `PICK_UP`, `COMPOSTER_FILL`,
    /// `COMPOSTER_HARVEST`, `GET_MATERIALS`, `GATHERING_REQUIRED_MATERIALS`) ne sont
    /// pas listés : l'écolier n'en visite aucun — il ne pose pas de bloc, ne mine pas,
    /// ne remplit pas de tonneau et ne va jamais chercher de pile, puisqu'il ne crée
    /// aucune requête (`EntityAIWorkPupil.java` ne touche ni `needsCurrently` ni le
    /// système de requêtes).
    pub const ALL: [PupilState; 8] = [
        PupilState::Base(WorkerState::Idle),
        PupilState::Base(WorkerState::StartWorking),
        PupilState::Base(WorkerState::InventoryFull),
        PupilState::Base(WorkerState::NeedsItem),
        PupilState::Base(WorkerState::Decide),
        PupilState::Base(WorkerState::Paused),
        PupilState::Study,
        PupilState::Recess,
    ];

    /// L'identifiant Java, c'est-à-dire ce qu'un historique d'états ou le nametag du
    /// citoyen affiche (`AbstractEntityAIBasic.java:475`).
    pub fn java(self) -> &'static str {
        match self {
            PupilState::Base(state) => state.java_name(),
            PupilState::Study => "STUDY",
            PupilState::Recess => "RECESS",
        }
    }

    /// La ligne de `AIWorkerState.java` qui déclare l'état.
    ///
    /// Chacun des états de base que cette table touche a la sienne ; un état que ce
    /// métier ne visite pas n'a pas de ligne ici, seulement le fichier — c'est
    /// [`PupilState::ALL`] qui dit lequel des deux cas s'applique.
    pub fn java_line(self) -> &'static str {
        match self {
            PupilState::Base(WorkerState::Idle) => "AIWorkerState.java:15",
            PupilState::Base(WorkerState::InventoryFull) => "AIWorkerState.java:23",
            PupilState::Base(WorkerState::StartWorking) => "AIWorkerState.java:31",
            PupilState::Base(WorkerState::NeedsItem) => "AIWorkerState.java:35",
            PupilState::Base(WorkerState::Decide) => "AIWorkerState.java:59",
            PupilState::Base(WorkerState::Paused) => "AIWorkerState.java:63",
            PupilState::Base(WorkerState::GatheringRequiredMaterials) => "AIWorkerState.java:310",
            PupilState::Base(_) => "AIWorkerState.java",
            PupilState::Study => "AIWorkerState.java:439",
            PupilState::Recess => "AIWorkerState.java:624",
        }
    }

    /// L'état de base que celui-ci rend à `crate::ai`, ou `None` pour un état que ce
    /// métier possède.
    pub fn worker(self) -> Option<WorkerState> {
        match self {
            PupilState::Base(state) => Some(state),
            PupilState::Study | PupilState::Recess => None,
        }
    }

    /// `AIWorkerState.isOkayToEat` pour cet état — le drapeau que lit
    /// `AbstractAISkeleton.canBeInterrupted()` (`AbstractAISkeleton.java:126-131`).
    ///
    /// `STUDY(true)` (`:439`) et `RECESS(true)` (`:624`) : une leçon et une
    /// récréation sont interrompables, exactement comme les états d'attente de la
    /// classe de base. Aucun des deux n'est un état « il faut finir » comme
    /// `INVENTORY_FULL` (`:23`) ou `MINING` (`:43`).
    pub fn okay_to_eat(self) -> bool {
        match self {
            PupilState::Base(state) => state.okay_to_eat(),
            PupilState::Study | PupilState::Recess => true,
        }
    }
}

impl fmt::Display for PupilState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.java())
    }
}

/// Pourquoi l'IA de l'écolier n'avance pas, nommé.
///
/// Java n'a pas ce type : un état qui ne peut pas avancer fait `return getState()`
/// et laisse la raison implicite dans la branche prise. Un appelant doit pouvoir
/// répondre à un joueur (« pourquoi mon écolier tourne en rond ? ») et un test doit
/// pouvoir asserter dessus, donc chaque variante porte la ligne qui la décide.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive : une variante par `return <même état>` et par sortie silencieuse du fichier
pub enum PupilRefusal {
    /// `getRandomPlaceToSit()` a rendu `null` : l'école n'a ni siège marqué `sit` ni
    /// tapis enregistré (`BuildingSchool.java:127-141`). L'écolier déclenche alors
    /// l'interaction bloquante `PUPIL_NO_CARPET` et **reste en `DECIDE`**
    /// (`EntityAIWorkPupil.java:96-100`). C'est le refus le plus visible du métier :
    /// il attend que le joueur pose un tapis.
    NoSeat,
    /// Le tapis tiré au sort n'est plus un tapis : Java le retire de la liste de
    /// l'école et rend `null` (`BuildingSchool.java:138-143`). La conséquence est la
    /// même que [`PupilRefusal::NoSeat`] — `DECIDE` — mais la cause est une donnée
    /// périmée, pas une école vide, et c'est le monde qu'il faut réparer.
    StaleCarpet,
    /// Un autre citoyen occupe déjà le siège : le point d'étude est oublié et
    /// l'écolier retourne choisir (`EntityAIWorkPupil.java:139-144`).
    SeatTaken,
    /// `walkToBuilding()` n'est pas arrivé (`:218-221`) : l'écolier est encore en
    /// chemin, il reste en `START_WORKING`.
    NotAtBuilding,
    /// `walkToWorkPos(studyPos)` n'est pas arrivé (`:134-137`) : il reste en
    /// `STUDY` et continue de marcher.
    NotAtSeat,
    /// Le siège a été choisi mais le citoyen n'est pas assis et le point d'étude a
    /// été effacé (`:129-132`) : retour en `DECIDE`.
    NoStudyPos,
    /// Aucune feuille de papier dans l'inventaire (`:153`, `:160-164`). **Ce n'est
    /// pas un arrêt** : Java continue la leçon, change seulement la particule et
    /// paie zéro point d'Intelligence (`:179-196`). Le refus est nommé pour que le
    /// joueur voie pourquoi la compétence ne monte pas.
    NoPaper,
}

impl PupilRefusal {
    /// La ligne Java qui décide ce refus.
    pub fn java(self) -> &'static str {
        match self {
            PupilRefusal::NoSeat => "EntityAIWorkPupil.java:96-100",
            PupilRefusal::StaleCarpet => "BuildingSchool.java:138-143",
            PupilRefusal::SeatTaken => "EntityAIWorkPupil.java:139-144",
            PupilRefusal::NotAtBuilding => "EntityAIWorkPupil.java:218-221",
            PupilRefusal::NotAtSeat => "EntityAIWorkPupil.java:134-137",
            PupilRefusal::NoStudyPos => "EntityAIWorkPupil.java:129-132",
            PupilRefusal::NoPaper => "EntityAIWorkPupil.java:160-164",
        }
    }

    /// La clé de traduction à afficher au joueur, quand Java en a une.
    ///
    /// Seul [`PupilRefusal::NoSeat`] en a une : `PUPIL_NO_CARPET`
    /// (`TranslationConstants.java:344`), brandie comme interaction **bloquante**
    /// (`EntityAIWorkPupil.java:98`), c'est-à-dire une demande à laquelle le joueur
    /// doit répondre.
    pub fn message_key(self) -> Option<&'static str> {
        match self {
            PupilRefusal::NoSeat => Some(crate::ai_work_05::PUPIL_NO_CARPET),
            PupilRefusal::StaleCarpet
            | PupilRefusal::SeatTaken
            | PupilRefusal::NotAtBuilding
            | PupilRefusal::NotAtSeat
            | PupilRefusal::NoStudyPos
            | PupilRefusal::NoPaper => None,
        }
    }

    /// Une clé stable, pour un journal ou une assertion.
    pub fn key(self) -> &'static str {
        match self {
            PupilRefusal::NoSeat => "no_seat",
            PupilRefusal::StaleCarpet => "stale_carpet",
            PupilRefusal::SeatTaken => "seat_taken",
            PupilRefusal::NotAtBuilding => "not_at_building",
            PupilRefusal::NotAtSeat => "not_at_seat",
            PupilRefusal::NoStudyPos => "no_study_pos",
            PupilRefusal::NoPaper => "no_paper",
        }
    }
}

/// Ce qu'une action a décidé, dans les deux moitiés que Java garde séparées : l'état
/// qu'elle rend (`return getState()` quand elle ne peut pas aller plus loin) et la
/// branche qui a refusé.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decision {
    /// L'état rendu par l'action, `None` pour « je reste dans l'état courant » — la
    /// même forme que l'`Option` d'`Action::Run`.
    pub next: Option<PupilState>,
    /// Pourquoi elle s'est arrêtée, quand elle s'est arrêtée.
    pub refusal: Option<PupilRefusal>,
}

impl Decision {
    /// Aller vers `state`.
    pub fn go(state: PupilState) -> Decision {
        Decision {
            next: Some(state),
            refusal: None,
        }
    }

    /// `return getState()` : aucun changement d'état, aucune plainte.
    pub fn stay() -> Decision {
        Decision {
            next: None,
            refusal: None,
        }
    }

    /// `return getState()` avec la raison notée.
    pub fn refuse(refusal: PupilRefusal) -> Decision {
        Decision {
            next: None,
            refusal: Some(refusal),
        }
    }

    /// Aller vers `state` et noter pourquoi.
    pub fn go_after(state: PupilState, refusal: PupilRefusal) -> Decision {
        Decision {
            next: Some(state),
            refusal: Some(refusal),
        }
    }

    /// Ajouter une raison à une décision qui n'en a pas.
    pub fn because(mut self, refusal: PupilRefusal) -> Decision {
        if self.refusal.is_none() {
            self.refusal = Some(refusal);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pupil_names_two_states_of_its_own_and_borrows_the_rest() {
        // `EntityAIWorkPupil.java:72-78` : cinq cibles, dont deux états neufs. IDLE
        // (`:15`), START_WORKING (`:31`) et DECIDE (`:59`) sont déjà au port.
        assert_eq!(PupilState::ALL.len(), 8);
        let own: Vec<&str> = PupilState::ALL
            .iter()
            .filter(|state| state.worker().is_none())
            .map(|state| state.java())
            .collect();
        assert_eq!(own, vec!["STUDY", "RECESS"]);
        assert_eq!(
            PupilState::Base(WorkerState::Decide).java(),
            "DECIDE",
            "DECIDE n'est pas réinventé : il vient de WorkerState"
        );
    }

    #[test]
    fn study_and_recess_are_interruptible_like_the_waiting_states() {
        // `AIWorkerState.java:439` et `:624` passent `true` au constructeur.
        assert!(PupilState::Study.okay_to_eat());
        assert!(PupilState::Recess.okay_to_eat());
        assert!(PupilState::Base(WorkerState::Idle).okay_to_eat());
        // Le contre-exemple : vider son inventaire n'est pas interrompable
        // (`AIWorkerState.java:23`).
        assert!(!PupilState::Base(WorkerState::InventoryFull).okay_to_eat());
    }

    #[test]
    fn every_state_cites_its_own_java_line() {
        // Chaque état de `ALL` a une ligne distincte, et elle tombe dans le fichier
        // d'`AIWorkerState`.
        let mut seen = Vec::new();
        for state in PupilState::ALL {
            let line = state.java_line();
            assert!(line.starts_with("AIWorkerState.java:"), "{state} -> {line}");
            assert!(!seen.contains(&line), "{state} réutilise {line}");
            seen.push(line);
        }
        assert_eq!(PupilState::Study.java_line(), "AIWorkerState.java:439");
        assert_eq!(PupilState::Recess.java_line(), "AIWorkerState.java:624");
        assert_eq!(
            PupilState::Base(WorkerState::Decide).java_line(),
            "AIWorkerState.java:59"
        );
    }

    #[test]
    fn only_the_missing_carpet_has_a_message_for_the_player() {
        // `EntityAIWorkPupil.java:98` : une `StandardInteraction` bloquante, la seule
        // du fichier. Les autres refus sont muets dans Java aussi.
        assert_eq!(
            PupilRefusal::NoSeat.message_key(),
            Some("com.minecolonies.coremod.school.nocarpet")
        );
        for refusal in [
            PupilRefusal::StaleCarpet,
            PupilRefusal::SeatTaken,
            PupilRefusal::NotAtBuilding,
            PupilRefusal::NotAtSeat,
            PupilRefusal::NoStudyPos,
            PupilRefusal::NoPaper,
        ] {
            assert_eq!(refusal.message_key(), None, "{refusal:?}");
        }
        // Et chaque refus nomme la ligne qui le décide.
        assert_eq!(PupilRefusal::NoSeat.java(), "EntityAIWorkPupil.java:96-100");
        assert_eq!(
            PupilRefusal::SeatTaken.java(),
            "EntityAIWorkPupil.java:139-144"
        );
        assert_eq!(
            PupilRefusal::NoPaper.java(),
            "EntityAIWorkPupil.java:160-164"
        );
    }

    #[test]
    fn a_decision_keeps_the_state_and_the_reason_apart() {
        assert_eq!(Decision::go(PupilState::Study).next, Some(PupilState::Study));
        assert_eq!(Decision::stay().next, None);
        let refused = Decision::refuse(PupilRefusal::NoSeat);
        assert_eq!(refused.next, None);
        assert_eq!(refused.refusal, Some(PupilRefusal::NoSeat));
        // `because` ne remplace jamais une raison déjà écrite.
        assert_eq!(
            Decision::refuse(PupilRefusal::SeatTaken)
                .because(PupilRefusal::NoPaper)
                .refusal,
            Some(PupilRefusal::SeatTaken)
        );
        assert_eq!(
            Decision::go(PupilState::Recess)
                .because(PupilRefusal::NoPaper)
                .refusal,
            Some(PupilRefusal::NoPaper)
        );
    }
}
