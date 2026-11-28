//! What a generated list shows in its cells and rows: the action on each row.
#![allow(clippy::expect_used, clippy::panic)]

use std::rc::Rc;

use ravenrs_app::{
    Action, ActionCategory, Application, CriteriaMode, EntityInfo, FieldInfo, FieldKind, Identity,
    MemoryDatabase, Module, ObjectSpace, Operation, PermissionPolicy, Role, RoleMerge, SecuredObjectSpace,
    SecurityContext, SpaceMaker, UserDirectory, Value, ViewController, security_entities,
};
use ravenrs_app_ui::{AppShell, Session, SystemModule};
use ravenrs_core::{AccessAction, Role as AccessRole};
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

fn bill() -> EntityInfo {
    EntityInfo::new("Bill")
        .caption("Bills")
        .navigation("Money")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).default_property())
        .field(FieldInfo::new("status", FieldKind::Text))
}

/// "Pay" on each row of an open bill, which sets it paid with no code.
struct Paying;

impl ViewController for Paying {
    fn id(&self) -> &str {
        "Paying"
    }

    fn actions(&self) -> Vec<Action> {
        vec![
            Action::simple("Bill.Pay", "Pay")
                .category(ActionCategory::InRow)
                .target_entity("Bill")
                .target_criteria("[status] = 'open'", CriteriaMode::TrueForAll)
                .requires(Operation::Write)
                .icon("check")
                .assign(&bill(), "status = 'paid'"),
        ]
    }
}

struct PayingModule;

impl Module for PayingModule {
    fn name(&self) -> &str {
        "Paying"
    }

    fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
        vec![Box::new(Paying)]
    }
}

fn database() -> MemoryDatabase {
    let mut all = vec![bill()];
    all.extend(security_entities());
    let database = MemoryDatabase::new(all).expect("valid");
    let mut space = database.space();
    for (id, name, status) in [(1.0, "Rent", "open"), (2.0, "Power", "paid")] {
        let mut record = space.new_object("Bill").expect("known");
        record.set("id", id);
        record.set("name", name);
        record.set("status", status);
        space.save(record).expect("waits");
    }
    space.commit().expect("keeps");
    database
}

struct World {
    ui: Ui,
    session: Session,
}

fn world(database: &MemoryDatabase, role: Option<Role>) -> World {
    let mut application = Application::builder("Test")
        .module(SystemModule::new())
        .module(PayingModule)
        .entity(bill())
        .build()
        .expect("valid");
    let shell = match role {
        None => {
            let source = database.clone();
            AppShell::new(application, move || source.space())
        }
        Some(role) => {
            let raw = database.clone();
            let maker: SpaceMaker = Rc::new(move || Box::new(raw.space()) as Box<dyn ObjectSpace>);
            let mut all = vec![bill()];
            all.extend(security_entities());
            let context = Rc::new(SecurityContext::new(
                Identity::new("bia", "bia", vec![role]),
                UserDirectory::new(maker, Vec::new()).expect("roles"),
                all,
                RoleMerge::AnyRole,
            ));
            application.set_security(context.clone());
            let source = database.clone();
            AppShell::new(application, move || {
                SecuredObjectSpace::new(Box::new(source.space()), context.clone())
            })
        }
    };
    let session = shell.session();
    let mut world = World { ui: Ui::new(shell, Theme::raven_dark()), session };
    world.session.open_view("Bill_ListView");
    world.settle();
    world
}

impl World {
    fn settle(&mut self) {
        for _ in 0..4 {
            self.ui.update();
        }
    }

    /// The row buttons of this action, which say the row after the action, as "Pay Rent".
    fn buttons(&mut self, name: &str) -> Vec<(u64, bool)> {
        self.settle();
        self.ui
            .access_tree()
            .nodes
            .iter()
            .filter(|node| {
                node.role == AccessRole::Button
                    && node.name.as_deref().is_some_and(|said| said.starts_with(&format!("{name} ")))
            })
            .map(|node| (node.id, !node.disabled))
            .collect()
    }
}

#[test]
fn an_action_in_row_lights_only_on_the_rows_that_meet_it_and_runs_on_its_row() {
    let database = database();
    let mut world = world(&database, None);
    let pay = world.buttons("Pay");
    assert_eq!(pay.len(), 2, "one button a row: {pay:?}");
    world.settle();
    let above = world
        .ui
        .access_tree()
        .nodes
        .iter()
        .any(|n| n.role == AccessRole::Button && n.name.as_deref() == Some("Pay"));
    assert!(!above, "an action in row shows above the list too");
    assert_eq!(pay.iter().filter(|(_, on)| *on).count(), 1, "only the open bill: {pay:?}");
    let (id, _) = pay.iter().find(|(_, on)| *on).copied().expect("there");
    world.ui.perform_action(id, AccessAction::Click);
    world.settle();
    let rent = database.space().find("Bill", &Value::Number(1.0)).expect("reads").expect("there");
    assert_eq!(rent.text("status"), "paid", "the row's record was paid through the action");
    assert!(world.buttons("Pay").iter().all(|(_, on)| !on), "no open bill is left");
}

#[test]
fn an_action_in_row_is_not_there_for_who_cannot_do_what_it_does() {
    let database = database();
    let mut world = world(&database, Some(Role::new("r").policy(PermissionPolicy::ReadOnlyAll)));
    assert!(world.buttons("Pay").is_empty(), "a reader gets a button to pay");
}
