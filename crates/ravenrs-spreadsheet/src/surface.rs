//! The cells: painted for the visible window only, with headers that stay in place.

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_core::{Color, Key, Modifiers, Point, Quad, Rect, Role, Size};
use ravenrs_layout::{Align, Axis as LayoutAxis, LayoutStyle};
use ravenrs_reactive::Binding;
use ravenrs_sheet::base::{CellRef, MAX_COLS, MAX_ROWS, RangeRef, Value, col_to_letters};
use ravenrs_sheet::engine::display;
use ravenrs_sheet::model::{
    BorderLine, BorderStyle, HAlign, PatternType, Sheet, Underline, VAlign, Workbook,
};
use ravenrs_style::{Semantic, TextStyle, Theme};
use ravenrs_text::TextAttrs;
use ravenrs_widgets::{BuildCx, Drag, DragPhase, NodeId, Paint, PaintCx, Semantics, TextField, View};
use serde_json::json;

use crate::geometry::{Axis, HEADER_HEIGHT, ROW_HEADER_WIDTH};
use crate::state::{EditPlace, SpreadsheetState};

/// Lines kept past the used area and the visible window, so there is always room to scroll.
const SPARE_ROWS: u32 = 100;
const SPARE_COLUMNS: u32 = 26;
/// How close to a column header's edge a drag resizes the column.
const RESIZE_GRIP: f32 = 4.0;
/// Narrowest a column gets by dragging.
const MIN_COLUMN: f32 = 8.0;
/// Empty cells to the right that left-aligned text may run into.
const OVERFLOW_CELLS: u32 = 12;

/// Where a point falls on the surface.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Hit {
    Corner,
    Column(u32),
    /// Near the right edge of a column's header.
    ColumnEdge(u32),
    Row(u32),
    Cell(CellRef),
}

/// Rows, columns and scroll, enough to turn a point into a cell.
#[derive(Clone, Debug)]
struct Grid {
    rows: Axis,
    columns: Axis,
    offset: Point,
}

impl Grid {
    fn hit(&self, local: Point) -> Hit {
        let in_header = local.y < HEADER_HEIGHT;
        let in_row_header = local.x < ROW_HEADER_WIDTH;
        let x = local.x - ROW_HEADER_WIDTH + self.offset.x;
        let y = local.y - HEADER_HEIGHT + self.offset.y;
        match (in_header, in_row_header) {
            (true, true) => Hit::Corner,
            (true, false) => {
                let col = self.columns.at(x);
                let right = self.columns.start(col) + self.columns.size(col);
                let left = self.columns.start(col);
                if right - x <= RESIZE_GRIP {
                    Hit::ColumnEdge(col)
                } else if x - left <= RESIZE_GRIP && col > 0 {
                    Hit::ColumnEdge(col - 1)
                } else {
                    Hit::Column(col)
                }
            }
            (false, true) => Hit::Row(self.rows.at(y)),
            (false, false) => Hit::Cell(CellRef::new(self.rows.at(y), self.columns.at(x))),
        }
    }

    /// The cell's rectangle relative to the surface.
    fn cell_rect(&self, c: CellRef) -> Rect {
        Rect::new(
            ROW_HEADER_WIDTH + self.columns.start(c.col) - self.offset.x,
            HEADER_HEIGHT + self.rows.start(c.row) - self.offset.y,
            self.columns.size(c.col),
            self.rows.size(c.row),
        )
    }

    fn range_rect(&self, r: RangeRef) -> Rect {
        let a = self.cell_rect(r.start);
        let b = self.cell_rect(r.end);
        Rect::new(a.x(), a.y(), b.x() + b.width() - a.x(), b.y() + b.height() - a.y())
    }
}

/// A line along one edge of a cell.
#[derive(Clone, Copy, Debug)]
struct Edge {
    width: f32,
    color: Color,
}

/// One cell, ready to paint.
#[derive(Clone, Debug)]
struct CellLook {
    rect: Rect,
    /// Where its text may draw: wider than `rect` when text runs into empty neighbors.
    clip: Rect,
    fill: Option<Color>,
    text: String,
    color: Color,
    attrs: TextAttrs,
    align: HAlign,
    valign: VAlign,
    underline: bool,
    strike: bool,
    /// Left, top, right, bottom.
    edges: [Option<Edge>; 4],
}

/// Everything the paint function needs, worked out during the build.
#[derive(Debug)]
struct Picture {
    size: Size,
    grid: Grid,
    rows: Vec<u32>,
    columns: Vec<u32>,
    cells: Vec<CellLook>,
    /// Merged areas, which hide the gridlines inside them.
    merges: Vec<(Rect, Option<Color>)>,
    gridlines: bool,
    selection: Vec<Rect>,
    active: Rect,
    selected_rows: Vec<(u32, u32)>,
    selected_columns: Vec<(u32, u32)>,
    theme: Theme,
}

/// The scrolling grid of cells, with its headers.
pub(crate) struct Surface {
    pub(crate) state: SpreadsheetState,
    pub(crate) name: String,
    pub(crate) read_only: bool,
}

impl View for Surface {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let _ = self.state.version();
        let edit = self.state.edit().get();
        let theme = *cx.theme();
        let offset = cx.scroll_offset();
        let window = cx.viewport();
        let size = cx.scroll_viewport().unwrap_or(Size::new(window.width, window.height * 0.6));
        let selection = self.state.selection_now();

        let (picture, content) = self.state.read(|wb| {
            let sheet = wb.active();
            let empty = Sheet::new(String::from("Sheet1"));
            let sheet = sheet.unwrap_or(&empty);
            let grid = Grid { rows: Axis::rows(sheet, 1.0), columns: Axis::columns(sheet, 1.0), offset };
            picture(wb, sheet, grid, size, &selection, theme)
        });

        // Keeps the active cell in view when the keyboard moved it.
        let last_active = cx.use_state(|| selection.active);
        if *last_active.peek() != selection.active {
            last_active.set(selection.active);
            if let Some(to) = scroll_to_show(&picture.grid, selection.active, size) {
                cx.set_scroll_offset(to);
            }
        }
        // After an edit over the cell ends, typing goes to the grid again.
        let was_editing = cx.use_state(|| false);
        let editing_here = edit.as_ref().is_some_and(|e| e.place == EditPlace::Cell);
        if *was_editing.peek() != editing_here {
            was_editing.set(editing_here);
            if !editing_here {
                cx.request_focus();
            }
        }

        let grid = picture.grid.clone();
        let active_text = self.state.cell_text(&selection.active.a1());
        let mut children = Vec::new();
        if let Some(edit) = edit.filter(|e| e.place == EditPlace::Cell) {
            children.push(self.editor(cx, &grid, edit.cell));
        }
        let picture = Rc::new(picture);
        let surface = cx.container(
            LayoutStyle {
                absolute: Some((offset.x, offset.y)),
                width: Some(size.width),
                height: Some(size.height),
                ..LayoutStyle::leaf()
            },
            &children,
            Paint::Rich(Rc::new({
                let picture = picture.clone();
                move |frame: Rect, paint: &mut PaintCx<'_>| paint_picture(&picture, frame, paint)
            })),
        );
        cx.focusable(surface, 0.0);
        cx.semantics(
            surface,
            Semantics::new(Role::Grid)
                .name(self.name.clone())
                .value(format!("{} {}", selection.active.a1(), active_text))
                .read_only(self.read_only),
        );
        self.pointer(cx, surface, &grid);
        self.keys(cx, surface);

        let content = cx.container(
            LayoutStyle { width: Some(content.width), height: Some(content.height), ..LayoutStyle::leaf() },
            &[surface],
            Paint::None,
        );
        let body = cx.container(
            LayoutStyle {
                grow: 1.0,
                min_height: Some(0.0),
                overflow_hidden: true,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(LayoutAxis::Vertical)
            },
            &[content],
            Paint::None,
        );
        cx.scrollable_virtual(body, true);
        body
    }
}

impl Surface {
    /// The text field over the cell being edited.
    fn editor(&self, cx: &mut BuildCx<'_>, grid: &Grid, cell: CellRef) -> NodeId {
        let rect = grid.cell_rect(cell);
        let (read, write) = (self.state.clone(), self.state.clone());
        let (submit, cancel, keys) = (self.state.clone(), self.state.clone(), self.state.clone());
        let field = TextField::new(Binding::new(
            move || read.edit().get().map(|e| e.text).unwrap_or_default(),
            move |text: String| {
                write.edit().update(|e| {
                    if let Some(e) = e {
                        e.text = text
                    }
                })
            },
        ))
        .compact()
        .autofocus()
        .width(rect.width().max(120.0))
        .on_submit(move |_| submit.commit_edit(1, 0))
        .on_cancel(move || cancel.cancel_edit())
        .intercept_keys(Rc::new(move |key: &Key, modifiers: Modifiers| match key {
            Key::Tab => {
                keys.commit_edit(0, if modifiers.shift { -1 } else { 1 });
                true
            }
            Key::Enter if modifiers.shift => {
                keys.commit_edit(-1, 0);
                true
            }
            _ => false,
        }));
        let node = cx.child(&field);
        cx.container(
            LayoutStyle { absolute: Some((rect.x().max(0.0), rect.y().max(0.0))), ..LayoutStyle::leaf() },
            &[node],
            Paint::None,
        )
    }

    fn pointer(&self, cx: &mut BuildCx<'_>, surface: NodeId, grid: &Grid) {
        // Where the latest press landed, relative to the surface: a drag starts on every press,
        // so this is where a click or a double click happened.
        // Both outlive this build: the press itself changes the selection, which rebuilds.
        let pressed: Rc<Cell<Option<Point>>> = cx.use_state(|| Rc::new(Cell::new(None))).peek().clone();
        let start: Rc<Cell<Option<Hit>>> = cx.use_state(|| Rc::new(Cell::new(None))).peek().clone();
        let grid = Rc::new(grid.clone());
        cx.on_double_click(surface, {
            let (state, grid, pressed, read_only) =
                (self.state.clone(), grid.clone(), pressed.clone(), self.read_only);
            Rc::new(move || match pressed.get().map(|local| grid.hit(local)) {
                Some(Hit::Cell(_)) if !read_only => state.begin_edit(None, EditPlace::Cell),
                Some(Hit::ColumnEdge(col)) if !read_only => {
                    let cols = format!("{0}:{0}", col_to_letters(col));
                    let _ = state.execute("home.autofitColumnWidth", json!({ "cols": cols }));
                }
                _ => {}
            })
        });
        // A drag either picks a range of cells or, from a header's edge, sizes a column.
        let (state, read_only, drag_modifiers) = (self.state.clone(), self.read_only, cx.modifiers());
        cx.on_drag(
            surface,
            Rc::new(move |drag: Drag| {
                let local = Point::new(drag.position.x - drag.frame.x(), drag.position.y - drag.frame.y());
                match drag.phase {
                    DragPhase::Start => {
                        pressed.set(Some(local));
                        let hit = grid.hit(local);
                        let extend = drag_modifiers.get().shift;
                        // Shift starts from the active cell, as a Shift-click does.
                        let anchor = match hit {
                            Hit::Cell(_) if extend => Hit::Cell(state.selection_now().active),
                            other => other,
                        };
                        start.set(Some(anchor));
                        if !matches!(hit, Hit::ColumnEdge(_)) {
                            click(&state, &grid, hit, extend);
                        }
                    }
                    DragPhase::Move | DragPhase::End => match start.get() {
                        Some(Hit::Cell(from)) => {
                            if let Hit::Cell(to) = grid.hit(clamped(local, drag.frame.size)) {
                                state.select_range(RangeRef::new(from, to), from);
                            }
                        }
                        Some(Hit::ColumnEdge(col)) if !read_only && drag.phase == DragPhase::End => {
                            let left = ROW_HEADER_WIDTH + grid.columns.start(col) - grid.offset.x;
                            let width = (local.x - left).max(MIN_COLUMN).round();
                            let cols = format!("{0}:{0}", col_to_letters(col));
                            let _ =
                                state.execute("home.columnWidth", json!({ "cols": cols, "width": width }));
                        }
                        _ => {}
                    },
                    _ => {}
                }
                if drag.phase == DragPhase::End {
                    start.set(None);
                }
            }),
        );
    }

    fn keys(&self, cx: &mut BuildCx<'_>, surface: NodeId) {
        let state = self.state.clone();
        let clipboard = cx.clipboard();
        let read_only = self.read_only;
        cx.on_key(
            surface,
            Rc::new(move |key: &Key, m: Modifiers| {
                let command = m.ctrl || m.meta;
                match key {
                    Key::ArrowUp => step(&state, -1, 0, m),
                    Key::ArrowDown => step(&state, 1, 0, m),
                    Key::ArrowLeft => step(&state, 0, -1, m),
                    Key::ArrowRight => step(&state, 0, 1, m),
                    Key::PageUp | Key::PageDown => {
                        let dr = if *key == Key::PageUp { -1 } else { 1 };
                        let _ = state.execute(
                            "selection.move",
                            json!({ "dr": dr, "dc": 0, "page": true, "extend": m.shift }),
                        );
                    }
                    Key::Enter => state.move_by(if m.shift { -1 } else { 1 }, 0, false),
                    Key::Tab => state.move_by(0, if m.shift { -1 } else { 1 }, false),
                    Key::Home => {
                        let active = state.selection_now().active;
                        let to = if command { CellRef::new(0, 0) } else { CellRef::new(active.row, 0) };
                        state.select_range(RangeRef::new(to, to), to);
                    }
                    Key::Delete | Key::Backspace if !read_only => {
                        let _ = state.execute("edit.clearContents", json!({}));
                    }
                    Key::Other(name) if name == "F2" && !read_only => state.begin_edit(None, EditPlace::Cell),
                    Key::Character(text) if command => {
                        return shortcut(&state, &clipboard, &text.to_lowercase(), m.shift, read_only);
                    }
                    Key::Character(text) if !read_only && !text.is_empty() => {
                        if text.contains(['\t', '\n']) {
                            let _ = state.execute("edit.paste", json!({ "text": text }));
                        } else {
                            state.begin_edit(Some(text.clone()), EditPlace::Cell);
                        }
                    }
                    _ => return false,
                }
                true
            }),
        );
    }
}

fn step(state: &SpreadsheetState, dr: i64, dc: i64, m: Modifiers) {
    let jump = m.ctrl || m.meta;
    let _ = state.execute("selection.move", json!({ "dr": dr, "dc": dc, "extend": m.shift, "jump": jump }));
}

/// Cmd or Ctrl with a letter.
fn shortcut(
    state: &SpreadsheetState,
    clipboard: &ravenrs_widgets::Clipboard,
    key: &str,
    shift: bool,
    read_only: bool,
) -> bool {
    let run = |command: &str| {
        let _ = state.execute(command, json!({}));
    };
    match key {
        "c" => copy(state, clipboard, "edit.copy"),
        "a" => run("edit.selectAll"),
        _ if read_only => return false,
        "z" if shift => run("edit.redo"),
        "z" => run("edit.undo"),
        "y" => run("edit.redo"),
        "b" => run("home.bold"),
        "i" => run("home.italic"),
        "u" => run("home.underline"),
        "x" => copy(state, clipboard, "edit.cut"),
        "v" => {
            let params = clipboard.read().map_or_else(|| json!({}), |text| json!({ "text": text }));
            let _ = state.execute("edit.paste", params);
        }
        _ => return false,
    }
    true
}

fn copy(state: &SpreadsheetState, clipboard: &ravenrs_widgets::Clipboard, command: &str) {
    if let Ok(reply) = state.execute(command, json!({}))
        && let Some(text) = reply.get("text").and_then(|t| t.as_str())
    {
        clipboard.write(text);
    }
}

fn click(state: &SpreadsheetState, grid: &Grid, hit: Hit, extend: bool) {
    let active = state.selection_now().active;
    let (range, at) = match hit {
        Hit::Corner => {
            let _ = state.execute("edit.selectAll", json!({}));
            return;
        }
        Hit::Column(col) | Hit::ColumnEdge(col) => {
            let from = if extend { active.col.min(col) } else { col };
            let to = if extend { active.col.max(col) } else { col };
            (
                RangeRef::new(CellRef::new(0, from), CellRef::new(MAX_ROWS - 1, to)),
                CellRef::new(grid.offset_row(), col),
            )
        }
        Hit::Row(row) => {
            let from = if extend { active.row.min(row) } else { row };
            let to = if extend { active.row.max(row) } else { row };
            (RangeRef::new(CellRef::new(from, 0), CellRef::new(to, MAX_COLS - 1)), CellRef::new(row, 0))
        }
        Hit::Cell(c) if extend => (RangeRef::new(active, c), active),
        Hit::Cell(c) => (RangeRef::new(c, c), c),
    };
    let active = if extend { active } else { at };
    state.select_range(range, active);
}

impl Grid {
    /// The first row in view, where a whole column selection puts its active cell.
    fn offset_row(&self) -> u32 {
        self.rows.at(self.offset.y)
    }
}

/// Keeps a drag that left the surface on its edge.
fn clamped(p: Point, size: Size) -> Point {
    Point::new(
        p.x.clamp(ROW_HEADER_WIDTH, size.width.max(ROW_HEADER_WIDTH)),
        p.y.clamp(HEADER_HEIGHT, size.height.max(HEADER_HEIGHT)),
    )
}

/// The scroll offset that shows `cell`, if it is not in view now.
fn scroll_to_show(grid: &Grid, cell: CellRef, size: Size) -> Option<Point> {
    let (x, w) = (grid.columns.start(cell.col), grid.columns.size(cell.col));
    let (y, h) = (grid.rows.start(cell.row), grid.rows.size(cell.row));
    let view_w = (size.width - ROW_HEADER_WIDTH).max(0.0);
    let view_h = (size.height - HEADER_HEIGHT).max(0.0);
    let mut to = grid.offset;
    if x < to.x {
        to.x = x;
    } else if x + w > to.x + view_w {
        to.x = (x + w - view_w).max(0.0);
    }
    if y < to.y {
        to.y = y;
    } else if y + h > to.y + view_h {
        to.y = (y + h - view_h).max(0.0);
    }
    (to != grid.offset).then_some(to)
}

/// Works out what the visible window shows, and the size of the whole scrollable area.
fn picture(
    wb: &Workbook,
    sheet: &Sheet,
    grid: Grid,
    size: Size,
    selection: &ravenrs_sheet::engine::Selection,
    theme: Theme,
) -> (Picture, Size) {
    let first_row = grid.rows.at(grid.offset.y);
    let first_col = grid.columns.at(grid.offset.x);
    let mut rows = Vec::new();
    let mut r = first_row;
    while r < grid.rows.count() && grid.rows.start(r) < grid.offset.y + size.height {
        if grid.rows.size(r) > 0.0 {
            rows.push(r);
        }
        r += 1;
    }
    let mut columns = Vec::new();
    let mut c = first_col;
    while c < grid.columns.count() && grid.columns.start(c) < grid.offset.x + size.width {
        if grid.columns.size(c) > 0.0 {
            columns.push(c);
        }
        c += 1;
    }
    let used = sheet.used_range();
    let extent_rows = (used.map_or(0, |u| u.end.row + 1).max(r) + SPARE_ROWS).min(MAX_ROWS);
    let extent_cols = (used.map_or(0, |u| u.end.col + 1).max(c) + SPARE_COLUMNS).min(MAX_COLS);
    let content = Size::new(
        ROW_HEADER_WIDTH + grid.columns.start(extent_cols),
        HEADER_HEIGHT + grid.rows.start(extent_rows),
    );

    let ink = theme.color(Semantic::Ink);
    let mut cells = Vec::new();
    let mut merges = Vec::new();
    for merge in &sheet.merges {
        let visible =
            rows.first().zip(rows.last()).is_some_and(|(a, b)| merge.end.row >= *a && merge.start.row <= *b)
                && columns
                    .first()
                    .zip(columns.last())
                    .is_some_and(|(a, b)| merge.end.col >= *a && merge.start.col <= *b);
        if visible {
            let style = wb.styles.get(sheet.style_id(merge.start));
            merges.push((grid.range_rect(*merge), fill_color(wb, style)));
            if let Some(look) = cell_look(wb, sheet, &grid, merge.start, Some(*merge), ink) {
                cells.push(look);
            }
        }
    }
    for &row in &rows {
        for &col in &columns {
            let at = CellRef::new(row, col);
            if sheet.merge_at(at).is_some() {
                continue;
            }
            if let Some(look) = cell_look(wb, sheet, &grid, at, None, ink) {
                cells.push(look);
            }
        }
    }

    let selection_rects =
        selection.ranges.iter().map(|r| grid.range_rect(clip_range(*r, extent_rows, extent_cols))).collect();
    let active = match sheet.merge_at(selection.active) {
        Some(m) => grid.range_rect(m),
        None => grid.cell_rect(selection.active),
    };
    let selected_rows = selection.ranges.iter().map(|r| (r.start.row, r.end.row)).collect();
    let selected_columns = selection.ranges.iter().map(|r| (r.start.col, r.end.col)).collect();
    let picture = Picture {
        size,
        grid,
        rows,
        columns,
        cells,
        merges,
        gridlines: sheet.show_gridlines,
        selection: selection_rects,
        active,
        selected_rows,
        selected_columns,
        theme,
    };
    (picture, content)
}

/// A range cut to the area that exists on screen, so a whole column does not make a huge rectangle.
fn clip_range(r: RangeRef, rows: u32, cols: u32) -> RangeRef {
    let end = CellRef::new(r.end.row.min(rows.saturating_sub(1)), r.end.col.min(cols.saturating_sub(1)));
    RangeRef::new(r.start, end)
}

fn rgb(c: [u8; 3]) -> Color {
    Color::rgb(c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0)
}

fn fill_color(wb: &Workbook, style: &ravenrs_sheet::model::Style) -> Option<Color> {
    match style.fill.pattern {
        PatternType::None => None,
        PatternType::Solid => style.fill.fg.resolve(&wb.theme).map(rgb),
        _ => {
            // Patterns show as a blend of their two colors.
            let fg = style.fill.fg.resolve(&wb.theme).unwrap_or([0, 0, 0]);
            let bg = style.fill.bg.resolve(&wb.theme).unwrap_or([255, 255, 255]);
            let mix = |i: usize| ((fg[i] as u16 + bg[i] as u16 * 3) / 4) as u8;
            Some(rgb([mix(0), mix(1), mix(2)]))
        }
    }
}

/// Text with no color of its own: the theme's ink, unless the cell's fill makes it hard to read.
fn automatic_ink(ink: Color, fill: Option<Color>) -> Color {
    match fill {
        Some(fill) if ink.contrast_ratio(fill) < 3.0 => {
            if Color::BLACK.contrast_ratio(fill) >= Color::WHITE.contrast_ratio(fill) {
                Color::BLACK
            } else {
                Color::WHITE
            }
        }
        _ => ink,
    }
}

fn edge(wb: &Workbook, line: &BorderLine, ink: Color) -> Option<Edge> {
    let width = match line.style {
        BorderStyle::None => return None,
        BorderStyle::Medium
        | BorderStyle::MediumDashed
        | BorderStyle::MediumDashDot
        | BorderStyle::MediumDashDotDot => 2.0,
        BorderStyle::Thick | BorderStyle::Double => 3.0,
        _ => 1.0,
    };
    Some(Edge { width, color: line.color.resolve(&wb.theme).map_or(ink, rgb) })
}

/// How one cell looks, or nothing when it has no fill, no borders and no text.
fn cell_look(
    wb: &Workbook,
    sheet: &Sheet,
    grid: &Grid,
    at: CellRef,
    merge: Option<RangeRef>,
    ink: Color,
) -> Option<CellLook> {
    let style = wb.styles.get(sheet.style_id(at));
    let value = sheet.value(at);
    let fill = fill_color(wb, style);
    let b = &style.borders;
    let edges =
        [edge(wb, &b.left, ink), edge(wb, &b.top, ink), edge(wb, &b.right, ink), edge(wb, &b.bottom, ink)];
    if matches!(value, Value::Empty) && fill.is_none() && edges.iter().all(Option::is_none) {
        return None;
    }
    let rect = merge.map_or_else(|| grid.cell_rect(at), |m| grid.range_rect(m));
    let formatted = display::format(&value, style.num_fmt.as_str(), wb);
    let text = if sheet.show_formulas {
        display::cell_text(wb, sheet, at)
    } else if !sheet.show_zeros && value.as_f64() == Some(0.0) {
        String::new()
    } else {
        formatted.text
    };
    let color = formatted
        .color
        .and_then(|c| c.rgb())
        .map(rgb)
        .or_else(|| style.font.color.resolve(&wb.theme).map(rgb))
        .unwrap_or_else(|| automatic_ink(ink, fill));
    let align = match style.align.h {
        HAlign::General => match value {
            Value::Number(_) => HAlign::Right,
            Value::Bool(_) | Value::Error(_) => HAlign::Center,
            _ if formatted.numeric => HAlign::Right,
            _ => HAlign::Left,
        },
        other => other,
    };
    // Left-aligned text runs into the empty cells to its right.
    let mut clip = rect;
    if merge.is_none() && align == HAlign::Left && matches!(value, Value::Text(_)) && !style.align.wrap {
        let mut col = at.col + 1;
        while col < at.col + OVERFLOW_CELLS
            && col < MAX_COLS
            && sheet.value(CellRef::new(at.row, col)) == Value::Empty
        {
            col += 1;
        }
        let end = grid.cell_rect(CellRef::new(at.row, col - 1));
        clip = Rect::new(rect.x(), rect.y(), end.x() + end.width() - rect.x(), rect.height());
    }
    let size_px = (style.font.size * 96.0 / 72.0).clamp(6.0, 96.0);
    let attrs = TextAttrs {
        size: size_px,
        line_height: (size_px * 1.25).round(),
        weight: if style.font.bold { 600.0 } else { 400.0 },
        ..TextAttrs::default()
    };
    Some(CellLook {
        rect,
        clip,
        fill,
        text,
        color,
        attrs,
        align,
        valign: style.align.v,
        underline: style.font.underline != Underline::None,
        strike: style.font.strike,
        edges,
    })
}

fn quad(paint: &mut PaintCx<'_>, rect: Rect, color: Color) {
    if rect.width() > 0.0 && rect.height() > 0.0 {
        paint.scene.push_quad(Quad::new(rect, color));
    }
}

fn at(frame: Rect, r: Rect) -> Rect {
    Rect::new(frame.x() + r.x(), frame.y() + r.y(), r.width(), r.height())
}

fn paint_picture(p: &Picture, frame: Rect, paint: &mut PaintCx<'_>) {
    let theme = &p.theme;
    let canvas = theme.color(Semantic::Canvas);
    let line = theme.color(Semantic::Border).with_alpha(0.55);
    let accent = theme.color(Semantic::Accent);
    let cells = Rect::new(
        frame.x() + ROW_HEADER_WIDTH,
        frame.y() + HEADER_HEIGHT,
        (p.size.width - ROW_HEADER_WIDTH).max(0.0),
        (p.size.height - HEADER_HEIGHT).max(0.0),
    );
    quad(paint, frame, canvas);
    paint.scene.push_clip(cells);
    if p.gridlines {
        for &col in &p.columns {
            let r = p.grid.cell_rect(CellRef::new(0, col));
            quad(paint, Rect::new(frame.x() + r.x() + r.width() - 1.0, cells.y(), 1.0, cells.height()), line);
        }
        for &row in &p.rows {
            let r = p.grid.cell_rect(CellRef::new(row, 0));
            quad(paint, Rect::new(cells.x(), frame.y() + r.y() + r.height() - 1.0, cells.width(), 1.0), line);
        }
    }
    for (rect, fill) in &p.merges {
        let inner = at(frame, *rect);
        quad(
            paint,
            Rect::new(inner.x(), inner.y(), inner.width() - 1.0, inner.height() - 1.0),
            fill.unwrap_or(canvas),
        );
    }
    for cell in &p.cells {
        if let Some(fill) = cell.fill {
            quad(paint, at(frame, cell.rect), fill);
        }
    }
    for cell in &p.cells {
        paint_edges(cell, frame, paint);
    }
    for cell in p.cells.iter().filter(|c| !c.text.is_empty()) {
        paint_text(cell, frame, paint);
    }
    // The selection: a tint over the range, and a frame around the active cell.
    for r in &p.selection {
        quad(paint, at(frame, *r), accent.with_alpha(0.10));
        let r = at(frame, *r);
        paint.scene.push_quad(Quad::new(r, Color::TRANSPARENT).border(1.0, accent));
    }
    let active = at(frame, p.active);
    quad(paint, active, canvas.with_alpha(0.0));
    paint.scene.push_quad(
        Quad::new(
            Rect::new(active.x() - 1.0, active.y() - 1.0, active.width() + 1.0, active.height() + 1.0),
            Color::TRANSPARENT,
        )
        .border(2.0, accent),
    );
    paint.scene.pop_clip();
    paint_headers(p, frame, paint);
}

fn paint_edges(cell: &CellLook, frame: Rect, paint: &mut PaintCx<'_>) {
    let r = at(frame, cell.rect);
    let [left, top, right, bottom] = cell.edges;
    if let Some(e) = left {
        quad(paint, Rect::new(r.x() - 1.0, r.y() - 1.0, e.width, r.height() + 1.0), e.color);
    }
    if let Some(e) = top {
        quad(paint, Rect::new(r.x() - 1.0, r.y() - 1.0, r.width() + 1.0, e.width), e.color);
    }
    if let Some(e) = right {
        quad(paint, Rect::new(r.x() + r.width() - e.width, r.y() - 1.0, e.width, r.height() + 1.0), e.color);
    }
    if let Some(e) = bottom {
        quad(paint, Rect::new(r.x() - 1.0, r.y() + r.height() - e.width, r.width() + 1.0, e.width), e.color);
    }
}

fn paint_text(cell: &CellLook, frame: Rect, paint: &mut PaintCx<'_>) {
    const PAD: f32 = 3.0;
    let clip = at(frame, cell.clip);
    let rect = at(frame, cell.rect);
    let mut layout = paint.text.layout(&cell.text, &cell.attrs);
    let measured = layout.measure(None);
    let x = match cell.align {
        HAlign::Right => rect.x() + rect.width() - PAD - measured.width,
        HAlign::Center | HAlign::CenterAcross => rect.x() + (rect.width() - measured.width) / 2.0,
        _ => rect.x() + PAD,
    };
    let y = match cell.valign {
        VAlign::Top => rect.y() + 1.0,
        VAlign::Center | VAlign::Justify | VAlign::Distributed => {
            rect.y() + (rect.height() - measured.height) / 2.0
        }
        _ => rect.y() + rect.height() - measured.height - 1.0,
    };
    paint.scene.push_clip(Rect::new(
        clip.x(),
        clip.y(),
        (clip.width() - 1.0).max(0.0),
        (clip.height() - 1.0).max(0.0),
    ));
    layout.paint(Point::new(x.round(), y.round()), cell.color, paint.scene);
    let baseline = y + measured.height * 0.82;
    if cell.underline {
        quad(paint, Rect::new(x, baseline.round(), measured.width, 1.0), cell.color);
    }
    if cell.strike {
        quad(paint, Rect::new(x, (y + measured.height * 0.55).round(), measured.width, 1.0), cell.color);
    }
    paint.scene.pop_clip();
}

fn paint_headers(p: &Picture, frame: Rect, paint: &mut PaintCx<'_>) {
    let theme = &p.theme;
    let surface = theme.color(Semantic::Surface);
    let border = theme.color(Semantic::Border);
    let muted = theme.color(Semantic::InkMuted);
    let accent = theme.color(Semantic::Accent);
    let style = theme.text(TextStyle::Caption);
    let attrs = TextAttrs {
        size: style.size,
        line_height: style.line_height,
        weight: style.weight.0,
        ..TextAttrs::default()
    };
    let strong = TextAttrs { weight: 600.0, ..attrs.clone() };

    let top = Rect::new(frame.x(), frame.y(), frame.width(), HEADER_HEIGHT);
    let left = Rect::new(frame.x(), frame.y(), ROW_HEADER_WIDTH, frame.height());
    quad(paint, top, surface);
    quad(paint, left, surface);

    paint.scene.push_clip(Rect::new(
        frame.x() + ROW_HEADER_WIDTH,
        frame.y(),
        (frame.width() - ROW_HEADER_WIDTH).max(0.0),
        HEADER_HEIGHT,
    ));
    for &col in &p.columns {
        let r = p.grid.cell_rect(CellRef::new(0, col));
        let cell = Rect::new(frame.x() + r.x(), frame.y(), r.width(), HEADER_HEIGHT);
        let selected = p.selected_columns.iter().any(|&(a, b)| col >= a && col <= b);
        if selected {
            quad(paint, cell, accent.with_alpha(0.14));
            quad(paint, Rect::new(cell.x(), cell.y() + HEADER_HEIGHT - 2.0, cell.width(), 2.0), accent);
        }
        quad(paint, Rect::new(cell.x() + cell.width() - 1.0, cell.y(), 1.0, HEADER_HEIGHT), border);
        let label = col_to_letters(col);
        let mut layout = paint.text.layout(&label, if selected { &strong } else { &attrs });
        let m = layout.measure(None);
        let color = if selected { accent } else { muted };
        layout.paint(
            Point::new(
                (cell.x() + (cell.width() - m.width) / 2.0).round(),
                (cell.y() + (HEADER_HEIGHT - m.height) / 2.0).round(),
            ),
            color,
            paint.scene,
        );
    }
    paint.scene.pop_clip();

    paint.scene.push_clip(Rect::new(
        frame.x(),
        frame.y() + HEADER_HEIGHT,
        ROW_HEADER_WIDTH,
        (frame.height() - HEADER_HEIGHT).max(0.0),
    ));
    for &row in &p.rows {
        let r = p.grid.cell_rect(CellRef::new(row, 0));
        let cell = Rect::new(frame.x(), frame.y() + r.y(), ROW_HEADER_WIDTH, r.height());
        let selected = p.selected_rows.iter().any(|&(a, b)| row >= a && row <= b);
        if selected {
            quad(paint, cell, accent.with_alpha(0.14));
            quad(paint, Rect::new(cell.x() + ROW_HEADER_WIDTH - 2.0, cell.y(), 2.0, cell.height()), accent);
        }
        quad(paint, Rect::new(cell.x(), cell.y() + cell.height() - 1.0, ROW_HEADER_WIDTH, 1.0), border);
        let label = (row + 1).to_string();
        let mut layout = paint.text.layout(&label, if selected { &strong } else { &attrs });
        let m = layout.measure(None);
        let color = if selected { accent } else { muted };
        layout.paint(
            Point::new(
                (cell.x() + (ROW_HEADER_WIDTH - m.width) / 2.0).round(),
                (cell.y() + (cell.height() - m.height) / 2.0).round(),
            ),
            color,
            paint.scene,
        );
    }
    paint.scene.pop_clip();

    quad(paint, Rect::new(frame.x(), frame.y() + HEADER_HEIGHT - 1.0, frame.width(), 1.0), border);
    quad(paint, Rect::new(frame.x() + ROW_HEADER_WIDTH - 1.0, frame.y(), 1.0, frame.height()), border);
    quad(paint, Rect::new(frame.x(), frame.y(), ROW_HEADER_WIDTH - 1.0, HEADER_HEIGHT - 1.0), surface);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> Grid {
        let sheet = Sheet::new(String::from("S"));
        Grid { rows: Axis::rows(&sheet, 1.0), columns: Axis::columns(&sheet, 1.0), offset: Point::ZERO }
    }

    #[test]
    fn points_land_on_headers_cells_and_column_edges() {
        let g = grid();
        let w = g.columns.size(0);
        let h = g.rows.size(0);
        assert_eq!(g.hit(Point::new(5.0, 5.0)), Hit::Corner);
        assert_eq!(g.hit(Point::new(ROW_HEADER_WIDTH + w * 1.5, 5.0)), Hit::Column(1));
        assert_eq!(g.hit(Point::new(ROW_HEADER_WIDTH + w - 1.0, 5.0)), Hit::ColumnEdge(0));
        assert_eq!(g.hit(Point::new(5.0, HEADER_HEIGHT + h * 2.5)), Hit::Row(2));
        assert_eq!(
            g.hit(Point::new(ROW_HEADER_WIDTH + w * 2.5, HEADER_HEIGHT + h * 3.5)),
            Hit::Cell(CellRef::new(3, 2))
        );
    }

    #[test]
    fn automatic_text_stays_readable_on_a_fill() {
        let light_ink = Color::hex(0xeef1f7);
        assert_eq!(automatic_ink(light_ink, Some(Color::hex(0xdce6f1))), Color::BLACK);
        assert_eq!(automatic_ink(light_ink, Some(Color::hex(0x202020))), light_ink);
        assert_eq!(automatic_ink(Color::hex(0x111111), None), Color::hex(0x111111));
    }

    #[test]
    fn scrolling_shows_a_cell_out_of_view_and_leaves_one_in_view() {
        let g = grid();
        let size = Size::new(400.0, 300.0);
        assert_eq!(scroll_to_show(&g, CellRef::new(1, 1), size), None);
        let to = scroll_to_show(&g, CellRef::new(100, 30), size).unwrap();
        assert!(to.y > 0.0 && to.x > 0.0);
    }
}
