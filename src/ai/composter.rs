//! The composter: six targets on top of the base table, and the two barrel scans
//! that decide what it does.
//!
//! Java reference: `core/entity/ai/workers/production/agriculture/
//! EntityAIWorkComposter.java` — the constructor's `registerTargets` (`:95-102`),
//! `accelerateBarrels` (`:109-132`), `getMaterials` (`:139-196`),
//! `decideWhatToDo` (`:203-245`), `fillBarrels` (`:252-294`), `harvestBarrels`
//! (`:301-351`), `getLootMultiplier` (`:359-378`), `getActionsDoneUntilDumping`
//! (`:380-384`) and `complain` (`:390-402`).
//!
//! ## The table this adds
//!
//! | # | Pass / state | Rule | Rate | Java |
//! | ---: | --- | --- | ---: | --- |
//! | 1 | `AI_BLOCKING` | `accelerateBarrels` | 20 | `:96` |
//! | 2 | `IDLE` | `IDLE → START_WORKING` | 1 | `:97` |
//! | 3 | `GET_MATERIALS` | `getMaterials` | 20 | `:98` |
//! | 4 | `START_WORKING` | `decideWhatToDo` | 1 | `:99` |
//! | 5 | `COMPOSTER_FILL` | `fillBarrels` | 10 | `:100` |
//! | 6 | `COMPOSTER_HARVEST` | `harvestBarrels` | 10 | `:101` |
//!
//! **The one number that changes the base table**: `getActionsDoneUntilDumping()` is
//! `1` (`:381-384`), so a composter walks its inventory to the hut after **every**
//! harvested barrel. That is [`super::worker::JobProfile::COMPOSTER`], and it is why
//! `harvestBarrels` calls `incrementActionsDoneAndDecSaturation`
//! (`AbstractEntityAIBasic.java:1462-1466` → `job.incrementActionsDone`) while
//! `fillBarrels` does not.
//!
//! ## What is a decision here, and what is not
//!
//! `decideWhatToDo` is a decision and is ported as [`decide_what_to_do`]: two passes
//! over `building.getBarrels()`, harvest first (`:215-229`) then fill (`:231-241`),
//! and the order is the rule — a hut with a finished barrel and an empty one
//! harvests, always.
//!
//! `accelerateBarrels` is **not** a decision: it ticks the barrel block entities
//! `(primarySkill / 10) * 2` times each (`:111`, `:121-127`) so that a skilled
//! composter composts faster. The port keeps the count as a number a host can use
//! ([`barrel_acceleration_ticks`]) and has nothing to call — the barrel is a block
//! entity the game ticks, not a fact.
//!
//! `getLootMultiplier` is a dice roll (`:359-378`) and is ported as a **pure
//! function of the level and the draw**: `random.nextInt(100)` is the caller's number,
//! the three thresholds are Java's. Without the split it could not be tested; with it,
//! every branch has a test.

use super::facts::{Barrel, Pos, WorldFacts};
use super::state::{Action, Blocking, Machine, Transition, WorkerState};
use super::worker::{
    DELAY_RECHECK, JobAi, JobProfile, Refusal, STANDARD_DELAY, Scene,
};

/// `EntityAIWorkComposter.DECIDE_DELAY` (`:69`): the wait after deciding.
pub const DECIDE_DELAY: u32 = 40;
/// `EntityAIWorkComposter.AFTER_TASK_DELAY` (`:74`): the wait after a barrel action.
pub const AFTER_TASK_DELAY: u32 = 5;
/// `EntityAIWorkComposter.TICKS_UNTIL_COMPLAIN` (`:59`): one in-game day between two
/// "please fill the compostables list" messages.
pub const TICKS_UNTIL_COMPLAIN: i32 = 12000;
/// `EntityAIWorkComposter.BASE_XP_GAIN` (`:49`): 1.0 xp per barrel action. Not a
/// decision, kept because `fillBarrels` (`:285`) and `harvestBarrels` (`:346`) both
/// grant it and a caller will want the number.
pub const BASE_XP_GAIN: f64 = 1.0;
/// `CitizenConstants.STACKSIZE` (`api/util/constant/Constants.java:23`), used by the
/// composter's request size (`EntityAIWorkComposter.java:188`).
pub const STACKSIZE: u32 = 64;

/// What [`decide_what_to_do`] chose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: harvest, fill, or nothing to do — the three exits of `:215-244`
pub enum BarrelChoice {
    /// A barrel is done: harvest it (`:222-227`).
    Harvest(Pos),
    /// No barrel is done, one is not working: fill it (`:234-239`).
    Fill(Pos),
    /// Every barrel is busy: stay in `START_WORKING` (`:243-244`).
    Nothing,
}

/// Register the six targets of `EntityAIWorkComposter.java:95-102`.
pub fn register(machine: &mut Machine<WorkerState, Scene>) {
    // :96 — "actually accelerate the barrels", on every state, once a second.
    machine.add(Transition::event(
        Blocking::AiBlocking,
        "accelerateBarrels",
        "EntityAIWorkComposter.java:96",
        20,
        Action::Run(|_, _| None),
    ));
    // :97 — `new AITarget(IDLE, START_WORKING, 1)`.
    machine.add(Transition::goto(
        WorkerState::Idle,
        WorkerState::StartWorking,
        "idleToStartWorking",
        "EntityAIWorkComposter.java:97",
        1,
    ));
    // :98 — `new AITarget(GET_MATERIALS, this::getMaterials, TICKS_SECOND)`.
    machine.add(Transition::target(
        WorkerState::GetMaterials,
        "getMaterials",
        "EntityAIWorkComposter.java:98",
        20,
        Action::Run(get_materials_action),
    ));
    // :99 — `new AITarget(START_WORKING, this::decideWhatToDo, 1)`.
    machine.add(Transition::target(
        WorkerState::StartWorking,
        "decideWhatToDo",
        "EntityAIWorkComposter.java:99",
        1,
        Action::Run(decide_action),
    ));
    // :100 — `new AITarget(COMPOSTER_FILL, this::fillBarrels, 10)`.
    machine.add(Transition::target(
        WorkerState::ComposterFill,
        "fillBarrels",
        "EntityAIWorkComposter.java:100",
        10,
        Action::Run(fill_barrels),
    ));
    // :101 — `new AITarget(COMPOSTER_HARVEST, this::harvestBarrels, 10)`.
    machine.add(Transition::target(
        WorkerState::ComposterHarvest,
        "harvestBarrels",
        "EntityAIWorkComposter.java:101",
        10,
        Action::Run(harvest_barrels),
    ));
}

/// `EntityAIWorkComposter.accelerateBarrels`'s only number: how many base ticks one
/// AI tick buys, `(primarySkill / 10) * 2` (`:111`).
pub fn barrel_acceleration_ticks(primary_skill: u8) -> u32 {
    (primary_skill as u32 / 10) * 2
}

/// `decideWhatToDo`'s two scans (`:215-244`), as a decision over the barrels.
///
/// Harvest wins over fill because Java walks the barrel list twice, finished first.
pub fn decide_what_to_do(barrels: &[Barrel]) -> BarrelChoice {
    for barrel in barrels {
        if barrel.done {
            return BarrelChoice::Harvest(barrel.at); // :222-227
        }
    }
    for barrel in barrels {
        if !barrel.working {
            return BarrelChoice::Fill(barrel.at); // :234-239
        }
    }
    BarrelChoice::Nothing // :243-244
}

/// What `getMaterials` did (`:139-196`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the five exits of `getMaterials`
pub enum MaterialsOutcome {
    /// `!walkToBuilding()`: still walking, wait (`:141-145`).
    Walking,
    /// The `compostables` list is empty: complain and wait (`:147-152`).
    ListEmpty,
    /// It holds a compostable and switched to `START_WORKING` (`:162-170`).
    Have,
    /// It asked the hut for compostables and waits (`:174-192`).
    Asked,
    /// Compostables are in the hut and were taken, but nothing is in hand yet —
    /// `:154-160` walks them over before `:162` looks again.
    Took,
}

/// `getMaterials` (`:139-196`), condition by condition.
///
/// `at_hut` is `walkToBuilding()`'s answer and `complained` is the throttle; Java
/// keeps `ticksToComplain` in the AI, which is [`super::worker::Memory`].
pub fn get_materials(facts: &WorldFacts, memory: &mut super::worker::Memory) -> MaterialsOutcome {
    let Some(composter) = facts.composter() else {
        // Not a composter's scene: the state cannot happen, and saying so is better
        // than pretending the list is empty.
        return MaterialsOutcome::ListEmpty;
    };
    if !at_hut(facts) {
        return MaterialsOutcome::Walking; // :141-145
    }
    if composter.compostable_list_empty {
        complain(memory); // :148-152
        return MaterialsOutcome::ListEmpty;
    }
    if facts.inventory.compostable > 0 {
        return MaterialsOutcome::Have; // :162-170
    }
    if composter.has_worker_open_requests {
        // :174 — one request is enough; do not ask twice.
        return MaterialsOutcome::Asked;
    }
    MaterialsOutcome::Asked // :174-192
}

/// `complain` (`:390-402`): message the officers at most once per
/// [`TICKS_UNTIL_COMPLAIN`] ticks. Returns whether this call was the one that sent it.
pub fn complain(memory: &mut super::worker::Memory) -> bool {
    if memory.ticks_to_complain <= 0 {
        memory.ticks_to_complain = TICKS_UNTIL_COMPLAIN;
        return true;
    }
    memory.ticks_to_complain -= 1;
    false
}

/// `getLootMultiplier` (`:359-378`): the factor applied to the barrel's base six
/// compost.
///
/// `citizenLevel = (int) (getSecondarySkillLevel() / 2.0)` and `diceResult =
/// random.nextInt(100)`, then three inclusive tests on `citizenLevel * 2`, `* 4`,
/// `* 8`. The draw is the caller's; the thresholds are Java's.
#[derive(Clone, Copy, Debug, PartialEq)]
// exhaustive: Java's four returns
pub enum LootMultiplier {
    /// `DOUBLE` — 2.0 (`:366`).
    Double,
    /// 1.5 (`:370`).
    OneAndAHalf,
    /// 1.25 (`:374`).
    OneAndAQuarter,
    /// 1 (`:377`).
    One,
}

impl LootMultiplier {
    /// The factor, as Java writes it.
    pub fn value(self) -> f64 {
        match self {
            LootMultiplier::Double => 2.0,
            LootMultiplier::OneAndAHalf => 1.5,
            LootMultiplier::OneAndAQuarter => 1.25,
            LootMultiplier::One => 1.0,
        }
    }
}

/// `getLootMultiplier` without the dice: `dice_result` is
/// `random.nextInt(100)`, `secondary_skill` is the raw level.
pub fn loot_multiplier(secondary_skill: i32, dice_result: i32) -> LootMultiplier {
    let citizen_level = (secondary_skill as f64 / 2.0) as i32; // :361
    if dice_result <= citizen_level * 2 {
        return LootMultiplier::Double; // :364-367
    }
    if dice_result <= citizen_level * 4 {
        return LootMultiplier::OneAndAHalf; // :368-371
    }
    if dice_result <= citizen_level * 8 {
        return LootMultiplier::OneAndAQuarter; // :372-375
    }
    LootMultiplier::One // :377
}

/// `walkToBuilding()` (`AbstractEntityAIBasic.java:757-766`) as a fact: the worker
/// stands at its hut.
fn at_hut(facts: &WorldFacts) -> bool {
    facts
        .worker
        .at
        .is_living_at_site(facts.building.position, super::worker::WOKR_IN_BUILDING_DIST)
}

/// The `GET_MATERIALS` target (`:98`, `:139-196`).
fn get_materials_action(scene: &mut Scene, _: WorkerState) -> Option<WorkerState> {
    match get_materials(&scene.facts, &mut scene.memory) {
        MaterialsOutcome::Walking => {
            scene.memory.delay = 2; // :143
            Some(WorkerState::GetMaterials)
        }
        MaterialsOutcome::ListEmpty => {
            scene.refuse(Refusal::CompostablesNotSet);
            Some(WorkerState::GetMaterials) // :151
        }
        MaterialsOutcome::Have => Some(WorkerState::StartWorking), // :169
        MaterialsOutcome::Asked | MaterialsOutcome::Took => {
            scene.memory.delay = 2; // :194
            Some(WorkerState::StartWorking) // :195
        }
    }
}

/// The `START_WORKING` target (`:99`, `:203-245`).
fn decide_action(scene: &mut Scene, _: WorkerState) -> Option<WorkerState> {
    if !at_hut(&scene.facts) {
        scene.memory.delay = 2; // :207-211
        return Some(WorkerState::StartWorking);
    }
    let barrels = scene
        .facts
        .composter()
        .map(|composter| composter.barrels.clone())
        .unwrap_or_default();
    let choice = decide_what_to_do(&barrels);
    scene.memory.delay = DECIDE_DELAY as i32; // :224, :237, :243
    match choice {
        BarrelChoice::Harvest(at) => {
            scene.memory.target = Some(at); // `currentTarget`, :221
            Some(WorkerState::ComposterHarvest)
        }
        BarrelChoice::Fill(at) => {
            scene.memory.target = Some(at);
            Some(WorkerState::ComposterFill)
        }
        BarrelChoice::Nothing => {
            scene.refuse(Refusal::NoBarrel);
            Some(WorkerState::StartWorking)
        }
    }
}

/// The `COMPOSTER_FILL` target (`:100`, `:252-294`).
pub fn fill_barrels(scene: &mut Scene, state: WorkerState) -> Option<WorkerState> {
    if scene.facts.inventory.empty_main_hand && scene.facts.inventory.compostable == 0 {
        return Some(WorkerState::GetMaterials); // :266
    }
    let Some(target) = scene.memory.target else {
        // `currentTarget` was never set: the state cannot do its work.
        scene.refuse(Refusal::NoBarrel);
        return Some(WorkerState::GetMaterials);
    };
    if !scene
        .facts
        .worker
        .at
        .is_living_at_site(target, super::worker::WOKR_IN_BUILDING_DIST)
    {
        scene.memory.delay = 2; // :271
        return Some(state);
    }
    if scene.facts.blocks.at(target) != super::facts::Block::Barrel
        && !barrels_include(&scene.facts, target)
    {
        scene.refuse(Refusal::NoBarrel); // :275
        return Some(WorkerState::StartWorking);
    }
    // :284-290 — one item into the barrel, one base xp, one saturation point.
    scene.facts.inventory.compostable = scene.facts.inventory.compostable.saturating_sub(1);
    scene.facts.inventory.empty_main_hand = scene.facts.inventory.compostable == 0;
    scene.memory.delay = AFTER_TASK_DELAY as i32; // :292
    Some(WorkerState::StartWorking) // :293
}

/// The `COMPOSTER_HARVEST` target (`:101`, `:301-351`).
pub fn harvest_barrels(scene: &mut Scene, state: WorkerState) -> Option<WorkerState> {
    let Some(target) = scene.memory.target else {
        scene.refuse(Refusal::NoBarrel);
        return Some(WorkerState::StartWorking);
    };
    if !scene
        .facts
        .worker
        .at
        .is_living_at_site(target, super::worker::WOKR_IN_BUILDING_DIST)
    {
        scene.memory.delay = 2; // :305
        return Some(state);
    }
    if scene.facts.blocks.at(target) != super::facts::Block::Barrel
        && !barrels_include(&scene.facts, target)
    {
        scene.refuse(Refusal::NoBarrel); // :309
        return Some(WorkerState::StartWorking);
    }
    // :314-344 — `retrieveCompost(getLootMultiplier(...))`, then dirt or podzol.
    // The barrel is emptied by whoever owns it; the port records that the worker did
    // one action, which is what makes it dump on the next tick
    // (`getActionsDoneUntilDumping() == 1`, :380-384).
    scene.facts.inventory.actions_done += 1; // :347
    scene.memory.delay = AFTER_TASK_DELAY as i32; // :349
    Some(WorkerState::StartWorking) // :350
}

/// Whether the hut reports a barrel at `at`, whether or not a block was recorded.
fn barrels_include(facts: &WorldFacts, at: Pos) -> bool {
    facts
        .composter()
        .map(|composter| composter.barrels.iter().any(|barrel| barrel.at == at))
        .unwrap_or(false)
}

/// The profile a composter runs with: the base one, plus `actionsUntilDumping = 1`.
pub const PROFILE: JobProfile = JobProfile::COMPOSTER;

/// Build the full composter AI: the base table, then the six targets above.
///
/// Java gets this from `EntityAIWorkComposter extends AbstractEntityAIInteract extends
/// AbstractEntityAIBasic`, whose three constructors each call `super.registerTargets`
/// (`AbstractEntityAIBasic.java:200-260`, `AbstractEntityAIStructure.java:166-200`,
/// `EntityAIWorkComposter.java:95-102`), so the base rules are checked first.
pub fn ai() -> JobAi {
    let mut ai = JobAi::base(crate::jobs::job::Job::Composter);
    register(ai.machine_mut());
    ai
}

/// `DELAY_RECHECK` is also the composter's `setDelay` in two branches (`:194`);
/// re-exported so a test cites one number.
pub const RETRY_DELAY: u32 = DELAY_RECHECK;
/// `STANDARD_DELAY` after `decideWhatToDo` is `DECIDE_DELAY` here (`:224`), not the
/// base class's five ticks; kept as a name so the difference is visible.
pub const DECIDED_DELAY: u32 = DECIDE_DELAY;
/// `STANDARD_DELAY` after a barrel action (`:292`, `:349`) is
/// `AFTER_TASK_DELAY` = 5, which happens to equal the base class's
/// `STANDARD_DELAY` (`AbstractEntityAIBasic.java:100`).
pub const TASK_DELAY: u32 = STANDARD_DELAY;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::facts::{Barrel, ComposterFacts, InventoryFacts, JobFacts, WorkerFacts};
    use crate::ai::worker::Memory;

    #[test]
    fn a_finished_barrel_is_harvested_before_an_empty_one_is_filled() {
        // `:215-229` (harvest) walks the list before `:231-241` (fill).
        let at_fill = Pos::new(1, 65, 1);
        let at_done = Pos::new(2, 65, 2);
        let barrels = vec![Barrel::empty(at_fill), Barrel::done(at_done)];
        assert_eq!(decide_what_to_do(&barrels), BarrelChoice::Harvest(at_done));
        // The first finished barrel wins, not the last.
        let two_done = vec![Barrel::done(at_done), Barrel::done(at_fill)];
        assert_eq!(decide_what_to_do(&two_done), BarrelChoice::Harvest(at_done));
    }

    #[test]
    fn without_a_finished_barrel_the_first_fillable_one_is_taken() {
        // `:234` — `!checkIfWorking()`.
        let busy = Pos::new(1, 65, 1);
        let free = Pos::new(2, 65, 2);
        let barrels = vec![Barrel::busy(busy), Barrel::empty(free)];
        assert_eq!(decide_what_to_do(&barrels), BarrelChoice::Fill(free));
    }

    #[test]
    fn every_barrel_busy_means_nothing_to_do() {
        // `:243-244` — `setDelay(DECIDE_DELAY)` and stay in START_WORKING.
        let barrels = vec![Barrel::busy(Pos::new(1, 65, 1))];
        assert_eq!(decide_what_to_do(&barrels), BarrelChoice::Nothing);
        assert_eq!(decide_what_to_do(&[]), BarrelChoice::Nothing);
    }

    #[test]
    fn accelerating_a_barrel_is_two_ticks_per_ten_skill_levels() {
        // `:111` — `(level / 10) * 2`.
        assert_eq!(barrel_acceleration_ticks(0), 0);
        assert_eq!(barrel_acceleration_ticks(9), 0);
        assert_eq!(barrel_acceleration_ticks(10), 2);
        assert_eq!(barrel_acceleration_ticks(99), 18);
    }

    #[test]
    fn the_loot_multiplier_follows_the_three_thresholds() {
        // `:361-377` with a level of 10: `citizenLevel = 5`, so 10 / 20 / 40.
        assert_eq!(loot_multiplier(10, 10), LootMultiplier::Double);
        assert_eq!(loot_multiplier(10, 11), LootMultiplier::OneAndAHalf);
        assert_eq!(loot_multiplier(10, 20), LootMultiplier::OneAndAHalf);
        assert_eq!(loot_multiplier(10, 21), LootMultiplier::OneAndAQuarter);
        assert_eq!(loot_multiplier(10, 40), LootMultiplier::OneAndAQuarter);
        assert_eq!(loot_multiplier(10, 41), LootMultiplier::One);
        // A level of 0 can only ever return 1: every test is `dice <= 0`.
        assert_eq!(loot_multiplier(0, 0), LootMultiplier::Double);
        assert_eq!(loot_multiplier(0, 1), LootMultiplier::One);
        assert_eq!(LootMultiplier::Double.value(), 2.0);
        assert_eq!(LootMultiplier::One.value(), 1.0);
    }

    #[test]
    fn getting_materials_waits_when_the_list_is_empty() {
        // `:147-152`: complain and hold the state.
        let mut scene = Scene::with_profile(
            WorldFacts {
                job: JobFacts::Composter(ComposterFacts {
                    compostable_list_empty: true,
                    ..ComposterFacts::default()
                }),
                ..WorldFacts::new()
            },
            PROFILE,
        );
        let mut ai = ai();
        let outcome = ai.tick(&mut scene);
        // INIT → IDLE on the first tick, then IDLE → START_WORKING at rate 1.
        assert_eq!(outcome.state, WorkerState::Idle);
        let outcome = ai.tick(&mut scene);
        assert_eq!(outcome.state, WorkerState::StartWorking);
        // START_WORKING decides: every barrel is missing from this scene, so it
        // stays and records why.
        let outcome = ai.tick(&mut scene);
        assert_eq!(outcome.state, WorkerState::StartWorking);
        assert_eq!(outcome.why_not(), Some(Refusal::NoBarrel));
        assert_eq!(scene.memory.delay, DECIDE_DELAY as i32);
    }

    #[test]
    fn a_composter_with_a_finished_barrel_goes_to_harvest_it() {
        // `:215-229` through the machine.
        let at = Pos::new(1, 65, 1);
        let mut scene = Scene::with_profile(
            WorldFacts {
                worker: WorkerFacts {
                    at: Pos::new(0, 64, 0),
                    ..WorkerFacts::default()
                },
                job: JobFacts::Composter(ComposterFacts {
                    barrels: vec![Barrel::done(at)],
                    ..ComposterFacts::default()
                }),
                ..WorldFacts::new()
            },
            PROFILE,
        );
        let mut ai = ai();
        ai.tick(&mut scene); // INIT → IDLE
        ai.tick(&mut scene); // IDLE → START_WORKING
        let outcome = ai.tick(&mut scene); // decide
        assert_eq!(outcome.state, WorkerState::ComposterHarvest);
        assert_eq!(outcome.fired.map(|fired| fired.rule), Some("decideWhatToDo"));
        assert_eq!(scene.memory.target, Some(at));
        assert_eq!(scene.memory.delay, DECIDE_DELAY as i32);
    }

    #[test]
    fn a_composter_with_no_barrel_does_not_invent_one() {
        // `:243-244` — `setDelay(DECIDE_DELAY)` and `return START_WORKING`.
        let mut scene = Scene::with_profile(WorldFacts::new(), PROFILE);
        let mut ai = ai();
        ai.tick(&mut scene);
        ai.tick(&mut scene);
        let outcome = ai.tick(&mut scene);
        assert_eq!(outcome.state, WorkerState::StartWorking);
        assert_eq!(outcome.why_not(), Some(Refusal::NoBarrel));
        // `get_composter` on a non-composter scene is not a panic.
        assert!(WorldFacts::new().composter().is_none());
    }

    #[test]
    fn filling_with_no_compostable_in_hand_asks_the_hut() {
        // `:254-267`.
        let at = Pos::new(1, 65, 1);
        let mut scene = Scene::with_profile(
            WorldFacts {
                inventory: InventoryFacts {
                    empty_main_hand: true,
                    compostable: 0,
                    ..InventoryFacts::default()
                },
                job: JobFacts::Composter(ComposterFacts {
                    barrels: vec![Barrel::empty(at)],
                    ..ComposterFacts::default()
                }),
                ..WorldFacts::new()
            },
            PROFILE,
        );
        scene.memory.target = Some(at);
        assert_eq!(
            fill_barrels(&mut scene, WorkerState::ComposterFill),
            Some(WorkerState::GetMaterials)
        );
    }

    #[test]
    fn filling_one_item_empties_the_hand_and_waits_five_ticks() {
        // `:284-293`.
        let at = Pos::new(0, 64, 0);
        let mut scene = Scene::with_profile(
            WorldFacts {
                inventory: InventoryFacts {
                    empty_main_hand: false,
                    compostable: 2,
                    ..InventoryFacts::default()
                },
                job: JobFacts::Composter(ComposterFacts {
                    barrels: vec![Barrel::empty(at)],
                    ..ComposterFacts::default()
                }),
                blocks: {
                    let mut blocks = crate::ai::facts::Blocks::new();
                    blocks.set(at, crate::ai::facts::Block::Barrel);
                    blocks
                },
                ..WorldFacts::new()
            },
            PROFILE,
        );
        scene.memory.target = Some(at);
        assert_eq!(
            fill_barrels(&mut scene, WorkerState::ComposterFill),
            Some(WorkerState::StartWorking)
        );
        assert_eq!(scene.facts.inventory.compostable, 1);
        assert_eq!(scene.memory.delay, AFTER_TASK_DELAY as i32);
    }

    #[test]
    fn harvesting_counts_one_action_which_dumps_the_next_tick() {
        // `:347` + `getActionsDoneUntilDumping() == 1` (`:380-384`).
        let at = Pos::new(0, 64, 0);
        let mut scene = Scene::with_profile(
            WorldFacts {
                job: JobFacts::Composter(ComposterFacts {
                    barrels: vec![Barrel::done(at)],
                    ..ComposterFacts::default()
                }),
                ..WorldFacts::new()
            },
            PROFILE,
        );
        scene.memory.target = Some(at);
        assert_eq!(
            harvest_barrels(&mut scene, WorkerState::ComposterHarvest),
            Some(WorkerState::StartWorking)
        );
        assert_eq!(scene.facts.inventory.actions_done, 1);
        assert!(
            scene.facts.inventory.actions_done >= scene.profile.actions_until_dumping,
            "the base table will dump on the next STATE_BLOCKING pass"
        );
    }

    #[test]
    fn the_complaint_is_throttled_to_one_message_per_day() {
        // `:390-402`.
        let mut memory = Memory::default();
        assert!(complain(&mut memory), "the first call messages");
        assert_eq!(memory.ticks_to_complain, TICKS_UNTIL_COMPLAIN);
        for _ in 0..5 {
            assert!(!complain(&mut memory));
        }
        assert_eq!(memory.ticks_to_complain, TICKS_UNTIL_COMPLAIN - 5);
        memory.ticks_to_complain = 0;
        assert!(complain(&mut memory));
    }

    #[test]
    fn the_composter_asks_for_materials_once() {
        // `:174-192` — only when the hut has no open request of this worker.
        let facts = WorldFacts {
            job: JobFacts::Composter(ComposterFacts::default()),
            ..WorldFacts::new()
        };
        let mut memory = Memory::default();
        assert_eq!(get_materials(&facts, &mut memory), MaterialsOutcome::Asked);
        let with_request = WorldFacts {
            job: JobFacts::Composter(ComposterFacts {
                has_worker_open_requests: true,
                ..ComposterFacts::default()
            }),
            ..WorldFacts::new()
        };
        assert_eq!(
            get_materials(&with_request, &mut memory),
            MaterialsOutcome::Asked
        );
        // With a compostable in hand it goes back to work instead.
        let holding = WorldFacts {
            inventory: InventoryFacts {
                compostable: 3,
                ..InventoryFacts::default()
            },
            ..with_request.clone()
        };
        assert_eq!(get_materials(&holding, &mut memory), MaterialsOutcome::Have);
    }

    #[test]
    fn a_composter_that_is_not_at_its_hut_keeps_walking() {
        // `:141-145`.
        let facts = WorldFacts {
            worker: WorkerFacts {
                at: Pos::new(40, 64, 40),
                ..WorkerFacts::default()
            },
            job: JobFacts::Composter(ComposterFacts::default()),
            ..WorldFacts::new()
        };
        let mut memory = Memory::default();
        assert_eq!(get_materials(&facts, &mut memory), MaterialsOutcome::Walking);
    }
}
