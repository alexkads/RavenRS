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

#[test]
fn the_driver_moves_an_item_before_another_by_their_keys() {
    use ravenrs_widgets::testing::SortableDriver;
    let order = State::new(vec![1_u64, 2, 3]);
    let o = order.clone();
    let view = ravenrs_widgets::view_fn(move || {
        let (shown, moved) = (o.get(), o.clone());
        let names = shown.clone();
        SortableList::new(shown.len(), move |i| Text::new(format!("Item {}", names[i])))
            .key({
                let shown = shown.clone();
                move |i| shown[i]
            })
            .on_reorder(move |from, to| {
                moved.update(|o| {
                    let item = o.remove(from);
                    o.insert(to, item);
                })
            })
    });
    let mut e2e = session(view);
    SortableDriver::new(&mut e2e).move_before(2, 1).unwrap();
    assert_eq!(order.get(), vec![2, 1, 3]);
}

#[test]
fn a_bar_of_a_chart_is_found_with_its_value_and_a_command_of_the_ribbon_by_its_label() {
    use ravenrs_widgets::{BarChart, Ribbon, RibbonGroup, RibbonItem, RibbonPage};
    let mut e2e = session(VStack::new((BarChart::new([("Jan", 120.0), ("Fev", 80.0)]),)));
    let fev = e2e.by_test_id("point:1:Fev").node().unwrap();
    assert!(fev.name.as_deref().is_some_and(|n| n.contains("Fev") && n.contains("80")), "{fev:?}");

    let saved = State::new(false);
    let s = saved.clone();
    let ribbon =
        Ribbon::new().page(RibbonPage::new("Início").group(RibbonGroup::new("Arquivo").item(
            RibbonItem::button("Salvar", ravenrs_widgets::IconName::Save).on_click(move || s.set(true)),
        )));
    let mut e2e = session(VStack::new((ribbon,)));
    e2e.by_test_id("command:Salvar").click().unwrap();
    assert!(saved.get());
}

#[test]
fn a_row_far_down_a_list_of_ten_thousand_is_scrolled_to_and_clicked() {
    use ravenrs_widgets::{Button, VirtualList};
    let clicked = State::new(None);
    let c = clicked.clone();
    let list = VirtualList::new(10_000, move |i| {
        let c = c.clone();
        Button::new(format!("Linha {i}")).on_click(move || c.set(Some(i)))
    })
    .key(|i| i as u64)
    .height(400.0);
    let mut e2e = session(VStack::new((list,)));
    e2e.by_test_id("row:9999").scroll_into_view().unwrap();
    // The button inside the row: the middle of the row is past the end of its text.
    e2e.by_role(ravenrs_core::Role::Button).within(ravenrs_e2e::Query::test_id("row:9999")).click().unwrap();
    assert_eq!(clicked.get(), Some(9999));
}

#[test]
fn a_disabled_part_of_a_collection_refuses_the_action() {
    use ravenrs_e2e::ErrorCode;
    use ravenrs_widgets::{Ribbon, RibbonGroup, RibbonItem, RibbonPage};
    let ribbon = Ribbon::new().page(
        RibbonPage::new("Início").group(
            RibbonGroup::new("Arquivo")
                .item(RibbonItem::button("Colar", ravenrs_widgets::IconName::Clipboard).enabled(false)),
        ),
    );
    let mut e2e = session(VStack::new((ribbon,)));
    assert_eq!(e2e.by_test_id("command:Colar").click().unwrap_err().code(), ErrorCode::Disabled);
}
