//! The goals of the building site and of the workshop: what a builder does with one
//! block, what a crafter does with one recipe, where a courier goes with one request,
//! and how long each of those takes.
//!
//! Java reference: `core/entity/ai/workers/builder/EntityAIStructureBuilder.java`
//! (340 lines), `core/entity/ai/workers/AbstractEntityAIStructure.java` (1 168),
//! `AbstractEntityAIStructureWithWorkOrder.java` (545),
//! `core/entity/ai/workers/crafting/AbstractEntityAICrafting.java` (785) and
//! `core/entity/ai/workers/service/EntityAIWorkDeliveryman.java` (662).
//!
//! ## The brief said `core/entity/ai/goals/`, and that package does not exist
//!
//! Measured on this tree: `find minecolonies-src -type d -name goals` answers
//! nothing, and the two names the brief gives —
//! `EntityAIWorkBuilder.java`, `EntityAIWorkCrafter.java` — are in no file of it
//! either. Those are 1.12-era names. In the version this port follows, the same
//! classes live under `core/entity/ai/workers/` and are called:
//!
//! | Brief | What is really there | Lines |
//! | --- | --- | ---: |
//! | `EntityAIStructureBuilder` | `workers/builder/EntityAIStructureBuilder.java` | 340 |
//! | `EntityAIWorkBuilder` | the parent pair: `AbstractEntityAIStructure.java` + `AbstractEntityAIStructureWithWorkOrder.java` | 1 168 + 545 |
//! | `EntityAIWorkCrafter` | `workers/crafting/AbstractEntityAICrafting.java`, and the **19** concrete AIs that descend from it (measured: 20 `extends`, one of them abstract — `EntityAIWorkSawmill.java` is the one used as the example) | 785 |
//! | `EntityAIWorkDeliveryman` | `workers/service/EntityAIWorkDeliveryman.java` | 662 |
//!
//! Every citation below names the file and the line that decided, so the rename does
//! not hide anything.
//!
//! ## What was already here, and what this module adds
//!
//! **`crate::ai` already carries the builder's state machine.** `ai::builder::register`
//! is the nine targets of `AbstractEntityAIStructure.java:166-200` plus the builder's
//! five overrides, `ai::worker::register_base` is the fourteen of
//! `AbstractEntityAIBasic.java:204-259`, and `ai::builder::step_outcome` is the six
//! exits of `structureStep`. Re-porting them would be a second copy of a table that
//! already has tests, so this module does **not** touch them: a caller that wants the
//! site's decision asks `crate::ai::worker::JobAi::for_job(Job::Builder)`.
//!
//! What `crate::ai` does **not** answer, and what each file here does:
//!
//! | File | Question it answers | Java |
//! | --- | --- | --- |
//! | [`work_time`] | how many ticks to mine one block, to place one block, to hit a work surface once | `AbstractEntityAIInteract.java:308-343`, `AbstractEntityAIStructure.java:531-532`, `AbstractEntityAICrafting.java:734-738` |
//! | [`materials`] | is a wanted stack in the inventory, in the hut's bucket, or must it be asked for — and in which of the five request stages | `AbstractEntityAIStructure.java:816-923`, `AbstractEntityAIStructureWithWorkOrder.java:47-62`, `:203-343` |
//! | [`order`] | at this stage, at this position, do we place, remove, drain, spawn or skip — and in which direction does the iterator walk | `AbstractEntityAIStructure.java:121-130`, `:324-327`, `:379-459`, `:606-672` |
//! | [`crafter`] | the six targets of the workshop: read the recipe, count the runs, fetch the inputs, hit, finalize | `AbstractEntityAICrafting.java:160-646` |
//! | [`delivery`] | the six states of the courier, the warehouse door, the four errands | `EntityAIWorkDeliveryman.java:113-661` |
//!
//! **`crate::building::behaviour` was read first, and it is a different layer.** It
//! answers what happens to a *hut* — the ordered writes of a pose
//! (`behaviour::pose`), what an upgrade keeps (`behaviour::amelioration`), what a
//! deconstruction gives back (`behaviour::casse`) and which modules a kind carries
//! (`behaviour::modules`). None of it decides anything about one block of a
//! blueprint, which is this module's whole subject. `crate::structures` answers
//! *which file* a building and a level resolve to, and is used here only as the
//! reason a blueprint exists at all.
//!
//! ### The four occupations of a site, and where each is decided
//!
//! The brief names them one by one; none of them needed a new table, so this is the
//! map instead of a fifth copy:
//!
//! | Occupation | State carrying it | Decided in | Java |
//! | --- | --- | --- | --- |
//! | go to the site | `START_WORKING` → `START_BUILDING` | the builder table `crate::ai::builder::register` installs, `ai::builder::ai()` | `EntityAIStructureBuilder.java:144-161` |
//! | place one block | `BUILDING_STEP` | `crate::ai::builder::step_outcome` + [`order::decide`] | `AbstractEntityAIStructure.java:334-535` |
//! | ask for a material | `NEEDS_ITEM` | [`materials::resolve`] | `AbstractEntityAIStructure.java:816-923` |
//! | wait for it | `NEEDS_ITEM` (self-transition, 40 ticks) | the base table of `crate::ai::worker::JobAi::base` | `AbstractEntityAIBasic.java:238`, `:534-643` |
//!
//! [`materials::resolve`] is the half of "ask for a material" that `crate::ai` did not
//! have: `ai::builder::structure_check_if_needs_item` says *whether* the builder goes
//! to `NEEDS_ITEM`, and this module says *what* it is missing and whether a request
//! must be created, is already open, or whether the whole material list is stale.
//!
//! **`crate::ai::state::WorkerState` does not have the workshop's states.** It names
//! eighteen of `AIWorkerState`'s 127 — the builder's and the composter's. `CRAFT`,
//! `QUERY_ITEMS`, `GET_RECIPE`, `PREPARE_DELIVERY`, `DELIVERY`, `PICKUP` and `DUMPING`
//! are not among them, and `crate::ai/` belongs to another agent, so [`crafter`] and
//! [`delivery`] bring their own enums. The **engine** is still `crate::ai::Machine`:
//! the three passes, the tick-rate countdown and the trap rule are the same code, so a
//! crafter and a builder tick identically.
//!
//! ## The shape every file follows
//!
//! A **decision function** takes facts and answers a named enum: one variant per
//! `return` of the Java method, with the line that returns it. No `bool` where Java
//! has four answers, no `false` where Java has a message. The facts are observations a
//! hook wrote down, never calls: nothing here walks, places, breaks, crafts, opens a
//! chest or reads a file. That is what makes 66 tests possible without a world.
//!
//! ```ignore
//! use ported_minecolonies::ai_build::{crafter, delivery, materials, order, work_time};
//!
//! // How long to mine the block that is in the way (`:338`), builder buff included.
//! let ticks = work_time::mining_ticks(work_time::MiningTime {
//!     hardness: 3.0,
//!     tool_speed: 8.0,
//!     break_level: 4,
//!     builder_buff: true,
//!     ..work_time::MiningTime::default()
//! })?;
//!
//! // The blueprint wants eight planks, the builder carries one.
//! let answer = materials::resolve(&[materials::Need::new(
//!     materials::NeedKey::plain("minecraft:oak_planks"), 8).carried(1)], true);
//! match answer {
//!     materials::Resolution::Failed(failed) => {
//!         // `:903-906` — create the request the decision named.
//!         let _ = failed.because; // `RequestCreated` or `RequestAlreadyOpen`
//!     }
//!     _ => {}
//! }
//! ```
//!
//! ## What is deliberately absent
//!
//! - **The builder's nine targets** — they are `crate::ai::builder`'s, and this module
//!   says so instead of copying them (see above).
//! - **The request system.** Creating a `Stack` request, moving a completed one into
//!   the citizen's sync list, registering a block as needed: `crate::requests` owns
//!   the record, this module owns the decision to ask. `materials::Because` names which
//!   of the two happened.
//! - **Every host action.** Walking, placing, breaking, hitting, transferring items,
//!   damaging a tool: facts. `MANQUANT.md` lists the gaps this leaves, one per section.

pub mod crafter;
pub mod delivery;
pub mod materials;
pub mod order;
pub mod work_time;

pub use crafter::{
    Craft, CraftFacts, CraftState, Decide as CraftDecision, Execution, IdleDecision, IdleStep,
    Input, Items, Recipe, check_for_items, completion_experience, craft, decide as craft_decide,
    execute, get_recipe, has_work_to_do, idle as craft_idle, next_crafting_state,
};
pub use delivery::{
    Carry, Decide as DeliveryDecision, Delivery, DeliveryFacts, DeliveryState, Dump, NoCollect,
    Pickup, Prepare, Refusal as DeliveryRefusal, Task, cannot_hold_more_items, check_if_execute,
    decide as delivery_decide, deliver, dump, max_parallel_deliveries, next_pickup,
    pickup, prepare,
};
pub use materials::{
    Because, Failed, Need, NeedKey, Progress as RequestProgress, RequestStage, Resolution, Skip,
    Step, amount_of_resources, advance as advance_request, first_stage, is_block_free,
    needs_registration, reduced, registration, requests, resolve, total_amount,
};
pub use order::{
    BlockFacts, Decision, Direction, Operation, Refusal as OrderRefusal, Skip as SkipPosition, StepFacts,
    accepts, before, decide as decide_placement, is_decoration, order_of, skip_building,
    skip_clearing, skip_decorate, skip_removal, walks,
};
pub use work_time::{
    MiningTime, Refusal as TimeRefusal, craft_hits, craft_ticks, mining_ticks, place_ticks,
};
