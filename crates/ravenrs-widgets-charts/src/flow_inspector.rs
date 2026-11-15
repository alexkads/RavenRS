//! `FlowInspector`: the properties of what is selected in a `FlowCanvas`, beside it.
//!
//! It works on the same diagram, selection and history as the canvas, and asks the same
//! `on_change`, so a change typed here is undone, refused and announced as one made on the
//! canvas.

use std::fmt;
use std::rc::Rc;

use ravenrs_core::Role;
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Weight};

use crate::context::{BuildCx, NodeId, Paint, Semantics};
use crate::flow::{
    EdgeCap, EdgeShape, FlowChange, FlowEdge, FlowGraph, FlowGuard, FlowHistory, FlowNode, FlowNodeStatus,
    FlowSelection, FlowShape, FlowStyle,
};
use crate::{Checkbox, ColorPicker, FormField, NumberField, SegmentedControl, Select, TextField, View, util};
use ravenrs_core::Color;
use ravenrs_graphics::flow::{color_hex, parse_color};

const SHAPES: [FlowShape; 10] = [
    FlowShape::Card,
    FlowShape::Rectangle,
    FlowShape::RoundedRectangle,
    FlowShape::Ellipse,
    FlowShape::Diamond,
    FlowShape::Parallelogram,
    FlowShape::Document,
    FlowShape::Cylinder,
    FlowShape::Note,
    FlowShape::Text,
];
const LINES: [EdgeShape; 3] = [EdgeShape::Curve, EdgeShape::Step, EdgeShape::Straight];
const CAPS: [EdgeCap; 5] =
    [EdgeCap::None, EdgeCap::Arrow, EdgeCap::OpenArrow, EdgeCap::Diamond, EdgeCap::Circle];

/// The properties of what is selected in a [`FlowCanvas`](crate::FlowCanvas): title, line,
/// kind, shape, size, place and status of a node; label, shape, ends and motion of a
/// connection; for several nodes, what they have in common, a value that differs shown as
/// mixed and, when chosen, given to all of them.
///
/// Give it the canvas's diagram and selection, and, so that its changes are undone and refused
/// with the canvas's, the same [`FlowHistory`] and the same `on_change`. Text is kept when
/// Enter is pressed or the field is left; a choice or a number, as it is made.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{FlowCanvas, FlowGraph, FlowHistory, FlowInspector, FlowSelection, HStack};
///
/// let (graph, selection, history) =
///     (State::new(FlowGraph::new()), State::new(FlowSelection::default()), State::new(FlowHistory::new()));
/// let editor = HStack::new((
///     FlowCanvas::new(graph.binding()).selection(selection.binding()).history(history.binding()),
///     FlowInspector::new(graph.binding(), selection.binding()).history(history.binding()),
/// ));
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FlowInspector {
    graph: Binding<FlowGraph>,
    selection: Binding<FlowSelection>,
    history: Option<Binding<FlowHistory>>,
    guard: Option<FlowGuard>,
    kinds: Rc<Vec<(String, String)>>,
    read_only: bool,
    width: f32,
}

impl fmt::Debug for FlowInspector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlowInspector").field("kinds", &self.kinds).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FlowInspector {
    /// The properties of what `selection` holds in `graph`, 288 pixels wide.
    pub fn new(graph: Binding<FlowGraph>, selection: Binding<FlowSelection>) -> Self {
        Self {
            graph,
            selection,
            history: None,
            guard: None,
            kinds: Rc::new(Vec::new()),
            read_only: false,
            width: 288.0,
        }
    }

    /// Keeps its changes in this history, the canvas's, so Cmd/Ctrl+Z there undoes them.
    pub fn history(mut self, history: Binding<FlowHistory>) -> Self {
        self.history = Some(history);
        self
    }

    /// Asks this before every change, as [`FlowCanvas::on_change`](crate::FlowCanvas::on_change)
    /// does; a refusal leaves the diagram as it was and shows why under the fields.
    pub fn on_change(
        mut self,
        guard: impl Fn(&FlowGraph, &FlowGraph, &FlowChange) -> Result<(), String> + 'static,
    ) -> Self {
        self.guard = Some(Rc::new(guard));
        self
    }

    /// A kind a node may have, by its name in the diagram and the words shown for it. Without
    /// any, the kind is not offered.
    pub fn kind(mut self, kind: impl Into<String>, caption: impl Into<String>) -> Self {
        Rc::make_mut(&mut self.kinds).push((kind.into(), caption.into()));
        self
    }

    /// The kinds of `notation`, with their words.
    pub fn notation(mut self, notation: &crate::FlowNotation) -> Self {
        for (kind, caption) in &notation.kinds {
            Rc::make_mut(&mut self.kinds).push((kind.clone(), caption.clone()));
        }
        self
    }

    /// Shows the properties without letting them change, as when the canvas is locked.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Another width, in logical pixels.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

/// How a change typed in the panel reaches the diagram.
#[derive(Clone)]
struct Writer {
    graph: Binding<FlowGraph>,
    history: Option<Binding<FlowHistory>>,
    guard: Option<FlowGuard>,
    notice: State<Option<String>>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Writer {
    /// Writes `change` made by `edit`, when the app lets it, remembering the diagram before.
    fn write(&self, nodes: &[String], edges: &[String], edit: impl FnOnce(&mut FlowGraph)) {
        let before = self.graph.get();
        let mut next = before.clone();
        edit(&mut next);
        if next == before {
            return;
        }
        let change = FlowChange::Edit { nodes: nodes.to_vec(), edges: edges.to_vec() };
        if let Some(guard) = &self.guard
            && let Err(why) = guard(&before, &next, &change)
        {
            self.notice.set(Some(why));
            return;
        }
        self.notice.set_if_changed(None);
        if let Some(history) = &self.history {
            history.update(|h| h.record(before));
        }
        self.graph.set(next);
    }

    /// A field bound to one value of every node in `ids`: read from the first, written to all.
    fn nodes<T: Clone + PartialEq + Default + 'static>(
        &self,
        ids: Vec<String>,
        read: impl Fn(&FlowNode) -> T + 'static,
        write: impl Fn(&mut FlowNode, T) + 'static,
    ) -> Binding<T> {
        let (reader, writer, read_ids) = (self.graph.clone(), self.clone(), ids.clone());
        Binding::new(
            move || {
                let g = reader.get();
                read_ids.first().and_then(|id| g.find_node(id)).map(&read).unwrap_or_default()
            },
            move |value: T| {
                writer.write(&ids, &[], |g| {
                    for node in g.nodes.iter_mut().filter(|n| ids.contains(&n.id)) {
                        write(node, value.clone());
                    }
                });
            },
        )
    }

    /// A field bound to one value of the connection `id`.
    fn edge<T: Clone + Default + 'static>(
        &self,
        id: String,
        read: impl Fn(&FlowEdge) -> T + 'static,
        write: impl Fn(&mut FlowEdge, T) + 'static,
    ) -> Binding<T> {
        let (reader, writer, read_id) = (self.graph.clone(), self.clone(), id.clone());
        Binding::new(
            move || reader.get().find_edge(&read_id).map(&read).unwrap_or_default(),
            move |value: T| {
                writer.write(&[], std::slice::from_ref(&id), |g| {
                    if let Some(edge) = g.edges.iter_mut().find(|e| e.id == id) {
                        write(edge, value);
                    }
                });
            },
        )
    }
}

/// A text field that keeps what is typed until Enter or leaving, then writes it.
fn text_field(cx: &mut BuildCx<'_>, label: &str, value: Binding<String>, read_only: bool) -> NodeId {
    let draft = cx.use_state(|| value.get());
    let current = value.get();
    // What the diagram holds wins when it changed elsewhere (undo, the canvas).
    let seen = cx.use_state(|| current.clone());
    if seen.get() != current {
        seen.set(current.clone());
        draft.set(current);
    }
    let (on_enter, on_leave) = ((value.clone(), draft.clone()), (value, draft.clone()));
    let field = TextField::new(draft.binding())
        .read_only(read_only)
        .on_submit(move |text| on_enter.0.set(text))
        .on_leave(move || on_leave.0.set(on_leave.1.get()));
    cx.child(&FormField::new(label, field))
}

impl View for FlowInspector {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let notice = cx.use_state(|| None::<String>);
        let writer = Writer {
            graph: self.graph.clone(),
            history: self.history.clone(),
            guard: self.guard.clone(),
            notice: notice.clone(),
        };
        let graph = self.graph.get();
        let selection = self.selection.get();
        let nodes: Vec<String> =
            selection.nodes.iter().filter(|id| graph.find_node(id).is_some()).cloned().collect();
        let edges: Vec<String> =
            selection.edges.iter().filter(|id| graph.find_edge(id).is_some()).cloned().collect();
        let ro = self.read_only;
        let muted = theme.color(Semantic::InkMuted);
        let mut rows: Vec<NodeId> = Vec::new();
        let heading = |cx: &mut BuildCx<'_>, words: &str, aside: Option<&str>| {
            let title = util::label(cx, words, TextStyle::Caption, Some(Weight::SEMIBOLD), muted);
            let mut parts = vec![title];
            if let Some(aside) = aside {
                parts.push(util::label(cx, aside, TextStyle::Caption, None, muted));
            }
            cx.container(
                LayoutStyle {
                    justify: ravenrs_layout::Justify::SpaceBetween,
                    align: Align::Center,
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &parts,
                Paint::None,
            )
        };

        if nodes.is_empty() && edges.is_empty() {
            rows.push(util::label(cx, &labels.flow_inspector_empty, TextStyle::Callout, None, muted));
        } else if let ([only], []) = (nodes.as_slice(), edges.as_slice()) {
            let node = graph.find_node(only).cloned().unwrap_or_default();
            rows.push(heading(cx, &labels.flow_inspector_node, Some(&node.id)));
            let ids = vec![only.clone()];
            let title = writer.nodes(
                ids.clone(),
                |n| n.title.clone(),
                |n, t: String| {
                    if !t.trim().is_empty() {
                        n.title = t.trim().to_owned();
                    }
                },
            );
            rows.push(cx.keyed_child(
                util::hash_text(&format!("title{only}")),
                &FieldView { label: labels.flow_field_title.clone(), value: title, read_only: ro },
            ));
            let line = writer.nodes(
                ids.clone(),
                |n| n.subtitle.clone().unwrap_or_default(),
                |n, t: String| {
                    n.subtitle = (!t.trim().is_empty()).then(|| t.trim().to_owned());
                },
            );
            rows.push(cx.keyed_child(
                util::hash_text(&format!("line{only}")),
                &FieldView { label: labels.flow_field_line.clone(), value: line, read_only: ro },
            ));
            rows.extend(self.kind_and_shape(cx, &writer, ids.clone(), &graph, &labels, ro));
            let number = |cx: &mut BuildCx<'_>,
                          label: &str,
                          read: fn(&FlowNode) -> f32,
                          write: fn(&mut FlowNode, f32)| {
                let value = writer.nodes(
                    ids.clone(),
                    move |n| Some(f64::from(read(n))),
                    move |n, v: Option<f64>| {
                        if let Some(v) = v {
                            write(n, v as f32);
                        }
                    },
                );
                cx.child(&NumberField::new(value).label(label).disabled(ro))
            };
            let w = number(cx, &labels.flow_field_width, |n| n.width, |n, v| n.width = v.max(1.0));
            let h = number(cx, &labels.flow_field_height, |n| n.height, |n, v| n.height = v.max(1.0));
            let x = number(cx, &labels.flow_field_x, |n| n.x, |n, v| n.x = v);
            let y = number(cx, &labels.flow_field_y, |n| n.y, |n, v| n.y = v);
            rows.push(grid(cx, &[w, h, x, y], theme.spacing.s3));
            let status_names = labels.flow_status_names.clone();
            if ro {
                let index = status_index(&node.status);
                rows.push(util::label(cx, &status_names[index], TextStyle::Callout, None, muted));
            } else {
                let status = writer.nodes(
                    ids.clone(),
                    |n| status_index(&n.status),
                    |n, i: usize| {
                        n.status = match i {
                            1 => FlowNodeStatus::Warning(status_words(&n.status)),
                            2 => FlowNodeStatus::Error(status_words(&n.status)),
                            3 => FlowNodeStatus::Disabled,
                            _ => FlowNodeStatus::Normal,
                        };
                    },
                );
                rows.push(cx.child(
                    &SegmentedControl::new(status, status_names).label(labels.flow_field_status.clone()),
                ));
            }
            rows.extend(self.appearance(cx, &writer, ids.clone(), &labels, ro));
        } else if let ([], [only]) = (nodes.as_slice(), edges.as_slice()) {
            rows.push(heading(cx, &labels.flow_inspector_edge, Some(only)));
            let label = writer.edge(
                only.clone(),
                |e| e.label.clone().unwrap_or_default(),
                |e, t: String| {
                    e.label = (!t.trim().is_empty()).then(|| t.trim().to_owned());
                },
            );
            rows.push(cx.keyed_child(
                util::hash_text(&format!("label{only}")),
                &FieldView { label: labels.flow_field_label.clone(), value: label, read_only: ro },
            ));
            let shape = writer.edge(
                only.clone(),
                |e| LINES.iter().position(|s| *s == e.shape).unwrap_or(0),
                |e, i: usize| e.shape = LINES[i.min(LINES.len() - 1)],
            );
            rows.push(
                cx.child(
                    &Select::new(shape, labels.flow_line_names.to_vec())
                        .label(labels.flow_field_shape.clone())
                        .disabled(ro),
                ),
            );
            let cap = |cx: &mut BuildCx<'_>, label: &str, start: bool| {
                let value = writer.edge(
                    only.clone(),
                    move |e| CAPS.iter().position(|c| *c == if start { e.start } else { e.end }).unwrap_or(0),
                    move |e, i: usize| {
                        let cap = CAPS[i.min(CAPS.len() - 1)];
                        if start { e.start = cap } else { e.end = cap }
                    },
                );
                cx.child(&Select::new(value, labels.flow_cap_names.to_vec()).label(label).disabled(ro))
            };
            let (start, end) =
                (cap(cx, &labels.flow_field_start, true), cap(cx, &labels.flow_field_end, false));
            rows.push(grid(cx, &[start, end], theme.spacing.s3));
            let animated = writer.edge(only.clone(), |e| e.animated, |e, on: bool| e.animated = on);
            rows.push(cx.child(&Checkbox::new(labels.flow_field_animated.clone(), animated).disabled(ro)));
            let dashed = writer.edge(only.clone(), |e| e.dashed, |e, on: bool| e.dashed = on);
            rows.push(cx.child(&Checkbox::new(labels.flow_field_dashed.clone(), dashed).disabled(ro)));
            let color = writer.edge(
                only.clone(),
                |e| e.color.as_deref().and_then(parse_color),
                |e, c: Option<Color>| e.color = c.map(color_hex),
            );
            rows.push(
                cx.child(
                    &ColorPicker::new(color)
                        .label(labels.flow_field_stroke.clone())
                        .width(self.width - 32.0)
                        .disabled(ro),
                ),
            );
        } else {
            let count = labels.flow_inspector_nodes.replace("{0}", &nodes.len().to_string());
            rows.push(heading(cx, &count, None));
            rows.extend(self.kind_and_shape(cx, &writer, nodes.clone(), &graph, &labels, ro));
            rows.extend(self.appearance(cx, &writer, nodes.clone(), &labels, ro));
        }
        if ro {
            rows.push(util::label(cx, &labels.flow_inspector_read_only, TextStyle::Caption, None, muted));
        }
        if let Some(why) = notice.get() {
            let words = util::label(cx, &why, TextStyle::Callout, None, theme.color(Semantic::Danger));
            cx.semantics(words, Semantics::new(Role::Status).name(why));
            rows.push(words);
        }
        let root = cx.container(
            LayoutStyle {
                width: Some(self.width),
                padding: ravenrs_core::Insets::uniform(theme.spacing.s4),
                spacing: theme.spacing.s3,
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &rows,
            Paint::Quad(crate::context::QuadPaint::new(theme.color(Semantic::Surface))),
        );
        cx.semantics(root, Semantics::new(Role::Group).name(labels.flow_inspector.clone()));
        root
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1"))]
impl FlowInspector {
    /// The kind and the shape of the nodes `ids`, shown as mixed where they differ.
    /// The colors of the nodes in `ids` and their outline's width: each left empty is the
    /// theme's.
    fn appearance(
        &self,
        cx: &mut BuildCx<'_>,
        writer: &Writer,
        ids: Vec<String>,
        labels: &crate::Labels,
        ro: bool,
    ) -> Vec<NodeId> {
        let theme = *cx.theme();
        let half = ((self.width - 32.0 - theme.spacing.s3) / 2.0).floor();
        let color = |cx: &mut BuildCx<'_>,
                     label: &str,
                     read: fn(&FlowStyle) -> Option<Color>,
                     write: fn(&mut FlowStyle, Option<String>)| {
            let value = writer.nodes(
                ids.clone(),
                move |n| read(&n.style),
                move |n, c: Option<Color>| write(&mut n.style, c.map(color_hex)),
            );
            cx.child(&ColorPicker::new(value).label(label).width(half).disabled(ro))
        };
        let fill = color(cx, &labels.flow_field_fill, FlowStyle::fill_color, |s, c| s.fill = c);
        let stroke = color(cx, &labels.flow_field_stroke, FlowStyle::stroke_color, |s, c| s.stroke = c);
        let text = color(cx, &labels.flow_field_text, FlowStyle::text_color, |s, c| s.text = c);
        let width = writer.nodes(
            ids.clone(),
            |n| n.style.stroke_width.map(f64::from),
            |n, v: Option<f64>| n.style.stroke_width = v.map(|v| (v as f32).clamp(0.5, 12.0)),
        );
        let width =
            cx.child(&NumberField::new(width).label(labels.flow_field_stroke_width.as_str()).disabled(ro));
        vec![grid(cx, &[fill, stroke, text, width], theme.spacing.s3)]
    }

    fn kind_and_shape(
        &self,
        cx: &mut BuildCx<'_>,
        writer: &Writer,
        ids: Vec<String>,
        graph: &FlowGraph,
        labels: &crate::Labels,
        ro: bool,
    ) -> Vec<NodeId> {
        let chosen: Vec<&FlowNode> = ids.iter().filter_map(|id| graph.find_node(id)).collect();
        let mut out = Vec::new();
        // A first option "Mixed" stands for values that differ, until one is chosen.
        let mixed = |same: bool, names: Vec<String>| {
            if same {
                (0, names)
            } else {
                (1, std::iter::once(labels.flow_inspector_mixed.clone()).chain(names).collect())
            }
        };
        if !self.kinds.is_empty() {
            let kinds = self.kinds.clone();
            let same = chosen.windows(2).all(|w| w[0].kind == w[1].kind);
            let (offset, names) = mixed(same, kinds.iter().map(|(_, c)| c.clone()).collect());
            let (read_kinds, write_kinds) = (kinds.clone(), kinds);
            let value = writer.nodes(
                ids.clone(),
                move |n| {
                    if offset == 1 {
                        0
                    } else {
                        read_kinds.iter().position(|(k, _)| *k == n.kind).unwrap_or(0)
                    }
                },
                move |n, i: usize| {
                    if let Some((kind, _)) = i.checked_sub(offset).and_then(|i| write_kinds.get(i)) {
                        n.kind = kind.clone();
                    }
                },
            );
            out.push(cx.child(&Select::new(value, names).label(labels.flow_field_kind.clone()).disabled(ro)));
        }
        let same = chosen.windows(2).all(|w| w[0].shape == w[1].shape);
        let (offset, names) = mixed(same, labels.flow_shape_names.to_vec());
        let value = writer.nodes(
            ids,
            move |n| if offset == 1 { 0 } else { SHAPES.iter().position(|s| *s == n.shape).unwrap_or(0) },
            move |n, i: usize| {
                if let Some(shape) = i.checked_sub(offset).and_then(|i| SHAPES.get(i)) {
                    n.shape = *shape;
                }
            },
        );
        out.push(cx.child(&Select::new(value, names).label(labels.flow_field_shape.clone()).disabled(ro)));
        out
    }
}

/// A text field of the panel, a view of its own so that its draft lives with it.
struct FieldView {
    label: String,
    value: Binding<String>,
    read_only: bool,
}

impl View for FieldView {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        text_field(cx, &self.label, self.value.clone(), self.read_only)
    }
}

/// Two fields to a row.
fn grid(cx: &mut BuildCx<'_>, cells: &[NodeId], gap: f32) -> NodeId {
    let rows: Vec<NodeId> = cells
        .chunks(2)
        .map(|pair| {
            let cells: Vec<NodeId> = pair
                .iter()
                .map(|&c| {
                    cx.container(
                        LayoutStyle { grow: 1.0, ..LayoutStyle::stack(Axis::Vertical) },
                        &[c],
                        Paint::None,
                    )
                })
                .collect();
            cx.container(
                LayoutStyle { spacing: gap, ..LayoutStyle::stack(Axis::Horizontal) },
                &cells,
                Paint::None,
            )
        })
        .collect();
    cx.container(
        LayoutStyle { spacing: gap, align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
        &rows,
        Paint::None,
    )
}

fn status_index(status: &FlowNodeStatus) -> usize {
    match status {
        FlowNodeStatus::Warning(_) => 1,
        FlowNodeStatus::Error(_) => 2,
        FlowNodeStatus::Disabled => 3,
        _ => 0,
    }
}

/// The words a status keeps when it changes between warning and error.
fn status_words(status: &FlowNodeStatus) -> String {
    match status {
        FlowNodeStatus::Warning(w) | FlowNodeStatus::Error(w) => w.clone(),
        _ => String::new(),
    }
}
