//! What a colony **unlocks**, and what it **counts**.
//!
//! Java reference: `com.minecolonies.api.advancements` (16 criterion triggers,
//! 45 files) and the advancement data under
//! `src/datagen/generated/minecolonies/data/minecolonies/advancements/`.
//!
//! **The count, and where it comes from.** The generated data holds **217**
//! advancement JSON files: **155** of them are recipe-unlock books
//! (`advancements/recipes/misc/*.json`) whose only job is to teach the vanilla
//! recipe book a crafting grid, and **62** are the colony advancements this
//! module carries — the number `tree::TREE` is asserted against. Their folders:
//! `minecolonies/` 26, `production/` 26, `military/` 9, `minecraft/` 1.
//! `core/advancements/` does not exist in this revision: the tree lives in data,
//! and the only Java side is the trigger set.
//!
//! **What is ported is the logic, not the trigger.** Java hands an advancement
//! to the player's own `PlayerAdvancements` book and lets the game fire a
//! criterion; a wasm mod has neither. So the port keeps the five things that
//! decide:
//!
//! | File | Question |
//! | --- | --- |
//! | `tree` | which advancement hangs under which, at what frame, on what condition |
//! | `state` | what a colony can be asked, and what happened in it |
//! | `condition` | does that condition hold, and the named refusal when it does not |
//! | `stats` | what the colony counts, day by day, and its compound |
//! | `unlock` | what is unlocked, what unlocks what, and the named refusal |
//!
//! **Two compound conversions, both round-tripped in tests.** [`stats::Stats`]
//! keeps Java's own layout (`stat_manager` → `{id, stat: [{time, quantity}]}`,
//! `StatisticsManager.java:181-205`); [`unlock::Progress`] has no Java
//! counterpart — the mod never saves its advancements, the game does — so it
//! uses the port's one key, `advancements`.
//!
//! **Nothing here calls the host.** No clock either: a day number is a
//! parameter, exactly as `colony.getDay()` is at every Java call site. That is
//! what lets the whole module be tested on the host target with no game.

pub mod condition;
pub mod state;
pub mod stats;
pub mod tree;
pub mod unlock;

pub use condition::{Condition, Refusal};
pub use state::{ColonyState, Event};
pub use stats::Stats;
pub use tree::{Advancement, Frame, Parent, TREE};
pub use unlock::Progress;

/// How many colony advancements the Java data holds, and the number
/// [`TREE`] carries. Kept next to the table so a row added without the count
/// shows up as a failing test rather than as a silent difference.
pub const ADVANCEMENT_COUNT: usize = 62;

/// The criterion triggers `api/advancements/AdvancementTriggers.java:27-42`
/// registers. The tree uses 15 of them, plus vanilla's `placed_block` once
/// (`production/post_and_stash`), which is not one of the 16.
pub const TRIGGER_COUNT: usize = 16;

/// The one trigger no generated advancement references:
/// `open_gui_window` (`Constants.java:76`) is registered
/// (`AdvancementTriggers.java:30`) and unused by the 62 rows.
pub const UNUSED_TRIGGER: &str = "open_gui_window";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tree_holds_exactly_the_advancements_the_data_has() {
        assert_eq!(TREE.len(), ADVANCEMENT_COUNT);
    }
}
