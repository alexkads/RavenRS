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

/// A list of bills whose situation is a badge, in danger by an appearance rule when overdue, and
/// how many badges in danger it draws.
fn danger_badges(situations: &[&str], model: Option<(&str, &str)>) -> usize {
    use ravenrs_app::{AppearanceRule, ModelLayer};
    use ravenrs_widgets::Tone;
    let bill = EntityInfo::new("Bill")
        .caption("Contas")
        .navigation("Finanças")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("description", FieldKind::Text).default_property())
        .field(FieldInfo::new("situation", FieldKind::Text).badge())
        .appearance(AppearanceRule::new("late", ["situation"]).criteria("[situation] = 'Vencida'").font(Tint::Danger));
    let mut all = vec![bill.clone()];
    all.extend(security_entities());
    let database = MemoryDatabase::new(all).expect("valid");
    let mut space = database.space();
    for (at, situation) in situations.iter().enumerate() {
        let mut record = space.new_object("Bill").expect("known");
        record.set("id", at as f64 + 1.0);
        record.set("description", format!("Conta {at}"));
        record.set("situation", *situation);
        space.save(record).expect("waits");
    }
    space.commit().expect("keeps");
    let mut application =
        Application::builder("Casa").module(SystemModule::new()).entity(bill).build().expect("valid");
    if let Some((name, value)) = model {
        application.layers_mut().set(
            &ModelLayer::User,
            &["Views", "Bill_ListView", "Columns", "situation"],
            name,
            value.into(),
        );
        application.refresh_model().expect("valid");
    }
    let shell = AppShell::new(application, move || database.space());
    let session = shell.session();
    let theme = Theme::raven_dark();
    let mut ui = Ui::new(shell, theme);
    session.open_view("Bill_ListView");
    for _ in 0..6 {
        ui.update();
    }
    let edge = format!("{:?}", Tone::Danger.color(&theme).with_alpha(0.28));
    format!("{:?}", ui.scene()).matches(&edge).count()
}

#[test]
fn a_badge_on_a_plain_field_takes_the_tone_of_its_record() {
    assert_eq!(danger_badges(&["Em dia", "Em dia"], None), 0, "no record overdue, no badge in danger");
    assert_eq!(danger_badges(&["Vencida", "Em dia"], None), 1, "only the overdue one");
    assert_eq!(danger_badges(&["Vencida", "Vencida"], None), 2);
}

#[test]
fn the_model_turns_the_badge_of_a_column_off_and_on() {
    assert_eq!(danger_badges(&["Vencida"], Some(("Display", "Text"))), 0, "the column in the model wins");
    assert_eq!(danger_badges(&["Vencida"], Some(("Display", "Badge"))), 1);
}

#[test]
fn the_model_says_which_field_goes_under_another() {
    use ravenrs_app::ModelLayer;
    let texts = |detail: &str| {
        let mut all = vec![member()];
        all.extend(security_entities());
        let database = MemoryDatabase::new(all).expect("valid");
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
        application.layers_mut().set(
            &ModelLayer::User,
            &["Views", "Member_ListView", "Columns", "name"],
            "Detail",
            detail.into(),
        );
        application.refresh_model().expect("valid");
        let shell = AppShell::new(application, move || database.space());
        let session = shell.session();
        let mut ui = Ui::new(shell, Theme::raven_dark());
        session.open_view("Member_ListView");
        for _ in 0..6 {
            ui.update();
        }
        said(&mut ui)
    };
    let none = texts("");
    assert!(none.iter().any(|t| t == "Ana"), "an empty Detail takes the line away: {none:?}");
    assert!(!none.iter().any(|t| t.contains("Ana · ")), "{none:?}");
    let status = texts("status");
    assert!(status.iter().any(|t| t.contains("Ana · Ativo")), "the status under the name: {status:?}");
}
