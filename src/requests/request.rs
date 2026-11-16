//! One request: what a building asks for, who asks, and the five states this port
//! can honour.
//!
//! Java reference: `requestable/Stack.java` (the object and the two counts),
//! `request/RequestState.java` (thirteen states), `requests/AbstractRequest.java`
//! (the record: id, requester, state, deliveries), `requester/IRequester.java`
//! (who asks, and from where), `requestable/deliveryman/AbstractDeliverymanRequestable.java`
//! (the priority scale and the aging cap).
//!
//! **Five states, and the reason there are not thirteen.** Java tracks a resolver
//! (`REPORTED`, `ASSIGNING`), its follow-ups (`RESOLVED`, `FOLLOWUP_IN_PROGRESS`),
//! the receipt (`RECEIVED`) and two clean-up states (`FINALIZING`, `FAILED`) —
//! `RequestState.java:13-78`. This port has no resolver, no child requests and no
//! deliveryman, so it keeps the five states a building can actually be in and
//! refuses the rest by name instead of pretending. [`State::as_str`] is what is
//! saved: Java saves an **index into the thirteen-value enum**
//! (`RequestState.java:99-118`), which no five-value enum can honour.
//!
//! **What a transition is worth.** Java's `setState` writes the field and tells the
//! parent (`AbstractRequest.java:137-154`) — it has no legality table at all, the
//! manager is trusted. The port is the layer that has to be trusted, so every move
//! here is a named transition and an illegal one is
//! [`Refusal::IllegalTransition`], carrying both ends.

use mod_api::{Compound, Tag};

use crate::inventory::{ItemName, Stack};

use super::nbt;
use super::Refusal;

/// The highest priority a player may set in a building's window
/// (`AbstractDeliverymanRequestable.java:15`). Zero is the field's default (`:20`)
/// and this port keeps it as "not chosen".
pub const MAX_BUILDING_PRIORITY: i32 = 10;

/// The highest priority the aging mechanism may hand out, four above the player's
/// ceiling (`AbstractDeliverymanRequestable.java:17`). Past it, a very old request
/// competes with a fresh maximum one instead of overtaking it forever.
const MAX_AGING_PRIORITY: i32 = 14;

/// The saved keys. `Count` and `MinCount` are the deliverable's own
/// (`requestable/Stack.java:37-38`), `priority` is the shared tag
/// (`api/util/constant/NbtTagConstants.java:663`), `location` the position (`:145`);
/// the rest are this port's, named after the field they carry.
pub const KEY_ID: &str = "id";
pub const KEY_ITEM: &str = "item";
pub const KEY_COUNT: &str = "Count";
pub const KEY_MIN_COUNT: &str = "MinCount";
pub const KEY_PRIORITY: &str = "priority";
pub const KEY_STATE: &str = "state";
pub const KEY_DELIVERED: &str = "delivered";
pub const KEY_REQUESTER: &str = "requester";
pub const KEY_ASSIGNED_TO: &str = "assignedTo";
pub const KEY_TYPE: &str = "type";
pub const KEY_LOCATION: &str = "location";

/// Where a request stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: a closed set the port matches without a wildcard
pub enum State {
    /// Filed, and nobody has taken it: Java's `CREATED` (`RequestState.java:18`).
    Created,
    /// A supplier took it and has not started: `ASSIGNED` (`:33`).
    Assigned,
    /// Work has begun, or part of the quantity arrived: `IN_PROGRESS` (`:38`).
    InProgress,
    /// The asked quantity is in. Terminal.
    Delivered,
    /// Withdrawn, or impossible. Terminal.
    Cancelled,
}

impl State {
    /// Every state, in the order a request can reach them.
    pub const ALL: [State; 5] =
        [State::Created, State::Assigned, State::InProgress, State::Delivered, State::Cancelled];

    /// Whether the request can still move: the three states that are not final.
    pub fn is_open(self) -> bool {
        matches!(self, State::Created | State::Assigned | State::InProgress)
    }

    /// Whether nothing more can happen to it.
    pub fn is_done(self) -> bool {
        !self.is_open()
    }

    /// The saved name — a string, deliberately: see the module's note on Java's
    /// ordinal.
    pub fn as_str(self) -> &'static str {
        match self {
            State::Created => "created",
            State::Assigned => "assigned",
            State::InProgress => "in_progress",
            State::Delivered => "delivered",
            State::Cancelled => "cancelled",
        }
    }

    /// The state a saved name means. Unknown is `None`: guessing `Created` would
    /// file a request nobody saved.
    pub fn parse(name: &str) -> Option<State> {
        State::ALL.into_iter().find(|state| state.as_str() == name)
    }
}

/// Who asks, and from where. Java's `IRequester` is a token plus an `ILocation`
/// (`requester/IRequester.java:26-31`); a building is found by its position
/// (`resolvers/BuildingRequestResolver.java:186`), and the kind is what a colony
/// record already stores (`src/colony/colonie.rs:94-98`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Requester {
    pub building: String,
    pub at: [i32; 3],
}

impl Requester {
    pub fn new(building: &str, at: [i32; 3]) -> Requester {
        Requester { building: building.to_owned(), at }
    }
}

/// One thing a building wants.
///
/// `count` is what it asked for, `min_count` the least that already counts as
/// enough — clamped to `count`, as `Stack` clamps it (`requestable/Stack.java:178`),
/// because a minimum above the total is a request that can never be satisfied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    id: i32,
    item: ItemName,
    count: u32,
    min_count: u32,
    requester: Requester,
    priority: i32,
    state: State,
    delivered: u32,
    assigned_to: Option<[i32; 3]>,
}

impl Request {
    /// A fresh, unassigned request at priority zero. `Err(EmptyQuantity)` for a
    /// zero total: a request for nothing is not a request.
    pub fn new(
        id: i32,
        item: ItemName,
        count: u32,
        min_count: u32,
        requester: Requester,
    ) -> Result<Request, Refusal> {
        if count == 0 {
            return Err(Refusal::EmptyQuantity);
        }
        Ok(Request {
            id,
            item,
            count,
            min_count: min_count.min(count),
            requester,
            priority: 0,
            state: State::Created,
            delivered: 0,
            assigned_to: None,
        })
    }

    pub fn id(&self) -> i32 {
        self.id
    }

    pub fn item(&self) -> &ItemName {
        &self.item
    }

    pub fn count(&self) -> u32 {
        self.count
    }

    pub fn min_count(&self) -> u32 {
        self.min_count
    }

    pub fn requester(&self) -> &Requester {
        &self.requester
    }

    pub fn priority(&self) -> i32 {
        self.priority
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn delivered(&self) -> u32 {
        self.delivered
    }

    /// The hut that took it, by position, or `None`.
    pub fn assigned_to(&self) -> Option<[i32; 3]> {
        self.assigned_to
    }

    /// What is still missing: `count - delivered`, never negative.
    pub fn outstanding(&self) -> u32 {
        self.count.saturating_sub(self.delivered)
    }

    /// Whether enough has arrived to call the need met. True while the request is
    /// still open is normal — a building that wanted ten and settles for four has
    /// four.
    pub fn satisfied(&self) -> bool {
        self.delivered >= self.min_count
    }

    /// What a player may set, kept inside `0..=MAX_BUILDING_PRIORITY`. Aging is not
    /// a player: [`Self::raise_priority`] climbs to the aging ceiling even above
    /// the player's own (`AbstractDeliverymanRequestable.java:96-101`).
    pub fn set_priority(&mut self, priority: i32) {
        self.priority = priority.clamp(0, MAX_BUILDING_PRIORITY);
    }

    /// One step of the anti-starvation rule: `+1`, capped at the aging ceiling.
    /// Called on the requests a pick left waiting, from [`super::Queue`].
    pub fn raise_priority(&mut self) -> i32 {
        self.priority = self.priority.saturating_add(1).min(MAX_AGING_PRIORITY);
        self.priority
    }

    /// A supplier takes it: `Created → Assigned`. Taking it again with the same
    /// position is a no-op (asking twice is not stealing from itself, the rule
    /// `WorkOrder::claim` already follows); another hut is
    /// [`Refusal::AlreadyAssigned`].
    pub fn assign(&mut self, by: [i32; 3]) -> Result<(), Refusal> {
        match self.state {
            State::Created => {
                self.state = State::Assigned;
                self.assigned_to = Some(by);
                Ok(())
            }
            State::Assigned if self.assigned_to == Some(by) => Ok(()),
            State::Assigned => Err(Refusal::AlreadyAssigned { by: self.assigned_to.unwrap_or(by) }),
            from => Err(Refusal::IllegalTransition { from, to: State::Assigned }),
        }
    }

    /// Work begins: `Assigned → InProgress`. There is no door from `Created`:
    /// something has to be owed to somebody first.
    pub fn begin(&mut self) -> Result<(), Refusal> {
        match self.state {
            State::Assigned => {
                self.state = State::InProgress;
                Ok(())
            }
            from => Err(Refusal::IllegalTransition { from, to: State::InProgress }),
        }
    }

    /// Hand `stack` in, and say how many of it the request took: never more than it
    /// was missing, never zero.
    ///
    /// A partial hand-in leaves it `InProgress`; the one that completes the count
    /// makes it `Delivered`. Only an assigned request takes a delivery: Java's
    /// deliveries come from the resolver the request was assigned to
    /// (`AbstractWarehouseRequestResolver.java:289-297`), never from nobody.
    pub fn deliver(&mut self, stack: &Stack) -> Result<u32, Refusal> {
        let accepted = stack.count().min(self.outstanding());
        let to = if accepted >= self.outstanding() {
            State::Delivered
        } else {
            State::InProgress
        };
        if !matches!(self.state, State::Assigned | State::InProgress) {
            return Err(Refusal::IllegalTransition { from: self.state, to });
        }
        if stack.item() != &self.item {
            return Err(Refusal::WrongItem);
        }
        self.delivered += accepted;
        self.state = if self.delivered >= self.count {
            State::Delivered
        } else {
            State::InProgress
        };
        Ok(accepted)
    }

    /// Withdraw it: the three live states become `Cancelled`, a terminal one stays
    /// put and is refused.
    pub fn cancel(&mut self) -> Result<(), Refusal> {
        if !self.state.is_open() {
            return Err(Refusal::IllegalTransition { from: self.state, to: State::Cancelled });
        }
        self.state = State::Cancelled;
        Ok(())
    }

    /// The saved form. Java keys requests by token in a data store
    /// (`StandardRequestIdentitiesDataStore.java:96-110`); this port keys them by
    /// the id the colony's counter handed out and saves the record whole.
    pub fn to_compound(&self) -> Compound {
        let mut data = Compound::from([
            (KEY_ID.into(), Tag::Int(self.id)),
            (KEY_ITEM.into(), Tag::String(self.item.as_str().into())),
            (KEY_COUNT.into(), saved(self.count)),
            (KEY_MIN_COUNT.into(), saved(self.min_count)),
            (KEY_PRIORITY.into(), Tag::Int(self.priority)),
            (KEY_STATE.into(), Tag::String(self.state.as_str().into())),
            (KEY_DELIVERED.into(), saved(self.delivered)),
            (
                KEY_REQUESTER.into(),
                Tag::compound([
                    (KEY_TYPE, Tag::String(self.requester.building.as_str().into())),
                    (KEY_LOCATION, nbt::position(self.requester.at)),
                ]),
            ),
        ]);
        if let Some(by) = self.assigned_to {
            data.insert(KEY_ASSIGNED_TO.into(), nbt::position(by));
        }
        data
    }

    /// Reads a request. `None` when an essential part is missing or foreign: no id,
    /// no item name the engine can parse, no count, no position, no state.
    ///
    /// **Two contradictions are repaired, not carried.** A request saved as
    /// `Delivered` with a short count gets its count (a delivered request has the
    /// goods); one saved as `Assigned` with nobody named goes back to `Created` —
    /// the assignment lives in the resolver's data store in Java, so a half-write
    /// is possible and guessing a hut would be worse.
    pub fn from_compound(data: &Compound) -> Option<Request> {
        let requester = data.get(&KEY_REQUESTER.into())?;
        let at = nbt::position_of(requester.get(KEY_LOCATION)?)?;
        let building = requester.get(KEY_TYPE).and_then(nbt::text_of).unwrap_or_default();
        let state = State::parse(&data.get(&KEY_STATE.into()).and_then(nbt::text_of)?)?;
        let count = u32::try_from(nbt::int(data, KEY_COUNT)?).ok()?;
        let mut request = Request::new(
            nbt::int(data, KEY_ID)?,
            ItemName::new(&data.get(&KEY_ITEM.into()).and_then(nbt::text_of)?)?,
            count,
            u32::try_from(nbt::int(data, KEY_MIN_COUNT).unwrap_or(0)).unwrap_or(0),
            Requester::new(&building, at),
        )
        .ok()?;
        request.priority = nbt::int(data, KEY_PRIORITY).unwrap_or(0);
        request.delivered =
            u32::try_from(nbt::int(data, KEY_DELIVERED).unwrap_or(0)).unwrap_or(0).min(count);
        request.assigned_to = nbt::position_in(data, KEY_ASSIGNED_TO);
        request.state = match state {
            State::Delivered => {
                request.delivered = count;
                State::Delivered
            }
            State::Assigned if request.assigned_to.is_none() => State::Created,
            State::Assigned => State::Assigned,
            // A supplier can be at work on it: `InProgress` keeps its assignee, exactly
            // as `Assigned` does. Only the states where nobody can be assigned drop it.
            State::InProgress => State::InProgress,
            other => {
                request.assigned_to = None;
                other
            }
        };
        Some(request)
    }
}

/// A count written as an NBT int. Java's `IDeliverable.getCount()` is an `int`
/// (`requestable/IDeliverable.java:24`), so a count past `i32::MAX` has no saved
/// form; it is clamped rather than wrapped into a negative that would read back as
/// no request at all.
fn saved(count: u32) -> Tag {
    Tag::Int(i32::try_from(count).unwrap_or(i32::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HUT: [i32; 3] = [10, 64, 10];
    const OTHER: [i32; 3] = [20, 64, 10];

    fn ladder() -> ItemName {
        ItemName::new("minecraft:ladder").expect("a valid name")
    }

    fn asking(count: u32, min_count: u32) -> Request {
        Request::new(1, ladder(), count, min_count, Requester::new("minecolonies:blockhutbuilder", HUT))
            .expect("a nonzero request")
    }

    fn stack(item: ItemName, count: u32) -> Stack {
        Stack::new(item, count).expect("a nonzero stack")
    }

    #[test]
    fn a_request_for_nothing_is_refused_and_the_minimum_never_exceeds_the_total() {
        assert_eq!(
            Request::new(1, ladder(), 0, 0, Requester::new("hut", HUT)),
            Err(Refusal::EmptyQuantity)
        );
        assert_eq!(asking(4, 9).min_count(), 4, "clamped down to the total");
        assert_eq!(asking(10, 4).min_count(), 4, "and kept below it");
        assert!(!asking(4, 4).satisfied(), "nothing has arrived yet");
    }

    #[test]
    fn assigning_goes_from_created_once_and_to_one_hut() {
        let mut request = asking(4, 4);
        assert_eq!(request.assign(HUT), Ok(()));
        assert_eq!((request.state(), request.assigned_to()), (State::Assigned, Some(HUT)));
        assert_eq!(request.assign(HUT), Ok(()), "asking twice is not stealing");
        assert_eq!(request.assign(OTHER), Err(Refusal::AlreadyAssigned { by: HUT }));
        assert_eq!(request.state(), State::Assigned, "the first claim stands");
    }

    #[test]
    fn work_starts_only_on_something_assigned() {
        let mut request = asking(4, 4);
        assert_eq!(
            request.begin(),
            Err(Refusal::IllegalTransition { from: State::Created, to: State::InProgress })
        );
        request.assign(HUT).expect("assign");
        assert_eq!(request.begin(), Ok(()));
        assert_eq!(request.state(), State::InProgress);
        assert_eq!(
            request.assign(OTHER),
            Err(Refusal::IllegalTransition { from: State::InProgress, to: State::Assigned })
        );
    }

    #[test]
    fn a_full_hand_in_closes_it_and_a_partial_one_leaves_it_open() {
        let mut request = asking(4, 2);
        request.assign(HUT).expect("assign");
        assert_eq!(request.deliver(&stack(ladder(), 2)), Ok(2));
        assert_eq!(request.state(), State::InProgress);
        assert!(request.satisfied(), "two was the minimum it settles for");
        assert_eq!(request.outstanding(), 2, "but two are still missing");
        assert_eq!(request.deliver(&stack(ladder(), 9)), Ok(2), "only what is missing");
        assert_eq!((request.state(), request.delivered()), (State::Delivered, 4));
    }

    #[test]
    fn nothing_is_delivered_before_assignment_and_the_wrong_item_changes_nothing() {
        let mut request = asking(4, 4);
        assert_eq!(
            request.deliver(&stack(ladder(), 1)),
            Err(Refusal::IllegalTransition { from: State::Created, to: State::InProgress })
        );
        request.assign(HUT).expect("assign");
        let planks = ItemName::new("minecraft:oak_planks").expect("a valid name");
        assert_eq!(request.deliver(&stack(planks, 4)), Err(Refusal::WrongItem));
        assert_eq!((request.delivered(), request.state()), (0, State::Assigned));
    }

    #[test]
    fn cancelling_ends_it_from_every_live_state_and_nothing_follows() {
        for live in [State::Created, State::Assigned, State::InProgress] {
            let mut request = asking(4, 4);
            if live != State::Created {
                request.assign(HUT).expect("assign");
            }
            if live == State::InProgress {
                request.begin().expect("begin");
            }
            assert_eq!(request.cancel(), Ok(()));
            assert_eq!(request.state(), State::Cancelled);
            assert!(request.state().is_done());
            for refused in [request.assign(HUT), request.begin(), request.cancel()] {
                assert!(matches!(refused, Err(Refusal::IllegalTransition { .. })));
            }
            assert_eq!(
                request.deliver(&stack(ladder(), 1)),
                Err(Refusal::IllegalTransition { from: State::Cancelled, to: State::InProgress })
            );
        }
    }

    #[test]
    fn a_player_stops_at_ten_and_aging_climbs_to_fourteen() {
        let mut request = asking(4, 4);
        request.set_priority(99);
        assert_eq!(request.priority(), MAX_BUILDING_PRIORITY);
        request.set_priority(-5);
        assert_eq!(request.priority(), 0);
        for _ in 0..20 {
            request.raise_priority();
        }
        assert_eq!(request.priority(), 14, "Java's MAX_AGING_PRIORITY");
        assert!(MAX_AGING_PRIORITY > MAX_BUILDING_PRIORITY, "aging passes the player's ceiling");
    }

    #[test]
    fn a_request_round_trips_with_and_without_an_assignee() {
        let mut request = asking(6, 3);
        assert_eq!(Request::from_compound(&request.to_compound()), Some(request.clone()));
        request.assign(HUT).expect("assign");
        request.set_priority(7);
        request.deliver(&stack(ladder(), 2)).expect("2 of 6");
        let read = Request::from_compound(&request.to_compound()).expect("read");
        assert_eq!(read, request);
        assert_eq!((read.state(), read.assigned_to()), (State::InProgress, Some(HUT)));
    }

    #[test]
    fn a_contradictory_save_is_repaired_or_refused() {
        let mut data = asking(4, 2).to_compound();
        data.insert(KEY_STATE.into(), Tag::String("delivered".into()));
        let read = Request::from_compound(&data).expect("read");
        assert_eq!((read.state(), read.delivered()), (State::Delivered, 4));
        data.insert(KEY_STATE.into(), Tag::String("assigned".into()));
        assert_eq!(
            Request::from_compound(&data).map(|request| request.state()),
            Some(State::Created),
            "assigned with nobody named is nobody assigned"
        );
        data.insert(KEY_STATE.into(), Tag::String("reported".into()));
        assert_eq!(Request::from_compound(&data), None, "a Java-only state is not guessed");
    }

    #[test]
    fn every_state_has_its_own_saved_name() {
        for (index, state) in State::ALL.iter().enumerate() {
            assert_eq!(State::parse(state.as_str()), Some(*state));
            for other in &State::ALL[index + 1..] {
                assert_ne!(state.as_str(), other.as_str());
            }
        }
    }
}
