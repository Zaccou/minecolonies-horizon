//! The only file that talks to the engine: read a building's slots, write them
//! back, and answer a per-item limit.
//!
//! **What the engine gives.** A mod block entity's item slots, by index:
//! `BlockEntityRef::item_slot(host, slot)` reads one "without automation policy"
//! (`crates/horizon-mod-vocab/src/host/block_entities.rs:84-92`, forwarded at
//! `generated/mod_api_generated_host.rs:75`) and
//! `BlockEntityRef::set_item_slot(host, tx, slot, stack)` writes one inside a
//! transaction (`host/block_entities.rs:173-182`, forwarded at
//! `generated/mod_api_generated_host.rs:117`). Both answer
//! `BlockEntityItemError`, whose `SlotOutOfBounds { slot, slots }` names the
//! container's declared count and whose `StackTooLarge { count, limit }` names the
//! limit the engine would accept (`crates/horizon-mod-vocab/src/block/block_entity_items.rs:5-33`).
//!
//! **Why a transaction.** A write is one slot at a time, so a building's state is
//! only coherent if the whole write-back either lands or does not.
//! `HostTransaction::transaction` commits on `Ok` and puts every storage it wrote
//! back on `Err`, including on a panic
//! (`crates/horizon-mod-vocab/src/item/transaction.rs:131-165`).
//!
//! **What the engine does not give, and this file therefore does not do.**
//!
//! - **The slot count.** `ModContainer` declares it (`slots`, `menu`,
//!   `item/containers.rs:200-215`) and the engine enforces it, but no read answers
//!   it: `item_slot` only names how many there are when it refuses one too many.
//!   The caller passes the count it declared — `crate::hut::SLOTS` for this hut.
//! - **An item's quality tier.** MineColonies reads a food item's tier through
//!   `instanceof IMinecoloniesFoodItem` (`api/util/FoodUtils.java:126-128`), which
//!   has no equivalent here: `item_default_component` reads a *component*, not a
//!   Java type. So [super::CraftingRequest] takes the tier as an argument.
//! - **Resizing a container.** `container_set_stack_limit` exists for a vanilla
//!   container (`host/items.rs:392-394`); a mod container's slot count is fixed at
//!   registration, so a hut's level does not change how many slots it has.
//!
//! **A slot the port cannot name is left alone.** If a slot holds a stack whose id
//! does not parse as an [ItemName], the write-back skips it instead of clearing it:
//! an inventory layer that silently deleted what it could not read would be worse
//! than one that reports fewer writes than it made.

use crate::inventory::{
    slots::Inventory,
    stack::{ItemName, SlotLimits, Stack},
};
use mod_api::{
    BlockEntityItemError, BlockEntityRef, Host, HostTransaction, Identifier, ItemStack, ServerHost,
};

/// [SlotLimits] answered by the engine: `Item.getDefaultMaxStackSize`, as the game
/// reports it for a resolved item key
/// (`crates/horizon-mod-vocab/src/host/items.rs:8-12`).
///
/// Two resolutions per answer — name to key (`host/items.rs:19-22`), key to limit —
/// and `None` for either, which is the registry saying it has no such item.
pub struct HostLimits<'a> {
    host: &'a dyn Host,
}

impl<'a> HostLimits<'a> {
    pub fn new(host: &'a dyn Host) -> Self {
        Self { host }
    }
}

impl SlotLimits for HostLimits<'_> {
    fn max_count(&self, item: &ItemName) -> Option<u32> {
        let id = Identifier::parse(item.as_str()).ok()?;
        let key = self.host.item_key(&id)?;
        self.host.max_stack_size(key)
    }
}

/// Read `slots` slots of a block entity's container into a plain slot vector, in
/// slot order, empty ones as `None`.
///
/// The vector is what [`Inventory::with_slots`] takes: reading does not check a
/// limit (nothing moves), so it needs no [SlotLimits] and no engine metadata.
pub fn read_slots(
    host: &dyn Host,
    entity: BlockEntityRef,
    slots: u32,
) -> Result<Vec<Option<Stack>>, BlockEntityItemError> {
    let mut view = Vec::with_capacity(slots as usize);
    for index in 0..slots {
        let held = entity.item_slot(host, index)?;
        view.push(held.as_ref().and_then(|stack| view_of(stack, host)));
    }
    Ok(view)
}

/// Write the inventory back, slot by slot, in one transaction, and say how many
/// slots actually changed.
///
/// Only slots whose contents differ from what the engine holds are written, so a
/// view that was read and not touched writes nothing and moves no revision. The
/// whole write is one transaction: a refusal anywhere aborts all of it.
pub fn write_back(
    host: &mut dyn ServerHost,
    entity: BlockEntityRef,
    inventory: &Inventory<'_>,
) -> Result<u32, WriteBackError> {
    let slots = u32::try_from(inventory.len()).unwrap_or(u32::MAX);
    host.transaction(|host, tx| {
        let mut written = 0u32;
        for index in 0..slots {
            let held = entity.item_slot(&*host, index)?;
            let current = held.as_ref().and_then(|stack| view_of(stack, &*host));
            if held.is_some() && current.is_none() {
                continue;
            }
            let wanted = inventory.slot(index as usize).cloned();
            if current == wanted {
                continue;
            }
            let replacement = match &wanted {
                None => None,
                Some(stack) => Some(
                    engine_stack(stack, &*host)
                        .ok_or_else(|| WriteBackError::Unnameable(stack.item().clone()))?,
                ),
            };
            entity.set_item_slot(&mut *host, tx, index, replacement)?;
            written += 1;
        }
        Ok(written)
    })
}

/// The port's view of one engine stack: its canonical id and its count.
///
/// `None` only where the id does not name an item this crate can hold or the count
/// is zero; an engine `ItemStack` is never zero-count (`item_stack.rs:46-52`), so
/// in practice this is the identity, and the `Option` is there so the write-back
/// can refuse to guess.
fn view_of(stack: &ItemStack, host: &dyn Host) -> Option<Stack> {
    let name = ItemName::new(&stack.item().id(host))?;
    Stack::new(name, stack.count()).ok()
}

/// The engine's stack for one of the port's: resolve the name to a key, then build
/// a stack of that count. `None` when the registry has no such item, which is the
/// refusal `set_item_slot` would give as `InvalidStack`.
fn engine_stack(stack: &Stack, host: &dyn Host) -> Option<ItemStack> {
    let id = Identifier::parse(stack.item().as_str()).ok()?;
    let key = host.item_key(&id)?;
    ItemStack::new(key, stack.count())
}

/// Why a write-back stopped. The engine's own refusal is kept as it is rather than
/// renamed: a caller that wants to retry a stale handle and give up on a bad item
/// needs the distinction, and this layer has no reason to flatten it.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own set, closed on purpose
pub enum WriteBackError {
    /// The read or the write was refused; the transaction was aborted.
    Engine(BlockEntityItemError),
    /// A slot of the view names an item the registry does not know.
    Unnameable(ItemName),
}

impl From<BlockEntityItemError> for WriteBackError {
    fn from(error: BlockEntityItemError) -> Self {
        Self::Engine(error)
    }
}

impl std::fmt::Display for WriteBackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Engine(error) => write!(f, "the engine refused the write: {error:?}"),
            Self::Unnameable(item) => write!(f, "the registry does not know {item}"),
        }
    }
}

impl std::error::Error for WriteBackError {}
