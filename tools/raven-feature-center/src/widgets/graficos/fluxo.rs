//! `FlowCanvas`, every feature: the diagram and what it is bound to, how nodes look and what goes
//! inside them, the rules on what joins what, the keys, the grid and the view, the cables, the
//! check and the run, a run told from outside and the read-only look, the export, and the events,
//! each written on a line the tour reads.

use std::rc::Rc;

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "FlowCanvasDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Diagrama de fluxo";

fn sample() -> FlowGraph {
    FlowGraph::new()
        .node(FlowNode::new("start", "start", "Pedido chegou", 40.0, 80.0).port(FlowPort::output("out")))
        .node(
            FlowNode::new("check", "step", "Conferir estoque", 330.0, 60.0)
                .port(FlowPort::input("in"))
                .port(FlowPort::output("out")),
        )
        .node(FlowNode::new("note", "note", "Nota", 330.0, 240.0).port(FlowPort::input("in")))
        .node(FlowNode::new("end", "end", "Entregar", 620.0, 100.0).port(FlowPort::input("in")))
        .edge(FlowEdge::new("e1", FlowEndpoint::new("start", "out"), FlowEndpoint::new("check", "in")))
        .edge(FlowEdge::new("e2", FlowEndpoint::new("check", "out"), FlowEndpoint::new("end", "in")))
}

/// How each kind of node lets a run through.
fn rules() -> FlowRunRules {
    FlowRunRules {
        gate: Rc::new(|node: &FlowNode| match node.kind.as_str() {
            "start" => FlowGate::Start,
            "end" => FlowGate::End,
            _ => FlowGate::Pass,
        }),
        follows: Rc::new(|_: &FlowEdge| true),
    }
}

/// The page.
#[derive(Debug, Default)]
pub struct FlowPage;

impl View for FlowPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let graph = cx.use_state(sample);
        let view = cx.use_state(FlowViewport::fit);
        let selection = cx.use_state(FlowSelection::default);
        let history = cx.use_state(FlowHistory::new);
        let arrivals = cx.use_state(|| None::<FlowNode>);
        let said = cx.use_state(|| "Nada aconteceu ainda.".to_string());
        // Options, each a switch on the page.
        let cables = cx.use_state(|| true);
        let locked = cx.use_state(|| false);
        let minimap = cx.use_state(|| true);
        let controls = cx.use_state(|| true);
        let snap = cx.use_state(|| true);
        let quiet = cx.use_state(|| false);
        let quick = cx.use_state(|| true);
        let resizable = cx.use_state(|| true);
        let rotatable = cx.use_state(|| true);
        let titles = cx.use_state(|| true);
        let told = cx.use_state(|| false);
        let strip = cx.use_state(|| false);
        let hint = cx.use_state(|| false);
        let read_only = cx.use_state(|| false);
        // Off, the wheel scrolls this page past the diagram instead of panning it.
        let wheel = cx.use_state(|| true);
        let grid = cx.use_state(|| 1_usize);
        let layout = cx.use_state(|| 0_usize);

        let notation = FlowNotation::new()
            .kind("start", "Início")
            .look("start", FlowKind::new(IconName::Zap, "Início").tone(Semantic::Warning))
            .cables(true);
        let (on_change, on_export) = (said.clone(), said.clone());
        // On a phone the options go one under the other and the diagram takes the width there is.
        let narrow = cx.viewport().width < 720.0;
        let wide = if narrow { (cx.viewport().width - 96.0).max(240.0) } else { 900.0 };
        let canvas = FlowCanvas::new(graph.binding())
            .notation(&notation)
            .view(view.binding())
            .selection(selection.binding())
            .history(history.binding())
            .arrivals(arrivals.binding())
            .kind("step", FlowKind::new(IconName::Package, "Etapa"))
            .kind("end", FlowKind::new(IconName::Check, "Fim").tone(Semantic::Success))
            .kind("note", FlowKind::new(IconName::Info, "Nota"))
            // Rules on what joins what: nothing goes into a start.
            .validate(|g, _, to| match g.find_node(&to.node) {
                Some(node) if node.kind == "start" => Err("Nada entra num início".into()),
                _ => Ok(()),
            })
            .connection(|_, edge| edge.color("#2F9E6A"))
            // A connection let go in the open, or Enter twice on a port, searches these.
            .open_end([
                (
                    "Etapa de conferência",
                    FlowNode::new("conferir", "step", "Conferir de novo", 0.0, 0.0)
                        .port(FlowPort::input("in"))
                        .port(FlowPort::output("out")),
                ),
                (
                    "Fim do pedido",
                    FlowNode::new("fim", "end", "Fim do pedido", 0.0, 0.0).port(FlowPort::input("in")),
                ),
                (
                    "Outro início",
                    FlowNode::new("inicio", "start", "Outro início", 0.0, 0.0).port(FlowPort::output("out")),
                ),
            ])
            .on_change(move |_, after, change| {
                on_change.set(format!("Mudança: {change:?}. Nós no diagrama: {}", after.nodes.len()));
                Ok(())
            })
            .on_export(move |format, _| on_export.set(format!("Exportado em {format:?}")))
            .checks(|g| {
                g.nodes
                    .iter()
                    .filter(|n| {
                        n.kind != "note" && !g.edges.iter().any(|e| e.from.node == n.id || e.to.node == n.id)
                    })
                    .map(|n| FlowIssue::new(format!("{} está solto", n.title), [n.id.clone()]))
                    .collect()
            })
            .runs(rules())
            .next_node(|_, _| {
                Some(
                    FlowNode::new("", "step", "Próxima etapa", 0.0, 0.0)
                        .port(FlowPort::input("in"))
                        .port(FlowPort::output("out")),
                )
            })
            .content(|node| {
                (node.kind == "note")
                    .then(|| Box::new(Text::new("Uma vista do app dentro do nó.")) as AnyView)
            })
            .layout(if layout.get() == 0 { FlowLayout::default() } else { FlowLayout::mind_map() })
            .grid([12.0, 24.0, 48.0][grid.get().min(2)])
            .snap(snap.get())
            .controls(controls.get())
            .minimap(minimap.get())
            .locked(locked.get())
            .cables(cables.get())
            .ports_on_hover(quiet.get())
            .quick_create(quick.get())
            .resizable(resizable.get())
            .rotatable(rotatable.get())
            .editable_titles(titles.get())
            .range(0.5, 2.0)
            .read_only(read_only.get())
            .pan_with_wheel(wheel.get())
            .label("Diagrama de exemplo")
            .size(wide, 460.0)
            .grow(0.0);
        // height: a strip as wide as the page, as the stages of a build.
        let canvas = if strip.get() { canvas.height(200.0) } else { canvas };
        // hint: what a click does, at the end of the zoom controls.
        let canvas = if hint.get() { canvas.hint("Clique num nó para ver os detalhes") } else { canvas };
        // A run that happened in the app, as it tells the canvas: the order arrived and went on,
        // the stock check waits, and the note did not run.
        let canvas = if told.get() {
            canvas.status_icons(true).execution(
                FlowRunView::new()
                    .node("start", FlowNodeRun::new(FlowNodeStatus::Success).items("out", 3))
                    .node("check", FlowNodeRun::new(FlowNodeStatus::Waiting("Aguardando o estoque".into())))
                    .node("note", FlowNodeRun::new(FlowNodeStatus::Skipped)),
            )
        } else {
            canvas
        };

        let a = arrivals.clone();
        let options = Grid::new(
            if narrow { 1 } else { 4 },
            // A list, not a tuple: there are more switches than a tuple of views holds.
            vec![
                Toggle::new(cables.binding()).label("Ligações como cabos"),
                Toggle::new(locked.binding()).label("Travado"),
                Toggle::new(minimap.binding()).label("Mapa"),
                Toggle::new(controls.binding()).label("Controles"),
                Toggle::new(snap.binding()).label("Encaixar na grade"),
                Toggle::new(quiet.binding()).label("Portas ao passar"),
                Toggle::new(quick.binding()).label("Criação rápida"),
                Toggle::new(resizable.binding()).label("Redimensionar"),
                Toggle::new(rotatable.binding()).label("Girar"),
                Toggle::new(titles.binding()).label("Títulos editáveis"),
                Toggle::new(told.binding()).label("Execução de fora"),
                Toggle::new(strip.binding()).label("Faixa de 200 pt"),
                Toggle::new(hint.binding()).label("Dica nos controles"),
                Toggle::new(read_only.binding()).label("Só leitura"),
                Toggle::new(wheel.binding()).label("Roda move o diagrama"),
            ],
        );
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new("Opções").style(TextStyle::Headline),
                options,
                HStack::new((
                    SegmentedControl::new(grid.binding(), ["Grade 12", "Grade 24", "Grade 48"]),
                    SegmentedControl::new(layout.binding(), ["Da esquerda", "Mapa mental"]),
                    Button::new("Chegar um nó").on_click(move || {
                        a.set(Some(
                            FlowNode::new("", "step", "Nó que chegou", 0.0, 0.0).port(FlowPort::input("in")),
                        ));
                    }),
                ))
                .spacing(12.0)
                .wrap(),
                canvas,
                Text::new("Eventos").style(TextStyle::Headline),
                Text::new(said.get()),
                Text::new(format!("Nós no diagrama: {}", graph.get().nodes.len())),
                Text::new(format!(
                    "Selecionados: {}",
                    selection.get().nodes.len() + selection.get().edges.len()
                )),
            ))
            .spacing(12.0)
            .padding(16.0)
            .align(Align::Start),
        )
    }
}
