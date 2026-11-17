//! The goals of three crafter trades: the carpenter (sawmill), the sifter and the
//! smelter — the states they walk, what they ask the world, the refusals they name,
//! and how long each step takes.
//!
//! Java reference: `core/entity/ai/workers/crafting/EntityAIWorkSawmill.java` (45
//! lines), `crafting/EntityAIWorkSifter.java` (223), `crafting/EntityAIWorkSmelter.java`
//! (239), plus the two parents those files really live in —
//! `crafting/AbstractEntityAICrafting.java` (785) and
//! `workers/AbstractEntityAIUsesFurnace.java` (507).
//!
//! ## The finding first: the sawmill has no state machine of its own
//!
//! `EntityAIWorkSawmill` is 45 lines — `package`, eight imports, one class javadoc — and
//! declares **not one** `static final` number, **not one** `AITarget`, and no override of
//! `decide`, `craft` or `getActionsDoneUntilDumping`. Its three additions (its
//! constructor only forwards to `super`, `:28-31`) are the expected building
//! (`:33-37`), a visible-status constant (`:20-21`) and an override that sets that
//! status before calling `super.craft()` (`:39-44`). Its whole table is
//! `AbstractEntityAICrafting`'s six targets, and **that table is already ported**:
//! `crate::ai_build::crafter` carries the six targets, the eight `getRecipe` exits, the
//! eight `craft` exits and 25 tests, and `ai_build::work_time::craft_hits` carries the
//! duration formula. Re-porting them here would be a second copy of a table that is
//! already proven, which the brief forbids. So [`sawmill`] adds only what the 45 lines
//! add, and says where the rest lives.
//!
//! What this module therefore **brings**, in lines of Java nobody had ported yet:
//!
//! | Trade | Table | Java |
//! | --- | --- | --- |
//! | Sifter | 4 reachable targets (`IDLE`×2, `START_WORKING`, `SIFT`) | `EntityAIWorkSifter.java:72-74` + `AbstractEntityAICrafting.java:164-166` |
//! | Smelter | 7 targets (`IDLE`, `START_WORKING`, `FILL_UP_FURNACES`, `RETRIEVING_END_PRODUCT_FROM_FURNACE`, `RETRIEVING_USED_FUEL_FROM_FURNACE`, `BREAK_ORES`, one `AI_BLOCKING`) | `AbstractEntityAIUsesFurnace.java:76-82` + `EntityAIWorkSmelter.java:69` |
//!
//! ## The two tables
//!
//! | # | Pass / state | Rule | Rate | Java |
//! | ---: | --- | --- | ---: | --- |
//! | 1 | `IDLE` | `hasWorkToDo` → `START_WORKING` (sifter's override) | 20 | `AbstractEntityAICrafting.java:164`, `EntityAIWorkSifter.java:97-100` |
//! | 2 | `IDLE` | `idle` (pastimes, `NO_CHANGE`) | 20 | `:165` |
//! | 3 | `START_WORKING` | `decide` → `SIFT` | 5 | `:166`, `EntityAIWorkSifter.java:91-94` |
//! | 4 | `SIFT` | `sift` | 10 | `EntityAIWorkSifter.java:73` |
//! | — | `INVENTORY_FULL` | base `afterDump` → `IDLE` | — | `AbstractEntityAIBasic.java:1213-1222` |
//!
//! | # | Pass / state | Rule | Rate | Java |
//! | ---: | --- | --- | ---: | --- |
//! | 1 | `IDLE` | → `START_WORKING`, unconditionally | 5 | `AbstractEntityAIUsesFurnace.java:77` |
//! | 2 | `START_WORKING` | `startWorking` | 60 | `:78` |
//! | 3 | `FILL_UP_FURNACES` | `fillUpFurnace` | 5 | `:79` |
//! | 4 | `AI_BLOCKING` | `accelerateFurnaces` (returns null: never stops the tick) | 20 | `:80` |
//! | 5 | `RETRIEVING_END_PRODUCT_FROM_FURNACE` | `retrieveSmeltableFromFurnace` | 5 | `:81` |
//! | 6 | `RETRIEVING_USED_FUEL_FROM_FURNACE` | `retrieveUsedFuel` | 5 | `:82` |
//! | 7 | `BREAK_ORES` | `breakOres` | 20 | `EntityAIWorkSmelter.java:69` |
//! | — | `INVENTORY_FULL`, `GATHERING_REQUIRED_MATERIALS` | base `afterDump`, `getStateAfterPickUp` | — | `AbstractEntityAIBasic.java:1213-1222`, `:323-326` |
//!
//! **The smelter's `IDLE` has no condition.** The sifter's first target is
//! `AITarget(IDLE, this::hasWorkToDo, () -> START_WORKING, TICKS_SECOND)` because a
//! crafter may have an empty queue; the furnace user's is
//! `AITarget(IDLE, START_WORKING, STANDARD_DELAY)` (`:77`), which fires whatever the
//! queue says — a smelter with nothing to smelt still walks to its hut and asks
//! `startWorking` what to do. That difference is one test on each side.
//!
//! ## What is reused, and from where
//!
//! - **The engine**: `crate::ai::state::{Machine, Transition, Action, Blocking}` — the
//!   four passes, the tick-rate countdowns, the trap rule. Not copied.
//! - **The AI's working memory**: `crate::ai::worker::Memory` (`delay`, `walk_to`, …) is
//!   taken whole by [`facts::Scene`], and `AbstractEntityAIBasic.STANDARD_DELAY` (5) is
//!   the rate of three of the seven furnace targets. The base table's own `Refusal` is
//!   **not** reused: none of the reasons below is one of its twenty (see
//!   [`states::Refusal`]).
//! - **The crafter's rules**: `crate::ai_build::crafter::{CraftFacts, CraftState,
//!   idle, has_work_to_do, decide, …}` — the sifter *composes* them
//!   ([`facts::SiftFacts::craft`]) instead of restating them.
//! - **The duration formulas already ported**: `crate::ai::worker::effective_skill_level`
//!   (the level curve `getEffectiveSkillLevel`), used by [`sifter::sift_hits`].
//!
//! ## What is deliberately absent
//!
//! Walking, item transfers, recipe execution, particles, sounds, statistics, the
//! furnace block entity tick: every one of those is an observation the caller writes
//! down ([`facts`]) or an effect it performs after reading a decision. Nothing here
//! imports `mod_api`, which is what makes 69 tests possible with no world.
//!
//! ## The two seams this module cannot close by itself
//!
//! 1. `crate::ai::facts::JobFacts` has three variants (`Composter`, `Builder`,
//!    `Unported`) and lives in a file this agent does not own, so the trade half of
//!    the facts is [`facts::SiftFacts`] / [`facts::SmelterFacts`], not a fourth
//!    variant. `crate::ai::worker::Scene` pairs `WorldFacts` with `Memory`; the
//!    scenes here pair the trade facts with the same `Memory` and a [`states::Refusal`].
//! 2. `crate::ai::worker::JobAi::for_job` answers `Err(Unported)` for these three
//!    jobs, so a caller cannot yet assemble "base table + this table" into one
//!    `JobAi`. [`states::SiftState::base`] and [`states::SmeltState::base`] map the
//!    four states the two tables share, which is the whole of the join. `MANQUANT.md`
//!    §1 carries the exact shape the fix needs.
//!
//! ## Where it stands on the Java
//!
//! Every rule below cites `fichier.java:ligne`; every constant carries its line in its
//! own doc and each of the two trade files exports a `SOURCE` naming the whole
//! provenance; every refusal names the `return` it stands for. Where Java's behaviour
//! looks like a bug it is copied and flagged, not repaired: the sifter's
//! `progress > MAX_LEVEL - (effective / 2)` fires on the **first** hit from skill 99
//! (`EntityAIWorkSifter.java:188` with `AbstractEntityAIBasic.java:682-685`), and the
//! smelter's `RETRIEVE_SMELTABLE_IF_MORE_THAN` harvests a furnace whose result slot
//! passed ten items even while it is lit (`AbstractEntityAIUsesFurnace.java:141`).

pub mod facts;
pub mod sawmill;
pub mod sifter;
pub mod smelter;
pub mod states;

#[cfg(test)]
mod tests;

/// `Constants.TICKS_SECOND` (`api/util/constant/Constants.java:21`): one second of game
/// ticks, the rate of the sifter's `IDLE` targets (`AbstractEntityAICrafting.java:164-165`),
/// of the smelter's acceleration event (`AbstractEntityAIUsesFurnace.java:80`) and of
/// `BREAK_ORES` (`EntityAIWorkSmelter.java:69`).
pub const TICKS_SECOND: u32 = 20;

pub use facts::{Furnace, Ore, OreRecipe, Scene, SiftFacts, SiftRecipe, SmelterFacts};
pub use sawmill::{CRAFTING, SawmillDelta, VisibleStatus, craft_visible_status, delta};
pub use sifter::{Sift, max_daily_quantity, sift, sift_hits, sift_ticks};
pub use smelter::{
    AbleToSmelt, BreakOres, Demand, Fill, ImportantJob, Material, Retrieve, Start,
    acceleration_ticks, break_ores, check_for_important_jobs, check_if_able_to_smelt,
    fill_up_furnace, is_smeltable, oven_to_retrieve_from, oven_to_retrieve_fuel_from,
    reached_max_to_keep, request_smeltable, retrieve_product, retrieve_used_fuel,
    smelt_request_count, start_working,
};
pub use states::{Refusal, SiftState, SmeltState};
