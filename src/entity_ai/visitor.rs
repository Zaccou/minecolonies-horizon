//! The tavern visitor: five states, a seat, and a stomach that empties.
//!
//! Java reference: `core/entity/ai/visitor/EntityAIVisitor.java`. A visitor is not a
//! colonist: it has no job, it never works, and it leaves when its saturation runs
//! out — **`reduceTime` takes two hundredths per check** (`:95`), so a visitor at
//! full saturation lasts 3000 checks at fifty ticks each, about two hours of game
//! time, and one sitting in the tavern burns that down at the same rate as one
//! standing in the street.
//!
//! **The seat is chosen on a coin, not on comfort.** The tavern branch fires on
//! `nextInt(5)` returning 0, or returning 1 **while it is night** (`:172`) — the
//! condition is `random == 0 || random == 1 && !colony.isDay()`, which reads as
//! `random == 0 || (random == 1 && night)`. A daytime visitor therefore sits one
//! time in five; a night-time one, two in five.
//!
//! **A visitor standing still never gets hungry.** `decide` (`:164-191`) always
//! answers a state — `IDLE` at worst — and a transition that produces a state stops
//! the tick (`BasicStateMachine.java:199-236`). `reduceTime` is registered **after**
//! it in the same state's list (`:77` then `:83`), so in `IDLE` it is never reached:
//! saturation only drops while **wandering** or **sitting**, where `wander` and `sit`
//! answer `null` and let the next rule run. The port keeps the dead rule and says so
//! rather than "fixing" it, because the fix would change how long a visitor stays.
//!
//! **`VisitorState.SLEEPING` is dead.** It is declared (`:39`) and never assigned,
//! so the five states are really four; the port keeps the variant and says so.

use crate::ai::facts::Pos;
use crate::ai::state::{Action, Machine, Transition, ENTITY_AI_TICKRATE};
use super::facts::{Tuned, VisitorFacts};
use super::goals::{Rule, VisitorState, VISITOR_TABLE};

/// `COMBAT_UPDATE_RATE` (`EntityAIVisitor.java:31`).
pub const COMBAT_UPDATE_RATE: Tuned = Tuned::new(20, "visitor/EntityAIVisitor.java:31");
/// The saturation a visitor loses per check (`:95`): `decreaseSaturation(0.02)`.
pub const SATURATION_PER_CHECK: f64 = 0.02;
/// The combat timeout (`:153`): `30 * 20` ticks.
pub const COMBAT_TIMEOUT: Tuned = Tuned::new(600, "visitor/EntityAIVisitor.java:153");
/// The sitting timer base (`:179`): `nextInt(2500) + 3000`.
pub const SIT_TICKS: Tuned = Tuned::new(3000, "visitor/EntityAIVisitor.java:179");
/// The sitting timer spread (`:179`).
pub const SIT_SPREAD: Tuned = Tuned::new(2500, "visitor/EntityAIVisitor.java:179");
/// The day wander timer (`:186`): `nextInt(1000) + 1000`.
pub const DAY_WANDER_TICKS: Tuned = Tuned::new(1000, "visitor/EntityAIVisitor.java:186");
/// The night wander timer (`:186`): a flat 300.
pub const NIGHT_WANDER_TICKS: Tuned = Tuned::new(300, "visitor/EntityAIVisitor.java:186");
/// The visitor's punch (`:127`): ten points of damage.
pub const VISITOR_DAMAGE: Tuned = Tuned::new(10, "visitor/EntityAIVisitor.java:127");
/// The wander speed (`:145`): `0.6`.
pub const VISITOR_SPEED: f64 = 0.6;
/// The wander range (`:145`): ten blocks.
pub const VISITOR_WANDER_RANGE: Tuned = Tuned::new(10, "visitor/EntityAIVisitor.java:145");

/// The visitor's own fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Memory {
    /// `actionTimeoutCounter` (`:58`).
    pub action_timeout: i32,
    /// `target` (`:63`).
    pub target: Option<Pos>,
    /// `tavern` (`:53`).
    pub tavern: Option<Pos>,
    /// `((VisitorData) citizenData).getSittingPosition()` (`:177`, `:211`).
    pub sitting_position: Option<Pos>,
}

/// The facts the visitor machine reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    /// The visitor and the tavern.
    pub facts: VisitorFacts,
    /// What it remembers.
    pub memory: Memory,
    /// The visitor left the colony: `removeCivilian` (`:99`).
    pub left: bool,
    /// `SittingEntity.sitDown(moveTo, citizen, actionTimeoutCounter)` (`:214`).
    pub sat_down: bool,
    /// `citizen.setLastHurtByMob(null)` and `setTarget(null)` (`:119-120`).
    pub calmed_down: bool,
}

/// `isEntityLoaded` (`:224-240`).
pub fn is_entity_loaded(scene: &mut Scene) -> Option<VisitorState> {
    if !scene.facts.loaded {
        return None;
    }
    if scene.facts.has_tavern {
        scene.memory.tavern = scene.facts.tavern;
    }
    Some(VisitorState::Idle)
}

/// `getTarget` (`:247-258`): the current target, or whoever hurt it last.
pub fn has_target(scene: &Scene) -> bool {
    scene.memory.target.is_some() || scene.facts.target.is_some()
}

/// `shouldFight` (`:149-157`), which also arms the thirty-second timer.
pub fn should_fight(scene: &mut Scene) -> bool {
    if let Some(target) = scene.facts.target {
        scene.memory.target = Some(target);
        scene.memory.action_timeout = COMBAT_TIMEOUT.v() as i32;
        return true;
    }
    false
}

/// `reduceTime` (`:93-107`).
///
/// Returns `true` when the visitor has had enough and must leave — which is also the
/// only way this AI ends, since nothing else removes the entity.
pub fn reduce_time(scene: &mut Scene) -> bool {
    scene.facts.saturation -= SATURATION_PER_CHECK;
    if scene.facts.saturation <= 0.0 {
        scene.left = true;
        return true;
    }
    false
}

/// `decide` (`:164-191`).
///
/// Five branches: fight, sit, wander (on `random == 2`), or stay idle. The sittable
/// branch is the `random == 0 || (random == 1 && night)` one.
pub fn decide(scene: &mut Scene) -> Option<VisitorState> {
    if should_fight(scene) {
        return Some(VisitorState::Combat);
    }
    let roll = scene.facts.roll.rem_euclid(5);
    let wants_a_seat = roll == 0 || (roll == 1 && !scene.facts.day);
    if wants_a_seat && scene.memory.tavern.is_some() && scene.facts.free_seat.is_some() {
        let seat = scene.facts.free_seat.unwrap_or(scene.facts.at);
        scene.memory.sitting_position = Some(seat);
        scene.memory.action_timeout =
            SIT_TICKS.v() as i32 + scene.facts.sit_roll.rem_euclid(SIT_SPREAD.v() as i32);
        return Some(VisitorState::Sitting);
    }
    if roll == 2 {
        scene.memory.action_timeout = if scene.facts.day {
            DAY_WANDER_TICKS.v() as i32
                + scene.facts.wander_roll.rem_euclid(DAY_WANDER_TICKS.v() as i32)
        } else {
            NIGHT_WANDER_TICKS.v() as i32
        };
        return Some(VisitorState::Wandering);
    }
    Some(VisitorState::Idle)
}

/// `wander` (`:138-147`).
pub fn wander(scene: &mut Scene) -> Option<VisitorState> {
    scene.memory.action_timeout -= 50;
    if scene.memory.action_timeout <= 0 {
        return Some(VisitorState::Idle);
    }
    None
}

/// `sit` (`:198-217`).
pub fn sit(scene: &mut Scene) -> Option<VisitorState> {
    scene.memory.action_timeout -= 50;
    if scene.memory.action_timeout <= 0 {
        scene.memory.sitting_position = None;
        return Some(VisitorState::Idle);
    }
    if scene.facts.nav_in_progress {
        return None;
    }
    if scene.memory.sitting_position.is_some() {
        scene.sat_down = true;
    }
    None
}

/// `doFight` (`:114-131`).
pub fn do_fight(scene: &mut Scene) -> Option<VisitorState> {
    scene.memory.action_timeout -= COMBAT_UPDATE_RATE.v() as i32;
    if scene.memory.target.is_none() || scene.memory.action_timeout <= 0 {
        scene.memory.target = None;
        scene.calmed_down = true;
        return Some(VisitorState::Idle);
    }
    if scene.facts.line_of_sight {
        scene.facts.target = scene.memory.target;
    }
    None
}

/// The visitor's machine: [`VISITOR_TABLE`], rule for rule.
pub fn machine() -> Machine<VisitorState, Scene> {
    let mut machine = Machine::new(VisitorState::Init, ENTITY_AI_TICKRATE);
    machine.add(Transition::target(
        VisitorState::Init,
        "is_entity_loaded",
        "visitor/EntityAIVisitor.java:76",
        50,
        Action::Run(|scene: &mut Scene, _: VisitorState| is_entity_loaded(scene)),
    ));
    machine.add(Transition::target(
        VisitorState::Idle,
        "decide",
        "visitor/EntityAIVisitor.java:77",
        50,
        Action::Run(|scene: &mut Scene, _: VisitorState| decide(scene)),
    ));
    machine.add(Transition::target(
        VisitorState::Wandering,
        "wander_done",
        "visitor/EntityAIVisitor.java:78",
        200,
        Action::Run(|scene: &mut Scene, _: VisitorState| wander(scene)),
    ));
    machine.add(Transition::target(
        VisitorState::Wandering,
        "wander_fight",
        "visitor/EntityAIVisitor.java:79",
        200,
        Action::Run(|scene: &mut Scene, _: VisitorState| {
            if should_fight(scene) {
                Some(VisitorState::Combat)
            } else {
                None
            }
        }),
    ));
    machine.add(Transition::target(
        VisitorState::Sitting,
        "sit_done",
        "visitor/EntityAIVisitor.java:80",
        50,
        Action::Run(|scene: &mut Scene, _: VisitorState| sit(scene)),
    ));
    machine.add(Transition::target(
        VisitorState::Sitting,
        "sit_fight",
        "visitor/EntityAIVisitor.java:81",
        50,
        Action::Run(|scene: &mut Scene, _: VisitorState| {
            if should_fight(scene) {
                Some(VisitorState::Combat)
            } else {
                None
            }
        }),
    ));
    machine.add(Transition::target(
        VisitorState::Combat,
        "do_fight",
        "visitor/EntityAIVisitor.java:82",
        COMBAT_UPDATE_RATE.v() as u32,
        Action::Run(|scene: &mut Scene, _: VisitorState| do_fight(scene)),
    ));
    machine.add(Transition::target(
        VisitorState::Idle,
        "reduce_time_idle",
        "visitor/EntityAIVisitor.java:83",
        50,
        Action::Run(|scene: &mut Scene, _: VisitorState| {
            reduce_time(scene);
            None
        }),
    ));
    machine.add(Transition::target(
        VisitorState::Wandering,
        "reduce_time_wander",
        "visitor/EntityAIVisitor.java:84",
        50,
        Action::Run(|scene: &mut Scene, _: VisitorState| {
            reduce_time(scene);
            None
        }),
    ));
    machine.add(Transition::target(
        VisitorState::Sitting,
        "reduce_time_sit",
        "visitor/EntityAIVisitor.java:85",
        50,
        Action::Run(|scene: &mut Scene, _: VisitorState| {
            reduce_time(scene);
            None
        }),
    ));
    machine
}

/// The table this module implements.
pub fn table() -> &'static [Rule<VisitorState>] {
    VISITOR_TABLE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visitor() -> Scene {
        Scene {
            // `tavern` est le champ prive du but (`:53`), rempli une seule fois par
            // `isEntityLoaded` (`:232-235`) : une scene de taverne deja chargee.
            memory: Memory {
                tavern: Some(Pos::new(0, 64, 0)),
                ..Memory::default()
            },
            facts: VisitorFacts {
                loaded: true,
                has_tavern: true,
                tavern: Some(Pos::new(0, 64, 0)),
                free_seat: Some(Pos::new(2, 64, 2)),
                saturation: 30.0,
                day: true,
                roll: 0,
                sit_roll: 0,
                wander_roll: 0,
                ..VisitorFacts::default()
            },
            ..Scene::default()
        }
    }

    #[test]
    fn an_unloaded_visitor_does_not_start() {
        // `:224-240`.
        let mut scene = visitor();
        scene.facts.loaded = false;
        assert_eq!(is_entity_loaded(&mut scene), None);
        scene.facts.loaded = true;
        assert_eq!(
            is_entity_loaded(&mut scene),
            Some(VisitorState::Idle)
        );
        assert_eq!(scene.memory.tavern, Some(Pos::new(0, 64, 0)));
    }

    #[test]
    fn the_idle_rule_is_decide_and_the_reduce_time_rule_behind_it_is_dead() {
        // `:77` then `:83`: `decide` always answers a state, so `reduceTime` never
        // runs from `IDLE`. This is the reason a visitor standing still keeps its
        // saturation.
        // La propriete qui compte est « jamais `null` », pas « toujours IDLE » :
        // `decide` peut aussi bien s'asseoir ou partir en balade (`:164-191`).
        let mut scene = visitor();
        assert!(decide(&mut scene).is_some());
        scene.facts.roll = 3;
        scene.facts.target = None;
        scene.facts.free_seat = None;
        assert_eq!(decide(&mut scene), Some(VisitorState::Idle));
        let before = scene.facts.saturation;
        let _ = reduce_time(&mut scene);
        assert!(scene.facts.saturation < before, "the rule itself works");
        // ... but the machine never gets there from IDLE: `decide` is registered
        // first in that state's list and `reduce_time_idle` second.
        let machine = machine();
        assert_eq!(machine.transitions()[1].rule, "decide");
        assert_eq!(machine.transitions()[7].rule, "reduce_time_idle");
        assert_eq!(machine.transitions()[7].tick_rate, 50);
    }

    #[test]
    fn a_visitor_sits_one_time_in_five_by_day_and_two_by_night() {
        // `:171-181` — `random == 0 || random == 1 && !isDay()`.
        let mut day = visitor();
        day.facts.roll = 1;
        assert_eq!(decide(&mut day), Some(VisitorState::Idle));
        let mut night = visitor();
        night.facts.day = false;
        night.facts.roll = 1;
        assert_eq!(decide(&mut night), Some(VisitorState::Sitting));
        let mut always = visitor();
        always.facts.roll = 0;
        assert_eq!(decide(&mut always), Some(VisitorState::Sitting));
    }

    #[test]
    fn without_a_tavern_or_a_free_seat_the_visitor_does_not_sit() {
        // `:172-181` needs both.
        let mut no_tavern = visitor();
        no_tavern.facts.has_tavern = false;
        no_tavern.memory.tavern = None;
        assert_eq!(decide(&mut no_tavern), Some(VisitorState::Idle));
        let mut full = visitor();
        full.facts.free_seat = None;
        assert_eq!(decide(&mut full), Some(VisitorState::Idle));
    }

    #[test]
    fn the_sitting_timer_is_three_to_five_thousand_ticks() {
        // `:179`.
        let mut scene = visitor();
        let _ = decide(&mut scene);
        assert!(scene.memory.action_timeout >= 3000);
        assert!(scene.memory.action_timeout < 5500);
        assert_eq!(scene.memory.sitting_position, Some(Pos::new(2, 64, 2)));
    }

    #[test]
    fn rolling_a_two_wanders_for_longer_by_day_than_by_night() {
        // `:183-188`.
        let mut day = visitor();
        day.facts.roll = 2;
        assert_eq!(decide(&mut day), Some(VisitorState::Wandering));
        assert!(day.memory.action_timeout >= 1000);
        let mut night = visitor();
        night.facts.roll = 2;
        night.facts.day = false;
        assert_eq!(decide(&mut night), Some(VisitorState::Wandering));
        assert_eq!(night.memory.action_timeout, 300);
    }

    #[test]
    fn a_visitor_with_a_target_fights_before_anything_else() {
        // `:166-169` comes before the roll.
        let mut scene = visitor();
        scene.facts.target = Some(Pos::new(5, 64, 5));
        assert_eq!(decide(&mut scene), Some(VisitorState::Combat));
        assert_eq!(scene.memory.action_timeout, 600);
    }

    #[test]
    fn the_fight_ends_when_the_target_goes_or_thirty_seconds_pass() {
        // `:114-122`.
        let mut scene = visitor();
        scene.memory.target = Some(Pos::new(5, 64, 5));
        scene.memory.action_timeout = 600;
        scene.facts.target = Some(Pos::new(5, 64, 5));
        assert_eq!(do_fight(&mut scene), None);
        scene.memory.action_timeout = 20;
        assert_eq!(do_fight(&mut scene), Some(VisitorState::Idle));
        assert!(scene.calmed_down);
        assert_eq!(scene.memory.target, None);
    }

    #[test]
    fn two_hundredths_of_saturation_are_lost_per_check() {
        // `:93-107`.
        let mut scene = visitor();
        assert!(!reduce_time(&mut scene));
        assert!((scene.facts.saturation - 29.98).abs() < 1e-9);
        scene.facts.saturation = 0.01;
        assert!(reduce_time(&mut scene));
        assert!(scene.left);
    }

    #[test]
    fn a_wander_ends_when_its_timer_runs_out() {
        // `:138-147`.
        let mut scene = visitor();
        scene.memory.action_timeout = 50;
        assert_eq!(wander(&mut scene), Some(VisitorState::Idle));
        scene.memory.action_timeout = 100;
        assert_eq!(wander(&mut scene), None);
        assert_eq!(scene.memory.action_timeout, 50);
    }

    #[test]
    fn a_sitting_visitor_stays_seated_until_its_timer_or_its_legs_move() {
        // `:198-217`.
        let mut scene = visitor();
        scene.memory.action_timeout = 100;
        scene.memory.sitting_position = Some(Pos::new(2, 64, 2));
        assert_eq!(sit(&mut scene), None);
        assert!(scene.sat_down);
        let mut walking = visitor();
        walking.facts.nav_in_progress = true;
        walking.memory.action_timeout = 100;
        walking.memory.sitting_position = Some(Pos::new(2, 64, 2));
        assert_eq!(sit(&mut walking), None);
        assert!(!walking.sat_down);
        let mut done = visitor();
        done.memory.action_timeout = 50;
        assert_eq!(sit(&mut done), Some(VisitorState::Idle));
        assert_eq!(done.memory.sitting_position, None);
    }

    #[test]
    fn the_visitor_machine_matches_its_table_rule_for_rule() {
        let machine = machine();
        let rules: Vec<&'static str> = machine
            .transitions()
            .iter()
            .map(|transition| transition.rule)
            .collect();
        let expected: Vec<&'static str> = VISITOR_TABLE.iter().map(|rule| rule.rule).collect();
        assert_eq!(rules, expected);
        assert_eq!(machine.state(), VisitorState::Init);
    }

    #[test]
    fn the_sleeping_state_is_declared_and_never_used() {
        // `:39` — nothing in the file assigns it.
        let machine = machine();
        assert!(!machine.can_leave(VisitorState::Sleeping));
        assert_eq!(VisitorState::Sleeping.java_name(), "SLEEPING");
    }

    #[test]
    fn the_visitor_constants_keep_their_lines() {
        assert_eq!(COMBAT_TIMEOUT.v(), 600);
        assert_eq!(SIT_TICKS.v(), 3000);
        assert_eq!(NIGHT_WANDER_TICKS.v(), 300);
        assert_eq!(VISITOR_DAMAGE.v(), 10);
        assert_eq!(SATURATION_PER_CHECK, 0.02);
        assert!(COMBAT_UPDATE_RATE.source.contains("EntityAIVisitor.java:31"));
    }
}
