//! `WindowBannerPicker`: the colony flag, its layers, and the pattern grid.
//!
//! Java reference: `core/client/gui/WindowBannerPicker.java` (573 lines). **This one is not a
//! BlockUI window**: it extends the game's own `Screen` (`:49`), has no XML, no pane id and no
//! `registerButton` — it draws a banner with the block-entity renderer and builds its buttons as
//! anonymous objects (`:179-222`). [`crate::ui::field::Field`] and [`crate::ui::screen::Screen`] do
//! not apply to it, so this module describes it as a [`Picker`]: the state a click moves, plus the
//! rules that decide what is shown, enabled and selectable.
//!
//! **The screen's data is four things, and one of them is not a value.**
//!
//! | What | Java | Here |
//! | --- | --- | --- |
//! | the colony's saved flag, `Patterns` (`NbtTagConstants.java:473`) | `colony.getColonyFlag()`, `:157` | [`initial_layers`] |
//! | the town hall's level, read **once** to drop patterns | `buildingView.getBuildingLevel()`, `:144` | [`excluded`] |
//! | the pattern registry | `BuiltInRegistries.BANNER_PATTERN.holders()`, `:152` | not carried: it is registry data, see `MANQUANT.md` |
//! | the layer the player is editing, the scroll row, the selected dye | `activeLayer`, `scrollRow`, `colors`, `:121-127` | [`Picker`] |
//!
//! **Two numbers look like a level and a progression and are neither.** The level is not shown; it
//! filters the grid, and the filter is a **range** — `for (int i = level; i <= maxLevel; i++)` over a
//! six-row table (`:144-150`), so a level-1 town hall drops every row from index 1 up and *keeps* the
//! two level-1 patterns, which is the opposite of what the table's own comments suggest at a glance.
//!
//! **The pattern count is not a constant.** It is "whatever the registry holds, minus the excluded
//! rows", so the grid, the scroll bound and the number of pattern buttons are all derived at open
//! time. That is why there is no static list of pattern buttons here: [`scroll_bound`],
//! [`visible`] and [`blocked`] are the rules a generated button follows.
//!
//! **A dead branch, named rather than dropped.** [`blocked`] is Java's patreon lock: a pattern in the
//! `minecolonies` namespace is blacked out while the feature is locked (`:216-220`, `:348-355`).
//! **No banner pattern is registered in that namespace** in this checkout — the registry is vanilla's
//! only, there is no `data/minecolonies/banner_pattern/` and no Java `BannerPattern` under the mod id
//! — so the branch cannot fire today. It is carried because it is a rule, and it is empty because
//! today's registry says so.
//!
//! **Not here: the drawing.** `center` (`:262-265`) is grid arithmetic, `SIDE`/`GUI_Y`/`PATTERN_HEIGHT`
//! (`:51-64`) are pixel sizes, `drawFlag`/`drawBannerPattern`/`renderBanner` (`:321-389`) drive the game
//! renderer, and `PatternButton.renderWidget` (`:553-571`) is four colour fills. `PATTERN_COLUMNS` and
//! `PATTERN_ROWS` are kept because they are **not** pixels: they are the size of the visible window,
//! which is a rule about what the player can reach.

use crate::ui::action::Effect;

use super::{Afterwards, Button, Refusal};

/// The number of columns the pattern grid is arranged in (`WindowBannerPicker.java:67`), and the
/// number the scroll arithmetic divides by (`:296`, `:398`, `:532`).
pub const PATTERN_COLUMNS: usize = 8;

/// The number of rows of the pattern grid that are visible at once
/// (`WindowBannerPicker.java:70`) — the size of the window [`visible`] answers for.
pub const PATTERN_ROWS: usize = 4;

/// The number of layer buttons: `for (int layer = 0; layer <= 6; layer++)`
/// (`WindowBannerPicker.java:181`), so **seven** — the base and six over it.
pub const LAYER_BUTTONS: usize = 7;

/// The base layer's button label (`WindowBannerPicker.java:459`), and the one layer whose label is a
/// key: the six others are literal digits (`Component.literal(String.valueOf(layer))`, `:460`).
pub const BASE_LAYER_KEY: &str = "com.minecolonies.coremod.gui.flag.base_layer";

/// The instruction drawn at the top of the picker (`WindowBannerPicker.java:311`).
pub const CHOOSE_KEY: &str = "com.minecolonies.coremod.gui.flag.choose";

/// The tooltip of a locked pattern (`WindowBannerPicker.java:218`).
pub const PATREON_KEY: &str = "com.minecolonies.core.gui.banner.patreon";

/// The picker's title, and the only one in this slice that is a **literal**, not a translation key:
/// `super(Component.literal("Flag"))` (`WindowBannerPicker.java:136`).
pub const TITLE: &str = "Flag";

/// The mod id Java compares a pattern's namespace against
/// (`Constants.MOD_ID`, used at `WindowBannerPicker.java:216`, `:348`). **Not**
/// [`crate::MOD_ID`]: that is the id of this port's package.
pub const JAVA_MOD_ID: &str = "minecolonies";

/// Vanilla's base banner pattern, `BannerPatterns.BASE`, which the picker writes when a colour click
/// lands on layer zero (`:278`) and which `createPatterns` always prepends (`:157`).
pub const BASE_PATTERN: &str = "minecraft:base";

/// The dye ids, in vanilla's declaration order — the order `DyeColor.getId()` indexes and the order
/// `ColorPalette` lays out (`ColorPalette.java:23`, `:40-41`). Sixteen colours, which is the whole
/// palette: [`Picker::colour`] is one of these ids.
pub const DYES: [&str; 16] = [
    "white",
    "orange",
    "magenta",
    "light_blue",
    "yellow",
    "lime",
    "pink",
    "gray",
    "light_gray",
    "cyan",
    "purple",
    "blue",
    "brown",
    "green",
    "red",
    "black",
];

/// The **six** rows of the exclusion table, in the order Java declares them
/// (`WindowBannerPicker.java:77-95`), by Java's constant name.
///
/// Row `i` holds the patterns that a town hall of level **`i`** already knows and that must therefore
/// be *excluded* from a hall of level `level <= i` — the loop reads `EXCLUSION[level] ..= EXCLUSION[max]`
/// (`:144-150`), so the row at the hall's **own** level is the first one dropped.
///
/// The last row is the odd one: `{ BannerPatterns.BASE }`, commented "Excluded completely" (`:92-94`).
/// It is why the base pattern never appears in the grid.
pub const EXCLUSION: [&[&str]; 6] = [
    &["BannerPatterns.GRADIENT", "BannerPatterns.GRADIENT_UP"],
    &["BannerPatterns.BRICKS", "BannerPatterns.FLOWER"],
    &["BannerPatterns.SKULL", "BannerPatterns.CREEPER"],
    &["BannerPatterns.GLOBE", "BannerPatterns.PIGLIN"],
    &["BannerPatterns.MOJANG"],
    &["BannerPatterns.BASE"],
];

/// One layer of the flag: a pattern and the dye it is drawn in.
///
/// Both are ids rather than values: the pattern is a registry id and the dye one of [`DYES`]. Java
/// pairs them as `Pair<Holder<BannerPattern>, DyeColor>` (`:101`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Layer {
    /// The pattern's registry id, `minecraft:base` for the base layer.
    pub pattern: &'static str,
    /// The dye's id, one of [`DYES`].
    pub colour: &'static str,
}

/// What one edit does to the layer list, so that the caller owns the mutation.
///
/// Java mutates `layers` in place (`:283`, `:285`, `:192`); the four outcomes are the same four, named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four things `setLayer` and the X button can do
pub enum LayerEdit {
    /// Nothing: a colour click past the last layer (`:277`, `return`).
    Unchanged,
    /// Replace the layer at this index (`layers.set`, `:285`).
    Replace { at: usize, layer: Layer },
    /// Add a layer at the end (`layers.add`, `:283`).
    Append(Layer),
    /// Remove the layer at this index (`layers.remove(activeLayer)`, `:192`).
    RemoveAt(usize),
}

/// The picker's own state: the flag it is editing, and where the player is in it.
///
/// Five fields, and Java's five (`:100-127`): the layers, the active layer, the scroll row, whether
/// the patreon feature is unlocked, and the selected dye. `scrolling` (`:124`) is a mouse-drag
/// latch — it belongs to the input handler and isn't carried.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picker {
    /// The flag as saved, base layer included: [`initial_layers`].
    pub layers: Vec<Layer>,
    /// The layer being edited, zero for the base (`:121`).
    pub active: usize,
    /// How many rows of the pattern grid are scrolled past (`:127`).
    pub scroll: usize,
    /// Whether the patreon feature is unlocked (`:115`), which decides [`blocked`].
    pub unlocked: bool,
    /// The dye a pattern click uses, one of [`DYES`] (`ColorPalette.selected`, `:31`).
    pub colour: &'static str,
}

impl Picker {
    /// A picker on a saved flag, opened on the base layer with white selected.
    pub fn new(flag: &[Layer], unlocked: bool) -> Picker {
        Picker {
            layers: initial_layers(flag),
            active: 0,
            scroll: 0,
            unlocked,
            colour: "white",
        }
    }

    /// The layer being edited, `None` when the player is past the last one — the state in which a
    /// colour click appends (`:282-283`).
    pub fn active_layer(&self) -> Option<Layer> {
        self.layers.get(self.active).copied()
    }
}

/// The flag the picker edits, from the colony's saved one
/// (`WindowBannerPicker.java:157-160`).
///
/// Java calls vanilla's `BannerBlockEntity.createPatterns(DyeColor.WHITE, flag)`, which **prepends** a
/// white base layer, then drops that first entry when the list is longer than one. The net rule is the
/// one here: a saved flag answers itself; an **empty** flag answers a single white base layer. This is
/// reachable: `ColonyView`'s own default flag is already one BASE/WHITE layer
/// (`ColonyView.java:110-112`), so a fresh colony opens the picker with exactly one layer.
pub fn initial_layers(flag: &[Layer]) -> Vec<Layer> {
    if flag.is_empty() {
        vec![Layer { pattern: BASE_PATTERN, colour: "white" }]
    } else {
        flag.to_vec()
    }
}

/// The patterns a town hall of `level` must not offer, from the source of its level to its maximum
/// (`WindowBannerPicker.java:144-150`).
///
/// The range is Java's: **from `level`**, not from `level + 1`, so the row of the hall's own level is
/// the first one dropped and row 0 is only dropped by a level-0 hall. The refusal is
/// [`Refusal::LevelOutOfRange`]: the table has six rows and Java would throw past them.
pub fn excluded(level: i32, max_level: i32) -> Result<Vec<&'static str>, Refusal> {
    if level < 0 || max_level < level || max_level as usize >= EXCLUSION.len() {
        return Err(Refusal::LevelOutOfRange);
    }
    let mut dropped = Vec::new();
    for row in &EXCLUSION[level as usize..=max_level as usize] {
        dropped.extend_from_slice(row);
    }
    Ok(dropped)
}

/// What a **colour** click does to the layer at the picker's index
/// (`WindowBannerPicker.java:272-286`).
///
/// Java's three branches, in its order:
/// 1. no pattern given and the index is **past the end** → nothing at all (`:277`);
/// 2. no pattern given and the index is **zero** → the pattern stays the base (`:278`);
/// 3. no pattern given otherwise → the pattern stays what it was (`:279`).
///
/// Then appending past the end (`:282-283`) or replacing (`:284-285`). A pattern click always gives the
/// pattern, so it only ever takes the last two steps.
pub fn set_layer(
    picker: &Picker,
    pattern: Option<&'static str>,
    colour: &'static str,
) -> Result<LayerEdit, Refusal> {
    let active = picker.active;
    if active > picker.layers.len() {
        return Err(Refusal::LayerOutOfRange);
    }
    let pattern = match pattern {
        Some(pattern) => pattern,
        None => {
            if active == picker.layers.len() {
                return Ok(LayerEdit::Unchanged);
            }
            if active == 0 {
                BASE_PATTERN
            } else {
                picker.layers[active].pattern
            }
        }
    };
    let layer = Layer { pattern, colour };
    if active == picker.layers.len() {
        Ok(LayerEdit::Append(layer))
    } else {
        Ok(LayerEdit::Replace { at: active, layer })
    }
}

/// What the red X does (`WindowBannerPicker.java:188-200`).
///
/// The button is enabled only when `activeLayer < layers.size() && activeLayer != 0` (`:197`), so the
/// **base layer cannot be removed** and neither can a layer that is not there. Two reasons, two names.
pub fn remove_layer(picker: &Picker) -> Result<LayerEdit, Refusal> {
    if picker.active >= picker.layers.len() {
        return Err(Refusal::LayerOutOfRange);
    }
    if picker.active == 0 {
        return Err(Refusal::BaseLayerNotRemovable);
    }
    Ok(LayerEdit::RemoveAt(picker.active))
}

/// Whether the X is enabled for this state — Java's own test, `:197`.
pub fn can_remove(active: usize, layers: usize) -> bool {
    active < layers && active != 0
}

/// Whether the layer button of `layer` is enabled: `this.active = this.layer <= layers.size()`
/// (`WindowBannerPicker.java:481`).
///
/// **`<=`, not `<`**: the button *one past the last layer* stays enabled, and that is exactly the
/// button that appends a new layer ([`set_layer`]).
pub fn layer_enabled(layer: usize, layers: usize) -> bool {
    layer <= layers
}

/// The label of the layer button of `layer` (`WindowBannerPicker.java:458-460`).
pub fn layer_label(layer: usize) -> &'static str {
    match layer {
        0 => BASE_LAYER_KEY,
        1 => "1",
        2 => "2",
        3 => "3",
        4 => "4",
        5 => "5",
        6 => "6",
        _ => "",
    }
}

/// How many rows of the grid hold `patterns` patterns, rounded up (`:296`).
fn grid_rows(patterns: usize) -> usize {
    patterns.div_ceil(PATTERN_COLUMNS)
}

/// The highest scroll row, from the pattern count (`WindowBannerPicker.java:296-298`, `:398`).
///
/// Java writes `ceil(size / 8) - PATTERN_ROWS + 1`, which is [`grid_rows`] minus three. When the grid
/// has fewer than three rows that number is **negative**, and what vanilla's `Mth.clamp` does with a
/// negative bound is not something this checkout carries: the port floors it at zero and says so here
/// rather than claiming Java's answer.
pub fn scroll_bound(patterns: usize) -> usize {
    grid_rows(patterns).saturating_sub(PATTERN_ROWS - 1)
}

/// The scroll row a wheel or a drag lands on (`WindowBannerPicker.java:394-400`, `:427-431`).
///
/// `delta` positive scrolls up. Java clamps `scrollRow - scroll` into `0..=bound` as a `double` and
/// then casts to `int`, so a fractional notch moves nothing — the cast is kept.
pub fn next_scroll(picker: &Picker, patterns: usize, delta: f64) -> usize {
    if picker.active == 0 {
        return picker.scroll;
    }
    let bound = scroll_bound(patterns);
    ((picker.scroll as f64) - delta).clamp(0.0, bound as f64) as usize
}

/// Whether the pattern at `index` is inside the four visible rows
/// (`WindowBannerPicker.java:532`).
pub fn visible(index: usize, scroll: usize) -> bool {
    scroll * PATTERN_COLUMNS <= index && index < PATTERN_COLUMNS * (scroll + PATTERN_ROWS)
}

/// Whether a pattern is blacked out by the patreon lock
/// (`WindowBannerPicker.java:216-220`).
///
/// `true` only for a pattern of the mod's own namespace while the feature is locked. No such pattern
/// exists in today's registry, so this answers `false` for every id vanilla registers.
pub fn blocked(namespace: &str, unlocked: bool) -> bool {
    !unlocked && namespace == JAVA_MOD_ID
}

/// The dye a pattern button is drawn in: black when locked, white otherwise
/// (`WindowBannerPicker.java:347-355`).
pub fn pattern_colour(namespace: &str, unlocked: bool) -> &'static str {
    if blocked(namespace, unlocked) {
        "black"
    } else {
        "white"
    }
}

/// The dye selected after a layer button is clicked
/// (`WindowBannerPicker.java:468-476`).
///
/// Past the last layer, Java picks the dye that **contrasts with the base**: white if the base is
/// black, black otherwise (`:473`). On an existing layer it picks the layer's own dye (`:475`), so
/// clicking a layer shows its colour in the palette.
pub fn next_selected_colour(layers: &[Layer], layer: usize) -> &'static str {
    if layer >= layers.len() {
        return match layers.first().map(|first| first.colour) {
            Some("black") => "white",
            // Java would find no base only through an empty list, and `layers.get(0)` would throw.
            None => "white",
            Some(_) => "black",
        };
    }
    layers[layer].colour
}

/// The three buttons of the picker that are not generated.
///
/// The seven layer buttons ([`LAYER_BUTTONS`]) and the pattern buttons are built in a loop from data
/// (`:181-186`, `:208-214`), so they have no static list; their rules are [`layer_enabled`],
/// [`layer_label`], [`visible`] and [`blocked`].
pub const BUTTONS: [Button; 3] = [
    // The X. Its label is a literal, `ChatFormatting.RED + "X"` (:191), not a key, and its click
    // sends no message at all: it only drops a layer of the window's own list (`:192`), which is
    // what `Effect::Toggles` already means for a state that lives in the window.
    Button::guarded(
        "layer.remove",
        "X",
        Effect::Toggles,
        &[Refusal::BaseLayerNotRemovable, Refusal::LayerOutOfRange],
    ),
    // Done: `colony.setColonyFlag(...)`, which sends this message (ColonyView.java:1188-1191), then
    // `window.open()` (:234-241).
    Button::plain("done", "gui.done", Effect::Sends("ColonyFlagChangeMessage")).then(Afterwards::Reopens),
    // Cancel: `window.open()` and nothing else (:243-249).
    Button::plain("cancel", "gui.cancel", Effect::Reopens),
];

/// The picker's buttons, by id.
pub fn buttons() -> &'static [Button] {
    &BUTTONS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(pattern: &'static str, colour: &'static str) -> Layer {
        Layer { pattern, colour }
    }

    #[test]
    fn an_empty_flag_gives_one_white_base_layer_and_a_saved_flag_answers_itself() {
        let fresh = initial_layers(&[]);
        assert_eq!(fresh, [layer(BASE_PATTERN, "white")]);
        // A colony's default flag already holds that one base layer (ColonyView.java:110-112), so
        // the picker opens on a single layer by the other route.
        let saved = [layer(BASE_PATTERN, "white")];
        assert_eq!(initial_layers(&saved), saved);
        // Two saved layers stay two: the prepended base is dropped, not kept.
        let two = [layer(BASE_PATTERN, "red"), layer("minecraft:bricks", "blue")];
        assert_eq!(initial_layers(&two), two);
        let picker = Picker::new(&two, false);
        assert_eq!(picker.active, 0);
        assert_eq!(picker.colour, "white");
        assert_eq!(picker.active_layer(), Some(two[0]));
    }

    #[test]
    fn the_exclusion_is_a_range_from_the_halls_own_level_up_to_its_maximum() {
        // A level-1 hall with a maximum of 5: rows 1..=5 are dropped, row 0 is **kept**.
        let dropped = excluded(1, 5);
        assert!(dropped.is_ok());
        if let Ok(dropped) = dropped {
            assert_eq!(dropped.len(), 8);
            assert!(!dropped.contains(&"BannerPatterns.GRADIENT"), "level-1 patterns are offered");
            assert!(!dropped.contains(&"BannerPatterns.GRADIENT_UP"));
            assert!(dropped.contains(&"BannerPatterns.BRICKS"), "level-2 patterns are not");
            assert!(dropped.contains(&"BannerPatterns.BASE"), "the last row is dropped always");
        }
        // A level-0 hall drops row 0 too — the only way the gradients ever disappear.
        if let Ok(dropped) = excluded(0, 5) {
            assert_eq!(dropped.len(), 10);
            assert!(dropped.contains(&"BannerPatterns.GRADIENT"));
        }
        // A level-5 hall drops the base and nothing else.
        if let Ok(dropped) = excluded(5, 5) {
            assert_eq!(dropped, ["BannerPatterns.BASE"]);
        }
        // Past the six rows Java throws; the port names it.
        assert_eq!(excluded(5, 6), Err(Refusal::LevelOutOfRange));
        assert_eq!(excluded(6, 6), Err(Refusal::LevelOutOfRange));
        assert_eq!(excluded(3, 2), Err(Refusal::LevelOutOfRange));
        assert_eq!(excluded(-1, 5), Err(Refusal::LevelOutOfRange));
        assert_eq!(Refusal::LevelOutOfRange.key(), None);
    }

    #[test]
    fn a_colour_click_past_the_end_appends_and_on_the_base_keeps_the_base() {
        let picker = Picker::new(&[layer(BASE_PATTERN, "white")], false);
        // Layer 0, colour only: the base pattern is kept (`:278`).
        assert_eq!(
            set_layer(&picker, None, "red"),
            Ok(LayerEdit::Replace { at: 0, layer: layer(BASE_PATTERN, "red") })
        );
        // Past the end with a pattern: appended (`:282-283`).
        let mut past = picker.clone();
        past.active = 1;
        assert_eq!(
            set_layer(&past, Some("minecraft:bricks"), "blue"),
            Ok(LayerEdit::Append(layer("minecraft:bricks", "blue")))
        );
        // Past the end with a colour only: nothing at all (`:277`).
        assert_eq!(set_layer(&past, None, "blue"), Ok(LayerEdit::Unchanged));
        // A pattern click on an existing layer replaces it, keeping the caller's pattern.
        assert_eq!(
            set_layer(&picker, Some("minecraft:skull"), "green"),
            Ok(LayerEdit::Replace { at: 0, layer: layer("minecraft:skull", "green") })
        );
        // Two layers past the end is a broken index, not an append.
        let mut too_far = picker.clone();
        too_far.active = 5;
        assert_eq!(set_layer(&too_far, None, "red"), Err(Refusal::LayerOutOfRange));
    }

    #[test]
    fn the_base_layer_cannot_be_removed_and_neither_can_a_layer_that_is_not_there() {
        let mut picker = Picker::new(&[layer(BASE_PATTERN, "white"), layer("minecraft:bricks", "red")], false);
        assert_eq!(remove_layer(&picker), Err(Refusal::BaseLayerNotRemovable));
        assert!(!can_remove(0, 2), "WindowBannerPicker.java:197");
        assert!(can_remove(1, 2));
        picker.active = 1;
        assert_eq!(remove_layer(&picker), Ok(LayerEdit::RemoveAt(1)));
        // A layer index equal to the count is not removable either, whatever the base rule says.
        picker.active = 2;
        assert_eq!(remove_layer(&picker), Err(Refusal::LayerOutOfRange));
        assert!(!can_remove(2, 2));
        assert_ne!(Refusal::BaseLayerNotRemovable.name(), Refusal::LayerOutOfRange.name());
    }

    #[test]
    fn the_layer_buttons_stop_at_seven_and_the_one_past_the_end_appends() {
        assert_eq!(LAYER_BUTTONS, 7);
        assert_eq!(layer_label(0), BASE_LAYER_KEY);
        assert_eq!(layer_label(1), "1", "the six other labels are literal digits, :460");
        assert_eq!(layer_label(6), "6");
        // With one layer, buttons 0 and 1 are enabled — 1 is the append button.
        assert!(layer_enabled(0, 1));
        assert!(layer_enabled(1, 1));
        assert!(!layer_enabled(2, 1));
        // With seven layers (the most seven buttons can reach), all seven are enabled.
        for layer in 0..LAYER_BUTTONS {
            assert!(layer_enabled(layer, LAYER_BUTTONS));
        }
    }

    #[test]
    fn the_grid_shows_four_rows_and_the_scroll_stops_at_the_last_page() {
        // 40 patterns: five rows, so two rows of scroll.
        assert_eq!(scroll_bound(40), 2);
        // Exactly four rows: no scrolling at all.
        assert_eq!(scroll_bound(32), 1);
        assert_eq!(scroll_bound(24), 0);
        // Fewer than three rows makes Java's bound negative; the port floors it instead.
        assert_eq!(scroll_bound(3), 0);
        assert_eq!(scroll_bound(0), 0);
        // The wheel moves one row at a time, and never past the bound. `delta` positive scrolls up,
        // because Java computes `scrollRow - scroll` (:396).
        let mut picker = Picker::new(&[layer(BASE_PATTERN, "white")], false);
        picker.active = 1;
        assert_eq!(next_scroll(&picker, 40, -1.0), 1);
        assert_eq!(next_scroll(&picker, 40, 9.0), 0, "clamped at the first row");
        let mut scrolled = picker.clone();
        scrolled.scroll = 2;
        assert_eq!(next_scroll(&scrolled, 40, -1.0), 2, "clamped at the last page");
        assert_eq!(next_scroll(&scrolled, 40, 1.0), 1);
        // A fractional notch moves nothing: Java casts the clamped double to int.
        assert_eq!(next_scroll(&picker, 40, 0.5), 0);
        // On the base layer the wheel is swallowed and the row does not move (:394).
        let mut on_base = scrolled.clone();
        on_base.active = 0;
        assert_eq!(next_scroll(&on_base, 40, -1.0), 2);
        // Which patterns the third scroll row shows.
        assert!(!visible(15, 2));
        assert!(visible(16, 2));
        assert!(visible(47, 2));
        assert!(!visible(48, 2), "four rows of eight, from 16 to 47");
    }

    #[test]
    fn clicking_a_layer_button_picks_that_layers_dye_or_a_dye_that_contrasts_with_the_base() {
        let layers = [layer(BASE_PATTERN, "white"), layer("minecraft:bricks", "red")];
        assert_eq!(next_selected_colour(&layers, 1), "red", "the layer's own dye, :475");
        assert_eq!(next_selected_colour(&layers, 2), "black", "white base, so black, :473");
        let black_base = [layer(BASE_PATTERN, "black")];
        assert_eq!(next_selected_colour(&black_base, 5), "white");
        assert_eq!(DYES.len(), 16);
        assert_eq!(DYES[0], "white");
        assert_eq!(DYES[15], "black");
    }

    #[test]
    fn the_patreon_lock_only_fires_on_a_mod_namespace_pattern_and_none_exists_today() {
        assert!(blocked(JAVA_MOD_ID, false));
        assert!(!blocked(JAVA_MOD_ID, true));
        assert!(!blocked("minecraft", false));
        assert_eq!(pattern_colour(JAVA_MOD_ID, false), "black");
        assert_eq!(pattern_colour("minecraft", false), "white");
        // The registry this checkout ships has no pattern under the mod's namespace: the branch is a
        // rule without an instance, which is why it is carried and not exercised.
        assert_ne!(JAVA_MOD_ID, crate::MOD_ID, "the port's id is not Java's mod id");
    }

    #[test]
    fn done_writes_the_flag_and_reopens_the_town_hall_while_cancel_only_reopens() {
        let done = BUTTONS.iter().find(|button| button.id == "done");
        assert!(done.is_some());
        if let Some(done) = done {
            assert_eq!(done.effect, Effect::Sends("ColonyFlagChangeMessage"));
            assert_eq!(done.afterwards, Afterwards::Reopens);
            assert!(!done.is_guarded());
        }
        let cancel = BUTTONS.iter().find(|button| button.id == "cancel");
        if let Some(cancel) = cancel {
            assert_eq!(cancel.effect, Effect::Reopens);
            assert_eq!(cancel.afterwards, Afterwards::Nothing);
        }
        let remove = BUTTONS.iter().find(|button| button.id == "layer.remove");
        if let Some(remove) = remove {
            assert_eq!(remove.label, "X", "a literal, WindowBannerPicker.java:191");
            assert_eq!(remove.refusals.len(), 2);
            assert_eq!(remove.afterwards, Afterwards::Nothing, "the window stays open");
        }
        assert_eq!(buttons().len(), 3);
        assert!(!TITLE.contains('.'), "the picker's title is not a translation key");
        assert!(CHOOSE_KEY.starts_with("com.minecolonies."));
        assert!(PATREON_KEY.starts_with("com.minecolonies."));
    }
}
