//! The four tag reads and the one tag shape `quests` and `loot` need, in Java's
//! own answers.
//!
//! Java reference: `net.minecraft.nbt.CompoundTag`'s `getInt`, `getDouble`,
//! `getString`, `getList` and `putDouble` — the five calls `QuestManager.java`
//! makes (`:254-300`) plus the one `DatagenLootTableManager` does not need (it
//! reads a file, not a compound).
//!
//! **This is a fifth copy, on purpose and with the same regret the four others
//! state.** `src/requests/nbt.rs`, `src/persistence/nbt.rs`,
//! `src/manager_04/nbt.rs` and `src/manager_07/nbt.rs` are all `mod nbt;` —
//! private, so a sibling cannot name them — and widening one would be an edit
//! outside this folder. Two differences from `src/manager_07/nbt.rs`, both
//! deliberate:
//!
//! - [`double`] exists, because `getDouble(TAG_REPUTATION)` is a real read and
//!   Java's `getDouble` accepts every numeric width.
//! - [`int`] answers `None` for an absent key, like `manager_07`'s and unlike
//!   Java's `getInt`, because `QuestManager` has one place where "the file says
//!   zero" and "the file says nothing" differ: a `finishedquests` record with no
//!   `quantity` is a quest completed zero times
//!   (`QuestManager.java:290` reads it and `:288` writes it).

use mod_api::{Compound, Tag};

/// `CompoundTag.getInt`: an integer in whichever width it was saved.
pub fn int(data: &Compound, key: &str) -> Option<i32> {
    match data.get(&key.into())? {
        Tag::Int(value) => Some(*value),
        Tag::Short(value) => Some(i32::from(*value)),
        Tag::Byte(value) => Some(i32::from(*value)),
        _ => None,
    }
}

/// `CompoundTag.getDouble` (`:300`), which widens every numeric tag rather than
/// only `Double` — the reason `questreputation` written by an older file as a
/// float still reads.
pub fn double(data: &Compound, key: &str) -> Option<f64> {
    match data.get(&key.into())? {
        Tag::Double(bits) => Some(f64::from_bits(*bits)),
        Tag::Float(bits) => Some(f64::from(f32::from_bits(*bits))),
        Tag::Int(value) => Some(f64::from(*value)),
        Tag::Long(value) => Some(*value as f64),
        Tag::Short(value) => Some(f64::from(*value)),
        Tag::Byte(value) => Some(f64::from(*value)),
        _ => None,
    }
}

/// `CompoundTag.getString`: a tag of another kind is `None`, not its debug form.
pub fn text(data: &Compound, key: &str) -> Option<String> {
    match data.get(&key.into())? {
        Tag::String(value) => Some(value.to_string_lossy()),
        _ => None,
    }
}

/// The compound a key holds, if it holds one — `getCompound` without Java's
/// fallback to an empty compound, so a caller can tell "absent" from "empty".
/// `Colony.deserializeNBT` does **not** tell them apart for this manager
/// (`Colony.java:783` reads `quest_manager` with no `contains` guard, unlike the
/// request manager two lines earlier at `:863`).
pub fn compound<'a>(data: &'a Compound, key: &str) -> Option<&'a Compound> {
    match data.get(&key.into())? {
        Tag::Compound(inner) => Some(inner),
        _ => None,
    }
}

/// Every compound of the list under `key`, in order. An absent or corrupt list is
/// empty — `getList(key, Tag.TAG_COMPOUND)` answers an empty list for both, and
/// so does this.
pub fn compounds<'a>(data: &'a Compound, key: &str) -> Vec<&'a Compound> {    match data.get(&key.into()) {
        Some(Tag::List(items)) => items
            .iter()
            .filter_map(|item| match item {
                Tag::Compound(inner) => Some(inner),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// A list of compounds: NBT lists are homogeneous
/// (`crates/horizon-core/src/nbt.rs:52`), so a list of records is a
/// `List<Compound>` and nothing else.
pub fn compound_list(items: Vec<Compound>) -> Tag {
    Tag::List(items.into_iter().map(Tag::Compound).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_key_is_none_and_a_zero_is_a_zero() {
        let data = Compound::from([("quantity".into(), Tag::Int(0))]);
        assert_eq!(int(&data, "quantity"), Some(0));
        assert_eq!(int(&data, "absent"), None);
    }

    #[test]
    fn get_double_widens_every_numeric_width_and_get_string_never_does() {
        let data = Compound::from([
            ("double".into(), Tag::Double(1.5f64.to_bits())),
            ("float".into(), Tag::Float(0.25f32.to_bits())),
            ("int".into(), Tag::Int(-3)),
            ("text".into(), Tag::String("minecraft:quest".into())),
        ]);
        assert_eq!(double(&data, "double"), Some(1.5));
        assert_eq!(double(&data, "float"), Some(0.25));
        assert_eq!(double(&data, "int"), Some(-3.0));
        assert_eq!(double(&data, "text"), None);
        assert_eq!(text(&data, "int"), None);
        assert_eq!(text(&data, "text").as_deref(), Some("minecraft:quest"));
    }

    #[test]
    fn a_list_of_records_keeps_its_order_and_drops_what_is_not_a_record() {
        let one = Compound::from([("id".into(), Tag::String("a:a".into()))]);
        let two = Compound::from([("id".into(), Tag::String("a:b".into()))]);
        let data = Compound::from([(
            "availquests".into(),
            Tag::List(vec![Tag::Compound(one), Tag::Int(9), Tag::Compound(two)]),
        )]);
        let read = compounds(&data, "availquests");
        assert_eq!(read.len(), 2, "the stray int is not a record");
        assert_eq!(
            text(read[1], "id").as_deref(),
            Some("a:b"),
            "the order is the file's"
        );
        assert!(compounds(&data, "absent").is_empty());
        assert_eq!(
            compound_list(vec![read[0].clone()]),
            Tag::List(vec![Tag::Compound(read[0].clone())])
        );
    }
}
