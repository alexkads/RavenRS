//! The example does what the framework promises, and stays short.
#![allow(clippy::expect_used)]

use ravenrs_core::{Event, Point, PointerButton, Role, Size};
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

fn click(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
    ui.update();
}

#[test]
fn it_opens_with_its_records_and_its_commands_in_portuguese() {
    let mut ui = Ui::new(quick_app::app(), Theme::raven_dark());
    ui.set_viewport(Size::new(1100.0, 720.0), 1.0);
    ui.update();
    ui.update();
    let says = |ui: &mut Ui, text: &str| {
        ui.access_tree()
            .nodes
            .iter()
            .any(|n| n.name.as_deref() == Some(text) || n.value.as_deref() == Some(text))
    };
    assert!(says(&mut ui, "Conta corrente"));
    assert!(says(&mut ui, "Itaú"), "the bank's name, and not its code");
    assert!(says(&mut ui, "Corrente"), "the option's caption");
    let new = ui
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.role == Role::Button && n.name.as_deref() == Some("Novo"))
        .expect("the command")
        .bounds
        .center();
    click(&mut ui, new);
    assert!(says(&mut ui, "Salvar"), "the form");
    assert!(says(&mut ui, "Saldo inicial"));
}

#[test]
fn it_stays_under_100_lines() {
    let lines =
        include_str!("../src/lib.rs").lines().count() + include_str!("../src/main.rs").lines().count();
    assert!(lines < 100, "{lines} lines");
}
