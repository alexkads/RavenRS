//! Text selection, the clipboard, DataGrid copy and the menubar keyboard
//! pattern, driven through `Ui` as a user would.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::collections::BTreeSet;

use ravenrs_core::{AccessAction, Event, Key, Modifiers, Point, PointerButton, Role, Size};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::grid::{GridColumn, GridData, Value};
use ravenrs_widgets::*;

fn ui(view: impl View + 'static) -> Ui {
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.update();
    ui
}

fn press(ui: &mut Ui, key: Key, modifiers: Modifiers) {
    ui.handle_event(&Event::KeyDown { key, modifiers });
    ui.update();
}

fn plain(ui: &mut Ui, key: Key) {
    press(ui, key, Modifiers::default());
}

fn command(ui: &mut Ui, c: &str) {
    press(ui, Key::Character(c.into()), Modifiers { meta: true, ..Modifiers::default() });
}

fn typed(ui: &mut Ui, text: &str) {
    for c in text.chars() {
        let key = if c == ' ' { Key::Space } else { Key::Character(c.to_string()) };
        plain(ui, key);
    }
}

fn focused_name(ui: &mut Ui) -> Option<String> {
    ui.access_tree().focused().and_then(|n| n.name.clone())
}

fn field_frame(ui: &mut Ui) -> ravenrs_core::Rect {
    ui.access_tree().nodes.iter().find(|n| n.role == Role::TextInput).expect("text field").bounds
}

#[test]
fn keyboard_selection_copies_cuts_and_pastes() {
    let text = State::new(String::new());
    let mut ui = ui(TextField::new(text.binding()).placeholder("Note"));
    plain(&mut ui, Key::Tab);
    typed(&mut ui, "hello brave world");
    // Select "world" with Shift+Alt+Left, then copy it.
    let shift_word = Modifiers { shift: true, alt: true, ..Modifiers::default() };
    press(&mut ui, Key::ArrowLeft, shift_word);
    command(&mut ui, "c");
    assert_eq!(ui.clipboard().read().as_deref(), Some("world"));
    // Cut everything, then paste twice.
    command(&mut ui, "a");
    command(&mut ui, "x");
    assert_eq!(text.get(), "");
    assert_eq!(ui.clipboard().read().as_deref(), Some("hello brave world"));
    command(&mut ui, "v");
    typed(&mut ui, " ");
    command(&mut ui, "v");
    assert_eq!(text.get(), "hello brave world hello brave world");
}

#[test]
fn dragging_selects_and_double_click_selects_a_word() {
    let text = State::new("alpha beta gamma".to_owned());
    let mut ui = ui(TextField::new(text.binding()).width(400.0));
    let frame = field_frame(&mut ui);
    let y = frame.center().y;
    // From the far left edge of the text to well past its end.
    let (from, to) = (Point::new(frame.x() + 13.0, y), Point::new(frame.max_x() - 4.0, y));
    ui.handle_event(&Event::PointerMove { position: from });
    ui.handle_event(&Event::PointerDown { position: from, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerMove { position: to });
    ui.handle_event(&Event::PointerUp { position: to, button: PointerButton::Primary });
    ui.update();
    command(&mut ui, "c");
    assert_eq!(ui.clipboard().read().as_deref(), Some("alpha beta gamma"));

    // A double-click on the middle word selects just that word.
    let middle = Point::new(frame.x() + 12.0 + 55.0, y);
    for _ in 0..2 {
        ui.handle_event(&Event::PointerMove { position: middle });
        ui.handle_event(&Event::PointerDown { position: middle, button: PointerButton::Primary });
        ui.handle_event(&Event::PointerUp { position: middle, button: PointerButton::Primary });
    }
    ui.handle_event(&Event::DoubleClick { position: middle });
    ui.update();
    command(&mut ui, "c");
    assert_eq!(ui.clipboard().read().as_deref(), Some("beta"));
}

#[test]
fn password_fields_paste_but_never_copy() {
    let secret = State::new(String::new());
    let mut ui = ui(TextField::new(secret.binding()).secure());
    ui.clipboard().write("s3cret");
    plain(&mut ui, Key::Tab);
    command(&mut ui, "v");
    assert_eq!(secret.get(), "s3cret");
    ui.clipboard().write("unchanged");
    command(&mut ui, "a");
    command(&mut ui, "c");
    assert_eq!(ui.clipboard().read().as_deref(), Some("unchanged"));
}

fn grid_data() -> GridData {
    GridData::new(
        (0..40)
            .map(|i| {
                vec![
                    Value::from(format!("Customer {i}")),
                    Value::from(["Brazil", "Chile"][i % 2]),
                    Value::from(i as f64),
                ]
            })
            .collect(),
    )
}

fn columns() -> Vec<GridColumn> {
    vec![GridColumn::text("Name"), GridColumn::text("Country"), GridColumn::number("Seats")]
}

fn focus_grid(ui: &mut Ui) {
    let grid = ui.access_tree().nodes.iter().find(|n| n.role == Role::Grid).expect("grid").id;
    ui.perform_action(grid, AccessAction::Focus);
    ui.update();
}

#[test]
fn data_grid_copies_selected_rows_as_tab_separated_text() {
    let selection = State::new(BTreeSet::from([3usize, 1]));
    let mut ui = ui(DataGrid::new(grid_data(), columns())
        .toolbar(false)
        .height(400.0)
        .selection_mode(SelectionMode::Multiple)
        .selection(selection.binding()));
    focus_grid(&mut ui);
    command(&mut ui, "c");
    assert_eq!(ui.clipboard().read().as_deref(), Some("Customer 1\tChile\t1\nCustomer 3\tChile\t3"));
    command(&mut ui, "a");
    assert_eq!(selection.get().len(), 40, "Cmd/Ctrl+A selects every row");
}

#[test]
fn data_grid_copies_the_keyboard_row_without_a_selection_and_pages() {
    let mut ui = ui(DataGrid::new(grid_data(), columns())
        .toolbar(false)
        .height(400.0)
        .selection_mode(SelectionMode::None));
    focus_grid(&mut ui);
    plain(&mut ui, Key::PageDown);
    command(&mut ui, "c");
    assert_eq!(ui.clipboard().read().as_deref(), Some("Customer 15\tChile\t15"), "Page Down moves 15 rows");
    plain(&mut ui, Key::PageUp);
    command(&mut ui, "c");
    assert_eq!(ui.clipboard().read().as_deref(), Some("Customer 0\tBrazil\t0"));
}

#[test]
fn menubar_arrows_move_between_titles_and_open_menus() {
    let mut ui = ui(Menubar::new()
        .menu("File")
        .item(MenuItem::new("New"))
        .menu("Edit")
        .item(MenuItem::new("Undo"))
        .menu("View")
        .item(MenuItem::new("Zoom")));
    plain(&mut ui, Key::Tab);
    assert_eq!(focused_name(&mut ui).as_deref(), Some("File"));
    plain(&mut ui, Key::ArrowRight);
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Edit"));
    plain(&mut ui, Key::ArrowDown);
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Undo"), "Down opens the menu on its first item");
    plain(&mut ui, Key::ArrowRight);
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Zoom"), "Right opens the next menu");
    plain(&mut ui, Key::ArrowRight);
    assert_eq!(focused_name(&mut ui).as_deref(), Some("New"), "and wraps");
    plain(&mut ui, Key::ArrowLeft);
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Zoom"));
    plain(&mut ui, Key::Escape);
    assert_eq!(focused_name(&mut ui).as_deref(), Some("View"), "Escape returns to the title");
    assert!(ui.access_tree().nodes.iter().all(|n| n.role != Role::Menu), "the menu closed");
}
