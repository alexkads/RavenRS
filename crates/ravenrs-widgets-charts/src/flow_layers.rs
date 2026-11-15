//! `FlowLayers`: the nodes of a diagram in the order they are drawn, to pick, hide and put in
//! front of or behind one another.

use std::fmt;

use ravenrs_core::{Insets, Role};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::Binding;
use ravenrs_style::{Semantic, TextStyle, Weight};

use crate::context::{BuildCx, NodeId, Paint, Semantics};
use crate::flow::{FlowGraph, FlowHistory, FlowSelection};
use crate::{ButtonSize, ButtonStyle, IconButton, IconName, ListItem, ScrollView, View, util};

/// The layers of a diagram: its nodes from the front one down, what a frame holds under it and
/// set in. A row selects its node on the canvas sharing the selection; its buttons hide and
/// show the node (with what it holds and its lines), and bring it forward or send it back,
/// each a change the shared history undoes.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{FlowCanvas, FlowGraph, FlowLayers, FlowSelection, HStack};
///
/// let (graph, selection) = (State::new(FlowGraph::new()), State::new(FlowSelection::default()));
/// let board = HStack::new((
///     FlowLayers::new(graph.binding(), selection.binding()),
///     FlowCanvas::new(graph.binding()).selection(selection.binding()),
/// ));
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FlowLayers {
    graph: Binding<FlowGraph>,
    selection: Binding<FlowSelection>,
    history: Option<Binding<FlowHistory>>,
    width: f32,
}

impl fmt::Debug for FlowLayers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlowLayers").finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FlowLayers {
    /// The layers of `graph`, picking into `selection`, 240 pixels wide.
    pub fn new(graph: Binding<FlowGraph>, selection: Binding<FlowSelection>) -> Self {
        Self { graph, selection, history: None, width: 240.0 }
    }

    /// Keeps its changes in this history, the canvas's.
    pub fn history(mut self, history: Binding<FlowHistory>) -> Self {
        self.history = Some(history);
        self
    }

    /// Another width, in logical pixels.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

/// Writes a change to the diagram, remembering it.
fn write(
    graph: &Binding<FlowGraph>,
    history: &Option<Binding<FlowHistory>>,
    edit: impl FnOnce(&mut FlowGraph),
) {
    let before = graph.get();
    let mut next = before.clone();
    edit(&mut next);
    if next != before {
        if let Some(history) = history {
            history.update(|h| h.record(before));
        }
        graph.set(next);
    }
}

/// Moves a node one place in the drawing order among the nodes of its own frame: forward is
/// toward the front.
fn shift(graph: &mut FlowGraph, id: &str, forward: bool) {
    let Some(i) = graph.nodes.iter().position(|n| n.id == id) else { return };
    let parent = graph.nodes[i].parent.clone();
    let mates: Vec<usize> = (0..graph.nodes.len()).filter(|k| graph.nodes[*k].parent == parent).collect();
    let Some(at) = mates.iter().position(|k| *k == i) else { return };
    let other = if forward { mates.get(at + 1) } else { at.checked_sub(1).and_then(|a| mates.get(a)) };
    if let Some(&j) = other {
        graph.nodes.swap(i, j);
    }
}

impl View for FlowLayers {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let graph = self.graph.get();
        let selection = self.selection.get();
        let muted = theme.color(Semantic::InkMuted);
        let mut rows: Vec<NodeId> = Vec::new();
        rows.push(util::label(cx, &labels.flow_layers, TextStyle::Caption, Some(Weight::SEMIBOLD), muted));
        // From the front down, each frame followed by what it holds.
        let mut order: Vec<(usize, usize)> = Vec::new();
        fn walk(graph: &FlowGraph, parent: Option<&str>, depth: usize, order: &mut Vec<(usize, usize)>) {
            for (i, node) in graph.nodes.iter().enumerate().rev() {
                if node.parent.as_deref() == parent {
                    order.push((i, depth));
                    if node.group {
                        walk(graph, Some(&node.id), depth + 1, order);
                    }
                }
            }
        }
        walk(&graph, None, 0, &mut order);
        let mut list: Vec<NodeId> = Vec::new();
        for (i, depth) in order {
            let node = &graph.nodes[i];
            let name = if node.title.trim().is_empty() { node.id.clone() } else { node.title.clone() };
            let (pick, id) = (self.selection.clone(), node.id.clone());
            let item = ListItem::new(name.clone())
                .icon(if node.group { IconName::Layers } else { IconName::Square })
                .selected(selection.has_node(&node.id))
                .on_click(move || pick.set(FlowSelection::node(id.clone())));
            let row = cx.keyed_child(util::hash_text(&format!("layer-{}", node.id)), &item);
            let button = |cx: &mut BuildCx<'_>, icon: IconName, words: String, run: Box<dyn Fn()>| {
                cx.child(
                    &IconButton::new(icon)
                        .label(words)
                        .size(ButtonSize::Small)
                        .style(ButtonStyle::Ghost)
                        .on_click(run),
                )
            };
            let (g1, h1, id1, hidden) =
                (self.graph.clone(), self.history.clone(), node.id.clone(), node.hidden);
            let eye = button(
                cx,
                if hidden { IconName::EyeOff } else { IconName::Eye },
                if hidden {
                    labels.flow_layer_show.replace("{0}", &name)
                } else {
                    labels.flow_layer_hide.replace("{0}", &name)
                },
                Box::new(move || {
                    write(&g1, &h1, |g| {
                        if let Some(n) = g.find_node_mut(&id1) {
                            n.hidden = !n.hidden;
                        }
                    });
                }),
            );
            let (g2, h2, id2) = (self.graph.clone(), self.history.clone(), node.id.clone());
            let up = button(
                cx,
                IconName::ChevronUp,
                labels.flow_layer_up.replace("{0}", &name),
                Box::new(move || write(&g2, &h2, |g| shift(g, &id2, true))),
            );
            let (g3, h3, id3) = (self.graph.clone(), self.history.clone(), node.id.clone());
            let down = button(
                cx,
                IconName::ChevronDown,
                labels.flow_layer_down.replace("{0}", &name),
                Box::new(move || write(&g3, &h3, |g| shift(g, &id3, false))),
            );
            let grow = cx.container(
                LayoutStyle { grow: 1.0, ..LayoutStyle::stack(Axis::Vertical) },
                &[row],
                Paint::None,
            );
            list.push(cx.container(
                LayoutStyle {
                    align: Align::Center,
                    spacing: 2.0,
                    padding: Insets { left: 16.0 * depth as f32, ..Insets::default() },
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &[grow, eye, up, down],
                Paint::None,
            ));
        }
        let list = cx.container(
            LayoutStyle { spacing: 2.0, align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &list,
            Paint::None,
        );
        let scroll = cx.child(&ScrollView::new((Raw(list),)).spacing(0.0));
        rows.push(scroll);
        let root = cx.container(
            LayoutStyle {
                width: Some(self.width),
                padding: Insets::uniform(theme.spacing.s4),
                spacing: theme.spacing.s2,
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &rows,
            Paint::Quad(crate::context::QuadPaint::new(theme.color(Semantic::Surface))),
        );
        cx.semantics(root, Semantics::new(Role::Group).name(labels.flow_layers.clone()));
        root
    }
}

/// A node already built, as a view.
struct Raw(NodeId);

impl View for Raw {
    fn build(&self, _cx: &mut BuildCx<'_>) -> NodeId {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::shift;
    use crate::flow::{FlowGraph, FlowNode};

    #[test]
    fn a_layer_moves_among_the_nodes_of_its_own_frame() {
        let mut g = FlowGraph::new()
            .node(FlowNode::new("a", "", "A", 0.0, 0.0))
            .node(FlowNode::new("f", "", "F", 0.0, 0.0))
            .node(FlowNode::new("in", "", "In", 0.0, 0.0).parent("f"))
            .node(FlowNode::new("b", "", "B", 0.0, 0.0));
        shift(&mut g, "a", true);
        let ids: Vec<&str> = g.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["f", "a", "in", "b"]);
        shift(&mut g, "in", true);
        assert_eq!(g.nodes[2].id, "in", "alone in its frame, it stays");
        shift(&mut g, "b", false);
        let ids: Vec<&str> = g.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["f", "b", "in", "a"]);
    }
}
