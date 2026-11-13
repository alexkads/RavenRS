//! Long lists: each test fails if its feature is removed.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "test helpers fail loudly")]

use super::lists_support;

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;

use ravenrs_core::{ImageData, Key, Modifiers, Point, Primitive, Role, Size};
use ravenrs_reactive::State;
use ravenrs_widgets::*;

use lists_support::*;

fn height_of(index: usize) -> f32 {
    20.0 + (index % 5) as f32 * 30.0
}

fn tall_list(count: usize) -> VirtualList {
    VirtualList::new(count, |i| Tall(format!("row {i}"), height_of(i))).height(300.0).key(|i| i as u64)
}

#[test]
fn only_what_shows_is_built() {
    let mut ui = ui(tall_list(10_000));
    assert!(has(&mut ui, "row 0"));
    assert!(!has(&mut ui, "row 500"));
    assert!(ui.element_count() < 200, "built {} elements", ui.element_count());
    assert_eq!(of_role(&mut ui, Role::List)[0].description.as_deref(), Some("10000"));
}

#[test]
fn rows_are_placed_by_their_measured_height() {
    let mut ui = ui(tall_list(100));
    let mut y = |i: usize| named(&mut ui, &format!("row {i}")).bounds.y();
    for i in 1..4 {
        assert!((y(i) - y(i - 1) - height_of(i - 1)).abs() < 0.5, "row {i}");
    }
}

#[test]
fn scrolling_reaches_the_last_row_with_a_true_length() {
    let mut ui = ui(tall_list(1000));
    tab_to(&mut ui, "List");
    press(&mut ui, Key::End, Modifiers::default());
    settle(&mut ui);
    let last = named(&mut ui, "row 999");
    let bottom = last.bounds.y() + last.bounds.height();
    assert!(bottom <= 300.5 && bottom > 200.0, "last row ends at {bottom}");
}

#[test]
fn jump_goes_to_a_row_and_stays_on_it_as_the_rows_above_are_measured() {
    let target = State::new(None::<usize>);
    let mut ui = ui(tall_list(5000).scroll_to(target.binding()));
    target.set(Some(4000));
    settle(&mut ui);
    assert_eq!(target.get(), None);
    assert!(named(&mut ui, "row 4000").bounds.y().abs() < 1.0, "{:?}", named(&mut ui, "row 4000").bounds);
}

#[test]
fn a_fixed_height_needs_no_measuring_and_scrolls_by_arithmetic() {
    let target = State::new(None::<usize>);
    let mut ui = ui(VirtualList::new(10_000, |i| Tall(format!("row {i}"), 30.0))
        .row_height(30.0)
        .height(300.0)
        .scroll_to(target.binding()));
    target.set(Some(5000));
    settle(&mut ui);
    assert!(named(&mut ui, "row 5000").bounds.y().abs() < 0.5);
    assert!((named(&mut ui, "row 5001").bounds.y() - 30.0).abs() < 0.5);
}

/// Rows that come from the app, which can grow on either side.
fn keyed_rows(rows: State<Vec<u64>>) -> impl View {
    view_fn(move || {
        let shown = rows.get();
        let (keys, count) = (shown.clone(), shown.len());
        VirtualList::new(count, move |i| Tall(format!("row {}", shown[i]), 40.0))
            .key(move |i| keys[i])
            .estimated_height(40.0)
            .height(300.0)
    })
}

#[test]
fn rows_entering_above_leave_what_is_on_screen_still() {
    let rows = State::new((100..300).collect::<Vec<u64>>());
    let mut ui = ui(keyed_rows(rows.clone()));
    wheel(&mut ui, Point::new(100.0, 100.0), Point::new(0.0, -800.0));
    let before = named(&mut ui, "row 125").bounds.y();
    rows.update(|v| {
        for key in (50..100).rev() {
            v.insert(0, key);
        }
    });
    settle(&mut ui);
    let after = named(&mut ui, "row 125").bounds.y();
    assert!((before - after).abs() < 1.0, "moved from {before} to {after}");
}

#[test]
fn following_the_end_keeps_new_rows_in_view_until_the_user_scrolls_away() {
    let rows = State::new((0..50).collect::<Vec<u64>>());
    let list = {
        let rows = rows.clone();
        view_fn(move || {
            let shown = rows.get();
            let (keys, count) = (shown.clone(), shown.len());
            VirtualList::new(count, move |i| Tall(format!("row {}", shown[i]), 40.0))
                .key(move |i| keys[i])
                .height(300.0)
                .follow_end(true)
        })
    };
    let mut ui = ui(list);
    assert!(has(&mut ui, "row 49"), "it starts at the end");
    rows.update(|v| v.push(50));
    settle(&mut ui);
    let last = named(&mut ui, "row 50");
    assert!((last.bounds.y() + 40.0 - 300.0).abs() < 1.0, "{:?}", last.bounds);
    wheel(&mut ui, Point::new(100.0, 100.0), Point::new(0.0, 400.0));
    rows.update(|v| v.push(51));
    settle(&mut ui);
    assert!(!has(&mut ui, "row 51"), "scrolled away, it stays put");
}

#[test]
fn arrows_select_and_reveal_and_enter_activates() {
    let selected = State::new(None::<usize>);
    let opened = Rc::new(Cell::new(None));
    let seen = opened.clone();
    let mut ui = ui(VirtualList::new(100, |i| Tall(format!("row {i}"), 50.0))
        .row_height(50.0)
        .height(200.0)
        .selection(selected.binding())
        .on_activate(move |i| seen.set(Some(i))));
    tab_to(&mut ui, "List");
    for _ in 0..8 {
        press(&mut ui, Key::ArrowDown, Modifiers::default());
    }
    assert_eq!(selected.get(), Some(8));
    assert!(named(&mut ui, "row 8").bounds.y() < 200.0 - 49.0);
    press(&mut ui, Key::Enter, Modifiers::default());
    assert_eq!(opened.get(), Some(8));
    press(&mut ui, Key::Home, Modifiers::default());
    assert_eq!(selected.get(), Some(0));
    assert!(has(&mut ui, "row 0"));
    let items = of_role(&mut ui, Role::ListItem);
    assert!(items.iter().any(|n| n.selected == Some(true)));
}

#[test]
fn a_list_with_nothing_to_select_scrolls_from_the_keyboard() {
    let mut ui = ui(tall_list(500));
    tab_to(&mut ui, "List");
    press(&mut ui, Key::PageDown, Modifiers::default());
    press(&mut ui, Key::PageDown, Modifiers::default());
    assert!(!has(&mut ui, "row 0"));
    press(&mut ui, Key::Home, Modifiers::default());
    assert!(has(&mut ui, "row 0"));
}

#[test]
fn nothing_to_show_and_no_room_are_not_a_problem() {
    let mut ui = ui(VirtualList::new(0, |i| Tall(format!("row {i}"), 30.0)).height(100.0));
    tab_to(&mut ui, "List");
    press(&mut ui, Key::ArrowDown, Modifiers::default());
    let selection = State::new(BTreeSet::new());
    let mut strip = self::ui(
        Filmstrip::new(0, |_: FilmstripCell| Tall(String::new(), 1.0)).selection(selection.binding()),
    );
    assert!(cells(&mut strip).is_empty());
    tab_to(&mut strip, "Thumbnails");
    press(&mut strip, Key::End, Modifiers::default());
    strip.set_viewport(Size::new(0.0, 0.0), 1.0);
    settle(&mut strip);
    let document = PagedDocumentView::new(0, |_| Size::new(1.0, 1.0), |_| None);
    let mut empty = self::ui(document);
    settle(&mut empty);
}

#[test]
fn it_says_which_rows_show() {
    let seen = Rc::new(RefCell::new(0..0));
    let out = seen.clone();
    let mut ui = ui(tall_list(500).on_visible(move |range| *out.borrow_mut() = range));
    assert_eq!(seen.borrow().start, 0);
    wheel(&mut ui, Point::new(100.0, 100.0), Point::new(0.0, -2000.0));
    assert!(seen.borrow().start > 10, "{:?}", seen.borrow());
}

fn strip(selection: &State<BTreeSet<usize>>, mode: SelectionMode) -> Filmstrip {
    Filmstrip::new(3000, |cell: FilmstripCell| Tall(format!("photo {}", cell.index), 60.0))
        .cell_name(|i| format!("cell {i}"))
        .selection(selection.binding())
        .mode(mode)
        .cell_width(100.0)
        .height(80.0)
}

fn cells(ui: &mut Ui) -> Vec<ravenrs_core::AccessNode> {
    of_role(ui, Role::ListBoxOption)
}

#[test]
fn a_strip_builds_little_however_long_it_is() {
    let selection = State::new(BTreeSet::new());
    let mut ui = ui(strip(&selection, SelectionMode::Single));
    assert!(cells(&mut ui).len() < 16, "{}", cells(&mut ui).len());
    assert!(has(&mut ui, "photo 0"));
    assert!(!has(&mut ui, "photo 2000"));
}

#[test]
fn modifiers_choose_how_a_click_selects() {
    let selection = State::new(BTreeSet::new());
    let mut ui = ui(strip(&selection, SelectionMode::Multiple));
    let at = |ui: &mut Ui, i: usize| center(&named(ui, &format!("cell {i}")));
    let first = at(&mut ui, 1);
    click_at(&mut ui, first, Modifiers::default());
    assert_eq!(selection.get(), BTreeSet::from([1]));
    let third = at(&mut ui, 3);
    click_at(&mut ui, third, ctrl());
    assert_eq!(selection.get(), BTreeSet::from([1, 3]));
    let fifth = at(&mut ui, 5);
    click_at(&mut ui, fifth, shift());
    assert_eq!(selection.get(), BTreeSet::from([3, 4, 5]));
    let again = at(&mut ui, 4);
    click_at(&mut ui, again, Modifiers::default());
    assert_eq!(selection.get(), BTreeSet::from([4]));
    assert_eq!(named(&mut ui, "cell 4").selected, Some(true));
}

#[test]
fn single_selection_ignores_the_modifiers() {
    let selection = State::new(BTreeSet::new());
    let mut ui = ui(strip(&selection, SelectionMode::Single));
    let one = center(&named(&mut ui, "cell 1"));
    let two = center(&named(&mut ui, "cell 2"));
    click_at(&mut ui, one, Modifiers::default());
    click_at(&mut ui, two, ctrl());
    assert_eq!(selection.get(), BTreeSet::from([2]));
}

#[test]
fn the_keyboard_walks_the_strip_and_keeps_the_selection_in_view() {
    let selection = State::new(BTreeSet::new());
    let mut ui = ui(strip(&selection, SelectionMode::Multiple));
    tab_to(&mut ui, "Thumbnails");
    press(&mut ui, Key::End, Modifiers::default());
    assert_eq!(selection.get(), BTreeSet::from([2999]));
    assert!(has(&mut ui, "cell 2999"), "the end came into view");
    let bounds = named(&mut ui, "cell 2999").bounds;
    assert!(bounds.max_x() <= 800.5, "{bounds:?}");
    press(&mut ui, Key::ArrowLeft, shift());
    assert_eq!(selection.get(), BTreeSet::from([2998, 2999]));
    press(&mut ui, Key::Home, Modifiers::default());
    assert_eq!(selection.get(), BTreeSet::from([0]));
    assert!(has(&mut ui, "cell 0"));
    press(&mut ui, Key::ArrowRight, ctrl());
    assert_eq!(selection.get(), BTreeSet::from([0]), "Ctrl moves without selecting");
    press(&mut ui, Key::Space, Modifiers::default());
    assert_eq!(selection.get(), BTreeSet::from([0, 1]));
    press(&mut ui, Key::Character("a".into()), ctrl());
    assert_eq!(selection.get().len(), 3000);
}

#[test]
fn a_selection_changed_from_outside_comes_into_view() {
    let selection = State::new(BTreeSet::new());
    let mut ui = ui(strip(&selection, SelectionMode::Single));
    selection.set(BTreeSet::from([1500]));
    settle(&mut ui);
    assert!(has(&mut ui, "cell 1500"));
}

#[test]
fn arrows_show_only_on_the_ends_with_more_and_move_the_strip() {
    let selection = State::new(BTreeSet::new());
    let mut ui = ui(strip(&selection, SelectionMode::Single));
    assert!(!has(&mut ui, "Scroll back"));
    let forward = center(&named(&mut ui, "Scroll forward"));
    click_at(&mut ui, forward, Modifiers::default());
    assert!(has(&mut ui, "Scroll back"));
    assert!(!has(&mut ui, "cell 0"));
}

#[test]
fn a_plain_wheel_moves_the_strip_sideways() {
    let selection = State::new(BTreeSet::new());
    let mut ui = ui(strip(&selection, SelectionMode::Single));
    wheel(&mut ui, Point::new(300.0, 30.0), Point::new(0.0, -1500.0));
    assert!(!has(&mut ui, "cell 0"));
}

#[test]
fn each_cell_is_told_its_state() {
    let selection = State::new(BTreeSet::from([2]));
    let told = Rc::new(RefCell::new(Vec::new()));
    let out = told.clone();
    let mut ui = ui(Filmstrip::new(5, move |cell: FilmstripCell| {
        out.borrow_mut().push((cell.index, cell.selected));
        Tall(String::new(), 10.0)
    })
    .selection(selection.binding()));
    settle(&mut ui);
    assert!(told.borrow().contains(&(2, true)));
    assert!(told.borrow().contains(&(1, false)));
}

fn document(
    page: &State<usize>,
    zoom: &State<ZoomView>,
    asked: Rc<RefCell<BTreeSet<usize>>>,
) -> PagedDocumentView {
    let picture = ImageData::from_fn(8, 8, |_, _| ravenrs_core::Color::rgb(1.0, 1.0, 1.0)).unwrap();
    PagedDocumentView::new(
        200,
        |_| Size::new(400.0, 500.0),
        move |index| {
            asked.borrow_mut().insert(index);
            (index != 1).then(|| picture.clone())
        },
    )
    .page(page.binding())
    .zoom(zoom.binding())
    .height(600.0)
}

/// A page, not the line in the bar that says the same.
fn sheet(ui: &mut Ui, name: &str) -> ravenrs_core::AccessNode {
    of_role(ui, Role::Image).into_iter().find(|n| n.name.as_deref() == Some(name)).expect("that page")
}

#[test]
fn only_the_pages_that_show_are_asked_for() {
    let (page, zoom) = (State::new(0), State::new(ZoomView::at(1.0)));
    let asked = Rc::new(RefCell::new(BTreeSet::new()));
    let mut ui = ui(document(&page, &zoom, asked.clone()));
    assert!(asked.borrow().contains(&0));
    assert!(asked.borrow().len() < 12, "{:?}", asked.borrow());
    assert!(has(&mut ui, "Page 1 of 200"));
}

#[test]
fn the_page_follows_the_scroll_and_going_to_a_page_scrolls() {
    let (page, zoom) = (State::new(0), State::new(ZoomView::at(1.0)));
    let mut ui = ui(document(&page, &zoom, Rc::default()));
    page.set(100);
    settle(&mut ui);
    assert!(has(&mut ui, "Page 101 of 200"));
    assert_eq!(page.get(), 100, "going there does not move the page again");
    wheel(&mut ui, Point::new(400.0, 300.0), Point::new(0.0, -1100.0));
    assert_eq!(page.get(), 102);
}

#[test]
fn a_document_takes_the_room_of_its_parent_and_scrolls_inside_it() {
    let picture = ImageData::from_fn(8, 8, |_, _| ravenrs_core::Color::rgb(1.0, 1.0, 1.0)).unwrap();
    let page = State::new(0);
    let mut ui = ui(PagedDocumentView::new(40, |_| Size::new(400.0, 500.0), move |_| Some(picture.clone()))
        .page(page.binding()));
    wheel(&mut ui, Point::new(400.0, 300.0), Point::new(0.0, -5000.0));
    assert!(page.get() > 3, "{}", page.get());
}

#[test]
fn the_bar_and_the_keys_change_the_page_and_the_zoom() {
    let (page, zoom) = (State::new(0), State::new(ZoomView::at(1.0)));
    let mut ui = ui(document(&page, &zoom, Rc::default()));
    let next = center(&named(&mut ui, "Next page"));
    click_at(&mut ui, next, Modifiers::default());
    assert_eq!(page.get(), 1);
    let more = center(&named(&mut ui, "Zoom in"));
    click_at(&mut ui, more, Modifiers::default());
    assert!(zoom.get().scale > 1.0);
    let width = sheet(&mut ui, "Page 2 of 200").bounds.width();
    assert!((width - 400.0 * zoom.get().scale).abs() < 1.0, "{width}");
    tab_to(&mut ui, "Document");
    press(&mut ui, Key::PageDown, Modifiers::default());
    assert_eq!(page.get(), 2);
    press(&mut ui, Key::End, Modifiers::default());
    assert_eq!(page.get(), 199);
    let before = zoom.get().scale;
    press(&mut ui, Key::Character("-".into()), Modifiers::default());
    assert!(zoom.get().scale < before);
    press(&mut ui, Key::Character("0".into()), Modifiers::default());
    assert!(zoom.get().fit);
}

#[test]
fn the_fit_follows_the_width_of_the_view() {
    let (page, zoom) = (State::new(0), State::new(ZoomView::fit()));
    let mut ui = ui(document(&page, &zoom, Rc::default()));
    let wide = sheet(&mut ui, "Page 1 of 200").bounds.width();
    assert!((wide - (800.0 - 32.0)).abs() < 2.0, "{wide}");
    assert!((zoom.get().scale - 768.0 / 400.0).abs() < 0.01);
    ui.set_viewport(Size::new(500.0, 600.0), 1.0);
    settle(&mut ui);
    let narrow = sheet(&mut ui, "Page 1 of 200").bounds.width();
    assert!((narrow - (500.0 - 32.0)).abs() < 2.0, "{narrow}");
}

#[test]
fn a_page_without_its_picture_keeps_its_place() {
    let (page, zoom) = (State::new(0), State::new(ZoomView::at(1.0)));
    let mut ui = ui(document(&page, &zoom, Rc::default()));
    let pictures = ui.scene().primitives().iter().filter(|p| matches!(p, Primitive::Image(_))).count();
    assert!(pictures >= 1);
    assert!((sheet(&mut ui, "Page 2 of 200").bounds.height() - 500.0).abs() < 1.0);
}
