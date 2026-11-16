//! A raid starts, a raid ends: the colony-event machine, and why it is not on
//! the bus.
//!
//! **Measured, and it contradicts the brief.** `core/colony/events/` holds no
//! `IModEvent` at all: `RaiderConstants` aside, it is the raid *implementations*
//! (`HordeRaidEvent`, `AbstractShipRaidEvent`, the five flavours) and
//! `RaidManager`. Not one of them calls `getEventBus().post(...)` — the twenty
//! classes of `api/eventbus/events` never mention a raid. In Java, "an attack
//! begins" is **not** a bus event: a mod observes it by polling
//! `IEventManager.getEvents()` (`EventManager.java:215`) and reading
//! `IColonyRaidEvent.isRaidActive()` (`IColonyRaidEvent.java:50-55`).
//!
//! So this file ports the machine that makes that observation possible —
//! [`Lifecycle`] is `EventManager`'s per-colony map plus its tick
//! (`:184-212`), and [`RaidStep`] is what a tick did to one raid. A hook that
//! wants to react can drive the machine and read the steps; it cannot subscribe,
//! because Java gives it nothing to subscribe to. `MANQUANT.md` §3 is the
//! request that would change that.
//!
//! **The asymmetry worth knowing.** `onFinish()` runs when the status reaches
//! `DONE` (`:191-197`) — and the structure manager restores the event's backup
//! there (`:194`). A `CANCELED` event is removed **without** `onFinish`
//! (`:202-206`), i.e. without that restore. A canceled pirate raid leaves its
//! ship's terrain behind; that is Java's behaviour, and [`RaidStep::Dropped`]
//! names it rather than hiding it.

use std::collections::BTreeMap;

use super::trace::Trace;

/// `EventStatus` (`api/colony/colonyEvents/EventStatus.java:6-13`), in the Java
/// declaration order — which is also the order `serializeNBT` stores it in, an
/// ordinal (`AbstractShipRaidEvent.java:531`, `HordeRaidEvent.java:531`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: mirrors a Java enum, not a `mod_api` type
pub enum Status {
    /// Built, not started yet: the tick calls `onStart` (`EventManager.java:198`).
    Starting,
    /// The raid is placing its ship or its camp (`AbstractShipRaidEvent.java:182`).
    Preparing,
    /// Raiders are alive and fighting (`HordeRaidEvent.java:248`).
    Progressing,
    /// Paused by the event's own logic (`AbstractShipRaidEvent.java:252`).
    Waiting,
    /// Over: `onFinish` runs and the event leaves the map (`EventManager.java:191`).
    Done,
    /// Called off: the event leaves the map with **no** `onFinish`
    /// (`EventManager.java:202`).
    Canceled,
}

impl Status {
    /// The six, in ordinal order.
    pub const ALL: [Status; 6] = [
        Status::Starting,
        Status::Preparing,
        Status::Progressing,
        Status::Waiting,
        Status::Done,
        Status::Canceled,
    ];

    /// The Java constant's name.
    pub fn java_name(self) -> &'static str {
        match self {
            Status::Starting => "STARTING",
            Status::Preparing => "PREPARING",
            Status::Progressing => "PROGRESSING",
            Status::Waiting => "WAITING",
            Status::Done => "DONE",
            Status::Canceled => "CANCELED",
        }
    }

    /// `IColonyRaidEvent.isRaidActive()` (`:50-55`): **only** these two, so a
    /// raid in `Waiting` is not "active" and a raid in `Starting` neither.
    pub fn is_active(self) -> bool {
        matches!(self, Status::Preparing | Status::Progressing)
    }
}

/// What one tick of the machine did to one colony event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum RaidStep {
    /// `onStart` (`EventManager.java:198-201`). Fires on **every** tick while the
    /// status stays `STARTING`, because nothing changes it there.
    Started { raid: i32 },
    /// `onUpdate` (`:209`), for every status that is neither `STARTING`, `DONE`
    /// nor `CANCELED`.
    Updated { raid: i32 },
    /// `onFinish` then removal (`:191-197`).
    Finished { raid: i32 },
    /// Removal **without** `onFinish` (`:202-206`).
    Dropped { raid: i32 },
}

impl RaidStep {
    /// The Java method the step stands for.
    pub fn name(self) -> &'static str {
        match self {
            RaidStep::Started { .. } => "onStart",
            RaidStep::Updated { .. } => "onUpdate",
            RaidStep::Finished { .. } => "onFinish",
            RaidStep::Dropped { .. } => "removed",
        }
    }

    /// The colony event's id, which the port uses where Java uses the map key.
    pub fn raid(self) -> i32 {
        match self {
            RaidStep::Started { raid }
            | RaidStep::Updated { raid }
            | RaidStep::Finished { raid }
            | RaidStep::Dropped { raid } => raid,
        }
    }
}

/// One colony event: the part of `IColonyEvent` the machine reads.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Raid {
    /// `IColonyEvent.getID()` (`:34`).
    pub id: i32,
    /// `getEventTypeID()` (`:41`), `minecolonies:pirate_raid` style.
    pub kind: String,
    /// `getStatus()` (`:20`).
    pub status: Status,
}

impl Raid {
    /// A raid with its id already taken (`Lifecycle::take_id`).
    pub fn new(id: i32, kind: &str) -> Raid {
        Raid {
            id,
            kind: kind.to_owned(),
            status: Status::Starting,
        }
    }
}

/// The tick's margin before it wraps the counter (`EventManager.java:87`):
/// `if (currentEventID > Integer.MAX_VALUE - 100) currentEventID = 1;`.
pub const ID_CEILING_MARGIN: i32 = 100;
/// Where [`ID_CEILING_MARGIN`] comes from.
pub const ID_CEILING_MARGIN_SOURCE: &str = "core/colony/managers/EventManager.java:87";

/// The counter's first value (`EventManager.java:44`).
pub const FIRST_ID: i32 = 1;
/// Where [`FIRST_ID`] comes from.
pub const FIRST_ID_SOURCE: &str = "core/colony/managers/EventManager.java:44";

/// Why the machine refused a change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum LifecycleRefusal {
    /// An event with id zero. `addEvent` logs and returns (`:70-74`).
    NoId,
    /// The id names no stored event. Java dereferences `null` here; the port
    /// names it.
    Unknown(i32),
}

impl LifecycleRefusal {
    /// A stable name, for a log line and for a test.
    pub fn name(&self) -> &'static str {
        match self {
            LifecycleRefusal::NoId => "no_id",
            LifecycleRefusal::Unknown(_) => "unknown_event",
        }
    }
}

/// The counter step of `getAndTakeNextEventID` (`:85-95`), as a pure function:
/// the wrap check, then the increment, and the id is the value *before* the
/// increment — which is why the first id taken is [`FIRST_ID`] and not one more.
///
/// Returns `(id, counter after the call)`.
pub fn take_from(counter: i32) -> (i32, i32) {
    let counter = if counter > i32::MAX - ID_CEILING_MARGIN {
        FIRST_ID
    } else {
        counter
    };
    (counter, counter + 1)
}

/// One colony's event map and its tick: `EventManager`, cut to the events a
/// raid needs.
///
/// **Iteration order is the port's choice.** Java walks a `HashMap`
/// (`EventManager.java:54`, `:186`), whose order is unspecified; the trace is
/// worth nothing if two runs of the same save disagree, so [`BTreeMap`] by id it
/// is. Nothing in `onColonyTick` depends on the order the events are visited in.
#[derive(Clone, Debug)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Lifecycle {
    counter: i32,
    raids: BTreeMap<i32, Raid>,
}

impl Default for Lifecycle {
    fn default() -> Lifecycle {
        Lifecycle::new()
    }
}

impl Lifecycle {
    /// A fresh colony: counter at [`FIRST_ID`], no event (`:44`).
    pub fn new() -> Lifecycle {
        Lifecycle {
            counter: FIRST_ID,
            raids: BTreeMap::new(),
        }
    }

    /// `getAndTakeNextEventID` (`:85-95`), whole: wrap first, increment, hand
    /// back the value before the increment — so the first id taken is
    /// [`FIRST_ID`], not [`FIRST_ID`] + 1.
    pub fn take_id(&mut self) -> i32 {
        let (id, next) = take_from(self.counter);
        self.counter = next;
        id
    }

    /// `addEvent` (`:68-77`): id zero is refused, and an id already present is
    /// **overwritten** — `events.put` is a `HashMap` put, not a check.
    pub fn add(&mut self, raid: Raid) -> Result<(), LifecycleRefusal> {
        if raid.id == 0 {
            return Err(LifecycleRefusal::NoId);
        }
        self.raids.insert(raid.id, raid);
        Ok(())
    }

    /// `setStatus` (`IColonyEvent.java:27`).
    pub fn set_status(&mut self, id: i32, status: Status) -> Result<(), LifecycleRefusal> {
        match self.raids.get_mut(&id) {
            Some(raid) => {
                raid.status = status;
                Ok(())
            }
            None => Err(LifecycleRefusal::Unknown(id)),
        }
    }

    /// The status of one event, `None` when the colony does not hold it.
    pub fn status(&self, id: i32) -> Option<Status> {
        self.raids.get(&id).map(|raid| raid.status)
    }

    /// `IColonyRaidEvent.isRaidActive()` (`:50-55`) for a stored event.
    pub fn is_active(&self, id: i32) -> bool {
        self.status(id).map(Status::is_active).unwrap_or(false)
    }

    /// The ids the colony holds, ascending.
    pub fn ids(&self) -> Vec<i32> {
        self.raids.keys().copied().collect()
    }

    /// How many events the colony holds.
    pub fn len(&self) -> usize {
        self.raids.len()
    }

    /// Whether the colony holds no event at all.
    pub fn is_empty(&self) -> bool {
        self.raids.is_empty()
    }

    /// `onColonyTick` (`:184-212`): visit every event once, do its one thing,
    /// and record it in the trace.
    ///
    /// `DONE` finishes and leaves, `STARTING` starts and **stays**, `CANCELED`
    /// leaves silently, everything else updates.
    pub fn tick(&mut self, trace: &mut Trace) -> Vec<RaidStep> {
        let mut steps = Vec::new();
        for id in self.ids() {
            let Some(status) = self.status(id) else {
                continue;
            };
            let step = match status {
                Status::Done => {
                    self.raids.remove(&id);
                    RaidStep::Finished { raid: id }
                }
                Status::Canceled => {
                    self.raids.remove(&id);
                    RaidStep::Dropped { raid: id }
                }
                Status::Starting => RaidStep::Started { raid: id },
                Status::Preparing | Status::Progressing | Status::Waiting => {
                    RaidStep::Updated { raid: id }
                }
            };
            trace.note_step(step);
            steps.push(step);
        }
        steps
    }
}
