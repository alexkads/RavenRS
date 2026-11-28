//! The numbers above a list and what an empty list says to each person (#865).
#![allow(clippy::expect_used, clippy::panic)]

use ravenrs_app::{
    EntityInfo, FieldInfo, FieldKind, ListIndicator, MemoryDatabase, ObjectSpace, SECURITY_MEMBER_PERMISSION,
    SECURITY_ROLE, SECURITY_USER, SECURITY_USER_ROLE, security_entities,
};
use ravenrs_app_ui::{App, SecuredApp};
use ravenrs_core::{AccessAction, Event, Key, Modifiers, Role as AccessRole, Size};
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

fn bill() -> EntityInfo {
    EntityInfo::new("Bill")
        .caption("Bills")
        .navigation("Finance")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).default_property())
        .field(FieldInfo::new("amount", FieldKind::Integer))
        .field(FieldInfo::new("status", FieldKind::Text))
        .indicator(ListIndicator::new("Abertas", "").filter("[status] = 'open'"))
        .indicator(
            ListIndicator::new("Total aberto", "amount:Sum")
                .filter("[status] = 'open'")
                .note("Somente em aberto"),
        )
}

fn note() -> EntityInfo {
    EntityInfo::new("Note")
        .caption("Notes")
        .navigation("Finance")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).default_property())
        .empty_text("Nenhuma nota.")
        .empty_hint("Comece pelo título.")
}

fn database() -> MemoryDatabase {
    let mut entities = vec![bill(), note()];
    entities.extend(security_entities());
    let database = MemoryDatabase::new(entities).expect("valid");
    let mut space = database.space();
    for (name, amount, status) in [("Luz", 1_200.0, "open"), ("Água", 300.0, "open"), ("Gás", 50.0, "paid")]
    {
        let mut record = space.new_object("Bill").expect("bill");
        record.set("name", name);
        record.set("amount", amount);
        record.set("status", status);
        space.save(record).expect("saved");
    }
    space.commit().expect("committed");
    database
}

struct World {
    ui: Ui,
}

impl World {
    fn new(ui: Ui) -> Self {
        let mut world = Self { ui };
        world.ui.set_viewport(Size::new(1400.0, 900.0), 1.0);
        world.settle();
        world
    }

    fn settle(&mut self) {
        for _ in 0..5 {
            self.ui.update();
        }
    }

    fn texts(&mut self) -> String {
        self.settle();
        let tree = self.ui.access_tree();
        let all: Vec<String> = tree
            .nodes
            .iter()
            .flat_map(|n| [n.name.clone(), n.value.clone(), n.description.clone()])
            .flatten()
            .collect();
        all.join(" | ")
    }

    fn click(&mut self, name: &str) {
        self.settle();
        let tree = self.ui.access_tree();
        let found = tree.nodes.iter().rev().find(|n| n.name.as_deref() == Some(name));
        let id = found.unwrap_or_else(|| panic!("nothing called {name}: {}", self.texts())).id;
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
            .unwrap_or_else(|| panic!("no field {field}"))
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
}

fn plain() -> World {
    let app = App::new("Shop").entity_info(bill()).entity_info(note());
    World::new(Ui::new(
        app.seed(|space: &mut dyn ObjectSpace| {
            for (name, amount, status) in
                [("Luz", 1_200.0, "open"), ("Água", 300.0, "open"), ("Gás", 50.0, "paid")]
            {
                let mut record = space.new_object("Bill").map_err(|e| e.to_string())?;
                record.set("name", name);
                record.set("amount", amount);
                record.set("status", status);
                space.save(record).map_err(|e| e.to_string())?;
            }
            Ok(())
        })
        .build()
        .expect("valid"),
        Theme::raven_dark(),
    ))
}

#[test]
fn the_numbers_above_a_list_count_and_sum_what_they_filter() {
    let mut world = plain();
    world.click("Bills");
    let said = world.texts();
    assert!(
        said.contains("ABERTAS | 2")
            && said.contains("TOTAL ABERTO | 1,500")
            && said.contains("Somente em aberto"),
        "{said}"
    );
    assert!(
        said.contains("1500") || said.contains("1,500") || said.contains("1.500"),
        "the open sum: {said}"
    );
}

#[test]
fn an_empty_list_gives_the_next_step_to_whom_may_create() {
    let mut world = plain();
    world.click("Notes");
    let said = world.texts();
    assert!(said.contains("Nenhuma nota.") && said.contains("Comece pelo título."), "{said}");
    let news = world.ui.access_tree().nodes.iter().filter(|n| n.name.as_deref() == Some("New")).count();
    assert_eq!(news, 2, "the toolbar's and the empty state's: {said}");
    // The button of the empty state opens a new record, as the toolbar's does.
    world.click("New");
    let said = world.texts();
    assert!(said.contains("Save"), "a new record opens: {said}");
}

/// Someone who reads everything but the amount of a bill.
fn reader(database: &MemoryDatabase) {
    let mut space = database.space();
    let mut role = space.new_object(SECURITY_ROLE).expect("role");
    role.set("name", "Readers");
    role.set("policy", "read_only_all");
    let key = role.get("id");
    space.save(role).expect("saved");
    let mut hidden = space.new_object(SECURITY_MEMBER_PERMISSION).expect("permission");
    hidden.set("role_id", key.clone());
    hidden.set("target_entity", "Bill");
    hidden.set("fields", "amount");
    hidden.set("can_read", "deny");
    space.save(hidden).expect("saved");
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

#[test]
fn whom_may_not_read_the_amount_sees_restricted_never_zero_and_no_next_step() {
    let database = database();
    let source = database.clone();
    let app = SecuredApp::new(
        "Shop",
        move || source.space(),
        |_| App::new("Shop").entity_info(bill()).entity_info(note()),
    );
    let mut world = World::new(Ui::new(app.build().expect("valid"), Theme::raven_dark()));
    world.type_into("Name", "ana");
    world.type_into("New password", "correct horse");
    world.type_into("Repeat the password", "correct horse");
    world.click("Create administrator");
    world.click("Sign out");
    reader(&database);
    world.type_into("Name", "bia");
    world.type_into("Password", "bia's password");
    world.click("Sign in");
    world.click("Bills");
    let said = world.texts();
    assert!(said.contains("TOTAL ABERTO | Restricted to your role"), "{said}");
    assert!(said.contains("ABERTAS | 2"), "the count needs no amount: {said}");
    assert!(!said.contains("1500") && !said.contains("1,500"), "{said}");
    world.click("Notes");
    let said = world.texts();
    assert!(said.contains("Nenhuma nota. Records show up here once someone allowed to adds them."), "{said}");
    assert!(!said.contains("Comece pelo título"), "{said}");
}

#[test]
fn the_numbers_go_one_under_the_other_in_a_narrow_window() {
    let mut world = plain();
    world.click("Bills");
    let tops = |world: &mut World| {
        world.settle();
        let tree = world.ui.access_tree();
        let top = |name: &str| {
            let node = tree.nodes.iter().find(|n| n.name.as_deref() == Some(name));
            node.unwrap_or_else(|| panic!("no {name}")).bounds.origin.y
        };
        (top("ABERTAS"), top("TOTAL ABERTO"))
    };
    let (first, second) = tops(&mut world);
    assert!((first - second).abs() < 1.0, "side by side in a wide window: {first} and {second}");
    world.ui.set_viewport(Size::new(600.0, 900.0), 1.0);
    let (first, second) = tops(&mut world);
    assert!(second > first + 20.0, "one under the other below 720: {first} and {second}");
}
