//! The kinds of node a flow may use: what each one says about itself ([`NodeSpec`]), the code
//! that runs it ([`NodeKind`]) and the register of them by name and version ([`Catalog`]).
//! Above `item` and `definition`; the plan and the run use it, never the other way round.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::definition::NodeDef;
use crate::item::{Item, Ports};
use crate::time::{CancelToken, Clock, SystemClock};

/// The output every node has besides its kind's: where the items that failed go, each saying
/// why. A connection may leave it from any node.
pub const ERROR_PORT: &str = "error";

/// What a kind of node says about itself: its ports and how a run treats it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeSpec {
    /// Its name in the catalog.
    pub kind: String,
    /// Its version: a node set up for another version needs that one in the catalog.
    pub version: u32,
    /// What it is called where no title is given.
    pub title: String,
    /// Its input ports, in order; the first is the one a disabled node passes on.
    pub inputs: Vec<String>,
    /// Its output ports, in order.
    pub outputs: Vec<String>,
    /// The parameters a node of this kind must have.
    pub required: Vec<String>,
    /// A trigger: a run starts here, with no input.
    pub trigger: bool,
    /// A loop: a connection may come back to it from a node after it.
    pub loops: bool,
    /// It sends each item to one of its outputs (a branch), rather than to all of them.
    pub chooses: bool,
}

impl NodeSpec {
    /// The kind `kind` at `version`, titled by its name, with no ports.
    pub fn new(kind: impl Into<String>, version: u32) -> Self {
        let kind = kind.into();
        Self {
            title: kind.clone(),
            kind,
            version,
            inputs: Vec::new(),
            outputs: Vec::new(),
            required: Vec::new(),
            trigger: false,
            loops: false,
            chooses: false,
        }
    }

    /// Titled `title`.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// With the input port `port`.
    pub fn input(mut self, port: impl Into<String>) -> Self {
        self.inputs.push(port.into());
        self
    }

    /// With the output port `port`.
    pub fn output(mut self, port: impl Into<String>) -> Self {
        self.outputs.push(port.into());
        self
    }

    /// Requiring the parameter `name`.
    pub fn requires(mut self, name: impl Into<String>) -> Self {
        self.required.push(name.into());
        self
    }

    /// A trigger.
    pub fn trigger(mut self) -> Self {
        self.trigger = true;
        self
    }

    /// A loop.
    pub fn loops(mut self) -> Self {
        self.loops = true;
        self
    }

    /// A branch.
    pub fn chooses(mut self) -> Self {
        self.chooses = true;
        self
    }
}

/// Whether a run is a test, from the editor, or the flow working.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Mode {
    /// The flow working: pinned items are not used.
    #[default]
    Active,
    /// A test from the editor: a node with pinned items gives them instead of running.
    Test,
}

/// What a node has while it runs: its setup, the items that reached each input, the time and
/// the mode.
#[derive(Debug)]
pub struct NodeContext<'a> {
    node: &'a NodeDef,
    inputs: &'a Ports,
    now_ms: i64,
    mode: Mode,
    iteration: usize,
    attempt: u32,
    clock: Arc<dyn Clock>,
    token: CancelToken,
    deadline_ms: Option<i64>,
    events: Arc<BTreeMap<String, Vec<Item>>>,
    woken: Option<Wake>,
    nodes: Arc<BTreeMap<String, Value>>,
    run: Option<Arc<str>>,
}

impl<'a> NodeContext<'a> {
    /// The context of `node`, with `inputs`, at `now_ms` milliseconds since 1970, in `mode`, on
    /// the machine's clock, with nothing to cancel it and no time limit.
    pub fn new(node: &'a NodeDef, inputs: &'a Ports, now_ms: i64, mode: Mode) -> Self {
        Self {
            node,
            inputs,
            now_ms,
            mode,
            iteration: 0,
            attempt: 1,
            clock: Arc::new(SystemClock),
            token: CancelToken::new(),
            deadline_ms: None,
            events: Arc::default(),
            woken: None,
            nodes: Arc::default(),
            run: None,
        }
    }

    /// The same context, in the run `id`.
    pub fn in_run(mut self, id: Option<Arc<str>>) -> Self {
        self.run = id;
        self
    }

    /// The id of the run the node is in, when its host gave one ([`Executor::run_id`](crate::Executor::run_id)):
    /// how a node finds what its host keeps for the run, such as the folder and the log of a build.
    pub fn run_id(&self) -> Option<&str> {
        self.run.as_deref()
    }

    /// The same context, with the events that came for the run, by name.
    pub fn with_events(mut self, events: Arc<BTreeMap<String, Vec<Item>>>) -> Self {
        self.events = events;
        self
    }

    /// The same context, for a node woken after it waited as `wake` said.
    pub fn woken_from(mut self, wake: Option<Wake>) -> Self {
        self.woken = wake;
        self
    }

    /// The first item each node that already ran gave, by id.
    pub fn nodes(&self) -> Arc<BTreeMap<String, Value>> {
        self.nodes.clone()
    }

    /// With the first item each node that already ran gave, by id, for the expressions that
    /// read `nodes.<id>`.
    pub fn with_nodes(mut self, nodes: Arc<BTreeMap<String, Value>>) -> Self {
        self.nodes = nodes;
        self
    }

    /// The parameter `name` written as text with expressions inside it (`Olá {{ nome }}`),
    /// worked out for `item`; `None` when the node has no such parameter. Expressions read the
    /// item, the nodes that already ran and the run (see [`crate::expression`]).
    pub fn text(&self, name: &str, item: &Item) -> Result<Option<String>, NodeError> {
        let Some(value) = self.parameter(name) else { return Ok(None) };
        let Some(text) = value.as_str() else {
            return Err(NodeError::new(format!("the parameter {name} must be text")));
        };
        let record = crate::expression::record(
            &item.json,
            &self.nodes,
            &serde_json::Map::new(),
            &crate::expression::run_value(self.now_ms),
        );
        crate::expression::render(text, &record)
            .map(Some)
            .map_err(|why| NodeError::new(format!("the parameter {name}: {why}")))
    }

    /// The items the event `event` brought, when it came.
    pub fn event(&self, event: &str) -> Option<&[Item]> {
        self.events.get(event).map(Vec::as_slice)
    }

    /// How the node was waiting, when this run woke it after a wait.
    pub fn woken(&self) -> Option<&Wake> {
        self.woken.as_ref()
    }

    /// The same context, waiting on `clock`.
    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    /// The same context, stopped by `token`.
    pub fn with_token(mut self, token: CancelToken) -> Self {
        self.token = token;
        self
    }

    /// The same context, with `deadline_ms` (on its clock) as the time it must end by.
    pub fn with_deadline(mut self, deadline_ms: Option<i64>) -> Self {
        self.deadline_ms = deadline_ms;
        self
    }

    /// The same context, at the try `attempt`, from 1.
    pub fn at_attempt(mut self, attempt: u32) -> Self {
        self.attempt = attempt;
        self
    }

    /// The same context, at the turn `iteration` of a loop.
    pub fn at_iteration(mut self, iteration: usize) -> Self {
        self.iteration = iteration;
        self
    }

    /// The node being run.
    pub fn node(&self) -> &NodeDef {
        self.node
    }

    /// Its parameter `name`.
    pub fn parameter(&self, name: &str) -> Option<&Value> {
        self.node.parameters.get(name)
    }

    /// All its parameters.
    pub fn parameters(&self) -> &Map<String, Value> {
        &self.node.parameters
    }

    /// The items that reached the input `port`; none when nothing did.
    pub fn input(&self, port: &str) -> &[Item] {
        self.inputs.get(port).map(Vec::as_slice).unwrap_or(&[])
    }

    /// The items of every input, by port.
    pub fn inputs(&self) -> &Ports {
        self.inputs
    }

    /// The run's clock, in milliseconds since 1970: the same for every node of a run.
    pub fn now_ms(&self) -> i64 {
        self.now_ms
    }

    /// Whether this is a test or the flow working.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Which turn of its loop this is, from 0. A loop node runs once per turn, and what came
    /// back to it in every earlier turn reaches its input from the back connections; any other
    /// node is at 0, or at the turn of the loop it is inside.
    pub fn iteration(&self) -> usize {
        self.iteration
    }

    /// Which try this is, from 1: more than 1 when the node failed in a way worth trying again.
    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    /// The clock the run waits on.
    pub fn clock(&self) -> &Arc<dyn Clock> {
        &self.clock
    }

    /// What says the run, or this node, should stop. A node that calls something slow gives it
    /// on, or a child of it, so the call can stop too.
    pub fn token(&self) -> &CancelToken {
        &self.token
    }

    /// Whether the node should stop now.
    pub fn cancelled(&self) -> bool {
        self.token.is_cancelled()
    }

    /// How long the node has left, in milliseconds on its clock, when it has a time limit.
    pub fn time_left_ms(&self) -> Option<i64> {
        self.deadline_ms.map(|d| d - self.clock.now_ms())
    }

    /// `Ok` while the node may go on; the error to return when it was cancelled or its time is
    /// up. A long node calls it between steps.
    pub fn check(&self) -> Result<(), NodeError> {
        if self.token.is_cancelled() {
            return Err(NodeError::cancelled(self.token.reason().unwrap_or_else(|| "cancelled".to_owned())));
        }
        if self.time_left_ms().is_some_and(|left| left <= 0) {
            return Err(NodeError::timed_out("the time limit was reached"));
        }
        Ok(())
    }

    /// Waits `ms` milliseconds on the run's clock, a little at a time, and stops early when the
    /// node is cancelled or its time is up.
    pub fn sleep(&self, ms: u64) -> Result<(), NodeError> {
        const STEP: u64 = 10;
        let mut left = ms;
        while left > 0 {
            self.check()?;
            let now = STEP.min(left);
            self.clock.sleep_ms(now);
            left -= now;
        }
        self.check()
    }
}

/// When a waiting node is to be woken: at a time, when an event comes, or both.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wake {
    /// The time on the run's clock to wake at, in milliseconds since 1970.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until_ms: Option<i64>,
    /// The event that wakes it, by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
}

/// What kind of failure it was, which decides whether the node is tried again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Failure {
    /// Trying again will not help: a wrong setup, a refusal (the default).
    #[default]
    Permanent,
    /// It may work if tried again: a service busy or out of reach.
    Retryable,
    /// No one knows whether it happened: the call went and no answer came. It is not tried
    /// again until [`NodeKind::reconcile`] says it did not happen.
    Unknown,
    /// The node was cancelled.
    Cancelled,
    /// The node's time was up.
    TimedOut,
    /// Not a failure: the node waits, for a time or an event, and the run stops there until a
    /// durable run wakes it ([`NodeError::wait_until`], [`NodeError::wait_for`]).
    Suspended,
}

/// Why a node failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeError {
    /// What went wrong, said to the person.
    pub message: String,
    /// What kind of failure it was.
    pub failure: Failure,
    /// When a node that waits is to be woken.
    pub wake: Option<Wake>,
}

impl NodeError {
    /// A failure saying `message` that trying again will not mend.
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into(), failure: Failure::Permanent, wake: None }
    }

    /// A failure that may pass if the node is tried again.
    pub fn retryable(message: impl Into<String>) -> Self {
        Self { failure: Failure::Retryable, ..Self::new(message) }
    }

    /// A call whose result is not known.
    pub fn unknown(message: impl Into<String>) -> Self {
        Self { failure: Failure::Unknown, ..Self::new(message) }
    }

    /// The node was cancelled, for `reason`.
    pub fn cancelled(reason: impl Into<String>) -> Self {
        Self { failure: Failure::Cancelled, ..Self::new(reason) }
    }

    /// The node's time was up.
    pub fn timed_out(message: impl Into<String>) -> Self {
        Self { failure: Failure::TimedOut, ..Self::new(message) }
    }

    /// The node waits until `until_ms` on the run's clock; the run stops there, and a durable
    /// run starts it again then.
    pub fn wait_until(until_ms: i64) -> Self {
        Self {
            failure: Failure::Suspended,
            wake: Some(Wake { until_ms: Some(until_ms), event: None }),
            ..Self::new(format!("waiting until {until_ms}"))
        }
    }

    /// The node waits for the event `event`; the run stops there until it comes.
    pub fn wait_for(event: impl Into<String>) -> Self {
        let event = event.into();
        Self {
            failure: Failure::Suspended,
            message: format!("waiting for {event}"),
            wake: Some(Wake { until_ms: None, event: Some(event) }),
        }
    }
}

/// What a kind found when asked whether a call of unknown result happened.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Reconciled {
    /// It happened, and gave these items.
    Done(Ports),
    /// It did not happen: the node may be tried again.
    NotDone,
    /// Still not known: the node is left as uncertain, not tried again.
    Unknown,
}

impl fmt::Display for NodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for NodeError {}

/// A kind of node: what it says about itself and the code that runs it.
///
/// It is `Send + Sync` so that a run may happen on a worker thread.
pub trait NodeKind: Send + Sync {
    /// Its ports and how a run treats it.
    fn spec(&self) -> NodeSpec;

    /// The output ports of `node`, a node of this kind: its spec's, unless they depend on how
    /// the node is set up (a switch with one output per case).
    fn outputs(&self, node: &NodeDef) -> Vec<String> {
        let _ = node;
        self.spec().outputs
    }

    /// Runs one node of this kind: from what reached its inputs, the items of each output port.
    /// A port it leaves out gives no items. Items that failed one by one go to
    /// [`ERROR_PORT`] (see [`Item::failed`]); a failure of the whole node is the `Err`.
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError>;

    /// After [`execute`](Self::execute) failed with [`Failure::Unknown`], finds out whether
    /// the call happened (looking it up in the service, by a key the node sent). A kind that
    /// cannot tell leaves the default, and the node stays uncertain rather than be repeated.
    fn reconcile(&self, cx: &NodeContext<'_>) -> Reconciled {
        let _ = cx;
        Reconciled::Unknown
    }
}

/// The kinds a flow may use, by name and version.
#[derive(Clone, Default)]
pub struct Catalog {
    kinds: BTreeMap<(String, u32), Arc<dyn NodeKind>>,
}

impl fmt::Debug for Catalog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Catalog").field("kinds", &self.kinds.keys().collect::<Vec<_>>()).finish()
    }
}

impl Catalog {
    /// An empty catalog.
    pub fn new() -> Self {
        Self::default()
    }

    /// With `kind` added; a kind of the same name and version is replaced.
    pub fn with(mut self, kind: impl NodeKind + 'static) -> Self {
        self.add(kind);
        self
    }

    /// Adds `kind`; a kind of the same name and version is replaced.
    pub fn add(&mut self, kind: impl NodeKind + 'static) {
        let spec = kind.spec();
        self.kinds.insert((spec.kind, spec.version), Arc::new(kind));
    }

    /// The kind `kind` at `version`, shared, so a run can hand it to another thread.
    pub fn shared(&self, kind: &str, version: u32) -> Option<Arc<dyn NodeKind>> {
        self.kinds.get(&(kind.to_owned(), version)).cloned()
    }

    /// The kind `kind` at `version`.
    pub fn get(&self, kind: &str, version: u32) -> Option<&dyn NodeKind> {
        self.kinds.get(&(kind.to_owned(), version)).map(|k| k.as_ref())
    }

    /// What every kind says about itself, by name and then version.
    pub fn specs(&self) -> Vec<NodeSpec> {
        self.kinds.values().map(|k| k.spec()).collect()
    }
}
