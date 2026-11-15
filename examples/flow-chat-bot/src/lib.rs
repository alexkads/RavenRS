//! An automation builder for chat conversations, made only with `FlowCanvas`, `FlowToolbox`
//! and `FlowInspector`: a trigger starts the conversation, messages offer buttons, each
//! button leads somewhere, conditions split the way and actions reach other systems.
//!
//! It validates the component as a builder of this kind: everything here is the library's,
//! and what it could not do is written in the README.

use std::rc::Rc;

use ravenrs::core::{MemoryStorage, Storage};
use ravenrs::prelude::*;

/// The kinds of block, with the icon and the word the canvas shows on each.
pub const KINDS: [(&str, &str); 4] =
    [("trigger", "Gatilho"), ("message", "Mensagem"), ("logic", "Lógica"), ("action", "Ação")];

fn trigger(id: &str, title: &str, line: &str) -> FlowNode {
    FlowNode::new(id, "trigger", title, 0.0, 0.0).subtitle(line).port(FlowPort::output("out"))
}

fn message(id: &str, title: &str, text: &str) -> FlowNode {
    FlowNode::new(id, "message", title, 0.0, 0.0)
        .subtitle(text)
        .port(FlowPort::input("in"))
        .port(FlowPort::output("out"))
}

/// A message with a button per answer: each answer is an output of its own, so each leads
/// somewhere else.
fn buttons(id: &str, title: &str, answers: &[&str]) -> FlowNode {
    let mut node = FlowNode::new(id, "message", title, 0.0, 0.0)
        .size(240.0, 64.0 + 24.0 * answers.len() as f32)
        .port(FlowPort::input("in"));
    for answer in answers {
        node = node.port(FlowPort::output(answer.to_lowercase()).label(*answer));
    }
    node
}

fn condition(id: &str, title: &str) -> FlowNode {
    FlowNode::new(id, "logic", title, 0.0, 0.0)
        .shape(FlowShape::Diamond)
        .size(176.0, 104.0)
        .port(FlowPort::input("in").side(PortSide::Left))
        .port(FlowPort::output("sim").label("Sim").side(PortSide::Right))
        .port(FlowPort::output("não").label("Não").side(PortSide::Bottom))
}

fn action(id: &str, title: &str, line: &str) -> FlowNode {
    FlowNode::new(id, "action", title, 0.0, 0.0).subtitle(line).port(FlowPort::input("in"))
}

fn at(node: FlowNode, x: f32, y: f32) -> FlowNode {
    FlowNode { x, y, ..node }
}

fn link(id: &str, from: (&str, &str), to: &str) -> FlowEdge {
    FlowEdge::new(id, FlowEndpoint::new(from.0, from.1), FlowEndpoint::new(to, "in"))
}

/// A conversation that welcomes, asks what the person wants and takes each answer its way.
pub fn sample() -> FlowGraph {
    FlowGraph::new()
        .node(at(trigger("start", "Palavra-chave", "“oi”, “olá”, “menu”"), 0.0, 120.0))
        .node(at(buttons("menu", "Boas-vindas", &["Pedidos", "Horários", "Atendente"]), 288.0, 96.0))
        .node(at(message("hours", "Horários", "Seg a sex, 8h às 18h"), 600.0, 0.0))
        .node(at(condition("open", "Dentro do horário?"), 600.0, 168.0))
        .node(at(action("human", "Avisar a equipe", "Canal #atendimento"), 888.0, 168.0))
        .node(at(message("later", "Fora do horário", "Respondemos amanhã cedo"), 600.0, 360.0))
        .node(at(action("orders", "Consultar pedidos", "API da loja"), 600.0, -168.0))
        .edge(link("e1", ("start", "out"), "menu"))
        .edge(link("e2", ("menu", "pedidos"), "orders"))
        .edge(link("e3", ("menu", "horários"), "hours"))
        .edge(link("e4", ("menu", "atendente"), "open"))
        .edge(link("e5", ("open", "sim"), "human"))
        .edge(link("e6", ("open", "não"), "later"))
}

/// The blocks to drag in, by group.
pub fn toolbox() -> FlowToolbox {
    FlowToolbox::new()
        .group("Gatilhos")
        .model("Palavra-chave", trigger("keyword", "Palavra-chave", "Quando a pessoa escreve…"))
        .model("Novo contato", trigger("contact", "Novo contato", "Primeira mensagem"))
        .group("Mensagens")
        .model("Texto", message("text", "Mensagem", "Escreva a mensagem"))
        .model("Botões", buttons("buttons", "Pergunta", &["Sim", "Não"]))
        .model("Imagem", message("image", "Imagem", "Escolha o arquivo"))
        .group("Lógica")
        .model("Condição", condition("condition", "Condição"))
        .model(
            "Espera",
            FlowNode::new("wait", "logic", "Esperar", 0.0, 0.0)
                .subtitle("1 hora")
                .port(FlowPort::input("in"))
                .port(FlowPort::output("out")),
        )
        .group("Ações")
        .model("Etiqueta", action("tag", "Pôr etiqueta", "Cliente"))
        .model("Webhook", action("webhook", "Chamar sistema", "https://…"))
        .model("Avisar equipe", action("notify", "Avisar a equipe", "Canal"))
}

/// What comes after a block made with the arrow or Cmd/Ctrl+Enter: a message after anything
/// but an action, which ends the way.
pub fn next(from: &FlowNode, _side: PortSide) -> Option<FlowNode> {
    (from.kind != "action").then(|| message("text", "Mensagem", "Escreva a mensagem"))
}

/// Whether a block may be joined to another: nothing goes into a trigger, and an action ends
/// the way.
pub fn validate(graph: &FlowGraph, from: &FlowEndpoint, to: &FlowEndpoint) -> Result<(), String> {
    let kind = |id: &str| graph.find_node(id).map(|n| n.kind.clone()).unwrap_or_default();
    if kind(&to.node) == "trigger" {
        return Err("Um gatilho começa a conversa: nada chega nele".into());
    }
    if kind(&from.node) == "action" {
        return Err("Uma ação termina o caminho".into());
    }
    Ok(())
}

/// The way a test run takes from the first trigger: each block and the connection it left
/// by, its first way out (the first answer of a question, "yes" of a condition).
pub fn run_path(graph: &FlowGraph) -> Vec<(String, Option<String>)> {
    let mut path = Vec::new();
    let Some(mut at) = graph.nodes.iter().find(|n| n.kind == "trigger").map(|n| n.id.clone()) else {
        return path;
    };
    while !path.iter().any(|(id, _): &(String, Option<String>)| *id == at) {
        let out = graph.edges.iter().find(|e| e.from.node == at);
        path.push((at.clone(), out.map(|e| e.id.clone())));
        match out {
            Some(edge) => at = edge.to.node.clone(),
            None => break,
        }
    }
    path
}

/// The diagram as a test run shows it at `step` of `path`: the blocks before it done, the
/// one at it running, the connections taken moving; `None` puts every block back.
pub fn marked(graph: &FlowGraph, path: &[(String, Option<String>)], step: Option<usize>) -> FlowGraph {
    let mut next = graph.clone();
    for node in &mut next.nodes {
        if matches!(node.status, FlowNodeStatus::Running | FlowNodeStatus::Success) {
            node.status = FlowNodeStatus::Normal;
        }
    }
    for edge in &mut next.edges {
        edge.animated = false;
    }
    let Some(step) = step else { return next };
    for (i, (id, left_by)) in path.iter().enumerate().take(step + 1) {
        if let Some(node) = next.nodes.iter_mut().find(|n| n.id == *id) {
            node.status = if i == step { FlowNodeStatus::Running } else { FlowNodeStatus::Success };
        }
        if i < step
            && let Some(edge) = left_by.as_ref().and_then(|e| next.edges.iter_mut().find(|x| x.id == *e))
        {
            edge.animated = true;
        }
    }
    next
}

fn icon(kind: &str) -> IconName {
    match kind {
        "trigger" => IconName::Zap,
        "message" => IconName::MessageCircle,
        "logic" => IconName::Split,
        _ => IconName::Send,
    }
}

/// The builder: blocks on the left, the conversation in the middle, the selected block's
/// properties on the right.
pub struct Builder {
    /// Where the diagrams are kept between runs.
    storage: Rc<dyn Storage>,
}

impl std::fmt::Debug for Builder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Builder").finish_non_exhaustive()
    }
}

impl View for Builder {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let graph = cx.use_state(sample);
        let selection = cx.use_state(FlowSelection::default);
        let history = cx.use_state(FlowHistory::new);
        let arrivals = cx.use_state(|| None::<FlowNode>);
        let view = cx.use_state(FlowViewport::fit);
        let mut canvas = FlowCanvas::new(graph.binding())
            .view(view.binding())
            .label("Conversa")
            .selection(selection.binding())
            .history(history.binding())
            .arrivals(arrivals.binding())
            .validate(validate)
            .next_node(next)
            .on_export(ravenrs_flow_export::saver(cx.file_picker(), "Conversa"));
        let mut inspector =
            FlowInspector::new(graph.binding(), selection.binding()).history(history.binding());
        for (kind, caption) in KINDS {
            let tone = if kind == "trigger" { Semantic::Warning } else { Semantic::Accent };
            canvas = canvas.kind(kind, FlowKind::new(icon(kind), caption).tone(tone));
            inspector = inspector.kind(kind, caption);
        }
        // A test run: the trigger first, then a step at a time along the first way out.
        let step = cx.use_state(|| None::<usize>);
        let path = run_path(&graph.get());
        let (g1, s1, g2, s2, g3, s3) =
            (graph.clone(), step.clone(), graph.clone(), step.clone(), graph.clone(), step.clone());
        let last = path.len().saturating_sub(1);
        let (p1, p2, p3) = (path.clone(), path.clone(), path);
        let running = step.get().is_some();
        let bar = HStack::new((
            Button::new("Testar conversa").icon(IconName::Play).disabled(running).on_click(move || {
                s1.set(Some(0));
                g1.set(marked(&g1.get(), &p1, Some(0)));
            }),
            Button::new("Próximo passo")
                .style(ButtonStyle::Secondary)
                .disabled(!running || step.get() == Some(last))
                .on_click(move || {
                    let next = s2.get().map_or(0, |s| (s + 1).min(last));
                    s2.set(Some(next));
                    g2.set(marked(&g2.get(), &p2, Some(next)));
                }),
            Button::new("Parar").style(ButtonStyle::Ghost).disabled(!running).on_click(move || {
                s3.set(None);
                g3.set(marked(&g3.get(), &p3, None));
            }),
        ))
        .spacing(8.0)
        .padding(8.0);
        cx.child(
            &VStack::new((
                bar,
                HStack::new((
                    VStack::new((
                        FlowLibrary::new(graph.binding())
                            .title("Conversas")
                            .key("chats")
                            .history(history.binding())
                            .selection(selection.binding())
                            .view(view.binding())
                            .storage(self.storage.clone()),
                        toolbox().title("Blocos").arrivals(arrivals.binding()),
                    ))
                    .spacing(0.0)
                    .align(Align::Stretch),
                    canvas,
                    inspector,
                ))
                .align(Align::Stretch)
                .grow(1.0),
            ))
            .spacing(0.0)
            .align(Align::Stretch)
            .grow(1.0),
        )
    }
}

/// The whole app.
pub fn app() -> Localized<Builder> {
    app_kept_in(Rc::new(MemoryStorage::default()))
}

/// The whole app, keeping its diagrams in `storage`.
pub fn app_kept_in(storage: Rc<dyn Storage>) -> Localized<Builder> {
    Localized::new(Labels::pt_br(), Builder { storage })
}
