//! The seven "open this screen" messages: the two decoration windows, the plantation
//! field's, the suggestion tool's, a citizen's or a hut's inventory, and the two that
//! only trigger an advancement.
//!
//! Java reference: `core/network/messages/client/OpenBuildWindowMessage.java` (98, the
//! abstract base), `OpenDecoBuildWindowMessage.java` (43),
//! `OpenPlantationFieldBuildWindowMessage.java` (44),
//! `OpenSuggestionWindowMessage.java` (88), `server/colony/OpenInventoryMessage.java`
//! (183), `server/OpenGuiWindowTriggerMessage.java` (62),
//! `server/ClickGuiButtonTriggerMessage.java` (70).
//!
//! **"Same bytes, different screen" is kept as two types.** `OpenDecoBuildWindow` and
//! `OpenPlantationFieldBuildWindow` are subclasses of the abstract
//! `OpenBuildWindowMessage` that add **no field at all** (`:16-20` of each, against the
//! base's `:21-41`); what they change is which window opens and which work-order
//! message its click creates (`:38-42`, `:39-43`). The port keeps the base as a
//! struct and the two as newtypes over it, so the two screens stay two names and the
//! wire stays one.
//!
//! **Two of the seven open nothing.** `ClickGuiButtonTriggerMessage` and
//! `OpenGuiWindowTriggerMessage` fire an advancement trigger and return
//! (`ClickGuiButtonTriggerMessage.java:61-68`, `OpenGuiWindowTriggerMessage.java:52-60`);
//! they are server-bound, which is the opposite of everything else in this family —
//! the client is *telling* the server that a button was pressed, so the advancement
//! can be awarded. The port keeps them here because their subject is a screen, and
//! says in each type's note which side runs it.
//!
//! **`OpenInventoryMessage` packs two shapes into one message.** The enum
//! `InventoryType` decides whether the payload continues with an entity id or with a
//! block position (`:89-99`), and the reader must branch on it. The port keeps one
//! struct with both fields optional **and refuses a message that carries the wrong
//! one** — Java's `switch` has no `default`, so a third value would silently do
//! nothing (`:127-137`).

use super::common::{
    bool_of, boolean, envelope, envelope_of, int, int_of, pos_of, position, stack, stack_of, text,
    text_of, ItemStack, Message, Refusal, Rule,
};
use crate::base::Dimension;
use mod_api::Compound;

/// Vanilla's four rotations, in the order `Rotation` declares them.
///
/// Java writes `rotation.ordinal()` (`OpenBuildWindowMessage.java:71`, `:81`) and
/// reads it back by index; the port writes the name, as `common`'s note argues.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four values of vanilla's Rotation
pub enum Rotation {
    None,
    Clockwise90,
    Clockwise180,
    CounterClockwise90,
}

impl Rotation {
    pub const ALL: [Rotation; 4] =
        [Rotation::None, Rotation::Clockwise90, Rotation::Clockwise180, Rotation::CounterClockwise90];

    pub fn name(self) -> &'static str {
        match self {
            Rotation::None => "NONE",
            Rotation::Clockwise90 => "CLOCKWISE_90",
            Rotation::Clockwise180 => "CLOCKWISE_180",
            Rotation::CounterClockwise90 => "COUNTERCLOCKWISE_90",
        }
    }

    pub fn parse(name: &str) -> Option<Rotation> {
        Rotation::ALL.into_iter().find(|rotation| rotation.name() == name)
    }
}

/// `OpenBuildWindowMessage.java:16-97` — a blueprint preview, with where it goes and
/// how it is turned.
///
/// `mirror` is a **boolean** on the wire (`:70`, `:80`): the base collapses
/// `Mirror.NONE` and everything else into it (`:61`), so the port keeps the bool and
/// notes that Java throws the mirror axis away — a north-south mirror and an
/// east-west one are the same message in Java, and in the port.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenBuildWindow {
    pub pos: [i32; 3],
    /// The blueprint's path inside its pack (`:26`).
    pub path: String,
    pub pack_name: String,
    pub rotation: Rotation,
    pub mirror: bool,
}

impl Message for OpenBuildWindow {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("pos".into(), position(self.pos)),
            ("path".into(), text(&self.path)),
            ("packName".into(), text(&self.pack_name)),
            ("mirror".into(), boolean(self.mirror)),
            ("rotation".into(), text(self.rotation.name())),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let rotation = text_of(data, "rotation")?;
        Ok(OpenBuildWindow {
            pos: pos_of(data, "pos")?,
            path: text_of(data, "path")?,
            pack_name: text_of(data, "packName")?,
            rotation: Rotation::parse(&rotation).ok_or(Refusal::Name { key: "rotation" })?,
            mirror: bool_of(data, "mirror")?,
        })
    }
}

/// `OpenDecoBuildWindowMessage.java:14-43` — the decoration preview, whose click
/// creates a `DecorationBuildRequestMessage` (`:38-42`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenDecoBuildWindow(pub OpenBuildWindow);

impl Message for OpenDecoBuildWindow {
    fn to_compound(&self) -> Compound {
        self.0.to_compound()
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(OpenDecoBuildWindow(OpenBuildWindow::from_compound(data)?))
    }
}

/// `OpenPlantationFieldBuildWindowMessage.java:14-44` — the plantation field's
/// preview, whose click creates a `PlantationFieldBuildRequestMessage` (`:39-43`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenPlantationFieldBuildWindow {
    pub window: OpenBuildWindow,
}

impl Message for OpenPlantationFieldBuildWindow {
    fn to_compound(&self) -> Compound {
        self.window.to_compound()
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(OpenPlantationFieldBuildWindow { window: OpenBuildWindow::from_compound(data)? })
    }
}

/// `OpenSuggestionWindowMessage.java:19-87` — the build tool suggests what to put on
/// this block (`:85`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenSuggestionWindow {
    /// `Block.getId(state)` (`:70`): the suggested block's state id
    /// (`MANQUANT.md` §3).
    pub state: i32,
    pub pos: [i32; 3],
    pub stack: ItemStack,
}

impl Message for OpenSuggestionWindow {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("state".into(), int(self.state)),
            ("pos".into(), position(self.pos)),
            ("stack".into(), stack(&self.stack)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(OpenSuggestionWindow {
            state: int_of(data, "state")?,
            pos: pos_of(data, "pos")?,
            stack: stack_of(data, "stack")?,
        })
    }
}

/// What `OpenInventoryMessage` is opening
/// (`OpenInventoryMessage.java:177-181`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two-value enum of Java's InventoryType
pub enum InventoryType {
    /// `INVENTORY_CITIZEN` (`:179`) — a citizen's backpack.
    Citizen,
    /// `INVENTORY_CHEST` (`:180`) — a rack or a grave.
    Chest,
}

impl InventoryType {
    pub const ALL: [InventoryType; 2] = [InventoryType::Citizen, InventoryType::Chest];

    pub fn name(self) -> &'static str {
        match self {
            InventoryType::Citizen => "INVENTORY_CITIZEN",
            InventoryType::Chest => "INVENTORY_CHEST",
        }
    }

    pub fn parse(name: &str) -> Option<InventoryType> {
        InventoryType::ALL.into_iter().find(|kind| kind.name() == name)
    }
}

/// `OpenInventoryMessage.java:27-183` — open a citizen's inventory or a hut's rack
/// (`:140-172`).
///
/// **One message, two payloads.** Which of [`Self::entity_id`] and [`Self::chest_pos`]
/// is present is decided by [`Self::inventory_type`] (`:91-115`), and a message whose
/// fields disagree with its type is refused here — Java's `switch` has no `default`,
/// so it would open nothing in silence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenInventory {
    pub colony_id: i32,
    pub dimension: Dimension,
    /// The name given to a citizen's inventory (`:90`), empty for a chest
    /// (`:81`).
    pub name: String,
    pub inventory_type: InventoryType,
    /// Set for [`InventoryType::Citizen`] only.
    pub entity_id: Option<i32>,
    /// Set for [`InventoryType::Chest`] only.
    pub chest_pos: Option<[i32; 3]>,
}

impl Message for OpenInventory {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert("inventoryType".into(), text(self.inventory_type.name()));
        data.insert("name".into(), text(&self.name));
        if let Some(entity) = self.entity_id {
            data.insert("entityID".into(), int(entity));
        }
        if let Some(pos) = self.chest_pos {
            data.insert("tePos".into(), position(pos));
        }
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        let kind = text_of(data, "inventoryType")?;
        let inventory_type = InventoryType::parse(&kind).ok_or(Refusal::Name { key: "inventoryType" })?;
        let entity_id = match data.get(&"entityID".into()) {
            Some(_) => Some(int_of(data, "entityID")?),
            None => None,
        };
        let chest_pos = match data.get(&"tePos".into()) {
            Some(_) => Some(pos_of(data, "tePos")?),
            None => None,
        };
        match (inventory_type, entity_id.is_some(), chest_pos.is_some()) {
            (InventoryType::Citizen, true, false) | (InventoryType::Chest, false, true) => {}
            (InventoryType::Citizen, _, _) => return Err(Refusal::Missing { key: "entityID" }),
            (InventoryType::Chest, _, _) => return Err(Refusal::Missing { key: "tePos" }),
        }
        Ok(OpenInventory {
            colony_id,
            dimension,
            name: text_of(data, "name")?,
            inventory_type,
            entity_id,
            chest_pos,
        })
    }
}

/// `OpenGuiWindowTriggerMessage.java:12-61` — the client opened the window whose
/// resource id this is, and the advancement trigger fires (`:58`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenGuiWindowTrigger {
    /// The window's resource location (`:17`, `:36`).
    pub resource: String,
}

impl Message for OpenGuiWindowTrigger {
    fn to_compound(&self) -> Compound {
        Compound::from([("resource".into(), text(&self.resource))])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(OpenGuiWindowTrigger { resource: named(data, "resource")? })
    }
}

/// `ClickGuiButtonTriggerMessage.java:12-69` — the client clicked the button whose id
/// this is (`:66`), and the advancement trigger fires with the button *and* the
/// screen it was on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClickGuiButtonTrigger {
    /// The button's own id (`:17`), a json path.
    pub button_id: String,
    /// The screen the button is on (`:22`).
    pub resource: String,
}

impl Message for ClickGuiButtonTrigger {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("resource".into(), text(&self.resource)),
            ("buttonId".into(), text(&self.button_id)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(ClickGuiButtonTrigger {
            button_id: named(data, "buttonId")?,
            resource: named(data, "resource")?,
        })
    }
}

fn named(data: &Compound, key: &'static str) -> Result<String, Refusal> {
    let value = text_of(data, key)?;
    if value.is_empty() {
        return Err(Refusal::Name { key });
    }
    Ok(value)
}

/// The seven messages of this family, by Java file and Rust type.
pub const CENSUS: &[(&str, &str)] = &[
    ("client/OpenBuildWindowMessage.java", "OpenBuildWindow"),
    ("client/OpenDecoBuildWindowMessage.java", "OpenDecoBuildWindow"),
    ("client/OpenPlantationFieldBuildWindowMessage.java", "OpenPlantationFieldBuildWindow"),
    ("client/OpenSuggestionWindowMessage.java", "OpenSuggestionWindow"),
    ("server/colony/OpenInventoryMessage.java", "OpenInventory"),
    ("server/OpenGuiWindowTriggerMessage.java", "OpenGuiWindowTrigger"),
    ("server/ClickGuiButtonTriggerMessage.java", "ClickGuiButtonTrigger"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::common::round_trip;

    fn overworld() -> Dimension {
        Dimension::new("minecraft:overworld")
    }

    fn window() -> OpenBuildWindow {
        OpenBuildWindow {
            pos: [3, 64, 3],
            path: "decoration/bell_tower1.blueprint".to_owned(),
            pack_name: "minecolonies".to_owned(),
            rotation: Rotation::Clockwise180,
            mirror: true,
        }
    }

    #[test]
    fn a_build_window_round_trips_every_rotation() {
        for rotation in Rotation::ALL {
            let turned = OpenBuildWindow { rotation, ..window() };
            assert_eq!(round_trip(turned).rotation, rotation);
        }
        let mut data = window().to_compound();
        data.insert("rotation".into(), text("SIDEWAYS"));
        assert_eq!(OpenBuildWindow::from_compound(&data), Err(Refusal::Name { key: "rotation" }));
    }

    #[test]
    fn the_two_subclasses_write_the_same_bytes_as_their_base() {
        let base = window();
        assert_eq!(OpenDecoBuildWindow(base.clone()).to_compound(), base.to_compound());
        let plantation = OpenPlantationFieldBuildWindow { window: base.clone() };
        assert_eq!(plantation.to_compound(), base.to_compound());
        assert_eq!(round_trip(OpenDecoBuildWindow(base.clone())).0, base);
        assert_eq!(round_trip(plantation).window, base);
    }

    #[test]
    fn a_suggestion_round_trips_the_block_it_suggests() {
        let suggestion = OpenSuggestionWindow {
            state: 77,
            pos: [8, 65, 8],
            stack: ItemStack::new("minecraft:oak_fence", 1).expect("a valid stack"),
        };
        assert_eq!(round_trip(suggestion).state, 77);
    }

    #[test]
    fn an_inventory_round_trips_both_of_its_shapes() {
        let citizen = OpenInventory {
            colony_id: 2,
            dimension: overworld(),
            name: "Sac de Simon".to_owned(),
            inventory_type: InventoryType::Citizen,
            entity_id: Some(31),
            chest_pos: None,
        };
        assert_eq!(round_trip(citizen).entity_id, Some(31));

        let chest = OpenInventory {
            colony_id: 2,
            dimension: overworld(),
            name: String::new(),
            inventory_type: InventoryType::Chest,
            entity_id: None,
            chest_pos: Some([4, 64, 4]),
        };
        assert_eq!(round_trip(chest).chest_pos, Some([4, 64, 4]));
    }

    #[test]
    fn an_inventory_whose_fields_disagree_with_its_type_is_refused() {
        let mut data = OpenInventory {
            colony_id: 1,
            dimension: overworld(),
            name: String::new(),
            inventory_type: InventoryType::Chest,
            entity_id: None,
            chest_pos: Some([0, 0, 0]),
        }
        .to_compound();
        data.insert("inventoryType".into(), text("INVENTORY_CITIZEN"));
        assert_eq!(OpenInventory::from_compound(&data), Err(Refusal::Missing { key: "entityID" }));
        data.insert("inventoryType".into(), text("INVENTORY_BACKPACK"));
        assert_eq!(
            OpenInventory::from_compound(&data),
            Err(Refusal::Name { key: "inventoryType" })
        );
    }

    #[test]
    fn the_two_trigger_messages_round_trip_their_resource_names() {
        let opened = OpenGuiWindowTrigger { resource: "minecolonies:window/townhall".to_owned() };
        assert_eq!(round_trip(opened).resource, "minecolonies:window/townhall");
        let clicked = ClickGuiButtonTrigger {
            button_id: "minecolonies:window/townhall/button/rename".to_owned(),
            resource: "minecolonies:window/townhall".to_owned(),
        };
        assert_eq!(
            round_trip(clicked).button_id,
            "minecolonies:window/townhall/button/rename"
        );
        let mut data = Compound::from([("resource".into(), text(""))]);
        assert_eq!(
            OpenGuiWindowTrigger::from_compound(&data),
            Err(Refusal::Name { key: "resource" })
        );
        data.insert("resource".into(), text("minecolonies:window/townhall"));
        assert!(OpenGuiWindowTrigger::from_compound(&data).is_ok());
        assert_eq!(CENSUS.len(), 7);
    }
}
