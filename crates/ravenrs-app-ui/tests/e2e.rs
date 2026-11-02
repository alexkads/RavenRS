//! A whole application, driven the way people use it, with no window.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_app::*;
use ravenrs_app_ui::*;
use ravenrs_core::{AccessNode, Event, Key, Modifiers, Point, PointerButton, Role, Size};
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

fn entities() -> Vec<EntityInfo> {
    vec![
        EntityInfo::new("Account")
            .caption("Accounts")
            .navigation("Records")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("name", FieldKind::Text).caption("Name").default_property())
            .field(FieldInfo::new("opening", FieldKind::Money).caption("Opening").default_value(0.0))
            .field(FieldInfo::new("active", FieldKind::Boolean).caption("Active").default_value(true)),
        EntityInfo::new("Bank")
            .caption("Banks")
            .navigation("Records")
            .field(FieldInfo::new("code", FieldKind::Text).caption("Code").typed_key())
            .field(FieldInfo::new("name", FieldKind::Text).caption("Name")),
    ]
}

type Files = Rc<RefCell<Vec<(String, Vec<u8>)>>>;

struct World {
    ui: Ui,
    database: MemoryDatabase,
    session: Session,
    files: Files,
}

fn world_with(setup: impl FnOnce(ApplicationBuilder) -> ApplicationBuilder) -> World {
    let database = MemoryDatabase::new(entities()).expect("valid");
    let builder = entities()
        .into_iter()
        .fold(Application::builder("Test").module(SystemModule::new()), ApplicationBuilder::entity);
    let application = setup(builder).build().expect("valid");
    let source = database.clone();
    let files = Rc::new(RefCell::new(Vec::new()));
    let log = files.clone();
    let shell = AppShell::new(application, move || source.space())
        .on_file(move |name, bytes| log.borrow_mut().push((name, bytes)));
    let session = shell.session();
    let mut ui = Ui::new(shell, Theme::raven_dark());
    ui.set_viewport(Size::new(1200.0, 800.0), 1.0);
    ui.update();
    ui.update();
    World { ui, database, session, files }
}

fn world() -> World {
    world_with(|builder| builder)
}

impl World {
    fn nodes(&mut self, role: Role) -> Vec<AccessNode> {
        self.ui.access_tree().nodes.into_iter().filter(|n| n.role == role).collect()
    }

    fn says(&mut self, text: &str) -> bool {
        self.ui
            .access_tree()
            .nodes
            .iter()
            .any(|n| n.name.as_deref() == Some(text) || n.value.as_deref() == Some(text))
    }

    fn click_at(&mut self, at: Point) {
        self.ui.handle_event(&Event::PointerMove { position: at });
        self.ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
        self.ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
        self.settle();
    }

    fn settle(&mut self) {
        self.ui.update();
        self.ui.update();
    }

    fn find(&mut self, role: Role, name: &str) -> Option<AccessNode> {
        self.nodes(role).into_iter().find(|n| n.name.as_deref() == Some(name))
    }

    fn press(&mut self, name: &str) {
        let spot = self
            .find(Role::Button, name)
            .or_else(|| self.find(Role::MenuItem, name))
            .unwrap_or_else(|| panic!("no button {name}: {:?}", self.buttons()))
            .bounds
            .center();
        self.click_at(spot);
    }

    fn buttons(&mut self) -> Vec<String> {
        self.nodes(Role::Button).into_iter().filter_map(|n| n.name).collect()
    }

    fn enabled(&mut self, name: &str) -> bool {
        !self.find(Role::Button, name).unwrap_or_else(|| panic!("no button {name}")).disabled
    }

    fn key(&mut self, key: Key) {
        self.ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers::default() });
        self.settle();
    }

    fn typed(&mut self, text: &str) {
        for c in text.chars() {
            let key = if c == ' ' { Key::Space } else { Key::Character(c.to_string()) };
            self.key(key);
        }
    }

    /// Clicks a form's text field by what it is called, and types over what it holds.
    fn fill(&mut self, caption: &str, text: &str) {
        let spot = self
            .nodes(Role::TextInput)
            .into_iter()
            .find(|n| n.name.as_deref() == Some(caption))
            .unwrap_or_else(|| panic!("no field {caption}"))
            .bounds
            .center();
        self.click_at(spot);
        let current = self
            .nodes(Role::TextInput)
            .into_iter()
            .find(|n| n.name.as_deref() == Some(caption))
            .and_then(|n| n.value)
            .unwrap_or_default();
        self.key(Key::End);
        for _ in current.chars() {
            self.key(Key::Backspace);
        }
        self.typed(text);
    }

    fn accounts(&self) -> Vec<String> {
        let found = self.database.space().query("Account", &Query::all()).expect("reads");
        found
            .iter()
            .map(|o| if let Value::Text(name) = o.get("name") { name } else { String::new() })
            .collect()
    }

    fn add(&mut self, name: &str) {
        self.press("New");
        self.fill("Name", name);
        self.press("Save and close");
    }

    fn open_row(&mut self, name: &str) {
        let tree = self.ui.access_tree();
        let spot = tree
            .nodes
            .iter()
            .find(|n| {
                n.role == Role::Cell && (n.name.as_deref() == Some(name) || n.value.as_deref() == Some(name))
            })
            .unwrap_or_else(|| panic!("no row {name}"))
            .bounds
            .center();
        self.ui.handle_event(&Event::PointerMove { position: spot });
        self.ui.handle_event(&Event::DoubleClick { position: spot });
        self.settle();
    }

    fn check_row(&mut self, name: &str) {
        let tree = self.ui.access_tree();
        let cell = tree
            .nodes
            .iter()
            .find(|n| {
                n.role == Role::Cell && (n.name.as_deref() == Some(name) || n.value.as_deref() == Some(name))
            })
            .unwrap_or_else(|| panic!("no row {name}"))
            .bounds;
        let check = tree
            .nodes
            .iter()
            .find(|n| n.role == Role::CheckBox && (n.bounds.center().y - cell.center().y).abs() < 4.0)
            .expect("its check box")
            .bounds
            .center();
        self.click_at(check);
    }
}

#[test]
fn it_opens_on_the_first_list_with_the_navigation_of_the_model() {
    let mut w = world();
    assert!(w.says("RECORDS"), "the group");
    assert!(w.find(Role::Button, "Accounts").is_some() || w.says("Accounts"));
    assert!(w.says("Banks"));
    assert!(w.find(Role::Button, "New").is_some());
    assert!(w.find(Role::Button, "Save").is_none(), "only on a form");
    assert!(!w.enabled("Delete"), "nothing selected");
    assert!(!w.nodes(Role::Grid).is_empty());
}

#[test]
fn creating_a_record_keeps_it_and_shows_it_in_the_list() {
    let mut w = world();
    w.press("New");
    assert!(w.find(Role::Button, "Save").is_some());
    w.fill("Name", "Corrente");
    assert!(w.accounts().is_empty(), "nothing is kept yet");
    w.press("Save and close");
    assert_eq!(w.accounts(), ["Corrente"]);
    assert!(w.find(Role::Button, "Save").is_none(), "back on the list");
    assert!(w.says("Corrente"));
}

#[test]
fn save_stays_on_the_form_and_save_and_new_starts_another() {
    let mut w = world();
    w.press("New");
    w.fill("Name", "A");
    w.press("Save");
    assert_eq!(w.accounts(), ["A"]);
    assert!(w.find(Role::Button, "Save").is_some(), "still on the form");
    assert!(!w.session.has_unsaved_changes(), "nothing is left to save");
    w.fill("Name", "A2");
    w.press("Save and new");
    assert_eq!(w.accounts(), ["A2"], "the same record, changed");
    w.fill("Name", "B");
    w.press("Save and close");
    assert_eq!(w.accounts(), ["A2", "B"]);
}

#[test]
fn changing_and_canceling_leaves_the_record_as_it_was() {
    let mut w = world();
    w.add("Corrente");
    w.open_row("Corrente");
    assert!(w.find(Role::Button, "Save").is_some(), "the form opened");
    w.fill("Name", "Mudou");
    assert!(w.session.has_unsaved_changes());
    w.press("Cancel");
    assert_eq!(w.accounts(), ["Corrente"]);
    assert!(w.find(Role::Button, "Save").is_none());
    w.open_row("Corrente");
    w.fill("Name", "Mudou");
    w.press("Save and close");
    assert_eq!(w.accounts(), ["Mudou"]);
    assert!(w.says("Mudou"), "the list read again");
}

#[test]
fn leaving_with_changes_asks_first() {
    let mut w = world();
    w.press("New");
    w.fill("Name", "Rascunho");
    w.press("Banks");
    assert!(!w.nodes(Role::Dialog).is_empty(), "it asks");
    w.press("Cancel");
    assert!(w.nodes(Role::Dialog).is_empty());
    assert!(w.find(Role::Button, "Save").is_some(), "it stays on the form");
    w.press("Banks");
    w.press("Discard");
    assert!(w.find(Role::Button, "Save").is_none());
    assert!(w.accounts().is_empty());
}

#[test]
fn deleting_asks_and_works_from_the_list_and_from_the_form() {
    let mut w = world();
    w.add("A");
    w.add("B");
    w.check_row("A");
    assert!(w.enabled("Delete"));
    w.press("Delete");
    assert_eq!(w.accounts().len(), 2, "not before the answer");
    let dialog = w.nodes(Role::Dialog);
    assert!(!dialog.is_empty());
    let last_delete = w
        .nodes(Role::Button)
        .into_iter()
        .rfind(|n| n.name.as_deref() == Some("Delete"))
        .expect("the answer")
        .bounds
        .center();
    w.click_at(last_delete);
    assert_eq!(w.accounts(), ["B"]);
    assert!(!w.enabled("Delete"), "what was selected is gone");

    w.open_row("B");
    w.press("Delete");
    let last_delete = w
        .nodes(Role::Button)
        .into_iter()
        .rfind(|n| n.name.as_deref() == Some("Delete"))
        .expect("the answer")
        .bounds
        .center();
    w.click_at(last_delete);
    assert!(w.accounts().is_empty());
    assert!(w.find(Role::Button, "Save").is_none(), "back on the list");
}

#[test]
fn a_new_record_cannot_be_deleted_or_stepped_from() {
    let mut w = world();
    w.add("A");
    w.press("New");
    assert!(!w.enabled("Delete"));
    assert!(!w.enabled("Previous"));
    assert!(!w.enabled("Next"));
}

#[test]
fn moving_between_records_follows_the_list() {
    let mut w = world();
    for name in ["A", "B", "C"] {
        w.add(name);
    }
    w.open_row("B");
    let name = |w: &mut World| {
        w.nodes(Role::TextInput).into_iter().find(|n| n.name.as_deref() == Some("Name")).and_then(|n| n.value)
    };
    assert_eq!(name(&mut w).as_deref(), Some("B"));
    w.press("Next");
    assert_eq!(name(&mut w).as_deref(), Some("C"));
    assert!(!w.enabled("Next"), "the last");
    w.press("Previous");
    w.press("Previous");
    assert_eq!(name(&mut w).as_deref(), Some("A"));
    assert!(!w.enabled("Previous"), "the first");
    w.fill("Name", "A2");
    w.press("Next");
    assert!(!w.nodes(Role::Dialog).is_empty(), "it does not lose changes on the way");
    w.press("Discard");
    assert_eq!(name(&mut w).as_deref(), Some("B"));
    assert_eq!(w.accounts(), ["A", "B", "C"]);
}

#[test]
fn searching_filters_the_list() {
    let mut w = world();
    for name in ["Corrente", "Poupança"] {
        w.add(name);
    }
    assert!(w.says("Corrente") && w.says("Poupança"));
    let search = w
        .nodes(Role::TextInput)
        .into_iter()
        .find(|n| n.name.as_deref() == Some("Search all columns"))
        .expect("the search box")
        .bounds
        .center();
    w.click_at(search);
    w.typed("poup");
    assert!(!w.says("Corrente"));
    assert!(w.says("Poupança"));
}

#[test]
fn exporting_hands_over_a_real_file() {
    let mut w = world();
    w.add("Corrente");
    w.press("Export");
    w.press("PDF (.pdf)");
    let files = w.files.borrow();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].0, "Accounts.pdf");
    assert!(files[0].1.starts_with(b"%PDF"));
}

#[test]
fn a_duplicate_key_says_so_and_keeps_the_form() {
    let mut w = world();
    w.press("Banks");
    w.press("New");
    w.fill("Code", "001");
    w.fill("Name", "Um");
    w.press("Save and close");
    w.press("New");
    w.fill("Code", "001");
    w.fill("Name", "Dois");
    w.press("Save and close");
    assert!(w.find(Role::Button, "Save").is_some(), "it stays on the form");
    assert!(w.session.error().is_some_and(|e| e.contains("001")), "{:?}", w.session.error());
    let kept = w.database.space().query("Bank", &Query::all()).expect("reads");
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].get("name"), Value::Text("Um".into()));
    w.fill("Code", "002");
    w.press("Save and close");
    assert_eq!(w.database.space().query("Bank", &Query::all()).expect("reads").len(), 2, "the error is gone");
}

#[test]
fn a_key_typed_by_people_only_changes_while_the_record_is_new() {
    let mut w = world();
    w.press("Banks");
    w.press("New");
    w.fill("Code", "001");
    w.press("Save");
    let code =
        w.nodes(Role::TextInput).into_iter().find(|n| n.name.as_deref() == Some("Code")).expect("the field");
    assert!(code.read_only);
}

#[test]
fn a_reference_shows_the_name_of_the_record_it_points_to() {
    let bank = EntityInfo::new("Bank")
        .caption("Banks")
        .field(FieldInfo::new("code", FieldKind::Text).typed_key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Name"));
    let account = EntityInfo::new("Account")
        .caption("Accounts")
        .navigation("Records")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("bank", FieldKind::Reference("Bank".into())).caption("Bank"));
    let database = MemoryDatabase::new(vec![bank.clone(), account.clone()]).expect("valid");
    let mut setup = database.space();
    let mut object = setup.new_object("Bank").expect("the entity");
    object.set("code", "341");
    object.set("name", "Itau");
    setup.save(object).expect("saves");
    let mut object = setup.new_object("Account").expect("the entity");
    object.set("bank", "341");
    setup.save(object).expect("saves");
    setup.commit().expect("commits");
    let application = Application::builder("T")
        .module(SystemModule::new())
        .entity(account)
        .entity(bank)
        .build()
        .expect("valid");
    let source = database.clone();
    let shell = AppShell::new(application, move || source.space());
    let session = shell.session();
    let mut ui = Ui::new(shell, Theme::raven_dark());
    ui.set_viewport(Size::new(1200.0, 800.0), 1.0);
    ui.update();
    let mut w = World { ui, database, session, files: Rc::default() };
    assert!(w.says("Itau"), "the name, in the list");
    assert!(!w.says("341"), "and not the key");
    w.open_row("Itau");
    let field = w.nodes(Role::ComboBox);
    assert_eq!(field.first().and_then(|n| n.value.as_deref()), Some("Itau"), "and on the form");
}

struct Extra(Rc<RefCell<Vec<String>>>);

struct ExtraController(Rc<RefCell<Vec<String>>>);

impl ViewController for ExtraController {
    fn id(&self) -> &str {
        "Extra"
    }
    fn scope(&self) -> TargetScope {
        TargetScope::any().entity("Account")
    }
    fn actions(&self) -> Vec<Action> {
        let log = self.0.clone();
        let picked = self.0.clone();
        vec![
            Action::simple("Archive", "Archive")
                .selection(Selection::RequireSingle)
                .target_criteria("[active] = True", CriteriaMode::TrueForAll)
                .on_execute(move |cx| {
                    log.borrow_mut().push(format!("archived {}", cx.selected.len()));
                    Ok(())
                }),
            Action::popup("Link", "Link", "Bank_ListView").on_execute(move |cx| {
                picked.borrow_mut().push(format!("linked {:?}", cx.input));
                Ok(())
            }),
        ]
    }
}

impl Module for Extra {
    fn name(&self) -> &str {
        "Extra"
    }
    fn dependencies(&self) -> Vec<String> {
        vec!["System".to_owned()]
    }
    fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
        vec![Box::new(ExtraController(self.0.clone()))]
    }
}

#[test]
fn a_modules_actions_show_in_their_place_and_follow_the_selection() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let module = Extra(log.clone());
    let mut w = world_with(move |builder| builder.module(module));
    w.add("Corrente");
    assert!(!w.enabled("Archive"), "needs one selected");
    w.check_row("Corrente");
    assert!(w.enabled("Archive"));
    w.press("Archive");
    assert_eq!(*log.borrow(), ["archived 1"]);
    w.press("Banks");
    assert!(w.find(Role::Button, "Archive").is_none(), "out of place on another entity");
}

#[test]
fn a_popup_shows_a_list_and_hands_over_the_record_picked() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let module = Extra(log.clone());
    let mut w = world_with(move |builder| builder.module(module));
    w.press("Banks");
    w.press("New");
    w.fill("Code", "341");
    w.fill("Name", "Itaú");
    w.press("Save and close");
    w.press("Accounts");
    w.press("Link");
    assert!(!w.nodes(Role::Dialog).is_empty());
    assert!(w.says("Itaú"), "the list of the other entity");
    w.check_row("Itaú");
    w.press("OK");
    assert!(w.nodes(Role::Dialog).is_empty());
    assert_eq!(*log.borrow(), ["linked Parameter(Text(\"341\"))"]);
    w.press("Link");
    let last_cancel = w
        .nodes(Role::Button)
        .into_iter()
        .rfind(|n| n.name.as_deref() == Some("Cancel"))
        .expect("its button")
        .bounds
        .center();
    w.click_at(last_cancel);
    assert!(w.nodes(Role::Dialog).is_empty());
    assert_eq!(log.borrow().len(), 1, "nothing ran");
}

#[test]
fn a_person_changing_the_model_changes_the_screen() {
    let mut w = {
        let database = MemoryDatabase::new(entities()).expect("valid");
        let mut application = entities()
            .into_iter()
            .fold(Application::builder("Test").module(SystemModule::new()), ApplicationBuilder::entity)
            .build()
            .expect("valid");
        application.layers_mut().set(
            &ModelLayer::User,
            &["Views", "Account_ListView", "Columns", "opening"],
            "Index",
            (-1).into(),
        );
        application.layers_mut().set(
            &ModelLayer::User,
            &["NavigationItems", "Records", "Bank"],
            "Caption",
            "Instituições".into(),
        );
        application.refresh_model().expect("valid");
        let source = database.clone();
        let shell = AppShell::new(application, move || source.space());
        let session = shell.session();
        let mut ui = Ui::new(shell, Theme::raven_dark());
        ui.set_viewport(Size::new(1200.0, 800.0), 1.0);
        ui.update();
        World { ui, database, session, files: Rc::default() }
    };
    assert!(w.says("Instituições"));
    assert!(!w.says("Banks"));
    assert!(!w.says("Opening"), "a negative index keeps the column out");
    assert!(w.says("Name"));
}

#[test]
fn the_words_can_be_portuguese() {
    let application = entities()
        .into_iter()
        .fold(
            Application::builder("Teste").module(SystemModule::with_words(Words::pt_br())),
            ApplicationBuilder::entity,
        )
        .build()
        .expect("valid");
    let database = MemoryDatabase::new(entities()).expect("valid");
    let shell = AppShell::with_options(
        application,
        move || database.space(),
        Words::pt_br(),
        EditorRegistry::default(),
    );
    let mut ui = Ui::new(shell, Theme::raven_dark());
    ui.set_viewport(Size::new(1200.0, 800.0), 1.0);
    ui.update();
    let tree = ui.access_tree();
    assert!(tree.nodes.iter().any(|n| n.role == Role::Button && n.name.as_deref() == Some("Novo")));
}
