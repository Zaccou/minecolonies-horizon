//! The colony's event bus: which events exist, who hears them, in what order,
//! and what a run left behind.
//!
//! Java reference: `api/eventbus/` — `EventBus` (`:17`), `DefaultEventBus`
//! (51 lines), `IModEvent` (`:15`), and the twenty classes under
//! `api/eventbus/events/` whose package this module ports.
//!
//! | File | Question |
//! | --- | --- |
//! | [`topic`] | which class is being posted, and which abstract class it hangs from |
//! | [`payload`] | what the event carries, with the Java field list |
//! | [`bus`] | who is called, in which order, and the named refusal when a subscriber loops |
//! | [`trace`] | what a test can read back, and replay |
//! | [`raid`] | "an attack begins": the colony-event machine, which is **not** on the bus |
//!
//! **The decision, in one sentence.** A [`payload::ColonieEvent`] carries its own
//! id and its Java fields; [`bus::EventBus::post`] delivers it to the
//! subscriptions whose [`topic::Topic`] is **exactly** its class, once each, in
//! registration order (`DefaultEventBus.java:26,40`), recording every delivery in
//! a [`trace::Trace`] that [`bus::EventBus::replay`] can feed back through a bus
//! wired the same way.
//!
//! ```ignore
//! let mut bus = EventBus::new();
//! let deaths = bus.subscribe(Topic::CitizenDied, "graveyard", |event, _ctx| {
//!     let id = event.citizen();
//!     Ok(())
//! });
//! let mut event = ColonieEvent::CitizenDied { id: EventId::UNASSIGNED, colony: 4,
//!     citizen: 12, damage: "minecraft:fall".into() };
//! bus.post(&mut event)?;
//! assert_eq!(bus.trace().labels(event.id()), ["graveyard"]);
//! ```
//!
//! **Three things the brief assumed, measured instead.**
//!
//! - `api/colony/events/` **does not exist**. The events live in
//!   `api/eventbus/events/`; `api/colony/colonyEvents/` is a different thing
//!   altogether (the save-backed raid/entity-spawn events of `raid`).
//! - "The Java has a priority rule" is true of **Forge's** bus only.
//!   `DefaultEventBus` orders by registration (`:26`, `:40`); `EventPriority` is
//!   used by `@SubscribeEvent` handlers (`journeymap/v6/EventListener.java:61`,
//!   `QuestObjectiveEventHandler.java:61`), and the one event of this package
//!   that rides Forge's bus is `CustomRecipesReloadedEvent`
//!   (`CustomRecipeManager.java:438`). [`bus::Priority`] exists for exactly that
//!   topic and is refused everywhere else.
//! - "An attack begins" is **not** on the bus at all: no raid class posts an
//!   `IModEvent`. See [`raid`]'s header.
//!
//! **Pure data and pure rules**: no host call, no clock, no file. What the port
//! cannot express — a `UUID` source, subscribing from a handler, a raid on the
//! bus — is in `MANQUANT.md` with what it cost.

pub mod bus;
pub mod payload;
pub mod raid;
pub mod topic;
pub mod trace;

#[cfg(test)]
mod tests;

pub use bus::{
    Ctx, Delivery, EventBus, HandlerError, Inert, Priority, Refusal, SubscriberId, DEPTH_LIMIT,
    DEPTH_LIMIT_SOURCE,
};
pub use payload::{
    CitizenAddedSource, EventId, ManagerRef, Notices, RemovalReason,
    NOTICES_SHOW_DEFAULT, NOTICES_SHOW_DEFAULT_SOURCE, NOTICES_SPECTATORS_DEFAULT,
    NOTICES_SPECTATORS_DEFAULT_SOURCE,
};
pub use payload::ColonieEvent;
pub use raid::{
    take_from, FIRST_ID, FIRST_ID_SOURCE, ID_CEILING_MARGIN, ID_CEILING_MARGIN_SOURCE, Lifecycle,
    LifecycleRefusal, Raid, RaidStep, Status,
};
pub use topic::{Bus, Family, Topic};
pub use trace::{Outcome, Record, Trace};
