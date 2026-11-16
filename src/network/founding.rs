//! Founding: the six messages that create, inspect, reactivate, pick up, place and
//! abandon a colony, and the four windows the town hall opens to say why.
//!
//! Java reference: `core/network/messages/server/CreateColonyMessage.java` (202
//! lines), `GetColonyInfoMessage.java` (128), `PickUpTownhallMessage.java` (92),
//! `ReactivateBuildingMessage.java` (109), `DirectPlaceMessage.java` (166),
//! `server/colony/ColonyAbandonOwnMessage.java` (60),
//! `server/colony/ColonyDeleteOwnMessage.java` (60), and the four client windows
//! `OpenCantFoundColonyWarningMessage.java` (68),
//! `OpenColonyFoundingCovenantMessage.java` (58), `OpenReactivateColonyMessage.java`
//! (58), `OpenDeleteAbandonColonyMessage.java` (74).
//!
//! **`CreateColonyMessage` is the whole founding covenant on the wire** — where the
//! town hall is, whether the land is claimed, the colony's name, and the structure
//! pack and blueprint path the hut will build (`:78-84`). Everything else the
//! message does is a rule: the distance from spawn (`:147-163`), the distance from
//! other colonies (`:165-169`), the reactivation of a deactivated hut (`:134-142`).
//! Those rules already live in `founding::rules` and `base::territory`; this type
//! carries what they are applied to.
//!
//! **Two of the four windows carry the same three fields.** `OpenReactivateColony`
//! and `OpenColonyFoundingCovenant` are byte-for-byte the same message with two
//! different screens behind them (`:43-49` of each), so the port keeps two types:
//! they are two different questions for the player, and collapsing them would make
//! one of the two screens unnameable.
//!
//! **`DirectPlaceMessage` carries a `BlockState` as an int.** Java writes
//! `Block.getId(state)` (`:99`) and reads `Block.stateById` (`:86`), which is a
//! global registry table this port has no address for; the field is the number, and
//! `MANQUANT.md` §3 says what resolving it would take.

use super::common::{
    boolean, component, component_of, int, int_of, pos_of, position, stack, stack_of, text, text_of,
    Component, ItemStack, Message, Refusal,
};
use mod_api::Compound;

/// `CreateColonyMessage.java:36-201` — found a colony, or reactivate one.
///
/// Gated by nothing at the message level: the guards are the range check
/// (`:110-113`), the spawn distance (`:147`) and "one colony per player"
/// (`:171-201`), all inside `onExecute`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateColony {
    pub town_hall: [i32; 3],
    /// Whether the surrounding land is claimed at founding (`:46`, `:80`).
    pub claim: bool,
    pub colony_name: String,
    pub pack_name: String,
    pub path_name: String,
}

impl Message for CreateColony {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("townHall".into(), position(self.town_hall)),
            ("claim".into(), boolean(self.claim)),
            ("colonyName".into(), text(&self.colony_name)),
            ("packName".into(), text(&self.pack_name)),
            ("pathName".into(), text(&self.path_name)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(CreateColony {
            town_hall: pos_of(data, "townHall")?,
            claim: super::common::bool_of(data, "claim")?,
            colony_name: text_of(data, "colonyName")?,
            pack_name: text_of(data, "packName")?,
            path_name: text_of(data, "pathName")?,
        })
    }
}

/// `GetColonyInfoMessage.java:33-125` — the town hall was right-clicked and nobody
/// owns it yet: which of the four windows opens depends on what is around
/// (`:86-124`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GetColonyInfo {
    pub pos: [i32; 3],
}

impl Message for GetColonyInfo {
    fn to_compound(&self) -> Compound {
        Compound::from([("pos".into(), position(self.pos))])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(GetColonyInfo { pos: pos_of(data, "pos")? })
    }
}

/// `PickUpTownhallMessage.java:24-91` — take the town hall block back, when it
/// belongs to no colony (`:71-74`) and the inventory has room (`:82-89`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickUpTownhall {
    pub pos: [i32; 3],
}

impl Message for PickUpTownhall {
    fn to_compound(&self) -> Compound {
        Compound::from([("pos".into(), position(self.pos))])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(PickUpTownhall { pos: pos_of(data, "pos")? })
    }
}

/// `ReactivateBuildingMessage.java:23-108` — a deactivated hut asks to come back.
///
/// The gate is `MANAGE_HUTS` and it is checked in `onExecute` (`:90`), after the
/// range check (`:84`), because the colony is resolved from the position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReactivateBuilding {
    pub pos: [i32; 3],
}

impl Message for ReactivateBuilding {
    fn to_compound(&self) -> Compound {
        Compound::from([("pos".into(), position(self.pos))])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(ReactivateBuilding { pos: pos_of(data, "pos")? })
    }
}

/// `DirectPlaceMessage.java:38-165` — place a hut block straight into the world,
/// when the player holds it and the place is free (`:114-121`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectPlace {
    /// `Block.getId(state)` (`:99`) — the block state's registry id, which the port
    /// carries as it is (`MANQUANT.md` §3).
    pub state: i32,
    pub pos: [i32; 3],
    pub stack: ItemStack,
}

impl Message for DirectPlace {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("state".into(), int(self.state)),
            ("pos".into(), position(self.pos)),
            ("stack".into(), stack(&self.stack)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(DirectPlace {
            state: int_of(data, "state")?,
            pos: pos_of(data, "pos")?,
            stack: stack_of(data, "stack")?,
        })
    }
}

/// `ColonyAbandonOwnMessage.java:18-59` — the owner gives the colony up.
///
/// **No fields at all** (`:20-30` is empty): the colony is whoever's the sender
/// owns (`:48`). A message with no payload is still a message — it is the *click*
/// that carries meaning — so the port keeps a type rather than a comment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColonyAbandonOwn;

impl Message for ColonyAbandonOwn {
    fn to_compound(&self) -> Compound {
        Compound::new()
    }

    fn from_compound(_data: &Compound) -> Result<Self, Refusal> {
        Ok(ColonyAbandonOwn)
    }
}

/// `ColonyDeleteOwnMessage.java:19-59` — the owner deletes it outright (`:52-53`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColonyDeleteOwn;

impl Message for ColonyDeleteOwn {
    fn to_compound(&self) -> Compound {
        Compound::new()
    }

    fn from_compound(_data: &Compound) -> Result<Self, Refusal> {
        Ok(ColonyDeleteOwn)
    }
}

/// `OpenCantFoundColonyWarningMessage.java:14-67` — "you cannot found here", with
/// the screen's own reason and whether to offer the config tooltip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenCantFoundColonyWarning {
    pub warning: Component,
    pub town_hall_pos: [i32; 3],
    pub display_config_tooltip: bool,
}

impl Message for OpenCantFoundColonyWarning {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("warningMessageTranslationKey".into(), component(&self.warning)),
            ("townHallPos".into(), position(self.town_hall_pos)),
            ("displayConfigTooltip".into(), boolean(self.display_config_tooltip)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(OpenCantFoundColonyWarning {
            warning: component_of(data, "warningMessageTranslationKey")?,
            town_hall_pos: pos_of(data, "townHallPos")?,
            display_config_tooltip: super::common::bool_of(data, "displayConfigTooltip")?,
        })
    }
}

/// `OpenColonyFoundingCovenantMessage.java:12-57` — the covenant screen, with the
/// nearest colony's name and how far away it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenColonyFoundingCovenant {
    pub closest_name: String,
    /// `Integer.MAX_VALUE` when there is no colony to name (`:110`).
    pub closest_distance: i32,
    pub town_hall_pos: [i32; 3],
}

impl Message for OpenColonyFoundingCovenant {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("closestName".into(), text(&self.closest_name)),
            ("closestDistance".into(), int(self.closest_distance)),
            ("townHallPos".into(), position(self.town_hall_pos)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(OpenColonyFoundingCovenant {
            closest_name: text_of(data, "closestName")?,
            closest_distance: int_of(data, "closestDistance")?,
            town_hall_pos: pos_of(data, "townHallPos")?,
        })
    }
}

/// `OpenReactivateColonyMessage.java:12-57` — the reactivation screen. The same
/// three fields as the covenant above, and a different question (`:40`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenReactivateColony {
    pub closest_name: String,
    pub closest_distance: i32,
    pub town_hall_pos: [i32; 3],
}

impl Message for OpenReactivateColony {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("closestName".into(), text(&self.closest_name)),
            ("closestDistance".into(), int(self.closest_distance)),
            ("townHallPos".into(), position(self.town_hall_pos)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(OpenReactivateColony {
            closest_name: text_of(data, "closestName")?,
            closest_distance: int_of(data, "closestDistance")?,
            town_hall_pos: pos_of(data, "townHallPos")?,
        })
    }
}

/// `OpenDeleteAbandonColonyMessage.java:12-73` — the abandoned colony in the way,
/// and the town hall being placed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenDeleteAbandonColony {
    pub current_town_hall_pos: [i32; 3],
    pub old_colony_name: String,
    pub old_colony_pos: [i32; 3],
    /// Sent although the window never reads it (`:54` passes only the position):
    /// the port carries it because the wire does, and says so rather than dropping
    /// a field Java sends.
    pub old_colony_id: i32,
}

impl Message for OpenDeleteAbandonColony {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("currentTownHallPos".into(), position(self.current_town_hall_pos)),
            ("oldColonyName".into(), text(&self.old_colony_name)),
            ("oldColonyPos".into(), position(self.old_colony_pos)),
            ("oldColonyId".into(), int(self.old_colony_id)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(OpenDeleteAbandonColony {
            current_town_hall_pos: pos_of(data, "currentTownHallPos")?,
            old_colony_name: text_of(data, "oldColonyName")?,
            old_colony_pos: pos_of(data, "oldColonyPos")?,
            old_colony_id: int_of(data, "oldColonyId")?,
        })
    }
}

/// The eleven messages of this family, by Java file and Rust type.
pub const CENSUS: &[(&str, &str)] = &[
    ("server/CreateColonyMessage.java", "CreateColony"),
    ("server/GetColonyInfoMessage.java", "GetColonyInfo"),
    ("server/PickUpTownhallMessage.java", "PickUpTownhall"),
    ("server/ReactivateBuildingMessage.java", "ReactivateBuilding"),
    ("server/DirectPlaceMessage.java", "DirectPlace"),
    ("server/colony/ColonyAbandonOwnMessage.java", "ColonyAbandonOwn"),
    ("server/colony/ColonyDeleteOwnMessage.java", "ColonyDeleteOwn"),
    ("client/OpenCantFoundColonyWarningMessage.java", "OpenCantFoundColonyWarning"),
    ("client/OpenColonyFoundingCovenantMessage.java", "OpenColonyFoundingCovenant"),
    ("client/OpenReactivateColonyMessage.java", "OpenReactivateColony"),
    ("client/OpenDeleteAbandonColonyMessage.java", "OpenDeleteAbandonColony"),
];

// The keys this family writes are the Java field names, camelCased: `townHall`,
// `colonyName`, `packName`, `pathName`, `pos`, `state`, `stack`,
// `warningMessageTranslationKey`, `townHallPos`, `displayConfigTooltip`,
// `closestName`, `closestDistance`, `currentTownHallPos`, `oldColonyName`,
// `oldColonyPos`, `oldColonyId`.
//
// **No founding message carries a colony envelope.** There is no colony yet when one
// is founded, and the four windows name the town hall by position because that is
// what the player clicked — which is also why this family imports no `Dimension`.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::common::round_trip;

    #[test]
    fn founding_round_trips_with_and_without_a_claim() {
        let founding = CreateColony {
            town_hall: [100, 70, -200],
            claim: true,
            colony_name: "Royaume".to_owned(),
            pack_name: "minecolonies".to_owned(),
            path_name: "medieval/townhall1.blueprint".to_owned(),
        };
        assert!(round_trip(founding).claim);
        let placeless = CreateColony {
            town_hall: [0, 0, 0],
            claim: false,
            colony_name: String::new(),
            pack_name: String::new(),
            path_name: String::new(),
        };
        assert!(!round_trip(placeless).claim);
    }

    #[test]
    fn the_three_position_messages_round_trip() {
        assert_eq!(round_trip(GetColonyInfo { pos: [4, 65, 4] }).pos, [4, 65, 4]);
        assert_eq!(round_trip(PickUpTownhall { pos: [-1, 64, 3] }).pos, [-1, 64, 3]);
        assert_eq!(round_trip(ReactivateBuilding { pos: [7, 70, 9] }).pos, [7, 70, 9]);
    }

    #[test]
    fn a_direct_place_carries_its_block_state_id_and_the_stack() {
        let place = DirectPlace {
            state: 1234,
            pos: [10, 64, 10],
            stack: ItemStack::new("minecolonies:blockhutbuilder", 1).expect("a valid stack"),
        };
        let read = round_trip(place);
        assert_eq!(read.state, 1234);
        assert_eq!(read.stack.item.as_str(), "minecolonies:blockhutbuilder");
    }

    #[test]
    fn the_two_payload_free_messages_round_trip_to_an_empty_compound() {
        assert_eq!(round_trip(ColonyAbandonOwn).to_compound(), Compound::new());
        assert_eq!(round_trip(ColonyDeleteOwn).to_compound(), Compound::new());
    }

    #[test]
    fn the_warning_window_keeps_the_component_and_its_tooltip_flag() {
        let warning = OpenCantFoundColonyWarning {
            warning: Component::translatable("com.minecolonies.core.founding.tooclosetospawn")
                .with_argument("120"),
            town_hall_pos: [1, 2, 3],
            display_config_tooltip: true,
        };
        let read = round_trip(warning);
        assert_eq!(read.warning.arguments, vec!["120".to_owned()]);
        assert!(read.display_config_tooltip);
    }

    #[test]
    fn the_two_distance_windows_are_two_types_with_one_shape() {
        let covenant = OpenColonyFoundingCovenant {
            closest_name: "Royaume".to_owned(),
            closest_distance: 512,
            town_hall_pos: [0, 64, 0],
        };
        let reactivate = OpenReactivateColony {
            closest_name: "Royaume".to_owned(),
            closest_distance: 512,
            town_hall_pos: [0, 64, 0],
        };
        assert_eq!(round_trip(covenant.clone()).closest_distance, 512);
        assert_eq!(round_trip(reactivate).closest_name, covenant.closest_name);
        assert_eq!(covenant.to_compound(), reactivate_to_compound(&covenant));
    }

    /// The two messages really do write the same three keys: the day one of them
    /// grows a field, this comparison fails and forces the reader to look.
    fn reactivate_to_compound(covenant: &OpenColonyFoundingCovenant) -> Compound {
        OpenReactivateColony {
            closest_name: covenant.closest_name.clone(),
            closest_distance: covenant.closest_distance,
            town_hall_pos: covenant.town_hall_pos,
        }
        .to_compound()
    }

    #[test]
    fn the_abandoned_colony_window_round_trips_all_four_fields() {
        let window = OpenDeleteAbandonColony {
            current_town_hall_pos: [0, 64, 0],
            old_colony_name: "Vieux".to_owned(),
            old_colony_pos: [-64, 64, 12],
            old_colony_id: 3,
        };
        let read = round_trip(window);
        assert_eq!((read.old_colony_id, read.old_colony_pos), (3, [-64, 64, 12]));
    }

    #[test]
    fn a_founding_message_that_lost_its_town_hall_is_refused() {
        let mut data = GetColonyInfo { pos: [0, 0, 0] }.to_compound();
        data.remove(&"pos".into());
        assert_eq!(GetColonyInfo::from_compound(&data), Err(Refusal::Missing { key: "pos" }));
        let data = Compound::from([("pos".into(), int(4))]);
        assert_eq!(GetColonyInfo::from_compound(&data), Err(Refusal::Kind { key: "pos" }));
    }

    #[test]
    fn the_family_census_matches_the_types_it_declares() {
        assert_eq!(CENSUS.len(), 11);
        for (index, (file, name)) in CENSUS.iter().enumerate() {
            assert!(file.ends_with(".java"), "{file} is not a Java file");
            for (other, _) in &CENSUS[index + 1..] {
                assert_ne!(file, other, "two rows for {file}");
            }
            assert!(!name.is_empty());
        }
    }
}
