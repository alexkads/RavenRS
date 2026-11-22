//! Foundation types shared by every RavenRS crate.
//!
//! `ravenrs-core` has no dependencies. It defines logical-pixel geometry, colors,
//! platform-neutral input events, [`Scene`] (the display list that widgets
//! produce and renderers consume) and [`AccessTree`] (what assistive technology
//! sees). Keeping them here lets widgets stay independent of any GPU or
//! operating-system API. [`csv`] is here because the grid and the report
//! exporters share nothing else lower down. [`cable`] holds the rope model behind
//! `CablePlug`, shared by the widgets, the exporters and the renderer.

mod access;
pub mod cable;
mod color;
pub mod csv;
mod diagnostics;
mod event;
mod geometry;
mod scene;
mod storage;
mod tray;

pub use access::{AccessAction, AccessNode, AccessTree, NumericValue, Role, Toggled};
pub use color::Color;
pub use diagnostics::{
    AdapterReport, Bottleneck, BuildCause, BuildCauses, DiagnosticFinding, DiagnosticReport, FrameLog,
    FrameSample, FrameSummary, FrameWork, GpuReport, MAX_ISSUE_URL, Percentiles, PerformanceResult,
    PerformanceVerdict, PhaseTimes, RenderWork, SurfaceReport, SystemReport, ViewCost, new_issue_url,
    redact_user_paths,
};
pub use event::{CursorIcon, Event, Key, Modifiers, PointerButton};
pub use geometry::{Insets, Point, Rect, Size, monotone_slopes};
pub use scene::{
    Border, Brush, ColorStop, Effect, FillRule, FontRef, GlyphRun, ImageData, ImageQuad, LinearGradient,
    PathClip, PathElement, PathFill, PathPaint, PositionedGlyph, Primitive, Quad, Scene, Shadow,
};
#[cfg(not(target_arch = "wasm32"))]
pub use storage::FileStorage;
pub use storage::{MemoryStorage, Storage};
pub use tray::{OPENS_WINDOW, TrayEntry, TrayMenu};
