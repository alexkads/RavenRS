//! A flow checked against the catalog and put in order: every node's kind found, every
//! connection between ports that exist, the required parameters present, and the nodes sorted
//! so that each runs after the ones that feed it. A cycle is refused unless it goes through a
//! loop node. Above `definition` and `catalog`; the run uses it.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::catalog::{Catalog, NodeSpec};
use crate::definition::{Connection, FlowDefinition};

/// Why a flow could not be put in order.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PlanError {
    /// Two nodes have the same id.
    DuplicateNode {
        /// The id.
        node: String,
    },
    /// A node's kind, at its version, is not in the catalog.
    UnknownKind {
        /// The node.
        node: String,
        /// Its kind.
        kind: String,
        /// The kind's version it was set up for.
        version: u32,
    },
    /// A connection names a node the flow does not have.
    UnknownNode {
        /// The id named.
        node: String,
    },
    /// A connection names a port its node's kind does not have, or uses an input as an output.
    UnknownPort {
        /// The node.
        node: String,
        /// The port named.
        port: String,
    },
    /// A node lacks a parameter its kind requires.
    MissingParameter {
        /// The node.
        node: String,
        /// The parameter.
        parameter: String,
    },
    /// The connections go round through no loop node: the nodes on the way, back to the first.
    Cycle {
        /// The nodes in order, the first repeated at the end.
        path: Vec<String>,
    },
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateNode { node } => write!(f, "two nodes are called {node}"),
            Self::UnknownKind { node, kind, version } => {
                write!(f, "node {node}: the kind {kind} (version {version}) is not in the catalog")
            }
            Self::UnknownNode { node } => {
                write!(f, "a connection names {node}, which the flow does not have")
            }
            Self::UnknownPort { node, port } => {
                write!(f, "node {node} has no port {port} for that connection")
            }
            Self::MissingParameter { node, parameter } => {
                write!(f, "node {node} needs the parameter {parameter}")
            }
            Self::Cycle { path } => {
                write!(f, "the connections go round with no loop node: {}", path.join(" → "))
            }
        }
    }
}

impl std::error::Error for PlanError {}

/// A flow ready to run: checked, with its nodes in the order they run.
#[derive(Clone, Debug)]
pub struct Plan {
    definition: FlowDefinition,
    specs: BTreeMap<String, NodeSpec>,
    order: Vec<String>,
    feeds: BTreeMap<String, Vec<Connection>>,
    back: Vec<Connection>,
    unreachable: Vec<String>,
}

impl Plan {
    /// Checks `definition` against `catalog` and puts it in order, or says everything wrong
    /// with it.
    pub fn compile(definition: &FlowDefinition, catalog: &Catalog) -> Result<Self, Vec<PlanError>> {
        let mut errors = Vec::new();
        let mut specs = BTreeMap::new();
        let mut index = BTreeMap::new();
        for (i, node) in definition.nodes.iter().enumerate() {
            if index.insert(node.id.clone(), i).is_some() {
                errors.push(PlanError::DuplicateNode { node: node.id.clone() });
                continue;
            }
            match catalog.get(&node.kind, node.version) {
                Some(kind) => {
                    let spec = kind.spec();
                    for name in &spec.required {
                        if !node.parameters.contains_key(name) {
                            errors.push(PlanError::MissingParameter {
                                node: node.id.clone(),
                                parameter: name.clone(),
                            });
                        }
                    }
                    specs.insert(node.id.clone(), spec);
                }
                None => errors.push(PlanError::UnknownKind {
                    node: node.id.clone(),
                    kind: node.kind.clone(),
                    version: node.version,
                }),
            }
        }
        for c in &definition.connections {
            for (end, outward) in [(&c.from, true), (&c.to, false)] {
                if !index.contains_key(&end.node) {
                    errors.push(PlanError::UnknownNode { node: end.node.clone() });
                } else if let Some(spec) = specs.get(&end.node) {
                    let ports = if outward { &spec.outputs } else { &spec.inputs };
                    if !ports.contains(&end.port) {
                        errors
                            .push(PlanError::UnknownPort { node: end.node.clone(), port: end.port.clone() });
                    }
                }
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }

        let ids: Vec<String> = definition.nodes.iter().map(|n| n.id.clone()).collect();
        let back = back_edges(&ids, &index, &definition.connections, &specs, &mut errors);
        if !errors.is_empty() {
            return Err(errors);
        }
        let forward: Vec<&Connection> = definition.connections.iter().filter(|c| !back.contains(c)).collect();
        let order = in_order(&ids, &index, &forward);
        if order.len() < ids.len() {
            let kept: Vec<Connection> = forward.iter().map(|c| (*c).clone()).collect();
            for component in looping(&ids, &index, &kept) {
                errors.push(PlanError::Cycle { path: cycle_path(&component, &kept) });
            }
            return Err(errors);
        }

        let mut feeds: BTreeMap<String, Vec<Connection>> = BTreeMap::new();
        for c in &forward {
            feeds.entry(c.to.node.clone()).or_default().push((*c).clone());
        }
        let unreachable = unreachable(&ids, &specs, &forward);
        Ok(Self { definition: definition.clone(), specs, order, feeds, back, unreachable })
    }

    /// The flow it was made from.
    pub fn definition(&self) -> &FlowDefinition {
        &self.definition
    }

    /// The node ids in the order they run: each after every node that feeds it; among those
    /// free to run, in the order the flow lists them.
    pub fn order(&self) -> &[String] {
        &self.order
    }

    /// What the kind of `node` says about itself.
    pub fn spec(&self, node: &str) -> Option<&NodeSpec> {
        self.specs.get(node)
    }

    /// The connections that feed `node`, leaving out those that come back to a loop.
    pub fn feeds(&self, node: &str) -> &[Connection] {
        self.feeds.get(node).map(Vec::as_slice).unwrap_or(&[])
    }

    /// The connections that come back to a loop node from a node after it. A run does not
    /// follow them yet: going round belongs to the loop operators.
    pub fn back_connections(&self) -> &[Connection] {
        &self.back
    }

    /// The nodes no trigger leads to. They run only when a run starts from them.
    pub fn unreachable(&self) -> &[String] {
        &self.unreachable
    }

    /// `node` and every node it leads to, in the order they run.
    pub fn downstream(&self, node: &str) -> Vec<String> {
        let mut reached = BTreeSet::from([node.to_owned()]);
        for id in &self.order {
            if self.feeds(id).iter().any(|c| reached.contains(&c.from.node)) {
                reached.insert(id.clone());
            }
        }
        self.order.iter().filter(|id| reached.contains(*id)).cloned().collect()
    }
}

/// The connections that close a cycle at a loop node, or a `Cycle` error for each cycle with
/// none.
fn back_edges(
    ids: &[String],
    index: &BTreeMap<String, usize>,
    connections: &[Connection],
    specs: &BTreeMap<String, NodeSpec>,
    errors: &mut Vec<PlanError>,
) -> Vec<Connection> {
    let mut back = Vec::new();
    for component in looping(ids, index, connections) {
        let inside: BTreeSet<&str> = component.iter().map(String::as_str).collect();
        let closes =
            |c: &&Connection| inside.contains(c.from.node.as_str()) && inside.contains(c.to.node.as_str());
        let loop_nodes: Vec<&String> =
            component.iter().filter(|id| specs.get(*id).is_some_and(|s| s.loops)).collect();
        match loop_nodes.first() {
            Some(head) => {
                back.extend(connections.iter().filter(closes).filter(|c| c.to.node == **head).cloned());
            }
            None => errors.push(PlanError::Cycle { path: cycle_path(&component, connections) }),
        }
    }
    back
}

/// The groups of nodes that go round: more than one node reaching each other, or a node
/// connected to itself.
fn looping(ids: &[String], index: &BTreeMap<String, usize>, connections: &[Connection]) -> Vec<Vec<String>> {
    strongly_connected(ids, index, connections)
        .into_iter()
        .filter(|g| g.len() > 1 || connections.iter().any(|c| c.from.node == g[0] && c.to.node == g[0]))
        .collect()
}

/// The groups of nodes that reach each other (Tarjan), each in the flow's order.
fn strongly_connected(
    ids: &[String],
    index: &BTreeMap<String, usize>,
    connections: &[Connection],
) -> Vec<Vec<String>> {
    struct Walk {
        next: Vec<Vec<usize>>,
        order: Vec<Option<usize>>,
        low: Vec<usize>,
        stack: Vec<usize>,
        on_stack: Vec<bool>,
        counter: usize,
        found: Vec<Vec<usize>>,
    }
    fn visit(w: &mut Walk, v: usize) {
        w.order[v] = Some(w.counter);
        w.low[v] = w.counter;
        w.counter += 1;
        w.stack.push(v);
        w.on_stack[v] = true;
        for k in 0..w.next[v].len() {
            let u = w.next[v][k];
            match w.order[u] {
                None => {
                    visit(w, u);
                    w.low[v] = w.low[v].min(w.low[u]);
                }
                Some(o) if w.on_stack[u] => w.low[v] = w.low[v].min(o),
                Some(_) => {}
            }
        }
        if Some(w.low[v]) == w.order[v] {
            let mut group = Vec::new();
            while let Some(u) = w.stack.pop() {
                w.on_stack[u] = false;
                group.push(u);
                if u == v {
                    break;
                }
            }
            group.sort_unstable();
            w.found.push(group);
        }
    }
    let n = ids.len();
    let mut next = vec![Vec::new(); n];
    for c in connections {
        if let (Some(&a), Some(&b)) = (index.get(&c.from.node), index.get(&c.to.node)) {
            next[a].push(b);
        }
    }
    let mut w = Walk {
        next,
        order: vec![None; n],
        low: vec![0; n],
        stack: Vec::new(),
        on_stack: vec![false; n],
        counter: 0,
        found: Vec::new(),
    };
    for v in 0..n {
        if w.order[v].is_none() {
            visit(&mut w, v);
        }
    }
    let mut groups: Vec<Vec<usize>> = w.found;
    groups.sort_by_key(|g| g[0]);
    groups.into_iter().map(|g| g.into_iter().map(|i| ids[i].clone()).collect()).collect()
}

/// One way round a group of nodes that reach each other, from its first node back to it.
fn cycle_path(component: &[String], connections: &[Connection]) -> Vec<String> {
    let inside: BTreeSet<&str> = component.iter().map(String::as_str).collect();
    let start = component[0].clone();
    let mut path = vec![start.clone()];
    let mut seen = BTreeSet::from([start.clone()]);
    let mut at = start.clone();
    loop {
        let mut ways = connections
            .iter()
            .filter(|c| c.from.node == at && inside.contains(c.to.node.as_str()))
            .map(|c| c.to.node.clone());
        let next = ways.clone().find(|n| *n == start).or_else(|| ways.find(|n| !seen.contains(n)));
        match next {
            Some(n) if n == start => {
                path.push(n);
                return path;
            }
            Some(n) => {
                seen.insert(n.clone());
                path.push(n.clone());
                at = n;
            }
            None => {
                path.push(start);
                return path;
            }
        }
    }
}

/// The nodes in the order they run (Kahn), ties broken by the flow's order.
fn in_order(ids: &[String], index: &BTreeMap<String, usize>, forward: &[&Connection]) -> Vec<String> {
    let n = ids.len();
    let mut waiting = vec![0usize; n];
    let mut next = vec![Vec::new(); n];
    for c in forward {
        let (a, b) = (index[&c.from.node], index[&c.to.node]);
        next[a].push(b);
        waiting[b] += 1;
    }
    let mut ready: BTreeSet<usize> = (0..n).filter(|&i| waiting[i] == 0).collect();
    let mut order = Vec::with_capacity(n);
    while let Some(i) = ready.pop_first() {
        order.push(ids[i].clone());
        for &j in &next[i] {
            waiting[j] -= 1;
            if waiting[j] == 0 {
                ready.insert(j);
            }
        }
    }
    order
}

/// The nodes no trigger leads to, triggers left out, in the flow's order.
fn unreachable(ids: &[String], specs: &BTreeMap<String, NodeSpec>, forward: &[&Connection]) -> Vec<String> {
    let mut reached: BTreeSet<&str> =
        ids.iter().filter(|id| specs.get(*id).is_some_and(|s| s.trigger)).map(String::as_str).collect();
    loop {
        let before = reached.len();
        for c in forward {
            if reached.contains(c.from.node.as_str()) {
                reached.insert(c.to.node.as_str());
            }
        }
        if reached.len() == before {
            break;
        }
    }
    ids.iter().filter(|id| !reached.contains(id.as_str())).cloned().collect()
}
