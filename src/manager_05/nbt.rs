//! The six tag reads these four managers share, and why this is a fifth copy.
//!
//! **A copy, and the fifth of its kind.** `crate::colony::nbt` is `mod nbt;`
//! inside `crate::colony` (`src/colony/mod.rs:31`), so it is private to that
//! module; `src/building/nbt.rs:8`, `src/requests/nbt.rs:9` and
//! `src/manager_04/nbt.rs:4` already carry the same copy and say why. This is the
//! one `crate::manager_05` needs, and it is here for the same reason: these
//! managers are written against the tag helpers without editing a module another
//! agent owns. The five copies are byte-for-byte compatible; the day one of them
//! becomes public, four of them go.
//!
//! **Tolerant reads, strict writes.** `int` accepts the three integer widths NBT
//! can hold for a small number, because Java's `getInt` does; `text` refuses
//! anything that is not a string rather than returning a debug form, because a
//! building named `<Compound>` is not a name; `boolean` is Java's `getBoolean`,
//! where anything but a zero byte is true and an absent key is false; a missing
//! coordinate is `None`, not zero, because a grave at the origin is not a grave
//! without a position.

use mod_api::{Compound, Tag};

/// A position, laid out as `BlockPosUtil` lays one out: three ints under `x`,
/// `y`, `z` (`api/util/BlockPosUtil.java:219-226`).
pub fn position(at: [i32; 3]) -> Tag {
    Tag::compound([
        ("x", Tag::Int(at[0])),
        ("y", Tag::Int(at[1])),
        ("z", Tag::Int(at[2])),
    ])
}

/// The position a tag holds (`BlockPosUtil.java:190-197`).
pub fn position_of(tag: &Tag) -> Option<[i32; 3]> {
    Some([
        number(tag.get("x")?)?,
        number(tag.get("y")?)?,
        number(tag.get("z")?)?,
    ])
}

/// The position stored under `key`.
pub fn position_in(data: &Compound, key: &str) -> Option<[i32; 3]> {
    position_of(data.get(&key.into())?)
}

/// An integer under `key`, in whichever width it was saved.
pub fn int(data: &Compound, key: &str) -> Option<i32> {
    number(data.get(&key.into())?)
}

/// A string under `key`. A tag of another kind is `None`, not its debug form.
pub fn text(data: &Compound, key: &str) -> Option<String> {
    match data.get(&key.into())? {
        Tag::String(value) => Some(value.to_string_lossy()),
        _ => None,
    }
}

/// Java's `CompoundTag.getBoolean`: **anything but a zero byte is true**, and an
/// absent key is false.
pub fn boolean(data: &Compound, key: &str) -> bool {
    match data.get(&key.into()) {
        Some(Tag::Byte(value)) => *value != 0,
        Some(Tag::Int(value)) => *value != 0,
        _ => false,
    }
}

/// Whether the key is there at all — Java's `compound.contains(key)`.
pub fn holds(data: &Compound, key: &str) -> bool {
    data.contains_key(&key.into())
}

/// Every compound of the list under `key`, in order. An absent or corrupt list is
/// empty: a colony whose backup list was hand-edited into a string still loads.
pub fn compounds<'a>(data: &'a Compound, key: &str) -> Vec<&'a Compound> {
    items(data, key)
        .iter()
        .filter_map(|item| match item {
            Tag::Compound(inner) => Some(inner),
            _ => None,
        })
        .collect()
}

/// The raw items of the list under `key`, compounds or not — the read a caller
/// needs when it has to **name** an item that is not a compound instead of
/// dropping it.
pub fn items<'a>(data: &'a Compound, key: &str) -> &'a [Tag] {
    match data.get(&key.into()) {
        Some(Tag::List(items)) => items.as_slice(),
        _ => &[],
    }
}

/// A list of compounds: NBT lists are homogeneous (`horizon-core/src/nbt.rs:52`),
/// so a list of records is a `List<Compound>`.
pub fn compound_list(items: Vec<Compound>) -> Tag {
    Tag::List(items.into_iter().map(Tag::Compound).collect())
}

fn number(tag: &Tag) -> Option<i32> {
    match tag {
        Tag::Int(value) => Some(*value),
        Tag::Byte(value) => Some(i32::from(*value)),
        Tag::Short(value) => Some(i32::from(*value)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_position_round_trips_and_a_missing_coordinate_is_none() {
        assert_eq!(position_of(&position([-3, 71, 1208])), Some([-3, 71, 1208]));
        let halved = Tag::compound([("x", Tag::Int(4)), ("z", Tag::Int(9))]);
        assert_eq!(position_of(&halved), None);
        assert_eq!(position_in(&Compound::new(), "pos"), None);
    }

    #[test]
    fn a_boolean_is_java_s_get_and_a_list_of_records_keeps_its_order() {
        let data = Compound::from([
            ("on".into(), Tag::Byte(1)),
            ("off".into(), Tag::Byte(0)),
            ("absent".into(), Tag::End),
        ]);
        assert!(boolean(&data, "on"));
        assert!(!boolean(&data, "off"), "getBoolean reads a zero byte as false");
        assert!(!boolean(&data, "absent"), "and a missing key as false");
        assert!(!boolean(&data, "zzz"));
        assert_eq!(int(&data, "on"), Some(1), "a byte reads as the same integer");
        assert!(!holds(&data, "zzz"), "contains is not the same test as a value");

        let first = Compound::from([("id".into(), Tag::Int(1))]);
        let second = Compound::from([("id".into(), Tag::Int(2))]);
        let listed = Compound::from([("l".into(), compound_list(vec![first, second]))]);
        let read = compounds(&listed, "l");
        assert_eq!(read.len(), 2);
        assert_eq!(int(read[1], "id"), Some(2));
        assert_eq!(items(&listed, "l").len(), 2);
        assert!(compounds(&listed, "nope").is_empty());
    }
}
