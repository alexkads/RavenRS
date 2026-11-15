//! A flow as the app keeps it: nodes with their kind, version and parameters, connections
//! between named ports, and the revision. It is data only, written as versioned JSON; it knows
//! no kind's code (the catalog) and no geometry (the canvas keeps that).

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::item::Item;

/// The version of the JSON [`FlowDefinition::to_json`] writes.
pub const FORMAT: u32 = 1;

fn format() -> u32 {
    FORMAT
}

fn one() -> u32 {
    1
}

/// What a node does when it fails.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ErrorPolicy {
    /// The run stops at the node (the default).
    #[default]
    Stop,
    /// The node gives nothing and the run goes on.
    Continue,
}

/// A port of a node: where a connection starts or ends.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PortRef {
    /// The node's id.
    pub node: String,
    /// The port's id on that node.
    pub port: String,
}

impl PortRef {
    /// The port `port` of the node `node`.
    pub fn new(node: impl Into<String>, port: impl Into<String>) -> Self {
        Self { node: node.into(), port: port.into() }
    }
}

/// A connection: the items of an output port go to an input port.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connection {
    /// The output port the items leave.
    pub from: PortRef,
    /// The input port they reach.
    pub to: PortRef,
}

/// A node of a flow: which kind it is, at which version, and how it is set up.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeDef {
    /// Unique in the flow, and kept when the node is renamed or moved.
    pub id: String,
    /// The kind's name in the catalog.
    pub kind: String,
    /// The kind's version the node was set up for.
    #[serde(default = "one")]
    pub version: u32,
    /// What the person called it, when not the kind's own title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Its parameters, by name.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub parameters: Map<String, Value>,
    /// A disabled node passes the items of its first input to its first output unchanged.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disabled: bool,
    /// What it does when it fails.
    #[serde(default, skip_serializing_if = "is_default")]
    pub on_error: ErrorPolicy,
    /// Items fixed on its output ports, used instead of running it while testing.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pinned: BTreeMap<String, Vec<Item>>,
}

fn is_default(policy: &ErrorPolicy) -> bool {
    *policy == ErrorPolicy::default()
}

impl NodeDef {
    /// A node `id` of the kind `kind`, version 1, with no parameters.
    pub fn new(id: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: kind.into(),
            version: 1,
            title: None,
            parameters: Map::new(),
            disabled: false,
            on_error: ErrorPolicy::default(),
            pinned: BTreeMap::new(),
        }
    }

    /// Set up for the version `version` of its kind.
    pub fn version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }

    /// Called `title`.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// With the parameter `name` set to `value`.
    pub fn parameter(mut self, name: impl Into<String>, value: Value) -> Self {
        self.parameters.insert(name.into(), value);
        self
    }

    /// Disabled, or not.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Doing `policy` when it fails.
    pub fn on_error(mut self, policy: ErrorPolicy) -> Self {
        self.on_error = policy;
        self
    }

    /// With `items` fixed on its output port `port`.
    pub fn pin(mut self, port: impl Into<String>, items: Vec<Item>) -> Self {
        self.pinned.insert(port.into(), items);
        self
    }
}

/// A flow: its nodes and connections, and the revision they are at.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FlowDefinition {
    /// The version of the JSON it was read from or will be written as.
    #[serde(default = "format")]
    pub format: u32,
    /// Unique among the app's flows.
    pub id: String,
    /// What the person called it.
    pub name: String,
    /// Which revision it is; the app raises it on each saved change.
    #[serde(default)]
    pub revision: u32,
    /// The nodes, in the order they were made.
    #[serde(default)]
    pub nodes: Vec<NodeDef>,
    /// The connections, in the order they were made.
    #[serde(default)]
    pub connections: Vec<Connection>,
}

impl FlowDefinition {
    /// An empty flow `id` called `name`, at revision 0.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            format: FORMAT,
            id: id.into(),
            name: name.into(),
            revision: 0,
            nodes: Vec::new(),
            connections: Vec::new(),
        }
    }

    /// With the node `node` added at the end.
    pub fn node(mut self, node: NodeDef) -> Self {
        self.nodes.push(node);
        self
    }

    /// With a connection from the output `from_port` of `from` to the input `to_port` of `to`.
    pub fn connect(mut self, from: &str, from_port: &str, to: &str, to_port: &str) -> Self {
        self.connections
            .push(Connection { from: PortRef::new(from, from_port), to: PortRef::new(to, to_port) });
        self
    }

    /// The node with the id `id`.
    pub fn find(&self, id: &str) -> Option<&NodeDef> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// The flow written as JSON, at the version [`FORMAT`].
    pub fn to_json(&self) -> String {
        let mut written = self.clone();
        written.format = FORMAT;
        serde_json::to_string(&written).unwrap_or_default()
    }

    /// A flow read from JSON. A version newer than [`FORMAT`] is refused, so an older build
    /// never drops what it does not know.
    pub fn from_json(text: &str) -> Result<Self, DefinitionError> {
        let read: Self =
            serde_json::from_str(text).map_err(|e| DefinitionError::Unreadable(e.to_string()))?;
        if read.format > FORMAT {
            return Err(DefinitionError::Newer { format: read.format });
        }
        Ok(Self { format: FORMAT, ..read })
    }
}

/// Why a flow could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DefinitionError {
    /// The text is not a flow; the reason is the reader's.
    Unreadable(String),
    /// It was written by a newer build, at this version of the format.
    Newer {
        /// The version it was written at.
        format: u32,
    },
}

impl fmt::Display for DefinitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable(why) => write!(f, "not a flow: {why}"),
            Self::Newer { format } => {
                write!(f, "written at version {format} of the format; this build reads up to {FORMAT}")
            }
        }
    }
}

impl std::error::Error for DefinitionError {}
