//! Saves what a [`FlowCanvas`](ravenrs_widgets::FlowCanvas) exports as a file: a PNG picture at
//! twice the diagram's size, an SVG drawing, or a PDF page through the report engine's
//! exporter.
//!
//! ```
//! use ravenrs_flow_export::export;
//! use ravenrs_widgets::{FlowColors, FlowExportFormat, FlowGraph, FlowNode};
//!
//! let graph = FlowGraph::new().node(FlowNode::new("a", "step", "Start", 0.0, 0.0));
//! let picture = graph.picture(None, FlowColors::paper()).expect("a node to draw");
//! let file = export(&picture, FlowExportFormat::Svg, "Order flow")?;
//! assert_eq!(file.name, "Order flow.svg");
//! assert!(String::from_utf8_lossy(&file.bytes).contains("Start"));
//! # Ok::<(), String>(())
//! ```

use std::io::Cursor;

use ravenrs_core::{Color, Point, Primitive, Rect, Scene};
use ravenrs_graphics::vector::{self, Layer, Shape};
use ravenrs_report::{Document, Item, Page, PlacedLine, TextAlign, TextBlock};
use ravenrs_report_export::ExportFormat;
use ravenrs_text::{TextAttrs, TextSystem};
use ravenrs_widgets::{FilePicker, FlowExportFormat, FlowPicture, FlowText};
use swash::scale::{Render, ScaleContext, Source};
use swash::zeno::{Format, Vector};

/// How many pixels a PNG gets per unit of the diagram.
pub const PNG_SCALE: f32 = 2.0;

/// The family SVG names for its words: the one the screen draws them in.
const SVG_FAMILY: &str = "Inter, system-ui, sans-serif";

/// A diagram written as a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlowFile {
    /// The file's name: the title, with what file systems refuse taken out, and the extension.
    pub name: String,
    /// The media type.
    pub mime: &'static str,
    /// The content.
    pub bytes: Vec<u8>,
}

/// Writes `picture` in `format`, named after `title`.
pub fn export(picture: &FlowPicture, format: FlowExportFormat, title: &str) -> Result<FlowFile, String> {
    let bytes = match format {
        FlowExportFormat::Svg => picture.to_svg(SVG_FAMILY).into_bytes(),
        FlowExportFormat::Png => png(picture, PNG_SCALE, &mut TextSystem::new())?,
        FlowExportFormat::Pdf => pdf(picture, title, &mut TextSystem::new())?,
        _ => return Err(format!("{format:?} is not written yet")),
    };
    Ok(FlowFile { name: file_name(title, format), mime: format.mime(), bytes })
}

/// What [`FlowCanvas::on_export`](ravenrs_widgets::FlowCanvas::on_export) takes to hand each
/// export to the person as a file, through the platform's way of saving one.
pub fn saver(
    picker: FilePicker,
    title: impl Into<String>,
) -> impl Fn(FlowExportFormat, FlowPicture) + 'static {
    let title = title.into();
    move |format, picture| {
        if let Ok(file) = export(&picture, format, &title) {
            picker.save(&file.name, &file.bytes);
        }
    }
}

/// A title as a file name, keeping accents and dropping what file systems refuse.
pub fn file_name(title: &str, format: FlowExportFormat) -> String {
    let kept: String = title
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') && !c.is_control())
        .collect();
    let kept = kept.trim();
    let stem = if kept.is_empty() { "diagram" } else { kept };
    format!("{stem}.{}", format.extension())
}

/// Where one run of words lands, in units of the diagram from the picture's corner: its
/// left, top, baseline and width, and the words that fit.
struct Placed {
    text: String,
    left: f32,
    top: f32,
    baseline: f32,
    width: f32,
    height: f32,
}

fn attrs(t: &FlowText, scale: f32) -> TextAttrs {
    TextAttrs {
        size: t.size * scale,
        line_height: t.size * 1.3 * scale,
        weight: ravenrs_report::text_weight(t.bold),
        letter_spacing: 0.0,
        family: None,
    }
}

/// The words of `t` that fit its width, ended with `…` when some are cut.
fn fitted(t: &FlowText, text: &mut TextSystem) -> String {
    let fits = |words: &str, text: &mut TextSystem| {
        let mut layout = text.layout(words, &attrs(t, 1.0));
        layout.measure(None).width <= t.width
    };
    if fits(&t.text, text) {
        return t.text.clone();
    }
    let mut chars: Vec<char> = t.text.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let words = format!("{}…", chars.iter().collect::<String>().trim_end());
        if fits(&words, text) {
            return words;
        }
    }
    String::new()
}

/// Lays `t` out, cut to its width, and says where it sits from `origin`.
fn place(t: &FlowText, origin: Point, text: &mut TextSystem) -> Option<Placed> {
    let shown = fitted(t, text);
    let mut layout = text.layout(&shown, &attrs(t, 1.0));
    layout.measure(None);
    let line = layout.lines().into_iter().next()?;
    let left = if t.centered { t.at.x - line.width / 2.0 } else { t.at.x } - origin.x;
    let top = t.at.y - line.height / 2.0 - origin.y;
    Some(Placed {
        text: shown,
        left,
        top,
        baseline: top + line.baseline,
        width: line.width,
        height: line.height,
    })
}

/// The picture in pixels, `scale` per unit, encoded as PNG.
fn png(picture: &FlowPicture, scale: f32, text: &mut TextSystem) -> Result<Vec<u8>, String> {
    let image = vector::rasterize_layers(&picture.layers, picture.bounds, scale)
        .ok_or_else(|| "the diagram is too large for a picture".to_owned())?;
    let (width, height) = (image.width(), image.height());
    let mut pixels = image.pixels().to_vec();
    let mut glyphs = ScaleContext::new();
    let origin = picture.bounds.origin;
    for t in &picture.texts {
        let mut layout = text.layout_ellipsized(&t.text, &attrs(t, scale), (t.width * scale).max(1.0));
        layout.measure(None);
        let Some(line) = layout.lines().into_iter().next() else { continue };
        let left = if t.centered { t.at.x * scale - line.width / 2.0 } else { t.at.x * scale };
        let top = t.at.y * scale - line.height / 2.0;
        let mut scene = Scene::new();
        layout.paint(Point::new(left - origin.x * scale, top - origin.y * scale), t.color, &mut scene);
        for primitive in scene.primitives() {
            if let Primitive::Glyphs(run) = primitive {
                draw_run(&mut pixels, (width, height), run, t.color, &mut glyphs);
            }
        }
    }
    let buffer = image::RgbaImage::from_raw(width, height, pixels).ok_or_else(|| "bad picture".to_owned())?;
    let mut out = Cursor::new(Vec::new());
    buffer.write_to(&mut out, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

/// Lays a run of glyphs over straight-alpha RGBA pixels in `color`.
fn draw_run(
    pixels: &mut [u8],
    (width, height): (u32, u32),
    run: &ravenrs_core::GlyphRun,
    color: Color,
    context: &mut ScaleContext,
) {
    let Some(font) = swash::FontRef::from_index(run.font.bytes(), run.font.index as usize) else { return };
    let mut scaler = context.builder(font).size(run.size).hint(false).build();
    let [r, g, b, a] = color.to_array();
    for glyph in &run.glyphs {
        let Ok(id) = u16::try_from(glyph.id) else { continue };
        let pen = glyph.x.floor();
        let Some(mask) = Render::new(&[Source::Outline])
            .format(Format::Alpha)
            .offset(Vector::new(glyph.x - pen, 0.0))
            .render(&mut scaler, id)
        else {
            continue;
        };
        let left = pen as i64 + i64::from(mask.placement.left);
        let top = glyph.y.round() as i64 - i64::from(mask.placement.top);
        let w = mask.placement.width.max(1) as usize;
        for (row, coverage) in mask.data.chunks_exact(w).enumerate() {
            for (column, &cover) in coverage.iter().enumerate() {
                let (x, y) = (left + column as i64, top + row as i64);
                if cover == 0 || x < 0 || y < 0 || x >= i64::from(width) || y >= i64::from(height) {
                    continue;
                }
                let i = ((y as usize) * width as usize + x as usize) * 4;
                let alpha = f32::from(cover) / 255.0 * a;
                let under = f32::from(pixels[i + 3]) / 255.0;
                let out = alpha + under * (1.0 - alpha);
                if out <= 0.0 {
                    continue;
                }
                for (k, ink) in [r, g, b].into_iter().enumerate() {
                    let below = f32::from(pixels[i + k]) / 255.0;
                    let mixed = (ink * alpha + below * under * (1.0 - alpha)) / out;
                    pixels[i + k] = (mixed * 255.0).round().clamp(0.0, 255.0) as u8;
                }
                pixels[i + 3] = (out * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

/// The picture as one page of a document, the size of the diagram, written by the report
/// engine's PDF exporter: shapes as vectors, words as text.
fn pdf(picture: &FlowPicture, title: &str, text: &mut TextSystem) -> Result<Vec<u8>, String> {
    let origin = picture.bounds.origin;
    let to_page = Point::new(-origin.x, -origin.y);
    let shapes: Vec<Shape> = picture
        .layers
        .iter()
        .map(|layer| match layer {
            Layer::Dashed { path, width, color, .. } => {
                Shape { path: path.clone(), fill: None, stroke: Some((*width, *color)) }.translated(to_page)
            }
            other => other.to_shape().translated(to_page),
        })
        .collect();
    let (w, h) = (picture.bounds.width(), picture.bounds.height());
    let mut items = vec![Item::Vector { rect: Rect::new(0.0, 0.0, w, h), shapes }];
    for t in &picture.texts {
        let Some(placed) = place(t, origin, text) else { continue };
        items.push(Item::Text(TextBlock {
            clip: Rect::new(placed.left, placed.top, placed.width + 2.0, placed.height),
            lines: vec![PlacedLine {
                text: placed.text,
                x: placed.left,
                top: placed.top,
                baseline: placed.baseline,
                width: placed.width,
            }],
            size: t.size,
            line_height: placed.height,
            bold: t.bold,
            color: t.color,
            align: TextAlign::Left,
            value: None,
            format: None,
        }));
    }
    let document = Document {
        title: title.to_owned(),
        culture: "en-US".to_owned(),
        pages: vec![Page { width: w, height: h, items, ..Page::default() }],
        errors: Vec::new(),
    };
    ravenrs_report_export::export(&document, ExportFormat::Pdf, None).map(|file| file.bytes)
}
