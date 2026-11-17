//! The widget layer: what a screen holds, where it sits, and what the SDK can draw.
//!
//! **One sentence.** A window is panes with a rectangle and a state; this module lays
//! them out in GUI pixels, refuses two that cover the same pixels, and turns each into
//! the drawing calls Horizon really has — and nothing else.
//!
//! # Three parts, deliberately apart
//!
//! | Part | Question it answers | What it never does |
//! | --- | --- | --- |
//! | [`model`] | what the window holds, and in what state | touch the game |
//! | [`layout`] | which pixels each pane occupies, and which pair collides | draw |
//! | [`adapt`] | which call of `DrawContext` draws this pane | decide |
//!
//! The model is data, so it is tested without a game, a font or a hook:
//! `cargo test -p ported-minecolonies widgets` runs the whole layer on the host target.
//! **This module never draws.** [`adapt::plan`] answers a list of [`adapt::Op`], and
//! [`adapt::execute`] is the one function that calls the drawing context; hooking it is
//! the next slice, not this one.
//!
//! # The four widgets, and what each becomes
//!
//! MineColonies describes its 94 screens in BlockUI XML; four elements carry almost all
//! of them (`/workspace/ISSUES_WIDGETS.md` counts: 342 `<text>` in 86 files, 271
//! `<button>` in 78, 64 `<list>` in 45, and a progress bar is a `Gradient` over an
//! `Image`).
//!
//! | BlockUI | [`model`] | [`adapt`] asks for | Gap when the SDK cannot |
//! | --- | --- | --- | --- |
//! | `Text` | [`model::Label`] | `text`, or `gui_paint` for a component | wrap, cut, scale, measure |
//! | `Button` / `ButtonImage` | [`model::Button`] | `widget_button`, or `gui_nine_slice` + `text` | the narration of a sprite button |
//! | `Gradient` bar | [`model::Bar`] | two `fill` | a fading fill |
//! | `ScrollingList` | [`model::List`] | `push_clip` + `text` per row + `pop_clip` | the scrollbar |
//!
//! # What the API really offers, measured
//!
//! - the drawing context is `DrawContext`
//!   (`crates/horizon-mod-vocab/src/host/draw.rs:26-172`): `fill` (`:43`), `text` on a
//!   **`&str`** (`:61`), `sprite` (`:51`), `gui_nine_slice` (`:78`), `gui_tooltip`
//!   (`:83`), `text_width` on a **`&str`** (`:87`), `push_clip` / `pop_clip`
//!   (`:122`, `:128`), `widget_button` (`:155`), `widget_text_field` (`:170`), and the
//!   batch `gui_paint` (`:74`) whose `GuiDraw::Text` is the only text call that takes a
//!   [`Text`](mod_api::Text) (`crates/horizon-mod-vocab/src/client/hud_layout.rs:87-93`).
//! - the clip exists, so a scrolling list is drawable: `push_clip` cuts sprites, glyphs
//!   and items, nests by intersection, and is closed at the end of the hook call
//!   (`host/draw.rs:112-130`). `/workspace/ISSUES_WIDGETS.md` was written before that
//!   landed and says the opposite; the code is what counts.
//! - where such a plan is executed is the screen hooks: `screen_render_post`
//!   (`crates/horizon-mod-vocab/src/hooks/screen_render_post.rs:4-20`), `screen_draw`
//!   for a block's own screen (`hooks/screen_draw.rs:4-17`), or `gui_layer_draw` for a
//!   HUD layer with `register_gui_layers` (`hooks/gui_layer_draw.rs:4-13`).
//! - the wheel a list needs is `screen_mouse_scrolled_pre`, which gives the screen, the
//!   pointer and the movement in x and y and can cancel the screen's own scrolling
//!   (`hooks/screen_mouse_scrolled_pre.rs:6-31`). [`model::List::set_scroll`] is the
//!   arithmetic it feeds.
//!
//! # Two premises to check before believing a report
//!
//! The brief for this slice said `text_lines` (#4490) and the sprite button (#4491)
//! were merged and available. **They are not in this build.** `sdk.json` pins
//! `build.commit db7638b30c`, and neither `DrawContext::text_lines` nor `ButtonSprites`
//! exists anywhere in `.horizon/sdk/api/crates/` — the two PRs live on other branches
//! (`/workspace/RAPPORT_4490.md`, `/workspace/RAPPORT_4491.md`). No code here may call
//! them; [`adapt::Gap`] names what their absence costs, and `MANQUANT.md` says it in
//! full.
//!
//! # What this slice does not do
//!
//! - **No nesting.** One pane tree level: a list row is one line, not a box with an icon
//!   and a button (`model::Row`).
//! - **No input.** Clicks, the wheel's effect on a list, focus and hover beyond the
//!   pointer position a plan is given: the input slice.
//! - **No hooks.** Nothing here implements `Hooks`; `lib.rs` declares the module
//!   (`pub mod widgets;`) and wiring a plan to `screen_render_post` is the next slice.

pub mod adapt;
pub mod layout;
pub mod model;

#[cfg(test)]
mod tests;

pub use adapt::{Call, Gap, Metrics, Op, Plan, execute, plan};
pub use layout::{Container, Layout, Overlap, Placed, Refusal, check, place, resolve, visible_rows};
pub use model::{
    Alignment, Bar, Body, Button, Id, LABEL_DEFAULTS, Label, LabelDefaults, Line, List, Pane, Rect,
    Row, Screen, State, Widget,
};
