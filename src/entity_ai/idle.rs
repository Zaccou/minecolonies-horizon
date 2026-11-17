//! Idle behaviour: the wanderer at its leisure site, and the child at play.
//!
//! Java references: `core/entity/ai/minimal/EntityAICitizenWander.java` and
//! `core/entity/ai/minimal/EntityAICitizenChild.java`.
//!
//! **The wanderer is a coupon collector.** Five times in a hundred it goes to a
//! leisure site (`:266-287`); the rest of the time it walks ten blocks at random
//! (`:289`). Once at the site, ninety-five in a hundred checks just keep walking
//! around it (`:152`), so a citizen in a library spends most of its visit on its
//! feet and only sometimes sits down or opens a book.
//!
//! **The child is a clock, not a goal.** `tryGrowUp` is an `AIEventTarget` in the
//! `EVENT` pass (`:115`), so it runs whatever the child is doing — including while
//! it follows somebody. It needs `MIN_ACTIVE_TIME` **ticks of activity**, and the
//! colony may lend it time (`useAdditionalChildTime`, `:273`). Growth is then a
//! 1-in-(70/modifier) draw *or* a hard cut-off at `70000 / modifier` (`:284`), so a
//! very unlucky child still grows up.

use super::facts::{
    ChildFacts, CitizenFacts, SiteKind, Tuned, WanderFacts, DEFAULT_SPEED,
};
use super::goals::{ChildState, Rule, Step, WanderState, CHILD_TABLE, WANDER_TABLE};
use crate::ai::facts::Pos;
use crate::jobs::kinds::service::ONE_IN_X_CHANCE;

/// `LEISURE_CHANCE` (`EntityAICitizenWander.java:38`): five in a hundred.
pub const LEISURE_CHANCE: Tuned = Tuned::new(5, "EntityAICitizenWander.java:38");
/// The delay `setCurrentDelay(60 * 20)` after choosing a leisure site (`:284`).
pub const LEISURE_DELAY: Tuned = Tuned::new(1200, "EntityAICitizenWander.java:284");
/// `nextInt(60 * 5) < 1` ends the visit (`:152`).
pub const LEISURE_VISIT_DENOMINATOR: Tuned = Tuned::new(300, "EntityAICitizenWander.java:152");
/// `nextInt(10) <= 0` decides whether to pick a spot at all (`:167`).
pub const SPOT_DENOMINATOR: Tuned = Tuned::new(10, "EntityAICitizenWander.java:167");
/// `nextInt(100) < 5` ends the reading (`:103`).
pub const BOOK_DENOMINATOR: Tuned = Tuned::new(100, "EntityAICitizenWander.java:103");
/// The walk range of the plain wander (`:289`): ten blocks.
pub const WANDER_RANGE: Tuned = Tuned::new(10, "EntityAICitizenWander.java:289");
/// `GROW_UP_NOTIFY_LIMIT` (`EntityAICitizenChild.java:37`).
pub const GROW_UP_NOTIFY_LIMIT: Tuned = Tuned::new(10, "EntityAICitizenChild.java:37");
/// `START_FOLLOW_DISTANCE` (`:67`): ten blocks.
pub const START_FOLLOW_DISTANCE: Tuned = Tuned::new(10, "EntityAICitizenChild.java:67");
/// `MIN_ACTIVE_TIME` (`:77`): four thousand ticks of activity before growing.
pub const MIN_ACTIVE_TIME: Tuned = Tuned::new(4000, "EntityAICitizenChild.java:77");
/// `BONUS_TIME_COLONY` (`:82`): what a colony lends once (`:273`).
pub const BONUS_TIME_COLONY: Tuned = Tuned::new(2000, "EntityAICitizenChild.java:82");
/// The hard growth cut-off (`:284`): `aiActiveTime > 70000 / growthModifier`.
pub const GROW_UP_CUTOFF: Tuned = Tuned::new(70000, "EntityAICitizenChild.java:284");
/// `rand.nextInt((int) (70 / growthModifier) + 1) == 0` (`:284`).
pub const GROW_UP_BASE: Tuned = Tuned::new(70, "EntityAICitizenChild.java:284");
/// The visit timer, `3 * 60 * 20` (`:223`).
pub const VISIT_TICKS: Tuned = Tuned::new(3600, "EntityAICitizenChild.java:223");
/// `setDelayForNextAction` (`:148`): `rand.nextInt(2*60*20) + 3*60*20`.
pub const DELAY_MIN: Tuned = Tuned::new(3600, "EntityAICitizenChild.java:148");
/// The draw on top of [`DELAY_MIN`] (`:148`).
pub const DELAY_RANGE: Tuned = Tuned::new(2400, "EntityAICitizenChild.java:148");
/// The follow time, `rand.nextInt(30 * 20) + 30 * 20` (`:181`).
pub const FOLLOW_MIN: Tuned = Tuned::new(600, "EntityAICitizenChild.java:181");
/// The draw on top of [`FOLLOW_MIN`] (`:181`).
pub const FOLLOW_RANGE: Tuned = Tuned::new(600, "EntityAICitizenChild.java:181");

/// The wanderer's own fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WanderMemory {
    /// `walkTo` (`:63`).
    pub walk_to: Option<Pos>,
    /// `leisureSite` (`:68`).
    pub leisure_site: Option<Pos>,
    /// The delay `setCurrentDelay` pushed (`:284`).
    pub delay: i32,
}

/// The facts the wanderer reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WanderScene {
    /// The citizen.
    pub citizen: CitizenFacts,
    /// The leisure site, its seats, and the draws.
    pub wander: WanderFacts,
    /// The AI's memory.
    pub memory: WanderMemory,
    /// `SittingEntity.sitDown(walkTo, citizen, TICKS_SECOND * 30)` ran
    /// (`:245-248`).
    pub sat_down: bool,
    /// `citizen.getCitizenSkillHandler().tryLevelUpIntelligence` was called
    /// (`:108-110`).
    pub studied: bool,
    /// The item in the main hand, `None` for `ItemStack.EMPTY` (`:105`, `:114`).
    pub held: Option<&'static str>,
}

/// `canUse` (`:293-297`): not moving, not a baby, not a guard.
pub fn can_use(scene: &WanderScene) -> bool {
    scene.wander.nav_done && !scene.wander.baby && !scene.wander.guard_job
}

/// `decide` (`:259-291`).
pub fn decide(scene: &mut WanderScene) -> Step<WanderState> {
    if !can_use(scene) {
        return Step::Next(WanderState::CitizenIdle);
    }
    if scene.wander.roll < LEISURE_CHANCE.v() as i32 {
        if let Some(site) = scene.wander.leisure_site {
            scene.memory.leisure_site = Some(site);
            scene.memory.delay = LEISURE_DELAY.v() as i32;
            return Step::Next(WanderState::GoToLeisureSite);
        }
    }
    // `EntityNavigationUtils.walkToRandomPos(citizen, 10, speed)` (`:289`).
    let _ = (WANDER_RANGE, DEFAULT_SPEED);
    Step::Next(WanderState::CitizenIdle)
}

/// `goToLeisureSite` (`:134-148`).
pub fn go_to_leisure_site(scene: &mut WanderScene) -> Step<WanderState> {
    if scene.memory.leisure_site.is_none() {
        return Step::Next(WanderState::CitizenIdle);
    }
    if !scene.wander.arrived {
        return Step::Next(WanderState::GoToLeisureSite);
    }
    Step::Next(WanderState::WanderAtLeisureSite)
}

/// `wanderAtLeisureSite` (`:150-257`).
///
/// The branch order is the reason a citizen reads: `READ_A_BOOK` is only reached
/// when the site is a library or a university **and** a spot was chosen — a plain
/// hut never produces it, however long the visit.
pub fn wander_at_leisure_site(scene: &mut WanderScene) -> Step<WanderState> {
    // `:152-157` — `leisureSite == null || nextInt(60 * 5) < 1` : the visit ends and
    // **both** fields are cleared.
    if scene.memory.leisure_site.is_none() || scene.wander.wander_roll < 1 {
        scene.memory.leisure_site = None;
        scene.memory.walk_to = None;
        return Step::Next(WanderState::CitizenIdle);
    }
    // `:159-162` — `walkTo != null && !walkToPos(...)` : still on the way.
    if scene.memory.walk_to.is_some() && !scene.wander.arrived {
        return Step::Next(WanderState::WanderAtLeisureSite);
    }

    // `:164-165`, `:256` — no blueprint data at the site: nothing to do there.
    if scene.wander.site_kind == SiteKind::None {
        return Step::Next(WanderState::CitizenIdle);
    }

    // `:167-171` — **first** branch: one look in ten walks at random inside the
    // hut's corners and goes no further. This is an `if`, not an `else`.
    if scene.memory.walk_to.is_none()
        && scene
            .wander
            .spot_roll
            .rem_euclid(SPOT_DENOMINATOR.v() as i32)
            <= 0
    {
        return Step::Next(WanderState::WanderAtLeisureSite);
    }

    // `:172-178` — the `else if`: a library or a university, five looks in a
    // hundred. Reading is therefore the *rare* branch behind the random walk.
    if scene.memory.walk_to.is_none()
        && matches!(
            scene.wander.site_kind,
            SiteKind::Library | SiteKind::University
        )
        && scene.wander.book_roll < (BOOK_DENOMINATOR.v() / 20) as i32
    {
        return Step::Next(WanderState::ReadABook);
    }

    // `:181-242` — pick a seat, else a standing spot, else nothing at all.
    if scene.memory.walk_to.is_none() {
        scene.memory.walk_to = pick_spot(scene);
        return Step::Next(WanderState::WanderAtLeisureSite);
    }

    // `:243-251` — arrived: sit if the spot is a seat, then forget it and start
    // over on the next look.
    if scene.wander.arrived {
        scene.sat_down = scene
            .memory
            .walk_to
            .map(|pos| {
                scene
                    .wander
                    .seats
                    .iter()
                    .any(|seat| seat.pos == pos && seat.sitting)
            })
            .unwrap_or(false);
        scene.memory.walk_to = None;
    }
    Step::Next(WanderState::WanderAtLeisureSite)
}

/// The seat the wanderer should try, if any: a free seat first, a standing spot
/// second, and nothing when the site has neither (`:191-239`).
pub fn pick_spot(scene: &WanderScene) -> Option<Pos> {
    // `citizen.getRandom().nextInt(size)` (`:195`, `:207`, `:214`, `:226`, `:232`):
    // its own draw, distinct from the 1-in-10 of `:167`.
    let index = scene.wander.choice_roll.unsigned_abs() as usize;
    let seats = scene.wander.candidate_seats();
    if !seats.is_empty() {
        return seats.get(index % seats.len()).copied();
    }
    let standing = scene.wander.candidate_standing();
    if standing.is_empty() {
        return None;
    }
    standing.get(index % standing.len()).copied()
}

/// `readABook` (`:88-131`).
pub fn read_a_book(scene: &mut WanderScene) -> Step<WanderState> {
    if scene.memory.leisure_site.is_none() {
        scene.memory.walk_to = None;
        return Step::Next(WanderState::CitizenIdle);
    }
    if scene.memory.walk_to.is_none() {
        // `:118-131` — `library.getRandomBookShelf()` / `university.getRandomBookShelf()`.
        scene.memory.walk_to = scene
            .wander
            .book_shelf
            .or(scene.wander.leisure_site)
            .or(Some(scene.citizen.at));
        return Step::Next(WanderState::ReadABook);
    }
    if !scene.wander.arrived {
        return Step::Next(WanderState::ReadABook);
    }
    if scene.wander.book_roll < (BOOK_DENOMINATOR.v() / 20) as i32 {
        scene.held = None;
        scene.memory.walk_to = None;
        scene.memory.leisure_site = None;
        scene.studied = true;
        let _ = ONE_IN_X_CHANCE;
        return Step::Next(WanderState::CitizenIdle);
    }
    scene.held = Some("minecraft:book");
    Step::Next(WanderState::ReadABook)
}

/// The rule of one wanderer state.
pub fn wander_step(state: WanderState, scene: &mut WanderScene) -> Step<WanderState> {
    match state {
        WanderState::CitizenIdle => decide(scene),
        WanderState::GoToLeisureSite => go_to_leisure_site(scene),
        WanderState::WanderAtLeisureSite => wander_at_leisure_site(scene),
        WanderState::ReadABook => read_a_book(scene),
    }
}

/// The wanderer's table.
pub fn wander_table() -> &'static [Rule<WanderState>] {
    WANDER_TABLE
}

// ---------------------------------------------------------------------------
// The child
// ---------------------------------------------------------------------------

/// The child's own fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChildMemory {
    /// `actionTimer` (`EntityAICitizenChild.java:62`).
    pub action_timer: i32,
    /// `aiActiveTime` (`:72`).
    pub active_time: i32,
    /// `followTarget` (`:87`).
    pub follow_target: Option<Pos>,
    /// `followStart` (`:92`).
    pub follow_start: Option<Pos>,
    /// `visitingHut` (`:97`).
    pub visiting_hut: Option<Pos>,
}

impl ChildMemory {
    /// `setDelayForNextAction` (`:145-149`): three to five minutes.
    pub fn set_delay(&mut self, roll: i32) {
        let span = DELAY_RANGE.v() as i32;
        self.action_timer = DELAY_MIN.v() as i32 + roll.rem_euclid(span.max(1));
    }
}

/// The facts the child reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChildScene {
    /// The child.
    pub citizen: CitizenFacts,
    /// Its game.
    pub child: ChildFacts,
    /// The colony's `GROWTH` research strength, which decides the growth chance
    /// (`:281`).
    pub growth: f64,
    /// Its memory.
    pub memory: ChildMemory,
    /// `child.getCitizenColonyHandler().getColonyOrRegister().getServerBuildingManager()
    /// .getBuildings()` was read and a hut chosen (`:226-231`).
    pub building_chosen: bool,
    /// `child.setIsChild(false)` and `citizenData.setIsChild(false)` (`:295-297`).
    pub grew_up: bool,
    /// The colony was told the child grew up (`:290-293`).
    pub notified: bool,
}

/// `isReadyForActivity` (`:127-140`).
///
/// `actionTimer -= 100` on every look, a 1-in-10 draw, and `canUse()`: three
/// conditions, and the timer is decremented **before** they are read.
pub fn is_ready_for_activity(scene: &mut ChildScene) -> bool {
    if scene.memory.action_timer > 0 {
        scene.memory.action_timer -= 100;
    }
    let can_use = scene.citizen.baby && scene.memory.action_timer <= 0;
    if can_use && scene.child.ready_roll.rem_euclid(10) == 0 {
        scene.memory.set_delay(scene.child.delay_roll);
        return true;
    }
    false
}

/// `tryGrowUp` (`:264-303`).
///
/// Returns the state the EVENT pass re-asserts — Java returns
/// `citizen.getCitizenAI().getState()`, so the child stays where it is.
pub fn try_grow_up(scene: &mut ChildScene) -> bool {
    if !scene.citizen.baby {
        return false;
    }
    if scene.child.colony_granted_time {
        scene.memory.active_time += BONUS_TIME_COLONY.v() as i32;
    }
    if (scene.memory.active_time as i64) < MIN_ACTIVE_TIME.v() {
        return false;
    }
    let modifier = 1.0 + scene.child_growth();
    let denominator = ((GROW_UP_BASE.v() as f64 / modifier) as i32) + 1;
    let lucky = scene.child.grow_roll.rem_euclid(denominator.max(1)) == 0;
    let old = (scene.memory.active_time as f64) > (GROW_UP_CUTOFF.v() as f64 / modifier);
    if lucky || old {
        scene.grew_up = true;
        if scene.child.colony_citizens <= GROW_UP_NOTIFY_LIMIT.v() as usize {
            scene.notified = true;
        }
        return true;
    }
    false
}

impl ChildScene {
    /// The `GROWTH` research strength, which the scene carries on the citizen's
    /// behalf.
    pub fn child_growth(&self) -> f64 {
        self.growth
    }
}

/// `searchEntityToFollow` (`:156-187`).
pub fn search_entity_to_follow(scene: &mut ChildScene) -> Step<ChildState> {
    if !is_ready_for_activity(scene) {
        return Step::Stayed;
    }
    match scene.child.follow_candidate {
        Some(target) => {
            scene.memory.follow_target = Some(target);
            scene.memory.follow_start = Some(scene.citizen.at);
            let span = FOLLOW_RANGE.v() as i32;
            scene.memory.action_timer =
                FOLLOW_MIN.v() as i32 + scene.child.follow_roll.rem_euclid(span.max(1));
            Step::Next(ChildState::Following)
        }
        None => Step::Stayed,
    }
}

/// `isReadyForActivity`'s first use: go and visit a hut (`:114`, `:215-257`).
pub fn visit(scene: &mut ChildScene) -> Step<ChildState> {
    if !is_ready_for_activity(scene) {
        return Step::Stayed;
    }
    if scene.child.buildings.is_empty() {
        return Step::Stayed;
    }
    let index = scene.child.ready_roll.unsigned_abs() as usize % scene.child.buildings.len();
    scene.memory.visiting_hut = scene.child.buildings.get(index).copied();
    scene.memory.action_timer = VISIT_TICKS.v() as i32;
    scene.building_chosen = true;
    Step::Next(ChildState::Visiting)
}

/// `followingEntity` (`:194-208`).
pub fn following_entity(scene: &mut ChildScene) -> Step<ChildState> {
    scene.memory.action_timer -= 20;
    if scene.memory.action_timer <= 0 || scene.memory.follow_target.is_none() {
        // `walkToPos(child, followStart, 2, true)` (`:200`).
        scene.memory.follow_target = None;
        scene.memory.set_delay(scene.child.delay_roll);
        return Step::Next(ChildState::Bored);
    }
    Step::Next(ChildState::Following)
}

/// `visitHuts` (`:215-257`).
pub fn visit_huts(scene: &mut ChildScene) -> Step<ChildState> {
    if scene.memory.action_timer <= 0 {
        scene.memory.action_timer = VISIT_TICKS.v() as i32;
    }
    scene.memory.action_timer -= 120;
    if scene.memory.action_timer > 0 {
        return Step::Next(ChildState::Visiting);
    }
    scene.memory.visiting_hut = None;
    scene.memory.set_delay(scene.child.delay_roll);
    Step::Next(ChildState::Bored)
}

/// The rule of one child state.
pub fn child_step(state: ChildState, scene: &mut ChildScene) -> Step<ChildState> {
    match state {
        // `BORED` is the idle state of this class: `IDLE` in Java, `BORED` in the
        // enum (`:44`).
        ChildState::Bored => Step::Stayed,
        ChildState::Playing => Step::Stayed,
        ChildState::Visiting => visit_huts(scene),
        ChildState::Following => following_entity(scene),
    }
}

/// The child's table.
pub fn child_table() -> &'static [super::goals::GoalRule] {
    CHILD_TABLE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wander() -> WanderScene {
        WanderScene {
            citizen: CitizenFacts::default(),
            wander: WanderFacts {
                nav_done: true,
                leisure_site: Some(Pos::new(8, 64, 8)),
                site_kind: SiteKind::Library,
                roll: 4,
                wander_roll: 10,
                // `nextInt(10) <= 0` : 3 donc la branche 1-in-10 de `:167` ne tire
                // pas, et la selection de place est atteinte.
                spot_roll: 3,
                choice_roll: 0,
                book_roll: 99,
                seats: vec![super::super::facts::Seat {
                    pos: Pos::new(9, 64, 8),
                    inside: true,
                    sitting: true,
                    occupied: false,
                }],
                arrived: true,
                ..WanderFacts::default()
            },
            ..WanderScene::default()
        }
    }

    fn child() -> ChildScene {
        ChildScene {
            citizen: CitizenFacts {
                baby: true,
                ..CitizenFacts::default()
            },
            child: ChildFacts {
                baby: true,
                ready_roll: 10,
                delay_roll: 0,
                follow_roll: 0,
                grow_roll: 0,
                colony_citizens: 3,
                buildings: vec![Pos::new(2, 64, 2)],
                follow_candidate: Some(Pos::new(3, 64, 3)),
                ..ChildFacts::default()
            },
            ..ChildScene::default()
        }
    }

    #[test]
    fn a_guard_never_wanders_and_a_baby_neither() {
        // `:293-297`.
        let mut scene = wander();
        scene.wander.guard_job = true;
        assert_eq!(
            wander_step(WanderState::CitizenIdle, &mut scene),
            Step::Next(WanderState::CitizenIdle)
        );
        let mut baby = wander();
        baby.wander.baby = true;
        assert_eq!(
            wander_step(WanderState::CitizenIdle, &mut baby),
            Step::Next(WanderState::CitizenIdle)
        );
        let mut moving = wander();
        moving.wander.nav_done = false;
        assert_eq!(
            wander_step(WanderState::CitizenIdle, &mut moving),
            Step::Next(WanderState::CitizenIdle)
        );
    }

    #[test]
    fn five_in_a_hundred_goes_to_the_leisure_site_and_the_rest_walks() {
        // `:266-289`.
        let mut lucky = wander();
        assert_eq!(
            wander_step(WanderState::CitizenIdle, &mut lucky),
            Step::Next(WanderState::GoToLeisureSite)
        );
        assert_eq!(lucky.memory.delay, 1200);
        let mut unlucky = wander();
        unlucky.wander.roll = 5;
        assert_eq!(
            wander_step(WanderState::CitizenIdle, &mut unlucky),
            Step::Next(WanderState::CitizenIdle)
        );
        assert_eq!(unlucky.memory.leisure_site, None);
    }

    #[test]
    fn a_lucky_roll_without_a_site_site_still_only_walks() {
        // `:282-286`: the delay is set only when a site exists.
        let mut scene = wander();
        scene.wander.leisure_site = None;
        assert_eq!(
            wander_step(WanderState::CitizenIdle, &mut scene),
            Step::Next(WanderState::CitizenIdle)
        );
        assert_eq!(scene.memory.delay, 0);
    }

    #[test]
    fn walking_to_the_site_arrives_then_wanders_there() {
        // `:134-148`. `goToLeisureSite` relit `leisureSite` (`:136`), le champ du
        // but : sans lui, il rend `IDLE`.
        let mut scene = wander();
        scene.memory.leisure_site = Some(Pos::new(8, 64, 8));
        scene.wander.arrived = false;
        assert_eq!(
            wander_step(WanderState::GoToLeisureSite, &mut scene),
            Step::Next(WanderState::GoToLeisureSite)
        );
        scene.wander.arrived = true;
        assert_eq!(
            wander_step(WanderState::GoToLeisureSite, &mut scene),
            Step::Next(WanderState::WanderAtLeisureSite)
        );
        let mut lost = wander();
        lost.memory.leisure_site = None;
        assert_eq!(
            wander_step(WanderState::GoToLeisureSite, &mut lost),
            Step::Next(WanderState::CitizenIdle)
        );
    }

    #[test]
    fn a_library_visit_can_end_in_a_book_and_a_plain_hut_cannot() {
        // `:167-178` : lire est un **`else if`** derriere la balade 1-sur-10, et il
        // faut une bibliotheque ou une universite. `leisureSite` est le champ prive
        // du but (`:68`), donc la memoire : c'est lui que la regle relit.
        let mut library = wander();
        library.memory.leisure_site = Some(Pos::new(8, 64, 8));
        library.wander.book_roll = 4;
        assert_eq!(
            wander_step(WanderState::WanderAtLeisureSite, &mut library),
            Step::Next(WanderState::ReadABook)
        );

        // Le meme tirage dans un hut ordinaire : jamais de livre.
        let mut hut = wander();
        hut.memory.leisure_site = Some(Pos::new(8, 64, 8));
        hut.wander.site_kind = SiteKind::Other;
        hut.wander.book_roll = 4;
        assert_eq!(
            wander_step(WanderState::WanderAtLeisureSite, &mut hut),
            Step::Next(WanderState::WanderAtLeisureSite)
        );

        // Et la balade 1-sur-10 passe **avant** : meme dans une bibliotheque avec un
        // tirage de livre gagnant, `spotRoll = 0` fait marcher au hasard.
        let mut drawn = wander();
        drawn.memory.leisure_site = Some(Pos::new(8, 64, 8));
        drawn.wander.spot_roll = 0;
        drawn.wander.book_roll = 0;
        assert_eq!(
            wander_step(WanderState::WanderAtLeisureSite, &mut drawn),
            Step::Next(WanderState::WanderAtLeisureSite)
        );
    }

    #[test]
    fn a_reached_seat_is_sat_on_and_the_walk_is_forgotten() {
        // `:243-251` — `sitDown(walkTo, citizen, 30s)`, `setCurrentDelay(30)`,
        // `walkTo = null`.
        let mut scene = wander();
        scene.memory.leisure_site = Some(Pos::new(8, 64, 8));
        scene.memory.walk_to = Some(Pos::new(9, 64, 8));
        assert_eq!(
            wander_step(WanderState::WanderAtLeisureSite, &mut scene),
            Step::Next(WanderState::WanderAtLeisureSite)
        );
        assert!(scene.sat_down);
        assert_eq!(scene.memory.walk_to, None);
    }

    #[test]
    fn a_standing_spot_is_reached_without_sitting() {
        // `:205-209` et `:243-251` : `!=` une place assise, donc pas de `sitDown`.
        let mut scene = wander();
        scene.memory.leisure_site = Some(Pos::new(8, 64, 8));
        scene.wander.seats = vec![super::super::facts::Seat {
            pos: Pos::new(9, 64, 8),
            inside: true,
            sitting: false,
            occupied: false,
        }];
        scene.memory.walk_to = Some(Pos::new(9, 64, 8));
        let _ = wander_step(WanderState::WanderAtLeisureSite, &mut scene);
        assert!(!scene.sat_down);
    }

    #[test]
    fn reading_ends_five_times_in_a_hundred_and_starts_with_a_book_in_hand() {
        // `:103-115`.
        let mut reading = wander();
        reading.wander.book_roll = 4;
        reading.wander.arrived = true;
        reading.memory.leisure_site = Some(Pos::new(8, 64, 8));
        reading.memory.walk_to = Some(Pos::new(9, 64, 8));
        assert_eq!(
            wander_step(WanderState::ReadABook, &mut reading),
            Step::Next(WanderState::CitizenIdle)
        );
        assert!(reading.studied);
        assert_eq!(reading.held, None);
        assert_eq!(reading.memory.leisure_site, None);

        let mut still = wander();
        still.wander.book_roll = 5;
        still.memory.leisure_site = Some(Pos::new(8, 64, 8));
        still.memory.walk_to = Some(Pos::new(9, 64, 8));
        assert_eq!(
            wander_step(WanderState::ReadABook, &mut still),
            Step::Next(WanderState::ReadABook)
        );
        assert_eq!(still.held, Some("minecraft:book"));
    }

    #[test]
    fn a_seat_is_preferred_to_a_standing_spot() {
        // `:191-243`.
        let seat = super::super::facts::Seat {
            pos: Pos::new(1, 64, 1),
            inside: true,
            sitting: true,
            occupied: false,
        };
        let stand = super::super::facts::Seat {
            pos: Pos::new(2, 64, 2),
            inside: true,
            sitting: false,
            occupied: false,
        };
        let mut scene = wander();
        scene.wander.seats = vec![seat, stand];
        assert_eq!(pick_spot(&scene), Some(seat.pos));
        scene.wander.seats = vec![stand];
        assert_eq!(pick_spot(&scene), Some(stand.pos));
        scene.wander.seats = vec![];
        assert_eq!(pick_spot(&scene), None);
    }

    #[test]
    fn a_child_is_ready_when_the_timer_is_spent_and_the_draw_lands() {
        // `:127-140`.
        let mut scene = child();
        scene.child.ready_roll = 0;
        scene.child.delay_roll = 0;
        assert!(is_ready_for_activity(&mut scene));
        assert_eq!(scene.memory.action_timer, 3600);
        let mut not_yet = child();
        not_yet.child.ready_roll = 3;
        assert!(!is_ready_for_activity(&mut not_yet));
        let mut busy = child();
        busy.memory.action_timer = 500;
        busy.child.ready_roll = 0;
        assert!(!is_ready_for_activity(&mut busy));
        assert_eq!(busy.memory.action_timer, 400, "the timer loses 100 per look");
    }

    #[test]
    fn a_ready_child_follows_a_neighbour_for_thirty_to_sixty_seconds() {
        // `:156-187`.
        let mut scene = child();
        scene.child.ready_roll = 0;
        assert_eq!(
            child_follow_step(&mut scene),
            Step::Next(ChildState::Following)
        );
        assert_eq!(scene.memory.follow_target, Some(Pos::new(3, 64, 3)));
        assert_eq!(scene.memory.follow_start, Some(scene.citizen.at));
        assert!(scene.memory.action_timer >= 600);
        assert!(scene.memory.action_timer <= 1200);
    }

    /// The `IDLE → FOLLOWING` transition of `:116`, which needs a ready draw.
    fn child_follow_step(scene: &mut ChildScene) -> Step<ChildState> {
        search_entity_to_follow(scene)
    }

    #[test]
    fn a_ready_child_without_a_neighbour_neither_follows_nor_visits() {
        // `:156-186` returns false, and `:134` needs the same draw.
        let mut scene = child();
        scene.child.ready_roll = 0;
        scene.child.follow_candidate = None;
        assert_eq!(child_follow_step(&mut scene), Step::Stayed);
        let mut no_hut = child();
        no_hut.child.ready_roll = 0;
        no_hut.child.buildings.clear();
        assert_eq!(visit(&mut no_hut), Step::Stayed);
    }

    #[test]
    fn a_ready_child_visits_a_hut_for_three_minutes() {
        // `:114`, `:215-257`.
        let mut scene = child();
        scene.child.ready_roll = 0;
        assert_eq!(visit(&mut scene), Step::Next(ChildState::Visiting));
        assert_eq!(scene.memory.visiting_hut, Some(Pos::new(2, 64, 2)));
        assert!(scene.building_chosen);
        assert_eq!(scene.memory.action_timer, 3600);
    }

    #[test]
    fn a_follow_ends_when_the_timer_runs_out_or_the_target_goes() {
        // `:194-208`.
        let mut scene = child();
        scene.memory.follow_target = Some(Pos::new(3, 64, 3));
        // `actionTimer -= 20` on every look (`:196`).
        scene.memory.action_timer = 40;
        assert_eq!(
            child_step(ChildState::Following, &mut scene),
            Step::Next(ChildState::Following)
        );
        assert_eq!(
            child_step(ChildState::Following, &mut scene),
            Step::Next(ChildState::Bored)
        );
        let mut lost = child();
        lost.memory.follow_target = None;
        lost.memory.action_timer = 500;
        assert_eq!(
            child_step(ChildState::Following, &mut lost),
            Step::Next(ChildState::Bored)
        );
    }

    #[test]
    fn a_visit_ends_after_the_timer_and_sets_a_new_delay() {
        // `:237-256`. `actionTimer -= 120` puis `> 0` (strict) : a 120 pile la
        // visite se termine deja, il faut donc 240 pour voir un tick de visite.
        let mut scene = child();
        scene.memory.action_timer = 240;
        assert_eq!(
            child_step(ChildState::Visiting, &mut scene),
            Step::Next(ChildState::Visiting)
        );
        assert_eq!(
            child_step(ChildState::Visiting, &mut scene),
            Step::Next(ChildState::Bored)
        );
        assert!(scene.memory.action_timer >= 3600);
    }

    #[test]
    fn a_child_only_grows_up_after_four_thousand_ticks_of_activity() {
        // `:279-302`.
        let mut scene = child();
        scene.memory.active_time = 3999;
        assert!(!try_grow_up(&mut scene));
        assert!(!scene.grew_up);
        scene.memory.active_time = 4000;
        scene.child.grow_roll = 0;
        assert!(try_grow_up(&mut scene));
        assert!(scene.grew_up);
        assert!(scene.notified, "ten citizens or fewer: the colony is told");
    }

    #[test]
    fn a_colony_can_lend_two_thousand_ticks_to_a_child() {
        // `:271-277`.
        let mut scene = child();
        scene.memory.active_time = 2000;
        scene.child.colony_granted_time = true;
        scene.child.grow_roll = 1;
        assert!(!try_grow_up(&mut scene));
        assert_eq!(scene.memory.active_time, 4000);
    }

    #[test]
    fn an_unlucky_child_still_grows_up_at_seventy_thousand_ticks() {
        // `:284`: the `||` half of the condition.
        let mut scene = child();
        scene.memory.active_time = 70001;
        scene.child.grow_roll = 1;
        assert!(try_grow_up(&mut scene));
    }

    #[test]
    fn a_large_colony_is_not_told_about_every_growth() {
        // `:290-293`.
        let mut scene = child();
        scene.memory.active_time = 70001;
        scene.child.grow_roll = 1;
        scene.child.colony_citizens = 11;
        assert!(try_grow_up(&mut scene));
        assert!(!scene.notified);
    }

    #[test]
    fn an_adult_is_not_grown_up_again() {
        // `:266-269`.
        let mut scene = child();
        scene.citizen.baby = false;
        scene.memory.active_time = 70000;
        scene.child.grow_roll = 1;
        assert!(!try_grow_up(&mut scene));
    }

    #[test]
    fn the_two_timers_keep_their_bounds() {
        assert_eq!(LEISURE_CHANCE.v(), 5);
        assert_eq!(MIN_ACTIVE_TIME.v(), 4000);
        assert_eq!(BONUS_TIME_COLONY.v(), 2000);
        assert_eq!(GROW_UP_CUTOFF.v(), 70000);
        assert_eq!(VISIT_TICKS.v(), 3600);
        assert_eq!(START_FOLLOW_DISTANCE.v(), 10);
        assert!(DELAY_MIN.source.contains("EntityAICitizenChild.java:148"));
    }
}
