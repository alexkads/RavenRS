//! What is asked of the local model, and how its answer becomes a process. The model only
//! writes a plan in a small JSON; the app checks it, builds the elements and lays them out,
//! so a model never writes BPMN, and an answer that is not a plan changes nothing.

use ravenrs_widgets::*;
use serde_json::Value;

use crate::elements::{self, SEQUENCE};

/// How the model is told to draft a process.
pub const DRAFT_RULES: &str = "Você transforma a descrição de um processo de negócio em um plano BPMN. \
Responda somente com JSON numa linha só, sem espaços sobrando nem comentários, no formato \
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

/// A process drafted from a model's answer, with what had to be mended on the way.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drafted {
    /// The process, laid out from left to right.
    pub graph: FlowGraph,
    /// What the answer got wrong and was mended, one sentence each, for the person to see.
    pub mended: Vec<String>,
}

/// The objects of the list after `"key"`, as text, each whole even when the list around them
/// is broken: small models leave out a comma or close a list twice.
fn objects<'a>(text: &'a str, key: &str) -> Vec<&'a str> {
    let Some(at) = text.find(&format!("\"{key}\"")) else { return Vec::new() };
    let Some(open) = text[at..].find('[').map(|i| at + i) else { return Vec::new() };
    let mut found = Vec::new();
    let (mut depth, mut start, mut quoted, mut escaped) = (0usize, None, false, false);
    for (i, c) in text[open + 1..].char_indices() {
        let i = open + 1 + i;
        if quoted {
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => quoted = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => quoted = true,
            '{' => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            '}' if depth > 0 => {
                depth -= 1;
                if depth == 0
                    && let Some(from) = start.take()
                {
                    found.push(&text[from..=i]);
                }
            }
            ']' if depth == 0 => break,
            _ => {}
        }
    }
    found
}

/// The text after `"name":` in an object, read even when the object is not valid JSON.
fn field(object: &str, name: &str) -> Option<String> {
    if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(object) {
        return map.get(name).and_then(Value::as_str).map(|v| v.trim().to_owned());
    }
    let at = object.find(&format!("\"{name}\""))? + name.len() + 2;
    let rest = object[at..].trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    Some(rest[..rest.find('"')?].trim().to_owned())
}

/// Every quoted text in an object, in order.
fn texts(object: &str) -> Vec<String> {
    object.split('"').skip(1).step_by(2).map(str::to_owned).collect()
}

/// The process a model's answer plans, laid out from left to right, with what had to be
/// mended (a step without a type, a missing start or end); why not, when the answer is no plan.
pub fn plan(answer: &str) -> Result<Drafted, String> {
    let steps = objects(answer, "etapas");
    if steps.is_empty() {
        return Err(if answer.contains('{') {
            "O plano não tem etapas".into()
        } else {
            "A resposta não tem um plano em JSON".into()
        });
    }
    let mut mended = Vec::new();
    let mut graph = FlowGraph::new();
    for (i, step) in steps.iter().enumerate() {
        let id = field(step, "id").filter(|s| !s.is_empty()).unwrap_or_else(|| format!("e{}", i + 1));
        if graph.find_node(&id).is_some() {
            mended.push(format!("A etapa {id} aparecia duas vezes; ficou a primeira"));
            continue;
        }
        let tipo = match field(step, "tipo") {
            Some(tipo) => tipo,
            // A step called "fim" or "inicio" without a type is that.
            None => {
                let guess = if matches!(id.as_str(), "fim" | "inicio" | "início") {
                    id.clone()
                } else {
                    "tarefa".into()
                };
                mended.push(format!("A etapa {id} veio sem tipo; ficou {guess}"));
                guess
            }
        };
        let name = field(step, "nome").or_else(|| texts(step).last().cloned()).unwrap_or_default();
        graph = graph.node(elements::element(kind_of(&tipo), &id, &name));
    }
    let links = objects(answer, "ligacoes");
    for (i, link) in links.iter().enumerate() {
        let (from, to) = (field(link, "de").unwrap_or_default(), field(link, "para").unwrap_or_default());
        let Some(a) = graph.find_node(&from) else {
            mended.push(format!("A ligação {} sai de {from}, que não existe; ficou de fora", i + 1));
            continue;
        };
        if graph.find_node(&to).is_none() {
            mended.push(format!("A ligação {} vai para {to}, que não existe; ficou de fora", i + 1));
            continue;
        }
        if a.kind == "endEvent" || from == to {
            mended.push(format!("A ligação de {from} para {to} não cabe num processo; ficou de fora"));
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
        edge.label = field(link, "rotulo").filter(|l| !l.is_empty());
        graph = graph.edge(edge);
    }
    // A process starts and ends: what is missing is added and said.
    let leads_on = |g: &FlowGraph, id: &str| g.edges.iter().any(|e| e.from.node == id);
    let reached = |g: &FlowGraph, id: &str| g.edges.iter().any(|e| e.to.node == id);
    if !graph.nodes.iter().any(|n| n.kind == "startEvent") {
        let first = graph.nodes.iter().find(|n| !reached(&graph, &n.id)).map(|n| n.id.clone());
        graph = graph.node(elements::element("startEvent", "inicio_auto", "Início"));
        if let Some(first) = first {
            graph = graph.edge(elements::connection(
                "f_inicio",
                FlowEndpoint::new("inicio_auto", "out"),
                FlowEndpoint::new(first, "in"),
                SEQUENCE,
            ));
        }
        mended.push("Faltava o início; acrescentei um".into());
    }
    if !graph.nodes.iter().any(|n| n.kind == "endEvent") {
        let loose: Vec<String> = graph
            .nodes
            .iter()
            .filter(|n| n.kind != "startEvent" && !leads_on(&graph, &n.id))
            .map(|n| n.id.clone())
            .collect();
        graph = graph.node(elements::element("endEvent", "fim_auto", "Fim"));
        for (i, id) in loose.iter().enumerate() {
            graph = graph.edge(elements::connection(
                &format!("f_fim{i}"),
                FlowEndpoint::new(id.clone(), "out"),
                FlowEndpoint::new("fim_auto", "in"),
                SEQUENCE,
            ));
        }
        mended.push("Faltava o fim; acrescentei um".into());
    }
    FlowLayout::new(FlowDirection::Right).gaps(64.0, 48.0).apply(&mut graph);
    Ok(Drafted { graph, mended })
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
