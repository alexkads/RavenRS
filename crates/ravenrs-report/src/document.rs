//! The rendered result: pages of positioned primitives, in points. The viewer
//! and every exporter draw this same structure, so they cannot disagree.

use std::fmt::Write as _;

use ravenrs_core::{Color, Point, Rect};

/// A paginated document.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    /// The title (the report's name).
    pub title: String,
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
                    for l in &t.lines {
                        let _ = write!(
                            s,
                            r#"<text x="{}" y="{}" font-family="Inter, sans-serif" font-size="{}" font-weight="{}" fill="{}">{}</text>"#,
                            l.x,
                            l.baseline,
                            t.size,
                            if t.bold { 600 } else { 400 },
                            css(t.color),
                            escape(&l.text)
                        );
                    }
                }
            }
        }
        s.push_str("</svg>");
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
