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
