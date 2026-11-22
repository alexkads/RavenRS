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

/// The scenario the Studio opened by the self-test runs (and the same as
/// `tests/e2e/amostras/lancar-despesa.raventest`): it is part of the self-test, not a scenario for people.
///
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

/// Scenarios read from `.raventest` files, one after the other in a single flow: each starts
/// with its Open block, so a run opens each app anew.
pub fn from_scenarios(scenarios: &[ravenrs_e2e_wire::Scenario]) -> FlowGraph {
    let blocks = scenarios
        .iter()
        .enumerate()
        .flat_map(|(s, scenario)| {
            scenario.blocks.iter().enumerate().map(move |(b, block)| {
                let id = if scenarios.len() == 1 {
                    format!("b{}", b + 1)
                } else {
                    format!("s{}b{}", s + 1, b + 1)
                };
                self::block(&id, block.kind.word(), &block.title, &block.line())
            })
        })
        .collect();
    chain(blocks)
}

/// The scenario a flow stands for, as a `.raventest` file writes it: its blocks in the order
/// they run, read the way the runner reads them. `Err` says which block cannot be written.
pub fn scenario_of(graph: &FlowGraph, name: &str) -> Result<ravenrs_e2e_wire::Scenario, String> {
    use ravenrs_e2e_wire::{Block, Kind};
    let mut blocks = Vec::new();
    for id in crate::runner::path(graph) {
        let Some(node) = graph.nodes.iter().find(|n| n.id == id) else { continue };
        let kind = Kind::read(&node.kind)
            .ok_or_else(|| format!("{}: bloco desconhecido {}", node.title, node.kind))?;
        let line = node.subtitle.as_deref().unwrap_or_default().trim();
        let (target, text) = match line.split_once('=') {
            Some((aim, text)) if kind != Kind::Open => (aim.trim().to_owned(), Some(text.trim().to_owned())),
            _ => (line.to_owned(), None),
        };
        blocks.push(Block { kind, target, text, title: node.title.clone() });
    }
    if blocks.first().map(|b| b.kind) != Some(Kind::Open) {
        return Err("o cenário começa por um bloco Início, que abre o app".to_owned());
    }
    Ok(ravenrs_e2e_wire::Scenario { name: name.to_owned(), blocks })
}

/// Whether a scenario opens an app apart, one not compiled into the Studio, whose build the
/// Studio's cache knows nothing about.
pub fn opens_apart(graph: &FlowGraph) -> bool {
    graph.nodes.iter().any(|n| {
        let name = n.subtitle.as_deref().unwrap_or_default();
        n.kind == "open"
            && (crate::apps::learned(name)
                || !matches!(
                    n.subtitle.as_deref(),
                    Some(EXPENSE_APP | STUDIO_APP | BROKEN_STUDIO_APP) | None
                ))
    })
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
        block("log", "check", "O registro diz o último passo", "#log:5 = 5. Conferir o resultado: passou"),
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
        .model("Abrir um app", block("open", "open", "Abrir o app", "Nome do app"))
        .model(
            "Abrir o Raven Finanças",
            block("open", "open", "Abrir o Raven Finanças", crate::apps::FINANCE_APP),
        )
        .group("Agir")
        .model("Clicar", block("click", "click", "Clicar", "#alvo"))
        .model("Digitar", block("type", "type", "Digitar", "#alvo = texto"))
        .model("Preencher", block("fill", "fill", "Preencher", "#alvo = texto"))
        .model("Escolher", block("select", "select", "Escolher", "#alvo = opção"))
        .model("Expandir", block("expand", "expand", "Expandir", "#alvo"))
        .model("Recolher", block("collapse", "collapse", "Recolher", "#alvo"))
        .model("Definir valor", block("set-value", "set-value", "Definir valor", "#alvo = valor"))
        .model("Abrir o menu", block("show-menu", "show-menu", "Abrir o menu", "#alvo"))
        .model("Dispensar", block("dismiss", "dismiss", "Dispensar", "#alvo"))
        .model("Rolar até", block("scroll", "scroll", "Rolar até", "#alvo"))
        .model("Arrastar", block("drag", "drag", "Arrastar", "#de = #até"))
        .group("Conferir")
        .model("Esperar", block("wait", "wait", "Esperar", "#alvo = texto"))
        .model("Conferir texto", block("check", "check", "Conferir", "#alvo = texto"))
        .model("Conferir que menciona", block("mention", "mention", "Conferir que menciona", "#alvo = texto"))
}
