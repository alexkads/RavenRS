//! `FlowCanvas` as a diagram editor draws it (#600): connections that join the shapes
//! themselves, labels typed and slid on the line, the context menu and the line's style.
#![cfg(feature = "flow")]
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_core::{AccessNode, Event, Key, Modifiers, Point, PointerButton, Size};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::*;

/// Two boxes with no ports, side by side, the first at (100, 100).
fn boxes() -> FlowGraph {
    FlowGraph::new()
        .node(FlowNode::new("a", "", "Order", 100.0, 100.0).size(120.0, 60.0).shape(FlowShape::Rectangle))
        .node(FlowNode::new("b", "", "Invoice", 500.0, 100.0).size(120.0, 60.0).shape(FlowShape::Rectangle))
}

struct Rig {
    ui: Ui,
    graph: State<FlowGraph>,
    selection: State<FlowSelection>,
}

fn rig(graph: FlowGraph, tweak: impl FnOnce(FlowCanvas) -> FlowCanvas) -> Rig {
    let graph = State::new(graph);
    let view = State::new(FlowViewport::at(Point::new(0.0, 0.0), 1.0));
    let selection = State::new(FlowSelection::default());
    let canvas =
        FlowCanvas::new(graph.binding()).view(view.binding()).selection(selection.binding()).size(900.0, 600.0);
    let mut ui = Ui::new(tweak(canvas), Theme::raven_dark());
    ui.set_viewport(Size::new(900.0, 600.0), 1.0);
    settle(&mut ui);
    Rig { ui, graph, selection }
}

fn settle(ui: &mut Ui) {
    ui.update();
    ui.update();
}

fn hover(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    settle(ui);
}

fn drag(ui: &mut Ui, from: Point, to: Point) {
    hover(ui, from);
    ui.handle_event(&Event::PointerDown { position: from, button: PointerButton::Primary });
    settle(ui);
    for i in 1..=4 {
        let t = i as f32 / 4.0;
        hover(ui, Point::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t));
    }
    ui.handle_event(&Event::PointerUp { position: to, button: PointerButton::Primary });
    settle(ui);
}

fn click(ui: &mut Ui, at: Point) {
    drag(ui, at, at);
}

fn double_click(ui: &mut Ui, at: Point) {
    click(ui, at);
    ui.handle_event(&Event::DoubleClick { position: at });
    settle(ui);
}

fn right_click(ui: &mut Ui, at: Point) {
    hover(ui, at);
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Secondary });
    settle(ui);
}

fn key(ui: &mut Ui, key: Key, modifiers: Modifiers) {
    ui.handle_event(&Event::KeyDown { key, modifiers });
    settle(ui);
}

fn type_text(ui: &mut Ui, text: &str) {
    for letter in text.chars() {
        key(ui, Key::Character(letter.to_string()), Modifiers::default());
    }
}

fn hold_shift(ui: &mut Ui, held: bool) {
    let modifiers = Modifiers { shift: held, ..Modifiers::default() };
    ui.handle_event(&Event::KeyUp { key: Key::Character("x".into()), modifiers });
}

fn undo(ui: &mut Ui) {
    key(ui, Key::Character("z".into()), Modifiers { meta: true, ..Modifiers::default() });
}

fn named(ui: &mut Ui, name: &str) -> Option<AccessNode> {
    ui.access_tree().nodes.iter().find(|n| n.name.as_deref() == Some(name)).cloned()
}

fn every(ui: &mut Ui, name: &str) -> Vec<AccessNode> {
    ui.access_tree().nodes.iter().filter(|n| n.name.as_deref() == Some(name)).cloned().collect()
}

fn field(ui: &mut Ui) -> Option<AccessNode> {
    ui.access_tree().nodes.iter().find(|n| n.role == ravenrs_core::Role::TextInput).cloned()
}

#[test]
fn a_shape_with_no_ports_offers_handles_and_a_drag_joins_it_to_another() {
    let mut rig = rig(boxes(), |c| c.connect_anywhere(true));
    assert!(named(&mut rig.ui, "Connect from Order").is_none(), "no handles until the pointer is there");
    hover(&mut rig.ui, Point::new(160.0, 130.0));
    assert_eq!(every(&mut rig.ui, "Connect from Order").len(), 4, "one on each side");
    // From the right handle onto the body of the other box.
    drag(&mut rig.ui, Point::new(220.0, 130.0), Point::new(560.0, 140.0));
    let g = rig.graph.get();
    assert_eq!(g.edges.len(), 1, "connected");
    let edge = &g.edges[0];
    assert_eq!((&edge.from, &edge.to), (&FlowEndpoint::floating("a"), &FlowEndpoint::floating("b")));
    assert_eq!(rig.selection.get(), FlowSelection::edge(edge.id.clone()));
    assert!(g.check().is_ok());
    undo(&mut rig.ui);
    assert!(rig.graph.get().edges.is_empty(), "one change to undo");
}

#[test]
fn with_shift_the_end_holds_where_it_was_let_go_and_a_shape_does_not_join_itself() {
    let mut rig = rig(boxes(), |c| c.connect_anywhere(true));
    hover(&mut rig.ui, Point::new(160.0, 130.0));
    hold_shift(&mut rig.ui, true);
    drag(&mut rig.ui, Point::new(220.0, 130.0), Point::new(500.0 + 30.0, 100.0 + 6.0));
    let g = rig.graph.get();
    let to = &g.edges.first().expect("connected").to;
    assert_eq!(to, &FlowEndpoint::fixed("b", 0.3, 0.1));
    hold_shift(&mut rig.ui, false);
    // Back onto itself: nothing.
    hover(&mut rig.ui, Point::new(160.0, 130.0));
    drag(&mut rig.ui, Point::new(220.0, 130.0), Point::new(140.0, 120.0));
    assert_eq!(rig.graph.get().edges.len(), 1);
}

#[test]
fn without_connect_anywhere_or_when_locked_there_are_no_handles() {
    let mut plain = rig(boxes(), |c| c);
    hover(&mut plain.ui, Point::new(160.0, 130.0));
    assert!(named(&mut plain.ui, "Connect from Order").is_none());
    let mut locked = rig(boxes(), |c| c.connect_anywhere(true).locked(true));
    hover(&mut locked.ui, Point::new(160.0, 130.0));
    assert!(named(&mut locked.ui, "Connect from Order").is_none());
}

fn labelled() -> FlowGraph {
    boxes().edge(
        FlowEdge::new("e1", FlowEndpoint::floating("a"), FlowEndpoint::floating("b"))
            .shape(EdgeShape::Straight)
            .label("sends"),
    )
}

#[test]
fn a_double_click_on_a_label_types_it_in_place_and_escape_keeps_the_old_one() {
    let mut rig = rig(labelled(), |c| c);
    let label = named(&mut rig.ui, "sends").expect("the label").bounds.center();
    double_click(&mut rig.ui, label);
    let open = field(&mut rig.ui).expect("the label field");
    assert_eq!(open.value.as_deref(), Some("sends"));
    type_text(&mut rig.ui, "bills");
    key(&mut rig.ui, Key::Enter, Modifiers::default());
    assert_eq!(rig.graph.get().edges[0].label.as_deref(), Some("bills"));
    assert!(field(&mut rig.ui).is_none(), "closed");
    undo(&mut rig.ui);
    assert_eq!(rig.graph.get().edges[0].label.as_deref(), Some("sends"), "one change");

    // F2 with the line selected opens it too; Escape leaves it as it was.
    click(&mut rig.ui, Point::new(300.0, 130.0));
    assert_eq!(rig.selection.get(), FlowSelection::edge("e1"));
    key(&mut rig.ui, Key::Function(2), Modifiers::default());
    assert!(field(&mut rig.ui).is_some());
    type_text(&mut rig.ui, "gone");
    key(&mut rig.ui, Key::Escape, Modifiers::default());
    assert_eq!(rig.graph.get().edges[0].label.as_deref(), Some("sends"));
}

#[test]
fn an_emptied_label_goes_and_a_label_slides_along_its_line() {
    let mut rig = rig(labelled(), |c| c);
    let label = named(&mut rig.ui, "sends").expect("the label").bounds.center();
    // The line runs from (220, 130) to (500, 130): the label starts at its middle.
    assert!((label.x - 360.0).abs() < 1.0, "{label:?}");
    drag(&mut rig.ui, label, Point::new(290.0, 140.0));
    let at = rig.graph.get().edges[0].label_at;
    assert!((at - 0.25).abs() < 0.02, "{at}");
    let moved = named(&mut rig.ui, "sends").expect("still there").bounds.center();
    assert!((moved.x - 290.0).abs() < 2.0, "{moved:?}");
    undo(&mut rig.ui);
    assert_eq!(rig.graph.get().edges[0].label_at, 0.5, "one change");

    double_click(&mut rig.ui, label);
    key(&mut rig.ui, Key::Backspace, Modifiers::default());
    key(&mut rig.ui, Key::Enter, Modifiers::default());
    assert_eq!(rig.graph.get().edges[0].label, None);
}

fn menu_item(ui: &mut Ui, name: &str) -> AccessNode {
    named(ui, name).unwrap_or_else(|| panic!("the menu has {name}"))
}

fn choose(ui: &mut Ui, name: &str) {
    let id = menu_item(ui, name).id;
    ui.perform_action(id, ravenrs_core::AccessAction::Click);
    settle(ui);
}

#[test]
fn a_right_click_selects_the_shape_and_its_menu_acts_on_it() {
    let picked = Rc::new(RefCell::new(None::<FlowSelection>));
    let seen = picked.clone();
    let mut rig = rig(labelled(), move |c| c.menu_item("Inspect", move |s| *seen.borrow_mut() = Some(s.clone())));
    right_click(&mut rig.ui, Point::new(560.0, 130.0));
    assert_eq!(rig.selection.get(), FlowSelection::node("b"), "what was clicked is selected");
    for name in ["Cut", "Copy", "Paste", "Duplicate", "Bring to front", "Send to back", "Select all"] {
        menu_item(&mut rig.ui, name);
    }
    assert!(menu_item(&mut rig.ui, "Group").disabled, "one shape is not a group");
    choose(&mut rig.ui, "Delete");
    let g = rig.graph.get();
    assert!(g.find_node("b").is_none() && g.edges.is_empty(), "the shape went with its line");
    undo(&mut rig.ui);
    assert!(rig.graph.get().find_node("b").is_some());

    right_click(&mut rig.ui, Point::new(160.0, 130.0));
    choose(&mut rig.ui, "Duplicate");
    assert_eq!(rig.graph.get().nodes.len(), 3, "a copy beside it");
    right_click(&mut rig.ui, Point::new(160.0, 130.0));
    choose(&mut rig.ui, "Inspect");
    assert_eq!(picked.borrow().clone(), Some(FlowSelection::node("a")), "the app's item gets the selection");
}

#[test]
fn locked_the_menu_turns_off_what_edits_and_it_can_be_left_out() {
    let mut locked = rig(labelled(), |c| c.locked(true));
    right_click(&mut locked.ui, Point::new(160.0, 130.0));
    assert!(menu_item(&mut locked.ui, "Delete").disabled);
    assert!(menu_item(&mut locked.ui, "Paste").disabled);
    assert!(!menu_item(&mut locked.ui, "Copy").disabled, "copying edits nothing");
    let mut off = rig(labelled(), |c| c.context_menu(false));
    right_click(&mut off.ui, Point::new(160.0, 130.0));
    assert!(named(&mut off.ui, "Delete").is_none());
}

#[test]
fn line_styles_mix_and_the_picker_says_so() {
    let solid = LineStyle::new(1.5, LineDash::Solid, EdgeCap::None, EdgeCap::Arrow);
    let dotted = LineStyle::new(1.5, LineDash::Dot, EdgeCap::None, EdgeCap::CrowFoot);
    let shared = LineStyle::shared([solid, dotted]);
    assert_eq!(shared, LineStyle { width: Some(1.5), dash: None, start: Some(EdgeCap::None), end: None });
    assert_eq!(LineStyle::shared([]), LineStyle::default());

    let style = State::new(shared);
    let mut ui = Ui::new(LineStylePicker::new(style.binding()), Theme::raven_dark());
    ui.set_viewport(Size::new(400.0, 400.0), 1.0);
    settle(&mut ui);
    assert!(named(&mut ui, "Line style").is_some());
    for field in ["Width", "Pattern", "Start", "End"] {
        assert!(named(&mut ui, field).is_some(), "{field}");
    }
    let values: Vec<String> = ui.access_tree().nodes.iter().filter_map(|n| n.value.clone()).collect();
    assert_eq!(values.iter().filter(|v| v.as_str() == "Mixed").count(), 2, "{values:?}");

    let mut no_caps = Ui::new(LineStylePicker::new(style.binding()).caps(false), Theme::raven_dark());
    no_caps.set_viewport(Size::new(400.0, 400.0), 1.0);
    settle(&mut no_caps);
    assert!(named(&mut no_caps, "Start").is_none());
}

#[test]
fn the_inspector_edits_the_style_of_several_lines_at_once() {
    let graph = State::new(
        labelled()
            .node(FlowNode::new("c", "", "Receipt", 100.0, 400.0).size(120.0, 60.0))
            .edge(FlowEdge::new("e2", FlowEndpoint::floating("b"), FlowEndpoint::floating("c")).dash(LineDash::Dot)),
    );
    let selection = State::new(FlowSelection { nodes: Vec::new(), edges: vec!["e1".into(), "e2".into()] });
    let history = State::new(FlowHistory::new());
    let inspector = FlowInspector::new(graph.binding(), selection.binding()).history(history.binding());
    let mut ui = Ui::new(inspector, Theme::raven_dark());
    ui.set_viewport(Size::new(400.0, 800.0), 1.0);
    settle(&mut ui);
    assert!(named(&mut ui, "2 connections").is_some(), "the heading counts them");
    assert!(named(&mut ui, "Line style").is_some());
    let values: Vec<String> = ui.access_tree().nodes.iter().filter_map(|n| n.value.clone()).collect();
    assert!(values.iter().any(|v| v == "Mixed"), "the patterns differ: {values:?}");
}
