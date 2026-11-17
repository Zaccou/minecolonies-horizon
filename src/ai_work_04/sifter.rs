//! The sifter: one `SIFT` target, a daily quota, and the sieve's own hit counter.
//!
//! Java reference: `core/entity/ai/workers/crafting/EntityAIWorkSifter.java` — the four
//! constants (`:42-57`), `registerTargets` (`:72-74`), `getActionsDoneUntilDumping`
//! (`:85-88`), `decide` (`:91-94`), `hasWorkToDo` (`:97-100`) and `sift`
//! (`:107-222`); plus the two parent targets it keeps (`AbstractEntityAICrafting.java:
//! 164-165`) and the hut's quota formula (`BuildingSifter.java:80-90`).
//!
//! ## The table
//!
//! | # | State | Rule | Rate | Java |
//! | ---: | --- | --- | ---: | --- |
//! | 1 | `IDLE` | `hasWorkToDo` → `START_WORKING` | 20 | `AbstractEntityAICrafting.java:164`, overridden at `EntityAIWorkSifter.java:97-100` |
//! | 2 | `IDLE` | `idle` (pastimes, returns null) | 20 | `AbstractEntityAICrafting.java:165` |
//! | 3 | `START_WORKING` | `decide` → `SIFT` | 5 | `:166`, overridden at `EntityAIWorkSifter.java:91-94` |
//! | 4 | `SIFT` | `sift` | 10 | `EntityAIWorkSifter.java:73` |
//! | — | `INVENTORY_FULL` | base `afterDump` → `IDLE` | — | `AbstractEntityAIBasic.java:1213-1222` |
//!
//! **The parent's other three targets are not registered, on purpose.** Java registers
//! `QUERY_ITEMS`, `GET_RECIPE` and `CRAFT` in the parent constructor
//! (`AbstractEntityAICrafting.java:167-169`) and the sifter keeps them, but nothing can
//! ever *enter* them: `decide()` is overridden to answer `SIFT` on every branch
//! (`EntityAIWorkSifter.java:91-94`), and those three states are only reached from
//! `decide()` or from each other. `crate::ai_build::crafter` already carries them with
//! their tests; registering them here would add three dead transitions and a second
//! copy of `get_recipe`.
//!
//! ## The one duration
//!
//! `sift` counts a `progress` up and fires when it passes
//! `MAX_LEVEL - (getEffectiveSkillLevel(getSecondarySkillLevel()) / 2)`
//! (`:188`). [`sift_hits`] is that number of hits, [`sift_ticks`] the same in game
//! ticks at [`SIFT_TICK_DELAY`] ticks per hit: **510 ticks (51 hits) at skill 0, 10
//! ticks (1 hit) at skill 99**. The curve is `getEffectiveSkillLevel`'s, which
//! `crate::ai::worker::effective_skill_level` already carries — the port calls it
//! instead of writing the polynomial a second time.
//!
//! **The cliff is Java's.** `progress > threshold` with `threshold` reaching zero means
//! a level-99 sifter sieves on its **first** hit, like the level-20 crafter whose
//! `craftHits` reaches zero (`AbstractEntityAICrafting.java:734-738`). It is copied,
//! not repaired: see `crate::ai_build::work_time`.

use crate::ai::ENTITY_AI_TICKRATE;
use crate::ai::state::{Action, Machine, Transition};
use crate::ai::worker::STANDARD_DELAY;

use super::TICKS_SECOND;
use super::facts::{Scene, SiftFacts};
use super::states::{Refusal, SiftState};

/// Where this file's rules and numbers come from, as one string a log line or a doc
/// page can print instead of restating them.
pub const SOURCE: &str = "EntityAIWorkSifter.java:42-57,:72-74,:85-88,:91-94,:97-100,:107-222 · \
AbstractEntityAICrafting.java:164-169,:734-738 · AbstractEntityAIBasic.java:682-685,:1213-1222 · \
BuildingSifter.java:22,:32,:80-90 · Constants.java:21,:50";

/// `EntityAIWorkSifter.MAX_LEVEL` (`:42`): the cap of the sieve threshold, i.e. the
/// number of hits a level-0 sifter needs, minus one.
pub const SIFT_MAX_LEVEL: u32 = 50;
/// `EntityAIWorkSifter.TICK_DELAY` (`:47`): the rate of the `SIFT` target (`:73`).
pub const SIFT_TICK_DELAY: u32 = 10;
/// `EntityAIWorkSifter.CHANCE_TO_DUMP_INV` (`:52`): one draw in a hundred dumps the
/// inventory after an output (`:192`).
pub const CHANCE_TO_DUMP_INV: u32 = 10;
/// `EntityAIWorkSifter.NO_MESH_DELAY` (`:57`): the wait when the hut has no mesh
/// (`:144`).
pub const NO_MESH_DELAY: u32 = 100;
/// `Constants.ONE_HUNDRED_PERCENT` (`api/util/constant/Constants.java:50`): the
/// modulus of the dump draw (`:192`).
pub const ONE_HUNDRED_PERCENT: u32 = 100;
/// `BuildingSifter.BUILDING_LEVEL_MULTIPLIER` (`:22`): the quota per building level
/// squared.
pub const BUILDING_LEVEL_MULTIPLIER: u32 = 64;
/// `BuildingSifter.MAX_BUILDING_LEVEL` (`:32`): from this level up the quota is
/// unlimited (`:82-85`).
pub const SIFTER_MAX_BUILDING_LEVEL: u32 = 5;
/// `EntityAIWorkSifter`'s experience per output (`:211`, `addExperience(0.2)`).
pub const SIFT_XP_GAIN: f64 = 0.2;

/// `BuildingSifter.getMaxDailyQuantity` (`:80-90`): `level² × 64`, and
/// `Integer.MAX_VALUE` from level 5 on (`:82-85`).
///
/// Java returns an `int`; the port returns `u32` and keeps `i32::MAX` as the value
/// Java actually produces, so a comparison against a daily count cannot overflow.
pub fn max_daily_quantity(building_level: u32) -> u32 {
    if building_level >= SIFTER_MAX_BUILDING_LEVEL {
        return i32::MAX as u32; // :84 — `Integer.MAX_VALUE`
    }
    building_level
        .saturating_mul(building_level)
        .saturating_mul(BUILDING_LEVEL_MULTIPLIER) // `(int) (Math.pow(level, 2) * 64)`, :87
}

/// How many sieve hits one output takes (`EntityAIWorkSifter.java:188`).
///
/// Java: `progress++` (`:186`) then `progress > MAX_LEVEL - (getEffectiveSkillLevel(
/// getSecondarySkillLevel()) / 2)` — so the recipe fires when the counter *passes* the
/// threshold, i.e. after `threshold + 1` hits.
pub fn sift_hits(secondary_skill_level: i32) -> Result<u32, Refusal> {
    if secondary_skill_level < 0 {
        // `AbstractEntityAIBasic.getEffectiveSkillLevel` is a polynomial on a level
        // that is never negative; a fact that is means a broken observation, not a
        // slow sifter.
        return Err(Refusal::BrokenSkillLevel);
    }
    let effective = crate::ai::effective_skill_level(secondary_skill_level); // :188
    let threshold = SIFT_MAX_LEVEL as i32 - effective / 2; // int division, `:188`
    Ok(((threshold + 1) as u32).max(1))
}

/// [`sift_hits`] in game ticks: one hit every [`SIFT_TICK_DELAY`] ticks (`:47`, `:73`).
pub fn sift_ticks(secondary_skill_level: i32) -> Result<u32, Refusal> {
    Ok(sift_hits(secondary_skill_level)? * SIFT_TICK_DELAY)
}

/// What one `sift` call decided — one variant per `return` of `:107-222`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the nine exits of `sift`
pub enum Sift {
    /// The day's quota is spent (`:110-113`) → `IDLE`.
    QuotaReached,
    /// `!walkToBuilding()` (`:115-118`): stay in `SIFT`.
    Walking,
    /// `!hasSpace()` (`:120-123`) → `INVENTORY_FULL`.
    InventoryFull,
    /// No mesh in the hut, but one in the pockets (`:135-140`): one action done, then
    /// `INVENTORY_FULL` so the dump takes the mesh back to the hut.
    MeshInInventory,
    /// No mesh in the hut and none in the pockets (`:133-146`): message the player,
    /// wait [`NO_MESH_DELAY`], then `START_WORKING`.
    NoMesh,
    /// The hut has meshes but the module has no fulfillable recipe (`:147-157`): the
    /// hands are emptied and the tick goes to `START_WORKING`.
    NoRecipe,
    /// The recipe is missing its mesh or its input (`:166-170`): forget it and stay.
    RecipeIncomplete,
    /// The sieve-speed level is negative, so the threshold has no answer (`:188`).
    BrokenSkill,
    /// Still sieving: the progress after this hit, which stays below the threshold
    /// (`:186`, `:188`).
    Hitting {
        /// `progress` after `:186`.
        progress: u32,
    },
    /// One output made (`:188-212`): the counter is back to zero, the day's quota went
    /// up by one, and `dump` is Java's `:192` decision.
    Produced {
        /// `building.getCurrentDailyQuantity() >= building.getMaxDailyQuantity() ||
        /// worker.getRandom().nextInt(100) < CHANCE_TO_DUMP_INV` (`:192`), evaluated on
        /// the **incremented** quota as Java does.
        dump: bool,
        /// `fullfillRecipeAndCopy(...) == null` (`:199`): the recipe did not run, so
        /// Java forgets it and stays.
        failed: bool,
    },
}

impl Sift {
    /// The state this decision leads to, for a caller that only wants the goal.
    pub fn next(self) -> SiftState {
        match self {
            Sift::QuotaReached => SiftState::Idle,
            Sift::Walking | Sift::RecipeIncomplete | Sift::BrokenSkill => SiftState::Sift,
            Sift::InventoryFull | Sift::MeshInInventory => SiftState::InventoryFull,
            Sift::NoMesh | Sift::NoRecipe => SiftState::StartWorking,
            Sift::Hitting { .. } | Sift::Produced { .. } => SiftState::Sift,
        }
    }
}

/// `hasWorkToDo` (`EntityAIWorkSifter.java:97-100`): the parent's answer, **or** a
/// quota that is not spent. That second half is why a sifter with an empty queue still
/// walks back to its hut.
pub fn has_work_to_do(facts: &SiftFacts) -> bool {
    crate::ai_build::crafter::has_work_to_do(&facts.craft) // :99, the `super.` call
        || facts.daily_quantity < facts.max_daily_quantity // :99, the override
}

/// `sift` (`:107-222`), condition by condition.
pub fn sift(facts: &SiftFacts) -> Sift {
    if facts.daily_quantity >= facts.max_daily_quantity {
        return Sift::QuotaReached; // :110-113
    }
    if !facts.craft.at_building {
        return Sift::Walking; // :115-118
    }
    if !facts.has_space {
        return Sift::InventoryFull; // :120-123
    }
    let Some(recipe) = facts.recipe else {
        // :131-158 — the module answered null. The mesh decides what happens next.
        if facts.building_meshes == 0 {
            // :133 — `hasBuildingEnoughElseCount(..., meshes, 1) == 0`.
            if facts.worker_meshes > 0 {
                return Sift::MeshInInventory; // :135-140
            }
            return Sift::NoMesh; // :141-146
        }
        // :147-157 — the hut has meshes, so this is not a "no mesh" message: Java
        // empties both hands, resets the progress and asks `decide` again.
        return Sift::NoRecipe;
    };
    if !recipe.mesh_present || !recipe.input_present {
        return Sift::RecipeIncomplete; // :166-170
    }
    let Ok(hits) = sift_hits(facts.secondary_skill_level) else {
        return Sift::BrokenSkill; // :188 through `getEffectiveSkillLevel`
    };
    let progress = facts.progress.saturating_add(1); // :186
    if progress < hits {
        return Sift::Hitting { progress };
    }
    // :190-212 — one output. `:191` increments the quota first, and `:192` compares the
    // incremented value, so the draw and the quota see the same number.
    let daily_after = facts.daily_quantity.saturating_add(1);
    let dump = daily_after >= facts.max_daily_quantity
        || facts.roll % ONE_HUNDRED_PERCENT < CHANCE_TO_DUMP_INV; // :192
    Sift::Produced {
        dump,
        failed: facts.fulfil_failed, // :199-203
    }
}

// ------------------------------------------------------------------- actions

/// `:164` — `hasWorkToDo` carries the quota half (`:97-100`).
fn idle_has_work(facts: &Scene<SiftFacts>, _: SiftState) -> bool {
    has_work_to_do(&facts.facts)
}

/// `:165` — `idle`: the crafter's pastimes, unchanged. The decision (and the
/// `idlePos` it stores) is `crate::ai_build::crafter::idle`, not a second copy.
fn idle_action(scene: &mut Scene<SiftFacts>, _: SiftState) -> Option<SiftState> {
    // Reaching this target at all means `:164`'s condition failed — the queue is empty
    // or spent, and the quota is gone. That is the "nothing to do" the player asks
    // about, named before the pastime is picked.
    if !has_work_to_do(&scene.facts) {
        scene.refuse(Refusal::NothingToCraft);
    }
    let decision = crate::ai_build::crafter::idle(&scene.facts.craft);
    scene.facts.craft.idle_pos = decision.idle_pos;
    None // `NO_CHANGE` (`:68`): a null supplier does not stop the tick
}

/// `:166` through the sifter's override (`:91-94`): `decide` always answers `SIFT`.
fn decide_action(_: &mut Scene<SiftFacts>, _: SiftState) -> Option<SiftState> {
    Some(SiftState::Sift) // `EntityAIWorkSifter.java:93`
}

/// `:107-222` — `sift`.
fn sift_action(scene: &mut Scene<SiftFacts>, state: SiftState) -> Option<SiftState> {
    match sift(&scene.facts) {
        Sift::QuotaReached => {
            scene.refuse(Refusal::QuotaReached); // :110-113
            Some(SiftState::Idle)
        }
        Sift::Walking => {
            scene.refuse(Refusal::Walking); // :115-118
            Some(state)
        }
        Sift::InventoryFull => {
            scene.refuse(Refusal::InventoryFull); // :120-123
            Some(SiftState::InventoryFull)
        }
        Sift::MeshInInventory => {
            scene.facts.craft.actions_done += 1; // `incrementActionsDone()`, :138
            scene.refuse(Refusal::MeshInInventory);
            Some(SiftState::InventoryFull) // :139
        }
        Sift::NoMesh => {
            // :144 — `setDelay(NO_MESH_DELAY)`; the message is the caller's (see
            // `MANQUANT.md` §4).
            scene.memory.delay = NO_MESH_DELAY as i32;
            scene.facts.progress = 0; // :156
            scene.refuse(Refusal::NoMesh);
            Some(SiftState::StartWorking) // :157
        }
        Sift::NoRecipe => {
            scene.facts.progress = 0; // :156
            scene.refuse(Refusal::NoRecipe);
            Some(SiftState::StartWorking) // :157
        }
        Sift::RecipeIncomplete => {
            scene.facts.recipe = None; // :168, `currentRecipeStorage = null`
            scene.refuse(Refusal::RecipeIncomplete);
            Some(state) // :169, `getState()`
        }
        Sift::BrokenSkill => {
            scene.refuse(Refusal::BrokenSkillLevel);
            Some(state)
        }
        Sift::Hitting { progress } => {
            scene.facts.progress = progress; // :186
            Some(state) // :221
        }
        Sift::Produced { dump, failed } => {
            scene.facts.progress = 0; // :190
            scene.facts.daily_quantity = scene.facts.daily_quantity.saturating_add(1); // :191
            if dump {
                scene.facts.craft.actions_done += 1; // `incrementActionsDoneAndDecSaturation`, :192-195
            }
            if failed {
                scene.facts.recipe = None; // :201
                scene.refuse(Refusal::RecipeFailed); // :202
            }
            Some(state) // :221, `getState()`
        }
    }
}

/// Register the sifter's table: the two `IDLE` targets of the parent
/// (`AbstractEntityAICrafting.java:164-165`), its `START_WORKING` target (`:166`) and
/// its own `SIFT` (`EntityAIWorkSifter.java:73`), plus the exit the base class gives
/// `INVENTORY_FULL`.
pub fn register(machine: &mut Machine<SiftState, Scene<SiftFacts>>) {
    // :164 — `new AITarget(IDLE, this::hasWorkToDo, () -> START_WORKING, TICKS_SECOND)`.
    machine.add(Transition::target_when(
        SiftState::Idle,
        "hasWorkToDo",
        "AbstractEntityAICrafting.java:164",
        TICKS_SECOND,
        idle_has_work,
        Action::Fixed(Some(SiftState::StartWorking)),
    ));
    // :165 — `new AITarget(IDLE, this::idle, TICKS_SECOND)`.
    machine.add(Transition::target(
        SiftState::Idle,
        "idle",
        "AbstractEntityAICrafting.java:165",
        TICKS_SECOND,
        Action::Run(idle_action),
    ));
    // :166 — `new AITarget(START_WORKING, this::decide, STANDARD_DELAY)`, whose
    // `decide` is `EntityAIWorkSifter.java:91-94`.
    machine.add(Transition::target(
        SiftState::StartWorking,
        "decide",
        "EntityAIWorkSifter.java:91-94",
        STANDARD_DELAY, // `AbstractEntityAIBasic.java:100`, the rate of `:166`
        Action::Run(decide_action),
    ));
    // :73 — `new AITarget(SIFT, this::sift, TICK_DELAY)`.
    machine.add(Transition::target(
        SiftState::Sift,
        "sift",
        "EntityAIWorkSifter.java:73",
        SIFT_TICK_DELAY,
        Action::Run(sift_action),
    ));
    // The base class's `afterDump` (`AbstractEntityAIBasic.java:1213-1222`): a crafter
    // that is not paused goes back to `IDLE`. The crafting override's extra work
    // (finishing the request, shipping the secondary outputs, `:679-721`) is the
    // crafter's, and the sifter never has a request in this table.
    machine.add(Transition::goto(
        SiftState::InventoryFull,
        SiftState::Idle,
        "afterDump",
        "AbstractEntityAIBasic.java:1213-1222",
        1,
    ));
}

/// A sifter's machine, started in `IDLE`.
pub fn ai() -> Machine<SiftState, Scene<SiftFacts>> {
    let mut machine = Machine::new(SiftState::Idle, ENTITY_AI_TICKRATE);
    register(&mut machine);
    machine
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::worker::STANDARD_DELAY;
    use crate::ai_build::crafter::CraftFacts;
    use crate::ai_work_04::facts::SiftRecipe;

    fn sieveable() -> SiftFacts {
        SiftFacts {
            building_meshes: 4,
            recipe: Some(SiftRecipe::complete()),
            craft: CraftFacts {
                task_queue: 1,
                current_task: true,
                ..CraftFacts::default()
            },
            ..SiftFacts::default()
        }
    }

    #[test]
    fn the_quota_is_the_building_level_squared_times_sixty_four() {
        // `BuildingSifter.java:80-90`.
        assert_eq!(max_daily_quantity(1), 64);
        assert_eq!(max_daily_quantity(2), 256);
        assert_eq!(max_daily_quantity(4), 1024);
        // From level 5 on, `Integer.MAX_VALUE` (`:82-85`).
        assert_eq!(max_daily_quantity(5), i32::MAX as u32);
        assert_eq!(max_daily_quantity(9), i32::MAX as u32);
        assert_eq!(BUILDING_LEVEL_MULTIPLIER, 64);
        assert_eq!(SIFTER_MAX_BUILDING_LEVEL, 5);
    }

    #[test]
    fn a_level_zero_sifter_needs_fifty_one_hits_and_a_level_ninety_nine_one() {
        // `:188`: `progress > 50 - effective(level) / 2`.
        assert_eq!(sift_hits(0), Ok(51));
        // `effective(99) = 100`, so `100 / 2 = 50` and the threshold is zero.
        assert_eq!(sift_hits(99), Ok(1));
        assert_eq!(sift_ticks(0), Ok(510));
        assert_eq!(sift_ticks(99), Ok(10));
        assert_eq!(SIFT_MAX_LEVEL, 50);
        assert_eq!(SIFT_TICK_DELAY, 10);
    }

    #[test]
    fn the_hit_count_follows_the_effective_curve_the_base_table_already_carries() {
        // `AbstractEntityAIBasic.java:682-685`, reused rather than rewritten.
        let level = 20;
        let effective = crate::ai::effective_skill_level(level);
        assert_eq!(effective, 37);
        // 50 - 37/2 = 50 - 18 = 32, so 33 hits.
        assert_eq!(sift_hits(level), Ok(33));
        // And a negative level is a broken fact, not a slow sifter.
        assert_eq!(sift_hits(-1), Err(Refusal::BrokenSkillLevel));
    }

    #[test]
    fn a_spent_quota_sends_the_sifter_to_idle() {
        // `:110-113`.
        let facts = SiftFacts {
            daily_quantity: 64,
            ..sieveable()
        };
        assert_eq!(sift(&facts), Sift::QuotaReached);
        assert_eq!(sift(&facts).next(), SiftState::Idle);
    }

    #[test]
    fn a_sifter_away_from_its_hut_keeps_walking() {
        // `:115-118`.
        let facts = SiftFacts {
            craft: CraftFacts {
                at_building: false,
                ..sieveable().craft
            },
            ..sieveable()
        };
        assert_eq!(sift(&facts), Sift::Walking);
    }

    #[test]
    fn a_full_inventory_is_dumped_before_anything_else() {
        // `:120-123`.
        let facts = SiftFacts {
            has_space: false,
            ..sieveable()
        };
        assert_eq!(sift(&facts), Sift::InventoryFull);
    }

    #[test]
    fn no_mesh_anywhere_complains_and_waits_a_hundred_ticks() {
        // `:133-146`.
        let facts = SiftFacts {
            building_meshes: 0,
            worker_meshes: 0,
            recipe: None,
            ..sieveable()
        };
        assert_eq!(sift(&facts), Sift::NoMesh);
        assert_eq!(NO_MESH_DELAY, 100);
        assert_eq!(sift(&facts).next(), SiftState::StartWorking);
    }

    #[test]
    fn a_mesh_in_the_pockets_goes_back_to_the_hut() {
        // `:135-140` — one action done, then `INVENTORY_FULL`.
        let facts = SiftFacts {
            building_meshes: 0,
            worker_meshes: 1,
            recipe: None,
            ..sieveable()
        };
        assert_eq!(sift(&facts), Sift::MeshInInventory);
        assert_eq!(sift(&facts).next(), SiftState::InventoryFull);
    }

    #[test]
    fn meshes_in_the_hut_without_a_recipe_simply_retry() {
        // `:147-157`: the hands are emptied, the progress reset, `START_WORKING`.
        let facts = SiftFacts {
            recipe: None,
            ..sieveable()
        };
        assert_eq!(sift(&facts), Sift::NoRecipe);
        // The distinction with `NoMesh` is the whole reason the two exits exist.
        assert_ne!(sift(&facts), Sift::NoMesh);
    }

    #[test]
    fn a_recipe_missing_its_input_is_dropped_and_the_state_held() {
        // `:166-170`.
        let facts = SiftFacts {
            recipe: Some(SiftRecipe::without_input()),
            ..sieveable()
        };
        assert_eq!(sift(&facts), Sift::RecipeIncomplete);
        assert_eq!(sift(&facts).next(), SiftState::Sift);
    }

    #[test]
    fn the_progress_counts_up_one_hit_at_a_time() {
        // `:186` and `:188` with a level-0 sifter: 51 hits.
        let facts = SiftFacts {
            progress: 10,
            ..sieveable()
        };
        assert_eq!(sift(&facts), Sift::Hitting { progress: 11 });
        let almost = SiftFacts {
            progress: 49,
            ..sieveable()
        };
        assert_eq!(sift(&almost), Sift::Hitting { progress: 50 });
        let last = SiftFacts {
            progress: 50,
            ..sieveable()
        };
        // The 51st hit is the one that fires: `progress > 50`.
        assert!(matches!(sift(&last), Sift::Produced { .. }));
    }

    #[test]
    fn an_output_dumps_when_the_quota_lands_or_when_the_draw_is_low() {
        // `:191-192`, on the incremented quota.
        let landed = SiftFacts {
            progress: 50,
            daily_quantity: 63,
            max_daily_quantity: 64,
            ..sieveable()
        };
        assert_eq!(
            sift(&landed),
            Sift::Produced {
                dump: true,
                failed: false
            }
        );

        let lucky = SiftFacts {
            progress: 50,
            roll: CHANCE_TO_DUMP_INV - 1,
            ..sieveable()
        };
        assert_eq!(
            sift(&lucky),
            Sift::Produced {
                dump: true,
                failed: false
            }
        );

        let unlucky = SiftFacts {
            progress: 50,
            roll: CHANCE_TO_DUMP_INV,
            ..sieveable()
        };
        assert_eq!(
            sift(&unlucky),
            Sift::Produced {
                dump: false,
                failed: false
            }
        );
    }

    #[test]
    fn a_recipe_that_refused_to_run_forgets_itself() {
        // `:199-203`.
        let facts = SiftFacts {
            progress: 50,
            fulfil_failed: true,
            ..sieveable()
        };
        assert_eq!(
            sift(&facts),
            Sift::Produced {
                dump: false,
                failed: true
            }
        );
    }

    #[test]
    fn a_sifter_works_with_an_empty_queue_because_of_its_quota() {
        // `:97-100` — the override is the second half of the test.
        let empty_queue = SiftFacts {
            craft: CraftFacts::default(),
            ..SiftFacts::default()
        };
        assert!(!crate::ai_build::crafter::has_work_to_do(
            &empty_queue.craft
        ));
        assert!(has_work_to_do(&empty_queue), "the quota is not spent");

        let spent = SiftFacts {
            daily_quantity: 64,
            max_daily_quantity: 64,
            ..SiftFacts::default()
        };
        assert!(
            !has_work_to_do(&spent),
            "nothing to craft and no quota left"
        );
    }

    #[test]
    fn the_table_has_four_targets_and_each_state_can_be_left() {
        let machine = ai();
        let rules: Vec<&str> = machine
            .transitions()
            .iter()
            .map(|transition| transition.rule)
            .collect();
        assert_eq!(
            rules,
            vec!["hasWorkToDo", "idle", "decide", "sift", "afterDump"]
        );
        assert_eq!(machine.state(), SiftState::Idle);
        // The rates Java registers: 20, 20, 5, 10 (`:164`, `:165`, `:166`, `:73`).
        assert_eq!(machine.transitions()[0].tick_rate, TICKS_SECOND);
        assert_eq!(machine.transitions()[1].tick_rate, TICKS_SECOND);
        assert_eq!(machine.transitions()[2].tick_rate, STANDARD_DELAY);
        assert_eq!(machine.transitions()[3].tick_rate, SIFT_TICK_DELAY);
        for state in SiftState::ALL {
            assert!(machine.can_leave(state), "{state} is a trap state");
        }
    }
}
