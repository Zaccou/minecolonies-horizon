//! The 128 network messages, indexed: which side runs each one, what it carries, what it
//! does, and what the layer under it must offer for it to be done.
//!
//! **The four files, and what each answers.**
//!
//! | File | Answers |
//! | --- | --- |
//! | [`catalogue`] | 134 rows: file, class line, side, fields, verb, Rust type |
//! | [`effects`] | 27 verbs, 28 capabilities, and the verdict on each capability |
//! | [`bounds`] | the checks the *receiver* makes inside `onExecute`, as pure functions |
//! | [`fragments`] | `SplitPacketMessage`: the reassembly Java keeps in a static cache |
//!
//! **What was already there, and what was not.** [`crate::network`] (18 files, 8 760 lines
//! when this module was written) already carries the *fields* of all 134 types, with
//! `to_compound`/`from_compound` and 119 tests; it says so itself and its census proves it.
//! So this module does not re-code a single message — a second codec of a field is a bug
//! waiting to disagree with the first. What no codec can carry is:
//!
//! - **the side** (`getExecutionSide`, or the base a class inherits it from), which is a
//!   property of the message and not of its bytes;
//! - **the verb** (`onExecute`), which needs a `Host` and therefore cannot live in a codec;
//! - **the capability** that verb needs, which is the question a port asks when it cannot
//!   copy 95 lines of colony code;
//! - **the bound** the receiver applies *after* the bytes are read, which the codec
//!   deliberately does not apply (a codec refuses a wrong `int`; 40 is a right `int`);
//! - **the reassembly** of a split message, which is state, not a format.
//!
//! **The arithmetic, checked by [`census`].** 128 `.java` files, of which 126 are messages
//! and 2 are the abstract bases; one message file holds nine nested classes, so 125 + 9 =
//! **134 rows**. The verbs are painted by those 134 rows and no verb is left unused
//! (`every_effect_of_the_vocabulary_is_used_by_a_row`). The 28 capabilities are answered
//! one by one, and **5 are absent, 3 partial** — `MANQUANT.md` is their table, and it is
//! computed from [`effects::absent`] rather than typed twice.
//!
//! **What this module does not do.**
//!
//! - **It does not encode.** [`crate::network`] does, once.
//! - **It does not execute.** No `Spawn`, no `Teleport`, no `open_menu` call: the effect
//!   layer is the port's own, with its own tests, and this module only says which of its
//!   capabilities a message needs.
//! - **It does not register or send a payload.** `PayloadRegistrar`,
//!   `player_send_payload` and `send_payload_to_server` exist
//!   (`crates/horizon-mod-vocab/src/payload.rs:11-14`,
//!   `host/payloads.rs:11-26`); wiring 134 messages onto them is a session's job, and
//!   `MANQUANT.md` §4 says what a dispatcher still needs.
//! - **It does not copy Java's `onExecute` bodies.** `RAPPORT.md` lists the seven places
//!   where copying them would have copied a defect.

pub mod bounds;
pub mod catalogue;
pub mod effects;
pub mod fragments;

pub use catalogue::Side;
pub use effects::{Effect, Finding, Need, Verdict};

#[cfg(test)]
pub(crate) mod census;
