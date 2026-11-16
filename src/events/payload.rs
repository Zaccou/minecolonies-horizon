//! The twenty payloads, with the fields Java gives each one.
//!
//! One enum, one variant per class of `api/eventbus/events` — this is where
//! [`super::topic::Topic`]'s twenty names get their body. The variants are
//! **flat**: Java's inheritance does not become a Rust hierarchy, because
//! dispatch is by exact class anyway (`DefaultEventBus.java:32`), and the family
//! questions a subscriber asks (`getColony`, `getCitizen`, `getBuilding`) are
//! answered by the accessors below instead.
//!
//! **Three types are cut, each with a reason.**
//!
//! - `IColony`, `IColonyView`, `IBuilding`, `ICitizenData` → an `i32` id, a
//!   `[i32; 3]` position, or the citizen's id: the same cut
//!   `crate::base::ident` already made (`InColony`, `Identified`).
//! - `IColonyManager` → [`ManagerRef`]: Java hands over the singleton
//!   (`ColonyManagerLoadedModEvent.java:15`), which a wasm mod cannot hold.
//! - `WorkOrderBuilding` → `crate::colony::WorkOrder`, the port's own record, so
//!   a subscriber reads the same fields as `getWorkOrder()` gives in Java.
//!
//! **What is *not* cut.** `CitizenBuriedModEvent`'s saved NBT stays a
//! `Compound`, and it stays a copy (`:46` in, `:68` out). `getDay()` is read
//! when the event is built (`:50`), so the day is a plain field here — the port
//! takes it from the caller.

use mod_api::Compound;

use crate::colony::{Rank, WorkOrder};

use super::topic::Topic;

/// `UUID eventId`, assigned in the constructor (`AbstractModEvent.java:16-23`).
///
/// **The port assigns it at post, from a counter** (`bus::EventBus`), for two
/// reasons: the SDK has no UUID source (see `MANQUANT.md` §1), and a random id
/// makes a [`super::trace::Trace`] impossible to replay. [`EventId::UNASSIGNED`]
/// is the state a freshly built event is in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct EventId(u64);

impl EventId {
    /// What an event built by its caller carries: no identity yet.
    pub const UNASSIGNED: EventId = EventId(0);

    /// The counter value, for a log line or a test.
    pub fn raw(self) -> u64 {
        self.0
    }

    /// An id from a counter, for the bus and for a replay.
    pub fn new(raw: u64) -> EventId {
        EventId(raw)
    }

    /// Whether the bus still has to name this event.
    pub fn is_unassigned(self) -> bool {
        self == EventId::UNASSIGNED
    }
}

/// Java's `IColonyManager`, cut to a handle: the event names *which* manager
/// loaded, and the port has no singleton to hand over.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct ManagerRef(pub u32);

/// How a citizen came into the colony (`CitizenAddedModEvent.java:40-62`), in
/// the Java declaration order.
///
/// `BORN` is declared and **never posted**: `ReproductionManager.java:213` fires
/// the *description* event (`CitizenBornEvent`), not this one. The variant is
/// kept because a save read by a mod may name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: mirrors a Java enum, not a `mod_api` type
pub enum CitizenAddedSource {
    /// The colony's first citizens (`:45`), posted at `CitizenManager.java:632`.
    Initial,
    /// Born in the colony (`:49`) — declared, never posted here.
    Born,
    /// Hired from the tavern (`:53`), posted at `RecruitmentInteraction.java:204`.
    Hired,
    /// Raised from a grave (`:57`), posted at `CitizenManager.java:373`.
    Resurrected,
    /// Spawned by a command (`:61`), posted at `CommandCitizenSpawnNew.java:37`.
    Commands,
}

impl CitizenAddedSource {
    /// The five values, in the order that fixes nothing but a test.
    pub const ALL: [CitizenAddedSource; 5] = [
        CitizenAddedSource::Initial,
        CitizenAddedSource::Born,
        CitizenAddedSource::Hired,
        CitizenAddedSource::Resurrected,
        CitizenAddedSource::Commands,
    ];

    /// The Java constant's name.
    pub fn java_name(self) -> &'static str {
        match self {
            CitizenAddedSource::Initial => "INITIAL",
            CitizenAddedSource::Born => "BORN",
            CitizenAddedSource::Hired => "HIRED",
            CitizenAddedSource::Resurrected => "RESURRECTED",
            CitizenAddedSource::Commands => "COMMANDS",
        }
    }
}

/// Java's `Entity.RemovalReason`, the five ways an entity leaves a level.
///
/// Not in this tree — it is Minecraft's enum — so the list comes from the game,
/// not from a `fichier.java:ligne`: `KILLED`, `DISCARDED`, `UNLOADED_TO_CHUNK`,
/// `UNLOADED_WITH_PLAYER`, `CHANGED_DIMENSION`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: mirrors a vanilla enum, not a `mod_api` type
pub enum RemovalReason {
    Killed,
    Discarded,
    UnloadedToChunk,
    UnloadedWithPlayer,
    ChangedDimension,
}

/// The two flags `PlayerEnteringModEvent`/`PlayerLeavingModEvent` carry
/// (`:21`, `:26`), and the getter that reads the live player's spectator state
/// (`:53-56`).
///
/// **A handler may veto, and the caller reads the result** — that is the whole
/// point of these two events (`Colony.java:1643` checks
/// `shouldShowNotification()` *after* `post`). So they are the only payload
/// fields a handler is expected to change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Notices {
    /// `shouldShowNotification`, `true` at construction.
    pub show: bool,
    /// `shouldShowForSpectators`, `false` at construction.
    pub for_spectators: bool,
}

/// `PlayerEnteringModEvent.java:21` — `shouldShowNotification = true`.
pub const NOTICES_SHOW_DEFAULT: bool = true;
/// Where [`NOTICES_SHOW_DEFAULT`] comes from.
pub const NOTICES_SHOW_DEFAULT_SOURCE: &str =
    "api/eventbus/events/colony/permissions/PlayerEnteringModEvent.java:21";

/// `PlayerEnteringModEvent.java:26` — `shouldShowForSpectators = false`.
pub const NOTICES_SPECTATORS_DEFAULT: bool = false;
/// Where [`NOTICES_SPECTATORS_DEFAULT`] comes from.
pub const NOTICES_SPECTATORS_DEFAULT_SOURCE: &str =
    "api/eventbus/events/colony/permissions/PlayerEnteringModEvent.java:26";

impl Notices {
    /// The state both permission events start in (`:21`, `:26`).
    pub fn new() -> Notices {
        Notices {
            show: NOTICES_SHOW_DEFAULT,
            for_spectators: NOTICES_SPECTATORS_DEFAULT,
        }
    }

    /// `disableNotification()` (`:61-64`): no notice, ever, for this event.
    pub fn disable(&mut self) {
        self.show = false;
    }

    /// `allowForSpectators()` (`:69-72`): lift the spectator filter.
    pub fn allow_spectators(&mut self) {
        self.for_spectators = true;
    }

    /// `shouldShowNotification()` (`:53-56`), whole:
    /// `shouldShowNotification && (!player.isSpectator() || shouldShowForSpectators)`.
    ///
    /// The spectator state is the caller's to pass: the port does not hold the
    /// `Player` (`payload`'s header).
    pub fn should_show(&self, spectator: bool) -> bool {
        self.show && (!spectator || self.for_spectators)
    }
}

impl Default for Notices {
    fn default() -> Notices {
        Notices::new()
    }
}

/// Everything one `post` can carry. See the module header for the cuts.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: mirrors the Java package, one variant per class
pub enum ColonieEvent {
    /// `ColonyManagerLoadedModEvent`: the manager instance (`:15`). Posted at
    /// `ColonyManager.java:669`, server side, after a world loads.
    ColonyManagerLoaded { id: EventId, manager: ManagerRef },
    /// `ColonyManagerUnloadedModEvent`: the manager instance (`:15`). Posted at
    /// `ColonyManager.java:696`, only when the world being unloaded had colonies.
    ColonyManagerUnloaded { id: EventId, manager: ManagerRef },
    /// `CustomRecipesReloadedEvent`: **no field** (`:11-13`). Posted on Forge's
    /// bus at `CustomRecipeManager.java:438`, client side, once per data-pack
    /// load.
    CustomRecipesReloaded { id: EventId },
    /// `ColonyCreatedModEvent`: the colony (`:16`). Posted at
    /// `CreateColonyMessage.java:191`.
    ColonyCreated { id: EventId, colony: i32 },
    /// `ColonyDeletedModEvent`: the colony (`:16`). Posted at
    /// `ColonyManager.java:209`.
    ColonyDeleted { id: EventId, colony: i32 },
    /// `ColonyFlagChangedModEvent`: the colony (`:16`). Posted at
    /// `ColonyFlagChangeMessage.java:47`.
    ColonyFlagChanged { id: EventId, colony: i32 },
    /// `ColonyNameChangedModEvent`: the colony (`:16`). Posted at
    /// `TownHallRenameMessage.java:58`.
    ColonyNameChanged { id: EventId, colony: i32 },
    /// `ColonyPlayerRankChangedModEvent`: the player (`:17`), their new rank
    /// (`:22`) and their old one (`:27`). Posted at `Permissions.java:743`.
    ColonyPlayerRankChanged {
        id: EventId,
        colony: i32,
        player: u128,
        new_rank: Rank,
        old_rank: Rank,
    },
    /// `ColonyTeamColorChangedModEvent`: the colony (`:16`). Posted at
    /// `TeamColonyColorChangeMessage.java:70`.
    ColonyTeamColorChanged { id: EventId, colony: i32 },
    /// `ColonyViewUpdatedModEvent`: the colony *view* (`:16`, narrowed by the
    /// override at `:22-25`). Posted client side at `ColonyManager.java:725`,
    /// once per colony view packet.
    ColonyViewUpdated { id: EventId, colony: i32 },
    /// `BuildingAddedModEvent`: the building (`AbstractBuildingModEvent.java:14`),
    /// whose colony comes from `building.getColony()` (`:23`). Posted at
    /// `RegisteredStructureManager.java:615`.
    BuildingAdded {
        id: EventId,
        colony: i32,
        building: [i32; 3],
    },
    /// `BuildingConstructionModEvent`: the building, and the completed work
    /// order (`:14`, `:33`). Posted at
    /// `AbstractEntityAIStructureWithWorkOrder.java:456` for build, upgrade,
    /// repair **and** remove — the event does not say which.
    BuildingConstruction {
        id: EventId,
        colony: i32,
        building: [i32; 3],
        work_order: WorkOrder,
    },
    /// `BuildingRemovedModEvent`: the building (`:14`). Posted at
    /// `RegisteredStructureManager.java:680`.
    BuildingRemoved {
        id: EventId,
        colony: i32,
        building: [i32; 3],
    },
    /// `CitizenAddedModEvent`: the citizen (`AbstractCitizenModEvent.java:17`)
    /// and how it arrived (`:13`).
    CitizenAdded {
        id: EventId,
        colony: i32,
        citizen: i32,
        source: CitizenAddedSource,
    },
    /// `CitizenBuriedModEvent`: grave position (`:14`), a **copy** of the saved
    /// NBT (`:16`, copied at `:46`), name (`:18`), job name or null (`:20`), the
    /// undertaker (`:22`) and the day it was built (`:24`, `:50`). Posted at
    /// `GraveyardManagementModule.java:358`, only when the saved NBT is present.
    CitizenBuried {
        id: EventId,
        colony: i32,
        grave: [i32; 3],
        saved: Compound,
        citizen_name: String,
        citizen_job: Option<String>,
        undertaker: i32,
        day: i32,
    },
    /// `CitizenDiedModEvent`: the citizen and the damage source (`:15`). Posted
    /// at `EntityCitizen.java:1626`, inside `die`, after the citizen data was
    /// removed from the manager.
    CitizenDied {
        id: EventId,
        colony: i32,
        citizen: i32,
        damage: String,
    },
    /// `CitizenJobChangedModEvent`: the previous job (`:14`), `null` when the
    /// citizen had none. Posted at `CitizenData.java:1022` — **not** on load.
    CitizenJobChanged {
        id: EventId,
        colony: i32,
        citizen: i32,
        previous_job: Option<String>,
    },
    /// `CitizenRemovedModEvent`: the citizen's **id**, not the citizen, and the
    /// removal reason (`:16`, `:22`). Posted at `EntityCitizen.java:1635`,
    /// *after* `super.remove(reason)`.
    CitizenRemoved {
        id: EventId,
        colony: i32,
        citizen: i32,
        reason: RemovalReason,
    },
    /// `PlayerEnteringModEvent`: the player (`:16`) and its two veto flags.
    /// Posted at `Colony.java:1641`, only for a non-manager visitor in a colony
    /// with `ENTER_LEAVE_MESSAGES` on.
    PlayerEntering {
        id: EventId,
        colony: i32,
        player: u128,
        notices: Notices,
    },
    /// `PlayerLeavingModEvent`: same shape, posted at `Colony.java:1662`.
    PlayerLeaving {
        id: EventId,
        colony: i32,
        player: u128,
        notices: Notices,
    },
}

impl ColonieEvent {
    /// The event's id: `UNASSIGNED` until the bus names it.
    pub fn id(&self) -> EventId {
        match self {
            ColonieEvent::ColonyManagerLoaded { id, .. }
            | ColonieEvent::ColonyManagerUnloaded { id, .. }
            | ColonieEvent::CustomRecipesReloaded { id }
            | ColonieEvent::ColonyCreated { id, .. }
            | ColonieEvent::ColonyDeleted { id, .. }
            | ColonieEvent::ColonyFlagChanged { id, .. }
            | ColonieEvent::ColonyNameChanged { id, .. }
            | ColonieEvent::ColonyPlayerRankChanged { id, .. }
            | ColonieEvent::ColonyTeamColorChanged { id, .. }
            | ColonieEvent::ColonyViewUpdated { id, .. }
            | ColonieEvent::BuildingAdded { id, .. }
            | ColonieEvent::BuildingConstruction { id, .. }
            | ColonieEvent::BuildingRemoved { id, .. }
            | ColonieEvent::CitizenAdded { id, .. }
            | ColonieEvent::CitizenBuried { id, .. }
            | ColonieEvent::CitizenDied { id, .. }
            | ColonieEvent::CitizenJobChanged { id, .. }
            | ColonieEvent::CitizenRemoved { id, .. }
            | ColonieEvent::PlayerEntering { id, .. }
            | ColonieEvent::PlayerLeaving { id, .. } => *id,
        }
    }

    /// Names the event. The bus does this once, on the first `post`.
    pub fn set_id(&mut self, new: EventId) {
        match self {
            ColonieEvent::ColonyManagerLoaded { id, .. }
            | ColonieEvent::ColonyManagerUnloaded { id, .. }
            | ColonieEvent::CustomRecipesReloaded { id }
            | ColonieEvent::ColonyCreated { id, .. }
            | ColonieEvent::ColonyDeleted { id, .. }
            | ColonieEvent::ColonyFlagChanged { id, .. }
            | ColonieEvent::ColonyNameChanged { id, .. }
            | ColonieEvent::ColonyPlayerRankChanged { id, .. }
            | ColonieEvent::ColonyTeamColorChanged { id, .. }
            | ColonieEvent::ColonyViewUpdated { id, .. }
            | ColonieEvent::BuildingAdded { id, .. }
            | ColonieEvent::BuildingConstruction { id, .. }
            | ColonieEvent::BuildingRemoved { id, .. }
            | ColonieEvent::CitizenAdded { id, .. }
            | ColonieEvent::CitizenBuried { id, .. }
            | ColonieEvent::CitizenDied { id, .. }
            | ColonieEvent::CitizenJobChanged { id, .. }
            | ColonieEvent::CitizenRemoved { id, .. }
            | ColonieEvent::PlayerEntering { id, .. }
            | ColonieEvent::PlayerLeaving { id, .. } => *id = new,
        }
    }

    /// `event.getClass()` (`DefaultEventBus.java:32`), as a topic.
    pub fn topic(&self) -> Topic {
        match self {
            ColonieEvent::ColonyManagerLoaded { .. } => Topic::ColonyManagerLoaded,
            ColonieEvent::ColonyManagerUnloaded { .. } => Topic::ColonyManagerUnloaded,
            ColonieEvent::CustomRecipesReloaded { .. } => Topic::CustomRecipesReloaded,
            ColonieEvent::ColonyCreated { .. } => Topic::ColonyCreated,
            ColonieEvent::ColonyDeleted { .. } => Topic::ColonyDeleted,
            ColonieEvent::ColonyFlagChanged { .. } => Topic::ColonyFlagChanged,
            ColonieEvent::ColonyNameChanged { .. } => Topic::ColonyNameChanged,
            ColonieEvent::ColonyPlayerRankChanged { .. } => Topic::ColonyPlayerRankChanged,
            ColonieEvent::ColonyTeamColorChanged { .. } => Topic::ColonyTeamColorChanged,
            ColonieEvent::ColonyViewUpdated { .. } => Topic::ColonyViewUpdated,
            ColonieEvent::BuildingAdded { .. } => Topic::BuildingAdded,
            ColonieEvent::BuildingConstruction { .. } => Topic::BuildingConstruction,
            ColonieEvent::BuildingRemoved { .. } => Topic::BuildingRemoved,
            ColonieEvent::CitizenAdded { .. } => Topic::CitizenAdded,
            ColonieEvent::CitizenBuried { .. } => Topic::CitizenBuried,
            ColonieEvent::CitizenDied { .. } => Topic::CitizenDied,
            ColonieEvent::CitizenJobChanged { .. } => Topic::CitizenJobChanged,
            ColonieEvent::CitizenRemoved { .. } => Topic::CitizenRemoved,
            ColonieEvent::PlayerEntering { .. } => Topic::PlayerEntering,
            ColonieEvent::PlayerLeaving { .. } => Topic::PlayerLeaving,
        }
    }

    /// `AbstractColonyModEvent.getColony()` (`:32`): the colony, for the three
    /// colony-derived families and `None` for the three events above them.
    pub fn colony(&self) -> Option<i32> {
        match self {
            ColonieEvent::ColonyManagerLoaded { .. }
            | ColonieEvent::ColonyManagerUnloaded { .. }
            | ColonieEvent::CustomRecipesReloaded { .. } => None,
            ColonieEvent::ColonyCreated { colony, .. }
            | ColonieEvent::ColonyDeleted { colony, .. }
            | ColonieEvent::ColonyFlagChanged { colony, .. }
            | ColonieEvent::ColonyNameChanged { colony, .. }
            | ColonieEvent::ColonyPlayerRankChanged { colony, .. }
            | ColonieEvent::ColonyTeamColorChanged { colony, .. }
            | ColonieEvent::ColonyViewUpdated { colony, .. }
            | ColonieEvent::BuildingAdded { colony, .. }
            | ColonieEvent::BuildingConstruction { colony, .. }
            | ColonieEvent::BuildingRemoved { colony, .. }
            | ColonieEvent::CitizenAdded { colony, .. }
            | ColonieEvent::CitizenBuried { colony, .. }
            | ColonieEvent::CitizenDied { colony, .. }
            | ColonieEvent::CitizenJobChanged { colony, .. }
            | ColonieEvent::CitizenRemoved { colony, .. }
            | ColonieEvent::PlayerEntering { colony, .. }
            | ColonieEvent::PlayerLeaving { colony, .. } => Some(*colony),
        }
    }

    /// `AbstractCitizenModEvent.getCitizen().getId()` (`:36`), for the three
    /// events that class covers, plus `CitizenRemoved` — not a citizen event
    /// (`topic::Topic::family`), but it carries the id all the same (`:16`).
    pub fn citizen(&self) -> Option<i32> {
        match self {
            ColonieEvent::CitizenAdded { citizen, .. }
            | ColonieEvent::CitizenDied { citizen, .. }
            | ColonieEvent::CitizenJobChanged { citizen, .. }
            | ColonieEvent::CitizenRemoved { citizen, .. } => Some(*citizen),
            _ => None,
        }
    }

    /// `AbstractBuildingModEvent.getBuilding()` (`:32`), answered as the
    /// building's position — its identity in this port (`crate::base::ident`).
    pub fn building(&self) -> Option<[i32; 3]> {
        match self {
            ColonieEvent::BuildingAdded { building, .. }
            | ColonieEvent::BuildingConstruction { building, .. }
            | ColonieEvent::BuildingRemoved { building, .. } => Some(*building),
            _ => None,
        }
    }

    /// The player, for the three events that name one. Java has two different
    /// getters (`getPlayer()` returns a `ColonyPlayer` on the rank event, a
    /// `Player` on the two permission ones); the port answers the uuid both
    /// carry.
    pub fn player(&self) -> Option<u128> {
        match self {
            ColonieEvent::ColonyPlayerRankChanged { player, .. }
            | ColonieEvent::PlayerEntering { player, .. }
            | ColonieEvent::PlayerLeaving { player, .. } => Some(*player),
            _ => None,
        }
    }

    /// The two veto flags, on the two events that have them and `None` on the
    /// others.
    pub fn notices(&self) -> Option<&Notices> {
        match self {
            ColonieEvent::PlayerEntering { notices, .. }
            | ColonieEvent::PlayerLeaving { notices, .. } => Some(notices),
            _ => None,
        }
    }

    /// The same flags, for a handler that wants to veto.
    pub fn notices_mut(&mut self) -> Option<&mut Notices> {
        match self {
            ColonieEvent::PlayerEntering { notices, .. }
            | ColonieEvent::PlayerLeaving { notices, .. } => Some(notices),
            _ => None,
        }
    }
}
