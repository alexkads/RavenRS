//! `StatusBar`: one fixed line whose center gives way, and its landmark.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_core::{AccessNode, AccessTree, Event, Point, PointerButton, Rect, Role, Size};
use ravenrs_style::Theme;
use ravenrs_widgets::*;

fn ui(root: impl View + 'static, width: f32) -> Ui {
    let mut ui = Ui::new(root, Theme::raven_dark());
    ui.set_viewport(Size::new(width, 600.0), 1.0);
    ui.update();
    ui
}

fn node<'a>(tree: &'a AccessTree, name: &str) -> &'a AccessNode {
    tree.nodes.iter().find(|n| n.name.as_deref() == Some(name)).expect(name)
}

fn bounds(ui: &mut Ui, name: &str) -> Rect {
    node(&ui.access_tree(), name).bounds
}

fn click(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
}

fn bar(notice: &str) -> impl View + use<> {
    VStack::new((
        Spacer::new(),
        StatusBar::new()
            .left((StatusBar::text("Version 1.2"),))
            .center((StatusBar::text(notice).truncate().grow(1.0), StatusBar::action("Update")))
            .right((StatusBar::text("12 items"),)),
    ))
    .spacing(0.0)
    .align(ravenrs_layout::Align::Stretch)
    .grow(1.0)
}

#[test]
fn it_is_one_short_line_whatever_the_width() {
    for width in [320.0, 800.0, 1600.0] {
        let mut ui = ui(bar("Version 1.3 is available"), width);
        let (left, right) = (bounds(&mut ui, "Version 1.2"), bounds(&mut ui, "12 items"));
        assert!((left.center().y - right.center().y).abs() < 2.0, "a stacked bar at {width}");
        let tree = ui.access_tree();
        let landmark = tree.nodes.iter().find(|n| n.role == Role::ContentInfo).expect("landmark");
        assert!(landmark.bounds.height() <= 30.0, "too tall at {width}: {:?}", landmark.bounds);
    }
}

#[test]
fn the_sides_stay_at_the_ends_and_a_long_notice_gives_way() {
    let long =
        "A very long notice about the version that does not fit at all in this narrow window ".repeat(4);
    let mut ui = ui(bar(&long), 640.0);
    let left = bounds(&mut ui, "Version 1.2");
    let right = bounds(&mut ui, "12 items");
    let action = bounds(&mut ui, "Update");
    assert!(left.x() < 20.0);
    assert!(right.max_x() > 620.0 && right.max_x() <= 640.0);
    assert!(action.max_x() <= right.x(), "the action stays visible: {action:?} {right:?}");
}

#[test]
fn its_actions_work_and_are_small_enough_for_the_line() {
    let clicks = Rc::new(Cell::new(0));
    let count = clicks.clone();
    let mut ui = ui(
        StatusBar::new()
            .left((StatusBar::action("Version 1.2").on_click(move || count.set(count.get() + 1)),)),
        800.0,
    );
    let action = bounds(&mut ui, "Version 1.2");
    assert!(action.height() <= 22.0);
    assert!(node(&ui.access_tree(), "Version 1.2").focusable);
    click(&mut ui, action.center());
    assert_eq!(clicks.get(), 1);
}

#[test]
fn only_a_center_with_something_in_it_is_announced() {
    let mut empty = ui(StatusBar::new().left((StatusBar::text("v1"),)).center_tone(Tone::Warning), 800.0);
    assert!(!empty.access_tree().nodes.iter().any(|n| n.role == Role::Status));
    let mut told = ui(StatusBar::new().center((StatusBar::text("Saved"),)), 800.0);
    assert!(told.access_tree().nodes.iter().any(|n| n.role == Role::Status));
}
