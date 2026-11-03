//! Rules that only warn, records two people change, editing on a list, what no storage keeps, and
//! the other things a business application turns out to need.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_app::*;
use ravenrs_app_ui::*;
use ravenrs_core::{AccessAction, Event, Key, Modifiers, Point, PointerButton, Role, Size};
use ravenrs_style::Theme;
use ravenrs_widgets::{Text as Label, Ui, View};

fn product() -> EntityInfo {
    EntityInfo::new("Product")
        .caption("Products")
        .navigation("Sales")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Name").default_property())
        .field(FieldInfo::new("seats", FieldKind::Integer).caption("Seats"))
        .field(
            FieldInfo::new(
                "state",
                FieldKind::Enum(vec![EnumOption::new("o", "Open"), EnumOption::new("c", "Closed")]),
            )
            .caption("State"),
        )
}

fn add(database: &MemoryDatabase, name: &str, seats: f64) -> Value {
    let mut space = database.space();
    let mut object = space.new_object("Product").expect("known");
    object.set("name", name);
    object.set("seats", seats);
    let key = object.get("id");
    space.save(object).expect("saves");
    space.commit().expect("keeps");
    key
}

fn read(database: &MemoryDatabase, key: &Value, field: &str) -> Value {
    database.space().find("Product", key).expect("reads").expect("there").get(field)
}

struct World {
    ui: Ui,
    session: Session,
}

impl World {
    fn over(
        entities: Vec<EntityInfo>,
        setup: impl FnOnce(ApplicationBuilder) -> ApplicationBuilder,
    ) -> (Self, MemoryDatabase) {
        let database = MemoryDatabase::new(entities.clone()).expect("valid");
        (Self::on(&database, entities, setup), database)
    }

    fn on(
        database: &MemoryDatabase,
        entities: Vec<EntityInfo>,
        setup: impl FnOnce(ApplicationBuilder) -> ApplicationBuilder,
    ) -> Self {
        let builder = entities
            .into_iter()
            .fold(Application::builder("Test").module(SystemModule::new()), ApplicationBuilder::entity);
        let application = setup(builder).build().expect("valid");
        let source = database.clone();
        let shell = AppShell::new(application, move || source.space());
        let session = shell.session();
        Self::showing(shell, session)
    }

    fn showing(view: impl View + 'static, session: Session) -> Self {
        let mut ui = Ui::new(view, Theme::raven_dark());
        ui.set_viewport(Size::new(1200.0, 900.0), 1.0);
        ui.update();
        ui.update();
        Self { ui, session }
    }

    fn settle(&mut self) {
        self.ui.update();
        self.ui.update();
        self.ui.update();
    }

    fn says(&mut self, text: &str) -> bool {
        self.ui.access_tree().nodes.iter().any(|n| {
            [&n.name, &n.value, &n.description].iter().any(|t| t.as_deref().is_some_and(|t| t.contains(text)))
        })
    }

    fn names(&mut self, role: Role) -> Vec<String> {
        self.ui.access_tree().nodes.iter().filter(|n| n.role == role).filter_map(|n| n.name.clone()).collect()
    }

    fn press(&mut self, name: &str) {
        let tree = self.ui.access_tree();
        let found = tree.nodes.iter().find(|n| n.role == Role::Button && n.name.as_deref() == Some(name));
        let id = found.unwrap_or_else(|| panic!("no button {name} in {:?}", self.names(Role::Button))).id;
        self.ui.perform_action(id, AccessAction::Click);
        self.settle();
    }

    fn keys(&mut self, text: &str) {
        for character in text.chars() {
            self.ui.handle_event(&Event::KeyDown {
                key: Key::Character(character.to_string()),
                modifiers: Modifiers::default(),
            });
        }
        self.settle();
    }

    fn type_into(&mut self, field: &str, text: &str) {
        let tree = self.ui.access_tree();
        let id = tree
            .nodes
            .iter()
            .find(|n| n.role == Role::TextInput && n.name.as_deref().is_some_and(|t| t.starts_with(field)))
            .unwrap_or_else(|| panic!("no field {field} in {:?}", self.names(Role::TextInput)))
            .id;
        self.ui.perform_action(id, AccessAction::Focus);
        self.settle();
        self.keys(text);
    }

    fn open(&mut self, entity: &str, key: &Value) {
        self.session.open_detail(entity, key);
        self.settle();
    }

    fn list(&mut self, view: &str) {
        self.session.open_view(view);
        self.settle();
    }

    fn cell_at(&mut self, text: &str) -> Point {
        let tree = self.ui.access_tree();
        tree.nodes
            .iter()
            .find(|n| {
                n.role == Role::Cell && (n.name.as_deref() == Some(text) || n.value.as_deref() == Some(text))
            })
            .map(|n| n.bounds.center())
            .unwrap_or_else(|| panic!("no cell {text} in {:?}", self.names(Role::Cell)))
    }

    /// Double-clicks a cell, takes out what it holds, and types this and Enter.
    fn edit_cell(&mut self, holding: &str, typed: &str) {
        let spot = self.cell_at(holding);
        let press = |ui: &mut Ui| {
            ui.handle_event(&Event::PointerDown { position: spot, button: PointerButton::Primary });
            ui.handle_event(&Event::PointerUp { position: spot, button: PointerButton::Primary });
        };
        self.ui.handle_event(&Event::PointerMove { position: spot });
        press(&mut self.ui);
        self.ui.update();
        self.ui.handle_event(&Event::DoubleClick { position: spot });
        self.settle();
        self.clear_and_type(typed);
    }

    fn clear_and_type(&mut self, typed: &str) {
        for _ in 0..12 {
            self.ui.handle_event(&Event::KeyDown { key: Key::Backspace, modifiers: Modifiers::default() });
        }
        self.ui.update();
        self.ui.handle_event(&Event::KeyDown {
            key: Key::Character(typed.to_owned()),
            modifiers: Modifiers::default(),
        });
        self.ui.update();
        self.ui.handle_event(&Event::KeyDown { key: Key::Enter, modifiers: Modifiers::default() });
        self.settle();
    }
}

#[test]
fn a_rule_that_only_warns_asks_and_lets_people_go_on() {
    let entity =
        product().rule(Rule::range("seats", Some(1.0), None).warning().message("Few seats on {name}"));
    let (mut w, database) = World::over(vec![entity], |b| b);
    let key = add(&database, "Mesa", 5.0);
    w.open("Product", &key);
    w.type_into("Name", " grande");
    w.session.set_form_value("seats", Value::Number(0.0));
    w.settle();
    w.press("Save");
    assert!(w.says("Save anyway?"), "it asks");
    assert!(w.says("Few seats on Mesa grande"), "and says why");
    assert_eq!(read(&database, &key, "seats"), Value::Number(5.0), "nothing kept yet");
    w.press("Save anyway");
    assert_eq!(read(&database, &key, "seats"), Value::Number(0.0));
    assert!(w.says("Few seats on Mesa grande"), "and the form still notes it");
    assert!(!w.session.has_unsaved_changes());
}

#[test]
fn saving_and_closing_past_a_warning_closes_only_once_answered() {
    let entity = product().rule(Rule::range("seats", Some(1.0), None).warning());
    let (mut w, database) = World::over(vec![entity], |b| b);
    let key = add(&database, "Mesa", 0.0);
    w.list("Product_ListView");
    w.open("Product", &key);
    w.type_into("Name", "!");
    w.press("Save and close");
    assert!(w.says("Save anyway?"));
    assert!(w.session.form_record().is_some(), "still on the form");
    w.press("Save anyway");
    assert!(w.session.form_record().is_none(), "gone from the form");
    assert_eq!(read(&database, &key, "name"), Value::Text("Mesa!".into()));
}

#[test]
fn a_rule_that_only_informs_saves_and_leaves_a_note_and_an_error_still_stops() {
    let entity = product()
        .rule(Rule::range("seats", Some(100.0), None).information().message("Small room"))
        .rule(Rule::required("name"));
    let (mut w, database) = World::over(vec![entity], |b| b);
    let key = add(&database, "Mesa", 5.0);
    w.open("Product", &key);
    w.type_into("Name", "!");
    w.press("Save");
    assert!(!w.says("Save anyway"), "nothing to ask");
    assert_eq!(read(&database, &key, "name"), Value::Text("Mesa!".into()));
    assert!(w.says("Small room"), "a note on the field");
    w.session.set_form_value("name", Value::Null);
    w.settle();
    w.press("Save");
    assert_eq!(read(&database, &key, "name"), Value::Text("Mesa!".into()), "an error stops it");
    assert!(w.says("Name is required"));
}

#[test]
fn a_rule_about_the_whole_record_that_warns_shows_above_the_form() {
    let entity = product().rule(Rule::criteria("big", "[seats] > 10").information().message("Not many"));
    let (mut w, database) = World::over(vec![entity], |b| b);
    let key = add(&database, "Mesa", 5.0);
    w.open("Product", &key);
    w.type_into("Name", "!");
    w.press("Save");
    assert!(w.says("Not many"));
}

#[test]
fn an_action_asks_about_what_its_context_only_warns_of() {
    struct Pay;
    impl ViewController for Pay {
        fn id(&self) -> &str {
            "Pay"
        }
        fn actions(&self) -> Vec<Action> {
            vec![
                Action::simple("Pay", "Pay")
                    .validation_contexts(["Pay"])
                    .selection(Selection::RequireSingle)
                    .on_execute(|cx| {
                        cx.message(Message::new("Paid", MessageKind::Success));
                        Ok(())
                    }),
            ]
        }
    }
    struct Paying;
    impl Module for Paying {
        fn name(&self) -> &str {
            "Paying"
        }
        fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
            vec![Box::new(Pay)]
        }
    }
    let entity =
        product().rule(Rule::range("seats", Some(1.0), None).contexts(["Pay"]).warning().message("Empty"));
    let (mut w, database) = World::over(vec![entity], |b| b.module(Paying));
    let key = add(&database, "Mesa", 0.0);
    w.open("Product", &key);
    w.press("Pay");
    assert!(w.says("Empty"), "it asks first");
    assert!(!w.says("Paid"));
    w.press("Save anyway");
    assert!(w.says("Paid"), "the action tells people, in a message");
}

fn two_people(entity: EntityInfo) -> (World, World, MemoryDatabase, Value) {
    let database = MemoryDatabase::new(vec![entity.clone()]).expect("valid");
    let key = add(&database, "Mesa", 5.0);
    let mine = World::on(&database, vec![entity.clone()], |b| b);
    let theirs = World::on(&database, vec![entity], |b| b);
    (mine, theirs, database, key)
}

#[test]
fn what_two_people_changed_in_different_fields_both_stand() {
    let (mut mine, mut theirs, database, key) = two_people(product());
    mine.open("Product", &key);
    theirs.open("Product", &key);
    mine.type_into("Name", " grande");
    theirs.session.set_form_value("seats", Value::Number(9.0));
    theirs.settle();
    theirs.press("Save");
    mine.press("Save");
    assert!(!mine.says("Keep mine"), "nothing to decide");
    assert_eq!(read(&database, &key, "name"), Value::Text("Mesa grande".into()));
    assert_eq!(read(&database, &key, "seats"), Value::Number(9.0), "theirs is still there");
    assert!(!mine.session.has_unsaved_changes(), "and the form follows");
}

#[test]
fn what_two_people_changed_in_the_same_field_takes_a_decision() {
    let (mut mine, mut theirs, database, key) = two_people(product());
    mine.open("Product", &key);
    theirs.open("Product", &key);
    mine.type_into("Name", " minha");
    theirs.type_into("Name", " dela");
    theirs.press("Save");
    mine.press("Save");
    assert!(mine.says("Someone else changed this record"));
    assert!(mine.says("They changed Name too"));
    assert_eq!(read(&database, &key, "name"), Value::Text("Mesa dela".into()), "nothing kept yet");
    mine.press("Keep mine");
    assert_eq!(read(&database, &key, "name"), Value::Text("Mesa minha".into()));
}

#[test]
fn a_record_someone_else_deleted_cannot_be_saved_and_says_so() {
    let (mut mine, _theirs, database, key) = two_people(product());
    mine.open("Product", &key);
    mine.type_into("Name", " minha");
    let mut space = database.space();
    space.delete("Product", &key).expect("marks");
    space.commit().expect("keeps");
    mine.press("Save");
    assert!(mine.says("Someone else deleted this record"));
}

#[test]
fn values_change_right_on_a_list_with_the_rules_checked() {
    let entity = product().list_editable().rule(Rule::range("seats", Some(1.0), None).message("Too few"));
    let (mut w, database) = World::over(vec![entity], |b| b);
    let key = add(&database, "Mesa", 5.0);
    w.list("Product_ListView");
    w.edit_cell("5", "9");
    assert_eq!(read(&database, &key, "seats"), Value::Number(9.0));
    w.edit_cell("9", "0");
    assert_eq!(read(&database, &key, "seats"), Value::Number(9.0), "refused");
    assert!(w.says("Too few"), "and it says why");
}

#[test]
fn a_list_can_only_be_edited_where_it_says_and_never_a_key() {
    let (mut w, database) = World::over(vec![product()], |b| b);
    let key = add(&database, "Mesa", 5.0);
    w.list("Product_ListView");
    w.edit_cell("5", "9");
    assert_eq!(read(&database, &key, "seats"), Value::Number(5.0));
}

#[test]
fn options_are_edited_on_a_list_by_what_they_are_called() {
    let entity = product().list_editable();
    let (mut w, database) = World::over(vec![entity], |b| b);
    let key = add(&database, "Mesa", 5.0);
    let mut space = database.space();
    let mut object = space.find("Product", &key).expect("reads").expect("there");
    object.set("state", "o");
    space.save(object).expect("saves");
    space.commit().expect("keeps");
    w.list("Product_ListView");
    w.edit_cell("Open", "Closed");
    assert_eq!(read(&database, &key, "state"), Value::Text("c".into()));
    w.edit_cell("Closed", "Nada");
    assert_eq!(read(&database, &key, "state"), Value::Text("c".into()));
    assert!(w.says("State is not valid"));
}

#[test]
fn a_warning_on_a_list_goes_on_with_a_message() {
    let entity =
        product().list_editable().rule(Rule::range("seats", Some(1.0), None).warning().message("Few"));
    let (mut w, database) = World::over(vec![entity], |b| b);
    let key = add(&database, "Mesa", 5.0);
    w.list("Product_ListView");
    w.edit_cell("5", "0");
    assert_eq!(read(&database, &key, "seats"), Value::Number(0.0));
    assert!(w.says("Few"));
}

#[test]
fn a_list_that_reads_only_its_columns_still_selects_whole_records() {
    let entity = product()
        .data_access(DataAccess::DataView)
        .field(FieldInfo::new("notes", FieldKind::Text).caption("Notes").in_list(false));
    let (mut w, database) = World::over(vec![entity], |b| b);
    let key = add(&database, "Mesa", 5.0);
    let mut space = database.space();
    let mut object = space.find("Product", &key).expect("reads").expect("there");
    object.set("notes", "Hidden");
    space.save(object).expect("saves");
    space.commit().expect("keeps");
    w.list("Product_ListView");
    assert!(w.says("Mesa"));
    let spot = w.cell_at("Mesa");
    w.ui.handle_event(&Event::PointerMove { position: spot });
    w.ui.handle_event(&Event::PointerDown { position: spot, button: PointerButton::Primary });
    w.ui.handle_event(&Event::PointerUp { position: spot, button: PointerButton::Primary });
    w.settle();
    let selected = w.session.frame().selected();
    assert_eq!(selected.first().map(|o| o.get("notes")), Some(Value::Text("Hidden".into())));
}

#[test]
fn a_list_set_to_read_as_needed_brings_its_rows() {
    let entity = product().data_access(DataAccess::Server);
    let (mut w, database) = World::over(vec![entity], |b| b);
    for at in 0..3 {
        add(&database, &format!("Mesa {at}"), 5.0);
    }
    w.list("Product_ListView");
    assert!(w.says("Mesa 2"));
}

#[test]
fn a_key_made_up_for_a_record_works_on_lists_and_forms() {
    let entity = EntityInfo::new("Note")
        .caption("Notes")
        .navigation("Sales")
        .field(FieldInfo::new("id", FieldKind::Guid).key())
        .field(FieldInfo::new("text", FieldKind::Text).caption("Text").default_property());
    let (mut w, database) = World::over(vec![entity], |b| b);
    w.session.open_new("Note");
    w.settle();
    w.type_into("Text", "Hello");
    w.press("Save and close");
    let found = database.space().query("Note", &Query::all()).expect("reads");
    assert_eq!(found.len(), 1);
    let key = found[0].get("id");
    w.open("Note", &key);
    w.type_into("Text", " again");
    w.press("Save");
    assert_eq!(
        database.space().find("Note", &key).expect("reads").expect("there").get("text"),
        Value::Text("Hello again".into())
    );
}

#[test]
fn fields_show_with_the_controls_and_formats_they_ask_for() {
    let target = EntityInfo::new("Target")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("title", FieldKind::Text).caption("Title").default_property());
    let entity = EntityInfo::new("Thing")
        .caption("Things")
        .navigation("Sales")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(
            FieldInfo::new("done", FieldKind::Integer)
                .caption("Done")
                .editor("Progress")
                .display_format("{} %"),
        )
        .field(
            FieldInfo::new("fine", FieldKind::Decimal(9)).caption("Fine").display_format("{:.4}").read_only(),
        )
        .field(FieldInfo::new("phone", FieldKind::Text).caption("Phone").edit_mask("(00) 0000"))
        .field(FieldInfo::new("size", FieldKind::Text).caption("Size").predefined(["Small", "Big"]))
        .field(
            FieldInfo::new(
                "colors",
                FieldKind::Tags(vec![EnumOption::new("r", "Red"), EnumOption::new("b", "Blue")]),
            )
            .caption("Colors"),
        )
        .field(FieldInfo::new("rule", FieldKind::Criteria).caption("Rule").criteria_of("Target"))
        .field(FieldInfo::new("street", FieldKind::Text).caption("Street").group("Address"));
    let (mut w, database) = World::over(vec![entity, target], |b| b);
    let mut space = database.space();
    let mut object = space.new_object("Thing").expect("known");
    object.set("done", 60.0);
    object.set("fine", 0.123_456_789);
    object.set("colors", "r,b");
    object.set("size", "Huge");
    let key = object.get("id");
    space.save(object).expect("saves");
    space.commit().expect("keeps");

    w.list("Thing_ListView");
    assert!(w.says("60 %"), "a number with its format: {:?}", w.names(Role::Cell));
    assert!(w.says("Red, Blue"), "tags by caption");

    w.open("Thing", &key);
    assert!(w.says("60%"), "a bar says how far along");
    assert!(w.says("0.1235"), "read-only with its format");
    assert!(w.says("Huge"), "what it held stays among the values");
    assert!(w.says("Red") && w.says("Blue"), "tags");
    assert!(w.says("Address"), "a group of its own");
    assert!(w.says("Add condition") || w.says("Add"), "a filter builder: {:?}", w.names(Role::Button));
    w.type_into("Phone", "1234");
    w.session.frame().execute(ids::SAVE, ActionInput::None).expect("saves");
    w.settle();
    let found = database.space().find("Thing", &key).expect("reads").expect("there");
    assert_eq!(found.get("phone"), Value::Text("(12) 34".into()), "through its mask");
}

struct Tools;

impl ViewController for Tools {
    fn id(&self) -> &str {
        "Tools"
    }
    fn actions(&self) -> Vec<Action> {
        vec![Action::simple("Stamp", "Stamp").category(ActionCategory::InForm("Tools".into())).on_execute(
            |cx| {
                cx.message(Message::new("Stamped", MessageKind::Information));
                Ok(())
            },
        )]
    }
}

struct ToolsModule;

impl Module for ToolsModule {
    fn name(&self) -> &str {
        "Tools"
    }
    fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
        vec![Box::new(Tools)]
    }
}

#[test]
fn an_action_can_go_inside_a_form_and_not_above_it_too() {
    let entity = product().form_actions("Tools");
    let (mut w, database) = World::over(vec![entity], |b| b.module(ToolsModule));
    let key = add(&database, "Mesa", 5.0);
    w.open("Product", &key);
    assert_eq!(w.names(Role::Button).iter().filter(|name| *name == "Stamp").count(), 1);
    w.press("Stamp");
    assert!(w.says("Stamped"));
}

#[test]
fn a_message_can_have_a_button_and_say_when_it_went_away() {
    let (mut w, _) = World::over(vec![product()], |b| b);
    let seen: Rc<RefCell<Vec<&str>>> = Rc::default();
    let (pressed, closed) = (seen.clone(), seen.clone());
    w.session.show_message(
        Message::new("Saved it", MessageKind::Success)
            .action("Undo", move || pressed.borrow_mut().push("undo"))
            .on_close(move || closed.borrow_mut().push("closed")),
    );
    w.settle();
    assert!(w.says("Saved it"));
    w.press("Undo");
    assert_eq!(*seen.borrow(), ["undo", "closed"]);
    assert!(!w.says("Saved it"));
}

fn transient_entities() -> Vec<EntityInfo> {
    vec![
        product(),
        EntityInfo::new("Line")
            .caption("Lines")
            .transient()
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("text", FieldKind::Text).caption("Text").default_property()),
        EntityInfo::new("Draft")
            .caption("Drafts")
            .transient()
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("title", FieldKind::Text).caption("Title").default_property())
            .field(
                FieldInfo::new("item", FieldKind::Reference("Product".into()))
                    .caption("Item")
                    .editor("Select"),
            )
            .field(FieldInfo::new("lines", FieldKind::Collection("Line".into())).caption("Lines")),
    ]
}

/// A place in the navigation for a form, with no list before it.
struct DraftPlaces;

impl Module for DraftPlaces {
    fn name(&self) -> &str {
        "DraftPlaces"
    }
    fn model_difference(&self) -> Option<&'static str> {
        Some(
            r#"{ "NavigationItems": { "Sales": {
                "$new": true, "Caption": "Sales",
                "NewDraft": { "$new": true, "Caption": "New draft", "View": "Draft_DetailView" },
                "OldDraft": { "$new": true, "Caption": "Old draft", "View": "Draft_DetailView", "ObjectKey": "7" }
            } } }"#,
        )
    }
}

fn draft_world() -> (World, MemoryDatabase) {
    let (w, database) = World::over(transient_entities(), |b| b.module(DraftPlaces));
    add(&database, "Mesa", 5.0);
    w.session.use_records("Draft", || {
        let mut old = Object::from_values("Draft", Default::default());
        old.set("id", 7.0);
        old.set("title", "From before");
        vec![old]
    });
    w.session.use_collection("Draft", "lines", |draft| {
        let mut line = Object::from_values("Line", Default::default());
        line.set("id", 1.0);
        line.set("text", format!("A line of {}", display_of(draft)));
        vec![line]
    });
    (w, database)
}

fn display_of(draft: &Object) -> String {
    if let Value::Text(title) = draft.get("title") { title } else { "nothing".to_owned() }
}

#[test]
fn a_form_of_what_no_storage_keeps_opens_from_the_navigation() {
    let (mut w, database) = draft_world();
    w.press("New draft");
    assert!(w.session.form_record().is_some(), "new, with no list before it");
    w.type_into("Title", "Plan");
    w.press("Save");
    assert!(!w.says("no entity"), "nothing goes to storage");
    assert_eq!(database.space().query("Draft", &Query::all()).expect("reads").len(), 0);
    assert_eq!(display_of(&w.session.form_record().expect("still there")), "Plan");
    assert!(!w.session.has_unsaved_changes());
}

#[test]
fn a_form_of_what_no_storage_keeps_opens_by_its_key_and_points_to_what_is_stored() {
    let (mut w, _) = draft_world();
    w.press("Old draft");
    assert_eq!(display_of(&w.session.form_record().expect("there")), "From before");
    assert!(w.says("Mesa"), "it can point to a record that is stored");
    assert!(w.says("A line of From before"), "and it lists what is given for it");
}

#[test]
fn a_dashboard_changes_to_another_by_its_menu() {
    let first = DashboardInfo::new("Home", "Home")
        .navigation("Start")
        .variant("sales", "Sales", "Home")
        .variant("staff", "Staff", "Staff")
        .control("Sales part", "sales");
    let second = DashboardInfo::new("Staff", "Staff").control("Staff part", "staff");
    let (mut w, _) = World::over(vec![product()], |b| b.dashboard(first).dashboard(second));
    w.session.register_control("sales", || Box::new(Label::new("Selling")));
    w.session.register_control("staff", || Box::new(Label::new("Staffing")));
    w.list("Home_DashboardView");
    assert!(w.says("Selling") && !w.says("Staffing"));
    w.session.frame().execute(ids::VARIANTS, ActionInput::Choice("staff".into())).expect("runs");
    w.settle();
    assert!(w.says("Staffing") && !w.says("Selling"));
}

#[test]
fn a_dashboard_with_no_other_to_change_to_has_no_menu() {
    let home = DashboardInfo::new("Home", "Home").navigation("Start").control("Part", "part");
    let (mut w, _) = World::over(vec![product()], |b| b.dashboard(home));
    w.list("Home_DashboardView");
    let on = w.session.frame().action(ids::VARIANTS).is_some_and(|action| action.is_active());
    assert!(!on);
}

#[test]
fn an_application_adds_fields_in_its_settings_and_they_reach_the_screens() {
    let window = App::new("Test")
        .entity_info(product())
        .administrator_settings(
            r#"{ "Entities": { "Product": { "Members": {
                "city": { "$new": true, "Kind": "Text", "Caption": "City" },
                "twice": { "$new": true, "Kind": "Integer", "Caption": "Twice", "Expression": "[seats] * 2" }
            } } } }"#,
        )
        .seed(|space| {
            let mut object = space.new_object("Product").map_err(|e| e.to_string())?;
            object.set("name", "Mesa");
            object.set("seats", 4.0);
            object.set("city", "Recife");
            space.save(object).map_err(|e| e.to_string())
        })
        .build()
        .expect("valid");
    let mut ui = Ui::new(window, Theme::raven_dark());
    ui.set_viewport(Size::new(1200.0, 900.0), 1.0);
    ui.update();
    ui.update();
    let tree = ui.access_tree();
    let says = |text: &str| tree.nodes.iter().any(|n| n.name.as_deref() == Some(text));
    assert!(says("City"), "a column");
    assert!(says("Recife"), "kept in a field the application did not declare");
    assert!(says("8"), "worked out by its expression");
}

#[test]
fn the_screens_translate_from_a_file_of_captions() {
    let (mut w, _) = World::over(vec![product()], |b| b);
    let csv = w.session.export_captions();
    let filled =
        csv.replace("Entities/Product/Members/name,Name,", "Entities/Product/Members/name,Name,Nome");
    w.session.import_captions("pt-BR", &filled).expect("reads");
    w.session.set_language("pt-BR").expect("valid");
    w.list("Product_ListView");
    assert!(w.says("Nome"));
}

#[derive(BusinessObject, Clone, Debug, PartialEq)]
#[object(caption = "Declared", list_editable, data_access = "data_view", form_actions = "Tools")]
#[rule(criteria = "[seats] > 0", message = "No seats on {name}", warning)]
#[rule(criteria = "[seats] > 9", message = "Few", information)]
struct Declared {
    #[field(key, guid)]
    id: String,
    #[field(group = "Details", predefined = "a,b")]
    name: String,
    #[field(display_format = "{} %", edit_mask = "00")]
    seats: i64,
    #[field(tags = "red,blue")]
    colors: String,
    #[field(criteria_of = "Other")]
    rule: String,
}

#[test]
fn a_struct_says_all_of_this_in_its_attributes() {
    let info = Declared::entity_info();
    assert!(info.list_editable);
    assert_eq!(info.data_access, DataAccess::DataView);
    assert_eq!(info.form_actions, ["Tools"]);
    assert_eq!(info.rules[0].severity, Severity::Warning);
    assert_eq!(info.rules[1].severity, Severity::Information);
    let field = |name: &str| info.field_named(name).expect("there");
    assert_eq!(field("id").kind, FieldKind::Guid);
    assert!(field("id").is_key);
    assert_eq!(field("name").group.as_deref(), Some("Details"));
    assert_eq!(field("name").predefined, ["a", "b"]);
    assert_eq!(field("seats").display_format.as_deref(), Some("{} %"));
    assert_eq!(field("seats").edit_mask.as_deref(), Some("00"));
    let FieldKind::Tags(options) = &field("colors").kind else { panic!("tags") };
    assert_eq!(options[1].code, "blue");
    assert_eq!(field("rule").criteria_of.as_deref(), Some("Other"));
}
