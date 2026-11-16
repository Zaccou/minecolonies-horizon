//! The splitter: one body too big for one payload, cut into fragments, and back.
//!
//! Java reference: `core/network/messages/splitting/SplitPacketMessage.java` (the
//! fragment: five fields, `toBytes`/`fromBytes` at `:65-82`, the reassembly at
//! `:85-153`) and `core/network/NetworkChannel.java` (the cut at `:434-479`, the
//! counter at `:458`, the cache at `:82-85`).
//!
//! **Does the port need it? Yes, and for the same reason Java does.** The engine
//! refuses one payload body past 1,048,576 bytes
//! (`crates/horizon-mod-vocab/src/payload.rs:148-157`) — and Java's chunk size,
//! 943,718, is "90% of max packet size" (`NetworkChannel.java:451-452`), the same
//! budget. So the two messages that carry a whole `FriendlyByteBuf` — the colony view
//! and a building's, `network::views` — can pass that bound, and nothing but a
//! fragment carrier gets them across. What the port does **not** need is Java's reason
//! for the *cache*: Netty delivers fragments on its own threads, so Java keeps them in
//! a Guava cache with a one-minute access expiry (`:82-85`); the port's assembly is a
//! plain value the caller owns, and time is the caller's to keep.
//!
//! **The five fields, in Java's order of writing.** `communicationId`, `packetIndex`,
//! `terminator`, `innerMessageId`, `payload` (`SplitPacketMessage.java:65-72`). A
//! compound has no order, so the port keeps the *names* Java uses and says so; the
//! numbers are the same numbers. `communicationId` is one per *message*, shared by its
//! fragments, and Java's is an `AtomicInteger` that wraps (`NetworkChannel.java:90`,
//! `:458`) — [`Splitter`] wraps identically rather than refusing at 2^31, because a
//! counter that stops is a channel that stops.
//!
//! **Four things Java does that this does not, each because it loses a message.**
//!
//! 1. **An empty body is sent.** `handleSplitting`'s `while (currentIndex < data.length)`
//!    (`:461`) never runs for a message that serialized to zero bytes, so the message
//!    is silently *not sent*; [`Splitter::split`] emits one terminator fragment with an
//!    empty payload, which reassembles to the empty body it started from.
//! 2. **A gap is refused.** Java concatenates whatever reached the cache, sorted by
//!    index (`SplitPacketMessage.java:103-107`) — a lost fragment yields a shorter body
//!    that `fromBytes` decodes as garbage or throws on. [`Assembly::accept`] answers
//!    [`Refusal::Gap`] and delivers nothing.
//! 3. **A repeated index is refused.** Java's `Map.put` keeps the last one
//!    (`:92`), so a peer can rewrite a fragment it already sent.
//! 4. **Open assemblies are bounded.** Java's cache is unbounded per
//!    `communicationId` and only expires by time; a peer that opens and never
//!    terminates holds memory. [`MAX_OPEN`] is the port's answer, named.
//!
//! **The inner message is not decoded here.** Java looks the id up and *creates* the
//! message (`:110-113`) — and would throw a `NullPointerException` on an id nobody
//! registered, because `getMessagesTypes().get(...)` is never checked. The port hands
//! the reassembled body and its id to [`super::route::Channel::receive`], which has the
//! table and answers [`Refusal::UnknownId`] by name.

use std::collections::BTreeMap;

use mod_api::Compound;

use super::Refusal;
use super::registry::MessageId;
use super::route::MAX_PAYLOAD;
use crate::network::common;

/// The largest fragment Java sends: "943718; //This is 90% of max packet size"
/// (`NetworkChannel.java:451-452`). Kept as-is rather than recomputed, because it is a
/// number on the wire: a peer that sends Java's chunks must be reassembled by it.
pub const MAX_CHUNK: usize = 943_718;

/// How many fragments one message may be cut into. Java has no bound; 64 chunks is
/// 60 MB of body, which is already absurd for a colony view and far past
/// [`MAX_PAYLOAD`] — the bound exists so a `communicationId` cannot be made to
/// allocate without end.
pub const MAX_FRAGMENTS: usize = 64;

/// How many messages may be half-received at once. Java's cache is bounded by a
/// one-minute expiry instead (`NetworkChannel.java:82-85`).
pub const MAX_OPEN: usize = 32;

/// The fragment's five keys, as Java names the fields.
pub const KEY_COMMUNICATION: &str = "communicationId";
pub const KEY_INDEX: &str = "packetIndex";
pub const KEY_TERMINATOR: &str = "terminator";
pub const KEY_INNER: &str = "innerMessageId";
pub const KEY_PAYLOAD: &str = "payload";

/// One piece of a message: which message it belongs to, where it sits, and whether it
/// is the last.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fragment {
    /// `SplitPacketMessage.communicationId` — one per message, shared by its pieces.
    pub communication: i32,
    /// `packetIndex` — from 0, one per piece, in the order they were cut.
    pub index: i32,
    /// `terminator` — true on the last piece only. Java tests this field and nothing
    /// else to decide that a message is complete (`:95-99`).
    pub terminator: bool,
    /// `innerMessageId` — the id's row in [`super::registry::ALL`].
    pub inner: MessageId,
    /// `payload` — `writeByteArray`, so an opaque slice of the serialized message.
    pub body: Vec<u8>,
}

impl Fragment {
    pub fn to_compound(&self) -> Compound {
        Compound::from([
            (KEY_COMMUNICATION.into(), common::int(self.communication)),
            (KEY_INDEX.into(), common::int(self.index)),
            (KEY_TERMINATOR.into(), common::boolean(self.terminator)),
            (KEY_INNER.into(), common::int(i32::from(self.inner.raw()))),
            (KEY_PAYLOAD.into(), common::bytes(&self.body)),
        ])
    }

    /// Reads a fragment, refusing the shapes a sender cannot legally produce: a
    /// negative `communicationId` (Java's `-1` is "not set", `:26-41`), a negative or
    /// oversized `packetIndex`, an inner id past [`u16::MAX`].
    pub fn from_compound(data: &Compound) -> Result<Fragment, Refusal> {
        let communication = common::int_of(data, KEY_COMMUNICATION)?;
        let index = common::int_of(data, KEY_INDEX)?;
        let inner = common::int_in(data, KEY_INNER, 0, i32::from(u16::MAX))?;
        if communication < 0 || index < 0 || index as usize >= MAX_FRAGMENTS {
            return Err(Refusal::BadFragment { communication, index });
        }
        Ok(Fragment {
            communication,
            index,
            terminator: common::bool_of(data, KEY_TERMINATOR)?,
            // `int_in` bounded `inner` to 0..=65535, so this cast is exact.
            inner: MessageId::of(inner as u16),
            body: common::bytes_of(data, KEY_PAYLOAD)?,
        })
    }
}

/// The sending half: Java's `messageCounter` and its `handleSplitting` loop.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Splitter {
    next: i32,
}

impl Splitter {
    pub fn new() -> Splitter {
        Splitter { next: 0 }
    }

    /// The communication id the next message will use, without spending it.
    pub fn peek(&self) -> i32 {
        self.next
    }

    /// Cut `body` into fragments of at most [`MAX_CHUNK`] bytes.
    ///
    /// One fragment for a body that fits, `terminator` set on the last, and every piece
    /// carries `id` so the receiver never has to guess what it is reassembling. A body
    /// of zero bytes still gets one fragment (deviation 1 of the module note).
    pub fn split(&mut self, id: MessageId, body: &[u8]) -> Result<Vec<Fragment>, Refusal> {
        let pieces = body.len().div_ceil(MAX_CHUNK).max(1);
        if pieces > MAX_FRAGMENTS {
            return Err(Refusal::TooMany { count: pieces, limit: MAX_FRAGMENTS });
        }
        let communication = self.next;
        // Java's `AtomicInteger.getAndIncrement()` wraps (`NetworkChannel.java:90`,
        // `:458`); a counter that refused to wrap would stop the channel at 2^31.
        self.next = self.next.wrapping_add(1);
        let mut fragments = Vec::with_capacity(pieces);
        for index in 0..pieces {
            let start = index * MAX_CHUNK;
            let end = (start + MAX_CHUNK).min(body.len());
            fragments.push(Fragment {
                communication,
                index: index as i32,
                terminator: index + 1 == pieces,
                inner: id,
                body: body[start.min(body.len())..end].to_vec(),
            });
        }
        Ok(fragments)
    }
}

/// One message being put back together.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Partial {
    id: MessageId,
    pieces: BTreeMap<i32, Vec<u8>>,
    /// The index the terminator named, once it has arrived. `None` is "no last
    /// fragment yet", which is Java's `terminator == false`.
    last: Option<i32>,
}

/// The receiving half: Java's `messageCache` plus the reassembly in
/// `SplitPacketMessage.onExecute`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Assembly {
    open: BTreeMap<i32, Partial>,
}

impl Assembly {
    pub fn new() -> Assembly {
        Assembly { open: BTreeMap::new() }
    }

    /// Take one fragment. `Ok(None)` is "still waiting", `Ok(Some(..))` is the whole
    /// message, and every other shape is a [`Refusal`].
    ///
    /// **The terminator is what completes a message, but it does not have to arrive
    /// last.** Java waits for the terminator and then concatenates what it has
    /// (`:95-107`), which means a terminator that arrives first completes a message out
    /// of the one fragment it brought. Here a terminator whose lower indices are missing
    /// answers [`Refusal::Gap`] *and keeps its state*: the missing fragment may still
    /// arrive, and the message then completes — out of order, whole, and only once.
    /// A message whose last fragment never comes stays open, so the caller decides when
    /// to give up ([`Assembly::abandon`]); Java lets a cache expiry decide.
    pub fn accept(&mut self, fragment: Fragment) -> Result<Option<Complete>, Refusal> {
        let communication = fragment.communication;
        let index = fragment.index;
        if index < 0 || index as usize >= MAX_FRAGMENTS {
            return Err(Refusal::BadFragment { communication, index });
        }
        if !self.open.contains_key(&communication) && self.open.len() >= MAX_OPEN {
            return Err(Refusal::TooMany { count: self.open.len() + 1, limit: MAX_OPEN });
        }
        let partial = self.open.entry(communication).or_insert_with(|| Partial {
            id: fragment.inner,
            pieces: BTreeMap::new(),
            last: None,
        });
        if partial.id != fragment.inner {
            // Two of one message's fragments name different messages: Java would use
            // whichever arrived with the terminator.
            return Err(Refusal::BadFragment { communication, index });
        }
        if fragment.terminator {
            if partial.last.is_some() {
                // A message has one last fragment; a second one is a sender that cut
                // its body twice.
                return Err(Refusal::BadFragment { communication, index });
            }
            partial.last = Some(index);
        }
        if partial.pieces.contains_key(&index) {
            return Err(Refusal::DuplicateFragment { communication, index });
        }
        partial.pieces.insert(index, fragment.body);

        let Some(last) = partial.last else {
            return Ok(None);
        };
        // A fragment past the one the terminator named would make the body longer than
        // the sender said it is.
        if let Some(beyond) = partial.pieces.keys().find(|key| **key > last).copied() {
            return Err(Refusal::BadFragment { communication, index: beyond });
        }
        for expected in 0..=last {
            if !partial.pieces.contains_key(&expected) {
                return Err(Refusal::Gap { communication, missing: expected });
            }
        }
        self.complete(communication)
    }

    /// The body, when every piece is there: Java's
    /// `sorted(Map.Entry.comparingByKey()).reduce(Bytes::concat)`
    /// (`SplitPacketMessage.java:103-107`), with the completeness check Java lacks.
    fn complete(&mut self, communication: i32) -> Result<Option<Complete>, Refusal> {
        let Some(done) = self.open.remove(&communication) else {
            return Ok(None);
        };
        let size: u64 = done.pieces.values().map(|piece| piece.len() as u64).sum();
        if size > MAX_PAYLOAD {
            return Err(Refusal::TooLarge { size, limit: MAX_PAYLOAD });
        }
        let mut body = Vec::with_capacity(size as usize);
        for piece in done.pieces.values() {
            body.extend_from_slice(piece);
        }
        Ok(Some(Complete { id: done.id, body }))
    }

    /// Take a whole message's fragments in the order they were cut.
    pub fn accept_all(
        &mut self,
        fragments: impl IntoIterator<Item = Fragment>,
    ) -> Result<Option<Complete>, Refusal> {
        let mut done = None;
        for fragment in fragments {
            if let Some(complete) = self.accept(fragment)? {
                done = Some(complete);
            }
        }
        Ok(done)
    }

    /// How many messages are half-received.
    pub fn open_count(&self) -> usize {
        self.open.len()
    }

    /// How many fragments of `communication` have arrived, terminator included.
    pub fn arrived(&self, communication: i32) -> usize {
        self.open.get(&communication).map(|partial| partial.pieces.len()).unwrap_or(0)
    }

    /// Whether the terminator of `communication` has been seen.
    pub fn terminated(&self, communication: i32) -> bool {
        self.open.get(&communication).map(|partial| partial.last.is_some()).unwrap_or(false)
    }

    /// Forget a message. Java calls this "one minute without access"
    /// (`NetworkChannel.java:82-85`); the port leaves the clock to the caller.
    pub fn abandon(&mut self, communication: i32) -> bool {
        self.open.remove(&communication).is_some()
    }
}

/// A message put back together: its id, and the bytes the sender serialized.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Complete {
    pub id: MessageId,
    pub body: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::route::{Channel, Envelope};
    use crate::network::items::{Hand, MarkStoryReadOnItem};

    /// `MarkStoryReadOnItemMessage`, so the id a fragment carries is a real one.
    const STORY: MessageId = MessageId::of(91);

    fn body(len: usize) -> Vec<u8> {
        (0..len).map(|index| (index % 251) as u8).collect()
    }

    #[test]
    fn a_body_that_fits_is_one_fragment_that_is_also_the_last() {
        let mut splitter = Splitter::new();
        let fragments = splitter.split(STORY, b"short").expect("one fragment");
        assert_eq!(fragments.len(), 1);
        assert_eq!(fragments[0].index, 0);
        assert!(fragments[0].terminator, "the only fragment ends the message");
        assert_eq!(fragments[0].inner, STORY);
        assert_eq!(fragments[0].body, b"short");
        assert_eq!(fragments[0].communication, 0);
    }

    #[test]
    fn a_body_of_two_chunks_and_a_byte_is_cut_into_three() {
        let mut splitter = Splitter::new();
        let body = body(2 * MAX_CHUNK + 1);
        let fragments = splitter.split(STORY, &body).expect("three fragments");
        assert_eq!(fragments.len(), 3);
        assert_eq!(fragments.iter().map(|f| f.index).collect::<Vec<_>>(), vec![0, 1, 2]);
        assert_eq!(fragments[0].body.len(), MAX_CHUNK);
        assert_eq!(fragments[1].body.len(), MAX_CHUNK);
        assert_eq!(fragments[2].body.len(), 1);
        assert_eq!(
            fragments.iter().filter(|f| f.terminator).count(),
            1,
            "exactly one terminator, and it is the last"
        );
        assert!(fragments[2].terminator);
        assert_eq!(fragments.iter().filter(|f| f.communication == fragments[0].communication).count(), 3);
        let joined: Vec<u8> = fragments.iter().flat_map(|f| f.body.clone()).collect();
        assert_eq!(joined, body, "the pieces are the body, in order");
    }

    #[test]
    fn an_empty_body_still_becomes_one_fragment() {
        // Java's `while (currentIndex < data.length)` (`NetworkChannel.java:461`) runs
        // zero times here and the message is silently never sent.
        let mut splitter = Splitter::new();
        let fragments = splitter.split(STORY, &[]).expect("one fragment");
        assert_eq!(fragments.len(), 1);
        assert!(fragments[0].terminator);
        assert!(fragments[0].body.is_empty());
        let mut assembly = Assembly::new();
        let done = assembly.accept_all(fragments).expect("no refusal").expect("complete");
        assert_eq!(done, Complete { id: STORY, body: Vec::new() });
    }

    #[test]
    fn each_message_spends_one_communication_id_and_the_counter_wraps() {
        let mut splitter = Splitter::new();
        let first = splitter.split(STORY, b"a").expect("one");
        let second = splitter.split(STORY, b"b").expect("one");
        assert_eq!(first[0].communication, 0);
        assert_eq!(second[0].communication, 1);
        assert_eq!(splitter.peek(), 2);

        let mut splitter = Splitter { next: i32::MAX };
        let last = splitter.split(STORY, b"a").expect("one");
        assert_eq!(last[0].communication, i32::MAX);
        assert_eq!(splitter.peek(), i32::MIN, "an AtomicInteger wraps rather than stopping");
    }

    #[test]
    fn a_body_past_the_fragment_bound_is_refused() {
        let mut splitter = Splitter::new();
        // 60 MB of zeroes: one byte past the last fragment the port will cut.
        let body = vec![0u8; MAX_FRAGMENTS * MAX_CHUNK + 1];
        assert_eq!(
            splitter.split(STORY, &body).err(),
            Some(Refusal::TooMany { count: MAX_FRAGMENTS + 1, limit: MAX_FRAGMENTS })
        );
        assert_eq!(splitter.peek(), 0, "a refused split spends no communication id");
    }

    #[test]
    fn a_fragment_round_trips_through_the_five_keys_java_writes() {
        let fragment = Fragment {
            communication: 4,
            index: 2,
            terminator: true,
            inner: STORY,
            body: vec![1, 2, 3],
        };
        let written = fragment.to_compound();
        for key in [KEY_COMMUNICATION, KEY_INDEX, KEY_TERMINATOR, KEY_INNER, KEY_PAYLOAD] {
            assert!(written.get(&key.into()).is_some(), "{key} is not written");
        }
        assert_eq!(Fragment::from_compound(&written), Ok(fragment));
    }

    #[test]
    fn a_fragment_a_sender_cannot_legally_produce_is_refused() {
        let good = Fragment {
            communication: 1,
            index: 0,
            terminator: true,
            inner: STORY,
            body: vec![],
        };
        for (communication, index) in [(-1, 0), (1, -1), (1, MAX_FRAGMENTS as i32)] {
            let mut data = good.to_compound();
            data.insert(KEY_COMMUNICATION.into(), common::int(communication));
            data.insert(KEY_INDEX.into(), common::int(index));
            assert_eq!(
                Fragment::from_compound(&data).err(),
                Some(Refusal::BadFragment { communication, index }),
                "Java's -1 sentinel is not a wire value"
            );
        }
        let mut data = good.to_compound();
        data.insert(KEY_INNER.into(), common::int(70_000));
        assert!(Fragment::from_compound(&data).is_err(), "an id past u16 is not an id");
        let mut data = good.to_compound();
        data.remove(&KEY_PAYLOAD.into());
        assert_eq!(
            Fragment::from_compound(&data).err(),
            Some(Refusal::Field(common::Refusal::Missing { key: KEY_PAYLOAD }))
        );
    }

    #[test]
    fn fragments_in_order_rebuild_the_body() {
        // Two chunks, and the whole still under the engine's payload bound: 1,048,575
        // bytes is the largest body that can be both.
        let body = body(MAX_PAYLOAD as usize - 1);
        let mut splitter = Splitter::new();
        let fragments = splitter.split(STORY, &body).expect("two");
        assert_eq!(fragments.len(), 2);
        let mut assembly = Assembly::new();
        let done = assembly.accept_all(fragments).expect("no refusal").expect("complete");
        assert_eq!(done.id, STORY);
        assert_eq!(done.body, body);
        assert_eq!(assembly.open_count(), 0, "a completed message is closed");
    }

    #[test]
    fn fragments_out_of_order_rebuild_the_same_body() {
        let body = body(2 * 1000);
        let fragments = vec![
            Fragment { communication: 5, index: 0, terminator: false, inner: STORY, body: body[..1000].to_vec() },
            Fragment { communication: 5, index: 1, terminator: true, inner: STORY, body: body[1000..].to_vec() },
        ];
        let mut assembly = Assembly::new();
        // The last fragment first: the gap is named, then the first one closes it.
        assert!(assembly.accept(fragments[1].clone()).is_err());
        let done = assembly.accept(fragments[0].clone()).expect("no refusal").expect("complete");
        assert_eq!(done, Complete { id: STORY, body });
    }

    #[test]
    fn a_terminator_that_arrives_first_is_a_gap_and_the_late_piece_still_completes_it() {
        // Java concatenates what arrived (`SplitPacketMessage.java:103-107`), so a
        // terminator arriving alone would hand over a third of the body as if it were
        // all of it. Here the gap is named, the state is kept, and the late piece still
        // completes the message — once.
        let piece = body(1000);
        let mut assembly = Assembly::new();
        let last = Fragment {
            communication: 7,
            index: 2,
            terminator: true,
            inner: STORY,
            body: piece.clone(),
        };
        assert_eq!(
            assembly.accept(last).err(),
            Some(Refusal::Gap { communication: 7, missing: 0 })
        );
        assert_eq!(assembly.open_count(), 1, "the message is kept, not dropped");
        assert!(assembly.terminated(7), "the terminator was seen");
        assert_eq!(assembly.arrived(7), 1);

        let middle = Fragment { index: 1, terminator: false, ..base(7) };
        assert_eq!(assembly.accept(middle).err(), Some(Refusal::Gap { communication: 7, missing: 0 }));
        let first = Fragment { index: 0, terminator: false, ..base(7) };
        let done = assembly.accept(first).expect("no refusal").expect("complete");
        assert_eq!(done.body.len(), 3000);
        assert_eq!(assembly.open_count(), 0, "completed and closed");
    }

    /// One fragment of communication `communication`, index 0, not the last.
    fn base(communication: i32) -> Fragment {
        Fragment { communication, index: 0, terminator: false, inner: STORY, body: body(1000) }
    }

    #[test]
    fn a_piece_past_the_terminator_is_refused() {
        let mut assembly = Assembly::new();
        assert_eq!(
            assembly
                .accept(Fragment { communication: 2, index: 1, terminator: true, inner: STORY, body: vec![1] })
                .err(),
            Some(Refusal::Gap { communication: 2, missing: 0 }),
            "the body ends at 1 and 0 never came"
        );
        assert_eq!(
            assembly
                .accept(Fragment { communication: 2, index: 5, terminator: false, inner: STORY, body: vec![2] })
                .err(),
            Some(Refusal::BadFragment { communication: 2, index: 5 }),
            "the sender said the body ends at 1"
        );
        assert_eq!(
            assembly
                .accept(Fragment { communication: 2, index: 3, terminator: true, inner: STORY, body: vec![3] })
                .err(),
            Some(Refusal::BadFragment { communication: 2, index: 3 }),
            "a second terminator"
        );
    }

    #[test]
    fn a_repeated_index_is_refused_rather_than_overwritten() {
        let mut splitter = Splitter::new();
        let fragments = splitter.split(STORY, &body(MAX_CHUNK + 10)).expect("two");
        let mut assembly = Assembly::new();
        assert_eq!(assembly.accept(fragments[0].clone()), Ok(None));
        assert_eq!(
            assembly.accept(fragments[0].clone()).err(),
            Some(Refusal::DuplicateFragment { communication: 0, index: 0 })
        );
        assert_eq!(assembly.arrived(0), 1);
    }

    #[test]
    fn two_fragments_naming_different_messages_are_refused() {
        let mut splitter = Splitter::new();
        let mut fragments = splitter.split(STORY, &body(MAX_CHUNK + 1)).expect("two");
        fragments[1].inner = MessageId::of(138);
        let mut assembly = Assembly::new();
        assembly.accept(fragments[0].clone()).expect("the first");
        assert_eq!(
            assembly.accept(fragments[1].clone()).err(),
            Some(Refusal::BadFragment { communication: 0, index: 1 })
        );
    }

    #[test]
    fn open_assemblies_are_bounded_and_abandon_frees_one() {
        let mut assembly = Assembly::new();
        for communication in 0..MAX_OPEN as i32 {
            let fragment = Fragment {
                communication,
                index: 0,
                terminator: false,
                inner: STORY,
                body: vec![1],
            };
            assert_eq!(assembly.accept(fragment), Ok(None));
        }
        assert_eq!(assembly.open_count(), MAX_OPEN);
        let one_too_many = Fragment {
            communication: MAX_OPEN as i32,
            index: 0,
            terminator: false,
            inner: STORY,
            body: vec![1],
        };
        assert_eq!(
            assembly.accept(one_too_many).err(),
            Some(Refusal::TooMany { count: MAX_OPEN + 1, limit: MAX_OPEN })
        );
        assert!(assembly.abandon(0));
        assert!(!assembly.abandon(0), "forgetting twice is not an error, it is a false");
        assert_eq!(assembly.open_count(), MAX_OPEN - 1);
    }

    #[test]
    fn a_reassembled_body_past_the_payload_bound_is_refused() {
        let mut splitter = Splitter::new();
        let fragments = splitter.split(STORY, &body(2 * MAX_CHUNK)).expect("two");
        assert!((2 * MAX_CHUNK as u64) > MAX_PAYLOAD, "the bound is what makes this a refusal");
        let mut assembly = Assembly::new();
        assert_eq!(
            assembly.accept_all(fragments).err(),
            Some(Refusal::TooLarge { size: 2 * MAX_CHUNK as u64, limit: MAX_PAYLOAD })
        );
    }

    #[test]
    fn the_id_the_fragments_carried_is_the_id_the_router_routes() {
        let mut splitter = Splitter::new();
        let fragments = splitter
            .split(STORY, &body(MAX_CHUNK + 1))
            .expect("two fragments, one over the chunk size");
        assert_eq!(fragments.len(), 2);
        let mut assembly = Assembly::new();
        let done = assembly.accept_all(fragments).expect("no refusal").expect("complete");
        assert_eq!(done.body.len(), MAX_CHUNK + 1);

        let message = MarkStoryReadOnItem { hand: Hand::Off };
        let routed = Channel::new()
            .receive(Envelope::of(done.id, &message), mod_api::LogicalSide::Client)
            .expect("the id the last fragment carried is a server-bound message");
        assert_eq!(routed.decode::<MarkStoryReadOnItem>(), Ok(message));
    }
}
