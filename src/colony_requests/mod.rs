//! The manager half of the request system: who may move a request, in what order
//! the colony looks for a supplier, and what bounds the looking.
//!
//! Java reference: `com.minecolonies.api.colony.requestsystem` and
//! `com.minecolonies.core.colony.requestsystem` — `token/` (the identity of a
//! request and of a resolver), `management/handlers/{RequestHandler,
//! ResolverHandler,TokenHandler}.java` (the assignment loop and its guards),
//! `request/RequestState.java` (the thirteen states and their order),
//! `resolvers/` (the priorities and what each resolver refuses),
//! `api/util/constant/RSConstants.java` (the numbers).
//!
//! **This module is not `src/requests/`, and does not redo it.** `src/requests/`
//! owns the request itself — its `Stack`, its five states, the queue that orders
//! them, and the pure choice among *candidate suppliers that hold stock*
//! (`requests/resolver.rs`, `choose`). What it deliberately leaves out, and what
//! is here instead:
//!
//! | File | Question | Java |
//! | --- | --- | --- |
//! | `token` | what identifies a request or a resolver, and what makes an id | `token/IToken.java`, `StandardToken.java`, `TokenHandler.java:48` |
//! | `assignment` | who holds a request, and which citizen works it | `IAssignmentDataStore.java`, `ResolverHandler.java:342-386`, `StandardRequestSystemBuildingDataStore.java:105-130` |
//! | `transition` | which state moves exist, and which actor may make each | `RequestState.java:18-78`, `RequestHandler.java:130-271`, `:517`, `AbstractRequest.java:366-370` |
//! | `chain` | in what order the resolvers are asked, and what one refuses | `RequestHandler.java:130-225`, `AbstractCraftingRequestResolver.java:226-252` |
//! | `bounds` | how many tries, how long, how deep, which priority wins | `StandardRetryingRequestResolver.java:31-32`, `Constants.java:108`, `AbstractDeliverymanRequestable.java:16-18` |
//!
//! **One thing this module states rather than ports.** Java has **no bound on
//! the number of requests a colony holds**: `WarehouseRequestQueueModule` is a
//! plain `List` that only ever grows (`:68-72`), and `ServerConfiguration` has no
//! request-system limit at all (`:114`, the only key under that heading is
//! `creativeResolve`). [`bounds::Capacity`] exists because a wasm guest has to
//! bound its own memory, and its default is [`bounds::Capacity::unbounded`],
//! which is exactly Java. The report says so too.
//!
//! **Three seams this module cannot close**, each refused by name instead of
//! being faked:
//!
//! * A resolver that is **not a building** (the player resolver, the retrying
//!   one) holds requests Java marks `ASSIGNED` with no position, while
//!   `requests::Request` stores its assignee as a `[i32; 3]` hut position and
//!   repairs "assigned with nobody named" back to `Created`
//!   (`requests/request.rs:356`). [`Refusal::NoPosition`] says so rather than
//!   inventing a position. `requests/mod.rs:22-27` already declares those two
//!   resolvers out of scope, for the host and the network they need.
//! * A player may **overrule** a request in any state — Java does not check
//!   (`StandardRequestManager.java:346-356`) — but the port's
//!   `Request::deliver` only accepts `Assigned` and `InProgress`
//!   (`requests/request.rs:274`). The overrule of a `Created` request is
//!   therefore refused by [`Refusal::Model`], carrying the model's own reason.
//! * `RequestState` saves an **index into the thirteen-value enum**
//!   (`RequestState.java:105-118`); the port's five-value state saves a name
//!   (`requests/request.rs:89-105`). Nothing here re-encodes it.
//!
//! **Data and rules, no host, no clock.** Like `src/requests/`, this module makes
//! no game call: a chain is a list of descriptors the caller built, a tick is an
//! integer a caller decrements, and a supply test is a trait the caller
//! implements. That is what lets every rule below be tested on the host target.

mod assignment;
mod bounds;
mod chain;
mod token;
mod transition;

pub use assignment::{Assignment, Book, Moved, Owner, WAITING};
pub use bounds::{
    Attempt, Capacity, Due, MAX_AGING_PRIORITY, MAX_BUILDING_PRIORITY, MAX_CRAFTING_CYCLE_DEPTH,
    MAX_TRIES, Origin, PLAYER_ACTION_PRIORITY, RETRY_DELAY_TICKS, Retries,
};
pub use chain::{Chain, Outcome, Resolver, Supply, Type, Wanted, creates_cycle};
pub use token::{Token, Tokens};
pub use transition::{Actor, End, Move, Step, advance, allowed, assign_to, child_started};
pub use transition::{holder_may, may, parent_can_resolve, progress_rank};

use crate::requests::Refusal as ModelRefusal;
use crate::requests::State;

/// Why the request system refused an operation, named.
///
/// **A variant is not a `lang` key.** The player-facing sentences live in
/// `assets/minecolonies/lang/`, which this module does not own; these names are
/// for logs and for tests, namespaced so they can never be mistaken for one.
///
/// **Twelve variants for twelve different bugs.** Java throws
/// `IllegalArgumentException` with a sentence for most of them
/// (`RequestHandler.java:64-68`, `:536-539`, `ResolverHandler.java:282-284`) and
/// silently returns for the rest; a caller that cannot tell "nobody took it yet"
/// from "another hut took it" retries the wrong thing, so each case is its own
/// case here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// The pair of states is not a move Java makes at all — nobody may.
    IllegalTransition { from: State, to: State },
    /// The move exists, but not for this actor: Java trusts its manager and has
    /// no such guard (`StandardRequestManager.java:294-332`).
    NotAllowed { to: State, by: Actor },
    /// The request model below refused, unchanged: its reason is more precise
    /// than anything this layer could restate.
    Model(ModelRefusal),
    /// Nothing holds the request. Java throws here
    /// (`ResolverHandler.java:401-404`).
    NotAssigned { id: i32 },
    /// Another resolver holds it; this one may not act on it.
    NotTheHolder { holder: Token },
    /// Two resolvers cannot hold the same request — the reverse lookup
    /// `getAssignmentForValue` silently answers the first map key it finds
    /// (`IAssignmentDataStore.java:32-35`), which would make the holder
    /// depend on iteration order.
    AlreadyAssigned { id: i32, holder: Token },
    /// The resolver token is unknown to this book
    /// (`ResolverHandler.java:280-285`).
    UnknownResolver { token: Token },
    /// A resolver that still holds requests cannot be dropped
    /// (`ResolverHandler.java:160-166`).
    ResolverInUse { token: Token, held: usize },
    /// The holder has no position, so `Request::assigned_to` cannot record it.
    /// See the module's third seam note.
    NoPosition { holder: Token },
    /// The retry budget is spent: three parks, then Java blacklists itself
    /// (`StandardRetryingRequestResolver.java:31`, `:159`).
    TryBudgetSpent { id: i32 },
    /// The port's own capacity guard; Java has no bound at all.
    QueueFull { limit: usize },
    /// The citizen book only moves a request that is parked under `-1`; a
    /// request with no entry is left alone (`StandardRequestSystemBuildingDataStore.java:108`).
    NotWaiting { id: i32 },
}

impl Refusal {
    /// A stable name for logs and tests, namespaced like a registry entry.
    pub fn name(self) -> &'static str {
        match self {
            Self::IllegalTransition { .. } => "requestsystem.refusal.transition",
            Self::NotAllowed { .. } => "requestsystem.refusal.actor",
            Self::Model(_) => "requestsystem.refusal.model",
            Self::NotAssigned { .. } => "requestsystem.refusal.unassigned",
            Self::NotTheHolder { .. } => "requestsystem.refusal.holder",
            Self::AlreadyAssigned { .. } => "requestsystem.refusal.assigned",
            Self::UnknownResolver { .. } => "requestsystem.refusal.unknown_resolver",
            Self::ResolverInUse { .. } => "requestsystem.refusal.in_use",
            Self::NoPosition { .. } => "requestsystem.refusal.no_position",
            Self::TryBudgetSpent { .. } => "requestsystem.refusal.budget",
            Self::QueueFull { .. } => "requestsystem.refusal.capacity",
            Self::NotWaiting { .. } => "requestsystem.refusal.not_waiting",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

impl From<ModelRefusal> for Refusal {
    fn from(cause: ModelRefusal) -> Refusal {
        Refusal::Model(cause)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_refusal_has_its_own_name_and_its_own_namespace() {
        let all = [
            Refusal::IllegalTransition { from: State::Created, to: State::Delivered },
            Refusal::NotAllowed { to: State::Assigned, by: Actor::Child },
            Refusal::Model(ModelRefusal::WrongItem),
            Refusal::NotAssigned { id: 1 },
            Refusal::NotTheHolder { holder: Token::new(1) },
            Refusal::AlreadyAssigned { id: 1, holder: Token::new(2) },
            Refusal::UnknownResolver { token: Token::new(3) },
            Refusal::ResolverInUse { token: Token::new(4), held: 1 },
            Refusal::NoPosition { holder: Token::new(5) },
            Refusal::TryBudgetSpent { id: 6 },
            Refusal::QueueFull { limit: 7 },
            Refusal::NotWaiting { id: 8 },
        ];
        for (index, refusal) in all.iter().enumerate() {
            for other in &all[index + 1..] {
                assert_ne!(refusal.name(), other.name(), "two refusals share a name");
            }
            assert!(refusal.name().starts_with("requestsystem.refusal."));
        }
        assert_eq!(
            Refusal::QueueFull { limit: 7 }.to_string(),
            "requestsystem.refusal.capacity"
        );
    }

    #[test]
    fn a_model_refusal_is_carried_whole_rather_than_restated() {
        let inner = ModelRefusal::WrongItem;
        let wrapped: Refusal = inner.into();
        assert_eq!(wrapped, Refusal::Model(ModelRefusal::WrongItem));
        assert_eq!(wrapped.name(), "requestsystem.refusal.model");
        // The inner reason survives the trip, so a caller keeps the precision.
        assert!(matches!(wrapped, Refusal::Model(ModelRefusal::WrongItem)));
    }
}
