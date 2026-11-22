//! A flow diagram fitted into a box: the shapes of its picture moved and scaled, and where each
//! of its words goes. The report engine prints it as a control, and a diagram exported alone as
//! a PDF is laid out the same way.

use ravenrs_core::{Color, Point, Rect};
use ravenrs_graphics::flow::{FlowPicture, FlowText, dashes, flatten};
use ravenrs_graphics::vector::{Layer, PathSegment, Shape};

/// One run of words of a fitted diagram.
#[derive(Clone, Debug, PartialEq)]
pub struct DiagramLabel {
    /// The words.
    pub text: String,
    /// The box they are centred in, vertically, and start at or centred in, across.
    pub rect: Rect,
    /// Font size, in points.
    pub size: f32,
    /// Semibold.
    pub bold: bool,
    /// Centred across rather than from the left.
    pub centered: bool,
    /// Their color.
    pub color: Color,
}

/// A diagram fitted into a box.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FittedDiagram {
    /// The shapes, back to front; the picture's background is left out, so the box shows its own,
    /// and dashed lines come as one short line per dash.
    pub shapes: Vec<Shape>,
    /// The words, over the shapes.
    pub labels: Vec<DiagramLabel>,
}

/// Words smaller than this, in points, are left out: they could not be read.
pub const SMALLEST_WORDS: f32 = 2.0;

/// `picture` scaled to fit inside `rect` whole, keeping its proportions, and centred there.
pub fn fit(picture: &FlowPicture, rect: Rect) -> FittedDiagram {
    let b = picture.bounds;
    if b.width() <= 0.0 || b.height() <= 0.0 {
        return FittedDiagram::default();
    }
    let scale = (rect.width() / b.width()).min(rect.height() / b.height()).max(0.0);
    let (ox, oy) = (
        rect.x() + (rect.width() - b.width() * scale) / 2.0 - b.x() * scale,
        rect.y() + (rect.height() - b.height() * scale) / 2.0 - b.y() * scale,
    );
    let at = |p: Point| Point::new(ox + p.x * scale, oy + p.y * scale);
    let moved = |path: &[PathSegment]| -> Vec<PathSegment> {
        path.iter()
            .map(|s| match *s {
                PathSegment::Move(p) => PathSegment::Move(at(p)),
                PathSegment::Line(p) => PathSegment::Line(at(p)),
                PathSegment::Cubic(a, b, c) => PathSegment::Cubic(at(a), at(b), at(c)),
                other => other,
            })
            .collect()
    };
    let mut shapes = Vec::new();
    for layer in picture.layers.iter().skip(1) {
        match layer {
            // A dashed line is cut into its dashes, each a short line of its own, so it prints
            // dashed in every output, which know only whole shapes.
            Layer::Dashed { path, width, dash, color } => {
                let points: Vec<Point> = flatten(path).into_iter().map(at).collect();
                let step = (dash * scale).max(0.5);
                for piece in dashes(&points, step, step, 0.0) {
                    let line: Vec<PathSegment> = piece
                        .iter()
                        .enumerate()
                        .map(|(i, p)| if i == 0 { PathSegment::Move(*p) } else { PathSegment::Line(*p) })
                        .collect();
                    shapes.push(Shape {
                        path: line,
                        fill: None,
                        stroke: Some(((width * scale).max(0.25), *color)),
                    });
                }
            }
            other => {
                let shape = other.to_shape();
                shapes.push(Shape {
                    path: moved(&shape.path),
                    fill: shape.fill,
                    stroke: shape.stroke.map(|(w, c)| ((w * scale).max(0.25), c)),
                });
            }
        }
    }
    let labels = picture
        .texts
        .iter()
        .filter(|t| t.size * scale >= SMALLEST_WORDS)
        .map(|t: &FlowText| {
            let (size, width) = (t.size * scale, t.width * scale);
            let height = size * 1.4;
            let p = at(t.at);
            let left = if t.centered { p.x - width / 2.0 } else { p.x };
            DiagramLabel {
                text: t.text.clone(),
                rect: Rect::new(left, p.y - height / 2.0, width, height),
                size,
                bold: t.bold,
                centered: t.centered,
                color: t.color,
            }
        })
        .collect();
    FittedDiagram { shapes, labels }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ravenrs_graphics::flow::{FlowColors, FlowGraph, FlowNode};

    #[test]
    fn a_diagram_fits_its_box_keeping_its_proportions() {
        let g = FlowGraph::new()
            .node(FlowNode::new("a", "", "A", 0.0, 0.0))
            .node(FlowNode::new("b", "", "B", 600.0, 0.0));
        let picture = g.picture(None, FlowColors::paper()).expect("a picture");
        let rect = Rect::new(10.0, 20.0, 200.0, 200.0);
        let fitted = fit(&picture, rect);
        let points: Vec<Point> = fitted
            .shapes
            .iter()
            .flat_map(|s| s.path.iter())
            .filter_map(|s| match s {
                PathSegment::Move(p) | PathSegment::Line(p) => Some(*p),
                _ => None,
            })
            .collect();
        for p in &points {
            assert!(rect.contains(*p) || (p.x - rect.max_x()).abs() < 0.01, "{p:?} is inside");
        }
        // Wider than tall: the width fills the box, and the drawing is centred down it.
        let (least, most) = points.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
        assert!((least - rect.y()) > 10.0 && (rect.max_y() - most) > 10.0);
        assert_eq!(fitted.labels.len(), 2, "both titles");
    }

    #[test]
    fn a_dashed_line_comes_as_its_dashes() {
        use ravenrs_graphics::flow::{FlowEdge, FlowEndpoint, FlowPort};
        let g = FlowGraph::new()
            .node(FlowNode::new("a", "", "A", 0.0, 0.0).port(FlowPort::output("o")))
            .node(FlowNode::new("b", "", "B", 600.0, 0.0).port(FlowPort::input("i")))
            .edge(FlowEdge::new("e", FlowEndpoint::new("a", "o"), FlowEndpoint::new("b", "i")).dashed());
        let picture = g.picture(None, FlowColors::paper()).expect("a picture");
        let whole = fit(&picture, Rect::new(0.0, 0.0, 800.0, 200.0));
        let solid = g.clone();
        let mut solid = solid;
        solid.edges[0].dashed = false;
        let plain = fit(
            &solid.picture(None, FlowColors::paper()).expect("a picture"),
            Rect::new(0.0, 0.0, 800.0, 200.0),
        );
        assert!(whole.shapes.len() > plain.shapes.len() + 5, "many short dashes instead of one line");
        let open = whole.shapes.iter().filter(|s| s.fill.is_none() && s.path.len() >= 2).count();
        assert!(open > 5, "each dash a short open line");
    }
}
