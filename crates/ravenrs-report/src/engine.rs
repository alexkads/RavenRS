//! Rendering: binds data to bands, groups and sorts rows, lays out each band
//! instance (controls grow to fit their text), and paginates.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use ravenrs_core::{Color, Point, Rect};
use ravenrs_text::{TextAttrs, TextSystem};
use serde_json::Value as Json;

use crate::document::{Document, Item, Page, PlacedLine, TextBlock, parse_color};
use ravenrs_criteria::{
    Criteria, Culture, CustomFunction, DateTime, Environment, ParseError, Scope, Value, format_composite,
    format_value,
};

use crate::model::{
    Band, Control, ControlKind, LineDirection, ParameterKind, Report, ShapeKind, SortField, Style, TableCell,
    TextAlign, VerticalAlign,
};

/// Settings for one run of a report.
#[derive(Clone, Debug, Default)]
pub struct RenderOptions {
    /// Parameter values by name; missing ones take their defaults.
    pub parameters: HashMap<String, serde_json::Value>,
    /// The time `Today()` and `Now()` return; the system clock when `None`.
    pub now: Option<DateTime>,
}

/// Renders reports. Keep one around: it caches fonts and text measurements.
///
/// ```
/// use ravenrs_report::{Band, Control, Report, ReportEngine};
/// use serde_json::json;
///
/// let report = Report::new("Hello").detail(Band::new(20.0).control(Control::field("[Name]").at(0.0, 0.0, 200.0, 20.0)));
/// let doc = ReportEngine::new().render(&report, &json!([{ "Name": "Ana" }, { "Name": "Bruno" }]));
/// assert_eq!(doc.pages.len(), 1);
/// assert!(doc.page_contains(0, "Bruno"));
/// ```
pub struct ReportEngine {
    text: TextSystem,
    measured: HashMap<(String, u32, bool, u32), Rc<Measured>>,
    functions: Vec<(String, CustomFunction)>,
}

impl std::fmt::Debug for ReportEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReportEngine").finish_non_exhaustive()
    }
}

impl Default for ReportEngine {
    fn default() -> Self {
        Self::new()
    }
}

struct Measured {
    lines: Vec<(std::ops::Range<usize>, f32, f32, f32)>, // range, width, top, baseline
    height: f32,
}

/// A laid-out band instance: its height and items relative to its top-left.
struct Block {
    height: f32,
    items: Vec<Item>,
    keep_with_next: bool,
    break_before: bool,
    break_after: bool,
    /// Top-level row printed by this block (for page aggregates).
    row: Option<usize>,
    /// Group level this block opens a header for (repeated headers).
    header_level: Option<usize>,
    /// The report header: printed above the page header on the first page.
    report_header: bool,
}

const DEFAULT_SIZE: f32 = 10.0;

impl ReportEngine {
    /// An engine with the embedded Inter font.
    pub fn new() -> Self {
        Self { text: TextSystem::new(), measured: HashMap::new(), functions: Vec::new() }
    }

    /// Adds a function to the expressions of every report this engine renders.
    pub fn register_function(
        &mut self,
        name: &str,
        function: impl Fn(&[Value]) -> Result<Value, String> + 'static,
    ) {
        self.functions.push((name.to_owned(), Rc::new(function)));
    }

    /// Renders `report` over `data` with default parameters.
    pub fn render(&mut self, report: &Report, data: &Json) -> Document {
        self.render_with(report, data, &RenderOptions::default())
    }

    /// Renders `report` over `data`. Expression errors do not stop the render:
    /// they are listed in [`Document::errors`] and shown in place.
    pub fn render_with(&mut self, report: &Report, data: &Json, options: &RenderOptions) -> Document {
        let run = Run::new(report, options, &self.functions);
        let rows_owned = run.rows(data);
        let rows: Vec<&Json> = rows_owned.iter().collect();
        let blocks = run.blocks(self, &rows);
        let pages = run.paginate(self, blocks, &rows);
        let errors = run.errors.into_inner();
        Document { title: report.name.clone(), pages, errors }
    }

    fn measure(&mut self, text: &str, size: f32, bold: bool, width: Option<f32>) -> Rc<Measured> {
        let key = (text.to_owned(), size.to_bits(), bold, width.map_or(u32::MAX, f32::to_bits));
        if let Some(m) = self.measured.get(&key) {
            return m.clone();
        }
        let attrs = text_attrs(size, bold);
        let mut layout = self.text.layout(text, &attrs);
        let size = layout.measure(width);
        let lines = layout.lines().into_iter().map(|l| (l.range, l.width, l.top, l.baseline)).collect();
        let m = Rc::new(Measured { lines, height: if text.is_empty() { 0.0 } else { size.height } });
        // Bounded, so a long report cannot grow the cache without limit.
        if self.measured.len() > 50_000 {
            self.measured.clear();
        }
        self.measured.insert(key, m.clone());
        m
    }

    /// The text system used for measuring, to paint documents with the same metrics.
    pub fn text_system(&mut self) -> &mut TextSystem {
        &mut self.text
    }
}

pub(crate) fn text_attrs(size: f32, bold: bool) -> TextAttrs {
    TextAttrs {
        size,
        line_height: (size * 1.25).ceil(),
        weight: if bold { 600.0 } else { 400.0 },
        letter_spacing: 0.0,
        family: None,
    }
}

/// Per-render state.
struct Run<'r> {
    report: &'r Report,
    env: Environment,
    exprs: RefCell<HashMap<String, Rc<Result<Criteria, ParseError>>>>,
    errors: RefCell<Vec<String>>,
}

/// A resolved style with defaults filled in.
struct Look {
    size: f32,
    bold: bool,
    color: Color,
    background: Option<Color>,
    border: Option<(f32, Color, [bool; 4])>,
    padding: f32,
    align: TextAlign,
    valign: VerticalAlign,
    wrap: bool,
}

impl<'r> Run<'r> {
    fn new(report: &'r Report, options: &RenderOptions, functions: &[(String, CustomFunction)]) -> Self {
        let mut env = Environment::new()
            .culture(Culture::named(&report.culture))
            .at(options.now.unwrap_or_else(DateTime::now));
        for p in &report.parameters {
            let json = options.parameters.get(&p.name).or(p.default.as_ref());
            let v = json.map_or(Value::Null, Value::from_json);
            let v = match (p.kind, &v) {
                (ParameterKind::Date, _) => v.as_date().map_or(v, Value::Date),
                (ParameterKind::Number, Value::Text(_)) => v.as_number().map_or(v, Value::Number),
                _ => v,
            };
            env = env.parameter(p.name.clone(), v);
        }
        for (k, v) in &options.parameters {
            if !report.parameters.iter().any(|p| &p.name == k) {
                env = env.parameter(k.clone(), Value::from_json(v));
            }
        }
        for (name, function) in functions {
            let function = function.clone();
            env = env.register(name, move |args| function(args));
        }
        let errors = RefCell::new(Vec::new());
        for c in &report.calculated_fields {
            match Criteria::parse(&c.expression) {
                Ok(e) => env = env.calculated(c.name.clone(), e),
                Err(e) => errors.borrow_mut().push(format!("calculated field '{}': {e}", c.name)),
            }
        }
        Self { report, env, exprs: RefCell::new(HashMap::new()), errors }
    }

    /// The parsed expression for `src`, cached.
    fn expr(&self, src: &str) -> Rc<Result<Criteria, ParseError>> {
        if let Some(e) = self.exprs.borrow().get(src) {
            return e.clone();
        }
        let parsed = Rc::new(Criteria::parse(src));
        self.exprs.borrow_mut().insert(src.to_owned(), parsed.clone());
        parsed
    }

    fn eval(&self, ctx: &Scope<'_, Json>, src: &str, what: &str) -> Result<Value, String> {
        let problem = match &*self.expr(src) {
            Ok(e) => match ctx.evaluate(e) {
                Ok(v) => return Ok(v),
                Err(e) => e.to_string(),
            },
            Err(e) => e.to_string(),
        };
        let msg = format!("{what}: {problem} in `{src}`");
        let mut errors = self.errors.borrow_mut();
        if !errors.contains(&msg) {
            errors.push(msg);
        }
        Err(problem)
    }

    fn ctx<'a>(
        &'a self,
        row: Option<&'a Json>,
        scope: &'a [&'a Json],
        position: Option<usize>,
    ) -> Scope<'a, Json> {
        Scope::new(&self.env).maybe_row(row).rows(scope).position(position)
    }

    /// The report's rows: located, filtered and sorted (group keys first).
    fn rows(&self, data: &Json) -> Vec<Json> {
        let base = match &self.report.data_member {
            Some(path) if !path.trim().is_empty() => {
                let segs: Vec<String> = path.split('.').map(|s| s.trim().to_owned()).collect();
                match lookup(Some(data), &segs) {
                    Some(v) => v,
                    None => {
                        self.errors
                            .borrow_mut()
                            .push(format!("data member '{path}' was not found in the data"));
                        return Vec::new();
                    }
                }
            }
            _ => data,
        };
        let rows: Vec<&Json> = match base {
            Json::Array(items) => items.iter().collect(),
            Json::Null => Vec::new(),
            other => vec![other],
        };
        let mut keys: Vec<SortField> = self
            .report
            .groups
            .iter()
            .map(|g| SortField { expression: g.expression.clone(), descending: g.descending })
            .collect();
        keys.extend(self.report.sort.iter().cloned());
        filter_and_sort(self, &rows, self.report.filter.as_deref(), &keys, None)
    }

    /// Lays out every band instance in print order (page bands excluded).
    fn blocks(&self, engine: &mut ReportEngine, rows: &[&Json]) -> Vec<Block> {
        let report = self.report;
        let width = report.page.content_width();
        let mut out = Vec::new();
        let first = rows.first().copied();
        let last = rows.last().copied();
        if let Some(band) = &report.report_header
            && let Some(mut b) = self.band(engine, band, &self.ctx(first, rows, None), width, None)
        {
            b.report_header = true;
            out.push(b);
        }
        // Group boundaries: the key of each level for each row.
        let levels = report.groups.len();
        let keys: Vec<Vec<Value>> = rows
            .iter()
            .map(|r| {
                report
                    .groups
                    .iter()
                    .map(|g| {
                        self.eval(&self.ctx(Some(r), rows, None), &g.expression, "group").unwrap_or_default()
                    })
                    .collect()
            })
            .collect();
        // For each level, the row range of the group containing each row.
        let mut starts = vec![0usize; levels];
        let group_end = |level: usize, start: usize| -> usize {
            let mut end = start + 1;
            while end < rows.len()
                && (0..=level).all(|l| {
                    keys[end][l].loose_eq(&keys[start][l])
                        || (keys[end][l].is_null() && keys[start][l].is_null())
                })
            {
                end += 1;
            }
            end
        };
        let mut ends = vec![0usize; levels];
        let mut first_group = vec![true; levels];
        for (i, row) in rows.iter().enumerate() {
            // Open groups that start at this row, outermost first.
            for level in 0..levels {
                let opens = i == 0 || i >= ends[level];
                if !opens {
                    continue;
                }
                starts[level] = i;
                ends[level] = group_end(level, i);
                let g = &report.groups[level];
                if let Some(header) = &g.header {
                    let scope = &rows[i..ends[level]];
                    let ctx = self.ctx(Some(row), scope, Some(0));
                    if let Some(mut b) = self.band(engine, header, &ctx, width, None) {
                        b.header_level = Some(level);
                        b.break_before |= g.page_break_before && !first_group[level];
                        b.keep_with_next = true;
                        out.push(b);
                    }
                } else if g.page_break_before && !first_group[level] {
                    out.push(Block {
                        height: 0.0,
                        items: Vec::new(),
                        keep_with_next: false,
                        break_before: true,
                        break_after: false,
                        row: None,
                        header_level: None,
                        report_header: false,
                    });
                }
                first_group[level] = false;
            }
            // The detail band, its aggregates scoped to the innermost group.
            let (scope, position) = match levels {
                0 => (rows, i),
                n => (&rows[starts[n - 1]..ends[n - 1]], i - starts[n - 1]),
            };
            let ctx = self.ctx(Some(row), scope, Some(position));
            if let Some(mut b) = self.band(engine, &report.detail, &ctx, width, Some(i % 2 == 0)) {
                b.row = Some(i);
                out.push(b);
            }
            for dr in &report.detail_reports {
                self.detail_report(engine, dr, row, width, &mut out);
            }
            // Close groups that end after this row, innermost first.
            for level in (0..levels).rev() {
                if i + 1 != ends[level] {
                    continue;
                }
                if let Some(footer) = &report.groups[level].footer {
                    let scope = &rows[starts[level]..ends[level]];
                    let ctx = self.ctx(Some(row), scope, None);
                    if let Some(b) = self.band(engine, footer, &ctx, width, None) {
                        out.push(b);
                    }
                }
            }
        }
        if let Some(band) = &report.report_footer
            && let Some(b) = self.band(engine, band, &self.ctx(last, rows, None), width, None)
        {
            out.push(b);
        }
        out
    }

    fn detail_report(
        &self,
        engine: &mut ReportEngine,
        dr: &crate::model::DetailReport,
        master: &Json,
        width: f32,
        out: &mut Vec<Block>,
    ) {
        let segs: Vec<String> = dr.data_member.split('.').map(|s| s.trim().to_owned()).collect();
        let items: Vec<&Json> = match lookup(Some(master), &segs) {
            Some(Json::Array(a)) => a.iter().collect(),
            _ => Vec::new(),
        };
        let owned = filter_and_sort(self, &items, dr.filter.as_deref(), &dr.sort, Some(master));
        let rows: Vec<&Json> = owned.iter().collect();
        if rows.is_empty() {
            return;
        }
        if let Some(h) = &dr.header {
            let ctx = self.ctx(Some(master), &rows, None).parent(Some(master));
            if let Some(mut b) = self.band(engine, h, &ctx, width, None) {
                b.keep_with_next = true;
                out.push(b);
            }
        }
        for (j, r) in rows.iter().enumerate() {
            let ctx = self.ctx(Some(r), &rows, Some(j)).parent(Some(master));
            if let Some(b) = self.band(engine, &dr.detail, &ctx, width, Some(j % 2 == 0)) {
                out.push(b);
            }
        }
        if let Some(f) = &dr.footer {
            let ctx = self.ctx(Some(master), &rows, None).parent(Some(master));
            if let Some(b) = self.band(engine, f, &ctx, width, None) {
                out.push(b);
            }
        }
    }

    /// Lays out one instance of `band`; `None` when it is hidden.
    fn band(
        &self,
        engine: &mut ReportEngine,
        band: &Band,
        ctx: &Scope<'_, Json>,
        width: f32,
        odd: Option<bool>,
    ) -> Option<Block> {
        if let Some(v) = &band.visible
            && !self.eval(ctx, v, "band visibility").map(|v| v.is_true()).unwrap_or(true)
        {
            return None;
        }
        let zebra = match odd {
            Some(true) => band.odd_style.as_deref(),
            Some(false) => band.even_style.as_deref(),
            None => None,
        };
        let mut items = Vec::new();
        if let Some(name) = zebra
            && let Some(bg) =
                self.report.styles.get(name).and_then(|s| s.background.as_deref()).and_then(parse_color)
        {
            // Row striping fills the whole band; it is placed first so content draws on top.
            items.push(Item::Rect {
                rect: Rect::new(0.0, 0.0, width, band.height),
                fill: Some(bg),
                stroke: None,
                radius: 0.0,
            });
        }
        let height = self.controls(engine, &band.controls, ctx, Point::ZERO, zebra, &mut items);
        let height = height.max(band.height);
        // The stripe grows with the band.
        if zebra.is_some()
            && let Some(Item::Rect { rect, .. }) = items.first_mut()
        {
            *rect = Rect::new(0.0, 0.0, width, height);
        }
        Some(Block {
            height,
            items,
            keep_with_next: band.keep_with_next,
            break_before: band.page_break_before,
            break_after: band.page_break_after,
            row: None,
            header_level: None,
            report_header: false,
        })
    }

    /// Lays out controls at `origin`, moving controls below a grown one down by
    /// as much as it grew. Returns the bottom of the content.
    fn controls(
        &self,
        engine: &mut ReportEngine,
        controls: &[Control],
        ctx: &Scope<'_, Json>,
        origin: Point,
        zebra: Option<&str>,
        items: &mut Vec<Item>,
    ) -> f32 {
        let mut order: Vec<&Control> = controls.iter().collect();
        order.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
        // (original bottom, how much it grew)
        let mut grown: Vec<(f32, f32)> = Vec::new();
        let mut bottom: f32 = 0.0;
        for c in order {
            if let Some(v) = &c.visible
                && !self.eval(ctx, v, &control_name(c)).map(|v| v.is_true()).unwrap_or(true)
            {
                continue;
            }
            let shift = grown.iter().filter(|(b, _)| *b <= c.y + 0.5).map(|(_, d)| *d).fold(0.0, f32::max);
            let at = Point::new(origin.x + c.x, origin.y + c.y + shift);
            let h = self.control(engine, c, ctx, at, zebra, items);
            if h > c.height + 0.01 {
                grown.push((c.y + c.height, h - c.height + shift));
            }
            bottom = bottom.max(c.y + shift + h);
        }
        bottom
    }

    /// Resolves a style from layers (each a named style and own settings,
    /// later layers winning), then conditional rules on top.
    fn look(
        &self,
        layers: &[(Option<&str>, &Style)],
        rules: &[crate::model::FormattingRule],
        ctx: &Scope<'_, Json>,
        zebra: Option<&str>,
    ) -> Look {
        let mut s = Style::default();
        if let Some(z) = zebra.and_then(|z| self.report.styles.get(z)) {
            // Striping contributes text settings, not a second fill.
            s = s.merged(&Style { background: None, ..z.clone() });
        }
        for (name, own) in layers {
            if let Some(named) = name.and_then(|n| self.report.styles.get(n)) {
                s = s.merged(named);
            }
            s = s.merged(own);
        }
        for r in rules {
            if self.eval(ctx, &r.condition, "formatting rule").map(|v| v.is_true()).unwrap_or(false) {
                s = s.merged(&r.style);
            }
        }
        let border_w = s.border_width.unwrap_or(0.0);
        let border = (border_w > 0.0).then(|| {
            let sides = s.borders.as_deref().unwrap_or("all").to_lowercase();
            let all = sides.contains("all");
            let has = |side: &str| all || sides.contains(side);
            let color =
                s.border_color.as_deref().and_then(parse_color).unwrap_or(Color::rgba(0.0, 0.0, 0.0, 1.0));
            (border_w, color, [has("top"), has("right"), has("bottom"), has("left")])
        });
        Look {
            size: s.font_size.unwrap_or(DEFAULT_SIZE).clamp(1.0, 400.0),
            bold: s.bold.unwrap_or(false),
            color: s.color.as_deref().and_then(parse_color).unwrap_or(Color::rgba(0.0, 0.0, 0.0, 1.0)),
            background: s.background.as_deref().and_then(parse_color),
            border,
            padding: s.padding.unwrap_or(2.0).max(0.0),
            align: s.text_align.unwrap_or_default(),
            valign: s.vertical_align.unwrap_or_default(),
            wrap: s.word_wrap.unwrap_or(true),
        }
    }

    /// Lays out a control at `at`; returns its final height.
    fn control(
        &self,
        engine: &mut ReportEngine,
        c: &Control,
        ctx: &Scope<'_, Json>,
        at: Point,
        zebra: Option<&str>,
        items: &mut Vec<Item>,
    ) -> f32 {
        let look = self.look(&[(c.style_name.as_deref(), &c.style)], &c.rules, ctx, zebra);
        match &c.kind {
            ControlKind::Label(label) => {
                let text = self.label_text(ctx, label, &control_name(c));
                self.text_box(
                    engine,
                    &text,
                    &look,
                    Rect::new(at.x, at.y, c.width, c.height),
                    label.grows(),
                    items,
                )
            }
            ControlKind::CheckBox(cb) => {
                let rect = Rect::new(at.x, at.y, c.width, c.height);
                backdrop(&look, rect, items);
                let checked =
                    self.eval(ctx, &cb.expression, &control_name(c)).map(|v| v.is_true()).unwrap_or(false);
                let side = (look.size * 1.1).min(c.height);
                let bx = Rect::new(at.x + look.padding, at.y + (c.height - side) / 2.0, side, side);
                items.push(Item::Rect {
                    rect: bx,
                    fill: None,
                    stroke: Some((0.75, look.color)),
                    radius: 1.5,
                });
                if checked {
                    let p =
                        |fx: f32, fy: f32| Point::new(bx.x() + bx.width() * fx, bx.y() + bx.height() * fy);
                    items.push(Item::Line {
                        from: p(0.22, 0.52),
                        to: p(0.42, 0.72),
                        width: 1.2,
                        color: look.color,
                    });
                    items.push(Item::Line {
                        from: p(0.42, 0.72),
                        to: p(0.8, 0.28),
                        width: 1.2,
                        color: look.color,
                    });
                }
                if !cb.text.is_empty() {
                    let tx = bx.max_x() + look.padding * 2.0;
                    let rest = Rect::new(tx, at.y, (rect.max_x() - tx).max(0.0), c.height);
                    let mut plain = Vec::new();
                    self.text_box(
                        engine,
                        &cb.text,
                        &Look { background: None, border: None, ..look },
                        rest,
                        false,
                        &mut plain,
                    );
                    items.extend(plain);
                }
                c.height
            }
            ControlKind::Line(line) => {
                let w = line.thickness.unwrap_or(1.0);
                let r = Rect::new(at.x, at.y, c.width, c.height);
                let (from, to) = match line.direction {
                    LineDirection::Vertical => {
                        (Point::new(r.center().x, r.y()), Point::new(r.center().x, r.max_y()))
                    }
                    LineDirection::Slant => (r.origin, Point::new(r.max_x(), r.max_y())),
                    _ => (Point::new(r.x(), r.center().y), Point::new(r.max_x(), r.center().y)),
                };
                items.push(Item::Line { from, to, width: w, color: look.color });
                c.height
            }
            ControlKind::Shape(shape) => {
                let rect = Rect::new(at.x, at.y, c.width, c.height);
                let stroke = look.border.map(|(w, color, _)| (w, color));
                items.push(match shape.shape {
                    ShapeKind::Ellipse => Item::Ellipse { rect, fill: look.background, stroke },
                    _ => Item::Rect { rect, fill: look.background, stroke, radius: shape.radius },
                });
                c.height
            }
            ControlKind::Table(table) => {
                let mut y = at.y;
                for row in &table.rows {
                    // Measure every cell, then draw them at the row's final height.
                    let mut cells: Vec<(&TableCell, Look, String)> = Vec::new();
                    let mut row_h = row.height;
                    for cell in &row.cells {
                        // Cells inherit the table's style; the table's rules apply on top.
                        let look = self.look(
                            &[(c.style_name.as_deref(), &c.style), (cell.style_name.as_deref(), &cell.style)],
                            &c.rules,
                            ctx,
                            zebra,
                        );
                        let text = self.label_text(ctx, &cell.label, &control_name(c));
                        let needed = self.text_height(engine, &text, &look, cell.width);
                        if cell.label.grows() {
                            row_h = row_h.max(needed);
                        }
                        cells.push((cell, look, text));
                    }
                    let mut x = at.x;
                    for (cell, look, text) in cells {
                        self.text_box(engine, &text, &look, Rect::new(x, y, cell.width, row_h), false, items);
                        x += cell.width;
                    }
                    y += row_h;
                }
                (y - at.y).max(c.height)
            }
            ControlKind::Panel(panel) => {
                let mut content = Vec::new();
                let inner = self.controls(engine, &panel.controls, ctx, at, zebra, &mut content);
                let h = inner.max(c.height);
                backdrop(&look, Rect::new(at.x, at.y, c.width, h), items);
                items.extend(content);
                h
            }
        }
    }

    fn label_text(&self, ctx: &Scope<'_, Json>, label: &crate::model::Label, what: &str) -> String {
        let Some(src) = label.expression.as_deref().filter(|s| !s.trim().is_empty()) else {
            return label.text.clone();
        };
        match self.eval(ctx, src, what) {
            Ok(v) => match label.format.as_deref().map(str::trim).filter(|f| !f.is_empty()) {
                Some(f) if f.contains('{') => format_composite(f, &[v], self.env.formats()),
                Some(f) => format_value(&v, f, self.env.formats()),
                None => format_value(&v, "", self.env.formats()),
            },
            Err(_) => "#ERRO".to_owned(),
        }
    }

    fn text_height(&self, engine: &mut ReportEngine, text: &str, look: &Look, width: f32) -> f32 {
        let inner = (width - look.padding * 2.0).max(1.0);
        let m = engine.measure(text, look.size, look.bold, look.wrap.then_some(inner));
        m.height + look.padding * 2.0
    }

    /// Draws a text box (fill, borders, text); returns its height, grown to
    /// fit the text when `grows`.
    fn text_box(
        &self,
        engine: &mut ReportEngine,
        text: &str,
        look: &Look,
        rect: Rect,
        grows: bool,
        items: &mut Vec<Item>,
    ) -> f32 {
        let inner_w = (rect.width() - look.padding * 2.0).max(1.0);
        let m = engine.measure(text, look.size, look.bold, look.wrap.then_some(inner_w));
        let needed = m.height + look.padding * 2.0;
        let height = if grows { rect.height().max(needed) } else { rect.height() };
        let rect = Rect::new(rect.x(), rect.y(), rect.width(), height);
        backdrop(look, rect, items);
        if m.lines.is_empty() || text.is_empty() {
            return height;
        }
        let inner = Rect::new(
            rect.x() + look.padding,
            rect.y() + look.padding,
            inner_w,
            (height - look.padding * 2.0).max(0.0),
        );
        let dy = match look.valign {
            VerticalAlign::Top => 0.0,
            VerticalAlign::Bottom => inner.height() - m.height,
            _ => (inner.height() - m.height) / 2.0,
        }
        .max(0.0);
        let lines = m
            .lines
            .iter()
            .map(|(range, width, top, baseline)| {
                let dx = match look.align {
                    TextAlign::Center => (inner.width() - width) / 2.0,
                    TextAlign::Right => inner.width() - width,
                    _ => 0.0,
                };
                let s = text.get(range.clone()).unwrap_or_default().trim_end_matches(['\n', '\r']).to_owned();
                PlacedLine {
                    text: s,
                    x: inner.x() + dx.max(0.0),
                    top: inner.y() + dy + top,
                    baseline: inner.y() + dy + baseline,
                    width: *width,
                }
            })
            .collect();
        items.push(Item::Text(TextBlock {
            clip: rect,
            lines,
            size: look.size,
            line_height: (look.size * 1.25).ceil(),
            bold: look.bold,
            color: look.color,
        }));
        height
    }

    /// Places blocks on pages, then adds page headers and footers.
    fn paginate(&self, engine: &mut ReportEngine, blocks: Vec<Block>, rows: &[&Json]) -> Vec<Page> {
        let report = self.report;
        let page = report.page;
        let header_h = report.page_header.as_ref().map_or(0.0, |b| b.height);
        let footer_h = report.page_footer.as_ref().map_or(0.0, |b| b.height);
        let top = page.margin_top + header_h;
        let bottom = (page.height - page.margin_bottom - footer_h).max(top + 1.0);
        let x0 = page.margin_left;
        let mut pages: Vec<(Vec<Item>, Vec<usize>)> = vec![(Vec::new(), Vec::new())];
        let mut y = top;
        // The report header sits above the page header on the first page.
        let mut blocks = blocks;
        let mut report_header_h = 0.0;
        if blocks.first().is_some_and(|b| b.report_header) {
            let b = blocks.remove(0);
            report_header_h = b.height;
            pages[0].0.extend(b.items.iter().map(|it| translate(it, Point::new(x0, page.margin_top))));
            y += b.height;
        }
        // The latest header of each group level that repeats.
        let mut repeat: Vec<Option<(f32, Vec<Item>)>> = vec![None; report.groups.len()];
        let new_page =
            |pages: &mut Vec<(Vec<Item>, Vec<usize>)>, y: &mut f32, repeat: &[Option<(f32, Vec<Item>)>]| {
                pages.push((Vec::new(), Vec::new()));
                *y = top;
                for (h, items) in repeat.iter().flatten() {
                    let last = pages.len() - 1;
                    pages[last].0.extend(items.iter().map(|i| translate(i, Point::new(x0, *y))));
                    *y += h;
                }
            };
        let mut i = 0;
        while i < blocks.len() {
            // A run of keep-with-next blocks travels with the block after it.
            let mut j = i;
            while j + 1 < blocks.len() && blocks[j].keep_with_next {
                j += 1;
            }
            let run = &blocks[i..=j];
            let run_h: f32 = run.iter().map(|b| b.height).sum();
            let at_top = y <= top + repeat.iter().flatten().map(|(h, _)| *h).sum::<f32>() + 0.01;
            if run[0].break_before && !at_top {
                // A new group on a new page: its own header replaces the repeated one.
                if let Some(level) = run[0].header_level {
                    for r in repeat.iter_mut().skip(level) {
                        *r = None;
                    }
                }
                new_page(&mut pages, &mut y, &repeat);
            } else if y + run_h > bottom + 0.01 && !at_top {
                if let Some(level) = run[0].header_level {
                    for r in repeat.iter_mut().skip(level) {
                        *r = None;
                    }
                }
                new_page(&mut pages, &mut y, &repeat);
            }
            for b in run {
                // A single block taller than the space left starts a new page.
                let at_top = y <= top + 0.01;
                if y + b.height > bottom + 0.01 && !at_top {
                    new_page(&mut pages, &mut y, &repeat);
                }
                if let Some(level) = b.header_level {
                    let repeats = report.groups.get(level).is_some_and(|g| g.repeat_header);
                    for r in repeat.iter_mut().skip(level) {
                        *r = None;
                    }
                    if repeats {
                        repeat[level] = Some((b.height, b.items.clone()));
                    }
                }
                let last = pages.len() - 1;
                pages[last].0.extend(b.items.iter().map(|it| translate(it, Point::new(x0, y))));
                if let Some(r) = b.row {
                    pages[last].1.push(r);
                }
                y += b.height;
                if b.break_after {
                    new_page(&mut pages, &mut y, &repeat);
                }
            }
            i = j + 1;
        }
        // Drop a trailing empty page left by a final page break.
        if pages.len() > 1 && pages.last().is_some_and(|p| p.0.is_empty()) {
            pages.pop();
        }
        let count = pages.len();
        let width = page.content_width();
        pages
            .into_iter()
            .enumerate()
            .map(|(n, (mut items, page_rows))| {
                let scope: Vec<&Json> = page_rows.iter().map(|&r| rows[r]).collect();
                let first = scope.first().copied();
                let ctx = self.ctx(first, &scope, None).page(n + 1, count);
                let mut chrome = Vec::new();
                if let Some(band) = &report.page_header
                    && let Some(b) = self.band(engine, band, &ctx, width, None)
                {
                    let py = page.margin_top + if n == 0 { report_header_h } else { 0.0 };
                    chrome.extend(b.items.iter().map(|it| translate(it, Point::new(x0, py))));
                }
                if let Some(band) = &report.page_footer
                    && let Some(b) = self.band(engine, band, &ctx, width, None)
                {
                    let fy = page.height - page.margin_bottom - band.height;
                    chrome.extend(b.items.iter().map(|it| translate(it, Point::new(x0, fy))));
                }
                chrome.append(&mut items);
                Page { width: page.width, height: page.height, items: chrome }
            })
            .collect()
    }
}

/// Fill and borders of a box.
fn backdrop(look: &Look, rect: Rect, items: &mut Vec<Item>) {
    if let Some(fill) = look.background {
        items.push(Item::Rect { rect, fill: Some(fill), stroke: None, radius: 0.0 });
    }
    if let Some((w, color, [top, right, bottom, left])) = look.border {
        let (x0, y0, x1, y1) = (rect.x(), rect.y(), rect.max_x(), rect.max_y());
        let mut line = |a: Point, b: Point| items.push(Item::Line { from: a, to: b, width: w, color });
        if top {
            line(Point::new(x0, y0), Point::new(x1, y0));
        }
        if right {
            line(Point::new(x1, y0), Point::new(x1, y1));
        }
        if bottom {
            line(Point::new(x0, y1), Point::new(x1, y1));
        }
        if left {
            line(Point::new(x0, y0), Point::new(x0, y1));
        }
    }
}

fn control_name(c: &Control) -> String {
    c.name.clone().unwrap_or_else(|| match &c.kind {
        ControlKind::Label(l) => {
            l.expression.clone().map_or_else(|| format!("label '{}'", l.text), |e| format!("field {e}"))
        }
        _ => "control".to_owned(),
    })
}

/// Filters and sorts rows; returns them as owned JSON (cheap enough at report
/// sizes, and it frees the result from the input's lifetime).
fn filter_and_sort(
    run: &Run<'_>,
    rows: &[&Json],
    filter: Option<&str>,
    keys: &[SortField],
    parent: Option<&Json>,
) -> Vec<Json> {
    let mut kept: Vec<&Json> = match filter.filter(|f| !f.trim().is_empty()) {
        Some(f) => rows
            .iter()
            .copied()
            .filter(|r| {
                let ctx = run.ctx(Some(r), rows, None).parent(parent);
                run.eval(&ctx, f, "filter").map(|v| v.is_true()).unwrap_or(true)
            })
            .collect(),
        None => rows.to_vec(),
    };
    if !keys.is_empty() {
        let mut keyed: Vec<(Vec<Value>, &Json)> = kept
            .iter()
            .map(|r| {
                let ctx = run.ctx(Some(r), rows, None).parent(parent);
                (keys.iter().map(|k| run.eval(&ctx, &k.expression, "sort").unwrap_or_default()).collect(), *r)
            })
            .collect();
        keyed.sort_by(|a, b| {
            for (i, k) in keys.iter().enumerate() {
                let o = a.0[i].compare(&b.0[i]);
                let o = if k.descending { o.reverse() } else { o };
                if o.is_ne() {
                    return o;
                }
            }
            std::cmp::Ordering::Equal
        });
        kept = keyed.into_iter().map(|(_, r)| r).collect();
    }
    kept.into_iter().cloned().collect()
}

fn translate(item: &Item, by: Point) -> Item {
    let r = |rect: &Rect| Rect::from_origin_size(Point::new(rect.x() + by.x, rect.y() + by.y), rect.size);
    let p = |pt: &Point| Point::new(pt.x + by.x, pt.y + by.y);
    match item {
        Item::Rect { rect, fill, stroke, radius } => {
            Item::Rect { rect: r(rect), fill: *fill, stroke: *stroke, radius: *radius }
        }
        Item::Ellipse { rect, fill, stroke } => Item::Ellipse { rect: r(rect), fill: *fill, stroke: *stroke },
        Item::Line { from, to, width, color } => {
            Item::Line { from: p(from), to: p(to), width: *width, color: *color }
        }
        Item::Text(t) => Item::Text(TextBlock {
            clip: r(&t.clip),
            lines: t
                .lines
                .iter()
                .map(|l| PlacedLine {
                    x: l.x + by.x,
                    top: l.top + by.y,
                    baseline: l.baseline + by.y,
                    ..l.clone()
                })
                .collect(),
            ..t.clone()
        }),
    }
}

/// The JSON value at `path` under `base`.
pub(crate) fn lookup<'j>(base: Option<&'j Json>, path: &[String]) -> Option<&'j Json> {
    let mut cur = base?;
    for seg in path {
        cur = match cur {
            Json::Object(map) => map.get(seg)?,
            Json::Array(items) => items.get(seg.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(cur)
}
