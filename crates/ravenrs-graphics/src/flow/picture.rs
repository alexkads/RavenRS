//! A diagram as a picture to save: its shapes and its words, in units of the diagram, with a
//! margin, and SVG written from them. Pixels and PDF need fonts, so the crates that have them
//! (`ravenrs-flow-export`) draw the same picture.

use std::fmt::Write as _;

use ravenrs_core::{Color, Point, Rect};

use super::{
    EdgeCap, FlowGraph, FlowNode, FlowNodeStatus, cap_outline, end_direction, flatten, midpoint,
    shape_outline, trim_end, trim_start, union,
};
use crate::vector::{Layer, PathSegment, Shape};

/// The colors a diagram is drawn in, as the screen draws it or on paper.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlowColors {
    /// Behind everything.
    pub background: Color,
    /// Inside the nodes.
    pub node: Color,
    /// Around the nodes.
    pub border: Color,
    /// The titles.
    pub ink: Color,
    /// The lines under titles, and group titles.
    pub muted: Color,
    /// The connections.
    pub line: Color,
    /// A node that failed.
    pub danger: Color,
    /// A node that warns.
    pub warning: Color,
}

impl FlowColors {
    /// Dark ink on white, for paper.
    pub const fn paper() -> Self {
        Self {
            background: Color::WHITE,
            node: Color::rgba(0.97, 0.97, 0.98, 1.0),
            border: Color::rgba(0.78, 0.8, 0.84, 1.0),
            ink: Color::rgba(0.1, 0.11, 0.14, 1.0),
            muted: Color::rgba(0.4, 0.43, 0.5, 1.0),
            line: Color::rgba(0.3, 0.4, 0.93, 1.0),
            danger: Color::rgba(0.86, 0.2, 0.2, 1.0),
            warning: Color::rgba(0.85, 0.55, 0.05, 1.0),
        }
    }
}

/// A file a diagram is saved as.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FlowExportFormat {
    /// A picture, at twice the size of the diagram, for screens and documents.
    #[default]
    Png,
    /// A vector drawing that stays sharp at any size.
    Svg,
    /// A page to print, on white paper.
    Pdf,
}

impl FlowExportFormat {
    /// The file name's extension, without the dot.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Svg => "svg",
            Self::Pdf => "pdf",
        }
    }

    /// The media type.
    pub fn mime(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Svg => "image/svg+xml",
            Self::Pdf => "application/pdf",
        }
    }
}

/// One run of words of a [`FlowPicture`], on one line.
#[derive(Clone, Debug, PartialEq)]
pub struct FlowText {
    /// The words.
    pub text: String,
    /// The middle of the line's left end, or of the whole line when `centered`.
    pub at: Point,
    /// The width the words may take; longer ones are cut with an ellipsis by who draws them.
    pub width: f32,
    /// Font size, in units of the diagram.
    pub size: f32,
    /// Semibold rather than regular.
    pub bold: bool,
    /// Centred on `at` rather than starting there.
    pub centered: bool,
    /// Its color.
    pub color: Color,
}

/// A diagram, or part of it, as shapes and words, ready to save.
#[derive(Clone, Debug, PartialEq)]
pub struct FlowPicture {
    /// What it covers, margin included, in units of the diagram.
    pub bounds: Rect,
    /// The background first, then groups, lines and nodes, in the order they are drawn.
    pub layers: Vec<Layer>,
    /// The words, drawn over the layers.
    pub texts: Vec<FlowText>,
}

/// The margin around a picture, in units of the diagram.
pub const PICTURE_MARGIN: f32 = 24.0;

fn shape(path: Vec<PathSegment>, fill: Option<Color>, stroke: Option<(f32, Color)>) -> Layer {
    Layer::Shape(Shape { path, fill, stroke })
}

fn polyline(points: &[Point]) -> Vec<PathSegment> {
    let mut path = Vec::with_capacity(points.len());
    for (i, &p) in points.iter().enumerate() {
        path.push(if i == 0 { PathSegment::Move(p) } else { PathSegment::Line(p) });
    }
    path
}

/// `path` turned by `degrees` around `center`.
fn turned(path: Vec<PathSegment>, center: Point, degrees: f32) -> Vec<PathSegment> {
    if degrees == 0.0 {
        return path;
    }
    let (sin, cos) = degrees.to_radians().sin_cos();
    let turn = |p: Point| {
        let (dx, dy) = (p.x - center.x, p.y - center.y);
        Point::new(center.x + dx * cos - dy * sin, center.y + dx * sin + dy * cos)
    };
    path.into_iter()
        .map(|s| match s {
            PathSegment::Move(p) => PathSegment::Move(turn(p)),
            PathSegment::Line(p) => PathSegment::Line(turn(p)),
            PathSegment::Cubic(a, b, c) => PathSegment::Cubic(turn(a), turn(b), turn(c)),
            other => other,
        })
        .collect()
}

impl FlowGraph {
    /// The diagram as a picture in `colors`: every node and connection, or, with `only`, those
    /// nodes and the connections between them. `None` when there is nothing to draw.
    pub fn picture(&self, only: Option<&[String]>, colors: FlowColors) -> Option<FlowPicture> {
        let keep = |id: &str| only.is_none_or(|ids| ids.iter().any(|i| i == id));
        let nodes: Vec<&FlowNode> = self.nodes.iter().filter(|n| keep(&n.id)).collect();
        let first = nodes.first()?.extent();
        let mut bounds = nodes.iter().fold(first, |all, n| union(all, n.extent()));
        let edges: Vec<_> = self.edges.iter().filter(|e| keep(&e.from.node) && keep(&e.to.node)).collect();
        let lines: Vec<(Vec<Point>, &super::FlowEdge)> =
            edges.iter().filter_map(|e| Some((flatten(&self.edge_path(e)?), *e))).collect();
        for (points, _) in &lines {
            for p in points {
                bounds = union(bounds, Rect::new(p.x, p.y, 0.0, 0.0));
            }
        }
        let bounds = Rect::new(
            bounds.x() - PICTURE_MARGIN,
            bounds.y() - PICTURE_MARGIN,
            bounds.width() + PICTURE_MARGIN * 2.0,
            bounds.height() + PICTURE_MARGIN * 2.0,
        );
        let mut layers = vec![shape(crate::vector::rectangle(bounds, 0.0), Some(colors.background), None)];
        let mut texts = Vec::new();

        // Groups behind, then the lines, then the nodes.
        for group in nodes.iter().filter(|n| n.group) {
            let r = Rect::new(group.x, group.y, group.width, group.height);
            layers.push(Layer::Dashed {
                path: crate::vector::rectangle(r, 12.0),
                width: 1.5,
                dash: 6.0,
                color: colors.border,
            });
            texts.push(FlowText {
                text: group.title.clone(),
                at: Point::new(r.x() + 16.0, r.y() + 20.0),
                width: r.width() - 32.0,
                size: 13.0,
                bold: true,
                centered: false,
                color: colors.muted,
            });
        }
        for (points, edge) in &lines {
            let mut body = points.clone();
            let cap = 8.0;
            if edge.end != EdgeCap::None {
                body = trim_end(&body, cap);
            }
            if edge.start != EdgeCap::None {
                body = trim_start(&body, cap);
            }
            layers.push(shape(polyline(&body), None, Some((2.0, colors.line))));
            if let (Some(&tip), Some(direction)) = (points.last(), end_direction(points)) {
                let outline = cap_outline(edge.end, tip, direction, cap);
                if !outline.is_empty() {
                    layers.push(shape(outline, Some(colors.line), None));
                }
            }
            let back: Vec<Point> = points.iter().rev().copied().collect();
            if let (Some(&tip), Some(direction)) = (back.last(), end_direction(&back)) {
                let outline = cap_outline(edge.start, tip, direction, cap);
                if !outline.is_empty() {
                    layers.push(shape(outline, Some(colors.line), None));
                }
            }
            if let (Some(label), Some(middle)) = (&edge.label, midpoint(points)) {
                let width = label.chars().count() as f32 * 7.0 + 16.0;
                let pill = Rect::new(middle.x - width / 2.0, middle.y - 11.0, width, 22.0);
                layers.push(shape(
                    crate::vector::rectangle(pill, 11.0),
                    Some(colors.node),
                    Some((1.0, colors.border)),
                ));
                texts.push(FlowText {
                    text: label.clone(),
                    at: middle,
                    width,
                    size: 12.0,
                    bold: false,
                    centered: true,
                    color: colors.ink,
                });
            }
        }
        for node in nodes.iter().filter(|n| !n.group) {
            let r = Rect::new(node.x, node.y, node.width, node.height);
            let edge = match node.status {
                FlowNodeStatus::Error(_) => colors.danger,
                FlowNodeStatus::Warning(_) => colors.warning,
                _ => colors.border,
            };
            let outline = turned(shape_outline(node.shape, r), r.center(), node.rotation);
            layers.push(shape(outline, Some(colors.node), Some((1.5, edge))));
            let centered = node.shape != super::FlowShape::Card;
            let (x, width) =
                if centered { (r.center().x, r.width() - 24.0) } else { (r.x() + 16.0, r.width() - 32.0) };
            let middle = r.center().y;
            let title_y = if node.subtitle.is_some() { middle - 9.0 } else { middle };
            texts.push(FlowText {
                text: node.title.clone(),
                at: Point::new(x, title_y),
                width,
                size: 14.0,
                bold: true,
                centered,
                color: colors.ink,
            });
            if let Some(line) = &node.subtitle {
                texts.push(FlowText {
                    text: line.clone(),
                    at: Point::new(x, middle + 10.0),
                    width,
                    size: 12.0,
                    bold: false,
                    centered,
                    color: colors.muted,
                });
            }
        }
        Some(FlowPicture { bounds, layers, texts })
    }
}

fn hex(color: Color) -> String {
    let [r, g, b, _] = color.to_array();
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", byte(r), byte(g), byte(b))
}

fn alpha(color: Color) -> f32 {
    color.to_array()[3]
}

fn path_data(path: &[PathSegment], origin: Point) -> String {
    let mut d = String::new();
    let p = |q: Point| (q.x - origin.x, q.y - origin.y);
    for segment in path {
        let _ = match *segment {
            PathSegment::Move(q) => write!(d, "M{:.2} {:.2}", p(q).0, p(q).1),
            PathSegment::Line(q) => write!(d, "L{:.2} {:.2}", p(q).0, p(q).1),
            PathSegment::Cubic(a, b, c) => write!(
                d,
                "C{:.2} {:.2} {:.2} {:.2} {:.2} {:.2}",
                p(a).0,
                p(a).1,
                p(b).0,
                p(b).1,
                p(c).0,
                p(c).1
            ),
            PathSegment::Close => write!(d, "Z"),
            #[allow(unreachable_patterns, reason = "the enum may grow")]
            _ => Ok(()),
        };
    }
    d
}

fn escaped(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

impl FlowPicture {
    /// The picture as an SVG document, at one pixel per unit of the diagram, its words in the
    /// family named.
    pub fn to_svg(&self, family: &str) -> String {
        let (w, h, o) = (self.bounds.width(), self.bounds.height(), self.bounds.origin);
        let mut out = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.0}\" height=\"{h:.0}\" viewBox=\"0 0 {w:.0} {h:.0}\">\n"
        );
        for layer in &self.layers {
            write_layer(&mut out, layer, o);
        }
        for t in &self.texts {
            let (x, y) = (t.at.x - o.x, t.at.y - o.y);
            let _ = writeln!(
                out,
                "<text x=\"{x:.2}\" y=\"{y:.2}\" font-family=\"{}\" font-size=\"{}\" font-weight=\"{}\" fill=\"{}\" dominant-baseline=\"middle\"{}>{}</text>",
                escaped(family),
                t.size,
                if t.bold { 600 } else { 400 },
                hex(t.color),
                if t.centered { " text-anchor=\"middle\"" } else { "" },
                escaped(&t.text)
            );
        }
        out.push_str("</svg>\n");
        out
    }
}

fn write_layer(out: &mut String, layer: &Layer, o: Point) {
    match layer {
        Layer::Shape(s) => {
            let fill = s.fill.map_or("none".to_owned(), hex);
            let fill_alpha = s.fill.map_or(1.0, alpha);
            let stroke = s.stroke.map_or(String::new(), |(width, color)| {
                format!(" stroke=\"{}\" stroke-width=\"{width}\" stroke-linejoin=\"round\" stroke-linecap=\"round\"", hex(color))
            });
            let _ = writeln!(
                out,
                "<path d=\"{}\" fill=\"{fill}\" fill-opacity=\"{fill_alpha:.3}\"{stroke}/>",
                path_data(&s.path, o)
            );
        }
        Layer::Dashed { path, width, dash, color } => {
            let _ = writeln!(
                out,
                "<path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{width}\" stroke-dasharray=\"{dash} {dash}\"/>",
                path_data(path, o),
                hex(*color)
            );
        }
        Layer::Gradient { path, top, .. } => {
            let _ = writeln!(out, "<path d=\"{}\" fill=\"{}\"/>", path_data(path, o), hex(top.1));
        }
        Layer::Clipped { layer, .. } => write_layer(out, layer, o),
        #[allow(unreachable_patterns, reason = "the enum may grow")]
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::{FlowEdge, FlowEndpoint, FlowPort};

    fn sample() -> FlowGraph {
        FlowGraph::new()
            .node(FlowNode::new("a", "", "Start <here>", 0.0, 0.0).port(FlowPort::output("out")))
            .node(FlowNode::new("b", "", "End", 400.0, 0.0).subtitle("done").port(FlowPort::input("in")))
            .node(FlowNode::new("c", "", "Apart", 0.0, 300.0))
            .edge(
                FlowEdge::new("e", FlowEndpoint::new("a", "out"), FlowEndpoint::new("b", "in")).label("Yes"),
            )
    }

    #[test]
    fn a_picture_holds_every_node_with_a_margin() {
        let g = sample();
        let all = g.picture(None, FlowColors::paper()).expect("a picture");
        let bounds = g.bounds().expect("nodes");
        assert_eq!(all.bounds.x(), bounds.x() - PICTURE_MARGIN);
        assert_eq!(all.bounds.width(), bounds.width() + PICTURE_MARGIN * 2.0);
        let words: Vec<&str> = all.texts.iter().map(|t| t.text.as_str()).collect();
        assert!(words.contains(&"Yes") && words.contains(&"done") && words.contains(&"Apart"));
    }

    #[test]
    fn only_the_selection_leaves_the_rest_out() {
        let g = sample();
        let some = g.picture(Some(&["a".into(), "b".into()]), FlowColors::paper()).expect("a picture");
        assert!(some.texts.iter().all(|t| t.text != "Apart"));
        assert!(some.texts.iter().any(|t| t.text == "Yes"), "the line between them stays");
        let one = g.picture(Some(&["a".into()]), FlowColors::paper()).expect("a picture");
        assert!(one.texts.iter().all(|t| t.text != "Yes"), "a line with one end out goes");
        assert!(g.picture(Some(&[]), FlowColors::paper()).is_none());
    }

    #[test]
    fn svg_escapes_the_words_and_sizes_to_the_picture() {
        let picture = sample().picture(None, FlowColors::paper()).expect("a picture");
        let svg = picture.to_svg("Inter");
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.contains("Start &lt;here&gt;"));
        assert!(svg.contains(&format!("width=\"{:.0}\"", picture.bounds.width())));
        assert!(svg.trim_end().ends_with("</svg>"));
    }
}
