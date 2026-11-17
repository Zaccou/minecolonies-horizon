//! `SortingUtils`: the order a warehouse sorts into, and how many slots that takes.
//!
//! Java reference: `core/util/SortingUtils.java` (193 lines, four private
//! methods and one public one). `sort(CombinedItemHandler)` empties an inventory
//! slot by slot, merges the exact stacks into a `HashMap`, sorts the entries by
//! `(creative tab, item id, damage)` and pushes them back with a slot-skipping
//! heuristic. The *inventory* half is the host's (`getLastIndex`, `insertItem`,
//! `serializeNBT` for the rollback); the *order* and the *count* are decisions,
//! and they are the two things a warehouse has to get right.
//!
//! So this file is three functions and one comparison:
//!
//! | Rule | Java |
//! | --- | --- |
//! | merge two stacks of the same item | `:46-62` (`HashMap` accumulation) |
//! | how many slots an amount needs | `:186-189` (`Math.ceil(amount / maxStack)`) |
//! | what order the entries go in | `:143-161` (`compare`) |
//! | the total, and the per-tab subtotals | `:180-192` (`calcRequiredSlots`) |
//!
//! # What is not ported, and why it cannot be
//!
//! `pushIntoInv` (`:90-134`) walks the inventory with an `AtomicInteger` slot
//! cursor, retries an insertion five times ("`Could not insert full stack into
//! slot`", `:108-117`) and then *jumps* the cursor to the last index of the
//! current tab when two ratios say the tab is worth abandoning (`:123-133`). Both
//! ratios are made of `inv.getLastIndex(...)` and the running slot count — with no
//! inventory, the loop has nothing to read and nothing to write. What survives is
//! its bookkeeping, which [`slots_per_tab`] already answers: the heuristic exists
//! to know *when a tab is finished*, and that is `slots_per_tab[tab] <= 0`.
//!
//! # Two numbers the caller supplies, and why they are numbers
//!
//! `compare` sorts by the **creative tab** of a stack
//! (`IColonyManager.getInstance().getCompatibilityManager().getCreativeTabKey(entry)`)
//! and then by the **integer registry id** of its item
//! (`((ForgeRegistry<Item>) ForgeRegistries.ITEMS).getID(item)`). Neither is a
//! name: a tab is a server-side grouping, and an id is Forge's registration order,
//! which decides the visual order of the warehouse and is what makes sorting a
//! warehouse look stable across sessions. Both arrive as fields, so this file
//! neither invents them nor pretends a string sort would do.

use std::cmp::Ordering;
use std::collections::BTreeMap;

/// One exact stack after the merge: what Java's `ExactMatchItemStorage` keys on,
/// plus the three numbers `compare` reads.
///
/// The identity of the stack itself (item, damage value, NBT) is **not** here:
/// it is the caller's map key, because an NBT payload has no hash this file could
/// compute without assuming one. [`group`] takes whatever key the caller already
/// compares exactly.
// exhaustive: the five fields `compare` and `calcRequiredSlots` read
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SortEntry {
    /// How many of the item the merge accumulated (`:58-62`).
    pub amount: u32,
    /// `stack.getMaxStackSize()` (`:186`): **never** `0`, see [`required_slots`].
    pub max_stack: u32,
    /// `getCreativeTabKey(entry)` (`:145-146`): the group the entry belongs to.
    pub creative_tab: i32,
    /// `getId(entry.getItem())` (`:153-159`): Forge's registry id of the item.
    pub item_id: i32,
    /// `entry.getDamageValue()` (`:158`): the last tie-break.
    pub damage: i32,
}

/// The merge half of `sort`: accumulate an amount per exact key
/// (`SortingUtils.java:46-62`).
///
/// ```java
/// final ExactMatchItemStorage storage = new ExactMatchItemStorage(inv.getStackInSlot(i));
/// int amount = storage.getAmount();
/// if (map.containsKey(storage)) { amount += map.remove(storage); }
/// map.put(storage, amount);
/// ```
///
/// Two stacks of the same key — same item, same damage, same NBT — become one
/// entry carrying both counts. A `BTreeMap` rather than a `HashMap` so that two
/// sorts of the same warehouse produce the **same** order before [`compare`] runs;
/// Java's `HashMap` does not, and its sort only looks stable because `compare`
/// breaks every tie it can.
///
/// An empty stack contributes nothing: Java skips them before this point
/// (`:51-54`), so a `0` amount never reaches the map.
pub fn group<K: Ord>(stacks: impl IntoIterator<Item = (K, u32)>) -> BTreeMap<K, u32> {
    let mut merged: BTreeMap<K, u32> = BTreeMap::new();
    for (key, amount) in stacks {
        if amount == 0 {
            continue;
        }
        *merged.entry(key).or_insert(0) += amount;
    }
    merged
}

/// How many slots `amount` of a stack that holds `max_stack` takes:
/// `Math.ceil((double) entry.getValue() / entry.getKey().getItemStack().getMaxStackSize())`
/// (`SortingUtils.java:186`, and again at `:188`).
///
/// The `ceil` is why the answer is not `amount / max_stack`: 65 of a 64-stack is
/// **two** slots, and 128 is still two.
///
/// `None` for `max_stack == 0`, which is the port's refusal: Java divides
/// `double`s, so a zero gives `Infinity`, `Math.ceil(Infinity)` is `Infinity`, and
/// the `sum += ` at `:186` narrows it to `Integer.MAX_VALUE` — a warehouse that
/// asks for two billion slots instead of saying what is wrong. No item reports a
/// max stack of zero (`Host::max_stack_size`, and vanilla's floor is 1), so the
/// refusal only fires on a caller's own arithmetic bug.
pub fn required_slots(amount: u32, max_stack: u32) -> Option<u32> {
    if max_stack == 0 {
        return None;
    }
    Some(amount.div_ceil(max_stack))
}

/// The order the entries are pushed in: `compare` (`SortingUtils.java:143-161`).
///
/// ```java
/// if (creativeTabId1 != creativeTabId2) return creativeTabId1 - creativeTabId2;
/// final int id1 = getId(t1.getKey().getItem());
/// final int id2 = getId(t2.getKey().getItem());
/// if (id1 == id2) return t1.getKey().getDamageValue() - t2.getKey().getDamageValue();
/// return id1 - id2;
/// ```
///
/// Three keys, in that order: **tab**, then **registry id**, then **damage**.
/// The damage comparison only happens when the ids are equal, which is the one
/// branch that is easy to flatten wrongly — a single tuple sort
/// `(tab, id, damage)` gives the same answer, and that is what this is.
///
/// Java returns `a - b` and relies on the caller; a difference of two `i32`s can
/// overflow, so the port uses [`Ord`] and cannot.
pub fn compare(a: &SortEntry, b: &SortEntry) -> Ordering {
    (a.creative_tab, a.item_id, a.damage).cmp(&(b.creative_tab, b.item_id, b.damage))
}

/// The slots the whole map needs: the `AtomicInteger.sum` of
/// `calcRequiredSlots` (`SortingUtils.java:180-192`).
///
/// `None` propagates [`required_slots`]' refusal: one entry with a stack size of
/// zero makes the total meaningless, and answering a partial sum would be worse
/// than answering nothing.
pub fn required_slot_total(entries: impl IntoIterator<Item = SortEntry>) -> Option<u32> {
    let mut total = 0u32;
    for entry in entries {
        total = total.checked_add(required_slots(entry.amount, entry.max_stack)?)?;
    }
    Some(total)
}

/// The slots each creative tab needs: the `Map<Integer, Integer>` half of
/// `calcRequiredSlots` (`:182-189`).
///
/// ```java
/// creativeTabs.put(index, creativeTabs.getOrDefault(index, 0) + (int) Math.ceil(amount / maxStack));
/// ```
///
/// This is what `pushIntoInv` counts **down** (`:120`) to decide a tab is done
/// (`:123`); returned as a map, it is also what a UI needs to show how much of the
/// warehouse each tab will take.
pub fn slots_per_tab(entries: impl IntoIterator<Item = SortEntry>) -> Option<BTreeMap<i32, u32>> {
    let mut tabs: BTreeMap<i32, u32> = BTreeMap::new();
    for entry in entries {
        let slots = required_slots(entry.amount, entry.max_stack)?;
        let held = tabs.entry(entry.creative_tab).or_insert(0);
        *held = held.checked_add(slots)?;
    }
    Some(tabs)
}

/// The entries in the order `sort` pushes them — `entries.stream().sorted(SortingUtils::compare)`
/// (`SortingUtils.java:68-69`).
///
/// Stable, like Java's `Stream.sorted`: two entries that compare equal keep the
/// order they came in, which for a `BTreeMap` source means their key order.
pub fn sort_order(entries: &mut [SortEntry]) {
    entries.sort_by(compare);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(amount: u32, max_stack: u32, tab: i32, item_id: i32, damage: i32) -> SortEntry {
        SortEntry {
            amount,
            max_stack,
            creative_tab: tab,
            item_id,
            damage,
        }
    }

    /// 65 of a 64-stack is two slots: the `ceil` is the whole rule.
    #[test]
    fn a_partial_stack_takes_a_whole_slot() {
        assert_eq!(required_slots(1, 64), Some(1));
        assert_eq!(required_slots(63, 64), Some(1));
        assert_eq!(required_slots(64, 64), Some(1));
        assert_eq!(required_slots(65, 64), Some(2));
        assert_eq!(required_slots(128, 64), Some(2));
        assert_eq!(required_slots(129, 64), Some(3));
    }

    /// A single-item stack (a tool) takes one slot per item.
    #[test]
    fn a_stack_size_of_one_never_groups() {
        assert_eq!(required_slots(1, 1), Some(1));
        assert_eq!(required_slots(7, 1), Some(7));
    }

    /// An amount of `0` takes no slot, and a zero stack size is refused instead
    /// of saturating to `i32::MAX` like Java's narrowing cast.
    #[test]
    fn the_two_edges_are_zero_amount_and_zero_stack_size() {
        assert_eq!(required_slots(0, 64), Some(0));
        assert_eq!(required_slots(5, 0), None);
    }

    /// Duplicates merge their counts; distinct keys stay apart.
    #[test]
    fn grouping_merges_the_same_key_and_keeps_the_others() {
        let merged = group([("oak", 10), ("birch", 5), ("oak", 7)]);
        assert_eq!(merged.get("oak"), Some(&17));
        assert_eq!(merged.get("birch"), Some(&5));
        assert_eq!(merged.len(), 2);
    }

    /// An empty stack adds nothing — Java skips them before the merge (`:51`).
    #[test]
    fn a_zero_count_never_enters_the_map() {
        let merged = group([("oak", 0), ("oak", 3)]);
        assert_eq!(merged.get("oak"), Some(&3));
        let only_empty = group([("oak", 0)]);
        assert!(only_empty.is_empty());
    }

    /// The comparison is tab first: a lower tab wins whatever the item id.
    #[test]
    fn the_creative_tab_is_the_first_key() {
        let a = entry(1, 64, 0, 500, 0);
        let b = entry(1, 64, 1, 1, 0);
        assert_eq!(compare(&a, &b), Ordering::Less);
        assert_eq!(compare(&b, &a), Ordering::Greater);
    }

    /// Inside a tab, the registry id decides.
    #[test]
    fn the_registry_id_is_the_second_key() {
        let a = entry(1, 64, 2, 10, 3);
        let b = entry(1, 64, 2, 11, 0);
        assert_eq!(compare(&a, &b), Ordering::Less);
    }

    /// And only then the damage value — a damaged item sorts after the pristine
    /// one *with the same id*, not before it.
    #[test]
    fn the_damage_value_is_the_last_key() {
        let unused = entry(1, 64, 2, 10, 0);
        let worn = entry(1, 64, 2, 10, 5);
        assert_eq!(compare(&unused, &worn), Ordering::Less);
        assert_eq!(compare(&worn, &unused), Ordering::Greater);
    }

    /// A full sort puts tabs in order, and within a tab ids in order.
    #[test]
    fn the_order_is_tab_then_id_then_damage() {
        let mut entries = [
            entry(1, 64, 3, 1, 0),
            entry(1, 64, 1, 9, 0),
            entry(1, 64, 1, 2, 4),
            entry(1, 64, 1, 2, 1),
            entry(1, 64, 2, 5, 0),
        ];
        sort_order(&mut entries);
        let order: Vec<(i32, i32, i32)> = entries
            .iter()
            .map(|e| (e.creative_tab, e.item_id, e.damage))
            .collect();
        assert_eq!(order, vec![(1, 2, 1), (1, 2, 4), (1, 9, 0), (2, 5, 0), (3, 1, 0)]);
    }

    /// Equal entries keep their input order: the sort is stable, like Java's.
    #[test]
    fn equal_entries_keep_the_order_they_came_in() {
        let mut entries = [entry(1, 64, 1, 1, 0), entry(2, 64, 1, 1, 0), entry(3, 64, 1, 1, 0)];
        sort_order(&mut entries);
        assert_eq!(
            entries.iter().map(|e| e.amount).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    /// The total is the sum of the per-entry slot counts, `ceil` included.
    #[test]
    fn the_total_is_the_sum_of_the_per_entry_slots() {
        let entries = [entry(65, 64, 1, 1, 0), entry(1, 64, 1, 2, 0), entry(3, 16, 2, 3, 0)];
        assert_eq!(required_slot_total(entries), Some(2 + 1 + 1));
    }

    /// An empty warehouse needs no slot at all.
    #[test]
    fn an_empty_map_needs_no_slots() {
        assert_eq!(required_slot_total(std::iter::empty()), Some(0));
        assert_eq!(slots_per_tab(std::iter::empty()), Some(BTreeMap::new()));
    }

    /// The per-tab subtotals are what `pushIntoInv` counts down, so an entry that
    /// lands in two tabs must be counted in both.
    #[test]
    fn the_per_tab_counts_group_only_within_a_tab() {
        let entries = [
            entry(65, 64, 1, 1, 0), // tab 1: two slots
            entry(1, 64, 1, 2, 0),  // tab 1: one more
            entry(64, 64, 2, 3, 0), // tab 2: one
        ];
        let tabs = slots_per_tab(entries);
        let expected: BTreeMap<i32, u32> = [(1, 3), (2, 1)].into_iter().collect();
        assert_eq!(tabs, Some(expected));
    }

    /// One unbounded entry sinks the subtotals rather than under-reporting them.
    #[test]
    fn a_zero_stack_size_sinks_both_totals() {
        let broken = entry(4, 0, 1, 1, 0);
        assert_eq!(required_slot_total([broken]), None);
        assert_eq!(slots_per_tab([broken]), None);
        assert_eq!(required_slot_total([entry(1, 64, 1, 1, 0), broken]), None);
    }
}
