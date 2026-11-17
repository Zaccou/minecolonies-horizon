//! The tag shapes the two extension payloads are saved in: a position, a list of
//! positions, and an int array.
//!
//! **What this file is not.** Reads go through `crate::blueprint::nbt`'s
//! `tag`/`as_text`/`as_i64`, which are public and answer every integer width and
//! every string the same way (`src/blueprint/nbt.rs:499-533`). That module is the
//! blueprint *byte* layer and has no writer for a position, which is what is here.
//!
//! **Three private copies of a position writer already exist** — `src/colony/nbt.rs:17-25`,
//! `src/building/nbt.rs:20-32` and five more in the tree — and this is the fourth.
//! The reason is the same one `src/building/nbt.rs:8-12` gives: `mod nbt;` inside
//! each of those domains, so nothing outside can reach them, and this agent may
//! not widen three other modules. The report says so; the fix is one `pub(crate)
//! mod nbt;` and the deletion of three copies, not a fourth one written quietly.
//!
//! **The layout is `BlockPosUtil`'s**, and that is the whole of it: three ints
//! under `x`, `y`, `z` (`api/util/BlockPosUtil.java:190-197`, written at
//! `:219-226` for one position and `:235-243` for a list). `BlockPosUtil.read` is
//! **tolerant**: an absent compound reads as `(0, 0, 0)` because
//! `CompoundTag.getCompound` answers an empty one and `getInt` answers 0
//! (`:190-197`). [`position_in`] keeps that tolerance and says so.

use crate::blueprint::nbt::{as_i64, tag};
use mod_api::{Compound, Tag};

/// A position, as `BlockPosUtil.write` lays one out (`:219-226`).
pub fn position(at: [i32; 3]) -> Tag {
    Tag::compound([
        ("x", Tag::Int(at[0])),
        ("y", Tag::Int(at[1])),
        ("z", Tag::Int(at[2])),
    ])
}

/// The position a tag holds, or `None` when a coordinate is missing. **Not
/// tolerant**: a caller that means Java's `read` uses [`position_in`], which is.
pub fn position_of(value: &Tag) -> Option<[i32; 3]> {
    let Tag::Compound(coords) = value else {
        return None;
    };
    Some([
        i32::try_from(as_i64(tag(coords, "x")?)?).ok()?,
        i32::try_from(as_i64(tag(coords, "y")?)?).ok()?,
        i32::try_from(as_i64(tag(coords, "z")?)?).ok()?,
    ])
}

/// `BlockPosUtil.read(compound, key)` (`:190-197`): the position under `key`, and
/// **(0, 0, 0) when it is absent or unreadable**. That is Java's answer, not a
/// fallback this port invented: `getCompound` on a missing key is an empty
/// compound and `getInt` on a missing key is 0.
pub fn position_in(data: &Compound, key: &str) -> [i32; 3] {
    tag(data, key).and_then(position_of).unwrap_or([0, 0, 0])
}

/// `BlockPosUtil.writePosListToNBT` (`:235-243`): a list of compounds, each one
/// position, in the order given.
pub fn positions(entries: &[[i32; 3]]) -> Tag {
    Tag::List(entries.iter().map(|at| position(*at)).collect())
}

/// `BlockPosUtil.readPosListFromNBT` (`:251-262`): every compound of the list, in
/// file order, each read as a position. A list holding something that is not a
/// position contributes nothing rather than a wrong coordinate.
pub fn positions_in(data: &Compound, key: &str) -> Vec<[i32; 3]> {
    match tag(data, key) {
        Some(Tag::List(items)) => items.iter().filter_map(position_of).collect(),
        _ => Vec::new(),
    }
}

/// `CompoundTag.putIntArray`: the tag `FarmField.serializeNBT` writes the radii
/// with (`core/colony/buildingextensions/FarmField.java:108`).
pub fn int_array(values: &[i32]) -> Tag {
    Tag::IntArray(values.to_vec())
}

/// `CompoundTag.getIntArray` (`FarmField.java:118`): the array under `key`, or
/// `None` when the tag is not an int array — which is **not** the same as an empty
/// array, and Java loses that difference (see [`super::building_extension`]).
pub fn ints_in(data: &Compound, key: &str) -> Option<Vec<i32>> {
    match tag(data, key) {
        Some(Tag::IntArray(values)) => Some(values.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_position_round_trips() {
        let at = [10, 64, -3];
        assert_eq!(position_of(&position(at)), Some(at));
    }

    #[test]
    fn an_absent_position_reads_as_the_origin_like_block_pos_util() {
        // `BlockPosUtil.java:190-197`: `getCompound` on a missing key is empty and
        // `getInt` on a missing key is 0, so the answer is (0, 0, 0) and not a
        // refusal. The port keeps it, and every caller that needs "absent" to mean
        // something else asks with `tag` first.
        assert_eq!(position_in(&Compound::new(), "position"), [0, 0, 0]);
        assert_eq!(position_of(&Tag::End), None);
    }

    #[test]
    fn a_position_list_keeps_its_order_and_drops_nothing_it_can_read() {
        let entries = [[1, 2, 3], [-4, 70, 8]];
        let data = Compound::from([("workingPositions".into(), positions(&entries))]);
        assert_eq!(positions_in(&data, "workingPositions"), entries);
        // A list of something else is empty, not a list of origins.
        let junk = Compound::from([("workingPositions".into(), Tag::List(vec![Tag::Int(3)]))]);
        assert_eq!(positions_in(&junk, "workingPositions"), Vec::<[i32; 3]>::new());
    }

    #[test]
    fn an_int_array_is_a_tag_and_its_absence_is_not_an_empty_array() {
        let data = Compound::from([("radius".into(), int_array(&[5, 5, 5, 5]))]);
        assert_eq!(ints_in(&data, "radius"), Some(vec![5, 5, 5, 5]));
        assert_eq!(ints_in(&data, "seeds"), None);
        // `CompoundTag.getIntArray` on a tag of another kind is an empty array;
        // this keeps the difference and the caller names it.
        let wrong = Compound::from([("radius".into(), Tag::Int(5))]);
        assert_eq!(ints_in(&wrong, "radius"), None);
    }
}
