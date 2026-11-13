//! RavenRS `DockArea`, its arrangement (`dock::layout`), geometry (`dock::geometry`) and drawing
//! (`dock::area`), and the `Ribbon`.
//!
//! Applications depend on `ravenrs-widgets` (or `ravenrs`), which re-exports all of this by the
//! same names.

// The base and the kit, by the names the components use: `crate::Button`, `crate::Tabs`,
// `crate::context::BuildCx`.
#[allow(unused_imports)]
use ravenrs_widgets_base::*;
#[allow(unused_imports)]
use ravenrs_widgets_kit::*;

#[doc(hidden)]
pub mod dock;
#[doc(hidden)]
pub mod ribbon;

pub use dock::{
    DockArea, DockChild, DockDrop, DockEdge, DockLayout, DockLayoutError, DockNode, DockPanel, DockTabBar,
};
pub use ribbon::{Ribbon, RibbonGroup, RibbonItem, RibbonPage, RibbonSize};
