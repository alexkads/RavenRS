//! Screen reader audit of the whole gallery: on every page, every control has
//! a name and every id is unique.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::collections::HashSet;

use component_gallery::Gallery;
use ravenrs::prelude::*;

/// Roles that are operated, so they must say what they are.
fn is_control(role: Role) -> bool {
    matches!(
        role,
        Role::Button
            | Role::Link
            | Role::CheckBox
            | Role::Switch
            | Role::RadioButton
            | Role::TextInput
            | Role::Slider
            | Role::SpinButton
            | Role::ComboBox
            | Role::Tab
            | Role::MenuItem
            | Role::ListBoxOption
    )
}

fn audit(tree: &AccessTree, page: &str) {
    let mut ids = HashSet::new();
    for node in &tree.nodes {
        assert!(ids.insert(node.id), "{page}: duplicate id for {:?} {:?}", node.role, node.name);
        if is_control(node.role) {
            let named = node.name.as_deref().is_some_and(|n| !n.trim().is_empty());
            assert!(named, "{page}: unnamed {:?} at {:?}", node.role, node.bounds);
        }
    }
    if let Some(focus) = tree.focus {
        assert!(tree.get(focus).is_some(), "{page}: focus points at a missing node");
    }
}

#[test]
fn every_page_names_its_controls() {
    let mut ui = Ui::new(Gallery::new(), Theme::raven_dark());
    ui.set_viewport(Size::new(1400.0, 900.0), 1.0);
    ui.update();
    let tree = ui.access_tree();
    let nav = tree.nodes.iter().find(|n| n.role == Role::Button && n.selected == Some(true)).expect("nav");
    let pages: Vec<String> = tree
        .nodes
        .iter()
        .filter(|n| n.role == Role::Button && n.selected.is_some() && n.bounds.x() < nav.bounds.max_x())
        .filter_map(|n| n.name.clone())
        .collect();
    assert!(pages.len() >= 10, "the sidebar lists the pages: {pages:?}");
    let mut audited = 0;
    for page in &pages {
        let tree = ui.access_tree();
        let item = tree.find(Role::Button, page).expect("sidebar item");
        ui.perform_action(item.id, AccessAction::Click);
        ui.update();
        let tree = ui.access_tree();
        // Resource links open documents instead of pages.
        if tree.find(Role::Button, page).and_then(|n| n.selected) != Some(true) {
            continue;
        }
        audit(&tree, page);
        audited += 1;
    }
    assert!(audited >= 12, "audited {audited} pages");
}
