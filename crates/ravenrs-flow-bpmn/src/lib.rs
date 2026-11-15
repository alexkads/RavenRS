//! BPMN 2.0 for the RavenRS diagram components, as data and functions only: the components
//! are the library's own, and this crate hands them what BPMN is.
//!
//! - [`notation`] is BPMN as a [`FlowNotation`]: its elements grouped for a palette, how
//!   tasks look, which connections may be made and how each is drawn (sequence, association,
//!   message), what comes next from the keys, the checks of a process and the BPMN XML
//!   format. Give it to `FlowCanvas`, `FlowToolbox`, `FlowInspector` and `FlowLibrary`.
//! - [`xml`] writes a process as BPMN 2.0 XML with its drawing (BPMNDI) and reads what other
//!   tools write, elements, lanes, labels and bend points included.
//! - [`rules`] says what is wrong with a process.
//! - [`plan`] is what a local model is asked, and how its answer becomes a process: the model
//!   only writes a small plan the app checks, never the XML.
//!
//! Words shown are in Brazilian Portuguese.
//!
//! ```
//! use ravenrs_reactive::State;
//! use ravenrs_widgets::{FlowCanvas, FlowGraph, FlowToolbox};
//!
//! let bpmn = ravenrs_flow_bpmn::notation();
//! let graph = State::new(ravenrs_flow_bpmn::example_process());
//! let canvas = FlowCanvas::new(graph.binding()).notation(&bpmn);
//! let palette = FlowToolbox::new().notation(&bpmn);
//! let file = ravenrs_flow_bpmn::xml::to_xml(&graph.get(), "Compra");
//! assert!(file.contains("<bpmn:process"));
//! ```

use ravenrs_widgets::*;

pub mod elements;
pub mod plan;
pub mod rules;
pub mod xml;

use elements::{ELEMENTS, Family, LANE, SEQUENCE};

/// BPMN 2.0 as a notation for the diagram components.
pub fn notation() -> FlowNotation {
    let mut notation = FlowNotation::new();
    for (group, family) in [
        ("Eventos", Family::Event),
        ("Atividades", Family::Activity),
        ("Gateways", Family::Gateway),
        ("Dados e anotações", Family::Artifact),
        ("Raias", Family::Lane),
    ] {
        for (kind, caption, _) in ELEMENTS.iter().filter(|(_, _, f)| *f == family) {
            notation = notation.model(group, *caption, elements::element(kind, kind, caption));
        }
    }
    for (kind, caption, _) in ELEMENTS {
        notation = notation.kind(kind, caption);
    }
    for (kind, word) in
        [("task", "Tarefa"), ("userTask", "Usuário"), ("serviceTask", "Serviço"), ("scriptTask", "Script")]
    {
        notation = notation.look(kind, FlowKind::new(elements::icon(kind), word));
    }
    notation
        .connection(|graph, edge| {
            let mut shaped = FlowGraph { edges: vec![edge], ..graph.clone() };
            elements::normalize(&mut shaped);
            shaped.edges.pop().unwrap_or_default()
        })
        .validate(rules::validate)
        .next_node(next)
        .checks(rules::problems)
        .format(bpmn_format())
}

/// BPMN 2.0 XML as a file format of a `FlowLibrary`.
pub fn bpmn_format() -> FlowFileFormat {
    FlowFileFormat::new("BPMN 2.0", "bpmn", |text| xml::from_xml(text).map(|(graph, _)| graph), xml::to_xml)
}

/// The next element made with the arrow or Cmd/Ctrl+Enter: a task after anything that leads
/// on, nothing after an end or outside the flow.
pub fn next(from: &FlowNode, _side: PortSide) -> Option<FlowNode> {
    match elements::family(&from.kind) {
        Some(Family::Event | Family::Activity | Family::Gateway) if from.kind != "endEvent" => {
            Some(elements::element("task", "task", "Nova tarefa"))
        }
        _ => None,
    }
}

fn at(node: FlowNode, x: f32, y: f32) -> FlowNode {
    FlowNode { x, y, ..node }
}

fn flow(id: &str, from: (&str, &str), to: (&str, &str)) -> FlowEdge {
    elements::connection(id, FlowEndpoint::new(from.0, from.1), FlowEndpoint::new(to.0, to.1), SEQUENCE)
}

/// A purchase approved by the finance team, in two lanes: a process to start from.
pub fn example_process() -> FlowGraph {
    FlowGraph::new()
        .node(at(elements::element(LANE, "lane_requester", "Solicitante"), 0.0, 0.0).size(1040.0, 200.0))
        .node(at(elements::element(LANE, "lane_finance", "Financeiro"), 0.0, 200.0).size(1040.0, 240.0))
        .node(
            at(elements::element("startEvent", "start", "Precisa comprar"), 72.0, 80.0)
                .parent("lane_requester"),
        )
        .node(
            at(elements::element("userTask", "request", "Pedir compra"), 168.0, 64.0)
                .parent("lane_requester"),
        )
        .node(
            at(elements::element("exclusiveGateway", "amount", "Valor alto?"), 424.0, 272.0)
                .parent("lane_finance"),
        )
        .node(
            at(elements::element("userTask", "approve", "Aprovar pedido"), 560.0, 260.0)
                .parent("lane_finance"),
        )
        .node(
            at(elements::element("serviceTask", "order", "Emitir pedido ao fornecedor"), 808.0, 64.0)
                .parent("lane_requester"),
        )
        .node(at(elements::element("endEvent", "done", "Compra feita"), 976.0, 80.0).parent("lane_requester"))
        .node(at(elements::element("dataStoreReference", "erp", "ERP"), 848.0, 312.0).parent("lane_finance"))
        .node(
            at(
                elements::element("textAnnotation", "note", "Acima de R$ 5.000 precisa de aprovação"),
                200.0,
                352.0,
            )
            .parent("lane_finance"),
        )
        .edge(flow("f1", ("start", "out"), ("request", "in")))
        .edge(flow("f2", ("request", "down"), ("amount", "top")))
        .edge(FlowEdge { label: Some("Sim".into()), ..flow("f3", ("amount", "out"), ("approve", "in")) })
        .edge(FlowEdge {
            label: Some("Não".into()),
            ..flow("f4", ("amount", "up"), ("order", "in")).waypoint(452.0, 100.0)
        })
        .edge(flow("f5", ("approve", "out"), ("order", "in")))
        .edge(flow("f6", ("order", "out"), ("done", "in")))
        .edge(elements::connection(
            "a1",
            FlowEndpoint::new("order", "down"),
            FlowEndpoint::new("erp", "in"),
            elements::ASSOCIATION,
        ))
}
