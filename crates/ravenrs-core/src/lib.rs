//! Foundation types shared by every RavenRS crate.
//!
//! `ravenrs-core` has no dependencies. It defines logical-pixel geometry, colors,
//! platform-neutral input events, [`Scene`] (the display list that widgets
//! produce and renderers consume) and [`AccessTree`] (what assistive technology
//! sees). Keeping them here lets widgets stay independent of any GPU or
//! operating-system API.

mod access;
mod color;
mod event;
mod geometry;
mod scene;

pub use access::{AccessAction, AccessNode, AccessTree, NumericValue, Role, Toggled};
pub use color::Color;
pub use event::{CursorIcon, Event, Key, Modifiers, PointerButton};
pub use geometry::{Insets, Point, Rect, Size};
pub use scene::{
    Border, Brush, FontRef, GlyphRun, ImageData, ImageQuad, LinearGradient, PositionedGlyph, Primitive, Quad,
    Scene, Shadow,
};
