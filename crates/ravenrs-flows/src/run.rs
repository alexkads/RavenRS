//! Running a plan in memory, one node after another in the plan's order, on the thread that
//! asks: the whole flow, from a node with what an earlier run left upstream, or one node alone.
//! A loop goes round its body while it sends items into it. Same flow, items and clock, same
//! run. The top layer of the crate.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::catalog::{Catalog, ERROR_PORT, Mode, NodeContext};
use crate::definition::{ErrorPolicy, NodeDef};
use crate::item::{Item, Ports};
use crate::plan::Plan;

/// The most turns a loop takes, unless [`Executor::loop_limit`] says otherwise, before the run
/// says it goes round for ever. Each turn the loop node gets everything that came back so far,
/// so a loop of many turns costs more than its turns.
pub const LOOP_LIMIT: usize = 10_000;

/// Where a run takes the time from.
pub trait Clock: Send + Sync {
    /// Milliseconds since 1970.
    fn now_ms(&self) -> i64;
}

/// A clock that always says the same time: for tests and repeatable runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedClock(pub i64);

impl Clock for FixedClock {
    fn now_ms(&self) -> i64 {
        self.0
    }
}

/// The machine's clock.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SystemClock;

impl Clock for SystemClock {
    #[cfg(not(target_arch = "wasm32"))]
    fn now_ms(&self) -> i64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
    }

    /// In a browser, `std` has no clock: the page's is used.
    #[cfg(target_arch = "wasm32")]
    fn now_ms(&self) -> i64 {
        js_sys::Date::now() as i64
    }
}

/// How a node's part of a run went.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NodeStatus {
    /// It ran.
    Done,
    /// It gave its pinned items, in a test.
    Pinned,
    /// It is disabled and passed its first input on.
    Disabled,
    /// No item reached it, so it did not run.
    Skipped,
    /// It failed.
    Failed,
    /// It did not run again: what it gave is an earlier run's.
    Reused,
}

/// A node's part of a run: what reached it, what it gave, and how it went. For a node inside
/// a loop, these are the last turn's.
#[derive(Clone, Debug, PartialEq)]
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
}

/// A run of a flow: each node's part, in the order they ran.
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

    /// Whether it ended without stopping at a failure.
    pub fn succeeded(&self) -> bool {
        self.stopped_at.is_none()
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

/// Runs a plan with the kinds of a catalog.
pub struct Executor<'a> {
    plan: &'a Plan,
    catalog: &'a Catalog,
    mode: Mode,
    clock: Box<dyn Clock + 'a>,
    loop_limit: usize,
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
        Self { plan, catalog, mode: Mode::Active, clock: Box::new(SystemClock), loop_limit: LOOP_LIMIT }
    }

    /// In `mode`.
    pub fn mode(mut self, mode: Mode) -> Self {
        self.mode = mode;
        self
    }

    /// On `clock`.
    pub fn clock(mut self, clock: impl Clock + 'a) -> Self {
        self.clock = Box::new(clock);
        self
    }

    /// With loops stopped after `turns` turns ([`LOOP_LIMIT`] by default).
    pub fn loop_limit(mut self, turns: usize) -> Self {
        self.loop_limit = turns.max(1);
        self
    }

    /// Runs the whole flow: triggers first, then each node as the plan orders them, each loop
    /// going round its body.
    pub fn run(&self) -> Run {
        let all: BTreeSet<String> = self.plan.order().iter().cloned().collect();
        self.go(&all, &Run::default())
    }

    /// Runs `node` and every node it leads to; the nodes upstream do not run again, and what
    /// they gave is taken from `previous`. A node inside a loop's body runs that turn once.
    pub fn run_from(&self, node: &str, previous: &Run) -> Result<Run, RunError> {
        self.known(node)?;
        let ahead: BTreeSet<String> = self.plan.downstream(node).into_iter().collect();
        self.needs(&ahead, previous)?;
        Ok(self.go(&ahead, previous))
    }

    /// Runs `node` alone, with what reached it in `previous`; nothing after it runs, and a loop
    /// node takes one turn.
    pub fn run_node(&self, node: &str, previous: &Run) -> Result<Run, RunError> {
        self.known(node)?;
        let only = BTreeSet::from([node.to_owned()]);
        self.needs(&only, previous)?;
        Ok(self.go(&only, previous))
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

    /// Runs the nodes in `running`, in the plan's order, reusing from `previous` the parts of
    /// the nodes upstream that feed them.
    fn go(&self, running: &BTreeSet<String>, previous: &Run) -> Run {
        let mut run = Run { started_ms: self.clock.now_ms(), ..Run::default() };
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
            let body: Vec<&String> =
                self.plan.loop_body(id).iter().filter(|b| running.contains(*b)).collect();
            let next =
                if body.is_empty() { self.once(id, 0, &mut run) } else { self.round(id, &body, &mut run) };
            gone_round.extend(body.iter().map(|b| b.as_str()));
            if next == Next::Stop {
                break;
            }
        }
        run
    }

    /// One part of `node`, at the turn `iteration`, recorded in `run`.
    fn once(&self, node: &str, iteration: usize, run: &mut Run) -> Next {
        let Some(def) = self.plan.definition().find(node) else { return Next::Go };
        let inputs = self.inputs_of(node, run);
        let part = self.node(def, inputs, run.started_ms, iteration);
        self.settle(def, part, run)
    }

    /// Records `part` and says whether the run stops there.
    fn settle(&self, def: &NodeDef, part: NodeRun, run: &mut Run) -> Next {
        let stop = part.status == NodeStatus::Failed && def.on_error == ErrorPolicy::Stop;
        run.record(&def.id, part);
        if stop {
            run.stopped_at = Some(def.id.clone());
            Next::Stop
        } else {
            Next::Go
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
            let mut inputs = self.inputs_of(head, run);
            for (port, items) in &came_back {
                inputs.entry(port.clone()).or_default().extend(items.iter().cloned());
            }
            let part = self.node(def, inputs, run.started_ms, iteration);
            let sends = part
                .outputs
                .iter()
                .any(|(port, items)| !items.is_empty() && into_body.contains(port.as_str()));
            if self.settle(def, part, run) == Next::Stop {
                return Next::Stop;
            }
            if !sends {
                return Next::Go;
            }
            for b in body {
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

    /// One node's part.
    fn node(&self, node: &NodeDef, inputs: Ports, now_ms: i64, iteration: usize) -> NodeRun {
        let Some(spec) = self.plan.spec(&node.id) else {
            return failed(inputs, format!("the kind {} is not in the plan", node.kind));
        };
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
            let mut given = Ports::new();
            if let Some(first_out) = self.plan.outputs(&node.id).first() {
                given.insert(first_out.clone(), first_in);
            }
            return done(NodeStatus::Disabled, inputs, place(&node.id, given));
        }
        let Some(kind) = self.catalog.get(&node.kind, node.version) else {
            return failed(
                inputs,
                format!("the kind {} (version {}) is not in the catalog", node.kind, node.version),
            );
        };
        let cx = NodeContext::new(node, &inputs, now_ms, self.mode).at_iteration(iteration);
        let result = kind.execute(&cx);
        let outputs = self.plan.outputs(&node.id);
        match result {
            Ok(given) => {
                if let Some(port) = given.keys().find(|p| *p != ERROR_PORT && !outputs.contains(p)) {
                    return failed(inputs, format!("the kind {} has no output {port}", node.kind));
                }
                let failures = given.get(ERROR_PORT).map_or(0, Vec::len);
                if failures > 0 && node.on_error == ErrorPolicy::Stop {
                    let why =
                        given[ERROR_PORT][0].get("error").and_then(|e| e.as_str()).unwrap_or("it failed");
                    let message = format!("{failures} item(s) failed; the first: {why}");
                    return NodeRun {
                        error: Some(message),
                        ..done(NodeStatus::Failed, inputs, place(&node.id, given))
                    };
                }
                done(NodeStatus::Done, inputs, place(&node.id, given))
            }
            Err(e) if node.on_error == ErrorPolicy::Route => {
                let sent: Vec<Item> = inputs.values().flatten().map(|i| i.failed(&e.message)).collect();
                let given = Ports::from([(ERROR_PORT.to_owned(), sent)]);
                NodeRun { error: Some(e.message), ..done(NodeStatus::Failed, inputs, place(&node.id, given)) }
            }
            Err(e) => failed(inputs, e.message),
        }
    }
}

fn done(status: NodeStatus, inputs: Ports, outputs: Ports) -> NodeRun {
    NodeRun { status, inputs, outputs, error: None, runs: 1 }
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
