//! Unit tests on the host target: no game, no font file, no hook.
//!
//! The model is data and the plan is a list of values, so almost everything here
//! compares values; only [`execute`] needs a drawing context, and it gets a recorder
//! that implements the five calls a plan makes — the pattern the SDK itself uses
//! (`crates/horizon-mod-vocab/src/client/widgets_tests.rs:36-64`).
//!
//! The fake font is 6 pixels a character and 9 pixels a line — the game's own line
//! height, so the `lineHeight - 1` of `AbstractTextElement.java:270` is visible in the
//! expected positions.

use mod_api::{DrawContext, GuiClipRefusal, GuiDraw, Text, TextFormError};

use crate::widgets::adapt::{Call, Gap, Metrics, Op, Plan, execute, plan};
use crate::widgets::layout::{Container, Refusal, check, place, resolve, visible_rows};
use crate::widgets::model::{
    Alignment, Bar, Button, Id, LABEL_DEFAULTS, Label, Line, List, Pane, Rect, Row, Screen, State,
    Widget,
};

/// A font where every printable character is 6 pixels wide.
struct Font;

impl Metrics for Font {
    fn line_height(&self) -> f32 {
        9.0
    }

    fn width(&self, text: &str) -> f32 {
        6.0 * text.chars().count() as f32
    }
}

/// Whether two GUI positions are the same, to a thousandth of a pixel.
fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

/// A 200 by 150 window with no padding.
fn window() -> Screen {
    Screen::new(Id::new("window"), Rect::new(0.0, 0.0, 200.0, 150.0))
}

/// A label at `rect`, laid out with [`Font`].
fn label_at(id: &'static str, rect: Rect, text: &str) -> Widget {
    Widget::label(
        Pane::new(Id::new(id), rect),
        Label::line(Line::literal(text)),
    )
}

/// The line, position and colour of the `index`th text op.
fn text_at(plan: &Plan, index: usize) -> (&Line, [f32; 2], u32) {
    match &plan.ops[index] {
        Op::Text {
            line, at, argb, ..
        } => (line, *at, *argb),
        other => panic!("op {index} is {other:?}, not a text"),
    }
}

/// The rectangle and colour of the `index`th fill op.
fn fill_at(plan: &Plan, index: usize) -> (Rect, u32) {
    match &plan.ops[index] {
        Op::Fill { rect, argb } => (*rect, *argb),
        other => panic!("op {index} is {other:?}, not a fill"),
    }
}

/// A list of `rows` rows of 18 pixels, 2 pixels apart.
fn list_of(rows: usize) -> List {
    let mut list = List::new(18.0);
    list.child_spacing = 2.0;
    for index in 0..rows {
        list.rows.push(Row::text(format!("row {index}")));
    }
    list
}

/// A screen with one list, laid out and planned.
fn list_plan(list: List, rect: Rect) -> Plan {
    let mut screen = window();
    screen.push(Widget::list(Pane::new(Id::new("rows"), rect), list));
    let layout = check(&screen).expect("one list collides with nothing");
    plan(&screen, &layout, &Font, [0.0, 0.0])
}

// ---------------------------------------------------------------- the model

#[test]
fn a_rect_tells_where_its_edges_are() {
    let rect = Rect::new(10.0, 20.0, 30.0, 40.0);
    assert_eq!(rect.right(), 40.0);
    assert_eq!(rect.bottom(), 60.0);
    assert_eq!(rect.to_array(), [10.0, 20.0, 30.0, 40.0]);
    assert_eq!(rect.shift(2.0, 3.0), Rect::new(12.0, 23.0, 30.0, 40.0));
}

#[test]
fn a_point_on_the_far_edge_is_outside_as_java_has_it() {
    // `Pane.isPointInPane`, `Pane.java:431-434`: `mx >= x && mx < x + width`.
    let rect = Rect::new(10.0, 20.0, 30.0, 40.0);
    assert!(rect.contains([10.0, 20.0]));
    assert!(rect.contains([39.9, 59.9]));
    assert!(!rect.contains([40.0, 30.0]));
    assert!(!rect.contains([20.0, 60.0]));
}

#[test]
fn an_empty_rect_contains_nothing() {
    let rect = Rect::new(0.0, 0.0, 0.0, 10.0);
    assert!(rect.is_empty());
    assert!(!rect.contains([0.0, 5.0]));
    assert!(Rect::new(0.0, 0.0, 0.5, 0.5).intersect(Rect::ZERO).is_none());
}

#[test]
fn intersect_answers_the_shared_pixels_and_nothing_when_they_touch() {
    let a = Rect::new(0.0, 0.0, 10.0, 10.0);
    assert_eq!(a.intersect(Rect::new(5.0, 5.0, 10.0, 10.0)), Some(Rect::new(5.0, 5.0, 5.0, 5.0)));
    // Right edge at 10, the other's left edge at 10: no pixel is in both.
    assert_eq!(a.intersect(Rect::new(10.0, 0.0, 5.0, 5.0)), None);
}

#[test]
fn alignment_flags_follow_the_words_java_reads() {
    // `Alignment.java:28-34`: the four flags are read off the attribute string.
    let expected = [
        (Alignment::TopLeft, false, false, false, false),
        (Alignment::TopMiddle, false, false, true, false),
        (Alignment::TopRight, true, false, false, false),
        (Alignment::MiddleLeft, false, false, false, true),
        (Alignment::Middle, false, false, true, true),
        (Alignment::MiddleRight, true, false, false, true),
        (Alignment::BottomLeft, false, true, false, false),
        (Alignment::BottomMiddle, false, true, true, false),
        (Alignment::BottomRight, true, true, false, false),
    ];
    for (align, right, bottom, centred_h, centred_v) in expected {
        assert_eq!(align.right(), right, "{} right", align.name());
        assert_eq!(align.bottom(), bottom, "{} bottom", align.name());
        assert_eq!(align.centred_h(), centred_h, "{} centred h", align.name());
        assert_eq!(align.centred_v(), centred_v, "{} centred v", align.name());
    }
    assert_eq!(expected.len(), Alignment::ALL.len());
}

#[test]
fn alignment_names_round_trip() {
    // The nine words of `Alignment.java:13-21`.
    for align in Alignment::ALL {
        assert_eq!(Alignment::from_name(align.name()), Some(align));
    }
    assert_eq!(Alignment::from_name("top left"), Some(Alignment::TopLeft));
    assert_eq!(Alignment::from_name("middle"), None);
}

#[test]
fn a_label_starts_at_blockuis_defaults() {
    let label = Label::new(Vec::new());
    assert_eq!(label.align, Alignment::MiddleLeft);
    assert_eq!(label.color, 0xFFFF_FFFF);
    assert!(!label.shadow);
    assert!(!label.wrap);
    assert_eq!(label.line_spacing, 0.0);
    assert_eq!(label.scale, 1.0);
    assert_eq!(LABEL_DEFAULTS.source, "controls/AbstractTextElement.java:44-49");
}

#[test]
fn the_default_white_is_opaque_because_the_call_takes_argb() {
    // Java writes `0xffffff` (`AbstractTextElement.java:46`) and the game supplies the
    // alpha; `DrawContext::text` takes `0xAARRGGBB` and names `0xFFFFFFFF` opaque white
    // (`crates/horizon-mod-vocab/src/host/draw.rs:59-61`).
    assert_eq!(LABEL_DEFAULTS.color, 0xFFFF_FFFF);
    assert_ne!(LABEL_DEFAULTS.color, 0x00FF_FFFF);
}

#[test]
fn a_bar_clamps_its_fraction_and_reads_a_nan_as_nothing() {
    assert_eq!(Bar::new(1.5).fraction(), 1.0);
    assert_eq!(Bar::new(-0.2).fraction(), 0.0);
    assert_eq!(Bar::new(f32::NAN).fraction(), 0.0);
    assert_eq!(Bar::new(f32::INFINITY).fraction(), 0.0);
    let mut bar = Bar::new(0.0);
    bar.set_fraction(0.4);
    assert_eq!(bar.fraction(), 0.4);
}

#[test]
fn a_list_measures_its_content_between_rows() {
    // `ScrollingListContainer.java:170` then `:187`: `n * h + (n - 1) * spacing`.
    let list = list_of(3);
    assert_eq!(list.content_height(), 3.0 * 18.0 + 2.0 * 2.0);
    assert_eq!(List::new(18.0).content_height(), 0.0);
}

#[test]
fn a_list_clamps_its_scroll_to_the_content() {
    let mut list = list_of(10);
    // 10 rows of 18 with 9 gaps of 2 = 198; in a 100-pixel viewport, 98 is the end.
    assert_eq!(list.content_height(), 198.0);
    assert_eq!(list.max_scroll(100.0), 98.0);
    assert!(list.set_scroll(999.0, 100.0));
    assert_eq!(list.scroll(), 98.0);
    assert!(list.set_scroll(-5.0, 100.0));
    assert_eq!(list.scroll(), 0.0);
    assert!(!list.scroll_by(-10.0, 100.0));
    assert!(list.scroll_by(20.0, 100.0));
    assert_eq!(list.scroll(), 20.0);
    assert_eq!(List::page_height(100.0), 90.0);
}

#[test]
fn a_list_narrows_its_viewport_for_a_scrollbar() {
    // `ScrollingView.java:12`, `:43`: 8 pixels on the right.
    let mut list = List::new(18.0);
    let rect = Rect::new(10.0, 10.0, 100.0, 50.0);
    assert_eq!(list.viewport(rect), rect);
    list.scrollbar = true;
    assert_eq!(list.viewport(rect), Rect::new(10.0, 10.0, 92.0, 50.0));
}

#[test]
fn a_screen_finds_a_widget_by_the_id_java_puts_on_it() {
    let mut screen = window();
    screen.push(label_at("title", Rect::new(0.0, 0.0, 10.0, 10.0), "a"));
    screen.push(label_at("hint", Rect::new(0.0, 20.0, 10.0, 10.0), "b"));
    assert_eq!(screen.len(), 2);
    assert!(!screen.is_empty());
    assert_eq!(screen.get(Id::new("hint")).map(Widget::id), Some(Id::new("hint")));
    assert_eq!(screen.get(Id::new("missing")), None);
    assert_eq!(screen.widget_at(1).map(Widget::id), Some(Id::new("hint")));
    assert_eq!(screen.get_mut(Id::new("title")).map(|w| w.pane.rect.y), Some(0.0));
}

// ---------------------------------------------------------------- the layout

#[test]
fn a_widget_sits_at_its_parents_origin_plus_its_own_xy() {
    let container = Container::new(Rect::new(10.0, 20.0, 100.0, 80.0));
    let pane = Pane::new(Id::new("a"), Rect::new(5.0, 6.0, 20.0, 10.0));
    assert_eq!(resolve(&container, &pane), Rect::new(15.0, 26.0, 20.0, 10.0));
}

#[test]
fn padding_moves_the_interior_and_shrinks_it() {
    // `View.padding`, `View.java:279-287`.
    let container = Container::padded(Rect::new(0.0, 0.0, 100.0, 50.0), 4.0);
    assert_eq!(container.interior(), Rect::new(4.0, 4.0, 92.0, 42.0));
    let pane = Pane::new(Id::new("a"), Rect::new(0.0, 0.0, 10.0, 10.0));
    assert_eq!(resolve(&container, &pane), Rect::new(4.0, 4.0, 10.0, 10.0));
}

#[test]
fn a_negative_width_is_the_interior_minus_its_absolute() {
    // `View.java:325-331`.
    let container = Container::padded(Rect::new(0.0, 0.0, 100.0, 50.0), 4.0);
    let pane = Pane::new(Id::new("a"), Rect::new(0.0, 0.0, -10.0, 10.0));
    assert_eq!(resolve(&container, &pane), Rect::new(4.0, 4.0, 82.0, 10.0));
}

#[test]
fn a_negative_height_is_the_interior_minus_its_absolute() {
    // `View.java:344-348`.
    let container = Container::padded(Rect::new(0.0, 0.0, 100.0, 50.0), 4.0);
    let pane = Pane::new(Id::new("a"), Rect::new(0.0, 0.0, 10.0, -12.0));
    assert_eq!(resolve(&container, &pane), Rect::new(4.0, 4.0, 10.0, 30.0));
}

#[test]
fn a_negative_side_never_goes_below_zero() {
    let container = Container::padded(Rect::new(0.0, 0.0, 100.0, 50.0), 4.0);
    let pane = Pane::new(Id::new("a"), Rect::new(0.0, 0.0, -500.0, -500.0));
    let rect = resolve(&container, &pane);
    assert_eq!(rect.w, 0.0);
    assert_eq!(rect.h, 0.0);
}

#[test]
fn right_alignment_counts_back_from_the_interior_edge() {
    // `View.java:334-337`.
    let container = Container::padded(Rect::new(0.0, 0.0, 100.0, 50.0), 4.0);
    let pane = Pane::new(Id::new("a"), Rect::new(0.0, 0.0, 20.0, 10.0))
        .with_align(Alignment::TopRight);
    assert_eq!(resolve(&container, &pane).x, 4.0 + (92.0 - 20.0));
}

#[test]
fn centred_alignment_centres_on_both_axes() {
    // `View.java:338-341`, `:354-357`.
    let container = Container::padded(Rect::new(0.0, 0.0, 100.0, 50.0), 4.0);
    let pane =
        Pane::new(Id::new("a"), Rect::new(0.0, 0.0, 20.0, 10.0)).with_align(Alignment::Middle);
    let rect = resolve(&container, &pane);
    assert_eq!(rect.x, 4.0 + (92.0 - 20.0) / 2.0);
    assert_eq!(rect.y, 4.0 + (42.0 - 10.0) / 2.0);
}

#[test]
fn an_offset_on_an_aligned_child_is_added_after_centring() {
    // Java centres, then adds the child's own x (`View.java:340`).
    let container = Container::padded(Rect::new(0.0, 0.0, 100.0, 50.0), 4.0);
    let pane =
        Pane::new(Id::new("a"), Rect::new(3.0, 0.0, 20.0, 10.0)).with_align(Alignment::Middle);
    assert_eq!(resolve(&container, &pane).x, 4.0 + 36.0 + 3.0);
}

#[test]
fn bottom_right_alignment_moves_on_both_axes() {
    let container = Container::padded(Rect::new(0.0, 0.0, 100.0, 50.0), 4.0);
    let pane = Pane::new(Id::new("a"), Rect::new(0.0, 0.0, 20.0, 10.0))
        .with_align(Alignment::BottomRight);
    let rect = resolve(&container, &pane);
    assert_eq!(rect.x, 76.0);
    assert_eq!(rect.y, 4.0 + (42.0 - 10.0));
}

#[test]
fn place_refuses_a_rectangle_that_is_not_finite() {
    let mut screen = window();
    screen.push(label_at("lost", Rect::new(f32::NAN, 0.0, 10.0, 10.0), "x"));
    assert_eq!(
        place(&screen),
        Err(Refusal::NotFinite {
            id: Id::new("lost")
        })
    );
}

#[test]
fn place_refuses_the_same_id_twice() {
    // `Pane.findPaneByID` answers one of them (`Pane.java:460-472`) and nothing says
    // which, so the screen is ambiguous.
    let mut screen = window();
    screen.push(label_at("twice", Rect::new(0.0, 0.0, 10.0, 10.0), "a"));
    screen.push(label_at("twice", Rect::new(0.0, 20.0, 10.0, 10.0), "b"));
    assert_eq!(
        place(&screen),
        Err(Refusal::DuplicateId {
            id: Id::new("twice")
        })
    );
}

#[test]
fn check_refuses_two_widgets_on_the_same_pixels_and_names_them() {
    let mut screen = window();
    screen.push(label_at("a", Rect::new(0.0, 0.0, 20.0, 10.0), "first"));
    screen.push(label_at("b", Rect::new(5.0, 5.0, 20.0, 10.0), "second"));
    let refusal = check(&screen).expect_err("5 by 5 pixels are in both");
    assert_eq!(
        refusal,
        Refusal::Overlap {
            first: Id::new("a"),
            second: Id::new("b"),
            area: Rect::new(5.0, 5.0, 15.0, 5.0),
        }
    );
    assert_eq!(refusal.id(), Id::new("b"));
    assert_eq!(refusal.reason(), "two panes cover the same pixels");
    assert!(refusal.to_string().contains("covers"));
}

#[test]
fn a_widget_that_only_touches_its_neighbour_does_not_collide() {
    let mut screen = window();
    screen.push(label_at("a", Rect::new(0.0, 0.0, 10.0, 10.0), "a"));
    screen.push(label_at("b", Rect::new(10.0, 0.0, 10.0, 10.0), "b"));
    assert!(check(&screen).is_ok());
}

#[test]
fn an_overlay_pane_may_cover_what_it_sits_on() {
    let mut screen = window();
    screen.push(label_at("a", Rect::new(0.0, 0.0, 20.0, 10.0), "under"));
    let mut over = label_at("b", Rect::new(0.0, 0.0, 20.0, 10.0), "over");
    over.pane.overlay = true;
    screen.push(over);
    let layout = check(&screen).expect("an overlay is an intended overlap");
    assert!(layout.collisions().is_empty());
}

#[test]
fn every_collision_is_listed_not_only_the_first() {
    let mut screen = window();
    for id in ["a", "b", "c"] {
        screen.push(label_at(id, Rect::new(0.0, 0.0, 20.0, 10.0), id));
    }
    let layout = place(&screen).expect("three panes, three names");
    assert_eq!(layout.collisions().len(), 3);
    assert_eq!(
        layout.disjoint(),
        Err(Refusal::Overlap {
            first: Id::new("a"),
            second: Id::new("b"),
            area: Rect::new(0.0, 0.0, 20.0, 10.0),
        })
    );
}

#[test]
fn a_list_shows_the_rows_the_viewport_reaches() {
    // `ScrollingListContainer.java:142`, both comparisons inclusive.
    let list = list_of(10);
    assert_eq!(visible_rows(&list, 40.0), 0..3);
    let mut scrolled = list_of(10);
    scrolled.set_scroll(30.0, 40.0);
    assert_eq!(visible_rows(&scrolled, 40.0), 1..4);
    assert_eq!(visible_rows(&List::new(18.0), 40.0), 0..0);
}

#[test]
fn the_last_row_is_shown_once_the_scroll_reaches_the_end() {
    let mut list = list_of(10);
    let end = list.max_scroll(40.0);
    list.set_scroll(end, 40.0);
    assert_eq!(visible_rows(&list, 40.0), 7..10);
}

// ----------------------------------------------------------------- the plan

#[test]
fn a_left_aligned_label_draws_at_its_panes_left_edge_centred_vertically() {
    // `MIDDLE_LEFT` (`AbstractTextElement.java:45`): left edge, vertical middle.
    let mut screen = window();
    screen.push(label_at("a", Rect::new(10.0, 20.0, 100.0, 20.0), "abc"));
    let layout = check(&screen).expect("one label");
    let plan = plan(&screen, &layout, &Font, [0.0, 0.0]);
    assert!(plan.gaps.is_empty());
    let (line, at, argb) = text_at(&plan, 0);
    assert_eq!(line.plain(), Some("abc"));
    assert_eq!(at[0], 10.0);
    assert!(close(at[1], 20.0 + (20.0 - 8.0) / 2.0));
    assert_eq!(argb, LABEL_DEFAULTS.color);
}

#[test]
fn a_centred_label_is_offset_by_half_the_leftover() {
    // `AbstractTextElement.java:293-298`: `(textWidth - renderedTextWidth) / 2`.
    let mut screen = window();
    let mut label = label_at("a", Rect::new(10.0, 20.0, 100.0, 20.0), "abc");
    if let crate::widgets::model::Body::Label(body) = &mut label.body {
        body.align = Alignment::Middle;
    }
    screen.push(label);
    let layout = check(&screen).expect("one label");
    let plan = plan(&screen, &layout, &Font, [0.0, 0.0]);
    // "abc" is 18 pixels wide in a 100-pixel box.
    assert_eq!(text_at(&plan, 0).1[0], 10.0 + (100.0 - 18.0) / 2.0);
}

#[test]
fn a_right_aligned_label_ends_at_the_panes_right_edge() {
    let mut screen = window();
    let mut label = label_at("a", Rect::new(10.0, 20.0, 100.0, 20.0), "abc");
    if let crate::widgets::model::Body::Label(body) = &mut label.body {
        body.align = Alignment::MiddleRight;
    }
    screen.push(label);
    let layout = check(&screen).expect("one label");
    let plan = plan(&screen, &layout, &Font, [0.0, 0.0]);
    assert_eq!(text_at(&plan, 0).1[0], 110.0 - 18.0);
}

#[test]
fn without_wrap_only_the_first_paragraph_is_drawn() {
    // `recalcPreparedTextBox` keeps `preparedText.subList(0, 1)`
    // (`AbstractTextElement.java:266-271`).
    let mut screen = window();
    let label = Label::new(vec![Line::literal("first"), Line::literal("second")]);
    screen.push(Widget::label(
        Pane::new(Id::new("a"), Rect::new(0.0, 0.0, 100.0, 20.0)),
        label,
    ));
    let layout = check(&screen).expect("one label");
    let plan = plan(&screen, &layout, &Font, [0.0, 0.0]);
    assert_eq!(plan.ops.len(), 1);
    assert_eq!(text_at(&plan, 0).0.plain(), Some("first"));
}

#[test]
fn a_wrapped_label_is_a_named_gap_and_draws_nothing() {
    // `font.split` is the engine's and `text_lines` (#4490) is not in this SDK build.
    let mut screen = window();
    let mut label = label_at("a", Rect::new(0.0, 0.0, 40.0, 60.0), "a long sentence");
    if let crate::widgets::model::Body::Label(body) = &mut label.body {
        body.wrap = true;
    }
    screen.push(label);
    let layout = check(&screen).expect("one label");
    let plan = plan(&screen, &layout, &Font, [0.0, 0.0]);
    assert!(plan.is_empty());
    assert_eq!(
        plan.gaps,
        vec![Gap::WrappedLabel {
            id: Id::new("a"),
            blocks: 1
        }]
    );
    assert!(Gap::WrappedLabel { id: Id::new("a"), blocks: 1 }.reason().contains("Font.split"));
}

#[test]
fn a_line_wider_than_its_pane_is_reported_and_drawn_whole() {
    // Java splits at the box width (`AbstractTextElement.java:211-213`) and keeps the
    // first piece; the split is the same missing call, so the line overflows and says so.
    let mut screen = window();
    screen.push(label_at("a", Rect::new(0.0, 0.0, 20.0, 20.0), "abcdef"));
    let layout = check(&screen).expect("one label");
    let plan = plan(&screen, &layout, &Font, [0.0, 0.0]);
    assert_eq!(
        plan.gaps,
        vec![Gap::CutLine {
            id: Id::new("a"),
            width: 36.0,
            room: 20.0
        }]
    );
    assert_eq!(plan.ops.len(), 1);
}

#[test]
fn a_scaled_label_is_a_named_gap_and_keeps_its_scale_in_the_position() {
    let mut screen = window();
    let mut label = label_at("a", Rect::new(0.0, 20.0, 100.0, 20.0), "abc");
    if let crate::widgets::model::Body::Label(body) = &mut label.body {
        body.scale = 0.5;
    }
    screen.push(label);
    let layout = check(&screen).expect("one label");
    let plan = plan(&screen, &layout, &Font, [0.0, 0.0]);
    assert_eq!(
        plan.gaps,
        vec![Gap::ScaledText {
            id: Id::new("a"),
            scale: 0.5
        }]
    );
    // `(lineHeight - 1) * scale` is 4, centred in 20: y is 28.
    assert!(close(text_at(&plan, 0).1[1], 28.0));
}

#[test]
fn a_component_with_no_characters_is_unmeasurable_when_it_must_be_centred() {
    let mut screen = window();
    let label = Label::line(Line::component(Text::translate("gui.minecolonies.title", Vec::new())))
        .with_align(Alignment::Middle);
    screen.push(Widget::label(
        Pane::new(Id::new("a"), Rect::new(10.0, 0.0, 100.0, 20.0)),
        label,
    ));
    let layout = check(&screen).expect("one label");
    let plan = plan(&screen, &layout, &Font, [0.0, 0.0]);
    assert_eq!(
        plan.gaps,
        vec![Gap::Unmeasurable {
            id: Id::new("a")
        }]
    );
    assert_eq!(text_at(&plan, 0).1[0], 10.0);
    assert_eq!(text_at(&plan, 0).0.plain(), None);
}

#[test]
fn a_component_on_the_left_edge_needs_no_width_and_is_not_a_gap() {
    let mut screen = window();
    let label = Label::line(Line::component(Text::literal("styled")));
    screen.push(Widget::label(
        Pane::new(Id::new("a"), Rect::new(10.0, 0.0, 100.0, 20.0)),
        label,
    ));
    let layout = check(&screen).expect("one label");
    let plan = plan(&screen, &layout, &Font, [0.0, 0.0]);
    assert!(plan.gaps.is_empty());
    assert_eq!(plan.ops.len(), 1);
}

#[test]
fn the_hovered_and_disabled_colours_follow_java() {
    // `enabled ? (hovered ? hover : normal) : disabled`,
    // `AbstractTextElement.java:288`.
    let mut screen = window();
    let mut label = label_at("a", Rect::new(0.0, 0.0, 100.0, 20.0), "abc");
    if let crate::widgets::model::Body::Label(body) = &mut label.body {
        body.color = 0xFFFF_FFFF;
        body.color_hover = Some(0xFFFF_0000);
        body.color_disabled = Some(0xFF80_8080);
    }
    screen.push(label);
    let layout = check(&screen).expect("one label");
    let hovered = plan(&screen, &layout, &Font, [10.0, 10.0]);
    assert_eq!(text_at(&hovered, 0).2, 0xFFFF_0000);
    let away = plan(&screen, &layout, &Font, [500.0, 500.0]);
    assert_eq!(text_at(&away, 0).2, 0xFFFF_FFFF);
    screen
        .get_mut(Id::new("a"))
        .expect("the label")
        .pane
        .state = State::DISABLED;
    let layout = check(&screen).expect("one label");
    let disabled = plan(&screen, &layout, &Font, [10.0, 10.0]);
    assert_eq!(text_at(&disabled, 0).2, 0xFF80_8080);
}

#[test]
fn a_button_asks_for_the_games_button_and_the_pointer_highlights_it() {
    let mut screen = window();
    screen.push(Widget::button(
        Pane::new(Id::new("ok"), Rect::new(0.0, 0.0, 100.0, 20.0)),
        Button::text("Ok"),
    ));
    let layout = check(&screen).expect("one button");
    let plan = plan(&screen, &layout, &Font, [50.0, 10.0]);
    assert_eq!(
        plan.ops,
        vec![Op::Button {
            rect: Rect::new(0.0, 0.0, 100.0, 20.0),
            label: Text::literal("Ok"),
            active: true,
            highlighted: true,
        }]
    );
    let pressed = crate::widgets::adapt::plan(&screen, &layout, &Font, [500.0, 500.0]);
    assert_eq!(
        pressed.ops,
        vec![Op::Button {
            rect: Rect::new(0.0, 0.0, 100.0, 20.0),
            label: Text::literal("Ok"),
            active: true,
            highlighted: false,
        }]
    );
}

#[test]
fn a_disabled_button_is_still_drawn_and_inactive() {
    let mut screen = window();
    screen.push(Widget::button(
        Pane::new(Id::new("ok"), Rect::new(0.0, 0.0, 100.0, 20.0)).disabled(),
        Button::text("Ok"),
    ));
    let layout = check(&screen).expect("one button");
    let plan = plan(&screen, &layout, &Font, [50.0, 10.0]);
    assert_eq!(
        plan.ops,
        vec![Op::Button {
            rect: Rect::new(0.0, 0.0, 100.0, 20.0),
            label: Text::literal("Ok"),
            active: false,
            highlighted: false,
        }]
    );
}

#[test]
fn a_sprite_button_is_nine_sliced_and_loses_its_narration() {
    let mut screen = window();
    screen.push(Widget::button(
        Pane::new(Id::new("ok"), Rect::new(0.0, 0.0, 100.0, 20.0)),
        Button::text("Ok").with_sprite("minecolonies:gui/widgets/button"),
    ));
    let layout = check(&screen).expect("one button");
    let plan = plan(&screen, &layout, &Font, [0.0, 0.0]);
    assert_eq!(plan.ops.len(), 2);
    assert_eq!(
        plan.ops[0],
        Op::NineSlice {
            sprite: "minecolonies:gui/widgets/button",
            rect: Rect::new(0.0, 0.0, 100.0, 20.0),
            argb: 0xFFFF_FFFF,
        }
    );
    // "Ok" is 12 pixels wide, centred in 100.
    assert_eq!(text_at(&plan, 1).1[0], (100.0 - 12.0) / 2.0);
    assert_eq!(
        plan.gaps,
        vec![Gap::ButtonNarration {
            id: Id::new("ok"),
            sprite: "minecolonies:gui/widgets/button",
        }]
    );
}

#[test]
fn a_bar_draws_its_track_then_the_filled_part() {
    let mut screen = window();
    screen.push(Widget::bar(
        Pane::new(Id::new("progress"), Rect::new(0.0, 0.0, 100.0, 10.0)),
        Bar::new(0.25),
    ));
    let layout = check(&screen).expect("one bar");
    let plan = plan(&screen, &layout, &Font, [0.0, 0.0]);
    assert_eq!(plan.ops.len(), 2);
    let (track, _) = fill_at(&plan, 0);
    let (filled, _) = fill_at(&plan, 1);
    assert_eq!(track, Rect::new(0.0, 0.0, 100.0, 10.0));
    assert_eq!(filled, Rect::new(0.0, 0.0, 25.0, 10.0));
}

#[test]
fn an_empty_bar_draws_its_track_only() {
    let mut screen = window();
    screen.push(Widget::bar(
        Pane::new(Id::new("progress"), Rect::new(0.0, 0.0, 100.0, 10.0)),
        Bar::new(0.0),
    ));
    let layout = check(&screen).expect("one bar");
    let plan = plan(&screen, &layout, &Font, [0.0, 0.0]);
    assert_eq!(plan.ops.len(), 1);
    assert!(plan.gaps.is_empty());
}

#[test]
fn a_gradient_bar_is_a_named_gap() {
    let mut screen = window();
    screen.push(Widget::bar(
        Pane::new(Id::new("progress"), Rect::new(0.0, 0.0, 100.0, 10.0)),
        Bar::new(0.5).with_gradient(),
    ));
    let layout = check(&screen).expect("one bar");
    let plan = plan(&screen, &layout, &Font, [0.0, 0.0]);
    assert_eq!(
        plan.gaps,
        vec![Gap::Gradient {
            id: Id::new("progress")
        }]
    );
    assert_eq!(plan.ops.len(), 2);
}

#[test]
fn a_list_clips_its_viewport_and_draws_the_rows_in_it() {
    let plan = list_plan(list_of(10), Rect::new(0.0, 0.0, 100.0, 40.0));
    assert_eq!(
        plan.ops.first(),
        Some(&Op::Clip {
            rect: Rect::new(0.0, 0.0, 100.0, 40.0)
        })
    );
    assert_eq!(plan.ops.last(), Some(&Op::Unclip));
    // Three rows reach a 40-pixel viewport: rows at y 0, 20 and 40
    // (`ScrollingListContainer.java:142`).
    assert_eq!(plan.ops.len(), 5);
    // The second row is drawn from y 20, the text centred in its 18-pixel row.
    assert_eq!(text_at(&plan, 2).0.plain(), Some("row 1"));
    assert!(close(text_at(&plan, 2).1[1], 20.0 + (18.0 - 8.0) / 2.0));
}

#[test]
fn a_list_scrolled_to_the_end_shows_the_last_row_above_the_clip() {
    let mut list = list_of(10);
    let end = list.max_scroll(40.0);
    list.set_scroll(end, 40.0);
    let plan = list_plan(list, Rect::new(0.0, 0.0, 100.0, 40.0));
    // Rows 7, 8 and 9, the last at 180 - 158 = 22 in the viewport.
    assert_eq!(plan.ops.len(), 5);
    assert_eq!(text_at(&plan, 3).0.plain(), Some("row 9"));
    assert!(close(text_at(&plan, 3).1[1], 22.0 + (18.0 - 8.0) / 2.0));
}

#[test]
fn an_empty_list_draws_its_empty_text_instead_of_rows() {
    // `ScrollingListContainer.java:179-186`.
    let mut list = List::new(18.0);
    list.empty_text = Some(Line::literal("nothing here"));
    let plan = list_plan(list, Rect::new(0.0, 0.0, 100.0, 60.0));
    assert_eq!(plan.ops.len(), 1);
    let (line, at, _) = text_at(&plan, 0);
    assert_eq!(line.plain(), Some("nothing here"));
    assert!(close(at[1], (60.0 - 8.0) / 2.0));
}

#[test]
fn a_list_with_a_scrollbar_narrows_its_viewport_and_names_the_gap() {
    let mut list = list_of(3);
    list.scrollbar = true;
    let plan = list_plan(list, Rect::new(0.0, 0.0, 100.0, 40.0));
    assert_eq!(
        plan.ops.first(),
        Some(&Op::Clip {
            rect: Rect::new(0.0, 0.0, 92.0, 40.0)
        })
    );
    assert_eq!(
        plan.gaps,
        vec![Gap::Scrollbar {
            id: Id::new("rows")
        }]
    );
}

#[test]
fn a_hidden_widget_draws_nothing_and_says_nothing() {
    // `Pane.shouldDraw`, `Pane.java:205-210`.
    let mut screen = window();
    screen.push(Widget::label(
        Pane::new(Id::new("gone"), Rect::new(0.0, 0.0, 100.0, 20.0)).hidden(),
        Label::line(Line::literal("invisible")),
    ));
    let layout = check(&screen).expect("a hidden pane collides with nothing");
    let plan = plan(&screen, &layout, &Font, [0.0, 0.0]);
    assert!(plan.is_empty());
    assert!(plan.gaps.is_empty());
}

#[test]
fn every_widget_kind_is_planned_in_the_order_the_screen_draws_them() {
    let mut screen = window();
    screen.push(label_at("name", Rect::new(0.0, 0.0, 100.0, 10.0), "colony"));
    screen.push(Widget::bar(
        Pane::new(Id::new("progress"), Rect::new(0.0, 20.0, 100.0, 10.0)),
        Bar::new(1.0),
    ));
    screen.push(Widget::button(
        Pane::new(Id::new("ok"), Rect::new(0.0, 40.0, 100.0, 20.0)),
        Button::text("Ok"),
    ));
    let layout = check(&screen).expect("three panes, no overlap");
    let plan = plan(&screen, &layout, &Font, [0.0, 0.0]);
    // One text, two fills, one button, in that order (`View.java:87-97`).
    assert!(matches!(plan.ops[0], Op::Text { .. }));
    assert!(matches!(plan.ops[1], Op::Fill { .. }));
    assert!(matches!(plan.ops[2], Op::Fill { .. }));
    assert!(matches!(plan.ops[3], Op::Button { .. }));
    assert_eq!(plan.ops.len(), 4);
}

// -------------------------------------------------------------- the drawing

/// What a plan asked the game to draw: one line per call, in order.
#[derive(Default)]
struct Recorder {
    calls: Vec<String>,
    open: usize,
}

impl DrawContext for Recorder {
    fn fill(&mut self, rect: [f32; 4], argb: u32) {
        self.calls.push(format!("fill {rect:?} {argb:08X}"));
    }

    fn text(&mut self, text: &str, at: [f32; 2], argb: u32, shadow: bool) {
        self.calls
            .push(format!("text {text:?} {at:?} {argb:08X} {shadow}"));
    }

    fn gui_paint(&mut self, draws: &[GuiDraw]) -> Result<(), TextFormError> {
        self.calls.push(format!("paint {}", draws.len()));
        Ok(())
    }

    fn push_clip(&mut self, rect: [f32; 4]) -> Result<(), GuiClipRefusal> {
        self.open += 1;
        self.calls.push(format!("clip {rect:?}"));
        Ok(())
    }

    fn pop_clip(&mut self) -> Result<(), GuiClipRefusal> {
        if self.open == 0 {
            return Err(GuiClipRefusal::NoneOpen);
        }
        self.open -= 1;
        self.calls.push("unclip".to_owned());
        Ok(())
    }

    fn widget_button(
        &mut self,
        rect: [f32; 4],
        _label: &Text,
        active: bool,
        highlighted: bool,
    ) -> Result<(), TextFormError> {
        self.calls
            .push(format!("button {rect:?} {active} {highlighted}"));
        Ok(())
    }
}

#[test]
fn execute_sends_every_plain_op_to_the_draw_context() {
    let mut screen = window();
    screen.push(label_at("name", Rect::new(0.0, 0.0, 100.0, 10.0), "colony"));
    screen.push(Widget::button(
        Pane::new(Id::new("ok"), Rect::new(0.0, 40.0, 100.0, 20.0)),
        Button::text("Ok"),
    ));
    screen.push(Widget::list(
        Pane::new(Id::new("rows"), Rect::new(0.0, 70.0, 100.0, 40.0)),
        list_of(2),
    ));
    let layout = check(&screen).expect("three panes, no overlap");
    let plan = plan(&screen, &layout, &Font, [0.0, 0.0]);
    let mut recorder = Recorder::default();
    execute(&plan, &mut recorder).expect("nothing refuses");
    assert_eq!(recorder.open, 0, "the plan closed every clip it opened");
    assert_eq!(recorder.calls[0], "text \"colony\" [0.0, 1.0] FFFFFFFF false");
    assert_eq!(recorder.calls[1], "button [0.0, 40.0, 100.0, 20.0] true false");
    assert_eq!(recorder.calls[2], "clip [0.0, 70.0, 100.0, 40.0]");
    assert_eq!(recorder.calls.last().map(String::as_str), Some("unclip"));
}

#[test]
fn execute_sends_a_component_through_the_batch_and_a_plain_line_directly() {
    let mut screen = window();
    let component = Label::line(Line::component(Text::translate("gui.minecolonies.title", Vec::new())));
    screen.push(Widget::label(
        Pane::new(Id::new("title"), Rect::new(0.0, 0.0, 100.0, 10.0)),
        component,
    ));
    screen.push(label_at("hint", Rect::new(0.0, 20.0, 100.0, 10.0), "plain"));
    let layout = check(&screen).expect("two labels");
    let plan = plan(&screen, &layout, &Font, [0.0, 0.0]);
    let mut recorder = Recorder::default();
    execute(&plan, &mut recorder).expect("nothing refuses");
    assert_eq!(recorder.calls[0], "paint 1");
    assert!(recorder.calls[1].starts_with("text \"plain\""));
}

#[test]
fn execute_refuses_an_unpaired_unclip_instead_of_swallowing_it() {
    let plan = Plan {
        ops: vec![Op::Unclip],
        gaps: Vec::new(),
    };
    let mut recorder = Recorder::default();
    assert_eq!(
        execute(&plan, &mut recorder),
        Err(Call::Clip(GuiClipRefusal::NoneOpen))
    );
    assert!(Call::Clip(GuiClipRefusal::NoneOpen).to_string().starts_with("clip:"));
}
