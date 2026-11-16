//! Eight messages that show something or ship something whole: the pathfinding
//! debug overlay, the two chunk-claim updates, the compatibility manager, the quest
//! pack, the disease pack, and the fragment carrier underneath all of them.
//!
//! Java reference: `core/network/messages/client/SyncPathMessage.java` (166),
//! `SyncPathReachedMessage.java` (74), `UpdateChunkCapabilityMessage.java` (92),
//! `UpdateChunkRangeCapabilityMessage.java` (116),
//! `UpdateClientWithCompatibilityMessage.java` (78), `GlobalQuestSyncMessage.java` (74),
//! `client/colony/GlobalDiseaseSyncMessage.java` (73),
//! `splitting/SplitPacketMessage.java` (156).
//!
//! **Six of the eight carry a payload that is somebody else's encoding.** The
//! compatibility manager (`:40`, a registry of every recipe and every item the colony
//! knows), the quest pack (`:38`, a json listener), the disease pack (`:37`) and the
//! `SplitPacketMessage` fragments (`:71`) are all opaque here — bytes, with
//! `MANQUANT.md` §1 naming the writer of each.
//!
//! **The two path messages are the only ones the port can read whole.**
//! [`SyncPath`] is six lists of `MNode` — position, parent position, cost, heuristic,
//! reached (`MNode.java:102-136`) — and [`common::PathNode`] already models one, so the
//! debug overlay is the one place in this family where a field has a type rather than a
//! length.
//!
//! **`SplitPacketMessage` is the framing under every other message.** A payload too
//! large for one packet is cut into fragments, each carrying the same communication
//! id, its index, whether it is the last, and which message type to rebuild from
//! (`:26-46`); [`SplitPacket`] keeps all four numbers and the bytes, and **cannot
//! reassemble anything**: the message table it would need
//! (`Network.getNetwork().getMessagesTypes()`, `:110`) is the network layer this port
//! does not have. `RAPPORT.md` records that boundary — the type is the fragment, not
//! the reassembler.

use super::common::{
    bool_of, boolean, bytes, bytes_of, chunk_cap, chunk_cap_of, int, int_in, nodes,
    nodes_of, pos_of, position, ChunkCap, Message, PathNode, Refusal,
};
use mod_api::Compound;

/// `SyncPathMessage.java:19-165` — the pathfinding overlay's last search, in six
/// lists.
///
/// Java writes them in one order — visited, not visited, path, visited later,
/// original path, extra — and a `Set<MNode>` each (`:24-37`), which has **no order**.
/// The port keeps a `Vec` per list: a set's iteration order is not a property of the
/// message (Java's is a `HashSet`, so the same set has no stable order either), and a
/// vector is what a renderer iterates.
#[derive(Clone, Debug, PartialEq)]
pub struct SyncPath {
    /// `lastDebugNodesVisited` — every node the search opened.
    pub visited: Vec<PathNode>,
    /// `lastDebugNodesNotVisited` — the frontier it left behind.
    pub not_visited: Vec<PathNode>,
    /// `lastDebugNodesPath` — the path it found.
    pub path: Vec<PathNode>,
    /// `debugNodesVisitedLater` — nodes a later search opened.
    pub visited_later: Vec<PathNode>,
    /// `debugNodesOrgPath` — the path before smoothing.
    pub original_path: Vec<PathNode>,
    /// `debugNodesExtra` — whatever the search marked extra.
    pub extra: Vec<PathNode>,
}

impl Message for SyncPath {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("lastDebugNodesVisited".into(), nodes(&self.visited)),
            ("lastDebugNodesNotVisited".into(), nodes(&self.not_visited)),
            ("lastDebugNodesPath".into(), nodes(&self.path)),
            ("debugNodesVisitedLater".into(), nodes(&self.visited_later)),
            ("debugNodesOrgPath".into(), nodes(&self.original_path)),
            ("debugNodesExtra".into(), nodes(&self.extra)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(SyncPath {
            visited: nodes_of(data, "lastDebugNodesVisited")?,
            not_visited: nodes_of(data, "lastDebugNodesNotVisited")?,
            path: nodes_of(data, "lastDebugNodesPath")?,
            visited_later: nodes_of(data, "debugNodesVisitedLater")?,
            original_path: nodes_of(data, "debugNodesOrgPath")?,
            extra: nodes_of(data, "debugNodesExtra")?,
        })
    }
}

/// `SyncPathReachedMessage.java:17-73` — the worker arrived at this node, so the
/// overlay paints it (`:65-72`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncPathReached {
    pub reached: [i32; 3],
}

impl Message for SyncPathReached {
    fn to_compound(&self) -> Compound {
        Compound::from([("reached".into(), position(self.reached))])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(SyncPathReached { reached: pos_of(data, "reached")? })
    }
}

/// `UpdateChunkCapabilityMessage.java:23-91` — one chunk's claim record changed
/// (`:47-63`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateChunkCapability {
    pub cap: ChunkCap,
}

impl Message for UpdateChunkCapability {
    fn to_compound(&self) -> Compound {
        Compound::from([("chunkCapData".into(), chunk_cap(&self.cap))])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(UpdateChunkCapability { cap: chunk_cap_of(data, "chunkCapData")? })
    }
}

/// `UpdateChunkRangeCapabilityMessage.java:27-115` — the same record for every chunk
/// of a square (`:51-70`), which is what a colony border looks like when it moves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateChunkRangeCapability {
    pub caps: Vec<ChunkCap>,
}

impl Message for UpdateChunkRangeCapability {
    fn to_compound(&self) -> Compound {
        let rows: Vec<Compound> = self
            .caps
            .iter()
            .map(|cap| match chunk_cap(cap) {
                mod_api::Tag::Compound(compound) => compound,
                _ => Compound::new(),
            })
            .collect();
        Compound::from([("caps".into(), super::common::records(&rows))])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let mut caps = Vec::new();
        for row in super::common::records_of(data, "caps")? {
            caps.push(chunk_cap_of(&Compound::from([("row".into(), mod_api::Tag::Compound(row.clone()))]), "row")?);
        }
        Ok(UpdateChunkRangeCapability { caps })
    }
}

/// `UpdateClientWithCompatibilityMessage.java:18-77` — every recipe, every item and
/// every block the colony knows, on connect (`:40`).
///
/// `compatibility` is `ICompatibilityManager.serialize`'s buffer (`:40-41`), opaque
/// here: that class is a registry of nine tables and a port of its own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateClientWithCompatibility {
    pub compatibility: Vec<u8>,
}

impl Message for UpdateClientWithCompatibility {
    fn to_compound(&self) -> Compound {
        Compound::from([("compatibility".into(), bytes(&self.compatibility))])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(UpdateClientWithCompatibility { compatibility: bytes_of(data, "compatibility")? })
    }
}

/// `GlobalQuestSyncMessage.java:17-73` — the quest pack, loaded once (`:67-71`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlobalQuestSync {
    pub quests: Vec<u8>,
}

impl Message for GlobalQuestSync {
    fn to_compound(&self) -> Compound {
        Compound::from([("quests".into(), bytes(&self.quests))])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(GlobalQuestSync { quests: bytes_of(data, "quests")? })
    }
}

/// `GlobalDiseaseSyncMessage.java:17-72` — the disease pack, loaded once (`:66-70`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlobalDiseaseSync {
    pub diseases: Vec<u8>,
}

impl Message for GlobalDiseaseSync {
    fn to_compound(&self) -> Compound {
        Compound::from([("diseases".into(), bytes(&self.diseases))])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(GlobalDiseaseSync { diseases: bytes_of(data, "diseases")? })
    }
}

/// `splitting/SplitPacketMessage.java:21-155` — one fragment of a message that was
/// too big to send.
///
/// The four numbers are Java's (`:26-46`), and their defaults are `-1`, not `0`
/// (`:26`, `:31`, `:41`): a fragment that never got its index is distinguishable from
/// the first one. The port keeps `i32` and refuses a negative index here, because
/// `-1` is Java's *unset* and a fragment on the wire has always been set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitPacket {
    pub communication_id: i32,
    pub packet_index: i32,
    /// Whether this is the last fragment of its message (`:36`, `:95-98`).
    pub terminator: bool,
    /// The message type to rebuild from, in the sender's own table (`:41`, `:110`).
    pub inner_message_id: i32,
    pub payload: Vec<u8>,
}

impl Message for SplitPacket {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("communicationId".into(), int(self.communication_id)),
            ("packetIndex".into(), int(self.packet_index)),
            ("terminator".into(), boolean(self.terminator)),
            ("innerMessageId".into(), int(self.inner_message_id)),
            ("payload".into(), bytes(&self.payload)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(SplitPacket {
            communication_id: int_in(data, "communicationId", 0, i32::MAX)?,
            packet_index: int_in(data, "packetIndex", 0, i32::MAX)?,
            terminator: bool_of(data, "terminator")?,
            inner_message_id: int_in(data, "innerMessageId", 0, i32::MAX)?,
            payload: bytes_of(data, "payload")?,
        })
    }
}

/// The eight messages of this family, by Java file and Rust type.
pub const CENSUS: &[(&str, &str)] = &[
    ("client/SyncPathMessage.java", "SyncPath"),
    ("client/SyncPathReachedMessage.java", "SyncPathReached"),
    ("client/UpdateChunkCapabilityMessage.java", "UpdateChunkCapability"),
    ("client/UpdateChunkRangeCapabilityMessage.java", "UpdateChunkRangeCapability"),
    ("client/UpdateClientWithCompatibilityMessage.java", "UpdateClientWithCompatibility"),
    ("client/GlobalQuestSyncMessage.java", "GlobalQuestSync"),
    ("client/colony/GlobalDiseaseSyncMessage.java", "GlobalDiseaseSync"),
    ("splitting/SplitPacketMessage.java", "SplitPacket"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::common::round_trip;

    fn node(at: [i32; 3], cost: f64) -> PathNode {
        PathNode { parent: None, at, cost, heuristic: 4.0, reached: false }
    }

    #[test]
    fn the_path_overlay_round_trips_six_lists() {
        let message = SyncPath {
            visited: vec![node([0, 64, 0], 0.0), node([1, 64, 0], 1.0)],
            not_visited: vec![node([2, 64, 0], 2.5)],
            path: vec![node([0, 64, 0], 0.0)],
            visited_later: Vec::new(),
            original_path: vec![node([0, 65, 0], 9.0)],
            extra: vec![node([-1, 64, 0], 0.5)],
        };
        let read = round_trip(message.clone());
        assert_eq!(read.visited.len(), 2);
        assert_eq!(read.not_visited, message.not_visited);
        assert!(read.visited_later.is_empty());
        assert_eq!(read.original_path[0].cost, 9.0);
    }

    #[test]
    fn a_reached_node_round_trips() {
        assert_eq!(round_trip(SyncPathReached { reached: [4, 63, -7] }).reached, [4, 63, -7]);
    }

    #[test]
    fn a_single_chunk_claim_round_trips_and_a_range_of_them_too() {
        let single = ChunkCap { x: 1, z: 2, owning_colony: 3, claiming_colonies: vec![3, 4] };
        assert_eq!(round_trip(UpdateChunkCapability { cap: single.clone() }).cap, single);

        let range = UpdateChunkRangeCapability {
            caps: vec![
                ChunkCap { x: 0, z: 0, owning_colony: 1, claiming_colonies: Vec::new() },
                ChunkCap { x: 1, z: 0, owning_colony: 1, claiming_colonies: vec![1] },
            ],
        };
        assert_eq!(round_trip(range.clone()).caps, range.caps);
        assert!(round_trip(UpdateChunkRangeCapability { caps: Vec::new() }).caps.is_empty());
    }

    #[test]
    fn the_three_whole_payloads_round_trip_as_bytes() {
        assert_eq!(
            round_trip(UpdateClientWithCompatibility { compatibility: vec![1, 2, 3] }).compatibility,
            vec![1, 2, 3]
        );
        assert_eq!(round_trip(GlobalQuestSync { quests: vec![9; 8] }).quests, vec![9; 8]);
        assert_eq!(round_trip(GlobalDiseaseSync { diseases: vec![4, 5] }).diseases, vec![4, 5]);
    }

    #[test]
    fn a_fragment_round_trips_its_four_numbers_and_its_bytes() {
        let fragment = SplitPacket {
            communication_id: 12,
            packet_index: 3,
            terminator: true,
            inner_message_id: 42,
            payload: vec![0, 255, 17],
        };
        let read = round_trip(fragment);
        assert_eq!((read.communication_id, read.packet_index), (12, 3));
        assert!(read.terminator);
        assert_eq!(read.payload, vec![0, 255, 17]);
    }

    #[test]
    fn a_fragment_with_an_unset_index_is_refused_like_java_never_sends_one() {
        let mut data = SplitPacket {
            communication_id: 0,
            packet_index: 0,
            terminator: false,
            inner_message_id: 0,
            payload: Vec::new(),
        }
        .to_compound();
        data.insert("packetIndex".into(), int(-1));
        assert_eq!(
            SplitPacket::from_compound(&data),
            Err(Refusal::Number { key: "packetIndex", min: 0, max: i64::from(i32::MAX) })
        );
        data.insert("packetIndex".into(), int(0));
        data.remove(&"payload".into());
        assert_eq!(SplitPacket::from_compound(&data), Err(Refusal::Missing { key: "payload" }));
        assert_eq!(CENSUS.len(), 8);
    }
}
