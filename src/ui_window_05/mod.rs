//! Four windows `src/ui` **names and does not describe**: what each asks for, what each button
//! sends, and what state a click moves.
//!
//! Java reference: `core/client/gui/WindowInteraction.java` (191 lines),
//! `WindowPlantationField.java` (174), `WindowPostBoxMain.java` (306), `WindowPostBoxMinStock.java`
//! (29) — plus the two classes they lean on for their body, `MinimumStockModuleWindow.java` (149)
//! and `RequestTreeWindowModule.java` (386).
//!
//! # These four are the other side of `src/ui`'s own refusal
//!
//! `src/ui` describes thirteen hut screens and hands the rest back by name:
//! `Refusal::NotAHutScreen` carries the town hall's `WindowMainPage` and **the postbox's
//! `WindowPostBoxMain`** (`src/ui/table.rs:62-67`, citing `PostBox.java:125`). This module is that
//! window, plus three `src/ui` had no row for. It plugs in rather than standing beside:
//!
//! - **[`Pane`] is `src/ui`'s field**, `(id, label, value, scope)` with
//!   `crate::ui::field::{Value, Scope}` reused unchanged and `Pane::from_ui` / `Pane::to_ui`
//!   converting between the shapes;
//! - **[`Effect`] is `src/ui`'s effect**, carried by nine of the eleven buttons, so
//!   `Effect::Container`, `Effect::Toggles` and `Effect::OpensJava` mean here what they mean there;
//! - **[`Refusal::Ui`] forwards** any refusal `src/ui` already names, the way
//!   `ui::action::Refusal::HutRefuses` forwards the jobs table's;
//! - **[`Title`] keeps `src/ui`'s `Fixed`** and adds the one shape no hut window has: a speaker.
//!
//! Not duplicated: no second `Value`, no second `Scope`, no second hut-kind table.
//!
//! # The four windows
//!
//! | id | class | resource | title |
//! | --- | --- | --- | --- |
//! | `interaction` | `WindowInteraction` | `gui/citizen/windowinteraction.xml` | the speaker |
//! | `plantation_field` | `WindowPlantationField` | `gui/windowplantationfield.xml` | a fixed key |
//! | `postbox.request` | `WindowPostBoxMain` | `gui/windowpostboxrequest.xml` **+** `gui/layouthuts/layoutrequeststree.xml` | a fixed key |
//! | `postbox.minstock` | `WindowPostBoxMinStock` | `gui/windowpostboxminstock.xml` | a fixed key |
//!
//! # The counts, measured in the source
//!
//! | What | How many | Where |
//! | --- | --- | --- |
//! | windows | **4** | [`WINDOWS`] |
//! | of them a `WindowHut*` file | **0** | none of the nine `WindowHut*.java` is here |
//! | windows mounting **two** XMLs | 1 | `WindowPostBoxMain.java:104` |
//! | panes | 23 | 4 + 4 + 11 + 4 |
//! | buttons | 11 | 2 + 1 + 6 + 2 |
//! | button families whose id carries an index | **1**, unbounded | [`interaction::response_id`] |
//! | buttons doing two things in order | 2 | `ClosesAndSends`, `ClosesThenOpens` |
//! | buttons registered and never implemented | **1** | [`postbox::TREE_FULFILL_ID`] |
//! | refusal names | 10 — 9 new, 1 forwarded | [`refusal::Refusal`] |
//! | `Value` kinds these windows use | **6** of 12 | the third test |
//! | `Value::{Level, Progress, Step, Stock, Priority, Occupancy}` used | **0** | same test |
//!
//! # Four premises the source contradicts
//!
//! 1. **"Ces écrans montrent nom, niveau, progression, stock."** Two of the four words exist
//!    **nowhere**: no pane holds a building level and none holds a percentage, so `Value::Level`,
//!    `Value::Progress`, `Value::Step`, `Value::Priority` and `Value::Occupancy` are all unused.
//!    "Stock" exists only as a **minimum** the player sets (`resourceQty`), a floor rather than a
//!    quantity on hand — `Value::Stock`, `src/ui`'s "available / needed" line, is unused too. This
//!    is the finding `src/ui` reached from the other side: the only progress bar in the mod is the
//!    builder's reserves tab.
//! 2. **"`WindowPostBoxMinStock` est un second écran."** It is a **29-line** subclass that passes
//!    another XML and re-registers the same tabs; the body, both buttons and both refusals are
//!    `MinimumStockModuleWindow`'s. One behaviour, two layouts — and its
//!    `shouldRenderDefaultSidebar() = false` does **not** remove the tabs, it replaces the default
//!    strip with the postbox's own copy of it (`WindowPostBoxMain.java:207-237`).
//! 3. **"Un clic envoie un message."** Nine of eleven do; one **closes and sends**
//!    (`WindowInteraction.java:149-158`), one **closes and opens another window**
//!    (`WindowPlantationField.java:87-103`), and one is registered, shown and does **nothing**
//!    (`RequestTreeWindowModule.java:288-292`). [`Click`] exists because `Effect` cannot say that.
//! 4. **"Chaque bouton a un id constant."** True for ten of eleven; the interaction window builds
//!    `response_1` … `response_N` at runtime (`:128`), so its id is a **prefix** and the arithmetic
//!    is [`interaction::response_id`] / [`interaction::response_index`].
//!
//! # The state, because a screen is also a transition
//!
//! Each window carries what its clicks move, and none of it is drawing: [`interaction::State`] is
//! the dialogue cursor with Java's small-talk skip, [`plantation::State`] the one colony condition
//! that flips four panes every tick, [`postbox::State`] the filter, its countdown and the delivery
//! toggle whose label and flag can disagree, [`minstock::Stock`] the list an add overwrites and a
//! remove empties before the message leaves. `src/ui` carries no state at all — this module does,
//! because a click here is not always a message.
//!
//! # What is not here
//!
//! The **drawing** (positions, sizes, sprites, and the response buttons' wrapping at
//! `WindowInteraction.java:118-142`) and the **network**: eight message classes are named and none
//! is sent — `InteractionClose`, `InteractionResponse`, `PostBoxRequestMessage`,
//! `UpdateRequestStateMessage`, `PlantationFieldBuildRequestMessage`,
//! `AddMinimumStockToBuildingModuleMessage`, `RemoveMinimumStockFromBuildingModuleMessage`,
//! `OpenInventoryMessage`. `MANQUANT.md` says what a client slice would need.

pub mod field;
pub mod interaction;
pub mod minstock;
pub mod plantation;
pub mod postbox;
pub mod refusal;

use crate::ui::action::Effect;
use crate::ui::screen::Title as UiTitle;

pub use field::{Pane, Source};
pub use refusal::Refusal;

/// What a screen's header reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two shapes these four windows need
pub enum Title {
    /// A fixed key the XML writes into its own `text` pane
    /// (`windowplantationfield.xml:11`, `windowpostboxrequest.xml:6`, and the module
    /// description `AbstractModuleWindow.java:57-66` paints).
    Fixed(&'static str),
    /// The name of the citizen talking, which the window writes **in front of** the inquiry into
    /// the same pane (`WindowInteraction.java:111`). [`crate::ui::screen::Title::BuildingName`]
    /// cannot say it: no building view is involved and the name is half of a line.
    Speaker,
}

impl Title {
    /// The same title, read off a hut screen, when `src/ui` can carry it — `None` for
    /// [`Title::Speaker`], which a hut screen has no equivalent of.
    pub fn from_ui(title: UiTitle) -> Option<Title> {
        match title {
            UiTitle::Fixed(key) => Some(Title::Fixed(key)),
            UiTitle::BuildingName => None,
        }
    }

    /// The key the title reads, when it is a fixed one.
    pub fn key(self) -> Option<&'static str> {
        match self {
            Title::Fixed(key) => Some(key),
            Title::Speaker => None,
        }
    }
}

/// What one click does once nothing refused it.
///
/// [`Effect`] carries nine of the eleven buttons. It cannot carry three real cases, and pretending
/// otherwise would be a lie in a data table: a click that does **two** things in order, and a
/// button whose body Java never wrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: what the eleven buttons of these four windows do
pub enum Click {
    /// The one effect `src/ui` already names.
    One(Effect),
    /// Registered, wired, shown — and its handler does nothing:
    /// `RequestTreeWindowModule.java:288-292`.
    Nothing,
    /// Closes this window, and the handler sends `message` on the way out
    /// (`WindowInteraction.java:149-158`).
    ClosesAndSends(&'static str),
    /// Closes this window, then opens the Java window named with the data in hand
    /// (`WindowPlantationField.java:87-103`).
    ClosesThenOpens(&'static str),
}

impl Click {
    /// The message this click sends, when it sends exactly one and names it.
    pub fn message(self) -> Option<&'static str> {
        match self {
            Click::One(Effect::Sends(name)) | Click::ClosesAndSends(name) => Some(name),
            Click::One(_) | Click::Nothing | Click::ClosesThenOpens(_) => None,
        }
    }

    /// Whether this click changes the screen the player looks at.
    pub fn leaves_the_window(self) -> bool {
        match self {
            Click::ClosesAndSends(_) | Click::ClosesThenOpens(_) => true,
            Click::One(Effect::OpensJava(_)) => true,
            Click::One(_) | Click::Nothing => false,
        }
    }
}

/// One button of one of these windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Action {
    /// The XML button id `registerButton` binds (`AbstractWindowSkeleton.java:130`) — or, for the
    /// one family of generated buttons, [`interaction::RESPONSE_ID_PREFIX`].
    pub id: &'static str,
    /// The label's key; `""` for a button whose label the class writes later or never.
    pub label: &'static str,
    /// What the click does.
    pub click: Click,
    /// Every way the click is refused, in the order Java hides or guards it.
    pub refusals: &'static [Refusal],
}

impl Action {
    /// A button with its click and its refusals.
    pub const fn new(
        id: &'static str,
        label: &'static str,
        click: Click,
        refusals: &'static [Refusal],
    ) -> Action {
        Action { id, label, click, refusals }
    }

    /// Whether Java can refuse this click.
    pub fn is_guarded(&self) -> bool {
        !self.refusals.is_empty()
    }
}

/// One window: the class, the resource, the panes and the buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    /// This table's name for the window — **not** Java's, which opens a window from a block, a
    /// module view or a citizen.
    pub id: &'static str,
    /// The Java class that builds it.
    pub class: &'static str,
    /// The BlockUI window, under `assets/minecolonies/`.
    pub resource: &'static str,
    /// A second resource mounted **into** the same window, when there is one
    /// (`registerLayoutModule`, `WindowPostBoxMain.java:104`).
    pub layout: Option<&'static str>,
    /// What the header reads.
    pub title: Title,
    /// The panes the class fills.
    pub panes: &'static [Pane],
    /// The buttons the class registers.
    pub actions: &'static [Action],
}

impl Window {
    /// The pane with this id, copied out.
    pub fn pane(&self, id: &str) -> Option<Pane> {
        self.panes.iter().find(|pane| pane.id == id).copied()
    }

    /// The action with this button id, copied out.
    pub fn action(&self, id: &str) -> Option<Action> {
        self.actions.iter().find(|action| action.id == id).copied()
    }
}

/// The window ids, as constants so every row and every test agrees.
pub const INTERACTION_ID: &str = "interaction";
pub const PLANTATION_ID: &str = "plantation_field";
pub const POSTBOX_ID: &str = "postbox.request";
pub const MINSTOCK_ID: &str = "postbox.minstock";

/// The dialogue a citizen opens (`WindowInteraction.java:58-64`).
pub static INTERACTION: Window = Window {
    id: INTERACTION_ID,
    class: "WindowInteraction",
    resource: "minecolonies:gui/citizen/windowinteraction.xml",
    layout: None,
    title: Title::Speaker,
    panes: interaction::PANES,
    actions: interaction::ACTIONS,
};

/// The plantation field's window — a block, not a building (`WindowPlantationField.java:73-85`).
pub static PLANTATION: Window = Window {
    id: PLANTATION_ID,
    class: "WindowPlantationField",
    resource: "minecolonies:gui/windowplantationfield.xml",
    layout: None,
    title: Title::Fixed(plantation::TITLE_KEY),
    panes: plantation::PANES,
    actions: plantation::ACTIONS,
};

/// The postbox's request window, with the request tree mounted in it (`WindowPostBoxMain.java:100-104`).
pub static POSTBOX: Window = Window {
    id: POSTBOX_ID,
    class: "WindowPostBoxMain",
    resource: "minecolonies:gui/windowpostboxrequest.xml",
    layout: Some("minecolonies:gui/layouthuts/layoutrequeststree.xml"),
    title: Title::Fixed("com.minecolonies.coremod.gui.citizen.stacks"),
    panes: postbox::PANES,
    actions: postbox::ACTIONS,
};

/// The postbox's minimum stock: the module window with the postbox's layout
/// (`WindowPostBoxMinStock.java:18-22`).
pub static MINSTOCK: Window = Window {
    id: MINSTOCK_ID,
    class: "WindowPostBoxMinStock",
    resource: minstock::POSTBOX_RESOURCE,
    layout: None,
    title: Title::Fixed(minstock::STOCK_KEY),
    panes: minstock::PANES,
    actions: minstock::ACTIONS,
};

/// The four windows, in the order this module declares them.
pub static WINDOWS: &[&Window] = &[&INTERACTION, &PLANTATION, &POSTBOX, &MINSTOCK];

/// The window with this id, or [`Refusal::UnknownWindow`].
pub fn window_by_id(id: &str) -> Result<&'static Window, Refusal> {
    WINDOWS.iter().copied().find(|window| window.id == id).ok_or(Refusal::UnknownWindow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::field::Value;

    #[test]
    fn four_windows_and_every_row_names_a_class_a_resource_and_a_closable_list() {
        assert_eq!(WINDOWS.len(), 4);
        for (index, window) in WINDOWS.iter().enumerate() {
            assert!(window.resource.starts_with("minecolonies:gui/"), "{}", window.id);
            assert!(window.resource.ends_with(".xml"));
            assert!(!window.panes.is_empty() && !window.actions.is_empty(), "{}", window.id);
            for other in &WINDOWS[index + 1..] {
                assert_ne!(window.id, other.id, "two windows share an id");
                assert_ne!(window.class, other.class, "two rows name one class");
                assert_ne!(window.resource, other.resource, "two rows read one XML");
            }
            // A row's panes belong to a list of the same window, and the list is a row.
            for pane in window.panes {
                if let crate::ui::field::Scope::Row(list) = pane.scope {
                    assert!(
                        window
                            .panes
                            .iter()
                            .any(|other| other.id == list && other.value == Value::Row),
                        "{}: {} is a row of {list}",
                        window.id,
                        pane.id
                    );
                }
            }
        }
        // One window mounts a second resource; the other three are one page.
        let with_layout: Vec<&str> =
            WINDOWS.iter().filter(|w| w.layout.is_some()).map(|w| w.id).collect();
        assert_eq!(with_layout, [POSTBOX_ID]);
    }

    #[test]
    fn a_window_id_resolves_or_is_refused_by_name_and_none_is_a_hut_screen() {
        for (id, class) in [
            (INTERACTION_ID, "WindowInteraction"),
            (PLANTATION_ID, "WindowPlantationField"),
            (POSTBOX_ID, "WindowPostBoxMain"),
            (MINSTOCK_ID, "WindowPostBoxMinStock"),
        ] {
            assert_eq!(window_by_id(id).map(|window| window.class), Ok(class));
        }
        assert_eq!(window_by_id("hir"), Err(Refusal::UnknownWindow));
        assert_eq!(Refusal::UnknownWindow.key(), None);
        // Not one is a hut screen: `src/ui` names the postbox and refuses it.
        for window in WINDOWS {
            assert!(!window.class.starts_with("WindowHut"));
            assert!(crate::ui::screen_by_id(window.id).is_err());
        }
        // The accessors answer the first pane and the only button of a given id.
        let field_window = window_by_id(PLANTATION_ID).unwrap_or(&PLANTATION);
        assert_eq!(field_window.pane(plantation::PLANTS_LIST_ID).map(|p| p.value), Some(Value::Row));
        assert_eq!(field_window.pane("nope"), None);
        assert_eq!(
            field_window.action(plantation::REPAIR_ID).map(|a| a.click),
            Some(Click::ClosesThenOpens(plantation::BUILD_WINDOW))
        );
        assert!(field_window.action("nope").is_none());
    }

    #[test]
    fn the_totals_are_what_the_module_doc_says_and_six_value_kinds_of_twelve_are_used() {
        let panes: usize = WINDOWS.iter().map(|w| w.panes.len()).sum();
        let actions: usize = WINDOWS.iter().map(|w| w.actions.len()).sum();
        assert_eq!((panes, actions), (23, 11));
        assert_eq!(
            (interaction::PANES.len(), plantation::PANES.len()),
            (4, 4)
        );
        assert_eq!((postbox::PANES.len(), minstock::PANES.len()), (11, 4));
        assert_eq!((interaction::ACTIONS.len(), plantation::ACTIONS.len()), (2, 1));
        assert_eq!((postbox::ACTIONS.len(), minstock::ACTIONS.len()), (6, 2));
        // One button does nothing; one closes and sends.
        let mut nothing = 0;
        let mut closes_and_sends = 0;
        for window in WINDOWS {
            for action in window.actions {
                if action.click == Click::Nothing {
                    nothing += 1;
                }
                if action.click.leaves_the_window() && action.click.message().is_some() {
                    closes_and_sends += 1;
                }
            }
        }
        assert_eq!((nothing, closes_and_sends), (1, 1));
        // The one label that is an **XML literal** and not a key: `removeStock` is
        // `<button label="X" id="removeStock" …/>` — `windowpostboxminstock.xml:14`,
        // `layoutminimumstock.xml:12`. Java never wraps it in `Component.translatable`, so
        // there is no key to look for; anything else must be a key.
        const XML_LITERALS: [&str; 1] = ["X"];
        for window in WINDOWS {
            for action in window.actions {
                assert_eq!(action.is_guarded(), !action.refusals.is_empty());
                if !action.label.is_empty() {
                    assert!(
                        action.label.starts_with("com.minecolonies.")
                            || action.label.starts_with("container.")
                            || XML_LITERALS.contains(&action.label),
                        "{}.{} labels itself {:?}",
                        window.id,
                        action.id,
                        action.label
                    );
                }
            }
        }
        // Six of the twelve value kinds, and the four words of the brief held against them.
        let mut used: Vec<Value> = Vec::new();
        for pane in WINDOWS.iter().flat_map(|window| window.panes.iter()) {
            if !used.contains(&pane.value) {
                used.push(pane.value);
            }
        }
        assert_eq!(used.len(), 6, "six kinds of the twelve, no more");
        for kind in [Value::Name, Value::Text, Value::Row, Value::Input, Value::Icon, Value::Count] {
            assert!(used.contains(&kind), "{}", kind.name());
        }
        for unused in [
            Value::Level,
            Value::Progress,
            Value::Step,
            Value::Stock,
            Value::Priority,
            Value::Occupancy,
        ] {
            assert!(!used.contains(&unused), "{} is on none of these four", unused.name());
        }
    }

    #[test]
    fn a_hut_screens_effect_and_title_are_used_as_they_are() {
        // `src/ui`'s effect, carried unchanged: no translation, no wrapper.
        assert_eq!(Click::One(Effect::Container), Click::One(Effect::Container));
        assert_ne!(Click::One(Effect::Toggles), Click::Nothing);
        assert_eq!(Click::One(Effect::Sends("HireFireMessage")).message(), Some("HireFireMessage"));
        assert_eq!(Click::One(Effect::OpensJava("WindowInfo")).message(), None);
        assert!(Click::One(Effect::OpensJava("WindowInfo")).leaves_the_window());
        assert!(!Click::One(Effect::Toggles).leaves_the_window());
        // A hut screen's fixed title travels; its building name does not.
        assert_eq!(Title::from_ui(UiTitle::Fixed("a.key")), Some(Title::Fixed("a.key")));
        assert_eq!(Title::from_ui(UiTitle::BuildingName), None);
        assert_eq!((Title::Speaker.key(), Title::Fixed("a.key").key()), (None, Some("a.key")));
    }
}
