//! What is asked of the local model, and how its answer becomes a process. The model only
//! writes a plan in a small JSON; the app checks it, builds the elements and lays them out,
//! so a model never writes BPMN, and an answer that is not a plan changes nothing.

use ravenrs_widgets::*;
use serde_json::Value;

use crate::elements::{self, SEQUENCE};

/// How the model is told to draft a process.
pub const DRAFT_RULES: &str = "Você transforma a descrição de um processo de negócio em um plano BPMN. \
Responda somente com JSON, sem comentários, no formato \
{\"etapas\":[{\"id\":\"e1\",\"tipo\":\"inicio\",\"nome\":\"...\"}],\"ligacoes\":[{\"de\":\"e1\",\"para\":\"e2\",\"rotulo\":\"\"}]}. \
Tipos: inicio, fim, tarefa, usuario, servico, script, decisao, paralelo, espera. \
Comece com um inicio e termine com pelo menos um fim. Uma decisao tem duas ou mais saídas, cada uma com rotulo. \
Use nomes curtos, no infinitivo, em português do Brasil.";

/// How the model is told to review one.
pub const REVIEW_RULES: &str = "Você revisa processos BPMN. Em português do Brasil, em até cinco itens curtos, \
aponte etapas que faltam, decisões sem saída, nomes vagos e o que poderia ser automatizado. Use só o processo dado.";

/// The element's XML name for a type the model wrote.
fn kind_of(tipo: &str) -> &'static str {
    match tipo.trim().to_lowercase().as_str() {
        "inicio" | "início" | "start" => "startEvent",
        "fim" | "end" => "endEvent",
        "usuario" | "usuário" | "user" => "userTask",
        "servico" | "serviço" | "service" => "serviceTask",
        "script" => "scriptTask",
        "decisao" | "decisão" | "gateway" | "exclusivo" => "exclusiveGateway",
        "paralelo" | "parallel" => "parallelGateway",
        "espera" | "timer" | "aguardar" => "intermediateCatchEvent",
        _ => "task",
    }
}

/// The process a model's answer plans, laid out from left to right; why not, when the
/// answer is no plan.
pub fn plan(answer: &str) -> Result<FlowGraph, String> {
    let (start, end) =
        (answer.find('{').ok_or("Sem JSON na resposta")?, answer.rfind('}').ok_or("Sem JSON")?);
    let value: Value = serde_json::from_str(answer.get(start..=end).ok_or("JSON cortado")?)
        .map_err(|e| format!("JSON inválido: {e}"))?;
    let steps = value.get("etapas").and_then(Value::as_array).ok_or("Sem etapas")?;
    let mut graph = FlowGraph::new();
    for (i, step) in steps.iter().enumerate() {
        let id = step.get("id").and_then(Value::as_str).map_or_else(|| format!("e{}", i + 1), str::to_owned);
        if graph.find_node(&id).is_some() {
            continue;
        }
        let kind = kind_of(step.get("tipo").and_then(Value::as_str).unwrap_or("tarefa"));
        let name = step.get("nome").and_then(Value::as_str).unwrap_or("").trim().to_owned();
        graph = graph.node(elements::element(kind, &id, &name));
    }
    if graph.nodes.is_empty() {
        return Err("O plano não tem etapas".into());
    }
    let links = value.get("ligacoes").and_then(Value::as_array).cloned().unwrap_or_default();
    for (i, link) in links.iter().enumerate() {
        let field = |name: &str| link.get(name).and_then(Value::as_str).unwrap_or("").trim().to_owned();
        let (from, to) = (field("de"), field("para"));
        let (Some(a), Some(_)) = (graph.find_node(&from), graph.find_node(&to)) else { continue };
        if a.kind == "endEvent" || from == to {
            continue;
        }
        // A split leaves by its side, then below, then above; the rest leave by the side.
        let used = graph.edges.iter().filter(|e| e.from.node == from).count();
        let out = match (a.kind.ends_with("Gateway"), used) {
            (true, 1) => "down",
            (true, 2) => "up",
            _ => "out",
        };
        let mut edge = elements::connection(
            &format!("f{}", i + 1),
            FlowEndpoint::new(from, out),
            FlowEndpoint::new(to, "in"),
            SEQUENCE,
        );
        let label = field("rotulo");
        edge.label = (!label.is_empty()).then_some(label);
        graph = graph.edge(edge);
    }
    FlowLayout::new(FlowDirection::Right).gaps(64.0, 48.0).apply(&mut graph);
    Ok(graph)
}

/// The process in words, for the model to review.
pub fn summary(graph: &FlowGraph) -> String {
    let name = |id: &str| {
        graph.find_node(id).map_or(id.to_owned(), |n| format!("{} ({})", n.title, elements::caption(&n.kind)))
    };
    let mut out = String::from("Etapas:\n");
    for node in graph.nodes.iter().filter(|n| elements::family(&n.kind).is_some()) {
        out.push_str(&format!("- {}\n", name(&node.id)));
    }
    out.push_str("Ligações:\n");
    for e in graph.edges.iter().filter(|e| e.kind.is_empty()) {
        let label = e.label.as_deref().map(|l| format!(" [{l}]")).unwrap_or_default();
        out.push_str(&format!("- {} -> {}{label}\n", name(&e.from.node), name(&e.to.node)));
    }
    out
}
