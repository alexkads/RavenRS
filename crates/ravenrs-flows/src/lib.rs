//! Runs flows drawn on a RavenRS `FlowCanvas`.
//!
//! A flow ([`FlowDefinition`]) is a list of nodes, each of a kind at a version with its
//! parameters, and connections between their named ports. A [`Catalog`] holds the kinds: what
//! each one says about itself ([`NodeSpec`]) and the code that runs it ([`NodeKind`]). A
//! [`Plan`] checks a flow against the catalog and puts its nodes in order, and an [`Executor`]
//! runs it, passing lists of [`Item`]s from node to node: the whole flow, from one node on
//! (reusing an earlier run's items upstream), or one node alone. Every item says where it is
//! and what it was made from, and the same flow, items and clock always give the same run.
//!
//! No screen and no GPU: the crate runs on a server or a worker thread. The bridge to the
//! canvas ([`FlowDefinition::to_graph`], [`FlowDefinition::from_graph`] and
//! [`Catalog::run_rules`]) speaks the `ravenrs-graphics` model the `FlowCanvas` draws.
//!
//! ```
//! use ravenrs_flows::{
//!     Catalog, Executor, FixedClock, FlowDefinition, Item, NodeContext, NodeDef, NodeError, NodeKind,
//!     NodeSpec, Plan, Ports,
//! };
//! use serde_json::json;
//!
//! /// Starts a run with the items of its parameter `items`.
//! struct Manual;
//! impl NodeKind for Manual {
//!     fn spec(&self) -> NodeSpec {
//!         NodeSpec::new("manual", 1).title("Manual start").output("main").trigger()
//!     }
//!     fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
//!         let items = cx.parameter("items").and_then(|v| v.as_array()).cloned().unwrap_or_default();
//!         Ok(Ports::from([("main".into(), items.into_iter().map(Item::new).collect())]))
//!     }
//! }
//!
//! /// Keeps the items whose `paid` is true.
//! struct Paid;
//! impl NodeKind for Paid {
//!     fn spec(&self) -> NodeSpec {
//!         NodeSpec::new("paid", 1).input("main").output("main")
//!     }
//!     fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
//!         let kept = cx.input("main").iter().filter(|i| i.get("paid") == Some(&json!(true))).cloned();
//!         Ok(Ports::from([("main".into(), kept.collect())]))
//!     }
//! }
//!
//! let catalog = Catalog::new().with(Manual).with(Paid);
//! let flow = FlowDefinition::new("f1", "Paid orders")
//!     .node(NodeDef::new("start", "manual").parameter("items", json!([{"paid": true}, {"paid": false}])))
//!     .node(NodeDef::new("filter", "paid"))
//!     .connect("start", "main", "filter", "main");
//!
//! let plan = Plan::compile(&flow, &catalog).expect("a flow the catalog knows");
//! let run = Executor::new(&plan, &catalog).clock(FixedClock(0)).run();
//! assert_eq!(run.output("filter", "main").len(), 1);
//! // The item kept says it came from the first item of the start.
//! assert_eq!(run.output("filter", "main")[0].from[0].index, 0);
//! ```
//!
//! The built-in kinds are in [`nodes`], all at once in [`Catalog::builtin`]: a manual start,
//! `if`, `switch`, `merge` (append, by position, by key, first), a loop in batches, and the
//! nodes that set, filter, sort, group, split and join items. Their conditions and values are
//! written in the criteria language of `ravenrs-criteria`. A loop node goes round its body
//! while it sends items into it ([`NodeContext::iteration`], [`Plan::loop_body`]), and items
//! that fail one by one go to every node's error output, [`ERROR_PORT`].
//!
//! A run can be cancelled from any thread ([`CancelToken`], [`Executor::cancel_with`]), has
//! time limits for each node and for the whole run, and tries a node again as its [`Retry`]
//! says, by the kind of [`Failure`]: a call whose answer was lost is not repeated until
//! [`NodeKind::reconcile`] says it did not happen. Waits run on the run's [`Clock`];
//! [`ManualClock`] makes them take no time in tests.
//!
//! A durable run hears each step ([`Executor::on_step`]) and goes on from it
//! ([`Executor::resume`]); `ravenrs-flows-runtime` does this on SQLite, and runs the nodes that
//! wait for a time or an event (`delay`, `wait_event`) again when they are due.
//!
//! Not here yet: the schema of a kind's parameters (#530).

mod catalog;
mod definition;
mod graph;
mod item;
pub mod nodes;
mod plan;
mod run;
mod time;

pub use catalog::{
    Catalog, ERROR_PORT, Failure, Mode, NodeContext, NodeError, NodeKind, NodeSpec, Reconciled, Wake,
};
pub use definition::{
    Connection, DefinitionError, ErrorPolicy, FORMAT, FlowDefinition, NodeDef, PortRef, Retry,
};
pub use graph::GRAPH_KEY;
pub use item::{BinaryRef, Item, ItemRef, Ports};
pub use plan::{Plan, PlanError};
pub use run::{Executor, LOOP_LIMIT, NodeRun, NodeStatus, Run, RunError, RunOutcome, Step};
pub use time::{CancelToken, Clock, FixedClock, ManualClock, SystemClock};
