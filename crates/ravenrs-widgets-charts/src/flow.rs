//! `FlowCanvas`: a diagram of nodes joined by connections, edited on an endless surface.
//!
//! The model and the lines' geometry live in `ravenrs_graphics::flow`, which knows no engine;
//! here they become nodes that take focus and the pointer, ports that start connections, and
//! lines, labels, a grid, controls and a map drawn from the same view.
//!
//! The runtime has no transform for a laid out subtree, so a node does not scale a view of its
//! own: each node is a box placed and sized by the zoom, which draws its icon, caption, title
//! and line at that zoom. Clicks, focus and screen readers find it where it shows.

use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Color, CursorIcon, Key, PathFill, Point, Quad, Rect, Role, Scene, Size};
use ravenrs_graphics::flow as geometry;
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Theme, Weight};
use ravenrs_text::TextAttrs;

pub use ravenrs_graphics::flow::{
    EdgeShape, FlowEdge, FlowEndpoint, FlowGraph, FlowNode, FlowNodeStatus, FlowPort, PortDirection, PortSide,
};

use crate::context::{BuildCx, DragPhase, NodeId, Paint, PaintCx, Semantics};
use crate::{ButtonSize, ButtonStyle, IconButton, IconName, View, util};

/// The least and the most zoom by default.
const ZOOM_RANGE: (f32, f32) = (0.25, 2.0);
/// How much a press of a zoom button or of `+`/`-` changes the zoom.
const ZOOM_STEP: f32 = 1.25;
/// Room left around the diagram when it is fitted to the view, in screen pixels.
const FIT_MARGIN: f32 = 48.0;
/// How near a line the pointer must be to pick it, in screen pixels.
const EDGE_REACH: f32 = 6.0;
/// How near a port the pointer must be to drop a connection on it, in screen pixels.
const PORT_REACH: f32 = 20.0;
/// The size of a port's dot at 100%, and of the area that takes the pointer around it.
const PORT_DOT: f32 = 12.0;
const PORT_HIT: f32 = 24.0;
/// Below this zoom, nodes draw only their boxes: the words would be too small to read.
const TEXT_ZOOM: f32 = 0.4;
/// How fast the dashes of an animated connection travel, in screen pixels per second.
const DASH_SPEED: f32 = 24.0;
/// The map in the corner, in screen pixels.
const MAP_SIZE: Size = Size::new(200.0, 132.0);

/// Which part of the diagram is in view: where its origin lands in the view and how large it
/// is drawn. [`FlowCanvas`] writes it as people pan and zoom, and anything sharing the binding
/// can move the view.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FlowViewport {
    /// Where the diagram's point `(0, 0)` is, from the top left corner of the view.
    pub offset: Point,
    /// Screen pixels per unit of the diagram: 1 is 100%.
    pub zoom: f32,
    /// Fits the whole diagram in view on the next build, then turns itself off.
    pub fit: bool,
}

impl Default for FlowViewport {
    fn default() -> Self {
        Self::fit()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FlowViewport {
    /// The whole diagram in view.
    pub const fn fit() -> Self {
        Self { offset: Point::new(0.0, 0.0), zoom: 1.0, fit: true }
    }

    /// At this zoom, with the diagram's origin at `offset`.
    pub const fn at(offset: Point, zoom: f32) -> Self {
        Self { offset, zoom, fit: false }
    }

    /// Where a point of the diagram shows, from the top left corner of the view.
    pub fn to_screen(&self, p: Point) -> Point {
        Point::new(self.offset.x + p.x * self.zoom, self.offset.y + p.y * self.zoom)
    }

    /// The point of the diagram under a point of the view.
    pub fn to_content(&self, p: Point) -> Point {
        let zoom = self.zoom.max(f32::EPSILON);
        Point::new((p.x - self.offset.x) / zoom, (p.y - self.offset.y) / zoom)
    }

    /// At another zoom, with the point under `anchor` (in the view) staying there.
    pub fn zoomed(self, zoom: f32, anchor: Point) -> Self {
        let under = self.to_content(anchor);
        Self { offset: Point::new(anchor.x - under.x * zoom, anchor.y - under.y * zoom), zoom, fit: false }
    }

    /// The zoom and offset that show `bounds` whole and centered in a view of `size`.
    pub fn fitting(bounds: Rect, size: Size, least: f32, most: f32) -> Self {
        let room =
            Size::new((size.width - FIT_MARGIN * 2.0).max(1.0), (size.height - FIT_MARGIN * 2.0).max(1.0));
        let zoom = (room.width / bounds.width().max(1.0))
            .min(room.height / bounds.height().max(1.0))
            .clamp(least, most);
        let center = bounds.center();
        Self {
            offset: Point::new(size.width / 2.0 - center.x * zoom, size.height / 2.0 - center.y * zoom),
            zoom,
            fit: false,
        }
    }
}

/// What is selected in a [`FlowCanvas`], by id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FlowSelection {
    /// Selected nodes.
    pub nodes: Vec<String>,
    /// Selected connections.
    pub edges: Vec<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FlowSelection {
    /// Nothing.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.edges.is_empty()
    }

    /// Whether this node is selected.
    pub fn has_node(&self, id: &str) -> bool {
        self.nodes.iter().any(|n| n == id)
    }

    /// Whether this connection is selected.
    pub fn has_edge(&self, id: &str) -> bool {
        self.edges.iter().any(|e| e == id)
    }

    /// Only this node.
    pub fn node(id: impl Into<String>) -> Self {
        Self { nodes: vec![id.into()], edges: Vec::new() }
    }

    /// Only this connection.
    pub fn edge(id: impl Into<String>) -> Self {
        Self { nodes: Vec::new(), edges: vec![id.into()] }
    }
}

/// How the nodes of a kind look: their icon, the word above their title and the color of the
/// icon, a theme color.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FlowKind {
    icon: IconName,
    caption: String,
    tone: Semantic,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FlowKind {
    /// Nodes with this icon and this word above the title, the icon in the accent color.
    pub fn new(icon: IconName, caption: impl Into<String>) -> Self {
        Self { icon, caption: caption.into(), tone: Semantic::Accent }
    }

    /// With the icon in another theme color, such as [`Semantic::Warning`] for triggers.
    pub fn tone(mut self, tone: Semantic) -> Self {
        self.tone = tone;
        self
    }
}

/// Decides whether a connection may go from the first endpoint to the second, and when it
/// may not, says why in words people read beside the pointer.
pub type FlowValidator = Rc<dyn Fn(&FlowGraph, &FlowEndpoint, &FlowEndpoint) -> Result<(), String>>;

/// A diagram of nodes joined by connections, on an endless surface with a dotted grid.
///
/// * Drag the surface or turn the wheel to pan; pinch, or turn the wheel with Cmd/Ctrl, to zoom
///   around the pointer; `+`, `-` and `0` zoom in, out and to the whole diagram.
/// * Press a node to select it (Shift adds to the selection), drag it to move every selected
///   node, and Shift-drag the surface to select with a rectangle. Arrows move what is selected
///   one grid step (with Shift, four), Delete takes it out and Escape lets it go.
/// * Drag from a port to another to connect them. While dragging, the ports that would accept
///   the connection light up; one that refuses turns red, and the reason shows by the pointer.
///
/// The diagram is the app's, through a [`Binding`]: every move, connection and deletion is
/// written back to it. Nodes say what they are with a [`FlowKind`] registered by name.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{
///     FlowCanvas, FlowEdge, FlowEndpoint, FlowGraph, FlowKind, FlowNode, FlowPort, IconName,
/// };
///
/// let graph = State::new(
///     FlowGraph::new()
///         .node(FlowNode::new("sale", "trigger", "New sale", 0.0, 0.0).port(FlowPort::output("out")))
///         .node(FlowNode::new("stock", "action", "Check stock", 280.0, 0.0).port(FlowPort::input("in")))
///         .edge(FlowEdge::new("e1", FlowEndpoint::new("sale", "out"), FlowEndpoint::new("stock", "in"))),
/// );
/// let canvas = FlowCanvas::new(graph.binding())
///     .kind("trigger", FlowKind::new(IconName::Zap, "Trigger"))
///     .kind("action", FlowKind::new(IconName::Package, "Action"));
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FlowCanvas {
    graph: Binding<FlowGraph>,
    view: Option<Binding<FlowViewport>>,
    selection: Option<Binding<FlowSelection>>,
    kinds: Rc<Vec<(String, FlowKind)>>,
    validate: Option<FlowValidator>,
    grid: f32,
    snap: bool,
    controls: bool,
    minimap: bool,
    locked: bool,
    range: (f32, f32),
    label: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
    grow: f32,
}

impl fmt::Debug for FlowCanvas {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlowCanvas").field("kinds", &self.kinds).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FlowCanvas {
    /// Edits this diagram, with a grid of 24 units that nodes snap to, the zoom controls and
    /// the map in the corner.
    pub fn new(graph: Binding<FlowGraph>) -> Self {
        Self {
            graph,
            view: None,
            selection: None,
            kinds: Rc::new(Vec::new()),
            validate: None,
            grid: 24.0,
            snap: true,
            controls: true,
            minimap: true,
            locked: false,
            range: ZOOM_RANGE,
            label: None,
            width: None,
            height: None,
            grow: 1.0,
        }
    }

    /// Shares the view, to read or move it from elsewhere. Without it, the canvas keeps its own
    /// and starts with the whole diagram in view.
    pub fn view(mut self, view: Binding<FlowViewport>) -> Self {
        self.view = Some(view);
        self
    }

    /// Shares the selection, to show what is selected elsewhere (a panel of properties, say).
    pub fn selection(mut self, selection: Binding<FlowSelection>) -> Self {
        self.selection = Some(selection);
        self
    }

    /// How nodes of `kind` look. A kind with no entry shows a plain box.
    pub fn kind(mut self, kind: impl Into<String>, look: FlowKind) -> Self {
        Rc::make_mut(&mut self.kinds).push((kind.into(), look));
        self
    }

    /// Decides which connections may be made, beyond the rules that always hold: an output
    /// goes to an input of another node, once.
    pub fn validate(
        mut self,
        validate: impl Fn(&FlowGraph, &FlowEndpoint, &FlowEndpoint) -> Result<(), String> + 'static,
    ) -> Self {
        self.validate = Some(Rc::new(validate));
        self
    }

    /// The distance between the grid's dots, in units of the diagram (24 by default).
    pub fn grid(mut self, step: f32) -> Self {
        self.grid = step.max(1.0);
        self
    }

    /// Whether moved nodes snap to the grid (on by default).
    pub fn snap(mut self, snap: bool) -> Self {
        self.snap = snap;
        self
    }

    /// Whether the zoom controls show in the bottom left corner (on by default).
    pub fn controls(mut self, controls: bool) -> Self {
        self.controls = controls;
        self
    }

    /// Whether the map shows in the bottom right corner (on by default).
    pub fn minimap(mut self, minimap: bool) -> Self {
        self.minimap = minimap;
        self
    }

    /// Starts with editing locked: the view still pans and zooms, but nothing moves, connects
    /// or goes. The lock in the controls toggles it.
    pub fn locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        self
    }

    /// The least and the most zoom, where 1 is 100% (25% to 200% by default).
    pub fn range(mut self, least: f32, most: f32) -> Self {
        self.range = (least.max(0.01), most.max(least.max(0.01)));
        self
    }

    /// The name screen readers announce, instead of the one in [`Labels`](crate::Labels).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// A fixed size in logical pixels, instead of taking the free space.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self.grow = 0.0;
        self
    }

    /// How much of the free space along the parent stack it takes (1 by default).
    pub fn grow(mut self, grow: f32) -> Self {
        self.grow = grow;
        self
    }
}

/// A connection being dragged out of a port.
#[derive(Clone, Debug, PartialEq)]
struct Connecting {
    /// The port it started from.
    from: FlowEndpoint,
    /// Where the pointer is, in the view.
    pointer: Point,
    /// The port under the pointer, and whether it takes the connection.
    target: Option<(FlowEndpoint, Result<(), String>)>,
}

/// What a press on the surface or on a node is doing.
#[derive(Clone, Debug, Default)]
enum Gesture {
    #[default]
    Idle,
    /// Moving the view: where the pointer went down and where the origin was.
    Pan { pressed: Point, offset: Point, moved: bool },
    /// Selecting with a rectangle from where the pointer went down, adding to `base`.
    Select { pressed: Point, base: FlowSelection },
    /// Moving nodes: where the pointer went down and where each of them was.
    Move { pressed: Point, origins: Vec<(String, f32, f32)> },
}

/// What the canvas remembers between events, none of which should rebuild it.
#[derive(Debug, Default)]
struct Memory {
    /// The canvas's frame on screen, as of the last paint.
    frame: Cell<Rect>,
    /// The pointer, in the view.
    pointer: Cell<Option<Point>>,
    gesture: RefCell<Gesture>,
}

/// How the parts of a canvas reach the same diagram.
#[derive(Clone)]
struct Flow {
    graph: Binding<FlowGraph>,
    view: Binding<FlowViewport>,
    selection: Binding<FlowSelection>,
    locked: State<bool>,
    connecting: State<Option<Connecting>>,
    marquee: State<Option<Rect>>,
    memory: Rc<Memory>,
    validate: Option<FlowValidator>,
    grid: f32,
    snap: bool,
    range: (f32, f32),
    refused: String,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Flow {
    /// A point on screen, in the view.
    fn local(&self, p: Point) -> Point {
        let frame = self.memory.frame.get();
        Point::new(p.x - frame.x(), p.y - frame.y())
    }

    fn set_view(&self, next: FlowViewport) {
        if self.view.get() != next {
            self.view.set(next);
        }
    }

    fn select(&self, next: FlowSelection) {
        if self.selection.get() != next {
            self.selection.set(next);
        }
    }

    fn zoom_to(&self, zoom: f32, anchor: Option<Point>) {
        let size = self.memory.frame.get().size;
        let anchor = anchor.unwrap_or(Point::new(size.width / 2.0, size.height / 2.0));
        let zoom = zoom.clamp(self.range.0, self.range.1);
        self.set_view(self.view.get().zoomed(zoom, anchor));
    }

    fn zoom_by(&self, factor: f32) {
        self.zoom_to(self.view.get().zoom * factor, None);
    }

    fn fit(&self) {
        let v = self.view.get();
        self.set_view(FlowViewport { fit: true, ..v });
    }

    fn pan_by(&self, dx: f32, dy: f32) {
        let v = self.view.get();
        self.set_view(FlowViewport { offset: Point::new(v.offset.x + dx, v.offset.y + dy), ..v });
    }

    fn snapped(&self, value: f32) -> f32 {
        if self.snap { geometry::snap(value, self.grid) } else { value }
    }

    /// Moves the selected nodes by this much, in units of the diagram.
    fn nudge(&self, dx: f32, dy: f32) -> bool {
        let selection = self.selection.get();
        if selection.nodes.is_empty() || self.locked.get() {
            return false;
        }
        self.graph.update(|g| {
            for node in g.nodes.iter_mut().filter(|n| selection.has_node(&n.id)) {
                node.x += dx;
                node.y += dy;
            }
        });
        true
    }

    fn delete_selection(&self) -> bool {
        let selection = self.selection.get();
        if selection.is_empty() || self.locked.get() {
            return false;
        }
        let mut graph = self.graph.get();
        if graph.remove(&selection.nodes, &selection.edges) {
            self.graph.set(graph);
        }
        self.select(FlowSelection::default());
        true
    }

    /// The keys every part of the canvas answers.
    fn on_key(&self, key: &Key, shift: bool, command: bool) -> bool {
        let step = self.grid * if shift { 4.0 } else { 1.0 };
        let has_nodes = !self.selection.get().nodes.is_empty();
        match key {
            Key::Character(c) => match c.as_str() {
                "+" | "=" => self.zoom_by(ZOOM_STEP),
                "-" | "_" => self.zoom_by(1.0 / ZOOM_STEP),
                "0" => self.fit(),
                "a" | "A" if command => {
                    let g = self.graph.get();
                    self.select(FlowSelection {
                        nodes: g.nodes.iter().map(|n| n.id.clone()).collect(),
                        edges: g.edges.iter().map(|e| e.id.clone()).collect(),
                    });
                }
                _ => return false,
            },
            Key::ArrowLeft if has_nodes => return self.nudge(-step, 0.0),
            Key::ArrowRight if has_nodes => return self.nudge(step, 0.0),
            Key::ArrowUp if has_nodes => return self.nudge(0.0, -step),
            Key::ArrowDown if has_nodes => return self.nudge(0.0, step),
            Key::ArrowLeft => self.pan_by(step, 0.0),
            Key::ArrowRight => self.pan_by(-step, 0.0),
            Key::ArrowUp => self.pan_by(0.0, step),
            Key::ArrowDown => self.pan_by(0.0, -step),
            Key::Delete | Key::Backspace => return self.delete_selection(),
            Key::Escape => {
                if self.selection.get().is_empty() {
                    return false;
                }
                self.select(FlowSelection::default());
            }
            _ => return false,
        }
        true
    }

    /// The connection whose line passes nearest `at` (in the view), if one is near enough.
    fn edge_at(&self, at: Point) -> Option<String> {
        let (graph, view) = (self.graph.get(), self.view.get());
        graph
            .edges
            .iter()
            .filter_map(|edge| {
                let points = screen_points(&graph, edge, &view)?;
                Some((geometry::distance_to(&points, at), edge.id.clone()))
            })
            .filter(|(d, _)| *d <= EDGE_REACH)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id)
    }

    /// A press and drag on the empty surface.
    fn on_surface_drag(&self, phase: DragPhase, position: Point, shift: bool) {
        let at = self.local(position);
        self.memory.pointer.set(Some(at));
        match phase {
            DragPhase::Start => {
                let gesture = if shift && !self.locked.get() {
                    Gesture::Select { pressed: at, base: self.selection.get() }
                } else {
                    Gesture::Pan { pressed: at, offset: self.view.get().offset, moved: false }
                };
                *self.memory.gesture.borrow_mut() = gesture;
            }
            DragPhase::Move => {
                let gesture = self.memory.gesture.borrow().clone();
                match gesture {
                    Gesture::Pan { pressed, offset, .. } => {
                        let moved = (at.x - pressed.x).hypot(at.y - pressed.y) > 3.0;
                        *self.memory.gesture.borrow_mut() = Gesture::Pan { pressed, offset, moved };
                        let v = self.view.get();
                        self.set_view(FlowViewport {
                            offset: Point::new(offset.x + at.x - pressed.x, offset.y + at.y - pressed.y),
                            fit: false,
                            ..v
                        });
                    }
                    Gesture::Select { pressed, base } => {
                        let area = Rect::new(
                            pressed.x.min(at.x),
                            pressed.y.min(at.y),
                            (at.x - pressed.x).abs(),
                            (at.y - pressed.y).abs(),
                        );
                        self.marquee.set_if_changed(Some(area));
                        self.select(self.selected_in(area, base));
                    }
                    _ => {}
                }
            }
            DragPhase::End => {
                let gesture = std::mem::take(&mut *self.memory.gesture.borrow_mut());
                self.marquee.set_if_changed(None);
                if let Gesture::Pan { moved: false, .. } = gesture {
                    // A press that did not move picks the line under it, or lets go of everything.
                    let next = self.edge_at(at).map(FlowSelection::edge).unwrap_or_default();
                    self.select(next);
                }
            }
            // The base may add more; they act as nothing here.
            _ => {}
        }
    }

    /// `base` and every node the rectangle (in the view) touches.
    fn selected_in(&self, area: Rect, base: FlowSelection) -> FlowSelection {
        let (graph, view) = (self.graph.get(), self.view.get());
        let mut next = base;
        for node in &graph.nodes {
            let r = node.rect();
            let (a, b) = (view.to_screen(r.origin), view.to_screen(Point::new(r.max_x(), r.max_y())));
            let touches = a.x < area.max_x() && b.x > area.x() && a.y < area.max_y() && b.y > area.y();
            if touches && !next.has_node(&node.id) {
                next.nodes.push(node.id.clone());
            }
        }
        next
    }

    /// A press and drag on a node.
    fn on_node_drag(&self, id: &str, phase: DragPhase, position: Point, shift: bool) {
        let at = self.local(position);
        self.memory.pointer.set(Some(at));
        match phase {
            DragPhase::Start => {
                let mut selection = self.selection.get();
                if shift {
                    if selection.has_node(id) {
                        selection.nodes.retain(|n| n != id);
                    } else {
                        selection.nodes.push(id.to_owned());
                    }
                } else if !selection.has_node(id) {
                    selection = FlowSelection::node(id);
                }
                self.select(selection.clone());
                if self.locked.get() || !selection.has_node(id) {
                    *self.memory.gesture.borrow_mut() = Gesture::Idle;
                    return;
                }
                let graph = self.graph.get();
                let origins = graph
                    .nodes
                    .iter()
                    .filter(|n| selection.has_node(&n.id))
                    .map(|n| (n.id.clone(), n.x, n.y))
                    .collect();
                *self.memory.gesture.borrow_mut() = Gesture::Move { pressed: at, origins };
            }
            DragPhase::Move => {
                let Gesture::Move { pressed, origins } = self.memory.gesture.borrow().clone() else { return };
                let zoom = self.view.get().zoom.max(f32::EPSILON);
                let (dx, dy) = ((at.x - pressed.x) / zoom, (at.y - pressed.y) / zoom);
                let mut graph = self.graph.get();
                let mut changed = false;
                for (id, x, y) in &origins {
                    let (nx, ny) = (self.snapped(x + dx), self.snapped(y + dy));
                    if let Some(node) = graph.find_node_mut(id)
                        && (node.x != nx || node.y != ny)
                    {
                        node.x = nx;
                        node.y = ny;
                        changed = true;
                    }
                }
                if changed {
                    self.graph.set(graph);
                }
            }
            DragPhase::End => {
                *self.memory.gesture.borrow_mut() = Gesture::Idle;
            }
            // The base may add more; they act as nothing here.
            _ => {}
        }
    }

    /// Whether a connection may go from `from` to `to`, and why not.
    fn check(&self, graph: &FlowGraph, from: &FlowEndpoint, to: &FlowEndpoint) -> Result<(), String> {
        let (Some(a), Some(b)) = (graph.port(from), graph.port(to)) else { return Err(self.refused.clone()) };
        if from.node == to.node || a.direction == b.direction || graph.connects(from, to) {
            return Err(self.refused.clone());
        }
        match &self.validate {
            Some(validate) => validate(graph, from, to),
            None => Ok(()),
        }
    }

    /// The connection as drawn from `start` towards `other`, outputs first.
    fn oriented(
        graph: &FlowGraph,
        start: &FlowEndpoint,
        other: &FlowEndpoint,
    ) -> (FlowEndpoint, FlowEndpoint) {
        let starts_at_input = graph.port(start).is_some_and(|p| p.direction == PortDirection::Input);
        if starts_at_input { (other.clone(), start.clone()) } else { (start.clone(), other.clone()) }
    }

    /// A drag that starts on a port.
    fn on_port_drag(&self, from: &FlowEndpoint, phase: DragPhase, position: Point) {
        if self.locked.get() {
            return;
        }
        let at = self.local(position);
        self.memory.pointer.set(Some(at));
        match phase {
            DragPhase::Start => {
                self.connecting.set(Some(Connecting { from: from.clone(), pointer: at, target: None }));
            }
            DragPhase::Move => {
                let (graph, view) = (self.graph.get(), self.view.get());
                let target = nearest_port(&graph, &view, at, from).map(|end| {
                    let (a, b) = Self::oriented(&graph, from, &end);
                    let verdict = self.check(&graph, &a, &b);
                    (end, verdict)
                });
                self.connecting.set_if_changed(Some(Connecting { from: from.clone(), pointer: at, target }));
            }
            DragPhase::End => {
                let Some(connecting) = self.connecting.get() else { return };
                self.connecting.set(None);
                let Some((end, Ok(()))) = connecting.target else { return };
                let mut graph = self.graph.get();
                let (a, b) = Self::oriented(&graph, from, &end);
                let id = graph.fresh_edge_id("e");
                graph.edges.push(FlowEdge::new(id.clone(), a, b));
                self.graph.set(graph);
                self.select(FlowSelection::edge(id));
            }
            // The base may add more; they act as nothing here.
            _ => {}
        }
    }
}

/// A connection's line in the view, as points.
fn screen_points(graph: &FlowGraph, edge: &FlowEdge, view: &FlowViewport) -> Option<Vec<Point>> {
    let path = graph.edge_path(edge)?;
    Some(geometry::flatten(&path).into_iter().map(|p| view.to_screen(p)).collect())
}

/// The port nearest `at` (in the view), other than `except`'s, if one is near enough.
fn nearest_port(
    graph: &FlowGraph,
    view: &FlowViewport,
    at: Point,
    except: &FlowEndpoint,
) -> Option<FlowEndpoint> {
    graph
        .nodes
        .iter()
        .flat_map(|node| node.ports.iter().map(move |port| (node, port)))
        .filter(|(node, port)| !(node.id == except.node && port.id == except.port))
        .filter_map(|(node, port)| {
            let p = view.to_screen(node.port_point(&port.id)?);
            let d = (p.x - at.x).hypot(p.y - at.y);
            (d <= PORT_REACH).then(|| (d, FlowEndpoint::new(node.id.clone(), port.id.clone())))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, end)| end)
}

/// A line of the width of `width` through `points`.
fn stroke(scene: &mut Scene, points: &[Point], width: f32, color: Color) {
    if points.len() >= 2 {
        scene.push_path(PathFill::solid(crate::util::stroke_outline(points, width), color));
    }
}

fn fill(scene: &mut Scene, path: Vec<ravenrs_graphics::vector::PathSegment>, color: Color) {
    let elements: Vec<ravenrs_core::PathElement> = path.into_iter().filter_map(element).collect();
    scene.push_path(PathFill::solid(elements, color));
}

fn element(segment: ravenrs_graphics::vector::PathSegment) -> Option<ravenrs_core::PathElement> {
    use ravenrs_core::PathElement;
    use ravenrs_graphics::vector::PathSegment;
    Some(match segment {
        PathSegment::Move(p) => PathElement::MoveTo(p),
        PathSegment::Line(p) => PathElement::LineTo(p),
        PathSegment::Cubic(a, b, c) => PathElement::CubicTo(a, b, c),
        PathSegment::Close => PathElement::Close,
        _ => return None,
    })
}

/// The colors of the canvas, read once per build.
#[derive(Clone, Copy, Debug)]
struct Palette {
    canvas: Color,
    surface: Color,
    elevated: Color,
    control: Color,
    border: Color,
    ink: Color,
    muted: Color,
    accent: Color,
    focus: Color,
    warning: Color,
    danger: Color,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Palette {
    fn of(theme: &Theme) -> Self {
        Self {
            canvas: theme.color(Semantic::Canvas),
            surface: theme.color(Semantic::Surface),
            elevated: theme.color(Semantic::SurfaceElevated),
            control: theme.color(Semantic::Control),
            border: theme.color(Semantic::Border),
            ink: theme.color(Semantic::Ink),
            muted: theme.color(Semantic::InkMuted),
            accent: theme.color(Semantic::Accent),
            focus: theme.color(Semantic::FocusRing),
            warning: theme.color(Semantic::Warning),
            danger: theme.color(Semantic::Danger),
        }
    }
}

/// The surface: the grid, the lines, the connection being made and the selection rectangle.
#[derive(Clone)]
struct Surface {
    graph: FlowGraph,
    view: FlowViewport,
    selection: FlowSelection,
    connecting: Option<Connecting>,
    marquee: Option<Rect>,
    grid: f32,
    palette: Palette,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Surface {
    fn paint(&self, frame: Rect, time: f64, scene: &mut Scene) {
        let p = &self.palette;
        let at = |q: Point| Point::new(frame.x() + q.x, frame.y() + q.y);
        scene.push_quad(Quad::new(frame, p.canvas));
        // The grid: one dot per step, every other step when they would crowd.
        let mut step = self.grid * self.view.zoom;
        while step > 0.0 && step < 12.0 {
            step *= 2.0;
        }
        if step > 0.0 {
            let start = Point::new(self.view.offset.x.rem_euclid(step), self.view.offset.y.rem_euclid(step));
            let mut y = start.y;
            while y < frame.height() {
                let mut x = start.x;
                while x < frame.width() {
                    scene.push_quad(
                        Quad::new(Rect::new(frame.x() + x - 1.0, frame.y() + y - 1.0, 2.0, 2.0), p.border)
                            .radius(1.0),
                    );
                    x += step;
                }
                y += step;
            }
        }
        let touching = |edge: &FlowEdge| {
            self.selection.has_edge(&edge.id)
                || self.selection.has_node(&edge.from.node)
                || self.selection.has_node(&edge.to.node)
        };
        for edge in &self.graph.edges {
            let Some(points) = screen_points(&self.graph, edge, &self.view) else { continue };
            // The line stops at the edge of the port's dot, so its arrow shows.
            let dot = PORT_DOT * self.view.zoom.clamp(0.75, 1.25) / 2.0 + 1.0;
            let points: Vec<Point> = geometry::trim_end(&points, dot).into_iter().map(at).collect();
            let on = touching(edge);
            let (color, width) = if on { (p.accent, 2.0) } else { (p.muted, 1.5) };
            if edge.animated {
                let phase = (time as f32 * DASH_SPEED).rem_euclid(12.0);
                for piece in geometry::dashes(&points, 6.0, 6.0, phase) {
                    stroke(scene, &piece, width, color);
                }
            } else {
                stroke(scene, &points, width, color);
            }
            if let (Some(&tip), Some(direction)) = (points.last(), geometry::end_direction(&points)) {
                fill(scene, geometry::arrow_head(tip, direction, 8.0), color);
            }
        }
        if let Some(connecting) = &self.connecting
            && let Some((start, side)) = self.graph.anchor(&connecting.from)
        {
            let start = self.view.to_screen(start);
            let (end, end_side, color) = match &connecting.target {
                Some((target, verdict)) => {
                    let (point, side) =
                        self.graph.anchor(target).unwrap_or((Point::new(0.0, 0.0), PortSide::Left));
                    let color = if verdict.is_ok() { p.accent } else { p.danger };
                    (self.view.to_screen(point), side, color)
                }
                None => {
                    let opposite = match side {
                        PortSide::Left => PortSide::Right,
                        PortSide::Right => PortSide::Left,
                        PortSide::Top => PortSide::Bottom,
                        PortSide::Bottom => PortSide::Top,
                    };
                    (connecting.pointer, opposite, p.accent)
                }
            };
            let path = geometry::route(start, side, end, end_side, EdgeShape::Curve);
            let points: Vec<Point> = geometry::flatten(&path).into_iter().map(at).collect();
            for piece in geometry::dashes(&points, 6.0, 5.0, 0.0) {
                stroke(scene, &piece, 2.0, color);
            }
        }
        if let Some(area) = self.marquee {
            let area = area.translate(frame.origin);
            scene.push_quad(Quad::new(area, p.accent.with_alpha(0.1)).radius(4.0).border(1.5, p.focus));
        }
    }
}

impl View for FlowCanvas {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let palette = Palette::of(&theme);
        let own_view = cx.use_state(FlowViewport::fit);
        let own_selection = cx.use_state(FlowSelection::default);
        let locked = cx.use_state(|| self.locked);
        let connecting = cx.use_state(|| None::<Connecting>);
        let marquee = cx.use_state(|| None::<Rect>);
        let memory = cx.use_state(|| Rc::new(Memory::default())).get();
        let flow = Flow {
            graph: self.graph.clone(),
            view: self.view.clone().unwrap_or_else(|| own_view.binding()),
            selection: self.selection.clone().unwrap_or_else(|| own_selection.binding()),
            locked: locked.clone(),
            connecting: connecting.clone(),
            marquee: marquee.clone(),
            memory: memory.clone(),
            validate: self.validate.clone(),
            grid: self.grid,
            snap: self.snap,
            range: self.range,
            refused: labels.flow_refused.clone(),
        };
        let graph = self.graph.get();
        let size = cx.scroll_viewport();
        let mut view = flow.view.get();
        if view.fit
            && let Some(size) = size
        {
            view = match graph.bounds() {
                Some(bounds) => FlowViewport::fitting(bounds, size, self.range.0, self.range.1.min(1.0)),
                None => FlowViewport::at(Point::new(FIT_MARGIN, FIT_MARGIN), 1.0),
            };
            flow.view.set(view);
        }
        let selection = flow.selection.get();
        let connecting_now = connecting.get();
        let is_locked = locked.get();

        let mut children = Vec::new();
        for node in &graph.nodes {
            let look = self.kinds.iter().find(|(k, _)| *k == node.kind).map(|(_, look)| look.clone());
            let part = NodeBox {
                node: node.clone(),
                look,
                view,
                selected: selection.has_node(&node.id),
                connecting: connecting_now.clone(),
                connected: graph
                    .edges
                    .iter()
                    .flat_map(|e| [&e.from, &e.to])
                    .filter(|end| end.node == node.id)
                    .map(|end| end.port.clone())
                    .collect(),
                flow: flow.clone(),
                palette,
            };
            children.push(cx.keyed_child(util::hash_text(&node.id), &part));
        }
        // The words on the lines, as text the screen shows and reads like any other.
        for edge in &graph.edges {
            let Some(label) = &edge.label else { continue };
            let Some(points) = screen_points(&graph, edge, &view) else { continue };
            let Some(middle) = geometry::midpoint(&points) else { continue };
            children.push(edge_label(cx, label, middle, &theme, palette));
        }
        if let Some(Connecting { pointer, target: Some((_, Err(reason))), .. }) = &connecting_now {
            children.push(refusal(cx, reason, *pointer, &theme, palette));
        }
        if let Some(size) = size {
            if self.controls {
                children.push(controls(cx, &flow, view, is_locked, size, &theme, palette));
            }
            if self.minimap && !graph.nodes.is_empty() {
                let map = MiniMap {
                    graph: graph.clone(),
                    view,
                    size,
                    selection: selection.clone(),
                    flow: flow.clone(),
                    palette,
                };
                let map = cx.child(&map);
                children.push(map);
            }
        }

        let surface = Surface {
            graph: graph.clone(),
            view,
            selection,
            connecting: connecting_now,
            marquee: marquee.get(),
            grid: self.grid,
            palette,
        };
        let animated = graph.edges.iter().any(|e| e.animated) && !cx.reduced_motion();
        let seen = memory.clone();
        let paint = if animated {
            Paint::Live(Rc::new(move |frame: Rect, time, scene: &mut Scene| {
                seen.frame.set(frame);
                surface.paint(frame, time, scene);
            }))
        } else {
            Paint::Custom(Rc::new(move |frame: Rect, scene: &mut Scene| {
                seen.frame.set(frame);
                surface.paint(frame, 0.0, scene);
            }))
        };
        let root = cx.container(
            LayoutStyle {
                width: self.width,
                height: self.height,
                grow: self.grow,
                align: Align::Start,
                align_self: Some(if self.width.is_some() { Align::Start } else { Align::Stretch }),
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &children,
            paint,
        );
        cx.clip(root);
        cx.report_frame(root);
        cx.focusable(root, theme.radii.md);
        cx.cursor(root, CursorIcon::Default);
        let name = self.label.clone().unwrap_or_else(|| labels.flow_canvas.clone());
        let count = labels.flow_count(graph.nodes.len(), graph.edges.len());
        cx.semantics(root, Semantics::new(Role::Group).name(name).description(count));
        let modifiers = cx.modifiers();
        let dragged = (flow.clone(), modifiers.clone());
        cx.on_drag(
            root,
            Rc::new(move |d| dragged.0.on_surface_drag(d.phase, d.position, dragged.1.get().shift)),
        );
        let moved = flow.clone();
        cx.on_pointer_move(
            root,
            Rc::new(move |at, frame| {
                moved.memory.frame.set(frame);
                moved.memory.pointer.set(Some(Point::new(at.x - frame.x(), at.y - frame.y())));
            }),
        );
        let wheeled = flow.clone();
        cx.on_wheel(root, Rc::new(move |delta| wheeled.pan_by(delta.x, delta.y)));
        let pinched = flow.clone();
        cx.on_pinch(
            root,
            Rc::new(move |delta| {
                let zoom = pinched.view.get().zoom * (1.0 + delta).max(0.05);
                pinched.zoom_to(zoom, pinched.memory.pointer.get());
            }),
        );
        let keyed = (flow, modifiers);
        cx.on_key(root, Rc::new(move |key, m| keyed.0.on_key(key, m.shift, m.meta || m.ctrl)));
        root
    }
}

/// A word on the middle of a line.
fn edge_label(cx: &mut BuildCx<'_>, text: &str, at: Point, theme: &Theme, p: Palette) -> NodeId {
    let attrs = util::attrs(theme, TextStyle::Caption, Some(Weight::SEMIBOLD));
    let size = cx.measure_text(text, &attrs);
    let pad = Size::new(theme.spacing.s2, 2.0);
    let (w, h) = (size.width + pad.width * 2.0, size.height + pad.height * 2.0);
    let words = cx.text(text, &attrs, p.ink, LayoutStyle::leaf());
    cx.container(
        LayoutStyle {
            absolute: Some((at.x - w / 2.0, at.y - h / 2.0)),
            width: Some(w),
            height: Some(h),
            align: Align::Center,
            justify: ravenrs_layout::Justify::Center,
            ..LayoutStyle::stack(Axis::Horizontal)
        },
        &[words],
        Paint::Quad(crate::context::QuadPaint {
            corner_radius: theme.radii.full,
            border: Some(ravenrs_core::Border { width: 1.0, color: p.border }),
            ..crate::context::QuadPaint::new(p.elevated)
        }),
    )
}

/// Why a connection cannot be made, beside the pointer.
fn refusal(cx: &mut BuildCx<'_>, reason: &str, at: Point, theme: &Theme, p: Palette) -> NodeId {
    let attrs = util::attrs(theme, TextStyle::Callout, None);
    let icon = util::icon(cx, IconName::CircleX, 16.0, p.danger);
    let words = cx.text(reason, &attrs, p.ink, LayoutStyle { max_width: Some(280.0), ..LayoutStyle::leaf() });
    let node = cx.container(
        LayoutStyle {
            absolute: Some((at.x + 16.0, at.y + 16.0)),
            padding: ravenrs_core::Insets::symmetric(theme.spacing.s2, theme.spacing.s3),
            spacing: theme.spacing.s2,
            align: Align::Start,
            ..LayoutStyle::stack(Axis::Horizontal)
        },
        &[icon, words],
        Paint::Quad(crate::context::QuadPaint {
            corner_radius: theme.radii.md,
            border: Some(ravenrs_core::Border { width: 1.0, color: p.danger }),
            shadow: Some(theme.elevation.raised),
            ..crate::context::QuadPaint::new(p.elevated)
        }),
    );
    cx.semantics(node, Semantics::new(Role::Status).name(reason.to_owned()));
    node
}

/// The zoom buttons, the percentage and the lock, in the bottom left corner.
#[allow(clippy::too_many_arguments, reason = "the pieces of one corner")]
fn controls(
    cx: &mut BuildCx<'_>,
    flow: &Flow,
    view: FlowViewport,
    locked: bool,
    size: Size,
    theme: &Theme,
    p: Palette,
) -> NodeId {
    let labels = cx.labels();
    let button = |cx: &mut BuildCx<'_>, icon: IconName, name: &str, run: Rc<dyn Fn()>| {
        let b =
            IconButton::new(icon).label(name.to_owned()).size(ButtonSize::Small).style(ButtonStyle::Ghost);
        cx.child(&b.on_click(move || run()))
    };
    let (a, b, c, d) = (flow.clone(), flow.clone(), flow.clone(), flow.locked.clone());
    let zoom_in = button(cx, IconName::Plus, &labels.zoom_in, Rc::new(move || a.zoom_by(ZOOM_STEP)));
    let zoom_out = button(cx, IconName::Minus, &labels.zoom_out, Rc::new(move || b.zoom_by(1.0 / ZOOM_STEP)));
    let fit = button(cx, IconName::Maximize2, &labels.zoom_fit, Rc::new(move || c.fit()));
    let lock_name = if locked { &labels.flow_unlock } else { &labels.flow_lock };
    let lock = cx.child(
        &IconButton::new(if locked { IconName::Lock } else { IconName::LockOpen })
            .label(lock_name.clone())
            .size(ButtonSize::Small)
            .style(ButtonStyle::Ghost)
            .selected(locked)
            .on_click(move || d.set(!d.get())),
    );
    let percent = format!("{}%", (view.zoom * 100.0).round());
    let percent = util::label(cx, &percent, TextStyle::Callout, None, p.muted);
    let percent = cx.container(
        LayoutStyle {
            padding: ravenrs_core::Insets::symmetric(0.0, theme.spacing.s2),
            ..LayoutStyle::leaf()
        },
        &[percent],
        Paint::None,
    );
    let height = 40.0;
    let node = cx.container(
        LayoutStyle {
            absolute: Some((theme.spacing.s4, size.height - theme.spacing.s4 - height)),
            height: Some(height),
            padding: ravenrs_core::Insets::uniform(theme.spacing.s1),
            spacing: 2.0,
            align: Align::Center,
            ..LayoutStyle::stack(Axis::Horizontal)
        },
        &[zoom_in, zoom_out, fit, lock, percent],
        Paint::Quad(crate::context::QuadPaint {
            corner_radius: theme.radii.md,
            border: Some(ravenrs_core::Border { width: 1.0, color: p.border }),
            shadow: Some(theme.elevation.soft),
            ..crate::context::QuadPaint::new(p.surface)
        }),
    );
    cx.semantics(node, Semantics::new(Role::Toolbar).name(labels.flow_controls.clone()));
    node
}

/// A node of the diagram: a box placed and sized by the zoom, with its ports.
struct NodeBox {
    node: FlowNode,
    look: Option<FlowKind>,
    view: FlowViewport,
    selected: bool,
    connecting: Option<Connecting>,
    connected: Vec<String>,
    flow: Flow,
    palette: Palette,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl NodeBox {
    /// The node's description for screen readers: its kind, its line and how it is doing.
    fn description(&self, labels: &crate::Labels) -> String {
        let mut parts = Vec::new();
        if let Some(look) = &self.look {
            parts.push(look.caption.clone());
        }
        if let Some(subtitle) = &self.node.subtitle {
            parts.push(subtitle.clone());
        }
        match &self.node.status {
            FlowNodeStatus::Warning(m) | FlowNodeStatus::Error(m) => parts.push(m.clone()),
            FlowNodeStatus::Disabled => parts.push(labels.flow_disabled.clone()),
            _ => {}
        }
        parts.join(". ")
    }
}

impl View for NodeBox {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let hovered = cx.is_hovered();
        let p = self.palette;
        let (view, node) = (self.view, &self.node);
        let origin = view.to_screen(Point::new(node.x, node.y));
        let (w, h) = (node.width * view.zoom, node.height * view.zoom);

        let mut ports = Vec::new();
        for port in &node.ports {
            let Some(at) = node.port_point(&port.id) else { continue };
            let end = FlowEndpoint::new(node.id.clone(), port.id.clone());
            let state = match &self.connecting {
                Some(c) if c.from == end => PortState::Source,
                Some(Connecting { target: Some((target, verdict)), .. }) if *target == end => {
                    if verdict.is_ok() { PortState::Accepts } else { PortState::Refuses }
                }
                _ if self.selected => PortState::Selected,
                _ if self.connected.contains(&port.id) => PortState::Connected,
                _ => PortState::Free,
            };
            let dot = PortDot {
                at: Point::new((at.x - node.x) * view.zoom, (at.y - node.y) * view.zoom),
                zoom: view.zoom,
                end,
                state,
                flow: self.flow.clone(),
                palette: p,
            };
            ports.push(cx.keyed_child(util::hash_text(&port.id), &dot));
        }

        let look = self.look.clone();
        let (title, subtitle, status) = (node.title.clone(), node.subtitle.clone(), node.status.clone());
        let selected = self.selected;
        let zoom = view.zoom;
        let paint = Paint::Rich(Rc::new(move |frame: Rect, paint: &mut PaintCx<'_>| {
            draw_node(
                paint,
                frame,
                zoom,
                &theme,
                p,
                look.as_ref(),
                &title,
                subtitle.as_deref(),
                &status,
                selected,
                hovered,
            );
        }));
        let root = cx.container(
            LayoutStyle {
                absolute: Some((origin.x, origin.y)),
                width: Some(w),
                height: Some(h),
                ..LayoutStyle::leaf()
            },
            &ports,
            paint,
        );
        cx.focusable(root, theme.radii.md * view.zoom);
        cx.cursor(root, CursorIcon::Pointer);
        cx.semantics(
            root,
            Semantics::new(Role::Button)
                .name(node.title.clone())
                .description(self.description(&labels))
                .selected(self.selected)
                .disabled(node.status == FlowNodeStatus::Disabled),
        );
        let modifiers = cx.modifiers();
        let (dragged, id) = (self.flow.clone(), node.id.clone());
        cx.on_drag(
            root,
            Rc::new(move |d| dragged.on_node_drag(&id, d.phase, d.position, modifiers.get().shift)),
        );
        let moved = self.flow.memory.clone();
        cx.on_pointer_move(
            root,
            Rc::new(move |at, _| {
                let frame = moved.frame.get();
                moved.pointer.set(Some(Point::new(at.x - frame.x(), at.y - frame.y())));
            }),
        );
        let (keyed, id) = (self.flow.clone(), node.id.clone());
        cx.on_key(
            root,
            Rc::new(move |key, m| {
                // The arrows move the node that has the focus, even when it was not selected.
                let arrow = matches!(key, Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown);
                if arrow && !keyed.selection.get().has_node(&id) {
                    keyed.select(FlowSelection::node(id.clone()));
                }
                if *key == Key::Enter {
                    keyed.select(FlowSelection::node(id.clone()));
                    return true;
                }
                keyed.on_key(key, m.shift, m.meta || m.ctrl)
            }),
        );
        root
    }
}

/// Draws a node at the zoom: its card, its icon and its words.
#[allow(clippy::too_many_arguments, reason = "what one card shows")]
fn draw_node(
    paint: &mut PaintCx<'_>,
    frame: Rect,
    zoom: f32,
    theme: &Theme,
    p: Palette,
    look: Option<&FlowKind>,
    title: &str,
    subtitle: Option<&str>,
    status: &FlowNodeStatus,
    selected: bool,
    hovered: bool,
) {
    let radius = theme.radii.md * zoom;
    let disabled = *status == FlowNodeStatus::Disabled;
    let (edge, edge_width) = match status {
        _ if selected => (p.accent, 2.0),
        FlowNodeStatus::Warning(_) => (p.warning, 2.0),
        FlowNodeStatus::Error(_) => (p.danger, 2.0),
        _ => (p.border, 1.0),
    };
    if disabled {
        let outline = util::dashed_outline(frame, radius, 1.0, 6.0, 4.0);
        paint.scene.push_path(PathFill::solid(outline, if selected { p.accent } else { p.border }));
    } else {
        let fill = if hovered { p.control } else { p.elevated };
        let shadow = if selected { theme.elevation.raised } else { theme.elevation.soft };
        paint.scene.push_quad(Quad::new(frame, fill).radius(radius).border(edge_width, edge).shadow(shadow));
    }
    let pad = theme.spacing.s3 * zoom;
    let tile = 36.0 * zoom;
    let tile_rect = Rect::new(frame.x() + pad, frame.y() + (frame.height() - tile) / 2.0, tile, tile);
    paint.scene.push_quad(
        Quad::new(tile_rect, if hovered { p.elevated } else { p.control }).radius(theme.radii.sm * zoom),
    );
    if zoom < TEXT_ZOOM {
        return;
    }
    let tone = if disabled { p.muted } else { look.map_or(p.accent, |l| theme.color(l.tone)) };
    if let Some(look) = look {
        let size = 18.0 * zoom;
        let mut glyph = [0u8; 4];
        let glyph = look.icon.glyph().encode_utf8(&mut glyph);
        let mut layout = paint.text.layout(glyph, &util::icon_attrs(size));
        layout.measure(None);
        layout.paint(
            Point::new(tile_rect.center().x - size / 2.0, tile_rect.center().y - size / 2.0),
            tone,
            paint.scene,
        );
    }
    let scaled = |style: TextStyle, weight: Option<Weight>| {
        let a = util::attrs(theme, style, weight);
        TextAttrs {
            size: a.size * zoom,
            line_height: a.line_height * zoom,
            letter_spacing: a.letter_spacing * zoom,
            ..a
        }
    };
    let x = tile_rect.max_x() + pad;
    let room = (frame.max_x() - pad - x).max(0.0);
    let ink = if disabled { p.muted } else { p.ink };
    let note = match status {
        FlowNodeStatus::Warning(m) => Some((m.as_str(), p.warning)),
        FlowNodeStatus::Error(m) => Some((m.as_str(), p.danger)),
        _ => subtitle.map(|s| (s, p.muted)),
    };
    let mut lines: Vec<(String, TextAttrs, Color)> = Vec::new();
    if let Some(look) = look {
        lines.push((look.caption.to_uppercase(), scaled(TextStyle::Caption, None), p.muted));
    }
    lines.push((title.to_owned(), scaled(TextStyle::Body, Some(Weight::SEMIBOLD)), ink));
    if let Some((text, color)) = note {
        lines.push((text.to_owned(), scaled(TextStyle::Callout, None), color));
    }
    let total: f32 = lines.iter().map(|(_, a, _)| a.line_height).sum();
    let mut y = frame.y() + (frame.height() - total) / 2.0;
    for (text, attrs, color) in lines {
        let layout = paint.text.layout_ellipsized(&text, &attrs, room);
        layout.paint(Point::new(x, y), color, paint.scene);
        y += attrs.line_height;
    }
}

/// How a port shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PortState {
    /// Nothing connects to it.
    Free,
    /// A connection touches it.
    Connected,
    /// Its node is selected.
    Selected,
    /// The connection being made starts here.
    Source,
    /// The connection being made would end here.
    Accepts,
    /// The connection being made cannot end here.
    Refuses,
}

/// A port: a dot on a node's edge that starts connections, with room for the pointer around it.
struct PortDot {
    /// Its center, from the top left corner of its node.
    at: Point,
    zoom: f32,
    end: FlowEndpoint,
    state: PortState,
    flow: Flow,
    palette: Palette,
}

impl View for PortDot {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let p = self.palette;
        let state = self.state;
        let dot = PORT_DOT * self.zoom.clamp(0.75, 1.25);
        let canvas = p.canvas;
        let paint = Paint::Custom(Rc::new(move |frame: Rect, scene: &mut Scene| {
            let c = frame.center();
            let halo = match state {
                PortState::Accepts | PortState::Source => Some(p.accent),
                PortState::Refuses => Some(p.danger),
                _ => None,
            };
            if let Some(color) = halo {
                scene.push_quad(Quad::new(frame, color.with_alpha(0.2)).radius(frame.width() / 2.0));
            }
            let rect = Rect::new(c.x - dot / 2.0, c.y - dot / 2.0, dot, dot);
            let ring = Quad::new(rect, canvas).radius(dot / 2.0);
            let quad = match state {
                PortState::Free => ring.border(2.0, p.muted),
                PortState::Connected => Quad::new(rect, p.muted).radius(dot / 2.0).border(2.0, canvas),
                PortState::Selected | PortState::Source => {
                    Quad::new(rect, p.accent).radius(dot / 2.0).border(2.0, canvas)
                }
                PortState::Accepts => ring.border(2.0, p.accent),
                PortState::Refuses => ring.border(2.0, p.danger),
            };
            scene.push_quad(quad);
        }));
        let node = cx.container(
            LayoutStyle {
                absolute: Some((self.at.x - PORT_HIT / 2.0, self.at.y - PORT_HIT / 2.0)),
                width: Some(PORT_HIT),
                height: Some(PORT_HIT),
                ..LayoutStyle::leaf()
            },
            &[],
            paint,
        );
        cx.cursor(node, CursorIcon::Pointer);
        // Connections are made with the pointer for now; the node speaks for its ports.
        cx.semantics(node, Semantics::hidden());
        let (dragged, end) = (self.flow.clone(), self.end.clone());
        cx.on_drag(node, Rc::new(move |d| dragged.on_port_drag(&end, d.phase, d.position)));
        node
    }
}

/// The whole diagram in small, with the part in view outlined; a press or drag moves the view.
struct MiniMap {
    graph: FlowGraph,
    view: FlowViewport,
    size: Size,
    selection: FlowSelection,
    flow: Flow,
    palette: Palette,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl MiniMap {
    /// The area the map shows, in units of the diagram: the nodes and the view together.
    fn world(&self) -> Rect {
        let seen = Rect::from_origin_size(
            self.view.to_content(Point::new(0.0, 0.0)),
            Size::new(
                self.size.width / self.view.zoom.max(f32::EPSILON),
                self.size.height / self.view.zoom.max(f32::EPSILON),
            ),
        );
        let all = self.graph.bounds().map_or(seen, |b| geometry::union(b, seen));
        all.inflate(FIT_MARGIN)
    }
}

impl View for MiniMap {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let p = self.palette;
        let world = self.world();
        let scale = (MAP_SIZE.width / world.width().max(1.0)).min(MAP_SIZE.height / world.height().max(1.0));
        let to_map = move |frame: Rect, q: Point| {
            let inset = Point::new(
                (MAP_SIZE.width - world.width() * scale) / 2.0,
                (MAP_SIZE.height - world.height() * scale) / 2.0,
            );
            Point::new(
                frame.x() + inset.x + (q.x - world.x()) * scale,
                frame.y() + inset.y + (q.y - world.y()) * scale,
            )
        };
        let boxes: Vec<(Rect, Color)> = self
            .graph
            .nodes
            .iter()
            .map(|n| {
                let color = match &n.status {
                    _ if self.selection.has_node(&n.id) => p.accent,
                    FlowNodeStatus::Warning(_) => p.warning,
                    FlowNodeStatus::Error(_) => p.danger,
                    _ => p.control,
                };
                (n.rect(), color)
            })
            .collect();
        let view = self.view;
        let size = self.size;
        let radius = theme.radii.md;
        let node = cx.container(
            LayoutStyle {
                absolute: Some((
                    size.width - theme.spacing.s4 - MAP_SIZE.width,
                    size.height - theme.spacing.s4 - MAP_SIZE.height,
                )),
                width: Some(MAP_SIZE.width),
                height: Some(MAP_SIZE.height),
                ..LayoutStyle::leaf()
            },
            &[],
            Paint::Custom(Rc::new(move |frame: Rect, scene: &mut Scene| {
                scene.push_quad(
                    Quad::new(frame, p.surface)
                        .radius(radius)
                        .border(1.0, p.border)
                        .shadow(theme.elevation.soft),
                );
                scene.push_clip(frame);
                for (r, color) in &boxes {
                    let (a, b) = (to_map(frame, r.origin), to_map(frame, Point::new(r.max_x(), r.max_y())));
                    scene.push_quad(Quad::new(Rect::new(a.x, a.y, b.x - a.x, b.y - a.y), *color).radius(3.0));
                }
                let (a, b) = (
                    to_map(frame, view.to_content(Point::new(0.0, 0.0))),
                    to_map(frame, view.to_content(Point::new(size.width, size.height))),
                );
                scene.push_quad(
                    Quad::new(Rect::new(a.x, a.y, b.x - a.x, b.y - a.y), Color::TRANSPARENT)
                        .radius(4.0)
                        .border(1.5, p.focus),
                );
                scene.pop_clip();
            })),
        );
        cx.semantics(node, Semantics::new(Role::Slider).name(labels.minimap.clone()));
        cx.cursor(node, CursorIcon::Pointer);
        let dragged = self.flow.clone();
        cx.on_drag(
            node,
            Rc::new(move |d| {
                if d.phase == DragPhase::End {
                    return;
                }
                let inset = Point::new(
                    (MAP_SIZE.width - world.width() * scale) / 2.0,
                    (MAP_SIZE.height - world.height() * scale) / 2.0,
                );
                let target = Point::new(
                    world.x() + (d.position.x - d.frame.x() - inset.x) / scale,
                    world.y() + (d.position.y - d.frame.y() - inset.y) / scale,
                );
                let v = dragged.view.get();
                let offset =
                    Point::new(size.width / 2.0 - target.x * v.zoom, size.height / 2.0 - target.y * v.zoom);
                dragged.set_view(FlowViewport { offset, fit: false, ..v });
            }),
        );
        node
    }
}
