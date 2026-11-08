//! [`QuadField`]: a plane of small squares that waves like a floor and, as the page
//! scrolls, stands up into a picture drawn square by square.

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Color, Point, Quad, Rect, Scene};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_style::Semantic;

use crate::View;
use crate::context::{BuildCx, NodeId, Paint};
use crate::scroll_scene::ScrollRange;

type Picture = Rc<dyn Fn(f32, f32, f64) -> Color>;

/// A plane of `columns × rows` squares drawn on every frame. At rest it lies like a
/// floor that waves, lit in the accent and lifting under the pointer; as the scroll
/// progresses through its [`ScrollRange`] it stands up and becomes the picture its
/// function paints, one color per square. Nothing is rebuilt or laid out while it
/// moves. Under reduced motion the floor stands still.
///
/// ```
/// use ravenrs_core::Color;
/// use ravenrs_widgets::QuadField;
///
/// // A blue square on a dark screen.
/// let field = QuadField::new(96, 54, |u, v, _time| {
///     if (0.4..0.6).contains(&u) && (0.4..0.6).contains(&v) { Color::hex(0x4c66ee) } else { Color::hex(0x12151c) }
/// })
/// .height(720.0);
/// ```
pub struct QuadField {
    columns: u16,
    rows: u16,
    picture: Picture,
    height: f32,
    range: ScrollRange,
}

impl fmt::Debug for QuadField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QuadField")
            .field("columns", &self.columns)
            .field("rows", &self.rows)
            .finish_non_exhaustive()
    }
}

impl QuadField {
    /// A field of `columns × rows` squares forming the picture `picture(u, v, seconds)`,
    /// where `u` and `v` run from 0 to 1 across and down.
    pub fn new(columns: u16, rows: u16, picture: impl Fn(f32, f32, f64) -> Color + 'static) -> Self {
        Self {
            columns: columns.max(1),
            rows: rows.max(1),
            picture: Rc::new(picture),
            height: 600.0,
            range: ScrollRange::Scene,
        }
    }

    /// The height of the field (600 by default); it fills the width it is given.
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// The stretch of scrolling over which it stands up ([`ScrollRange::Scene`] by
    /// default: the scene around it, or its own pass across the view outside one).
    pub fn range(mut self, range: ScrollRange) -> Self {
        self.range = range;
        self
    }
}

impl View for QuadField {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let accent = theme.color(Semantic::Accent);
        let glow = theme.color(Semantic::FocusRing);
        let pointer = cx.use_state(|| Rc::new(Cell::new(Point::new(-1e4, -1e4)))).get();
        let progress = Rc::new(Cell::new(0.0_f32));
        let shown = progress.clone();
        let (columns, rows, picture) = (self.columns, self.rows, self.picture.clone());
        let at = pointer.clone();
        let paint = Paint::Live(Rc::new(move |frame: Rect, time: f64, scene: &mut Scene| {
            let e = smooth(shown.get());
            draw(scene, frame, time, e, at.get(), columns, rows, &picture, accent, glow);
        }));
        let node = cx.container(
            LayoutStyle {
                height: Some(self.height),
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[],
            paint,
        );
        cx.scroll_progress(node, self.range, progress);
        cx.on_pointer_move(
            node,
            Rc::new(move |p, frame| pointer.set(Point::new(p.x - frame.x(), p.y - frame.y()))),
        );
        cx.clip(node);
        node
    }
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Paints the field for `e`, from 0 (the floor) to 1 (the picture, facing the viewer).
#[allow(clippy::too_many_arguments)]
fn draw(
    scene: &mut Scene,
    frame: Rect,
    time: f64,
    e: f32,
    pointer: Point,
    columns: u16,
    rows: u16,
    picture: &Picture,
    accent: Color,
    glow: Color,
) {
    let (cw, ch) = (frame.width(), frame.height());
    if cw <= 0.0 || ch <= 0.0 {
        return;
    }
    let t = time as f32;
    let (c, r) = (f32::from(columns), f32::from(rows));
    let w = (cw * 0.9).min(1180.0) / 2.0;
    let s = 2.3 - 1.3 * e;
    let hh = w * r / c;
    let a = 1.22 * (1.0 - e);
    let (ca, sa) = (a.cos(), a.sin());
    let d = 900.0;
    // A screen taller than wide leaves more room below the text over the floor.
    let base = if cw < ch { 0.86 } else { 0.7 };
    let center = Point::new(cw * 0.5, ch * (base - (base - 0.5) * e));
    let amp = 30.0 * (1.0 - e);
    let cell = 2.0 * w / c;
    let sweep = (t * 0.22) % 1.8 - 0.4;
    for j in 0..rows {
        for i in 0..columns {
            let u = (f32::from(i) + 0.5) / c;
            let v = (f32::from(j) + 0.5) / r;
            let x = (u * 2.0 - 1.0) * w * s;
            let py = (v * 2.0 - 1.0) * hh * s;
            let h = amp
                * ((x * 0.006 + t * 1.1).sin()
                    + 0.6 * (py * 0.01 - t * 1.5).sin()
                    + 0.4 * ((x + py) * 0.004 + t * 0.6).sin());
            let dz = d + py * sa + h * ca;
            if dz < 60.0 {
                continue;
            }
            let k = d / dz;
            let mut p = Point::new(center.x + x * k, center.y + (py * ca - h * sa) * k);
            let (dx, dy) = (p.x - pointer.x, p.y - pointer.y);
            let near = (-(dx * dx + dy * dy) / 9000.0).exp();
            p.y -= near * 46.0 * (1.0 - e);
            if p.x < -20.0 || p.x > cw + 20.0 || p.y < -20.0 || p.y > ch + 20.0 {
                continue;
            }
            let fl = ((h / amp.max(0.001) + 1.4) / 2.8).clamp(0.0, 1.0);
            let depth = (k * 0.9).clamp(0.0, 1.0);
            let floor = accent.lerp(glow, 0.45 * fl);
            let floor_alpha = ((0.15 + 0.85 * fl) * depth + near * 0.6).min(1.0);
            let mut shown = picture(u, v, time);
            let shine = (1.0 - (u - sweep - v * 0.3).abs() * 9.0).max(0.0) * 0.25 + near * 0.3;
            shown = shown.lerp(Color::WHITE, shine * e);
            let color = floor.with_alpha(floor_alpha).lerp(shown, e);
            let size = (cell * s * k * (0.2 + 0.66 * e)).max(1.0);
            let rect = Rect::new(frame.x() + p.x - size / 2.0, frame.y() + p.y - size / 2.0, size, size);
            scene.push_quad(Quad::new(rect, color));
        }
    }
}
