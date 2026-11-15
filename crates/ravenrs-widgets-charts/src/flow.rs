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

use ravenrs_core::{Color, CursorIcon, Key, Modifiers, PathFill, Point, Quad, Rect, Role, Scene, Size};
use ravenrs_graphics::flow as geometry;
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Theme, Weight};
use ravenrs_text::TextAttrs;

pub use ravenrs_graphics::flow::{
    EdgeCap, EdgeShape, FlowDirection, FlowEdge, FlowEndpoint, FlowGraph, FlowHistory, FlowJsonError,
    FlowLayout, FlowLayoutKind, FlowNode, FlowNodeStatus, FlowPort, FlowShape, PortDirection, PortSide,
};

use crate::context::{BuildCx, DragPhase, NodeId, Paint, PaintCx, Semantics};
use crate::{AnyView, ButtonSize, ButtonStyle, Clipboard, IconButton, IconName, TextField, View, util};

/// What a node shows inside its card instead of its icon and words, for
/// [`FlowCanvas::content`].
pub type FlowContent = Rc<dyn Fn(&FlowNode) -> Option<AnyView>>;

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
/// How near, in screen pixels, an edge or middle must come to another node's to line up.
const GUIDE_REACH: f32 = 6.0;
/// How far a node turns at a time, in degrees, by its handle or the keys.
const TURN_STEP: f32 = 15.0;
/// The round handle that turns a selected node, and how far above the node it sits, in screen
/// pixels.
const TURNER: f32 = 20.0;
const TURNER_GAP: f32 = 24.0;
/// How long the cap at an end of a line is, in screen pixels.
const CAP: f32 = 8.0;
/// The square in a selected node's corner that resizes it, in screen pixels.
const HANDLE: f32 = 14.0;
/// The least a node can be resized to, in units of the diagram.
const MIN_NODE: (f32, f32) = (96.0, 48.0);

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

/// What a change to the diagram in a [`FlowCanvas`] is, with the ids it touches, for
/// [`FlowCanvas::on_change`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum FlowChange {
    /// Nodes moved, by a drag (asked once, when it is let go) or by the arrows.
    Move {
        /// The nodes that moved, with what the groups among them hold.
        nodes: Vec<String>,
    },
    /// Nodes changed size.
    Resize {
        /// The nodes resized.
        nodes: Vec<String>,
    },
    /// A connection was made.
    Connect {
        /// Its id.
        edge: String,
    },
    /// An end of a connection went to another port.
    Reconnect {
        /// Its id.
        edge: String,
    },
    /// Nodes and connections went, the nodes with what they held and every connection that
    /// touched them.
    Delete {
        /// The nodes selected.
        nodes: Vec<String>,
        /// The connections selected.
        edges: Vec<String>,
    },
    /// Nodes and connections came from the clipboard.
    Paste {
        /// The new nodes.
        nodes: Vec<String>,
        /// The new connections.
        edges: Vec<String>,
    },
    /// Nodes went into a new group.
    Group {
        /// The new group.
        group: String,
        /// What it holds.
        nodes: Vec<String>,
    },
    /// Groups were taken apart.
    Ungroup {
        /// The groups.
        groups: Vec<String>,
    },
    /// The whole diagram was arranged.
    Arrange,
    /// The last change was undone.
    Undo,
    /// The last undo was redone.
    Redo,
    /// Nodes went in front of or behind the others.
    Reorder {
        /// The nodes that moved in the order.
        nodes: Vec<String>,
    },
    /// A node got another title, typed on it.
    Rename {
        /// The node.
        node: String,
    },
    /// Nodes were turned.
    Rotate {
        /// The nodes turned.
        nodes: Vec<String>,
    },
    /// A node was made next to another, by its arrows or Cmd/Ctrl+Enter.
    Create {
        /// The new node.
        node: String,
        /// The connection made to it, when the two have ports to join.
        edge: Option<String>,
    },
}

/// Gives the node made next to `from`, on `side` of it, by the arrows of quick creation: its
/// id and place are set by the canvas. `None` makes nothing on that side.
pub type FlowNext = Rc<dyn Fn(&FlowNode, PortSide) -> Option<FlowNode>>;

/// Decides whether a change may happen, given the diagram before and after it. When it may
/// not, the diagram stays as it was, nothing goes into the history, and the reason shows.
pub type FlowGuard = Rc<dyn Fn(&FlowGraph, &FlowGraph, &FlowChange) -> Result<(), String>>;

/// A diagram of nodes joined by connections, on an endless surface with a dotted grid.
///
/// * Drag the surface or turn the wheel to pan; pinch, or turn the wheel with Cmd/Ctrl, to zoom
///   around the pointer; `+`, `-` and `0` zoom in, out and to the whole diagram.
/// * Press a node to select it (Shift adds to the selection), drag it to move every selected
///   node, and Shift-drag the surface to select with a rectangle. Arrows move what is selected
///   one grid step (with Shift, four), Delete takes it out and Escape lets it go.
/// * Drag from a port to another to connect them. While dragging, the ports that would accept
///   the connection light up; one that refuses turns red, and the reason shows by the pointer.
///   With the keys, Tab to a port, press Enter, Tab to the other port and press Enter again.
/// * Select a connection and drag one of its ends, which show as handles, to another port to
///   move that end.
/// * Drag the corner of a selected node, or press Alt with the arrows, to resize it. The
///   arrange button in the controls lays the diagram out in layers ([`FlowLayout`]).
/// * Cmd/Ctrl+G puts the selected nodes in a group, which moves, copies and goes with what it
///   holds; Shift+Cmd/Ctrl+G takes the selected groups apart. Dropping a node inside a group
///   puts it there, and dropping it outside takes it out.
/// * Double-click a node, or press F2 (or Enter on a selected node), to type its title in
///   place: Enter keeps it, Escape gives it up.
/// * Drag the round handle above a selected node to turn it, 15 degrees at a time (with Shift,
///   freely), or press Alt+Shift with the arrows; Alt+Shift+0 puts it straight. Groups do
///   not turn.
/// * With the pointer over a node, arrows show on its sides: a click on one makes the next
///   node there, joined to it, with its title open to type. Cmd/Ctrl+Enter does it to the right
///   of the selected node, Cmd/Ctrl+Shift+Enter below ([`FlowCanvas::next_node`]).
/// * Cmd/Ctrl+Z undoes, Shift+Cmd/Ctrl+Z (or Cmd/Ctrl+Y) redoes. Cmd/Ctrl+C copies the selected
///   nodes with the connections between them, as JSON, Cmd/Ctrl+X cuts them and Cmd/Ctrl+V
///   pastes them back with new ids, a little further along each time.
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
    history: Option<Binding<FlowHistory>>,
    kinds: Rc<Vec<(String, FlowKind)>>,
    validate: Option<FlowValidator>,
    guard: Option<FlowGuard>,
    layout: FlowLayout,
    resizable: bool,
    rotatable: bool,
    editable_titles: bool,
    content: Option<FlowContent>,
    next: Option<FlowNext>,
    quick_create: bool,
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
            history: None,
            kinds: Rc::new(Vec::new()),
            validate: None,
            guard: None,
            layout: FlowLayout::default(),
            resizable: true,
            rotatable: true,
            editable_titles: true,
            content: None,
            next: None,
            quick_create: true,
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

    /// Shares what can be undone and redone, to offer undo and redo buttons elsewhere (with
    /// [`FlowHistory::can_undo`]) or to forget it when another diagram is opened. Without it,
    /// the canvas keeps its own. Undo puts back the whole diagram as it was, so a change the
    /// app made to the diagram in between goes too.
    pub fn history(mut self, history: Binding<FlowHistory>) -> Self {
        self.history = Some(history);
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

    /// Asked before every change the canvas makes to the diagram, with the diagram before and
    /// after and what the change is; an `Err` refuses it and says why. A drag is asked once,
    /// when it is let go, and a refused one puts the nodes back.
    ///
    /// ```
    /// use ravenrs_reactive::State;
    /// use ravenrs_widgets::{FlowCanvas, FlowChange, FlowGraph};
    ///
    /// let graph = State::new(FlowGraph::new());
    /// let canvas = FlowCanvas::new(graph.binding()).on_change(|_, _, change| match change {
    ///     FlowChange::Delete { .. } => Err("Published steps stay".into()),
    ///     _ => Ok(()),
    /// });
    /// ```
    pub fn on_change(
        mut self,
        guard: impl Fn(&FlowGraph, &FlowGraph, &FlowChange) -> Result<(), String> + 'static,
    ) -> Self {
        self.guard = Some(Rc::new(guard));
        self
    }

    /// How the arrange button lays the diagram out: from left to right by default.
    pub fn layout(mut self, layout: FlowLayout) -> Self {
        self.layout = layout;
        self
    }

    /// Whether selected nodes can be resized, by their corner or with Alt and the arrows (on
    /// by default).
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// Whether selected nodes can be turned, by their handle or with Alt+Shift and the arrows
    /// (on by default). Groups never turn.
    pub fn rotatable(mut self, rotatable: bool) -> Self {
        self.rotatable = rotatable;
        self
    }

    /// What the arrows of quick creation make next to a node: given the node and the side,
    /// the node to make (its id and place are the canvas's), or none. Without it, a copy of the
    /// node is made, with its kind, shape, size and ports.
    pub fn next_node(mut self, next: impl Fn(&FlowNode, PortSide) -> Option<FlowNode> + 'static) -> Self {
        self.next = Some(Rc::new(next));
        self
    }

    /// Whether the arrows and keys of quick creation are there (on by default).
    pub fn quick_create(mut self, on: bool) -> Self {
        self.quick_create = on;
        self
    }

    /// Puts any view inside the nodes it is given for: `content` is asked for each node (not
    /// groups) on every build, and a node it answers shows that view inside its card, laid out
    /// at the node's size in units of the diagram and drawn at the zoom, its text sharp at any
    /// zoom. The view's own controls take the pointer and the keys; where it has none, the
    /// node is pressed, dragged and connected as ever. A node without a view shows its icon
    /// and words.
    ///
    /// ```
    /// use ravenrs_reactive::State;
    /// use ravenrs_widgets::{AnyView, FlowCanvas, FlowGraph, Text};
    ///
    /// let graph = State::new(FlowGraph::new());
    /// let canvas = FlowCanvas::new(graph.binding()).content(|node| {
    ///     (node.kind == "note").then(|| Box::new(Text::new(node.title.clone())) as AnyView)
    /// });
    /// ```
    pub fn content(mut self, content: impl Fn(&FlowNode) -> Option<AnyView> + 'static) -> Self {
        self.content = Some(Rc::new(content));
        self
    }

    /// Whether a node's title can be typed in place, by a double click, F2 or Enter (on by
    /// default).
    pub fn editable_titles(mut self, editable: bool) -> Self {
        self.editable_titles = editable;
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

/// A connection being dragged out of a port, or chosen with the keys.
#[derive(Clone, Debug, PartialEq)]
struct Connecting {
    /// The port it started from: for a connection whose end is being moved, its other end.
    from: FlowEndpoint,
    /// Where the pointer is, in the view.
    pointer: Point,
    /// The port under the pointer, and whether it takes the connection.
    target: Option<(FlowEndpoint, Result<(), String>)>,
    /// The connection whose end is being moved, which is drawn as this line meanwhile.
    moving: Option<String>,
    /// Started with Enter on a port: the next Enter on a port ends it.
    keyboard: bool,
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
    /// Moving nodes: where the pointer went down, where each of them was and the diagram
    /// before, which undo brings back.
    Move { pressed: Point, origins: Vec<(String, f32, f32)>, before: FlowGraph, lead: String },
    /// Resizing a node by its corner: where the pointer went down, its size then and the
    /// diagram before.
    Resize { id: String, pressed: Point, size: (f32, f32), before: FlowGraph },
    /// Turning a node by its handle, with the diagram before.
    Turn { id: String, before: FlowGraph },
}

/// What the canvas remembers between events, none of which should rebuild it.
#[derive(Debug, Default)]
struct Memory {
    /// The canvas's frame on screen, as of the last paint.
    frame: Cell<Rect>,
    /// The pointer, in the view.
    pointer: Cell<Option<Point>>,
    gesture: RefCell<Gesture>,
    /// How many grid steps along the next paste lands: none after a cut, two more each time.
    paste_step: Cell<u32>,
    /// What was copied last, for a clipboard that cannot be read now (in the browser).
    copied: RefCell<Option<String>>,
    /// Nodes went, perhaps the one with the focus, or the diagram was arranged from its button:
    /// the canvas takes the focus on the next build, so the keys (undo, say) still reach it.
    refocus: Cell<bool>,
    /// The view before a title was opened from a zoom too small to read it, put back after.
    view_before: Cell<Option<FlowViewport>>,
    /// The node that takes the focus on the next build, once its title closes.
    focus_node: RefCell<Option<String>>,
    /// Step lines already worked out around the nodes in their way.
    routes: RefCell<geometry::RouteCache>,
}

/// How the parts of a canvas reach the same diagram.
#[derive(Clone)]
struct Flow {
    graph: Binding<FlowGraph>,
    view: Binding<FlowViewport>,
    selection: Binding<FlowSelection>,
    history: Binding<FlowHistory>,
    clipboard: Clipboard,
    locked: State<bool>,
    connecting: State<Option<Connecting>>,
    marquee: State<Option<Rect>>,
    /// The guides shown while a node is dragged: an upright line at this x and a level one at
    /// this y, in units of the diagram.
    guides: State<(Option<f32>, Option<f32>)>,
    memory: Rc<Memory>,
    validate: Option<FlowValidator>,
    guard: Option<FlowGuard>,
    /// Why the last change was refused, until the next gesture.
    notice: State<Option<String>>,
    layout: FlowLayout,
    resizable: bool,
    rotatable: bool,
    /// Why a group does not turn.
    no_turn: String,
    next: Option<FlowNext>,
    quick_create: bool,
    editable_titles: bool,
    /// The node whose title is being typed, and what has been typed.
    editing: State<Option<String>>,
    draft: State<String>,
    grid: f32,
    snap: bool,
    range: (f32, f32),
    refused: String,
    /// What a new group is called.
    group_title: String,
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

    /// Whether the app lets this change happen. A refusal shows its reason.
    fn allowed(&self, before: &FlowGraph, after: &FlowGraph, change: &FlowChange) -> bool {
        let Some(guard) = &self.guard else { return true };
        match guard(before, after, change) {
            Ok(()) => true,
            Err(reason) => {
                self.notice.set(Some(reason));
                false
            }
        }
    }

    /// Writes a changed diagram, when the app lets it, remembering the one before for undo.
    /// Says whether it was written (or there was nothing to write).
    fn commit(&self, next: FlowGraph, change: FlowChange) -> bool {
        let before = self.graph.get();
        if before == next {
            return true;
        }
        if !self.allowed(&before, &next, &change) {
            return false;
        }
        self.history.update(|h| h.record(before));
        self.graph.set(next);
        true
    }

    /// Ends a drag that changed the diagram live: asks the app once, and either keeps the
    /// change as one step of the history or puts the diagram back.
    fn settle(&self, before: FlowGraph, change: FlowChange) {
        let after = self.graph.get();
        if before == after {
            return;
        }
        if self.allowed(&before, &after, &change) {
            self.history.update(|h| h.record(before));
        } else {
            self.graph.set(before);
        }
    }

    /// Lets go of the reason of the last refusal, as a new gesture starts.
    fn forget_notice(&self) {
        self.notice.set_if_changed(None);
    }

    /// Opens the title of `id` for typing, with all of it selected. Below the zoom at which
    /// nodes show words, the view first goes to 100% around the node, and comes back after.
    /// The node to make next to `from` on `side`, and the connection that joins them (output
    /// to input, in the way the side points), placed a layer away on the grid, lower down
    /// while that place is taken.
    fn next_to(
        &self,
        graph: &FlowGraph,
        from: &FlowNode,
        side: PortSide,
    ) -> Option<(FlowNode, Option<FlowEdge>)> {
        let made = match &self.next {
            Some(next) => next(from, side)?,
            None => FlowNode {
                status: FlowNodeStatus::Normal,
                parent: None,
                group: false,
                rotation: 0.0,
                ..from.clone()
            },
        };
        let gap = 96.0;
        let (w, h) = (made.width, made.height);
        let (mut x, mut y) = match side {
            PortSide::Right => (from.x + from.width + gap, from.y),
            PortSide::Left => (from.x - gap - w, from.y),
            PortSide::Bottom => (from.x, from.y + from.height + gap),
            PortSide::Top => (from.x, from.y - gap - h),
        };
        (x, y) = (self.snapped(x), self.snapped(y));
        let taken = |x: f32, y: f32| {
            let r = Rect::new(x, y, w, h);
            graph.nodes.iter().any(|n| {
                let e = n.extent();
                !n.group && e.x() < r.max_x() && r.x() < e.max_x() && e.y() < r.max_y() && r.y() < e.max_y()
            })
        };
        for _ in 0..20 {
            if !taken(x, y) {
                break;
            }
            y = self.snapped(y + h + 48.0);
        }
        let id = graph.fresh_node_id(&made.id);
        let made = FlowNode { id: id.clone(), x, y, ..made };
        // Forward (right, down): the old node's output to the new one's input; backward the
        // other way round. A port on the side faced is preferred.
        let forward = matches!(side, PortSide::Right | PortSide::Bottom);
        let opposite = match side {
            PortSide::Left => PortSide::Right,
            PortSide::Right => PortSide::Left,
            PortSide::Top => PortSide::Bottom,
            PortSide::Bottom => PortSide::Top,
        };
        let pick = |node: &FlowNode, direction: PortDirection, facing: PortSide| {
            let of: Vec<&FlowPort> = node.ports.iter().filter(|p| p.direction == direction).collect();
            of.iter()
                .find(|p| p.side == facing)
                .or(of.first())
                .map(|p| FlowEndpoint::new(node.id.clone(), p.id.clone()))
        };
        let ends = if forward {
            pick(from, PortDirection::Output, side).zip(pick(&made, PortDirection::Input, opposite))
        } else {
            pick(&made, PortDirection::Output, opposite).zip(pick(from, PortDirection::Input, side))
        };
        let edge = ends.map(|(a, b)| FlowEdge::new(graph.fresh_edge_id("e"), a, b));
        Some((made, edge))
    }

    /// Makes the next node beside `id` on `side`, joined to it, selects it and opens its title.
    fn quick_create(&self, id: &str, side: PortSide) -> bool {
        if self.locked.get() || !self.quick_create {
            return false;
        }
        let mut graph = self.graph.get();
        let Some(from) = graph.find_node(id).filter(|n| !n.group).cloned() else { return false };
        let Some((made, edge)) = self.next_to(&graph, &from, side) else { return false };
        let new_id = made.id.clone();
        let edge_id = edge.as_ref().map(|e| e.id.clone());
        graph.nodes.push(made);
        graph.edges.extend(edge);
        if !self.commit(graph, FlowChange::Create { node: new_id.clone(), edge: edge_id }) {
            return true;
        }
        self.select(FlowSelection::node(new_id.clone()));
        self.begin_rename(&new_id);
        true
    }

    fn begin_rename(&self, id: &str) -> bool {
        if self.locked.get() || !self.editable_titles {
            return false;
        }
        let Some(node) = self.graph.get().find_node(id).cloned() else { return false };
        let view = self.view.get();
        if view.zoom < TEXT_ZOOM {
            self.memory.view_before.set(Some(view));
            let size = self.memory.frame.get().size;
            let middle = node.rect().center();
            self.set_view(FlowViewport::at(
                Point::new(size.width / 2.0 - middle.x, size.height / 2.0 - middle.y),
                1.0,
            ));
        }
        self.select(FlowSelection::node(id));
        self.draft.set(node.title);
        self.editing.set(Some(id.to_owned()));
        true
    }

    /// Closes the title being typed, keeping what was typed when `keep` and the app lets it.
    /// An empty or unchanged title changes nothing. The node gets the focus back.
    fn end_rename(&self, keep: bool) {
        let Some(id) = self.editing.get() else { return };
        self.editing.set(None);
        if let Some(view) = self.memory.view_before.take() {
            self.set_view(view);
        }
        *self.memory.focus_node.borrow_mut() = Some(id.clone());
        let title = self.draft.get().trim().to_owned();
        let mut graph = self.graph.get();
        let Some(node) = graph.find_node_mut(&id) else { return };
        if !keep || title.is_empty() || node.title == title {
            return;
        }
        node.title = title;
        self.commit(graph, FlowChange::Rename { node: id });
    }

    /// Brings the selected nodes in front of the others, or sends them behind.
    fn reorder(&self, front: bool) -> bool {
        let selection = self.selection.get();
        if selection.nodes.is_empty() || self.locked.get() {
            return false;
        }
        let mut graph = self.graph.get();
        let changed =
            if front { graph.bring_to_front(&selection.nodes) } else { graph.send_to_back(&selection.nodes) };
        if changed {
            self.commit(graph, FlowChange::Reorder { nodes: selection.nodes.clone() });
        }
        true
    }

    /// Moves the selected nodes by this much, in units of the diagram.
    fn nudge(&self, dx: f32, dy: f32) -> bool {
        let selection = self.selection.get();
        if selection.nodes.is_empty() || self.locked.get() {
            return false;
        }
        let mut graph = self.graph.get();
        let moving = graph.with_descendants(&selection.nodes);
        for node in graph.nodes.iter_mut().filter(|n| moving.contains(&n.id)) {
            node.x += dx;
            node.y += dy;
        }
        Self::regroup(&mut graph, &moving);
        self.commit(graph, FlowChange::Move { nodes: moving });
        true
    }

    /// After a move, puts each moved node (that did not move with its group) in the smallest
    /// group its middle is over, or in none.
    fn regroup(graph: &mut FlowGraph, moved: &[String]) {
        for id in moved {
            let Some(node) = graph.find_node(id) else { continue };
            if node.parent.as_ref().is_some_and(|p| moved.contains(p)) {
                continue;
            }
            let middle = node.rect().center();
            let inside = graph.descendants(id);
            let target = graph
                .nodes
                .iter()
                .filter(|g| g.group && &g.id != id && !inside.contains(&g.id) && g.rect().contains(middle))
                .min_by(|a, b| (a.width * a.height).total_cmp(&(b.width * b.height)))
                .map(|g| g.id.clone());
            if target != node.parent {
                graph.set_parent(id, target.as_deref());
            }
        }
    }

    /// Puts the selected nodes in a new group, which is then selected.
    fn group_selection(&self) -> bool {
        let selection = self.selection.get();
        if selection.nodes.is_empty() || self.locked.get() {
            return false;
        }
        let mut graph = self.graph.get();
        // Only the outermost of what is selected: a node whose group is selected goes with it.
        let members: Vec<String> = selection
            .nodes
            .iter()
            .filter(|id| {
                graph.find_node(id).and_then(|n| n.parent.clone()).is_none_or(|p| !selection.has_node(&p))
            })
            .cloned()
            .collect();
        let id = graph.fresh_node_id("group");
        let group = FlowNode::new_group(id.clone(), self.group_title.clone(), Rect::default());
        if !graph.group(&members, group) {
            return false;
        }
        if self.commit(graph, FlowChange::Group { group: id.clone(), nodes: members }) {
            self.select(FlowSelection::node(id));
        }
        true
    }

    /// Takes the selected groups apart, selecting what they held.
    fn ungroup_selection(&self) -> bool {
        let selection = self.selection.get();
        if self.locked.get() {
            return false;
        }
        let mut graph = self.graph.get();
        let mut freed: Vec<String> = Vec::new();
        let mut groups: Vec<String> = Vec::new();
        for id in &selection.nodes {
            let held: Vec<String> = graph.children(id).map(|n| n.id.clone()).collect();
            if graph.ungroup(id) {
                freed.extend(held);
                groups.push(id.clone());
            }
        }
        if groups.is_empty() {
            return false;
        }
        if self.commit(graph, FlowChange::Ungroup { groups }) {
            self.select(FlowSelection { nodes: freed, edges: Vec::new() });
        }
        true
    }

    /// Turns the selected nodes by `degrees` (or puts them straight, with `None`). A group among
    /// them refuses, saying why.
    fn turn_by(&self, degrees: Option<f32>) -> bool {
        let selection = self.selection.get();
        if selection.nodes.is_empty() || self.locked.get() || !self.rotatable {
            return false;
        }
        let mut graph = self.graph.get();
        if selection.nodes.iter().any(|id| graph.find_node(id).is_some_and(|n| n.group)) {
            self.notice.set(Some(self.no_turn.clone()));
            return true;
        }
        for node in graph.nodes.iter_mut().filter(|n| selection.has_node(&n.id)) {
            node.rotation = match degrees {
                Some(by) => (node.rotation + by).rem_euclid(360.0),
                None => 0.0,
            };
        }
        self.commit(graph, FlowChange::Rotate { nodes: selection.nodes.clone() });
        true
    }

    /// A drag on the round handle above a node, which turns it around its middle: the angle of
    /// the pointer around it, 15 degrees at a time unless `free`.
    fn on_turn_drag(&self, id: &str, phase: DragPhase, position: Point, free: bool) {
        if self.locked.get() || !self.rotatable {
            return;
        }
        match phase {
            DragPhase::Start => {
                self.forget_notice();
                *self.memory.gesture.borrow_mut() =
                    Gesture::Turn { id: id.to_owned(), before: self.graph.get() };
            }
            DragPhase::Move => {
                let Gesture::Turn { id, .. } = self.memory.gesture.borrow().clone() else { return };
                let mut graph = self.graph.get();
                let Some(node) = graph.find_node_mut(&id) else { return };
                let middle = self.view.get().to_screen(node.rect().center());
                let at = self.local(position);
                // Straight up is no turn: the handle starts above the node.
                let mut degrees = (at.y - middle.y).atan2(at.x - middle.x).to_degrees() + 90.0;
                if !free {
                    degrees = (degrees / TURN_STEP).round() * TURN_STEP;
                }
                let degrees = degrees.rem_euclid(360.0);
                if node.rotation != degrees {
                    node.rotation = degrees;
                    self.graph.set(graph);
                }
            }
            DragPhase::End => {
                let gesture = std::mem::take(&mut *self.memory.gesture.borrow_mut());
                if let Gesture::Turn { id, before } = gesture {
                    self.settle(before, FlowChange::Rotate { nodes: vec![id] });
                }
            }
            // The base may add more; they act as nothing here.
            _ => {}
        }
    }

    /// Makes the selected nodes larger or smaller by this much, no smaller than [`MIN_NODE`].
    fn resize_by(&self, dw: f32, dh: f32) -> bool {
        let selection = self.selection.get();
        if selection.nodes.is_empty() || self.locked.get() || !self.resizable {
            return false;
        }
        let mut graph = self.graph.get();
        for node in graph.nodes.iter_mut().filter(|n| selection.has_node(&n.id)) {
            node.width = (node.width + dw).max(MIN_NODE.0);
            node.height = (node.height + dh).max(MIN_NODE.1);
        }
        self.commit(graph, FlowChange::Resize { nodes: selection.nodes.clone() });
        true
    }

    /// Lays the diagram out in layers, on the grid when nodes snap to it, and fits it in view.
    fn arrange(&self) -> bool {
        if self.locked.get() {
            return false;
        }
        let mut graph = self.graph.get();
        self.layout.apply(&mut graph);
        for node in &mut graph.nodes {
            node.x = self.snapped(node.x);
            node.y = self.snapped(node.y);
        }
        if !self.commit(graph, FlowChange::Arrange) {
            return true;
        }
        self.fit();
        // From the button, the keys come back to the canvas: Cmd+Z undoes the arrangement.
        self.memory.refocus.set(true);
        true
    }

    /// A drag on the corner of a node, which resizes it.
    fn on_resize_drag(&self, id: &str, phase: DragPhase, position: Point) {
        if self.locked.get() || !self.resizable {
            return;
        }
        let at = self.local(position);
        match phase {
            DragPhase::Start => {
                self.forget_notice();
                let graph = self.graph.get();
                let Some(node) = graph.find_node(id) else { return };
                let size = (node.width, node.height);
                *self.memory.gesture.borrow_mut() =
                    Gesture::Resize { id: id.to_owned(), pressed: at, size, before: graph };
            }
            DragPhase::Move => {
                let Gesture::Resize { id, pressed, size, .. } = self.memory.gesture.borrow().clone() else {
                    return;
                };
                let zoom = self.view.get().zoom.max(f32::EPSILON);
                // The pointer's way in the node's own axes, for a turned node.
                let turn = self.graph.get().find_node(&id).map_or(0.0, |n| n.rotation.to_radians());
                let (dx, dy) = ((at.x - pressed.x) / zoom, (at.y - pressed.y) / zoom);
                let (sin, cos) = (-turn).sin_cos();
                let (dx, dy) = (dx * cos - dy * sin, dx * sin + dy * cos);
                let (w, h) = (size.0 + dx, size.1 + dy);
                let (w, h) = (self.snapped(w).max(MIN_NODE.0), self.snapped(h).max(MIN_NODE.1));
                let mut graph = self.graph.get();
                if let Some(node) = graph.find_node_mut(&id)
                    && (node.width != w || node.height != h)
                {
                    node.width = w;
                    node.height = h;
                    self.graph.set(graph);
                }
            }
            DragPhase::End => {
                let gesture = std::mem::take(&mut *self.memory.gesture.borrow_mut());
                if let Gesture::Resize { id, before, .. } = gesture {
                    self.settle(before, FlowChange::Resize { nodes: vec![id] });
                }
            }
            // The base may add more; they act as nothing here.
            _ => {}
        }
    }

    fn delete_selection(&self) -> bool {
        let selection = self.selection.get();
        if selection.is_empty() || self.locked.get() {
            return false;
        }
        let mut graph = self.graph.get();
        if graph.remove(&selection.nodes, &selection.edges) {
            let change =
                FlowChange::Delete { nodes: selection.nodes.clone(), edges: selection.edges.clone() };
            if !self.commit(graph, change) {
                return true;
            }
            self.memory.refocus.set(!selection.nodes.is_empty());
        }
        self.select(FlowSelection::default());
        true
    }

    /// Goes back one change (or forward again, with `redo`), keeping in the selection what is
    /// still there.
    fn travel(&self, redo: bool) -> bool {
        if self.locked.get() {
            return false;
        }
        let now = self.graph.get();
        let mut history = self.history.get();
        let Some(graph) = (if redo { history.redo(now.clone()) } else { history.undo(now.clone()) }) else {
            return false;
        };
        if !self.allowed(&now, &graph, if redo { &FlowChange::Redo } else { &FlowChange::Undo }) {
            return true;
        }
        self.history.set(history);
        let mut selection = self.selection.get();
        selection.nodes.retain(|id| graph.find_node(id).is_some());
        selection.edges.retain(|id| graph.find_edge(id).is_some());
        let before = self.graph.get();
        self.memory.refocus.set(before.nodes.iter().any(|n| graph.find_node(&n.id).is_none()));
        self.graph.set(graph);
        self.select(selection);
        true
    }

    /// Puts the selected nodes and the connections between them on the clipboard, as JSON.
    fn copy(&self) -> bool {
        let selection = self.selection.get();
        if selection.nodes.is_empty() {
            return false;
        }
        let json = self.graph.get().extract(&selection.nodes).to_json();
        self.clipboard.write(&json);
        *self.memory.copied.borrow_mut() = Some(json);
        self.memory.paste_step.set(2);
        true
    }

    fn cut(&self) -> bool {
        if self.locked.get() || !self.copy() {
            return false;
        }
        self.delete_selection();
        // The first paste after a cut puts the nodes back where they were.
        self.memory.paste_step.set(0);
        true
    }

    /// Adds the nodes on the clipboard, with new ids, and selects them.
    fn paste(&self) -> bool {
        if self.locked.get() {
            return false;
        }
        let text = self.clipboard.read().or_else(|| self.memory.copied.borrow().clone());
        let Some(piece) = text.and_then(|t| FlowGraph::from_json(&t).ok()) else { return false };
        if piece.nodes.is_empty() {
            return false;
        }
        let step = self.memory.paste_step.get();
        self.memory.paste_step.set(step + 2);
        let along = self.grid * step as f32;
        let mut graph = self.graph.get();
        let (nodes, edges) = graph.insert(&piece, along, along);
        if self.commit(graph, FlowChange::Paste { nodes: nodes.clone(), edges: edges.clone() }) {
            self.select(FlowSelection { nodes, edges });
        }
        true
    }

    /// The keys every part of the canvas answers.
    fn on_key(&self, key: &Key, m: Modifiers) -> bool {
        let (shift, command) = (m.shift, m.meta || m.ctrl);
        self.forget_notice();
        let step = self.grid * if shift { 4.0 } else { 1.0 };
        let has_nodes = !self.selection.get().nodes.is_empty();
        if *key == Key::Escape && self.connecting.get().is_some() {
            self.connecting.set(None);
            return true;
        }
        if command && let Key::Character(c) = key {
            match c.to_lowercase().as_str() {
                "z" => return self.travel(shift),
                "y" => return self.travel(true),
                "c" => return self.copy(),
                "x" => return self.cut(),
                "v" => return self.paste(),
                "g" if shift => return self.ungroup_selection(),
                "g" => return self.group_selection(),
                "]" | "}" => return self.reorder(true),
                "[" | "{" => return self.reorder(false),
                _ => {}
            }
        }
        if m.alt && shift {
            match key {
                Key::ArrowLeft => return self.turn_by(Some(-TURN_STEP)),
                Key::ArrowRight => return self.turn_by(Some(TURN_STEP)),
                Key::Character(c) if c == "0" || m.base_key == Some('0') => return self.turn_by(None),
                _ => {}
            }
        }
        if m.alt {
            match key {
                Key::ArrowLeft => return self.resize_by(-step, 0.0),
                Key::ArrowRight => return self.resize_by(step, 0.0),
                Key::ArrowUp => return self.resize_by(0.0, -step),
                Key::ArrowDown => return self.resize_by(0.0, step),
                _ => {}
            }
        }
        if *key == Key::Enter
            && command
            && let [only] = self.selection.get().nodes.as_slice()
        {
            return self.quick_create(only, if shift { PortSide::Bottom } else { PortSide::Right });
        }
        if *key == Key::Function(2)
            && let [only] = self.selection.get().nodes.as_slice()
        {
            return self.begin_rename(only);
        }
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
                let points = screen_points(&graph, edge, &view, &self.memory.routes)?;
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
                self.forget_notice();
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
            let r = node.extent();
            let (a, b) = (view.to_screen(r.origin), view.to_screen(Point::new(r.max_x(), r.max_y())));
            let touches = a.x < area.max_x() && b.x > area.x() && a.y < area.max_y() && b.y > area.y();
            if touches && !next.has_node(&node.id) {
                next.nodes.push(node.id.clone());
            }
        }
        next
    }

    /// A press and drag on a node.
    fn on_node_drag(&self, id: &str, phase: DragPhase, position: Point, m: Modifiers) {
        let shift = m.shift;
        let at = self.local(position);
        self.memory.pointer.set(Some(at));
        match phase {
            DragPhase::Start => {
                self.forget_notice();
                // A group lies under the lines inside it: a press on one of them picks the line.
                let on_group = self.graph.get().find_node(id).is_some_and(|n| n.group);
                if on_group && let Some(edge) = self.edge_at(at) {
                    self.select(FlowSelection::edge(edge));
                    *self.memory.gesture.borrow_mut() = Gesture::Idle;
                    return;
                }
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
                let moving = graph.with_descendants(&selection.nodes);
                let origins = graph
                    .nodes
                    .iter()
                    .filter(|n| moving.contains(&n.id))
                    .map(|n| (n.id.clone(), n.x, n.y))
                    .collect();
                *self.memory.gesture.borrow_mut() =
                    Gesture::Move { pressed: at, origins, before: graph, lead: id.to_owned() };
            }
            DragPhase::Move => {
                let Gesture::Move { pressed, origins, lead, .. } = self.memory.gesture.borrow().clone()
                else {
                    return;
                };
                let zoom = self.view.get().zoom.max(f32::EPSILON);
                let (mut dx, mut dy) = ((at.x - pressed.x) / zoom, (at.y - pressed.y) / zoom);
                let mut graph = self.graph.get();
                // Guides: the node held lines up its edges or middle with another node's, within
                // a few pixels on screen, before the grid; Alt does without.
                let (mut across, mut down) = (None, None);
                if !m.alt
                    && let Some((_, x0, y0)) = origins.iter().find(|(id, _, _)| *id == lead)
                    && let Some(held) = graph.find_node(&lead)
                {
                    let moving: Vec<&String> = origins.iter().map(|(id, _, _)| id).collect();
                    let others: Vec<Rect> = graph
                        .nodes
                        .iter()
                        .filter(|n| !n.group && !moving.contains(&&n.id))
                        .map(FlowNode::extent)
                        .collect();
                    let reach = GUIDE_REACH / zoom;
                    let (w, h) = (held.width, held.height);
                    let lines = |at: f32, size: f32| [at, at + size / 2.0, at + size];
                    let best = |mine: [f32; 3], theirs: &dyn Fn(&Rect) -> [f32; 3]| {
                        others
                            .iter()
                            .flat_map(|r| theirs(r))
                            .flat_map(|t| mine.iter().map(move |m| (t - m, t)))
                            .filter(|(d, _)| d.abs() <= reach)
                            .min_by(|a, b| a.0.abs().total_cmp(&b.0.abs()))
                    };
                    if let Some((d, t)) = best(lines(x0 + dx, w), &|r: &Rect| lines(r.x(), r.width())) {
                        dx += d;
                        across = Some(t);
                    }
                    if let Some((d, t)) = best(lines(y0 + dy, h), &|r: &Rect| lines(r.y(), r.height())) {
                        dy += d;
                        down = Some(t);
                    }
                }
                self.guides.set_if_changed((across, down));
                let mut changed = false;
                for (id, x, y) in &origins {
                    let nx = if across.is_some() { x + dx } else { self.snapped(x + dx) };
                    let ny = if down.is_some() { y + dy } else { self.snapped(y + dy) };
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
                self.guides.set_if_changed((None, None));
                let gesture = std::mem::take(&mut *self.memory.gesture.borrow_mut());
                if let Gesture::Move { before, origins, .. } = gesture {
                    let mut graph = self.graph.get();
                    if before == graph {
                        return;
                    }
                    let moved: Vec<String> = origins.into_iter().map(|(id, _, _)| id).collect();
                    Self::regroup(&mut graph, &moved);
                    if graph != self.graph.get() {
                        self.graph.set(graph);
                    }
                    // The whole move is one change to undo, not one per step of the pointer, and
                    // the app is asked once, now.
                    self.settle(before, FlowChange::Move { nodes: moved });
                }
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

    /// The diagram a connection is checked against: without the connection whose end moves,
    /// so it may go back where it was.
    fn without(graph: &FlowGraph, moving: Option<&String>) -> FlowGraph {
        match moving {
            Some(id) => {
                let mut g = graph.clone();
                g.edges.retain(|e| &e.id != id);
                g
            }
            None => graph.clone(),
        }
    }

    /// The selected connection with an end at this port, whose end a drag from it moves.
    fn selected_end(&self, port: &FlowEndpoint) -> Option<(String, FlowEndpoint)> {
        let selection = self.selection.get();
        let graph = self.graph.get();
        graph.edges.iter().filter(|e| selection.has_edge(&e.id)).find_map(|e| {
            if &e.to == port {
                Some((e.id.clone(), e.from.clone()))
            } else if &e.from == port {
                Some((e.id.clone(), e.to.clone()))
            } else {
                None
            }
        })
    }

    /// What dropping the connection being made on `end` would do.
    fn verdict(&self, connecting: &Connecting, end: &FlowEndpoint) -> Result<(), String> {
        let graph = Self::without(&self.graph.get(), connecting.moving.as_ref());
        let (a, b) = Self::oriented(&graph, &connecting.from, end);
        self.check(&graph, &a, &b)
    }

    /// Ends the connection being made on `end`: adds it, or moves the end of the one being
    /// moved, and selects it. Says whether it ended: false when the ports refuse, true when it
    /// was made or the app refused it (which shows why).
    fn finish(&self, connecting: &Connecting, end: &FlowEndpoint) -> bool {
        if self.verdict(connecting, end).is_err() {
            return false;
        }
        let mut graph = self.graph.get();
        let (a, b) = Self::oriented(&graph, &connecting.from, end);
        let id = match &connecting.moving {
            Some(id) => {
                let Some(edge) = graph.edges.iter_mut().find(|e| &e.id == id) else { return false };
                edge.from = a;
                edge.to = b;
                id.clone()
            }
            None => {
                let id = graph.fresh_edge_id("e");
                graph.edges.push(FlowEdge::new(id.clone(), a, b));
                id
            }
        };
        self.connecting.set(None);
        let change = match &connecting.moving {
            Some(_) => FlowChange::Reconnect { edge: id.clone() },
            None => FlowChange::Connect { edge: id.clone() },
        };
        if self.commit(graph, change) {
            self.select(FlowSelection::edge(id));
        }
        true
    }

    /// A drag that starts on a port: a new connection, or, when a selected connection ends
    /// there, that end moving.
    fn on_port_drag(&self, port: &FlowEndpoint, phase: DragPhase, position: Point) {
        if self.locked.get() {
            return;
        }
        let at = self.local(position);
        self.memory.pointer.set(Some(at));
        match phase {
            DragPhase::Start => {
                self.forget_notice();
                let (moving, from) = match self.selected_end(port) {
                    Some((edge, other)) => (Some(edge), other),
                    None => (None, port.clone()),
                };
                self.connecting.set(Some(Connecting {
                    from,
                    pointer: at,
                    target: None,
                    moving,
                    keyboard: false,
                }));
            }
            DragPhase::Move => {
                let Some(mut connecting) = self.connecting.get() else { return };
                let (graph, view) = (self.graph.get(), self.view.get());
                connecting.target = nearest_port(&graph, &view, at, &connecting.from).map(|end| {
                    let verdict = self.verdict(&connecting, &end);
                    (end, verdict)
                });
                connecting.pointer = at;
                self.connecting.set_if_changed(Some(connecting));
            }
            DragPhase::End => {
                let Some(connecting) = self.connecting.get() else { return };
                self.connecting.set(None);
                if let Some((end, Ok(()))) = &connecting.target {
                    self.finish(&connecting, end);
                }
            }
            // The base may add more; they act as nothing here.
            _ => {}
        }
    }

    /// Enter or Space on a port: starts a connection there, or ends the one started on another
    /// port; on the port it started from, gives it up. A refused end stays open, saying why.
    fn on_port_enter(&self, port: &FlowEndpoint) -> bool {
        if self.locked.get() {
            return false;
        }
        let pointer = {
            let (graph, view) = (self.graph.get(), self.view.get());
            graph.anchor(port).map_or(Point::new(0.0, 0.0), |(p, _)| view.to_screen(p))
        };
        match self.connecting.get() {
            Some(connecting) if &connecting.from == port => self.connecting.set(None),
            Some(mut connecting) => {
                if !self.finish(&connecting, port) {
                    connecting.target = Some((port.clone(), self.verdict(&connecting, port)));
                    connecting.pointer = pointer;
                    self.connecting.set(Some(connecting));
                }
            }
            None => {
                let (moving, from) = match self.selected_end(port) {
                    Some((edge, other)) => (Some(edge), other),
                    None => (None, port.clone()),
                };
                self.connecting.set(Some(Connecting { from, pointer, target: None, moving, keyboard: true }));
            }
        }
        true
    }
}

/// A connection's line in the view, as points; step lines are worked out again only when
/// something near them changed (`routes`).
fn screen_points(
    graph: &FlowGraph,
    edge: &FlowEdge,
    view: &FlowViewport,
    routes: &RefCell<geometry::RouteCache>,
) -> Option<Vec<Point>> {
    let path = graph.edge_path_cached(edge, &mut routes.borrow_mut())?;
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
    /// The alignment guides: a tone of their own, apart from the selection's accent.
    guide: Color,
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
            guide: theme.color(Semantic::Guide),
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
    guides: (Option<f32>, Option<f32>),
    grid: f32,
    /// The corner radius of a group's panel at 100%.
    radius: f32,
    palette: Palette,
    /// Where the lines worked out before are kept.
    memory: Rc<Memory>,
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
        // The panels of the groups, under the lines, outermost first as the diagram has them.
        let radius = self.radius;
        for group in self.graph.nodes.iter().filter(|n| n.group) {
            let r = group.rect();
            let (a, b) =
                (self.view.to_screen(r.origin), self.view.to_screen(Point::new(r.max_x(), r.max_y())));
            let panel = Rect::new(frame.x() + a.x, frame.y() + a.y, b.x - a.x, b.y - a.y);
            scene.push_quad(Quad::new(panel, p.surface.with_alpha(0.7)).radius(radius * self.view.zoom));
        }
        let touching = |edge: &FlowEdge| {
            self.selection.has_edge(&edge.id)
                || self.selection.has_node(&edge.from.node)
                || self.selection.has_node(&edge.to.node)
        };
        let moving = self.connecting.as_ref().and_then(|c| c.moving.as_deref());
        for edge in self.graph.edges.iter().filter(|e| Some(e.id.as_str()) != moving) {
            let Some(points) = screen_points(&self.graph, edge, &self.view, &self.memory.routes) else {
                continue;
            };
            // The line reaches the edge of each port's dot (the start only when it has a cap), so
            // its caps show; then it stops inside each cap.
            let dot = PORT_DOT * self.view.zoom.clamp(0.75, 1.25) / 2.0 + 1.0;
            let mut reach: Vec<Point> = geometry::trim_end(&points, dot).into_iter().map(at).collect();
            if edge.start != EdgeCap::None {
                reach = geometry::trim_start(&reach, dot);
            }
            let caps = [
                (edge.end, reach.last().copied(), geometry::end_direction(&reach)),
                (edge.start, reach.first().copied(), {
                    let back: Vec<Point> = reach.iter().rev().copied().collect();
                    geometry::end_direction(&back)
                }),
            ];
            let points =
                geometry::trim_start(&geometry::trim_end(&reach, edge.end.inset(CAP)), edge.start.inset(CAP));
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
            for (cap, tip, direction) in caps {
                let (Some(tip), Some(direction)) = (tip, direction) else { continue };
                let outline = geometry::cap_outline(cap, tip, direction, CAP);
                if cap.filled() {
                    fill(scene, outline, color);
                } else if !outline.is_empty() {
                    stroke(scene, &geometry::flatten(&outline), width, color);
                }
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
        if let Some(x) = self.guides.0 {
            let sx = frame.x() + self.view.to_screen(Point::new(x, 0.0)).x;
            stroke(scene, &[Point::new(sx, frame.y()), Point::new(sx, frame.max_y())], 1.0, p.guide);
        }
        if let Some(y) = self.guides.1 {
            let sy = frame.y() + self.view.to_screen(Point::new(0.0, y)).y;
            stroke(scene, &[Point::new(frame.x(), sy), Point::new(frame.max_x(), sy)], 1.0, p.guide);
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
        let own_history = cx.use_state(FlowHistory::new);
        let locked = cx.use_state(|| self.locked);
        let connecting = cx.use_state(|| None::<Connecting>);
        let marquee = cx.use_state(|| None::<Rect>);
        let guides = cx.use_state(|| (None::<f32>, None::<f32>));
        let notice = cx.use_state(|| None::<String>);
        let editing = cx.use_state(|| None::<String>);
        let draft = cx.use_state(String::new);
        let memory = cx.use_state(|| Rc::new(Memory::default())).get();
        if memory.refocus.take() {
            cx.request_focus();
        }
        let flow = Flow {
            graph: self.graph.clone(),
            view: self.view.clone().unwrap_or_else(|| own_view.binding()),
            selection: self.selection.clone().unwrap_or_else(|| own_selection.binding()),
            history: self.history.clone().unwrap_or_else(|| own_history.binding()),
            clipboard: cx.clipboard(),
            locked: locked.clone(),
            connecting: connecting.clone(),
            marquee: marquee.clone(),
            guides: guides.clone(),
            memory: memory.clone(),
            validate: self.validate.clone(),
            guard: self.guard.clone(),
            notice: notice.clone(),
            layout: self.layout,
            resizable: self.resizable,
            rotatable: self.rotatable,
            no_turn: labels.flow_no_turn.clone(),
            next: self.next.clone(),
            quick_create: self.quick_create,
            editable_titles: self.editable_titles,
            editing: editing.clone(),
            draft: draft.clone(),
            grid: self.grid,
            snap: self.snap,
            range: self.range,
            refused: labels.flow_refused.clone(),
            group_title: labels.flow_group.clone(),
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
        let editing_now = editing.get();
        let is_locked = locked.get();

        let mut children = Vec::new();
        for node in &graph.nodes {
            let look = self.kinds.iter().find(|(k, _)| *k == node.kind).map(|(_, look)| look.clone());
            let inside = if node.group { None } else { self.content.as_ref().and_then(|f| f(node)) };
            let part = NodeBox {
                inside,
                node: node.clone(),
                look,
                view,
                selected: selection.has_node(&node.id),
                resizable: self.resizable && !is_locked && selection.has_node(&node.id),
                rotatable: self.rotatable,
                quick: self.quick_create && !is_locked,
                selected_many: selection.nodes.len() > 1,
                members: graph.children(&node.id).count(),
                editing: editing_now.as_deref() == Some(node.id.as_str()),
                handles: graph
                    .edges
                    .iter()
                    .filter(|e| selection.has_edge(&e.id))
                    .flat_map(|e| [&e.from, &e.to])
                    .filter(|end| end.node == node.id)
                    .map(|end| end.port.clone())
                    .collect(),
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
            let Some(points) = screen_points(&graph, edge, &view, &memory.routes) else { continue };
            let Some(middle) = geometry::midpoint(&points) else { continue };
            children.push(edge_label(cx, label, middle, &theme, palette));
        }
        match &connecting_now {
            Some(Connecting { pointer, target: Some((_, Err(reason))), .. }) => {
                children.push(refusal(cx, reason, *pointer, palette));
            }
            Some(Connecting { pointer, keyboard: true, .. }) => {
                children.push(hint(cx, &labels.flow_pick_port, *pointer, palette));
            }
            _ => {}
        }
        // Why the app refused the last change, by the pointer, until the next gesture.
        if let Some(reason) = notice.get() {
            let at = memory.pointer.get().unwrap_or(Point::new(FIT_MARGIN, FIT_MARGIN));
            children.push(refusal(cx, &reason, at, palette));
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
            guides: guides.get(),
            grid: self.grid,
            radius: theme.radii.lg,
            palette,
            memory: memory.clone(),
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
        cx.on_key(root, Rc::new(move |key, m| keyed.0.on_key(key, m)));
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
fn refusal(cx: &mut BuildCx<'_>, reason: &str, at: Point, p: Palette) -> NodeId {
    bubble(cx, IconName::CircleX, p.danger, reason, at, p)
}

/// What to do next while a connection is made with the keys, beside the port it starts from.
fn hint(cx: &mut BuildCx<'_>, text: &str, at: Point, p: Palette) -> NodeId {
    bubble(cx, IconName::Link, p.accent, text, at, p)
}

/// A few words beside a point of the view, outlined in `tone`, which screen readers announce.
fn bubble(cx: &mut BuildCx<'_>, icon: IconName, tone: Color, text: &str, at: Point, p: Palette) -> NodeId {
    util::bubble(cx, icon, tone, text, Point::new(at.x + 16.0, at.y + 16.0), p.ink, p.elevated)
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
    let (a, b, c, d, e) = (flow.clone(), flow.clone(), flow.clone(), flow.locked.clone(), flow.clone());
    let zoom_in = button(cx, IconName::Plus, &labels.zoom_in, Rc::new(move || a.zoom_by(ZOOM_STEP)));
    let zoom_out = button(cx, IconName::Minus, &labels.zoom_out, Rc::new(move || b.zoom_by(1.0 / ZOOM_STEP)));
    let fit = button(cx, IconName::Maximize2, &labels.zoom_fit, Rc::new(move || c.fit()));
    let arrange = cx.child(
        &IconButton::new(IconName::LayoutDashboard)
            .label(labels.flow_arrange.clone())
            .size(ButtonSize::Small)
            .style(ButtonStyle::Ghost)
            .disabled(locked)
            .on_click(move || {
                e.arrange();
            }),
    );
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
        &[zoom_in, zoom_out, fit, arrange, lock, percent],
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
    /// The app's view inside the card, from [`FlowCanvas::content`].
    inside: Option<AnyView>,
    node: FlowNode,
    look: Option<FlowKind>,
    view: FlowViewport,
    selected: bool,
    /// Its ports where a selected connection ends, which show as handles to drag.
    handles: Vec<String>,
    /// Shows the corner that resizes it.
    resizable: bool,
    /// Shows the handle that turns it (with the corner, on a selected node).
    rotatable: bool,
    /// Shows the arrows of quick creation while the pointer is over it.
    quick: bool,
    /// More than one node is selected: the arrows stay away.
    selected_many: bool,
    /// For a group, how many nodes it holds directly.
    members: usize,
    /// Its title is being typed.
    editing: bool,
    connecting: Option<Connecting>,
    connected: Vec<String>,
    flow: Flow,
    palette: Palette,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl NodeBox {
    /// The field over the title while it is typed: where the title is, never wider than the
    /// node on screen, except that a node narrower than 120 pixels gets a 240-pixel field.
    fn title_field(&self, cx: &mut BuildCx<'_>, theme: &Theme, w: f32, h: f32) -> NodeId {
        let zoom = self.view.zoom;
        let pad = theme.spacing.s3 * zoom;
        let field_h = 30.0;
        let (x, y, width) = if self.node.group {
            (pad, (group_header(zoom) - field_h) / 2.0, w - pad * 2.0)
        } else {
            let x = pad + 36.0 * zoom + pad;
            (x, (h - field_h) / 2.0, w - x - pad)
        };
        let width = if w < 120.0 { 240.0 } else { width.max(80.0) };
        let flow = (self.flow.clone(), self.flow.clone(), self.flow.clone());
        let field = TextField::new(self.flow.draft.binding())
            .placeholder(cx.labels().flow_title.clone())
            .compact()
            .autofocus()
            .select_on_open()
            .on_submit(move |_| flow.0.end_rename(true))
            .on_cancel(move || flow.1.end_rename(false))
            .on_leave(move || flow.2.end_rename(true));
        let field = cx.child(&field);
        cx.container(
            LayoutStyle {
                absolute: Some((x, y)),
                width: Some(width),
                height: Some(field_h),
                ..LayoutStyle::leaf()
            },
            &[field],
            Paint::None,
        )
    }

    /// The node's description for screen readers: its kind, its line and how it is doing, and
    /// for a group, how many nodes it holds.
    fn description(&self, labels: &crate::Labels) -> String {
        let mut parts = Vec::new();
        if self.node.group {
            parts.push(labels.flow_group.clone());
            parts.push(labels.flow_members.replace("{0}", &self.members.to_string()));
        }
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
        if self.node.rotation != 0.0 {
            parts.push(labels.flow_turned.replace("{0}", &format!("{}", self.node.rotation.round())));
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
                _ if self.handles.contains(&port.id) => PortState::Handle,
                _ if self.selected => PortState::Selected,
                _ if self.connected.contains(&port.id) => PortState::Connected,
                _ => PortState::Free,
            };
            let template =
                if port.direction == PortDirection::Input { &labels.flow_input } else { &labels.flow_output };
            let dot = PortDot {
                at: Point::new((at.x - node.x) * view.zoom, (at.y - node.y) * view.zoom),
                zoom: view.zoom,
                name: port.label.clone().unwrap_or_else(|| port.id.clone()),
                description: template.replace("{0}", &node.title),
                end,
                state,
                flow: self.flow.clone(),
                palette: p,
            };
            ports.push(cx.keyed_child(util::hash_text(&port.id), &dot));
        }

        if self.editing {
            ports.push(self.title_field(cx, &theme, w, h));
        }
        // Over the node, or on a node selected alone, so the pointer can cross to an arrow.
        let near = cx.is_hover_within() || self.selected;
        if near && self.quick && !node.group && !self.selected_many {
            for side in [PortSide::Right, PortSide::Bottom, PortSide::Left, PortSide::Top] {
                let arrow = NextArrow {
                    size: Size::new(w, h),
                    side,
                    id: node.id.clone(),
                    flow: self.flow.clone(),
                    palette: p,
                };
                ports.push(cx.keyed_child(util::hash_text(&format!("\u{0}next{side:?}")), &arrow));
            }
        }
        if self.resizable && self.rotatable && !node.group {
            let turner = Turner { width: w, id: node.id.clone(), flow: self.flow.clone(), palette: p };
            ports.push(cx.keyed_child(util::hash_text("\u{0}turner"), &turner));
        }
        if self.resizable {
            let corner =
                Corner { size: Size::new(w, h), id: node.id.clone(), flow: self.flow.clone(), palette: p };
            ports.push(cx.keyed_child(util::hash_text("\u{0}corner"), &corner));
        }
        // The app's view goes first, under the ports and handles: laid out at the node's size
        // in units of the diagram and drawn at the zoom.
        let card_only = self.inside.is_some();
        if let Some(inside) = &self.inside {
            let shown = cx.keyed_child(util::hash_text("\u{0}inside"), inside);
            let holder = cx.container(
                LayoutStyle {
                    absolute: Some((0.0, 0.0)),
                    width: Some(node.width),
                    height: Some(node.height),
                    align: Align::Stretch,
                    ..LayoutStyle::stack(Axis::Vertical)
                },
                &[shown],
                Paint::None,
            );
            cx.scale(holder, view.zoom);
            ports.insert(0, holder);
        }
        let look = self.look.clone();
        let (title, subtitle, status) = (node.title.clone(), node.subtitle.clone(), node.status.clone());
        let selected = self.selected;
        let zoom = view.zoom;
        let is_group = node.group;
        let shape = node.shape;
        // The outline on screen, from the node's corner, for drawing and for the pointer.
        let outline: Rc<[Point]> = node
            .outline()
            .into_iter()
            .map(|q| Point::new((q.x - node.x) * zoom, (q.y - node.y) * zoom))
            .collect();
        let drawn = outline.clone();
        let paint = Paint::Rich(Rc::new(move |frame: Rect, paint: &mut PaintCx<'_>| {
            if is_group {
                draw_group(paint, frame, zoom, &theme, p, &title, selected, hovered);
                return;
            }
            if shape != FlowShape::Card && !card_only {
                let at: Vec<Point> =
                    drawn.iter().map(|q| Point::new(frame.x() + q.x, frame.y() + q.y)).collect();
                draw_shape(
                    paint,
                    frame,
                    &at,
                    shape,
                    zoom,
                    &theme,
                    p,
                    &title,
                    subtitle.as_deref(),
                    &status,
                    selected,
                    hovered,
                );
                return;
            }
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
                card_only,
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
        if !matches!(node.shape, FlowShape::Card | FlowShape::Rectangle | FlowShape::Note) && !node.group {
            cx.hit_outline(root, outline);
        }
        if node.rotation != 0.0 {
            cx.rotate(root, node.rotation.to_radians());
        }
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
        cx.on_drag(root, Rc::new(move |d| dragged.on_node_drag(&id, d.phase, d.position, modifiers.get())));
        let (renamed, id) = (self.flow.clone(), node.id.clone());
        cx.on_double_click(
            root,
            Rc::new(move || {
                renamed.begin_rename(&id);
            }),
        );
        let wanted = self.flow.memory.focus_node.borrow().as_deref() == Some(node.id.as_str());
        if wanted {
            self.flow.memory.focus_node.borrow_mut().take();
            cx.request_focus();
        }
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
                if *key == Key::Enter && (m.meta || m.ctrl) {
                    let side = if m.shift { PortSide::Bottom } else { PortSide::Right };
                    return keyed.quick_create(&id, side);
                }
                if *key == Key::Enter || *key == Key::Function(2) {
                    let selected = keyed.selection.get() == FlowSelection::node(id.clone());
                    if (selected || *key == Key::Function(2)) && keyed.begin_rename(&id) {
                        return true;
                    }
                    keyed.select(FlowSelection::node(id.clone()));
                    return true;
                }
                keyed.on_key(key, m)
            }),
        );
        root
    }
}

/// Draws a node in a shape other than the card: the outline filled and edged, its title (and
/// line) in the middle; a note wraps its title over lines from its top left corner.
#[allow(clippy::too_many_arguments, reason = "what one shape shows")]
fn draw_shape(
    paint: &mut PaintCx<'_>,
    frame: Rect,
    outline: &[Point],
    shape: FlowShape,
    zoom: f32,
    theme: &Theme,
    p: Palette,
    title: &str,
    subtitle: Option<&str>,
    status: &FlowNodeStatus,
    selected: bool,
    hovered: bool,
) {
    let note = shape == FlowShape::Note;
    let (edge, width) = match status {
        _ if selected => (p.accent, 2.0),
        FlowNodeStatus::Warning(_) => (p.warning, 2.0),
        FlowNodeStatus::Error(_) => (p.danger, 2.0),
        _ if note => (p.warning, 1.0),
        _ => (p.border, 1.0),
    };
    let fill = match () {
        _ if note => p.warning.with_alpha(0.18),
        _ if hovered => p.control,
        _ => p.elevated,
    };
    let path: Vec<ravenrs_core::PathElement> =
        outline
            .iter()
            .enumerate()
            .map(|(i, &q)| {
                if i == 0 {
                    ravenrs_core::PathElement::MoveTo(q)
                } else {
                    ravenrs_core::PathElement::LineTo(q)
                }
            })
            .chain(std::iter::once(ravenrs_core::PathElement::Close))
            .collect();
    paint.scene.push_path(PathFill::solid(path, fill));
    let mut ring: Vec<Point> = outline.to_vec();
    ring.extend(outline.first().copied());
    stroke(paint.scene, &ring, width, edge);
    if zoom < TEXT_ZOOM {
        return;
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
    let pad = theme.spacing.s3 * zoom;
    let ink = if *status == FlowNodeStatus::Disabled { p.muted } else { p.ink };
    if note {
        let mut layout = paint.text.layout(title, &scaled(TextStyle::Callout, None));
        layout.measure(Some((frame.width() - pad * 2.0).max(0.0)));
        layout.paint(Point::new(frame.x() + pad, frame.y() + pad), ink, paint.scene);
        return;
    }
    // Words stay inside the narrower middle of a diamond or an ellipse.
    let room = match shape {
        FlowShape::Diamond | FlowShape::Ellipse => frame.width() * 0.6,
        _ => frame.width() - pad * 2.0,
    }
    .max(0.0);
    let mut lines: Vec<(String, TextAttrs, Color)> =
        vec![(title.to_owned(), scaled(TextStyle::Body, Some(Weight::SEMIBOLD)), ink)];
    if let Some(line) = subtitle {
        lines.push((line.to_owned(), scaled(TextStyle::Callout, None), p.muted));
    }
    let total: f32 = lines.iter().map(|(_, a, _)| a.line_height).sum();
    let mut y = frame.y() + (frame.height() - total) / 2.0;
    for (text, attrs, color) in lines {
        let mut layout = paint.text.layout_ellipsized(&text, &attrs, room);
        let wide = layout.measure(None).width;
        layout.paint(Point::new(frame.center().x - wide / 2.0, y), color, paint.scene);
        y += attrs.line_height;
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
    card_only: bool,
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
    if card_only {
        return;
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

/// Draws a group's edge and title at the zoom; [`Surface::paint`] lays its panel under the lines.
#[allow(clippy::too_many_arguments, reason = "what one panel shows")]
fn draw_group(
    paint: &mut PaintCx<'_>,
    frame: Rect,
    zoom: f32,
    theme: &Theme,
    p: Palette,
    title: &str,
    selected: bool,
    hovered: bool,
) {
    // The panel itself is painted by the surface, under the lines; here only its edge and title,
    // so the lines inside a group show.
    let radius = theme.radii.lg * zoom;
    let (edge, width) = match () {
        _ if selected => (p.accent, 2.0),
        _ if hovered => (p.muted, 1.0),
        _ => (p.border, 1.0),
    };
    paint.scene.push_quad(Quad::new(frame, Color::TRANSPARENT).radius(radius).border(width, edge));
    if zoom < TEXT_ZOOM {
        return;
    }
    let a = util::attrs(theme, TextStyle::Callout, Some(Weight::SEMIBOLD));
    let attrs = TextAttrs { size: a.size * zoom, line_height: a.line_height * zoom, ..a };
    let pad = theme.spacing.s3 * zoom;
    let room = (frame.width() - pad * 2.0).max(0.0);
    let header = group_header(zoom);
    let y = frame.y() + (header - attrs.line_height) / 2.0;
    let layout = paint.text.layout_ellipsized(title, &attrs, room);
    layout.paint(Point::new(frame.x() + pad, y), if selected { p.accent } else { p.muted }, paint.scene);
}

/// The height of a group's title band on screen: the room [`FlowNode::GROUP_PADDING`] leaves
/// above what it holds.
fn group_header(zoom: f32) -> f32 {
    FlowNode::GROUP_PADDING.1 * zoom
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
    /// A selected connection ends here: dragging it moves that end.
    Handle,
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
    /// What screen readers call it, and what it is for.
    name: String,
    description: String,
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
                PortState::Accepts | PortState::Source | PortState::Handle => Some(p.accent),
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
                PortState::Selected | PortState::Source | PortState::Handle => {
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
        cx.focusable(node, PORT_HIT / 2.0);
        cx.semantics(
            node,
            Semantics::new(Role::Button)
                .name(self.name.clone())
                .description(self.description.clone())
                .selected(matches!(state, PortState::Source | PortState::Handle)),
        );
        let (dragged, end) = (self.flow.clone(), self.end.clone());
        cx.on_drag(node, Rc::new(move |d| dragged.on_port_drag(&end, d.phase, d.position)));
        let (keyed, end) = (self.flow.clone(), self.end.clone());
        cx.on_key(
            node,
            Rc::new(move |key, m| match key {
                Key::Enter | Key::Space => keyed.on_port_enter(&end),
                _ => keyed.on_key(key, m),
            }),
        );
        node
    }
}

/// The corner of a selected node that resizes it when dragged. The keys do the same with Alt
/// and the arrows, so screen readers do not need it.
struct Corner {
    /// The node's size on screen.
    size: Size,
    id: String,
    flow: Flow,
    palette: Palette,
}

impl View for Corner {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let p = self.palette;
        let paint = Paint::Custom(Rc::new(move |frame: Rect, scene: &mut Scene| {
            // Two short strokes across the corner, as on a window's edge.
            let end = Point::new(frame.max_x() - 3.0, frame.max_y() - 3.0);
            for reach in [5.0, 9.0] {
                stroke(
                    scene,
                    &[Point::new(end.x - reach, end.y), Point::new(end.x, end.y - reach)],
                    1.5,
                    p.accent,
                );
            }
        }));
        let node = cx.container(
            LayoutStyle {
                absolute: Some((self.size.width - HANDLE, self.size.height - HANDLE)),
                width: Some(HANDLE),
                height: Some(HANDLE),
                ..LayoutStyle::leaf()
            },
            &[],
            paint,
        );
        cx.cursor(node, CursorIcon::NwseResize);
        cx.semantics(node, Semantics::hidden());
        let (dragged, id) = (self.flow.clone(), self.id.clone());
        cx.on_drag(node, Rc::new(move |d| dragged.on_resize_drag(&id, d.phase, d.position)));
        node
    }
}

/// An arrow on a side of the node under the pointer: a click makes the next node there.
struct NextArrow {
    /// The node's size on screen.
    size: Size,
    side: PortSide,
    id: String,
    flow: Flow,
    palette: Palette,
}

impl View for NextArrow {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        const ARROW: f32 = 24.0;
        const OUT: f32 = 12.0;
        let (w, h) = (self.size.width, self.size.height);
        let (x, y) = match self.side {
            PortSide::Right => (w + OUT, h / 2.0 - ARROW / 2.0),
            PortSide::Left => (-OUT - ARROW, h / 2.0 - ARROW / 2.0),
            PortSide::Bottom => (w / 2.0 - ARROW / 2.0, h + OUT),
            PortSide::Top => (w / 2.0 - ARROW / 2.0, -OUT - ARROW),
        };
        let (p, side, hovered) = (self.palette, self.side, cx.is_hovered());
        let paint = Paint::Custom(Rc::new(move |frame: Rect, scene: &mut Scene| {
            let r = ARROW / 2.0;
            let fill = if hovered { p.accent } else { p.elevated };
            let ink = if hovered { Color::WHITE } else { p.accent };
            scene.push_quad(Quad::new(frame, fill).radius(r).border(1.0, p.accent));
            let c = frame.center();
            let d = side.outward();
            let tip = Point::new(c.x + d.x * 6.0, c.y + d.y * 6.0);
            let tail = Point::new(c.x - d.x * 6.0, c.y - d.y * 6.0);
            stroke(scene, &[tail, tip], 2.0, ink);
            fill_arrow(scene, tip, d, ink);
        }));
        let node = cx.container(
            LayoutStyle {
                absolute: Some((x, y)),
                width: Some(ARROW),
                height: Some(ARROW),
                ..LayoutStyle::leaf()
            },
            &[],
            paint,
        );
        cx.cursor(node, CursorIcon::Pointer);
        // The keys do the same with Cmd/Ctrl+Enter, so screen readers do not need the arrows.
        cx.semantics(node, Semantics::hidden());
        let (made, id, side) = (self.flow.clone(), self.id.clone(), self.side);
        cx.on_click(
            node,
            Rc::new(move || {
                made.quick_create(&id, side);
            }),
        );
        node
    }
}

/// A small filled arrowhead at `tip` pointing along `direction`.
fn fill_arrow(scene: &mut Scene, tip: Point, direction: Point, color: Color) {
    fill(scene, geometry::arrow_head(tip, direction, 6.0), color);
}

/// The round handle above a selected node that turns it when dragged. The keys do the same with
/// Alt+Shift and the arrows, so screen readers do not need it.
struct Turner {
    /// The node's width on screen.
    width: f32,
    id: String,
    flow: Flow,
    palette: Palette,
}

impl View for Turner {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let p = self.palette;
        let paint = Paint::Custom(Rc::new(move |frame: Rect, scene: &mut Scene| {
            let c = Point::new(frame.center().x, frame.y() + TURNER / 2.0);
            // The stem down to the node, then the round handle on it.
            stroke(
                scene,
                &[Point::new(c.x, c.y + TURNER / 2.0), Point::new(c.x, frame.max_y())],
                2.0,
                p.accent,
            );
            let r = TURNER / 2.0;
            scene.push_quad(
                Quad::new(Rect::new(c.x - r, c.y - r, TURNER, TURNER), p.elevated)
                    .radius(r)
                    .border(2.0, p.accent),
            );
        }));
        let node = cx.container(
            LayoutStyle {
                absolute: Some((self.width / 2.0 - TURNER / 2.0, -(TURNER + TURNER_GAP))),
                width: Some(TURNER),
                height: Some(TURNER + TURNER_GAP),
                ..LayoutStyle::leaf()
            },
            &[],
            paint,
        );
        cx.cursor(node, CursorIcon::Pointer);
        cx.semantics(node, Semantics::hidden());
        let modifiers = cx.modifiers();
        let (dragged, id) = (self.flow.clone(), self.id.clone());
        cx.on_drag(
            node,
            Rc::new(move |d| dragged.on_turn_drag(&id, d.phase, d.position, modifiers.get().shift)),
        );
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
                (n.extent(), color)
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
