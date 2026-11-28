//! How a generated list draws a cell: a value as a badge of its tone, and another field under it,
//! which whoever cannot read it does not see (#865).
#![allow(clippy::expect_used, clippy::panic)]

use std::rc::Rc;

use ravenrs_app::{
    Access, Application, EntityInfo, EnumOption, FieldInfo, FieldKind, Identity, MemoryDatabase, ObjectSpace,
    Operation, PermissionPolicy, Role, RoleMerge, SecuredObjectSpace, SecurityContext, SpaceMaker, Tint,
    UserDirectory, security_entities,
};
use ravenrs_app_ui::{AppShell, SystemModule};
use ravenrs_core::Role as AccessRole;
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

fn member() -> EntityInfo {
    let status = FieldKind::Enum(vec![
        EnumOption::new("active", "Ativo").tint(Tint::Success),
        EnumOption::new("away", "Afastado").tint(Tint::Warning),
    ]);
    EntityInfo::new("Member")
        .caption("Membros")
        .navigation("Pessoas")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).default_property().list_detail("phone"))
        .field(FieldInfo::new("phone", FieldKind::Text))
        .field(FieldInfo::new("status", status))
}

fn ui(role: Option<Role>) -> Ui {
    let mut all = vec![member()];
    all.extend(security_entities());
    let database = MemoryDatabase::new(all.clone()).expect("valid");
    let mut space = database.space();
    let mut ana = space.new_object("Member").expect("known");
    ana.set("id", 1.0);
    ana.set("name", "Ana");
    ana.set("phone", "(11) 9999-0000");
    ana.set("status", "active");
    space.save(ana).expect("waits");
    space.commit().expect("keeps");
    let mut application =
        Application::builder("Igreja").module(SystemModule::new()).entity(member()).build().expect("valid");
    let shell = match role {
        None => AppShell::new(application, move || database.space()),
        Some(role) => {
            let raw = database.clone();
            let maker: SpaceMaker = Rc::new(move || Box::new(raw.space()) as Box<dyn ObjectSpace>);
            let context = Rc::new(SecurityContext::new(
                Identity::new("bia", "bia", vec![role]),
                UserDirectory::new(maker, Vec::new()).expect("roles"),
                all,
                RoleMerge::AnyRole,
            ));
            application.set_security(context.clone());
            AppShell::new(application, move || {
                SecuredObjectSpace::new(Box::new(database.space()), context.clone())
            })
        }
    };
    let session = shell.session();
    let mut ui = Ui::new(shell, Theme::raven_dark());
    session.open_view("Member_ListView");
    for _ in 0..6 {
        ui.update();
    }
    ui
}

fn said(ui: &mut Ui) -> Vec<String> {
    ui.update();
    ui.access_tree()
        .nodes
        .iter()
        .filter(|n| n.role != AccessRole::Button)
        .flat_map(|n| [n.name.clone(), n.value.clone()])
        .flatten()
        .collect()
}

#[test]
fn a_status_shows_as_a_badge_and_the_phone_under_the_name() {
    let mut ui = ui(None);
    let texts = said(&mut ui);
    assert!(texts.iter().any(|t| t == "Ativo"), "the status, as a badge: {texts:?}");
    // The cell reads both lines, the name first; the row, as it is exported, with " · " between.
    assert!(texts.iter().any(|t| t == "Ana (11) 9999-0000"), "the name and the phone under it: {texts:?}");
    assert!(texts.iter().any(|t| t.contains("Ana · (11) 9999-0000")), "one line to export: {texts:?}");
}

#[test]
fn whoever_cannot_read_the_phone_sees_only_the_name() {
    let hidden = Role::new("r").policy(PermissionPolicy::ReadOnlyAll).member_permission(
        "Member",
        ["phone"],
        None,
        [Operation::Read],
        Access::Deny,
    );
    let mut ui = ui(Some(hidden));
    let texts = said(&mut ui);
    assert!(texts.iter().any(|t| t == "Ana"), "{texts:?}");
    assert!(
        !texts.iter().any(|t| t.contains("9999")),
        "the phone shows to whoever cannot read it: {texts:?}"
    );
}
