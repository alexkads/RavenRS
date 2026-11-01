//! Foundation types shared by every RavenRS crate.
//!
//! `ravenrs-core` has no dependencies. It defines logical-pixel geometry, colors,
//! platform-neutral input events, [`Scene`] (the display list that widgets
//! produce and renderers consume) and [`AccessTree`] (what assistive technology
//! sees). Keeping them here lets widgets stay independent of any GPU or
//! operating-system API. [`csv`] is here because the grid and the report
//! exporters share nothing else lower down.

mod access;
mod color;
pub mod csv;
mod event;
mod geometry;
mod scene;

pub use access::{AccessAction, AccessNode, AccessTree, NumericValue, Role, Toggled};
pub use color::Color;
pub use event::{CursorIcon, Event, Key, Modifiers, PointerButton};
pub use geometry::{Insets, Point, Rect, Size};
pub use scene::{
    Border, Brush, Effect, FontRef, GlyphRun, ImageData, ImageQuad, LinearGradient, PositionedGlyph,
    Primitive, Quad, Scene, Shadow,
};
