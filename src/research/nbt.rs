//! The tag reads and writes [`super::progress`] uses.
//!
//! `Compound` is `BTreeMap<JavaString, Tag>`, `Tag` the NBT value
//! (`crates/horizon-core/src/nbt.rs:35`, `:41-56`), both re-exported by the API
//! (`crates/horizon-mod-vocab/src/exports/horizon_world.rs:7-8`). The crate has
//! one dependency, `horizon-mod-api` (`Cargo.toml`), so a serialisable model here
//! is a pair of functions, not a `Serialize` derive.
//!
//! **This is the third copy of the same four functions.** `src/colony/nbt.rs`
//! and `src/building/nbt.rs` each carry their own, and `building` says why in its
//! own header (`src/building/nbt.rs:9-13`): they are `mod nbt;`, not
//! `pub(crate)`, so a module in another folder cannot reach them. Making one of
//! them `pub(crate)` is a one-line change to a file this agent does not own; the
//! report says so rather than duplicating it quietly. What is kept here is only
//! what [`super::progress`] reads and writes: an int, a string, a list of
//! compounds, and a way to write a list of compounds back.
//!
//! **Tolerant reads, strict writes.** A read accepts the three integer widths NBT
//! can hold for a small number, so a `/data merge` written by hand stays
//! readable; a write always uses the width the model means.

use mod_api::{Compound, Tag};

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
/// is empty: a colony with a broken research list still has its other records.
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
    fn an_int_reads_in_three_widths_and_nothing_else() {
        let data = Compound::from([
            ("a".into(), Tag::Int(5)),
            ("b".into(), Tag::Byte(5)),
            ("c".into(), Tag::Short(5)),
            ("d".into(), Tag::String("5".into())),
        ]);
        assert_eq!(int(&data, "a"), Some(5));
        assert_eq!(int(&data, "b"), Some(5));
        assert_eq!(int(&data, "c"), Some(5));
        assert_eq!(int(&data, "d"), None, "a string is not a number");
        assert_eq!(int(&data, "e"), None, "an absent key is not zero");
    }

    #[test]
    fn a_string_reads_and_a_number_does_not() {
        let data = Compound::from([
            ("a".into(), Tag::String("civilian".into())),
            ("b".into(), Tag::Int(3)),
        ]);
        assert_eq!(text(&data, "a").as_deref(), Some("civilian"));
        assert_eq!(text(&data, "b"), None);
        assert_eq!(text(&data, "c"), None);
    }

    #[test]
    fn a_list_of_compounds_round_trips_and_skips_what_is_not_one() {
        let record = Compound::from([("id".into(), Tag::String("civilian/keen".into()))]);
        let list = Compound::from([(
            "records".into(),
            Tag::List(vec![
                Tag::Compound(record),
                Tag::Int(3),
                Tag::Compound(Compound::new()),
            ]),
        )]);
        assert_eq!(compounds(&list, "records").len(), 2, "the int is skipped");
        let written = Compound::from([(
            "records".into(),
            compound_list(vec![Compound::new(), Compound::new()]),
        )]);
        assert_eq!(compounds(&written, "records").len(), 2);
    }

    #[test]
    fn an_absent_or_wrong_list_is_empty_not_a_panic() {
        let data = Compound::from([("records".into(), Tag::String("no".into()))]);
        assert!(compounds(&data, "records").is_empty());
        assert!(compounds(&Compound::new(), "records").is_empty());
    }
}
