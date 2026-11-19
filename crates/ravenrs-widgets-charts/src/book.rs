//! The stories of the components of this crate, for RavenComponentBook: see
//! `ravenrs_widgets_base::book`.

#[allow(unused_imports)]
use ravenrs_widgets_base::book::{ComponentBook, Meta};

/// The stories of the components of this crate that give them, in the order the book lists
/// them.
pub fn stories() -> Vec<Meta> {
    generated()
}

/// The diagram the stories of the flow panels share.
#[cfg(feature = "flow")]
mod flow_graph_sample;

// rcb-scaffold: begin
#[cfg(feature = "charts")]
mod area_chart;
#[cfg(feature = "charts")]
mod bar_chart;
#[cfg(feature = "charts")]
mod barcode;
#[cfg(feature = "charts")]
mod chart;
#[cfg(feature = "charts")]
mod chart_card;
#[cfg(feature = "flow")]
mod flow_canvas;
#[cfg(feature = "flow")]
mod flow_inspector;
#[cfg(feature = "flow")]
mod flow_layers;
#[cfg(feature = "flow")]
mod flow_library;
#[cfg(feature = "flow")]
mod flow_toolbox;
#[cfg(feature = "charts")]
mod sankey_chart;
#[allow(unused_imports)]
use ravenrs_widgets_base::AnyView;

/// The stories written by scripts/rcb-scaffold.py, in name order.
#[allow(
    unused_mut,
    clippy::vec_init_then_push,
    reason = "an entry may sit behind a feature, as its component does"
)]
pub(crate) fn generated() -> Vec<Meta> {
    let mut all = Vec::new();
    #[cfg(feature = "charts")]
    all.push(<crate::AreaChart as ravenrs_widgets_base::book::ComponentBook>::book());
    #[cfg(feature = "charts")]
    all.push(<crate::BarChart as ravenrs_widgets_base::book::ComponentBook>::book());
    #[cfg(feature = "charts")]
    all.push(<crate::Barcode as ravenrs_widgets_base::book::ComponentBook>::book());
    #[cfg(feature = "charts")]
    all.push(<crate::Chart as ravenrs_widgets_base::book::ComponentBook>::book());
    #[cfg(feature = "charts")]
    all.push(<crate::ChartCard<AnyView> as ravenrs_widgets_base::book::ComponentBook>::book());
    #[cfg(feature = "flow")]
    all.push(<crate::FlowCanvas as ravenrs_widgets_base::book::ComponentBook>::book());
    #[cfg(feature = "flow")]
    all.push(<crate::FlowInspector as ravenrs_widgets_base::book::ComponentBook>::book());
    #[cfg(feature = "flow")]
    all.push(<crate::FlowLayers as ravenrs_widgets_base::book::ComponentBook>::book());
    #[cfg(feature = "flow")]
    all.push(<crate::FlowLibrary as ravenrs_widgets_base::book::ComponentBook>::book());
    #[cfg(feature = "flow")]
    all.push(<crate::FlowToolbox as ravenrs_widgets_base::book::ComponentBook>::book());
    #[cfg(feature = "charts")]
    all.push(<crate::SankeyChart as ravenrs_widgets_base::book::ComponentBook>::book());
    all
}
// rcb-scaffold: end
