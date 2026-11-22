//! Durable runs of [`ravenrs_flows`] flows, kept in SQLite.
//!
//! A [`Runtime`] keeps a queue of runs in a SQLite file that several processes may share.
//! [`Runtime::enqueue`] asks for a run with the flow as it is now and the items it starts
//! with. A worker [`claim`](Runtime::claim)s the run due first with a lease, in one statement,
//! so two workers never take the same run. It runs the run and writes a checkpoint at every
//! node.
//!
//! When a worker dies, its lease runs out and another worker takes the run. That worker goes
//! on from the checkpoint: the nodes that were over keep their part and do not run again, and
//! a node that was running when the worker died is asked whether its call happened
//! (`NodeKind::reconcile`) before it runs again.
//!
//! A node can wait for a time (`delay`) or an event (`wait_event`). The run stops there and is
//! due again when the time comes or [`Runtime::signal`] says the event came. Runs can be run
//! again on the flow as it was or as it is now ([`Runtime::rerun`]). Old runs are forgotten
//! with their files ([`Runtime::prune`]).
//!
//! ```
//! use ravenrs_flows::{Catalog, FlowDefinition, NodeDef};
//! use ravenrs_flows_runtime::{Request, RunStatus, Runtime};
//! use serde_json::json;
//!
//! let runtime = Runtime::in_memory(Catalog::builtin()).expect("a database in memory");
//! let flow = FlowDefinition::new("hello", "Hello")
//!     .node(NodeDef::new("start", "manual").parameter("items", json!([{ "name": "Ana" }])));
//! let id = runtime.enqueue(Request::new(flow)).expect("asked for");
//! assert_eq!(runtime.work_until_idle("worker-1").expect("ran"), 1);
//! assert_eq!(runtime.get(&id).expect("kept").status, RunStatus::Finished);
//! ```
//!
//! A worker in another process, with no access to the file (a build agent on another machine),
//! runs a run through a [`Lease`] and sends back each [`StepRecord`]: see [`Runtime::lease`].
//!
//! A loop is one checkpoint: a run stopped inside a loop goes round it again from the start
//! when it resumes.

mod remote;
mod runtime;
mod store;

pub use remote::{Held, Lease, StepRecord};
pub use runtime::{LEASE_MS, Pruned, Request, Retention, Revision, RunRecord, RunStatus, Runtime};
pub use store::StoreError;
