//! What an application at a counter leans on: notices nobody can dismiss, work in the background,
//! a single record, keys in sight and a locked window, with no window of its own.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_app::*;
use ravenrs_app_ui::*;
use ravenrs_core::{Event, Key, Modifiers, Size};
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

struct Keys(Rc<Cell<u32>>);

struct KeyActions(Rc<Cell<u32>>);

impl ViewController for KeyActions {
    fn id(&self) -> &str {
        "KeyActions"
    }

    fn actions(&self) -> Vec<Action> {
        let count = self.0.clone();
        let alt = self.0.clone();
        vec![
            Action::simple("Pay", "Pay").hotkey("f9").on_execute(move |_| {
                count.set(count.get() + 1);
                Ok(())
            }),
            Action::simple("Customer", "Customer").hotkey("alt+c").on_execute(move |_| {
                alt.set(alt.get() + 100);
                Ok(())
            }),
        ]
    }
}

impl Module for Keys {
    fn name(&self) -> &str {
        "Keys"
    }

    fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
        vec![Box::new(KeyActions(self.0.clone()))]
    }

    fn notices(&self) -> Vec<Notice> {
        vec![Notice::warning("BETA", "Experimental, for tests only.")]
    }
}

fn entities() -> Vec<EntityInfo> {
    vec![
        EntityInfo::new("Settings")
            .caption("Settings")
            .navigation("Records")
            .singleton()
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("store", FieldKind::Text).caption("Store").default_property()),
    ]
}

struct World {
    ui: Ui,
    session: Session,
    database: MemoryDatabase,
}

fn world(count: Rc<Cell<u32>>, template: impl FrameTemplate + 'static) -> World {
    let database = MemoryDatabase::new(entities()).expect("valid");
    let application = entities()
        .into_iter()
        .fold(
            Application::builder("Test").module(SystemModule::new()).module(Keys(count)),
            ApplicationBuilder::entity,
        )
        .build()
        .expect("valid");
    let source = database.clone();
    let shell = AppShell::new(application, move || source.space()).template(template);
    let session = shell.session();
    let mut ui = Ui::new(shell, Theme::raven_light());
    ui.set_viewport(Size::new(1200.0, 800.0), 1.0);
    ui.update();
    ui.update();
    World { ui, session, database }
}

impl World {
    fn settle(&mut self) {
        self.ui.update();
        self.ui.update();
    }

    fn says(&mut self, text: &str) -> bool {
        self.ui
            .access_tree()
            .nodes
            .iter()
            .any(|n| n.name.as_deref().is_some_and(|name| name.contains(text)) || n.value.as_deref() == Some(text))
    }

    fn key(&mut self, key: Key, modifiers: Modifiers) {
        self.ui.handle_event(&Event::KeyDown { key, modifiers });
        self.settle();
    }
}

#[test]
fn a_notice_is_on_every_window_whatever_the_template() {
    struct Bare;
    impl FrameTemplate for Bare {
        fn arrange(&self, parts: TemplateParts) -> ravenrs_widgets::AnyView {
            parts.content
        }
    }
    let mut w = world(Rc::default(), Bare);
    assert!(w.says("BETA"), "a template cannot leave it out");
    assert_eq!(w.session.notices().len(), 1);
}

#[test]
fn an_entity_with_one_record_opens_on_its_form_with_no_list() {
    let mut w = world(Rc::default(), MainTemplate);
    assert!(w.session.form_record().is_some(), "goes to a new record while there is none");
    let mut space = w.database.space();
    let mut record = space.new_object("Settings").unwrap();
    record.set("store", "Loja".to_owned());
    space.save(record).unwrap();
    space.commit().unwrap();
    w.session.open_view("Settings_ListView");
    w.settle();
    let record = w.session.form_record().expect("its form");
    assert_eq!(record.get("store"), Value::Text("Loja".into()));
}

#[test]
fn work_in_the_background_comes_back_on_the_screens_thread() {
    let w = world(Rc::default(), MainTemplate);
    let got = Rc::new(Cell::new(0));
    let seen = got.clone();
    let main = std::thread::current().id();
    w.session.run_background(
        "Sum",
        move |progress| {
            assert_ne!(std::thread::current().id(), main, "out of the way of the screens");
            progress.report(0.5, "half");
            21 * 2
        },
        move |_session, sum| seen.set(sum),
    );
    w.session.finish_background();
    assert_eq!(got.get(), 42);
    assert!(w.session.background_jobs().is_empty());
}

#[test]
fn work_in_the_background_hears_it_was_canceled() {
    let w = world(Rc::default(), MainTemplate);
    let got = Rc::new(Cell::new(false));
    let seen = got.clone();
    let id = w.session.run_background(
        "Forever",
        |progress| {
            while !progress.is_canceled() {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            true
        },
        move |_session, stopped| seen.set(stopped),
    );
    w.session.cancel_background(id);
    w.session.finish_background();
    assert!(got.get());
}

#[test]
fn the_counter_shows_the_keys_in_a_bar_and_they_work() {
    let count = Rc::new(Cell::new(0));
    let mut w = world(count.clone(), CounterTemplate);
    assert!(w.says("Pay"), "the bar shows what the key does");
    assert!(w.says("F9"));
    w.key(Key::Function(9), Modifiers::default());
    assert_eq!(count.get(), 1);
    w.key(Key::Function(8), Modifiers::default());
    assert_eq!(count.get(), 1, "another key does nothing");
}

#[test]
fn alt_and_a_letter_is_the_same_key_when_the_system_composes_an_accent() {
    let count = Rc::new(Cell::new(0));
    let mut w = world(count.clone(), CounterTemplate);
    let alt = Modifiers { alt: true, base_key: Some('c'), ..Modifiers::default() };
    w.key(Key::Character("ç".into()), alt);
    assert_eq!(count.get(), 100);
    w.key(Key::Character("c".into()), Modifiers { alt: true, ..Modifiers::default() });
    assert_eq!(count.get(), 200);
    w.key(Key::Character("c".into()), Modifiers::default());
    assert_eq!(count.get(), 200, "without Alt it is only a letter");
}

#[test]
fn a_locked_window_hides_everything_until_it_passes() {
    let count = Rc::new(Cell::new(0));
    let mut w = world(count.clone(), CounterTemplate);
    w.session.lock("Caixa bloqueado", |typed| typed == "1234");
    w.settle();
    assert!(w.says("Caixa bloqueado"));
    assert!(!w.says("Pay"), "nothing else shows");
    w.key(Key::Function(9), Modifiers::default());
    assert_eq!(count.get(), 0, "its keys do nothing");
    for letter in "0000".chars() {
        w.key(Key::Character(letter.to_string()), Modifiers::default());
    }
    w.key(Key::Enter, Modifiers::default());
    assert!(w.says("Caixa bloqueado"), "a wrong one stays locked");
    for letter in "1234".chars() {
        w.key(Key::Character(letter.to_string()), Modifiers::default());
    }
    w.key(Key::Enter, Modifiers::default());
    assert!(w.says("Pay"), "the right one gets back");
}

#[test]
fn the_keys_of_what_can_be_done_can_be_listed() {
    let mut w = world(Rc::default(), CounterTemplate);
    w.session.show_shortcuts();
    w.settle();
    assert!(w.says("Keyboard shortcuts"));
}
