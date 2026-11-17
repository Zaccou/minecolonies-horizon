//! The numbers the request system is bounded by, and the rules built on them.
//!
//! Java reference: `core/.../resolvers/StandardRetryingRequestResolver.java:31`
//! (`MAX_RETRIES`), `:32` (`RETRY_DELAY`), `:64-71` (both exposed as the two
//! methods the retry loop reads), `:93-96` (the refusal once they are spent),
//! `:109-110` (parking a request), `:150-158` (the tick that counts down) and
//! `:159` (the blacklist that ends it); `api/util/constant/Constants.java:108`
//! (`MAX_CRAFTING_CYCLE_DEPTH`); and
//! `api/colony/requestsystem/requestable/deliveryman/AbstractDeliverymanRequestable.java:15-18`
//! (the four priority ceilings).
//!
//! **How long a retryable request lives.** Three parks at twelve hundred ticks
//! each: `3 × 1200 = 3600` ticks, three minutes at twenty ticks a second, and
//! then the retrying resolver blacklists itself for that request and it stops
//! being reassigned (`StandardRetryingRequestResolver.java:159`). That is the
//! whole of "expiry" in this system — Java has no wall-clock deadline for a
//! request, and this port does not invent one: a request that is never retried
//! simply waits, which is what a warehouse with nothing in it does.
//!
//! **How many requests a colony may hold: no answer, and that is Java's answer.**
//! `WarehouseRequestQueueModule.addRequest` appends to a `List` without looking at
//! its length (`:68-72`), and `ServerConfiguration` has no request-system limit
//! (`:110-114`: the only key under that heading is `creativeResolve`). Java is
//! unbounded. [`Capacity`] exists because a wasm guest must bound its own memory
//! before the host does it for it, and its default — [`Capacity::unbounded`] — is
//! exactly Java, so a caller that does not think about it gets no change in
//! behaviour.
//!
//! **The four priorities, and which one wins.** A building sets at most 10
//! (`:15`); aging adds one per pass up to 14 (`:17`, `:96-101`), so an old
//! delivery eventually outranks a fresh maximum one; a delivery the system itself
//! files starts at 13 (`:16`), between the two; and a player pressing "pick up
//! now" gets 15 (`:18`), above everything the automatic rules can reach. The
//! column is [`Origin`], and [`Origin::ceiling`] is the only place these numbers
//! are stated.

/// The highest priority a player may set in a building's window
/// (`AbstractDeliverymanRequestable.java:15`). Already modelled by the request
/// itself (`requests/request.rs:35`), re-exported so this file is the one place
/// every bound is read from.
pub use crate::requests::MAX_BUILDING_PRIORITY;

use super::Refusal;
use super::token::Token;

/// The highest priority aging may hand out (`:17`), four above the player's own
/// ceiling: past it, a very old request competes with a fresh maximum one rather
/// than overtaking it forever (`:98-100`).
pub const MAX_AGING_PRIORITY: i32 = 14;

/// The priority a delivery the system files for itself starts at (`:16`).
pub const DEFAULT_DELIVERY_PRIORITY: i32 = 13;

/// The priority of a player asking for a delivery right now (`:18`). Above
/// [`MAX_AGING_PRIORITY`], which is why it is the only priority nothing automatic
/// can beat.
pub const PLAYER_ACTION_PRIORITY: i32 = 15;

/// How many times the retrying resolver will park one request before giving up
/// (`StandardRetryingRequestResolver.java:31`).
pub const MAX_TRIES: u32 = 3;

/// The delay one park is worth, in ticks (`:32`).
pub const RETRY_DELAY_TICKS: i32 = 1200;

/// How many levels up the parent chain a crafting resolver walks before it calls
/// the chain a loop (`Constants.java:108`).
pub const MAX_CRAFTING_CYCLE_DEPTH: u32 = 20;

/// Where a priority came from, which is the same thing as its ceiling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four ceilings Java defines
pub enum Origin {
    /// A player, in the building's window: [`MAX_BUILDING_PRIORITY`].
    Building,
    /// The aging pass: [`MAX_AGING_PRIORITY`].
    Aging,
    /// The system itself, filing a delivery: [`DEFAULT_DELIVERY_PRIORITY`].
    Delivery,
    /// A player asking now: [`PLAYER_ACTION_PRIORITY`].
    Player,
}

impl Origin {
    /// The highest value this origin may reach.
    pub fn ceiling(self) -> i32 {
        match self {
            Origin::Building => MAX_BUILDING_PRIORITY,
            Origin::Aging => MAX_AGING_PRIORITY,
            Origin::Delivery => DEFAULT_DELIVERY_PRIORITY,
            Origin::Player => PLAYER_ACTION_PRIORITY,
        }
    }

    /// `value`, kept inside what this origin may ask.
    ///
    /// **The ceiling is Java's; the floor of zero is the port's.** Java's only
    /// writer of the priority field is the constructor (`:27-30`) and the aging
    /// step (`:100`), which is `Math.min(getMaxAgingPriority(true), priority + 1)`
    /// — a cap with no floor at all. There is **no `setPriority`** anywhere in
    /// `api/colony/requestsystem/`; the constants are *described* as "the maximum
    /// priority allowed to be set in the building GUI" (`:46-47`), and
    /// `WindowHutRequestTaskModule.java:112` only displays the value. The port
    /// adds the floor because a negative priority would sort behind a request
    /// nobody set one on, and nothing in Java produces one.
    pub fn clamp(self, value: i32) -> i32 {
        value.clamp(0, self.ceiling())
    }
}

/// The retry budget: how many parks, and how long each is worth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Retries {
    max: u32,
    delay: i32,
}

impl Retries {
    /// The budget Java ships with: three tries, twelve hundred ticks each
    /// (`StandardRetryingRequestResolver.java:31-32`).
    pub fn java() -> Retries {
        Retries { max: MAX_TRIES, delay: RETRY_DELAY_TICKS }
    }

    /// Another budget, for a test or for a colony that configures one. A delay of
    /// zero is refused by clamping to one: a park of no time would retry on the
    /// very next tick, forever.
    pub fn new(max: u32, delay: i32) -> Retries {
        Retries { max, delay: delay.max(1) }
    }

    pub fn max(self) -> u32 {
        self.max
    }

    pub fn delay(self) -> i32 {
        self.delay
    }

    /// How long one request can live in the retrying resolver, in ticks:
    /// `max × delay`, 3600 for [`Retries::java`].
    pub fn lifetime(self) -> i32 {
        let lifetime = i64::from(self.max) * i64::from(self.delay);
        i32::try_from(lifetime).unwrap_or(i32::MAX)
    }
}

impl Default for Retries {
    fn default() -> Retries {
        Retries::java()
    }
}

/// What one tick of the retry clock says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: a clock either fires or it does not
pub enum Due {
    /// Still counting down.
    NotYet,
    /// The delay reached zero: reassign now (`:156-158`).
    Now,
}

/// One request's retry state: how many times it was parked, and how long until the
/// next attempt.
///
/// Java keeps two maps keyed by request token
/// (`StandardRetryingRequestResolver.java:38-39`); one [`Attempt`] per request is
/// the same state with the key outside, which is what lets it be tested without a
/// colony.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attempt {
    budget: Retries,
    tries: u32,
    delay: i32,
}

impl Attempt {
    /// A request that has never been parked.
    pub fn new(budget: Retries) -> Attempt {
        Attempt { budget, tries: 0, delay: 0 }
    }

    /// A request the retrying resolver has just taken: the delay is wound back up
    /// and the attempt counted — `resolveRequest`
    /// (`StandardRetryingRequestResolver.java:107-111`).
    ///
    /// [`Refusal::TryBudgetSpent`] when the budget is already used: Java stops
    /// asking this resolver at that point (`:159`, the self-blacklist), so a
    /// caller that respects [`Self::accepts_more`] never sees this. It is here
    /// because the alternative is a silently ignored park.
    pub fn park(&mut self, id: i32) -> Result<(), Refusal> {
        if !self.accepts_more() {
            return Err(Refusal::TryBudgetSpent { id });
        }
        self.tries += 1;
        self.delay = self.budget.delay;
        Ok(())
    }

    /// One tick of the resolver's clock (`:150-158`).
    ///
    /// Java decrements with `--` (`:153`) and then keeps only the entries that
    /// read **exactly** zero (`:157`), so a delay that goes past zero never fires
    /// again. The port subtracts with the same consequence: after [`Due::Now`] the
    /// same attempt answers [`Due::NotYet`] until it is parked again.
    pub fn tick(&mut self) -> Due {
        self.delay = self.delay.wrapping_sub(1);
        if self.delay == 0 { Due::Now } else { Due::NotYet }
    }

    /// How many times it has been parked.
    pub fn tries(&self) -> u32 {
        self.tries
    }

    /// Ticks left before the next attempt; zero or below means the clock has
    /// already fired.
    pub fn delay(&self) -> i32 {
        self.delay
    }

    pub fn budget(&self) -> Retries {
        self.budget
    }

    /// Whether this resolver may take the request again.
    ///
    /// **The bound is the self-blacklist, not `canResolveRequest`.**
    /// `canResolveRequest` (`:93-96`) is
    /// `getCurrentlyBeingReassignedRequest() == null || id != current || getCurrentReassignmentAttempt() < getMaximalTries()`,
    /// and `getCurrentReassignmentAttempt` answers `-1` while a reassignment is
    /// under way (`:76`) — so the third disjunct is always true, and that method
    /// **never refuses**. What stops a fourth park is `:159`: once the count
    /// reaches `MAX_RETRIES`, the resolver puts itself on the blacklist of the
    /// reassignment and `RequestHandler.java:169` skips it.
    pub fn accepts_more(&self) -> bool {
        self.tries < self.budget.max
    }

    /// Whether the resolver must now put itself on the request's blacklist
    /// (`:159`): that is exactly "the budget is used up".
    pub fn gives_up(&self) -> bool {
        !self.accepts_more()
    }

    /// The blacklist this resolver builds for one request —
    /// `StandardRetryingRequestResolver.java:159`:
    /// `assignedRequests.get(t) < getMaximalTries() ? ImmutableSet.of() : ImmutableSet.of(id)`.
    ///
    /// Hand it to [`super::Chain::pick`]'s `skip` and the resolver stops being
    /// asked for that request, which is what makes the third park the last one.
    pub fn blocklist(&self, own: Token) -> Vec<Token> {
        if self.gives_up() { vec![own] } else { Vec::new() }
    }
}

/// How many requests the colony may hold. Java's answer is "no bound"
/// (`WarehouseRequestQueueModule.java:68-72`); [`Capacity::unbounded`] is that
/// answer, and the default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capacity {
    limit: Option<usize>,
}

impl Capacity {
    /// What Java does: nothing stops a queue from growing.
    pub fn unbounded() -> Capacity {
        Capacity { limit: None }
    }

    /// A ceiling the port adds for its own memory. Not a Java rule.
    pub fn with_limit(limit: usize) -> Capacity {
        Capacity { limit: Some(limit) }
    }

    pub fn limit(self) -> Option<usize> {
        self.limit
    }

    /// Whether one more request may be filed into a queue that already holds
    /// `queued`. [`Refusal::QueueFull`] carries the ceiling, so a log line says
    /// which one was hit.
    pub fn admits(self, queued: usize) -> Result<(), Refusal> {
        match self.limit {
            None => Ok(()),
            Some(limit) if queued < limit => Ok(()),
            Some(limit) => Err(Refusal::QueueFull { limit }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_numbers_are_the_ones_the_java_files_hold() {
        assert_eq!(MAX_BUILDING_PRIORITY, 10, "AbstractDeliverymanRequestable.java:15");
        assert_eq!(MAX_AGING_PRIORITY, 14, ":17");
        assert_eq!(DEFAULT_DELIVERY_PRIORITY, 13, ":16");
        assert_eq!(PLAYER_ACTION_PRIORITY, 15, ":18");
        assert_eq!(MAX_TRIES, 3, "StandardRetryingRequestResolver.java:31");
        assert_eq!(RETRY_DELAY_TICKS, 1200, ":32");
        assert_eq!(MAX_CRAFTING_CYCLE_DEPTH, 20, "Constants.java:108");
    }

    #[test]
    fn aging_passes_the_player_and_a_player_action_passes_aging() {
        assert!(Origin::Aging.ceiling() > Origin::Building.ceiling());
        assert!(Origin::Player.ceiling() > Origin::Aging.ceiling());
        assert!(Origin::Delivery.ceiling() > Origin::Building.ceiling());
        assert!(Origin::Delivery.ceiling() < Origin::Aging.ceiling());
        assert_eq!(Origin::Building.clamp(99), 10);
        assert_eq!(Origin::Aging.clamp(99), 14);
        assert_eq!(Origin::Player.clamp(99), 15);
        assert_eq!(Origin::Delivery.clamp(-3), 0, "nothing produces a negative priority");
        assert_eq!(Origin::Building.clamp(4), 4, "below the ceiling is left alone");
    }

    #[test]
    fn one_request_can_live_three_parked_tries_of_twelve_hundred_ticks() {
        let budget = Retries::java();
        assert_eq!(budget.max(), 3);
        assert_eq!(budget.delay(), 1200);
        assert_eq!(budget.lifetime(), 3600, "three minutes at twenty ticks a second");
        assert_eq!(Retries::default(), budget);
        assert_eq!(Retries::new(2, 0).delay(), 1, "a zero delay would retry every tick");
        assert_eq!(Retries::new(u32::MAX, i32::MAX).lifetime(), i32::MAX, "clamped, not wrapped");
    }

    #[test]
    fn a_fresh_attempt_has_no_try_and_no_clock() {
        let attempt = Attempt::new(Retries::java());
        assert_eq!(attempt.tries(), 0);
        assert_eq!(attempt.delay(), 0);
        assert!(attempt.accepts_more());
        assert!(!attempt.gives_up());
        assert_eq!(attempt.budget(), Retries::java());
    }

    #[test]
    fn parking_winds_the_clock_up_and_counts_one_try() {
        let mut attempt = Attempt::new(Retries::java());
        assert_eq!(attempt.park(7), Ok(()));
        assert_eq!(attempt.tries(), 1);
        assert_eq!(attempt.delay(), 1200, "resolveRequest:109");
        assert_eq!(attempt.tick(), Due::NotYet);
        assert_eq!(attempt.delay(), 1199);
    }

    #[test]
    fn the_clock_fires_once_after_exactly_twelve_hundred_ticks() {
        let mut attempt = Attempt::new(Retries::java());
        attempt.park(7).expect("the first park");
        for tick in 1..RETRY_DELAY_TICKS {
            assert_eq!(attempt.tick(), Due::NotYet, "tick {tick}");
        }
        assert_eq!(attempt.tick(), Due::Now, "the twelve hundredth tick");
        assert_eq!(
            attempt.tick(),
            Due::NotYet,
            "a delay past zero never fires again (StandardRetryingRequestResolver.java:157)"
        );
        assert_eq!(attempt.delay(), -1);
    }

    #[test]
    fn three_parks_are_allowed_and_the_fourth_is_refused() {
        let mut attempt = Attempt::new(Retries::java());
        for _ in 0..MAX_TRIES {
            assert!(attempt.accepts_more());
            assert_eq!(attempt.park(9), Ok(()));
            assert_eq!(attempt.tick(), Due::NotYet, "and the clock restarts each time");
        }
        assert_eq!(attempt.tries(), 3);
        assert!(attempt.gives_up(), "the resolver blacklists itself (:159)");
        assert!(!attempt.accepts_more());
        assert_eq!(attempt.park(9), Err(Refusal::TryBudgetSpent { id: 9 }));
        assert_eq!(attempt.tries(), 3, "a refused park counts nothing");
    }

    #[test]
    fn the_resolver_blacklists_itself_only_once_the_budget_is_spent() {
        // StandardRetryingRequestResolver.java:159 —
        // `assignedRequests.get(t) < getMaximalTries() ? {} : {id}`.
        let own = Token::new(42);
        let mut attempt = Attempt::new(Retries::java());
        assert_eq!(attempt.blocklist(own), Vec::new(), "a fresh request is retried by anybody");
        attempt.park(1).expect("park");
        assert_eq!(attempt.blocklist(own), Vec::new(), "one try left the blacklist empty");
        attempt.park(1).expect("park");
        assert_eq!(attempt.blocklist(own), Vec::new(), "two tries, still empty");
        attempt.park(1).expect("park");
        assert_eq!(attempt.blocklist(own), vec![own], "three tries: it steps aside");
        assert_eq!(attempt.blocklist(Token::new(7)), vec![Token::new(7)], "for that request only");
    }

    #[test]
    fn two_requests_keep_two_independent_budgets() {
        let budget = Retries::java();
        let mut first = Attempt::new(budget);
        let mut second = Attempt::new(budget);
        first.park(1).expect("park");
        first.park(1).expect("park");
        assert_eq!((first.tries(), second.tries()), (2, 0));
        assert!(second.accepts_more());
        assert!(first.accepts_more());
        first.park(1).expect("park");
        assert!(first.gives_up());
        assert!(!second.gives_up(), "one request's budget says nothing about another's");
    }

    #[test]
    fn a_budget_of_one_stops_after_the_first_park() {
        let mut attempt = Attempt::new(Retries::new(1, 5));
        assert_eq!(attempt.park(3), Ok(()));
        assert!(attempt.gives_up());
        assert_eq!(attempt.park(3), Err(Refusal::TryBudgetSpent { id: 3 }));
        for tick in 1..5 {
            assert_eq!(attempt.tick(), Due::NotYet, "tick {tick}");
        }
        assert_eq!(attempt.tick(), Due::Now, "five ticks, as the budget says");
    }

    #[test]
    fn the_java_capacity_is_no_capacity_at_all() {
        let java = Capacity::unbounded();
        assert_eq!(java, Capacity::default());
        assert_eq!(java.limit(), None);
        for queued in [0, 1, 1_000_000] {
            assert_eq!(java.admits(queued), Ok(()), "WarehouseRequestQueueModule.java:68-72");
        }
    }

    #[test]
    fn a_bounded_capacity_refuses_exactly_at_its_limit() {
        let capacity = Capacity::with_limit(3);
        assert_eq!(capacity.limit(), Some(3));
        assert_eq!(capacity.admits(0), Ok(()));
        assert_eq!(capacity.admits(2), Ok(()));
        assert_eq!(capacity.admits(3), Err(Refusal::QueueFull { limit: 3 }));
        assert_eq!(capacity.admits(4), Err(Refusal::QueueFull { limit: 3 }));
        assert_eq!(
            Capacity::with_limit(0).admits(0),
            Err(Refusal::QueueFull { limit: 0 }),
            "a limit of zero admits nothing"
        );
    }
}
