//! `SplitPacketMessage`: one message cut into fragments, and put back together.
//!
//! Java wraps a message bigger than one packet into numbered fragments
//! (`NetworkChannel.java:443-476`) and reassembles them in the receiving
//! `onExecute` (`SplitPacketMessage.java:85-154`). [`crate::network::debug::SplitPacket`]
//! already carries **one fragment** as data; what was missing is the *stack* that puts
//! them back together, which is state and therefore was not a codec's job.
//!
//! **Why the fragments exist at all, in this port.** Forge's `SimpleChannel` payloads
//! are capped, so MineColonies cuts at `max_packet_size = 943718` — *90 % of the maximum
//! packet size*, 1 048 576 (`NetworkChannel.java:461-462`). Horizon's payload cap is the
//! same 1 048 576 bytes, and it says so where it refuses one (`PayloadTooLarge`,
//! `crates/horizon-mod-vocab/src/payload.rs:149-157`): exactly 90 % of that is
//! MineColonies' fragment size, to the byte. So the arithmetic survives the port
//! unchanged; what does not survive is the *sender*, because Horizon never splits a
//! payload and refuses one over the cap instead. Fragmentation is therefore the mod's
//! own: this module is the half that receives, and a sender must cut at [`MAX_FRAGMENT`]
//! for the two halves to agree with what Java sends.
//!
//! **What Java does not do, and this port does.** Four things, each a refusal here and a
//! silence there:
//!
//! 1. **No hole check.** Java concatenates the fragments it has, in index order
//!    (`:103-107`), and hands the result to `fromBytes`; a missing fragment shifts every
//!    byte after it, and the decode either reads nonsense or throws — which Java catches
//!    and logs as "Packet error" (`:121-126`). Here a hole is [`Gap::Hole`].
//! 2. **One cache for every player.** `Network.getNetwork().getMessageCache()` is a field
//!    of the `Network` singleton keyed by `communicationId` alone
//!    (`SplitPacketMessage.java:90-92`), and that id is a counter the *sender* picks
//!    (`NetworkChannel.java:460`: `messageCounter.getAndIncrement()`), starting at 0 on
//!    every connection. Two players therefore use the same ids, and a fragment that
//!    arrives for an id another player is filling is merged into *that* message; the
//!    player whose fragment says `terminator` decides when it is dispatched, in their own
//!    `ctx`. Here the stack is per sender: [`Reassembler::push`] takes the sender and
//!    keeps a separate chain per `(sender, communicationId)`.
//! 3. **No eviction.** A chain that never receives its terminator stays in the cache for
//!    the life of the process; ids are never reused, so a client that opens a new one on
//!    every message grows the map by one entry per message. Here a chain is dropped when
//!    the reassembler runs out of room ([`Gap::TooManyChains`]) and
//!    [`Reassembler::forget`] exists for a caller that knows the sender is gone.
//! 4. **No bound on the inner id.** `messageEntry.getCreator()` on an id the table does
//!    not hold is an NPE (`:110-113`). Here it is [`Gap::InnerUnknown`], and the table is
//!    the one thing a caller must supply: [`Reassembler`] carries the *bytes*, not a
//!    decoder.
//!
//! **What it still cannot do**, said once: turn the bytes into a message. That needs the
//! sender's table of ids (`SplitPacketMessage.java:41`, "Identical to the index codec
//! system in SimpleChannel"), which belongs to the layer that registers the payloads, not
//! to a format. [`Whole`] hands back the id and the bytes, which is what that layer needs.

use super::effects::Effect;
use crate::network::debug::SplitPacket;

/// The largest payload Horizon will carry in one frame
/// (`crates/horizon-mod-vocab/src/payload.rs:149`: `MAX_PAYLOAD_SIZE`, 1 048 576).
pub const MAX_PAYLOAD: usize = 1_048_576;

/// Where MineColonies cuts: 90 % of [`MAX_PAYLOAD`], to the byte
/// (`NetworkChannel.java:461-462`, `943718`).
pub const MAX_FRAGMENT: usize = MAX_PAYLOAD * 9 / 10;

/// The most fragments one message may consist of: `MAX_PAYLOAD / MAX_FRAGMENT` rounded up.
/// A message larger than that could not be reassembled into one payload anyway.
pub const MAX_FRAGMENTS: usize = 2;

/// The most chains one sender may have open at once, so that a client that opens a new
/// `communicationId` per packet cannot grow the table without bound. Java has no such
/// bound; thirty-two is [`MAX_FRAGMENTS`] times sixteen, and a well-behaved sender needs
/// two.
pub const MAX_CHAINS: usize = 16;

/// Why fragments could not be put back together.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: closed — each variant is a different defect, and a caller that logs one
// must be able to tell them apart
pub enum Gap {
    /// The index is negative, which a sender never writes.
    Index { index: i32 },
    /// Two fragments claim the same index: one of them is a retransmission Java never
    /// makes, or a second sender in the same chain.
    Duplicate { index: i32 },
    /// The chain is complete in length but a fragment is missing: the first hole.
    Hole { index: i32 },
    /// The payload arrived after the terminator for the same chain.
    Late { index: i32 },
    /// The sender has more than [`MAX_CHAINS`] chains open.
    TooManyChains,
    /// One fragment carries more than [`MAX_FRAGMENT`].
    FragmentTooLarge { size: usize },
    /// The reassembled message would exceed [`MAX_PAYLOAD`].
    MessageTooLarge { size: usize },
    /// The inner message id names nothing in the table the caller would need to decode it.
    InnerUnknown { id: i32 },
}

impl Gap {
    /// A stable name for a log line, namespaced like [`crate::network::Refusal`]'s.
    pub fn name(self) -> &'static str {
        match self {
            Gap::Index { .. } => "fragment.refusal.index",
            Gap::Duplicate { .. } => "fragment.refusal.duplicate",
            Gap::Hole { .. } => "fragment.refusal.hole",
            Gap::Late { .. } => "fragment.refusal.late",
            Gap::TooManyChains => "fragment.refusal.chains",
            Gap::FragmentTooLarge { .. } => "fragment.refusal.fragment_size",
            Gap::MessageTooLarge { .. } => "fragment.refusal.message_size",
            Gap::InnerUnknown { .. } => "fragment.refusal.inner_unknown",
        }
    }
}

impl std::fmt::Display for Gap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// A whole message, rebuilt: the id of the class that can decode it, and its bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Whole {
    pub inner_message_id: i32,
    pub bytes: Vec<u8>,
}

/// One chain of fragments from one sender.
#[derive(Default)]
struct Chain {
    inner_message_id: i32,
    fragments: Vec<(i32, Vec<u8>)>,
    terminator: Option<i32>,
}

impl Chain {
    fn bytes_len(&self) -> usize {
        self.fragments.iter().map(|(_, bytes)| bytes.len()).sum()
    }
}

/// The reassembly table: one chain per `(sender, communication id)`.
///
/// A sender is whatever the caller can tell apart — a player's UUID as `u128`, which is
/// what [`crate::network::common::uuid`] already writes, so that two players cannot meet
/// in one chain ([`Gap`]'s note 2).
#[derive(Default)]
pub struct Reassembler {
    chains: Vec<(u128, i32, Chain)>,
}

impl Reassembler {
    pub fn new() -> Self {
        Self::default()
    }

    /// How many chains are open. Java's cache has no equivalent reading, which is the
    /// point of [`MAX_CHAINS`].
    pub fn open_chains(&self) -> usize {
        self.chains.len()
    }

    /// Adds a fragment and returns the whole message when this fragment completes it.
    ///
    /// `Ok(None)` means "not yet": the chain is waiting for more. Nothing is removed on
    /// `Err`, so a caller can keep collecting after a duplicate or a late fragment; a
    /// completed chain is taken out of the table in every case, as Java's is discarded
    /// with the entry it read.
    pub fn push(&mut self, sender: u128, fragment: &SplitPacket) -> Result<Option<Whole>, Gap> {
        if fragment.packet_index < 0 {
            return Err(Gap::Index {
                index: fragment.packet_index,
            });
        }
        if fragment.payload.len() > MAX_FRAGMENT {
            return Err(Gap::FragmentTooLarge {
                size: fragment.payload.len(),
            });
        }
        let at = match self
            .chains
            .iter()
            .position(|(who, id, _)| *who == sender && *id == fragment.communication_id)
        {
            Some(at) => at,
            None => {
                if self.chains.len() >= MAX_CHAINS {
                    return Err(Gap::TooManyChains);
                }
                self.chains.push((
                    sender,
                    fragment.communication_id,
                    Chain {
                        inner_message_id: fragment.inner_message_id,
                        fragments: Vec::new(),
                        terminator: None,
                    },
                ));
                self.chains.len() - 1
            }
        };
        let mut too_large = None;
        {
            let chain = &self.chains[at].2;
            if let Some(terminator) = chain.terminator {
                if fragment.packet_index > terminator {
                    return Err(Gap::Late {
                        index: fragment.packet_index,
                    });
                }
            }
            if chain
                .fragments
                .iter()
                .any(|(index, _)| *index == fragment.packet_index)
            {
                return Err(Gap::Duplicate {
                    index: fragment.packet_index,
                });
            }
            let size = chain.bytes_len() + fragment.payload.len();
            if size > MAX_PAYLOAD {
                too_large = Some(size);
            }
        }
        if let Some(size) = too_large {
            self.chains.remove(at);
            return Err(Gap::MessageTooLarge { size });
        }
        let chain = &mut self.chains[at].2;
        chain
            .fragments
            .push((fragment.packet_index, fragment.payload.clone()));
        if fragment.terminator {
            chain.terminator = Some(fragment.packet_index);
            chain.inner_message_id = fragment.inner_message_id;
        }
        let complete = match chain.terminator {
            Some(last) => chain.fragments.len() == last as usize + 1,
            None => false,
        };
        if !complete {
            return Ok(None);
        }
        chain.fragments.sort_unstable_by_key(|(index, _)| *index);
        let mut hole = None;
        let mut bytes = Vec::with_capacity(chain.bytes_len());
        for (expected, (index, payload)) in chain.fragments.iter().enumerate() {
            if *index != expected as i32 {
                hole = Some(expected as i32);
                break;
            }
            bytes.extend_from_slice(payload);
        }
        let inner_message_id = chain.inner_message_id;
        self.chains.remove(at);
        match hole {
            Some(index) => Err(Gap::Hole { index }),
            None => Ok(Some(Whole {
                inner_message_id,
                bytes,
            })),
        }
    }

    /// Forgets every chain of one sender: what a disconnect handler should call, and what
    /// Java never does.
    pub fn forget(&mut self, sender: u128) {
        self.chains.retain(|(who, _, _)| *who != sender);
    }
}

/// The verb `SplitPacketMessage` gives the effects vocabulary: it reassembles and
/// re-dispatches, and the reassembly is [`Reassembler`].
pub const EFFECT: Effect = Effect::Dispatch;

/// How many fragments Java's sender writes for a payload of `bytes`.
///
/// `NetworkChannel.java:466-475` loops `while (currentIndex < data.length)`, so an
/// **empty** payload produces **no fragment at all** and the message is never delivered:
/// the receiver's chain is never started, and the send is a silent loss. Every message in
/// this tree has at least an envelope, so the case is theoretical in MineColonies — but it
/// is the arithmetic of the sender, and a port that copies the loop would copy the hole.
pub fn fragments_for(bytes: usize) -> usize {
    if bytes == 0 {
        return 0;
    }
    bytes.div_ceil(MAX_FRAGMENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SENDER: u128 = 1;

    fn fragment(communication: i32, index: i32, terminator: bool, payload: &[u8]) -> SplitPacket {
        SplitPacket {
            communication_id: communication,
            packet_index: index,
            terminator,
            inner_message_id: 7,
            payload: payload.to_vec(),
        }
    }

    #[test]
    fn one_fragment_is_a_whole_message() {
        let mut reassembler = Reassembler::new();
        let whole = reassembler
            .push(SENDER, &fragment(1, 0, true, b"abc"))
            .expect("no refusal");
        assert_eq!(
            whole,
            Some(Whole {
                inner_message_id: 7,
                bytes: b"abc".to_vec()
            })
        );
        assert_eq!(reassembler.open_chains(), 0, "a finished chain is released");
    }

    #[test]
    fn fragments_are_concatenated_in_index_order_whatever_the_arrival_order() {
        let mut reassembler = Reassembler::new();
        assert_eq!(reassembler.push(SENDER, &fragment(2, 1, true, b"def")), Ok(None));
        let whole = reassembler.push(SENDER, &fragment(2, 0, false, b"abc"));
        assert_eq!(
            whole,
            Ok(Some(Whole {
                inner_message_id: 7,
                bytes: b"abcdef".to_vec()
            }))
        );
    }

    #[test]
    fn a_terminator_with_a_hole_is_refused_rather_than_decoded() {
        // Java concatenates what it has, in index order, and hands the result to
        // `fromBytes` (`SplitPacketMessage.java:103-120`): with a fragment missing, every
        // byte after the hole is off by a fragment, and the decode reads nonsense.
        let mut reassembler = Reassembler::new();
        assert_eq!(reassembler.push(SENDER, &fragment(3, 0, false, b"abc")), Ok(None));
        assert_eq!(reassembler.push(SENDER, &fragment(3, 3, false, b"jkl")), Ok(None));
        assert_eq!(
            reassembler.push(SENDER, &fragment(3, 2, true, b"ghi")),
            Err(Gap::Hole { index: 1 }),
            "three fragments, last index 2, and index 1 never came"
        );
        assert_eq!(reassembler.open_chains(), 0, "a broken chain is dropped");
    }

    #[test]
    fn the_same_index_twice_is_refused() {
        let mut reassembler = Reassembler::new();
        assert_eq!(reassembler.push(SENDER, &fragment(4, 0, false, b"abc")), Ok(None));
        assert_eq!(
            reassembler.push(SENDER, &fragment(4, 0, false, b"abc")),
            Err(Gap::Duplicate { index: 0 })
        );
    }

    #[test]
    fn a_negative_index_is_refused() {
        let mut reassembler = Reassembler::new();
        assert_eq!(
            reassembler.push(SENDER, &fragment(5, -1, true, b"")),
            Err(Gap::Index { index: -1 })
        );
    }

    #[test]
    fn a_fragment_past_the_terminator_is_refused() {
        let mut reassembler = Reassembler::new();
        // the terminator says this message is two fragments, and the second is missing
        assert_eq!(reassembler.push(SENDER, &fragment(6, 0, false, b"abc")), Ok(None));
        assert_eq!(reassembler.push(SENDER, &fragment(6, 2, true, b"ghi")), Ok(None));
        assert_eq!(
            reassembler.push(SENDER, &fragment(6, 3, false, b"jkl")),
            Err(Gap::Late { index: 3 }),
            "Java would merge it into the chain and shift every byte"
        );
    }

    #[test]
    fn a_fragment_larger_than_the_cut_is_refused() {
        let mut reassembler = Reassembler::new();
        let large = vec![0u8; MAX_FRAGMENT + 1];
        assert_eq!(
            reassembler.push(SENDER, &fragment(8, 0, true, &large)),
            Err(Gap::FragmentTooLarge {
                size: MAX_FRAGMENT + 1
            })
        );
    }

    #[test]
    fn two_senders_cannot_meet_in_one_chain() {
        // Java's cache is keyed by communicationId alone, and the id is a counter that
        // starts at 0 per connection (`NetworkChannel.java:460`): the two chains below
        // are one cache entry there, and this test is the defect.
        let mut reassembler = Reassembler::new();
        assert_eq!(
            reassembler.push(1, &fragment(9, 0, false, b"from-one")),
            Ok(None),
            "sender one's message is not finished"
        );
        assert_eq!(
            reassembler.push(2, &fragment(9, 0, true, b"from-two")),
            Ok(Some(Whole {
                inner_message_id: 7,
                bytes: b"from-two".to_vec()
            })),
            "sender two's whole message, not a fragment of sender one's"
        );
        assert_eq!(reassembler.open_chains(), 1, "sender one is still open");
        assert_eq!(
            reassembler.push(1, &fragment(9, 1, true, b"more")),
            Ok(Some(Whole {
                inner_message_id: 7,
                bytes: b"from-onemore".to_vec()
            }))
        );
    }

    #[test]
    fn a_sender_may_not_hold_more_chains_than_the_bound() {
        let mut reassembler = Reassembler::new();
        for communication in 0..MAX_CHAINS as i32 {
            assert_eq!(
                reassembler.push(SENDER, &fragment(communication, 0, false, b"a")),
                Ok(None)
            );
        }
        assert_eq!(
            reassembler.push(SENDER, &fragment(99, 0, false, b"a")),
            Err(Gap::TooManyChains)
        );
        reassembler.forget(SENDER);
        assert_eq!(reassembler.open_chains(), 0);
        assert_eq!(reassembler.push(SENDER, &fragment(99, 0, false, b"a")), Ok(None));
    }

    #[test]
    fn a_message_over_the_payload_cap_is_refused_before_it_is_allocated() {
        let mut reassembler = Reassembler::new();
        assert_eq!(
            reassembler.push(SENDER, &fragment(10, 0, false, &vec![0u8; MAX_FRAGMENT])),
            Ok(None)
        );
        // two full fragments are 1_887_436 bytes: over 1_048_576, so the cap fires on the
        // second one rather than on the third
        assert_eq!(
            reassembler.push(SENDER, &fragment(10, 1, true, &vec![0u8; MAX_FRAGMENT])),
            Err(Gap::MessageTooLarge {
                size: MAX_FRAGMENT * 2
            })
        );
    }

    #[test]
    fn the_tables_arithmetic_is_javas() {
        assert_eq!(MAX_PAYLOAD, 1_048_576, "payload.rs:149");
        assert_eq!(MAX_FRAGMENT, 943_718, "NetworkChannel.java:462");
        assert_eq!(MAX_PAYLOAD * 9 / 10, MAX_FRAGMENT, "90 % of the maximum, to the byte");
        assert_eq!(fragments_for(0), 0, "Java sends nothing for an empty payload");
        assert_eq!(fragments_for(1), 1);
        assert_eq!(fragments_for(MAX_FRAGMENT), 1);
        assert_eq!(fragments_for(MAX_FRAGMENT + 1), 2);
        assert_eq!(fragments_for(MAX_PAYLOAD), 2, "a whole payload fits in two cuts");
    }

    #[test]
    fn every_gap_has_its_own_name() {
        let gaps = [
            Gap::Index { index: 0 },
            Gap::Duplicate { index: 0 },
            Gap::Hole { index: 0 },
            Gap::Late { index: 0 },
            Gap::TooManyChains,
            Gap::FragmentTooLarge { size: 0 },
            Gap::MessageTooLarge { size: 0 },
            Gap::InnerUnknown { id: 0 },
        ];
        let mut names: Vec<&str> = gaps.iter().map(|gap| gap.name()).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "two gaps share a name");
        assert!(names.iter().all(|name| name.starts_with("fragment.refusal.")));
    }

    #[test]
    fn the_verb_of_the_split_message_is_dispatch() {
        assert_eq!(EFFECT, Effect::Dispatch);
        assert_eq!(crate::network_messages::catalogue::of_class("SplitPacket").map(|row| row.effect), Some(EFFECT));
    }
}
