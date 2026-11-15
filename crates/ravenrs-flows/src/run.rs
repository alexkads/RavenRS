//! Running a plan in memory, one node after another in the plan's order, on the thread that
//! asks: the whole flow, from a node with what an earlier run left upstream, or one node alone.
//! Same flow, items and clock, same run. The top layer of the crate.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::catalog::{Catalog, Mode, NodeContext};
use crate::definition::{ErrorPolicy, NodeDef};
use crate::item::{Item, Ports};
use crate::plan::Plan;

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

/// A node's part of a run: what reached it, what it gave, and how it went.
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
}

/// A run of a flow: each node's part, in the order they ran.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Run {
    /// Each node's part, by id.
    pub nodes: BTreeMap<String, NodeRun>,
    /// The ids of the nodes that took part, in order.
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
}

impl fmt::Debug for Executor<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Executor")
            .field("flow", &self.plan.definition().id)
            .field("mode", &self.mode)
            .finish()
    }
}

impl<'a> Executor<'a> {
    /// Runs `plan` with the kinds of `catalog`, as the flow working, on the machine's clock.
    pub fn new(plan: &'a Plan, catalog: &'a Catalog) -> Self {
        Self { plan, catalog, mode: Mode::Active, clock: Box::new(SystemClock) }
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

    /// Runs the whole flow: triggers first, then each node as the plan orders them.
    pub fn run(&self) -> Run {
        let all: BTreeSet<String> = self.plan.order().iter().cloned().collect();
        self.go(&all, &Run::default())
    }

    /// Runs `node` and every node it leads to; the nodes upstream do not run again, and what
    /// they gave is taken from `previous`.
    pub fn run_from(&self, node: &str, previous: &Run) -> Result<Run, RunError> {
        self.known(node)?;
        let ahead: BTreeSet<String> = self.plan.downstream(node).into_iter().collect();
        self.needs(&ahead, previous)?;
        Ok(self.go(&ahead, previous))
    }

    /// Runs `node` alone, with what reached it in `previous`; nothing after it runs.
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
        for id in self.plan.order() {
            if upstream.contains(id.as_str()) {
                if let Some(earlier) = previous.nodes.get(id) {
                    run.nodes.insert(id.clone(), NodeRun { status: NodeStatus::Reused, ..earlier.clone() });
                    run.order.push(id.clone());
                }
                continue;
            }
            if !running.contains(id) {
                continue;
            }
            let Some(node) = self.plan.definition().find(id) else { continue };
            let inputs = self.inputs_of(id, &run);
            let part = self.node(node, inputs, run.started_ms);
            let stop = part.status == NodeStatus::Failed && node.on_error == ErrorPolicy::Stop;
            run.nodes.insert(id.clone(), part);
            run.order.push(id.clone());
            if stop {
                run.stopped_at = Some(id.clone());
                break;
            }
        }
        run
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
    fn node(&self, node: &NodeDef, inputs: Ports, now_ms: i64) -> NodeRun {
        let Some(spec) = self.plan.spec(&node.id) else {
            return failed(inputs, format!("the kind {} is not in the plan", node.kind));
        };
        let fed = !self.plan.feeds(&node.id).is_empty();
        let nothing_came = inputs.values().all(Vec::is_empty);
        if self.mode == Mode::Test && !node.pinned.is_empty() {
            return NodeRun {
                status: NodeStatus::Pinned,
                outputs: place(&node.id, node.pinned.clone()),
                inputs,
                error: None,
            };
        }
        if !spec.trigger && (!fed || nothing_came) {
            return NodeRun { status: NodeStatus::Skipped, inputs, outputs: Ports::new(), error: None };
        }
        if node.disabled {
            let first_in = spec.inputs.first().and_then(|p| inputs.get(p)).cloned().unwrap_or_default();
            let mut given = Ports::new();
            if let Some(first_out) = spec.outputs.first() {
                given.insert(first_out.clone(), first_in);
            }
            return NodeRun {
                status: NodeStatus::Disabled,
                outputs: place(&node.id, given),
                inputs,
                error: None,
            };
        }
        let Some(kind) = self.catalog.get(&node.kind, node.version) else {
            return failed(
                inputs,
                format!("the kind {} (version {}) is not in the catalog", node.kind, node.version),
            );
        };
        let result = kind.execute(&NodeContext::new(node, &inputs, now_ms, self.mode));
        match result {
            Ok(given) => match given.keys().find(|p| !spec.outputs.contains(p)) {
                Some(port) => failed(inputs, format!("the kind {} has no output {port}", node.kind)),
                None => {
                    NodeRun { status: NodeStatus::Done, outputs: place(&node.id, given), inputs, error: None }
                }
            },
            Err(e) => failed(inputs, e.message),
        }
    }
}

fn failed(inputs: Ports, error: String) -> NodeRun {
    NodeRun { status: NodeStatus::Failed, inputs, outputs: Ports::new(), error: Some(error) }
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
