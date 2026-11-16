//! The 128 network messages, as data.
//!
//! Java reference: `com.minecolonies.core.network.messages` —
//! `src/main/java/com/minecolonies/core/network/messages/`, **128 files** measured
//! on the tree this slice was cut from. Each carries its own `toBytes`/`fromBytes`
//! pair and an `onExecute` that needs a server: opening a window, hiring a citizen,
//! placing a hut.
//!
//! | File | Family | Types |
//! | --- | --- | --- |
//! | `common` | the codecs, the refusal, the two envelopes | — |
//! | `permissions` | `PermissionsMessage`'s nine nested classes | 9 |
//! | `founding` | founding, reactivating, abandoning a colony; the town hall's own windows | 11 |
//! | `colony_settings` | name, styles, banner, colour, free-to-interact, mercenaries, spies, teleport | 11 |
//! | `buildings` | the twenty-one messages a hut answers directly | 21 |
//! | `building_modules` | the twenty a hut's module answers: fields, recipes, levels, filters | 20 |
//! | `work` | work orders, requests, deliveries, the clipboard's snapshot | 7 |
//! | `citizens` | skills, pausing, recalling, restarting, the two interaction answers | 8 |
//! | `views` | the colony view stream: `ColonyViewMessage` and its thirteen companions | 14 |
//! | `particles` | the eight particle effects | 8 |
//! | `sounds` | music, sound events, the stop | 4 |
//! | `windows` | the eight "open this screen" messages | 8 |
//! | `items` | items, tools, banners, recipes, blueprint saving | 9 |
//! | `debug` | paths, chunk claims, the two global syncs, split packets | 8 |
//!
//! **What every type here is.** A plain struct, its fields public, and
//! [`common::Message`]: `to_compound` writes every field under a **camelCase key
//! named after the Java field** (`colonyId`, `buildingId`, `dimensionId`), and
//! `from_compound` reads them back or returns a named
//! [`common::Refusal`]. `from_compound(to_compound(m)) == m` is the contract, and
//! every type in this module is round-tripped by a test.
//!
//! **What no type here is.** Not a packet: there is no socket, no
//! `FriendlyByteBuf`, no `NetworkEvent.Context`, and **no `onExecute`**. What Java
//! does with a message needs a `Host` — open a screen, spawn a mob, mutate a
//! colony — and the port's layer for that is not this one. This module answers
//! *what a message carries* and *what it refuses*, which is the part that can be
//! tested with no game at all. `MANQUANT.md` lists what the port cannot express,
//! field by field.
//!
//! **The three cuts that shape the rest**, each argued where it happens:
//!
//! 1. **A payload this port cannot decode is carried as bytes.** Fourteen messages
//!    carry a `FriendlyByteBuf` another class wrote — the whole colony view
//!    (`ColonyViewMessage.java:76-84`), a building's view, a citizen's, the work
//!    orders, the research tree, the building extensions, the visitors, the
//!    animals, the compatibility manager, the quest and disease packs. Those
//!    encoders are separate ports that do not exist yet, so the field is
//!    `Vec<u8>`, [`common::bytes`], and the message says so instead of pretending
//!    to hold a colony. `MANQUANT.md` §1 names the fourteen and their producer.
//! 2. **An enum is a name, never an ordinal.** See `common`'s module note: Java
//!    writes `ordinal()` and its own `RequestState` warns what that costs
//!    (`RequestState.java:84-90`). The two enums that are *vanilla's*
//!    (`ChatFormatting`, `SoundSource`) stay integers, because the port owns no
//!    type for them.
//! 3. **A permission gate is data, not a method.** Java's
//!    `AbstractColonyServerMessage.permissionNeeded()` is `MANAGE_HUTS` unless a
//!    subclass says otherwise; here it is [`common::Message::RULE`], one const per
//!    type, so a reader can see which messages gate on what without opening a
//!    window.

pub mod building_modules;
pub mod buildings;
pub mod citizens;
pub mod colony_settings;
pub mod common;
pub mod debug;
pub mod founding;
pub mod items;
pub mod particles;
pub mod permissions;
pub mod sounds;
pub mod views;
pub mod windows;
pub mod work;

pub use common::{Component, ItemStack, Message, Refusal, Rule};

#[cfg(test)]
mod census;
