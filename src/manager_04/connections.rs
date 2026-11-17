//! The roads between two colonies: the signs, the gate houses, the paths that are still
//! being walked, and the diplomacy that travels over them.
//!
//! Java reference: `core/colony/managers/ColonyConnectionManager.java` (805 lines), the
//! six types of `api/colony/connections/` (`ColonyConnectionNode.java:13`,
//! `PendingConnectionNode.java:15`, `ColonyConnection.java:14`, `DiplomacyStatus.java:6`,
//! `ConnectionEvent.java:15`, `ConnectionEventType.java:6`) and the six save keys
//! (`NbtTagConstants.java:99-112`).
//!
//! | Question | Answer | Java |
//! | --- | --- | --- |
//! | how far a sign may link from the last one | **50 blocks**, squared compare | `:102`, `:127`, `:292`, `:533` |
//! | how many signs may stack | **5**, and it is the *lowest* of the stack that links | `:33`, `:454-466` |
//! | when a sign may be placed | never while **any** path is pending | `:119-123` |
//! | when a node is written to the network | only once its path succeeded — and only for a `DEFAULT` sign | `:227-238` |
//! | when two colonies are linked | once **both** gates are reachable, at the end only | `:341-430` |
//! | what an unknown colony id in the map does | it is forgotten on the next tick | `:477-482` |
//!
//! **Four measured facts, and three of them are bugs the port does not fix.**
//!
//! 1. **A chain of four signs refuses the fifth.** `addNewConnectionNode` walks back to a
//!    gate house (`:142-154`), but the walk adds the node it *lands on* to `visitedNodes`
//!    (`:147`) instead of the node it *left*, so the loop stops after two steps: from
//!    `S4` it reaches `S2`, `gateHouses.contains(S2)` is false, and the sign is refused
//!    with `COM_MINECOLONIES_SIGN_MISSING_LINK`. [`Connections::terminal`] is that walk,
//!    line for line, and a test pins both sides of it. **`connect`'s own walks
//!    (`:351-361`, `:379-387`) do it correctly** — they add the current position before
//!    stepping — so the port has two walkers, not one, because Java has two.
//! 2. **A `FIX_PATH` or `CONNECT_COLONY` node is never written into the network.**
//!    `colonyConnections.put` sits inside `if (type == DEFAULT)` (`:227-238`), so a sign
//!    placed to mend a broken path is announced as connected (`:240-243`) and stays a
//!    stranger to every later walk. The port keeps it and says so.
//! 3. **A path that fails is retried forever for those two kinds.** `isDone()` without
//!    `isPathReachingDestination()` does `continue` for anything but `DEFAULT`
//!    (`:319-322`), leaving the entry in the map: the sign is never broken and the
//!    colony can never place another one, because *any* pending entry refuses a new sign
//!    (`:119-123`). [`Connections::resolve`] answers [`Resolved::Waiting`] and keeps it.
//! 4. **A reloaded node's target is `0`, a fresh one's is `-1`.** `ColonyConnectionNode.read`
//!    uses `getInt(TAG_TARGET_COLONY_ID)` (`:124-130`), whose answer for an absent key is
//!    zero, while the field is initialised to `-1` (`:33`). Every `!= -1` test then reads
//!    a reloaded node as pointing at colony **0**. The port writes and reads the same
//!    three keys with the same default; [`NO_TARGET`] is the fresh value, not the loaded
//!    one, and a test says so.
//!
//! **Two divergences, both about not hanging.** Java's chain walks in `connect` have no
//! cycle guard and a hand-edited save can make them loop for ever; the port breaks on a
//! repeat and names nothing, it just stops. And a null `directlyConnectedColonies.get(id)`
//! would throw at `:271` — the port reads it as [`Status::Neutral`], which is what
//! `getColonyDiplomacyStatus` answers for an unknown id anyway (`:803`).
//!
//! **What is deliberately not here.** The path job itself (`PathJobSignConnection`,
//! `:444-446`) and the block reads of `findLowestPoint` (`:454-466`): a wasm mod has no
//! pathfinder and no block state, so a caller hands [`Connections::resolve`] the one bit
//! it needs — `reached` — and does the breaking and the messages. The four `save`s of the
//! messages to the managers are [`MANQUANT.md`](MANQUANT) §2, and `src/ui_window_14/alliance.rs`
//! already names [`Status`] and [`EventKind`]; this module reuses them instead of
//! rewriting two enumerations.

use std::collections::{BTreeMap, BTreeSet};

use mod_api::{Compound, Tag};

use crate::manager_04::nbt;
use crate::ui_window_14::alliance::{EventKind, Status};
use crate::ui_window_14::Refusal as Ui14;

/// Java's `BlockPos.ZERO`: the position that means "no neighbour" in a node
/// (`ColonyConnectionNode.java:18`), the sentinel `hasNextNode` tests against (`:137-140`).
pub const ZERO: [i32; 3] = [0, 0, 0];

/// How many signs may stack, and therefore how far down `findLowestPoint` looks
/// (`ColonyConnectionManager.java:33`).
pub const MAX_SIGN_RANGE: i32 = 5;

/// The reach of one link, in blocks: every distance in this file is compared squared
/// against `50 * 50` (`:102`, `:127`, `:292`, `:533`).
pub const LINK_RANGE: i64 = 50;

/// `50 * 50`, the number actually compared.
pub const LINK_RANGE_SQUARED: i64 = LINK_RANGE * LINK_RANGE;

/// A fresh node's target before it has one (`ColonyConnectionNode.java:33`).
pub const NO_TARGET: i32 = -1;

/// `NbtTagConstants.java:99` — the node list.
pub const KEY_CONNECTIONS: &str = "connections";
/// `:41` — the directly connected colonies.
pub const KEY_COLONIES: &str = "colonies";
/// `:101`.
pub const KEY_GATEHOUSES: &str = "gatehouses";
/// `:102`.
pub const KEY_EVENTS: &str = "connectionevents";
/// `:103`.
pub const KEY_PENDING: &str = "pending";
/// `:112`.
pub const KEY_DIPLOMACY: &str = "diplomacy";
/// `:51`.
pub const KEY_POS: &str = "pos";
/// `:52`.
pub const KEY_PREV: &str = "prevpos";
/// `:53`.
pub const KEY_NEXT: &str = "nextpos";
/// `:62`.
pub const KEY_TARGET_COLONY: &str = "targetcolony";
/// `:61`.
pub const KEY_COLONY_ID: &str = "colony";
/// `:72`.
pub const KEY_STATUS: &str = "status";
/// `:104`.
pub const KEY_CONNECTION_TYPE: &str = "connectiontype";
/// `:8`.
pub const KEY_ID: &str = "id";
/// `:9`.
pub const KEY_NAME: &str = "name";

/// One link of the network: a sign, or a gate house standing in for one.
///
/// Java puts both in one class and calls the gate house a node with no `previous`
/// (`:229`, `:255`); the port keeps the same shape so that a walk does not have to know
/// which of the two it is looking at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four fields `ColonyConnectionNode` holds
pub struct Node {
    /// The neighbour towards the gate (`getPreviousNode`).
    pub previous: [i32; 3],
    /// The neighbour towards the next colony (`getNextNode`).
    pub next: [i32; 3],
    /// The colony this chain leads to, or [`NO_TARGET`] (`getTargetColonyId`).
    pub target: i32,
}

impl Node {
    /// A node that points nowhere: the two fields Java initialises to `BlockPos.ZERO`
    /// and the target to [`NO_TARGET`] (`ColonyConnectionNode.java:18-33`).
    pub fn new() -> Node {
        node_at(ZERO, NO_TARGET)
    }

    /// `hasNextNode` (`ColonyConnectionNode.java:137-140`): a node may receive a link
    /// only while its `next` is still zero.
    pub fn has_next(&self) -> bool {
        self.next != ZERO
    }
}

/// Which of the three pending kinds a node is (`PendingConnectionNode.java:20-25`).
///
/// The ordinal is saved (`:68`, `:80`), so the order is a data format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: three values in the Java, and a fourth would need a migration
pub enum PendingKind {
    /// A sign a player placed (`addNewConnectionNode`).
    Default,
    /// A sign placed to mend a path (`:301`, `:535`).
    FixPath,
    /// A link between two colonies (`:198`).
    ConnectColony,
}

impl PendingKind {
    /// The ordinal `write()` stores.
    pub fn ordinal(self) -> i32 {
        match self {
            PendingKind::Default => 0,
            PendingKind::FixPath => 1,
            PendingKind::ConnectColony => 2,
        }
    }

    /// The ordinal read back. Java indexes `values()` (`:80`) and throws out of range;
    /// the port names the refusal.
    pub fn from_ordinal(ordinal: i32) -> Result<PendingKind, Refusal> {
        match ordinal {
            0 => Ok(PendingKind::Default),
            1 => Ok(PendingKind::FixPath),
            2 => Ok(PendingKind::ConnectColony),
            other => Err(Refusal::UnknownConnectionType { ordinal: other }),
        }
    }
}

/// A connection that is being walked: Java's `PendingConnectionNode` (`:15-90`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the node plus the kind Java adds on top of it
pub struct Pending {
    pub node: Node,
    pub kind: PendingKind,
}

/// What a caller has to do to start a pending connection: the position, the neighbour it
/// will attach to, and the colony it will lead to.
///
/// Java hands back a `PathResult` it has already started (`:129`, `:198`, `:535`); the
/// port hands back the data, because the pathfinder is not its to run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: what a pending node is made of
pub struct Plan {
    pub at: [i32; 3],
    pub previous: [i32; 3],
    pub target: i32,
    pub kind: PendingKind,
}

/// One colony on the other end of a link: `ColonyConnection.java:14-83` with the name
/// owned instead of borrowed.
///
/// `ui_window_14::alliance::Connection` is the same four fields as a *borrowed view*, for
/// a screen that already holds the colony. A stored link outlives the colony record it
/// was copied from, so it owns its name.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the four fields `ColonyConnection` writes
pub struct Link {
    pub id: i32,
    pub name: String,
    /// The gate this colony is reached at (`ColonyConnection.pos`).
    pub gate: [i32; 3],
    pub status: Status,
}

/// Java's `ConnectionEvent` (`ConnectionEvent.java:15`): which colony spoke, its name, and
/// what it said.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the three fields of the record
pub struct Notice {
    pub id: i32,
    pub name: String,
    pub kind: EventKind,
}

/// Why a connection operation did nothing. Every one of these is a `return false`, a
/// `return` or a message in Java; none of them is a Java refusal that names itself.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// A path is already being walked, so a new sign is refused — **for the whole
    /// colony**, not just near the position (`:119-123`).
    PathPending { pending: [i32; 3] },
    /// No open node within [`LINK_RANGE`] blocks (`:109-112`).
    TooFar { at: [i32; 3] },
    /// The chain the closest node belongs to does not end at a gate house
    /// (`:150-154`).
    ChainBroken { at: [i32; 3], terminal: [i32; 3] },
    /// There is nothing pending at that position (`:222-225`).
    NotPending { at: [i32; 3] },
    /// The colony the gate names is not in this dimension (`:343-348`).
    NoSuchColony { colony: i32 },
    /// The target node already carries a link and is not a gate house (`:370-375`).
    TargetBusy { at: [i32; 3] },
    /// The target's own chain does not reach one of its gate houses (`:389-393`).
    TargetChainBroken { at: [i32; 3] },
    /// The colony that sent the notice is neither a direct nor an indirect connection
    /// (`:727-730`) — **nothing is written**.
    UnknownOrigin { id: i32 },
    /// The colony that sent the notice does not know us back (`:773-776`) — the notice
    /// **is** recorded, the diplomacy is not.
    OriginUnreachable { id: i32 },
    /// The ordinal a save carries is not a `PendingConnectionType` (`:80`).
    UnknownConnectionType { ordinal: i32 },
    /// A status or event ordinal of `ui_window_14`, which already names both.
    Ui14(Ui14),
}

impl Refusal {
    pub fn name(&self) -> &'static str {
        match self {
            Refusal::PathPending { .. } => "manager_04.connections.refusal.path_pending",
            Refusal::TooFar { .. } => "manager_04.connections.refusal.too_far",
            Refusal::ChainBroken { .. } => "manager_04.connections.refusal.chain_broken",
            Refusal::NotPending { .. } => "manager_04.connections.refusal.not_pending",
            Refusal::NoSuchColony { .. } => "manager_04.connections.refusal.no_such_colony",
            Refusal::TargetBusy { .. } => "manager_04.connections.refusal.target_busy",
            Refusal::TargetChainBroken { .. } => "manager_04.connections.refusal.target_chain_broken",
            Refusal::UnknownOrigin { .. } => "manager_04.connections.refusal.unknown_origin",
            Refusal::OriginUnreachable { .. } => "manager_04.connections.refusal.origin_unreachable",
            Refusal::UnknownConnectionType { .. } => {
                "manager_04.connections.refusal.unknown_connection_type"
            }
            Refusal::Ui14(inner) => inner.name(),
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

/// What one finished path did (`tick`, `:209-327`).
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the five endings Java has, one variant each
pub enum Resolved {
    /// A `DEFAULT` sign whose previous neighbour is neither a node nor a gate house: the
    /// block is broken and **no node is written** (`:229-236`). This is the one refusal
    /// of `tick` that writes nothing, and the reason it is a value rather than an error
    /// is that the caller still has a block to break.
    Refused { at: [i32; 3], previous: [i32; 3] },
    /// A `FIX_PATH` or `CONNECT_COLONY` path that did not reach: kept pending
    /// (`:319-322`).
    Waiting { at: [i32; 3] },
    /// A `DEFAULT` path that did not reach: the sign is broken and the entry is gone
    /// (`:323-325`).
    Failed { at: [i32; 3], previous: [i32; 3] },
    /// The node joined the chain (`:237`). What is left is data for the caller: a repair
    /// to schedule, a colony to tell, a link to establish.
    Linked {
        at: [i32; 3],
        previous: [i32; 3],
        kind: PendingKind,
        /// The `FIX_PATH` a `DEFAULT` sign asks for once it is in (`:283-314`).
        repair: Option<Plan>,
        /// What the colony at the other end must record about us (`:255-275`), if this
        /// sign is a gate house.
        reciprocal: Option<Reciprocal>,
        /// The colony-to-colony link still to establish (`:277-280`).
        connect: Option<Connect>,
    },
}

/// A write that happens **on the other colony's manager** — the one effect a manager
/// cannot do alone.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: one colony and the link it must hold
pub struct Reciprocal {
    pub colony: i32,
    pub link: Link,
}

/// A `CONNECT_COLONY` that has arrived: the two positions
/// `connectToColony(thisPos, targetId, targetPos)` is called with (`:277-280`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three arguments Java passes
pub struct Connect {
    pub colony: i32,
    pub at: [i32; 3],
    pub previous: [i32; 3],
}

/// What `removeConnectionNode` severed (`:168-187`): the two neighbours whose pointers
/// were zeroed, for the two messages Java sends.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: the two neighbours a chain has
pub struct Severed {
    pub previous: Option<[i32; 3]>,
    pub next: Option<[i32; 3]>,
}

/// What `updateConnectedColonies` changed (`:472-502`), so the caller can save and
/// redraw. Java only marks the colony dirty for the third.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three writes of one pass
pub enum Change {
    /// The colony is gone: the entry is dropped (`:478-482`).
    Forgotten { id: i32 },
    /// The colony was renamed: the cached name is replaced (`:484-488`).
    Renamed { id: i32 },
    /// An ally's ally became reachable (`:490-499`) — the one case that marks dirty.
    Indirect { id: i32 },
}

/// What an update pass needs to know about a colony it holds a link to. The port has no
/// colony registry, so the caller reads it and hands it over.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: the two facts `updateConnectedColonies` reads from another colony
pub struct Peer {
    pub name: String,
    /// That colony's own direct links (`getDirectlyConnectedColonies`).
    pub direct: Vec<Link>,
}

/// One colony's connections: the signs, the gates, the pending paths, the two maps of
/// neighbours and the diplomacy over them.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the seven fields `ColonyConnectionManager` keeps
pub struct Connections {
    id: i32,
    name: String,
    nodes: BTreeMap<[i32; 3], Node>,
    pending: BTreeMap<[i32; 3], Pending>,
    gate_houses: Vec<[i32; 3]>,
    direct: BTreeMap<i32, Link>,
    indirect: BTreeMap<i32, Link>,
    notices: BTreeMap<i32, Notice>,
    diplomacy: BTreeMap<i32, Status>,
}

impl Connections {
    /// The manager of one colony: Java's constructor takes the colony for its id and its
    /// name (`:81-84`), and both are needed the day a link has to be written on the
    /// other side.
    pub fn new(id: i32, name: &str) -> Connections {
        Connections {
            id,
            name: name.to_owned(),
            nodes: BTreeMap::new(),
            pending: BTreeMap::new(),
            gate_houses: Vec::new(),
            direct: BTreeMap::new(),
            indirect: BTreeMap::new(),
            notices: BTreeMap::new(),
            diplomacy: BTreeMap::new(),
        }
    }

    pub fn id(&self) -> i32 {
        self.id
    }

    pub fn nodes(&self) -> &BTreeMap<[i32; 3], Node> {
        &self.nodes
    }

    pub fn node(&self, at: [i32; 3]) -> Option<&Node> {
        self.nodes.get(&at)
    }

    pub fn pending(&self) -> &BTreeMap<[i32; 3], Pending> {
        &self.pending
    }

    pub fn gate_houses(&self) -> &[[i32; 3]] {
        &self.gate_houses
    }

    pub fn direct(&self) -> &BTreeMap<i32, Link> {
        &self.direct
    }

    pub fn indirect(&self) -> &BTreeMap<i32, Link> {
        &self.indirect
    }

    pub fn notices(&self) -> &BTreeMap<i32, Notice> {
        &self.notices
    }

    /// The diplomacy this colony declared about another, over the value cached in the
    /// link: Java's own field (`:73`), written only by [`notice`](Self::notice).
    pub fn declared(&self, id: i32) -> Option<Status> {
        self.diplomacy.get(&id).copied()
    }

    /// Whether this colony holds a link to `id`, direct or indirect — the test
    /// `triggerConnectionEvent` makes on the *other* manager (`:763-772`).
    pub fn knows(&self, id: i32) -> bool {
        self.direct.contains_key(&id) || self.indirect.contains_key(&id)
    }

    /// The status a player sees: the direct link, then the indirect one, then neutral —
    /// `getColonyDiplomacyStatus` (`:792-804`).
    pub fn diplomacy_status(&self, id: i32) -> Status {
        if let Some(link) = self.direct.get(&id) {
            return link.status;
        }
        if let Some(link) = self.indirect.get(&id) {
            return link.status;
        }
        Status::Neutral
    }

    /// `addNewConnectionNode` (`:116-165`), with the three answers Java has.
    ///
    /// **The order is the rule**, and it is why this writes nothing until the end: the
    /// pending guard, then a gate house within reach, then the closest open node, then
    /// the walk back to a gate. A sign that is refused leaves [`pending`](Self::pending)
    /// exactly as it was.
    pub fn attempt_add_node(&mut self, at: [i32; 3]) -> Result<Plan, Refusal> {
        if let Some(pending) = self.pending.keys().next().copied() {
            return Err(Refusal::PathPending { pending });
        }
        if let Some(gate) = self
            .gate_houses
            .iter()
            .copied()
            .find(|gate| dist_sqr(*gate, at) <= LINK_RANGE_SQUARED)
        {
            let plan = Plan {
                at,
                previous: gate,
                target: NO_TARGET,
                kind: PendingKind::Default,
            };
            self.pending.insert(at, Pending { node: node_at(gate, NO_TARGET), kind: PendingKind::Default });
            return Ok(plan);
        }
        let Some((anchor, anchor_target)) = self.closest_open(at) else {
            return Err(Refusal::TooFar { at });
        };
        let start = self.node(anchor).map(|node| node.previous).unwrap_or(ZERO);
        let terminal = self.terminal(start);
        if !self.gate_houses.contains(&terminal) {
            return Err(Refusal::ChainBroken { at, terminal });
        }
        let plan = Plan {
            at,
            previous: anchor,
            target: anchor_target,
            kind: PendingKind::Default,
        };
        self.pending.insert(
            at,
            Pending {
                node: node_at(anchor, anchor_target),
                kind: PendingKind::Default,
            },
        );
        Ok(plan)
    }

    /// The closest node with an open next, within reach: `getClosestNodeWithOpenConnection`
    /// (`:92-114`). Ties are broken by the first one met, which Java decides by its
    /// `LinkedHashMap`'s insertion order and the port by position order — a tie is two
    /// signs at the same distance, and nothing else.
    pub fn closest_open(&self, at: [i32; 3]) -> Option<([i32; 3], i32)> {
        let mut best: Option<([i32; 3], i32, i64)> = None;
        for (pos, node) in &self.nodes {
            if node.has_next() {
                continue;
            }
            let distance = dist_sqr(*pos, at);
            if distance > LINK_RANGE_SQUARED {
                continue;
            }
            if best.is_none_or(|(_, _, closest)| distance < closest) {
                best = Some((*pos, node.target, distance));
            }
        }
        best.map(|(at, target, _)| (at, target))
    }

    /// The walk back to a gate house, **as Java writes it** (`:142-148`).
    ///
    /// `visitedNodes.add` lands on the node that was just stepped to, so the loop stops
    /// one step early; fact 1 of the module note is this function, and a test pins it.
    pub fn terminal(&self, start: [i32; 3]) -> [i32; 3] {
        let mut visited: BTreeSet<[i32; 3]> = BTreeSet::new();
        let mut at = start;
        while self.nodes.contains_key(&at) && !visited.contains(&at) {
            at = self.node(at).map(|node| node.previous).unwrap_or(ZERO);
            visited.insert(at);
        }
        at
    }

    /// The walk `connectToColony` uses (`:351-361`, `:379-387`), which is the correct
    /// one: the current position is added before the step, so it follows the whole chain
    /// and stops only on a repeat.
    ///
    /// Returns the terminal and the last node it left, which is the position Java puts in
    /// the `COM_MINECOLONIES_CONNECTION_BROKEN` message.
    fn walk(&self, start: [i32; 3]) -> ([i32; 3], [i32; 3]) {
        let mut visited: BTreeSet<[i32; 3]> = BTreeSet::new();
        let mut at = start;
        let mut last = start;
        while self.nodes.contains_key(&at) {
            last = at;
            at = self.node(at).map(|node| node.previous).unwrap_or(ZERO);
            if !visited.insert(at) {
                break;
            }
        }
        (at, last)
    }

    /// `removeConnectionNode` (`:167-187`): the node goes first, then both neighbours lose
    /// the pointer that named it, and whatever was pending there goes with it.
    pub fn remove_node(&mut self, at: [i32; 3]) -> Severed {
        let severed = match self.nodes.remove(&at) {
            None => Severed::default(),
            Some(node) => {
                let mut severed = Severed::default();
                if node.previous != ZERO {
                    if let Some(previous) = self.nodes.get_mut(&node.previous) {
                        previous.next = ZERO;
                        severed.previous = Some(node.previous);
                    }
                }
                if node.next != ZERO {
                    if let Some(next) = self.nodes.get_mut(&node.next) {
                        next.previous = ZERO;
                        severed.next = Some(node.next);
                    }
                }
                severed
            }
        };
        self.pending.remove(&at);
        severed
    }

    /// `attemptEstablishConnection` (`:189-204`): the link starts from an **existing** open
    /// node, so the sign is already in the network when the path begins.
    ///
    /// Java does not check the pending map here, unlike `addNewConnectionNode`: a link
    /// asked for while a sign is pending overwrites the entry at that position. The port
    /// keeps the asymmetry and does not pretend otherwise.
    pub fn attempt_connect(
        &mut self,
        target: [i32; 3],
        colony: i32,
    ) -> Result<Plan, Refusal> {
        let Some((at, _)) = self.closest_open(target) else {
            return Err(Refusal::TooFar { at: target });
        };
        let plan = Plan {
            at,
            previous: target,
            target: colony,
            kind: PendingKind::ConnectColony,
        };
        self.pending.insert(
            at,
            Pending {
                node: node_at(target, colony),
                kind: PendingKind::ConnectColony,
            },
        );
        Ok(plan)
    }

    /// One pending path, finished or failed: the decision table of `tick`
    /// (`:209-327`), one node at a time.
    ///
    /// **The order of the writes is the point.** The entry is removed first (`:222-225`,
    /// which is itself the refusal), then, for a `DEFAULT` sign only, the chain is
    /// checked (`:229-236`), and the node is written **after** that check (`:237`). A
    /// [`Resolved::Refused`] leaves [`nodes`](Self::nodes) exactly as it was; a test
    /// proves it.
    pub fn resolve(&mut self, at: [i32; 3], reached: bool) -> Result<Resolved, Refusal> {
        let Some(pending) = self.pending.remove(&at) else {
            return Err(Refusal::NotPending { at });
        };
        let previous = pending.node.previous;
        let target = pending.node.target;
        if !reached {
            if pending.kind == PendingKind::Default {
                return Ok(Resolved::Failed { at, previous });
            }
            // Java's `continue` leaves the entry where it was (`:319-322`).
            self.pending.insert(at, pending);
            return Ok(Resolved::Waiting { at });
        }
        let connection = if previous != ZERO && self.nodes.contains_key(&previous) {
            Some(previous)
        } else {
            None
        };
        if pending.kind == PendingKind::Default
            && connection.is_none()
            && !self.gate_houses.contains(&previous)
        {
            return Ok(Resolved::Refused { at, previous });
        }
        if pending.kind == PendingKind::Default {
            self.nodes.insert(
                at,
                Node {
                    previous: pending.node.previous,
                    next: pending.node.next,
                    target,
                },
            );
        }
        if let Some(link) = connection {
            if let Some(node) = self.nodes.get_mut(&link) {
                node.next = at;
            }
            if let Some(node) = self.nodes.get_mut(&at) {
                node.previous = link;
            }
        }
        let mut reciprocal = None;
        if self.gate_houses.contains(&at) {
            let next = self.node(at).map(|node| node.next).unwrap_or(ZERO);
            if next != ZERO {
                if let Some(node) = self.nodes.get_mut(&next) {
                    node.previous = at;
                }
                if target != NO_TARGET {
                    // Java reads `directlyConnectedColonies.get(target).diplomacyStatus`
                    // and would throw on a null (`:271`); the port answers Neutral, which
                    // is what `getColonyDiplomacyStatus` says for an unknown id anyway.
                    let status = self
                        .direct
                        .get(&target)
                        .map(|link| link.status)
                        .unwrap_or(Status::Neutral);
                    reciprocal = Some(Reciprocal {
                        colony: target,
                        link: Link {
                            id: self.id,
                            name: self.name.clone(),
                            gate: at,
                            status,
                        },
                    });
                }
            }
        }
        let connect = if pending.kind == PendingKind::ConnectColony && target != NO_TARGET {
            Some(Connect {
                colony: target,
                at,
                previous,
            })
        } else {
            None
        };
        let repair = if pending.kind == PendingKind::Default {
            self.schedule_repair(at, target)
        } else {
            None
        };
        Ok(Resolved::Linked {
            at,
            previous,
            kind: pending.kind,
            repair,
            reciprocal,
            connect,
        })
    }

    /// The repair a `DEFAULT` sign asks for once it joined (`:283-314`): the closest node
    /// whose `previous` is still zero, other than the newcomer itself, within reach.
    fn schedule_repair(&mut self, at: [i32; 3], target: i32) -> Option<Plan> {
        let mut best: Option<([i32; 3], i32, i64)> = None;
        for (pos, node) in &self.nodes {
            if node.previous != ZERO || *pos == at {
                continue;
            }
            let distance = dist_sqr(*pos, at);
            if distance > LINK_RANGE_SQUARED {
                continue;
            }
            if best.is_none_or(|(_, _, closest)| distance < closest) {
                best = Some((*pos, node.target, distance));
            }
        }
        let (pos, node_target, _) = best?;
        let next = self.node(pos).map(|node| node.next).unwrap_or(ZERO);
        let target = if target != NO_TARGET {
            target
        } else if node_target != NO_TARGET {
            node_target
        } else {
            NO_TARGET
        };
        let plan = Plan {
            at: pos,
            previous: at,
            target,
            kind: PendingKind::FixPath,
        };
        self.pending.insert(
            pos,
            Pending {
                node: Node {
                    previous: at,
                    next,
                    target,
                },
                kind: PendingKind::FixPath,
            },
        );
        Some(plan)
    }

    /// `connectToColony` (`:341-430`): the two-sided link, with **all four checks before
    /// the first write**.
    ///
    /// The port takes the other colony's manager as an argument because it has no colony
    /// registry; `None` is Java's `getColonyByDimension` answering null (`:343-348`).
    /// The four refusals are Java's four messages, in Java's order: the other colony is
    /// gone, our own chain is broken, the target node is taken, the target's chain is
    /// broken. Nothing is written until all four pass, and a test checks both maps after
    /// each of them.
    pub fn connect(
        &mut self,
        other: Option<&mut Connections>,
        at: [i32; 3],
        target_at: [i32; 3],
    ) -> Result<Link, Refusal> {
        let Some(other) = other else {
            return Err(Refusal::NoSuchColony { colony: NO_TARGET });
        };
        let (our_gate, last) = self.walk(at);
        if !self.gate_houses.contains(&our_gate) {
            return Err(Refusal::ChainBroken { at: last, terminal: our_gate });
        }
        let theirs = other.node(target_at).copied();
        if let Some(node) = theirs {
            if node.has_next() && !other.gate_houses.contains(&target_at) {
                return Err(Refusal::TargetBusy { at: target_at });
            }
        }
        let start = theirs.map(|node| node.previous).unwrap_or(target_at);
        let (their_gate, _) = other.walk(start);
        if !other.gate_houses.contains(&their_gate) {
            return Err(Refusal::TargetChainBroken { at: their_gate });
        }

        // ---- every check passed: the writes start here (`:396-424`) ----
        let ours = Link {
            id: other.id,
            name: other.name.clone(),
            gate: their_gate,
            status: Status::Neutral,
        };
        self.direct.insert(other.id, ours.clone());
        other.direct.insert(
            self.id,
            Link {
                id: self.id,
                name: self.name.clone(),
                gate: our_gate,
                status: Status::Neutral,
            },
        );
        if let Some(node) = self.nodes.get_mut(&at) {
            node.next = target_at;
            node.target = other.id;
        }
        if let Some(node) = other.nodes.get_mut(&target_at) {
            node.next = at;
            node.target = self.id;
        }
        // The two chains remember which colony they lead to (`:409-424`). Java walks the
        // target's chain from the target node and ours from `at`, with no cycle guard;
        // the port stops on a repeat rather than hanging on a hand-edited save.
        let mut probe = start;
        let mut visited = BTreeSet::new();
        while other.nodes.contains_key(&probe) && visited.insert(probe) {
            let previous = other.node(probe).map(|node| node.previous).unwrap_or(ZERO);
            if let Some(node) = other.nodes.get_mut(&probe) {
                node.target = self.id;
            }
            probe = previous;
        }
        let mut probe = at;
        let mut visited = BTreeSet::new();
        while self.nodes.contains_key(&probe) && visited.insert(probe) {
            let previous = self.node(probe).map(|node| node.previous).unwrap_or(ZERO);
            if let Some(node) = self.nodes.get_mut(&probe) {
                node.target = other.id;
            }
            probe = previous;
        }
        Ok(ours)
    }

    /// `addNewGateHouse` (`:522-545`): a new gate joins, and every node that is still
    /// waiting for a way home gets a `FIX_PATH` towards it.
    ///
    /// Java keys every one of those pendings by the **same** position — the gate's — so
    /// only the last survives (`:540`); the port reproduces that and returns the survivor.
    pub fn add_gate_house(&mut self, at: [i32; 3]) -> Option<Plan> {
        if self.gate_houses.contains(&at) {
            return None;
        }
        self.gate_houses.push(at);
        let mut survivor = None;
        let candidates: Vec<([i32; 3], i32)> = self
            .nodes
            .iter()
            .filter(|(_, node)| node.previous == ZERO)
            .filter(|(pos, _)| dist_sqr(**pos, at) <= LINK_RANGE_SQUARED)
            .map(|(pos, node)| (*pos, node.target))
            .collect();
        for (pos, target) in candidates {
            let plan = Plan {
                at,
                previous: pos,
                target,
                kind: PendingKind::FixPath,
            };
            self.pending.insert(
                at,
                Pending {
                    node: Node {
                        previous: pos,
                        next: pos,
                        target,
                    },
                    kind: PendingKind::FixPath,
                },
            );
            survivor = Some(plan);
        }
        survivor
    }

    /// `removeGateHouse` (`:547-571`): the nodes that leaned on it lose their `previous`,
    /// the gate goes, and every directly connected colony is told that our gate is now
    /// the origin. Those last writes belong to the other managers, so they come back as
    /// data.
    pub fn remove_gate_house(&mut self, at: [i32; 3]) -> Vec<Reciprocal> {
        let leaning: Vec<[i32; 3]> = self
            .nodes
            .iter()
            .filter(|(_, node)| node.previous == at)
            .map(|(pos, _)| *pos)
            .collect();
        for pos in leaning {
            if let Some(node) = self.nodes.get_mut(&pos) {
                node.previous = ZERO;
            }
        }
        self.gate_houses.retain(|gate| *gate != at);
        self.direct
            .values()
            .map(|link| Reciprocal {
                colony: link.id,
                link: Link {
                    id: self.id,
                    name: self.name.clone(),
                    gate: ZERO,
                    status: link.status,
                },
            })
            .collect()
    }

    /// `updateConnectedColonies` (`:472-502`), for one of the two maps.
    ///
    /// `view` says what the caller's colony registry knows about the ids this manager
    /// holds. A colony it does not know is **forgotten**; a renamed one is renamed; and an
    /// ally's own links become indirect links — recorded at the **ally's gate**, which is
    /// Java's own choice (`:496`) and the reason an indirect distance is measured from the
    /// wrong place.
    pub fn update(&mut self, indirect: bool, view: &BTreeMap<i32, Peer>) -> Vec<Change> {
        let held: Vec<(i32, Link)> = if indirect {
            self.indirect.iter().map(|(id, link)| (*id, link.clone())).collect()
        } else {
            self.direct.iter().map(|(id, link)| (*id, link.clone())).collect()
        };
        let mut changes = Vec::new();
        for (id, link) in held {
            let Some(peer) = view.get(&id) else {
                if indirect {
                    self.indirect.remove(&id);
                } else {
                    self.direct.remove(&id);
                }
                changes.push(Change::Forgotten { id });
                continue;
            };
            if peer.name != link.name {
                let renamed = Link {
                    name: peer.name.clone(),
                    ..link.clone()
                };
                if indirect {
                    self.indirect.insert(id, renamed);
                } else {
                    self.direct.insert(id, renamed);
                }
                changes.push(Change::Renamed { id });
            }
            if link.status != Status::Allies {
                continue;
            }
            for reachable in &peer.direct {
                if reachable.id == self.id {
                    continue;
                }
                if self.direct.contains_key(&reachable.id) || self.indirect.contains_key(&reachable.id) {
                    continue;
                }
                let status = self
                    .diplomacy
                    .get(&reachable.id)
                    .copied()
                    .unwrap_or(Status::Neutral);
                self.indirect.insert(
                    reachable.id,
                    Link {
                        id: reachable.id,
                        name: reachable.name.clone(),
                        gate: link.gate,
                        status,
                    },
                );
                changes.push(Change::Indirect { id: reachable.id });
            }
        }
        changes
    }

    /// `triggerConnectionEvent` (`:722-784`), with **the two refusals at their real
    /// places**: an unknown origin writes nothing at all (`:727-730`, before `:732`), and
    /// an origin that does not know us is registered in the notice map and *then* refused
    /// (`:732` before `:773-776`).
    ///
    /// Java writes into the other colony's manager directly; the port takes it as an
    /// argument, and `None` is that colony being gone.
    pub fn notice(
        &mut self,
        notice: Notice,
        other: Option<&mut Connections>,
    ) -> Result<Status, Refusal> {
        let held = self
            .direct
            .get(&notice.id)
            .or_else(|| self.indirect.get(&notice.id))
            .cloned();
        let Some(held) = held else {
            return Err(Refusal::UnknownOrigin { id: notice.id });
        };
        let indirect = !self.direct.contains_key(&notice.id);
        // `:732`: the notice is recorded before anything else can fail.
        self.notices.insert(notice.id, notice.clone());
        let status = notice.kind.sets().unwrap_or(held.status);
        let updated = Link {
            status,
            ..held
        };
        if indirect {
            self.indirect.insert(notice.id, updated);
        } else {
            self.direct.insert(notice.id, updated);
        }
        let Some(other) = other else {
            return Err(Refusal::OriginUnreachable { id: notice.id });
        };
        let theirs = other
            .direct
            .get(&self.id)
            .or_else(|| other.indirect.get(&self.id))
            .cloned();
        let Some(theirs) = theirs else {
            return Err(Refusal::OriginUnreachable { id: notice.id });
        };
        // `:778-781`: the two diplomacy maps, then the origin's own record of us.
        self.diplomacy.insert(notice.id, status);
        other.diplomacy.insert(self.id, status);
        let record = Link {
            id: self.id,
            name: self.name.clone(),
            gate: theirs.gate,
            status,
        };
        if other.direct.contains_key(&self.id) {
            other.direct.insert(self.id, record);
        } else {
            other.indirect.insert(self.id, record);
        }
        Ok(status)
    }

    /// `serializeNBT` (`:671-720`): six lists, in Java's order.
    pub fn to_compound(&self) -> Compound {
        let mut nodes = Vec::with_capacity(self.nodes.len());
        for (pos, node) in &self.nodes {
            nodes.push(Compound::from([
                (KEY_POS.into(), nbt::position(*pos)),
                (KEY_PREV.into(), nbt::position(node.previous)),
                (KEY_NEXT.into(), nbt::position(node.next)),
                (KEY_TARGET_COLONY.into(), Tag::Int(node.target)),
            ]));
        }
        let mut colonies = Vec::with_capacity(self.direct.len());
        for link in self.direct.values() {
            colonies.push(link_to_compound(link));
        }
        let mut gates = Vec::with_capacity(self.gate_houses.len());
        for gate in &self.gate_houses {
            gates.push(Compound::from([(KEY_POS.into(), nbt::position(*gate))]));
        }
        let mut notices = Vec::with_capacity(self.notices.len());
        for notice in self.notices.values() {
            notices.push(Compound::from([
                (KEY_ID.into(), Tag::Int(notice.id)),
                (KEY_NAME.into(), Tag::String(notice.name.as_str().into())),
                (KEY_STATUS.into(), Tag::Int(notice.kind.ordinal())),
            ]));
        }
        let mut pending = Vec::with_capacity(self.pending.len());
        for (pos, entry) in &self.pending {
            pending.push(Compound::from([
                (KEY_POS.into(), nbt::position(*pos)),
                (KEY_PREV.into(), nbt::position(entry.node.previous)),
                (KEY_NEXT.into(), nbt::position(entry.node.next)),
                (KEY_TARGET_COLONY.into(), Tag::Int(entry.node.target)),
                (KEY_CONNECTION_TYPE.into(), Tag::Int(entry.kind.ordinal())),
            ]));
        }
        let mut diplomacy = Vec::with_capacity(self.diplomacy.len());
        for (id, status) in &self.diplomacy {
            diplomacy.push(Compound::from([
                (KEY_COLONY_ID.into(), Tag::Int(*id)),
                (KEY_STATUS.into(), Tag::Int(status.ordinal())),
            ]));
        }
        Compound::from([
            (KEY_CONNECTIONS.into(), nbt::compound_list(nodes)),
            (KEY_COLONIES.into(), nbt::compound_list(colonies)),
            (KEY_GATEHOUSES.into(), nbt::compound_list(gates)),
            (KEY_EVENTS.into(), nbt::compound_list(notices)),
            (KEY_PENDING.into(), nbt::compound_list(pending)),
            (KEY_DIPLOMACY.into(), nbt::compound_list(diplomacy)),
        ])
    }

    /// `deserializeNBT` (`:621-669`), in Java's read order, with the three ordinal reads
    /// refusing by name instead of throwing.
    ///
    /// **Java merges into whatever the manager already holds** — it clears only the gate
    /// list and the notice map (`:640`, `:647`) — so reading a save twice merges two
    /// colonies' roads. The port builds a fresh manager, which is the only reading that
    /// cannot double a node.
    pub fn read(id: i32, name: &str, data: &Compound) -> Result<Connections, Refusal> {
        let mut connections = Connections::new(id, name);
        for record in nbt::compounds(data, KEY_CONNECTIONS) {
            let pos = nbt::position_in(record, KEY_POS).unwrap_or(ZERO);
            let node = Node {
                previous: nbt::position_in(record, KEY_PREV).unwrap_or(ZERO),
                next: nbt::position_in(record, KEY_NEXT).unwrap_or(ZERO),
                // `getInt` of an absent key is 0, not `NO_TARGET` — fact 4.
                target: nbt::int(record, KEY_TARGET_COLONY).unwrap_or(0),
            };
            connections.nodes.insert(pos, node);
        }
        for record in nbt::compounds(data, KEY_COLONIES) {
            let link = link_from_compound(record)?;
            connections.direct.insert(link.id, link);
        }
        for record in nbt::compounds(data, KEY_GATEHOUSES) {
            connections
                .gate_houses
                .push(nbt::position_in(record, KEY_POS).unwrap_or(ZERO));
        }
        for record in nbt::compounds(data, KEY_EVENTS) {
            let kind = EventKind::from_ordinal(nbt::int(record, KEY_STATUS).unwrap_or(0))
                .map_err(Refusal::Ui14)?;
            let notice = Notice {
                id: nbt::int(record, KEY_ID).unwrap_or(0),
                name: nbt::text(record, KEY_NAME).unwrap_or_default(),
                kind,
            };
            connections.notices.insert(notice.id, notice);
        }
        for record in nbt::compounds(data, KEY_PENDING) {
            let pos = nbt::position_in(record, KEY_POS).unwrap_or(ZERO);
            let kind = PendingKind::from_ordinal(nbt::int(record, KEY_CONNECTION_TYPE).unwrap_or(0))?;
            let node = Node {
                previous: nbt::position_in(record, KEY_PREV).unwrap_or(ZERO),
                next: nbt::position_in(record, KEY_NEXT).unwrap_or(ZERO),
                target: nbt::int(record, KEY_TARGET_COLONY).unwrap_or(0),
            };
            connections.pending.insert(pos, Pending { node, kind });
        }
        for record in nbt::compounds(data, KEY_DIPLOMACY) {
            let id = nbt::int(record, KEY_COLONY_ID).unwrap_or(0);
            let status = Status::from_ordinal(nbt::int(record, KEY_STATUS).unwrap_or(0))
                .map_err(Refusal::Ui14)?;
            connections.diplomacy.insert(id, status);
        }
        Ok(connections)
    }
}

/// A node that points nowhere yet, which is what both constructors start from
/// (`ColonyConnectionNode.java:39-42`).
fn node_at(previous: [i32; 3], target: i32) -> Node {
    Node {
        previous,
        next: ZERO,
        target,
    }
}

/// The squared distance Java compares, in `i64`: `BlockPos.distSqr` squares three `int`
/// differences and adds them, which does not fit an `i32` on a 60-million-block world.
fn dist_sqr(a: [i32; 3], b: [i32; 3]) -> i64 {
    let dx = i64::from(a[0]) - i64::from(b[0]);
    let dy = i64::from(a[1]) - i64::from(b[1]);
    let dz = i64::from(a[2]) - i64::from(b[2]);
    dx * dx + dy * dy + dz * dz
}

fn link_to_compound(link: &Link) -> Compound {
    Compound::from([
        (KEY_ID.into(), Tag::Int(link.id)),
        (KEY_NAME.into(), Tag::String(link.name.as_str().into())),
        (KEY_POS.into(), nbt::position(link.gate)),
        (KEY_STATUS.into(), Tag::Int(link.status.ordinal())),
    ])
}

fn link_from_compound(record: &Compound) -> Result<Link, Refusal> {
    let status = Status::from_ordinal(nbt::int(record, KEY_STATUS).unwrap_or(0))
        .map_err(Refusal::Ui14)?;
    Ok(Link {
        id: nbt::int(record, KEY_ID).unwrap_or(0),
        name: nbt::text(record, KEY_NAME).unwrap_or_default(),
        gate: nbt::position_in(record, KEY_POS).unwrap_or(ZERO),
        status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A colony whose gate house is 210 blocks west, with `length` signs leading to it:
    /// node *k* sits at `x = 10k` and node 1's `previous` is the gate itself.
    ///
    /// The gate is deliberately **not** at the origin: a gate house within 50 blocks of a
    /// new sign takes the gate branch of `addNewConnectionNode` before any node is looked
    /// at (`:125-135`), which would hide the walk this chain is here to test.
    const GATE: [i32; 3] = [-200, 64, 0];

    fn with_chain(length: usize) -> Connections {
        let mut connections = Connections::new(1, "Maison");
        connections.gate_houses.push(GATE);
        // Sign k sits at x = 10 * k, and 10 blocks apart is inside the 50-block reach.
        let mut previous = GATE;
        for step in 1..=length {
            let at = [10 * step as i32, 64, 0];
            connections.nodes.insert(
                at,
                Node {
                    previous,
                    next: ZERO,
                    target: 2,
                },
            );
            if previous != GATE {
                if let Some(node) = connections.nodes.get_mut(&previous) {
                    node.next = at;
                }
            }
            previous = at;
        }
        connections
    }

    #[test]
    fn a_sign_links_to_the_gate_or_to_the_closest_open_node() {
        let mut connections = Connections::new(1, "Maison");
        assert_eq!(
            connections.attempt_add_node([5, 64, 0]),
            Err(Refusal::TooFar { at: [5, 64, 0] }),
            "nothing to attach to, and no gate house"
        );
        assert!(connections.pending().is_empty(), "a refusal writes nothing");
        assert!(connections.nodes().is_empty());

        // The gate stands at the same height as the sign: `BlockPos.distSqr` counts y, so a
        // gate 64 blocks below the sign is 64² further away (`ColonyConnectionManager.java:102`).
        connections.gate_houses.push([0, 64, 0]);
        let plan = connections
            .attempt_add_node([50, 64, 0])
            .expect("exactly 50 blocks from the gate is in reach");
        assert_eq!(plan.previous, [0, 64, 0]);
        assert_eq!(plan.target, NO_TARGET, "a gate house names no colony");
        assert_eq!(plan.kind, PendingKind::Default);
        assert_eq!(connections.pending().len(), 1);
        assert!(connections.nodes().is_empty(), "the node is written when the path ends");

        // And any pending entry refuses a second sign, anywhere (`:119-123`).
        assert_eq!(
            connections.attempt_add_node([500, 64, 0]),
            Err(Refusal::PathPending {
                pending: [50, 64, 0]
            })
        );
        assert_eq!(connections.pending().len(), 1);
        // A gate one block lower is out of reach of a sign this far away, which is why the
        // test above had to put the two at the same height.
        let mut lower = Connections::new(1, "Maison");
        lower.gate_houses.push([0, 63, 0]);
        assert_eq!(
            lower.attempt_add_node([50, 64, 0]),
            Err(Refusal::TooFar { at: [50, 64, 0] })
        );
    }

    #[test]
    fn the_walk_back_to_a_gate_stops_one_step_early_because_java_does() {
        // The sign of the module note: a chain of four refuses the fifth.
        let chain = with_chain(3);
        // From node 3: previous is 2, and `terminal` stops on 2, which is not a gate.
        assert_eq!(chain.terminal([30, 64, 0]), [20, 64, 0]);
        assert!(!chain.gate_houses().contains(&[20, 64, 0]));
        // From node 2 the walk lands on node 1 and stops there too.
        assert_eq!(chain.terminal([20, 64, 0]), [10, 64, 0]);
        // Only the first node's `previous` is the gate, so only that walk reaches it.
        assert_eq!(chain.terminal([10, 64, 0]), GATE);
        assert!(chain.gate_houses().contains(&GATE));
        // So a fourth sign next to the third is refused...
        let mut four = with_chain(3);
        assert_eq!(
            four.attempt_add_node([35, 64, 0]),
            Err(Refusal::ChainBroken {
                at: [35, 64, 0],
                terminal: [10, 64, 0]
            })
        );
        assert!(four.pending().is_empty(), "a refusal writes no pending entry");
        // ...while a third sign next to the second is accepted: the walk from node 2 lands
        // on node 1, and node 1's `previous` is the gate.
        let mut two = with_chain(2);
        let plan = two.attempt_add_node([25, 64, 0]).expect("one step to the gate");
        assert_eq!(plan.previous, [20, 64, 0]);
        assert_eq!(plan.target, 2, "the target colony travels with the chain");
    }

    #[test]
    fn a_default_sign_with_no_chain_is_refused_without_being_written() {
        let mut connections = Connections::new(1, "Maison");
        connections.gate_houses.push([0, 64, 0]);
        connections.attempt_add_node([-30, 64, 0]).expect("30 blocks from the gate");
        // The pending entry says it hangs off the gate; take the gate away and the same
        // resolution has no chain to attach to.
        connections.gate_houses.clear();
        let before = connections.nodes().clone();
        let resolved = connections
            .resolve([-30, 64, 0], true)
            .expect("the entry was pending");
        assert_eq!(
            resolved,
            Resolved::Refused {
                at: [-30, 64, 0],
                previous: [0, 64, 0]
            }
        );
        assert_eq!(connections.nodes(), &before, "no node before the check passes");
        assert!(connections.pending().is_empty(), "the entry is gone :222");
    }

    #[test]
    fn a_path_that_never_arrives_breaks_a_sign_and_keeps_a_repair() {
        let mut connections = Connections::new(1, "Maison");
        connections.gate_houses.push([0, 64, 0]);
        connections.attempt_add_node([20, 64, 0]).expect("in reach");
        assert_eq!(
            connections.resolve([20, 64, 0], false),
            Ok(Resolved::Failed {
                at: [20, 64, 0],
                previous: [0, 64, 0]
            })
        );
        assert!(connections.nodes().is_empty());

        // The same failure on a FIX_PATH keeps it pending — and therefore refuses every
        // new sign for ever (facts 2 and 3).
        connections.pending.insert(
            [40, 64, 0],
            Pending {
                node: node_at([30, 64, 0], NO_TARGET),
                kind: PendingKind::FixPath,
            },
        );
        assert_eq!(
            connections.resolve([40, 64, 0], false),
            Ok(Resolved::Waiting { at: [40, 64, 0] })
        );
        assert_eq!(connections.pending().len(), 1);
        assert_eq!(
            connections.attempt_add_node([41, 64, 0]),
            Err(Refusal::PathPending {
                pending: [40, 64, 0]
            })
        );
        assert_eq!(
            connections.resolve([99, 64, 0], true),
            Err(Refusal::NotPending { at: [99, 64, 0] })
        );
    }

    #[test]
    fn a_sign_in_the_middle_asks_for_the_repair_of_the_closest_orphan() {
        let mut connections = with_chain(2);
        // Node 1 lost its way home — the gate house that stood there was removed, and
        // `removeGateHouse` zeroes its `previous` (`:552-556`). That is the orphan the
        // repair looks for (`:289`).
        if let Some(node) = connections.nodes.get_mut(&[10, 64, 0]) {
            node.previous = ZERO;
        }
        // Node 3 arrives next to node 2.
        connections.pending.insert(
            [30, 64, 0],
            Pending {
                node: node_at([20, 64, 0], 2),
                kind: PendingKind::Default,
            },
        );
        let resolved = connections.resolve([30, 64, 0], true).expect("pending there");
        let Resolved::Linked {
            repair,
            kind,
            reciprocal,
            connect,
            ..
        } = resolved
        else {
            panic!("the path reached, it must link");
        };
        assert_eq!(kind, PendingKind::Default);
        assert!(reciprocal.is_none(), "a plain sign is not a gate house");
        assert!(connect.is_none());
        // Node 1 is the only one whose `previous` is zero, so the repair starts there and
        // leans on the sign that just arrived (`:299-314`).
        assert_eq!(
            repair,
            Some(Plan {
                at: [10, 64, 0],
                previous: [30, 64, 0],
                target: 2,
                kind: PendingKind::FixPath,
            })
        );
        assert_eq!(connections.pending().len(), 1, "the repair is pending");
        assert_eq!(connections.node([20, 64, 0]).map(|n| n.next), Some([30, 64, 0]));
    }

    #[test]
    fn two_colonies_link_only_when_both_chains_reach_a_gate() {
        let mut ours = with_chain(1);
        let mut theirs = Connections::new(2, "Voisine");
        theirs.gate_houses.push([500, 64, 0]);
        theirs.nodes.insert(
            [400, 64, 0],
            Node {
                previous: [500, 64, 0],
                next: ZERO,
                target: NO_TARGET,
            },
        );

        // 1. the target node is taken and is not a gate: refused, and neither side was
        // touched.
        if let Some(node) = theirs.nodes.get_mut(&[400, 64, 0]) {
            node.next = [999, 64, 0];
        }
        let before = (ours.direct().clone(), theirs.direct().clone());
        assert_eq!(
            ours.connect(Some(&mut theirs), [10, 64, 0], [400, 64, 0]),
            Err(Refusal::TargetBusy { at: [400, 64, 0] })
        );
        assert_eq!((ours.direct().clone(), theirs.direct().clone()), before);
        if let Some(node) = theirs.nodes.get_mut(&[400, 64, 0]) {
            node.next = ZERO;
        }

        // 2. our own chain has no gate house: refused before anything on the other side is
        // read.
        let gates = ours.gate_houses().to_vec();
        ours.gate_houses.clear();
        assert_eq!(
            ours.connect(Some(&mut theirs), [10, 64, 0], [400, 64, 0]),
            Err(Refusal::ChainBroken {
                at: [10, 64, 0],
                terminal: GATE
            })
        );
        ours.gate_houses = gates;

        // 3. every check passes: both maps hold the other colony, at the other's gate.
        let ours_link = ours
            .connect(Some(&mut theirs), [10, 64, 0], [400, 64, 0])
            .expect("both gates are reachable");
        assert_eq!(ours_link.id, 2);
        assert_eq!(ours_link.gate, [500, 64, 0], "the target's gate, :396");
        assert_eq!(ours_link.status, Status::Neutral);
        let theirs_link = theirs.direct().get(&1).expect("the other side was written");
        assert_eq!(theirs_link.gate, GATE, "our gate, :397");
        assert_eq!(theirs_link.name, "Maison");
        // The two middle nodes now point at each other.
        assert_eq!(ours.node([10, 64, 0]).map(|n| n.next), Some([400, 64, 0]));
        assert_eq!(theirs.node([400, 64, 0]).map(|n| n.next), Some([10, 64, 0]));

        // 4. a colony that does not exist is Java's null.
        assert_eq!(
            ours.connect(None, [10, 64, 0], [400, 64, 0]),
            Err(Refusal::NoSuchColony { colony: NO_TARGET })
        );
    }

    #[test]
    fn a_notice_is_registered_before_the_origin_side_can_refuse_it() {
        let mut ours = Connections::new(1, "Maison");
        let mut theirs = Connections::new(2, "Voisine");
        let notice = |kind| Notice {
            id: 2,
            name: "Voisine".to_owned(),
            kind,
        };
        // 1. an unknown origin writes nothing at all, not even the notice (:727-730).
        assert_eq!(
            ours.notice(notice(EventKind::FeudStarted), Some(&mut theirs)),
            Err(Refusal::UnknownOrigin { id: 2 })
        );
        assert!(ours.notices().is_empty());

        // 2. a known origin that does not know us back: the notice is recorded and the
        // diplomacy is not (:732 before :773).
        ours.direct.insert(
            2,
            Link {
                id: 2,
                name: "Voisine".to_owned(),
                gate: [100, 64, 0],
                status: Status::Neutral,
            },
        );
        assert_eq!(
            ours.notice(notice(EventKind::FeudStarted), Some(&mut theirs)),
            Err(Refusal::OriginUnreachable { id: 2 })
        );
        assert_eq!(ours.notices().get(&2).map(|n| n.kind), Some(EventKind::FeudStarted));
        assert_eq!(ours.declared(2), None, "no diplomacy was written");
        assert_eq!(ours.diplomacy_status(2), Status::Hostile, "the link itself changed");
        assert!(theirs.notices().is_empty());

        // 3. both sides know each other: the status lands on both, and ALLY_REQUEST
        // changes nothing.
        theirs.direct.insert(
            1,
            Link {
                id: 1,
                name: "Maison".to_owned(),
                gate: ZERO,
                status: Status::Neutral,
            },
        );
        assert_eq!(
            ours.notice(notice(EventKind::AllyRequest), Some(&mut theirs)),
            Ok(Status::Hostile),
            "ALLY_REQUEST falls into the default and keeps the current status"
        );
        assert_eq!(ours.declared(2), Some(Status::Hostile));
        assert_eq!(theirs.declared(1), Some(Status::Hostile));
        assert_eq!(theirs.diplomacy_status(1), Status::Hostile);
        assert_eq!(ours.notices().get(&2).map(|n| n.kind), Some(EventKind::AllyRequest));
    }

    #[test]
    fn an_ally_s_ally_becomes_indirect_at_the_ally_s_gate_and_an_unknown_one_is_forgotten() {
        let mut connections = Connections::new(1, "Maison");
        connections.direct.insert(
            2,
            Link {
                id: 2,
                name: "Voisine".to_owned(),
                gate: [100, 64, 0],
                status: Status::Allies,
            },
        );
        let mut view = BTreeMap::new();
        view.insert(
            2,
            Peer {
                name: "Voisine".to_owned(),
                direct: vec![
                    Link {
                        id: 3,
                        name: "Lointaine".to_owned(),
                        gate: [900, 64, 0],
                        status: Status::Neutral,
                    },
                    Link {
                        id: 1,
                        name: "Maison".to_owned(),
                        gate: ZERO,
                        status: Status::Allies,
                    },
                ],
            },
        );
        let changes = connections.update(false, &view);
        assert_eq!(changes, vec![Change::Indirect { id: 3 }]);
        let indirect = connections.indirect().get(&3).expect("the ally's ally");
        assert_eq!(indirect.gate, [100, 64, 0], "recorded at the ally's gate, :496");
        assert_eq!(indirect.name, "Lointaine");

        // A colony the registry no longer holds is removed on the next pass.
        let changes = connections.update(false, &BTreeMap::new());
        assert_eq!(changes, vec![Change::Forgotten { id: 2 }]);
        assert!(connections.direct().is_empty());
        // The indirect entry survives: no view was taken of that layer.
        assert!(connections.indirect().contains_key(&3));
    }

    #[test]
    fn the_save_keeps_six_lists_and_a_reloaded_target_is_zero_not_minus_one() {
        let mut connections = with_chain(1);
        connections.gate_houses.push(ZERO);
        connections.add_gate_house([300, 64, 0]);
        connections.direct.insert(
            2,
            Link {
                id: 2,
                name: "Voisine".to_owned(),
                gate: [300, 64, 0],
                status: Status::Allies,
            },
        );
        connections.diplomacy.insert(2, Status::Hostile);
        connections.notices.insert(
            2,
            Notice {
                id: 2,
                name: "Voisine".to_owned(),
                kind: EventKind::NeutralSet,
            },
        );
        connections.pending.insert(
            [400, 64, 0],
            Pending {
                node: node_at([300, 64, 0], 2),
                kind: PendingKind::ConnectColony,
            },
        );
        let data = connections.to_compound();
        assert_eq!(data.len(), 6);
        let read = Connections::read(1, "Maison", &data).expect("a save this port wrote");
        assert_eq!(read, connections, "the round trip is exact");

        // Fact 4: a node whose target key is absent reads as colony 0, not as -1.
        let bare = Compound::from([(
            KEY_CONNECTIONS.into(),
            Tag::List(vec![Tag::Compound(Compound::from([(
                KEY_POS.into(),
                nbt::position([1, 2, 3]),
            )]))]),
        )]);
        let reloaded = Connections::read(1, "Maison", &bare).expect("a node with no target");
        assert_eq!(reloaded.node([1, 2, 3]).map(|n| n.target), Some(0));
        assert_eq!(NO_TARGET, -1, "a fresh node is -1, ColonyConnectionNode.java:33");
    }

    #[test]
    fn an_ordinal_a_save_cannot_have_is_refused_by_name_in_all_three_places() {
        let bad_status = Compound::from([(
            KEY_COLONIES.into(),
            Tag::List(vec![Tag::Compound(Compound::from([
                (KEY_ID.into(), Tag::Int(2)),
                (KEY_STATUS.into(), Tag::Int(3)),
            ]))]),
        )]);
        let refused = Connections::read(1, "Maison", &bad_status);
        assert_eq!(refused, Err(Refusal::Ui14(Ui14::UnknownStatus { ordinal: 3 })));
        assert!(refused.unwrap_err().name().starts_with("ui14.refusal."));

        let bad_kind = Compound::from([(
            KEY_PENDING.into(),
            Tag::List(vec![Tag::Compound(Compound::from([(
                KEY_CONNECTION_TYPE.into(),
                Tag::Int(7),
            )]))]),
        )]);
        assert_eq!(
            Connections::read(1, "Maison", &bad_kind),
            Err(Refusal::UnknownConnectionType { ordinal: 7 })
        );
        assert_eq!(
            Refusal::UnknownConnectionType { ordinal: 7 }.to_string(),
            "manager_04.connections.refusal.unknown_connection_type"
        );
        // The three kinds keep the ordinals the save uses.
        for kind in [PendingKind::Default, PendingKind::FixPath, PendingKind::ConnectColony] {
            assert_eq!(PendingKind::from_ordinal(kind.ordinal()), Ok(kind));
        }
    }
}
