//! What an application adds to its navigation: a brand at the top, counts beside items, and the
//! command palette on Cmd/Ctrl+K with the places this person can open.
#![allow(clippy::expect_used, clippy::panic)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ravenrs_app::{
    EntityInfo, FieldInfo, FieldKind, MemoryDatabase, ObjectSpace, Query, SECURITY_ROLE,
    SECURITY_TYPE_PERMISSION, SECURITY_USER, SECURITY_USER_ROLE, security_entities,
};
use ravenrs_app_ui::{App, NavigationCount, SecuredApp, Session};
use ravenrs_core::{AccessAction, AccessNode, Event, Key, Modifiers, Role, Size};
use ravenrs_style::Theme;
use ravenrs_widgets::{IconName, IconTile, Ui};

fn entity(name: &str, caption: &str, group: &str) -> EntityInfo {
    EntityInfo::new(name)
        .caption(caption)
        .navigation(group)
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).default_property())
}

fn lessons() -> EntityInfo {
    entity("Lesson", "Lessons", "School")
}

fn offerings() -> EntityInfo {
    entity("Offering", "Offerings", "Treasury")
}

struct World {
    ui: Ui,
}

impl World {
    fn over(view: ravenrs_widgets::AnyView) -> Self {
        let mut theme = Theme::raven_dark();
        theme.interaction.reduced_motion = true;
        let mut ui = Ui::new(view, theme);
        ui.set_viewport(Size::new(1200.0, 800.0), 1.0);
        let mut world = Self { ui };
        world.settle();
        world
    }

    fn settle(&mut self) {
        for _ in 0..4 {
            self.ui.update();
        }
    }

    fn nodes(&mut self) -> Vec<AccessNode> {
        self.settle();
        self.ui.access_tree().nodes
    }

    fn named(&mut self, name: &str) -> Option<AccessNode> {
        self.nodes().into_iter().find(|n| n.name.as_deref() == Some(name))
    }

    fn key(&mut self, key: Key, command: bool) {
        let modifiers = Modifiers { meta: command, ..Modifiers::default() };
        self.ui.handle_event(&Event::KeyDown { key, modifiers });
        self.settle();
    }

    fn type_text(&mut self, text: &str) {
        for character in text.chars() {
            self.key(Key::Character(character.to_string()), false);
        }
    }

    fn press(&mut self, name: &str) {
        let id = self.nodes().into_iter().find(|n| n.role == Role::Button && n.name.as_deref() == Some(name));
        let id = id.unwrap_or_else(|| panic!("no button {name}")).id;
        self.ui.perform_action(id, AccessAction::Click);
        self.settle();
    }
}

fn church(count: Rc<Cell<usize>>, seen: Rc<RefCell<Option<Session>>>) -> World {
    let database = MemoryDatabase::new(vec![lessons(), offerings()]).expect("valid");
    let view = App::new("Comunhão")
        .entity_info(lessons())
        .entity_info(offerings())
        .storage(move || database.space())
        .brand(IconTile::new(IconName::Star))
        .brand_subtitle("Central Church")
        .navigation_count("Lesson", move |_| {
            let open = count.get();
            (open > 0).then(|| {
                NavigationCount::new(open.to_string()).spoken(format!("{open} with attendance to take"))
            })
        })
        .with_session(move |session| *seen.borrow_mut() = Some(session.clone()))
        .build()
        .expect("it puts together");
    World::over(view)
}

#[test]
fn the_brand_is_at_the_top_and_keeps_its_name_when_reduced_to_icons() {
    let mut world = church(Rc::new(Cell::new(0)), Rc::default());
    let title = world.named("Comunhão").expect("the title beside the mark");
    let subtitle = world.named("Central Church").expect("the line under it");
    let first = world.named("Lessons").expect("the navigation");
    assert!(title.bounds.y() < first.bounds.y() && subtitle.bounds.y() < first.bounds.y(), "at the top");
    world.key(Key::Character("b".into()), true);
    let mark = world.named("Comunhão").expect("still named");
    assert_eq!(mark.role, Role::Image, "reduced to the mark");
    assert_eq!(mark.description.as_deref(), Some("Central Church"));
    assert!(world.named("Lessons").is_some_and(|n| n.bounds.width() <= 48.0), "the sidebar is reduced");
}

#[test]
fn a_count_shows_beside_its_item_says_its_words_and_follows_the_records() {
    let (count, seen) = (Rc::new(Cell::new(1)), Rc::new(RefCell::new(None)));
    let mut world = church(count.clone(), seen.clone());
    let lessons = world.named("Lessons").expect("the item");
    assert_eq!(lessons.description.as_deref(), Some("1 with attendance to take"));
    assert!(world.named("Offerings").and_then(|n| n.description).is_none(), "only where one is asked for");

    count.set(3);
    seen.borrow().as_ref().expect("opened").records_changed();
    assert_eq!(
        world.named("Lessons").and_then(|n| n.description).as_deref(),
        Some("3 with attendance to take")
    );
    count.set(0);
    seen.borrow().as_ref().expect("opened").records_changed();
    assert!(world.named("Lessons").and_then(|n| n.description).is_none(), "none goes away");
}

#[test]
fn command_k_opens_the_places_to_go_and_enter_goes_there() {
    let seen = Rc::new(RefCell::new(None));
    let mut world = church(Rc::new(Cell::new(0)), seen.clone());
    assert!(
        world
            .nodes()
            .iter()
            .all(|n| n.role != Role::TextInput || n.name.as_deref() != Some("Search commands"))
    );
    world.key(Key::Character("k".into()), true);
    let field = world.nodes().into_iter().find(|n| n.role == Role::TextInput);
    assert!(field.is_some(), "the palette opened with its search");
    world.type_text("offe");
    world.key(Key::Enter, false);
    let session = seen.borrow().clone().expect("opened");
    let view = session.frame().view();
    assert_eq!(view.as_ref().and_then(|v| v.entity.clone()).as_deref(), Some("Offering"));
}

#[test]
fn the_palette_offers_only_what_the_person_can_open() {
    let mut all = security_entities();
    all.extend([lessons(), offerings()]);
    let database = MemoryDatabase::new(all).expect("valid");
    let mut space = database.space();
    let mut role = space.new_object(SECURITY_ROLE).expect("role");
    role.set("name", "Teachers");
    role.set("policy", "read_only_all");
    let role_key = role.get("id");
    space.save(role).expect("saved");
    let mut denied = space.new_object(SECURITY_TYPE_PERMISSION).expect("permission");
    denied.set("role_id", role_key.clone());
    denied.set("target_entity", "Offering");
    denied.set("can_read", "deny");
    space.save(denied).expect("saved");
    let mut user = space.new_object(SECURITY_USER).expect("user");
    user.set("name", "bia");
    user.set("must_change_password", false);
    user.set("password_hash", ravenrs_app::hash_password("bia's password").expect("hashed"));
    let user_key = user.get("id");
    space.save(user).expect("saved");
    let mut link = space.new_object(SECURITY_USER_ROLE).expect("link");
    link.set("user_id", user_key);
    link.set("role_id", role_key);
    space.save(link).expect("saved");
    space.commit().expect("committed");
    assert_eq!(database.space().query(SECURITY_USER, &Query::all()).expect("reads").len(), 1);

    let (source, seen) = (database.clone(), Rc::new(RefCell::new(None::<Session>)));
    let keep = seen.clone();
    let app = SecuredApp::new(
        "Comunhão",
        move || source.space(),
        move |_| {
            let keep = keep.clone();
            App::new("Comunhão")
                .entity_info(lessons())
                .entity_info(offerings())
                .with_session(move |session| *keep.borrow_mut() = Some(session.clone()))
        },
    )
    .demo_credentials("bia", "bia's password");
    let mut world = World::over(app.build().expect("valid"));
    world.press("Sign in");
    let opened = |world: &mut World, typed: &str| {
        world.key(Key::Character("k".into()), true);
        world.type_text(typed);
        world.key(Key::Enter, false);
        let session = seen.borrow().clone().expect("signed in");
        let view = session.frame().view();
        view.as_ref().and_then(|v| v.entity.clone())
    };
    assert_eq!(opened(&mut world, "less").as_deref(), Some("Lesson"));
    assert_eq!(
        opened(&mut world, "offe").as_deref(),
        Some("Lesson"),
        "a place they cannot read is not offered"
    );
}
