//! The summary of what falls due as a message when the session opens (#860).
#![allow(clippy::expect_used, clippy::panic)]

use ravenrs_app::{DueDigest, EntityInfo, FieldInfo, FieldKind, ObjectSpace, Value};
use ravenrs_app_ui::{App, Language};
use ravenrs_core::{AccessAction, Size};
use ravenrs_criteria::DateTime;
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

fn invoice() -> EntityInfo {
    EntityInfo::new("Invoice")
        .caption("Contas")
        .navigation("Finanças")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("description", FieldKind::Text).default_property())
        .field(FieldInfo::new("due_on", FieldKind::Date))
        .field(FieldInfo::new("amount", FieldKind::Money))
        .field(FieldInfo::new("status", FieldKind::Text))
        .due_digest(
            DueDigest::new("due_on").pending("[status] = 'open'").amount("amount").words("conta", "contas"),
        )
}

fn ui(notice: bool) -> Ui {
    let today = ravenrs_app::local_today();
    let app = App::new("Casa")
        .language(Language::BrazilianPortuguese)
        .entity_info(invoice())
        .due_digest_notice(notice)
        .seed(move |space: &mut dyn ObjectSpace| {
            for (name, days, cents) in [("Luz", -1, 48_000.0), ("Água", 2, 9_000.0), ("Gás", 30, 7_000.0)] {
                let mut bill = space.new_object("Invoice").map_err(|e| e.to_string())?;
                bill.set("description", name);
                bill.set("due_on", Value::Date(DateTime { days: today.days + days, seconds: 0 }));
                bill.set("amount", cents);
                bill.set("status", "open");
                space.save(bill).map_err(|e| e.to_string())?;
            }
            Ok(())
        });
    let mut ui = Ui::new(app.build().expect("valid"), Theme::raven_dark());
    ui.set_viewport(Size::new(1200.0, 800.0), 1.0);
    for _ in 0..6 {
        ui.update();
    }
    ui
}

fn texts(ui: &mut Ui) -> String {
    for _ in 0..4 {
        ui.update();
    }
    let tree = ui.access_tree();
    let all: Vec<String> =
        tree.nodes.iter().flat_map(|n| [n.name.clone(), n.value.clone()]).flatten().collect();
    all.join(" | ")
}

#[test]
fn the_session_opens_with_what_falls_due_and_its_button_opens_the_list() {
    let mut ui = ui(true);
    let said = texts(&mut ui);
    let notice = said.split(" | ").find(|text| text.contains("conta vencida")).unwrap_or_default().to_owned();
    assert_eq!(
        notice, "1 conta vencida e 1 a vencer — Luz (R$ 480,00), Água (R$ 90,00). Total: R$ 570,00.",
        "{said}"
    );
    let tree = ui.access_tree();
    let open = tree.nodes.iter().find(|n| n.name.as_deref() == Some("Abrir")).expect("the button").id;
    ui.perform_action(open, AccessAction::Click);
    let said = texts(&mut ui);
    assert!(said.contains("Luz") && said.contains("Água") && said.contains("Gás"), "the list: {said}");
}

#[test]
fn an_application_can_turn_the_notice_off() {
    let mut ui = ui(false);
    let said = texts(&mut ui);
    assert!(!said.contains("conta vencida"), "{said}");
}

#[test]
fn each_person_chooses_the_notices_beside_their_account_and_it_is_kept() {
    use ravenrs_app::{MemorySettings, digest_preference};
    let store = MemorySettings::default();
    let app = App::new("Casa")
        .language(Language::BrazilianPortuguese)
        .entity_info(invoice())
        .due_digest_notice(false)
        .settings(store.clone())
        .signed_in_remotely("ana".to_owned(), || {});
    let mut ui = Ui::new(app.build().expect("valid"), Theme::raven_dark());
    ui.set_viewport(Size::new(1200.0, 800.0), 1.0);
    let click = |ui: &mut Ui, name: &str| {
        let said = texts(ui);
        let id = ui
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("no {name} in {said}"))
            .id;
        ui.perform_action(id, AccessAction::Click);
    };
    click(&mut ui, "Avisos de vencimento");
    let said = texts(&mut ui);
    assert!(said.contains("Contas") && said.contains("3 dias antes"), "the choice of each entity: {said}");
    click(&mut ui, "7 dias antes");
    texts(&mut ui);
    assert_eq!(digest_preference(&store, "ana", "Invoice").advance_days, 7);
    click(&mut ui, "Desligado");
    texts(&mut ui);
    assert!(!digest_preference(&store, "ana", "Invoice").enabled, "kept for ana");
}
