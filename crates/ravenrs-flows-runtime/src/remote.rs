//! Running a run in another process, one that has no access to the SQLite file: a build agent on
//! another machine. Above `runtime`.
//!
//! # Its role
//!
//! [`Runtime::execute`] runs a run in the process that holds the file: it reads the checkpoint,
//! runs the flow and writes each step as it happens. A worker elsewhere needs the same three
//! things carried over a wire, and nothing else changes: the checkpoint goes out as a [`Lease`],
//! each step comes back as a [`StepRecord`], and the end comes back as a
//! [`RunOutcome`](ravenrs_flows::RunOutcome).
//!
//! # How it works
//!
//! 1. The host picks the run ([`Runtime::due`], [`Runtime::claim_id`]) for a worker of its own
//!    naming (`agent/linux-1/2`), and hands over [`Runtime::lease`].
//! 2. The worker runs it with [`Lease::run`]: the same `Executor::resume` that `execute` uses, so
//!    the nodes that settled keep their part and a node that was in progress is asked whether its
//!    call happened (`NodeKind::reconcile`) before it runs again.
//! 3. Each step the worker hears goes back and the host writes it with [`Runtime::record`], which
//!    also renews the lease and answers whether the worker still holds the run and whether someone
//!    asked to cancel it ([`Held`]). [`Runtime::renew`] does the same with no step: a node that
//!    runs for minutes writes nothing.
//! 4. The end goes back and the host writes it with [`Runtime::finish`].
//!
//! Writing a step twice is harmless (the same node settles to the same part), so a worker that
//! lost the connection sends again whatever it is not sure arrived. A worker that died comes
//! back to the same run with a fresh [`Runtime::lease`], from where the last step it sent left it.
//!
//! ```
//! use ravenrs_flows::{CancelToken, Catalog, FlowDefinition, NodeDef, SystemClock};
//! use ravenrs_flows_runtime::{Request, RunStatus, Runtime};
//! use serde_json::json;
//! use std::sync::Arc;
//!
//! let runtime = Runtime::in_memory(Catalog::builtin())?;
//! let flow = FlowDefinition::new("hello", "Hello")
//!     .node(NodeDef::new("start", "manual").parameter("items", json!([{ "name": "Ana" }])));
//! let id = runtime.enqueue(Request::new(flow))?;
//! assert!(runtime.claim_id(&id, "agent/a/1")?);
//! // What crosses the wire, as JSON.
//! let lease = serde_json::to_string(&runtime.lease(&id, "agent/a/1")?).expect("JSON");
//! // On the other side.
//! let lease: ravenrs_flows_runtime::Lease = serde_json::from_str(&lease).expect("JSON");
//! let mut records = Vec::new();
//! let run = lease
//!     .run(&Catalog::builtin(), Arc::new(SystemClock), CancelToken::new(), |step| records.push(step))
//!     .expect("compiles");
//! // Back on the host.
//! for step in &records {
//!     assert!(runtime.record(&id, "agent/a/1", step)?.held);
//! }
//! assert_eq!(runtime.finish(&id, "agent/a/1", &run.outcome)?, RunStatus::Finished);
//! # Ok::<(), ravenrs_flows_runtime::StoreError>(())
//! ```

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ravenrs_flows::{
    CancelToken, Catalog, Clock, Executor, FlowDefinition, Item, Mode, NodeRun, Plan, Run, Step, Wake,
};
use serde::{Deserialize, Serialize};

/// A run as a worker elsewhere needs it: the flow as the run kept it, what it starts with, and
/// what it kept so far. It is plain data, made to go out as JSON.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lease {
    /// The run's id: what its nodes see as their run id.
    pub id: String,
    /// The worker it was handed to.
    pub worker: String,
    /// The flow, as the run kept it, in its JSON.
    pub definition: String,
    /// A test from the editor rather than the flow working.
    pub test: bool,
    /// The node that starts with [`input`](Self::input) instead of running.
    pub start_node: Option<String>,
    /// The items the run starts with.
    pub input: Vec<Item>,
    /// When the run first started, on the host's clock.
    pub started_ms: i64,
    /// The parts of the nodes that settled, in the order they first ran (a loop's only once it
    /// ended).
    pub settled: Vec<(String, NodeRun)>,
    /// The nodes the run was in when its last worker stopped: each is asked whether its call
    /// happened before it runs again.
    pub in_progress: BTreeSet<String>,
    /// How each waiting node waits.
    pub woken: BTreeMap<String, Wake>,
    /// What came for the run while it waited, by name.
    pub events: BTreeMap<String, Vec<Item>>,
    /// Why someone asked to cancel it, when they did before it was handed over.
    pub cancel_requested: Option<String>,
}

/// One step of a run, owned, as a worker elsewhere sends it back: what
/// [`Step`](ravenrs_flows::Step) is, without the borrow.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "step")]
#[non_exhaustive]
pub enum StepRecord {
    /// A node is about to run.
    Started {
        /// The node.
        node: String,
    },
    /// A node's part is over.
    Settled {
        /// The node.
        node: String,
        /// Its part.
        part: NodeRun,
    },
    /// A loop went round for the last time.
    LoopDone {
        /// The loop node.
        head: String,
    },
}

impl StepRecord {
    /// The step `step`, owned; `None` for a kind of step a durable run does not keep.
    pub fn of(step: Step<'_>) -> Option<Self> {
        match step {
            Step::Started { node } => Some(Self::Started { node: node.to_owned() }),
            Step::Settled { node, part } => Some(Self::Settled { node: node.to_owned(), part: part.clone() }),
            Step::LoopDone { head } => Some(Self::LoopDone { head: head.to_owned() }),
            _ => None,
        }
    }
}

/// What the host says back to a worker that wrote a step or renewed its lease.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Held {
    /// The worker still holds the run. When it does not (its lease ran out and another took it,
    /// or the run ended), it stops and writes nothing more.
    pub held: bool,
    /// Someone asked to cancel the run, and why: the worker cancels its token.
    pub cancel: Option<String>,
}

impl Lease {
    /// The flow, read back.
    pub fn flow(&self) -> Result<FlowDefinition, String> {
        FlowDefinition::from_json(&self.definition).map_err(|e| e.to_string())
    }

    /// Runs the run from where it was, with the kinds of `catalog` on `clock`, cancelled through
    /// `token`, telling `on_step` each step to send back. The run as it ended (or as it waits);
    /// an error, in words, when the flow does not compile with `catalog`.
    pub fn run(
        &self,
        catalog: &Catalog,
        clock: Arc<dyn Clock>,
        token: CancelToken,
        on_step: impl FnMut(StepRecord),
    ) -> Result<Run, String> {
        let flow = self.flow()?;
        let plan = Plan::compile(&flow, catalog)
            .map_err(|errors| errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "))?;
        if let Some(reason) = &self.cancel_requested {
            token.cancel(reason.clone());
        }
        let mut checkpoint = Run::default();
        checkpoint.started_ms = self.started_ms;
        for (node, part) in &self.settled {
            checkpoint.order.push(node.clone());
            checkpoint.nodes.insert(node.clone(), part.clone());
        }
        let on_step = RefCell::new(on_step);
        let mut executor = Executor::new(&plan, catalog)
            .run_id(self.id.clone())
            .mode(if self.test { Mode::Test } else { Mode::Active })
            .shared_clock(clock)
            .events(self.events.clone())
            .woken(self.woken.clone())
            .cancel_cooperatively(token)
            .on_step(|step| {
                if let Some(record) = StepRecord::of(step) {
                    (on_step.borrow_mut())(record);
                }
            });
        if let Some(node) = &self.start_node {
            executor = executor.start_with(node.clone(), self.input.clone());
        }
        Ok(executor.resume(&checkpoint, &self.in_progress))
    }
}
