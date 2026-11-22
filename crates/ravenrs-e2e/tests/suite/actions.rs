//! Acting through real input events, and the checks after.

use ravenrs_core::{Key, Role};
use ravenrs_e2e::{E2e, ErrorCode, Query};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::{Button, TextField, Toggle, VStack, View, ViewExt};

fn session(view: impl View + 'static) -> E2e {
    E2e::new(view, Theme::raven_dark())
}

#[test]
fn a_disabled_button_refuses_the_click_and_its_handler_never_runs() {
    let clicks = State::new(0);
    let c = clicks.clone();
    let mut e2e = session(VStack::new((Button::new("Archive")
        .on_click(move || c.update(|n| *n += 1))
        .disabled(true)
        .test_id("archive"),)));
    let error = e2e.by_test_id("archive").click().unwrap_err();
    assert_eq!(error.code(), ErrorCode::Disabled);
    assert_eq!(clicks.get(), 0);
    e2e.by_test_id("archive").expect().to_be_disabled().unwrap();
    assert_eq!(
        e2e.by_test_id("archive").expect().to_be_enabled().unwrap_err().code(),
        ErrorCode::Expectation
    );
}

#[test]
fn typing_and_backspace_edit_a_text_field() {
    let text = State::new(String::new());
    let mut e2e = session(VStack::new((TextField::new(text.binding()).placeholder("Name").test_id("name"),)));
    e2e.by_test_id("name").type_text("abc").unwrap();
    e2e.by_test_id("name").expect().to_be_focused().unwrap();
    e2e.by_test_id("name").press(Key::Backspace).unwrap();
    e2e.by_test_id("name").expect().to_have_value("ab").unwrap();
    assert_eq!(text.get(), "ab");
    let wrong = e2e.by_test_id("name").expect().to_have_value("abc").unwrap_err();
    assert_eq!(wrong.code(), ErrorCode::Expectation);
    assert!(wrong.message().contains("\"ab\""), "{wrong}");
}

#[test]
fn a_secret_value_is_never_given_out() {
    let secret = State::new(String::new());
    let mut e2e = session(VStack::new((TextField::new(secret.binding())
        .secure()
        .placeholder("Password")
        .test_id("password"),)));
    e2e.by_test_id("password").type_text("hunter2").unwrap();
    assert_eq!(secret.get(), "hunter2", "the field still takes the text");
    let error = e2e.by_test_id("password").expect().to_have_value("hunter2").unwrap_err();
    assert_eq!(error.code(), ErrorCode::ProtectedValue);
    assert!(!e2e.snapshot().contains("hunter2"));
    assert!(e2e.snapshot().contains("value=protected"));
    assert!(e2e.find(&Query::text("hunter2")).is_err(), "the secret cannot be searched for either");
}

#[test]
fn a_toggle_checks_when_clicked() {
    let on = State::new(false);
    let mut e2e = session(VStack::new((Toggle::new(on.binding()).label("Dark mode").test_id("dark"),)));
    e2e.by_test_id("dark").click().unwrap();
    assert!(on.get());
    e2e.by_test_id("dark").expect().to_be_checked().unwrap();
    e2e.by_test_id("dark").expect().to_have_count(1).unwrap();
    e2e.by_role(Role::Switch).expect().to_be_visible().unwrap();
}
