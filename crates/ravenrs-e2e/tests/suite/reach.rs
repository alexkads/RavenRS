//! Acting on a part out of sight: it is scrolled to first, as a person scrolls before clicking
//! (#732). The window stays the size of a real one.

use ravenrs_core::Size;
use ravenrs_e2e::{E2e, ErrorCode};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::{AnyView, Button, ScrollView, VStack, View, ViewExt};

/// Buttons `item:0` to `item:{count - 1}`; clicking one leaves its number in `clicked`.
fn items(count: usize, clicked: &State<Option<usize>>) -> Vec<AnyView> {
    (0..count)
        .map(|i| {
            let c = clicked.clone();
            Box::new(
                Button::new(format!("Item {i}"))
                    .on_click(move || c.set(Some(i)))
                    .test_id(format!("item:{i}")),
            ) as AnyView
        })
        .collect()
}

/// An empty block this tall, to push what follows down.
fn gap(height: f32) -> VStack<Vec<AnyView>> {
    VStack::new(Vec::new()).height(height)
}

fn session(view: impl View + 'static, width: f32, height: f32) -> E2e {
    E2e::new(view, Theme::raven_dark()).viewport(Size::new(width, height), 1.0)
}

#[test]
fn a_button_below_the_window_in_a_page_that_scrolls_is_scrolled_to_and_clicked() {
    let clicked = State::new(None);
    let mut e2e = session(ScrollView::new(items(30, &clicked)), 400.0, 300.0);
    e2e.by_test_id("item:25").click().unwrap();
    assert_eq!(clicked.get(), Some(25));
}

#[test]
fn a_button_a_short_scroll_view_hides_is_scrolled_to_even_inside_the_window() {
    // The window is tall enough for the button's place, but the list shows only its top:
    // at that point the pointer would land on what comes after the list.
    let clicked = State::new(None);
    let after = State::new(false);
    let a = after.clone();
    let view = VStack::new((
        ScrollView::new(items(20, &clicked)).height(140.0),
        Button::new("After the list").on_click(move || a.set(true)).test_id("after"),
        gap(900.0),
    ));
    let mut e2e = session(view, 400.0, 1080.0);
    e2e.by_test_id("item:15").click().unwrap();
    assert_eq!(clicked.get(), Some(15));
    assert!(!after.get(), "the click went to what is after the list");
}

#[test]
fn a_list_inside_a_page_that_scrolls_is_reached_through_both() {
    // A dialog's list far down a long form: the page scrolls to the list, the list to the item.
    let clicked = State::new(None);
    let view = ScrollView::new((gap(700.0), ScrollView::new(items(20, &clicked)).height(150.0), gap(400.0)));
    let mut e2e = session(view, 400.0, 300.0);
    e2e.by_test_id("item:18").click().unwrap();
    assert_eq!(clicked.get(), Some(18));
}

#[test]
fn a_button_out_of_the_window_with_nothing_to_scroll_it_is_not_visible() {
    let clicked = State::new(None);
    let mut items = items(1, &clicked);
    items.insert(0, Box::new(gap(600.0)));
    let mut e2e = session(VStack::new(items), 400.0, 300.0);
    let error = e2e.by_test_id("item:0").click().unwrap_err();
    assert_eq!(error.code(), ErrorCode::NotVisible, "{error}");
    assert_eq!(clicked.get(), None);
}
