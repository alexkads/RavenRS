//! Vector shapes: curves and polygons that PDF and SVG write as they are, and
//! that the screen and spreadsheets get as a picture.

use ravenrs_core::{Color, ImageData, Point, Rect};

/// One step of a shape's outline, in the page's coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum PathSegment {
    /// Starts an outline here.
    Move(Point),
    /// A straight line to this point.
    Line(Point),
    /// A curve with two control points, to the last one.
    Cubic(Point, Point, Point),
    /// Back to where the outline started.
    Close,
}

/// A filled and/or outlined shape.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    /// Its outline.
    pub path: Vec<PathSegment>,
    /// Fill color.
    pub fill: Option<Color>,
    /// Outline width and color.
    pub stroke: Option<(f32, Color)>,
}

/// A quarter circle as a Bézier curve: the handle's length for radius 1.
const KAPPA: f32 = 0.552_284_8;

impl Shape {
    /// The same shape moved by an offset.
    pub fn translated(&self, by: Point) -> Self {
        let moved = |p: &Point| Point::new(p.x + by.x, p.y + by.y);
        let path = self
            .path
            .iter()
            .map(|segment| match segment {
                PathSegment::Move(p) => PathSegment::Move(moved(p)),
                PathSegment::Line(p) => PathSegment::Line(moved(p)),
                PathSegment::Cubic(a, b, c) => PathSegment::Cubic(moved(a), moved(b), moved(c)),
                PathSegment::Close => PathSegment::Close,
            })
            .collect();
        Self { path, fill: self.fill, stroke: self.stroke }
    }

    /// A shape filled with `fill`, without an outline.
    pub fn filled(path: Vec<PathSegment>, fill: Color) -> Self {
        Self { path, fill: Some(fill), stroke: None }
    }
}

/// The outline of a rectangle, with rounded corners when `radius` is above zero.
pub fn rectangle(rect: Rect, radius: f32) -> Vec<PathSegment> {
    let (x0, x1, y0, y1) = (rect.x(), rect.max_x(), rect.y(), rect.max_y());
    let mut path = Vec::with_capacity(5);
    append_rectangle(&mut path, x0, y0, x1, y1, radius);
    path
}

/// Appends a rectangle to an outline, for many rectangles in one shape.
pub fn append_rectangle(path: &mut Vec<PathSegment>, x0: f32, y0: f32, x1: f32, y1: f32, radius: f32) {
    let r = radius.clamp(0.0, (x1 - x0).min(y1 - y0) / 2.0);
    let at = Point::new;
    if r <= 0.0 {
        path.extend([
            PathSegment::Move(at(x0, y0)),
            PathSegment::Line(at(x1, y0)),
            PathSegment::Line(at(x1, y1)),
            PathSegment::Line(at(x0, y1)),
            PathSegment::Close,
        ]);
        return;
    }
    let k = r * (1.0 - KAPPA);
    path.extend([
        PathSegment::Move(at(x0 + r, y0)),
        PathSegment::Line(at(x1 - r, y0)),
        PathSegment::Cubic(at(x1 - k, y0), at(x1, y0 + k), at(x1, y0 + r)),
        PathSegment::Line(at(x1, y1 - r)),
        PathSegment::Cubic(at(x1, y1 - k), at(x1 - k, y1), at(x1 - r, y1)),
        PathSegment::Line(at(x0 + r, y1)),
        PathSegment::Cubic(at(x0 + k, y1), at(x0, y1 - k), at(x0, y1 - r)),
        PathSegment::Line(at(x0, y0 + r)),
        PathSegment::Cubic(at(x0, y0 + k), at(x0 + k, y0), at(x0 + r, y0)),
        PathSegment::Close,
    ]);
}

/// The outline of an ellipse inscribed in `rect`.
pub fn ellipse(rect: Rect) -> Vec<PathSegment> {
    let center = rect.center();
    let (cx, cy) = (center.x, center.y);
    let (rx, ry) = (rect.width() / 2.0, rect.height() / 2.0);
    let (kx, ky) = (rx * KAPPA, ry * KAPPA);
    let at = Point::new;
    vec![
        PathSegment::Move(at(cx + rx, cy)),
        PathSegment::Cubic(at(cx + rx, cy + ky), at(cx + kx, cy + ry), at(cx, cy + ry)),
        PathSegment::Cubic(at(cx - kx, cy + ry), at(cx - rx, cy + ky), at(cx - rx, cy)),
        PathSegment::Cubic(at(cx - rx, cy - ky), at(cx - kx, cy - ry), at(cx, cy - ry)),
        PathSegment::Cubic(at(cx + kx, cy - ry), at(cx + rx, cy - ky), at(cx + rx, cy)),
        PathSegment::Close,
    ]
}

/// The most pixels a side of a rasterized drawing can have.
const MAX_SIDE: f32 = 4096.0;

/// Draws shapes into a picture of `rect` at `scale` pixels per point, smoothed.
pub fn rasterize(shapes: &[Shape], rect: Rect, scale: f32) -> Option<ImageData> {
    let scale = scale.min(MAX_SIDE / rect.width().max(1.0)).min(MAX_SIDE / rect.height().max(1.0));
    let width = (rect.width() * scale).ceil().max(1.0) as u32;
    let height = (rect.height() * scale).ceil().max(1.0) as u32;
    let mut pixmap = tiny_skia::Pixmap::new(width, height)?;
    let transform = tiny_skia::Transform::from_translate(-rect.x(), -rect.y()).post_scale(scale, scale);
    for shape in shapes {
        let Some(path) = build_path(&shape.path) else { continue };
        if let Some(fill) = shape.fill {
            let paint = paint_of(fill);
            pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, transform, None);
        }
        if let Some((width, color)) = shape.stroke {
            let paint = paint_of(color);
            let stroke = tiny_skia::Stroke {
                width,
                line_join: tiny_skia::LineJoin::Round,
                line_cap: tiny_skia::LineCap::Round,
                ..tiny_skia::Stroke::default()
            };
            pixmap.stroke_path(&path, &paint, &stroke, transform, None);
        }
    }
    // The pixmap is premultiplied; images hold straight alpha.
    let mut pixels = pixmap.take();
    for pixel in pixels.as_chunks_mut::<4>().0 {
        let alpha = pixel[3];
        if alpha != 0 && alpha != 255 {
            for channel in &mut pixel[..3] {
                *channel = (u16::from(*channel) * 255 / u16::from(alpha)).min(255) as u8;
            }
        }
    }
    ImageData::from_rgba8(width, height, pixels)
}

fn paint_of(color: Color) -> tiny_skia::Paint<'static> {
    let mut paint = tiny_skia::Paint { anti_alias: true, ..tiny_skia::Paint::default() };
    paint.set_color_rgba8(
        (color.r.clamp(0.0, 1.0) * 255.0).round() as u8,
        (color.g.clamp(0.0, 1.0) * 255.0).round() as u8,
        (color.b.clamp(0.0, 1.0) * 255.0).round() as u8,
        (color.a.clamp(0.0, 1.0) * 255.0).round() as u8,
    );
    paint
}

fn build_path(segments: &[PathSegment]) -> Option<tiny_skia::Path> {
    let mut builder = tiny_skia::PathBuilder::new();
    for segment in segments {
        match segment {
            PathSegment::Move(p) => builder.move_to(p.x, p.y),
            PathSegment::Line(p) => builder.line_to(p.x, p.y),
            PathSegment::Cubic(a, b, c) => builder.cubic_to(a.x, a.y, b.x, b.y, c.x, c.y),
            PathSegment::Close => builder.close(),
        }
    }
    builder.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Color = Color::rgba(1.0, 0.0, 0.0, 1.0);

    fn square() -> Shape {
        Shape {
            path: vec![
                PathSegment::Move(Point::new(10.0, 10.0)),
                PathSegment::Line(Point::new(20.0, 10.0)),
                PathSegment::Line(Point::new(20.0, 20.0)),
                PathSegment::Line(Point::new(10.0, 20.0)),
                PathSegment::Close,
            ],
            fill: Some(RED),
            stroke: None,
        }
    }

    fn pixel(image: &ImageData, x: u32, y: u32) -> [u8; 4] {
        let at = ((y * image.width() + x) * 4) as usize;
        let p = &image.pixels()[at..at + 4];
        [p[0], p[1], p[2], p[3]]
    }

    #[test]
    fn shapes_land_where_the_page_says_at_the_scale() {
        let image = rasterize(&[square()], Rect::new(10.0, 10.0, 20.0, 20.0), 2.0).expect("an image");
        assert_eq!((image.width(), image.height()), (40, 40));
        assert_eq!(pixel(&image, 10, 10), [255, 0, 0, 255], "inside");
        assert_eq!(pixel(&image, 30, 30)[3], 0, "outside");
    }

    #[test]
    fn translucent_colors_come_out_straight() {
        let mut shape = square();
        shape.fill = Some(Color::rgba(1.0, 0.0, 0.0, 0.5));
        let image = rasterize(&[shape], Rect::new(10.0, 10.0, 20.0, 20.0), 1.0).expect("an image");
        let [r, _, _, a] = pixel(&image, 5, 5);
        assert!(r >= 254, "{r}");
        assert!((126..=129).contains(&a), "{a}");
    }

    #[test]
    fn a_huge_scale_is_held_back() {
        let image = rasterize(&[square()], Rect::new(0.0, 0.0, 800.0, 800.0), 100.0).expect("an image");
        assert!(image.width() <= 4096);
    }

    #[test]
    fn an_empty_shape_is_skipped() {
        let empty = Shape { path: Vec::new(), fill: Some(RED), stroke: None };
        assert!(rasterize(&[empty], Rect::new(0.0, 0.0, 10.0, 10.0), 1.0).is_some());
    }
}
