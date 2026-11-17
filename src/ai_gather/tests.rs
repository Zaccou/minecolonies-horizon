//! The module's own table: four trades, one machine, and the seam to `crate::ai`.
//!
//! Every test here is about the wiring `mod.rs` owns — which trade enters which state,
//! what a scene built for the wrong trade answers, what `dump_due` reads. The per-trade
//! decision rules are tested next to the rule, in `lumberjack.rs`, `miner.rs`,
//! `farmer.rs` and `fisherman.rs`, so nothing is asserted twice.
//!
//! Java reference: `AbstractEntityAIBasic.java:200-260` (the base table and the `IDLE`
//! hand-over), the four constructors' `registerTargets`, and
//! `AbstractEntityAIBasic.inventoryNeedsDump` (`:409-416`).

use crate::ai::facts::{Pos, WorldFacts};
use crate::ai::state::WorkerState;

use super::facts::{FarmerFacts, GatherJob, GatherScene, MinerFacts};
use super::{
    at_hut, dump_due, has_not_delayed, set_delay, states_of, Decision, GatherAi, GatherRefusal,
    GatherState, GatherTrade, TICKS_SECOND,
};

/// A scene whose trade half is the trade's own, empty facts.
fn scene(trade: GatherTrade) -> GatherScene {
    GatherScene::new(WorldFacts::new(), trade.job_facts())
}

#[test]
fn the_four_trades_are_the_four_java_classes() {
    // The class each trade's AI is in Java, which is what a log line names.
    assert_eq!(GatherTrade::ALL.len(), 4);
    let table = [
        (GatherTrade::Lumberjack, "lumberjack", "EntityAIWorkLumberjack"),
        (GatherTrade::Miner, "miner", "EntityAIStructureMiner"),
        (GatherTrade::Farmer, "farmer", "EntityAIWorkFarmer"),
        (GatherTrade::Fisherman, "fisherman", "EntityAIWorkFisherman"),
    ];
    for (trade, name, class) in table {
        assert_eq!(trade.name(), name);
        assert_eq!(trade.class(), class);
    }
    // The miner has no `EntityAIWorkMiner`: its AI is a structure AI
    // (`EntityAIStructureMiner.java:64`).
    assert_ne!(GatherTrade::Miner.class(), "EntityAIWorkMiner");
}

#[test]
fn every_trade_enters_the_state_its_constructor_names() {
    // All four get `IDLE → START_WORKING` from their parent, and each leaves
    // `START_WORKING` itself: `AbstractEntityAICrafting.java:164` (forester, farmer),
    // `AbstractEntityAIStructure.java:186` (miner) and
    // `EntityAIWorkFisherman.java:169` (fisherman). The forester only reaches
    // `LUMBERJACK_START_WORKING` from `decide()` (`EntityAIWorkLumberjack.java:251`).
    for (trade, java) in [
        (GatherTrade::Lumberjack, "AbstractEntityAICrafting.java:164"),
        (GatherTrade::Miner, "AbstractEntityAIStructure.java:186"),
        (GatherTrade::Farmer, "AbstractEntityAICrafting.java:164"),
        (GatherTrade::Fisherman, "EntityAIWorkFisherman.java:169"),
    ] {
        assert_eq!(
            trade.entry(),
            GatherState::Base(WorkerState::StartWorking),
            "{trade}"
        );
        assert_eq!(trade.entry_java(), java, "{trade}");
    }
    // What `START_WORKING` decides is the trade's own: the forester's `decide` answers
    // `LUMBERJACK_START_WORKING` (`:251`), the farmer's answers `PREPARING`
    // (`:184-187`), the miner walks then prepares (`:199-206`).
}

#[test]
fn a_machine_starts_in_idle_and_hands_over_to_that_entry() {
    for trade in GatherTrade::ALL {
        let mut ai = GatherAi::new(trade);
        assert_eq!(
            ai.state(),
            GatherState::Base(WorkerState::Idle),
            "{trade} starts in IDLE"
        );
        let mut scene = scene(trade);
        let outcome = ai.tick(&mut scene);
        assert_eq!(outcome.goal(), trade.entry(), "{trade} hands over");
        assert!(outcome.moved(), "{trade}: IDLE really left");
        assert!(!outcome.faulted(), "{trade}: no trap");
        assert_eq!(
            outcome.why(),
            Some(trade.entry_java()),
            "{trade}: the rule that fired is the base class's"
        );
        // `resetAI` (`AbstractAISkeleton.java:145-152`) puts it back.
        ai.reset();
        assert_eq!(ai.state(), GatherState::Base(WorkerState::Idle));
    }
}

#[test]
fn states_of_is_the_base_seam_plus_the_trade_s_own_states() {
    for trade in GatherTrade::ALL {
        let states = states_of(trade);
        assert!(
            states.contains(&GatherState::Base(WorkerState::Idle)),
            "{trade}: IDLE is the seam (`AbstractEntityAIBasic.java:1213-1222`)"
        );
        assert!(
            states.contains(&GatherState::Base(WorkerState::InventoryFull)),
            "{trade}: a full inventory comes back through the dump"
        );
        for other in GatherTrade::ALL {
            if other == trade {
                continue;
            }
            let foreign: Vec<GatherState> = states
                .iter()
                .copied()
                .filter(|state| state.belongs_to(other))
                .filter(|state| !state.belongs_to(trade))
                .collect();
            assert!(
                foreign.is_empty(),
                "{trade} must not carry {other}'s states: {foreign:?}"
            );
        }
    }
    // The miner's three structure states are the one asymmetry
    // (`EntityAIStructureMiner.java:199-206`).
    let miner = states_of(GatherTrade::Miner);
    assert!(miner.contains(&GatherState::Base(WorkerState::MineBlock)));
    assert!(miner.contains(&GatherState::Base(WorkerState::LoadStructure)));
    assert!(miner.contains(&GatherState::Base(WorkerState::BuildingStep)));
    let farmer = states_of(GatherTrade::Farmer);
    assert!(!farmer.contains(&GatherState::Base(WorkerState::MineBlock)));
}

#[test]
fn a_scene_built_for_another_trade_is_refused_by_name() {
    // A farmer's table handed a miner's facts answers `NoFactsForTrade` instead of
    // pretending the field is empty. The first tick only hands over, so the refusal
    // arrives on the target's own rate.
    let mut ai = GatherAi::new(GatherTrade::Farmer);
    let mut scene = ai.scene_with(WorldFacts::new(), GatherJob::Miner(MinerFacts::default()));
    let mut refused = false;
    for _ in 0..(TICKS_SECOND * 4) {
        if ai.tick(&mut scene).why_not() == Some(GatherRefusal::NoFactsForTrade) {
            refused = true;
            break;
        }
    }
    assert!(refused, "the macro records the mismatch by name");
}

#[test]
fn the_dump_is_due_on_a_full_inventory_or_at_the_count() {
    // `inventoryNeedsDump` (`AbstractEntityAIBasic.java:409-416`): `:412` is the full
    // inventory, `:413` the count, and the farmer adds its own flag (`:159`).
    let mut scene = scene(GatherTrade::Farmer);
    assert!(!dump_due(GatherTrade::Farmer, &scene), "empty inventory, no work");

    scene.facts.inventory.full = true; // `:412`
    assert!(dump_due(GatherTrade::Farmer, &scene), "full inventory");
    scene.facts.inventory.full = false;

    let threshold = super::farmer::MAX_BLOCKS_MINED; // `:80`, `:197-201`
    scene.facts.inventory.actions_done = threshold; // `:413`
    assert!(dump_due(GatherTrade::Farmer, &scene), "the count is reached");
    scene.facts.inventory.actions_done = threshold - 1;
    assert!(!dump_due(GatherTrade::Farmer, &scene), "one action short");

    // `wantInventoryDumped` (`EntityAIWorkFarmer.java:156-165`): a farmer-only flag.
    scene.memory.farmer.should_dump_inventory = true;
    assert!(dump_due(GatherTrade::Farmer, &scene));
    let mut lumberjack = scene_with_lumberjack();
    lumberjack.memory.farmer.should_dump_inventory = true;
    assert!(
        !dump_due(GatherTrade::Lumberjack, &lumberjack),
        "the flag is the farmer's, not every trade's"
    );
}

/// A forester's scene carrying the farmer's memory flag set, to show the flag is read
/// only when the job is a farmer's.
fn scene_with_lumberjack() -> GatherScene {
    GatherScene::new(
        WorldFacts::new(),
        GatherJob::Lumberjack(super::facts::LumberjackFacts::default()),
    )
}

#[test]
fn the_hut_is_a_distance_rule_not_a_state_rule() {
    // `walkToBuilding` (`AbstractEntityAIBasic.java:757-766`) with
    // `WOKR_IN_BUILDING_DIST`.
    let mut world = WorldFacts::new();
    world.building.position = Pos::new(0, 64, 0);
    world.worker.at = Pos::new(20, 64, 0);
    assert!(!at_hut(&world), "twenty blocks away is not at the hut");
    world.worker.at = Pos::new(1, 64, 0);
    assert!(at_hut(&world), "next to the hut is");
}

#[test]
fn a_delay_is_set_once_and_waited_out_once() {
    // `hasNotDelayed` / `setDelay` (`AbstractEntityAIBasic.java:1441-1451`, `:397-400`):
    // the first answer arms the delay, the second clears it.
    let mut scene = scene(GatherTrade::Lumberjack);
    assert_eq!(scene.memory.base.delay, 0);

    set_delay(&mut scene, TICKS_SECOND);
    assert_eq!(scene.memory.base.delay, TICKS_SECOND as i32, "`:397-400`");
    scene.memory.base.has_delayed = false;

    assert!(has_not_delayed(&mut scene, TICKS_SECOND), "the delay is armed");
    assert!(!has_not_delayed(&mut scene, TICKS_SECOND), "and it is spent");
    assert!(has_not_delayed(&mut scene, TICKS_SECOND), "then armed again");
}

#[test]
fn a_decision_says_where_and_why_in_one_value() {
    // The shape of `return X` in Java, with the refusal kept next to it so a caller can
    // log both (see `gather_action!`).
    let go = Decision::go(GatherState::FishermanStartFishing);
    assert_eq!(go.next, Some(GatherState::FishermanStartFishing));
    assert_eq!(go.refusal, None);

    assert_eq!(Decision::stay().next, None);

    let refused = Decision::refuse(GatherRefusal::NoWater);
    assert_eq!(refused.next, None);
    assert_eq!(refused.refusal, Some(GatherRefusal::NoWater));

    let after = Decision::go_after(GatherState::Base(WorkerState::Preparing), GatherRefusal::NoRod);
    assert_eq!(after.next, Some(GatherState::Base(WorkerState::Preparing)));
    assert_eq!(after.refusal, Some(GatherRefusal::NoRod));

    // `because` fills a reason only when there is none.
    assert_eq!(
        Decision::stay().because(GatherRefusal::NoWater).refusal,
        Some(GatherRefusal::NoWater)
    );
    assert_eq!(
        Decision::refuse(GatherRefusal::NoRod)
            .because(GatherRefusal::NoWater)
            .refusal,
        Some(GatherRefusal::NoRod),
        "the first reason wins"
    );
}

#[test]
fn every_state_of_every_trade_has_a_way_out() {
    // Java's trap state (`BasicStateMachine.java:220-228`) resets the AI to `INIT`. A
    // state no transition names is a bug here, not a game event — and the seam makes it
    // easy to leave one out, since the base states are registered elsewhere.
    for trade in GatherTrade::ALL {
        let ai = GatherAi::new(trade);
        for state in states_of(trade) {
            assert!(
                ai.machine().can_leave(state),
                "{} leaves {} with no transition",
                trade.class(),
                state.java()
            );
        }
    }
}

#[test]
fn a_scene_is_the_world_the_trade_facts_and_the_memory() {
    // `GatherScene` is what the macro splits, so its three parts have to be reachable
    // and the memory has to start empty.
    let scene = GatherScene::new(
        WorldFacts::new(),
        GatherJob::Farmer(FarmerFacts::default()),
    );
    assert!(scene.farmer().is_some(), "the farmer's facts are readable");
    assert!(scene.lumberjack().is_none(), "and the forester's are not");
    assert_eq!(scene.memory, super::facts::GatherMemory::default());
    assert_eq!(scene.refusal, None, "a new scene has no refusal");
}
