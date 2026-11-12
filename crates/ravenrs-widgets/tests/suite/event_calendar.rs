//! The event calendar and the recurrence editor, driven as a person drives them.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_core::{AccessAction, Event, Key, Modifiers, Point, PointerButton, Rect, Role, Size};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::{
    CalendarEvent, CalendarView, Date, EventCalendar, Frequency, Labels, Localized, RecurrenceEditor,
    RecurrenceEnd, RecurrenceRule, TimePoint, Tone, Ui, View,
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

#[test]
fn the_days_shown_are_whole_weeks_the_week_from_sunday_or_the_day() {
    // November 2026 starts on a Sunday and ends on a Monday.
    let anchor = date(2026, 11, 18);
    assert_eq!(CalendarView::Month.range(anchor), (date(2026, 11, 1), date(2026, 12, 5)));
    assert_eq!(CalendarView::Week.range(anchor), (date(2026, 11, 15), date(2026, 11, 21)));
    assert_eq!(CalendarView::Day.range(anchor), (anchor, anchor));
}

type Moves = Rc<std::cell::RefCell<Vec<(u64, TimePoint, TimePoint)>>>;

struct Hours {
    anchor: State<Date>,
    view: State<CalendarView>,
    created: Rc<Cell<Option<(TimePoint, TimePoint)>>>,
    moved: Moves,
}

/// A week of November 2026, from 8 to 18, with a meeting on Tuesday at 9 and two
/// things at once on Thursday at 14.
fn week() -> (Ui, Hours) {
    let h = Hours {
        anchor: State::new(date(2026, 11, 18)),
        view: State::new(CalendarView::Week),
        created: Rc::new(Cell::new(None)),
        moved: Rc::default(),
    };
    let (created, moved) = (h.created.clone(), h.moved.clone());
    let tuesday = date(2026, 11, 17);
    let thursday = date(2026, 11, 19);
    let view = EventCalendar::new(h.anchor.binding(), State::new(None).binding())
        .today(date(2026, 11, 18))
        .view(h.view.binding())
        .hours(8, 18)
        .events([
            CalendarEvent::timed(1, TimePoint::at(tuesday, 9, 0), TimePoint::at(tuesday, 10, 0), "Reunião"),
            CalendarEvent::timed(2, TimePoint::at(thursday, 14, 0), TimePoint::at(thursday, 15, 0), "Culto"),
            CalendarEvent::timed(
                3,
                TimePoint::at(thursday, 14, 30),
                TimePoint::at(thursday, 16, 0),
                "Ensaio",
            )
            .locked(true),
            CalendarEvent::new(4, date(2026, 11, 20), "Feriado"),
        ])
        .on_create(move |start, end| created.set(Some((start, end))))
        .on_move(move |id, start, end| moved.borrow_mut().push((id, start, end)));
    let mut ui = Ui::new(view, Theme::raven_light());
    ui.set_viewport(Size::new(1400.0, 1000.0), 1.0);
    for _ in 0..4 {
        ui.update();
    }
    (ui, h)
}

fn bounds(ui: &mut Ui, role: Role, name: &str) -> Rect {
    ui.access_tree()
        .nodes
        .iter()
        .find(|n| n.role == role && n.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no {role:?} '{name}'"))
        .bounds
}

#[test]
fn a_week_places_each_event_on_its_day_at_its_time() {
    let (mut ui, _) = week();
    let meeting = bounds(&mut ui, Role::Button, "Reunião");
    let nine = bounds(&mut ui, Role::Button, "New event 17 November 2026 09:00");
    let tuesday = bounds(&mut ui, Role::Group, "17 November 2026");
    assert!((meeting.y() - nine.y()).abs() < 1.5, "it starts on the line of 9: {meeting:?} {nine:?}");
    assert!((meeting.height() - nine.height()).abs() < 3.0, "and lasts an hour");
    assert!(
        meeting.x() >= tuesday.x() && meeting.x() + meeting.width() <= tuesday.x() + tuesday.width() + 0.5
    );
    let wednesday = bounds(&mut ui, Role::Group, "18 November 2026");
    assert!(wednesday.x() > tuesday.x(), "Sunday first, so Wednesday is to the right");
    let description = ui
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some("Reunião"))
        .and_then(|n| n.description.clone());
    assert_eq!(description.as_deref(), Some("09:00 – 10:00"));
}

#[test]
fn events_at_the_same_time_stand_side_by_side() {
    let (mut ui, _) = week();
    let (worship, rehearsal) =
        (bounds(&mut ui, Role::Button, "Culto"), bounds(&mut ui, Role::Button, "Ensaio"));
    let thursday = bounds(&mut ui, Role::Group, "19 November 2026");
    assert!(worship.x() + worship.width() <= rehearsal.x() + 0.5, "{worship:?} {rehearsal:?}");
    assert!(worship.width() < thursday.width() / 2.0 + 1.0);
    assert!(rehearsal.y() > worship.y(), "the second starts half an hour later");
}

#[test]
fn what_takes_the_whole_day_sits_above_the_hours() {
    let (mut ui, _) = week();
    let holiday = bounds(&mut ui, Role::Button, "Feriado");
    let eight = bounds(&mut ui, Role::Button, "New event 20 November 2026 08:00");
    assert!(holiday.y() + holiday.height() <= eight.y(), "{holiday:?} {eight:?}");
    assert!(
        says(&mut ui, "All day")
            || ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some("Feriado"))
    );
}

#[test]
fn an_empty_hour_creates_an_event_there() {
    let (mut ui, h) = week();
    press(&mut ui, Role::Button, "New event 18 November 2026 15:00");
    let (start, end) = h.created.get().expect("created");
    assert_eq!(start, TimePoint::at(date(2026, 11, 18), 15, 0));
    assert_eq!(end, TimePoint::at(date(2026, 11, 18), 16, 0));
}

fn drag(ui: &mut Ui, from: Point, to: Point) {
    ui.handle_event(&Event::PointerMove { position: from });
    ui.update();
    ui.handle_event(&Event::PointerDown { position: from, button: PointerButton::Primary });
    ui.update();
    ui.handle_event(&Event::PointerMove {
        position: Point::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0),
    });
    ui.update();
    ui.handle_event(&Event::PointerMove { position: to });
    ui.update();
    ui.handle_event(&Event::PointerUp { position: to, button: PointerButton::Primary });
    ui.update();
    ui.update();
}

#[test]
fn dragging_an_event_moves_it_by_days_and_quarter_hours() {
    let (mut ui, h) = week();
    let meeting = bounds(&mut ui, Role::Button, "Reunião");
    let (tuesday, wednesday) =
        (bounds(&mut ui, Role::Group, "17 November 2026"), bounds(&mut ui, Role::Group, "18 November 2026"));
    let from = meeting.center();
    // A day to the right and an hour and a half down.
    let to = Point::new(from.x + (wednesday.x() - tuesday.x()), from.y + 72.0);
    drag(&mut ui, from, to);
    let moved = h.moved.borrow().clone();
    let wednesday = date(2026, 11, 18);
    assert_eq!(moved, [(1, TimePoint::at(wednesday, 10, 30), TimePoint::at(wednesday, 11, 30))]);
}

#[test]
fn a_locked_event_stays_where_it_is() {
    let (mut ui, h) = week();
    let rehearsal = bounds(&mut ui, Role::Button, "Ensaio");
    let from = rehearsal.center();
    drag(&mut ui, from, Point::new(from.x, from.y + 96.0));
    assert!(h.moved.borrow().is_empty());
}

#[test]
fn the_view_switches_and_each_moves_by_its_own_step() {
    let (mut ui, h) = week();
    press(&mut ui, Role::Button, "Next");
    assert_eq!(h.anchor.get(), date(2026, 11, 25), "a week ahead");
    press(&mut ui, Role::RadioButton, "Day");
    assert_eq!(h.view.get(), CalendarView::Day);
    assert!(says(&mut ui, "New event 25 November 2026 08:00"));
    assert!(!says(&mut ui, "New event 26 November 2026 08:00"), "only the day");
    press(&mut ui, Role::Button, "Previous");
    assert_eq!(h.anchor.get(), date(2026, 11, 24));
    press(&mut ui, Role::RadioButton, "Month");
    assert!(says(&mut ui, "November 2026"));
    assert!(!says(&mut ui, "New event"), "a month has no hours");
}

#[test]
fn a_month_still_shows_timed_events_on_their_days_in_order() {
    let (mut ui, h) = week();
    h.view.set(CalendarView::Month);
    ui.update();
    ui.update();
    let day = node_description(&mut ui, "19 November 2026").expect("a description");
    assert_eq!(day, "Culto, Ensaio");
}
