//! The widget model: panes, their state, and the four widgets this slice knows.
//!
//! Java reference: BlockUI's `Pane` (`/workspace/blockui-src/src/main/java/com/ldtteam/blockui/Pane.java:36-52`),
//! its `Alignment` (`Alignment.java:6-21`), `AbstractTextElement`
//! (`controls/AbstractTextElement.java:44-105`), `Button` (`controls/Button.java:20-120`),
//! `Gradient` (`controls/Gradient.java:9-60`), `ScrollingList`
//! (`views/ScrollingList.java:20-60`) and `ScrollingContainer`
//! (`views/ScrollingContainer.java:13-175`).
//!
//! **Nothing here knows a font, a screen or a hook.** A [`Screen`] is a list of
//! widgets in draw order — `View.drawSelf` draws its children in that order
//! (`views/View.java:70-100`) — each with a rectangle relative to its parent until
//! [`crate::widgets::layout`] resolves it, and a [`State`] copied from
//! `Pane.visible` / `Pane.enabled` (`Pane.java:41-42`).
//!
//! **What is deliberately absent.** BlockUI's pane tree (a `Pane` knows its parent and
//! `findPaneOfTypeByID` walks it depth first, `Pane.java:458-478`): this slice is one
//! level, because a list's rows are the only nesting MineColonies needs for its 94
//! screens and a row is modelled as one line ([`Row`]). Adding children later is a
//! `Vec<Widget>` on [`Widget`], not a redesign of what is here.
//!
//! **`View.setSize`'s zero-fills-the-parent rule is not modelled.** A `View` child of
//! width 0 is widened to the parent's width minus its x when the window is parsed
//! (`views/View.java:198-215`); a plain `Pane` keeps its 0. A model cannot tell the two
//! apart without a per-widget flag, so a screen that relies on it must give the size
//! explicitly. The XML's own idiom is the negative side, which *is* modelled.

use std::fmt;

use mod_api::Text;

/// A rectangle in GUI pixels, `[x, y, width, height]` — the shape every drawing call
/// takes (`crates/horizon-mod-vocab/src/host/draw.rs:43`, `:122`, `:136`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    /// Left edge, GUI pixels.
    pub x: f32,
    /// Top edge, GUI pixels.
    pub y: f32,
    /// Width; negative means "the parent's interior minus its absolute value"
    /// (`views/View.java:325-331`), and is resolved by
    /// [`crate::widgets::layout::resolve`].
    pub w: f32,
    /// Height, with the same negative idiom (`views/View.java:344-348`).
    pub h: f32,
}

impl Rect {
    /// A rectangle at the origin, empty.
    pub const ZERO: Rect = Rect {
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
    };

    /// `[x, y, width, height]`.
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    /// The x just past the right edge.
    pub fn right(self) -> f32 {
        self.x + self.w
    }

    /// The y just past the bottom edge.
    pub fn bottom(self) -> f32 {
        self.y + self.h
    }

    /// No pixels at all: a zero or negative side.
    pub fn is_empty(self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }

    /// Every coordinate and side is a finite number. A NaN reaching a drawing call is
    /// a rectangle `fill` and `push_clip` refuse (`host/draw.rs:112-122`), so the
    /// refusal belongs here, before the plan.
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.w.is_finite() && self.h.is_finite()
    }

    /// `Pane.isPointInPane` (`Pane.java:431-434`): the left and top edges are in, the
    /// right and bottom edges are out, so two panes side by side share no point.
    pub fn contains(self, at: [f32; 2]) -> bool {
        at[0] >= self.x && at[0] < self.right() && at[1] >= self.y && at[1] < self.bottom()
    }

    /// The pixels two rectangles share, or `None` when they only touch: the same
    /// half-open rule as [`Rect::contains`].
    pub fn intersect(self, other: Rect) -> Option<Rect> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        (right > x && bottom > y).then(|| Rect::new(x, y, right - x, bottom - y))
    }

    /// The same size, moved by `dx, dy`.
    pub fn shift(self, dx: f32, dy: f32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.w, self.h)
    }

    /// The `[x, y, width, height]` a drawing call takes.
    pub fn to_array(self) -> [f32; 4] {
        [self.x, self.y, self.w, self.h]
    }
}

/// BlockUI's nine alignments (`Alignment.java:6-21`), all of them: they say how a
/// child sits inside its parent (a `View` repositions it, `views/View.java:334-357`)
/// and, for text, how a label sits inside its own rectangle
/// (`controls/AbstractTextElement.java:293-309`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Alignment {
    /// Top left; the default for a container's child.
    #[default]
    TopLeft,
    /// Top edge, horizontally centred.
    TopMiddle,
    /// Top edge, against the right edge.
    TopRight,
    /// Vertically centred, against the left edge.
    MiddleLeft,
    /// Centred on both axes.
    Middle,
    /// Vertically centred, against the right edge.
    MiddleRight,
    /// Bottom edge, against the left edge.
    BottomLeft,
    /// Bottom edge, horizontally centred.
    BottomMiddle,
    /// Bottom right.
    BottomRight,
}

impl Alignment {
    /// The nine, in the order Java declares them (`Alignment.java:13-21`).
    pub const ALL: [Alignment; 9] = [
        Alignment::TopLeft,
        Alignment::TopMiddle,
        Alignment::TopRight,
        Alignment::MiddleLeft,
        Alignment::Middle,
        Alignment::MiddleRight,
        Alignment::BottomLeft,
        Alignment::BottomMiddle,
        Alignment::BottomRight,
    ];

    /// The XML word Java parses (`Alignment.java:13-21`, read by
    /// `PaneParams.getEnum`). One string per variant, and `from_name` is its inverse.
    pub fn name(self) -> &'static str {
        match self {
            Alignment::TopLeft => "top left",
            Alignment::TopMiddle => "top horizontal",
            Alignment::TopRight => "top right",
            Alignment::MiddleLeft => "vertical left",
            Alignment::Middle => "vertical horizontal",
            Alignment::MiddleRight => "vertical right",
            Alignment::BottomLeft => "bottom left",
            Alignment::BottomMiddle => "bottom horizontal",
            Alignment::BottomRight => "bottom right",
        }
    }

    /// The alignment a name picks, or `None` for a word BlockUI does not know.
    pub fn from_name(name: &str) -> Option<Alignment> {
        Alignment::ALL.into_iter().find(|a| a.name() == name)
    }

    /// `Alignment.isRightAligned` (`Alignment.java:36-39`): the child's right edge
    /// meets the interior's right edge.
    pub fn right(self) -> bool {
        matches!(
            self,
            Alignment::TopRight | Alignment::MiddleRight | Alignment::BottomRight
        )
    }

    /// `Alignment.isBottomAligned` (`Alignment.java:41-44`).
    pub fn bottom(self) -> bool {
        matches!(
            self,
            Alignment::BottomLeft | Alignment::BottomMiddle | Alignment::BottomRight
        )
    }

    /// `Alignment.isHorizontalCentered` (`Alignment.java:46-49`).
    pub fn centred_h(self) -> bool {
        matches!(
            self,
            Alignment::TopMiddle | Alignment::Middle | Alignment::BottomMiddle
        )
    }

    /// `Alignment.isVerticalCentered` (`Alignment.java:51-54`).
    pub fn centred_v(self) -> bool {
        matches!(
            self,
            Alignment::MiddleLeft | Alignment::Middle | Alignment::MiddleRight
        )
    }
}

/// `Pane.visible` and `Pane.enabled` (`Pane.java:41-42`): hidden draws nothing,
/// disabled draws greyed and takes no click (`Pane.java:655-661`,
/// `controls/AbstractTextElement.java:288`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct State {
    /// `Pane.visible`; `Pane.shouldDraw` is this (`Pane.java:205-210`), the hover
    /// exception aside.
    pub visible: bool,
    /// `Pane.enabled`.
    pub enabled: bool,
}

impl State {
    /// Drawn and clickable.
    pub const SHOWN: State = State {
        visible: true,
        enabled: true,
    };
    /// Drawn, greyed, inert.
    pub const DISABLED: State = State {
        visible: true,
        enabled: false,
    };
    /// Neither drawn nor clickable.
    pub const HIDDEN: State = State {
        visible: false,
        enabled: false,
    };

    /// This state, hidden.
    pub fn hidden(self) -> State {
        State {
            visible: false,
            ..self
        }
    }

    /// This state, disabled.
    pub fn disabled(self) -> State {
        State {
            enabled: false,
            ..self
        }
    }
}

impl Default for State {
    fn default() -> Self {
        State::SHOWN
    }
}

/// A pane's name. BlockUI keys panes by string (`Pane.setID`, `Pane.java:150-153`) and
/// MineColonies finds them by it (`findPaneOfTypeByID`, `Pane.java:460-472`).
///
/// `&'static str` because every id in the port comes from an XML file the build
/// embeds; an id computed at run time would need an owned string and a
/// [`std::borrow::Cow`], which nothing here asks for yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(&'static str);

impl Id {
    /// An id from a static name.
    pub const fn new(name: &'static str) -> Self {
        Id(name)
    }

    /// The name, as the XML wrote it.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// One paragraph of a label. `AbstractTextElement.text` is a *list* of components,
/// each drawn below the previous (`controls/AbstractTextElement.java:95`,
/// `:331-369`), so a label holds a `Vec<Line>` and one line is one component.
///
/// `plain` is the same characters when the line *is* one literal string, `None` when it
/// is a component (a translation key, a style, an inline object). The two are not the
/// same fact:
///
/// - the SDK exports `Text` and nothing to read it back — `TextContent` is not
///   re-exported (`crates/horizon-mod-vocab/generated/mod_api_exports_root.rs:265-266`,
///   `pub use text::{Text, TextHover}`), so a mod cannot turn a component into its
///   characters;
/// - width is measured on a `&str` only (`DrawContext::text_width`,
///   `crates/horizon-mod-vocab/src/host/draw.rs:86-89`).
///
/// Without `plain`, a line can be drawn but not measured, and measuring is what a
/// centred or right-aligned label needs. Keeping the characters a label was built from
/// is therefore not duplication: it is the only measurable form the API offers, and
/// [`crate::widgets::adapt::Gap::Unmeasurable`] names the case where it is missing.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    /// The component, handed to the drawing call unchanged.
    pub text: Text,
    /// The same characters, when the line is one literal string.
    pub plain: Option<String>,
}

impl Line {
    /// A line of plain characters: measurable, and drawn by `DrawContext::text`.
    pub fn literal(value: impl Into<String>) -> Self {
        let value: String = value.into();
        Self {
            text: Text::literal(value.clone()),
            plain: Some(value),
        }
    }

    /// A line that is already a component: drawn through `gui_paint`, never measured.
    pub fn component(text: Text) -> Self {
        Self { text, plain: None }
    }

    /// The literal characters, when there are some.
    pub fn plain(&self) -> Option<&str> {
        self.plain.as_deref()
    }
}

/// The defaults a `<text>` without attributes takes. Each value is Java's own, and
/// `source` is where it is written, so a later build that changes one changes the
/// citation with it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LabelDefaults {
    /// `DEFAULT_TEXT_ALIGNMENT = Alignment.MIDDLE_LEFT`
    /// (`controls/AbstractTextElement.java:45`) — left, and vertically centred.
    pub align: Alignment,
    /// `DEFAULT_TEXT_COLOR = 0xffffff` (`:46`), read as **opaque** white here:
    /// Java's literal is RGB and the game supplies the alpha, while
    /// `DrawContext::text` takes `0xAARRGGBB` and names `0xFFFFFFFF` opaque white
    /// (`crates/horizon-mod-vocab/src/host/draw.rs:59-61`).
    pub color: u32,
    /// `DEFAULT_TEXT_SHADOW = false` (`:47`).
    pub shadow: bool,
    /// `DEFAULT_TEXT_WRAP = false` (`:48`).
    pub wrap: bool,
    /// `DEFAULT_TEXT_LINESPACE = 0` (`:49`), pixels between two drawn lines.
    pub line_spacing: f32,
    /// `DEFAULT_TEXT_SCALE = 1.0` (`:44`).
    pub scale: f32,
    /// Where the six values above are written.
    pub source: &'static str,
}

/// The defaults of [`Label`], with their citation.
pub const LABEL_DEFAULTS: LabelDefaults = LabelDefaults {
    align: Alignment::MiddleLeft,
    color: 0xFFFF_FFFF,
    shadow: false,
    wrap: false,
    line_spacing: 0.0,
    scale: 1.0,
    source: "controls/AbstractTextElement.java:44-49",
};

/// A text element (`controls/Text.java:10-28`, the class `AbstractTextElement`
/// gives its fields to).
///
/// **Without `wrap`, one line is drawn.** `recalcPreparedTextBox` keeps
/// `preparedText.subList(0, 1)` (`controls/AbstractTextElement.java:266-271`), so a
/// label of three blocks set to `wrap = false` shows the first one and drops two.
/// With `wrap = true` all the lines that fit the height are drawn, and the rest are
/// cut (`:232-241`).
///
/// **One colour per state, as Java has three.** `AbstractTextElement` keeps
/// `textColor`, `textHoverColor` and `textDisabledColor` and picks by
/// `enabled` and `wasCursorInPane` (`:288`); `None` here means "the same as
/// [`Label::color`]", which is what a `<text color="…">` sets for all three
/// (`:172-178`).
#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    /// The paragraphs, top to bottom.
    pub text: Vec<Line>,
    /// How the block sits in the pane (`textalign`), and, inside the block, how each
    /// line sits against the widest one (`AbstractTextElement.java:293-309`, `:352-366`).
    pub align: Alignment,
    /// ARGB, the enabled and hovered colour.
    pub color: u32,
    /// ARGB when the pointer is in the pane; `None` keeps [`Label::color`].
    pub color_hover: Option<u32>,
    /// ARGB when the pane is disabled; `None` keeps [`Label::color`].
    pub color_disabled: Option<u32>,
    /// `AbstractTextElement.textShadow`.
    pub shadow: bool,
    /// `AbstractTextElement.textWrap`.
    pub wrap: bool,
    /// Pixels added after each drawn line (`textLinespace`).
    pub line_spacing: f32,
    /// `textscale`: 1.0 draws at the font's own size. The drawing calls take no
    /// scale (`host/draw.rs:61`), so anything else is a named gap in
    /// [`crate::widgets::adapt`].
    pub scale: f32,
}

impl Label {
    /// A label of `lines` with [`LABEL_DEFAULTS`].
    pub fn new(lines: Vec<Line>) -> Self {
        Self {
            text: lines,
            align: LABEL_DEFAULTS.align,
            color: LABEL_DEFAULTS.color,
            color_hover: None,
            color_disabled: None,
            shadow: LABEL_DEFAULTS.shadow,
            wrap: LABEL_DEFAULTS.wrap,
            line_spacing: LABEL_DEFAULTS.line_spacing,
            scale: LABEL_DEFAULTS.scale,
        }
    }

    /// A one-line label.
    pub fn line(line: Line) -> Self {
        Self::new(vec![line])
    }

    /// The same label, aligned differently inside its pane.
    pub fn with_align(mut self, align: Alignment) -> Self {
        self.align = align;
        self
    }

    /// The same label, in another colour.
    pub fn with_color(mut self, argb: u32) -> Self {
        self.color = argb;
        self
    }

    /// The same label, with the colour a disabled pane draws it in.
    pub fn with_disabled_color(mut self, argb: u32) -> Self {
        self.color_disabled = Some(argb);
        self
    }
}

/// A button (`controls/Button.java:20-120`): its label is a text element like any
/// other, and the game draws the box.
#[derive(Clone, Debug, PartialEq)]
pub struct Button {
    /// The label's paragraphs; `widget_button` draws the first as one centred line
    /// (`crates/horizon-mod-vocab/src/host/draw.rs:149-163`).
    pub label: Vec<Line>,
    /// The sprite a `ButtonImage` is drawn on (`source=` in XML): 226 of
    /// MineColonies' 271 buttons carry one. `None` is the game's own
    /// `widget/button` sprite.
    pub sprite: Option<&'static str>,
}

impl Button {
    /// A button on the game's own sprite.
    pub fn new(label: Vec<Line>) -> Self {
        Self {
            label,
            sprite: None,
        }
    }

    /// A one-line button.
    pub fn text(label: impl Into<String>) -> Self {
        Self::new(vec![Line::literal(label)])
    }

    /// The same button, drawn on a sprite of the mod's.
    pub fn with_sprite(mut self, sprite: &'static str) -> Self {
        self.sprite = Some(sprite);
        self
    }
}

/// A progress bar, as MineColonies draws one: a `Gradient` sized to the fraction,
/// over the `Image` that frames it (`core/client/gui/WindowResearchTree.java:569-590`
/// — `progressRatio` at `:577`, `setSize((int) (ratio * width))` at `:581-590`).
///
/// The fraction is private because it has an invariant: Java multiplies a width by it,
/// so a NaN or a value past 1 would ask for a rectangle that is not one. A non-finite
/// ratio is 0 — Java's `(int)` cast of NaN is 0 too — and the rest is clamped.
#[derive(Clone, Debug, PartialEq)]
pub struct Bar {
    fraction: f32,
    /// ARGB of the whole bar, drawn first (`Image` behind the `Gradient`). The port's
    /// own grey: BlockUI has no track/fill pair to copy, a `Gradient` over an `Image`
    /// is the widget.
    pub track: u32,
    /// ARGB of the filled part. The port's own green, for the same reason.
    pub fill: u32,
    /// The fill is a `Gradient` and not a flat colour: Java's class fades between
    /// `gradientstart` and `gradientend` (`controls/Gradient.java:13-18`), and the
    /// drawing calls here are flat (`host/draw.rs:42-45`). A named gap in
    /// [`crate::widgets::adapt`].
    pub gradient: bool,
}

impl Bar {
    /// A flat bar, filled to `fraction` of its width.
    pub fn new(fraction: f32) -> Self {
        Self {
            fraction: sanitise(fraction),
            track: 0x8000_0000,
            fill: 0xFF20_8020,
            gradient: false,
        }
    }

    /// How much of the bar is filled, always in `[0, 1]`.
    pub fn fraction(&self) -> f32 {
        self.fraction
    }

    /// Fill it further or less far; a non-finite value reads as 0.
    pub fn set_fraction(&mut self, fraction: f32) {
        self.fraction = sanitise(fraction);
    }

    /// The same bar, its fill fading between two colours.
    pub fn with_gradient(mut self) -> Self {
        self.gradient = true;
        self
    }
}

/// A ratio a width can be multiplied by: NaN and the infinities are 0, the rest is
/// clamped to `[0, 1]`.
fn sanitise(fraction: f32) -> f32 {
    if fraction.is_finite() {
        fraction.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// One row of a list, as `IPaneUpdater.updateElement` fills one
/// (`views/ScrollingList.java:189-250`).
///
/// **One line, not a box.** The rows of MineColonies' biggest list are a `Box` holding
/// an `ItemIcon`, two `Text` and a `Button`
/// (`assets/minecolonies/gui/windowhutallinventory.xml:11-20`), which needs a pane tree
/// this slice does not have (see the module documentation). A row here is the text a
/// row shows; an icon or a button inside one is the next slice.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// What the row says.
    pub line: Line,
}

impl Row {
    /// A row of one line.
    pub fn new(line: Line) -> Self {
        Self { line }
    }

    /// A row of plain characters.
    pub fn text(value: impl Into<String>) -> Self {
        Self::new(Line::literal(value))
    }
}

/// A scrolling list (`views/ScrollingList.java:20-60` over
/// `views/ScrollingContainer.java`).
///
/// **The arithmetic is Java's, and it is the part worth porting.** Rows are placed at
/// `currentYpos`, then `currentYpos += elementHeight + childSpacing`
/// (`views/ScrollingListContainer.java:136-170`); the content is
/// `currentYpos - childSpacing` (`:187`); the furthest scroll is what does not fit
/// (`views/ScrollingContainer.java:64-67`); a scroll request is clamped between 0 and
/// that (`:112-127`); a page is 90 % of the height (`:132-135`).
///
/// **`childSpacing` may be negative** — one MineColonies list sets `childspacing="-1"`
/// so its rows overlap by a pixel — and the arithmetic above allows it; the rows are
/// not panes of the model, so the overlap check of
/// [`crate::widgets::layout`] does not see them.
///
/// **The scrollbar is not drawn.** `ScrollingView` reserves its width
/// (`views/ScrollingView.java:12`, `:43`) and this model narrows the viewport the same
/// way when [`List::scrollbar`] is set, but the thumb's geometry and its drag belong to
/// the input slice.
#[derive(Clone, Debug, PartialEq)]
pub struct List {
    /// The rows, in order.
    pub rows: Vec<Row>,
    /// How tall one row is.
    pub row_height: f32,
    /// Pixels between two rows (`childspacing`, `views/ScrollingList.java:22`, `:45`);
    /// negative is legal.
    pub child_spacing: f32,
    /// Shown instead of the rows when there are none (`emptytext`,
    /// `views/ScrollingList.java:52-56`).
    pub empty_text: Option<Line>,
    /// Reserve [`List::SCROLLBAR_WIDTH`] on the right (`views/ScrollingView.java:43`).
    pub scrollbar: bool,
    scroll: f32,
}

impl List {
    /// The width `ScrollingView` takes for its scrollbar
    /// (`views/ScrollingView.java:12`).
    pub const SCROLLBAR_WIDTH: f32 = 8.0;

    /// An empty list of rows `row_height` tall, with no space between them.
    pub fn new(row_height: f32) -> Self {
        Self {
            rows: Vec::new(),
            row_height,
            child_spacing: 0.0,
            empty_text: None,
            scrollbar: false,
            scroll: 0.0,
        }
    }

    /// How far it is scrolled, in pixels from the top of the content.
    pub fn scroll(&self) -> f32 {
        self.scroll
    }

    /// The height of everything the rows take:
    /// `rows * row_height + (rows - 1) * child_spacing`
    /// (`views/ScrollingListContainer.java:170`, `:187`), and 0 for an empty list
    /// (which takes the `setContentHeight` of the empty branch, `:179-186`).
    pub fn content_height(&self) -> f32 {
        let rows = self.rows.len() as f32;
        if rows == 0.0 {
            0.0
        } else {
            rows * self.row_height + (rows - 1.0) * self.child_spacing
        }
    }

    /// The furthest it may scroll in a viewport `height` tall:
    /// `max(0, content - height)` (`views/ScrollingContainer.java:64-67`).
    pub fn max_scroll(&self, height: f32) -> f32 {
        (self.content_height() - height).max(0.0)
    }

    /// Scroll to `scroll`, clamped to `[0, max_scroll]`; answers whether it moved
    /// (`views/ScrollingContainer.java:112-127`).
    pub fn set_scroll(&mut self, scroll: f32, height: f32) -> bool {
        let before = self.scroll;
        self.scroll = if scroll.is_finite() {
            scroll.clamp(0.0, self.max_scroll(height))
        } else {
            0.0
        };
        self.scroll != before
    }

    /// Scroll by `delta` pixels, clamped the same way
    /// (`views/ScrollingContainer.java:135-144`).
    pub fn scroll_by(&mut self, delta: f32, height: f32) -> bool {
        self.set_scroll(self.scroll + delta, height)
    }

    /// One wheel page: 90 % of the viewport (`views/ScrollingContainer.java:132-135`).
    pub fn page_height(height: f32) -> f32 {
        height * 0.9
    }

    /// The width the rows are drawn in: the list's width, less the scrollbar's
    /// (`views/ScrollingView.java:43`).
    pub fn viewport(&self, rect: Rect) -> Rect {
        if self.scrollbar {
            Rect::new(rect.x, rect.y, (rect.w - List::SCROLLBAR_WIDTH).max(0.0), rect.h)
        } else {
            rect
        }
    }
}

/// What a widget is: the four elements this slice draws.
#[derive(Clone, Debug, PartialEq)]
pub enum Body {
    /// A text element.
    Label(Label),
    /// A button.
    Button(Button),
    /// A progress bar.
    Bar(Bar),
    /// A scrolling list.
    List(List),
}

/// A pane: what every BlockUI element is before it is a kind
/// (`Pane.java:36-52`, `:150-197`).
#[derive(Clone, Debug, PartialEq)]
pub struct Pane {
    /// Its name, which is how a screen finds it again.
    pub id: Id,
    /// Its rectangle, **relative to its parent's interior** until
    /// [`crate::widgets::layout`] resolves it (`views/View.java:319-358`).
    pub rect: Rect,
    /// How it sits in its parent (`Pane.alignment`, `Pane.java:185-197`).
    pub align: Alignment,
    /// Visible and enabled (`Pane.java:41-42`).
    pub state: State,
    /// Drawn over its siblings on purpose: a bar's fill over its track, a label over a
    /// box. BlockUI has no such flag — it draws children in order and lets them overlap
    /// (`views/View.java:87-97`) — so this says only which overlaps are intended, and
    /// is what makes [`crate::widgets::layout::Layout::disjoint`] refuse the others.
    pub overlay: bool,
}

impl Pane {
    /// A visible, enabled pane with no alignment offset.
    pub fn new(id: Id, rect: Rect) -> Self {
        Self {
            id,
            rect,
            align: Alignment::TopLeft,
            state: State::SHOWN,
            overlay: false,
        }
    }

    /// The same pane, with another rectangle.
    pub fn with_rect(mut self, rect: Rect) -> Self {
        self.rect = rect;
        self
    }

    /// The same pane, aligned differently in its parent.
    pub fn with_align(mut self, align: Alignment) -> Self {
        self.align = align;
        self
    }

    /// The same pane, in another state.
    pub fn with_state(mut self, state: State) -> Self {
        self.state = state;
        self
    }

    /// The same pane, hidden.
    pub fn hidden(mut self) -> Self {
        self.state = self.state.hidden();
        self
    }

    /// The same pane, disabled.
    pub fn disabled(mut self) -> Self {
        self.state = self.state.disabled();
        self
    }

    /// The same pane, allowed to overlap.
    pub fn overlay(mut self) -> Self {
        self.overlay = true;
        self
    }
}

/// A pane and what it draws.
#[derive(Clone, Debug, PartialEq)]
pub struct Widget {
    /// Where it is and in what state.
    pub pane: Pane,
    /// What it is.
    pub body: Body,
}

impl Widget {
    /// A widget of any kind.
    pub fn new(pane: Pane, body: Body) -> Self {
        Self { pane, body }
    }

    /// A label.
    pub fn label(pane: Pane, label: Label) -> Self {
        Self::new(pane, Body::Label(label))
    }

    /// A button.
    pub fn button(pane: Pane, button: Button) -> Self {
        Self::new(pane, Body::Button(button))
    }

    /// A progress bar.
    pub fn bar(pane: Pane, bar: Bar) -> Self {
        Self::new(pane, Body::Bar(bar))
    }

    /// A scrolling list.
    pub fn list(pane: Pane, list: List) -> Self {
        Self::new(pane, Body::List(list))
    }

    /// The pane this widget belongs to.
    pub fn id(&self) -> Id {
        self.pane.id
    }
}

/// A window: the widgets it holds, in draw order, and the box they are laid out in
/// (`BOWindow`, drawn as any `View` is — `views/View.java:70-100`).
#[derive(Clone, Debug, PartialEq)]
pub struct Screen {
    /// The window's own id.
    pub id: Id,
    /// Its box in GUI pixels, as `screen.rect` gives it to a screen hook
    /// (`crates/horizon-mod-vocab/src/hooks/screen_render_post.rs:4-9`).
    pub area: Rect,
    /// `View.padding` (`views/View.java:21`, `:46`): the margin between the window's
    /// edge and its children, on all four sides (`:279-287`).
    pub padding: f32,
    widgets: Vec<Widget>,
}

impl Screen {
    /// An empty window.
    pub fn new(id: Id, area: Rect) -> Self {
        Self {
            id,
            area,
            padding: 0.0,
            widgets: Vec::new(),
        }
    }

    /// The same window, with a margin inside its edge.
    pub fn with_padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    /// Adds `widget` after the others: later means drawn over earlier
    /// (`views/View.java:87-97`).
    pub fn push(&mut self, widget: Widget) {
        self.widgets.push(widget);
    }

    /// Every widget, in draw order.
    pub fn widgets(&self) -> &[Widget] {
        &self.widgets
    }

    /// The widget at `slot`, which is what a [`crate::widgets::layout::Placed`] holds.
    pub fn widget_at(&self, slot: usize) -> Option<&Widget> {
        self.widgets.get(slot)
    }

    /// The widget called `id`, which is what `findPaneOfTypeByID` answers
    /// (`Pane.java:460-472`) — the first one in draw order, so a duplicate is
    /// ambiguous and [`crate::widgets::layout::place`] refuses it.
    pub fn get(&self, id: Id) -> Option<&Widget> {
        self.widgets.iter().find(|w| w.pane.id == id)
    }

    /// The widget called `id`, to change it.
    pub fn get_mut(&mut self, id: Id) -> Option<&mut Widget> {
        self.widgets.iter_mut().find(|w| w.pane.id == id)
    }

    /// How many widgets it holds.
    pub fn len(&self) -> usize {
        self.widgets.len()
    }

    /// Whether it holds none.
    pub fn is_empty(&self) -> bool {
        self.widgets.is_empty()
    }
}
