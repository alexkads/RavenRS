//! Panels docked in a tree of splits and tabs: the arrangement and its arithmetic, which need no
//! UI, apart from the drawing.

mod area;
mod geometry;
mod layout;

pub use area::{DockArea, DockGuard, DockPanel, DockTabBar};
pub use layout::{DockChange, DockChild, DockDrop, DockEdge, DockLayout, DockLayoutError, DockNode};
