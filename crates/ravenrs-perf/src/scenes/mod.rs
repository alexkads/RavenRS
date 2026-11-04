//! Every scene the gate measures.

use crate::PerfScene;

pub mod apps;
mod components;
mod curve;

/// All the scenes, in the order they are reported.
pub fn all() -> Vec<PerfScene> {
    let mut scenes = curve::scenes();
    scenes.extend(components::scenes());
    scenes.extend(apps::scenes());
    scenes
}
