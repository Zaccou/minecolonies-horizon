//! `ItemNbtCalculator`: which NBT keys make two stacks of the same item
//! *different*, and the file that tells the colony's inventory so.
//!
//! Java reference: `core/generation/ItemNbtCalculator.java` (253 lines). It is
//! the only generator that reads the **item registry** rather than writing a
//! table: it walks every item of every creative tab, extracts the keys of the
//! item's default `CompoundTag`, adds three the tag does not show, removes one it
//! does, and writes the result to `data/minecolonies/compatibility/itemnbtmatching.json`
//! (`:128`), which `ItemNbtMatching` reads back at runtime.
//!
//! | Rule | Java | Effect |
//! | --- | --- | --- |
//! | a key of the stack's tag is a checked key | `:93` | the tag's own keys |
//! | a compound value is checked **recursively** | `:215-232` | `display` carries `color` |
//! | a dyeable item is checked on `display` | `:95-98` | `DyeableLeatherItem` |
//! | an enchantable item is checked on `Enchantments` | `:99-102` | `stack.isEnchantable()` |
//! | a repairable item is checked on `RepairCost` | `:103-106` | `stack.isRepairable()` |
//! | **damage is never checked** | `:107-108` | `keys.remove("Damage")` |
//! | an item whose NBT is ignored contributes nothing | `:92` | `ModTags.ignoreNBT` |
//!
//! # Why damage is removed
//!
//! A pickaxe at 40 % durability is the same item to a colony: a builder does not
//! refuse a worn tool, and a request for one diamond pickaxe must not be filled
//! by a *specific* durability. Removing the key is the whole point of the
//! calculator, and it is why the file is called *matching* and not *nbt*.
//!
//! # The two things this port cannot do, and both are about the item list
//!
//! 1. **the rows.** Java gets them from `CraftingUtils.forEachCreativeTabItems`
//!    (`:62`) — every item in every creative tab, which needs the item registry
//!    and the creative tab registry. This port has the item table
//!    ([`crate::items`]) but not the tabs, so [`matching`] takes the rows as an
//!    argument and [`Pack::build`](super::Pack::build) refuses the file by name
//!    instead of writing an empty one.
//! 2. **`textureData`.** A Domum Ornamentum block gets a synthetic
//!    `textureData` compound whose keys are its component ids and whose values are
//!    the default material of each (`:66-78`). That component list lives in
//!    another mod's registry; [`Facts::texture_data`] carries it as data when a
//!    caller has it.
//!
//! # Java's set order is not reproducible, and does not need to be
//!
//! The keys of one item are a `HashSet` (`:110`) and the children of a compound
//! too (`:220`), so the order inside `checkednbtkeys` is whatever the hash
//! produced for that JVM. This port sorts, which is a **strictly stronger**
//! property: two runs agree, and a diff of the file is readable. The delivered
//! file is absent from `data/` (see [`super::DELIVERED`]), so nothing compares
//! against it; the JSON shape below is the one `ItemNbtMatching` parses.

use super::json::{arr, obj, s, Json};
use super::{File, Refusal};

/// One checked key, and the keys of its compound value.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key {
    pub key: String,
    pub children: Vec<Key>,
}

impl Key {
    pub fn new(key: &str) -> Key {
        Key {
            key: key.to_owned(),
            children: Vec::new(),
        }
    }

    /// The JSON `ItemNbtCalculator.serializeKeyToJson` writes (`:153-165`): the
    /// key, and a `children` array only when there is one.
    pub fn json(&self) -> Json {
        let mut pairs = vec![("key", s(&self.key))];
        if !self.children.is_empty() {
            pairs.push(("children", arr(self.children.iter().map(Key::json).collect())));
        }
        obj(pairs)
    }
}

/// What the calculator needs to know about one stack, with no registry in sight.
pub struct Facts {
    /// The item id, `minecolonies:ancienttome`.
    pub item: String,
    /// The keys of the stack's tag, each with the keys of its compound value —
    /// an empty list for a scalar (`Facts::scalar("display")` for a plain tag).
    pub keys: Vec<Key>,
    /// `stack.getItem() instanceof DyeableLeatherItem`.
    pub dyeable: bool,
    /// `stack.isEnchantable()`.
    pub enchantable: bool,
    /// `stack.isRepairable()`.
    pub repairable: bool,
    /// Whether the item is in `ModTags.ignoreNBT`, in which case its own tag
    /// contributes nothing (`:92`) — the four traits still apply.
    pub ignored: bool,
}

impl Facts {
    pub fn new(item: &str) -> Facts {
        Facts {
            item: item.to_owned(),
            keys: Vec::new(),
            dyeable: false,
            enchantable: false,
            repairable: false,
            ignored: false,
        }
    }

    pub fn scalar(mut self, key: &str) -> Facts {
        self.keys.push(Key::new(key));
        self
    }

    pub fn compound(mut self, key: &str, children: &[&str]) -> Facts {
        self.keys.push(Key {
            key: key.to_owned(),
            children: children.iter().map(|child| Key::new(child)).collect(),
        });
        self
    }

    pub fn dyeable(mut self) -> Facts {
        self.dyeable = true;
        self
    }

    pub fn enchantable(mut self) -> Facts {
        self.enchantable = true;
        self
    }

    pub fn repairable(mut self) -> Facts {
        self.repairable = true;
        self
    }

    pub fn ignored(mut self) -> Facts {
        self.ignored = true;
        self
    }
}

/// The checked keys of one stack, sorted and deduplicated.
///
/// The order of the two halves matters and is Java's: the tag's own keys first,
/// then the three synthetic ones — *then* `Damage` is removed, which is why an
/// item whose tag has nothing but `Damage` still ends with an empty list and not
/// with nothing at all: the stack exists, it is just indistinguishable from a new
/// one.
///
/// **Deduplication is part of the rule, not a nicety.** Java collects the keys in
/// a `HashSet<String>` (`ItemNbtCalculator.java:93`) and the children of a compound
/// in a second one (`:220`), so a tag that repeats a key collapses to one entry
/// before the JSON is written. Sorting instead of hashing is this port's choice
/// (see the module doc); a sorted list with the duplicates still in it would be a
/// different list.
pub fn checked_keys(facts: &Facts) -> Vec<Key> {
    let mut keys: Vec<Key> = if facts.ignored {
        Vec::new()
    } else {
        facts.keys.clone()
    };

    let mut ensure = |key: &str| {
        if !keys.iter().any(|existing| existing.key == key) {
            keys.push(Key::new(key));
        }
    };
    if facts.dyeable {
        ensure("display");
    }
    if facts.enchantable {
        ensure("Enchantments");
    }
    if facts.repairable {
        ensure("RepairCost");
    }
    keys.retain(|key| key.key != "Damage");
    for key in &mut keys {
        sort_children(key);
    }
    // By key alone: two rows of the same name are one key in Java's set, and which
    // of the two child lists survives is not something the file can say.
    keys.sort_by(|left, right| left.key.cmp(&right.key));
    keys.dedup_by(|left, right| left.key == right.key);
    keys
}

/// Sort a key's children, and theirs: Java's `createKeyFromNbt` fills a `HashSet`
/// at every level (`:215-232`).
fn sort_children(key: &mut Key) {
    key.children.sort();
    for child in &mut key.children {
        sort_children(child);
    }
}

/// One row of the file: the item and its checked keys.
pub fn row(facts: &Facts) -> Json {
    let keys = checked_keys(facts);
    let mut pairs = vec![("item", s(&facts.item))];
    if !keys.is_empty() {
        pairs.push(("checkednbtkeys", arr(keys.iter().map(Key::json).collect())));
    }
    obj(pairs)
}

/// The whole file, from the rows — `:129-145`.
///
/// The rows are sorted by item id, because Java keeps them in a `TreeMap`
/// (`:88`) and that order *is* part of the output.
pub fn matching(rows: &[Facts]) -> File {
    let mut sorted: Vec<&Facts> = rows.iter().collect();
    sorted.sort_by(|left, right| left.item.cmp(&right.item));
    File {
        path: "minecolonies/compatibility/itemnbtmatching.json".to_owned(),
        json: arr(sorted.iter().map(|facts| row(facts)).collect()),
    }
}

/// Why [`Pack::build`](super::Pack::build) writes no NBT file.
pub fn refusal() -> Refusal {
    Refusal::NoSource {
        what: "itemnbtmatching",
        why: "the rows are the keys of every item of every creative tab, and this port has no creative-tab registry; the rule is ported and tested, the row list is not available",
        java: "ItemNbtCalculator.java:57-96",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::json::s;

    #[test]
    fn damage_is_the_one_key_that_is_never_checked() {
        let facts = Facts::new("minecraft:diamond_pickaxe").scalar("Damage").scalar("display");
        let keys = checked_keys(&facts);
        assert_eq!(keys, vec![Key::new("display")]);
    }

    #[test]
    fn an_item_with_nothing_but_damage_is_still_a_row_with_no_keys() {
        // The row exists (the item is in the file) and carries no key, which is
        // how a reader learns "any durability will do".
        let facts = Facts::new("minecraft:iron_shovel").scalar("Damage");
        assert!(checked_keys(&facts).is_empty());
        let json = row(&facts);
        assert!(json.get("checkednbtkeys").is_none(), "{json}");
        assert_eq!(json["item"], s("minecraft:iron_shovel"));
    }

    #[test]
    fn a_dyed_dyed_armour_checks_display_and_its_colour() {
        let facts = Facts::new("minecraft:leather_helmet")
            .compound("display", &["color", "Name"])
            .dyeable();
        let keys = checked_keys(&facts);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "display");
        assert_eq!(
            keys[0].children,
            vec![Key::new("Name"), Key::new("color")],
            "children are sorted, Java's HashSet is not"
        );
        let json = keys[0].json();
        assert_eq!(json["children"][1]["key"], s("color"));
    }

    #[test]
    fn an_enchanted_and_repaired_tool_gets_both_synthetic_keys() {
        let facts = Facts::new("minecraft:diamond_pickaxe")
            .scalar("Damage")
            .enchantable()
            .repairable();
        assert_eq!(
            checked_keys(&facts),
            vec![Key::new("Enchantments"), Key::new("RepairCost")]
        );
    }

    #[test]
    fn an_item_in_the_ignore_tag_contributes_none_of_its_own_keys() {
        // `:92` — `stack.is(ModTags.ignoreNBT)` empties the tag before the keys
        // are read, but the three traits are still added afterwards.
        let facts = Facts::new("minecolonies:scroll_buff")
            .scalar("someCustomTag")
            .ignored()
            .repairable();
        assert_eq!(checked_keys(&facts), vec![Key::new("RepairCost")]);
    }

    #[test]
    fn a_key_is_never_added_twice() {
        let facts = Facts::new("minecraft:book")
            .scalar("display")
            .dyeable()
            .scalar("display");
        let keys = checked_keys(&facts);
        assert_eq!(keys.iter().filter(|key| key.key == "display").count(), 1);
    }

    #[test]
    fn the_file_is_sorted_by_item_and_skips_an_empty_key_list() {
        let rows = vec![
            Facts::new("minecraft:zombie_head"),
            Facts::new("minecraft:apple").scalar("Damage"),
            Facts::new("minecraft:mace").enchantable(),
        ];
        let file = matching(&rows);
        assert_eq!(file.path, "minecolonies/compatibility/itemnbtmatching.json");
        let items: Vec<&str> = file.json.as_array().unwrap().iter().filter_map(|row| row["item"].as_str()).collect();
        assert_eq!(items, vec!["minecraft:apple", "minecraft:mace", "minecraft:zombie_head"]);
        assert!(file.json[0].get("checkednbtkeys").is_none());
        assert_eq!(file.json[1]["checkednbtkeys"][0]["key"], s("Enchantments"));
    }

    #[test]
    fn the_refusal_names_the_registry_that_is_missing() {
        let refusal = refusal();
        assert!(refusal.sentence().contains("creative-tab"));
        assert!(refusal.sentence().contains("ItemNbtCalculator.java"));
    }
}
