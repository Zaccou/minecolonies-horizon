//! The catalogue: one row per message, with the side it runs on, the fields it
//! carries, what it does and which Rust type already holds it.
//!
//! **134 rows for 128 files**, the same arithmetic as
//! the census `network` carries: 125 files with one class, one file with nine nested
//! classes (`PermissionsMessage.java:42-723`), and two abstract bases that are not
//! messages and have no row. `the_rows_add_up_to_the_files` holds this table to
//! `network`'s own census, so the two cannot drift apart in silence.
//!
//! **The four columns that are measurements**, each with the line it came from:
//!
//! | Column | What it is | How it was read |
//! | --- | --- | --- |
//! | `file` | the Java file, relative to `core/network/messages/` | `find … -name '*.java'` |
//! | `line` | the `class` declaration | the file itself |
//! | `side` | where `onExecute` runs | `getExecutionSide`, or the base it inherits |
//! | `fields` | the payload's field names, as Java spells them | the class's own fields |
//!
//! **`fields` is the payload, not the envelope.** Every colony message also carries
//! `dimensionId` and `colonyId`, and every building message a `buildingId`
//! (`AbstractColonyServerMessage.java:26-31,83-89`,
//! `AbstractBuildingServerMessage.java:19,59-67`). Those three are written by the
//! base class and are already [`crate::network::common::envelope`], so repeating them
//! 108 times would say nothing. `-` means the message carries no payload field at all:
//! it exists for its envelope and its side effect (`StopMusicMessage.java:13-46`,
//! `MarkBuildingDirtyMessage.java:15-48`).
//!
//! **Three sides, and the difference matters.** `Side::Client` is a message the
//! client executes; `Side::Server` one the server executes; `Side::Both` one that
//! declares nothing, and `IMessage.getExecutionSide` returns `null` for it
//! (`api/network/IMessage.java:36-44`) — which is not a figure of speech: Forge runs
//! it wherever it lands, and `SplitPacketMessage` relies on that
//! (`SplitPacketMessage.java:131-143`). A `Server` row whose `side_at` is `0` does not
//! declare a side either: it inherits `SERVER` from
//! `AbstractColonyServerMessage.java:112`. `side_at` is the line of the `return`,
//! so a reader can check the claim in one file without a search.
//!
//! **What this table is not.** It is not a codec (that is [`crate::network`]) and not
//! an `onExecute` (that is [`super::effects`], which names the verb and what the layer
//! below must offer). It is the index: which file, which line, which side, which
//! fields, which type — so that a question about one message can be answered without
//! opening 128 files.

use super::effects::Effect;

/// Which side runs a message (`IMessage.getExecutionSide`, `:36-44`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: vanilla's three answers, and `Both` is the default, not a choice
pub enum Side {
    /// `LogicalSide.CLIENT`: the client executes it.
    Client,
    /// `LogicalSide.SERVER`: the server executes it.
    Server,
    /// `null`: no side declared, so it runs wherever it lands.
    Both,
}

impl Side {
    pub fn name(self) -> &'static str {
        match self {
            Side::Client => "client",
            Side::Server => "server",
            Side::Both => "both",
        }
    }
}

/// One message.
pub struct Row {
    /// The Java file, relative to `src/main/java/com/minecolonies/core/network/messages/`.
    pub file: &'static str,
    /// The `class` declaration line in that file.
    pub line: u32,
    /// The class name, without its `Message` suffix (`InteractionClose` has none).
    pub class: &'static str,
    /// Where `onExecute` runs.
    pub side: Side,
    /// The line of the `return LogicalSide.…`, `0` when the class declares none —
    /// see the module note on inheritance and on `Both`.
    pub side_at: u32,
    /// The payload's field names, comma-separated, as Java spells them; `-` for none.
    pub fields: &'static str,
    /// The verb [`super::effects`] names for it.
    pub effect: Effect,
    /// The Rust type of [`crate::network`] that already carries the same fields.
    pub rust: &'static str,
}

macro_rules! row {
    ($file:literal, $line:literal, $class:literal, $side:expr, $side_at:literal, $fields:literal, $effect:expr, $rust:literal) => {
        Row {
            file: $file,
            line: $line,
            class: $class,
            side: $side,
            side_at: $side_at,
            fields: $fields,
            effect: $effect,
            rust: $rust,
        }
    };
}

/// The 134 rows, in the tree's own order: `PermissionsMessage.java` first (it sorts
/// before `client/`), then `client/`, `server/`, `splitting/`.
pub static CATALOGUE: [Row; 134] = [
    // ── PermissionsMessage.java, the one file with nine classes ────────────────
    row!("PermissionsMessage.java", 42, "PermissionsView", Side::Client, 85, "colonyID,data,dimension", Effect::ClientView, "PermissionsView"),
    row!("PermissionsMessage.java", 111, "Permission", Side::Server, 151, "colonyID,enable,rank,action,dimension", Effect::Permissions, "Permission"),
    row!("PermissionsMessage.java", 198, "AddPlayer", Side::Server, 248, "colonyID,playerName,dimension", Effect::Permissions, "AddPlayer"),
    row!("PermissionsMessage.java", 272, "AddRank", Side::Both, 0, "colonyID,rankName,dimension", Effect::Permissions, "AddRank"),
    row!("PermissionsMessage.java", 338, "AddPlayerOrFakePlayer", Side::Server, 393, "colonyID,playerName,id,dimension", Effect::Permissions, "AddPlayerOrFakePlayer"),
    row!("PermissionsMessage.java", 419, "ChangePlayerRank", Side::Both, 0, "colonyID,playerID,rank,dimension", Effect::Permissions, "ChangePlayerRank"),
    row!("PermissionsMessage.java", 495, "RemovePlayer", Side::Server, 545, "colonyID,playerID,dimension", Effect::Permissions, "RemovePlayer"),
    row!("PermissionsMessage.java", 576, "RemoveRank", Side::Both, 0, "colonyID,rankId,dimension", Effect::Permissions, "RemoveRank"),
    row!("PermissionsMessage.java", 642, "EditRankType", Side::Both, 0, "colonyId,rankId,rankType,dimension", Effect::Permissions, "EditRankType"),
    // ── client/ ────────────────────────────────────────────────────────────────
    row!("client/BlockParticleEffectMessage.java", 20, "BlockParticleEffect", Side::Client, 70, "pos,block,side", Effect::ClientBlockParticles, "BlockParticleEffect"),
    row!("client/CircleParticleEffectMessage.java", 20, "CircleParticleEffect", Side::Client, 93, "type,posX,posY,posZ,stage", Effect::ClientParticles, "CircleParticleEffect"),
    row!("client/CompostParticleMessage.java", 20, "CompostParticle", Side::Client, 67, "pos", Effect::ClientParticles, "CompostParticle"),
    row!("client/GlobalQuestSyncMessage.java", 17, "GlobalQuestSync", Side::Client, 60, "questBuffer", Effect::ClientManager, "GlobalQuestSync"),
    row!("client/ItemParticleEffectMessage.java", 22, "ItemParticleEffect", Side::Client, 121, "stack,rotationPitch,rotationYaw,eyeHeight,posX,posY,posZ", Effect::ClientParticles, "ItemParticleEffect"),
    row!("client/LocalizedParticleEffectMessage.java", 22, "LocalizedParticleEffect", Side::Client, 86, "stack,posX,posY,posZ", Effect::ClientParticles, "LocalizedParticleEffect"),
    row!("client/OpenBuildWindowMessage.java", 16, "OpenBuildWindow", Side::Client, 87, "pos,path,packName,rotation,mirror", Effect::OpenScreen, "OpenBuildWindow"),
    row!("client/OpenCantFoundColonyWarningMessage.java", 14, "OpenCantFoundColonyWarning", Side::Both, 0, "townHallPos,warningMessageTranslationKey,displayConfigTooltip", Effect::OpenScreen, "OpenCantFoundColonyWarning"),
    row!("client/OpenColonyFoundingCovenantMessage.java", 12, "OpenColonyFoundingCovenant", Side::Both, 0, "closestName,closestDistance,townHallPos", Effect::OpenScreen, "OpenColonyFoundingCovenant"),
    row!("client/OpenDecoBuildWindowMessage.java", 14, "OpenDecoBuildWindow", Side::Both, 0, "-", Effect::OpenScreen, "OpenDecoBuildWindow"),
    row!("client/OpenDeleteAbandonColonyMessage.java", 12, "OpenDeleteAbandonColony", Side::Both, 0, "currentTownHallPos,oldColonyPos,oldColonyName,oldColonyId", Effect::OpenScreen, "OpenDeleteAbandonColony"),
    row!("client/OpenPlantationFieldBuildWindowMessage.java", 14, "OpenPlantationFieldBuildWindow", Side::Both, 0, "-", Effect::OpenScreen, "OpenPlantationFieldBuildWindow"),
    row!("client/OpenReactivateColonyMessage.java", 12, "OpenReactivateColony", Side::Both, 0, "closestName,closestDistance,townHallPos", Effect::OpenScreen, "OpenReactivateColony"),
    row!("client/OpenSuggestionWindowMessage.java", 19, "OpenSuggestionWindow", Side::Client, 79, "state,pos,stack", Effect::OpenScreen, "OpenSuggestionWindow"),
    row!("client/PlayAudioMessage.java", 27, "PlayAudio", Side::Client, 85, "soundEvent,category", Effect::ClientSound, "PlayAudio"),
    row!("client/SaveStructureNBTMessage.java", 27, "SaveStructureNBT", Side::Client, 97, "compoundNBT,fileName", Effect::ClientFile, "SaveStructureNbt"),
    row!("client/SleepingParticleMessage.java", 16, "SleepingParticle", Side::Client, 57, "x,y,z", Effect::ClientParticles, "SleepingParticle"),
    row!("client/StopMusicMessage.java", 13, "StopMusic", Side::Client, 36, "-", Effect::ClientStopSound, "StopMusic"),
    row!("client/StreamParticleEffectMessage.java", 20, "StreamParticleEffect", Side::Client, 126, "type,sPosX,sPosY,sPosZ,ePosX,ePosY,ePosZ,stage,maxStage", Effect::ClientParticles, "StreamParticleEffect"),
    row!("client/SyncPathMessage.java", 19, "SyncPath", Side::Client, 151, "lastDebugNodesVisited,lastDebugNodesNotVisited,lastDebugNodesPath,debugNodesVisitedLater,debugNodesOrgPath,debugNodesExtra", Effect::ClientDebug, "SyncPath"),
    row!("client/SyncPathReachedMessage.java", 17, "SyncPathReached", Side::Client, 58, "reached", Effect::ClientDebug, "SyncPathReached"),
    row!("client/UpdateChunkCapabilityMessage.java", 23, "UpdateChunkCapability", Side::Client, 69, "chunkCapData", Effect::ClientChunkClaim, "UpdateChunkCapability"),
    row!("client/UpdateChunkRangeCapabilityMessage.java", 27, "UpdateChunkRangeCapability", Side::Client, 96, "caps", Effect::ClientChunkClaim, "UpdateChunkRangeCapability"),
    row!("client/UpdateClientWithCompatibilityMessage.java", 18, "UpdateClientWithCompatibility", Side::Client, 60, "buffer", Effect::ClientManager, "UpdateClientWithCompatibility"),
    row!("client/VanillaParticleMessage.java", 24, "VanillaParticle", Side::Client, 70, "x,y,z,type", Effect::ClientParticles, "VanillaParticle"),
    row!("client/colony/ColonyListMessage.java", 19, "ColonyList", Side::Both, 0, "id,center,name,citizencount,owner,prestige", Effect::ClientMap, "ColonyList"),
    row!("client/colony/ColonyViewAnimalViewDataMessage.java", 26, "ColonyViewAnimalViewData", Side::Client, 106, "colonyId,dimension,animals,animalBuf,refresh", Effect::ClientView, "ColonyViewAnimalViewData"),
    row!("client/colony/ColonyViewBuildingExtensionsUpdateMessage.java", 26, "ColonyViewBuildingExtensionsUpdate", Side::Client, 101, "colonyId,dimension,extensions", Effect::ClientView, "ColonyViewBuildingExtensionsUpdate"),
    row!("client/colony/ColonyViewBuildingViewMessage.java", 21, "ColonyViewBuildingView", Side::Client, 89, "colonyId,buildingId,buildingData,dimension", Effect::ClientView, "ColonyViewBuildingView"),
    row!("client/colony/ColonyViewCitizenViewMessage.java", 21, "ColonyViewCitizenView", Side::Client, 79, "colonyId,citizenId,citizenBuffer,dimension", Effect::ClientView, "ColonyViewCitizenView"),
    row!("client/colony/ColonyViewMessage.java", 22, "ColonyView", Side::Client, 90, "colonyId,isNewSubscription,colonyBuffer,dim", Effect::ClientView, "ColonyView"),
    row!("client/colony/ColonyViewRemoveBuildingMessage.java", 17, "ColonyViewRemoveBuilding", Side::Client, 60, "colonyId,buildingId", Effect::ClientView, "ColonyViewRemoveBuilding"),
    row!("client/colony/ColonyViewRemoveCitizenMessage.java", 16, "ColonyViewRemoveCitizen", Side::Client, 59, "colonyId,citizenId", Effect::ClientView, "ColonyViewRemoveCitizen"),
    row!("client/colony/ColonyViewRemoveMessage.java", 17, "ColonyViewRemove", Side::Client, 51, "id,dimension", Effect::ClientView, "ColonyViewRemove"),
    row!("client/colony/ColonyViewRemoveWorkOrderMessage.java", 16, "ColonyViewRemoveWorkOrder", Side::Client, 60, "colonyId,workOrderId", Effect::ClientView, "ColonyViewRemoveWorkOrder"),
    row!("client/colony/ColonyViewResearchManagerViewMessage.java", 23, "ColonyViewResearchManagerView", Side::Client, 81, "colonyId,researchManagerData,dimension", Effect::ClientView, "ColonyViewResearchManagerView"),
    row!("client/colony/ColonyViewWorkOrderMessage.java", 24, "ColonyViewWorkOrder", Side::Client, 79, "colonyId,dimension,workOrderBuffer", Effect::ClientView, "ColonyViewWorkOrder"),
    row!("client/colony/ColonyVisitorViewDataMessage.java", 25, "ColonyVisitorViewData", Side::Client, 105, "colonyId,dimension,visitors,visitorBuf,refresh", Effect::ClientView, "ColonyVisitorViewData"),
    row!("client/colony/GlobalDiseaseSyncMessage.java", 17, "GlobalDiseaseSync", Side::Client, 59, "buffer", Effect::ClientManager, "GlobalDiseaseSync"),
    row!("client/colony/OpenBuildingUIMessage.java", 22, "OpenBuildingUI", Side::Client, 73, "colonyId,buildingId,dimension", Effect::OpenScreen, "OpenBuildingUI"),
    row!("client/colony/PlayMusicAtPosMessage.java", 23, "PlayMusicAtPos", Side::Client, 97, "soundEvent,pos,dimensionID,volume,pitch", Effect::ClientSound, "PlayMusicAtPos"),
    row!("client/colony/PlaySoundForCitizenMessage.java", 29, "PlaySoundForCitizen", Side::Client, 182, "entityid,soundEvent,soundSource,pos,dimensionID,volume,pitch,length,repetitions", Effect::ClientSound, "PlaySoundForCitizen"),
    row!("server/ClickGuiButtonTriggerMessage.java", 12, "ClickGuiButtonTrigger", Side::Server, 57, "buttonId,resource", Effect::Advancement, "ClickGuiButtonTrigger"),
    row!("server/CreateColonyMessage.java", 36, "CreateColony", Side::Server, 101, "-", Effect::ColonyWrite, "CreateColony"),
    row!("server/DecorationBuildRequestMessage.java", 37, "DecorationBuildRequest", Side::Server, 138, "pos,packName,path,rotation,mirror,dimension,workOrderType,builder", Effect::WorkOrders, "DecorationBuildRequest"),
    row!("server/DirectPlaceMessage.java", 38, "DirectPlace", Side::Server, 108, "state,pos,stack", Effect::BlockWrite, "DirectPlace"),
    row!("server/GetColonyInfoMessage.java", 33, "GetColonyInfo", Side::Server, 66, "-", Effect::OpenScreen, "GetColonyInfo"),
    row!("server/ItemSettingMessage.java", 13, "ItemSetting", Side::Both, 0, "settingName,settingValue", Effect::PlayerItem, "ItemSetting"),
    row!("server/MarkStoryReadOnItemMessage.java", 19, "MarkStoryReadOnItem", Side::Server, 61, "hand", Effect::PlayerItem, "MarkStoryReadOnItem"),
    row!("server/OpenGuiWindowTriggerMessage.java", 12, "OpenGuiWindowTrigger", Side::Server, 49, "resource", Effect::Advancement, "OpenGuiWindowTrigger"),
    row!("server/PickUpTownhallMessage.java", 24, "PickUpTownhall", Side::Server, 57, "-", Effect::BlockWrite, "PickUpTownhall"),
    row!("server/PlantationFieldBuildRequestMessage.java", 37, "PlantationFieldBuildRequest", Side::Server, 146, "pos,packName,path,rotation,mirror,dimension,workOrderType,builder", Effect::WorkOrders, "PlantationFieldBuildRequest"),
    row!("server/PlayerAssistantBuildRequestMessage.java", 17, "PlayerAssistantBuildRequest", Side::Server, 0, "workorderID,interactPos", Effect::BlockWrite, "PlayerAssistantBuildRequest"),
    row!("server/ReactivateBuildingMessage.java", 23, "ReactivateBuilding", Side::Server, 75, "pos", Effect::BuildingWrite, "ReactivateBuilding"),
    row!("server/RemoveFromRallyingListMessage.java", 24, "RemoveFromRallyingList", Side::Server, 75, "banner,location", Effect::PlayerItem, "RemoveFromRallyingList"),
    row!("server/ResourceScrollSaveWarehouseSnapshotMessage.java", 23, "ResourceScrollSaveWarehouseSnapshot", Side::Server, 107, "builderPos,snapshot,workOrderHash", Effect::PlayerItem, "ResourceScrollSaveWarehouseSnapshot"),
    row!("server/SwitchBuildingWithToolMessage.java", 16, "SwitchBuildingWithTool", Side::Server, 68, "stack", Effect::PlayerItem, "SwitchBuildingWithTool"),
    row!("server/SwitchRecipeCraftingTeachingMessage.java", 14, "SwitchRecipeCraftingTeaching", Side::Server, 38, "-", Effect::PlayerContainer, "SwitchRecipeCraftingTeaching"),
    row!("server/ToggleBannerRallyGuardsMessage.java", 22, "ToggleBannerRallyGuards", Side::Server, 64, "banner", Effect::PlayerItem, "ToggleBannerRallyGuards"),
    row!("server/TransferRecipeCraftingTeachingMessage.java", 21, "TransferRecipeCraftingTeaching", Side::Server, 82, "complete,itemStacks", Effect::PlayerContainer, "TransferRecipeCraftingTeaching"),
    row!("server/colony/ChangeFreeToInteractBlockMessage.java", 24, "ChangeFreeToInteractBlock", Side::Server, 0, "pos,block,type,mode", Effect::ColonySetting, "ChangeFreeToInteractBlock"),
    row!("server/colony/ColonyAbandonOwnMessage.java", 18, "ColonyAbandonOwn", Side::Server, 36, "-", Effect::ColonySetting, "ColonyAbandonOwn"),
    row!("server/colony/ColonyDeleteOwnMessage.java", 19, "ColonyDeleteOwn", Side::Server, 37, "-", Effect::ColonyWrite, "ColonyDeleteOwn"),
    row!("server/colony/ColonyFlagChangeMessage.java", 18, "ColonyFlagChange", Side::Server, 0, "patterns", Effect::ColonySetting, "ColonyFlagChange"),
    row!("server/colony/ColonyNameStyleMessage.java", 11, "ColonyNameStyle", Side::Server, 0, "style", Effect::ColonySetting, "ColonyNameStyle"),
    row!("server/colony/ColonyStructureStyleMessage.java", 11, "ColonyStructureStyle", Side::Server, 0, "pack", Effect::ColonySetting, "ColonyStructureStyle"),
    row!("server/colony/ColonyTextureStyleMessage.java", 11, "ColonyTextureStyle", Side::Server, 0, "style", Effect::ColonySetting, "ColonyTextureStyle"),
    row!("server/colony/HireMercenaryMessage.java", 16, "HireMercenary", Side::Server, 0, "-", Effect::SpawnEntity, "HireMercenary"),
    row!("server/colony/HireSpiesMessage.java", 18, "HireSpies", Side::Server, 0, "-", Effect::ColonySetting, "HireSpies"),
    row!("server/colony/InteractionClose.java", 16, "InteractionClose", Side::Server, 0, "citizenId,key", Effect::CitizenWrite, "InteractionClose"),
    row!("server/colony/InteractionResponse.java", 16, "InteractionResponse", Side::Server, 0, "citizenId,key,responseId", Effect::CitizenWrite, "InteractionResponse"),
    row!("server/colony/OpenInventoryMessage.java", 27, "OpenInventory", Side::Server, 0, "name,inventoryType,entityID,tePos", Effect::OpenContainer, "OpenInventory"),
    row!("server/colony/TeamColonyColorChangeMessage.java", 16, "TeamColonyColorChange", Side::Server, 0, "colorOrdinal", Effect::ColonySetting, "TeamColonyColorChange"),
    row!("server/colony/TeleportToColonyMessage.java", 33, "TeleportToColony", Side::Server, 0, "originColonyId,cost,pos", Effect::Teleport, "TeleportToColony"),
    row!("server/colony/TownHallRenameMessage.java", 15, "TownHallRename", Side::Server, 0, "name", Effect::ColonySetting, "TownHallRename"),
    row!("server/colony/TriggerConnectionEventMessage.java", 17, "TriggerConnectionEvent", Side::Server, 0, "connectionEventData,targetColonyId", Effect::ColonySetting, "TriggerConnectionEvent"),
    row!("server/colony/UpdateRequestStateMessage.java", 17, "UpdateRequestState", Side::Server, 0, "itemStack,state", Effect::RequestState, "UpdateRequestState"),
    row!("server/colony/WorkOrderChangeMessage.java", 13, "WorkOrderChange", Side::Server, 0, "workOrderId,priority,removeWorkOrder", Effect::WorkOrders, "WorkOrderChange"),
    row!("server/colony/building/AddMinimumStockToBuildingModuleMessage.java", 16, "AddMinimumStockToBuildingModule", Side::Server, 0, "itemStack,quantity", Effect::ModuleWrite, "AddMinimumStockToBuildingModule"),
    row!("server/colony/building/AlterRestaurantMenuItemMessage.java", 16, "AlterRestaurantMenuItem", Side::Server, 0, "itemStack,id,add", Effect::ModuleWrite, "AlterRestaurantMenuItem"),
    row!("server/colony/building/AssignFilterableEntityMessage.java", 16, "AssignFilterableEntity", Side::Server, 0, "assign,entity,id", Effect::ModuleWrite, "AssignFilterableEntity"),
    row!("server/colony/building/AssignFilterableItemMessage.java", 16, "AssignFilterableItem", Side::Server, 0, "assign,item,id", Effect::ModuleWrite, "AssignFilterableItem"),
    row!("server/colony/building/BuildPickUpMessage.java", 15, "BuildPickUp", Side::Server, 0, "-", Effect::BuildingWrite, "BuildPickUp"),
    row!("server/colony/building/BuildRequestMessage.java", 18, "BuildRequest", Side::Server, 0, "mode,builder", Effect::BuildingWrite, "BuildRequest"),
    row!("server/colony/building/BuildingSetStyleMessage.java", 14, "BuildingSetStyle", Side::Server, 0, "structurePack", Effect::BuildingWrite, "BuildingSetStyle"),
    row!("server/colony/building/ChangeDeliveryPriorityMessage.java", 13, "ChangeDeliveryPriority", Side::Server, 0, "up", Effect::BuildingWrite, "ChangeDeliveryPriority"),
    row!("server/colony/building/CourierHiringModeMessage.java", 16, "CourierHiringMode", Side::Server, 0, "id,mode", Effect::ModuleWrite, "CourierHiringMode"),
    row!("server/colony/building/ForcePickupMessage.java", 21, "ForcePickup", Side::Server, 0, "-", Effect::BuildingWrite, "ForcePickup"),
    row!("server/colony/building/GiveToolMessage.java", 23, "GiveTool", Side::Server, 0, "item", Effect::PlayerItem, "GiveTool"),
    row!("server/colony/building/HireFireMessage.java", 16, "HireFire", Side::Server, 0, "hire,citizenID,moduleId", Effect::ModuleWrite, "HireFire"),
    row!("server/colony/building/HutRenameMessage.java", 14, "HutRename", Side::Server, 0, "name", Effect::BuildingWrite, "HutRename"),
    row!("server/colony/building/MarkBuildingDirtyMessage.java", 15, "MarkBuildingDirty", Side::Server, 0, "-", Effect::BuildingWrite, "MarkBuildingDirty"),
    row!("server/colony/building/OpenCraftingGUIMessage.java", 27, "OpenCraftingGUI", Side::Server, 0, "id", Effect::OpenContainer, "OpenCraftingGUI"),
    row!("server/colony/building/QuarryHiringModeMessage.java", 16, "QuarryHiringMode", Side::Server, 0, "mode,moduleID", Effect::ModuleWrite, "QuarryHiringMode"),
    row!("server/colony/building/RecallCitizenHutMessage.java", 27, "RecallCitizenHut", Side::Server, 0, "-", Effect::Teleport, "RecallCitizenHut"),
    row!("server/colony/building/RemoveMinimumStockFromBuildingModuleMessage.java", 16, "RemoveMinimumStockFromBuildingModule", Side::Server, 0, "moduleId,itemStack", Effect::ModuleWrite, "RemoveMinimumStockFromBuildingModule"),
    row!("server/colony/building/ResetFilterableItemMessage.java", 15, "ResetFilterableItem", Side::Server, 0, "id", Effect::ModuleWrite, "ResetFilterableItem"),
    row!("server/colony/building/TransferItemsRequestMessage.java", 29, "TransferItemsRequest", Side::Server, 0, "itemStack,quantity,attemptResolve", Effect::ModuleWrite, "TransferItemsRequest"),
    row!("server/colony/building/TriggerSettingMessage.java", 24, "TriggerSetting", Side::Server, 0, "key,value,moduleID,buildingPos", Effect::ModuleWrite, "TriggerSetting"),
    row!("server/colony/building/builder/BuilderSelectWorkOrderMessage.java", 11, "BuilderSelectWorkOrder", Side::Server, 0, "workOrder", Effect::WorkOrders, "BuilderSelectWorkOrder"),
    row!("server/colony/building/enchanter/EnchanterWorkerSetMessage.java", 16, "EnchanterWorkerSet", Side::Server, 0, "worker,add", Effect::ModuleWrite, "EnchanterWorkerSet"),
    row!("server/colony/building/fields/AssignFieldMessage.java", 18, "AssignField", Side::Server, 0, "moduleID,fieldData,assign", Effect::ModuleWrite, "AssignField"),
    row!("server/colony/building/fields/AssignmentModeMessage.java", 15, "AssignmentMode", Side::Server, 0, "id,assignmentMode", Effect::ModuleWrite, "AssignmentMode"),
    row!("server/colony/building/fields/FarmFieldPlotResizeMessage.java", 22, "FarmFieldPlotResize", Side::Both, 0, "size,direction,position", Effect::ModuleWrite, "FarmFieldPlotResize"),
    row!("server/colony/building/fields/FarmFieldRegistrationMessage.java", 17, "FarmFieldRegistration", Side::Server, 0, "position", Effect::ModuleWrite, "FarmFieldRegistration"),
    row!("server/colony/building/fields/FarmFieldUpdateSeedMessage.java", 16, "FarmFieldUpdateSeed", Side::Server, 0, "newSeed,position", Effect::ModuleWrite, "FarmFieldUpdateSeed"),
    row!("server/colony/building/guard/GuardSetMinePosMessage.java", 16, "GuardSetMinePos", Side::Server, 0, "minePos,hasMinePos", Effect::ModuleWrite, "GuardSetMinePos"),
    row!("server/colony/building/home/AssignUnassignMessage.java", 20, "AssignUnassign", Side::Server, 0, "assign,citizenID,jobEntry", Effect::ModuleWrite, "AssignUnassign"),
    row!("server/colony/building/miner/MinerRepairLevelMessage.java", 15, "MinerRepairLevel", Side::Server, 0, "level", Effect::ModuleWrite, "MinerRepairLevel"),
    row!("server/colony/building/miner/MinerSetLevelMessage.java", 15, "MinerSetLevel", Side::Server, 0, "level", Effect::ModuleWrite, "MinerSetLevel"),
    row!("server/colony/building/postbox/PostBoxRequestMessage.java", 16, "PostBoxRequest", Side::Server, 0, "itemStack,deliverAvailable,reqQuantity", Effect::RequestState, "PostBoxRequest"),
    row!("server/colony/building/university/TryResearchMessage.java", 18, "TryResearch", Side::Server, 0, "researchId,branch,reset", Effect::ModuleWrite, "TryResearch"),
    row!("server/colony/building/warehouse/SortBuildingMessage.java", 17, "SortBuilding", Side::Server, 0, "-", Effect::BuildingWrite, "SortBuilding"),
    row!("server/colony/building/warehouse/UpgradeWarehouseMessage.java", 18, "UpgradeWarehouse", Side::Server, 0, "-", Effect::BuildingWrite, "UpgradeWarehouse"),
    row!("server/colony/building/worker/AddRemoveRecipeMessage.java", 35, "AddRemoveRecipe", Side::Server, 0, "remove,storage,id", Effect::ModuleWrite, "AddRemoveRecipe"),
    row!("server/colony/building/worker/BuildingHiringModeMessage.java", 16, "BuildingHiringMode", Side::Server, 0, "mode,moduleId", Effect::ModuleWrite, "BuildingHiringMode"),
    row!("server/colony/building/worker/ChangeRecipePriorityMessage.java", 15, "ChangeRecipePriority", Side::Server, 0, "recipeLocation,up,id,fullMove", Effect::ModuleWrite, "ChangeRecipePriority"),
    row!("server/colony/building/worker/RecallCitizenMessage.java", 30, "RecallCitizen", Side::Server, 0, "-", Effect::Teleport, "RecallCitizen"),
    row!("server/colony/building/worker/ToggleRecipeMessage.java", 15, "ToggleRecipe", Side::Server, 0, "recipeLocation,id", Effect::ModuleWrite, "ToggleRecipe"),
    row!("server/colony/citizen/AdjustSkillCitizenMessage.java", 20, "AdjustSkillCitizen", Side::Server, 0, "citizenId,quantity,skill", Effect::CitizenWrite, "AdjustSkillCitizen"),
    row!("server/colony/citizen/PauseCitizenMessage.java", 14, "PauseCitizen", Side::Server, 0, "citizenID", Effect::CitizenWrite, "PauseCitizen"),
    row!("server/colony/citizen/RecallSingleCitizenMessage.java", 24, "RecallSingleCitizen", Side::Server, 0, "citizenId", Effect::Teleport, "RecallSingleCitizen"),
    row!("server/colony/citizen/RestartCitizenMessage.java", 18, "RestartCitizen", Side::Server, 0, "citizenID", Effect::CitizenWrite, "RestartCitizen"),
    row!("server/colony/citizen/TransferItemsToCitizenRequestMessage.java", 26, "TransferItemsToCitizenRequest", Side::Server, 0, "citizenId,itemStack,quantity", Effect::CitizenWrite, "TransferItemsToCitizenRequest"),
    row!("splitting/SplitPacketMessage.java", 21, "SplitPacket", Side::Both, 0, "communicationId,packetIndex,terminator,innerMessageId,payload", Effect::Dispatch, "SplitPacket"),
];

/// The row of a class, by the name this table spells it.
pub fn of_class(class: &str) -> Option<&'static Row> {
    CATALOGUE.iter().find(|row| row.class == class)
}

/// The rows of a file. Nine for `PermissionsMessage.java`, one for the rest.
pub fn of_file(file: &str) -> Vec<&'static Row> {
    CATALOGUE.iter().filter(|row| row.file == file).collect()
}

/// The row that names a Rust type of [`crate::network`].
pub fn of_rust(rust: &str) -> Option<&'static Row> {
    CATALOGUE.iter().find(|row| row.rust == rust)
}

/// Every row that runs on a side.
pub fn on(side: Side) -> Vec<&'static Row> {
    CATALOGUE.iter().filter(|row| row.side == side).collect()
}

/// Every row whose `onExecute` does `effect`.
pub fn doing(effect: Effect) -> Vec<&'static Row> {
    CATALOGUE.iter().filter(|row| row.effect == effect).collect()
}

/// The distinct files the table names, sorted: 126 of them, the tree's 128 less the
/// two abstract bases, which carry no message and have no row.
pub fn files() -> Vec<&'static str> {
    let mut files: Vec<&'static str> = CATALOGUE.iter().map(|row| row.file).collect();
    files.sort_unstable();
    files.dedup();
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network_messages::census::network_types;
    use crate::network_messages::effects;

    /// 125 files with one class, one with nine, and the two abstract bases which are
    /// not messages: `125 + 1 + 2 = 128` files, `125 + 9 = 134` rows.
    #[test]
    fn the_rows_add_up_to_the_files() {
        assert_eq!(CATALOGUE.len(), 134, "125 single-class files plus nine nested");
        assert_eq!(
            files().len(),
            126,
            "126 message files: the tree's 128 less the two abstract bases"
        );
        assert_eq!(of_file("PermissionsMessage.java").len(), 9);
        for file in files() {
            if file != "PermissionsMessage.java" {
                assert_eq!(of_file(file).len(), 1, "{file} has one class");
            }
        }
    }

    /// The two tables are the same table: a file `network` holds a type for is a file
    /// with a row here, and the Rust type is the one `network` names.
    #[test]
    fn every_row_names_a_type_that_network_holds() {
        let types = network_types();
        for row in &CATALOGUE {
            assert!(
                types.contains(&row.rust),
                "{} names {} which network does not have",
                row.class,
                row.rust
            );
        }
    }

    #[test]
    fn every_rust_type_network_holds_has_a_row() {
        for name in network_types() {
            assert!(
                of_rust(name).is_some(),
                "{name} is in network and in no catalogue row"
            );
        }
    }

    #[test]
    fn no_two_rows_claim_the_same_class() {
        for (index, row) in CATALOGUE.iter().enumerate() {
            for other in CATALOGUE.iter().skip(index + 1) {
                assert_ne!(row.class, other.class, "{} is claimed twice", row.class);
            }
        }
    }

    #[test]
    fn the_rows_are_sorted_by_file_then_line() {
        let mut sorted: Vec<(&str, u32)> = CATALOGUE.iter().map(|r| (r.file, r.line)).collect();
        let original = sorted.clone();
        sorted.sort_unstable();
        assert_eq!(original, sorted, "a row is out of the tree's order");
    }

    #[test]
    fn the_sides_are_thirty_six_eighty_four_and_fourteen() {
        // 36 client, 84 server (22 declaring, 62 inheriting SERVER from
        // AbstractColonyServerMessage.java:112), 14 both.
        assert_eq!(on(Side::Client).len(), 36, "36 classes override to CLIENT");
        assert_eq!(on(Side::Server).len(), 84);
        assert_eq!(on(Side::Both).len(), 14, "the rest declare null (both sides)");
        assert_eq!(36 + 84 + 14, 134);
        assert_eq!(
            on(Side::Server).iter().filter(|row| row.side_at == 0).count(),
            62,
            "62 server messages inherit their side instead of declaring it"
        );
    }

    #[test]
    fn a_declared_side_names_the_line_and_an_inherited_one_says_zero() {
        for row in &CATALOGUE {
            match row.side {
                Side::Client => assert!(row.side_at > 0, "{} declares CLIENT where?", row.class),
                Side::Both => assert_eq!(row.side_at, 0, "{} declares a side and Both", row.class),
                // a server message either declares SERVER or inherits it from
                // AbstractColonyServerMessage.java:112, which is `side_at == 0`
                Side::Server => {}
            }
        }
        assert_eq!(
            on(Side::Server).into_iter().filter(|row| row.side_at > 0).count(),
            22,
            "22 server messages declare their side; the other 62 inherit it"
        );
        // the five inherited ones, named so the arithmetic above is checkable
        let inherited: Vec<&str> = on(Side::Server)
            .into_iter()
            .filter(|row| row.side_at == 0)
            .map(|row| row.class)
            .collect();
        assert_eq!(
            inherited,
            vec![
                "PlayerAssistantBuildRequest",
                "ChangeFreeToInteractBlock",
                "ColonyFlagChange",
                "ColonyNameStyle",
                "ColonyStructureStyle",
                "ColonyTextureStyle",
                "HireMercenary",
                "HireSpies",
                "InteractionClose",
                "InteractionResponse",
                "OpenInventory",
                "TeamColonyColorChange",
                "TeleportToColony",
                "TownHallRename",
                "TriggerConnectionEvent",
                "UpdateRequestState",
                "WorkOrderChange",
                "AddMinimumStockToBuildingModule",
                "AlterRestaurantMenuItem",
                "AssignFilterableEntity",
                "AssignFilterableItem",
                "BuildPickUp",
                "BuildRequest",
                "BuildingSetStyle",
                "ChangeDeliveryPriority",
                "CourierHiringMode",
                "ForcePickup",
                "GiveTool",
                "HireFire",
                "HutRename",
                "MarkBuildingDirty",
                "OpenCraftingGUI",
                "QuarryHiringMode",
                "RecallCitizenHut",
                "RemoveMinimumStockFromBuildingModule",
                "ResetFilterableItem",
                "TransferItemsRequest",
                "TriggerSetting",
                "BuilderSelectWorkOrder",
                "EnchanterWorkerSet",
                "AssignField",
                "AssignmentMode",
                "FarmFieldRegistration",
                "FarmFieldUpdateSeed",
                "GuardSetMinePos",
                "AssignUnassign",
                "MinerRepairLevel",
                "MinerSetLevel",
                "PostBoxRequest",
                "TryResearch",
                "SortBuilding",
                "UpgradeWarehouse",
                "AddRemoveRecipe",
                "BuildingHiringMode",
                "ChangeRecipePriority",
                "RecallCitizen",
                "ToggleRecipe",
                "AdjustSkillCitizen",
                "PauseCitizen",
                "RecallSingleCitizen",
                "RestartCitizen",
                "TransferItemsToCitizenRequest",
            ]
        );
    }

    #[test]
    fn a_client_row_does_what_only_a_client_can_do() {
        for row in on(Side::Client) {
            assert!(
                row.effect.is_client(),
                "{} runs on the client and does {}",
                row.class,
                row.effect.name()
            );
        }
    }

    #[test]
    fn a_server_row_that_is_not_dispatch_does_what_only_a_server_can_do() {
        for row in on(Side::Server) {
            assert!(
                !row.effect.is_client() || row.effect == Effect::OpenScreen,
                "{} runs on the server and does {}",
                row.class,
                row.effect.name()
            );
        }
    }

    /// `GetColonyInfoMessage` is the one server message whose act is on the client: it
    /// answers the player with one of the four founding windows
    /// (`GetColonyInfoMessage.java:88,98,102,106,110,123`).
    #[test]
    fn the_server_that_opens_a_window_is_get_colony_info_alone() {
        let openers: Vec<&str> = doing(Effect::OpenScreen)
            .into_iter()
            .filter(|row| row.side == Side::Server)
            .map(|row| row.class)
            .collect();
        assert_eq!(openers, vec!["GetColonyInfo"]);
    }

    #[test]
    fn every_effect_of_the_vocabulary_is_used_by_a_row() {
        for effect in Effect::ALL {
            assert!(
                !doing(effect).is_empty(),
                "{} is a verb no message uses",
                effect.name()
            );
        }
    }

    #[test]
    fn the_client_rows_are_the_thirty_six_readers_counted_by_kind() {
        let views = doing(Effect::ClientView).len();
        assert_eq!(views, 13, "the colony view, its eleven companions and the permission pane");
        assert_eq!(doing(Effect::ClientParticles).len(), 7, "seven spawn a particle type");
        assert_eq!(
            doing(Effect::ClientBlockParticles).len(),
            1,
            "one asks for a block state's own break particles"
        );
        assert_eq!(doing(Effect::ClientSound).len(), 3, "two jingles and a citizen voice");
        assert_eq!(doing(Effect::ClientStopSound).len(), 1, "the one stop-all");
        assert_eq!(doing(Effect::ClientDebug).len(), 2, "the two path debug messages");
        assert_eq!(doing(Effect::ClientChunkClaim).len(), 2, "one chunk, one range");
        assert_eq!(doing(Effect::ClientManager).len(), 3, "compatibility, quests, diseases");
        assert_eq!(doing(Effect::ClientFile).len(), 1, "the saved schematic");
        assert_eq!(doing(Effect::ClientMap).len(), 1, "the colony list");
    }

    #[test]
    fn the_empty_payloads_are_the_eighteen_that_say_so() {
        let empty: Vec<&str> = CATALOGUE
            .iter()
            .filter(|row| row.fields == "-")
            .map(|row| row.class)
            .collect();
        assert_eq!(
            empty,
            vec![
                "OpenDecoBuildWindow",
                "OpenPlantationFieldBuildWindow",
                "StopMusic",
                "CreateColony",
                "GetColonyInfo",
                "PickUpTownhall",
                "SwitchRecipeCraftingTeaching",
                "ColonyAbandonOwn",
                "ColonyDeleteOwn",
                "HireMercenary",
                "HireSpies",
                "BuildPickUp",
                "ForcePickup",
                "MarkBuildingDirty",
                "RecallCitizenHut",
                "SortBuilding",
                "UpgradeWarehouse",
                "RecallCitizen",
            ],
            "a row with no field is a message carried by its envelope alone"
        );
    }

    #[test]
    fn a_row_that_says_what_it_does_also_says_which_type_carries_it() {
        for row in &CATALOGUE {
            assert!(!row.rust.is_empty(), "{} has no Rust type", row.class);
            assert!(row.line > 0, "{} has no line", row.class);
            assert!(row.file.ends_with(".java"), "{} is filed under {}", row.class, row.file);
        }
    }

    #[test]
    fn the_secondary_effects_name_a_class_the_catalogue_holds() {
        for secondary in effects::SECONDARY.iter() {
            assert!(
                of_class(secondary.class).is_some(),
                "{} is a secondary effect of a message that does not exist",
                secondary.class
            );
        }
    }
}
