//! Placement: which pixels each pane occupies, and the refusal when two collide.
//!
//! Java reference: `View.adjustChild` turns a child's parent-relative
//! rectangle into the one it occupies (`/workspace/blockui-src/src/main/java/com/ldtteam/blockui/views/View.java:319-358`),
//! `View.drawSelf` draws children from the parent's origin **plus its padding**
//! (`:70-100`), and the interior a child is measured against is the box minus twice the
//! padding (`:279-287`).
//!
//! Three rules, all of them Java's, and one this port adds:
//!
//! | Rule | Where Java says it |
//! | --- | --- |
//! | a negative width is the interior minus its absolute value | `View.java:325-331` |
//! | right or centred alignment counts back from the interior's right edge | `View.java:334-341` |
//! | a negative height, and bottom or centred alignment, on the other axis | `View.java:344-357` |
//! | **two panes that cover the same pixels are refused** | *the port's own check* |
//!
//! The last one is a lint, not a BlockUI rule: BlockUI draws a `View`'s children in
//! order and a later child simply covers an earlier one (`View.java:87-97`), which a
//! screen needs (a bar's fill over its track). [`Pane::overlay`] is how a model says
//! "this overlap is intended", and [`Layout::disjoint`] refuses every other one, naming
//! both panes and the pixels they share. That refusal is worth having because a
//! translation that grows a label, or a column that moves by ten pixels, is exactly how
//! a screen ends up with two buttons on top of each other and nobody notices until a
//! player clicks the wrong one.
//!
//! **Nesting is one level.** Every widget is a child of the window, so the collision
//! check sees every widget pair. A list's rows are not widgets (see
//! [`crate::widgets::model::Row`]).

use std::fmt;
use std::ops::Range;

use crate::widgets::model::{Id, List, Pane, Rect, Screen, Widget};

/// The box a widget is placed in: a rectangle and the padding between its edge and its
/// children (`views/View.java:21`, `:279-287`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Container {
    /// The box itself, in GUI pixels.
    pub rect: Rect,
    /// `View.padding`, on all four sides.
    pub padding: f32,
}

impl Container {
    /// A container with no padding — what a pane itself is.
    pub const fn new(rect: Rect) -> Self {
        Self { rect, padding: 0.0 }
    }

    /// A container with a margin inside its edge.
    pub const fn padded(rect: Rect, padding: f32) -> Self {
        Self { rect, padding }
    }

    /// The box children are measured against: the rectangle, moved in by `padding` and
    /// shrunk by twice it (`View.getInteriorWidth`, `:279-287`).
    pub fn interior(&self) -> Rect {
        Rect::new(
            self.rect.x + self.padding,
            self.rect.y + self.padding,
            (self.rect.w - self.padding * 2.0).max(0.0),
            (self.rect.h - self.padding * 2.0).max(0.0),
        )
    }
}

/// Where one widget goes: its absolute rectangle, and the slot it came from so
/// [`crate::widgets::adapt`] can reach its body.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    /// The pane's name.
    pub id: Id,
    /// Its index in [`Screen::widgets`].
    pub slot: usize,
    /// The rectangle it occupies, in GUI pixels, absolute.
    pub rect: Rect,
    /// Whether it is allowed to cover its siblings, copied from
    /// [`Pane::overlay`] so the collision check needs nothing else.
    pub overlay: bool,
}

/// Two panes that cover the same pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Overlap {
    /// The pane drawn first.
    pub first: Id,
    /// The pane drawn over it.
    pub second: Id,
    /// The pixels they share.
    pub area: Rect,
}

/// Why a screen cannot be laid out, or cannot be laid out as asked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Refusal {
    /// Two panes that are not declared [`Pane::overlay`] cover the same pixels. The
    /// first pair in draw order is named; [`Layout::collisions`] lists every pair.
    Overlap {
        /// The pane drawn first.
        first: Id,
        /// The pane drawn over it.
        second: Id,
        /// What they share.
        area: Rect,
    },
    /// A rectangle with a NaN or an infinity in it: no drawing call can take it
    /// (`crates/horizon-mod-vocab/src/host/draw.rs:112-122`).
    NotFinite {
        /// The pane that has one.
        id: Id,
    },
    /// The same id twice. Java answers one of them and nothing says which
    /// (`Pane.findPaneByID`, `Pane.java:460-472`), so the screen is ambiguous.
    DuplicateId {
        /// The name both panes carry.
        id: Id,
    },
}

impl Refusal {
    /// The pane the refusal is about: the second of an overlap, since it is the one
    /// that would change.
    pub fn id(self) -> Id {
        match self {
            Refusal::Overlap { second, .. } => second,
            Refusal::NotFinite { id } | Refusal::DuplicateId { id } => id,
        }
    }

    /// What a caller shows or logs. No colour, no translation: the port's messages come
    /// from `assets/minecolonies/lang/` (`crate::commands::reply`), and this line is for
    /// the developer reading a test failure.
    pub fn reason(self) -> &'static str {
        match self {
            Refusal::Overlap { .. } => "two panes cover the same pixels",
            Refusal::NotFinite { .. } => "a rectangle is not a finite number",
            Refusal::DuplicateId { .. } => "two panes carry the same id",
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::Overlap {
                first,
                second,
                area,
            } => write!(
                f,
                "{} covers {} at x {} y {} ({} x {})",
                second, first, area.x, area.y, area.w, area.h
            ),
            Refusal::NotFinite { id } => write!(f, "{} has a rectangle that is not finite", id),
            Refusal::DuplicateId { id } => write!(f, "{} is used by two panes", id),
        }
    }
}

/// Where every widget of a screen sits, in draw order.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    container: Container,
    placed: Vec<Placed>,
}

impl Layout {
    /// The box the widgets were placed in.
    pub fn container(&self) -> Container {
        self.container
    }

    /// Every widget's place, in the order the screen draws them.
    pub fn placed(&self) -> &[Placed] {
        &self.placed
    }

    /// The rectangle of the widget called `id`.
    pub fn rect(&self, id: Id) -> Option<Rect> {
        self.placed.iter().find(|p| p.id == id).map(|p| p.rect)
    }

    /// Each widget with its rectangle, skipping a slot the screen no longer has (the
    /// layout is built from the screen, so this is 1:1; the pairing is written as a
    /// `filter_map` so a stale layout cannot panic).
    pub fn iter<'a>(&'a self, screen: &'a Screen) -> impl Iterator<Item = (&'a Widget, Rect)> + 'a {
        self.placed
            .iter()
            .filter_map(move |p| screen.widget_at(p.slot).map(|w| (w, p.rect)))
    }

    /// Every unintended overlap, first pair first, in draw order.
    pub fn collisions(&self) -> Vec<Overlap> {
        let mut overlaps = Vec::new();
        for (i, a) in self.placed.iter().enumerate() {
            for b in self.placed.iter().skip(i + 1) {
                if a.overlay || b.overlay {
                    continue;
                }
                if let Some(area) = a.rect.intersect(b.rect) {
                    overlaps.push(Overlap {
                        first: a.id,
                        second: b.id,
                        area,
                    });
                }
            }
        }
        overlaps
    }

    /// `Ok` when no two panes that are not [`Pane::overlay`] share a pixel, otherwise
    /// the first such pair in draw order.
    pub fn disjoint(&self) -> Result<(), Refusal> {
        self.collisions()
            .into_iter()
            .next()
            .map_or(Ok(()), |overlap| {
                Err(Refusal::Overlap {
                    first: overlap.first,
                    second: overlap.second,
                    area: overlap.area,
                })
            })
    }
}

/// Where `pane` sits inside `container`, following `View.adjustChild`
/// (`views/View.java:319-358`) and the origin `View.drawSelf` draws from
/// (`:70-100`).
///
/// The rectangle is **not** clamped to the container: BlockUI lets a child hang over its
/// parent (a label that overflows keeps its overflow) and the clipping, when a list
/// wants it, is the drawing context's (`DrawContext::push_clip`,
/// `crates/horizon-mod-vocab/src/host/draw.rs:112-130`).
pub fn resolve(container: &Container, pane: &Pane) -> Rect {
    let interior = container.interior();
    let mut w = pane.rect.w;
    let mut h = pane.rect.h;
    let mut x = pane.rect.x;
    let mut y = pane.rect.y;

    // A negative side is the interior minus its absolute value, and never less than
    // nothing: `View.java:325-331` and `:344-348`.
    if w < 0.0 {
        w = (interior.w + w).max(0.0);
    }
    if h < 0.0 {
        h = (interior.h + h).max(0.0);
    }

    // Right or centred first, then the child's own x is added on top
    // (`View.java:334-341`).
    if pane.align.right() {
        x = (interior.w - w) - x;
    } else if pane.align.centred_h() {
        x = ((interior.w - w) / 2.0) + x;
    }
    if pane.align.bottom() {
        y = (interior.h - h) - y;
    } else if pane.align.centred_v() {
        y = ((interior.h - h) / 2.0) + y;
    }

    Rect::new(interior.x + x, interior.y + y, w, h)
}

/// Places every widget of `screen` inside its box, in draw order.
///
/// Refuses a rectangle that is not finite and an id used twice; says nothing about
/// overlaps, which [`Layout::disjoint`] or [`check`] answer.
pub fn place(screen: &Screen) -> Result<Layout, Refusal> {
    let container = Container::padded(screen.area, screen.padding);
    let mut placed: Vec<Placed> = Vec::with_capacity(screen.len());
    for (slot, widget) in screen.widgets().iter().enumerate() {
        let rect = resolve(&container, &widget.pane);
        if !rect.is_finite() {
            return Err(Refusal::NotFinite { id: widget.id() });
        }
        if placed.iter().any(|p| p.id == widget.id()) {
            return Err(Refusal::DuplicateId { id: widget.id() });
        }
        placed.push(Placed {
            id: widget.id(),
            slot,
            rect,
            overlay: widget.pane.overlay,
        });
    }
    Ok(Layout { container, placed })
}

/// Places the screen and refuses any unintended overlap: the one call a drawing path
/// wants before it builds a plan.
pub fn check(screen: &Screen) -> Result<Layout, Refusal> {
    let layout = place(screen)?;
    layout.disjoint()?;
    Ok(layout)
}

/// The rows a viewport `height` tall shows, for a list scrolled where it is.
///
/// Java creates a row when `currentYpos + elementHeight >= scrollY` **and**
/// `currentYpos <= scrollY + height` (`views/ScrollingListContainer.java:142`), so the
/// last row is drawn as soon as it touches the bottom edge and the first for as long as
/// it touches the top. Both comparisons are inclusive and the rows are contiguous, so
/// the answer is one range.
pub fn visible_rows(list: &List, height: f32) -> Range<usize> {
    let scroll = list.scroll();
    let mut first = None;
    let mut last = 0usize;
    let mut y = 0.0f32;
    for index in 0..list.rows.len() {
        if y + list.row_height >= scroll && y <= scroll + height {
            first.get_or_insert(index);
            last = index;
        }
        y += list.row_height + list.child_spacing;
    }
    match first {
        Some(first) => first..last + 1,
        None => 0..0,
    }
}
