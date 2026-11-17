//! Four managers of a colony's event side: the event book, the terrain an event
//! leaves behind, the graves, and the buildings that survive both.
//!
//! Java reference: `core/colony/managers/EventManager.java` (271 lines),
//! `EventStructureManager.java` (212), `GraveManager.java` (332) and
//! `RegisteredStructureManager.java` (919).
//!
//! | File | Question | Java |
//! | --- | --- | --- |
//! | [`events`] | what an event's save holds, and who an entity belongs to | `EventManager.java:68-264` |
//! | [`structure`] | which anchor a backup is filed under and when the ground is given back | `EventStructureManager.java:83-211` |
//! | [`graves`] | where a grave is, which one is taken, what a death puts in one, when it rots | `GraveManager.java:70-331`, `TileEntityGrave.java:228-251` |
//! | [`registry`] | the buildings, the four lists derived from them, the chunk bounds, the extensions | `RegisteredStructureManager.java:128-857` |
//! | [`nbt`] | the six tag reads the four share, and why they are a copy | — |
//!
//! **What was already covered, and is therefore not repeated here.**
//!
//! - **`EventManager`'s live half is `crate::events::raid`**: `Lifecycle` holds
//!   the map and runs `onColonyTick`'s four outcomes, `RaidStep` names them, and
//!   `take_from` is the id counter's arithmetic. [`events::EventBook`] is the other
//!   half — the compound, the counter's own round trip, the entity routing — and
//!   it *calls* `take_from` rather than restating the wrap.
//! - **`EventDescriptionManager` is `crate::manager_04::events`**, a different
//!   class with a different ring (a hundred descriptions, a day, a summary). Not
//!   one key is shared: that one saves `event_descs_list`, this one `events_list`.
//! - **A building record is `crate::building`**, a saved `type` is
//!   `crate::manager_02::from_record`, a claim is
//!   `crate::building::behaviour::claim_radius`, and an extension is
//!   `crate::manager_02::Extension`. [`registry::Registry`] is the *collection*
//!   over them and re-reads nothing.
//! - **The colony's own flat list is `crate::colony::Colonie::buildings`**, which
//!   carries `(type, location, level)` and no index. A save being read by both is
//!   a fact of the port, not a rule from Java: Java has one manager and no second
//!   list.
//!
//! **Four premises of the brief, checked against the sources.**
//!
//! 1. **"Port the rules, not the world" holds for all four**, with one cost each,
//!    written in `MANQUANT.md`: the blueprint writer/reader (`structure`), the
//!    block writes and the item transfer (`graves`), the packet half and the
//!    random leisure ladder (`registry`), and the live map's tick (`events`, which
//!    delegates).
//! 2. **"The order of the writes" is a rule, and each file proves its own.** A
//!    refused `addEvent` writes nothing (`events`), a refused backup is refused
//!    before its anchor exists (`structure`), every grave refusal is answered
//!    before the block is placed and the registration is the **last** of seven
//!    lines (`graves`), and a taken position is refused before any list or bound
//!    moves (`registry`). Each file exposes a `writes()` tally — Java's
//!    `markDirty` — so a test can say "nothing was saved", not merely "nothing
//!    changed".
//! 3. **"The managers are four independent slices" is false for two of them.**
//!    `EventStructureManager.readFromNBT` is called **from inside**
//!    `EventManager.readFromNBT` (`:244`), so a colony compound with no
//!    `event_manager` never reads its backups — they are lost silently.
//!    [`structure::Backups::read_from_colony`] checks that guard itself and
//!    answers `read: false`. And `EventManager.writeToNBT` writes the structure
//!    manager on the **colony** compound (`:263`) while its own state goes inside
//!    `event_manager`: two levels, one method.
//! 4. **"A cancelled event leaves nothing behind" is false.** `CANCELED` is
//!    removed without `onFinish` (`EventManager.java:202-206`), so its backups
//!    are neither restored nor forgotten; the next load finds an event the save
//!    does not hold and **restores the terrain then**. A test holds it in
//!    [`structure`], and it is the one finding of this slice that changes what a
//!    player sees.
//!
//! **Pure data, pure rules, the same bargain as `crate::manager`,
//! `crate::colony`, `crate::events` and `crate::manager_03`**: no host call, no
//! clock, no thread, no file. What a manager needs from the world arrives as a
//! closure — a [`graves::Slot`], a [`graves::Decay`], a
//! [`registry::Occupancy`], an "is there a block entity here" — and what it owes
//! the world comes back as a value the caller executes: a
//! [`structure::Plan`], a list of anchors to place back, a
//! [`graves::Deposit`] log.

pub mod events;
pub mod graves;
pub mod nbt;
pub mod registry;
pub mod structure;

pub use events::{Event, EventBook, IdCounter, Loaded, Route, Skipped, Refusal as EventRefusal};
pub use graves::{Added, Decay, DecayStep, Deposit, Graves, Load, Medium, Refusal as GraveRefusal, SiteRefusal, Slot, Ticked, TickedRemoval};
pub use registry::{AddRefusal, ChunkBounds, Occupancy, Registry, Removal, Read as RegistryRead};
pub use structure::{Backups, Plan, Read as StructureRead, Refusal as StructureRefusal};
