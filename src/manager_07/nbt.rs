//! The tag reads these four managers share, in Java's own answers.
//!
//! Java reference: `net.minecraft.nbt.CompoundTag`'s `getInt`, `getLong`,
//! `getString`, `getList` and `getCompound` — the five reads
//! `VisitorManager.java`, `StandardDataStoreManager.java`,
//! `StandardRecipeManager.java` and `StandardRequestManager.java` make between
//! them.
//!
//! **This duplicates `crate::requests::nbt` and `crate::persistence::nbt`, on
//! purpose and with the same regret those two already state.** Both are private
//! modules (`src/requests/mod.rs:28`, `src/persistence/mod.rs:47`), so a sibling
//! cannot name them, and widening either would be an edit outside this folder.
//! The tolerance is the same as theirs: a read accepts the three integer widths
//! NBT can hold for a small number, a write always uses the width the model
//! means.
//!
//! **Two absences are not the same value.** `int` answers `None` for an absent
//! key where Java's `getInt` answers `0`, because every caller here has to tell
//! "the file says zero" from "the file says nothing" — a visitor counter of `0`
//! is a bug (see `visitors`), a request version of `0` is an old save.

use mod_api::{Compound, Tag};

/// Whether the key is there at all, which is what Java tests with `contains`
/// and what decides between a default and a value.
pub fn has(data: &Compound, key: &str) -> bool {
    data.contains_key(&key.into())
}

/// `CompoundTag.getInt`: an integer in whichever width it was saved.
pub fn int(data: &Compound, key: &str) -> Option<i32> {
    number(data.get(&key.into())?)
}

/// `CompoundTag.getLong`, widened from the narrower widths the same way `int`
/// narrows nothing: `AbstractTokenFactory` reads two longs and a file saved by
/// hand may hold an int.
pub fn long(data: &Compound, key: &str) -> Option<i64> {
    match data.get(&key.into())? {
        Tag::Long(value) => Some(*value),
        Tag::Int(value) => Some(i64::from(*value)),
        Tag::Short(value) => Some(i64::from(*value)),
        Tag::Byte(value) => Some(i64::from(*value)),
        _ => None,
    }
}

/// The text a tag holds, for a read that starts from a nested compound.
pub fn text_of(tag: &Tag) -> Option<String> {
    match tag {
        Tag::String(value) => Some(value.to_string_lossy()),
        _ => None,
    }
}

/// The compound a key holds, if it holds one.
pub fn compound<'a>(data: &'a Compound, key: &str) -> Option<&'a Compound> {
    match data.get(&key.into())? {
        Tag::Compound(inner) => Some(inner),
        _ => None,
    }
}

/// Every compound of the list under `key`, in order. An absent or corrupt list is
/// empty — `getList` answers an empty list for both, and so does this.
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

/// The entries of the list under `key`, whatever they hold.
///
/// **Why this exists beside `compounds`.** `CompoundTag.getList(key, type)`
/// answers the list only when the *list's declared element type* is that type —
/// NBT lists are homogeneous — so a Java read sees either every element or none.
/// A `Tag` in this SDK carries no declared type
/// (`crates/horizon-core/src/nbt.rs:41-56`) and a mixed list is therefore
/// representable, so a reader that only wanted compounds would drop the odd ones
/// **without a trace**. A reader that wants to report what it dropped walks this
/// and keeps its indices aligned with what was saved.
pub fn list<'a>(data: &'a Compound, key: &str) -> Option<&'a Vec<Tag>> {
    match data.get(&key.into()) {
        Some(Tag::List(items)) => Some(items),
        _ => None,
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
    fn an_absent_key_is_none_and_a_zero_is_a_zero() {
        let data = Compound::from([("next".into(), Tag::Int(0)), ("wide".into(), Tag::Long(7))]);
        assert_eq!(int(&data, "next"), Some(0));
        assert_eq!(int(&data, "absent"), None);
        assert!(has(&data, "next") && !has(&data, "absent"));
        assert_eq!(long(&data, "wide"), Some(7));
        assert_eq!(long(&data, "absent"), None);
    }

    #[test]
    fn a_narrow_width_reads_as_the_wide_one_and_text_is_never_a_number() {
        let data = Compound::from([
            ("byte".into(), Tag::Byte(3)),
            ("name".into(), Tag::Int(1)),
            ("tag".into(), Tag::String("a:b".into())),
        ]);
        assert_eq!((int(&data, "byte"), long(&data, "byte")), (Some(3), Some(3)));
        assert_eq!(text_of(data.get(&"name".into()).expect("just written")), None);
        assert_eq!(text_of(data.get(&"tag".into()).expect("just written")), Some("a:b".to_owned()));
    }

    #[test]
    fn a_list_of_records_keeps_its_order_and_drops_what_is_not_a_record() {
        let one = Compound::from([("id".into(), Tag::Int(1))]);
        let two = Compound::from([("id".into(), Tag::Int(2))]);
        let data = Compound::from([(
            "records".into(),
            Tag::List(vec![Tag::Compound(one), Tag::Int(9), Tag::Compound(two)]),
        )]);
        let read = compounds(&data, "records");
        assert_eq!(read.len(), 2);
        assert_eq!(read[1].get(&"id".into()), Some(&Tag::Int(2)));
        assert!(compounds(&data, "absent").is_empty());
    }

    #[test]
    fn a_compound_is_read_only_when_it_is_one() {
        let inner = Compound::from([("a".into(), Tag::Int(1))]);
        let data = Compound::from([
            ("inner".into(), Tag::Compound(inner.clone())),
            ("flat".into(), Tag::Int(2)),
        ]);
        assert_eq!(compound(&data, "inner"), Some(&inner));
        assert_eq!(compound(&data, "flat"), None);
        assert_eq!(compound_list(vec![inner]), Tag::List(vec![Tag::Compound(
            Compound::from([("a".into(), Tag::Int(1))])
        )]));
    }
}
