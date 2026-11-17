//! What each block of a scenario does, and running it with `ravenrs-e2e`.
//!
//! A block's kind says the action and its subtitle says the target and the text, as a person
//! writes them: `#valor = 152,40` types into the field tagged `valor`, `Salvar` clicks what
//! shows "Salvar". Each block becomes a command of the test channel (`ravenrs-e2e-wire`), run on
//! an app compiled into the Studio or on one opened apart in a child process.

use ravenrs::prelude::*;
use ravenrs_e2e::E2e;
pub use ravenrs_e2e_runner::session::{Failure, Opener, Outcome, Session, Step, perform, summary};
use ravenrs_e2e_wire::{Block, Kind};

use crate::sample;

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
    Step::of(&Block { kind, target: aim.to_owned(), text, title: node.title.clone() })
}

/// Opens the sample form, the only app this module knows by itself.
pub fn open_sample(name: &str) -> Option<Result<Session, String>> {
    (name.is_empty() || name == sample::EXPENSE_APP)
        .then(|| Ok(Session::Here(Box::new(E2e::new(sample::expense_app(), Theme::raven_light())))))
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
