//! Who holds a request, and which citizen is working it.
//!
//! Java reference: `api/colony/requestsystem/data/IAssignmentDataStore.java`
//! (a key→values store with a reverse lookup, `:22-35`),
//! `core/.../management/handlers/ResolverHandler.java:342-386` (adding and
//! removing a request on a resolver), `:130-169` and `:280-288` (dropping a
//! resolver, and the two refusals it can meet),
//! `core/colony/requestsystem/data/StandardRequestResolverRequestAssignmentDataStore.java`
//! (the map itself), and
//! `core/colony/requestsystem/data/StandardRequestSystemBuildingDataStore.java:105-130`
//! (the citizen book and its `-1` sentinel).
//!
//! **Two different "who", and Java keeps them in two different maps.**
//!
//! * [`Assignment`] answers *which resolver holds the request* — the map Java
//!   writes in `addRequestToResolver` (`ResolverHandler.java:351`) and reads
//!   through `getAssignmentForValue` (`IAssignmentDataStore.java:32-35`). This
//!   is the right to move the request: a resolver that is not the holder has no
//!   business calling `begin` or `deliver` on it.
//! * [`Book`] answers *which citizen on that building is working it*. Java keeps
//!   `citizenByOpenRequest` beside two per-citizen lists, and parks a request
//!   nobody works yet under the citizen id **`-1`**
//!   (`StandardRequestSystemBuildingDataStore.java:113`).
//!
//! **The reverse lookup is the one rule the port tightens.** Java's
//! `getAssignmentForValue` scans the map's keys and answers the *first* one that
//! contains the value (`IAssignmentDataStore.java:34`); a request written into
//! two resolvers' sets therefore has a holder that depends on map iteration
//! order. [`Assignment::bind`] refuses the second resolver instead
//! ([`super::Refusal::AlreadyAssigned`]). Two holders for one request is a bug
//! that would deliver twice, not a state to tolerate.
//!
//! **The citizen book copies a trap on purpose.** `moveToSyncCitizen` reads
//! `citizenByOpenRequest.getOrDefault(id, Integer.MAX_VALUE) == -1`
//! (`:108`): a request with **no entry at all** is *not* adopted, it is silently
//! left ownerless. That reads like an accident, but it is the behaviour of every
//! save in the wild, so the port keeps the rule and names the refusal
//! ([`super::Refusal::NotWaiting`]) instead of repeating the silence.

use std::collections::{BTreeMap, BTreeSet};

use super::token::Token;
use super::Refusal;

/// The citizen id Java parks an unclaimed request under
/// (`StandardRequestSystemBuildingDataStore.java:113`).
pub const WAITING: i32 = -1;

/// Which resolver holds which request — Java's
/// `IRequestResolverRequestAssignmentDataStore`, both directions.
///
/// The per-resolver list keeps the order the requests were added in. Java uses a
/// `HashSet` there (`ResolverHandler.java:346`, `:351`), so its order is whatever
/// the hash gives; the retrying resolver replays that list every tick
/// (`StandardRetryingRequestResolver.java:151-158`), and a list that reshuffles
/// itself between ticks makes a bug unrepeatable.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Assignment {
    by_request: BTreeMap<i32, Token>,
    by_resolver: BTreeMap<Token, Vec<i32>>,
    known: BTreeSet<Token>,
}

impl Assignment {
    pub fn new() -> Assignment {
        Assignment::default()
    }

    /// Declare a resolver that holds nothing yet: Java's identities store knows
    /// it before any request is assigned (`ResolverHandler.java:88`).
    pub fn register(&mut self, holder: Token) {
        self.known.insert(holder);
    }

    /// Whether this book has heard of `holder` at all.
    pub fn knows(&self, holder: Token) -> bool {
        self.known.contains(&holder)
    }

    /// How many requests are assigned across all resolvers.
    pub fn len(&self) -> usize {
        self.by_request.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_request.is_empty()
    }

    /// Give `id` to `holder` — `addRequestToResolver`
    /// (`ResolverHandler.java:342-354`).
    ///
    /// Re-binding the same request to the same holder is a no-op, as asking twice
    /// is not stealing from yourself. Another holder is
    /// [`Refusal::AlreadyAssigned`].
    pub fn bind(&mut self, id: i32, holder: Token) -> Result<(), Refusal> {
        match self.by_request.get(&id) {
            Some(current) if *current == holder => return Ok(()),
            Some(current) => return Err(Refusal::AlreadyAssigned { id, holder: *current }),
            None => {}
        }
        self.register(holder);
        self.by_request.insert(id, holder);
        let held = self.by_resolver.entry(holder).or_default();
        if !held.contains(&id) {
            held.push(id);
        }
        Ok(())
    }

    /// Give a request up — `removeRequestFromResolver`
    /// (`ResolverHandler.java:366-386`): the entry disappears when it was the
    /// resolver's last one (`:382-385`).
    pub fn release(&mut self, id: i32) -> Option<Token> {
        let holder = self.by_request.remove(&id)?;
        if let Some(held) = self.by_resolver.get_mut(&holder) {
            held.retain(|&held_id| held_id != id);
            if held.is_empty() {
                self.by_resolver.remove(&holder);
            }
        }
        Some(holder)
    }

    /// Which resolver holds `id` — Java's `getAssignmentForValue`
    /// (`IAssignmentDataStore.java:32-35`), answered from an index instead of a
    /// scan.
    pub fn holder_of(&self, id: i32) -> Option<Token> {
        self.by_request.get(&id).copied()
    }

    /// The requests `holder` holds, in the order it received them — what
    /// `getAllAssignedRequests` answers (`ResolverHandler.java:257-267`). An
    /// unknown resolver answers nothing rather than failing, as Java does.
    pub fn requests_of(&self, holder: Token) -> &[i32] {
        self.by_resolver.get(&holder).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Something to work on next for `holder`, or `None` when it holds nothing.
    pub fn first_of(&self, holder: Token) -> Option<i32> {
        self.requests_of(holder).first().copied()
    }

    /// Stop a resolver that holds nothing — `removeResolver`
    /// (`ResolverHandler.java:130-169`). Unknown is [`Refusal::UnknownResolver`]
    /// (`:133-136`); still holding is [`Refusal::ResolverInUse`] (`:160-166`).
    pub fn release_holder(&mut self, holder: Token) -> Result<(), Refusal> {
        if !self.known.contains(&holder) {
            return Err(Refusal::UnknownResolver { token: holder });
        }
        let held = self.requests_of(holder).len();
        if held > 0 {
            return Err(Refusal::ResolverInUse { token: holder, held });
        }
        self.known.remove(&holder);
        self.by_resolver.remove(&holder);
        Ok(())
    }

    /// Everything a resolver holds, handed back so the caller can reassign it —
    /// `removeResolverWithAssignedRequests`
    /// (`ResolverHandler.java:218-249`), which reassigns rather than refusing.
    pub fn take_all(&mut self, holder: Token) -> Result<Vec<i32>, Refusal> {
        if !self.known.contains(&holder) {
            return Err(Refusal::UnknownResolver { token: holder });
        }
        let taken = self.by_resolver.remove(&holder).unwrap_or_default();
        for id in &taken {
            if self.holder_of(*id) == Some(holder) {
                self.by_request.remove(id);
            }
        }
        Ok(taken)
    }
}

/// Who is working a request, or that nobody is yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    /// Parked under [`WAITING`]: filed, and no citizen has taken it.
    Waiting,
    /// A citizen with this id.
    Citizen(i32),
}

impl Owner {
    /// The id Java stores, sentinel included.
    pub fn id(self) -> i32 {
        match self {
            Owner::Waiting => WAITING,
            Owner::Citizen(id) => id,
        }
    }

    /// Read back what [`Owner::id`] wrote.
    pub fn parse(id: i32) -> Owner {
        if id == WAITING { Owner::Waiting } else { Owner::Citizen(id) }
    }
}

/// What moving a request onto a citizen changed. Java moves the open list and the
/// completed list under two separate conditions
/// (`StandardRequestSystemBuildingDataStore.java:108-127`), so it can move one
/// and not the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Moved {
    /// The request was waiting and is now the citizen's.
    pub open: bool,
    /// The request was in the completed-under-`-1` list and moved with it.
    pub completed: bool,
}

impl Moved {
    /// Whether anything at all happened.
    pub fn any(self) -> bool {
        self.open || self.completed
    }
}

/// `citizenByOpenRequest` and the two per-citizen lists
/// (`StandardRequestSystemBuildingDataStore.java:88-127`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Book {
    owner: BTreeMap<i32, Owner>,
    open: BTreeMap<i32, Vec<i32>>,
    completed: BTreeMap<i32, Vec<i32>>,
}

impl Book {
    pub fn new() -> Book {
        Book::default()
    }

    /// File a request as waiting for a citizen: it lands under [`WAITING`]
    /// (`:113`) before anyone is picked.
    pub fn park(&mut self, id: i32) {
        self.owner.insert(id, Owner::Waiting);
        push_once(self.open.entry(WAITING).or_default(), id);
    }

    /// File a request as already completed under [`WAITING`]: what a building
    /// does with a finished request before a citizen has seen it (`:120-127`).
    pub fn park_completed(&mut self, id: i32) {
        push_once(self.completed.entry(WAITING).or_default(), id);
    }

    /// Who works `id`. `None` means the book has no entry for it — which is
    /// **not** the same as [`Owner::Waiting`], and the difference is the trap
    /// `move_to_citizen` keeps.
    pub fn owner_of(&self, id: i32) -> Option<Owner> {
        self.owner.get(&id).copied()
    }

    /// The requests waiting under one owner, in order.
    pub fn open_of(&self, owner: i32) -> &[i32] {
        self.open.get(&owner).map(Vec::as_slice).unwrap_or(&[])
    }

    /// The completed requests filed under one owner, in order.
    pub fn completed_of(&self, owner: i32) -> &[i32] {
        self.completed.get(&owner).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Hand a request to a citizen — `moveToSyncCitizen`
    /// (`StandardRequestSystemBuildingDataStore.java:105-130`).
    ///
    /// The first citizen to take a waiting request keeps it; a later call
    /// changes nothing in the open list (`:108`). The completed list is moved on
    /// its own condition (`:120`), so the two halves are reported separately.
    ///
    /// A request the book has no entry for is [`Refusal::NotWaiting`]: Java
    /// would skip it in silence, and silence here means a citizen that never
    /// picks up its own request.
    pub fn move_to_citizen(&mut self, id: i32, citizen: i32) -> Result<Moved, Refusal> {
        let owner = self.owner.get(&id).copied().ok_or(Refusal::NotWaiting { id })?;

        let open = match owner {
            Owner::Waiting => {
                self.owner.insert(id, Owner::Citizen(citizen));
                take_from(self.open.get_mut(&WAITING), id);
                push_once(self.open.entry(citizen).or_default(), id);
                true
            }
            Owner::Citizen(_) => false,
        };

        let completed = if self.completed_of(WAITING).contains(&id) {
            take_from(self.completed.get_mut(&WAITING), id);
            push_once(self.completed.entry(citizen).or_default(), id);
            true
        } else {
            false
        };

        Ok(Moved { open, completed })
    }
}

fn push_once(list: &mut Vec<i32>, id: i32) {
    if !list.contains(&id) {
        list.push(id);
    }
}

fn take_from(list: Option<&mut Vec<i32>>, id: i32) -> bool {
    match list {
        Some(list) => {
            let before = list.len();
            list.retain(|&held| held != id);
            list.len() != before
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HUT: [i32; 3] = [10, 64, 10];
    const OTHER: [i32; 3] = [20, 64, 10];

    fn holder(n: u128) -> Token {
        Token::new(n)
    }

    #[test]
    fn binding_is_idempotent_and_a_second_holder_is_refused() {
        let mut book = Assignment::new();
        assert_eq!(book.bind(7, holder(1)), Ok(()));
        assert_eq!(book.holder_of(7), Some(holder(1)));
        assert_eq!(book.bind(7, holder(1)), Ok(()), "asking twice is not stealing");
        assert_eq!(
            book.bind(7, holder(2)),
            Err(Refusal::AlreadyAssigned { id: 7, holder: holder(1) })
        );
        assert_eq!(book.holder_of(7), Some(holder(1)), "the first holder stands");
        assert_eq!(book.requests_of(holder(2)).len(), 0, "and got nothing");
    }

    #[test]
    fn a_holder_keeps_the_order_it_received_its_requests_in() {
        let mut book = Assignment::new();
        for id in [5, 3, 9, 3] {
            book.bind(id, holder(1)).expect("a fresh request");
        }
        assert_eq!(book.requests_of(holder(1)), &[5, 3, 9][..], "duplicates collapse, order holds");
        assert_eq!(book.first_of(holder(1)), Some(5));
        assert_eq!(book.len(), 3);
        assert!(!book.is_empty());
        assert_eq!(book.first_of(holder(2)), None);
    }

    #[test]
    fn releasing_a_request_empties_both_directions_and_drops_the_key() {
        let mut book = Assignment::new();
        book.bind(1, holder(1)).expect("bind");
        book.bind(2, holder(1)).expect("bind");
        assert_eq!(book.release(1), Some(holder(1)));
        assert_eq!(book.holder_of(1), None);
        assert_eq!(book.requests_of(holder(1)), &[2][..], "the other one stays");
        assert_eq!(book.release(2), Some(holder(1)));
        assert_eq!(book.requests_of(holder(1)).len(), 0, "empty, as ResolverHandler.java:382-385");
        assert_eq!(book.release(2), None, "and only once");
    }

    #[test]
    fn a_holder_that_still_holds_something_cannot_be_dropped() {
        let mut book = Assignment::new();
        book.bind(1, holder(1)).expect("bind");
        assert_eq!(
            book.release_holder(holder(1)),
            Err(Refusal::ResolverInUse { token: holder(1), held: 1 })
        );
        assert_eq!(book.take_all(holder(1)), Ok(vec![1]), "the way out is reassigning");
        assert_eq!(book.len(), 0);
        assert_eq!(book.release_holder(holder(1)), Ok(()), "now it holds nothing");
        assert!(!book.knows(holder(1)));
    }

    #[test]
    fn an_unknown_holder_is_named_and_answers_nothing() {
        let mut book = Assignment::new();
        assert_eq!(
            book.release_holder(holder(3)),
            Err(Refusal::UnknownResolver { token: holder(3) })
        );
        assert_eq!(
            book.take_all(holder(3)),
            Err(Refusal::UnknownResolver { token: holder(3) })
        );
        assert!(book.requests_of(holder(3)).is_empty(), "reading is not an error, as :257-267");
        book.register(holder(4));
        assert!(book.knows(holder(4)));
        assert_eq!(book.release_holder(holder(4)), Ok(()), "known and idle");
    }

    #[test]
    fn taking_everything_back_hands_it_out_in_order() {
        let mut book = Assignment::new();
        for id in [4, 2, 8] {
            book.bind(id, holder(9)).expect("bind");
        }
        assert_eq!(book.take_all(holder(9)), Ok(vec![4, 2, 8]));
        assert_eq!(book.holder_of(4), None);
        assert_eq!(book.len(), 0);
        assert_eq!(book.take_all(holder(9)), Ok(Vec::new()), "known, and now empty");
    }

    #[test]
    fn a_parked_request_moves_to_the_citizen_that_takes_it() {
        let mut book = Book::new();
        book.park(11);
        assert_eq!(book.owner_of(11), Some(Owner::Waiting));
        assert_eq!(book.open_of(WAITING), &[11][..]);
        assert_eq!(book.move_to_citizen(11, 3), Ok(Moved { open: true, completed: false }));
        assert_eq!(book.owner_of(11), Some(Owner::Citizen(3)));
        assert_eq!(book.open_of(WAITING).len(), 0);
        assert_eq!(book.open_of(3), &[11][..]);
        assert_eq!(Owner::parse(WAITING), Owner::Waiting);
        assert_eq!(Owner::Citizen(3).id(), 3);
    }

    #[test]
    fn the_first_citizen_keeps_it_and_the_completed_list_moves_on_its_own() {
        let mut book = Book::new();
        book.park(12);
        book.park_completed(12);
        assert_eq!(book.move_to_citizen(12, 3), Ok(Moved { open: true, completed: true }));
        assert_eq!(book.completed_of(3), &[12][..]);
        assert_eq!(
            book.move_to_citizen(12, 4),
            Ok(Moved { open: false, completed: false }),
            "the first owner stands, and the completed list already moved"
        );
        assert_eq!(book.owner_of(12), Some(Owner::Citizen(3)));
        assert_eq!(book.open_of(4).len(), 0);
    }

    #[test]
    fn a_completed_request_moves_without_an_open_one() {
        let mut book = Book::new();
        book.park_completed(13);
        assert_eq!(book.move_to_citizen(13, 5), Err(Refusal::NotWaiting { id: 13 }), "no owner");
        book.park(13);
        assert_eq!(book.move_to_citizen(13, 5), Ok(Moved { open: true, completed: true }));
        assert!(!Moved { open: false, completed: false }.any());
        assert!(Moved { open: true, completed: false }.any());
    }

    #[test]
    fn a_request_the_book_never_heard_of_is_refused_instead_of_skipped() {
        let mut book = Book::new();
        assert_eq!(
            book.move_to_citizen(99, 1),
            Err(Refusal::NotWaiting { id: 99 }),
            "StandardRequestSystemBuildingDataStore.java:108 would skip it in silence"
        );
        assert_eq!(book.owner_of(99), None, "and None is not Waiting");
        assert_ne!(book.owner_of(99), Some(Owner::Waiting));
        assert!(book.open_of(1).is_empty());
    }
}
