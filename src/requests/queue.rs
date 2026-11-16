//! The file: in what order the colony serves the requests it holds.
//!
//! Java reference: `core/colony/buildings/modules/WarehouseRequestQueueModule.java`
//! (the list a warehouse keeps, `:22-98`), `core/colony/jobs/JobDeliveryman.java`
//! (the pick, `:237-253`, and the anti-starvation step, `:279-287`),
//! `requestable/deliveryman/AbstractDeliverymanRequestable.java` (what a step of
//! aging is worth, `:96-101`).
//!
//! **The order is a rule, not a sort the caller repeats.** Java keeps the requests
//! in arrival order in a plain `List` (`WarehouseRequestQueueModule.java:22`) and
//! finds the one to serve by scanning it, keeping the strictly greater score
//! (`JobDeliveryman.java:248`) — so a tie goes to the request that waited longest.
//! [`Queue::order`] states that order once and [`Queue::next`] reads its first
//! element; nothing here re-sorts the stored list, so arrival order stays the age
//! every other rule reads.
//!
//! **What the scan scores, and what this port leaves out.** Java's score is
//! `priority + (list size - position) - (int) sqrt(source.distManhattan(target))`,
//! with a thousand off an unloaded target and a hundred off a pickup not due yet
//! (`JobDeliveryman.java:187-206`). This port keeps the first two terms — priority,
//! then age — and drops the distance one: it needs both ends of a *delivery* (a
//! source held by a warehouse tile entity, a target held by the requester), and
//! there is no delivery requestable in between. The report names it.
//!
//! **One refusal Java does not have.** `addRequest` appends a token without looking
//! (`WarehouseRequestQueueModule.java:82-86`); a token already in the list would be
//! served twice. [`Queue::push`] answers [`Refusal::Duplicate`].

use std::cmp::Reverse;

use mod_api::Compound;

use super::nbt;
use super::request::Request;
use super::Refusal;

/// The list's key. Java saves the identities as `{Token, Request}` pairs under
/// `List` (`StandardRequestIdentitiesDataStore.java:96-110`); this port saves the
/// records whole under the field's own name, the id being inside each one.
pub const KEY_REQUESTS: &str = "requests";

/// The requests a colony holds, in arrival order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Queue {
    entries: Vec<Request>,
}

impl Queue {
    pub fn new() -> Queue {
        Queue { entries: Vec::new() }
    }

    /// A queue over requests that already exist, in the order given: that order is
    /// the age, so a caller building one from a save keeps the saved order.
    pub fn with_requests(entries: Vec<Request>) -> Queue {
        Queue { entries }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Every request, in arrival order — oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &Request> {
        self.entries.iter()
    }

    pub fn get(&self, id: i32) -> Option<&Request> {
        self.entries.iter().find(|request| request.id() == id)
    }

    pub fn get_mut(&mut self, id: i32) -> Option<&mut Request> {
        self.entries.iter_mut().find(|request| request.id() == id)
    }

    /// File a request: it goes to the back, which makes it the youngest.
    /// `Err(Duplicate)` for an id already in the list.
    pub fn push(&mut self, request: Request) -> Result<(), Refusal> {
        if self.get(request.id()).is_some() {
            return Err(Refusal::Duplicate { id: request.id() });
        }
        self.entries.push(request);
        Ok(())
    }

    /// Take one out, by id: what a resolver, a requester or a cancellation does
    /// (`DeliverymenRequestResolver.java:154` removes by token).
    pub fn remove(&mut self, id: i32) -> Option<Request> {
        let index = self.entries.iter().position(|request| request.id() == id)?;
        Some(self.entries.remove(index))
    }

    /// Take out every request filed by the building at `at`, in arrival order. Java
    /// groups by destination when it batches a deliveryman's tasks
    /// (`JobDeliveryman.java:294`); one building is one destination, so its position
    /// is this port's key.
    pub fn remove_for(&mut self, at: [i32; 3]) -> Vec<Request> {
        let (taken, kept): (Vec<Request>, Vec<Request>) =
            self.entries.drain(..).partition(|request| request.requester().at == at);
        self.entries = kept;
        taken
    }

    /// The service order as indices into the arrival order: higher priority first,
    /// and the older request first among equals (`JobDeliveryman.java:248-252`). The
    /// list itself is untouched, so an index stays an age.
    pub fn order(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = (0..self.entries.len()).collect();
        indices.sort_by_key(|&index| (Reverse(self.entries[index].priority()), index));
        indices
    }

    /// The request to serve now, or `None` on an empty queue.
    pub fn next(&self) -> Option<&Request> {
        self.order().first().and_then(|&index| self.entries.get(index))
    }

    /// One step of aging for every request `chosen` left waiting: the ones filed
    /// before it in arrival order gain one priority point, up to the aging ceiling
    /// (`JobDeliveryman.java:279-287`, `AbstractDeliverymanRequestable.java:99-100`).
    /// Returns how many were raised.
    ///
    /// "Before it" is the arrival order, as in Java — not the service order: the
    /// step exists so a request nobody picks eventually overtakes, and Java ages
    /// whatever sat earlier in the list.
    pub fn age_waiting(&mut self, chosen: i32) -> Result<usize, Refusal> {
        let position = self
            .entries
            .iter()
            .position(|request| request.id() == chosen)
            .ok_or(Refusal::Unknown { id: chosen })?;
        let waiting = &mut self.entries[..position];
        let raised = waiting.len();
        for request in waiting.iter_mut() {
            request.raise_priority();
        }
        Ok(raised)
    }

    /// The saved form: one compound per request, in arrival order. A queue whose
    /// order was lost would lose its ages with it.
    pub fn to_compound(&self) -> Compound {
        Compound::from([(
            KEY_REQUESTS.into(),
            nbt::compound_list(self.entries.iter().map(Request::to_compound).collect()),
        )])
    }

    /// Reads a queue. A request that cannot be read is **skipped**, not guessed: a
    /// queue with one fewer entry still serves the rest, and an absent list is an
    /// empty queue.
    pub fn from_compound(data: &Compound) -> Queue {
        Queue::with_requests(
            nbt::compounds(data, KEY_REQUESTS)
                .into_iter()
                .filter_map(Request::from_compound)
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::ItemName;
    use crate::requests::request::{KEY_ID, Requester};

    const HUT: [i32; 3] = [10, 64, 10];
    const OTHER: [i32; 3] = [20, 64, 10];

    fn request(id: i32, priority: i32, at: [i32; 3]) -> Request {
        let item = ItemName::new("minecraft:ladder").expect("a valid name");
        let mut request = Request::new(id, item, 4, 4, Requester::new("minecolonies:blockhutbuilder", at))
            .expect("a nonzero request");
        request.set_priority(priority);
        request
    }

    fn filed(entries: Vec<Request>) -> Queue {
        let mut queue = Queue::new();
        for entry in entries {
            queue.push(entry).expect("a fresh id");
        }
        queue
    }

    /// The ids in service order, which is what every ordering test asserts.
    fn ids(queue: &Queue) -> Vec<i32> {
        queue
            .order()
            .into_iter()
            .map(|index| queue.iter().nth(index).expect("an index from order()").id())
            .collect()
    }

    #[test]
    fn the_highest_priority_goes_first_and_equals_by_arrival() {
        let mut queue = filed(vec![request(1, 2, HUT), request(2, 9, HUT), request(3, 9, OTHER)]);
        assert_eq!(ids(&queue), vec![2, 3, 1], "nine before two, the older nine first");
        queue.get_mut(1).expect("id 1 is there").set_priority(10);
        assert_eq!(queue.next().map(Request::id), Some(1), "nothing moved");
    }

    #[test]
    fn a_mixed_set_is_served_by_priority_then_by_age() {
        let queue = filed(vec![
            request(1, 0, HUT),
            request(2, 5, OTHER),
            request(3, 0, HUT),
            request(4, 10, OTHER),
            request(5, 5, HUT),
        ]);
        assert_eq!(ids(&queue), vec![4, 2, 5, 1, 3]);
        assert_eq!(queue.next().map(Request::id), Some(4));
    }

    #[test]
    fn taking_one_out_keeps_the_age_of_the_rest() {
        let mut queue = filed(vec![request(1, 0, HUT), request(2, 0, HUT), request(3, 0, HUT)]);
        assert_eq!(queue.remove(2).map(|taken| taken.id()), Some(2));
        assert_eq!(queue.len(), 2);
        assert_eq!(ids(&queue), vec![1, 3], "the third did not become the second");
        assert_eq!(queue.remove(2), None, "and it is gone only once");
    }

    #[test]
    fn an_id_already_in_the_file_is_refused() {
        let mut queue = filed(vec![request(1, 0, HUT)]);
        assert_eq!(queue.push(request(1, 7, OTHER)), Err(Refusal::Duplicate { id: 1 }));
        assert_eq!(queue.get(1).map(Request::priority), Some(0), "the first one stands");
    }

    #[test]
    fn every_request_of_one_building_comes_out_together() {
        let mut queue = filed(vec![request(1, 0, HUT), request(2, 0, OTHER), request(3, 0, HUT)]);
        assert_eq!(
            queue.remove_for(HUT).iter().map(Request::id).collect::<Vec<i32>>(),
            vec![1, 3],
            "in arrival order"
        );
        assert_eq!(queue.len(), 1);
        assert!(queue.remove_for(HUT).is_empty());
        assert_eq!(queue.next().map(Request::id), Some(2));
    }

    #[test]
    fn ageing_raises_what_a_pick_left_waiting_and_stops_at_the_ceiling() {
        let mut queue = filed(vec![request(1, 0, HUT), request(2, 0, HUT), request(3, 1, HUT)]);
        assert_eq!(queue.next().map(Request::id), Some(3));
        assert_eq!(queue.age_waiting(3), Ok(2), "the two filed before it");
        assert_eq!(queue.get(1).map(Request::priority), Some(1));
        assert_eq!(queue.next().map(Request::id), Some(1), "now the oldest of the three");
        for _ in 0..20 {
            queue.age_waiting(3).expect("still there");
        }
        assert_eq!(queue.get(2).map(Request::priority), Some(14), "Java's MAX_AGING_PRIORITY");
        assert_eq!(queue.age_waiting(3), Ok(2), "a pass at the ceiling still counts");
        assert_eq!(queue.get(2).map(Request::priority), Some(14), "and changes nothing");
        assert_eq!(queue.age_waiting(99), Err(Refusal::Unknown { id: 99 }));
    }

    #[test]
    fn a_queue_round_trips_and_something_unreadable_is_skipped() {
        let mut first = request(1, 3, HUT);
        first.assign(HUT).expect("assign");
        let queue = filed(vec![first, request(2, 0, OTHER)]);
        let read = Queue::from_compound(&queue.to_compound());
        assert_eq!(read, queue);
        assert_eq!(read.next().map(Request::id), Some(1), "and keeps serving in order");

        let mut data = queue.to_compound();
        let mut entries: Vec<Compound> =
            nbt::compounds(&data, KEY_REQUESTS).into_iter().cloned().collect();
        entries[1].insert(KEY_ID.into(), mod_api::Tag::String("nonsense".into()));
        data.insert(KEY_REQUESTS.into(), nbt::compound_list(entries));
        assert_eq!(Queue::from_compound(&data).len(), 1, "the broken one is dropped");
        assert!(Queue::from_compound(&Compound::new()).is_empty());
    }

    #[test]
    fn an_empty_queue_has_nothing_to_serve() {
        let queue = Queue::new();
        assert!(queue.is_empty());
        assert_eq!(queue.order(), Vec::<usize>::new());
        assert_eq!(queue.next(), None);
        assert_eq!(Queue::default(), queue);
        assert_eq!(Queue::from_compound(&queue.to_compound()), queue);
    }
}
