//! The resource timeline, driven as a person drives it.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_core::{AccessAction, Event, Key, Modifiers, Point, PointerButton, Rect, Role, Size};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::{
    Date, Resource, Scheduler, SchedulerChange, SchedulerEvent, TimeAxis, TimePoint, TimeScale, Ui,
};

fn date(d: u8) -> Date {
    Date::new(2026, 11, d).expect("valid date")
}

fn day(d: u8) -> TimePoint {
    TimePoint::from_date(date(d))
}

struct Fixture {
    ui: Ui,
    axis: State<TimeAxis>,
    changes: Rc<RefCell<Vec<SchedulerChange>>>,
    created: Rc<RefCell<Vec<SchedulerChange>>>,
    opened: Rc<RefCell<Vec<u64>>>,
    deleted: Rc<RefCell<Vec<u64>>>,
}

fn fixture(events: Vec<SchedulerEvent>, can_drop: bool) -> Fixture {
    let axis = State::new(TimeAxis::days(date(1), 30));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let created = Rc::new(RefCell::new(Vec::new()));
    let opened = Rc::new(RefCell::new(Vec::new()));
    let deleted = Rc::new(RefCell::new(Vec::new()));
    let (c, n, o, d) = (changes.clone(), created.clone(), opened.clone(), deleted.clone());
    let mut scheduler = Scheduler::new(axis.binding())
        .resources([Resource::new(1, "Ana"), Resource::new(2, "Bruno"), Resource::new(3, "Lia")])
        .events(events)
        .on_change(move |change| c.borrow_mut().push(change))
        .on_create(move |change| n.borrow_mut().push(change))
        .on_open(move |id| o.borrow_mut().push(id))
        .on_delete(move |id| d.borrow_mut().push(id));
    if can_drop {
        scheduler = scheduler.can_drop(|change| change.resource != 3);
    }
    let mut ui = Ui::new(scheduler, Theme::raven_light());
    ui.set_viewport(Size::new(1400.0, 700.0), 1.0);
    for _ in 0..3 {
        ui.update();
    }
    Fixture { ui, axis, changes, created, opened, deleted }
}

fn trip(id: u64, resource: u64, from: u8, to: u8) -> SchedulerEvent {
    SchedulerEvent::new(id, resource, day(from), day(to), format!("Evento {id}"))
}

fn bounds(ui: &mut Ui, role: Role, name: &str) -> Rect {
    ui.access_tree()
        .nodes
        .iter()
        .find(|n| n.role == role && n.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no {role:?} {name}"))
        .bounds
}

fn drag(ui: &mut Ui, from: Point, to: Point) {
    ui.handle_event(&Event::PointerMove { position: from });
    ui.update();
    ui.handle_event(&Event::PointerDown { position: from, button: PointerButton::Primary });
    ui.update();
    let halfway = Point::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
    ui.handle_event(&Event::PointerMove { position: halfway });
    ui.update();
    ui.handle_event(&Event::PointerMove { position: to });
    ui.update();
    ui.handle_event(&Event::PointerUp { position: to, button: PointerButton::Primary });
    ui.update();
    ui.update();
}

fn key(ui: &mut Ui, key: Key, shift: bool) {
    let modifiers = Modifiers { shift, ..Modifiers::default() };
    ui.handle_event(&Event::KeyDown { key, modifiers });
    ui.update();
    ui.update();
}

fn focus(ui: &mut Ui, name: &str) {
    let id = ui
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.role == Role::Button && n.name.as_deref() == Some(name))
        .expect("the event")
        .id;
    ui.perform_action(id, AccessAction::Focus);
    ui.update();
}

#[test]
fn a_bar_sits_where_its_time_is_on_its_row() {
    let mut f = fixture(vec![trip(1, 2, 3, 6)], false);
    let bar = bounds(&mut f.ui, Role::Button, "Evento 1");
    let (ana, bruno) = (bounds(&mut f.ui, Role::Row, "Ana"), bounds(&mut f.ui, Role::Row, "Bruno"));
    assert_eq!(bar.width(), 3.0 * 40.0, "three days at 40 px");
    assert!(bar.y() >= bruno.y() && bar.max_y() <= bruno.max_y(), "on its row: {bar:?} {bruno:?}");
    assert!(bar.y() >= ana.max_y(), "and not on another");
    let ana_right = ana.max_x();
    assert_eq!(bar.x() - ana_right, 2.0 * 40.0, "two days after the axis starts");
}

#[test]
fn dragging_moves_an_event_along_the_time_and_between_rows() {
    let mut f = fixture(vec![trip(1, 1, 3, 6)], false);
    let bar = bounds(&mut f.ui, Role::Button, "Evento 1");
    let row = bounds(&mut f.ui, Role::Row, "Bruno");
    let from = bar.center();
    drag(&mut f.ui, from, Point::new(from.x + 80.0, row.center().y));
    let changes = f.changes.borrow();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0], SchedulerChange { id: Some(1), resource: 2, start: day(5), end: day(8) });
}

#[test]
fn a_small_drag_snaps_back_and_changes_nothing() {
    let mut f = fixture(vec![trip(1, 1, 3, 6)], false);
    let from = bounds(&mut f.ui, Role::Button, "Evento 1").center();
    drag(&mut f.ui, from, Point::new(from.x + 5.0, from.y));
    assert!(f.changes.borrow().is_empty());
}

#[test]
fn the_ends_resize_an_event() {
    let mut f = fixture(vec![trip(1, 1, 3, 6)], false);
    let bar = bounds(&mut f.ui, Role::Button, "Evento 1");
    let end = Point::new(bar.max_x() - 2.0, bar.center().y);
    drag(&mut f.ui, end, Point::new(end.x + 80.0, end.y));
    assert_eq!(
        f.changes.borrow()[0],
        SchedulerChange { id: Some(1), resource: 1, start: day(3), end: day(8) }
    );

    let mut f = fixture(vec![trip(1, 1, 3, 6)], false);
    let bar = bounds(&mut f.ui, Role::Button, "Evento 1");
    let start = Point::new(bar.x() + 2.0, bar.center().y);
    drag(&mut f.ui, start, Point::new(start.x + 40.0, start.y));
    assert_eq!(
        f.changes.borrow()[0],
        SchedulerChange { id: Some(1), resource: 1, start: day(4), end: day(6) }
    );
}

#[test]
fn an_event_cannot_be_resized_to_nothing() {
    let mut f = fixture(vec![trip(1, 1, 3, 6)], false);
    let bar = bounds(&mut f.ui, Role::Button, "Evento 1");
    let end = Point::new(bar.max_x() - 2.0, bar.center().y);
    drag(&mut f.ui, end, Point::new(end.x - 500.0, end.y));
    assert_eq!(f.changes.borrow()[0].end, day(4), "at least one step long");
}

#[test]
fn a_refused_place_changes_nothing() {
    let mut f = fixture(vec![trip(1, 1, 3, 6)], true);
    let from = bounds(&mut f.ui, Role::Button, "Evento 1").center();
    let lia = bounds(&mut f.ui, Role::Row, "Lia");
    drag(&mut f.ui, from, Point::new(from.x, lia.center().y));
    assert!(f.changes.borrow().is_empty(), "Lia's row is not allowed");
}

#[test]
fn an_event_that_cannot_resize_only_moves() {
    let mut f = fixture(vec![trip(1, 1, 3, 4).resizable(false)], false);
    focus(&mut f.ui, "Evento 1");
    key(&mut f.ui, Key::ArrowRight, true);
    assert!(f.changes.borrow().is_empty(), "Shift+Right does nothing");
    let bar = bounds(&mut f.ui, Role::Button, "Evento 1");
    let end = Point::new(bar.max_x() - 2.0, bar.center().y);
    drag(&mut f.ui, end, Point::new(end.x + 80.0, end.y));
    let change = f.changes.borrow()[0];
    assert_eq!((change.start, change.end), (day(5), day(6)), "its end drags it along, the same length");
}

#[test]
fn a_locked_event_stays_put() {
    let mut f = fixture(vec![trip(1, 1, 3, 6).locked(true)], false);
    let from = bounds(&mut f.ui, Role::Button, "Evento 1").center();
    drag(&mut f.ui, from, Point::new(from.x + 120.0, from.y));
    assert!(f.changes.borrow().is_empty());
    focus(&mut f.ui, "Evento 1");
    key(&mut f.ui, Key::ArrowRight, false);
    assert!(f.changes.borrow().is_empty(), "nor from the keyboard");
}

#[test]
fn dragging_on_an_empty_stretch_draws_a_new_event() {
    let mut f = fixture(vec![], false);
    let row = bounds(&mut f.ui, Role::Row, "Bruno");
    let from = Point::new(row.max_x() + 80.0, row.center().y);
    drag(&mut f.ui, from, Point::new(from.x + 120.0, from.y));
    assert_eq!(f.created.borrow()[0], SchedulerChange { id: None, resource: 2, start: day(3), end: day(6) });
}

#[test]
fn overlapping_events_stack_and_the_row_grows() {
    let mut alone = fixture(vec![trip(1, 1, 3, 6)], false);
    let single = bounds(&mut alone.ui, Role::Row, "Ana").height();
    let mut f = fixture(vec![trip(1, 1, 3, 6), trip(2, 1, 4, 7)], false);
    let double = bounds(&mut f.ui, Role::Row, "Ana").height();
    assert!(double > single, "{double} > {single}");
    let (a, b) = (bounds(&mut f.ui, Role::Button, "Evento 1"), bounds(&mut f.ui, Role::Button, "Evento 2"));
    assert!(a.max_y() <= b.y() || b.max_y() <= a.y(), "they do not overlap: {a:?} {b:?}");
}

#[test]
fn the_keyboard_moves_resizes_opens_and_deletes() {
    let mut f = fixture(vec![trip(1, 1, 3, 6)], false);
    focus(&mut f.ui, "Evento 1");
    key(&mut f.ui, Key::ArrowRight, false);
    assert_eq!(
        f.changes.borrow()[0],
        SchedulerChange { id: Some(1), resource: 1, start: day(4), end: day(7) }
    );
    key(&mut f.ui, Key::ArrowRight, true);
    assert_eq!(
        f.changes.borrow()[1],
        SchedulerChange { id: Some(1), resource: 1, start: day(3), end: day(7) }
    );
    key(&mut f.ui, Key::ArrowDown, false);
    assert_eq!(f.changes.borrow()[2].resource, 2);
    key(&mut f.ui, Key::Enter, false);
    assert_eq!(*f.opened.borrow(), [1]);
    key(&mut f.ui, Key::Delete, false);
    assert_eq!(*f.deleted.borrow(), [1]);
}

#[test]
fn zoom_and_moving_through_time_change_the_axis() {
    let mut f = fixture(vec![], false);
    let press = |ui: &mut Ui, name: &str| {
        let id = ui
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.role == Role::Button && n.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("no {name}"))
            .id;
        ui.perform_action(id, AccessAction::Click);
        ui.update();
        ui.update();
    };
    let wide = f.axis.get().width();
    press(&mut f.ui, "Zoom in");
    assert!(f.axis.get().width() > wide);
    press(&mut f.ui, "Zoom out");
    press(&mut f.ui, "Zoom out");
    assert_eq!(f.axis.get().scale, TimeScale::Weeks);
    let start = f.axis.get().start;
    press(&mut f.ui, "Next month");
    assert!(f.axis.get().start > start);
}

#[test]
fn it_opens_with_now_in_view() {
    let axis = State::new(TimeAxis::days(date(1), 365));
    let now = day(1).plus_days(200);
    let start = now.plus_days(1);
    let scheduler = Scheduler::new(axis.binding())
        .resources([Resource::new(1, "Ana")])
        .events([SchedulerEvent::new(1, 1, start, start.plus_days(1), "Perto de hoje")])
        .now(now);
    let mut ui = Ui::new(scheduler, Theme::raven_light());
    ui.set_viewport(Size::new(1000.0, 500.0), 1.0);
    for _ in 0..4 {
        ui.update();
    }
    let bar = bounds(&mut ui, Role::Button, "Perto de hoje");
    assert!(bar.x() > 200.0 && bar.max_x() < 1000.0, "in the window: {bar:?}");
    let group =
        ui.access_tree().nodes.iter().find(|n| n.name.as_deref() == Some("May 2027")).map(|n| n.bounds);
    let group = group.expect("the month in view is named");
    assert!(group.x() >= 200.0 && group.x() < 1000.0, "its name stays in view after scrolling: {group:?}");
}

#[test]
fn groups_collapse() {
    let axis = State::new(TimeAxis::days(date(1), 30));
    let scheduler = Scheduler::new(axis.binding()).resources([
        Resource::new(1, "Ana").group("Casa"),
        Resource::new(2, "Bruno").group("Casa"),
        Resource::new(3, "Lia"),
    ]);
    let mut ui = Ui::new(scheduler, Theme::raven_light());
    ui.set_viewport(Size::new(1400.0, 700.0), 1.0);
    ui.update();
    ui.update();
    assert!(ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some("Ana")));
    let id = ui.access_tree().nodes.iter().find(|n| n.name.as_deref() == Some("Casa")).expect("the group").id;
    ui.perform_action(id, AccessAction::Click);
    ui.update();
    ui.update();
    let names: Vec<_> = ui.access_tree().nodes.iter().filter_map(|n| n.name.clone()).collect();
    assert!(!names.iter().any(|n| n == "Ana"), "{names:?}");
    assert!(names.iter().any(|n| n == "Lia"), "other rows stay");
}

#[test]
fn only_what_is_in_view_gets_built() {
    let resources: Vec<Resource> = (0..5000).map(|i| Resource::new(i, format!("Pessoa {i}"))).collect();
    let events: Vec<SchedulerEvent> =
        (0..5000).map(|i| SchedulerEvent::new(i, i, day(3), day(6), format!("Evento {i}"))).collect();
    let axis = State::new(TimeAxis::days(date(1), 30));
    let mut ui =
        Ui::new(Scheduler::new(axis.binding()).resources(resources).events(events), Theme::raven_light());
    ui.set_viewport(Size::new(1400.0, 700.0), 1.0);
    for _ in 0..3 {
        ui.update();
    }
    let tree = ui.access_tree();
    let bars =
        tree.nodes.iter().filter(|n| n.name.as_deref().is_some_and(|t| t.starts_with("Evento "))).count();
    assert!(bars > 5 && bars < 60, "{bars} bars built out of 5000");
}
