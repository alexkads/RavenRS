//! `Footer`: three areas that keep their places, stack when the window is
//! narrow, and show up as the page's landmark.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_core::{AccessNode, AccessTree, Event, Key, Modifiers, Rect, Role, Size};
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

fn sample() -> Footer {
    Footer::new()
        .left((Footer::text("A long copyright line on the left"),))
        .center((Footer::link("Privacy"),))
        .right((Footer::text("v1"),))
}

/// `VStack` stretches its children, as a page would.
fn page(footer: Footer) -> impl View {
    VStack::new((footer,)).align(ravenrs_layout::Align::Stretch)
}

#[test]
fn wide_windows_get_three_areas_with_a_truly_centered_center() {
    let mut ui = ui(page(sample()), 1000.0);
    let (left, center, right) = (
        bounds(&mut ui, "A long copyright line on the left"),
        bounds(&mut ui, "Privacy"),
        bounds(&mut ui, "v1"),
    );
    assert!((left.center().y - center.center().y).abs() < 1.0, "areas line up: {left:?} {center:?}");
    assert!(left.x() < 30.0, "left area starts at the left: {left:?}");
    assert!((center.center().x - 500.0).abs() < 1.0, "centered despite uneven sides: {center:?}");
    assert!((right.max_x() - 984.0).abs() < 1.0, "right area ends at the padding: {right:?}");
}

#[test]
fn narrow_windows_stack_the_areas_centered_in_reading_order() {
    let mut ui = ui(page(sample()), 400.0);
    let (left, center, right) = (
        bounds(&mut ui, "A long copyright line on the left"),
        bounds(&mut ui, "Privacy"),
        bounds(&mut ui, "v1"),
    );
    assert!(left.max_y() <= center.y() && center.max_y() <= right.y(), "{left:?} {center:?} {right:?}");
    for area in [left, center, right] {
        assert!((area.center().x - 200.0).abs() < 1.0, "centered: {area:?}");
        assert!(area.x() >= 0.0 && area.max_x() <= 400.0, "stays in the window: {area:?}");
    }
}

#[test]
fn the_breakpoint_and_stacked_choose_the_layout() {
    let is_stacked = |footer: Footer, width: f32| {
        let mut ui = ui(page(footer), width);
        bounds(&mut ui, "Privacy").max_y() <= bounds(&mut ui, "v1").y()
    };
    assert!(!is_stacked(sample(), 700.0));
    assert!(is_stacked(sample().breakpoint(800.0), 700.0));
    assert!(!is_stacked(sample().breakpoint(300.0), 400.0));
    assert!(is_stacked(sample().stacked(), 1000.0));
}

#[test]
fn a_missing_area_leaves_the_others_where_they_were() {
    let mut only_sides =
        ui(page(Footer::new().left((Footer::text("L"),)).right((Footer::text("R"),))), 1000.0);
    assert!(bounds(&mut only_sides, "L").x() < 30.0);
    assert!((bounds(&mut only_sides, "R").max_x() - 984.0).abs() < 1.0);

    let mut no_left =
        ui(page(Footer::new().center((Footer::text("C"),)).right((Footer::text("R"),))), 1000.0);
    assert!((bounds(&mut no_left, "C").center().x - 500.0).abs() < 1.0);
    assert!((bounds(&mut no_left, "R").max_x() - 984.0).abs() < 1.0);

    let mut only_right = ui(page(Footer::new().right((Footer::text("R"),))), 1000.0);
    assert!((bounds(&mut only_right, "R").max_x() - 984.0).abs() < 1.0);

    let mut only_center = ui(page(Footer::new().center((Footer::text("C"),))), 1000.0);
    assert!((bounds(&mut only_center, "C").center().x - 500.0).abs() < 1.0);
}

#[test]
fn it_stretches_across_a_page_that_does_not_stretch_its_children() {
    let mut ui = ui(VStack::new((sample(),)), 1000.0);
    let tree = ui.access_tree();
    let footer = tree.nodes.iter().find(|n| n.role == Role::ContentInfo).expect("landmark");
    assert_eq!(footer.bounds.width(), 1000.0);
}

#[test]
fn long_text_wraps_inside_its_column_and_many_items_wrap_in_their_area() {
    let long = "A very long copyright line that cannot possibly fit in one third of this window";
    let links: Vec<_> = (0..12).map(|i| Footer::link(format!("Link {i}"))).collect();
    let mut ui =
        ui(page(Footer::new().left((Footer::text(long),)).center(links).right((Footer::text("v1"),))), 700.0);
    let center = (0..12).map(|i| bounds(&mut ui, &format!("Link {i}"))).collect::<Vec<_>>();
    let text = bounds(&mut ui, long);
    assert!(text.max_x() <= center[0].x(), "text stays in its column: {text:?} {:?}", center[0]);
    assert!(center.iter().any(|b| b.y() > center[0].y()), "links move to a new line");
    for link in &center {
        assert!(
            link.x() >= 700.0 / 3.0 && link.max_x() <= 2.0 * 700.0 / 3.0 + 16.0,
            "in its column: {link:?}"
        );
    }
}

#[test]
fn the_divider_is_optional() {
    let separators = |footer: Footer| {
        let mut ui = ui(page(footer), 1000.0);
        ui.access_tree().nodes.iter().filter(|n| n.role == Role::Separator).count()
    };
    assert_eq!(separators(sample()), 1);
    assert_eq!(separators(sample().divider(false)), 0);
}

#[test]
fn screen_readers_find_the_landmark_with_its_content_inside() {
    let mut ui = ui(page(sample()), 1000.0);
    let tree = ui.access_tree();
    let footer = tree.nodes.iter().find(|n| n.role == Role::ContentInfo).expect("landmark");
    let link = node(&tree, "Privacy");
    assert_eq!(link.role, Role::Button);
    let mut parent = tree.nodes.iter().find(|n| n.children.contains(&link.id));
    while let Some(p) = parent {
        if p.id == footer.id {
            return;
        }
        parent = tree.nodes.iter().find(|n| n.children.contains(&p.id));
    }
    panic!("the link is not inside the landmark");
}

#[test]
fn links_work_with_the_keyboard() {
    let clicks = Rc::new(Cell::new(0));
    let counter = clicks.clone();
    let footer =
        Footer::new().center((Footer::link("Help").on_click(move || counter.set(counter.get() + 1)),));
    let mut ui = ui(page(footer), 1000.0);
    for key in [Key::Tab, Key::Enter] {
        ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers::default() });
        ui.update();
    }
    assert_eq!(clicks.get(), 1);
}
