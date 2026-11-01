//! XLSX and CSV export for RavenRS Reporting: lays a [`Document`] out on a grid
//! of cells that follows the positions on the page, with real numbers and dates
//! in the cells so a spreadsheet can sum and sort them.
//!
//! Pages flow into one sheet: the page header prints once, at the top, and page
//! footers are left out.
//!
//! ```
//! use ravenrs_report::{Band, Control, Report, ReportEngine};
//! use serde_json::json;
//!
//! let report = Report::new("Sales").culture("pt-BR").detail(
//!     Band::new(18.0)
//!         .control(Control::field("[Customer]").at(0.0, 0.0, 200.0, 18.0))
//!         .control(Control::field("[Amount]").format("N2").at(200.0, 0.0, 80.0, 18.0)),
//! );
//! let document = ReportEngine::new().render(&report, &json!([{ "Customer": "Ana", "Amount": 10.5 }]));
//! assert_eq!(ravenrs_report_xlsx::to_csv(&document), "Ana;10,50\r\n");
//! let xlsx = ravenrs_report_xlsx::to_xlsx(&document)?;
//! assert!(xlsx.starts_with(b"PK"));
//! # Ok::<(), ravenrs_report_xlsx::XlsxExportError>(())
//! ```

mod format;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;

use ravenrs_core::{Color, ImageData, Rect};
use ravenrs_report::vector::{Shape, rasterize};
use ravenrs_report::{Culture, Document, DocumentImage, Item, TextAlign, TextBlock};
use rust_xlsxwriter::{
    Color as XlsxColor, Format, FormatAlign, FormatBorder, Image, Url, Workbook, Worksheet, XlsxError,
};

/// Why a document could not be written as XLSX.
#[derive(Debug)]
#[non_exhaustive]
pub enum XlsxExportError {
    /// The workbook writer refused the content, for example past Excel's row limit.
    Workbook(XlsxError),
}

impl fmt::Display for XlsxExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Workbook(e) => write!(f, "could not write the workbook: {e}"),
        }
    }
}

impl std::error::Error for XlsxExportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Workbook(e) => Some(e),
        }
    }
}

impl From<XlsxError> for XlsxExportError {
    fn from(e: XlsxError) -> Self {
        Self::Workbook(e)
    }
}

/// Edges closer than this, in points, are one grid line.
const SNAP: f32 = 1.5;
/// A rule this close to a grid line, in points, borders it instead of adding a row.
const RULE_SNAP: f32 = 6.0;
/// The closest font to the report's Inter that every spreadsheet program has.
const FONT: &str = "Arial";
/// Pixels per point in Excel's column widths.
const PIXELS_PER_POINT: f32 = 96.0 / 72.0;

/// Writes `document` as an XLSX workbook with one sheet.
pub fn to_xlsx(document: &Document) -> Result<Vec<u8>, XlsxExportError> {
    let flow = Flow::new(document);
    let culture = Culture::named(&document.culture);

    let mut xs: Vec<f32> = Vec::new();
    let mut ys: Vec<f32> = Vec::new();
    for t in &flow.texts {
        xs.extend([t.rect.x(), t.rect.max_x()]);
        ys.extend([t.rect.y(), t.rect.max_y()]);
    }
    for b in &flow.boxes {
        xs.extend([b.rect.x(), b.rect.max_x()]);
        ys.extend([b.rect.y(), b.rect.max_y()]);
    }
    for drawing in &flow.drawings {
        xs.extend([drawing.rect.x(), drawing.rect.max_x()]);
        ys.extend([drawing.rect.y(), drawing.rect.max_y()]);
    }
    for picture in &flow.pictures {
        xs.extend([picture.rect.x(), picture.rect.max_x()]);
        ys.extend([picture.rect.y(), picture.rect.max_y()]);
    }
    let xs = Grid::new(xs, SNAP);
    let mut ys = Grid::new(ys, SNAP);
    for rule in &flow.rules {
        ys.add_if_far(rule.y, RULE_SNAP);
    }

    let mut styles: HashMap<(u32, u16), CellStyle> = HashMap::new();
    for b in &flow.boxes {
        let (r0, r1) = ys.span(b.rect.y(), b.rect.max_y());
        let (c0, c1) = xs.span(b.rect.x(), b.rect.max_x());
        for r in r0..r1 {
            for c in c0..c1 {
                let style = styles.entry((r, column(c))).or_default();
                if let Some(fill) = b.fill {
                    style.fill = Some(fill);
                }
                if let Some(line) = b.stroke {
                    if r == r0 {
                        style.top = Some(line);
                    }
                    if r + 1 == r1 {
                        style.bottom = Some(line);
                    }
                    if c == c0 {
                        style.left = Some(line);
                    }
                    if c + 1 == c1 {
                        style.right = Some(line);
                    }
                }
            }
        }
    }
    for rule in &flow.rules {
        let row = ys.nearest(rule.y);
        let (c0, c1) = xs.span(rule.x0, rule.x1);
        for c in c0..c1 {
            // The line between two rows belongs to the one below it; the last has none below.
            if row < ys.rows() {
                styles.entry((row, column(c))).or_default().top = Some(rule.color);
            } else if row > 0 {
                styles.entry((row - 1, column(c))).or_default().bottom = Some(rule.color);
            }
        }
    }

    let mut book = Workbook::new();
    let sheet = book.add_worksheet();
    sheet.set_name(sheet_name(&document.title))?;
    sheet.set_screen_gridlines(false);
    for (c, width) in xs.sizes().enumerate() {
        sheet
            .set_column_width_pixels(column(c as u32), (width * PIXELS_PER_POINT).round().max(1.0) as u32)?;
    }
    for (r, height) in ys.sizes().enumerate() {
        sheet.set_row_height(r as u32, f64::from(height).max(1.0))?;
    }

    let mut formats = Formats::default();
    let mut taken: HashSet<(u32, u16)> = HashSet::new();
    let mut cells: BTreeMap<(u32, u16), Placed<'_>> = BTreeMap::new();
    for t in &flow.texts {
        let (r0, r1) = ys.span(t.rect.y(), t.rect.max_y());
        let (c0, c1) = xs.span(t.rect.x(), t.rect.max_x());
        let key = (r0, column(c0));
        // Two controls over the same corner share the cell, as Excel has only one there.
        if let Some(placed) = cells.get_mut(&key) {
            placed.text.push('\n');
            placed.text.push_str(&t.text);
            placed.block = None;
            continue;
        }
        let free = (r0..r1).all(|r| (c0..c1).all(|c| !taken.contains(&(r, column(c)))));
        let (r1, c1) = if free { (r1, c1) } else { (r0 + 1, c0 + 1) };
        for r in r0..r1 {
            for c in c0..c1 {
                taken.insert((r, column(c)));
            }
        }
        let center = t.rect.center();
        let url = flow.links.iter().find(|l| l.rect.contains(center)).map(|l| l.url.as_str());
        cells.insert(key, Placed { text: t.text.clone(), block: Some(t.block), r1, c1, url });
    }

    // Filled and bordered cells without text.
    for (&(r, c), style) in &styles {
        if !taken.contains(&(r, c)) {
            let format = formats.get(&Look { style: *style, ..Look::default() });
            sheet.write_blank(r, c, format)?;
        }
    }
    for (&(r0, c0), placed) in &cells {
        write_cell(sheet, &mut formats, &styles, &culture, (r0, c0), placed)?;
    }
    for drawing in &flow.drawings {
        let Some(pixels) = rasterize(&drawing.shapes, drawing.page_rect, DRAWING_SCALE) else { continue };
        let png = drawing_png(&pixels)?;
        let width = (drawing.rect.width() * PIXELS_PER_POINT).max(1.0).round() as u32;
        let height = (drawing.rect.height() * PIXELS_PER_POINT).max(1.0).round() as u32;
        let image = Image::new_from_buffer(&png)?.set_scale_to_size(width, height, false);
        sheet.insert_image(ys.nearest(drawing.rect.y()), column(xs.nearest(drawing.rect.x())), &image)?;
    }
    for picture in &flow.pictures {
        let width = (picture.rect.width() * PIXELS_PER_POINT).max(1.0).round() as u32;
        let height = (picture.rect.height() * PIXELS_PER_POINT).max(1.0).round() as u32;
        let image = Image::new_from_buffer(picture.image.encoded())?.set_scale_to_size(width, height, false);
        sheet.insert_image(ys.nearest(picture.rect.y()), column(xs.nearest(picture.rect.x())), &image)?;
    }
    Ok(book.save_to_buffer()?)
}

/// Writes `document` as CSV: one record per line of the report, one field per
/// column of text, with the text as printed. Fields are separated by `;` in
/// cultures whose decimal separator is a comma and by `,` in the others, and
/// records end in CRLF.
pub fn to_csv(document: &Document) -> String {
    let flow = Flow::new(document);
    let separator = if Culture::named(&document.culture).decimal == ',' { ';' } else { ',' };
    let xs = Grid::new(flow.texts.iter().map(|t| t.rect.x()).collect(), SNAP);
    let ys = Grid::new(flow.texts.iter().map(|t| t.rect.y()).collect(), SNAP);
    let mut rows: BTreeMap<u32, BTreeMap<u32, String>> = BTreeMap::new();
    for t in &flow.texts {
        let field =
            rows.entry(ys.nearest(t.rect.y())).or_default().entry(xs.nearest(t.rect.x())).or_default();
        if !field.is_empty() {
            field.push(' ');
        }
        field.push_str(&t.text);
    }
    let used_last = rows.values().filter_map(|r| r.keys().next_back().copied()).max().unwrap_or(0);
    let mut out = String::new();
    for row in rows.values() {
        for c in 0..=used_last {
            if c > 0 {
                out.push(separator);
            }
            if let Some(text) = row.get(&c) {
                push_csv_field(&mut out, text, separator);
            }
        }
        out.push_str("\r\n");
    }
    out
}

fn push_csv_field(out: &mut String, text: &str, separator: char) {
    if text.contains([separator, '"', '\n', '\r']) {
        out.push('"');
        out.push_str(&text.replace('"', "\"\""));
        out.push('"');
    } else {
        out.push_str(text);
    }
}

/// The document's pages flowed into one long page.
struct Flow<'d> {
    texts: Vec<FlowText<'d>>,
    boxes: Vec<FlowBox>,
    rules: Vec<Rule>,
    pictures: Vec<FlowPicture>,
    drawings: Vec<FlowDrawing>,
    links: Vec<FlowLink>,
}

/// Shapes placed as one picture, such as a chart or a barcode. Coordinates stay those of the
/// page, so `page_rect` is what they are drawn from.
struct FlowDrawing {
    rect: Rect,
    page_rect: Rect,
    shapes: Vec<Shape>,
}

struct FlowPicture {
    rect: Rect,
    image: DocumentImage,
}

struct FlowLink {
    rect: Rect,
    url: String,
}

struct FlowText<'d> {
    rect: Rect,
    text: String,
    block: &'d TextBlock,
}

struct FlowBox {
    rect: Rect,
    fill: Option<Rgb>,
    stroke: Option<Rgb>,
}

/// A horizontal line.
struct Rule {
    y: f32,
    x0: f32,
    x1: f32,
    color: Rgb,
}

impl<'d> Flow<'d> {
    fn new(document: &'d Document) -> Self {
        let mut flow = Self {
            texts: Vec::new(),
            boxes: Vec::new(),
            rules: Vec::new(),
            pictures: Vec::new(),
            drawings: Vec::new(),
            links: Vec::new(),
        };
        // Where the next page's first item lands.
        let mut bottom = 0.0_f32;
        for (n, page) in document.pages.iter().enumerate() {
            let kept: Vec<&Item> = page
                .items
                .iter()
                .enumerate()
                .filter(|(i, _)| !page.page_footer.contains(i) && (n == 0 || !page.page_header.contains(i)))
                .map(|(_, item)| item)
                .collect();
            let top = kept.iter().filter_map(|i| extent(i)).map(|(t, _)| t).reduce(f32::min).unwrap_or(0.0);
            let end = kept.iter().filter_map(|i| extent(i)).map(|(_, b)| b).reduce(f32::max).unwrap_or(top);
            let dy = bottom - top;
            for item in kept {
                flow.add(item, dy);
            }
            bottom += end - top;
        }
        flow
    }

    fn add(&mut self, item: &'d Item, dy: f32) {
        match item {
            Item::Text(block) => {
                let text = block.lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n");
                if !text.is_empty() {
                    self.texts.push(FlowText { rect: shifted(block.clip, dy), text, block });
                }
            }
            Item::Rect { rect, fill, stroke, .. } | Item::Ellipse { rect, fill, stroke } => {
                let fill = fill.filter(|c| c.a > 0.0).map(rgb);
                let stroke = stroke.filter(|(w, c)| *w > 0.0 && c.a > 0.0).map(|(_, c)| rgb(c));
                if fill.is_some() || stroke.is_some() {
                    self.boxes.push(FlowBox { rect: shifted(*rect, dy), fill, stroke });
                }
            }
            Item::Line { from, to, color, .. } if (from.y - to.y).abs() < 0.5 => {
                self.rules.push(Rule {
                    y: from.y + dy,
                    x0: from.x.min(to.x),
                    x1: from.x.max(to.x),
                    color: rgb(*color),
                });
            }
            Item::Image { rect, image, .. } => {
                self.pictures.push(FlowPicture { rect: shifted(*rect, dy), image: image.clone() });
            }
            Item::Vector { rect, shapes } => self.drawings.push(FlowDrawing {
                rect: shifted(*rect, dy),
                page_rect: *rect,
                shapes: shapes.clone(),
            }),
            Item::Link { rect, url } => {
                self.links.push(FlowLink { rect: shifted(*rect, dy), url: url.clone() })
            }
            _ => {}
        }
    }
}

/// The top and bottom of an item.
fn extent(item: &Item) -> Option<(f32, f32)> {
    match item {
        Item::Text(t) => Some((t.clip.y(), t.clip.max_y())),
        Item::Rect { rect, .. } | Item::Ellipse { rect, .. } => Some((rect.y(), rect.max_y())),
        Item::Line { from, to, .. } => Some((from.y.min(to.y), from.y.max(to.y))),
        Item::Image { clip, .. } => Some((clip.y(), clip.max_y())),
        Item::Vector { rect, .. } => Some((rect.y(), rect.max_y())),
        _ => None,
    }
}

fn shifted(rect: Rect, dy: f32) -> Rect {
    Rect::new(rect.x(), rect.y() + dy, rect.width(), rect.height())
}

/// Pixels per point when rasterizing drawings: sharp enough for a scanner on paper.
const DRAWING_SCALE: f32 = 6.0;

fn drawing_png(pixels: &ImageData) -> Result<Vec<u8>, XlsxExportError> {
    let image = image::RgbaImage::from_raw(pixels.width(), pixels.height(), pixels.pixels().to_vec())
        .ok_or_else(|| XlsxExportError::Workbook(XlsxError::ParameterError("bad chart size".to_owned())))?;
    let mut bytes = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|e| XlsxExportError::Workbook(XlsxError::ParameterError(e.to_string())))?;
    Ok(bytes.into_inner())
}

/// Sorted grid lines on one axis.
struct Grid {
    lines: Vec<f32>,
}

impl Grid {
    fn new(mut edges: Vec<f32>, snap: f32) -> Self {
        edges.sort_by(f32::total_cmp);
        let mut lines: Vec<f32> = Vec::new();
        for edge in edges {
            if lines.last().is_none_or(|last| edge - last >= snap) {
                lines.push(edge);
            }
        }
        Self { lines }
    }

    fn add_if_far(&mut self, edge: f32, snap: f32) {
        let at = self.lines.partition_point(|l| *l < edge);
        let near = |i: usize| self.lines.get(i).is_some_and(|l| (l - edge).abs() < snap);
        if !(near(at) || (at > 0 && near(at - 1))) {
            self.lines.insert(at, edge);
        }
    }

    /// The index of the grid line nearest to `edge`.
    fn nearest(&self, edge: f32) -> u32 {
        let at = self.lines.partition_point(|l| *l < edge);
        let index = if at == 0 {
            0
        } else if at == self.lines.len() || edge - self.lines[at - 1] <= self.lines[at] - edge {
            at - 1
        } else {
            at
        };
        index as u32
    }

    /// The cells between two edges, at least one.
    fn span(&self, from: f32, to: f32) -> (u32, u32) {
        let start = self.nearest(from);
        (start, self.nearest(to).max(start + 1))
    }

    /// The number of cells between the lines.
    fn rows(&self) -> u32 {
        self.lines.len().saturating_sub(1) as u32
    }

    /// The size of each cell.
    fn sizes(&self) -> impl Iterator<Item = f32> + '_ {
        self.lines.windows(2).map(|w| w[1] - w[0])
    }
}

fn column(c: u32) -> u16 {
    u16::try_from(c).unwrap_or(u16::MAX)
}

/// A color as `0xRRGGBB`, translucent colors blended over the white page.
type Rgb = u32;

fn rgb(c: Color) -> Rgb {
    let channel = |v: f32| ((v * c.a + 1.0 - c.a).clamp(0.0, 1.0) * 255.0).round() as u32;
    channel(c.r) << 16 | channel(c.g) << 8 | channel(c.b)
}

/// Fill and borders of a cell.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
struct CellStyle {
    fill: Option<Rgb>,
    top: Option<Rgb>,
    bottom: Option<Rgb>,
    left: Option<Rgb>,
    right: Option<Rgb>,
}

impl CellStyle {
    /// Adds what `other` sets; used to give a merged cell the borders of its parts.
    fn merge(&mut self, other: CellStyle) {
        self.fill = self.fill.or(other.fill);
        self.top = self.top.or(other.top);
        self.bottom = self.bottom.or(other.bottom);
        self.left = self.left.or(other.left);
        self.right = self.right.or(other.right);
    }
}

/// Everything a cell's format depends on.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
struct Look {
    style: CellStyle,
    /// Font size in hundredths of a point; zero for a cell without text.
    size: u32,
    bold: bool,
    color: Rgb,
    align: u8,
    wrap: bool,
    number: Option<String>,
}

/// One format per distinct look: workbooks have a limit, and it keeps files small.
#[derive(Default)]
struct Formats(HashMap<Look, Format>);

impl Formats {
    fn get(&mut self, look: &Look) -> &Format {
        self.0.entry(look.clone()).or_insert_with(|| build_format(look))
    }
}

fn build_format(look: &Look) -> Format {
    let mut f = Format::new();
    if look.size > 0 {
        f = f
            .set_font_name(FONT)
            .set_font_size(f64::from(look.size) / 100.0)
            .set_font_color(XlsxColor::RGB(look.color));
        f = f.set_align(FormatAlign::VerticalCenter).set_align(match look.align {
            1 => FormatAlign::Center,
            2 => FormatAlign::Right,
            _ => FormatAlign::Left,
        });
    }
    if look.bold {
        f = f.set_bold();
    }
    if look.wrap {
        f = f.set_text_wrap();
    }
    if let Some(number) = &look.number {
        f = f.set_num_format(number);
    }
    let s = look.style;
    if let Some(fill) = s.fill {
        f = f.set_background_color(XlsxColor::RGB(fill));
    }
    if let Some(c) = s.top {
        f = f.set_border_top(FormatBorder::Thin).set_border_top_color(XlsxColor::RGB(c));
    }
    if let Some(c) = s.bottom {
        f = f.set_border_bottom(FormatBorder::Thin).set_border_bottom_color(XlsxColor::RGB(c));
    }
    if let Some(c) = s.left {
        f = f.set_border_left(FormatBorder::Thin).set_border_left_color(XlsxColor::RGB(c));
    }
    if let Some(c) = s.right {
        f = f.set_border_right(FormatBorder::Thin).set_border_right_color(XlsxColor::RGB(c));
    }
    f
}

/// A text placed on the grid.
struct Placed<'d> {
    text: String,
    /// `None` once two controls share the cell: its value is then just text.
    block: Option<&'d TextBlock>,
    r1: u32,
    c1: u32,
    /// The address the cell opens, when it is a link.
    url: Option<&'d str>,
}

fn write_cell(
    sheet: &mut Worksheet,
    formats: &mut Formats,
    styles: &HashMap<(u32, u16), CellStyle>,
    culture: &Culture,
    (r0, c0): (u32, u16),
    placed: &Placed<'_>,
) -> Result<(), XlsxError> {
    let c1 = column(placed.c1);
    let mut style = CellStyle::default();
    for r in r0..placed.r1 {
        for c in c0..c1 {
            if let Some(part) = styles.get(&(r, c)) {
                style.merge(*part);
            }
        }
    }
    let cell =
        placed.block.and_then(|b| b.value.as_ref().map(|v| format::typed(v, b.format.as_deref(), culture)));
    let (number, excel) = cell.flatten().map_or((None, None), |(n, f)| (Some(n), f));
    let block = placed.block;
    let look = Look {
        style,
        size: block.map_or(1100, |b| (b.size * 100.0).round() as u32),
        bold: block.is_some_and(|b| b.bold),
        color: block.map_or(0, |b| rgb(b.color)),
        align: match block.map(|b| b.align) {
            Some(TextAlign::Center) => 1,
            Some(TextAlign::Right) => 2,
            _ => 0,
        },
        wrap: placed.text.contains('\n'),
        number: excel,
    };
    let format = formats.get(&look).clone();
    if placed.r1 > r0 + 1 || c1 > c0 + 1 {
        sheet.merge_range(r0, c0, placed.r1 - 1, c1 - 1, &placed.text, &format)?;
    }
    if let Some(url) = placed.url {
        // Excel refuses very long addresses; the text matters more than the link.
        let link = Url::new(url).set_text(&placed.text);
        if sheet.write_url_with_format(r0, c0, link, &format).is_ok() {
            return Ok(());
        }
    }
    match number {
        Some(n) => sheet.write_number_with_format(r0, c0, n, &format)?,
        None => sheet.write_string_with_format(r0, c0, &placed.text, &format)?,
    };
    Ok(())
}

/// A sheet name Excel accepts: at most 31 characters, none of `[]:*?/\`, and
/// not starting or ending in an apostrophe.
fn sheet_name(title: &str) -> String {
    let clean: String = title
        .chars()
        .map(|c| if matches!(c, '[' | ']' | ':' | '*' | '?' | '/' | '\\') { ' ' } else { c })
        .take(31)
        .collect();
    let clean = clean.trim().trim_matches('\'').trim();
    if clean.is_empty() { "Report".to_owned() } else { clean.to_owned() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_lines_snap_and_cells_never_collapse() {
        let grid = Grid::new(vec![10.0, 10.5, 40.0, 41.0, 100.0], 1.5);
        assert_eq!(grid.lines, [10.0, 40.0, 100.0]);
        assert_eq!(grid.span(10.4, 40.7), (0, 1));
        assert_eq!(grid.span(40.0, 40.2), (1, 2));
        assert_eq!(grid.span(10.0, 100.0), (0, 2));
    }

    #[test]
    fn rules_near_a_line_add_no_row() {
        let mut grid = Grid::new(vec![0.0, 20.0], 1.5);
        grid.add_if_far(21.0, 6.0);
        assert_eq!(grid.lines, [0.0, 20.0]);
        grid.add_if_far(40.0, 6.0);
        assert_eq!(grid.lines, [0.0, 20.0, 40.0]);
    }

    #[test]
    fn sheet_names_are_what_excel_accepts() {
        assert_eq!(sheet_name("Vendas: 2026/11"), "Vendas  2026 11");
        assert_eq!(sheet_name("'Fatura'"), "Fatura");
        assert_eq!(sheet_name("???"), "Report");
        assert_eq!(sheet_name(&"x".repeat(40)).chars().count(), 31);
    }

    #[test]
    fn translucent_colors_blend_over_white() {
        assert_eq!(rgb(Color::rgba(0.0, 0.0, 0.0, 1.0)), 0x000000);
        assert_eq!(rgb(Color::rgba(0.0, 0.0, 0.0, 0.0)), 0xffffff);
        assert_eq!(rgb(Color::rgba(1.0, 0.0, 0.0, 0.5)), 0xff8080);
    }
}
