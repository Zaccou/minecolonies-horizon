//! `ClientChunkUpdatedEvent.java`: an event, and the queue that defers it.
//!
//! Java reference: `ClientChunkUpdatedEvent.java`, 35 lines — **an event class and a
//! getter**, no listener at all. It is in the brief because it is the mod's own event
//! and the port has to decide whether to keep it; the answer is yes, as a *value*,
//! because what it announces is already in this port (`crate::network::common::ChunkCap`,
//! the four synced fields of `ChunkCapData`).
//!
//! **Two rules the class states and a port can get wrong.**
//!
//! 1. **It fires even for a chunk no colony owns, and even when nothing changed**
//!    (`:9-11`): "as that may also be interesting; and even if the data did not
//!    actually change, as it might still be new to the listener". So there is no
//!    dirty check to write — and a port that added one would silently stop telling a
//!    listener about a chunk it has never seen.
//! 2. **It fires from `applyCap`, which also writes the capability**
//!    (`ChunkClientDataHelper.java:67-75`), so a listener always reads the new value,
//!    never the old one. Java's `chunk.getCapability(CLOSE_COLONY_CAP, null).orElseGet(null)`
//!    throws when the chunk has no capability — `orElseGet` wants a supplier and is
//!    handed `null` — and the event is posted **outside** the null check anyway, so a
//!    chunk without the attachment still notifies.
//!
//! **Why the queue is here.** `ChunkClientDataHelper` keeps a static `chunkCapsToAdd`
//! list because a capability packet can arrive **before** the chunk it names: the
//! message is sent from `EventHandler.onEnteringChunk` (`:383-384`), and
//! `ChunkEvent.Load` (`:272-278`) then drains the entries whose `x`/`z` match. That
//! ordering is the only stateful thing in the file, and [`Late`] is it.

use crate::network::common::ChunkCap;

/// One notification: the chunk, and the claim record it now carries.
///
/// The capability's own value is `crate::network::common::ChunkCap`; a chunk with no
/// claim has `owning_colony == 0` and an empty `claiming_colonies`, which is the
/// record `ChunkCapData(x, z)` builds (`ChunkCapData.java:33-39`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Updated {
    /// The chunk that changed, in chunk coordinates.
    pub at: [i32; 2],
    /// The record it now holds.
    pub cap: ChunkCap,
}

/// `ChunkClientDataHelper.chunkCapsToAdd`, as a value.
///
/// `Late::push` is the packet handler's half (`UpdateChunkCapabilityMessage` →
/// `applyCap` or, when the chunk is not there yet, the list), and `Late::apply` is
/// `applyLate` (`ChunkClientDataHelper.java:41-59`). The drain is by position and
/// removes **every** entry that matches, which is what the iterator-with-remove loop
/// does; two packets for one chunk are both applied, in arrival order.
#[derive(Clone, Debug, Default)]
pub struct Late {
    pending: Vec<ChunkCap>,
}

impl Late {
    /// A queue with nothing waiting.
    pub fn new() -> Late {
        Late::default()
    }

    /// How many records are waiting for their chunk.
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// Whether nothing is waiting.
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// A record whose chunk is not in the client level yet.
    pub fn push(&mut self, cap: ChunkCap) {
        self.pending.push(cap);
    }

    /// `applyLate`: the records of this chunk, in arrival order, which leave the
    /// queue.
    ///
    /// `ChunkClientDataHelper.java:44-46` starts with `if (chunkCapsToAdd.isEmpty())
    /// return;`, and the port's empty answer says the same without the branch.
    pub fn apply(&mut self, at: [i32; 2]) -> Vec<ChunkCap> {
        let mut applied = Vec::new();
        let mut waiting = Vec::with_capacity(self.pending.len());
        for cap in self.pending.drain(..) {
            if [cap.x, cap.z] == at {
                applied.push(cap);
            } else {
                waiting.push(cap);
            }
        }
        self.pending = waiting;
        applied
    }
}

/// What a chunk-load does with one applied record: the capability write, then the
/// notification.
///
/// Java's `applyCap` (`:67-75`) writes `setOwningColony` and `setStaticColonyClaim`
/// and then posts the event **unconditionally**. [`apply`] returns the notification
/// so a caller cannot forget the second half: the rule "even when nothing changed" is
/// in the return type, not in a comment.
pub fn apply(cap: ChunkCap) -> Updated {
    Updated {
        at: [cap.x, cap.z],
        cap,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cap(x: i32, z: i32, owning: i32, claiming: &[i32]) -> ChunkCap {
        ChunkCap {
            x,
            z,
            owning_colony: owning,
            claiming_colonies: claiming.to_vec(),
        }
    }

    #[test]
    fn a_record_waiting_for_its_chunk_is_applied_when_the_chunk_arrives() {
        let mut queue = Late::new();
        queue.push(cap(3, 4, 7, &[7]));
        assert_eq!(queue.len(), 1);
        let applied = queue.apply([3, 4]);
        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0].owning_colony, 7);
        assert!(queue.is_empty(), "the record left the queue");
    }

    #[test]
    fn a_record_of_another_chunk_stays() {
        let mut queue = Late::new();
        queue.push(cap(3, 4, 7, &[7]));
        assert!(queue.apply([3, 5]).is_empty());
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn two_records_for_one_chunk_are_both_applied_in_arrival_order() {
        let mut queue = Late::new();
        queue.push(cap(1, 1, 2, &[2]));
        queue.push(cap(1, 1, 3, &[3]));
        let applied = queue.apply([1, 1]);
        assert_eq!(applied.len(), 2);
        assert_eq!(applied[0].owning_colony, 2);
        assert_eq!(applied[1].owning_colony, 3);
    }

    #[test]
    fn a_chunk_without_a_capability_still_notifies() {
        // `ChunkCapData(x, z)`'s own record: no owner, no claim (`:33-39`).
        let updated = apply(cap(0, 0, 0, &[]));
        assert_eq!(updated.at, [0, 0]);
        assert_eq!(updated.cap.owning_colony, crate::game_events::event_handler::NO_COLONY_ID);
        assert!(updated.cap.claiming_colonies.is_empty());
    }

    #[test]
    fn the_same_record_applied_twice_notifies_twice() {
        // `:9-11`: "even if the data did not actually change".
        let record = cap(2, 2, 5, &[5]);
        let first = apply(record.clone());
        let second = apply(record);
        assert_eq!(first, second);
    }

    #[test]
    fn an_empty_queue_applies_nothing() {
        let mut queue = Late::new();
        assert!(queue.apply([0, 0]).is_empty());
        assert!(queue.is_empty());
    }
}
