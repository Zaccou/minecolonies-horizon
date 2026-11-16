//! The item and its count, and the one rule the engine does not answer: how many
//! of an item one slot takes.
//!
//! Vanilla's `ItemStack` is already here — `mod_api::ItemStack` — and it is a
//! **count without a limit**: "Maxima are checked when the game accepts a write,
//! not by this record" (`crates/horizon-mod-vocab/src/item/item_stack.rs:46-50`).
//! The game answers that limit as metadata, `Host::max_stack_size`
//! (`crates/horizon-mod-vocab/src/host/items.rs:8-12`, "for a mod's container that
//! fills or merges stacks itself" — this port is exactly that caller). So [Stack]
//! is *what* a slot holds and never consults anything, and [SlotLimits] is *how
//! much* it takes, asked of whoever knows: the engine in
//! [super::engine::HostLimits], a table in `ItemLimits` for a test.
//!
//! **Why the item is a name here and a key in the engine.** `ItemKey` has a private
//! field and no public constructor — `ItemKey::from_slot` is `pub(crate)`
//! (`crates/horizon-mod-vocab/src/content/registry_keys.rs:33-44`) — so a name is
//! the only item identity this crate can build, compare and test without a game.
//! `mod_api::ItemStack` cannot even be *constructed* on the host target without a
//! `Host` to resolve the name, which is why the pure layer is on names and
//! [super::engine] converts at the boundary.

use crate::inventory::Refusal;
use mod_api::Identifier;

/// A registry name, `minecraft:oak_planks`.
///
/// Normalised once, at construction: a bare name takes the vanilla namespace, as
/// every string that names an item in this API does — "`stone` is
/// `minecraft:stone`" (`crates/horizon-mod-vocab/src/host/items.rs:54-55`).
/// The charset is the engine's own, `Identifier::parse`
/// (`horizon-core/src/identifier.rs:65-76`), so a name this type accepts is one
/// the engine can resolve.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ItemName(String);

impl ItemName {
    /// `None` for text the engine cannot parse, and for the empty path that
    /// `Identifier::parse("")` accepts (`from_parts` only checks the characters).
    pub fn new(name: &str) -> Option<Self> {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        let id = Identifier::parse(name).ok()?;
        if id.path().is_empty() {
            return None;
        }
        Some(Self(id.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ItemName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// An item and a positive count: one slot's worth, or part of one.
///
/// Built through [`Stack::new`], which is the only place the "never empty"
/// invariant is enforced. `mod_api::ItemStack` keeps the same invariant with a
/// `NonZeroU32` (`item_stack.rs:62-67`); this one keeps it with a `u32` and a
/// constructor, because a `Stack` is also what a *request* carries before any
/// slot exists to hold it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stack {
    item: ItemName,
    count: u32,
}

impl Stack {
    /// `Err(EmptyQuantity)` for a zero count: an empty slot is `None`, not a
    /// stack of nothing.
    pub fn new(item: ItemName, count: u32) -> Result<Self, Refusal> {
        if count == 0 {
            return Err(Refusal::EmptyQuantity);
        }
        Ok(Self { item, count })
    }

    pub fn item(&self) -> &ItemName {
        &self.item
    }

    pub fn count(&self) -> u32 {
        self.count
    }

    /// Whether this stack and `other` may share one slot: same item, and no
    /// components in the port's model to disagree about. Vanilla also compares
    /// the component patch (`item_stack.rs:251-258`); this port carries none.
    pub fn stackable_with(&self, other: &Self) -> bool {
        self.item == other.item
    }

    /// Take up to `count` items off `self`, consuming it: what was taken, and
    /// what is left. Either may be absent, and neither is ever empty — the same
    /// shape as `ItemStack::split` (`item_stack.rs:143-152`, `:225-245`).
    pub fn split(self, count: u32) -> (Option<Self>, Option<Self>) {
        if count == 0 {
            return (None, Some(self));
        }
        if count >= self.count {
            return (Some(self), None);
        }
        let taken = Self {
            item: self.item.clone(),
            count,
        };
        let rest = Self {
            item: self.item,
            count: self.count - count,
        };
        (Some(taken), Some(rest))
    }

    /// Top `self` up from `incoming`, no further than `cap`: how many moved, and
    /// what is left of `incoming`.
    ///
    /// A different item moves nothing (`0, Some(incoming)`), and a full self moves
    /// nothing either — the caller decides where the remainder goes.
    pub fn merge(&mut self, incoming: Self, cap: u32) -> (u32, Option<Self>) {
        if !self.stackable_with(&incoming) {
            return (0, Some(incoming));
        }
        let room = cap.saturating_sub(self.count);
        if room == 0 {
            return (0, Some(incoming));
        }
        let (taken, rest) = incoming.split(room);
        let moved = taken.map_or(0, |taken| {
            self.count += taken.count;
            taken.count
        });
        (moved, rest)
    }
}

/// How many of an item one slot holds — the engine's
/// `Item.getDefaultMaxStackSize`, behind `Host::max_stack_size`
/// (`crates/horizon-mod-vocab/src/host/items.rs:8-12`).
///
/// `None` means the registry does not know the item, and no slot takes it: a
/// write would be refused `BlockEntityItemError::InvalidStack`
/// (`crates/horizon-mod-vocab/src/block/block_entity_items.rs:19-21`). This
/// layer says the same thing up front instead of writing and rolling back.
pub trait SlotLimits {
    fn max_count(&self, item: &ItemName) -> Option<u32>;
}

/// One limit for every item, for a caller that has no registry to ask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SingleLimit(pub u32);

impl SlotLimits for SingleLimit {
    fn max_count(&self, _item: &ItemName) -> Option<u32> {
        Some(self.0)
    }
}

/// Per-item limits, and nothing for an item the table does not list: the test
/// double of a registry, and honest about it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ItemLimits(Vec<(ItemName, u32)>);

impl ItemLimits {
    pub fn new(entries: impl IntoIterator<Item = (ItemName, u32)>) -> Self {
        Self(entries.into_iter().collect())
    }

    /// A table with one entry, which is most tests.
    pub fn one(item: ItemName, cap: u32) -> Self {
        Self(vec![(item, cap)])
    }
}

impl SlotLimits for ItemLimits {
    fn max_count(&self, item: &ItemName) -> Option<u32> {
        self.0
            .iter()
            .find(|(known, _)| known == item)
            .map(|(_, cap)| *cap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn planks() -> ItemName {
        ItemName::new("minecraft:oak_planks").expect("a valid name")
    }

    fn stone() -> ItemName {
        ItemName::new("minecraft:stone").expect("a valid name")
    }

    #[test]
    fn a_bare_name_takes_the_vanilla_namespace_and_a_bad_one_is_refused() {
        assert_eq!(
            ItemName::new("stone").expect("bare names resolve").as_str(),
            "minecraft:stone"
        );
        assert_eq!(
            ItemName::new("  minecolonies:blockhutbuilder  ")
                .expect("surrounding space is trimmed")
                .as_str(),
            "minecolonies:blockhutbuilder"
        );
        // `Identifier::parse("")` succeeds with an empty path; this type does not.
        assert_eq!(ItemName::new(""), None);
        assert_eq!(ItemName::new("   "), None);
        assert_eq!(ItemName::new("stone:"), None);
        // The engine's charset is lower case only.
        assert_eq!(ItemName::new("Minecraft:Stone"), None);
        assert_eq!(ItemName::new("minecraft:oak planks"), None);
    }

    #[test]
    fn a_stack_is_never_empty() {
        assert_eq!(Stack::new(planks(), 0), Err(Refusal::EmptyQuantity));
        let one = Stack::new(planks(), 1).expect("one item is a stack");
        assert_eq!(one.count(), 1);
        assert_eq!(one.item(), &planks());
    }

    #[test]
    fn a_split_conserves_the_items_whatever_the_cut() {
        for cut in [0, 1, 3, 4, 5, u32::MAX] {
            let whole = Stack::new(planks(), 4).expect("four items");
            let (taken, rest) = whole.clone().split(cut);
            let moved = taken.as_ref().map_or(0, Stack::count);
            let kept = rest.as_ref().map_or(0, Stack::count);
            assert_eq!(moved, cut.min(4), "cut {cut}");
            assert_eq!(moved + kept, 4, "cut {cut} lost or made items");
            for portion in [taken, rest].into_iter().flatten() {
                assert_eq!(portion.item(), &planks());
                assert!(portion.count() > 0, "a split portion is never empty");
            }
        }
    }

    #[test]
    fn a_merge_stops_at_the_cap_and_gives_the_rest_back() {
        let mut held = Stack::new(planks(), 10).expect("ten items");
        let (moved, rest) = held.merge(Stack::new(planks(), 60).expect("sixty"), 64);
        assert_eq!(moved, 54, "64 - 10");
        assert_eq!(held.count(), 64);
        assert_eq!(rest.map(|rest| rest.count()), Some(6), "the remainder comes back");

        // At the cap, nothing moves and the incoming stack is untouched.
        let (moved, rest) = held.merge(Stack::new(planks(), 5).expect("five"), 64);
        assert_eq!(moved, 0);
        assert_eq!(rest.map(|rest| rest.count()), Some(5));
        assert_eq!(held.count(), 64);
    }

    #[test]
    fn a_merge_of_another_item_moves_nothing() {
        let mut held = Stack::new(planks(), 10).expect("ten items");
        let (moved, rest) = held.merge(Stack::new(stone(), 10).expect("ten stone"), 64);
        assert_eq!(moved, 0);
        assert_eq!(rest.map(|rest| rest.item().clone()), Some(stone()));
        assert_eq!(held.count(), 10, "the other item never lands here");
    }

    #[test]
    fn the_limits_are_asked_per_item_and_an_unknown_one_has_none() {
        let limits = ItemLimits::one(planks(), 64);
        assert_eq!(limits.max_count(&planks()), Some(64));
        assert_eq!(
            limits.max_count(&stone()),
            None,
            "a table that never heard of an item knows no limit for it"
        );
        assert_eq!(SingleLimit(16).max_count(&stone()), Some(16));
    }
}
