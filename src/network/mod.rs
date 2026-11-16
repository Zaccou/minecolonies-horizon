//! The network messages, as data.
//!
//! Java reference: `com.minecolonies.core.network.messages` —
//! `src/main/java/com/minecolonies/core/network/messages/`, **128 `.java` files**
//! measured on the tree this slice was cut from
//! (`find … -name '*.java' | wc -l`). Each carries a `toBytes`/`fromBytes` pair and an
//! `onExecute` that needs a server: opening a window, hiring a citizen, placing a hut.
//!
//! | File | Family | Types |
//! | --- | --- | --- |
//! | `common` | the eleven field codecs, the refusal, the two envelopes, the four shared values | — |
//! | `permissions` | `PermissionsMessage.java`'s nine nested classes | 9 |
//! | `founding` | founding, picking up, reactivating, abandoning a colony; the town hall's four windows | 11 |
//! | `colony_settings` | name, three styles, banner, colour, free-to-interact, mercenaries, spies, connection event, gate travel | 11 |
//! | `buildings` | the twenty-one messages a hut answers directly | 21 |
//! | `building_modules` | the twenty a hut's module answers: fields, recipes, levels, filters | 20 |
//! | `work` | work orders, request state, deliveries, recipes, the two blueprint requests | 9 |
//! | `citizens` | skills, pausing, recalling, restarting, the two dialogue answers | 6 |
//! | `views` | the colony view stream, its thirteen companions, one server-side "open this hut" | 14 |
//! | `particles` | the eight particle effects | 8 |
//! | `sounds` | two jingles, the stop, music and a citizen's voice | 4 |
//! | `windows` | the seven "open this screen" messages | 7 |
//! | `items` | the six messages an item sends for itself | 6 |
//! | `debug` | paths, chunk claims, three whole payloads, the fragment carrier | 8 |
//! | `census` | the 128 files, counted and checked | — |
//!
//! **134 types for 128 files, and the arithmetic closes.** 126 files are messages; one
//! of them, `PermissionsMessage.java`, is a namespace holding nine nested classes
//! (`PermissionsMessage.java:35-723`), which is `125 + 9 = 134` types. The other two
//! files are the abstract bases — `AbstractColonyServerMessage.java` and
//! `AbstractBuildingServerMessage.java` — and they are **not types here**: their two
//! fields are [`common::envelope`] and [`common::building_envelope`], their gate is
//! [`common::Message::RULE`], and their range constants are [`common::RANGE`] and
//! [`common::BUILDING_RANGE`]. `census` asserts all of it.
//!
//! **What every type here is.** A plain struct with public fields, and
//! [`common::Message`]. `to_compound` writes every field under a **camelCase key named
//! after the Java field** (`colonyId`, `buildingId`, `colonyBuffer`, `visitorBuf`,
//! `questBuffer`); where Java writes an *unnamed* value — a count, a length, a
//! dimension's name — the port names it after what it is (`count`, `dimensionId`).
//! `from_compound` reads the keys back or returns a named [`common::Refusal`].
//! `from_compound(to_compound(m)) == m` is the contract, and every one of the 134 types
//! is round-tripped by at least one of the 119 tests below.
//!
//! **What no type here is.** Not a packet: there is no socket, no `FriendlyByteBuf`, no
//! `NetworkEvent.Context`, and **no `onExecute`**. What Java does with a message needs
//! a `Host` — open a screen, spawn a mob, mutate a colony — and the port's layer for
//! that is not this one. This module answers *what a message carries* and *what it
//! refuses*, which is the part that can be tested with no game at all. `MANQUANT.md`
//! lists what the port cannot express, field by field, and `RAPPORT.md` says what was
//! copied, what was cut, and what was found wrong in Java on the way.
//!
//! **The three cuts that shape the rest**, each argued where it happens:
//!
//! 1. **A payload this port cannot decode is carried as it arrived.** Fourteen fields
//!    hold what another class encoded: **twelve** `FriendlyByteBuf`s — the whole
//!    colony view (`ColonyViewMessage.java:79-83`), a building's view, a citizen's,
//!    the work orders, the building extensions, the visitors, the animals, the
//!    compatibility manager, the quest pack, the disease pack, a field's extension,
//!    and the permissions pane — carried as `Vec<u8>` ([`common::bytes`]); and **two**
//!    objects a factory serialized (a setting, a recipe), also bytes. Those encoders
//!    are separate ports that do not exist yet, so the message says so instead of
//!    pretending to hold a colony. `MANQUANT.md` §1 names each producer. **Not everything is opaque**: the research tree is one NBT
//!    compound (`ColonyViewResearchManagerViewMessage.java:56`), a saved schematic is
//!    NBT (`SaveStructureNBTMessage.java:78-85`) and a path debug node has five named
//!    fields (`MNode.java:102-136`), and all three travel as themselves.
//! 2. **An enum is a name, never an ordinal.** See [`common`]'s module note: Java
//!    writes `ordinal()` and its own `RequestState` warns what that costs
//!    (`RequestState.java:84-90`). The enums that are *vanilla's* — `ChatFormatting`,
//!    `SoundSource`, a block's state id, a job's registry id, a module's runtime id —
//!    stay integers, because the port owns no table for them and inventing one would
//!    be a claim it cannot back.
//! 3. **A permission gate is data, not a method.** Java's
//!    `AbstractColonyServerMessage.permissionNeeded()` is `MANAGE_HUTS` unless a
//!    subclass says otherwise; here it is [`common::Message::RULE`], one const per
//!    type, so a reader sees which messages gate on what without opening a window —
//!    and the three overrides (`EDIT_PERMISSIONS`, `PLACE_BLOCKS`, `null`) are visible
//!    where they happen instead of buried in an `onExecute`.
//!
//! **One thing this module cannot do, said once here**: it cannot *reassemble* a
//! `SplitPacketMessage` (`debug::SplitPacket`), because that needs the sender's message
//! table (`SplitPacketMessage.java:110`), which is the network layer's, not a codec's.

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
