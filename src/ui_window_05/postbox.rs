//! `WindowPostBoxMain`: the postbox's request window — filter, list, quantity, request tree.
//!
//! Java reference: `core/client/gui/WindowPostBoxMain.java` (306 lines, BlockUI + **two** XMLs:
//! `gui/windowpostboxrequest.xml` and, mounted as a layout module at 261,44,
//! `gui/layouthuts/layoutrequeststree.xml` — `:104`). Opened by `PostBox.View.getWindow()`
//! (`PostBox.java:124-127`).
//!
//! # A filter over the player's own items
//!
//! `updateResources` reads `ItemStackUtils.allItemsPlusInventory(player)` — the **client**
//! player's inventory plus every item the game knows (`:252`) — keeps what matches and sorts it.
//! Three ways to match (`:254-260`): the item's description id contains the filter, or its
//! display name does, or — for an enchanted book — its **first** enchantment id does; both sides
//! are lowercased. The sort is `getLevenshteinDistance(name, filter)` ascending, which has a
//! consequence worth stating because it contradicts the obvious guess: **with an empty filter
//! every distance is the name's own length**, so an empty filter sorts by name length, shortest
//! first, not in registry order ([`sorted`]).
//!
//! # Two countdowns for one keystroke, and only one is debounced
//!
//! Typing in the filter pane does two things (`:111-131`): it **immediately** clears the
//! quantity field of every rendered row, and it arms a **ten-tick** countdown that refreshes the
//! list (`:268-276`). Re-typing the same text does neither — the block is under
//! `if (!newFilter.equals(filter))`. [`State::typed`] answers whether the filter moved, because
//! a caller that ignores it clears the player's quantities on every keystroke that changed
//! nothing. The clearing only touches **rendered** rows (Java walks the live children, `:117-129`),
//! which is a rendering fact this port cannot hold and names instead.
//!
//! # The quantity field disagrees with itself, and that is why it works
//!
//! The `qty` pane takes 25 characters (`windowpostboxrequest.xml:13`) and accepts digits only;
//! its filter parses the text and **erases** it unless the number is strictly positive, clamping
//! anything above 999 (`InputFilters.java:22-52`). Reading it back, Java falls back to the stack's
//! maximum on a parse failure (`:171`, `:176-183`) — which is what an empty field is. A quantity
//! of **zero** is thus unreachable through the UI and perfectly reachable through the number,
//! which is why [`qty_or_default`] and [`filter_qty`] are tested against each other.
//!
//! # The tabs
//!
//! `registerPostboxTabs` (`:207-237`) is the **same loop** as `AbstractBuildingWindow`'s default
//! sidebar (`:57-85`), written again so a window that is not a building window can show one. The
//! `Random(id.hashCode())` argument seeds only the **tab icons** (`TabsWindowModule.java:109`,
//! `:172`), never the order.

use super::field::{Pane, Source};
use super::refusal::Refusal;
use super::{Action, Click};
use crate::ui::action::Effect;
use crate::ui::field::Value;

/// The filter pane (`WindowConstants.java:272`; `windowpostboxrequest.xml:7`).
pub const FILTER_ID: &str = "name";
/// The item list (`WindowConstants.java:499`).
pub const LIST_ID: &str = "resources";
/// An item's icon inside a row (`WindowConstants.java:508`).
pub const ROW_ICON_ID: &str = "resourceIcon";
/// An item's display name inside a row (`WindowConstants.java:502`).
pub const ROW_NAME_ID: &str = "resourceName";
/// The quantity field inside a row (`WindowConstants.java:928`).
pub const ROW_QTY_ID: &str = "qty";
/// The per-row request button (`WindowConstants.java:929`).
pub const REQUEST_ID: &str = "request";
/// The container button (`WindowConstants.java:557`).
pub const INVENTORY_ID: &str = "inventory";
/// The request button's label (`windowpostboxrequest.xml:14`).
pub const REQUEST_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.request";
/// The container button's label (`windowpostboxrequest.xml:24`).
pub const INVENTORY_KEY: &str = "container.inventory";

/// The "deliver what is available" toggle (`WindowPostBoxMain.java:48`).
pub const DELIVER_ID: &str = "deliverAvailable";
/// Its unticked label, a red underlined X (`:53`).
pub const RED_X: &str = "§n§4X";
/// Its ticked label (`:58`).
pub const APPROVE: &str = "✓";
/// How many ticks the list waits after the filter changes (`:116`, `:272`).
pub const REFRESH_TICKS: i32 = 10;
/// The largest quantity the field accepts (`InputFilters.java:32-35`).
pub const QTY_MAX: i32 = 999;
/// The filter field's XML limit (`windowpostboxrequest.xml:7`).
pub const FILTER_FIELD_MAX_LENGTH: usize = 25;
/// The quantity field's XML limit — three digits can survive the filter
/// (`windowpostboxrequest.xml:13`).
pub const QTY_FIELD_MAX_LENGTH: usize = 25;
/// Where the shared tab strip starts (`:43`).
pub const TABS_X_OFFSET: i32 = 34;
/// The first tab's label (`GuiTranslationConstants.java:11`, rendered at `:217`).
pub const MAIN_TAB_KEY: &str = "com.minecolonies.coremod.gui.maintab";

/// The request tree's list (`layoutrequeststree.xml:1`).
pub const TREE_LIST_ID: &str = "requests";
/// The tree's detail button — inside a row, registered on the **window** (`:106`).
pub const TREE_DETAIL_ID: &str = "detail";
/// The tree's cancel button (`:107`).
pub const TREE_CANCEL_ID: &str = "cancel";
/// The tree's fulfil button (`:108`).
pub const TREE_FULFILL_ID: &str = "fulfill";
/// The detail button's label (`layoutrequeststree.xml:4`).
pub const TREE_DETAIL_KEY: &str = "com.minecolonies.coremod.gui.citizen.requests";
/// The fulfil button's label (`layoutrequeststree.xml:12`).
pub const TREE_FULFILL_KEY: &str = "com.minecolonies.coremod.gui.citizen.requestsfulfill";
/// The cancel button's label (`layoutrequeststree.xml:17`).
pub const TREE_CANCEL_KEY: &str = "com.minecolonies.coremod.gui.citizen.requestscancel";
/// The window the detail button opens (`RequestTreeWindowModule.java:381-384`).
pub const DETAIL_WINDOW: &str = "WindowRequestDetail";
/// The message the per-row button sends (`:186`).
pub const REQUEST_MESSAGE: &str = "PostBoxRequestMessage";
/// The message a cancel sends (`RequestTreeWindowModule.java:328`).
pub const CANCEL_MESSAGE: &str = "UpdateRequestStateMessage";

/// One item as the filter and the sort see it: the three strings Java reads off an `ItemStack`
/// (`WindowPostBoxMain.java:254-260`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item {
    /// `getDescriptionId()`, e.g. `item.minecraft.iron_ingot`.
    pub description_id: &'static str,
    /// `getHoverName().getString()`.
    pub name: &'static str,
    /// The first enchantment's id, for an enchanted book only.
    pub enchantment: Option<&'static str>,
    /// `getMaxStackSize()`, the fallback quantity (`:171`).
    pub max_stack: u32,
}

impl Item {
    /// An item with no enchantment and a stack of 64.
    pub const fn new(description_id: &'static str, name: &'static str) -> Item {
        Item { description_id, name, enchantment: None, max_stack: 64 }
    }

    /// The same item with a stack size of its own.
    pub const fn stack(self, max_stack: u32) -> Item {
        Item {
            description_id: self.description_id,
            name: self.name,
            enchantment: self.enchantment,
            max_stack,
        }
    }

    /// The same item carrying the id of its first enchantment.
    pub const fn enchanted(self, enchantment: &'static str) -> Item {
        Item {
            description_id: self.description_id,
            name: self.name,
            enchantment: Some(enchantment),
            max_stack: self.max_stack,
        }
    }
}

/// Whether one item survives the filter (`WindowPostBoxMain.java:253-260`).
pub fn matches(item: &Item, filter: &str) -> bool {
    if filter.is_empty() {
        return true;
    }
    let needle = filter.to_lowercase();
    item.description_id.to_lowercase().contains(&needle)
        || item.name.to_lowercase().contains(&needle)
        || item.enchantment.map(|id| id.to_lowercase().contains(&needle)).unwrap_or(false)
}

/// `StringUtils.getLevenshteinDistance`, the distance the sort compares against the filter
/// (`:261`).
///
/// Java walks UTF-16 units and this walks `char`s, so an astral character costs two there and one
/// here: that changes a distance, never a match, and only reorders names already equally close.
pub fn levenshtein(left: &str, right: &str) -> usize {
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    if left.is_empty() || right.is_empty() {
        return left.len().max(right.len());
    }
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0usize; right.len() + 1];
    for (row, left_char) in left.iter().enumerate() {
        current[0] = row + 1;
        for (column, right_char) in right.iter().enumerate() {
            let cost = usize::from(left_char != right_char);
            current[column + 1] = (previous[column] + cost)
                .min(current[column] + 1)
                .min(previous[column + 1] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}

/// The items that pass the filter, in Java's order: ascending edit distance between the display
/// name and the filter, ties keeping the order they were read in (`:252-262`).
///
/// **An empty filter does not preserve the input order**: every distance is the name's length, so
/// the list comes out shortest name first.
pub fn sorted(items: &[Item], filter: &str) -> Vec<Item> {
    let mut kept: Vec<Item> = items.iter().copied().filter(|item| matches(item, filter)).collect();
    // `sort_by_key` is stable, like `Stream.sorted` on an ordered stream.
    kept.sort_by_key(|item| levenshtein(item.name, filter));
    kept
}

/// What the `qty` field holds after the player types `text` (`InputFilters.java:22-44`).
///
/// A parse failure or a non-positive number answers the **empty string**, anything above
/// [`QTY_MAX`] answers `"999"`. The parse is Java's `Integer.parseInt`, so a number too large for
/// an `int` is **erased**, not clamped: `"9999999999"` answers `""`, `"1000"` answers `"999"`.
pub fn filter_qty(text: &str) -> String {
    let Ok(mut value) = text.parse::<i32>() else {
        return String::new();
    };
    if value <= 0 {
        return String::new();
    }
    if value > QTY_MAX {
        value = QTY_MAX;
    }
    value.to_string()
}

/// The quantity a click sends: the parsed field, or the stack's maximum when it cannot be read
/// (`:171`, `:176-183`). [`filter_qty`] is what makes `"0"` unreachable; this turns the empty
/// result back into a full stack.
pub fn qty_or_default(text: &str, max_stack: u32) -> u32 {
    text.parse::<u32>().unwrap_or(max_stack)
}

/// The window's own state: the filter, the delivery flag, the countdown — three fields of the
/// class (`:83`, `:88`, `:93`) and nothing the server owns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    filter: String,
    deliver_available: bool,
    tick: i32,
}

impl State {
    /// A freshly constructed window: empty filter, delivery off, no countdown.
    pub fn new() -> State {
        State { filter: String::new(), deliver_available: false, tick: 0 }
    }

    /// The filter, as typed.
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Whether "deliver what is available" is ticked.
    pub fn deliver_available(&self) -> bool {
        self.deliver_available
    }

    /// The ticks left before the list refreshes; `0` when nothing is armed.
    pub fn tick(&self) -> i32 {
        self.tick
    }

    /// The player typed `text` into the filter pane (`:111-131`).
    ///
    /// `true` when the filter actually moved — and then, and only then, the countdown is armed
    /// **and** the caller must clear every rendered row's quantity field. That second half cannot
    /// live here: the fields belong to the rows.
    pub fn typed(&mut self, text: &str) -> bool {
        if text == self.filter {
            return false;
        }
        self.filter = text.to_owned();
        self.tick = REFRESH_TICKS;
        true
    }

    /// One tick of `onUpdate` (`:268-276`); `true` on the tick the list must refresh.
    pub fn update(&mut self) -> bool {
        if self.tick <= 0 {
            return false;
        }
        self.tick -= 1;
        self.tick == 0
    }

    /// The toggle's click (`:190-202`): flag and label flip together, which is why Java decides by
    /// **reading the button's own text**.
    pub fn toggle_delivery(&mut self) -> &'static str {
        self.deliver_available = !self.deliver_available;
        self.deliver_label()
    }

    /// The label the toggle shows now.
    pub fn deliver_label(&self) -> &'static str {
        if self.deliver_available { APPROVE } else { RED_X }
    }

    /// The state `onOpened` leaves: the label is rewritten to the red X (`:243`) while the flag
    /// keeps its value, so a window reopened after a ticked delivery shows the unticked label and
    /// still sends `true`.
    pub fn reopened(&self) -> State {
        State { filter: self.filter.clone(), deliver_available: self.deliver_available, tick: 0 }
    }
}

impl Default for State {
    fn default() -> Self {
        State::new()
    }
}

/// How many tabs the postbox shows: the main one plus one per **visible** module view
/// (`:214-236`).
pub fn tabs(visible_modules: usize) -> usize {
    1 + visible_modules
}

/// Whether a request row's cancel button is shown: only a top-level request may be cancelled from
/// here (`RequestTreeWindowModule.java:258-268`, `:168-171`).
pub fn may_cancel(open_requests: usize, depth: usize) -> Result<(), Refusal> {
    if open_requests == 0 {
        return Err(Refusal::NoOpenRequest);
    }
    if depth > 0 {
        return Err(Refusal::NotTopLevelRequest);
    }
    Ok(())
}

/// Whether a request row's fulfil button is shown: the postbox tree inherits the base answer,
/// which is always `false` (`:274-277`), and the body it would call does nothing (`:288-292`).
pub const fn may_fulfill() -> Result<(), Refusal> {
    Err(Refusal::NotFulfillable)
}

/// The panes of the window's own XML, then the tree's rows.
pub const PANES: &[Pane] = &[
    // The filter; the XML's `desc` text beside it is a label the class never writes.
    Pane::new(FILTER_ID, Value::Input, Source::Window),
    Pane::new(LIST_ID, Value::Row, Source::PLAYER_INVENTORY),
    Pane::in_row(LIST_ID, ROW_ICON_ID, Value::Icon, Source::PLAYER_INVENTORY),
    Pane::in_row(LIST_ID, ROW_NAME_ID, Value::Name, Source::PLAYER_INVENTORY),
    Pane::in_row(LIST_ID, ROW_QTY_ID, Value::Input, Source::Window),
    // The request tree, a second XML mounted into this window (`:104`).
    Pane::new(TREE_LIST_ID, Value::Row, Source::REQUEST),
    Pane::in_row(TREE_LIST_ID, "requestStack", Value::Icon, Source::REQUEST),
    Pane::in_row(TREE_LIST_ID, "deliveryImage", Value::Icon, Source::REQUEST),
    Pane::in_row(TREE_LIST_ID, "shortDetail", Value::Text, Source::REQUEST),
    Pane::in_row(TREE_LIST_ID, "detailIcon", Value::Icon, Source::REQUEST),
    Pane::in_row(TREE_LIST_ID, "requester", Value::Name, Source::REQUEST),
];

/// The six buttons: three of its own and the tree's three.
pub const ACTIONS: &[Action] = &[
    // `OpenInventoryMessage(postBoxView)` (`:157-160`), which `src/ui`'s `Effect::Container`
    // already describes.
    Action::new(INVENTORY_ID, INVENTORY_KEY, Click::One(Effect::Container), &[]),
    // A pure local boolean: no window, no message (`:190-202`).
    Action::new(DELIVER_ID, "", Click::One(Effect::Toggles), &[]),
    // One message per row, carrying the stack, the quantity and the flag (`:186`).
    Action::new(
        REQUEST_ID,
        REQUEST_KEY,
        Click::One(Effect::Sends(REQUEST_MESSAGE)),
        &[Refusal::RowNotFound],
    ),
    // The tree's three, registered on the window although they live inside a row (`:106-108`).
    Action::new(
        TREE_DETAIL_ID,
        TREE_DETAIL_KEY,
        Click::One(Effect::OpensJava(DETAIL_WINDOW)),
        &[Refusal::RowNotFound, Refusal::NoOpenRequest],
    ),
    Action::new(
        TREE_CANCEL_ID,
        TREE_CANCEL_KEY,
        Click::One(Effect::Sends(CANCEL_MESSAGE)),
        &[Refusal::RowNotFound, Refusal::NoOpenRequest, Refusal::NotTopLevelRequest],
    ),
    // Registered and shown only when `isFulfillable` says so — and here it never does.
    Action::new(TREE_FULFILL_ID, TREE_FULFILL_KEY, Click::Nothing, &[Refusal::NotFulfillable]),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> [Item; 4] {
        [
            Item::new("item.minecraft.iron_ingot", "Iron Ingot"),
            Item::new("item.minecraft.iron_block", "Block of Iron"),
            Item::new("item.minecraft.diamond", "Diamond"),
            Item::new("item.minecraft.enchanted_book", "Enchanted Book")
                .enchanted("minecraft:mending"),
        ]
    }

    #[test]
    fn the_filter_matches_a_description_id_a_name_or_an_enchantment() {
        let items = sample();
        assert!(items.iter().all(|item| matches(item, "")), "an empty filter keeps all");
        assert!(matches(&items[0], "IRON_INGOT"), "by description id, case-insensitively");
        assert!(matches(&items[1], "of iron"), "by display name");
        assert!(matches(&items[3], "mending"), "by the first enchantment, book only");
        assert!(!matches(&items[2], "mending"));
        assert!(!matches(&items[2], "iron"));
        // A substring test, not a template.
        assert!(!matches(&items[0], "iron ingot "));
    }

    #[test]
    fn an_empty_filter_sorts_by_name_length_not_by_input_order() {
        let items = sample();
        let order: Vec<&str> = sorted(&items, "").iter().map(|item| item.name).collect();
        assert_eq!(order, ["Diamond", "Iron Ingot", "Block of Iron", "Enchanted Book"]);
        // A filter sorts by distance to it, so the closest name comes first.
        let iron: Vec<&str> = sorted(&items, "iron").iter().map(|item| item.name).collect();
        assert_eq!(iron, ["Iron Ingot", "Block of Iron"]);
        assert_eq!(sorted(&items, "nothing").len(), 0);
        assert_eq!(levenshtein("", ""), 0);
        assert_eq!((levenshtein("iron", ""), levenshtein("", "iron")), (4, 4));
        assert_eq!(levenshtein("iron", "irons"), 1);
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("Iron Ingot", "iron"), 7);
        // Symmetric, which the DP has to be to compare names against a filter.
        assert_eq!(levenshtein("abc", "abd"), levenshtein("abd", "abc"));
        // The empty-filter distance is the length, which is why the sort is by length.
        assert_eq!(levenshtein("Diamond", ""), "Diamond".chars().count());
    }

    #[test]
    fn the_quantity_field_erases_zero_and_clamps_above_999() {
        assert_eq!(filter_qty("12"), "12");
        assert_eq!(filter_qty("1000"), "999");
        assert_eq!(filter_qty("9999999999"), "", "Integer.parseInt throws past i32::MAX");
        assert_eq!(filter_qty("0"), "", "a quantity of zero is erased, not kept");
        assert_eq!(filter_qty("-5"), "");
        assert_eq!(filter_qty(""), "");
        assert_eq!(filter_qty("abc"), "");
        // The field takes 25 characters and three can survive.
        assert!(QTY_FIELD_MAX_LENGTH > 3);
        assert_eq!(QTY_FIELD_MAX_LENGTH, FILTER_FIELD_MAX_LENGTH);
        // Reading it back falls to the stack size, which is what an empty field is.
        assert_eq!(qty_or_default("", 64), 64);
        assert_eq!(qty_or_default("12", 64), 12);
        // The pair is the rule: what the filter lets through, the reader accepts.
        assert_eq!(qty_or_default(&filter_qty("1000"), 64), 999);
        assert_eq!(qty_or_default(&filter_qty("0"), 64), 64);
        let pearl = Item::new("item.minecraft.ender_pearl", "Ender Pearl").stack(16);
        assert_eq!(qty_or_default(&filter_qty(""), pearl.max_stack), 16);
    }

    #[test]
    fn typing_arms_ten_ticks_once_and_the_delivery_toggle_moves_its_label() {
        let mut state = State::new();
        assert!(state.typed("iro"));
        assert_eq!(state.tick(), REFRESH_TICKS);
        // The same text changes nothing and does not restart the countdown.
        assert!(!state.typed("iro"));
        assert_eq!(state.tick(), REFRESH_TICKS);
        for _ in 0..3 {
            assert!(!state.update());
        }
        assert_eq!(state.tick(), 7);
        assert!(state.typed("iron"));
        assert_eq!(state.tick(), 10);
        let refreshed = (0..10).filter(|_| state.update()).count();
        assert_eq!(refreshed, 1, "exactly one refresh");
        assert!(!state.update(), "no countdown, no refresh");
        assert_eq!(state.filter(), "iron");
        // The toggle moves flag and label together, and `onOpened` rewrites only the label.
        assert_eq!(state.deliver_label(), RED_X);
        assert_eq!(state.toggle_delivery(), APPROVE);
        let reopened = state.reopened();
        assert!(reopened.deliver_available());
        assert_eq!(reopened.tick(), 0);
    }

    #[test]
    fn only_a_top_level_request_is_cancelled_and_the_window_names_its_parts() {
        assert_eq!(may_cancel(0, 0), Err(Refusal::NoOpenRequest));
        assert_eq!(may_cancel(3, 0), Ok(()));
        assert_eq!(may_cancel(3, 1), Err(Refusal::NotTopLevelRequest));
        assert_eq!(may_fulfill(), Err(Refusal::NotFulfillable));
        // The fulfil action is the one button whose body does nothing at all.
        let fulfill =
            ACTIONS.iter().find(|action| action.id == TREE_FULFILL_ID).copied();
        assert_eq!(fulfill.map(|action| action.click), Some(Click::Nothing));
        // Panes: five of its own and six of the tree.
        assert_eq!(PANES.len(), 11);
        assert_eq!((PANES[0].id, PANES[0].value), (FILTER_ID, Value::Input));
        assert_eq!(PANES[1].value, Value::Row);
        assert!(PANES[2].is_row_pane());
        let ids: Vec<&str> = ACTIONS.iter().map(|action| action.id).collect();
        assert_eq!(
            ids,
            [INVENTORY_ID, DELIVER_ID, REQUEST_ID, TREE_DETAIL_ID, TREE_CANCEL_ID, TREE_FULFILL_ID]
        );
        // One tab for the main page, then one per visible module view.
        assert_eq!((tabs(0), tabs(3)), (1, 4));
        assert_eq!(ACTIONS[1].label, "", "the toggle's text is Java's, at runtime");
    }
}
