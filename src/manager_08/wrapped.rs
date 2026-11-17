//! A wrapped request manager: one state, shared, and one method it refuses.
//!
//! Java reference: `core/colony/requestsystem/management/manager/wrapped/`
//! `AbstractWrappedRequestManager.java` (251 lines) over
//! `api/colony/requestsystem/manager/IRequestManager.java`.
//!
//! **Twenty-five of its methods are the same line.**
//! `createRequest`, `assignRequest`, `getRequestForToken`, `updateRequestState`,
//! `serializeNBT`, `tick`, `isDirty` … (`:38-249`) all answer
//! `wrappedManager.<same>(…)`. A delegation that copies no state cannot disagree
//! with the manager it wraps, so there is no rule to port in them, and this file
//! does not respell them: [`Registry`] *is* the shared state, and the wrappers in
//! `assign` and `suppress` borrow it instead of forwarding to it.
//!
//! **Two of them are not the same line, and both are ported.**
//!
//! - [`on_colony_update`] throws
//!   `UnsupportedOperationException("This method cannot be used by Wrapped Request
//!   Managers!")` (`:250-254`) — the one method a wrapper may never answer.
//! - [`create_and_assign`] is the abstract wrapper's *template* method
//!   (`:113-121`): it calls `createRequest` and then `assignRequest`, and because
//!   Java's call is virtual it dispatches to the subclass's override. That is what
//!   makes a blacklisting wrapper blacklist the very first assignment, which is the
//!   only reason `WrappedBlacklistAssignmentRequestManager` works at all. In Rust
//!   the same guarantee is a parameter, and the tests of `assign` read it.
//!
//! **And one method answers differently from its sibling.**
//! `getRequestForToken` is deliberately lenient — it calls
//! `getRequestHandler().getRequestOrNull(token)` (`:139-147`) — where
//! `getRequest` throws on an unknown token (`RequestHandler.java:126`). The port
//! keeps both answers, [`Registry::request`] and [`Registry::require`], because a
//! caller that cannot tell them apart retries the wrong thing.

use std::collections::BTreeMap;

use crate::requests::{Queue, Refusal as RequestRefusal, Request, State};

use super::Refusal;

/// A resolver, as this port can name one.
///
/// Java's token is an opaque UUID (`api/colony/requestsystem/token/IToken.java:10-30`)
/// and *every* resolver, request and provider has one. The port's resolvers are
/// huts and **a hut is its position** (`building/instance.rs:10-17`,
/// `AbstractSchematicProvider.java:290-294`), so a token is a position here.
///
/// The cost, named: two resolvers standing at the same position — a warehouse that
/// is also a crafting hut — cannot be blacklisted apart, where Java's can. No such
/// pair exists in the port's candidate set (`src/requests/resolver.rs:82-116`), and
/// `MANQUANT.md` §1 keeps the count.
pub type Resolver = [i32; 3];

/// What a manager does *around* a write, in the order Java does it.
///
/// The journal exists so "nothing is written before a refusal" is a test and not a
/// comment: an operation that refuses leaves this list empty.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four things Java tells the world about one state change
pub enum Notice {
    /// `manager.log("Updated state from: …")` (`AbstractRequest.java:140`).
    Logged { id: i32 },
    /// `manager.getRequestForToken(getParent()).childStateUpdated(manager, id)`
    /// (`AbstractRequest.java:142-153`).
    ParentTold { child: i32, parent: i32 },
    /// The resolver holds the request now —
    /// `getResolverHandler().addRequestToResolver(resolver, request)`
    /// (`RequestHandler.java:241`) followed by `onRequestAssigned` (`:243`).
    ResolverTold { id: i32, by: Resolver },
    /// `markDirty()`, and through it the colony's own flag
    /// (`StandardRequestManager.java:213-221`, called at `:233`, `:258`, `:302`).
    Dirty,
}

impl Notice {
    pub fn name(self) -> &'static str {
        match self {
            Self::Logged { .. } => "manager_08.notice.logged",
            Self::ParentTold { .. } => "manager_08.notice.parent",
            Self::ResolverTold { .. } => "manager_08.notice.resolver",
            Self::Dirty => "manager_08.notice.dirty",
        }
    }
}

/// The state every wrapped manager shares: the requests, the parents, and the two
/// things a write leaves behind.
///
/// Java's base manager owns nine data stores (`StandardRequestManager.java:60-160`);
/// this is the one a wrapper can observe — the requests themselves
/// (`crate::requests::Queue`, which already owns ids, arrival order and the
/// compound) — plus what a notification is worth.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Registry {
    requests: Queue,
    /// Java's `getParent()` on a request (`AbstractRequest.java:146`). The *chain*
    /// — a parent that waits for its children — is not ported
    /// (`src/requests/mod.rs:26-28`); this one link is here because it is the write
    /// the static manager exists to silence (`suppress`).
    parents: BTreeMap<i32, i32>,
    notices: Vec<Notice>,
    dirty: bool,
}

impl Registry {
    pub fn new() -> Registry {
        Registry::default()
    }

    /// Registers a request, refusing an id that is already taken
    /// (`WarehouseRequestQueueModule.java:82-86` through `Queue::push`): serving
    /// one id twice would deliver twice.
    pub fn register(&mut self, request: Request) -> Result<(), Refusal> {
        self.requests.push(request).map_err(Refusal::Request)
    }

    /// The lenient lookup — `getRequestForToken` → `getRequestOrNull`
    /// (`AbstractWrappedRequestManager.java:139-147`).
    pub fn request(&self, id: i32) -> Option<&Request> {
        self.requests.get(id)
    }

    pub fn request_mut(&mut self, id: i32) -> Option<&mut Request> {
        self.requests.get_mut(id)
    }

    /// The strict lookup — `getRequest(token)`, which throws
    /// (`RequestHandler.java:126`).
    pub fn require(&self, id: i32) -> Result<&Request, Refusal> {
        self.request(id).ok_or(Refusal::UnknownRequest { id })
    }

    pub fn len(&self) -> usize {
        self.requests.len()
    }

    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    /// Says `child` has `parent`, which is `childRequest.setParent(request.getId())`
    /// (`RequestHandler.java:251`).
    pub fn adopt(&mut self, child: i32, parent: i32) {
        self.parents.insert(child, parent);
    }

    pub fn parent_of(&self, child: i32) -> Option<i32> {
        self.parents.get(&child).copied()
    }

    /// The journal of the last operation, in the order it was written.
    pub fn notices(&self) -> &[Notice] {
        &self.notices
    }

    pub fn dirty(&self) -> bool {
        self.dirty
    }

    /// Records a notification. [`Notice::Dirty`] is the only one that is also a
    /// flag: Java's `markDirty()` sets the field, the other three write a line or
    /// call a method.
    pub fn tell(&mut self, notice: Notice) {
        if notice == Notice::Dirty {
            self.dirty = true;
        }
        self.notices.push(notice);
    }

    /// Forgets the journal and the dirty flag, so a test can read one operation at
    /// a time. The requests and the parents stay: only what was *said* is cleared.
    pub fn settle(&mut self) {
        self.notices.clear();
        self.dirty = false;
    }

    /// A resolver takes the request, in Java's order.
    ///
    /// The state machine first (`Created → Assigned`, `requests/request.rs:235-246`)
    /// — a refusal there leaves nothing behind, which is the whole point of doing it
    /// first — then what Java tells the world:
    ///
    /// 1. `getResolverHandler().addRequestToResolver(resolver, request)` (`:241`) and
    ///    `resolver.onRequestAssigned(...)` (`:243`): the resolver holds it.
    /// 2. **Nothing about the state.** `RequestHandler.resolve` writes it through the
    ///    *static* manager (`:268`), so no log line is written and no parent is
    ///    told — that is the silence `suppress` is about, and this journal shows it.
    /// 3. `markDirty()` last, in the manager's own wrapper
    ///    (`StandardRequestManager.java:230-234`), after the search and after the
    ///    state write.
    pub fn assign(&mut self, id: i32, by: Resolver) -> Result<(), Refusal> {
        {
            let Some(request) = self.requests.get_mut(id) else {
                return Err(Refusal::UnknownRequest { id });
            };
            request.assign(by).map_err(Refusal::Request)?;
        }
        self.tell(Notice::ResolverTold { id, by });
        self.tell(Notice::Dirty);
        Ok(())
    }
}

/// `AbstractWrappedRequestManager.onColonyUpdate` (`:250-254`).
///
/// A wrapper cannot re-assign through itself: the callback exists so a manager
/// reconsiders the requests a colony's stock changed for, and a wrapper is a
/// *temporary* manager handed to one resolver during one search
/// (`RequestHandler.java:190`, `:203`). Answering it would run the search again
/// from inside the search.
///
/// The registry is taken by shared reference **on purpose**: a method that can only
/// read cannot write before it refuses, and the test of `mod.rs` reads the empty
/// journal to prove it.
pub fn on_colony_update(registry: &Registry) -> Result<(), Refusal> {
    let _ = registry;
    Err(Refusal::NotDelegated { method: "onColonyUpdate" })
}

/// The abstract wrapper's template method (`:113-121`): create, then assign.
///
/// `assign` is the caller's `assignRequest`, so a blacklisting wrapper's own rule
/// applies to the first assignment too — Java gets that from a virtual call, this
/// port from a parameter.
///
/// **A refused assignment does not undo the creation**, and that is Java: the
/// template has no `catch`, so the request is registered and unassigned when the
/// search finds nobody (`RequestHandler.java:276` returns `null` with the request
/// already in the store). The port keeps that order — [`Registry::register`] runs
/// before the search — so a refused search leaves the request in the registry and
/// [`Registry::len`] still counts it.
pub fn create_and_assign(
    registry: &mut Registry,
    request: Request,
    assign: impl FnOnce(&mut Registry, i32) -> Result<Resolver, Refusal>,
) -> Result<(i32, Resolver), Refusal> {
    let id = request.id();
    registry.register(request)?;
    let resolver = assign(registry, id)?;
    Ok((id, resolver))
}

/// The state a request is in, for a caller that only wants to know whether a
/// refused operation moved anything.
pub fn state_of(registry: &Registry, id: i32) -> Option<State> {
    registry.request(id).map(Request::state)
}

/// Whether the request system's own refusal was a state-machine refusal, which is
/// what a caller retries differently from a missing request.
pub fn is_transition(refusal: Refusal) -> bool {
    matches!(
        refusal,
        Refusal::Request(RequestRefusal::IllegalTransition { .. })
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::ItemName;
    use crate::requests::Requester;

    const HUT: [i32; 3] = [10, 64, 10];
    const OTHER: [i32; 3] = [20, 64, 10];

    fn asking(id: i32) -> Request {
        let item = ItemName::new("minecraft:ladder").expect("a valid name");
        Request::new(id, item, 4, 4, Requester::new("minecolonies:builder", HUT))
            .expect("a nonzero request")
    }

    #[test]
    fn a_wrapper_shares_one_state_and_a_write_through_it_is_seen_by_the_base() {
        let mut registry = Registry::new();
        registry.register(asking(1)).expect("a fresh register");
        assert_eq!(registry.len(), 1, "the wrapper holds no copy: this is the state");
        assert!(registry.assign(1, OTHER).is_ok(), "the wrapper writes here");
        assert_eq!(
            state_of(&registry, 1),
            Some(State::Assigned),
            "the base reads what the wrapper wrote"
        );
        assert_eq!(registry.request(1).and_then(Request::assigned_to), Some(OTHER));
        assert!(registry.dirty(), "and the dirty flag is the base's too");
    }

    #[test]
    fn on_colony_update_is_the_one_method_a_wrapper_refuses() {
        let mut registry = Registry::new();
        registry.register(asking(1)).expect("a fresh register");
        assert_eq!(
            on_colony_update(&registry),
            Err(Refusal::NotDelegated { method: "onColonyUpdate" })
        );
        assert!(registry.notices().is_empty(), "a refusal announces nothing");
        assert!(!registry.dirty());
        assert_eq!(state_of(&registry, 1), Some(State::Created), "and moves nothing");
    }

    #[test]
    fn the_two_lookups_answer_differently_on_an_unknown_token() {
        let registry = Registry::new();
        assert_eq!(registry.request(9), None, "getRequestForToken is lenient");
        assert_eq!(
            registry.require(9).err(),
            Some(Refusal::UnknownRequest { id: 9 }),
            "getRequest throws"
        );
    }

    #[test]
    fn a_request_takes_one_resolver_and_the_journal_names_every_write_in_order() {
        let mut registry = Registry::new();
        registry.register(asking(1)).expect("a fresh register");
        registry.adopt(1, 7);
        registry.assign(1, HUT).expect("the hut takes it");
        assert_eq!(
            registry.notices(),
            &[Notice::ResolverTold { id: 1, by: HUT }, Notice::Dirty],
            "RequestHandler.java:241-243, then StandardRequestManager.java:233 — and no parent, \
             because the state write of :268 goes through the static manager"
        );
        assert_eq!(
            registry.assign(1, OTHER),
            Err(Refusal::Request(crate::requests::Refusal::AlreadyAssigned { by: HUT })),
            "a second resolver is the state machine's refusal, passed through"
        );
    }

    #[test]
    fn a_creation_that_is_refused_keeps_the_request_and_moves_no_state() {
        let mut registry = Registry::new();
        let out = create_and_assign(&mut registry, asking(1), |registry, id| {
            registry.require(id)?;
            Err(Refusal::NoResolver)
        });
        assert_eq!(out, Err(Refusal::NoResolver));
        assert_eq!(registry.len(), 1, "Java's template has no rollback either");
        assert_eq!(state_of(&registry, 1), Some(State::Created));
        assert!(registry.notices().is_empty());
        assert!(is_transition(Refusal::Request(
            crate::requests::Refusal::IllegalTransition {
                from: State::Delivered,
                to: State::Assigned,
            }
        )));
        assert!(!is_transition(Refusal::NoResolver));
    }

    #[test]
    fn a_taken_id_is_refused_and_the_incumbent_stands() {
        let mut registry = Registry::new();
        registry.register(asking(1)).expect("a fresh register");
        assert_eq!(
            registry.register(asking(1)).err(),
            Some(Refusal::Request(crate::requests::Refusal::Duplicate { id: 1 }))
        );
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn settling_clears_what_was_said_and_not_what_is_held() {
        let mut registry = Registry::new();
        registry.register(asking(1)).expect("a fresh register");
        registry.assign(1, HUT).expect("the hut takes it");
        registry.settle();
        assert!(registry.notices().is_empty());
        assert!(!registry.dirty());
        assert_eq!(state_of(&registry, 1), Some(State::Assigned), "the request stays");
    }
}
