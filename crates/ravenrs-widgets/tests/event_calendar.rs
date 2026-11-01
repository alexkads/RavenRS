//! The event calendar and the recurrence editor, driven as a person drives them.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_core::{AccessAction, Event, Key, Modifiers, Role, Size};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::{
    CalendarEvent, Date, EventCalendar, Frequency, Labels, Localized, RecurrenceEditor, RecurrenceEnd,
    RecurrenceRule, Tone, Ui, View,
};

fn date(y: i32, m: u8, d: u8) -> Date {
    Date::new(y, m, d).expect("valid date")
}

fn ui_of(view: impl View + 'static) -> Ui {
    let mut ui = Ui::new(view, Theme::raven_light());
    ui.set_viewport(Size::new(1100.0, 900.0), 1.0);
    ui.update();
    ui.update();
    ui
}

fn press(ui: &mut Ui, role: Role, name: &str) {
    let id = ui
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.role == role && n.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no {role:?} '{name}'"))
        .id;
    ui.perform_action(id, AccessAction::Click);
    ui.update();
    ui.update();
}

fn node_description(ui: &mut Ui, name: &str) -> Option<String> {
    ui.access_tree()
        .nodes
        .iter()
        .find(|n| n.role == Role::Button && n.name.as_deref() == Some(name))
        .and_then(|n| n.description.clone())
}

fn says(ui: &mut Ui, text: &str) -> bool {
    ui.access_tree().nodes.iter().any(|n| n.name.as_deref().is_some_and(|t| t.contains(text)))
}

struct Fixture {
    month: State<Date>,
    day: State<Option<Date>>,
    clicked: Rc<Cell<Option<u64>>>,
}

fn calendar() -> (Ui, Fixture) {
    let today = date(2026, 11, 15);
    let f = Fixture {
        month: State::new(today.first_of_month()),
        day: State::new(None),
        clicked: Rc::new(Cell::new(None)),
    };
    let clicked = f.clicked.clone();
    let view = EventCalendar::new(f.month.binding(), f.day.binding())
        .today(today)
        .events([
            CalendarEvent::new(1, date(2026, 11, 10), "Aluguel").tone(Tone::Warning),
            CalendarEvent::new(2, date(2026, 11, 20), "Salário").tone(Tone::Success).done(true),
            CalendarEvent::new(3, date(2026, 11, 20), "Luz"),
            CalendarEvent::new(4, date(2026, 11, 20), "Água"),
            CalendarEvent::new(5, date(2026, 11, 20), "Internet"),
        ])
        .on_event(move |id| clicked.set(Some(id)));
    (ui_of(view), f)
}

#[test]
fn a_day_reads_out_what_is_on_it() {
    let (mut ui, _) = calendar();
    let day = node_description(&mut ui, "20 November 2026").expect("a description");
    assert_eq!(day, "Salário, Luz, Água, Internet", "even what has no room on screen");
    assert_eq!(node_description(&mut ui, "15 November 2026").as_deref(), Some("Today"));
    assert_eq!(node_description(&mut ui, "16 November 2026"), None);
}

#[test]
fn a_full_day_shows_only_what_fits() {
    let (mut ui, _) = calendar();
    assert!(says(&mut ui, "Água"));
    assert!(!says(&mut ui, "Internet") || node_description(&mut ui, "Internet").is_none());
    let tree = ui.access_tree();
    let chips = tree.nodes.iter().filter(|n| n.name.as_deref() == Some("Internet")).count();
    assert_eq!(chips, 0, "the fourth event is counted, not drawn");
}

#[test]
fn clicking_an_event_picks_its_day_and_says_which() {
    let (mut ui, f) = calendar();
    press(&mut ui, Role::Button, "Aluguel");
    assert_eq!(f.clicked.get(), Some(1));
    assert_eq!(f.day.get(), Some(date(2026, 11, 10)));
}

#[test]
fn clicking_a_day_picks_it() {
    let (mut ui, f) = calendar();
    press(&mut ui, Role::Button, "3 November 2026");
    assert_eq!(f.day.get(), Some(date(2026, 11, 3)));
    assert_eq!(f.clicked.get(), None);
}

#[test]
fn the_month_moves_and_today_comes_back() {
    let (mut ui, f) = calendar();
    press(&mut ui, Role::Button, "Next month");
    assert_eq!(f.month.get(), date(2026, 12, 1));
    assert!(!says(&mut ui, "Aluguel"), "another month's events are not shown");
    press(&mut ui, Role::Button, "Today");
    assert_eq!(f.month.get(), date(2026, 11, 1));
    assert_eq!(f.day.get(), Some(date(2026, 11, 15)));
}

#[test]
fn only_the_weeks_of_the_month_are_shown() {
    let today = date(2026, 2, 1);
    let (month, day) = (State::new(today), State::new(None));
    // February 2026 starts on a Sunday and fills exactly four weeks.
    let mut ui = ui_of(EventCalendar::new(month.binding(), day.binding()));
    assert!(says(&mut ui, "28 February 2026"));
    assert!(!says(&mut ui, "1 March 2026"), "no empty week at the end");
}

#[test]
fn the_keyboard_moves_between_days_and_months() {
    let (mut ui, f) = calendar();
    let id = ui
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some("15 November 2026"))
        .expect("today")
        .id;
    ui.perform_action(id, AccessAction::Focus);
    ui.update();
    let key = |ui: &mut Ui, key: Key| {
        ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers::default() });
        ui.update();
        ui.update();
    };
    key(&mut ui, Key::ArrowDown);
    key(&mut ui, Key::Enter);
    assert_eq!(f.day.get(), Some(date(2026, 11, 22)), "a week down");
    key(&mut ui, Key::PageDown);
    assert_eq!(f.month.get(), date(2026, 12, 1), "a month ahead");
}

fn editor(rule: RecurrenceRule) -> (Ui, State<RecurrenceRule>) {
    let state = State::new(rule);
    let ui = ui_of(RecurrenceEditor::new(state.binding(), date(2026, 11, 5)));
    (ui, state)
}

#[test]
fn the_editor_changes_how_often() {
    let (mut ui, state) = editor(RecurrenceRule::default());
    press(&mut ui, Role::RadioButton, "Weekly");
    assert_eq!(state.get().frequency, Frequency::Weekly);
    press(&mut ui, Role::RadioButton, "Once");
    assert_eq!(state.get().frequency, Frequency::Once);
    assert!(!says(&mut ui, "Ends"), "a single day has no end to choose");
}

#[test]
fn the_editor_chooses_weekdays_keeping_the_first_days_own() {
    // November 5, 2026 is a Thursday.
    let (mut ui, state) = editor(RecurrenceRule::new(Frequency::Weekly));
    press(&mut ui, Role::Button, "Mo");
    let chosen = state.get().weekdays;
    assert!(chosen[1], "Monday was added");
    assert!(chosen[4], "and Thursday, which was showing on, stayed");
    press(&mut ui, Role::Button, "Th");
    assert!(!state.get().weekdays[4], "it can be turned off");
}

#[test]
fn the_editor_sets_the_end() {
    let (mut ui, state) = editor(RecurrenceRule::default());
    press(&mut ui, Role::RadioButton, "After");
    assert_eq!(state.get().end, RecurrenceEnd::After(12));
    press(&mut ui, Role::RadioButton, "On a date");
    assert_eq!(state.get().end, RecurrenceEnd::Until(date(2027, 11, 5)));
    press(&mut ui, Role::RadioButton, "Never");
    assert_eq!(state.get().end, RecurrenceEnd::Never);
}

#[test]
fn the_editor_says_what_it_chose_in_words() {
    let start = date(2026, 11, 5);
    let pt = Labels::pt_br();
    let rule = RecurrenceRule::new(Frequency::Monthly).end(RecurrenceEnd::After(2));
    assert_eq!(pt.recurrence_summary(&rule, start), "Todo mês, 2 vezes. Próximas: 05/11/2026, 05/12/2026");
    let rule = RecurrenceRule::new(Frequency::Weekly).every(2);
    assert_eq!(
        pt.recurrence_summary(&rule, start),
        "A cada 2 semanas. Próximas: 05/11/2026, 19/11/2026, 03/12/2026"
    );
    let rule = RecurrenceRule::new(Frequency::Once);
    assert_eq!(pt.recurrence_summary(&rule, start), "Uma vez. Próximas: 05/11/2026");
}

#[test]
fn the_editor_speaks_the_language_around_it() {
    let state = State::new(RecurrenceRule::default());
    let mut ui =
        ui_of(Localized::new(Labels::pt_br(), RecurrenceEditor::new(state.binding(), date(2026, 11, 5))));
    assert!(says(&mut ui, "Mensal"));
    assert!(says(&mut ui, "Todo mês"));
}
