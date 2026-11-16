//! The selection of goals: what a hired citizen works on next, in what order, and
//! why it stops.
//!
//! Java reference: `core/entity/ai/workers/AbstractAISkeleton.java` (the tick and
//! its machine), `AbstractEntityAIBasic.java` (the fourteen targets every job
//! inherits), `AbstractEntityAIStructure.java` + `AbstractEntityAIStructureWithWorkOrder
//! .java` + `builder/EntityAIStructureBuilder.java` (the builder) and
//! `production/agriculture/EntityAIWorkComposter.java` (the composter) — the ones
//! named in `EntityAIWorker`'s brief, which exists in this tree as
//! `AbstractAISkeleton` + `AbstractEntityAIBasic`: there is **no** `EntityAIWorker.java`
//! and no `EntityAIComposter.java` / `EntityAIBuilder.java` here, the real names are
//! the four above.
//!
//! | File | Question |
//! | --- | --- |
//! | [`state`] | what a state machine is here: passes, transitions, tick rates |
//! | [`facts`] | what the AI may read: the world as observations, not calls |
//! | [`worker`] | the base table: the fourteen targets, in Java's order |
//! | [`composter`] | one simple trade: six targets, two barrel scans |
//! | [`builder`] | one complex trade: nine targets, a stage list, six outcomes |
//!
//! **The decision, in one sentence.** Given [`WorldFacts`] and the AI's own
//! [`worker::Memory`], a [`worker::JobAi`] ticks and answers [`worker::Outcome`]:
//! the state now in force (the goal), the rule that chose it with its
//! `fichier.java:ligne`, and — when it could not go further — a named
//! [`worker::Refusal`]. Nothing else: this module does not move a citizen, place a
//! block, or open a chest.
//!
//! ```ignore
//! let mut ai = JobAi::for_job(Job::Composter)?;      // or JobAi::base(job)
//! let mut scene = Scene::with_profile(facts, JobProfile::COMPOSTER);
//! let outcome = ai.tick(&mut scene);
//! match (outcome.goal(), outcome.why_not()) {
//!     (WorkerState::ComposterFill, _) => /* fill a barrel */,
//!     (_, Some(Refusal::CompostablesNotSet)) => /* ask the player for a list */,
//!     _ => {}
//! }
//! ```
//!
//! **Where the citizen-level decision ends and this begins.** Which *state* a
//! citizen is in — eating, sleeping, sick, mourning, working — is decided by
//! `crate::citizens::priority` (a port of `CitizenAI.calculateNextState`, 14 rungs);
//! this module starts where that one answers `CitizenState::Working`
//! (`CitizenAI.java:92-104` ticks the job's AI only then). The two tables meet at
//! `canGoIdle`: a job whose AI may idle (`worker::can_go_idle`) hands the citizen
//! back to `CitizenState::Idle` instead (`CitizenAI.java:250-260`), and the port
//! answers that question from [`worker::JobProfile::idle`].
//!
//! **What is deliberately absent.**
//!
//! - **The goals themselves.** No pathing, no block breaking, no chest transfer, no
//!   barrel: every one of those needs the level, and the level is the reason the
//!   facts exist. [`worker::Memory`] keeps what the AI remembers between ticks
//!   (`delay`, `slot_at`, `walk_to`), so the *decision* is replayable without a world.
//! - **The request system.** A tool request, a pickup request and a stack request are
//!   facts here (`BuildingFacts::open_tool_requests`, `pickup_priority`,
//!   `completed_requests`); creating one is `crate::requests`' business.
//! - **The other 48 trades.** [`worker::JobAi::for_job`] answers `Err(Unported)` for
//!   them instead of a table of guesses; [`worker::JobAi::base`] is the half they
//!   really have. `RAPPORT.md` §3 lists them and the cost.
//! - **Randomness.** The two draws `dumpOneMoreSlot` makes are facts
//!   (`WorkerFacts::coin`, `WorkerFacts::roll`) — a decision that depends on
//!   `worker.getRandom()` cannot be replayed, and an unreplayable decision cannot be
//!   tested.
//!
//! **No SDK call.** Nothing here imports `mod_api`: the module is pure data and pure
//! rules, the same choice `crate::jobs` and `crate::citizens` made. `MANQUANT.md`
//! lists the two API gaps this slice would need to *act* on its decisions.

pub mod builder;
pub mod composter;
pub mod facts;
pub mod state;
pub mod worker;

#[cfg(test)]
mod tests;

pub use builder::{
    BuilderStep, CancelReason, MiningOutcome, WorkOrderCheck, check_for_work_order,
    check_if_canceled, is_there_a_structure, mining_decision, stages_for, step_outcome,
};
pub use composter::{
    BarrelChoice, LootMultiplier, MaterialsOutcome, barrel_acceleration_ticks, complain,
    decide_what_to_do, get_materials, loot_multiplier,
};
pub use facts::{
    Barrel, Block, Blocks, BuilderFacts, BuildingFacts, ComposterFacts, DumpSlot,
    InventoryFacts, JobFacts, OrderKind, Placement, Pos, Stage, Stages, WorkOrder,
    WorkerFacts, WorldFacts,
};
pub use state::{
    Action, Blocking, Condition, ENTITY_AI_TICKRATE, Fired, MAX_AI_TICKRATE,
    MAX_TICKRATE_VARIANT, Machine, MissingTransition, Transition, WorkerState,
    clamp_tick_rate,
};
pub use worker::{
    AfterDumpPickup, BestTool, IdleRule, JobAi, JobProfile, JobStatus, Memory, NeedsItemRule,
    Outcome, Refusal, Scene, ToolCheck, Unported, after_dump, base_check_if_needs_item,
    best_tool_slot, can_go_idle, check_for_tool, effective_skill_level, has_not_delayed,
    inverted_effective_skill_level, set_delay,
};
