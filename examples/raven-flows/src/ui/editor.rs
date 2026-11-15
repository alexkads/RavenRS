//! The editor of a flow: the canvas, the toolbox or the parameters of the node selected, and
//! what each node did in the last run.

use ravenrs::prelude::*;
use ravenrs_flows::{Catalog, FlowDefinition, GRAPH_KEY, NodeDef, NodeStatus, Plan};
use serde_json::Map;
use serde_json::Value as Json;

use super::{FLOWS, RavenFlows, Screens};
use crate::casa::{self, Casa};
use crate::marcas;
use crate::nos;

/// Opens the flow `id` in the editor.
pub(super) fn open_flow(casa: &Casa, catalog: &Catalog, s: &Screens, id: &str) {
    let Some(flow) = casa.flow(id) else {
        s.message.set("Este fluxo não existe mais.".to_owned());
        return;
    };
    s.graph.set(nos::drawn(&flow, catalog));
    s.history.set(FlowHistory::new());
    s.selection.set(FlowSelection::default());
    s.view.set(FlowViewport::fit());
    s.parameters_of.set(None);
    s.open.set(Some(id.to_owned()));
    s.page.set(FLOWS);
}

/// The flow as drawn, with the id, name and revision the drawing keeps.
fn drawn(graph: &FlowGraph, catalog: &Catalog) -> FlowDefinition {
    FlowDefinition::from_graph(graph, catalog)
}

/// Saves the drawing; the saved flow, or why not.
fn save(casa: &Casa, catalog: &Catalog, s: &Screens) -> Result<FlowDefinition, String> {
    let mut flow = drawn(&s.graph.get(), catalog);
    casa.save(&mut flow)?;
    let revision = flow.revision;
    s.graph.update(|g| {
        if let Some(head) = g.data.get_mut(GRAPH_KEY) {
            head["revision"] = revision.into();
        }
    });
    Ok(flow)
}

/// The parameters of the card `id` of `graph`, as JSON text.
fn parameters_text(graph: &FlowGraph, id: &str) -> String {
    let Some(card) = graph.find_node(id) else { return String::new() };
    let parameters = card
        .data
        .get(GRAPH_KEY)
        .and_then(|setup| setup.get("parameters"))
        .cloned()
        .unwrap_or_else(|| Json::Object(Map::new()));
    serde_json::to_string_pretty(&parameters).unwrap_or_default()
}

/// Puts `text` as the parameters of the card `id`: refused, with why, when it is not a JSON
/// object.
pub fn apply_parameters(graph: &mut FlowGraph, id: &str, text: &str) -> Result<(), String> {
    let parameters: Map<String, Json> = match serde_json::from_str::<Json>(text) {
        Ok(Json::Object(map)) => map,
        Ok(_) => return Err("Os parâmetros são um objeto JSON, entre { e }.".to_owned()),
        Err(e) => return Err(format!("JSON inválido: {e}")),
    };
    let card = graph.nodes.iter_mut().find(|n| n.id == id).ok_or("O nó não está mais no fluxo.")?;
    let mut setup: NodeDef = card
        .data
        .get(GRAPH_KEY)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_else(|| NodeDef::new(&card.id, &card.kind));
    setup.parameters = parameters;
    let mut data = if card.data.is_object() { card.data.clone() } else { Json::Object(Map::new()) };
    data[GRAPH_KEY] = serde_json::to_value(&setup).map_err(|e| e.to_string())?;
    card.data = data;
    Ok(())
}

impl RavenFlows {
    /// The editor of the flow `id`.
    pub(super) fn editor(&self, cx: &mut BuildCx<'_>, id: &str) -> AnyView {
        let s = self.s.clone();
        let (casa, catalog) = (self.casa.clone(), self.catalog.clone());
        let _ = cx;
        // The parameters of the node selected now.
        let selected = s.selection.get().nodes.first().cloned();
        if s.parameters_of.get() != selected {
            s.parameters
                .set(selected.as_deref().map(|n| parameters_text(&s.graph.get(), n)).unwrap_or_default());
            s.parameters_of.set(selected.clone());
        }
        let name = s
            .graph
            .get()
            .data
            .get(GRAPH_KEY)
            .and_then(|h| h.get("name"))
            .and_then(Json::as_str)
            .unwrap_or("Fluxo")
            .to_owned();

        let back = {
            let s = s.clone();
            move || s.open.set(None)
        };
        let save_click = {
            let (casa, catalog, s) = (casa.clone(), catalog.clone(), s.clone());
            move || match save(&casa, &catalog, &s) {
                Ok(flow) => s.message.set(format!("Salvo, revisão {}.", flow.revision)),
                Err(why) => s.message.set(why),
            }
        };
        let run_click = {
            let (casa, catalog, s) = (casa.clone(), catalog.clone(), s.clone());
            move || {
                let flow = drawn(&s.graph.get(), &catalog);
                if let Err(errors) = Plan::compile(&flow, &catalog) {
                    let words: Vec<String> = errors.iter().map(ToString::to_string).collect();
                    s.message.set(format!("O fluxo não pode rodar: {}", words.join("; ")));
                    return;
                }
                match save(&casa, &catalog, &s).and_then(|flow| casa.run(&flow)) {
                    Ok(_) => s.message.set("Execução pedida.".to_owned()),
                    Err(why) => s.message.set(why),
                }
            }
        };
        let apply = {
            let s = s.clone();
            move || {
                let Some(node) = s.parameters_of.get() else { return };
                let mut graph = s.graph.get();
                match apply_parameters(&mut graph, &node, &s.parameters.get()) {
                    Ok(()) => {
                        let before = s.graph.get();
                        s.history.update(|h| h.record(before));
                        s.graph.set(graph);
                        s.message.set("Parâmetros aplicados. Salve para guardar.".to_owned());
                    }
                    Err(why) => s.message.set(why),
                }
            }
        };

        let mut canvas = FlowCanvas::new(s.graph.binding())
            .label("Área do fluxo")
            .view(s.view.binding())
            .selection(s.selection.binding())
            .history(s.history.binding())
            .arrivals(s.arrivals.binding())
            .editable_titles(true)
            .minimap(true)
            .grow(1.0);
        // The last run, drawn on the canvas as it moves on; the flow itself is not touched.
        if let Some(run) = casa.last_run(id) {
            canvas = canvas.execution(marcas::run_view(&casa.parts(&run.id)));
        }
        let canvas = nos::kinds(canvas, &catalog);
        let side: AnyView = match &selected {
            Some(node) => Box::new(
                VStack::new((
                    Text::new(format!("Parâmetros de {node}")).style(TextStyle::Headline),
                    Text::new("Em JSON, até o formulário de parâmetros existir (#530).")
                        .style(TextStyle::Callout)
                        .foreground(Semantic::InkMuted),
                    CodeEditor::new(s.parameters.binding()).language(CodeLanguage::Plain).fill(),
                    Button::new("Aplicar parâmetros")
                        .icon(IconName::Check)
                        .style(ButtonStyle::Secondary)
                        .on_click(apply),
                ))
                .spacing(8.0)
                .padding(12.0)
                .align(Align::Stretch)
                .width(320.0),
            ),
            None => {
                Box::new(VStack::new((nos::toolbox(&catalog).arrivals(s.arrivals.binding()),)).width(240.0))
            }
        };

        let mut table = Table::new([
            Column::new("Nó"),
            Column::new("Estado"),
            Column::new("Itens"),
            Column::new("Tentativas"),
            Column::new("Erro"),
        ]);
        let mut summary = "Ainda não executado.".to_owned();
        if let Some(run) = casa.last_run(id) {
            summary = format!("Última execução: {}", casa::run_words(run.status));
            let parts = casa.parts(&run.id);
            // In the flow's order, the nodes that took part.
            for card in s.graph.get().nodes.iter().filter(|n| parts.contains_key(&n.id)) {
                let part = &parts[&card.id];
                let given = match part.status {
                    NodeStatus::Waiting | NodeStatus::Skipped => "—".to_owned(),
                    _ => marcas::items(marcas::given(part)),
                };
                table = table.row([
                    card.title.clone(),
                    casa::node_words(part.status).to_owned(),
                    given,
                    part.attempts.to_string(),
                    part.error.clone().unwrap_or_default(),
                ]);
            }
        }

        Box::new(
            VStack::new((
                HStack::new((
                    Button::new("Voltar aos fluxos")
                        .icon(IconName::ArrowLeft)
                        .style(ButtonStyle::Ghost)
                        .on_click(back),
                    Text::new(name).style(TextStyle::Title),
                    Spacer::new(),
                    Button::new("Salvar")
                        .icon(IconName::Save)
                        .style(ButtonStyle::Secondary)
                        .on_click(save_click),
                    Button::new("Executar fluxo").icon(IconName::Play).on_click(run_click),
                ))
                .spacing(8.0)
                .align(Align::Center)
                .padding(12.0),
                HStack::new((side, canvas)).align(Align::Stretch).grow(1.0),
                VStack::new((Text::new(summary).style(TextStyle::Headline), table))
                    .spacing(8.0)
                    .padding(12.0),
            ))
            .spacing(0.0)
            .align(Align::Stretch)
            .grow(1.0),
        )
    }
}
