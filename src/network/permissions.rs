//! `PermissionsMessage`'s nine classes: the pane that shows who may do what, and
//! the eight edits a colony manager can make to it.
//!
//! Java reference: `core/network/messages/PermissionsMessage.java` (724 lines).
//! The outer class holds **no state at all** — it is a namespace for nine nested
//! `IMessage`s (`:35-36`), and the port keeps that shape: this file is the
//! namespace, the nine structs are the messages.
//!
//! **Seven of the nine are the same three fields** — the colony, its dimension, and
//! one payload — because that is exactly what the permission pane needs to name the
//! colony it is editing and to be refused if it is the wrong one. `View` is the odd
//! one: it carries the whole serialized pane as an opaque buffer
//! (`:99-105`), because the pane is rendered by `Permissions.serializeViewNetworkData`
//! — a 1 091-line class this port has not ported.
//!
//! **A rank is an id, an action is a name.** Java writes `rank.getId()`
//! (`:174`) and reads it back through the colony's rank table (`:190`), so the port
//! keeps [`Rank`] and its id; an id the port does not know is
//! [`Refusal::Name`], never a silent `Neutral`. An action travels as
//! `action.name()` (`:175`) — the `Action` enum is `api`'s, twenty-seven values,
//! and the port's [`Action`] has the same twenty-seven bits with different
//! spellings, so [`action_name`] / [`action_from_name`] are the one translation
//! table between them and a test holds it to [`Action::bit`].
//!
//! **None of the nine is gated here.** They do not extend
//! `AbstractColonyServerMessage`; each one checks `EDIT_PERMISSIONS` or
//! `MANAGE_HUTS` inside its own `onExecute` (`:258`, `:328`, `:403`, `:485`,
//! `:562-566`, `:632`, `:702`), which is a colony call and belongs to the layer
//! that owns the colony. [`Message::RULE`] is [`crate::network::common::Rule::None`] for all nine, as
//! `RAPPORT.md` §"ce que je n'ai pas copié" records.

use super::common::{
    boolean, bytes, envelope, envelope_of, int, int_of, rank_of, text, text_of, uuid, uuid_of,
    Message, Refusal,
};
use crate::base::Dimension;
use crate::colony::{Action, Rank};
use mod_api::Compound;

/// The payload key each class names after its field. `colonyId` and `dimensionId`
/// are the shared two (`common::KEY_COLONY`, `common::KEY_DIMENSION`); these are
/// the rest.
pub const KEY_DATA: &str = "data";
pub const KEY_ENABLE: &str = "enable";
pub const KEY_RANK: &str = "rank";
pub const KEY_ACTION: &str = "action";
pub const KEY_PLAYER_NAME: &str = "playerName";
pub const KEY_RANK_NAME: &str = "rankName";
pub const KEY_PLAYER: &str = "playerID";
pub const KEY_RANK_ID: &str = "rankId";
pub const KEY_RANK_TYPE: &str = "rankType";

/// Java's `Action` names, with the port's variant and the bit it occupies
/// (`api/colony/permissions/Action.java:9-43`). Twenty-seven rows; `bit` is
/// checked against [`Action::bit`] by a test, so the table cannot drift from the
/// enum it translates.
const JAVA_ACTIONS: [(&str, Action); 27] = [
    ("ACCESS_HUTS", Action::AccessHuts),
    ("PLACE_HUTS", Action::PlaceHuts),
    ("BREAK_HUTS", Action::BreakHuts),
    ("EDIT_PERMISSIONS", Action::EditPermissions),
    ("MANAGE_HUTS", Action::ManageHuts),
    ("RECEIVE_MESSAGES", Action::ReceiveMessages),
    ("USE_SCAN_TOOL", Action::UseScanTool),
    ("PLACE_BLOCKS", Action::PlaceBlocks),
    ("BREAK_BLOCKS", Action::BreakBlocks),
    ("TOSS_ITEM", Action::TossItem),
    ("PICKUP_ITEM", Action::PickupItem),
    ("FILL_BUCKET", Action::FillBucket),
    ("OPEN_CONTAINER", Action::OpenContainer),
    ("RIGHTCLICK_BLOCK", Action::RightClickBlock),
    ("RIGHTCLICK_ENTITY", Action::RightClickEntity),
    ("THROW_POTION", Action::ThrowPotion),
    ("SHOOT_ARROW", Action::ShootArrow),
    ("ATTACK_CITIZEN", Action::AttackCitizen),
    ("ATTACK_ENTITY", Action::AttackEntity),
    ("TELEPORT_TO_COLONY", Action::TeleportToColony),
    ("EXPLODE", Action::Explode),
    ("RALLY_GUARDS", Action::RallyGuards),
    ("HURT_CITIZEN", Action::HurtCitizen),
    ("HURT_VISITOR", Action::HurtVisitor),
    ("MAP_BORDER", Action::MapBorder),
    ("MAP_DEATHS", Action::MapDeaths),
    ("ACCESS_TOGGLEABLES", Action::AccessToggleables),
];

/// The name Java writes for an action: `action.name()` (`:175`).
///
/// **An exhaustive match, not a lookup.** A lookup into the table above would need a
/// fallback for the day `colony::permissions::Action` grows a variant — and a fallback
/// here is a wrong name on the wire. The match makes that day a compile error, and the
/// test below holds the two directions to each other.
pub fn action_name(action: Action) -> &'static str {
    match action {
        Action::AccessHuts => "ACCESS_HUTS",
        Action::PlaceHuts => "PLACE_HUTS",
        Action::BreakHuts => "BREAK_HUTS",
        Action::EditPermissions => "EDIT_PERMISSIONS",
        Action::ManageHuts => "MANAGE_HUTS",
        Action::ReceiveMessages => "RECEIVE_MESSAGES",
        Action::UseScanTool => "USE_SCAN_TOOL",
        Action::PlaceBlocks => "PLACE_BLOCKS",
        Action::BreakBlocks => "BREAK_BLOCKS",
        Action::TossItem => "TOSS_ITEM",
        Action::PickupItem => "PICKUP_ITEM",
        Action::FillBucket => "FILL_BUCKET",
        Action::OpenContainer => "OPEN_CONTAINER",
        Action::RightClickBlock => "RIGHTCLICK_BLOCK",
        Action::RightClickEntity => "RIGHTCLICK_ENTITY",
        Action::ThrowPotion => "THROW_POTION",
        Action::ShootArrow => "SHOOT_ARROW",
        Action::AttackCitizen => "ATTACK_CITIZEN",
        Action::AttackEntity => "ATTACK_ENTITY",
        Action::TeleportToColony => "TELEPORT_TO_COLONY",
        Action::Explode => "EXPLODE",
        Action::RallyGuards => "RALLY_GUARDS",
        Action::HurtCitizen => "HURT_CITIZEN",
        Action::HurtVisitor => "HURT_VISITOR",
        Action::MapBorder => "MAP_BORDER",
        Action::MapDeaths => "MAP_DEATHS",
        Action::AccessToggleables => "ACCESS_TOGGLEABLES",
    }
}

/// The action a Java name means. `None` for a name this port does not know, which
/// the reader turns into [`Refusal::Name`].
pub fn action_from_name(name: &str) -> Option<Action> {
    JAVA_ACTIONS.iter().find(|(known, _)| *known == name).map(|(_, action)| *action)
}

/// `PermissionsMessage.View` (`:42-106`): the pane, serialized elsewhere.
///
/// `data` is the buffer `Permissions.serializeViewNetworkData` filled
/// (`:70`), carried as bytes: it is a serialization of a 1 091-line class the port
/// has not ported, and `MANQUANT.md` §1 says so. Client-bound (`:85-88`), so no
/// permission gate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermissionsView {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub data: Vec<u8>,
}

impl Message for PermissionsView {
    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        // `:101-104` — colony, dimension, then the buffer, in that order.
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert(KEY_DATA.into(), bytes(&self.data));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        Ok(PermissionsView { colony_id, dimension, data: super::common::bytes_of(data, KEY_DATA)? })
    }
}

/// `PermissionsMessage.Permission` (`:111-193`): turn one action on or off for one
/// rank.
///
/// Server-bound (`:149-154`), and the gate is the colony's own: Java asks
/// `alterPermission(getRank(sender), …)` (`:166`), which the permission layer
/// checks. [`crate::network::common::Rule::None`] here for that reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Permission {
    pub colony_id: i32,
    pub enable: bool,
    pub rank: Rank,
    pub action: Action,
    pub dimension: Dimension,
}

impl Message for Permission {
    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert(KEY_ENABLE.into(), boolean(self.enable));
        data.insert(KEY_RANK.into(), int(self.rank.id()));
        data.insert(KEY_ACTION.into(), text(action_name(self.action)));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        let name = text_of(data, KEY_ACTION)?;
        Ok(Permission {
            colony_id,
            dimension,
            enable: super::common::bool_of(data, KEY_ENABLE)?,
            rank: rank_of(data, KEY_RANK)?,
            action: action_from_name(&name).ok_or(Refusal::Name { key: KEY_ACTION })?,
        })
    }
}

/// `PermissionsMessage.AddPlayer` (`:198-267`): invite a name to the colony.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddPlayer {
    pub colony_id: i32,
    pub player_name: String,
    pub dimension: Dimension,
}

impl Message for AddPlayer {
    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert(KEY_PLAYER_NAME.into(), text(&self.player_name));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        Ok(AddPlayer { colony_id, dimension, player_name: text_of(data, KEY_PLAYER_NAME)? })
    }
}

/// `PermissionsMessage.AddRank` (`:272-333`): a new rank, named by the player.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddRank {
    pub colony_id: i32,
    pub rank_name: String,
    pub dimension: Dimension,
}

impl Message for AddRank {
    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert(KEY_RANK_NAME.into(), text(&self.rank_name));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        Ok(AddRank { colony_id, dimension, rank_name: text_of(data, KEY_RANK_NAME)? })
    }
}

/// `PermissionsMessage.AddPlayerOrFakePlayer` (`:338-414`): invite a name **and** an
/// id, which is how a fake player — a bot, a corpse's owner — gets a rank
/// (`:405-407`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddPlayerOrFakePlayer {
    pub colony_id: i32,
    pub player_name: String,
    /// The `UUID` as a `u128`, the shape `colony::permissions::uuid` reads and
    /// writes. Java sends `PacketUtils.writeUUID`'s two longs (`:378`).
    pub id: u128,
    pub dimension: Dimension,
}

impl Message for AddPlayerOrFakePlayer {
    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert(KEY_PLAYER_NAME.into(), text(&self.player_name));
        data.insert("id".into(), uuid(self.id));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        Ok(AddPlayerOrFakePlayer {
            colony_id,
            dimension,
            player_name: text_of(data, KEY_PLAYER_NAME)?,
            id: uuid_of(data, "id")?,
        })
    }
}

/// `PermissionsMessage.ChangePlayerRank` (`:419-490`): promote or demote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangePlayerRank {
    pub colony_id: i32,
    pub player_id: u128,
    pub dimension: Dimension,
    pub rank: Rank,
}

impl Message for ChangePlayerRank {
    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert(KEY_PLAYER.into(), uuid(self.player_id));
        data.insert(KEY_RANK.into(), int(self.rank.id()));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        Ok(ChangePlayerRank {
            colony_id,
            dimension,
            player_id: uuid_of(data, KEY_PLAYER)?,
            rank: rank_of(data, KEY_RANK)?,
        })
    }
}

/// `PermissionsMessage.RemovePlayer` (`:495-571`): expel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemovePlayer {
    pub colony_id: i32,
    pub player_id: u128,
    pub dimension: Dimension,
}

impl Message for RemovePlayer {
    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert(KEY_PLAYER.into(), uuid(self.player_id));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        Ok(RemovePlayer { colony_id, dimension, player_id: uuid_of(data, KEY_PLAYER)? })
    }
}

/// `PermissionsMessage.RemoveRank` (`:576-637`): delete a rank.
///
/// The rank travels as its **id**, and Java resolves it against the colony's own
/// table (`:634`) — which is why the port keeps an `i32` here and does not refuse
/// an id it does not know: a colony may hold a rank the five stock ones do not
/// cover, and this message is how such a rank is removed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoveRank {
    pub colony_id: i32,
    pub rank_id: i32,
    pub dimension: Dimension,
}

impl Message for RemoveRank {
    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert(KEY_RANK_ID.into(), int(self.rank_id));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        Ok(RemoveRank { colony_id, dimension, rank_id: int_of(data, KEY_RANK_ID)? })
    }
}

/// What a rank *is*, as the three-way switch of
/// `EditRankType.onExecute` reads it (`:705-719`): `0` makes it a colony
/// manager, `1` makes it hostile, anything else makes it neither. Java sends the
/// raw int; the port names the three branches, because "anything else" is a
/// branch, not a value, and a name cannot fall into the wrong one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three branches of Java's switch, default included
pub enum RankKind {
    /// `case 0:` — `setColonyManager(true)`, `setHostile(false)` (`:707-710`).
    Manager,
    /// `case 1:` — `setHostile(true)`, `setColonyManager(false)` (`:711-714`).
    Hostile,
    /// `default:` — both false (`:715-718`).
    Regular,
}

impl RankKind {
    pub const ALL: [RankKind; 3] = [RankKind::Manager, RankKind::Hostile, RankKind::Regular];

    pub fn name(self) -> &'static str {
        match self {
            RankKind::Manager => "manager",
            RankKind::Hostile => "hostile",
            RankKind::Regular => "regular",
        }
    }

    pub fn parse(name: &str) -> Option<RankKind> {
        RankKind::ALL.into_iter().find(|kind| kind.name() == name)
    }

    /// Whether the rank is a colony manager after this change, which is `case 0:`
    /// only.
    pub fn is_manager(self) -> bool {
        self == RankKind::Manager
    }

    /// Whether the rank is hostile after this change, which is `case 1:` only.
    pub fn is_hostile(self) -> bool {
        self == RankKind::Hostile
    }
}

/// `PermissionsMessage.EditRankType` (`:642-723`): change what a rank *is*.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditRankType {
    pub colony_id: i32,
    pub rank_id: i32,
    pub dimension: Dimension,
    pub rank_type: RankKind,
}

impl Message for EditRankType {
    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert(KEY_RANK_ID.into(), int(self.rank_id));
        data.insert(KEY_RANK_TYPE.into(), text(self.rank_type.name()));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        let name = text_of(data, KEY_RANK_TYPE)?;
        Ok(EditRankType {
            colony_id,
            dimension,
            rank_id: int_of(data, KEY_RANK_ID)?,
            rank_type: RankKind::parse(&name).ok_or(Refusal::Name { key: KEY_RANK_TYPE })?,
        })
    }
}

/// The nine classes of `PermissionsMessage.java`, by Java file and Rust type. The
/// census in `src/network/census.rs` counts these against the tree.
pub const CENSUS: &[(&str, &str)] = &[
    ("PermissionsMessage.java", "PermissionsView"),
    ("PermissionsMessage.java", "Permission"),
    ("PermissionsMessage.java", "AddPlayer"),
    ("PermissionsMessage.java", "AddRank"),
    ("PermissionsMessage.java", "AddPlayerOrFakePlayer"),
    ("PermissionsMessage.java", "ChangePlayerRank"),
    ("PermissionsMessage.java", "RemovePlayer"),
    ("PermissionsMessage.java", "RemoveRank"),
    ("PermissionsMessage.java", "EditRankType"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::common::round_trip;

    fn overworld() -> Dimension {
        Dimension::new("minecraft:overworld")
    }

    const PLAYER: u128 = 0x0123_4567_89ab_cdef_0123_4567_89ab_cdef;

    #[test]
    fn the_pane_round_trips_with_its_opaque_buffer() {
        let view = PermissionsView {
            colony_id: 12,
            dimension: overworld(),
            data: vec![0, 1, 2, 250, 251],
        };
        assert_eq!(round_trip(view.clone()).data, view.data);
    }

    #[test]
    fn a_permission_toggles_one_action_for_one_rank() {
        let edit = Permission {
            colony_id: 12,
            enable: false,
            rank: Rank::Friend,
            action: Action::BreakHuts,
            dimension: overworld(),
        };
        let read = round_trip(edit);
        assert_eq!((read.rank, read.action, read.enable), (Rank::Friend, Action::BreakHuts, false));
    }

    #[test]
    fn the_three_name_messages_round_trip() {
        round_trip(AddPlayer { colony_id: 3, player_name: "Simon".to_owned(), dimension: overworld() });
        round_trip(AddRank { colony_id: 3, rank_name: "Builders".to_owned(), dimension: overworld() });
        round_trip(AddPlayerOrFakePlayer {
            colony_id: 3,
            player_name: "cadaver".to_owned(),
            id: PLAYER,
            dimension: overworld(),
        });
    }

    #[test]
    fn the_three_rank_messages_round_trip() {
        round_trip(ChangePlayerRank {
            colony_id: 4,
            player_id: PLAYER,
            dimension: overworld(),
            rank: Rank::Officer,
        });
        round_trip(RemovePlayer { colony_id: 4, player_id: PLAYER, dimension: overworld() });
        round_trip(RemoveRank { colony_id: 4, rank_id: 7, dimension: overworld() });
        for kind in RankKind::ALL {
            let edit = EditRankType {
                colony_id: 4,
                rank_id: 7,
                dimension: overworld(),
                rank_type: kind,
            };
            assert_eq!(round_trip(edit).rank_type, kind);
        }
    }

    #[test]
    fn an_unknown_action_rank_or_rank_type_is_refused() {
        let mut data = Permission {
            colony_id: 1,
            enable: true,
            rank: Rank::Owner,
            action: Action::AccessHuts,
            dimension: overworld(),
        }
        .to_compound();
        data.insert(KEY_ACTION.into(), text("GUARDS_ATTACK"));
        assert_eq!(Permission::from_compound(&data), Err(Refusal::Name { key: KEY_ACTION }));
        data.insert(KEY_ACTION.into(), text("ACCESS_HUTS"));
        data.insert(KEY_RANK.into(), int(9));
        assert_eq!(Permission::from_compound(&data), Err(Refusal::Name { key: KEY_RANK }));

        let mut data = EditRankType {
            colony_id: 1,
            rank_id: 0,
            dimension: overworld(),
            rank_type: RankKind::Regular,
        }
        .to_compound();
        data.insert(KEY_RANK_TYPE.into(), text("owner"));
        assert_eq!(EditRankType::from_compound(&data), Err(Refusal::Name { key: KEY_RANK_TYPE }));
    }

    #[test]
    fn every_java_action_name_maps_to_the_bit_the_port_declares() {
        assert_eq!(JAVA_ACTIONS.len(), 27);
        for (index, (name, action)) in JAVA_ACTIONS.iter().enumerate() {
            assert_eq!(action_from_name(name), Some(*action), "{name} does not read back");
            assert_eq!(action_name(*action), *name);
            for (other, _) in &JAVA_ACTIONS[index + 1..] {
                assert_ne!(name, other, "two rows share the Java name {name}");
            }
        }
        // The bits Java assigns, spot-checked where the port's spelling differs
        // from the Java name.
        assert_eq!(action_from_name("RIGHTCLICK_BLOCK"), Some(Action::RightClickBlock));
        assert_eq!(Action::RightClickBlock.bit(), 14);
        assert_eq!(Action::UseScanTool.bit(), 7);
        assert_eq!(Action::TeleportToColony.bit(), 21);
        assert_eq!(Action::AccessToggleables.bit(), 30);
        assert_eq!(action_from_name("MINING"), None);
    }

    #[test]
    fn each_rank_kind_answers_the_two_java_setters_of_its_branch() {
        assert!(RankKind::Manager.is_manager() && !RankKind::Manager.is_hostile());
        assert!(!RankKind::Hostile.is_manager() && RankKind::Hostile.is_hostile());
        assert!(!RankKind::Regular.is_manager() && !RankKind::Regular.is_hostile());
        for kind in RankKind::ALL {
            assert_eq!(RankKind::parse(kind.name()), Some(kind));
        }
    }

    #[test]
    fn none_of_the_nine_is_gated_here_because_java_gates_them_in_on_execute() {
        assert_eq!(PermissionsView::RULE, super::super::common::Rule::None);
        assert_eq!(Permission::RULE, super::super::common::Rule::None);
        assert_eq!(EditRankType::RULE, super::super::common::Rule::None);
        assert_eq!(CENSUS.len(), 9);
    }
}
