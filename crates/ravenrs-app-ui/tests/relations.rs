//! Records that point to each other, and the windows and views that come with that.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_app::*;
use ravenrs_app_ui::*;
use ravenrs_core::{AccessAction, AccessNode, Event, Key, Modifiers, PointerButton, Role, Size};
use ravenrs_style::Theme;
use ravenrs_widgets::{AnyView, ButtonStyle, HStack, Text, Ui, VStack, View};

fn entities() -> Vec<EntityInfo> {
    vec![
        EntityInfo::new("Customer")
            .caption("Customers")
            .navigation("Sales")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("name", FieldKind::Text).caption("Name").default_property())
            .field(FieldInfo::new("orders", FieldKind::Collection("Order".into())).caption("Orders")),
        EntityInfo::new("Order")
            .caption("Orders")
            .navigation("Sales")
            .variant(ViewVariant::new("all", "All"))
            .variant(ViewVariant::new("open", "Open").filter("[status] = 'open'"))
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("title", FieldKind::Text).caption("Title").default_property())
            .field(
                FieldInfo::new(
                    "status",
                    FieldKind::Enum(vec![EnumOption::new("open", "Open"), EnumOption::new("done", "Done")]),
                )
                .caption("Status"),
            )
            .field(FieldInfo::new("customer", FieldKind::Reference("Customer".into())).caption("Customer")),
        EntityInfo::new("Tag")
            .caption("Tags")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("name", FieldKind::Text).caption("Name").default_property())
            .field(
                FieldInfo::new(
                    "side",
                    FieldKind::Enum(vec![EnumOption::new("a", "A"), EnumOption::new("b", "B")]),
                )
                .caption("Side"),
            ),
        EntityInfo::new("Note")
            .caption("Notes")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(
                FieldInfo::new(
                    "side",
                    FieldKind::Enum(vec![EnumOption::new("a", "A"), EnumOption::new("b", "B")]),
                )
                .caption("Side")
                .editor("Segmented"),
            )
            .field(FieldInfo::new("text", FieldKind::Text).caption("Text").wide())
            .field(
                FieldInfo::new("tag", FieldKind::Reference("Tag".into()))
                    .caption("Tag")
                    .editor("Select")
                    .lookup_filter("[side] = ?side"),
            )
            .field(
                FieldInfo::new("owner", FieldKind::Reference("Customer".into()))
                    .caption("Owner")
                    .editor("Select")
                    .none_caption("Nobody"),
            )
            .rule(Rule::required("text")),
        EntityInfo::new("Payment")
            .caption("Payment")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("amount", FieldKind::Integer).caption("Amount"))
            .rule(Rule::required("amount")),
    ]
}

fn database() -> MemoryDatabase {
    let database = MemoryDatabase::new(entities()).expect("valid");
    let mut space = database.space();
    let mut add = |entity: &str, values: &[(&str, Value)]| {
        let mut object = space.new_object(entity).expect("known");
        for (field, value) in values {
            object.set(field, value.clone());
        }
        space.save(object).expect("saves");
    };
    let text = |t: &str| Value::Text(t.to_owned());
    add("Customer", &[("name", text("Ana"))]);
    add("Customer", &[("name", text("Bia"))]);
    add("Order", &[("title", text("Mesa")), ("status", text("open")), ("customer", Value::Number(1.0))]);
    add("Order", &[("title", text("Vaso")), ("status", text("done")), ("customer", Value::Number(1.0))]);
    add("Order", &[("title", text("Lata")), ("status", text("open")), ("customer", Value::Number(2.0))]);
    add("Tag", &[("name", text("Azul")), ("side", text("a"))]);
    add("Tag", &[("name", text("Bege")), ("side", text("b"))]);
    space.commit().expect("keeps");
    database
}

struct World {
    ui: Ui,
    database: MemoryDatabase,
    session: Session,
}

fn shell_world(setup: impl FnOnce(ApplicationBuilder) -> ApplicationBuilder) -> World {
    let database = database();
    let builder = entities()
        .into_iter()
        .fold(Application::builder("Test").module(SystemModule::new()), ApplicationBuilder::entity);
    let application = setup(builder).build().expect("valid");
    let source = database.clone();
    let shell = AppShell::new(application, move || source.space());
    let session = shell.session();
    World::over(shell, database, session)
}

impl World {
    fn over(view: impl View + 'static, database: MemoryDatabase, session: Session) -> Self {
        let mut ui = Ui::new(view, Theme::raven_dark());
        ui.set_viewport(Size::new(1200.0, 900.0), 1.0);
        ui.update();
        ui.update();
        Self { ui, database, session }
    }

    fn settle(&mut self) {
        self.ui.update();
        self.ui.update();
        self.ui.update();
    }

    fn names(&mut self, role: Role) -> Vec<String> {
        self.ui.access_tree().nodes.iter().filter(|n| n.role == role).filter_map(|n| n.name.clone()).collect()
    }

    fn says(&mut self, text: &str) -> bool {
        self.ui.access_tree().nodes.iter().any(|n| {
            [&n.name, &n.value, &n.description].iter().any(|t| t.as_deref().is_some_and(|t| t.contains(text)))
        })
    }

    fn act(&mut self, role: Role, name: &str, action: AccessAction) {
        let tree = self.ui.access_tree();
        let found = tree.nodes.iter().find(|n| n.role == role && n.name.as_deref() == Some(name));
        let id = found.unwrap_or_else(|| panic!("no {role:?} {name} in {:?}", self.names(role))).id;
        self.ui.perform_action(id, action);
        self.settle();
    }

    fn press(&mut self, name: &str) {
        self.act(Role::Button, name, AccessAction::Click);
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
        for character in text.chars() {
            self.ui.handle_event(&Event::KeyDown {
                key: Key::Character(character.to_string()),
                modifiers: Modifiers::default(),
            });
        }
        self.settle();
    }

    fn select_row(&mut self, text: &str) {
        self.select_cell(text, false);
    }

    /// Selects the row of a cell, by its check box if it has one. The last, for a window over a screen.
    fn select_cell(&mut self, text: &str, last: bool) {
        let tree = self.ui.access_tree();
        let is_it = |n: &&AccessNode| {
            n.role == Role::Cell && (n.name.as_deref() == Some(text) || n.value.as_deref() == Some(text))
        };
        let cell = if last { tree.nodes.iter().rfind(is_it) } else { tree.nodes.iter().find(is_it) };
        let cell = cell.unwrap_or_else(|| panic!("no row with {text}")).bounds;
        let check = tree
            .nodes
            .iter()
            .rfind(|n| n.role == Role::CheckBox && (n.bounds.center().y - cell.center().y).abs() < 4.0);
        let spot = check.map_or(cell.center(), |check| check.bounds.center());
        self.ui.handle_event(&Event::PointerMove { position: spot });
        self.ui.handle_event(&Event::PointerDown { position: spot, button: PointerButton::Primary });
        self.ui.handle_event(&Event::PointerUp { position: spot, button: PointerButton::Primary });
        self.settle();
    }

    fn count(&self, entity: &str, filter: &str) -> usize {
        let query = Query::all().filter(Criteria::parse(filter).expect("parses"));
        self.database.space().query(entity, &query).expect("reads").len()
    }
}

/// A form in a window, on its own.
fn form_world(saved: Rc<RefCell<Vec<Object>>>, answer: bool) -> (World, EntityForm) {
    let database = database();
    let source = database.clone();
    let form = EntityForm::new(
        "Note",
        entities(),
        move || source.space(),
        Words::default(),
        EditorRegistry::default(),
    )
    .expect("valid")
    .on_save(move |record, _| {
        saved.borrow_mut().push(record.clone());
        answer
    });
    let session = form.session().clone();
    (World::over(form.clone(), database, session), form)
}

#[test]
fn a_form_in_a_window_checks_its_rules_and_hands_the_record_over() {
    let saved = Rc::new(RefCell::new(Vec::new()));
    let (mut w, form) = form_world(saved.clone(), true);
    assert!(!w.says("New note"), "closed until it is asked for");
    form.start_new(FormText::new("New note", "Keep note").description("It goes to the wall."), |note| {
        note.set("side", "a");
    });
    w.settle();
    assert!(w.says("New note"));
    assert!(w.says("It goes to the wall."));

    w.press("Keep note");
    assert!(saved.borrow().is_empty(), "the rule stopped it");
    assert!(w.says("New note"), "and it stays open");

    w.type_into("Text", "Olá");
    w.press("Keep note");
    assert_eq!(saved.borrow().len(), 1);
    assert_eq!(saved.borrow()[0].get("text"), Value::Text("Olá".into()));
    assert!(!w.says("New note"), "it closed");
}

#[test]
fn a_form_stays_open_when_whoever_keeps_the_record_refuses() {
    let saved = Rc::new(RefCell::new(Vec::new()));
    let (mut w, form) = form_world(saved.clone(), false);
    form.start_new(FormText::new("New note", "Keep note"), |note| note.set("text", "x"));
    w.settle();
    w.press("Keep note");
    assert_eq!(saved.borrow().len(), 1);
    assert!(w.says("New note"));
}

#[test]
fn a_reference_picked_from_a_list_follows_the_field_it_depends_on() {
    let saved = Rc::new(RefCell::new(Vec::new()));
    let (mut w, form) = form_world(saved.clone(), true);
    form.start_new(FormText::new("New note", "Keep note"), |note| {
        note.set("text", "x");
        note.set("side", "a");
    });
    w.settle();
    w.press_radio_then_save("B", "Keep note");
    let record = saved.borrow()[0].clone();
    assert_eq!(record.get("side"), Value::Text("b".into()));
    assert_eq!(record.get("tag"), Value::Number(7.0), "the tag of the other side took its place");
    assert_eq!(record.get("owner"), Value::Null, "with a word for empty, it can stay empty");
}

impl World {
    fn press_radio_then_save(&mut self, radio: &str, save: &str) {
        self.act(Role::RadioButton, radio, AccessAction::Click);
        self.press(save);
    }
}

#[test]
fn starting_on_a_record_reads_it_and_fill_changes_it_first() {
    let saved = Rc::new(RefCell::new(Vec::new()));
    let database = database();
    let source = database.clone();
    let log = saved.clone();
    let form = EntityForm::new(
        "Order",
        entities(),
        move || source.space(),
        Words::default(),
        EditorRegistry::default(),
    )
    .expect("valid")
    .on_save(move |record: &Object, is_new| {
        log.borrow_mut().push((record.get("title"), is_new));
        true
    });
    let session = form.session().clone();
    let mut w = World::over(form.clone(), database, session);
    // Keys are numbered across entities, and two customers came first.
    form.start_edit(FormText::new("Order", "Keep"), &Value::Number(3.0), |_| {});
    w.settle();
    form.set("title", "Mesa grande");
    w.settle();
    w.press("Keep");
    assert_eq!(*saved.borrow(), [(Value::Text("Mesa grande".into()), false)]);
}

#[test]
fn a_list_inside_a_form_shows_what_points_to_the_record_and_links_and_unlinks() {
    let mut w = shell_world(|builder| builder);
    w.session.open_detail("Customer", &Value::Number(1.0));
    w.settle();
    assert!(w.says("Mesa"), "Ana's");
    assert!(!w.says("Lata"), "and not Bia's");

    w.select_row("Vaso");
    w.press("Unlink");
    assert_eq!(w.count("Order", "[customer] = 1"), 1, "taken off the list");
    assert_eq!(w.count("Order", "[title] = 'Vaso'"), 1, "but not removed");
    assert!(!w.says("Vaso"));

    w.press("Link");
    assert!(w.says("Lata"), "the list to pick from");
    w.select_cell("Lata", true);
    w.press("OK");
    assert_eq!(w.count("Order", "[customer] = 1"), 2, "kept right away");
    assert_eq!(w.count("Order", "[customer] = 2"), 0);
}

#[test]
fn a_new_record_has_no_list_inside_yet_and_says_so() {
    let mut w = shell_world(|builder| builder);
    w.session.open_new("Customer");
    w.settle();
    assert!(w.says("Save this record"));
    assert!(w.names(Role::Button).iter().all(|name| name != "Link"));
}

#[test]
fn two_ways_to_see_a_list_bring_a_menu_and_the_choice_stays_in_the_persons_layer() {
    let mut w = shell_world(|builder| builder);
    w.session.open_view("Order_ListView");
    w.settle();
    assert!(w.says("Vaso"), "the first way shows all of them");
    w.press("View");
    w.act(Role::MenuItem, "Open", AccessAction::Click);
    assert!(!w.says("Vaso"), "only the open ones");
    assert!(w.says("Mesa"));
    let settings = w.session.user_settings().expect("a layer");
    assert!(settings.contains("CurrentVariant") && settings.contains("open"), "{settings}");
    w.session.open_view("Customer_ListView");
    w.settle();
    assert!(!w.names(Role::Button).iter().any(|name| name == "View"), "one way is no choice");
}

struct AskForPayment(Rc<RefCell<Vec<Object>>>);

impl ViewController for AskForPayment {
    fn id(&self) -> &str {
        "AskForPayment"
    }
    fn actions(&self) -> Vec<Action> {
        let log = self.0.clone();
        vec![
            Action::popup("Pay", "Pay", "Payment_DetailView")
                .target_entity("Order")
                .on_prepare(|_, payment| payment.set("amount", 5.0))
                .on_execute(move |cx| {
                    if let ActionInput::Record(record) = &cx.input {
                        log.borrow_mut().push(record.clone());
                    }
                    Ok(())
                }),
            Action::parametrized("Grow", "Grow by", FieldKind::Integer).target_entity("Order").on_execute({
                let log = self.0.clone();
                move |cx| {
                    if let ActionInput::Parameter(value) = &cx.input {
                        let mut record = Object::from_values("Payment", Default::default());
                        record.set("amount", value.clone());
                        log.borrow_mut().push(record);
                    }
                    Ok(())
                }
            }),
        ]
    }
}

struct Payments(Rc<RefCell<Vec<Object>>>);

impl Module for Payments {
    fn name(&self) -> &str {
        "Payments"
    }
    fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
        vec![Box::new(AskForPayment(self.0.clone()))]
    }
}

#[test]
fn a_popup_can_ask_for_a_record_to_be_filled_in() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let module = Payments(log.clone());
    let mut w = shell_world(|builder| builder.module(module));
    w.session.open_view("Order_ListView");
    w.settle();
    w.press("Pay");
    assert!(w.says("Amount"), "the form");
    w.press("OK");
    assert_eq!(log.borrow()[0].get("amount"), Value::Number(5.0), "with what it was filled in with");
    assert!(!w.says("Amount"), "and it closed");
}

#[test]
fn a_popup_with_a_form_keeps_to_its_rules() {
    struct Empty;
    impl ViewController for Empty {
        fn id(&self) -> &str {
            "Empty"
        }
        fn actions(&self) -> Vec<Action> {
            vec![Action::popup("Pay", "Pay", "Payment_DetailView").on_execute(|_| Err("no".to_owned()))]
        }
    }
    struct M;
    impl Module for M {
        fn name(&self) -> &str {
            "M"
        }
        fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
            vec![Box::new(Empty)]
        }
    }
    let mut w = shell_world(|builder| builder.module(M));
    w.press("Pay");
    w.press("OK");
    assert!(w.says("Amount"), "the rule stopped it");
    w.type_into("Amount", "7");
    w.press("OK");
    assert!(w.says("no"), "what went wrong shows inside, and it stays open to try again");
    assert!(w.says("Amount"));
}

#[test]
fn a_value_an_action_asks_for_arrives_as_what_it_asks_for() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let module = Payments(log.clone());
    let mut w = shell_world(|builder| builder.module(module));
    w.session.open_view("Order_ListView");
    w.settle();
    w.type_into("Grow by", "12");
    w.ui.handle_event(&Event::KeyDown { key: Key::Enter, modifiers: Modifiers::default() });
    w.settle();
    assert_eq!(log.borrow()[0].get("amount"), Value::Number(12.0));
}

#[test]
fn a_dashboard_shows_views_and_controls_together_each_on_its_own() {
    let mut w = shell_world(|builder| {
        builder.dashboard(
            DashboardInfo::new("Home", "Home")
                .navigation("Start")
                .view("All customers", "Customer_ListView")
                .view("All orders", "Order_ListView")
                .control("Hello", "hello"),
        )
    });
    w.session.register_control("hello", || Box::new(Text::new("From the application")) as AnyView);
    w.session.open_view("Home_DashboardView");
    w.settle();
    for words in ["All customers", "All orders", "Hello", "Ana", "Mesa", "From the application"] {
        assert!(w.says(words), "{words}");
    }
    assert!(!w.names(Role::Button).iter().any(|name| name == "New"), "it has no records to create");
}

#[test]
fn a_dashboard_pointing_at_nothing_is_refused_when_put_together() {
    let result = Application::builder("Test")
        .entity(entities().remove(0))
        .dashboard(DashboardInfo::new("Home", "Home").view("Gone", "Gone_ListView"))
        .build();
    assert!(result.is_err_and(|e| e.0.contains("Gone_ListView")));
}

#[test]
fn a_variant_with_a_bad_expression_is_refused_when_put_together() {
    let entity = EntityInfo::new("A")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .variant(ViewVariant::new("v", "V").filter("[nope] = 1"));
    assert!(entity.check().is_err_and(|e| e.0.contains('v')));
}

fn screen_session() -> (Session, MemoryDatabase) {
    let database = database();
    let source = database.clone();
    let application = entities()
        .into_iter()
        .fold(Application::builder("Test"), ApplicationBuilder::entity)
        .module(Commands)
        .build()
        .expect("valid");
    let session = Session::embedded(application, move || source.space(), Words::default());
    session.show_list("Order_ListView");
    (session, database)
}

struct Commands;

struct OrderCommands;

impl ViewController for OrderCommands {
    fn id(&self) -> &str {
        "OrderCommands"
    }
    fn actions(&self) -> Vec<Action> {
        vec![
            Action::simple("Close", "Close order")
                .icon("check")
                .selection(Selection::RequireSingle)
                .target_criteria("[status] = 'open'", CriteriaMode::TrueForAll)
                .confirmation("Close it?")
                .confirmation_words("It stays on the list.", "Yes, close")
                .on_execute(|cx| {
                    let mut order = cx.selected[0].clone();
                    order.set("status", "done");
                    let mut space = cx.space.borrow_mut();
                    space.save(order).map_err(|e| e.to_string())?;
                    space.commit().map_err(|e| e.to_string())?;
                    Session::of(cx).expect("a session").records_changed();
                    Ok(())
                }),
            Action::simple("Reopen", "Reopen")
                .selection(Selection::RequireSingle)
                .target_criteria("[status] = 'done'", CriteriaMode::TrueForAll)
                .on_execute(|_| Ok(())),
        ]
    }
}

impl Module for Commands {
    fn name(&self) -> &str {
        "Commands"
    }
    fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
        vec![Box::new(OrderCommands)]
    }
}

/// A screen an application made itself, with a list and buttons of the framework.
struct Screen(Session, Option<Criteria>, bool);

impl View for Screen {
    fn build(&self, cx: &mut ravenrs_widgets::BuildCx<'_>) -> ravenrs_widgets::NodeId {
        let session = self.0.clone();
        let view = session.frame().view().expect("a view");
        let list = ListView::new(session.clone(), session.frame(), view)
            .filter(self.1.clone())
            .sort("title", true)
            .single_selection()
            .plain_toolbar();
        let list: AnyView = if self.2 { Box::new(list.remote()) } else { Box::new(list) };
        cx.child(&VStack::new((
            HStack::new((
                ActionButton::new(session.clone(), "Close").style(ButtonStyle::Secondary),
                ActionButton::new(session.clone(), "Reopen").hide_when_off(),
            )),
            ravenrs_widgets::ViewExt::height(list, 400.0),
            session.confirmations(),
        )))
    }
}

#[test]
fn a_screen_of_its_own_uses_lists_and_buttons_of_the_framework() {
    let (session, database) = screen_session();
    let mut w = World::over(Screen(session.clone(), None, false), database, session);
    assert!(w.names(Role::Button).iter().any(|name| name == "Close order"), "faded, and in its place");
    assert!(!w.names(Role::Button).iter().any(|name| name == "Reopen"), "hidden while it cannot be used");

    w.select_row("Mesa");
    w.press("Close order");
    assert!(w.says("It stays on the list."), "the question says what it was told to");
    w.press("Yes, close");
    assert_eq!(w.count("Order", "[status] = 'done'"), 2);
    assert!(w.names(Role::Button).iter().any(|name| name == "Reopen"), "judged as it is now");
}

#[test]
fn a_filter_from_the_screen_narrows_the_list() {
    let (session, database) = screen_session();
    let filter = Criteria::parse("[customer] = 2").ok();
    let mut w = World::over(Screen(session.clone(), filter, false), database, session);
    assert!(w.says("Lata"));
    assert!(!w.says("Mesa"));
}

#[test]
fn a_remote_list_brings_its_rows_from_storage() {
    let (session, database) = screen_session();
    let filter = Criteria::parse("[customer] = 1").ok();
    let mut w = World::over(Screen(session.clone(), filter, true), database, session);
    for _ in 0..4 {
        w.settle();
    }
    assert!(w.says("Mesa") && w.says("Vaso"));
    assert!(!w.says("Lata"), "it asks storage for what it was told to");
}

#[test]
fn pictures_and_files_have_controls_on_a_form() {
    let entity = || {
        EntityInfo::new("Doc")
            .navigation("Docs")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("scan", FieldKind::Image).caption("Scan"))
            .field(FieldInfo::new("paper", FieldKind::File).caption("Paper"))
    };
    let database = MemoryDatabase::new(vec![entity()]).expect("valid");
    let source = database.clone();
    let application =
        Application::builder("Test").module(SystemModule::new()).entity(entity()).build().expect("valid");
    let shell = AppShell::new(application, move || source.space());
    let session = shell.session();
    let mut w = World::over(shell, database, session.clone());
    session.open_new("Doc");
    w.settle();
    let buttons = w.names(Role::Button);
    assert!(buttons.iter().any(|name| name == "Choose image"), "{buttons:?}");
    assert!(buttons.iter().any(|name| name == "Choose files"), "{buttons:?}");
}
