//! What passes from node to node: items, each a JSON value with the files it carries and
//! where it came from. The bottom layer of the crate: it knows no definition, kind or run.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Where an item is: the node that gave it, the output port and its place in that port's list.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ItemRef {
    /// The node's id.
    pub node: String,
    /// The output port's id.
    pub port: String,
    /// Its place among the items of that port, from 0.
    pub index: usize,
}

impl ItemRef {
    /// The item `index` of the port `port` of the node `node`.
    pub fn new(node: impl Into<String>, port: impl Into<String>, index: usize) -> Self {
        Self { node: node.into(), port: port.into(), index }
    }
}

/// A file an item carries, kept apart from its JSON: what it is, how big, and where it is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BinaryRef {
    /// Its media type, such as `image/png`.
    pub mime: String,
    /// Its size in bytes.
    pub size: u64,
    /// Where it is kept: a path or a key in a store, as the app decides.
    pub location: String,
}

/// One item of a list passing along a connection.
///
/// A node that makes an item from another uses [`Item::derive`], so the run can tell where
/// each item came from; a node that passes an item on unchanged returns it as it came, and the
/// run says that it came from where it was.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Item {
    /// What the item says.
    pub json: Value,
    /// The files it carries, by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub binary: BTreeMap<String, BinaryRef>,
    /// Where it is, once a run has put it on a port; `None` while a node is making it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<ItemRef>,
    /// The items it was made from, in the order they were given.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub from: Vec<ItemRef>,
}

impl Item {
    /// A new item saying `json`, made from nothing that came before.
    pub fn new(json: Value) -> Self {
        Self { json, ..Self::default() }
    }

    /// A new item saying `json`, made from this one: it keeps this one's files and says it came
    /// from here.
    pub fn derive(&self, json: Value) -> Self {
        Self { json, binary: self.binary.clone(), at: None, from: self.at.iter().cloned().collect() }
    }

    /// A new item saying `json`, made from all of `parents` (a merge, a group).
    pub fn derive_all<'a>(parents: impl IntoIterator<Item = &'a Item>, json: Value) -> Self {
        Self { json, from: parents.into_iter().filter_map(|p| p.at.clone()).collect(), ..Self::default() }
    }

    /// A new item made from this one, saying it failed because of `message`: an object keeps its
    /// fields and gains `error`; anything else is kept under `value`.
    pub fn failed(&self, message: &str) -> Self {
        let json = match &self.json {
            Value::Object(map) => {
                let mut map = map.clone();
                map.insert("error".to_owned(), Value::String(message.to_owned()));
                Value::Object(map)
            }
            other => serde_json::json!({ "error": message, "value": other }),
        };
        self.derive(json)
    }

    /// The same item carrying the file `binary` under `name`.
    pub fn with_binary(mut self, name: impl Into<String>, binary: BinaryRef) -> Self {
        self.binary.insert(name.into(), binary);
        self
    }

    /// The value at `key`, when the item is a JSON object that has it.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.json.get(key)
    }

    /// Puts the item on the port `port` of `node` at `index`. An item that was already somewhere
    /// passed through, and is said to come from there.
    pub(crate) fn placed(mut self, node: &str, port: &str, index: usize) -> Self {
        if let Some(previous) = self.at.take() {
            self.from = vec![previous];
        }
        self.at = Some(ItemRef::new(node, port, index));
        self
    }
}

/// The items of a node's ports, by port id.
pub type Ports = BTreeMap<String, Vec<Item>>;
