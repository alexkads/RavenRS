//! Draws document pages into a RavenRS [`Scene`] for on-screen previews.

use ravenrs_core::{Color, ImageQuad, Point, Quad, Rect, Scene};
use ravenrs_text::TextSystem;

use crate::document::{Item, Page};
use crate::engine::text_attrs;

/// Paints `page` with its top-left corner at `origin`, `scale` logical pixels
/// per point. Draws the white paper with a soft shadow, then the content.
pub fn paint_page(page: &Page, text: &mut TextSystem, scene: &mut Scene, origin: Point, scale: f32) {
    let at = |x: f32, y: f32| Point::new(origin.x + x * scale, origin.y + y * scale);
    let rect = |r: &Rect| {
        Rect::from_origin_size(
            at(r.x(), r.y()),
            ravenrs_core::Size::new(r.width() * scale, r.height() * scale),
        )
    };
    let paper = Rect::new(origin.x, origin.y, page.width * scale, page.height * scale);
    scene.push_quad(Quad::new(paper, Color::WHITE).shadow(ravenrs_core::Shadow {
        offset: Point::new(0.0, 2.0),
        blur: 12.0,
        color: Color::rgba(0.0, 0.0, 0.0, 0.35),
    }));
    scene.push_clip(paper);
    for item in &page.items {
        match item {
            Item::Rect { rect: r, fill, stroke, radius } => {
                let mut q = Quad::new(rect(r), fill.unwrap_or(Color::TRANSPARENT)).radius(radius * scale);
                if let Some((w, c)) = stroke {
                    q = q.border((w * scale).max(0.5), *c);
                }
                scene.push_quad(q);
            }
            Item::Ellipse { rect: r, fill, stroke } => {
                let r = rect(r);
                let mut q =
                    Quad::new(r, fill.unwrap_or(Color::TRANSPARENT)).radius(r.width().min(r.height()) / 2.0);
                if let Some((w, c)) = stroke {
                    q = q.border((w * scale).max(0.5), *c);
                }
                scene.push_quad(q);
            }
            Item::Line { from, to, width, color } => {
                // Lines are axis-aligned in practice (borders, rules); a slanted
                // one is drawn as a thin box along its bounding rectangle's diagonal span.
                let (a, b) = (at(from.x, from.y), at(to.x, to.y));
                let w = (width * scale).max(0.75);
                let r = if (a.y - b.y).abs() < 0.01 {
                    Rect::new(a.x.min(b.x), a.y - w / 2.0, (a.x - b.x).abs(), w)
                } else if (a.x - b.x).abs() < 0.01 {
                    Rect::new(a.x - w / 2.0, a.y.min(b.y), w, (a.y - b.y).abs())
                } else {
                    // Approximate with dots along the segment.
                    let steps = ((b.x - a.x).hypot(b.y - a.y) / w).ceil().max(1.0) as usize;
                    for i in 0..=steps {
                        let t = i as f32 / steps as f32;
                        let p = Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
                        scene.push_quad(
                            Quad::new(Rect::new(p.x - w / 2.0, p.y - w / 2.0, w, w), *color).radius(w / 2.0),
                        );
                    }
                    continue;
                };
                scene.push_quad(Quad::new(r, *color));
            }
            Item::Text(t) => {
                scene.push_clip(rect(&t.clip));
                let attrs = text_attrs(t.size * scale, t.bold);
                let attrs = ravenrs_text::TextAttrs { line_height: t.line_height * scale, ..attrs };
                for line in &t.lines {
                    let mut layout = text.layout(&line.text, &attrs);
                    layout.measure(None);
                    layout.paint(at(line.x, line.top), t.color, scene);
                }
                scene.pop_clip();
            }
            Item::Image { rect: r, clip, image } => {
                scene.push_clip(rect(clip));
                scene.push_image(ImageQuad::new(rect(r), image.pixels().clone()));
                scene.pop_clip();
            }
            Item::Marks { color, marks, .. } => {
                for mark in marks {
                    scene.push_quad(Quad::new(rect(mark), *color));
                }
            }
            // Nothing to print: they matter in the exported file.
            Item::Link { .. } | Item::Bookmark { .. } => {}
        }
    }
    scene.pop_clip();
}
