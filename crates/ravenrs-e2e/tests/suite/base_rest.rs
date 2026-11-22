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

#[test]
fn a_stepper_and_a_color_picker_take_their_value() {
    use ravenrs_core::Color;
    use ravenrs_e2e::ErrorCode;
    use ravenrs_widgets::{ColorPicker, Stepper};
    let (count, color) = (State::new(1_i32), State::new(None));
    let view = VStack::new((
        Stepper::new(count.binding()).range(0, 10).test_id("quantidade"),
        ColorPicker::new(color.binding()).test_id("cor"),
    ));
    let mut e2e = session(view);
    e2e.by_test_id("quantidade").set_value("7").unwrap();
    assert_eq!(count.get(), 7);
    e2e.by_test_id("quantidade").set_value("99").unwrap();
    assert_eq!(count.get(), 10, "kept inside its range");
    e2e.by_test_id("cor").set_value("#FF0000").unwrap();
    assert_eq!(color.get(), Some(Color::hex(0xFF0000)));
    assert_eq!(e2e.by_test_id("cor").set_value("vermelho").unwrap_err().code(), ErrorCode::Expectation);
    e2e.by_test_id("cor").expand().unwrap();
    e2e.by_test_id("cor").collapse().unwrap();
}

#[test]
fn a_slide_a_swatch_and_an_anchor_point_are_found_by_what_they_are() {
    use ravenrs_core::Color;
    use ravenrs_widgets::{AnchorPoint, AnyView, Carousel, NinePointAnchor, SwatchGrid, Text};
    let picked = State::new(None);
    let anchor = State::new(AnchorPoint::Center);
    let p = picked.clone();
    let slides: Vec<AnyView> =
        vec![Box::new(Text::new("Um")), Box::new(Text::new("Dois")), Box::new(Text::new("Três"))];
    let view = VStack::new((
        Carousel::new(slides),
        SwatchGrid::new([(Color::hex(0xFF0000), "Vermelho"), (Color::hex(0x00FF00), "Verde")])
            .on_pick(move |color, _| p.set(Some(color))),
        NinePointAnchor::new(anchor.binding()),
    ));
    let mut e2e = session(view);
    e2e.by_test_id("slide:3").click().unwrap();
    assert_eq!(e2e.by_test_id("slide:3").node().unwrap().selected, Some(true));
    e2e.by_test_id("swatch:Verde").click().unwrap();
    assert_eq!(picked.get(), Some(Color::hex(0x00FF00)));
    // The nine points are found by their name; the first is the top left corner.
    let first = e2e.by_role(ravenrs_core::Role::RadioButton).nth(0).node().unwrap();
    let id = first.test_id.clone().expect("an id by its name");
    assert!(id.starts_with("option:"), "{id}");
    e2e.by_test_id(id).perform(AccessAction::Select).unwrap();
    assert_eq!(anchor.get(), AnchorPoint::TopLeft);
}
