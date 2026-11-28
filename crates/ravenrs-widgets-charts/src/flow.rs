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
    EdgeCap, EdgeShape, FLOW_FORMAT, FlowColors, FlowDirection, FlowDocument, FlowEdge, FlowEndpoint,
    FlowExportFormat, FlowGate, FlowGraph, FlowHistory, FlowJsonError, FlowLayout, FlowLayoutKind, FlowNode,
    FlowNodeRun, FlowNodeStatus, FlowPicture, FlowPort, FlowRun, FlowRunEvent, FlowRunRules, FlowRunView,
    FlowShape, FlowShelf, FlowStyle, FlowText, NODE_REFS, PortDirection, PortSide,
};

use crate::context::{BuildCx, DragPhase, NodeId, Paint, PaintCx, Semantics};
use crate::{
    Alert, AnyView, ButtonSize, ButtonStyle, Clipboard, DropdownMenu, IconButton, IconName, MenuItem,
    TextField, View, util,
};

/// What a node shows inside its card instead of its icon and words, for
/// [`FlowCanvas::content`].
pub type FlowContent = Rc<dyn Fn(&FlowNode) -> Option<AnyView>>;

/// The least and the most zoom by default.
const ZOOM_RANGE: (f32, f32) = (0.25, 2.0);
/// How much a press of a zoom button or of `+`/`-` changes the zoom.
const ZOOM_STEP: f32 = 1.25;
/// Room left around the diagram when it is fitted to the view, in screen pixels.
const FIT_MARGIN: f32 = 48.0;
/// How tall the bar of zoom controls is, in screen pixels.
const CONTROLS_HEIGHT: f32 = 40.0;
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
/// How near the straight between its neighbours a bend point let go is taken out, in screen
/// pixels.
const BEND_STRAIGHT: f32 = 4.0;
fn flow_run_limit() -> usize {
    ravenrs_graphics::flow::RUN_LIMIT + 1
}

/// The debug box, at the canvas's top right: the buttons of the run, the ways out to choose
/// from, what went wrong and the last things that happened.
#[allow(clippy::too_many_arguments, reason = "what the box shows")]
fn debug_box(
    cx: &mut BuildCx<'_>,
    flow: &Flow,
    graph: &FlowGraph,
    rules: &FlowRunRules,
    run: Option<&FlowRun>,
    size: Size,
    theme: &Theme,
    labels: &crate::Labels,
) -> NodeId {
    let (f1, f2, f3, f4) = (flow.clone(), flow.clone(), flow.clone(), flow.clone());
    let over = run.is_some_and(FlowRun::is_over);
    let waiting = run.is_some_and(|r| r.choosing.is_some());
    let mut rows: Vec<NodeId> = Vec::new();
    rows.push(util::label(
        cx,
        &labels.flow_debug,
        TextStyle::Caption,
        Some(Weight::SEMIBOLD),
        theme.color(Semantic::InkMuted),
    ));
    let buttons = crate::HStack::new((
        crate::Button::new(labels.flow_run_start.clone())
            .icon(IconName::Play)
            .size(ButtonSize::Small)
            .on_click(move || f1.run_start()),
        crate::Button::new(labels.flow_run_step.clone())
            .size(ButtonSize::Small)
            .style(ButtonStyle::Secondary)
            .disabled(over || waiting)
            .on_click(move || f2.run_step(None)),
        crate::Button::new(labels.flow_run_continue.clone())
            .size(ButtonSize::Small)
            .style(ButtonStyle::Secondary)
            .disabled(over || waiting)
            .on_click(move || f3.run_on()),
        crate::Button::new(labels.flow_run_stop.clone())
            .icon(IconName::Square)
            .size(ButtonSize::Small)
            .style(ButtonStyle::Ghost)
            .disabled(run.is_none())
            .on_click(move || f4.run.set(None)),
    ))
    .spacing(4.0);
    rows.push(cx.child(&buttons));
    let f5 = flow.clone();
    rows.push(
        cx.child(
            &crate::Button::new(labels.flow_run_breakpoint.clone())
                .size(ButtonSize::Small)
                .style(ButtonStyle::Ghost)
                .disabled(flow.selection.get().nodes.is_empty())
                .on_click(move || f5.toggle_breakpoints()),
        ),
    );
    if let Some(r) = run {
        let many = r
            .choosing
            .as_ref()
            .and_then(|n| graph.find_node(n))
            .is_some_and(|n| (rules.gate)(n) == FlowGate::ChooseMany);
        if let (Some(node), true) = (&r.choosing, many) {
            // One or more ways: a box each, then Follow.
            let name = graph.find_node(node).map_or(node.clone(), |n| n.title.clone());
            rows.push(util::label(
                cx,
                &labels.flow_run_choose_many.replace("{0}", &name),
                TextStyle::Callout,
                None,
                theme.color(Semantic::Ink),
            ));
            for e in FlowRun::ways_out(graph, rules, node) {
                let to = graph.find_node(&e.to.node).map_or(e.to.node.clone(), |n| n.title.clone());
                let words = e.label.clone().map_or(to.clone(), |l| format!("{l} → {to}"));
                let (picked, id) = (flow.picked.clone(), e.id.clone());
                let (reader, writer) = (picked.clone(), id.clone());
                let ticked = ravenrs_reactive::Binding::new(
                    move || reader.get().contains(&id),
                    move |on: bool| {
                        let mut all = picked.get();
                        all.retain(|w| *w != writer);
                        if on {
                            all.push(writer.clone());
                        }
                        picked.set(all);
                    },
                );
                rows.push(cx.child(&crate::Checkbox::new(words, ticked)));
            }
            let follow = flow.clone();
            rows.push(
                cx.child(
                    &crate::Button::new(labels.flow_run_follow.clone())
                        .size(ButtonSize::Small)
                        .on_click(move || follow.run_ways()),
                ),
            );
            if flow.refused_choice.get() {
                rows.push(util::label(
                    cx,
                    &labels.flow_run_none_chosen,
                    TextStyle::Caption,
                    None,
                    theme.color(Semantic::Danger),
                ));
            }
        } else if let Some(node) = &r.choosing {
            let ways: Vec<crate::Button> = FlowRun::ways_out(graph, rules, node)
                .into_iter()
                .map(|e| {
                    let to = graph.find_node(&e.to.node).map_or(e.to.node.clone(), |n| n.title.clone());
                    let words = e.label.clone().map_or(to.clone(), |l| format!("{l} → {to}"));
                    let (chosen, id) = (flow.clone(), e.id.clone());
                    crate::Button::new(words)
                        .size(ButtonSize::Small)
                        .style(ButtonStyle::Secondary)
                        .on_click(move || chosen.run_step(Some(&id)))
                })
                .collect();
            let name = graph.find_node(node).map_or(node.clone(), |n| n.title.clone());
            rows.push(util::label(
                cx,
                &labels.flow_run_choose.replace("{0}", &name),
                TextStyle::Callout,
                None,
                theme.color(Semantic::Ink),
            ));
            rows.push(cx.child(&crate::VStack::new(ways).spacing(4.0).align(Align::Stretch)));
        }
        if let Some(failure) = r.failure() {
            rows.push(cx.child(&Alert::message(run_words(failure, graph, labels)).tone(crate::Tone::Danger)));
        } else if r.is_over() {
            rows.push(cx.child(&Alert::message(labels.flow_run_done.clone()).tone(crate::Tone::Success)));
        }
        for event in r.events.iter().rev().take(6).rev() {
            let color = if event.is_error() {
                theme.color(Semantic::Danger)
            } else {
                theme.color(Semantic::InkMuted)
            };
            rows.push(util::label(cx, &run_words(event, graph, labels), TextStyle::Caption, None, color));
        }
    } else {
        rows.push(util::label(
            cx,
            &labels.flow_run_hint,
            TextStyle::Caption,
            None,
            theme.color(Semantic::InkMuted),
        ));
    }
    let width = CHECK_WIDTH;
    let root = cx.container(
        LayoutStyle {
            absolute: Some(((size.width - width - theme.spacing.s4).max(0.0), theme.spacing.s4)),
            width: Some(width),
            padding: ravenrs_core::Insets::uniform(theme.spacing.s3),
            spacing: theme.spacing.s2,
            align: Align::Stretch,
            ..LayoutStyle::stack(Axis::Vertical)
        },
        &rows,
        Paint::Quad(crate::context::QuadPaint {
            corner_radius: theme.radii.md,
            border: Some(ravenrs_core::Border { width: 1.0, color: theme.color(Semantic::Border) }),
            shadow: Some(theme.elevation.soft),
            ..crate::context::QuadPaint::new(theme.color(Semantic::Surface))
        }),
    );
    cx.semantics(root, Semantics::new(Role::Group).name(labels.flow_debug.clone()));
    root
}

/// What a run's event says, in the app's words.
fn run_words(event: &FlowRunEvent, graph: &FlowGraph, labels: &crate::Labels) -> String {
    let name = |id: &str| {
        graph
            .find_node(id)
            .map(|n| if n.title.trim().is_empty() { n.id.clone() } else { n.title.clone() })
            .unwrap_or_else(|| id.to_owned())
    };
    match event {
        FlowRunEvent::Moved { from, to, .. } => {
            labels.flow_run_moved.replace("{0}", &name(from)).replace("{1}", &name(to))
        }
        FlowRunEvent::Ended { node } => labels.flow_run_ended.replace("{0}", &name(node)),
        FlowRunEvent::Choosing { node } => labels.flow_run_choose.replace("{0}", &name(node)),
        FlowRunEvent::Finished => labels.flow_run_done.clone(),
        FlowRunEvent::DeadEnd { node } => labels.flow_run_dead_end.replace("{0}", &name(node)),
        FlowRunEvent::Stuck { node } => labels.flow_run_stuck.replace("{0}", &name(node)),
        FlowRunEvent::Looping => labels.flow_run_looping.clone(),
        _ => labels.flow_run_no_start.clone(),
    }
}

/// How wide the box of a check is, in logical pixels.
const CHECK_WIDTH: f32 = 340.0;
/// The least zoom at which ports show their names.
const PORT_LABEL_ZOOM: f32 = 0.5;
/// The side of a bend point's square, in screen pixels.
const BEND: f32 = 10.0;
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

    /// The zoom and offset that show `bounds` whole and centered in a view of `size`. A short
    /// strip keeps at most a sixth of its height around the diagram, not the whole margin.
    pub fn fitting(bounds: Rect, size: Size, least: f32, most: f32) -> Self {
        let margin = FIT_MARGIN.min(size.height / 6.0).min(size.width / 6.0);
        let room = Size::new((size.width - margin * 2.0).max(1.0), (size.height - margin * 2.0).max(1.0));
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

/// Something wrong with a diagram, said in words, with the nodes it is about, which the
/// canvas outlines while the check is shown.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FlowIssue {
    /// What is wrong.
    pub text: String,
    /// The nodes it is about, by id.
    pub nodes: Vec<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FlowIssue {
    /// `text`, about these nodes.
    pub fn new(text: impl Into<String>, nodes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self { text: text.into(), nodes: nodes.into_iter().map(Into::into).collect() }
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

    /// The look of a node with `status`, for [`FlowCanvas::status_icons`].
    fn for_status(self, status: &FlowNodeStatus) -> Self {
        let (icon, tone) = match status {
            FlowNodeStatus::Success => (IconName::CircleCheck, Semantic::Success),
            FlowNodeStatus::Running => (IconName::LoaderCircle, Semantic::Accent),
            FlowNodeStatus::Error(_) => (IconName::CircleAlert, Semantic::Danger),
            FlowNodeStatus::Cancelled => (IconName::CircleX, Semantic::InkMuted),
            _ => return self,
        };
        Self { icon, tone, ..self }
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
    /// The points a connection's line passes through were added, moved or taken out.
    Bend {
        /// Its id.
        edge: String,
    },
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
    /// Properties changed in a [`FlowInspector`](crate::FlowInspector).
    Edit {
        /// The nodes changed.
        nodes: Vec<String>,
        /// The connections changed.
        edges: Vec<String>,
    },
    /// A node was made next to another, by its arrows or Cmd/Ctrl+Enter.
    Create {
        /// The new node.
        node: String,
        /// The connection made to it, when the two have ports to join.
        edge: Option<String>,
    },
}

/// The kind of a [`DragPayload`](crate::DragPayload) that carries a [`FlowNode`] to a
/// [`FlowCanvas`], as a [`FlowToolbox`](crate::FlowToolbox) does: dropped on the canvas, a copy
/// of the node is made there, with an id of its own.
pub const FLOW_NODE_DRAG: &str = "ravenrs/flow-node";

/// Gets the diagram, or the part of it selected, as a picture in the format chosen from the
/// canvas's Export menu, to save it (`ravenrs-flow-export` writes it as a file).
pub type FlowExporter = Rc<dyn Fn(FlowExportFormat, FlowPicture)>;

/// Gives the node made next to `from`, on `side` of it, by the arrows of quick creation: its
/// id and place are set by the canvas. `None` makes nothing on that side.
pub type FlowNext = Rc<dyn Fn(&FlowNode, PortSide) -> Option<FlowNode>>;

/// Gives a connection made on the canvas, or moved to other ports, the look and kind it should
/// have for its ends (in the diagram as it is), from the plain one the canvas made.
pub type FlowConnector = Rc<dyn Fn(&FlowGraph, FlowEdge) -> FlowEdge>;

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
    arrivals: Option<Binding<Option<FlowNode>>>,
    export: Option<FlowExporter>,
    connector: Option<FlowConnector>,
    checks: Option<crate::flow_notation::FlowChecks>,
    runs: Option<FlowRunRules>,
    quiet_ports: bool,
    quick_create: bool,
    grid: f32,
    snap: bool,
    controls: bool,
    hint: Option<String>,
    minimap: bool,
    status_icons: bool,
    locked: bool,
    read_only: bool,
    wheel_pans: bool,
    execution: Option<FlowRunView>,
    open_end: Option<Rc<Vec<(String, FlowNode)>>>,
    cables: bool,
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
            arrivals: None,
            export: None,
            connector: None,
            checks: None,
            runs: None,
            quiet_ports: false,
            quick_create: true,
            grid: 24.0,
            snap: true,
            controls: true,
            hint: None,
            minimap: true,
            status_icons: false,
            locked: false,
            read_only: false,
            wheel_pans: true,
            execution: None,
            open_end: None,
            cables: false,
            range: ZOOM_RANGE,
            label: None,
            width: None,
            height: None,
            grow: 1.0,
        }
    }

    /// Shapes every connection made or moved on the canvas: `shape` gets the diagram and the
    /// plain connection (its id and ends set) and gives it back with its kind, look and
    /// caps, as a notation draws that kind of line between those ends.
    pub fn connection(mut self, shape: impl Fn(&FlowGraph, FlowEdge) -> FlowEdge + 'static) -> Self {
        self.connector = Some(Rc::new(shape));
        self
    }

    /// Offers Export in the controls, and Cmd/Ctrl+Shift+E: the diagram, or with "Only the
    /// selection" what is selected, goes to `export` as a picture in the format chosen, PNG,
    /// SVG or PDF (on paper colors), which `ravenrs_flow_export::saver` turns into a file
    /// handed to the person. Exporting changes nothing, and works when locked too.
    pub fn on_export(mut self, export: impl Fn(FlowExportFormat, FlowPicture) + 'static) -> Self {
        self.export = Some(Rc::new(export));
        self
    }

    /// Takes the nodes left here, as a [`FlowToolbox`](crate::FlowToolbox) given the same
    /// binding leaves the one picked with Enter: each is copied to the middle of the view, with
    /// an id of its own, selected, and the binding emptied.
    pub fn arrivals(mut self, arrivals: Binding<Option<FlowNode>>) -> Self {
        self.arrivals = Some(arrivals);
        self
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

    /// Offers Check in the controls: what `checks` finds wrong shows in a box over the canvas,
    /// kept up to date while it is open, and the nodes it is about are outlined.
    pub fn checks(mut self, checks: impl Fn(&FlowGraph) -> Vec<FlowIssue> + 'static) -> Self {
        self.checks = Some(Rc::new(checks));
        self
    }

    /// Offers Debug in the controls: the diagram runs a step at a time as `rules` say, tokens
    /// going from the starts along the connections. The nodes they are at, have left and
    /// stopped at wrongly show it, the ways taken move, a way out is chosen where there is a
    /// choice, F9 puts a breakpoint on the nodes selected, and what happened is listed.
    pub fn runs(mut self, rules: FlowRunRules) -> Self {
        self.runs = Some(rules);
        self
    }

    /// Takes from `notation` how its kinds look, which connections may be made and how they
    /// are drawn, what comes next from the keys, and what makes a diagram right.
    pub fn notation(mut self, notation: &crate::FlowNotation) -> Self {
        for (kind, look) in &notation.looks {
            Rc::make_mut(&mut self.kinds).push((kind.clone(), look.clone()));
        }
        if notation.validate.is_some() {
            self.validate = notation.validate.clone();
        }
        if notation.connector.is_some() {
            self.connector = notation.connector.clone();
        }
        if notation.next.is_some() {
            self.next = notation.next.clone();
        }
        if notation.checks.is_some() {
            self.checks = notation.checks.clone();
        }
        if notation.runs.is_some() {
            self.runs = notation.runs.clone();
        }
        if let Some(cables) = notation.cables {
            self.cables = cables;
        }
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

    /// Shows the ports that nothing joins only by the node under the pointer (and while a
    /// connection is made), as a whiteboard does; the ports in use always show.
    pub fn ports_on_hover(mut self, quiet: bool) -> Self {
        self.quiet_ports = quiet;
        self
    }

    /// Whether the arrows and Cmd/Ctrl+Enter make the next node (on by default).
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

    /// A short line at the end of the zoom controls that says what a click does, such as "Click
    /// a stage to see only its log". It shows with the controls.
    pub fn hint(mut self, text: impl Into<String>) -> Self {
        self.hint = Some(text.into());
        self
    }

    /// Whether the map shows in the bottom right corner (on by default).
    pub fn minimap(mut self, minimap: bool) -> Self {
        self.minimap = minimap;
        self
    }

    /// The icon of a node of a [`kind`](Self::kind) says how it went once its status says so: a
    /// check when it passed, a turning circle while it runs, a mark when it failed or was
    /// cancelled. Before that, and for any other status, the kind's own icon shows. Off by default.
    pub fn status_icons(mut self, on: bool) -> Self {
        self.status_icons = on;
        self
    }

    /// Starts with editing locked: the view still pans and zooms, but nothing moves, connects
    /// or goes. The lock in the controls toggles it.
    pub fn locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        self
    }

    /// Locks editing for good, as for a run that already happened: the view still pans and
    /// zooms and nodes can be selected and read, but the lock is not offered in the controls,
    /// so nothing unlocks it.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Whether the wheel and the two-finger swipe pan the diagram (on by default). Off, they
    /// go to what holds the canvas, so a page scrolls past a diagram set in it instead of
    /// stopping on it; dragging and the keys still pan.
    pub fn pan_with_wheel(mut self, pans: bool) -> Self {
        self.wheel_pans = pans;
        self
    }

    /// Letting go of a connection where no port is, or pressing Enter again on the port it
    /// started from, opens a search of these models there, each with the words it is found by.
    /// Only the models the port accepts are listed, by the same rules as any connection, and
    /// choosing one creates it there, connected, as one change: one Undo takes both away.
    /// Escape closes the search and creates nothing.
    pub fn open_end(mut self, models: impl IntoIterator<Item = (impl Into<String>, FlowNode)>) -> Self {
        self.open_end = Some(Rc::new(models.into_iter().map(|(words, node)| (words.into(), node)).collect()));
        self
    }

    /// Shows a run that happened outside the canvas, as the app tells it: each node's
    /// status, drawn and said to screen readers, and on each connection how many items went
    /// along it, after its own words. Only the drawing changes, never the graph; give the run
    /// again as it moves on, and a node it does not name keeps its own status.
    pub fn execution(mut self, run: FlowRunView) -> Self {
        self.execution = Some(run);
        self
    }

    /// Draws the connections as cables that hang, swing and settle: a new one drops into
    /// its sag, moving a node or an end makes its cables swing, and one taken away falls.
    /// Off by default. Under reduced motion they show at rest; exports draw them at rest.
    pub fn cables(mut self, cables: bool) -> Self {
        self.cables = cables;
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

    /// A fixed height and the width its parent gives: a strip of stages across a page. A parent
    /// that stretches it gives it its width; one that does not (a column that fits what it holds,
    /// a row beside a text) gives it the width of its diagram, never more than the parent has, and
    /// what does not fit scrolls. With no node it keeps 240 points and its hint (#971).
    pub fn height(mut self, height: f32) -> Self {
        self.width = None;
        self.height = Some(height);
        self.grow = 0.0;
        self
    }
}

/// A connection let go where no port was, waiting for the node it will lead to: the port it
/// starts from and where it was let go, in the view.
#[derive(Clone, Debug, PartialEq)]
struct OpenEnd {
    from: FlowEndpoint,
    at: Point,
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
    /// Moving the bend point `index` of a connection, with the diagram before.
    Bend { edge: String, index: usize, before: FlowGraph },
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
    /// The connections drawn as cables so far; `None` before the first paint, whose
    /// cables show up at rest instead of dropping.
    cable_edges: RefCell<Option<std::collections::HashSet<String>>>,
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
    /// The window's cables and the group of this canvas's, when it draws cables.
    cables: Option<(crate::Cables, u64)>,
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
    connector: Option<FlowConnector>,
    checks: Option<crate::flow_notation::FlowChecks>,
    runs: Option<FlowRunRules>,
    /// Whether the check's box is open.
    checking: State<bool>,
    /// Whether the debug box is open, the run going on and the nodes it stops at.
    debugging: State<bool>,
    run: State<Option<FlowRun>>,
    breakpoints: State<Vec<String>>,
    /// The ways out ticked where one or more are chosen, and why the last choice was refused.
    picked: State<Vec<String>>,
    refused_choice: State<bool>,
    export: Option<FlowExporter>,
    /// Whether Export takes only what is selected, and the format chosen last.
    export_only: State<bool>,
    export_format: State<FlowExportFormat>,
    /// Whether the Export menu is open, which Cmd/Ctrl+Shift+E opens.
    export_menu: State<bool>,
    palette: Palette,
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
    /// The models a connection let go in the open can lead to, and the search open now.
    open_models: Option<Rc<Vec<(String, FlowNode)>>>,
    opened: State<Option<OpenEnd>>,
    open_query: State<String>,
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
        let edge = ends.map(|(a, b)| self.shaped(graph, FlowEdge::new(graph.fresh_edge_id("e"), a, b)));
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

    /// The box a node carried in lands in, centred on `at` (in units of the diagram), on the grid.
    fn landing(&self, node: &FlowNode, at: Point) -> Rect {
        let (x, y) = (self.snapped(at.x - node.width / 2.0), self.snapped(at.y - node.height / 2.0));
        Rect::new(x, y, node.width, node.height)
    }

    /// Makes a copy of a node carried in and dropped at `at` (from the canvas's corner), with an
    /// id of its own, and selects it.
    fn drop_node(&self, template: &FlowNode, at: Point) {
        if self.locked.get() {
            return;
        }
        let place = self.landing(template, self.view.get().to_content(at));
        let mut graph = self.graph.get();
        let id = graph.fresh_node_id(&template.id);
        graph.nodes.push(FlowNode {
            id: id.clone(),
            x: place.x(),
            y: place.y(),
            parent: None,
            ..template.clone()
        });
        if self.commit(graph, FlowChange::Create { node: id.clone(), edge: None }) {
            self.select(FlowSelection::node(id));
            self.memory.refocus.set(true);
        }
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

    /// Hands the diagram, or what is selected when "Only the selection" is on, to the app's
    /// exporter as a picture in `format`.
    fn export(&self, format: FlowExportFormat) -> bool {
        let Some(export) = &self.export else { return false };
        self.export_format.set_if_changed(format);
        let graph = self.graph.get();
        let selection = self.selection.get();
        let only =
            (self.export_only.get() && !selection.nodes.is_empty()).then_some(selection.nodes.as_slice());
        let p = self.palette;
        let colors = match format {
            FlowExportFormat::Pdf => FlowColors::paper(),
            _ => FlowColors {
                background: p.canvas,
                node: p.elevated,
                border: p.border,
                ink: p.ink,
                muted: p.muted,
                line: p.accent,
                danger: p.danger,
                warning: p.warning,
                success: p.success,
            },
        };
        if let Some(picture) = graph.picture_with(only, colors, self.cables.is_some()) {
            export(format, picture);
        }
        true
    }

    /// Puts a breakpoint on each node selected, or takes it off.
    fn toggle_breakpoints(&self) {
        let mut points = self.breakpoints.get();
        for id in self.selection.get().nodes {
            match points.iter().position(|p| *p == id) {
                Some(i) => {
                    points.remove(i);
                }
                None => points.push(id),
            }
        }
        self.breakpoints.set(points);
    }

    /// Starts a run over, from the starts.
    fn run_start(&self) {
        if let Some(rules) = &self.runs {
            self.run.set(Some(FlowRun::start(&self.graph.get(), rules)));
        }
    }

    /// Moves the run one step, along `choice` where a way out is to be chosen.
    fn run_step(&self, choice: Option<&str>) {
        let Some(rules) = &self.runs else { return };
        let graph = self.graph.get();
        let mut run = self.run.get().unwrap_or_else(|| FlowRun::start(&graph, rules));
        run.step(&graph, rules, choice);
        self.run.set(Some(run));
    }

    /// Follows the ways out ticked, where one or more are chosen.
    fn run_ways(&self) {
        let Some(rules) = &self.runs else { return };
        let graph = self.graph.get();
        let Some(mut run) = self.run.get() else { return };
        let ticked = self.picked.get();
        let ways: Vec<&str> = ticked.iter().map(String::as_str).collect();
        match run.step_ways(&graph, rules, &ways) {
            Ok(()) => {
                self.picked.set(Vec::new());
                self.refused_choice.set(false);
                self.run.set(Some(run));
            }
            Err(_) => self.refused_choice.set(true),
        }
    }

    /// Moves the run on until it ends, a way out is to be chosen or a token reaches a
    /// breakpoint.
    fn run_on(&self) {
        let Some(rules) = &self.runs else { return };
        let graph = self.graph.get();
        let points = self.breakpoints.get();
        let mut run = self.run.get().unwrap_or_else(|| FlowRun::start(&graph, rules));
        for _ in 0..flow_run_limit() {
            if run.is_over() || run.choosing.is_some() {
                break;
            }
            run.step(&graph, rules, None);
            if run.tokens.first().is_some_and(|t| points.contains(t)) {
                break;
            }
        }
        self.run.set(Some(run));
    }

    /// A connection as the app's connector shapes it, keeping its id and ends.
    fn shaped(&self, graph: &FlowGraph, edge: FlowEdge) -> FlowEdge {
        match &self.connector {
            Some(shape) => {
                let (id, from, to) = (edge.id.clone(), edge.from.clone(), edge.to.clone());
                FlowEdge { id, from, to, ..shape(graph, edge) }
            }
            None => edge,
        }
    }

    /// Where `at` (in the view) lands in the diagram, on the grid.
    fn bend_point(&self, at: Point) -> (f32, f32) {
        let c = self.view.get().to_content(at);
        (self.snapped(c.x), self.snapped(c.y))
    }

    /// A drag on a bend point of a connection, or on the `+` in the middle of a stretch
    /// (`add`), which makes a point there, `index` among its points, and moves it.
    fn on_bend_drag(&self, edge: &str, index: usize, add: bool, phase: DragPhase, position: Point) {
        if self.locked.get() {
            return;
        }
        let at = self.local(position);
        let point = self.bend_point(at);
        match phase {
            DragPhase::Start => {
                self.forget_notice();
                let before = self.graph.get();
                if add {
                    let mut graph = before.clone();
                    if let Some(e) = graph.find_edge_mut(edge) {
                        e.waypoints.insert(index.min(e.waypoints.len()), point);
                    }
                    self.graph.set(graph);
                }
                *self.memory.gesture.borrow_mut() = Gesture::Bend { edge: edge.to_owned(), index, before };
            }
            DragPhase::Move => {
                let Gesture::Bend { edge, index, .. } = self.memory.gesture.borrow().clone() else { return };
                let mut graph = self.graph.get();
                if let Some(w) = graph.find_edge_mut(&edge).and_then(|e| e.waypoints.get_mut(index))
                    && *w != point
                {
                    *w = point;
                    self.graph.set(graph);
                }
            }
            DragPhase::End => {
                let gesture = std::mem::take(&mut *self.memory.gesture.borrow_mut());
                let Gesture::Bend { edge, index, before } = gesture else { return };
                // A point let go on the straight between its neighbours is not needed: it goes.
                let mut graph = self.graph.get();
                let zoom = self.view.get().zoom;
                let straight = graph.find_edge(&edge).and_then(|e| graph.edge_controls(e)).is_some_and(|c| {
                    c.len() > index + 2
                        && geometry::distance_to(&[c[index], c[index + 2]], c[index + 1]) * zoom
                            <= BEND_STRAIGHT
                });
                if straight && let Some(e) = graph.find_edge_mut(&edge) {
                    e.waypoints.remove(index);
                    self.graph.set(graph);
                }
                self.settle(before, FlowChange::Bend { edge });
            }
            // The base may add more; they act as nothing here.
            _ => {}
        }
    }

    /// The keys on a bend point: the arrows move it a step of the grid (four with Shift),
    /// Delete takes it out.
    fn bend_key(&self, edge: &str, index: usize, key: &Key, m: Modifiers) -> bool {
        if self.locked.get() {
            return false;
        }
        let step = self.grid.max(1.0) * if m.shift { 4.0 } else { 1.0 };
        let mut graph = self.graph.get();
        let Some(e) = graph.find_edge_mut(edge) else { return false };
        let Some(&(x, y)) = e.waypoints.get(index) else { return false };
        match key {
            Key::ArrowLeft => e.waypoints[index] = (x - step, y),
            Key::ArrowRight => e.waypoints[index] = (x + step, y),
            Key::ArrowUp => e.waypoints[index] = (x, y - step),
            Key::ArrowDown => e.waypoints[index] = (x, y + step),
            Key::Delete | Key::Backspace => {
                e.waypoints.remove(index);
                self.memory.refocus.set(true);
            }
            _ => return false,
        }
        self.commit(graph, FlowChange::Bend { edge: edge.to_owned() });
        true
    }

    /// A double click on a line: a bend point there, on the grid.
    fn add_bend_at(&self, at: Point) -> bool {
        if self.locked.get() {
            return false;
        }
        let Some(id) = self.edge_at(at) else { return false };
        let point = self.bend_point(at);
        let mut graph = self.graph.get();
        let Some(controls) = graph.find_edge(&id).and_then(|e| graph.edge_controls(e)) else { return false };
        let index = geometry::bend_index(&controls, Point::new(point.0, point.1)) - 1;
        if let Some(e) = graph.find_edge_mut(&id) {
            e.waypoints.insert(index, point);
        }
        if self.commit(graph, FlowChange::Bend { edge: id.clone() }) {
            self.select(FlowSelection::edge(id));
        }
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
        if *key == Key::Escape && self.opened.get().is_some() {
            self.opened.set(None);
            return true;
        }
        if *key == Key::Escape && self.connecting.get().is_some() {
            self.connecting.set(None);
            return true;
        }
        if *key == Key::Function(9) && self.runs.is_some() && has_nodes {
            self.toggle_breakpoints();
            return true;
        }
        if command && let Key::Character(c) = key {
            match c.to_lowercase().as_str() {
                "z" => return self.travel(shift),
                "y" => return self.travel(true),
                "c" => return self.copy(),
                "x" => return self.cut(),
                "v" => return self.paste(),
                "e" if shift && self.export.is_some() => {
                    self.export_menu.set(true);
                    return true;
                }
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

    /// A connection's line in the view: the cable where it hangs now, when the canvas
    /// draws cables, or the line worked out from its shape.
    fn line(&self, graph: &FlowGraph, edge: &FlowEdge, view: &FlowViewport) -> Option<Vec<Point>> {
        if let Some((cables, group)) = &self.cables
            && let Some(shape) = cables.shape(cable_id(*group, &edge.id))
        {
            return Some(shape.iter().map(|p| view.to_screen(*p)).collect());
        }
        screen_points(graph, edge, view, &self.memory.routes)
    }

    /// The connection whose line passes nearest `at` (in the view), if one is near enough.
    fn edge_at(&self, at: Point) -> Option<String> {
        let (graph, view) = (self.graph.get(), self.view.get());
        graph
            .edges
            .iter()
            .filter_map(|edge| {
                let points = self.line(&graph, edge, &view)?;
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
                        // Other nodes, and the frame the node held is in, so it lines up with its edges.
                        .filter(|n| {
                            !moving.contains(&&n.id)
                                && (!n.group || held.parent.as_deref() == Some(n.id.as_str()))
                        })
                        .map(FlowNode::extent)
                        .collect();
                    let reach = GUIDE_REACH / zoom;
                    let (w, h) = (held.width, held.height);
                    let lines = |at: f32, size: f32| [at, at + size / 2.0, at + size];
                    let best = |mine: [f32; 3], theirs: &dyn Fn(&Rect) -> [f32; 3]| {
                        others
                            .iter()
                            .flat_map(theirs)
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
                // A line whose two ends move takes its bend points along; with one end, they stay.
                if let Gesture::Move { before, .. } = &*self.memory.gesture.borrow()
                    && let Some((_, x0, y0)) = origins.iter().find(|(id, _, _)| *id == lead)
                {
                    let (nx, ny) = (
                        if across.is_some() { x0 + dx } else { self.snapped(x0 + dx) },
                        if down.is_some() { y0 + dy } else { self.snapped(y0 + dy) },
                    );
                    let moving = |id: &str| origins.iter().any(|(o, _, _)| o == id);
                    for edge in &mut graph.edges {
                        if edge.waypoints.is_empty() || !moving(&edge.from.node) || !moving(&edge.to.node) {
                            continue;
                        }
                        let Some(was) = before.find_edge(&edge.id) else { continue };
                        let shifted: Vec<(f32, f32)> =
                            was.waypoints.iter().map(|(x, y)| (x + nx - x0, y + ny - y0)).collect();
                        if shifted != edge.waypoints {
                            edge.waypoints = shifted;
                            changed = true;
                        }
                    }
                }
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
    /// `graph` with `model` placed at `at`, in the view, and connected to `from` by the same rules
    /// as any connection; the node's id and the connection's, or `None` when the port does not
    /// accept it.
    fn linked(
        &self,
        graph: &FlowGraph,
        from: &FlowEndpoint,
        model: &FlowNode,
        at: Point,
    ) -> Option<(FlowGraph, String, String)> {
        let outgoing = graph.port(from)?.direction == PortDirection::Output;
        let wanted = if outgoing { PortDirection::Input } else { PortDirection::Output };
        let port = model.ports.iter().find(|p| p.direction == wanted)?.id.clone();
        let mut next = graph.clone();
        let place = self.landing(model, self.view.get().to_content(at));
        let id = next.fresh_node_id(&model.id);
        next.nodes.push(FlowNode {
            id: id.clone(),
            x: place.x(),
            y: place.y(),
            parent: None,
            ..model.clone()
        });
        let end = FlowEndpoint::new(id.clone(), port);
        let (a, b) = if outgoing { (from.clone(), end) } else { (end, from.clone()) };
        self.check(&next, &a, &b).ok()?;
        let edge = next.fresh_edge_id("e");
        let shaped = self.shaped(&next, FlowEdge::new(edge.clone(), a, b));
        next.edges.push(shaped);
        Some((next, id, edge))
    }

    /// The models the search open now lists: those its port accepts, with its words found.
    fn open_choices(&self) -> Vec<(String, FlowNode)> {
        let (Some(open), Some(models)) = (self.opened.get(), &self.open_models) else { return Vec::new() };
        let (graph, query) = (self.graph.get(), self.open_query.get().to_lowercase());
        models
            .iter()
            .filter(|(words, _)| query.trim().is_empty() || words.to_lowercase().contains(query.trim()))
            .filter(|(_, model)| self.linked(&graph, &open.from, model, open.at).is_some())
            .cloned()
            .collect()
    }

    /// Creates `model` where the search is open, connected to its port, as one change.
    fn create_linked(&self, model: &FlowNode) {
        let Some(open) = self.opened.get() else { return };
        self.opened.set(None);
        let Some((graph, node, edge)) = self.linked(&self.graph.get(), &open.from, model, open.at) else {
            return;
        };
        if self.commit(graph, FlowChange::Create { node: node.clone(), edge: Some(edge) }) {
            self.select(FlowSelection::node(node));
            self.memory.refocus.set(true);
        }
    }

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
                let Some(i) = graph.edges.iter().position(|e| &e.id == id) else { return false };
                let moved = FlowEdge { from: a, to: b, ..graph.edges[i].clone() };
                graph.edges[i] = self.shaped(&graph, moved);
                id.clone()
            }
            None => {
                let id = graph.fresh_edge_id("e");
                let edge = self.shaped(&graph, FlowEdge::new(id.clone(), a, b));
                graph.edges.push(edge);
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
                match &connecting.target {
                    Some((end, Ok(()))) => {
                        self.finish(&connecting, end);
                    }
                    // Let go where no port is: the search of what it can lead to opens there.
                    None if connecting.moving.is_none() && self.open_models.is_some() => {
                        self.open_query.set(String::new());
                        self.opened.set(Some(OpenEnd { from: connecting.from.clone(), at }));
                    }
                    _ => {}
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
            // Enter again on the port it started from: the search opens there, when there is one.
            Some(connecting) if &connecting.from == port => {
                self.connecting.set(None);
                if self.open_models.is_some() {
                    self.open_query.set(String::new());
                    self.opened.set(Some(OpenEnd { from: port.clone(), at: pointer }));
                }
            }
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

/// The cable of the connection `edge` on the canvas of `group`, the same from frame to frame.
fn cable_id(group: u64, edge: &str) -> u64 {
    group ^ util::hash_text(edge).wrapping_mul(0x9e37_79b9_7f4a_7c15)
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

pub(crate) fn fill(scene: &mut Scene, path: Vec<ravenrs_graphics::vector::PathSegment>, color: Color) {
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
    success: Color,
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
            success: theme.color(Semantic::Success),
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
    /// Where a node carried over the canvas would land, in units of the diagram.
    landing: Option<Rect>,
    grid: f32,
    /// The corner radius of a group's panel at 100%.
    radius: f32,
    palette: Palette,
    /// Where the lines worked out before are kept.
    memory: Rc<Memory>,
    /// The window's cables and this canvas's group, when connections are cables.
    cables: Option<(crate::Cables, u64)>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Surface {
    /// The connections as cables: each one held at its ports and bend points, in units of
    /// the diagram so that panning does not shake them, drawn through the view. The one
    /// whose end is being moved hangs from the pointer.
    fn paint_cables(
        &self,
        frame: Rect,
        cables: &crate::Cables,
        group: u64,
        scene: &mut Scene,
        touching: &dyn Fn(&FlowEdge) -> bool,
    ) {
        use ravenrs_core::cable::{CableEnd, CableRelease, CableView};
        let p = &self.palette;
        let zoom = self.view.zoom.max(f32::EPSILON);
        let view = CableView {
            scale: zoom,
            offset: Point::new(frame.x() + self.view.offset.x, frame.y() + self.view.offset.y),
        };
        let mut known = self.memory.cable_edges.borrow_mut();
        let first = known.is_none();
        let seen = known.get_or_insert_with(Default::default);
        let mut now = std::collections::HashSet::new();
        // The caps go after every cable, so the cables are drawn together and the caps too.
        let mut caps_scene = Scene::new();
        for edge in &self.graph.edges {
            let id = cable_id(group, &edge.id);
            let Some(mut cable) = self.graph.edge_cable(edge, id) else { continue };
            if let Some(c) = &self.connecting
                && c.moving.as_deref() == Some(edge.id.as_str())
            {
                let hand = CableEnd::at(self.view.to_content(c.pointer));
                if c.from == edge.to {
                    cable.from = hand;
                } else {
                    cable.to = hand;
                }
            }
            let on = touching(edge);
            let own = edge.color.as_deref().and_then(geometry::parse_color);
            let (color, width) = if on { (p.accent, 2.0) } else { (own.unwrap_or(p.muted), 1.5) };
            cable.group = group;
            cable.release = CableRelease::Fall;
            cable.grow = !first && !seen.contains(&edge.id);
            cable.view = view;
            cable.look.color = color;
            // The same width on screen at any zoom, a little fuller than a line.
            cable.look.thickness = (width + 1.0) / zoom;
            cable.look.shine = 0.35;
            if edge.dashed || edge.animated {
                cable.look.dash = Some([7.0, 5.0]);
            }
            now.insert(edge.id.clone());
            scene.push_cable(cable);
            // The caps where the cable hangs now.
            let Some(shape) = cables.shape(id) else { continue };
            let points: Vec<Point> = shape.iter().map(|q| view.apply(*q)).collect();
            let back: Vec<Point> = points.iter().rev().copied().collect();
            let caps = [
                (edge.end, points.last().copied(), geometry::end_direction(&points)),
                (edge.start, points.first().copied(), geometry::end_direction(&back)),
            ];
            for (cap, tip, direction) in caps {
                let (Some(tip), Some(direction)) = (tip, direction) else { continue };
                let outline = geometry::cap_outline(cap, tip, direction, CAP);
                if cap.filled() {
                    fill(&mut caps_scene, outline, color);
                } else if !outline.is_empty() {
                    stroke(&mut caps_scene, &geometry::flatten(&outline), width, color);
                }
            }
        }
        scene.extend_from_slice(caps_scene.primitives());
        *seen = now;
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1"))]
impl Surface {
    /// Paints the canvas inside its frame. `BuildCx::clip` keeps the nodes, which are children,
    /// inside the canvas, but not what the canvas paints itself: a connection to a node out of
    /// view, a cable's sag or a line being drawn would cross whatever stands beside it.
    fn paint(&self, frame: Rect, time: f64, scene: &mut Scene) {
        scene.push_clip(frame);
        self.paint_inside(frame, time, scene);
        scene.pop_clip();
    }

    fn paint_inside(&self, frame: Rect, time: f64, scene: &mut Scene) {
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
        if let Some((cables, group)) = &self.cables {
            self.paint_cables(frame, cables, *group, scene, &touching);
        }
        for edge in self.graph.edges.iter().filter(|e| Some(e.id.as_str()) != moving && self.cables.is_none())
        {
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
            let own = edge.color.as_deref().and_then(geometry::parse_color);
            let (color, width) = if on { (p.accent, 2.0) } else { (own.unwrap_or(p.muted), 1.5) };
            if edge.animated {
                let phase = (time as f32 * DASH_SPEED).rem_euclid(12.0);
                for piece in geometry::dashes(&points, 6.0, 6.0, phase) {
                    stroke(scene, &piece, width, color);
                }
            } else if edge.dashed {
                for piece in geometry::dashes(&points, 7.0, 5.0, 0.0) {
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
            && !(self.cables.is_some() && connecting.moving.is_some())
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
        if let Some(r) = self.landing {
            let (a, b) =
                (self.view.to_screen(r.origin), self.view.to_screen(Point::new(r.max_x(), r.max_y())));
            let shown = Rect::new(frame.x() + a.x, frame.y() + a.y, b.x - a.x, b.y - a.y);
            scene.push_quad(Quad::new(shown, p.elevated.with_alpha(0.8)).radius(8.0 * self.view.zoom));
            let outline = util::dashed_outline(shown, 8.0 * self.view.zoom, 1.5, 6.0, 4.0);
            scene.push_path(PathFill::solid(outline, p.focus));
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
        let export_only = cx.use_state(|| false);
        let checking = cx.use_state(|| false);
        let debugging = cx.use_state(|| false);
        let run = cx.use_state(|| None::<FlowRun>);
        let breakpoints = cx.use_state(Vec::<String>::new);
        let export_format = cx.use_state(FlowExportFormat::default);
        let export_menu = cx.use_state(|| false);
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
            cables: self.cables.then(|| (cx.cables(), cx.widget_id().raw())),
            validate: self.validate.clone(),
            guard: self.guard.clone(),
            notice: notice.clone(),
            layout: self.layout,
            resizable: self.resizable,
            rotatable: self.rotatable,
            no_turn: labels.flow_no_turn.clone(),
            next: self.next.clone(),
            export: self.export.clone(),
            connector: self.connector.clone(),
            checks: self.checks.clone(),
            checking: checking.clone(),
            runs: self.runs.clone(),
            debugging: debugging.clone(),
            run: run.clone(),
            breakpoints: breakpoints.clone(),
            picked: cx.use_state(Vec::<String>::new),
            refused_choice: cx.use_state(|| false),
            export_only,
            export_format,
            export_menu,
            palette,
            quick_create: self.quick_create,
            editable_titles: self.editable_titles,
            editing: editing.clone(),
            draft: draft.clone(),
            grid: self.grid,
            snap: self.snap,
            range: self.range,
            refused: labels.flow_refused.clone(),
            group_title: labels.flow_group.clone(),
            open_models: self.open_end.clone(),
            opened: cx.use_state(|| None::<OpenEnd>),
            open_query: cx.use_state(String::new),
        };
        let graph = self.graph.get();
        let size = cx.scroll_viewport();
        let mut view = flow.view.get();
        // A view fitted to a size the canvas no longer has, and not moved since, fits again: the
        // first size can be one that another view left at this place in the tree, and a window
        // that changes size keeps the diagram whole.
        let fitted: State<Option<(Size, FlowViewport)>> = cx.use_state(|| None);
        if let (Some((at, fitted_view)), Some(size)) = (fitted.get(), size)
            && at != size
            && fitted_view == view
        {
            view.fit = true;
        }
        if view.fit
            && let Some(size) = size
        {
            // With the zoom controls showing, the diagram fits above them, not under them.
            let above = if self.controls {
                Size::new(size.width, (size.height - CONTROLS_HEIGHT - 2.0 * cx.theme().spacing.s4).max(1.0))
            } else {
                size
            };
            match graph.bounds() {
                Some(bounds) => {
                    view = FlowViewport::fitting(bounds, above, self.range.0, self.range.1.min(1.0));
                    flow.view.set(view);
                    fitted.set(Some((size, view)));
                }
                // Nothing to fit yet, as while the nodes are loading: it fits when they come.
                None => {
                    view = FlowViewport {
                        fit: true,
                        ..FlowViewport::at(Point::new(FIT_MARGIN, FIT_MARGIN), 1.0)
                    }
                }
            }
        }
        if let (Some(arrivals), Some(size)) = (&self.arrivals, size)
            && let Some(node) = arrivals.get()
        {
            arrivals.set(None);
            flow.drop_node(&node, Point::new(size.width / 2.0, size.height / 2.0));
        }
        let selection = flow.selection.get();
        let connecting_now = connecting.get();
        let editing_now = editing.get();
        if self.read_only {
            locked.set_if_changed(true);
        }
        let is_locked = locked.get() || self.read_only;

        // The check, kept up to date while its box is open.
        let issues: Option<Vec<FlowIssue>> =
            self.checks.as_ref().filter(|_| checking.get()).map(|checks| checks(&graph));
        // What the check and the run show on the nodes and lines, on a copy for drawing only:
        // the problem written on its node; where the tokens are, have been and went wrong.
        let run_now = run.get().filter(|_| debugging.get() && self.runs.is_some());
        let points = breakpoints.get();
        let real = graph.clone();
        let mut graph = graph;
        // A run told from outside: its statuses on the nodes and its counts on the lines.
        if let Some(run) = &self.execution {
            for node in &mut graph.nodes {
                if let Some(part) = run.nodes.get(&node.id) {
                    node.status = part.status.clone();
                }
            }
            for edge in &mut graph.edges {
                if let Some(n) = run.items(&edge.from) {
                    let count = if n == 1 {
                        labels.flow_item_one.clone()
                    } else {
                        labels.flow_items.replace("{count}", &n.to_string())
                    };
                    edge.label = Some(match edge.label.take() {
                        Some(words) => format!("{words} · {count}"),
                        None => count,
                    });
                }
            }
        }
        // Hidden nodes, what they hold and their lines are left out of the drawing.
        let hidden: Vec<String> = graph
            .nodes
            .iter()
            .filter(|n| n.hidden)
            .flat_map(|n| std::iter::once(n.id.clone()).chain(graph.descendants(&n.id)))
            .collect();
        if !hidden.is_empty() {
            graph.nodes.retain(|n| !hidden.contains(&n.id));
            graph.edges.retain(|e| !hidden.contains(&e.from.node) && !hidden.contains(&e.to.node));
        }
        for issue in issues.iter().flatten() {
            for id in &issue.nodes {
                if let Some(n) = graph.find_node_mut(id)
                    && n.status == FlowNodeStatus::Normal
                {
                    n.status = FlowNodeStatus::Warning(issue.text.clone());
                }
            }
        }
        if let Some(r) = &run_now {
            for node in &mut graph.nodes {
                if r.visited.contains(&node.id) {
                    node.status = FlowNodeStatus::Success;
                }
                if r.tokens.contains(&node.id) || r.waiting.iter().any(|(j, _)| *j == node.id) {
                    node.status = FlowNodeStatus::Running;
                }
            }
            if let Some(failure) = r.failure()
                && let Some(n) = failure.node().and_then(|id| graph.find_node_mut(id))
            {
                n.status = FlowNodeStatus::Error(run_words(failure, &real, &labels));
            }
            for edge in &mut graph.edges {
                if r.taken.contains(&edge.id) {
                    edge.animated = true;
                }
            }
        }
        // The ports each node has connected, and those of the selected connections, found once:
        // walking every connection for every node is millions of comparisons at a thousand.
        let mut connected: std::collections::HashMap<&str, Vec<String>> = std::collections::HashMap::new();
        let mut handles: std::collections::HashMap<&str, Vec<String>> = std::collections::HashMap::new();
        for edge in &graph.edges {
            let chosen = selection.has_edge(&edge.id);
            for end in [&edge.from, &edge.to] {
                connected.entry(end.node.as_str()).or_default().push(end.port.clone());
                if chosen {
                    handles.entry(end.node.as_str()).or_default().push(end.port.clone());
                }
            }
        }
        let mut children = Vec::new();
        for node in &graph.nodes {
            let mut look = self.kinds.iter().find(|(k, _)| *k == node.kind).map(|(_, look)| look.clone());
            if self.status_icons {
                look = look.map(|look| look.for_status(&node.status));
            }
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
                quiet_ports: self.quiet_ports,
                selected_many: selection.nodes.len() > 1,
                breakpoint: points.contains(&node.id),
                members: graph.children(&node.id).count(),
                editing: editing_now.as_deref() == Some(node.id.as_str()),
                handles: handles.get(node.id.as_str()).cloned().unwrap_or_default(),
                connecting: connecting_now.clone(),
                connected: connected.get(node.id.as_str()).cloned().unwrap_or_default(),
                flow: flow.clone(),
                palette,
            };
            children.push(cx.keyed_child(util::hash_text(&node.id), &part));
        }
        // The words on the lines, as text the screen shows and reads like any other.
        for edge in &graph.edges {
            let Some(label) = &edge.label else { continue };
            let Some(points) = flow.line(&graph, edge, &view) else { continue };
            let Some(middle) = geometry::midpoint(&points) else { continue };
            children.push(edge_label(cx, label, middle, &theme, palette));
        }
        // The bend points of the one line selected, and a `+` in the middle of each stretch.
        if let ([], [only]) = (selection.nodes.as_slice(), selection.edges.as_slice())
            && !is_locked
            && let Some(edge) = graph.find_edge(only)
            && let Some(controls) = graph.edge_controls(edge)
        {
            let name = edge.label.clone().unwrap_or_else(|| edge.id.clone());
            let count = edge.waypoints.len();
            for (i, pair) in controls.windows(2).enumerate() {
                let middle = Point::new((pair[0].x + pair[1].x) / 2.0, (pair[0].y + pair[1].y) / 2.0);
                let part = BendHandle {
                    at: view.to_screen(middle),
                    edge: edge.id.clone(),
                    index: i,
                    add: true,
                    name: String::new(),
                    flow: flow.clone(),
                    palette,
                };
                children.push(cx.keyed_child(util::hash_text(&format!("bend+{}-{i}", edge.id)), &part));
            }
            for (i, &(x, y)) in edge.waypoints.iter().enumerate() {
                let part = BendHandle {
                    at: view.to_screen(Point::new(x, y)),
                    edge: edge.id.clone(),
                    index: i,
                    add: false,
                    name: labels
                        .flow_bend
                        .replace("{0}", &(i + 1).to_string())
                        .replace("{1}", &count.to_string())
                        .replace("{2}", &name),
                    flow: flow.clone(),
                    palette,
                };
                children.push(cx.keyed_child(util::hash_text(&format!("bend-{}-{i}", edge.id)), &part));
            }
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
        // The search a connection let go in the open leads to, at where it was let go.
        if let Some(open) = flow.opened.get() {
            let choices = flow.open_choices();
            let mut rows: Vec<AnyView> = vec![Box::new(
                crate::TextField::new(flow.open_query.binding()).placeholder(labels.flow_open_search.clone()),
            )];
            if choices.is_empty() {
                rows.push(Box::new(
                    crate::Text::new(labels.flow_open_none.clone())
                        .style(ravenrs_style::TextStyle::Callout)
                        .foreground(Semantic::InkMuted),
                ));
            }
            for (words, model) in choices {
                let f = flow.clone();
                rows.push(Box::new(
                    crate::Button::new(words)
                        .style(ButtonStyle::Ghost)
                        .size(ButtonSize::Small)
                        .on_click(move || f.create_linked(&model)),
                ));
            }
            let close = flow.opened.clone();
            rows.push(Box::new(
                crate::Button::new(labels.flow_open_close.clone())
                    .style(ButtonStyle::Ghost)
                    .size(ButtonSize::Small)
                    .on_click(move || close.set(None)),
            ));
            let list = cx.child(&crate::VStack::new(rows).spacing(2.0).align(Align::Stretch));
            children.push(cx.container(
                LayoutStyle {
                    absolute: Some((open.at.x, open.at.y)),
                    width: Some(OPEN_WIDTH),
                    padding: ravenrs_core::Insets::uniform(theme.spacing.s2),
                    ..LayoutStyle::stack(Axis::Vertical)
                },
                &[list],
                Paint::Quad(crate::context::QuadPaint {
                    corner_radius: theme.radii.md,
                    border: Some(ravenrs_core::Border { width: 1.0, color: palette.border }),
                    shadow: Some(theme.elevation.raised),
                    ..crate::context::QuadPaint::new(palette.surface)
                }),
            ));
        }
        // Why the app refused the last change, by the pointer, until the next gesture.
        if let Some(reason) = notice.get() {
            let at = memory.pointer.get().unwrap_or(Point::new(FIT_MARGIN, FIT_MARGIN));
            children.push(refusal(cx, &reason, at, palette));
        }
        if let Some(issues) = &issues {
            let title = if issues.is_empty() {
                labels.flow_check_ok.clone()
            } else {
                labels.flow_check_found.replace("{0}", &issues.len().to_string())
            };
            let message: Vec<String> = issues.iter().map(|i| format!("• {}", i.text)).collect();
            let close = checking.clone();
            let alert = Alert::new(title, message.join("\n"))
                .tone(if issues.is_empty() { crate::Tone::Success } else { crate::Tone::Warning })
                .action(labels.flow_check_close.clone(), move || close.set(false));
            let shown = cx.child(&alert);
            children.push(cx.container(
                LayoutStyle {
                    absolute: Some((theme.spacing.s4, theme.spacing.s4)),
                    width: Some(CHECK_WIDTH),
                    ..LayoutStyle::stack(Axis::Vertical)
                },
                &[shown],
                Paint::None,
            ));
        }
        if debugging.get()
            && let (Some(rules), Some(size)) = (&self.runs, size)
        {
            children.push(debug_box(cx, &flow, &real, rules, run_now.as_ref(), size, &theme, &labels));
        }
        if let Some(size) = size {
            if self.controls {
                let hint = self.hint.as_deref();
                children.push(controls(
                    cx,
                    &flow,
                    view,
                    is_locked,
                    self.read_only,
                    hint,
                    size,
                    &theme,
                    palette,
                ));
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

        // A node carried here from a toolbox: where it would land, on the grid.
        let landing = cx.carrying().filter(|c| c.over && c.verdict == Some(Ok(()))).and_then(|c| {
            let node = c.payload.data::<FlowNode>()?;
            let at = flow.local(c.position);
            Some(flow.landing(node, view.to_content(at)))
        });
        let surface = Surface {
            graph: graph.clone(),
            view,
            selection,
            connecting: connecting_now,
            marquee: marquee.get(),
            guides: guides.get(),
            landing,
            grid: self.grid,
            radius: theme.radii.lg,
            palette,
            memory: memory.clone(),
            cables: flow.cables.clone(),
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
        // A strip measures as wide as its diagram, so a parent that does not stretch it still
        // gives it room; one that stretches it, or is narrower, still decides (#971).
        if self.width.is_none()
            && let Some(tall) = self.height
        {
            let wide = strip_width(&self.graph.get(), tall);
            // A row of pieces that wraps: it asks for `wide` but gives way to a narrower parent, as one
            // box of `wide` would not (its least width would be all of it).
            let piece = wide / STRIP_PIECES;
            let pieces: Vec<NodeId> = (0..STRIP_PIECES as usize)
                .map(|_| {
                    cx.container(
                        LayoutStyle { width: Some(piece), height: Some(0.0), ..LayoutStyle::default() },
                        &[],
                        Paint::None,
                    )
                })
                .collect();
            children.push(cx.container(
                LayoutStyle { wrap: true, ..LayoutStyle::stack(Axis::Horizontal) },
                &pieces,
                Paint::None,
            ));
        }
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
        let bent = flow.clone();
        cx.on_double_click(
            root,
            Rc::new(move || {
                if let Some(at) = bent.memory.pointer.get() {
                    bent.add_bend_at(at);
                }
            }),
        );
        let (checked, dropped, refused) =
            (flow.locked.clone(), flow.clone(), labels.flow_inspector_read_only.clone());
        cx.drop_target(
            root,
            move |payload| match () {
                _ if payload.kind() != FLOW_NODE_DRAG || payload.data::<FlowNode>().is_none() => {
                    Err(String::new())
                }
                _ if checked.get() => Err(refused.clone()),
                _ => Ok(()),
            },
            move |payload, at| {
                if let Some(node) = payload.data::<FlowNode>() {
                    dropped.drop_node(node, at);
                }
            },
        );
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
        if self.wheel_pans {
            let wheeled = flow.clone();
            cx.on_wheel(root, Rc::new(move |delta| wheeled.pan_by(delta.x, delta.y)));
        }
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
/// The width of a strip `tall` points high that shows `graph` at 100% at most, with the margin it
/// keeps around it; 240 with no node (#971).
/// How many pieces the width of a strip comes in: the least it asks is one of them (#971).
const STRIP_PIECES: f32 = 16.0;

fn strip_width(graph: &FlowGraph, tall: f32) -> f32 {
    const EMPTY: f32 = 240.0;
    let Some(bounds) = graph.bounds() else { return EMPTY };
    let margin = FIT_MARGIN.min(tall / 6.0);
    let zoom = ((tall - margin * 2.0).max(1.0) / bounds.height().max(1.0)).min(1.0);
    (bounds.width() * zoom + margin * 2.0).max(EMPTY)
}

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
    read_only: bool,
    hint: Option<&str>,
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
    // Read only offers no way out of the lock.
    let lock = (!read_only).then(|| {
        cx.child(
            &IconButton::new(if locked { IconName::Lock } else { IconName::LockOpen })
                .label(lock_name.clone())
                .size(ButtonSize::Small)
                .style(ButtonStyle::Ghost)
                .selected(locked)
                .on_click(move || d.set(!d.get())),
        )
    });
    let debug = flow.runs.is_some().then(|| {
        let open = flow.debugging.clone();
        let on = open.get();
        cx.child(
            &IconButton::new(IconName::Play)
                .label(labels.flow_debug.clone())
                .size(ButtonSize::Small)
                .style(ButtonStyle::Ghost)
                .selected(on)
                .on_click(move || open.set(!on)),
        )
    });
    let check = flow.checks.is_some().then(|| {
        let open = flow.checking.clone();
        let on = open.get();
        cx.child(
            &IconButton::new(IconName::CircleCheck)
                .label(labels.flow_check.clone())
                .size(ButtonSize::Small)
                .style(ButtonStyle::Ghost)
                .selected(on)
                .on_click(move || open.set(!on)),
        )
    });
    let export = flow.export.is_some().then(|| {
        let item = |name: &str, aside: Option<&str>, format: FlowExportFormat| {
            let f = flow.clone();
            let item = MenuItem::new(name.to_owned()).on_select(move || {
                f.export(format);
            });
            match aside {
                Some(aside) => item.shortcut(aside.to_owned()),
                None => item,
            }
        };
        let only = flow.export_only.clone();
        let mut menu = DropdownMenu::new(labels.flow_export.clone())
            .open(flow.export_menu.binding())
            .icon(IconName::Download)
            .icon_only()
            .size(ButtonSize::Small)
            .style(ButtonStyle::Ghost)
            .item(item(&labels.flow_export_png, Some("2×"), FlowExportFormat::Png))
            .item(item(&labels.flow_export_svg, None, FlowExportFormat::Svg))
            .item(item(&labels.flow_export_pdf, None, FlowExportFormat::Pdf));
        if !flow.selection.get().nodes.is_empty() {
            let on = only.get();
            menu = menu.separator().item(
                MenuItem::new(labels.flow_export_selection.clone())
                    .checked(on)
                    .on_select(move || only.set(!on)),
            );
        }
        cx.child(&menu)
    });
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
    // What a click does, after the zoom, in the muted words of the controls.
    let hint = hint.map(|hint| {
        let words = util::label(cx, hint, TextStyle::Callout, None, p.muted);
        cx.container(
            LayoutStyle {
                padding: ravenrs_core::Insets::symmetric(0.0, theme.spacing.s2),
                ..LayoutStyle::leaf()
            },
            &[words],
            Paint::None,
        )
    });
    let height = CONTROLS_HEIGHT;
    let node = cx.container(
        LayoutStyle {
            absolute: Some((theme.spacing.s4, size.height - theme.spacing.s4 - height)),
            height: Some(height),
            padding: ravenrs_core::Insets::uniform(theme.spacing.s1),
            spacing: 2.0,
            align: Align::Center,
            ..LayoutStyle::stack(Axis::Horizontal)
        },
        &[zoom_in, zoom_out, fit, arrange]
            .into_iter()
            .chain(lock)
            .chain(check)
            .chain(debug)
            .chain(export)
            .chain([percent])
            .chain(hint)
            .collect::<Vec<_>>(),
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
    /// Ports that nothing joins show only by the node under the pointer.
    quiet_ports: bool,
    /// More than one node is selected: the arrows stay away.
    selected_many: bool,
    /// A run stops when a token reaches it.
    breakpoint: bool,
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
            FlowNodeStatus::Warning(m) | FlowNodeStatus::Error(m) | FlowNodeStatus::Waiting(m) => {
                parts.push(m.clone());
            }
            FlowNodeStatus::Disabled => parts.push(labels.flow_disabled.clone()),
            FlowNodeStatus::Running => parts.push(labels.flow_running.clone()),
            FlowNodeStatus::Success => parts.push(labels.flow_done.clone()),
            FlowNodeStatus::Skipped => parts.push(labels.flow_skipped.clone()),
            FlowNodeStatus::Cancelled => parts.push(labels.flow_cancelled.clone()),
            FlowNodeStatus::Pinned => parts.push(labels.flow_pinned.clone()),
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
            // Quiet ports that nothing joins show only by the node under the pointer.
            if self.quiet_ports
                && state == PortState::Free
                && !cx.is_hover_within()
                && self.connecting.is_none()
            {
                continue;
            }
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
            // A port with a name shows it inside the node, beside it: the answers of a
            // question, the ways out of a choice. Too small to read, it is left out.
            if let Some(name) = &port.label
                && view.zoom >= PORT_LABEL_ZOOM
                && node.rotation == 0.0
                && !node.group
            {
                // As large as the node's own words at this zoom.
                let mut attrs = util::attrs(&theme, TextStyle::Caption, None);
                attrs.size *= view.zoom;
                attrs.line_height *= view.zoom;
                let size = cx.measure_text(name, &attrs);
                let local = Point::new((at.x - node.x) * view.zoom, (at.y - node.y) * view.zoom);
                let gap = 8.0 * view.zoom;
                // In a box, inside it by the port; at the tips of a diamond or an ellipse, whose
                // middle holds the title, outside, along the way out, as flowcharts label them.
                let boxed = !matches!(node.shape, FlowShape::Diamond | FlowShape::Ellipse);
                let (x, y) = match (boxed, local.x <= 0.5, local.x >= w - 0.5, local.y <= 0.5) {
                    (true, true, _, _) => (local.x + gap, local.y - size.height / 2.0),
                    (true, _, true, _) => (local.x - gap - size.width, local.y - size.height / 2.0),
                    (true, _, _, true) => (local.x - size.width / 2.0, local.y + gap),
                    (true, ..) => (local.x - size.width / 2.0, local.y - gap - size.height),
                    (false, true, _, _) => (local.x - gap - size.width, local.y - size.height - 2.0),
                    (false, _, true, _) => (local.x + gap, local.y - size.height - 2.0),
                    (false, _, _, true) => (local.x + gap, local.y - size.height - gap),
                    (false, ..) => (local.x + gap, local.y + 2.0),
                };
                let style = LayoutStyle {
                    absolute: Some((x, y)),
                    width: Some(size.width),
                    height: Some(size.height),
                    ..LayoutStyle::leaf()
                };
                let words = cx.text(name, &attrs, p.muted, style);
                cx.semantics(words, Semantics::hidden());
                ports.push(words);
            }
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
        if self.breakpoint {
            // A red dot on its corner, as code editors mark a line.
            let dot = cx.container(
                LayoutStyle {
                    absolute: Some((-6.0, -6.0)),
                    width: Some(12.0),
                    height: Some(12.0),
                    ..LayoutStyle::leaf()
                },
                &[],
                Paint::Quad(crate::context::QuadPaint {
                    corner_radius: 6.0,
                    border: Some(ravenrs_core::Border { width: 2.0, color: p.canvas }),
                    ..crate::context::QuadPaint::new(p.danger)
                }),
            );
            ports.push(dot);
        }
        let style = node.style.clone();
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
                    &style,
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
                &style,
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
        if !matches!(node.shape, FlowShape::Card | FlowShape::Rectangle | FlowShape::Note | FlowShape::Text)
            && !node.group
        {
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
        // Found by its key, which stays the same when the diagram moves or grows.
        cx.test_id(root, &format!("node:{}", node.id));
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
    style: &FlowStyle,
    selected: bool,
    hovered: bool,
) {
    let note = shape == FlowShape::Note;
    let words = shape == FlowShape::Text;
    let (edge, width) = match status {
        _ if selected => (p.accent, 2.0),
        FlowNodeStatus::Warning(_) | FlowNodeStatus::Waiting(_) => (p.warning, 2.0),
        FlowNodeStatus::Error(_) => (p.danger, 2.0),
        FlowNodeStatus::Running => (p.accent, 2.5),
        FlowNodeStatus::Success => (p.success, 2.0),
        FlowNodeStatus::Pinned => (p.accent, 2.0),
        _ if note => (style.stroke_color().unwrap_or(p.warning), 1.0),
        _ => (style.stroke_color().unwrap_or(p.border), 1.0),
    };
    // A width of the app's grows with the zoom, as the outline does.
    let width = match status {
        FlowNodeStatus::Normal | FlowNodeStatus::Disabled if !selected => {
            style.stroke_width.map_or(width, |w| (w * zoom).max(1.0))
        }
        _ => width,
    };
    let fill = match (style.fill_color(), note, hovered) {
        (Some(own), _, true) => own.lerp(p.control, 0.3),
        (Some(own), ..) => own,
        (None, true, _) => p.warning.with_alpha(0.18),
        (None, _, true) => p.control,
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
    // Words alone show their box only under the pointer or selected, for moving them.
    let mut ring: Vec<Point> = outline.to_vec();
    ring.extend(outline.first().copied());
    if !words {
        paint.scene.push_path(PathFill::solid(path, fill));
        stroke(paint.scene, &ring, width, edge);
    } else if selected || hovered {
        stroke(paint.scene, &ring, 1.0, if selected { p.accent } else { p.border });
    }
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
    let ink = if faded(status) { p.muted } else { style.text_color().unwrap_or(p.ink) };
    // A sign in the middle, as large as the outline allows.
    if let Some(marker) = style.marker.as_deref().filter(|m| !m.is_empty()) {
        let size = frame.width().min(frame.height()) * 0.5;
        let attrs = TextAttrs {
            size,
            line_height: size,
            ..util::attrs(theme, TextStyle::Body, Some(Weight::SEMIBOLD))
        };
        let mut layout = paint.text.layout(marker, &attrs);
        let wide = layout.measure(None).width;
        layout.paint(
            Point::new(frame.center().x - wide / 2.0, frame.center().y - size / 2.0),
            ink,
            paint.scene,
        );
    }
    // The title under the outline, in as much room as twice its width.
    if style.title_below {
        let attrs = scaled(TextStyle::Callout, Some(Weight::SEMIBOLD));
        let mut y = frame.max_y() + 4.0 * zoom;
        for (text, color) in std::iter::once((title, ink)).chain(subtitle.map(|s| (s, p.muted))) {
            let mut layout = paint.text.layout_ellipsized(text, &attrs, frame.width() * 2.5);
            let wide = layout.measure(None).width;
            layout.paint(Point::new(frame.center().x - wide / 2.0, y), color, paint.scene);
            y += attrs.line_height;
        }
        return;
    }
    if words {
        let mut layout = paint.text.layout(title, &scaled(TextStyle::Title, Some(Weight::SEMIBOLD)));
        layout.measure(Some(frame.width().max(0.0)));
        layout.paint(Point::new(frame.x(), frame.y()), ink, paint.scene);
        return;
    }
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
    style: &FlowStyle,
    selected: bool,
    hovered: bool,
    card_only: bool,
) {
    let radius = theme.radii.md * zoom;
    // Switched off, or left out of the run: dashed and grey.
    let disabled = faded(status);
    let (edge, edge_width) = match status {
        _ if selected => (p.accent, 2.0),
        FlowNodeStatus::Warning(_) | FlowNodeStatus::Waiting(_) => (p.warning, 2.0),
        FlowNodeStatus::Error(_) => (p.danger, 2.0),
        FlowNodeStatus::Running => (p.accent, 2.5),
        FlowNodeStatus::Success => (p.success, 2.0),
        FlowNodeStatus::Pinned => (p.accent, 2.0),
        _ => (
            style.stroke_color().unwrap_or(p.border),
            style.stroke_width.map_or(1.0, |w| (w * zoom).max(1.0)),
        ),
    };
    if disabled {
        let outline = util::dashed_outline(frame, radius, 1.0, 6.0, 4.0);
        paint.scene.push_path(PathFill::solid(outline, if selected { p.accent } else { p.border }));
    } else {
        let fill = match style.fill_color() {
            Some(own) if hovered => own.lerp(p.control, 0.3),
            Some(own) => own,
            None if hovered => p.control,
            None => p.elevated,
        };
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
    let ink = if disabled { p.muted } else { style.text_color().unwrap_or(p.ink) };
    let note = match status {
        FlowNodeStatus::Warning(m) | FlowNodeStatus::Waiting(m) => Some((m.as_str(), p.warning)),
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
        // Found by the node's key and the port's id, which do not move with the diagram.
        cx.test_id(node, &format!("port:{}:{}", self.end.node, self.end.port));
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

/// A bend point of the selected line, which moves when dragged and with the arrows, or the
/// `+` in the middle of a stretch (`add`), which becomes one when dragged.
struct BendHandle {
    /// Where it is, in the view.
    at: Point,
    edge: String,
    index: usize,
    add: bool,
    /// What screen readers call it.
    name: String,
    flow: Flow,
    palette: Palette,
}

impl View for BendHandle {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let (p, add, hovered, focused) = (self.palette, self.add, cx.is_hovered(), cx.is_focused());
        let side = if add { BEND + 2.0 } else { BEND };
        let paint = Paint::Custom(Rc::new(move |frame: Rect, scene: &mut Scene| {
            if add {
                let fill = if hovered { p.accent } else { p.elevated };
                let ink = if hovered { Color::WHITE } else { p.focus };
                scene.push_quad(Quad::new(frame, fill).radius(side / 2.0).border(1.5, p.focus));
                let c = frame.center();
                stroke(scene, &[Point::new(c.x - 3.0, c.y), Point::new(c.x + 3.0, c.y)], 1.5, ink);
                stroke(scene, &[Point::new(c.x, c.y - 3.0), Point::new(c.x, c.y + 3.0)], 1.5, ink);
            } else {
                if focused {
                    let ring = Rect::new(
                        frame.x() - 4.0,
                        frame.y() - 4.0,
                        frame.width() + 8.0,
                        frame.height() + 8.0,
                    );
                    scene.push_quad(Quad::new(ring, Color::TRANSPARENT).radius(5.0).border(2.0, p.focus));
                }
                scene.push_quad(Quad::new(frame, p.accent).radius(2.0).border(1.5, Color::WHITE));
            }
        }));
        let node = cx.container(
            LayoutStyle {
                absolute: Some((self.at.x - side / 2.0, self.at.y - side / 2.0)),
                width: Some(side),
                height: Some(side),
                ..LayoutStyle::leaf()
            },
            &[],
            paint,
        );
        cx.cursor(node, CursorIcon::Pointer);
        let (dragged, edge, index) = (self.flow.clone(), self.edge.clone(), self.index);
        cx.on_drag(node, Rc::new(move |d| dragged.on_bend_drag(&edge, index, add, d.phase, d.position)));
        if add {
            // A double click on the line and the keys do the same, so screen readers skip it.
            cx.semantics(node, Semantics::hidden());
            // It sits on the line, so a double click there still makes a point.
            let (doubled, at) = (self.flow.clone(), self.at);
            cx.on_double_click(
                node,
                Rc::new(move || {
                    doubled.add_bend_at(at);
                }),
            );
        } else {
            cx.focusable(node, 2.0);
            cx.semantics(node, Semantics::new(Role::Button).name(self.name.clone()));
            let (keyed, edge) = (self.flow.clone(), self.edge.clone());
            cx.on_key(node, Rc::new(move |key, m| keyed.bend_key(&edge, index, key, m)));
        }
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

/// Whether a node is drawn grey: switched off, or left out of a run (it did not run, or it was
/// stopped).
fn faded(status: &FlowNodeStatus) -> bool {
    matches!(status, FlowNodeStatus::Disabled | FlowNodeStatus::Skipped | FlowNodeStatus::Cancelled)
}

/// How wide the search a connection let go in the open leads to is.
const OPEN_WIDTH: f32 = 240.0;

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
                    FlowNodeStatus::Warning(_) | FlowNodeStatus::Waiting(_) => p.warning,
                    FlowNodeStatus::Error(_) => p.danger,
                    FlowNodeStatus::Running | FlowNodeStatus::Pinned => p.accent,
                    FlowNodeStatus::Success => p.success,
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
