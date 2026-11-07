//! The spreadsheet driven as a person drives it: pointer, keyboard and toolbar, with no window.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use ravenrs_core::{AccessAction, Event, Key, Modifiers, Point, PointerButton, Primitive, Rect, Role, Size};
use ravenrs_spreadsheet::{Spreadsheet, SpreadsheetLabels, SpreadsheetState};
use ravenrs_style::Theme;
use ravenrs_widgets::{MemoryClipboard, Ui};
use serde_json::json;

/// Column and row sizes of a new sheet, and the header strips around the cells.
const COLUMN: f32 = 64.0;
const ROW: f32 = 20.0;
const ROW_HEADER: f32 = 48.0;
const HEADER: f32 = 24.0;

fn ui_for(state: &SpreadsheetState) -> Ui {
    let state = state.clone();
    let mut ui = Ui::new(
        ravenrs_widgets::view_fn(move || Spreadsheet::new(&state).labels(SpreadsheetLabels::pt_br())),
        Theme::raven_light(),
    );
    ui.set_clipboard(MemoryClipboard::default());
    ui.set_viewport(Size::new(1100.0, 700.0), 1.0);
    ui.update();
    ui.update();
    ui
}

fn grid_bounds(ui: &mut Ui) -> Rect {
    ui.access_tree().nodes.iter().find(|n| n.role == Role::Grid).expect("a grid").bounds
}

fn grid_value(ui: &mut Ui) -> String {
    ui.access_tree()
        .nodes
        .iter()
        .find(|n| n.role == Role::Grid)
        .and_then(|n| n.value.clone())
        .unwrap_or_default()
}

/// The middle of a cell, given by row and column from zero.
fn cell_point(ui: &mut Ui, row: u32, col: u32) -> Point {
    let b = grid_bounds(ui);
    Point::new(b.x() + ROW_HEADER + COLUMN * (col as f32 + 0.5), b.y() + HEADER + ROW * (row as f32 + 0.5))
}

fn click(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
    ui.update();
}

fn press_key(ui: &mut Ui, key: Key, modifiers: Modifiers) {
    ui.handle_event(&Event::KeyDown { key, modifiers });
    ui.update();
    ui.update();
}

fn type_text(ui: &mut Ui, text: &str) {
    for c in text.chars() {
        press_key(ui, Key::Character(c.to_string()), Modifiers::default());
    }
}

fn press(ui: &mut Ui, role: Role, name: &str) {
    let id = ui
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.role == role && n.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no {role:?} '{name}'"))
        .id;
    ui.perform_action(id, AccessAction::Click);
    ui.update();
    ui.update();
}

fn command() -> Modifiers {
    Modifiers { meta: true, ..Modifiers::default() }
}

#[test]
fn clicking_a_cell_selects_it_and_typing_enters_a_value() {
    let state = SpreadsheetState::new();
    let mut ui = ui_for(&state);
    assert!(grid_value(&mut ui).starts_with("A1"));
    let at = cell_point(&mut ui, 2, 1);
    click(&mut ui, at);
    assert_eq!(state.active_cell(), "B3");
    assert!(grid_value(&mut ui).starts_with("B3"));

    type_text(&mut ui, "42");
    press_key(&mut ui, Key::Enter, Modifiers::default());
    assert_eq!(state.cell_text("B3"), "42");
    assert_eq!(state.active_cell(), "B4", "Enter moves down");
}

#[test]
fn a_formula_typed_in_the_grid_recalculates() {
    let state = SpreadsheetState::new();
    state.set_cell("A1", "3").unwrap();
    state.set_cell("A2", "4").unwrap();
    let mut ui = ui_for(&state);
    let at = cell_point(&mut ui, 2, 0);
    click(&mut ui, at);
    type_text(&mut ui, "=A1*A2");
    press_key(&mut ui, Key::Enter, Modifiers::default());
    assert_eq!(state.cell_text("A3"), "12");
    assert_eq!(state.cell_input("A3"), "=A1*A2");

    state.set_cell("A1", "10").unwrap();
    ui.update();
    assert_eq!(state.cell_text("A3"), "40");
}

#[test]
fn escape_leaves_the_cell_as_it_was() {
    let state = SpreadsheetState::new();
    state.set_cell("A1", "antes").unwrap();
    let mut ui = ui_for(&state);
    let at = cell_point(&mut ui, 0, 0);
    click(&mut ui, at);
    type_text(&mut ui, "depois");
    press_key(&mut ui, Key::Escape, Modifiers::default());
    assert_eq!(state.cell_text("A1"), "antes");
    // The grid has the keyboard again.
    press_key(&mut ui, Key::ArrowRight, Modifiers::default());
    assert_eq!(state.active_cell(), "B1");
}

#[test]
fn arrows_move_and_shift_extends_the_selection() {
    let state = SpreadsheetState::new();
    let mut ui = ui_for(&state);
    let at = cell_point(&mut ui, 0, 0);
    click(&mut ui, at);
    press_key(&mut ui, Key::ArrowDown, Modifiers::default());
    press_key(&mut ui, Key::ArrowRight, Modifiers::default());
    assert_eq!(state.active_cell(), "B2");
    let shift = Modifiers { shift: true, ..Modifiers::default() };
    press_key(&mut ui, Key::ArrowRight, shift);
    press_key(&mut ui, Key::ArrowDown, shift);
    assert_eq!(state.selection(), "B2:C3");
    let to = cell_point(&mut ui, 4, 3);
    click_with_shift(&mut ui, to);
    assert_eq!(state.selection(), "B2:D5");
}

fn click_with_shift(ui: &mut Ui, at: Point) {
    // The modifiers of the latest input event are what a click reads.
    let shift = Modifiers { shift: true, ..Modifiers::default() };
    ui.handle_event(&Event::KeyDown { key: Key::Other("Shift".into()), modifiers: shift });
    click(ui, at);
}

#[test]
fn delete_clears_and_undo_brings_it_back() {
    let state = SpreadsheetState::new();
    state.set_cell("A1", "valor").unwrap();
    let mut ui = ui_for(&state);
    let at = cell_point(&mut ui, 0, 0);
    click(&mut ui, at);
    press_key(&mut ui, Key::Delete, Modifiers::default());
    assert_eq!(state.cell_text("A1"), "");
    press_key(&mut ui, Key::Character("z".into()), command());
    assert_eq!(state.cell_text("A1"), "valor");
    press(&mut ui, Role::Button, "Refazer");
    assert_eq!(state.cell_text("A1"), "");
    press(&mut ui, Role::Button, "Desfazer");
    assert_eq!(state.cell_text("A1"), "valor");
}

#[test]
fn copy_and_paste_go_through_the_clipboard() {
    let state = SpreadsheetState::new();
    state.set_cell("A1", "um").unwrap();
    state.set_cell("B1", "2").unwrap();
    let mut ui = ui_for(&state);
    let first = cell_point(&mut ui, 0, 0);
    click(&mut ui, first);
    state.select("A1:B1").unwrap();
    ui.update();
    press_key(&mut ui, Key::Character("c".into()), command());
    let at = cell_point(&mut ui, 3, 0);
    click(&mut ui, at);
    press_key(&mut ui, Key::Character("v".into()), command());
    assert_eq!(state.cell_text("A4"), "um");
    assert_eq!(state.cell_text("B4"), "2");
}

#[test]
fn toolbar_formats_the_selection() {
    let state = SpreadsheetState::new();
    state.set_cell("A1", "0.25").unwrap();
    let mut ui = ui_for(&state);
    press(&mut ui, Role::Button, "Negrito");
    let bold = state.read(|wb| {
        let sheet = wb.active().unwrap();
        wb.styles.get(sheet.style_id(ravenrs_sheet::base::CellRef::new(0, 0))).font.bold
    });
    assert!(bold);
    press(&mut ui, Role::Button, "Porcentagem");
    assert_eq!(state.cell_text("A1"), "25%");
}

#[test]
fn sheets_are_added_and_chosen_from_the_tabs() {
    let state = SpreadsheetState::new();
    let mut ui = ui_for(&state);
    press(&mut ui, Role::Button, "Nova planilha");
    assert_eq!(state.sheet_names().len(), 2);
    let first = state.sheet_names()[0].clone();
    press(&mut ui, Role::Button, &first);
    assert_eq!(state.active_sheet(), 0);
    state.set_cell("A1", "na primeira").unwrap();
    let second = state.sheet_names()[1].clone();
    press(&mut ui, Role::Button, &second);
    assert_eq!(state.active_sheet(), 1);
    assert_eq!(state.cell_text("A1"), "");
}

#[test]
fn the_formula_bar_edits_the_active_cell() {
    let state = SpreadsheetState::new();
    state.set_cell("C2", "=1+1").unwrap();
    let mut ui = ui_for(&state);
    let at = cell_point(&mut ui, 1, 2);
    click(&mut ui, at);
    let bar = ui
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.role == Role::TextInput && n.value.as_deref() == Some("=1+1"))
        .map(|n| n.id)
        .expect("the formula bar shows the formula");
    ui.perform_action(bar, AccessAction::Focus);
    ui.update();
    press_key(&mut ui, Key::Character("0".into()), Modifiers::default());
    press_key(&mut ui, Key::Enter, Modifiers::default());
    assert_eq!(state.cell_text("C2"), "11");
    assert_eq!(state.cell_input("C2"), "=1+10");
}

#[test]
fn values_are_painted_with_their_format_and_the_view_stays_small() {
    let state = SpreadsheetState::new();
    for row in 1..=2000 {
        state.set_cell(&format!("A{row}"), &row.to_string()).unwrap();
    }
    state.execute("home.numberFormat", json!({ "range": "B1", "code": "0.00" })).unwrap();
    state.set_cell("B1", "3.14159").unwrap();
    let ui = ui_for(&state);
    assert_eq!(state.cell_text("B1"), "3.14");
    assert!(ui.element_count() < 400, "only the visible window is built: {}", ui.element_count());
    let glyph_runs = ui.scene().primitives().iter().filter(|p| matches!(p, Primitive::Glyphs(_))).count();
    assert!(glyph_runs > 20, "headers and values are painted: {glyph_runs}");
}

#[test]
fn a_saved_workbook_opens_again_with_its_sheets() {
    let state = SpreadsheetState::new();
    state.set_cell("A1", "=SUM(2,3)").unwrap();
    state.execute("home.insertSheet", json!({ "name": "Resumo", "before": 9 })).unwrap();
    let bytes = state.to_xlsx().unwrap();
    let back = SpreadsheetState::open("orcamento.xlsx", &bytes).unwrap();
    assert_eq!(back.sheet_names().len(), 2);
    assert!(back.sheet_names().contains(&"Resumo".to_owned()));
    back.execute("sheet.activate", json!({ "sheet": 0 })).unwrap();
    assert_eq!(back.cell_text("A1"), "5");
    assert_eq!(back.cell_input("A1"), "=SUM(2,3)");
    assert!(SpreadsheetState::open("ruim.xlsx", b"PK\x03\x04 not a workbook").is_err());
}

#[test]
fn a_click_with_no_pointer_move_before_it_selects_that_cell() {
    let state = SpreadsheetState::new();
    state.set_cell("A1", "1").unwrap();
    state.set_cell("C3", "2").unwrap();
    let mut ui = ui_for(&state);
    let at = cell_point(&mut ui, 2, 2);
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
    assert_eq!(state.selection(), "C3");
}

#[test]
fn a_double_click_edits_the_cell_from_its_content() {
    let state = SpreadsheetState::new();
    state.set_cell("B2", "=2+2").unwrap();
    let mut ui = ui_for(&state);
    let at = cell_point(&mut ui, 1, 1);
    click(&mut ui, at);
    ui.handle_event(&Event::DoubleClick { position: at });
    ui.update();
    ui.update();
    press_key(&mut ui, Key::Character("0".into()), Modifiers::default());
    press_key(&mut ui, Key::Enter, Modifiers::default());
    assert_eq!(state.cell_input("B2"), "=2+20");
    assert_eq!(state.cell_text("B2"), "22");
}

#[test]
fn dragging_over_cells_selects_the_range() {
    let state = SpreadsheetState::new();
    let mut ui = ui_for(&state);
    let from = cell_point(&mut ui, 1, 1);
    let to = cell_point(&mut ui, 5, 3);
    ui.handle_event(&Event::PointerMove { position: from });
    ui.handle_event(&Event::PointerDown { position: from, button: PointerButton::Primary });
    ui.update();
    for step in 1..=4 {
        let t = step as f32 / 4.0;
        let at = Point::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
        ui.handle_event(&Event::PointerMove { position: at });
        ui.update();
    }
    ui.handle_event(&Event::PointerUp { position: to, button: PointerButton::Primary });
    ui.update();
    assert_eq!(state.selection(), "B2:D6");
    assert_eq!(state.active_cell(), "B2");
}
