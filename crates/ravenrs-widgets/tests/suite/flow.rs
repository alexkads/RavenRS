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
