//! A list becoming a report, driven the way people use it.
#![allow(clippy::expect_used)]

use ravenrs_app::{Application, EntityInfo, FieldInfo, FieldKind, MemoryDatabase, ObjectSpace};
use ravenrs_app_report::ReportModule;
use ravenrs_app_ui::{AppShell, SystemModule};
use ravenrs_core::{AccessAction, Role, Size};
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

fn account() -> EntityInfo {
    EntityInfo::new("Account")
        .caption("Accounts")
        .navigation("Records")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Name"))
        .field(FieldInfo::new("opening", FieldKind::Money).caption("Opening"))
}

#[test]
fn the_report_action_shows_the_list_in_the_viewer() {
    let database = MemoryDatabase::new(vec![account()]).expect("valid");
    let mut space = database.space();
    let mut object = space.new_object("Account").expect("known");
    object.set("name", "Corrente");
    object.set("opening", 12_345.0);
    space.save(object).expect("saves");
    space.commit().expect("keeps");
    let application = Application::builder("Test")
        .module(SystemModule::new())
        .module(ReportModule::new("Report"))
        .entity(account())
        .build()
        .expect("valid");
    let source = database.clone();
    let shell = AppShell::new(application, move || source.space());
    let session = shell.session();
    let table = session.frame().view().and_then(|view| session.list_table(&view)).expect("a table");
    assert_eq!(table.title, "Accounts");
    assert_eq!(table.columns.len(), 2, "what the list shows, without its hidden key");
    assert_eq!(table.rows.len(), 1);

    let mut ui = Ui::new(shell, Theme::raven_dark());
    ui.set_viewport(Size::new(1200.0, 900.0), 1.0);
    ui.update();
    ui.update();
    let tree = ui.access_tree();
    let button = tree.nodes.iter().find(|n| n.role == Role::Button && n.name.as_deref() == Some("Report"));
    ui.perform_action(button.expect("the action").id, AccessAction::Click);
    ui.update();
    ui.update();
    let shows = ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some("Print"));
    assert!(shows, "the viewer opened, with its print button");
}
