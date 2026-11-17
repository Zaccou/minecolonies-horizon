//! The states the two education trades add to `AIWorkerState`, the flag each one
//! carries, and the refusals they name.
//!
//! Java reference: `api/entity/ai/statemachine/states/AIWorkerState.java`, one enum
//! for **every** trade in the mod (127 values). The port keeps that shape —
//! [`EduState`] is one enum holding the two states these trades add plus the seven
//! base states they hand control back to — and not one enum per trade, because
//! Java's own tables are written against the single enum: the researcher's `study`
//! returns `START_WORKING` (`EntityAIWorkResearcher.java:104`), a *base* state, from
//! a trade table.
//!
//! **Why `Base(...)`.** `crate::ai::state::WorkerState` already names the eighteen
//! base states this port gives a table to, and that enum belongs to `crate::ai` (this
//! module writes only inside `src/ai_work_06`). So a base state is
//! [`EduState::Base`] wrapping the `WorkerState` the AI module already knows, and
//! [`EduState::worker`] gives it back. No state is spelled twice.
//!
//! **Two states, not four.** `STUDY` (`:439`) is the researcher's bookshelf pass and
//! `TEACH` (`:622`) is the teacher's lesson; `RECESS` (`:624`) belongs to the pupil,
//! whose AI is a fourth file of the same package (`EntityAIWorkPupil.java`) and a
//! separate port (`src/ai_work_05`) — it is named here only so the two tables never
//! claim it.
//!
//! **What is *not* here.** The behaviour of a base state (dumping the inventory,
//! waiting for a request, pausing) is `crate::ai::worker::register_base`. An
//! education table enters and leaves those states and says so, but re-implementing
//! them would be the second copy of a table that already exists and is already
//! tested; `MANQUANT.md` §1 writes down the seam this leaves.

use core::fmt;

use crate::ai::state::WorkerState;

/// One state of an education trade's table: a base state handed back to `crate::ai`,
/// or a state this module names.
///
/// Java reference: `AIWorkerState.java` — the line in brackets is the declaration of
/// the constant, and that line is what [`EduState::java_declaration`] reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
// exhaustive: the seven base states these two tables enter plus the two they own
pub enum EduState {
    /// A state of `crate::ai::state::WorkerState` — `IDLE`, `START_WORKING`,
    /// `DECIDE`, `INVENTORY_FULL`, `NEEDS_ITEM`, `PAUSED`,
    /// `GATHERING_REQUIRED_MATERIALS`.
    Base(WorkerState),
    /// `STUDY` (`AIWorkerState.java:439`) — walk to a bookshelf of the university and
    /// study at it for one pass (`EntityAIWorkResearcher.java:52`, `:68-105`).
    Study,
    /// `TEACH` (`AIWorkerState.java:622`) — walk to a seated pupil, sit down next to
    /// it, and give the lesson (`EntityAIWorkTeacher.java:79`, `:117-176`).
    Teach,
}

impl EduState {
    /// Every state this module names, base states first in `WorkerState`'s order,
    /// then the two trade states in `AIWorkerState`'s.
    pub const ALL: [EduState; 9] = [
        EduState::Base(WorkerState::Idle),
        EduState::Base(WorkerState::StartWorking),
        EduState::Base(WorkerState::Decide),
        EduState::Base(WorkerState::InventoryFull),
        EduState::Base(WorkerState::NeedsItem),
        EduState::Base(WorkerState::Paused),
        EduState::Base(WorkerState::GatheringRequiredMaterials),
        EduState::Study,
        EduState::Teach,
    ];

    /// Java's identifier (`AIWorkerState.java`), which is what a state history or the
    /// citizen's nametag shows (`AbstractEntityAIBasic.java:475`).
    pub fn java(self) -> &'static str {
        match self {
            EduState::Base(state) => state.java_name(),
            EduState::Study => "STUDY",
            EduState::Teach => "TEACH",
        }
    }

    /// The line that declares this state, `AIWorkerState.java:<line>`.
    pub fn java_declaration(self) -> &'static str {
        match self {
            EduState::Base(_) => "crate::ai::state::WorkerState (see AIWorkerState.java)",
            EduState::Study => "AIWorkerState.java:439",
            EduState::Teach => "AIWorkerState.java:622",
        }
    }

    /// `AIWorkerState.isOkayToEat` — the flag `AbstractAISkeleton.canBeInterrupted()`
    /// returns (`AbstractAISkeleton.java:126-131`), which `inventoryNeedsDump` reads
    /// (`AbstractEntityAIBasic.java:411`).
    ///
    /// Both education states may be interrupted: `STUDY(true)` (`:439`) and
    /// `TEACH(true)` (`:622`) are constructed with the flag at `true`, so a teacher in
    /// the middle of a lesson still goes to eat. Neither is a "must finish" state like
    /// `INVENTORY_FULL` (`:23`).
    pub fn okay_to_eat(self) -> bool {
        match self {
            EduState::Base(state) => state.okay_to_eat(),
            EduState::Study | EduState::Teach => true,
        }
    }

    /// The `WorkerState` behind a base state, `None` for a trade state.
    pub fn worker(self) -> Option<WorkerState> {
        match self {
            EduState::Base(state) => Some(state),
            _ => None,
        }
    }

    /// Whether this state belongs to `trade`'s own table.
    ///
    /// `STUDY` is the researcher's (`EntityAIWorkResearcher.java:52`); `TEACH` is the
    /// teacher's (`EntityAIWorkTeacher.java:79`). `STUDY` is also the pupil's — the
    /// same `AIWorkerState` constant, a different AI — and that is why nothing here
    /// claims it exclusively.
    pub fn belongs_to(self, trade: super::EducationTrade) -> bool {
        match trade {
            super::EducationTrade::Researcher => self == EduState::Study,
            super::EducationTrade::Teacher => self == EduState::Teach,
        }
    }
}

impl fmt::Display for EduState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.java())
    }
}

/// Why an education AI does not go further, named.
///
/// Java has no such type: each branch that cannot proceed does `return getState()`
/// (or `return START_WORKING`) and the reason stays in the shape of the `if`. A
/// player asking "why is my teacher standing there" needs it, and a test asserting on
/// a state sequence needs to know *which* branch held, so every variant carries the
/// line that decides it.
///
/// **A refusal is not a failure.** Three of these — [`EduRefusal::NoResearchInProgress`],
/// [`EduRefusal::WalkAbandoned`], [`EduRefusal::NoPaper`] — are recorded on a pass
/// that still *acts*: Java studies where it stands, pays the experience and sets the
/// delay. They are named so the player sees the effect that is missing, not to stop
/// the tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: one variant per branch of the five AI methods that can stop or degrade
pub enum EduRefusal {
    /// The scene does not carry this trade's facts — a caller mistake, not a Java
    /// state. Named so it can never look like "nothing to study".
    NoFactsForTrade,
    /// `walkToBuilding()` answered false (`EntityAIWorkResearcher.java:114-117`,
    /// `EntityAIWorkTeacher.java:231-234`): the citizen is still walking to its hut.
    NotAtBuilding,
    /// `walkToWorkPos(studyPos)` answered false, and `walkDelay` is still under
    /// `STUDY_DELAY` (`EntityAIWorkResearcher.java:75-79`): one more second of walking.
    NotAtBookshelf,
    /// The walk budget ran out — `walkDelay` reached `STUDY_DELAY` (`:80`) — and Java
    /// studies **from where it stands** instead. The pass happens; the bookshelf was
    /// never reached.
    WalkAbandoned,
    /// `getResearchInProgress()` is empty (`:84`): the university has nobody running a
    /// research, so the pass pays experience and advances nothing.
    NoResearchInProgress,
    /// `job.getCurrentMana() <= 0` (`:84`): the researcher's stored mana is spent, and
    /// the pass advances nothing. Mana is **not** refilled by working —
    /// `JobResearch.processOfflineTime` (`JobResearch.java:77-93`) is the only writer,
    /// so a colony that never goes offline never researches. See `RAPPORT.md` §4.
    NoMana,
    /// `walkToWorkPos(pupilToTeach.blockPosition())` answered false
    /// (`EntityAIWorkTeacher.java:125-128`): still walking to the pupil.
    NotAtPupil,
    /// `pupilToTeach == null` (`:119`): no pupil was picked, or the pick was cleared.
    PupilNotChosen,
    /// `pupilToTeach.vehicle == null` (`:119`): the pupil stood up (or was never
    /// seated) before the lesson; the teacher gives up on that pupil.
    PupilNotSeated,
    /// `sittingTicks < maxSittingTicks` (`:140-143`): the lesson is running, one
    /// second at a time.
    Sitting,
    /// `findFirstSlotInItemHandlerWith(worker.getInventoryCitizen(), PAPER)` is `-1`
    /// (`:151`, `:153`): the teacher has no paper, so the lesson is given without one.
    /// The pupil still gains Intelligence — only the handout is missing.
    NoPaper,
    /// The pupil already carries paper (`pupilSlot != -1`, `:152-153`): no second sheet
    /// is given. This is the `&&` of the transfer condition, named because it is the
    /// half a player sees as "my teacher keeps the paper".
    PupilHasPaper,
    /// `paperInBuilding + paperInInv <= 0` (`:93-96`): the school holds none and the
    /// teacher carries none, so a `Stack` request of [`super::teacher::PAPER_TO_REQUEST`]
    /// is created (once — `:211-213` avoids a second one).
    PaperMissing,
    /// `paperInInv == 0 && paperInBuilding > 0` (`:98-102`): the paper is in the
    /// school's racks, so the teacher goes to fetch
    /// [`super::teacher::PAPER_TO_REQUEST`] of it.
    PaperInHutOnly,
    /// `getEntitiesWithinBuilding(...)` found no seated pupil
    /// (`EntityAIWorkTeacher.java:104-114`): the school has no child sitting on a
    /// carpet, so there is nothing to teach this pass.
    NoPupil,
    /// `inventoryNeedsDump` said yes (`AbstractEntityAIBasic.java:409-416`): the
    /// teacher's threshold is [`super::teacher::ACTIONS_UNTIL_DUMPING`] = 50, the
    /// researcher's is the base class's 32 (`:425-428`).
    InventoryDumpDue,
}

impl EduRefusal {
    /// The Java line that decides this refusal.
    pub fn java(self) -> &'static str {
        match self {
            EduRefusal::NoFactsForTrade => "port: src/ai_work_06, not a Java branch",
            EduRefusal::NotAtBuilding => "EntityAIWorkResearcher.java:114-117",
            EduRefusal::NotAtBookshelf => "EntityAIWorkResearcher.java:75-79",
            EduRefusal::WalkAbandoned => "EntityAIWorkResearcher.java:80",
            EduRefusal::NoResearchInProgress => "EntityAIWorkResearcher.java:84",
            EduRefusal::NoMana => "EntityAIWorkResearcher.java:84 / JobResearch.java:69-72",
            EduRefusal::NotAtPupil => "EntityAIWorkTeacher.java:125-128",
            EduRefusal::PupilNotChosen => "EntityAIWorkTeacher.java:119",
            EduRefusal::PupilNotSeated => "EntityAIWorkTeacher.java:119",
            EduRefusal::Sitting => "EntityAIWorkTeacher.java:140-143",
            EduRefusal::NoPaper => "EntityAIWorkTeacher.java:151",
            EduRefusal::PupilHasPaper => "EntityAIWorkTeacher.java:152-153",
            EduRefusal::PaperMissing => "EntityAIWorkTeacher.java:93-96",
            EduRefusal::PaperInHutOnly => "EntityAIWorkTeacher.java:98-102",
            EduRefusal::NoPupil => "EntityAIWorkTeacher.java:104-114",
            EduRefusal::InventoryDumpDue => "AbstractEntityAIBasic.java:409-416",
        }
    }

    /// A stable key for a message the player will read, next to
    /// `crate::ai::worker::Refusal::key`.
    pub fn key(self) -> &'static str {
        match self {
            EduRefusal::NoFactsForTrade => "ai_work_06.refusal.no_facts_for_trade",
            EduRefusal::NotAtBuilding => "ai_work_06.refusal.not_at_building",
            EduRefusal::NotAtBookshelf => "ai_work_06.refusal.not_at_bookshelf",
            EduRefusal::WalkAbandoned => "ai_work_06.refusal.walk_abandoned",
            EduRefusal::NoResearchInProgress => "ai_work_06.refusal.no_research_in_progress",
            EduRefusal::NoMana => "ai_work_06.refusal.no_mana",
            EduRefusal::NotAtPupil => "ai_work_06.refusal.not_at_pupil",
            EduRefusal::PupilNotChosen => "ai_work_06.refusal.pupil_not_chosen",
            EduRefusal::PupilNotSeated => "ai_work_06.refusal.pupil_not_seated",
            EduRefusal::Sitting => "ai_work_06.refusal.sitting",
            EduRefusal::NoPaper => "ai_work_06.refusal.no_paper",
            EduRefusal::PupilHasPaper => "ai_work_06.refusal.pupil_has_paper",
            EduRefusal::PaperMissing => "ai_work_06.refusal.paper_missing",
            EduRefusal::PaperInHutOnly => "ai_work_06.refusal.paper_in_hut_only",
            EduRefusal::NoPupil => "ai_work_06.refusal.no_pupil",
            EduRefusal::InventoryDumpDue => "ai_work_06.refusal.inventory_dump_due",
        }
    }

    /// Whether the pass this refusal is recorded on still **acts**.
    ///
    /// Java does not stop on these three: `study` sets its delay and pays
    /// `XP_PER_STUDYPOS` anyway (`EntityAIWorkResearcher.java:99-103`), a walk that
    /// gave up studies where the citizen stands (`:80`), and `teach` grants the
    /// Intelligence to the pupil even with no paper in hand (`EntityAIWorkTeacher.java:
    /// 162-171`).
    pub fn still_acts(self) -> bool {
        matches!(
            self,
            EduRefusal::NoResearchInProgress
                | EduRefusal::NoMana
                | EduRefusal::WalkAbandoned
                | EduRefusal::NoPaper
                | EduRefusal::PupilHasPaper
                | EduRefusal::PaperMissing
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_names_nine_states_and_no_two_share_a_name() {
        assert_eq!(EduState::ALL.len(), 9);
        let mut names: Vec<&str> = EduState::ALL.iter().map(|state| state.java()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 9, "two states share a Java name");
        // Seven base states, two trade states.
        let trade = EduState::ALL
            .iter()
            .filter(|state| state.worker().is_none())
            .count();
        assert_eq!(trade, 2);
        assert_eq!(EduState::Study.java_declaration(), "AIWorkerState.java:439");
        assert_eq!(EduState::Teach.java_declaration(), "AIWorkerState.java:622");
    }

    #[test]
    fn both_education_states_may_be_interrupted() {
        // `STUDY(true)` (`:439`), `TEACH(true)` (`:622`).
        assert!(EduState::Study.okay_to_eat());
        assert!(EduState::Teach.okay_to_eat());
        // A base state keeps the flag `crate::ai` already gives it (`:23`).
        assert!(!EduState::Base(WorkerState::InventoryFull).okay_to_eat());
        assert!(EduState::Base(WorkerState::StartWorking).okay_to_eat());
    }

    #[test]
    fn each_trade_owns_only_its_own_trade_state() {
        use super::super::EducationTrade;
        assert!(EduState::Study.belongs_to(EducationTrade::Researcher));
        assert!(!EduState::Study.belongs_to(EducationTrade::Teacher));
        assert!(EduState::Teach.belongs_to(EducationTrade::Teacher));
        assert!(!EduState::Teach.belongs_to(EducationTrade::Researcher));
        // A base state belongs to neither: it is `crate::ai`'s.
        assert!(!EduState::Base(WorkerState::Idle).belongs_to(EducationTrade::Teacher));
    }

    #[test]
    fn every_refusal_names_a_java_line_and_a_message_key() {
        let all = [
            EduRefusal::NoFactsForTrade,
            EduRefusal::NotAtBuilding,
            EduRefusal::NotAtBookshelf,
            EduRefusal::WalkAbandoned,
            EduRefusal::NoResearchInProgress,
            EduRefusal::NoMana,
            EduRefusal::NotAtPupil,
            EduRefusal::PupilNotChosen,
            EduRefusal::PupilNotSeated,
            EduRefusal::Sitting,
            EduRefusal::NoPaper,
            EduRefusal::PupilHasPaper,
            EduRefusal::PaperMissing,
            EduRefusal::PaperInHutOnly,
            EduRefusal::NoPupil,
            EduRefusal::InventoryDumpDue,
        ];
        for refusal in all {
            assert!(!refusal.java().is_empty(), "{refusal:?}");
            assert!(
                refusal.key().starts_with("ai_work_06.refusal."),
                "{refusal:?}"
            );
        }
        // The one entry that is the port's and not Java's says so out loud.
        assert!(EduRefusal::NoFactsForTrade.java().starts_with("port:"));
    }

    #[test]
    fn six_refusals_are_recorded_on_a_pass_that_still_acts() {
        // The three the documentation names, plus the two paper ones that only skip
        // the handout, plus the request that is created anyway.
        assert!(EduRefusal::NoResearchInProgress.still_acts());
        assert!(EduRefusal::NoMana.still_acts());
        assert!(EduRefusal::WalkAbandoned.still_acts());
        assert!(EduRefusal::NoPaper.still_acts());
        assert!(EduRefusal::PupilHasPaper.still_acts());
        assert!(EduRefusal::PaperMissing.still_acts());
        // A walk that has not arrived does stop the tick (`:75-79`).
        assert!(!EduRefusal::NotAtBookshelf.still_acts());
        assert!(!EduRefusal::Sitting.still_acts());
        assert!(!EduRefusal::NoPupil.still_acts());
    }
}
