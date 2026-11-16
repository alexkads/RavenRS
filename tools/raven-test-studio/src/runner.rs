//! What each block of a scenario does, and running it with `ravenrs-e2e`.
//!
//! A block's kind says the action and its subtitle says the target and the text, as a person
//! writes them: `#valor = 152,40` types into the field tagged `valor`, `Salvar` clicks what
//! shows "Salvar".

use ravenrs::prelude::*;
use ravenrs_e2e::{E2e, Query};

use crate::sample;

/// Opens the app a scenario's Open block names, or says there is none by that name.
pub type Opener = fn(&str) -> Option<E2e>;

/// One step a block stands for.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// Opens the app under test by name, from nothing.
    Open(String),
    /// Clicks the target.
    Click(Query),
    /// Types the text into the target.
    Type(Query, String),
    /// The target's value, or its text when it has no value, is exactly the text.
    Check(Query, String),
    /// The target's name or description contains the text, such as the state of a diagram
    /// block ("Concluído", an error code).
    Mention(Query, String),
}

/// The target of a block: `#id` is a test id, anything else is the text shown.
fn target(text: &str) -> Query {
    match text.strip_prefix('#') {
        Some(id) => Query::test_id(id.trim()),
        None => Query::text(text),
    }
}

/// What `node` stands for, or why it cannot run.
pub fn step_of(node: &FlowNode) -> Result<Step, String> {
    let line = node.subtitle.as_deref().unwrap_or_default().trim();
    let (aim, text) = match line.split_once('=') {
        Some((aim, text)) => (aim.trim(), Some(text.trim().to_owned())),
        None => (line, None),
    };
    let needs_aim = || {
        if aim.is_empty() {
            Err("Diga o alvo: #id ou o texto que aparece".to_owned())
        } else {
            Ok(target(aim))
        }
    };
    let needs_text = || text.clone().ok_or_else(|| "Diga o texto depois do alvo: #alvo = texto".to_owned());
    match node.kind.as_str() {
        "open" => Ok(Step::Open(aim.to_owned())),
        "click" => Ok(Step::Click(needs_aim()?)),
        "type" => Ok(Step::Type(needs_aim()?, needs_text()?)),
        "check" => Ok(Step::Check(needs_aim()?, needs_text()?)),
        "mention" => Ok(Step::Mention(needs_aim()?, needs_text()?)),
        other => Err(format!("Bloco desconhecido: {other}")),
    }
}

/// Opens the sample form, the only app this module knows by itself.
pub fn open_sample(name: &str) -> Option<E2e> {
    (name.is_empty() || name == sample::EXPENSE_APP)
        .then(|| E2e::new(sample::expense_app(), Theme::raven_light()))
}

/// Why a block failed: the message, and from `ravenrs-e2e` its stable code, how to fix it and
/// what came close on the screen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Failure {
    /// What went wrong, starting with the code when there is one: `[E2E_NOT_FOUND] …`.
    pub message: String,
    /// The stable code, such as `E2E_NOT_FOUND`.
    pub code: Option<String>,
    /// What the failure means, in Portuguese, from its code.
    pub meaning: Option<String>,
    /// The detail `ravenrs-e2e` gave: which query, what it found.
    pub detail: Option<String>,
    /// How to fix it, when there is an obvious way.
    pub fix: Option<String>,
    /// Up to three nodes that came close to what the block asked for.
    pub close: Vec<String>,
}

impl Failure {
    fn plain(message: impl Into<String>) -> Self {
        Self { message: message.into(), ..Self::default() }
    }
}

/// What a failure means and how to fix it, in Portuguese, by its stable code. The detail from
/// `ravenrs-e2e` (which query, what was found) follows the meaning.
fn in_portuguese(code: ravenrs_e2e::ErrorCode) -> (&'static str, Option<&'static str>) {
    use ravenrs_e2e::ErrorCode as C;
    match code {
        C::NotFound => (
            "nada na tela corresponde ao alvo",
            Some("confira a tela do app testado; uma parte sem nome pode ganhar um ViewExt::test_id"),
        ),
        C::Ambiguous => (
            "mais de uma parte corresponde ao alvo",
            Some("dê um test_id à parte, restrinja com within ou escolha uma com nth"),
        ),
        C::Disabled => (
            "o alvo está desabilitado",
            Some("faça antes o que o habilita, ou confira que está desabilitado"),
        ),
        C::Covered => ("outra coisa está por cima do alvo", Some("feche ou mova antes o que está por cima")),
        C::NotVisible => ("o alvo está fora da janela", Some("role até ele ou aumente a janela")),
        C::ProtectedValue => {
            ("o valor é secreto e nunca é mostrado", Some("confira o que o app faz com o valor, como entrar"))
        }
        C::Expectation => ("a conferência não bateu", None),
        _ => ("o passo falhou", None),
    }
}

impl From<ravenrs_e2e::E2eError> for Failure {
    fn from(e: ravenrs_e2e::E2eError) -> Self {
        let (meaning, fix) = in_portuguese(e.code());
        Self {
            message: format!("[{}] {meaning}: {}", e.code(), e.message()),
            code: Some(e.code().as_str().to_owned()),
            meaning: Some(meaning.to_owned()),
            detail: Some(e.message().to_owned()),
            fix: fix.map(str::to_owned).or_else(|| e.fix().map(str::to_owned)),
            close: e.close_matches().to_vec(),
        }
    }
}

/// Runs `step` on the app in `session`, opening apps with `open`.
pub fn perform(session: &mut Option<E2e>, step: &Step, open: Opener) -> Result<(), Box<Failure>> {
    if let Step::Open(name) = step {
        let opened = open(name)
            .ok_or_else(|| Box::new(Failure::plain(format!("Não há app chamado {name:?} para abrir"))))?;
        *session = Some(opened);
        return Ok(());
    }
    let Some(e2e) = session.as_mut() else {
        return Err(Box::new(Failure::plain("O app não está aberto: comece o cenário por um bloco Abrir")));
    };
    let done = match step {
        Step::Open(_) => Ok(()),
        Step::Click(q) => e2e.locate(q.clone()).click(),
        Step::Type(q, text) => e2e.locate(q.clone()).type_text(text),
        Step::Check(q, text) => match e2e.find(q) {
            Ok(node) if node.value.is_some() => e2e.locate(q.clone()).expect().to_have_value(text),
            Ok(_) => e2e.locate(q.clone()).expect().to_have_name(text),
            Err(e) => Err(e),
        },
        Step::Mention(q, text) => e2e.locate(q.clone()).expect().to_mention(text),
    };
    done.map_err(|e| Box::new(Failure::from(e)))
}

/// The blocks a run goes through: from the first Open block, along the first way out of each.
pub fn path(graph: &FlowGraph) -> Vec<String> {
    let mut path: Vec<String> = Vec::new();
    let Some(mut at) = graph.nodes.iter().find(|n| n.kind == "open").map(|n| n.id.clone()) else {
        return path;
    };
    while !path.contains(&at) {
        path.push(at.clone());
        match graph.edges.iter().find(|e| e.from.node == at) {
            Some(edge) => at = edge.to.node.clone(),
            None => break,
        }
    }
    path
}

/// A run in progress: the app it drives and the next block on its path.
#[derive(Debug)]
pub struct Run {
    session: Option<E2e>,
    next: usize,
    failed: bool,
    open: Opener,
}

impl Default for Run {
    fn default() -> Self {
        Self::new(open_sample)
    }
}

/// What a step of a run did, for the log and for whoever called the Studio.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    /// The block's id in the scenario.
    pub id: String,
    /// The block's kind: `open`, `click`, `type`, `check` or `mention`.
    pub kind: String,
    /// The block's title.
    pub title: String,
    /// The block's target and text, as written on it.
    pub line: String,
    /// `None` when it passed, else what went wrong, starting with its code when it has one.
    pub error: Option<String>,
    /// The stable code of the failure, such as `E2E_NOT_FOUND`, when it has one.
    pub code: Option<String>,
    /// What the failure means, in Portuguese, from its code.
    pub meaning: Option<String>,
    /// The detail `ravenrs-e2e` gave: which query, what it found.
    pub detail: Option<String>,
    /// How to fix the failure, when there is an obvious way.
    pub fix: Option<String>,
    /// Up to three nodes that came close to what the block asked for.
    pub close: Vec<String>,
    /// The screen of the app under test when the block failed, as `E2e::snapshot` writes it.
    pub screen: Option<String>,
}

impl Run {
    /// A run that opens apps with `open`.
    pub fn new(open: Opener) -> Self {
        Self { session: None, next: 0, failed: false, open }
    }

    /// A run with nothing left to do, as one whose result came from the cache.
    pub fn finished_with(open: Opener) -> Self {
        Self { session: None, next: usize::MAX, failed: false, open }
    }

    /// Whether the run has nothing left to do: it reached the end or a block failed.
    pub fn finished(&self, graph: &FlowGraph) -> bool {
        self.failed || self.next >= path(graph).len()
    }

    /// Runs the next block, marks it in `graph` and says how it went; `None` when finished.
    pub fn step(&mut self, graph: &mut FlowGraph) -> Option<Outcome> {
        if self.finished(graph) {
            return None;
        }
        let ids = path(graph);
        let id = ids.get(self.next)?.clone();
        self.next += 1;
        let node = graph.nodes.iter_mut().find(|n| n.id == id)?;
        let result = step_of(node)
            .map_err(|m| Box::new(Failure::plain(m)))
            .and_then(|step| perform(&mut self.session, &step, self.open));
        let (title, kind, line) =
            (node.title.clone(), node.kind.clone(), node.subtitle.clone().unwrap_or_default());
        node.status = match &result {
            Ok(()) => FlowNodeStatus::Success,
            Err(e) => FlowNodeStatus::Error(e.message.clone()),
        };
        if result.is_err() {
            self.failed = true;
            // What comes after a failed block does not run.
            for later in &ids[self.next..] {
                if let Some(n) = graph.nodes.iter_mut().find(|n| n.id == *later) {
                    n.status = FlowNodeStatus::Skipped;
                }
            }
        }
        let failure = result.err();
        let screen = failure.as_ref().and_then(|_| self.session.as_ref().map(E2e::snapshot));
        let f = failure.map(|f| *f).unwrap_or_default();
        let error = (!f.message.is_empty()).then_some(f.message);
        Some(Outcome {
            id,
            kind,
            title,
            line,
            error,
            code: f.code,
            meaning: f.meaning,
            detail: f.detail,
            fix: f.fix,
            close: f.close,
            screen,
        })
    }

    /// Runs every block left, and says how each went.
    pub fn to_end(&mut self, graph: &mut FlowGraph) -> Vec<Outcome> {
        let mut outcomes = Vec::new();
        while let Some(outcome) = self.step(graph) {
            outcomes.push(outcome);
        }
        outcomes
    }
}

/// `graph` marked as a run that went as `outcomes` says: done, failed with its error, and
/// what came after a failure as not run. It is how a run from the cache shows.
pub fn applied(graph: &FlowGraph, outcomes: &[Outcome]) -> FlowGraph {
    let mut next = cleared(graph);
    let ids = path(graph);
    let failed_at = outcomes.iter().position(|o| o.error.is_some());
    for (i, id) in ids.iter().enumerate() {
        let Some(node) = next.nodes.iter_mut().find(|n| n.id == *id) else { continue };
        node.status = match (outcomes.get(i), failed_at) {
            (Some(Outcome { error: None, .. }), _) => FlowNodeStatus::Success,
            (Some(Outcome { error: Some(e), .. }), _) => FlowNodeStatus::Error(e.clone()),
            (None, Some(_)) => FlowNodeStatus::Skipped,
            (None, None) => FlowNodeStatus::Normal,
        };
    }
    next
}

/// `graph` with every block back to how it was before a run.
pub fn cleared(graph: &FlowGraph) -> FlowGraph {
    let mut next = graph.clone();
    for node in &mut next.nodes {
        node.status = FlowNodeStatus::Normal;
    }
    next
}

/// One line that sums up a run: how many blocks passed out of the path.
pub fn summary(outcomes: &[Outcome], total: usize) -> String {
    let passed = outcomes.iter().filter(|o| o.error.is_none()).count();
    match outcomes.iter().position(|o| o.error.is_some()) {
        Some(i) => format!("Falhou no passo {} de {total}", i + 1),
        None if passed == total => format!("{passed} de {total} passos passaram"),
        None => format!("{passed} de {total} passos até agora"),
    }
}
