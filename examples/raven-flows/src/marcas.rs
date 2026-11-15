//! A run drawn on the diagram, with what the canvas already has: each node's
//! `FlowNodeStatus` and each connection's `label` with the number of items that went along it.
//! No new piece of the canvas: a node that waits is a warning that says so, and a node that did
//! not run keeps its plain look.

use std::collections::BTreeMap;

use ravenrs::prelude::{FlowGraph, FlowNodeStatus};
use ravenrs_flows::{NodeRun, NodeStatus};

/// `graph` with the marks of a run whose nodes did `parts`; with no parts, the marks of an
/// earlier run are taken off. A node switched off keeps its look either way. A node that did
/// not run, or still waits, puts no count on what leaves it.
pub fn marked(graph: &FlowGraph, parts: &BTreeMap<String, NodeRun>) -> FlowGraph {
    let mut next = graph.clone();
    for node in &mut next.nodes {
        if node.status == FlowNodeStatus::Disabled {
            continue;
        }
        node.status = match parts.get(&node.id) {
            None => FlowNodeStatus::Normal,
            Some(part) => status(part),
        };
    }
    for edge in &mut next.edges {
        edge.label = parts
            .get(&edge.from.node)
            .filter(|part| !matches!(part.status, NodeStatus::Skipped | NodeStatus::Waiting))
            .map(|part| part.outputs.get(&edge.from.port).map_or(0, Vec::len))
            .map(items);
    }
    next
}

/// How a node's part shows on its card.
fn status(part: &NodeRun) -> FlowNodeStatus {
    let why = || part.error.clone().unwrap_or_else(|| crate::casa::node_words(part.status).to_owned());
    match part.status {
        NodeStatus::Done | NodeStatus::Pinned | NodeStatus::Reused => FlowNodeStatus::Success,
        NodeStatus::Failed | NodeStatus::TimedOut | NodeStatus::Uncertain => FlowNodeStatus::Error(why()),
        NodeStatus::Waiting => FlowNodeStatus::Warning("Aguardando".to_owned()),
        NodeStatus::Disabled => FlowNodeStatus::Disabled,
        _ => FlowNodeStatus::Normal,
    }
}

/// "1 item", "4 itens".
pub fn items(n: usize) -> String {
    if n == 1 { "1 item".to_owned() } else { format!("{n} itens") }
}

/// How many items a node gave, on all its outputs.
pub fn given(part: &NodeRun) -> usize {
    part.outputs.values().map(Vec::len).sum()
}
