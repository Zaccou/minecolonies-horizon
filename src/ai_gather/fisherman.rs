//! The fisherman: a pond, an angle, a rod.
//!
//! Java reference: `core/entity/ai/workers/production/agriculture/EntityAIWorkFisherman.java`
//! — the constructor's `registerTargets` (`:168-176`), the six `AITarget` bodies
//! (`:191-544`) and the numbers each one reads — plus `api/util/Pond.java` for what a
//! pond is (the three `PondState` values and the 5x2 requirement).
//!
//! **What a decision is here.** Nothing walks and nothing throws: the water search is a
//! path job (`PathJobFindWater`, `:389-402`), the bobber is an entity
//! (`NewBobberEntity`, `:560-565`) and the swing is an animation. A state *decides*
//! which of those the caller should do next, from [`FishermanFacts`] and
//! [`GatherMemory`] alone — the same shape as [`super::lumberjack`] and
//! [`super::farmer`].
//!
//! **Three Java calls have no landing place in a fact, and are named rather than
//! hidden.**
//!
//! 1. `job.setWater(null)` / `job.removeFromPonds(...)` (`:334-336`, `:345-347`,
//!    `:492`) — the port cannot write a fact. A decision that Java answers with
//!    "drop the pond, go and search" returns that state, and the caller clears
//!    `Water` on the scene; the state it is told is the one that makes it search.
//! 2. `world.getBlockState(worker.blockPosition()).liquid()` and
//!    `BlockUtils.isAnySolid(...below the water)` (`:340-352`) are
//!    [`FishermanFacts::standing_in_liquid`] and [`FishermanFacts::pond_floor_solid`],
//!    so `tryDifferentAngles` keeps both branches — including the two angles a
//!    fisherman standing in his own pond counts.
//! 3. `stuckCounter` starts at `3` in Java (`:158`) and the port's
//!    [`GatherMemory::default`] starts it at `0`: a `GatherScene` built from
//!    [`super::GatherScene::new`] takes three more stuck answers before it retrieves.
//!    The difference is named here rather than papered over by a constructor the AI
//!    does not own.
//!
//! **The numbers.** `MAX_FISHES_IN_INV` is what `mod.rs` asks this module for
//! (`getActionsDoneUntilDumping`, `:225-229`); the rest are the delays and chances of
//! the state table. Not one is a round guess: each carries the line it came from.

use crate::ai::facts::WorldFacts;
use crate::ai::state::WorkerState;

use super::facts::{FishermanFacts, GatherMemory, Hook, PathState, PondState};
use super::state::{GatherRefusal, GatherState};
use super::{at_hut, gather_action, Decision, TICKS_SECOND};

/// `EntityAIWorkFisherman.MAX_PONDS` (`:85`): how many ponds the job remembers before
/// it stops searching and fishes a known one (`findWater`, `:374`).
pub const MAX_PONDS: u32 = 20;

/// `FISHING_SKILL_CHANCE` (`:90`): the base of the `nextInt` bound in
/// `testRandomChance` (`:592`).
pub const FISHING_SKILL_CHANCE: u32 = 10;

/// `CHANCE` (`:95`): the draw up to which the fisherman waits instead of throwing.
pub const CHANCE: u32 = 2;

/// `MIN_DISTANCE_TO_WATER` (`:100`): how close water has to be before the rod is
/// thrown (`isReadyToFish`, `:612`).
pub const MIN_DISTANCE_TO_WATER: i32 = 3;

/// `MAX_FISHES_IN_INV` (`:105`): `getActionsDoneUntilDumping` (`:225-229`).
pub const MAX_FISHES_IN_INV: u32 = 10;

/// `MAX_ROTATIONS` (`:110`): how many angles a pond gets before it is dropped
/// (`:332-338`).
pub const MAX_ROTATIONS: u32 = 6;

/// `SEARCH_RANGE` (`:115`): the radius `setRandomWater` searches in (`:424`);
/// `findNewWater` searches three times as far (`:443`).
pub const SEARCH_RANGE: i32 = 50;

/// `CHANCE_NEW_POND` (`:120`): the chance a catch sends the fisherman to another pond
/// (`:490`).
pub const CHANCE_NEW_POND: f64 = 0.05;

/// `FISHING_TIMEOUT` (`:125`): the delay `testRandomChance` sets (`:591`).
pub const FISHING_TIMEOUT: u32 = 5;

/// `LURE_SPEED_DIVIDER` (`:130`): the divisor of the lure bonus `setAngler` gets
/// (`:563`), applied where the hook is created — not by a decision here.
pub const LURE_SPEED_DIVIDER: u32 = 25;

/// `testRandomChance`'s bound (`:592`): `FISHING_SKILL_CHANCE + secondary / 5`.
///
/// The draw itself is a fact ([`FishermanFacts::draw_int`]), so the caller has to draw
/// with this bound for the distribution to be Java's — `MANQUANT.md` §5 names this
/// function as that contract.
pub fn random_chance_bound(secondary_skill: u32) -> u32 {
    FISHING_SKILL_CHANCE + secondary_skill / 5 // `:592`
}

/// `testRandomChance` (`:588-594`): the tick is delayed by [`FISHING_TIMEOUT`] and the
/// caller-supplied draw says whether the fisherman waits.
///
/// `trade.draw_int` **is** Java's `nextInt(`[`random_chance_bound`]`)`: the secondary
/// skill bounds the draw, and it is a fact the caller fills, so a tick replays.
pub fn test_random_chance(trade: &FishermanFacts, memory: &mut GatherMemory) -> bool {
    memory.base.delay = FISHING_TIMEOUT as i32; // `setDelay(FISHING_TIMEOUT)` `:591`
    trade.draw_int <= CHANCE // `:593`
}

/// `isFishHookStuck` (`:575-579`): the bobber is on land, ready to be pulled in, dead,
/// or holding an entity.
pub fn is_fish_hook_stuck(hook: &Hook) -> bool {
    (!hook.in_water && (hook.on_ground || hook.should_stop)) || !hook.alive || hook.caught_entity
}

/// `caughtFish` (`:654-673`): a hook that is out, ready, and passes the skill draw.
///
/// The retrieve itself is `retrieveRod` (`:678-688`) — the caller's, because it swings,
/// damages the rod and spawns the loot (`generateBonusLoot`, `:693-721`).
pub fn caught_fish(trade: &FishermanFacts, memory: &mut GatherMemory) -> bool {
    let Some(hook) = trade.hook else {
        return false; // `:656-659`
    };
    if !hook.ready_to_catch {
        return false; // `:660-663`
    }
    if test_random_chance(trade, memory) {
        return false; // `:665-668`
    }
    true // `:670-672`
}

/// `isReadyToFish` (`:601-627`), as the three early answers it can give.
///
/// `None` means "all three requirements hold": a rod in the inventory, water within
/// [`MIN_DISTANCE_TO_WATER`], and that rod in the main hand. `Some(decision)` is the
/// answer Java returns from `doFishing` (`:479-483`) after it has equipped the rod or
/// asked for one.
fn is_ready_to_fish(trade: &FishermanFacts) -> Option<Decision> {
    if trade.rod_slot.is_none() {
        // `:604-609` — `getRodSlot() == -1`: empty the hand and go back to PREPARING.
        return Some(Decision::go_after(
            GatherState::Base(WorkerState::Preparing),
            GatherRefusal::NoRod,
        ));
    }
    if !trade.water_in_range {
        // `:611-615` — no water within three blocks: walk closer.
        return Some(Decision::go_after(
            GatherState::FishermanWalkingToWater,
            GatherRefusal::WaterTooFar,
        ));
    }
    if !trade.rod_equipped {
        // `:617-625` — `equipRod(); return getState();`: the caller puts the rod in the
        // hand, the state does not move.
        return Some(Decision::refuse(GatherRefusal::RodNotEquipped));
    }
    None
}

/// `startWorkingAtOwnBuilding` (`:191-198`): walk to the hut, then PREPARING.
pub fn start_working(
    facts: &WorldFacts,
    _trade: &FishermanFacts,
    _memory: &mut GatherMemory,
) -> Decision {
    if !at_hut(facts) {
        return Decision::stay(); // `:193-196` — `return getState()`
    }
    Decision::go(GatherState::Base(WorkerState::Preparing)) // `:197`
}

/// `prepareForFishing` (`:205-215`): ask for a fishing rod, or go to the water.
///
/// Java's `checkForToolOrWeapon` (`:207`) is the request: when it answers `true` a
/// request is out and the state waits. `rod_slot` is the same registry query
/// `getRodSlot` makes (`:642-646`), so `None` is that answer.
pub fn prepare_for_fishing(
    _facts: &WorldFacts,
    trade: &FishermanFacts,
    _memory: &mut GatherMemory,
) -> Decision {
    if trade.rod_slot.is_none() {
        return Decision::refuse(GatherRefusal::NoRod); // `:207-212`
    }
    Decision::go(GatherState::FishermanWalkingToWater) // `:214`
}

/// `getToWater` (`:278-308`): no pond means search, no arrival means keep walking, a
/// suboptimal pond means complain and search, otherwise check the water.
pub fn get_to_water(
    _facts: &WorldFacts,
    trade: &FishermanFacts,
    _memory: &mut GatherMemory,
) -> Decision {
    if trade.water.is_none() {
        return Decision::go(GatherState::FishermanSearchingWater); // `:280-282`
    }
    if !trade.at_water {
        return Decision::refuse(GatherRefusal::NotAtWater); // `:285-288`
    }
    if trade.pond_state == PondState::Suboptimal {
        // `:295-304` — `triggerInteraction(SUBOPTIMAL_POND, …, lastPathResult.pond)`.
        return Decision::go_after(
            GatherState::FishermanSearchingWater,
            GatherRefusal::SuboptimalPond,
        );
    }
    Decision::go(GatherState::FishermanCheckWater) // `:307`
}

/// `tryDifferentAngles` (`:326-359`): count the angle, and drop the pond once
/// [`MAX_ROTATIONS`] of them are used up.
///
/// The two world questions of `:340-352` are facts:
/// [`FishermanFacts::standing_in_liquid`] and [`FishermanFacts::pond_floor_solid`]. A
/// fisherman standing in the pond itself counts two angles — one for the water
/// (`:351`) and one for the turn (`:357`) — and a pond with nothing solid under it is
/// forgotten outright (`:342-348`).
pub fn try_different_angles(
    _facts: &WorldFacts,
    trade: &FishermanFacts,
    memory: &mut GatherMemory,
) -> Decision {
    if trade.water.is_none() {
        return Decision::go(GatherState::FishermanSearchingWater); // `:328-331`
    }
    if memory.fisherman.executed_rotations >= MAX_ROTATIONS {
        memory.fisherman.executed_rotations = 0; // `:335`
        return Decision::go_after(
            GatherState::FishermanSearchingWater,
            GatherRefusal::RotationsExhausted,
        ); // `:332-338` — `removeFromPonds(job.getWater()); job.setWater(null)`
    }
    if trade.standing_in_liquid {
        if !trade.pond_floor_solid {
            // `:342-348` — no floor under the pond: forget it and go back to the hut.
            memory.fisherman.executed_rotations = 0;
            return Decision::go(GatherState::Base(WorkerState::StartWorking));
        }
        memory.fisherman.executed_rotations += 1; // `:351`
    }
    memory.fisherman.executed_rotations += 1; // `:357`
    Decision::go(GatherState::FishermanStartFishing) // `:358`
}

/// `findWater` (`:366-379`): fish a known pond once the list is full, search otherwise.
pub fn find_water(
    _facts: &WorldFacts,
    trade: &FishermanFacts,
    memory: &mut GatherMemory,
) -> Decision {
    memory.fisherman.executed_rotations = 0; // `:370`
    let full = trade.ponds >= MAX_PONDS; // `:374`
    let last_had_no_pond =
        memory.fisherman.has_last_result && trade.last_pond_missing && trade.ponds > 0; // `:374`
    if full || last_had_no_pond {
        return set_random_water(trade, memory); // `:374-377`
    }
    find_new_water(trade, memory) // `:379`
}

/// `setRandomWater` (`:409-431`): no pond at all means search (and complain when the
/// last search failed), otherwise fish a pond from the list.
///
/// Java picks with `job.getPonds().get(worker.getRandom().nextInt(size))` (`:429`).
/// The list lives in the job, not in a fact: the scene arrives with `water` already
/// picked, or with `None` when the pick has not happened yet — which is the
/// [`GatherRefusal::NoWater`] answer below.
pub fn set_random_water(trade: &FishermanFacts, memory: &mut GatherMemory) -> Decision {
    if trade.ponds == 0 {
        let too_far = (trade.path_failed && !memory.fisherman.has_last_result)
            || (memory.fisherman.has_last_result
                && trade.last_pond_missing
                && trade.path != PathState::Cancelled); // `:413-420`
        // `:422-425` — `pathResult == null || !pathResult.isInProgress()`: the search is
        // started again, which is the caller's job (`searchWater`, `:389-402`).
        if matches!(trade.path, PathState::None | PathState::Done) {
            memory.fisherman.searching = true;
        }
        let decision = Decision::go(GatherState::Base(WorkerState::StartWorking)); // `:427`
        return if too_far {
            decision.because(GatherRefusal::WaterTooFar)
        } else {
            decision
        };
    }
    if trade.water.is_some() {
        return Decision::go(GatherState::FishermanCheckWater); // `:430`
    }
    Decision::go_after(GatherState::FishermanSearchingWater, GatherRefusal::NoWater)
}

/// `findNewWater` (`:439-467`): start the search, take what it found, or give up on a
/// cancelled one.
pub fn find_new_water(trade: &FishermanFacts, memory: &mut GatherMemory) -> Decision {
    if trade.path == PathState::None {
        memory.fisherman.searching = true; // `:441-444` — the caller starts `searchWater`
        return Decision::stay();
    }
    if trade.path_failed {
        return set_random_water(trade, memory); // `:446-449`
    }
    if trade.path_reaching {
        // `:452-455` — `job.setWater(new Tuple<>(pond, parent)); job.addToPonds(…)`: both
        // are the job's, and the scene carries the pair it landed on.
        memory.fisherman.has_last_result = true; // `:457`
        memory.fisherman.searching = false; // `:458` — `pathResult = null`
        return Decision::go(GatherState::FishermanCheckWater); // `:459`
    }
    if trade.path == PathState::Cancelled {
        memory.fisherman.searching = false; // `:462` — `pathResult = null`
        return Decision::go(GatherState::Base(WorkerState::Preparing)); // `:464`
    }
    Decision::stay() // `:466` — `return getState()`
}

/// `doFishing` (`:476-498`): the readiness checks, the catch, then the hook.
pub fn do_fishing(
    _facts: &WorldFacts,
    trade: &FishermanFacts,
    memory: &mut GatherMemory,
) -> Decision {
    if let Some(not_ready) = is_ready_to_fish(trade) {
        return not_ready; // `:479-483`
    }
    if caught_fish(trade, memory) {
        // `:487-488` — the sound, `incrementActionsDoneAndDecSaturation()` and the XP
        // orbs are the caller's: a catch is not a state change by itself.
        if trade.draw_double < CHANCE_NEW_POND {
            // `:490-494` — `job.setWater(null)`: the caller drops the pond.
            return Decision::go(GatherState::FishermanSearchingWater);
        }
        return Decision::go(GatherState::FishermanWalkingToWater); // `:495`
    }
    throw_or_retrieve_hook(trade, memory) // `:497`
}

/// `throwOrRetrieveHook` (`:513-544`): wait on the skill draw, throw the rod, or pull
/// it back in when it is stuck.
pub fn throw_or_retrieve_hook(trade: &FishermanFacts, memory: &mut GatherMemory) -> Decision {
    let Some(hook) = trade.hook else {
        // `:515-523` — no bobber out. Only sometimes does the fisherman get to throw.
        if test_random_chance(trade, memory) {
            return Decision::stay(); // `:518-521`
        }
        return Decision::stay(); // `:522` — `throwRod()`: the rod is the caller's
    };
    if is_fish_hook_stuck(&hook) {
        if memory.fisherman.stuck_counter > 3 {
            memory.fisherman.stuck_counter = 0; // `:531`
            return Decision::go_after(
                GatherState::FishermanWalkingToWater,
                GatherRefusal::HookStuck,
            ); // `:532-533` — `retrieveRod()`
        }
        memory.fisherman.stuck_counter += 1; // `:535`
        return Decision::stay();
    }
    memory.fisherman.stuck_counter = 0; // `:539`
    Decision::stay() // `:541-543` — `setInUse(); return getState();`
}

gather_action!(start_working_action, Fisherman, start_working);
gather_action!(prepare_action, Fisherman, prepare_for_fishing);
gather_action!(get_to_water_action, Fisherman, get_to_water);
gather_action!(angles_action, Fisherman, try_different_angles);
gather_action!(find_water_action, Fisherman, find_water);
gather_action!(fishing_action, Fisherman, do_fishing);

/// Register the six targets of `EntityAIWorkFisherman.java:170-175`.
///
/// The seventh of the constructor's list, `IDLE → START_WORKING` (`:169`), is
/// [`super::register_handover`]'s: it is the same line in all four trades and the base
/// class is where Java writes it (`AbstractEntityAIBasic.java:204-260`).
pub fn register(machine: &mut crate::ai::state::Machine<GatherState, super::GatherScene>) {
    // `:170` — `new AITarget(START_WORKING, this::startWorkingAtOwnBuilding, TICKS_SECOND)`.
    super::add_target(
        machine,
        GatherState::Base(WorkerState::StartWorking),
        "startWorkingAtOwnBuilding",
        "EntityAIWorkFisherman.java:170",
        TICKS_SECOND,
        start_working_action,
    );
    // `:171` — `new AITarget(PREPARING, this::prepareForFishing, TICKS_SECOND)`.
    super::add_target(
        machine,
        GatherState::Base(WorkerState::Preparing),
        "prepareForFishing",
        "EntityAIWorkFisherman.java:171",
        TICKS_SECOND,
        prepare_action,
    );
    // `:172` — `new AITarget(FISHERMAN_CHECK_WATER, this::tryDifferentAngles, 1)`.
    super::add_target(
        machine,
        GatherState::FishermanCheckWater,
        "tryDifferentAngles",
        "EntityAIWorkFisherman.java:172",
        1,
        angles_action,
    );
    // `:173` — `new AITarget(FISHERMAN_SEARCHING_WATER, this::findWater, TICKS_SECOND)`.
    super::add_target(
        machine,
        GatherState::FishermanSearchingWater,
        "findWater",
        "EntityAIWorkFisherman.java:173",
        TICKS_SECOND,
        find_water_action,
    );
    // `:174` — `new AITarget(FISHERMAN_WALKING_TO_WATER, this::getToWater, TICKS_SECOND)`.
    super::add_target(
        machine,
        GatherState::FishermanWalkingToWater,
        "getToWater",
        "EntityAIWorkFisherman.java:174",
        TICKS_SECOND,
        get_to_water_action,
    );
    // `:175` — `new AITarget(FISHERMAN_START_FISHING, this::doFishing, TICKS_SECOND)`.
    super::add_target(
        machine,
        GatherState::FishermanStartFishing,
        "doFishing",
        "EntityAIWorkFisherman.java:175",
        TICKS_SECOND,
        fishing_action,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::facts::Pos;
    use crate::ai_gather::facts::{GatherJob, GatherScene, Water};

    /// The water pair Java keeps as `job.getWater()`: the block the hook goes at, and
    /// where the fisherman stands (`:317`, `:356`).
    fn water() -> Water {
        Water {
            target: Pos::new(8, 63, 8),
            stand: Pos::new(6, 64, 8),
        }
    }

    fn trade() -> FishermanFacts {
        FishermanFacts {
            ponds: 1,
            water: Some(water()),
            at_water: true,
            water_in_range: true,
            rod_slot: Some(3),
            rod_equipped: true,
            ..FishermanFacts::default()
        }
    }

    fn scene() -> GatherScene {
        GatherScene::new(
            WorldFacts::new(),
            GatherJob::Fisherman(FishermanFacts::default()),
        )
    }

    #[test]
    fn the_numbers_are_the_java_ones() {
        // Each of these is a `private static final int` of `EntityAIWorkFisherman`.
        assert_eq!(MAX_FISHES_IN_INV, 10, ":105");
        assert_eq!(MAX_PONDS, 20, ":85");
        assert_eq!(MAX_ROTATIONS, 6, ":110");
        assert_eq!(MIN_DISTANCE_TO_WATER, 3, ":100");
        assert_eq!(SEARCH_RANGE, 50, ":115");
        assert_eq!(FISHING_TIMEOUT, 5, ":125");
        assert_eq!(CHANCE, 2, ":95");
        assert_eq!(FISHING_SKILL_CHANCE, 10, ":90");
        assert_eq!(CHANCE_NEW_POND, 0.05, ":120");
        assert_eq!(LURE_SPEED_DIVIDER, 25, ":130");
        // `:592` — the draw's bound grows by one every five secondary-skill points, and
        // the caller has to draw with it (`MANQUANT.md` §5).
        assert_eq!(random_chance_bound(0), FISHING_SKILL_CHANCE);
        assert_eq!(random_chance_bound(50), 20);
        assert_eq!(random_chance_bound(4), 10, "one point short of the next step");
        // `getActionsDoneUntilDumping` answers `MAX_FISHES_IN_INV` (`:225-229`), which is
        // what `mod.rs` asks the trade for.
        assert_eq!(
            super::super::GatherTrade::Fisherman.actions_until_dumping(&scene()),
            MAX_FISHES_IN_INV
        );
    }

    #[test]
    fn the_skill_draw_waits_up_to_two_and_sets_the_timeout() {
        // `:588-594` — `setDelay(FISHING_TIMEOUT); return draw <= CHANCE;`.
        let mut memory = GatherMemory::default();
        let waiting = FishermanFacts {
            draw_int: 2,
            ..FishermanFacts::default()
        };
        assert!(test_random_chance(&waiting, &mut memory));
        assert_eq!(memory.base.delay, FISHING_TIMEOUT as i32);

        let throwing = FishermanFacts {
            draw_int: 3,
            ..FishermanFacts::default()
        };
        assert!(!test_random_chance(&throwing, &mut memory));
    }

    #[test]
    fn a_hook_is_stuck_on_land_dead_or_holding_an_entity() {
        // `:577-578`.
        let fine = Hook {
            in_water: true,
            alive: true,
            ..Hook::default()
        };
        assert!(!is_fish_hook_stuck(&fine));

        let on_land = Hook {
            on_ground: true,
            alive: true,
            ..Hook::default()
        };
        assert!(is_fish_hook_stuck(&on_land), "on ground, out of water");

        let stopping = Hook {
            should_stop: true,
            alive: true,
            ..Hook::default()
        };
        assert!(is_fish_hook_stuck(&stopping));

        let dead = Hook {
            in_water: true,
            ..Hook::default()
        };
        assert!(is_fish_hook_stuck(&dead), "not alive");

        let holding = Hook {
            in_water: true,
            alive: true,
            caught_entity: true,
            ..Hook::default()
        };
        assert!(is_fish_hook_stuck(&holding), "an entity is on the hook");

        let in_water = Hook {
            in_water: true,
            on_ground: true,
            should_stop: true,
            alive: true,
            ..Hook::default()
        };
        assert!(
            !is_fish_hook_stuck(&in_water),
            "in water wins over ground and shouldStop"
        );
    }

    #[test]
    fn a_catch_needs_a_ready_hook_and_a_passing_draw() {
        // `:654-673`.
        let mut memory = GatherMemory::default();
        assert!(!caught_fish(&trade(), &mut memory), "no hook out");

        let ready = FishermanFacts {
            hook: Some(Hook {
                ready_to_catch: true,
                ..Hook::default()
            }),
            draw_int: 9,
            ..trade()
        };
        assert!(caught_fish(&ready, &mut memory));

        let unlucky = FishermanFacts {
            draw_int: 0,
            ..ready
        };
        assert!(!caught_fish(&unlucky, &mut memory), "the draw says wait");

        let not_ready = FishermanFacts {
            hook: Some(Hook::default()),
            ..trade()
        };
        assert!(!caught_fish(&not_ready, &mut memory));
    }

    #[test]
    fn preparing_asks_for_a_rod_before_the_water() {
        // `:205-215`.
        let mut memory = GatherMemory::default();
        let no_rod = FishermanFacts {
            rod_slot: None,
            ..trade()
        };
        let decision = prepare_for_fishing(&WorldFacts::new(), &no_rod, &mut memory);
        assert_eq!(decision.next, None);
        assert_eq!(decision.refusal, Some(GatherRefusal::NoRod));

        let with_rod = prepare_for_fishing(&WorldFacts::new(), &trade(), &mut memory);
        assert_eq!(decision_next(with_rod), Some(GatherState::FishermanWalkingToWater));
    }

    /// `Decision::next` as a plain `Option`, so a test reads like the Java `return`.
    fn decision_next(decision: Decision) -> Option<GatherState> {
        decision.next
    }

    #[test]
    fn walking_to_water_searches_first_arrives_then_checks() {
        // `:278-308`.
        let mut memory = GatherMemory::default();
        let nowhere = FishermanFacts {
            water: None,
            ..trade()
        };
        assert_eq!(
            decision_next(get_to_water(&WorldFacts::new(), &nowhere, &mut memory)),
            Some(GatherState::FishermanSearchingWater)
        );

        let walking = FishermanFacts {
            at_water: false,
            ..trade()
        };
        let decision = get_to_water(&WorldFacts::new(), &walking, &mut memory);
        assert_eq!(decision.next, None, "`return getState()`");
        assert_eq!(decision.refusal, Some(GatherRefusal::NotAtWater));

        let suboptimal = FishermanFacts {
            pond_state: PondState::Suboptimal,
            ..trade()
        };
        let decision = get_to_water(&WorldFacts::new(), &suboptimal, &mut memory);
        assert_eq!(
            decision.next,
            Some(GatherState::FishermanSearchingWater),
            "a suboptimal pond is dropped"
        );
        assert_eq!(decision.refusal, Some(GatherRefusal::SuboptimalPond));

        assert_eq!(
            decision_next(get_to_water(&WorldFacts::new(), &trade(), &mut memory)),
            Some(GatherState::FishermanCheckWater)
        );
    }

    #[test]
    fn six_angles_are_the_most_a_pond_gets() {
        // `:326-359`.
        let mut memory = GatherMemory::default();
        memory.fisherman.executed_rotations = MAX_ROTATIONS - 1;
        let decision = try_different_angles(&WorldFacts::new(), &trade(), &mut memory);
        assert_eq!(decision.next, Some(GatherState::FishermanStartFishing));
        assert_eq!(memory.fisherman.executed_rotations, MAX_ROTATIONS);

        let decision = try_different_angles(&WorldFacts::new(), &trade(), &mut memory);
        assert_eq!(decision.next, Some(GatherState::FishermanSearchingWater));
        assert_eq!(decision.refusal, Some(GatherRefusal::RotationsExhausted));
        assert_eq!(memory.fisherman.executed_rotations, 0, "`executedRotations = 0`");

        let nowhere = FishermanFacts {
            water: None,
            ..trade()
        };
        assert_eq!(
            decision_next(try_different_angles(&WorldFacts::new(), &nowhere, &mut memory)),
            Some(GatherState::FishermanSearchingWater)
        );
    }

    #[test]
    fn searching_resets_the_angles_and_fishes_a_known_pond_when_the_list_is_full() {
        // `:366-379`.
        let mut memory = GatherMemory::default();
        memory.fisherman.executed_rotations = 4;
        let full = FishermanFacts {
            ponds: MAX_PONDS,
            ..trade()
        };
        let decision = find_water(&WorldFacts::new(), &full, &mut memory);
        assert_eq!(memory.fisherman.executed_rotations, 0, "`:370`");
        assert_eq!(decision.next, Some(GatherState::FishermanCheckWater));

        // A last search that found no pond, with ponds to fall back on, also picks one
        // (`:374`).
        let mut memory = GatherMemory::default();
        memory.fisherman.has_last_result = true;
        let empty_last = FishermanFacts {
            last_pond_missing: true,
            ponds: 2,
            ..trade()
        };
        assert_eq!(
            decision_next(find_water(&WorldFacts::new(), &empty_last, &mut memory)),
            Some(GatherState::FishermanCheckWater)
        );
    }

    #[test]
    fn no_pond_at_all_sends_the_fisherman_back_to_start_working() {
        // `:411-428`.
        let mut memory = GatherMemory::default();
        let none = FishermanFacts {
            ponds: 0,
            path: PathState::None,
            ..trade()
        };
        let decision = set_random_water(&none, &mut memory);
        assert_eq!(decision.next, Some(GatherState::Base(WorkerState::StartWorking)));
        assert!(memory.fisherman.searching, "the caller starts `searchWater`");

        // A search that failed, with no earlier result: the player is told the water is
        // too far (`:413-420`).
        let mut memory = GatherMemory::default();
        let failed = FishermanFacts {
            ponds: 0,
            path: PathState::Done,
            path_failed: true,
            ..trade()
        };
        let decision = set_random_water(&failed, &mut memory);
        assert_eq!(decision.refusal, Some(GatherRefusal::WaterTooFar));

        // A pond in the list whose water the scene has not picked yet cannot be fished.
        let mut memory = GatherMemory::default();
        let unpicked = FishermanFacts {
            water: None,
            ..trade()
        };
        let decision = set_random_water(&unpicked, &mut memory);
        assert_eq!(decision.next, Some(GatherState::FishermanSearchingWater));
        assert_eq!(decision.refusal, Some(GatherRefusal::NoWater));
    }

    #[test]
    fn a_search_is_started_taken_or_given_up_on() {
        // `:439-467`.
        let mut memory = GatherMemory::default();
        let idle = FishermanFacts {
            path: PathState::None,
            ..trade()
        };
        assert_eq!(
            decision_next(find_new_water(&idle, &mut memory)),
            None,
            "`return getState()` while the search starts"
        );
        assert!(memory.fisherman.searching);

        let mut memory = GatherMemory::default();
        let reaching = FishermanFacts {
            path: PathState::Done,
            path_reaching: true,
            pond: Some(Pos::new(8, 63, 8)),
            pond_stand: Some(Pos::new(6, 64, 8)),
            ..trade()
        };
        assert_eq!(
            decision_next(find_new_water(&reaching, &mut memory)),
            Some(GatherState::FishermanCheckWater)
        );
        assert!(memory.fisherman.has_last_result, "`:457`");
        assert!(!memory.fisherman.searching, "`:458`");

        let mut memory = GatherMemory::default();
        memory.fisherman.searching = true;
        let cancelled = FishermanFacts {
            path: PathState::Cancelled,
            ..trade()
        };
        assert_eq!(
            decision_next(find_new_water(&cancelled, &mut memory)),
            Some(GatherState::Base(WorkerState::Preparing)),
            "`:461-465`"
        );
        assert!(!memory.fisherman.searching);

        let mut memory = GatherMemory::default();
        let failed = FishermanFacts {
            path: PathState::Done,
            path_failed: true,
            ponds: 1,
            ..trade()
        };
        assert_eq!(
            decision_next(find_new_water(&failed, &mut memory)),
            Some(GatherState::FishermanCheckWater),
            "a failed search falls back to a known pond"
        );
    }

    #[test]
    fn fishing_reports_the_first_requirement_that_does_not_hold() {
        // `:601-627` and `:479-483`.
        let mut memory = GatherMemory::default();
        let no_rod = FishermanFacts {
            rod_slot: None,
            ..trade()
        };
        let decision = do_fishing(&WorldFacts::new(), &no_rod, &mut memory);
        assert_eq!(decision.next, Some(GatherState::Base(WorkerState::Preparing)));
        assert_eq!(decision.refusal, Some(GatherRefusal::NoRod));

        let far = FishermanFacts {
            water_in_range: false,
            ..trade()
        };
        let decision = do_fishing(&WorldFacts::new(), &far, &mut memory);
        assert_eq!(decision.next, Some(GatherState::FishermanWalkingToWater));
        assert_eq!(decision.refusal, Some(GatherRefusal::WaterTooFar));

        let unequipped = FishermanFacts {
            rod_equipped: false,
            ..trade()
        };
        let decision = do_fishing(&WorldFacts::new(), &unequipped, &mut memory);
        assert_eq!(decision.next, None, "`return getState()`");
        assert_eq!(decision.refusal, Some(GatherRefusal::RodNotEquipped));
    }

    #[test]
    fn a_catch_usually_means_fishing_again_and_sometimes_a_new_pond() {
        // `:485-495`.
        let mut memory = GatherMemory::default();
        let catch = FishermanFacts {
            hook: Some(Hook {
                ready_to_catch: true,
                in_water: true,
                alive: true,
                ..Hook::default()
            }),
            draw_int: 9,
            draw_double: 0.5,
            ..trade()
        };
        assert_eq!(
            decision_next(do_fishing(&WorldFacts::new(), &catch, &mut memory)),
            Some(GatherState::FishermanWalkingToWater)
        );

        let new_pond = FishermanFacts {
            draw_double: 0.01,
            ..catch
        };
        assert_eq!(
            decision_next(do_fishing(&WorldFacts::new(), &new_pond, &mut memory)),
            Some(GatherState::FishermanSearchingWater),
            "`job.setWater(null)` at `:492`"
        );
    }

    #[test]
    fn a_stuck_hook_is_retrieved_after_four_stuck_answers() {
        // `:527-540`.
        let mut memory = GatherMemory::default();
        let stuck = FishermanFacts {
            hook: Some(Hook {
                on_ground: true,
                alive: true,
                ..Hook::default()
            }),
            ..trade()
        };
        for expected in 1..=4 {
            let decision = throw_or_retrieve_hook(&stuck, &mut memory);
            assert_eq!(decision.next, None, "still waiting after {expected}");
            assert_eq!(memory.fisherman.stuck_counter, expected);
        }
        let decision = throw_or_retrieve_hook(&stuck, &mut memory);
        assert_eq!(decision.next, Some(GatherState::FishermanWalkingToWater));
        assert_eq!(decision.refusal, Some(GatherRefusal::HookStuck));
        assert_eq!(memory.fisherman.stuck_counter, 0, "`:531`");

        // A hook in the water resets the counter (`:539`).
        memory.fisherman.stuck_counter = 2;
        let fine = FishermanFacts {
            hook: Some(Hook {
                in_water: true,
                alive: true,
                ..Hook::default()
            }),
            ..trade()
        };
        assert_eq!(throw_or_retrieve_hook(&fine, &mut memory).next, None);
        assert_eq!(memory.fisherman.stuck_counter, 0);
    }

    #[test]
    fn the_table_is_the_seven_targets_of_the_constructor() {
        // `EntityAIWorkFisherman.java:169-175`. The first is `register_handover`'s, so
        // this module's own list is the six below it.
        use super::super::{states_of, GatherAi, GatherTrade};

        let mut ai = GatherAi::new(GatherTrade::Fisherman);
        assert_eq!(ai.state(), GatherState::Base(WorkerState::Idle));
        let mut scene = ai.scene(WorldFacts::new());
        let outcome = ai.tick(&mut scene);
        assert_eq!(outcome.goal(), GatherState::Base(WorkerState::StartWorking));
        assert!(
            outcome.why_not().is_none(),
            "the fisher's facts are the scene's, so no NoFactsForTrade"
        );

        let states = states_of(GatherTrade::Fisherman);
        for state in [
            GatherState::FishermanSearchingWater,
            GatherState::FishermanWalkingToWater,
            GatherState::FishermanCheckWater,
            GatherState::FishermanStartFishing,
        ] {
            assert!(states.contains(&state), "{state} belongs to the fisherman");
        }
        assert!(!states.contains(&GatherState::FarmerHoe), "and not the farmer's");
    }

    #[test]
    fn the_actions_refuse_a_scene_built_for_another_trade() {
        // The four trades share one scene type; a fisherman handed a miner's facts says
        // so instead of pretending it has no water.
        let mut scene = GatherScene::new(
            WorldFacts::new(),
            GatherJob::Miner(crate::ai_gather::MinerFacts::default()),
        );
        let answer = fishing_action(&mut scene, GatherState::FishermanSearchingWater);
        assert_eq!(
            scene.refusal,
            Some(GatherRefusal::NoFactsForTrade),
            "the macro records the refusal"
        );
        assert_eq!(answer, Some(GatherState::FishermanSearchingWater));
    }
}
