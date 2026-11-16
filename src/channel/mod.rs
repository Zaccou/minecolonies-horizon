//! The channel: what turns 134 message types into a conversation.
//!
//! Java reference: `core/network/NetworkChannel.java` (the registration and the
//! order), `core/network/Network.java` (the one instance) and
//! `core/network/messages/splitting/SplitPacketMessage.java` (the fragment carrier).
//! [The registry](registry) holds the table; [the router](route) decides who may
//! treat what; [the splitter](split) cuts a body too big for one packet.
//!
//! **What was missing, said plainly.** [`crate::network`] carries the 134 types as
//! values — `to_compound`, `from_compound`, a permission [Rule](crate::network::Rule) —
//! and nothing that says *which* of them a received compound is, *which way* it
//! travels, or *who* may act on it. Java answers all three at registration
//! (`NetworkChannel.java:107-280` writes the index) and at execution
//! (`IMessage.getExecutionSide`, `AbstractColonyServerMessage.permissionNeeded`).
//! This module is that answer, as data: one table, one lookup, one named refusal
//! per way a packet can be wrong.
//!
//! **The four pieces, and the source of each.**
//!
//! | Here | Java | What it is |
//! | --- | --- | --- |
//! | [`registry::ALL`] | `registerCommonMessages` (`:107-280`), `setupInternalMessages` (`:282-294`) | 139 rows: id, Java class, port type, file, direction, range |
//! | [`route::Envelope`] | `SimpleChannel`'s index codec + `IMessage.toBytes` | a message id and its body |
//! | [`route::Channel`] | `SimpleChannel.send*` and `handleSplitting`'s class lookup (`:437-441`) | who may treat an envelope, from which side |
//! | [`split::Assembly`] | `SplitPacketMessage.onExecute` (`:85-153`) | fragments back into one body |
//!
//! **The count, and it closes.** 138 classes go through `registerMessage(++idx, …)`
//! and get ids 1 to 138; `SplitPacketMessage` is registered by hand on `id 0`
//! (`:284`). That is **139 ids**. [`crate::network`]'s census counts **134 types**
//! (`census.rs`), and 132 of them own one of those 138 ids — the two that do not are
//! `SplitPacket` itself (id 0, `registry::SPLIT_PACKET`) and `OpenBuildWindow`, an
//! abstract base whose two concrete subclasses are registered and whose own name
//! never is (`client/OpenBuildWindowMessage.java:16`, `registry::NO_ID`). Six further
//! ids are classes that live **outside** `core/network/messages/` — two in
//! `core/research/` and `core/colony/crafting/`, four in `core/debug/messages/` — so
//! the registry carries them with an empty port type and refusing them by name
//! ([`Refusal::NotPorted`]) is not the same answer as refusing an id nobody
//! registered ([`Refusal::UnknownId`]).
//!
//! **Three deviations from Java, each a bug the port refuses to copy.**
//!
//! 1. **The side check always runs.** Java tests
//!    `packetOrigin.equals(message.getExecutionSide())` inside
//!    `SplitPacketMessage.onExecute` (`:131-136`) — that is, *only for a message that
//!    arrived split*. A small message on the wrong side is executed without a word.
//!    [`Channel::receive`] asks every time.
//! 2. **A duplicate id is refused, not overwritten.** `registerMessage` calls
//!    `messagesTypes.put(id, …)` and `messageTypeToIdMap.put(clazz, id)`
//!    (`:304-308`): two messages on one id silently leave the first unreachable, and
//!    two ids for one class silently rebind it. [`Registry::from_entries`] answers
//!    [`Refusal::DuplicateId`] or [`Refusal::DuplicateName`].
//! 3. **A fragment that never arrived is refused, not skipped.** Java concatenates
//!    whatever reached the cache, sorted by index (`:103-107`), so a lost fragment
//!    yields a body that decodes as garbage; [`Assembly::accept`] answers
//!    [`Refusal::Gap`] when the terminator arrives with an index missing.
//!
//! **Two things this module cannot do, said here and in `MANQUANT.md`.**
//!
//! - It cannot turn a [`mod_api::Compound`] into the bytes a
//!   [`mod_api::PayloadBytes`] carries. The engine's network NBT codec exists
//!   (`horizon_core::nbt::write_network` / `read_network`) but `horizon-mod-api`
//!   re-exports only `NbtError` and `NETWORK_NBT_QUOTA` from it
//!   (`crates/horizon-mod-vocab/src/exports/horizon_core.rs:13`). So the body crosses
//!   this module as a compound, [the splitter](split) works on `&[u8]` as Java's does
//!   (`NetworkChannel.java:446-448` treats the serialized message as opaque bytes),
//!   and the one step between them is `MANQUANT.md` §1.
//! - It cannot *execute* a message. There is no `onExecute` here for the reason
//!   [`crate::network`] gives: opening a window or hiring a citizen needs a `Host`,
//!   and that is the session layer's business. [`route::Routed`] is the call that
//!   layer receives — the entry, the body, and the side it came from.

pub mod registry;
pub mod route;
pub mod split;

pub use registry::{Entry, MessageId, Registry, ALL, NO_ID, SPLIT_PACKET};
pub use route::{Channel, Envelope, Routed, CHANNEL, CHANNEL_VERSION, MAX_PAYLOAD};
pub use split::{Assembly, Complete, Fragment, Splitter};

/// Why the channel refused a message, named.
///
/// One set for the three pieces: two ways a registration collides, an id nobody
/// registered, a row this port has no type for, a message travelling the wrong way, a
/// peer on another version, a field the codec could not read, a position out of the
/// range its message documents, and the five ways a fragment is not a fragment.
/// Thirteen variants for thirteen different senders' bugs — a caller that cannot tell
/// "never registered" from "registered but not ported" retries the wrong thing, and one
/// that cannot tell "gap" from "duplicate" silently reassembles a corrupt body.
///
/// Copy, so a refusal travels out of a `Result` and into a log line without an
/// allocation; the value it is about (a body, a fragment) stays where the caller has it.
#[derive(Clone, Copy, Debug, PartialEq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// Two rows claim one id. Java keeps the last and forgets the first.
    DuplicateId { id: u16 },
    /// Two rows claim one Java class, or one port type.
    DuplicateName { name: &'static str },
    /// No row carries this id. Java's `getOrDefault(msg.getClass(), -1)`
    /// (`NetworkChannel.java:437`) and then throws.
    UnknownId { id: u16 },
    /// The id is registered in Java but this port has no type for it: the six classes
    /// outside `core/network/messages/`.
    NotPorted { id: u16 },
    /// The message arrived from the side its own `getExecutionSide()` names
    /// (`SplitPacketMessage.java:131-136`).
    WrongSide { id: u16, direction: mod_api::PayloadDirection, from: mod_api::LogicalSide },
    /// The peer speaks another version of the channel; Java accepts one exact string
    /// and nothing else (`NetworkChannel.java:100-101`).
    Version,
    /// A field the envelope, the fragment or a refusal needed was missing or of the
    /// wrong kind, as [`crate::network::common`] names it.
    Field(crate::network::common::Refusal),
    /// The sender is past the range the message carries (`IRangeRestrictedPacket.java:11`,
    /// `AbstractBuildingServerMessage.java:70-74`).
    OutOfRange { id: u16, range: i32, squared: i64 },
    /// A fragment index is negative, or past what the sender could have sent: Java's
    /// `-1` is "not set" (`SplitPacketMessage.java:26-41`), never a wire value.
    BadFragment { communication: i32, index: i32 },
    /// Two fragments of one message carry the same index. Java overwrites the first
    /// (`SplitPacketMessage.java:92`).
    DuplicateFragment { communication: i32, index: i32 },
    /// The terminator arrived and this index never did. Java concatenates the rest
    /// (`:103-107`).
    Gap { communication: i32, missing: i32 },
    /// One message split into more fragments than this port will hold, or more bodies
    /// open at once than it will track. Java bounds neither: only a one-minute cache
    /// expiry does (`NetworkChannel.java:82-85`). `count` is what the caller asked for,
    /// `limit` the bound that refused it.
    TooMany { count: usize, limit: usize },
    /// A reassembled body past the engine's payload bound
    /// (`crates/horizon-mod-vocab/src/payload.rs:148-157`).
    TooLarge { size: u64, limit: u64 },
}

impl Refusal {
    /// A stable name for logs and tests, namespaced like a registry entry so it can
    /// never be mistaken for a player-facing sentence.
    pub fn name(self) -> &'static str {
        match self {
            Self::DuplicateId { .. } => "channel.refusal.duplicate_id",
            Self::DuplicateName { .. } => "channel.refusal.duplicate_name",
            Self::UnknownId { .. } => "channel.refusal.unknown_id",
            Self::NotPorted { .. } => "channel.refusal.not_ported",
            Self::WrongSide { .. } => "channel.refusal.wrong_side",
            Self::Version => "channel.refusal.version",
            Self::Field(_) => "channel.refusal.field",
            Self::OutOfRange { .. } => "channel.refusal.out_of_range",
            Self::BadFragment { .. } => "channel.refusal.bad_fragment",
            Self::DuplicateFragment { .. } => "channel.refusal.duplicate_fragment",
            Self::Gap { .. } => "channel.refusal.gap",
            Self::TooMany { .. } => "channel.refusal.too_many",
            Self::TooLarge { .. } => "channel.refusal.too_large",
        }
    }
}

impl From<crate::network::common::Refusal> for Refusal {
    /// The codec's refusal, kept whole: it already names the field it wanted and
    /// whether it was absent or of the wrong kind.
    fn from(refusal: crate::network::common::Refusal) -> Self {
        Self::Field(refusal)
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())?;
        match self {
            Self::DuplicateId { id } => write!(f, ": {id}"),
            Self::DuplicateName { name } => write!(f, ": {name}"),
            Self::UnknownId { id } | Self::NotPorted { id } => write!(f, ": {id}"),
            Self::WrongSide { id, direction, from } => {
                write!(f, ": #{id} {direction:?} arrived from {from:?}")
            }
            Self::Version => Ok(()),
            Self::Field(refusal) => write!(f, ": {refusal}"),
            Self::OutOfRange { id, range, squared } => {
                write!(f, ": #{id} range {range}, squared distance {squared}")
            }
            Self::BadFragment { communication, index } => {
                write!(f, ": {communication}/{index}")
            }
            Self::DuplicateFragment { communication, index } => {
                write!(f, ": {communication}/{index}")
            }
            Self::Gap { communication, missing } => write!(f, ": {communication} missing {missing}"),
            Self::TooMany { count, limit } => write!(f, ": {count} asked, limit {limit}"),
            Self::TooLarge { size, limit } => write!(f, ": {size} bytes, limit {limit}"),
        }
    }
}

impl std::error::Error for Refusal {}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{LogicalSide, PayloadDirection};

    /// Every variant, so a new one cannot be added without a name of its own.
    fn all() -> Vec<Refusal> {
        vec![
            Refusal::DuplicateId { id: 1 },
            Refusal::DuplicateName { name: "a" },
            Refusal::UnknownId { id: 2 },
            Refusal::NotPorted { id: 3 },
            Refusal::WrongSide {
                id: 4,
                direction: PayloadDirection::ToServer,
                from: LogicalSide::Server,
            },
            Refusal::Version,
            Refusal::Field(crate::network::common::Refusal::Missing { key: "id" }),
            Refusal::OutOfRange { id: 5, range: 15, squared: 256 },
            Refusal::BadFragment { communication: 6, index: 7 },
            Refusal::DuplicateFragment { communication: 8, index: 9 },
            Refusal::Gap { communication: 10, missing: 11 },
            Refusal::TooMany { count: 12, limit: 13 },
            Refusal::TooLarge { size: 14, limit: 15 },
        ]
    }

    #[test]
    fn every_refusal_has_its_own_name_and_it_is_namespaced() {
        let all = all();
        assert_eq!(all.len(), 13, "thirteen variants, thirteen names");
        for (index, refusal) in all.iter().enumerate() {
            assert!(refusal.name().starts_with("channel.refusal."), "{} is not namespaced", refusal.name());
            for other in &all[index + 1..] {
                assert_ne!(refusal.name(), other.name(), "two refusals share a name");
            }
        }
    }

    #[test]
    fn a_refusal_says_what_it_refused_and_carries_no_translation() {
        assert_eq!(Refusal::Version.to_string(), "channel.refusal.version");
        assert_eq!(
            Refusal::Gap { communication: 3, missing: 1 }.to_string(),
            "channel.refusal.gap: 3 missing 1"
        );
        assert_eq!(
            Refusal::TooLarge { size: 2_000_000, limit: 1_048_576 }.to_string(),
            "channel.refusal.too_large: 2000000 bytes, limit 1048576"
        );
        assert_eq!(
            Refusal::OutOfRange { id: 20, range: 5, squared: 36 }.to_string(),
            "channel.refusal.out_of_range: #20 range 5, squared distance 36"
        );
        assert_eq!(
            Refusal::Field(crate::network::common::Refusal::Missing { key: "payload" }).to_string(),
            "channel.refusal.field: network.refusal.missing: payload",
            "the codec's own sentence, kept whole"
        );
    }
}
