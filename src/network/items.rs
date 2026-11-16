//! The six messages an item sends for itself: a clipboard setting, "I have read the
//! story", swapping a block for the build tool, a rally banner toggled or untied, and
//! a scanned blueprint on its way to the client.
//!
//! Java reference: `core/network/messages/server/ItemSettingMessage.java` (62),
//! `MarkStoryReadOnItemMessage.java` (75), `SwitchBuildingWithToolMessage.java` (105),
//! `ToggleBannerRallyGuardsMessage.java` (83), `RemoveFromRallyingListMessage.java`
//! (94), `client/SaveStructureNBTMessage.java` (115).
//!
//! **Five are the player's own item, so none carries a colony.** The item is found in
//! the sender's hand or inventory (`MarkStoryReadOnItemMessage.java:68`,
//! `ToggleBannerRallyGuardsMessage.java:71`, `SwitchBuildingWithToolMessage.java:76-97`)
//! and a message that named a colony would be lying about who it acts on. The sixth,
//! `SaveStructureNBTMessage`, is server-to-client: the scan is finished and the client
//! is told to write it to disk (`:106-111`).
//!
//! **`RemoveFromRallyingListMessage` carries an `ILocation`.** Java serializes it with
//! `StandardFactoryController` into an NBT compound (`:68`) and reads it back the same
//! way (`:61`); the location classes are `api/colony/requestsystem/location` — a
//! hierarchy this port has not ported, so the compound travels as it is
//! (`MANQUANT.md` §1). It is *not* a position: a location may be an entity.
//!
//! **`SaveStructureNBTMessage` carries a `CompoundTag` and a file name** — and Java
//! puts both inside another compound, gzips it, and writes it into the buffer
//! (`:78-91`). The port keeps the two fields as they are: the wrapper and the gzip are
//! an envelope around a payload the port already has as NBT, and re-creating them here
//! would add nothing a caller can use. `RAPPORT.md` records the deviation.
//!
//! **`ItemSettingMessage` has no side.** It implements `IMessage` and never overrides
//! `getExecutionSide` (`:13-62`), unlike every other message in the tree; Java's
//! dispatcher therefore treats it as "either side". The port keeps the message and
//! notes the gap rather than inventing a side for it.

use super::common::{
    int, int_of, nested, nested_of, stack, stack_of, text, text_of, ItemStack, Message, Refusal,
};
use mod_api::Compound;

/// `net.minecraft.world.InteractionHand`, in the order Java declares it: main hand,
/// then off hand (`InteractionHand.values()[buf.readInt()]`,
/// `MarkStoryReadOnItemMessage.java:48`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two values of vanilla's InteractionHand
pub enum Hand {
    Main,
    Off,
}

impl Hand {
    pub const ALL: [Hand; 2] = [Hand::Main, Hand::Off];

    pub fn name(self) -> &'static str {
        match self {
            Hand::Main => "MAIN_HAND",
            Hand::Off => "OFF_HAND",
        }
    }

    pub fn parse(name: &str) -> Option<Hand> {
        Hand::ALL.into_iter().find(|hand| hand.name() == name)
    }
}

/// `ItemSettingMessage.java:13-62` — the clipboard stores one integer under one
/// name (`:52-54`).
///
/// The name is the setting's own key in the stack's NBT (`:53`), written by the item
/// itself, so the port cannot check it against a list it does not have.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemSetting {
    pub setting_name: String,
    pub setting_value: i32,
}

impl Message for ItemSetting {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("settingName".into(), text(&self.setting_name)),
            ("settingValue".into(), int(self.setting_value)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(ItemSetting {
            setting_name: text_of(data, "settingName")?,
            setting_value: int_of(data, "settingValue")?,
        })
    }
}

/// `MarkStoryReadOnItemMessage.java:19-74` — "this supply item's story has been read"
/// (`:69-72`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkStoryReadOnItem {
    pub hand: Hand,
}

impl Message for MarkStoryReadOnItem {
    fn to_compound(&self) -> Compound {
        Compound::from([("hand".into(), text(self.hand.name()))])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let hand = text_of(data, "hand")?;
        Ok(MarkStoryReadOnItem { hand: Hand::parse(&hand).ok_or(Refusal::Name { key: "hand" })? })
    }
}

/// `SwitchBuildingWithToolMessage.java:16-105` — swap the clicked block for the build
/// tool in the hotbar (`:76-102`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwitchBuildingWithTool {
    pub stack: ItemStack,
}

impl Message for SwitchBuildingWithTool {
    fn to_compound(&self) -> Compound {
        Compound::from([("stack".into(), stack(&self.stack))])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(SwitchBuildingWithTool { stack: stack_of(data, "stack")? })
    }
}

/// `ToggleBannerRallyGuardsMessage.java:22-82` — arm or disarm the rally banner in the
/// sender's inventory (`:71-80`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToggleBannerRallyGuards {
    pub banner: ItemStack,
}

impl Message for ToggleBannerRallyGuards {
    fn to_compound(&self) -> Compound {
        Compound::from([("banner".into(), stack(&self.banner))])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(ToggleBannerRallyGuards { banner: stack_of(data, "banner")? })
    }
}

/// `RemoveFromRallyingListMessage.java:24-93` — untie one guard tower from the banner
/// (`:91`).
///
/// `location` is an `ILocation`, carried as the compound the factory wrote
/// (`:68`): a location may be a position, a building or a live entity
/// (`api/colony/requestsystem/location/`), and the port has none of the three as a
/// message type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoveFromRallyingList {
    pub banner: ItemStack,
    pub location: Compound,
}

impl Message for RemoveFromRallyingList {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("banner".into(), stack(&self.banner)),
            ("location".into(), nested(&self.location)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(RemoveFromRallyingList {
            banner: stack_of(data, "banner")?,
            location: nested_of(data, "location")?,
        })
    }
}

/// `SaveStructureNBTMessage.java:27-114` — the scan is ready: write this NBT as that
/// file name (`:105-111`).
///
/// The schematic is a `CompoundTag` (`:32`) — the structure's own NBT, which is
/// exactly what this port speaks, so it is not opaque.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveStructureNbt {
    pub file_name: String,
    pub schematic: Compound,
}

impl Message for SaveStructureNbt {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("fileName".into(), text(&self.file_name)),
            ("schematic".into(), nested(&self.schematic)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(SaveStructureNbt {
            file_name: text_of(data, "fileName")?,
            schematic: nested_of(data, "schematic")?,
        })
    }
}

/// The six messages of this family, by Java file and Rust type.
pub const CENSUS: &[(&str, &str)] = &[
    ("server/ItemSettingMessage.java", "ItemSetting"),
    ("server/MarkStoryReadOnItemMessage.java", "MarkStoryReadOnItem"),
    ("server/SwitchBuildingWithToolMessage.java", "SwitchBuildingWithTool"),
    ("server/ToggleBannerRallyGuardsMessage.java", "ToggleBannerRallyGuards"),
    ("server/RemoveFromRallyingListMessage.java", "RemoveFromRallyingList"),
    ("client/SaveStructureNBTMessage.java", "SaveStructureNbt"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::common::round_trip;

    fn banner() -> ItemStack {
        ItemStack::new("minecolonies:banner_rally_guards", 1).expect("a valid stack")
    }

    #[test]
    fn a_clipboard_setting_round_trips_its_name_and_value() {
        let setting = ItemSetting { setting_name: "clipboard_mode".to_owned(), setting_value: 2 };
        assert_eq!(round_trip(setting).setting_value, 2);
        let mut data =
            ItemSetting { setting_name: "clipboard_mode".to_owned(), setting_value: 0 }.to_compound();
        data.insert("settingValue".into(), text("two"));
        assert_eq!(ItemSetting::from_compound(&data), Err(Refusal::Kind { key: "settingValue" }));
    }

    #[test]
    fn a_story_mark_round_trips_both_hands() {
        for hand in Hand::ALL {
            assert_eq!(round_trip(MarkStoryReadOnItem { hand }).hand, hand);
            assert_eq!(Hand::parse(hand.name()), Some(hand));
        }
        let data = Compound::from([("hand".into(), text("THIRD_HAND"))]);
        assert_eq!(MarkStoryReadOnItem::from_compound(&data), Err(Refusal::Name { key: "hand" }));
    }

    #[test]
    fn the_two_stack_messages_round_trip() {
        let tool = SwitchBuildingWithTool {
            stack: ItemStack::new("structurize:buildtool", 1).expect("a valid stack"),
        };
        assert_eq!(round_trip(tool).stack.item.as_str(), "structurize:buildtool");
        assert_eq!(round_trip(ToggleBannerRallyGuards { banner: banner() }).banner.count, 1);
    }

    #[test]
    fn an_untying_round_trips_the_location_compound_it_carries() {
        let location = Compound::from([
            ("type".into(), text("minecolonies:static")),
            ("dim".into(), text("minecraft:overworld")),
        ]);
        let untie = RemoveFromRallyingList { banner: banner(), location: location.clone() };
        assert_eq!(round_trip(untie).location, location);
        let data = Compound::from([("banner".into(), stack(&banner()))]);
        assert_eq!(
            RemoveFromRallyingList::from_compound(&data),
            Err(Refusal::Missing { key: "location" })
        );
    }

    #[test]
    fn a_saved_schematic_round_trips_its_nbt_and_its_name() {
        let schematic = Compound::from([
            ("version".into(), int(2)),
            ("size".into(), super::super::common::position([5, 4, 5])),
        ]);
        let saved = SaveStructureNbt {
            file_name: "maison1.blueprint".to_owned(),
            schematic: schematic.clone(),
        };
        let read = round_trip(saved);
        assert_eq!((read.file_name.as_str(), read.schematic), ("maison1.blueprint", schematic));
        assert_eq!(CENSUS.len(), 6);
    }
}
