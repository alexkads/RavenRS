//! Finding parts of the screen, and failing well when the query does not find one.

use ravenrs_core::Role;
use ravenrs_e2e::{E2e, ErrorCode, Query};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::{Button, Dialog, VStack, View, ViewExt};

fn session(view: impl View + 'static) -> E2e {
    E2e::new(view, Theme::raven_dark())
}

#[test]
fn a_test_id_survives_a_button_inserted_before_it() {
    for extra in [false, true] {
        let clicks = State::new(0);
        let c = clicks.clone();
        let save = Button::new("Save").on_click(move || c.update(|n| *n += 1)).test_id("save");
        let mut e2e = if extra {
            session(VStack::new((Button::new("New"), save)))
        } else {
            session(VStack::new((save,)))
        };
        e2e.by_test_id("save").click().unwrap();
        assert_eq!(clicks.get(), 1, "clicked once with extra={extra}");
    }
}

#[test]
fn a_test_id_on_a_plain_container_reaches_the_first_node_inside() {
    let e2e = session(VStack::new((Button::new("Inside"), Button::new("Next"))).test_id("box"));
    let node = e2e.find(&Query::test_id("box")).unwrap();
    assert_eq!(node.role, Role::Button);
    assert_eq!(node.name.as_deref(), Some("Inside"));
    let next = e2e.find(&Query::role(Role::Button).name("Next")).unwrap();
    assert_eq!(next.test_id, None, "the id goes to one node only");
}

#[test]
fn nothing_found_says_what_was_there_and_what_came_close() {
    let e2e = session(VStack::new((Button::new("Save"), Button::new("Cancel"))));
    let error = e2e.find(&Query::role(Role::Button).name("Salve")).unwrap_err();
    assert_eq!(error.code(), ErrorCode::NotFound);
    assert!(error.message().contains("name \"Salve\""), "{error}");
    assert!(error.close_matches().iter().any(|line| line.contains("\"Save\"")), "{error}");
    assert!(error.screen_text().contains("button \"Cancel\""), "{error}");
    assert!(error.close_matches().len() <= 3);
    assert!(error.to_string().starts_with("[E2E_NOT_FOUND]"));
}

#[test]
fn two_matches_are_ambiguous_until_the_query_picks_one() {
    let e2e = session(VStack::new((Button::new("Open"), Button::new("Open"))));
    let error = e2e.find(&Query::role(Role::Button).name("Open")).unwrap_err();
    assert_eq!(error.code(), ErrorCode::Ambiguous);
    assert!(error.message().contains("2 nodes"), "{error}");
    let second = e2e.find(&Query::role(Role::Button).name("Open").nth(1)).unwrap();
    let first = e2e.find(&Query::role(Role::Button).name("Open").nth(0)).unwrap();
    assert_ne!(first.id, second.id);
    assert_eq!(e2e.count(&Query::role(Role::Button).name("Open")).unwrap(), 2);
}

#[test]
fn within_looks_only_inside_the_parent() {
    let open = State::new(true);
    let e2e =
        session(VStack::new((Dialog::new(open.binding(), "Delete file?").action(Button::new("Delete")),)));
    let inside = Query::role(Role::Button).name("Delete").within(Query::role(Role::Dialog));
    assert_eq!(e2e.find(&inside).unwrap().name.as_deref(), Some("Delete"));
    let elsewhere = Query::role(Role::Button).name("Delete").within(Query::role(Role::Button).name("Close"));
    assert_eq!(e2e.find(&elsewhere).unwrap_err().code(), ErrorCode::NotFound);
}

#[test]
fn a_modal_covers_the_page_behind_it() {
    let open = State::new(true);
    let mut e2e = session(VStack::new((
        Button::new("Behind"),
        Dialog::new(open.binding(), "Delete file?").action(Button::new("Delete")),
    )));
    let error = e2e.by_role(Role::Button).name("Behind").click().unwrap_err();
    assert_eq!(error.code(), ErrorCode::Covered);
    assert!(error.message().contains("Delete file?"), "names the dialog that covers it: {error}");
}
