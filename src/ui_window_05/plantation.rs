//! `WindowPlantationField`: the plantation field's window, its two states and its grid.
//!
//! Java reference: `core/client/gui/WindowPlantationField.java` (174 lines, BlockUI +
//! `gui/windowplantationfield.xml`). It is **not** a building window — it extends
//! `AbstractWindowSkeleton`, so no sidebar, no tabs and no `IBuildingView`: every fact it
//! shows comes from the block entity it was handed (`:61`).
//!
//! # One condition, four panes, every tick
//!
//! `updateElementStates` (`:108-116`) reads the colony **once** and sets four panes from it:
//! `not-in-colony` when there is none, and `plants-header`, `plants` and `repair` when there
//! is one. `onUpdate` calls it **every tick** (`:133-138`), so the window flips state while
//! open: place the block outside a colony, found the colony, and the body appears without
//! reopening. [`State::visible`] is that rule, and it is this window's whole gate — Java does
//! not refuse to open it, it opens it and hides the body.
//!
//! # The grid, and the arithmetic Java gets wrong
//!
//! The list is four icons wide (`LIST_PLANTS_COLUMN_COUNT = 4`, `:56`) and the row count is
//! `ceil(plants / 4)` (`:151`). The cell for column `c` (1-based) of row `r` (0-based) is
//!
//! ```text
//! itemIndex = ((index + 1) * id) - 1;      // WindowPlantationField.java:164
//! ```
//!
//! which is **not** a row-major grid: a grid would be `r * 4 + (c - 1)`. Java's formula reads
//! row 0 as `0 1 2 3` and row 1 as `1 3 5 7`, so with eight plants the fifth and seventh are
//! **never drawn** and the second and fourth are drawn twice. This port reproduces it — a
//! silent fix would draw something the Java mod never draws — and [`grid_cell`] is the formula
//! the loop looks like it means, so the divergence is a named, testable fact.
//!
//! # The repair is two windows, not one message
//!
//! `repair` **closes this window** (`:89`) and opens `WindowBuildDecoration` with the block
//! entity's own facts plus a factory that builds a `PlantationFieldBuildRequestMessage` for
//! the builder the player picks (`:90-102`). This window therefore sends **nothing**;
//! [`RepairTarget`] is the five facts, and [`RepairTarget::work_type`] the one word needed.

use super::field::{Pane, Source};
use super::refusal::Refusal;
use super::{Action, Click};
use crate::colony::ordre::WorkType;
use crate::ui::field::Value;

/// The text shown when the block is in no colony (`windowplantationfield.xml:14`).
pub const NOT_IN_COLONY_ID: &str = "not-in-colony";

/// Its label — the key [`Refusal::NotInColony`] carries.
pub const NOT_IN_COLONY_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.fields.notincolony";

/// The header of the plant list (`windowplantationfield.xml:16`).
pub const PLANTS_HEADER_ID: &str = "plants-header";

/// Its label.
pub const PLANTS_HEADER_KEY: &str =
    "com.minecolonies.coremod.gui.workerhuts.plantationfields.plants";

/// The list of plants (`WindowPlantationField.java:41`).
pub const PLANTS_LIST_ID: &str = "plants";

/// The icon prefix inside a row: `icon1`…`icon4` (`:46`, `:159`).
pub const ICON_ID_PREFIX: &str = "icon";

/// The repair button (`:51`, `windowplantationfield.xml:30`).
pub const REPAIR_ID: &str = "repair";

/// Its label (`windowplantationfield.xml:32`).
pub const REPAIR_KEY: &str =
    "com.minecolonies.coremod.gui.workerhuts.plantationfields.repair";

/// The title pane has **no id** (`windowplantationfield.xml:11`), so it is a title, not a pane.
pub const TITLE_KEY: &str = "block.minecolonies.blockhutplantationfield";

/// How many icons a row holds (`:56`).
pub const COLUMNS: usize = 4;

/// The window the repair button opens (`:90`).
pub const BUILD_WINDOW: &str = "WindowBuildDecoration";

/// The message the **decoration** window sends once a builder is picked (`:95-102`).
pub const BUILD_MESSAGE: &str = "PlantationFieldBuildRequestMessage";

/// Whether the block is in a colony, and how many plants the field can be planted with.
///
/// Both are the block entity's: the colony read every update (`:110`), the plants once in the
/// constructor (`:77-80`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    /// `getCurrentColony() != null` (`:112-115`).
    pub in_colony: bool,
    /// `plants.size()` (`:77-80`).
    pub plants: usize,
}

impl State {
    /// Whether the pane `id` is visible; an id this window does not carry is not, by the
    /// XML's default.
    pub fn visible(&self, id: &str) -> bool {
        match id {
            NOT_IN_COLONY_ID => !self.in_colony,
            PLANTS_HEADER_ID | PLANTS_LIST_ID | REPAIR_ID => self.in_colony,
            _ => false,
        }
    }

    /// The rows of the list: `ceil(plants / 4)` (`:151`).
    pub fn rows(&self) -> usize {
        row_count(self.plants)
    }
}

/// `(int) Math.ceil(plants / LIST_PLANTS_COLUMN_COUNT)`.
pub fn row_count(plants: usize) -> usize {
    plants.div_ceil(COLUMNS)
}

/// The plant the icon at `column` (1-based) of `row` shows: Java's formula, kept as it is,
/// with the guard `itemIndex < plants.size()` (`:164-166`).
pub fn cell(row: usize, column: usize, plants: usize) -> Option<usize> {
    let index = (row + 1).checked_mul(column)?.checked_sub(1)?;
    if index < plants { Some(index) } else { None }
}

/// The cell a row-major grid would show — the formula the loop looks like it means.
pub fn grid_cell(row: usize, column: usize) -> Option<usize> {
    let column = column.checked_sub(1)?;
    row.checked_mul(COLUMNS)?.checked_add(column)
}

/// The pane ids of one row, in column order.
pub fn icon_ids() -> [String; COLUMNS] {
    [
        format!("{ICON_ID_PREFIX}1"),
        format!("{ICON_ID_PREFIX}2"),
        format!("{ICON_ID_PREFIX}3"),
        format!("{ICON_ID_PREFIX}4"),
    ]
}

/// What the repair button carries to the decoration window: the block's own facts, plus the
/// work type.
///
/// Java passes six arguments; the **dimension** is the sixth and it is the game's, not the
/// block's — this port has no level handle, so it is not carried.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepairTarget {
    /// `getBlockPos()`.
    pub at: [i32; 3],
    /// `getPackName()`.
    pub pack: &'static str,
    /// `getBlueprintPath()`.
    pub path: &'static str,
    /// `getRotation()`, as `net.minecraft.world.level.block.Rotation`'s ordinal.
    pub rotation: u8,
    /// `getMirror()`.
    pub mirror: bool,
}

impl RepairTarget {
    /// `WorkOrderType.REPAIR` (`:95`).
    pub fn work_type(self) -> WorkType {
        WorkType::Repair
    }

    /// The class the decoration window sends for this target (`:95-102`).
    pub fn message(self) -> &'static str {
        BUILD_MESSAGE
    }
}

/// Whether the repair button may be clicked — hidden, not refused, outside a colony (`:115`).
pub fn may_repair(state: &State) -> Result<(), Refusal> {
    if state.in_colony { Ok(()) } else { Err(Refusal::NotInColony) }
}

/// The panes: **four** — three of the window and one row of its list.
///
/// `updateElementStates` also toggles `repair` (`:112-115`), which is an [`Action`] here: a
/// [`Pane`] is a pane a class **fills**, and no class writes that button.
/// [`State::visible`] still answers for it.
pub const PANES: &[Pane] = &[
    Pane::labelled(NOT_IN_COLONY_ID, NOT_IN_COLONY_KEY, Value::Text, Source::COLONY),
    Pane::labelled(PLANTS_HEADER_ID, PLANTS_HEADER_KEY, Value::Text, Source::COLONY),
    Pane::new(PLANTS_LIST_ID, Value::Row, Source::TileEntity),
    Pane::in_row(PLANTS_LIST_ID, ICON_ID_PREFIX, Value::Icon, Source::TileEntity),
];

/// The one button: it closes this window and opens the decoration window.
pub const ACTIONS: &[Action] = &[Action::new(
    REPAIR_ID,
    REPAIR_KEY,
    Click::ClosesThenOpens(BUILD_WINDOW),
    &[Refusal::NotInColony],
)];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_panes_and_a_button_flip_on_one_condition() {
        let outside = State { in_colony: false, plants: 3 };
        let inside = State { in_colony: true, plants: 3 };
        assert!(outside.visible(NOT_IN_COLONY_ID));
        assert!(!outside.visible(PLANTS_HEADER_ID));
        assert!(!outside.visible(PLANTS_LIST_ID));
        assert!(!outside.visible(REPAIR_ID));
        assert!(!inside.visible(NOT_IN_COLONY_ID));
        assert!(inside.visible(PLANTS_HEADER_ID));
        assert!(inside.visible(PLANTS_LIST_ID));
        assert!(inside.visible(REPAIR_ID), "the button is one of the four");
        assert!(!inside.visible("plants-header-typo"));
        // Three window panes and one row.
        assert_eq!(PANES.len(), 4);
        assert_eq!(PANES[2].id, PLANTS_LIST_ID);
        assert_eq!(PANES[2].value, Value::Row);
        assert!(PANES[3].is_row_pane());
        assert_eq!(ACTIONS.len(), 1, "the repair button is the only one");
        assert_eq!(may_repair(&outside), Err(Refusal::NotInColony));
        assert_eq!(may_repair(&inside), Ok(()));
    }

    #[test]
    fn the_rows_are_the_plants_divided_by_four_rounded_up() {
        assert_eq!(row_count(0), 0);
        assert_eq!(row_count(1), 1);
        assert_eq!(row_count(4), 1);
        assert_eq!(row_count(5), 2, "one plant on a second row");
        assert_eq!(row_count(9), 3);
        assert_eq!(State { in_colony: true, plants: 5 }.rows(), 2);
        assert_eq!(COLUMNS, 4);
        assert_eq!(icon_ids(), ["icon1", "icon2", "icon3", "icon4"]);
    }

    #[test]
    fn the_cell_formula_is_javas_and_not_a_grid() {
        // Row 0 is a plain row in both formulas.
        for column in 1..=COLUMNS {
            assert_eq!(cell(0, column, 64), grid_cell(0, column));
        }
        assert_eq!((cell(0, 1, 64), cell(0, 4, 64)), (Some(0), Some(3)));
        // Row 1 diverges: Java reads 1, 3, 5, 7 where a grid reads 4, 5, 6, 7.
        let java: Vec<Option<usize>> = (1..=COLUMNS).map(|c| cell(1, c, 64)).collect();
        assert_eq!(java, [Some(1), Some(3), Some(5), Some(7)]);
        let grid: Vec<Option<usize>> = (1..=COLUMNS).map(|c| grid_cell(1, c)).collect();
        assert_eq!(grid, [Some(4), Some(5), Some(6), Some(7)]);
        // With eight plants two are never drawn and two are drawn twice.
        let drawn: Vec<usize> = (0..2)
            .flat_map(|row| (1..=COLUMNS).filter_map(move |column| cell(row, column, 8)))
            .collect();
        assert!(!drawn.contains(&4) && !drawn.contains(&6));
        assert_eq!(drawn.iter().filter(|index| **index == 1).count(), 2);
        // The guard Java applies, and the column bound.
        assert_eq!(cell(3, 4, 8), None, "15 is past the eight plants Java has");
        assert_eq!(cell(2, 4, 12), Some(11));
        assert_eq!(grid_cell(0, 0), None, "the loop's columns start at 1");
    }

    #[test]
    fn the_repair_button_closes_and_opens_and_sends_nothing_itself() {
        let action = &ACTIONS[0];
        assert_eq!((action.id, action.label), (REPAIR_ID, REPAIR_KEY));
        assert_eq!(action.click, Click::ClosesThenOpens(BUILD_WINDOW));
        assert_eq!(action.refusals, [Refusal::NotInColony]);
        // The message is the decoration window's, and its type is REPAIR.
        let target = RepairTarget {
            at: [12, 64, -8],
            pack: "minecolonies",
            path: "plantationfield/level1",
            rotation: 1,
            mirror: false,
        };
        assert_eq!(target.work_type(), WorkType::Repair);
        assert_eq!(target.message(), "PlantationFieldBuildRequestMessage");
        assert!(!matches!(action.click, Click::One(crate::ui::action::Effect::Sends(_))));
        // The title has no id, so it is not a pane.
        assert!(!PANES.iter().any(|pane| pane.id.is_empty()));
    }
}
