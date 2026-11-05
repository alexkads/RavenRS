//! `SankeyChart`: its layout comes from `ravenrs-graphics`, so these check what the
//! component adds: what it paints, what it says and how it reaches the keyboard.
#![cfg(feature = "charts")]
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use ravenrs_core::{AccessNode, Key, Point, Primitive, Size};
use ravenrs_layout::Align;
use ravenrs_style::{Semantic, Theme};
use ravenrs_widgets::*;

fn budget() -> SankeyChart {
    SankeyChart::new()
        .alt("Budget")
        .flow("Salary", "Rent", 60.0)
        .flow("Salary", "Savings", 40.0)
        .flow("Savings", "Funds", 40.0)
}

fn ui(view: impl View + 'static) -> Ui {
    let mut ui = Ui::new(VStack::new((view,)).align(Align::Stretch), Theme::raven_dark());
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.update();
    ui.update();
    ui
}

fn node(ui: &mut Ui) -> AccessNode {
    let tree = ui.access_tree();
    tree.nodes.iter().find(|n| n.name.as_deref() == Some("Budget")).expect("the chart").clone()
}

fn paths(ui: &Ui) -> Vec<ravenrs_core::PathFill> {
    ui.scene()
        .primitives()
        .iter()
        .filter_map(|p| if let Primitive::Path(path) = p { Some(path.clone()) } else { None })
        .collect()
}

fn hover(ui: &mut Ui, x: f32, y: f32) {
    ui.handle_event(&ravenrs_core::Event::PointerMove { position: Point::new(x, y) });
    ui.update();
    ui.update();
}

fn press(ui: &mut Ui, key: Key) {
    ui.handle_event(&ravenrs_core::Event::KeyDown { key, modifiers: ravenrs_core::Modifiers::default() });
    ui.update();
    ui.update();
}

fn settle(ui: &mut Ui) {
    let mut time = 0.0;
    for _ in 0..120 {
        time += 1.0 / 30.0;
        ui.set_time(time);
        ui.update();
    }
}

fn stops(path: &ravenrs_core::PathFill) -> (f32, ravenrs_core::Color, ravenrs_core::Color) {
    let ravenrs_core::PathPaint::Linear { stops, .. } = &path.paint else { panic!("a gradient") };
    (stops[0].color.a, stops[0].color, stops[1].color)
}

#[test]
fn every_flow_is_a_ribbon_from_one_color_to_the_other_and_every_node_is_named() {
    let ui = ui(budget());
    let found = paths(&ui);
    assert_eq!(found.len(), 3);
    let (_, from, to) = stops(&found[0]);
    assert_ne!((from.r, from.g), (to.r, to.g));
    let glyphs = ui.scene().primitives().iter().filter(|p| matches!(p, Primitive::Glyphs(_))).count();
    assert_eq!(glyphs, 4, "the four nodes");
    assert!(!ui.scene().primitives().iter().any(|p| matches!(p, Primitive::Image(_))), "no picture to send");
}

#[test]
fn ribbons_can_keep_the_color_of_their_source() {
    let ui = ui(budget().no_gradient());
    let (_, from, to) = stops(&paths(&ui)[0]);
    assert_eq!(from, to);
}

#[test]
fn a_screen_reader_hears_every_flow() {
    let mut ui = ui(budget());
    let spoken = node(&mut ui).description.unwrap_or_default();
    assert_eq!(spoken, "Salary → Rent: 60; Salary → Savings: 40; Savings → Funds: 40");
}

#[test]
fn the_keyboard_walks_the_nodes_and_then_the_flows_and_says_each_one() {
    let mut ui = ui(budget());
    press(&mut ui, Key::Tab);
    press(&mut ui, Key::ArrowRight);
    let first = node(&mut ui).value.unwrap_or_default();
    assert_eq!(first, "Salary, → Rent: 60, → Savings: 40, Salary: 100");
    press(&mut ui, Key::End);
    assert_eq!(node(&mut ui).value.as_deref(), Some("Savings → Funds, Funds: 40"));
    press(&mut ui, Key::ArrowLeft);
    assert_eq!(node(&mut ui).value.as_deref(), Some("Salary → Savings, Savings: 40"));
    press(&mut ui, Key::Home);
    assert_eq!(node(&mut ui).value.as_deref(), Some(first.as_str()));
    press(&mut ui, Key::Escape);
    assert_eq!(node(&mut ui).value, None);
}

#[test]
fn the_pointer_finds_a_node_and_the_rest_fades() {
    let mut ui = ui(budget());
    let rest = stops(&paths(&ui)[2]).0;
    ui.set_time(0.0);
    ui.update();
    // The first column starts at the left edge of the chart.
    hover(&mut ui, 5.0, 150.0);
    settle(&mut ui);
    let value = node(&mut ui).value.unwrap_or_default();
    assert!(value.starts_with("Salary"), "{value}");
    let found = paths(&ui);
    assert!(stops(&found[0]).0 > rest, "what leaves it stands out");
    assert!(stops(&found[2]).0 < rest, "what does not fades");
}

#[test]
fn flows_in_a_circle_say_which_nodes_they_go_through_in_place_of_a_diagram() {
    let circle = || SankeyChart::new().flow("In", "A", 1.0).flow("A", "B", 1.0).flow("B", "A", 1.0);
    let mut plain = ui(circle());
    assert!(paths(&plain).is_empty());
    let tree = plain.access_tree();
    let said = tree.nodes.iter().filter_map(|n| n.name.as_deref()).find(|n| n.contains("circle"));
    assert_eq!(said, Some("These flows go in a circle: A → B → A"));
    let mut local = ui(Localized::new(Labels::pt_br(), circle()));
    let tree = local.access_tree();
    assert!(tree.nodes.iter().filter_map(|n| n.name.as_deref()).any(|n| n.starts_with("Estes fluxos")));
}

#[test]
fn a_flow_of_nothing_is_not_drawn_or_said() {
    let chart = SankeyChart::new().alt("Budget").flow("A", "B", 5.0).flow("A", "C", 0.0);
    let mut ui = ui(chart);
    assert_eq!(paths(&ui).len(), 1);
    assert_eq!(node(&mut ui).description.as_deref(), Some("A → B: 5"));
}

#[test]
fn still_it_asks_for_no_frames_and_a_rebuild_redoes_nothing_it_can_keep() {
    let mut ui = ui(budget());
    settle(&mut ui);
    assert!(!ui.needs_redraw());
    let first = paths(&ui)[0].elements.clone();
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.update();
    assert!(std::sync::Arc::ptr_eq(&paths(&ui)[0].elements, &first), "the same outlines");
}

#[test]
fn it_can_take_no_focus_and_colors_come_from_the_theme_when_asked() {
    let mut ui = ui(budget().hide_tooltip().node_color("Salary", Semantic::Success));
    press(&mut ui, Key::Tab);
    press(&mut ui, Key::ArrowRight);
    assert_eq!(node(&mut ui).value, None);
    let success = Theme::raven_dark().color(Semantic::Success);
    let (_, from, _) = stops(&paths(&ui)[0]);
    assert_eq!((from.r, from.g, from.b), (success.r, success.g, success.b));
}
