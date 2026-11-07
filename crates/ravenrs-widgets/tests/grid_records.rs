//! A data grid over records: keys, dates, edits that can be refused, buttons on rows.
#![allow(clippy::expect_used)]

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;

use ravenrs_core::{Event, Key, Modifiers, Point, PointerButton, Role, Size};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::grid::{GridQuery, Row, Value};
use ravenrs_widgets::*;

fn ui(view: impl View + 'static) -> Ui {
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(900.0, 600.0), 1.0);
    ui.update();
    ui
}

fn click(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
}

fn double_click(ui: &mut Ui, at: Point) {
    click(ui, at);
    ui.handle_event(&Event::DoubleClick { position: at });
    ui.update();
    ui.update();
}

fn key(ui: &mut Ui, key: Key) {
    ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers::default() });
    ui.update();
}

fn named(ui: &mut Ui, role: Role, name: &str) -> Option<Point> {
    let tree = ui.access_tree();
    tree.nodes.iter().find(|n| n.role == role && n.name.as_deref() == Some(name)).map(|n| n.bounds.center())
}

fn says(ui: &mut Ui, text: &str) -> bool {
    let tree = ui.access_tree();
    tree.nodes.iter().any(|n| n.name.as_deref() == Some(text))
}

fn data() -> GridData {
    GridData::new(vec![
        vec![Value::Number(10.0), "Acme".into(), 5.into()],
        vec![Value::Number(20.0), "Globex".into(), 7.into()],
        vec![Value::Number(30.0), "Initech".into(), 9.into()],
    ])
}

fn columns() -> Vec<GridColumn> {
    vec![
        GridColumn::number("Id").field("id").hidden(),
        GridColumn::text("Name").field("name"),
        GridColumn::number("Seats").field("seats").editable(),
    ]
}

fn cell_of(ui: &mut Ui, text: &str) -> Point {
    let tree = ui.access_tree();
    tree.nodes
        .iter()
        .find(|n| n.role == Role::Cell && n.name.as_deref() == Some(text))
        .map(|n| n.bounds.center())
        .expect("the cell")
}

#[test]
fn columns_know_their_field() {
    assert_eq!(GridColumn::text("Nome").field("name").field_name(), Some("name"));
    assert_eq!(GridColumn::text("Nome").field_name(), None);
}

#[test]
fn selected_keys_follow_the_record_when_the_rows_change() {
    let keys: State<BTreeSet<String>> = State::new(BTreeSet::new());
    let rows = data();
    let mut ui =
        ui(DataGrid::new(rows.clone(), columns()).key_column(0).selected_keys(keys.binding()).height(400.0));
    let spot = cell_of(&mut ui, "Globex");
    click(&mut ui, spot);
    assert_eq!(keys.get(), BTreeSet::from(["20".to_owned()]));

    rows.remove(0);
    ui.update();
    let tree = ui.access_tree();
    let selected = tree.nodes.iter().filter(|n| n.role == Role::Row && n.selected == Some(true)).count();
    assert_eq!(selected, 1, "the same record stays selected after a row above it goes");
    assert_eq!(keys.get(), BTreeSet::from(["20".to_owned()]));
}

#[test]
fn opening_a_row_gives_its_values() {
    let opened: Rc<RefCell<Option<Row>>> = Rc::default();
    let store = opened.clone();
    let mut ui = ui(DataGrid::new(data(), columns())
        .on_row_open(move |row| *store.borrow_mut() = Some(row.clone()))
        .height(400.0));
    let spot = cell_of(&mut ui, "Initech");
    double_click(&mut ui, spot);
    assert_eq!(opened.borrow().as_ref().and_then(|r| r.first().cloned()), Some(Value::Number(30.0)));
}

#[test]
fn dates_sort_by_day_and_read_in_expressions() {
    let day = |d| Value::Date(Date::new(2026, 11, d).expect("a real date"));
    let rows = GridData::new(vec![vec![day(20)], vec![day(3)], vec![day(11)]]);
    let cols = vec![GridColumn::date("Due")];
    let query = GridQuery { sort: vec![(0, SortDirection::Ascending)], ..GridQuery::default() };
    let result = grid::query::run_query(&cols, &rows.rows(), &query);
    assert_eq!(result.matched, [1, 2, 0]);
    let query = GridQuery { filters: vec!["[Due] > #2026-11-10#".to_owned()], ..GridQuery::default() };
    assert_eq!(grid::query::run_query(&cols, &rows.rows(), &query).matched.len(), 2);
}

#[test]
fn an_edit_can_be_refused_and_says_why() {
    let rows = data();
    let seen: Rc<RefCell<Vec<CellEdit>>> = Rc::default();
    let store = seen.clone();
    let mut ui = ui(DataGrid::new(rows.clone(), columns())
        .on_cell_edit(move |edit| {
            store.borrow_mut().push(edit.clone());
            if edit.value == Value::Number(99.0) { Err("Too many seats".to_owned()) } else { Ok(()) }
        })
        .height(400.0));
    let spot = cell_of(&mut ui, "5");
    double_click(&mut ui, spot);
    for _ in 0..3 {
        key(&mut ui, Key::Backspace);
    }
    key(&mut ui, Key::Character("99".to_owned()));
    key(&mut ui, Key::Enter);
    assert_eq!(rows.rows()[0][2], Value::Number(5.0), "refused, so it was not kept");
    assert!(says(&mut ui, "Too many seats"), "and it says why");
    for _ in 0..3 {
        key(&mut ui, Key::Backspace);
    }
    key(&mut ui, Key::Character("8".to_owned()));
    key(&mut ui, Key::Enter);
    assert_eq!(rows.rows()[0][2], Value::Number(8.0));
    assert!(!says(&mut ui, "Too many seats"), "the message goes with the problem");
    assert_eq!(seen.borrow().last().map(|e| (e.row, e.column)), Some((0, 2)));
}

#[test]
fn row_buttons_name_their_record_and_can_be_off() {
    let hit: Rc<RefCell<Vec<Value>>> = Rc::default();
    let store = hit.clone();
    let mut ui = ui(DataGrid::new(data(), columns())
        .row_action(
            GridRowAction::new("Delete", IconName::Trash2, move |row| {
                store.borrow_mut().push(row[0].clone())
            })
            .enabled(|row| row[0] != Value::Number(30.0)),
        )
        .height(400.0));
    let spot = named(&mut ui, Role::Button, "Delete Globex").expect("named after the row");
    click(&mut ui, spot);
    assert_eq!(*hit.borrow(), [Value::Number(20.0)]);
    let spot = named(&mut ui, Role::Button, "Delete Initech").expect("named too");
    click(&mut ui, spot);
    assert_eq!(hit.borrow().len(), 1, "this one is off");
}

#[test]
fn rows_can_be_added_and_deleted_from_the_grid() {
    let (added, removed) = (Rc::new(Cell::new(0)), Rc::new(RefCell::new(Vec::new())));
    let (a, r) = (added.clone(), removed.clone());
    let rows = data();
    let store = rows.clone();
    let mut ui = ui(DataGrid::new(rows, columns())
        .on_add_row(move || a.set(a.get() + 1))
        .on_delete_rows(move |picked: Vec<usize>| {
            for &row in picked.iter().rev() {
                store.remove(row);
            }
            r.borrow_mut().extend(picked);
        })
        .height(400.0));
    let spot = named(&mut ui, Role::Button, "Add").expect("the button");
    click(&mut ui, spot);
    assert_eq!(added.get(), 1);
    assert!(named(&mut ui, Role::Button, "Delete").is_none(), "nothing selected to delete");
    let spot = cell_of(&mut ui, "Acme");
    click(&mut ui, spot);
    let spot = named(&mut ui, Role::Button, "Delete").expect("now there is");
    click(&mut ui, spot);
    assert_eq!(*removed.borrow(), [0]);
}

#[test]
fn the_delete_key_asks_for_the_selected_rows_only_when_it_is_offered() {
    let removed = Rc::new(RefCell::new(Vec::new()));
    let r = removed.clone();
    let mut ui = ui(DataGrid::new(data(), columns())
        .on_delete_rows(move |picked| r.borrow_mut().extend(picked))
        .height(400.0));
    let spot = cell_of(&mut ui, "Globex");
    click(&mut ui, spot);
    key(&mut ui, Key::Delete);
    assert_eq!(*removed.borrow(), [1]);
}

#[test]
fn rows_can_be_inserted_and_removed_in_the_data() {
    let rows = data();
    rows.insert(1, vec![Value::Number(15.0), "New".into(), 1.into()]);
    assert_eq!(rows.rows()[1][0], Value::Number(15.0));
    assert_eq!(rows.remove(1).map(|r| r[0].clone()), Some(Value::Number(15.0)));
    assert_eq!(rows.remove(99), None);
    assert_eq!(rows.len(), 3);
}

#[test]
fn the_filter_builder_in_the_toolbar_filters_and_clears_with_the_others() {
    let data = GridData::new(vec![
        vec!["Ana".into(), 10.0.into()],
        vec!["Bia".into(), 20.0.into()],
        vec!["Caio".into(), 30.0.into()],
    ]);
    let layout = State::new(GridLayout::default());
    let grid = DataGrid::new(data, [GridColumn::text("Name"), GridColumn::number("Age")])
        .filter_builder(true)
        .layout(layout.binding());
    let mut ui = Ui::new(grid, ravenrs_style::Theme::raven_dark());
    ui.set_viewport(Size::new(1000.0, 700.0), 1.0);
    ui.update();
    let spot = {
        let tree = ui.access_tree();
        tree.nodes
            .iter()
            .find(|n| n.role == Role::Button && n.name.as_deref() == Some("Filter"))
            .expect("the button")
            .bounds
            .center()
    };
    for event in [
        Event::PointerMove { position: spot },
        Event::PointerDown { position: spot, button: PointerButton::Primary },
        Event::PointerUp { position: spot, button: PointerButton::Primary },
    ] {
        ui.handle_event(&event);
    }
    ui.update();
    assert!(ui.access_tree().nodes.iter().any(|n| n.role == Role::Dialog), "it opens");
}

#[test]
fn an_expression_in_a_filter_filters_the_whole_grid_from_any_column() {
    let columns = [GridColumn::text("Name"), GridColumn::number("Age")];
    let rows = vec![vec!["Ana".into(), 10.0.into()], vec!["Bia".into(), 20.0.into()]];
    let mut query = ravenrs_widgets::grid::GridQuery::default();
    query.set_filter(0, "[Age] > 15".to_owned());
    let result = ravenrs_widgets::grid::query::run_query(&columns, &rows, &query);
    assert_eq!(result.matched, [1]);
}

#[test]
fn rows_on_a_server_are_edited_from_the_keyboard() {
    let remote = RemoteRows::new(|request, answer| {
        let rows: Vec<Row> =
            (request.offset..request.offset + request.limit).map(|i| vec![format!("R{i}").into()]).collect();
        answer.send(GridPage::new(rows, true));
    });
    let edits = Rc::new(RefCell::new(Vec::new()));
    let log = edits.clone();
    let mut ui = ui(DataGrid::remote(remote, [GridColumn::text("Name").editable()])
        .on_cell_edit(move |edit| {
            log.borrow_mut().push((edit.row, edit.value.clone()));
            Ok(())
        })
        .height(400.0));
    for second in 1..4 {
        ui.set_time(f64::from(second));
        ui.update();
    }
    let spot = {
        let tree = ui.access_tree();
        tree.nodes.iter().find(|n| n.role == Role::Grid).expect("the grid").bounds.center()
    };
    click(&mut ui, spot);
    let press = |ui: &mut Ui, key: Key| {
        ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers::default() });
        ui.update();
    };
    press(&mut ui, Key::Home);
    press(&mut ui, Key::Enter);
    let editing =
        ui.access_tree().nodes.iter().any(|n| n.role == Role::TextInput && n.value.as_deref() == Some("R0"));
    assert!(editing, "Enter opens the cell with what it holds");
    press(&mut ui, Key::Character("!".into()));
    press(&mut ui, Key::Enter);
    assert_eq!(edits.borrow().as_slice(), [(0, Value::from("R0!"))]);
}

#[test]
fn a_cell_can_look_different_by_what_its_row_holds() {
    let data = GridData::new(vec![vec!["low".into(), 1.0.into()], vec!["high".into(), 9.0.into()]]);
    let danger = Theme::raven_dark().color(ravenrs_style::Semantic::Danger).with_alpha(0.14);
    let count = |styled: bool| {
        let mut grid =
            DataGrid::new(data.clone(), [GridColumn::text("Name"), GridColumn::number("N")]).height(300.0);
        if styled {
            grid = grid.cell_style(|row, column| {
                let high = row[1].as_f64().is_some_and(|n| n > 5.0);
                let fill = (high && column == 1).then_some(ravenrs_style::Semantic::Danger);
                GridCellStyle::new(None, fill, false)
            });
        }
        let ui = ui(grid);
        format!("{:?}", ui.scene()).matches(&format!("{danger:?}")).count()
    };
    assert_eq!(count(false), 0);
    assert_eq!(count(true), 1, "only the cell that fits");
}

#[test]
fn the_filter_line_opens_the_whole_filter_in_the_editor() {
    let data = GridData::new(vec![
        vec!["Rent".into(), "Home".into()],
        vec!["Power".into(), "Home".into()],
        vec!["Movies".into(), "Fun".into()],
    ]);
    let mut start = GridLayout::default();
    let mut filter = FilterLayout::default();
    filter.title = "Category".into();
    filter.text = "[Category] In ('Home')".into();
    start.filters.push(filter);
    let layout = State::new(start);
    let grid = DataGrid::new(data, [GridColumn::text("Name"), GridColumn::text("Category")])
        .layout(layout.binding());
    let mut ui = ui(grid);
    assert!(named(&mut ui, Role::Dialog, "Edit filter").is_none());
    let edit = named(&mut ui, Role::Button, "Edit filter").expect("the filter line offers the editor");
    click(&mut ui, edit);
    assert!(named(&mut ui, Role::Dialog, "Edit filter").is_some(), "it opens");
    let tree = ui.access_tree();
    let shown = |text: &str| {
        tree.nodes.iter().any(|n| n.name.as_deref() == Some(text) || n.value.as_deref() == Some(text))
    };
    assert!(shown("is any of"), "the list reads as a line, not as an expression");
    assert!(shown("Home"), "with what it picked");
}
