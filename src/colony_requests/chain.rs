//! In what order the resolvers are asked, and what one of them refuses.
//!
//! Java reference: `api/colony/requestsystem/resolver/IRequestResolver.java`
//! (`getRequestType`, `canResolveRequest:34-40`, `attemptResolveRequest:44-58`,
//! `getSuitabilityMetric:126-129`), `api/util/constant/RSConstants.java:11-16`
//! (the six priorities), `core/.../management/handlers/RequestHandler.java:121-225`
//! (the assignment loop) and `:235-276` (what happens once one answers), and
//! `core/.../resolvers/core/AbstractCraftingRequestResolver.java:226-252` (the one
//! refusal whose rule is more than a stock check).
//!
//! **The order, in four steps, exactly as Java builds it.**
//!
//! 1. **Only resolvers that serve the request's type are asked.** Java walks the
//!    request's superclass set and keeps the resolvers registered for one of
//!    those types (`:132-156`), so a `Delivery` request never reaches the
//!    warehouse resolver and a `Stack` request never reaches the deliveryman.
//!    [`Type::accepts`] is that rule.
//! 2. **Descending priority, and the highest tier that answers ends the search.**
//!    The list is sorted by `-priority` (`:158`), de-duplicated keeping the first
//!    of each resolver (`:161`, a `LinkedHashSet`), and the loop `break`s the
//!    moment the priority changes after somebody answered (`:174-177`). A hut
//!    holding the item therefore beats a warehouse next door, whatever the
//!    distance.
//! 3. **Inside one tier the lowest suitability metric wins**, and a tie keeps the
//!    candidate already in hand: the comparison is a strict `<` (`:197-198`).
//!    The metric itself is `requests::resolver::metric`, which already ports the
//!    three formulas and is **not** repeated here.
//! 4. **A resolver may refuse twice.** `canResolveRequest` is the cheap pre-check
//!    (`:180-183`), `attemptResolveRequest` returning `null` the real one
//!    (`:188-204`), and a resolver that answers the second question with a list
//!    of child tokens is saying "satisfy these first". When a better candidate
//!    replaces the current one, Java **cancels** the children of the attempt it
//!    drops (`:206-213`); [`Outcome::Assigned`] hands them back in `superseded`
//!    so the caller can do exactly that.
//!
//! **Two things this file decides rather than ports.**
//!
//! * Java's second sort key is `typeIndexList.indexOf(resolver.getRequestType())`
//!   where `typeIndexList` came from a `Set` built by reflection (`:134`, `:159`)
//!   — its order is unspecified, so **Java's tie-break between two resolvers of
//!   one priority but different types is arbitrary**. The port keeps declaration
//!   order, which is stable, and says so.
//! * Java's `previousMetric` is recomputed after a successful attempt (`:205`);
//!   the port reads the metric once per resolver per pass. That is the same
//!   number for any metric that only reads the request and the resolver, which is
//!   what [`Supply::metric`] is documented to be.

use std::cmp::Reverse;

use crate::inventory::ItemName;
use crate::requests::Source;

use super::bounds::MAX_CRAFTING_CYCLE_DEPTH;
use super::token::Token;

/// The requestable types that decide which resolvers are asked
/// (`requestable/` — each concrete class and the interfaces it implements).
///
/// `IDeliverable extends IRetryable extends IRequestable`
/// (`IDeliverable.java:9`), `IDeliverymanRequestable extends IRequestable`
/// (`IDeliverymanRequestable.java:8`), and `AbstractCrafting implements
/// IRequestable` **only** (`crafting/AbstractCrafting.java:15`) — which is why a
/// crafting *production* request is not served by a deliverable resolver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the requestable vocabulary the resolvers declare
pub enum Type {
    /// `IRequestable` — everything (`TypeConstants.java:36`).
    Requestable,
    /// `IRetryable` (`:37`).
    Retryable,
    /// `IDeliverable` (`:39`): `Stack`, `MinimumStack`, `StackList`, `Tool`,
    /// `Food`, `Burnable`, `RequestTag`.
    Deliverable,
    /// `Delivery` — carried by a deliveryman (`deliveryman/Delivery.java:20`).
    Delivery,
    /// `Pickup` (`deliveryman/Pickup.java:17`).
    Pickup,
    /// `Tool` (`requestable/Tool.java:22`).
    Tool,
    /// `Food` (`requestable/Food.java:25`).
    Food,
    /// `PublicCrafting` / `PrivateCrafting` (`crafting/PublicCrafting.java:18`).
    Crafting,
}

impl Type {
    /// Everything this type *is*, itself included — the set Java builds by
    /// reflection (`ReflectionUtils.getSuperClasses`, used at
    /// `requestable/Stack.java:29`) and then walks at `RequestHandler.java:132`.
    pub fn supertypes(self) -> &'static [Type] {
        match self {
            Type::Requestable => &[Type::Requestable],
            Type::Retryable => &[Type::Retryable, Type::Requestable],
            Type::Deliverable | Type::Tool | Type::Food => {
                &[Type::Deliverable, Type::Retryable, Type::Requestable]
            }
            Type::Delivery => &[Type::Delivery, Type::Requestable],
            Type::Pickup => &[Type::Pickup, Type::Requestable],
            Type::Crafting => &[Type::Crafting, Type::Requestable],
        }
    }

    /// Whether a resolver that declared `self` is asked for a request of type
    /// `request` — the membership test of `RequestHandler.java:151`.
    ///
    /// The player resolver declares [`Type::Requestable`] and so takes anything
    /// (`StandardPlayerRequestResolver.java:59-62`); the retrying resolver
    /// declares [`Type::Retryable`] and so never takes a delivery
    /// (`StandardRetryingRequestResolver.java:87-90`).
    pub fn accepts(self, request: Type) -> bool {
        request.supertypes().contains(&self)
    }
}

/// A resolver, as the chain needs to see it: its identity, the priority tier its
/// kind belongs to (`RSConstants.java:11-16`), the requestable type it declared
/// (`getRequestType`), and where it stands.
///
/// The position is `None` for the two resolvers that are not a building — the
/// player resolver and the retrying one — because nothing assigns them a place,
/// and `requests::Request` stores an assignee as a position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolver {
    token: Token,
    kind: Source,
    serves: Type,
    at: Option<[i32; 3]>,
}

impl Resolver {
    pub fn new(token: Token, kind: Source, serves: Type, at: Option<[i32; 3]>) -> Resolver {
        Resolver { token, kind, serves, at }
    }

    /// One of the building-shaped resolvers — the building itself
    /// (`BuildingRequestResolver.java:47-50`), a warehouse
    /// (`AbstractWarehouseRequestResolver.java:58-61`), a crafting request
    /// resolver (`AbstractCraftingRequestResolver.java:80-83`) or a station
    /// (`StationRequestResolver extends BuildingRequestResolver`): all four
    /// declare `IDeliverable` and all four stand somewhere.
    pub fn building(token: Token, kind: Source, at: [i32; 3]) -> Resolver {
        Resolver::new(token, kind, Type::Deliverable, Some(at))
    }

    /// The player resolver, last of all (`StandardPlayerRequestResolver.java:151-154`),
    /// and the only one that takes every type (`:59-62`).
    pub fn player(token: Token) -> Resolver {
        Resolver::new(token, Source::Player, Type::Requestable, None)
    }

    /// The retrying resolver: its identity is derived from the colony
    /// (`StandardRetryingRequestResolver.java:45`) and it serves `IRetryable`
    /// (`:87-90`).
    pub fn retrying(colony: i32) -> Resolver {
        Resolver::new(Token::retrying(colony), Source::Retrying, Type::Retryable, None)
    }

    /// The same resolver declaring another type — a deliveryman resolver
    /// (`DeliveryRequestResolver.java:38-41` declares `Delivery`), a crafting
    /// production resolver (`AbstractCraftingProductionResolver.java:55-58`
    /// declares its crafting class).
    pub fn serving(mut self, serves: Type) -> Resolver {
        self.serves = serves;
        self
    }

    pub fn token(self) -> Token {
        self.token
    }

    pub fn kind(self) -> Source {
        self.kind
    }

    pub fn serves(self) -> Type {
        self.serves
    }

    pub fn at(self) -> Option<[i32; 3]> {
        self.at
    }

    /// The resolver priority Java gives this kind, via [`Source::priority`]
    /// (`RSConstants.java:11-16`).
    pub fn priority(self) -> i32 {
        self.kind.priority()
    }

    /// Whether this resolver stands somewhere, i.e. whether a request assigned to
    /// it can record an assignee position.
    pub fn is_positional(self) -> bool {
        self.at.is_some()
    }
}

/// What a caller must answer for each resolver the chain asks. A trait, not an
/// implementation: the answers read a colony's stock, its crafting queue and a
/// resolver's own bookkeeping, and none of that is data this module owns.
pub trait Supply {
    /// The cheap pre-check — `canResolveRequest`
    /// (`IRequestResolver.java:34-40`). `false` means "do not even try".
    fn can_resolve(&self, resolver: &Resolver) -> bool;

    /// The real attempt — `attemptResolveRequest` (`:44-58`). `None` means this
    /// resolver cannot do it right now; `Some(children)` means it can, once those
    /// child requests exist (`Some(vec![])` for "no requirement").
    fn attempt(&self, resolver: &Resolver) -> Option<Vec<i32>>;

    /// The suitability metric, lower is better — `getSuitabilityMetric`
    /// (`:126-129`). Must depend on nothing but the request and the resolver: the
    /// chain reads it once and compares.
    fn metric(&self, resolver: &Resolver) -> i32;
}

/// The answer to "who supplies this". Java returns a resolver token or `null`
/// (`RequestHandler.java:219-224`); [`Outcome::Nobody`] is that `null`, and it is
/// a normal state — the request waits.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: somebody answered, or nobody did
pub enum Outcome {
    /// One resolver answered. `children` are the requests it wants satisfied
    /// first (`:248-254`), `superseded` the children of every attempt it replaced
    /// and which the caller must cancel (`:206-213`).
    Assigned {
        holder: Token,
        at: Option<[i32; 3]>,
        metric: i32,
        children: Vec<i32>,
        superseded: Vec<i32>,
    },
    /// Nobody could, or everyone able was skipped.
    Nobody,
}

impl Outcome {
    /// The resolver that answered, if any.
    pub fn holder(&self) -> Option<Token> {
        match self {
            Outcome::Assigned { holder, .. } => Some(*holder),
            Outcome::Nobody => None,
        }
    }

    /// The children to create before this request can be resolved.
    pub fn children(&self) -> &[i32] {
        match self {
            Outcome::Assigned { children, .. } => children,
            Outcome::Nobody => &[],
        }
    }

    /// The children of the attempts this one replaced — the requests Java cancels
    /// (`RequestHandler.java:206-213`).
    pub fn superseded(&self) -> &[i32] {
        match self {
            Outcome::Assigned { superseded, .. } => superseded,
            Outcome::Nobody => &[],
        }
    }
}

/// The resolvers a colony has, and the order question asked of them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Chain {
    resolvers: Vec<Resolver>,
}

impl Chain {
    pub fn new(resolvers: Vec<Resolver>) -> Chain {
        Chain { resolvers }
    }

    pub fn len(&self) -> usize {
        self.resolvers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.resolvers.is_empty()
    }

    pub fn resolvers(&self) -> &[Resolver] {
        &self.resolvers
    }

    /// The resolvers that would be asked for a request of type `request_type`, in
    /// the order they are asked — indices into [`Self::resolvers`].
    ///
    /// Descending priority, then declaration order; a token declared twice counts
    /// once, at its best position (`RequestHandler.java:158-161`).
    pub fn order(&self, request_type: Type) -> Vec<usize> {
        let mut indices: Vec<usize> = (0..self.resolvers.len())
            .filter(|&index| self.resolvers[index].serves().accepts(request_type))
            .collect();
        indices.sort_by_key(|&index| (Reverse(self.resolvers[index].priority()), index));

        let mut seen: Vec<Token> = Vec::new();
        indices.retain(|&index| {
            let token = self.resolvers[index].token();
            if seen.contains(&token) {
                false
            } else {
                seen.push(token);
                true
            }
        });
        indices
    }

    /// Ask, in order, until somebody answers — `assignRequestDefault`
    /// (`RequestHandler.java:121-225`).
    ///
    /// `skip` is every resolver not to ask *now*: the caller's blacklist of the
    /// moment, plus the ones being removed (`:169`). Java holds those in two
    /// collections; they mean the same thing to this loop.
    ///
    /// The request itself is not passed: everything the loop needs from it is its
    /// type (for [`Self::order`]) and what [`Supply`] reads. Passing it would let
    /// this file start deciding what to do with the answer.
    pub fn pick(&self, request_type: Type, supply: &dyn Supply, skip: &[Token]) -> Outcome {
        let mut best: Option<(Token, Option<[i32; 3]>, i32, Vec<i32>)> = None;
        let mut superseded: Vec<i32> = Vec::new();
        let mut answered_priority: Option<i32> = None;

        for index in self.order(request_type) {
            let resolver = self.resolvers[index];
            if skip.contains(&resolver.token()) {
                continue;
            }
            // :174-177 — once somebody in a tier answered, no lower tier is
            // consulted, however much better its metric looks.
            if answered_priority.is_some_and(|priority| priority != resolver.priority()) {
                break;
            }
            if !supply.can_resolve(&resolver) {
                continue;
            }

            let previous_metric = best.as_ref().map(|(_, _, metric, _)| *metric);
            match previous_metric {
                // :185-194 — the first attempt of the tier decides the metric.
                None => {
                    if let Some(children) = supply.attempt(&resolver) {
                        answered_priority = Some(resolver.priority());
                        best = Some((resolver.token(), resolver.at(), supply.metric(&resolver), children));
                    }
                }
                // :195-216 — only a strictly better metric is worth an attempt.
                Some(previous) => {
                    let current = supply.metric(&resolver);
                    if current < previous {
                        if let Some(children) = supply.attempt(&resolver) {
                            if let Some((_, _, _, dropped)) = best.take() {
                                superseded.extend(dropped);
                            }
                            answered_priority = Some(resolver.priority());
                            best = Some((resolver.token(), resolver.at(), current, children));
                        }
                    }
                }
            }
        }

        match best {
            None => Outcome::Nobody,
            Some((holder, at, metric, children)) => {
                Outcome::Assigned { holder, at, metric, children, superseded }
            }
        }
    }
}

/// One thing a crafting resolver would have to make: an item and how many.
///
/// The port's `Request` carries an [`ItemName`] and two counts
/// (`requests/request.rs:130-140`); Java's `Stack.equals` also compares the match
/// tags and the "resolved by a building" flag (`requestable/Stack.java`), which
/// this type does not have. So "the same requestable" is "the same item" here,
/// and the count test is the separate one Java already applies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wanted {
    pub item: ItemName,
    pub count: u32,
}

impl Wanted {
    pub fn new(item: ItemName, count: u32) -> Wanted {
        Wanted { item, count }
    }
}

/// Whether asking for `target` would make a crafting loop —
/// `AbstractCraftingRequestResolver.createsCraftingCycle` (`:226-252`).
///
/// Java walks **up** the parent chain from the request being resolved: at each
/// level it asks whether the item asked for is already the target (with a count
/// that would not get smaller, `:238-244`), and it gives up after
/// [`MAX_CRAFTING_CYCLE_DEPTH`] levels (`:233-236`). The port walks a slice of
/// ancestors instead of calling back into a manager, which is the same walk with
/// the map taken out.
///
/// `target_is_this_request` is Java's `targetRequest == null ||
/// !targetRequest.equals(request)` term (`:238`): the two call sites pass the
/// request itself (`:141`) or `null` (`:164`), and the difference decides whether
/// a request may be compared with itself.
pub fn creates_cycle(
    request: &Wanted,
    target: &Wanted,
    target_is_this_request: bool,
    ancestors: &[Wanted],
) -> bool {
    walk(request, target, target_is_this_request, ancestors, 0)
}

fn walk(
    current: &Wanted,
    target: &Wanted,
    target_is_this_request: bool,
    ancestors: &[Wanted],
    depth: u32,
) -> bool {
    // :233-236 — past the depth bound, assume the loop rather than walk forever.
    if depth > MAX_CRAFTING_CYCLE_DEPTH {
        return true;
    }
    // :238-244 — the same item, at a count that is not going up, is a loop.
    if !target_is_this_request && current.item == target.item && current.count <= target.count {
        return true;
    }
    match ancestors.split_first() {
        // :246-249 — no parent left, so no loop in this chain.
        None => false,
        // :251 — one level up, and the "is it this very request" test no longer
        // applies: the parent is a different request by construction.
        Some((parent, rest)) => walk(parent, target, false, rest, depth + 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(n: u128) -> Token {
        Token::new(n)
    }

    fn ladder() -> ItemName {
        ItemName::new("minecraft:ladder").expect("a valid name")
    }

    fn stone() -> ItemName {
        ItemName::new("minecraft:stone").expect("a valid name")
    }

    /// A `Supply` scripted per resolver token: the default is "yes, no children,
    /// metric zero", and a test overrides only what it is about.
    #[derive(Default)]
    struct Script {
        can: Vec<(Token, bool)>,
        attempts: Vec<(Token, Option<Vec<i32>>)>,
        metrics: Vec<(Token, i32)>,
    }

    impl Script {
        fn new() -> Script {
            Script::default()
        }

        fn cannot(mut self, who: Token) -> Script {
            self.can.push((who, false));
            self
        }

        fn refuses(mut self, who: Token) -> Script {
            self.attempts.push((who, None));
            self
        }

        fn wants(mut self, who: Token, children: Vec<i32>) -> Script {
            self.attempts.push((who, Some(children)));
            self
        }

        fn scores(mut self, who: Token, metric: i32) -> Script {
            self.metrics.push((who, metric));
            self
        }
    }

    impl Supply for Script {
        fn can_resolve(&self, resolver: &Resolver) -> bool {
            self.can
                .iter()
                .find(|(who, _)| *who == resolver.token())
                .map(|(_, yes)| *yes)
                .unwrap_or(true)
        }

        fn attempt(&self, resolver: &Resolver) -> Option<Vec<i32>> {
            self.attempts
                .iter()
                .find(|(who, _)| *who == resolver.token())
                .map(|(_, answer)| answer.clone())
                .unwrap_or(Some(Vec::new()))
        }

        fn metric(&self, resolver: &Resolver) -> i32 {
            self.metrics
                .iter()
                .find(|(who, _)| *who == resolver.token())
                .map(|(_, metric)| *metric)
                .unwrap_or(0)
        }
    }

    fn warehouse(n: u128) -> Resolver {
        Resolver::building(token(n), Source::Warehouse, [n as i32, 64, 0])
    }

    fn building(n: u128) -> Resolver {
        Resolver::building(token(n), Source::Building, [n as i32, 64, 0])
    }

    /// The tokens of `order`, in the order they would be asked.
    fn asked(chain: &Chain, request_type: Type) -> Vec<Token> {
        chain
            .order(request_type)
            .into_iter()
            .map(|index| chain.resolvers()[index].token())
            .collect()
    }

    #[test]
    fn a_resolver_only_serves_the_types_it_declares() {
        assert!(Type::Requestable.accepts(Type::Delivery), "the player resolver takes all");
        assert!(Type::Requestable.accepts(Type::Crafting));
        assert!(Type::Deliverable.accepts(Type::Deliverable));
        assert!(Type::Deliverable.accepts(Type::Tool), "Tool implements IDeliverable");
        assert!(!Type::Deliverable.accepts(Type::Delivery), "Delivery is not a deliverable");
        assert!(
            !Type::Deliverable.accepts(Type::Crafting),
            "AbstractCrafting is not IDeliverable"
        );
        assert!(Type::Retryable.accepts(Type::Deliverable), "IRetryable is a Deliverable's parent");
        assert!(!Type::Retryable.accepts(Type::Delivery), "Delivery is not retryable");
        assert!(Type::Delivery.accepts(Type::Delivery));
        assert!(Type::Pickup.accepts(Type::Pickup));
        assert!(!Type::Pickup.accepts(Type::Delivery));
        assert!(Type::Crafting.accepts(Type::Crafting));
        assert_eq!(Type::Deliverable.supertypes().len(), 3, "Deliverable, Retryable, Requestable");
    }

    #[test]
    fn the_order_is_priority_first_and_declaration_order_within_a_tier() {
        let chain = Chain::new(vec![
            warehouse(1),
            building(2),
            warehouse(3),
            Resolver::player(token(4)),
        ]);
        // The player resolver declares `IRequestable`
        // (`StandardPlayerRequestResolver.java:59-62`), which a `Stack` is
        // (`Stack.java:29`), so `RequestHandler.java:132-156` puts it in the list
        // for a deliverable request too — last, at priority 0.
        assert_eq!(
            asked(&chain, Type::Deliverable),
            vec![token(2), token(1), token(3), token(4)],
            "200, 150, 150, then the player resolver at 0"
        );
        // Membership goes the other way: a resolver is a candidate when its
        // declared type is *in the request's* supertypes (`RequestHandler.java:151`).
        // A request declared exactly `IRequestable` therefore reaches only a
        // resolver declared exactly `IRequestable` — the three deliverable ones
        // drop out. No concrete requestable has that type; this is the boundary.
        assert_eq!(asked(&chain, Type::Requestable), vec![token(4)]);
        // A Delivery is not an IDeliverable, so the three building resolvers are
        // not candidates at all and only the player resolver is left.
        assert_eq!(asked(&chain, Type::Delivery), vec![token(4)]);
        assert_eq!(chain.len(), 4);
        assert_eq!(chain.resolvers().len(), 4);
    }

    #[test]
    fn a_resolver_of_the_wrong_type_is_not_even_asked() {
        let chain = Chain::new(vec![
            warehouse(1),
            Resolver::building(token(2), Source::Default, [2, 64, 0]).serving(Type::Delivery),
        ]);
        assert_eq!(
            asked(&chain, Type::Deliverable),
            vec![token(1)],
            "a deliveryman resolver does not answer a Stack"
        );
        assert_eq!(
            asked(&chain, Type::Delivery),
            vec![token(2)],
            "and the warehouse does not answer a Delivery"
        );
        let retrying = Chain::new(vec![Resolver::retrying(3)]);
        assert_eq!(asked(&retrying, Type::Deliverable), vec![Token::retrying(3)]);
        assert!(asked(&retrying, Type::Delivery).is_empty(), "a Delivery is not IRetryable");
    }

    #[test]
    fn a_token_declared_twice_is_asked_once_at_its_best_position() {
        let chain = Chain::new(vec![warehouse(1), building(2), warehouse(1)]);
        assert_eq!(asked(&chain, Type::Deliverable), vec![token(2), token(1)]);
        assert_eq!(chain.order(Type::Deliverable).len(), 2, "three declarations, two resolvers");
        assert_eq!(chain.pick(Type::Deliverable, &Script::new(), &[]).holder(), Some(token(2)));
    }

    #[test]
    fn the_highest_tier_wins_even_when_it_is_further_away() {
        let chain = Chain::new(vec![warehouse(1), building(2)]);
        let supply = Script::new().scores(token(1), 0).scores(token(2), 999);
        let outcome = chain.pick(Type::Deliverable, &supply, &[]);
        assert_eq!(
            outcome.holder(),
            Some(token(2)),
            "priority 200 before 150, whatever the metric says"
        );
        assert!(matches!(outcome, Outcome::Assigned { metric: 999, .. }));
        assert_eq!(
            chain.pick(Type::Deliverable, &supply, &[]),
            Outcome::Assigned {
                holder: token(2),
                at: Some([2, 64, 0]),
                metric: 999,
                children: Vec::new(),
                superseded: Vec::new(),
            }
        );
    }

    #[test]
    fn within_one_tier_the_lowest_metric_wins_and_a_tie_keeps_the_first() {
        let chain = Chain::new(vec![warehouse(1), warehouse(2), warehouse(3)]);
        let nearest = Script::new().scores(token(1), 9).scores(token(2), 2).scores(token(3), 5);
        assert_eq!(chain.pick(Type::Deliverable, &nearest, &[]).holder(), Some(token(2)));
        // All three are scored: an untested resolver in `Script` answers the
        // default metric of zero, which is the *best* score and would legitimately
        // replace the holder (`RequestHandler.java:197-205`) — not a tie at all.
        let tied =
            Script::new().scores(token(1), 4).scores(token(2), 4).scores(token(3), 6);
        assert_eq!(
            chain.pick(Type::Deliverable, &tied, &[]).holder(),
            Some(token(1)),
            "a strict < keeps the one already in hand (:198)"
        );
        let beaten = Script::new().scores(token(1), 4).scores(token(2), 3).scores(token(3), 6);
        assert_eq!(
            chain.pick(Type::Deliverable, &beaten, &[]).holder(),
            Some(token(2)),
            "one below is enough to replace it"
        );
    }

    #[test]
    fn a_skipped_resolver_is_neither_asked_nor_answered() {
        let chain = Chain::new(vec![warehouse(1), warehouse(2)]);
        let supply = Script::new().wants(token(1), vec![7]);
        let outcome = chain.pick(Type::Deliverable, &supply, &[token(1)]);
        assert_eq!(outcome.holder(), Some(token(2)), "the blacklisted one is passed over");
        assert!(outcome.children().is_empty(), "and its children never existed");
        assert_eq!(chain.pick(Type::Deliverable, &supply, &[token(1), token(2)]), Outcome::Nobody);
    }

    #[test]
    fn a_resolver_that_says_no_is_passed_over_twice_over() {
        let chain = Chain::new(vec![warehouse(1), warehouse(2)]);
        let refused_early = Script::new().cannot(token(1));
        assert_eq!(chain.pick(Type::Deliverable, &refused_early, &[]).holder(), Some(token(2)));
        let refused_late = Script::new().refuses(token(1));
        assert_eq!(chain.pick(Type::Deliverable, &refused_late, &[]).holder(), Some(token(2)));
    }

    #[test]
    fn nobody_able_to_supply_is_a_normal_answer() {
        let chain = Chain::new(vec![warehouse(1)]);
        let refuses = Script::new().refuses(token(1));
        assert_eq!(chain.pick(Type::Deliverable, &refuses, &[]), Outcome::Nobody);
        assert!(Chain::new(Vec::new()).is_empty());
        assert_eq!(Chain::new(Vec::new()).pick(Type::Deliverable, &Script::new(), &[]), Outcome::Nobody);
        assert!(Outcome::Nobody.children().is_empty());
        assert!(Outcome::Nobody.superseded().is_empty());
        assert_eq!(Outcome::Nobody.holder(), None);
    }

    #[test]
    fn a_replaced_attempt_hands_back_its_children_to_be_cancelled() {
        let chain = Chain::new(vec![warehouse(1), warehouse(2)]);
        let supply = Script::new()
            .scores(token(1), 8)
            .scores(token(2), 1)
            .wants(token(1), vec![70, 71])
            .wants(token(2), vec![72]);
        let outcome = chain.pick(Type::Deliverable, &supply, &[]);
        assert_eq!(outcome.holder(), Some(token(2)), "the better metric replaced it");
        assert_eq!(outcome.children(), &[72][..]);
        assert_eq!(outcome.superseded(), &[70, 71][..], "the dropped attempt's children");
        assert!(matches!(outcome, Outcome::Assigned { metric: 1, .. }));
    }

    #[test]
    fn a_lower_tier_is_never_consulted_once_a_tier_answered() {
        // The warehouse answers; a default-priority resolver with a perfect metric
        // is behind it and must never be reached (RequestHandler.java:174-177).
        let chain = Chain::new(vec![
            warehouse(1),
            Resolver::building(token(2), Source::Default, [2, 64, 0]),
        ]);
        let supply = Script::new().scores(token(1), 5_000).scores(token(2), 0);
        assert_eq!(chain.pick(Type::Deliverable, &supply, &[]).holder(), Some(token(1)));
    }

    #[test]
    fn a_failed_tier_does_not_stop_the_next_one() {
        let chain = Chain::new(vec![warehouse(1), Resolver::player(token(2))]);
        let supply = Script::new().refuses(token(1));
        assert_eq!(
            chain.pick(Type::Deliverable, &supply, &[]).holder(),
            Some(token(2)),
            "the player resolver is the last attempt, not no attempt"
        );
        let nobody_but_the_player = Script::new().cannot(token(1)).scores(token(2), 42);
        assert_eq!(
            chain.pick(Type::Deliverable, &nobody_but_the_player, &[]).holder(),
            Some(token(2))
        );
    }

    #[test]
    fn a_crafting_loop_is_the_same_item_at_a_count_that_does_not_grow() {
        let target = Wanted::new(ladder(), 8);
        assert!(creates_cycle(&Wanted::new(ladder(), 8), &target, false, &[]));
        assert!(creates_cycle(&Wanted::new(ladder(), 2), &target, false, &[]), "fewer is a loop");
        assert!(!creates_cycle(&Wanted::new(ladder(), 9), &target, false, &[]), "more is progress");
        assert!(!creates_cycle(&Wanted::new(stone(), 1), &target, false, &[]), "another item");
    }

    #[test]
    fn a_request_may_be_compared_with_itself_only_when_java_says_so() {
        let target = Wanted::new(ladder(), 8);
        let itself = Wanted::new(ladder(), 8);
        assert!(
            !creates_cycle(&itself, &target, true, &[]),
            "the :141 call passes the request as its own target"
        );
        assert!(creates_cycle(&itself, &target, false, &[]));
        // Up one level the identity test no longer applies, so the ancestor is
        // compared with the target — and `AbstractCraftingRequestResolver:240`
        // also demands a count that is not going up: four is a loop, sixty-four
        // is progress (making more of it cannot be why you need it).
        let small_parent = [Wanted::new(ladder(), 4)];
        assert!(creates_cycle(&Wanted::new(ladder(), 80), &target, true, &small_parent));
        let big_parent = [Wanted::new(ladder(), 64)];
        assert!(
            !creates_cycle(&Wanted::new(ladder(), 80), &target, true, &big_parent),
            "64 > 8: the ancestor asks for more than the target, so it is not a loop"
        );
    }

    #[test]
    fn a_chain_deeper_than_twenty_is_refused_as_a_loop() {
        let target = Wanted::new(ladder(), 1);
        let unrelated = |count: u32| Wanted::new(stone(), count);
        let mut ancestors: Vec<Wanted> = (0..20).map(unrelated).collect();
        assert!(
            !creates_cycle(&unrelated(1), &target, false, &ancestors),
            "twenty levels of unrelated items is not a loop"
        );
        ancestors.push(unrelated(21));
        assert!(
            creates_cycle(&unrelated(1), &target, false, &ancestors),
            "twenty-one is past MAX_CRAFTING_CYCLE_DEPTH"
        );
    }
}
