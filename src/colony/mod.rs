//! The colony: its record, its permissions, its work orders.
//!
//! Java reference: `com.minecolonies.core.colony.Colony` and its managers
//! (`src/main/java/com/minecolonies/core/colony/`), 30 043 lines with the
//! permissions, the events and the work orders.
//!
//! Nothing here talks to the game yet: this module owns the data model and the
//! rules, and the hooks in `lib.rs` will call into it.
//!
//! | File | Question |
//! | --- | --- |
//! | `colonie` | what a colony *is*, and its compound |
//! | `permissions` | who may do what, and the named refusal that says why not |
//! | `ordre` | what a work order is, and which state changes it accepts |
//! | `nbt` | the tag reads and writes the three others share |
//!
//! **Pure data, pure rules**: no host call, no clock, no thread, no file —
//! nothing a wasm mod could do anyway, and what lets every rule be tested on the
//! host target with no game, no wasm and no prepared resources (`src/tests.rs`
//! makes the same choice for the first slice).
//!
//! **Storage is a `Compound`**: `BTreeMap<JavaString, Tag>`
//! (`crates/horizon-core/src/nbt.rs:35`), which is what a block entity saves,
//! what `player_persistent_data` returns
//! (`crates/horizon-mod-vocab/src/host/players.rs:372`) and what an attachment
//! carries (`crates/horizon-mod-vocab/src/wire/wire_types.rs:1702`).

mod nbt;
pub mod colonie;
pub mod ordre;
pub mod permissions;

pub use colonie::{Building, Citizen, Colony, OrderRefusal};
pub use ordre::{OrderState, TransitionRefusal, WorkOrder, WorkType};
pub use permissions::{Action, Members, Rank, Refusal};
