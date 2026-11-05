//! Rows and tabs that change places: each test fails if its feature is removed.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "test helpers fail loudly")]

mod lists_support;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ravenrs_core::{Event, Key, Modifiers, Point, PointerButton, Primitive, Role};
use ravenrs_reactive::State;
use ravenrs_style::{Semantic, Theme};
use ravenrs_widgets::*;

use lists_support::*;

fn sortable(items: &State<Vec<&'static str>>) -> impl View + use<> {
    let (items, moved) = (items.clone(), items.clone());
    view_fn(move || {
        let shown = items.get();
        let names = shown.clone();
        let moved = moved.clone();
        SortableList::new(shown.len(), move |i| Tall(shown[i].to_owned(), 30.0))
            .row_name(move |i| format!("item {}", names[i]))
            .key(|i| i as u64)
            .on_reorder(move |from, to| {
                moved.update(|v| {
                    let item = v.remove(from);
                    v.insert(to, item);
                });
            })
    })
}

fn drag(ui: &mut Ui, from: Point, to: Point) {
    ui.handle_event(&Event::PointerMove { position: from });
    ui.handle_event(&Event::PointerDown { position: from, button: PointerButton::Primary });
    settle(ui);
    ui.handle_event(&Event::PointerMove { position: to });
    settle(ui);
}

fn accent_lines(ui: &mut Ui) -> usize {
    let accent = Theme::raven_dark().color(Semantic::Accent);
    ui.scene()
        .primitives()
        .iter()
        .filter(|p| match p {
            Primitive::Quad(q) => {
                matches!(q.fill, ravenrs_core::Brush::Solid(c) if c == accent)
                    && (q.rect.height() <= 2.5 || q.rect.width() <= 2.5)
            }
            _ => false,
        })
        .count()
}

fn release(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    settle(ui);
}

#[test]
fn dragging_shows_the_target_and_moves_on_release() {
    let items = State::new(vec!["a", "b", "c", "d"]);
    let mut ui = ui(sortable(&items));
    let (from, to) = (named(&mut ui, "item a"), named(&mut ui, "item d"));
    assert_eq!(accent_lines(&mut ui), 0);
    let target = Point::new(center(&to).x, to.bounds.max_y() + 2.0);
    drag(&mut ui, center(&from), target);
    assert_eq!(accent_lines(&mut ui), 1, "the line that shows where it lands");
    assert_eq!(items.get(), vec!["a", "b", "c", "d"], "nothing moves before the release");
    release(&mut ui, target);
    assert_eq!(items.get(), vec!["b", "c", "d", "a"]);
    assert_eq!(accent_lines(&mut ui), 0);
}

#[test]
fn a_small_movement_is_still_a_click_and_landing_in_place_moves_nothing() {
    let items = State::new(vec!["a", "b", "c"]);
    let mut ui = ui(sortable(&items));
    let from = center(&named(&mut ui, "item b"));
    drag(&mut ui, from, Point::new(from.x, from.y + 2.0));
    assert_eq!(accent_lines(&mut ui), 0);
    release(&mut ui, from);
    assert_eq!(items.get(), vec!["a", "b", "c"]);
}

#[test]
fn escape_calls_a_drag_off() {
    let items = State::new(vec!["a", "b", "c"]);
    let mut ui = ui(sortable(&items));
    let from = center(&named(&mut ui, "item a"));
    let to = center(&named(&mut ui, "item c"));
    let target = Point::new(to.x, to.y + 14.0);
    drag(&mut ui, from, target);
    assert_eq!(accent_lines(&mut ui), 1);
    press(&mut ui, Key::Escape, Modifiers::default());
    assert_eq!(accent_lines(&mut ui), 0);
    release(&mut ui, target);
    assert_eq!(items.get(), vec!["a", "b", "c"]);
}

#[test]
fn alt_and_the_arrows_move_keep_the_keyboard_and_announce() {
    let items = State::new(vec!["a", "b", "c"]);
    let mut ui = ui(sortable(&items));
    tab_to(&mut ui, "item a");
    press(&mut ui, Key::ArrowDown, alt());
    assert_eq!(items.get(), vec!["b", "a", "c"]);
    assert_eq!(focused_name(&mut ui).as_deref(), Some("item a"));
    assert!(has(&mut ui, "Moved to position 2 of 3"));
    let status = of_role(&mut ui, Role::Status);
    assert_eq!(status[0].name.as_deref(), Some("Moved to position 2 of 3"));
    press(&mut ui, Key::End, alt());
    assert_eq!(items.get(), vec!["b", "c", "a"]);
    press(&mut ui, Key::ArrowDown, alt());
    assert_eq!(items.get(), vec!["b", "c", "a"], "already last");
    press(&mut ui, Key::ArrowUp, Modifiers::default());
    assert_eq!(focused_name(&mut ui).as_deref(), Some("item c"), "without Alt it only walks");
    assert_eq!(items.get(), vec!["b", "c", "a"]);
    assert!(named(&mut ui, "item a").description.as_deref().is_some_and(|d| d.contains("Alt")));
}

#[derive(Clone)]
struct Open {
    tabs: State<Vec<DocumentTab>>,
    active: State<u64>,
    closed: Rc<RefCell<Vec<u64>>>,
}

fn open_tabs() -> Open {
    Open {
        tabs: State::new(vec![
            DocumentTab::new(1, "Wedding"),
            DocumentTab::new(2, "Portraits").unsaved(true).color(ravenrs_core::Color::rgb(0.9, 0.2, 0.2)),
            DocumentTab::new(3, "Studio").icon(IconName::Camera),
            DocumentTab::new(4, "Pinned").closable(false),
        ]),
        active: State::new(1),
        closed: Rc::default(),
    }
}

fn tabs_view(open: &Open) -> impl View + use<> {
    let open = open.clone();
    view_fn(move || {
        let (moved, renamed, closed, closed_tabs) =
            (open.tabs.clone(), open.tabs.clone(), open.closed.clone(), open.tabs.clone());
        DocumentTabs::new(open.tabs.get(), open.active.binding())
            .on_close(move |id| {
                closed.borrow_mut().push(id);
                closed_tabs.update(|v| v.retain(|t| t.id != id));
            })
            .on_reorder(move |from, to| {
                moved.update(|v| {
                    let tab = v.remove(from);
                    v.insert(to, tab);
                });
            })
            .on_rename(move |id, name| {
                renamed.update(|v| {
                    if let Some(t) = v.iter_mut().find(|t| t.id == id) {
                        t.title = name;
                    }
                });
            })
            .menu(|_| vec![MenuItem::new("Duplicate")])
    })
}

fn order(open: &Open) -> Vec<u64> {
    open.tabs.get().iter().map(|t| t.id).collect()
}

#[test]
fn a_click_opens_a_tab_and_the_state_is_said() {
    let open = open_tabs();
    let mut ui = ui(tabs_view(&open));
    let tab = center(&named(&mut ui, "Studio"));
    click_at(&mut ui, tab, Modifiers::default());
    assert_eq!(open.active.get(), 3);
    assert_eq!(named(&mut ui, "Studio").selected, Some(true));
    assert_eq!(named(&mut ui, "Wedding").selected, Some(false));
    assert_eq!(named(&mut ui, "Portraits").description.as_deref(), Some("Unsaved changes"));
    assert!(has(&mut ui, "Unsaved changes"), "the dot speaks too");
    assert_eq!(of_role(&mut ui, Role::TabList).len(), 1);
}

#[test]
fn the_button_and_delete_close_a_tab_except_one_that_does_not_close() {
    let open = open_tabs();
    let mut ui = ui(tabs_view(&open));
    let closers = |ui: &mut Ui| {
        ui.access_tree()
            .nodes
            .iter()
            .filter(|n| n.name.as_deref() == Some("Close tab"))
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(closers(&mut ui).len(), 3, "the pinned one has none");
    let first = center(&closers(&mut ui)[0]);
    click_at(&mut ui, first, Modifiers::default());
    assert_eq!(*open.closed.borrow(), vec![1]);
    tab_to(&mut ui, "Studio");
    press(&mut ui, Key::Delete, Modifiers::default());
    assert_eq!(*open.closed.borrow(), vec![1, 3]);
    tab_to(&mut ui, "Pinned");
    press(&mut ui, Key::Delete, Modifiers::default());
    assert_eq!(*open.closed.borrow(), vec![1, 3]);
}

#[test]
fn dragging_a_tab_marks_the_gap_and_moves_it_on_release() {
    let open = open_tabs();
    let mut ui = ui(tabs_view(&open));
    let (from, to) = (center(&named(&mut ui, "Wedding")), center(&named(&mut ui, "Studio")));
    let target = Point::new(to.x + 6.0, to.y);
    drag(&mut ui, from, target);
    assert_eq!(accent_lines(&mut ui), 1);
    assert_eq!(order(&open), vec![1, 2, 3, 4]);
    release(&mut ui, target);
    assert_eq!(order(&open), vec![2, 3, 1, 4]);
}

#[test]
fn alt_and_the_arrows_move_a_tab_and_alone_walk_and_open() {
    let open = open_tabs();
    let mut ui = ui(tabs_view(&open));
    tab_to(&mut ui, "Wedding");
    press(&mut ui, Key::ArrowRight, alt());
    assert_eq!(order(&open), vec![2, 1, 3, 4]);
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Wedding"));
    assert!(has(&mut ui, "Moved to position 2 of 4"));
    press(&mut ui, Key::ArrowRight, Modifiers::default());
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Studio"));
    assert_eq!(open.active.get(), 3);
}

#[test]
fn a_tab_is_renamed_with_f2_a_double_click_or_the_menu() {
    let open = open_tabs();
    let mut ui = ui(tabs_view(&open));
    tab_to(&mut ui, "Wedding");
    press(&mut ui, Key::Other("F2".into()), Modifiers::default());
    press(&mut ui, Key::Character("Bodas".into()), Modifiers::default());
    press(&mut ui, Key::Enter, Modifiers::default());
    assert_eq!(open.tabs.get()[0].title, "Bodas");

    let tab = center(&named(&mut ui, "Studio"));
    ui.handle_event(&Event::PointerMove { position: tab });
    ui.handle_event(&Event::DoubleClick { position: tab });
    settle(&mut ui);
    assert!(!of_role(&mut ui, Role::TextInput).is_empty(), "the field opened");
    press(&mut ui, Key::Escape, Modifiers::default());
    assert!(of_role(&mut ui, Role::TextInput).is_empty());
    assert_eq!(open.tabs.get()[2].title, "Studio");
}

#[test]
fn the_menu_closes_the_others_and_carries_what_the_app_adds() {
    let open = open_tabs();
    let mut ui = ui(tabs_view(&open));
    let tab = center(&named(&mut ui, "Portraits"));
    ui.handle_event(&Event::PointerMove { position: tab });
    ui.handle_event(&Event::PointerDown { position: tab, button: PointerButton::Secondary });
    ui.handle_event(&Event::PointerUp { position: tab, button: PointerButton::Secondary });
    settle(&mut ui);
    assert!(has(&mut ui, "Duplicate"));
    assert!(has(&mut ui, "Rename"));
    assert!(has(&mut ui, "Close tabs to the right"));
    let others = center(&named(&mut ui, "Close other tabs"));
    click_at(&mut ui, others, Modifiers::default());
    assert_eq!(order(&open), vec![2, 4], "the pinned one stays");
}

#[test]
fn tabs_that_do_not_fit_scroll_with_arrows() {
    let tabs: Vec<DocumentTab> = (0..40).map(|i| DocumentTab::new(i, format!("Document {i}"))).collect();
    let active = State::new(0);
    let mut ui = ui(DocumentTabs::new(tabs, active.binding()));
    assert!(has(&mut ui, "Scroll forward"));
}

#[test]
fn a_new_tab_button_appears_when_asked() {
    let active = State::new(1);
    let made = Rc::new(Cell::new(0));
    let out = made.clone();
    let mut ui = ui(DocumentTabs::new(vec![DocumentTab::new(1, "One")], active.binding())
        .on_new(move || out.set(out.get() + 1)));
    let button = center(&named(&mut ui, "New tab"));
    click_at(&mut ui, button, Modifiers::default());
    assert_eq!(made.get(), 1);
}
