//! What the tests of lists share.
#![allow(
    dead_code,
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "not every test file uses every helper"
)]

use ravenrs_core::{Event, Key, Modifiers, Point, PointerButton, Role, Size};
use ravenrs_layout::LayoutStyle;
use ravenrs_style::Theme;
use ravenrs_widgets::*;

pub fn ui(view: impl View + 'static) -> Ui {
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    settle(&mut ui);
    ui
}

pub fn settle(ui: &mut Ui) {
    for _ in 0..6 {
        ui.update();
    }
}

pub fn press(ui: &mut Ui, key: Key, modifiers: Modifiers) {
    ui.handle_event(&Event::KeyDown { key, modifiers });
    settle(ui);
}

pub fn alt() -> Modifiers {
    Modifiers { alt: true, ..Modifiers::default() }
}

pub fn ctrl() -> Modifiers {
    Modifiers { ctrl: true, ..Modifiers::default() }
}

pub fn shift() -> Modifiers {
    Modifiers { shift: true, ..Modifiers::default() }
}

pub fn tab_to(ui: &mut Ui, name: &str) {
    for _ in 0..80 {
        press(ui, Key::Tab, Modifiers::default());
        if focused_name(ui).as_deref() == Some(name) {
            return;
        }
    }
    panic!("no focus stop called {name}, last was {:?}", focused_name(ui));
}

pub fn focused_name(ui: &mut Ui) -> Option<String> {
    ui.access_tree().focused().and_then(|n| n.name.clone())
}

pub fn hold(ui: &mut Ui, modifiers: Modifiers) {
    ui.handle_event(&Event::KeyUp { key: Key::Other("none".into()), modifiers });
}

pub fn click_at(ui: &mut Ui, at: Point, modifiers: Modifiers) {
    hold(ui, modifiers);
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    settle(ui);
    hold(ui, Modifiers::default());
}

pub fn wheel(ui: &mut Ui, at: Point, delta: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::Scroll { position: at, delta });
    settle(ui);
}

pub fn named(ui: &mut Ui, name: &str) -> ravenrs_core::AccessNode {
    ui.access_tree()
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some(name))
        .cloned()
        .unwrap_or_else(|| panic!("nothing called {name}"))
}

pub fn has(ui: &mut Ui, name: &str) -> bool {
    ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some(name))
}

pub fn of_role(ui: &mut Ui, role: Role) -> Vec<ravenrs_core::AccessNode> {
    ui.access_tree().nodes.iter().filter(|n| n.role == role).cloned().collect()
}

pub fn center(node: &ravenrs_core::AccessNode) -> Point {
    Point::new(node.bounds.x() + node.bounds.width() / 2.0, node.bounds.y() + node.bounds.height() / 2.0)
}

/// A row of a chosen height, named so it can be found.
#[derive(Debug)]
pub struct Tall(pub String, pub f32);

impl View for Tall {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let node =
            cx.container(LayoutStyle { height: Some(self.1), ..LayoutStyle::leaf() }, &[], Paint::None);
        cx.semantics(node, Semantics::new(Role::Label).name(self.0.clone()));
        node
    }
}
