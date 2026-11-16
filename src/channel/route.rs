//! The router: what an identifier plus a compound *is*, and what may be done with it.
//!
//! Java reference: `core/network/NetworkChannel.java` for the send surface
//! (`:315-426`), the class-to-id lookup that refuses an unknown class (`:434-441`),
//! the version predicates (`:100-101`); `core/network/messages/splitting/
//! SplitPacketMessage.java:131-136` for the side test; `api/network/IMessage.java:27-36`
//! for what a side even means.
//!
//! **One envelope, two levels.** Java writes the message into a `FriendlyByteBuf` and
//! hands the bytes to `SimpleChannel`, which prepends its own index: the id is the
//! channel's, the body is the message's, and neither knows the other's encoding
//! (`NetworkChannel.java:443-448` reads the serialized message back as opaque bytes).
//! [`Envelope`] is exactly that pair. The body is a [`Compound`] here — the shape
//! [`crate::network`] chose for every message — and the bytes are the layer below:
//! see `MANQUANT.md` §1 for the one function pair that is not exported to a mod.
//!
//! **`receive` answers the call to treat, or names why not.** Five checks, in the
//! order a receiver needs them: is the id registered ([`Refusal::UnknownId`]); does
//! this port have a type for it ([`Refusal::NotPorted`]); may it arrive *from* the
//! side it came from ([`Refusal::WrongSide`]); is its body a compound
//! ([`Refusal::Field`], raised by the codec when the caller decodes); is the sender
//! near enough for the range the row carries ([`Routed::in_range`], which needs
//! positions the router does not have). The third one is the check Java only runs on
//! the split path — this is deviation 1 of the module note.
//!
//! **What a routed message is not.** Not `executed`. Java's `onExecute` takes a
//! `NetworkEvent.Context`, a `ServerPlayer`, a colony and a level; the port owns none
//! of those here. [`Routed`] is the hand-off: the row (so a caller knows the side, the
//! range, the permission [`crate::network::Rule`] of the type it decodes) and the body.
//! The layer that owns a session calls [`Routed::decode`] with the type the row names
//! and does the rest.
//!
//! **The channel name and version are Java's, and they are a handshake.** Java makes
//! one channel named `net-channel` in the `minecolonies` namespace
//! (`core/Network.java:18` with `NetworkChannel.java:101`) and accepts a peer whose
//! version string is *equal* to its own, both ways (`:100-101`): not compatible, not
//! newer — equal. [`CHANNEL_VERSION`] is this crate's version for that reason, and
//! [`Channel::accepts`] is the one comparison.

use mod_api::{
    Compound, HandlerThread, LogicalSide, PayloadDeclaration, PayloadDirection, PayloadId,
    PayloadProtocol,
};

use super::registry::{Entry, MessageId, Registry};
use super::Refusal;
use crate::network::common::{self, Message};

/// The channel's registry name: `new ResourceLocation(Constants.MOD_ID, channelName)`
/// with `Constants.MOD_ID` = `minecolonies` and `channelName` = `net-channel`
/// (`core/Network.java:18`, `NetworkChannel.java:101`).
pub const CHANNEL: &str = "minecolonies:net-channel";

/// The exact string a peer must present. Java pins it to the mod's own version
/// (`NetworkChannel.java:100-101`), so this crate's version is the faithful choice.
pub const CHANNEL_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The engine's bound on one payload body, in bytes
/// (`crates/horizon-mod-vocab/src/payload.rs:148-157`: vanilla's
/// `ClientboundCustomPayloadPacket.MAX_PAYLOAD_SIZE`, 1,048,576). Java's own chunk size
/// is 90% of it (`NetworkChannel.java:451-452`), which is why the splitter's
/// [`super::split::MAX_CHUNK`] is the number it is.
pub const MAX_PAYLOAD: u64 = 1_048_576;

/// The envelope's two keys. `id` because Java's index is one (`registerMessage(int id,
/// …)`); `payload` because the body is what `toBytes` writes.
pub const KEY_ID: &str = "id";
pub const KEY_BODY: &str = "payload";

/// A message on the wire: which one, and what it carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Envelope {
    pub id: MessageId,
    pub body: Compound,
}

impl Envelope {
    pub fn new(id: MessageId, body: Compound) -> Envelope {
        Envelope { id, body }
    }

    /// A message of the [`crate::network`] layer, wrapped with the id its row gives it.
    ///
    /// The caller supplies the id rather than the type carrying it because the port's
    /// message structs are plain data with no id field — Java gets the id from a
    /// `Map<Class, Integer>` (`NetworkChannel.java:72-77`), which is the table
    /// [`super::registry::ALL`] now is. [`Registry::of_rust`] turns a type name back
    /// into a row when only the name is in hand.
    pub fn of<M: Message>(id: MessageId, message: &M) -> Envelope {
        Envelope { id, body: message.to_compound() }
    }

    /// The envelope as the port's own wire: two keys, the id as an `Int`, the body as a
    /// nested compound.
    pub fn to_compound(&self) -> Compound {
        Compound::from([
            (KEY_ID.into(), common::int(i32::from(self.id.raw()))),
            (KEY_BODY.into(), common::nested(&self.body)),
        ])
    }

    /// Reads an envelope, naming a missing or out-of-range id.
    ///
    /// An id past [`u16::MAX`] is [`crate::network::common::Refusal::Number`], not a
    /// truncated id: the table stops at 138 and a wrapped id would name a message
    /// nobody sent. A body that is absent is a refusal; a body that is an *empty
    /// compound* is legal, and the two are different — a message with no fields writes
    /// zero bytes in Java (`toBytes` writes nothing) and is still a message.
    pub fn from_compound(data: &Compound) -> Result<Envelope, Refusal> {
        let raw = common::int_in(data, KEY_ID, 0, i32::from(u16::MAX))?;
        // `int_in` bounded `raw` to 0..=65535, so this cast is exact.
        Ok(Envelope {
            id: MessageId::of(raw as u16),
            body: common::nested_of(data, KEY_BODY)?,
        })
    }
}

/// The channel: a registry, and the two questions a peer asks it.
///
/// Java's `NetworkChannel` also *sends* (`sendToServer`, `sendToPlayer`,
/// `sendToDimension`, six distributors, `:315-426`). The port cannot: a send needs a
/// connection, which is the engine's, and the mod-facing way to reach it is
/// `PlayerRef::send_payload` / `send_payload_to_server`
/// (`crates/horizon-mod-vocab/src/host/payloads.rs:14-35`). What this type still owns
/// is the decision *before* the send — may this message leave from this side — which is
/// the same predicate the receiver applies, and the declaration the engine needs at
/// load time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Channel {
    registry: Registry,
}

impl Channel {
    /// The channel of [`super::registry::ALL`].
    pub fn new() -> Channel {
        Channel { registry: Registry::new() }
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// The handshake: `peer` must be exactly [`CHANNEL_VERSION`]
    /// (`NetworkChannel.java:100-101`).
    pub fn accepts(peer: &str) -> Result<(), Refusal> {
        if peer == CHANNEL_VERSION {
            Ok(())
        } else {
            Err(Refusal::Version)
        }
    }

    /// Route one received message: the entry to treat it by, or a named refusal.
    ///
    /// `from` is the side that *sent* it — Java's
    /// `ctx.getDirection().getOriginationSide()` (`SplitPacketMessage.java:131`), not the
    /// side that received it. A message whose row runs on the server must come from the
    /// client.
    pub fn receive(&self, envelope: Envelope, from: LogicalSide) -> Result<Routed, Refusal> {
        let entry = self.registry.ported(envelope.id)?;
        if !entry.comes_from(from) {
            return Err(Refusal::WrongSide {
                id: entry.id.raw(),
                direction: entry.direction,
                from,
            });
        }
        Ok(Routed { entry, body: envelope.body, from })
    }

    /// The same predicate on the sending side: may `id` leave from `from`?
    ///
    /// Java asks nothing here — `handleSplitting` looks up the class and throws when the
    /// channel does not know it (`:437-441`), and sends in whichever direction the
    /// caller asked for. The port refuses a client-bound message leaving the client,
    /// which Java would deliver to the server and then execute with the wrong state.
    pub fn send(&self, id: MessageId, from: LogicalSide) -> Result<&'static Entry, Refusal> {
        let entry = self.registry.ported(id)?;
        if !entry.comes_from(from) {
            return Err(Refusal::WrongSide { id: entry.id.raw(), direction: entry.direction, from });
        }
        Ok(entry)
    }

    /// What the engine needs at load time to carry this channel.
    ///
    /// **One declaration, because Java has one channel.** The 139 ids are
    /// discriminators *inside* the body, so the channel itself is `Bidirectional` and
    /// each message's side is the table's column. The rest is read off
    /// `NetworkChannel.java:98-102`:
    ///
    /// - `id` — `ResourceLocation(Constants.MOD_ID, "net-channel")`, the channel's own
    ///   name (`core/Network.java:18`).
    /// - `version` — the mod's version, and it is compared for **equality** in both
    ///   directions (`:100-101`), which is why [`Channel::accepts`] is one string
    ///   comparison and nothing else.
    /// - `protocol` — `Play`: the channel is built when the level's messages flow, not
    ///   during configuration.
    /// - `direction` — `Bidirectional`, for the reason above.
    /// - `optional: false` — a peer that disagrees on the version is refused, so a peer
    ///   that has no channel at all cannot be admitted either.
    /// - `thread` — `Main`, which is where Java ends up: its handler runs on the Netty
    ///   thread and immediately calls `ctx.enqueueWork` to reach the main one
    ///   (`SplitPacketMessage.java:138-148`). The port asks for the main thread instead
    ///   of copying the hop.
    ///
    /// Written as the struct the SDK documents as "constructed explicitly by the
    /// caller" (`crates/horizon-mod-vocab/src/payload.rs:57-73`) rather than through
    /// `PayloadDeclaration::new`, so a reader sees all six choices in one place.
    pub fn declaration() -> PayloadDeclaration {
        PayloadDeclaration {
            id: PayloadId(CHANNEL.to_owned()),
            version: CHANNEL_VERSION.to_owned(),
            protocol: PayloadProtocol::Play,
            direction: PayloadDirection::Bidirectional,
            optional: false,
            thread: HandlerThread::Main,
        }
    }
}

impl Default for Channel {
    fn default() -> Self {
        Self::new()
    }
}

/// One message, accepted: the row to treat it by, the body, and where it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Routed {
    pub entry: &'static Entry,
    pub body: Compound,
    pub from: LogicalSide,
}

impl Routed {
    pub fn id(&self) -> MessageId {
        self.entry.id
    }

    /// The body as the type the row names, through that type's own codec — so a
    /// malformed body is the codec's named refusal and not a default value.
    ///
    /// The type is not checked against the row: `decode::<M>` on a `Routed` for another
    /// message gives that message's refusal (usually a missing key), never a silently
    /// wrong message. Tying the type to the row at the type level would need 132 trait
    /// implementations and would move a table fact into 132 places — `MANQUANT.md` §4
    /// says so and keeps the table the single source.
    pub fn decode<M: Message>(&self) -> Result<M, Refusal> {
        M::from_compound(&self.body).map_err(Refusal::from)
    }

    /// The range gate, for the rows that carry one.
    ///
    /// `at` is the sender's position, `about` the position the message is about — a
    /// building, a town hall, a citizen. Java:
    /// `from.distSqr(to) <= getMaxRange() * getMaxRange()`, inclusive
    /// (`IRangeRestrictedPacket.java:18-21`). A row with no range is never refused here,
    /// which is 94 of the 139.
    pub fn in_range(&self, at: [i32; 3], about: [i32; 3]) -> Result<(), Refusal> {
        let Some(range) = self.entry.range else {
            return Ok(());
        };
        let dx = i64::from(at[0]) - i64::from(about[0]);
        let dy = i64::from(at[1]) - i64::from(about[1]);
        let dz = i64::from(at[2]) - i64::from(about[2]);
        let squared = dx * dx + dy * dy + dz * dz;
        let limit = i64::from(range) * i64::from(range);
        if squared > limit {
            return Err(Refusal::OutOfRange { id: self.entry.id.raw(), range, squared });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::items::{Hand, ItemSetting, MarkStoryReadOnItem};

    /// `MarkStoryReadOnItemMessage`'s id, from the table.
    const STORY: MessageId = MessageId::of(91);
    /// `BuildRequestMessage`'s id: server-bound, five blocks.
    const BUILD_REQUEST: MessageId = MessageId::of(20);

    fn story() -> MarkStoryReadOnItem {
        MarkStoryReadOnItem { hand: Hand::Main }
    }

    #[test]
    fn an_envelope_round_trips_through_its_own_compound() {
        let envelope = Envelope::of(STORY, &story());
        let written = envelope.to_compound();
        assert_eq!(Envelope::from_compound(&written), Ok(envelope));
        assert_eq!(written.get(&KEY_ID.into()), Some(&common::int(91)));
        assert!(written.get(&KEY_BODY.into()).is_some());
    }

    #[test]
    fn an_envelope_with_no_id_is_a_missing_field_and_one_past_u16_is_a_number() {
        let data = story().to_compound();
        let envelope = Compound::from([(KEY_BODY.into(), common::nested(&data))]);
        assert_eq!(
            Envelope::from_compound(&envelope).err(),
            Some(Refusal::Field(common::Refusal::Missing { key: KEY_ID }))
        );

        let envelope = Compound::from([
            (KEY_ID.into(), common::int(70_000)),
            (KEY_BODY.into(), common::nested(&data)),
        ]);
        assert_eq!(
            Envelope::from_compound(&envelope).err(),
            Some(Refusal::Field(common::Refusal::Number {
                key: KEY_ID,
                min: 0,
                max: i64::from(u16::MAX),
            }))
        );
        let envelope = Compound::from([(KEY_ID.into(), common::int(-1)), (KEY_BODY.into(), common::nested(&data))]);
        assert!(Envelope::from_compound(&envelope).is_err(), "Java's -1 is not a wire id");
    }

    #[test]
    fn an_absent_body_and_an_empty_body_are_two_different_answers() {
        let absent = Compound::from([(KEY_ID.into(), common::int(91))]);
        assert_eq!(
            Envelope::from_compound(&absent).err(),
            Some(Refusal::Field(common::Refusal::Missing { key: KEY_BODY }))
        );
        let empty = Compound::from([
            (KEY_ID.into(), common::int(91)),
            (KEY_BODY.into(), common::nested(&Compound::new())),
        ]);
        let envelope = Envelope::from_compound(&empty).expect("a message with no field is legal");
        assert_eq!(envelope.id, STORY);
        assert!(envelope.body.is_empty());
    }

    #[test]
    fn a_message_from_the_right_side_is_routed_and_names_its_row() {
        let channel = Channel::new();
        let routed = channel
            .receive(Envelope::of(STORY, &story()), LogicalSide::Client)
            .expect("MarkStoryReadOnItem is server-bound, so it comes from the client");
        assert_eq!(routed.id(), STORY);
        assert_eq!(routed.entry.java, "MarkStoryReadOnItemMessage");
        assert_eq!(routed.entry.rust_type(), Some("items::MarkStoryReadOnItem"));
        assert_eq!(routed.from, LogicalSide::Client);
        assert_eq!(routed.decode::<MarkStoryReadOnItem>(), Ok(story()), "the body survived");
    }

    #[test]
    fn a_message_from_the_side_it_runs_on_is_refused_and_names_both_sides() {
        let channel = Channel::new();
        let refused = channel.receive(Envelope::of(STORY, &story()), LogicalSide::Server);
        assert_eq!(
            refused.err(),
            Some(Refusal::WrongSide {
                id: 91,
                direction: PayloadDirection::ToServer,
                from: LogicalSide::Server,
            })
        );

        // `ColonyViewMessage` is the mirror image: it runs on the client.
        let view = Envelope::new(MessageId::of(1), Compound::new());
        assert!(channel.receive(view.clone(), LogicalSide::Server).is_ok());
        assert_eq!(
            channel.receive(view, LogicalSide::Client).err(),
            Some(Refusal::WrongSide {
                id: 1,
                direction: PayloadDirection::ToClient,
                from: LogicalSide::Client,
            })
        );
    }

    #[test]
    fn an_unknown_id_and_a_row_without_a_type_are_two_refusals() {
        let channel = Channel::new();
        let absent = Envelope::new(MessageId::of(200), Compound::new());
        assert_eq!(
            channel.receive(absent, LogicalSide::Client).err(),
            Some(Refusal::UnknownId { id: 200 })
        );
        let untyped = Envelope::new(MessageId::of(128), Compound::new());
        assert_eq!(
            channel.receive(untyped, LogicalSide::Server).err(),
            Some(Refusal::NotPorted { id: 128 })
        );
        assert_eq!(
            channel.send(MessageId::of(128), LogicalSide::Server).err(),
            Some(Refusal::NotPorted { id: 128 })
        );
        assert_eq!(
            channel.send(MessageId::of(200), LogicalSide::Server).err(),
            Some(Refusal::UnknownId { id: 200 })
        );
    }

    #[test]
    fn a_routed_body_decoded_as_the_wrong_type_is_the_codec_refusal() {
        let channel = Channel::new();
        let setting = ItemSetting { setting_name: "clipboard".to_owned(), setting_value: 3 };
        let routed = channel
            .receive(Envelope::of(MessageId::of(138), &setting), LogicalSide::Client)
            .expect("ItemSettingMessage declares no side, so either one arrives");
        assert_eq!(
            routed.decode::<ItemSetting>(),
            Ok(setting),
            "the message the envelope named"
        );
        assert_eq!(
            routed.decode::<MarkStoryReadOnItem>().err(),
            Some(Refusal::Field(common::Refusal::Missing { key: "hand" })),
            "another type finds no field of its own and says which"
        );
    }

    #[test]
    fn sending_asks_the_same_question_as_receiving() {
        let channel = Channel::new();
        assert!(channel.send(BUILD_REQUEST, LogicalSide::Client).is_ok(), "a request leaves the client");
        assert_eq!(
            channel.send(BUILD_REQUEST, LogicalSide::Server).err(),
            Some(Refusal::WrongSide {
                id: 20,
                direction: PayloadDirection::ToServer,
                from: LogicalSide::Server,
            })
        );
        assert!(channel.send(MessageId::of(1), LogicalSide::Server).is_ok(), "a view leaves the server");
        assert!(channel.send(MessageId::SPLIT, LogicalSide::Client).is_ok(), "id 0 is either way");
    }

    #[test]
    fn the_handshake_is_one_exact_string() {
        assert_eq!(Channel::accepts(CHANNEL_VERSION), Ok(()));
        assert_eq!(Channel::accepts("0.1.1"), Err(Refusal::Version));
        assert_eq!(Channel::accepts(""), Err(Refusal::Version));
        assert_eq!(Channel::accepts(" 0.1.0"), Err(Refusal::Version), "no trimming");
        assert!(!CHANNEL_VERSION.is_empty(), "the engine refuses a blank version");
    }

    #[test]
    fn the_declaration_is_one_play_channel_required_and_on_the_main_thread() {
        let declaration = Channel::declaration();
        assert_eq!(declaration.id, PayloadId("minecolonies:net-channel".to_owned()));
        assert_eq!(declaration.version, CHANNEL_VERSION);
        assert_eq!(declaration.protocol, PayloadProtocol::Play);
        assert_eq!(declaration.direction, PayloadDirection::Bidirectional);
        assert!(!declaration.optional, "Java refuses a peer with another version");
        assert_eq!(declaration.thread, HandlerThread::Main);
    }

    #[test]
    fn the_range_gate_is_inclusive_and_only_applies_where_the_row_has_one() {
        let channel = Channel::new();
        let request = channel
            .receive(Envelope::new(BUILD_REQUEST, Compound::new()), LogicalSide::Client)
            .expect("BuildRequestMessage is server-bound");
        assert_eq!(request.entry.range, Some(5));
        assert_eq!(request.in_range([0, 64, 0], [0, 64, 0]), Ok(()));
        assert_eq!(request.in_range([5, 64, 0], [0, 64, 0]), Ok(()), "5 blocks away is the limit");
        assert_eq!(
            request.in_range([6, 64, 0], [0, 64, 0]).err(),
            Some(Refusal::OutOfRange { id: 20, range: 5, squared: 36 })
        );

        let story = channel
            .receive(Envelope::of(STORY, &story()), LogicalSide::Client)
            .expect("MarkStoryReadOnItem is server-bound");
        assert_eq!(story.entry.range, None);
        assert_eq!(story.in_range([0, 0, 0], [100_000, -100_000, 0]), Ok(()), "no range, no gate");
    }

    #[test]
    fn a_refusal_prints_its_name_and_what_it_refused() {
        let refusal = Refusal::WrongSide {
            id: 91,
            direction: PayloadDirection::ToServer,
            from: LogicalSide::Server,
        };
        assert_eq!(
            refusal.to_string(),
            "channel.refusal.wrong_side: #91 ToServer arrived from Server"
        );
        assert_eq!(Refusal::UnknownId { id: 3 }.to_string(), "channel.refusal.unknown_id: 3");
    }
}
