//! Keyboard operation of single-choice inputs, and behavior of the smaller
//! components that the other suites do not reach.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_core::{Event, Key, Modifiers, Point, PointerButton, Primitive, Quad, Rect, Size};
use ravenrs_reactive::State;
use ravenrs_style::{Semantic, Theme};
use ravenrs_widgets::*;

fn ui(view: impl View + 'static) -> Ui {
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.update();
    ui
}

fn quads(ui: &Ui) -> Vec<Quad> {
    ui.scene()
        .primitives()
        .iter()
        .filter_map(|p| if let Primitive::Quad(q) = p { Some(*q) } else { None })
        .collect()
}

/// Pen positions of the first glyph of every text run, in paint order.
fn text_origins(ui: &Ui) -> Vec<Point> {
    ui.scene()
        .primitives()
        .iter()
        .filter_map(|p| match p {
            Primitive::Glyphs(run) => run.glyphs.first().map(|g| Point::new(g.x, g.y)),
            _ => None,
        })
        .collect()
}

fn click(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
}

fn key(ui: &mut Ui, key: Key) {
    ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers::default() });
    ui.update();
}

/// The focus ring is drawn as an accent-colored border around the focused element.
fn focus_ring(ui: &Ui) -> Option<Rect> {
    let accent = Theme::raven_dark().color(Semantic::FocusRing);
    quads(ui).into_iter().find(|q| q.border.is_some_and(|b| b.color == accent)).map(|q| q.rect)
}

#[test]
fn radio_group_arrows_move_selection_and_focus_with_wraparound() {
    let choice = State::new(0usize);
    let mut ui = ui(RadioGroup::new(choice.binding(), ["Starter", "Team", "Enterprise"]));
    key(&mut ui, Key::Tab);
    let first = focus_ring(&ui).expect("Tab shows a focus ring");
    key(&mut ui, Key::ArrowDown);
    assert_eq!(choice.get(), 1);
    let second = focus_ring(&ui).expect("the ring follows the selection");
    assert!(second.y() > first.y());
    key(&mut ui, Key::ArrowDown);
    assert_eq!(choice.get(), 2, "focus moved, so the next arrow continues from the new option");
    key(&mut ui, Key::ArrowDown);
    assert_eq!(choice.get(), 0, "wraps around");
    key(&mut ui, Key::End);
    assert_eq!(choice.get(), 2);
    key(&mut ui, Key::Home);
    assert_eq!(choice.get(), 0);
    key(&mut ui, Key::ArrowUp);
    assert_eq!(choice.get(), 2, "wraps backwards");
}

#[test]
fn segmented_control_arrows_and_click() {
    let choice = State::new(1usize);
    let mut ui = ui(SegmentedControl::new(choice.binding(), ["Day", "Week", "Month"]));
    let labels = text_origins(&ui);
    click(&mut ui, Point::new(labels[2].x + 4.0, labels[2].y - 4.0));
    assert_eq!(choice.get(), 2);
    key(&mut ui, Key::ArrowRight);
    assert_eq!(choice.get(), 0, "clicking focused the segment; Right wraps to the first");
    key(&mut ui, Key::ArrowLeft);
    assert_eq!(choice.get(), 2);
}

#[test]
fn stepper_buttons_and_keys_stay_within_range() {
    let value = State::new(1);
    let mut ui = ui(Stepper::new(value.binding()).range(0, 3));
    let buttons: Vec<Rect> =
        quads(&ui).into_iter().map(|q| q.rect).filter(|r| r.width() < 40.0 && r.height() > 20.0).collect();
    let (minus, plus) = (buttons.first().expect("minus button"), buttons.last().expect("plus button"));
    click(&mut ui, minus.center());
    click(&mut ui, minus.center());
    assert_eq!(value.get(), 0, "clamped at the minimum");
    click(&mut ui, plus.center());
    assert_eq!(value.get(), 1);
    // Reach the stepper itself with Tab: the arrow keys then step it.
    for _ in 0..3 {
        key(&mut ui, Key::ArrowUp);
        if value.get() != 1 {
            break;
        }
        key(&mut ui, Key::Tab);
    }
    assert_eq!(value.get(), 2, "ArrowUp steps once focused");
    key(&mut ui, Key::End);
    assert_eq!(value.get(), 3);
    key(&mut ui, Key::ArrowUp);
    assert_eq!(value.get(), 3, "clamped at the maximum");
    key(&mut ui, Key::Home);
    assert_eq!(value.get(), 0);
}

#[test]
fn icon_button_clicks_and_activates_from_the_keyboard() {
    let hits = Rc::new(Cell::new(0));
    let h = hits.clone();
    let mut ui = ui(IconButton::new(IconName::Settings).on_click(move || h.set(h.get() + 1)));
    let rect = quads(&ui).first().expect("button background").rect;
    click(&mut ui, rect.center());
    assert_eq!(hits.get(), 1);
    key(&mut ui, Key::Enter);
    key(&mut ui, Key::Space);
    assert_eq!(hits.get(), 3, "Enter and Space activate the focused button");
}

#[test]
fn label_shows_required_marker_and_hint() {
    let plain = ui(Label::new("Email"));
    let full = ui(Label::new("Email").required().hint("We never share it."));
    assert!(text_origins(&full).len() >= text_origins(&plain).len() + 2, "a marker and a hint line");
    let hint_y = text_origins(&full).iter().map(|p| p.y).fold(f32::MIN, f32::max);
    assert!(hint_y > text_origins(&plain)[0].y, "the hint sits below the label");
}

#[test]
fn aspect_ratio_derives_height_from_width() {
    let ui = ui(VStack::new((
        AspectRatio::new(16.0 / 9.0, Spacer::new().background(Semantic::Accent)).width(320.0),
    )));
    let accent = Theme::raven_dark().color(Semantic::Accent);
    let r = quads(&ui).into_iter().find(|q| q.fill == accent.into()).expect("content").rect;
    assert_eq!(r.width(), 320.0);
    assert!((r.height() - 180.0).abs() < 0.5, "16:9 at 320 wide is 180 tall, got {}", r.height());
}

#[test]
fn dividers_are_one_pixel_and_stretch() {
    let border = Theme::raven_dark().color(Semantic::Border);
    let h = ui(VStack::new((Divider::horizontal(),)).width(300.0));
    let line = quads(&h).into_iter().find(|q| q.fill == border.into()).expect("horizontal divider").rect;
    assert_eq!((line.width(), line.height()), (300.0, 1.0));
    let v = ui(HStack::new((Spacer::new().height(40.0), Divider::vertical())));
    let line = quads(&v).into_iter().find(|q| q.fill == border.into()).expect("vertical divider").rect;
    assert_eq!((line.width(), line.height()), (1.0, 40.0));
}

#[test]
fn popover_opens_from_its_binding_and_closes_on_escape() {
    let open = State::new(false);
    let mut ui = ui(Popover::new(open.binding(), Text::new("Anchor"), Text::new("Details")));
    let closed = text_origins(&ui).len();
    open.set(true);
    ui.update();
    assert_eq!(text_origins(&ui).len(), closed + 1, "the content is shown");
    key(&mut ui, Key::Escape);
    assert!(!open.get(), "Escape dismisses");
    assert_eq!(text_origins(&ui).len(), closed);
}

#[test]
fn menubar_opens_a_menu_and_runs_the_chosen_item() {
    let saved = Rc::new(Cell::new(false));
    let s = saved.clone();
    let mut ui = ui(Menubar::new()
        .menu("File")
        .item(MenuItem::new("Save").on_select(move || s.set(true)))
        .separator()
        .item(MenuItem::new("Close"))
        .menu("Edit")
        .item(MenuItem::new("Undo")));
    let titles = text_origins(&ui);
    assert_eq!(titles.len(), 2, "File and Edit");
    click(&mut ui, Point::new(titles[0].x + 4.0, titles[0].y - 4.0));
    let items = text_origins(&ui);
    assert_eq!(items.len(), 4, "the File menu adds Save and Close");
    let save = items[2];
    click(&mut ui, Point::new(save.x + 4.0, save.y - 4.0));
    assert!(saved.get());
    assert_eq!(text_origins(&ui).len(), 2, "choosing an item closes the menu");
}

fn date(y: i32, m: u8, d: u8) -> Date {
    Date::new(y, m, d).expect("valid date")
}

/// Presses Tab until the focus ring surrounds the accent-filled (selected) box.
fn tab_to_selected(ui: &mut Ui) {
    let accent = Theme::raven_dark().color(Semantic::Accent);
    for _ in 0..8 {
        key(ui, Key::Tab);
        let selected = quads(ui).into_iter().find(|q| q.fill == accent.into()).map(|q| q.rect);
        if let (Some(ring), Some(sel)) = (focus_ring(ui), selected)
            && ring.contains(sel.center())
            && ring.width() < sel.width() + 12.0
        {
            return;
        }
    }
    panic!("Tab never reached the selected item");
}

#[test]
fn tabs_open_as_arrow_keys_move_focus() {
    let tab = State::new(0usize);
    let mut ui = ui(Tabs::new(tab.binding())
        .tab("Overview", Text::new("one"))
        .tab("Activity", Text::new("two"))
        .tab("Settings", Text::new("three")));
    key(&mut ui, Key::Tab);
    key(&mut ui, Key::ArrowRight);
    assert_eq!(tab.get(), 1, "automatic activation");
    key(&mut ui, Key::ArrowRight);
    assert_eq!(tab.get(), 2, "focus followed, so the next arrow continues");
    key(&mut ui, Key::ArrowRight);
    assert_eq!(tab.get(), 0, "wraps");
    key(&mut ui, Key::End);
    assert_eq!(tab.get(), 2);
}

#[test]
fn toggle_group_arrows_move_focus_and_space_toggles() {
    let on = State::new(std::collections::BTreeSet::new());
    let mut ui = ui(ToggleGroup::multiple(
        on.binding(),
        [(Some(IconName::Bold), "Bold"), (Some(IconName::Italic), "Italic"), (None, "Underline")],
    ));
    key(&mut ui, Key::Tab);
    let first = focus_ring(&ui).expect("ring");
    key(&mut ui, Key::ArrowRight);
    assert!(on.get().is_empty(), "arrows only move focus in a toolbar");
    assert!(focus_ring(&ui).expect("ring").x() > first.x(), "{first:?} -> {:?}", focus_ring(&ui));
    key(&mut ui, Key::Space);
    assert_eq!(on.get().into_iter().collect::<Vec<_>>(), vec![1]);
}

#[test]
fn dropdown_menu_is_operable_from_the_keyboard() {
    let chosen = Rc::new(Cell::new(""));
    let (a, b) = (chosen.clone(), chosen.clone());
    let mut ui = ui(DropdownMenu::new("Actions")
        .item(MenuItem::new("Rename").on_select(move || a.set("rename")))
        .separator()
        .item(MenuItem::new("Duplicate").on_select(move || b.set("duplicate"))));
    key(&mut ui, Key::Tab);
    let trigger = focus_ring(&ui).expect("trigger focused");
    key(&mut ui, Key::Enter);
    let row = focus_ring(&ui).expect("the first item takes focus");
    assert!(row.y() > trigger.max_y(), "focus is in the open menu");
    key(&mut ui, Key::ArrowDown);
    key(&mut ui, Key::Enter);
    assert_eq!(chosen.get(), "duplicate", "the separator is skipped");
    assert_eq!(focus_ring(&ui), Some(trigger), "focus returns to the button");
}

#[test]
fn select_opens_with_arrows_on_the_current_option() {
    let choice = State::new(1usize);
    let mut ui = ui(Select::new(choice.binding(), ["Low", "Medium", "High"]));
    key(&mut ui, Key::Tab);
    let field = focus_ring(&ui).expect("field focused");
    key(&mut ui, Key::ArrowDown);
    let current = focus_ring(&ui).expect("an option is focused");
    assert!(current.y() > field.max_y());
    key(&mut ui, Key::ArrowDown);
    key(&mut ui, Key::Enter);
    assert_eq!(choice.get(), 2, "moved from Medium to High");
    assert_eq!(focus_ring(&ui), Some(field), "focus returns to the field");
}

#[test]
fn calendar_grid_is_one_tab_stop_with_arrow_and_page_keys() {
    let picked = State::new(Some(date(2026, 10, 15)));
    let mut ui = ui(Calendar::new(picked.binding(), date(2026, 10, 1)));
    tab_to_selected(&mut ui);
    key(&mut ui, Key::ArrowRight);
    key(&mut ui, Key::Enter);
    assert_eq!(picked.get(), Some(date(2026, 10, 16)));
    key(&mut ui, Key::ArrowDown);
    key(&mut ui, Key::Enter);
    assert_eq!(picked.get(), Some(date(2026, 10, 23)), "down is one week");
    key(&mut ui, Key::PageDown);
    key(&mut ui, Key::Space);
    assert_eq!(picked.get(), Some(date(2026, 11, 23)), "Page Down is one month");
    key(&mut ui, Key::Home);
    key(&mut ui, Key::Enter);
    assert_eq!(picked.get(), Some(date(2026, 11, 22)), "Home is the Sunday of that week");
    for _ in 0..4 {
        key(&mut ui, Key::ArrowUp);
    }
    key(&mut ui, Key::Enter);
    assert_eq!(picked.get(), Some(date(2026, 10, 25)), "crossing into the previous month shows it");
}

#[test]
fn dialog_traps_focus_and_returns_it_on_close() {
    let open = State::new(false);
    let opener = open.clone();
    let mut ui = ui(VStack::new((
        Button::new("Open").on_click(move || opener.set(true)),
        Dialog::new(open.binding(), "Delete file?")
            .action(Button::new("Cancel"))
            .action(Button::new("Delete")),
    )));
    key(&mut ui, Key::Tab);
    let button = focus_ring(&ui).expect("Open focused");
    key(&mut ui, Key::Enter);
    assert!(open.get());
    for _ in 0..6 {
        key(&mut ui, Key::Tab);
        let ring = focus_ring(&ui).expect("focus stays visible");
        assert_ne!(ring, button, "Tab never leaves the dialog");
    }
    key(&mut ui, Key::Escape);
    assert!(!open.get());
    key(&mut ui, Key::Enter);
    assert!(open.get(), "focus went back to Open, so Enter reopens");
}

#[test]
fn checkbox_and_switch_toggle_with_space() {
    let (a, b) = (State::new(false), State::new(false));
    let mut ui =
        ui(VStack::new((Checkbox::new("Remember me", a.binding()), Toggle::new(b.binding()).label("Wi-Fi"))));
    key(&mut ui, Key::Tab);
    key(&mut ui, Key::Space);
    assert!(a.get());
    key(&mut ui, Key::Tab);
    key(&mut ui, Key::Space);
    assert!(b.get());
    key(&mut ui, Key::Space);
    assert!(!b.get());
}
