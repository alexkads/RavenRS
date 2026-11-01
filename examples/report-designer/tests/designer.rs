//! The designer driven as a user drives it: templates, the wizard, dragging a
//! field onto a band, undo, and an accessibility audit of its controls.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use ravenrs::core::{Event, Key, Modifiers, PointerButton};
use ravenrs::prelude::*;
use report_designer::Designer;

fn designer() -> Ui {
    let mut ui = Ui::new(Designer::new(), Theme::raven_light());
    ui.set_viewport(Size::new(1440.0, 900.0), 1.0);
    ui.update();
    ui
}

fn press(ui: &mut Ui, role: Role, name_starts: &str) {
    let tree = ui.access_tree();
    let id = tree
        .nodes
        .iter()
        .find(|n| n.role == role && n.name.as_deref().is_some_and(|t| t.starts_with(name_starts)))
        .unwrap_or_else(|| panic!("no {role:?} '{name_starts}'"))
        .id;
    ui.perform_action(id, AccessAction::Click);
    ui.update();
    ui.update();
}

fn names(ui: &mut Ui, role: Role) -> Vec<String> {
    ui.access_tree().nodes.iter().filter(|n| n.role == role).filter_map(|n| n.name.clone()).collect()
}

fn audit(ui: &mut Ui, screen: &str) {
    let tree = ui.access_tree();
    for n in &tree.nodes {
        if matches!(
            n.role,
            Role::Button
                | Role::CheckBox
                | Role::Switch
                | Role::RadioButton
                | Role::TextInput
                | Role::ComboBox
                | Role::Slider
                | Role::SpinButton
                | Role::Tab
        ) {
            assert!(
                n.name.as_deref().is_some_and(|s| !s.trim().is_empty()),
                "{screen}: unnamed {:?} at {:?}",
                n.role,
                n.bounds
            );
        }
    }
}

#[test]
fn opens_with_the_template_gallery_and_every_template_loads() {
    let mut ui = designer();
    audit(&mut ui, "gallery");
    assert!(names(&mut ui, Role::Dialog).contains(&"Novo relatório".to_owned()));
    for template in ["Fatura", "Vendas por região", "Lista de clientes"] {
        press(&mut ui, Role::Button, template);
        assert!(names(&mut ui, Role::Dialog).is_empty(), "{template} closes the gallery");
        audit(&mut ui, template);
        // Back to the gallery from the ribbon.
        press(&mut ui, Role::Button, "Modelos");
    }
}

#[test]
fn the_wizard_generates_a_report_and_shows_it() {
    let mut ui = designer();
    press(&mut ui, Role::Button, "Assistente");
    audit(&mut ui, "wizard");
    assert!(names(&mut ui, Role::Dialog).contains(&"Assistente de relatório".to_owned()));
    press(&mut ui, Role::Button, "Gerar relatório");
    assert!(names(&mut ui, Role::Dialog).is_empty());
    let tree = ui.access_tree();
    assert!(
        tree.nodes.iter().any(|n| n.name.as_deref().is_some_and(|s| s.starts_with("Página 1 de "))),
        "the preview shows the generated pages"
    );
}

#[test]
fn dragging_a_field_onto_a_band_adds_it_and_undo_removes_it() {
    let mut ui = designer();
    press(&mut ui, Role::Button, "Em branco");
    let fields = |ui: &mut Ui| {
        ui.access_tree().nodes.iter().filter(|n| n.name.as_deref().is_some_and(|s| s == "[Customer]")).count()
    };
    assert_eq!(fields(&mut ui), 0);
    // The field list row reads as text; find it and the detail band's caption.
    let tree = ui.access_tree();
    let source = tree.find(Role::Label, "Customer").expect("field in the list").bounds.center();
    let band = tree.find(Role::Label, "Detalhe").expect("detail band caption").bounds;
    let target = Point::new(band.x() + 200.0, band.max_y() + 14.0);
    ui.handle_event(&Event::PointerMove { position: source });
    ui.handle_event(&Event::PointerDown { position: source, button: PointerButton::Primary });
    ui.update();
    ui.handle_event(&Event::PointerMove { position: target });
    ui.update();
    ui.handle_event(&Event::PointerUp { position: target, button: PointerButton::Primary });
    ui.update();
    ui.update();
    assert_eq!(fields(&mut ui), 1, "the field landed on the band");

    ui.handle_event(&Event::KeyDown {
        key: Key::Character("z".into()),
        modifiers: Modifiers { meta: true, ..Modifiers::default() },
    });
    ui.update();
    ui.update();
    assert_eq!(fields(&mut ui), 0, "undo removes it");
    ui.handle_event(&Event::KeyDown {
        key: Key::Character("y".into()),
        modifiers: Modifiers { meta: true, ..Modifiers::default() },
    });
    ui.update();
    ui.update();
    assert_eq!(fields(&mut ui), 1, "redo brings it back");
}

#[test]
fn the_large_template_really_has_ten_thousand_rows() {
    let mut ui = designer();
    press(&mut ui, Role::Button, "Vendas · 10.000 linhas");
    let tree = ui.access_tree();
    let status = tree
        .nodes
        .iter()
        .filter_map(|n| n.name.as_deref())
        .find(|s| s.contains("linhas de dados"))
        .expect("status bar");
    assert!(status.contains("10000 linhas"), "{status}");
    let pages = tree
        .nodes
        .iter()
        .filter_map(|n| n.name.as_deref())
        .find(|s| s.starts_with("Página 1 de "))
        .expect("page counter");
    let count: usize = pages.trim_start_matches("Página 1 de ").parse().expect("a number");
    assert!(count > 300, "{pages}");
}
