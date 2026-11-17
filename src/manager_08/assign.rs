//! The 40-line manager that is the whole reason `WrappedBlacklistAssignment` exists:
//! the same assignment search, with a list of resolvers it may not use.
//!
//! Java reference: `WrappedBlacklistAssignmentRequestManager.java` (40 lines, of
//! which one method), over `RequestHandler.assignRequest` and
//! `assignRequestDefault` (`core/colony/requestsystem/management/handlers/RequestHandler.java:96-276`).
//!
//! **The wrapper adds exactly one thing.** Its `assignRequest` (`:29-38`) does not
//! delegate to the base manager's; it calls
//! `getRequestHandler().assignRequest(getRequest(token), blackListedResolvers)`
//! — the *handler* with the blacklist, not the manager without it. Everything else
//! is the abstract wrapper's pass-through.
//!
//! **The blacklist is a live view, not a copy.** The field is
//! `final Collection<IToken<?>> blackListedResolvers` (`:21-26`) and it is stored,
//! never copied, and read once per candidate inside the loop (`RequestHandler.java:169`).
//! A caller that adds a token after the wrapper is built changes the search.
//! [`Blacklist`] is a borrowed `RefCell<BTreeSet>` for the same reason: a plain
//! `&BTreeSet` would make the collection immutable for as long as the wrapper lives,
//! and the rule — one live collection, still open for writing — would be
//! unrepresentable and so untestable.
//!
//! **Where the blacklist sits in the order of the search is the rule.** It is
//! checked *before* `canResolveRequest` and before the priority tier is fixed
//! (`:169-177`), so a blacklisted building resolver does not merely lose to a
//! warehouse — it does not enter the tier at all. Filtering after the choice would
//! answer `NoResolver` where Java answers the warehouse. [`search`] filters first
//! and hands the survivors to `requests::resolver::choose`, which already is the
//! tier-then-metric loop of `:158-198` (`src/requests/resolver.rs:10-31`).
//!
//! **What is not here, with its cost in [`MANQUANT.md`](../MANQUANT.md).**
//!
//! - `reassignRequest` (`:278-314`), refused by [`reassign`] with a name rather than
//!   faked: the assignment lives on `requests::Request` in this port and in a
//!   resolver → requests store in Java, so "take it away from the resolver that
//!   holds it" has no operation to call.
//! - A resolver's own follow-up requests (`:190`, `:203`: the resolver is handed a
//!   blacklisting manager so the children it files are assigned under the *same*
//!   blacklist, and `:257-262` assigns them). No chains in the port
//!   (`src/requests/mod.rs:26-28`), so no call to make.
//! - The `ASSIGNING` state Java writes before the search (`:130`). The port's record
//!   has five states and that is not one of them
//!   (`src/requests/request.rs:10-17`), so a failed search leaves the request
//!   `Created` here and `ASSIGNING` in Java — a real difference, and the reason the
//!   next pass of `assign` is legal in this port and not in Java.

use std::cell::RefCell;
use std::collections::BTreeSet;

use crate::inventory::Inventory;
use crate::requests::{choose, Candidate, Request, Source};

use super::wrapped::{Registry, Resolver};
use super::Refusal;

/// The tokens the search may not use — Java's
/// `Collection<IToken<?>> blackListedResolvers`, held by reference so a caller can
/// add to it between two searches
/// (`WrappedBlacklistAssignmentRequestManager.java:21-26`).
///
/// **A `RefCell` and not a plain `&BTreeSet`, and that is the rule itself.** Java's
/// field is a *mutable* collection the manager reads once per candidate
/// (`RequestHandler.java:169`): the caller that built the wrapper may extend it
/// afterwards, and the search sees the extension. A `&BTreeSet` cannot express
/// that — it would make the collection immutable for as long as the wrapper lives,
/// so the rule would be untestable and a second search would read a snapshot. The
/// `RefCell` is what "the same collection, still open for writing" costs in Rust.
#[derive(Clone, Copy, Debug)]
pub struct Blacklist<'a> {
    tokens: &'a RefCell<BTreeSet<Resolver>>,
}

impl<'a> Blacklist<'a> {
    pub fn new(tokens: &'a RefCell<BTreeSet<Resolver>>) -> Blacklist<'a> {
        Blacklist { tokens }
    }

    /// `resolverTokenBlackList.contains(resolver.getId())`
    /// (`RequestHandler.java:169`).
    pub fn holds(&self, resolver: Resolver) -> bool {
        self.tokens.borrow().contains(&resolver)
    }

    pub fn len(&self) -> usize {
        self.tokens.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.borrow().is_empty()
    }
}

/// One resolver that might supply, with the token the blacklist names it by.
///
/// `requests::resolver::Candidate` is the same candidate without a token — it is
/// what the choice reads (`metric`, `eligible`, `choose`), and it deliberately
/// carries no identity because nothing in the choice depends on who the resolver
/// is. The blacklist does depend on it, so the token rides beside the candidate
/// here instead of being added to that type.
pub struct Offer<'a> {
    pub resolver: Resolver,
    pub source: Source,
    pub inventory: &'a Inventory<'a>,
    /// The resolver's queue length: the warehouse metric reads it
    /// (`AbstractWarehouseRequestResolver.java:352-361`).
    pub queue: u32,
    /// `resolver.canResolveRequest(manager, request)` (`RequestHandler.java:180`).
    /// A resolver being removed (`manager.getResolverHandler().isBeingRemoved`)
    /// answers `false` here: the port has no resolver registry
    /// (`MANQUANT.md` §1).
    pub can_resolve: bool,
}

impl<'a> Offer<'a> {
    pub fn new(resolver: Resolver, source: Source, inventory: &'a Inventory<'a>) -> Offer<'a> {
        Offer { resolver, source, inventory, queue: 0, can_resolve: true }
    }

    pub fn with_queue(mut self, queue: u32) -> Offer<'a> {
        self.queue = queue;
        self
    }

    pub fn silent(mut self) -> Offer<'a> {
        self.can_resolve = false;
        self
    }

    fn candidate(&self) -> Candidate<'a> {
        Candidate::new(self.source, self.resolver, self.inventory).with_queue(self.queue)
    }
}

/// Assigns `id` to the best resolver that is neither blacklisted nor unable.
///
/// Three refusals, all of them before any write and all of them named:
///
/// - [`Refusal::UnknownRequest`] — `getRequest(request.getId())` is the first line
///   of the search (`RequestHandler.java:126`).
/// - [`Refusal::Request`] carrying `AlreadyAssigned` — the wrapper's javadoc
///   promises it (`WrappedBlacklistAssignmentRequestManager.java:34-38`) and the
///   code does not check it; the port keeps the promise, because an assignment that
///   silently replaced another would leave the first resolver holding a request it
///   no longer has.
/// - [`Refusal::NoResolver`] — every survivor was blacklisted, unable, short of the
///   request's minimum, or there were none. Java answers `null` (`:276`).
pub fn assign(
    registry: &mut Registry,
    id: i32,
    offers: &[Offer<'_>],
    blacklist: &Blacklist<'_>,
) -> Result<Resolver, Refusal> {
    let winner = {
        let request = registry.require(id)?;
        if let Some(by) = request.assigned_to() {
            return Err(Refusal::Request(
                crate::requests::Refusal::AlreadyAssigned { by },
            ));
        }
        search(request, offers, blacklist)?
    };
    registry.assign(id, winner)?;
    Ok(winner)
}

/// The choice, as a function of the request and the offers: the highest priority
/// tier among the survivors, then the lowest metric, a tie keeping the earlier
/// offer (`RequestHandler.java:158-198`).
pub fn search(
    request: &Request,
    offers: &[Offer<'_>],
    blacklist: &Blacklist<'_>,
) -> Result<Resolver, Refusal> {
    let mut survivors: Vec<&Offer<'_>> = Vec::new();
    for offer in offers {
        // `if (resolverTokenBlackList.contains(resolver.getId()) || …isBeingRemoved) continue;`
        // (`RequestHandler.java:169`).
        if blacklist.holds(offer.resolver) {
            continue;
        }
        // `if (!resolver.canResolveRequest(manager, request)) continue;`
        // (`RequestHandler.java:180`).
        if !offer.can_resolve {
            continue;
        }
        survivors.push(offer);
    }
    let candidates: Vec<Candidate<'_>> = survivors.iter().map(|offer| offer.candidate()).collect();
    let index = choose(request, &candidates).ok_or(Refusal::NoResolver)?;
    survivors
        .get(index)
        .map(|offer| offer.resolver)
        .ok_or(Refusal::NoResolver)
}

/// `RequestHandler.reassignRequest` (`:278-314`), refused by name.
///
/// Java's version is the blacklist's *other* caller: it takes the request away from
/// the resolver that holds it (`:295-308`), marks it `REPORTED` (`:309`), and searches
/// again under the same blacklist (`:310`). The own javadoc promises to put it back on the
/// original resolver when nothing is found (`:279-280`) and the code does not — a
/// failed reassignment leaves the request unassigned.
///
/// **Neither half can be ported.** This port's `Request` *is* the assignment
/// (`src/requests/request.rs:201-203`, set once by `assign` and never cleared), and
/// Java's assignment is a resolver → requests map beside the request; taking it away
/// is a map mutation there and has no operation here. Keeping the first refusal —
/// the token must be registered (`StandardRequestManager.java:257`) — is worth more than a function that pretends.
pub fn reassign(registry: &Registry, id: i32) -> Result<Resolver, Refusal> {
    registry.require(id)?;
    Err(Refusal::NotPortable { what: "RequestHandler.reassignRequest" })
}

#[cfg(test)]
mod tests {
    use super::super::wrapped::Notice;
    use super::*;
    use crate::inventory::{ItemLimits, ItemName, Stack};
    use crate::requests::{Requester, State};

    const HUT: [i32; 3] = [10, 64, 10];
    const WAREHOUSE: [i32; 3] = [40, 64, 40];
    const FAR: [i32; 3] = [200, 64, 200];

    fn ladder() -> ItemName {
        ItemName::new("minecraft:ladder").expect("a valid name")
    }

    fn limits() -> ItemLimits {
        ItemLimits::one(ladder(), 64)
    }

    fn stocked(inventory: &mut Inventory<'_>, count: u32) {
        if count > 0 {
            let stack = Stack::new(ladder(), count).expect("a nonzero stack");
            assert!(inventory.set_slot(0, Some(stack)));
        }
    }

    fn asking() -> Request {
        Request::new(1, ladder(), 4, 4, Requester::new("minecolonies:builder", HUT))
            .expect("a nonzero request")
    }

    fn registry_with(request: Request) -> Registry {
        let mut registry = Registry::new();
        registry.register(request).expect("a fresh id");
        registry
    }

    /// A blacklist holding exactly `tokens`. Java's field is a live collection, so
    /// the holder is a `RefCell`: a test that adds to it after the wrapper exists is
    /// the rule, not a trick of the fixture.
    fn blocking(tokens: &[Resolver]) -> RefCell<BTreeSet<Resolver>> {
        RefCell::new(tokens.iter().copied().collect())
    }

    /// The blacklist is the only difference between these two calls: the blacklisted
    /// hut holds the item and outranks the warehouse, and the search must still find
    /// the warehouse, because the filter happens before the tier is fixed
    /// (`RequestHandler.java:169-177`).
    #[test]
    fn a_blacklisted_hut_does_not_enter_the_tier_and_the_warehouse_takes_it() {
        let limits = limits();
        let (mut hut, mut warehouse) = (Inventory::new(1, &limits), Inventory::new(1, &limits));
        stocked(&mut hut, 64);
        stocked(&mut warehouse, 64);
        let offers = [
            Offer::new(HUT, Source::Building, &hut),
            Offer::new(WAREHOUSE, Source::Warehouse, &warehouse),
        ];
        let none = blocking(&[]);
        let mut registry = registry_with(asking());
        assert_eq!(
            assign(&mut registry, 1, &offers, &Blacklist::new(&none)),
            Ok(HUT),
            "a building resolver outranks a warehouse"
        );

        let blocked = blocking(&[HUT]);
        let mut registry = registry_with(asking());
        assert_eq!(
            assign(&mut registry, 1, &offers, &Blacklist::new(&blocked)),
            Ok(WAREHOUSE),
            "filtered before the tier, not after the choice"
        );
    }

    #[test]
    fn a_search_that_finds_nobody_refuses_and_writes_nothing() {
        let limits = limits();
        let mut hut = Inventory::new(1, &limits);
        stocked(&mut hut, 64);
        let offers = [Offer::new(HUT, Source::Building, &hut)];
        let blocked = blocking(&[HUT]);
        let mut registry = registry_with(asking());
        assert_eq!(
            assign(&mut registry, 1, &offers, &Blacklist::new(&blocked)),
            Err(Refusal::NoResolver)
        );
        assert!(registry.notices().is_empty(), "no notification, no dirty flag");
        assert_eq!(
            registry.request(1).map(|request| request.state()),
            Some(State::Created),
            "the request is where it was — Java leaves it ASSIGNING, this port has no such state"
        );
    }

    #[test]
    fn the_blacklist_is_a_view_and_a_token_added_between_two_searches_counts() {
        let limits = limits();
        let mut hut = Inventory::new(1, &limits);
        stocked(&mut hut, 64);
        let offers = [Offer::new(HUT, Source::Building, &hut)];
        let tokens = blocking(&[]);
        let blacklist = Blacklist::new(&tokens);
        assert_eq!(search(&asking(), &offers, &blacklist), Ok(HUT));
        tokens.borrow_mut().insert(HUT);
        assert_eq!(
            search(&asking(), &offers, &blacklist),
            Err(Refusal::NoResolver),
            "the collection is stored by reference, WrappedBlacklistAssignmentRequestManager.java:21-26"
        );
        assert_eq!(blacklist.len(), 1);
        assert!(blacklist.holds(HUT));
    }

    #[test]
    fn a_resolver_that_cannot_resolve_is_skipped_even_outside_the_blacklist() {
        let limits = limits();
        let mut hut = Inventory::new(1, &limits);
        stocked(&mut hut, 64);
        let offers = [Offer::new(HUT, Source::Building, &hut).silent()];
        let none = blocking(&[]);
        assert_eq!(
            search(&asking(), &offers, &Blacklist::new(&none)),
            Err(Refusal::NoResolver),
            "RequestHandler.java:180"
        );
    }

    #[test]
    fn the_journal_of_a_found_resolver_names_the_resolver_then_dirty() {
        let limits = limits();
        let mut hut = Inventory::new(1, &limits);
        stocked(&mut hut, 64);
        let offers = [Offer::new(HUT, Source::Building, &hut)];
        let none = blocking(&[]);
        let mut registry = registry_with(asking());
        registry.adopt(1, 7);
        assert_eq!(assign(&mut registry, 1, &offers, &Blacklist::new(&none)), Ok(HUT));
        assert_eq!(
            registry.notices(),
            &[Notice::ResolverTold { id: 1, by: HUT }, Notice::Dirty],
            "and no parent: the state write of RequestHandler.java:268 is silent"
        );
    }

    #[test]
    fn an_assigned_request_is_refused_before_the_search_runs() {
        let limits = limits();
        let mut far = Inventory::new(1, &limits);
        stocked(&mut far, 64);
        let offers = [Offer::new(FAR, Source::Building, &far)];
        let none = blocking(&[]);
        let mut registry = registry_with(asking());
        registry.assign(1, HUT).expect("the first resolver");
        registry.settle();
        assert_eq!(
            assign(&mut registry, 1, &offers, &Blacklist::new(&none)),
            Err(Refusal::Request(crate::requests::Refusal::AlreadyAssigned {
                by: HUT
            })),
            "the javadoc's promise, WrappedBlacklistAssignmentRequestManager.java:34-38"
        );
        assert!(registry.notices().is_empty(), "and the search never ran");
        assert_eq!(
            registry.request(1).and_then(|request| request.assigned_to()),
            Some(HUT)
        );
    }

    #[test]
    fn an_unknown_request_is_the_first_refusal() {
        let offers: [Offer<'_>; 0] = [];
        let none = blocking(&[]);
        let mut registry = Registry::new();
        assert_eq!(
            assign(&mut registry, 9, &offers, &Blacklist::new(&none)),
            Err(Refusal::UnknownRequest { id: 9 })
        );
        assert!(registry.notices().is_empty());
    }

    #[test]
    fn a_reassignment_is_refused_by_name_and_its_first_check_still_answers() {
        let registry = registry_with(asking());
        assert_eq!(
            reassign(&registry, 9),
            Err(Refusal::UnknownRequest { id: 9 }),
            "getRequest(token) comes first, StandardRequestManager.java:257"
        );
        assert_eq!(
            reassign(&registry, 1),
            Err(Refusal::NotPortable { what: "RequestHandler.reassignRequest" })
        );
        // And the operation Java's reassign needs, this port's record refuses: an
        // assignment is set once and never cleared (`src/requests/request.rs:235-246`).
        let mut request = asking();
        request.assign(HUT).expect("the first resolver");
        assert_eq!(
            request.assign(FAR),
            Err(crate::requests::Refusal::AlreadyAssigned { by: HUT })
        );
    }
}
