//! What is drawn over a picture: each test fails if its feature is removed.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "test helpers fail loudly")]

use ravenrs_core::{
    Brush, Color, CursorIcon, Event, ImageData, Key, Modifiers, Point, PointerButton, Primitive, Rect, Role,
    Size,
};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::*;

fn ui(view: impl View + 'static) -> Ui {
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.update();
    ui.update();
    ui
}

fn settle(ui: &mut Ui) {
    ui.update();
    ui.update();
}

fn quads(ui: &Ui) -> Vec<Rect> {
    ui.scene()
        .primitives()
        .iter()
        .filter_map(|p| match p {
            Primitive::Quad(q) if matches!(q.fill, Brush::Solid(_)) => Some(q.rect),
            _ => None,
        })
        .collect()
}

fn paths(ui: &Ui) -> usize {
    ui.scene().primitives().iter().filter(|p| matches!(p, Primitive::Path(_))).count()
}

fn press(ui: &mut Ui, key: Key, modifiers: Modifiers) {
    ui.handle_event(&Event::KeyDown { key, modifiers });
    settle(ui);
}

fn shift() -> Modifiers {
    Modifiers { shift: true, ..Modifiers::default() }
}

fn alt() -> Modifiers {
    Modifiers { alt: true, ..Modifiers::default() }
}

fn down(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.update();
}

fn drag_to(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.update();
}

fn up(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    settle(ui);
}

fn drag(ui: &mut Ui, from: Point, to: Point) {
    down(ui, from);
    drag_to(ui, to);
    up(ui, to);
}

fn hover(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    settle(ui);
}

fn node(ui: &mut Ui, role: Role) -> ravenrs_core::AccessNode {
    let tree = ui.access_tree();
    tree.nodes.iter().find(|n| n.role == role).cloned().expect("a node of that role")
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.51
}

fn picture() -> ImageData {
    ImageData::from_fn(100, 50, |x, _| Color::rgb(x as f32 / 100.0, 0.2, 0.3)).unwrap()
}

// -- ContentSpace ------------------------------------------------------------------------------

#[test]
fn a_space_maps_both_ways() {
    let frame = Rect::new(10.0, 20.0, 200.0, 200.0);
    let fit = ContentSpace::Fit(Size::new(100.0, 50.0));
    // 100x50 fills the width at scale 2, centered vertically.
    assert_eq!(fit.placed(frame), Rect::new(10.0, 70.0, 200.0, 100.0));
    assert_eq!(fit.to_screen(frame, Point::new(50.0, 25.0)), Point::new(110.0, 120.0));
    assert_eq!(fit.to_content(frame, Point::new(110.0, 120.0)), Point::new(50.0, 25.0));

    let view = State::new(ZoomView::at(2.0).centered_on(Point::new(0.0, 0.0)));
    let over = ContentSpace::View(view.binding(), Size::new(100.0, 50.0));
    // The corner of the content is in the middle of the frame, at twice the size.
    assert_eq!(over.to_screen(frame, Point::ZERO), Point::new(110.0, 120.0));
    assert_eq!(over.to_screen(frame, Point::new(10.0, 10.0)), Point::new(130.0, 140.0));
    view.set(ZoomView::fit());
    assert_eq!(over.placed(frame), Rect::new(10.0, 70.0, 200.0, 100.0));
}

// -- RuleOfThirdsGrid --------------------------------------------------------------------------

#[test]
fn a_grid_draws_its_guide_where_asked() {
    let grid = ui(RuleOfThirdsGrid::new().size(300.0, 150.0));
    let lines = quads(&grid);
    let vertical: Vec<f32> = lines.iter().filter(|r| r.height() > 100.0).map(|r| r.x()).collect();
    assert!(vertical.iter().any(|x| near(*x, 100.0)), "a line at a third: {vertical:?}");
    assert!(vertical.iter().any(|x| near(*x, 200.0)), "a line at two thirds: {vertical:?}");

    let fine = ui(RuleOfThirdsGrid::new().guide(CompositionGuide::Divisions(8)).size(300.0, 150.0));
    assert!(quads(&fine).len() > lines.len(), "eight parts draw more lines than three");

    let golden = ui(RuleOfThirdsGrid::new().guide(CompositionGuide::GoldenRatio).size(300.0, 150.0));
    assert!(quads(&golden).iter().any(|r| r.height() > 100.0 && near(r.x(), 185.4)));

    let squares = ui(RuleOfThirdsGrid::new().guide(CompositionGuide::Squares(2)).size(300.0, 150.0));
    let cells = quads(&squares);
    assert!(cells.iter().any(|r| r.height() > 100.0 && near(r.x(), 75.0)), "cells as wide as they are tall");
    assert!(cells.iter().any(|r| r.width() > 200.0 && near(r.y(), 75.0)));

    let area = ui(RuleOfThirdsGrid::new().area(Rect::new(30.0, 0.0, 30.0, 30.0)).size(300.0, 150.0));
    assert!(quads(&area).iter().any(|r| near(r.x(), 40.0)), "thirds of the area, not of the box");
}

#[test]
fn a_grid_leaves_the_pointer_to_what_is_under_it() {
    let clicks = State::new(0);
    let counted = clicks.clone();
    let mut ui = ui(ZStack::new((
        Button::new("Under").on_click(move || counted.update(|c| *c += 1)),
        RuleOfThirdsGrid::new().size(300.0, 150.0),
    )));
    let at = Point::new(10.0, 10.0);
    down(&mut ui, at);
    up(&mut ui, at);
    assert_eq!(clicks.get(), 1);
}

#[test]
fn a_stage_gives_the_overlay_its_size() {
    let view = RuleOfThirdsGrid::new().stage(Image::new(picture()).size(300.0, 150.0));
    let grid = ui(view);
    let lines: Vec<f32> = quads(&grid).iter().filter(|r| r.height() > 100.0).map(|r| r.x()).collect();
    assert!(lines.iter().any(|x| near(*x, 100.0)), "over the image: {lines:?}");
    let on_image = grid.scene().primitives().iter().position(|p| matches!(p, Primitive::Image(_)));
    let on_line = grid.scene().primitives().iter().position(|p| matches!(p, Primitive::Quad(_)));
    assert!(on_image < on_line, "the image goes under the lines");
}

// -- RubberBand --------------------------------------------------------------------------------

#[test]
fn a_band_follows_a_drag_and_selects_on_release() {
    let selection = State::new(None::<Rect>);
    let chosen = State::new(None::<Rect>);
    let told = chosen.clone();
    let mut ui = ui(RubberBand::new(selection.binding())
        .on_select(move |rect| told.set(Some(rect)))
        .size(300.0, 200.0));
    down(&mut ui, Point::new(100.0, 100.0));
    drag_to(&mut ui, Point::new(40.0, 150.0));
    assert_eq!(selection.get(), Some(Rect::new(40.0, 100.0, 60.0, 50.0)), "any direction");
    assert_eq!(chosen.get(), None, "selects only on release");
    up(&mut ui, Point::new(40.0, 150.0));
    assert_eq!(chosen.get(), Some(Rect::new(40.0, 100.0, 60.0, 50.0)));
    assert_eq!(selection.get(), chosen.get(), "it stays by default");
    assert!(quads(&ui).iter().any(|r| *r == Rect::new(40.0, 100.0, 60.0, 50.0)));

    // Beyond the box it stops at the edge.
    drag(&mut ui, Point::new(100.0, 100.0), Point::new(900.0, 900.0));
    assert_eq!(selection.get(), Some(Rect::new(100.0, 100.0, 200.0, 100.0)));

    // A press and release is a click, which clears and selects nothing.
    chosen.set(None);
    down(&mut ui, Point::new(10.0, 10.0));
    up(&mut ui, Point::new(10.0, 10.0));
    assert_eq!(selection.get(), None);
    assert_eq!(chosen.get(), None);
}

#[test]
fn a_band_that_does_not_keep_goes_away_after_selecting() {
    let selection = State::new(None::<Rect>);
    let chosen = State::new(None::<Rect>);
    let told = chosen.clone();
    let mut ui = ui(RubberBand::new(selection.binding())
        .keep(false)
        .on_select(move |rect| told.set(Some(rect)))
        .size(300.0, 200.0));
    drag(&mut ui, Point::new(10.0, 10.0), Point::new(50.0, 50.0));
    assert!(chosen.get().is_some());
    assert_eq!(selection.get(), None);
}

#[test]
fn a_band_speaks_content_units_over_a_view() {
    let view = State::new(ZoomView::at(2.0).centered_on(Point::new(0.5, 0.5)));
    let selection = State::new(None::<Rect>);
    let mut ui = ui(RubberBand::new(selection.binding())
        .space(ContentSpace::View(view.binding(), Size::new(100.0, 100.0)))
        .size(200.0, 200.0));
    // At twice the size, the content starts at the corner of a box of 200.
    drag(&mut ui, Point::new(20.0, 40.0), Point::new(60.0, 100.0));
    assert_eq!(selection.get(), Some(Rect::new(10.0, 20.0, 20.0, 30.0)));
    // The same rectangle follows when the view zooms.
    view.set(ZoomView::at(1.0));
    settle(&mut ui);
    assert!(quads(&ui).contains(&Rect::new(60.0, 70.0, 20.0, 30.0)));
}

#[test]
fn a_band_drag_costs_no_builds_when_nobody_reads_it() {
    let selection = State::new(None::<Rect>);
    let mut ui = ui(RubberBand::new(selection.binding()).size(300.0, 200.0));
    down(&mut ui, Point::new(10.0, 10.0));
    settle(&mut ui);
    let before = ui.stats().builds;
    for i in 0..5 {
        ui.set_time(f64::from(i + 1));
        drag_to(&mut ui, Point::new(50.0 + i as f32 * 10.0, 60.0));
    }
    assert_eq!(ui.stats().builds, before, "a drag only repaints");
    assert!(quads(&ui).iter().any(|r| near(r.max_x(), 90.0)), "and the rectangle moved");
    up(&mut ui, Point::new(90.0, 60.0));
}

#[test]
fn a_band_works_from_the_keyboard() {
    let selection = State::new(None::<Rect>);
    let chosen = State::new(None::<Rect>);
    let told = chosen.clone();
    let mut ui = ui(RubberBand::new(selection.binding())
        .on_select(move |rect| told.set(Some(rect)))
        .size(200.0, 100.0));
    press(&mut ui, Key::Tab, Modifiers::default());
    press(&mut ui, Key::ArrowRight, Modifiers::default());
    assert_eq!(selection.get(), Some(Rect::new(50.0, 25.0, 100.0, 50.0)), "starts from the middle");
    press(&mut ui, Key::ArrowRight, shift());
    assert_eq!(selection.get(), Some(Rect::new(60.0, 25.0, 100.0, 50.0)));
    press(&mut ui, Key::ArrowDown, alt());
    assert_eq!(selection.get(), Some(Rect::new(60.0, 25.0, 100.0, 51.0)));
    press(&mut ui, Key::Enter, Modifiers::default());
    assert_eq!(chosen.get(), selection.get());
    press(&mut ui, Key::Escape, Modifiers::default());
    assert_eq!(selection.get(), None);
}

#[test]
fn escape_cancels_a_band_being_dragged() {
    let selection = State::new(None::<Rect>);
    let chosen = State::new(None::<Rect>);
    let told = chosen.clone();
    let mut ui = ui(RubberBand::new(selection.binding())
        .on_select(move |rect| told.set(Some(rect)))
        .size(300.0, 200.0));
    down(&mut ui, Point::new(10.0, 10.0));
    drag_to(&mut ui, Point::new(90.0, 90.0));
    press(&mut ui, Key::Escape, Modifiers::default());
    assert_eq!(selection.get(), None);
    drag_to(&mut ui, Point::new(120.0, 120.0));
    assert_eq!(selection.get(), None, "the rest of the drag does nothing");
    up(&mut ui, Point::new(120.0, 120.0));
    assert_eq!(chosen.get(), None);
}

#[test]
fn a_disabled_band_lets_the_pointer_through() {
    let selection = State::new(None::<Rect>);
    let mut ui = ui(RubberBand::new(selection.binding()).disabled(true).size(300.0, 200.0));
    drag(&mut ui, Point::new(10.0, 10.0), Point::new(90.0, 90.0));
    assert_eq!(selection.get(), None);
}

#[test]
fn a_band_says_what_it_holds() {
    let selection = State::new(Some(Rect::new(1.0, 2.0, 30.0, 40.0)));
    let mut ui = ui(RubberBand::new(selection.binding()).label("Zoom box").size(300.0, 200.0));
    let group = node(&mut ui, Role::Group);
    assert_eq!(group.name.as_deref(), Some("Zoom box"));
    assert!(group.value.as_deref().is_some_and(|v| v.contains("30 × 40")), "{:?}", group.value);
}

// -- Marquee -----------------------------------------------------------------------------------

#[test]
fn a_marquee_marches_and_stops_with_reduced_motion() {
    let rect = Rect::new(20.0, 20.0, 80.0, 80.0);
    let mut moving = ui(Marquee::rect(rect).size(200.0, 200.0));
    assert!(moving.next_wake().is_some(), "the dashes walk");
    let first = quads(&moving);
    moving.set_time(0.1);
    moving.update();
    assert_ne!(quads(&moving), first, "a moment later the dashes moved");

    let mut still = Ui::new(Marquee::rect(rect).size(200.0, 200.0), Theme::raven_dark());
    still.set_viewport(Size::new(800.0, 600.0), 1.0);
    still.set_reduced_motion(true);
    settle(&mut still);
    assert_eq!(still.next_wake(), None, "reduced motion asks for no frames");
    assert!(quads(&still).len() > 4, "and the dashes are still there");

    let off = ui(Marquee::rect(rect).animated(false).size(200.0, 200.0));
    assert_eq!(off.next_wake(), None);
}

#[test]
fn a_marquee_draws_a_rectangle_a_path_or_segments() {
    let rect = ui(Marquee::rect(Rect::new(20.0, 20.0, 80.0, 40.0)).animated(false).size(200.0, 200.0));
    let lines = quads(&rect);
    assert!(lines.iter().any(|r| near(r.width(), 81.5) && near(r.y(), 19.25)), "top edge: {lines:?}");
    assert_eq!(paths(&rect), 0, "straight edges stay sharp quads");

    let triangle = [Point::new(10.0, 10.0), Point::new(90.0, 10.0), Point::new(50.0, 80.0)];
    let path = ui(Marquee::path(triangle).animated(false).size(200.0, 200.0));
    assert!(paths(&path) > 0, "slanted edges go in a path");

    let loose = ui(Marquee::segments([
        (Point::new(10.0, 10.0), Point::new(50.0, 10.0)),
        (Point::new(70.0, 70.0), Point::new(70.0, 90.0)),
    ])
    .animated(false)
    .size(200.0, 200.0));
    let lines = quads(&loose);
    assert!(lines.iter().any(|r| near(r.height(), 21.5) && near(r.x(), 69.25)), "{lines:?}");
    assert!(!lines.iter().any(|r| near(r.y(), 40.0)), "nothing joins the segments");
}

#[test]
fn a_marquee_follows_a_view_and_takes_no_input() {
    let view = State::new(ZoomView::at(2.0).centered_on(Point::new(0.5, 0.5)));
    let clicks = State::new(0);
    let counted = clicks.clone();
    let mut ui = ui(ZStack::new((
        Button::new("Under").on_click(move || counted.update(|c| *c += 1)),
        Marquee::rect(Rect::new(0.0, 0.0, 50.0, 50.0))
            .animated(false)
            .space(ContentSpace::View(view.binding(), Size::new(100.0, 100.0)))
            .size(200.0, 200.0),
    )));
    assert!(quads(&ui).iter().any(|r| near(r.width(), 101.5)), "twice the size");
    view.set(ZoomView::at(1.0));
    settle(&mut ui);
    assert!(quads(&ui).iter().any(|r| near(r.width(), 51.5)));
    let at = Point::new(10.0, 10.0);
    down(&mut ui, at);
    up(&mut ui, at);
    assert_eq!(clicks.get(), 1);
    assert_eq!(ui.cursor(), CursorIcon::Pointer, "the button under it still answers");
}

#[test]
fn a_marquee_has_a_name() {
    let mut ui = ui(Marquee::rect(Rect::new(0.0, 0.0, 50.0, 50.0)).size(100.0, 100.0));
    assert_eq!(node(&mut ui, Role::Image).name.as_deref(), Some("Selection"));
}

// -- TransformHandles --------------------------------------------------------------------------

fn boxed(rect: Rect) -> State<BoxTransform> {
    State::new(BoxTransform::new(rect))
}

fn close(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 0.1 && (a.y - b.y).abs() < 0.1
}

const BOX: Rect = Rect::new(100.0, 100.0, 200.0, 100.0);

#[test]
fn a_transform_knows_its_geometry() {
    let t = BoxTransform::new(Rect::new(0.0, 0.0, 100.0, 50.0)).rotated(90.0);
    // A quarter turn clockwise around the middle (50, 25).
    assert!(close(t.to_world(Point::ZERO), Point::new(75.0, -25.0)));
    assert!(close(t.handle(BoxHandle::BottomRight), Point::new(25.0, 75.0)));
    assert!(close(t.to_local(t.to_world(Point::new(10.0, 20.0))), Point::new(10.0, 20.0)));
    assert_eq!(BoxHandle::ALL.len(), 8);
}

#[test]
fn handles_resize_from_the_opposite_side() {
    let value = boxed(BOX);
    let mut ui = ui(TransformHandles::new(value.binding()).keep_proportions(false).size(600.0, 400.0));
    // The right side grows and the left stays.
    drag(&mut ui, Point::new(300.0, 150.0), Point::new(340.0, 170.0));
    assert_eq!(value.get().rect, Rect::new(100.0, 100.0, 240.0, 100.0));
    // The top left corner moves both, and the bottom right stays.
    drag(&mut ui, Point::new(100.0, 100.0), Point::new(80.0, 90.0));
    assert_eq!(value.get().rect, Rect::new(80.0, 90.0, 260.0, 110.0));
}

#[test]
fn corners_keep_proportions_until_shift() {
    let value = boxed(BOX);
    let mut ui = ui(TransformHandles::new(value.binding()).size(600.0, 400.0));
    drag(&mut ui, Point::new(300.0, 200.0), Point::new(400.0, 210.0));
    let rect = value.get().rect;
    assert_eq!((rect.x(), rect.y()), (100.0, 100.0), "the opposite corner stays");
    assert!((rect.width() / rect.height() - 2.0).abs() < 0.01, "{rect:?}");
    assert!(near(rect.width(), 300.0));

    value.set(BoxTransform::new(BOX));
    settle(&mut ui);
    ui.handle_event(&Event::KeyDown { key: Key::Character("x".into()), modifiers: shift() });
    drag(&mut ui, Point::new(300.0, 200.0), Point::new(400.0, 210.0));
    assert_eq!(value.get().rect, Rect::new(100.0, 100.0, 300.0, 110.0), "Shift lets go");
}

#[test]
fn alt_resizes_around_the_pivot() {
    let value = boxed(BOX);
    let mut ui = ui(TransformHandles::new(value.binding()).keep_proportions(false).size(600.0, 400.0));
    ui.handle_event(&Event::KeyDown { key: Key::Character("x".into()), modifiers: alt() });
    drag(&mut ui, Point::new(300.0, 150.0), Point::new(320.0, 150.0));
    assert_eq!(value.get().rect, Rect::new(80.0, 100.0, 240.0, 100.0), "both sides move");
}

#[test]
fn inside_moves_and_the_knob_turns() {
    let value = boxed(BOX);
    let commits = State::new(0);
    let counted = commits.clone();
    let mut ui = ui(TransformHandles::new(value.binding())
        .on_commit(move |_| counted.update(|c| *c += 1))
        .size(600.0, 400.0));
    drag(&mut ui, Point::new(150.0, 130.0), Point::new(170.0, 120.0));
    assert_eq!(value.get().rect, Rect::new(120.0, 90.0, 200.0, 100.0));
    assert_eq!(commits.get(), 1, "one step to undo");

    // From above the middle, 24 pixels over the top side, to the right of the middle.
    let t = BoxTransform::new(BOX);
    value.set(t);
    settle(&mut ui);
    let middle = t.pivot_point();
    drag(&mut ui, Point::new(200.0, 76.0), Point::new(middle.x + 74.0, middle.y));
    assert!((value.get().angle - 90.0).abs() < 0.5, "a quarter turn: {:?}", value.get());
    assert_eq!(value.get().rect, BOX, "turning does not resize");

    // Shift holds it to steps.
    value.set(t);
    settle(&mut ui);
    ui.handle_event(&Event::KeyDown { key: Key::Character("x".into()), modifiers: shift() });
    drag(&mut ui, Point::new(200.0, 76.0), Point::new(middle.x + 20.0, middle.y - 74.0));
    assert_eq!(value.get().angle, 15.0);
}

#[test]
fn a_turned_box_resizes_along_its_own_axes() {
    let t = BoxTransform::new(BOX).rotated(90.0);
    let value = State::new(t);
    let mut ui = ui(TransformHandles::new(value.binding()).keep_proportions(false).size(600.0, 400.0));
    let right = t.handle(BoxHandle::Right);
    let left = t.handle(BoxHandle::Left);
    // Turned a quarter, the right side faces down.
    drag(&mut ui, right, Point::new(right.x, right.y + 30.0));
    let after = value.get();
    assert!(near(after.rect.width(), 230.0), "{after:?}");
    assert!(close(after.handle(BoxHandle::Left), left), "the opposite side stays on screen");
}

#[test]
fn the_pivot_moves_without_moving_the_box() {
    let t = BoxTransform::new(BOX).rotated(30.0);
    let value = State::new(t);
    let mut ui = ui(TransformHandles::new(value.binding()).size(600.0, 400.0));
    let before = t.corners();
    let from = t.pivot_point();
    drag(&mut ui, from, Point::new(from.x + 30.0, from.y + 10.0));
    let after = value.get();
    assert!(close(after.pivot_point(), Point::new(from.x + 30.0, from.y + 10.0)));
    for (a, b) in after.corners().into_iter().zip(before) {
        assert!(close(a, b), "the box stays: {a:?} {b:?}");
    }
    // A double click on it puts it back in the middle, again without moving the box.
    hover(&mut ui, after.pivot_point());
    ui.handle_event(&Event::DoubleClick { position: after.pivot_point() });
    settle(&mut ui);
    let back = value.get();
    assert!((back.pivot.x - 0.5).abs() < 0.001 && (back.pivot.y - 0.5).abs() < 0.001, "{back:?}");
    for (a, b) in back.corners().into_iter().zip(before) {
        assert!(close(a, b));
    }
}

#[test]
fn the_cursor_says_what_a_press_will_do() {
    let value = boxed(BOX);
    let mut ui = ui(TransformHandles::new(value.binding()).size(600.0, 400.0));
    hover(&mut ui, Point::new(300.0, 150.0));
    assert_eq!(ui.cursor(), CursorIcon::ColResize);
    hover(&mut ui, Point::new(200.0, 100.0));
    assert_eq!(ui.cursor(), CursorIcon::RowResize);
    hover(&mut ui, Point::new(100.0, 100.0));
    assert_eq!(ui.cursor(), CursorIcon::Pointer);
    hover(&mut ui, Point::new(500.0, 350.0));
    assert_eq!(ui.cursor(), CursorIcon::Default, "nothing to do out there");
    // A quarter turn swaps the two sides.
    value.set(BoxTransform::new(BOX).rotated(90.0));
    settle(&mut ui);
    let right = value.get().handle(BoxHandle::Right);
    hover(&mut ui, right);
    assert_eq!(ui.cursor(), CursorIcon::RowResize);
}

#[test]
fn the_keyboard_moves_resizes_and_turns() {
    let value = boxed(BOX);
    let commits = State::new(0);
    let counted = commits.clone();
    let mut ui = ui(TransformHandles::new(value.binding())
        .on_commit(move |_| counted.update(|c| *c += 1))
        .size(600.0, 400.0));
    press(&mut ui, Key::Tab, Modifiers::default());
    press(&mut ui, Key::ArrowRight, Modifiers::default());
    assert_eq!(value.get().rect.x(), 101.0);
    press(&mut ui, Key::ArrowDown, shift());
    assert_eq!(value.get().rect.y(), 110.0);
    press(&mut ui, Key::ArrowRight, alt());
    assert_eq!(value.get().rect, Rect::new(101.0, 110.0, 201.0, 100.0));
    press(&mut ui, Key::ArrowUp, alt());
    assert_eq!(value.get().rect, Rect::new(101.0, 110.0, 201.0, 99.0));
    press(&mut ui, Key::Character("]".into()), Modifiers::default());
    assert_eq!(value.get().angle, 1.0);
    press(&mut ui, Key::Character("[".into()), shift());
    assert_eq!(value.get().angle, -14.0);
    assert_eq!(commits.get(), 6);
    let group = node(&mut ui, Role::Group);
    assert_eq!(group.name.as_deref(), Some("Transform box"));
    assert!(group.value.is_some_and(|v| v.contains("201 × 99") && v.contains("-14")));
}

#[test]
fn escape_puts_a_dragged_box_back() {
    let value = boxed(BOX);
    let mut ui = ui(TransformHandles::new(value.binding()).size(600.0, 400.0));
    down(&mut ui, Point::new(150.0, 130.0));
    drag_to(&mut ui, Point::new(250.0, 230.0));
    assert_ne!(value.get().rect, BOX);
    press(&mut ui, Key::Escape, Modifiers::default());
    assert_eq!(value.get().rect, BOX);
    drag_to(&mut ui, Point::new(10.0, 10.0));
    assert_eq!(value.get().rect, BOX);
    up(&mut ui, Point::new(10.0, 10.0));
}

#[test]
fn a_confined_box_stays_in_the_content() {
    let value = boxed(BOX);
    let mut ui = ui(TransformHandles::new(value.binding())
        .confined(true)
        .keep_proportions(false)
        .rotatable(false)
        .pivot_handle(false)
        .size(400.0, 300.0));
    drag(&mut ui, Point::new(150.0, 130.0), Point::new(900.0, 130.0));
    assert_eq!(value.get().rect.max_x(), 400.0, "moving stops at the edge");
    // A corner past one edge still follows along the other.
    value.set(BoxTransform::new(BOX));
    settle(&mut ui);
    drag(&mut ui, Point::new(300.0, 200.0), Point::new(900.0, 250.0));
    let rect = value.get().rect;
    assert!(near(rect.max_x(), 400.0) && near(rect.max_y(), 250.0), "{rect:?}");
}

#[test]
fn a_box_has_a_least_size() {
    let value = boxed(BOX);
    let mut ui =
        ui(TransformHandles::new(value.binding()).keep_proportions(false).min_size(30.0).size(600.0, 400.0));
    drag(&mut ui, Point::new(300.0, 150.0), Point::new(10.0, 150.0));
    assert_eq!(value.get().rect, Rect::new(100.0, 100.0, 30.0, 100.0), "stops short and never flips");
}

#[test]
fn handles_follow_a_view_in_content_units() {
    let view = State::new(ZoomView::at(2.0));
    let content = Size::new(200.0, 200.0);
    let value = boxed(Rect::new(50.0, 50.0, 100.0, 100.0));
    let mut ui = ui(TransformHandles::new(value.binding())
        .keep_proportions(false)
        .space(ContentSpace::View(view.binding(), content))
        .size(400.0, 400.0));
    // At twice the size in a box of 400, a unit is two pixels from the corner.
    drag(&mut ui, Point::new(300.0, 200.0), Point::new(340.0, 200.0));
    assert_eq!(value.get().rect, Rect::new(50.0, 50.0, 120.0, 100.0), "40 pixels are 20 units");
}

#[test]
fn a_drag_of_handles_costs_no_builds_when_nobody_reads_it() {
    let value = boxed(BOX);
    let mut ui = ui(TransformHandles::new(value.binding()).size(600.0, 400.0));
    down(&mut ui, Point::new(150.0, 130.0));
    settle(&mut ui);
    let before = ui.stats().builds;
    for i in 0..5 {
        ui.set_time(f64::from(i + 1));
        drag_to(&mut ui, Point::new(160.0 + i as f32 * 10.0, 130.0));
    }
    assert_eq!(ui.stats().builds, before, "a drag only repaints");
    assert_eq!(value.get().rect.x(), 150.0);
    assert!(quads(&ui).iter().any(|r| near(r.x(), 150.0) && near(r.width(), 200.0)), "and the box moved");
    up(&mut ui, Point::new(200.0, 130.0));
}

#[test]
fn a_disabled_box_takes_nothing() {
    let value = boxed(BOX);
    let mut ui = ui(TransformHandles::new(value.binding()).disabled(true).size(600.0, 400.0));
    drag(&mut ui, Point::new(150.0, 130.0), Point::new(170.0, 130.0));
    assert_eq!(value.get().rect, BOX);
}

// -- Protractor --------------------------------------------------------------------------------

#[test]
fn a_protractor_follows_a_drag_from_anywhere() {
    let angle = State::new(5.0_f32);
    let commits = State::new(Vec::new());
    let told = commits.clone();
    let mut ui = ui(Protractor::new(angle.binding())
        .on_commit(move |a| told.update(|c: &mut Vec<f32>| c.push(a)))
        .width(278.0));
    // 278 wide with 14 of margin each side is 250 pixels for 90 degrees.
    let per_degree = 250.0 / 90.0;
    down(&mut ui, Point::new(30.0, 20.0));
    drag_to(&mut ui, Point::new(30.0 + per_degree * 10.0, 20.0));
    assert!((angle.get() - 15.0).abs() < 0.11, "relative to where it began: {}", angle.get());
    assert!(commits.get().is_empty());
    up(&mut ui, Point::new(30.0 + per_degree * 10.0, 20.0));
    assert_eq!(commits.get().len(), 1, "one step to undo");
    // It stops at the end of the range.
    drag(&mut ui, Point::new(30.0, 20.0), Point::new(780.0, 20.0));
    assert_eq!(angle.get(), 45.0);
    hover(&mut ui, Point::new(30.0, 20.0));
    assert_eq!(ui.cursor(), CursorIcon::ColResize);
}

#[test]
fn a_double_click_zeroes_a_protractor() {
    let angle = State::new(12.5_f32);
    let mut ui = ui(Protractor::new(angle.binding()));
    ui.handle_event(&Event::DoubleClick { position: Point::new(30.0, 20.0) });
    settle(&mut ui);
    assert_eq!(angle.get(), 0.0);
    assert!(angle.get().is_sign_positive());
}

#[test]
fn a_protractor_works_from_the_keyboard_and_says_its_value() {
    let angle = State::new(0.0_f32);
    let mut ui = ui(Protractor::new(angle.binding()).range(-10.0, 20.0));
    press(&mut ui, Key::Tab, Modifiers::default());
    press(&mut ui, Key::ArrowRight, Modifiers::default());
    assert!((angle.get() - 0.1).abs() < 0.001);
    press(&mut ui, Key::ArrowLeft, shift());
    assert!((angle.get() + 0.9).abs() < 0.001);
    let slider = node(&mut ui, Role::Slider);
    assert_eq!(slider.name.as_deref(), Some("Angle"));
    assert_eq!(slider.value.as_deref(), Some("-0.9°"));
    press(&mut ui, Key::End, Modifiers::default());
    assert_eq!(angle.get(), 20.0);
    press(&mut ui, Key::Home, Modifiers::default());
    assert_eq!(angle.get(), -10.0);
    press(&mut ui, Key::Delete, Modifiers::default());
    assert_eq!(angle.get(), 0.0);
    assert_eq!(node(&mut ui, Role::Slider).value.as_deref(), Some("0.0°"), "no sign, and never -0.0");
}

#[test]
fn a_disabled_protractor_stays_put() {
    let angle = State::new(3.0_f32);
    let mut ui = ui(Protractor::new(angle.binding()).disabled(true));
    drag(&mut ui, Point::new(30.0, 20.0), Point::new(130.0, 20.0));
    assert_eq!(angle.get(), 3.0);
    assert!(paths(&ui) > 0, "it still shows the angle");
}
