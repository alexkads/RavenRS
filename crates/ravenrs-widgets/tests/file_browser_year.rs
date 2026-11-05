//! Folders and years: each test fails if its feature is removed.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "test helpers fail loudly")]

mod lists_support;

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;

use ravenrs_core::{Key, Modifiers, Point, Role, Size};
use ravenrs_reactive::State;
use ravenrs_widgets::*;

use lists_support::*;

fn folder() -> Vec<FileEntry> {
    vec![
        FileEntry::file("b.txt", 2048).modified("Nov 2", 2),
        FileEntry::folder("Zeta"),
        FileEntry::file("a.txt", 10).modified("Nov 3", 3).id("first"),
        FileEntry::folder("alpha"),
    ]
}

fn rows_top_down(ui: &mut Ui) -> Vec<String> {
    let mut rows = of_role(ui, Role::ListBoxOption);
    rows.sort_by(|a, b| a.bounds.y().total_cmp(&b.bounds.y()));
    rows.into_iter().filter_map(|r| r.name).collect()
}

#[test]
fn folders_come_first_and_the_headers_sort() {
    let mut ui = ui(FileBrowser::new(["Root"], folder()).height(400.0));
    assert_eq!(rows_top_down(&mut ui), vec!["alpha", "Zeta", "a.txt", "b.txt"]);
    let name = center(&named(&mut ui, "Name"));
    click_at(&mut ui, name, Modifiers::default());
    assert_eq!(rows_top_down(&mut ui), vec!["Zeta", "alpha", "b.txt", "a.txt"]);
    let size = center(&named(&mut ui, "Size"));
    click_at(&mut ui, size, Modifiers::default());
    assert_eq!(rows_top_down(&mut ui), vec!["alpha", "Zeta", "a.txt", "b.txt"]);
    assert_eq!(named(&mut ui, "Size").selected, Some(true));
    click_at(&mut ui, size, Modifiers::default());
    assert_eq!(rows_top_down(&mut ui), vec!["alpha", "Zeta", "b.txt", "a.txt"]);
    let date = center(&named(&mut ui, "Modified"));
    click_at(&mut ui, date, Modifiers::default());
    assert_eq!(rows_top_down(&mut ui), vec!["alpha", "Zeta", "b.txt", "a.txt"]);
}

#[test]
fn the_breadcrumb_goes_back_and_backspace_goes_up() {
    let went = Rc::new(RefCell::new(Vec::new()));
    let out = went.clone();
    let mut ui = ui(FileBrowser::new(["Root", "2026", "October"], folder())
        .height(400.0)
        .on_navigate(move |depth| out.borrow_mut().push(depth)));
    let root = center(&named(&mut ui, "Root"));
    click_at(&mut ui, root, Modifiers::default());
    assert_eq!(*went.borrow(), vec![0]);
    tab_to(&mut ui, "Files");
    press(&mut ui, Key::Backspace, Modifiers::default());
    assert_eq!(*went.borrow(), vec![0, 1]);
}

#[test]
fn selection_opens_and_says_what_each_entry_is() {
    let selection = State::new(BTreeSet::<String>::new());
    let opened = Rc::new(RefCell::new(String::new()));
    let out = opened.clone();
    let mut ui = ui(FileBrowser::new(["Root"], folder())
        .height(400.0)
        .selection(selection.binding())
        .on_open(move |entry| *out.borrow_mut() = entry.name.clone()));
    let a = center(&named(&mut ui, "a.txt"));
    click_at(&mut ui, a, Modifiers::default());
    assert_eq!(selection.get(), BTreeSet::from(["first".to_owned()]), "by its id");
    let b = center(&named(&mut ui, "b.txt"));
    click_at(&mut ui, b, ctrl());
    assert_eq!(selection.get().len(), 2);
    let alpha = center(&named(&mut ui, "alpha"));
    click_at(&mut ui, alpha, shift());
    assert_eq!(selection.get().len(), 4, "from the anchor on b.txt up to alpha");
    assert_eq!(named(&mut ui, "alpha").description.as_deref(), Some("Folder"));
    assert_eq!(named(&mut ui, "b.txt").description.as_deref(), Some("2.0 KB"));
    tab_to(&mut ui, "Files");
    press(&mut ui, Key::Home, Modifiers::default());
    assert_eq!(selection.get(), BTreeSet::from(["alpha".to_owned()]));
    press(&mut ui, Key::Enter, Modifiers::default());
    assert_eq!(*opened.borrow(), "alpha");
}

#[test]
fn an_empty_folder_says_so_in_the_language_of_the_app() {
    let mut ui = ui(Localized::new(Labels::pt_br(), FileBrowser::new(["Raiz"], Vec::new()).height(300.0)));
    let words: String = ui.access_tree().nodes.iter().filter_map(|n| n.name.clone()).collect();
    assert!(words.contains("Esta pasta está vazia."), "{words}");
    assert!(has(&mut ui, "Nome"));
}

#[test]
fn a_browser_takes_the_room_of_its_parent() {
    let many: Vec<FileEntry> = (0..500).map(|i| FileEntry::file(format!("f{i:05}"), i)).collect();
    let mut ui = ui(FileBrowser::new(["Root"], many));
    assert!(has(&mut ui, "f00000"));
    wheel(&mut ui, Point::new(300.0, 300.0), Point::new(0.0, -6000.0));
    assert!(!has(&mut ui, "f00000"));
}

#[test]
fn many_files_stay_virtual() {
    let many: Vec<FileEntry> = (0..20_000).map(|i| FileEntry::file(format!("f{i:05}"), i)).collect();
    let mut ui = ui(FileBrowser::new(["Root"], many).height(400.0));
    assert!(ui.element_count() < 300, "{}", ui.element_count());
    assert!(has(&mut ui, "f00000"));
}

fn day(y: i32, m: u8, d: u8) -> Date {
    Date::new(y, m, d).unwrap()
}

#[test]
fn a_year_shows_every_day_and_its_marks() {
    let year = State::new(2026);
    let mut ui = ui(YearView::new(year.binding())
        .today(day(2026, 11, 5))
        .marks(|d| (d.month() == 3 && d.day() == 8).then(|| DayMark::new().label("Wedding"))));
    let days = of_role(&mut ui, Role::Button).len();
    assert_eq!(days, 365 + 2, "every day and two arrows");
    assert_eq!(of_role(&mut ui, Role::Calendar).len(), 12);
    assert_eq!(named(&mut ui, "8 March 2026").description.as_deref(), Some("Wedding"));
    assert_eq!(named(&mut ui, "5 November 2026").description.as_deref(), Some("Today"));
}

#[test]
fn a_mark_without_words_is_still_said() {
    let year = State::new(2026);
    let mut ui = ui(YearView::new(year.binding()).marks(|d| (d.day() == 2).then(DayMark::new)));
    assert_eq!(named(&mut ui, "2 January 2026").description.as_deref(), Some("Marked"));
}

#[test]
fn clicks_pick_a_day_and_the_arrows_change_the_year() {
    let (year, chosen) = (State::new(2026), State::new(None::<Date>));
    let picked = Rc::new(Cell::new(None));
    let out = picked.clone();
    let mut ui =
        ui(YearView::new(year.binding()).selected(chosen.binding()).on_pick(move |d| out.set(Some(d))));
    let target = center(&named(&mut ui, "10 February 2026"));
    click_at(&mut ui, target, Modifiers::default());
    assert_eq!(picked.get(), Some(day(2026, 2, 10)));
    assert_eq!(chosen.get(), Some(day(2026, 2, 10)));
    assert_eq!(named(&mut ui, "10 February 2026").selected, Some(true));
    let next = center(&named(&mut ui, "Next year"));
    click_at(&mut ui, next, Modifiers::default());
    assert_eq!(year.get(), 2027);
    assert!(has(&mut ui, "1 January 2027"));
}

#[test]
fn the_keyboard_walks_the_year_and_past_its_end() {
    let (year, chosen) = (State::new(2026), State::new(Some(day(2026, 12, 31))));
    let mut ui = ui(YearView::new(year.binding()).selected(chosen.binding()));
    tab_to(&mut ui, "31 December 2026");
    press(&mut ui, Key::ArrowLeft, Modifiers::default());
    assert_eq!(focused_name(&mut ui).as_deref(), Some("30 December 2026"));
    press(&mut ui, Key::PageUp, Modifiers::default());
    assert_eq!(focused_name(&mut ui).as_deref(), Some("30 November 2026"));
    press(&mut ui, Key::Enter, Modifiers::default());
    assert_eq!(chosen.get(), Some(day(2026, 11, 30)));
    press(&mut ui, Key::PageDown, Modifiers::default());
    press(&mut ui, Key::PageDown, Modifiers::default());
    assert_eq!(year.get(), 2027);
    assert_eq!(focused_name(&mut ui).as_deref(), Some("30 January 2027"));
}

#[test]
fn the_name_of_a_month_opens_it_when_asked() {
    let year = State::new(2026);
    let opened = Rc::new(Cell::new(None));
    let out = opened.clone();
    let mut ui = ui(YearView::new(year.binding()).on_month(move |d| out.set(Some(d))));
    let may = center(&named(&mut ui, "May"));
    click_at(&mut ui, may, Modifiers::default());
    assert_eq!(opened.get(), Some(day(2026, 5, 1)));
}

#[test]
fn as_many_months_go_on_a_line_as_fit() {
    let year = State::new(2026);
    let mut ui = ui(YearView::new(year.binding()));
    let same_line = |ui: &mut Ui| {
        let y = named(ui, "1 January 2026").bounds.y();
        ["1 February 2026", "1 March 2026", "1 April 2026"]
            .iter()
            .filter(|n| (named(ui, n).bounds.y() - y).abs() < 60.0)
            .count()
    };
    assert_eq!(same_line(&mut ui), 2, "three months at this width: {}", ui.element_count());
    ui.set_viewport(Size::new(460.0, 600.0), 1.0);
    settle(&mut ui);
    assert_eq!(same_line(&mut ui), 1, "two months at this width");
}
