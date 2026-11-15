//! The kinds of node of `Catalog::builtin`, as the app shows them: a name in Portuguese, an
//! icon and a group, and the `FlowToolbox` they are dragged from, whose cards are the bridge's
//! (`ravenrs_flows::card`), so a node dragged in has the ports `to_graph` would give it.

use ravenrs::prelude::*;
use ravenrs_flows::Catalog;

/// How a kind shows: its name, its icon and its group in the toolbox.
pub fn look(kind: &str) -> (&'static str, IconName, &'static str) {
    match kind {
        "manual" => ("Início manual", IconName::Play, "Gatilhos"),
        "if" => ("Se", IconName::Split, "Lógica"),
        "switch" => ("Escolher caminho", IconName::GitBranch, "Lógica"),
        "merge" => ("Juntar", IconName::Merge, "Lógica"),
        "batches" => ("Em lotes", IconName::Layers, "Lógica"),
        "wait" => ("Esperar", IconName::Timer, "Tempo"),
        "delay" => ("Esperar, mesmo fechado", IconName::Clock, "Tempo"),
        "wait_event" => ("Esperar aprovação", IconName::Bell, "Tempo"),
        "set" => ("Definir campos", IconName::FileText, "Dados"),
        "filter" => ("Filtrar", IconName::Filter, "Dados"),
        "sort" => ("Ordenar", IconName::ListOrdered, "Dados"),
        "group" => ("Agrupar", IconName::Sigma, "Dados"),
        "split" => ("Separar lista", IconName::Split, "Dados"),
        "join" => ("Juntar em lista", IconName::Layers, "Dados"),
        _ => ("Nó", IconName::Square, "Outros"),
    }
}

/// The groups of the toolbox, in order.
const GROUPS: [&str; 5] = ["Gatilhos", "Lógica", "Dados", "Tempo", "Outros"];

/// The toolbox, a group per kind of work, with every kind of `catalog`.
pub fn toolbox(catalog: &Catalog) -> FlowToolbox {
    let specs = catalog.specs();
    let mut toolbox = FlowToolbox::new().title("Nós").searchable(true);
    for group in GROUPS {
        let mut any = false;
        for spec in specs.iter().filter(|s| look(&s.kind).2 == group) {
            if !any {
                toolbox = toolbox.group(group);
                any = true;
            }
            toolbox =
                toolbox.model(look(&spec.kind).0, ravenrs_flows::card(spec, &spec.kind, look(&spec.kind).0));
        }
    }
    toolbox
}

/// `flow` drawn for the canvas, each node the catalog names in English called by its name in
/// Portuguese; a title the person gave stays.
pub fn drawn(flow: &ravenrs_flows::FlowDefinition, catalog: &Catalog) -> FlowGraph {
    let mut graph = flow.to_graph(catalog);
    let specs = catalog.specs();
    for card in &mut graph.nodes {
        if specs.iter().any(|s| s.kind == card.kind && s.title == card.title) {
            card.title = look(&card.kind).0.to_owned();
        }
    }
    graph
}

/// The canvas with every kind of `catalog` drawn with its icon and name.
pub fn kinds(mut canvas: FlowCanvas, catalog: &Catalog) -> FlowCanvas {
    for spec in catalog.specs() {
        let (name, icon, group) = look(&spec.kind);
        let tone = if group == "Gatilhos" { Semantic::Warning } else { Semantic::Accent };
        canvas = canvas.kind(spec.kind.as_str(), FlowKind::new(icon, name).tone(tone));
    }
    canvas
}
