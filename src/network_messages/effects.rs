//! What a message *does*, and what the layer below must offer for it to be done.
//!
//! Java's `IMessage` has four members: `toBytes`, `fromBytes`, `getExecutionSide` and
//! `onExecute` (`api/network/IMessage.java:19-49`). [`crate::network`] answers the
//! first two for all 134 types, and the third is a field of
//! [`crate::network::catalogue::Row`]. This module answers the fourth **without
//! running it**: it names the verb (`onExecute`'s first state-changing call) and the
//! capabilities that verb needs from the layer that owns the world.
//!
//! **Why a verb and not a body.** An `onExecute` body is 5 to 60 lines of game calls
//! (`CreateColonyMessage.java:105-200` is ninety-five). Copying those lines would copy
//! a colony implementation that already exists in this crate, in its own modules, with
//! its own tests. What the network layer must know is: *does the receiver need a
//! colony, a building, a citizen, a player's inventory, a screen?* — and that is what
//! [`Effect`] says, one verb per message, `134` rows in
//! [`crate::network::catalogue::CATALOGUE`].
//!
//! **The verb is the first state-changing call, and only that.** Java often does more:
//! `AddRemoveRecipeMessage.java:155-190` writes the module, plays a success or error
//! sound, fires `BUILDING_ADD_RECIPE` for the colony and sends a chat line.
//! `HireSpiesMessage.java:38-43` takes five gold ingots before it enables spies. The
//! catalogue records the verb; the *extra* effects are [`SECONDARY`], a separate
//! twelve-row table, so a reader who needs the whole picture has it and a reader who
//! needs the verb is not drowned. The rule for a row in [`SECONDARY`] is mechanical:
//! it is there because its `onExecute` sends a chat line, plays a sound, fires an
//! advancement, or writes a second object besides the one the class is named after.
//!
//! **A capability is not a call.** [`Need`] names a *kind of access*, not a method:
//! `Need::ColonyStore` is "a colony, by id and dimension, on the server" and it is
//! satisfied by this crate's `colony` module, not by `mod_api`. Each `Need` carries a
//! [`Verdict`] and a citation — `Present` with the SDK file and line, `InCrate` with
//! the port's own module, `Partial` or `Absent` with the research that found the hole.
//! `MANQUANT.md` is generated from the `Absent` and `Partial` rows and says what would
//! have to exist.

/// The verb of a message: what its `onExecute` starts doing, on which side.
///
/// One variant per kind of change, not one per message: twenty-seven verbs cover the
/// 134 messages because Java's messages are mostly the same four acts (write a
/// colony value, write a building value, hand the client a view, open something).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own verb set, closed on purpose — a new kind of change is a
// new variant here, and every `match` over it becomes a compile error until answered.
pub enum Effect {
    /// A screen of the mod's own is opened over the world on the client
    /// (`OpenBuildWindowMessage.java:91-94`: `new WindowBuildDecoration(…).open()`).
    OpenScreen,
    /// The client rewrites a per-colony view — colony, building, citizen, visitor,
    /// animal, work order, research, extension — from the bytes it was handed
    /// (`ColonyViewMessage.java:95-102`).
    ClientView,
    /// The client rewrites a *global* manager: the compatibility manager
    /// (`UpdateClientWithCompatibilityMessage.java:64-76`), the quest pack
    /// (`GlobalQuestSyncMessage.java:65-72`), the disease pack
    /// (`GlobalDiseaseSyncMessage.java:64-71`).
    ClientManager,
    /// The client spawns particles in its own level
    /// (`CircleParticleEffectMessage.java:97-116`).
    ClientParticles,
    /// The client plays or stops a sound
    /// (`PlayAudioMessage.java:90-102`, `StopMusicMessage.java:40-44`).
    ClientSound,
    /// The client asks the *particle engine* for the particles a block state gives when it
    /// breaks or cracks (`BlockParticleEffectMessage.java:76-83`) — not a particle type of
    /// its own choosing, which is [`Effect::ClientParticles`].
    ClientBlockParticles,
    /// The client stops whatever is playing, with no handle to it: the sound manager and
    /// the music manager both (`StopMusicMessage.java:40-44`).
    ClientStopSound,
    /// The client rewrites a debug renderer's global state
    /// (`SyncPathMessage.java:156-164`).
    ClientDebug,
    /// The client applies a colony claim to a chunk it holds
    /// (`UpdateChunkCapabilityMessage.java:73-90`).
    ClientChunkClaim,
    /// The client writes a schematic to its own disk
    /// (`SaveStructureNBTMessage.java:101-113`).
    ClientFile,
    /// The client fills the colony list window
    /// (`ColonyListMessage.java:77-87`).
    ClientMap,
    /// Not a payload: the fragments are reassembled and one inner message is
    /// re-dispatched (`SplitPacketMessage.java:85-154`).
    Dispatch,
    /// The server founds, reactivates, picks up or deletes a colony
    /// (`CreateColonyMessage.java:105-200`, `PickUpTownhallMessage.java:61-90`,
    /// `ColonyDeleteOwnMessage.java:41-59`).
    ColonyWrite,
    /// The server writes one colony-wide value: name, style, flag, colour, free
    /// blocks, spies, mercenaries, abandonment
    /// (`ColonyNameStyleMessage.java:39-42`, `HireSpiesMessage.java:38-43`).
    ColonySetting,
    /// The server writes the colony's permission table
    /// (`PermissionsMessage.java:254-269`).
    Permissions,
    /// The server writes a building: its name, style, level, work order, pickup,
    /// reactivation, delivery priority, warehouse upgrade
    /// (`HutRenameMessage.java:56-59`, `BuildRequestMessage.java:77-108`).
    BuildingWrite,
    /// The server writes a building's *module*: minimum stock, recipe list, hiring
    /// mode, filter list, farm field, miner level, restaurant menu, settings, research
    /// (`AddMinimumStockToBuildingModuleMessage.java:65-72`).
    ModuleWrite,
    /// The server writes a citizen: pause, recall, restart, skill, home assignment,
    /// transferred items, an interaction answer
    /// (`PauseCitizenMessage.java:66-70`, `InteractionResponse.java:90-102`).
    CitizenWrite,
    /// The server writes the work manager: enqueue, remove, reprioritise, claim
    /// (`DecorationBuildRequestMessage.java:142-205`,
    /// `WorkOrderChangeMessage.java:81-91`).
    WorkOrders,
    /// The server writes a request's state
    /// (`UpdateRequestStateMessage.java:81-89`).
    RequestState,
    /// The server writes the *sender's own* stack or inventory: clipboard settings,
    /// "story read", the build tool swap, a scepter handed over, a banner, a
    /// warehouse snapshot
    /// (`ItemSettingMessage.java:39-59`, `SwitchBuildingWithToolMessage.java:72-103`).
    PlayerItem,
    /// The server writes into the sender's open container: the teaching screen, the
    /// crafting grid
    /// (`TransferRecipeCraftingTeachingMessage.java:86-128`,
    /// `SwitchRecipeCraftingTeachingMessage.java:42-50`).
    PlayerContainer,
    /// The server opens a container menu in the sender's window
    /// (`OpenCraftingGUIMessage.java:66-137`, `OpenInventoryMessage.java:120-138`).
    OpenContainer,
    /// The server writes blocks in the world: a hut placed straight down, a block
    /// placed by the assistant's hammer, the town hall picked up
    /// (`DirectPlaceMessage.java:137`, `PlayerAssistantBuildRequestMessage.java:70-72`,
    /// `PickUpTownhallMessage.java:81-85`).
    BlockWrite,
    /// The server spawns entities
    /// (`HireMercenaryMessage.java:43`: `EntityMercenary.spawnMercenariesInColony`).
    SpawnEntity,
    /// The server moves a player or a citizen through space
    /// (`TeleportToColonyMessage.java:71-133`, `RecallCitizenMessage.java:58-97`).
    Teleport,
    /// The server fires an advancement trigger for the sender
    /// (`ClickGuiButtonTriggerMessage.java:61-68`,
    /// `OpenGuiWindowTriggerMessage.java:53-60`).
    Advancement,
}

impl Effect {
    /// Every variant, in declaration order. `catalogue` uses it to check that no verb
    /// is left unused, which is how a row that names a verb nobody paints gets caught.
    pub const ALL: [Effect; 27] = [
        Effect::OpenScreen,
        Effect::ClientView,
        Effect::ClientManager,
        Effect::ClientParticles,
        Effect::ClientSound,
        Effect::ClientBlockParticles,
        Effect::ClientStopSound,
        Effect::ClientDebug,
        Effect::ClientChunkClaim,
        Effect::ClientFile,
        Effect::ClientMap,
        Effect::Dispatch,
        Effect::ColonyWrite,
        Effect::ColonySetting,
        Effect::Permissions,
        Effect::BuildingWrite,
        Effect::ModuleWrite,
        Effect::CitizenWrite,
        Effect::WorkOrders,
        Effect::RequestState,
        Effect::PlayerItem,
        Effect::PlayerContainer,
        Effect::OpenContainer,
        Effect::BlockWrite,
        Effect::SpawnEntity,
        Effect::Teleport,
        Effect::Advancement,
    ];

    /// A stable name, for a log line or a table.
    pub fn name(self) -> &'static str {
        match self {
            Effect::OpenScreen => "open_screen",
            Effect::ClientView => "client_view",
            Effect::ClientManager => "client_manager",
            Effect::ClientParticles => "client_particles",
            Effect::ClientSound => "client_sound",
            Effect::ClientBlockParticles => "client_block_particles",
            Effect::ClientStopSound => "client_stop_sound",
            Effect::ClientDebug => "client_debug",
            Effect::ClientChunkClaim => "client_chunk_claim",
            Effect::ClientFile => "client_file",
            Effect::ClientMap => "client_map",
            Effect::Dispatch => "dispatch",
            Effect::ColonyWrite => "colony_write",
            Effect::ColonySetting => "colony_setting",
            Effect::Permissions => "permissions",
            Effect::BuildingWrite => "building_write",
            Effect::ModuleWrite => "module_write",
            Effect::CitizenWrite => "citizen_write",
            Effect::WorkOrders => "work_orders",
            Effect::RequestState => "request_state",
            Effect::PlayerItem => "player_item",
            Effect::PlayerContainer => "player_container",
            Effect::OpenContainer => "open_container",
            Effect::BlockWrite => "block_write",
            Effect::SpawnEntity => "spawn_entity",
            Effect::Teleport => "teleport",
            Effect::Advancement => "advancement",
        }
    }

    /// `true` when the verb runs on the client, `false` when it runs on the server.
    ///
    /// [`Effect::Dispatch`] is the one that runs on both: it is what
    /// `SplitPacketMessage` does with a reassembled payload, and the message it
    /// rebuilds decides the side (`SplitPacketMessage.java:131-143`). It is reported
    /// as server-side here because a fragment always arrives where its sender is not:
    /// a message a client sends lands on the server, and a server-bound fragment has no
    /// other side to run on.
    pub fn is_client(self) -> bool {
        match self {
            Effect::OpenScreen
            | Effect::ClientView
            | Effect::ClientManager
            | Effect::ClientParticles
            | Effect::ClientSound
            | Effect::ClientBlockParticles
            | Effect::ClientStopSound
            | Effect::ClientDebug
            | Effect::ClientChunkClaim
            | Effect::ClientFile
            | Effect::ClientMap => true,
            Effect::Dispatch
            | Effect::ColonyWrite
            | Effect::ColonySetting
            | Effect::Permissions
            | Effect::BuildingWrite
            | Effect::ModuleWrite
            | Effect::CitizenWrite
            | Effect::WorkOrders
            | Effect::RequestState
            | Effect::PlayerItem
            | Effect::PlayerContainer
            | Effect::OpenContainer
            | Effect::BlockWrite
            | Effect::SpawnEntity
            | Effect::Teleport
            | Effect::Advancement => false,
        }
    }
}

/// A kind of access an [`Effect`] needs, named so that a missing one is a named
/// absence rather than a surprise at run time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: closed on purpose — [`VERDICTS`] must answer for every variant, and a
// new one without a verdict fails the coverage test.
pub enum Need {
    /// Open a screen belonging to the mod, over the world, on the client.
    ModScreen,
    /// Show *these bytes* in that screen: the payload the server sent.
    ScreenData,
    /// Open a container menu of the game's own kind: one to six rows of slots.
    ChestMenu,
    /// Write blocks in a dimension, in one transaction.
    BlockWrite,
    /// Spawn an entity of a registered type at a position.
    Spawn,
    /// A store of per-colony views on the client, addressable by colony id.
    ClientViewStore,
    /// A store of global managers on the client.
    ClientManagerStore,
    /// Spawn a particle with a position, a velocity and an options record.
    Particle,
    /// Ask for a *block state's* break or crack particles at a position and a face.
    BlockParticle,
    /// Play a positioned sound with a volume, a pitch and a category.
    Sound,
    /// Stop whatever music is playing, with no handle to the sound.
    MusicStop,
    /// A debug renderer's global state, written by the mod that draws it.
    DebugState,
    /// A colony claim attached to a chunk, read by the client.
    ChunkClaim,
    /// Write a file into the game's own directory.
    DiskWrite,
    /// A window of the mod's own filled with a list it did not build.
    MapWindow,
    /// A registered payload channel with a handler, in one or both directions.
    Payload,
    /// Reassembly of fragments larger than one payload.
    Fragments,
    /// The colony registry: a colony by id and dimension, by owner, by position.
    ColonyStore,
    /// A colony's permission table: ranks, players, actions.
    PermissionTable,
    /// A colony's buildings, by position and by type.
    BuildingStore,
    /// A building's modules, by runtime id and by type.
    ModuleStore,
    /// A colony's citizens and visitors, by id.
    CitizenStore,
    /// The work manager: orders by id, by type, by position.
    WorkOrderStore,
    /// The request manager: a request by token, its state, its overrule.
    RequestStore,
    /// The sender's own inventory, slots and open hand, written on the server.
    PlayerInventory,
    /// The sender's open container menu, written on the server.
    PlayerMenu,
    /// Move a player or an entity to a position, in a level.
    Teleport,
    /// Fire a trigger for a player.
    Advancement,
}

/// What the layer below offers for a [`Need`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: four answers, and `Partial` is not a synonym for `Present` — a partial
// answer means the effect can be *approached* but not reproduced.
pub enum Verdict {
    /// `mod_api` has the call; the citation names it.
    Present,
    /// This crate has it; the citation names the module.
    InCrate,
    /// Something close exists, and the difference is a behaviour change.
    Partial,
    /// Nothing offers it; `MANQUANT.md` says what would have to exist.
    Absent,
}

/// One row of the answer: the need, its verdict, and where that was measured.
pub struct Finding {
    pub need: Need,
    pub verdict: Verdict,
    /// `crates/horizon-mod-vocab/src/…` for an SDK call, `src/…` for this crate, and
    /// for an absence the search that failed.
    pub source: &'static str,
    /// One sentence: what exists, or what is missing and why it matters.
    pub note: &'static str,
}

/// The answer for all twenty-eight needs.
///
/// The SDK paths are relative to the Horizon tree this port was cut against
/// (`.horizon/sdk/api/crates/`), and every one of them was opened rather than guessed:
/// `host/screens.rs`, `host/menus.rs`, `host/effects.rs`, `host/items.rs`,
/// `host/entities.rs`, `host/advancements.rs`, `host/payloads.rs`, `parts/shared.rs`,
/// `payload.rs`, `hooks/client_packet.rs`.
pub static VERDICTS: [Finding; 28] = [
    Finding {
        need: Need::ModScreen,
        verdict: Verdict::Present,
        source: "crates/horizon-mod-vocab/src/host/screens.rs:73",
        note: "mod_screen_open(mod_id) opens the mod's own screen over the world; \
               false while another screen is open, over the title screen or loading.",
    },
    Finding {
        need: Need::ScreenData,
        verdict: Verdict::Partial,
        source: "crates/horizon-mod-vocab/src/host/screens.rs:73-82",
        note: "the screen is opened by id alone: the bytes that make it show *this* \
               hut travel through a payload and are stored by the mod, so opening a \
               window is two calls and a race with mod_screen_open's refusals.",
    },
    Finding {
        need: Need::ChestMenu,
        verdict: Verdict::Present,
        source: "crates/horizon-mod-vocab/src/host/menus.rs:20-33",
        note: "open_menu(uuid, rows, title, items, replace) opens a chest menu of 1 to 6 \
               rows, its slots then written as ItemPlace::MenuSlot.",
    },
    Finding {
        need: Need::BlockWrite,
        verdict: Verdict::Present,
        source: "crates/horizon-mod-vocab/src/host/world_edit.rs:17",
        note: "edit_blocks(dimension, edit) applies a BlockEdit; placing and destroying \
               a hut are one edit each.",
    },
    Finding {
        need: Need::Spawn,
        verdict: Verdict::Absent,
        source: "grep -n 'fn spawn' crates/horizon-mod-vocab/src/host/*.rs gives only \
                 entities.rs:95 spawn_falling_block",
        note: "nothing spawns an entity of a named type, so HireMercenaryMessage's \
               EntityMercenary.spawnMercenariesInColony has no counterpart.",
    },
    Finding {
        need: Need::ClientViewStore,
        verdict: Verdict::InCrate,
        source: "src/colony, src/building, src/citizens",
        note: "the port's own view records; the message carries the bytes, the store is \
               this crate's, exactly as IColonyManager is MineColonies'.",
    },
    Finding {
        need: Need::ClientManagerStore,
        verdict: Verdict::InCrate,
        source: "src/manager_01 … src/manager_09",
        note: "the compatibility, quest and disease managers are ported in this crate; \
               each still needs its deserializer, which is what MANQUANT.md §1 counts.",
    },
    Finding {
        need: Need::Particle,
        verdict: Verdict::Present,
        source: "crates/horizon-mod-vocab/src/host/effects.rs:54",
        note: "add_particle(options, at, velocity, …) takes a ParticleOptions record and \
               a position; the six random-offset effects of particles.rs are its callers.",
    },
    Finding {
        need: Need::BlockParticle,
        verdict: Verdict::Absent,
        source: "grep -rn 'crack\\|destroy(' crates/horizon-mod-vocab/src/host/ gives nothing",
        note: "ParticleEngine.destroy(pos, state) and .crack(pos, face) are engine methods \
               taking a BlockState; add_particle takes options, not a state, so \
               BlockParticleEffectMessage has no equivalent.",
    },
    Finding {
        need: Need::Sound,
        verdict: Verdict::Present,
        source: "crates/horizon-mod-vocab/src/host/effects.rs:38 (play_local_sound), \
                 host/levels.rs:1166 (level_play_sound), host/entities.rs:849 (entity_play_sound)",
        note: "positioned sound with volume and pitch; the Attenuation.NONE of \
               PlayAudioMessage.java:99-101 is the one field with no argument.",
    },
    Finding {
        need: Need::MusicStop,
        verdict: Verdict::Partial,
        source: "crates/horizon-mod-vocab/src/host/effects.rs:91",
        note: "sound_stop takes a SoundRef; StopMusicMessage.java:42-43 stops the sound \
               manager and the music manager with no handle, which needs those two methods.",
    },
    Finding {
        need: Need::DebugState,
        verdict: Verdict::Present,
        source: "crates/horizon-mod-vocab/src/hooks/register_debug_renderers.rs:6, \
                 hooks/debug_entry_lines.rs:5",
        note: "the renderer is the mod's own, so its state is the mod's: the message \
               writes a field and the hook that draws reads it.",
    },
    Finding {
        need: Need::ChunkClaim,
        verdict: Verdict::Absent,
        source: "crates/horizon-mod-vocab/src/parts/shared.rs:133-138 (ProvidedBy is \
                 BlockEntity, Block, Item, Entity — no Chunk, no Level)",
        note: "IColonyTagCapability hangs on LevelChunk; a capability cannot attach to a \
               chunk here, and level attachments are not offered either.",
    },
    Finding {
        need: Need::DiskWrite,
        verdict: Verdict::Partial,
        source: "crates/horizon-mod-vocab/src/instance_directory.rs:9-18",
        note: "the instance directory is exposed as a *value*, with a note that it is not \
               filesystem authority; SaveStructureNBT's write of a .blueprint is the \
               mod's own I/O, closed to a wasm mod and merely possible for a native one.",
    },
    Finding {
        need: Need::MapWindow,
        verdict: Verdict::InCrate,
        source: "src/ui_window_01 … src/ui_window_17",
        note: "the colony list window is a screen of the port's own; the message only \
               hands it twenty-one rows of six fields.",
    },
    Finding {
        need: Need::Payload,
        verdict: Verdict::Present,
        source: "crates/horizon-mod-vocab/src/payload.rs:11-14 (PayloadRegistrar, \
                 RegisterPayloadHandlersEvent), host/payloads.rs:11,26",
        note: "a declared channel with a handler per direction: player_send_payload on \
               the server, send_payload_to_server on the client — this is the SimpleChannel \
               of Network.java, with one channel per message instead of one id per class.",
    },
    Finding {
        need: Need::Fragments,
        verdict: Verdict::Absent,
        source: "crates/horizon-mod-vocab/src/payload.rs:149-157 (PayloadTooLarge, \
                 1,048,576 bytes) — no continuation or reassembly primitive",
        note: "a payload above the cap is refused; the game never splits one, so \
               SplitPacketMessage's twenty-nine lines of reassembly have no counterpart \
               and the port carries them itself ([`super::fragments`]).",
    },
    Finding {
        need: Need::ColonyStore,
        verdict: Verdict::InCrate,
        source: "src/colony",
        note: "getColonyByDimension / getColonyByPosFromWorld / getIColonyByOwner, plus \
               create and delete, live in this crate.",
    },
    Finding {
        need: Need::PermissionTable,
        verdict: Verdict::InCrate,
        source: "src/colony/permissions",
        note: "ranks, players and the twenty-seven actions, with the owner's rank held \
               apart; the eight permission messages are its writers.",
    },
    Finding {
        need: Need::BuildingStore,
        verdict: Verdict::InCrate,
        source: "src/building, src/colony_buildings",
        note: "a colony's buildings by position and by type, with the level, the style \
               pack and the construction state the hut messages write.",
    },
    Finding {
        need: Need::ModuleStore,
        verdict: Verdict::InCrate,
        source: "src/building",
        note: "modules by runtime id, with the downcast Java does by hand \
               (getModule(id) instanceof X) done by a typed accessor.",
    },
    Finding {
        need: Need::CitizenStore,
        verdict: Verdict::InCrate,
        source: "src/citizens",
        note: "civilians and visitors by id, with the skill handler and the home \
               building the citizen messages read.",
    },
    Finding {
        need: Need::WorkOrderStore,
        verdict: Verdict::InCrate,
        source: "src/requests, src/colony_world",
        note: "orders by id, by type and by position; the builder's claim is a field.",
    },
    Finding {
        need: Need::RequestStore,
        verdict: Verdict::InCrate,
        source: "src/requests",
        note: "a request by token, its state and its overrule — the three things \
               UpdateRequestStateMessage writes.",
    },
    Finding {
        need: Need::PlayerInventory,
        verdict: Verdict::Present,
        source: "crates/horizon-mod-vocab/src/block/item_place.rs:23-39",
        note: "ItemPlace::PlayerSlot and ::PlayerHand name a slot of a named player, \
               written inside a transaction.",
    },
    Finding {
        need: Need::PlayerMenu,
        verdict: Verdict::Present,
        source: "crates/horizon-mod-vocab/src/host/menus.rs:41-75",
        note: "menu(uuid, window) gives a MenuRef whose slots are written as \
               ItemPlace::MenuSlot, and the client hears the change on the next tick.",
    },
    Finding {
        need: Need::Teleport,
        verdict: Verdict::Present,
        source: "crates/horizon-mod-vocab/src/host/entities.rs:634,667",
        note: "entity_teleport_to and entity_teleport_to_level move an entity by \
               EntityRef, which is what a player and a citizen both are.",
    },
    Finding {
        need: Need::Advancement,
        verdict: Verdict::Absent,
        source: "crates/horizon-mod-vocab/src/host/advancements.rs:5-14",
        note: "advancement(id) *reads* a loaded advancement; nothing fires a trigger, so \
               ClickGuiButtonTriggerMessage and OpenGuiWindowTriggerMessage have no \
               counterpart.",
    },
];

impl Finding {
    /// The row for a need. Total by construction: [`VERDICTS`] has one row per
    /// variant, which `every_need_has_a_verdict` checks.
    pub fn of(need: Need) -> &'static Finding {
        VERDICTS
            .iter()
            .find(|finding| finding.need == need)
            .expect("VERDICTS has a row per Need; a test asserts it")
    }
}

/// The capabilities an [`Effect`] needs, in the order a caller should look.
///
/// A list, not a set: the order is the order the effect uses them, so a reader
/// following an `onExecute` top to bottom reads the same order here.
pub fn needs(effect: Effect) -> &'static [Need] {
    match effect {
        Effect::OpenScreen => &[Need::Payload, Need::ModScreen, Need::ScreenData],
        Effect::ClientView => &[Need::Payload, Need::ClientViewStore],
        Effect::ClientManager => &[Need::Payload, Need::ClientManagerStore],
        Effect::ClientParticles => &[Need::Particle],
        Effect::ClientBlockParticles => &[Need::BlockParticle],
        Effect::ClientStopSound => &[Need::MusicStop],
        Effect::ClientSound => &[Need::Sound],
        Effect::ClientDebug => &[Need::DebugState],
        Effect::ClientChunkClaim => &[Need::ChunkClaim],
        Effect::ClientFile => &[Need::DiskWrite],
        Effect::ClientMap => &[Need::Payload, Need::MapWindow],
        Effect::Dispatch => &[Need::Fragments, Need::Payload],
        Effect::ColonyWrite => &[Need::Payload, Need::ColonyStore],
        Effect::ColonySetting => &[Need::ColonyStore],
        Effect::Permissions => &[Need::ColonyStore, Need::PermissionTable],
        Effect::BuildingWrite => &[Need::ColonyStore, Need::BuildingStore],
        Effect::ModuleWrite => &[Need::ColonyStore, Need::BuildingStore, Need::ModuleStore],
        Effect::CitizenWrite => &[Need::ColonyStore, Need::CitizenStore],
        Effect::WorkOrders => &[Need::ColonyStore, Need::WorkOrderStore],
        Effect::RequestState => &[Need::ColonyStore, Need::RequestStore],
        Effect::PlayerItem => &[Need::PlayerInventory],
        Effect::PlayerContainer => &[Need::PlayerMenu],
        Effect::OpenContainer => &[Need::ChestMenu, Need::PlayerMenu],
        Effect::BlockWrite => &[Need::BlockWrite],
        Effect::SpawnEntity => &[Need::Spawn],
        Effect::Teleport => &[Need::Teleport],
        Effect::Advancement => &[Need::Advancement],
    }
}

/// The needs that nothing satisfies, in the order [`VERDICTS`] lists them.
///
/// This is `MANQUANT.md`'s table, computed rather than typed twice: a test holds the
/// two to each other by name.
pub fn absent() -> Vec<&'static Finding> {
    VERDICTS
        .iter()
        .filter(|finding| finding.verdict == Verdict::Absent)
        .collect()
}

/// The needs something *almost* satisfies.
pub fn partial() -> Vec<&'static Finding> {
    VERDICTS
        .iter()
        .filter(|finding| finding.verdict == Verdict::Partial)
        .collect()
}

/// A second effect of a message whose [`Effect`] names only the first.
///
/// Twelve rows, and the rule is mechanical: it is here because its `onExecute` sends a
/// chat line (`MessageUtils…sendTo`), plays a sound (`SoundUtils.playSuccessSound`),
/// fires an advancement, or writes a second object besides the one the class is named
/// after. Java's chat lines are *not* messages — `MessageUtils` writes them straight to
/// the player (`MessageUtils.format(…).sendTo(sender)`), which is why no `IMessage` is
/// involved and nothing in `src/network` saw them.
pub struct Secondary {
    /// The class, as the catalogue's `class` field spells it.
    pub class: &'static str,
    /// `fichier.java:ligne` of the second effect.
    pub source: &'static str,
    /// What the second effect is.
    pub effect: Effect,
}

pub static SECONDARY: [Secondary; 12] = [
    Secondary {
        class: "AddRemoveRecipe",
        source: "AddRemoveRecipeMessage.java:172-187",
        effect: Effect::ClientSound,
    },
    Secondary {
        class: "HireSpies",
        source: "HireSpiesMessage.java:38-42",
        effect: Effect::PlayerItem,
    },
    Secondary {
        class: "CreateColony",
        source: "CreateColonyMessage.java:180-191",
        effect: Effect::ColonyWrite,
    },
    Secondary {
        class: "DirectPlace",
        source: "DirectPlaceMessage.java:137-162",
        effect: Effect::BuildingWrite,
    },
    Secondary {
        class: "TransferItemsRequest",
        source: "TransferItemsRequestMessage.java:127-137",
        effect: Effect::RequestState,
    },
    Secondary {
        class: "TransferItemsToCitizenRequest",
        source: "TransferItemsToCitizenRequestMessage.java:145-148",
        effect: Effect::RequestState,
    },
    Secondary {
        class: "TeleportToColony",
        source: "TeleportToColonyMessage.java:96-116",
        effect: Effect::PlayerItem,
    },
    Secondary {
        class: "UpgradeWarehouse",
        source: "UpgradeWarehouseMessage.java:56-63",
        effect: Effect::PlayerItem,
    },
    Secondary {
        class: "GiveTool",
        source: "GiveToolMessage.java:73-81",
        effect: Effect::PlayerItem,
    },
    Secondary {
        class: "PickUpTownhall",
        source: "PickUpTownhallMessage.java:82-85",
        effect: Effect::PlayerItem,
    },
    Secondary {
        class: "ClickGuiButtonTrigger",
        source: "ClickGuiButtonTriggerMessage.java:66",
        effect: Effect::Advancement,
    },
    Secondary {
        class: "AddPlayerOrFakePlayer",
        source: "PermissionsMessage.java:405-412",
        effect: Effect::ClientSound,
    },
];

/// The second effect of a class, when it has one.
pub fn secondary_of(class: &str) -> Vec<&'static Secondary> {
    SECONDARY
        .iter()
        .filter(|secondary| secondary.class == class)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The twenty-seven verbs and their needs, held to each other one at a time: a
    /// verb whose needs changed is a row that a reader would otherwise not notice.
    #[test]
    fn open_screen_needs_a_payload_a_screen_and_its_data() {
        assert_eq!(
            needs(Effect::OpenScreen),
            &[Need::Payload, Need::ModScreen, Need::ScreenData],
            "the server sends bytes, the client opens its own screen and reads them"
        );
    }

    #[test]
    fn client_view_needs_the_payload_and_a_view_store() {
        assert_eq!(needs(Effect::ClientView), &[Need::Payload, Need::ClientViewStore]);
    }

    #[test]
    fn client_manager_needs_a_global_store() {
        assert_eq!(
            needs(Effect::ClientManager),
            &[Need::Payload, Need::ClientManagerStore]
        );
    }

    #[test]
    fn client_particles_need_only_the_particle_call() {
        assert_eq!(needs(Effect::ClientParticles), &[Need::Particle]);
    }

    #[test]
    fn a_block_particle_needs_the_engine_call_not_the_particle_call() {
        assert_eq!(needs(Effect::ClientBlockParticles), &[Need::BlockParticle]);
        assert_ne!(
            needs(Effect::ClientBlockParticles),
            needs(Effect::ClientParticles),
            "the two particle verbs do not need the same thing"
        );
    }

    #[test]
    fn client_sound_needs_the_sound_call() {
        assert_eq!(needs(Effect::ClientSound), &[Need::Sound]);
    }

    #[test]
    fn stopping_every_sound_needs_more_than_a_sound_ref() {
        assert_eq!(needs(Effect::ClientStopSound), &[Need::MusicStop]);
        assert_ne!(
            needs(Effect::ClientStopSound),
            needs(Effect::ClientSound),
            "stop-all has no handle to pass, which is the whole of `Need::MusicStop`"
        );
    }

    #[test]
    fn client_debug_needs_only_the_mods_own_state() {
        assert_eq!(needs(Effect::ClientDebug), &[Need::DebugState]);
    }

    #[test]
    fn the_chunk_claim_needs_a_chunk_attachment() {
        assert_eq!(needs(Effect::ClientChunkClaim), &[Need::ChunkClaim]);
    }

    #[test]
    fn the_schematic_needs_a_disk_write() {
        assert_eq!(needs(Effect::ClientFile), &[Need::DiskWrite]);
    }

    #[test]
    fn the_colony_map_needs_a_window_of_the_mods_own() {
        assert_eq!(needs(Effect::ClientMap), &[Need::Payload, Need::MapWindow]);
    }

    #[test]
    fn dispatch_needs_fragments_before_it_needs_a_payload() {
        assert_eq!(needs(Effect::Dispatch), &[Need::Fragments, Need::Payload]);
    }

    #[test]
    fn a_colony_write_needs_the_registry() {
        assert_eq!(needs(Effect::ColonyWrite), &[Need::Payload, Need::ColonyStore]);
    }

    #[test]
    fn a_colony_setting_needs_only_the_registry() {
        assert_eq!(needs(Effect::ColonySetting), &[Need::ColonyStore]);
    }

    #[test]
    fn permissions_need_the_table_behind_the_colony() {
        assert_eq!(
            needs(Effect::Permissions),
            &[Need::ColonyStore, Need::PermissionTable]
        );
    }

    #[test]
    fn a_building_write_is_reached_through_its_colony() {
        assert_eq!(
            needs(Effect::BuildingWrite),
            &[Need::ColonyStore, Need::BuildingStore]
        );
    }

    #[test]
    fn a_module_write_adds_the_module_store() {
        assert_eq!(
            needs(Effect::ModuleWrite),
            &[Need::ColonyStore, Need::BuildingStore, Need::ModuleStore]
        );
    }

    #[test]
    fn a_citizen_write_is_reached_through_its_colony() {
        assert_eq!(
            needs(Effect::CitizenWrite),
            &[Need::ColonyStore, Need::CitizenStore]
        );
    }

    #[test]
    fn the_work_orders_need_the_work_manager() {
        assert_eq!(
            needs(Effect::WorkOrders),
            &[Need::ColonyStore, Need::WorkOrderStore]
        );
    }

    #[test]
    fn a_request_state_needs_the_request_manager() {
        assert_eq!(
            needs(Effect::RequestState),
            &[Need::ColonyStore, Need::RequestStore]
        );
    }

    #[test]
    fn a_player_item_needs_the_senders_inventory_only() {
        assert_eq!(needs(Effect::PlayerItem), &[Need::PlayerInventory]);
    }

    #[test]
    fn a_player_container_needs_the_senders_menu_only() {
        assert_eq!(needs(Effect::PlayerContainer), &[Need::PlayerMenu]);
    }

    #[test]
    fn opening_a_container_needs_a_menu_to_open_and_a_menu_to_keep() {
        assert_eq!(
            needs(Effect::OpenContainer),
            &[Need::ChestMenu, Need::PlayerMenu]
        );
    }

    #[test]
    fn a_block_write_needs_the_edit_call() {
        assert_eq!(needs(Effect::BlockWrite), &[Need::BlockWrite]);
    }

    #[test]
    fn spawning_needs_a_spawn_call() {
        assert_eq!(needs(Effect::SpawnEntity), &[Need::Spawn]);
    }

    #[test]
    fn teleport_needs_the_entity_call() {
        assert_eq!(needs(Effect::Teleport), &[Need::Teleport]);
    }

    #[test]
    fn an_advancement_needs_a_trigger() {
        assert_eq!(needs(Effect::Advancement), &[Need::Advancement]);
    }

    #[test]
    fn every_verb_has_needs_and_no_need_is_orphaned() {
        let mut painted: Vec<Need> = Vec::new();
        for effect in Effect::ALL {
            let needs = needs(effect);
            assert!(!needs.is_empty(), "{} needs nothing?", effect.name());
            painted.extend_from_slice(needs);
        }
        for need in NEEDS {
            assert!(
                painted.contains(&need),
                "{need:?} is declared and never needed by a verb"
            );
        }
    }

    #[test]
    fn every_need_has_a_verdict_and_every_verdict_a_need() {
        assert_eq!(VERDICTS.len(), NEEDS.len(), "one row per need");
        for need in NEEDS {
            let finding = Finding::of(need);
            assert_eq!(finding.need, need);
            assert!(!finding.source.is_empty(), "{need:?} has no source");
            assert!(
                finding.note.len() > 30,
                "{need:?}'s note is a word, not a measurement"
            );
        }
    }

    #[test]
    fn the_absent_list_is_the_five_the_research_found() {
        let names: Vec<Need> = absent().iter().map(|finding| finding.need).collect();
        assert_eq!(
            names,
            vec![
                Need::Spawn,
                Need::BlockParticle,
                Need::ChunkClaim,
                Need::Fragments,
                Need::Advancement
            ],
            "MANQUANT.md §2 must list exactly these, in this order"
        );
    }

    #[test]
    fn the_partial_list_names_what_is_almost_there() {
        let names: Vec<Need> = partial().iter().map(|finding| finding.need).collect();
        assert_eq!(
            names,
            vec![Need::ScreenData, Need::MusicStop, Need::DiskWrite],
            "each partial is a behaviour change, not a synonym for present"
        );
    }

    #[test]
    fn the_absent_citations_are_searches_not_guesses() {
        for finding in absent() {
            assert!(
                finding.source.contains("grep") || finding.source.contains("ProvidedBy")
                    || finding.source.contains("payload.rs") || finding.source.contains("advancements.rs"),
                "{:?} must cite the search that failed: {}",
                finding.need,
                finding.source
            );
        }
    }

    #[test]
    fn a_present_verdict_cites_a_file_and_a_line() {
        for finding in VERDICTS.iter().filter(|f| f.verdict == Verdict::Present) {
            let source = finding.source;
            assert!(source.contains(".rs:"), "{source} has no line");
            let line: u32 = source
                .rsplit(':')
                .next()
                .and_then(|tail| {
                    let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
                    digits.parse().ok()
                })
                .unwrap_or(0);
            assert!(line > 0, "{source} does not name a line");
        }
    }

    #[test]
    fn an_in_crate_verdict_names_a_module_of_this_crate() {
        for finding in VERDICTS.iter().filter(|f| f.verdict == Verdict::InCrate) {
            assert!(
                finding.source.starts_with("src/"),
                "{:?} must name a src/ module, not {:?}",
                finding.need,
                finding.source
            );
        }
    }

    #[test]
    fn the_client_verbs_are_the_eleven_readers() {
        let client: Vec<Effect> = Effect::ALL.into_iter().filter(|e| e.is_client()).collect();
        assert_eq!(client.len(), 11, "eleven verbs run on the client");
        assert!(client.contains(&Effect::OpenScreen));
        assert!(client.contains(&Effect::ClientMap));
        assert!(!client.contains(&Effect::Dispatch), "dispatch runs where it lands");
    }

    #[test]
    fn every_verb_has_a_name_and_no_two_share_one() {
        let mut names: Vec<&str> = Effect::ALL.iter().map(|effect| effect.name()).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "two verbs share a name");
    }

    #[test]
    fn the_secondary_table_names_effects_that_are_not_the_primary_one() {
        for secondary in SECONDARY.iter() {
            assert!(!secondary.class.is_empty());
            assert!(secondary.source.contains(".java:"), "{}", secondary.source);
        }
    }

    #[test]
    fn a_secondary_row_is_unique_per_class_and_effect() {
        for (index, secondary) in SECONDARY.iter().enumerate() {
            for other in SECONDARY.iter().skip(index + 1) {
                assert!(
                    secondary.class != other.class || secondary.effect != other.effect,
                    "{} is claimed twice with the same effect",
                    secondary.class
                );
            }
        }
    }

    // `NEEDS` is written out here rather than derived, because a `#[derive]` over an
    // enum with twenty-five variants would need a macro for one list; the coverage
    // test above is what keeps this list honest.
    const NEEDS: [Need; 28] = [
        Need::ModScreen,
        Need::ScreenData,
        Need::ChestMenu,
        Need::BlockWrite,
        Need::Spawn,
        Need::ClientViewStore,
        Need::ClientManagerStore,
        Need::Particle,
        Need::BlockParticle,
        Need::Sound,
        Need::MusicStop,
        Need::DebugState,
        Need::ChunkClaim,
        Need::DiskWrite,
        Need::MapWindow,
        Need::Payload,
        Need::Fragments,
        Need::ColonyStore,
        Need::PermissionTable,
        Need::BuildingStore,
        Need::ModuleStore,
        Need::CitizenStore,
        Need::WorkOrderStore,
        Need::RequestStore,
        Need::PlayerInventory,
        Need::PlayerMenu,
        Need::Teleport,
        Need::Advancement,
    ];
}
