//! The example changes how it shows a list by the model alone.
#![allow(clippy::expect_used)]

use ravenrs_core::{Role, Size};
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

#[test]
fn it_opens_as_a_list_with_the_other_ways_on_its_menu() {
    let mut ui = Ui::new(quick_plan::app(), Theme::raven_dark());
    ui.set_viewport(Size::new(1200.0, 800.0), 1.0);
    ui.update();
    ui.update();
    let tree = ui.access_tree();
    assert!(tree.nodes.iter().any(|n| n.role == Role::Cell));
    let says = |text: &str| {
        tree.nodes.iter().any(|n| n.name.as_deref() == Some(text) || n.value.as_deref() == Some(text))
    };
    assert!(says("Fundação"));
    assert!(says("Ana"), "a reference shows the name");
}
