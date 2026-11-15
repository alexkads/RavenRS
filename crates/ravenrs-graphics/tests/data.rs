//! The acceptance of #529: the app's data rides with the diagram, through undo, paste and JSON,
//! with no window.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use ravenrs_graphics::flow::{
    FLOW_FORMAT, FlowEdge, FlowEndpoint, FlowGraph, FlowHistory, FlowJsonError, FlowNode, FlowPort, NODE_REFS,
};
use serde_json::json;

fn node(id: &str, data: serde_json::Value) -> FlowNode {
    FlowNode {
        data,
        ..FlowNode::new(id, "step", id, 0.0, 0.0).port(FlowPort::input("in")).port(FlowPort::output("out"))
    }
}

#[test]
fn undoing_a_change_of_a_parameter_brings_the_parameter_back_with_the_place() {
    let mut graph = FlowGraph::new().node(node("a", json!({ "url": "https://old" })));
    let mut history = FlowHistory::new();

    history.record(graph.clone());
    graph.nodes[0].data["url"] = json!("https://new");
    graph.nodes[0].x = 300.0;

    let before = history.undo(graph.clone()).expect("one change to undo");
    assert_eq!(before.nodes[0].data, json!({ "url": "https://old" }));
    assert_eq!(before.nodes[0].x, 0.0, "the move and the parameter were one change");
    let after = history.redo(before).expect("and to redo");
    assert_eq!(after.nodes[0].data["url"], json!("https://new"));
}

#[test]
fn pasting_two_nodes_where_one_names_the_other_renames_the_name_to_the_new_id() {
    let mut graph = FlowGraph::new()
        .node(node("check", json!({ "on_fail": "notify", NODE_REFS: ["/on_fail"] })))
        .node(node("notify", json!({ "to": "ops" })))
        .node(node("outside", json!({})));
    let piece = graph.extract(&["check".to_owned(), "notify".to_owned()]);

    let (nodes, _) = graph.insert(&piece, 40.0, 40.0);
    let (check, notify) = (&nodes[0], &nodes[1]);
    assert!(check != "check" && notify != "notify", "the copies have ids of their own: {nodes:?}");
    let copy = graph.find_node(check).expect("pasted");
    assert_eq!(copy.data["on_fail"], json!(notify), "it names the copy, not the node it was copied from");
    assert_eq!(graph.find_node("check").expect("still there").data["on_fail"], json!("notify"));
}

#[test]
fn a_name_of_a_node_not_pasted_with_it_is_left_as_it_was() {
    let mut graph = FlowGraph::new()
        .node(node("check", json!({ "on_fail": "outside", NODE_REFS: ["/on_fail", "/nowhere"] })))
        .node(node("outside", json!({})));
    let piece = graph.extract(&["check".to_owned()]);
    let (nodes, _) = graph.insert(&piece, 10.0, 0.0);
    assert_eq!(graph.find_node(&nodes[0]).expect("pasted").data["on_fail"], json!("outside"));
}

#[test]
fn the_app_renames_what_the_convention_does_not_cover_with_its_own_hook() {
    let mut graph = FlowGraph::new()
        .node(node("a", json!({ "steps": "b;c" })))
        .node(node("b", json!({})))
        .node(node("c", json!({})));
    let piece = graph.extract(&["a".to_owned(), "b".to_owned(), "c".to_owned()]);
    let (nodes, _) = graph.insert_with(&piece, 0.0, 100.0, |copy, renamed| {
        if let Some(steps) = copy.data["steps"].as_str() {
            let new: Vec<String> =
                steps.split(';').map(|id| renamed(id).unwrap_or_else(|| id.to_owned())).collect();
            copy.data["steps"] = json!(new.join(";"));
        }
    });
    assert_eq!(
        graph.find_node(&nodes[0]).expect("pasted").data["steps"],
        json!(format!("{};{}", nodes[1], nodes[2]))
    );
}

#[test]
fn json_of_the_version_before_opens_and_is_written_at_this_version() {
    let before = r#"{
        "nodes": [
            { "id": "a", "kind": "step", "title": "A", "x": 0, "y": 0, "width": 200, "height": 84,
              "ports": [{ "id": "out", "side": "Right", "direction": "Output" }] },
            { "id": "b", "kind": "step", "title": "B", "x": 300, "y": 0, "width": 200, "height": 84,
              "ports": [{ "id": "in", "side": "Left", "direction": "Input" }] }
        ],
        "edges": [{ "id": "e", "from": { "node": "a", "port": "out" }, "to": { "node": "b", "port": "in" } }]
    }"#;
    let graph = FlowGraph::from_json(before).expect("version 1 still opens");
    assert_eq!(graph.nodes.len(), 2);
    assert!(graph.nodes[0].data.is_null());

    let written: serde_json::Value = serde_json::from_str(&graph.to_json()).expect("JSON");
    assert_eq!(written["format"], json!(FLOW_FORMAT));
    assert_eq!(FlowGraph::from_json(&graph.to_json()), Ok(graph));
}

#[test]
fn data_the_app_does_not_know_comes_back_as_it_was() {
    let odd = json!({
        "plugin": { "name": "made elsewhere", "settings": [1, 2.5, null, { "deep": [true] }] },
        "big": 12_345_678_901_234_567u64
    });
    let mut graph = FlowGraph::new().node(node("a", odd.clone())).node(node("b", json!({})));
    graph.data = json!({ "owner": "ana", "revision": 7 });
    graph.edges.push(FlowEdge::new("e", FlowEndpoint::new("a", "out"), FlowEndpoint::new("b", "in")));

    let read = FlowGraph::from_json(&graph.to_json()).expect("reads back");
    assert_eq!(read.nodes[0].data, odd);
    assert_eq!(read.data, json!({ "owner": "ana", "revision": 7 }));
    assert_eq!(read, graph);
}

#[test]
fn json_of_a_newer_version_is_refused_so_nothing_is_dropped() {
    let newer = json!({ "format": FLOW_FORMAT + 1, "nodes": [], "edges": [] }).to_string();
    let refused = FlowGraph::from_json(&newer).expect_err("a newer build wrote it");
    assert_eq!(refused, FlowJsonError::Newer(FLOW_FORMAT + 1));
    assert!(refused.to_string().contains(&FLOW_FORMAT.to_string()));
}

#[test]
fn a_node_without_data_writes_none() {
    let graph = FlowGraph::new().node(FlowNode::new("a", "step", "A", 0.0, 0.0));
    assert!(!graph.to_json().contains("\"data\""), "no field for data nobody gave");
}
