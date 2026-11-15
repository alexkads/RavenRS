//! What makes a process well formed, said in words people read: checked on the whole diagram
//! when asked, and on each connection as it is made.

use std::collections::BTreeSet;

use ravenrs_widgets::*;

use crate::elements::{Family, family};

/// Whether a connection may be made: nothing goes into a start, nothing leaves an end.
pub fn validate(graph: &FlowGraph, from: &FlowEndpoint, to: &FlowEndpoint) -> Result<(), String> {
    let kind = |id: &str| graph.find_node(id).map(|n| n.kind.clone()).unwrap_or_default();
    if kind(&to.node) == "startEvent" {
        return Err("Um evento de início não recebe fluxo".into());
    }
    if kind(&from.node) == "endEvent" {
        return Err("Um evento de fim não leva a lugar nenhum".into());
    }
    if from.node == to.node {
        return Err("Um elemento não liga a si mesmo".into());
    }
    Ok(())
}

/// What is wrong with the process, one sentence each with the elements it is about; empty
/// when it is well formed.
pub fn problems(graph: &FlowGraph) -> Vec<FlowIssue> {
    let mut found = Vec::new();
    let flows: Vec<&FlowEdge> = graph.edges.iter().filter(|e| e.kind.is_empty()).collect();
    let in_flow =
        |n: &FlowNode| matches!(family(&n.kind), Some(Family::Event | Family::Activity | Family::Gateway));
    let named =
        |n: &FlowNode| if n.title.trim().is_empty() { n.id.clone() } else { format!("“{}”", n.title) };
    let about = |text: String, n: &FlowNode| FlowIssue::new(text, [n.id.clone()]);
    let starts: Vec<&FlowNode> = graph.nodes.iter().filter(|n| n.kind == "startEvent").collect();
    if starts.is_empty() {
        found.push(FlowIssue::new("O processo não tem evento de início", Vec::<String>::new()));
    }
    if !graph.nodes.iter().any(|n| n.kind == "endEvent") {
        found.push(FlowIssue::new("O processo não tem evento de fim", Vec::<String>::new()));
    }
    for node in graph.nodes.iter().filter(|n| in_flow(n)) {
        let ins = flows.iter().filter(|e| e.to.node == node.id).count();
        let outs = flows.iter().filter(|e| e.from.node == node.id).count();
        if node.title.trim().is_empty() && family(&node.kind) == Some(Family::Activity) {
            found.push(about(format!("A tarefa {} não tem nome", node.id), node));
        }
        match node.kind.as_str() {
            "startEvent" if outs == 0 => {
                found.push(about(format!("O início {} não leva a nada", named(node)), node))
            }
            "endEvent" if ins == 0 => {
                found.push(about(format!("O fim {} nunca é alcançado", named(node)), node))
            }
            "startEvent" | "endEvent" => {}
            _ if outs == 0 => found.push(about(format!("{} não leva a lugar nenhum", named(node)), node)),
            _ => {}
        }
        if node.kind == "exclusiveGateway" && outs == 1 && ins == 1 {
            found.push(about(format!("A decisão {} tem um caminho só", named(node)), node));
        }
        let unnamed = flows.iter().filter(|e| e.from.node == node.id && e.label.is_none()).count();
        if node.kind == "exclusiveGateway" && outs > 1 && unnamed > 0 {
            found.push(about(format!("Os caminhos da decisão {} precisam de nome", named(node)), node));
        }
    }
    // What no start reaches.
    let mut reached: BTreeSet<String> = starts.iter().map(|n| n.id.clone()).collect();
    let mut grew = true;
    while grew {
        grew = false;
        for e in &flows {
            if reached.contains(&e.from.node) && reached.insert(e.to.node.clone()) {
                grew = true;
            }
        }
    }
    if !starts.is_empty() {
        for node in graph.nodes.iter().filter(|n| in_flow(n) && !reached.contains(&n.id)) {
            found.push(about(format!("{} não é alcançado a partir do início", named(node)), node));
        }
    }
    found
}
