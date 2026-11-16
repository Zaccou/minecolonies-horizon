//! The tag reads and writes the colony model shares.
//!
//! `Compound` is `BTreeMap<JavaString, Tag>`, `Tag` the NBT value
//! (`crates/horizon-core/src/nbt.rs:35`, `:41-56`), both re-exported by the API
//! (`crates/horizon-mod-vocab/src/exports/horizon_world.rs:7-8`). The crate has
//! one dependency, `horizon-mod-api` (`Cargo.toml`), so a serialisable model
//! here is a pair of functions, not a `Serialize` derive.
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

/// The position a tag holds. A missing coordinate is `None`, not zero: an order
/// without a position is not an order at the origin.
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

/// Every compound of the list under `key`, in order. An absent or corrupt list
/// is empty: a colony with a broken citizen list still has its id.
pub fn compounds<'a>(data: &'a Compound, key: &str) -> Vec<&'a Compound> {
    match data.get(&key.into()) {
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
        assert_eq!(position_in(&Compound::new(), "center"), None);
    }

    #[test]
    fn a_byte_reads_as_the_same_integer_and_other_kinds_do_not_read_as_text() {
        let data = Compound::from([
            ("level".into(), Tag::Byte(3)),
            ("name".into(), Tag::Int(1)),
        ]);
        assert_eq!(int(&data, "level"), Some(3));
        assert_eq!(int(&data, "absent"), None);
        assert_eq!(text(&data, "name"), None);
    }

    #[test]
    fn a_list_of_records_keeps_its_order_and_forgets_nothing_else() {
        let first = Compound::from([("id".into(), Tag::Int(1))]);
        let second = Compound::from([("id".into(), Tag::Int(2))]);
        let data = Compound::from([("citizens".into(), compound_list(vec![first, second]))]);
        let read = compounds(&data, "citizens");
        assert_eq!(read.len(), 2);
        assert_eq!(int(read[1], "id"), Some(2));
        assert!(compounds(&data, "buildings").is_empty());
    }
}
