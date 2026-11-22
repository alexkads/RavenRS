//! RavenRS charts and diagrams: `Chart` and its kinds, `Barcode`, `SankeyChart` (feature `charts`)
//! and `FlowCanvas` (feature `flow`), drawn with `ravenrs-graphics` like the reports draw them.
//!
//! Applications depend on `ravenrs-widgets` (or `ravenrs`), which re-exports all of this by the
//! same names.

// The base and the kit, by the names the components use: `crate::Button`, `crate::Tabs`,
// `crate::context::BuildCx`.
/// The stories of the components of this crate (feature `book`).
#[cfg(feature = "book")]
pub mod book;

#[allow(unused_imports)]
use ravenrs_widgets_base::*;
#[allow(unused_imports)]
use ravenrs_widgets_kit::*;

#[cfg(feature = "charts")]
#[doc(hidden)]
pub mod chart;
#[cfg(feature = "flow")]
#[doc(hidden)]
pub mod flow;
#[cfg(feature = "flow")]
#[doc(hidden)]
pub mod flow_inspector;
#[cfg(feature = "flow")]
#[doc(hidden)]
pub mod flow_layers;
#[cfg(feature = "flow")]
#[doc(hidden)]
pub mod flow_library;
#[cfg(feature = "flow")]
#[doc(hidden)]
pub mod flow_notation;
#[cfg(feature = "flow")]
#[doc(hidden)]
pub mod flow_toolbox;
#[cfg(feature = "charts")]
#[doc(hidden)]
pub mod sankey;
#[cfg(feature = "testing")]
pub mod testing;

#[cfg(feature = "charts")]
pub use chart::{
    AreaChart, BarChart, Barcode, Chart, ChartBarFrom, ChartCard, ChartCurve, ChartGrid, ChartIndicator,
    ChartKind, ChartLabelAt, ChartLegend, ChartPolarGrid, ChartReference, ChartStack, ChartTooltip,
    Symbology,
};
#[cfg(feature = "flow")]
pub use flow::{
    EdgeCap, EdgeShape, FLOW_FORMAT, FLOW_NODE_DRAG, FlowCanvas, FlowChange, FlowColors, FlowConnector,
    FlowContent, FlowDirection, FlowDocument, FlowEdge, FlowEndpoint, FlowExportFormat, FlowExporter,
    FlowGate, FlowGraph, FlowGuard, FlowHistory, FlowIssue, FlowJsonError, FlowKind, FlowLayout,
    FlowLayoutKind, FlowNext, FlowNode, FlowNodeRun, FlowNodeStatus, FlowPicture, FlowPort, FlowRun,
    FlowRunEvent, FlowRunRules, FlowRunView, FlowSelection, FlowShape, FlowShelf, FlowStyle, FlowText,
    FlowValidator, FlowViewport, NODE_REFS, PortDirection, PortSide,
};
#[cfg(feature = "flow")]
pub use flow_inspector::FlowInspector;
#[cfg(feature = "flow")]
pub use flow_layers::FlowLayers;
#[cfg(feature = "flow")]
pub use flow_library::FlowLibrary;
#[cfg(feature = "flow")]
pub use flow_notation::{FlowChecks, FlowFileFormat, FlowNotation};
#[cfg(feature = "flow")]
pub use flow_toolbox::FlowToolbox;
#[cfg(feature = "charts")]
pub use sankey::{SankeyAlign, SankeyChart};
