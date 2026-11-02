//! What must hold for any mix of settings on any mix of layers.
#![allow(clippy::expect_used)]

use proptest::prelude::*;
use ravenrs_app::{Diff, ModelLayer, ModelLayers, ModelNode};

fn zero() -> ModelNode {
    ModelNode::new("Application")
        .child(ModelNode::new("A").with("P", "zero").child(ModelNode::new("B").with("P", "zero")))
}

const PATHS: [&[&str]; 2] = [&["A"], &["A", "B"]];

fn layer(index: u8) -> ModelLayer {
    match index {
        0 => ModelLayer::Module("first".into()),
        1 => ModelLayer::Module("second".into()),
        2 => ModelLayer::Administrator,
        _ => ModelLayer::User,
    }
}

/// `(layer, path, value)`; `None` clears the setting.
fn steps() -> impl Strategy<Value = Vec<(u8, usize, Option<i64>)>> {
    prop::collection::vec((0..4_u8, 0..2_usize, prop::option::of(0..100_i64)), 0..40)
}

proptest! {
    #[test]
    fn the_highest_layer_that_still_holds_a_setting_answers(steps in steps()) {
        let mut layers = ModelLayers::new(zero());
        // Modules stack in the order they first came, so fix that order.
        layers.set(&layer(0), &["A"], "Touched", true.into());
        layers.set(&layer(1), &["A"], "Touched", true.into());
        let mut held: [[Option<i64>; 2]; 4] = [[None; 2]; 4];
        for (l, p, value) in steps {
            match value {
                Some(value) => layers.set(&layer(l), PATHS[p], "P", value.into()),
                None => layers.clear(&layer(l), PATHS[p], "P"),
            }
            held[l as usize][p] = value;
        }
        let merged = layers.merged();
        prop_assert!(merged.unusable.is_empty());
        for (p, path) in PATHS.iter().enumerate() {
            let expected = (0..4).rev().find_map(|l| held[l][p]);
            let node = merged.root.at(path).expect("the node");
            match expected {
                Some(value) => prop_assert_eq!(node.integer("P"), Some(value)),
                None => prop_assert_eq!(node.text("P"), Some("zero")),
            }
        }
    }

    #[test]
    fn differences_survive_going_to_json_and_back(steps in steps()) {
        let mut layers = ModelLayers::new(zero());
        // Modules stack in the order they first came, so fix that order.
        layers.set(&layer(0), &["A"], "Touched", true.into());
        layers.set(&layer(1), &["A"], "Touched", true.into());
        for (l, p, value) in steps {
            if let Some(value) = value {
                layers.set(&layer(l), PATHS[p], "P", value.into());
            }
        }
        let mut again = ModelLayers::new(zero());
        for l in 0..4 {
            if let Some(difference) = layers.difference(&layer(l)) {
                again.put(&layer(l), Diff::parse(&difference.to_json()).expect("valid"));
            }
        }
        prop_assert_eq!(again.merged(), layers.merged());
    }

    #[test]
    fn merging_changes_nothing_in_the_layers(steps in steps()) {
        let mut layers = ModelLayers::new(zero());
        for (l, p, value) in steps {
            if let Some(value) = value {
                layers.set(&layer(l), PATHS[p], "P", value.into());
            }
        }
        let before = layers.clone();
        prop_assert_eq!(layers.merged(), before.merged());
        prop_assert_eq!(layers, before);
    }
}
