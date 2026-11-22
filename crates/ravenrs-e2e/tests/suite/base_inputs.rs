//! The input controls of `ravenrs-widgets-base` keep the testability contract (#737, first lot):
//! their parts are found by test ids made from the data, and they take the actions a screen
//! reader asks for, so a scenario needs no coordinates.

use ravenrs_core::Size;
use ravenrs_e2e::{E2e, ErrorCode};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::{
    Checkbox, Combobox, Date, DatePicker, InputOtp, RadioGroup, Select, Slider, TextField, Toggle, VStack,
    View, ViewExt,
};

fn session(view: impl View + 'static) -> E2e {
    E2e::new(view, Theme::raven_dark()).viewport(Size::new(900.0, 700.0), 1.0)
}

const FRUITS: [&str; 3] = ["Maçã", "Banana", "Uva"];

#[test]
fn a_text_field_takes_its_value_and_a_secret_one_never_gives_it() {
    let (name, word) = (State::new(String::new()), State::new(String::new()));
    let view = VStack::new((
        TextField::new(name.binding()).test_id("nome"),
        TextField::new(word.binding()).secure().test_id("senha"),
    ));
    let mut e2e = session(view);
    e2e.by_test_id("nome").set_value("Ana").unwrap();
    assert_eq!(name.get(), "Ana");
    e2e.by_test_id("senha").set_value("segredo").unwrap();
    assert_eq!(word.get(), "segredo");
    let error = e2e.by_test_id("senha").expect().to_have_value("segredo").unwrap_err();
    assert_eq!(error.code(), ErrorCode::ProtectedValue);
}

#[test]
fn a_disabled_field_refuses_the_value() {
    let text = State::new(String::new());
    let mut e2e = session(VStack::new((TextField::new(text.binding()).disabled(true).test_id("nome"),)));
    assert_eq!(e2e.by_test_id("nome").set_value("x").unwrap_err().code(), ErrorCode::Disabled);
    assert_eq!(text.get(), "");
}

#[test]
fn a_checkbox_and_a_toggle_are_clicked() {
    let (check, on) = (State::new(false), State::new(false));
    let view = VStack::new((
        Checkbox::new("Aceito", check.binding()).test_id("aceito"),
        Toggle::new(on.binding()).label("Ligado").test_id("ligado"),
    ));
    let mut e2e = session(view);
    e2e.by_test_id("aceito").click().unwrap();
    e2e.by_test_id("ligado").click().unwrap();
    assert!(check.get() && on.get());
}

#[test]
fn a_slider_takes_a_number_on_its_steps_and_inside_its_range() {
    let value = State::new(0.0_f32);
    let mut e2e =
        session(VStack::new((Slider::new(value.binding()).range(0.0, 10.0).step(0.5).test_id("volume"),)));
    e2e.by_test_id("volume").set_value("3,3").unwrap();
    assert_eq!(value.get(), 3.5);
    e2e.by_test_id("volume").set_value("99").unwrap();
    assert_eq!(value.get(), 10.0);
}

#[test]
fn a_select_opens_and_its_option_is_chosen_by_its_value() {
    let chosen = State::new(0_usize);
    let mut e2e = session(VStack::new((Select::new(chosen.binding(), FRUITS).test_id("fruta"),)));
    e2e.by_test_id("fruta").expand().unwrap();
    e2e.by_test_id("option:Uva").perform(ravenrs_core::AccessAction::Select).unwrap();
    assert_eq!(chosen.get(), 2);
    e2e.by_test_id("fruta").select("Banana").unwrap();
    assert_eq!(chosen.get(), 1);
}

#[test]
fn a_combobox_takes_the_text_of_an_option_and_finds_its_options_by_value() {
    let chosen = State::new(None);
    let mut e2e = session(VStack::new((Combobox::new(chosen.binding(), FRUITS).test_id("fruta"),)));
    e2e.by_test_id("fruta").set_value("banana").unwrap();
    assert_eq!(chosen.get(), Some(1));
    e2e.by_test_id("fruta").expect().to_have_value("Banana").unwrap();
    e2e.by_test_id("fruta").expand().unwrap();
    e2e.by_test_id("option:Uva").perform(ravenrs_core::AccessAction::Select).unwrap();
    assert_eq!(chosen.get(), Some(2));
    // A text that is no option is refused, and the choice stays.
    assert_eq!(e2e.by_test_id("fruta").set_value("Kiwi").unwrap_err().code(), ErrorCode::Expectation);
    assert_eq!(chosen.get(), Some(2));
}

#[test]
fn a_radio_group_keeps_the_ids_of_its_options_when_they_change_order() {
    let chosen = State::new(0_usize);
    let options = State::new(FRUITS.map(String::from).to_vec());
    let (c, o) = (chosen.clone(), options.clone());
    let view = ravenrs_widgets::view_fn(move || RadioGroup::new(c.binding(), o.get()));
    let mut e2e = session(view);
    e2e.by_test_id("option:Uva").perform(ravenrs_core::AccessAction::Select).unwrap();
    assert_eq!(chosen.get(), 2);
    // The options change order: the same id still finds "Uva", now first.
    options.set(vec!["Uva".into(), "Maçã".into(), "Banana".into()]);
    e2e.advance(0.1);
    e2e.by_test_id("option:Banana").perform(ravenrs_core::AccessAction::Select).unwrap();
    assert_eq!(chosen.get(), 2, "Banana is the third option now");
    let uva = e2e.by_test_id("option:Uva").node().unwrap();
    assert_eq!(uva.name.as_deref(), Some("Uva"));
}

#[test]
fn a_date_picker_takes_a_day_and_its_days_are_found_by_date() {
    let day = State::new(None);
    let month = Date::new(2026, 11, 1).unwrap();
    let mut e2e = session(VStack::new((DatePicker::new(day.binding(), month).test_id("dia"),)));
    e2e.by_test_id("dia").set_value("2026-11-20").unwrap();
    assert_eq!(day.get(), Date::new(2026, 11, 20));
    e2e.by_test_id("dia").expand().unwrap();
    e2e.by_test_id("day:2026-11-03").perform(ravenrs_core::AccessAction::Select).unwrap();
    assert_eq!(day.get(), Date::new(2026, 11, 3));
    assert_eq!(e2e.by_test_id("dia").set_value("ontem").unwrap_err().code(), ErrorCode::Expectation);
}

#[test]
fn a_code_field_takes_the_whole_code_digits_only() {
    let code = State::new(String::new());
    let done = State::new(String::new());
    let d = done.clone();
    let view =
        VStack::new((InputOtp::new(code.binding(), 6).on_complete(move |c| d.set(c)).test_id("codigo"),));
    let mut e2e = session(view);
    e2e.by_test_id("codigo").set_value("12-34-56").unwrap();
    assert_eq!(code.get(), "123456");
    assert_eq!(done.get(), "123456");
}
