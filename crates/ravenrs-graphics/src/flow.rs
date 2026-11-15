//! Diagrams of nodes joined by connections: the model and the geometry, with no engine.
//!
//! A [`FlowGraph`] holds nodes placed in content units and the edges between their ports.
//! [`route`] turns an edge into the outline of its line, and the helpers after it flatten that
//! outline, measure how near a point is to it, find its middle for a label and cut it into
//! dashes. The UI draws and hit-tests with them, and an exporter can draw the same lines.

use ravenrs_core::{Point, Rect};
use serde::{Deserialize, Serialize};

use crate::vector::PathSegment;

mod layout;

pub use layout::{FlowDirection, FlowLayout};

/// Which side of a node a port sits on, and the way a line leaves it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum PortSide {
    /// The left edge; lines leave towards the left.
    #[default]
    Left,
    /// The right edge.
    Right,
    /// The top edge.
    Top,
    /// The bottom edge.
    Bottom,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl PortSide {
    /// The unit direction a line takes when it leaves a port on this side.
    pub fn outward(self) -> Point {
        match self {
            Self::Left => Point::new(-1.0, 0.0),
            Self::Right => Point::new(1.0, 0.0),
            Self::Top => Point::new(0.0, -1.0),
            Self::Bottom => Point::new(0.0, 1.0),
        }
    }

    /// Left or right: lines leave it horizontally.
    pub fn is_horizontal(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

/// Whether a port starts connections or ends them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum PortDirection {
    /// Connections end here.
    #[default]
    Input,
    /// Connections start here.
    Output,
}

/// A point on a node's edge where connections start or end.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FlowPort {
    /// Unique within its node.
    pub id: String,
    /// Where it sits.
    pub side: PortSide,
    /// Whether it starts or ends connections.
    pub direction: PortDirection,
    /// What it is called, for screen readers and labels beside it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// What travels through it (a type name of the app's), for deciding what may connect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FlowPort {
    /// An input on the left.
    pub fn input(id: impl Into<String>) -> Self {
        Self { id: id.into(), side: PortSide::Left, direction: PortDirection::Input, ..Self::default() }
    }

    /// An output on the right.
    pub fn output(id: impl Into<String>) -> Self {
        Self { id: id.into(), side: PortSide::Right, direction: PortDirection::Output, ..Self::default() }
    }

    /// On another side.
    pub fn side(mut self, side: PortSide) -> Self {
        self.side = side;
        self
    }

    /// With a name.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Carrying this kind of thing.
    pub fn kind(mut self, kind: impl Into<String>) -> Self {
        self.kind = Some(kind.into());
        self
    }
}

/// How a node is doing, which it shows.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum FlowNodeStatus {
    /// Nothing to say.
    #[default]
    Normal,
    /// Something needs attention, said in words.
    Warning(String),
    /// Something is wrong, said in words.
    Error(String),
    /// Switched off: it stays in the diagram but does not run.
    Disabled,
}

/// A box of the diagram, in content units.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FlowNode {
    /// Unique within its graph.
    pub id: String,
    /// What sort of node it is, a name of the app's: it picks the icon, the caption and the
    /// color the node is drawn with.
    pub kind: String,
    /// What it is called.
    pub title: String,
    /// A line under the title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
    /// Where connections start and end, spread evenly along each side in this order.
    #[serde(default)]
    pub ports: Vec<FlowPort>,
    /// How it is doing.
    #[serde(default)]
    pub status: FlowNodeStatus,
    /// The group it sits in, by id. Its place stays its own (in units of the diagram, like any
    /// node's), and it moves when the group moves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// A group: a box that holds the nodes whose `parent` it is, drawn behind them with its
    /// title on top.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub group: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FlowNode {
    /// The default size of a node, in content units.
    pub const SIZE: (f32, f32) = (200.0, 84.0);
    /// The kind [`FlowNode::new_group`] gives a group.
    pub const GROUP: &'static str = "group";
    /// The room a group leaves around what it holds: at the sides and bottom, and at the top,
    /// where its title goes.
    pub const GROUP_PADDING: (f32, f32) = (24.0, 48.0);

    /// A node of this kind at `(x, y)`, of the default size and with no ports.
    pub fn new(
        id: impl Into<String>,
        kind: impl Into<String>,
        title: impl Into<String>,
        x: f32,
        y: f32,
    ) -> Self {
        Self {
            id: id.into(),
            kind: kind.into(),
            title: title.into(),
            x,
            y,
            width: Self::SIZE.0,
            height: Self::SIZE.1,
            ..Self::default()
        }
    }

    /// With a line under the title.
    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Of another size.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// With one more port.
    pub fn port(mut self, port: FlowPort) -> Self {
        self.ports.push(port);
        self
    }

    /// In this state.
    pub fn status(mut self, status: FlowNodeStatus) -> Self {
        self.status = status;
        self
    }

    /// Inside this group.
    pub fn parent(mut self, group: impl Into<String>) -> Self {
        self.parent = Some(group.into());
        self
    }

    /// A group called `title`, with this box and no ports, of the kind [`FlowNode::GROUP`].
    pub fn new_group(id: impl Into<String>, title: impl Into<String>, rect: Rect) -> Self {
        Self {
            id: id.into(),
            kind: Self::GROUP.to_owned(),
            title: title.into(),
            x: rect.x(),
            y: rect.y(),
            width: rect.width(),
            height: rect.height(),
            group: true,
            ..Self::default()
        }
    }

    /// Its box.
    pub fn rect(&self) -> Rect {
        Rect::new(self.x, self.y, self.width, self.height)
    }

    /// The port with this id.
    pub fn find_port(&self, id: &str) -> Option<&FlowPort> {
        self.ports.iter().find(|p| p.id == id)
    }

    /// Where a port of this node sits, in content units: its side's ports are spread evenly
    /// along it, in the order they were added.
    pub fn port_point(&self, id: &str) -> Option<Point> {
        let port = self.find_port(id)?;
        let on_side: Vec<&FlowPort> = self.ports.iter().filter(|p| p.side == port.side).collect();
        let index = on_side.iter().position(|p| p.id == id)? as f32;
        let share = (index + 1.0) / (on_side.len() as f32 + 1.0);
        Some(match port.side {
            PortSide::Left => Point::new(self.x, self.y + self.height * share),
            PortSide::Right => Point::new(self.x + self.width, self.y + self.height * share),
            PortSide::Top => Point::new(self.x + self.width * share, self.y),
            PortSide::Bottom => Point::new(self.x + self.width * share, self.y + self.height),
        })
    }
}

/// One end of a connection: a port of a node.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FlowEndpoint {
    /// The node's id.
    pub node: String,
    /// The port's id on that node.
    pub port: String,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FlowEndpoint {
    /// The port `port` of the node `node`.
    pub fn new(node: impl Into<String>, port: impl Into<String>) -> Self {
        Self { node: node.into(), port: port.into() }
    }
}

/// How a connection's line goes from one port to the other.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum EdgeShape {
    /// A smooth curve that leaves and arrives square to each side.
    #[default]
    Curve,
    /// Straight runs at right angles, with rounded corners.
    Step,
    /// One straight line.
    Straight,
}

/// A connection from an output to an input.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FlowEdge {
    /// Unique within its graph.
    pub id: String,
    /// Where it starts.
    pub from: FlowEndpoint,
    /// Where it ends.
    pub to: FlowEndpoint,
    /// How its line goes.
    #[serde(default)]
    pub shape: EdgeShape,
    /// A word on its middle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Shows something travelling along it.
    #[serde(default)]
    pub animated: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FlowEdge {
    /// A curve from one port to another.
    pub fn new(id: impl Into<String>, from: FlowEndpoint, to: FlowEndpoint) -> Self {
        Self { id: id.into(), from, to, ..Self::default() }
    }

    /// With another shape.
    pub fn shape(mut self, shape: EdgeShape) -> Self {
        self.shape = shape;
        self
    }

    /// With a word on its middle.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Showing something travelling along it.
    pub fn animated(mut self, animated: bool) -> Self {
        self.animated = animated;
        self
    }
}

/// Nodes and the connections between them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FlowGraph {
    /// Back to front.
    #[serde(default)]
    pub nodes: Vec<FlowNode>,
    /// Drawn below the nodes, in this order.
    #[serde(default)]
    pub edges: Vec<FlowEdge>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FlowGraph {
    /// An empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// With one more node.
    pub fn node(mut self, node: FlowNode) -> Self {
        self.nodes.push(node);
        self
    }

    /// With one more connection.
    pub fn edge(mut self, edge: FlowEdge) -> Self {
        self.edges.push(edge);
        self
    }

    /// The node with this id.
    pub fn find_node(&self, id: &str) -> Option<&FlowNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// The node with this id, to change.
    pub fn find_node_mut(&mut self, id: &str) -> Option<&mut FlowNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// The connection with this id.
    pub fn find_edge(&self, id: &str) -> Option<&FlowEdge> {
        self.edges.iter().find(|e| e.id == id)
    }

    /// Where an endpoint sits and the side its port is on.
    pub fn anchor(&self, end: &FlowEndpoint) -> Option<(Point, PortSide)> {
        let node = self.find_node(&end.node)?;
        let side = node.find_port(&end.port)?.side;
        Some((node.port_point(&end.port)?, side))
    }

    /// The port an endpoint names.
    pub fn port(&self, end: &FlowEndpoint) -> Option<&FlowPort> {
        self.find_node(&end.node)?.find_port(&end.port)
    }

    /// The outline of a connection's line, if both its ends exist.
    pub fn edge_path(&self, edge: &FlowEdge) -> Option<Vec<PathSegment>> {
        let (from, from_side) = self.anchor(&edge.from)?;
        let (to, to_side) = self.anchor(&edge.to)?;
        Some(route(from, from_side, to, to_side, edge.shape))
    }

    /// Takes out these nodes with everything in them (when they are groups) and every
    /// connection that touches them, and these connections. Says whether anything went.
    pub fn remove(&mut self, nodes: &[String], edges: &[String]) -> bool {
        let before = (self.nodes.len(), self.edges.len());
        let nodes = self.with_descendants(nodes);
        let nodes = &nodes;
        self.nodes.retain(|n| !nodes.contains(&n.id));
        self.edges.retain(|e| {
            !edges.contains(&e.id) && !nodes.contains(&e.from.node) && !nodes.contains(&e.to.node)
        });
        before != (self.nodes.len(), self.edges.len())
    }

    /// An id no connection has yet, made from `stem`.
    pub fn fresh_edge_id(&self, stem: &str) -> String {
        (1..)
            .map(|n| format!("{stem}{n}"))
            .find(|id| self.find_edge(id).is_none())
            .unwrap_or_else(|| stem.to_owned())
    }

    /// Whether this exact connection is already there.
    pub fn connects(&self, from: &FlowEndpoint, to: &FlowEndpoint) -> bool {
        self.edges.iter().any(|e| &e.from == from && &e.to == to)
    }

    /// The box around every node, if there is any.
    pub fn bounds(&self) -> Option<Rect> {
        let mut nodes = self.nodes.iter();
        let first = nodes.next()?.rect();
        Some(nodes.fold(first, |all, n| union(all, n.rect())))
    }

    /// An id no node has yet, made from `id`: `id` itself when it is free, otherwise `id-2`,
    /// `id-3` and so on (a copy of `sale-2` becomes `sale-3`, not `sale-2-2`).
    pub fn fresh_node_id(&self, id: &str) -> String {
        if self.find_node(id).is_none() {
            return id.to_owned();
        }
        let stem = match id.rsplit_once('-') {
            Some((stem, n)) if !stem.is_empty() && !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => {
                stem
            }
            _ => id,
        };
        (2..)
            .map(|n| format!("{stem}-{n}"))
            .find(|id| self.find_node(id).is_none())
            .unwrap_or_else(|| stem.to_owned())
    }

    /// These nodes, with everything in the groups among them, and the connections that run
    /// between two of them: the part of the diagram that is copied, in the order the diagram
    /// has them. A node whose group stays out is no longer in a group.
    pub fn extract(&self, nodes: &[String]) -> FlowGraph {
        let nodes = self.with_descendants(nodes);
        let has = |id: &str| nodes.iter().any(|n| n == id);
        FlowGraph {
            nodes: self
                .nodes
                .iter()
                .filter(|n| has(&n.id))
                .map(|n| FlowNode { parent: n.parent.clone().filter(|p| has(p)), ..n.clone() })
                .collect(),
            edges: self.edges.iter().filter(|e| has(&e.from.node) && has(&e.to.node)).cloned().collect(),
        }
    }

    /// The nodes whose group is `id`, in the diagram's order.
    pub fn children<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a FlowNode> + 'a {
        self.nodes.iter().filter(move |n| n.parent.as_deref() == Some(id))
    }

    /// Everything inside `id`: its children, theirs and so on (not `id` itself).
    pub fn descendants(&self, id: &str) -> Vec<String> {
        let mut found: Vec<String> = Vec::new();
        let mut next = vec![id.to_owned()];
        while let Some(at) = next.pop() {
            for child in self.children(&at) {
                if child.id != id && !found.contains(&child.id) {
                    found.push(child.id.clone());
                    next.push(child.id.clone());
                }
            }
        }
        found
    }

    /// These nodes and everything inside the groups among them, each once.
    pub fn with_descendants(&self, ids: &[String]) -> Vec<String> {
        let mut all: Vec<String> = Vec::with_capacity(ids.len());
        for id in ids {
            for one in std::iter::once(id.clone()).chain(self.descendants(id)) {
                if !all.contains(&one) {
                    all.push(one);
                }
            }
        }
        all
    }

    /// The outermost group `id` is in, or `id` itself when it is in none.
    pub fn top(&self, id: &str) -> String {
        let mut at = id.to_owned();
        let mut steps = 0;
        while let Some(parent) = self.find_node(&at).and_then(|n| n.parent.clone()) {
            // A cycle cannot be built through this model, but a diagram read by hand might hold one.
            steps += 1;
            if steps > self.nodes.len() {
                break;
            }
            at = parent;
        }
        at
    }

    /// Puts `id` in the group `parent` (or in none). Refused, saying false, when `parent` is
    /// not a group or is `id` or inside it. The node and what it holds move after the group in
    /// the diagram's order, so the group is drawn behind them.
    pub fn set_parent(&mut self, id: &str, parent: Option<&str>) -> bool {
        if let Some(group) = parent {
            let ok = self.find_node(group).is_some_and(|g| g.group)
                && group != id
                && !self.descendants(id).iter().any(|d| d == group);
            if !ok {
                return false;
            }
        }
        let Some(node) = self.find_node_mut(id) else { return false };
        if node.parent.as_deref() == parent {
            return true;
        }
        node.parent = parent.map(str::to_owned);
        if parent.is_some() {
            let moving = self.with_descendants(&[id.to_owned()]);
            let (mut kept, taken): (Vec<FlowNode>, Vec<FlowNode>) =
                std::mem::take(&mut self.nodes).into_iter().partition(|n| !moving.contains(&n.id));
            kept.extend(taken);
            self.nodes = kept;
        }
        true
    }

    /// Puts these nodes in a new group `group`, sized to hold them with
    /// [`FlowNode::GROUP_PADDING`] around, and in the group they shared, if they all shared
    /// one. The group goes behind the first of them. Returns false, changing nothing, when
    /// there are no such nodes or the id is taken.
    pub fn group(&mut self, members: &[String], group: FlowNode) -> bool {
        if self.find_node(&group.id).is_some() {
            return false;
        }
        let inside: Vec<&FlowNode> = self.nodes.iter().filter(|n| members.contains(&n.id)).collect();
        let Some(first) = inside.first() else { return false };
        let bounds = inside.iter().skip(1).fold(first.rect(), |all, n| union(all, n.rect()));
        let shared = first.parent.clone().filter(|p| inside.iter().all(|n| n.parent.as_ref() == Some(p)));
        let (side, top) = FlowNode::GROUP_PADDING;
        let group = FlowNode {
            x: bounds.x() - side,
            y: bounds.y() - top,
            width: bounds.width() + side * 2.0,
            height: bounds.height() + top + side,
            parent: shared,
            group: true,
            ..group
        };
        let id = group.id.clone();
        let at = self.nodes.iter().position(|n| members.contains(&n.id)).unwrap_or(0);
        self.nodes.insert(at, group);
        for node in self.nodes.iter_mut().filter(|n| members.contains(&n.id)) {
            node.parent = Some(id.clone());
        }
        true
    }

    /// Takes out the group `id`, leaving what it held in the group it was in. Says whether it
    /// was a group.
    pub fn ungroup(&mut self, id: &str) -> bool {
        let Some(group) = self.find_node(id).filter(|g| g.group).cloned() else { return false };
        for node in self.nodes.iter_mut().filter(|n| n.parent.as_deref() == Some(id)) {
            node.parent = group.parent.clone();
        }
        self.nodes.retain(|n| n.id != id);
        self.edges.retain(|e| e.from.node != id && e.to.node != id);
        true
    }

    /// Adds a copy of `piece` moved by `(dx, dy)`, on top of the other nodes. Nodes and
    /// connections whose ids are taken get new ones, and the connections follow their nodes;
    /// a connection whose ends are not both in `piece` stays out. Returns the ids the copies
    /// got, nodes first.
    pub fn insert(&mut self, piece: &FlowGraph, dx: f32, dy: f32) -> (Vec<String>, Vec<String>) {
        let mut renamed: Vec<(&str, String)> = Vec::with_capacity(piece.nodes.len());
        let first = self.nodes.len();
        for node in &piece.nodes {
            let id = self.fresh_node_id(&node.id);
            self.nodes.push(FlowNode { id: id.clone(), x: node.x + dx, y: node.y + dy, ..node.clone() });
            renamed.push((&node.id, id));
        }
        let new_id = |old: &str| renamed.iter().find(|(o, _)| *o == old).map(|(_, id)| id.clone());
        // The copies sit in the copies of their groups, or in none.
        for node in &mut self.nodes[first..] {
            node.parent = node.parent.as_deref().and_then(new_id);
        }
        let mut edges = Vec::new();
        for edge in &piece.edges {
            let (Some(from), Some(to)) = (new_id(&edge.from.node), new_id(&edge.to.node)) else { continue };
            let id =
                if self.find_edge(&edge.id).is_none() { edge.id.clone() } else { self.fresh_edge_id("e") };
            self.edges.push(FlowEdge {
                id: id.clone(),
                from: FlowEndpoint::new(from, edge.from.port.clone()),
                to: FlowEndpoint::new(to, edge.to.port.clone()),
                ..edge.clone()
            });
            edges.push(id);
        }
        (renamed.into_iter().map(|(_, id)| id).collect(), edges)
    }

    /// The diagram as JSON, indented, which [`Self::from_json`] reads back.
    pub fn to_json(&self) -> String {
        // Nothing in the model can fail to serialize: every field is text, a number or a list.
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Reads a diagram written by [`Self::to_json`] (or by hand), and checks that it holds
    /// together: ids are not repeated and every connection joins ports that exist.
    pub fn from_json(text: &str) -> Result<Self, FlowJsonError> {
        let graph: Self = serde_json::from_str(text).map_err(|e| FlowJsonError::Syntax(e.to_string()))?;
        graph.check()?;
        Ok(graph)
    }

    /// The first thing that keeps the diagram from holding together, if any: a repeated node,
    /// port or connection id, a connection to a node or port that is not there, or a node in a
    /// group that is not there, is not a group or holds it.
    pub fn check(&self) -> Result<(), FlowJsonError> {
        for (i, node) in self.nodes.iter().enumerate() {
            if self.nodes[..i].iter().any(|n| n.id == node.id) {
                return Err(FlowJsonError::RepeatedNode(node.id.clone()));
            }
            if let Some(parent) = &node.parent {
                if !self.find_node(parent).is_some_and(|g| g.group) {
                    return Err(FlowJsonError::MissingGroup { node: node.id.clone(), group: parent.clone() });
                }
                if parent == &node.id || self.descendants(&node.id).contains(parent) {
                    return Err(FlowJsonError::GroupCycle(node.id.clone()));
                }
            }
            for (j, port) in node.ports.iter().enumerate() {
                if node.ports[..j].iter().any(|p| p.id == port.id) {
                    return Err(FlowJsonError::RepeatedPort { node: node.id.clone(), port: port.id.clone() });
                }
            }
        }
        for (i, edge) in self.edges.iter().enumerate() {
            if self.edges[..i].iter().any(|e| e.id == edge.id) {
                return Err(FlowJsonError::RepeatedEdge(edge.id.clone()));
            }
            for end in [&edge.from, &edge.to] {
                if self.port(end).is_none() {
                    return Err(FlowJsonError::MissingEnd { edge: edge.id.clone(), end: end.clone() });
                }
            }
        }
        Ok(())
    }
}

/// Why a diagram could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum FlowJsonError {
    /// It is not JSON, or not shaped like a diagram; the reader's own words.
    Syntax(String),
    /// Two nodes have this id.
    RepeatedNode(String),
    /// Two ports of this node have the same id.
    RepeatedPort {
        /// The node.
        node: String,
        /// The repeated id.
        port: String,
    },
    /// Two connections have this id.
    RepeatedEdge(String),
    /// A connection names a node or a port that is not there.
    MissingEnd {
        /// The connection.
        edge: String,
        /// The end that is missing.
        end: FlowEndpoint,
    },
    /// A node is in a group that is not there or is not a group.
    MissingGroup {
        /// The node.
        node: String,
        /// The group it names.
        group: String,
    },
    /// A node is in a group that it holds, or in itself.
    GroupCycle(String),
}

impl std::fmt::Display for FlowJsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Syntax(why) => write!(f, "not a diagram: {why}"),
            Self::RepeatedNode(id) => write!(f, "two nodes are called \"{id}\""),
            Self::RepeatedPort { node, port } => write!(f, "node \"{node}\" has two ports called \"{port}\""),
            Self::RepeatedEdge(id) => write!(f, "two connections are called \"{id}\""),
            Self::MissingEnd { edge, end } => {
                write!(
                    f,
                    "connection \"{edge}\" goes to port \"{}\" of \"{}\", which is not there",
                    end.port, end.node
                )
            }
            Self::MissingGroup { node, group } => {
                write!(f, "node \"{node}\" is in group \"{group}\", which is not a group of this diagram")
            }
            Self::GroupCycle(id) => write!(f, "node \"{id}\" is in a group it holds"),
        }
    }
}

impl std::error::Error for FlowJsonError {}

/// The diagrams before and after each change, for undo and redo. It keeps whole diagrams,
/// which is simple and right for diagrams of a few thousand nodes; the oldest go beyond
/// [`FlowHistory::LIMIT`].
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FlowHistory {
    past: Vec<FlowGraph>,
    future: Vec<FlowGraph>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FlowHistory {
    /// How many changes it remembers.
    pub const LIMIT: usize = 100;

    /// Nothing to undo or redo.
    pub fn new() -> Self {
        Self::default()
    }

    /// Remembers the diagram as it was before a change, and forgets what could be redone.
    pub fn record(&mut self, before: FlowGraph) {
        self.future.clear();
        if self.past.last() == Some(&before) {
            return;
        }
        self.past.push(before);
        if self.past.len() > Self::LIMIT {
            self.past.remove(0);
        }
    }

    /// The diagram before the last change, given the diagram as it is `now`, which redo
    /// brings back.
    pub fn undo(&mut self, now: FlowGraph) -> Option<FlowGraph> {
        let before = self.past.pop()?;
        self.future.push(now);
        Some(before)
    }

    /// The diagram the last undo went back from, given the diagram as it is `now`.
    pub fn redo(&mut self, now: FlowGraph) -> Option<FlowGraph> {
        let after = self.future.pop()?;
        self.past.push(now);
        Some(after)
    }

    /// Whether there is a change to undo.
    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    /// Whether there is an undo to redo.
    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    /// Forgets everything, as when another diagram is opened.
    pub fn clear(&mut self) {
        self.past.clear();
        self.future.clear();
    }
}

/// The smallest box holding both.
pub fn union(a: Rect, b: Rect) -> Rect {
    let (x, y) = (a.x().min(b.x()), a.y().min(b.y()));
    Rect::new(x, y, a.max_x().max(b.max_x()) - x, a.max_y().max(b.max_y()) - y)
}

/// How far a line runs straight out of a port before it turns.
pub const PORT_RUN: f32 = 20.0;
/// The radius of the corners of a [`EdgeShape::Step`] line.
pub const STEP_RADIUS: f32 = 8.0;

/// The outline of a line from `from`, leaving a port on `from_side`, to `to`, arriving at a
/// port on `to_side`.
pub fn route(
    from: Point,
    from_side: PortSide,
    to: Point,
    to_side: PortSide,
    shape: EdgeShape,
) -> Vec<PathSegment> {
    match shape {
        EdgeShape::Straight => vec![PathSegment::Move(from), PathSegment::Line(to)],
        EdgeShape::Curve => {
            let reach = |side: PortSide| {
                let gap = if side.is_horizontal() { (to.x - from.x).abs() } else { (to.y - from.y).abs() };
                (gap * 0.5).max(PORT_RUN * 2.0)
            };
            let (a, b) = (from_side.outward(), to_side.outward());
            let (ra, rb) = (reach(from_side), reach(to_side));
            vec![
                PathSegment::Move(from),
                PathSegment::Cubic(
                    Point::new(from.x + a.x * ra, from.y + a.y * ra),
                    Point::new(to.x + b.x * rb, to.y + b.y * rb),
                    to,
                ),
            ]
        }
        EdgeShape::Step => rounded(&step_points(from, from_side, to, to_side), STEP_RADIUS),
    }
}

/// The corners of a line at right angles between two ports.
fn step_points(from: Point, from_side: PortSide, to: Point, to_side: PortSide) -> Vec<Point> {
    let (a, b) = (from_side.outward(), to_side.outward());
    let out = Point::new(from.x + a.x * PORT_RUN, from.y + a.y * PORT_RUN);
    let into = Point::new(to.x + b.x * PORT_RUN, to.y + b.y * PORT_RUN);
    let mut points = vec![from, out];
    if from_side.is_horizontal() {
        let middle = (out.x + into.x) / 2.0;
        points.push(Point::new(middle, out.y));
        points.push(Point::new(middle, into.y));
    } else {
        let middle = (out.y + into.y) / 2.0;
        points.push(Point::new(out.x, middle));
        points.push(Point::new(into.x, middle));
    }
    points.extend([into, to]);
    points.dedup_by(|p, q| (p.x - q.x).abs() < 0.01 && (p.y - q.y).abs() < 0.01);
    // Corners that do not turn are not corners.
    let mut kept: Vec<Point> = Vec::with_capacity(points.len());
    for p in points {
        if kept.len() >= 2 {
            let (u, v) = (kept[kept.len() - 2], kept[kept.len() - 1]);
            let cross = (v.x - u.x) * (p.y - v.y) - (v.y - u.y) * (p.x - v.x);
            if cross.abs() < 0.01 {
                kept.pop();
            }
        }
        kept.push(p);
    }
    kept
}

/// A polyline with each corner rounded by up to `radius`.
fn rounded(points: &[Point], radius: f32) -> Vec<PathSegment> {
    let Some(&first) = points.first() else { return Vec::new() };
    let mut path = vec![PathSegment::Move(first)];
    for i in 1..points.len() {
        let p = points[i];
        let Some(&next) = points.get(i + 1) else {
            path.push(PathSegment::Line(p));
            break;
        };
        let prev = points[i - 1];
        let (d1, d2) = (distance(prev, p), distance(p, next));
        let r = radius.min(d1 / 2.0).min(d2 / 2.0);
        if r < 0.5 {
            path.push(PathSegment::Line(p));
            continue;
        }
        let toward = |a: Point, b: Point, d: f32, by: f32| {
            Point::new(a.x + (b.x - a.x) / d * by, a.y + (b.y - a.y) / d * by)
        };
        let start = toward(p, prev, d1, r);
        let end = toward(p, next, d2, r);
        // A quarter circle as a cubic: control points 0.552 of the way to the corner.
        let k = 0.552_284_8;
        let c1 = Point::new(start.x + (p.x - start.x) * k, start.y + (p.y - start.y) * k);
        let c2 = Point::new(end.x + (p.x - end.x) * k, end.y + (p.y - end.y) * k);
        path.push(PathSegment::Line(start));
        path.push(PathSegment::Cubic(c1, c2, end));
    }
    path
}

fn distance(a: Point, b: Point) -> f32 {
    (b.x - a.x).hypot(b.y - a.y)
}

/// The points along an outline, with curves cut into short straight pieces.
pub fn flatten(path: &[PathSegment]) -> Vec<Point> {
    let mut points: Vec<Point> = Vec::new();
    let mut at = Point::new(0.0, 0.0);
    for segment in path {
        match *segment {
            PathSegment::Move(p) | PathSegment::Line(p) => {
                points.push(p);
                at = p;
            }
            PathSegment::Cubic(c1, c2, end) => {
                let length = distance(at, c1) + distance(c1, c2) + distance(c2, end);
                let steps = ((length / 6.0).ceil() as usize).clamp(2, 64);
                for step in 1..=steps {
                    let t = step as f32 / steps as f32;
                    let u = 1.0 - t;
                    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    points.push(Point::new(
                        a * at.x + b * c1.x + c * c2.x + d * end.x,
                        a * at.y + b * c1.y + c * c2.y + d * end.y,
                    ));
                }
                at = end;
            }
            PathSegment::Close => {}
        }
    }
    points
}

/// How far `p` is from the nearest point of the polyline.
pub fn distance_to(points: &[Point], p: Point) -> f32 {
    if let [only] = points {
        return distance(*only, p);
    }
    points
        .windows(2)
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let length = dx * dx + dy * dy;
            let t = if length <= f32::EPSILON {
                0.0
            } else {
                (((p.x - a.x) * dx + (p.y - a.y) * dy) / length).clamp(0.0, 1.0)
            };
            distance(Point::new(a.x + dx * t, a.y + dy * t), p)
        })
        .fold(f32::INFINITY, f32::min)
}

/// The length of the polyline.
pub fn length(points: &[Point]) -> f32 {
    points.windows(2).map(|w| distance(w[0], w[1])).sum()
}

/// The point halfway along the polyline, where a label goes.
pub fn midpoint(points: &[Point]) -> Option<Point> {
    let half = length(points) / 2.0;
    let mut walked = 0.0;
    for w in points.windows(2) {
        let piece = distance(w[0], w[1]);
        if walked + piece >= half && piece > 0.0 {
            let t = (half - walked) / piece;
            return Some(Point::new(w[0].x + (w[1].x - w[0].x) * t, w[0].y + (w[1].y - w[0].y) * t));
        }
        walked += piece;
    }
    points.first().copied()
}

/// The polyline without its last `by` units of length.
pub fn trim_end(points: &[Point], by: f32) -> Vec<Point> {
    let mut kept = points.to_vec();
    let mut left = by;
    while left > 0.0 && kept.len() >= 2 {
        let (last, before) = (kept[kept.len() - 1], kept[kept.len() - 2]);
        let piece = distance(before, last);
        if piece > left {
            let t = (piece - left) / piece;
            let end = kept.len() - 1;
            kept[end] = Point::new(before.x + (last.x - before.x) * t, before.y + (last.y - before.y) * t);
            break;
        }
        left -= piece;
        kept.pop();
    }
    kept
}

/// The direction the polyline is going at its end, as a unit vector.
pub fn end_direction(points: &[Point]) -> Option<Point> {
    let (&last, rest) = points.split_last()?;
    let before = rest.iter().rev().find(|p| distance(**p, last) > 0.5)?;
    let d = distance(*before, last);
    Some(Point::new((last.x - before.x) / d, (last.y - before.y) / d))
}

/// The pieces of the polyline drawn when it is dashed: `dash` long, `gap` apart, the pattern
/// shifted along by `phase`, so a phase that grows over time makes the dashes travel.
pub fn dashes(points: &[Point], dash: f32, gap: f32, phase: f32) -> Vec<Vec<Point>> {
    let period = dash + gap;
    if period <= 0.0 || points.len() < 2 {
        return Vec::new();
    }
    let mut pieces = Vec::new();
    let mut current: Vec<Point> = Vec::new();
    // Where in the pattern the start of the line falls.
    let mut position = (-phase).rem_euclid(period);
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        let piece = distance(a, b);
        if piece <= 0.0 {
            continue;
        }
        let at = |t: f32| Point::new(a.x + (b.x - a.x) * t / piece, a.y + (b.y - a.y) * t / piece);
        let mut walked = 0.0;
        while walked < piece - 1e-4 {
            if position < dash {
                let step = (dash - position).min(piece - walked);
                if current.is_empty() {
                    current.push(at(walked));
                }
                walked += step;
                position += step;
                current.push(at(walked));
                if position >= dash - 1e-4 {
                    pieces.push(std::mem::take(&mut current));
                }
            } else {
                let step = (period - position).min(piece - walked);
                walked += step;
                position += step;
                if position >= period - 1e-4 {
                    position = 0.0;
                }
            }
        }
    }
    if current.len() >= 2 {
        pieces.push(current);
    }
    pieces
}

/// A small triangle with its tip at `tip`, pointing along `direction` (a unit vector).
pub fn arrow_head(tip: Point, direction: Point, size: f32) -> Vec<PathSegment> {
    let back = Point::new(tip.x - direction.x * size, tip.y - direction.y * size);
    let side = Point::new(-direction.y * size * 0.5, direction.x * size * 0.5);
    vec![
        PathSegment::Move(tip),
        PathSegment::Line(Point::new(back.x + side.x, back.y + side.y)),
        PathSegment::Line(Point::new(back.x - side.x, back.y - side.y)),
        PathSegment::Close,
    ]
}

/// `value` on the nearest line of a grid `step` apart (none when `step` is not positive).
pub fn snap(value: f32, step: f32) -> f32 {
    if step > 0.0 { (value / step).round() * step } else { value }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph() -> FlowGraph {
        FlowGraph::new()
            .node(FlowNode::new("a", "action", "A", 0.0, 0.0).port(FlowPort::output("out")))
            .node(
                FlowNode::new("b", "action", "B", 300.0, 100.0)
                    .port(FlowPort::input("in"))
                    .port(FlowPort::input("other")),
            )
            .edge(FlowEdge::new("e1", FlowEndpoint::new("a", "out"), FlowEndpoint::new("b", "in")))
    }

    #[test]
    fn ports_spread_along_their_side() {
        let g = graph();
        let a = g.find_node("a").map(|n| n.port_point("out"));
        assert_eq!(a, Some(Some(Point::new(200.0, 42.0))));
        let b = g.find_node("b").map(|n| (n.port_point("in"), n.port_point("other")));
        assert_eq!(b, Some((Some(Point::new(300.0, 128.0)), Some(Point::new(300.0, 156.0)))));
    }

    #[test]
    fn every_shape_starts_and_ends_at_the_ports() {
        let g = graph();
        for shape in [EdgeShape::Curve, EdgeShape::Step, EdgeShape::Straight] {
            let edge = FlowEdge { shape, ..g.edges[0].clone() };
            let points = flatten(&g.edge_path(&edge).unwrap_or_default());
            assert_eq!(points.first(), Some(&Point::new(200.0, 42.0)), "{shape:?}");
            let last = points.last().copied().unwrap_or(Point::new(0.0, 0.0));
            assert!(distance(last, Point::new(300.0, 128.0)) < 0.01, "{shape:?}");
        }
    }

    #[test]
    fn a_step_line_only_runs_across_and_down() {
        let points =
            step_points(Point::new(0.0, 0.0), PortSide::Right, Point::new(200.0, 100.0), PortSide::Left);
        for w in points.windows(2) {
            assert!(w[0].x == w[1].x || w[0].y == w[1].y, "{w:?}");
        }
        // The runs out of each port continue the turns, so only the turns stay.
        assert_eq!(points.len(), 4);
    }

    #[test]
    fn a_curve_leaves_square_to_its_side() {
        let path = route(
            Point::new(0.0, 0.0),
            PortSide::Bottom,
            Point::new(100.0, 200.0),
            PortSide::Top,
            EdgeShape::Curve,
        );
        let Some(PathSegment::Cubic(c1, c2, _)) = path.get(1).copied() else { panic!("a curve") };
        assert_eq!(c1.x, 0.0);
        assert!(c1.y > 0.0);
        assert_eq!(c2.x, 100.0);
        assert!(c2.y < 200.0);
    }

    #[test]
    fn distance_midpoint_and_end() {
        let line = [Point::new(0.0, 0.0), Point::new(100.0, 0.0)];
        assert_eq!(distance_to(&line, Point::new(50.0, 7.0)), 7.0);
        assert_eq!(distance_to(&line, Point::new(-3.0, 4.0)), 5.0);
        assert_eq!(midpoint(&line), Some(Point::new(50.0, 0.0)));
        assert_eq!(end_direction(&line), Some(Point::new(1.0, 0.0)));
        let bent = [Point::new(0.0, 0.0), Point::new(10.0, 0.0), Point::new(10.0, 4.0)];
        assert_eq!(trim_end(&bent, 6.0), vec![Point::new(0.0, 0.0), Point::new(8.0, 0.0)]);
        assert_eq!(trim_end(&line, 0.0), line.to_vec());
    }

    #[test]
    fn dashes_cover_the_pattern_and_travel_with_the_phase() {
        let line = [Point::new(0.0, 0.0), Point::new(36.0, 0.0)];
        let still = dashes(&line, 6.0, 6.0, 0.0);
        assert_eq!(still.len(), 3);
        assert_eq!(still[0], vec![Point::new(0.0, 0.0), Point::new(6.0, 0.0)]);
        let moved = dashes(&line, 6.0, 6.0, 3.0);
        assert_eq!(moved[0], vec![Point::new(3.0, 0.0), Point::new(9.0, 0.0)]);
        assert_eq!(moved[1].first(), Some(&Point::new(15.0, 0.0)));
    }

    #[test]
    fn removing_a_node_takes_its_connections() {
        let mut g = graph();
        assert!(g.remove(&["b".to_owned()], &[]));
        assert!(g.edges.is_empty());
        assert!(!g.remove(&["b".to_owned()], &[]));
        assert_eq!(g.fresh_edge_id("e"), "e1");
    }

    #[test]
    fn json_goes_out_and_comes_back_the_same() {
        let g = graph()
            .node(
                FlowNode::new("c", "end", "C", 0.0, 200.0)
                    .subtitle("last")
                    .status(FlowNodeStatus::Warning("no account".into()))
                    .port(FlowPort::input("in").side(PortSide::Top).label("In").kind("sale")),
            )
            .edge(
                FlowEdge::new("e2", FlowEndpoint::new("a", "out"), FlowEndpoint::new("c", "in"))
                    .shape(EdgeShape::Step)
                    .label("No")
                    .animated(true),
            );
        assert_eq!(FlowGraph::from_json(&g.to_json()), Ok(g));
        assert_eq!(FlowGraph::from_json("{}"), Ok(FlowGraph::new()));
    }

    #[test]
    fn json_that_does_not_hold_together_says_why() {
        assert!(matches!(FlowGraph::from_json("[1, 2]"), Err(FlowJsonError::Syntax(_))));
        let twice = graph().node(FlowNode::new("a", "action", "A again", 0.0, 0.0));
        assert_eq!(FlowGraph::from_json(&twice.to_json()), Err(FlowJsonError::RepeatedNode("a".into())));
        let nowhere =
            graph().edge(FlowEdge::new("e2", FlowEndpoint::new("a", "out"), FlowEndpoint::new("b", "gone")));
        let error = FlowGraph::from_json(&nowhere.to_json());
        assert_eq!(
            error,
            Err(FlowJsonError::MissingEnd { edge: "e2".into(), end: FlowEndpoint::new("b", "gone") })
        );
        assert_eq!(
            error.map_err(|e| e.to_string()),
            Err("connection \"e2\" goes to port \"gone\" of \"b\", which is not there".into())
        );
    }

    #[test]
    fn a_copied_piece_comes_back_with_new_ids_and_its_own_connections() {
        let mut g = graph();
        let piece = g.extract(&["a".into(), "b".into()]);
        assert_eq!((piece.nodes.len(), piece.edges.len()), (2, 1));
        assert_eq!(g.extract(&["a".into()]).edges.len(), 0, "a connection needs both ends");
        let (nodes, edges) = g.insert(&piece, 24.0, 48.0);
        assert_eq!(nodes, vec!["a-2".to_owned(), "b-2".to_owned()]);
        assert_eq!(edges, vec!["e2".to_owned()]);
        let copy = g.find_edge("e2").cloned().unwrap_or_default();
        assert_eq!((copy.from, copy.to), (FlowEndpoint::new("a-2", "out"), FlowEndpoint::new("b-2", "in")));
        assert_eq!(g.find_node("b-2").map(|n| (n.x, n.y)), Some((324.0, 148.0)));
        assert_eq!(g.check(), Ok(()));
        // A copy of a copy counts on, and a free id stays as it is.
        assert_eq!(g.fresh_node_id("a-2"), "a-3");
        assert_eq!(g.fresh_node_id("new"), "new");
    }

    fn grouped() -> FlowGraph {
        let mut g = graph().node(FlowNode::new("c", "action", "C", 300.0, 300.0));
        assert!(g.group(&["a".into(), "b".into()], FlowNode::new_group("g", "Sale", Rect::default())));
        g
    }

    #[test]
    fn a_group_holds_its_nodes_with_room_and_is_drawn_behind_them() {
        let g = grouped();
        let group = g.find_node("g").cloned().unwrap_or_default();
        // a and b span (0, 0) to (500, 184); 24 around and 48 on top.
        assert_eq!(group.rect(), Rect::new(-24.0, -48.0, 548.0, 256.0));
        assert!(group.group && group.parent.is_none());
        assert_eq!(g.nodes[0].id, "g", "before the first of them");
        assert_eq!(g.children("g").map(|n| n.id.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(g.top("a"), "g");
        assert_eq!(g.check(), Ok(()));
        assert!(
            !grouped().group(&["c".into()], FlowNode::new_group("g", "Again", Rect::default())),
            "id taken"
        );
    }

    #[test]
    fn groups_nest_and_cannot_hold_themselves() {
        let mut g = grouped();
        assert!(g.group(&["g".into(), "c".into()], FlowNode::new_group("outer", "All", Rect::default())));
        assert_eq!(g.top("a"), "outer");
        let mut inside = g.descendants("outer");
        inside.sort();
        assert_eq!(inside, ["a", "b", "c", "g"]);
        assert!(!g.set_parent("outer", Some("g")), "a group inside what it holds");
        assert!(!g.set_parent("c", Some("a")), "a is not a group");
        assert!(g.set_parent("c", Some("g")));
        let order: Vec<&str> = g.nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(order.iter().position(|id| *id == "g") < order.iter().position(|id| *id == "c"));
        assert_eq!(g.check(), Ok(()));
    }

    #[test]
    fn removing_a_group_takes_what_it_holds_and_ungrouping_keeps_it() {
        let mut gone = grouped();
        assert!(gone.remove(&["g".into()], &[]));
        assert_eq!(gone.nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(), ["c"]);
        assert!(gone.edges.is_empty());

        let mut loose = grouped();
        assert!(loose.ungroup("g"));
        assert!(!loose.ungroup("a"), "not a group");
        assert!(loose.nodes.iter().all(|n| n.parent.is_none()));
        assert_eq!(loose.edges.len(), 1);
    }

    #[test]
    fn a_copied_group_comes_with_what_it_holds_in_a_copy_of_itself() {
        let mut g = grouped();
        let piece = g.extract(&["g".into()]);
        assert_eq!(piece.nodes.len(), 3);
        assert_eq!(piece.edges.len(), 1);
        let (nodes, _) = g.insert(&piece, 0.0, 400.0);
        assert_eq!(nodes, ["g-2", "a-2", "b-2"]);
        assert_eq!(g.find_node("a-2").and_then(|n| n.parent.clone()).as_deref(), Some("g-2"));
        // Copying a node out of its group leaves the group behind.
        let alone = g.extract(&["a".into()]);
        assert_eq!(alone.nodes[0].parent, None);
        assert_eq!(g.check(), Ok(()));
    }

    #[test]
    fn json_keeps_groups_and_refuses_a_missing_or_circular_one() {
        let g = grouped();
        assert_eq!(FlowGraph::from_json(&g.to_json()), Ok(g.clone()));
        assert!(!FlowGraph::new().node(FlowNode::new("x", "k", "X", 0.0, 0.0)).to_json().contains("group"));
        let lost = graph().node(FlowNode::new("x", "k", "X", 0.0, 0.0).parent("nowhere"));
        assert_eq!(
            FlowGraph::from_json(&lost.to_json()),
            Err(FlowJsonError::MissingGroup { node: "x".into(), group: "nowhere".into() })
        );
        let mut circle = grouped();
        if let Some(group) = circle.find_node_mut("g") {
            group.parent = Some("g".into());
        }
        assert_eq!(FlowGraph::from_json(&circle.to_json()), Err(FlowJsonError::GroupCycle("g".into())));
    }

    #[test]
    fn history_undoes_and_redoes_in_order() {
        let (one, two, three) =
            (FlowGraph::new(), graph(), graph().node(FlowNode::new("c", "action", "C", 0.0, 0.0)));
        let mut h = FlowHistory::new();
        assert!(!h.can_undo());
        h.record(one.clone());
        h.record(two.clone());
        assert_eq!(h.undo(three.clone()), Some(two.clone()));
        assert_eq!(h.undo(two.clone()), Some(one.clone()));
        assert_eq!(h.undo(one.clone()), None);
        assert_eq!(h.redo(one.clone()), Some(two.clone()));
        assert_eq!(h.redo(two.clone()), Some(three.clone()));
        assert!(!h.can_redo());
        // A new change forgets what could be redone.
        h.undo(three);
        h.record(two);
        assert!(!h.can_redo());
        for _ in 0..FlowHistory::LIMIT + 10 {
            h.record(FlowGraph::new());
            h.record(graph());
        }
        let mut undone = 0;
        while h.undo(FlowGraph::new()).is_some() {
            undone += 1;
        }
        assert_eq!(undone, FlowHistory::LIMIT);
    }

    #[test]
    fn bounds_and_snapping() {
        assert_eq!(graph().bounds(), Some(Rect::new(0.0, 0.0, 500.0, 184.0)));
        assert_eq!(FlowGraph::new().bounds(), None);
        assert_eq!(snap(13.0, 8.0), 16.0);
        assert_eq!(snap(13.0, 0.0), 13.0);
    }
}
