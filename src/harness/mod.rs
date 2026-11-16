//! Exercising the port without launching the game.
//!
//! The slice is tested through the host the engine gives a hook; this module makes
//! that host scriptable in a unit test: a fake level with blocks, block entity data
//! and mobs, a fake player, a record of every call the port makes, and a scenario
//! that replays "place the hut, click it, hire".
//!
//! **What is faked, and what is not.** [`FakeHost`] implements the eighteen host
//! calls `hut`, `menu`, `hire` and `citizen` make, plus `list_levels` (the dimension,
//! which the harness asks for itself), and no other. Every method it leaves out keeps
//! the default the SDK's `#[host_area]` macro writes — a panic naming the method and
//! the load phase it needs (`crates/horizon-mod-api-macros/src/lib.rs:285-334`). That
//! is the `unimplemented!()` a fake host should have: regenerated from the
//! declarations, it cannot drift, and it fails in the test that made the call instead
//! of quietly answering something the game would refuse.
//!
//! Three of the four handle types have no mod-facing constructor and are built
//! through the game's own hidden ones — `__private::game::{block_entity_ref,
//! player_ref}`, `EntityRef::new`, `MobRef::new` — and `BlockKey` through
//! `__private::parts_installation::block_key`. `RAPPORT_PORT_HARNESS.md` says which
//! and why; nothing here adds a line to `crates/`.

mod calls;
mod host;
mod level;
mod scenario;
#[cfg(test)]
mod tests;

pub use calls::Call;
pub use host::{FakeHost, PLAYER_ENTITY};
pub use level::{EPOCH, FakeBlock, FakeLevel, FakeMob, block_key, dimension_key};
pub use scenario::Scenario;
