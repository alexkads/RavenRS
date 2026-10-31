//! The display list: an ordered list of primitives in logical pixels.
//!
//! Widgets append primitives in painter's order (back to front). Renderers may
//! batch consecutive primitives of the same kind but must preserve this order.

use std::fmt;
use std::sync::Arc;

use crate::{Color, Point, Rect};

/// How a shape is filled.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Brush {
    Solid(Color),
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
    pub start: Point,
    pub end: Point,
    pub from: Color,
    pub to: Color,
}

impl LinearGradient {
    pub const fn vertical(from: Color, to: Color) -> Self {
        Self { start: Point::new(0.0, 0.0), end: Point::new(0.0, 1.0), from, to }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Border {
    pub width: f32,
    pub color: Color,
}

/// A soft drop shadow. `blur` is the Gaussian standard deviation times two,
/// matching what CSS `box-shadow` calls the blur radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub offset: Point,
    pub blur: f32,
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
    pub rect: Rect,
    /// Corner radii: top-left, top-right, bottom-right, bottom-left.
    pub corner_radii: [f32; 4],
    pub fill: Brush,
    pub border: Option<Border>,
    pub shadow: Option<Shadow>,
}

impl Quad {
    pub fn new(rect: Rect, fill: impl Into<Brush>) -> Self {
        Self { rect, corner_radii: [0.0; 4], fill: fill.into(), border: None, shadow: None }
    }

    pub fn radius(mut self, r: f32) -> Self {
        self.corner_radii = [r; 4];
        self
    }

    pub fn border(mut self, width: f32, color: Color) -> Self {
        self.border = Some(Border { width, color });
        self
    }

    pub fn shadow(mut self, shadow: Shadow) -> Self {
        self.shadow = Some(shadow);
        self
    }
}

/// Font bytes plus a stable identity used as a cache key by the glyph atlas.
#[derive(Clone)]
pub struct FontRef {
    pub id: u64,
    pub data: Arc<dyn AsRef<[u8]> + Send + Sync>,
    /// Index inside a font collection; 0 for single-font files.
    pub index: u32,
}

impl FontRef {
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PositionedGlyph {
    pub id: u32,
    /// Pen position on the baseline, in logical pixels.
    pub x: f32,
    pub y: f32,
}

/// Shaped glyphs sharing one font, size and color.
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphRun {
    pub font: FontRef,
    /// Font size in logical pixels.
    pub size: f32,
    pub color: Color,
    pub glyphs: Vec<PositionedGlyph>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Primitive {
    Quad(Quad),
    Glyphs(GlyphRun),
    /// Restricts everything until the matching [`Primitive::PopClip`] to the
    /// intersection of this rectangle and the enclosing clip.
    PushClip(Rect),
    PopClip,
}

/// An ordered display list. Reused between frames to avoid reallocating.
#[derive(Clone, Debug, Default)]
pub struct Scene {
    primitives: Vec<Primitive>,
    clear_color: Color,
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    /// Empties the list but keeps its allocation.
    pub fn clear(&mut self) {
        self.primitives.clear();
    }

    pub fn set_clear_color(&mut self, color: Color) {
        self.clear_color = color;
    }

    pub fn clear_color(&self) -> Color {
        self.clear_color
    }

    pub fn push_quad(&mut self, quad: Quad) {
        self.primitives.push(Primitive::Quad(quad));
    }

    pub fn push_glyphs(&mut self, run: GlyphRun) {
        if !run.glyphs.is_empty() {
            self.primitives.push(Primitive::Glyphs(run));
        }
    }

    /// Clips subsequent primitives to `rect` (intersected with the current clip)
    /// until [`Scene::pop_clip`].
    pub fn push_clip(&mut self, rect: Rect) {
        self.primitives.push(Primitive::PushClip(rect));
    }

    pub fn pop_clip(&mut self) {
        self.primitives.push(Primitive::PopClip);
    }

    pub fn primitives(&self) -> &[Primitive] {
        &self.primitives
    }

    pub fn len(&self) -> usize {
        self.primitives.len()
    }

    pub fn is_empty(&self) -> bool {
        self.primitives.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
