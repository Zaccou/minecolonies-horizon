//! What an education goal asks the world, written as observations.
//!
//! Java reference: `core/entity/ai/workers/education/EntityAIWorkResearcher.java`
//! (126 lines) and `.../EntityAIWorkTeacher.java` (237), plus the four classes they
//! read — `colony/buildings/workerbuildings/BuildingUniversity.java` (`:115-128`),
//! `.../BuildingSchool.java` (`:124-146`), `colony/jobs/JobResearch.java` and
//! `api/util/InventoryUtils.java` (`:782-805`).
//!
//! **Why the base half is reused and the trade half is not.** The citizen, its
//! inventory, its hut and the block map already exist in `crate::ai::facts`, and this
//! module reads them from there ([`WorldFacts`], [`Pos`]). What does not exist is the
//! trade half: `WorldFacts` carries a `JobFacts` enum with exactly two variants
//! (composter, builder) and that file is outside this module's write boundary, so an
//! education trade's facts live in [`EduJob`] and [`EduScene`] *wraps* `WorldFacts`
//! instead of extending it. Nothing is copied: `EduScene::facts` is the same
//! `WorldFacts` value, so a caller that also ticks `crate::ai::worker::JobAi` hands the
//! one it already filled to both.
//!
//! **Two `String`s, and why they are not an API gap.** `getResearchInProgress()`
//! answers `ILocalResearch` records and the researcher hands one of them to
//! `research()` (`EntityAIWorkResearcher.java:83-97`). The port reads the *id* and the
//! two numbers that decide whether the pass concludes it — `ticks` and `progress`
//! (`LocalResearch.java:56-75`) — and the actual state change is
//! `crate::research::Progress::advance`, which already exists and is already tested.
//! The scene does not own a `Progress`: a decision reads, and the caller writes.
//!
//! **Absent is not false, and a draw is not a decision.** Every `*_draw` field below is
//! a number Java obtained from `worker.getRandom()`. The port never draws: a decision
//! that cannot be replayed cannot be tested, so the caller supplies the number and the
//! field says which call it stands for.

use crate::ai::facts::{Pos, WorldFacts};
use crate::ai::worker::Memory;

/// One research the colony is running, as the researcher's pass reads it.
///
/// Java reference: `getResearchInProgress()` (`EntityAIWorkResearcher.java:83`) gives
/// `ILocalResearch` values; the two numbers are `LocalResearch.getProgress()` and the
/// global research's `ticks` (the branch's base time at the record's depth,
/// `research/branch.rs` in this crate). `LocalResearch.research` adds one progress and
/// compares (`LocalResearch.java:56-75`), which is what [`InProgress::next_concludes`]
/// restates so the decision can tell the caller "this pass finishes it".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InProgress {
    /// `research.getId()`, the key `Progress::advance` takes.
    pub id: String,
    /// `LocalResearch.getProgress()`, in study actions.
    pub progress: u32,
    /// How many actions this research needs: `Branch::ticks(depth)`.
    pub ticks_total: u32,
}

impl InProgress {
    /// A research at `progress` of `ticks_total`.
    pub fn new(id: &str, progress: u32, ticks_total: u32) -> InProgress {
        InProgress {
            id: id.to_string(),
            progress,
            ticks_total,
        }
    }

    /// Would one more action finish it? `LocalResearch.research` (`LocalResearch.java:
    /// 56-75`) adds one and compares with `>=`, so a research one action short of its
    /// total concludes on this pass.
    pub fn next_concludes(&self) -> bool {
        self.progress.saturating_add(1) >= self.ticks_total
    }
}

/// What the researcher's pass reads (`EntityAIWorkResearcher.java:68-105`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResearcherFacts {
    /// `building.getRandomBookShelf()` (`:72`, `:121`).
    ///
    /// **This is never `null`**, which is why there is no "no bookshelf" refusal:
    /// `BuildingUniversity.getRandomBookShelf` answers the hut's own position when the
    /// bookshelf list is empty or the drawn shelf is no longer a bookshelf
    /// (`BuildingUniversity.java:115-128`). A university built without a library
    /// therefore makes its researcher study at the door.
    pub book_shelf: Pos,
    /// `walkToWorkPos(studyPos)`'s answer (`:75`): the navigation result, which the
    /// caller reports because the port has no pathfinder. `walkToPosInBuilding` walks
    /// while it answers false (`AbstractEntityAIBasic.java:800-808`).
    pub at_book_shelf: bool,
    /// `getResearchInProgress()` (`:83`), in the tree's order.
    pub in_progress: Vec<InProgress>,
    /// **Not a Java call.** `worker.getRandom().nextInt(inProgress.size())` (`:86`):
    /// which in-progress research this pass advances. The caller draws it.
    pub draw: usize,
}

impl Default for ResearcherFacts {
    /// A university with an empty bookshelf list — so the shelf *is* the hut —, a
    /// researcher already standing there, and no research running.
    fn default() -> ResearcherFacts {
        ResearcherFacts {
            book_shelf: Pos::new(0, 64, 0),
            at_book_shelf: true,
            in_progress: Vec::new(),
            draw: 0,
        }
    }
}

impl ResearcherFacts {
    /// The research this pass advances, `None` when the colony runs none.
    ///
    /// Java draws an index with `nextInt(size)` (`:86`); the port keeps the draw's
    /// answer as a fact and wraps it modulo the list, so a caller that draws
    /// `size - 1` gets the last one and a caller that hands a stale draw still gets a
    /// research instead of a panic (`nextInt` on 0 throws in Java; here the empty list
    /// is [`ResearcherFacts::in_progress`] being empty, which is a named refusal).
    pub fn advance_target(&self) -> Option<&InProgress> {
        if self.in_progress.is_empty() {
            return None;
        }
        self.in_progress.get(self.draw % self.in_progress.len())
    }
}

/// One pupil of the school, as the teacher's `decide` sees it.
///
/// Java reference: the filter of `WorldUtil.getEntitiesWithinBuilding`
/// (`EntityAIWorkTeacher.java:104-107`) and the pupil's own inventory in the teaching
/// pass (`:152`). The first three fields are **the predicate itself**: a citizen that
/// fails one of them is not a pupil the teacher may pick, and Java filters it out
/// before the list ever reaches the draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pupil {
    /// `AbstractEntityCitizen.getId()`, the identity a message and a test can print.
    pub id: u32,
    /// `pupilToTeach.blockPosition()` (`:125`), where the teacher walks.
    pub at: Pos,
    /// `cit.isBaby()` (`:106`).
    pub baby: bool,
    /// `cit.vehicle != null` (`:107`): the child is sitting on a school carpet. A pupil
    /// standing up is not teachable — the teacher sends itself back to `START_WORKING`
    /// when its chosen pupil stops being seated (`:119`).
    pub seated: bool,
    /// `cit.getCitizenJobHandler().getColonyJob() instanceof JobPupil` (`:107`).
    pub is_pupil_job: bool,
    /// `findFirstSlotInItemHandlerWith(pupilToTeach.getInventoryCitizen(), PAPER) != -1`
    /// (`:152`): the pupil already holds paper, so the handout is skipped.
    pub carries_paper: bool,
}

impl Pupil {
    /// A seated pupil of the right job, carrying nothing.
    pub fn seated(id: u32, at: Pos) -> Pupil {
        Pupil {
            id,
            at,
            baby: true,
            seated: true,
            is_pupil_job: true,
            carries_paper: false,
        }
    }

    /// The `cit -> cit.isBaby() && cit.vehicle != null && …JobPupil` predicate
    /// (`EntityAIWorkTeacher.java:106-107`), as one answer.
    pub fn teachable(&self) -> bool {
        self.baby && self.seated && self.is_pupil_job
    }
}

/// What the teacher's two passes read (`EntityAIWorkTeacher.java:89-176`).
#[derive(Clone, Debug, PartialEq)]
pub struct TeacherFacts {
    /// `InventoryUtils.hasBuildingEnoughElseCount(building, PAPER, 1)` (`:91`): how
    /// many sheets the school's racks hold, counted only inside loaded chunks
    /// (`InventoryUtils.java:782-805`).
    pub paper_in_building: u32,
    /// `InventoryUtils.getItemCountInItemHandler(worker.getInventoryCitizen(), PAPER)`
    /// (`:92`).
    pub paper_in_inventory: u32,
    /// `building.hasWorkerOpenRequestsFiltered(...)` for a `Stack` of paper
    /// (`:211-212`): a request is already on its way, so `requestPaper` does not make a
    /// second one.
    pub open_paper_request: bool,
    /// `WorldUtil.getEntitiesWithinBuilding(world, AbstractEntityCitizen.class,
    /// building, …)` (`:104-107`): every citizen inside the school's box. The filter is
    /// [`Pupil::teachable`], applied by [`TeacherFacts::pupils_to_teach`].
    pub pupils: Vec<Pupil>,
    /// **Not a Java call.** `worker.getRandom().nextInt(pupils.size())` (`:110`): which
    /// pupil this pass teaches.
    pub pupil_draw: usize,
    /// **Not a Java call.** `worker.getRandom().nextInt(jobModifier / 2)` (`:134`): the
    /// low half of the sitting time. The caller draws it; see
    /// [`super::teacher::max_sitting_ticks`] for the range.
    pub sit_roll: u32,
    /// `walkToWorkPos(pupilToTeach.blockPosition())`'s answer (`:125`): the navigation
    /// result, reported by the caller.
    pub at_pupil: bool,
    /// `worker.vehicle != null` (`:130`, `:145`): the teacher is sitting on its
    /// `SittingEntity`. Java re-sits when this is false even if the timer says
    /// otherwise, which is a real branch and not a detail.
    pub teacher_seated: bool,
    /// `getSecondarySkillLevel()` = `Skill.Mana` for this job
    /// (`BuildingModules.java:414-416`, `AbstractEntityAIBasic.java:660-663`), raw
    /// `0..99`. It is the **divisor** of the sitting time (`:133`): the better the
    /// teacher's Mana, the shorter the lesson.
    pub mana_level: i32,
    /// `getPrimarySkillLevel()` = `Skill.Knowledge` (`BuildingModules.java:414-416`),
    /// raw `0..99`, the last factor of the experience formula (`:165`).
    pub knowledge_level: i32,
    /// `worker.getCitizenData().getCitizenSkillHandler().getLevel(Skill.Intelligence)`
    /// (`:163`), raw `0..99`, the second factor.
    pub intelligence_level: i32,
    /// `getResearchEffects().getEffectStrength(TEACHING)` (`:164`), the research effect
    /// `effects/teachingmultiplier` (`ResearchConstants.java:138`). Zero when the
    /// colony has not researched it.
    pub teaching_effect: f64,
}

impl Default for TeacherFacts {
    /// A school with no paper, no pupil and a teacher of level zero standing at the
    /// origin, not sitting.
    fn default() -> TeacherFacts {
        TeacherFacts {
            paper_in_building: 0,
            paper_in_inventory: 0,
            open_paper_request: false,
            pupils: Vec::new(),
            pupil_draw: 0,
            sit_roll: 0,
            at_pupil: true,
            teacher_seated: false,
            mana_level: 0,
            knowledge_level: 0,
            intelligence_level: 0,
            teaching_effect: 0.0,
        }
    }
}

impl TeacherFacts {
    /// The pupils the draw may pick, in the order the caller reported them.
    ///
    /// Java filters inside `getEntitiesWithinBuilding` (`:104-107`); the port keeps the
    /// whole list as a fact and filters here, so a test can hand a school full of
    /// adults and standing children and watch the teacher ignore all of them.
    pub fn pupils_to_teach(&self) -> Vec<&Pupil> {
        self.pupils
            .iter()
            .filter(|pupil| pupil.teachable())
            .collect()
    }

    /// The pupil this pass would teach (`:110`), by the caller's draw.
    pub fn chosen_pupil(&self) -> Option<&Pupil> {
        let teachable = self.pupils_to_teach();
        if teachable.is_empty() {
            return None;
        }
        teachable.get(self.pupil_draw % teachable.len()).copied()
    }

    /// One pupil by id, teachable or not: `teach` must be able to notice that its
    /// chosen pupil stopped being seated (`:119-122`), which the filtered list hides.
    pub fn pupil(&self, id: u32) -> Option<&Pupil> {
        self.pupils.iter().find(|pupil| pupil.id == id)
    }

    /// `paperInBuilding + paperInInv` (`:93`).
    pub fn paper_available(&self) -> u32 {
        self.paper_in_building.saturating_add(self.paper_in_inventory)
    }
}

/// The job-specific half of the facts.
///
/// A job whose trade this module does not carry gets [`EduJob::Unported`], not an
/// empty [`EduJob::Researcher`]: "this worker has no research today" and "this port
/// cannot say what this worker does" must not read the same.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: the two education trades of this slice, plus "not ported"
pub enum EduJob {
    /// The university's researcher.
    Researcher(ResearcherFacts),
    /// The school's teacher.
    Teacher(TeacherFacts),
    /// This job's AI is not this module's; the base table still runs.
    Unported,
}

impl Default for EduJob {
    fn default() -> EduJob {
        EduJob::Unported
    }
}

/// What an education AI remembers between two ticks — **not** a fact about the world.
///
/// Java keeps these on the AI object and on the job: `studyPos`
/// (`EntityAIWorkResearcher.java:34`), `walkDelay` (`:39`), `pupilToTeach`
/// (`EntityAIWorkTeacher.java:55`), `maxSittingTicks` (`:60`) and `sittingTicks`
/// (`:65`). `delay` and `hasDelayed` are the base class's and are reused from
/// [`Memory`] in [`EduMemory::base`], because `setDelay` and `hasNotDelayed` are the
/// same two fields for every trade.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EduMemory {
    /// `AbstractEntityAIBasic.delay` and `.hasDelayed` (`:121`, `:126`).
    pub base: Memory,
    /// What the researcher keeps.
    pub researcher: ResearcherMemory,
    /// What the teacher keeps.
    pub teacher: TeacherMemory,
}

/// `EntityAIWorkResearcher`'s own two fields (`:34`, `:39`), plus the job's mana.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResearcherMemory {
    /// `studyPos` (`:34`): the bookshelf this pass walks to, `None` after a pass
    /// (`:101`) and before the first one.
    pub study_pos: Option<Pos>,
    /// `walkDelay` (`:39`): game ticks spent walking to the bookshelf, raised by
    /// `TICKS_SECOND` per attempt (`:77`) and reset on arrival (`:80`).
    pub walk_delay: i32,
    /// `JobResearch.currentMana` (`JobResearch.java:34`): the one thing that decides
    /// whether a pass advances a research (`EntityAIWorkResearcher.java:84`). It is
    /// **job state**, persisted under `TAG_CURR_MANA` (`JobResearch.java:113-116`), so
    /// it lives in the memory rather than in the facts.
    pub mana: u32,
}

/// `EntityAIWorkTeacher`'s own three fields (`:55`, `:60`, `:65`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TeacherMemory {
    /// `pupilToTeach` (`:55`): the citizen id picked by `decide`, `None` before the
    /// first pick. Java keeps the entity; the port keeps the id, because an entity
    /// handle is not data a decision can be replayed with.
    pub pupil_to_teach: Option<u32>,
    /// `maxSittingTicks` (`:60`): how many seconds the lesson lasts, `0` when no lesson
    /// is in flight.
    pub max_sitting_ticks: i32,
    /// `sittingTicks` (`:65`): how many seconds it has lasted so far.
    pub sitting_ticks: i32,
}

/// The whole scene one education decision reads: the base facts, the trade facts, the
/// AI's memory, and the two channels out.
///
/// [`WorldFacts`] is *the same value* the `crate::ai` tables take — nothing is copied
/// out of it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EduScene {
    /// The citizen, its inventory, its hut and its blocks (`crate::ai::facts`).
    pub facts: WorldFacts,
    /// This trade's own observations.
    pub job: EduJob,
    /// What the AI remembers between ticks.
    pub memory: EduMemory,
    /// The last refusal, cleared at the top of every tick.
    pub refusal: Option<super::state::EduRefusal>,
    /// What the pass asks the caller to do, cleared at the top of every tick. Not a
    /// fact and not a memory: an order, which the caller either carries out or drops.
    pub effects: Vec<super::Effect>,
}

impl EduScene {
    /// A scene on `facts` with an empty memory and `job`, the trade facts.
    pub fn new(facts: WorldFacts, job: EduJob) -> EduScene {
        EduScene {
            facts,
            job,
            memory: EduMemory::default(),
            refusal: None,
            effects: Vec::new(),
        }
    }

    /// The researcher's facts, if this is a researcher.
    pub fn researcher(&self) -> Option<&ResearcherFacts> {
        match &self.job {
            EduJob::Researcher(facts) => Some(facts),
            _ => None,
        }
    }

    /// The teacher's facts, if this is a teacher.
    pub fn teacher(&self) -> Option<&TeacherFacts> {
        match &self.job {
            EduJob::Teacher(facts) => Some(facts),
            _ => None,
        }
    }

    /// Record why this tick cannot go further. Last one wins, as in
    /// `crate::ai::worker::Scene::refuse`.
    pub fn refuse(&mut self, refusal: super::state::EduRefusal) {
        self.refusal = Some(refusal);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_research_one_action_short_concludes_and_one_further_does_not() {
        // `LocalResearch.research` adds one and compares with `>=` (`:56-75`).
        assert!(InProgress::new("harmony", 9, 10).next_concludes());
        assert!(InProgress::new("harmony", 10, 10).next_concludes(), "already over");
        assert!(!InProgress::new("harmony", 8, 10).next_concludes());
        // Saturating, so a corrupt record does not panic.
        assert!(InProgress::new("harmony", u32::MAX, u32::MAX).next_concludes());
    }

    #[test]
    fn the_draw_wraps_over_the_running_list_and_an_empty_list_has_no_target() {
        let empty = ResearcherFacts::default();
        assert!(empty.advance_target().is_none());

        let facts = ResearcherFacts {
            in_progress: vec![
                InProgress::new("a", 0, 5),
                InProgress::new("b", 1, 5),
            ],
            draw: 3,
            ..ResearcherFacts::default()
        };
        // `nextInt(2)` can only answer 0 or 1; 3 wraps instead of panicking.
        assert_eq!(facts.advance_target().map(|r| r.id.as_str()), Some("b"));
    }

    #[test]
    fn only_a_seated_baby_with_the_pupil_job_is_teachable() {
        // `EntityAIWorkTeacher.java:106-107`.
        let at = Pos::new(4, 65, 4);
        assert!(Pupil::seated(1, at).teachable());
        assert!(!Pupil { seated: false, ..Pupil::seated(1, at) }.teachable());
        assert!(!Pupil { baby: false, ..Pupil::seated(1, at) }.teachable());
        assert!(!Pupil { is_pupil_job: false, ..Pupil::seated(1, at) }.teachable());
    }

    #[test]
    fn the_filtered_list_holds_only_the_teachable_pupils_and_the_draw_picks_inside_it() {
        let at = Pos::new(4, 65, 4);
        let facts = TeacherFacts {
            pupils: vec![
                Pupil { seated: false, ..Pupil::seated(1, at) },
                Pupil::seated(2, at),
                Pupil::seated(3, at),
            ],
            pupil_draw: 1,
            ..TeacherFacts::default()
        };
        assert_eq!(facts.pupils_to_teach().len(), 2);
        assert_eq!(facts.chosen_pupil().map(|p| p.id), Some(3));
        // The unfiltered lookup still finds the standing one: `teach` needs it to notice
        // that its pick stopped sitting (`:119-122`).
        assert_eq!(facts.pupil(1).map(|p| p.seated), Some(false));
        assert_eq!(facts.pupil(9), None);
    }

    #[test]
    fn a_school_with_no_teachable_pupil_picks_nobody() {
        let at = Pos::new(0, 64, 0);
        let facts = TeacherFacts {
            pupils: vec![Pupil { baby: false, ..Pupil::seated(1, at) }],
            ..TeacherFacts::default()
        };
        assert!(facts.chosen_pupil().is_none());
        assert!(facts.pupils_to_teach().is_empty());
    }

    #[test]
    fn the_paper_total_is_the_hut_plus_the_pocket_and_saturates() {
        // `:93` adds the two counts.
        let facts = TeacherFacts {
            paper_in_building: 3,
            paper_in_inventory: 5,
            ..TeacherFacts::default()
        };
        assert_eq!(facts.paper_available(), 8);
        let huge = TeacherFacts {
            paper_in_building: u32::MAX,
            paper_in_inventory: 1,
            ..TeacherFacts::default()
        };
        assert_eq!(huge.paper_available(), u32::MAX, "no overflow panic");
    }

    #[test]
    fn the_default_scene_has_no_trade_facts() {
        let scene = EduScene::default();
        assert_eq!(scene.job, EduJob::Unported);
        assert!(scene.researcher().is_none());
        assert!(scene.teacher().is_none());
        assert!(scene.effects.is_empty());
        // A researcher whose bookshelf list is empty studies at the hut itself
        // (`BuildingUniversity.java:115-120`), so the default shelf *is* the hut.
        let default = ResearcherFacts::default();
        assert_eq!(default.book_shelf, WorldFacts::new().building.position);
        assert!(default.at_book_shelf);
    }
}
