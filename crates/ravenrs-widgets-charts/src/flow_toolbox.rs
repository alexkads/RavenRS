//! `FlowToolbox`: the models of node a `FlowCanvas` can be given, to drag onto it.
//!
//! Each model is a source of the drag between components, carrying a `FlowNode`; the canvas
//! is a target for it and makes a copy where it is dropped. With the keyboard, Enter leaves the
//! model in a binding the canvas shares ([`FlowCanvas::arrivals`](crate::FlowCanvas::arrivals)),
//! and the canvas puts the copy in the middle of its view; Space picks it up to carry with the
//! arrows, as any other source.

use std::fmt;
use std::rc::Rc;

use ravenrs_core::{CursorIcon, Insets, Key, Quad, Rect, Role, Scene};
use ravenrs_graphics::flow as geometry;
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::Binding;
use ravenrs_style::{Semantic, TextStyle, Weight};

use crate::context::{BuildCx, DragPayload, NodeId, Paint, Semantics};
use crate::flow::{FLOW_NODE_DRAG, FlowNode, fill};
use crate::{Disclosure, ScrollView, Text, TextField, View, util};

/// One model: the words on its tile and the node a copy is made of.
#[derive(Clone, Debug)]
struct Model {
    caption: String,
    node: FlowNode,
}

/// A group of models under a title that opens and closes.
#[derive(Clone, Debug)]
struct Group {
    title: String,
    models: Vec<Model>,
}

/// The models of node for a [`FlowCanvas`](crate::FlowCanvas), in groups that open and close,
/// with a field that keeps only those whose words match. Each tile shows the model's shape and
/// its words.
///
/// Drag a tile onto the canvas and a copy of its node lands where it is dropped, on the grid,
/// with an id of its own, selected, as one change the canvas's history undoes. With the keys,
/// Enter on a tile puts the copy in the middle of the canvas's view, when both are given the
/// same binding ([`Self::arrivals`] and [`FlowCanvas::arrivals`](crate::FlowCanvas::arrivals)).
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{FlowCanvas, FlowGraph, FlowNode, FlowShape, FlowToolbox, HStack};
///
/// let (graph, arrivals) = (State::new(FlowGraph::new()), State::new(None::<FlowNode>));
/// let editor = HStack::new((
///     FlowToolbox::new()
///         .group("Flow")
///         .model("Step", FlowNode::new("step", "step", "Step", 0.0, 0.0))
///         .model("Choice", FlowNode::new("choice", "choice", "Choice", 0.0, 0.0).shape(FlowShape::Diamond))
///         .arrivals(arrivals.binding()),
///     FlowCanvas::new(graph.binding()).arrivals(arrivals.binding()),
/// ));
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FlowToolbox {
    groups: Vec<Group>,
    arrivals: Option<Binding<Option<FlowNode>>>,
    title: Option<String>,
    searchable: bool,
    width: f32,
}

impl fmt::Debug for FlowToolbox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlowToolbox").field("groups", &self.groups).finish_non_exhaustive()
    }
}

impl Default for FlowToolbox {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FlowToolbox {
    /// No models yet, with the search field, 240 pixels wide.
    pub fn new() -> Self {
        Self { groups: Vec::new(), arrivals: None, title: None, searchable: true, width: 240.0 }
    }

    /// Starts a group under this title; the models after it go in it.
    pub fn group(mut self, title: impl Into<String>) -> Self {
        self.groups.push(Group { title: title.into(), models: Vec::new() });
        self
    }

    /// A model called `caption` whose copies are `node`: its kind, shape, size, title and
    /// ports. Its place and id are the canvas's to choose. Before any group, one without a
    /// title is started.
    pub fn model(mut self, caption: impl Into<String>, node: FlowNode) -> Self {
        if self.groups.is_empty() {
            self.groups.push(Group { title: String::new(), models: Vec::new() });
        }
        if let Some(group) = self.groups.last_mut() {
            group.models.push(Model { caption: caption.into(), node });
        }
        self
    }

    /// Leaves the model picked with Enter here, for the canvas given the same binding.
    pub fn arrivals(mut self, arrivals: Binding<Option<FlowNode>>) -> Self {
        self.arrivals = Some(arrivals);
        self
    }

    /// The elements of `notation`, in its groups, in the order they were given.
    pub fn notation(mut self, notation: &crate::FlowNotation) -> Self {
        for (group, caption, node) in &notation.models {
            if self.groups.last().is_none_or(|g| g.title != *group) {
                self = self.group(group.clone());
            }
            self = self.model(caption.clone(), node.clone());
        }
        self
    }

    /// Another heading than "Shapes", for what the models are in this app: blocks, steps,
    /// pieces. Screen readers name the panel by it too.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Without the search field, for a few models.
    pub fn searchable(mut self, searchable: bool) -> Self {
        self.searchable = searchable;
        self
    }

    /// Another width, in logical pixels.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

/// Whether every word typed is in the caption or the group's title, ignoring case.
fn matches(query: &str, caption: &str, group: &str) -> bool {
    let words = format!("{} {}", caption.to_lowercase(), group.to_lowercase());
    query.split_whitespace().all(|w| words.contains(&w.to_lowercase()))
}

impl View for FlowToolbox {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let query = cx.use_state(String::new);
        let muted = theme.color(Semantic::InkMuted);
        let title = self.title.clone().unwrap_or_else(|| labels.flow_toolbox.clone());
        let mut rows: Vec<NodeId> = Vec::new();
        rows.push(util::label(cx, &title, TextStyle::Caption, Some(Weight::SEMIBOLD), muted));
        if self.searchable {
            rows.push(
                cx.child(&TextField::new(query.binding()).placeholder(labels.flow_toolbox_search.clone())),
            );
        }
        // The groups scroll under the heading and the field when they do not fit.
        let inner = self.width - theme.spacing.s4 * 2.0;
        let groups = ModelGroups {
            groups: self.groups.clone(),
            arrivals: self.arrivals.clone(),
            typed: query.get(),
            inner,
        };
        rows.push(cx.child(&ScrollView::new((groups,)).spacing(0.0)));
        let root = cx.container(
            LayoutStyle {
                width: Some(self.width),
                padding: Insets::uniform(theme.spacing.s4),
                spacing: theme.spacing.s3,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &rows,
            Paint::Quad(crate::context::QuadPaint::new(theme.color(Semantic::Surface))),
        );
        cx.semantics(root, Semantics::new(Role::Group).name(title));
        root
    }
}

/// The groups whose models match what was typed, each opening and closing.
struct ModelGroups {
    groups: Vec<Group>,
    arrivals: Option<Binding<Option<FlowNode>>>,
    typed: String,
    /// The width inside the panel's padding.
    inner: f32,
}

impl View for ModelGroups {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let muted = theme.color(Semantic::InkMuted);
        let mut rows: Vec<NodeId> = Vec::new();
        let mut shown = 0;
        for (g, group) in self.groups.iter().enumerate() {
            let models: Vec<Model> = group
                .models
                .iter()
                .filter(|m| matches(&self.typed, &m.caption, &group.title))
                .cloned()
                .collect();
            if models.is_empty() {
                continue;
            }
            shown += models.len();
            // A group's tiles sit inside the disclosure's indent; a group without a title has none.
            let room = if group.title.is_empty() { self.inner } else { self.inner - DISCLOSURE_INDENT };
            let width = ((room - theme.spacing.s2) / 2.0).floor();
            let tiles = ModelTiles {
                models,
                arrivals: self.arrivals.clone(),
                hint: labels.flow_toolbox_hint.clone(),
                width,
            };
            let key = util::hash_text(&format!("flow-toolbox-{g}-{}", group.title));
            if group.title.is_empty() {
                rows.push(cx.keyed_child(key, &tiles));
            } else {
                // Searching opens every group, so what matches is never hidden.
                let open = !self.typed.trim().is_empty() || g == 0;
                rows.push(cx.keyed_child(key, &Disclosure::new(group.title.clone(), tiles).open(open)));
            }
        }
        if shown == 0 {
            rows.push(util::label(cx, &labels.flow_toolbox_none, TextStyle::Callout, None, muted));
        }
        cx.container(
            LayoutStyle {
                spacing: theme.spacing.s2,
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &rows,
            Paint::None,
        )
    }
}

/// How far a [`Disclosure`] sets its content in: 32 pixels on the left, 8 on the right.
const DISCLOSURE_INDENT: f32 = 40.0;

/// The tiles of one group, two to a row.
struct ModelTiles {
    models: Vec<Model>,
    arrivals: Option<Binding<Option<FlowNode>>>,
    hint: String,
    /// Each tile's width.
    width: f32,
}

impl View for ModelTiles {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let spacing = cx.theme().spacing.s2;
        let tiles: Vec<NodeId> = self
            .models
            .iter()
            .enumerate()
            .map(|(i, model)| {
                cx.keyed_child(
                    util::hash_text(&format!("{i}-{}", model.caption)),
                    &ModelTile {
                        model: model.clone(),
                        arrivals: self.arrivals.clone(),
                        hint: self.hint.clone(),
                        width: self.width,
                    },
                )
            })
            .collect();
        cx.container(
            LayoutStyle { wrap: true, spacing, ..LayoutStyle::stack(Axis::Horizontal) },
            &tiles,
            Paint::None,
        )
    }
}

/// One model: its shape drawn small over its words, a source of the drag.
struct ModelTile {
    model: Model,
    arrivals: Option<Binding<Option<FlowNode>>>,
    hint: String,
    width: f32,
}

impl View for ModelTile {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        const PREVIEW: (f32, f32) = (56.0, 32.0);
        let theme = *cx.theme();
        let (hovered, focused) = (cx.is_hovered(), cx.is_focused());
        let (accent, border) = (theme.color(Semantic::Accent), theme.color(Semantic::Border));
        let shape = self.model.node.shape;
        let preview = cx.container(
            LayoutStyle { width: Some(PREVIEW.0), height: Some(PREVIEW.1), ..LayoutStyle::leaf() },
            &[],
            Paint::Custom(Rc::new(move |frame: Rect, scene: &mut Scene| {
                // The shape is drawn as the canvas draws it, as a line one pixel wide.
                fill(scene, geometry::shape_outline(shape, frame), accent);
                let inner =
                    Rect::new(frame.x() + 1.5, frame.y() + 1.5, frame.width() - 3.0, frame.height() - 3.0);
                fill(
                    scene,
                    geometry::shape_outline(shape, inner),
                    theme.color(Semantic::SurfaceElevated).lerp(accent, 0.08),
                );
            })),
        );
        // A long name is cut with an ellipsis, so two tiles always fit a row; screen readers
        // still get it whole.
        let caption = cx.child(&Text::new(self.model.caption.clone()).style(TextStyle::Caption).truncate());
        let r = theme.radii.md;
        let tile = cx.container(
            LayoutStyle {
                width: Some(self.width),
                padding: Insets::uniform(theme.spacing.s2),
                spacing: theme.spacing.s1,
                overflow_hidden: true,
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[preview, caption],
            Paint::Custom(Rc::new(move |frame: Rect, scene: &mut Scene| {
                let edge = if hovered || focused { accent } else { border };
                scene.push_quad(
                    Quad::new(frame, theme.color(Semantic::SurfaceElevated)).radius(r).border(1.0, edge),
                );
            })),
        );
        let node = self.model.node.clone();
        cx.drag_source(tile, DragPayload::new(FLOW_NODE_DRAG, &self.model.caption, node.clone()));
        cx.focusable(tile, r);
        cx.cursor(tile, CursorIcon::Pointer);
        cx.semantics(
            tile,
            Semantics::new(Role::Button).name(self.model.caption.clone()).description(self.hint.clone()),
        );
        if let Some(arrivals) = self.arrivals.clone() {
            cx.on_key(
                tile,
                Rc::new(move |key, _| {
                    if *key != Key::Enter {
                        return false;
                    }
                    arrivals.set(Some(node.clone()));
                    true
                }),
            );
        }
        tile
    }
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn every_word_must_match() {
        assert!(matches("", "Step", "Flow"));
        assert!(matches("st", "Step", "Flow"));
        assert!(matches("flow step", "Step", "Flow"));
        assert!(!matches("step choice", "Step", "Flow"));
        assert!(matches("CHO", "Choice", ""));
    }
}
