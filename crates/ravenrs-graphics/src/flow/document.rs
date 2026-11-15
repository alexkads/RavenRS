//! Diagrams kept by name: what a library of diagrams writes to storage and reads back.

use serde::{Deserialize, Serialize};

use super::FlowGraph;

/// A diagram with a name, as a library keeps it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FlowDocument {
    /// Unique among the documents of a shelf.
    pub id: String,
    /// What the person called it.
    pub name: String,
    /// The diagram.
    pub graph: FlowGraph,
}

/// Every document a library keeps, in the order they were made.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FlowShelf {
    /// The documents.
    pub documents: Vec<FlowDocument>,
}

impl FlowShelf {
    /// The shelf written as JSON, which [`Self::from_json`] reads back.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// A shelf read from JSON; nothing readable gives an empty shelf, and says why.
    pub fn from_json(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }

    /// The document with this id.
    pub fn find(&self, id: &str) -> Option<&FlowDocument> {
        self.documents.iter().find(|d| d.id == id)
    }

    /// An id no document has yet.
    pub fn fresh_id(&self) -> String {
        (1..).map(|n| format!("d{n}")).find(|id| self.find(id).is_none()).unwrap_or_default()
    }

    /// Keeps `document`, in place of the one with its id, or after the others.
    pub fn put(&mut self, document: FlowDocument) {
        match self.documents.iter_mut().find(|d| d.id == document.id) {
            Some(kept) => *kept = document,
            None => self.documents.push(document),
        }
    }

    /// Forgets the document with this id. Says whether there was one.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.documents.len();
        self.documents.retain(|d| d.id != id);
        before != self.documents.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::FlowNode;

    #[test]
    fn a_shelf_keeps_replaces_and_forgets_documents() {
        let mut shelf = FlowShelf::default();
        let id = shelf.fresh_id();
        let graph = FlowGraph::new().node(FlowNode::new("a", "", "A", 0.0, 0.0));
        shelf.put(FlowDocument { id: id.clone(), name: "One".into(), graph: graph.clone() });
        assert_ne!(shelf.fresh_id(), id);
        shelf.put(FlowDocument { id: id.clone(), name: "Renamed".into(), graph });
        assert_eq!(shelf.documents.len(), 1);
        assert_eq!(shelf.find(&id).map(|d| d.name.as_str()), Some("Renamed"));
        let back = FlowShelf::from_json(&shelf.to_json());
        assert_eq!(back.as_ref(), Ok(&shelf));
        assert!(shelf.remove(&id) && shelf.documents.is_empty());
        assert!(FlowShelf::from_json("not json").is_err());
    }
}
