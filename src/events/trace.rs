//! What a run left behind: the events, who received them, in which order, and
//! what the raid machine did.
//!
//! Java logs (`DefaultEventBus.java:38`, `:48`) and remembers nothing; a test
//! cannot read a log, and a mod that wants to explain "why did nobody react to my
//! colony being deleted" has nothing to look at. The trace is that memory, and it
//! is built to be **compared**: the same events replayed into a bus wired the same
//! way give the same trace (`bus::EventBus::replay`).
//!
//! Three lists, in the order they matter to a proof:
//!
//! - [`Trace::posted`] — the events, as they were **before** any handler ran, with
//!   the ids the bus gave them. This is the replay source.
//! - [`Trace::records`] — one entry per (event, subscription) delivery, plus its
//!   outcome. This is the answer to "who heard it, and in which order".
//! - [`Trace::notes`] — what the raid machine did on each tick
//!   ([`super::raid::RaidStep`]), which is *not* a bus event: see `raid`'s header.

use super::bus::{HandlerError, SubscriberId};
use super::payload::{ColonieEvent, EventId};
use super::raid::RaidStep;
use super::topic::Topic;

/// One delivery, or one attempt at delivering that a handler refused.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Record {
    /// Position in the trace: 0 is the first delivery of the whole run.
    pub seq: usize,
    /// The event's id, which links back to [`Trace::posted`].
    pub event: EventId,
    /// Its class.
    pub topic: Topic,
    /// Which subscription was called.
    pub subscriber: SubscriberId,
    /// Its label, repeated here so a trace reads on its own.
    pub label: &'static str,
    /// What it answered.
    pub outcome: Outcome,
}

/// A handler's answer, as Java would have seen it: `void`, or an exception the
/// bus swallows and logs (`DefaultEventBus.java:42-50`).
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum Outcome {
    /// The handler returned.
    Applied,
    /// The handler failed; the bus recorded it and went on with the next one.
    Failed(HandlerError),
}

/// The record of a run.
#[derive(Clone, Debug, Default, PartialEq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Trace {
    posted: Vec<ColonieEvent>,
    records: Vec<Record>,
    notes: Vec<RaidStep>,
}

impl Trace {
    /// The events posted, in order, as they were before delivery.
    pub fn posted(&self) -> &[ColonieEvent] {
        &self.posted
    }

    /// Every delivery, in order.
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    /// What the raid machine did, tick after tick.
    pub fn notes(&self) -> &[RaidStep] {
        &self.notes
    }

    /// How many deliveries were made.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether nothing was delivered (an event with no subscriber still lands in
    /// [`Trace::posted`]).
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The posted events of one topic.
    pub fn events_of(&self, topic: Topic) -> Vec<&ColonieEvent> {
        self.posted
            .iter()
            .filter(|event| event.topic() == topic)
            .collect()
    }

    /// The deliveries of one event, in order.
    pub fn deliveries(&self, event: EventId) -> Vec<&Record> {
        self.records
            .iter()
            .filter(|record| record.event == event)
            .collect()
    }

    /// **The order assertion**: which subscriptions heard one event, in which
    /// order.
    pub fn sequence(&self, event: EventId) -> Vec<SubscriberId> {
        self.deliveries(event)
            .into_iter()
            .map(|record| record.subscriber)
            .collect()
    }

    /// The same, by label, which is what a test reads.
    pub fn labels(&self, event: EventId) -> Vec<&'static str> {
        self.deliveries(event)
            .into_iter()
            .map(|record| record.label)
            .collect()
    }

    /// The deliveries that failed, with the error each handler gave.
    pub fn failures(&self) -> Vec<(&'static str, HandlerError)> {
        self.records
            .iter()
            .filter_map(|record| match &record.outcome {
                // Read through the reference, then clone the one owned field.
                Outcome::Failed(error) => Some((record.label, error.clone())),
                Outcome::Applied => None,
            })
            .collect()
    }

    /// The events whose first delivery was to `subscriber`, in order: "what this
    /// mod heard, and when".
    pub fn heard_by(&self, subscriber: SubscriberId) -> Vec<(EventId, Topic)> {
        self.records
            .iter()
            .filter(|record| record.subscriber == subscriber)
            .map(|record| (record.event, record.topic))
            .collect()
    }

    /// The event the bus gave `id`, if it posted it.
    pub fn event(&self, id: EventId) -> Option<&ColonieEvent> {
        self.posted.iter().find(|event| event.id() == id)
    }

    /// Records a post. Called by the bus before any handler runs.
    pub(super) fn note_posted(&mut self, event: ColonieEvent) {
        self.posted.push(event);
    }

    /// Records one delivery. Called by the bus after each handler answers.
    pub(super) fn note_delivered(
        &mut self,
        event: EventId,
        topic: Topic,
        subscriber: SubscriberId,
        label: &'static str,
        outcome: Outcome,
    ) {
        self.records.push(Record {
            seq: self.records.len(),
            event,
            topic,
            subscriber,
            label,
            outcome,
        });
    }

    /// Records one lifecycle step. Called by [`super::raid::Lifecycle::tick`].
    pub(super) fn note_step(&mut self, step: RaidStep) {
        self.notes.push(step);
    }
}
