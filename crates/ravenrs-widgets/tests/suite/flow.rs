//! `FlowCanvas`: the lines' geometry is tested in `ravenrs-graphics`; these drive the canvas
//! with the pointer, the wheel and the keys, and check what it writes back and what it says.
#![cfg(feature = "flow")]
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use ravenrs_core::{AccessNode, Event, Key, Modifiers, Point, PointerButton, Role, Size};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::*;

/// A sale that is checked and then billed; `c` refuses anything but an approved order.
fn graph() -> FlowGraph {
    FlowGraph::new()
        .node(
            FlowNode::new("a", "trigger", "New sale", 48.0, 48.0)
                .subtitle("Register 1")
                .port(FlowPort::output("out").kind("sale")),
        )
        .node(
            FlowNode::new("b", "action", "Check stock", 408.0, 48.0)
                .port(FlowPort::input("in").kind("sale"))
                .port(FlowPort::output("out").kind("sale")),
        )
        .node(
            FlowNode::new("c", "action", "Send refusal", 408.0, 288.0)
                .status(FlowNodeStatus::Warning("No e-mail account".into()))
                .port(FlowPort::input("in").kind("refused")),
        )
}

struct Rig {
    ui: Ui,
    graph: State<FlowGraph>,
    view: State<FlowViewport>,
    selection: State<FlowSelection>,
}

fn rig_with(graph: FlowGraph, tweak: impl FnOnce(FlowCanvas) -> FlowCanvas) -> Rig {
    let graph = State::new(graph);
    let view = State::new(FlowViewport::at(Point::new(0.0, 0.0), 1.0));
    let selection = State::new(FlowSelection::default());
    let canvas = FlowCanvas::new(graph.binding())
        .view(view.binding())
        .selection(selection.binding())
        .kind("trigger", FlowKind::new(IconName::Zap, "Trigger").tone(ravenrs_style::Semantic::Warning))
        .kind("action", FlowKind::new(IconName::Package, "Action"))
        .validate(|g, from, to| {
            let (a, b) = (g.port(from).and_then(|p| p.kind.clone()), g.port(to).and_then(|p| p.kind.clone()));
            if a == b { Ok(()) } else { Err("Send refusal expects a refused order".into()) }
        })
        .size(900.0, 600.0);
    let mut ui = Ui::new(tweak(canvas), Theme::raven_dark());
    ui.set_viewport(Size::new(900.0, 600.0), 1.0);
    settle(&mut ui);
    Rig { ui, graph, view, selection }
}

fn rig() -> Rig {
    rig_with(graph(), |c| c)
}

fn settle(ui: &mut Ui) {
    ui.update();
    ui.update();
}

fn drag(ui: &mut Ui, from: Point, to: Point) {
    ui.handle_event(&Event::PointerMove { position: from });
    ui.handle_event(&Event::PointerDown { position: from, button: PointerButton::Primary });
    settle(ui);
    let steps = 4;
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let at = Point::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
        ui.handle_event(&Event::PointerMove { position: at });
        settle(ui);
    }
    ui.handle_event(&Event::PointerUp { position: to, button: PointerButton::Primary });
    settle(ui);
}

fn click(ui: &mut Ui, at: Point) {
    drag(ui, at, at);
}

fn key(ui: &mut Ui, key: Key, modifiers: Modifiers) {
    ui.handle_event(&Event::KeyDown { key, modifiers });
    settle(ui);
}

fn hold_shift(ui: &mut Ui, held: bool) {
    let modifiers = Modifiers { shift: held, ..Modifiers::default() };
    ui.handle_event(&Event::KeyUp { key: Key::Character("x".into()), modifiers });
}

fn named(ui: &mut Ui, name: &str) -> Option<AccessNode> {
    ui.access_tree().nodes.iter().find(|n| n.name.as_deref() == Some(name)).cloned()
}

fn node_at(rig: &Rig, id: &str) -> (f32, f32) {
    let g = rig.graph.get();
    let n = g.find_node(id).expect("the node");
    (n.x, n.y)
}

#[test]
fn nodes_are_buttons_that_say_what_they_are_and_how_they_are() {
    let mut rig = rig();
    let canvas = named(&mut rig.ui, "Flow diagram").expect("the canvas");
    assert_eq!(canvas.role, Role::Group);
    assert_eq!(canvas.description.as_deref(), Some("3 nodes, 0 connections"));
    let sale = named(&mut rig.ui, "New sale").expect("a node");
    assert_eq!(sale.role, Role::Button);
    assert!(sale.focusable);
    assert_eq!(sale.description.as_deref(), Some("Trigger. Register 1"));
    assert_eq!(sale.bounds.origin, Point::new(48.0, 48.0));
    let refusal = named(&mut rig.ui, "Send refusal").expect("a node");
    assert_eq!(refusal.description.as_deref(), Some("Action. No e-mail account"));
    for control in ["Zoom in", "Zoom out", "Fit", "Lock editing", "Navigator"] {
        assert!(named(&mut rig.ui, control).is_some(), "{control}");
    }
}

#[test]
fn a_fitted_view_shows_every_node() {
    let mut rig = rig();
    rig.view.set(FlowViewport::fit());
    settle(&mut rig.ui);
    let v = rig.view.get();
    assert!(!v.fit, "fitting happens once");
    assert!(v.zoom <= 1.0);
    let bounds = rig.graph.get().bounds().expect("nodes");
    let (a, b) = (v.to_screen(bounds.origin), v.to_screen(Point::new(bounds.max_x(), bounds.max_y())));
    assert!(a.x >= 0.0 && a.y >= 0.0 && b.x <= 900.0 && b.y <= 600.0, "{a:?} {b:?}");
}

#[test]
fn dragging_a_node_moves_it_on_the_grid_and_selects_it() {
    let mut rig = rig();
    drag(&mut rig.ui, Point::new(120.0, 70.0), Point::new(170.0, 122.0));
    assert_eq!(node_at(&rig, "a"), (96.0, 96.0), "50 and 52 units, snapped to 48");
    assert_eq!(rig.selection.get(), FlowSelection::node("a"));
    assert_eq!(named(&mut rig.ui, "New sale").and_then(|n| n.selected), Some(true));
}

#[test]
fn shift_adds_to_the_selection_and_moves_the_group() {
    let mut rig = rig();
    click(&mut rig.ui, Point::new(120.0, 70.0));
    hold_shift(&mut rig.ui, true);
    click(&mut rig.ui, Point::new(480.0, 70.0));
    hold_shift(&mut rig.ui, false);
    assert_eq!(rig.selection.get().nodes, vec!["a".to_owned(), "b".to_owned()]);
    drag(&mut rig.ui, Point::new(480.0, 70.0), Point::new(480.0, 118.0));
    assert_eq!((node_at(&rig, "a"), node_at(&rig, "b")), ((48.0, 96.0), (408.0, 96.0)));
}

#[test]
fn dragging_the_surface_pans_and_a_press_on_it_lets_go_of_the_selection() {
    let mut rig = rig();
    click(&mut rig.ui, Point::new(120.0, 70.0));
    drag(&mut rig.ui, Point::new(300.0, 500.0), Point::new(340.0, 470.0));
    assert_eq!(rig.view.get().offset, Point::new(40.0, -30.0));
    assert_eq!(rig.selection.get(), FlowSelection::node("a"), "a pan keeps the selection");
    click(&mut rig.ui, Point::new(300.0, 500.0));
    assert!(rig.selection.get().is_empty());
}

#[test]
fn the_wheel_pans_and_a_pinch_zooms_around_the_pointer() {
    let mut rig = rig();
    rig.ui
        .handle_event(&Event::Scroll { position: Point::new(300.0, 300.0), delta: Point::new(-10.0, -20.0) });
    settle(&mut rig.ui);
    assert_eq!(rig.view.get().offset, Point::new(-10.0, -20.0));
    let at = Point::new(200.0, 150.0);
    let under = rig.view.get().to_content(at);
    rig.ui.handle_event(&Event::PointerMove { position: at });
    rig.ui.handle_event(&Event::Pinch { position: at, delta: 0.5 });
    settle(&mut rig.ui);
    let v = rig.view.get();
    assert!((v.zoom - 1.5).abs() < 0.001);
    let still = v.to_screen(under);
    assert!((still.x - at.x).abs() < 0.01 && (still.y - at.y).abs() < 0.01, "{still:?}");
    rig.ui.handle_event(&Event::Pinch { position: at, delta: 5.0 });
    settle(&mut rig.ui);
    assert_eq!(rig.view.get().zoom, 2.0, "the most zoom");
}

#[test]
fn connecting_two_ports_adds_the_connection_and_selects_it() {
    let mut rig = rig();
    // From the output of `a` (right edge, halfway down) to the input of `b`.
    drag(&mut rig.ui, Point::new(248.0, 90.0), Point::new(410.0, 92.0));
    let g = rig.graph.get();
    assert_eq!(g.edges.len(), 1);
    assert_eq!(g.edges[0].from, FlowEndpoint::new("a", "out"));
    assert_eq!(g.edges[0].to, FlowEndpoint::new("b", "in"));
    assert_eq!(rig.selection.get(), FlowSelection::edge("e1"));
    // The same again is refused: the connection is already there.
    drag(&mut rig.ui, Point::new(248.0, 90.0), Point::new(410.0, 92.0));
    assert_eq!(rig.graph.get().edges.len(), 1);
}

#[test]
fn a_connection_can_start_at_the_input_too() {
    let mut rig = rig();
    drag(&mut rig.ui, Point::new(408.0, 90.0), Point::new(250.0, 92.0));
    let g = rig.graph.get();
    assert_eq!(g.edges.len(), 1);
    assert_eq!((g.edges[0].from.node.as_str(), g.edges[0].to.node.as_str()), ("a", "b"));
}

#[test]
fn a_refused_connection_says_why_and_is_not_made() {
    let mut rig = rig();
    let (from, over) = (Point::new(248.0, 90.0), Point::new(410.0, 332.0));
    rig.ui.handle_event(&Event::PointerMove { position: from });
    rig.ui.handle_event(&Event::PointerDown { position: from, button: PointerButton::Primary });
    settle(&mut rig.ui);
    rig.ui.handle_event(&Event::PointerMove { position: Point::new(330.0, 200.0) });
    settle(&mut rig.ui);
    rig.ui.handle_event(&Event::PointerMove { position: over });
    settle(&mut rig.ui);
    let status = named(&mut rig.ui, "Send refusal expects a refused order").expect("the reason");
    assert_eq!(status.role, Role::Status);
    rig.ui.handle_event(&Event::PointerUp { position: over, button: PointerButton::Primary });
    settle(&mut rig.ui);
    assert!(rig.graph.get().edges.is_empty());
    assert!(named(&mut rig.ui, "Send refusal expects a refused order").is_none());
}

#[test]
fn a_press_on_a_line_selects_it_and_delete_takes_it_out() {
    let mut rig = rig_with(
        graph().edge(
            FlowEdge::new("e1", FlowEndpoint::new("a", "out"), FlowEndpoint::new("b", "in"))
                .shape(EdgeShape::Straight)
                .label("Yes"),
        ),
        |c| c,
    );
    assert!(named(&mut rig.ui, "Yes").is_some(), "the label is text");
    click(&mut rig.ui, Point::new(300.0, 92.0));
    assert_eq!(rig.selection.get(), FlowSelection::edge("e1"));
    key(&mut rig.ui, Key::Delete, Modifiers::default());
    assert!(rig.graph.get().edges.is_empty());
    assert_eq!(rig.graph.get().nodes.len(), 3);
}

#[test]
fn deleting_a_node_takes_its_connections_with_it() {
    let mut rig = rig_with(
        graph().edge(FlowEdge::new("e1", FlowEndpoint::new("a", "out"), FlowEndpoint::new("b", "in"))),
        |c| c,
    );
    click(&mut rig.ui, Point::new(480.0, 70.0));
    key(&mut rig.ui, Key::Backspace, Modifiers::default());
    let g = rig.graph.get();
    assert!(g.find_node("b").is_none());
    assert!(g.edges.is_empty());
}

#[test]
fn shift_dragging_the_surface_selects_with_a_rectangle() {
    let mut rig = rig();
    hold_shift(&mut rig.ui, true);
    drag(&mut rig.ui, Point::new(380.0, 20.0), Point::new(700.0, 420.0));
    hold_shift(&mut rig.ui, false);
    assert_eq!(rig.selection.get().nodes, vec!["b".to_owned(), "c".to_owned()]);
    assert_eq!(rig.view.get().offset, Point::new(0.0, 0.0), "no pan");
}

#[test]
fn the_keys_move_zoom_and_let_go() {
    let mut rig = rig();
    click(&mut rig.ui, Point::new(120.0, 70.0));
    key(&mut rig.ui, Key::ArrowRight, Modifiers::default());
    key(&mut rig.ui, Key::ArrowDown, Modifiers { shift: true, ..Modifiers::default() });
    assert_eq!(node_at(&rig, "a"), (72.0, 144.0));
    key(&mut rig.ui, Key::Character("+".into()), Modifiers::default());
    assert!((rig.view.get().zoom - 1.25).abs() < 0.001);
    key(&mut rig.ui, Key::Escape, Modifiers::default());
    assert!(rig.selection.get().is_empty());
    key(&mut rig.ui, Key::Character("a".into()), Modifiers { meta: true, ..Modifiers::default() });
    assert_eq!(rig.selection.get().nodes.len(), 3);
}

#[test]
fn a_locked_canvas_pans_but_changes_nothing() {
    let mut rig = rig_with(graph(), |c| c.locked(true));
    assert!(named(&mut rig.ui, "Unlock editing").is_some());
    drag(&mut rig.ui, Point::new(120.0, 70.0), Point::new(220.0, 170.0));
    assert_eq!(node_at(&rig, "a"), (48.0, 48.0));
    drag(&mut rig.ui, Point::new(248.0, 90.0), Point::new(410.0, 92.0));
    assert!(rig.graph.get().edges.is_empty());
    key(&mut rig.ui, Key::Delete, Modifiers::default());
    assert_eq!(rig.graph.get().nodes.len(), 3);
    drag(&mut rig.ui, Point::new(300.0, 500.0), Point::new(310.0, 500.0));
    assert_eq!(rig.view.get().offset, Point::new(10.0, 0.0));
}

#[test]
fn the_zoom_buttons_and_the_map_move_the_view() {
    let mut rig = rig();
    let zoom_in = named(&mut rig.ui, "Zoom in").expect("the button").bounds.center();
    click(&mut rig.ui, zoom_in);
    assert!((rig.view.get().zoom - 1.25).abs() < 0.001);
    let map = named(&mut rig.ui, "Navigator").expect("the map").bounds;
    let before = rig.view.get().offset;
    click(&mut rig.ui, Point::new(map.x() + 10.0, map.y() + 10.0));
    assert_ne!(rig.view.get().offset, before);
}

/// The outlines the canvas drew, which is where the lines are.
fn outlines(ui: &Ui) -> Vec<ravenrs_core::PathFill> {
    ui.scene()
        .primitives()
        .iter()
        .filter_map(|p| if let ravenrs_core::Primitive::Path(path) = p { Some(path.clone()) } else { None })
        .collect()
}

#[test]
fn an_animated_connection_travels_until_motion_is_reduced() {
    let animated = || {
        graph().edge(
            FlowEdge::new("e1", FlowEndpoint::new("a", "out"), FlowEndpoint::new("b", "in")).animated(true),
        )
    };
    let mut rig = rig_with(animated(), |c| c);
    rig.ui.set_time(1.0);
    settle(&mut rig.ui);
    let before = outlines(&rig.ui);
    rig.ui.set_time(1.25);
    settle(&mut rig.ui);
    assert!(!before.is_empty());
    assert_ne!(outlines(&rig.ui), before, "the dashes moved without a build");

    let mut still = rig_with(animated(), |c| c);
    still.ui.set_reduced_motion(true);
    settle(&mut still.ui);
    still.ui.set_time(1.0);
    settle(&mut still.ui);
    let before = outlines(&still.ui);
    still.ui.set_time(1.25);
    settle(&mut still.ui);
    assert_eq!(outlines(&still.ui), before);
}

fn command(shift: bool) -> Modifiers {
    Modifiers { meta: true, shift, ..Modifiers::default() }
}

/// Presses a shortcut with Cmd (and Shift).
fn shortcut(ui: &mut Ui, letter: &str, shift: bool) {
    key(ui, Key::Character(letter.into()), command(shift));
}

/// Presses Tab until the focus is on what screen readers describe as `description`.
fn tab_to(ui: &mut Ui, description: &str) {
    for _ in 0..40 {
        let tree = ui.access_tree();
        let focused = tree.focus.and_then(|id| tree.get(id)).and_then(|n| n.description.clone());
        if focused.as_deref() == Some(description) {
            return;
        }
        key(ui, Key::Tab, Modifiers::default());
    }
    panic!("no focus reaches {description}");
}

#[test]
fn a_move_is_one_change_to_undo_and_redo() {
    let mut rig = rig();
    drag(&mut rig.ui, Point::new(120.0, 70.0), Point::new(170.0, 122.0));
    assert_eq!(node_at(&rig, "a"), (96.0, 96.0));
    shortcut(&mut rig.ui, "z", false);
    assert_eq!(node_at(&rig, "a"), (48.0, 48.0), "the four steps of the pointer undo at once");
    shortcut(&mut rig.ui, "z", true);
    assert_eq!(node_at(&rig, "a"), (96.0, 96.0));
    shortcut(&mut rig.ui, "z", false);
    shortcut(&mut rig.ui, "y", false);
    assert_eq!(node_at(&rig, "a"), (96.0, 96.0), "Cmd+Y redoes too");
}

#[test]
fn undo_brings_back_a_connection_and_a_deleted_node() {
    let history = State::new(FlowHistory::new());
    let mut rig = rig_with(graph(), |c| c.history(history.binding()));
    drag(&mut rig.ui, Point::new(248.0, 90.0), Point::new(410.0, 92.0));
    assert_eq!(rig.graph.get().edges.len(), 1);
    click(&mut rig.ui, Point::new(480.0, 70.0));
    key(&mut rig.ui, Key::Delete, Modifiers::default());
    assert!(rig.graph.get().find_node("b").is_none());
    assert!(history.get().can_undo() && !history.get().can_redo(), "the app sees the history");
    shortcut(&mut rig.ui, "z", false);
    let g = rig.graph.get();
    assert!(g.find_node("b").is_some());
    assert_eq!(g.edges.len(), 1, "the connection came back with its node");
    shortcut(&mut rig.ui, "z", false);
    assert!(rig.graph.get().edges.is_empty());
    assert!(!history.get().can_undo() && history.get().can_redo());
}

#[test]
fn a_locked_canvas_neither_undoes_nor_pastes() {
    let mut rig = rig();
    drag(&mut rig.ui, Point::new(120.0, 70.0), Point::new(170.0, 122.0));
    shortcut(&mut rig.ui, "c", false);
    let lock = named(&mut rig.ui, "Lock editing").expect("the lock").bounds.center();
    click(&mut rig.ui, lock);
    click(&mut rig.ui, Point::new(170.0, 120.0));
    shortcut(&mut rig.ui, "z", false);
    shortcut(&mut rig.ui, "v", false);
    assert_eq!(node_at(&rig, "a"), (96.0, 96.0));
    assert_eq!(rig.graph.get().nodes.len(), 3);
}

#[test]
fn copied_nodes_paste_with_new_ids_a_little_further_each_time() {
    let mut rig = rig_with(
        graph().edge(FlowEdge::new("e1", FlowEndpoint::new("a", "out"), FlowEndpoint::new("b", "in"))),
        |c| c,
    );
    click(&mut rig.ui, Point::new(120.0, 70.0));
    hold_shift(&mut rig.ui, true);
    click(&mut rig.ui, Point::new(480.0, 70.0));
    hold_shift(&mut rig.ui, false);
    shortcut(&mut rig.ui, "c", false);
    let copied = rig.ui.clipboard().read().expect("the clipboard has the nodes");
    let piece = FlowGraph::from_json(&copied).expect("JSON of a diagram");
    assert_eq!((piece.nodes.len(), piece.edges.len()), (2, 1));

    shortcut(&mut rig.ui, "v", false);
    let g = rig.graph.get();
    assert_eq!(g.nodes.len(), 5);
    assert_eq!(g.find_node("a-2").map(|n| (n.x, n.y)), Some((96.0, 96.0)), "two grid steps along");
    let copy = g.find_edge("e2").expect("the connection came along");
    assert_eq!((copy.from.node.as_str(), copy.to.node.as_str()), ("a-2", "b-2"));
    assert_eq!(rig.selection.get().nodes, vec!["a-2".to_owned(), "b-2".to_owned()]);
    assert_eq!(rig.selection.get().edges, vec!["e2".to_owned()]);

    shortcut(&mut rig.ui, "v", false);
    assert_eq!(rig.graph.get().find_node("a-3").map(|n| (n.x, n.y)), Some((144.0, 144.0)));
    shortcut(&mut rig.ui, "z", false);
    shortcut(&mut rig.ui, "z", false);
    assert_eq!(rig.graph.get().nodes.len(), 3, "each paste is one change");
}

#[test]
fn a_cut_node_pastes_back_where_it_was() {
    let mut rig = rig();
    click(&mut rig.ui, Point::new(120.0, 70.0));
    shortcut(&mut rig.ui, "x", false);
    assert!(rig.graph.get().find_node("a").is_none());
    // The focus went with the node, and came back to the canvas: no click needed.
    shortcut(&mut rig.ui, "v", false);
    assert_eq!(node_at(&rig, "a"), (48.0, 48.0), "the id is free again, and so is the place");
    // Text that is not a diagram pastes nothing.
    rig.ui.clipboard().write("hello");
    shortcut(&mut rig.ui, "v", false);
    assert_eq!(rig.graph.get().nodes.len(), 3);
}

#[test]
fn dragging_the_end_of_a_selected_connection_moves_it_to_another_port() {
    let mut rig = rig_with(
        graph()
            .node(
                FlowNode::new("d", "action", "Count items", 48.0, 288.0)
                    .port(FlowPort::input("in").kind("sale")),
            )
            .edge(
                FlowEdge::new("e1", FlowEndpoint::new("a", "out"), FlowEndpoint::new("b", "in"))
                    .shape(EdgeShape::Straight),
            ),
        |c| c,
    );
    click(&mut rig.ui, Point::new(300.0, 92.0));
    assert_eq!(rig.selection.get(), FlowSelection::edge("e1"));
    // A refused port leaves the connection as it was.
    drag(&mut rig.ui, Point::new(408.0, 90.0), Point::new(410.0, 332.0));
    assert_eq!(rig.graph.get().edges[0].to, FlowEndpoint::new("b", "in"));
    drag(&mut rig.ui, Point::new(408.0, 90.0), Point::new(50.0, 330.0));
    let g = rig.graph.get();
    assert_eq!(g.edges.len(), 1, "moved, not added");
    assert_eq!(
        (g.edges[0].from.clone(), g.edges[0].to.clone()),
        (FlowEndpoint::new("a", "out"), FlowEndpoint::new("d", "in"))
    );
    assert_eq!(rig.selection.get(), FlowSelection::edge("e1"));
    shortcut(&mut rig.ui, "z", false);
    assert_eq!(rig.graph.get().edges[0].to, FlowEndpoint::new("b", "in"));
}

#[test]
fn ports_connect_with_the_keys_and_say_what_they_are() {
    let mut rig = rig();
    let port = rig
        .ui
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.description.as_deref() == Some("Output of New sale"))
        .cloned()
        .expect("the port speaks");
    assert_eq!((port.role, port.name.as_deref(), port.focusable), (Role::Button, Some("out"), true));
    click(&mut rig.ui, Point::new(120.0, 70.0));
    tab_to(&mut rig.ui, "Output of New sale");
    key(&mut rig.ui, Key::Enter, Modifiers::default());
    assert!(named(&mut rig.ui, "Go to the other port and press Enter; Escape cancels").is_some());
    // A port that refuses says why, and the connection waits for another.
    tab_to(&mut rig.ui, "Input of Send refusal");
    key(&mut rig.ui, Key::Enter, Modifiers::default());
    assert!(named(&mut rig.ui, "Send refusal expects a refused order").is_some());
    assert!(rig.graph.get().edges.is_empty());
    tab_to(&mut rig.ui, "Input of Check stock");
    key(&mut rig.ui, Key::Enter, Modifiers::default());
    let g = rig.graph.get();
    assert_eq!(g.edges.len(), 1);
    assert_eq!(
        (g.edges[0].from.clone(), g.edges[0].to.clone()),
        (FlowEndpoint::new("a", "out"), FlowEndpoint::new("b", "in"))
    );
    assert!(named(&mut rig.ui, "Go to the other port and press Enter; Escape cancels").is_none());
    // Escape gives up a connection started with the keys.
    tab_to(&mut rig.ui, "Output of Check stock");
    key(&mut rig.ui, Key::Enter, Modifiers::default());
    key(&mut rig.ui, Key::Escape, Modifiers::default());
    assert!(named(&mut rig.ui, "Go to the other port and press Enter; Escape cancels").is_none());
    assert_eq!(rig.graph.get().edges.len(), 1);
}

fn size_of(rig: &Rig, id: &str) -> (f32, f32) {
    let g = rig.graph.get();
    let n = g.find_node(id).expect("the node");
    (n.width, n.height)
}

#[test]
fn the_arrange_button_lays_the_diagram_out_in_layers_as_one_change() {
    let chain = graph()
        .edge(FlowEdge::new("e1", FlowEndpoint::new("a", "out"), FlowEndpoint::new("b", "in")))
        .edge(FlowEdge::new("e2", FlowEndpoint::new("b", "out"), FlowEndpoint::new("c", "in")));
    let mut rig = rig_with(chain, |c| c);
    let arrange = named(&mut rig.ui, "Arrange").expect("the button").bounds.center();
    click(&mut rig.ui, arrange);
    let (a, b, c) = (node_at(&rig, "a"), node_at(&rig, "b"), node_at(&rig, "c"));
    assert!(a.0 < b.0 && b.0 < c.0, "one layer after the other: {a:?} {b:?} {c:?}");
    assert!(a.1 == b.1 && b.1 == c.1, "a chain on one line");
    for (x, y) in [a, b, c] {
        assert_eq!((x % 24.0, y % 24.0), (0.0, 0.0), "on the grid");
    }
    assert!(rig.view.get().fit || rig.view.get().zoom <= 1.0, "the whole diagram in view");
    shortcut(&mut rig.ui, "z", false);
    assert_eq!((node_at(&rig, "b"), node_at(&rig, "c")), ((408.0, 48.0), (408.0, 288.0)));
}

#[test]
fn a_locked_canvas_does_not_arrange() {
    let mut rig = rig_with(
        graph().edge(FlowEdge::new("e1", FlowEndpoint::new("a", "out"), FlowEndpoint::new("b", "in"))),
        |c| c.locked(true),
    );
    let arrange = named(&mut rig.ui, "Arrange").expect("the button");
    assert!(arrange.disabled);
    click(&mut rig.ui, arrange.bounds.center());
    assert_eq!(node_at(&rig, "b"), (408.0, 48.0));
}

#[test]
fn the_corner_of_a_selected_node_resizes_it_on_the_grid() {
    let mut rig = rig();
    click(&mut rig.ui, Point::new(120.0, 70.0));
    // The node is 200 by 84 at (48, 48): its corner ends at (248, 132).
    drag(&mut rig.ui, Point::new(242.0, 126.0), Point::new(292.0, 168.0));
    assert_eq!(size_of(&rig, "a"), (240.0, 120.0), "250 and 126, snapped to the grid");
    assert_eq!(node_at(&rig, "a"), (48.0, 48.0), "the corner does not move the node");
    shortcut(&mut rig.ui, "z", false);
    assert_eq!(size_of(&rig, "a"), (200.0, 84.0), "the whole resize undoes at once");
    // Not too small.
    drag(&mut rig.ui, Point::new(242.0, 126.0), Point::new(60.0, 60.0));
    assert_eq!(size_of(&rig, "a"), (96.0, 48.0));
}

#[test]
fn alt_and_the_arrows_resize_the_selected_nodes() {
    let mut rig = rig();
    click(&mut rig.ui, Point::new(120.0, 70.0));
    let alt = Modifiers { alt: true, ..Modifiers::default() };
    key(&mut rig.ui, Key::ArrowRight, alt);
    key(&mut rig.ui, Key::ArrowDown, Modifiers { shift: true, ..alt });
    assert_eq!(size_of(&rig, "a"), (224.0, 180.0));
    assert_eq!(node_at(&rig, "a"), (48.0, 48.0));

    let mut fixed = rig_with(graph(), |c| c.resizable(false));
    click(&mut fixed.ui, Point::new(120.0, 70.0));
    key(&mut fixed.ui, Key::ArrowRight, alt);
    assert_eq!(size_of(&fixed, "a"), (200.0, 84.0));
    // Without a corner, a drag there moves the node instead.
    drag(&mut fixed.ui, Point::new(242.0, 126.0), Point::new(290.0, 126.0));
    assert_eq!((size_of(&fixed, "a"), node_at(&fixed, "a")), ((200.0, 84.0), (96.0, 48.0)));
}

fn parent_of(rig: &Rig, id: &str) -> Option<String> {
    rig.graph.get().find_node(id).and_then(|n| n.parent.clone())
}

/// `a` and `b` in a group, with a line between them; `c` outside.
fn grouped_rig() -> Rig {
    let mut g = graph().edge(
        FlowEdge::new("e1", FlowEndpoint::new("a", "out"), FlowEndpoint::new("b", "in"))
            .shape(EdgeShape::Straight),
    );
    assert!(
        g.group(&["a".into(), "b".into()], FlowNode::new_group("g", "Sale", ravenrs_core::Rect::default()))
    );
    rig_with(g, |c| c)
}

#[test]
fn cmd_g_puts_the_selection_in_a_group_and_shift_takes_it_apart() {
    let mut rig = rig();
    click(&mut rig.ui, Point::new(120.0, 70.0));
    hold_shift(&mut rig.ui, true);
    click(&mut rig.ui, Point::new(480.0, 70.0));
    hold_shift(&mut rig.ui, false);
    shortcut(&mut rig.ui, "g", false);
    let g = rig.graph.get();
    let group = g.find_node("group").expect("a new group").clone();
    assert!(group.group);
    assert_eq!(group.rect(), ravenrs_core::Rect::new(24.0, 0.0, 608.0, 156.0), "around a and b with room");
    assert_eq!((parent_of(&rig, "a"), parent_of(&rig, "b")), (Some("group".into()), Some("group".into())));
    assert_eq!(rig.selection.get(), FlowSelection::node("group"));
    let spoken = named(&mut rig.ui, "Group").expect("the group speaks");
    assert_eq!(spoken.description.as_deref(), Some("Group. 2 nodes inside"));

    shortcut(&mut rig.ui, "g", true);
    assert!(rig.graph.get().find_node("group").is_none());
    assert_eq!(parent_of(&rig, "a"), None);
    assert_eq!(rig.selection.get().nodes, vec!["a".to_owned(), "b".to_owned()]);
    shortcut(&mut rig.ui, "z", false);
    assert_eq!(parent_of(&rig, "a"), Some("group".into()), "ungrouping undoes");
}

#[test]
fn dragging_a_group_moves_what_it_holds() {
    let mut rig = grouped_rig();
    // The group's title band, above a: the group is at (24, 0).
    drag(&mut rig.ui, Point::new(60.0, 20.0), Point::new(60.0, 68.0));
    assert_eq!(node_at(&rig, "g"), (24.0, 48.0));
    assert_eq!((node_at(&rig, "a"), node_at(&rig, "b")), ((48.0, 96.0), (408.0, 96.0)));
    assert_eq!(node_at(&rig, "c"), (408.0, 288.0));
    shortcut(&mut rig.ui, "z", false);
    assert_eq!(node_at(&rig, "a"), (48.0, 48.0), "one change");
}

#[test]
fn dropping_a_node_inside_a_group_puts_it_there_and_outside_takes_it_out() {
    let mut rig = grouped_rig();
    // c into the group: its middle lands at (300, 120), inside (24, 0)-(632, 156).
    drag(&mut rig.ui, Point::new(500.0, 330.0), Point::new(392.0, 120.0));
    assert_eq!(parent_of(&rig, "c"), Some("g".into()));
    let order: Vec<String> = rig.graph.get().nodes.iter().map(|n| n.id.clone()).collect();
    assert!(
        order.iter().position(|id| id == "g") < order.iter().position(|id| id == "c"),
        "drawn above its group"
    );
    // a out of it.
    drag(&mut rig.ui, Point::new(120.0, 70.0), Point::new(120.0, 470.0));
    assert_eq!(parent_of(&rig, "a"), None);
    assert_eq!(parent_of(&rig, "b"), Some("g".into()));
}

#[test]
fn a_line_inside_a_group_is_still_picked_and_deleting_the_group_takes_its_nodes() {
    let mut rig = grouped_rig();
    click(&mut rig.ui, Point::new(330.0, 90.0));
    assert_eq!(rig.selection.get(), FlowSelection::edge("e1"));
    click(&mut rig.ui, Point::new(60.0, 20.0));
    assert_eq!(rig.selection.get(), FlowSelection::node("g"));
    shortcut(&mut rig.ui, "c", false);
    key(&mut rig.ui, Key::Delete, Modifiers::default());
    let g = rig.graph.get();
    assert_eq!(g.nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(), ["c"]);
    shortcut(&mut rig.ui, "v", false);
    let g = rig.graph.get();
    assert_eq!(g.nodes.len(), 4, "the group pastes back with what it held");
    assert_eq!(g.find_node("a").and_then(|n| n.parent.clone()).as_deref(), Some("g"));
}

/// A canvas whose app refuses the changes `refuse` names, saying "Published steps stay".
fn guarded(refuse: fn(&FlowChange) -> bool) -> (Rig, State<FlowHistory>, State<Vec<FlowChange>>) {
    let history = State::new(FlowHistory::new());
    let asked = State::new(Vec::<FlowChange>::new());
    let seen = asked.clone();
    let rig = rig_with(graph(), |c| {
        c.history(history.binding()).on_change(move |_, _, change| {
            seen.update(|v| v.push(change.clone()));
            if refuse(change) { Err("Published steps stay".into()) } else { Ok(()) }
        })
    });
    (rig, history, asked)
}

#[test]
fn the_app_can_refuse_a_change_which_says_why_and_leaves_no_history() {
    let (mut rig, history, asked) = guarded(|c| matches!(c, FlowChange::Delete { .. }));
    click(&mut rig.ui, Point::new(120.0, 70.0));
    key(&mut rig.ui, Key::Delete, Modifiers::default());
    assert!(rig.graph.get().find_node("a").is_some(), "still there");
    assert_eq!(asked.get().last(), Some(&FlowChange::Delete { nodes: vec!["a".into()], edges: Vec::new() }));
    let reason = named(&mut rig.ui, "Published steps stay").expect("the reason shows");
    assert_eq!(reason.role, Role::Status);
    assert!(!history.get().can_undo());
    // The next gesture lets go of the reason; a change it allows goes through.
    key(&mut rig.ui, Key::ArrowRight, Modifiers::default());
    assert!(named(&mut rig.ui, "Published steps stay").is_none());
    assert_eq!(node_at(&rig, "a"), (72.0, 48.0));
    assert_eq!(asked.get().last(), Some(&FlowChange::Move { nodes: vec!["a".into()] }));
    assert!(history.get().can_undo());
}

#[test]
fn a_refused_drag_puts_the_nodes_back_and_is_asked_once() {
    let (mut rig, history, asked) = guarded(|c| matches!(c, FlowChange::Move { .. }));
    drag(&mut rig.ui, Point::new(120.0, 70.0), Point::new(220.0, 170.0));
    assert_eq!(node_at(&rig, "a"), (48.0, 48.0), "back where it was");
    assert_eq!(asked.get(), vec![FlowChange::Move { nodes: vec!["a".into()] }], "once, when let go");
    assert!(!history.get().can_undo());
    assert!(named(&mut rig.ui, "Published steps stay").is_some());
}

#[test]
fn the_app_can_refuse_an_undo() {
    let (mut rig, history, _) = guarded(|c| *c == FlowChange::Undo);
    click(&mut rig.ui, Point::new(120.0, 70.0));
    key(&mut rig.ui, Key::ArrowRight, Modifiers::default());
    shortcut(&mut rig.ui, "z", false);
    assert_eq!(node_at(&rig, "a"), (72.0, 48.0));
    assert!(history.get().can_undo(), "the history stays as it was");
}

#[test]
fn cmd_brackets_bring_a_node_to_the_front_or_send_it_back() {
    // `d` overlaps `a` from below and to the right, and is in front of it.
    let mut rig = rig_with(graph().node(FlowNode::new("d", "action", "Overlap", 168.0, 96.0)), |c| c);
    let overlap = Point::new(200.0, 110.0);
    click(&mut rig.ui, overlap);
    assert_eq!(rig.selection.get(), FlowSelection::node("d"));
    click(&mut rig.ui, Point::new(70.0, 60.0));
    shortcut(&mut rig.ui, "]", false);
    let order: Vec<String> = rig.graph.get().nodes.iter().map(|n| n.id.clone()).collect();
    assert_eq!(order.last().map(String::as_str), Some("a"));
    click(&mut rig.ui, overlap);
    assert_eq!(rig.selection.get(), FlowSelection::node("a"), "a is in front now");
    shortcut(&mut rig.ui, "[", false);
    assert_eq!(rig.graph.get().nodes[0].id, "a");
    click(&mut rig.ui, overlap);
    assert_eq!(rig.selection.get(), FlowSelection::node("d"));
    shortcut(&mut rig.ui, "z", false);
    shortcut(&mut rig.ui, "z", false);
    assert_eq!(rig.graph.get().nodes.last().map(|n| n.id.as_str()), Some("d"), "both undo");
}

fn double_click(ui: &mut Ui, at: Point) {
    click(ui, at);
    ui.handle_event(&Event::DoubleClick { position: at });
    settle(ui);
}

fn type_text(ui: &mut Ui, text: &str) {
    for letter in text.chars() {
        key(ui, Key::Character(letter.to_string()), Modifiers::default());
    }
}

fn title_of(rig: &Rig, id: &str) -> String {
    rig.graph.get().find_node(id).map(|n| n.title.clone()).unwrap_or_default()
}

fn field(ui: &mut Ui) -> Option<AccessNode> {
    ui.access_tree().nodes.iter().find(|n| n.role == Role::TextInput).cloned()
}

#[test]
fn a_double_click_types_the_title_in_place_and_enter_keeps_it() {
    let mut rig = rig();
    double_click(&mut rig.ui, Point::new(120.0, 70.0));
    let open = field(&mut rig.ui).expect("the title field");
    assert_eq!(open.value.as_deref(), Some("New sale"));
    // All of it was selected: typing replaces it.
    type_text(&mut rig.ui, "Charge");
    key(&mut rig.ui, Key::Enter, Modifiers::default());
    assert_eq!(title_of(&rig, "a"), "Charge");
    assert!(field(&mut rig.ui).is_none(), "closed");
    assert!(named(&mut rig.ui, "Charge").is_some(), "read with its new name");
    let tree = rig.ui.access_tree();
    let focused = tree.focus.and_then(|id| tree.get(id)).and_then(|n| n.name.clone());
    assert_eq!(focused.as_deref(), Some("Charge"), "the focus is back on the node");
    shortcut(&mut rig.ui, "z", false);
    assert_eq!(title_of(&rig, "a"), "New sale", "one change");
}

#[test]
fn f2_opens_the_title_and_escape_or_an_empty_title_changes_nothing() {
    let (mut rig, history, _) = guarded(|_| false);
    click(&mut rig.ui, Point::new(120.0, 70.0));
    key(&mut rig.ui, Key::Function(2), Modifiers::default());
    type_text(&mut rig.ui, "Gone");
    key(&mut rig.ui, Key::Escape, Modifiers::default());
    assert_eq!(title_of(&rig, "a"), "New sale");
    // Enter on a node already selected opens it too.
    key(&mut rig.ui, Key::Enter, Modifiers::default());
    assert!(field(&mut rig.ui).is_some());
    key(&mut rig.ui, Key::Backspace, Modifiers::default());
    key(&mut rig.ui, Key::Enter, Modifiers::default());
    assert_eq!(title_of(&rig, "a"), "New sale", "an empty title is not kept");
    assert!(!history.get().can_undo());
}

#[test]
fn the_app_can_refuse_a_title() {
    let (mut rig, _, asked) = guarded(|c| matches!(c, FlowChange::Rename { .. }));
    double_click(&mut rig.ui, Point::new(120.0, 70.0));
    type_text(&mut rig.ui, "Cancel");
    key(&mut rig.ui, Key::Enter, Modifiers::default());
    assert_eq!(title_of(&rig, "a"), "New sale");
    assert_eq!(asked.get().last(), Some(&FlowChange::Rename { node: "a".into() }));
    assert!(named(&mut rig.ui, "Published steps stay").is_some());
    assert!(field(&mut rig.ui).is_none(), "the field closes anyway");
}

#[test]
fn titles_stay_shut_when_turned_off_or_locked() {
    let mut off = rig_with(graph(), |c| c.editable_titles(false));
    double_click(&mut off.ui, Point::new(120.0, 70.0));
    key(&mut off.ui, Key::Function(2), Modifiers::default());
    assert!(field(&mut off.ui).is_none());
    let mut locked = rig_with(graph(), |c| c.locked(true));
    double_click(&mut locked.ui, Point::new(120.0, 70.0));
    assert!(field(&mut locked.ui).is_none());
}

#[test]
fn a_title_opened_too_small_to_read_comes_to_full_size_and_back() {
    let mut rig = rig();
    rig.view.set(FlowViewport::at(Point::new(0.0, 0.0), 0.3));
    settle(&mut rig.ui);
    // At 30% the node `a` covers (14, 14) to (74, 40).
    click(&mut rig.ui, Point::new(40.0, 25.0));
    key(&mut rig.ui, Key::Function(2), Modifiers::default());
    assert_eq!(rig.view.get().zoom, 1.0);
    let middle = rig.view.get().to_screen(Point::new(148.0, 90.0));
    assert_eq!(middle, Point::new(450.0, 300.0), "the node in the middle of the view");
    type_text(&mut rig.ui, "Charge");
    key(&mut rig.ui, Key::Enter, Modifiers::default());
    assert_eq!(rig.view.get(), FlowViewport::at(Point::new(0.0, 0.0), 0.3));
    assert_eq!(title_of(&rig, "a"), "Charge");
}

#[test]
fn the_ends_of_a_line_draw_the_caps_asked_for() {
    let line = |start: EdgeCap, end: EdgeCap| {
        let edge = FlowEdge::new("e1", FlowEndpoint::new("a", "out"), FlowEndpoint::new("b", "in"))
            .shape(EdgeShape::Straight)
            .start(start)
            .end(end);
        let rig = rig_with(graph().edge(edge), |c| c);
        outlines(&rig.ui).len()
    };
    let bare = line(EdgeCap::None, EdgeCap::None);
    assert_eq!(line(EdgeCap::None, EdgeCap::Arrow), bare + 1, "the arrow, as before");
    assert_eq!(line(EdgeCap::Circle, EdgeCap::Diamond), bare + 2, "one cap at each end");
    assert_eq!(line(EdgeCap::None, EdgeCap::OpenArrow), bare + 1);
}

fn turn_of(rig: &Rig, id: &str) -> f32 {
    rig.graph.get().find_node(id).map_or(f32::NAN, |n| n.rotation)
}

fn alt_shift() -> Modifiers {
    Modifiers { alt: true, shift: true, ..Modifiers::default() }
}

#[test]
fn alt_shift_and_the_arrows_turn_a_node_fifteen_degrees_at_a_time() {
    let mut rig = rig();
    click(&mut rig.ui, Point::new(120.0, 70.0));
    key(&mut rig.ui, Key::ArrowRight, alt_shift());
    key(&mut rig.ui, Key::ArrowRight, alt_shift());
    assert_eq!(turn_of(&rig, "a"), 30.0);
    let spoken = named(&mut rig.ui, "New sale").and_then(|n| n.description).unwrap_or_default();
    assert!(spoken.ends_with("Turned 30°"), "{spoken}");
    key(&mut rig.ui, Key::ArrowLeft, alt_shift());
    assert_eq!(turn_of(&rig, "a"), 15.0);
    key(&mut rig.ui, Key::Character("0".into()), alt_shift());
    assert_eq!(turn_of(&rig, "a"), 0.0, "straight again");
    shortcut(&mut rig.ui, "z", false);
    assert_eq!(turn_of(&rig, "a"), 15.0, "each key is a change");
}

#[test]
fn the_handle_turns_a_node_and_the_pointer_finds_it_turned() {
    let mut rig = rig();
    click(&mut rig.ui, Point::new(120.0, 70.0));
    // Node `a` is 200 by 84 at (48, 48), its middle at (148, 90); the handle sits 24 above it.
    drag(&mut rig.ui, Point::new(148.0, 14.0), Point::new(260.0, 92.0));
    assert_eq!(turn_of(&rig, "a"), 90.0, "the pointer due right of the middle, on a 15° step");
    assert_eq!(
        rig.graph.get().anchor(&FlowEndpoint::new("a", "out")).map(|a| a.1),
        Some(PortSide::Bottom),
        "the output now faces down"
    );
    // Standing up, it covers 106..190 across and -10..190 down.
    click(&mut rig.ui, Point::new(300.0, 500.0));
    click(&mut rig.ui, Point::new(148.0, 170.0));
    assert_eq!(rig.selection.get(), FlowSelection::node("a"), "found where it shows");
    click(&mut rig.ui, Point::new(60.0, 70.0));
    assert!(rig.selection.get().is_empty(), "not where it used to lie");
    shortcut(&mut rig.ui, "z", false);
    assert_eq!(turn_of(&rig, "a"), 0.0, "the whole drag is one change");
}

#[test]
fn groups_do_not_turn_and_turning_can_be_switched_off() {
    let mut rig = grouped_rig();
    click(&mut rig.ui, Point::new(60.0, 20.0));
    key(&mut rig.ui, Key::ArrowRight, alt_shift());
    assert_eq!(turn_of(&rig, "g"), 0.0);
    assert!(named(&mut rig.ui, "Groups do not turn").is_some());

    let mut fixed = rig_with(graph(), |c| c.rotatable(false));
    click(&mut fixed.ui, Point::new(120.0, 70.0));
    key(&mut fixed.ui, Key::ArrowRight, alt_shift());
    drag(&mut fixed.ui, Point::new(148.0, 14.0), Point::new(260.0, 92.0));
    assert_eq!(turn_of(&fixed, "a"), 0.0, "no handle, no keys");
}

/// A canvas where `a` shows an Approve button that counts its presses.
fn with_content() -> (Rig, std::rc::Rc<std::cell::Cell<u32>>) {
    let pressed = std::rc::Rc::new(std::cell::Cell::new(0));
    let count = pressed.clone();
    let rig = rig_with(graph(), move |c| {
        c.content(move |node| {
            let count = count.clone();
            (node.id == "a").then(|| {
                Box::new(Button::new("Approve").on_click(move || count.set(count.get() + 1))) as AnyView
            })
        })
    });
    (rig, pressed)
}

#[test]
fn a_view_inside_a_node_takes_its_clicks_at_any_zoom() {
    let (mut rig, pressed) = with_content();
    let button = named(&mut rig.ui, "Approve").expect("the view inside the node");
    let full = button.bounds;
    assert!(full.x() >= 48.0 && full.max_x() <= 248.0 && full.y() >= 48.0, "{full:?}");
    click(&mut rig.ui, full.center());
    assert_eq!(pressed.get(), 1);
    assert!(rig.selection.get().is_empty(), "the button took the click, not the node");

    rig.view.set(FlowViewport::at(Point::new(0.0, 0.0), 0.5));
    settle(&mut rig.ui);
    let half = named(&mut rig.ui, "Approve").expect("still there").bounds;
    assert!((half.width() - full.width() / 2.0).abs() < 0.5, "drawn at the zoom: {half:?} {full:?}");
    assert!((half.x() - full.x() / 2.0).abs() < 0.5 && (half.y() - full.y() / 2.0).abs() < 0.5);
    click(&mut rig.ui, half.center());
    assert_eq!(pressed.get(), 2, "found where it shows at 50%");
}

#[test]
fn the_rest_of_a_card_with_a_view_still_moves_the_node() {
    let (mut rig, pressed) = with_content();
    // Alt keeps the alignment guides out of it: this is about the card, not lining up.
    rig.ui.handle_event(&Event::KeyUp {
        key: Key::Character("x".into()),
        modifiers: Modifiers { alt: true, ..Modifiers::default() },
    });
    // Below the button, inside the card.
    drag(&mut rig.ui, Point::new(200.0, 124.0), Point::new(250.0, 172.0));
    assert_eq!(node_at(&rig, "a"), (96.0, 96.0));
    assert_eq!(pressed.get(), 0);
    assert!(named(&mut rig.ui, "New sale").is_some(), "the node keeps its name");
    assert!(named(&mut rig.ui, "Check stock").is_some(), "a node without a view is as before");
}

#[test]
fn arrange_as_a_tree_puts_the_root_over_what_it_leads_to() {
    let tree = graph()
        .edge(FlowEdge::new("e1", FlowEndpoint::new("a", "out"), FlowEndpoint::new("b", "in")))
        .node(FlowNode::new("d", "action", "Bill", 900.0, 900.0).port(FlowPort::input("in").kind("sale")))
        .edge(FlowEdge::new("e2", FlowEndpoint::new("a", "out"), FlowEndpoint::new("d", "in")));
    let mut rig = rig_with(tree, |c| c.layout(FlowLayout::tree(FlowDirection::Down)));
    let arrange = named(&mut rig.ui, "Arrange").expect("the button").bounds.center();
    click(&mut rig.ui, arrange);
    let g = rig.graph.get();
    let middle = |id: &str| g.find_node(id).map(|n| n.rect().center()).unwrap_or(Point::new(f32::NAN, 0.0));
    let (a, b, d) = (middle("a"), middle("b"), middle("d"));
    assert!(b.y == d.y && b.y > a.y, "the two below the root");
    assert!((a.x - (b.x + d.x) / 2.0).abs() <= 12.0, "the root over them, to the grid: {a:?} {b:?} {d:?}");
}

#[test]
fn a_diamond_is_found_inside_its_outline_with_its_ports_on_the_tips() {
    // A decision 200 by 100 at (600, 48): its tips at (600, 98), (800, 98) and (700, 148).
    let decision = FlowNode::new("d", "action", "Enough?", 600.0, 48.0)
        .size(200.0, 100.0)
        .shape(FlowShape::Diamond)
        .port(FlowPort::input("in").kind("sale"))
        .port(FlowPort::output("no").side(PortSide::Bottom).kind("sale"));
    let mut rig = rig_with(graph().node(decision), |c| c);
    click(&mut rig.ui, Point::new(700.0, 98.0));
    assert_eq!(rig.selection.get(), FlowSelection::node("d"), "the middle is the node");
    click(&mut rig.ui, Point::new(612.0, 56.0));
    assert!(rig.selection.get().is_empty(), "the corner of its box is not");
    assert_eq!(
        rig.graph.get().anchor(&FlowEndpoint::new("d", "no")).map(|a| a.0),
        Some(Point::new(700.0, 148.0))
    );
    assert_eq!(
        rig.graph.get().anchor(&FlowEndpoint::new("d", "in")).map(|a| a.0),
        Some(Point::new(600.0, 98.0))
    );
}

#[test]
fn a_note_wraps_its_words_and_shapes_draw_their_outline() {
    let note = FlowNode::new(
        "n",
        "note",
        "Check the tax code of the items without one before publishing.",
        48.0,
        300.0,
    )
    .size(200.0, 120.0)
    .shape(FlowShape::Note);
    let rig = rig_with(graph().node(note), |c| c);
    let lines: std::collections::BTreeSet<i32> = rig
        .ui
        .scene()
        .primitives()
        .iter()
        .filter_map(|p| if let ravenrs_core::Primitive::Glyphs(run) = p { Some(run) } else { None })
        .flat_map(|run| run.glyphs.iter())
        .filter(|g| g.x > 48.0 && g.x < 248.0 && g.y > 300.0 && g.y < 420.0)
        .map(|g| g.y.round() as i32)
        .collect();
    assert!(lines.len() >= 3, "the note's words wrap over lines: {lines:?}");
}

#[test]
fn the_arrow_beside_a_node_makes_the_next_one_joined_with_its_title_open() {
    let mut rig = rig();
    click(&mut rig.ui, Point::new(480.0, 70.0));
    // `b` is 200 by 84 at (408, 48): its right arrow sits 12 past its edge, in the middle.
    click(&mut rig.ui, Point::new(632.0, 90.0));
    let g = rig.graph.get();
    let made = g.find_node("b-2").cloned().expect("the next node");
    assert_eq!((made.x, made.y), (696.0, 48.0), "a layer to the right, on the grid");
    assert_eq!(made.kind, "action");
    let joined = g.edges.iter().find(|e| e.to.node == "b-2").expect("joined");
    assert_eq!(joined.from, FlowEndpoint::new("b", "out"));
    assert_eq!(rig.selection.get(), FlowSelection::node("b-2"));
    assert!(field(&mut rig.ui).is_some(), "its title is open");
    type_text(&mut rig.ui, "Bill");
    key(&mut rig.ui, Key::Enter, Modifiers::default());
    assert_eq!(title_of(&rig, "b-2"), "Bill");
    shortcut(&mut rig.ui, "z", false);
    shortcut(&mut rig.ui, "z", false);
    let g = rig.graph.get();
    assert!(g.find_node("b-2").is_none() && g.edges.is_empty(), "made and joined in one change");
}

#[test]
fn cmd_shift_enter_makes_it_below_and_finds_a_free_place() {
    let mut rig = rig();
    click(&mut rig.ui, Point::new(480.0, 70.0));
    key(&mut rig.ui, Key::Escape, Modifiers::default());
    click(&mut rig.ui, Point::new(480.0, 70.0));
    key(&mut rig.ui, Key::Enter, Modifiers { meta: true, shift: true, ..Modifiers::default() });
    // Below `b` is (408, 240), where `c` at (408, 288) is in the way: one place further down.
    assert_eq!(node_at(&rig, "b-2"), (408.0, 384.0));
}

#[test]
fn the_app_chooses_the_next_node_and_quick_creation_can_be_switched_off() {
    let mut rig = rig_with(graph(), |c| {
        c.next_node(|_, side| {
            (side == PortSide::Right).then(|| {
                FlowNode::new("step", "action", "New step", 0.0, 0.0)
                    .port(FlowPort::input("in").kind("sale"))
                    .port(FlowPort::output("out").kind("sale"))
            })
        })
    });
    click(&mut rig.ui, Point::new(120.0, 70.0));
    key(&mut rig.ui, Key::Enter, Modifiers { meta: true, ..Modifiers::default() });
    let g = rig.graph.get();
    assert_eq!(g.find_node("step").map(|n| n.title.clone()).as_deref(), Some("New step"));
    assert!(
        g.edges
            .iter()
            .any(|e| e.from == FlowEndpoint::new("a", "out") && e.to == FlowEndpoint::new("step", "in"))
    );

    let mut off = rig_with(graph(), |c| c.quick_create(false));
    click(&mut off.ui, Point::new(480.0, 70.0));
    key(&mut off.ui, Key::Enter, Modifiers { meta: true, ..Modifiers::default() });
    click(&mut off.ui, Point::new(632.0, 90.0));
    assert_eq!(off.graph.get().nodes.len(), 3);
}

#[test]
fn a_dragged_node_lines_up_with_another_before_the_grid_and_alt_does_without() {
    // `d` sits off the grid, its top at 61; `c` is dragged until its top is at 64.
    let with_d = || graph().node(FlowNode::new("d", "action", "Off the grid", 700.0, 61.0));
    let mut rig = rig_with(with_d(), |c| c);
    drag(&mut rig.ui, Point::new(500.0, 330.0), Point::new(500.0, 106.0));
    assert_eq!(node_at(&rig, "c"), (408.0, 61.0), "lined up with the top of `d`, not the grid");

    let mut free = rig_with(with_d(), |c| c);
    free.ui.handle_event(&Event::KeyUp {
        key: Key::Character("x".into()),
        modifiers: Modifiers { alt: true, ..Modifiers::default() },
    });
    drag(&mut free.ui, Point::new(500.0, 330.0), Point::new(500.0, 106.0));
    assert_eq!(node_at(&free, "c"), (408.0, 72.0), "with Alt, the grid alone");
}

struct Editor {
    ui: Ui,
    graph: State<FlowGraph>,
    selection: State<FlowSelection>,
    history: State<FlowHistory>,
}

/// The canvas and its inspector side by side, on the same diagram, selection and history.
fn editor(tweak: impl FnOnce(FlowInspector) -> FlowInspector) -> Editor {
    let graph = State::new(graph().edge(FlowEdge::new(
        "e1",
        FlowEndpoint::new("a", "out"),
        FlowEndpoint::new("b", "in"),
    )));
    let selection = State::new(FlowSelection::default());
    let history = State::new(FlowHistory::new());
    let canvas = FlowCanvas::new(graph.binding())
        .selection(selection.binding())
        .history(history.binding())
        .view(State::new(FlowViewport::at(Point::new(0.0, 0.0), 1.0)).binding())
        .size(700.0, 600.0);
    let inspector = tweak(
        FlowInspector::new(graph.binding(), selection.binding())
            .history(history.binding())
            .kind("trigger", "Trigger")
            .kind("action", "Action"),
    );
    let mut ui = Ui::new(HStack::new((canvas, inspector)), Theme::raven_dark());
    ui.set_viewport(Size::new(1000.0, 600.0), 1.0);
    settle(&mut ui);
    Editor { ui, graph, selection, history }
}

fn input_named(ui: &mut Ui, name: &str) -> Option<AccessNode> {
    ui.access_tree()
        .nodes
        .iter()
        .find(|n| n.role == Role::TextInput && n.name.as_deref() == Some(name))
        .cloned()
}

#[test]
fn the_inspector_edits_the_selected_node_through_the_canvas_history() {
    let mut ed = editor(|i| i);
    assert!(named(&mut ed.ui, "Select a node or a connection to see its properties").is_some());
    ed.selection.set(FlowSelection::node("a"));
    settle(&mut ed.ui);
    let title = input_named(&mut ed.ui, "Title").expect("the title field");
    assert_eq!(title.value.as_deref(), Some("New sale"));
    click(&mut ed.ui, title.bounds.center());
    key(&mut ed.ui, Key::Character("a".into()), Modifiers { meta: true, ..Modifiers::default() });
    type_text(&mut ed.ui, "Charge");
    key(&mut ed.ui, Key::Enter, Modifiers::default());
    let a = ed.graph.get().find_node("a").cloned().expect("a");
    assert_eq!(a.title, "Charge");
    assert!(ed.history.get().can_undo(), "in the canvas's history");
}

#[test]
fn the_inspector_shows_mixed_values_and_gives_a_choice_to_all() {
    let mut ed = editor(|i| i);
    ed.graph.update(|g| {
        if let Some(b) = g.find_node_mut("b") {
            b.shape = FlowShape::Diamond;
        }
    });
    ed.selection.set(FlowSelection { nodes: vec!["a".into(), "b".into()], edges: Vec::new() });
    settle(&mut ed.ui);
    assert!(named(&mut ed.ui, "2 nodes").is_some());
    assert!(input_named(&mut ed.ui, "Title").is_none(), "only what they share");
    let shape = ed
        .ui
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some("Shape"))
        .cloned()
        .expect("the shape choice");
    assert_eq!(shape.value.as_deref(), Some("Mixed"));
}

#[test]
fn the_inspector_edits_a_connection_and_can_be_refused_or_read_only() {
    let mut ed = editor(|i| i.on_change(|_, _, _| Err("Published steps stay".into())));
    ed.selection.set(FlowSelection::edge("e1"));
    settle(&mut ed.ui);
    let animated = named(&mut ed.ui, "Animated").expect("the checkbox");
    click(&mut ed.ui, animated.bounds.center());
    assert!(!ed.graph.get().edges[0].animated, "refused");
    assert!(named(&mut ed.ui, "Published steps stay").is_some(), "and says why");

    let mut free = editor(|i| i);
    free.selection.set(FlowSelection::edge("e1"));
    settle(&mut free.ui);
    let animated = named(&mut free.ui, "Animated").expect("the checkbox");
    click(&mut free.ui, animated.bounds.center());
    assert!(free.graph.get().edges[0].animated);

    let mut locked = editor(|i| i.read_only(true));
    locked.selection.set(FlowSelection::node("a"));
    settle(&mut locked.ui);
    assert!(named(&mut locked.ui, "Editing is locked: read only").is_some());
}

/// A toolbox beside the canvas, sharing the binding the models picked with Enter go through.
struct Shop {
    ui: Ui,
    graph: State<FlowGraph>,
    selection: State<FlowSelection>,
}

fn shop() -> Shop {
    let graph = State::new(graph());
    let selection = State::new(FlowSelection::default());
    let history = State::new(FlowHistory::new());
    let arrivals = State::new(None::<FlowNode>);
    let toolbox = FlowToolbox::new()
        .group("Flow")
        .model("Step", FlowNode::new("step", "action", "Step", 0.0, 0.0))
        .model(
            "Choice",
            FlowNode::new("choice", "action", "Choice", 0.0, 0.0).shape(FlowShape::Diamond).size(120.0, 80.0),
        )
        .arrivals(arrivals.binding());
    let canvas = FlowCanvas::new(graph.binding())
        .selection(selection.binding())
        .history(history.binding())
        .arrivals(arrivals.binding())
        .view(State::new(FlowViewport::at(Point::new(0.0, 0.0), 1.0)).binding())
        .size(700.0, 600.0);
    let mut ui = Ui::new(HStack::new((toolbox, canvas)), Theme::raven_dark());
    ui.set_viewport(Size::new(940.0, 600.0), 1.0);
    settle(&mut ui);
    Shop { ui, graph, selection }
}

fn snapped(v: f32) -> f32 {
    (v / 24.0).round() * 24.0
}

#[test]
fn a_model_dragged_onto_the_canvas_lands_there_as_a_new_node() {
    let mut shop = shop();
    let before = shop.graph.get().nodes.len();
    let tile = named(&mut shop.ui, "Choice").expect("the model").bounds.center();
    // The canvas's corner, from the node at (48, 48) of the diagram.
    let left = named(&mut shop.ui, "New sale").expect("a node").bounds.origin.x - 48.0;
    drag(&mut shop.ui, tile, Point::new(left + 300.0, 300.0));

    let graph = shop.graph.get();
    assert_eq!(graph.nodes.len(), before + 1, "one node more");
    let made = graph.nodes.last().expect("the new node");
    assert_eq!((made.title.as_str(), made.shape), ("Choice", FlowShape::Diamond));
    assert_eq!(
        (made.x, made.y),
        (snapped(300.0 - 60.0), snapped(300.0 - 40.0)),
        "centred where dropped, on the grid"
    );
    assert_eq!(shop.selection.get(), FlowSelection::node(made.id.clone()), "and selected");

    let first = made.id.clone();

    // A second copy gets an id of its own.
    drag(&mut shop.ui, tile, Point::new(left + 500.0, 450.0));
    let graph = shop.graph.get();
    assert_eq!(graph.nodes.len(), before + 2);
    assert_ne!(graph.nodes.last().expect("the second").id, first, "ids of their own");

    // Each is one change: undo takes them away one by one.
    let cmd = command(false);
    key(&mut shop.ui, Key::Character("z".into()), cmd);
    key(&mut shop.ui, Key::Character("z".into()), cmd);
    assert_eq!(shop.graph.get().nodes.len(), before);
}

#[test]
fn enter_on_a_model_puts_a_copy_in_the_middle_of_the_view() {
    let mut shop = shop();
    let before = shop.graph.get().nodes.len();
    tab_to(&mut shop.ui, "Drag onto the diagram, or press Enter to place it in the middle");
    key(&mut shop.ui, Key::Enter, Modifiers::default());
    settle(&mut shop.ui);

    let graph = shop.graph.get();
    assert_eq!(graph.nodes.len(), before + 1);
    let made = graph.nodes.last().expect("the new node");
    assert_eq!(made.title, "Step");
    let (w, h) = (made.width, made.height);
    assert_eq!((made.x, made.y), (snapped(350.0 - w / 2.0), snapped(300.0 - h / 2.0)));
    assert_eq!(shop.selection.get(), FlowSelection::node(made.id.clone()));
}

#[test]
fn a_locked_canvas_takes_no_model() {
    let mut shop = shop();
    let before = shop.graph.get().nodes.len();
    let lock = named(&mut shop.ui, "Lock editing").expect("the lock").bounds.center();
    click(&mut shop.ui, lock);
    let tile = named(&mut shop.ui, "Step").expect("the model").bounds.center();
    // The canvas's corner, from the node at (48, 48) of the diagram.
    let left = named(&mut shop.ui, "New sale").expect("a node").bounds.origin.x - 48.0;
    drag(&mut shop.ui, tile, Point::new(left + 300.0, 400.0));
    assert_eq!(shop.graph.get().nodes.len(), before, "nothing lands");
}

fn bent() -> Rig {
    rig_with(
        graph().edge(
            FlowEdge::new("e1", FlowEndpoint::new("a", "out"), FlowEndpoint::new("b", "in")).label("Yes"),
        ),
        |c| c,
    )
}

/// The middle of the line's only stretch, where its `+` is, in the view (at 100%, from 0, 0).
fn stretch_middle(rig: &Rig) -> Point {
    let g = rig.graph.get();
    let c = g.edge_controls(&g.edges[0]).expect("the line");
    Point::new((c[0].x + c[1].x) / 2.0, (c[0].y + c[1].y) / 2.0)
}

#[test]
fn dragging_the_plus_of_a_selected_line_makes_a_bend_point_on_the_grid() {
    let mut rig = bent();
    let middle = stretch_middle(&rig);
    click(&mut rig.ui, middle);
    assert_eq!(rig.selection.get(), FlowSelection::edge("e1"), "the line is selected");

    drag(&mut rig.ui, middle, Point::new(middle.x + 10.0, 250.0));
    let point = (snapped(middle.x + 10.0), snapped(250.0));
    assert_eq!(rig.graph.get().edges[0].waypoints, vec![point]);
    // The line goes through it.
    let g = rig.graph.get();
    let path = ravenrs_graphics::flow::flatten(&g.edge_path(&g.edges[0]).expect("a path"));
    assert!(ravenrs_graphics::flow::distance_to(&path, Point::new(point.0, point.1)) < 1.0);

    // One change to undo.
    key(&mut rig.ui, Key::Character("z".into()), command(false));
    assert!(rig.graph.get().edges[0].waypoints.is_empty());
}

#[test]
fn a_bend_point_moves_with_the_arrows_and_goes_with_delete() {
    let mut rig = bent();
    rig.graph.update(|g| g.edges[0].waypoints = vec![(336.0, 240.0)]);
    rig.selection.set(FlowSelection::edge("e1"));
    settle(&mut rig.ui);
    let point = named(&mut rig.ui, "Point 1 of 1 of the connection Yes")
        .expect("the point, named for screen readers");
    click(&mut rig.ui, point.bounds.center());
    key(&mut rig.ui, Key::ArrowRight, Modifiers::default());
    assert_eq!(rig.graph.get().edges[0].waypoints, vec![(360.0, 240.0)]);
    key(&mut rig.ui, Key::ArrowUp, Modifiers { shift: true, ..Modifiers::default() });
    assert_eq!(rig.graph.get().edges[0].waypoints, vec![(360.0, 144.0)]);
    key(&mut rig.ui, Key::Delete, Modifiers::default());
    assert!(rig.graph.get().edges[0].waypoints.is_empty());
    assert_eq!(rig.selection.get(), FlowSelection::edge("e1"), "the line stays selected");
}

#[test]
fn a_double_click_on_a_line_makes_a_point_and_one_dropped_on_the_straight_goes() {
    let mut rig = bent();
    let middle = stretch_middle(&rig);
    double_click(&mut rig.ui, middle);
    assert_eq!(rig.graph.get().edges[0].waypoints.len(), 1, "a point where it was clicked");
    assert_eq!(rig.selection.get(), FlowSelection::edge("e1"));

    // Dragged back onto the straight between the ports, it is no longer needed.
    rig.graph.update(|g| g.edges[0].waypoints = vec![(336.0, 240.0)]);
    settle(&mut rig.ui);
    let g = rig.graph.get();
    let c = g.edge_controls(&g.edges[0]).expect("the line");
    let on_straight = Point::new((c[0].x + c[2].x) / 2.0, (c[0].y + c[2].y) / 2.0);
    drag(&mut rig.ui, Point::new(336.0, 240.0), on_straight);
    // Snapped to the grid it may sit a little off the straight; near enough, it goes.
    let left = rig.graph.get().edges[0].waypoints.clone();
    assert!(left.is_empty() || left.iter().all(|&(_, y)| (y - on_straight.y).abs() > 4.0), "{left:?}");
}

#[test]
fn a_line_whose_two_ends_move_takes_its_points_along() {
    let mut rig = bent();
    rig.graph.update(|g| g.edges[0].waypoints = vec![(336.0, 240.0)]);
    rig.selection.set(FlowSelection { nodes: vec!["a".into(), "b".into()], edges: vec![] });
    settle(&mut rig.ui);
    drag(&mut rig.ui, Point::new(120.0, 70.0), Point::new(168.0, 118.0));
    assert_eq!(rig.graph.get().edges[0].waypoints, vec![(384.0, 288.0)], "moved with both nodes");

    // With one end moving, the point stays.
    rig.selection.set(FlowSelection::node("a"));
    settle(&mut rig.ui);
    drag(&mut rig.ui, Point::new(168.0, 118.0), Point::new(168.0, 166.0));
    assert_eq!(rig.graph.get().edges[0].waypoints, vec![(384.0, 288.0)]);
}

#[test]
fn the_shortcut_opens_the_export_menu_and_exporting_changes_nothing() {
    let got: std::rc::Rc<std::cell::RefCell<Vec<(FlowExportFormat, FlowPicture)>>> = Default::default();
    let seen = got.clone();
    let mut rig = rig_with(graph(), move |c| {
        c.on_export(move |format, picture| seen.borrow_mut().push((format, picture)))
    });
    assert!(named(&mut rig.ui, "Export").is_some(), "a button in the controls");

    // Cmd/Ctrl+Shift+E opens the Export menu; its first item, PNG, exports the whole diagram.
    click(&mut rig.ui, Point::new(600.0, 500.0));
    key(&mut rig.ui, Key::Character("e".into()), command(true));
    assert!(got.borrow().is_empty(), "the shortcut opens the menu, it does not export yet");
    let png = named(&mut rig.ui, "PNG image").expect("the menu is open").id;
    rig.ui.perform_action(png, ravenrs_core::AccessAction::Click);
    settle(&mut rig.ui);
    {
        let got = got.borrow();
        let (format, picture) = got.last().expect("an export");
        assert_eq!(*format, FlowExportFormat::Png);
        let words: Vec<&str> = picture.texts.iter().map(|t| t.text.as_str()).collect();
        assert!(words.contains(&"New sale") && words.contains(&"Send refusal"));
    }
    // Nothing changed in the diagram.
    assert_eq!(rig.graph.get(), graph());
}

#[test]
fn without_on_export_there_is_no_export_button() {
    let mut rig = rig();
    assert!(named(&mut rig.ui, "Export").is_none());
}

/// What a test file chooser was asked to save: names and bytes.
type Saved = std::rc::Rc<std::cell::RefCell<Vec<(String, Vec<u8>)>>>;

/// A file chooser that hands over one file, and remembers what it was asked to save.
struct Files {
    open: Option<PickedFile>,
    saved: Saved,
}

impl FilePickerProvider for Files {
    fn pick(&self, _request: &FileRequest, done: Box<dyn FnOnce(Vec<PickedFile>)>) {
        done(self.open.clone().into_iter().collect());
    }

    fn save(&self, name: &str, bytes: &[u8]) {
        self.saved.borrow_mut().push((name.to_owned(), bytes.to_vec()));
    }
}

struct Desk {
    ui: Ui,
    graph: State<FlowGraph>,
    history: State<FlowHistory>,
    saved: Saved,
}

fn desk(storage: std::rc::Rc<ravenrs_core::MemoryStorage>, open: Option<PickedFile>) -> Desk {
    let graph = State::new(graph());
    let history = State::new(FlowHistory::new());
    let library = FlowLibrary::new(graph.binding()).history(history.binding()).storage(storage);
    let canvas = FlowCanvas::new(graph.binding()).history(history.binding()).size(700.0, 600.0);
    let mut ui = Ui::new(HStack::new((library, canvas)), Theme::raven_dark());
    let saved = std::rc::Rc::default();
    ui.set_file_picker(Files { open, saved: std::rc::Rc::clone(&saved) });
    ui.set_viewport(Size::new(940.0, 600.0), 1.0);
    settle(&mut ui);
    Desk { ui, graph, history, saved }
}

fn press(ui: &mut Ui, name: &str) {
    let id = named(ui, name).unwrap_or_else(|| panic!("no {name}")).id;
    ui.perform_action(id, ravenrs_core::AccessAction::Click);
    settle(ui);
}

#[test]
fn a_diagram_is_saved_and_says_when_it_has_changes_not_saved() {
    let storage = std::rc::Rc::new(ravenrs_core::MemoryStorage::default());
    let mut d = desk(storage.clone(), None);
    assert!(named(&mut d.ui, "Changes not saved").is_some(), "a diagram never kept");
    press(&mut d.ui, "Save");
    assert!(named(&mut d.ui, "Saved").is_some());
    let kept = ravenrs_core::Storage::read(&*storage, "flows").expect("readable").expect("kept");
    let shelf = FlowShelf::from_json(&String::from_utf8_lossy(&kept)).expect("a shelf");
    assert_eq!(shelf.documents.len(), 1);
    assert_eq!(shelf.documents[0].graph, graph());

    d.graph.update(|g| g.nodes[0].title = "Changed".into());
    settle(&mut d.ui);
    assert!(named(&mut d.ui, "Changes not saved").is_some(), "an edit is noticed");

    // The app opens again on the same storage: the diagram is there to open and edit.
    let mut again = desk(storage, None);
    assert!(named(&mut again.ui, "Untitled").is_some() || named(&mut again.ui, "New sale").is_some());
}

#[test]
fn new_and_switching_keep_what_was_open_and_start_the_history_over() {
    let storage = std::rc::Rc::new(ravenrs_core::MemoryStorage::default());
    let mut d = desk(storage.clone(), None);
    d.history.update(|h| h.record(FlowGraph::new()));
    press(&mut d.ui, "New");
    assert!(d.graph.get().nodes.is_empty(), "an empty diagram");
    assert!(!d.history.get().can_undo(), "with a history of its own");
    let shelf = FlowShelf::from_json(&String::from_utf8_lossy(
        &ravenrs_core::Storage::read(&*storage, "flows").unwrap().unwrap(),
    ))
    .unwrap();
    assert_eq!(shelf.documents.len(), 1, "the one open before was kept first");

    // Back to the first one.
    assert!(starting(&mut d.ui, "Untitled 2 ").is_some(), "the new one is told apart");
    let first = starting(&mut d.ui, "Untitled 3 nodes").expect("the first, listed").id;
    d.ui.perform_action(first, ravenrs_core::AccessAction::Click);
    settle(&mut d.ui);
    assert_eq!(d.graph.get(), graph());
}

fn starting(ui: &mut Ui, prefix: &str) -> Option<AccessNode> {
    ui.access_tree().nodes.iter().find(|n| n.name.as_deref().is_some_and(|x| x.starts_with(prefix))).cloned()
}

#[test]
fn delete_asks_first_and_a_file_goes_out_and_comes_in() {
    let storage = std::rc::Rc::new(ravenrs_core::MemoryStorage::default());
    let incoming = PickedFile::new("Pedidos.json", graph().to_json().into_bytes());
    let mut d = desk(storage.clone(), Some(incoming));
    press(&mut d.ui, "Save");

    press(&mut d.ui, "Save as file");
    let saved = d.saved.borrow().clone();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].0, "Untitled.json");
    assert_eq!(FlowGraph::from_json(&String::from_utf8_lossy(&saved[0].1)).ok(), Some(graph()));

    press(&mut d.ui, "Open file");
    assert!(starting(&mut d.ui, "Pedidos").is_some(), "the file opens as a diagram named after it");

    press(&mut d.ui, "Delete Untitled");
    assert!(named(&mut d.ui, "Delete “Untitled”?").is_some(), "it asks first");
}

#[test]
fn a_model_held_still_starts_carrying_and_a_quick_move_does_not() {
    let mut shop = shop();
    let before = shop.graph.get().nodes.len();
    let tile = named(&mut shop.ui, "Step").expect("the model").bounds.center();
    let left = named(&mut shop.ui, "New sale").expect("a node").bounds.origin.x - 48.0;

    // Held still: once the time passes, it is carried, and the platform stops scrolling.
    shop.ui.set_time(10.0);
    shop.ui.handle_event(&Event::PointerMove { position: tile });
    shop.ui.handle_event(&Event::PointerDown { position: tile, button: PointerButton::Primary });
    settle(&mut shop.ui);
    assert!(!shop.ui.is_dragging(), "not yet");
    assert!(shop.ui.next_wake().is_some_and(|w| w <= 10.0 + HOLD_TO_CARRY + 0.001), "it asks to be woken");
    shop.ui.set_time(10.0 + HOLD_TO_CARRY + 0.01);
    settle(&mut shop.ui);
    assert!(shop.ui.is_dragging(), "held long enough, it is carried");
    let to = Point::new(left + 300.0, 300.0);
    shop.ui.handle_event(&Event::PointerMove { position: to });
    settle(&mut shop.ui);
    shop.ui.handle_event(&Event::PointerUp { position: to, button: PointerButton::Primary });
    settle(&mut shop.ui);
    assert_eq!(shop.graph.get().nodes.len(), before + 1, "and it lands");

    // A touch that becomes a scroll (the platform sends PointerLeave) carries nothing later.
    shop.ui.set_time(20.0);
    shop.ui.handle_event(&Event::PointerMove { position: tile });
    shop.ui.handle_event(&Event::PointerDown { position: tile, button: PointerButton::Primary });
    shop.ui.handle_event(&Event::PointerLeave);
    shop.ui.set_time(21.0);
    settle(&mut shop.ui);
    assert!(!shop.ui.is_dragging(), "the press was given up");
}

#[test]
fn starters_show_only_while_nothing_is_kept() {
    let storage = std::rc::Rc::new(ravenrs_core::MemoryStorage::default());
    let make = |storage: std::rc::Rc<ravenrs_core::MemoryStorage>| {
        let graph = State::new(graph());
        let library = FlowLibrary::new(graph.binding())
            .storage(storage)
            .starter("Exemplo", graph.get())
            .starter("Vazio", FlowGraph::new());
        let mut ui = Ui::new(library, Theme::raven_dark());
        ui.set_viewport(Size::new(300.0, 600.0), 1.0);
        settle(&mut ui);
        ui
    };
    let mut ui = make(storage.clone());
    assert!(starting(&mut ui, "Exemplo").is_some() && starting(&mut ui, "Vazio").is_some());
    assert!(starting(&mut ui, "Untitled").is_none(), "the open diagram is the starter it equals");
    press(&mut ui, "Delete Vazio");
    let confirm = ui
        .access_tree()
        .nodes
        .iter()
        .rfind(|n| n.name.as_deref() == Some("Delete"))
        .map(|n| n.id)
        .expect("the confirm button");
    ui.perform_action(confirm, ravenrs_core::AccessAction::Click);
    settle(&mut ui);
    // Something is kept now: starting again shows what was kept, not the starters.
    let mut again = make(storage);
    assert!(starting(&mut again, "Exemplo").is_some());
    assert!(starting(&mut again, "Vazio").is_none(), "the deleted starter does not come back");
}

mod cables {
    use super::*;

    fn tied() -> FlowGraph {
        graph().edge(FlowEdge::new("e1", FlowEndpoint::new("a", "out"), FlowEndpoint::new("b", "in")))
    }

    /// Runs the clock for `seconds` at 60 frames a second, from `from`.
    fn run(ui: &mut Ui, from: f64, seconds: f64) -> f64 {
        let mut t = from;
        while t < from + seconds {
            t += 1.0 / 60.0;
            ui.set_time(t);
            ui.update();
        }
        t
    }

    fn lowest(ui: &Ui) -> f32 {
        let id = ui.cables().ids()[0];
        ui.cables().shape(id).expect("a cable").iter().map(|p| p.y).fold(f32::MIN, f32::max)
    }

    #[test]
    fn without_cables_the_lines_are_those_of_today() {
        let plain = rig_with(tied(), |c| c);
        let off = rig_with(tied(), |c| c.cables(false));
        assert_eq!(outlines(&plain.ui), outlines(&off.ui));
        assert!(plain.ui.cables().ids().is_empty());
    }

    #[test]
    fn connections_there_from_the_start_hang_at_rest_and_ask_for_no_frames() {
        let mut rig = rig_with(tied(), |c| c.cables(true));
        rig.ui.set_time(0.0);
        settle(&mut rig.ui);
        assert_eq!(rig.ui.cables().ids().len(), 1);
        assert_eq!(rig.ui.next_wake(), None, "at rest from the start");
        assert!(lowest(&rig.ui) > 70.0, "it hangs below the ports");
    }

    #[test]
    fn a_new_connection_drops_and_settles_and_then_the_canvas_sleeps() {
        let mut rig = rig_with(graph(), |c| c.cables(true));
        rig.ui.set_time(0.0);
        settle(&mut rig.ui);
        assert_eq!(rig.ui.next_wake(), None);
        rig.graph.set(tied());
        settle(&mut rig.ui);
        assert!(rig.ui.next_wake().is_some(), "the new cable drops");
        let t = run(&mut rig.ui, 0.0, 1.0);
        assert_eq!(rig.ui.next_wake(), None, "settled in under a second");
        assert!(lowest(&rig.ui) > 70.0);
        let _ = t;
    }

    #[test]
    fn moving_a_node_swings_its_cable_until_it_settles() {
        let mut rig = rig_with(tied(), |c| c.cables(true));
        rig.ui.set_time(0.0);
        settle(&mut rig.ui);
        let mut g = rig.graph.get();
        g.nodes.iter_mut().find(|n| n.id == "b").expect("b").y += 160.0;
        rig.graph.set(g);
        settle(&mut rig.ui);
        let t = run(&mut rig.ui, 0.0, 0.1);
        assert!(rig.ui.next_wake().is_some(), "it swings");
        run(&mut rig.ui, t, 2.0);
        assert_eq!(rig.ui.next_wake(), None, "and settles");
    }

    #[test]
    fn panning_the_view_moves_the_cables_without_shaking_them() {
        let mut rig = rig_with(tied(), |c| c.cables(true));
        rig.ui.set_time(0.0);
        settle(&mut rig.ui);
        let before = rig.ui.cables().shape(rig.ui.cables().ids()[0]).expect("a cable");
        rig.view.set(FlowViewport::at(Point::new(60.0, 40.0), 1.5));
        settle(&mut rig.ui);
        assert_eq!(rig.ui.next_wake(), None);
        let after = rig.ui.cables().shape(rig.ui.cables().ids()[0]).expect("a cable");
        assert_eq!(before, after, "the cable keeps the diagram's units");
    }

    #[test]
    fn reduced_motion_shows_new_cables_at_rest_without_frames() {
        let mut rig = rig_with(graph(), |c| c.cables(true));
        rig.ui.set_reduced_motion(true);
        rig.ui.set_time(0.0);
        settle(&mut rig.ui);
        rig.graph.set(tied());
        settle(&mut rig.ui);
        assert_eq!(rig.ui.next_wake(), None);
        assert!(lowest(&rig.ui) > 70.0);
    }

    #[test]
    fn a_click_on_the_hanging_cable_selects_its_connection() {
        let mut rig = rig_with(tied(), |c| c.cables(true));
        rig.ui.set_time(0.0);
        settle(&mut rig.ui);
        let shape = rig.ui.cables().shape(rig.ui.cables().ids()[0]).expect("a cable");
        let middle = rig.view.get().to_screen(shape[shape.len() / 2]);
        click(&mut rig.ui, middle);
        assert_eq!(rig.selection.get().edges, vec!["e1".to_string()]);
    }

    #[test]
    fn the_export_draws_the_cables_at_rest() {
        let mut g = tied();
        g.nodes.retain(|n| n.id != "c");
        // Far apart, so the belly hangs below the cards.
        g.nodes.iter_mut().find(|n| n.id == "b").expect("b").x = 900.0;
        let flat = g.picture(None, FlowColors::paper()).expect("a picture");
        let hanging = g.picture_with(None, FlowColors::paper(), true).expect("a picture");
        assert!(hanging.bounds.height() > flat.bounds.height(), "the cable's belly makes it taller");
        assert_eq!(
            hanging,
            g.picture_with(None, FlowColors::paper(), true).expect("a picture"),
            "the same every time"
        );
    }
}

#[test]
fn a_run_told_from_outside_marks_the_nodes_and_counts_the_items_without_touching_the_graph() {
    let line = FlowEdge::new("e1", FlowEndpoint::new("a", "out"), FlowEndpoint::new("b", "in")).label("sale");
    let run = FlowRunView::new()
        .node("a", FlowNodeRun::new(FlowNodeStatus::Success).items("out", 3))
        .node("b", FlowNodeRun::new(FlowNodeStatus::Waiting("Waiting for approval".into())))
        .node("c", FlowNodeRun::new(FlowNodeStatus::Skipped));
    let before = graph().edge(line);
    let mut rig = rig_with(before.clone(), move |c| c.execution(run));
    let said = |ui: &mut Ui, title: &str| {
        let tree = ui.access_tree();
        let node = tree.nodes.iter().find(|n| n.name.as_deref() == Some(title)).expect(title);
        node.description.clone().unwrap_or_default()
    };
    assert!(said(&mut rig.ui, "New sale").contains("Done"), "{}", said(&mut rig.ui, "New sale"));
    assert!(said(&mut rig.ui, "Check stock").contains("Waiting for approval"));
    assert!(said(&mut rig.ui, "Send refusal").contains("Did not run"), "the run's status over its own");
    assert!(named(&mut rig.ui, "sale · 3 items").is_some(), "the count after the line's own words");
    assert_eq!(rig.graph.get(), before, "the drawing changes, the graph does not");

    let one = FlowRunView::new().node("a", FlowNodeRun::new(FlowNodeStatus::Pinned).items("out", 1));
    assert_eq!(one.items(&FlowEndpoint::new("a", "out")), Some(1));
    assert_eq!(one.items(&FlowEndpoint::new("b", "out")), None, "a node that did nothing says nothing");
    let waiting =
        FlowRunView::new().node("a", FlowNodeRun::new(FlowNodeStatus::Waiting("x".into())).items("out", 5));
    assert_eq!(
        waiting.items(&FlowEndpoint::new("a", "out")),
        None,
        "a node that waits has given nothing yet"
    );
}

#[test]
fn a_read_only_canvas_offers_no_unlock_and_changes_nothing() {
    let mut rig = rig_with(graph(), |c| c.read_only(true));
    assert!(named(&mut rig.ui, "Unlock editing").is_none(), "no way out of the lock");
    assert!(named(&mut rig.ui, "Lock editing").is_none());
    drag(&mut rig.ui, Point::new(120.0, 70.0), Point::new(220.0, 170.0));
    assert_eq!(node_at(&rig, "a"), (48.0, 48.0));
    key(&mut rig.ui, Key::Delete, Modifiers::default());
    assert_eq!(rig.graph.get().nodes.len(), 3);
    double_click(&mut rig.ui, Point::new(120.0, 70.0));
    assert!(field(&mut rig.ui).is_none(), "no title to type in");
    drag(&mut rig.ui, Point::new(300.0, 500.0), Point::new(310.0, 500.0));
    assert_eq!(rig.view.get().offset, Point::new(10.0, 0.0), "the view still pans");
    click(&mut rig.ui, Point::new(120.0, 70.0));
    assert_eq!(rig.selection.get().nodes, ["a"], "and a node can still be picked to read");
}

#[test]
fn a_connection_let_go_in_the_open_offers_what_the_port_takes_and_creates_it_connected() {
    let models = [
        (
            "Check again",
            FlowNode::new("again", "action", "Check again", 0.0, 0.0)
                .port(FlowPort::input("in").kind("sale"))
                .port(FlowPort::output("out").kind("sale")),
        ),
        (
            "Refuse",
            FlowNode::new("refuse", "action", "Refuse", 0.0, 0.0).port(FlowPort::input("in").kind("refused")),
        ),
        (
            "New trigger",
            FlowNode::new("start", "trigger", "New trigger", 0.0, 0.0).port(FlowPort::output("out")),
        ),
    ];
    let mut rig = rig_with(graph(), move |c| c.open_end(models.clone()));
    drag(&mut rig.ui, Point::new(248.0, 90.0), Point::new(620.0, 480.0));
    assert!(named(&mut rig.ui, "Find the next node").is_some(), "the search opens where it was let go");
    assert!(named(&mut rig.ui, "Check again").is_some(), "a model the port takes");
    assert!(named(&mut rig.ui, "Refuse").is_none(), "one the rules refuse is left out");
    assert!(named(&mut rig.ui, "New trigger").is_none(), "and one with no way in");

    let id = named(&mut rig.ui, "Check again").expect("listed").id;
    rig.ui.perform_action(id, ravenrs_core::AccessAction::Click);
    settle(&mut rig.ui);
    let g = rig.graph.get();
    assert_eq!(g.nodes.len(), 4, "the node is there");
    assert_eq!(g.edges.len(), 1, "and connected");
    assert_eq!((g.edges[0].from.node.as_str(), g.edges[0].to.port.as_str()), ("a", "in"));
    assert!(named(&mut rig.ui, "Find the next node").is_none(), "the search closed");
    key(&mut rig.ui, Key::Character("z".into()), Modifiers { meta: true, ..Modifiers::default() });
    assert_eq!((rig.graph.get().nodes.len(), rig.graph.get().edges.len()), (3, 0), "one Undo takes both");

    drag(&mut rig.ui, Point::new(248.0, 90.0), Point::new(620.0, 480.0));
    key(&mut rig.ui, Key::Escape, Modifiers::default());
    assert!(named(&mut rig.ui, "Find the next node").is_none(), "Escape closes it");
    assert_eq!(rig.graph.get().nodes.len(), 3, "and creates nothing");
}

#[test]
fn without_models_a_connection_let_go_in_the_open_is_given_up_as_before() {
    let mut rig = rig();
    drag(&mut rig.ui, Point::new(248.0, 90.0), Point::new(620.0, 480.0));
    assert!(named(&mut rig.ui, "Find the next node").is_none());
    assert!(rig.graph.get().edges.is_empty());
}

#[test]
fn every_node_is_found_by_its_key() {
    let mut r = rig();
    let tree = r.ui.access_tree();
    for (id, title) in [("a", "New sale"), ("b", "Check stock"), ("c", "Send refusal")] {
        let node = tree.by_test_id(&format!("node:{id}")).unwrap_or_else(|| panic!("node:{id} is tagged"));
        assert_eq!(node.name.as_deref(), Some(title));
    }
}
