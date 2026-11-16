//! Delivery: who a post reaches, in which order, and what happens when a
//! subscriber loops.
//!
//! Java's whole implementation is 51 lines (`DefaultEventBus.java`), and it is
//! reproduced rule by rule:
//!
//! | Java | Here |
//! | --- | --- |
//! | a handler list per **exact class** (`:32`) | one list per [`Topic`], no supertype lookup |
//! | `List.add` at the end (`:26`), iterated in that order (`:40`) | registration order; the sort only exists for Forge's bus ([`Priority`]) |
//! | `catch (Exception)` around **each** handler, log, carry on (`:42-50`) | a handler answers `Result`; an `Err` is recorded in the trace and the next handler runs |
//! | no handler for the class → return (`:33-35`) | `Ok` with an empty [`Delivery`] |
//! | nothing guards re-entry | three named [`Refusal`]s — Java recurses until `StackOverflowError` |
//!
//! **Java's one bug the port cannot reproduce.** Subscribing from inside a
//! handler mutates the `ArrayList` the `for` loop is iterating
//! (`:26` + `:40`), and the resulting `ConcurrentModificationException` is
//! thrown by the iterator **outside** the `try`, so it escapes `post` and stops
//! the remaining handlers. In the port a handler is handed a [`Ctx`] that only
//! posts: there is no way to subscribe mid-delivery, so that failure mode is
//! structurally gone rather than emulated. `MANQUANT.md` §2 records the trade.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use super::payload::{ColonieEvent, EventId};
use super::topic::{Bus, Family, Topic};
use super::trace::{Outcome, Trace};

/// How deep a chain of nested posts may go before it is refused.
///
/// **No Java equivalent**: `DefaultEventBus.post` has no depth guard (`:30-51`),
/// and a cycle across topics ends in a `StackOverflowError` — an `Error`, so the
/// `catch (Exception)` at `:46` does not even see it. The port needs a number
/// because a wasm stack is small and because a name is more useful than a trap.
pub const DEPTH_LIMIT: usize = 8;

/// Where [`DEPTH_LIMIT`] comes from: this port, not Java.
pub const DEPTH_LIMIT_SOURCE: &str =
    "port-only: DefaultEventBus.java:30-51 has no depth guard; StackOverflowError is an Error, \
     not caught at :46";

/// An identity for one subscription, stable for the bus's life.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct SubscriberId(pub u32);

/// Craft's ordering, the one `@SubscribeEvent(priority = ...)` names
/// (`journeymap/v6/EventListener.java:61,64`, `QuestObjectiveEventHandler.java:61`).
///
/// **Only Forge's bus has it.** MineColonies' own `subscribe` takes a class and a
/// handler and nothing else (`EventBus.java:17`); a priority there would be an
/// invention, which is why `subscribe_by_priority` refuses a mod-bus topic
/// ([`Refusal::WrongBus`]). Dispatch order is `Highest` first — Forge's own
/// contract, not something this tree states.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: mirrors Forge's enum, not a `mod_api` type
pub enum Priority {
    /// `EventPriority.HIGHEST`: first.
    Highest,
    /// `EventPriority.HIGH`.
    High,
    /// `EventPriority.NORMAL`, what `@SubscribeEvent` defaults to.
    Normal,
    /// `EventPriority.LOW` (`journeymap/v6/EventListener.java:61`).
    Low,
    /// `EventPriority.LOWEST` (`QuestObjectiveEventHandler.java:61`): last, so
    /// quests read the world after everyone else had a say.
    Lowest,
}

/// Why a post did not happen, or why a subscription went nowhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum Refusal {
    /// A handler posted a topic that is already being delivered.
    ///
    /// This is the loop: Java re-enters `post` on the same list (`:40-44`) and
    /// recurses until the stack dies. `depth` is how many posts were already
    /// open.
    Reentered { topic: Topic, depth: usize },
    /// Nested posts went deeper than [`DEPTH_LIMIT`] through *distinct* topics:
    /// not a loop yet, but a chain the port will not follow.
    TooDeep { depth: usize, limit: usize },
    /// A priority was asked for a topic that rides the mod bus, where Java's
    /// `subscribe` has no such parameter (`EventBus.java:17`).
    WrongBus { topic: Topic, bus: Bus },
}

impl Refusal {
    /// A stable name, for a log line and for a test's assertion.
    pub fn name(&self) -> &'static str {
        match self {
            Refusal::Reentered { .. } => "reentered",
            Refusal::TooDeep { .. } => "too_deep",
            Refusal::WrongBus { .. } => "wrong_bus",
        }
    }
}

/// What a handler answers. Java's handlers return `void` and fail by throwing
/// (`:46-49`); the port's return `Err` instead, and the bus treats it the same
/// way — record it, keep going.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct HandlerError(pub &'static str);

/// What a `post` did: which subscribers were called, and which of them failed.
///
/// Java's `post` returns `void` and leaves that in the log (`:38`, `:48`); a test
/// cannot read a log.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Delivery {
    /// The event's id, assigned by this post when the caller left it unassigned.
    pub event: EventId,
    /// Its class.
    pub topic: Topic,
    /// Every subscriber called, in call order — one entry per subscription.
    pub called: Vec<SubscriberId>,
    /// The subset of `called` that answered `Err`.
    pub failed: Vec<SubscriberId>,
}

/// A subscription that Java accepts and never delivers to.
///
/// `DefaultEventBus.subscribe` takes any `Class<T>` (`:22`), and `post` looks up
/// `event.getClass()` (`:32`), so two mistakes compile and do nothing:
/// subscribing to an abstract class, and subscribing to a class this bus never
/// sees posted. The port lists them instead of dropping them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum Inert {
    /// An abstract class: no event ever *is* it (`:32`).
    Family(Family),
    /// A topic whose events ride the other bus ([`Bus`]).
    ForeignBus(Topic),
}

type BoxedHandler = Box<dyn FnMut(&mut ColonieEvent, &Ctx) -> Result<(), HandlerError>>;

struct Subscription {
    id: SubscriberId,
    priority: Priority,
    label: &'static str,
    handler: BoxedHandler,
}

/// What a handler gets besides the event: the ability to post, and nothing else.
///
/// Java's handler holds the API singleton and can do anything with it; the port
/// hands over one verb, because posting is the only thing a colony handler
/// legitimately does through the bus (`Permissions` → `Colony` → …).
#[derive(Clone)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Ctx {
    inner: Rc<RefCell<Inner>>,
}

impl Ctx {
    /// Posts, exactly as `EventBus::post` does, from inside a handler.
    pub fn post(&self, event: &mut ColonieEvent) -> Result<Delivery, Refusal> {
        post_into(&self.inner, event)
    }
}

/// The bus. Cloneable: every clone is the same bus, which is what lets a
/// handler hold a [`Ctx`] while the caller keeps posting.
#[derive(Clone, Default)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct EventBus {
    inner: Rc<RefCell<Inner>>,
}

#[derive(Default)]
struct Inner {
    next_subscriber: u32,
    next_event: u64,
    by_topic: BTreeMap<Topic, Vec<Subscription>>,
    inert: Vec<(SubscriberId, Inert, &'static str)>,
    stack: Vec<(EventId, Topic)>,
    trace: Trace,
}

impl EventBus {
    /// An empty bus: no subscriber, no trace.
    pub fn new() -> EventBus {
        EventBus::default()
    }

    /// `DefaultEventBus.subscribe` (`:22-27`): append, never replace.
    ///
    /// The label stands in for Java's method reference — a trace has to say
    /// *who* was called (`journeymap-deathpoints`, `colony-rename-notice`), and a
    /// `FnMut` cannot name itself.
    ///
    /// Subscribing the same handler twice gives two subscriptions and two calls,
    /// which is Java's own behaviour (`:26` appends, `:40` iterates the list).
    pub fn subscribe<F>(&mut self, topic: Topic, label: &'static str, handler: F) -> SubscriberId
    where
        F: FnMut(&mut ColonieEvent, &Ctx) -> Result<(), HandlerError> + 'static,
    {
        let mut inner = self.inner.borrow_mut();
        let id = inner.take_subscriber_id();
        if topic.bus() == Bus::Forge {
            inner.inert.push((id, Inert::ForeignBus(topic), label));
            return id;
        }
        inner
            .by_topic
            .entry(topic)
            .or_default()
            .push(Subscription {
                id,
                priority: Priority::Normal,
                label,
                handler: Box::new(handler),
            });
        id
    }

    /// Subscribes to an abstract class, which **never fires** — Java's `:32`
    /// looks the event's own class up, and no event's class is an abstract one.
    ///
    /// Kept rather than refused because that is what Java does; the subscription
    /// shows up in [`EventBus::inert`] so a port that inherited the mistake can
    /// see it, and `Family::topics` says which concrete topics it probably
    /// wanted.
    pub fn subscribe_family<F>(&mut self, family: Family, label: &'static str, handler: F) -> SubscriberId
    where
        F: FnMut(&mut ColonieEvent, &Ctx) -> Result<(), HandlerError> + 'static,
    {
        let mut inner = self.inner.borrow_mut();
        let id = inner.take_subscriber_id();
        inner.inert.push((id, Inert::Family(family), label));
        let _ = handler;
        id
    }

    /// Subscribes on Forge's bus, with a priority. Only a [`Bus::Forge`] topic is
    /// accepted: the mod bus has no ordering but registration (`EventBus.java:17`).
    pub fn subscribe_by_priority<F>(
        &mut self,
        topic: Topic,
        priority: Priority,
        label: &'static str,
        handler: F,
    ) -> Result<SubscriberId, Refusal>
    where
        F: FnMut(&mut ColonieEvent, &Ctx) -> Result<(), HandlerError> + 'static,
    {
        if topic.bus() != Bus::Forge {
            return Err(Refusal::WrongBus {
                topic,
                bus: topic.bus(),
            });
        }
        let mut inner = self.inner.borrow_mut();
        let id = inner.take_subscriber_id();
        let list = inner.by_topic.entry(topic).or_default();
        // Forge inserts by priority, registration order inside one priority:
        // find the first entry that sorts after this one.
        let at = list
            .iter()
            .position(|other| other.priority > priority)
            .unwrap_or(list.len());
        list.insert(
            at,
            Subscription {
                id,
                priority,
                label,
                handler: Box::new(handler),
            },
        );
        Ok(id)
    }

    /// Posts to the topic's subscribers, once each, in order (`:30-51`).
    ///
    /// `&self`: the bus's state is inside it, so a handler can post while the
    /// caller still holds the bus. The event is **borrowed, not consumed** — the
    /// caller reads it again after the call, which is what `Colony.java:1643`
    /// does with a `PlayerEnteringModEvent`'s veto.
    pub fn post(&self, event: &mut ColonieEvent) -> Result<Delivery, Refusal> {
        post_into(&self.inner, event)
    }

    /// Re-posts what a trace recorded, in the recorded order.
    ///
    /// Ids are carried by the events themselves and kept by [`EventBus::post`],
    /// so a replay into a bus wired with the same subscribers produces the same
    /// [`Trace`] — that equality is what makes a run *provable* rather than
    /// described.
    pub fn replay(&self, trace: &Trace) -> Result<Vec<Delivery>, Refusal> {
        let mut out = Vec::with_capacity(trace.posted().len());
        for recorded in trace.posted() {
            let mut event = recorded.clone();
            out.push(self.post(&mut event)?);
        }
        Ok(out)
    }

    /// The subscribers of one topic, in dispatch order.
    pub fn subscribers(&self, topic: Topic) -> Vec<(SubscriberId, &'static str)> {
        self.inner
            .borrow()
            .by_topic
            .get(&topic)
            .map(|list| list.iter().map(|sub| (sub.id, sub.label)).collect())
            .unwrap_or_default()
    }

    /// The label of one subscription, for a trace assertion that knows the id.
    pub fn label_of(&self, id: SubscriberId) -> Option<&'static str> {
        let inner = self.inner.borrow();
        inner
            .by_topic
            .values()
            .flat_map(|list| list.iter())
            .find(|sub| sub.id == id)
            .map(|sub| sub.label)
            .or_else(|| {
                inner
                    .inert
                    .iter()
                    .find(|(entry, _, _)| *entry == id)
                    .map(|(_, _, label)| *label)
            })
    }

    /// The subscriptions that can never fire, with the reason.
    pub fn inert(&self) -> Vec<(SubscriberId, Inert, &'static str)> {
        self.inner.borrow().inert.clone()
    }

    /// What the bus did so far. A copy: the bus keeps its own.
    pub fn trace(&self) -> Trace {
        self.inner.borrow().trace.clone()
    }

    /// The trace, taken out of the bus.
    pub fn take_trace(&mut self) -> Trace {
        let mut inner = self.inner.borrow_mut();
        std::mem::take(&mut inner.trace)
    }

    /// A handle a handler can use to post.
    pub fn ctx(&self) -> Ctx {
        Ctx {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl Inner {
    fn take_subscriber_id(&mut self) -> SubscriberId {
        let id = SubscriberId(self.next_subscriber);
        self.next_subscriber += 1;
        id
    }
}

/// Puts the subscribers back and closes the post, even if a handler panics.
///
/// Java's `catch (Exception)` (`:46-49`) does not cover a Rust panic, and the
/// list has to leave the map to be iterated while a handler is allowed to post.
/// Without this guard a panic would leave the topic with no subscribers at all.
struct Open<'a> {
    inner: &'a Rc<RefCell<Inner>>,
    topic: Topic,
    subs: Vec<Subscription>,
}

impl Drop for Open<'_> {
    fn drop(&mut self) {
        let subs = std::mem::take(&mut self.subs);
        let mut inner = self.inner.borrow_mut();
        inner.by_topic.insert(self.topic, subs);
        inner.stack.pop();
    }
}

fn post_into(inner: &Rc<RefCell<Inner>>, event: &mut ColonieEvent) -> Result<Delivery, Refusal> {
    let topic = event.topic();
    {
        let state = inner.borrow();
        if state.stack.iter().any(|(_, open)| *open == topic) {
            return Err(Refusal::Reentered {
                topic,
                depth: state.stack.len(),
            });
        }
        if state.stack.len() >= DEPTH_LIMIT {
            return Err(Refusal::TooDeep {
                depth: state.stack.len(),
                limit: DEPTH_LIMIT,
            });
        }
    }

    let id = {
        let mut state = inner.borrow_mut();
        let id = if event.id().is_unassigned() {
            state.next_event += 1;
            let fresh = EventId::new(state.next_event);
            event.set_id(fresh);
            fresh
        } else {
            event.id()
        };
        state.stack.push((id, topic));
        state.trace.note_posted(event.clone());
        id
    };

    let subs = inner.borrow_mut().by_topic.remove(&topic).unwrap_or_default();
    let mut open = Open {
        inner,
        topic,
        subs,
    };

    let mut called = Vec::with_capacity(open.subs.len());
    let mut failed = Vec::new();
    {
        let ctx = Ctx {
            inner: Rc::clone(inner),
        };
        for sub in open.subs.iter_mut() {
            called.push(sub.id);
            let outcome = match (sub.handler)(event, &ctx) {
                Ok(()) => Outcome::Applied,
                Err(error) => {
                    failed.push(sub.id);
                    Outcome::Failed(error)
                }
            };
            inner
                .borrow_mut()
                .trace
                .note_delivered(id, topic, sub.id, sub.label, outcome);
        }
    }
    drop(open);

    Ok(Delivery {
        event: id,
        topic,
        called,
        failed,
    })
}
