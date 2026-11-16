//! One name per Java event class, and the abstract class it hangs from.
//!
//! Java subscribes with a `Class<T>` and looks a post up by `event.getClass()`
//! (`DefaultEventBus.java:32`), so **a subscription is a class, exactly** — not
//! a supertype. [`Topic`] is that class, one variant per concrete
//! `IModEvent` under `api/eventbus/events`, and [`Family`] is the abstract class a
//! mod might have subscribed to *believing* it would match: it never does in
//! Java, and `bus::EventBus::subscribe_family` keeps that true, loudly — an inert
//! subscription shows up in `EventBus::inert` instead of vanishing.
//!
//! The table below is the module's only source of truth: name, package, family
//! and bus are written once, and `java_class` is `concat!`ed from the first two,
//! so a renaming cannot leave half the class name behind. The fields each class
//! carries are in `payload`; the `fichier.java:ligne` of each `post` is on the
//! payload variant.

/// The two buses a MineColonies event can ride.
///
/// Measured, not assumed: every `post` in the tree goes through
/// `IMinecoloniesAPI.getInstance().getEventBus()` except one —
/// `CustomRecipeManager.java:438` posts `CustomRecipesReloadedEvent` on
/// `MinecraftForge.EVENT_BUS`, because that class extends Forge's `Event`
/// (`CustomRecipesReloadedEvent.java:9`), not `AbstractModEvent`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: mirrors a Java fact, not a `mod_api` type
pub enum Bus {
    /// `api/eventbus/EventBus`, the mod's own: registration order, no priority.
    Mod,
    /// `MinecraftForge.EVENT_BUS`: ordered by `EventPriority`.
    Forge,
}

/// The abstract classes of `api/eventbus/events`, as family names.
///
/// `CustomRecipesReloadedEvent` has no MineColonies ancestor at all — it extends
/// Forge's `Event` (`:9`) — so it gets [`Family::ForgeEvent`] rather than being
/// filed under `AbstractModEvent`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: mirrors a Java fact, not a `mod_api` type
pub enum Family {
    /// `events/AbstractModEvent.java:10`, the root of every `IModEvent`.
    AbstractModEvent,
    /// `events/colony/AbstractColonyModEvent.java:10`.
    AbstractColonyModEvent,
    /// `events/colony/buildings/AbstractBuildingModEvent.java:9`.
    AbstractBuildingModEvent,
    /// `events/colony/citizens/AbstractCitizenModEvent.java:11`.
    AbstractCitizenModEvent,
    /// Forge's `Event`, the one non-`IModEvent` of the package.
    ForgeEvent,
}

impl Family {
    /// The five families, root first.
    pub const ALL: &'static [Family] = &[
        Family::AbstractModEvent,
        Family::AbstractColonyModEvent,
        Family::AbstractBuildingModEvent,
        Family::AbstractCitizenModEvent,
        Family::ForgeEvent,
    ];

    /// The abstract class's binary name.
    pub fn java_class(self) -> &'static str {
        match self {
            Family::AbstractModEvent => "com.minecolonies.api.eventbus.events.AbstractModEvent",
            Family::AbstractColonyModEvent => {
                "com.minecolonies.api.eventbus.events.colony.AbstractColonyModEvent"
            }
            Family::AbstractBuildingModEvent => {
                "com.minecolonies.api.eventbus.events.colony.buildings.AbstractBuildingModEvent"
            }
            Family::AbstractCitizenModEvent => {
                "com.minecolonies.api.eventbus.events.colony.citizens.AbstractCitizenModEvent"
            }
            Family::ForgeEvent => "net.minecraftforge.eventbus.api.Event",
        }
    }

    /// The concrete topics this family *seems* to cover — what a modder who
    /// subscribed to the abstract class was after. Java delivers none of them
    /// (`DefaultEventBus.java:32`).
    pub fn topics(self) -> Vec<Topic> {
        Topic::ALL
            .iter()
            .copied()
            .filter(|topic| topic.family() == self)
            .collect()
    }
}

macro_rules! topics {
    ($($variant:ident => $simple:literal, $package:literal, $family:ident, $bus:ident;)*) => {
        /// One concrete event class of the package: `Topic::ColonyCreated` is
        /// `ColonyCreatedModEvent` of the colony package.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        // exhaustive: the package is a closed list in Java
        pub enum Topic {
            $($variant,)*
        }

        impl Topic {
            /// The twenty classes of the package, in package order.
            pub const ALL: &'static [Topic] = &[$(Topic::$variant,)*];

            /// The Java simple name: `"ColonyCreatedModEvent"`. One exception,
            /// and it is Java's: `CustomRecipesReloadedEvent` has no `Mod` in its
            /// name because it does not ride the mod bus.
            pub fn simple_name(self) -> &'static str {
                match self {
                    $(Topic::$variant => $simple,)*
                }
            }

            /// The package, as Java writes it.
            pub fn package(self) -> &'static str {
                match self {
                    $(Topic::$variant => $package,)*
                }
            }

            /// The class's binary name — what a Java `subscribe` names.
            pub fn java_class(self) -> &'static str {
                match self {
                    $(Topic::$variant => concat!($package, ".", $simple),)*
                }
            }

            /// The abstract class it extends, nearest first.
            pub fn family(self) -> Family {
                match self {
                    $(Topic::$variant => Family::$family,)*
                }
            }

            /// The bus its events are posted on. See [`Bus`].
            pub fn bus(self) -> Bus {
                match self {
                    $(Topic::$variant => Bus::$bus,)*
                }
            }
        }
    };
}

topics! {
    // `events/` — plain `AbstractModEvent`s, not colony-scoped.
    ColonyManagerLoaded =>
        "ColonyManagerLoadedModEvent", "com.minecolonies.api.eventbus.events", AbstractModEvent, Mod;
    ColonyManagerUnloaded =>
        "ColonyManagerUnloadedModEvent", "com.minecolonies.api.eventbus.events", AbstractModEvent, Mod;
    // The one event of the package that extends Forge's `Event` (`:9`).
    CustomRecipesReloaded =>
        "CustomRecipesReloadedEvent", "com.minecolonies.api.eventbus.events", ForgeEvent, Forge;
    // `events/colony/`
    ColonyCreated =>
        "ColonyCreatedModEvent", "com.minecolonies.api.eventbus.events.colony", AbstractColonyModEvent, Mod;
    ColonyDeleted =>
        "ColonyDeletedModEvent", "com.minecolonies.api.eventbus.events.colony", AbstractColonyModEvent, Mod;
    ColonyFlagChanged =>
        "ColonyFlagChangedModEvent", "com.minecolonies.api.eventbus.events.colony", AbstractColonyModEvent, Mod;
    ColonyNameChanged =>
        "ColonyNameChangedModEvent", "com.minecolonies.api.eventbus.events.colony", AbstractColonyModEvent, Mod;
    ColonyPlayerRankChanged =>
        "ColonyPlayerRankChangedModEvent", "com.minecolonies.api.eventbus.events.colony", AbstractColonyModEvent, Mod;
    ColonyTeamColorChanged =>
        "ColonyTeamColorChangedModEvent", "com.minecolonies.api.eventbus.events.colony", AbstractColonyModEvent, Mod;
    // Client side: `ColonyViewUpdatedModEvent` narrows `getColony()` to a view
    // (`:22-25`).
    ColonyViewUpdated =>
        "ColonyViewUpdatedModEvent", "com.minecolonies.api.eventbus.events.colony", AbstractColonyModEvent, Mod;
    // `events/colony/buildings/`
    BuildingAdded =>
        "BuildingAddedModEvent", "com.minecolonies.api.eventbus.events.colony.buildings", AbstractBuildingModEvent, Mod;
    BuildingConstruction =>
        "BuildingConstructionModEvent", "com.minecolonies.api.eventbus.events.colony.buildings", AbstractBuildingModEvent, Mod;
    BuildingRemoved =>
        "BuildingRemovedModEvent", "com.minecolonies.api.eventbus.events.colony.buildings", AbstractBuildingModEvent, Mod;
    // `events/colony/citizens/` — the three that really are citizen events.
    CitizenAdded =>
        "CitizenAddedModEvent", "com.minecolonies.api.eventbus.events.colony.citizens", AbstractCitizenModEvent, Mod;
    CitizenDied =>
        "CitizenDiedModEvent", "com.minecolonies.api.eventbus.events.colony.citizens", AbstractCitizenModEvent, Mod;
    CitizenJobChanged =>
        "CitizenJobChangedModEvent", "com.minecolonies.api.eventbus.events.colony.citizens", AbstractCitizenModEvent, Mod;
    // In the same package, but `extends AbstractColonyModEvent` (`:12`, `:11`).
    CitizenBuried =>
        "CitizenBuriedModEvent", "com.minecolonies.api.eventbus.events.colony.citizens", AbstractColonyModEvent, Mod;
    CitizenRemoved =>
        "CitizenRemovedModEvent", "com.minecolonies.api.eventbus.events.colony.citizens", AbstractColonyModEvent, Mod;
    // `events/colony/permissions/` — colony events with a permissions package.
    PlayerEntering =>
        "PlayerEnteringModEvent", "com.minecolonies.api.eventbus.events.colony.permissions", AbstractColonyModEvent, Mod;
    PlayerLeaving =>
        "PlayerLeavingModEvent", "com.minecolonies.api.eventbus.events.colony.permissions", AbstractColonyModEvent, Mod;
}

impl Topic {
    /// The topic a Java binary name names, or `None` for anything else — no
    /// guessing from a suffix.
    pub fn from_java_class(class: &str) -> Option<Topic> {
        Topic::ALL
            .iter()
            .copied()
            .find(|topic| topic.java_class() == class)
    }

    /// Whether the event names a colony: true for everything under
    /// `AbstractColonyModEvent` (`:32`), the three families below it included.
    pub fn colony_scoped(self) -> bool {
        !matches!(self.family(), Family::AbstractModEvent | Family::ForgeEvent)
    }
}
