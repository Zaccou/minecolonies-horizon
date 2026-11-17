//! The scenarios: a set of facts, ticked, that walks the **same sequence of states** as
//! Java does — which is the test this module exists for.
//!
//! Each scenario names the Java method whose body it is replaying and asserts on the
//! states in the order the machine entered them, not on a single call. A single call
//! would pass with the branches in any order; a sequence will not.
//!
//! **What a sequence proves, and what it does not.** The order and the branching are
//! Java's. The *timing* is not: `setDelay(1200)` (`EntityAIWorkResearcher.java:103`) is
//! spent by the base class's `waitingForSomething`
//! (`AbstractEntityAIBasic.java:496-517`), which belongs to `crate::ai::worker`'s table
//! and cannot live inside this machine (see `MANQUANT.md` §1). So a loop here is faster
//! than a loop in the game by exactly the delay the base table would have burned, and
//! the scenarios assert the delay *value* instead — a caller that ticks both machines
//! gets Java's pace back, and
//! `the_researcher_sets_a_one_minute_delay_the_base_table_owns` is the test that pins it.
//!
//! **Why 20 game ticks per tick of the counter.** Both constructors build their targets
//! with `TICKS_SECOND` (`EntityAIWorkResearcher.java:51-52`,
//! `EntityAIWorkTeacher.java:77-79`) and the machine ticks at `ENTITY_AI_TICKRATE` = 5
//! (`AbstractEntityCitizen.java:64`), so a trade pass runs once every four machine ticks
//! — one second of game time. That is what makes `sittingTicks` a second counter and
//! `walkDelay` a second counter, and it is why the tests below multiply by four.

use crate::ai::facts::{Pos, WorldFacts};
use crate::ai::state::WorkerState;
use crate::ai_work_06::facts::{EduScene, InProgress, Pupil, ResearcherFacts, TeacherFacts};
use crate::ai_work_06::{
    EduAi, EduJob, EduOutcome, EduRefusal, EduState, EducationTrade, Effect,
};

/// Tick `ai` `ticks` times on `scene` and give back every outcome, in order.
fn run(ai: &mut EduAi, scene: &mut EduScene, ticks: usize) -> Vec<EduOutcome> {
    (0..ticks).map(|_| ai.tick(scene)).collect()
}

/// The states a run passed through, with consecutive repeats folded — so a transition
/// that holds its state (`return getState()`) does not hide the ones that moved.
fn states(outcomes: &[EduOutcome]) -> Vec<EduState> {
    let mut path: Vec<EduState> = Vec::new();
    for outcome in outcomes {
        if path.last() != Some(&outcome.state) {
            path.push(outcome.state);
        }
    }
    path
}

/// The whole journey: where the machine started, then every state it stood in. This is
/// the sequence to compare with Java's own history ring, and it starts at `IDLE` because
/// that is where `crate::ai` hands the citizen over.
fn journey(start: EduState, outcomes: &[EduOutcome]) -> Vec<EduState> {
    let mut path = vec![start];
    for state in states(outcomes) {
        if path.last() != Some(&state) {
            path.push(state);
        }
    }
    path
}

/// Every refusal the run recorded, in order, without the repeats.
fn refusals(outcomes: &[EduOutcome]) -> Vec<EduRefusal> {
    let mut seen: Vec<EduRefusal> = Vec::new();
    for outcome in outcomes {
        if let Some(refusal) = outcome.refusal {
            if seen.last() != Some(&refusal) {
                seen.push(refusal);
            }
        }
    }
    seen
}

/// Every effect the run left, in order.
fn effects(outcomes: &[EduOutcome]) -> Vec<Effect> {
    outcomes
        .iter()
        .flat_map(|outcome| outcome.effects.iter().cloned())
        .collect()
}

/// The rules that actually fired, in order: the transition history Java keeps in its
/// twenty-entry ring (`BasicStateMachine.java:66-70`).
fn rules(outcomes: &[EduOutcome]) -> Vec<&'static str> {
    outcomes
        .iter()
        .filter_map(|outcome| outcome.fired.map(|fired| fired.rule))
        .collect()
}

/// How many times `rule` ran — one run per second of AI time for a trade target.
fn passes(outcomes: &[EduOutcome], rule: &str) -> usize {
    rules(outcomes).into_iter().filter(|fired| *fired == rule).count()
}

fn in_progress(id: &str, progress: u32, total: u32) -> InProgress {
    InProgress::new(id, progress, total)
}

/// A university whose researcher has arrived, has mana, and has one research running.
fn researcher_scene(mana: u32) -> EduScene {
    let job = EduJob::Researcher(ResearcherFacts {
        book_shelf: Pos::new(2, 65, 2),
        in_progress: vec![in_progress("harmony", 0, 10)],
        ..ResearcherFacts::default()
    });
    let mut scene = EduScene::new(WorldFacts::new(), job);
    scene.memory.researcher.mana = mana;
    scene
}

/// A school with paper in the teacher's pocket and one seated pupil.
fn teacher_scene(mana_level: i32, paper_in_pocket: u32) -> EduScene {
    let job = EduJob::Teacher(TeacherFacts {
        paper_in_inventory: paper_in_pocket,
        pupils: vec![Pupil::seated(9, Pos::new(5, 65, 5))],
        mana_level,
        // The teacher is on its seat: the *first* pass books it (`maxSittingTicks == 0`),
        // and a fact that said otherwise would have it re-seat on every pass.
        teacher_seated: true,
        ..TeacherFacts::default()
    });
    EduScene::new(WorldFacts::new(), job)
}

#[test]
fn the_researcher_runs_idle_start_working_study_and_back() {
    // `EntityAIWorkResearcher.java:49-53` — `IDLE → START_WORKING → STUDY → START_WORKING`.
    let mut ai = EduAi::new(EducationTrade::Researcher);
    let start = ai.state();
    let mut scene = researcher_scene(4);
    // A 20-tick target fires on the first look and then once per four machine ticks, so
    // twenty machine ticks hold three passes and the walk into the fourth.
    let outcomes = run(&mut ai, &mut scene, 20);

    assert_eq!(
        journey(start, &outcomes),
        vec![
            EduState::Base(WorkerState::Idle),
            EduState::Base(WorkerState::StartWorking),
            EduState::Study,
            EduState::Base(WorkerState::StartWorking),
            EduState::Study,
            EduState::Base(WorkerState::StartWorking),
            EduState::Study,
            EduState::Base(WorkerState::StartWorking),
        ],
        "one pass, then the same loop"
    );
    // The rules that moved it, in order — the two constructor lines and the hand-over.
    assert_eq!(
        rules(&outcomes),
        vec![
            "idleToEntry",
            "startWorkingAtOwnBuilding",
            "study",
            "startWorkingAtOwnBuilding",
            "study",
            "startWorkingAtOwnBuilding",
            "study",
        ]
    );
    // Three passes: one research advanced per pass, one mana per pass.
    let advances = effects(&outcomes)
        .into_iter()
        .filter(|effect| matches!(effect, Effect::Studied { research: Some(_), .. }))
        .count();
    assert_eq!(advances, 3);
    assert_eq!(scene.memory.researcher.mana, 1, "4 - 3 passes");
    // Nothing refused: this is the happy path.
    assert!(refusals(&outcomes).is_empty());
    assert!(ai.missing().is_none());
}

#[test]
fn a_researcher_that_never_arrives_stays_in_study_and_advances_nothing() {
    // `EntityAIWorkResearcher.java:75-79` — `walkDelay < STUDY_DELAY` is `getState()`.
    let mut ai = EduAi::new(EducationTrade::Researcher);
    let job = EduJob::Researcher(ResearcherFacts {
        at_book_shelf: false,
        in_progress: vec![in_progress("harmony", 0, 10)],
        ..ResearcherFacts::default()
    });
    let start = ai.state();
    let mut scene = EduScene::new(WorldFacts::new(), job);
    scene.memory.researcher.mana = 4;
    let outcomes = run(&mut ai, &mut scene, 40);

    assert_eq!(
        journey(start, &outcomes),
        vec![
            EduState::Base(WorkerState::Idle),
            EduState::Base(WorkerState::StartWorking),
            EduState::Study,
        ],
        "STUDY holds: the walk is not over"
    );
    assert_eq!(scene.memory.researcher.mana, 4, "nothing spent while walking");
    assert!(effects(&outcomes).is_empty(), "no pass, no experience");
    // Every look recorded the same refusal, and the walk budget grew by a second each.
    assert_eq!(refusals(&outcomes), vec![EduRefusal::NotAtBookshelf]);
    assert_eq!(passes(&outcomes, "study"), 10, "ten of the sixty seconds");
    assert_eq!(scene.memory.researcher.walk_delay, 10 * 20, "a second per pass");
}

#[test]
fn a_teacher_runs_idle_start_working_decide_teach_and_back() {
    // `EntityAIWorkTeacher.java:75-80` then `:104-112` then `:175`.
    let mut ai = EduAi::new(EducationTrade::Teacher);
    // Mana 99 makes the lesson one second long (`:133-134`), so the whole loop fits.
    let start = ai.state();
    let mut scene = teacher_scene(99, 1);
    let outcomes = run(&mut ai, &mut scene, 14);

    assert_eq!(
        journey(start, &outcomes),
        vec![
            EduState::Base(WorkerState::Idle),
            EduState::Base(WorkerState::StartWorking),
            EduState::Base(WorkerState::Decide),
            EduState::Teach,
            EduState::Base(WorkerState::StartWorking),
            EduState::Base(WorkerState::Decide),
            EduState::Teach,
        ],
        "DECIDE → TEACH → START_WORKING → DECIDE, as Java's `:114` and `:175` read"
    );
    assert_eq!(
        rules(&outcomes),
        vec![
            "idleToEntry",
            "startWorkingAtOwnBuilding",
            "decide",
            "teach",
            "startWorkingAtOwnBuilding",
            "decide",
        ]
    );
    // The lesson handed one sheet over and paid both experiences (`:151-171`).
    let produced = effects(&outcomes);
    assert!(
        produced.contains(&Effect::GivePaper {
            pupil: 9,
            count: 1
        }),
        "one sheet, in {produced:?}"
    );
    assert!(
        produced
            .iter()
            .any(|effect| matches!(effect, Effect::Taught { pupil: 9, .. })),
        "one lesson"
    );
    // One second of sitting, then the counters are back to zero (`:173-174`).
    assert!(
        produced
            .iter()
            .any(|effect| matches!(effect, Effect::Sit { ticks, .. } if *ticks == 20)),
        "a one-second seat, in {produced:?}"
    );
    assert_eq!(scene.memory.teacher.sitting_ticks, 0);
    assert!(ai.missing().is_none());
}

#[test]
fn a_school_with_no_seated_pupil_loops_start_working_decide_forever() {
    // `EntityAIWorkTeacher.java:114` — `return START_WORKING;`, which walks to the hut,
    // which decides again. The loop is Java's, and it is why the trade needs a pupil.
    let mut ai = EduAi::new(EducationTrade::Teacher);
    let job = EduJob::Teacher(TeacherFacts {
        paper_in_inventory: 4,
        pupils: vec![Pupil {
            seated: false,
            ..Pupil::seated(1, Pos::new(5, 65, 5))
        }],
        ..TeacherFacts::default()
    });
    let start = ai.state();
    let mut scene = EduScene::new(WorldFacts::new(), job);
    let outcomes = run(&mut ai, &mut scene, 20);

    assert_eq!(
        journey(start, &outcomes),
        vec![
            EduState::Base(WorkerState::Idle),
            EduState::Base(WorkerState::StartWorking),
            EduState::Base(WorkerState::Decide),
            EduState::Base(WorkerState::StartWorking),
            EduState::Base(WorkerState::Decide),
            EduState::Base(WorkerState::StartWorking),
            EduState::Base(WorkerState::Decide),
            EduState::Base(WorkerState::StartWorking),
        ],
        "the loop Java's `:114` makes"
    );
    assert_eq!(
        refusals(&outcomes),
        vec![EduRefusal::NoPupil],
        "the one reason, repeated"
    );
    assert!(effects(&outcomes).is_empty());
}

#[test]
fn a_teacher_fetches_the_school_s_paper_before_teaching() {
    // `EntityAIWorkTeacher.java:98-102` — `GATHERING_REQUIRED_MATERIALS`, whose base
    // `getNeededItem` comes back to `START_WORKING` (`:323-326`).
    let mut ai = EduAi::new(EducationTrade::Teacher);
    let job = EduJob::Teacher(TeacherFacts {
        paper_in_building: 5,
        paper_in_inventory: 0,
        pupils: vec![Pupil::seated(2, Pos::new(5, 65, 5))],
        ..TeacherFacts::default()
    });
    let start = ai.state();
    let mut scene = EduScene::new(WorldFacts::new(), job);
    let outcomes = run(&mut ai, &mut scene, 16);

    assert_eq!(
        journey(start, &outcomes),
        vec![
            EduState::Base(WorkerState::Idle),
            EduState::Base(WorkerState::StartWorking),
            EduState::Base(WorkerState::Decide),
            EduState::Base(WorkerState::GatheringRequiredMaterials),
            EduState::Base(WorkerState::StartWorking),
            EduState::Base(WorkerState::Decide),
            EduState::Base(WorkerState::GatheringRequiredMaterials),
            EduState::Base(WorkerState::StartWorking),
        ],
        "the fetch comes before the pupil, and the hand-over returns to START_WORKING"
    );
    // The fetch asked for sixteen, once per pass.
    let produced = effects(&outcomes);
    assert_eq!(
        produced.first(),
        Some(&Effect::FetchPaper { count: 16 }),
        "in {produced:?}"
    );
    assert_eq!(scene.memory.base.need, Some(16), "`needsCurrently` (`:100`)");
    assert!(
        !states(&outcomes).contains(&EduState::Teach),
        "the paper is on the way: nothing is taught yet"
    );
}

#[test]
fn a_lesson_lasts_exactly_the_seconds_the_formula_asked_for() {
    // `:133-143`: `sittingTicks++` once per `teach` call and `sittingTicks <
    // maxSittingTicks` holds the state. Mana 10 gives `jobModifier = 100 / 5 = 20`; a
    // roll of 0 gives `20 / 2 + 0` = ten seconds, so exactly ten `teach` passes.
    let mut ai = EduAi::new(EducationTrade::Teacher);
    let job = EduJob::Teacher(TeacherFacts {
        paper_in_inventory: 1,
        pupils: vec![Pupil::seated(9, Pos::new(5, 65, 5))],
        mana_level: 10,
        sit_roll: 0,
        teacher_seated: true,
        ..TeacherFacts::default()
    });
    let mut scene = EduScene::new(WorldFacts::new(), job);
    // The teach pass is at ticks 4, 8, … 40: ten of them, then the way back.
    let outcomes = run(&mut ai, &mut scene, 42);

    assert_eq!(passes(&outcomes, "teach"), 10, "ten seconds of lesson");
    assert_eq!(
        passes(&outcomes, "decide"),
        1,
        "one pupil picked, one lesson given"
    );
    // The seat was booked for the same ten seconds, in game ticks (`:136`).
    assert!(
        effects(&outcomes)
            .iter()
            .any(|effect| matches!(effect, Effect::Sit { ticks, .. } if *ticks == 200)),
        "ten seconds times `SIT_TICK_FACTOR`"
    );
    // One lesson only, and it paid a level-zero pupil's 1.5 (`:163`).
    let xp: Vec<f64> = effects(&outcomes)
        .into_iter()
        .filter_map(|effect| match effect {
            Effect::Taught { pupil_xp, .. } => Some(pupil_xp),
            _ => None,
        })
        .collect();
    assert_eq!(xp, vec![1.5]);
    assert_eq!(scene.memory.teacher.sitting_ticks, 0);
    assert_eq!(scene.memory.teacher.max_sitting_ticks, 0);
}

#[test]
fn an_empty_school_produces_one_paper_request_and_no_lesson() {
    // The comparison case for the test above: same machine, no pupil, no paper — the
    // only order that leaves is the `Stack` request of `:209-216`, once. The *once* is
    // the caller's side of the contract: `requestPaper` reads
    // `hasWorkerOpenRequestsFiltered` (`:211-212`), so a caller that created the request
    // has to say so, and this loop does exactly that.
    let mut ai = EduAi::new(EducationTrade::Teacher);
    let mut scene = EduScene::new(WorldFacts::new(), EduJob::Teacher(TeacherFacts::default()));
    let mut outcomes = Vec::new();
    let mut requests = 0;
    for _ in 0..20 {
        let outcome = ai.tick(&mut scene);
        if outcome
            .effects
            .iter()
            .any(|effect| matches!(effect, Effect::RequestPaper { .. }))
        {
            requests += 1;
            // The caller created the `Stack`: the fact the next pass reads flips.
            if let EduJob::Teacher(facts) = &mut scene.job {
                facts.open_paper_request = true;
            }
        }
        outcomes.push(outcome);
    }

    assert_eq!(requests, 1, "one request, then the open-request flag holds it");
    assert!(!states(&outcomes).contains(&EduState::Teach));
    assert!(refusals(&outcomes).contains(&EduRefusal::PaperMissing));
    // Nothing was taught, so nothing was handed over.
    assert!(
        !effects(&outcomes)
            .iter()
            .any(|effect| matches!(effect, Effect::Taught { .. } | Effect::GivePaper { .. }))
    );
}

#[test]
fn no_scenario_falls_into_a_trap_state() {
    // `BasicStateMachine.java:220-228` — a state with no transition is a trap, and every
    // state either machine can reach must be leavable. Ticking both trades for a while
    // and asserting no reset is the cheap version of that proof.
    for (trade, mut scene) in [
        (EducationTrade::Researcher, researcher_scene(1)),
        (EducationTrade::Teacher, teacher_scene(0, 1)),
    ] {
        let mut ai = EduAi::new(trade);
        let outcomes = run(&mut ai, &mut scene, 40);
        assert!(ai.missing().is_none(), "{trade} trapped");
        assert!(
            outcomes.iter().all(|outcome| !outcome.faulted()),
            "{trade} reset a state it could not leave"
        );
        for state in states(&outcomes) {
            assert!(
                ai.machine().can_leave(state),
                "{trade} reached {state} and cannot leave it"
            );
        }
    }
}

#[test]
fn the_researcher_sets_a_one_minute_delay_the_base_table_owns() {
    // `EntityAIWorkResearcher.java:103` — `setDelay(STUDY_DELAY)`. The machine records it;
    // `AbstractEntityAIBasic.waitingForSomething` (`:496-517`) is what spends it, and that
    // is `crate::ai::worker`'s table (see `MANQUANT.md` §1). A caller that ticks both gets
    // one pass per minute; this test pins the number the caller must respect.
    let mut ai = EduAi::new(EducationTrade::Researcher);
    let mut scene = researcher_scene(2);
    let outcomes = run(&mut ai, &mut scene, 6);
    assert_eq!(scene.memory.base.delay, 1200, "sixty seconds of game ticks");
    // And the pass really did happen in that tick, not after the delay.
    assert!(
        outcomes
            .iter()
            .any(|outcome| matches!(outcome.effects.as_slice(), [Effect::Studied { .. }])),
        "the study pass ran"
    );
    // The trade has no rule of its own in `IDLE`'s list: the hand-over is one line.
    let idle_rules: Vec<&'static str> = ai
        .machine()
        .transitions()
        .iter()
        .filter(|transition| transition.state == Some(EduState::Base(WorkerState::Idle)))
        .map(|transition| transition.rule)
        .collect();
    assert_eq!(idle_rules, vec!["idleToEntry"]);
}

#[test]
fn an_education_machine_starts_where_the_base_table_arrives() {
    // `AbstractAISkeleton` starts in `INIT` and `initSafetyChecks` sends it to `IDLE`
    // (`AbstractEntityAIBasic.java:204-205`, `:446-465`), which `crate::ai` owns.
    for trade in EducationTrade::ALL {
        let ai = EduAi::new(trade);
        assert_eq!(ai.state(), EduState::Base(WorkerState::Idle));
        assert_eq!(
            ai.machine().init_state(),
            EduState::Base(WorkerState::Idle)
        );
        assert!(ai.machine().tick_rate() > 0);
    }
}
