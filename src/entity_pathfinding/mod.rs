//! Pathfinding, second pass: **the rules a search is asked with**.
//!
//! The first version (`crate::pathfinding`) is the search itself — nodes, costs,
//! the A\*, the walkability check. This module is what Java wraps around it: the
//! fourteen jobs and their goal tests, the node ceilings they get, the block
//! classification `PathfindingUtils` answers, where a body really starts, the
//! result state machine, the recent-target cache and the stuck handler. Both
//! sides share the same types ([`crate::pathfinding::Costs`], `Pos`, `Surface`),
//! so a job here plans with the module there.
//!
//! Java reference: `core/entity/pathfinding/` — 39 files, 10 243 lines.
//!
//! | File | Question | Java |
//! | --- | --- | --- |
//! | [`job`] | what each of the fourteen jobs aims at, and at what price | `pathjobs/*` |
//! | [`ceiling`] | how many nodes a job may visit, and when it may ask for more | `AbstractPathJob` constructors, `stopOnNodeLimit` |
//! | [`classify`] | what a block is: surface, water, lava, ladder, danger, passability, line of sight | `SurfaceType`, `PathfindingUtils` |
//! | [`start`] | which cell a body standing in a fence, a carpet or water starts from | `PathfindingUtils.prepareStart` |
//! | [`result`] | computing, following, reached, failed, cancelled | `PathResult`, `PathFindingStatus` |
//! | [`recent`] | how a crowd spreads over targets it has already used | `RecentTargetCache` |
//! | [`stuck`] | when a body is stuck, and what it may do about it | `navigation/PathingStuckHandler` |
//! | [`parity`] | the first version's numbers against the Java they claim | `PathingOptions`, `MNode`, `AbstractPathJob` |
//!
//! **What is not here**, named with its cost:
//!
//! - **The navigator** (`MinecoloniesAdvancedPathNavigate`, 1 387 lines): it
//!   follows a path by moving an entity each tick and replans behind the
//!   engine's back. The API contract is a finished path handed to the engine
//!   (`arismoko/horizon#4443`), so there is nothing to drive — `MANQUANT.md` §1.
//! - **The chunk cache** (`world/ChunkCache`, `CachingBlockLookup`): a snapshot
//!   of the blocks a job may read off-thread. The caller supplies a `WorldView`
//!   instead; whether it copies is theirs to decide.
//! - **`PathingStuckHandler`'s world writes** (place ladders, build leaf
//!   bridges, break blocks, teleport): refused *by name* in [`stuck`], because
//!   the SDK has no verb that would do it well — `MANQUANT.md` §3.
//! - **The walk-to proxy** (`proxy/*`, 679 lines): for a target further than
//!   400 blocks², Java walks to a chain of intermediate positions. It needs the
//!   entity's position each tick, which is the navigator again.

mod ceiling;
mod classify;
mod job;
#[cfg(test)]
mod parity;
mod recent;
mod result;
mod start;
mod stuck;

pub use ceiling::{Ceiling, Stop, StopPolicy, Watch};
pub use classify::{
    Barrier, BlockFacts, ClimbFacts, Danger, FLAT, FULL_WIDTH, Fluid, LOW, TALL, WaterFacts,
    enters_door, enters_gate, has_any_collision_along, is_dangerous, is_ladder, is_lava, is_water,
    passes, surface_of,
};
pub use job::{
    Box3, GoalFacts, Job, Options, POND_SEPARATION, Reach, Step, distance, manhattan, sq,
    sq_at_height,
};
pub use recent::{JUST_USED_PENALTY, REMOVALS_PER_CLEAN, RecentTargets, TARGET_TTL};
pub use result::{Progress, Reached, Status};
pub use start::{Start, StartDefect, StartWorld, prepare_start};
pub use stuck::{Action, StuckHandler, Tick, Unstuck};
