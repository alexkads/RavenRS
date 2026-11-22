//! A run of the runtime as the canvas is told it: each node's part becomes a `FlowNodeRun`,
//! with its status and how many items left by each port, and the canvas draws it with
//! `FlowCanvas::execution`, on a copy, never touching the flow.

use std::collections::BTreeMap;

use ravenrs::prelude::{FlowNodeRun, FlowNodeStatus, FlowRunView};
use ravenrs_flows::{NodeRun, NodeStatus};

/// The run whose nodes did `parts`, as the canvas draws it.
pub fn run_view(parts: &BTreeMap<String, NodeRun>) -> FlowRunView {
    parts.iter().fold(FlowRunView::new(), |view, (id, part)| {
        let run = part
            .outputs
            .iter()
            .fold(FlowNodeRun::new(status(part)), |run, (port, items)| run.items(port.as_str(), items.len()));
        view.node(id.as_str(), run)
    })
}

/// How a node's part shows on its card.
fn status(part: &NodeRun) -> FlowNodeStatus {
    let why = || part.error.clone().unwrap_or_else(|| crate::casa::node_words(part.status).to_owned());
    match part.status {
        NodeStatus::Done | NodeStatus::Reused => FlowNodeStatus::Success,
        NodeStatus::Pinned => FlowNodeStatus::Pinned,
        NodeStatus::Failed | NodeStatus::TimedOut | NodeStatus::Uncertain => FlowNodeStatus::Error(why()),
        NodeStatus::Waiting => FlowNodeStatus::Waiting("Aguardando".to_owned()),
        NodeStatus::Skipped => FlowNodeStatus::Skipped,
        NodeStatus::Cancelled => FlowNodeStatus::Cancelled,
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
