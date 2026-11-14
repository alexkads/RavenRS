//! A screen of signing in arranged by the application (`SecuredApp::door`), with checking the
//! password, the permissions and the audit trail still in the framework.
#![allow(clippy::expect_used, clippy::panic)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ravenrs_app::{
    AuditMode, AuditSettings, EntityInfo, FieldInfo, FieldKind, MemoryDatabase, ObjectSpace, Query,
    SECURITY_AUDIT_ENTRY, SECURITY_USER, Value, audit_entity, security_entities,
};
use ravenrs_app_ui::{App, DoorParts, SecuredApp, Session, SignInAction};
use ravenrs_core::{AccessAction, AccessNode, Event, Key, Modifiers, Role as AccessRole, Size};
use ravenrs_style::{TextStyle, Theme};
use ravenrs_widgets::{AnyView, Button, ButtonStyle, Divider, HStack, Text, VStack};

const LEGEND: &str = "CENTRAL CHURCH";
const TITLE: &str = "Sign in to the church";
const ASIDE: &str = "Morning Star";

fn order() -> EntityInfo {
    EntityInfo::new("Order")
        .caption("Orders")
        .navigation("Sales")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).default_property())
}

fn database() -> MemoryDatabase {
    let mut entities = vec![order(), audit_entity()];
    entities.extend(security_entities());
    let database = MemoryDatabase::new(entities).expect("valid");
    let mut space = database.space();
    let mut record = space.new_object("Order").expect("order");
    record.set("name", "First");
    space.save(record).expect("saved");
    space.commit().expect("committed");
    database
}

/// The arrangement of the design: legend, title and help, the fields, Sign in beside going back,
/// a divider and chips of a demonstration that only fill in.
fn arranged(parts: &DoorParts) -> AnyView {
    let chip = |label: &str, name: &'static str, password: &'static str| {
        let filler = parts.clone();
        Button::new(label).style(ButtonStyle::Secondary).on_click(move || filler.fill(name, password))
    };
    let mut row: Vec<AnyView> = vec![Box::new(parts.sign_in_button())];
    row.extend(parts.actions().into_iter().map(|b| Box::new(b.style(ButtonStyle::Secondary)) as AnyView));
    Box::new(
        VStack::new((
            Text::new(LEGEND).style(TextStyle::Caption),
            Text::new(parts.title()).style(TextStyle::Title),
            Text::new("Use the name and password the church gave you.").style(TextStyle::Callout),
            parts.name_field(),
            parts.password_field(),
            parts.error(),
            HStack::new(row).spacing(8.0),
            Divider::horizontal(),
            Text::new("DEMONSTRATION PROFILES").style(TextStyle::Caption),
            HStack::new((
                chip("Administrators", "ana", "correct horse"),
                chip("Treasury", "ana", "not the password"),
            ))
            .spacing(8.0)
            .wrap(),
        ))
        .spacing(12.0),
    )
}

/// What the test sees of the application behind the door.
#[derive(Default)]
struct Behind {
    /// How many times the application was made, which happens only for someone signed in.
    made: Cell<u32>,
    session: RefCell<Option<Session>>,
    back: Cell<u32>,
}

fn secured(database: &MemoryDatabase, behind: &Rc<Behind>) -> SecuredApp {
    let (source, seen, back) = (database.clone(), behind.clone(), behind.clone());
    SecuredApp::new(
        TITLE,
        move || source.space(),
        move |_person| {
            seen.made.set(seen.made.get() + 1);
            let keep = seen.clone();
            App::new("Shop").entity_info(order()).with_session(move |session| {
                *keep.session.borrow_mut() = Some(session.clone());
            })
        },
    )
    .audit(AuditSettings::new(AuditMode::Full))
    .sign_in_action(SignInAction::new("Change church", move || back.back.set(back.back.get() + 1)))
}

fn composed(database: &MemoryDatabase, behind: &Rc<Behind>) -> SecuredApp {
    secured(database, behind).door(arranged).aside(Text::new(ASIDE).style(TextStyle::Title))
}

struct World {
    ui: Ui,
}

use ravenrs_widgets::Ui;

impl World {
    fn over(app: SecuredApp) -> Self {
        Self::at(app, 1200.0)
    }

    fn at(app: SecuredApp, width: f32) -> Self {
        let mut ui = Ui::new(app.build().expect("valid"), Theme::raven_dark());
        ui.set_viewport(Size::new(width, 800.0), 1.0);
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

    fn node(&mut self, role: AccessRole, name: &str) -> Option<AccessNode> {
        self.nodes().into_iter().find(|n| n.role == role && n.name.as_deref() == Some(name))
    }

    fn named(&mut self, name: &str) -> AccessNode {
        let nodes = self.nodes();
        let names: Vec<_> = nodes.iter().filter_map(|n| n.name.clone()).collect();
        nodes
            .into_iter()
            .find(|n| n.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("no {name} in {names:?}"))
    }

    fn field(&mut self, label: &str) -> AccessNode {
        self.nodes()
            .into_iter()
            .find(|n| {
                n.role == AccessRole::TextInput && n.name.as_deref().is_some_and(|t| t.starts_with(label))
            })
            .unwrap_or_else(|| panic!("no field {label}"))
    }

    fn has_button(&mut self, name: &str) -> bool {
        self.node(AccessRole::Button, name).is_some()
    }

    fn says(&mut self, text: &str) -> bool {
        self.nodes().iter().any(|n| {
            n.name.as_deref().is_some_and(|name| name.contains(text))
                || n.value.as_deref().is_some_and(|value| value.contains(text))
        })
    }

    fn press(&mut self, name: &str) {
        let id = self.node(AccessRole::Button, name).unwrap_or_else(|| panic!("no button {name}")).id;
        self.ui.perform_action(id, AccessAction::Click);
        self.settle();
    }

    fn key(&mut self, key: Key) {
        self.ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers::default() });
        self.settle();
    }

    fn typed(&mut self, text: &str) {
        for character in text.chars() {
            self.key(Key::Character(character.to_string()));
        }
    }

    fn type_into(&mut self, label: &str, text: &str) {
        let id = self.field(label).id;
        self.ui.perform_action(id, AccessAction::Focus);
        self.settle();
        self.typed(text);
    }

    fn inside(&mut self) -> bool {
        self.has_button("Sign out")
    }
}

/// Creates the first administrator, `ana`, through the framework's screen and leaves.
fn with_ana(app: SecuredApp) -> World {
    let mut world = World::over(app);
    assert!(world.has_button("Create administrator"), "the first person is the framework's screen");
    world.type_into("Name", "ana");
    world.type_into("New password", "correct horse");
    world.type_into("Repeat the password", "correct horse");
    world.press("Create administrator");
    assert!(world.inside());
    world.press("Sign out");
    world
}

#[test]
fn the_application_arranges_the_door() {
    let behind = Rc::new(Behind::default());
    let mut world = with_ana(composed(&database(), &behind));
    let (legend, title) = (world.named(LEGEND), world.named(TITLE));
    assert!(legend.bounds.y() < title.bounds.y(), "the legend is above the title");
    let sign_in = world.node(AccessRole::Button, "Sign in").expect("Sign in");
    let back = world.node(AccessRole::Button, "Change church").expect("the action");
    assert!((sign_in.bounds.y() - back.bounds.y()).abs() < 1.0, "Sign in and the action share a row");
    assert!(sign_in.bounds.x() < back.bounds.x(), "Sign in comes first");
    assert!(world.has_button("Administrators") && world.has_button("Treasury"));
    assert!(world.says(ASIDE), "a wide window shows the aside");
}

#[test]
fn the_right_password_gets_in_and_changes_are_audited_as_at_the_usual_door() {
    let (database, behind) = (database(), Rc::new(Behind::default()));
    let mut world = with_ana(composed(&database, &behind));
    let made = behind.made.get();
    world.type_into("Name", "ana");
    world.type_into("Password", "correct horse");
    assert_eq!(behind.made.get(), made, "nothing of the application is made at the door");
    world.press("Sign in");
    assert!(world.inside());
    assert_eq!(behind.made.get(), made + 1);

    let session = behind.session.borrow().clone().expect("opened");
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
}

#[test]
fn a_wrong_password_shows_the_error_and_stays_at_the_door() {
    let behind = Rc::new(Behind::default());
    let mut world = with_ana(composed(&database(), &behind));
    let made = behind.made.get();
    world.type_into("Name", "ana");
    world.type_into("Password", "wrong password");
    world.press("Sign in");
    assert!(!world.inside());
    assert!(world.says("is not right"));
    assert!(!world.says("wrong password"), "what was typed went");
    assert_eq!(behind.made.get(), made);
}

#[test]
fn a_chip_fills_in_without_getting_in_and_keeps_the_password_hidden() {
    let behind = Rc::new(Behind::default());
    let mut world = with_ana(composed(&database(), &behind));
    world.press("Administrators");
    assert!(!world.inside(), "a chip only fills in");
    assert_eq!(world.field("Name").value.as_deref(), Some("ana"));
    assert!(!world.says("correct horse"), "the password stays hidden");
    world.press("Sign in");
    assert!(world.inside(), "Sign in checks what the chip filled in");

    world.press("Sign out");
    world.press("Treasury");
    world.press("Sign in");
    assert!(!world.inside(), "a chip is checked like anyone else");
    assert!(world.says("is not right"));
    world.press("Administrators");
    assert!(!world.says("is not right"), "filling in again clears the message");
}

#[test]
fn choosing_a_new_password_keeps_the_framework_s_screen() {
    let (database, behind) = (database(), Rc::new(Behind::default()));
    let mut world = with_ana(composed(&database, &behind));
    let mut space = database.space();
    let mut ana = space.query(SECURITY_USER, &Query::all()).expect("reads").remove(0);
    ana.set("must_change_password", true);
    space.save(ana).expect("saved");
    space.commit().expect("committed");

    world.type_into("Name", "ana");
    world.type_into("Password", "correct horse");
    world.press("Sign in");
    assert!(world.says("Choose your own"));
    assert!(!world.inside());
    assert!(!world.has_button("Administrators"), "chips are only where someone signs in");
    assert!(world.says(ASIDE), "the aside stays beside it");
    world.type_into("Current password", "correct horse");
    world.type_into("New password", "ana chose this");
    world.type_into("Repeat the password", "ana chose this");
    world.press("Save password");
    assert!(world.inside());
}

#[test]
fn going_back_from_a_composed_door_forgets_what_was_typed() {
    let behind = Rc::new(Behind::default());
    let mut world = with_ana(composed(&database(), &behind));
    world.press("Administrators");
    world.press("Change church");
    assert_eq!(behind.back.get(), 1);
    assert_eq!(world.field("Name").value.unwrap_or_default(), "");
    world.press("Sign in");
    assert!(!world.inside());
}

#[test]
fn the_keyboard_goes_from_name_to_password_and_enter_signs_in() {
    let behind = Rc::new(Behind::default());
    let mut world = with_ana(composed(&database(), &behind));
    let name = world.field("Name").id;
    assert_eq!(world.ui.access_tree().focus, Some(name), "the name takes the keyboard");
    world.typed("ana");
    world.key(Key::Tab);
    let password = world.field("Password").id;
    assert_eq!(world.ui.access_tree().focus, Some(password), "Tab goes to the password");
    world.typed("correct horse");
    world.key(Key::Enter);
    assert!(world.inside(), "Enter in the password signs in");
}

#[test]
fn a_screen_reader_hears_the_fields_and_the_chips_by_name() {
    let behind = Rc::new(Behind::default());
    let mut world = with_ana(composed(&database(), &behind));
    let nodes = world.nodes();
    let inputs: Vec<_> = nodes.iter().filter(|n| n.role == AccessRole::TextInput).collect();
    assert_eq!(inputs.len(), 2);
    assert!(inputs.iter().all(|n| n.name.as_deref().is_some_and(|name| !name.is_empty())));
    for chip in ["Administrators", "Treasury", "Sign in", "Change church"] {
        let button = world.node(AccessRole::Button, chip).expect("named");
        assert!(button.focusable, "{chip} can be reached with Tab");
    }
}

#[test]
fn a_narrow_window_leaves_out_the_aside_and_fits_the_card() {
    let behind = Rc::new(Behind::default());
    let database = database();
    with_ana(composed(&database, &behind));
    let mut world = World::at(composed(&database, &behind), 360.0);
    assert!(!world.says(ASIDE));
    // The card keeps a gutter of 16 on each side, and its padding of 24 is inside it.
    for label in ["Name", "Password"] {
        let field = world.field(label);
        let (left, right) = (field.bounds.x(), field.bounds.max_x());
        assert!(
            left >= 16.0 + 24.0 - 0.5 && right <= 360.0 - 16.0 - 24.0 + 0.5,
            "{label} fits: {:?}",
            field.bounds
        );
    }
    assert!(world.has_button("Administrators"));
}

#[test]
fn without_a_door_the_usual_screen_stays_as_it_was() {
    let behind = Rc::new(Behind::default());
    let database = database();
    with_ana(secured(&database, &behind));
    let mut usual = World::over(secured(&database, &behind));
    let mut beside = World::over(secured(&database, &behind).aside(Text::new(ASIDE)));
    let names = |world: &mut World| -> Vec<(AccessRole, Option<String>)> {
        world
            .nodes()
            .into_iter()
            .filter(|n| n.name.as_deref() != Some(ASIDE))
            .map(|n| (n.role, n.name))
            .collect()
    };
    assert!(!usual.says(LEGEND));
    assert!(usual.has_button("Sign in") && usual.has_button("Change church"));
    assert_eq!(names(&mut usual), names(&mut beside), "an aside alone changes nothing in the card");
    let card = usual.node(AccessRole::Button, "Sign in").expect("Sign in").bounds;
    assert!((card.width() - (380.0 - 48.0)).abs() < 1.0, "Sign in still takes the card's width: {card:?}");
}
