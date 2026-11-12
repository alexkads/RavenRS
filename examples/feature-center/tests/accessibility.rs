//! Every demonstration of the center reaches screen readers and the keyboard, as the one who signs
//! in sees it, with nothing written for them.
#![allow(clippy::expect_used)]

use ravenrs_app::ObjectSpace;
use ravenrs_app_a11y::Audit;
use ravenrs_core::{AccessAction, Event, Key, Modifiers, Role, Size};
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

use feature_center::seed::PASSWORD;
use feature_center::{memory_database, watched};

fn settle(ui: &mut Ui) {
    for _ in 0..4 {
        ui.update();
    }
}

fn type_into(ui: &mut Ui, field: &str, text: &str) {
    settle(ui);
    let tree = ui.access_tree();
    let id = tree
        .nodes
        .iter()
        .find(|n| n.role == Role::TextInput && n.name.as_deref().is_some_and(|t| t.starts_with(field)))
        .expect("the field")
        .id;
    ui.perform_action(id, AccessAction::Focus);
    for character in text.chars() {
        ui.handle_event(&Event::KeyDown {
            key: Key::Character(character.to_string()),
            modifiers: Modifiers::default(),
        });
    }
    settle(ui);
}

#[test]
fn the_sign_in_and_every_demonstration_pass_the_audit() {
    let database = memory_database().expect("the database");
    let app = watched(move || Box::new(database.space()) as Box<dyn ObjectSpace>, |_| {});
    let mut ui = Ui::new(app.build().expect("it puts together"), Theme::raven_dark());
    ui.set_viewport(Size::new(1600.0, 1200.0), 1.0);
    settle(&mut ui);
    let audit = Audit::new().theme(Theme::raven_light()).theme(Theme::raven_glass());
    audit.screen(&mut ui, "entrar").assert_clean();

    type_into(&mut ui, "Nome", "ana");
    type_into(&mut ui, "Senha", PASSWORD);
    let tree = ui.access_tree();
    let enter = tree.find(Role::Button, "Entrar").expect("the button");
    ui.perform_action(enter.id, AccessAction::Click);
    settle(&mut ui);

    let report = audit.open("Novo").skip("Sair").app(&mut ui);
    assert!(report.places.len() >= 30, "{:?}", report.places);
    report.assert_clean();
}
