//! The manager whose whole code is a silence: a state change made so that nothing
//! downstream hears about it.
//!
//! Java reference: `WrappedStaticStateRequestManager.java` (38 lines: a constructor
//! and one empty method), `AbstractRequest.setState`
//! (`core/colony/requestsystem/requests/AbstractRequest.java:136-154`),
//! `StandardRequestManager.updateRequestState` (`:295-303`) and the four places the
//! static manager is handed over (`RequestHandler.java:130`, `:268`, `:296`, `:307`).
//!
//! **The class has two uses, and four rules come out of them.**
//!
//! | Call | The state | The log line | The parent | Dirty |
//! | --- | --- | --- | --- | --- |
//! | `request.setState(base, s)` | written | yes, `:143` | yes, `:142-153` | no |
//! | `request.setState(static, s)` | written | no | no | no |
//! | `manager.updateRequestState(t, s)` | written, through *static* | yes, `:299` | no | yes, `:302` |
//! | `static.updateRequestState(t, s)` | **nothing at all** | no | no | no |
//!
//! The table is the file. Row 1 is [`update_state`] with [`Voice::Direct`], row 2
//! with [`Voice::Static`], row 3 is [`manager_update_state`], row 4 is
//! [`suppressed`].
//!
//! **Two things the table says that the Java does not read like at first glance.**
//!
//! - **The base manager does not tell the parent either.** `updateRequestState`
//!   writes through a static manager (`:301`), so even the loud entry point is
//!   silent towards the parent. Only a *direct* `setState(base, …)` — `:342` for a
//!   follow-up, for instance — reaches it.
//! - **`markDirty` differs by entry point.** `updateRequestState` marks dirty after
//!   the write (`:302`); `assignRequest` after the search (`:230`), search that may
//!   have found nobody; `reassignRequest` *before* the search (`:258`). The port
//!   keeps the last one out (see `assign::reassign`) and the first two in their order.
//!
//! **What the silence buys, exactly.** `RequestHandler.assignRequestDefault` sets
//! `ASSIGNING` through the silent manager (`:130`), assigns, then sets
//! `IN_PROGRESS` through it again (`:268`). With the ordinary manager each of
//! those writes tells the parent, and a parent's `childStateUpdated` starts a fresh
//! assignment search (`AbstractRequest.java:142-153` → `RequestHandler.java:96`) —
//! from inside the search that is running. The state the silent write leaves behind
//! is the one Java then checks at `:266` (`getState().ordinal() <
//! RequestState.IN_PROGRESS.ordinal()`), so the write is load-bearing and only the
//! *notification* is dropped.
//!
//! **The parent link itself is not ported.** A chain of requests — a parent that
//! waits for its children (`RequestHandler.java:245-264`) — is out of the port
//! (`src/requests/mod.rs:26-28`). What is here is the one write that chain receives,
//! because it is the write the silence exists to remove.

use crate::requests::{Refusal as RequestRefusal, Request};

use super::wrapped::{Notice, Registry};
use super::Refusal;

/// Which manager the request's own `setState` is handed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: Java has two managers that differ on exactly this
pub enum Voice {
    /// `request.setState(manager, state)` with the real manager: the log line and
    /// the parent (`AbstractRequest.java:142-153`).
    Direct,
    /// `request.setState(new WrappedStaticStateRequestManager(manager), state)`:
    /// the field, and nothing else (`WrappedStaticStateRequestManager.java:14-31`).
    Static,
}

/// What the static manager's own `updateRequestState` did: nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one outcome, because the method has one
pub enum Outcome {
    /// The call returned without reading its token and without writing anything
    /// (`:29`, the `TODO`).
    Suppressed,
}

/// `request.setState(voice, state)`: moves the request and tells what that voice
/// tells.
///
/// `step` is the transition itself — one of `Request::assign`, `begin` or `cancel`,
/// so this function cannot invent a state Java does not have. That is the division
/// of labour: the state machine belongs to `crate::requests`
/// (`src/requests/request.rs:231-297`), the *notification* belongs to the manager,
/// and this is the manager.
///
/// A refused step writes nothing and says nothing, through either voice: the
/// request's own refusal is returned as it is and the journal stays empty. Note that
/// **neither voice marks the manager dirty** — `AbstractRequest.setState` never
/// does (`:136-154`); whatever is dirty is the caller's business.
pub fn update_state(
    registry: &mut Registry,
    id: i32,
    voice: Voice,
    step: impl FnOnce(&mut Request) -> Result<(), RequestRefusal>,
) -> Result<(), Refusal> {
    {
        let Some(request) = registry.request_mut(id) else {
            return Err(Refusal::UnknownRequest { id });
        };
        step(request).map_err(Refusal::Request)?;
    }
    if voice == Voice::Direct {
        registry.tell(Notice::Logged { id });
        if let Some(parent) = registry.parent_of(id) {
            registry.tell(Notice::ParentTold { child: id, parent });
        }
    }
    Ok(())
}

/// `StandardRequestManager.updateRequestState(token, state)` (`:295-303`).
///
/// Four steps, in this order: the token is looked up **strictly** — an unknown one
/// throws before anything is written (`:297`) — the manager logs the old and the new
/// state (`:299`), the request takes the new state through the *static* manager
/// (`:301`), and the manager marks itself dirty (`:302`), which reaches the colony
/// (`:213-221`).
///
/// What is not ported is the `switch` that follows (`:304-360`): it files follow-up
/// requests for `RESOLVED`, cancels children, and calls `onRequestCompleted` — all of
/// it chain work the port has none of (`src/requests/mod.rs:26-38`).
pub fn manager_update_state(
    registry: &mut Registry,
    id: i32,
    step: impl FnOnce(&mut Request) -> Result<(), RequestRefusal>,
) -> Result<(), Refusal> {
    {
        let Some(request) = registry.request_mut(id) else {
            return Err(Refusal::UnknownRequest { id });
        };
        step(request).map_err(Refusal::Request)?;
    }
    registry.tell(Notice::Logged { id });
    registry.tell(Notice::Dirty);
    Ok(())
}

/// `WrappedStaticStateRequestManager.updateRequestState` (`:26-30`).
///
/// The body is a `TODO` in Java — "implement when link is created with worker" —
/// followed by the end of the method. So this takes the manager and the token and
/// **touches neither**: no state write, no log line, no parent, no dirty flag, and no
/// refusal even for a token nobody registered, because a body that reads nothing
/// cannot fail. The parameters are named with a leading underscore for that reason,
/// not because they are an oversight.
pub fn suppressed(_registry: &mut Registry, _id: i32) -> Outcome {
    Outcome::Suppressed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::ItemName;
    use crate::requests::{Requester, State};

    const HUT: [i32; 3] = [10, 64, 10];

    fn asking(id: i32) -> Request {
        let item = ItemName::new("minecraft:ladder").expect("a valid name");
        Request::new(id, item, 4, 4, Requester::new("minecolonies:builder", HUT))
            .expect("a nonzero request")
    }

    /// A registry with a parent (`7`) and its child (`1`), both unassigned.
    fn family() -> Registry {
        let mut registry = Registry::new();
        registry.register(asking(7)).expect("the parent");
        registry.register(asking(1)).expect("the child");
        registry.adopt(1, 7);
        registry
    }

    #[test]
    fn the_same_write_tells_a_different_world_through_each_voice() {
        let mut loud = family();
        update_state(&mut loud, 1, Voice::Direct, |request| request.assign(HUT))
            .expect("the child is unassigned");
        assert_eq!(
            loud.notices(),
            &[Notice::Logged { id: 1 }, Notice::ParentTold { child: 1, parent: 7 }],
            "AbstractRequest.java:140 and :142-153 — and no dirty, which setState never sets"
        );
        assert!(!loud.dirty());

        let mut silent = family();
        update_state(&mut silent, 1, Voice::Static, |request| request.assign(HUT))
            .expect("the same write");
        assert!(silent.notices().is_empty(), "the static manager tells nobody");
        assert!(!silent.dirty());
        assert_eq!(
            silent.request(1).map(|request| request.state()),
            Some(State::Assigned),
            "but the state was written: the silence drops the notification, not the move"
        );
    }

    #[test]
    fn the_silent_voice_is_what_stops_the_parent_from_starting_a_second_search() {
        let mut registry = family();
        registry.request_mut(1).expect("the child").assign(HUT).expect("first");
        registry.settle();
        update_state(&mut registry, 1, Voice::Static, |request| request.begin())
            .expect("work begins");
        assert!(registry.notices().is_empty());
        assert_eq!(
            registry.request(7).map(|parent| parent.state()),
            Some(State::Created),
            "the parent was not reached: that notification is the recursion RequestHandler.java:130 avoids"
        );
        assert_eq!(
            registry.request(1).map(|child| child.state()),
            Some(State::InProgress)
        );
    }

    #[test]
    fn the_manager_own_entry_point_logs_and_dirties_but_never_tells_the_parent() {
        let mut registry = family();
        manager_update_state(&mut registry, 1, |request| request.assign(HUT))
            .expect("the child is unassigned");
        assert_eq!(
            registry.notices(),
            &[Notice::Logged { id: 1 }, Notice::Dirty],
            "StandardRequestManager.java:299 and :302 — the write of :301 goes through the static manager"
        );
        assert!(registry.dirty());
        assert_eq!(
            registry.request(7).map(|parent| parent.state()),
            Some(State::Created),
            "the parent still learned nothing"
        );
    }

    #[test]
    fn a_refused_step_writes_nothing_through_any_voice() {
        for voice in [Voice::Direct, Voice::Static] {
            let mut registry = family();
            assert_eq!(
                update_state(&mut registry, 1, voice, |request| request.begin()),
                Err(Refusal::Request(RequestRefusal::IllegalTransition {
                    from: State::Created,
                    to: State::InProgress,
                })),
                "nothing is assigned, so nothing can begin"
            );
            assert!(registry.notices().is_empty(), "{voice:?}: no write, no notice");
            assert!(!registry.dirty());
            assert_eq!(
                registry.request(1).map(|request| request.state()),
                Some(State::Created)
            );
        }

        let mut registry = family();
        assert_eq!(
            manager_update_state(&mut registry, 1, |request| request.begin()),
            Err(Refusal::Request(RequestRefusal::IllegalTransition {
                from: State::Created,
                to: State::InProgress,
            })),
            "the manager does not mark dirty before the transition either"
        );
        assert!(registry.notices().is_empty());
        assert!(!registry.dirty());
    }

    #[test]
    fn an_unknown_request_is_refused_by_the_two_entry_points_that_read_the_token() {
        let mut registry = family();
        assert_eq!(
            update_state(&mut registry, 9, Voice::Direct, |request| request.cancel()),
            Err(Refusal::UnknownRequest { id: 9 })
        );
        assert_eq!(
            manager_update_state(&mut registry, 9, |request| request.cancel()),
            Err(Refusal::UnknownRequest { id: 9 }),
            "getRequest(token) is the first line of :297"
        );
        assert!(registry.notices().is_empty());
        assert!(!registry.dirty());
    }

    #[test]
    fn the_static_managers_own_update_does_nothing_at_all() {
        let mut registry = family();
        assert_eq!(suppressed(&mut registry, 1), Outcome::Suppressed);
        assert_eq!(
            suppressed(&mut registry, 9),
            Outcome::Suppressed,
            "even for a token it never saw: the body reads nothing"
        );
        assert!(registry.notices().is_empty());
        assert!(!registry.dirty());
        assert_eq!(
            registry.request(1).map(|request| request.state()),
            Some(State::Created),
            "Java's body is a TODO, so the port reproduces the absence"
        );
    }
}
