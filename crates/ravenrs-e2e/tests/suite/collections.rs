//! The collections of lists, scheduler, dock and charts in the testability contract (#740):
//! each item is found by its key or its date, which stay with it when the collection changes.

use ravenrs_core::Size;
use ravenrs_e2e::E2e;
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::{
    CalendarEvent, Date, DocumentTab, DocumentTabs, EventCalendar, SortableList, Text, Timeline,
    TimelineItem, TimelineTrack, VStack, View, YearView,
};

fn session(view: impl View + 'static) -> E2e {
    E2e::new(view, Theme::raven_dark()).viewport(Size::new(1200.0, 900.0), 1.0)
}

#[test]
fn a_document_tab_keeps_its_id_when_the_tabs_are_reordered() {
    let active = State::new(1_u64);
    let tabs = State::new(vec![DocumentTab::new(1, "main.rs"), DocumentTab::new(2, "lib.rs")]);
    let (a, t) = (active.clone(), tabs.clone());
    let mut e2e = session(ravenrs_widgets::view_fn(move || DocumentTabs::new(t.get(), a.binding())));
    e2e.by_test_id("tab:2").click().unwrap();
    assert_eq!(active.get(), 2);
    tabs.set(vec![DocumentTab::new(2, "lib.rs"), DocumentTab::new(1, "main.rs")]);
    e2e.advance(0.1);
    assert_eq!(e2e.by_test_id("tab:1").node().unwrap().name.as_deref(), Some("main.rs"));
}

#[test]
fn a_sortable_item_is_found_by_its_key() {
    let names = ["Ana", "Bia", "Caio"];
    let list = SortableList::new(3, move |i| Text::new(names[i])).key(|i| 100 + i as u64);
    let mut e2e = session(VStack::new((list,)));
    assert_eq!(e2e.by_test_id("item:102").count().unwrap(), 1);
}

#[test]
fn a_day_of_the_year_and_an_event_of_the_calendar_are_found_by_date_and_id() {
    let year = State::new(2026);
    let mut e2e = session(VStack::new((YearView::new(year.binding()),)));
    assert_eq!(e2e.by_test_id("day:2026-11-22").count().unwrap(), 1);

    let (month, day) = (State::new(Date::new(2026, 11, 1).unwrap()), State::new(None));
    let calendar = EventCalendar::new(month.binding(), day.binding()).events([CalendarEvent::new(
        7,
        Date::new(2026, 11, 10).unwrap(),
        "Reunião",
    )]);
    let mut e2e = session(VStack::new((calendar,)));
    assert_eq!(e2e.by_test_id("event:7").node().unwrap().name.as_deref(), Some("Reunião"));
    assert_eq!(e2e.by_test_id("day:2026-11-10").count().unwrap(), 1);
}

#[test]
fn an_item_of_a_timeline_is_found_and_chosen_by_its_id() {
    let chosen = State::new(None);
    let c = chosen.clone();
    let track = TimelineTrack::new("Vídeo", 40.0, vec![TimelineItem::new(42, 0, 2000, "Abertura")]);
    let timeline = Timeline::new(vec![track]).on_select(move |id| c.set(Some(id)));
    let mut e2e = session(VStack::new((timeline,)));
    e2e.by_test_id("item:42").click().unwrap();
    assert_eq!(chosen.get(), Some(42));
}
