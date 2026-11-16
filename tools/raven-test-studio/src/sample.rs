//! The apps the first scenarios run against, and the scenarios that come with the Studio.

use ravenrs::prelude::*;

/// The name an Open block gives to open the sample form.
pub const EXPENSE_APP: &str = "Novo lançamento";
/// The name an Open block gives to open another Studio, with the sample scenario.
pub const STUDIO_APP: &str = "Raven Test Studio";
/// The name an Open block gives to open another Studio whose scenario fails on purpose.
pub const BROKEN_STUDIO_APP: &str = "Raven Test Studio com cenário quebrado";

/// A small form, "Novo lançamento", with every part tagged for tests: what a scenario drives
/// until the Studio can open the person's own app in another process.
pub fn expense_app() -> impl View {
    ExpenseApp
}

struct ExpenseApp;

impl View for ExpenseApp {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let description = cx.use_state(String::new);
        let amount = cx.use_state(String::new);
        let saved = cx.use_state(String::new);
        let (d, a, s) = (description.clone(), amount.clone(), saved.clone());
        cx.child(
            &VStack::new((
                Text::new("Novo lançamento").style(TextStyle::Title),
                FormField::new("Descrição", TextField::new(description.binding()).test_id("descricao")),
                FormField::new("Valor", TextField::new(amount.binding()).test_id("valor")),
                Button::new("Salvar")
                    .on_click(move || s.set(format!("Salvo: {}, R$ {}", d.get(), a.get())))
                    .test_id("salvar"),
                Text::new(saved.get()).test_id("resultado"),
            ))
            .spacing(12.0)
            .padding(24.0),
        )
    }
}

/// A block of a scenario: `kind` says the action, `line` the target and the text.
pub fn block(id: &str, kind: &str, title: &str, line: &str) -> FlowNode {
    let node = FlowNode::new(id, kind, title, 0.0, 0.0).subtitle(line);
    let node = if kind == "open" { node } else { node.port(FlowPort::input("in")) };
    node.port(FlowPort::output("out"))
}

fn at(node: FlowNode, x: f32, y: f32) -> FlowNode {
    FlowNode { x, y, ..node }
}

/// Joins block `from` to block `to`.
pub fn link(id: &str, from: &str, to: &str) -> FlowEdge {
    FlowEdge::new(id, FlowEndpoint::new(from, "out"), FlowEndpoint::new(to, "in"))
}

/// The blocks joined one after the other, four to a row.
fn chain(blocks: Vec<FlowNode>) -> FlowGraph {
    let ids: Vec<String> = blocks.iter().map(|b| b.id.clone()).collect();
    let mut graph = FlowGraph::new();
    for (i, node) in blocks.into_iter().enumerate() {
        graph = graph.node(at(node, (i % 4) as f32 * 288.0, (i / 4) as f32 * 144.0));
    }
    for (i, pair) in ids.windows(2).enumerate() {
        graph = graph.edge(link(&format!("e{}", i + 1), &pair[0], &pair[1]));
    }
    graph
}

/// The scenario a sample Studio opens with: fill in an expense, save it, check what it says.
pub fn scenario() -> FlowGraph {
    chain(vec![
        block("open", "open", "Abrir o app", EXPENSE_APP),
        block("description", "type", "Digitar a descrição", "#descricao = Mercado"),
        block("amount", "type", "Digitar o valor", "#valor = 152,40"),
        block("save", "click", "Salvar", "#salvar"),
        block("check", "check", "Conferir o resultado", "#resultado = Salvo: Mercado, R$ 152,40"),
    ])
}

/// A scenario that fails on purpose at its second block, for the Studio to show a failure.
pub fn broken_scenario() -> FlowGraph {
    chain(vec![
        block("open", "open", "Abrir o app", EXPENSE_APP),
        block("lost", "click", "Clicar no que não existe", "#nao-existe"),
        block("after", "click", "Salvar", "#salvar"),
    ])
}

/// The Studio testing the Studio: it opens another Studio and checks that running,
/// stepping, stopping and failing all show as they should.
pub fn self_test() -> FlowGraph {
    chain(vec![
        block("open", "open", "Abrir outro Studio", STUDIO_APP),
        block("idle", "check", "Ainda não rodou", "#summary = Ainda não rodou"),
        block("run", "click", "Rodar o cenário dele", "#run"),
        block("passed", "check", "Os cinco passos passaram", "#summary = 5 de 5 passos passaram"),
        block("first-done", "mention", "O primeiro bloco concluiu", "#node:open = Concluído"),
        block("last-done", "mention", "O último bloco concluiu", "#node:check = Concluído"),
        block("log", "check", "O registro diz o último passo", "#log-5 = 5. Conferir o resultado: passou"),
        block("stop", "click", "Parar", "#stop"),
        block("reset", "check", "Voltou ao início", "#summary = Ainda não rodou"),
        block("step-1", "click", "Próximo passo", "#next"),
        block("step-2", "click", "Próximo passo de novo", "#next"),
        block("stepped", "check", "Dois passos até agora", "#summary = 2 de 5 passos até agora"),
        block("second-done", "mention", "O segundo bloco concluiu", "#node:description = Concluído"),
        block("broken", "open", "Abrir um Studio com cenário quebrado", BROKEN_STUDIO_APP),
        block("run-broken", "click", "Rodar o cenário quebrado", "#run"),
        block("failed", "check", "Falhou no passo 2", "#summary = Falhou no passo 2 de 3"),
        block("why", "mention", "O bloco diz por que falhou", "#node:lost = E2E_NOT_FOUND"),
        block("skipped", "mention", "O bloco seguinte não rodou", "#node:after = Não rodou"),
    ])
}

/// The blocks to drag in, by group.
pub fn toolbox() -> FlowToolbox {
    FlowToolbox::new()
        .group("Início")
        .model("Abrir o formulário", block("open", "open", "Abrir o app", EXPENSE_APP))
        .model("Abrir outro Studio", block("open", "open", "Abrir outro Studio", STUDIO_APP))
        .group("Agir")
        .model("Clicar", block("click", "click", "Clicar", "#alvo"))
        .model("Digitar", block("type", "type", "Digitar", "#alvo = texto"))
        .group("Conferir")
        .model("Conferir texto", block("check", "check", "Conferir", "#alvo = texto"))
        .model("Conferir que menciona", block("mention", "mention", "Conferir que menciona", "#alvo = texto"))
}
