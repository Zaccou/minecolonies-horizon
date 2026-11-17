//! Three managers of Java's tree, and the one that was already here.
//!
//! Java reference: `api/sounds/SoundManager.java` (127 lines),
//! `core/client/render/worldevent/HighlightManager.java` (132),
//! `core/colony/CitizenDataManager.java` (40) + `api/colony/ICitizenDataManager.java`
//! (37), and `core/colony/ColonyManager.java` (875).
//!
//! | File | Question | Java |
//! | --- | --- | --- |
//! | `sound` | who speaks, how long a voice waits, how many voices a tick may advance | `SoundManager.java:13-127` |
//! | `highlight` | what a client draws in the world, for how long, and in which order it starts, draws and stops | `HighlightManager.java:8-132` |
//! | `views` | what a client keeps per citizen, and how an update reaches it without losing the rest | `CitizenDataManager.java:22-39` |
//!
//! **Two of the four briefed managers are not ported here, on purpose, and this is
//! the answer rather than a duplication.**
//!
//! 1. **`ColonyManager` is already ported**, file by file, in `crate::manager`:
//!    `index.rs` answers `getColonyByPosFromWorld`, `getBuilding`,
//!    `getClosestColony`, `getIColonyByOwner`, `isCoordinateInAnyColony` and the
//!    deletions; `claims.rs` the chunk capability; `proximity.rs`
//!    `isFarEnoughFromColonies` and `getMinimumDistanceBetweenTownHalls`;
//!    `lifecycle.rs` the away timer and `Dissolution`; `persist.rs` `write`/`read`.
//!    Its own `MANQUANT.md` already names what the client half would need.
//!    Writing a second `ColonyManager` here would be two answers to "who owns this
//!    position", which is the one thing that file exists to answer once.
//! 2. **The file half of `CitizenDataManager` — `createFromNBT` (`:14-20`) — is
//!    already ported** in `crate::persistence::roster::read`: the id, Java's
//!    `getInt` answer of 0 for an absent id, the refusal of an id no factory hands
//!    out, and what is read beside it. `views.rs` ports the *other* half, the
//!    network one, and its module note keeps the difference between the two visible
//!    (a bad citizen costs the colony on disk, and only itself over the wire).
//!
//! **What is left is the client's own bookkeeping**, which is why all three files
//! are pure values: a queue, a map of timers, a map of views, each advanced by a
//! caller that has the game in hand. No host call, no clock, no thread, no file —
//! the same bargain `crate::manager`, `crate::colony` and `crate::founding` made,
//! and the only one a wasm mod can keep.
//!
//! **The same three divergences, deliberately, in all three files.** Java walks
//! `HashMap`s, so its order is unspecified; a port that keeps that cannot be
//! tested. Each map here is a `BTreeMap`, and each module note says which Java
//! behaviour that pins down. A consequence is stated where it matters rather than
//! hidden: `sound`'s four-voices-per-tick budget now starves the same voices every
//! time, where Java would starve a random four.
//!
//! **Nothing here is wired to a hook.** `crate::lib` has no client tick and no
//! render hook in the five it subscribes to, so these three managers are values
//! the client layer will own the day one exists; `MANQUANT.md` says what each
//! would need and what it costs to have nothing.

pub mod highlight;
pub mod sound;
pub mod views;

pub use highlight::{Highlight, Highlights, Step, TICKS_PER_SECOND};
pub use sound::{MAX_CONCURRENT_SOUNDS, Playback, QUEUES_PER_TICK, SoundQueue, Speaker, TimedSound};
pub use views::{Refusal, Views};
