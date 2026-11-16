//! `FlowToolbox`, `FlowInspector`, `FlowLayers` and `FlowLibrary`, every feature: the panels
//! around a `FlowCanvas`, all on the same diagram, selection and history. The toolbox gives
//! the models of node, the inspector changes what is selected, the layers pick, hide and order
//! the nodes, and the library keeps the diagrams, also as a file of its own format.

use std::rc::Rc;

use ravenrs::core::{MemoryStorage, Storage};
use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "FlowPanelsDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Painéis do fluxo";

/// The diagram the page starts with, and the library offers as a starter.
pub fn order() -> FlowGraph {
    FlowGraph::new()
        .node(FlowNode::new("start", "start", "Pedido chegou", 40.0, 80.0).port(FlowPort::output("out")))
        .node(
            FlowNode::new("check", "step", "Conferir estoque", 320.0, 60.0)
                .port(FlowPort::input("in"))
                .port(FlowPort::output("out")),
        )
        .node(FlowNode::new("end", "end", "Entregar", 600.0, 100.0).port(FlowPort::input("in")))
        .edge(FlowEdge::new("e1", FlowEndpoint::new("start", "out"), FlowEndpoint::new("check", "in")))
        .edge(FlowEdge::new("e2", FlowEndpoint::new("check", "out"), FlowEndpoint::new("end", "in")))
}

fn step(title: &str) -> FlowNode {
    FlowNode::new("", "step", title, 0.0, 0.0).port(FlowPort::input("in")).port(FlowPort::output("out"))
}

/// A file of one title per line: the simplest format a library can be taught.
fn lines_format() -> FlowFileFormat {
    FlowFileFormat::new(
        "Lista de etapas",
        "txt",
        |text| {
            let mut graph = FlowGraph::new();
            for (i, title) in text.lines().map(str::trim).filter(|t| !t.is_empty()).enumerate() {
                graph =
                    graph.node(FlowNode::new(format!("n{i}"), "step", title, 40.0 + i as f32 * 260.0, 80.0));
            }
            if graph.nodes.is_empty() { Err("O arquivo não tem nenhuma etapa".into()) } else { Ok(graph) }
        },
        |graph, _name| graph.nodes.iter().map(|n| n.title.clone()).collect::<Vec<_>>().join("\n"),
    )
}

/// The page.
#[derive(Debug, Default)]
pub struct FlowPanelsPage;

impl View for FlowPanelsPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let graph = cx.use_state(order);
        let selection = cx.use_state(FlowSelection::default);
        let history = cx.use_state(FlowHistory::new);
        let view = cx.use_state(FlowViewport::fit);
        let arrivals = cx.use_state(|| None::<FlowNode>);
        // Kept between builds, so what the library saved is there when the page rebuilds.
        let storage = cx.use_state(|| Rc::new(MemoryStorage::default()) as Rc<dyn Storage>);
        let read_only = cx.use_state(|| false);
        let searchable = cx.use_state(|| true);
        let said = cx.use_state(|| "Leve uma forma ao diagrama ou escolha um nó nas camadas.".to_string());
        let narrow = cx.viewport().width < 720.0;
        let room = (cx.viewport().width - 32.0).max(240.0);
        let panel = if narrow { room } else { 240.0 };

        let notation = FlowNotation::new()
            .kind("start", "Início")
            .kind("step", "Etapa")
            .kind("end", "Fim")
            .look("start", FlowKind::new(IconName::Zap, "Início").tone(Semantic::Warning))
            .look("end", FlowKind::new(IconName::Check, "Fim").tone(Semantic::Success))
            .model("Pedido", "Etapa", step("Nova etapa"))
            .model("Pedido", "Fim", FlowNode::new("", "end", "Fim", 0.0, 0.0).port(FlowPort::input("in")))
            .format(lines_format());

        let toolbox = FlowToolbox::new()
            .notation(&notation)
            .group("Atalhos")
            .model("Conferência", step("Conferir de novo"))
            .model("Embalagem", step("Embalar"))
            .title("Peças do pedido")
            .searchable(searchable.get())
            .arrivals(arrivals.binding())
            .width(panel);
        let changed = said.clone();
        let inspector = FlowInspector::new(graph.binding(), selection.binding())
            .history(history.binding())
            .notation(&notation)
            .kind("note", "Nota")
            .on_change(move |_, after, change| {
                // A title cannot be left blank: the inspector shows why and keeps the old one.
                if after.nodes.iter().any(|n| n.title.trim().is_empty()) {
                    return Err("Todo nó precisa de um título".into());
                }
                changed.set(format!("Propriedades: {change:?}"));
                Ok(())
            })
            .read_only(read_only.get())
            .width(if narrow { room } else { 288.0 });
        let layers =
            FlowLayers::new(graph.binding(), selection.binding()).history(history.binding()).width(panel);
        let library = FlowLibrary::new(graph.binding())
            .history(history.binding())
            .selection(selection.binding())
            .view(view.binding())
            .storage(storage.get())
            .key("fluxos-do-centro")
            .title("Fluxos do pedido")
            .notation(&notation)
            .format(lines_format())
            .starter("Pedido de exemplo", order())
            .width(panel);
        let canvas = FlowCanvas::new(graph.binding())
            .notation(&notation)
            .selection(selection.binding())
            .history(history.binding())
            .view(view.binding())
            .arrivals(arrivals.binding())
            .read_only(read_only.get())
            .label("Diagrama dos painéis")
            .size(if narrow { room } else { 620.0 }, 420.0)
            .grow(0.0);

        let options = HStack::new((
            Toggle::new(read_only.binding()).label("Painéis só de leitura"),
            Toggle::new(searchable.binding()).label("Busca nas peças"),
        ))
        .spacing(16.0)
        .wrap();
        let selected = selection.get();
        let state = VStack::new((
            Text::new(said.get()),
            Text::new(format!("Nós no diagrama: {}", graph.get().nodes.len())),
            Text::new(format!("Ocultos: {}", graph.get().nodes.iter().filter(|n| n.hidden).count())),
            Text::new(format!("Selecionados: {}", selected.nodes.len() + selected.edges.len())),
        ))
        .spacing(4.0);
        let body: AnyView = if narrow {
            Box::new(VStack::new((toolbox, canvas, inspector, layers, library)).spacing(12.0))
        } else {
            Box::new(
                VStack::new((
                    HStack::new((toolbox, canvas, inspector)).spacing(12.0).align(Align::Start),
                    HStack::new((layers, library)).spacing(12.0).align(Align::Start),
                ))
                .spacing(12.0),
            )
        };
        cx.child(
            &VStack::new((Text::new(PAGE).style(TextStyle::Title), options, body, state))
                .spacing(12.0)
                .padding(if narrow { 0.0 } else { 16.0 })
                .align(Align::Start),
        )
    }
}
