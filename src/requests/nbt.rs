//! The tag reads and writes this module's two compounds share.
//!
//! `Compound` is `BTreeMap<JavaString, Tag>` and `Tag` the NBT value
//! (`crates/horizon-core/src/nbt.rs:35`, `:41-56`), both re-exported by the API
//! (`crates/horizon-mod-vocab/src/exports/horizon_world.rs:7-8`). The crate has
//! one dependency (`Cargo.toml`), so a serialisable model here is a pair of
//! functions, never a `Serialize` derive.
//!
//! **This duplicates `colony/nbt.rs`, on purpose and with regret.** `colony::nbt`
//! is a private module (`src/colony/mod.rs:28`, `mod nbt;`), so a sibling cannot
//! name it, and widening it would be a change outside `src/requests/`. Same
//! tolerance: a read accepts the three integer widths NBT can hold for a small
//! number, a write always uses the width the model means.
//!
//! **Why the `&Tag` forms.** A request's requester is a compound of its own
//! (`StandardRequestFactories` writes it nested), so its position and its kind are
//! read from a `Tag`, not from the root compound.

use mod_api::{Compound, Tag};

/// A position, laid out as `BlockPosUtil` lays one out: three ints under `x`, `y`,
/// `z` (`api/util/BlockPosUtil.java:190-197`).
pub fn position(at: [i32; 3]) -> Tag {
    Tag::compound([("x", Tag::Int(at[0])), ("y", Tag::Int(at[1])), ("z", Tag::Int(at[2]))])
}

/// The position a tag holds. A missing coordinate is `None`, not zero: an order
/// without a position is not an order at the origin.
pub fn position_of(tag: &Tag) -> Option<[i32; 3]> {
    Some([number(tag.get("x")?)?, number(tag.get("y")?)?, number(tag.get("z")?)?])
}

/// The position stored under `key`.
pub fn position_in(data: &Compound, key: &str) -> Option<[i32; 3]> {
    position_of(data.get(&key.into())?)
}

/// An integer under `key`, in whichever width it was saved.
pub fn int(data: &Compound, key: &str) -> Option<i32> {
    number(data.get(&key.into())?)
}

/// A string a tag holds. A tag of another kind is `None`, not its debug form.
pub fn text_of(tag: &Tag) -> Option<String> {
    match tag {
        Tag::String(value) => Some(value.to_string_lossy()),
        _ => None,
    }
}

/// Every compound of the list under `key`, in order. An absent or corrupt list is
/// empty: a queue whose list is broken is an empty queue, not no queue.
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

/// A list of compounds: NBT lists are homogeneous (`crates/horizon-core/src/nbt.rs:52`),
/// so a list of records is a `List<Compound>` and nothing else.
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
    fn a_position_round_trips_and_a_partial_one_is_refused() {
        let data = Compound::from([("at".into(), position([-3, 71, 1208]))]);
        assert_eq!(position_in(&data, "at"), Some([-3, 71, 1208]));
        assert_eq!(position_of(&Tag::compound([("x", Tag::Int(4)), ("z", Tag::Int(9))])), None);
        assert_eq!(position_in(&Compound::new(), "at"), None);
    }

    #[test]
    fn a_byte_reads_as_an_int_and_an_int_is_not_text() {
        let data = Compound::from([("level".into(), Tag::Byte(3)), ("name".into(), Tag::Int(1))]);
        assert_eq!((int(&data, "level"), int(&data, "absent")), (Some(3), None));
        assert_eq!(text_of(data.get(&"name".into()).expect("just written")), None);
        assert_eq!(text_of(&Tag::String("hut".into())), Some("hut".to_owned()));
        let requests = Compound::from([("list".into(), compound_list(vec![data.clone()]))]);
        assert_eq!(compounds(&requests, "list").len(), 1);
        assert!(compounds(&requests, "absent").is_empty());
    }
}
