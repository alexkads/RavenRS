//! The bridge to the canvas: a flow drawn as a [`FlowGraph`] for the `FlowCanvas`, the drawing
//! read back into the flow, and the [`FlowRunRules`] its debugger follows. Each node's setup
//! rides in its card's `data` and the flow's name in the graph's, so copying, pasting and undoing
//! on the canvas keep them; the drawing itself reads none of it, so no automation rule goes into
//! the geometry. Above `definition` and `catalog`.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use ravenrs_graphics::flow::{
    FlowEdge, FlowEndpoint, FlowGate, FlowGraph, FlowNode, FlowNodeStatus, FlowPort, FlowRunRules,
};

use serde_json::{Value, json};

use crate::catalog::Catalog;
use crate::definition::{Connection, FORMAT, FlowDefinition, NodeDef, PortRef};

/// The field of a card's `data`, and of the graph's, that holds what a flow keeps there.
pub const GRAPH_KEY: &str = "flow";

/// How far apart [`FlowDefinition::to_graph`] puts the nodes it lays out.
const STEP: (f32, f32) = (260.0, 120.0);

fn title_of(node: &NodeDef, catalog: &Catalog) -> String {
    node.title.clone().unwrap_or_else(|| match catalog.get(&node.kind, node.version) {
        Some(kind) => kind.spec().title,
        None => node.kind.clone(),
    })
}

fn edge_id(c: &Connection) -> String {
    format!("{}.{}>{}.{}", c.from.node, c.from.port, c.to.node, c.to.port)
}

impl FlowDefinition {
    /// The flow drawn for a `FlowCanvas`: a node card per node, with the ports of its kind,
    /// titled by its title or its kind's, shown as disabled when it is, and the node's whole
    /// setup in its `data`; one connection per connection; the flow's id, name and revision in
    /// the graph's `data`. The nodes are laid out in columns by how far they are from a start,
    /// which the app then moves as the person draws.
    pub fn to_graph(&self, catalog: &Catalog) -> FlowGraph {
        let depth = depths(self);
        let mut rows: BTreeMap<usize, usize> = BTreeMap::new();
        let mut graph = FlowGraph::new();
        graph.data = json!({ GRAPH_KEY: { "id": self.id, "name": self.name, "revision": self.revision } });
        for node in &self.nodes {
            let col = depth.get(node.id.as_str()).copied().unwrap_or(0);
            let row = rows.entry(col).or_default();
            let mut card = FlowNode::new(
                &node.id,
                &node.kind,
                title_of(node, catalog),
                col as f32 * STEP.0,
                *row as f32 * STEP.1,
            );
            *row += 1;
            if let Some(kind) = catalog.get(&node.kind, node.version) {
                let spec = kind.spec();
                for port in &spec.inputs {
                    card = card.port(FlowPort::input(port));
                }
                for port in &spec.outputs {
                    card = card.port(FlowPort::output(port));
                }
            }
            if node.disabled {
                card.status = FlowNodeStatus::Disabled;
            }
            card.data = json!({ GRAPH_KEY: node });
            graph.nodes.push(card);
        }
        for c in &self.connections {
            graph.edges.push(FlowEdge::new(
                edge_id(c),
                FlowEndpoint::new(&c.from.node, &c.from.port),
                FlowEndpoint::new(&c.to.node, &c.to.port),
            ));
        }
        graph
    }

    /// The flow as the person left the drawing: its id, name and revision from the graph's
    /// `data`, its nodes and connections the graph's, in the graph's order. Each node takes its
    /// setup from its card's `data`, so a pasted card brings the setup of the node it was copied
    /// from under its new id; a card with none (drawn from the toolbox) starts with no
    /// parameters. A title the person typed over the kind's is kept, and the disabled look sets
    /// `disabled`. Groups are drawing only, and are left out.
    pub fn from_graph(graph: &FlowGraph, catalog: &Catalog) -> FlowDefinition {
        let head = graph.data.get(GRAPH_KEY);
        let text =
            |key: &str| head.and_then(|h| h.get(key)).and_then(Value::as_str).unwrap_or_default().to_owned();
        let mut flow = FlowDefinition::new(text("id"), text("name"));
        flow.format = FORMAT;
        flow.revision = head.and_then(|h| h.get("revision")).and_then(Value::as_u64).unwrap_or(0) as u32;
        for card in graph.nodes.iter().filter(|n| !n.group) {
            let kept = card
                .data
                .get(GRAPH_KEY)
                .and_then(|setup| serde_json::from_value::<NodeDef>(setup.clone()).ok());
            let mut node = kept.unwrap_or_else(|| NodeDef::new(&card.id, &card.kind));
            node.id = card.id.clone();
            node.kind = card.kind.clone();
            node.title = None;
            if card.title != title_of(&node, catalog) {
                node.title = Some(card.title.clone());
            }
            node.disabled = card.status == FlowNodeStatus::Disabled;
            flow.nodes.push(node);
        }
        let kept: BTreeSet<&str> = flow.nodes.iter().map(|n| n.id.as_str()).collect();
        for edge in &graph.edges {
            if kept.contains(edge.from.node.as_str()) && kept.contains(edge.to.node.as_str()) {
                flow.connections.push(Connection {
                    from: PortRef::new(&edge.from.node, &edge.from.port),
                    to: PortRef::new(&edge.to.node, &edge.to.port),
                });
            }
        }
        flow
    }
}

impl Catalog {
    /// How the `FlowCanvas` debugger moves through `flow`: a trigger starts, a branch chooses
    /// one way out, a node that leads nowhere in the flow ends, any other node passes on; a node
    /// the flow does not have (a note) and connections it does not have are left out.
    ///
    /// The rules are taken from the flow as it is now: when the person changes the flow, ask
    /// for them again.
    pub fn run_rules(&self, flow: &FlowDefinition) -> FlowRunRules {
        let leads: BTreeSet<&str> = flow.connections.iter().map(|c| c.from.node.as_str()).collect();
        let gates: BTreeMap<String, FlowGate> = flow
            .nodes
            .iter()
            .map(|node| {
                let spec = self.get(&node.kind, node.version).map(|k| k.spec());
                let gate = match spec {
                    Some(s) if s.trigger => FlowGate::Start,
                    _ if !leads.contains(node.id.as_str()) => FlowGate::End,
                    Some(s) if s.chooses => FlowGate::Choose,
                    _ => FlowGate::Pass,
                };
                (node.id.clone(), gate)
            })
            .collect();
        let edges: BTreeSet<String> = flow.connections.iter().map(edge_id).collect();
        FlowRunRules {
            gate: Rc::new(move |node: &FlowNode| gates.get(&node.id).copied().unwrap_or(FlowGate::Outside)),
            follows: Rc::new(move |edge: &FlowEdge| {
                edges.contains(&format!(
                    "{}.{}>{}.{}",
                    edge.from.node, edge.from.port, edge.to.node, edge.to.port
                ))
            }),
        }
    }
}

/// How many connections each node is from a node nothing feeds, by the longest way, going
/// round no more than once.
fn depths(flow: &FlowDefinition) -> BTreeMap<&str, usize> {
    let mut depth: BTreeMap<&str, usize> = flow.nodes.iter().map(|n| (n.id.as_str(), 0)).collect();
    for _ in 0..flow.nodes.len() {
        let mut moved = false;
        for c in &flow.connections {
            let from = depth.get(c.from.node.as_str()).copied().unwrap_or(0);
            if let Some(to) = depth.get_mut(c.to.node.as_str())
                && *to < from + 1
                && from + 1 < flow.nodes.len()
            {
                *to = from + 1;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    depth
}
