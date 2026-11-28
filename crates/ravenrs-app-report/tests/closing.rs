//! The actions and screen of a period closing, run as a person runs them.
#![allow(clippy::expect_used, clippy::panic)]

use std::rc::Rc;

use ravenrs_app::{
    Access, ActionInput, Application, AttachmentStore, EntityInfo, FieldInfo, FieldKind, Identity,
    MemoryAttachmentStore, MemoryDatabase, ObjectSpace, Operation, PeriodClosing, PermissionPolicy, Query,
    Role, RoleMerge, SecuredObjectSpace, SecurityContext, SpaceMaker, UserDirectory, Value,
    security_entities,
};
use ravenrs_app_report::ClosingModule;
use ravenrs_app_ui::{AppShell, Session, SystemModule};
use ravenrs_core::{AccessAction, Role as AccessRole, Size};
use ravenrs_report::Document;
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

fn entities() -> Vec<EntityInfo> {
    let closing = PeriodClosing::new("TermClosing")
        .freezes("Lesson", "[term_id]")
        .requires_none("Lesson", "[status] = 'draft'")
        .reopen_reason(10);
    let term = EntityInfo::new("Term")
        .caption("Term")
        .navigation("School")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).default_property())
        .period_closing(closing.clone());
    let lesson = EntityInfo::new("Lesson")
        .caption("Lesson")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("term_id", FieldKind::Reference("Term".into())))
        .field(FieldInfo::new("status", FieldKind::Text));
    let versions = closing.versions_entity(&term);
    let mut all = vec![term, lesson, versions];
    all.extend(security_entities());
    all
}

fn database() -> MemoryDatabase {
    let database = MemoryDatabase::new(entities()).expect("valid");
    let mut space = database.space();
    let mut term = space.new_object("Term").expect("known");
    term.set("id", 1.0);
    term.set("name", "1º semestre");
    space.save(term).expect("saved");
    let mut lesson = space.new_object("Lesson").expect("known");
    lesson.set("id", 1.0);
    lesson.set("term_id", 1.0);
    lesson.set("status", "draft");
    space.save(lesson).expect("saved");
    space.commit().expect("committed");
    database
}

struct World {
    ui: Ui,
    session: Session,
    database: MemoryDatabase,
}

impl World {
    fn new(database: &MemoryDatabase, files: Rc<MemoryAttachmentStore>, role: Option<Role>) -> Self {
        let module = ClosingModule::new("Term", files, |_space, _term| Ok(Document::default()))
            .approver(|| "ana".to_owned());
        let mut builder = Application::builder("Test").module(SystemModule::new()).module(module);
        for entity in entities().into_iter().take(3) {
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
                let directory = UserDirectory::new(maker, Vec::new()).expect("roles");
                let context = Rc::new(SecurityContext::new(
                    Identity::new("bia", "bia", vec![role]),
                    directory,
                    entities(),
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
        let mut ui = Ui::new(shell, Theme::raven_dark());
        ui.set_viewport(Size::new(1500.0, 1000.0), 1.0);
        let mut world = Self { ui, session, database: database.clone() };
        world.session.open_view("Term_ListView");
        world.settle();
        world
    }

    fn settle(&mut self) {
        for _ in 0..4 {
            self.ui.update();
        }
    }

    fn texts(&mut self) -> Vec<String> {
        self.settle();
        self.ui
            .access_tree()
            .nodes
            .iter()
            .flat_map(|node| [node.name.clone(), node.value.clone(), node.description.clone()])
            .flatten()
            .collect()
    }

    fn says(&mut self, text: &str) -> bool {
        self.texts().iter().any(|found| found.contains(text))
    }

    fn term(&self) -> ravenrs_app::Object {
        self.database.space().find("Term", &Value::Number(1.0)).expect("reads").expect("there")
    }

    fn run(&mut self, action: &str, input: ActionInput) {
        let frame = self.session.frame();
        frame.set_selected(vec![self.term()]);
        self.settle();
        let action = frame.action(action).unwrap_or_else(|| panic!("no action {action}"));
        let asks = action.confirmation_text().is_some();
        let caption = action.caption().to_owned();
        self.session.run_action(&action, input);
        self.settle();
        // A confirmation is answered yes, as a person who goes on.
        if asks {
            self.press(&caption);
        }
    }

    fn press(&mut self, name: &str) {
        self.settle();
        let tree = self.ui.access_tree();
        let found =
            tree.nodes.iter().find(|n| n.role == AccessRole::Button && n.name.as_deref() == Some(name));
        let id = found.unwrap_or_else(|| panic!("no button {name} in {:?}", self.texts())).id;
        self.ui.perform_action(id, AccessAction::Click);
        self.settle();
    }

    fn versions(&self) -> usize {
        self.database.space().query("TermClosing", &Query::all()).expect("reads").len()
    }
}

fn reason(text: &str) -> ActionInput {
    ActionInput::Parameter(Value::Text(text.to_owned()))
}

#[test]
fn a_term_is_approved_reopened_and_its_versions_open_checked_against_the_hash() {
    let database = database();
    let files = Rc::new(MemoryAttachmentStore::default());
    let mut world = World::new(&database, files.clone(), None);

    // A draft lesson keeps the term open: approving is refused and keeps nothing.
    world.run("Term.ApproveClosing", ActionInput::None);
    assert!(world.says("still has Lesson that keep it open"), "{:?}", world.texts());
    assert_eq!(world.versions(), 0);

    let mut space = database.space();
    let mut lesson = space.find("Lesson", &Value::Number(1.0)).expect("reads").expect("there");
    lesson.set("status", "done");
    space.save(lesson).expect("saved");
    space.commit().expect("committed");

    world.run("Term.ApproveClosing", ActionInput::None);
    assert!(world.says("Approved, version 1."), "{:?}", world.texts());
    let version =
        database.space().find("TermClosing", &Value::Text("1/1".into())).expect("reads").expect("kept");
    assert_eq!(version.text("approved_by"), "ana");
    assert_eq!(version.text("digest").len(), 64);

    // A short reason does not reopen; a justified one does.
    world.run("Term.ReopenClosing", reason("curto"));
    assert!(world.says("the reason to reopen needs at least 10"), "{:?}", world.texts());
    world.run("Term.ReopenClosing", reason("faltou uma presença"));
    assert!(world.says("Reopened."), "{:?}", world.texts());
    world.run("Term.ApproveClosing", ActionInput::None);
    assert!(world.says("Approved, version 2."), "{:?}", world.texts());

    world.run("Term.ClosingVersions", ActionInput::None);
    assert!(world.says("Version 1") && world.says("Replaced"), "{:?}", world.texts());
    assert!(world.says("Version 2") && world.says("Current"), "{:?}", world.texts());

    // Each version opens its document, read through the space and checked against its hash.
    let buttons = world.ui.access_tree().nodes.iter().filter(|n| n.name.as_deref() == Some("Open")).count();
    assert_eq!(buttons, 2, "{:?}", world.texts());
    assert_eq!(files.get(&version.text("document")).expect("reads").map(|b| b.is_empty()), Some(false));
}

#[test]
fn a_role_without_write_on_the_term_does_not_get_the_actions_to_approve_or_reopen() {
    let database = database();
    let files = Rc::new(MemoryAttachmentStore::default());
    let reader = Role::new("r").policy(PermissionPolicy::ReadOnlyAll);
    let mut world = World::new(&database, files, Some(reader));
    world.session.frame().set_selected(vec![world.term()]);
    world.settle();
    let frame = world.session.frame();
    for id in ["Term.ApproveClosing", "Term.ReopenClosing"] {
        assert!(frame.action(id).is_none_or(|action| !action.is_enabled()), "{id} is offered");
    }
    let denied = Role::new("d").policy(PermissionPolicy::ReadOnlyAll).type_permission(
        "TermClosing",
        [Operation::Read],
        Access::Deny,
    );
    // Whoever cannot read the versions does not see them, though the term has one.
    let mut space = database.space();
    let mut lesson = space.find("Lesson", &Value::Number(1.0)).expect("reads").expect("there");
    lesson.set("status", "done");
    space.save(lesson).expect("saved");
    space.commit().expect("committed");
    let files = Rc::new(MemoryAttachmentStore::default());
    let mut world = World::new(&database, files.clone(), None);
    world.run("Term.ApproveClosing", ActionInput::None);
    assert_eq!(world.versions(), 1);
    let mut world = World::new(&database, files, Some(denied));
    world.run("Term.ClosingVersions", ActionInput::None);
    assert!(!world.says("Version 1"), "{:?}", world.texts());
}
