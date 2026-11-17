//! The colony's hand on the world: four writes, one order, one refusal, one claim.
//!
//! Java reference: `core/colony/Colony.java` — the operations that touch the world, of
//! which this version has exactly one that matters, `isCoordInColony`
//! (`:1298-1308`), plus the state a ticking colony publishes
//! (`isActive`, `:1903-1906`) — and `api/util/BlockPosUtil.java` — the primitives every
//! one of them goes through: `setBlock` (`:664-667`), `getBlockState` (`:650-654`), the
//! chunk box (`:591-598`) and the distance (`:469-481`).
//!
//! | File | Question |
//! | --- | --- |
//! | `refusal` | **why** a write was not made, named, one name per reason |
//! | `guard` | the five questions asked before one write, and the order they are asked in |
//! | `intent` | the four operations, the batch, and the order it is handed over in |
//! | `engine` | which half carries a change out, and what the engine cannot carry |
//! | `mark` | claim and unclaim: what changes for [`crate::manager`] |
//!
//! **What this module decides, and what it never does.** It decides *whether* a colony
//! may write a cell, *which* of the four operations that is, and *in what order* a batch
//! reaches the engine. It never writes: no host call, no clock, no thread, no file —
//! the same rule `crate::colony` and `crate::manager` keep, and the reason every rule
//! here is provable on the host target with no game and no world.
//!
//! **The premise this module was asked to build on is partly false, and here is where.**
//! `place_template` — the engine's own paste (`crates/horizon-world/src/level/bonemeal.rs:162`,
//! `horizon-client/src/game/administration/structure_blocks.rs:384`) — is **not reachable
//! from a mod**: it is not a host method, not a hook, and the SDK's `world_edit` area
//! declares only `edit_blocks`, `structure_template`, `surface_heights`,
//! `read_columns` and the two NBT upgrades
//! (`crates/horizon-mod-vocab/src/host/world_edit.rs`). A mod reads a template with
//! `structure_template` and writes blocks with `edit_blocks`, and that is the whole
//! surface. So the split this module keeps is not "decision here, paste there": it is
//! **decision here, `edit_blocks` there, and a paste that a caller has to reassemble
//! from a snapshot** — losing the block entities' data, the template's entities, the
//! neighbour updates and the chunk tickets. `engine` names each loss with its line, and
//! `MANQUANT.md` §1 lists them with what was wanted.
//!
//! **Nothing about a colony's land is decided in this module.** Who owns a chunk is
//! [`crate::manager`]'s (`Manager::owner_at`); [`guard::Claim`] is that question and
//! `Manager` is the only implementation that answers it for real. Nothing about a
//! building is decided here either: [`crate::building`] owns what a hut is.
//!
//! **Two guarantees worth stating apart from the rest.**
//!
//! 1. **No write before a refusal.** [`intent::Intent`] has one constructor,
//!    [`intent::Intent::plan`], and it either accepts the whole batch or answers the
//!    first refusal and builds nothing. A sink can only be reached through an `Intent`,
//!    so a caller cannot write half a batch by forgetting a check.
//! 2. **One order.** An accepted batch is handed over by `y` then `x` then `z`
//!    ([`intent::order`], the order `structure_template` reports a template in), so two
//!    calls that name the same cells hand the engine the same sequence.

pub mod engine;
pub mod guard;
pub mod intent;
pub mod mark;
pub mod refusal;

#[cfg(test)]
pub(crate) mod fakes;

pub use engine::{AIR, Handled, Sink, UPDATE_FLAG, handled};
pub use guard::{Claim, Guard, Occupant, Open, Protection, State, World};
pub use intent::{Change, Intent, RefusalAt, key, order};
pub use mark::{Effect, Mark, Marks};
pub use refusal::{Refusal, Stop};
