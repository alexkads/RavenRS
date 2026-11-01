//! The rendered result: pages of positioned primitives, in points. The viewer
//! and every exporter draw this same structure, so they cannot disagree.

use std::fmt::Write as _;

use std::ops::Range;
use std::sync::Arc;

use ravenrs_core::{Color, ImageData, Point, Rect};
use ravenrs_criteria::Value;

use crate::base64::encode;
use crate::model::TextAlign;

/// A paginated document.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    /// The title (the report's name).
    pub title: String,
    /// The culture the document was formatted with (`pt-BR`, `en-US`).
    pub culture: String,
    /// The pages, in order.
    pub pages: Vec<Page>,
    /// Problems found while rendering (expression errors, missing data), with
    /// where they happened. The document is still produced.
    pub errors: Vec<String>,
}

/// One page.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Page {
    /// Paper width in points.
    pub width: f32,
    /// Paper height in points.
    pub height: f32,
    /// What is printed, back to front.
    pub items: Vec<Item>,
    /// The part of `items` that the page header printed. Exporters that flow
    /// pages together keep it only once.
    pub page_header: Range<usize>,
    /// The part of `items` that the page footer printed.
    pub page_footer: Range<usize>,
}

/// A printed primitive.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Item {
    /// A filled and/or outlined rectangle.
    Rect {
        /// Its box.
        rect: Rect,
        /// Fill color.
        fill: Option<Color>,
        /// Outline width and color.
        stroke: Option<(f32, Color)>,
        /// Corner radius.
        radius: f32,
    },
    /// A filled and/or outlined ellipse inscribed in `rect`.
    Ellipse {
        /// The bounding box.
        rect: Rect,
        /// Fill color.
        fill: Option<Color>,
        /// Outline width and color.
        stroke: Option<(f32, Color)>,
    },
    /// A straight line.
    Line {
        /// Start point.
        from: Point,
        /// End point.
        to: Point,
        /// Thickness.
        width: f32,
        /// Color.
        color: Color,
    },
    /// Lines of text in one font, clipped to a box.
    Text(TextBlock),
    /// An image.
    Image {
        /// Where the whole image is drawn; it can be larger than `clip`.
        rect: Rect,
        /// The control's box; the image is cut there.
        clip: Rect,
        /// The image.
        image: DocumentImage,
    },
    /// Vector shapes that belong together, such as a chart or a barcode. PDF and SVG write
    /// them as they are; the screen and spreadsheets get them as one picture.
    Vector {
        /// The box they are in.
        rect: Rect,
        /// The shapes, back to front, in the page's coordinates.
        shapes: Vec<crate::vector::Shape>,
    },
    /// An area that opens an address when clicked. Prints nothing.
    Link {
        /// The clickable area.
        rect: Rect,
        /// The address (`https:`, `http:` or `mailto:`).
        url: String,
    },
    /// An entry in the document's outline. Prints nothing.
    Bookmark {
        /// Its text.
        title: String,
        /// How deep it is: 0 at the top.
        level: u32,
        /// Where on the page it leads.
        y: f32,
    },
}

/// The file format of a [`DocumentImage`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ImageFormat {
    /// PNG.
    Png,
    /// JPEG.
    Jpeg,
}

/// A decoded image, with the file it came from: exporters can embed the file
/// itself, which keeps JPEG photos small and unchanged. Clones share the data.
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentImage {
    pixels: ImageData,
    encoded: Arc<[u8]>,
    format: ImageFormat,
    channels: u8,
}

impl DocumentImage {
    pub(crate) fn new(pixels: ImageData, encoded: Arc<[u8]>, format: ImageFormat, channels: u8) -> Self {
        Self { pixels, encoded, format, channels }
    }

    /// The decoded pixels, RGBA8.
    pub fn pixels(&self) -> &ImageData {
        &self.pixels
    }

    /// The original file.
    pub fn encoded(&self) -> &[u8] {
        &self.encoded
    }

    /// The original file's format.
    pub fn format(&self) -> ImageFormat {
        self.format
    }

    /// How many color channels the original file has: 1 gray, 3 color; 2 or 4 with transparency.
    pub fn channels(&self) -> u8 {
        self.channels
    }
}

/// Text already broken into lines and positioned.
#[derive(Clone, Debug, PartialEq)]
pub struct TextBlock {
    /// The control's box; text outside it is clipped.
    pub clip: Rect,
    /// The lines.
    pub lines: Vec<PlacedLine>,
    /// Font size in points.
    pub size: f32,
    /// Line height in points.
    pub line_height: f32,
    /// Bold weight.
    pub bold: bool,
    /// Text color.
    pub color: Color,
    /// How the text is aligned in its box.
    pub align: TextAlign,
    /// The value behind the text, when it is one number, date or truth value,
    /// so data exports can write the value and not its formatted text.
    pub value: Option<Value>,
    /// The format specifier that turned `value` into the text (`C2`, `dd/MM/yyyy`).
    pub format: Option<String>,
}

/// A line of text at its position on the page.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedLine {
    /// The text of the line, without the line break.
    pub text: String,
    /// Left edge.
    pub x: f32,
    /// Top of the line box.
    pub top: f32,
    /// Baseline.
    pub baseline: f32,
    /// Width of the text.
    pub width: f32,
}

impl Document {
    /// All the text of a page in reading order, one line per text line (for
    /// tests, search and accessibility).
    pub fn page_text(&self, page: usize) -> Vec<String> {
        let Some(p) = self.pages.get(page) else { return Vec::new() };
        let mut lines: Vec<(f32, f32, String)> = p
            .items
            .iter()
            .filter_map(|i| if let Item::Text(t) = i { Some(t) } else { None })
            .flat_map(|t| t.lines.iter().map(|l| (l.baseline, l.x, l.text.clone())))
            .collect();
        lines.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        lines.into_iter().map(|(_, _, s)| s).collect()
    }

    /// Whether `needle` appears on the page (case-insensitive).
    pub fn page_contains(&self, page: usize, needle: &str) -> bool {
        let needle = needle.to_lowercase();
        self.page_text(page).iter().any(|l| l.to_lowercase().contains(&needle))
    }

    /// A page as SVG, for previews, tests and debugging. Text uses the Inter
    /// font the document was measured with.
    pub fn page_svg(&self, page: usize) -> String {
        let Some(p) = self.pages.get(page) else { return String::new() };
        let mut s = String::new();
        let _ = write!(
            s,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}pt" height="{h}pt" viewBox="0 0 {w} {h}"><rect width="{w}" height="{h}" fill="#fff"/>"##,
            w = p.width,
            h = p.height
        );
        for item in &p.items {
            match item {
                Item::Rect { rect, fill, stroke, radius } => {
                    let _ = write!(
                        s,
                        r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{radius}" fill="{}"{}/>"#,
                        rect.x(),
                        rect.y(),
                        rect.width(),
                        rect.height(),
                        fill.map_or("none".into(), css),
                        stroke_attr(*stroke)
                    );
                }
                Item::Ellipse { rect, fill, stroke } => {
                    let c = rect.center();
                    let _ = write!(
                        s,
                        r#"<ellipse cx="{}" cy="{}" rx="{}" ry="{}" fill="{}"{}/>"#,
                        c.x,
                        c.y,
                        rect.width() / 2.0,
                        rect.height() / 2.0,
                        fill.map_or("none".into(), css),
                        stroke_attr(*stroke)
                    );
                }
                Item::Line { from, to, width, color } => {
                    let _ = write!(
                        s,
                        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{width}"/>"#,
                        from.x,
                        from.y,
                        to.x,
                        to.y,
                        css(*color)
                    );
                }
                Item::Text(t) => {
                    // Text wider than its box is cut there, as on screen.
                    let overflows = t.lines.iter().any(|l| l.x + l.width > t.clip.max_x() + 0.5);
                    if overflows {
                        let c = t.clip;
                        let _ = write!(
                            s,
                            r#"<svg x="{x}" y="{y}" width="{w}" height="{h}" viewBox="{x} {y} {w} {h}">"#,
                            x = c.x(),
                            y = c.y(),
                            w = c.width(),
                            h = c.height()
                        );
                    }
                    for l in &t.lines {
                        // textLength keeps the measured width even if another font is substituted.
                        let _ = write!(
                            s,
                            r#"<text x="{}" y="{}" textLength="{}" lengthAdjust="spacingAndGlyphs" xml:space="preserve" font-family="Inter, sans-serif" font-size="{}" font-weight="{}" fill="{}">{}</text>"#,
                            l.x,
                            l.baseline,
                            l.width.max(0.01),
                            t.size,
                            if t.bold { 600 } else { 400 },
                            css(t.color),
                            escape(l.text.trim_end())
                        );
                    }
                    if overflows {
                        s.push_str("</svg>");
                    }
                }
                Item::Image { rect, clip, image } => {
                    let mime = match image.format() {
                        ImageFormat::Png => "image/png",
                        ImageFormat::Jpeg => "image/jpeg",
                    };
                    // A nested viewport cuts the image at its box.
                    let _ = write!(
                        s,
                        r#"<svg x="{x}" y="{y}" width="{w}" height="{h}" viewBox="{x} {y} {w} {h}"><image x="{}" y="{}" width="{}" height="{}" preserveAspectRatio="none" href="data:{mime};base64,{}"/></svg>"#,
                        rect.x(),
                        rect.y(),
                        rect.width(),
                        rect.height(),
                        encode(image.encoded()),
                        x = clip.x(),
                        y = clip.y(),
                        w = clip.width(),
                        h = clip.height(),
                    );
                }
                Item::Link { rect, url } => {
                    let _ = write!(
                        s,
                        r#"<a href="{}"><rect x="{}" y="{}" width="{}" height="{}" fill="transparent"/></a>"#,
                        escape(url).replace('"', "&quot;"),
                        rect.x(),
                        rect.y(),
                        rect.width(),
                        rect.height(),
                    );
                }
                Item::Vector { shapes, .. } => {
                    for shape in shapes {
                        let mut d = String::new();
                        for segment in &shape.path {
                            match segment {
                                crate::vector::PathSegment::Move(p) => {
                                    let _ = write!(d, "M{} {}", p.x, p.y);
                                }
                                crate::vector::PathSegment::Line(p) => {
                                    let _ = write!(d, "L{} {}", p.x, p.y);
                                }
                                crate::vector::PathSegment::Cubic(a, b, c) => {
                                    let _ = write!(d, "C{} {} {} {} {} {}", a.x, a.y, b.x, b.y, c.x, c.y);
                                }
                                crate::vector::PathSegment::Close => d.push('Z'),
                                _ => {}
                            }
                        }
                        let _ = write!(
                            s,
                            r#"<path d="{d}" fill="{}"{}/>"#,
                            shape.fill.map_or("none".into(), css),
                            stroke_attr(shape.stroke)
                        );
                    }
                }
                Item::Bookmark { .. } => {}
            }
        }
        s.push_str("</svg>");
        s
    }

    /// The whole document as a standalone HTML page sized for printing: one
    /// sheet per page, with the Inter font embedded so it prints as measured.
    /// Open it in a browser and print, or save it as PDF from the print dialog.
    pub fn print_html(&self) -> String {
        let (w, h) = self.pages.first().map_or((595.28, 841.89), |p| (p.width, p.height));
        let mut s = String::new();
        let _ = write!(
            s,
            "<!doctype html><html><head><meta charset=\"utf-8\"><title>{}</title><style>\
             @font-face{{font-family:Inter;font-weight:400;src:url(data:font/ttf;base64,{})}}\
             @font-face{{font-family:Inter;font-weight:600;src:url(data:font/ttf;base64,{})}}\
             @page{{size:{w}pt {h}pt;margin:0}}\
             html,body{{margin:0;padding:0;background:#fff}}\
             svg{{display:block}}\
             .sheet{{break-after:page;overflow:hidden}}\
             .sheet:last-child{{break-after:auto}}\
             </style></head><body>",
            escape(&self.title),
            encode(ravenrs_text::inter_font(false)),
            encode(ravenrs_text::inter_font(true)),
        );
        for page in 0..self.pages.len() {
            s.push_str("<div class=\"sheet\">");
            s.push_str(&self.page_svg(page));
            s.push_str("</div>");
        }
        s.push_str("</body></html>");
        s
    }
}

fn stroke_attr(stroke: Option<(f32, Color)>) -> String {
    stroke.map_or(String::new(), |(w, c)| format!(r#" stroke="{}" stroke-width="{w}""#, css(c)))
}

/// A color as CSS `rgba(...)`.
fn css(c: Color) -> String {
    format!(
        "rgba({},{},{},{})",
        (c.r * 255.0).round() as u8,
        (c.g * 255.0).round() as u8,
        (c.b * 255.0).round() as u8,
        c.a
    )
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Parses `#rgb`, `#rrggbb` or `#rrggbbaa`.
pub(crate) fn parse_color(s: &str) -> Option<Color> {
    let hex = s.trim().strip_prefix('#')?;
    let digit = |i: usize, len: usize| u8::from_str_radix(hex.get(i..i + len)?, 16).ok();
    let (r, g, b, a) = match hex.len() {
        3 => (digit(0, 1)? * 17, digit(1, 1)? * 17, digit(2, 1)? * 17, 255),
        6 => (digit(0, 2)?, digit(2, 2)?, digit(4, 2)?, 255),
        8 => (digit(0, 2)?, digit(2, 2)?, digit(4, 2)?, digit(6, 2)?),
        _ => return None,
    };
    Some(Color::rgba(f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, f32::from(a) / 255.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_parse() {
        assert_eq!(parse_color("#ff0000"), Some(Color::rgba(1.0, 0.0, 0.0, 1.0)));
        assert_eq!(parse_color("#0f0"), Some(Color::rgba(0.0, 1.0, 0.0, 1.0)));
        assert_eq!(parse_color("#00000080").map(|c| (c.a * 255.0).round()), Some(128.0));
        assert_eq!(parse_color("red"), None);
    }
}
