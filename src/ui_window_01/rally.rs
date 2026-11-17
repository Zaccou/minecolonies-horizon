//! `WindowBannerRallyGuards`: the guard towers a rallying banner holds, and the two buttons.
//!
//! Java reference: `core/client/gui/WindowBannerRallyGuards.java` (169 lines),
//! `gui/windowbannerrallyguards.xml`, and the item that opens it,
//! `core/items/ItemBannerRallyGuards.java`.
//!
//! **The list is not the colony's.** Every row comes from a tag on the **banner item**
//! (`guardtowerpositions`, `NbtTagConstants.java:606`), and each location is then resolved to a
//! building view that may no longer exist: a tower that was moved or destroyed keeps its row and
//! draws a **cookie** instead of a sword (`WindowBannerRallyGuards.java:116-134`). The row therefore
//! has three states, not two — present, missing, and the row that is neither because the index is
//! stale.
//!
//! **An item tag is not a `Source`.** [`crate::ui::field::Source`] has no variant for "the compound of
//! an item the player holds"; [`Source::PlayerInventory`] is the one the enum offers for the client
//! player's own items, and it is borrowed here with this comment rather than a new variant added to
//! another agent's module. The **guard count** is the exception: it is read off the guard tower's
//! building view, so it is [`Source::Building`]. `MANQUANT.md` names the gap.
//!
//! **The count is not a mix.** The line reads `knight|ranger: N` where `N` is
//! `guardTowerView.getGuards().size()` — *every* guard of the tower, knights and rangers alike, printed
//! under both names (`:119-123`, with Java's own `//todo we probably want to display the exact mix.` at
//! `:112`). [`GuardType`] keeps the two keys and the one count so the label cannot pretend otherwise.
//!
//! **The window is only ever opened on a non-empty list.** `handleRightClick` sends a chat line instead
//! when there is nothing to show (`ItemBannerRallyGuards.java:220-226`), so the window's own
//! precondition is a *data* one — and `src/ui`'s `Gate` carries only an advancement, which is why it is
//! [`may_open`] and not a gate.
//!
//! **One red is rgba(255, 0, 0, 1).** The missing-tower line is set to `Color.rgbaToInt(255, 0, 0, 1)`
//! (`:132`) — four components where the last one is an alpha of **1**, not 255, so the port keeps the
//! intent ([`super::Emphasis::Red`]) and quotes Java's literal instead of packing it.

use crate::jobs::Job;
use crate::ui::action::Effect;
use crate::ui::field::{Field, Source, Value};
use crate::ui::screen::Title;

use super::{Afterwards, Button, Panel, Refusal};

/// The tower list (`WindowBannerRallyGuards.java:36`).
pub const LIST_TOWERS: &str = "guardtowers";
/// The item icon of a row (`WindowBannerRallyGuards.java:41`).
pub const ICON_GUARD: &str = "guardicon";
/// The row's remove button (`WindowBannerRallyGuards.java:46`) — the same value as
/// `WindowConstants.BUTTON_REMOVE` (`:584`), declared again rather than imported.
pub const BUTTON_REMOVE: &str = "remove";
/// The window's one window-wide button (`WindowBannerRallyGuards.java:51`).
pub const BUTTON_RALLY: &str = "rally";
/// The guard type line of a row (`WindowBannerRallyGuards.java:56`).
pub const LABEL_GUARD_TYPE: &str = "guardtype";
/// The location line of a row (`WindowBannerRallyGuards.java:61`).
pub const LABEL_POSITION: &str = "position";

/// The window's heading, a static label of the XML (`windowbannerrallyguards.xml:7`).
pub const TITLE_KEY: &str = "item.minecolonies.banner_rally_guards";

/// The `rally` button's label while the banner is **not** active (`:98`,
/// `TranslationConstants.java:119`).
pub const RALLY_KEY: &str = "com.minecolonies.coremod.item.bannerrallyguards.gui.rally";

/// The same button's label while the banner **is** active (`:94`, `TranslationConstants.java:121`) —
/// the button is a toggle, so its label is the state, not the action.
pub const DISMISS_KEY: &str = "com.minecolonies.coremod.item.bannerrallyguards.gui.dismiss";

/// The guard type line of a tower that is gone (`:131`, `TranslationConstants.java:117`).
pub const TOWER_MISSING_KEY: &str = "com.minecolonies.coremod.item.bannerrallyguards.gui.towermissing";

/// The row's remove label, which only the XML sets (`windowbannerrallyguards.xml:17`). Java has a
/// `TranslationConstants` constant for the rally, dismiss and missing-tower keys and **none** for
/// this one.
pub const REMOVE_KEY: &str = "com.minecolonies.coremod.item.bannerrallyguards.gui.remove";

/// Where the item keeps its list of towers (`NbtTagConstants.java:606`).
pub const TAG_RALLIED_TOWERS: &str = "guardtowerpositions";

/// Where the item keeps whether the banner is active (`ItemBannerRallyGuards.java:60`).
pub const TAG_IS_ACTIVE: &str = "isActive";

/// One row of the list: a tower's location, and the tower it resolves to.
///
/// `guards` is `None` when the building view is gone — Java's `Pair<ILocation, View>` with a null
/// second (`ItemBannerRallyGuards.java:416-424`), which is what the cookie branch draws.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tower {
    /// `ILocation.toString()`, the row's `position` text (`:124`, `:133`). Not a translation key: a
    /// location prints its dimension and its block position.
    pub location: String,
    /// `getGuardTowerViews(...).getSecond().getGuards().size()`, `None` for a tower that is gone.
    pub guards: Option<usize>,
}

/// The icon of a row (`:118`, `:128`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: two items Java puts in the slot
pub enum Icon {
    /// A live tower: `Items.IRON_SWORD` (`:118`) — the guard's weapon, not a guard.
    IronSword,
    /// A tower that is gone: `Items.COOKIE` (`:128`), Java's own joke.
    Cookie,
}

/// The `guardtype` line of a live row (`WindowBannerRallyGuards.java:119-123`).
///
/// Two job translation keys and one count, in that order, joined by `"|"` and `": "`. The keys are
/// [`Job::translation_key`]s — `com.minecolonies.job.knight` and `…ranger` — which is the same string
/// Java's `ModGuardTypes.knight.get().getJobTranslationKey()` answers
/// (`GuardType.java:102-107`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuardType {
    /// The knight's job key.
    pub knight: String,
    /// The ranger's job key.
    pub ranger: String,
    /// How many guards the tower has **in total**, knights and rangers together.
    pub count: usize,
}

impl GuardType {
    /// The line as Java concatenates it: `knight "|" ranger ": " count`.
    ///
    /// The two job keys are translatables with no argument, so concatenating them here loses nothing;
    /// a caller that renders will translate each key and keep the separators.
    pub fn literal(&self) -> String {
        format!("{}|{}: {}", self.knight, self.ranger, self.count)
    }
}

/// The `guardtype` line, from the count of a tower's guards (Java's `:119-123`).
pub fn guard_type(count: usize) -> GuardType {
    GuardType {
        knight: Job::Knight.translation_key(),
        ranger: Job::Ranger.translation_key(),
        count,
    }
}

/// What one row renders, for the three panes Java writes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowView {
    /// The item in `guardicon`.
    pub icon: Icon,
    /// `guardtype`: a count, or the missing-tower key.
    pub kind: RowKind,
    /// `position`: the location's own text.
    pub position: String,
}

/// What the `guardtype` pane of a row reads.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the two branches of `if (guardTowerView != null)`
pub enum RowKind {
    /// A live tower: two job keys and a count (`:119-123`).
    GuardType(GuardType),
    /// A tower that is gone: [`TOWER_MISSING_KEY`], drawn red (`:131-132`).
    MissingTower,
}

/// One row as the window writes it (`WindowBannerRallyGuards.java:110-134`).
pub fn row(tower: &Tower) -> RowView {
    match tower.guards {
        Some(count) => RowView {
            icon: Icon::IronSword,
            kind: RowKind::GuardType(guard_type(count)),
            position: tower.location.clone(),
        },
        None => RowView {
            icon: Icon::Cookie,
            kind: RowKind::MissingTower,
            position: tower.location.clone(),
        },
    }
}

/// The `rally` button's label, from whether the banner is active
/// (`WindowBannerRallyGuards.java:92-99`, `ItemBannerRallyGuards.java:458-462`).
pub fn rally_label(active: bool) -> &'static str {
    if active {
        DISMISS_KEY
    } else {
        RALLY_KEY
    }
}

/// Why the window is not opened at all: the banner rallies no tower
/// (`ItemBannerRallyGuards.java:220-226`).
pub fn may_open(towers: &[Tower]) -> Result<(), Refusal> {
    if towers.is_empty() {
        return Err(Refusal::EmptyRallyList);
    }
    Ok(())
}

/// The `remove` click: which row it drops (`WindowBannerRallyGuards.java:143-157`).
///
/// Java guards the index twice — `guardTowers.size() > row && row >= 0` (`:148`) — because the row
/// index comes from the list's own pane lookup. The item then removes the location from the banner's
/// tag on both sides (`:152`, `:155`), which is why the caller gets the index and not a copy of the
/// list.
pub fn remove_row(towers: &[Tower], row: usize) -> Result<usize, Refusal> {
    if row >= towers.len() {
        return Err(Refusal::NoSuchTower);
    }
    Ok(row)
}

/// The rally window: one list of three panes and two buttons.
pub static PANEL: Panel = Panel {
    id: "rally.towers",
    window: "WindowBannerRallyGuards",
    resource: "minecolonies:gui/windowbannerrallyguards.xml",
    title: Title::Fixed(TITLE_KEY),
    fields: &[
        // The list itself, read from the banner's `guardtowerpositions` tag.
        Field::new(LIST_TOWERS, Value::Row, Source::PlayerInventory),
        // The row's icon: a sword or a cookie, chosen by whether the tower still exists.
        Field::in_row(LIST_TOWERS, ICON_GUARD, Value::Icon, Source::PlayerInventory),
        // The guard type line: the tower's guard count, off its building view.
        Field::in_row(LIST_TOWERS, LABEL_GUARD_TYPE, Value::Text, Source::Building),
        // The location line: the ILocation's own text, off the banner's tag.
        Field::in_row(LIST_TOWERS, LABEL_POSITION, Value::Text, Source::PlayerInventory),
    ],
    buttons: &[
        Button::guarded(
            BUTTON_REMOVE,
            REMOVE_KEY,
            Effect::Sends("RemoveFromRallyingListMessage"),
            &[Refusal::NoSuchTower],
        ),
        // The label is written by `onOpened` from the banner's state (:94-99), so it starts empty.
        Button::plain(BUTTON_RALLY, "", Effect::Sends("ToggleBannerRallyGuardsMessage"))
            .then(Afterwards::Closes),
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    fn tower(location: &str, guards: Option<usize>) -> Tower {
        Tower { location: location.to_owned(), guards }
    }

    #[test]
    fn a_live_tower_reads_a_sword_two_job_keys_and_one_count() {
        let view = row(&tower("minecraft:overworld 100,70,-40", Some(5)));
        assert_eq!(view.icon, Icon::IronSword);
        assert_eq!(view.position, "minecraft:overworld 100,70,-40");
        match &view.kind {
            RowKind::GuardType(kind) => {
                assert_eq!(kind.count, 5);
                assert_eq!(kind.knight, "com.minecolonies.job.knight");
                assert_eq!(kind.ranger, "com.minecolonies.job.ranger");
                // Java's own concatenation, including the two separators.
                assert_eq!(
                    kind.literal(),
                    "com.minecolonies.job.knight|com.minecolonies.job.ranger: 5"
                );
            }
            other => panic!("unexpected kind {other:?}"),
        }
        // The count is every guard of the tower, under both names: WindowBannerRallyGuards.java:112.
        let zero = row(&tower("here", Some(0)));
        match zero.kind {
            RowKind::GuardType(kind) => assert_eq!(kind.count, 0),
            other => panic!("unexpected kind {other:?}"),
        }
    }

    #[test]
    fn a_tower_that_is_gone_reads_a_cookie_and_the_missing_key() {
        let view = row(&tower("minecraft:overworld 0,64,0", None));
        assert_eq!(view.icon, Icon::Cookie);
        assert_eq!(view.kind, RowKind::MissingTower);
        // The location is still printed: Java writes it in both branches (:124, :133).
        assert_eq!(view.position, "minecraft:overworld 0,64,0");
        assert_eq!(TOWER_MISSING_KEY, "com.minecolonies.coremod.item.bannerrallyguards.gui.towermissing");
        // The two icons are two different items, so a row can never look live while reading missing.
        assert_ne!(Icon::IronSword, Icon::Cookie);
    }

    #[test]
    fn the_rally_button_label_is_the_state_not_the_action() {
        assert_eq!(rally_label(false), RALLY_KEY);
        assert_eq!(rally_label(true), DISMISS_KEY);
        assert_ne!(rally_label(true), rally_label(false));
        assert_eq!(RALLY_KEY, "com.minecolonies.coremod.item.bannerrallyguards.gui.rally");
        assert_eq!(DISMISS_KEY, "com.minecolonies.coremod.item.bannerrallyguards.gui.dismiss");
    }

    #[test]
    fn a_row_out_of_range_is_refused_by_name_and_the_last_row_is_not() {
        let towers = [tower("a", Some(1)), tower("b", None)];
        assert_eq!(remove_row(&towers, 0), Ok(0));
        assert_eq!(remove_row(&towers, 1), Ok(1));
        assert_eq!(remove_row(&towers, 2), Err(Refusal::NoSuchTower));
        assert_eq!(Refusal::NoSuchTower.key(), None);
    }

    #[test]
    fn the_window_is_only_opened_on_a_list_that_holds_a_tower() {
        let towers = [tower("a", Some(1))];
        assert_eq!(may_open(&towers), Ok(()));
        assert_eq!(may_open(&[]), Err(Refusal::EmptyRallyList));
        // The empty case is handled by the item, and it tells the player in chat instead
        // (ItemBannerRallyGuards.java:222) — so this refusal has no key of its own.
        assert_eq!(Refusal::EmptyRallyList.key(), None);
    }

    #[test]
    fn the_panel_names_its_xml_its_six_private_ids_and_its_two_buttons() {
        use crate::ui_window_01::constants;
        assert_eq!(PANEL.window, "WindowBannerRallyGuards");
        assert_eq!(PANEL.resource, "minecolonies:gui/windowbannerrallyguards.xml");
        assert!(PANEL.resource.starts_with("minecolonies:gui/"));
        // Every id of this panel is one of the window's own six, plus the two buttons of the two.
        for field in PANEL.fields {
            let own = constants::RALLY_OWN_IDS.iter().any(|constant| constant.value == field.id);
            assert!(own, "{} is not one of the window's private ids", field.id);
        }
        // The one value the two Java files share, and the one button whose label is the state.
        assert_eq!(constants::value_of("BUTTON_REMOVE"), Some(BUTTON_REMOVE));
        assert_eq!(PANEL.button(BUTTON_RALLY).map(|button| button.label), Some(""));
        assert_eq!(
            PANEL.button(BUTTON_RALLY).map(|button| button.afterwards),
            Some(Afterwards::Closes)
        );
        // `src/ui` does not hold this window, and must not: it is not a hut screen.
        assert!(crate::ui::screen_by_id(PANEL.id).is_err());
        assert_eq!(PANEL.title, Title::Fixed(TITLE_KEY));
    }
}
