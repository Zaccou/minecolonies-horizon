//! The tag reads and writes the building model shares.
//!
//! `Compound` is `BTreeMap<JavaString, Tag>`, `Tag` the NBT value
//! (`crates/horizon-core/src/nbt.rs:35`, `:41-56`), both re-exported by the API.
//! The crate has one dependency, `horizon-mod-api` (`Cargo.toml`), so a
//! serialisable model here is a pair of functions, not a `Serialize` derive.
//!
//! **This is a copy of `crate::colony::nbt`,** which is `mod nbt;` inside
//! `colony` and therefore not reachable from here (`src/colony/mod.rs:17`).
//! Making it `pub(crate)` is a one-line change to a module this agent may not
//! touch; the report says so rather than duplicating it quietly.
//!
//! **Tolerant reads, strict writes.** A read accepts the three integer widths
//! NBT can hold for a small number, so a `/data merge block` written by hand
//! stays readable; a write always uses the width the model means.

use mod_api::{Compound, Tag};

/// A position, laid out as `BlockPosUtil` lays one out: three ints under `x`,
/// `y`, `z` (`api/util/BlockPosUtil.java:190-197`, written at `:219-226`).
pub fn position(at: [i32; 3]) -> Tag {
    Tag::compound([
        ("x", Tag::Int(at[0])),
        ("y", Tag::Int(at[1])),
        ("z", Tag::Int(at[2])),
    ])
}

/// The position a tag holds. A missing coordinate is `None`, not zero.
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

/// The integer a tag holds, in whichever width it was saved. The nested case:
/// a number inside a compound the caller already reached.
pub fn int_of(tag: &Tag) -> Option<i32> {
    number(tag)
}

/// A string under `key`. A tag of another kind is `None`, not its debug form.
pub fn text(data: &Compound, key: &str) -> Option<String> {
    match data.get(&key.into())? {
        Tag::String(value) => Some(value.to_string_lossy()),
        _ => None,
    }
}

/// A boolean under `key`. NBT has no boolean: Java's `putBoolean` writes a
/// `Byte` (`net.minecraft.nbt.CompoundTag#putBoolean`) and this reads the two
/// integer widths a hand-written `/data merge` may leave.
pub fn boolean(data: &Compound, key: &str) -> Option<bool> {
    match data.get(&key.into())? {
        Tag::Byte(value) => Some(*value != 0),
        Tag::Int(value) => Some(*value != 0),
        _ => None,
    }
}

/// The tag a boolean is written as: a `Byte`, like `putBoolean`.
pub fn boolean_tag(value: bool) -> Tag {
    Tag::Byte(if value { 1 } else { 0 })
}

/// Every compound of the list under `key`, in order. An absent or corrupt list
/// is empty: a building with a broken module list still has its level.
pub fn compounds<'a>(data: &'a Compound, key: &str) -> Vec<&'a Compound> {
    match data.get(&key.into()) {
        Some(tag) => compounds_of(tag),
        None => Vec::new(),
    }
}

/// Every compound of a list tag, in order. The nested case: a list inside a
/// compound inside a list, which is what a cost table is.
pub fn compounds_of<'a>(tag: &'a Tag) -> Vec<&'a Compound> {
    match tag {
        Tag::List(items) => items
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

/// A list of ints, which is how a triple of sizes is written here.
pub fn int_list(values: [i32; 3]) -> Tag {
    Tag::List(values.into_iter().map(Tag::Int).collect())
}

/// The three ints of an [`int_list`]. `None` if the list is short or holds
/// something else.
pub fn ints_of(tag: &Tag) -> Option<[i32; 3]> {
    let Tag::List(items) = tag else {
        return None;
    };
    if items.len() < 3 {
        return None;
    }
    let mut values = [0; 3];
    for (slot, item) in values.iter_mut().zip(items) {
        *slot = number(item)?;
    }
    Some(values)
}

/// The compound without one key. **Test-only**: it exists so a test can say
/// "this save never had that field" without touching a map in place, and no
/// production path drops a key it just wrote.
#[cfg(test)]
pub fn without(data: &Compound, key: &str) -> Compound {
    let mut copy = Compound::new();
    for (held, tag) in data {
        if held.to_string_lossy() != key {
            copy.insert(held.clone(), tag.clone());
        }
    }
    copy
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
        assert_eq!(position_in(&Compound::new(), "parent"), None);
    }

    #[test]
    fn a_boolean_is_a_byte_and_a_wrong_kind_is_none() {
        let data = Compound::from([
            ("isBuilt".into(), boolean_tag(true)),
            ("deconstructed".into(), Tag::Int(1)),
            ("level".into(), Tag::String("3".into())),
        ]);
        assert_eq!(boolean(&data, "isBuilt"), Some(true));
        assert_eq!(boolean(&data, "deconstructed"), Some(true));
        assert_eq!(boolean(&data, "level"), None, "a string is not a boolean");
        assert_eq!(boolean(&data, "absent"), None);
        assert_eq!(int(&data, "level"), None, "nor is a string a number");
    }

    #[test]
    fn a_list_of_records_keeps_its_order_and_a_broken_one_is_empty() {
        let first = Compound::from([("item".into(), Tag::String("minecraft:stone".into()))]);
        let second = Compound::from([("item".into(), Tag::String("minecraft:dirt".into()))]);
        let data = Compound::from([("items".into(), compound_list(vec![first, second]))]);
        assert_eq!(compounds(&data, "items").len(), 2);
        assert!(compounds(&data, "absent").is_empty());
        assert!(compounds(&Compound::from([("items".into(), Tag::Int(1))]), "items").is_empty());
    }

    #[test]
    fn a_triple_of_ints_round_trips() {
        assert_eq!(ints_of(&int_list([3, 7, 3])), Some([3, 7, 3]));
        assert_eq!(ints_of(&Tag::List(vec![Tag::Int(1)])), None);
        assert_eq!(ints_of(&Tag::Int(1)), None);
    }
}
