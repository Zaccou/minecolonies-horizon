//! Four managers of Java's `core/colony/managers/`, as rules.
//!
//! Java reference: `CitizenManager.java` (733 lines), `ColonyConnectionManager.java`
//! (805), `ColonyPackageManager.java` (323), `EventDescriptionManager.java` (150) —
//! **2 011 lines** between them, of which this module ports the tables, the thresholds,
//! the refusals and the order of the writes; not the world.
//!
//! | File | Question | Java |
//! | --- | --- | --- |
//! | [`citizens`] | which id is free, how many may live here, when the next one arrives, who mourns | `CitizenManager` |
//! | [`connections`] | how far a sign reaches, when a chain is broken, when two colonies link | `ColonyConnectionManager` |
//! | [`subscribers`] | who is watching, what they are sent, what a send forgets | `ColonyPackageManager` |
//! | [`events`] | the hundred events a colony keeps, and the evening summary | `EventDescriptionManager` |
//!
//! **Three of the four briefed managers are partly covered elsewhere, and this module
//! connects to those answers instead of writing a second one.**
//!
//! | What | Where it already is | What is left here |
//! | --- | --- | --- |
//! | the building walk that counts the beds (`:423-459`) | `crate::building::behaviour::census` | the three-way clamp, the id pool, the timers |
//! | the away timer (`:141-160`) | `crate::manager::lifecycle::Attendance`, with its one-update-late hour | [`subscribers::Subscribers::watched`], the bit it reads |
//! | the four view payloads (`:165-257`) | `crate::network::{views, permissions, work, citizens}` | the recipients, and which dirty flag a send clears |
//! | a citizen's own record (`:173-205`) | `crate::persistence::roster` and `crate::citizens` | the five facts the manager's rules read |
//! | `DiplomacyStatus` and `ConnectionEventType` | `crate::ui_window_14::alliance::{Status, EventKind}`, ordinals and refusals included | the maps and the writes |
//!
//! **`ColonyManager` is not here**: `crate::manager` already ports it file by file
//! (`index`, `claims`, `proximity`, `lifecycle`, `persist`), and a second answer to "who
//! owns this position" is the one thing that module exists to give once.
//!
//! **Every file is a value, not a service.** No host call, no clock, no thread, no file —
//! the same bargain `crate::manager`, `crate::colony` and `crate::founding` made, and the
//! only one a wasm mod can keep. What a caller supplies is the world: the day
//! ([`events::Events::set_day`]), the path result ([`connections::Connections::resolve`]),
//! the sightings ([`subscribers::Sighting`]), the tick ([`citizens::Phase`]).
//!
//! **The order of the writes is a rule in three of the four files**, because each has a
//! place where Java writes *before* it can refuse, or refuses *before* it writes:
//!
//! * [`connections`] — a sign is written to the network only after the chain check
//!   (`:229-238`), and the two colonies are linked only after four checks (`:341-430`);
//!   but a notice is recorded **before** the second colony is asked (`:732` against
//!   `:773`), and a port that swapped those two would lose the event on a broken link.
//! * [`subscribers`] — six dirty flags are cleared blindly and one is cleared only by the
//!   send that built its packet (`:189-199` against `:255`).
//! * [`citizens`] — a removal that names nobody touches no record, and the two setters of
//!   the count run only when the *clamped* number changed (`:453-458`).
//!
//! Each of those is a test, in the file that owns it. The facts that came out **wrong**
//! in Java rather than merely different — the chain walk that stops one step early, the
//! `FIX_PATH` node that never joins the network, the path that retries for ever, the
//! reloaded node whose target becomes colony 0 — are in the module note of
//! [`connections`] and in [`MANQUANT.md`](MANQUANT) §2, and none of them is silently
//! fixed: a port that repairs what a running server does becomes a second behaviour to
//! debug.

mod nbt;

pub mod citizens;
pub mod connections;
pub mod events;
pub mod subscribers;

pub use citizens::{Ceiling, Departure, Effect, Head, Manager as CitizenManager, Mourning, Phase, Rest};
pub use connections::{
    Change, Connect, Connections, Link, Node, Notice, Peer, Pending, PendingKind, Plan as LinkPlan,
    Reciprocal, Resolved, Severed,
};
pub use events::{Description, Events, Kind, Loaded, News, Skipped, Summary};
pub use subscribers::{
    Admission, Culled, Flags, Plan as SendPlan, Recipients, Sighting, Subscribers, Who,
};
