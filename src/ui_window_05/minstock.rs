//! `WindowPostBoxMinStock`: the postbox's minimum stock — **one behaviour, two resources**.
//!
//! Java reference: `core/client/gui/WindowPostBoxMinStock.java` (29 lines), a subclass of
//! `core/client/gui/modules/building/MinimumStockModuleWindow.java` (149 lines) that does three
//! things (`:18-28`): it passes `gui/windowpostboxminstock.xml` instead of
//! `gui/layouthuts/layoutminimumstock.xml`, it re-registers the postbox's tabs, and it answers
//! `false` to `shouldRenderDefaultSidebar()`.
//!
//! # Three things worth reading closely
//!
//! 1. **Two XMLs, the same pane ids.** Both declare `resourcesstock`, `resourceIcon`,
//!    `resourceName`, `resourceQty`, `removeStock` and `addStock`
//!    (`layoutminimumstock.xml:5-20`, `windowpostboxminstock.xml:8-24`); only the positions
//!    differ, 190 wide against 440. So this is **not a second screen**: one behaviour with a
//!    second layout.
//! 2. **"No default sidebar" does not mean "no tabs".** The class answers `false` (`:24-28`)
//!    and immediately calls `WindowPostBoxMain.registerPostboxTabs(this, …)` (`:21`) — the very
//!    loop the default sidebar is (`AbstractBuildingWindow.java:57-85`), rewritten in
//!    `WindowPostBoxMain.java:207-237` so a non-building window can show it.
//! 3. **The flag is about the parent**: the window keeps `AbstractModuleWindow`'s description
//!    header (`AbstractModuleWindow.java:57-66`) and replaces the tab strip it would otherwise
//!    inherit.
//!
//! # Two rules for "may I add", and they disagree on purpose
//!
//! The **window** asks the view one boolean, `hasReachedLimit()` (`MinimumStockModuleWindow.java:64`,
//! `:93`), and that boolean is read off the wire rather than computed
//! (`MinimumStockModuleView.java:45`, `:66-69`). The **server** asks another question
//! (`MinimumStockModule.java:64-70`): an item **already listed** is always accepted, a new one
//! only while the list is shorter than `(int)(level * 5 * (1 + research))` (`:44`, `:56-60`). A
//! full list refuses a new item through the window *and* would still take a re-add of an
//! existing one — which the window never lets through, since the add button is relabelled.
//!
//! # An add replaces, it does not sum
//!
//! `minimumStock.put(new ItemStorage(itemStack), quantity)` (`:66`) is a map write, so adding the
//! same item twice leaves the second quantity. And a removal is written **locally first** and
//! sent afterwards (`MinimumStockModuleWindow.java:79-83`).

use super::field::{Pane, Source};
use super::refusal::Refusal;
use super::{Action, Click};
use crate::ui::action::Effect;
use crate::ui::field::Value;

/// The stock list (`MinimumStockModuleWindow.java:56`).
pub const LIST_ID: &str = "resourcesstock";

/// An item's icon inside a row (`layoutminimumstock.xml:12`).
pub const ROW_ICON_ID: &str = "resourceIcon";

/// An item's display name inside a row (`layoutminimumstock.xml:13`).
pub const ROW_NAME_ID: &str = "resourceName";

/// The quantity the player keeps in stock (`WindowConstants.java:513`).
pub const ROW_QTY_ID: &str = "resourceQty";

/// The add button (`WindowConstants.java:511`).
pub const ADD_ID: &str = "addStock";

/// The remove button inside a row (`WindowConstants.java:512`).
pub const REMOVE_ID: &str = "removeStock";

/// The add button's label (`layoutminimumstock.xml:19`).
pub const ADD_KEY: &str = "com.minecolonies.coremod.gui.warehouse.add";

/// The label it takes once the ceiling is reached (`MinimumStockModuleWindow.java:49`, `:61-66`).
pub const LIMIT_KEY: &str = "com.minecolonies.coremod.gui.warehouse.limitreached";

/// The module's description, painted in the header (`MinimumStockModuleView.java:74-77`).
pub const STOCK_KEY: &str = "com.minecolonies.coremod.gui.warehouse.stock";

/// The list's empty text (`layoutminimumstock.xml:7`).
pub const EMPTY_KEY: &str = "com.minecolonies.coremod.gui.warehouse.stock.empty";

/// How many entries a level buys, before research (`MinimumStockModule.java:44`).
pub const STOCK_PER_LEVEL: i32 = 5;

/// The module window's own resource (`MinimumStockModuleWindow.java:51`).
pub const MODULE_RESOURCE: &str = "minecolonies:gui/layouthuts/layoutminimumstock.xml";

/// The postbox's resource for the same behaviour (`WindowPostBoxMinStock.java:20`).
pub const POSTBOX_RESOURCE: &str = "minecolonies:gui/windowpostboxminstock.xml";

/// The message a removal sends (`MinimumStockModuleWindow.java:83`).
pub const REMOVE_MESSAGE: &str = "RemoveMinimumStockFromBuildingModuleMessage";

/// The window the add button opens to pick an item (`MinimumStockModuleWindow.java:95`).
pub const PICKER_WINDOW: &str = "WindowSelectRes";

/// One row: which item, and how many the player wants kept (Java's `Tuple<ItemStorage, Integer>`,
/// `IMinimumStockModuleView.java:20`, shown at `MinimumStockModuleWindow.java:141-145`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    /// The item's registry name, which is what `ItemStorage` compares on.
    pub item: &'static str,
    /// The quantity the player asked to keep.
    pub quantity: i32,
}

impl Row {
    /// A row.
    pub const fn new(item: &'static str, quantity: i32) -> Row {
        Row { item, quantity }
    }
}

/// The stock list, with the two transitions a click can cause.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stock {
    rows: Vec<Row>,
}

impl Stock {
    /// What a freshly placed postbox holds: nothing (`MinimumStockModule.java:51`).
    pub fn new() -> Stock {
        Stock { rows: Vec::new() }
    }

    /// The list as the view sent it.
    pub fn from_rows(rows: &[Row]) -> Stock {
        Stock { rows: rows.to_vec() }
    }

    /// The rows, in list order.
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// How many entries it holds (`MinimumStockModuleWindow.java:131-134`).
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether it is empty — when the XML's `emptytext` shows.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// A removal: the row leaves **here first**, then the message is sent (`:79-83`).
    ///
    /// The row is returned because the message carries it. An index with no row is
    /// [`Refusal::RowNotFound`] where Java throws.
    pub fn remove(&mut self, index: usize) -> Result<Row, Refusal> {
        if index >= self.rows.len() {
            return Err(Refusal::RowNotFound);
        }
        Ok(self.rows.remove(index))
    }

    /// An addition: a **map write**, so an item already listed takes the new quantity instead of
    /// summing with the old one (`MinimumStockModule.java:66`).
    pub fn add(&mut self, row: Row) {
        match self.rows.iter_mut().find(|existing| existing.item == row.item) {
            Some(existing) => existing.quantity = row.quantity,
            None => self.rows.push(row),
        }
    }

    /// Whether the **server** would take this item: already listed, or room for one more
    /// (`MinimumStockModule.java:64-70`).
    pub fn accepts(&self, ceiling: i32, item: &str) -> bool {
        self.rows.iter().any(|row| row.item == item) || (self.rows.len() as i32) < ceiling
    }
}

impl Default for Stock {
    fn default() -> Self {
        Stock::new()
    }
}

/// How many entries a building may hold: `(int)(level * 5 * (1 + research))`
/// (`MinimumStockModule.java:56-60`).
///
/// The cast truncates like Java's, so a level-5 building at +50 % holds 37, not 38. `research` is
/// the colony's `MINIMUM_STOCK` effect strength (`:58`), passed in rather than read.
pub fn ceiling(level: i32, research: f64) -> i32 {
    if level <= 0 {
        return 0;
    }
    (f64::from(level) * f64::from(STOCK_PER_LEVEL) * (1.0 + research)) as i32
}

/// Whether the **window** lets the add button open the picker: one boolean off the wire
/// (`MinimumStockModuleWindow.java:93`, `MinimumStockModuleView.java:66-69`).
pub fn may_add(reached_limit: bool) -> Result<(), Refusal> {
    if reached_limit { Err(Refusal::StockLimitReached) } else { Ok(()) }
}

/// The add button's label in this state (`MinimumStockModuleWindow.java:61-66`).
pub fn add_label(reached_limit: bool) -> &'static str {
    if reached_limit { LIMIT_KEY } else { ADD_KEY }
}

/// The panes: the list and three row panes. **No window-level pane**: the header is a `Text` with
/// no id (`windowpostboxminstock.xml:8`), the module's description painted as a title.
pub const PANES: &[Pane] = &[
    Pane::new(LIST_ID, Value::Row, Source::module("minstock")),
    Pane::in_row(LIST_ID, ROW_ICON_ID, Value::Icon, Source::module("minstock")),
    Pane::in_row(LIST_ID, ROW_NAME_ID, Value::Name, Source::module("minstock")),
    // The quantity is an integer the player set, not a stock on hand: `Value::Count`, because
    // `Value::Stock` in `src/ui` is a build's "available / needed" line.
    Pane::in_row(LIST_ID, ROW_QTY_ID, Value::Count, Source::module("minstock")),
];

/// The two buttons: one for the list, one per row.
pub const ACTIONS: &[Action] = &[
    Action::new(
        ADD_ID,
        ADD_KEY,
        Click::One(Effect::OpensJava(PICKER_WINDOW)),
        &[Refusal::StockLimitReached],
    ),
    // The XML's own label, not a key: `<button label="X" id="removeStock" …/>`
    // (`windowpostboxminstock.xml:14`, `layoutminimumstock.xml:12`).
    Action::new(
        REMOVE_ID,
        "X",
        Click::One(Effect::Sends(REMOVE_MESSAGE)),
        &[Refusal::RowNotFound],
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ceiling_is_five_entries_per_level_plus_research() {
        assert_eq!(STOCK_PER_LEVEL, 5);
        assert_eq!(ceiling(0, 0.0), 0, "an unbuilt hut holds nothing");
        assert_eq!(ceiling(1, 0.0), 5);
        assert_eq!(ceiling(5, 0.0), 25);
        assert_eq!(ceiling(5, 1.0), 50, "a doubled effect doubles the entries");
        assert_eq!(ceiling(5, 0.5), 37, "the cast truncates, as Java's does");
        assert_eq!(ceiling(-1, 4.0), 0);
    }

    #[test]
    fn the_window_refuses_a_full_list_but_the_server_would_still_take_a_repeat() {
        let stock = Stock::from_rows(&[
            Row::new("minecraft:oak_log", 64),
            Row::new("minecraft:stone", 32),
        ]);
        // `level * STOCK_PER_LEVEL * (1 + research)` with STOCK_PER_LEVEL = 5
        // (`MinimumStockModule.java:44`, `:56-60`): a level-1 hut holds **five** entries, not
        // two. The "full list" below is a list of two rows held against a ceiling of two.
        assert_eq!(ceiling(1, 0.0), 5);
        assert_eq!(may_add(true), Err(Refusal::StockLimitReached));
        assert_eq!(add_label(true), LIMIT_KEY);
        assert_eq!(add_label(false), ADD_KEY);
        // The server's rule answers differently on both counts.
        assert!(stock.accepts(2, "minecraft:oak_log"), "already listed: always accepted");
        assert!(!stock.accepts(2, "minecraft:diamond"), "full and new: refused");
        let room = Stock::from_rows(&[Row::new("minecraft:oak_log", 64)]);
        assert!(room.accepts(2, "minecraft:diamond"));
        assert!(!room.accepts(0, "minecraft:diamond"), "a level-0 hut takes nothing");
    }

    #[test]
    fn an_add_replaces_a_quantity_and_never_sums_it() {
        let mut stock = Stock::new();
        assert!(stock.is_empty());
        stock.add(Row::new("minecraft:oak_log", 64));
        stock.add(Row::new("minecraft:stone", 32));
        assert_eq!(stock.len(), 2);
        // The same item again: `put`, not `merge` (MinimumStockModule.java:66).
        stock.add(Row::new("minecraft:oak_log", 8));
        assert_eq!(stock.len(), 2);
        assert_eq!(stock.rows()[0], Row::new("minecraft:oak_log", 8));
        assert_eq!(stock.rows()[1], Row::new("minecraft:stone", 32));
    }

    #[test]
    fn a_removal_takes_the_row_out_here_first_and_the_two_resources_are_one_screen() {
        let mut stock = Stock::from_rows(&[Row::new("a", 1), Row::new("b", 2), Row::new("c", 3)]);
        assert_eq!(stock.remove(1), Ok(Row::new("b", 2)));
        assert_eq!(stock.rows(), &[Row::new("a", 1), Row::new("c", 3)]);
        // An index with no row is a name here and an exception in Java.
        assert_eq!(stock.remove(2), Err(Refusal::RowNotFound));
        assert_eq!(stock.remove(0), Ok(Row::new("a", 1)));
        assert_eq!(stock.remove(0), Ok(Row::new("c", 3)));
        assert!(stock.is_empty());
        // Two resources, four identical row ids: a layout, not a second behaviour.
        let ids: Vec<&str> = PANES.iter().map(|pane| pane.id).collect();
        assert_eq!(ids, [LIST_ID, ROW_ICON_ID, ROW_NAME_ID, ROW_QTY_ID]);
        assert_ne!(MODULE_RESOURCE, POSTBOX_RESOURCE);
        assert!(MODULE_RESOURCE.ends_with("layoutminimumstock.xml"));
        assert!(POSTBOX_RESOURCE.ends_with("windowpostboxminstock.xml"));
        assert_ne!(EMPTY_KEY, STOCK_KEY);
        // One button opens the picker, the other sends; only the first is guarded.
        assert_eq!(ACTIONS.len(), 2);
        assert_eq!(ACTIONS[0].id, ADD_ID);
        assert_eq!(ACTIONS[0].click, Click::One(Effect::OpensJava("WindowSelectRes")));
        assert_eq!((ACTIONS[1].id, ACTIONS[1].label), (REMOVE_ID, "X"));
        assert!(ACTIONS[0].is_guarded() && ACTIONS[1].is_guarded());
        assert_eq!(PANES[3].value, Value::Count);
    }
}
