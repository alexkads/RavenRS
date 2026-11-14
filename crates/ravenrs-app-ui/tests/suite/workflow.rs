//! Transitions on the screens and reminders that come up by themselves.
#![allow(clippy::expect_used, clippy::panic)]

use ravenrs_app::{
    AppearanceRule, Application, EntityInfo, EnumOption, FieldInfo, FieldKind, MemoryDatabase, ObjectSpace,
    Query, Reminder, StateMachine, Value, local_now,
};
use ravenrs_app_ui::{AppShell, Session, SystemModule};
use ravenrs_core::{AccessAction, Role};
use ravenrs_criteria::DateTime;
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

fn task() -> EntityInfo {
    let options = vec![EnumOption::new("open", "Open"), EnumOption::new("done", "Done")];
    EntityInfo::new("Task")
        .caption("Tasks")
        .navigation("Work")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("title", FieldKind::Text).default_property())
        .field(FieldInfo::new("state", FieldKind::Enum(options)))
        .field(FieldInfo::new("notes", FieldKind::Text))
        .field(FieldInfo::new("remind_at", FieldKind::DateTime))
        .state_machine(
            StateMachine::new("state")
                .state("open", "Open")
                .state("done", "Done")
                .start("open")
                .transition("open", "done", "Finish"),
        )
        .state_appearance("state", "done", AppearanceRule::new("locked", ["notes"]).enabled(false))
        .reminder(Reminder::new("remind_at"))
}

fn moment(seconds_from_now: i64) -> Value {
    let now = local_now();
    let at = now.days * 86_400 + i64::from(now.seconds) + seconds_from_now;
    Value::Date(DateTime { days: at.div_euclid(86_400), seconds: at.rem_euclid(86_400) as u32 })
}

fn database(remind: Option<i64>) -> MemoryDatabase {
    let database = MemoryDatabase::new(vec![task()]).expect("valid");
    let mut space = database.space();
    let mut record = space.new_object("Task").expect("known");
    record.set("id", 1.0);
    record.set("title", "Roof");
    if let Some(seconds) = remind {
        record.set("remind_at", moment(seconds));
    }
    space.save(record).expect("waits");
    space.commit().expect("keeps");
    database
}

struct World {
    ui: Ui,
    session: Session,
}

fn world(database: &MemoryDatabase) -> World {
    let application =
        Application::builder("Test").module(SystemModule::new()).entity(task()).build().expect("valid");
    let source = database.clone();
    let shell = AppShell::new(application, move || source.space());
    let session = shell.session();
    let mut world = World { ui: Ui::new(shell, Theme::raven_dark()), session };
    world.settle();
    world
}

impl World {
    fn settle(&mut self) {
        for _ in 0..4 {
            self.ui.update();
        }
    }

    fn says(&mut self, text: &str) -> bool {
        self.settle();
        self.ui.access_tree().nodes.iter().any(|node| {
            node.name.as_deref().is_some_and(|name| name.contains(text))
                || node.value.as_deref().is_some_and(|value| value.contains(text))
        })
    }

    fn has_button(&mut self, name: &str) -> bool {
        self.settle();
        self.ui.access_tree().nodes.iter().any(|n| n.role == Role::Button && n.name.as_deref() == Some(name))
    }

    fn press(&mut self, name: &str) {
        self.settle();
        let tree = self.ui.access_tree();
        let names: Vec<_> = tree.nodes.iter().filter_map(|n| n.name.clone()).collect();
        let found = tree.nodes.iter().find(|n| n.role == Role::Button && n.name.as_deref() == Some(name));
        let id = found.unwrap_or_else(|| panic!("no button {name} in {names:?}")).id;
        self.ui.perform_action(id, AccessAction::Click);
        self.settle();
    }
}

fn stored(database: &MemoryDatabase) -> ravenrs_app::Object {
    database.space().query("Task", &Query::all()).expect("reads").remove(0)
}

#[test]
fn a_form_changes_state_by_its_action_shows_it_and_locks_what_the_state_says() {
    let database = database(None);
    let mut w = world(&database);
    w.session.open_detail("Task", &Value::Number(1.0));
    w.settle();
    let finish = w.session.frame().action("Task.state.open.done").expect("there");
    assert!(finish.is_enabled());
    w.session.run_action(&finish, ravenrs_app::ActionInput::None);
    w.settle();
    assert_eq!(stored(&database).get("state"), Value::Text("done".into()));
    assert!(w.says("Done"), "the form shows it");
    assert!(!finish.is_enabled(), "and the action goes by what it is now");
    let locked = w.ui.access_tree();
    let notes = locked
        .nodes
        .iter()
        .find(|n| {
            n.role == Role::TextInput && n.name.as_deref().is_some_and(|name| name.starts_with("Notes"))
        })
        .expect("there");
    assert!(notes.disabled || notes.read_only, "{notes:?}");
}

#[test]
fn what_someone_typed_stays_when_an_action_changes_the_record() {
    let database = database(None);
    let mut w = world(&database);
    w.session.open_detail("Task", &Value::Number(1.0));
    w.settle();
    w.session.set_form_value("title", Value::Text("Bigger roof".into()));
    w.settle();
    let finish = w.session.frame().action("Task.state.open.done").expect("there");
    w.session.run_action(&finish, ravenrs_app::ActionInput::None);
    w.settle();
    assert!(w.session.has_unsaved_changes());
    let action = w.session.frame().action("Save").expect("there");
    w.session.run_action(&action, ravenrs_app::ActionInput::None);
    w.settle();
    let record = stored(&database);
    assert_eq!(record.get("title"), Value::Text("Bigger roof".into()));
    assert_eq!(record.get("state"), Value::Text("done".into()), "and saving keeps both");
}

#[test]
fn a_new_form_starts_in_the_first_state_and_goes_through() {
    let database = database(None);
    let mut w = world(&database);
    w.session.open_new("Task");
    w.settle();
    w.session.set_form_value("id", Value::Number(5.0));
    w.session.set_form_value("title", Value::Text("Walls".into()));
    let action = w.session.frame().action("Save").expect("there");
    w.session.run_action(&action, ravenrs_app::ActionInput::None);
    w.settle();
    assert!(w.session.error().is_none(), "{:?}", w.session.error());
    let all = database.space().query("Task", &Query::all()).expect("reads");
    assert!(all.iter().any(|record| record.get("state") == Value::Text("open".into())
        && record.get("title") == Value::Text("Walls".into())));
}

#[test]
fn a_reminder_due_comes_up_with_what_to_do() {
    let database = database(Some(-60));
    let mut w = world(&database);
    assert!(w.has_button("Dismiss"), "it came up by itself");
    assert!(w.says("Roof"));
    w.press("In 15 minutes");
    assert!(!w.has_button("Dismiss"));
    let Value::Date(at) = stored(&database).get("remind_at") else { panic!("a time is kept") };
    let now = local_now();
    assert!(at.days * 86_400 + i64::from(at.seconds) > now.days * 86_400 + i64::from(now.seconds));
}

#[test]
fn dismissing_takes_it_away_for_good_and_opening_goes_to_the_record() {
    let database = database(Some(-60));
    let mut w = world(&database);
    w.press("Dismiss");
    assert_eq!(stored(&database).get("remind_at"), Value::Null);
    assert!(!w.has_button("Dismiss"));

    let database = self::database(Some(-60));
    let mut w = world(&database);
    w.press("Open");
    assert!(w.session.form_record().is_some());
    assert!(!w.has_button("Dismiss"), "it steps aside");
    assert!(w.has_button("Reminders (1)"), "and can be brought back");
    w.press("Reminders (1)");
    assert!(w.has_button("Dismiss"));
}

#[test]
fn one_not_due_does_not_show_and_nothing_runs_for_it() {
    let database = database(Some(3_600));
    let mut w = world(&database);
    assert!(!w.has_button("Dismiss"));
    assert!(!w.says("Reminders"));
    // It asks to be built again at the time due, and not before.
    assert!(w.ui.next_wake().is_some_and(|wake| wake > 3_000.0), "{:?}", w.ui.next_wake());
}

#[test]
fn nothing_with_a_time_asks_for_nothing() {
    let database = database(None);
    let mut w = world(&database);
    w.settle();
    assert!(w.ui.next_wake().is_none_or(|wake| wake < 10.0));
    assert!(!w.has_button("Dismiss"));
}

#[test]
fn an_action_that_went_to_the_overflow_menu_still_runs_by_pointer() {
    use ravenrs_core::{Event, Point, PointerButton, Size};
    let database = database(None);
    let mut w = world(&database);
    w.ui.set_viewport(Size::new(300.0, 800.0), 1.0);
    w.session.open_view("Task_ListView");
    w.settle();
    let record = stored(&database);
    w.session.frame().set_selected(vec![record]);
    w.settle();
    let click = |w: &mut World, at: Point| {
        w.ui.handle_event(&Event::PointerMove { position: at });
        w.ui.update();
        w.ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
        w.ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
        w.settle();
    };
    let bounds_of = |w: &mut World, name: &str| {
        w.settle();
        let tree = w.ui.access_tree();
        let names: Vec<_> = tree.nodes.iter().filter_map(|n| n.name.clone()).collect();
        tree.nodes
            .iter()
            .find(|n| n.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("no {name} in {names:?}"))
            .bounds
    };
    let more = bounds_of(&mut w, "More");
    click(&mut w, more.center());
    let item = bounds_of(&mut w, "Finish");
    click(&mut w, item.center());
    assert_eq!(stored(&database).get("state"), Value::Text("done".into()));
}

#[test]
fn a_long_navigation_scrolls_and_leaves_the_window_as_it_is() {
    use ravenrs_core::Size;
    let mut builder = Application::builder("Test").module(SystemModule::new()).entity(task());
    let mut entities = vec![task()];
    for at in 0..40 {
        let entity = EntityInfo::new(format!("Thing{at}"))
            .caption(format!("Thing {at}"))
            .navigation("Many")
            .field(FieldInfo::new("id", FieldKind::Integer).key());
        entities.push(entity.clone());
        builder = builder.entity(entity);
    }
    let application = builder.build().expect("valid");
    let database = MemoryDatabase::new(entities).expect("valid");
    let shell = AppShell::new(application, move || database.space());
    let mut ui = Ui::new(shell, Theme::raven_dark());
    ui.set_viewport(Size::new(1000.0, 500.0), 1.0);
    for _ in 0..4 {
        ui.update();
    }
    let tree = ui.access_tree();
    let navigation = tree.nodes.iter().find(|n| n.role == Role::Navigation).expect("there");
    assert!(
        navigation.bounds.max_y() <= 500.5,
        "the navigation stays in the window: {:?}",
        navigation.bounds
    );
}

#[test]
fn the_window_offers_the_themes_and_changes_to_the_one_chosen() {
    let database = database(None);
    let mut w = world(&database);
    assert!(w.has_button("Theme"));
    assert_eq!(w.ui.theme().name, Theme::raven_dark().name);
    w.press("Theme");
    let light =
        w.ui.access_tree()
            .nodes
            .iter()
            .find(|node| node.name.as_deref() == Some("Light"))
            .expect("the light theme")
            .id;
    w.ui.perform_action(light, ravenrs_core::AccessAction::Click);
    w.settle();
    assert_eq!(w.ui.theme().name, Theme::raven_light().name);
}

#[test]
fn following_the_system_is_there_the_next_time_it_opens() {
    // #289: choosing "follow the system" outlives the window, and so does choosing by hand.
    let store = ravenrs_app::MemorySettings::default();
    let database = database(None);
    let open = |system_dark: bool| {
        let source = database.clone();
        let view = ravenrs_app_ui::App::new("Temas")
            .entity_info(task())
            .storage(move || source.space())
            .settings(store.clone())
            .build()
            .expect("it puts together");
        let mut ui = Ui::new(view, Theme::raven_dark());
        ui.set_viewport(ravenrs_core::Size::new(1200.0, 800.0), 1.0);
        ui.update();
        // The platform says what the system is once the window is up.
        ui.set_system_dark(system_dark);
        for _ in 0..4 {
            ui.update();
        }
        ui
    };
    let mut first = open(false);
    assert!(!first.system_appearance().follows(), "nobody chose to follow yet");
    first.follow_system_theme(true);
    for _ in 0..4 {
        first.update();
    }
    assert_eq!(first.theme().name, Theme::raven_light().name);

    let mut again = open(true);
    assert!(again.system_appearance().follows(), "it still follows");
    assert_eq!(again.theme().name, Theme::raven_dark().name, "and the system is dark now");
    again.set_system_dark(false);
    again.update();
    assert_eq!(again.theme().name, Theme::raven_light().name, "it goes on following");

    // Light by hand: no longer following, even with the system dark.
    again.follow_system_theme(false);
    again.theme_state().set(Theme::raven_light());
    for _ in 0..4 {
        again.update();
    }
    let by_hand = open(true);
    assert!(!by_hand.system_appearance().follows());
    assert_eq!(by_hand.theme().name, Theme::raven_light().name);
}

#[test]
fn the_theme_someone_chose_is_there_the_next_time_it_opens() {
    let store = ravenrs_app::MemorySettings::default();
    let database = database(None);
    let open = |store: ravenrs_app::MemorySettings| {
        let source = database.clone();
        let view = ravenrs_app_ui::App::new("Temas")
            .entity_info(task())
            .storage(move || source.space())
            .settings(store)
            .build()
            .expect("it puts together");
        let mut ui = Ui::new(view, Theme::raven_dark());
        ui.set_viewport(ravenrs_core::Size::new(1200.0, 800.0), 1.0);
        for _ in 0..4 {
            ui.update();
        }
        ui
    };
    let mut first = open(store.clone());
    assert_eq!(first.theme().name, Theme::raven_dark().name);
    let mut edited = Theme::raven_light();
    edited.radii.md = 3.0;
    first.theme_state().set(edited);
    for _ in 0..4 {
        first.update();
    }
    let again = open(store.clone());
    assert_eq!(again.theme().name, Theme::raven_light().name);
    assert_eq!(again.theme().radii.md, 3.0, "with what was edited");

    // Whatever got in there is no reason not to open.
    ravenrs_app::SettingsStore::set(&store, &ravenrs_app::SettingScope::Global, "theme", "nonsense").unwrap();
    let broken = open(store);
    assert_eq!(broken.theme().name, Theme::raven_dark().name);
}
