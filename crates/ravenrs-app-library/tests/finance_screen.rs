//! Settling a bill on the screen: the review opens on a window, filled with today and what is
//! open, and OK settles what it says (#859).
#![allow(clippy::expect_used, clippy::panic)]

use ravenrs_app::{Application, MemoryDatabase, ObjectSpace, Query, Value};
use ravenrs_app_library::LibraryModule;
use ravenrs_app_library::finance::{self, COMMITMENT, LEDGER_ENTRY, SETTLE_BILL};
use ravenrs_app_ui::{AppShell, Session, SystemModule};
use ravenrs_core::{AccessAction, Role};
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

#[test]
fn settle_opens_the_review_and_ok_settles_what_it_says() {
    let database = MemoryDatabase::new(finance::entities()).expect("entities fit");
    let mut space = database.space();
    let mut bill = space.new_object(COMMITMENT).expect("known");
    bill.set("id", 1.0);
    bill.set("kind", "payable");
    bill.set("description", "Aluguel");
    bill.set("amount", 10_000.0);
    bill.set("status", "open");
    space.insert(bill).expect("waits");
    space.commit().expect("kept");

    let application = Application::builder("Casa")
        .module(SystemModule::new())
        .module(LibraryModule::new().finance())
        .build();
    let source = database.clone();
    let shell = AppShell::new(application.expect("valid"), move || source.space());
    let session: Session = shell.session();
    let mut ui = Ui::new(shell, Theme::raven_dark());
    let settle = |ui: &mut Ui| {
        for _ in 0..4 {
            ui.update();
        }
    };
    session.open_detail(COMMITMENT, &Value::Number(1.0));
    settle(&mut ui);
    let action = session.frame().action(SETTLE_BILL).expect("there");
    session.run_action(&action, ravenrs_app::ActionInput::None);
    settle(&mut ui);
    let said = |ui: &mut Ui, text: &str| {
        ui.access_tree()
            .nodes
            .iter()
            .any(|n| [&n.name, &n.value].iter().any(|t| t.as_deref().is_some_and(|t| t.contains(text))))
    };
    for field in ["Of the bill", "Interest", "Fine", "Discount", "To the cash book"] {
        assert!(said(&mut ui, field), "the review shows {field}");
    }
    let ok = ui
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.role == Role::Button && n.name.as_deref() == Some("OK"))
        .map(|n| n.id)
        .expect("the OK of the window");
    ui.perform_action(ok, AccessAction::Click);
    settle(&mut ui);
    let paid = database.space().find(COMMITMENT, &Value::Number(1.0)).expect("reads").expect("there");
    assert_eq!(paid.text("status"), "paid", "OK settled what was open");
    let entries = database.space().query(LEDGER_ENTRY, &Query::all()).expect("reads");
    assert_eq!(entries.len(), 1, "one entry in the cash book");
}
