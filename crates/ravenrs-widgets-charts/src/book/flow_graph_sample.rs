//! A small diagram for the stories of the panels that work on a `FlowCanvas`'s graph: the same
//! sale the gallery's `FlowCanvas` shows, cut to three steps.

use crate::{FlowEdge, FlowEndpoint, FlowGraph, FlowNode, FlowPort};

/// A sale, a check and a receipt, joined.
pub(super) fn graph() -> FlowGraph {
    FlowGraph::new()
        .node(
            FlowNode::new("sale", "trigger", "New sale", 0.0, 96.0)
                .subtitle("Registers 1 and 2")
                .port(FlowPort::output("out").kind("sale")),
        )
        .node(
            FlowNode::new("stock", "action", "Check stock", 264.0, 96.0)
                .port(FlowPort::input("in").kind("sale"))
                .port(FlowPort::output("out").kind("sale")),
        )
        .node(
            FlowNode::new("invoice", "action", "Issue receipt", 528.0, 96.0)
                .port(FlowPort::input("in").kind("sale")),
        )
        .edge(FlowEdge::new("e1", FlowEndpoint::new("sale", "out"), FlowEndpoint::new("stock", "in")))
        .edge(FlowEdge::new("e2", FlowEndpoint::new("stock", "out"), FlowEndpoint::new("invoice", "in")))
}
