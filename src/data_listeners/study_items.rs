//! `study_items/*.json`: what the library desk accepts, and two percentages that
//! are clamped rather than refused.
//!
//! Java reference: `core/datalistener/StudyItemListener.java` (132 lines), read by
//! `BuildingLibrary.java:53` (a `keepX` filter, 64 items) and
//! `EntityAIStudy.java:124` (the study action). Two files ship.
//!
//! **The clamp is deliberate and the port keeps it.** Both chances are read with
//! a default of 0 (`:127-128`) and clamped to `0..=100` with a warning naming the
//! field, the raw value and the clamped one (`:131-143`). That is Java choosing to
//! accept a wrong value, so [`percent`] returns the clamped value *and* whether it
//! had to clamp — the caller logs, nothing is refused.
//!
//! **The item is not checked, and that is a hole.** `BuiltInRegistries.ITEM.get`
//! returns `Items.AIR` for an id no registry knows (`:120`), and the row is stored
//! under its **file's** id anyway (`:146`). `isStudyItem` then asks
//! `ITEM.getKey(stack.getItem())` (`:84-88`), which for air is
//! `minecraft:air` — a key the file never claims. So the row exists, is listed by
//! `getAllStudyItems`, and can never match a stack: a silent no-op where the
//! author meant a desk that accepts something. [`Refusal::UnknownItem`] refuses it
//! instead.
//!
//! **What is not here**: `BuildingLibrary`'s 64-item cap and the `keepX` flag (the
//! building's rule, `:53`), and the skill roll `EntityAIStudy` makes — the port
//! takes the two chances and lets the caller decide, because a wasm mod that rolled
//! its own could not be replayed.

use super::Refused;

/// What the library does with one accepted item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three fields of `StudyItem` (`:66-70`)
pub struct StudyItem {
    /// The item, as the JSON spelled it.
    pub item: &'static str,
    /// `skill_increase_chance`, 0 to 100 (`:127`).
    pub skill_increase_chance: u8,
    /// `break_chance`, 0 to 100 (`:128`).
    pub break_chance: u8,
}

/// Why a study-item row is refused, named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// An item id no registry knows: Java stores an air entry that can never match
    /// (`:120`, `:84-88`).
    UnknownItem { id: &'static str, item: &'static str },
}

impl Refused for Refusal {
    fn name(&self) -> String {
        match *self {
            Refusal::UnknownItem { id, item } => {
                format!("data_listeners.refusal.unknown_study_item({id}.{item})")
            }
        }
    }
}

/// `Mth.clamp(raw, 0, 100)` (`:132`) with the flag Java only writes to a log.
///
/// The second value is `true` when the input was outside the range, i.e. when
/// `:134-142` warned.
pub fn percent(raw: i32) -> (u8, bool) {
    if raw < 0 {
        (0, true)
    } else if raw > 100 {
        (100, true)
    } else {
        ((raw as u8), false)
    }
}

/// Reads one row, `known_item` being the registry's answer.
pub fn parse(
    id: &'static str,
    item: &'static str,
    skill_increase: i32,
    break_chance: i32,
    known_item: &dyn Fn(&str) -> bool,
) -> Result<StudyItem, Refusal> {
    if !known_item(item) {
        return Err(Refusal::UnknownItem { id, item });
    }
    Ok(StudyItem {
        item,
        skill_increase_chance: percent(skill_increase).0,
        break_chance: percent(break_chance).0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known(_: &str) -> bool {
        true
    }

    #[test]
    fn a_percentage_outside_zero_to_a_hundred_is_clamped_and_flagged() {
        assert_eq!(percent(0), (0, false));
        assert_eq!(percent(100), (100, false));
        assert_eq!(percent(150), (100, true));
        assert_eq!(percent(-5), (0, true));
        // Java's default for an absent field is 0, not 100 (`:127-128`).
        assert_eq!(percent(0).0, 0);
    }

    #[test]
    fn both_chances_are_clamped_when_the_row_is_read() {
        let row = parse("study/bookshelf", "minecraft:book", 250, -1, &known).expect("valid");
        assert_eq!(
            row,
            StudyItem {
                item: "minecraft:book",
                skill_increase_chance: 100,
                break_chance: 0
            }
        );
    }

    #[test]
    fn an_unknown_item_is_refused_instead_of_becoming_a_row_that_never_matches() {
        let refusal = parse("study/x", "mod:gone", 10, 10, &|_| false).expect_err("unknown item");
        assert_eq!(
            refusal.name(),
            "data_listeners.refusal.unknown_study_item(study/x.mod:gone)"
        );
    }
}
