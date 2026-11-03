//! A node of the application model.

use std::collections::BTreeMap;

use serde_json::Value as Json;

/// A node of the application model: an id, settings and nodes under it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModelNode {
    /// What it is known by among the nodes beside it.
    pub id: String,
    /// Its settings.
    pub properties: BTreeMap<String, Json>,
    /// The nodes under it, in the order they came.
    pub children: Vec<ModelNode>,
}

impl ModelNode {
    /// A node with no settings and nothing under it.
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into(), ..Self::default() }
    }

    /// With this setting.
    pub fn with(mut self, name: &str, value: impl Into<Json>) -> Self {
        self.properties.insert(name.to_owned(), value.into());
        self
    }

    /// With this node under it.
    pub fn child(mut self, child: ModelNode) -> Self {
        self.children.push(child);
        self
    }

    /// The nodes under it, as they are, including those an index hides.
    pub(crate) fn in_order_all(&self) -> &[ModelNode] {
        &self.children
    }

    /// The node under it with this id.
    pub fn get(&self, id: &str) -> Option<&ModelNode> {
        self.children.iter().find(|c| c.id == id)
    }

    pub(crate) fn get_mut(&mut self, id: &str) -> Option<&mut ModelNode> {
        self.children.iter_mut().find(|c| c.id == id)
    }

    pub(crate) fn at_mut(&mut self, path: &[&str]) -> Option<&mut ModelNode> {
        path.iter().try_fold(self, |node, id| node.get_mut(id))
    }

    /// The node at this path of ids below it.
    pub fn at(&self, path: &[&str]) -> Option<&ModelNode> {
        path.iter().try_fold(self, |node, id| node.get(id))
    }

    /// A setting as text.
    pub fn text(&self, name: &str) -> Option<&str> {
        self.properties.get(name).and_then(Json::as_str)
    }

    /// A setting as it is typed: text as it is, and anything else as JSON.
    pub fn typed(&self, name: &str) -> Option<String> {
        self.properties.get(name).map(|value| match value {
            Json::String(text) => text.clone(),
            other => other.to_string(),
        })
    }

    /// A setting as a whole number.
    pub fn integer(&self, name: &str) -> Option<i64> {
        self.properties.get(name).and_then(Json::as_i64)
    }

    /// A setting as a number.
    pub fn number(&self, name: &str) -> Option<f64> {
        self.properties.get(name).and_then(Json::as_f64)
    }

    /// A setting as yes or no.
    pub fn flag(&self, name: &str) -> Option<bool> {
        self.properties.get(name).and_then(Json::as_bool)
    }

    /// The nodes under it in the order to show them: by `Index`, then as they came.
    /// A negative index keeps a node out of sight, so it is not here.
    pub fn in_order(&self) -> Vec<&ModelNode> {
        let mut shown: Vec<&ModelNode> =
            self.children.iter().filter(|c| c.integer("Index").is_none_or(|i| i >= 0)).collect();
        shown.sort_by_key(|c| c.integer("Index").unwrap_or(i64::MAX));
        shown
    }
}
