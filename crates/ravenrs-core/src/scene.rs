//! The display list: an ordered list of primitives in logical pixels.
//!
//! Widgets append primitives in painter's order (back to front). Renderers may
//! batch consecutive primitives of the same kind but must preserve this order.

use std::fmt;
use std::sync::Arc;

use crate::{Color, Point, Rect};

/// How a shape is filled.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum Brush {
    /// A single flat color.
    Solid(Color),
    /// A two-stop linear gradient.
    LinearGradient(LinearGradient),
}

impl From<Color> for Brush {
    fn from(c: Color) -> Self {
        Self::Solid(c)
    }
}

impl From<LinearGradient> for Brush {
    fn from(g: LinearGradient) -> Self {
        Self::LinearGradient(g)
    }
}

/// A two-stop linear gradient. `start` and `end` are relative to the shape's
/// rectangle: `(0, 0)` is its top-left corner and `(1, 1)` its bottom-right.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct LinearGradient {
    /// Gradient start, relative to the shape's rectangle.
    pub start: Point,
    /// Gradient end, relative to the shape's rectangle.
    pub end: Point,
    /// Color at `start`.
    pub from: Color,
    /// Color at `end`.
    pub to: Color,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl LinearGradient {
    /// Top-to-bottom gradient from `from` to `to`.
    pub const fn vertical(from: Color, to: Color) -> Self {
        Self { start: Point::new(0.0, 0.0), end: Point::new(0.0, 1.0), from, to }
    }
}

/// A solid stroke drawn inside a shape's edge.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Border {
    /// Stroke width in logical pixels.
    pub width: f32,
    /// Stroke color.
    pub color: Color,
}

/// A soft drop shadow. `blur` is the Gaussian standard deviation times two,
/// matching what CSS `box-shadow` calls the blur radius.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Shadow {
    /// Shadow offset from the shape, in logical pixels.
    pub offset: Point,
    /// Blur radius in logical pixels (CSS `box-shadow` semantics).
    pub blur: f32,
    /// Shadow color, usually translucent black.
    pub color: Color,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Shadow {
    /// The `shadow-soft` token of Raven Dark: `0 8px 24px rgba(0,0,0,0.35)`.
    pub const fn soft() -> Self {
        Self { offset: Point::new(0.0, 8.0), blur: 24.0, color: Color::rgba(0.0, 0.0, 0.0, 0.35) }
    }

    /// The `shadow-raised` token of Raven Dark: `0 16px 40px rgba(0,0,0,0.45)`.
    pub const fn raised() -> Self {
        Self { offset: Point::new(0.0, 16.0), blur: 40.0, color: Color::rgba(0.0, 0.0, 0.0, 0.45) }
    }
}

/// A rounded rectangle with optional border and shadow. The renderer draws it with
/// a signed-distance function, so corners stay crisp at any scale factor.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Quad {
    /// Bounds of the rectangle, in logical pixels.
    pub rect: Rect,
    /// Corner radii: top-left, top-right, bottom-right, bottom-left.
    pub corner_radii: [f32; 4],
    /// Interior fill.
    pub fill: Brush,
    /// Optional stroke along the edge.
    pub border: Option<Border>,
    /// Optional drop shadow drawn behind the shape.
    pub shadow: Option<Shadow>,
    /// Turned around the rectangle's center by this many radians, clockwise on screen. The
    /// shadow turns around its own center, keeping its offset on screen.
    pub rotation: f32,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Quad {
    /// Creates a sharp-cornered quad with no border or shadow.
    ///
    /// ```
    /// use ravenrs_core::{Color, Quad, Rect, Scene};
    ///
    /// let mut scene = Scene::new();
    /// scene.push_quad(Quad::new(Rect::new(0.0, 0.0, 120.0, 32.0), Color::hex(0x3b82f6)).radius(6.0));
    /// assert_eq!(scene.len(), 1);
    /// ```
    pub fn new(rect: Rect, fill: impl Into<Brush>) -> Self {
        Self { rect, corner_radii: [0.0; 4], fill: fill.into(), border: None, shadow: None, rotation: 0.0 }
    }

    /// Turned around its center by `radians`, clockwise on screen.
    pub fn rotated(mut self, radians: f32) -> Self {
        self.rotation = radians;
        self
    }

    /// Sets all four corner radii to `r` logical pixels.
    pub fn radius(mut self, r: f32) -> Self {
        self.corner_radii = [r; 4];
        self
    }

    /// Adds a border of `width` logical pixels.
    pub fn border(mut self, width: f32, color: Color) -> Self {
        self.border = Some(Border { width, color });
        self
    }

    /// Adds a drop shadow.
    pub fn shadow(mut self, shadow: Shadow) -> Self {
        self.shadow = Some(shadow);
        self
    }
}

/// Font bytes plus a stable identity used as a cache key by the glyph atlas.
#[derive(Clone)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FontRef {
    /// Unique identity of these font bytes; equal ids must mean equal data.
    pub id: u64,
    /// Raw font file bytes (TTF, OTF or a collection).
    pub data: Arc<dyn AsRef<[u8]> + Send + Sync>,
    /// Index inside a font collection; 0 for single-font files.
    pub index: u32,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FontRef {
    /// Borrows the raw font file bytes.
    pub fn bytes(&self) -> &[u8] {
        (*self.data).as_ref()
    }
}

impl fmt::Debug for FontRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FontRef").field("id", &self.id).field("index", &self.index).finish()
    }
}

impl PartialEq for FontRef {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.index == other.index
    }
}

/// One shaped glyph and its pen position.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct PositionedGlyph {
    /// Glyph index inside the font.
    pub id: u32,
    /// Pen position on the baseline, in logical pixels.
    pub x: f32,
    /// Baseline position in logical pixels.
    pub y: f32,
}

/// Shaped glyphs sharing one font, size and color.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct GlyphRun {
    /// Font the glyphs come from.
    pub font: FontRef,
    /// Font size in logical pixels.
    pub size: f32,
    /// Text color.
    pub color: Color,
    /// Glyphs in drawing order.
    pub glyphs: Vec<PositionedGlyph>,
    /// Each glyph turned around its own origin by this many radians, clockwise on screen, for
    /// text that runs at an angle (its origins are placed along that angle by whoever lays it
    /// out, as [`Effect`] does). Turned glyphs lose their snapping to whole pixels.
    pub rotation: f32,
}

/// Decoded pixels: RGBA8, straight (not premultiplied) alpha, sRGB, row-major.
/// Cloning is cheap; the renderer caches the GPU texture by `id`.
#[derive(Clone)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ImageData {
    id: u64,
    width: u32,
    height: u32,
    pixels: Arc<[u8]>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ImageData {
    /// Wraps RGBA8 pixels. Returns `None` when the buffer does not match the size.
    pub fn from_rgba8(width: u32, height: u32, pixels: impl Into<Arc<[u8]>>) -> Option<Self> {
        let pixels = pixels.into();
        if width == 0 || height == 0 || pixels.len() != width as usize * height as usize * 4 {
            return None;
        }
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Some(Self { id, width, height, pixels })
    }

    /// Builds an image by evaluating `f(x, y)` for every pixel (procedural art, tests).
    pub fn from_fn(width: u32, height: u32, f: impl Fn(u32, u32) -> Color) -> Option<Self> {
        let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
        for y in 0..height {
            for x in 0..width {
                let c = f(x, y);
                let to8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                pixels.extend_from_slice(&[to8(c.r), to8(c.g), to8(c.b), to8(c.a)]);
            }
        }
        Self::from_rgba8(width, height, pixels)
    }

    /// Stable identity used as the GPU cache key.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// RGBA8 pixel bytes, row-major, `width * height * 4` long.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Width divided by height.
    pub fn aspect_ratio(&self) -> f32 {
        self.width as f32 / self.height as f32
    }
}

impl fmt::Debug for ImageData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImageData").field("id", &self.id).field("size", &(self.width, self.height)).finish()
    }
}

impl PartialEq for ImageData {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

/// An image drawn into a rectangle, with optional rounded corners.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ImageQuad {
    /// Destination rectangle in logical pixels.
    pub rect: Rect,
    /// Image to draw.
    pub image: ImageData,
    /// The part of the image to show, in normalized coordinates (u0, v0, u1, v1).
    pub uv: [f32; 4],
    /// Corner radii in logical pixels: top-left, top-right, bottom-right, bottom-left.
    pub corner_radii: [f32; 4],
    /// Opacity multiplier, `0.0` to `1.0`.
    pub opacity: f32,
    /// Turned around the rectangle's center by this many radians, clockwise on screen.
    pub rotation: f32,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ImageQuad {
    /// Draws the whole image stretched into `rect`, opaque and sharp-cornered.
    pub fn new(rect: Rect, image: ImageData) -> Self {
        Self { rect, image, uv: [0.0, 0.0, 1.0, 1.0], corner_radii: [0.0; 4], opacity: 1.0, rotation: 0.0 }
    }

    /// Turned around its center by `radians`, clockwise on screen.
    pub fn rotated(mut self, radians: f32) -> Self {
        self.rotation = radians;
        self
    }

    /// Sets all four corner radii to `r` logical pixels.
    pub fn radius(mut self, r: f32) -> Self {
        self.corner_radii = [r; 4];
        self
    }
}

/// One step of a [`PathFill`]'s outline.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum PathElement {
    /// Starts a contour here.
    MoveTo(Point),
    /// A straight line to here.
    LineTo(Point),
    /// A cubic Bézier curve through two control points, ending at the last point.
    CubicTo(Point, Point, Point),
    /// Closes the contour. A contour left open is closed anyway when filled.
    Close,
}

/// Which parts of a self-crossing outline count as inside.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum FillRule {
    /// Inside wherever the contours wind around the point at all.
    #[default]
    NonZero,
    /// Inside wherever an odd number of contours surround the point.
    EvenOdd,
}

/// A color along a gradient, from `0.0` to `1.0`.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ColorStop {
    /// Where along the gradient.
    pub offset: f32,
    /// The color there.
    pub color: Color,
}

/// What fills a [`PathFill`], in the coordinates of its outline.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum PathPaint {
    /// One color.
    Solid(Color),
    /// Colors along the line from `start` to `end`.
    Linear {
        /// Where the first stop is.
        start: Point,
        /// Where the last stop is.
        end: Point,
        /// The colors, by increasing offset.
        stops: Arc<[ColorStop]>,
    },
    /// Colors from `center` outward to `radius`.
    Radial {
        /// Where the first stop is.
        center: Point,
        /// How far the last stop is.
        radius: f32,
        /// The colors, by increasing offset.
        stops: Arc<[ColorStop]>,
    },
    /// An image, seen through the outline.
    Image {
        /// The picture.
        image: ImageData,
        /// Takes the image's own square, from 0 to 1 on each side, to where it goes:
        /// the same six numbers as [`PathFill::transform`].
        placement: [f32; 6],
    },
}

/// What a [`PathFill`] is seen through, in the same coordinates: another outline.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct PathClip {
    /// The outline.
    pub elements: Arc<[PathElement]>,
    /// Which parts of it are inside.
    pub rule: FillRule,
    /// Shows what is outside the outline, instead of what is inside.
    pub inverted: bool,
}

/// A shape made of curves, filled with exact coverage at each pixel, so it stays
/// sharp at any size. A stroke is a fill of its expanded outline.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct PathFill {
    /// The outline.
    pub elements: Arc<[PathElement]>,
    /// Which parts of it are inside.
    pub rule: FillRule,
    /// What fills it.
    pub paint: PathPaint,
    /// `[a, b, c, d, e, f]`, taking `(x, y)` to `(a x + c y + e, b x + d y + f)` in
    /// logical pixels.
    pub transform: [f32; 6],
    /// Multiplies its alpha, from `0.0` to `1.0`.
    pub opacity: f32,
    /// Shows it only through this outline.
    pub clip: Option<Arc<PathClip>>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl PathFill {
    /// The transform that changes nothing.
    pub const IDENTITY: [f32; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

    /// `elements` filled with one color, in logical pixels.
    pub fn solid(elements: impl Into<Arc<[PathElement]>>, color: Color) -> Self {
        Self {
            elements: elements.into(),
            rule: FillRule::NonZero,
            paint: PathPaint::Solid(color),
            transform: Self::IDENTITY,
            opacity: 1.0,
            clip: None,
        }
    }
}

/// One entry of the display list.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum Primitive {
    /// A rounded rectangle.
    Quad(Quad),
    /// A run of shaped glyphs.
    Glyphs(GlyphRun),
    /// A textured rectangle.
    Image(ImageQuad),
    /// A filled shape made of curves.
    Path(PathFill),
    /// Restricts everything until the matching [`Primitive::PopClip`] to the
    /// intersection of this rectangle and the enclosing clip.
    PushClip(Rect),
    /// Restores the clip active before the matching [`Primitive::PushClip`].
    PopClip,
    /// A cable that hangs and swings; a [`CableWorld`](crate::cable::CableWorld) places
    /// its points.
    Cable(Box<crate::cable::Cable>),
}

/// A visual change applied to painted content without touching layout: what a
/// compositor would do to a layer, here done to the display list.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Effect {
    /// Multiplies alpha, `0.0` to `1.0`. Each primitive fades on its own, so
    /// shapes that overlap show through each other while partly faded.
    pub opacity: f32,
    /// Movement in logical pixels.
    pub offset: Point,
    /// Size multiplier around the pivot.
    pub scale: f32,
    /// Turn around the pivot, in radians, clockwise on screen, after scaling.
    pub rotation: f32,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Effect {
    /// No change.
    pub const IDENTITY: Self = Self { opacity: 1.0, offset: Point::new(0.0, 0.0), scale: 1.0, rotation: 0.0 };

    /// Fully transparent.
    pub const fn hidden() -> Self {
        Self { opacity: 0.0, ..Self::IDENTITY }
    }

    /// Moved by `(x, y)` logical pixels.
    pub const fn moved(x: f32, y: f32) -> Self {
        Self { offset: Point::new(x, y), ..Self::IDENTITY }
    }

    /// Scaled around the pivot.
    pub const fn scaled(scale: f32) -> Self {
        Self { scale, ..Self::IDENTITY }
    }

    /// Turned around the pivot by `radians`, clockwise on screen.
    pub const fn rotated(radians: f32) -> Self {
        Self { rotation: radians, ..Self::IDENTITY }
    }

    /// The same effect with this opacity.
    pub const fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    /// The same effect with this scale.
    pub const fn with_scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    /// The effect `t` of the way from `self` to `to`: `0.0` is `self`, `1.0` is `to`.
    /// `t` is not clamped, so springs and overshooting curves carry through.
    pub fn lerp(self, to: Self, t: f32) -> Self {
        let mix = |a: f32, b: f32| a + (b - a) * t;
        Self {
            opacity: mix(self.opacity, to.opacity).clamp(0.0, 1.0),
            offset: Point::new(mix(self.offset.x, to.offset.x), mix(self.offset.y, to.offset.y)),
            scale: mix(self.scale, to.scale).max(0.0),
            rotation: mix(self.rotation, to.rotation),
        }
    }

    /// `self` followed by `next`, as one effect: opacities and scales multiply and
    /// offsets and turns add, the way two nested elements with these effects would look
    /// around one pivot.
    pub fn then(self, next: Self) -> Self {
        Self {
            opacity: self.opacity * next.opacity,
            offset: Point::new(self.offset.x + next.offset.x, self.offset.y + next.offset.y),
            scale: self.scale * next.scale,
            rotation: self.rotation + next.rotation,
        }
    }

    /// True when applying the effect would change nothing.
    pub fn is_identity(&self) -> bool {
        *self == Self::IDENTITY
    }

    /// Where `p` goes: scaled around `pivot`, turned around it, then moved.
    pub fn point(&self, p: Point, pivot: Point) -> Point {
        let (sin, cos) = self.rotation.sin_cos();
        let (x, y) = ((p.x - pivot.x) * self.scale, (p.y - pivot.y) * self.scale);
        Point::new(pivot.x + x * cos - y * sin + self.offset.x, pivot.y + x * sin + y * cos + self.offset.y)
    }

    /// A rectangle of the scaled size around where its center goes; the turn is the
    /// primitive's own [`Quad::rotation`].
    fn rect(&self, r: Rect, pivot: Point) -> Rect {
        let size = r.size.scale(self.scale);
        let c = self.point(r.center(), pivot);
        Rect::new(c.x - size.width / 2.0, c.y - size.height / 2.0, size.width, size.height)
    }

    /// The box around where the corners of `r` go: a clip stays a rectangle on screen.
    fn bounds(&self, r: Rect, pivot: Point) -> Rect {
        let corners = [
            self.point(r.origin, pivot),
            self.point(Point::new(r.max_x(), r.y()), pivot),
            self.point(Point::new(r.max_x(), r.max_y()), pivot),
            self.point(Point::new(r.x(), r.max_y()), pivot),
        ];
        let (mut x0, mut y0, mut x1, mut y1) =
            (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
        for c in corners {
            (x0, y0, x1, y1) = (x0.min(c.x), y0.min(c.y), x1.max(c.x), y1.max(c.y));
        }
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }

    fn color(&self, c: Color) -> Color {
        c.with_alpha(c.a * self.opacity)
    }

    fn apply(&self, primitive: &mut Primitive, pivot: Point) {
        match primitive {
            Primitive::Quad(q) => {
                q.rect = self.rect(q.rect, pivot);
                q.rotation += self.rotation;
                q.corner_radii = q.corner_radii.map(|r| r * self.scale);
                q.fill = match q.fill {
                    Brush::Solid(c) => Brush::Solid(self.color(c)),
                    Brush::LinearGradient(g) => Brush::LinearGradient(LinearGradient {
                        from: self.color(g.from),
                        to: self.color(g.to),
                        ..g
                    }),
                };
                if let Some(b) = &mut q.border {
                    b.width *= self.scale;
                    b.color = self.color(b.color);
                }
                if let Some(s) = &mut q.shadow {
                    s.offset = Point::new(s.offset.x * self.scale, s.offset.y * self.scale);
                    s.blur *= self.scale;
                    s.color = self.color(s.color);
                }
            }
            Primitive::Glyphs(run) => {
                run.size *= self.scale;
                run.rotation += self.rotation;
                run.color = self.color(run.color);
                for g in &mut run.glyphs {
                    let p = self.point(Point::new(g.x, g.y), pivot);
                    (g.x, g.y) = (p.x, p.y);
                }
            }
            Primitive::Path(path) => {
                path.opacity *= self.opacity;
                let [a, b, c, d, e, f] = path.transform;
                let (sin, cos) = self.rotation.sin_cos();
                let k = self.scale;
                // Scale and turn around the pivot, then move: applied after the path's own.
                let turn = |x: f32, y: f32| (k * (cos * x - sin * y), k * (sin * x + cos * y));
                let (na, nb) = turn(a, b);
                let (nc, nd) = turn(c, d);
                let (ne, nf) = turn(e - pivot.x, f - pivot.y);
                path.transform = [na, nb, nc, nd, pivot.x + self.offset.x + ne, pivot.y + self.offset.y + nf];
            }
            Primitive::Image(image) => {
                image.rect = self.rect(image.rect, pivot);
                image.rotation += self.rotation;
                image.corner_radii = image.corner_radii.map(|r| r * self.scale);
                image.opacity *= self.opacity;
            }
            Primitive::PushClip(rect) => *rect = self.bounds(*rect, pivot),
            Primitive::PopClip => {}
            Primitive::Cable(cable) => {
                // The cable keeps its own units; the effect goes into its view (a turn is
                // not carried: cables hang down whatever turns around them).
                let v = cable.view;
                cable.view = crate::cable::CableView {
                    scale: v.scale * self.scale,
                    offset: self.point(v.offset, pivot),
                };
                cable.look.opacity *= self.opacity;
            }
        }
    }
}

impl Default for Effect {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// An ordered display list. Reused between frames to avoid reallocating.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Scene {
    primitives: Vec<Primitive>,
    clear_color: Color,
}

/// A closure of the playground draws into a scene of its own, which is then added to the real
/// one, as a `&mut Scene` cannot leave the paint.
#[cfg(feature = "live")]
impl ravenrs_live::bridge::Scratch for Scene {
    fn fresh() -> Self {
        Self::new()
    }

    fn merge_into(self, target: &mut Self) {
        target.extend_from_slice(self.primitives());
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Scene {
    /// Creates an empty display list with a transparent clear color.
    pub fn new() -> Self {
        Self::default()
    }

    /// Empties the list but keeps its allocation.
    pub fn clear(&mut self) {
        self.primitives.clear();
    }

    /// Sets the color the frame is cleared to before drawing.
    pub fn set_clear_color(&mut self, color: Color) {
        self.clear_color = color;
    }

    /// Color the frame is cleared to before drawing.
    pub fn clear_color(&self) -> Color {
        self.clear_color
    }

    /// Appends a shape made of curves.
    pub fn push_path(&mut self, path: PathFill) {
        self.primitives.push(Primitive::Path(path));
    }

    /// Appends a rounded rectangle.
    pub fn push_quad(&mut self, quad: Quad) {
        self.primitives.push(Primitive::Quad(quad));
    }

    /// Appends a glyph run; empty runs are skipped.
    pub fn push_glyphs(&mut self, run: GlyphRun) {
        if !run.glyphs.is_empty() {
            self.primitives.push(Primitive::Glyphs(run));
        }
    }

    /// Appends a cable.
    pub fn push_cable(&mut self, cable: crate::cable::Cable) {
        self.primitives.push(Primitive::Cable(Box::new(cable)));
    }

    /// Mutable access to the primitives, for the runtime that places cables after the
    /// paint.
    #[doc(hidden)]
    pub fn primitives_mut(&mut self) -> &mut Vec<Primitive> {
        &mut self.primitives
    }

    /// Appends an image.
    pub fn push_image(&mut self, image: ImageQuad) {
        self.primitives.push(Primitive::Image(image));
    }

    /// Clips subsequent primitives to `rect` (intersected with the current clip)
    /// until [`Scene::pop_clip`].
    pub fn push_clip(&mut self, rect: Rect) {
        self.primitives.push(Primitive::PushClip(rect));
    }

    /// Ends the clip started by the matching [`Scene::push_clip`].
    pub fn pop_clip(&mut self) {
        self.primitives.push(Primitive::PopClip);
    }

    /// Applies `effect` to a range of primitives, scaling around `pivot`.
    ///
    /// ```
    /// use ravenrs_core::{Color, Effect, Point, Primitive, Quad, Rect, Scene};
    ///
    /// let mut scene = Scene::new();
    /// scene.push_quad(Quad::new(Rect::new(0.0, 0.0, 10.0, 10.0), Color::WHITE));
    /// scene.apply_effect(0..1, Effect::moved(5.0, 0.0), Point::ZERO);
    /// let Primitive::Quad(q) = &scene.primitives()[0] else { unreachable!() };
    /// assert_eq!(q.rect.x(), 5.0);
    /// ```
    pub fn apply_effect(&mut self, range: std::ops::Range<usize>, effect: Effect, pivot: Point) {
        if effect.is_identity() {
            return;
        }
        if let Some(primitives) = self.primitives.get_mut(range) {
            for primitive in primitives {
                effect.apply(primitive, pivot);
            }
        }
    }

    /// Appends primitives recorded earlier.
    pub fn extend_from_slice(&mut self, primitives: &[Primitive]) {
        self.primitives.extend_from_slice(primitives);
    }

    /// All primitives in painter's order.
    pub fn primitives(&self) -> &[Primitive] {
        &self.primitives
    }

    /// Number of primitives.
    pub fn len(&self) -> usize {
        self.primitives.len()
    }

    /// Returns `true` when no primitives have been pushed.
    pub fn is_empty(&self) -> bool {
        self.primitives.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_data_validates_size_and_gets_unique_ids() {
        assert!(ImageData::from_rgba8(2, 2, vec![0u8; 15]).is_none());
        let a = ImageData::from_rgba8(2, 2, vec![0u8; 16]).unwrap();
        let b = ImageData::from_fn(2, 1, |x, _| if x == 0 { Color::WHITE } else { Color::BLACK }).unwrap();
        assert_ne!(a.id(), b.id());
        assert_eq!(&b.pixels()[..8], &[255, 255, 255, 255, 0, 0, 0, 255]);
        assert_eq!(a, a.clone());
    }

    fn near(a: Point, b: Point) -> bool {
        (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3
    }

    #[test]
    fn a_turn_moves_centers_and_origins_around_the_pivot_and_adds_its_angle() {
        use std::f32::consts::FRAC_PI_2;
        let mut scene = Scene::new();
        scene.push_quad(Quad::new(Rect::new(10.0, -5.0, 20.0, 10.0), Color::WHITE).rotated(0.25));
        scene.push_glyphs(GlyphRun {
            font: FontRef { id: 1, data: Arc::new(Vec::<u8>::new()), index: 0 },
            size: 12.0,
            color: Color::WHITE,
            glyphs: vec![PositionedGlyph { id: 1, x: 10.0, y: 0.0 }],
            rotation: 0.0,
        });
        scene.push_path(PathFill::solid(vec![PathElement::MoveTo(Point::new(10.0, 0.0))], Color::WHITE));
        scene.push_clip(Rect::new(0.0, 0.0, 20.0, 10.0));
        scene.apply_effect(0..4, Effect::rotated(FRAC_PI_2), Point::ZERO);
        let p = scene.primitives();
        let Primitive::Quad(q) = &p[0] else { panic!("a quad") };
        // The center (20, 0) goes to (0, 20); the size stays and the angles add.
        assert!(near(q.rect.center(), Point::new(0.0, 20.0)), "{:?}", q.rect);
        assert_eq!(q.rect.size, crate::Size::new(20.0, 10.0));
        assert!((q.rotation - (0.25 + FRAC_PI_2)).abs() < 1e-6);
        let Primitive::Glyphs(run) = &p[1] else { panic!("glyphs") };
        assert!(near(Point::new(run.glyphs[0].x, run.glyphs[0].y), Point::new(0.0, 10.0)));
        assert!((run.rotation - FRAC_PI_2).abs() < 1e-6);
        let Primitive::Path(path) = &p[2] else { panic!("a path") };
        let [a, b, c, d, e, f] = path.transform;
        let moved = Point::new(a * 10.0 + c * 0.0 + e, b * 10.0 + d * 0.0 + f);
        assert!(near(moved, Point::new(0.0, 10.0)), "{moved:?}");
        let Primitive::PushClip(clip) = &p[3] else { panic!("a clip") };
        assert!(near(clip.origin, Point::new(-10.0, 0.0)) && (clip.width() - 10.0).abs() < 1e-3);
    }

    #[test]
    fn turns_add_and_ease() {
        let a = Effect::rotated(0.5).then(Effect::rotated(0.25).with_scale(2.0));
        assert_eq!((a.rotation, a.scale), (0.75, 2.0));
        assert_eq!(Effect::IDENTITY.lerp(Effect::rotated(1.0), 0.5).rotation, 0.5);
        assert!(Effect::rotated(0.0).is_identity());
        // Scale first, then the turn, then the offset.
        let e =
            Effect { offset: Point::new(1.0, 0.0), ..Effect::rotated(std::f32::consts::PI).with_scale(2.0) };
        assert!(near(e.point(Point::new(3.0, 0.0), Point::new(1.0, 0.0)), Point::new(-2.0, 0.0)));
    }

    #[test]
    fn empty_glyph_runs_are_skipped() {
        let mut scene = Scene::new();
        scene.push_glyphs(GlyphRun {
            font: FontRef { id: 1, data: Arc::new(Vec::<u8>::new()), index: 0 },
            size: 12.0,
            color: Color::WHITE,
            glyphs: vec![],
            rotation: 0.0,
        });
        assert!(scene.is_empty());
        scene.push_quad(Quad::new(Rect::new(0.0, 0.0, 1.0, 1.0), Color::BLACK).radius(4.0));
        assert_eq!(scene.len(), 1);
        scene.clear();
        assert!(scene.is_empty());
    }
}
