//! Who supplies a request: the choice rule, as a pure function.
//!
//! Java reference: `api/colony/requestsystem/resolver/IRequestResolver.java`
//! (priority, eligibility, suitability metric),
//! `core/colony/requestsystem/management/handlers/RequestHandler.java:130-225` (the
//! loop that picks). The candidates — a hut that holds the item, a warehouse that
//! stores it — are passed in; this file never looks one up, so it needs no colony,
//! no host and no game.
//!
//! **The choice, in three steps, exactly as Java orders them.**
//!
//! 1. **The highest priority kind wins outright.** Resolvers are sorted by
//!    descending priority and only the first tier is tried: the loop `break`s the
//!    moment the priority changes (`RequestHandler.java:158-159`, `:174-177`). A hut
//!    holding the item beats a warehouse next door, whatever the distance.
//! 2. **Inside one tier the lowest suitability metric wins**, and a tie keeps the
//!    earlier candidate: the comparison is a strict `<`
//!    (`RequestHandler.java:197-198`), so the one already in hand stays.
//! 3. **A candidate that cannot supply is not a candidate**: a warehouse with none
//!    of the item is skipped (`AbstractWarehouseRequestResolver.java:103-107`), and a
//!    hut needs the request's **minimum** count in its own stock
//!    (`BuildingRequestResolver.java:93`) — the same threshold the warehouse path
//!    applies once it has summed every warehouse
//!    (`AbstractWarehouseRequestResolver.java:122`).
//!
//! **What is not ported.** The sum that threshold is applied to. Java adds up every
//! warehouse's stock before answering (`:111-122`) and files a child request for
//! what is missing (`:226-227`); that is the network, not the rule. A candidate here
//! is one place with one stock, and the caller decides whether to ask a second one.
//! Same split for `StandardPlayerRequestResolver`: its **tier** is here as
//! [`Source::Player`] (priority `0`, the last attempt), its packet is not.

use crate::inventory::{Inventory, ItemName};

use super::request::Request;

/// Which kind of supplier answers, and the resolver priority Java gives it
/// (`api/util/constant/RSConstants.java:11-16`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The hut that asked, from its own stock — `CONST_BUILDING_RESOLVER_PRIORITY`.
    Building,
    /// A warehouse — `CONST_WAREHOUSE_RESOLVER_PRIORITY`.
    Warehouse,
    /// A crafting resolver — `CONST_CRAFTING_RESOLVER_PRIORITY`.
    Crafting,
    /// Anything that does not say — `CONST_DEFAULT_RESOLVER_PRIORITY`.
    Default,
    /// The retrying resolver, tried last — `CONST_RETRYING_RESOLVER_PRIORITY`.
    Retrying,
    /// Someone who carries the item already — `STANDARD_PLAYER_REQUEST_PRIORITY`,
    /// the very last attempt (`StandardPlayerRequestResolver.java:151-154`). Java
    /// then sends a player a packet and waits; the port keeps only the tier.
    Player,
}

impl Source {
    /// Every kind, highest priority first.
    pub const ALL: [Source; 6] = [
        Source::Building,
        Source::Warehouse,
        Source::Crafting,
        Source::Default,
        Source::Retrying,
        Source::Player,
    ];

    /// The resolver priority: `200`, `150`, `125`, `100`, `50`, `0`
    /// (`RSConstants.java:11-16`).
    pub fn priority(self) -> i32 {
        match self {
            Source::Building => 200,
            Source::Warehouse => 150,
            Source::Crafting => 125,
            Source::Default => 100,
            Source::Retrying => 50,
            Source::Player => 0,
        }
    }
}

/// One place that might supply: its stock, where it stands, and what kind it is.
///
/// The stock is the working copy [`Inventory`] already models, not a count passed
/// by hand: the quantity a rule compares is the one the container really holds
/// (`src/inventory/slots.rs:84-91`).
pub struct Candidate<'a> {
    source: Source,
    at: [i32; 3],
    inventory: &'a Inventory<'a>,
    /// Requests already waiting here, for `Source::Warehouse`: the request queue's
    /// length (`AbstractWarehouseRequestResolver.java:360`).
    queue: u32,
}

impl<'a> Candidate<'a> {
    pub fn new(source: Source, at: [i32; 3], inventory: &'a Inventory<'a>) -> Candidate<'a> {
        Candidate { source, at, inventory, queue: 0 }
    }

    /// The same candidate with `queue` requests already waiting at it. Only the
    /// warehouse metric reads it; Java counts it for deliverymen too (`:360`).
    pub fn with_queue(mut self, queue: u32) -> Candidate<'a> {
        self.queue = queue;
        self
    }

    pub fn at(&self) -> [i32; 3] {
        self.at
    }

    /// How many of `item` this place holds right now.
    pub fn available(&self, item: &ItemName) -> u32 {
        self.inventory.count(item)
    }
}

/// The distance Java measures between two positions: the three-dimensional
/// Euclidean one, `sqrt(dx² + dy² + dz²)` (`api/util/BlockPosUtil.java:453-460`).
/// Not Manhattan, not 2D: a hut one floor up is one block further away.
pub fn distance(from: [i32; 3], to: [i32; 3]) -> f64 {
    let dx = i64::from(from[0]) - i64::from(to[0]);
    let dy = i64::from(from[1]) - i64::from(to[1]);
    let dz = i64::from(from[2]) - i64::from(to[2]);
    ((dx * dx + dy * dy + dz * dz) as f64).sqrt()
}

/// The suitability metric, lower is better — Java's `getSuitabilityMetric`.
///
/// Three formulas, taken as they are: a warehouse scores
/// `max(distance / 10, 1) + queue length` (`AbstractWarehouseRequestResolver.java:352-361`),
/// with Java's integer division and truncation, so a warehouse under ten blocks away
/// scores the same as one adjacent; a crafting resolver scores the raw truncated
/// distance (`AbstractCraftingRequestResolver.java:103-105`); everything else scores
/// `0`, the interface's default (`IRequestResolver.java:126-129`), which is what the
/// building, default, retrying and player resolvers answer
/// (`AbstractRequestResolver.java:46-49`).
pub fn metric(candidate: &Candidate<'_>, requester_at: [i32; 3]) -> i32 {
    let blocks = distance(requester_at, candidate.at) as i32;
    match candidate.source {
        Source::Warehouse => (blocks / 10).max(1) + candidate.queue as i32,
        Source::Crafting => blocks,
        Source::Building | Source::Default | Source::Retrying | Source::Player => 0,
    }
}

/// Whether `candidate` may supply `request` at all.
///
/// Two rules, both from the Java: it must hold at least the request's **minimum**
/// count (`BuildingRequestResolver.java:93`,
/// `AbstractWarehouseRequestResolver.java:122`); and a warehouse never answers its
/// own request (`:74-77`). A hut does answer its own — through the building
/// resolver, which only considers the building that asked
/// (`BuildingRequestResolver.java:33-38`) — so the same position is legal for
/// `Source::Building` and for nothing else.
pub fn eligible(candidate: &Candidate<'_>, request: &Request) -> bool {
    if candidate.at == request.requester().at && candidate.source != Source::Building {
        return false;
    }
    candidate.available(request.item()) >= request.min_count()
}

/// The best candidate for `request`, by index, or `None` when nobody can supply.
///
/// The index is the caller's: the list is not reordered and the candidate is not
/// consumed — what to do with the choice (assign the request, take the stacks, file
/// the delivery) is a decision this layer does not make.
pub fn choose(request: &Request, candidates: &[Candidate<'_>]) -> Option<usize> {
    let top = candidates
        .iter()
        .filter(|candidate| eligible(candidate, request))
        .map(|candidate| candidate.source.priority())
        .max()?;
    let mut best: Option<(usize, i32)> = None;
    for (index, candidate) in candidates.iter().enumerate() {
        if candidate.source.priority() != top || !eligible(candidate, request) {
            continue;
        }
        let score = metric(candidate, request.requester().at);
        if best.map_or(true, |(_, previous)| score < previous) {
            best = Some((index, score));
        }
    }
    best.map(|(index, _)| index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{ItemLimits, Stack};
    use crate::requests::request::Requester;

    const HUT: [i32; 3] = [0, 64, 0];

    fn planks() -> ItemName {
        ItemName::new("minecraft:oak_planks").expect("a valid name")
    }

    fn limits() -> ItemLimits {
        ItemLimits::one(planks(), 64)
    }

    fn stocked(inventory: &mut Inventory<'_>, count: u32) {
        if count > 0 {
            let stack = Stack::new(planks(), count).expect("a nonzero stack");
            assert!(inventory.set_slot(0, Some(stack)));
        }
    }

    fn asking(min_count: u32) -> Request {
        Request::new(1, planks(), 64, min_count, Requester::new("minecolonies:blockhutbuilder", HUT))
            .expect("a nonzero request")
    }

    #[test]
    fn a_hut_that_holds_it_answers_before_a_much_closer_warehouse() {
        let limits = limits();
        let (mut hut, mut warehouse) = (Inventory::new(1, &limits), Inventory::new(1, &limits));
        stocked(&mut hut, 4);
        stocked(&mut warehouse, 64);
        let candidates = [
            Candidate::new(Source::Building, [40, 64, 40], &hut),
            Candidate::new(Source::Warehouse, [1, 64, 0], &warehouse),
        ];
        assert_eq!(metric(&candidates[0], HUT), 0, "a building's metric is the default");
        assert_eq!(metric(&candidates[1], HUT), 1, "a warehouse next door scores one");
        assert_eq!(choose(&asking(4), &candidates), Some(0), "priority 200 before 150");
    }

    #[test]
    fn within_one_kind_the_nearest_wins_but_a_busy_one_loses() {
        let limits = limits();
        let (mut near, mut far) = (Inventory::new(1, &limits), Inventory::new(1, &limits));
        stocked(&mut near, 64);
        stocked(&mut far, 64);
        let request = asking(4);
        let nearby = [
            Candidate::new(Source::Warehouse, [5, 64, 0], &near),
            Candidate::new(Source::Warehouse, [60, 64, 0], &far),
        ];
        assert_eq!(choose(&request, &nearby), Some(0), "five blocks away, nothing waiting");
        let reversed = [
            Candidate::new(Source::Warehouse, [60, 64, 0], &far),
            Candidate::new(Source::Warehouse, [5, 64, 0], &near),
        ];
        assert_eq!(choose(&request, &reversed), Some(1), "the same one, other index");
        let busy = [
            Candidate::new(Source::Warehouse, [5, 64, 0], &near).with_queue(8),
            Candidate::new(Source::Warehouse, [60, 64, 0], &far),
        ];
        assert_eq!(metric(&busy[0], HUT), 9, "one block plus eight waiting");
        assert_eq!(choose(&request, &busy), Some(1), "the queue outweighs the walk");
    }

    #[test]
    fn a_supplier_short_of_the_minimum_is_not_asked() {
        let limits = limits();
        let (mut short, mut enough) = (Inventory::new(1, &limits), Inventory::new(1, &limits));
        stocked(&mut short, 2);
        stocked(&mut enough, 4);
        let candidates = [
            Candidate::new(Source::Building, [0, 64, 4], &short),
            Candidate::new(Source::Warehouse, [90, 64, 90], &enough),
        ];
        assert!(!eligible(&candidates[0], &asking(4)), "two is below the minimum of four");
        assert_eq!(choose(&asking(4), &candidates), Some(1));
        assert_eq!(choose(&asking(2), &candidates), Some(0), "with a minimum of two it qualifies");
    }

    #[test]
    fn nobody_able_to_supply_is_none() {
        let limits = limits();
        let mut one = Inventory::new(1, &limits);
        stocked(&mut one, 1);
        let empty = Inventory::new(1, &limits);
        let candidates = [
            Candidate::new(Source::Building, [0, 64, 4], &one),
            Candidate::new(Source::Warehouse, [9, 64, 9], &empty),
        ];
        assert_eq!(choose(&asking(4), &candidates), None);
        assert_eq!(choose(&asking(4), &[]), None);
    }

    #[test]
    fn a_warehouse_never_answers_its_own_request_but_a_hut_does() {
        let limits = limits();
        let mut own = Inventory::new(1, &limits);
        stocked(&mut own, 64);
        let request = asking(4);
        assert!(!eligible(&Candidate::new(Source::Warehouse, HUT, &own), &request));
        assert!(eligible(&Candidate::new(Source::Building, HUT, &own), &request));
        assert_eq!(choose(&request, &[Candidate::new(Source::Warehouse, HUT, &own)]), None);
        assert_eq!(choose(&request, &[Candidate::new(Source::Building, HUT, &own)]), Some(0));
    }

    #[test]
    fn the_metric_of_every_kind_is_the_one_java_answers() {
        let limits = limits();
        let mut stock = Inventory::new(1, &limits);
        stocked(&mut stock, 64);
        let far = [0, 64, 100];
        for (source, expected) in [
            (Source::Building, 0),
            (Source::Default, 0),
            (Source::Retrying, 0),
            (Source::Player, 0),
            (Source::Crafting, 100),
            (Source::Warehouse, 10),
        ] {
            assert_eq!(metric(&Candidate::new(source, far, &stock), HUT), expected, "{source:?}");
        }
        let busy = Candidate::new(Source::Warehouse, far, &stock).with_queue(4);
        assert_eq!(metric(&busy, HUT), 14, "ten from the distance, four from the queue");
        let priorities: Vec<i32> = Source::ALL.iter().map(|kind| kind.priority()).collect();
        assert_eq!(priorities, vec![200, 150, 125, 100, 50, 0], "RSConstants.java:11-16");
    }

    #[test]
    fn the_distance_is_three_dimensional_like_the_java_one() {
        assert_eq!(distance([0, 0, 0], [3, 4, 0]), 5.0, "the 3-4-5 triangle");
        assert_eq!(distance([0, 0, 0], [0, 9, 0]), 9.0, "a floor up counts");
        let limits = limits();
        let mut stock = Inventory::new(1, &limits);
        stocked(&mut stock, 64);
        for (at, expected) in [([9, 64, 0], 1), ([10, 64, 0], 1), ([19, 64, 0], 1), ([20, 64, 0], 2)] {
            let candidate = Candidate::new(Source::Warehouse, at, &stock);
            assert_eq!(metric(&candidate, HUT), expected, "{at:?}: distance/10 truncates");
        }
    }
}
