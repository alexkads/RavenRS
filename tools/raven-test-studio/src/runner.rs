//! What each block of a scenario does, and running it with `ravenrs-e2e`.
//!
//! A block's kind says the action and its subtitle says the target and the text, as a person
//! writes them: `#valor = 152,40` types into the field tagged `valor`, `Salvar` clicks what
//! shows "Salvar". Each block becomes a command of the test channel (`ravenrs-e2e-wire`), run on
//! an app compiled into the Studio or on one opened apart in a child process.

use ravenrs::prelude::*;
use ravenrs_e2e::E2e;
use ravenrs_e2e_wire::{Block, Client, Command, Kind};

use crate::sample;

/// The app a run drives: compiled into the Studio, or opened apart in a child process with no
/// window and driven over the test channel (`ravenrs-e2e-wire`). A step runs the same either way.
#[derive(Debug)]
pub enum Session {
    /// An app compiled into the Studio, such as the sample form or another Studio.
    Here(Box<E2e>),
    /// An app opened in a child process, such as Raven Finanças; it ends with the run.
    Apart(Client),
}

impl Session {
    /// Runs `command` on the app, as `Command::apply` answers.
    pub fn run(&mut self, command: &Command) -> Result<Option<String>, ravenrs_e2e_wire::Failure> {
        match self {
            Self::Here(e2e) => command.apply(e2e),
            Self::Apart(client) => client.send(command),
        }
    }

    /// The screen of the app as text, when it can still say.
    pub fn screen(&mut self) -> Option<String> {
        self.run(&Command::Snapshot).ok().flatten()
    }
}

/// Opens the app a scenario's Open block names: `None` when there is no app by that name, the
/// reason when there is one and it could not be opened.
pub type Opener = fn(&str) -> Option<Result<Session, String>>;

/// One step a block stands for.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// Opens the app under test by name, from nothing.
    Open(String),
    /// Anything done on the app once it is open: click, type, check or mention, with its target
    /// written as on the block (`#id` or the text shown).
    Do(Command),
}

/// What `node` stands for, or why it cannot run.
pub fn step_of(node: &FlowNode) -> Result<Step, String> {
    let line = node.subtitle.as_deref().unwrap_or_default().trim();
    let (aim, text) = match line.split_once('=') {
        Some((aim, text)) => (aim.trim(), Some(text.trim().to_owned())),
        None => (line, None),
    };
    let Some(kind) = Kind::read(&node.kind) else {
        return Err(format!("Bloco desconhecido: {}", node.kind));
    };
    if kind == Kind::Open {
        return Ok(Step::Open(aim.to_owned()));
    }
    if aim.is_empty() {
        return Err("Diga o alvo: #id ou o texto que aparece".to_owned());
    }
    let block = Block { kind, target: aim.to_owned(), text: text.clone(), title: node.title.clone() };
    let needs_text = matches!(kind, Kind::Type | Kind::Check | Kind::Mention | Kind::Fill | Kind::Select);
    if needs_text && text.is_none() {
        return Err("Diga o texto depois do alvo: #alvo = texto".to_owned());
    }
    let Some(command) = block.command() else {
        return Err(format!("Bloco desconhecido: {}", node.kind));
    };
    Ok(Step::Do(command))
}

/// Opens the sample form, the only app this module knows by itself.
pub fn open_sample(name: &str) -> Option<Result<Session, String>> {
    (name.is_empty() || name == sample::EXPENSE_APP)
        .then(|| Ok(Session::Here(Box::new(E2e::new(sample::expense_app(), Theme::raven_light())))))
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
    /// The screen of the app when it failed, when the failure brought it.
    pub screen: Option<String>,
}

impl Failure {
    fn plain(message: impl Into<String>) -> Self {
        Self { message: message.into(), ..Self::default() }
    }
}

/// What a failure means and how to fix it, in Portuguese, by its stable code. The detail from
/// `ravenrs-e2e` (which query, what was found) follows the meaning.
fn in_portuguese(code: &str) -> (&'static str, Option<&'static str>) {
    match code {
        "E2E_NOT_FOUND" => (
            "nada na tela corresponde ao alvo",
            Some("confira a tela do app testado; uma parte sem nome pode ganhar um ViewExt::test_id"),
        ),
        "E2E_AMBIGUOUS" => (
            "mais de uma parte corresponde ao alvo",
            Some("dê um test_id à parte, restrinja com within ou escolha uma com nth"),
        ),
        "E2E_DISABLED" => (
            "o alvo está desabilitado",
            Some("faça antes o que o habilita, ou confira que está desabilitado"),
        ),
        "E2E_COVERED" => {
            ("outra coisa está por cima do alvo", Some("feche ou mova antes o que está por cima"))
        }
        "E2E_NOT_VISIBLE" => ("o alvo está fora da janela", Some("role até ele ou aumente a janela")),
        "E2E_PROTECTED_VALUE" => {
            ("o valor é secreto e nunca é mostrado", Some("confira o que o app faz com o valor, como entrar"))
        }
        "E2E_EXPECTATION" => ("a conferência não bateu", None),
        "E2E_APP_START" => (
            "o app testado não abriu ou fechou no meio",
            Some(
                "veja no fim da mensagem o que o app escreveu ao sair; confira que ele foi compilado com o canal de teste",
            ),
        ),
        "E2E_TIMEOUT" => (
            "o app testado não respondeu a tempo e foi encerrado",
            Some("confira se o passo abre algo que espera por quem usa, como um diálogo do sistema"),
        ),
        "E2E_WIRE" => ("o app testado respondeu fora do protocolo do canal de teste", None),
        _ => ("o passo falhou", None),
    }
}

impl From<ravenrs_e2e_wire::Failure> for Failure {
    fn from(f: ravenrs_e2e_wire::Failure) -> Self {
        let (meaning, fix) = in_portuguese(&f.code);
        Self {
            message: format!("[{}] {meaning}: {}", f.code, f.message),
            meaning: Some(meaning.to_owned()),
            detail: Some(f.message),
            fix: fix.map(str::to_owned).or(f.fix),
            close: f.close,
            screen: (!f.screen.is_empty()).then_some(f.screen),
            code: Some(f.code),
        }
    }
}

/// Runs `step` on the app in `session`, opening apps with `open`.
pub fn perform(session: &mut Option<Session>, step: &Step, open: Opener) -> Result<(), Box<Failure>> {
    let command = match step {
        Step::Open(name) => {
            // The app a previous Open block started ends before the next one starts.
            *session = None;
            let opened = open(name)
                .ok_or_else(|| Box::new(Failure::plain(format!("Não há app chamado {name:?} para abrir"))))?;
            *session = Some(opened.map_err(|reason| Box::new(Failure::plain(reason)))?);
            return Ok(());
        }
        Step::Do(command) => command,
    };
    let Some(app) = session.as_mut() else {
        return Err(Box::new(Failure::plain("O app não está aberto: comece o cenário por um bloco Abrir")));
    };
    app.run(command).map(|_| ()).map_err(|f| Box::new(Failure::from(f)))
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
    session: Option<Session>,
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
    /// How long the block took, in milliseconds, waits included.
    pub ms: u64,
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
        let started = std::time::Instant::now();
        let result = step_of(node)
            .map_err(|m| Box::new(Failure::plain(m)))
            .and_then(|step| perform(&mut self.session, &step, self.open));
        let ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
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
        let failed = result.is_err();
        let f = result.err().map(|f| *f).unwrap_or_default();
        // The failure brings the screen as it was; when it did not, the app is asked for it.
        let screen = match &f.screen {
            Some(screen) => Some(screen.clone()),
            None if failed => self.session.as_mut().and_then(Session::screen),
            None => None,
        };
        let error = (!f.message.is_empty()).then_some(f.message);
        if self.finished(graph) {
            // An app opened apart ends with the run, not when the next one starts.
            self.session = None;
        }
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
            ms,
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
