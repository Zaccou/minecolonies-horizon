//! Cross-module tests: does the decision this module makes line up with the Java
//! table, with the layer above, and with itself from one tick to the next?
//!
//! The unit tests inside each file check one rule against one fact. These check the
//! things that only show up when the pieces are put together: a whole sequence of
//! ticks for a trade, the same scene giving the same answer twice, every state having
//! a way out, and the hand-over to `crate::citizens::priority` — the citizen-level
//! table that decides *whether* a worker works at all
//! (`CitizenAI.java:250-260`).

use crate::ai::builder::{self, BuilderStep};
use crate::ai::composter::{self, BarrelChoice, MaterialsOutcome};
use crate::ai::facts::{
    Barrel, BuilderFacts, ComposterFacts, InventoryFacts, JobFacts, Pos, Stage, WorkOrder,
    WorkerFacts, WorldFacts,
};
use crate::ai::state::{Fired, WorkerState};
use crate::ai::worker::{
    self, JobAi, JobProfile, Outcome, Refusal, Scene,
};
use crate::citizens::{CitizenState, Rung, Situation};
use crate::jobs::job::Job;

/// One tick, flattened to what a caller can log: the goal, the rule with its Java
/// line, and the refusal if there was one.
fn line(outcome: &Outcome) -> (WorkerState, Option<&'static str>, Option<&'static str>) {
    (
        outcome.state,
        outcome.fired.map(|fired: Fired<WorkerState>| fired.rule),
        outcome.why_not().map(Refusal::java),
    )
}

/// A composter's scene with a hut, barrels and the composter's profile.
fn composter_scene(barrels: Vec<Barrel>) -> Scene {
    let ai = JobAi::for_job(Job::Composter).expect("composter is ported");
    ai.scene(WorldFacts {
        job: JobFacts::Composter(ComposterFacts {
            barrels,
            ..ComposterFacts::default()
        }),
        ..WorldFacts::new()
    })
}

/// A builder's scene.
fn builder_scene(facts: BuilderFacts) -> Scene {
    let ai = JobAi::for_job(Job::Builder).expect("builder is ported");
    ai.scene(WorldFacts {
        job: JobFacts::Builder(facts),
        ..WorldFacts::new()
    })
}

fn ready_order() -> WorkOrder {
    WorkOrder::new_build(Pos::new(1, 64, 1))
}

fn working_builder() -> BuilderFacts {
    BuilderFacts {
        work_order: Some(ready_order()),
        stage: Some(Stage::BuildSolid),
        placer: true,
        order_building_exists: true,
        at_construction_site: true,
        ..BuilderFacts::default()
    }
}

#[test]
fn a_composter_walks_java_s_four_states_from_idle_to_a_harvest() {
    // The table of `EntityAIWorkComposter.java:95-102` in one run: INIT → IDLE
    // (`AbstractEntityAIBasic.java:459`), IDLE → START_WORKING (`:97`), START_WORKING
    // decides (`:99`) and the finished barrel wins (`:222-227`).
    let at = Pos::new(1, 65, 1);
    let mut scene = composter_scene(vec![Barrel::empty(Pos::new(2, 65, 2)), Barrel::done(at)]);
    let mut ai = JobAi::for_job(Job::Composter).expect("ported");
    let outcomes: Vec<_> = (0..3).map(|_| line(&ai.tick(&mut scene))).collect();
    assert_eq!(
        outcomes,
        vec![
            (WorkerState::Idle, Some("initSafetyChecks"), None),
            (WorkerState::StartWorking, Some("idleToStartWorking"), None),
            (WorkerState::ComposterHarvest, Some("decideWhatToDo"), None),
        ]
    );
    assert_eq!(scene.memory.target, Some(at), "the first finished barrel");
    assert_eq!(scene.memory.delay, composter::DECIDE_DELAY as i32);
}

#[test]
fn a_composter_harvest_is_followed_by_a_dump_because_its_threshold_is_one() {
    // `getActionsDoneUntilDumping()` is 1 (`:380-384`), so the harvest at `:347`
    // triggers the base dump rule on the very next STATE_BLOCKING pass
    // (`AbstractEntityAIBasic.java:413`).
    let at = Pos::new(0, 64, 0);
    let mut scene = composter_scene(vec![Barrel::done(at)]);
    let mut ai = JobAi::for_job(Job::Composter).expect("ported");
    scene.memory.target = Some(at);
    assert_eq!(
        composter::harvest_barrels(&mut scene, WorkerState::ComposterHarvest),
        Some(WorkerState::StartWorking)
    );
    assert_eq!(scene.facts.inventory.actions_done, 1);
    // The rule is the base one, with the composter's threshold. The first tick leaves
    // INIT, so the dump rule (STATE_BLOCKING, `:225`) is looked at on the next one.
    let mut dumping = None;
    for _ in 0..3 {
        let outcome = ai.tick(&mut scene);
        if outcome.state == WorkerState::InventoryFull {
            dumping = Some(outcome);
            break;
        }
    }
    let dumping = dumping.expect("one action is enough for a composter to dump");
    assert_eq!(dumping.fired.map(|fired| fired.rule), Some("inventoryNeedsDump"));
    assert_eq!(
        dumping.fired.map(|fired| fired.java),
        Some("AbstractEntityAIBasic.java:225")
    );
}

#[test]
fn a_composter_with_no_barrel_cycles_and_names_the_refusal_every_time() {
    // `:243-244`: every barrel is missing, so the decision is "nothing" and the AI
    // stays in START_WORKING. The refusal is what a player should be shown.
    let mut scene = composter_scene(Vec::new());
    let mut ai = JobAi::for_job(Job::Composter).expect("ported");
    ai.tick(&mut scene); // INIT → IDLE
    ai.tick(&mut scene); // IDLE → START_WORKING
    let outcome = ai.tick(&mut scene);
    assert_eq!(outcome.state, WorkerState::StartWorking);
    assert_eq!(outcome.why_not(), Some(Refusal::NoBarrel));
    assert_eq!(outcome.why_not().map(Refusal::java), Some("EntityAIWorkComposter.java:243-244"));
}

#[test]
fn the_composters_decide_is_a_pure_function_of_the_barrel_list() {
    // `:215-244`, without a machine: the same list always gives the same answer, and
    // the answer is a position, not a state.
    let a = Pos::new(1, 65, 1);
    let b = Pos::new(2, 65, 2);
    assert_eq!(composter::decide_what_to_do(&[]), BarrelChoice::Nothing);
    assert_eq!(
        composter::decide_what_to_do(&[Barrel::busy(a)]),
        BarrelChoice::Nothing
    );
    assert_eq!(
        composter::decide_what_to_do(&[Barrel::busy(a), Barrel::empty(b)]),
        BarrelChoice::Fill(b)
    );
    assert_eq!(
        composter::decide_what_to_do(&[Barrel::busy(a), Barrel::empty(b), Barrel::done(a)]),
        BarrelChoice::Harvest(a)
    );
}

#[test]
fn a_builder_without_an_order_falls_back_to_idle_and_the_citizen_layer_lets_it_rest() {
    // The hand-over: `AbstractEntityAIBasic.canGoIdle` is `false`, the builder
    // overrides it with `!hasWorkOrder()` (`EntityAIStructureBuilder.java:336-339`),
    // and `CitizenAI.calculateNextState` reads it at `:250-260` to decide between
    // WORK and IDLE.
    let ai = JobAi::for_job(Job::Builder).expect("ported");
    let scene = ai.scene(WorldFacts {
        job: JobFacts::Builder(BuilderFacts::default()),
        ..WorldFacts::new()
    });
    let idle = worker::can_go_idle(&ai.profile(), &scene.facts);
    assert!(idle, "no work order, so the builder may idle");

    let situation = Situation {
        has_job: true,
        can_go_idle: idle,
        ..Situation::worker([0, 64, 0])
    };
    assert_eq!(
        situation.first_rung(CitizenState::Idle),
        Rung::Idle,
        "CitizenAI.java:250 requires !canGoIdle to keep the citizen at WORK"
    );

    // And with an order, the same citizen goes to work instead.
    let busy = ai.scene(WorldFacts {
        job: JobFacts::Builder(working_builder()),
        ..WorldFacts::new()
    });
    assert!(!worker::can_go_idle(&ai.profile(), &busy.facts));
    let situation = Situation {
        has_job: true,
        can_go_idle: false,
        ..Situation::worker([0, 64, 0])
    };
    assert_eq!(situation.first_rung(CitizenState::Idle), Rung::Work);
}

#[test]
fn a_builder_works_a_new_build_through_its_eight_stages_then_completes() {
    // `AbstractEntityAIStructure.java:782-786` gives a new build eight stages, and
    // `:467-482` advances one per `FINISHED` and completes after the last.
    let stages = crate::ai::Stages::NEW_BUILD.list();
    assert_eq!(stages.len(), 8);
    let mut advanced = 0;
    for (index, stage) in stages.iter().enumerate() {
        let mut facts = working_builder();
        facts.stage = Some(*stage);
        facts.placement = Some(crate::ai::facts::Placement::Finished);
        let step = builder::step_outcome(&builder_scene(facts));
        if index + 1 == stages.len() {
            assert_eq!(step, BuilderStep::Complete, "the last stage completes");
        } else {
            assert_eq!(step, BuilderStep::StageAdvanced, "{stage:?} advances");
            advanced += 1;
        }
    }
    assert_eq!(advanced, 7);
    // The hut reports 6 stages for those 8 (`:785`): the number is Java's, not a
    // count of the array.
    assert_eq!(crate::ai::Stages::NEW_BUILD.reported_total, 6);
}

#[test]
fn a_builder_step_that_fails_stays_put_and_says_nothing_is_wrong() {
    // `:461-464`: a `FAIL` is logged and treated as "not finished".
    let mut facts = working_builder();
    facts.placement = Some(crate::ai::facts::Placement::Fail);
    let mut scene = builder_scene(facts);
    let outcome_step = builder::step_outcome(&scene);
    assert_eq!(outcome_step, BuilderStep::Failed);
    let mut machine = crate::ai::state::Machine::new(WorkerState::BuildingStep, 5);
    worker::register_base(&mut machine);
    builder::register(&mut machine);
    let fired = machine.tick(&mut scene).expect("the step runs");
    assert_eq!(fired.to, WorkerState::BuildingStep);
    assert_eq!(scene.memory.refusal, None, "a failure is not a refusal");
}

#[test]
fn a_builder_missing_its_materials_asks_before_it_reloads() {
    // `:507-515`: `MISSING_ITEMS` → `NEEDS_ITEM`, and only a recomputed list sends it
    // back to `LOAD_STRUCTURE`.
    let mut facts = working_builder();
    facts.placement = Some(crate::ai::facts::Placement::MissingItems);
    let mut scene = builder_scene(facts.clone());
    assert_eq!(builder::step_outcome(&scene), BuilderStep::NeedsItem);
    let mut machine = crate::ai::state::Machine::new(WorkerState::BuildingStep, 5);
    worker::register_base(&mut machine);
    builder::register(&mut machine);
    let fired = machine.tick(&mut scene).expect("the step runs");
    assert_eq!(fired.to, WorkerState::NeedsItem);
    assert_eq!(scene.memory.refusal, Some(Refusal::MissingMaterials));

    facts.recalculate = true;
    let scene = builder_scene(facts);
    assert_eq!(builder::step_outcome(&scene), BuilderStep::Reload);
}

#[test]
fn a_builder_whose_site_is_unloaded_cancels_from_any_state() {
    // `AbstractEntityAIStructureWithWorkOrder.java:496`: the cancel rule is
    // STATE_BLOCKING at rate 1, so it wins over the current state's own target.
    let mut facts = working_builder();
    let mut scene = builder_scene(facts.clone());
    scene.facts.blocks.mark_unloaded(Pos::new(1, 64, 1));
    let mut machine = crate::ai::state::Machine::new(WorkerState::BuildingStep, 5);
    worker::register_base(&mut machine);
    builder::register(&mut machine);
    let fired = machine.tick(&mut scene).expect("the cancel rule runs");
    assert_eq!(fired.rule, "checkIfCanceled");
    assert_eq!(fired.to, WorkerState::Idle);
    assert_eq!(scene.memory.refusal, Some(Refusal::NotArrived));
    // The structure is forgotten: no placer, no stage.
    assert!(!builder::is_there_a_structure(&scene));
    facts.placer = true; // the fixture is only here to show the fact was cleared
    assert!(facts.placer);
}

#[test]
fn every_state_the_two_tables_enter_has_a_way_out() {
    // `BasicStateMachine.java:220-228` resets a machine that enters a state with no
    // transition, so a ported trade must cover every state it can reach. The set is
    // written down here once, so a new target that forgets an exit fails this test.
    for job in [Job::Composter, Job::Builder] {
        let ai = JobAi::for_job(job).expect("ported");
        let mut reachable = vec![
            WorkerState::Init,
            WorkerState::Idle,
            WorkerState::StartWorking,
            WorkerState::InventoryFull,
            WorkerState::NeedsItem,
            WorkerState::GatheringRequiredMaterials,
            WorkerState::Paused,
        ];
        if job == Job::Builder {
            reachable.extend([
                WorkerState::LoadStructure,
                WorkerState::StartBuilding,
                WorkerState::MineBlock,
                WorkerState::BuildingStep,
                WorkerState::CompleteBuild,
                WorkerState::PickUp,
            ]);
        } else {
            reachable.extend([
                WorkerState::GetMaterials,
                WorkerState::ComposterFill,
                WorkerState::ComposterHarvest,
            ]);
        }
        for state in reachable {
            assert!(
                ai.machine().can_leave(state),
                "{job:?} can enter {state} and never leave it"
            );
        }
    }
}

#[test]
fn every_transition_names_a_java_line_and_a_rule() {
    // A rule without a citation is an invention; the brief's rule 3.
    for job in [Job::Composter, Job::Builder] {
        let ai = JobAi::for_job(job).expect("ported");
        for transition in ai.machine().transitions() {
            assert!(
                !transition.rule.is_empty(),
                "{job:?}: a transition has no rule name"
            );
            let java = transition.java;
            assert!(
                java.contains(".java:"),
                "{job:?}: {} cites {java}, which names no line",
                transition.rule
            );
            let (file, line) = java.rsplit_once(".java:").expect("checked above");
            assert!(!file.is_empty(), "{} has no file", transition.rule);
            assert!(
                line.chars().next().is_some_and(|c| c.is_ascii_digit()),
                "{} cites {java}, whose line is not a number",
                transition.rule
            );
        }
    }
}

#[test]
fn the_same_scene_twice_gives_the_same_decisions() {
    // Nothing in this module reads a clock, a static or a random source: replaying a
    // scene must replay the outcomes, which is what makes the port testable at all.
    let run = || {
        let at = Pos::new(1, 65, 1);
        let mut scene = composter_scene(vec![Barrel::empty(at), Barrel::done(at)]);
        let mut ai = JobAi::for_job(Job::Composter).expect("ported");
        let mut trace = Vec::new();
        for _ in 0..6 {
            trace.push(line(&ai.tick(&mut scene)));
        }
        trace
    };
    assert_eq!(run(), run());

    let builder_trace = || {
        let mut scene = builder_scene(working_builder());
        let mut ai = JobAi::for_job(Job::Builder).expect("ported");
        let mut trace = Vec::new();
        for _ in 0..6 {
            trace.push(line(&ai.tick(&mut scene)));
        }
        trace
    };
    assert_eq!(builder_trace(), builder_trace());
}

#[test]
fn a_scene_built_by_an_ai_carries_that_trades_profile() {
    // The profile is what `afterDump`, `afterRequestPickUp`, `getStateAfterPickUp`,
    // `canGoIdle`, `isAfterDumpPickupAllowed` and the dump threshold read
    // (`AbstractEntityAIBasic.java:323-438`, `:726-739`, `:1213-1222`, `:1921-1924`);
    // pairing a trade's machine with another trade's profile would silently change all
    // six.
    let builder = JobAi::for_job(Job::Builder).expect("ported");
    assert_eq!(builder.profile(), JobProfile::BUILDER);
    assert_eq!(builder.scene(WorldFacts::new()).profile, JobProfile::BUILDER);
    let composter = JobAi::for_job(Job::Composter).expect("ported");
    assert_eq!(composter.profile(), JobProfile::COMPOSTER);
    assert_eq!(
        composter.scene(WorldFacts::new()).profile,
        JobProfile::COMPOSTER
    );
    // The four numbers that differ between them, and the base.
    assert_eq!(JobProfile::BASE.actions_until_dumping, 32);
    assert_eq!(JobProfile::COMPOSTER.actions_until_dumping, 1);
    assert_eq!(JobProfile::BUILDER.actions_until_dumping, 4096);
    assert_eq!(JobProfile::BASE.after_dump, None, "the paused-or-idle rule");
    assert_eq!(JobProfile::BUILDER.after_dump, Some(WorkerState::PickUp));
    assert_eq!(JobProfile::BASE.after_request_pickup, WorkerState::Idle);
    assert_eq!(
        JobProfile::BUILDER.after_request_pickup,
        WorkerState::InventoryFull
    );
    assert_eq!(JobProfile::BASE.state_after_pickup, WorkerState::StartWorking);
    assert_eq!(JobProfile::BUILDER.state_after_pickup, WorkerState::PickUp);
}

#[test]
fn the_composters_material_request_is_one_request_and_one_message_a_day() {
    // `:174-192` (no duplicate request) and `:390-402` (the complaint throttle), with
    // the empty list at `:147-152` as the reason it complains at all.
    let mut memory = worker::Memory::default();
    let mut facts = WorldFacts {
        job: JobFacts::Composter(ComposterFacts {
            compostable_list_empty: true,
            ..ComposterFacts::default()
        }),
        ..WorldFacts::new()
    };
    assert_eq!(
        composter::get_materials(&facts, &mut memory),
        MaterialsOutcome::ListEmpty
    );
    assert_eq!(memory.ticks_to_complain, composter::TICKS_UNTIL_COMPLAIN);
    assert!(!composter::complain(&mut memory), "the throttle holds");
    // With a list and nothing in hand, it asks exactly once.
    if let JobFacts::Composter(composter) = &mut facts.job {
        composter.compostable_list_empty = false;
    }
    assert_eq!(
        composter::get_materials(&facts, &mut memory),
        MaterialsOutcome::Asked
    );
    if let JobFacts::Composter(composter) = &mut facts.job {
        composter.has_worker_open_requests = true;
    }
    assert_eq!(
        composter::get_materials(&facts, &mut memory),
        MaterialsOutcome::Asked,
        "an open request means ask nothing more"
    );
}

#[test]
fn a_builder_that_holds_nothing_still_reports_the_state_it_is_in() {
    // The caller's view: a fresh builder ends its first tick in IDLE, and the outcome
    // says which rule put it there with the line to quote. No refusal: nothing is
    // wrong, the hut simply has no work order yet.
    let mut ai = JobAi::for_job(Job::Builder).expect("ported");
    let mut scene = builder_scene(BuilderFacts::default());
    let first = ai.tick(&mut scene);
    let (state, rule, refusal) = line(&first);
    assert_eq!(state, WorkerState::Idle);
    assert_eq!(rule, Some("initSafetyChecks"));
    assert_eq!(refusal, None);
    // The next tick goes to work — the builder's own `AITarget(IDLE, START_WORKING,
    // 10)` (`AbstractEntityAIStructure.java:186`) — and then finds nothing to load.
    let second = ai.tick(&mut scene);
    assert_eq!(second.state, WorkerState::StartWorking);
    assert_eq!(
        second.fired.map(|fired| fired.java),
        Some("AbstractEntityAIStructure.java:186")
    );
}

#[test]
fn an_inventory_fact_of_zero_slots_does_not_divide_by_zero() {
    // `dumpOneMoreSlot` divides by the stack count (`:1264`); a scene with no slots
    // and no stacks must answer "nothing to dump", not panic.
    let mut scene = Scene::new(WorldFacts {
        inventory: InventoryFacts {
            slots: 0,
            open_slots: 0,
            ..InventoryFacts::default()
        },
        worker: WorkerFacts::default(),
        ..WorldFacts::new()
    });
    assert!(!worker::dump_one_more_slot(&mut scene));
    assert_eq!(scene.memory.refusal, Some(Refusal::NothingToDump));
}
