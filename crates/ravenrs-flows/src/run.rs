//! Running a plan in memory, one node after another in the plan's order: the whole flow, from a
//! node with what an earlier run left upstream, or one node alone. A loop goes round its body
//! while it sends items into it. A node is tried again when it fails in a way worth it, a run
//! can be cancelled from any thread, and time limits stop a node that takes too long. Same
//! flow, items and clock, same run. Above `plan`, `catalog` and `time`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::catalog::{
    Catalog, ERROR_PORT, Failure, Mode, NodeContext, NodeError, NodeKind, Reconciled, Wake,
};
use crate::definition::{ErrorPolicy, NodeDef, Retry};
use crate::item::{Item, Ports};
use crate::plan::Plan;
use crate::time::{CancelToken, Clock, SystemClock};

/// The most turns a loop takes, unless [`Executor::loop_limit`] says otherwise, before the run
/// says it goes round for ever. Each turn the loop node gets everything that came back so far,
/// so a loop of many turns costs more than its turns.
pub const LOOP_LIMIT: usize = 10_000;

/// How a node's part of a run went.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum NodeStatus {
    /// It ran.
    Done,
    /// It gave its pinned items, in a test, or the items the run was started with.
    Pinned,
    /// It is disabled and passed its first input on.
    Disabled,
    /// No item reached it, so it did not run.
    Skipped,
    /// It failed.
    Failed,
    /// It did not run again: what it gave is an earlier run's.
    Reused,
    /// It was cancelled while it ran.
    Cancelled,
    /// It took longer than its time limit, or than what was left of the run's.
    TimedOut,
    /// It called something and no one knows whether that happened. It was not tried again.
    Uncertain,
    /// It waits for a time or an event ([`NodeRun::wake`]); the run stopped there.
    Waiting,
}

/// A node's part of a run: what reached it, what it gave, and how it went. For a node inside
/// a loop, these are the last turn's.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeRun {
    /// How it went.
    pub status: NodeStatus,
    /// The items that reached each input.
    pub inputs: Ports,
    /// The items it gave on each output, placed and traced.
    pub outputs: Ports,
    /// Why it failed, when it did.
    pub error: Option<String>,
    /// How many times it ran in this run: more than once inside a loop.
    pub runs: usize,
    /// How many tries its last run took: more than 1 when it was tried again.
    pub attempts: u32,
    /// Whether its result came from asking the kind if a call of unknown result happened.
    pub reconciled: bool,
    /// When a waiting node is to be woken.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wake: Option<Wake>,
}

/// How a run ended.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "end")]
#[non_exhaustive]
pub enum RunOutcome {
    /// Every node that could run, ran (the default of a run not started).
    #[default]
    Finished,
    /// It stopped at a node that failed with [`ErrorPolicy::Stop`].
    Stopped {
        /// The node.
        node: String,
        /// Why it failed.
        reason: String,
    },
    /// It was cancelled.
    Cancelled {
        /// Why, as the token was told.
        reason: String,
    },
    /// The run took longer than its time limit.
    TimedOut {
        /// What took too long.
        reason: String,
    },
    /// A node waits; the run goes on when a durable run wakes it.
    Waiting {
        /// The node.
        node: String,
        /// What wakes it.
        wake: Wake,
    },
}

/// A step of a run, as [`Executor::on_step`] hears it: what a durable run writes down so that it
/// can start again where it was.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum Step<'r> {
    /// A node is about to run.
    Started {
        /// The node.
        node: &'r str,
    },
    /// A node's part is over: it ran, was skipped, failed or waits.
    Settled {
        /// The node.
        node: &'r str,
        /// Its part.
        part: &'r NodeRun,
    },
    /// A loop went round for the last time: it and its body are over.
    LoopDone {
        /// The loop node.
        head: &'r str,
    },
}

/// A run of a flow: each node's part, in the order they ran, and how it ended.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Run {
    /// Each node's part, by id.
    pub nodes: BTreeMap<String, NodeRun>,
    /// The ids of the nodes that took part, in the order they first ran.
    pub order: Vec<String>,
    /// The node the run stopped at, when one failed with [`ErrorPolicy::Stop`].
    pub stopped_at: Option<String>,
    /// The clock's time when the run started.
    pub started_ms: i64,
    /// How it ended.
    pub outcome: RunOutcome,
    deadline_ms: Option<i64>,
}

impl Run {
    /// The items `node` gave on `port`; none when it gave none or did not run.
    pub fn output(&self, node: &str, port: &str) -> &[Item] {
        self.nodes.get(node).and_then(|n| n.outputs.get(port)).map(Vec::as_slice).unwrap_or(&[])
    }

    /// How `node`'s part went, when it took part.
    pub fn status(&self, node: &str) -> Option<NodeStatus> {
        self.nodes.get(node).map(|n| n.status)
    }

    /// Whether it ended with every node that could run having run.
    pub fn succeeded(&self) -> bool {
        self.outcome == RunOutcome::Finished
    }

    /// Records `part` of `node`, counting one more time it ran.
    fn record(&mut self, node: &str, mut part: NodeRun) {
        match self.nodes.get(node) {
            Some(before) => part.runs = before.runs + 1,
            None => self.order.push(node.to_owned()),
        }
        self.nodes.insert(node.to_owned(), part);
    }
}

/// Why a run could not start.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RunError {
    /// The plan has no node with this id.
    UnknownNode {
        /// The id.
        node: String,
    },
    /// A node upstream has no part in the earlier run given, so its items are not there.
    NoPrevious {
        /// The node upstream.
        node: String,
    },
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownNode { node } => write!(f, "the flow has no node {node}"),
            Self::NoPrevious { node } => write!(f, "the earlier run has nothing from {node}"),
        }
    }
}

impl std::error::Error for RunError {}

/// What hears each step of a run.
type Listener<'a> = Box<dyn Fn(Step<'_>) + 'a>;

/// Runs a plan with the kinds of a catalog.
pub struct Executor<'a> {
    plan: &'a Plan,
    catalog: &'a Catalog,
    mode: Mode,
    clock: Arc<dyn Clock>,
    loop_limit: usize,
    token: CancelToken,
    watched: bool,
    time_limit_ms: Option<u64>,
    starts: BTreeMap<String, Vec<Item>>,
    on_step: Option<Listener<'a>>,
    events: Arc<BTreeMap<String, Vec<Item>>>,
    woken: BTreeMap<String, Wake>,
    /// The first item each node that already ran gave, for expressions that read `nodes.<id>`.
    seen: std::cell::RefCell<Arc<BTreeMap<String, serde_json::Value>>>,
    run: Option<Arc<str>>,
}

impl fmt::Debug for Executor<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Executor")
            .field("flow", &self.plan.definition().id)
            .field("mode", &self.mode)
            .finish()
    }
}

/// Whether the run goes on after a node's part.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Next {
    Go,
    Stop,
}

impl<'a> Executor<'a> {
    /// Runs `plan` with the kinds of `catalog`, as the flow working, on the machine's clock.
    pub fn new(plan: &'a Plan, catalog: &'a Catalog) -> Self {
        Self {
            plan,
            catalog,
            mode: Mode::Active,
            clock: Arc::new(SystemClock),
            loop_limit: LOOP_LIMIT,
            token: CancelToken::new(),
            watched: false,
            time_limit_ms: None,
            starts: BTreeMap::new(),
            on_step: None,
            events: Arc::default(),
            woken: BTreeMap::new(),
            seen: std::cell::RefCell::default(),
            run: None,
        }
    }

    /// In the run `id`, which every node reads with [`NodeContext::run_id`]: a durable runtime
    /// gives the id of its run, so a node finds what the host keeps for it.
    pub fn run_id(mut self, id: impl Into<String>) -> Self {
        self.run = Some(Arc::from(id.into()));
        self
    }

    /// Telling `listener` each step as it happens, on the thread that runs: before a node runs,
    /// when its part is over, and when a loop ends.
    pub fn on_step(mut self, listener: impl Fn(Step<'_>) + 'a) -> Self {
        self.on_step = Some(Box::new(listener));
        self
    }

    /// With the events that came for the run, by name, for the nodes that wait on them.
    pub fn events(mut self, events: BTreeMap<String, Vec<Item>>) -> Self {
        self.events = Arc::new(events);
        self
    }

    /// With the nodes woken after a wait, and how each was waiting.
    pub fn woken(mut self, woken: BTreeMap<String, Wake>) -> Self {
        self.woken = woken;
        self
    }

    fn tell(&self, step: Step<'_>) {
        if let Some(listener) = &self.on_step {
            listener(step);
        }
    }

    /// In `mode`.
    pub fn mode(mut self, mode: Mode) -> Self {
        self.mode = mode;
        self
    }

    /// On `clock`.
    pub fn clock(mut self, clock: impl Clock + 'static) -> Self {
        self.clock = Arc::new(clock);
        self
    }

    /// On a clock shared with something else, such as the flow that calls this one.
    pub fn shared_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    /// With loops stopped after `turns` turns ([`LOOP_LIMIT`] by default).
    pub fn loop_limit(mut self, turns: usize) -> Self {
        self.loop_limit = turns.max(1);
        self
    }

    /// Cancelled by `token`: keep a clone and call [`CancelToken::cancel`] from any thread. Each
    /// node then runs on a thread of its own, so a node that does not look at its token is
    /// still left behind when the run is cancelled.
    pub fn cancel_with(mut self, token: CancelToken) -> Self {
        self.token = token;
        self.watched = true;
        self
    }

    /// Cancelled by `token`, with the nodes running on the calling thread, as without a token:
    /// cancelling it stops the run before the next node and reaches the node running through
    /// its context ([`NodeContext::token`], [`NodeContext::check`], [`NodeContext::sleep`]).
    /// A node that does not look at its token runs to its end. Nothing runs on another thread,
    /// so a node that panics takes the caller down as it would with no token, and a run costs
    /// no thread per node: what a durable runtime wants, where a worker that dies is the
    /// failure it recovers from.
    pub fn cancel_cooperatively(mut self, token: CancelToken) -> Self {
        self.token = token;
        self.watched = false;
        self
    }

    /// With the whole run taking at most `ms` milliseconds.
    pub fn time_limit_ms(mut self, ms: u64) -> Self {
        self.time_limit_ms = Some(ms);
        self
    }

    /// With `node` giving `items` on its first output instead of running: how a run starts
    /// from what came in (a webhook, the items a calling flow gives a subflow).
    pub fn start_with(mut self, node: impl Into<String>, items: Vec<Item>) -> Self {
        self.starts.insert(node.into(), items);
        self
    }

    /// Runs the whole flow: triggers first, then each node as the plan orders them, each loop
    /// going round its body.
    pub fn run(&self) -> Run {
        let all: BTreeSet<String> = self.plan.order().iter().cloned().collect();
        self.go(&all, &Run::default(), None)
    }

    /// Goes on with a run that stopped: the nodes whose part is in `checkpoint` keep it and do
    /// not run again (a waiting node runs again, to see whether its wait is over), the nodes in
    /// `in_progress` were running when it stopped and are asked to reconcile first
    /// ([`NodeKind::reconcile`]), and the rest run. The run keeps the time it first started.
    pub fn resume(&self, checkpoint: &Run, in_progress: &BTreeSet<String>) -> Run {
        let all: BTreeSet<String> = self.plan.order().iter().cloned().collect();
        self.go(&all, &Run::default(), Some((checkpoint, in_progress)))
    }

    /// Runs `node` and every node it leads to; the nodes upstream do not run again, and what
    /// they gave is taken from `previous`. A node inside a loop's body runs that turn once.
    pub fn run_from(&self, node: &str, previous: &Run) -> Result<Run, RunError> {
        self.known(node)?;
        let ahead: BTreeSet<String> = self.plan.downstream(node).into_iter().collect();
        self.needs(&ahead, previous)?;
        Ok(self.go(&ahead, previous, None))
    }

    /// Runs `node` alone, with what reached it in `previous`; nothing after it runs, and a loop
    /// node takes one turn.
    pub fn run_node(&self, node: &str, previous: &Run) -> Result<Run, RunError> {
        self.known(node)?;
        let only = BTreeSet::from([node.to_owned()]);
        self.needs(&only, previous)?;
        Ok(self.go(&only, previous, None))
    }

    fn known(&self, node: &str) -> Result<(), RunError> {
        match self.plan.definition().find(node) {
            Some(_) => Ok(()),
            None => Err(RunError::UnknownNode { node: node.to_owned() }),
        }
    }

    /// Every node feeding the nodes to run, and not running itself, took part in `previous`.
    fn needs(&self, running: &BTreeSet<String>, previous: &Run) -> Result<(), RunError> {
        for id in running {
            for c in self.plan.feeds(id) {
                if !running.contains(&c.from.node) && !previous.nodes.contains_key(&c.from.node) {
                    return Err(RunError::NoPrevious { node: c.from.node.clone() });
                }
            }
        }
        Ok(())
    }

    /// Why the run must stop now, if it must: it was cancelled, or its time is up.
    fn halted(&self, run: &Run) -> Option<RunOutcome> {
        if self.token.is_cancelled() {
            return Some(RunOutcome::Cancelled {
                reason: self.token.reason().unwrap_or_else(|| "cancelled".to_owned()),
            });
        }
        match (run.deadline_ms, self.time_limit_ms) {
            (Some(deadline), Some(limit)) if self.clock.now_ms() >= deadline => {
                Some(RunOutcome::TimedOut { reason: format!("the run took more than {limit} ms") })
            }
            _ => None,
        }
    }

    /// Runs the nodes in `running`, in the plan's order, reusing from `previous` the parts of
    /// the nodes upstream that feed them.
    fn go(
        &self,
        running: &BTreeSet<String>,
        previous: &Run,
        resumed: Option<(&Run, &BTreeSet<String>)>,
    ) -> Run {
        let started = resumed.map_or_else(|| self.clock.now_ms(), |(checkpoint, _)| checkpoint.started_ms);
        let mut run = Run {
            started_ms: started,
            deadline_ms: self.time_limit_ms.map(|l| started.saturating_add(l as i64)),
            ..Run::default()
        };
        let upstream: BTreeSet<&str> = running
            .iter()
            .flat_map(|id| self.plan.feeds(id))
            .map(|c| c.from.node.as_str())
            .filter(|n| !running.contains(*n))
            .collect();
        let mut gone_round: BTreeSet<&str> = BTreeSet::new();
        for id in self.plan.order() {
            if gone_round.contains(id.as_str()) {
                continue;
            }
            if upstream.contains(id.as_str()) {
                if let Some(earlier) = previous.nodes.get(id) {
                    run.record(id, NodeRun { status: NodeStatus::Reused, runs: 0, ..earlier.clone() });
                }
                continue;
            }
            if !running.contains(id) {
                continue;
            }
            if let Some(outcome) = self.halted(&run) {
                run.outcome = outcome;
                break;
            }
            let body: Vec<&String> =
                self.plan.loop_body(id).iter().filter(|b| running.contains(*b)).collect();
            if let Some((checkpoint, busy)) = resumed {
                if let Some(kept) = checkpoint.nodes.get(id).filter(|k| k.status != NodeStatus::Waiting) {
                    run.record(id, kept.clone());
                    for b in &body {
                        if let Some(part) = checkpoint.nodes.get(*b) {
                            run.record(b, part.clone());
                        }
                    }
                    gone_round.extend(body.iter().map(|b| b.as_str()));
                    continue;
                }
                if busy.contains(id)
                    && body.is_empty()
                    && let Some(next) = self.reconciled(id, &mut run)
                {
                    if next == Next::Stop {
                        break;
                    }
                    continue;
                }
            }
            let next =
                if body.is_empty() { self.once(id, 0, &mut run) } else { self.round(id, &body, &mut run) };
            gone_round.extend(body.iter().map(|b| b.as_str()));
            if next == Next::Stop {
                break;
            }
        }
        run
    }

    /// A node that was running when its run stopped, asked whether its call happened: its part
    /// when the kind knows, `None` when it did not happen and the node is to run.
    fn reconciled(&self, node: &str, run: &mut Run) -> Option<Next> {
        let def = self.plan.definition().find(node)?;
        let kind = self.catalog.shared(&def.kind, def.version)?;
        let inputs = self.inputs_of(node, run);
        let cx = self.context(def, &inputs, run.started_ms, 0, 1, None, self.token.child());
        let part = match kind.reconcile(&cx) {
            Reconciled::NotDone => return None,
            Reconciled::Done(given) => self.given(def, inputs.clone(), given, 1, true),
            _ => {
                let unknown = NodeError::unknown(
                    "it was running when the run stopped, and whether it happened is not known",
                );
                self.failure(def, inputs.clone(), unknown, 1, NodeStatus::Uncertain)
            }
        };
        Some(self.settle(def, part, run))
    }

    /// One part of `node`, at the turn `iteration`, recorded in `run`.
    fn once(&self, node: &str, iteration: usize, run: &mut Run) -> Next {
        let Some(def) = self.plan.definition().find(node) else { return Next::Go };
        self.tell(Step::Started { node });
        let inputs = self.inputs_of(node, run);
        self.seen.replace(Arc::new(first_items(run)));
        let part = self.node(def, inputs, run, iteration);
        self.settle(def, part, run)
    }

    /// Records `part` and says whether the run stops there, and how it ended if it does.
    fn settle(&self, def: &NodeDef, part: NodeRun, run: &mut Run) -> Next {
        let reason = part.error.clone().unwrap_or_default();
        let outcome = match part.status {
            NodeStatus::Waiting => Some(RunOutcome::Waiting {
                node: def.id.clone(),
                wake: part.wake.clone().unwrap_or_default(),
            }),
            NodeStatus::Cancelled => Some(RunOutcome::Cancelled { reason }),
            NodeStatus::TimedOut if self.halted(run).is_some() => self.halted(run),
            NodeStatus::Failed | NodeStatus::TimedOut | NodeStatus::Uncertain
                if def.on_error == ErrorPolicy::Stop =>
            {
                run.stopped_at = Some(def.id.clone());
                Some(RunOutcome::Stopped { node: def.id.clone(), reason })
            }
            _ => None,
        };
        self.tell(Step::Settled { node: &def.id, part: &part });
        run.record(&def.id, part);
        match outcome {
            Some(outcome) => {
                run.outcome = outcome;
                Next::Stop
            }
            None => Next::Go,
        }
    }

    /// The loop `head` going round `body`: each turn the head runs with what came back so far,
    /// and the body runs while the head sends it items.
    fn round(&self, head: &str, body: &[&String], run: &mut Run) -> Next {
        let Some(def) = self.plan.definition().find(head) else { return Next::Go };
        let inside: BTreeSet<&str> = body.iter().map(|b| b.as_str()).collect();
        let into_body: BTreeSet<&str> = body
            .iter()
            .flat_map(|b| self.plan.feeds(b))
            .filter(|c| c.from.node == head)
            .map(|c| c.from.port.as_str())
            .collect();
        let back: Vec<_> = self.plan.back_connections().iter().filter(|c| c.to.node == head).collect();
        let mut came_back = Ports::new();
        let limit = self.loop_limit;
        for iteration in 0..=limit {
            if iteration == limit {
                let part = failed(Ports::new(), format!("the loop went round {limit} times without ending"));
                return self.settle(def, part, run);
            }
            if let Some(outcome) = self.halted(run) {
                run.outcome = outcome;
                return Next::Stop;
            }
            let mut inputs = self.inputs_of(head, run);
            for (port, items) in &came_back {
                inputs.entry(port.clone()).or_default().extend(items.iter().cloned());
            }
            self.tell(Step::Started { node: head });
            let part = self.node(def, inputs, run, iteration);
            let sends = part
                .outputs
                .iter()
                .any(|(port, items)| !items.is_empty() && into_body.contains(port.as_str()));
            if self.settle(def, part, run) == Next::Stop {
                return Next::Stop;
            }
            if !sends {
                self.tell(Step::LoopDone { head });
                return Next::Go;
            }
            for b in body {
                if let Some(outcome) = self.halted(run) {
                    run.outcome = outcome;
                    return Next::Stop;
                }
                if self.once(b, iteration, run) == Next::Stop {
                    return Next::Stop;
                }
            }
            for c in &back {
                if inside.contains(c.from.node.as_str()) {
                    let items = run.output(&c.from.node, &c.from.port).to_vec();
                    came_back.entry(c.to.port.clone()).or_default().extend(items);
                }
            }
        }
        Next::Go
    }

    /// The items reaching each input of `node`, from what the nodes feeding it gave in `run`.
    fn inputs_of(&self, node: &str, run: &Run) -> Ports {
        let mut inputs = Ports::new();
        for c in self.plan.feeds(node) {
            let items = run.output(&c.from.node, &c.from.port);
            inputs.entry(c.to.port.clone()).or_default().extend(items.iter().cloned());
        }
        inputs
    }

    /// One node's part: the items it starts with, its pinned items, skipped, disabled, or run,
    /// tried again as its setup says.
    fn node(&self, node: &NodeDef, inputs: Ports, run: &Run, iteration: usize) -> NodeRun {
        let Some(spec) = self.plan.spec(&node.id) else {
            return failed(inputs, format!("the kind {} is not in the plan", node.kind));
        };
        let first_out = self.plan.outputs(&node.id).first().cloned();
        if let Some(items) = self.starts.get(&node.id) {
            let given = first_out.map(|p| Ports::from([(p, items.clone())])).unwrap_or_default();
            return done(NodeStatus::Pinned, inputs, place(&node.id, given));
        }
        let fed = !self.plan.feeds(&node.id).is_empty();
        let nothing_came = inputs.values().all(Vec::is_empty);
        if self.mode == Mode::Test && !node.pinned.is_empty() {
            return done(NodeStatus::Pinned, inputs, place(&node.id, node.pinned.clone()));
        }
        if !spec.trigger && (!fed || nothing_came) {
            return done(NodeStatus::Skipped, inputs, Ports::new());
        }
        if node.disabled {
            let first_in = spec.inputs.first().and_then(|p| inputs.get(p)).cloned().unwrap_or_default();
            let given = first_out.map(|p| Ports::from([(p, first_in)])).unwrap_or_default();
            return done(NodeStatus::Disabled, inputs, place(&node.id, given));
        }
        let Some(kind) = self.catalog.shared(&node.kind, node.version) else {
            return failed(
                inputs,
                format!("the kind {} (version {}) is not in the catalog", node.kind, node.version),
            );
        };
        let retry = node.retry.unwrap_or(Retry::new(1, 0));
        let mut attempt = 1;
        loop {
            let deadline = self.deadline_of(node, run);
            let result = self.call(&kind, node, &inputs, run.started_ms, iteration, attempt, deadline);
            let error = match result {
                Ok(given) => return self.given(node, inputs, given, attempt, false),
                Err(e) if e.failure == Failure::Suspended => {
                    return NodeRun {
                        attempts: attempt,
                        wake: e.wake,
                        ..done(NodeStatus::Waiting, inputs, Ports::new())
                    };
                }
                Err(e) => e,
            };
            let again = match error.failure {
                Failure::Retryable => true,
                Failure::TimedOut => retry.on_timeout && self.halted(run).is_none(),
                Failure::Unknown => {
                    let cx = self.context(
                        node,
                        &inputs,
                        run.started_ms,
                        iteration,
                        attempt,
                        deadline,
                        self.token.child(),
                    );
                    match kind.reconcile(&cx) {
                        Reconciled::Done(given) => return self.given(node, inputs, given, attempt, true),
                        Reconciled::NotDone => true,
                        _ => return self.failure(node, inputs, error, attempt, NodeStatus::Uncertain),
                    }
                }
                _ => false,
            };
            if !again || attempt >= retry.tries {
                let status = match error.failure {
                    Failure::Cancelled => NodeStatus::Cancelled,
                    Failure::TimedOut => NodeStatus::TimedOut,
                    _ => NodeStatus::Failed,
                };
                return self.failure(node, inputs, error, attempt, status);
            }
            attempt += 1;
            if !self.wait(retry.wait_before(attempt)) {
                let why = self.token.reason().unwrap_or_else(|| "cancelled".to_owned());
                return self.failure(
                    node,
                    inputs,
                    NodeError::cancelled(why),
                    attempt - 1,
                    NodeStatus::Cancelled,
                );
            }
        }
    }

    /// When `node` must end by, on the run's clock: its own limit, or what is left of the run's.
    fn deadline_of(&self, node: &NodeDef, run: &Run) -> Option<i64> {
        let own = node.timeout_ms.map(|t| self.clock.now_ms().saturating_add(t as i64));
        match (own, run.deadline_ms) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Waits `ms` on the run's clock, a little at a time; `false` when the run was cancelled.
    fn wait(&self, ms: u64) -> bool {
        let mut left = ms;
        while left > 0 {
            if self.token.is_cancelled() {
                return false;
            }
            let now = left.min(10);
            self.clock.sleep_ms(now);
            left -= now;
        }
        !self.token.is_cancelled()
    }

    #[allow(clippy::too_many_arguments, reason = "the parts of one node's context")]
    fn context<'n>(
        &self,
        node: &'n NodeDef,
        inputs: &'n Ports,
        now_ms: i64,
        iteration: usize,
        attempt: u32,
        deadline: Option<i64>,
        token: CancelToken,
    ) -> NodeContext<'n> {
        NodeContext::new(node, inputs, now_ms, self.mode)
            .at_iteration(iteration)
            .at_attempt(attempt)
            .with_clock(self.clock.clone())
            .with_token(token)
            .with_deadline(deadline)
            .with_events(self.events.clone())
            .woken_from(self.woken.get(&node.id).cloned())
            .with_nodes(self.seen.borrow().clone())
            .in_run(self.run.clone())
    }

    /// One try of `kind` on `node`. With a time limit or a token to watch, the try runs on a
    /// thread of its own: when the run is cancelled or the time is up, the node's token is
    /// cancelled and the run stops waiting for it.
    #[allow(clippy::too_many_arguments, reason = "the parts of one node's context")]
    fn call(
        &self,
        kind: &Arc<dyn NodeKind>,
        node: &NodeDef,
        inputs: &Ports,
        now_ms: i64,
        iteration: usize,
        attempt: u32,
        deadline: Option<i64>,
    ) -> Result<Ports, NodeError> {
        let token = self.token.child();
        if !self.watched && deadline.is_none() || cfg!(target_arch = "wasm32") {
            return kind.execute(&self.context(node, inputs, now_ms, iteration, attempt, deadline, token));
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.on_thread(kind, node, inputs, now_ms, iteration, attempt, deadline, token)
        }
        #[cfg(target_arch = "wasm32")]
        {
            unreachable!("a browser runs every node on its own thread")
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[allow(clippy::too_many_arguments, reason = "the parts of one node's context")]
    fn on_thread(
        &self,
        kind: &Arc<dyn NodeKind>,
        node: &NodeDef,
        inputs: &Ports,
        now_ms: i64,
        iteration: usize,
        attempt: u32,
        deadline: Option<i64>,
        token: CancelToken,
    ) -> Result<Ports, NodeError> {
        use std::sync::mpsc::{RecvTimeoutError, channel};
        use std::time::{Duration, Instant};

        let (tx, rx) = channel();
        let (kind, node_def, given, clock, mode, own) =
            (kind.clone(), node.clone(), inputs.clone(), self.clock.clone(), self.mode, token.clone());
        let (events, woken, seen, run) = (
            self.events.clone(),
            self.woken.get(&node.id).cloned(),
            self.seen.borrow().clone(),
            self.run.clone(),
        );
        std::thread::spawn(move || {
            let cx = NodeContext::new(&node_def, &given, now_ms, mode)
                .at_iteration(iteration)
                .at_attempt(attempt)
                .with_clock(clock)
                .with_token(own)
                .with_deadline(deadline)
                .with_events(events)
                .with_nodes(seen)
                .woken_from(woken)
                .in_run(run);
            let _ = tx.send(kind.execute(&cx));
        });
        let began = Instant::now();
        let real_limit = deadline.map(|d| Duration::from_millis((d - self.clock.now_ms()).max(0) as u64));
        loop {
            match rx.recv_timeout(Duration::from_millis(5)) {
                Ok(result) => return result,
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(NodeError::new("the node stopped without answering"));
                }
                Err(RecvTimeoutError::Timeout) => {
                    if self.token.is_cancelled() {
                        let why = self.token.reason().unwrap_or_else(|| "cancelled".to_owned());
                        token.cancel(why.clone());
                        return Err(NodeError::cancelled(why));
                    }
                    let real = real_limit.is_some_and(|l| began.elapsed() >= l);
                    let on_clock = deadline.is_some_and(|d| self.clock.now_ms() >= d);
                    if real || on_clock {
                        token.cancel("the time limit was reached");
                        return Err(NodeError::timed_out(format!(
                            "it took more than {} ms",
                            real_limit.map_or(0, |l| l.as_millis())
                        )));
                    }
                }
            }
        }
    }

    /// The part of a node that answered, checked against its outputs and its error policy.
    fn given(&self, node: &NodeDef, inputs: Ports, given: Ports, attempt: u32, reconciled: bool) -> NodeRun {
        let outputs = self.plan.outputs(&node.id);
        if let Some(port) = given.keys().find(|p| *p != ERROR_PORT && !outputs.contains(p)) {
            return NodeRun {
                attempts: attempt,
                ..failed(inputs, format!("the kind {} has no output {port}", node.kind))
            };
        }
        let failures = given.get(ERROR_PORT).map_or(0, Vec::len);
        let mut part = done(NodeStatus::Done, inputs, place(&node.id, given));
        part.attempts = attempt;
        part.reconciled = reconciled;
        if failures > 0 && node.on_error == ErrorPolicy::Stop {
            let why = part.outputs[ERROR_PORT][0]
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("it failed")
                .to_owned();
            part.status = NodeStatus::Failed;
            part.error = Some(format!("{failures} item(s) failed; the first: {why}"));
        }
        part
    }

    /// The part of a node that failed as a whole: with [`ErrorPolicy::Route`], every item that
    /// reached it goes to its error output.
    fn failure(
        &self,
        node: &NodeDef,
        inputs: Ports,
        error: NodeError,
        attempt: u32,
        status: NodeStatus,
    ) -> NodeRun {
        let routed = node.on_error == ErrorPolicy::Route && status != NodeStatus::Cancelled;
        let outputs = if routed {
            let sent: Vec<Item> = inputs.values().flatten().map(|i| i.failed(&error.message)).collect();
            place(&node.id, Ports::from([(ERROR_PORT.to_owned(), sent)]))
        } else {
            Ports::new()
        };
        NodeRun { status, error: Some(error.message), attempts: attempt, ..done(status, inputs, outputs) }
    }
}

fn done(status: NodeStatus, inputs: Ports, outputs: Ports) -> NodeRun {
    NodeRun { status, inputs, outputs, error: None, runs: 1, attempts: 1, reconciled: false, wake: None }
}

fn failed(inputs: Ports, error: String) -> NodeRun {
    NodeRun { error: Some(error), ..done(NodeStatus::Failed, inputs, Ports::new()) }
}

/// The items of each port put where they are, each saying where it came from.
fn place(node: &str, given: Ports) -> Ports {
    given
        .into_iter()
        .map(|(port, items)| {
            let placed = items.into_iter().enumerate().map(|(i, item)| item.placed(node, &port, i)).collect();
            (port, placed)
        })
        .collect()
}

/// The first item each node of `run` gave, by id: on `main` when it gave there, otherwise on
/// the first port that has one.
fn first_items(run: &Run) -> BTreeMap<String, serde_json::Value> {
    run.nodes
        .iter()
        .filter_map(|(id, part)| {
            let first = part
                .outputs
                .get(crate::nodes::MAIN)
                .and_then(|items| items.first())
                .or_else(|| part.outputs.values().find_map(|items| items.first()))?;
            Some((id.clone(), first.json.clone()))
        })
        .collect()
}
