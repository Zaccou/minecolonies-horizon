//! The four vanilla `Goal`s of `entity/ai/minimal/`: running away, looking at
//! somebody, opening a gate, and not drowning.
//!
//! Java references: `EntityAICitizenAvoidEntity.java` (the only one of the four
//! that is a state machine), `LookAtEntityGoal.java`, `EntityAIInteractToggleAble.java`
//! and `EntityAIFloat.java`.
//!
//! **Three of them are not state machines.** `LookAtEntityGoal` and
//! `EntityAIInteractToggleAble` extend vanilla `Goal` and are driven by
//! `canUse`/`canContinueToUse`/`tick`; `EntityAIFloat` extends `FloatGoal` and
//! overrides `tick` only. Porting them as "states" would be an invention, so they
//! are ported as the four questions vanilla asks: may I start, must I continue,
//! what do I do, when do I stop. Their triggers are in
//! [`super::goals::LOOK_RULES`], [`super::goals::TOGGLE_RULES`] and
//! [`super::goals::FLOAT_RULES`].
//!
//! **The gate opener is the one piece of "physics" in this module**, and it is a
//! rule about *when* to open: only on a collision, only every ten ticks for
//! collisions and every fifty for the safety net (`:124-143`), and only for a block
//! with an `OPEN` property that is not air (`:377-393`). A `TrapDoor` it opened is
//! closed by it and by nobody else (`:562-565`), which is why the memory keeps
//! *which kinds* it opened rather than *which positions*.

use super::facts::{
    CitizenFacts, FleeFacts, LookFacts, ToggleFacts, ToggleKind, TogglePos, Tuned, MAX_GUARD_CALL_RANGE,
};
use super::goals::{FleeState, GoalRule, Step, FLOAT_RULES, FLEE_TABLE, LOOK_RULES, TOGGLE_RULES};
use crate::ai::facts::Pos;

/// `TOO_CLOSE_TO_MOB` (`EntityAICitizenAvoidEntity.java:33`): the near speed below
/// four blocks.
pub const TOO_CLOSE_TO_MOB: Tuned = Tuned::new(4, "EntityAICitizenAvoidEntity.java:33");
/// `CHECKS_BEFORE_SAFE` (`:38`): `20 * 10 / 5` — ten seconds of five-tick checks.
pub const CHECKS_BEFORE_SAFE: Tuned = Tuned::new(40, "EntityAICitizenAvoidEntity.java:38");
/// `MIN_MOVE_AWAY_DIST` (`:43`): a nearly unhurt citizen jogs seven blocks.
pub const MIN_MOVE_AWAY_DIST: Tuned = Tuned::new(7, "EntityAICitizenAvoidEntity.java:43");
/// `MED_MOVE_AWAY_DIST` (`:44`): half health, fifteen blocks.
pub const MED_MOVE_AWAY_DIST: Tuned = Tuned::new(15, "EntityAICitizenAvoidEntity.java:44");
/// `MAX_MOVE_AWAY_DIST` (`:45`): badly hurt, twenty blocks.
pub const MAX_MOVE_AWAY_DIST: Tuned = Tuned::new(20, "EntityAICitizenAvoidEntity.java:45");
/// The health margin of the first branch (`:195`): `maxHealth - 4`.
pub const HEALTH_MARGIN: Tuned = Tuned::new(4, "EntityAICitizenAvoidEntity.java:195");
/// `DEFAULT_PROBABILITY` of `LookAtEntityGoal` (`:30`).
pub const LOOK_PROBABILITY: f64 = 0.02;
/// The stare duration (`LookAtEntityGoal.java:102`): `40 + nextInt(40)`.
pub const LOOK_TICKS: Tuned = Tuned::new(40, "LookAtEntityGoal.java:102");
/// The collision check back-off (`EntityAIInteractToggleAble.java:126`, `:134`).
pub const COLLISION_BACKOFF: Tuned = Tuned::new(10, "EntityAIInteractToggleAble.java:126");
/// The slow safety-net check (`:141`).
pub const SLOW_CHECK_INTERVAL: Tuned = Tuned::new(50, "EntityAIInteractToggleAble.java:141");
/// `MAX_DISTANCE` (`:54`): a block further than five blocks is dropped from the
/// path list.
pub const TOGGLE_MAX_DISTANCE: Tuned = Tuned::new(25, "EntityAIInteractToggleAble.java:54");
/// `MIN_DISTANCE` (`:49`). Declared, and read nowhere.
pub const TOGGLE_MIN_DISTANCE: Tuned = Tuned::new(16, "EntityAIInteractToggleAble.java:49");
/// `LENGTH_TO_CHECK` (`:44`) and `DEFAULT_HEIGHT_TO_CHECK` (`:39`), used by the
/// path scan the caller performs.
pub const TOGGLE_LOOKAHEAD: Tuned = Tuned::new(2, "EntityAIInteractToggleAble.java:44");
/// The extra spread `ColonyConstants.rand.nextInt(40 + offSet)` (`:405`).
pub const TOGGLE_UPDATE_SPREAD: Tuned = Tuned::new(40, "EntityAIInteractToggleAble.java:405");
/// `nav.setPauseTicks(20 * 15)` after launching a water escape (`EntityAIFloat.java:102`).
pub const WATER_ESCAPE_PAUSE: Tuned = Tuned::new(300, "EntityAIFloat.java:102");
/// The follow range multiplier of the escape search (`:99`): `FOLLOW_RANGE * 5`.
pub const WATER_ESCAPE_RANGE_FACTOR: Tuned = Tuned::new(5, "EntityAIFloat.java:99");

/// The flee AI's own fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Memory {
    /// `safeTime` (`:61`).
    pub safe_time: i32,
    /// `startingPos` (`:77`).
    pub starting_pos: Option<Pos>,
    /// `closestLivingEntity` (`:56`).
    pub closest: Option<Pos>,
    /// `moveAwayPath != null` (`:66`).
    pub has_path: bool,
}

/// The facts the flee goal reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    /// The citizen.
    pub citizen: CitizenFacts,
    /// What it runs from.
    pub flee: FleeFacts,
    /// Its memory.
    pub memory: Memory,
    /// `citizen.callForHelp(newClosest, MAX_GUARD_CALL_RANGE)` was called (`:221`).
    pub called_for_help: bool,
    /// `moveAwayPath.isInProgress()` — the path the last walk produced (`:227`).
    pub path_in_progress: bool,
    /// The speed modifier the last tick asked for (`:234-241`).
    pub speed: f64,
}

/// `getMoveAwayDist` (`:193-204`): the healthier, the shorter the run.
pub fn move_away_dist(health: f64, max_health: f64) -> f64 {
    if health >= max_health - HEALTH_MARGIN.v() as f64 {
        MIN_MOVE_AWAY_DIST.v() as f64
    } else if health >= max_health / 2.0 {
        MED_MOVE_AWAY_DIST.v() as f64
    } else {
        MAX_MOVE_AWAY_DIST.v() as f64
    }
}

/// `isEntityClose` (`:118-138`).
pub fn is_entity_close(scene: &mut Scene) -> Step<FleeState> {
    scene.memory.safe_time += 1;
    if scene.memory.safe_time > CHECKS_BEFORE_SAFE.v() as i32 {
        scene.memory = Memory::default();
        return Step::Next(FleeState::Idle);
    }
    match scene.flee.closest {
        Some(pos) => {
            scene.memory.closest = Some(pos);
            scene.memory.safe_time = 0;
            scene.memory.has_path = perform_move_away(scene);
            Step::Next(FleeState::Running)
        }
        None => Step::Next(FleeState::CheckEntities),
    }
}

/// `performMoveAway` (`:173-185`): a new path only when none is running.
pub fn perform_move_away(scene: &mut Scene) -> bool {
    if scene.path_in_progress || !scene.flee.nav_done {
        return false;
    }
    scene.path_in_progress = true;
    true
}

/// `updateMoving` (`:211-244`).
///
/// **A change of pursuer calls for help** (`:218-223`) — the only link between a
/// fleeing citizen and the guards, and the reason `MAX_GUARD_CALL_RANGE` is 90000:
/// it is a **squared** distance, nine hundred blocks of reach.
pub fn update_moving(scene: &mut Scene) -> Step<FleeState> {
    if let Some(pos) = scene.flee.closest {
        if scene.memory.closest != Some(pos) {
            scene.called_for_help = true;
            let _ = MAX_GUARD_CALL_RANGE;
            scene.memory.closest = Some(pos);
        }
        let _ = perform_move_away(scene);
    }
    if !scene.path_in_progress {
        scene.memory.safe_time = 0;
        return Step::Next(FleeState::CheckEntities);
    }
    scene.speed = if scene.flee.distance < TOO_CLOSE_TO_MOB.v() as f64 {
        // `nearSpeed` — the citizen sprints when the thing is on top of it.
        1.0
    } else {
        // `farSpeed` — it keeps a walking pace when it has room.
        0.6
    };
    Step::Next(FleeState::Running)
}

/// The rule of one flee state.
pub fn flee_step(state: FleeState, scene: &mut Scene) -> Step<FleeState> {
    match state {
        FleeState::CitizenFlee => {
            scene.memory = Memory::default();
            scene.memory.starting_pos = Some(scene.citizen.at);
            Step::Next(FleeState::CheckEntities)
        }
        FleeState::CheckEntities => is_entity_close(scene),
        FleeState::Running => update_moving(scene),
        FleeState::Idle => Step::Stayed,
    }
}

/// The flee table.
pub fn flee_table() -> &'static [super::goals::Rule<FleeState>] {
    FLEE_TABLE
}

// ---------------------------------------------------------------------------
// Looking at somebody — `LookAtEntityGoal`
// ---------------------------------------------------------------------------

/// `canUse` (`:48-80`).
///
/// The probability is checked **first** (`:51`), before anything is looked up, and
/// a sleeping guard never looks (`:73-76`) — the same `isAsleep()` test the guard's
/// own `skipSearch` uses to wake a comrade.
pub fn look_can_use(facts: &LookFacts) -> bool {
    if facts.probability_roll >= facts.probability {
        return false;
    }
    if facts.guard_asleep {
        return false;
    }
    facts.target.is_some() || facts.nearest.is_some()
}

/// `start` (`:99-103`): a fresh timer of forty to eighty ticks.
pub fn look_ticks(roll: i32) -> i32 {
    LOOK_TICKS.v() as i32 + roll.rem_euclid(LOOK_TICKS.v() as i32)
}

/// `canContinueToUse` (`:82-97`): alive, in range, and time left.
pub fn look_can_continue(facts: &LookFacts) -> bool {
    facts.alive
        && facts.distance_sqr <= (facts.look_distance * facts.look_distance) as f64
        && facts.look_time > 0
}

/// `tick` (`:111-120`): one tick off the timer, and the head turned.
pub fn look_tick(facts: &mut LookFacts) {
    if facts.alive {
        facts.look_time -= 1;
    }
}

/// The look table.
pub fn look_rules() -> &'static [GoalRule] {
    LOOK_RULES
}

// ---------------------------------------------------------------------------
// Opening a gate — `EntityAIInteractToggleAble`
// ---------------------------------------------------------------------------

/// The toggle goal's own fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToggleMemory {
    /// `updateTimer` (`:93`).
    pub update_timer: i32,
    /// `executeTimerSlow` (`:94`).
    pub slow_timer: i32,
    /// `myToggled` (`:103`): the kinds this goal opened itself.
    pub my_toggled: Vec<ToggleKind>,
}

impl ToggleMemory {
    /// `resetAll` (`:351-369`): close what this goal opened and clear everything.
    pub fn reset(&mut self) {
        self.my_toggled.clear();
        self.update_timer = 0;
    }
}

/// `canUse` (`:118-146`), the three timers of it.
///
/// Returns the branch that fired, or `None` when the goal may not start.
pub fn toggle_can_use(facts: &ToggleFacts, memory: &mut ToggleMemory) -> Option<&'static str> {
    // `updateTimer-- <= 0` (`:124`, `:132`) and `executeTimerSlow-- <= 0` (`:139`):
    // Java **post-decrements inside the condition**, so every call that reaches the
    // test costs one tick even when the test fails.
    if facts.riding_mount && facts.mount_collided {
        let due = memory.update_timer <= 0;
        memory.update_timer -= 1;
        if due {
            memory.update_timer = COLLISION_BACKOFF.v() as i32;
            return Some("can_use_mount_collision");
        }
    }
    if facts.collided || facts.climbing {
        let due = memory.update_timer <= 0;
        memory.update_timer -= 1;
        if due {
            memory.update_timer = COLLISION_BACKOFF.v() as i32;
            return Some("can_use_collision");
        }
    }
    let due = memory.slow_timer <= 0;
    memory.slow_timer -= 1;
    if due {
        memory.slow_timer = SLOW_CHECK_INTERVAL.v() as i32;
        return Some("can_use_slow");
    }
    None
}

/// `canContinueToUse` (`:342-346`).
pub fn toggle_can_continue(facts: &ToggleFacts) -> bool {
    !facts.positions.is_empty()
}

/// What one `tick` of the gate goal produces (`:398-462`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Toggled {
    /// The blocks still on the path, after the invalid and the out-of-range ones
    /// were dropped (`:412-444`).
    pub kept: Vec<TogglePos>,
    /// The block to open now, if any (`:446-461`).
    pub open: Option<Pos>,
    /// The blocks to close again: the ones this goal opened and then left behind
    /// (`:394-401`, `:427-441`).
    pub close: Vec<Pos>,
}

/// One `tick` (`:398-462`).
///
/// Two things happen, and both matter: every block that is no longer valid, or
/// further than [`TOGGLE_MAX_DISTANCE`] squared, is dropped from the list — **and
/// closed if this goal opened it**, unless its kind says it must only close its
/// own; then one of the remaining blocks is opened. A `FenceGate` is closed even
/// when this goal never opened it (`!onlyCloseYourOpens`, `:433`), a `TrapDoor`
/// only when it did.
pub fn toggle_tick(facts: &ToggleFacts, memory: &mut ToggleMemory) -> Toggled {
    // `if (--updateTimer >= 0) return;` then `updateTimer = rand.nextInt(40 + offSet)`
    // (`:401-405`): the tick does nothing at all until the timer is spent, and the
    // list it would have scanned is left untouched.
    memory.update_timer -= 1;
    if memory.update_timer >= 0 {
        return Toggled {
            kept: facts.positions.clone(),
            open: None,
            close: Vec::new(),
        };
    }
    memory.update_timer = facts.update_roll;

    let mut kept = Vec::new();
    let mut close = Vec::new();
    for position in &facts.positions {
        if !position.valid() {
            continue;
        }
        if position.distance_sqr > TOGGLE_MAX_DISTANCE.v() as f64 {
            if !position.kind.closes_only_its_own() || memory.my_toggled.contains(&position.kind) {
                close.push(position.pos);
                memory.my_toggled.retain(|kind| *kind != position.kind);
            }
            continue;
        }
        kept.push(*position);
    }

    if kept.is_empty() {
        return Toggled {
            kept,
            open: None,
            close,
        };
    }
    let index = facts.choice_roll.unsigned_abs() as usize % kept.len();
    let chosen = kept[index];
    if chosen.has_open && !chosen.open {
        if !memory.my_toggled.contains(&chosen.kind) {
            memory.my_toggled.push(chosen.kind);
        }
        return Toggled {
            kept,
            open: Some(chosen.pos),
            close,
        };
    }
    Toggled {
        kept,
        open: None,
        close,
    }
}

/// The toggle table.
pub fn toggle_rules() -> &'static [GoalRule] {
    TOGGLE_RULES
}

// ---------------------------------------------------------------------------
// Not drowning — `EntityAIFloat`
// ---------------------------------------------------------------------------

/// What `EntityAIFloat.tick` decides (`:81-117`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three outcomes of the tick
pub enum FloatOutcome {
    /// Do nothing at all: the eyes are under water but the block above is air, so
    /// vanilla's own float can handle it (`:85-88`).
    Leave,
    /// Launch a water-escape path and pause the navigation for fifteen seconds
    /// (`:89-104`).
    Escape,
    /// Out of the water: drop the water path and un-pause (`:106-116`).
    StopEscape,
}

/// `tick` (`:81-117`).
pub fn float_tick(
    eyes_in_swimmable_fluid: bool,
    block_above_is_air: bool,
    water_path_running: bool,
) -> FloatOutcome {
    if eyes_in_swimmable_fluid {
        if block_above_is_air {
            return FloatOutcome::Leave;
        }
        if !water_path_running {
            return FloatOutcome::Escape;
        }
        return FloatOutcome::Leave;
    }
    FloatOutcome::StopEscape
}

/// The float table.
pub fn float_rules() -> &'static [GoalRule] {
    FLOAT_RULES
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flee_scene() -> Scene {
        Scene {
            citizen: CitizenFacts::default(),
            flee: FleeFacts {
                health: 20.0,
                max_health: 20.0,
                closest: Some(Pos::new(3, 64, 3)),
                closest_id: 7,
                nav_done: true,
                distance: 6.0,
                ..FleeFacts::default()
            },
            ..Scene::default()
        }
    }

    #[test]
    fn entering_the_flee_state_remembers_where_the_citizen_came_from() {
        // `:105-108`.
        let mut scene = flee_scene();
        scene.citizen.at = Pos::new(1, 64, 2);
        assert_eq!(
            flee_step(FleeState::CitizenFlee, &mut scene),
            Step::Next(FleeState::CheckEntities)
        );
        assert_eq!(scene.memory.starting_pos, Some(Pos::new(1, 64, 2)));
    }

    #[test]
    fn the_run_length_follows_the_health_twenty_then_fifteen_then_seven() {
        // `:193-204`.
        assert_eq!(move_away_dist(20.0, 20.0), 7.0);
        assert_eq!(move_away_dist(16.0, 20.0), 7.0);
        assert_eq!(move_away_dist(15.9, 20.0), 15.0);
        assert_eq!(move_away_dist(10.0, 20.0), 15.0);
        assert_eq!(move_away_dist(9.9, 20.0), 20.0);
    }

    #[test]
    fn forty_safe_checks_end_the_flee_and_send_the_citizen_home() {
        // `:120-126`.
        let mut scene = flee_scene();
        scene.flee.closest = None;
        scene.memory.safe_time = 40;
        assert_eq!(
            flee_step(FleeState::CheckEntities, &mut scene),
            Step::Next(FleeState::Idle)
        );
        assert_eq!(scene.memory.safe_time, 0);
    }

    #[test]
    fn seeing_something_resets_the_safe_counter_and_runs() {
        // `:128-135`.
        let mut scene = flee_scene();
        scene.memory.safe_time = 12;
        assert_eq!(
            flee_step(FleeState::CheckEntities, &mut scene),
            Step::Next(FleeState::Running)
        );
        assert_eq!(scene.memory.safe_time, 0);
        assert_eq!(scene.memory.closest, Some(Pos::new(3, 64, 3)));
        assert!(scene.path_in_progress);
    }

    #[test]
    fn a_path_already_running_is_not_replaced() {
        // `:175-183`.
        let mut scene = flee_scene();
        scene.path_in_progress = true;
        assert!(!perform_move_away(&mut scene));
        let mut moving = flee_scene();
        moving.flee.nav_done = false;
        assert!(!perform_move_away(&mut moving));
        let mut free = flee_scene();
        assert!(perform_move_away(&mut free));
    }

    #[test]
    fn a_new_pursuer_calls_the_guards() {
        // `:215-225`.
        let mut scene = flee_scene();
        scene.path_in_progress = true;
        scene.memory.closest = Some(Pos::new(1, 64, 1));
        assert_eq!(
            flee_step(FleeState::Running, &mut scene),
            Step::Next(FleeState::Running)
        );
        assert!(scene.called_for_help);
        assert_eq!(scene.memory.closest, Some(Pos::new(3, 64, 3)));
    }

    #[test]
    fn the_same_pursuer_does_not_call_for_help_twice() {
        // `:218-223`: the id comparison, modelled as a position comparison.
        let mut scene = flee_scene();
        scene.path_in_progress = true;
        scene.memory.closest = Some(Pos::new(3, 64, 3));
        let _ = flee_step(FleeState::Running, &mut scene);
        assert!(!scene.called_for_help);
    }

    #[test]
    fn a_finished_path_sends_the_citizen_back_to_the_checks() {
        // `:227-231`.
        // `updateMoving` ne relance un chemin que s'il reste quelque chose a fuir
        // (`:215-225`) : sans poursuivant, un chemin fini ramene aux verifications.
        let mut scene = flee_scene();
        scene.flee.closest = None;
        scene.path_in_progress = false;
        scene.memory.safe_time = 9;
        assert_eq!(
            flee_step(FleeState::Running, &mut scene),
            Step::Next(FleeState::CheckEntities)
        );
        assert_eq!(scene.memory.safe_time, 0);
    }

    #[test]
    fn the_speed_goes_up_when_the_pursuer_is_on_top_of_the_citizen() {
        // `:234-241`.
        let mut close = flee_scene();
        close.path_in_progress = true;
        close.flee.distance = 3.9;
        let _ = flee_step(FleeState::Running, &mut close);
        assert_eq!(close.speed, 1.0);
        let mut far = flee_scene();
        far.path_in_progress = true;
        far.flee.distance = 4.0;
        let _ = flee_step(FleeState::Running, &mut far);
        assert_eq!(far.speed, 0.6);
    }

    #[test]
    fn looking_at_somebody_needs_the_probability_then_a_target_then_an_awake_guard() {
        // `:48-80`.
        let mut facts = LookFacts {
            probability_roll: 0.01,
            probability: 0.02,
            nearest: Some(Pos::new(2, 64, 2)),
            alive: true,
            look_distance: 8.0,
            look_time: 40,
            distance_sqr: 4.0,
            ..LookFacts::default()
        };
        assert!(look_can_use(&facts));
        facts.probability_roll = 0.03;
        assert!(!look_can_use(&facts), "the draw is checked first");
        facts.probability_roll = 0.0;
        facts.guard_asleep = true;
        assert!(!look_can_use(&facts), "a sleeping guard does not look");
        facts.guard_asleep = false;
        facts.nearest = None;
        assert!(!look_can_use(&facts));
        facts.target = Some(Pos::new(1, 64, 1));
        assert!(look_can_use(&facts));
    }

    #[test]
    fn a_look_lasts_between_forty_and_eighty_ticks() {
        // `:102`.
        assert_eq!(look_ticks(0), 40);
        assert_eq!(look_ticks(39), 79);
        assert_eq!(look_ticks(40), 40, "the draw wraps at the base value");
    }

    #[test]
    fn a_look_stops_when_the_entity_dies_leaves_or_the_timer_ends() {
        // `:82-97`.
        let mut facts = LookFacts {
            alive: true,
            look_distance: 8.0,
            look_time: 3,
            distance_sqr: 64.0,
            ..LookFacts::default()
        };
        assert!(look_can_continue(&facts));
        facts.distance_sqr = 65.0;
        assert!(!look_can_continue(&facts), "strictly beyond the range");
        facts.distance_sqr = 1.0;
        facts.look_time = 0;
        assert!(!look_can_continue(&facts));
        facts.look_time = 1;
        facts.alive = false;
        assert!(!look_can_continue(&facts));
        facts.alive = true;
        look_tick(&mut facts);
        assert_eq!(facts.look_time, 0);
    }

    #[test]
    fn the_gate_goal_starts_on_a_collision_once_every_ten_ticks() {
        // `:124-143`.
        let mut memory = ToggleMemory::default();
        let facts = ToggleFacts {
            collided: true,
            ..ToggleFacts::default()
        };
        assert_eq!(
            toggle_can_use(&facts, &mut memory),
            Some("can_use_collision")
        );
        assert_eq!(memory.update_timer, 10);
        // Ten ticks of quiet, and the safety net is not due either.
        memory.slow_timer = 30;
        assert_eq!(toggle_can_use(&facts, &mut memory), None);
        assert_eq!(memory.update_timer, 9, "the failed test still costs a tick");
    }

    #[test]
    fn the_mount_collision_is_checked_before_the_citizens_own() {
        // `:120-129` comes first.
        let mut memory = ToggleMemory::default();
        let facts = ToggleFacts {
            riding_mount: true,
            mount_collided: true,
            collided: true,
            ..ToggleFacts::default()
        };
        assert_eq!(
            toggle_can_use(&facts, &mut memory),
            Some("can_use_mount_collision")
        );
    }

    #[test]
    fn the_fifty_tick_safety_net_fires_when_nothing_collides() {
        // `:138-143`.
        let mut memory = ToggleMemory {
            update_timer: 30,
            ..ToggleMemory::default()
        };
        let facts = ToggleFacts::default();
        assert_eq!(toggle_can_use(&facts, &mut memory), Some("can_use_slow"));
        assert_eq!(memory.slow_timer, 50);
        memory.slow_timer = 20;
        assert_eq!(toggle_can_use(&facts, &mut memory), None);
        assert_eq!(memory.slow_timer, 19, "post-decrement again");
    }

    #[test]
    fn opening_a_gate_picks_one_valid_block_and_remembers_the_kind() {
        // `:446-461`.
        let gate = TogglePos {
            pos: Pos::new(1, 64, 1),
            kind: ToggleKind::Fence,
            has_open: true,
            is_air: false,
            open: false,
            distance_sqr: 4.0,
        };
        let facts = ToggleFacts {
            positions: vec![gate],
            choice_roll: 0,
            ..ToggleFacts::default()
        };
        let mut memory = ToggleMemory::default();
        let toggled = toggle_tick(&facts, &mut memory);
        assert_eq!(toggled.kept.len(), 1);
        assert_eq!(toggled.open, Some(Pos::new(1, 64, 1)));
        assert!(toggled.close.is_empty());
        assert_eq!(memory.my_toggled, vec![ToggleKind::Fence]);
    }

    #[test]
    fn an_already_open_gate_is_not_opened_again() {
        // `TrapToggle.canOpen` is `!OPEN` (`:556-559`).
        let gate = TogglePos {
            pos: Pos::new(1, 64, 1),
            kind: ToggleKind::TrapDoor,
            has_open: true,
            is_air: false,
            open: true,
            distance_sqr: 4.0,
        };
        let facts = ToggleFacts {
            positions: vec![gate],
            choice_roll: 0,
            ..ToggleFacts::default()
        };
        let toggled = toggle_tick(&facts, &mut ToggleMemory::default());
        assert_eq!(toggled.kept.len(), 1);
        assert_eq!(toggled.open, None);
    }

    #[test]
    fn air_is_dropped_from_the_list_and_never_opened() {
        // `:377-393`, `:420-425`.
        let air = TogglePos {
            pos: Pos::new(1, 64, 1),
            kind: ToggleKind::Fence,
            has_open: true,
            is_air: true,
            open: false,
            distance_sqr: 1.0,
        };
        let facts = ToggleFacts {
            positions: vec![air],
            choice_roll: 0,
            ..ToggleFacts::default()
        };
        let toggled = toggle_tick(&facts, &mut ToggleMemory::default());
        assert!(toggled.kept.is_empty());
        assert_eq!(toggled.open, None);
    }

    #[test]
    fn a_block_left_behind_is_dropped_and_closed_if_this_goal_opened_it() {
        // `:427-441` and `onlyCloseYourOpens` (`:562-565`).
        let far_door = TogglePos {
            pos: Pos::new(30, 64, 30),
            kind: ToggleKind::Door,
            has_open: true,
            is_air: false,
            open: true,
            distance_sqr: 1800.0,
        };
        let facts = ToggleFacts {
            positions: vec![far_door],
            choice_roll: 0,
            ..ToggleFacts::default()
        };
        let mut memory = ToggleMemory::default();
        memory.my_toggled.push(ToggleKind::Door);
        let toggled = toggle_tick(&facts, &mut memory);
        assert!(toggled.kept.is_empty());
        assert_eq!(toggled.open, None);
        assert_eq!(toggled.close, vec![Pos::new(30, 64, 30)]);
        assert!(
            memory.my_toggled.is_empty(),
            "the door it opened is no longer its own"
        );
    }

    #[test]
    fn a_fence_gate_left_behind_is_closed_even_though_it_was_never_logged() {
        // `!onlyCloseYourOpens()` makes the check pass for a fence (`:433`).
        let far_fence = TogglePos {
            pos: Pos::new(30, 64, 30),
            kind: ToggleKind::Fence,
            has_open: true,
            is_air: false,
            open: true,
            distance_sqr: 1800.0,
        };
        let facts = ToggleFacts {
            positions: vec![far_fence],
            choice_roll: 0,
            ..ToggleFacts::default()
        };
        let mut memory = ToggleMemory::default();
        let toggled = toggle_tick(&facts, &mut memory);
        assert!(toggled.kept.is_empty());
        assert_eq!(toggled.close, vec![Pos::new(30, 64, 30)]);
    }

    #[test]
    fn the_goal_continues_only_while_the_path_holds_a_toggleable_block() {
        // `:342-346`.
        let gate = TogglePos {
            pos: Pos::new(1, 64, 1),
            kind: ToggleKind::Fence,
            has_open: true,
            is_air: false,
            open: false,
            distance_sqr: 4.0,
        };
        let mut facts = ToggleFacts {
            positions: vec![gate],
            ..ToggleFacts::default()
        };
        assert!(toggle_can_continue(&facts));
        facts.positions.clear();
        assert!(!toggle_can_continue(&facts));
    }

    #[test]
    fn resetting_the_gate_goal_forgets_what_it_opened() {
        // `:351-369`.
        let mut memory = ToggleMemory {
            my_toggled: vec![ToggleKind::TrapDoor],
            update_timer: 40,
            slow_timer: 12,
        };
        memory.reset();
        assert!(memory.my_toggled.is_empty());
        assert_eq!(memory.update_timer, 0);
        assert_eq!(memory.slow_timer, 12, "resetAll does not touch the slow timer");
    }

    #[test]
    fn floating_does_nothing_when_the_block_above_the_eyes_is_air() {
        // `:83-88`.
        assert_eq!(float_tick(true, true, false), FloatOutcome::Leave);
    }

    #[test]
    fn underwater_with_a_ceiling_launches_one_escape_path() {
        // `:89-104`.
        assert_eq!(float_tick(true, false, false), FloatOutcome::Escape);
        assert_eq!(
            float_tick(true, false, true),
            FloatOutcome::Leave,
            "a running escape is not restarted"
        );
    }

    #[test]
    fn out_of_the_water_the_escape_is_dropped() {
        // `:106-116`.
        assert_eq!(float_tick(false, false, true), FloatOutcome::StopEscape);
        assert_eq!(float_tick(false, false, false), FloatOutcome::StopEscape);
    }

    #[test]
    fn the_avoid_constants_keep_their_lines() {
        assert_eq!(TOO_CLOSE_TO_MOB.v(), 4);
        assert_eq!(CHECKS_BEFORE_SAFE.v(), 40);
        assert_eq!(MAX_GUARD_CALL_RANGE.v(), 90000);
        assert_eq!(TOGGLE_MAX_DISTANCE.v(), 25);
        assert_eq!(COLLISION_BACKOFF.v(), 10);
        assert_eq!(WATER_ESCAPE_PAUSE.v(), 300);
        assert!(MAX_GUARD_CALL_RANGE.source.contains("CitizenConstants.java:133"));
    }
}
