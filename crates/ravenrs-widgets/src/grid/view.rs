//! The data grid view: toolbar, group panel, header, filter row, virtualized
//! body and footer.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Color, CursorIcon, Insets, Key, Point, Quad, Rect, Role};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Theme, Weight};

use super::layout::{self, Arrangement, GridLayout, GridScenario};
use super::model::{CellAlign, GridColumn, GridData, Row, SortDirection, Summary, Value};
use super::query::{
    GridQuery, QueryResult, ViewRow, check_formula, formula_value, group_ranges, run_query, to_csv, to_tsv,
};
use super::remote::RemoteRows;
use crate::context::{BuildCx, DragPhase, NodeId, Paint, Placement, QuadPaint, Semantics};
use crate::overlay::{Entry, MenuList, PanelOf};
use crate::{
    Badge, Button, ButtonSize, ButtonStyle, Dialog, DropdownMenu, IconName, MenuItem, Pagination, Select,
    Spacer, TextField, View, util,
};

/// How rows can be selected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum SelectionMode {
    /// Rows cannot be selected.
    None,
    /// One row at a time; arrow keys move the selection.
    #[default]
    Single,
    /// Check boxes; Ctrl/Cmd-click toggles, Shift-click selects a range.
    Multiple,
}

/// A virtualized data grid in the spirit of enterprise grids: sorting by one or
/// several columns, a filter row with operators and filters by value, a search panel,
/// grouping by dragging columns to the group panel, group summaries, footer totals,
/// columns to resize, reorder, pin and hide, a menu on each column, single or multiple
/// selection, keyboard navigation, in-place editing, cell templates, CSV export, pages,
/// saved scenarios, and rows that stay on a server ([`DataGrid::remote`]). Only the
/// visible rows are built, so 100,000+ rows stay smooth.
///
/// ```
/// use ravenrs_widgets::{DataGrid, GridColumn, GridData, SortDirection, Summary, Ui};
///
/// let data = GridData::new(vec![
///     vec!["Acme".into(), 1200.0.into()],
///     vec!["Globex".into(), 860.5.into()],
/// ]);
/// let grid = DataGrid::new(
///     data,
///     [GridColumn::text("Customer"), GridColumn::currency("MRR", "$", 2).summary(Summary::Sum)],
/// )
/// .sort_by(1, SortDirection::Descending)
/// .height(320.0);
///
/// let mut ui = Ui::new(grid, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct DataGrid {
    empty_text: Option<String>,
    data: GridData,
    columns: Vec<GridColumn>,
    selection_mode: SelectionMode,
    selection: Option<Binding<BTreeSet<usize>>>,
    initial: GridQuery,
    filter_row: bool,
    search_panel: bool,
    toolbar: bool,
    footer: bool,
    height: Option<f32>,
    row_height: f32,
    striped: bool,
    on_export: Option<Rc<dyn Fn(String)>>,
    on_row_activate: Option<Rc<dyn Fn(usize)>>,
    remote: Option<RemoteRows>,
    group_panel: Option<bool>,
    page_size: Option<usize>,
    vertical_lines: bool,
    layout: Option<Binding<GridLayout>>,
    scenarios: Option<Binding<Vec<GridScenario>>>,
}

impl fmt::Debug for DataGrid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DataGrid")
            .field("columns", &self.columns)
            .field("rows", &self.data.len())
            .finish_non_exhaustive()
    }
}

impl DataGrid {
    /// What it says when there is no data at all, as opposed to filters leaving nothing.
    pub fn empty_text(mut self, text: impl Into<String>) -> Self {
        self.empty_text = Some(text.into());
        self
    }

    /// A grid over `data` with these columns; single selection, filter row, toolbar and footer on.
    pub fn new(data: GridData, columns: impl IntoIterator<Item = GridColumn>) -> Self {
        Self {
            empty_text: None,
            data,
            columns: columns.into_iter().collect(),
            selection_mode: SelectionMode::Single,
            selection: None,
            initial: GridQuery::default(),
            filter_row: true,
            search_panel: true,
            toolbar: true,
            footer: true,
            height: None,
            row_height: 36.0,
            striped: false,
            on_export: None,
            on_row_activate: None,
            remote: None,
            group_panel: None,
            page_size: None,
            vertical_lines: false,
            layout: None,
            scenarios: None,
        }
    }

    /// A grid over rows that stay on a server. It asks for what is on the screen, keeps
    /// a limited number of rows, and sorting, filtering and searching are up to the
    /// loader. It scrolls on as rows arrive, or in pages with
    /// [`page_size`](Self::page_size); grouping needs the rows at hand, so it is off.
    pub fn remote(rows: RemoteRows, columns: impl IntoIterator<Item = GridColumn>) -> Self {
        let mut grid = Self::new(GridData::new(Vec::new()), columns);
        grid.remote = Some(rows);
        grid.group_panel = Some(false);
        grid
    }

    /// Shows the rows a page at a time, with a pager at the bottom.
    pub fn page_size(mut self, rows: usize) -> Self {
        self.page_size = Some(rows.max(1));
        self
    }

    /// Shows or hides the space where columns are dropped to group by them. It shows
    /// along with the toolbar, unless this says otherwise.
    pub fn group_panel(mut self, show: bool) -> Self {
        self.group_panel = Some(show);
        self
    }

    /// Draws a line between columns.
    pub fn vertical_lines(mut self, show: bool) -> Self {
        self.vertical_lines = show;
        self
    }

    /// Where its arrangement lives: read when it changes, and written when someone moves
    /// something. Keep this to bring it back as it was.
    pub fn layout(mut self, layout: Binding<GridLayout>) -> Self {
        self.layout = Some(layout);
        self
    }

    /// Offers saving its arrangement under a name and coming back to it. Keep the list
    /// with [`GridScenario::list_to_json`].
    pub fn scenarios(mut self, scenarios: Binding<Vec<GridScenario>>) -> Self {
        self.scenarios = Some(scenarios);
        self
    }

    /// Sets how rows can be selected (single by default).
    pub fn selection_mode(mut self, mode: SelectionMode) -> Self {
        self.selection_mode = mode;
        self
    }

    /// Where the selected row indices live (otherwise the grid keeps them).
    pub fn selection(mut self, selection: Binding<BTreeSet<usize>>) -> Self {
        self.selection = Some(selection);
        self
    }

    /// Initial sort; call again to add more sort columns.
    pub fn sort_by(mut self, column: usize, direction: SortDirection) -> Self {
        self.initial.sort.push((column, direction));
        self
    }

    /// Initial grouping; call again for nested groups.
    pub fn group_by(mut self, column: usize) -> Self {
        self.initial.group_by.push(column);
        self
    }

    /// Shows or hides the filter row under the header.
    pub fn filter_row(mut self, show: bool) -> Self {
        self.filter_row = show;
        self
    }

    /// Shows or hides the search box in the toolbar.
    pub fn search_panel(mut self, show: bool) -> Self {
        self.search_panel = show;
        self
    }

    /// The toolbar with search, grouping, column chooser and export.
    pub fn toolbar(mut self, show: bool) -> Self {
        self.toolbar = show;
        self
    }

    /// The footer with column totals.
    pub fn footer(mut self, show: bool) -> Self {
        self.footer = show;
        self
    }

    /// A fixed height; otherwise the grid fills the space its parent gives it.
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// Sets the row height in logical pixels (36 by default, at least 24).
    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = height.max(24.0);
        self
    }

    /// Shades every other row.
    pub fn striped(mut self, striped: bool) -> Self {
        self.striped = striped;
        self
    }

    /// Shows an Export button; `handler` receives the matching rows as CSV.
    pub fn on_export(mut self, handler: impl Fn(String) + 'static) -> Self {
        self.on_export = Some(Rc::new(handler));
        self
    }

    /// Double-click or Enter on a row whose cell is not editable.
    pub fn on_row_activate(mut self, handler: impl Fn(usize) + 'static) -> Self {
        self.on_row_activate = Some(Rc::new(handler));
        self
    }
}

struct Cached {
    summaries: Vec<Option<Summary>>,
    query: GridQuery,
    rows: Rc<Vec<Row>>,
    result: Rc<QueryResult>,
}

/// Interaction state the grid keeps across rebuilds. Cells written from event
/// handlers (and read during the next build) use `Cell`s, so writing them does
/// not trigger an extra rebuild.
#[derive(Clone)]
struct Handles {
    query: State<GridQuery>,
    widths: State<Vec<f32>>,
    selection: Binding<BTreeSet<usize>>,
    focus_row: State<Option<usize>>,
    anchor: Rc<Cell<Option<usize>>>,
    reveal: Rc<Cell<Option<usize>>>,
    editing: State<Option<(usize, usize)>>,
    edit_buffer: State<String>,
    resize: Rc<Cell<Option<(usize, f32, f32)>>>,
    modifiers: Rc<Cell<ravenrs_core::Modifiers>>,
    arrange: State<Arrangement>,
    drag: State<Option<GridDrag>>,
    /// Where a press started, and whether it has become a drag.
    press: Rc<Cell<(Point, bool)>>,
    /// A drag just ended, so the click that comes with letting go does not count.
    dragged: Rc<Cell<bool>>,
    zones: Rc<RefCell<Zones>>,
    menu: State<Option<(usize, Option<Point>)>>,
    filter_open: State<Option<usize>>,
    filter_chosen: State<BTreeSet<String>>,
    filter_search: State<String>,
    page: State<usize>,
    saving: State<bool>,
    scenario_name: State<String>,
    active_scenario: State<Option<String>>,
    /// The layout as it first was, to come back to.
    original: Rc<RefCell<Option<GridLayout>>>,
    seen_layout: Rc<RefCell<Option<GridLayout>>>,
    /// Which column's summary is being edited.
    summary_edit: State<Option<usize>>,
    summary_draft: State<String>,
    /// What formulas came to, by column and range of rows, until the rows change.
    formula_cache: Rc<RefCell<FormulaCache>>,
}

#[derive(Default)]
struct FormulaCache {
    /// What the answers are about: the result of the query and the formulas.
    about: (usize, Vec<Option<String>>),
    answers: BTreeMap<(usize, usize, usize), String>,
}

/// Everything a row needs to build itself.
struct Shared<'a> {
    grid: &'a DataGrid,
    rows: &'a [Row],
    rows_rc: Rc<Vec<Row>>,
    result: Rc<QueryResult>,
    visible: &'a [usize],
    widths: &'a [f32],
    total_width: f32,
    select_width: f32,
    selected: &'a BTreeSet<usize>,
    focus_row: Option<usize>,
    editing: Option<(usize, usize)>,
    h: Handles,
    arrangement: &'a Arrangement,
    /// Pinned columns and where each sits from the left of the grid.
    fixed: Vec<(usize, f32)>,
    last_fixed: Option<usize>,
    /// How wide the columns that stay in place are, with the check boxes.
    fixed_width: f32,
    offset_x: f32,
    /// How far the first column of a data row goes in, under its groups.
    indent: f32,
    /// Title and whether it groups, for what a drag needs after the build.
    column_facts: Rc<Vec<(String, bool)>>,
    /// Where a dragged column would land.
    drop_before: Option<Option<usize>>,
    /// Where column lines go, from the left of the row.
    lines: Rc<Vec<f32>>,
    /// Where each visible row's group starts in the matching rows, and how long it is.
    ranges: Rc<Vec<(usize, usize)>>,
}

impl Shared<'_> {
    fn fixed_x(&self, column: usize) -> Option<f32> {
        self.fixed.iter().find(|(c, _)| *c == column).map(|(_, x)| *x)
    }

    /// What a formula comes to over a stretch of the matching rows.
    fn formula_text(&self, c: usize, range: (usize, usize)) -> Option<String> {
        let formula = self.arrangement.formulas.get(c)?.as_ref()?;
        let mut cache = self.h.formula_cache.borrow_mut();
        let key = (c, range.0, range.1);
        if let Some(known) = cache.answers.get(&key) {
            return Some(known.clone());
        }
        let columns = &self.grid.columns;
        let end = (range.0 + range.1).min(self.result.matched.len());
        let indices = &self.result.matched[range.0.min(end)..end];
        let answer = check_formula(columns, formula)
            .and_then(|parsed| formula_value(columns, self.rows, indices, &parsed))
            .map_or_else(|_| "#!".to_owned(), |value| formula_display(&columns[c], &value));
        cache.answers.insert(key, answer.clone());
        Some(answer)
    }

    /// Where a column starts in the row.
    fn x_of(&self, column: usize) -> f32 {
        let mut x = self.select_width;
        for &c in self.visible {
            if c == column {
                return x;
            }
            x += self.widths[c];
        }
        x
    }
}

fn cell_attrs(theme: &Theme) -> ravenrs_text::TextAttrs {
    util::attrs(theme, TextStyle::Callout, None)
}

fn justify(align: CellAlign) -> Justify {
    match align {
        CellAlign::Start => Justify::Start,
        CellAlign::Center => Justify::Center,
        CellAlign::End => Justify::End,
    }
}

/// A fixed-width cell box.
fn cell_style(width: f32, height: f32, align: CellAlign) -> LayoutStyle {
    LayoutStyle {
        width: Some(width),
        height: Some(height),
        padding: Insets::symmetric(0.0, 12.0),
        spacing: 6.0,
        align: Align::Center,
        justify: justify(align),
        overflow_hidden: true,
        ..LayoutStyle::stack(Axis::Horizontal)
    }
}

/// Applies a click on data row `row` (at view index `index`) to the selection.
fn click_row(shared_result: &QueryResult, h: &Handles, mode: SelectionMode, row: usize, index: usize) {
    let m = h.modifiers.get();
    h.focus_row.set(Some(index));
    match mode {
        SelectionMode::None => {}
        SelectionMode::Single => h.selection.set(BTreeSet::from([row])),
        SelectionMode::Multiple => {
            if m.shift
                && let Some(anchor) = h.anchor.get()
            {
                let (a, b) = if anchor <= index { (anchor, index) } else { (index, anchor) };
                let range = shared_result.rows[a..=b.min(shared_result.rows.len() - 1)]
                    .iter()
                    .filter_map(|r| if let ViewRow::Data(i) = r { Some(*i) } else { None })
                    .collect();
                h.selection.set(range);
                return;
            } else if m.ctrl || m.meta {
                h.selection.update(|s| {
                    if !s.remove(&row) {
                        s.insert(row);
                    }
                });
            } else {
                h.selection.set(BTreeSet::from([row]));
            }
        }
    }
    h.anchor.set(Some(index));
}

/// Who a grid's drag is moving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DragSource {
    /// A column, from its header.
    Column(usize),
    /// A grouped column, from its chip.
    Chip(usize),
}

/// What letting go would do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DropTarget {
    Nothing,
    /// Group by it, at this place among the groups.
    Group(usize),
    /// Show it before this column, or last.
    Column(Option<usize>),
    /// Stop grouping by it.
    Ungroup,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct GridDrag {
    source: DragSource,
    at: Point,
    target: DropTarget,
}

/// Where things landed in the last paint, to tell what a drag is over.
#[derive(Default)]
struct Zones {
    panel: Rect,
    header: Rect,
    chips: BTreeMap<usize, Rect>,
    cells: BTreeMap<usize, Rect>,
}

/// What the pointer has to travel before a press becomes a drag, and not a click.
const DRAG_FROM: f32 = 5.0;
const HEADER_HEIGHT: f32 = 38.0;
const FOOTER_HEIGHT: f32 = 40.0;
const FOOTER_NAME_FROM: f32 = 170.0;
const SUMMARY_EDITOR_WIDTH: f32 = 470.0;
const GROUP_INDENT: f32 = 20.0;
const GROUP_PANEL_HEIGHT: f32 = 44.0;
/// Below this many distinct values a filter menu lists them all.
const MAX_FILTER_VALUES: usize = 500;
/// Roughly how wide a character of a cell is, to fit a column to its content.
const CHAR_WIDTH: f32 = 7.2;
const PAGE_SIZES: [usize; 5] = [25, 50, 100, 200, 500];

/// A press on a header or a chip: it follows the pointer once it is a drag, and on
/// letting go does what `target` says.
fn drag_handler(
    h: &Handles,
    columns: &Rc<Vec<(String, bool)>>,
    source: DragSource,
) -> Rc<dyn Fn(crate::Drag)> {
    let (h, columns) = (h.clone(), columns.clone());
    Rc::new(move |d| match d.phase {
        DragPhase::Start => {
            h.press.set((d.position, false));
            h.dragged.set(false);
        }
        DragPhase::Move => {
            let (start, active) = h.press.get();
            if !active {
                let (dx, dy) = (d.position.x - start.x, d.position.y - start.y);
                if dx.hypot(dy) < DRAG_FROM {
                    return;
                }
                h.press.set((start, true));
                h.dragged.set(true);
                h.drag.set(Some(GridDrag { source, at: d.position, target: DropTarget::Nothing }));
            }
            // Escape took it away while it was being dragged.
            if h.drag.peek().is_none() {
                return;
            }
            let target = drop_target(&h, &columns, source, d.position);
            h.drag.set(Some(GridDrag { source, at: d.position, target }));
        }
        _ => {
            let (_, active) = h.press.get();
            h.press.set((d.position, false));
            let drag = *h.drag.peek();
            if active && let Some(drag) = drag {
                h.dragged.set(true);
                h.drag.set(None);
                finish_drag(&h, drag);
            }
        }
    })
}

fn drop_target(h: &Handles, columns: &[(String, bool)], source: DragSource, at: Point) -> DropTarget {
    let zones = h.zones.borrow();
    if zones.panel.contains(at) {
        if let DragSource::Column(c) = source
            && !columns.get(c).is_some_and(|col| col.1)
        {
            return DropTarget::Nothing;
        }
        let mut place = 0;
        let moving = if let DragSource::Chip(c) = source { Some(c) } else { None };
        for (i, &group) in h.query.peek().group_by.iter().enumerate() {
            if Some(group) == moving {
                continue;
            }
            let past = zones.chips.get(&i).is_some_and(|chip| at.x > chip.center().x);
            if past {
                place += 1;
            }
        }
        return DropTarget::Group(place);
    }
    if zones.header.contains(at) {
        let before = zones
            .cells
            .iter()
            .filter(|(_, r)| at.x < r.center().x)
            .min_by(|a, b| a.1.x().partial_cmp(&b.1.x()).unwrap_or(std::cmp::Ordering::Equal));
        return DropTarget::Column(before.map(|(c, _)| *c));
    }
    match source {
        DragSource::Chip(_) => DropTarget::Ungroup,
        DragSource::Column(_) => DropTarget::Nothing,
    }
}

fn finish_drag(h: &Handles, drag: GridDrag) {
    let column = match drag.source {
        DragSource::Column(c) | DragSource::Chip(c) => c,
    };
    match drag.target {
        DropTarget::Nothing => {}
        DropTarget::Group(place) => h.query.update(|q| {
            q.group_by.retain(|g| *g != column);
            let place = place.min(q.group_by.len());
            q.group_by.insert(place, column);
            q.collapsed.clear();
        }),
        DropTarget::Column(before) => {
            if matches!(drag.source, DragSource::Chip(_)) {
                h.query.update(|q| q.toggle_group(column));
            }
            h.arrange.update(|a| a.move_column(column, before));
        }
        DropTarget::Ungroup => h.query.update(|q| q.toggle_group(column)),
    }
}

/// The thin handle at a header's edge: drag to resize, double-click to fit.
struct ResizeHandle<'a> {
    column: usize,
    shared: &'a Shared<'a>,
}

impl View for ResizeHandle<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let (s, c) = (self.shared, self.column);
        let col = &s.grid.columns[c];
        let shown = cx.is_hovered() || s.h.resize.get().is_some_and(|(column, ..)| column == c);
        let line = theme.color(Semantic::Accent);
        let handle = cx.container(
            LayoutStyle { width: Some(6.0), height: Some(HEADER_HEIGHT), ..LayoutStyle::leaf() },
            &[],
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                if shown {
                    scene.push_quad(Quad::new(
                        Rect::new(f.center().x - 1.0, f.y() + 6.0, 2.0, f.height() - 12.0),
                        line,
                    ));
                }
            })),
        );
        let (widths, resize, min) = (s.h.widths.clone(), s.h.resize.clone(), col.min_width);
        cx.on_drag(
            handle,
            Rc::new(move |d| match d.phase {
                DragPhase::Start => {
                    let w = widths.peek().get(c).copied().unwrap_or(120.0);
                    resize.set(Some((c, d.position.x, w)));
                }
                DragPhase::Move => {
                    if let Some((col, start_x, start_w)) = resize.get()
                        && col == c
                    {
                        let w = (start_w + d.position.x - start_x).max(min);
                        widths.update(|ws| ws[c] = w);
                    }
                }
                _ => resize.set(None),
            }),
        );
        let (widths, width) = (s.h.widths.clone(), fit_width(s, c));
        cx.on_double_click(handle, Rc::new(move || widths.update(|ws| ws[c] = width)));
        cx.cursor(handle, CursorIcon::ColResize);
        handle
    }
}

/// The width that shows a column's title and what is on the screen of its values.
fn fit_width(s: &Shared<'_>, c: usize) -> f32 {
    let col = &s.grid.columns[c];
    let empty = Value::Empty;
    let longest = s
        .rows
        .iter()
        .take(2000)
        .map(|row| col.display_cow(row.get(c).unwrap_or(&empty)).chars().count())
        .max()
        .unwrap_or(0)
        .max(col.title.chars().count() + 4);
    (longest as f32 * CHAR_WIDTH + 32.0).max(col.min_width)
}

/// What a column's menu holds.
fn column_menu(s: &Shared<'_>, c: usize, labels: &crate::Labels) -> Vec<Entry> {
    let (col, query, h) = (&s.grid.columns[c], s.h.query.peek().clone(), &s.h);
    let mut entries = Vec::new();
    let item = |text: &str, icon: IconName| MenuItem::new(text.to_owned()).icon(icon);
    if col.sortable {
        for (text, icon, direction) in [
            (&labels.grid_sort_ascending, IconName::ArrowUp, SortDirection::Ascending),
            (&labels.grid_sort_descending, IconName::ArrowDown, SortDirection::Descending),
        ] {
            let q = h.query.clone();
            entries.push(Entry::Item(item(text, icon).on_select(move || {
                q.update(|q| {
                    q.sort.retain(|(sorted, _)| *sorted != c);
                    q.sort.insert(0, (c, direction));
                });
            })));
        }
        if query.sort.iter().any(|(sorted, _)| *sorted == c) {
            let q = h.query.clone();
            entries.push(Entry::Item(
                item(&labels.grid_clear_sort, IconName::X)
                    .on_select(move || q.update(|q| q.sort.retain(|(sorted, _)| *sorted != c))),
            ));
        }
        entries.push(Entry::Separator);
    }
    if col.groupable && s.grid.remote.is_none() {
        let q = h.query.clone();
        let grouped = query.group_by.contains(&c);
        let text = if grouped { &labels.grid_ungroup_column } else { &labels.grid_group_by_column };
        entries.push(Entry::Item(
            item(text, IconName::Layers).on_select(move || q.update(|q| q.toggle_group(c))),
        ));
    }
    let arrange = s.arrangement;
    let pinned = arrange.pinned.get(c).copied().unwrap_or(false);
    let a = h.arrange.clone();
    entries.push(Entry::Item(
        item(
            if pinned { &labels.grid_unpin_column } else { &labels.grid_pin_column },
            if pinned { IconName::PinOff } else { IconName::Pin },
        )
        .on_select(move || a.update(|a| a.pinned[c] = !pinned)),
    ));
    if s.grid.remote.is_none() {
        let edit = h.summary_edit.clone();
        let (draft, start) = (h.summary_draft.clone(), first_draft(s, c));
        entries.push(Entry::Item(item(&labels.grid_edit_summary, IconName::Sigma).on_select(move || {
            draft.set(start.clone());
            edit.set(Some(c));
        })));
    }
    let (widths, width) = (h.widths.clone(), fit_width(s, c));
    entries.push(Entry::Item(
        item(&labels.grid_fit_column, IconName::Columns3)
            .on_select(move || widths.update(|ws| ws[c] = width)),
    ));
    let q = h.query.clone();
    entries.push(Entry::Item(item(&labels.grid_hide_column, IconName::EyeOff).on_select(move || {
        q.update(|q| {
            if q.hidden.len() <= c {
                q.hidden.resize(c + 1, false);
            }
            q.hidden[c] = true;
        });
    })));
    if !query.filter(c).trim().is_empty() {
        let q = h.query.clone();
        entries.push(Entry::Item(
            item(&labels.grid_clear_filter, IconName::Filter)
                .on_select(move || q.update(|q| q.set_filter(c, String::new()))),
        ));
    }
    if s.grid.remote.is_none() {
        entries.push(Entry::Separator);
        let a = h.arrange.clone();
        let showing = arrange.group_panel;
        entries.push(Entry::Item(
            item(if showing { &labels.grid_hide_panel } else { &labels.grid_show_panel }, IconName::Rows3)
                .on_select(move || a.update(|a| a.group_panel = !showing)),
        ));
    }
    entries
}

/// What a formula's value looks like in its column.
fn formula_display(col: &GridColumn, value: &ravenrs_criteria::Value) -> String {
    use ravenrs_criteria::Value as Field;
    match value {
        Field::Number(n) => col.display_summary(Summary::Sum, *n),
        Field::Text(text) => text.clone(),
        Field::Bool(b) => b.to_string(),
        Field::Null => "—".to_owned(),
        other => format!("{other:?}"),
    }
}

/// What the formula field starts with.
fn first_draft(s: &Shared<'_>, c: usize) -> String {
    s.arrangement.formulas.get(c).cloned().flatten().unwrap_or_else(|| {
        let title = &s.grid.columns[c].title;
        match s.arrangement.summaries.get(c).copied().flatten() {
            Some(Summary::Count) => "Count()".to_owned(),
            Some(Summary::Average) => format!("Avg([{title}])"),
            Some(Summary::Min) => format!("Min([{title}])"),
            Some(Summary::Max) => format!("Max([{title}])"),
            _ => format!("Sum([{title}])"),
        }
    })
}

/// The kinds of summary to choose from, in the order the editor lists them.
const SUMMARY_KINDS: [Option<Summary>; 6] = [
    None,
    Some(Summary::Sum),
    Some(Summary::Average),
    Some(Summary::Min),
    Some(Summary::Max),
    Some(Summary::Count),
];
const FORMULA_CHOICE: usize = SUMMARY_KINDS.len();
const FORMULA_FUNCTIONS: [&str; 6] = ["Sum(", "Avg(", "Min(", "Max(", "Count()", "Iif("];

/// Edits what a column sums up: one of the usual kinds, or a formula.
struct SummaryEditor {
    column: usize,
    columns: Vec<GridColumn>,
    rows: Rc<Vec<Row>>,
    matched: Rc<QueryResult>,
    arrange: State<Arrangement>,
    draft: State<String>,
    close: Rc<dyn Fn()>,
}

impl View for SummaryEditor {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let c = self.column;
        let (ink, muted) = (theme.color(Semantic::Ink), theme.color(Semantic::InkMuted));
        let arrangement = self.arrange.get();
        let by_formula = arrangement.formulas[c].is_some();
        let title = format!("{} {}", labels.grid_summary_of, self.columns[c].title);
        let mut lines = vec![util::label(cx, &title, TextStyle::Callout, Some(Weight::SEMIBOLD), ink)];

        let mut names: Vec<String> = SUMMARY_KINDS
            .iter()
            .map(|kind| kind.map_or(labels.grid_summary_none.clone(), |s| labels.summary(s).to_owned()))
            .collect();
        names.push(labels.grid_summary_formula.clone());
        let (read, write, draft) = (self.arrange.clone(), self.arrange.clone(), self.draft.clone());
        let choice = Binding::new(
            move || {
                let now = read.get();
                if now.formulas[c].is_some() {
                    return FORMULA_CHOICE;
                }
                SUMMARY_KINDS.iter().position(|k| *k == now.summaries[c]).unwrap_or(0)
            },
            move |i: usize| {
                let formula = (i == FORMULA_CHOICE).then(|| draft.get());
                write.update(|a| {
                    a.formulas[c].clone_from(&formula);
                    if let Some(kind) = SUMMARY_KINDS.get(i) {
                        a.summaries[c] = *kind;
                    }
                });
            },
        );
        lines.push(cx.child(&crate::SegmentedControl::new(choice, names).label(title.clone())));

        if by_formula {
            let text = self.draft.get();
            let checked = check_formula(&self.columns, &text)
                .and_then(|parsed| formula_value(&self.columns, &self.rows, &self.matched.matched, &parsed));
            let (write, draft, close) = (self.arrange.clone(), self.draft.clone(), self.close.clone());
            let valid = checked.is_ok();
            let apply = Rc::new(move || {
                if valid {
                    let formula = draft.get();
                    write.update(|a| a.formulas[c] = Some(formula.clone()));
                    close();
                }
            });
            let on_enter = apply.clone();
            lines.push(
                cx.child(
                    &TextField::new(self.draft.binding())
                        .placeholder(labels.grid_formula_hint.clone())
                        .autofocus()
                        .on_submit(move |_| on_enter()),
                ),
            );
            lines.push(match &checked {
                Ok(value) => {
                    let shown = format!(
                        "{}: {}",
                        labels.grid_formula_result,
                        formula_display(&self.columns[c], value)
                    );
                    util::label(cx, &shown, TextStyle::Callout, Some(Weight::MEDIUM), ink)
                }
                Err(problem) => {
                    let shown = format!("{}: {problem}", labels.grid_formula_error);
                    cx.child(&crate::Text::new(shown).style(TextStyle::Callout).foreground(Semantic::Danger))
                }
            });
            let inserter = |piece: String, name: String, draft: State<String>| {
                Button::new(name).style(ButtonStyle::Secondary).size(ButtonSize::Small).on_click(move || {
                    draft.update(|text| text.push_str(&piece));
                })
            };
            lines.push(util::label(cx, &labels.grid_formula_columns, TextStyle::Caption, None, muted));
            let chips: Vec<NodeId> = self
                .columns
                .iter()
                .enumerate()
                .map(|(i, col)| {
                    let piece = format!("[{}]", col.title);
                    cx.keyed_child(i as u64, &inserter(piece, col.title.clone(), self.draft.clone()))
                })
                .collect();
            lines.push(cx.container(
                LayoutStyle { spacing: 4.0, wrap: true, ..LayoutStyle::stack(Axis::Horizontal) },
                &chips,
                Paint::None,
            ));
            lines.push(util::label(cx, &labels.grid_formula_functions, TextStyle::Caption, None, muted));
            let chips: Vec<NodeId> = FORMULA_FUNCTIONS
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    cx.keyed_child(
                        100 + i as u64,
                        &inserter((*f).to_owned(), (*f).to_owned(), self.draft.clone()),
                    )
                })
                .collect();
            lines.push(cx.container(
                LayoutStyle { spacing: 4.0, wrap: true, ..LayoutStyle::stack(Axis::Horizontal) },
                &chips,
                Paint::None,
            ));
            lines.push(
                cx.child(
                    &Button::new(labels.grid_apply.clone())
                        .disabled(!valid)
                        .size(ButtonSize::Small)
                        .on_click(move || apply()),
                ),
            );
        }
        cx.container(
            LayoutStyle {
                spacing: 10.0,
                width: Some(SUMMARY_EDITOR_WIDTH),
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &lines,
            Paint::None,
        )
    }
}

/// A column's total at the bottom: click to change what it sums up.
struct FooterCell<'a> {
    column: usize,
    shared: &'a Shared<'a>,
}

impl View for FooterCell<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let (s, c) = (self.shared, self.column);
        let col = &s.grid.columns[c];
        let labels = cx.labels();
        let (ink, muted) = (theme.color(Semantic::Ink), theme.color(Semantic::InkMuted));
        let bold = util::attrs(&theme, TextStyle::Callout, Some(Weight::SEMIBOLD));
        let hovered = cx.is_hover_within();
        let editable = s.grid.remote.is_none();
        let figure = match s.formula_text(c, (0, s.result.matched.len())) {
            Some(text) => Some((text, labels.grid_summary_formula.clone())),
            None => s.arrangement.summaries[c]
                .zip(s.result.totals.get(c).copied().flatten())
                .map(|(summary, v)| (col.display_summary(summary, v), labels.summary(summary).to_owned())),
        };
        let mut parts = Vec::new();
        let by_formula = s.arrangement.formulas[c].is_some();
        match &figure {
            Some((text, kind)) => {
                if by_formula {
                    parts.push(util::icon(cx, IconName::Sigma, 13.0, theme.color(Semantic::Accent)));
                } else if s.widths[c] >= FOOTER_NAME_FROM {
                    parts.push(util::label(cx, kind, TextStyle::Caption, None, muted));
                }
                parts.push(cx.text(text, &bold, ink, LayoutStyle::leaf()));
            }
            None if hovered && editable => parts.push(util::icon(cx, IconName::Sigma, 14.0, muted)),
            None => {}
        }
        let fixed = s.fixed_x(c);
        let mut style = cell_style(s.widths[c], FOOTER_HEIGHT, col.align);
        // In line with the column's values, even when the total barely fits.
        style.padding = Insets { left: 2.0, right: 12.0, ..Insets::ZERO };
        style.absolute = fixed.map(|x| (x, 0.0));
        let surface = theme.color(Semantic::Surface);
        let tint = if hovered && editable { ink.with_alpha(0.04) } else { Color::TRANSPARENT };
        let solid = fixed.is_some();
        let node = cx.container(
            style,
            &parts,
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                if solid {
                    scene.push_quad(Quad::new(f, surface));
                }
                scene.push_quad(Quad::new(f, tint));
            })),
        );
        if fixed.is_some() {
            cx.translate(node, Point::new(s.offset_x, 0.0));
        }
        if editable {
            let edit = s.h.summary_edit.clone();
            let (draft, start) = (s.h.summary_draft.clone(), first_draft(s, c));
            cx.on_click(
                node,
                Rc::new(move || {
                    draft.set(start.clone());
                    edit.set(Some(c));
                }),
            );
            cx.cursor(node, CursorIcon::Pointer);
            let value = figure.map_or_else(String::new, |(text, _)| text);
            cx.semantics(
                node,
                Semantics::new(Role::Button)
                    .name(format!("{} {}", labels.grid_edit_summary, col.title))
                    .value(value),
            );
        }
        node
    }
}

/// A value as the filter expression language writes it.
fn value_literal(value: &Value) -> Option<String> {
    match value {
        Value::Text(text) => Some(format!("'{}'", text.replace('\'', "''"))),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(if *b { "True" } else { "False" }.to_owned()),
        _ => None,
    }
}

/// The distinct values of a column, with what the filter menu shows for each.
fn values_of(rows: &[Row], col: &GridColumn, c: usize) -> Vec<(String, Value)> {
    let mut seen = BTreeMap::new();
    for row in rows {
        let Some(value) = row.get(c) else { continue };
        if matches!(value, Value::Empty) {
            continue;
        }
        let shown = col.display_cow(value);
        if !seen.contains_key(shown.as_ref()) {
            seen.insert(shown.into_owned(), value.clone());
            if seen.len() >= MAX_FILTER_VALUES {
                break;
            }
        }
    }
    seen.into_iter().collect()
}

/// A column's filter by value: a list of what is in it, to check.
struct ValueFilter {
    title: String,
    column: usize,
    values: Vec<(String, Value)>,
    query: State<GridQuery>,
    chosen: State<BTreeSet<String>>,
    search: State<String>,
}

impl ValueFilter {
    fn write(&self, chosen: &BTreeSet<String>) {
        let literals: Vec<String> = self
            .values
            .iter()
            .filter(|(shown, _)| chosen.contains(shown))
            .filter_map(|(_, value)| value_literal(value))
            .collect();
        let text = if literals.is_empty() {
            String::new()
        } else {
            format!("[{}] In ({})", self.title, literals.join(", "))
        };
        let c = self.column;
        self.query.update(|q| q.set_filter(c, text));
    }
}

impl View for ValueFilter {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let labels = cx.labels();
        let chosen = self.chosen.get();
        let needle = self.search.get().to_lowercase();
        let mut lines = vec![
            cx.child(
                &TextField::new(self.search.binding())
                    .placeholder(labels.grid_search.clone())
                    .icon(IconName::Search)
                    .compact(),
            ),
        ];
        let mut list = Vec::new();
        for (shown, _) in self.values.iter().filter(|(shown, _)| shown.to_lowercase().contains(&needle)) {
            let on = chosen.contains(shown);
            let (name, this) = (shown.clone(), ValueFilterHandle::of(self));
            let binding = Binding::new(move || on, move |checked: bool| this.set(&name, checked));
            list.push(cx.keyed_child(hash_of(shown), &crate::Checkbox::new(shown.clone(), binding)));
        }
        let inner = cx.container(
            LayoutStyle { spacing: 6.0, ..LayoutStyle::stack(Axis::Vertical) },
            &list,
            Paint::None,
        );
        let scroller = cx.container(
            LayoutStyle {
                height: Some((list.len() as f32 * 28.0).min(260.0)),
                overflow_hidden: true,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[inner],
            Paint::None,
        );
        cx.scrollable(scroller);
        lines.push(scroller);
        if !chosen.is_empty() {
            let this = ValueFilterHandle::of(self);
            lines.push(
                cx.child(
                    &Button::new(labels.grid_clear_filter.clone())
                        .style(ButtonStyle::Ghost)
                        .size(ButtonSize::Small)
                        .icon(IconName::X)
                        .on_click(move || this.clear()),
                ),
            );
        }
        cx.container(
            LayoutStyle {
                spacing: 8.0,
                width: Some(240.0),
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &lines,
            Paint::None,
        )
    }
}

/// What a checkbox of a [`ValueFilter`] keeps to write the filter.
#[derive(Clone)]
struct ValueFilterHandle {
    title: String,
    column: usize,
    values: Rc<Vec<(String, Value)>>,
    query: State<GridQuery>,
    chosen: State<BTreeSet<String>>,
}

impl ValueFilterHandle {
    fn of(filter: &ValueFilter) -> Self {
        Self {
            title: filter.title.clone(),
            column: filter.column,
            values: Rc::new(filter.values.clone()),
            query: filter.query.clone(),
            chosen: filter.chosen.clone(),
        }
    }

    fn write(&self, chosen: &BTreeSet<String>) {
        ValueFilter {
            title: self.title.clone(),
            column: self.column,
            values: self.values.as_ref().clone(),
            query: self.query.clone(),
            chosen: self.chosen.clone(),
            search: State::new(String::new()),
        }
        .write(chosen);
    }

    fn set(&self, name: &str, on: bool) {
        let mut chosen = self.chosen.get();
        if on {
            chosen.insert(name.to_owned());
        } else {
            chosen.remove(name);
        }
        self.write(&chosen);
        self.chosen.set(chosen);
    }

    fn clear(&self) {
        self.write(&BTreeSet::new());
        self.chosen.set(BTreeSet::new());
    }
}

fn hash_of(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3))
}

struct HeaderCell<'a> {
    column: usize,
    shared: &'a Shared<'a>,
}

impl View for HeaderCell<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let s = self.shared;
        let c = self.column;
        let col = &s.grid.columns[c];
        let labels = cx.labels();
        let query = s.h.query.peek().clone();
        let sort = query.sort.iter().position(|(sc, _)| *sc == c).map(|i| (i, query.sort[i].1));
        let hovered = cx.is_hover_within();
        let accent = theme.color(Semantic::Accent);
        let ink = if sort.is_some() || hovered {
            theme.color(Semantic::Ink)
        } else {
            theme.color(Semantic::InkMuted)
        };
        let a = util::attrs(&theme, TextStyle::Caption, Some(Weight::SEMIBOLD));
        let mut content = Vec::new();
        if s.arrangement.pinned.get(c).copied().unwrap_or(false) {
            content.push(util::icon(cx, IconName::Pin, 12.0, theme.color(Semantic::InkMuted)));
        }
        content.push(cx.text(&col.title, &a, ink, LayoutStyle::leaf()));
        if let Some((priority, dir)) = sort {
            let icon = if dir == SortDirection::Ascending { IconName::ArrowUp } else { IconName::ArrowDown };
            content.push(util::icon(cx, icon, 13.0, accent));
            if query.sort.len() > 1 {
                content.push(util::label(cx, &(priority + 1).to_string(), TextStyle::Caption, None, ink));
            }
        }
        let width = s.widths[c];
        let filtered = !query.filter(c).trim().is_empty();
        let can_filter = col.filterable && s.grid.remote.is_none();
        let opened_filter = s.h.filter_open.get() == Some(c);
        let opened_menu = s.h.menu.get().is_some_and(|(column, _)| column == c);
        let tools = hovered || opened_filter || opened_menu;
        let mut label_style = LayoutStyle {
            grow: 1.0,
            min_width: Some(0.0),
            height: Some(HEADER_HEIGHT),
            spacing: 6.0,
            align: Align::Center,
            justify: justify(col.align),
            overflow_hidden: true,
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        label_style.padding = Insets { left: 12.0, right: 4.0, ..Insets::ZERO };
        let label = cx.container(label_style, &content, Paint::None);
        let mut semantics = Semantics::new(Role::ColumnHeader).name(col.title.clone());
        if let Some((_, dir)) = sort {
            semantics = semantics.description(if dir == SortDirection::Ascending {
                labels.grid_sorted_ascending.clone()
            } else {
                labels.grid_sorted_descending.clone()
            });
        }
        cx.semantics(label, semantics);
        let dragged = s.h.dragged.clone();
        if col.sortable {
            let q = s.h.query.clone();
            let m = s.h.modifiers.clone();
            cx.on_click(
                label,
                Rc::new(move || {
                    // Letting go of a drag on its own header is not a click.
                    if dragged.replace(false) {
                        return;
                    }
                    let mods = m.get();
                    q.update(|q| q.toggle_sort(c, mods.shift || mods.ctrl || mods.meta));
                }),
            );
            cx.cursor(label, CursorIcon::Pointer);
        }
        cx.on_drag(label, drag_handler(&s.h, &s.column_facts, DragSource::Column(c)));
        let menu = s.h.menu.clone();
        cx.on_context_menu(label, Rc::new(move |p| menu.set(Some((c, Some(p))))));
        let mut parts = vec![label];
        let mut funnel_node = None;
        if can_filter && (tools || filtered) {
            let opener = s.h.filter_open.clone();
            let chosen = s.h.filter_chosen.clone();
            let search = s.h.filter_search.clone();
            let starts = if opened_filter { None } else { Some(c) };
            let (q2, rows2, col2) = (s.h.query.clone(), s.rows_rc.clone(), col.clone());
            let node = cx.keyed_child(
                1,
                &crate::IconButton::new(IconName::Filter)
                    .label(format!("{} {}", labels.grid_filter_values, col.title))
                    .style(if filtered { ButtonStyle::Secondary } else { ButtonStyle::Ghost })
                    .size(ButtonSize::Small)
                    .on_click(move || {
                        // The list starts from what the filter holds, written by hand or not.
                        let text = q2.peek().filter(c).to_owned();
                        let values = values_of(&rows2, &col2, c);
                        chosen.set(
                            values
                                .into_iter()
                                .filter(|(_, v)| {
                                    value_literal(v).is_some_and(|l| {
                                        text.contains(&format!("{l},")) || text.contains(&format!("{l})"))
                                    })
                                })
                                .map(|(shown, _)| shown)
                                .collect(),
                        );
                        search.set(String::new());
                        opener.set(starts);
                    }),
            );
            funnel_node = Some(node);
            parts.push(node);
        }
        let mut menu_node = None;
        if tools {
            let opener = s.h.menu.clone();
            let node = cx.keyed_child(
                2,
                &crate::IconButton::new(IconName::EllipsisVertical)
                    .label(format!("{} {}", labels.grid_column_menu, col.title))
                    .style(ButtonStyle::Ghost)
                    .size(ButtonSize::Small)
                    .on_click(move || {
                        opener.update(|m| *m = if m.is_some() { None } else { Some((c, None)) });
                    }),
            );
            menu_node = Some(node);
            parts.push(node);
        }
        parts.push(cx.keyed_child(3, &ResizeHandle { column: c, shared: s }));

        let fixed = s.fixed_x(c);
        let lit =
            s.drop_before == Some(Some(c)) || (s.drop_before == Some(None) && s.visible.last() == Some(&c));
        let at_end = s.drop_before == Some(None);
        let zones = s.h.zones.clone();
        let hover_fill =
            if hovered { theme.color(Semantic::Ink).with_alpha(0.03) } else { Color::TRANSPARENT };
        let solid = fixed.map(|_| theme.color(Semantic::Surface));
        let edge = (s.last_fixed == Some(c)).then(|| theme.color(Semantic::Border));
        let node = cx.container(
            LayoutStyle {
                width: Some(width),
                height: Some(HEADER_HEIGHT),
                align: Align::Center,
                absolute: fixed.map(|x| (x, 0.0)),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &parts,
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                zones.borrow_mut().cells.insert(c, f);
                if let Some(solid) = solid {
                    scene.push_quad(Quad::new(f, solid));
                }
                scene.push_quad(Quad::new(f, hover_fill));
                if let Some(edge) = edge {
                    scene.push_quad(Quad::new(Rect::new(f.max_x() - 1.0, f.y(), 1.0, f.height()), edge));
                }
                if lit {
                    let x = if at_end { f.max_x() - 2.0 } else { f.x() };
                    scene.push_quad(Quad::new(Rect::new(x, f.y() + 4.0, 2.0, f.height() - 8.0), accent));
                }
            })),
        );
        if fixed.is_some() {
            cx.translate(node, Point::new(s.offset_x, 0.0));
        }
        let opened = *s.h.menu.peek();
        if opened_menu && let Some(button_or_point) = opened {
            let entries = column_menu(s, c, &labels);
            let menu = s.h.menu.clone();
            let close: Rc<dyn Fn()> = Rc::new(move || menu.set(None));
            let list = MenuList { entries: &entries, close: close.clone(), on_side: None };
            match button_or_point.1 {
                Some(p) => cx.overlay(node, Placement::At(p), &list, Some(close)),
                None => cx.overlay(menu_node.unwrap_or(node), Placement::Below, &list, Some(close)),
            }
        }
        if opened_filter {
            let close: Rc<dyn Fn()> = {
                let open = s.h.filter_open.clone();
                Rc::new(move || open.set(None))
            };
            let filter = ValueFilter {
                title: col.title.clone(),
                column: c,
                values: values_of(s.rows, col, c),
                query: s.h.query.clone(),
                chosen: s.h.filter_chosen.clone(),
                search: s.h.filter_search.clone(),
            };
            cx.overlay(funnel_node.unwrap_or(node), Placement::Below, &PanelOf(&filter), Some(close));
        }
        node
    }
}

struct FilterCell<'a> {
    column: usize,
    shared: &'a Shared<'a>,
}

impl View for FilterCell<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let s = self.shared;
        let c = self.column;
        let col = &s.grid.columns[c];
        let mut children = Vec::new();
        if col.filterable {
            let (read, write) = (s.h.query.clone(), s.h.query.clone());
            let binding = Binding::new(
                move || read.with(|q| q.filter(c).to_owned()),
                move |t| write.update(|q| q.set_filter(c, t)),
            );
            let labels = cx.labels();
            let hint =
                if col.number.is_some() { &labels.grid_number_filter_hint } else { &labels.grid_filter_hint };
            let field = cx.child(&TextField::new(binding).placeholder(hint.clone()).compact());
            cx.label_control(field, &format!("{} {}", labels.grid_filter_column, col.title), Some(hint));
            children.push(field);
        }
        let mut style = cell_style(s.widths[c], HEADER_HEIGHT, CellAlign::Start);
        style.padding = Insets::symmetric(0.0, 6.0);
        style.align = Align::Center;
        style.overflow_hidden = false;
        let fixed = s.fixed_x(c);
        style.absolute = fixed.map(|x| (x, 0.0));
        let paint =
            fixed.map_or(Paint::None, |_| Paint::Quad(QuadPaint::new(theme.color(Semantic::Surface))));
        let node = cx.container(LayoutStyle { align: Align::Stretch, ..style }, &children, paint);
        if fixed.is_some() {
            cx.translate(node, Point::new(s.offset_x, 0.0));
        }
        node
    }
}

/// A chip of the group panel: drag to reorder, or off the panel to ungroup; click to
/// change the direction of the groups; arrows with Ctrl move it, Delete removes it.
struct GroupChip<'a> {
    index: usize,
    column: usize,
    grid: &'a DataGrid,
    h: &'a Handles,
    facts: &'a Rc<Vec<(String, bool)>>,
}

impl View for GroupChip<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let (c, i, h) = (self.column, self.index, self.h);
        let title = self.grid.columns.get(c).map_or("?", |col| col.title.as_str());
        let query = h.query.peek().clone();
        let descending = query.sort.iter().any(|(s, d)| *s == c && *d == SortDirection::Descending);
        let (ink, muted, accent) =
            (theme.color(Semantic::Ink), theme.color(Semantic::InkMuted), theme.color(Semantic::Accent));
        let grip = util::icon(cx, IconName::EllipsisVertical, 12.0, muted);
        let name = util::label(cx, title, TextStyle::Callout, Some(Weight::MEDIUM), ink);
        let arrow =
            util::icon(cx, if descending { IconName::ArrowDown } else { IconName::ArrowUp }, 13.0, accent);
        let body = cx.container(
            LayoutStyle {
                spacing: 6.0,
                align: Align::Center,
                height: Some(28.0),
                padding: Insets { left: 8.0, right: 2.0, ..Insets::ZERO },
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[grip, name, arrow],
            Paint::None,
        );
        cx.semantics(body, Semantics::new(Role::Button).name(format!("{} {title}", labels.grid_grouped_by)));
        cx.cursor(body, CursorIcon::Pointer);
        let (q, dragged) = (h.query.clone(), h.dragged.clone());
        cx.on_click(
            body,
            Rc::new(move || {
                if dragged.replace(false) {
                    return;
                }
                q.update(|q| {
                    q.sort.retain(|(s, _)| *s != c);
                    if !descending {
                        q.sort.push((c, SortDirection::Descending));
                    }
                });
            }),
        );
        cx.on_drag(body, drag_handler(h, self.facts, DragSource::Chip(c)));
        cx.focusable(body, 6.0);
        let q = h.query.clone();
        cx.on_key(
            body,
            Rc::new(move |key, mods| {
                match key {
                    Key::ArrowLeft if (mods.ctrl || mods.meta) && i > 0 => q.update(|q| {
                        q.group_by.swap(i, i - 1);
                        q.collapsed.clear();
                    }),
                    Key::ArrowRight if mods.ctrl || mods.meta => q.update(|q| {
                        if i + 1 < q.group_by.len() {
                            q.group_by.swap(i, i + 1);
                            q.collapsed.clear();
                        }
                    }),
                    Key::Delete | Key::Backspace => q.update(|q| q.toggle_group(c)),
                    _ => return false,
                }
                true
            }),
        );
        let q = h.query.clone();
        let remove = cx.keyed_child(
            1,
            &crate::IconButton::new(IconName::X)
                .label(format!("{} {title}", labels.grid_remove_group))
                .style(ButtonStyle::Ghost)
                .size(ButtonSize::Small)
                .on_click(move || q.update(|q| q.toggle_group(c))),
        );
        let zones = h.zones.clone();
        let (fill, line) = (theme.color(Semantic::SurfaceElevated), theme.color(Semantic::Border));
        let radius = theme.radii.sm;
        let moving = h.drag.peek().is_some_and(|d| d.source == DragSource::Chip(c));
        let (fill, line) = if moving { (fill.faded(0.4), line.faded(0.4)) } else { (fill, line) };
        cx.container(
            LayoutStyle { align: Align::Center, height: Some(30.0), ..LayoutStyle::stack(Axis::Horizontal) },
            &[body, remove],
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                zones.borrow_mut().chips.insert(i, f);
                scene.push_quad(Quad::new(f, fill).radius(radius).border(1.0, line));
            })),
        )
    }
}

/// What follows the pointer during a drag.
struct DragGhost {
    title: String,
    grouping: bool,
    ungrouping: bool,
}

impl View for DragGhost {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let accent = theme.color(Semantic::Accent);
        let icon = if self.ungrouping {
            IconName::X
        } else if self.grouping {
            IconName::Layers
        } else {
            IconName::Columns3
        };
        let mark = util::icon(cx, icon, 14.0, accent);
        let name = util::label(
            cx,
            &self.title,
            TextStyle::Callout,
            Some(Weight::SEMIBOLD),
            theme.color(Semantic::Ink),
        );
        let node = cx.container(
            LayoutStyle {
                spacing: 6.0,
                align: Align::Center,
                padding: Insets::symmetric(6.0, 10.0),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[mark, name],
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.sm,
                border: Some(ravenrs_core::Border { width: 1.0, color: accent }),
                shadow: Some(theme.elevation.raised),
                ..QuadPaint::new(theme.color(Semantic::SurfaceElevated))
            }),
        );
        cx.semantics(node, Semantics::hidden());
        node
    }
}

/// A row that has not arrived from the server yet.
struct WaitingRow<'a> {
    shared: &'a Shared<'a>,
}

impl View for WaitingRow<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let s = self.shared;
        let bar = theme.color(Semantic::Control);
        let line = theme.color(Semantic::Border).faded(0.6);
        let starts: Vec<(f32, f32)> = {
            let mut x = s.select_width;
            s.visible
                .iter()
                .map(|&c| {
                    let cell = (x, s.widths[c]);
                    x += s.widths[c];
                    cell
                })
                .collect()
        };
        let node = cx.container(
            LayoutStyle {
                width: Some(s.total_width),
                height: Some(s.grid.row_height),
                ..LayoutStyle::leaf()
            },
            &[],
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                for (x, width) in &starts {
                    let inner =
                        Rect::new(f.x() + x + 12.0, f.center().y - 5.0, (width - 40.0).max(8.0), 10.0);
                    scene.push_quad(Quad::new(inner, bar).radius(5.0));
                }
                scene.push_quad(Quad::new(Rect::new(f.x(), f.max_y() - 1.0, f.width(), 1.0), line));
            })),
        );
        cx.semantics(node, Semantics::hidden());
        node
    }
}

struct DataRow<'a> {
    index: usize,
    row: usize,
    values: Row,
    shared: &'a Shared<'a>,
}

impl View for DataRow<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let s = self.shared;
        let (index, row) = (self.index, self.row);
        let selected = s.selected.contains(&row);
        let hovered = cx.is_hover_within();
        let rh = s.grid.row_height;
        let mode = s.grid.selection_mode;
        let mut cells = Vec::with_capacity(s.visible.len() + 1);
        let mut fixed_cells = Vec::new();
        let ink = theme.color(Semantic::Ink);
        let fill = if selected {
            theme.color(Semantic::Accent).with_alpha(0.1)
        } else if hovered {
            ink.with_alpha(0.035)
        } else if s.grid.striped && index % 2 == 1 {
            ink.with_alpha(0.02)
        } else {
            Color::TRANSPARENT
        };
        let line = theme.color(Semantic::Border).faded(0.6);
        // What stays in place has to cover what scrolls under it.
        let surface = theme.color(Semantic::Surface);
        let solid = move |edge: Option<Color>| -> Paint {
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                scene.push_quad(Quad::new(f, surface));
                scene.push_quad(Quad::new(f, fill));
                scene.push_quad(Quad::new(Rect::new(f.x(), f.max_y() - 1.0, f.width(), 1.0), line));
                if let Some(edge) = edge {
                    scene.push_quad(Quad::new(Rect::new(f.max_x() - 1.0, f.y(), 1.0, f.height()), edge));
                }
            }))
        };
        let select_fixed = s.fixed_width > 0.0;
        if s.select_width > 0.0 {
            let b = crate::inputs::check_box(cx, &theme, selected, false);
            let cell = cx.container(
                LayoutStyle {
                    width: Some(s.select_width),
                    height: Some(rh),
                    align: Align::Center,
                    justify: Justify::Center,
                    absolute: select_fixed.then_some((0.0, 0.0)),
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &[b],
                if select_fixed { solid(None) } else { Paint::None },
            );
            let name = cx.labels().grid_select_row.clone();
            cx.semantics(cell, Semantics::new(Role::CheckBox).name(name).toggled(selected));
            let (sel, h) = (s.h.selection.clone(), s.h.clone());
            cx.on_click(
                cell,
                Rc::new(move || {
                    sel.update(|set| {
                        if !set.remove(&row) {
                            set.insert(row);
                        }
                    });
                    h.focus_row.set(Some(index));
                    h.anchor.set(Some(index));
                }),
            );
            cx.cursor(cell, CursorIcon::Pointer);
            if select_fixed {
                cx.translate(cell, Point::new(s.offset_x, 0.0));
                fixed_cells.push(cell);
            } else {
                cells.push(cell);
            }
        }
        let attrs = cell_attrs(&theme);
        let first_column = s.visible.first().copied();
        for &c in s.visible {
            let col = &s.grid.columns[c];
            let value = self.values.get(c).cloned().unwrap_or_default();
            let content = if s.editing == Some((row, c)) {
                let (data, buffer, editing, column) =
                    (s.grid.data.clone(), s.h.edit_buffer.clone(), s.h.editing.clone(), col.clone());
                let previous = value.clone();
                let cancel = s.h.editing.clone();
                cx.child(
                    &TextField::new(s.h.edit_buffer.binding())
                        .compact()
                        .autofocus()
                        .on_submit(move |text| {
                            data.set_cell(row, c, column.parse(&text, &previous));
                            buffer.set(String::new());
                            editing.set(None);
                        })
                        .on_cancel(move || cancel.set(None)),
                )
            } else if let Some(template) = &col.template {
                let view = template(&value);
                let inner = cx.keyed_child(c as u64, view.as_ref());
                // A neutral box, so the template's own cross-axis alignment cannot pull it off center.
                cx.container(LayoutStyle::stack(Axis::Horizontal), &[inner], Paint::None)
            } else {
                let ink = if value.is_empty() {
                    theme.color(Semantic::InkMuted)
                } else {
                    theme.color(Semantic::Ink)
                };
                let text = if value.is_empty() { "—".to_owned() } else { col.display(&value) };
                cx.text(&text, &attrs, ink, LayoutStyle::leaf())
            };
            let mut style = cell_style(s.widths[c], rh, col.align);
            if s.editing == Some((row, c)) {
                style.align = Align::Center;
                style.padding = Insets::symmetric(0.0, 4.0);
                style.justify = Justify::Start;
            } else if first_column == Some(c) {
                style.padding.left += s.indent;
            }
            let fixed = s.fixed_x(c);
            style.absolute = fixed.map(|x| (x, 0.0));
            let paint = if fixed.is_some() {
                solid((s.last_fixed == Some(c)).then(|| theme.color(Semantic::Border)))
            } else {
                Paint::None
            };
            let cell = cx.container(LayoutStyle { align: Align::Center, ..style }, &[content], paint);
            if s.editing != Some((row, c)) {
                cx.semantics(cell, Semantics::new(Role::Cell));
            }
            let result = s.result.clone();
            let h = s.h.clone();
            cx.on_click(cell, Rc::new(move || click_row(&result, &h, mode, row, index)));
            let (editable, h2, activate) =
                (col.editable && s.grid.remote.is_none(), s.h.clone(), s.grid.on_row_activate.clone());
            let rows_value = col.display(&value);
            cx.on_double_click(
                cell,
                Rc::new(move || {
                    if editable {
                        h2.edit_buffer.set(rows_value.clone());
                        h2.editing.set(Some((row, c)));
                    } else if let Some(a) = &activate {
                        a(row);
                    }
                }),
            );
            if fixed.is_some() {
                cx.translate(cell, Point::new(s.offset_x, 0.0));
                fixed_cells.push(cell);
            } else {
                cells.push(cell);
            }
        }
        if s.fixed_width > 0.0 {
            let room = cx.container(
                LayoutStyle { width: Some(s.fixed_width), height: Some(rh), ..LayoutStyle::leaf() },
                &[],
                Paint::None,
            );
            cells.insert(0, room);
        }
        cells.extend(fixed_cells);
        let focus = (s.focus_row == Some(index)).then(|| theme.color(Semantic::Accent));
        let column_lines = s.lines.clone();
        let node = cx.container(
            LayoutStyle {
                width: Some(s.total_width),
                height: Some(rh),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &cells,
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                scene.push_quad(Quad::new(f, fill));
                scene.push_quad(Quad::new(Rect::new(f.x(), f.max_y() - 1.0, f.width(), 1.0), line));
                for x in column_lines.iter() {
                    scene.push_quad(Quad::new(Rect::new(f.x() + x, f.y(), 1.0, f.height()), line));
                }
                if let Some(accent) = focus {
                    scene.push_quad(Quad::new(Rect::new(f.x(), f.y(), 2.0, f.height()), accent));
                }
            })),
        );
        // The grid body keeps focus; the keyboard row is announced as active.
        let mut semantics = Semantics::new(Role::Row).active(s.focus_row == Some(index));
        if mode != SelectionMode::None {
            semantics = semantics.selected(selected);
        }
        cx.semantics(node, semantics);
        node
    }
}

struct GroupRow<'a> {
    index: usize,
    shared: &'a Shared<'a>,
}

impl View for GroupRow<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let s = self.shared;
        let ViewRow::Group { level, column, label, key, count, collapsed, summaries } =
            &s.result.rows[self.index]
        else {
            return cx.empty();
        };
        let col = &s.grid.columns[*column];
        let (ink, muted) = (theme.color(Semantic::Ink), theme.color(Semantic::InkMuted));
        let chevron = if *collapsed { IconName::ChevronRight } else { IconName::ChevronDown };
        let mut children = vec![util::icon(cx, chevron, 16.0, muted)];
        let attrs = cell_attrs(&theme);
        children.push(cx.text(&format!("{}:", col.title), &attrs, muted, LayoutStyle::leaf()));
        let labels = cx.labels();
        let shown = if label.is_empty() { labels.grid_empty.as_str() } else { label.as_str() };
        let mut bold = attrs.clone();
        bold.weight = Weight::SEMIBOLD.0;
        children.push(cx.text(shown, &bold, ink, LayoutStyle::leaf()));
        let badge = cx.child(&Badge::new(crate::grid::model::NumberFormat::default().format(*count as f64)));
        children.push(cx.container(LayoutStyle::stack(Axis::Horizontal), &[badge], Paint::None));
        let rh = s.grid.row_height;
        let left = 12.0 + *level as f32 * GROUP_INDENT;
        let title = cx.container(
            LayoutStyle {
                height: Some(rh),
                padding: Insets { left, ..Insets::ZERO },
                spacing: 8.0,
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &children,
            Paint::None,
        );
        // The title stays in sight while the columns scroll.
        cx.translate(title, Point::new(s.offset_x, 0.0));
        // Each summary under its own column, where one can see them line up.
        let title_ends =
            left + 24.0 + (col.title.chars().count() + shown.chars().count() + 8) as f32 * CHAR_WIDTH;
        let mut parts = vec![title];
        let mut spoken = Vec::new();
        let range = s.ranges.get(self.index).copied().unwrap_or_default();
        for &c in s.visible {
            let other = &s.grid.columns[c];
            let figure = match s.formula_text(c, range) {
                Some(text) => Some((text, labels.grid_summary_formula.clone())),
                None => {
                    summaries.get(c).copied().flatten().zip(s.arrangement.summaries[c]).map(|(v, summary)| {
                        (other.display_summary(summary, v), labels.summary(summary).to_owned())
                    })
                }
            };
            if let Some((text, kind)) = figure {
                spoken.push(format!("{kind} {}: {text}", other.title));
                let x = s.x_of(c);
                if x < title_ends {
                    continue;
                }
                let value = cx.text(&text, &bold, ink, LayoutStyle::leaf());
                let mut style = cell_style(s.widths[c], rh, other.align);
                style.absolute = Some((x, 0.0));
                parts.push(cx.container(style, &[value], Paint::None));
            }
        }
        let hovered = cx.is_hover_within();
        let base = theme.color(Semantic::Canvas);
        let fill = if hovered { base.lerp(ink, 0.04) } else { base };
        let line = theme.color(Semantic::Border).faded(0.6);
        let node = cx.container(
            LayoutStyle {
                width: Some(s.total_width),
                height: Some(rh),
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &parts,
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                scene.push_quad(Quad::new(f, fill));
                scene.push_quad(Quad::new(Rect::new(f.x(), f.max_y() - 1.0, f.width(), 1.0), line));
            })),
        );
        let (q, key, focus, index) = (s.h.query.clone(), key.clone(), s.h.focus_row.clone(), self.index);
        cx.on_click(
            node,
            Rc::new(move || {
                focus.set(Some(index));
                q.update(|q| q.toggle_collapsed(&key));
            }),
        );
        cx.semantics(
            node,
            Semantics::new(Role::Row)
                .name(format!("{}: {shown}, {count} {}", col.title, labels.grid_rows))
                .description(spoken.join(", "))
                .expanded(!*collapsed)
                .active(s.focus_row == Some(self.index)),
        );
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}

impl DataGrid {
    fn handles(&self, cx: &mut BuildCx<'_>) -> Handles {
        let initial = {
            let mut q = self.initial.clone();
            q.hidden = self.columns.iter().map(|c| !c.visible).collect();
            q
        };
        let widths0: Vec<f32> = self.columns.iter().map(|c| c.width).collect();
        let query = cx.use_state(move || initial);
        let widths = cx.use_state(move || widths0);
        let first_arrangement =
            Arrangement::new(&self.columns, self.group_panel.unwrap_or(self.toolbar), self.page_size);
        let arrange = cx.use_state(move || first_arrangement);
        let own_selection: State<BTreeSet<usize>> = cx.use_state(BTreeSet::new);
        let focus_row = cx.use_state(|| None);
        let anchor = cx.use_state(|| Rc::new(Cell::new(None))).get();
        let reveal = cx.use_state(|| Rc::new(Cell::new(None))).get();
        let editing = cx.use_state(|| None);
        let edit_buffer = cx.use_state(String::new);
        let resize = cx.use_state(|| Rc::new(Cell::new(None))).get();
        Handles {
            query,
            widths,
            selection: self.selection.clone().unwrap_or_else(|| own_selection.binding()),
            focus_row,
            anchor,
            reveal,
            editing,
            edit_buffer,
            resize,
            modifiers: cx.modifiers(),
            arrange,
            drag: cx.use_state(|| None),
            press: cx.use_state(|| Rc::new(Cell::new((Point::ZERO, false)))).get(),
            dragged: cx.use_state(|| Rc::new(Cell::new(false))).get(),
            zones: cx.use_state(|| Rc::new(RefCell::new(Zones::default()))).get(),
            menu: cx.use_state(|| None),
            filter_open: cx.use_state(|| None),
            filter_chosen: cx.use_state(BTreeSet::new),
            filter_search: cx.use_state(String::new),
            page: cx.use_state(|| 0),
            saving: cx.use_state(|| false),
            scenario_name: cx.use_state(String::new),
            active_scenario: cx.use_state(|| None),
            original: cx.use_state(|| Rc::new(RefCell::new(None))).get(),
            seen_layout: cx.use_state(|| Rc::new(RefCell::new(None))).get(),
            summary_edit: cx.use_state(|| None),
            summary_draft: cx.use_state(String::new),
            formula_cache: cx.use_state(|| Rc::new(RefCell::new(FormulaCache::default()))).get(),
        }
    }

    fn toolbar_view(
        &self,
        cx: &mut BuildCx<'_>,
        h: &Handles,
        result: &QueryResult,
        rows: &Rc<Vec<Row>>,
        total: usize,
    ) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let mut items = Vec::new();
        if self.search_panel {
            let (read, write) = (h.query.clone(), h.query.clone());
            let search = Binding::new(
                move || read.with(|q| q.search.clone()),
                move |t| write.update(|q| q.search = t),
            );
            items.push(
                cx.child(
                    &TextField::new(search)
                        .placeholder(labels.grid_search.clone())
                        .icon(IconName::Search)
                        .compact()
                        .width(240.0),
                ),
            );
        }
        let query = h.query.peek().clone();
        if self.remote.is_none() && !h.arrange.peek().group_panel {
            let mut group =
                DropdownMenu::new(labels.grid_group.clone()).icon(IconName::Layers).size(ButtonSize::Small);
            for (c, col) in
                self.columns.iter().enumerate().filter(|(c, col)| col.groupable && !query.is_hidden(*c))
            {
                let q = h.query.clone();
                let mut item =
                    MenuItem::new(col.title.clone()).on_select(move || q.update(|q| q.toggle_group(c)));
                if query.group_by.contains(&c) {
                    item = item.icon(IconName::Check);
                }
                group = group.item(item);
            }
            if !query.group_by.is_empty() {
                let q = h.query.clone();
                group = group.separator().item(
                    MenuItem::new(labels.grid_clear_grouping.clone()).icon(IconName::X).on_select(
                        move || {
                            q.update(|q| {
                                q.group_by.clear();
                                q.collapsed.clear();
                            })
                        },
                    ),
                );
            }
            items.push(cx.child(&group));
        }
        if let Some(scenarios) = &self.scenarios {
            items.push(self.scenario_menu(cx, h, scenarios));
        }
        let mut chooser =
            DropdownMenu::new(labels.grid_columns.clone()).icon(IconName::Table).size(ButtonSize::Small);
        for (c, col) in self.columns.iter().enumerate() {
            let q = h.query.clone();
            let item = MenuItem::new(col.title.clone()).on_select(move || {
                q.update(|q| {
                    if q.hidden.len() <= c {
                        q.hidden.resize(c + 1, false);
                    }
                    q.hidden[c] = !q.hidden[c];
                })
            });
            chooser = chooser.item(if query.is_hidden(c) { item } else { item.icon(IconName::Check) });
        }
        items.push(cx.child(&chooser));
        if !query.sort.is_empty()
            || query.filters.iter().any(|f| !f.trim().is_empty())
            || !query.search.is_empty()
            || !query.group_by.is_empty()
        {
            let q = h.query.clone();
            items.push(
                cx.child(
                    &Button::new(labels.grid_reset.clone())
                        .style(ButtonStyle::Ghost)
                        .size(ButtonSize::Small)
                        .icon(IconName::RotateCcw)
                        .on_click(move || {
                            q.update(|q| {
                                q.sort.clear();
                                q.filters.clear();
                                q.search.clear();
                                q.group_by.clear();
                                q.collapsed.clear();
                            })
                        }),
                ),
            );
        }
        items.push(cx.child(&Spacer::new()));
        let selected = h.selection.get().len();
        let fmt = crate::grid::model::NumberFormat::default();
        let mut status = format!(
            "{} {} {} {}",
            fmt.format(result.matched.len() as f64),
            labels.grid_of,
            fmt.format(rows.len() as f64),
            labels.grid_rows
        );
        if self.remote.is_some() {
            status = format!("{} {}", fmt.format(total as f64), labels.grid_rows);
        }
        if selected > 0 {
            status.push_str(&format!(" · {} {}", fmt.format(selected as f64), labels.grid_selected));
        }
        items.push(util::label(cx, &status, TextStyle::Callout, None, theme.color(Semantic::InkMuted)));
        if let Some(export) = &self.on_export {
            let (export, columns, rows, q, matched) =
                (export.clone(), self.columns.clone(), rows.clone(), query.clone(), result.matched.clone());
            items.push(
                cx.child(
                    &Button::new(labels.grid_export.clone())
                        .style(ButtonStyle::Secondary)
                        .size(ButtonSize::Small)
                        .icon(IconName::Download)
                        .on_click(move || export(to_csv(&columns, &rows, &q, &matched))),
                ),
            );
        }
        cx.container(
            LayoutStyle {
                padding: Insets::symmetric(10.0, 12.0),
                spacing: 8.0,
                align: Align::Center,
                wrap: true,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &items,
            Paint::None,
        )
    }

    fn scenario_menu(
        &self,
        cx: &mut BuildCx<'_>,
        h: &Handles,
        scenarios: &Binding<Vec<GridScenario>>,
    ) -> NodeId {
        let labels = cx.labels();
        let active = h.active_scenario.get();
        let title = match &active {
            Some(name) => format!("{}: {name}", labels.grid_scenario),
            None => labels.grid_scenario.clone(),
        };
        let mut menu = DropdownMenu::new(title).icon(IconName::Bookmark).size(ButtonSize::Small);
        let columns = self.columns.clone();
        for scenario in scenarios.get() {
            let on = active.as_deref() == Some(scenario.name.as_str());
            let (h2, columns, name) = (h.clone(), columns.clone(), scenario.name.clone());
            let item = MenuItem::new(scenario.name.clone()).on_select(move || {
                apply_layout(&h2, &columns, &scenario.layout);
                h2.active_scenario.set(Some(name.clone()));
            });
            menu = menu.item(if on { item.icon(IconName::Check) } else { item });
        }
        if !scenarios.get().is_empty() {
            menu = menu.separator();
        }
        let saving = h.saving.clone();
        let name = h.scenario_name.clone();
        menu = menu.item(MenuItem::new(labels.grid_scenario_save.clone()).icon(IconName::Save).on_select(
            move || {
                name.set(String::new());
                saving.set(true);
            },
        ));
        if let Some(current) = active {
            let (h2, columns, list, shown) = (h.clone(), columns.clone(), scenarios.clone(), current.clone());
            menu = menu.item(
                MenuItem::new(format!("{} {shown}", labels.grid_scenario_update))
                    .icon(IconName::Check)
                    .on_select(move || {
                        let layout = current_layout(&h2, &columns);
                        list.update(|all| {
                            if let Some(s) = all.iter_mut().find(|s| s.name == current) {
                                s.layout = layout.clone();
                            }
                        });
                    }),
            );
            let (active, list, shown2) = (h.active_scenario.clone(), scenarios.clone(), shown.clone());
            menu = menu.item(
                MenuItem::new(format!("{} {shown}", labels.grid_scenario_delete))
                    .icon(IconName::Trash2)
                    .destructive()
                    .on_select(move || {
                        list.update(|all| all.retain(|s| s.name != shown2));
                        active.set(None);
                    }),
            );
        }
        let (h2, columns) = (h.clone(), columns);
        menu = menu.separator().item(
            MenuItem::new(labels.grid_scenario_original.clone()).icon(IconName::RotateCcw).on_select(
                move || {
                    if let Some(original) = h2.original.borrow().clone() {
                        apply_layout(&h2, &columns, &original);
                    }
                    h2.active_scenario.set(None);
                },
            ),
        );
        cx.child(&menu)
    }

    fn scenario_dialog(
        &self,
        cx: &mut BuildCx<'_>,
        h: &Handles,
        scenarios: &Binding<Vec<GridScenario>>,
    ) -> NodeId {
        let labels = cx.labels();
        let (h2, columns, list) = (h.clone(), self.columns.clone(), scenarios.clone());
        let save = move || {
            let name = h2.scenario_name.get().trim().to_owned();
            if name.is_empty() {
                return;
            }
            let layout = current_layout(&h2, &columns);
            list.update(|all| {
                all.retain(|s| s.name != name);
                all.push(GridScenario::new(name.clone(), layout.clone()));
            });
            h2.active_scenario.set(Some(name));
            h2.saving.set(false);
        };
        let on_enter = save.clone();
        cx.child(
            &Dialog::new(h.saving.binding(), labels.grid_scenario_save.trim_end_matches('…').to_owned())
                .content(
                    TextField::new(h.scenario_name.binding())
                        .placeholder(labels.grid_scenario_name.clone())
                        .autofocus()
                        .on_submit(move |_| on_enter()),
                )
                .action(Button::new(labels.grid_save.clone()).on_click(save))
                .width(380.0),
        )
    }

    fn group_panel_view(
        &self,
        cx: &mut BuildCx<'_>,
        h: &Handles,
        rows: &Rc<Vec<Row>>,
        facts: &Rc<Vec<(String, bool)>>,
    ) -> Option<NodeId> {
        let query = h.query.peek().clone();
        if self.remote.is_some() || !h.arrange.peek().group_panel {
            return None;
        }
        let theme = *cx.theme();
        let labels = cx.labels();
        let muted = theme.color(Semantic::InkMuted);
        let drag = *h.drag.peek();
        let target_place =
            drag.and_then(|d| if let DropTarget::Group(i) = d.target { Some(i) } else { None });
        let accent = theme.color(Semantic::Accent);
        let caret = |cx: &mut BuildCx<'_>| {
            cx.container(
                LayoutStyle { width: Some(2.0), height: Some(26.0), ..LayoutStyle::leaf() },
                &[],
                Paint::Quad(QuadPaint { corner_radius: 1.0, ..QuadPaint::new(accent) }),
            )
        };
        let moving = drag.and_then(|d| if let DragSource::Chip(c) = d.source { Some(c) } else { None });
        let mut items = Vec::new();
        let mut place = 0;
        for (i, &c) in query.group_by.iter().enumerate() {
            if i > 0 {
                items.push(util::icon(cx, IconName::ChevronRight, 14.0, muted));
            }
            if target_place == Some(place) && moving != Some(c) {
                items.push(caret(cx));
            }
            items
                .push(cx.keyed_child(c as u64 + 1, &GroupChip { index: i, column: c, grid: self, h, facts }));
            if moving != Some(c) {
                place += 1;
            }
        }
        if target_place == Some(place) {
            items.push(caret(cx));
        }
        if query.group_by.is_empty() || drag.is_some() {
            items.push(util::label(cx, &labels.grid_drag_hint, TextStyle::Callout, None, muted));
        }
        items.push(cx.child(&Spacer::new()));
        if !query.group_by.is_empty() {
            let (q, columns, rows2) = (h.query.clone(), self.columns.clone(), rows.clone());
            items.push(
                cx.child(
                    &Button::new(labels.grid_collapse_all.clone())
                        .style(ButtonStyle::Ghost)
                        .size(ButtonSize::Small)
                        .on_click(move || {
                            let keys = super::query::group_keys(&columns, &rows2, &q.peek());
                            q.update(|q| q.collapsed = keys.clone());
                        }),
                ),
            );
            let q = h.query.clone();
            items.push(
                cx.child(
                    &Button::new(labels.grid_expand_all.clone())
                        .style(ButtonStyle::Ghost)
                        .size(ButtonSize::Small)
                        .on_click(move || q.update(|q| q.collapsed.clear())),
                ),
            );
        }
        let zones = h.zones.clone();
        let fill =
            if target_place.is_some() { accent.with_alpha(0.08) } else { theme.color(Semantic::Canvas) };
        let line = theme.color(Semantic::Border);
        Some(cx.container(
            LayoutStyle {
                min_height: Some(GROUP_PANEL_HEIGHT),
                padding: Insets::symmetric(6.0, 12.0),
                spacing: 6.0,
                align: Align::Center,
                wrap: true,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &items,
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                zones.borrow_mut().panel = f;
                scene.push_quad(Quad::new(f, fill));
                scene.push_quad(Quad::new(Rect::new(f.x(), f.y(), f.width(), 1.0), line));
                scene.push_quad(Quad::new(Rect::new(f.x(), f.max_y() - 1.0, f.width(), 1.0), line));
            })),
        ))
    }

    /// A horizontally scrolled band (header, filter row, footer) kept in step
    /// with the body.
    fn band(
        cx: &mut BuildCx<'_>,
        cells: Vec<NodeId>,
        s: &Shared<'_>,
        offset_x: f32,
        fill: Color,
        top_line: bool,
        height: f32,
    ) -> NodeId {
        let theme = *cx.theme();
        let inner = cx.container(
            LayoutStyle {
                width: Some(s.total_width),
                height: Some(height),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &cells,
            Paint::None,
        );
        let line = theme.color(Semantic::Border);
        let outer = cx.container(
            LayoutStyle {
                height: Some(height),
                overflow_hidden: true,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[inner],
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                scene.push_quad(Quad::new(f, fill));
                let y = if top_line { f.y() } else { f.max_y() - 1.0 };
                scene.push_quad(Quad::new(Rect::new(f.x(), y, f.width(), 1.0), line));
            })),
        );
        cx.clip(outer);
        cx.translate(outer, Point::new(-offset_x, 0.0));
        outer
    }
}

fn key_handler(s: &Shared<'_>, mode: SelectionMode, clipboard: crate::Clipboard) -> crate::KeyHandler {
    let (result, h) = (s.result.clone(), s.h.clone());
    let visible: Vec<usize> = s.visible.to_vec();
    let columns: Vec<(usize, bool)> = s.visible.iter().map(|&c| (c, s.grid.columns[c].editable)).collect();
    let rows = s.rows_rc.clone();
    let display: Vec<GridColumn> = s.grid.columns.clone();
    let activate_row = s.grid.on_row_activate.clone();
    let page_size = s.arrangement.page_size;
    Rc::new(move |key, mods| {
        let len = result.rows.len();
        if len == 0 {
            return false;
        }
        let current = h.focus_row.get().unwrap_or(0).min(len - 1);
        let data_rows =
            || result.rows.iter().filter_map(|v| if let ViewRow::Data(r) = v { Some(*r) } else { None });
        if mods.ctrl || mods.meta {
            match key {
                // Copy the selected rows in view order, else the keyboard row.
                Key::Character(c) if c.eq_ignore_ascii_case("c") => {
                    let selection = h.selection.get();
                    let mut picked: Vec<usize> = data_rows().filter(|r| selection.contains(r)).collect();
                    if picked.is_empty()
                        && let ViewRow::Data(r) = result.rows[current]
                    {
                        picked.push(r);
                    }
                    if !picked.is_empty() {
                        clipboard.write(&to_tsv(&display, &rows, &visible, &picked));
                    }
                    return true;
                }
                Key::Character(c) if c.eq_ignore_ascii_case("a") && mode == SelectionMode::Multiple => {
                    h.selection.set(data_rows().collect());
                    return true;
                }
                _ => return false,
            }
        }
        // Enter / F2: edit the first editable cell, or activate the row, or toggle a group.
        let activate = |i: usize| match &result.rows[i] {
            ViewRow::Data(r) => {
                if let Some(&(c, _)) = columns.iter().find(|(_, editable)| *editable) {
                    let value = rows[*r].get(c).cloned().unwrap_or_default();
                    h.edit_buffer.set(display[c].display(&value));
                    h.editing.set(Some((*r, c)));
                } else if let Some(a) = &activate_row {
                    a(*r);
                }
            }
            ViewRow::Group { key, .. } => h.query.update(|q| q.toggle_collapsed(key)),
        };
        let move_to = |i: usize| {
            h.focus_row.set(Some(i));
            h.reveal.set(Some(i));
            if let Some(size) = page_size {
                h.page.set(i / size);
            }
            if let ViewRow::Data(r) = result.rows[i] {
                if mode == SelectionMode::Single {
                    h.selection.set(BTreeSet::from([r]));
                } else if mode == SelectionMode::Multiple && mods.shift {
                    h.selection.update(|s| {
                        s.insert(r);
                    });
                }
            }
        };
        match key {
            Key::ArrowDown => move_to((current + 1).min(len - 1)),
            Key::ArrowUp => move_to(current.saturating_sub(1)),
            Key::Home => move_to(0),
            Key::End => move_to(len - 1),
            Key::PageDown => move_to((current + 15).min(len - 1)),
            Key::PageUp => move_to(current.saturating_sub(15)),
            Key::Space => match &result.rows[current] {
                ViewRow::Data(r) if mode == SelectionMode::Multiple => h.selection.update(|s| {
                    if !s.remove(r) {
                        s.insert(*r);
                    }
                }),
                ViewRow::Group { key, .. } => h.query.update(|q| q.toggle_collapsed(key)),
                _ => return false,
            },
            Key::ArrowLeft | Key::ArrowRight => match &result.rows[current] {
                ViewRow::Group { key: group_key, collapsed, .. } => {
                    let expand = matches!(key, Key::ArrowRight);
                    if *collapsed == expand {
                        h.query.update(|q| q.toggle_collapsed(group_key));
                    }
                }
                _ => return false,
            },
            Key::Enter => activate(current),
            Key::Other(k) if k.contains("F2") => activate(current),
            _ => return false,
        }
        true
    })
}

fn current_layout(h: &Handles, columns: &[GridColumn]) -> GridLayout {
    layout::capture(columns, &h.query.peek(), &h.widths.peek(), &h.arrange.peek())
}

fn apply_layout(h: &Handles, columns: &[GridColumn], wanted: &GridLayout) {
    let (mut query, mut widths, mut arrange) =
        (h.query.peek().clone(), h.widths.peek().clone(), h.arrange.peek().clone());
    layout::apply(wanted, columns, &mut query, &mut widths, &mut arrange);
    h.query.set(query);
    h.widths.set(widths);
    h.arrange.set(arrange);
    h.page.set(0);
}

/// The keys of rows that stay on a server: there is no list of them to walk.
fn remote_key_handler(h: &Handles, len: usize, mode: SelectionMode) -> crate::KeyHandler {
    let h = h.clone();
    Rc::new(move |key, _| {
        if len == 0 {
            return false;
        }
        let current = h.focus_row.get().unwrap_or(0).min(len - 1);
        let next = match key {
            Key::ArrowDown => (current + 1).min(len - 1),
            Key::ArrowUp => current.saturating_sub(1),
            Key::Home => 0,
            Key::End => len - 1,
            Key::PageDown => (current + 15).min(len - 1),
            Key::PageUp => current.saturating_sub(15),
            _ => return false,
        };
        h.focus_row.set(Some(next));
        h.reveal.set(Some(next));
        if mode == SelectionMode::Single {
            h.selection.set(BTreeSet::from([next]));
        }
        true
    })
}

impl View for DataGrid {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let h = self.handles(cx);
        // A click that came with letting go of a drag has been and gone.
        h.dragged.set(false);
        // Someone else changed the arrangement, or it comes first from outside.
        if h.original.borrow().is_none() {
            *h.original.borrow_mut() = Some(current_layout(&h, &self.columns));
        }
        if let Some(binding) = &self.layout {
            let outside = binding.get();
            let changed = h.seen_layout.borrow().as_ref() != Some(&outside);
            if changed && outside != GridLayout::default() {
                apply_layout(&h, &self.columns, &outside);
            }
            *h.seen_layout.borrow_mut() = Some(outside);
        }
        if let Some(remote) = &self.remote {
            remote.watch();
        }
        let rows = self.data.rows();
        let query = h.query.get();
        let arrangement = h.arrange.get();
        let cache: Rc<RefCell<Option<Cached>>> = cx.use_state(|| Rc::new(RefCell::new(None))).get();
        let result = if self.remote.is_some() {
            let totals = self.remote.as_ref().map(RemoteRows::totals).unwrap_or_default();
            Rc::new(QueryResult { totals, ..QueryResult::default() })
        } else {
            let mut slot = cache.borrow_mut();
            match &*slot {
                Some(c)
                    if c.query == query
                        && Rc::ptr_eq(&c.rows, &rows)
                        && c.summaries == arrangement.summaries =>
                {
                    c.result.clone()
                }
                _ => {
                    let _span = tracing::trace_span!("ravenrs::datagrid::query", rows = rows.len()).entered();
                    // What each column sums up can have changed since it was declared.
                    let mut columns = self.columns.clone();
                    for (column, summary) in columns.iter_mut().zip(&arrangement.summaries) {
                        column.summary = *summary;
                    }
                    let result = Rc::new(run_query(&columns, &rows, &query));
                    *slot = Some(Cached {
                        summaries: arrangement.summaries.clone(),
                        query: query.clone(),
                        rows: rows.clone(),
                        result: result.clone(),
                    });
                    result
                }
            }
        };
        {
            // Answers are about these rows and these formulas.
            let about = (Rc::as_ptr(&result) as usize, arrangement.formulas.clone());
            let mut cache = h.formula_cache.borrow_mut();
            if cache.about != about {
                cache.about = about;
                cache.answers.clear();
            }
        }
        let by_formula = arrangement.formulas.iter().any(Option::is_some);
        let ranges = Rc::new(if by_formula { group_ranges(&result) } else { Vec::new() });
        let mut widths = h.widths.get();
        let visible: Vec<usize> = arrangement.visible(&query);
        // The first column takes in the steps of the groups, so its text keeps its room.
        if let Some(&first) = visible.first() {
            widths[first] += query.group_by.len() as f32 * GROUP_INDENT;
        }
        let select_width = if self.selection_mode == SelectionMode::Multiple { 44.0 } else { 0.0 };
        let fill_seen: Rc<Cell<(f32, f32)>> = cx.use_state(|| Rc::new(Cell::new((0.0, 0.0)))).get();
        if let Some(&fill) = visible.iter().find(|&&c| self.columns[c].fill) {
            match cx.scroll_viewport() {
                Some(view) => {
                    let used = select_width + visible.iter().map(|&c| widths[c]).sum::<f32>();
                    widths[fill] += (view.width - used).max(0.0);
                    // This build saw the last layout's width. Nothing else asks for a build
                    // after a layout that changed it, as when a sidebar closes or the
                    // window grows.
                    let seen = (view.width, cx.viewport().width);
                    if fill_seen.replace(seen) != seen {
                        let now = cx.now();
                        cx.wake_at(now);
                    }
                }
                // The width comes from the first layout, and nothing else asks for a second build.
                None => {
                    let now = cx.now();
                    cx.wake_at(now);
                }
            }
        }
        let total_width = select_width + visible.iter().map(|&c| widths[c]).sum::<f32>();
        let selected = h.selection.get();
        let focus_row = h.focus_row.get();
        let editing = h.editing.get();
        let offset = cx.scroll_offset();
        let mut fixed = Vec::new();
        let mut x = select_width;
        for &c in &visible {
            if arrangement.pinned.get(c).copied().unwrap_or(false) {
                fixed.push((c, x));
                x += widths[c];
            }
        }
        let last_fixed = fixed.last().map(|(c, _)| *c);
        let fixed_width = if fixed.is_empty() { 0.0 } else { x };
        let drag = h.drag.get();
        let column_facts: Rc<Vec<(String, bool)>> =
            Rc::new(self.columns.iter().map(|c| (c.title.clone(), c.groupable)).collect());
        let lines: Rc<Vec<f32>> = Rc::new(if self.vertical_lines {
            let mut x = select_width;
            visible
                .iter()
                .map(|&c| {
                    x += widths[c];
                    x - 1.0
                })
                .collect()
        } else {
            Vec::new()
        });
        let shared = Shared {
            grid: self,
            rows: &rows,
            rows_rc: rows.clone(),
            result: result.clone(),
            visible: &visible,
            widths: &widths,
            total_width,
            select_width,
            selected: &selected,
            focus_row,
            editing,
            h: h.clone(),
            arrangement: &arrangement,
            fixed,
            last_fixed,
            fixed_width,
            offset_x: offset.x,
            indent: query.group_by.len() as f32 * GROUP_INDENT,
            column_facts: column_facts.clone(),
            drop_before: drag.and_then(|d| if let DropTarget::Column(b) = d.target { Some(b) } else { None }),
            lines,
            ranges,
        };

        // How many rows there are, and which of them this page shows.
        let total = self.remote.as_ref().map_or(result.rows.len(), RemoteRows::len);
        let page_size = arrangement.page_size;
        let pages = page_size.map_or(1, |size| total.div_ceil(size).max(1));
        let page = h.page.get().min(pages - 1);
        let base = page_size.map_or(0, |size| page * size);
        let len = page_size.map_or(total, |size| size.min(total.saturating_sub(base)));
        let shown_page: Rc<Cell<usize>> = cx.use_state(|| Rc::new(Cell::new(0))).get();

        let mut sections = Vec::new();
        if self.toolbar {
            sections.push(self.toolbar_view(cx, &h, &result, &rows, total));
        }
        if let Some(panel) = self.group_panel_view(cx, &h, &rows, &column_facts) {
            sections.push(panel);
        }
        if let Some(scenarios) = &self.scenarios {
            sections.push(self.scenario_dialog(cx, &h, scenarios));
        }

        // Header.
        // The last paint's places stay, for whatever opens under a header in this build.
        h.zones.borrow_mut().cells.retain(|c, _| visible.contains(c));
        let select_fixed = shared.fixed_width > 0.0;
        let mut header_cells = Vec::new();
        if select_fixed {
            header_cells.push(cx.container(
                LayoutStyle { width: Some(shared.fixed_width), ..LayoutStyle::leaf() },
                &[],
                Paint::None,
            ));
        }
        if select_width > 0.0 {
            let all = !result.matched.is_empty() && result.matched.iter().all(|r| selected.contains(r));
            let b = crate::inputs::check_box(cx, &theme, all, false);
            let surface = theme.color(Semantic::Surface);
            let cell = cx.container(
                LayoutStyle {
                    width: Some(select_width),
                    height: Some(HEADER_HEIGHT),
                    align: Align::Center,
                    justify: Justify::Center,
                    absolute: select_fixed.then_some((0.0, 0.0)),
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &[b],
                if select_fixed { Paint::Quad(QuadPaint::new(surface)) } else { Paint::None },
            );
            if select_fixed {
                cx.translate(cell, Point::new(offset.x, 0.0));
            }
            let (sel, matched) = (h.selection.clone(), result.matched.clone());
            cx.on_click(
                cell,
                Rc::new(move || {
                    sel.set(if all { BTreeSet::new() } else { matched.iter().copied().collect() })
                }),
            );
            cx.cursor(cell, CursorIcon::Pointer);
            let name = cx.labels().grid_select_row.clone();
            cx.semantics(cell, Semantics::new(Role::CheckBox).name(name).toggled(all));
            header_cells.push(cell);
        }
        let flowing: Vec<usize> = visible.iter().copied().filter(|c| shared.fixed_x(*c).is_none()).collect();
        for &c in flowing.iter().chain(shared.fixed.iter().map(|(c, _)| c)) {
            header_cells.push(cx.keyed_child(c as u64, &HeaderCell { column: c, shared: &shared }));
        }
        let zones = h.zones.clone();
        let header =
            Self::band(cx, header_cells, &shared, offset.x, Color::TRANSPARENT, false, HEADER_HEIGHT);
        let header_zone = cx.container(
            LayoutStyle {
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[header],
            Paint::Custom(Rc::new(move |f: Rect, _| zones.borrow_mut().header = f)),
        );
        sections.push(header_zone);

        if self.filter_row {
            let mut cells = Vec::new();
            if select_fixed {
                cells.push(cx.container(
                    LayoutStyle { width: Some(shared.fixed_width), ..LayoutStyle::leaf() },
                    &[],
                    Paint::None,
                ));
            } else if select_width > 0.0 {
                cells.push(cx.container(
                    LayoutStyle { width: Some(select_width), ..LayoutStyle::leaf() },
                    &[],
                    Paint::None,
                ));
            }
            for &c in flowing.iter().chain(shared.fixed.iter().map(|(c, _)| c)) {
                cells.push(cx.keyed_child(1000 + c as u64, &FilterCell { column: c, shared: &shared }));
            }
            sections.push(Self::band(cx, cells, &shared, offset.x, Color::TRANSPARENT, false, HEADER_HEIGHT));
        }

        // Body: only the rows in (and just around) the viewport are built.
        let rh = self.row_height;
        let viewport_h =
            cx.scroll_viewport().map_or(self.height.unwrap_or(cx.viewport().height), |s| s.height);
        if shown_page.replace(page) != page {
            cx.set_scroll_offset(Point::new(offset.x, 0.0));
        }
        if let Some(i) = h.reveal.take() {
            let top = i.saturating_sub(base) as f32 * rh;
            let y = if top < offset.y {
                Some(top)
            } else if top + rh > offset.y + viewport_h {
                Some(top + rh - viewport_h)
            } else {
                None
            };
            if let Some(y) = y {
                cx.set_scroll_offset(Point::new(offset.x, y.max(0.0)));
            }
        }
        let overscan = 6;
        let first = ((offset.y / rh).floor() as usize).saturating_sub(overscan).min(len);
        let last = (((offset.y + viewport_h) / rh).ceil() as usize + overscan).min(len);
        if let Some(remote) = &self.remote {
            let (from, to) = (base + first, base + last.max(first + 1));
            let now = cx.now();
            if let Some(when) = remote.want(from, to, &query, now) {
                cx.wake_at(when);
            }
        }
        let mut body_rows = Vec::with_capacity(last - first + 2);
        body_rows.push(cx.container(
            LayoutStyle { height: Some(first as f32 * rh), width: Some(total_width), ..LayoutStyle::leaf() },
            &[],
            Paint::None,
        ));
        for i in (first..last).map(|i| base + i) {
            if let Some(remote) = &self.remote {
                let node = match remote.row(i) {
                    Some(values) => {
                        cx.keyed_child(i as u64, &DataRow { index: i, row: i, values, shared: &shared })
                    }
                    None => cx.keyed_child(i as u64 | (1 << 61), &WaitingRow { shared: &shared }),
                };
                body_rows.push(node);
                continue;
            }
            let node = match &result.rows[i] {
                ViewRow::Data(r) => cx.keyed_child(
                    *r as u64,
                    &DataRow { index: i, row: *r, values: rows[*r].clone(), shared: &shared },
                ),
                ViewRow::Group { key, .. } => {
                    cx.keyed_child(hash_of(key) | (1 << 62), &GroupRow { index: i, shared: &shared })
                }
            };
            body_rows.push(node);
        }
        body_rows.push(cx.container(
            LayoutStyle {
                height: Some((len - last) as f32 * rh),
                width: Some(total_width),
                ..LayoutStyle::leaf()
            },
            &[],
            Paint::None,
        ));
        let failure = self.remote.as_ref().and_then(RemoteRows::error);
        if let Some(message) = &failure {
            let text = util::label(cx, message, TextStyle::Body, None, theme.color(Semantic::Danger));
            let remote = self.remote.clone();
            let again = cx.child(
                &Button::new(cx.labels().grid_try_again.clone())
                    .style(ButtonStyle::Secondary)
                    .size(ButtonSize::Small)
                    .on_click(move || {
                        if let Some(remote) = &remote {
                            remote.retry();
                        }
                    }),
            );
            body_rows.insert(
                0,
                cx.container(
                    LayoutStyle {
                        padding: Insets::uniform(16.0),
                        spacing: 12.0,
                        align: Align::Center,
                        ..LayoutStyle::stack(Axis::Horizontal)
                    },
                    &[text, again],
                    Paint::None,
                ),
            );
        } else if total == 0 {
            let empty = self
                .empty_text
                .clone()
                .filter(|_| self.remote.is_none() && self.data.rows().is_empty())
                .unwrap_or_else(|| cx.labels().grid_no_rows.clone());
            let msg = util::label(cx, &empty, TextStyle::Body, None, theme.color(Semantic::InkMuted));
            body_rows.push(cx.container(
                LayoutStyle { padding: Insets::uniform(24.0), ..LayoutStyle::stack(Axis::Horizontal) },
                &[msg],
                Paint::None,
            ));
        }
        let content = cx.container(
            LayoutStyle { width: Some(total_width), ..LayoutStyle::stack(Axis::Vertical) },
            &body_rows,
            Paint::None,
        );
        let body = cx.container(
            LayoutStyle {
                grow: 1.0,
                min_height: Some(0.0),
                overflow_hidden: true,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[content],
            Paint::None,
        );
        cx.scrollable_virtual(body, true);
        cx.semantics(
            body,
            Semantics::new(Role::Grid)
                .name(cx.labels().grid_name.clone())
                .description(format!("{total} {}", cx.labels().grid_rows)),
        );
        cx.focusable(body, 0.0);
        let clipboard = cx.clipboard();
        let keys = if self.remote.is_some() {
            remote_key_handler(&h, total, self.selection_mode)
        } else {
            key_handler(&shared, self.selection_mode, clipboard)
        };
        cx.on_key(body, keys);
        sections.push(body);

        // What is being filtered, at a glance, with a way out.
        let filters: Vec<String> = (0..self.columns.len())
            .filter(|c| !query.filter(*c).trim().is_empty())
            .map(|c| {
                let text = query.filter(c);
                if text.contains('[') {
                    text.to_owned()
                } else {
                    format!("[{}]: {text}", self.columns[c].title)
                }
            })
            .collect();
        if !filters.is_empty() {
            let labels = cx.labels();
            let muted = theme.color(Semantic::InkMuted);
            let icon = util::icon(cx, IconName::Filter, 14.0, theme.color(Semantic::Accent));
            let text = format!("{}: {}", labels.grid_filter_label, filters.join(" · "));
            let label = util::label(cx, &text, TextStyle::Callout, None, muted);
            let q = h.query.clone();
            let clear = cx.child(
                &Button::new(labels.grid_clear_filter.clone())
                    .style(ButtonStyle::Ghost)
                    .size(ButtonSize::Small)
                    .on_click(move || q.update(|q| q.filters.clear())),
            );
            let line = theme.color(Semantic::Border);
            sections.push(cx.container(
                LayoutStyle {
                    padding: Insets::symmetric(4.0, 12.0),
                    spacing: 8.0,
                    align: Align::Center,
                    align_self: Some(Align::Stretch),
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &[icon, label, clear],
                Paint::Custom(Rc::new(move |f: Rect, scene| {
                    scene.push_quad(Quad::new(Rect::new(f.x(), f.y(), f.width(), 1.0), line));
                })),
            ));
        }

        // Footer totals.
        if self.footer
            && (arrangement.summaries.iter().any(Option::is_some)
                || arrangement.formulas.iter().any(Option::is_some))
        {
            let mut cells = Vec::new();
            if select_fixed {
                cells.push(cx.container(
                    LayoutStyle { width: Some(shared.fixed_width), ..LayoutStyle::leaf() },
                    &[],
                    Paint::None,
                ));
            } else if select_width > 0.0 {
                cells.push(cx.container(
                    LayoutStyle { width: Some(select_width), ..LayoutStyle::leaf() },
                    &[],
                    Paint::None,
                ));
            }
            let surface = theme.color(Semantic::Surface);
            for &c in flowing.iter().chain(shared.fixed.iter().map(|(c, _)| c)) {
                cells.push(cx.keyed_child(2000 + c as u64, &FooterCell { column: c, shared: &shared }));
            }
            sections.push(Self::band(cx, cells, &shared, offset.x, surface, true, FOOTER_HEIGHT));
        }

        // The pager.
        if page_size.is_some() {
            let labels = cx.labels();
            let muted = theme.color(Semantic::InkMuted);
            let fmt = crate::grid::model::NumberFormat::default();
            let range = if total == 0 {
                String::new()
            } else {
                format!(
                    "{}–{} {} {}",
                    fmt.format((base + 1) as f64),
                    fmt.format((base + len) as f64),
                    labels.grid_of,
                    fmt.format(total as f64)
                )
            };
            let (read, write, reset) = (h.arrange.clone(), h.arrange.clone(), h.page.clone());
            let chosen = Binding::new(
                move || {
                    let now = read.peek().page_size.unwrap_or(PAGE_SIZES[0]);
                    PAGE_SIZES.iter().position(|s| *s == now).unwrap_or(0)
                },
                move |i: usize| {
                    write.update(|a| a.page_size = PAGE_SIZES.get(i).copied());
                    reset.set(0);
                },
            );
            let (per_page, picker, status, pager, gap) = (
                util::label(cx, &labels.grid_rows_per_page, TextStyle::Callout, None, muted),
                cx.child(
                    &Select::new(chosen, PAGE_SIZES.map(|s| s.to_string()))
                        .label(labels.grid_rows_per_page.clone())
                        .width(84.0),
                ),
                util::label(cx, &range, TextStyle::Callout, None, muted),
                cx.child(&Pagination::new(h.page.binding(), pages).compact().labels(
                    labels.grid_previous.clone(),
                    labels.grid_next.clone(),
                    labels.grid_page.clone(),
                )),
                cx.child(&Spacer::new()),
            );
            let line = theme.color(Semantic::Border);
            sections.push(cx.container(
                LayoutStyle {
                    padding: Insets::symmetric(6.0, 12.0),
                    spacing: 8.0,
                    align: Align::Center,
                    align_self: Some(Align::Stretch),
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &[status, gap, per_page, picker, pager],
                Paint::Custom(Rc::new(move |f: Rect, scene| {
                    scene.push_quad(Quad::new(Rect::new(f.x(), f.y(), f.width(), 1.0), line));
                })),
            ));
        }

        let border = theme.color(Semantic::Border);
        let surface = theme.color(Semantic::Surface);
        let root = cx.container(
            LayoutStyle {
                height: self.height,
                grow: if self.height.is_none() { 1.0 } else { 0.0 },
                min_height: Some(200.0),
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &sections,
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.lg,
                border: Some(ravenrs_core::Border { width: 1.0, color: border }),
                ..QuadPaint::new(surface)
            }),
        );
        cx.clip(root);

        // A column's summary is edited under its header.
        if let Some(c) = h.summary_edit.get().filter(|c| *c < self.columns.len()) {
            let close: Rc<dyn Fn()> = {
                let edit = h.summary_edit.clone();
                Rc::new(move || edit.set(None))
            };
            let at =
                h.zones.borrow().cells.get(&c).map_or_else(
                    || Point::new(offset.x + 40.0, 120.0),
                    |r| Point::new(r.x(), r.max_y() + 4.0),
                );
            let editor = SummaryEditor {
                column: c,
                columns: self.columns.clone(),
                rows: rows.clone(),
                matched: result.clone(),
                arrange: h.arrange.clone(),
                draft: h.summary_draft.clone(),
                close: close.clone(),
            };
            cx.overlay(root, Placement::At(at), &PanelOf(&editor), Some(close));
        }

        // What is being dragged follows the pointer, over everything.
        if let Some(d) = drag {
            let column = match d.source {
                DragSource::Column(c) | DragSource::Chip(c) => c,
            };
            let ghost = DragGhost {
                title: self.columns.get(column).map_or_else(String::new, |c| c.title.clone()),
                grouping: matches!(d.target, DropTarget::Group(_)),
                ungrouping: d.target == DropTarget::Ungroup,
            };
            let cancel = h.drag.clone();
            cx.overlay(
                root,
                Placement::At(Point::new(d.at.x + 12.0, d.at.y + 8.0)),
                &ghost,
                Some(Rc::new(move || cancel.set(None))),
            );
        }

        // What was arranged, for whoever keeps it.
        if let Some(binding) = &self.layout {
            let now = current_layout(&h, &self.columns);
            let seen = h.seen_layout.borrow().clone();
            if seen.as_ref() != Some(&now) {
                *h.seen_layout.borrow_mut() = Some(now.clone());
                binding.set(now);
            }
        }
        root
    }
}
