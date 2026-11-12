//! Draws a page in black and white at the printer's resolution. Everything goes through a threshold;
//! barcodes do not even get that far, since their bars are laid on whole dots.

use ravenrs_core::{Color, ImageData, Point, Primitive, Rect, Scene};
use ravenrs_report::barcode::{Pattern, Symbology, encode, modules_across, runs};
use ravenrs_report::vector::{self, Shape};
use ravenrs_report::{DocumentImage, Item, Page, TextBlock};
use ravenrs_text::{TextAttrs, TextSystem};
use swash::scale::{Render, ScaleContext, Source};
use swash::zeno::{Format, Vector};

use crate::bitmap::Bitmap;
use crate::error::LabelError;

const POINTS_PER_INCH: f32 = 72.0;
const MM_PER_INCH: f32 = 25.4;
/// The most dots across, which is also the most a drawing can be rasterized at.
const MAX_WIDTH: u32 = 4096;
/// The most dots down: about 25 m of receipt paper at 203 dpi, which keeps the picture in megabytes.
const MAX_HEIGHT: u32 = 200_000;
/// A drawing taller than this is rasterized in bands. Each band stays well below the rasterizer's limit.
const DRAWING_BAND: u32 = 2048;
/// A glyph or shape covering at least this much of a dot burns it.
const COVERAGE: u8 = 128;

/// GS1's smallest EAN/UPC module: 80% of the nominal 0.33 mm.
const MINIMUM_EAN_UPC_MM: f32 = 0.264;
/// The smallest module for the other formats, 7.5 mil. It is the usual floor of general scanners,
/// not a rule of these standards: see FONTES.md.
const MINIMUM_OTHER_MM: f32 = 0.19;

fn minimum_module_mm(symbology: Symbology) -> f32 {
    match symbology {
        Symbology::Ean13 | Symbology::Ean8 | Symbology::UpcA => MINIMUM_EAN_UPC_MM,
        _ => MINIMUM_OTHER_MM,
    }
}

fn symbology_name(symbology: Symbology) -> &'static str {
    match symbology {
        Symbology::Code128 => "Code 128",
        Symbology::Code39 => "Code 39",
        Symbology::Ean13 => "EAN-13",
        Symbology::Ean8 => "EAN-8",
        Symbology::UpcA => "UPC-A",
        Symbology::Itf => "ITF",
        Symbology::Qr => "QR Code",
        Symbology::Itf14 => "ITF-14",
        _ => "desconhecido",
    }
}

/// What a color does to the paper: `Some(true)` burns, `Some(false)` leaves white over what is below,
/// and `None` does nothing because it is too transparent.
fn ink(gray: f32, alpha: f32) -> Option<bool> {
    if alpha * (1.0 - gray) >= 0.5 {
        Some(true)
    } else if alpha >= 0.5 {
        Some(false)
    } else {
        None
    }
}

fn gray_of(r: f32, g: f32, b: f32) -> f32 {
    0.299 * r + 0.587 * g + 0.114 * b
}

fn color_ink(color: Color) -> Option<bool> {
    ink(gray_of(color.r, color.g, color.b), color.a)
}

fn pixel_ink(pixel: &[u8]) -> Option<bool> {
    let &[r, g, b, a] = pixel else { return None };
    let unit = |channel: u8| f32::from(channel) / 255.0;
    ink(gray_of(unit(r), unit(g), unit(b)), unit(a))
}

/// A page's items in black and white, at `dpi` dots per inch.
///
/// A barcode that does not keep its minimum module on whole dots gives an error, which names it.
pub fn rasterize_page(page: &Page, dpi: u32, text: &mut TextSystem) -> Result<Bitmap, LabelError> {
    if dpi == 0 {
        return Err(LabelError::InvalidPage("a resolução precisa ser maior que zero".to_owned()));
    }
    let scale = dpi as f32 / POINTS_PER_INCH;
    let (width, height) = ((page.width * scale).round(), (page.height * scale).round());
    if !(width >= 1.0 && height >= 1.0 && width <= MAX_WIDTH as f32 && height <= MAX_HEIGHT as f32) {
        return Err(LabelError::InvalidPage(format!(
            "tamanho de {width} x {height} pontos; o limite é {MAX_WIDTH} x {MAX_HEIGHT}"
        )));
    }
    let mut painter =
        Painter { bitmap: Bitmap::new(width as u32, height as u32), scale, dpi, glyphs: ScaleContext::new() };
    let mut items = page.items.iter().peekable();
    while let Some(item) = items.next() {
        match item {
            Item::BarcodeMark { rect, symbology, content } => {
                painter.barcode(*rect, *symbology, content)?;
                // The drawing right after it is the same code: drawn already, on whole dots.
                if matches!(items.peek(), Some(Item::Vector { rect: drawn, .. }) if drawn == rect) {
                    items.next();
                }
            }
            Item::Rect { rect, fill, stroke, radius } => painter.rect(*rect, *fill, *stroke, *radius),
            Item::Ellipse { rect, fill, stroke } => {
                let stroke = painter.thick_enough(*stroke);
                let shape = Shape { path: vector::ellipse(*rect), fill: *fill, stroke };
                painter.shapes(&[shape], outset(*rect, stroke));
            }
            Item::Line { from, to, width, color } => painter.line(*from, *to, *width, *color),
            Item::Text(block) => painter.text(block, text),
            Item::Image { rect, clip, image } => painter.image(*rect, *clip, image),
            Item::Vector { rect, shapes } => painter.shapes(shapes, *rect),
            _ => {}
        }
    }
    Ok(painter.bitmap)
}

/// A box grown by half of its outline.
fn outset(rect: Rect, stroke: Option<(f32, Color)>) -> Rect {
    let grow = stroke.map_or(0.0, |(width, _)| width / 2.0 + 1.0);
    Rect::new(rect.x() - grow, rect.y() - grow, rect.width() + 2.0 * grow, rect.height() + 2.0 * grow)
}

struct Painter {
    bitmap: Bitmap,
    /// Dots per point.
    scale: f32,
    dpi: u32,
    glyphs: ScaleContext,
}

impl Painter {
    /// A position in points as the dot it falls on, held inside the page.
    fn dot(&self, points: f32, limit: u32) -> u32 {
        (points * self.scale).round().clamp(0.0, limit as f32) as u32
    }

    fn dot_x(&self, points: f32) -> u32 {
        self.dot(points, self.bitmap.width())
    }

    fn dot_y(&self, points: f32) -> u32 {
        self.dot(points, self.bitmap.height())
    }

    /// An outline no thinner than a dot, so it does not vanish at the threshold.
    fn thick_enough(&self, stroke: Option<(f32, Color)>) -> Option<(f32, Color)> {
        stroke.map(|(width, color)| (width.max(1.0 / self.scale), color))
    }

    fn fill_rect(&mut self, rect: Rect, color: Color) {
        if let Some(black) = color_ink(color) {
            let (x0, y0) = (self.dot_x(rect.x()), self.dot_y(rect.y()));
            let (x1, y1) = (self.dot_x(rect.max_x()), self.dot_y(rect.max_y()));
            self.bitmap.fill(x0, y0, x1, y1, black);
        }
    }

    fn rect(&mut self, rect: Rect, fill: Option<Color>, stroke: Option<(f32, Color)>, radius: f32) {
        if radius > 0.0 {
            let stroke = self.thick_enough(stroke);
            let shape = Shape { path: vector::rectangle(rect, radius), fill, stroke };
            self.shapes(&[shape], outset(rect, stroke));
            return;
        }
        if let Some(fill) = fill {
            self.fill_rect(rect, fill);
        }
        if let Some((width, color)) = stroke {
            let (left, right) = (rect.x(), rect.max_x());
            let (top, bottom) = (rect.y(), rect.max_y());
            let corners = [
                (Point::new(left, top), Point::new(right, top)),
                (Point::new(right, top), Point::new(right, bottom)),
                (Point::new(right, bottom), Point::new(left, bottom)),
                (Point::new(left, bottom), Point::new(left, top)),
            ];
            for (from, to) in corners {
                self.line(from, to, width, color);
            }
        }
    }

    fn line(&mut self, from: Point, to: Point, width: f32, color: Color) {
        let thick = width.max(1.0 / self.scale);
        let (dots, half) = ((thick * self.scale).round().max(1.0) as u32, thick / 2.0);
        let Some(black) = color_ink(color) else { return };
        if (from.y - to.y).abs() < f32::EPSILON {
            let y0 = self.dot_y(from.y - half);
            let (x0, x1) = (self.dot_x(from.x.min(to.x) - half), self.dot_x(from.x.max(to.x) + half));
            self.bitmap.fill(x0, y0, x1, y0 + dots, black);
        } else if (from.x - to.x).abs() < f32::EPSILON {
            let x0 = self.dot_x(from.x - half);
            let (y0, y1) = (self.dot_y(from.y.min(to.y) - half), self.dot_y(from.y.max(to.y) + half));
            self.bitmap.fill(x0, y0, x0 + dots, y1, black);
        } else {
            let path = vec![vector::PathSegment::Move(from), vector::PathSegment::Line(to)];
            let shape = Shape { path, fill: None, stroke: Some((thick, color)) };
            let bounds =
                Rect::new(from.x.min(to.x), from.y.min(to.y), (from.x - to.x).abs(), (from.y - to.y).abs());
            self.shapes(&[shape], outset(bounds, Some((thick, color))));
        }
    }

    /// Rasterizes shapes in a box lined up with the dots, so each pixel is exactly one dot.
    fn shapes(&mut self, shapes: &[Shape], bounds: Rect) {
        let scale = self.scale;
        let x0 = (bounds.x() * scale).floor().max(0.0);
        let y0 = (bounds.y() * scale).floor().max(0.0);
        let x1 = (bounds.max_x() * scale).ceil().min(self.bitmap.width() as f32);
        let y1 = (bounds.max_y() * scale).ceil().min(self.bitmap.height() as f32);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let (left, width) = (x0 / scale, (x1 - x0) / scale);
        let (mut top, bottom) = (y0 as u32, y1 as u32);
        while top < bottom {
            let band = DRAWING_BAND.min(bottom - top);
            let aligned = Rect::new(left, top as f32 / scale, width, band as f32 / scale);
            if let Some(picture) = vector::rasterize(shapes, aligned, scale) {
                self.stamp(&picture, x0 as u32, top, band);
            }
            top += band;
        }
    }

    /// Copies `rows` rows of a picture. Rounding can make it one row taller, which would cover the
    /// next band's first row.
    fn stamp(&mut self, picture: &ImageData, left: u32, top: u32, rows: u32) {
        let width = picture.width().max(1) as usize;
        for (row, pixels) in picture.pixels().chunks_exact(width * 4).take(rows as usize).enumerate() {
            for (column, pixel) in pixels.as_chunks::<4>().0.iter().enumerate() {
                if let Some(black) = pixel_ink(pixel) {
                    self.bitmap.set(left + column as u32, top + row as u32, black);
                }
            }
        }
    }

    fn text(&mut self, block: &TextBlock, text: &mut TextSystem) {
        let Some(black) = color_ink(block.color) else { return };
        let attrs = TextAttrs {
            size: block.size * self.scale,
            line_height: block.line_height * self.scale,
            weight: ravenrs_report::text_weight(block.bold),
            letter_spacing: 0.0,
            family: None,
        };
        let clip = (
            (block.clip.x() * self.scale).floor(),
            (block.clip.y() * self.scale).floor(),
            (block.clip.max_x() * self.scale).ceil(),
            (block.clip.max_y() * self.scale).ceil(),
        );
        for line in &block.lines {
            let mut layout = text.layout(&line.text, &attrs);
            layout.measure(None);
            let mut scene = Scene::new();
            let origin = Point::new(line.x * self.scale, line.top * self.scale);
            layout.paint(origin, Color::BLACK, &mut scene);
            for primitive in scene.primitives() {
                if let Primitive::Glyphs(run) = primitive {
                    self.glyph_run(run, black, clip);
                }
            }
        }
    }

    fn glyph_run(&mut self, run: &ravenrs_core::GlyphRun, black: bool, clip: (f32, f32, f32, f32)) {
        let Some(font) = swash::FontRef::from_index(run.font.bytes(), run.font.index as usize) else {
            return;
        };
        let mut scaler = self.glyphs.builder(font).size(run.size).hint(true).build();
        for glyph in &run.glyphs {
            let Ok(id) = u16::try_from(glyph.id) else { continue };
            let pen = glyph.x.floor();
            let Some(image) = Render::new(&[Source::Outline])
                .format(Format::Alpha)
                .offset(Vector::new(glyph.x - pen, 0.0))
                .render(&mut scaler, id)
            else {
                continue;
            };
            let left = pen as i32 + image.placement.left;
            let top = glyph.y.round() as i32 - image.placement.top;
            let width = image.placement.width.max(1) as usize;
            for (row, coverage) in image.data.chunks_exact(width).enumerate() {
                for (column, &alpha) in coverage.iter().enumerate() {
                    let (x, y) = (left + column as i32, top + row as i32);
                    let inside = x as f32 >= clip.0
                        && (x as f32) < clip.2
                        && y as f32 >= clip.1
                        && (y as f32) < clip.3;
                    if alpha >= COVERAGE && inside && x >= 0 && y >= 0 {
                        self.bitmap.set(x as u32, y as u32, black);
                    }
                }
            }
        }
    }

    /// Takes the nearest pixel of the image for each dot, cut at the control's box.
    fn image(&mut self, rect: Rect, clip: Rect, image: &DocumentImage) {
        let pixels = image.pixels();
        let (width, height) = (pixels.width() as f32, pixels.height() as f32);
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return;
        }
        for y in self.dot_y(clip.y())..self.dot_y(clip.max_y()) {
            let v = ((y as f32 + 0.5) / self.scale - rect.y()) / rect.height();
            if !(0.0..1.0).contains(&v) {
                continue;
            }
            for x in self.dot_x(clip.x())..self.dot_x(clip.max_x()) {
                let u = ((x as f32 + 0.5) / self.scale - rect.x()) / rect.width();
                if !(0.0..1.0).contains(&u) {
                    continue;
                }
                let at = ((v * height) as usize * pixels.width() as usize + (u * width) as usize) * 4;
                if let Some(black) = pixels.pixels().get(at..at + 4).and_then(pixel_ink) {
                    self.bitmap.set(x, y, black);
                }
            }
        }
    }

    fn barcode(&mut self, rect: Rect, symbology: Symbology, content: &str) -> Result<(), LabelError> {
        let name = symbology_name(symbology);
        let pattern = encode(symbology, content).map_err(|reason| LabelError::InvalidBarcode {
            symbology: name,
            content: content.to_owned(),
            reason,
        })?;
        let across = modules_across(&pattern) as u32;
        // Only whole dots inside the box: the quiet zone must stay in it.
        let left = (rect.x() * self.scale).ceil().max(0.0) as u32;
        let top = (rect.y() * self.scale).ceil().max(0.0) as u32;
        let right = ((rect.max_x() * self.scale).floor().max(0.0) as u32).min(self.bitmap.width());
        let bottom = ((rect.max_y() * self.scale).floor().max(0.0) as u32).min(self.bitmap.height());
        let width = right.saturating_sub(left);
        let height = bottom.saturating_sub(top);
        let available = match pattern {
            Pattern::Linear { .. } => width,
            Pattern::Matrix { .. } => width.min(height),
        };
        let module = available / across.max(1);
        let minimum_mm = minimum_module_mm(symbology);
        let dot_mm = MM_PER_INCH / self.dpi as f32;
        let minimum = ((minimum_mm / dot_mm).ceil() as u32).max(1);
        if module < minimum || height == 0 {
            return Err(LabelError::BarcodeDoesNotFit {
                symbology: name,
                content: content.to_owned(),
                at_mm: (left as f32 * dot_mm, top as f32 * dot_mm),
                available_mm: available as f32 * dot_mm,
                needed_mm: (minimum * across) as f32 * dot_mm,
                module_dots: module,
                minimum_dots: minimum,
                minimum_mm,
                dpi: self.dpi,
            });
        }
        // The quiet zone and whatever the leftover dots leave are white.
        self.bitmap.fill(left, top, right, bottom, false);
        match &pattern {
            Pattern::Linear { modules, .. } => {
                let start = left + (width - module * modules.len() as u32) / 2;
                for (first, length) in runs(modules) {
                    let x = start + first as u32 * module;
                    self.bitmap.fill(x, top, x + length as u32 * module, bottom, true);
                }
            }
            Pattern::Matrix { size, cells } => {
                let side = module * *size as u32;
                let start_x = left + (width - side) / 2;
                let start_y = top + (height - side) / 2;
                for (row, line) in cells.chunks((*size).max(1)).enumerate() {
                    let y = start_y + row as u32 * module;
                    for (first, length) in runs(line) {
                        let x = start_x + first as u32 * module;
                        self.bitmap.fill(x, y, x + length as u32 * module, y + module, true);
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(items: Vec<Item>) -> Page {
        Page { width: 72.0, height: 72.0, items, ..Page::default() }
    }

    fn draw(items: Vec<Item>) -> Result<Bitmap, LabelError> {
        rasterize_page(&page(items), 203, &mut TextSystem::new())
    }

    /// The width in dots of each black run on a row.
    fn black_runs(bitmap: &Bitmap, y: u32) -> Vec<u32> {
        let mut widths = Vec::new();
        let mut run = 0;
        for x in 0..=bitmap.width() {
            if bitmap.get(x, y) {
                run += 1;
            } else if run > 0 {
                widths.push(run);
                run = 0;
            }
        }
        widths
    }

    fn mark(rect: Rect, symbology: Symbology, content: &str) -> Item {
        Item::BarcodeMark { rect, symbology, content: content.to_owned() }
    }

    #[test]
    fn the_page_size_follows_the_resolution() {
        let mut text = TextSystem::new();
        let at_203 = rasterize_page(&page(Vec::new()), 203, &mut text).map(|b| (b.width(), b.height()));
        let at_300 = rasterize_page(&page(Vec::new()), 300, &mut text).map(|b| (b.width(), b.height()));
        assert_eq!(at_203, Ok((203, 203)));
        assert_eq!(at_300, Ok((300, 300)));
    }

    #[test]
    fn a_rule_is_all_black_and_exact() {
        let bitmap = draw(vec![Item::Rect {
            rect: Rect::new(0.0, 0.0, 36.0, 36.0),
            fill: Some(Color::BLACK),
            stroke: None,
            radius: 0.0,
        }]);
        let bitmap = bitmap.unwrap_or_else(|e| panic!("{e}"));
        assert!(bitmap.get(0, 0) && bitmap.get(100, 100));
        assert!(!bitmap.get(102, 102));
    }

    #[test]
    fn a_pale_fill_stays_white_and_a_dark_one_burns() {
        let fill = |gray: f32| Item::Rect {
            rect: Rect::new(0.0, 0.0, 72.0, 72.0),
            fill: Some(Color::rgba(gray, gray, gray, 1.0)),
            stroke: None,
            radius: 0.0,
        };
        let pale = draw(vec![fill(0.8)]).unwrap_or_else(|e| panic!("{e}"));
        let dark = draw(vec![fill(0.2)]).unwrap_or_else(|e| panic!("{e}"));
        assert!(!pale.get(10, 10));
        assert!(dark.get(10, 10));
    }

    #[test]
    fn bars_are_whole_dots_and_the_drawing_after_the_mark_is_skipped() {
        let rect = Rect::new(0.0, 0.0, 72.0, 36.0);
        // A black box right after the mark stands for the drawing: it must not be printed.
        let drawing =
            Item::Vector { rect, shapes: vec![Shape::filled(vector::rectangle(rect, 0.0), Color::BLACK)] };
        let bitmap = draw(vec![mark(rect, Symbology::Code128, "AB"), drawing]);
        let bitmap = bitmap.unwrap_or_else(|e| panic!("{e}"));
        let widths = black_runs(&bitmap, 10);
        assert!(!widths.is_empty());
        let module = widths.iter().copied().min().unwrap_or(0);
        assert!(module >= 2, "{widths:?}");
        assert!(widths.iter().all(|w| w % module == 0), "{widths:?}");
        assert!(!bitmap.get(0, 10), "the quiet zone is white");
    }

    #[test]
    fn a_drawing_that_is_not_the_mark_is_still_printed() {
        let rect = Rect::new(0.0, 0.0, 72.0, 36.0);
        let other = Rect::new(0.0, 40.0, 72.0, 20.0);
        let drawing = Item::Vector {
            rect: other,
            shapes: vec![Shape::filled(vector::rectangle(other, 0.0), Color::BLACK)],
        };
        let bitmap = draw(vec![mark(rect, Symbology::Code128, "AB"), drawing]);
        let bitmap = bitmap.unwrap_or_else(|e| panic!("{e}"));
        assert!(bitmap.get(100, 150));
    }

    #[test]
    fn a_code_with_no_room_is_an_explicit_error() {
        let rect = Rect::new(4.0, 8.0, 36.0, 36.0);
        let error = draw(vec![mark(rect, Symbology::Ean13, "789123456789")]);
        let Err(error) = error else { panic!("should not fit") };
        let LabelError::BarcodeDoesNotFit { module_dots, minimum_dots, .. } = &error else {
            panic!("{error:?}")
        };
        assert_eq!((*module_dots, *minimum_dots), (0, 3));
        let text = error.to_string();
        assert!(text.contains("EAN-13") && text.contains("789123456789") && text.contains("não cabe"));
    }

    #[test]
    fn ean_at_203_dpi_needs_three_dots_since_two_are_below_the_gs1_minimum() {
        // Room for 2 dots a module (0.250 mm), and GS1's floor is 0.264 mm.
        let rect = Rect::new(0.0, 0.0, 240.0 * 72.0 / 203.0, 30.0);
        let page =
            Page { width: 200.0, height: 72.0, ..page(vec![mark(rect, Symbology::Ean13, "789123456789")]) };
        let result = rasterize_page(&page, 203, &mut TextSystem::new());
        assert!(matches!(result, Err(LabelError::BarcodeDoesNotFit { module_dots: 2, minimum_dots: 3, .. })));
    }

    #[test]
    fn a_qr_stays_square_and_centered_in_its_box() {
        let rect = Rect::new(0.0, 0.0, 72.0, 72.0);
        let bitmap = draw(vec![mark(rect, Symbology::Qr, "RAVEN")]).unwrap_or_else(|e| panic!("{e}"));
        let first = (0..203).find(|&y| !black_runs(&bitmap, y).is_empty()).unwrap_or(0);
        let last = (0..203).rev().find(|&y| !black_runs(&bitmap, y).is_empty()).unwrap_or(0);
        assert!(first > 0);
        assert!(first.abs_diff(202 - last) <= 1, "{first} {last}");
    }

    #[test]
    fn invalid_content_is_an_error_and_not_a_panic() {
        let error = draw(vec![mark(Rect::new(0.0, 0.0, 72.0, 36.0), Symbology::Ean13, "abc")]);
        assert!(matches!(error, Err(LabelError::InvalidBarcode { .. })));
    }

    #[test]
    fn text_comes_out_black_with_accents() {
        let mut text = TextSystem::new();
        let block = TextBlock {
            clip: Rect::new(0.0, 0.0, 72.0, 30.0),
            lines: vec![ravenrs_report::PlacedLine {
                text: "Açúcar".to_owned(),
                x: 2.0,
                top: 2.0,
                baseline: 14.0,
                width: 60.0,
            }],
            size: 12.0,
            line_height: 15.0,
            bold: false,
            color: Color::BLACK,
            align: ravenrs_report::TextAlign::Left,
            value: None,
            format: None,
        };
        let bitmap = rasterize_page(&page(vec![Item::Text(block)]), 203, &mut text);
        let bitmap = bitmap.unwrap_or_else(|e| panic!("{e}"));
        let black = bitmap.bits().iter().map(|b| b.count_ones()).sum::<u32>();
        assert!(black > 200, "{black}");
        assert!(!(0..203).any(|x| bitmap.get(x, 100)), "clipped at its box");
    }

    #[test]
    fn a_huge_page_is_refused() {
        let mut text = TextSystem::new();
        let wide = Page { width: 100_000.0, height: 72.0, ..Page::default() };
        assert!(matches!(rasterize_page(&wide, 203, &mut text), Err(LabelError::InvalidPage(_))));
        let endless = Page { width: 200.0, height: 1_000_000.0, ..Page::default() };
        assert!(matches!(rasterize_page(&endless, 203, &mut text), Err(LabelError::InvalidPage(_))));
    }

    #[test]
    fn a_very_tall_page_uses_its_height_and_draws_to_the_bottom() {
        let height = 10_000.0 * 72.0 / 203.0;
        let full = Rect::new(10.0, 10.0, 100.0, height - 20.0);
        // Taller than the rasterizer's own limit: it must come out whole, with no seam between bands.
        let drawing = Item::Vector {
            rect: full,
            shapes: vec![Shape::filled(vector::rectangle(full, 0.0), Color::BLACK)],
        };
        let last = Rect::new(0.0, height - 5.0, 200.0, 5.0);
        let rule = Item::Rect { rect: last, fill: Some(Color::BLACK), stroke: None, radius: 0.0 };
        let page = Page { width: 200.0, height, items: vec![drawing, rule], ..Page::default() };
        let bitmap = rasterize_page(&page, 203, &mut TextSystem::new()).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(bitmap.height(), 10_000);
        assert!(bitmap.get(0, 9_999), "the rule at the bottom");
        assert!(!bitmap.get(100, 20), "above the drawing");
        for y in 40..9_900 {
            assert!(bitmap.get(100, y), "row {y} of the drawing");
        }
    }
}
