//! Layers of differences over the generated model.

use serde_json::{Map, Value as Json};

use super::node::ModelNode;
use crate::ModelError;

/// The key that marks a node in a difference as created by it. Any other node a
/// difference names has to exist already.
const NEW_NODE: &str = "$new";

/// A layer of the model, from the lowest to the highest. The highest to set a value wins.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum ModelLayer {
    /// What a module adds, by its name; modules stack in the order they first appear.
    Module(String),
    /// What the administrator of an installation set.
    Administrator,
    /// What the person using the application set.
    User,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ModelLayer {
    /// Where it goes among layers: modules first, the person last.
    fn rank(&self) -> u8 {
        match self {
            Self::Module(_) => 0,
            Self::Administrator => 1,
            Self::User => 2,
        }
    }
}

/// A difference over the layers below it, as JSON: an object is a node, by id, and anything else
/// is a setting.
///
/// ```json
/// { "Views": { "Account_ListView": { "Columns": { "opening": { "Index": -1 } } } } }
/// ```
///
/// A node that this difference itself creates has `"$new": true`.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Diff(Map<String, Json>);

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Diff {
    /// A difference that changes nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads it from JSON text.
    pub fn parse(text: &str) -> Result<Self, ModelError> {
        match serde_json::from_str(text) {
            Ok(Json::Object(map)) => Ok(Self(map)),
            Ok(_) => Err(ModelError("a model difference has to be a JSON object".into())),
            Err(e) => Err(ModelError(format!("a model difference is not valid JSON: {e}"))),
        }
    }

    /// Writes it as JSON text, ready for people to read.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self.0).unwrap_or_else(|_| "{}".to_owned())
    }

    /// Puts this over it, node by node.
    pub fn merge(&mut self, over: &Diff) {
        fn merged(into: &mut Map<String, Json>, over: &Map<String, Json>) {
            for (name, value) in over {
                match (into.get_mut(name), value) {
                    (Some(Json::Object(mine)), Json::Object(theirs)) => merged(mine, theirs),
                    _ => {
                        into.insert(name.clone(), value.clone());
                    }
                }
            }
        }
        merged(&mut self.0, &over.0);
    }

    /// Whether it changes anything.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn set(&mut self, path: &[&str], name: &str, value: Json) {
        let mut map = &mut self.0;
        for id in path {
            let entry = map.entry((*id).to_owned()).or_insert_with(|| Json::Object(Map::new()));
            if !entry.is_object() {
                *entry = Json::Object(Map::new());
            }
            let Json::Object(next) = entry else { return };
            map = next;
        }
        map.insert(name.to_owned(), value);
    }

    /// Takes the setting off, and with it whatever nodes this leaves empty.
    fn clear(&mut self, path: &[&str], name: &str) {
        fn walk(map: &mut Map<String, Json>, path: &[&str], name: &str) {
            match path.split_first() {
                None => {
                    map.remove(name);
                }
                Some((id, rest)) => {
                    if let Some(Json::Object(next)) = map.get_mut(*id) {
                        walk(next, rest, name);
                        if next.is_empty() {
                            map.remove(*id);
                        }
                    }
                }
            }
        }
        walk(&mut self.0, path, name);
    }
}

/// A part of a difference that has no node to land on, as after the entity it
/// refers to is gone. It is kept, and comes back into use if the node does.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct UnusableNode {
    /// The layer that holds it.
    pub layer: ModelLayer,
    /// The ids that lead to it.
    pub path: Vec<String>,
    /// What it wanted to change.
    pub difference: Json,
}

/// The model as it stands, with all its layers applied.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct MergedModel {
    /// The tree to read settings from.
    pub root: ModelNode,
    /// What had nowhere to go.
    pub unusable: Vec<UnusableNode>,
}

fn apply(
    node: &mut ModelNode,
    difference: &Map<String, Json>,
    layer: &ModelLayer,
    path: &mut Vec<String>,
    unusable: &mut Vec<UnusableNode>,
) {
    for (key, value) in difference {
        match value {
            Json::Object(inner) => {
                let creates = inner.get(NEW_NODE).and_then(Json::as_bool).unwrap_or(false);
                if creates && node.get(key).is_none() {
                    node.children.push(ModelNode::new(key));
                }
                path.push(key.clone());
                match node.get_mut(key) {
                    Some(child) => apply(child, inner, layer, path, unusable),
                    None => unusable.push(UnusableNode {
                        layer: layer.clone(),
                        path: path.clone(),
                        difference: value.clone(),
                    }),
                }
                path.pop();
            }
            _ if key == NEW_NODE => {}
            _ => {
                node.properties.insert(key.clone(), value.clone());
            }
        }
    }
}

/// The generated model and the layers over it. Taking a setting off a layer brings back what
/// the layer below says.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ModelLayers {
    zero: ModelNode,
    layers: Vec<(ModelLayer, Diff)>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ModelLayers {
    /// Layers over this generated model, all empty.
    pub fn new(zero: ModelNode) -> Self {
        Self { zero, layers: Vec::new() }
    }

    fn layer_mut(&mut self, layer: &ModelLayer) -> &mut Diff {
        let at = match self.layers.iter().position(|(l, _)| l == layer) {
            Some(at) => at,
            None => {
                let at = self.layers.partition_point(|(l, _)| l.rank() <= layer.rank());
                self.layers.insert(at, (layer.clone(), Diff::new()));
                at
            }
        };
        &mut self.layers[at].1
    }

    /// A setting at this path of ids, on this layer.
    pub fn set(&mut self, layer: &ModelLayer, path: &[&str], name: &str, value: Json) {
        self.layer_mut(layer).set(path, name, value);
    }

    /// Takes the setting off this layer, so the one below answers again.
    pub fn clear(&mut self, layer: &ModelLayer, path: &[&str], name: &str) {
        self.layer_mut(layer).clear(path, name);
    }

    /// Puts a whole difference on this layer, replacing what it held.
    pub fn put(&mut self, layer: &ModelLayer, difference: Diff) {
        *self.layer_mut(layer) = difference;
    }

    /// What this layer holds.
    pub fn difference(&self, layer: &ModelLayer) -> Option<&Diff> {
        self.layers.iter().find(|(l, _)| l == layer).map(|(_, d)| d)
    }

    /// The model, from the bottom up. What finds no node goes to the unusable list, and stays
    /// in its difference.
    pub fn merged(&self) -> MergedModel {
        let mut root = self.zero.clone();
        let mut unusable = Vec::new();
        for (layer, difference) in &self.layers {
            apply(&mut root, &difference.0, layer, &mut Vec::new(), &mut unusable);
        }
        MergedModel { root, unusable }
    }

    /// Reads this layer from a store, if the store has it.
    pub fn load(&mut self, layer: &ModelLayer, store: &dyn super::ModelStore) -> Result<(), ModelError> {
        if let Some(difference) = store.load()? {
            self.put(layer, difference);
        }
        Ok(())
    }

    /// Writes this layer to a store.
    pub fn save(&self, layer: &ModelLayer, store: &dyn super::ModelStore) -> Result<(), ModelError> {
        store.save(self.difference(layer).unwrap_or(&Diff::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zero() -> ModelNode {
        ModelNode::new("Application")
            .child(ModelNode::new("Views").child(ModelNode::new("A_ListView").with("Caption", "zero")))
    }

    const PATH: [&str; 2] = ["Views", "A_ListView"];

    fn caption(layers: &ModelLayers) -> String {
        let merged = layers.merged();
        merged.root.at(&PATH).and_then(|n| n.text("Caption")).unwrap_or("").to_owned()
    }

    #[test]
    fn the_highest_layer_wins_whatever_order_they_came_in() {
        let mut layers = ModelLayers::new(zero());
        layers.set(&ModelLayer::User, &PATH, "Caption", "user".into());
        layers.set(&ModelLayer::Administrator, &PATH, "Caption", "admin".into());
        layers.set(&ModelLayer::Module("m".into()), &PATH, "Caption", "module".into());
        assert_eq!(caption(&layers), "user");
    }

    #[test]
    fn clearing_brings_back_the_layer_below() {
        let mut layers = ModelLayers::new(zero());
        layers.set(&ModelLayer::Administrator, &PATH, "Caption", "admin".into());
        layers.set(&ModelLayer::User, &PATH, "Caption", "user".into());
        layers.clear(&ModelLayer::User, &PATH, "Caption");
        assert_eq!(caption(&layers), "admin");
        layers.clear(&ModelLayer::Administrator, &PATH, "Caption");
        assert_eq!(caption(&layers), "zero");
        assert_eq!(layers.difference(&ModelLayer::User), Some(&Diff::new()), "nothing is left behind");
    }

    #[test]
    fn modules_stack_in_the_order_they_first_came() {
        let mut layers = ModelLayers::new(zero());
        layers.set(&ModelLayer::Module("z".into()), &PATH, "Caption", "z".into());
        layers.set(&ModelLayer::Module("a".into()), &PATH, "Caption", "a".into());
        assert_eq!(caption(&layers), "a");
    }

    #[test]
    fn a_setting_with_no_node_is_kept_and_comes_back() {
        let mut layers = ModelLayers::new(zero());
        layers.set(&ModelLayer::User, &["Views", "Gone_ListView"], "Caption", "kept".into());
        let merged = layers.merged();
        assert_eq!(merged.unusable.len(), 1);
        assert_eq!(merged.unusable[0].path, ["Views", "Gone_ListView"]);
        assert_eq!(merged.unusable[0].layer, ModelLayer::User);
        // Saved with the rest, so a new version that has the node finds it.
        let saved = layers.difference(&ModelLayer::User).expect("a layer").to_json();
        let mut later = ModelLayers::new(zero());
        later.put(&ModelLayer::User, Diff::parse(&saved).expect("valid"));
        assert_eq!(later.merged().unusable.len(), 1, "still nowhere to go");
        let with_node = {
            let mut root = zero();
            root.get_mut("Views").expect("views").children.push(ModelNode::new("Gone_ListView"));
            let mut l = ModelLayers::new(root);
            l.put(&ModelLayer::User, Diff::parse(&saved).expect("valid"));
            l.merged()
        };
        assert!(with_node.unusable.is_empty());
        assert_eq!(
            with_node.root.at(&["Views", "Gone_ListView"]).and_then(|n| n.text("Caption")),
            Some("kept")
        );
    }

    #[test]
    fn a_difference_can_create_a_node() {
        let difference = Diff::parse(r#"{ "Views": { "B_ListView": { "$new": true, "Caption": "b" } } }"#);
        let mut layers = ModelLayers::new(zero());
        layers.put(&ModelLayer::Administrator, difference.expect("valid"));
        let merged = layers.merged();
        assert!(merged.unusable.is_empty());
        let node = merged.root.at(&["Views", "B_ListView"]).expect("the new node");
        assert_eq!(node.text("Caption"), Some("b"));
        assert!(!node.properties.contains_key(NEW_NODE), "the mark is not a setting");
    }

    #[test]
    fn bad_json_says_so() {
        assert!(Diff::parse("[1]").is_err_and(|e| e.0.contains("object")));
        assert!(Diff::parse("{").is_err_and(|e| e.0.contains("not valid")));
    }
}
