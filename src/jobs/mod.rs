//! The jobs: the 49 métiers a colony hires, the hut that hires each one, the two
//! skills it levels, the tool it states, and the experience curve they all share.
//!
//! Java reference: `api/colony/jobs/` (the registry and `IJob`),
//! `core/colony/jobs/` + `core/colony/jobs/guard/` (the 50 job classes),
//! `core/colony/buildings/modules/BuildingModules.java` (which hut hires which
//! job, with what skills), `api/equipment/ModEquipmentTypes.java` (the tools) and
//! `core/util/ExperienceUtils.java` (the curve).
//!
//! | File | Question |
//! | --- | --- |
//! | `job` | which jobs exist, what each is called, what tool each states |
//! | `employment` | which hut hires which job, with which skills, how many heads |
//! | `experience` | what one skill level costs, and what a grant does to it |
//! | `tool` | which tool types exist, and the level a hut accepts |
//!
//! **The counts, because three numbers look right.**
//!
//! | What | How many |
//! | --- | --- |
//! | `ModJobs` ids, and registered entries | 51 (`ModJobs.java:13-65`, `ModJobsInitializer.java:28-332`) |
//! | distinct job classes | **50** — `cookassistant` reuses `JobChef` |
//! | [`Job`] variants here | **50**, `Placeholder` included |
//! | jobs that work in a hut | **49** |
//! | worker modules (hut × job rows) | **58** |
//! | building kinds that hire somebody | **47** of 54 |
//!
//! **Pure data, pure rules.** Nothing here calls the host: no `LevelRef`, no
//! block entity, no mob, no clock, no thread, no file — the same choice
//! `crate::colony` and `crate::inventory` made, and what lets every rule be
//! tested with no game, no wasm and no prepared resources. The hooks that will
//! use it (a hire, a level-up, a tool request) call in from above.
//!
//! **What is *not* here, and what each gap costs.**
//!
//! - **the AI**, deliberately: no state machine, no targets, no pathing, no
//!   `JobStatus`. Java's worker AI is 29 780 lines
//!   (`core/entity/ai/workers/`) driven from `entity_tick_pre`, which is still a
//!   **global** per-tick hook
//!   (`crates/horizon-mod-vocab/src/hooks/entity_tick_pre.rs:19`), so a citizen's
//!   own tick does not exist yet. Without it a job can be described, hired and
//!   levelled, but not *performed*: the port knows a forester chops with an axe
//!   and cannot chop.
//! - **what a job produces**: its recipes and their outputs are data
//!   (`data/minecolonies/recipes/`, one file per recipe), not code, and a recipe
//!   format is its own slice.
//! - **the request system**: a tool request here is a [`ToolRequest`], a kind and
//!   two levels; turning it into an `IRequest` a courier carries is
//!   `crate::inventory`'s neighbour and does not exist either.
//! - **hiring and firing**: [`employ`] answers "may this hut take this job", not
//!   "assign this citizen": the citizen record lives in [`crate::colony`] and the
//!   two are not wired.
//! - **the model**: every job names a `ResourceLocation` render model
//!   (`IJob.getModel`), which is a client asset and not a rule.

pub mod employment;
pub mod kinds;
pub mod experience;
pub mod job;
pub mod tool;

pub use employment::{
    Capacity, EMPLOYMENTS, Employment, Refusal, Skill, Worker, employ, employees,
};
pub use experience::{
    LEVELS_PER_HOME_LEVEL, MAX_CITIZEN_LEVEL, Progress, gain, skill_cap, total_xp_for_level,
    xp_needed_for_next_level,
};
pub use job::Job;
pub use tool::{
    BASIC_TOOL_LEVEL, TOOL_LEVEL_HAND, TOOL_LEVEL_MAXIMUM, TOOL_LEVEL_WOOD_OR_GOLD, ToolKind,
    ToolRequest, WOOD_HUT_LEVEL, max_equipment_level, quality, request,
};
