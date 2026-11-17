//! Eating: the nine states of `EntityAIEatTask`, and the one rule each of them
//! carries.
//!
//! Java reference: `core/entity/ai/minimal/EntityAIEatTask.java`. Every state of
//! this class has **exactly one** outgoing transition, registered with the constant
//! condition `() -> true` (`:148-161`), so the state machine is a chain and the
//! priority within a state is trivial. What decides here is the *action*: which
//! state the branch of the `if` produces. The port therefore exposes
//! [`step`] — the action of the state we are in — and keeps [`super::goals::EAT_TABLE`]
//! for the order the entries were registered in.
//!
//! **`hasFood()` is impure and the port says so.** Java's `hasFood` sets
//! `foodSlot` **and** fires up to three interactions (`:519-546`):
//! `BETTER_FOOD_CHILDREN`, `BETTER_FOOD`, `RAW_FOOD`. [`has_food`] keeps the slot,
//! [`complaint`] answers the interaction, and neither is buried in the other.
//!
//! **What is not here.** Walking, sitting, swinging, eating the stack, the sound
//! and the particles all need the level: `MANQUANT.md` §1 names the primitives
//! (`walkToPos`, `sitDown`, `consumeFood`).

use super::facts::{
    AVERAGE_SATURATION, CitizenFacts, FoodFacts, HomeFacts, JobKind, MAX_SATURATION,
    RestaurantFacts, Tuned, TunedF,
};
pub use super::goals::Step;
use super::goals::{EatState, EAT_TABLE, Rule};
use crate::ai::facts::Pos;

/// `MINUTES_WAITING_TIME` (`EntityAIEatTask.java:60`): two minutes without food
/// and the citizen serves itself.
pub const MINUTES_WAITING_TIME: Tuned = Tuned::new(2, "EntityAIEatTask.java:60");
/// `REQUIRED_TIME_TO_EAT` (`:65`): five ticks of chewing per stack.
pub const REQUIRED_TIME_TO_EAT: Tuned = Tuned::new(5, "EntityAIEatTask.java:65");
/// `GET_YOURSELF_SATURATION` (`:70`): the threshold the brief of that field
/// describes; **unused by this class**, kept because it is declared.
pub const GET_YOURSELF_SATURATION: Tuned = Tuned::new(30, "EntityAIEatTask.java:70");
/// `RESTAURANT_LIMIT` (`:75`): how far a restaurant may be. Declared, and read
/// nowhere in this class.
pub const RESTAURANT_LIMIT: TunedF = TunedF::new(2.5, "EntityAIEatTask.java:75");
/// `MINUTES_BETWEEN_FOOD_CHECKS` (`:96`). Declared, and read nowhere.
pub const MINUTES_BETWEEN_FOOD_CHECKS: Tuned = Tuned::new(5, "EntityAIEatTask.java:96");
/// The walk-to-the-seat timeout written inline (`:287`): `timeOutWalking++ > 400`.
pub const EAT_POS_WALK_TIMEOUT: Tuned = Tuned::new(400, "EntityAIEatTask.java:287");
/// The extra delay added after sitting down, in tenths of a second (`:310`).
pub const SIT_DELAY: Tuned = Tuned::new(10, "EntityAIEatTask.java:310");
/// How long a citizen sits at the table (`:308`): `TICKS_SECOND * 60`.
pub const SIT_TICKS: Tuned = Tuned::new(1200, "EntityAIEatTask.java:308");
/// The `1.5` multiplier that hands out a bit of extra food (`:271`).
pub const FOOD_HANDOUT_EXTRA: TunedF = TunedF::new(1.5, "EntityAIEatTask.java:271");

/// The eating AI's own working memory: every `private` field of `EntityAIEatTask`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Memory {
    /// `waitingTicks` (`:106`).
    pub waiting_ticks: i32,
    /// `foodSlot` (`:111`), `None` for Java's `-1`.
    pub food_slot: Option<usize>,
    /// `eatPos` (`:116`).
    pub eat_pos: Option<Pos>,
    /// `restaurantPos` (`:121`).
    pub restaurant_pos: Option<Pos>,
    /// `timeOutWalking` (`:131`).
    pub time_out_walking: i32,
    /// `eatenFood` (`:136`): the items eaten in this meal, in order, so that
    /// `addLastEaten` is called once per distinct item (`:233-239`).
    pub eaten_food: Vec<&'static str>,
    /// `foodHandler.getLastEaten()` — what the citizen ate last, which
    /// `addLastEaten` must not set twice (`:235`).
    pub last_eaten: Option<&'static str>,
    /// `citizenData.setJustAte(true)` (`:241`, `:381`).
    pub just_ate: bool,
}

impl Memory {
    /// `reset()` (`:551-562`).
    pub fn reset(&mut self) {
        *self = Memory::default();
    }
}

/// The facts this goal reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    /// The citizen.
    pub citizen: CitizenFacts,
    /// Its inventory and the hut's chest.
    pub food: FoodFacts,
    /// The restaurant it heads for.
    pub restaurant: RestaurantFacts,
    /// Its home — only `isCook` and the `canEat` test need it.
    pub home: HomeFacts,
    /// Its memory.
    pub memory: Memory,
    /// `EntityNavigationUtils.walkToPos(citizen, eatPos, 2, true)` reports the
    /// citizen reached the seat (`:306`).
    pub at_eat_pos: bool,
    /// The interaction the last step raised, if any — Java calls
    /// `citizenData.triggerInteraction(...)` from inside the rule (`:508`,
    /// `:534-544`). Kept beside the scene so a rule stays a pure decision and the
    /// caller decides what a message means.
    pub interaction: Option<&'static str>,
}

/// `hasFood()` (`:519-546`) — the slot of the best food, if any.
///
/// The restaurant's menu is only consulted when a restaurant is known (`:521`),
/// which is why the caller passes the facts rather than the citizen alone.
pub fn has_food(scene: &mut Scene) -> bool {
    let slot = scene.food.best_slot;
    scene.memory.food_slot = slot;
    slot.is_some()
}

/// The interaction `hasFood` raises when it finds nothing (`:530-544`).
///
/// Three cases, and the order matters: an eatable stack that is not the *best*
/// food is a complaint about quality, a cookable one a complaint about rawness.
pub fn complaint(scene: &Scene) -> Option<&'static str> {
    // `hasFood` (`:521`) recomputes the best slot on every call: a best food found
    // here means "no complaint", whatever the memory last said.
    if scene.food.best_slot.is_some() {
        return None;
    }
    if scene.food.eatable > 0 {
        return Some(if scene.citizen.child {
            "com.minecolonies.coremod.gui.chat.betterfoodchildren"
        } else {
            "com.minecolonies.coremod.gui.chat.betterfood"
        });
    }
    if scene.food.cookable > 0 {
        return Some("com.minecolonies.coremod.gui.chat.rawfood");
    }
    None
}

/// `getFood` (`:169-177`): eat what we carry, otherwise try the hut.
pub fn get_food(scene: &mut Scene) -> Step<EatState> {
    if has_food(scene) {
        Step::Next(EatState::Eat)
    } else {
        Step::Next(EatState::GoToHut)
    }
}

/// `eat` (`:184-243`).
///
/// Five ticks per bite (`:212`), and **one stack at a time**: after eating, if the
/// citizen is still below full saturation and the slot still holds food, the timer
/// restarts and it eats again (`:227-231`).
pub fn eat(scene: &mut Scene) -> Step<EatState> {
    if !has_food(scene) {
        return Step::Next(EatState::CheckForFood);
    }
    if scene.memory.food_slot.is_none() {
        return Step::Next(EatState::CheckForFood);
    }

    scene.memory.waiting_ticks += 1;
    if scene.memory.waiting_ticks < REQUIRED_TIME_TO_EAT.v() as i32 {
        return Step::Next(EatState::Eat);
    }

    // `foodHandler.addLastEaten(foodStack.getItem())` — only when nothing has been
    // eaten yet in this meal and a restaurant supplied the food (`:218-221`).
    if scene.memory.eaten_food.is_empty() && scene.memory.restaurant_pos.is_some() {
        if let Some(item) = scene.food.restaurant_stack {
            scene.memory.last_eaten = Some(item);
        }
    }
    if let Some(item) = scene.food.restaurant_stack {
        if !scene.memory.eaten_food.contains(&item) {
            scene.memory.eaten_food.push(item);
        }
    }

    // `citizenData.getSaturation() < FULL_SATURATION && !stack.isEmpty()`
    // (`:227`): a second bite happens only when the stack still holds something.
    let still_hungry = scene.citizen.saturation < MAX_SATURATION.f64();
    if still_hungry && scene.food.stack_left > 0 {
        scene.memory.waiting_ticks = 0;
        return Step::Next(EatState::Eat);
    }

    // `for (foodItem : eatenFood) if (lastEaten != foodItem) addLastEaten`.
    if let Some(item) = scene.memory.eaten_food.last().copied() {
        if scene.memory.last_eaten != Some(item) {
            scene.memory.last_eaten = Some(item);
        }
    }
    scene.memory.eaten_food.clear();
    scene.memory.just_ate = true;
    Step::Next(EatState::Idle)
}

/// `getFoodYourself` (`:250-278`).
pub fn get_food_yourself(scene: &mut Scene) -> Step<EatState> {
    if scene.memory.restaurant_pos.is_none() {
        return Step::Next(EatState::SearchRestaurant);
    }
    if !scene.restaurant.we_are_inside {
        return Step::Next(EatState::GetFoodYourself);
    }
    if scene.food.restaurant_stack.is_some() {
        return Step::Next(EatState::Eat);
    }
    Step::Next(EatState::WaitForFood)
}

/// How many of a stack the citizen takes when serving itself: `ceil(max(1, (FULL -
/// saturation) / foodValue) * 1.5)` (`:269-271`).
pub fn self_serve_quantity(saturation: f64, food_value: f64) -> i32 {
    if food_value <= 0.0 {
        // `FoodUtils.getFoodValue` never answers zero for a food stack; a fact that
        // does is a caller's bug, and one item is the safe refusal.
        return 1;
    }
    // Java truncates **before** the extra: `qty = (int) max(1.0, missing / value)`
    // (`:269`), then `qty = (int) ceil(qty * 1.5)` (`:271`).
    let ratio = (MAX_SATURATION.f64() - saturation) / food_value;
    let qty = ratio.max(1.0) as i32;
    ((qty as f64) * FOOD_HANDOUT_EXTRA.v()).ceil() as i32
}

/// `goToEatingPlace` (`:285-324`).
pub fn go_to_eating_place(scene: &mut Scene) -> Step<EatState> {
    let timed_out = scene.memory.eat_pos.is_none()
        || {
            scene.memory.time_out_walking += 1;
            scene.memory.time_out_walking > EAT_POS_WALK_TIMEOUT.v() as i32
        };
    if timed_out {
        if has_food(scene) {
            scene.memory.time_out_walking = 0;
            return Step::Next(EatState::Eat);
        }
        scene.memory.waiting_ticks += 1;
        let guard_at_night = scene.citizen.job.is_guard() && !scene.citizen.daytime;
        if scene.memory.waiting_ticks > 60 * MINUTES_WAITING_TIME.v() as i32 || guard_at_night {
            scene.memory.waiting_ticks = 0;
            return Step::Next(EatState::GetFoodYourself);
        }
    }

    if scene.memory.eat_pos.is_some() && scene.at_eat_pos {
        // `SittingEntity.sitDown(eatPos, citizen, TICKS_SECOND * 60)` (`:308`).
        scene.memory.time_out_walking += SIT_DELAY.v() as i32;
        if !has_food(scene) {
            scene.memory.waiting_ticks += 1;
            if scene.memory.waiting_ticks > 60 * MINUTES_WAITING_TIME.v() as i32 {
                scene.memory.waiting_ticks = 0;
                return Step::Next(EatState::GetFoodYourself);
            }
        }
    }
    Step::Next(EatState::GoToEatPos)
}

/// `findPlaceToEat` (`:331-343`): a seat, but only inside a cook's building.
pub fn find_place_to_eat(scene: &Scene) -> Option<Pos> {
    if scene.memory.restaurant_pos.is_none() {
        return None;
    }
    scene.restaurant.sitting_pos
}

/// `waitForFood` (`:350-386`).
pub fn wait_for_food(scene: &mut Scene) -> Step<EatState> {
    if scene.restaurant.pos.is_none() {
        return Step::Next(EatState::SearchRestaurant);
    }
    scene.memory.restaurant_pos = scene.restaurant.pos;
    if !scene.restaurant.we_are_inside {
        return Step::Next(EatState::GoToRestaurant);
    }
    if let Some(pos) = find_place_to_eat(scene) {
        scene.memory.eat_pos = Some(pos);
        return Step::Next(EatState::GoToEatPos);
    }
    if has_food(scene) {
        return Step::Next(EatState::Eat);
    }
    if scene.citizen.saturation >= AVERAGE_SATURATION.f64() {
        scene.memory.reset();
        scene.memory.just_ate = true;
        return Step::Next(EatState::Done);
    }
    Step::Next(EatState::WaitForFood)
}

/// `goToHut` (`:393-442`).
pub fn go_to_hut(scene: &mut Scene) -> Step<EatState> {
    match scene.home.building {
        None => return Step::Next(EatState::SearchRestaurant),
        Some(_) if !scene.arrived_at_hut() => return Step::Next(EatState::GoToHut),
        Some(_) => {}
    }
    scene.memory.restaurant_pos = None;

    if scene.food.best_slot.is_some() {
        return Step::Next(EatState::Eat);
    }
    if scene.food.hut_stack.is_some() {
        return Step::Next(EatState::Eat);
    }
    if scene.citizen.job.is_cook() {
        scene.memory.reset();
        return Step::Next(EatState::Done);
    }
    Step::Next(EatState::SearchRestaurant)
}

impl Scene {
    /// `WorldUtil.isBlockLoaded(level, buildingWorker.getPosition())` and the walk
    /// that follows it (`:396-405`). A scene the caller has not filled in counts as
    /// arrived, so a test can stay about the branch it is testing.
    fn arrived_at_hut(&self) -> bool {
        self.home.arrived_at_work
    }
}

/// `goToRestaurant` (`:449-473`).
///
/// The cavalry branch is the interesting one: a mounted guard stops riding before
/// it gets near the dining hall (`:454-457`), so the horse is not ridden indoors.
pub fn go_to_restaurant(scene: &mut Scene) -> Step<EatState> {
    if scene.memory.restaurant_pos.is_none() {
        return Step::Next(EatState::SearchRestaurant);
    }
    if scene.citizen.job.is_cavalry()
        && scene.citizen.riding
        && scene.restaurant.within_parking_range
    {
        scene.citizen.riding = false;
    }
    if scene.restaurant.we_are_inside {
        return Step::Next(EatState::WaitForFood);
    }
    // `:466` — `!walkToBuilding(...)` means "keep walking"; a finished walk falls
    // through to `SEARCH_RESTAURANT` (`:472`), which looks strange and is exactly
    // what Java does.
    if !scene.restaurant.walk_arrived {
        return Step::Next(EatState::GoToRestaurant);
    }
    Step::Next(EatState::SearchRestaurant)
}

/// `searchRestaurant` (`:480-512`).
///
/// **A staffed cook first, any cook second** (`:488-492`): the citizen prefers a
/// restaurant somebody works in, and only then the nearest one.
pub fn search_restaurant(scene: &mut Scene) -> Step<EatState> {
    if scene.citizen.working {
        scene.citizen.working = false;
    }
    if scene.restaurant.pos.is_none() {
        if scene.citizen.saturation >= AVERAGE_SATURATION.f64() {
            scene.memory.reset();
            scene.memory.just_ate = true;
            return Step::Next(EatState::Done);
        }
        // `StandardInteraction(NO_RESTAURANT, BLOCKING)` (`:508`), then look again.
        scene.interaction = Some("com.minecolonies.coremod.gui.chat.norestaurant");
        return Step::Next(EatState::CheckForFood);
    }
    scene.memory.restaurant_pos = scene.restaurant.pos;
    Step::Next(EatState::GoToRestaurant)
}

/// The rule of one state: `EntityAIEatTask`'s action for `state`.
///
/// `EatState::Done` is handled by the table (`:153`), not here — one tick after
/// `DONE` the citizen machine gets `IDLE` back, and this module has nothing to add.
pub fn step(state: EatState, scene: &mut Scene) -> Step<EatState> {
    match state {
        EatState::CitizenEating => {
            scene.memory.reset();
            Step::Next(EatState::CheckForFood)
        }
        EatState::CheckForFood => get_food(scene),
        EatState::GoToHut => go_to_hut(scene),
        EatState::SearchRestaurant => search_restaurant(scene),
        EatState::GoToRestaurant => go_to_restaurant(scene),
        EatState::WaitForFood => wait_for_food(scene),
        EatState::GetFoodYourself => get_food_yourself(scene),
        EatState::GoToEatPos => go_to_eating_place(scene),
        EatState::Eat => eat(scene),
        // `:153` — `DONE` leads to `CitizenAIState.IDLE`, which the citizen machine
        // owns. Reported as such rather than as a change of this module.
        EatState::Done => Step::Next(EatState::Idle),
        // Nothing left to do: the citizen machine has the citizen back.
        EatState::Idle => Step::Stayed,
    }
}

/// The table this module implements, for `tests.rs` and the report.
pub fn table() -> &'static [Rule<EatState>] {
    EAT_TABLE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene_with_food(slot: usize) -> Scene {
        let mut scene = Scene::default();
        scene.food.best_slot = Some(slot);
        scene.food.eatable = 1;
        scene
    }

    #[test]
    fn entering_eating_resets_the_memory_and_asks_for_food() {
        // `:148-151`.
        let mut scene = Scene::default();
        scene.memory.waiting_ticks = 99;
        scene.memory.food_slot = Some(3);
        assert_eq!(
            step(EatState::CitizenEating, &mut scene),
            Step::Next(EatState::CheckForFood)
        );
        assert_eq!(scene.memory.waiting_ticks, 0);
        assert_eq!(scene.memory.food_slot, None);
    }

    #[test]
    fn food_in_the_pocket_goes_straight_to_eating_and_otherwise_to_the_hut() {
        // `:169-177`.
        let mut hungry = Scene::default();
        assert_eq!(
            step(EatState::CheckForFood, &mut hungry),
            Step::Next(EatState::GoToHut)
        );
        let mut fed = scene_with_food(2);
        assert_eq!(
            step(EatState::CheckForFood, &mut fed),
            Step::Next(EatState::Eat)
        );
        assert_eq!(fed.memory.food_slot, Some(2));
    }

    #[test]
    fn eating_takes_five_ticks_and_then_marks_the_citizen_as_fed() {
        // `:211-215` then `:241-242`.
        let mut scene = scene_with_food(0);
        for _ in 0..4 {
            assert_eq!(step(EatState::Eat, &mut scene), Step::Next(EatState::Eat));
        }
        assert_eq!(scene.memory.waiting_ticks, 4);
        // The fifth tick bites: nothing left to eat, so the meal ends.
        scene.food.best_slot = None;
        assert_eq!(step(EatState::Eat, &mut scene), Step::Next(EatState::CheckForFood));
        let mut scene = scene_with_food(0);
        scene.citizen.saturation = MAX_SATURATION.f64();
        for _ in 0..4 {
            let _ = step(EatState::Eat, &mut scene);
        }
        assert_eq!(step(EatState::Eat, &mut scene), Step::Next(EatState::Idle));
        assert!(scene.memory.just_ate);
    }

    #[test]
    fn losing_the_food_mid_meal_goes_back_to_the_search() {
        // `:186-189` and `:193-196`.
        let mut scene = Scene::default();
        assert_eq!(
            step(EatState::Eat, &mut scene),
            Step::Next(EatState::CheckForFood)
        );
    }

    #[test]
    fn the_self_serve_quantity_is_ceil_of_the_missing_saturation_times_one_and_a_half() {
        // `:269-271`.
        assert_eq!(
            self_serve_quantity(60.0, 6.0),
            2,
            "full: one item, then the extra 1.5 rounds up"
        );
        assert_eq!(self_serve_quantity(0.0, 6.0), 15, "60/6 = 10, times 1.5");
        assert_eq!(self_serve_quantity(30.0, 5.0), 9, "30/5 = 6, times 1.5");
        assert_eq!(
            self_serve_quantity(50.0, 4.0),
            3,
            "2.5 truncates to 2 first, then 3.0 — not 3.75 rounded up"
        );
        assert_eq!(self_serve_quantity(10.0, 0.0), 1, "a zero food value is a refusal");
    }

    #[test]
    fn a_cook_that_found_nothing_at_its_own_hut_stops_asking() {
        // `:435-439`.
        let mut scene = Scene::default();
        scene.citizen.job = JobKind::Cook;
        scene.home.building = Some(Pos::new(0, 64, 0));
        scene.home.arrived_at_work = true;
        assert_eq!(
            step(EatState::GoToHut, &mut scene),
            Step::Next(EatState::Done)
        );
        let mut worker = Scene::default();
        worker.home.building = Some(Pos::new(0, 64, 0));
        worker.home.arrived_at_work = true;
        assert_eq!(
            step(EatState::GoToHut, &mut worker),
            Step::Next(EatState::SearchRestaurant)
        );
    }

    #[test]
    fn a_citizen_without_a_hut_skips_straight_to_the_restaurant() {
        // `:395-399`.
        let mut scene = Scene::default();
        assert_eq!(
            step(EatState::GoToHut, &mut scene),
            Step::Next(EatState::SearchRestaurant)
        );
    }

    #[test]
    fn no_restaurant_at_all_is_a_blocking_complaint_unless_the_citizen_is_fed() {
        // `:500-510`.
        let mut scene = Scene::default();
        scene.citizen.saturation = 2.0;
        assert_eq!(
            step(EatState::SearchRestaurant, &mut scene),
            Step::Next(EatState::CheckForFood)
        );
        assert_eq!(
            scene.interaction,
            Some("com.minecolonies.coremod.gui.chat.norestaurant")
        );
        let mut fed = Scene::default();
        fed.citizen.saturation = 30.0;
        assert_eq!(
            step(EatState::SearchRestaurant, &mut fed),
            Step::Next(EatState::Done)
        );
        assert!(fed.memory.just_ate);
    }

    #[test]
    fn searching_for_a_restaurant_stops_the_working_flag() {
        // `:494-498`.
        let mut scene = Scene::default();
        scene.citizen.working = true;
        scene.restaurant.pos = Some(Pos::new(5, 64, 5));
        assert_eq!(
            step(EatState::SearchRestaurant, &mut scene),
            Step::Next(EatState::GoToRestaurant)
        );
        assert!(!scene.citizen.working);
        assert_eq!(scene.memory.restaurant_pos, Some(Pos::new(5, 64, 5)));
    }

    #[test]
    fn a_mounted_cavalryman_dismounts_before_entering_the_dining_hall() {
        // `:451-473`.
        let mut scene = Scene::default();
        scene.citizen.job = JobKind::Cavalry;
        scene.citizen.riding = true;
        scene.memory.restaurant_pos = Some(Pos::new(4, 64, 4));
        scene.restaurant.within_parking_range = true;
        // Le cheval est arrete avant d'arriver : `:454-457`.
        let stopped = step(EatState::GoToRestaurant, &mut scene);
        assert!(!scene.citizen.riding, "stopped riding");
        // ... mais la marche n'a pas encore abouti, donc Java redemande un chemin
        // (`:466-469`).
        assert_eq!(stopped, Step::Next(EatState::GoToRestaurant));
        // Une fois la marche terminee, `:472` renvoie `SEARCH_RESTAURANT`.
        scene.restaurant.walk_arrived = true;
        assert_eq!(
            step(EatState::GoToRestaurant, &mut scene),
            Step::Next(EatState::SearchRestaurant)
        );
    }

    #[test]
    fn a_ranger_does_not_dismount_because_it_has_no_horse() {
        // The branch is `instanceof JobCavalry` only (`:454`).
        let mut scene = Scene::default();
        scene.citizen.job = JobKind::Guard;
        scene.citizen.riding = true;
        scene.memory.restaurant_pos = Some(Pos::new(4, 64, 4));
        scene.restaurant.within_parking_range = true;
        let _ = step(EatState::GoToRestaurant, &mut scene);
        assert!(scene.citizen.riding);
    }

    #[test]
    fn waiting_for_food_prefers_a_seat_then_its_own_pocket_then_a_full_stomach() {
        // `:367-385`.
        let mut seated = Scene::default();
        seated.restaurant.pos = Some(Pos::new(1, 64, 1));
        seated.restaurant.we_are_inside = true;
        seated.restaurant.sitting_pos = Some(Pos::new(2, 64, 2));
        assert_eq!(
            step(EatState::WaitForFood, &mut seated),
            Step::Next(EatState::GoToEatPos)
        );

        let mut carrying = Scene::default();
        carrying.restaurant.pos = Some(Pos::new(1, 64, 1));
        carrying.restaurant.we_are_inside = true;
        carrying.food.best_slot = Some(0);
        assert_eq!(
            step(EatState::WaitForFood, &mut carrying),
            Step::Next(EatState::Eat)
        );

        let mut full = Scene::default();
        full.restaurant.pos = Some(Pos::new(1, 64, 1));
        full.restaurant.we_are_inside = true;
        full.citizen.saturation = 20.0;
        assert_eq!(
            step(EatState::WaitForFood, &mut full),
            Step::Next(EatState::Done)
        );
        assert!(full.memory.just_ate);
    }

    #[test]
    fn waiting_outside_the_restaurant_walks_in_first() {
        // `:362-365`.
        let mut scene = Scene::default();
        scene.memory.restaurant_pos = Some(Pos::new(1, 64, 1));
        assert_eq!(
            step(EatState::WaitForFood, &mut scene),
            Step::Next(EatState::SearchRestaurant)
        );
        let mut outside = Scene::default();
        outside.restaurant.pos = Some(Pos::new(1, 64, 1));
        assert_eq!(
            step(EatState::WaitForFood, &mut outside),
            Step::Next(EatState::GoToRestaurant)
        );
    }

    #[test]
    fn a_guard_serves_itself_at_night_instead_of_waiting_for_a_cook() {
        // `:296-303`: the guard branch needs the timeout *and* the night.
        let mut scene = Scene::default();
        scene.citizen.job = JobKind::Guard;
        scene.citizen.daytime = false;
        scene.memory.eat_pos = None;
        assert_eq!(
            step(EatState::GoToEatPos, &mut scene),
            Step::Next(EatState::GetFoodYourself)
        );
        // A worker at night does not: it waits out the two minutes.
        let mut worker = Scene::default();
        worker.citizen.job = JobKind::Worker;
        worker.citizen.daytime = false;
        assert_eq!(
            step(EatState::GoToEatPos, &mut worker),
            Step::Next(EatState::GoToEatPos),
            "one tick of waiting, not yet two minutes"
        );
        assert_eq!(worker.memory.waiting_ticks, 1);
    }

    #[test]
    fn serving_itself_without_a_restaurant_goes_back_to_the_search() {
        // `:252-255`.
        let mut scene = Scene::default();
        assert_eq!(
            step(EatState::GetFoodYourself, &mut scene),
            Step::Next(EatState::SearchRestaurant)
        );
    }

    #[test]
    fn the_complaint_is_about_quality_then_about_raw_food() {
        // `:530-544`.
        let mut scene = Scene::default();
        scene.food.eatable = 2;
        assert_eq!(
            complaint(&scene),
            Some("com.minecolonies.coremod.gui.chat.betterfood")
        );
        scene.citizen.child = true;
        assert_eq!(
            complaint(&scene),
            Some("com.minecolonies.coremod.gui.chat.betterfoodchildren")
        );
        let mut raw = Scene::default();
        raw.food.cookable = 1;
        assert_eq!(
            complaint(&raw),
            Some("com.minecolonies.coremod.gui.chat.rawfood")
        );
        let mut nothing = Scene::default();
        assert_eq!(complaint(&nothing), None);
        let mut fine = scene_with_food(0);
        assert_eq!(complaint(&fine), None, "a best food is not a complaint");
        assert!(has_food(&mut fine));
    }

    #[test]
    fn done_hands_the_citizen_back_and_idle_stays_put() {
        // `:153` and the absence of a transition out of `CitizenAIState.IDLE`.
        let mut scene = Scene::default();
        assert_eq!(step(EatState::Done, &mut scene), Step::Next(EatState::Idle));
        assert_eq!(step(EatState::Idle, &mut scene), Step::Stayed);
    }

    #[test]
    fn the_constants_keep_their_provenance() {
        assert_eq!(MINUTES_WAITING_TIME.v(), 2);
        assert_eq!(REQUIRED_TIME_TO_EAT.v(), 5);
        assert_eq!(EAT_POS_WALK_TIMEOUT.v(), 400);
        assert_eq!(SIT_TICKS.v(), 1200);
        assert_eq!(RESTAURANT_LIMIT.v(), 2.5);
        assert!(SIT_TICKS.source.contains("EntityAIEatTask.java:308"));
        // Two constants of this class are declared and unread: they are kept, and
        // named as such, rather than dropped silently.
        assert_eq!(GET_YOURSELF_SATURATION.v(), 30);
        assert_eq!(MINUTES_BETWEEN_FOOD_CHECKS.v(), 5);
    }
}
