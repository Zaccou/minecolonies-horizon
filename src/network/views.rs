//! The colony view stream: what the server tells a client about a colony, a
//! building, a citizen, a work order, a research tree, a field, a visitor, an
//! animal — and the one message that opens a hut's screen from the server side.
//!
//! Java reference: `core/network/messages/client/colony/` — thirteen files,
//! `ColonyListMessage.java` (133), `ColonyViewAnimalViewDataMessage.java` (125),
//! `ColonyViewBuildingExtensionsUpdateMessage.java` (130),
//! `ColonyViewBuildingViewMessage.java` (98), `ColonyViewCitizenViewMessage.java`
//! (89), `ColonyViewMessage.java` (104), `ColonyViewRemoveBuildingMessage.java` (72),
//! `ColonyViewRemoveCitizenMessage.java` (71), `ColonyViewRemoveMessage.java` (60),
//! `ColonyViewRemoveWorkOrderMessage.java` (72),
//! `ColonyViewResearchManagerViewMessage.java` (94),
//! `ColonyViewWorkOrderMessage.java` (89), `ColonyVisitorViewDataMessage.java` (124),
//! `OpenBuildingUIMessage.java` (85). One thousand two hundred and sixty-six lines,
//! and **eight hundred of them are somebody else's serializer**.
//!
//! **This family is where the port's biggest hole is, and it names it.** Eight of
//! the fourteen carry a `FriendlyByteBuf` another class filled:
//!
//! | Field | Written by | Java |
//! | --- | --- | --- |
//! | `colony_data` | `Colony.serializeViewNetworkData` | `ColonyViewMessage.java:79-83` |
//! | `building_data` | `IBuilding.serializeToView` | `ColonyViewBuildingViewMessage.java:78-82` |
//! | `citizen_data` | `ICitizenData.serializeViewNetworkData` | `ColonyViewCitizenViewMessage.java:66-73` |
//! | `work_orders` | `IServerWorkOrder.serializeViewNetworkData` | `ColonyViewWorkOrderMessage.java:69-73` |
//! | `extensions` | `BuildingExtensionDataManager.extensionToBuffer` | `ColonyViewBuildingExtensionsUpdateMessage.java:69-77` |
//! | `visitors` | `IVisitorData.serializeViewNetworkData` | `ColonyVisitorViewDataMessage.java:92-99` |
//! | `animals` | `IAnimalData.serializeViewNetworkData` | `ColonyViewAnimalViewDataMessage.java:92-100` |
//! | `research` | `IResearchManager.writeToNBT` | `ColonyViewResearchManagerViewMessage.java:52-56` |
//!
//! Each of those encoders is a port of its own (a colony is 1 300 lines, a building
//! view another thousand), so the port carries the payload as **bytes** and keeps the
//! framing Java puts around it — the count, the length prefix, the colony and the
//! dimension. That is the honest half: a reader of this module knows exactly which
//! bytes are whose, and `MANQUANT.md` §1 lists the eight classes that still have to be
//! ported before the payload means anything.
//!
//! **`ColonyViewResearchManagerViewMessage` is the exception that proves it**: its
//! payload is a single NBT compound (`:56`), and the port has NBT, so the research
//! tree travels as a `Compound` and is not opaque at all.
//!
//! **`ColonyViewRemoveMessage` has no colony envelope either** — id and dimension and
//! nothing else (`:33-38`), because there is nothing left to describe: the colony is
//! being removed.

use super::common::{
    boolean, bool_of, bytes, bytes_of, int, int_in, int_of, nested, nested_of, pos_of, position,
    records, records_of, text, Message, Refusal, KEY_DIMENSION,
};
use crate::base::Dimension;
use mod_api::Compound;

/// `ColonyListMessage.java:89-132` — one row of the colony map's list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColonyInfo {
    pub id: i32,
    pub center: [i32; 3],
    pub name: String,
    pub citizen_count: i32,
    pub owner: String,
    /// The colony's prestige (`getColonyPrestige`, `:72`).
    pub prestige: i32,
}

impl ColonyInfo {
    /// The row as a compound, for the list `ColonyListMessage` writes (`:62-73`).
    fn to_record(&self) -> Compound {
        Compound::from([
            ("id".into(), int(self.id)),
            ("center".into(), position(self.center)),
            ("name".into(), text(&self.name)),
            ("citizencount".into(), int(self.citizen_count)),
            ("owner".into(), text(&self.owner)),
            ("prestige".into(), int(self.prestige)),
        ])
    }

    fn from_record(tag: &Compound) -> Result<ColonyInfo, Refusal> {
        Ok(ColonyInfo {
            id: inner_int(tag, "id")?,
            center: pos_of(tag, "center")?,
            name: super::common::text_of(tag, "name")?,
            citizen_count: inner_int(tag, "citizencount")?,
            owner: super::common::text_of(tag, "owner")?,
            prestige: inner_int(tag, "prestige")?,
        })
    }
}

fn inner_int(tag: &Compound, key: &'static str) -> Result<i32, Refusal> {
    match tag.get(&key.into()) {
        Some(mod_api::Tag::Int(value)) => Ok(*value),
        Some(_) => Err(Refusal::Kind { key }),
        None => Err(Refusal::Missing { key }),
    }
}

/// `ColonyListMessage.java:19-132` — every colony of a level, for the map window
/// (`:44-74`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColonyList {
    pub colonies: Vec<ColonyInfo>,
}

impl Message for ColonyList {
    fn to_compound(&self) -> Compound {
        let rows: Vec<Compound> = self.colonies.iter().map(ColonyInfo::to_record).collect();
        Compound::from([("colonies".into(), records(&rows))])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let mut colonies = Vec::new();
        for row in records_of(data, "colonies")? {
            colonies.push(ColonyInfo::from_record(row)?);
        }
        Ok(ColonyList { colonies })
    }
}

/// `ColonyViewMessage.java:22-103` — the whole colony view, on subscribe and on
/// every change.
///
/// `colony_data` is `Colony.serializeViewNetworkData`'s buffer (`:79-83`), carried
/// as bytes (`MANQUANT.md` §1). `is_new_subscription` tells the client whether to
/// build the view or update it (`:58-64`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColonyView {
    pub colony_id: i32,
    pub is_new_subscription: bool,
    pub dimension: Dimension,
    pub colony_data: Vec<u8>,
}

impl Message for ColonyView {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("colonyId".into(), int(self.colony_id)),
            ("isNewSubscription".into(), boolean(self.is_new_subscription)),
            (KEY_DIMENSION.into(), text(self.dimension.as_str())),
            ("colonyBuffer".into(), bytes(&self.colony_data)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(ColonyView {
            colony_id: int_of(data, "colonyId")?,
            is_new_subscription: bool_of(data, "isNewSubscription")?,
            dimension: super::common::dimension_of(data, KEY_DIMENSION)?,
            colony_data: bytes_of(data, "colonyBuffer")?,
        })
    }
}

/// `ColonyViewBuildingViewMessage.java:21-97` — one hut's view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColonyViewBuildingView {
    pub colony_id: i32,
    pub building_id: [i32; 3],
    pub dimension: Dimension,
    /// `IBuilding.serializeToView`'s buffer (`:78-82`), opaque here.
    pub building_data: Vec<u8>,
}

impl Message for ColonyViewBuildingView {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("colonyId".into(), int(self.colony_id)),
            ("buildingId".into(), position(self.building_id)),
            (KEY_DIMENSION.into(), text(self.dimension.as_str())),
            ("buildingData".into(), bytes(&self.building_data)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(ColonyViewBuildingView {
            colony_id: int_of(data, "colonyId")?,
            building_id: pos_of(data, "buildingId")?,
            dimension: super::common::dimension_of(data, KEY_DIMENSION)?,
            building_data: bytes_of(data, "buildingData")?,
        })
    }
}

/// `ColonyViewCitizenViewMessage.java:21-88` — one citizen's view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColonyViewCitizenView {
    pub colony_id: i32,
    pub citizen_id: i32,
    pub dimension: Dimension,
    /// `ICitizenData.serializeViewNetworkData`'s buffer (`:66-73`), opaque here.
    pub citizen_data: Vec<u8>,
}

impl Message for ColonyViewCitizenView {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("colonyId".into(), int(self.colony_id)),
            ("citizenId".into(), int(self.citizen_id)),
            (KEY_DIMENSION.into(), text(self.dimension.as_str())),
            ("citizenBuffer".into(), bytes(&self.citizen_data)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(ColonyViewCitizenView {
            colony_id: int_of(data, "colonyId")?,
            citizen_id: int_of(data, "citizenId")?,
            dimension: super::common::dimension_of(data, KEY_DIMENSION)?,
            citizen_data: bytes_of(data, "citizenBuffer")?,
        })
    }
}

/// `ColonyViewWorkOrderMessage.java:24-88` — the colony's work orders, as one
/// buffer of `IServerWorkOrder.serializeViewNetworkData` records (`:50-54`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColonyViewWorkOrder {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub work_orders: Vec<u8>,
}

impl Message for ColonyViewWorkOrder {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("colonyId".into(), int(self.colony_id)),
            (KEY_DIMENSION.into(), text(self.dimension.as_str())),
            ("workOrderBuffer".into(), bytes(&self.work_orders)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(ColonyViewWorkOrder {
            colony_id: int_of(data, "colonyId")?,
            dimension: super::common::dimension_of(data, KEY_DIMENSION)?,
            work_orders: bytes_of(data, "workOrderBuffer")?,
        })
    }
}

/// `ColonyViewResearchManagerViewMessage.java:23-93` — the research tree.
///
/// **Not opaque**: the payload is one NBT compound (`:54-56`), and the port carries
/// compounds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColonyViewResearchManagerView {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub research: Compound,
}

impl Message for ColonyViewResearchManagerView {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("colonyId".into(), int(self.colony_id)),
            (KEY_DIMENSION.into(), text(self.dimension.as_str())),
            ("researchManagerData".into(), nested(&self.research)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(ColonyViewResearchManagerView {
            colony_id: int_of(data, "colonyId")?,
            dimension: super::common::dimension_of(data, KEY_DIMENSION)?,
            research: nested_of(data, "researchManagerData")?,
        })
    }
}

/// `ColonyViewBuildingExtensionsUpdateMessage.java:26-129` — the fields and other
/// extensions of a colony, each one a length-prefixed buffer (`:71-77`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColonyViewBuildingExtensionsUpdate {
    pub colony_id: i32,
    pub dimension: Dimension,
    /// One buffer per extension, in the sender's order.
    pub extensions: Vec<Vec<u8>>,
}

impl Message for ColonyViewBuildingExtensionsUpdate {
    fn to_compound(&self) -> Compound {
        let rows: Vec<Compound> =
            self.extensions.iter().map(|data| Compound::from([("data".into(), bytes(data))])).collect();
        Compound::from([
            ("colonyId".into(), int(self.colony_id)),
            (KEY_DIMENSION.into(), text(self.dimension.as_str())),
            ("extensions".into(), records(&rows)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let mut extensions = Vec::new();
        for row in records_of(data, "extensions")? {
            extensions.push(match row.get(&"data".into()) {
                Some(mod_api::Tag::ByteArray(value)) => value.clone(),
                Some(_) => return Err(Refusal::Kind { key: "data" }),
                None => return Err(Refusal::Missing { key: "data" }),
            });
        }
        Ok(ColonyViewBuildingExtensionsUpdate {
            colony_id: int_of(data, "colonyId")?,
            dimension: super::common::dimension_of(data, KEY_DIMENSION)?,
            extensions,
        })
    }
}

/// `ColonyVisitorViewDataMessage.java:25-123` — the colony's visitors, one buffer of
/// records preceded by its count (`:92-100`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColonyVisitorViewData {
    pub colony_id: i32,
    pub dimension: Dimension,
    /// Whether the client rebuilds its list or updates it (`:73`, `:97`).
    pub refresh: bool,
    /// How many visitor records the buffer holds (`:97`) — written *before* the
    /// bytes, and kept separately because the bytes are opaque.
    pub count: i32,
    pub visitors: Vec<u8>,
}

impl Message for ColonyVisitorViewData {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("colonyId".into(), int(self.colony_id)),
            (KEY_DIMENSION.into(), text(self.dimension.as_str())),
            ("refresh".into(), boolean(self.refresh)),
            ("count".into(), int(self.count)),
            ("visitorBuf".into(), bytes(&self.visitors)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(ColonyVisitorViewData {
            colony_id: int_of(data, "colonyId")?,
            dimension: super::common::dimension_of(data, KEY_DIMENSION)?,
            refresh: bool_of(data, "refresh")?,
            count: int_in(data, "count", 0, i32::MAX)?,
            visitors: bytes_of(data, "visitorBuf")?,
        })
    }
}

/// `ColonyViewAnimalViewDataMessage.java:26-124` — the colony's animals, same shape
/// as the visitors (`:92-100`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColonyViewAnimalViewData {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub refresh: bool,
    pub count: i32,
    pub animals: Vec<u8>,
}

impl Message for ColonyViewAnimalViewData {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("colonyId".into(), int(self.colony_id)),
            (KEY_DIMENSION.into(), text(self.dimension.as_str())),
            ("refresh".into(), boolean(self.refresh)),
            ("count".into(), int(self.count)),
            ("animalBuf".into(), bytes(&self.animals)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(ColonyViewAnimalViewData {
            colony_id: int_of(data, "colonyId")?,
            dimension: super::common::dimension_of(data, KEY_DIMENSION)?,
            refresh: bool_of(data, "refresh")?,
            count: int_in(data, "count", 0, i32::MAX)?,
            animals: bytes_of(data, "animalBuf")?,
        })
    }
}

/// `ColonyViewRemoveMessage.java:17-59` — the colony is gone. Id and dimension, no
/// envelope: there is nothing left to describe (`:33-38`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColonyViewRemove {
    pub id: i32,
    pub dimension: Dimension,
}

impl Message for ColonyViewRemove {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("id".into(), int(self.id)),
            (KEY_DIMENSION.into(), text(self.dimension.as_str())),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(ColonyViewRemove {
            id: int_of(data, "id")?,
            dimension: super::common::dimension_of(data, KEY_DIMENSION)?,
        })
    }
}

/// `ColonyViewRemoveBuildingMessage.java:17-71` — that hut is gone (`:42-54`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColonyViewRemoveBuilding {
    pub colony_id: i32,
    pub building_id: [i32; 3],
}

impl Message for ColonyViewRemoveBuilding {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("colonyId".into(), int(self.colony_id)),
            ("buildingId".into(), position(self.building_id)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(ColonyViewRemoveBuilding {
            colony_id: int_of(data, "colonyId")?,
            building_id: pos_of(data, "buildingId")?,
        })
    }
}

/// `ColonyViewRemoveCitizenMessage.java:16-70` — that citizen is gone (`:41-53`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColonyViewRemoveCitizen {
    pub colony_id: i32,
    pub citizen_id: i32,
}

impl Message for ColonyViewRemoveCitizen {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("colonyId".into(), int(self.colony_id)),
            ("citizenId".into(), int(self.citizen_id)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(ColonyViewRemoveCitizen {
            colony_id: int_of(data, "colonyId")?,
            citizen_id: int_of(data, "citizenId")?,
        })
    }
}

/// `ColonyViewRemoveWorkOrderMessage.java:16-71` — that work order is gone
/// (`:42-54`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColonyViewRemoveWorkOrder {
    pub colony_id: i32,
    pub work_order_id: i32,
}

impl Message for ColonyViewRemoveWorkOrder {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("colonyId".into(), int(self.colony_id)),
            ("workOrderId".into(), int(self.work_order_id)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(ColonyViewRemoveWorkOrder {
            colony_id: int_of(data, "colonyId")?,
            work_order_id: int_of(data, "workOrderId")?,
        })
    }
}

/// `OpenBuildingUIMessage.java:22-84` — open this hut's screen on the client
/// (`:79-82`). Sent by the server right after a colony is founded
/// (`CreateColonyMessage.java:192`), which is why it is here and not with the other
/// doors: nobody clicked anything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenBuildingUI {
    pub colony_id: i32,
    pub building_id: [i32; 3],
    pub dimension: Dimension,
}

impl Message for OpenBuildingUI {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("colonyId".into(), int(self.colony_id)),
            ("buildingId".into(), position(self.building_id)),
            (KEY_DIMENSION.into(), text(self.dimension.as_str())),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(OpenBuildingUI {
            colony_id: int_of(data, "colonyId")?,
            building_id: pos_of(data, "buildingId")?,
            dimension: super::common::dimension_of(data, KEY_DIMENSION)?,
        })
    }
}

/// The fourteen messages of this family, by Java file and Rust type.
pub const CENSUS: &[(&str, &str)] = &[
    ("client/colony/ColonyListMessage.java", "ColonyList"),
    ("client/colony/ColonyViewMessage.java", "ColonyView"),
    ("client/colony/ColonyViewBuildingViewMessage.java", "ColonyViewBuildingView"),
    ("client/colony/ColonyViewCitizenViewMessage.java", "ColonyViewCitizenView"),
    ("client/colony/ColonyViewWorkOrderMessage.java", "ColonyViewWorkOrder"),
    ("client/colony/ColonyViewResearchManagerViewMessage.java", "ColonyViewResearchManagerView"),
    ("client/colony/ColonyViewBuildingExtensionsUpdateMessage.java", "ColonyViewBuildingExtensionsUpdate"),
    ("client/colony/ColonyVisitorViewDataMessage.java", "ColonyVisitorViewData"),
    ("client/colony/ColonyViewAnimalViewDataMessage.java", "ColonyViewAnimalViewData"),
    ("client/colony/ColonyViewRemoveMessage.java", "ColonyViewRemove"),
    ("client/colony/ColonyViewRemoveBuildingMessage.java", "ColonyViewRemoveBuilding"),
    ("client/colony/ColonyViewRemoveCitizenMessage.java", "ColonyViewRemoveCitizen"),
    ("client/colony/ColonyViewRemoveWorkOrderMessage.java", "ColonyViewRemoveWorkOrder"),
    ("client/colony/OpenBuildingUIMessage.java", "OpenBuildingUI"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::common::round_trip;

    fn overworld() -> Dimension {
        Dimension::new("minecraft:overworld")
    }

    #[test]
    fn a_colony_list_round_trips_row_by_row() {
        let list = ColonyList {
            colonies: vec![
                ColonyInfo {
                    id: 1,
                    center: [0, 64, 0],
                    name: "Royaume".to_owned(),
                    citizen_count: 12,
                    owner: "Simon".to_owned(),
                    prestige: 3,
                },
                ColonyInfo {
                    id: 2,
                    center: [-500, 70, 900],
                    name: "Voisine".to_owned(),
                    citizen_count: 0,
                    owner: "Zaccou".to_owned(),
                    prestige: 0,
                },
            ],
        };
        assert_eq!(round_trip(list.clone()).colonies, list.colonies);
        assert_eq!(round_trip(ColonyList { colonies: Vec::new() }).colonies, Vec::new());
    }

    #[test]
    fn the_colony_view_round_trips_with_its_subscription_flag() {
        for fresh in [true, false] {
            let view = ColonyView {
                colony_id: 3,
                is_new_subscription: fresh,
                dimension: overworld(),
                colony_data: vec![0, 3, 0, 0, 9],
            };
            assert_eq!(round_trip(view).is_new_subscription, fresh);
        }
    }

    #[test]
    fn the_building_and_citizen_views_round_trip_their_buffers() {
        let building = ColonyViewBuildingView {
            colony_id: 3,
            building_id: [10, 64, 10],
            dimension: overworld(),
            building_data: vec![1, 2, 3],
        };
        assert_eq!(round_trip(building).building_data, vec![1, 2, 3]);
        let citizen = ColonyViewCitizenView {
            colony_id: 3,
            citizen_id: 8,
            dimension: overworld(),
            citizen_data: vec![4, 5],
        };
        assert_eq!(round_trip(citizen).citizen_data, vec![4, 5]);
        let orders = ColonyViewWorkOrder {
            colony_id: 3,
            dimension: overworld(),
            work_orders: vec![0, 0, 0, 2],
        };
        assert_eq!(round_trip(orders).work_orders, vec![0, 0, 0, 2]);
    }

    #[test]
    fn the_research_tree_is_a_compound_not_a_buffer() {
        let research = Compound::from([
            ("branches".into(), mod_api::Tag::List(Vec::new())),
            ("progress".into(), int(4)),
        ]);
        let message = ColonyViewResearchManagerView {
            colony_id: 3,
            dimension: overworld(),
            research: research.clone(),
        };
        assert_eq!(round_trip(message).research, research);
        let mut data = ColonyViewResearchManagerView {
            colony_id: 3,
            dimension: overworld(),
            research: Compound::new(),
        }
        .to_compound();
        data.insert("researchManagerData".into(), int(1));
        assert_eq!(
            ColonyViewResearchManagerView::from_compound(&data),
            Err(Refusal::Kind { key: "researchManagerData" })
        );
    }

    #[test]
    fn the_extensions_round_trip_their_length_prefixed_buffers() {
        let update = ColonyViewBuildingExtensionsUpdate {
            colony_id: 4,
            dimension: overworld(),
            extensions: vec![vec![1], vec![2, 3], Vec::new()],
        };
        assert_eq!(round_trip(update).extensions, vec![vec![1], vec![2, 3], Vec::new()]);
    }

    #[test]
    fn the_visitor_and_animal_streams_keep_their_count() {
        let visitors = ColonyVisitorViewData {
            colony_id: 5,
            dimension: overworld(),
            refresh: true,
            count: 2,
            visitors: vec![0, 0, 0, 1, 0, 0, 0, 2],
        };
        let read = round_trip(visitors);
        assert_eq!((read.count, read.refresh), (2, true));
        let animals = ColonyViewAnimalViewData {
            colony_id: 5,
            dimension: overworld(),
            refresh: false,
            count: 0,
            animals: Vec::new(),
        };
        assert_eq!(round_trip(animals).count, 0);
        let mut data = ColonyViewAnimalViewData {
            colony_id: 5,
            dimension: overworld(),
            refresh: false,
            count: 0,
            animals: Vec::new(),
        }
        .to_compound();
        data.insert("count".into(), int(-1));
        assert_eq!(
            ColonyViewAnimalViewData::from_compound(&data),
            Err(Refusal::Number { key: "count", min: 0, max: i64::from(i32::MAX) })
        );
    }

    #[test]
    fn the_three_removals_round_trip() {
        assert_eq!(round_trip(ColonyViewRemove { id: 6, dimension: overworld() }).id, 6);
        assert_eq!(
            round_trip(ColonyViewRemoveBuilding { colony_id: 6, building_id: [1, 2, 3] }).building_id,
            [1, 2, 3]
        );
        assert_eq!(round_trip(ColonyViewRemoveCitizen { colony_id: 6, citizen_id: 4 }).citizen_id, 4);
        assert_eq!(
            round_trip(ColonyViewRemoveWorkOrder { colony_id: 6, work_order_id: 2 }).work_order_id,
            2
        );
    }

    #[test]
    fn opening_a_hut_screen_round_trips_the_three_fields() {
        let open = OpenBuildingUI { colony_id: 7, building_id: [0, 70, 0], dimension: overworld() };
        let read = round_trip(open);
        assert_eq!((read.colony_id, read.dimension.as_str()), (7, "minecraft:overworld"));
    }

    #[test]
    fn a_view_without_its_colony_is_refused() {
        let mut data = ColonyViewRemoveBuilding { colony_id: 1, building_id: [0, 0, 0] }.to_compound();
        data.remove(&"colonyId".into());
        assert_eq!(
            ColonyViewRemoveBuilding::from_compound(&data),
            Err(Refusal::Missing { key: "colonyId" })
        );
        assert_eq!(CENSUS.len(), 14);
    }
}
