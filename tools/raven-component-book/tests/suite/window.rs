//! The book's window, driven as a person does: choose a component, a story, change a setting.

use ravenrs::prelude::*;
use ravenrs_e2e::E2e;

fn book() -> E2e {
    E2e::new(raven_component_book::app(), Theme::raven_dark()).viewport(Size::new(1440.0, 900.0), 1.0)
}

#[test]
fn choosing_a_component_shows_its_stories_and_changing_a_setting_redraws_it() {
    let mut book = book();
    book.by_test_id("search").fill("textf").expect("the search");
    book.by_test_id("component:TextField").click().expect("TextField in the sidebar");
    let screen = book.snapshot();
    assert!(screen.contains("Forms/TextField"), "{screen}");
    assert!(screen.contains("Password"), "its stories:\n{screen}");
    book.by_test_id("search").fill("").expect("the search");
    book.by_test_id("component:Button").scroll_into_view().expect("Button in the sidebar");
    book.by_test_id("component:Button").click().expect("Button in the sidebar");
    let canvas = |screen: &str| screen.lines().find(|l| l.contains("#canvas")).unwrap_or_default().to_owned();
    assert!(!canvas(&book.snapshot()).contains("disabled"));
    book.by_test_id("arg:disabled").click().expect("the disabled setting");
    let screen = book.snapshot();
    assert!(
        canvas(&screen).contains("button \"Save\"") && canvas(&screen).contains("disabled"),
        "the story redraws disabled:\n{screen}"
    );
}

#[test]
fn a_component_outside_the_widgets_shows_its_own_stories() {
    let mut book = book();
    book.by_test_id("search").fill("MoviePlayer").expect("the search");
    book.by_test_id("component:MoviePlayer").click().expect("MoviePlayer in the sidebar");
    let screen = book.snapshot();
    assert!(screen.contains("Application/MoviePlayer"), "{screen}");
    assert!(!screen.contains("histórias pendentes"), "it gives its own stories:\n{screen}");
    assert!(screen.contains("0 pendentes"), "the status bar counts none pending:\n{screen}");
}
