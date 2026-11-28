//! The generated action that amends a sealed record: it opens the amendment on its form, already
//! pointing to the record, and the commit applies it.
#![allow(clippy::expect_used, clippy::panic)]

use std::rc::Rc;

use ravenrs_app::{
    ActionInput, Amendment, Application, EntityInfo, FieldInfo, FieldKind, Identity, MemoryDatabase,
    ObjectSpace, Operation, PermissionPolicy, Role, RoleMerge, Seal, SecuredObjectSpace, SecurityContext,
    SpaceMaker, UserDirectory, Value, security_entities,
};
use ravenrs_app_ui::{AppShell, Session, SystemModule};
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

fn entities() -> Vec<EntityInfo> {
    vec![
        EntityInfo::new("Lesson")
            .caption("Lessons")
            .navigation("School")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("status", FieldKind::Text))
            .field(FieldInfo::new("present", FieldKind::Integer))
            .sealed(Seal::when("[status] = 'closed'")),
        EntityInfo::new("LessonCorrection")
            .caption("Correct the lesson")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("lesson_id", FieldKind::Reference("Lesson".into())))
            .field(FieldInfo::new("present", FieldKind::Integer))
            .field(FieldInfo::new("reason", FieldKind::LongText))
            .amends(Amendment::correction("Lesson", "lesson_id").copies(["present"]).reason("reason", 10)),
    ]
}

fn database() -> MemoryDatabase {
    let mut all = entities();
    all.extend(security_entities());
    let database = MemoryDatabase::new(all).expect("valid");
    let mut space = database.space();
    for (id, status) in [(1.0, "closed"), (2.0, "open")] {
        let mut lesson = space.new_object("Lesson").expect("known");
        lesson.set("id", id);
        lesson.set("status", status);
        lesson.set("present", 3.0);
        space.save(lesson).expect("waits");
    }
    space.commit().expect("keeps");
    database
}

struct World {
    ui: Ui,
    session: Session,
    database: MemoryDatabase,
}

fn world(database: &MemoryDatabase, role: Option<Role>) -> World {
    let mut builder = Application::builder("Test").module(SystemModule::new());
    for entity in entities() {
        builder = builder.entity(entity);
    }
    let mut application = builder.build().expect("valid");
    let shell = match role {
        None => {
            let source = database.clone();
            AppShell::new(application, move || source.space())
        }
        Some(role) => {
            let raw = database.clone();
            let maker: SpaceMaker = Rc::new(move || Box::new(raw.space()) as Box<dyn ObjectSpace>);
            let mut all = entities();
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
    let mut world = World { ui: Ui::new(shell, Theme::raven_dark()), session, database: database.clone() };
    world.session.open_view("Lesson_ListView");
    world.settle();
    world
}

impl World {
    fn settle(&mut self) {
        for _ in 0..4 {
            self.ui.update();
        }
    }

    fn select(&mut self, id: f64) {
        let lesson = self.database.space().find("Lesson", &Value::Number(id)).expect("reads").expect("there");
        self.session.frame().set_selected(vec![lesson]);
        self.settle();
    }

    fn offered(&self) -> Option<bool> {
        let action = self.session.frame().action("LessonCorrection.Amend")?;
        action.is_active().then(|| action.is_enabled())
    }
}

#[test]
fn a_closed_lesson_offers_its_correction_which_opens_pointing_to_it_and_applies_on_save() {
    let database = database();
    let mut world = world(&database, None);
    // Only a closed lesson takes a correction.
    world.select(2.0);
    assert_eq!(world.offered(), Some(false));
    world.select(1.0);
    assert_eq!(world.offered(), Some(true));

    let action = world.session.frame().action("LessonCorrection.Amend").expect("there");
    assert_eq!(action.caption(), "Correct the lesson");
    world.session.run_action(&action, ActionInput::None);
    world.settle();
    let form = world.session.form_record().expect("the correction is on its form");
    assert_eq!(form.entity(), "LessonCorrection");
    assert_eq!(form.get("lesson_id"), Value::Number(1.0));

    world.session.set_form_value("id", Value::Number(1.0));
    world.session.set_form_value("present", Value::Number(5.0));
    world.session.set_form_value("reason", Value::Text("two were late".into()));
    world.settle();
    let save = world.session.frame().action("Save").expect("there");
    world.session.run_action(&save, ActionInput::None);
    world.settle();
    let lesson = database.space().find("Lesson", &Value::Number(1.0)).expect("reads").expect("there");
    assert_eq!(lesson.get("present"), Value::Number(5.0), "the commit applied the correction");
}

#[test]
fn who_cannot_create_the_correction_does_not_see_the_action() {
    let database = database();
    let reader = Role::new("r").policy(PermissionPolicy::AllowAll).type_permission(
        "LessonCorrection",
        [Operation::Create],
        ravenrs_app::Access::Deny,
    );
    let mut world = world(&database, Some(reader));
    world.select(1.0);
    assert_eq!(world.offered(), None, "the action is offered without Create on the correction");
    // Write on the lesson is not what it needs: Create on the correction is.
    let writer = Role::new("w").policy(PermissionPolicy::ReadOnlyAll).type_permission(
        "LessonCorrection",
        [Operation::Create, Operation::Write],
        ravenrs_app::Access::Allow,
    );
    let mut world = self::world(&database, Some(writer));
    world.select(1.0);
    assert_eq!(world.offered(), Some(true));
}
