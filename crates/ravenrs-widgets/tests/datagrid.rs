//! The data grid driven through the public `Ui` API.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use ravenrs_core::{Event, Key, Modifiers, Point, PointerButton, Primitive, Size};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::*;

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
    // show a 5-glyph value ("2,450" filtered, "7,350" before).
    assert_eq!(NumberFormat::default().format(expected), "2,450");
    let runs = texts(&ui);
    assert!(runs.contains(&"5".to_string()), "footer shows 2,450");
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
    assert_eq!(ui.cursor(), ravenrs_core::CursorIcon::ColResize);
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerMove { position: Point::new(287.0, 30.0) });
    ui.handle_event(&Event::PointerUp { position: Point::new(287.0, 30.0), button: PointerButton::Primary });
    ui.update();
    // Country's handle moved right by 80px.
    ui.handle_event(&Event::PointerMove { position: Point::new(10.0 + 280.0 + 160.0 - 3.0, 30.0) });
    assert_eq!(ui.cursor(), ravenrs_core::CursorIcon::ColResize);
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
    let control = theme.color(ravenrs_style::Semantic::Control);
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

/// Where the last column's header starts in a wide window.
fn seats_at(columns: Vec<GridColumn>) -> f32 {
    let mut ui = Ui::new(DataGrid::new(data(3), columns), Theme::raven_light());
    ui.set_viewport(Size::new(1400.0, 600.0), 1.0);
    for _ in 0..3 {
        ui.update();
    }
    let tree = ui.access_tree();
    let header = tree.nodes.iter().find(|n| n.name.as_deref() == Some("Seats"));
    header.map_or(0.0, |n| n.bounds.x())
}

#[test]
fn a_filling_column_takes_what_the_others_leave() {
    let filling = vec![
        GridColumn::text("Name").width(200.0).fill(),
        GridColumn::text("Country"),
        GridColumn::number("Seats"),
    ];
    assert!(seats_at(filling) > seats_at(columns()) + 400.0);
}

#[test]
fn no_data_says_its_own_words_and_not_that_filters_left_nothing() {
    let mut ui = ui_with(DataGrid::new(GridData::new(Vec::new()), columns()).empty_text("Nothing yet"));
    let tree = ui.access_tree();
    assert!(tree.nodes.iter().any(|n| n.name.as_deref() == Some("Nothing yet")));
}

#[test]
fn a_filling_column_follows_a_wider_window() {
    let filling = vec![
        GridColumn::text("Name").width(200.0).fill(),
        GridColumn::text("Country"),
        GridColumn::number("Seats"),
    ];
    let mut ui = Ui::new(DataGrid::new(data(3), filling), Theme::raven_light());
    let seats = |ui: &mut Ui| {
        let tree = ui.access_tree();
        tree.nodes.iter().find(|n| n.name.as_deref() == Some("Seats")).map_or(0.0, |n| n.bounds.x())
    };
    ui.set_viewport(Size::new(800.0, 500.0), 1.0);
    for _ in 0..3 {
        ui.update();
    }
    let before = seats(&mut ui);
    ui.set_viewport(Size::new(1200.0, 500.0), 1.0);
    for _ in 0..3 {
        ui.update();
    }
    assert!(seats(&mut ui) > before + 300.0);
}

mod arranging {
    //! Grouping by dragging, moving columns, menus, scenarios, pages and rows on a server.
    use std::cell::{Cell, RefCell};
    use std::collections::BTreeSet;
    use std::rc::Rc;

    use ravenrs_core::{AccessAction, Event, Key, Modifiers, Point, PointerButton, Rect, Role, Size};
    use ravenrs_layout::Align;
    use ravenrs_reactive::State;
    use ravenrs_style::Theme;
    use ravenrs_widgets::*;

    fn rows() -> GridData {
        GridData::new(vec![
            vec!["Pro".into(), "BR".into(), 10.0.into()],
            vec!["Free".into(), "BR".into(), 20.0.into()],
            vec!["Pro".into(), "US".into(), 30.0.into()],
            vec!["Free".into(), "US".into(), 40.0.into()],
        ])
    }

    fn columns() -> Vec<GridColumn> {
        vec![
            GridColumn::text("Plan"),
            GridColumn::text("Country"),
            GridColumn::number("Seats").summary(Summary::Sum),
        ]
    }

    fn grid() -> DataGrid {
        DataGrid::new(rows(), columns()).height(500.0)
    }

    fn ui(view: impl View + 'static) -> Ui {
        let mut ui = Ui::new(VStack::new((view,)).align(Align::Stretch), Theme::raven_light());
        ui.set_viewport(Size::new(1000.0, 700.0), 1.0);
        settle(&mut ui);
        ui
    }

    fn settle(ui: &mut Ui) {
        ui.update();
        ui.update();
    }

    fn node(ui: &mut Ui, role: Role, name: &str) -> Option<Rect> {
        let tree = ui.access_tree();
        let found = tree.nodes.iter().find(|n| n.role == role && n.name.as_deref() == Some(name));
        found.map(|n| n.bounds)
    }

    fn header(ui: &mut Ui, name: &str) -> Rect {
        node(ui, Role::ColumnHeader, name).unwrap_or_else(|| panic!("no header {name}"))
    }

    fn says(ui: &mut Ui, name: &str) -> bool {
        ui.access_tree().nodes.iter().any(|n| n.name.as_deref().is_some_and(|t| t.contains(name)))
    }

    fn press(ui: &mut Ui, name: &str) {
        let tree = ui.access_tree();
        let id = tree
            .nodes
            .iter()
            .find(|n| n.name.as_deref().is_some_and(|t| t.starts_with(name)))
            .unwrap_or_else(|| panic!("no {name}"))
            .id;
        ui.perform_action(id, AccessAction::Click);
        settle(ui);
    }

    fn drag(ui: &mut Ui, from: Point, to: Point) {
        ui.handle_event(&Event::PointerMove { position: from });
        ui.handle_event(&Event::PointerDown { position: from, button: PointerButton::Primary });
        settle(ui);
        let middle = Point::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
        ui.handle_event(&Event::PointerMove { position: middle });
        settle(ui);
        ui.handle_event(&Event::PointerMove { position: to });
        settle(ui);
        ui.handle_event(&Event::PointerUp { position: to, button: PointerButton::Primary });
        settle(ui);
    }

    /// A place in the group panel, a little to the right of whatever is there.
    fn panel(ui: &mut Ui) -> Point {
        let first = ui_first_header(ui);
        let plan = header(ui, first);
        Point::new(plan.x() + 500.0, plan.y() - 20.0)
    }

    fn ui_first_header(ui: &mut Ui) -> &'static str {
        for name in ["Plan", "Country", "Seats"] {
            if node(ui, Role::ColumnHeader, name).is_some() {
                return name;
            }
        }
        panic!("no header")
    }

    #[test]
    fn dragging_a_column_to_the_panel_groups_by_it() {
        let mut ui = ui(grid().group_panel(true));
        assert!(says(&mut ui, "Drag a column here"));
        let from = header(&mut ui, "Plan").center();
        let to = panel(&mut ui);
        drag(&mut ui, from, to);
        assert!(says(&mut ui, "Grouped by Plan"), "the chip");
        assert!(says(&mut ui, "Plan: Free, 2"), "the groups");
        assert!(node(&mut ui, Role::ColumnHeader, "Plan").is_none(), "a grouped column shows in its groups");
    }

    #[test]
    fn a_click_on_a_header_still_sorts_and_a_drag_does_not() {
        let mut ui = ui(grid());
        let from = header(&mut ui, "Seats").center();
        let moved = Point::new(from.x - 200.0, from.y);
        drag(&mut ui, from, moved);
        let tree = ui.access_tree();
        let seats = tree.nodes.iter().find(|n| n.name.as_deref() == Some("Seats")).expect("the header");
        assert!(seats.description.as_deref().is_none_or(|d| !d.contains("ascending")), "not sorted");
        press(&mut ui, "Seats");
        let tree = ui.access_tree();
        let seats = tree.nodes.iter().find(|n| n.name.as_deref() == Some("Seats")).expect("the header");
        assert!(seats.description.is_some(), "sorted");
    }

    #[test]
    fn dragging_a_header_along_the_others_moves_the_column() {
        let mut ui = ui(grid());
        let (plan, seats) = (header(&mut ui, "Plan"), header(&mut ui, "Seats"));
        assert!(plan.x() < seats.x());
        let to = Point::new(plan.x() + 2.0, plan.center().y);
        drag(&mut ui, seats.center(), to);
        assert!(header(&mut ui, "Seats").x() < header(&mut ui, "Plan").x());
    }

    #[test]
    fn chips_change_places_by_dragging_and_leave_by_dragging_off() {
        let mut ui = ui(DataGrid::new(rows(), columns()).group_by(0).group_by(1).height(500.0));
        assert!(says(&mut ui, "Plan: Free"), "Plan is outermost");
        let plan = node(&mut ui, Role::Button, "Grouped by Plan").expect("a chip");
        let country = node(&mut ui, Role::Button, "Grouped by Country").expect("a chip");
        let to = Point::new(country.max_x() + 40.0, country.center().y);
        drag(&mut ui, plan.center(), to);
        let chips = (
            node(&mut ui, Role::Button, "Grouped by Plan").expect("a chip"),
            node(&mut ui, Role::Button, "Grouped by Country").expect("a chip"),
        );
        assert!(chips.1.x() < chips.0.x(), "Country came first");
        assert!(says(&mut ui, "Country: BR"), "and so did its groups");
        // Off the panel: no longer grouped by it.
        let first = header(&mut ui, "Seats");
        let off = Point::new(first.center().x, first.y() + 300.0);
        drag(&mut ui, chips.0.center(), off);
        assert!(!says(&mut ui, "Grouped by Plan"));
        assert!(says(&mut ui, "Grouped by Country"));
    }

    #[test]
    fn escape_lets_go_of_a_drag() {
        let mut ui = ui(grid());
        let from = header(&mut ui, "Plan").center();
        let to = panel(&mut ui);
        ui.handle_event(&Event::PointerMove { position: from });
        ui.handle_event(&Event::PointerDown { position: from, button: PointerButton::Primary });
        settle(&mut ui);
        ui.handle_event(&Event::PointerMove { position: to });
        settle(&mut ui);
        ui.handle_event(&Event::KeyDown { key: Key::Escape, modifiers: Modifiers::default() });
        settle(&mut ui);
        ui.handle_event(&Event::PointerUp { position: to, button: PointerButton::Primary });
        settle(&mut ui);
        assert!(!says(&mut ui, "Grouped by Plan"));
    }

    #[test]
    fn groups_can_all_close_and_open() {
        let mut ui = ui(DataGrid::new(rows(), columns()).group_by(0).height(500.0));
        let cells = |ui: &mut Ui| ui.access_tree().nodes.iter().filter(|n| n.role == Role::Cell).count();
        assert!(cells(&mut ui) > 0);
        press(&mut ui, "Collapse all");
        assert_eq!(cells(&mut ui), 0);
        press(&mut ui, "Expand all");
        assert!(cells(&mut ui) > 0);
    }

    #[test]
    fn a_right_click_opens_the_columns_menu() {
        let mut ui = ui(grid());
        let at = header(&mut ui, "Country").center();
        ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Secondary });
        settle(&mut ui);
        assert!(says(&mut ui, "Group by this column"));
        press(&mut ui, "Hide column");
        assert!(node(&mut ui, Role::ColumnHeader, "Country").is_none());
    }

    #[test]
    fn a_filter_by_value_keeps_what_was_checked() {
        let mut ui = ui(grid());
        let at = header(&mut ui, "Plan").center();
        ui.handle_event(&Event::PointerMove { position: at });
        settle(&mut ui);
        press(&mut ui, "Filter by value Plan");
        let tree = ui.access_tree();
        let pro = tree
            .nodes
            .iter()
            .find(|n| n.role == Role::CheckBox && n.name.as_deref() == Some("Pro"))
            .expect("the value")
            .id;
        ui.perform_action(pro, AccessAction::Click);
        settle(&mut ui);
        assert!(says(&mut ui, "In ('Pro')"), "the filter says it");
        assert!(says(&mut ui, "2 of 4"), "and two rows are left");
        press(&mut ui, "Clear filter");
        assert!(says(&mut ui, "4 of 4"));
    }

    #[test]
    fn a_pinned_column_stays_put_while_the_others_scroll() {
        let wide = || {
            vec![
                GridColumn::text("Plan").width(400.0).pinned(true),
                GridColumn::text("Country").width(600.0),
                GridColumn::number("Seats").width(600.0),
            ]
        };
        let mut ui = ui(DataGrid::new(rows(), wide()).height(500.0));
        let before = header(&mut ui, "Plan");
        let country = header(&mut ui, "Country");
        ui.handle_event(&Event::Scroll {
            position: Point::new(500.0, 300.0),
            delta: Point::new(-300.0, 0.0),
        });
        settle(&mut ui);
        assert_eq!(header(&mut ui, "Plan").x(), before.x());
        assert!(header(&mut ui, "Country").x() < country.x());
    }

    #[test]
    fn a_page_shows_only_its_rows_and_the_pager_moves_on() {
        let many = GridData::new((0..120).map(|n| vec![Value::Number(f64::from(n))]).collect());
        let mut ui = ui(DataGrid::new(many, [GridColumn::number("N")]).page_size(25).height(600.0));
        assert!(says(&mut ui, "1–25 of 120"));
        press(&mut ui, "Next");
        assert!(says(&mut ui, "26–50 of 120"));
        assert!(says(&mut ui, "Page 2 / 5"));
    }

    #[test]
    fn a_scenario_is_saved_and_comes_back() {
        let saved: State<Vec<GridScenario>> = State::new(Vec::new());
        let mut ui =
            ui(DataGrid::new(rows(), columns()).group_panel(true).scenarios(saved.binding()).height(500.0));
        let from = header(&mut ui, "Plan").center();
        let to = panel(&mut ui);
        drag(&mut ui, from, to);
        press(&mut ui, "Scenario");
        press(&mut ui, "Save scenario");
        for letter in ["M", "i", "n", "e"] {
            ui.handle_event(&Event::KeyDown {
                key: Key::Character(letter.to_owned()),
                modifiers: Modifiers::default(),
            });
            settle(&mut ui);
        }
        settle(&mut ui);
        let tree = ui.access_tree();
        let save = tree
            .nodes
            .iter()
            .find(|n| n.role == Role::Button && n.name.as_deref() == Some("Save"))
            .expect("the button")
            .id;
        ui.perform_action(save, AccessAction::Click);
        settle(&mut ui);
        assert_eq!(saved.get().len(), 1);
        assert_eq!(saved.get()[0].name, "Mine");
        assert_eq!(saved.get()[0].layout.group_by, ["Plan"]);
        // Away from it, and back.
        press(&mut ui, "Scenario");
        press(&mut ui, "Original layout");
        assert!(!says(&mut ui, "Grouped by Plan"));
        press(&mut ui, "Scenario");
        press(&mut ui, "Mine");
        assert!(says(&mut ui, "Grouped by Plan"));
    }

    #[test]
    fn a_layout_lives_outside_and_can_come_from_outside() {
        let kept: State<GridLayout> = State::new(GridLayout::default());
        let mut ui =
            ui(DataGrid::new(rows(), columns()).group_panel(true).layout(kept.binding()).height(500.0));
        settle(&mut ui);
        let from = header(&mut ui, "Plan").center();
        let to = panel(&mut ui);
        drag(&mut ui, from, to);
        assert_eq!(kept.get().group_by, ["Plan"]);
        let mut wanted = kept.get();
        wanted.group_by = vec!["Country".to_owned()];
        kept.set(wanted);
        settle(&mut ui);
        assert!(says(&mut ui, "Grouped by Country"));
    }

    #[test]
    fn rows_on_a_server_cost_only_what_is_on_the_screen() {
        let asked = Rc::new(Cell::new(0_usize));
        let count = asked.clone();
        let remote = RemoteRows::new(move |request, answer| {
            count.set(count.get() + 1);
            let rows = (request.offset..request.offset + request.limit)
                .map(|n| vec![Value::Number(n as f64)])
                .collect();
            answer.send(GridPage::new(rows, true));
        })
        .max_chunks(3);
        let peek = remote.clone();
        let mut ui = ui(DataGrid::remote(remote, [GridColumn::number("N")]).height(500.0));
        settle(&mut ui);
        assert!(asked.get() <= 2, "{} questions", asked.get());
        assert!(says(&mut ui, "rows"));
        assert!(!says(&mut ui, "Drag a column"), "grouping needs the rows at hand");
        let mut now = 0.0;
        for _ in 0..40 {
            ui.handle_event(&Event::Scroll {
                position: Point::new(300.0, 300.0),
                delta: Point::new(0.0, -800.0),
            });
            now += 1.0;
            ui.set_time(now);
            settle(&mut ui);
        }
        assert!(asked.get() > 2, "more came as it scrolled");
        assert!(peek.peak_rows() <= 300, "{} rows held", peek.peak_rows());
    }

    #[test]
    fn a_server_with_no_rows_says_what_it_was_given_to_say() {
        let asked = Rc::new(Cell::new(0_usize));
        let count = asked.clone();
        let remote = RemoteRows::new(move |_, answer| {
            count.set(count.get() + 1);
            answer.send(GridPage::new(Vec::new(), false));
        });
        let grid =
            DataGrid::remote(remote, [GridColumn::text("Name")]).empty_text("No classes yet.").height(300.0);
        let mut ui = ui(grid);
        settle(&mut ui);
        let names: Vec<_> = ui.access_tree().nodes.iter().filter_map(|n| n.name.clone()).collect();
        assert!(says(&mut ui, "No classes yet."), "{names:?} after {} questions", asked.get());
        // The chunk asked past the end came back empty and did not make room for 100 rows.
        assert!(!says(&mut ui, "100 rows"), "{names:?}");
    }

    #[test]
    fn a_page_of_rows_on_a_server_asks_only_for_that_page() {
        let seen: Rc<RefCell<BTreeSet<usize>>> = Rc::default();
        let log = seen.clone();
        let remote = RemoteRows::new(move |request, answer| {
            log.borrow_mut().insert(request.offset);
            let rows = (request.offset..request.offset + request.limit)
                .map(|n| vec![Value::Number(n as f64)])
                .collect();
            answer.send(GridPage::new(rows, true).total(100_000));
        })
        .chunk_size(50);
        let mut ui = ui(DataGrid::remote(remote, [GridColumn::number("N")]).page_size(50).height(500.0));
        settle(&mut ui);
        assert!(says(&mut ui, "1–50 of 100.000") || says(&mut ui, "1–50 of 100,000"));
        press(&mut ui, "Next");
        ui.set_time(5.0);
        settle(&mut ui);
        assert!(seen.borrow().contains(&50));
        assert!(!seen.borrow().iter().any(|o| *o > 200), "{:?}", seen.borrow());
    }
}

mod formulas {
    //! What a column sums up, changed while the grid runs.
    use ravenrs_core::{AccessAction, Event, Key, Modifiers, Role, Size};
    use ravenrs_layout::Align;
    use ravenrs_style::Theme;
    use ravenrs_widgets::*;

    fn rows() -> GridData {
        GridData::new(vec![
            vec!["Pro".into(), 10.0.into()],
            vec!["Free".into(), 20.0.into()],
            vec!["Pro".into(), 30.0.into()],
        ])
    }

    fn ui(view: impl View + 'static) -> Ui {
        let mut ui = Ui::new(VStack::new((view,)).align(Align::Stretch), Theme::raven_light());
        ui.set_viewport(Size::new(1000.0, 700.0), 1.0);
        ui.update();
        ui.update();
        ui
    }

    fn value_of(ui: &mut Ui, name: &str) -> Option<String> {
        let tree = ui.access_tree();
        tree.nodes.iter().find(|n| n.name.as_deref() == Some(name)).and_then(|n| n.value.clone())
    }

    fn press(ui: &mut Ui, role: Role, name: &str) {
        let tree = ui.access_tree();
        let id = tree
            .nodes
            .iter()
            .find(|n| n.role == role && n.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("no {name}"))
            .id;
        ui.perform_action(id, AccessAction::Click);
        ui.update();
        ui.update();
    }

    fn says(ui: &mut Ui, text: &str) -> bool {
        ui.access_tree().nodes.iter().any(|n| {
            [&n.name, &n.value, &n.description].iter().any(|t| t.as_deref().is_some_and(|t| t.contains(text)))
        })
    }

    fn columns() -> Vec<GridColumn> {
        vec![GridColumn::text("Plan"), GridColumn::number("Seats").summary(Summary::Sum)]
    }

    #[test]
    fn a_formula_given_in_code_shows_in_the_footer() {
        let columns =
            [GridColumn::text("Plan"), GridColumn::number("Seats").summary_formula("Sum([Seats]) / Count()")];
        let mut ui = ui(DataGrid::new(rows(), columns).height(400.0));
        assert_eq!(value_of(&mut ui, "Edit summary Seats").as_deref(), Some("20"));
    }

    #[test]
    fn a_formula_works_on_each_group_by_its_own_rows() {
        let columns =
            [GridColumn::text("Plan"), GridColumn::number("Seats").summary_formula("Max([Seats]) * 2")];
        let mut ui = ui(DataGrid::new(rows(), columns).group_by(0).height(400.0));
        assert!(says(&mut ui, "Formula Seats: 40"), "Free has only 20");
        assert!(says(&mut ui, "Formula Seats: 60"), "Pro goes up to 30");
        assert_eq!(value_of(&mut ui, "Edit summary Seats").as_deref(), Some("60"), "the footer, over all");
    }

    #[test]
    fn a_click_on_the_footer_changes_the_kind() {
        let mut ui = ui(DataGrid::new(rows(), columns()).height(400.0));
        assert_eq!(value_of(&mut ui, "Edit summary Seats").as_deref(), Some("60"));
        press(&mut ui, Role::Button, "Edit summary Seats");
        press(&mut ui, Role::RadioButton, "Avg");
        assert_eq!(value_of(&mut ui, "Edit summary Seats").as_deref(), Some("20"));
    }

    #[test]
    fn typing_a_formula_shows_what_it_comes_to_or_what_is_wrong() {
        let mut ui = ui(DataGrid::new(rows(), columns()).height(400.0));
        press(&mut ui, Role::Button, "Edit summary Seats");
        press(&mut ui, Role::RadioButton, "Formula");
        assert!(says(&mut ui, "Result: 60"), "it starts from the kind it had");
        for letter in [" ", "+", " ", "1"] {
            ui.handle_event(&Event::KeyDown {
                key: Key::Character(letter.to_owned()),
                modifiers: Modifiers::default(),
            });
            ui.update();
            ui.update();
        }
        assert!(says(&mut ui, "Result: 61"));
        ui.handle_event(&Event::KeyDown { key: Key::Enter, modifiers: Modifiers::default() });
        ui.update();
        ui.update();
        assert_eq!(value_of(&mut ui, "Edit summary Seats").as_deref(), Some("61"));
    }

    #[test]
    fn a_formula_with_a_column_that_does_not_exist_is_refused() {
        let mut ui = ui(DataGrid::new(rows(), columns()).height(400.0));
        press(&mut ui, Role::Button, "Edit summary Seats");
        press(&mut ui, Role::RadioButton, "Formula");
        for letter in ["[", "N", "o", "]"] {
            ui.handle_event(&Event::KeyDown {
                key: Key::Character(letter.to_owned()),
                modifiers: Modifiers::default(),
            });
            ui.update();
            ui.update();
        }
        assert!(says(&mut ui, "Error:"));
        ui.handle_event(&Event::KeyDown { key: Key::Enter, modifiers: Modifiers::default() });
        ui.update();
        ui.update();
        assert!(says(&mut ui, "Error:"), "it stays open, and the footer keeps the last good one");
    }
}

/// The quads of each row, by their top, whose fill is exactly this color.
fn tops_in(ui: &Ui, color: ravenrs_core::Color) -> Vec<f32> {
    let mut tops: Vec<f32> = ui
        .scene()
        .primitives()
        .iter()
        .filter_map(|p| match p {
            Primitive::Quad(q) if q.fill == color.into() && q.rect.width() > 100.0 => Some(q.rect.y()),
            _ => None,
        })
        .collect();
    tops.sort_by(f32::total_cmp);
    tops.dedup();
    tops
}

fn striped_ui(grid: DataGrid) -> Ui {
    let mut ui = Ui::new(grid.height(400.0), Theme::raven_light());
    ui.set_viewport(Size::new(800.0, 500.0), 1.0);
    ui.update();
    ui.update();
    ui
}

#[test]
fn striped_rows_tint_every_other_row_with_the_control_color() {
    let theme = Theme::raven_light();
    let tint = theme.color(ravenrs_style::Semantic::Control).faded(0.4);
    assert!(tops_in(&striped_ui(DataGrid::new(data(10), columns())), tint).is_empty(), "off by default");

    let tops = tops_in(&striped_ui(DataGrid::new(data(10), columns()).striped(true)), tint);
    assert!(tops.len() >= 4, "{tops:?}");
    let gap = tops[1] - tops[0];
    assert!(
        tops.windows(2).all(|pair| (pair[1] - pair[0] - gap).abs() < 0.5),
        "evenly, every other row: {tops:?}"
    );
}

#[test]
fn rows_can_have_their_own_colors_and_go_without_lines() {
    let (even, odd) =
        (ravenrs_core::Color::rgba(0.9, 0.5, 0.1, 1.0), ravenrs_core::Color::rgba(0.1, 0.5, 0.9, 1.0));
    let ui = striped_ui(DataGrid::new(data(10), columns()).even_row(even).odd_row(odd));
    let (evens, odds) = (tops_in(&ui, even), tops_in(&ui, odd));
    assert!(!evens.is_empty() && !odds.is_empty());
    assert!(odds[0] < evens[0], "the first row is an odd one");

    let line = Theme::raven_light().color(ravenrs_style::Semantic::Border).faded(0.6);
    let with = striped_ui(DataGrid::new(data(10), columns()));
    let without = striped_ui(DataGrid::new(data(10), columns()).horizontal_lines(false));
    assert!(tops_in(&without, line).len() < tops_in(&with, line).len());
}
