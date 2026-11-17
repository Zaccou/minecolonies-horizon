//! The four tag reads `manager_06` shares, and why this is a fifth copy.
//!
//! `crate::colony::nbt` is `mod nbt;` inside `crate::colony`, so it is private to
//! that module; `src/building/nbt.rs`, `src/requests/nbt.rs` and
//! `src/manager_04/nbt.rs` already carry the same copy and say why. This is the one
//! this module needs, and it is here for the same reason: these managers are
//! written against tag helpers without editing a module another agent owns. The
//! copies are byte-for-byte compatible on what they share; the day one of them
//! becomes public, five of them go.
//!
//! **Two position layouts, because Java has two.** `BlockPosUtil.write` spells a
//! position `x`/`y`/`z` (`api/util/BlockPosUtil.java:219-226`), which is what
//! `crate::colony::nbt::position` writes. `NbtUtils.writeBlockPos` — the one
//! `TravellingManager` uses (`TravellingManager.java:207`) — spells it `X`/`Y`/`Z`.
//! [`java_position`] and [`java_position_in`] are that second layout, and the read
//! accepts the lowercase spelling too: the two are the same three numbers and a
//! save is not worth losing over the case of a key.
//!
//! **Tolerant reads, strict writes**, like the other four copies: `int` accepts the
//! three integer widths NBT can hold for a small number, and `text` refuses
//! anything that is not a string rather than returning a debug form.

use mod_api::{Compound, Tag};

/// A position as `BlockPosUtil` lays one out: three ints under `x`, `y`, `z`
/// (`api/util/BlockPosUtil.java:219-226`).
pub fn position(at: [i32; 3]) -> Tag {
    Tag::compound([
        ("x", Tag::Int(at[0])),
        ("y", Tag::Int(at[1])),
        ("z", Tag::Int(at[2])),
    ])
}

/// A position as `NbtUtils.writeBlockPos` lays one out: `X`, `Y`, `Z`
/// (`TravellingManager.java:207`).
pub fn java_position(at: [i32; 3]) -> Tag {
    Tag::compound([
        ("X", Tag::Int(at[0])),
        ("Y", Tag::Int(at[1])),
        ("Z", Tag::Int(at[2])),
    ])
}

/// The position a tag holds, in either spelling.
pub fn position_of(tag: &Tag) -> Option<[i32; 3]> {
    let axis = |upper: &str, lower: &str| {
        tag.get(upper)
            .or_else(|| tag.get(lower))
            .and_then(number)
    };
    Some([axis("X", "x")?, axis("Y", "y")?, axis("Z", "z")?])
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

/// The raw items of the list under `key`, compounds or not — the read a caller
/// needs when it has to **name** an item that is not a compound instead of
/// dropping it silently.
pub fn items<'a>(data: &'a Compound, key: &str) -> &'a [Tag] {
    match data.get(&key.into()) {
        Some(Tag::List(items)) => items.as_slice(),
        _ => &[],
    }
}

/// A list of compounds: NBT lists are homogeneous
/// (`crates/horizon-core/src/nbt.rs:52`), so a list of records is a `List<Compound>`.
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

    /// Both spellings read back, and a missing coordinate is `None` rather than
    /// zero: a traveller at the origin is not a traveller without a target.
    #[test]
    fn the_two_position_layouts_read_back_and_a_partial_one_is_none() {
        assert_eq!(position_of(&java_position([-3, 71, 1208])), Some([-3, 71, 1208]));
        assert_eq!(position_of(&position([4, 5, 6])), Some([4, 5, 6]));
        let halved = Tag::compound([("X", Tag::Int(4)), ("Z", Tag::Int(9))]);
        assert_eq!(position_of(&halved), None);
        assert_eq!(position_in(&Compound::new(), "target"), None);
    }

    /// `int` reads the three widths; `text` refuses a number; `items` keeps the
    /// non-compounds so the caller can name them.
    #[test]
    fn the_scalars_are_tolerant_and_the_list_keeps_what_it_holds() {
        let data = Compound::from([
            ("wide".into(), Tag::Long(7)),
            ("byte".into(), Tag::Byte(-2)),
            ("short".into(), Tag::Short(300)),
            ("text".into(), Tag::String("hi".into())),
            ("mixed".into(), Tag::List(vec![Tag::Int(1), Tag::String("x".into())])),
        ]);
        assert_eq!(int(&data, "wide"), None, "a long is not read as an int");
        assert_eq!(int(&data, "byte"), Some(-2));
        assert_eq!(int(&data, "short"), Some(300));
        assert_eq!(text(&data, "text"), Some("hi".to_owned()));
        assert_eq!(text(&data, "byte"), None);
        assert_eq!(items(&data, "mixed").len(), 2);
        assert!(items(&data, "absent").is_empty());

        let listed = Compound::from([(
            "l".into(),
            compound_list(vec![Compound::from([("id".into(), Tag::Int(3))])]),
        )]);
        assert_eq!(items(&listed, "l").len(), 1);
    }
}
