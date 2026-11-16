//! The registry: one stable id per message, and the two maps Java keeps in step by
//! hand.
//!
//! Java reference: `core/network/NetworkChannel.java` — `registerCommonMessages`
//! (`:107-280`, `int idx = 0` then `registerMessage(++idx, …)`), the internal
//! `SplitPacketMessage` on `id 0` (`:282-294`), and `registerMessage` itself
//! (`:304-308`).
//!
//! **The ids are Java's, in Java's order, and that is the whole point.** `idx` is
//! incremented once per `registerMessage` call, so the id a class gets *is* its
//! position in that method, and moving one registration renumbers every registration
//! after it. Java's own `RequestState` warns what that costs
//! (`api/colony/requestsystem/request/RequestState.java:84-90`): "This should never be
//! changed! It is used to read and write from NBT so it has to persist between mod
//! versions." So [`ALL`] is written in the file's own order, id by id, and a test
//! walks 0..=138 to prove no row is missing and none is out of place. A port that
//! renumbered would still compile; this table would not be the proof it is.
//!
//! **Six columns, each measurable against the Java tree.**
//!
//! | Column | Where it is read |
//! | --- | --- |
//! | `id` | the position in `registerCommonMessages`, `0` for `SplitPacketMessage` |
//! | `java` | the class as Java spells it; `PermissionsMessage.View` keeps the nesting |
//! | `rust` | the port's type, `module::Type` from [`crate::network`]'s census, or `""` |
//! | `file` | the `.java` file, see the path rule below |
//! | `direction` | `getExecutionSide()` — `null` is `Bidirectional` |
//! | `range` | `IRangeRestrictedPacket.getMaxRange()`: `Some(15)`, `Some(5)` for a building message, `None` |
//!
//! **The path rule, one sentence.** A `file` that starts with `core/` is relative to
//! `com/minecolonies/` — those are the six classes that live outside
//! `core/network/messages/`; every other `file` is relative to
//! `com/minecolonies/core/network/messages/`, exactly as [`crate::network`]'s census
//! writes them. A test holds the two sets apart, because the six are also exactly the
//! rows with no port type.
//!
//! **The direction column is `getExecutionSide()`, and `null` is not a guess.**
//! `IMessage.getExecutionSide` defaults to `null` and says so — "CLIENT or SERVER or
//! null (for both)" (`api/network/IMessage.java:27-36`). `AbstractColonyServerMessage`
//! overrides it `final` returning `SERVER` (`:110-113`), and every server message
//! inherits that through both bases; the client messages that override it return
//! `CLIENT`; the ten that override nothing are `Bidirectional` here and were `null`
//! there. The numbers are 86, 41 and 12, and a test states them.
//!
//! **Two messages are server-bound by what they do and `null` by what they say** —
//! `FarmFieldPlotResizeMessage` and `ItemSettingMessage` implement `IMessage` directly
//! (`server/colony/building/fields/FarmFieldPlotResizeMessage.java:22`,
//! `server/ItemSettingMessage.java:13`). The table says `Bidirectional`, because that
//! is what the Java says; the reader who needs the intent should see the file's own
//! `server/` folder and the `RAPPORT.md` note, not a guess in a column.

use std::collections::BTreeMap;

use mod_api::{LogicalSide, PayloadDirection};
// The three answers `getExecutionSide()` can give, under the names the table reads by:
// `CLIENT`, `SERVER`, and Java's `null` — which the SDK spells `Bidirectional`.
use mod_api::PayloadDirection::{Bidirectional as Both, ToClient as Client, ToServer as Server};

use super::Refusal;

/// A message's id on the wire.
///
/// Java's is an `int` (`registerMessage(int id, …)`, `NetworkChannel.java:304`); the
/// port's is a `u16` because 139 is where the table stops and a negative id is never a
/// message. `-1` is Java's "not in the map" answer (`:437`), and the port spends a
/// [`Refusal::UnknownId`] instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MessageId(u16);

impl MessageId {
    /// The id of `SplitPacketMessage`, registered by hand before the loop
    /// (`NetworkChannel.java:284`).
    pub const SPLIT: MessageId = MessageId(0);

    pub const fn of(raw: u16) -> MessageId {
        MessageId(raw)
    }

    pub const fn raw(self) -> u16 {
        self.0
    }
}

impl std::fmt::Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "net.message#{}", self.0)
    }
}

/// One row of the table: what Java registered, and what the port made of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The wire id, Java's position in `registerCommonMessages`.
    pub id: MessageId,
    /// The Java class, as `registerMessage(…, X.class, …)` names it.
    pub java: &'static str,
    /// The port's type as `module::Type`, or `""` when the port has none — the six
    /// classes outside `core/network/messages/`.
    pub rust: &'static str,
    /// The `.java` file: relative to `com/minecolonies/core/network/messages/`, or to
    /// `com/minecolonies/` when it starts with `core/`. See the module note.
    pub file: &'static str,
    /// `IMessage.getExecutionSide()`: where the message runs, which is where it
    /// travels to. `Bidirectional` is Java's `null`.
    pub direction: PayloadDirection,
    /// `IRangeRestrictedPacket.getMaxRange()` in blocks, when the message has one.
    pub range: Option<i32>,
}

impl Entry {
    /// The port's type, or `None` for a row only Java has a type for.
    pub fn rust_type(self) -> Option<&'static str> {
        if self.rust.is_empty() {
            None
        } else {
            Some(self.rust)
        }
    }

    /// The side this message executes on, `None` for `Bidirectional` — Java's `null`,
    /// "for both".
    pub fn arrives_at(self) -> Option<LogicalSide> {
        match self.direction {
            PayloadDirection::ToClient => Some(LogicalSide::Client),
            PayloadDirection::ToServer => Some(LogicalSide::Server),
            PayloadDirection::Bidirectional => None,
        }
    }

    /// Whether a message with this row may travel **from** `origin`.
    ///
    /// Java's test is inverted-looking and means exactly this: a message that runs on
    /// the server must come *from* the client, and one that runs on the client must
    /// come from the server, so arriving from the side it names is the bug
    /// (`SplitPacketMessage.java:131-136`).
    pub fn comes_from(self, origin: LogicalSide) -> bool {
        match self.arrives_at() {
            None => true,
            Some(side) => side != origin,
        }
    }
}

/// The one row `registerCommonMessages` never reaches: `OpenBuildWindow` is the
/// abstract base of two registered messages, so the port's type exists and has no id
/// of its own (`client/OpenBuildWindowMessage.java:16`, extended by
/// `OpenDecoBuildWindowMessage.java:14` and
/// `OpenPlantationFieldBuildWindowMessage.java:14`).
///
/// Named here so the arithmetic in the tests closes: 134 census types minus this one
/// minus [`SPLIT_PACKET`] is the 132 rows that carry a port type.
pub const NO_ID: &[&str] = &["windows::OpenBuildWindow"];

const fn entry(
    id: u16,
    java: &'static str,
    rust: &'static str,
    file: &'static str,
    direction: PayloadDirection,
    range: Option<i32>,
) -> Entry {
    Entry { id: MessageId::of(id), java, rust, file, direction, range }
}

/// `SplitPacketMessage`, the one message whose id is not a position in
/// `registerCommonMessages`: `setupInternalMessages` registers it on `0` before the
/// loop (`NetworkChannel.java:282-294`).
pub const SPLIT_PACKET: Entry = entry(
    0,
    "SplitPacketMessage",
    "debug::SplitPacket",
    "splitting/SplitPacketMessage.java",
    PayloadDirection::Bidirectional,
    None,
);

/// Every message the channel knows, `id` ascending, in Java's registration order.
///
/// Read a row's `direction` as "where it runs", its `range` as "how far from what it
/// is about the sender must stand", and `rust` as "open this module to read the
/// fields". The six rows with no `rust` are the ones Java registers and this port has
/// not typed yet; [`Registry::ported`] refuses them by name instead of pretending.
pub const ALL: &[Entry] = &[
    entry(0, "SplitPacketMessage", "debug::SplitPacket", "splitting/SplitPacketMessage.java", Both, None),
    entry(1, "ColonyViewMessage", "views::ColonyView", "client/colony/ColonyViewMessage.java", Client, None),
    entry(2, "ColonyViewCitizenViewMessage", "views::ColonyViewCitizenView", "client/colony/ColonyViewCitizenViewMessage.java", Client, None),
    entry(3, "ColonyViewRemoveCitizenMessage", "views::ColonyViewRemoveCitizen", "client/colony/ColonyViewRemoveCitizenMessage.java", Client, None),
    entry(4, "ColonyViewBuildingViewMessage", "views::ColonyViewBuildingView", "client/colony/ColonyViewBuildingViewMessage.java", Client, None),
    entry(5, "ColonyViewRemoveBuildingMessage", "views::ColonyViewRemoveBuilding", "client/colony/ColonyViewRemoveBuildingMessage.java", Client, None),
    entry(6, "ColonyViewBuildingExtensionsUpdateMessage", "views::ColonyViewBuildingExtensionsUpdate", "client/colony/ColonyViewBuildingExtensionsUpdateMessage.java", Client, None),
    entry(7, "PermissionsMessage.View", "permissions::PermissionsView", "PermissionsMessage.java", Client, None),
    entry(8, "ColonyViewWorkOrderMessage", "views::ColonyViewWorkOrder", "client/colony/ColonyViewWorkOrderMessage.java", Client, None),
    entry(9, "ColonyViewRemoveWorkOrderMessage", "views::ColonyViewRemoveWorkOrder", "client/colony/ColonyViewRemoveWorkOrderMessage.java", Client, None),
    entry(10, "UpdateChunkCapabilityMessage", "debug::UpdateChunkCapability", "client/UpdateChunkCapabilityMessage.java", Client, None),
    entry(11, "ColonyViewResearchManagerViewMessage", "views::ColonyViewResearchManagerView", "client/colony/ColonyViewResearchManagerViewMessage.java", Client, None),
    entry(12, "PermissionsMessage.Permission", "permissions::Permission", "PermissionsMessage.java", Server, None),
    entry(13, "PermissionsMessage.AddPlayer", "permissions::AddPlayer", "PermissionsMessage.java", Server, None),
    entry(14, "PermissionsMessage.RemovePlayer", "permissions::RemovePlayer", "PermissionsMessage.java", Server, None),
    entry(15, "PermissionsMessage.ChangePlayerRank", "permissions::ChangePlayerRank", "PermissionsMessage.java", Both, None),
    entry(16, "PermissionsMessage.AddPlayerOrFakePlayer", "permissions::AddPlayerOrFakePlayer", "PermissionsMessage.java", Server, None),
    entry(17, "PermissionsMessage.AddRank", "permissions::AddRank", "PermissionsMessage.java", Both, None),
    entry(18, "PermissionsMessage.RemoveRank", "permissions::RemoveRank", "PermissionsMessage.java", Both, None),
    entry(19, "PermissionsMessage.EditRankType", "permissions::EditRankType", "PermissionsMessage.java", Both, None),
    entry(20, "BuildRequestMessage", "buildings::BuildRequest", "server/colony/building/BuildRequestMessage.java", Server, Some(5)),
    entry(21, "OpenInventoryMessage", "windows::OpenInventory", "server/colony/OpenInventoryMessage.java", Server, Some(15)),
    entry(22, "TownHallRenameMessage", "colony_settings::TownHallRename", "server/colony/TownHallRenameMessage.java", Server, None),
    entry(23, "MinerSetLevelMessage", "building_modules::MinerSetLevel", "server/colony/building/miner/MinerSetLevelMessage.java", Server, Some(5)),
    entry(24, "RecallCitizenMessage", "building_modules::RecallCitizen", "server/colony/building/worker/RecallCitizenMessage.java", Server, Some(5)),
    entry(25, "HireFireMessage", "buildings::HireFire", "server/colony/building/HireFireMessage.java", Server, Some(5)),
    entry(26, "WorkOrderChangeMessage", "work::WorkOrderChange", "server/colony/WorkOrderChangeMessage.java", Server, None),
    entry(27, "AssignFieldMessage", "building_modules::AssignField", "server/colony/building/fields/AssignFieldMessage.java", Server, Some(5)),
    entry(28, "AssignmentModeMessage", "building_modules::AssignmentMode", "server/colony/building/fields/AssignmentModeMessage.java", Server, Some(5)),
    entry(29, "GuardSetMinePosMessage", "building_modules::GuardSetMinePos", "server/colony/building/guard/GuardSetMinePosMessage.java", Server, Some(5)),
    entry(30, "RecallCitizenHutMessage", "buildings::RecallCitizenHut", "server/colony/building/RecallCitizenHutMessage.java", Server, Some(5)),
    entry(31, "TransferItemsRequestMessage", "buildings::TransferItemsRequest", "server/colony/building/TransferItemsRequestMessage.java", Server, Some(5)),
    entry(32, "MarkBuildingDirtyMessage", "buildings::MarkBuildingDirty", "server/colony/building/MarkBuildingDirtyMessage.java", Server, Some(5)),
    entry(33, "ChangeFreeToInteractBlockMessage", "colony_settings::ChangeFreeToInteractBlock", "server/colony/ChangeFreeToInteractBlockMessage.java", Server, None),
    entry(34, "CreateColonyMessage", "founding::CreateColony", "server/CreateColonyMessage.java", Server, Some(15)),
    entry(35, "ColonyDeleteOwnMessage", "founding::ColonyDeleteOwn", "server/colony/ColonyDeleteOwnMessage.java", Server, None),
    entry(36, "ColonyViewRemoveMessage", "views::ColonyViewRemove", "client/colony/ColonyViewRemoveMessage.java", Client, None),
    entry(37, "GiveToolMessage", "buildings::GiveTool", "server/colony/building/GiveToolMessage.java", Server, Some(5)),
    entry(38, "ColonyAbandonOwnMessage", "founding::ColonyAbandonOwn", "server/colony/ColonyAbandonOwnMessage.java", Server, None),
    entry(39, "TriggerConnectionEventMessage", "colony_settings::TriggerConnectionEvent", "server/colony/TriggerConnectionEventMessage.java", Server, None),
    entry(40, "AssignUnassignMessage", "building_modules::AssignUnassign", "server/colony/building/home/AssignUnassignMessage.java", Server, Some(5)),
    entry(41, "OpenCraftingGUIMessage", "buildings::OpenCraftingGUI", "server/colony/building/OpenCraftingGUIMessage.java", Server, Some(5)),
    entry(42, "AddRemoveRecipeMessage", "building_modules::AddRemoveRecipe", "server/colony/building/worker/AddRemoveRecipeMessage.java", Server, Some(5)),
    entry(43, "ChangeRecipePriorityMessage", "building_modules::ChangeRecipePriority", "server/colony/building/worker/ChangeRecipePriorityMessage.java", Server, Some(5)),
    entry(44, "ChangeDeliveryPriorityMessage", "buildings::ChangeDeliveryPriority", "server/colony/building/ChangeDeliveryPriorityMessage.java", Server, Some(5)),
    entry(45, "ForcePickupMessage", "buildings::ForcePickup", "server/colony/building/ForcePickupMessage.java", Server, Some(5)),
    entry(46, "UpgradeWarehouseMessage", "building_modules::UpgradeWarehouse", "server/colony/building/warehouse/UpgradeWarehouseMessage.java", Server, Some(5)),
    entry(47, "TransferItemsToCitizenRequestMessage", "work::TransferItemsToCitizenRequest", "server/colony/citizen/TransferItemsToCitizenRequestMessage.java", Server, None),
    entry(48, "UpdateRequestStateMessage", "work::UpdateRequestState", "server/colony/UpdateRequestStateMessage.java", Server, None),
    entry(49, "BuildingSetStyleMessage", "buildings::BuildingSetStyle", "server/colony/building/BuildingSetStyleMessage.java", Server, Some(5)),
    entry(50, "RecallSingleCitizenMessage", "citizens::RecallSingleCitizen", "server/colony/citizen/RecallSingleCitizenMessage.java", Server, Some(5)),
    entry(51, "AssignFilterableItemMessage", "buildings::AssignFilterableItem", "server/colony/building/AssignFilterableItemMessage.java", Server, Some(5)),
    entry(52, "TeamColonyColorChangeMessage", "colony_settings::TeamColonyColorChange", "server/colony/TeamColonyColorChangeMessage.java", Server, None),
    entry(53, "ColonyFlagChangeMessage", "colony_settings::ColonyFlagChange", "server/colony/ColonyFlagChangeMessage.java", Server, None),
    entry(54, "ColonyStructureStyleMessage", "colony_settings::ColonyStructureStyle", "server/colony/ColonyStructureStyleMessage.java", Server, None),
    entry(55, "PauseCitizenMessage", "citizens::PauseCitizen", "server/colony/citizen/PauseCitizenMessage.java", Server, None),
    entry(56, "RestartCitizenMessage", "citizens::RestartCitizen", "server/colony/citizen/RestartCitizenMessage.java", Server, None),
    entry(57, "SortBuildingMessage", "building_modules::SortBuilding", "server/colony/building/warehouse/SortBuildingMessage.java", Server, Some(5)),
    entry(58, "PostBoxRequestMessage", "building_modules::PostBoxRequest", "server/colony/building/postbox/PostBoxRequestMessage.java", Server, Some(5)),
    entry(59, "HireMercenaryMessage", "colony_settings::HireMercenary", "server/colony/HireMercenaryMessage.java", Server, None),
    entry(60, "HutRenameMessage", "buildings::HutRename", "server/colony/building/HutRenameMessage.java", Server, Some(5)),
    entry(61, "BuildingHiringModeMessage", "building_modules::BuildingHiringMode", "server/colony/building/worker/BuildingHiringModeMessage.java", Server, Some(5)),
    entry(62, "DecorationBuildRequestMessage", "work::DecorationBuildRequest", "server/DecorationBuildRequestMessage.java", Server, None),
    entry(63, "DirectPlaceMessage", "founding::DirectPlace", "server/DirectPlaceMessage.java", Server, None),
    entry(64, "TeleportToColonyMessage", "colony_settings::TeleportToColony", "server/colony/TeleportToColonyMessage.java", Server, Some(15)),
    entry(65, "EnchanterWorkerSetMessage", "building_modules::EnchanterWorkerSet", "server/colony/building/enchanter/EnchanterWorkerSetMessage.java", Server, Some(5)),
    entry(66, "InteractionResponse", "citizens::InteractionResponse", "server/colony/InteractionResponse.java", Server, None),
    entry(67, "TryResearchMessage", "building_modules::TryResearch", "server/colony/building/university/TryResearchMessage.java", Server, Some(5)),
    entry(68, "HireSpiesMessage", "colony_settings::HireSpies", "server/colony/HireSpiesMessage.java", Server, None),
    entry(69, "AddMinimumStockToBuildingModuleMessage", "buildings::AddMinimumStockToBuildingModule", "server/colony/building/AddMinimumStockToBuildingModuleMessage.java", Server, Some(5)),
    entry(70, "RemoveMinimumStockFromBuildingModuleMessage", "buildings::RemoveMinimumStockFromBuildingModule", "server/colony/building/RemoveMinimumStockFromBuildingModuleMessage.java", Server, Some(5)),
    entry(71, "FarmFieldPlotResizeMessage", "building_modules::FarmFieldPlotResize", "server/colony/building/fields/FarmFieldPlotResizeMessage.java", Both, Some(15)),
    entry(72, "FarmFieldRegistrationMessage", "building_modules::FarmFieldRegistration", "server/colony/building/fields/FarmFieldRegistrationMessage.java", Server, None),
    entry(73, "FarmFieldUpdateSeedMessage", "building_modules::FarmFieldUpdateSeed", "server/colony/building/fields/FarmFieldUpdateSeedMessage.java", Server, None),
    entry(74, "AdjustSkillCitizenMessage", "citizens::AdjustSkillCitizen", "server/colony/citizen/AdjustSkillCitizenMessage.java", Server, None),
    entry(75, "BuilderSelectWorkOrderMessage", "building_modules::BuilderSelectWorkOrder", "server/colony/building/builder/BuilderSelectWorkOrderMessage.java", Server, Some(5)),
    entry(76, "TriggerSettingMessage", "buildings::TriggerSetting", "server/colony/building/TriggerSettingMessage.java", Server, None),
    entry(77, "AssignFilterableEntityMessage", "buildings::AssignFilterableEntity", "server/colony/building/AssignFilterableEntityMessage.java", Server, Some(5)),
    entry(78, "BuildPickUpMessage", "buildings::BuildPickUp", "server/colony/building/BuildPickUpMessage.java", Server, Some(5)),
    entry(79, "SwitchBuildingWithToolMessage", "items::SwitchBuildingWithTool", "server/SwitchBuildingWithToolMessage.java", Server, None),
    entry(80, "ColonyTextureStyleMessage", "colony_settings::ColonyTextureStyle", "server/colony/ColonyTextureStyleMessage.java", Server, None),
    entry(81, "MinerRepairLevelMessage", "building_modules::MinerRepairLevel", "server/colony/building/miner/MinerRepairLevelMessage.java", Server, Some(5)),
    entry(82, "PlantationFieldBuildRequestMessage", "work::PlantationFieldBuildRequest", "server/PlantationFieldBuildRequestMessage.java", Server, None),
    entry(83, "ResetFilterableItemMessage", "buildings::ResetFilterableItem", "server/colony/building/ResetFilterableItemMessage.java", Server, Some(5)),
    entry(84, "CourierHiringModeMessage", "buildings::CourierHiringMode", "server/colony/building/CourierHiringModeMessage.java", Server, Some(5)),
    entry(85, "QuarryHiringModeMessage", "buildings::QuarryHiringMode", "server/colony/building/QuarryHiringModeMessage.java", Server, Some(5)),
    entry(86, "ToggleRecipeMessage", "building_modules::ToggleRecipe", "server/colony/building/worker/ToggleRecipeMessage.java", Server, Some(5)),
    entry(87, "ColonyNameStyleMessage", "colony_settings::ColonyNameStyle", "server/colony/ColonyNameStyleMessage.java", Server, None),
    entry(88, "InteractionClose", "citizens::InteractionClose", "server/colony/InteractionClose.java", Server, None),
    entry(89, "GetColonyInfoMessage", "founding::GetColonyInfo", "server/GetColonyInfoMessage.java", Server, Some(15)),
    entry(90, "PickUpTownhallMessage", "founding::PickUpTownhall", "server/PickUpTownhallMessage.java", Server, Some(15)),
    entry(91, "MarkStoryReadOnItemMessage", "items::MarkStoryReadOnItem", "server/MarkStoryReadOnItemMessage.java", Server, None),
    entry(92, "AlterRestaurantMenuItemMessage", "buildings::AlterRestaurantMenuItem", "server/colony/building/AlterRestaurantMenuItemMessage.java", Server, Some(5)),
    entry(93, "BlockParticleEffectMessage", "particles::BlockParticleEffect", "client/BlockParticleEffectMessage.java", Client, None),
    entry(94, "CompostParticleMessage", "particles::CompostParticle", "client/CompostParticleMessage.java", Client, None),
    entry(95, "ItemParticleEffectMessage", "particles::ItemParticleEffect", "client/ItemParticleEffectMessage.java", Client, None),
    entry(96, "LocalizedParticleEffectMessage", "particles::LocalizedParticleEffect", "client/LocalizedParticleEffectMessage.java", Client, None),
    entry(97, "UpdateChunkRangeCapabilityMessage", "debug::UpdateChunkRangeCapability", "client/UpdateChunkRangeCapabilityMessage.java", Client, None),
    entry(98, "OpenSuggestionWindowMessage", "windows::OpenSuggestionWindow", "client/OpenSuggestionWindowMessage.java", Client, None),
    entry(99, "UpdateClientWithCompatibilityMessage", "debug::UpdateClientWithCompatibility", "client/UpdateClientWithCompatibilityMessage.java", Client, None),
    entry(100, "CircleParticleEffectMessage", "particles::CircleParticleEffect", "client/CircleParticleEffectMessage.java", Client, None),
    entry(101, "StreamParticleEffectMessage", "particles::StreamParticleEffect", "client/StreamParticleEffectMessage.java", Client, None),
    entry(102, "SleepingParticleMessage", "particles::SleepingParticle", "client/SleepingParticleMessage.java", Client, None),
    entry(103, "VanillaParticleMessage", "particles::VanillaParticle", "client/VanillaParticleMessage.java", Client, None),
    entry(104, "StopMusicMessage", "sounds::StopMusic", "client/StopMusicMessage.java", Client, None),
    entry(105, "PlayAudioMessage", "sounds::PlayAudio", "client/PlayAudioMessage.java", Client, None),
    entry(106, "PlayMusicAtPosMessage", "sounds::PlayMusicAtPos", "client/colony/PlayMusicAtPosMessage.java", Client, None),
    entry(107, "ColonyVisitorViewDataMessage", "views::ColonyVisitorViewData", "client/colony/ColonyVisitorViewDataMessage.java", Client, None),
    entry(108, "ColonyViewAnimalViewDataMessage", "views::ColonyViewAnimalViewData", "client/colony/ColonyViewAnimalViewDataMessage.java", Client, None),
    entry(109, "SyncPathMessage", "debug::SyncPath", "client/SyncPathMessage.java", Client, None),
    entry(110, "SyncPathReachedMessage", "debug::SyncPathReached", "client/SyncPathReachedMessage.java", Client, None),
    entry(111, "ReactivateBuildingMessage", "founding::ReactivateBuilding", "server/ReactivateBuildingMessage.java", Server, Some(15)),
    entry(112, "PlaySoundForCitizenMessage", "sounds::PlaySoundForCitizen", "client/colony/PlaySoundForCitizenMessage.java", Client, None),
    entry(113, "OpenDecoBuildWindowMessage", "windows::OpenDecoBuildWindow", "client/OpenDecoBuildWindowMessage.java", Client, None),
    entry(114, "OpenPlantationFieldBuildWindowMessage", "windows::OpenPlantationFieldBuildWindow", "client/OpenPlantationFieldBuildWindowMessage.java", Client, None),
    entry(115, "SaveStructureNBTMessage", "items::SaveStructureNbt", "client/SaveStructureNBTMessage.java", Client, None),
    entry(116, "GlobalQuestSyncMessage", "debug::GlobalQuestSync", "client/GlobalQuestSyncMessage.java", Client, None),
    entry(117, "GlobalDiseaseSyncMessage", "debug::GlobalDiseaseSync", "client/colony/GlobalDiseaseSyncMessage.java", Client, None),
    entry(118, "OpenColonyFoundingCovenantMessage", "founding::OpenColonyFoundingCovenant", "client/OpenColonyFoundingCovenantMessage.java", Both, None),
    entry(119, "OpenBuildingUIMessage", "views::OpenBuildingUI", "client/colony/OpenBuildingUIMessage.java", Client, None),
    entry(120, "OpenCantFoundColonyWarningMessage", "founding::OpenCantFoundColonyWarning", "client/OpenCantFoundColonyWarningMessage.java", Both, None),
    entry(121, "OpenDeleteAbandonColonyMessage", "founding::OpenDeleteAbandonColony", "client/OpenDeleteAbandonColonyMessage.java", Both, None),
    entry(122, "OpenReactivateColonyMessage", "founding::OpenReactivateColony", "client/OpenReactivateColonyMessage.java", Both, None),
    entry(123, "TransferRecipeCraftingTeachingMessage", "work::TransferRecipeCraftingTeaching", "server/TransferRecipeCraftingTeachingMessage.java", Server, None),
    entry(124, "OpenGuiWindowTriggerMessage", "windows::OpenGuiWindowTrigger", "server/OpenGuiWindowTriggerMessage.java", Server, None),
    entry(125, "ClickGuiButtonTriggerMessage", "windows::ClickGuiButtonTrigger", "server/ClickGuiButtonTriggerMessage.java", Server, None),
    entry(126, "RemoveFromRallyingListMessage", "items::RemoveFromRallyingList", "server/RemoveFromRallyingListMessage.java", Server, None),
    entry(127, "ToggleBannerRallyGuardsMessage", "items::ToggleBannerRallyGuards", "server/ToggleBannerRallyGuardsMessage.java", Server, None),
    entry(128, "GlobalResearchTreeMessage", "", "core/research/GlobalResearchTreeMessage.java", Client, None),
    entry(129, "CustomRecipeManagerMessage", "", "core/colony/crafting/CustomRecipeManagerMessage.java", Client, None),
    entry(130, "ColonyListMessage", "views::ColonyList", "client/colony/ColonyListMessage.java", Both, None),
    entry(131, "ResourceScrollSaveWarehouseSnapshotMessage", "work::ResourceScrollSaveWarehouseSnapshot", "server/ResourceScrollSaveWarehouseSnapshotMessage.java", Server, None),
    entry(132, "SwitchRecipeCraftingTeachingMessage", "work::SwitchRecipeCraftingTeaching", "server/SwitchRecipeCraftingTeachingMessage.java", Server, None),
    entry(133, "PlayerAssistantBuildRequestMessage", "work::PlayerAssistantBuildRequest", "server/PlayerAssistantBuildRequestMessage.java", Server, None),
    entry(134, "QueryCitizenAIHistoryMessage", "", "core/debug/messages/QueryCitizenAIHistoryMessage.java", Server, None),
    entry(135, "DebugEnablePathfindingMessage", "", "core/debug/messages/DebugEnablePathfindingMessage.java", Server, None),
    entry(136, "DebugOutputMessage", "", "core/debug/messages/DebugOutputMessage.java", Client, None),
    entry(137, "DebugEnableMessage", "", "core/debug/messages/DebugEnableMessage.java", Client, None),
    entry(138, "ItemSettingMessage", "items::ItemSetting", "server/ItemSettingMessage.java", Both, None),
];

/// The two maps [`NetworkChannel`] keeps, built once and checked.
///
/// `NetworkChannel` holds `Map<Integer, NetworkingMessageEntry<?>>` and
/// `Map<Class<? extends IMessage>, Integer>` (`NetworkChannel.java:72-77`) and fills
/// both in `registerMessage` with `put`, which silently replaces (`:304-308`). This
/// one refuses instead, and keeps the third index Java does not need and a port does:
/// the port's own type name, so a test can ask "is this type registered, and where".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Registry {
    by_id: BTreeMap<u16, &'static Entry>,
    by_java: BTreeMap<&'static str, &'static Entry>,
    by_rust: BTreeMap<&'static str, &'static Entry>,
}

impl Registry {
    /// The registry of [`ALL`]. It cannot fail, and a test proves it by calling it:
    /// the table is a `const`, so a duplicate would be a build-time fact this call
    /// turns into a readable one.
    pub fn new() -> Registry {
        Self::from_entries(ALL).expect("the channel table has no duplicate id, class or type")
    }

    /// Build a registry from any table, naming the first collision.
    ///
    /// Java cannot answer this question: `put` overwrote. A table with two rows on one
    /// id, two rows on one class, or two rows on one port type is refused whole — half
    /// a registry would route the half that loaded and drop the rest silently, which is
    /// the failure this variant exists to prevent.
    pub fn from_entries(entries: &'static [Entry]) -> Result<Registry, Refusal> {
        let mut by_id = BTreeMap::new();
        let mut by_java = BTreeMap::new();
        let mut by_rust = BTreeMap::new();
        for entry in entries {
            if by_id.insert(entry.id.raw(), entry).is_some() {
                return Err(Refusal::DuplicateId { id: entry.id.raw() });
            }
            if by_java.insert(entry.java, entry).is_some() {
                return Err(Refusal::DuplicateName { name: entry.java });
            }
            if let Some(rust) = entry.rust_type() {
                if by_rust.insert(rust, entry).is_some() {
                    return Err(Refusal::DuplicateName { name: rust });
                }
            }
        }
        Ok(Registry { by_id, by_java, by_rust })
    }

    /// The row for `id`, whatever its port type — the answer a router needs before it
    /// can say *why* it will not treat this message.
    pub fn get(&self, id: MessageId) -> Result<&'static Entry, Refusal> {
        self.by_id.get(&id.raw()).copied().ok_or(Refusal::UnknownId { id: id.raw() })
    }

    /// The row for `id`, refusing the six the port has no type for.
    pub fn ported(&self, id: MessageId) -> Result<&'static Entry, Refusal> {
        let entry = self.get(id)?;
        entry.rust_type().map(|_| entry).ok_or(Refusal::NotPorted { id: id.raw() })
    }

    /// The row a Java class name names, for a caller holding a name and not an id.
    ///
    /// `None` rather than a [`Refusal`]: this is a lookup, not a routing decision, and
    /// the caller who needs a reason has the name in hand to put in its own sentence.
    pub fn of_java(&self, java: &str) -> Option<&'static Entry> {
        self.by_java.get(java).copied()
    }

    /// The row a port type names — `"buildings::BuildRequest"`.
    pub fn of_rust(&self, rust: &str) -> Option<&'static Entry> {
        self.by_rust.get(rust).copied()
    }

    /// Every row, id ascending.
    pub fn entries(&self) -> impl Iterator<Item = &'static Entry> + '_ {
        self.by_id.values().copied()
    }

    /// How many rows carry a port type.
    pub fn ported_count(&self) -> usize {
        self.by_rust.len()
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `network::census`'s count of types in the tree (`census.rs`, 134 rows).
    const CENSUS_TYPES: usize = 134;
    /// The classes `registerCommonMessages` names that live outside
    /// `core/network/messages/`: `GlobalResearchTreeMessage`, `CustomRecipeManagerMessage`,
    /// `QueryCitizenAIHistoryMessage`, `DebugEnablePathfindingMessage`,
    /// `DebugOutputMessage`, `DebugEnableMessage`.
    const OUTSIDE_MESSAGES: usize = 6;

    #[test]
    fn the_ids_are_zero_to_138_with_no_hole_and_no_repeat() {
        assert_eq!(ALL.len(), 139, "138 from the loop plus SplitPacketMessage on 0");
        assert_eq!(ALL[0], SPLIT_PACKET, "id 0 is registered by hand, before the loop");
        for (position, entry) in ALL.iter().enumerate() {
            assert_eq!(entry.id.raw() as usize, position, "{} is out of place", entry.java);
        }
        assert_eq!(ALL[138].id.raw(), 138, "the last registration");
    }

    #[test]
    fn no_row_repeats_a_java_class_or_a_port_type() {
        let registry = Registry::from_entries(ALL).expect("the table is clean");
        assert_eq!(registry.entries().count(), 139);
        assert_eq!(
            registry.ported_count(),
            133,
            "132 registered census types plus SplitPacketMessage, whose type the port has"
        );
    }

    #[test]
    fn the_arithmetic_of_the_census_closes() {
        let ported = ALL.iter().filter(|entry| entry.rust_type().is_some()).count();
        let outside = ALL.iter().filter(|entry| entry.rust.is_empty()).count();
        assert_eq!(outside, OUTSIDE_MESSAGES);
        assert_eq!(ported, 133, "132 census types with an id, plus SplitPacketMessage");
        // The census counts 134 types: those 133, plus OpenBuildWindow, which is the
        // abstract base of two registered messages and has no id of its own.
        assert_eq!(ported + NO_ID.len(), CENSUS_TYPES);
        assert_eq!(ported + outside, ALL.len());
    }

    #[test]
    fn the_six_untyped_rows_are_exactly_the_ones_outside_messages() {
        for entry in ALL {
            if entry.rust.is_empty() {
                assert!(
                    entry.file.starts_with("core/"),
                    "{} has no type but is filed inside messages/",
                    entry.java
                );
            } else if entry.id != MessageId::SPLIT {
                assert!(
                    !entry.file.starts_with("core/"),
                    "{} is outside messages/ and yet carries a type",
                    entry.java
                );
            }
        }
    }

    #[test]
    fn the_directions_are_86_server_41_client_12_both() {
        let count = |direction: PayloadDirection| {
            ALL.iter().filter(|entry| entry.direction == direction).count()
        };
        assert_eq!(count(PayloadDirection::ToServer), 86);
        assert_eq!(count(PayloadDirection::ToClient), 41);
        assert_eq!(count(PayloadDirection::Bidirectional), 12);
        assert_eq!(86 + 41 + 12, ALL.len());
    }

    #[test]
    fn the_ranges_are_38_buildings_7_places_and_94_none() {
        let five = ALL.iter().filter(|entry| entry.range == Some(5)).count();
        let fifteen = ALL.iter().filter(|entry| entry.range == Some(15)).count();
        let none = ALL.iter().filter(|entry| entry.range.is_none()).count();
        assert_eq!(five, 38, "every message of AbstractBuildingServerMessage");
        assert_eq!(fifteen, 7, "the explicit IRangeRestrictedPacket implementors");
        assert_eq!(none, 94);
        assert_eq!(five + fifteen + none, ALL.len());
    }

    #[test]
    fn the_two_ranges_are_the_java_constants() {
        assert_eq!(crate::network::common::RANGE, 15, "IRangeRestrictedPacket.java:11");
        assert_eq!(
            crate::network::common::BUILDING_RANGE, 5,
            "AbstractBuildingServerMessage.java:70-74"
        );
    }

    #[test]
    fn a_duplicate_id_is_refused_and_names_the_id() {
        let mut rows = [SPLIT_PACKET; 2];
        rows[1].id = MessageId::of(0);
        rows[1].java = "OtherMessage";
        let table: &'static [Entry] = Box::leak(rows.to_vec().into_boxed_slice());
        assert_eq!(Registry::from_entries(table).err(), Some(Refusal::DuplicateId { id: 0 }));
    }

    #[test]
    fn a_duplicate_class_or_port_type_is_refused_and_names_it() {
        let mut rows = [SPLIT_PACKET; 2];
        rows[1].id = MessageId::of(7);
        let table: &'static [Entry] = Box::leak(rows.to_vec().into_boxed_slice());
        assert_eq!(
            Registry::from_entries(table).err(),
            Some(Refusal::DuplicateName { name: "SplitPacketMessage" })
        );

        let mut rows = [SPLIT_PACKET; 2];
        rows[1].id = MessageId::of(7);
        rows[1].java = "OtherMessage";
        let table: &'static [Entry] = Box::leak(rows.to_vec().into_boxed_slice());
        assert_eq!(
            Registry::from_entries(table).err(),
            Some(Refusal::DuplicateName { name: "debug::SplitPacket" })
        );
    }

    #[test]
    fn an_id_nobody_registered_is_unknown_and_a_row_without_a_type_is_not() {
        let registry = Registry::new();
        let absent = registry.get(MessageId::of(200));
        assert_eq!(absent.err(), Some(Refusal::UnknownId { id: 200 }));
        let untyped = registry.get(MessageId::of(128)).expect("GlobalResearchTreeMessage");
        assert_eq!(untyped.java, "GlobalResearchTreeMessage");
        assert_eq!(untyped.rust_type(), None);
        assert_eq!(registry.ported(MessageId::of(128)).err(), Some(Refusal::NotPorted { id: 128 }));
        assert!(registry.ported(MessageId::of(20)).is_ok(), "BuildRequestMessage is typed");
    }

    #[test]
    fn the_two_names_look_up_the_same_row_and_an_unknown_one_is_none() {
        let registry = Registry::new();
        let by_java = registry.of_java("BuildRequestMessage").expect("registered");
        let by_rust = registry.of_rust("buildings::BuildRequest").expect("ported");
        assert_eq!(by_java, by_rust);
        assert_eq!(by_java.id.raw(), 20);
        assert_eq!(by_java.direction, PayloadDirection::ToServer);
        assert_eq!(by_java.range, Some(5), "an AbstractBuildingServerMessage");
        assert!(registry.of_rust("buildings::Nope").is_none());
        assert!(registry.of_java("Nope").is_none());
    }

    #[test]
    fn a_side_is_where_it_runs_and_coming_from_that_side_is_the_bug() {
        let registry = Registry::new();
        let view = registry.get(MessageId::of(1)).expect("ColonyViewMessage");
        assert_eq!(view.arrives_at(), Some(LogicalSide::Client));
        assert!(view.comes_from(LogicalSide::Server), "sent by the server");
        assert!(!view.comes_from(LogicalSide::Client), "arriving from the client is the bug");

        let rename = registry.get(MessageId::of(22)).expect("TownHallRenameMessage");
        assert_eq!(rename.arrives_at(), Some(LogicalSide::Server));
        assert!(rename.comes_from(LogicalSide::Client));
        assert!(!rename.comes_from(LogicalSide::Server));

        let either = registry.get(MessageId::SPLIT).expect("SplitPacketMessage");
        assert_eq!(either.arrives_at(), None, "Java's null is both sides");
        assert!(either.comes_from(LogicalSide::Client));
        assert!(either.comes_from(LogicalSide::Server));
    }

    #[test]
    fn every_row_has_a_java_file_and_the_two_that_are_not_messages_say_so() {
        let mut not_message = Vec::new();
        for entry in ALL {
            assert!(entry.file.ends_with(".java"), "{} has no file", entry.java);
            assert!(!entry.file.starts_with('/'), "{} is absolute", entry.java);
            if !entry.file.ends_with("Message.java") {
                not_message.push(entry.java);
            }
        }
        not_message.sort_unstable();
        assert_eq!(not_message, vec!["InteractionClose", "InteractionResponse"]);
    }

    #[test]
    fn the_permissions_file_carries_nine_rows_and_only_one_is_a_view() {
        let rows: Vec<_> = ALL.iter().filter(|entry| entry.file == "PermissionsMessage.java").collect();
        assert_eq!(rows.len(), 9, "PermissionsMessage.java:35-723 holds nine classes");
        assert_eq!(rows[0].java, "PermissionsMessage.View");
        assert_eq!(rows[0].rust, "permissions::PermissionsView");
        assert_eq!(rows[0].direction, PayloadDirection::ToClient);
        let both = rows
            .iter()
            .filter(|entry| entry.direction == PayloadDirection::Bidirectional)
            .count();
        assert_eq!(both, 4, "AddRank, ChangePlayerRank, RemoveRank and EditRankType override nothing");
    }

    #[test]
    fn an_id_reads_back_as_a_number_and_displays_as_a_namespaced_name() {
        let id = MessageId::of(42);
        assert_eq!(id.raw(), 42);
        assert_eq!(id.to_string(), "net.message#42");
        assert_eq!(MessageId::SPLIT.raw(), 0);
        assert!(MessageId::of(7) != MessageId::of(8));
    }
}
