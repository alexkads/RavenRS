//! The fan of report pages: real pages, a lifted page under the pointer or focus, links.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use ravenrs_core::{AccessTree, Brush, Event, Key, Modifiers, Point, PointerButton, Primitive, Rect, Size};
use ravenrs_report::{ReportEngine, samples};
use ravenrs_report_viewer::PageFan;
use ravenrs_style::Theme;
use ravenrs_widgets::{Ui, VStack};

fn fan() -> PageFan {
    let mut engine = ReportEngine::new();
    let invoice = engine.render(&samples::invoice(), &samples::invoice_data());
    let customers = samples::customer_list();
    let data = customers.embedded_data().cloned().unwrap_or_default();
    let list = engine.render(&customers, &data);
    PageFan::new().page(&list, 0, "Customer list", "designer/#customers").page(
        &invoice,
        0,
        "Invoice",
        "designer/#invoice",
    )
}

fn ui() -> Ui {
    let mut ui = Ui::new(VStack::new((fan(),)), Theme::raven_dark());
    ui.set_viewport(Size::new(900.0, 700.0), 1.0);
    ui.set_time(0.0);
    ui.update();
    ui
}

fn names(ui: &mut Ui) -> Vec<String> {
    let tree: AccessTree = ui.access_tree();
    tree.nodes.iter().filter_map(|n| n.name.clone()).collect()
}

fn bounds(ui: &mut Ui, name: &str) -> Rect {
    let tree: AccessTree = ui.access_tree();
    tree.nodes.iter().find(|n| n.name.as_deref() == Some(name)).expect(name).bounds
}

/// How many pages are lifted with an accent ring, and how many are dimmed behind them.
fn lifted(ui: &Ui) -> (usize, usize) {
    let quads = || {
        ui.scene().primitives().iter().filter_map(|p| match p {
            Primitive::Quad(q) => Some(*q),
            _ => None,
        })
    };
    // The accent ring, not the keyboard's focus ring; it grows with the lifted page.
    let rings = quads()
        .filter(|q| q.border.is_some_and(|b| (1.9..2.4).contains(&b.width) && b.color.r < 0.45))
        .count();
    let dims = quads().filter(|q| matches!(q.fill, Brush::Solid(c) if (c.a - 0.38).abs() < 0.01)).count();
    (rings, dims)
}

fn settle(ui: &mut Ui) {
    for n in 1..30 {
        ui.set_time(f64::from(n) / 60.0);
        ui.update();
    }
}

#[test]
fn every_page_is_a_named_link() {
    let mut ui = ui();
    let names = names(&mut ui);
    assert!(names.iter().any(|n| n == "Invoice. Open in the designer"), "{names:?}");
    assert_eq!(lifted(&ui), (0, 0), "at rest no page is lifted");
    assert!(names.iter().any(|n| n == "Customer list. Open in the designer"));
}

#[test]
fn the_page_under_the_pointer_comes_forward_and_a_click_opens_it() {
    let mut ui = ui();
    // The invoice is the last page: it lies on top, on the right.
    let page = bounds(&mut ui, "Invoice. Open in the designer");
    let at = Point::new(page.max_x() - 20.0, page.center().y);
    ui.handle_event(&Event::PointerMove { position: at });
    ui.update();
    settle(&mut ui);
    assert_eq!(lifted(&ui), (1, 1), "one page lifts with its ring, the other steps back");
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
    assert_eq!(ui.links().opened(), ["designer/#invoice"]);
}

#[test]
fn keyboard_focus_brings_a_page_forward() {
    let mut ui = ui();
    ui.handle_event(&Event::KeyDown { key: Key::Tab, modifiers: Modifiers::default() });
    ui.update();
    ui.update();
    assert_eq!(lifted(&ui), (1, 1), "the focused page comes forward");
}
