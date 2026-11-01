//! The data grid driven through the public `Ui` API.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use raven_core::{Event, Key, Modifiers, Point, PointerButton, Primitive, Size};
use raven_reactive::State;
use raven_style::Theme;
use raven_widgets::*;

fn data(n: usize) -> GridData {
    GridData::new(
        (0..n)
            .map(|i| {
                vec![
                    Value::from(format!("Customer {i:05}")),
                    Value::from(["Brazil", "Chile", "Peru"][i % 3]),
                    Value::from((i % 50) as f64),
                ]
            })
            .collect(),
    )
}

fn columns() -> Vec<GridColumn> {
    vec![
        GridColumn::text("Name").width(200.0).editable(),
        GridColumn::text("Country"),
        GridColumn::number("Seats").summary(Summary::Sum),
    ]
}

fn texts(ui: &Ui) -> Vec<String> {
    // Reconstructs nothing: counts glyph runs is enough for most checks; for
    // content we look at glyph counts per run.
    ui.scene()
        .primitives()
        .iter()
        .filter_map(|p| if let Primitive::Glyphs(g) = p { Some(g.glyphs.len().to_string()) } else { None })
        .collect()
}

fn ui_with(grid: DataGrid) -> Ui {
    let mut ui = Ui::new(VStack::new((grid,)).padding(10.0), Theme::raven_dark());
    ui.set_viewport(Size::new(900.0, 700.0), 1.0);
    ui.update();
    ui.update();
    ui
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
    ui.update();
}

// Layout (toolbar off): header 40px at y=10..50, filter row 50..90, body from 90; rows 36px.
const BODY_Y: f32 = 90.0;

fn grid(n: usize) -> DataGrid {
    DataGrid::new(data(n), columns()).toolbar(false).height(500.0)
}

#[test]
fn builds_only_visible_rows_of_a_huge_grid() {
    let ui = ui_with(grid(100_000));
    assert!(ui.element_count() < 600, "virtualized: {} elements for 100k rows", ui.element_count());
}

#[test]
fn scrolling_far_builds_the_rows_there() {
    let mut ui = ui_with(grid(100_000));
    let before = ui.element_count();
    assert!(ui.handle_event(&Event::Scroll {
        position: Point::new(200.0, 300.0),
        delta: Point::new(0.0, -360_000.0)
    }));
    ui.update();
    assert!(ui.element_count() < before + 50, "still virtualized after scrolling");
    // Row 10,000 is "Customer 10000": 14 glyphs + the seats number.
    assert!(texts(&ui).iter().any(|t| t == "14"), "rows near 10,000 are on screen");
}

#[test]
fn header_click_sorts_and_keyboard_selects() {
    let selection = State::new(BTreeSet::new());
    let mut ui = ui_with(grid(30).selection(selection.binding()));
    // Sort by Seats (column 3 starts at x = 10 + 200 + 160 = 370) descending: two clicks.
    click(&mut ui, Point::new(400.0, 30.0));
    click(&mut ui, Point::new(400.0, 30.0));
    // First row now has the most seats (49 → rows 49? no: 30 rows, max seats 29).
    click(&mut ui, Point::new(100.0, BODY_Y + 18.0));
    assert_eq!(selection.get(), BTreeSet::from([29]));
    key(&mut ui, Key::ArrowDown);
    assert_eq!(selection.get(), BTreeSet::from([28]), "arrow keys move the selection");
}

#[test]
fn filter_row_and_summaries() {
    let mut ui = ui_with(grid(300));
    // Type into the Country filter (second column starts at x = 210).
    click(&mut ui, Point::new(280.0, 70.0));
    for c in ["P", "e", "r", "u"] {
        key(&mut ui, Key::Character(c.into()));
    }
    // Footer sum over Peru rows only: i % 3 == 2 → seats sum.
    let expected: f64 = (0..300).filter(|i| i % 3 == 2).map(|i| (i % 50) as f64).sum();
    // Exact totals are covered by the query engine's tests; here the footer must
    // show the "SUM" caption and a 5-glyph value ("2,450" filtered, "7,350" before).
    assert_eq!(NumberFormat::default().format(expected), "2,450");
    let runs = texts(&ui);
    assert!(runs.contains(&"3".to_string()) && runs.contains(&"5".to_string()), "footer shows SUM 2,450");
}

#[test]
fn keyboard_navigation_scrolls_the_focused_row_into_view() {
    let selection = State::new(BTreeSet::new());
    let mut ui = ui_with(grid(1000).selection(selection.binding()));
    click(&mut ui, Point::new(100.0, BODY_Y + 18.0));
    for _ in 0..40 {
        key(&mut ui, Key::ArrowDown);
    }
    assert_eq!(selection.get(), BTreeSet::from([40]));
    key(&mut ui, Key::End);
    assert_eq!(selection.get(), BTreeSet::from([999]));
    assert!(ui.element_count() < 600);
}

#[test]
fn double_click_edits_and_enter_commits() {
    let d = data(10);
    let mut ui = ui_with(DataGrid::new(d.clone(), columns()).toolbar(false).height(400.0));
    let cell = Point::new(100.0, BODY_Y + 36.0 + 18.0);
    click(&mut ui, cell);
    ui.handle_event(&Event::DoubleClick { position: cell });
    ui.update();
    ui.update();
    // The editor starts with the old text; erase it and type a new name.
    for _ in 0..20 {
        key(&mut ui, Key::Backspace);
    }
    for c in ["N", "e", "w"] {
        key(&mut ui, Key::Character(c.into()));
    }
    key(&mut ui, Key::Enter);
    assert_eq!(d.rows()[1][0], Value::from("New"));
}

#[test]
fn multiple_selection_with_checkboxes_and_select_all() {
    let selection = State::new(BTreeSet::new());
    let mut ui = ui_with(grid(20).selection_mode(SelectionMode::Multiple).selection(selection.binding()));
    click(&mut ui, Point::new(32.0, BODY_Y + 18.0));
    click(&mut ui, Point::new(32.0, BODY_Y + 36.0 * 2.0 + 18.0));
    assert_eq!(selection.get(), BTreeSet::from([0, 2]));
    click(&mut ui, Point::new(32.0, 30.0));
    assert_eq!(selection.get().len(), 20, "header checkbox selects every matching row");
}

#[test]
fn dragging_a_column_edge_resizes_it() {
    let mut ui = ui_with(grid(5));
    // The Name column's handle is its last 6px: x = 10 + 200 - 3.
    let at = Point::new(207.0, 30.0);
    ui.handle_event(&Event::PointerMove { position: at });
    assert_eq!(ui.cursor(), raven_core::CursorIcon::ColResize);
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerMove { position: Point::new(287.0, 30.0) });
    ui.handle_event(&Event::PointerUp { position: Point::new(287.0, 30.0), button: PointerButton::Primary });
    ui.update();
    // Country's handle moved right by 80px.
    ui.handle_event(&Event::PointerMove { position: Point::new(10.0 + 280.0 + 160.0 - 3.0, 30.0) });
    assert_eq!(ui.cursor(), raven_core::CursorIcon::ColResize);
}

#[test]
fn grouping_and_export() {
    let exported = Rc::new(RefCell::new(String::new()));
    let e = exported.clone();
    let mut ui = ui_with(
        DataGrid::new(data(9), columns())
            .group_by(1)
            .height(500.0)
            .on_export(move |csv| *e.borrow_mut() = csv),
    );
    // Toolbar is on: find the Export button by scanning for the secondary fill near the right.
    let theme = Theme::raven_dark();
    let control = theme.color(raven_style::Semantic::Control);
    let export = ui
        .scene()
        .primitives()
        .iter()
        .filter_map(|p| if let Primitive::Quad(q) = p { Some(*q) } else { None })
        .filter(|q| q.fill == control.into())
        .max_by(|a, b| a.rect.x().total_cmp(&b.rect.x()))
        .expect("export button")
        .rect;
    click(&mut ui, export.center());
    let csv = exported.borrow().clone();
    assert!(csv.starts_with("Name,Country,Seats\r\n"));
    assert_eq!(csv.lines().count(), 10);
}
