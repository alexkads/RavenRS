//! The whiteboard through its interface: a flowchart whose loop goes round through bend
//! points, and a mind map that arranges itself as a tree.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use ravenrs::core::AccessAction;
use ravenrs::prelude::*;

fn opened() -> Ui {
    let mut ui = Ui::new(flow_whiteboard::app(), Theme::raven_light());
    ui.set_viewport(Size::new(1280.0, 800.0), 1.0);
    ui.update();
    ui.update();
    ui
}

fn shows(ui: &mut Ui, name: &str) -> bool {
    ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some(name))
}

#[test]
fn the_loop_back_goes_round_through_its_points() {
    let graph = flow_whiteboard::flowchart();
    let back = graph.find_edge("e").expect("the loop");
    assert_eq!(back.waypoints.len(), 3);
    // Its first point is right of every shape it goes round, so the line does not cross them.
    let (x, _) = back.waypoints[0];
    let shapes = ["check", "enough", "buy"];
    assert!(shapes.iter().all(|id| graph.find_node(id).is_some_and(|n| n.x + n.width < x)));
    assert!(graph.edge_path(back).is_some(), "and it is drawn");
}

#[test]
fn the_mind_map_is_a_tree_whose_ideas_never_overlap() {
    let graph = flow_whiteboard::mind_map();
    let root = graph.find_node("root").expect("the middle idea");
    for node in graph.nodes.iter().filter(|n| n.id != "root") {
        assert!(node.x > root.x, "{} grows to the right", node.id);
    }
    for (i, a) in graph.nodes.iter().enumerate() {
        for b in graph.nodes.iter().skip(i + 1) {
            let (ra, rb) = (a.extent(), b.extent());
            let apart =
                ra.max_x() <= rb.x() || rb.max_x() <= ra.x() || ra.max_y() <= rb.y() || rb.max_y() <= ra.y();
            assert!(apart, "{} and {} overlap", a.id, b.id);
        }
    }
}

#[test]
fn the_board_switches_between_flowchart_and_mind_map() {
    let mut ui = opened();
    assert!(shows(&mut ui, "Confere estoque"));
    let tab = ui
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some("Mapa mental") && n.role != Role::Group)
        .expect("the switch")
        .id;
    ui.perform_action(tab, AccessAction::Click);
    ui.update();
    ui.update();
    assert!(shows(&mut ui, "Lançamento"), "the mind map");
    assert!(!shows(&mut ui, "Confere estoque"));
}

#[test]
fn what_comes_next_depends_on_the_board() {
    let idea = flow_whiteboard::mind_map().find_node("site").cloned().expect("an idea");
    assert_eq!(flow_whiteboard::next(&idea, PortSide::Right).map(|n| n.kind), Some("idea".into()));
    let step = flow_whiteboard::flowchart().find_node("check").cloned().expect("a step");
    assert_eq!(flow_whiteboard::next(&step, PortSide::Bottom).map(|n| n.kind), Some("step".into()));
}
