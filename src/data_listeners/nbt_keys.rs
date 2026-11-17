//! `compatibility/*.json`: which NBT keys make two otherwise equal stacks differ.
//!
//! Java reference: `core/datalistener/ItemNbtListener.java` (81 lines), read by
//! `ItemStackUtils.java:580` on every item comparison, and therefore by request
//! matching in every colony. **One file ships**, an array of 30-odd entries.
//!
//! Two shapes worth writing down, because neither is obvious from the store:
//!
//! - The file's **top level is an array**, not an object (`:60`
//!   `entry.getValue().getAsJsonArray()`). Every other listener in the package
//!   reads an object. A file copied from `crafterrecipes/` would be refused by the
//!   array cast, with the stack trace of the `catch` at `:74-77` and no hint about
//!   which shape was expected.
//! - `checkednbtkeys` is **optional**: its absence stores an **empty set**
//!   (`:64-71`), which is a real answer — "this item has no key worth comparing" —
//!   and not a refusal.
//!
//! **The lookup that can return nothing.** `ForgeRegistries.ITEMS.getValue(itemLoc)`
//! returns `null` for an id no registry knows, and the `null` is used as a **map
//! key** (`:70-73`). Nothing throws: the entry is stored under `null`, survives the
//! client sync as registry id `-1` (`CompatibilityManager.java:210-213` calls
//! `BuiltInRegistries.ITEM.getId(null)`), comes back as `null` on the client, and
//! is never found by the comparison at `ItemStackUtils.java:580`, which looks up by
//! `itemStack1.getItem()`. A pack with a typo therefore loses a comparison rule
//! without a single line in the log. [`Refusal::UnknownItem`] refuses it while the
//! file name is still known.
//!
//! **What is not here**: `CheckedNbtKey` itself and the three tree shapes it
//! serialises (`ItemNbtCalculator.deserializeKeyFromJson`), and the client sync
//! (`CompatibilityManager.java:210-218`). The port keeps the id list, which is what
//! decides *whether two stacks are compared at all*.

use super::Refused;

/// One item of the compatibility file.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: `item` and the optional key list, nothing else is read
pub struct Row {
    /// The registry id, as the file spells it.
    pub item: &'static str,
    /// `checkednbtkeys`, empty when the field is absent (`:64-71`).
    pub keys: Vec<&'static str>,
}

/// Why a compatibility row is refused, named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// An item id no registry knows: Java stores it under a `null` key, where no
    /// comparison can find it (`:70-73`).
    UnknownItem { id: &'static str, item: &'static str },
}

impl Refused for Refusal {
    fn name(&self) -> String {
        match *self {
            Refusal::UnknownItem { id, item } => {
                format!("data_listeners.refusal.unknown_compat_item({id}.{item})")
            }
        }
    }
}

/// Reads one entry of the array, `known_item` being the registry's answer.
///
/// The `id` is the **file's** id, which the file itself never states: the
/// compatibility file holds no per-entry name, so every entry of one file carries
/// its file name and the item id is what tells them apart.
pub fn parse(
    id: &'static str,
    item: &'static str,
    keys: Vec<&'static str>,
    known_item: &dyn Fn(&str) -> bool,
) -> Result<Row, Refusal> {
    if !known_item(item) {
        return Err(Refusal::UnknownItem { id, item });
    }
    Ok(Row { item, keys })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known(item: &str) -> bool {
        item.starts_with("minecraft:") || item.starts_with("domum_ornamentum:")
    }

    #[test]
    fn a_key_list_is_kept_and_an_absent_one_is_an_empty_answer() {
        let with = parse("compatibility/x", "minecraft:barrel", vec!["minecraft:block/oak_planks"], &known)
            .expect("known item");
        assert_eq!(with.keys.len(), 1);
        let without = parse("compatibility/x", "minecraft:barrel", Vec::new(), &known)
            .expect("a missing field is not a refusal");
        assert!(without.keys.is_empty());
    }

    #[test]
    fn an_unknown_item_is_refused_instead_of_being_stored_under_a_null_key() {
        let refusal = parse("compatibility/x", "mod:gone", Vec::new(), &known)
            .expect_err("no registry knows it");
        assert_eq!(
            refusal.name(),
            "data_listeners.refusal.unknown_compat_item(compatibility/x.mod:gone)"
        );
    }

    #[test]
    fn the_shipped_file_names_the_items_the_renderer_depends_on() {
        // One sample of the real file, to pin the shape: an item with no key list.
        let shipped = parse("compatibility/domum", "domum_ornamentum:architectscutter", Vec::new(), &known)
            .expect("shipped item");
        assert_eq!(shipped.item, "domum_ornamentum:architectscutter");
        assert!(shipped.keys.is_empty());
    }
}
