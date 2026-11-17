//! Four managers of Java's colony, and the three answers that were already here.
//!
//! Java reference: `core/colony/requestsystem/management/manager/wrapped/`
//! `AbstractWrappedRequestManager.java` (251 lines),
//! `WrappedBlacklistAssignmentRequestManager.java` (40),
//! `WrappedStaticStateRequestManager.java` (38), and
//! `core/colony/workorders/WorkManager.java` (542) — 871 lines of Java, of which
//! this module ports the rules and none of the world.
//!
//! | File | Question | Java |
//! | --- | --- | --- |
//! | `wrapped` | what a wrapped manager *is*: one state shared, one method refused | `AbstractWrappedRequestManager.java:34-254` |
//! | `assign` | who may take a request when a list of resolvers is excluded | `WrappedBlacklistAssignmentRequestManager.java:26-38`, `RequestHandler.java:96-276` |
//! | `suppress` | a state write that tells nobody, and the loop it breaks | `WrappedStaticStateRequestManager.java:14-31`, `AbstractRequest.java:136-154` |
//! | `work` | the order board: what is added, who takes it, and in which order a removal is written | `WorkManager.java:74-518` |
//!
//! **Three quarters of these four files are already covered, and this module says
//! where instead of repeating them.**
//!
//! - `WorkManager.read` (`:246-283`) — the `newsystem` guard, the record that
//!   does not read, the duplicate id, the claim released on read — is
//!   `crate::persistence::read_orders` (`src/persistence/mod.rs:448-505`), and
//!   `WorkManager.write` (`:226-238`) is `written_work` (`:429-446`). Nothing of
//!   the read or the write is duplicated here.
//! - The order record and its four state transitions (`AbstractWorkOrder.java`)
//!   are `crate::colony::ordre`, and the priority comparator (`IWorkOrder.java:19-24`)
//!   is `ordre::compare`.
//! - `getWorkOrder(int)` (`:148-151`), `getWorkOrdersOfType` (`:183-188`), the id
//!   counter and the removal of a finished order (`:308-312`, `:80-95`) are
//!   `Colony::order`, `Colony::place_order`, `Colony::finish_order` and
//!   `Colony::cancel_order` (`src/colony/colonie.rs:220-273`).
//!
//! What is left, and what this module adds, is the half Java writes *around*
//! those: the assignment loop, the tick's three passes, the callbacks of a
//! removal, the out-of-colony box, and the two request-system wrappers whose
//! whole code is one refusal and one silence.
//!
//! # Premises, five of them measured, three of them false in Java
//!
//! 1. **The assignment of a request lives in the request, and Java keeps it
//!    beside it.** `requests::Request` records `assigned_to`
//!    (`src/requests/request.rs:201-203`); Java keeps a
//!    resolver → requests map in a data store and mutates *that*
//!    (`RequestHandler.java:295-308`). This is why `assignRequest` ports and
//!    `reassignRequest` (`:278-314`) does not: reassigning needs an unassign, and
//!    a request in this port has none. [`MANQUANT.md`](MANQUANT) states it, and
//!    `assign`'s last test proves it rather than describing it.
//! 2. **Java's assignment loop iterates a `HashMap`** of buildings
//!    (`WorkManager.java:439`), so *which* builder takes a work order when two
//!    qualify is not specified by Java. [`work::tick`] walks the caller's slice in
//!    its own order, so two runs of the same colony agree — the same choice
//!    `manager/claims.rs:195-197` makes against `HashSet.iterator().next()`.
//! 3. **`addWorkOrder` writes before it refuses, and loses an order doing it.**
//!    It sets `dirty` (`:288`), *deletes* the order already standing at the same
//!    position (`:292-299`), and only then answers the out-of-colony refusal
//!    (`:301-305`) — a refused order has destroyed its predecessor. [`work::add`]
//!    refuses first and keeps the incumbent; the report names the divergence.
//! 4. **`WrappedStaticStateRequestManager.updateRequestState` is not a rule.**
//!    Its body is a `TODO` (`:29`): it returns without reading its arguments.
//!    [`suppress::suppressed`] reproduces the absence and says so, instead of
//!    inventing the loop-breaking write Java never wrote.
//! 5. **The blacklist wrapper's javadoc and its code disagree.**
//!    `assignRequest`'s javadoc promises a refusal for a request *already assigned*
//!    (`:34-38`) and the code never checks it; the same javadoc of
//!    `reassignRequest` promises to restore the previous resolver on failure
//!    (`:279-280`) and the code does not. [`assign::assign`] follows the javadoc's
//!    first promise, because it is the only one that costs nothing to keep.
//!
//! **No host call, no clock, no thread, no file.** The requests, the huts, the
//! claims and the structure's box come in as parameters, as `crate::manager` takes
//! the claims and `crate::requests` takes the candidates. That is what a wasm mod
//! gets anyway, and it is what lets the four files be tested with no world.
//!
//! **One write is not here, and it is named**: `markDirty` reaches the colony in
//! Java (`WorkManager.java:116`) and the colony record of this port has no dirty
//! flag — [`work::Sink`] carries the two writes, and the caller decides what a
//! colony does with them.

pub mod assign;
pub mod suppress;
pub mod work;
pub mod wrapped;

pub use assign::{assign, Blacklist, Offer};
pub use suppress::{manager_update_state, suppressed, update_state, Outcome, Voice};
pub use work::{
    can_build, can_build_ignoring_distance, clear_work_for, first_unclaimed, ordered_for,
    within_colony, AddRefusal, Decision, Hut, Site, Skip, Tick, Write, MANUAL_SETTING, MODE_SETTING,
};
pub use wrapped::{create_and_assign, on_colony_update, Notice, Registry, Resolver};

/// Why one of the three wrapped managers refused, named.
///
/// [`Self::Request`] passes the request system's own refusal through instead of
/// respelling it: a caller that cannot tell "the request is finished" from "the
/// request is not registered" retries the wrong thing
/// (`src/requests/mod.rs:49-89`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// The request system refused the move itself — the state machine's answer.
    Request(crate::requests::Refusal),
    /// A wrapped manager cannot answer this method at all
    /// (`AbstractWrappedRequestManager.java:250-254`).
    NotDelegated { method: &'static str },
    /// No request of this id is registered with the manager
    /// (`RequestHandler.java:126`).
    UnknownRequest { id: i32 },
    /// Every resolver that could supply is blacklisted, or none could supply at
    /// all (`RequestHandler.java:169-198`), and nothing was written.
    NoResolver,
    /// A reassignment would need the request to be handed to a second resolver,
    /// which this port's record cannot express (see the module's first premise).
    NotPortable { what: &'static str },
}

impl Refusal {
    /// A stable name for logs and tests, namespaced like a registry entry so it can
    /// never be mistaken for a player-facing sentence.
    pub fn name(self) -> &'static str {
        match self {
            Self::Request(inner) => inner.name(),
            Self::NotDelegated { .. } => "manager_08.refusal.not_delegated",
            Self::UnknownRequest { .. } => "manager_08.refusal.unknown_request",
            Self::NoResolver => "manager_08.refusal.no_resolver",
            Self::NotPortable { .. } => "manager_08.refusal.not_portable",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colony::{Colony, WorkType};
    use crate::inventory::{Inventory, ItemLimits, ItemName};
    use crate::manager::ClaimBook;
    use crate::requests::{Request, Requester, Source};

    const HUT: [i32; 3] = [10, 64, 10];
    const OTHER: [i32; 3] = [20, 64, 10];

    fn planks() -> ItemName {
        ItemName::new("minecraft:oak_planks").expect("a valid name")
    }

    fn asking(id: i32) -> Request {
        Request::new(id, planks(), 4, 4, Requester::new("minecolonies:builder", HUT))
            .expect("a nonzero request")
    }

    #[test]
    fn every_refusal_has_its_own_name() {
        let all = [
            Refusal::Request(crate::requests::Refusal::WrongItem),
            Refusal::NotDelegated { method: "onColonyUpdate" },
            Refusal::UnknownRequest { id: 1 },
            Refusal::NoResolver,
            Refusal::NotPortable { what: "reassign" },
        ];
        for (index, refusal) in all.iter().enumerate() {
            for other in &all[index + 1..] {
                assert_ne!(refusal.name(), other.name(), "two refusals share a name");
            }
        }
        assert_eq!(
            Refusal::Request(crate::requests::Refusal::WrongItem).name(),
            "requests.refusal.item",
            "the request system's own name, not a respelling"
        );
        assert!(Refusal::NoResolver.name().starts_with("manager_08.refusal."));
        assert_eq!(Refusal::NoResolver.to_string(), "manager_08.refusal.no_resolver");
    }

    /// **No write before a refusal, in the three managers at once.** The wrapped
    /// manager's refused method, the assignment that finds nobody, and the order
    /// board's refused addition: three journals, all empty, and no state moved.
    #[test]
    fn the_three_managers_write_nothing_when_they_refuse() {
        let mut registry = Registry::new();
        registry.register(asking(1)).expect("a fresh id");

        assert_eq!(
            on_colony_update(&registry),
            Err(Refusal::NotDelegated { method: "onColonyUpdate" })
        );
        assert!(registry.notices().is_empty(), "a refusal writes nothing");

        let limits = ItemLimits::one(planks(), 64);
        let empty = Inventory::new(1, &limits);
        let blacklist = std::cell::RefCell::new(std::collections::BTreeSet::from([OTHER]));
        let blacklist = Blacklist::new(&blacklist);
        let offers = [Offer::new(OTHER, Source::Warehouse, &empty)];
        assert_eq!(
            assign(&mut registry, 1, &offers, &blacklist),
            Err(Refusal::NoResolver)
        );
        assert!(registry.notices().is_empty(), "and so does a search that fails");
        assert_eq!(
            registry.request(1).map(|request| request.state()),
            Some(crate::requests::State::Created)
        );

        let mut colony = Colony::new(7, "Bridgewater", "minecraft:overworld", [0, 70, 0], 1);
        let book = ClaimBook::new();
        let mut journal: Vec<Write> = Vec::new();
        assert_eq!(
            work::add(&mut colony, "minecolonies:builder", HUT, WorkType::Build, 1, &Site { bounds: None, book: &book }, &mut journal),
            Ok(1),
            "no bounds is the port's gap, not a refusal (MANQUANT.md §3)"
        );
        journal.clear();
        let bounds = crate::building::Footprint::between([0, 0, 0], [32, 0, 32]);
        assert_eq!(
            work::add(
                &mut colony,
                "minecolonies:builder",
                [200, 64, 200],
                WorkType::Build,
                1,
                &Site { bounds: Some(&bounds), book: &book },
                &mut journal
            ),
            Err(AddRefusal::OutOfColony { at: [200, 64, 200] })
        );
        assert!(journal.is_empty(), "the out-of-colony refusal precedes every write");
    }
}
