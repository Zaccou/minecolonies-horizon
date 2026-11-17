//! Three windows of `core/client/gui/`: what each one asks for, and what each one renders.
//!
//! Java reference: `api/util/constant/WindowConstants.java`,
//! `core/client/gui/WindowAssignCitizen.java` (401 lines, BlockUI + XML),
//! `core/client/gui/WindowBannerPicker.java` (573 lines, a **vanilla `Screen`**, no XML at all)
//! and `core/client/gui/WindowBannerRallyGuards.java` (169 lines, BlockUI + XML).
//!
//! **`src/ui` already describes one of the three, and this module does not describe it twice.**
//! [`crate::ui::screen::SCREENS`] holds the assign window under `assign`
//! ([`crate::ui::screen::ASSIGN_ID`], class `WindowAssignCitizen`), with its two lists, its two
//! row panes and its four buttons. [`assign`] here adds what that table cannot hold — the text a
//! row's `job` pane is composed from, the filter and the sort of the left-hand list, and the two
//! clicks — and [`assign::screen`] hands the `src/ui` record back so a caller has one entry point.
//! The two other windows are in **no** table: [`banner`] and [`rally`] build their own [`Panel`].
//!
//! # What these three screens show, and what the brief expects that they do not
//!
//! | Brief | Reality | Where |
//! | --- | --- | --- |
//! | a name | one name, the citizen's, per row | `WindowAssignCitizen.java:254`, `:350` |
//! | a level | **none on any of the three** — the level is read once, to filter the pattern list | `WindowBannerPicker.java:144-150` |
//! | a progression | **none**; the assign window shows two *distances* per row | `:260-272` |
//! | a stock | **none**; the rally window shows a guard *count* | `WindowBannerRallyGuards.java:119-123` |
//!
//! The only number that looks like a progression is the distance, and it is not one: it is
//! `BlockPosUtil.getDistance` (`BlockPosUtil.java:453-462`), compared against a **300-block**
//! threshold (`WindowAssignCitizen.java:39`), and measured on **two different pairs** depending on
//! which list the row is in ([`assign::job_line`] and [`assign::assigned_job_line`]).
//!
//! # Three windows, three ways of naming a pane
//!
//! 1. `WindowAssignCitizen` imports `WindowConstants.*` and reads **six** of its constants
//!    ([`constants`]) plus the two list ids of its own XML.
//! 2. `WindowBannerRallyGuards` imports **no** `WindowConstants` at all: its five ids are private
//!    `static final` fields of the class (`:36-61`), and `remove` collides in value with
//!    `WindowConstants.BUTTON_REMOVE` (`:584`) without ever naming it.
//! 3. `WindowBannerPicker` has **no pane id whatsoever**: it extends
//!    `net.minecraft.client.gui.screens.Screen` (`:49`), draws everything itself and builds its
//!    buttons as anonymous objects in `createLayerButtons`/`createPatternButtons`
//!    (`:179-222`). `Field`, `Screen` and the XML attributes therefore do not apply to it.
//!
//! # Why there is a second `Refusal` here, and why it does not overlap
//!
//! [`crate::ui::action::Refusal`] is a closed set of the **hut windows'** refusals. Two of these
//! three windows are not hut windows, and their refusals are not in that set: a picker cannot
//! remove its base layer, a rally row can point at a tower that no longer exists, an assign window
//! whose colony has no town hall cannot go back. Adding them to `src/ui` would be an edit to a
//! module another agent owns, so [`Refusal`] is declared here — and **every variant of it is new**:
//! the three refusals the assign window *shares* with the hut screens (`NoRoom`, `Travelling`,
//! `HousingHandledElsewhere`) are imported from `src/ui`, never restated, and
//! [`assign`] returns `Result<_, crate::ui::action::Refusal>` for them.
//!
//! A fourth, smaller premise: **`Gate` cannot gate the rally window.** `Screen.gate` carries one
//! variant, an advancement (`crate::ui::screen::Gate`); the rally window's condition is *data* —
//! it is only ever opened on a non-empty tower list (`ItemBannerRallyGuards.java:220-226`) — so it
//! is [`rally::may_open`] and not a gate.
//!
//! # What is not here
//!
//! - **the drawing.** `WindowBannerPicker.center` (`:262-265`) is grid arithmetic, the picker's
//!   `PatternButton.render` is a colour fill (`:530-571`), and the two BlockUI windows' `pos`/`size`
//!   are the XML's. None of it is carried, per `TRANCHE.md` §3 M2.
//! - **the network.** Every click that sends a message carries its **class name** only, exactly as
//!   [`crate::ui::action::Effect::Sends`] already says.
//! - **the item, the flag and the registry.** The rally window reads an `ItemStack`'s
//!   `guardtowerpositions` tag (`NbtTagConstants.java:606`), the picker reads the colony's
//!   `Patterns` tag (`:473`) and both read a client building view. `src/ui::field::Source` has no
//!   variant for an item tag or for a registry: `src/ui_window_01/MANQUANT.md` names the gap.

pub mod assign;
pub mod banner;
pub mod constants;
pub mod rally;

use crate::ui::action::{Action, Effect};
use crate::ui::field::{Field, Value};
use crate::ui::screen::Title;

/// How a line is drawn, when Java changes a component's style.
///
/// Java's `ChatFormatting`, three of the nineteen values, quoted where they are used:
/// `DARK_GREEN` for a home that is closer than the current one
/// (`WindowAssignCitizen.java:284-287`), `RED` for a distance over the threshold
/// (`:273-276`, `:366-370`) and the default style everywhere else.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: the three styles these three windows ask for
pub enum Emphasis {
    /// No `withStyle` at all — the player's default colour.
    Plain,
    /// A gain: the new distance beats the old one.
    DarkGreen,
    /// Too far, or a tower that is gone.
    Red,
}

/// What Java does with the window itself once a click's effect has happened.
///
/// [`crate::ui::action::Effect`] carries **one** effect per button, and four handlers here do two:
/// the picker writes the flag *and* reopens the town hall (`WindowBannerPicker.java:234-241`), the
/// rally button sends its message *and* closes itself (`WindowBannerRallyGuards.java:164-168`), and
/// the assign window's mode button sets the mode *and* has the view send it
/// (`LivingBuildingView.java:87-91`). The second thing is not a second `Effect` — it is not an
/// action on the model — so it is a flag on the [`Button`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: reopen the opener, close oneself, or nothing
pub enum Afterwards {
    /// The window stays as it is (`remove` in the rally window, `mode` everywhere).
    Nothing,
    /// The window that opened this one is opened again: `window.open()`
    /// (`WindowBannerPicker.java:240`, `:248`) and `building.openGui(false)`
    /// (`WindowAssignCitizen.java:174-178`).
    Reopens,
    /// The window closes itself: `this.close()` (`WindowBannerRallyGuards.java:167`).
    Closes,
}

/// One button of one of these windows.
///
/// **Not [`Action`], and the difference is two fields.** `effect` is the same
/// [`Effect`] `src/ui` uses; `afterwards` is the second thing Java's handler does, which `Action`
/// cannot carry; and `refusals` is this module's [`Refusal`], because `Action.refusals` is a slice
/// of `src/ui`'s closed set and the rally window's `NoSuchTower` is not a member of it.
/// [`Button::action`] hands back the `src/ui` shape **when the button has no refusal of its own**,
/// so the two descriptions agree wherever they can.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Button {
    /// The XML button id where there is one — `registerButton` binds exactly this string
    /// (`AbstractWindowSkeleton.java:129-131`). Where Java builds the button as an anonymous object
    /// with no id (`WindowBannerPicker.java:185-249`), this is **this module's name** for it, the
    /// same convention [`crate::ui::screen::Screen::id`] already uses.
    pub id: &'static str,
    /// The label's translation key; `""` for a button whose label the window writes later or that
    /// carries a literal (`WindowBannerPicker.java:191` writes a red "X").
    pub label: &'static str,
    /// What the click does to the model.
    pub effect: Effect,
    /// What the click does to the window afterwards.
    pub afterwards: Afterwards,
    /// Every way the click is refused, in the order Java checks them.
    pub refusals: &'static [Refusal],
}

impl Button {
    /// A button Java never refuses at click time.
    pub const fn plain(id: &'static str, label: &'static str, effect: Effect) -> Button {
        Button { id, label, effect, afterwards: Afterwards::Nothing, refusals: &[] }
    }

    /// A button Java refuses, by name.
    pub const fn guarded(
        id: &'static str,
        label: &'static str,
        effect: Effect,
        refusals: &'static [Refusal],
    ) -> Button {
        Button { id, label, effect, afterwards: Afterwards::Nothing, refusals }
    }

    /// The same button, with the second thing its handler does.
    pub const fn then(self, afterwards: Afterwards) -> Button {
        Button {
            id: self.id,
            label: self.label,
            effect: self.effect,
            afterwards,
            refusals: self.refusals,
        }
    }

    /// Whether Java can refuse this click.
    pub fn is_guarded(&self) -> bool {
        !self.refusals.is_empty()
    }

    /// This button in `src/ui`'s shape, or `None` when it carries a refusal `src/ui` cannot name.
    ///
    /// `None` is not a failure: it is "this refusal is not one of the hut windows'". A caller that
    /// only iterates `src/ui`'s tables gets `None` and must ask [`Button::refusals`] itself.
    pub fn action(&self) -> Option<Action> {
        if self.is_guarded() {
            return None;
        }
        Some(Action {
            id: self.id,
            label: self.label,
            effect: self.effect,
            refusals: &[],
        })
    }
}

/// One of the two windows `src/ui` does not hold: its panes, its buttons and its XML.
///
/// The same six fields as [`crate::ui::screen::Screen`] minus `gate`, plus [`Button`] instead of
/// [`Action`] — the two differences are the ones [`Button`] documents. `WindowBannerPicker` has no
/// XML and no panes: it is described by [`banner::Picker`], not by a `Panel`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Panel {
    /// This module's name for the window. **Not** Java's, and not a `src/ui` screen id either:
    /// [`crate::ui::screen_by_id`] answers `UnknownScreen` for it, which is the honest answer,
    /// because these are not hut screens.
    pub id: &'static str,
    /// The Java class that builds it.
    pub window: &'static str,
    /// The BlockUI window, under `assets/minecolonies/`.
    pub resource: &'static str,
    /// What the window's heading reads.
    pub title: Title,
    /// The panes the class fills, by id.
    pub fields: &'static [Field],
    /// The buttons the class registers.
    pub buttons: &'static [Button],
}

impl Panel {
    /// The field with this pane id.
    pub fn field(&self, id: &str) -> Option<Field> {
        self.fields.iter().find(|field| field.id == id).copied()
    }

    /// The button with this id.
    pub fn button(&self, id: &str) -> Option<Button> {
        self.buttons.iter().find(|button| button.id == id).copied()
    }

    /// Whether the window writes a pane of this kind.
    pub fn shows(&self, value: Value) -> bool {
        self.fields.iter().any(|field| field.value == value)
    }
}

/// Why one of these three clicks did nothing, in the cases `src/ui` does not name.
///
/// Every variant is **silent** in Java: a disabled button, a `return` at the top of a handler, or a
/// window that is never opened. [`Refusal::key`] therefore answers `None` for all of them, and a
/// caller must not print [`Refusal::name`] where the player expects a sentence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own closed set, disjoint from `src/ui`'s
pub enum Refusal {
    /// The assign window's `cancel` does nothing when the colony view has no town hall:
    /// `if (button.getID().equals(BUTTON_CANCEL) && …getTownHall() != null)`
    /// (`WindowAssignCitizen.java:174`). The XML button is there and enabled; the handler
    /// silently declines.
    NoTownHall,
    /// The picker's red X is disabled on the base layer, which cannot be removed
    /// (`WindowBannerPicker.java:197`: `activeLayer < layers.size() && activeLayer != 0`).
    BaseLayerNotRemovable,
    /// A layer index past the end of the list. Java reaches this only through a broken list and
    /// throws (`layers.get(activeLayer)` at `:279`, `layers.set` at `:285`); the port refuses.
    LayerOutOfRange,
    /// The rally window's `remove` was clicked for a row that the list no longer has — Java's
    /// guard is `guardTowers.size() > row && row >= 0` (`WindowBannerRallyGuards.java:148`).
    NoSuchTower,
    /// The rally window is only opened when the banner rallies at least one tower
    /// (`ItemBannerRallyGuards.java:220-226`); opened on an empty list it would show no row.
    EmptyRallyList,
    /// A building level outside the six rows of the picker's exclusion table
    /// (`WindowBannerPicker.java:77-95`, indexed by `:146`). Java throws an
    /// `ArrayIndexOutOfBoundsException`; the port names it.
    LevelOutOfRange,
}

impl Refusal {
    /// Every refusal of this module, in declaration order.
    pub const ALL: [Refusal; 6] = [
        Refusal::NoTownHall,
        Refusal::BaseLayerNotRemovable,
        Refusal::LayerOutOfRange,
        Refusal::NoSuchTower,
        Refusal::EmptyRallyList,
        Refusal::LevelOutOfRange,
    ];

    /// A stable name, namespaced so it can never be mistaken for a sentence.
    pub fn name(self) -> &'static str {
        match self {
            Refusal::NoTownHall => "ui.refusal.no_town_hall",
            Refusal::BaseLayerNotRemovable => "ui.refusal.base_layer",
            Refusal::LayerOutOfRange => "ui.refusal.layer_range",
            Refusal::NoSuchTower => "ui.refusal.no_such_tower",
            Refusal::EmptyRallyList => "ui.refusal.empty_rally",
            Refusal::LevelOutOfRange => "ui.refusal.level_range",
        }
    }

    /// Java's translation key for this refusal, when Java tells the player anything.
    ///
    /// `None` for all six: Java disables a button, returns early, or never opens the window.
    pub fn key(self) -> Option<&'static str> {
        None
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

/// The three windows this slice describes, by Java class.
///
/// One of them is already a `src/ui` screen ([`crate::ui::screen::ASSIGN_ID`]); the two banner
/// windows are in no table, and [`crate::ui::screen_by_id`] refuses them by name
/// ([`crate::ui::action::Refusal::UnknownScreen`]) rather than by inventing a row.
pub const WINDOWS: &[&str] = &["WindowAssignCitizen", "WindowBannerPicker", "WindowBannerRallyGuards"];

/// Whether this module describes this Java window class.
pub fn is_described(window: &str) -> bool {
    WINDOWS.contains(&window)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::action::Effect as UiEffect;

    #[test]
    fn the_three_windows_are_the_three_of_this_slice_and_only_one_is_a_hut_screen() {
        assert_eq!(WINDOWS.len(), 3);
        for window in WINDOWS {
            assert!(is_described(window), "{window} is not described");
            // Exactly one of them is a `src/ui` screen, and it is the assign window.
            let in_src_ui = crate::ui::screen::SCREENS
                .iter()
                .any(|screen| screen.window == *window);
            assert_eq!(in_src_ui, *window == "WindowAssignCitizen", "{window}");
        }
        assert!(!is_described("WindowHutWorkerModulePlaceholder"));
        // The two banner windows are in neither table: not a screen, not a `WindowHut*` file.
        for window in ["WindowBannerPicker", "WindowBannerRallyGuards"] {
            assert!(!crate::ui::screen::SCREENS.iter().any(|s| s.window == window));
            assert!(!crate::ui::screen::HUT_WINDOWS.iter().any(|(w, _)| *w == window));
            assert!(!window.starts_with("WindowHut"));
        }
    }

    #[test]
    fn a_button_answers_src_uis_action_exactly_when_it_has_no_refusal_of_its_own() {
        let plain = Button::plain("done", "gui.done", UiEffect::Sends("ColonyFlagChangeMessage"))
            .then(Afterwards::Reopens);
        assert!(!plain.is_guarded());
        assert_eq!(plain.afterwards, Afterwards::Reopens);
        let action = plain.action();
        assert!(action.is_some());
        if let Some(action) = action {
            assert_eq!(action.id, "done");
            assert_eq!(action.effect, UiEffect::Sends("ColonyFlagChangeMessage"));
        }
        // A guarded button carries a refusal `src/ui`'s `Action` cannot name, so it has no
        // `Action` form rather than a lying one with an empty slice.
        let guarded =
            Button::guarded("remove", "", UiEffect::Sends("RemoveFromRallyingListMessage"), &[
                Refusal::NoSuchTower,
            ]);
        assert!(guarded.is_guarded());
        assert_eq!(guarded.action(), None);
        assert_eq!(guarded.refusals.len(), 1);
        assert_eq!(Button::plain("x", "", UiEffect::Reopens).afterwards, Afterwards::Nothing);
    }

    #[test]
    fn the_six_refusals_are_six_names_and_none_of_them_is_a_sentence() {
        for (index, refusal) in Refusal::ALL.iter().enumerate() {
            assert!(refusal.name().starts_with("ui.refusal."), "{refusal}");
            assert_eq!(refusal.key(), None, "{refusal} is silent in Java");
            for other in &Refusal::ALL[index + 1..] {
                assert_ne!(refusal.name(), other.name(), "two refusals share a name");
            }
        }
        assert_eq!(Refusal::ALL.len(), 6);
    }
}
