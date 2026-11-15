//! An automation builder for chat conversations, made only with `FlowCanvas`, `FlowToolbox`
//! and `FlowInspector`: a trigger starts the conversation, messages offer buttons, each
//! button leads somewhere, conditions split the way and actions reach other systems.
//!
//! It validates the component as a builder of this kind: everything here is the library's,
//! and what it could not do is written in the README.

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
#[derive(Debug, Default)]
pub struct Builder;

impl View for Builder {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let graph = cx.use_state(sample);
        let selection = cx.use_state(FlowSelection::default);
        let history = cx.use_state(FlowHistory::new);
        let arrivals = cx.use_state(|| None::<FlowNode>);
        let mut canvas = FlowCanvas::new(graph.binding())
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
        cx.child(
            &HStack::new((toolbox().title("Blocos").arrivals(arrivals.binding()), canvas, inspector))
                .align(Align::Stretch)
                .grow(1.0),
        )
    }
}

/// The whole app.
pub fn app() -> Builder {
    Builder
}
