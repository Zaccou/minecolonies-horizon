//! The model against the calls that exist — and the named gaps where none does.
//!
//! **The rule of this module: ask for what the SDK draws today, name what it does not.**
//! A [`Plan`] is a list of [`Op`], and every `Op` is one call of
//! `DrawContext` (`crates/horizon-mod-vocab/src/host/draw.rs:26-172`) with values
//! already computed. Nothing here decides anything about look or a hook.
//!
//! The API this build offers, and what each part of a widget can therefore become:
//!
//! | What a widget needs | Call | Where |
//! | --- | --- | --- |
//! | a flat rectangle (a bar's track and fill) | `fill` | `host/draw.rs:42-45` |
//! | one plain line of text | `text` | `:59-63` |
//! | a component (a translation, a style) | `gui_paint` + `GuiDraw::Text` | `:72-76`, `client/hud_layout.rs:87-93` |
//! | the game's own button, narrated | `widget_button` | `:149-163` |
//! | a button on the mod's sprite, nine-sliced | `gui_nine_slice` | `:77-80` |
//! | a scrolling list's viewport | `push_clip` / `pop_clip` | `:112-130` |
//! | the width of a plain line | `text_width` | `:86-89` |
//!
//! The `text_lines` hook (issue #4490) and `ButtonSprites` (#4491) are **not in this
//! build**: `.horizon/sdk/sdk.json` pins `build.commit db7638b30c`, and the
//! `DrawContext` there has neither, so no code here may assume them. Wrapping a label,
//! scaling it, and drawing a sprite button *with* its narration are the gaps below;
//! `MANQUANT.md` says what each one costs on MineColonies' own screens.

use std::fmt;

use mod_api::{DrawContext, GuiClipRefusal, GuiDraw, Text, TextFormError};

use crate::widgets::layout::Layout;
use crate::widgets::model::{
    Alignment, Bar, Body, Button, Id, LABEL_DEFAULTS, Label, Line, List, Rect, Screen, State,
};

/// What the plan needs from the font: `Font.lineHeight` and `Font.width`, which the
/// game gives through `DrawContext::text_width` — for a **plain** string only
/// (`crates/horizon-mod-vocab/src/host/draw.rs:86-89`).
///
/// A trait rather than a struct because the game's font and a test's fake are the two
/// callers, and neither should have to know the other.
pub trait Metrics {
    /// `Font.lineHeight` in GUI pixels: 9 for the game's default font, and the step
    /// BlockUI adds `textLinespace` to between two lines
    /// (`controls/AbstractTextElement.java:331-369`).
    fn line_height(&self) -> f32;

    /// `Font.width`, for a line that has its characters ([`Line::plain`]).
    fn width(&self, text: &str) -> f32;
}

/// One drawing call, with everything it needs already worked out.
///
/// The variants are exhaustive on purpose: the game matches them without a wildcard, so
/// a new call is a compile error here rather than a silently unhandled case.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    /// `DrawContext::fill` (`host/draw.rs:42-45`): a rectangle and an ARGB colour.
    Fill {
        /// Where, in GUI pixels.
        rect: Rect,
        /// `0xAARRGGBB`.
        argb: u32,
    },
    /// One drawn line. `DrawContext::text` takes a `&str` (`:59-63`), so a line with
    /// [`Line::plain`] goes there; a component goes through `gui_paint` with
    /// `GuiDraw::Text` (`:72-76`, `client/hud_layout.rs:87-93`), which is the only call
    /// that takes a `Text`. [`execute`] picks the route.
    Text {
        /// The line, component and characters together.
        line: Line,
        /// Top left, GUI pixels.
        at: [f32; 2],
        /// `0xAARRGGBB`.
        argb: u32,
        /// The game's drop shadow.
        shadow: bool,
    },
    /// `DrawContext::widget_button` (`:149-163`): the game's own button, recorded for
    /// the narrator and a controller's focus.
    Button {
        /// Where, in GUI pixels.
        rect: Rect,
        /// The centred label.
        label: Text,
        /// `Widgets`' active state: greyed and inert when false.
        active: bool,
        /// Drawn highlighted: the pointer is on it, or it has the focus.
        highlighted: bool,
    },
    /// `DrawContext::gui_nine_slice` (`:77-80`): a sprite stretched in nine, which is
    /// how a button on a mod's own texture is drawn. The narration a
    /// [`Op::Button`] gets is lost — see [`Gap::ButtonNarration`].
    NineSlice {
        /// The sprite, `mymod:gui/…` for a content mod's.
        sprite: &'static str,
        /// Where, in GUI pixels.
        rect: Rect,
        /// The tint, ARGB.
        argb: u32,
    },
    /// `DrawContext::push_clip` (`:112-124`): from here on, nothing outside `rect` is
    /// drawn. A list's viewport is a clip.
    Clip {
        /// `[x, y, w, h]`, GUI pixels.
        rect: Rect,
    },
    /// `DrawContext::pop_clip` (`:125-130`): closes the [`Op::Clip`] before it.
    Unclip,
}

/// Something the model asks for and this build of the SDK has no call for.
///
/// Each variant names the pane and what was lost, so a report can list them instead of
/// a screen quietly looking wrong.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Gap {
    /// A label with `wrap`: the split of a component into lines is `Font.split`, which
    /// is the engine's, and `text_lines` (#4490) is not in this build. Nothing is drawn
    /// for that label (`MANQUANT.md` §1).
    WrappedLabel {
        /// The label.
        id: Id,
        /// How many paragraphs it asked to wrap.
        blocks: usize,
    },
    /// A line that has to be cut to its pane: Java splits it at the box width and keeps
    /// the first piece (`controls/AbstractTextElement.java:211-213`, `:266-271`). The
    /// split is the same missing call, so the line is **drawn whole**, overflowing —
    /// and this says so.
    CutLine {
        /// The label.
        id: Id,
        /// The line's measured width.
        width: f32,
        /// The width the pane gives it.
        room: f32,
    },
    /// A label that has to be centred or right-aligned and whose line has no plain
    /// form: a component's width is not knowable, because `text_width` takes a `&str`
    /// (`host/draw.rs:86-89`) and the SDK does not export `TextContent`
    /// (`crates/horizon-mod-vocab/generated/mod_api_exports_root.rs:265-266`). The line is drawn at
    /// the pane's left edge instead.
    Unmeasurable {
        /// The label.
        id: Id,
    },
    /// A label with a `textscale` other than 1: no drawing call takes a scale
    /// (`host/draw.rs:59-63`), and `GuiDraw::Push` needs `PoseOp`, which a mod cannot
    /// name. The line is drawn at the font's own size.
    ScaledText {
        /// The label.
        id: Id,
        /// The scale it asked for.
        scale: f32,
    },
    /// A bar whose fill is a `Gradient` (`controls/Gradient.java:9-60`): drawn flat,
    /// because `fill` is one colour (`host/draw.rs:42-45`) and the batch's own fill is
    /// flat too (`client/hud_layout.rs:80-86`).
    Gradient {
        /// The bar.
        id: Id,
    },
    /// A button on a sprite: drawn, but not recorded for the narrator or a controller's
    /// focus the way `widget_button` records it (`host/draw.rs:149-163`). `ButtonSprites`
    /// (#4491) would give the three states back.
    ButtonNarration {
        /// The button.
        id: Id,
        /// The sprite it is drawn on.
        sprite: &'static str,
    },
    /// A button with more than one paragraph: `widget_button` centres a single line, so
    /// the rest are not drawn (`host/draw.rs:151-160`).
    ButtonLines {
        /// The button.
        id: Id,
        /// How many paragraphs were dropped.
        dropped: usize,
    },
    /// A list with a scrollbar: its viewport is narrowed by
    /// [`List::SCROLLBAR_WIDTH`] (`views/ScrollingView.java:12`, `:43`), but the thumb —
    /// sprite, offset, drag — is not modelled, so nothing is drawn in the strip.
    Scrollbar {
        /// The list.
        id: Id,
    },
}
impl Gap {
    /// The pane this gap is about.
    pub fn id(self) -> Id {
        match self {
            Gap::WrappedLabel { id, .. }
            | Gap::CutLine { id, .. }
            | Gap::Unmeasurable { id }
            | Gap::ScaledText { id, .. }
            | Gap::Gradient { id }
            | Gap::ButtonNarration { id, .. }
            | Gap::ButtonLines { id, .. }
            | Gap::Scrollbar { id } => id,
        }
    }

    /// What was lost, in one line, for a report.
    pub fn reason(self) -> &'static str {
        match self {
            Gap::WrappedLabel { .. } => "a wrapped label needs Font.split, absent from this SDK",
            Gap::CutLine { .. } => "a line wider than its pane is drawn uncut",
            Gap::Unmeasurable { .. } => "a component cannot be measured, so it is not aligned",
            Gap::ScaledText { .. } => "no drawing call takes a text scale",
            Gap::Gradient { .. } => "a gradient bar is drawn flat",
            Gap::ButtonNarration { .. } => "a sprite button is not recorded for the narrator",
            Gap::ButtonLines { .. } => "a button draws one paragraph, the rest are dropped",
            Gap::Scrollbar { .. } => "a list's scrollbar thumb is not drawn",
        }
    }
}

/// A refusal from one of the drawing calls [`execute`] made.
#[derive(Clone, Debug, PartialEq)]
pub enum Call {
    /// A text was too deep for the engine's component tree: `TextFormError` is the same
    /// refusal `widget_button` and `gui_paint` answer (`host/draw.rs:154`, `:82`).
    Text(TextFormError),
    /// A clip the context would not open or close (`GuiClipRefusal`,
    /// `client/gui_clip.rs:5-15`).
    Clip(GuiClipRefusal),
}

impl fmt::Display for Call {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Call::Text(error) => write!(f, "text: {error}"),
            Call::Clip(refusal) => write!(f, "clip: {refusal:?}"),
        }
    }
}

impl std::error::Error for Call {}

/// One frame's worth of drawing: the calls to make, and what could not be asked for.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    /// In draw order.
    pub ops: Vec<Op>,
    /// Every model request no call could answer.
    pub gaps: Vec<Gap>,
}

impl Plan {
    /// Whether anything is drawn at all.
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }
}

/// Turns a laid-out screen into the calls of one frame.
///
/// `mouse` is the pointer in the same GUI pixels the layout uses
/// (`screen.mouse`, `crates/horizon-mod-vocab/src/hooks/screen_mouse_scrolled_pre.rs:6-13`);
/// it only decides the hovered colour and a button's highlight, exactly as
/// `wasCursorInPane` does in Java (`controls/AbstractTextElement.java:288`).
pub fn plan(screen: &Screen, layout: &Layout, metrics: &dyn Metrics, mouse: [f32; 2]) -> Plan {
    let mut plan = Plan::default();
    for (widget, rect) in layout.iter(screen) {
        // `Pane.shouldDraw` is `visible` (`Pane.java:205-210`); a disabled pane still
        // draws, greyed (`AbstractTextElement.java:288`).
        if !widget.pane.state.visible {
            continue;
        }
        let id = widget.pane.id;
        match &widget.body {
            Body::Label(label) => {
                label_ops(id, rect, label, widget.pane.state, mouse, metrics, &mut plan);
            }
            Body::Button(button) => {
                button_ops(id, rect, button, widget.pane.state, mouse, metrics, &mut plan);
            }
            Body::Bar(bar) => bar_ops(id, rect, bar, &mut plan),
            Body::List(list) => list_ops(id, rect, list, metrics, &mut plan),
        }
    }
    plan
}

/// The ARGB a text element draws in: `enabled ? (hovered ? hover : normal) : disabled`
/// (`controls/AbstractTextElement.java:288`).
fn color_of(state: State, hovered: bool, normal: u32, hover: Option<u32>, disabled: Option<u32>) -> u32 {
    if !state.enabled {
        disabled.unwrap_or(normal)
    } else if hovered {
        hover.unwrap_or(normal)
    } else {
        normal
    }
}

/// How tall one drawn line is: `(lineHeight - 1) * scale`, the `-1` being Java's
/// (`controls/AbstractTextElement.java:270`).
fn drawn_height(metrics: &dyn Metrics, scale: f32) -> f32 {
    (metrics.line_height() - 1.0) * scale
}

/// The x of a block of `width` inside `rect`, `AbstractTextElement.java:293-298`.
/// `None` when the alignment needs the width and there is none.
fn aligned_x(rect: Rect, align: Alignment, width: Option<f32>) -> Option<f32> {
    if align.right() {
        width.map(|width| rect.right() - width)
    } else if align.centred_h() {
        width.map(|width| rect.x + (rect.w - width) / 2.0)
    } else {
        Some(rect.x)
    }
}

/// The y of a block `height` tall inside `rect`, `AbstractTextElement.java:302-309`.
fn aligned_y(rect: Rect, align: Alignment, height: f32) -> f32 {
    if align.bottom() {
        rect.bottom() - height
    } else if align.centred_v() {
        rect.y + (rect.h - height) / 2.0
    } else {
        rect.y
    }
}

fn label_ops(
    id: Id,
    rect: Rect,
    label: &Label,
    state: State,
    mouse: [f32; 2],
    metrics: &dyn Metrics,
    plan: &mut Plan,
) {
    // An empty text prepares nothing at all (`AbstractTextElement.java:198-204`), and so
    // does a scale of zero or less.
    if label.text.is_empty() || !(label.scale > 0.0) {
        return;
    }
    if label.wrap {
        plan.gaps.push(Gap::WrappedLabel {
            id,
            blocks: label.text.len(),
        });
        return;
    }
    if label.scale != 1.0 {
        plan.gaps.push(Gap::ScaledText {
            id,
            scale: label.scale,
        });
    }

    // Without wrap, `recalcPreparedTextBox` keeps the first line and drops the rest
    // (`AbstractTextElement.java:266-271`) — Java's own behaviour, not a gap.
    let line = &label.text[0];
    let width = line.plain().map(|plain| metrics.width(plain));
    let argb = color_of(
        state,
        rect.contains(mouse),
        label.color,
        label.color_hover,
        label.color_disabled,
    );

    if let Some(width) = width {
        // `maxWidth = textWidth / textScale - (textShadow ? 1 : 0)` (`:210-213`).
        let room = rect.w / label.scale
            - if label.shadow { 1.0 } else { 0.0 };
        if width > room {
            plan.gaps.push(Gap::CutLine { id, width, room });
        }
    }

    let x = match aligned_x(rect, label.align, width) {
        Some(x) => x,
        None => {
            plan.gaps.push(Gap::Unmeasurable { id });
            rect.x
        }
    };
    let y = aligned_y(rect, label.align, drawn_height(metrics, label.scale));
    plan.ops.push(Op::Text {
        line: line.clone(),
        at: [x, y],
        argb,
        shadow: label.shadow,
    });
}

fn button_ops(
    id: Id,
    rect: Rect,
    button: &Button,
    state: State,
    mouse: [f32; 2],
    metrics: &dyn Metrics,
    plan: &mut Plan,
) {
    let Some(first) = button.label.first() else {
        // A button with no label is an empty box: `widget_button` centres nothing.
        if let Some(sprite) = button.sprite {
            plan.ops.push(Op::NineSlice {
                sprite,
                rect,
                argb: 0xFFFF_FFFF,
            });
        } else {
            plan.ops.push(Op::Button {
                rect,
                label: Text::literal(""),
                active: state.enabled,
                highlighted: false,
            });
        }
        return;
    };
    if button.label.len() > 1 {
        plan.gaps.push(Gap::ButtonLines {
            id,
            dropped: button.label.len() - 1,
        });
    }

    match button.sprite {
        Some(sprite) => {
            plan.gaps.push(Gap::ButtonNarration { id, sprite });
            plan.ops.push(Op::NineSlice {
                sprite,
                rect,
                argb: 0xFFFF_FFFF,
            });
            // The label, centred as `AbstractButton` centres one
            // (`host/draw.rs:149-160` draws it on the sprite's middle).
            let width = first.plain().map(|plain| metrics.width(plain));
            let x = match aligned_x(rect, Alignment::Middle, width) {
                Some(x) => x,
                None => {
                    plan.gaps.push(Gap::Unmeasurable { id });
                    rect.x
                }
            };
            plan.ops.push(Op::Text {
                line: first.clone(),
                at: [x, aligned_y(rect, Alignment::Middle, drawn_height(metrics, 1.0))],
                argb: LABEL_DEFAULTS.color,
                shadow: LABEL_DEFAULTS.shadow,
            });
        }
        None => {
            let active = state.enabled;
            plan.ops.push(Op::Button {
                rect,
                label: first.text.clone(),
                active,
                highlighted: active && rect.contains(mouse),
            });
        }
    }
}

fn bar_ops(id: Id, rect: Rect, bar: &Bar, plan: &mut Plan) {
    plan.ops.push(Op::Fill {
        rect,
        argb: bar.track,
    });
    // `setSize((int) (progressRatio * width), height)` (`WindowResearchTree.java:581-590`).
    let filled = Rect::new(rect.x, rect.y, rect.w * bar.fraction(), rect.h);
    if !filled.is_empty() {
        plan.ops.push(Op::Fill {
            rect: filled,
            argb: bar.fill,
        });
    }
    if bar.gradient {
        plan.gaps.push(Gap::Gradient { id });
    }
}

fn list_ops(id: Id, rect: Rect, list: &List, metrics: &dyn Metrics, plan: &mut Plan) {
    let viewport = list.viewport(rect);
    if list.scrollbar {
        // The viewport is narrowed the way `ScrollingView` narrows it
        // (`views/ScrollingView.java:43`), but the thumb is a sprite, a size and a
        // drag this slice does not model.
        plan.gaps.push(Gap::Scrollbar { id });
    }
    if list.rows.is_empty() {
        // `ScrollingListContainer` swaps the rows for one text element sized to the
        // viewport (`views/ScrollingListContainer.java:179-186`), which its own
        // `MIDDLE_LEFT` default centres vertically.
        if let Some(empty) = &list.empty_text {
            plan.ops.push(Op::Text {
                line: empty.clone(),
                at: [
                    viewport.x,
                    aligned_y(viewport, LABEL_DEFAULTS.align, drawn_height(metrics, 1.0)),
                ],
                argb: LABEL_DEFAULTS.color,
                shadow: LABEL_DEFAULTS.shadow,
            });
        }
        return;
    }

    // The viewport is the clip (`views/ScrollingContainer.java:69-82`): Java scissors,
    // translates by `-scrollY`, draws, and ends the scissors.
    plan.ops.push(Op::Clip { rect: viewport });
    let scroll = list.scroll();
    for index in crate::widgets::layout::visible_rows(list, viewport.h) {
        let row = &list.rows[index];
        let top = viewport.y + index as f32 * (list.row_height + list.child_spacing) - scroll;
        plan.ops.push(Op::Text {
            line: row.line.clone(),
            at: [
                viewport.x,
                aligned_y(
                    Rect::new(viewport.x, top, viewport.w, list.row_height),
                    LABEL_DEFAULTS.align,
                    drawn_height(metrics, 1.0),
                ),
            ],
            argb: LABEL_DEFAULTS.color,
            shadow: LABEL_DEFAULTS.shadow,
        });
    }
    plan.ops.push(Op::Unclip);
}

/// Makes the calls of `plan`, in order, and answers the first refusal.
///
/// This is the **only** function in the module that talks to the drawing context: a
/// test can compare a [`Plan`] without a game, and a screen hook only has to call this.
pub fn execute(plan: &Plan, draw: &mut dyn DrawContext) -> Result<(), Call> {
    let mut open = 0usize;
    for op in &plan.ops {
        match op {
            Op::Fill { rect, argb } => draw.fill(rect.to_array(), *argb),
            Op::Text {
                line,
                at,
                argb,
                shadow,
            } => match line.plain() {
                Some(plain) => draw.text(plain, *at, *argb, *shadow),
                None => {
                    // The component route: the batch takes the `Text` itself, at the
                    // integer position the game's font draws from.
                    let batch = [GuiDraw::Text {
                        text: line.text.clone(),
                        x: at[0] as i32,
                        y: at[1] as i32,
                        color: *argb,
                        shadow: *shadow,
                    }];
                    draw.gui_paint(&batch).map_err(Call::Text)?;
                }
            },
            Op::Button {
                rect,
                label,
                active,
                highlighted,
            } => {
                draw.widget_button(rect.to_array(), label, *active, *highlighted)
                    .map_err(Call::Text)?;
            }
            Op::NineSlice {
                sprite,
                rect,
                argb,
            } => draw.gui_nine_slice(sprite, rect.to_array(), *argb),
            Op::Clip { rect } => {
                draw.push_clip(rect.to_array()).map_err(Call::Clip)?;
                open += 1;
            }
            Op::Unclip => {
                // A `pop_clip` with nothing open is a refusal, not a no-op
                // (`client/gui_clip.rs:11-13`): say so instead of swallowing it.
                draw.pop_clip().map_err(Call::Clip)?;
                open = open.saturating_sub(1);
            }
        }
    }
    debug_assert_eq!(open, 0, "a plan must close every clip it opens");
    Ok(())
}
