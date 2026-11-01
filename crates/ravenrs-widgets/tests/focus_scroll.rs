//! Keyboard focus scrolls the control it lands on into view; a click does not.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use ravenrs_core::{AccessNode, AccessTree, Event, Key, Modifiers, Point, PointerButton, Rect, Size};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::*;

const HEIGHT: f32 = 600.0;

fn ui(root: impl View + 'static) -> Ui {
    let mut ui = Ui::new(root, Theme::raven_dark());
    ui.set_viewport(Size::new(800.0, HEIGHT), 1.0);
    ui.update();
    ui
}

fn node<'a>(tree: &'a AccessTree, name: &str) -> &'a AccessNode {
    tree.nodes.iter().find(|n| n.name.as_deref() == Some(name)).expect(name)
}

fn bounds(ui: &mut Ui, name: &str) -> Rect {
    node(&ui.access_tree(), name).bounds
}

fn focused(ui: &mut Ui) -> Option<String> {
    let tree = ui.access_tree();
    tree.focus.and_then(|id| tree.get(id)).and_then(|n| n.name.clone())
}

fn press(ui: &mut Ui, key: Key, modifiers: Modifiers) {
    ui.handle_event(&Event::KeyDown { key, modifiers });
    ui.update();
    ui.update();
}

fn tab(ui: &mut Ui) {
    press(ui, Key::Tab, Modifiers::default());
}

fn wheel(ui: &mut Ui, at: Point, dy: f32) {
    ui.handle_event(&Event::Scroll { position: at, delta: Point::new(0.0, dy) });
    ui.update();
}

fn assert_shows(ui: &mut Ui, name: &str, top: f32, bottom: f32) {
    let b = bounds(ui, name);
    assert!(b.y() >= top && b.max_y() <= bottom, "{name} at {b:?}, view {top}..{bottom}");
}

fn buttons(prefix: &str, count: usize) -> Vec<Button> {
    (0..count).map(|i| Button::new(format!("{prefix} {i}")).on_click(|| {})).collect()
}

/// `Row 0..40` in the rows of a sidebar that fills the window.
fn long_sidebar() -> impl View {
    let rows: Vec<SidebarItem> =
        (0..40).map(|i| SidebarItem::new(IconName::House, format!("Row {i}"))).collect();
    HStack::new((Sidebar::new(rows),)).height(HEIGHT).align(ravenrs_layout::Align::Stretch)
}

#[test]
fn tab_scrolls_down_a_long_column_and_shift_tab_back_up() {
    let mut ui = ui(ScrollView::new(buttons("Item", 40)));
    assert!(bounds(&mut ui, "Item 39").y() > HEIGHT, "starts out of view");
    for i in 0..40 {
        tab(&mut ui);
        assert_eq!(focused(&mut ui), Some(format!("Item {i}")));
        assert_shows(&mut ui, &format!("Item {i}"), 0.0, HEIGHT);
    }
    let shift = Modifiers { shift: true, ..Modifiers::default() };
    for i in (0..39).rev() {
        press(&mut ui, Key::Tab, shift);
        assert_shows(&mut ui, &format!("Item {i}"), 0.0, HEIGHT);
    }
}

#[test]
fn a_control_that_already_shows_does_not_move_the_column() {
    let mut ui = ui(ScrollView::new(buttons("Item", 40)));
    let before = bounds(&mut ui, "Item 2");
    tab(&mut ui);
    tab(&mut ui);
    tab(&mut ui);
    assert_eq!(focused(&mut ui).as_deref(), Some("Item 2"));
    assert_eq!(bounds(&mut ui, "Item 2"), before);
}

#[test]
fn arrows_in_a_sidebar_keep_the_row_in_view() {
    let mut ui = ui(long_sidebar());
    tab(&mut ui);
    assert_eq!(focused(&mut ui).as_deref(), Some("Row 0"));
    press(&mut ui, Key::End, Modifiers::default());
    assert_eq!(focused(&mut ui).as_deref(), Some("Row 39"));
    assert_shows(&mut ui, "Row 39", 0.0, HEIGHT);
    press(&mut ui, Key::ArrowDown, Modifiers::default());
    assert_eq!(focused(&mut ui).as_deref(), Some("Row 0"), "wraps to the top");
    assert_shows(&mut ui, "Row 0", 0.0, HEIGHT);
    for i in 1..40 {
        press(&mut ui, Key::ArrowDown, Modifiers::default());
        assert_shows(&mut ui, &format!("Row {i}"), 0.0, HEIGHT);
    }
}

#[test]
fn tab_in_a_sidebar_reaches_rows_out_of_view() {
    let mut ui = ui(long_sidebar());
    for i in 0..40 {
        tab(&mut ui);
        assert_eq!(focused(&mut ui), Some(format!("Row {i}")));
        assert_shows(&mut ui, &format!("Row {i}"), 0.0, HEIGHT);
    }
}

#[test]
fn nested_containers_all_scroll_to_show_the_control() {
    let inner = ScrollView::new(buttons("Inner", 30)).height(200.0);
    let outer = ScrollView::new((VStack::new(buttons("Before", 30)), inner));
    let mut ui = ui(outer);
    for _ in 0..30 {
        tab(&mut ui);
    }
    assert_eq!(focused(&mut ui).as_deref(), Some("Before 29"));
    for i in 0..30 {
        tab(&mut ui);
        let name = format!("Inner {i}");
        assert_eq!(focused(&mut ui).as_ref(), Some(&name));
        // Inside the inner view, which starts right below the last outer row, and on screen.
        let top = bounds(&mut ui, "Before 29").max_y();
        assert_shows(&mut ui, &name, top, (top + 16.0 + 200.0).min(HEIGHT));
    }
    assert!(bounds(&mut ui, "Inner 0").y() < bounds(&mut ui, "Before 29").max_y(), "the inner view scrolled");
}

#[test]
fn a_click_on_a_partly_hidden_control_does_not_scroll() {
    const VIEW: f32 = 100.0;
    let mut ui = ui(ScrollView::new(buttons("Item", 40)).spacing(0.0).height(VIEW));
    let cut = (0..40)
        .map(|i| format!("Item {i}"))
        .find(|n| bounds(&mut ui, n).max_y() > VIEW)
        .expect("a control reaches past the bottom edge");
    let before = bounds(&mut ui, &cut);
    assert!(before.y() < VIEW, "and starts above it");
    let at = Point::new(before.center().x, (before.y() + VIEW) / 2.0);
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
    ui.update();
    assert_eq!(focused(&mut ui).as_deref(), Some(cut.as_str()), "the click focused it");
    assert_eq!(bounds(&mut ui, &cut), before, "without scrolling");
}

#[test]
fn the_wheel_can_leave_the_focused_control_behind() {
    let mut ui = ui(ScrollView::new(buttons("Item", 40)));
    tab(&mut ui);
    let before = bounds(&mut ui, "Item 0");
    wheel(&mut ui, before.center(), -400.0);
    ui.update();
    assert!(bounds(&mut ui, "Item 0").max_y() < 0.0, "focus does not pull the view back");
    assert_eq!(focused(&mut ui).as_deref(), Some("Item 0"));
    tab(&mut ui);
    assert_shows(&mut ui, "Item 1", 0.0, HEIGHT);
}

#[test]
fn focus_in_a_popover_leaves_the_page_behind_it_alone() {
    let open = State::new(true);
    let popover = Popover::new(
        open.binding(),
        Button::new("Anchor").on_click(|| {}),
        VStack::new(buttons("Inside", 6)),
    );
    // The popover reaches past the bottom of this short page.
    let mut ui = ui(ScrollView::new((popover, VStack::new(buttons("Item", 40)))).height(150.0));
    let before = bounds(&mut ui, "Item 0");
    press(&mut ui, Key::Tab, Modifiers { shift: true, ..Modifiers::default() });
    assert_eq!(focused(&mut ui).as_deref(), Some("Inside 5"));
    assert_eq!(bounds(&mut ui, "Item 0"), before);
}
