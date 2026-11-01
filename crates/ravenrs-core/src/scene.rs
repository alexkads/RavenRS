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

impl LinearGradient {
    /// Top-to-bottom gradient from `from` to `to`.
    pub const fn vertical(from: Color, to: Color) -> Self {
        Self { start: Point::new(0.0, 0.0), end: Point::new(0.0, 1.0), from, to }
    }
}

/// A solid stroke drawn inside a shape's edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Border {
    /// Stroke width in logical pixels.
    pub width: f32,
    /// Stroke color.
    pub color: Color,
}

/// A soft drop shadow. `blur` is the Gaussian standard deviation times two,
/// matching what CSS `box-shadow` calls the blur radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    /// Shadow offset from the shape, in logical pixels.
    pub offset: Point,
    /// Blur radius in logical pixels (CSS `box-shadow` semantics).
    pub blur: f32,
    /// Shadow color, usually translucent black.
    pub color: Color,
}

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
}

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
        Self { rect, corner_radii: [0.0; 4], fill: fill.into(), border: None, shadow: None }
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
pub struct FontRef {
    /// Unique identity of these font bytes; equal ids must mean equal data.
    pub id: u64,
    /// Raw font file bytes (TTF, OTF or a collection).
    pub data: Arc<dyn AsRef<[u8]> + Send + Sync>,
    /// Index inside a font collection; 0 for single-font files.
    pub index: u32,
}

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
pub struct GlyphRun {
    /// Font the glyphs come from.
    pub font: FontRef,
    /// Font size in logical pixels.
    pub size: f32,
    /// Text color.
    pub color: Color,
    /// Glyphs in drawing order.
    pub glyphs: Vec<PositionedGlyph>,
}

/// Decoded pixels: RGBA8, straight (not premultiplied) alpha, sRGB, row-major.
/// Cloning is cheap; the renderer caches the GPU texture by `id`.
#[derive(Clone)]
pub struct ImageData {
    id: u64,
    width: u32,
    height: u32,
    pixels: Arc<[u8]>,
}

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
}

impl ImageQuad {
    /// Draws the whole image stretched into `rect`, opaque and sharp-cornered.
    pub fn new(rect: Rect, image: ImageData) -> Self {
        Self { rect, image, uv: [0.0, 0.0, 1.0, 1.0], corner_radii: [0.0; 4], opacity: 1.0 }
    }

    /// Sets all four corner radii to `r` logical pixels.
    pub fn radius(mut self, r: f32) -> Self {
        self.corner_radii = [r; 4];
        self
    }
}

/// One entry of the display list.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Primitive {
    /// A rounded rectangle.
    Quad(Quad),
    /// A run of shaped glyphs.
    Glyphs(GlyphRun),
    /// A textured rectangle.
    Image(ImageQuad),
    /// Restricts everything until the matching [`Primitive::PopClip`] to the
    /// intersection of this rectangle and the enclosing clip.
    PushClip(Rect),
    /// Restores the clip active before the matching [`Primitive::PushClip`].
    PopClip,
}

/// An ordered display list. Reused between frames to avoid reallocating.
#[derive(Clone, Debug, Default)]
pub struct Scene {
    primitives: Vec<Primitive>,
    clear_color: Color,
}

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

    #[test]
    fn empty_glyph_runs_are_skipped() {
        let mut scene = Scene::new();
        scene.push_glyphs(GlyphRun {
            font: FontRef { id: 1, data: Arc::new(Vec::<u8>::new()), index: 0 },
            size: 12.0,
            color: Color::WHITE,
            glyphs: vec![],
        });
        assert!(scene.is_empty());
        scene.push_quad(Quad::new(Rect::new(0.0, 0.0, 1.0, 1.0), Color::BLACK).radius(4.0));
        assert_eq!(scene.len(), 1);
        scene.clear();
        assert!(scene.is_empty());
    }
}
