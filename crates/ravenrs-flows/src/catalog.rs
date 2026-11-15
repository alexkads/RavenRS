//! The kinds of node a flow may use: what each one says about itself ([`NodeSpec`]), the code
//! that runs it ([`NodeKind`]) and the register of them by name and version ([`Catalog`]).
//! Above `item` and `definition`; the plan and the run use it, never the other way round.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use serde_json::{Map, Value};

use crate::definition::NodeDef;
use crate::item::{Item, Ports};

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
}

impl<'a> NodeContext<'a> {
    /// The context of `node`, with `inputs`, at `now_ms` milliseconds since 1970, in `mode`.
    pub fn new(node: &'a NodeDef, inputs: &'a Ports, now_ms: i64, mode: Mode) -> Self {
        Self { node, inputs, now_ms, mode, iteration: 0 }
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
}

/// Why a node failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeError {
    /// What went wrong, said to the person.
    pub message: String,
}

impl NodeError {
    /// A failure saying `message`.
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }
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

    /// The kind `kind` at `version`.
    pub fn get(&self, kind: &str, version: u32) -> Option<&dyn NodeKind> {
        self.kinds.get(&(kind.to_owned(), version)).map(|k| k.as_ref())
    }

    /// What every kind says about itself, by name and then version.
    pub fn specs(&self) -> Vec<NodeSpec> {
        self.kinds.values().map(|k| k.spec()).collect()
    }
}
