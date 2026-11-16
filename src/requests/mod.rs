//! The request system: what a building asks for, who supplies it, and in what
//! order the colony serves it.
//!
//! Java reference: `com.minecolonies.api.colony.requestsystem` and
//! `com.minecolonies.core.colony.requestsystem` — the requestable (`Stack`), the
//! thirteen `RequestState`s, the resolvers with their priorities and suitability
//! metrics, and the manager that runs the assignment loop.
//!
//! | File | Question |
//! | --- | --- |
//! | `request` | what a request *is*, the five states it can be in, and its compound |
//! | `resolver` | who supplies it: the choice among candidates, as a pure function |
//! | `queue` | in what order it is served, and how an old request stops starving |
//! | `nbt` | the tag reads and writes the two compounds share |
//!
//! **Data and rules, no host, no clock, no thread.** Like `src/colony/` and
//! `src/inventory/`, this module makes no game call: a resolver's candidate is an
//! [`Inventory`](crate::inventory::Inventory) the caller already read, and a
//! distance is arithmetic on two positions. That is what lets every rule here be
//! tested on the host target with no wasm, no prepared resources and no game.
//!
//! **What is deliberately not here**, each with the reason it cannot be: the
//! **manager** (tokens, the nine data stores, the update handler) because it owns
//! the maps and the ticks; the **network** (`StandardPlayerRequestResolver` sends a
//! packet so a player fetches the item, `DeliverymenRequestResolver:96-113` needs a
//! courier to walk it over) because both need the host; **request chains**
//! (`AbstractRequest.java:142-153`, `RequestHandler.java:248-260`) because a parent
//! is a second entry in the manager's map; **recipes**
//! (`PublicWorkerCraftingRequestResolver` over a 209-line `IRecipeStorage`) because
//! a crafting tree is a module of its own — the crafting resolver appears here only
//! as a candidate whose metric is a distance; and the **thirteen states**, whose
//! saved ordinal no five-value enum can honour.
//!
//! **The two models of "what is asked" in this crate.** `inventory::CraftingRequest`
//! is a hut's own need, with a quality floor and four states; [`Request`] is the
//! colony's request, with a requester, a priority and five states. They overlap on
//! purpose for now — merging them is a decision for the layer that assigns, and
//! `RAPPORT_PORT_REQUESTS.md` §6 says so.

mod nbt;
mod queue;
mod request;
mod resolver;

pub use queue::Queue;
pub use request::{MAX_BUILDING_PRIORITY, Request, Requester, State};
pub use resolver::{Candidate, Source, choose, distance, eligible, metric};

/// Why the request system refused an operation, named.
///
/// [`Self::IllegalTransition`] is the one that carries the rule: it says which
/// state the request was in and which one the caller asked for, so a log line names
/// the move that did not happen rather than "invalid state".
///
/// A variant is not a `lang` key: those live in `assets/minecolonies/lang/`, which
/// this module does not own. Six variants for six different bugs — a caller that
/// cannot tell "already taken" from "finished" retries the wrong thing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// The request cannot make this move from where it stands.
    IllegalTransition { from: State, to: State },
    /// Another supplier already holds the request.
    AlreadyAssigned { by: [i32; 3] },
    /// The stack handed in is not the item the request asks for.
    WrongItem,
    /// A count of zero: a request or a delivery of nothing is not a thing.
    EmptyQuantity,
    /// A request with this id is already in the queue; serving it twice would
    /// deliver twice.
    Duplicate { id: i32 },
    /// No request in the queue has this id.
    Unknown { id: i32 },
}

impl Refusal {
    /// A stable name for logs and tests, namespaced like a registry entry so it can
    /// never be mistaken for a player-facing sentence.
    pub fn name(self) -> &'static str {
        match self {
            Self::IllegalTransition { .. } => "requests.refusal.transition",
            Self::AlreadyAssigned { .. } => "requests.refusal.assigned",
            Self::WrongItem => "requests.refusal.item",
            Self::EmptyQuantity => "requests.refusal.empty_quantity",
            Self::Duplicate { .. } => "requests.refusal.duplicate",
            Self::Unknown { .. } => "requests.refusal.unknown",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_refusal_has_its_own_name() {
        let all = [
            Refusal::IllegalTransition { from: State::Created, to: State::Delivered },
            Refusal::AlreadyAssigned { by: [0, 0, 0] },
            Refusal::WrongItem,
            Refusal::EmptyQuantity,
            Refusal::Duplicate { id: 1 },
            Refusal::Unknown { id: 1 },
        ];
        for (index, refusal) in all.iter().enumerate() {
            for other in &all[index + 1..] {
                assert_ne!(refusal.name(), other.name(), "two refusals share a name");
            }
            assert!(refusal.name().starts_with("requests.refusal."));
        }
        assert_eq!(Refusal::WrongItem.to_string(), "requests.refusal.item");
    }
}
