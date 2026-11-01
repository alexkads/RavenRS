//! The data grid view: toolbar, group panel, header, filter row, virtualized
//! body and footer.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Color, CursorIcon, Insets, Key, Point, Quad, Rect, Role};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Theme, Weight};

use super::model::{CellAlign, GridColumn, GridData, Row, SortDirection};
use super::query::{GridQuery, QueryResult, ViewRow, run_query, to_csv, to_tsv};
use crate::context::{BuildCx, DragPhase, NodeId, Paint, QuadPaint, Semantics};
use crate::{
    Badge, Button, ButtonSize, ButtonStyle, DropdownMenu, IconName, MenuItem, Spacer, TextField, View, util,
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
/// several columns, a filter row with operators, a search panel, grouping with
/// group summaries, footer totals, column resizing and visibility, single or
/// multiple selection, keyboard navigation, in-place editing, cell templates and
/// CSV export. Only the visible rows are built, so 100,000+ rows stay smooth.
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
    /// A grid over `data` with these columns; single selection, filter row, toolbar and footer on.
    pub fn new(data: GridData, columns: impl IntoIterator<Item = GridColumn>) -> Self {
        Self {
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
        }
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
}

fn cell_attrs(theme: &Theme) -> ravenrs_text::TextAttrs {
    let mut a = util::attrs(theme, TextStyle::Body, None);
    a.size = 14.0;
    a.line_height = 20.0;
    a
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
        let query = s.h.query.peek().clone();
        let sort = query.sort.iter().position(|(sc, _)| *sc == c).map(|i| (i, query.sort[i].1));
        let hovered = cx.is_hovered() && col.sortable;
        let ink = if sort.is_some() || hovered {
            theme.color(Semantic::Ink)
        } else {
            theme.color(Semantic::InkMuted)
        };
        let mut a = util::attrs(&theme, TextStyle::Callout, Some(Weight::SEMIBOLD));
        a.size = 13.0;
        let mut content = vec![cx.text(&col.title, &a, ink, LayoutStyle::leaf())];
        if let Some((priority, dir)) = sort {
            let icon = if dir == SortDirection::Ascending { IconName::ArrowUp } else { IconName::ArrowDown };
            content.push(util::icon(cx, icon, 13.0, theme.color(Semantic::Accent).lerp(ink, 0.3)));
            if query.sort.len() > 1 {
                content.push(util::label(cx, &(priority + 1).to_string(), TextStyle::Caption, None, ink));
            }
        }
        let width = s.widths[c];
        let mut label_style = cell_style(width - 6.0, 40.0, col.align);
        label_style.padding = Insets { left: 12.0, right: 6.0, ..Insets::ZERO };
        let label = cx.container(label_style, &content, Paint::None);
        let mut semantics = Semantics::new(Role::ColumnHeader).name(col.title.clone());
        if let Some((_, dir)) = sort {
            let labels = cx.labels();
            semantics = semantics.description(if dir == SortDirection::Ascending {
                labels.grid_sorted_ascending.clone()
            } else {
                labels.grid_sorted_descending.clone()
            });
        }
        cx.semantics(label, semantics);
        if col.sortable {
            let q = s.h.query.clone();
            let m = s.h.modifiers.clone();
            cx.on_click(
                label,
                Rc::new(move || {
                    let mods = m.get();
                    q.update(|q| q.toggle_sort(c, mods.shift || mods.ctrl || mods.meta));
                }),
            );
            cx.cursor(label, CursorIcon::Pointer);
        }
        // The resize handle: a thin divider that can be dragged.
        let line = theme.color(Semantic::Border);
        let handle = cx.container(
            LayoutStyle { width: Some(6.0), height: Some(40.0), ..LayoutStyle::leaf() },
            &[],
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                scene.push_quad(Quad::new(
                    Rect::new(f.center().x - 0.5, f.y() + 10.0, 1.0, f.height() - 20.0),
                    line,
                ));
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
                DragPhase::End => resize.set(None),
            }),
        );
        cx.cursor(handle, CursorIcon::ColResize);
        let fill = if hovered { theme.color(Semantic::Ink).with_alpha(0.04) } else { Color::TRANSPARENT };
        cx.container(
            LayoutStyle { width: Some(width), height: Some(40.0), ..LayoutStyle::stack(Axis::Horizontal) },
            &[label, handle],
            Paint::Quad(QuadPaint::new(fill)),
        )
    }
}

struct FilterCell<'a> {
    column: usize,
    shared: &'a Shared<'a>,
}

impl View for FilterCell<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
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
        let mut style = cell_style(s.widths[c], 40.0, CellAlign::Start);
        style.padding = Insets::symmetric(0.0, 6.0);
        style.align = Align::Center;
        style.overflow_hidden = false;
        cx.container(LayoutStyle { align: Align::Stretch, ..style }, &children, Paint::None)
    }
}

struct DataRow<'a> {
    index: usize,
    row: usize,
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
        if s.select_width > 0.0 {
            let b = crate::inputs::check_box(cx, &theme, selected, false);
            let cell = cx.container(
                LayoutStyle {
                    width: Some(s.select_width),
                    height: Some(rh),
                    align: Align::Center,
                    justify: Justify::Center,
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &[b],
                Paint::None,
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
            cells.push(cell);
        }
        let attrs = cell_attrs(&theme);
        for &c in s.visible {
            let col = &s.grid.columns[c];
            let value = s.rows[row].get(c).cloned().unwrap_or_default();
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
            }
            let cell = cx.container(LayoutStyle { align: Align::Center, ..style }, &[content], Paint::None);
            if s.editing != Some((row, c)) {
                cx.semantics(cell, Semantics::new(Role::Cell));
            }
            let result = s.result.clone();
            let h = s.h.clone();
            cx.on_click(cell, Rc::new(move || click_row(&result, &h, mode, row, index)));
            let (editable, h2, activate) = (col.editable, s.h.clone(), s.grid.on_row_activate.clone());
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
            cells.push(cell);
        }
        let ink = theme.color(Semantic::Ink);
        let fill = if selected {
            theme.color(Semantic::Accent).with_alpha(0.16)
        } else if hovered {
            ink.with_alpha(0.045)
        } else if s.grid.striped && index % 2 == 1 {
            ink.with_alpha(0.02)
        } else {
            Color::TRANSPARENT
        };
        let line = theme.color(Semantic::Border).faded(0.6);
        let focus = (s.focus_row == Some(index)).then(|| theme.color(Semantic::Accent));
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
        for (c, value) in summaries.iter().enumerate() {
            if let (Some(v), Some(summary)) = (value, s.grid.columns[c].summary)
                && !s.h.query.peek().is_hidden(c)
            {
                let other = &s.grid.columns[c];
                let text = format!(
                    "{} {}: {}",
                    labels.summary(summary),
                    other.title,
                    other.display_summary(summary, *v)
                );
                children.push(util::label(cx, &text, TextStyle::Callout, None, muted));
            }
        }
        let hovered = cx.is_hovered();
        let base = theme.color(Semantic::Control);
        let fill = if hovered { base.lerp(ink, 0.06) } else { base.faded(0.55) };
        let line = theme.color(Semantic::Border).faded(0.6);
        let node = cx.container(
            LayoutStyle {
                width: Some(s.total_width),
                height: Some(s.grid.row_height),
                padding: Insets { left: 12.0 + *level as f32 * 22.0, ..Insets::ZERO },
                spacing: 8.0,
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &children,
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
        }
    }

    fn toolbar_view(
        &self,
        cx: &mut BuildCx<'_>,
        h: &Handles,
        result: &QueryResult,
        rows: &Rc<Vec<Row>>,
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
                MenuItem::new(labels.grid_clear_grouping.clone()).icon(IconName::X).on_select(move || {
                    q.update(|q| {
                        q.group_by.clear();
                        q.collapsed.clear();
                    })
                }),
            );
        }
        items.push(cx.child(&group));
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

    fn group_panel(&self, cx: &mut BuildCx<'_>, h: &Handles) -> Option<NodeId> {
        let query = h.query.peek().clone();
        if query.group_by.is_empty() {
            return None;
        }
        let theme = *cx.theme();
        let mut items =
            vec![util::label(cx, "GROUPED BY", TextStyle::Caption, None, theme.color(Semantic::InkMuted))];
        for (i, &c) in query.group_by.iter().enumerate() {
            if i > 0 {
                items.push(util::icon(cx, IconName::ChevronRight, 14.0, theme.color(Semantic::InkMuted)));
            }
            let q = h.query.clone();
            let title = self.columns.get(c).map_or("?", |col| col.title.as_str()).to_owned();
            items.push(
                cx.child(
                    &Button::new(title)
                        .style(ButtonStyle::Secondary)
                        .size(ButtonSize::Small)
                        .trailing_icon(IconName::X)
                        .on_click(move || q.update(|q| q.toggle_group(c))),
                ),
            );
        }
        Some(cx.container(
            LayoutStyle {
                padding: Insets { top: 0.0, bottom: 10.0, left: 12.0, right: 12.0 },
                spacing: 8.0,
                align: Align::Center,
                wrap: true,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &items,
            Paint::None,
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

impl View for DataGrid {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let h = self.handles(cx);
        let rows = self.data.rows();
        let query = h.query.get();
        let cache: Rc<RefCell<Option<Cached>>> = cx.use_state(|| Rc::new(RefCell::new(None))).get();
        let result = {
            let mut slot = cache.borrow_mut();
            match &*slot {
                Some(c) if c.query == query && Rc::ptr_eq(&c.rows, &rows) => c.result.clone(),
                _ => {
                    let _span = tracing::trace_span!("ravenrs::datagrid::query", rows = rows.len()).entered();
                    let result = Rc::new(run_query(&self.columns, &rows, &query));
                    *slot = Some(Cached { query: query.clone(), rows: rows.clone(), result: result.clone() });
                    result
                }
            }
        };
        let widths = h.widths.get();
        let visible: Vec<usize> = (0..self.columns.len()).filter(|&c| !query.is_hidden(c)).collect();
        let select_width = if self.selection_mode == SelectionMode::Multiple { 44.0 } else { 0.0 };
        let total_width = select_width + visible.iter().map(|&c| widths[c]).sum::<f32>();
        let selected = h.selection.get();
        let focus_row = h.focus_row.get();
        let editing = h.editing.get();
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
        };

        let offset = cx.scroll_offset();
        let mut sections = Vec::new();
        if self.toolbar {
            sections.push(self.toolbar_view(cx, &h, &result, &rows));
        }
        if let Some(panel) = self.group_panel(cx, &h) {
            sections.push(panel);
        }

        // Header.
        let mut header_cells = Vec::new();
        if select_width > 0.0 {
            let all = !result.matched.is_empty() && result.matched.iter().all(|r| selected.contains(r));
            let b = crate::inputs::check_box(cx, &theme, all, false);
            let cell = cx.container(
                LayoutStyle {
                    width: Some(select_width),
                    height: Some(40.0),
                    align: Align::Center,
                    justify: Justify::Center,
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &[b],
                Paint::None,
            );
            let (sel, matched) = (h.selection.clone(), result.matched.clone());
            cx.on_click(
                cell,
                Rc::new(move || {
                    sel.set(if all { BTreeSet::new() } else { matched.iter().copied().collect() })
                }),
            );
            cx.cursor(cell, CursorIcon::Pointer);
            header_cells.push(cell);
        }
        for &c in &visible {
            header_cells.push(cx.keyed_child(c as u64, &HeaderCell { column: c, shared: &shared }));
        }
        let header_fill = theme.color(Semantic::Control).faded(0.6);
        sections.push(Self::band(cx, header_cells, &shared, offset.x, header_fill, false, 40.0));

        if self.filter_row {
            let mut cells = Vec::new();
            if select_width > 0.0 {
                cells.push(cx.container(
                    LayoutStyle { width: Some(select_width), ..LayoutStyle::leaf() },
                    &[],
                    Paint::None,
                ));
            }
            for &c in &visible {
                cells.push(cx.keyed_child(1000 + c as u64, &FilterCell { column: c, shared: &shared }));
            }
            sections.push(Self::band(cx, cells, &shared, offset.x, Color::TRANSPARENT, false, 40.0));
        }

        // Body: only the rows in (and just around) the viewport are built.
        let rh = self.row_height;
        let len = result.rows.len();
        let viewport_h =
            cx.scroll_viewport().map_or(self.height.unwrap_or(cx.viewport().height), |s| s.height);
        if let Some(i) = h.reveal.take() {
            let top = i as f32 * rh;
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
        let mut body_rows = Vec::with_capacity(last - first + 2);
        body_rows.push(cx.container(
            LayoutStyle { height: Some(first as f32 * rh), width: Some(total_width), ..LayoutStyle::leaf() },
            &[],
            Paint::None,
        ));
        for i in first..last {
            let node = match &result.rows[i] {
                ViewRow::Data(r) => {
                    cx.keyed_child(*r as u64, &DataRow { index: i, row: *r, shared: &shared })
                }
                ViewRow::Group { key, .. } => {
                    let k = key.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
                        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
                    });
                    cx.keyed_child(k | (1 << 62), &GroupRow { index: i, shared: &shared })
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
        if len == 0 {
            let empty = cx.labels().grid_no_rows.clone();
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
                .description(format!("{len} {}", cx.labels().grid_rows)),
        );
        cx.focusable(body, 0.0);
        let clipboard = cx.clipboard();
        cx.on_key(body, key_handler(&shared, self.selection_mode, clipboard));
        sections.push(body);

        // Footer totals.
        if self.footer && self.columns.iter().any(|c| c.summary.is_some()) {
            let mut cells = Vec::new();
            if select_width > 0.0 {
                cells.push(cx.container(
                    LayoutStyle { width: Some(select_width), ..LayoutStyle::leaf() },
                    &[],
                    Paint::None,
                ));
            }
            let mut a = util::attrs(&theme, TextStyle::Callout, Some(Weight::SEMIBOLD));
            a.size = 13.0;
            for &c in &visible {
                let col = &self.columns[c];
                let mut lines = Vec::new();
                if let (Some(s), Some(v)) = (col.summary, result.totals.get(c).copied().flatten()) {
                    let muted = theme.color(Semantic::InkMuted);
                    let name = cx.labels().summary(s).to_uppercase();
                    lines.push(util::label(cx, &name, TextStyle::Caption, None, muted));
                    lines.push(cx.text(
                        &col.display_summary(s, v),
                        &a,
                        theme.color(Semantic::Ink),
                        LayoutStyle::leaf(),
                    ));
                }
                let align = match col.align {
                    CellAlign::Start => Align::Start,
                    CellAlign::Center => Align::Center,
                    CellAlign::End => Align::End,
                };
                cells.push(cx.container(
                    LayoutStyle {
                        width: Some(widths[c]),
                        height: Some(52.0),
                        padding: Insets::symmetric(0.0, 12.0),
                        spacing: 2.0,
                        align,
                        justify: Justify::Center,
                        overflow_hidden: true,
                        ..LayoutStyle::stack(Axis::Vertical)
                    },
                    &lines,
                    Paint::None,
                ));
            }
            let fill = theme.color(Semantic::Control).faded(0.6);
            sections.push(Self::band(cx, cells, &shared, offset.x, fill, true, 52.0));
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
        root
    }
}
