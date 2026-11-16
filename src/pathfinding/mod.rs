//! Pathfinding: find a path, and hand it to the engine.
//!
//! **One sentence.** Given a `WorldView` and two block positions, [`plan`]
//! returns a path the engine can make an entity follow, or a [`Refusal`] that
//! names what stopped it.
//!
//! **Why this is ours.** The API contract is "a mod supplies a finished path,
//! the engine makes the entity follow it" (`arismoko/horizon#4443`, merged),
//! and `crates/horizon-mod-vocab/src/host/mobs.rs` stops at `move_to`,
//! `move_to_path` and `stop` — no `NodeEvaluator`, no `PathNavigation` provider
//! (`:300-344`). There is no engine-side A\* to lean on, so this module is the
//! search itself.
//!
//! | File | Question |
//! | --- | --- |
//! | `node` | what a node is, the five kinds MineColonies prices apart, and what a step costs |
//! | `world` | the four questions a world answers, and the rules read from them |
//! | `search` | the A\*: frontier, ceiling, goal test, named refusal |
//! | `validate` | is the path *followable* — the check Java never needs and a mod does |
//! | `fake` | a ten-by-ten world in memory, for the tests |
//!
//! **No host call anywhere in this module**, exactly like `colony` and
//! `inventory`: it carries data and rules, and the caller supplies the world.
//! That is not a limitation of the design but the reason the module is worth
//! writing — every rule below is checked on the host target, with no game, no
//! wasm and no prepared resources, and the day a hook calls
//! [`plan`] it passes a `WorldView` built on `level_block_state`
//! (`crates/horizon-mod-vocab/src/host/levels.rs:304-331`).
//!
//! **Java reference, by file.** `core/entity/pathfinding/` — `AbstractPathJob`
//! (1 976 lines, the search), `PathJobMoveToLocation` (168, the goal test and
//! the ceiling of a walk-to job), `PathingOptions` (394, every price),
//! `SurfaceType` (134, what a block is to a walker), `MNode` (407, the node and
//! its ordering), `PathPointExtended` (168, the ladder and swim flags).
//!
//! **What this module does not do**, each with what it would cost:
//!
//! - **No rails** (`PathingOptions.canUseRails`, `:88`): a minecart path needs
//!   `BaseRailBlock` and its shape, which the world view does not describe.
//! - **No corner nodes** (`AbstractPathJob.java:786-840`): Java inserts an air
//!   node so a diagonal step over a gap has a parent to hang from. A path of
//!   cardinal steps needs none, and [`validate`] accepts none.
//! - **No on-path discount** (`PathingOptions.onPathCost`, `:28`): it needs
//!   `WorkerUtil.isPathBlock` (`AbstractPathJob.java:864`), a colony question,
//!   not a block one.
//! - **No randomness** (`randomnessFactor`, `:83`): Java jitters every step to
//!   spread a crowd of walkers. Determinism is worth more than spread here.
//! - **No best-effort path**: Java returns `finalizePath(bestNode)` when its
//!   ceiling is hit (`:506`); a mod that cannot replan behind the engine's back
//!   refuses instead — see [`Refusal::NodeLimit`].
//! - **No immutable chunk snapshot**: the search reads the world through
//!   whatever [`WorldView`] the caller has, on the caller's thread. Whether
//!   that is a copy of the chunks or a live read is the caller's problem, and
//!   `RAPPORT_PORT_PATHFINDING.md` says what the engine would have to offer to
//!   make it a copy.

mod node;
mod search;
mod validate;
mod world;

#[cfg(test)]
pub(crate) mod fake;

pub use node::{
    Costs, HORIZONTAL, MAX_COST, MAX_DROP, MAX_DROP_FORBIDDEN, MAX_JUMP_HEIGHT, MAX_JUMP_STEP,
    MAX_NODES, Node, NodeKind, Pos, Surface, at, down, offset, up,
};
pub use search::{Limits, Path, Refusal, Request, SLACK_SQUARED, plan, plan_with};
pub use validate::{PathDefect, is_walkable_path, is_walkable_with};
pub use world::WorldView;
