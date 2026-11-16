//! The working copy of a building's slots, and every move it makes.
//!
//! A hut's container declares a slot count and a menu (`src/hut.rs`); the engine
//! holds what those slots contain and answers them one at a time
//! (`crates/horizon-mod-vocab/src/host/block_entities.rs:84-92`). Reading one slot
//! per host call would make every count a dozen round trips and every failed write
//! a rollback, so this type holds them **once**, answers the questions MineColonies
//! asks of a building, and hands the differences back to
//! [super::engine::write_back].
//!
//! **The order is vanilla's.** `ResourceHandler.insert` tops up "the slots that
//! hold the same item and components first, then the first empty ones
//! (`SimpleContainer.addItem`'s order), no slot past its limit"
//! (`crates/horizon-mod-vocab/src/host/items.rs:288-302`), and MineColonies'
//! `insertItem` does the same (`InventoryCitizen.java:417` and `:448`). Two passes,
//! in that order, and a test that fails if they are swapped.

use crate::inventory::{
    Refusal,
    stack::{ItemName, SlotLimits, Stack},
};

/// The slots of one building, read once and written back once.
pub struct Inventory<'a> {
    slots: Vec<Option<Stack>>,
    limits: &'a dyn SlotLimits,
}

impl<'a> Inventory<'a> {
    /// `slots` empty slots, with the limits every move is checked against.
    pub fn new(slots: usize, limits: &'a dyn SlotLimits) -> Self {
        Self {
            slots: vec![None; slots],
            limits,
        }
    }

    /// A view over slots that were just read, empty ones included.
    pub fn with_slots(slots: Vec<Option<Stack>>, limits: &'a dyn SlotLimits) -> Self {
        Self { slots, limits }
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(Option::is_none)
    }

    /// No slot is free. Whether a **given** item still fits is [`Self::insert`]'s
    /// answer, not this one: a slot holding 3 of 64 is full for nothing.
    pub fn is_full(&self) -> bool {
        self.slots.iter().all(Option::is_some)
    }

    pub fn slots(&self) -> &[Option<Stack>] {
        &self.slots
    }

    pub fn slot(&self, index: usize) -> Option<&Stack> {
        self.slots.get(index)?.as_ref()
    }

    /// Put `stack` in slot `index`, or empty it with `None`. `false` for an index
    /// past the end, and for a stack above the item's limit — the same refusal
    /// the engine gives as `BlockEntityItemError::StackTooLarge`
    /// (`crates/horizon-mod-vocab/src/block/block_entity_items.rs:22-27`).
    pub fn set_slot(&mut self, index: usize, stack: Option<Stack>) -> bool {
        if index >= self.slots.len() {
            return false;
        }
        if let Some(incoming) = &stack {
            match self.limits.max_count(incoming.item()) {
                Some(cap) if incoming.count() <= cap => {}
                _ => return false,
            }
        }
        self.slots[index] = stack;
        true
    }

    /// How many of `item` the whole inventory holds, across every slot.
    pub fn count(&self, item: &ItemName) -> u32 {
        self.slots
            .iter()
            .flatten()
            .filter(|held| held.item() == item)
            .map(Stack::count)
            .sum()
    }

    /// The first slot holding `item`, in slot order: the "bring me the one that
    /// has some" of every MineColonies worker.
    pub fn first_slot_of(&self, item: &ItemName) -> Option<usize> {
        self.slots
            .iter()
            .position(|slot| slot.as_ref().is_some_and(|held| held.item() == item))
    }

    /// Store as much of `stack` as fits, and say how much that was.
    ///
    /// `Ok(moved)` with `moved < stack.count()` means the rest did **not** go in:
    /// the caller still holds its own `stack` (this takes a reference precisely so
    /// it can), and decides whether to drop it, hand it to another building, or
    /// leave the request open. `Err(InventoryFull)` means nothing moved at all;
    /// `Err(ItemAbsent)` means the registry does not know the item, so no slot
    /// takes it.
    pub fn insert(&mut self, stack: &Stack) -> Result<u32, Refusal> {
        let Some(cap) = self.limits.max_count(stack.item()) else {
            return Err(Refusal::ItemAbsent);
        };
        if cap == 0 {
            return Err(Refusal::InventoryFull);
        }
        let mut moving = Some(stack.clone());
        let mut moved = 0u32;

        // 1. The slots already holding this item, front to back.
        for slot in self.slots.iter_mut() {
            let Some(incoming) = moving.take() else {
                break;
            };
            let Some(held) = slot.as_mut() else {
                moving = Some(incoming);
                continue;
            };
            if !held.stackable_with(&incoming) {
                moving = Some(incoming);
                continue;
            }
            let (now, rest) = held.merge(incoming, cap);
            moved += now;
            moving = rest;
        }

        // 2. Then the empty ones, front to back.
        for slot in self.slots.iter_mut() {
            let Some(incoming) = moving.take() else {
                break;
            };
            if slot.is_some() {
                moving = Some(incoming);
                continue;
            }
            let take = incoming.count().min(cap);
            let (portion, rest) = incoming.split(take);
            moved += portion.as_ref().map_or(0, Stack::count);
            *slot = portion;
            moving = rest;
        }

        if moved == 0 {
            Err(Refusal::InventoryFull)
        } else {
            Ok(moved)
        }
    }

    /// Take up to `max` of `item`, front to back, and give back what came out —
    /// fewer than `max` when the inventory holds fewer, never an empty stack.
    ///
    /// `Err(EmptyQuantity)` for `max == 0`, `Err(ItemAbsent)` when there is none
    /// of it: the two are different bugs for the caller, so they stay different
    /// refusals.
    pub fn extract(&mut self, item: &ItemName, max: u32) -> Result<Stack, Refusal> {
        if max == 0 {
            return Err(Refusal::EmptyQuantity);
        }
        let mut taken = 0u32;
        for slot in self.slots.iter_mut() {
            if taken >= max {
                break;
            }
            let want = match slot.as_ref() {
                Some(held) if held.item() == item => (max - taken).min(held.count()),
                _ => continue,
            };
            let held = slot.take().expect("the slot was just read as holding");
            let (portion, rest) = held.split(want);
            taken += portion.as_ref().map_or(0, Stack::count);
            *slot = rest;
        }
        if taken == 0 {
            return Err(Refusal::ItemAbsent);
        }
        Ok(Stack::new(item.clone(), taken).expect("a nonzero take is a stack"))
    }

    /// Empty every slot, in slot order, and give back what was in them. What a
    /// hut does when it is broken, and what an upgrade reads before it rewrites
    /// the container (`InventoryCitizen.java:592-670` does the same for a
    /// citizen, from a compound).
    pub fn clear(&mut self) -> Vec<Stack> {
        self.slots.iter_mut().filter_map(Option::take).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::stack::{ItemLimits, SingleLimit};

    fn planks() -> ItemName {
        ItemName::new("minecraft:oak_planks").expect("a valid name")
    }

    fn stone() -> ItemName {
        ItemName::new("minecraft:stone").expect("a valid name")
    }

    fn stack(item: ItemName, count: u32) -> Stack {
        Stack::new(item, count).expect("a nonzero stack")
    }

    fn limits() -> ItemLimits {
        ItemLimits::new([
            (planks(), 64),
            (stone(), 64),
            (ItemName::new("minecraft:egg").expect("a valid name"), 16),
        ])
    }

    #[test]
    fn an_insert_tops_up_a_matching_slot_before_taking_an_empty_one() {
        let limits = limits();
        let mut inventory = Inventory::new(3, &limits);
        inventory.set_slot(0, Some(stack(planks(), 60)));
        let moved = inventory
            .insert(&stack(planks(), 4))
            .expect("a partial slot takes four");
        assert_eq!(moved, 4);
        assert_eq!(
            inventory.slot(0).map(Stack::count),
            Some(64),
            "the four went into the slot that already held planks"
        );
        assert_eq!(inventory.slot(1), None, "and not into the first empty slot");
    }

    #[test]
    fn an_insert_stops_at_the_cap_and_spills_into_the_next_slot() {
        let limits = limits();
        let mut inventory = Inventory::new(3, &limits);
        let moved = inventory
            .insert(&stack(planks(), 80))
            .expect("two empty slots take 128");
        assert_eq!(moved, 80);
        assert_eq!(inventory.slot(0).map(Stack::count), Some(64));
        assert_eq!(inventory.slot(1).map(Stack::count), Some(16));
        assert_eq!(inventory.slot(2), None, "nothing spills further than it must");
    }

    #[test]
    fn an_insert_reports_only_what_fitted_and_never_invents_room() {
        let limits = SingleLimit(10);
        let mut inventory = Inventory::new(1, &limits);
        let moved = inventory
            .insert(&stack(planks(), 7))
            .expect("the empty slot takes seven");
        assert_eq!(moved, 7);
        // Three more fit; the other seven of this stack do not.
        let moved = inventory
            .insert(&stack(planks(), 10))
            .expect("the slot has three left");
        assert_eq!(moved, 3, "the slot had three of its ten left, not ten");
        assert_eq!(inventory.count(&planks()), 10);
        assert_eq!(
            inventory.insert(&stack(planks(), 1)),
            Err(Refusal::InventoryFull)
        );
    }

    #[test]
    fn a_full_inventory_refuses_rather_than_reporting_a_partial_move() {
        let limits = limits();
        let mut inventory = Inventory::new(2, &limits);
        inventory.set_slot(0, Some(stack(planks(), 64)));
        inventory.set_slot(1, Some(stack(stone(), 64)));
        assert!(inventory.is_full());
        assert_eq!(
            inventory.insert(&stack(planks(), 1)),
            Err(Refusal::InventoryFull),
            "nothing moved, so it is a refusal and not Ok(0)"
        );
        assert_eq!(inventory.count(&planks()), 64);
    }

    #[test]
    fn an_item_the_limits_do_not_know_goes_nowhere() {
        let limits = ItemLimits::one(planks(), 64);
        let mut inventory = Inventory::new(4, &limits);
        assert_eq!(
            inventory.insert(&stack(stone(), 5)),
            Err(Refusal::ItemAbsent),
            "no slot takes an item the registry does not know"
        );
        assert_eq!(inventory.count(&stone()), 0);
        assert!(!inventory.is_full(), "and it did not fill anything");
    }

    #[test]
    fn an_extract_takes_in_slot_order_and_stops_at_the_number_asked() {
        let limits = limits();
        let mut inventory = Inventory::new(3, &limits);
        inventory.set_slot(0, Some(stack(planks(), 5)));
        inventory.set_slot(1, Some(stack(planks(), 50)));
        inventory.set_slot(2, Some(stack(planks(), 9)));

        let taken = inventory.extract(&planks(), 7).expect("there are 64");
        assert_eq!(taken.count(), 7);
        assert_eq!(inventory.slot(0), None, "the first slot gave all five");
        assert_eq!(inventory.slot(1).map(Stack::count), Some(48));
        assert_eq!(inventory.slot(2).map(Stack::count), Some(9), "untouched");
    }

    #[test]
    fn an_extract_asking_for_more_than_there_is_gives_what_there_is() {
        let limits = limits();
        let mut inventory = Inventory::new(2, &limits);
        inventory.set_slot(1, Some(stack(planks(), 3)));
        let taken = inventory.extract(&planks(), 64).expect("three are there");
        assert_eq!(taken.count(), 3);
        assert_eq!(inventory.count(&planks()), 0, "and the inventory is emptied");
    }

    #[test]
    fn extracting_nothing_of_it_is_a_named_refusal() {
        let limits = limits();
        let mut inventory = Inventory::new(2, &limits);
        assert_eq!(
            inventory.extract(&planks(), 1),
            Err(Refusal::ItemAbsent)
        );
        assert_eq!(
            inventory.extract(&planks(), 0),
            Err(Refusal::EmptyQuantity),
            "zero asked is a caller bug, not an empty inventory"
        );
    }

    #[test]
    fn counting_and_finding_add_up_across_slots() {
        let limits = limits();
        let mut inventory = Inventory::new(4, &limits);
        inventory.set_slot(1, Some(stack(planks(), 10)));
        inventory.set_slot(3, Some(stack(planks(), 5)));
        inventory.set_slot(2, Some(stack(stone(), 7)));
        assert_eq!(inventory.count(&planks()), 15);
        assert_eq!(inventory.count(&stone()), 7);
        assert_eq!(inventory.first_slot_of(&planks()), Some(1));
        assert_eq!(inventory.first_slot_of(&stone()), Some(2));
        assert_eq!(
            inventory.first_slot_of(&ItemName::new("minecraft:egg").expect("a valid name")),
            None
        );
    }

    #[test]
    fn clearing_returns_everything_it_took_and_leaves_nothing_behind() {
        let limits = limits();
        let mut inventory = Inventory::new(3, &limits);
        inventory.set_slot(0, Some(stack(planks(), 2)));
        inventory.set_slot(2, Some(stack(stone(), 3)));
        let removed = inventory.clear();
        assert_eq!(removed.len(), 2, "one entry per occupied slot");
        assert_eq!(removed[0].item(), &planks(), "in slot order");
        assert_eq!(removed[1].item(), &stone());
        assert!(inventory.is_empty());
        assert_eq!(inventory.clear(), Vec::new(), "clearing twice takes nothing");
    }

    #[test]
    fn a_slot_write_refuses_an_index_past_the_end_and_a_stack_over_the_cap() {
        let limits = ItemLimits::one(planks(), 16);
        let mut inventory = Inventory::new(2, &limits);
        assert!(!inventory.set_slot(2, Some(stack(planks(), 1))));
        assert!(!inventory.set_slot(0, Some(stack(planks(), 17))));
        assert!(!inventory.set_slot(0, Some(stack(stone(), 1))));
        assert!(inventory.set_slot(0, Some(stack(planks(), 16))));
        assert!(inventory.set_slot(0, None), "clearing a slot is always allowed");
        assert_eq!(inventory.slot(0), None);
    }
}
