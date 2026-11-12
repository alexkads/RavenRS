//! In a narrow window the navigation is a sheet, and a button in the header opens it.
#![allow(clippy::expect_used, clippy::panic)]

use ravenrs_app::{EntityInfo, FieldInfo, FieldKind};
use ravenrs_app_ui::App;
use ravenrs_core::{AccessAction, Event, Key, Modifiers, Role as AccessRole, Size};
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

fn entity(name: &str, caption: &str, group: &str) -> EntityInfo {
    EntityInfo::new(name)
        .caption(caption)
        .navigation(group)
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).default_property())
}

struct World {
    ui: Ui,
}

impl World {
    fn at(width: f32) -> Self {
        let app = App::new("Shop")
            .entity_info(entity("Order", "Orders", "Sales"))
            .entity_info(entity("Invoice", "Invoices", "Finance"));
        let mut ui = Ui::new(app.build().expect("valid"), Theme::raven_dark());
        ui.set_viewport(Size::new(width, 840.0), 1.0);
        let mut world = Self { ui };
        world.settle();
        world
    }

    fn settle(&mut self) {
        for _ in 0..6 {
            self.ui.update();
        }
    }

    fn names(&mut self) -> Vec<String> {
        self.settle();
        self.ui.access_tree().nodes.iter().filter_map(|n| n.name.clone()).collect()
    }

    fn shows(&mut self, name: &str) -> bool {
        self.names().iter().any(|found| found == name)
    }

    fn press(&mut self, name: &str) {
        self.settle();
        let tree = self.ui.access_tree();
        let found = tree
            .nodes
            .iter()
            .filter(|n| {
                matches!(
                    n.role,
                    AccessRole::Button
                        | AccessRole::Link
                        | AccessRole::MenuItem
                        | AccessRole::TreeItem
                        | AccessRole::ListItem
                )
            })
            .find(|n| n.name.as_deref() == Some(name))
            .map(|n| n.id);
        let id = found.unwrap_or_else(|| panic!("nothing called {name} in {:?}", self.names()));
        self.ui.perform_action(id, AccessAction::Click);
        self.settle();
    }

    fn escape(&mut self) {
        self.ui.handle_event(&Event::KeyDown { key: Key::Escape, modifiers: Modifiers::default() });
        self.settle();
    }
}

const TOGGLE: &str = "Toggle sidebar";

#[test]
fn a_narrow_window_has_a_button_that_opens_the_navigation() {
    for width in [460.0, 719.0] {
        let mut world = World::at(width);
        assert!(world.shows(TOGGLE), "no way to the navigation at {width}");
        assert!(!world.shows("Invoices"), "the sheet starts closed at {width}");
        world.press(TOGGLE);
        assert!(world.shows("Invoices"), "the button opened nothing at {width}: {:?}", world.names());
    }
}

#[test]
fn choosing_a_screen_shows_it_and_closes_the_sheet() {
    let mut world = World::at(460.0);
    world.press(TOGGLE);
    world.press("Invoices");
    // The title in the header says where we are; the sheet with the other screen is gone.
    assert!(world.shows("Invoices"));
    assert!(!world.shows("Orders"), "the sheet stayed open: {:?}", world.names());
}

#[test]
fn escape_closes_the_sheet() {
    let mut world = World::at(460.0);
    world.press(TOGGLE);
    assert!(world.shows("Invoices"));
    world.escape();
    assert!(!world.shows("Invoices"), "Escape left it open: {:?}", world.names());
}

#[test]
fn a_wide_window_keeps_the_navigation_beside_the_view_without_the_button() {
    let mut world = World::at(1200.0);
    assert!(world.shows("Invoices"));
    assert!(!world.shows(TOGGLE));
}
