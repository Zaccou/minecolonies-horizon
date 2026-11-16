//! Inventories and crafting, the port's own view of them.
//!
//! Java reference: `com.minecolonies.api.inventory` and
//! `com.minecolonies.api.crafting`, plus the request side of
//! `com.minecolonies.core.colony.requestsystem` for what a building asks for. It
//! sits on the engine's containers and menus; it does not define a second slot
//! model.
//!
//! **The three pieces.** [Stack] is an item and a positive count, and knows
//! nothing else; [Inventory] is a working copy of one container's slots and every
//! move they allow; [CraftingRequest] is one thing a building wants, with the four
//! states this port can honour. [engine] is the only file that calls the host.
//!
//! **Where the slots are.** A hut's slots belong to the mod's own block entity:
//! `Registrar::block_container` declared their count and their menu (`src/hut.rs`),
//! and the engine answers them one by one through `BlockEntityRef::item_slot` and
//! writes them through `set_item_slot`
//! (`crates/horizon-mod-vocab/src/host/block_entities.rs:84-92`, `:173-182`).
//!
//! **What is not here.** No citizen inventory (`InventoryCitizen` has 27 slots,
//! held-item indices and every armour rule, `api/inventory/InventoryCitizen.java:30-58`),
//! no recipe storage (`IRecipeStorage`, 209 lines), no request resolver
//! (`RequestState` has thirteen states, `request/RequestState.java:13-78`). Each is
//! a layer above this one; `RAPPORT_PORT_INVENTORY.md` says what each would cost.
//!
//! **The one rule every method follows.** It returns what really moved. An insert
//! that fits three of eight returns `3`; an extract that finds nothing is a named
//! refusal. Nothing here reads the clock, the disk or the network — a wasm mod has
//! none of the three.

pub mod engine;
mod request;
mod slots;
mod stack;

pub use engine::{HostLimits, WriteBackError};
pub use request::{CraftingRequest, Quality, RequestState};
pub use slots::Inventory;
pub use stack::{ItemLimits, ItemName, SingleLimit, SlotLimits, Stack};

/// Why this layer refused a move, named.
///
/// The brief's four come first; [`Self::QualityTooLow`] is the fifth, because a
/// request with a quality floor needs a word for missing it rather than using
/// "invalid state" for two unrelated things.
///
/// A variant is **not** a `lang` key: those live in `assets/minecolonies/lang/`,
/// which this module does not own, and a refusal that never reaches a player (a
/// full inventory inside a tick) needs none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// No slot took any of it: the inventory is full, or no slot accepts it.
    InventoryFull,
    /// There is none of this item here — also what a registry that does not know
    /// the item answers, since both mean "no slot holds it".
    ItemAbsent,
    /// A zero count was asked for or offered. A stack cannot hold one, so it is
    /// always a caller's mistake (`item/item_stack.rs:59-67`: an empty slot is
    /// `None`, not a zero-count item).
    EmptyQuantity,
    /// The delivered item is below the request's quality floor.
    QualityTooLow,
    /// The request's state forbids the move, or the block entity is no longer what
    /// the view describes.
    InvalidState,
}

impl Refusal {
    /// A stable name for logs and tests, namespaced like a registry entry so it
    /// can never be mistaken for a player-facing sentence.
    pub fn name(self) -> &'static str {
        match self {
            Self::InventoryFull => "inventory.refusal.full",
            Self::ItemAbsent => "inventory.refusal.absent",
            Self::EmptyQuantity => "inventory.refusal.empty_quantity",
            Self::QualityTooLow => "inventory.refusal.quality",
            Self::InvalidState => "inventory.refusal.state",
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
            Refusal::InventoryFull,
            Refusal::ItemAbsent,
            Refusal::EmptyQuantity,
            Refusal::QualityTooLow,
            Refusal::InvalidState,
        ];
        for (index, refusal) in all.iter().enumerate() {
            for other in &all[index + 1..] {
                assert_ne!(
                    refusal.name(),
                    other.name(),
                    "two refusals share a name, so a log line cannot tell them apart"
                );
            }
            assert!(refusal.name().starts_with("inventory.refusal."));
        }
    }
}
