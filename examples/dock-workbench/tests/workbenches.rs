//! Each workbench builds whole, with the parts it is made of where screen readers find them.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use dock_workbench::Workbench;
use ravenrs::prelude::*;

fn names(which: Workbench) -> Vec<String> {
    let mut ui = Ui::new(which.app(), Theme::raven_dark());
    ui.set_viewport(Size::new(1440.0, 900.0), 1.0);
    for _ in 0..4 {
        ui.update();
    }
    ui.access_tree().nodes.iter().filter_map(|node| node.name.clone()).collect()
}

fn has(names: &[String], name: &str) -> bool {
    names.iter().any(|seen| seen == name)
}

#[test]
fn the_code_editor_has_its_rail_side_bar_split_code_panels_and_hidden_outline() {
    let names = names(Workbench::Editor);
    for name in [
        "Explorer",
        "Search",
        "Source control",
        "main.rs",
        "sales.rs",
        "Terminal",
        "Problems",
        "Outline, hidden in the right dock",
        "Workbench",
    ] {
        assert!(has(&names, name), "{name} is missing: {names:?}");
    }
}

#[test]
fn the_ide_has_its_menus_toolbox_hidden_tree_over_properties_and_a_floating_search() {
    let names = names(Workbench::Ide);
    for name in [
        "File",
        "Build",
        "Toolbox, hidden in the left dock",
        "Server Explorer, hidden in the left dock",
        "Solution Explorer",
        "Properties",
        "Error List",
        "Output",
        "Find and Replace, floating",
        "order.rs",
    ] {
        assert!(has(&names, name), "{name} is missing: {names:?}");
    }
}

#[test]
fn the_chat_app_has_its_side_bar_conversation_and_artifact_with_no_tabs() {
    let names = names(Workbench::Chat);
    for name in ["Hide left dock", "Hide right dock", "New chat", "Sales report", "Workbench"] {
        assert!(has(&names, name), "{name} is missing: {names:?}");
    }
    let mut ui = Ui::new(Workbench::Chat.app(), Theme::raven_dark());
    ui.set_viewport(Size::new(1440.0, 900.0), 1.0);
    ui.update();
    ui.update();
    let tabs = ui.access_tree().nodes.iter().filter(|node| node.role == Role::Tab).count();
    assert_eq!(tabs, 0, "one panel to a group shows no tab");
}

#[test]
fn the_dock_fills_the_window_under_the_bars_of_each() {
    for which in Workbench::ALL {
        let mut ui = Ui::new(which.app(), Theme::raven_dark());
        ui.set_viewport(Size::new(1440.0, 900.0), 1.0);
        for _ in 0..4 {
            ui.update();
        }
        let tree = ui.access_tree();
        let area = tree.nodes.iter().find(|n| n.name.as_deref() == Some("Workbench")).expect("the dock");
        assert!(area.bounds.height() > 700.0, "{which:?}: the dock is {:?}", area.bounds);
        assert!(area.bounds.width() > 1300.0, "{which:?}: the dock is {:?}", area.bounds);
    }
}

#[test]
fn each_names_itself_from_the_command_line() {
    for which in Workbench::ALL {
        assert_eq!(Workbench::named(which.word()), Some(which));
    }
    assert_eq!(Workbench::named("other"), None);
}
