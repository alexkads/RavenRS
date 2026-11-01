//! Foundation types shared by every RavenRS crate.
//!
//! `raven-core` has no dependencies. It defines logical-pixel geometry, colors,
//! platform-neutral input events and [`Scene`], the display list that widgets
//! produce and renderers consume. Keeping the display list here lets widgets stay
//! independent of any GPU API.

mod color;
mod event;
mod geometry;
mod scene;

pub use color::Color;
pub use event::{CursorIcon, Event, Key, Modifiers, PointerButton};
pub use geometry::{Insets, Point, Rect, Size};
pub use scene::{
    Border, Brush, FontRef, GlyphRun, ImageData, ImageQuad, LinearGradient, PositionedGlyph, Primitive, Quad,
    Scene, Shadow,
};
