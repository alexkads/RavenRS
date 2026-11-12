//! Diagrams of nodes joined by connections: the model and the geometry, with no engine.
//!
//! A [`FlowGraph`] holds nodes placed in content units and the edges between their ports.
//! [`route`] turns an edge into the outline of its line, and the helpers after it flatten that
//! outline, measure how near a point is to it, find its middle for a label and cut it into
//! dashes. The UI draws and hit-tests with them, and an exporter can draw the same lines.

use ravenrs_core::{Point, Rect};
use serde::{Deserialize, Serialize};

use crate::vector::PathSegment;

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
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FlowNode {
    /// The default size of a node, in content units.
    pub const SIZE: (f32, f32) = (200.0, 84.0);

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
    pub nodes: Vec<FlowNode>,
    /// Drawn below the nodes, in this order.
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

    /// Takes out these nodes and every connection that touches them, and these connections.
    /// Says whether anything went.
    pub fn remove(&mut self, nodes: &[String], edges: &[String]) -> bool {
        let before = (self.nodes.len(), self.edges.len());
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
    fn bounds_and_snapping() {
        assert_eq!(graph().bounds(), Some(Rect::new(0.0, 0.0, 500.0, 184.0)));
        assert_eq!(FlowGraph::new().bounds(), None);
        assert_eq!(snap(13.0, 8.0), 16.0);
        assert_eq!(snap(13.0, 0.0), 13.0);
    }
}
