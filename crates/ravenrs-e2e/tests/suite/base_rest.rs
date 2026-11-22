//! Navigation and choice controls of `ravenrs-widgets-base` in the testability contract (#737,
//! third lot): their items are found by label or by number, wherever they are in the row.

use ravenrs_core::{AccessAction, Size};
use ravenrs_e2e::E2e;
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::{
    Calendar, Date, FavoriteToggle, Pagination, SegmentedControl, ToggleGroup, VStack, View, ViewExt,
};

fn session(view: impl View + 'static) -> E2e {
    E2e::new(view, Theme::raven_dark()).viewport(Size::new(1000.0, 760.0), 1.0)
}

#[test]
fn a_segment_is_chosen_by_its_label() {
    let chosen = State::new(0_usize);
    let mut e2e = session(VStack::new((SegmentedControl::new(chosen.binding(), ["Dia", "Semana", "Mês"]),)));
    e2e.by_test_id("option:Mês").perform(AccessAction::Select).unwrap();
    assert_eq!(chosen.get(), 2);
    e2e.by_test_id("option:Semana").click().unwrap();
    assert_eq!(chosen.get(), 1);
}

#[test]
fn a_page_is_opened_by_its_number() {
    let page = State::new(0_usize);
    let mut e2e = session(VStack::new((Pagination::new(page.binding(), 5),)));
    e2e.by_test_id("page:3").click().unwrap();
    assert_eq!(page.get(), 2, "page 3 is the third, counted from zero");
    assert_eq!(e2e.by_test_id("page:3").node().unwrap().selected, Some(true));
}

#[test]
fn an_item_of_a_toggle_group_is_found_by_its_label() {
    let align = State::new(0_usize);
    let group =
        ToggleGroup::single(align.binding(), [(None, "Esquerda"), (None, "Centro"), (None, "Direita")]);
    let mut e2e = session(VStack::new((group,)));
    e2e.by_test_id("option:Direita").click().unwrap();
    assert_eq!(align.get(), 2);
}

#[test]
fn a_day_of_a_calendar_is_chosen_by_its_date_and_a_toggle_by_a_click() {
    let (day, liked) = (State::new(None), State::new(false));
    let month = Date::new(2026, 11, 1).unwrap();
    let view = VStack::new((
        Calendar::new(day.binding(), month),
        FavoriteToggle::new(liked.binding()).test_id("favorito"),
    ));
    let mut e2e = session(view);
    e2e.by_test_id("day:2026-11-15").perform(AccessAction::Select).unwrap();
    assert_eq!(day.get(), Date::new(2026, 11, 15));
    e2e.by_test_id("favorito").click().unwrap();
    assert!(liked.get());
}
