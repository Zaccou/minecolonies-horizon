//! The eleven field codecs, the refusal, and the two envelopes every message of
//! this module shares.
//!
//! Java reference: `api/network/IMessage.java` (the contract: `toBytes`,
//! `fromBytes`, `getExecutionSide`, `onExecute`), `core/network/messages/server/
//! AbstractColonyServerMessage.java` (dimension + colony id, the permission gate)
//! and `server/AbstractBuildingServerMessage.java` (adds the building position and
//! the five-block range), `core/network/IRangeRestrictedPacket.java` (fifteen
//! blocks by default).
//!
//! **`Compound` is the wire here, not the save file.** A message in this port is a
//! value with `to_compound`/`from_compound` and nothing else; there is no socket,
//! no `FriendlyByteBuf` and no `NetworkEvent.Context`. That is deliberate: the
//! same shape as [`crate::requests::Request`], so a test can round-trip a message
//! on the host target with no game. What Java's `onExecute` *does* (open a window,
//! hire a citizen, spawn mercenaries) needs a `Host` and belongs to the layer that
//! will own the session, not here.
//!
//! **Eleven codecs, one per NBT kind Java's buffer writes.** Each is a pair — a
//! writer and a reader that names the field it wanted — and every reader returns
//! [`Refusal`], never a default:
//!
//! | Java write | NBT | Here |
//! | --- | --- | --- |
//! | `writeUtf` (≤ 32767 chars) | `String` | [`text`] / [`text_of`] |
//! | `writeInt`, `writeVarInt` | `Int` | [`int`] / [`int_of`] |
//! | `writeLong` | `Long` | [`long`] / [`long_of`] |
//! | `writeBoolean` | `Byte` 0/1 | [`boolean`] / [`bool_of`] |
//! | `writeDouble` | `Double` (bits) | [`double`] / [`double_of`] |
//! | `writeFloat` | `Float` (bits) | [`float`] / [`float_of`] |
//! | `writeBlockPos` | `{x, y, z}` | [`position`] / [`pos_of`] |
//! | `writeUUID` | `String` | [`uuid`] / [`uuid_of`] |
//! | `writeResourceLocation`, `writeUtf("<dimension>")` | `String` | [`text`] / [`text_of`] |
//! | `writeByteArray`, an opaque `FriendlyByteBuf` blob | `ByteArray` | [`bytes`] / [`bytes_of`] |
//! | `writeNbt` | `Compound` | [`nested`] / [`nested_of`] |
//!
//! **Three things the table hides, each with its reason.** A position is three
//! ints, not `writeBlockPos`'s packed long: `requests::nbt::position` already made
//! that choice against `BlockPosUtil` (`api/util/BlockPosUtil.java:190-197`), and a
//! second, different position codec in the same crate would be the one that
//! drifts. A double is stored as its **bit pattern** (`Tag::Double(u64)`), so the
//! round trip is exact including `-0.0`. A UUID is the string `UUID.toString()`
//! gives, as `colony::permissions::uuid` already reads it, because `u128` has no
//! NBT width and a pair of longs would not compare equal to the same id read from
//! a colony record.
//!
//! **Names, not ordinals, for every enum this module declares.** Java writes
//! `buf.writeInt(mode.ordinal())` in a dozen messages, and its own
//! `RequestState` warns what that costs: "This should never be changed! It is used
//! to read and write from NBT so it has to persist between mod versions"
//! (`api/colony/requestsystem/request/RequestState.java:84-90`). A port that
//! reorders a variant would silently renumber the wire; a name cannot. The
//! precedent is `src/requests/request.rs`, *The saved name — a string,
//! deliberately*. The two enums that are **vanilla's**, not MineColonies'
//! (`ChatFormatting`, `SoundSource`), stay raw integers: the port has no type for
//! them and inventing twenty variants for one field would be noise, not a rule.
//!
//! **What is deliberately absent.** No `Result` from `to_compound`: the value in
//! hand is already valid, and every bound (`MAX_TEXT`, `MAX_NAME`) is enforced by
//! the constructor that could break it. No `Default`: a message with a missing
//! field is [`Refusal::Missing`], never a zero.

use crate::base::Dimension;
use crate::colony::permissions::uuid as permissions_uuid;
use crate::colony::{Action, Rank};
use crate::inventory::ItemName;
use mod_api::{Compound, Tag};

/// The longest string Java's `FriendlyByteBuf.writeUtf(String)` accepts, in
/// UTF-16 code units (`net.minecraft.network.FriendlyByteBuf#writeUtf`).
pub const MAX_TEXT: usize = 32767;

/// The range a message crosses without being dropped
/// (`IRangeRestrictedPacket.java:11`), and the shorter one a message about a
/// building uses (`AbstractBuildingServerMessage.java:70-73`).
pub const RANGE: i32 = 15;
pub const BUILDING_RANGE: i32 = 5;

/// The keys the shared envelopes write. `dimensionId` and `colonyId` are the two
/// fields of `AbstractColonyServerMessage` (`:26-31`), written in that order
/// (`:83-89`); `buildingId` is the third one a building message adds
/// (`AbstractBuildingServerMessage.java:19`, written `:59-67`).
pub const KEY_DIMENSION: &str = "dimensionId";
pub const KEY_COLONY: &str = "colonyId";
pub const KEY_BUILDING: &str = "buildingId";

/// Why a message could not be read, named.
///
/// Five variants, and each is a different bug in the sender: a field that never
/// arrived, one that arrived as the wrong NBT kind, a name this port does not
/// know, a string past [`MAX_TEXT`], a number outside the range its field
/// documents. The key is `&'static str` so the refusal stays `Copy` and a log
/// line can name the field without allocating; the offending *value* stays in the
/// compound the caller still holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// A required field is not in the compound. Java would read a zero here.
    Missing { key: &'static str },
    /// The field is there, as the wrong NBT kind — a string where an int belongs.
    Kind { key: &'static str },
    /// The field names a variant or a registry entry this port cannot resolve.
    Name { key: &'static str },
    /// A text past [`MAX_TEXT`] code units, or a list past its documented bound.
    Size { key: &'static str, max: usize },
    /// An integer outside the inclusive range the field allows.
    Number { key: &'static str, min: i64, max: i64 },
}

impl Refusal {
    /// A stable name for logs, namespaced like a registry entry so it can never be
    /// mistaken for a player-facing sentence.
    pub fn name(self) -> &'static str {
        match self {
            Self::Missing { .. } => "network.refusal.missing",
            Self::Kind { .. } => "network.refusal.kind",
            Self::Name { .. } => "network.refusal.name",
            Self::Size { .. } => "network.refusal.size",
            Self::Number { .. } => "network.refusal.number",
        }
    }

    /// The field the refusal is about.
    pub fn key(self) -> &'static str {
        match self {
            Self::Missing { key }
            | Self::Kind { key }
            | Self::Name { key }
            | Self::Size { key, .. }
            | Self::Number { key, .. } => key,
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.name(), self.key())
    }
}

impl std::error::Error for Refusal {}

/// What a message is, in the two methods Java's `IMessage` has and this port keeps
/// (`api/network/IMessage.java`): a value that writes itself into a compound and
/// reads itself back.
///
/// [`Message::RULE`] is the other half of the contract. Java's
/// `AbstractColonyServerMessage` asks `permissionNeeded()` (default `MANAGE_HUTS`,
/// `:60-64`) and `ownerOnly()` (default `false`, `:66-69`) before it lets a
/// subclass run (`:122-141`); the port keeps those two answers as data on the type,
/// where a test can read them, instead of as a method a window may override.
pub trait Message: Sized {
    /// Which permission gates this message. [`Rule::None`] for everything that
    /// runs on the side that has no colony to check against.
    const RULE: Rule = Rule::None;

    fn to_compound(&self) -> Compound;
    fn from_compound(data: &Compound) -> Result<Self, Refusal>;
}

/// The permission gate of a server-bound message: `permissionNeeded()` and
/// `ownerOnly()`, together.
///
/// **Java's `ownerOnly` branch is inverted and the port does not copy it.** The
/// test reads `ownerOnly() && (player == null || owner.equals(player.getUUID()))`
/// and then **denies** — so the owner is refused and every non-owner passes
/// (`AbstractColonyServerMessage.java:132-141`). No message in
/// `core/network/messages` overrides `ownerOnly()`, so the branch never fires in
/// Java; [`Rule::allows`] implements the intent ("the owner only") and
/// `RAPPORT.md` §"ce que je n'ai pas copié" records the deviation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: a closed set of five gates, one per Java override plus the owner
pub enum Rule {
    /// No colony to check: a client-bound message, or one whose permission is
    /// explicitly `null` (`TeleportToColonyMessage.java:63-68`).
    None,
    /// `Action.MANAGE_HUTS`, the default of the abstract base (`:60-64`).
    ManageHuts,
    /// `Action.EDIT_PERMISSIONS` (`ChangeFreeToInteractBlockMessage.java:103-108`).
    EditPermissions,
    /// `Action.PLACE_BLOCKS` (`PlayerAssistantBuildRequestMessage.java:51-55`).
    PlaceBlocks,
    /// The colony's owner, and nobody else.
    OwnerOnly,
}

impl Rule {
    /// The action this gate asks for, or `None` for the two that are not an
    /// `Action`.
    pub fn action(self) -> Option<Action> {
        match self {
            Rule::None | Rule::OwnerOnly => None,
            Rule::ManageHuts => Some(Action::ManageHuts),
            Rule::EditPermissions => Some(Action::EditPermissions),
            Rule::PlaceBlocks => Some(Action::PlaceBlocks),
        }
    }

    /// Whether `player` may run this message. `Err(Refusal::Name)` is not used
    /// here: the answer is the colony layer's own refusal, returned as a bool and
    /// the reason asked separately, because this module owns no colony.
    pub fn allows(self, members: &crate::colony::Members, owner: u128, player: u128) -> bool {
        match self {
            Rule::None => true,
            Rule::OwnerOnly => player == owner,
            Rule::ManageHuts | Rule::EditPermissions | Rule::PlaceBlocks => {
                members.may(player, self.action().unwrap_or(Action::ManageHuts)).is_ok()
            }
        }
    }
}

/// The two fields `AbstractColonyServerMessage` carries, written first in the
/// order Java writes them (`:85-86`).
pub fn envelope(data: &mut Compound, dimension: &Dimension, colony: i32) {
    data.insert(KEY_DIMENSION.into(), text(dimension.as_str()));
    data.insert(KEY_COLONY.into(), Tag::Int(colony));
}

/// The same fields, added by `AbstractBuildingServerMessage` (`:61`).
pub fn building_envelope(data: &mut Compound, dimension: &Dimension, colony: i32, building: [i32; 3]) {
    envelope(data, dimension, colony);
    data.insert(KEY_BUILDING.into(), position(building));
}

/// Reads the colony envelope. A missing dimension is [`Refusal::Missing`], not the
/// overworld: a message for the wrong level is worse than a dropped one.
pub fn envelope_of(data: &Compound) -> Result<(Dimension, i32), Refusal> {
    Ok((dimension_of(data, KEY_DIMENSION)?, int_of(data, KEY_COLONY)?))
}

/// Reads the building envelope: the colony pair, then the hut's position.
pub fn building_envelope_of(data: &Compound) -> Result<(Dimension, i32, [i32; 3]), Refusal> {
    let (dimension, colony) = envelope_of(data)?;
    Ok((dimension, colony, pos_of(data, KEY_BUILDING)?))
}

// ---------------------------------------------------------------------------
// Writers. One per codec; a value that is invalid never reaches them.
// ---------------------------------------------------------------------------

/// A string, as `writeUtf` would carry it.
pub fn text(value: &str) -> Tag {
    Tag::String(value.into())
}

/// A count, an id, a flag: `writeInt`.
pub fn int(value: i32) -> Tag {
    Tag::Int(value)
}

/// `writeLong`: a packed block position, a millisecond stamp.
pub fn long(value: i64) -> Tag {
    Tag::Long(value)
}

/// `writeBoolean`: one NBT byte, `0` or `1`.
pub fn boolean(value: bool) -> Tag {
    Tag::Byte(if value { 1 } else { 0 })
}

/// `writeDouble`, kept as its IEEE bit pattern so `-0.0` survives.
pub fn double(value: f64) -> Tag {
    Tag::Double(value.to_bits())
}

/// `writeFloat`, same reason as [`double`].
pub fn float(value: f32) -> Tag {
    Tag::Float(value.to_bits())
}

/// A block position as `{x, y, z}`, the shape `requests::nbt::position` uses.
pub fn position(at: [i32; 3]) -> Tag {
    Tag::compound([("x", Tag::Int(at[0])), ("y", Tag::Int(at[1])), ("z", Tag::Int(at[2]))])
}

/// A `UUID` as the string `UUID.toString()` gives (`colony::permissions::uuid`).
pub fn uuid(id: u128) -> Tag {
    text(&uuid_text(id))
}

/// An opaque payload: Java's `writeByteArray`, or a whole `FriendlyByteBuf` an
/// inner serializer wrote (`buf.writeBytes(inner)`).
pub fn bytes(value: &[u8]) -> Tag {
    Tag::ByteArray(value.to_vec())
}

/// An NBT value passed through whole: `buf.writeNbt(compound)`.
pub fn nested(value: &Compound) -> Tag {
    Tag::Compound(value.clone())
}

/// A list of records: NBT lists are homogeneous, so a list of compounds.
pub fn records(values: &[Compound]) -> Tag {
    Tag::List(values.iter().cloned().map(Tag::Compound).collect())
}

/// A list of strings: `writeInt(size)` then one `writeUtf` each.
pub fn texts(values: &[String]) -> Tag {
    Tag::List(values.iter().map(|value| text(value)).collect())
}

// ---------------------------------------------------------------------------
// Readers. Every one names the field it wanted.
// ---------------------------------------------------------------------------

/// The tag under `key`, or [`Refusal::Missing`].
pub fn field<'a>(data: &'a Compound, key: &'static str) -> Result<&'a Tag, Refusal> {
    data.get(&key.into()).ok_or(Refusal::Missing { key })
}

/// A string under `key`, refusing one past [`MAX_TEXT`].
pub fn text_of(data: &Compound, key: &'static str) -> Result<String, Refusal> {
    match field(data, key)? {
        Tag::String(value) => {
            let value = value.to_string_lossy();
            // `writeUtf`'s limit counts UTF-16 code units, not bytes: a French
            // translation is half the bytes its `é` would suggest.
            if value.encode_utf16().count() > MAX_TEXT {
                return Err(Refusal::Size { key, max: MAX_TEXT });
            }
            Ok(value)
        }
        _ => Err(Refusal::Kind { key }),
    }
}

/// A string a nested tag holds, for a field inside a record of its own.
pub fn text_in(tag: &Tag, key: &'static str) -> Result<String, Refusal> {
    match tag.get(key) {
        Some(Tag::String(value)) => Ok(value.to_string_lossy()),
        Some(_) => Err(Refusal::Kind { key }),
        None => Err(Refusal::Missing { key }),
    }
}

/// An integer, in whichever width it was written. Java's `writeInt` is the width
/// this port writes; a `Byte` or a `Short` is accepted on read because a saved
/// compound from an older writer may hold one, as `requests::nbt` already allows.
pub fn int_of(data: &Compound, key: &'static str) -> Result<i32, Refusal> {
    match field(data, key)? {
        Tag::Int(value) => Ok(*value),
        Tag::Byte(value) => Ok(i32::from(*value)),
        Tag::Short(value) => Ok(i32::from(*value)),
        _ => Err(Refusal::Kind { key }),
    }
}

/// An integer that must sit inside `min..=max`. Out of range is
/// [`Refusal::Number`], never a clamp: a silently clamped count delivers the wrong
/// number of items.
pub fn int_in(data: &Compound, key: &'static str, min: i32, max: i32) -> Result<i32, Refusal> {
    let value = int_of(data, key)?;
    if value < min || value > max {
        return Err(Refusal::Number { key, min: i64::from(min), max: i64::from(max) });
    }
    Ok(value)
}

/// A `writeLong` value.
pub fn long_of(data: &Compound, key: &'static str) -> Result<i64, Refusal> {
    match field(data, key)? {
        Tag::Long(value) => Ok(*value),
        Tag::Int(value) => Ok(i64::from(*value)),
        _ => Err(Refusal::Kind { key }),
    }
}

/// A boolean, stored as one byte. Any nonzero byte is `true`, as NBT's own
/// `getBoolean` reads it.
pub fn bool_of(data: &Compound, key: &'static str) -> Result<bool, Refusal> {
    match field(data, key)? {
        Tag::Byte(value) => Ok(*value != 0),
        _ => Err(Refusal::Kind { key }),
    }
}

/// A double, from its bit pattern. An `Int` is refused: a 1 that means `1.0` and
/// a 1 that means one are not the same field.
pub fn double_of(data: &Compound, key: &'static str) -> Result<f64, Refusal> {
    match field(data, key)? {
        Tag::Double(bits) => Ok(f64::from_bits(*bits)),
        _ => Err(Refusal::Kind { key }),
    }
}

/// A float, from its bit pattern.
pub fn float_of(data: &Compound, key: &'static str) -> Result<f32, Refusal> {
    match field(data, key)? {
        Tag::Float(bits) => Ok(f32::from_bits(*bits)),
        _ => Err(Refusal::Kind { key }),
    }
}

/// A block position under `key`. A missing coordinate refuses the whole position:
/// an absence is not the origin, the rule `requests::nbt::position_of` states.
pub fn pos_of(data: &Compound, key: &'static str) -> Result<[i32; 3], Refusal> {
    let tag = field(data, key)?;
    if !matches!(tag, Tag::Compound(_)) {
        // An int where a position belongs is the wrong kind, not a position whose
        // `x` went missing.
        return Err(Refusal::Kind { key });
    }
    Ok([coordinate(tag, "x")?, coordinate(tag, "y")?, coordinate(tag, "z")?])
}

/// A position that may be absent, `None` when the key is not there. Used by the
/// three messages where Java writes a presence flag first
/// (`GuardSetMinePosMessage.java:56-64`, `ResourceScrollSaveWarehouseSnapshotMessage.java:69-76`).
pub fn opt_pos_of(data: &Compound, key: &'static str) -> Result<Option<[i32; 3]>, Refusal> {
    match data.get(&key.into()) {
        None => Ok(None),
        Some(_) => pos_of(data, key).map(Some),
    }
}

fn coordinate(tag: &Tag, key: &'static str) -> Result<i32, Refusal> {
    match tag.get(key) {
        Some(Tag::Int(value)) => Ok(*value),
        Some(_) => Err(Refusal::Kind { key }),
        None => Err(Refusal::Missing { key }),
    }
}

/// A UUID, as the string `UUID.toString()` gives. A string this port cannot parse
/// is [`Refusal::Name`], not a zeroed id.
pub fn uuid_of(data: &Compound, key: &'static str) -> Result<u128, Refusal> {
    let text = text_of(data, key)?;
    permissions_uuid::parse(&text).ok_or(Refusal::Name { key })
}

/// The textual form of an id, so a writer and a test agree on one spelling.
pub fn uuid_text(id: u128) -> String {
    permissions_uuid::to_text(id)
}

/// A byte array: an opaque payload, a `byte[]` field.
pub fn bytes_of(data: &Compound, key: &'static str) -> Result<Vec<u8>, Refusal> {
    match field(data, key)? {
        Tag::ByteArray(value) => Ok(value.clone()),
        _ => Err(Refusal::Kind { key }),
    }
}

/// A nested compound, passed through whole.
pub fn nested_of(data: &Compound, key: &'static str) -> Result<Compound, Refusal> {
    match field(data, key)? {
        Tag::Compound(value) => Ok(value.clone()),
        _ => Err(Refusal::Kind { key }),
    }
}

/// The list under `key`, as tags.
pub fn list_of<'a>(data: &'a Compound, key: &'static str) -> Result<&'a [Tag], Refusal> {
    match field(data, key)? {
        Tag::List(values) => Ok(values.as_slice()),
        _ => Err(Refusal::Kind { key }),
    }
}

/// Every compound of the list under `key`, in order. A non-compound element
/// refuses the list: half a record is not a record.
pub fn records_of<'a>(data: &'a Compound, key: &'static str) -> Result<Vec<&'a Compound>, Refusal> {
    let mut records = Vec::new();
    for value in list_of(data, key)? {
        match value {
            Tag::Compound(record) => records.push(record),
            _ => return Err(Refusal::Kind { key }),
        }
    }
    Ok(records)
}

/// The strings of the list under `key`, in order.
pub fn texts_of(data: &Compound, key: &'static str) -> Result<Vec<String>, Refusal> {
    let mut texts = Vec::new();
    for value in list_of(data, key)? {
        match value {
            Tag::String(text) => texts.push(text.to_string_lossy()),
            _ => return Err(Refusal::Kind { key }),
        }
    }
    Ok(texts)
}

/// A dimension's registry name. Java writes `dimension.location().toString()`
/// (`AbstractColonyServerMessage.java:85`) and reads it back with
/// `ResourceLocation`, so the port compares the full name — two spellings of one
/// dimension are two dimensions (`base::Dimension`).
pub fn dimension_of(data: &Compound, key: &'static str) -> Result<Dimension, Refusal> {
    let name = text_of(data, key)?;
    if !name.contains(':') {
        // `ResourceLocation` refuses an unqualified name; `minecraft:overworld` is
        // the shortest a dimension can be.
        return Err(Refusal::Name { key });
    }
    Ok(Dimension::new(&name))
}

// ---------------------------------------------------------------------------
// The four values more than one message carries, and no module owns.
// ---------------------------------------------------------------------------

/// One chat component, cut to what these messages need.
///
/// Java's `Component` is a tree — content, style, children, metadata
/// (`net.minecraft.network.chat.Component`), and `mod_api::Text` models all of it
/// (`crates/horizon-mod-vocab/src/text/mod.rs:9-31`). The four messages that carry
/// one here use the same shape every time: a translation key, with zero or one
/// number already rendered into it
/// (`OpenCantFoundColonyWarningMessage.java:98`, `InteractionResponse.java:100`).
/// [`Component`] keeps exactly that, and says so instead of pretending to carry a
/// style nobody reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Component {
    /// The translation key, or the literal text when `translatable` is false.
    pub text: String,
    /// Whether `text` is a key (`Component.translatable`) or literal
    /// (`Component.literal`).
    pub translatable: bool,
    /// The `%s` arguments, already formatted. A count, not a component tree.
    pub arguments: Vec<String>,
}

impl Component {
    pub fn translatable(key: &str) -> Component {
        Component { text: key.to_owned(), translatable: true, arguments: Vec::new() }
    }

    pub fn literal(text: &str) -> Component {
        Component { text: text.to_owned(), translatable: false, arguments: Vec::new() }
    }

    /// The key with one argument substituted, as
    /// `Component.translatable(key, value)` does.
    pub fn with_argument(mut self, value: &str) -> Component {
        self.arguments.push(value.to_owned());
        self
    }

    fn to_tag(&self) -> Tag {
        Tag::compound([
            ("text", text(&self.text)),
            ("translatable", boolean(self.translatable)),
            ("arguments", texts(&self.arguments)),
        ])
    }

    fn from_tag(tag: &Tag) -> Result<Component, Refusal> {
        let value = text_in(tag, "text")?;
        let translatable = match tag.get("translatable") {
            Some(Tag::Byte(flag)) => *flag != 0,
            Some(_) => return Err(Refusal::Kind { key: "translatable" }),
            None => return Err(Refusal::Missing { key: "translatable" }),
        };
        let mut arguments = Vec::new();
        match tag.get("arguments") {
            Some(Tag::List(values)) => {
                for value in values {
                    match value {
                        Tag::String(text) => arguments.push(text.to_string_lossy()),
                        _ => return Err(Refusal::Kind { key: "arguments" }),
                    }
                }
            }
            Some(_) => return Err(Refusal::Kind { key: "arguments" }),
            None => return Err(Refusal::Missing { key: "arguments" }),
        }
        Ok(Component { text: value, translatable, arguments })
    }
}

/// A component as a field: `Tag::Compound` of its three parts.
pub fn component(value: &Component) -> Tag {
    value.to_tag()
}

/// Reads a component written by [`component`].
pub fn component_of(data: &Compound, key: &'static str) -> Result<Component, Refusal> {
    let tag = field(data, key)?;
    if !matches!(tag, Tag::Compound(_)) {
        return Err(Refusal::Kind { key });
    }
    Component::from_tag(tag)
}

/// One item stack as the wire carries it: the registry name, a count, and the
/// stack's own data passed through.
/// **Why this is not [`crate::inventory::Stack`].** That type is an item and a
/// *positive* count, because it is what a slot holds ([`crate::inventory::Stack::new`]).
/// A message carries `ItemStack.EMPTY` too — `TransferRecipeCraftingTeachingMessage`
/// sends nine slots and most of them are empty
/// (`TransferRecipeCraftingTeachingMessage.java:93-103`) — so the count here may be
/// zero, and `data` is the stack's NBT components, carried opaque because a port
/// of `DataComponentMap` is not this module's business.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemStack {
    pub item: ItemName,
    /// `ItemStack.getCount()`. Zero is an empty stack, which Java writes.
    pub count: i32,
    /// The stack's components, passed through whole. `None` is a stack that
    /// carried none, which is not the same as an empty compound.
    pub data: Option<Compound>,
}

impl ItemStack {
    /// A stack with no components. `Err(Refusal::Name)` for a name the engine
    /// cannot parse, `Err(Refusal::Number)` for a negative count.
    pub fn new(item: &str, count: i32) -> Result<ItemStack, Refusal> {
        let key = "item";
        let item = ItemName::new(item).ok_or(Refusal::Name { key })?;
        if count < 0 {
            return Err(Refusal::Number { key: "count", min: 0, max: i64::from(i32::MAX) });
        }
        Ok(ItemStack { item, count, data: None })
    }

    /// Whether Java's `ItemStack.isEmpty()` would agree.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    fn to_tag(&self) -> Tag {
        let mut tag = Compound::from([("item".into(), text(self.item.as_str())), ("count".into(), int(self.count))]);
        if let Some(data) = &self.data {
            tag.insert("data".into(), nested(data));
        }
        Tag::Compound(tag)
    }

    fn from_tag(tag: &Tag) -> Result<ItemStack, Refusal> {
        let name = text_in(tag, "item")?;
        let item = ItemName::new(&name).ok_or(Refusal::Name { key: "item" })?;
        let count = match tag.get("count") {
            Some(Tag::Int(value)) => *value,
            Some(_) => return Err(Refusal::Kind { key: "count" }),
            None => return Err(Refusal::Missing { key: "count" }),
        };
        if count < 0 {
            return Err(Refusal::Number { key: "count", min: 0, max: i64::from(i32::MAX) });
        }
        let data = match tag.get("data") {
            Some(Tag::Compound(value)) => Some(value.clone()),
            Some(_) => return Err(Refusal::Kind { key: "data" }),
            None => None,
        };
        Ok(ItemStack { item, count, data })
    }
}

/// A stack as a field: `Tag::Compound` of item, count and components.
pub fn stack(value: &ItemStack) -> Tag {
    value.to_tag()
}

/// Reads a stack written by [`stack`].
pub fn stack_of(data: &Compound, key: &'static str) -> Result<ItemStack, Refusal> {
    let tag = field(data, key)?;
    if !matches!(tag, Tag::Compound(_)) {
        return Err(Refusal::Kind { key });
    }
    ItemStack::from_tag(tag)
}

/// Reads a stack from a tag that is already in hand, for a stack nested inside a
/// record of a list (a recipe slot, a snapshot row).
pub fn stack_in(tag: &Tag) -> Result<ItemStack, Refusal> {
    if !matches!(tag, Tag::Compound(_)) {
        return Err(Refusal::Kind { key: "stack" });
    }
    ItemStack::from_tag(tag)
}

/// One chunk's claim record, as `ChunkCapData` writes it.
///
/// Java's record also holds `allClaimBuildings`, and **its own `toBytes` does not
/// write them** — "todo in the future, when we need it, we can also sync the
/// claimed buildings. We don't do this atm as its a bunch of data"
/// (`api/util/ChunkCapData.java:56-72`). [`ChunkCap`] is the four fields that cross,
/// which is what a reader of that message can ever see anyway.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkCap {
    pub x: i32,
    pub z: i32,
    /// `0` for a chunk no colony owns (`IColonyTagCapability`'s empty value).
    pub owning_colony: i32,
    /// The colonies whose territory reaches this chunk, in the order written.
    pub claiming_colonies: Vec<i32>,
}

impl ChunkCap {
    /// The record as a compound, for a list of them.
    fn to_record(&self) -> Compound {
        Compound::from([
            ("x".into(), int(self.x)),
            ("z".into(), int(self.z)),
            ("owningColony".into(), int(self.owning_colony)),
            (
                "staticClaim".into(),
                Tag::List(self.claiming_colonies.iter().map(|id| int(*id)).collect()),
            ),
        ])
    }

    fn from_record(tag: &Compound) -> Result<ChunkCap, Refusal> {
        let mut claiming_colonies = Vec::new();
        match tag.get(&"staticClaim".into()) {
            Some(Tag::List(values)) => {
                for value in values {
                    match value {
                        Tag::Int(id) => claiming_colonies.push(*id),
                        _ => return Err(Refusal::Kind { key: "staticClaim" }),
                    }
                }
            }
            Some(_) => return Err(Refusal::Kind { key: "staticClaim" }),
            None => return Err(Refusal::Missing { key: "staticClaim" }),
        }
        Ok(ChunkCap {
            x: nested_int_record(tag, "x")?,
            z: nested_int_record(tag, "z")?,
            owning_colony: nested_int_record(tag, "owningColony")?,
            claiming_colonies,
        })
    }
}

fn nested_int_record(data: &Compound, key: &'static str) -> Result<i32, Refusal> {
    match data.get(&key.into()) {
        Some(Tag::Int(value)) => Ok(*value),
        Some(_) => Err(Refusal::Kind { key }),
        None => Err(Refusal::Missing { key }),
    }
}

fn nested_int(tag: &Tag, key: &'static str) -> Result<i32, Refusal> {
    match tag.get(key) {
        Some(Tag::Int(value)) => Ok(*value),
        Some(_) => Err(Refusal::Kind { key }),
        None => Err(Refusal::Missing { key }),
    }
}

/// A chunk's claim record as a field.
pub fn chunk_cap(value: &ChunkCap) -> Tag {
    Tag::Compound(value.to_record())
}

/// The record itself, for a list of them (`UpdateChunkRangeCapability`).
pub fn chunk_cap_record(value: &ChunkCap) -> Compound {
    value.to_record()
}

/// Reads a chunk claim written by [`chunk_cap`].
pub fn chunk_cap_of(data: &Compound, key: &'static str) -> Result<ChunkCap, Refusal> {
    let tag = field(data, key)?;
    if !matches!(tag, Tag::Compound(_)) {
        return Err(Refusal::Kind { key });
    }
    match tag {
        Tag::Compound(record) => ChunkCap::from_record(record),
        _ => Err(Refusal::Kind { key }),
    }
}

/// Reads a record written by [`chunk_cap_record`], from a compound already in hand.
pub fn chunk_cap_record_of(record: &Compound) -> Result<ChunkCap, Refusal> {
    ChunkCap::from_record(record)
}

/// One pathfinding node, as the debug packet writes it
/// (`core/entity/pathfinding/MNode.java:102-136`).
///
/// Java writes the parent's position only, "for debug display only" (`:125`), and
/// the three coordinates as `writeVarInt`. The port writes them as `Int`: a varint
/// is a width optimisation on a value that may be negative, and the number it
/// decodes to is the same.
#[derive(Clone, Debug, PartialEq)]
pub struct PathNode {
    /// `MNode.parent`, kept as its position: the search owns the pointers.
    pub parent: Option<[i32; 3]>,
    pub at: [i32; 3],
    /// `MNode.cost` — the A\* `g` value, the cost from the start.
    pub cost: f64,
    /// `MNode.heuristic` — the estimate to the target.
    pub heuristic: f64,
    /// `MNode.isReachedByWorker`.
    pub reached: bool,
}

impl PathNode {
    fn to_tag(&self) -> Tag {
        let mut tag = Compound::from([
            ("x".into(), int(self.at[0])),
            ("y".into(), int(self.at[1])),
            ("z".into(), int(self.at[2])),
            ("cost".into(), double(self.cost)),
            ("heuristic".into(), double(self.heuristic)),
            ("reached".into(), boolean(self.reached)),
        ]);
        if let Some(parent) = self.parent {
            tag.insert("parent".into(), position(parent));
        }
        Tag::Compound(tag)
    }

    fn from_tag(tag: &Tag) -> Result<PathNode, Refusal> {
        Ok(PathNode {
            parent: match tag.get("parent") {
                Some(value) => Some([
                    nested_int(value, "x")?,
                    nested_int(value, "y")?,
                    nested_int(value, "z")?,
                ]),
                None => None,
            },
            at: [nested_int(tag, "x")?, nested_int(tag, "y")?, nested_int(tag, "z")?],
            cost: nested_double(tag, "cost")?,
            heuristic: nested_double(tag, "heuristic")?,
            reached: match tag.get("reached") {
                Some(Tag::Byte(flag)) => *flag != 0,
                Some(_) => return Err(Refusal::Kind { key: "reached" }),
                None => return Err(Refusal::Missing { key: "reached" }),
            },
        })
    }
}

fn nested_double(tag: &Tag, key: &'static str) -> Result<f64, Refusal> {
    match tag.get(key) {
        Some(Tag::Double(bits)) => Ok(f64::from_bits(*bits)),
        Some(_) => Err(Refusal::Kind { key }),
        None => Err(Refusal::Missing { key }),
    }
}

/// A list of nodes, and back.
pub fn nodes(values: &[PathNode]) -> Tag {
    Tag::List(values.iter().map(PathNode::to_tag).collect())
}

/// Reads a list of nodes written by [`nodes`].
pub fn nodes_of(data: &Compound, key: &'static str) -> Result<Vec<PathNode>, Refusal> {
    let mut nodes = Vec::new();
    for value in list_of(data, key)? {
        match value {
            Tag::Compound(record) => nodes.push(PathNode::from_tag(&Tag::Compound(record.clone()))?),
            _ => return Err(Refusal::Kind { key }),
        }
    }
    Ok(nodes)
}

/// One `Rank` read from its Java id, or [`Refusal::Name`]: the port knows five
/// ranks (`colony::permissions::Rank`), Java lets a colony add more
/// (`Permissions.java:1054-1076`).
pub fn rank_of(data: &Compound, key: &'static str) -> Result<Rank, Refusal> {
    let id = int_of(data, key)?;
    Rank::from_id(id).ok_or(Refusal::Name { key })
}

#[cfg(test)]
pub(crate) use self::tests::round_trip;

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn round_trip<M>(message: M) -> M
    where
        M: Message + Clone + PartialEq + std::fmt::Debug,
    {
        let written = message.to_compound();
        let read = M::from_compound(&written).expect("a message this port wrote reads back");
        assert_eq!(read, message, "round trip changed the message");
        let again = read.to_compound();
        assert_eq!(again, written, "writing twice differs");
        read
    }

    fn sample() -> Compound {
        Compound::from([
            ("dimensionId".into(), text("minecraft:overworld")),
            ("colonyId".into(), Tag::Int(7)),
            ("buildingId".into(), position([10, 64, -30])),
        ])
    }

    #[test]
    fn the_colony_envelope_round_trips_and_a_missing_level_is_refused() {
        let data = sample();
        assert_eq!(
            envelope_of(&data),
            Ok((Dimension::new("minecraft:overworld"), 7)),
            "the two fields AbstractColonyServerMessage writes"
        );
        assert_eq!(building_envelope_of(&data), Ok((Dimension::new("minecraft:overworld"), 7, [10, 64, -30])));
        let mut data = sample();
        data.remove(&"dimensionId".into());
        assert_eq!(envelope_of(&data), Err(Refusal::Missing { key: KEY_DIMENSION }));
    }

    #[test]
    fn an_unqualified_dimension_is_a_name_refusal() {
        let data = Compound::from([("dimensionId".into(), text("overworld"))]);
        assert_eq!(dimension_of(&data, KEY_DIMENSION), Err(Refusal::Name { key: KEY_DIMENSION }));
        let data = Compound::from([("dimensionId".into(), Tag::Int(0))]);
        assert_eq!(dimension_of(&data, KEY_DIMENSION), Err(Refusal::Kind { key: KEY_DIMENSION }));
    }

    #[test]
    fn every_writer_reads_back_and_a_wrong_kind_is_named() {
        let data = Compound::from([
            ("s".into(), text("hello")),
            ("i".into(), int(-4)),
            ("l".into(), long(1 << 40)),
            ("b".into(), boolean(true)),
            ("d".into(), double(-0.0)),
            ("f".into(), float(0.5)),
            ("p".into(), position([1, 2, 3])),
            ("u".into(), uuid(0x0123_4567_89ab_cdef_0123_4567_89ab_cdef)),
            ("y".into(), bytes(&[1, 2, 3])),
            ("n".into(), nested(&sample())),
            ("t".into(), texts(&["a".to_owned(), "b".to_owned()])),
        ]);
        assert_eq!(text_of(&data, "s"), Ok("hello".to_owned()));
        assert_eq!(int_of(&data, "i"), Ok(-4));
        assert_eq!(long_of(&data, "l"), Ok(1 << 40));
        assert_eq!(bool_of(&data, "b"), Ok(true));
        assert_eq!(double_of(&data, "d").map(|value| value.to_bits()), Ok((-0.0f64).to_bits()));
        assert_eq!(float_of(&data, "f"), Ok(0.5));
        assert_eq!(pos_of(&data, "p"), Ok([1, 2, 3]));
        assert_eq!(uuid_of(&data, "u"), Ok(0x0123_4567_89ab_cdef_0123_4567_89ab_cdef));
        assert_eq!(bytes_of(&data, "y"), Ok(vec![1, 2, 3]));
        assert_eq!(nested_of(&data, "n"), Ok(sample()));
        assert_eq!(texts_of(&data, "t"), Ok(vec!["a".to_owned(), "b".to_owned()]));
        assert_eq!(text_of(&data, "i"), Err(Refusal::Kind { key: "i" }));
        assert_eq!(int_of(&data, "s"), Err(Refusal::Kind { key: "s" }));
        assert_eq!(int_of(&data, "absent"), Err(Refusal::Missing { key: "absent" }));
    }

    #[test]
    fn an_unparsable_uuid_is_a_name_refusal_and_a_bad_item_too() {
        let data = Compound::from([("player".into(), text("not-a-uuid"))]);
        assert_eq!(uuid_of(&data, "player"), Err(Refusal::Name { key: "player" }));
        assert_eq!(ItemStack::new("", 1), Err(Refusal::Name { key: "item" }));
        assert_eq!(ItemStack::new("minecraft:stone", -1).err(), Some(Refusal::Number {
            key: "count",
            min: 0,
            max: i64::from(i32::MAX)
        }));
    }

    #[test]
    fn a_nested_bounds_check_refuses_rather_than_clamps() {
        let data = Compound::from([("count".into(), Tag::Int(99))]);
        assert_eq!(int_in(&data, "count", 0, 10), Err(Refusal::Number { key: "count", min: 0, max: 10 }));
        assert_eq!(int_in(&data, "count", 0, 100), Ok(99));
    }

    #[test]
    fn a_component_and_an_item_stack_survive_an_empty_argument_list() {
        let component = Component::translatable("com.minecolonies.core.founding.tooclosetospawn").with_argument("120");
        let tag = component.to_tag();
        assert_eq!(Component::from_tag(&tag), Ok(component));
        assert_eq!(Component::from_tag(&Tag::Compound(Compound::new())), Err(Refusal::Missing { key: "text" }));

        let stack = ItemStack::new("minecraft:iron_ingot", 0).expect("an empty stack is written");
        assert!(stack.is_empty());
        assert_eq!(ItemStack::from_tag(&stack.to_tag()), Ok(stack));
    }

    #[test]
    fn a_chunk_cap_and_a_path_node_round_trip_through_their_nested_form() {
        let cap = ChunkCap { x: -3, z: 12, owning_colony: 4, claiming_colonies: vec![4, 9] };
        assert_eq!(chunk_cap_record_of(&chunk_cap_record(&cap)), Ok(cap));

        let node = PathNode {
            parent: Some([0, 63, 0]),
            at: [1, 64, 0],
            cost: 1.25,
            heuristic: 8.0,
            reached: true,
        };
        let data = Compound::from([("nodes".into(), nodes(std::slice::from_ref(&node)))]);
        assert_eq!(nodes_of(&data, "nodes"), Ok(vec![node]));
        let orphan = PathNode { parent: None, ..nodes_of(&data, "nodes").expect("read")[0].clone() };
        let data = Compound::from([("nodes".into(), nodes(std::slice::from_ref(&orphan)))]);
        assert_eq!(nodes_of(&data, "nodes"), Ok(vec![orphan]), "a node without a parent is legal");
    }

    #[test]
    fn a_rank_id_the_port_lacks_is_refused_and_the_five_it_has_read() {
        for rank in Rank::ALL {
            let data = Compound::from([("rank".into(), int(rank.id()))]);
            assert_eq!(rank_of(&data, "rank"), Ok(rank));
        }
        let data = Compound::from([("rank".into(), int(17))]);
        assert_eq!(rank_of(&data, "rank"), Err(Refusal::Name { key: "rank" }));
    }

    #[test]
    fn every_refusal_has_its_own_name_and_names_its_field() {
        let all = [
            Refusal::Missing { key: "a" },
            Refusal::Kind { key: "b" },
            Refusal::Name { key: "c" },
            Refusal::Size { key: "d", max: 3 },
            Refusal::Number { key: "e", min: 0, max: 1 },
        ];
        for (index, refusal) in all.iter().enumerate() {
            assert!(refusal.name().starts_with("network.refusal."));
            for other in &all[index + 1..] {
                assert_ne!(refusal.name(), other.name(), "two refusals share a name");
            }
        }
        assert_eq!(Refusal::Missing { key: "colonyId" }.to_string(), "network.refusal.missing: colonyId");
    }

    #[test]
    fn the_five_gates_answer_the_four_java_overrides_and_one_extra() {
        assert_eq!(Rule::None.action(), None);
        assert_eq!(Rule::ManageHuts.action(), Some(Action::ManageHuts));
        assert_eq!(Rule::EditPermissions.action(), Some(Action::EditPermissions));
        assert_eq!(Rule::PlaceBlocks.action(), Some(Action::PlaceBlocks));
        assert_eq!(Rule::OwnerOnly.action(), None, "the owner gate is not an action");

        let members = crate::colony::Members::with_owner(1);
        assert!(Rule::None.allows(&members, 1, 99));
        assert!(Rule::OwnerOnly.allows(&members, 1, 1));
        assert!(!Rule::OwnerOnly.allows(&members, 1, 99));
        assert!(Rule::ManageHuts.allows(&members, 1, 1));
        assert!(!Rule::ManageHuts.allows(&members, 1, 99), "a stranger holds no MANAGE_HUTS");
    }
}
