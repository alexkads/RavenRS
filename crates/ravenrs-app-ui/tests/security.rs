//! Signing in and what each person sees, driven through the accessibility tree.
#![allow(clippy::expect_used, clippy::panic)]

use ravenrs_app::{
    Access, AuditMode, AuditSettings, EntityInfo, FieldInfo, FieldKind, MemoryDatabase, ObjectSpace,
    Operation, PermissionPolicy, Query, Role, SECURITY_AUDIT_ENTRY, SECURITY_ROLE, SECURITY_USER,
    SECURITY_USER_ROLE, Value, audit_entity, security_entities,
};
use ravenrs_app_ui::{App, Language, SecuredApp};
use ravenrs_core::{AccessAction, Event, Key, Modifiers, Role as AccessRole};
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

fn order() -> EntityInfo {
    EntityInfo::new("Order")
        .caption("Orders")
        .navigation("Sales")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).default_property())
        .field(FieldInfo::new("cost", FieldKind::Integer))
}

fn invoice() -> EntityInfo {
    EntityInfo::new("Invoice")
        .caption("Invoices")
        .navigation("Finance")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).default_property())
}

fn database() -> MemoryDatabase {
    let mut entities = vec![order(), invoice(), audit_entity()];
    entities.extend(security_entities());
    let database = MemoryDatabase::new(entities).expect("valid");
    let mut space = database.space();
    let mut record = space.new_object("Order").expect("order");
    record.set("name", "First");
    record.set("cost", 10.0);
    space.save(record).expect("saved");
    space.commit().expect("committed");
    database
}

fn secured(database: &MemoryDatabase, language: Language) -> SecuredApp {
    let source = database.clone();
    SecuredApp::new(
        "Shop",
        move || source.space(),
        |_person| App::new("Shop").entity_info(order()).entity_info(invoice()),
    )
    .language(language)
    .audit(AuditSettings::new(AuditMode::Full))
}

struct World {
    ui: Ui,
}

impl World {
    fn over(app: SecuredApp) -> Self {
        let mut world = Self { ui: Ui::new(app.build().expect("valid"), Theme::raven_dark()) };
        world.settle();
        world
    }

    fn settle(&mut self) {
        for _ in 0..4 {
            self.ui.update();
        }
    }

    fn names(&mut self, role: AccessRole) -> Vec<String> {
        self.settle();
        self.ui.access_tree().nodes.iter().filter(|n| n.role == role).filter_map(|n| n.name.clone()).collect()
    }

    fn has_button(&mut self, name: &str) -> bool {
        self.names(AccessRole::Button).iter().any(|found| found == name)
    }

    fn press(&mut self, name: &str) {
        self.settle();
        let tree = self.ui.access_tree();
        let found =
            tree.nodes.iter().find(|n| n.role == AccessRole::Button && n.name.as_deref() == Some(name));
        let id =
            found.unwrap_or_else(|| panic!("no button {name} in {:?}", self.names(AccessRole::Button))).id;
        self.ui.perform_action(id, AccessAction::Click);
        self.settle();
    }

    fn type_into(&mut self, field: &str, text: &str) {
        self.settle();
        let tree = self.ui.access_tree();
        let id = tree
            .nodes
            .iter()
            .find(|n| {
                n.role == AccessRole::TextInput && n.name.as_deref().is_some_and(|t| t.starts_with(field))
            })
            .unwrap_or_else(|| panic!("no field {field} in {:?}", self.names(AccessRole::TextInput)))
            .id;
        self.ui.perform_action(id, AccessAction::Focus);
        self.settle();
        for character in text.chars() {
            self.ui.handle_event(&Event::KeyDown {
                key: Key::Character(character.to_string()),
                modifiers: Modifiers::default(),
            });
        }
        self.settle();
    }

    fn says(&mut self, text: &str) -> bool {
        self.settle();
        self.ui.access_tree().nodes.iter().any(|n| {
            n.name.as_deref().is_some_and(|name| name.contains(text))
                || n.value.as_deref().is_some_and(|value| value.contains(text))
        })
    }

    fn sign_in(&mut self, name: &str, password: &str) {
        self.type_into("Name", name);
        self.type_into("Password", password);
        self.press("Sign in");
    }
}

fn first_administrator(world: &mut World) {
    world.type_into("Name", "ana");
    world.type_into("New password", "correct horse");
    world.type_into("Repeat the password", "correct horse");
    world.press("Create administrator");
}

/// Someone with a role that reads, and a password they chose, already in storage.
fn add_reader(database: &MemoryDatabase, role: impl FnOnce(&mut dyn ObjectSpace, &Value)) {
    let mut space = database.space();
    let mut record = space.new_object(SECURITY_ROLE).expect("role");
    record.set("name", "Readers");
    record.set("policy", "read_only_all");
    let key = record.get("id");
    space.save(record).expect("saved");
    role(&mut space, &key);
    let mut user = space.new_object(SECURITY_USER).expect("user");
    user.set("name", "bia");
    user.set("must_change_password", false);
    user.set("password_hash", ravenrs_app::hash_password("bia's password").expect("hashed"));
    let user_key = user.get("id");
    space.save(user).expect("saved");
    let mut link = space.new_object(SECURITY_USER_ROLE).expect("link");
    link.set("user_id", user_key);
    link.set("role_id", key);
    space.save(link).expect("saved");
    space.commit().expect("committed");
}

fn with_admin() -> (World, MemoryDatabase) {
    let database = database();
    let mut world = World::over(secured(&database, Language::English));
    first_administrator(&mut world);
    world.press("Sign out");
    (world, database)
}

#[test]
fn a_new_installation_asks_for_the_first_person_and_nothing_shows_before_that() {
    let database = database();
    let mut world = World::over(secured(&database, Language::English));
    assert!(world.has_button("Create administrator"));
    assert!(!world.says("Orders"));
    assert!(!world.says("First"));
    world.type_into("Name", "ana");
    world.type_into("New password", "short");
    world.type_into("Repeat the password", "short");
    world.press("Create administrator");
    assert!(world.says("at least 8"));
}

#[test]
fn two_passwords_that_differ_create_nobody() {
    let database = database();
    let mut world = World::over(secured(&database, Language::English));
    world.type_into("Name", "ana");
    world.type_into("New password", "correct horse");
    world.type_into("Repeat the password", "correct house");
    world.press("Create administrator");
    assert!(world.says("not the same"));
    assert!(database.space().query(SECURITY_USER, &Query::all()).expect("reads").is_empty());
}

#[test]
fn the_administrator_gets_in_with_everything_and_can_leave() {
    let database = database();
    let mut world = World::over(secured(&database, Language::English));
    first_administrator(&mut world);
    assert!(world.says("Orders"));
    assert!(world.says("Users"));
    assert!(world.says("Roles"));
    assert!(world.says("Audit trail"));
    assert!(world.says("Application model"));
    world.press("Sign out");
    assert!(world.has_button("Sign in"));
    assert!(!world.says("Orders"));
}

#[test]
fn a_wrong_password_stays_at_the_door_and_says_the_same_as_a_wrong_name() {
    let (mut world, _database) = with_admin();
    world.sign_in("ana", "wrong password");
    assert!(world.says("is not right"));
    assert!(!world.says("Orders"));
    world.type_into("Password", "correct horse");
    world.press("Sign in");
    assert!(world.says("Orders"));

    let (mut world, _database) = with_admin();
    world.sign_in("nobody", "wrong password");
    assert!(world.says("is not right"));
    assert!(!world.says("Orders"));
}

#[test]
fn what_is_typed_does_not_stay_on_the_screen() {
    let (mut world, _database) = with_admin();
    world.sign_in("ana", "wrong password");
    assert!(!world.says("wrong password"));
}

#[test]
fn someone_given_a_password_has_to_choose_their_own_first() {
    let (mut world, database) = with_admin();
    add_reader(&database, |_, _| {});
    let mut space = database.space();
    let mut user = space
        .query(SECURITY_USER, &Query::all())
        .expect("reads")
        .into_iter()
        .find(|user| user.get("name") == Value::Text("bia".into()))
        .expect("there");
    user.set("must_change_password", true);
    space.save(user).expect("saved");
    space.commit().expect("committed");

    world.sign_in("bia", "bia's password");
    assert!(world.says("Choose your own"));
    assert!(!world.says("Orders"));
    world.type_into("Current password", "bia's password");
    world.type_into("New password", "bia chose this");
    world.type_into("Repeat the password", "bia chose this");
    world.press("Save password");
    assert!(world.says("Orders"), "{:?}", world.names(AccessRole::Button));
    world.press("Sign out");
    world.sign_in("bia", "bia chose this");
    assert!(world.says("Orders"));
}

#[test]
fn a_person_only_sees_where_they_can_go_and_the_buttons_that_work() {
    let (mut world, database) = with_admin();
    add_reader(&database, |space, role| {
        let mut denied = space.new_object(ravenrs_app::SECURITY_TYPE_PERMISSION).expect("permission");
        denied.set("role_id", role.clone());
        denied.set("target_entity", "Invoice");
        denied.set("can_read", "deny");
        space.save(denied).expect("saved");
    });
    world.sign_in("bia", "bia's password");
    assert!(world.says("Orders"));
    assert!(!world.says("Invoices"), "no way to an entity they cannot read");
    assert!(!world.says("Users"));
    assert!(!world.says("Audit trail"));
    assert!(!world.says("Application model"));
    assert!(world.says("First"));
    // They only read: the button is there, faded, and does nothing.
    let before = database.space().query("Order", &Query::all()).expect("reads").len();
    world.press("New");
    assert_eq!(database.space().query("Order", &Query::all()).expect("reads").len(), before);
    assert!(!world.has_button("Save"));
}

#[test]
fn a_field_they_cannot_read_says_it_is_protected_on_the_form() {
    let database = database();
    let source = database.clone();
    let role = Role::new("Sales").policy(PermissionPolicy::ReadOnlyAll).member_permission(
        "Order",
        ["cost"],
        None,
        [Operation::Read],
        Access::Deny,
    );
    let shown = std::rc::Rc::new(std::cell::RefCell::new(None));
    let keep = shown.clone();
    let app = SecuredApp::new(
        "Shop",
        move || source.space(),
        move |_| {
            let keep = keep.clone();
            App::new("Shop").entity_info(order()).entity_info(invoice()).with_session(move |session| {
                *keep.borrow_mut() = Some(session.clone());
            })
        },
    )
    .role(role)
    .audit(AuditSettings::new(AuditMode::Full));
    let mut world = World::over(app);
    first_administrator(&mut world);
    world.press("Sign out");
    let mut space = database.space();
    let sales = space
        .query(SECURITY_ROLE, &Query::all())
        .expect("reads")
        .into_iter()
        .find(|role| role.get("name") == Value::Text("Sales".into()))
        .expect("there");
    let mut user = space.new_object(SECURITY_USER).expect("user");
    user.set("name", "bia");
    user.set("must_change_password", false);
    user.set("password_hash", ravenrs_app::hash_password("bia's password").expect("hashed"));
    let key = user.get("id");
    space.save(user).expect("saved");
    let mut link = space.new_object(SECURITY_USER_ROLE).expect("link");
    link.set("user_id", key);
    link.set("role_id", sales.get("id"));
    space.save(link).expect("saved");
    space.commit().expect("committed");

    world.sign_in("bia", "bia's password");
    let session = shown.borrow().clone().expect("opened");
    let record = database.space().query("Order", &Query::all()).expect("reads").remove(0);
    session.open_detail("Order", &record.get("id"));
    world.settle();
    assert!(world.says("Protected"));
    assert!(!world.says("10"));
}

#[test]
fn changes_land_on_the_trail_under_the_name_of_who_made_them() {
    let database = database();
    let source = database.clone();
    let shown = std::rc::Rc::new(std::cell::RefCell::new(None));
    let keep = shown.clone();
    let app = SecuredApp::new(
        "Shop",
        move || source.space(),
        move |_| {
            let keep = keep.clone();
            App::new("Shop").entity_info(order()).entity_info(invoice()).with_session(move |session| {
                *keep.borrow_mut() = Some(session.clone());
            })
        },
    )
    .audit(AuditSettings::new(AuditMode::Full));
    let mut world = World::over(app);
    first_administrator(&mut world);
    let session = shown.borrow().clone().expect("opened");
    let record = database.space().query("Order", &Query::all()).expect("reads").remove(0);
    session.open_detail("Order", &record.get("id"));
    session.set_form_value("name", Value::Text("Renamed".into()));
    world.settle();
    world.press("Save");
    let trail = database.space().query(SECURITY_AUDIT_ENTRY, &Query::all()).expect("reads");
    let entry = trail
        .iter()
        .find(|entry| entry.get("target_entity") == Value::Text("Order".into()))
        .expect("written");
    assert_eq!(entry.get("author"), Value::Text("ana".into()));
    assert_eq!(entry.get("new_value"), Value::Text("Renamed".into()));
}

#[test]
fn an_administrator_gives_a_password_and_it_never_shows() {
    let database = database();
    let source = database.clone();
    let shown = std::rc::Rc::new(std::cell::RefCell::new(None));
    let keep = shown.clone();
    let app = SecuredApp::new(
        "Shop",
        move || source.space(),
        move |_| {
            let keep = keep.clone();
            App::new("Shop").entity_info(order()).entity_info(invoice()).with_session(move |session| {
                *keep.borrow_mut() = Some(session.clone());
            })
        },
    )
    .audit(AuditSettings::new(AuditMode::Full));
    let mut world = World::over(app);
    first_administrator(&mut world);
    add_reader(&database, |_, _| {});
    let session = shown.borrow().clone().expect("opened");
    let bia = database
        .space()
        .query(SECURITY_USER, &Query::all())
        .expect("reads")
        .into_iter()
        .find(|user| user.get("name") == Value::Text("bia".into()))
        .expect("there");
    session.open_detail(SECURITY_USER, &bia.get("id"));
    world.settle();
    assert!(!world.says("argon2"));
    let action = session.frame().action("SetPassword").expect("there");
    session.run_action(&action, ravenrs_app::ActionInput::None);
    world.settle();
    world.type_into("New password", "given by ana");
    world.type_into("Repeat the password", "given by ana");
    world.press("Save password");
    assert!(world.says("Password changed"));
    world.press("Sign out");
    world.sign_in("bia", "given by ana");
    assert!(world.says("Choose your own"));
}

#[test]
fn the_model_editor_changes_the_person_s_layer_and_is_closed_to_others() {
    let database = database();
    let source = database.clone();
    let shown = std::rc::Rc::new(std::cell::RefCell::new(None));
    let keep = shown.clone();
    let app = SecuredApp::new(
        "Shop",
        move || source.space(),
        move |_| {
            let keep = keep.clone();
            App::new("Shop").entity_info(order()).entity_info(invoice()).with_session(move |session| {
                *keep.borrow_mut() = Some(session.clone());
            })
        },
    )
    .audit(AuditSettings::new(AuditMode::Full));
    let mut world = World::over(app);
    first_administrator(&mut world);
    let session = shown.borrow().clone().expect("opened");
    session.open_view("ModelEditor_DashboardView");
    world.settle();
    assert!(world.says("Pick a node"));
    session.set_model_setting(&["NavigationItems", "Sales", "Order"], "Caption", "Selling").expect("changed");
    world.settle();
    assert!(world.says("Selling"));
    let index_error =
        session.set_model_setting(&["Views", "Order_ListView", "Columns", "cost"], "Index", "soon");
    assert!(index_error.is_err(), "a number stays a number");
    session.restore_model_settings().expect("restored");
    world.settle();
    assert!(!world.says("Selling"));

    world.press("Sign out");
    add_reader(&database, |_, _| {});
    world.sign_in("bia", "bia's password");
    let session = shown.borrow().clone().expect("opened");
    assert!(session.set_model_setting(&["NavigationItems", "Sales"], "Caption", "Mine").is_err());
    assert!(session.restore_model_settings().is_err());
    session.open_view("ModelEditor_DashboardView");
    world.settle();
    assert!(world.says("not allowed"));
    assert!(!world.says("Pick a node"));
}

#[test]
fn it_says_everything_in_portuguese() {
    let database = database();
    let mut world = World::over(secured(&database, Language::BrazilianPortuguese));
    assert!(world.has_button("Criar administrador"));
    world.type_into("Nome", "ana");
    world.type_into("Nova senha", "correct horse");
    world.type_into("Repita a senha", "correct horse");
    world.press("Criar administrador");
    assert!(world.says("Usuários"));
    assert!(world.says("Trilha de auditoria"));
    world.press("Sair");
    assert!(world.has_button("Entrar"));
}

#[test]
fn storage_that_does_not_know_the_entities_is_told_up_front() {
    let database = MemoryDatabase::new(vec![order()]).expect("valid");
    let source = database.clone();
    let result =
        SecuredApp::new("Shop", move || source.space(), |_| App::new("Shop").entity_info(order())).build();
    assert!(result.is_err());
    let mut entities = security_entities();
    entities.push(order());
    let database = MemoryDatabase::new(entities).expect("valid");
    let result = SecuredApp::new("Shop", move || database.space(), |_| App::new("Shop").entity_info(order()))
        .audit(AuditSettings::new(AuditMode::Full))
        .build();
    assert!(result.is_err());
}
