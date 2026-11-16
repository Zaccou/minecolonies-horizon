//! Colony settings: the town hall's name, its three styles, its banner, its colour,
//! the blocks guests may touch, the mercenaries and spies it can buy, and the gate
//! that teleports a visitor to an allied colony.
//!
//! Java reference: `core/network/messages/server/colony/TownHallRenameMessage.java`
//! (61), `ColonyNameStyleMessage.java` (56), `ColonyStructureStyleMessage.java` (56),
//! `ColonyTextureStyleMessage.java` (56), `ColonyFlagChangeMessage.java` (68),
//! `TeamColonyColorChangeMessage.java` (73), `ChangeFreeToInteractBlockMessage.java`
//! (173), `HireMercenaryMessage.java` (60), `HireSpiesMessage.java` (59),
//! `TriggerConnectionEventMessage.java` (77), `TeleportToColonyMessage.java` (151).
//!
//! **Nine of the eleven extend `AbstractColonyServerMessage`**, so they carry the
//! dimension and the colony id and nothing else of their own beyond one value — and
//! they inherit its gate, `MANAGE_HUTS` (`AbstractColonyServerMessage.java:60-64`).
//! Two override it: `ChangeFreeToInteractBlockMessage` asks `EDIT_PERMISSIONS`
//! (`:103-108`), and `TeleportToColonyMessage` asks for **no permission at all**
//! (`:63-68` returns `null`), because the two colonies it belongs to decide with
//! their own diplomacy and their own `TELEPORT_TO_COLONY` bit (`:84-90`).
//!
//! **The two empty ones are not empty on the wire by accident.**
//! `HireMercenaryMessage` and `HireSpiesMessage` carry only the envelope
//! (`:48-58`, `:46-57`): what they buy is priced in the colony, not in the message.
//!
//! **Java truncates a town hall's name and the port refuses it.** `TownHallRename`
//! keeps 25 characters and cuts at 24 when longer
//! (`TownHallRenameMessage.java:17-18`, `:38`, `:56`) — a silent rename to something
//! the player did not type. [`TownHallRename::new`] does the same cut, so a caller
//! that builds a message gets Java's behaviour, and `from_compound` **refuses** a
//! longer name (which Java can never send), so a wire that already carries one is
//! caught instead of quietly shortened.

use super::common::{
    envelope, envelope_of, int, int_in, int_of, pos_of, position, records, records_of, text, text_of,
    Message, Refusal, Rule,
};
use crate::base::Dimension;
use mod_api::Compound;

/// `TownHallRenameMessage.java:17-18` — `MAX_NAME_LENGTH` and the cut it makes.
pub const MAX_COLONY_NAME: usize = 25;

/// `TeamColonyColorChangeMessage.java:69` — `ChatFormatting.values()` has
/// twenty-two entries in 26.3, and an ordinal past the last one makes Java throw.
pub const MAX_COLOR_ORDINAL: i32 = 21;

/// `TownHallRenameMessage.java:15-60` — rename the colony.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TownHallRename {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub name: String,
}

impl TownHallRename {
    /// Java's own cut: 25 characters or fewer is kept, anything longer loses its
    /// tail (`:38`).
    pub fn new(dimension: &Dimension, colony_id: i32, name: &str) -> TownHallRename {
        TownHallRename { colony_id, dimension: dimension.clone(), name: capped(name) }
    }
}

fn capped(name: &str) -> String {
    let units: Vec<u16> = name.encode_utf16().collect();
    if units.len() <= MAX_COLONY_NAME {
        return name.to_owned();
    }
    String::from_utf16_lossy(&units[..MAX_COLONY_NAME - 1])
}

impl Message for TownHallRename {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert("name".into(), text(&self.name));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        let name = text_of(data, "name")?;
        if name.encode_utf16().count() > MAX_COLONY_NAME {
            return Err(Refusal::Size { key: "name", max: MAX_COLONY_NAME });
        }
        Ok(TownHallRename { colony_id, dimension, name })
    }
}

/// `ColonyNameStyleMessage.java:11-55` — how the colony's name is drawn on the map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColonyNameStyle {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub style: String,
}

impl Message for ColonyNameStyle {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert("style".into(), text(&self.style));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        Ok(ColonyNameStyle { colony_id, dimension, style: text_of(data, "style")? })
    }
}

/// `ColonyStructureStyleMessage.java:11-55` — which structure pack every new hut
/// builds from (`:41`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColonyStructureStyle {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub pack: String,
}

impl Message for ColonyStructureStyle {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert("pack".into(), text(&self.pack));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        Ok(ColonyStructureStyle { colony_id, dimension, pack: text_of(data, "pack")? })
    }
}

/// `ColonyTextureStyleMessage.java:11-55` — which texture pack the colony's blocks
/// use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColonyTextureStyle {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub style: String,
}

impl Message for ColonyTextureStyle {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert("style".into(), text(&self.style));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        Ok(ColonyTextureStyle { colony_id, dimension, style: text_of(data, "style")? })
    }
}

/// `ColonyFlagChangeMessage.java:18-67` — the banner the colony flies.
///
/// Java wraps the `ListTag` in a compound under `TAG_BANNER_PATTERNS` and writes
/// that (`:53-55`), reading it back with `getList(TAG_BANNER_PATTERNS, …)`
/// (`:61-65`). The port keeps the list as a list of compounds, which is what Java's
/// `ListTag<TAG_COMPOUND>` is; the wrapper exists only because `writeNbt` needs a
/// root compound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColonyFlagChange {
    pub colony_id: i32,
    pub dimension: Dimension,
    /// One compound per banner layer, in the order the banner draws them.
    pub patterns: Vec<Compound>,
}

impl Message for ColonyFlagChange {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert("bannerPatterns".into(), records(&self.patterns));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        let patterns =
            records_of(data, "bannerPatterns")?.into_iter().cloned().collect::<Vec<Compound>>();
        Ok(ColonyFlagChange { colony_id, dimension, patterns })
    }
}

/// `TeamColonyColorChangeMessage.java:16-73` — the colony's team colour, as a
/// `ChatFormatting` ordinal (`:69`).
///
/// **An ordinal, not a name.** `ChatFormatting` is vanilla's, and this port owns no
/// type for it (`common`'s note says why two enums stay integers); what the port
/// does own is the bound, so an ordinal past the last colour is
/// [`Refusal::Number`] instead of Java's array-index throw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamColonyColorChange {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub color_ordinal: i32,
}

impl Message for TeamColonyColorChange {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert("colorOrdinal".into(), int(self.color_ordinal));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        Ok(TeamColonyColorChange {
            colony_id,
            dimension,
            color_ordinal: int_in(data, "colorOrdinal", 0, MAX_COLOR_ORDINAL)?,
        })
    }
}

/// What the sceptre is asked to add or remove
/// (`ChangeFreeToInteractBlockMessage.java:158-162`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two-value enum of Java's MessageType
pub enum MessageType {
    /// `REMOVE_BLOCK` (`:160`) — the ordinal `0`.
    RemoveBlock,
    /// `ADD_BLOCK` (`:161`) — the ordinal `1`.
    AddBlock,
}

impl MessageType {
    pub const ALL: [MessageType; 2] = [MessageType::RemoveBlock, MessageType::AddBlock];

    pub fn name(self) -> &'static str {
        match self {
            MessageType::RemoveBlock => "REMOVE_BLOCK",
            MessageType::AddBlock => "ADD_BLOCK",
        }
    }

    pub fn parse(name: &str) -> Option<MessageType> {
        MessageType::ALL.into_iter().find(|kind| kind.name() == name)
    }
}

/// Whether the sceptre added a *place* or a *block kind*
/// (`ChangeFreeToInteractBlockMessage.java:167-171`). The two need different
/// arguments: a position, or a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two-value enum of Java's MessageMode
pub enum MessageMode {
    /// `LOCATION` (`:169`) — the payload is a position.
    Location,
    /// `BLOCK` (`:170`) — the payload is a block state.
    Block,
}

impl MessageMode {
    pub const ALL: [MessageMode; 2] = [MessageMode::Location, MessageMode::Block];

    pub fn name(self) -> &'static str {
        match self {
            MessageMode::Location => "LOCATION",
            MessageMode::Block => "BLOCK",
        }
    }

    pub fn parse(name: &str) -> Option<MessageMode> {
        MessageMode::ALL.into_iter().find(|mode| mode.name() == name)
    }
}

/// `ChangeFreeToInteractBlockMessage.java:24-172` — the permission sceptre: let
/// everybody touch this block, or this kind of block.
///
/// The gate is `EDIT_PERMISSIONS` (`:103-108`), the only message in this family that
/// asks for something other than `MANAGE_HUTS` besides the teleport.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangeFreeToInteractBlock {
    pub colony_id: i32,
    pub dimension: Dimension,
    /// `Block.getId(block)` (`:97-98`): the block state's registry id, carried as
    /// it is (`MANQUANT.md` §3). Defaults to dirt's state on the Java side
    /// (`:35`).
    pub block: i32,
    pub pos: [i32; 3],
    pub kind: MessageType,
    pub mode: MessageMode,
}

impl Message for ChangeFreeToInteractBlock {
    const RULE: Rule = Rule::EditPermissions;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert("block".into(), int(self.block));
        data.insert("pos".into(), position(self.pos));
        data.insert("type".into(), text(self.kind.name()));
        data.insert("mode".into(), text(self.mode.name()));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        let kind = text_of(data, "type")?;
        let mode = text_of(data, "mode")?;
        Ok(ChangeFreeToInteractBlock {
            colony_id,
            dimension,
            block: int_of(data, "block")?,
            pos: pos_of(data, "pos")?,
            kind: MessageType::parse(&kind).ok_or(Refusal::Name { key: "type" })?,
            mode: MessageMode::parse(&mode).ok_or(Refusal::Name { key: "mode" })?,
        })
    }
}

/// `HireMercenaryMessage.java:16-60` — buy a wave of mercenaries
/// (`:43`, `EntityMercenary.spawnMercenariesInColony`).
///
/// The envelope and nothing else: what is hired and at what price lives in the
/// colony (`:43`), and the message is a click with a permission attached.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HireMercenary {
    pub colony_id: i32,
    pub dimension: Dimension,
}

impl Message for HireMercenary {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        Ok(HireMercenary { colony_id, dimension })
    }
}

/// `HireSpiesMessage.java:18-59` — pay gold ingots for spies
/// (`BuildingBarracks.SPIES_GOLD_COST`, `:38-43`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HireSpies {
    pub colony_id: i32,
    pub dimension: Dimension,
}

impl Message for HireSpies {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        Ok(HireSpies { colony_id, dimension })
    }
}

/// `TriggerConnectionEventMessage.java:17-77` — tell another colony that a
/// connection event happened (a raid repelled, a gift given).
///
/// `ConnectionEvent` is serialized by its own `serializeByteBuf`
/// (`:66`), a class of `api/colony/connections` this port has not ported, so the
/// event travels as bytes (`MANQUANT.md` §1) and the target colony id travels beside
/// it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TriggerConnectionEvent {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub event: Vec<u8>,
    pub target_colony_id: i32,
}

impl Message for TriggerConnectionEvent {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert("connectionEvent".into(), super::common::bytes(&self.event));
        data.insert("targetColonyId".into(), int(self.target_colony_id));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        Ok(TriggerConnectionEvent {
            colony_id,
            dimension,
            event: super::common::bytes_of(data, "connectionEvent")?,
            target_colony_id: int_of(data, "targetColonyId")?,
        })
    }
}

/// `TeleportToColonyMessage.java:33-151` — take the gate house to an allied colony,
/// for a price in gold nuggets (`:94-116`).
///
/// `permissionNeeded()` is `null` here (`:63-68`): the two colonies decide, not a
/// rank — the origin's diplomacy status must be `ALLIES` (`:84-87`) and either end
/// must hold `TELEPORT_TO_COLONY` (`:89`). [`Rule::None`] is exactly that "ask the
/// colony, not the rank".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeleportToColony {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub pos: [i32; 3],
    /// The colony the player is leaving (`:38`), whose diplomacy is read.
    pub origin_colony_id: i32,
    /// The toll, in gold nuggets; `0` when the gate is free (`:94`).
    pub cost: i32,
}

impl Message for TeleportToColony {
    const RULE: Rule = Rule::None;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert("pos".into(), position(self.pos));
        data.insert("originColonyId".into(), int(self.origin_colony_id));
        data.insert("cost".into(), int(self.cost));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        let cost = int_of(data, "cost")?;
        if cost < 0 {
            return Err(Refusal::Number { key: "cost", min: 0, max: i64::from(i32::MAX) });
        }
        Ok(TeleportToColony {
            colony_id,
            dimension,
            pos: pos_of(data, "pos")?,
            origin_colony_id: int_of(data, "originColonyId")?,
            cost,
        })
    }
}

/// The eleven messages of this family, by Java file and Rust type.
pub const CENSUS: &[(&str, &str)] = &[
    ("server/colony/TownHallRenameMessage.java", "TownHallRename"),
    ("server/colony/ColonyNameStyleMessage.java", "ColonyNameStyle"),
    ("server/colony/ColonyStructureStyleMessage.java", "ColonyStructureStyle"),
    ("server/colony/ColonyTextureStyleMessage.java", "ColonyTextureStyle"),
    ("server/colony/ColonyFlagChangeMessage.java", "ColonyFlagChange"),
    ("server/colony/TeamColonyColorChangeMessage.java", "TeamColonyColorChange"),
    ("server/colony/ChangeFreeToInteractBlockMessage.java", "ChangeFreeToInteractBlock"),
    ("server/colony/HireMercenaryMessage.java", "HireMercenary"),
    ("server/colony/HireSpiesMessage.java", "HireSpies"),
    ("server/colony/TriggerConnectionEventMessage.java", "TriggerConnectionEvent"),
    ("server/colony/TeleportToColonyMessage.java", "TeleportToColony"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colony::Action;
    use crate::network::common::round_trip;

    fn overworld() -> Dimension {
        Dimension::new("minecraft:overworld")
    }

    #[test]
    fn the_three_style_messages_round_trip() {
        assert_eq!(
            round_trip(ColonyNameStyle {
                colony_id: 2,
                dimension: overworld(),
                style: "medieval".to_owned()
            })
            .style,
            "medieval"
        );
        assert_eq!(
            round_trip(ColonyStructureStyle {
                colony_id: 2,
                dimension: overworld(),
                pack: "minecolonies".to_owned()
            })
            .pack,
            "minecolonies"
        );
        assert_eq!(
            round_trip(ColonyTextureStyle {
                colony_id: 2,
                dimension: overworld(),
                style: "default".to_owned()
            })
            .style,
            "default"
        );
    }

    #[test]
    fn a_town_hall_name_is_cut_like_java_and_refused_when_the_wire_carries_more() {
        let long = "a".repeat(40);
        let renamed = TownHallRename::new(&overworld(), 1, &long);
        assert_eq!(renamed.name.encode_utf16().count(), MAX_COLONY_NAME - 1, "Java's substring(0, 24)");
        assert_eq!(round_trip(renamed).name, "a".repeat(24));

        let mut data =
            TownHallRename { colony_id: 1, dimension: overworld(), name: "ok".to_owned() }.to_compound();
        data.insert("name".into(), text(&long));
        assert_eq!(
            TownHallRename::from_compound(&data),
            Err(Refusal::Size { key: "name", max: MAX_COLONY_NAME })
        );
    }

    #[test]
    fn a_banner_round_trips_layer_by_layer() {
        let patterns = vec![
            Compound::from([("pattern".into(), text("minecraft:base"))]),
            Compound::from([("pattern".into(), text("minecraft:cross")), ("color".into(), int(14))]),
        ];
        let flag = ColonyFlagChange { colony_id: 5, dimension: overworld(), patterns };
        assert_eq!(round_trip(flag.clone()).patterns, flag.patterns);
    }

    #[test]
    fn a_colour_ordinal_past_the_last_formatting_is_refused() {
        let mut data =
            TeamColonyColorChange { colony_id: 5, dimension: overworld(), color_ordinal: 3 }.to_compound();
        assert_eq!(TeamColonyColorChange::from_compound(&data).map(|m| m.color_ordinal), Ok(3));
        data.insert("colorOrdinal".into(), int(22));
        assert_eq!(
            TeamColonyColorChange::from_compound(&data),
            Err(Refusal::Number { key: "colorOrdinal", min: 0, max: i64::from(MAX_COLOR_ORDINAL) })
        );
    }

    #[test]
    fn the_sceptre_round_trips_and_refuses_an_unknown_mode() {
        let sceptre = ChangeFreeToInteractBlock {
            colony_id: 6,
            dimension: overworld(),
            block: 42,
            pos: [3, 64, -8],
            kind: MessageType::AddBlock,
            mode: MessageMode::Location,
        };
        let read = round_trip(sceptre);
        assert_eq!((read.kind, read.mode), (MessageType::AddBlock, MessageMode::Location));

        let mut data = ChangeFreeToInteractBlock {
            colony_id: 6,
            dimension: overworld(),
            block: 0,
            pos: [0, 0, 0],
            kind: MessageType::RemoveBlock,
            mode: MessageMode::Block,
        }
        .to_compound();
        data.insert("mode".into(), text("EVERYTHING"));
        assert_eq!(ChangeFreeToInteractBlock::from_compound(&data), Err(Refusal::Name { key: "mode" }));
        data.insert("mode".into(), text("BLOCK"));
        data.insert("type".into(), text("add_block"));
        assert_eq!(ChangeFreeToInteractBlock::from_compound(&data), Err(Refusal::Name { key: "type" }));
    }

    #[test]
    fn the_two_purchases_round_trip_with_the_envelope_alone() {
        let mercenary = round_trip(HireMercenary { colony_id: 8, dimension: overworld() });
        assert_eq!(mercenary.colony_id, 8);
        assert!(mercenary.to_compound().contains_key(&"colonyId".into()));
        round_trip(HireSpies { colony_id: 8, dimension: Dimension::new("minecraft:the_nether") });
    }

    #[test]
    fn a_connection_event_round_trips_as_bytes_beside_its_target() {
        let event = TriggerConnectionEvent {
            colony_id: 1,
            dimension: overworld(),
            event: vec![9, 8, 7],
            target_colony_id: 2,
        };
        assert_eq!(round_trip(event).event, vec![9, 8, 7]);
    }

    #[test]
    fn a_teleport_round_trips_and_refuses_a_negative_toll() {
        let gate = TeleportToColony {
            colony_id: 2,
            dimension: overworld(),
            pos: [10, 65, 10],
            origin_colony_id: 1,
            cost: 8,
        };
        assert_eq!(round_trip(gate).cost, 8);
        let mut data = TeleportToColony {
            colony_id: 2,
            dimension: overworld(),
            pos: [0, 0, 0],
            origin_colony_id: 1,
            cost: 0,
        }
        .to_compound();
        data.insert("cost".into(), int(-3));
        assert_eq!(
            TeleportToColony::from_compound(&data),
            Err(Refusal::Number { key: "cost", min: 0, max: i64::from(i32::MAX) })
        );
    }

    #[test]
    fn the_gates_are_the_ones_java_overrides() {
        assert_eq!(TownHallRename::RULE, Rule::ManageHuts);
        assert_eq!(ChangeFreeToInteractBlock::RULE, Rule::EditPermissions);
        assert_eq!(TeleportToColony::RULE, Rule::None, "permissionNeeded() returns null");
        assert_eq!(ChangeFreeToInteractBlock::RULE.action(), Some(Action::EditPermissions));
        assert_eq!(CENSUS.len(), 11);
    }

    #[test]
    fn every_mode_and_type_name_parses_back() {
        for kind in MessageType::ALL {
            assert_eq!(MessageType::parse(kind.name()), Some(kind));
        }
        for mode in MessageMode::ALL {
            assert_eq!(MessageMode::parse(mode.name()), Some(mode));
        }
        assert_eq!(MessageType::parse("DELETE"), None);
        assert_eq!(MessageMode::parse("position"), None);
    }
}
