//! A diagramming whiteboard, made only with `FlowCanvas`, `FlowToolbox`, `FlowInspector`
//! and a `SegmentedControl`: a flowchart with step lines and bend points, and a mind map
//! that arranges itself as a tree; both grow from the keyboard (Cmd/Ctrl+Enter) and export.
//!
//! It validates the component as a whiteboard of this kind: everything here is the library's,
//! and what it could not do is written in the README.

use std::rc::Rc;

use ravenrs::core::{MemoryStorage, Storage};
use ravenrs::prelude::*;

fn step(id: &str, title: &str, shape: FlowShape) -> FlowNode {
    let (w, h) = match shape {
        FlowShape::Diamond => (168.0, 104.0),
        FlowShape::Ellipse => (152.0, 64.0),
        _ => (176.0, 72.0),
    };
    FlowNode::new(id, "step", title, 0.0, 0.0)
        .shape(shape)
        .size(w, h)
        .port(FlowPort::input("in").side(PortSide::Top))
        .port(FlowPort::output("out").side(PortSide::Bottom))
        .port(FlowPort::output("side").side(PortSide::Right))
}

fn at(node: FlowNode, x: f32, y: f32) -> FlowNode {
    FlowNode { x, y, ..node }
}

fn line(id: &str, from: (&str, &str), to: &str) -> FlowEdge {
    FlowEdge::new(id, FlowEndpoint::new(from.0, from.1), FlowEndpoint::new(to, "in")).shape(EdgeShape::Step)
}

/// A flowchart of an order, with a loop back that goes around through bend points.
pub fn flowchart() -> FlowGraph {
    FlowGraph::new()
        .node(at(step("start", "Pedido chega", FlowShape::Ellipse), 192.0, 0.0))
        .node(at(step("check", "Confere estoque", FlowShape::Rectangle), 180.0, 120.0))
        .node(at(step("enough", "Tem tudo?", FlowShape::Diamond), 184.0, 240.0))
        .node(at(step("buy", "Compra do fornecedor", FlowShape::Rectangle), 456.0, 256.0))
        .node(at(step("invoice", "Emite a nota", FlowShape::Document), 180.0, 408.0))
        .node(at(step("save", "Grava no sistema", FlowShape::Cylinder), 180.0, 528.0))
        .node(at(
            FlowNode::new("note", "note", "O fornecedor leva até 2 dias", 456.0, 384.0)
                .shape(FlowShape::Note)
                .size(184.0, 80.0),
            456.0,
            384.0,
        ))
        .edge(line("a", ("start", "out"), "check"))
        .edge(line("b", ("check", "out"), "enough"))
        .edge(line("c", ("enough", "out"), "invoice").label("Sim"))
        .edge(line("d", ("enough", "side"), "buy").label("Não"))
        .edge(
            line("e", ("buy", "side"), "check")
                .waypoint(696.0, 292.0)
                .waypoint(696.0, 96.0)
                .waypoint(268.0, 96.0),
        )
        .edge(line("f", ("invoice", "out"), "save"))
}

fn idea(id: &str, title: &str) -> FlowNode {
    FlowNode::new(id, "idea", title, 0.0, 0.0)
        .shape(FlowShape::RoundedRectangle)
        .size(176.0, 48.0)
        .port(FlowPort::input("in").side(PortSide::Left))
        .port(FlowPort::output("out").side(PortSide::Right))
}

/// A mind map: an idea in the middle and what it leads to, arranged as a tree.
pub fn mind_map() -> FlowGraph {
    let ideas = [
        ("root", "Lançamento", None),
        ("site", "Site", Some("root")),
        ("store", "Loja", Some("root")),
        ("people", "Equipe", Some("root")),
        ("pages", "Páginas novas", Some("site")),
        ("seo", "Busca", Some("site")),
        ("stock", "Estoque inicial", Some("store")),
        ("training", "Treinamento", Some("people")),
    ];
    let mut graph = FlowGraph::new();
    for (i, (id, title, from)) in ideas.iter().enumerate() {
        graph = graph.node(idea(id, title));
        if let Some(from) = from {
            let edge =
                FlowEdge::new(format!("m{i}"), FlowEndpoint::new(*from, "out"), FlowEndpoint::new(*id, "in"))
                    .shape(EdgeShape::Curve);
            graph = graph.edge(FlowEdge { end: EdgeCap::None, ..edge });
        }
    }
    FlowLayout::tree(FlowDirection::Right).gaps(72.0, 24.0).apply(&mut graph);
    graph
}

/// The shapes to drag in.
pub fn toolbox() -> FlowToolbox {
    FlowToolbox::new()
        .group("Fluxograma")
        .model("Processo", step("process", "Processo", FlowShape::Rectangle))
        .model("Decisão", step("decision", "Decisão?", FlowShape::Diamond))
        .model("Início ou fim", step("terminal", "Início", FlowShape::Ellipse))
        .model("Documento", step("document", "Documento", FlowShape::Document))
        .model("Dados", step("data", "Dados", FlowShape::Cylinder))
        .model("Entrada", step("input", "Entrada", FlowShape::Parallelogram))
        .group("Ideias")
        .model("Ideia", idea("idea", "Ideia"))
        .model(
            "Nota",
            FlowNode::new("note", "note", "Nota", 0.0, 0.0).shape(FlowShape::Note).size(184.0, 80.0),
        )
}

/// The next shape made with the arrow or Cmd/Ctrl+Enter: a process in a flowchart, another
/// idea in a mind map.
pub fn next(from: &FlowNode, _side: PortSide) -> Option<FlowNode> {
    match from.kind.as_str() {
        "idea" => Some(idea("idea", "Ideia")),
        "note" => None,
        _ => Some(step("process", "Processo", FlowShape::Rectangle)),
    }
}

/// The whiteboard: which board, its shapes, the board and the selected shape's properties.
pub struct Whiteboard {
    /// Where the diagrams are kept between runs.
    storage: Rc<dyn Storage>,
}

impl std::fmt::Debug for Whiteboard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Whiteboard").finish_non_exhaustive()
    }
}

impl View for Whiteboard {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let board = cx.use_state(|| 0_usize);
        let charts = [cx.use_state(flowchart), cx.use_state(mind_map)];
        let selection = cx.use_state(FlowSelection::default);
        let histories = [cx.use_state(FlowHistory::new), cx.use_state(FlowHistory::new)];
        let arrivals = cx.use_state(|| None::<FlowNode>);
        let views = [cx.use_state(FlowViewport::fit), cx.use_state(FlowViewport::fit)];
        let which = board.get().min(1);
        let (graph, history, view) = (&charts[which], &histories[which], &views[which]);
        let layout = if which == 0 {
            FlowLayout::new(FlowDirection::Down)
        } else {
            FlowLayout::tree(FlowDirection::Right).gaps(72.0, 24.0)
        };
        let canvas = FlowCanvas::new(graph.binding())
            .view(view.binding())
            .ports_on_hover(true)
            .label(if which == 0 { "Fluxograma" } else { "Mapa mental" })
            .selection(selection.binding())
            .history(history.binding())
            .arrivals(arrivals.binding())
            .layout(layout)
            .next_node(next)
            .kind("step", FlowKind::new(IconName::GitBranch, "Etapa"))
            .kind("idea", FlowKind::new(IconName::Zap, "Ideia"))
            .on_export(ravenrs_flow_export::saver(cx.file_picker(), "Quadro"));
        let inspector = FlowInspector::new(graph.binding(), selection.binding())
            .history(history.binding())
            .kind("step", "Etapa")
            .kind("idea", "Ideia")
            .kind("note", "Nota");
        let switcher = SegmentedControl::new(board.binding(), ["Fluxograma", "Mapa mental"]);
        cx.child(
            &VStack::new((
                switcher,
                HStack::new((
                    VStack::new((
                        Keyed::new(
                            which as u64,
                            FlowLibrary::new(graph.binding())
                                .title("Quadros")
                                .key(if which == 0 { "flowcharts" } else { "mind-maps" })
                                .history(history.binding())
                                .selection(selection.binding())
                                .view(view.binding())
                                .storage(self.storage.clone()),
                        ),
                        toolbox().title("Formas").arrivals(arrivals.binding()),
                    ))
                    .spacing(0.0)
                    .align(Align::Stretch),
                    Keyed::new(which as u64, canvas),
                    inspector,
                ))
                .align(Align::Stretch)
                .grow(1.0),
            ))
            .spacing(8.0)
            .padding(8.0)
            .align(Align::Stretch)
            .grow(1.0),
        )
    }
}

/// The whole app.
pub fn app() -> Localized<Whiteboard> {
    app_kept_in(Rc::new(MemoryStorage::default()))
}

/// The whole app, keeping its diagrams in `storage`.
pub fn app_kept_in(storage: Rc<dyn Storage>) -> Localized<Whiteboard> {
    Localized::new(Labels::pt_br(), Whiteboard { storage })
}
