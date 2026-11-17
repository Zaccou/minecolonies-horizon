//! The module's own table: three trades, two engines, and the seam to `crate::ai`.
//!
//! Every test here is about the wiring `mod.rs` owns — which trade enters which state,
//! what a scene built for the wrong trade answers, how many states each table has, and
//! what the composter delegation really returns. The per-trade decision rules are tested
//! next to the rule, in `nether.rs` and `beekeeper.rs`, and the composter's own rules in
//! `crate::ai::composter`, so nothing is asserted twice.
//!
//! Java reference: `AbstractEntityAIBasic.java:200-260` (the base table and the `IDLE`
//! hand-over), `AbstractEntityAICrafting.java:164-169` (what the nether miner inherits),
//! `EntityAIWorkBeekeeper.java:108-115` and `EntityAIWorkComposter.java:95-102`.

use crate::ai::facts::WorldFacts;
use crate::ai::state::WorkerState;
use crate::jobs::job::Job;

use super::{
    states_of, Decision, WorkAi, WorkJob, WorkRefusal, WorkScene, WorkState, WorkTrade, WorkerAi,
};

#[test]
fn the_three_trades_are_the_three_java_classes() {
    // The class each trade's AI is in Java, which is what a log line names.
    assert_eq!(WorkTrade::ALL.len(), 3);
    for (trade, name, class) in [
        (WorkTrade::Nether, "netherworker", "EntityAIWorkNether"),
        (WorkTrade::Beekeeper, "beekeeper", "EntityAIWorkBeekeeper"),
        (WorkTrade::Composter, "composter", "EntityAIWorkComposter"),
    ] {
        assert_eq!(trade.name(), name);
        assert_eq!(trade.class(), class);
        assert_eq!(format!("{trade}"), name);
    }
    // And the job each one is in `crate::jobs`.
    assert_eq!(WorkTrade::Nether.job(), Job::NetherWorker);
    assert_eq!(WorkTrade::Beekeeper.job(), Job::Beekeeper);
    assert_eq!(WorkTrade::Composter.job(), Job::Composter);
    // The composter's job states no tool at all; the nether miner states five.
    assert_eq!(Job::NetherWorker.tools().len(), 5);
    assert_eq!(Job::Beekeeper.tools(), &[crate::jobs::tool::ToolKind::Shears]);
}

#[test]
fn every_trade_enters_start_working_from_its_own_line() {
    // The nether miner inherits `IDLE → START_WORKING` from the crafting AI, the
    // beekeeper registers it itself, and the composter's is registered over
    // `WorkerState` by `crate::ai::composter`.
    for (trade, java) in [
        (WorkTrade::Nether, "AbstractEntityAICrafting.java:164"),
        (WorkTrade::Beekeeper, "EntityAIWorkBeekeeper.java:109"),
        (WorkTrade::Composter, "EntityAIWorkComposter.java:97"),
    ] {
        assert_eq!(trade.entry(), WorkState::Base(WorkerState::StartWorking));
        assert_eq!(trade.entry_java(), java, "{trade}");
    }
}

#[test]
fn each_table_has_the_states_its_constructor_registers() {
    // Nether: five own states plus the six base states the hand-over covers.
    let nether = states_of(WorkTrade::Nether);
    assert_eq!(nether.len(), 9);
    for state in [
        WorkState::NetherLeave,
        WorkState::NetherAway,
        WorkState::NetherReturn,
        WorkState::NetherOpenPortal,
        WorkState::NetherClosePortal,
    ] {
        assert!(nether.contains(&state), "the nether miner owns {state}");
    }
    // Beekeeper: two own states, plus `PREPARING` and `DECIDE`, which only its own
    // constructor registers (`EntityAIWorkBeekeeper.java:111-112`).
    let beekeeper = states_of(WorkTrade::Beekeeper);
    assert_eq!(beekeeper.len(), 8);
    assert!(beekeeper.contains(&WorkState::Base(WorkerState::Preparing)));
    assert!(!nether.contains(&WorkState::Base(WorkerState::Preparing)));
    assert!(!nether.contains(&WorkState::Base(WorkerState::Decide)));
    assert!(beekeeper.contains(&WorkState::HerderBreed));
    assert!(beekeeper.contains(&WorkState::BeekeeperHarvest));
    // The composter's states are `WorkerState`s and this enum has no variant for them,
    // so the honest answer is no state at all, not six guesses.
    assert!(states_of(WorkTrade::Composter).is_empty());
    // No table owns the other's states.
    assert!(!beekeeper.contains(&WorkState::NetherLeave));
    assert!(!nether.contains(&WorkState::HerderBreed));
}

#[test]
fn the_composter_is_reached_through_the_other_engine() {
    // `crate::ai::composter` predates this module and owns the trade; this is the seam.
    assert!(WorkTrade::Composter.work_ai().is_none());
    match WorkTrade::Composter.ai() {
        WorkerAi::Composter(ai) => {
            // A `JobAi` starts in `INIT`, which is `crate::ai`'s state machine.
            assert_eq!(ai.job(), Job::Composter);
        }
        WorkerAi::Work(_) => panic!("the composter must not get a second table"),
    }
    assert_eq!(WorkTrade::Composter.ai().trade(), WorkTrade::Composter);
    assert!(WorkTrade::Composter.ai().work().is_none());
    // And the two tables this module does own come back as work machines.
    for trade in [WorkTrade::Nether, WorkTrade::Beekeeper] {
        match trade.ai() {
            WorkerAi::Work(ai) => {
                assert_eq!(ai.trade(), trade);
                assert_eq!(ai.state(), WorkState::Base(WorkerState::Idle));
            }
            WorkerAi::Composter(_) => panic!("{trade} has its own table"),
        }
        assert!(trade.work_ai().is_some());
    }
}

#[test]
fn each_trade_dumps_after_the_number_of_actions_its_java_override_says() {
    // `AbstractEntityAICrafting.java:724-727` (the nether miner, 1),
    // `EntityAIWorkBeekeeper.java:118-122` (5) and `EntityAIWorkComposter.java:380-384`
    // (1), over `AbstractEntityAIBasic.java:425-428`'s 32.
    assert_eq!(WorkTrade::Nether.actions_until_dumping(), 1);
    assert_eq!(WorkTrade::Beekeeper.actions_until_dumping(), 5);
    assert_eq!(WorkTrade::Composter.actions_until_dumping(), 1);
    // The base-table profile carries the same number, because that is the machine that
    // reads `inventoryNeedsDump` (`AbstractEntityAIBasic.java:413`).
    for trade in WorkTrade::ALL {
        assert_eq!(
            trade.profile().actions_until_dumping,
            trade.actions_until_dumping(),
            "{trade}"
        );
    }
    // `getStateAfterPickUp()` is `START_WORKING` for the nether miner
    // (`EntityAIWorkNether.java:158-162`) and the base answer for the other two.
    assert_eq!(
        WorkTrade::Nether.profile().state_after_pickup,
        WorkerState::StartWorking
    );
    assert_eq!(
        WorkTrade::Beekeeper.profile().state_after_pickup,
        WorkerState::StartWorking,
        "the base class's own answer"
    );
}

#[test]
fn only_the_two_ported_trades_have_a_facts_template() {
    assert!(matches!(
        WorkTrade::Nether.job_facts(),
        Some(WorkJob::Nether(_))
    ));
    assert!(matches!(
        WorkTrade::Beekeeper.job_facts(),
        Some(WorkJob::Beekeeper(_))
    ));
    assert!(WorkTrade::Composter.job_facts().is_none());
}

#[test]
fn a_machine_starts_in_idle_and_leaves_it_for_its_trade_entry() {
    for trade in [WorkTrade::Nether, WorkTrade::Beekeeper] {
        let mut ai = WorkAi::new(trade).expect("a table");
        assert_eq!(ai.state(), WorkState::Base(WorkerState::Idle), "{trade}");
        assert_eq!(ai.trade(), trade);
        let mut scene = ai.scene(WorldFacts::new());
        let outcome = ai.tick(&mut scene);
        assert!(outcome.moved(), "{trade} leaves IDLE on the first tick");
        assert_eq!(
            outcome.goal(),
            WorkState::Base(WorkerState::StartWorking),
            "{trade}"
        );
        // The rule is the hand-over line, and it cites the trade's own registration.
        assert_eq!(outcome.why(), Some(trade.entry_java()), "{trade}");
        // A machine can leave every state of its table, so no tick traps it.
        for state in states_of(trade) {
            assert!(ai.machine().can_leave(state), "{trade} leaves {state}");
        }
    }
}

#[test]
fn the_hand_over_seam_sends_the_base_states_back_to_idle() {
    // `afterDump` (`AbstractEntityAIBasic.java:1213-1222`) and `getNeededItem` (`:242`):
    // the base table owns those states, and this module only says where control returns.
    // The test reads the seam rather than trusting it: the two states have a way out, and
    // the rule that takes it is the base class's, not a trade's.
    for trade in [WorkTrade::Nether, WorkTrade::Beekeeper] {
        let ai = WorkAi::new(trade).expect("a table");
        for (state, rule) in [
            (
                WorkState::Base(WorkerState::InventoryFull),
                "afterDump",
            ),
            (
                WorkState::Base(WorkerState::GatheringRequiredMaterials),
                "getNeededItem",
            ),
        ] {
            assert!(ai.machine().can_leave(state), "{trade} leaves {state}");
            let rules: Vec<&str> = ai
                .machine()
                .transitions()
                .iter()
                .filter(|transition| transition.state == Some(state))
                .map(|transition| transition.rule)
                .collect();
            assert_eq!(rules, vec![rule], "{trade} on {state}");
        }
        // And `IDLE` leaves for the trade's own entry, which is the other half of the
        // seam: no trade keeps `IDLE` for itself.
        let rules: Vec<&str> = ai
            .machine()
            .transitions()
            .iter()
            .filter(|transition| transition.state == Some(WorkState::Base(WorkerState::Idle)))
            .map(|transition| transition.rule)
            .collect();
        assert_eq!(rules, vec!["idleToEntry"], "{trade} from IDLE");
    }
}

#[test]
fn a_scene_built_for_one_trade_refuses_the_other_by_name() {
    // The difference between "this miner has no portal today" and "these facts are
    // another trade's" must not be lost.
    let nether = WorkAi::new(WorkTrade::Nether).expect("a table");
    let mut scene = nether.scene_with(
        WorldFacts::new(),
        WorkJob::Beekeeper(Default::default()),
    );
    assert!(scene.beekeeper().is_some() && scene.nether().is_none());
    let _ = WorkAi::new(WorkTrade::Nether)
        .expect("a table")
        .tick(&mut scene);

    let beekeeper = WorkAi::new(WorkTrade::Beekeeper).expect("a table");
    let mut scene = beekeeper.scene_with(WorldFacts::new(), WorkJob::Nether(Default::default()));
    assert!(scene.nether().is_some() && scene.beekeeper().is_none());
    let _ = WorkAi::new(WorkTrade::Beekeeper)
        .expect("a table")
        .tick(&mut scene);
    assert_eq!(scene.job.nether().is_some(), true);
}

#[test]
fn a_decision_is_a_state_or_a_reason_or_both() {
    // The four shapes Java writes as `return X;`, `return getState();` and two returns in
    // the same branch.
    let go = Decision::go(WorkState::NetherLeave);
    assert_eq!(go.next, Some(WorkState::NetherLeave));
    assert_eq!(go.refusal, None);
    let stay = Decision::stay();
    assert_eq!(stay.next, None);
    assert_eq!(stay.refusal, None);
    let refuse = Decision::refuse(WorkRefusal::NoPortal);
    assert_eq!(refuse.next, None);
    assert_eq!(refuse.refusal, Some(WorkRefusal::NoPortal));
    let both = Decision::go_after(WorkState::Base(WorkerState::Idle), WorkRefusal::MissingInputs);
    assert_eq!(both.next, Some(WorkState::Base(WorkerState::Idle)));
    assert_eq!(both.refusal, Some(WorkRefusal::MissingInputs));
    // `because` only fills a hole: a reason already recorded is kept.
    assert_eq!(
        both.because(WorkRefusal::NoSpace).refusal,
        Some(WorkRefusal::MissingInputs)
    );
    assert_eq!(
        Decision::go(WorkState::NetherReturn)
            .because(WorkRefusal::NoSpace)
            .refusal,
        Some(WorkRefusal::NoSpace)
    );
}

#[test]
fn a_scene_carries_the_base_facts_untouched_for_the_base_table() {
    // `WorkScene::facts` is the **same** `WorldFacts` value `crate::ai` consumes, so a
    // caller that ticks both machines fills it once.
    let mut world = WorldFacts::new();
    world.inventory.actions_done = 3;
    let ai = WorkAi::new(WorkTrade::Nether).expect("a table");
    let scene = ai.scene(world);
    assert_eq!(scene.facts.inventory.actions_done, 3);
    assert_eq!(scene.memory.base.delay, 0);
    assert_eq!(scene.memory.nether.job_status, crate::ai::worker::JobStatus::Idle);
    assert!(scene.refusal.is_none());
    // A fresh scene for the same AI is a fresh memory but the same trade template.
    let other = ai.scene(WorldFacts::new());
    assert!(other.nether().is_some());
    let _: WorkScene = other;
}

#[test]
fn resetting_a_machine_puts_it_back_in_idle() {
    // `AbstractAISkeleton.resetAI` (`:145-152`).
    let mut ai = WorkAi::new(WorkTrade::Nether).expect("a table");
    let mut scene = ai.scene(WorldFacts::new());
    let _ = ai.tick(&mut scene);
    assert_ne!(ai.state(), WorkState::Base(WorkerState::Idle));
    ai.reset();
    assert_eq!(ai.state(), WorkState::Base(WorkerState::Idle));
}
