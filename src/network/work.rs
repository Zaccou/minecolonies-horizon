//! Work: the work orders a builder takes, the state of a request, the clipboard's
//! warehouse snapshot, and the two "hand this over" messages — items to a citizen,
//! a recipe to a teaching screen.
//!
//! Java reference: `core/network/messages/server/colony/WorkOrderChangeMessage.java`
//! (93), `server/colony/UpdateRequestStateMessage.java` (91),
//! `server/ResourceScrollSaveWarehouseSnapshotMessage.java` (126),
//! `server/colony/citizen/TransferItemsToCitizenRequestMessage.java` (151),
//! `server/TransferRecipeCraftingTeachingMessage.java` (130),
//! `server/SwitchRecipeCraftingTeachingMessage.java` (52),
//! `server/PlayerAssistantBuildRequestMessage.java` (75).
//!
//! **`UpdateRequestStateMessage` is where the thirteen states come back.** The port
//! runs on a five-state [`crate::requests::State`] — `Created`, `Assigned`,
//! `InProgress`, `Delivered`, `Cancelled` — because it has no resolver and no
//! follow-ups (`src/requests/request.rs`, *Five states, and the reason there are not
//! thirteen*). The **wire** still carries Java's thirteen, because the request tree
//! window draws all of them; so [`RequestState`] here is the full thirteen, and the
//! mapping between the two enums is a decision for the request manager, not for a
//! message codec. Pretending the five were the thirteen would make a `RESOLVED`
//! message undecodable.
//!
//! **A request token has no type in this port.** Java's `IToken<?>` travels through
//! `StandardFactoryController.serialize` as an NBT compound
//! (`UpdateRequestStateMessage.java:72`), and the factory is a registry of
//! serializers this port has not ported; the token is carried as the compound it is
//! (`MANQUANT.md` §1).
//!
//! **Three of the nine are not colony messages**: the resource scroll's snapshot,
//! the recipe transfer and the recipe switch have no dimension and no colony
//! (`ResourceScrollSaveWarehouseSnapshotMessage.java:88-101`,
//! `TransferRecipeCraftingTeachingMessage.java:67-76`,
//! `SwitchRecipeCraftingTeachingMessage.java:29-32`), because they act on the
//! player's own container.

use super::common::{
    bool_of, boolean, envelope, envelope_of, int, int_in, int_of, nested, nested_of, opt_pos_of,
    pos_of, position, records_of, stack, stack_in, stack_of, text, text_of, ItemStack, Message,
    Refusal, Rule,
};
use crate::base::Dimension;
use mod_api::{Compound, Tag};

/// Java's thirteen request states, in the order Java declares them
/// (`api/colony/requestsystem/request/RequestState.java:16-78`).
///
/// The order is Java's and is kept for reading the file, **not** as a wire
/// encoding: this module writes the name, and Java's own comment on the enum is the
/// reason — "This should never be changed! It is used to read and write from NBT so
/// it has to persist between mod versions" (`:84-90`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the thirteen values of Java's RequestState
pub enum RequestState {
    /// `CREATED` (`:18`) — filed, not registered yet.
    Created,
    /// `REPORTED` (`:23`) — registered, not resolved.
    Reported,
    /// `ASSIGNING` (`:28`) — a resolver is being chosen.
    Assigning,
    /// `ASSIGNED` (`:33`) — chosen, not started.
    Assigned,
    /// `IN_PROGRESS` (`:38`) — being worked on.
    InProgress,
    /// `RESOLVED` (`:43`) — primary processing done, follow-ups open.
    Resolved,
    /// `FOLLOWUP_IN_PROGRESS` (`:48`).
    FollowupInProgress,
    /// `COMPLETED` (`:53`).
    Completed,
    /// `OVERRULED` (`:58`) — a player handed the goods over directly, which is the
    /// one state that carries a stack (`UpdateRequestStateMessage.java:63-66`).
    Overruled,
    /// `CANCELLED` (`:63`).
    Cancelled,
    /// `RECEIVED` (`:68`) — the requester took delivery.
    Received,
    /// `FINALIZING` (`:73`) — cancelled or overruled, cleaning up.
    Finalizing,
    /// `FAILED` (`:78`).
    Failed,
}

impl RequestState {
    /// Every state, in Java's declaration order.
    pub const ALL: [RequestState; 13] = [
        RequestState::Created,
        RequestState::Reported,
        RequestState::Assigning,
        RequestState::Assigned,
        RequestState::InProgress,
        RequestState::Resolved,
        RequestState::FollowupInProgress,
        RequestState::Completed,
        RequestState::Overruled,
        RequestState::Cancelled,
        RequestState::Received,
        RequestState::Finalizing,
        RequestState::Failed,
    ];

    /// The name Java writes — `RequestState.valueOf(action.name())`'s spelling, the
    /// enum constant as Java declares it.
    pub fn name(self) -> &'static str {
        match self {
            RequestState::Created => "CREATED",
            RequestState::Reported => "REPORTED",
            RequestState::Assigning => "ASSIGNING",
            RequestState::Assigned => "ASSIGNED",
            RequestState::InProgress => "IN_PROGRESS",
            RequestState::Resolved => "RESOLVED",
            RequestState::FollowupInProgress => "FOLLOWUP_IN_PROGRESS",
            RequestState::Completed => "COMPLETED",
            RequestState::Overruled => "OVERRULED",
            RequestState::Cancelled => "CANCELLED",
            RequestState::Received => "RECEIVED",
            RequestState::Finalizing => "FINALIZING",
            RequestState::Failed => "FAILED",
        }
    }

    pub fn parse(name: &str) -> Option<RequestState> {
        RequestState::ALL.into_iter().find(|state| state.name() == name)
    }

    /// Whether this state writes an item stack after itself — only `OVERRULED`
    /// (`UpdateRequestStateMessage.java:63-66`, `:74-77`).
    pub fn carries_item(self) -> bool {
        self == RequestState::Overruled
    }

    /// The five-state state this port would keep for it, or `None` when the port's
    /// reduced model has no place for it. The mapping is documented here and
    /// applied nowhere: `RAPPORT.md` records that the request manager is not ported.
    pub fn reduced(self) -> Option<crate::requests::State> {
        use crate::requests::State;
        match self {
            RequestState::Created | RequestState::Reported => Some(State::Created),
            RequestState::Assigning | RequestState::Assigned => Some(State::Assigned),
            RequestState::InProgress | RequestState::FollowupInProgress => Some(State::InProgress),
            RequestState::Completed | RequestState::Received => Some(State::Delivered),
            RequestState::Cancelled | RequestState::Failed | RequestState::Finalizing => {
                Some(State::Cancelled)
            }
            RequestState::Resolved | RequestState::Overruled => None,
        }
    }
}

/// `WorkOrderChangeMessage.java:13-92` — remove a work order, or move its priority
/// (`:83-90`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkOrderChange {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub work_order_id: i32,
    pub priority: i32,
    /// `true` is `removeWorkOrder` (`:85`), `false` is `setPriority` (`:89`). Both
    /// travel; the flag says which of the two the other field is for.
    pub remove_work_order: bool,
}

impl Message for WorkOrderChange {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert("workOrderId".into(), int(self.work_order_id));
        data.insert("priority".into(), int(self.priority));
        data.insert("removeWorkOrder".into(), boolean(self.remove_work_order));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        Ok(WorkOrderChange {
            colony_id,
            dimension,
            work_order_id: int_of(data, "workOrderId")?,
            priority: int_of(data, "priority")?,
            remove_work_order: bool_of(data, "removeWorkOrder")?,
        })
    }
}

/// `UpdateRequestStateMessage.java:17-90` — the request tree moved a request to
/// another state, or a player overruled it with an item in hand (`:83-89`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateRequestState {
    pub colony_id: i32,
    pub dimension: Dimension,
    /// The request's token, as `StandardFactoryController` wrote it (`:72`).
    pub token: Compound,
    pub state: RequestState,
    /// Present **exactly when** the state is `OVERRULED`: the stack the player
    /// handed over. `Some` for any other state is refused, `None` for `OVERRULED`
    /// too — a message Java cannot write.
    pub item_stack: Option<ItemStack>,
}

impl Message for UpdateRequestState {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert("token".into(), nested(&self.token));
        data.insert("state".into(), text(self.state.name()));
        if let Some(item) = &self.item_stack {
            data.insert("itemStack".into(), stack(item));
        }
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        let name = text_of(data, "state")?;
        let state = RequestState::parse(&name).ok_or(Refusal::Name { key: "state" })?;
        let item_stack = match data.get(&"itemStack".into()) {
            Some(_) => Some(stack_of(data, "itemStack")?),
            None => None,
        };
        match (state.carries_item(), item_stack.is_some()) {
            (true, false) => return Err(Refusal::Missing { key: "itemStack" }),
            (false, true) => return Err(Refusal::Kind { key: "itemStack" }),
            _ => {}
        }
        Ok(UpdateRequestState { colony_id, dimension, token: nested_of(data, "token")?, state, item_stack })
    }
}

/// `ResourceScrollSaveWarehouseSnapshotMessage.java:23-125` — the clipboard writes
/// what the warehouse held, keyed by item name (`:113-123`).
///
/// **No colony, no dimension**: the message edits the scroll in the player's own
/// inventory (`:113-116`), and the builder's position is what identifies which scroll
/// — carried as an `Option` because Java writes a presence flag (`:90-94`).
///
/// The snapshot is a `Map<String, Integer>` in Java (`:34`). A map has no order;
/// the port keeps the pairs in the order the sender wrote them, so two messages that
/// differ only by order are different messages — which is honest about what the
/// wire is and costs nothing, since nothing compares two snapshots for order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceScrollSaveWarehouseSnapshot {
    /// `None` when the message carries no builder, which Java sends as `false`
    /// (`:90`).
    pub builder_pos: Option<[i32; 3]>,
    /// `(item name, amount)`, in the sender's order.
    pub snapshot: Vec<(String, i32)>,
    /// The work order the snapshot was taken for (`:40`, `:100`).
    pub work_order_hash: String,
}

impl Message for ResourceScrollSaveWarehouseSnapshot {
    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        if let Some(builder) = self.builder_pos {
            data.insert("builderPos".into(), position(builder));
        }
        let rows: Vec<Compound> = self
            .snapshot
            .iter()
            .map(|(item, amount)| {
                Compound::from([("item".into(), text(item)), ("amount".into(), int(*amount))])
            })
            .collect();
        data.insert("snapshot".into(), super::common::records(&rows));
        data.insert("workOrderHash".into(), text(&self.work_order_hash));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let mut snapshot = Vec::new();
        for row in records_of(data, "snapshot")? {
            snapshot.push((super::common::text_in(&Tag::Compound(row.clone()), "item")?, {
                match row.get(&"amount".into()) {
                    Some(Tag::Int(amount)) => *amount,
                    Some(_) => return Err(Refusal::Kind { key: "amount" }),
                    None => return Err(Refusal::Missing { key: "amount" }),
                }
            }));
        }
        Ok(ResourceScrollSaveWarehouseSnapshot {
            builder_pos: opt_pos_of(data, "builderPos")?,
            snapshot,
            work_order_hash: text_of(data, "workOrderHash")?,
        })
    }
}

/// `TransferItemsToCitizenRequestMessage.java:26-150` — the player hands items to a
/// citizen, optionally answering the request they were looking at (`:145-148`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferItemsToCitizenRequest {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub citizen_id: i32,
    pub stack: ItemStack,
    /// Java refuses `<= 0` unless the player is creative (`:120-124`); the port
    /// refuses a count below `1` at the door and notes the creative case in
    /// `RAPPORT.md`, because "creative" is a player property this message does not
    /// carry.
    pub quantity: i32,
    /// The request being answered, as the factory wrote it; `None` when the player
    /// was not answering one (`:145`).
    pub token: Option<Compound>,
}

impl Message for TransferItemsToCitizenRequest {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert("citizenId".into(), int(self.citizen_id));
        data.insert("itemStack".into(), stack(&self.stack));
        data.insert("quantity".into(), int(self.quantity));
        if let Some(token) = &self.token {
            data.insert("token".into(), nested(token));
        }
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        Ok(TransferItemsToCitizenRequest {
            colony_id,
            dimension,
            citizen_id: int_of(data, "citizenId")?,
            stack: stack_of(data, "itemStack")?,
            quantity: int_in(data, "quantity", 1, i32::MAX)?,
            token: match data.get(&"token".into()) {
                Some(_) => Some(nested_of(data, "token")?),
                None => None,
            },
        })
    }
}

/// `TransferRecipeCraftingTeachingMessage.java:21-129` — the teaching screen sends
/// the nine slots it was filled with (`:93-127`).
///
/// Java keys the stacks by slot (`Map<Integer, ItemStack>`, `:31`) and treats
/// `complete` as "all nine slots, in order 0-8" or "the first four"
/// (`:93-111`). The port keeps the pairs in the sender's order, as the snapshot
/// above does, for the same reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferRecipeCraftingTeaching {
    /// `(slot, stack)`, in the sender's order.
    pub item_stacks: Vec<(i32, ItemStack)>,
    /// `true` for the whole grid, `false` for the first four slots (`:93`).
    pub complete: bool,
}

impl Message for TransferRecipeCraftingTeaching {
    fn to_compound(&self) -> Compound {
        let rows: Vec<Compound> = self
            .item_stacks
            .iter()
            .map(|(slot, item)| {
                Compound::from([("slot".into(), int(*slot)), ("stack".into(), stack(item))])
            })
            .collect();
        Compound::from([
            ("itemStacks".into(), super::common::records(&rows)),
            ("complete".into(), boolean(self.complete)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let mut item_stacks = Vec::new();
        for row in records_of(data, "itemStacks")? {
            let slot = match row.get(&"slot".into()) {
                Some(Tag::Int(slot)) => *slot,
                Some(_) => return Err(Refusal::Kind { key: "slot" }),
                None => return Err(Refusal::Missing { key: "slot" }),
            };
            let tag = row.get(&"stack".into()).ok_or(Refusal::Missing { key: "stack" })?;
            if !matches!(tag, Tag::Compound(_)) {
                return Err(Refusal::Kind { key: "stack" });
            }
            item_stacks.push((slot, stack_in(tag)?));
        }
        Ok(TransferRecipeCraftingTeaching {
            item_stacks,
            complete: bool_of(data, "complete")?,
        })
    }
}

/// `SwitchRecipeCraftingTeachingMessage.java:14-51` — flip the teaching screen
/// between its two modes (`:45-49`).
///
/// No fields (`:24-32`): which screen to flip is the one the player has open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SwitchRecipeCraftingTeaching;

impl Message for SwitchRecipeCraftingTeaching {
    fn to_compound(&self) -> Compound {
        Compound::new()
    }

    fn from_compound(_data: &Compound) -> Result<Self, Refusal> {
        Ok(SwitchRecipeCraftingTeaching)
    }
}

/// `PlayerAssistantBuildRequestMessage.java:17-74` — the assistant hammer places the
/// block of a work order for the player (`:69-72`).
/// `permissionNeeded()` is `PLACE_BLOCKS` (`:51-55`), not the builder's
/// `MANAGE_HUTS`: an assistant build is placing blocks, not managing a hut.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerAssistantBuildRequest {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub workorder_id: i32,
    pub interact_pos: [i32; 3],
}

impl Message for PlayerAssistantBuildRequest {
    const RULE: Rule = Rule::PlaceBlocks;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert("workorderID".into(), int(self.workorder_id));
        data.insert("interactPos".into(), position(self.interact_pos));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        Ok(PlayerAssistantBuildRequest {
            colony_id,
            dimension,
            workorder_id: int_of(data, "workorderID")?,
            interact_pos: pos_of(data, "interactPos")?,
        })
    }
}

/// `WorkOrderType.java:10-13` — what kind of work order is being created.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four values of Java's WorkOrderType
pub enum WorkOrderType {
    Build,
    Upgrade,
    Repair,
    Remove,
}

impl WorkOrderType {
    pub const ALL: [WorkOrderType; 4] =
        [WorkOrderType::Build, WorkOrderType::Upgrade, WorkOrderType::Repair, WorkOrderType::Remove];

    pub fn name(self) -> &'static str {
        match self {
            WorkOrderType::Build => "BUILD",
            WorkOrderType::Upgrade => "UPGRADE",
            WorkOrderType::Repair => "REPAIR",
            WorkOrderType::Remove => "REMOVE",
        }
    }

    pub fn parse(name: &str) -> Option<WorkOrderType> {
        WorkOrderType::ALL.into_iter().find(|kind| kind.name() == name)
    }
}

/// The eight fields a "build this blueprint here" request carries.
///
/// **One struct for two Java classes, on purpose.**
/// `DecorationBuildRequestMessage.java:42-82` and
/// `PlantationFieldBuildRequestMessage.java:42-82` declare the same eight fields and
/// write them with the same eight calls in the same order (`:122-132` and `:117-127`);
/// what differs is the work order the server builds from them
/// (`WorkOrderDecoration` against `WorkOrderPlantationField`, `:183-204` and
/// `:191-213`). Stating that once, and letting the two message types differ in their
/// name, is cheaper and clearer than copying fifty lines twice — and the day one of
/// them grows a field, the two newtypes stop sharing their base.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructureRequest {
    pub pos: [i32; 3],
    pub pack_name: String,
    /// The blueprint's path inside its pack (`:52`).
    pub path: String,
    pub dimension: Dimension,
    /// `Rotation.ordinal()` in Java (`:130`, `:125`).
    pub rotation: super::windows::Rotation,
    /// `mirror != Mirror.NONE` (`:62` of the window that sent it).
    pub mirror: bool,
    pub work_order_type: WorkOrderType,
    /// Where the build tool was clicked; `BlockPos.ZERO` means "nobody claimed it"
    /// (`:194-201`).
    pub builder: [i32; 3],
}

impl Message for StructureRequest {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("workOrderType".into(), text(self.work_order_type.name())),
            ("pos".into(), position(self.pos)),
            ("packName".into(), text(&self.pack_name)),
            ("path".into(), text(&self.path)),
            ("dimension".into(), text(self.dimension.as_str())),
            ("mirror".into(), boolean(self.mirror)),
            ("rotation".into(), text(self.rotation.name())),
            ("builder".into(), position(self.builder)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let kind = text_of(data, "workOrderType")?;
        let rotation = text_of(data, "rotation")?;
        Ok(StructureRequest {
            pos: pos_of(data, "pos")?,
            pack_name: text_of(data, "packName")?,
            path: text_of(data, "path")?,
            dimension: super::common::dimension_of(data, "dimension")?,
            rotation: super::windows::Rotation::parse(&rotation)
                .ok_or(Refusal::Name { key: "rotation" })?,
            mirror: bool_of(data, "mirror")?,
            work_order_type: WorkOrderType::parse(&kind).ok_or(Refusal::Name { key: "workOrderType" })?,
            builder: pos_of(data, "builder")?,
        })
    }
}

/// `DecorationBuildRequestMessage.java:37-206` — build a decoration blueprint, or
/// remove the one already there (`:157-165`).
///
/// Sent by the decoration preview's click (`OpenDecoBuildWindowMessage.java:38-42`);
/// the gate is `MANAGE_HUTS` (`:152`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecorationBuildRequest(pub StructureRequest);

impl Message for DecorationBuildRequest {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        self.0.to_compound()
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(DecorationBuildRequest(StructureRequest::from_compound(data)?))
    }
}

/// `PlantationFieldBuildRequestMessage.java:37-214` — build a plantation field
/// blueprint (`:165-173`).
///
/// The same eight fields as the decoration request, and a different work order class
/// (`:191-213`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlantationFieldBuildRequest {
    pub request: StructureRequest,
}

impl Message for PlantationFieldBuildRequest {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        self.request.to_compound()
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(PlantationFieldBuildRequest { request: StructureRequest::from_compound(data)? })
    }
}

/// The nine messages of this family, by Java file and Rust type.
pub const CENSUS: &[(&str, &str)] = &[
    ("server/colony/WorkOrderChangeMessage.java", "WorkOrderChange"),
    ("server/colony/UpdateRequestStateMessage.java", "UpdateRequestState"),
    ("server/ResourceScrollSaveWarehouseSnapshotMessage.java", "ResourceScrollSaveWarehouseSnapshot"),
    ("server/colony/citizen/TransferItemsToCitizenRequestMessage.java", "TransferItemsToCitizenRequest"),
    ("server/TransferRecipeCraftingTeachingMessage.java", "TransferRecipeCraftingTeaching"),
    ("server/SwitchRecipeCraftingTeachingMessage.java", "SwitchRecipeCraftingTeaching"),
    ("server/PlayerAssistantBuildRequestMessage.java", "PlayerAssistantBuildRequest"),
    ("server/DecorationBuildRequestMessage.java", "DecorationBuildRequest"),
    ("server/PlantationFieldBuildRequestMessage.java", "PlantationFieldBuildRequest"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::common::round_trip;

    fn overworld() -> Dimension {
        Dimension::new("minecraft:overworld")
    }

    fn token() -> Compound {
        Compound::from([("id".into(), text("0f8e1c4a-0000-0000-0000-000000000001"))])
    }

    fn stack_of_items(name: &str, count: i32) -> ItemStack {
        ItemStack::new(name, count).expect("a valid stack")
    }

    #[test]
    fn a_work_order_change_keeps_both_readings_of_its_flag() {
        for remove in [true, false] {
            let change = WorkOrderChange {
                colony_id: 2,
                dimension: overworld(),
                work_order_id: 7,
                priority: 3,
                remove_work_order: remove,
            };
            let read = round_trip(change);
            assert_eq!((read.remove_work_order, read.priority), (remove, 3));
        }
    }

    #[test]
    fn an_overruled_request_carries_its_stack_and_every_other_state_does_not() {
        let overruled = UpdateRequestState {
            colony_id: 2,
            dimension: overworld(),
            token: token(),
            state: RequestState::Overruled,
            item_stack: Some(stack_of_items("minecraft:bread", 4)),
        };
        let read = round_trip(overruled);
        assert_eq!(read.state, RequestState::Overruled);
        assert_eq!(read.item_stack.expect("the stack").count, 4);

        let reported = UpdateRequestState {
            colony_id: 2,
            dimension: overworld(),
            token: token(),
            state: RequestState::Reported,
            item_stack: None,
        };
        assert_eq!(round_trip(reported).state, RequestState::Reported);
    }

    #[test]
    fn a_state_and_its_item_must_agree_or_the_message_is_refused() {
        let mut data = UpdateRequestState {
            colony_id: 1,
            dimension: overworld(),
            token: token(),
            state: RequestState::Overruled,
            item_stack: None,
        }
        .to_compound();
        assert_eq!(
            UpdateRequestState::from_compound(&data),
            Err(Refusal::Missing { key: "itemStack" }),
            "OVERRULED without a stack is a message Java cannot write"
        );
        data.insert("itemStack".into(), stack(&stack_of_items("minecraft:bread", 1)));
        data.insert("state".into(), text("COMPLETED"));
        assert_eq!(UpdateRequestState::from_compound(&data), Err(Refusal::Kind { key: "itemStack" }));
        data.remove(&"itemStack".into());
        data.insert("state".into(), text("RESOLVING"));
        assert_eq!(UpdateRequestState::from_compound(&data), Err(Refusal::Name { key: "state" }));
    }

    #[test]
    fn every_thirteen_state_round_trips_and_maps_where_the_port_has_a_place() {
        for state in RequestState::ALL {
            assert_eq!(RequestState::parse(state.name()), Some(state));
            let data = Compound::from([("state".into(), text(state.name()))]);
            assert_eq!(text_of(&data, "state").map(|name| RequestState::parse(&name)), Ok(Some(state)));
        }
        assert_eq!(RequestState::ALL.len(), 13);
        assert_eq!(RequestState::Completed.reduced(), Some(crate::requests::State::Delivered));
        assert_eq!(RequestState::Overruled.reduced(), None, "no five-state home for an overrule");
        assert_eq!(RequestState::Resolved.reduced(), None);
        assert_eq!(
            RequestState::ALL.iter().filter(|state| state.carries_item()).count(),
            1,
            "only OVERRULED writes a stack"
        );
    }

    #[test]
    fn a_snapshot_round_trips_with_and_without_a_builder() {
        for builder in [None, Some([12, 64, -3])] {
            let snapshot = ResourceScrollSaveWarehouseSnapshot {
                builder_pos: builder,
                snapshot: vec![
                    ("minecraft:oak_log".to_owned(), 64),
                    ("minecraft:cobblestone".to_owned(), 128),
                ],
                work_order_hash: "a1b2c3".to_owned(),
            };
            assert_eq!(round_trip(snapshot).builder_pos, builder);
        }
        let empty = ResourceScrollSaveWarehouseSnapshot {
            builder_pos: None,
            snapshot: Vec::new(),
            work_order_hash: String::new(),
        };
        assert!(round_trip(empty).snapshot.is_empty());
    }

    #[test]
    fn a_hand_over_to_a_citizen_keeps_its_request_token() {
        let with_token = TransferItemsToCitizenRequest {
            colony_id: 3,
            dimension: overworld(),
            citizen_id: 21,
            stack: stack_of_items("minecraft:apple", 2),
            quantity: 2,
            token: Some(token()),
        };
        assert_eq!(round_trip(with_token.clone()).token, with_token.token);
        let without = TransferItemsToCitizenRequest { token: None, ..with_token };
        assert_eq!(round_trip(without).token, None);
    }

    #[test]
    fn a_transfer_to_a_citizen_refuses_a_quantity_of_nothing() {
        let mut data = TransferItemsToCitizenRequest {
            colony_id: 3,
            dimension: overworld(),
            citizen_id: 21,
            stack: stack_of_items("minecraft:apple", 1),
            quantity: 1,
            token: None,
        }
        .to_compound();
        data.insert("quantity".into(), int(0));
        assert_eq!(
            TransferItemsToCitizenRequest::from_compound(&data),
            Err(Refusal::Number { key: "quantity", min: 1, max: i64::from(i32::MAX) })
        );
    }

    #[test]
    fn a_recipe_transfer_round_trips_nine_slots_of_which_most_are_empty() {
        let transfer = TransferRecipeCraftingTeaching {
            item_stacks: vec![
                (0, stack_of_items("minecraft:oak_planks", 1)),
                (1, stack_of_items("minecraft:air", 0)),
                (2, stack_of_items("minecraft:stick", 8)),
            ],
            complete: true,
        };
        let read = round_trip(transfer.clone());
        assert_eq!(read.item_stacks, transfer.item_stacks);
        assert!(read.item_stacks[1].1.is_empty(), "an empty slot is a stack of zero");
    }

    #[test]
    fn a_recipe_switch_round_trips_to_nothing() {
        assert_eq!(round_trip(SwitchRecipeCraftingTeaching).to_compound(), Compound::new());
    }

    #[test]
    fn the_assistant_hammer_asks_for_placing_blocks_not_for_managing_huts() {
        let request = PlayerAssistantBuildRequest {
            colony_id: 4,
            dimension: overworld(),
            workorder_id: 11,
            interact_pos: [5, 64, 5],
        };
        assert_eq!(round_trip(request).interact_pos, [5, 64, 5]);
        assert_eq!(PlayerAssistantBuildRequest::RULE, Rule::PlaceBlocks);
        assert_eq!(WorkOrderChange::RULE, Rule::ManageHuts);
        assert_eq!(CENSUS.len(), 9);
    }

    #[test]
    fn the_two_build_requests_share_eight_fields_and_one_shape() {
        let structure = StructureRequest {
            pos: [12, 64, 12],
            pack_name: "minecolonies".to_owned(),
            path: "decoration/bell_tower1.blueprint".to_owned(),
            dimension: overworld(),
            rotation: super::super::windows::Rotation::Clockwise90,
            mirror: false,
            work_order_type: WorkOrderType::Build,
            builder: [0, 0, 0],
        };
        for kind in WorkOrderType::ALL {
            let decoration = DecorationBuildRequest(StructureRequest {
                work_order_type: kind,
                ..structure.clone()
            });
            assert_eq!(round_trip(decoration).0.work_order_type, kind);
        }
        let field = PlantationFieldBuildRequest { request: structure.clone() };
        assert_eq!(field.to_compound(), DecorationBuildRequest(structure.clone()).to_compound());
        assert_eq!(round_trip(field).request, structure);

        let mut data = DecorationBuildRequest(structure).to_compound();
        data.insert("workOrderType".into(), text("DESTROY"));
        assert_eq!(
            DecorationBuildRequest::from_compound(&data),
            Err(Refusal::Name { key: "workOrderType" })
        );
    }
}
